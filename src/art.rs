//! Our own sprites, drawn here pixel by pixel and painted over the stand-in
//! sheet at load time, slot by slot, until none of the stand-in is left.
//!
//! Each sprite is eight rows of eight characters: `.` transparent, else a
//! PICO-8 colour as one hex digit. Drawn from what each slot has to do in
//! the game (its size, the hitbox it must fill, the frames the code steps
//! through), not from the stand-in's pixels.

/// Hair colour in the climber's sprites; swapped at draw time to show the
/// dash state, as the original does with `pal(8, ...)`.
pub const HAIR: u8 = 10;

/// The hair colour for a dash state: blond with a dash ready, red when it
/// is spent, flashing white and gold with two (`frames` is the game clock).
pub fn hair_colour(djump: i32, frames: i32) -> u8 {
    match djump {
        1 => HAIR,
        2 if (frames / 3) % 2 == 0 => 7,
        2 => HAIR,
        _ => 8,
    }
}

/// Ceres, facing right. Blond (a) hair, peach skin (f), dark eyes (1), a
/// plum dress (2), grey boots (5). The physics box is columns 1-6, rows 3-7.
const CLIMBER: [(usize, [&str; 8]); 7] = [
    // 1: standing, and the first step of the run
    (1, ["........", "..aaaa..", ".aaaaaa.", ".aaf1f1.", ".aaffff.", "..2222..", ".f2222f.", "..5..5.."]),
    // 2: running
    (2, ["........", "..aaaa..", ".aaaaaa.", ".aaf1f1.", ".aaffff.", "..2222..", ".f2222f.", ".5....5."]),
    // 3: running, and in the air (arms out)
    (3, ["........", "..aaaa..", ".aaaaaa.", ".aaf1f1.", ".aaffff.", "f.2222.f", "..2222..", "...55..."]),
    // 4: running
    (4, ["........", "..aaaa..", ".aaaaaa.", ".aaf1f1.", ".aaffff.", "..2222..", ".f2222f.", "..5...5."]),
    // 5: sliding down a wall on her right, hand on it
    (5, ["........", "..aaaa..", ".aaaaaa.", ".aaff1f1", ".aaffff.", "..2222ff", ".22222..", "..5.5..."]),
    // 6: crouching (holding down, and landing from the spawn jump)
    (6, ["........", "........", "..aaaa..", ".aaaaaa.", ".aaf1f1.", ".aaffff.", ".f2222f.", ".55..55."]),
    // 7: looking up
    (7, ["........", "..aaaa..", ".aaf1f1.", ".aaffff.", "..aaff..", "..2222..", ".f2222f.", "..5..5.."]),
];

/// The collectible: cherries on joined stems, with a leaf. Red (8) with a
/// pink shine (e) and plum shade (2), green stems (3) and leaf (b).
const CHERRIES: [&str; 8] = ["....3b..", "...33bb.", "..3..3..", ".3....3.", "e88..e88", "888..888", "882..882", ".2....2."];

const COLLECTIBLES: [(usize, [&str; 8]); 5] = [
    // 26: cherries; also the icon on the summit score card
    (26, CHERRIES),
    // 28: flying cherries
    (28, CHERRIES),
    // 45-47: a wing, drawn on the right of the flying cherries and mirrored
    // on the left; 45 is the one the game shows (the original's quirk), the
    // others complete the flap
    (45, ["........", "....777.", "..77776.", ".7766...", "........", "........", "........", "........"]),
    (46, ["........", "........", "........", ".777777.", "..66776.", "........", "........", "........"]),
    (47, ["........", "........", "........", ".7766...", "..77776.", "....777.", "........", "........"]),
];

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

// ------------------------------------------------------------------ terrain
//
// The solid tiles are generated from the sides they are open on (which the
// levels' use of each tile shows), so edges and corners always agree.

/// Open sides: up, right, down, left.
type Open = [bool; 4];

const NONE: Open = [false; 4];

fn open(s: &str) -> Open {
    let b = s.as_bytes();
    [b.contains(&b'u'), b.contains(&b'r'), b.contains(&b'd'), b.contains(&b'l')]
}

/// Snow tiles by slot and open sides.
const SNOW: [(usize, &str); 16] = [
    (32, "urdl"), // a lone block
    (33, "ul"),   // corners
    (35, "ur"),
    (49, "dl"),
    (51, "rd"),
    (34, "u"), // edges
    (50, "d"),
    (36, "l"),
    (38, "r"),
    (37, ""), // fill
    (39, "url"), // a one-wide pillar: top, middle, bottom
    (48, "rl"),
    (55, "rdl"),
    (52, "udl"), // a one-high bar: left end, middle, right end
    (53, "ud"),
    (54, "urd"),
];

/// Ice (slippery, no wall slide) by slot and open sides.
const ICE: [(usize, &str); 14] = [
    (66, "ul"),
    (67, "u"),
    (68, "ur"),
    (82, "l"),
    (83, ""),
    (84, "r"),
    (85, "rl"),
    (98, "dl"),
    (99, "d"),
    (100, "rd"),
    (101, "rdl"),
    (114, "ul"),
    (115, "u"),
    (116, "ur"),
];

/// A small fixed hash, for texture that is the same every run.
fn hash(x: usize, y: usize, salt: usize) -> usize {
    let mut h = (x * 374_761_393 + y * 668_265_263 + salt * 2_246_822_519) as u64;
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    (h ^ (h >> 16)) as usize
}

/// Distances to the open sides, and whether the pixel is cut to round an
/// outside corner.
fn edges(o: Open, x: usize, y: usize) -> (usize, usize, usize, usize, bool) {
    let far = 99;
    let du = if o[0] { y } else { far };
    let dr = if o[1] { 7 - x } else { far };
    let dd = if o[2] { 7 - y } else { far };
    let dl = if o[3] { x } else { far };
    let cut = (o[0] && o[3] && x + y <= 1) || (o[0] && o[1] && (7 - x) + y <= 1) || (o[2] && o[3] && x + (7 - y) <= 1) || (o[2] && o[1] && (7 - x) + (7 - y) <= 1);
    (du, dr, dd, dl, cut)
}

/// Packed snow: white-grey body with sparkles, a bright cap where it is
/// open above, a lavender outline on the other open sides.
fn snow(o: Open, salt: usize, clear: u8) -> [u8; 64] {
    std::array::from_fn(|i| {
        let (x, y) = (i % 8, i / 8);
        let (du, dr, dd, dl, cut) = edges(o, x, y);
        let side = dr.min(dd).min(dl);
        if cut {
            clear
        } else if du == 0 || (du == 1 && side > 0) {
            7
        } else if side == 0 {
            13
        } else if du == 2 && side > 0 {
            6
        } else if hash(x, y, salt) % 11 == 0 {
            7
        } else {
            6
        }
    })
}

/// Ice: clear blue with a short glassy glint and a white rim.
fn ice(o: Open, clear: u8) -> [u8; 64] {
    std::array::from_fn(|i| {
        let (x, y) = (i % 8, i / 8);
        let (du, dr, dd, dl, cut) = edges(o, x, y);
        if cut {
            clear
        } else if du.min(dr).min(dd).min(dl) == 0 {
            7
        } else {
            // a glint across the upper left, and a faint second one lower down
            match (x + y, x) {
                (4, 1..=3) => 7,
                (10, 4..=6) => 6,
                _ => 12,
            }
        }
    })
}

/// Background rock: dark grey with strata and the odd pit. `shape` says
/// which pixels are rock.
fn rock(salt: usize, pit: bool, clear: u8, shape: impl Fn(usize, usize) -> bool) -> [u8; 64] {
    std::array::from_fn(|i| {
        let (x, y) = (i % 8, i / 8);
        if !shape(x, y) {
            return clear;
        }
        // a lighter rim along a cut edge
        let rim = [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)].iter().any(|&(dx, dy)| {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            (0..8).contains(&nx) && (0..8).contains(&ny) && !shape(nx as usize, ny as usize)
        });
        if rim {
            13
        } else if pit && (2..5).contains(&x) && (3..6).contains(&y) {
            if y == 3 || x == 2 { 1 } else { 0 }
        } else if (y + salt) % 8 == 2 && hash(x / 3, y, salt) % 3 == 0 {
            1
        } else {
            5
        }
    })
}

/// Spikes as ice shards: two to a tile, tips 4 pixels out from the base,
/// inside the band where they kill. Drawn pointing up; turned for the rest.
const SHARDS: [&str; 8] = ["........", "........", "........", "........", "..7...7.", ".7c6.7c6", "7cc67cc6", "7cc67cc6"];

fn grid(rows: &[&str; 8], clear: u8) -> [u8; 64] {
    std::array::from_fn(|i| hex(rows[i / 8].as_bytes()[i % 8]).unwrap_or(clear))
}

fn terrain(clear: u8) -> Vec<(usize, [u8; 64])> {
    let mut out = Vec::new();
    for (slot, o) in SNOW {
        out.push((slot, snow(open(o), slot, clear)));
    }
    out.push((72, snow(NONE, 72, clear)));
    for (slot, o) in ICE {
        out.push((slot, ice(open(o), clear)));
    }
    // background rock: fills, the four diagonals, and the top edges
    out.push((40, rock(0, false, clear, |_, _| true)));
    out.push((56, rock(2, false, clear, |_, _| true)));
    out.push((16, rock(1, true, clear, |_, _| true)));
    out.push((41, rock(0, false, clear, |x, y| x + y <= 7)));
    out.push((42, rock(0, false, clear, |x, y| x >= y)));
    out.push((57, rock(0, false, clear, |x, y| x <= y)));
    out.push((58, rock(0, false, clear, |x, y| x + y >= 7)));
    out.push((88, rock(0, false, clear, |_, y| y >= 4)));
    out.push((103, rock(0, false, clear, |x, y| y * 2 >= 4 + x)));
    out.push((104, rock(0, false, clear, |x, y| y * 2 + x >= 11)));
    // spikes: up, down, pointing right (on a wall to the left), left
    let up = grid(&SHARDS, clear);
    out.push((17, up));
    out.push((27, std::array::from_fn(|i| up[i % 8 + (7 - i / 8) * 8])));
    let right: [u8; 64] = std::array::from_fn(|i| up[i / 8 + (7 - i % 8) * 8]);
    out.push((43, right));
    out.push((59, std::array::from_fn(|i| right[(7 - i % 8) + (i / 8) * 8])));
    out
}

/// Paint every sprite drawn so far into `sprites` (`clear` is the
/// transparent index).
pub fn paint(sprites: &mut [[u8; 64]], clear: u8) {
    for (slot, tile) in terrain(clear) {
        sprites[slot] = tile;
    }
    for (slot, rows) in CLIMBER.into_iter().chain(COLLECTIBLES) {
        let s = &mut sprites[slot];
        for (y, row) in rows.iter().enumerate() {
            assert_eq!(row.len(), 8, "sprite {slot} row {y}");
            for (x, c) in row.bytes().enumerate() {
                s[x + y * 8] = hex(c).unwrap_or(clear);
            }
        }
    }
}
