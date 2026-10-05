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

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

/// Paint every sprite drawn so far into `sprites` (`clear` is the
/// transparent index).
pub fn paint(sprites: &mut [[u8; 64]], clear: u8) {
    for (slot, rows) in CLIMBER {
        let s = &mut sprites[slot];
        for (y, row) in rows.iter().enumerate() {
            assert_eq!(row.len(), 8, "sprite {slot} row {y}");
            for (x, c) in row.bytes().enumerate() {
                s[x + y * 8] = hex(c).unwrap_or(clear);
            }
        }
    }
}
