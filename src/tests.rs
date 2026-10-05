//! Headless checks of the port: the map, and a replay of the community any%
//! TAS (`tests/tas-any/`, see its README) through every level. A TAS is
//! frame-exact, so one wrong rounding, one misplaced update or one swapped
//! comparison anywhere in the physics makes it miss a ledge or hit a spike.

use crate::classic::{Classic, Kind};

#[test]
fn every_level_has_one_spawn_and_the_title_none() {
    let g = Classic::new(1);
    for level in 0..32 {
        let (rx, ry) = (level % 8, level / 8);
        let spawns = (0..16).flat_map(|x| (0..16).map(move |y| (x, y))).filter(|&(x, y)| g.e.mget(rx * 16 + x, ry * 16 + y) == 1).count();
        assert_eq!(spawns, if level == 31 { 0 } else { 1 }, "level {level}");
    }
}

#[test]
fn the_title_room_starts_and_hands_over_to_the_first_level() {
    let mut g = Classic::new(1);
    assert!(g.is_title());
    g.e.btn[crate::pico::JUMP] = true;
    for _ in 0..90 {
        g.update();
        g.draw();
    }
    assert_eq!(g.room, (0, 0));
}

struct Tas {
    seeds: Vec<f64>,
    frames: Vec<u8>,
}

fn load_tas(level: usize) -> Tas {
    let path = format!("{}/tests/tas-any/TAS{level}.tas", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let (head, body) = text.trim().trim_start_matches('[').split_once(']').expect("seed list");
    let num = |s: &str| s.trim().parse::<f64>().unwrap_or_else(|_| panic!("{path}: bad number {s:?}"));
    Tas {
        seeds: head.split(',').filter(|s| !s.trim().is_empty()).map(num).collect(),
        frames: body.split(',').filter(|s| !s.trim().is_empty()).map(|s| num(s) as u8).collect(),
    }
}

/// Play `level` (1-31) from a fresh room load with the TAS's inputs, set up
/// the way the TAS tool sets it up. Ok(frames) when the level is beaten.
fn replay(level: usize) -> Result<usize, String> {
    replay_traced(level, &mut String::new())
}

fn replay_traced(level: usize, trace: &mut String) -> Result<usize, String> {
    let tas = load_tas(level);
    let index = level as i32 - 1;
    let mut g = Classic::new(7);
    // the orb in level 22 gives the second dash for the rest of the climb
    g.max_djump = if index > 21 { 2 } else { 1 };
    g.load_room(index % 8, index / 8);
    let mut seeds = tas.seeds.iter();
    for o in &mut g.objects {
        match &mut o.kind {
            Kind::Balloon { offset, .. } => *offset = *seeds.next().unwrap_or(&0.0),
            Kind::Chest { .. } => {
                seeds.next();
            }
            // the tool's stand-in for the platform's move on the frame the
            // room loads during a real run
            Kind::Platform { dir, .. } => o.rem.x = *dir * 0.65,
            _ => {}
        }
    }
    let deaths = g.deaths;
    let (mut show_keys, mut next) = (false, 0usize);
    for frame in 0..tas.frames.len() + 200 {
        // `next` counts frames since the inputs started; the first one is 1
        let bits = if show_keys { tas.frames.get(next - 1).copied().unwrap_or(0) } else { 0 };
        g.e.btn = std::array::from_fn(|b| bits & (1 << b) != 0);
        g.update();
        if g.freeze <= 0 {
            g.draw();
        }
        match g.objects.iter().find(|o| !o.dead && matches!(o.kind, Kind::Player(_) | Kind::Spawn { .. })) {
            Some(p) => {
                let t = if matches!(p.kind, Kind::Player(_)) { 'P' } else { 'S' };
                trace.push_str(&format!("{frame} {t} {bits} {} {} {} {} {} {}\n", p.x, p.y, p.spd.x, p.spd.y, p.rem.x, p.rem.y));
            }
            None => trace.push_str(&format!("{frame} - {bits}\n")),
        }
        if g.deaths != deaths {
            let at = g.player().map(|p| (p.x, p.y));
            return Err(format!("died on frame {frame} (input {next} of {}), last player position {at:?}", tas.frames.len()));
        }
        if g.level_index() != index {
            return Ok(frame + 1);
        }
        if index == 30 && g.objects.iter().any(|o| matches!(o.kind, Kind::Flag { show: true, .. })) {
            return Ok(frame + 1);
        }
        // inputs start once the spawn animation is about to hand over
        if g.objects.iter().any(|o| !o.dead && matches!(o.kind, Kind::Spawn { state: 2, delay: 0, .. })) {
            show_keys = true;
        }
        if show_keys {
            next += 1;
        }
    }
    let at = g.player().map(|p| (p.x, p.y, p.spd.x, p.spd.y));
    Err(format!("inputs ran out without finishing; player {at:?}"))
}

#[test]
fn the_any_percent_tas_beats_every_level() {
    let mut failed = Vec::new();
    let mut total = 0;
    for level in 1..=31 {
        match replay(level) {
            Ok(frames) => {
                total += frames;
                println!("level {level:2}: beaten in {frames} frames");
            }
            Err(e) => {
                println!("level {level:2}: FAILED: {e}");
                failed.push(level);
            }
        }
    }
    println!("{} of 31 levels, {total} frames", 31 - failed.len());
    assert!(failed.is_empty(), "levels not beaten: {failed:?}");
}

/// With CERES_TRACE_DIR set, write a per-frame trace of the player for
/// every level, in the format of the Lua reference tracer, for diffing.
#[test]
#[ignore]
fn write_traces() {
    let Ok(dir) = std::env::var("CERES_TRACE_DIR") else { return };
    for level in 1..=31 {
        let mut out = String::new();
        let _ = replay_traced(level, &mut out);
        std::fs::write(format!("{dir}/rust{level}.txt"), out).unwrap();
    }
}

/// With CERES_SHOT_DIR set, save a few frames as PNGs (4x) to look at.
#[test]
#[ignore]
fn screenshots() {
    let Ok(dir) = std::env::var("CERES_SHOT_DIR") else { return };
    let save = |g: &Classic, name: &str| {
        let s = 4;
        let mut px = vec![0u8; 128 * s * 128 * s * 3];
        for y in 0..128 * s {
            for x in 0..128 * s {
                let (r, gr, b) = g.e.rgb(x / s, y / s);
                let i = (x + y * 128 * s) * 3;
                px[i..i + 3].copy_from_slice(&[r, gr, b]);
            }
        }
        let f = std::fs::File::create(format!("{dir}/{name}.png")).unwrap();
        let mut enc = png::Encoder::new(f, (128 * s) as u32, (128 * s) as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.write_header().unwrap().write_image_data(&px).unwrap();
    };
    let mut g = Classic::new(3);
    for _ in 0..10 {
        g.update();
        g.draw();
    }
    save(&g, "title");
    for (level, frames, name) in [(1, 60, "level1"), (3, 40, "level3"), (4, 35, "level4"), (5, 60, "level5"), (13, 40, "level13"), (22, 60, "level22"), (25, 40, "level25"), (31, 45, "summit")] {
        let tas = load_tas(level);
        let mut g = Classic::new(3);
        let index = level as i32 - 1;
        g.max_djump = if index > 21 { 2 } else { 1 };
        g.load_room(index % 8, index / 8);
        let (mut show, mut next) = (false, 0usize);
        for _ in 0..frames {
            let bits = if show { tas.frames.get(next - 1).copied().unwrap_or(0) } else { 0 };
            g.e.btn = std::array::from_fn(|b| bits & (1 << b) != 0);
            g.update();
            if g.freeze <= 0 {
                g.draw();
            }
            if g.objects.iter().any(|o| !o.dead && matches!(o.kind, Kind::Spawn { state: 2, delay: 0, .. })) {
                show = true;
            }
            if show {
                next += 1;
            }
        }
        save(&g, name);
    }
}
