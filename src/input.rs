//! Keyboard and gamepad, boiled down to PICO-8's six buttons.
//!
//! - Move: arrow keys or WASD, the d-pad or the left stick.
//! - Jump: Z, C, N or Space; pad A or Y.
//! - Dash: X, V or M; pad B, X or a shoulder button.
//!
//! The game reads buttons once per 30 Hz frame, and jump height and dashes
//! depend on exactly when a button goes down and comes up. With the kitty
//! keyboard protocol presses and releases are exact; a press is also latched
//! until the next frame reads it, so a tap shorter than a frame still counts.
//! Without the protocol the terminal only sends key repeats, and a key counts
//! as held while they keep arriving (the Panzer heuristic): playable, but
//! jumps and dashes will not feel right, which the game says up front.

use crate::pico::{DASH, DOWN, JUMP, LEFT, RIGHT, UP};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};

const PRE_REPEAT: Duration = Duration::from_millis(500);
const REPEAT_GAP: Duration = Duration::from_millis(100);
#[cfg(feature = "gamepad")]
const STICK: f32 = 0.5;

/// Commands outside the game itself, as edges.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cmd {
    Menu,
    Confirm,
    Quit,
    Restart,
    Mute,
    Shake,
    Aspect,
    Exit,
}

#[derive(Clone, Copy, Default)]
struct Key {
    down: bool,
    /// Pressed since the game last read the buttons.
    latched: bool,
    last: Option<Instant>,
    repeats: u32,
}

#[cfg(feature = "gamepad")]
fn usable_pad(p: &gilrs::Gamepad) -> bool {
    p.mapping_source() == gilrs::MappingSource::SdlMappings || std::env::var("CERES_ANY_PAD").is_ok_and(|v| v == "1")
}

pub struct Input {
    pub exact: bool,
    keys: [Key; 6],
    pub cmds: Vec<Cmd>,
    #[cfg(feature = "gamepad")]
    gilrs: Option<gilrs::Gilrs>,
    pub pad_name: Option<String>,
    pad: [bool; 6],
    pad_latched: [bool; 6],
}

impl Input {
    pub fn new(exact: bool) -> Input {
        Input {
            exact,
            keys: [Key::default(); 6],
            cmds: Vec::new(),
            #[cfg(feature = "gamepad")]
            gilrs: None,
            pad_name: None,
            pad: [false; 6],
            pad_latched: [false; 6],
        }
    }

    pub fn on_key(&mut self, k: KeyEvent, now: Instant) {
        let press = k.kind != KeyEventKind::Release;
        let fresh = k.kind == KeyEventKind::Press;
        if fresh && k.modifiers.contains(KeyModifiers::CONTROL) && matches!(k.code, KeyCode::Char('c' | 'C')) {
            self.cmds.push(Cmd::Exit);
            return;
        }
        let button = match k.code {
            KeyCode::Left | KeyCode::Char('a' | 'A') => Some(LEFT),
            KeyCode::Right | KeyCode::Char('d' | 'D') => Some(RIGHT),
            KeyCode::Up | KeyCode::Char('w' | 'W') => Some(UP),
            KeyCode::Down | KeyCode::Char('s' | 'S') => Some(DOWN),
            KeyCode::Char('z' | 'Z' | 'c' | 'C' | 'n' | 'N' | ' ') => Some(JUMP),
            KeyCode::Char('x' | 'X' | 'v' | 'V' | 'm' | 'M') => Some(DASH),
            _ => None,
        };
        if let Some(b) = button {
            let key = &mut self.keys[b];
            if self.exact {
                if press && !key.down {
                    key.latched = true;
                }
                key.down = press;
            } else {
                // without release events every repeat arrives as a press
                if held_by_repeat(key, now) {
                    key.repeats += 1;
                } else {
                    key.repeats = 0;
                    key.latched = true;
                }
                key.last = Some(now);
            }
        }
        if !fresh {
            return;
        }
        match k.code {
            KeyCode::Esc | KeyCode::Char('p' | 'P') => self.cmds.push(Cmd::Menu),
            KeyCode::Enter => self.cmds.push(Cmd::Confirm),
            KeyCode::Char('q' | 'Q') => self.cmds.push(Cmd::Quit),
            KeyCode::Char('r' | 'R') => self.cmds.push(Cmd::Restart),
            KeyCode::Char('k' | 'K') => self.cmds.push(Cmd::Shake),
            KeyCode::Char('a' | 'A') => self.cmds.push(Cmd::Aspect),
            KeyCode::F(8) => self.cmds.push(Cmd::Mute),
            _ => {}
        }
        if button == Some(JUMP) {
            self.cmds.push(Cmd::Confirm);
        }
    }

    /// The window lost focus: releases will not arrive, so let go of
    /// everything.
    pub fn release_all(&mut self) {
        for k in &mut self.keys {
            k.down = false;
            k.last = None;
        }
    }

    pub fn open_pad(&mut self) {
        #[cfg(feature = "gamepad")]
        match gilrs::Gilrs::new() {
            Ok(g) => {
                let all: Vec<_> = g.gamepads().map(|(_, p)| (p.name().to_string(), usable_pad(&p))).collect();
                log::info!("gamepads (name, used): {all:?}");
                self.pad_name = all.into_iter().find(|(_, used)| *used).map(|(n, _)| n);
                self.gilrs = Some(g);
            }
            Err(e) => log::warn!("gamepad support unavailable: {e}"),
        }
    }

    /// Drain gamepad events; call before reading the buttons.
    pub fn poll_pad(&mut self) {
        #[cfg(feature = "gamepad")]
        {
            use gilrs::{Axis, Button, EventType};
            let Some(g) = &mut self.gilrs else { return };
            let mut events = Vec::new();
            while let Some(ev) = g.next_event() {
                events.push(ev);
            }
            for ev in events {
                let g = self.gilrs.as_ref().unwrap();
                let pad = g.gamepad(ev.id);
                if !usable_pad(&pad) {
                    continue;
                }
                match ev.event {
                    EventType::Connected => {
                        self.pad_name = Some(pad.name().to_string());
                        log::info!("gamepad connected: {:?}", self.pad_name);
                    }
                    EventType::Disconnected => {
                        log::info!("gamepad disconnected");
                        self.pad_name = None;
                    }
                    EventType::ButtonPressed(b, _) => {
                        log::debug!("pad press {b:?}");
                        match b {
                            Button::South | Button::North => self.cmds.push(Cmd::Confirm),
                            Button::Start => self.cmds.push(Cmd::Menu),
                            _ => {}
                        }
                    }
                    EventType::ButtonReleased(b, _) => log::debug!("pad release {b:?}"),
                    _ => {}
                }
                let on = |b: Button| pad.is_pressed(b);
                let axis = |a: Axis| pad.axis_data(a).map_or(0.0, |d| d.value());
                let (sx, sy) = (axis(Axis::LeftStickX), axis(Axis::LeftStickY));
                let now = [
                    on(Button::DPadLeft) || sx < -STICK,
                    on(Button::DPadRight) || sx > STICK,
                    on(Button::DPadUp) || sy > STICK,
                    on(Button::DPadDown) || sy < -STICK,
                    on(Button::South) || on(Button::North),
                    on(Button::East) || on(Button::West) || on(Button::LeftTrigger) || on(Button::RightTrigger) || on(Button::RightTrigger2),
                ];
                for b in 0..6 {
                    if now[b] && !self.pad[b] {
                        self.pad_latched[b] = true;
                    }
                }
                self.pad = now;
            }
        }
    }

    /// The six buttons for this frame. Clears the latches.
    pub fn buttons(&mut self, now: Instant) -> [bool; 6] {
        let mut out = [false; 6];
        for b in 0..6 {
            let k = &mut self.keys[b];
            let held = if self.exact { k.down } else { held_by_repeat(k, now) };
            out[b] = held || k.latched || self.pad[b] || self.pad_latched[b];
            k.latched = false;
            self.pad_latched[b] = false;
        }
        out
    }
}

fn held_by_repeat(k: &Key, now: Instant) -> bool {
    let Some(last) = k.last else { return false };
    let window = if k.repeats == 0 { PRE_REPEAT } else { REPEAT_GAP };
    now.duration_since(last) <= window
}
