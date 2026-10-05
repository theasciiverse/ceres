//! The whole sprite sheet, drawn here: every sprite the game shows is
//! ours, built at start-up (there is no image file).
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

// ------------------------------------------------------------------ objects

const OBJECTS: [(usize, [&str; 8]); 27] = [
    // 8-10: the key turning (10 edge-on; the game mirrors it there)
    (8, ["...a....", "..a.9...", "..a.9...", "...a....", "...a....", "...aa...", "...a....", "...aa..."]),
    (9, ["..aaa...", ".a...9..", ".a...9..", "..aa9...", "...a....", "...aa...", "...a....", "...aa..."]),
    (10, ["...a....", "...9....", "...9....", "...a....", "...a....", "...a....", "...a....", "...a...."]),
    // 11-12: moving platform, a plank in two halves
    (11, [".9999999", "94444444", "44544444", ".2222222", "........", "........", "........", "........"]),
    (12, ["9999999.", "44444449", "44444544", "2222222.", "........", "........", "........", "........"]),
    // 13-15: the balloon's string, three wiggles
    (13, ["...6....", "...6....", "....6...", "....6...", "...6....", "........", "........", "........"]),
    (14, ["...6....", "....6...", "....6...", "...6....", "...6....", "........", "........", "........"]),
    (15, ["...6....", "...6....", "..6.....", "..6.....", "...6....", "........", "........", "........"]),
    // 18-19: spring, ready and sprung
    (18, ["........", "........", "........", "........", "........", ".888882.", "..6..6..", ".666666."]),
    (19, ["........", "........", ".888882.", "..6..6..", "...66...", "..6..6..", "...66...", ".666666."]),
    // 20: the small chest the key opens
    (20, ["........", "........", ".999999.", "94444449", "22222222", "444aa444", "4449a444", "42222224"]),
    // 22: balloon (gives the dash back)
    (22, ["..eeee..", ".e7eeee.", "e7eeeeee", "eeeeeeee", "eeeeeee2", ".eeeee2.", "..ee22..", "...22..."]),
    // 23-25: a clod of frozen earth that crumbles under you
    (23, [".999999.", "94444442", "94424442", "94444442", "94442442", "94444442", "94244442", ".222222."]),
    (24, [".999999.", "94444.42", "9442.442", "944.4442", "94.42442", "944.4442", "942.4442", ".222222."]),
    (25, [".99.999.", "944..442", "94.2.442", ".4.4.44.", "94..2.42", "9.4..442", "942.4.42", ".2.2.22."]),
    // 29-31: a puff of snow dust, fading
    (29, ["........", "..77....", ".7776...", ".7776.7.", "..66.776", "....766.", "........", "........"]),
    (30, ["........", "........", "..77....", "..76..7.", "......6.", "...7....", "........", "........"]),
    (31, ["........", "........", "........", "...6....", "......6.", "........", "........", "........"]),
    // 102: the orb, a golden sun-seed (two dashes from here on)
    (102, ["..aaaa..", ".a77aaa.", "a77aaaa9", "a7aaaa99", "aaaaaa99", "aaaaa999", ".aa9999.", "..9999.."]),
    // 118-120: the summit pennant, waving
    (118, [".7......", ".6eeeee.", ".6eeee2.", ".6eee2..", ".6e2....", ".6......", ".6......", ".6......"]),
    (119, [".7......", ".6eeee..", ".6eeeee2", ".6eeee2.", ".6ee2...", ".6......", ".6......", ".6......"]),
    (120, [".7......", ".6eeeee2", ".6eeee2.", ".6eeeee.", ".6e22...", ".6......", ".6......", ".6......"]),
    // 44 over 60: a young fir with snow on its branches
    (44, ["........", "...3....", "..373...", "..333...", ".37733..", ".33333..", "3377333.", "3333333."]),
    (60, [".37333..", "3333333.", "...4....", "...4....", "...4....", "...4....", "..444...", "..444..."]),
    // 61, 63: grass tufts; 62: an ear of wheat
    (61, ["........", "........", "........", "........", "....b...", ".b..3.b.", "..3.3.3.", ".3.333.."]),
    (63, ["........", "........", "........", "........", "........", "..b..b..", "...3.3.b", "..3.33.3"]),
    (62, ["....a...", "...a9...", "....a9..", "...a9...", "....a...", "....3...", "...3....", "....3..."]),
];

/// Sixteen-wide pieces: (left slot, right slot, rows).
const WIDE: [(usize, usize, [&str; 8]); 4] = [
    // 96-97 over 112-113: the big chest, lid and base
    (96, 97, ["................", "...444444444444.", "..49999999999994", ".444444444444444", ".4a4444aa44444a4", ".444444994444444", ".222222222222222", ".444444444444444"]),
    (112, 113, [".44444aaaa444444", ".4444a9449a44444", ".44444aaaa444444", ".444444444444444", ".4a444444444444a", ".444444444444444", ".222222222222222", "2222222222222222"]),
    // 70-71 over 86-87: the memorial, an engraved stone with snow on top
    (70, 71, ["................", "....77777777....", "...d77777777d...", "..dddddddddddd..", "..d5555dd555dd..", "..dddddddddddd..", "..d555d5555ddd..", "..dddddddddddd.."]),
    (86, 87, ["..d55555d55ddd..", "..dddddddddddd..", "..d5555d555ddd..", "..dddddddddddd..", "..dddddddddddd..", ".55555555555555.", ".55555555555555.", "5555555555555555"]),
];

/// The breakable wall: a 2x2 snow block like the terrain, with a crack
/// across it as the hint.
fn fake_wall(clear: u8) -> [(usize, [u8; 64]); 4] {
    let crack: [(usize, usize); 16] = [(4, 0), (4, 1), (5, 2), (6, 3), (6, 4), (7, 5), (8, 6), (8, 7), (9, 8), (10, 9), (10, 10), (11, 11), (12, 12), (11, 13), (12, 14), (13, 15)];
    let quads = [(64, "ul", 0, 0), (65, "ur", 8, 0), (80, "dl", 0, 8), (81, "rd", 8, 8)];
    quads.map(|(slot, o, ox, oy)| {
        let mut t = snow(open(o), 64, clear);
        for &(x, y) in &crack {
            if (ox..ox + 8).contains(&x) && (oy..oy + 8).contains(&y) {
                t[(x - ox) + (y - oy) * 8] = 5;
            }
        }
        (slot, t)
    })
}

/// The title: mountain lines behind CERES (the letters are painted by
/// `pico.rs`), in the logo's part of the sheet (x 72-127, y 32-63).
fn title_mountain(px: &mut [u8], w: usize, clear: u8) {
    // two peaks, lit from the left; the lines pass behind the letters
    let behind = |x: i32, y: i32| (79..=119).contains(&x) && (43..=51).contains(&y);
    let line = |px: &mut [u8], (x0, y0): (i32, i32), (x1, y1): (i32, i32), c: u8| {
        let n = (x1 - x0).abs().max((y1 - y0).abs());
        for k in 0..=n {
            let (x, y) = (x0 + (x1 - x0) * k / n, y0 + (y1 - y0) * k / n);
            let i = x as usize + y as usize * w;
            if px[i] == clear && !behind(x, y) {
                px[i] = c;
            }
        }
    };
    line(px, (73, 62), (90, 40), 12);
    line(px, (90, 40), (100, 34), 12);
    line(px, (100, 34), (112, 43), 1);
    line(px, (112, 43), (117, 39), 13);
    line(px, (117, 39), (127, 52), 1);
    for &(x, y) in &[(100usize, 34usize), (99, 35), (101, 35), (117, 39)] {
        px[x + y * w] = 7;
    }
}

// ------------------------------------------------------------------ variants

/// Which look a map tile at (`x`, `y`) gets: 0 its own, k its k-th
/// variant. A fixed hash, so a room always looks the same.
pub fn pick(x: i32, y: i32, variants: usize) -> usize {
    if variants == 0 {
        return 0;
    }
    hash(x as usize, y as usize, 7) % (variants + 1)
}

/// Alternative looks for the most repeated tiles, so big masses of snow and
/// rock don't show the same 8x8 over and over.
pub fn variants(clear: u8) -> Vec<(usize, [u8; 64])> {
    let mut out = Vec::new();
    // snow fill: other sparkles, one with a buried crystal
    for salt in [101, 202] {
        out.push((37, snow(NONE, salt, clear)));
    }
    let mut crystal = snow(NONE, 303, clear);
    for &(x, y, c) in &[(4, 3, 12), (5, 4, 12), (4, 4, 7), (3, 4, 13)] {
        crystal[x + y * 8] = c;
    }
    out.push((37, crystal));
    // snow tops: little drifts
    for (dip, salt) in [((1usize, 3usize), 11), ((4, 6), 12)] {
        let mut t = snow(open("u"), salt, clear);
        for x in dip.0..dip.1 {
            t[x] = clear;
        }
        out.push((34, t));
    }
    // rock fill: other strata
    out.push((40, rock(3, false, clear, |_, _| true)));
    out.push((40, rock(5, false, clear, |_, _| true)));
    // ice fill: the glint elsewhere
    let mut glint = ice(NONE, clear);
    for (i, p) in glint.iter_mut().enumerate() {
        let (x, y) = (i % 8, i / 8);
        *p = match (x + y, x) {
            (8, 3..=5) => 7,
            _ => 12,
        };
    }
    out.push((83, glint));
    out
}

/// Paint the title's mountain into the sheet's pixels before they are cut
/// into sprites.
pub fn paint_title(px: &mut [u8], w: usize, clear: u8) {
    title_mountain(px, w, clear);
}

/// Paint every sprite drawn so far into `sprites` (`clear` is the
/// transparent index).
pub fn paint(sprites: &mut [[u8; 64]], clear: u8) {
    for (slot, tile) in terrain(clear) {
        sprites[slot] = tile;
    }
    for (slot, tile) in fake_wall(clear) {
        sprites[slot] = tile;
    }
    for (slot, rows) in OBJECTS {
        sprites[slot] = grid(&rows, clear);
    }
    for (l, r, rows) in WIDE {
        for (slot, half) in [(l, 0), (r, 8)] {
            sprites[slot] = std::array::from_fn(|i| {
                assert_eq!(rows[i / 8].len(), 16, "wide sprite {l} row {}", i / 8);
                hex(rows[i / 8].as_bytes()[half + i % 8]).unwrap_or(clear)
            });
        }
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
