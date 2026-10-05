mod audio;
mod classic;
mod font;
mod input;
mod music;
mod pico;
mod render;
#[cfg(test)]
mod tests;

use classic::Classic;
use input::{Cmd, Input};
use ratatui::crossterm::event::{self, DisableFocusChange, EnableFocusChange, Event, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;
use std::io;
use std::time::{Duration, Instant};

/// PICO-8 runs Celeste at 30 frames a second.
const FRAME: Duration = Duration::from_micros(33_333);
const SNAPSHOT_EVERY: Duration = Duration::from_millis(500);

const HELP: &str = "ceres [--mute] [--no-shake] [--level N] [--seed N]

Celeste Classic, restored for the terminal. Climb the mountain.

  arrows / WASD   move
  Z C N Space     jump
  X V M           dash (eight ways)
  Esc / P         menu

  --level N       start at level N (1-31; 31 is the summit)
  --no-shake      no screen shake
  --sharp         exact 1:1 pixels, even where the font makes them tall
  --mute          no sound

Needs a terminal that reports key releases (the kitty keyboard protocol:
kitty, foot, WezTerm, Ghostty, Alacritty, iTerm2 and others), or a gamepad.
Session log: RUST_LOG=debug ceres 2>ceres.log";

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum Screen {
    /// The terminal can't report releases and there is no gamepad.
    Notice,
    Play,
    Menu,
}

fn main() -> io::Result<()> {
    env_logger::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{HELP}");
        return Ok(());
    }
    let arg_after = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1));
    let level = arg_after("--level").and_then(|v| v.parse::<i32>().ok()).map(|n| n.clamp(1, 31) - 1);
    let seed = arg_after("--seed").and_then(|v| v.parse::<u64>().ok()).unwrap_or_else(|| {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1)
    });

    // open audio before the alternate screen, so any ALSA chatter lands above it
    let mut audio = audio::Audio::new(args.iter().any(|a| a == "--mute"));
    log::info!("audio available {} muted {}", audio.available(), audio.muted());

    let mut terminal = ratatui::init();
    let kitty = supports_keyboard_enhancement().unwrap_or(false);
    if kitty {
        let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES | KeyboardEnhancementFlags::REPORT_EVENT_TYPES | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES;
        execute!(io::stdout(), PushKeyboardEnhancementFlags(flags))?;
    }
    let _ = execute!(io::stdout(), EnableFocusChange);
    let mut input = Input::new(kitty);
    input.open_pad();
    log::info!("start kitty_keyboard={kitty} pad={:?} seed={seed}", input.pad_name);

    let mut game = Classic::new(seed);
    game.screen_shake = !args.iter().any(|a| a == "--no-shake");
    if let Some(l) = level {
        // what a climb from the start would have by now: the orb's second
        // dash and pink sky after level 22, and that stretch's music
        if l > 21 {
            game.max_djump = 2;
            game.new_bg = true;
        }
        game.e.music.clear();
        game.e.music(match l {
            0..=10 => 0,
            12..=20 => 20,
            22..=29 => 10,
            _ => 30,
        });
        game.load_room(l % 8, l / 8);
    }
    let screen = if kitty || input.pad_name.is_some() { Screen::Play } else { Screen::Notice };
    let square = !args.iter().any(|a| a == "--sharp");
    let result = run(&mut terminal, &mut game, &mut input, &mut audio, screen, square);

    let _ = execute!(io::stdout(), DisableFocusChange);
    if kitty {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, game: &mut Classic, input: &mut Input, audio: &mut audio::Audio, mut screen: Screen, mut square: bool) -> io::Result<()> {
    let mut aspect = render::cell_aspect();
    log::info!("cell aspect {aspect:.3}");
    let mut next = Instant::now();
    let mut snapshot = Instant::now();
    let mut quit_armed = false;
    loop {
        // input until the next frame is due
        loop {
            let wait = next.saturating_duration_since(Instant::now());
            if !event::poll(wait)? {
                break;
            }
            match event::read()? {
                Event::Key(k) => {
                    log::debug!("key {:?} {:?} {:?}", k.code, k.kind, k.modifiers);
                    input.on_key(k, Instant::now());
                }
                Event::FocusLost => {
                    log::debug!("focus lost: releasing all keys");
                    input.release_all();
                }
                Event::Resize(w, h) => {
                    aspect = render::cell_aspect();
                    log::info!("resize {w}x{h} cell aspect {aspect:.3}");
                }
                _ => {}
            }
        }
        next += FRAME;
        let now = Instant::now();
        if next < now {
            // fell behind (suspended, slow terminal): don't race to catch up
            next = now + FRAME;
        }

        input.poll_pad();
        for cmd in std::mem::take(&mut input.cmds) {
            log::debug!("cmd {cmd:?} on {screen:?}");
            match (screen, cmd) {
                (_, Cmd::Exit) => return Ok(()),
                (Screen::Notice, Cmd::Confirm) => screen = Screen::Play,
                (Screen::Notice, Cmd::Menu | Cmd::Quit) => return Ok(()),
                (Screen::Play, Cmd::Menu) => {
                    screen = Screen::Menu;
                    quit_armed = false;
                }
                (Screen::Menu, Cmd::Menu) => screen = Screen::Play,
                (Screen::Menu, Cmd::Restart) => {
                    log::info!("restart from the menu");
                    let shake = game.screen_shake;
                    *game = Classic::new(game.frames as u64 ^ 0x9e37_79b9_7f4a_7c15);
                    game.screen_shake = shake;
                    screen = Screen::Play;
                }
                (Screen::Menu, Cmd::Quit) => {
                    if quit_armed {
                        return Ok(());
                    }
                    quit_armed = true;
                }
                (Screen::Menu, Cmd::Mute) | (Screen::Play, Cmd::Mute) => {
                    audio.set_muted(!audio.muted());
                    log::info!("muted {}", audio.muted());
                }
                (Screen::Menu, Cmd::Aspect) => {
                    square = !square;
                    log::info!("square pixels {square}");
                }
                (Screen::Menu, Cmd::Shake) => {
                    game.screen_shake = !game.screen_shake;
                    log::info!("screen shake {}", game.screen_shake);
                }
                _ => {}
            }
        }

        let buttons = input.buttons(Instant::now());
        if screen == Screen::Play {
            game.e.btn = buttons;
            game.update();
            if game.freeze <= 0 {
                game.draw();
            }
            for n in std::mem::take(&mut game.e.sfx) {
                audio.sfx(n);
            }
            for m in std::mem::take(&mut game.e.music) {
                audio.music(m);
            }
        }

        terminal.draw(|f| {
            let full = f.area();
            let rect = render::screen_rect(full, square, aspect);
            let buf = f.buffer_mut();
            render::draw_screen(buf, rect, &game.e);
            let left = if game.is_title() {
                " ceres".to_string()
            } else {
                format!(" {}  berries {}  deaths {}  {}:{:02}:{:02}", level_name(game), game.got_fruit.len(), game.deaths, game.minutes / 60, game.minutes % 60, game.seconds)
            };
            let keys = if input.exact || input.pad_name.is_some() { "" } else { "  (no key releases)" };
            render::draw_status(buf, full, rect, &left, &format!("esc menu{keys} "));
            match screen {
                Screen::Notice => render::draw_box(
                    buf,
                    rect,
                    "ceres",
                    &[
                        "This terminal does not report key releases".into(),
                        "(the kitty keyboard protocol), and no gamepad".into(),
                        "is connected. Ceres times jumps and dashes by".into(),
                        "when keys go down and up, so on this terminal".into(),
                        "they will feel wrong.".into(),
                        String::new(),
                        "kitty, foot, WezTerm, Ghostty, Alacritty and".into(),
                        "iTerm2 report releases; a gamepad works anywhere.".into(),
                        String::new(),
                        "Enter: play anyway    Esc: quit".into(),
                    ],
                ),
                Screen::Menu => render::draw_box(
                    buf,
                    rect,
                    "paused",
                    &[
                        "esc  continue".into(),
                        "r    restart from the title".into(),
                        format!("k    screen shake: {}", if game.screen_shake { "on" } else { "off" }),
                        format!("a    pixels: {}", if square { "square" } else { "sharp" }),
                        format!("f8   sound: {}", if audio.muted() { "off" } else { "on" }),
                        if quit_armed { "q    press again to quit".into() } else { "q    quit".into() },
                        String::new(),
                        "arrows move  z jump  x dash".into(),
                    ],
                ),
                Screen::Play => {}
            }
        })?;

        if snapshot.elapsed() >= SNAPSHOT_EVERY {
            snapshot = Instant::now();
            match game.player() {
                Some(p) => {
                    let djump = match &p.kind {
                        classic::Kind::Player(d) => d.djump,
                        _ => 0,
                    };
                    log::debug!(
                        "snapshot {:?} room {:?} player ({}, {}) spd ({:.2}, {:.2}) djump {} btn {:?} deaths {} berries {}",
                        screen,
                        game.room,
                        p.x,
                        p.y,
                        p.spd.x,
                        p.spd.y,
                        djump,
                        buttons,
                        game.deaths,
                        game.got_fruit.len()
                    );
                }
                None => log::debug!("snapshot {:?} room {:?} no player, btn {:?}", screen, game.room, buttons),
            }
        }
    }
}

fn level_name(game: &Classic) -> String {
    match game.level_index() {
        30 => "summit".into(),
        _ if game.room == (3, 1) => "old site".into(),
        l => format!("{}m", (l + 1) * 100),
    }
}
