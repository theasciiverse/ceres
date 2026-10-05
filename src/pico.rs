//! A small stand-in for the PICO-8 machine the game was written for, after
//! Noel Berry's `Emulator.cs`: a 128x128 screen of palette indices, the
//! sprite sheet and tile flags, the map, `btn`, `rnd` and PICO-8's
//! turn-based `sin`/`cos`. Sound and music calls are queued for the caller.

use crate::font;

pub const W: usize = 128;
/// Transparent sprite pixel.
const CLEAR: u8 = 255;

pub const PALETTE: [(u8, u8, u8); 16] = [
    (0x00, 0x00, 0x00),
    (0x1d, 0x2b, 0x53),
    (0x7e, 0x25, 0x53),
    (0x00, 0x87, 0x51),
    (0xab, 0x52, 0x36),
    (0x5f, 0x57, 0x4f),
    (0xc2, 0xc3, 0xc7),
    (0xff, 0xf1, 0xe8),
    (0xff, 0x00, 0x4d),
    (0xff, 0xa3, 0x00),
    (0xff, 0xec, 0x27),
    (0x00, 0xe4, 0x36),
    (0x29, 0xad, 0xff),
    (0x83, 0x76, 0x9c),
    (0xff, 0x77, 0xa8),
    (0xff, 0xcc, 0xaa),
];

/// Sprite flags, one byte per sprite (`Emulator.cs`'s `mask`): bit 0 solid,
/// 1 terrain, 2 background, 3 foreground, 4 ice.
const FLAGS: [u8; 128] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    4, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, //
    3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 2, 2, 0, 0, 0, //
    3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 2, 2, 2, 2, 2, //
    0, 0, 19, 19, 19, 19, 2, 2, 3, 2, 2, 2, 2, 2, 2, 2, //
    0, 0, 19, 19, 19, 19, 2, 2, 4, 2, 2, 2, 2, 2, 2, 2, //
    0, 0, 19, 19, 19, 19, 0, 4, 4, 2, 2, 2, 2, 2, 2, 2, //
    0, 0, 19, 19, 19, 19, 0, 0, 0, 2, 2, 2, 2, 2, 2, 2,
];

static MAP: &[u8; 8192] = include_bytes!("../data/map.bin");

/// Buttons, in PICO-8 order.
pub const LEFT: usize = 0;
pub const RIGHT: usize = 1;
pub const UP: usize = 2;
pub const DOWN: usize = 3;
pub const JUMP: usize = 4;
pub const DASH: usize = 5;

pub struct Pico {
    pub screen: [u8; W * W],
    /// Display palette swap, applied to the finished frame (`pal(a, b)`).
    pub pal: [u8; 16],
    cam: (i32, i32),
    sprites: Vec<[u8; 64]>,
    /// Alternative looks for a map tile (sprite indices past 127), picked
    /// by map position when drawing the map; the tile's number, and so its
    /// flags and the physics, stay the same.
    variants: Vec<Vec<usize>>,
    pub btn: [bool; 6],
    rng: u64,
    /// Sound effects requested since the caller last drained them.
    pub sfx: Vec<u8>,
    /// Music changes requested (`-1` stops), drained by the caller.
    pub music: Vec<i32>,
}

impl Pico {
    pub fn new(seed: u64) -> Pico {
        let mut p = Pico {
            screen: [0; W * W],
            pal: std::array::from_fn(|i| i as u8),
            cam: (0, 0),
            sprites: Vec::new(),
            variants: vec![Vec::new(); 128],
            btn: [false; 6],
            rng: seed | 1,
            sfx: Vec::new(),
            music: Vec::new(),
        };
        p.sprites = load_sprites();
        for (base, look) in crate::art::variants(CLEAR) {
            p.variants[base].push(p.sprites.len());
            p.sprites.push(look);
        }
        p
    }

    // ------------------------------------------------------------- maths

    pub fn rnd(&mut self, max: f64) -> f64 {
        // xorshift64*
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        let r = self.rng.wrapping_mul(0x2545_f491_4f6c_dd1d);
        ((r >> 40) as f64 / (1u64 << 24) as f64) * max
    }

    // ------------------------------------------------------------- input

    pub fn btn(&self, b: usize) -> bool {
        self.btn[b]
    }

    // ------------------------------------------------------------- sound

    pub fn sfx(&mut self, n: u8) {
        self.sfx.push(n);
    }

    pub fn music(&mut self, n: i32) {
        self.music.push(n);
    }

    // ------------------------------------------------------------- map

    pub fn mget(&self, tx: i32, ty: i32) -> i32 {
        MAP[(tx + ty * 128) as usize] as i32
    }

    pub fn fget(&self, tile: i32, flag: u32) -> bool {
        (tile as usize) < FLAGS.len() && FLAGS[tile as usize] & (1 << flag) != 0
    }

    // ------------------------------------------------------------- drawing

    pub fn camera(&mut self, x: f64, y: f64) {
        self.cam = (x.round() as i32, y.round() as i32);
    }

    pub fn pal_reset(&mut self) {
        self.pal = std::array::from_fn(|i| i as u8);
    }

    fn pset(&mut self, x: i32, y: i32, c: u8) {
        let (x, y) = (x - self.cam.0, y - self.cam.1);
        if (0..W as i32).contains(&x) && (0..W as i32).contains(&y) {
            self.screen[x as usize + y as usize * W] = c;
        }
    }

    fn rect_px(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: u8) {
        for y in y0.max(self.cam.1 - 1)..=y1.min(self.cam.1 + W as i32) {
            for x in x0.max(self.cam.0 - 1)..=x1.min(self.cam.0 + W as i32) {
                self.pset(x, y, c);
            }
        }
    }

    pub fn rectfill(&mut self, x: f64, y: f64, x2: f64, y2: f64, c: f64) {
        let (a, b) = (x.min(x2).floor() as i32, x.max(x2).floor() as i32);
        let (t, d) = (y.min(y2).floor() as i32, y.max(y2).floor() as i32);
        self.rect_px(a, t, b, d, (c as i32 % 16) as u8);
    }

    /// The small circles of `Emulator.cs`, built from rectangles.
    pub fn circfill(&mut self, x: f64, y: f64, r: f64, c: f64) {
        let (x, y) = (x.floor() as i32, y.floor() as i32);
        let c = (c as i32 % 16) as u8;
        if r <= 1.0 {
            self.rect_px(x - 1, y, x + 1, y, c);
            self.rect_px(x, y - 1, x, y + 1, c);
        } else if r <= 2.0 {
            self.rect_px(x - 2, y - 1, x + 2, y + 1, c);
            self.rect_px(x - 1, y - 2, x + 1, y + 2, c);
        } else if r <= 3.0 {
            self.rect_px(x - 3, y - 1, x + 3, y + 1, c);
            self.rect_px(x - 1, y - 3, x + 1, y + 3, c);
            self.rect_px(x - 2, y - 2, x + 2, y + 2, c);
        }
    }

    pub fn print(&mut self, s: &str, x: f64, y: f64, c: f64) {
        let (mut left, y) = (x.floor() as i32, y.floor() as i32);
        let c = (c as i32 % 16) as u8;
        for ch in s.chars() {
            if let Some(g) = font::glyph(ch) {
                for (row, bits) in g.iter().enumerate() {
                    for col in 0..3 {
                        if bits & (0b100 >> col) != 0 {
                            self.pset(left + col, y + row as i32, c);
                        }
                    }
                }
            }
            left += 4;
        }
    }

    pub fn spr(&mut self, n: f64, x: f64, y: f64, flip_x: bool, flip_y: bool) {
        self.spr_swap(n, x, y, flip_x, flip_y, None);
    }

    /// `spr` with one colour swapped, as `pal(a, b)` around a `spr` call.
    pub fn spr_swap(&mut self, n: f64, x: f64, y: f64, flip_x: bool, flip_y: bool, swap: Option<(u8, u8)>) {
        let n = n as i32;
        if n < 0 || n as usize >= self.sprites.len() {
            return;
        }
        let (x, y) = (x.floor() as i32, y.floor() as i32);
        let s = self.sprites[n as usize];
        for py in 0..8 {
            for px in 0..8 {
                let sx = if flip_x { 7 - px } else { px };
                let sy = if flip_y { 7 - py } else { py };
                let mut c = s[sx + sy * 8];
                if let Some((from, to)) = swap {
                    if c == from {
                        c = to;
                    }
                }
                if c != CLEAR {
                    self.pset(x + px as i32, y + py as i32, c);
                }
            }
        }
    }

    /// Draw `mw`x`mh` map tiles from (`mx`,`my`) at (`tx`,`ty`), only tiles
    /// with flag `flag` set.
    pub fn map(&mut self, mx: i32, my: i32, tx: i32, ty: i32, mw: i32, mh: i32, flag: u32) {
        for x in 0..mw {
            for y in 0..mh {
                let tile = self.mget(x + mx, y + my);
                if (tile as usize) < 128 && self.fget(tile, flag) {
                    let alts = &self.variants[tile as usize];
                    let look = match crate::art::pick(x + mx, y + my, alts.len()) {
                        0 => tile as usize,
                        k => alts[k - 1],
                    };
                    self.spr(look as f64, (tx + x * 8) as f64, (ty + y * 8) as f64, false, false);
                }
            }
        }
    }

    /// The finished frame as RGB, with the palette swap applied.
    pub fn rgb(&self, x: usize, y: usize) -> (u8, u8, u8) {
        PALETTE[self.pal[self.screen[x + y * W] as usize & 15] as usize]
    }
}

/// PICO-8 `sin`: angles in turns, y pointing down.
pub fn sin(a: f64) -> f64 {
    -(a * std::f64::consts::TAU).sin()
}

pub fn cos(a: f64) -> f64 {
    (a * std::f64::consts::TAU).cos()
}

/// PICO-8's `%`: floored, so the result takes the sign of the divisor.
pub fn modulo(a: f64, b: f64) -> f64 {
    a - (a / b).floor() * b
}

pub fn flr(v: f64) -> i32 {
    v.floor() as i32
}

pub fn sign(v: f64) -> i32 {
    if v > 0.0 {
        1
    } else if v < 0.0 {
        -1
    } else {
        0
    }
}

// ----------------------------------------------------------------- sprites

/// The sprite sheet, all of it ours: built here from `art.rs` and the
/// title logo below, 16x8 sprites of 8x8 pixels.
fn load_sprites() -> Vec<[u8; 64]> {
    let (w, h) = (128, 64);
    let mut px = vec![CLEAR; w * h];
    crate::art::paint_title(&mut px, w, CLEAR);
    paint_logo(&mut px, w);
    let mut sprites = Vec::new();
    for ty in 0..h / 8 {
        for tx in 0..w / 8 {
            sprites.push(std::array::from_fn(|i| px[tx * 8 + i % 8 + (ty * 8 + i / 8) * w]));
        }
    }
    crate::art::paint(&mut sprites, CLEAR);
    sprites
}

/// The title logo: CERES, light grey tops and lavender below, over the
/// mountain lines from `art.rs`.
fn paint_logo(px: &mut [u8], w: usize) {
    const TOP: usize = 44;
    const C: [&str; 7] = [".66666.", "6666666", "66...66", "dd.....", "dd...dd", "ddddddd", ".ddddd."];
    const E: [&str; 7] = ["666666.", "6666666", "66.....", "dddd...", "dd.....", "dddddd.", "ddddddd"];
    const R: [&str; 7] = ["666666.", "6666666", "66...66", "dddddd.", "dd.dd..", "dd..dd.", "dd...dd"];
    const S: [&str; 7] = [".66666.", "6666666", "66.....", "ddddddd", "......d", "ddddddd", ".ddddd."];
    for (k, letter) in [C, E, R, E, S].iter().enumerate() {
        for (row, line) in letter.iter().enumerate() {
            for (col, ch) in line.bytes().enumerate() {
                let c = match ch {
                    b'6' => 6,
                    b'd' => 13,
                    _ => continue,
                };
                px[80 + k * 8 + col + (TOP + row) * w] = c;
            }
        }
    }
}
