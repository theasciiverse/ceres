//! The game: Celeste Classic, ported from Noel Berry's `Classic.cs` (MIT),
//! checked line by line against Maddy Thorson and Noel Berry's PICO-8 Lua.
//! Where the C# drifted from the Lua, the Lua wins: a dash with no direction
//! held starts at speed 1 (not 5), right beats left and up beats down when
//! both are held, `%` is floored, objects keep fractional positions, and
//! objects created during a frame are updated in that same frame. The order
//! of operations and the quirks (the extra step in `move_x`, the flying
//! strawberry's frozen wings) are kept, because the feel lives in them.
//! Numbers are `f64`, like picolove's Lua, which the community TASes that
//! the tests replay were made on.
//!
//! Objects live in one list. Destroying an object marks it dead; every loop
//! over objects walks the live ones the way PICO-8's `all()` does (see
//! `each`), and dead ones are swept between loops.

use crate::pico::{cos, flr, modulo, sign, sin, Pico, DASH, DOWN, JUMP, LEFT, RIGHT, UP};
use std::collections::HashSet;

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct V2 {
    pub x: f64,
    pub y: f64,
}

fn v2(x: f64, y: f64) -> V2 {
    V2 { x, y }
}

#[derive(Clone, Copy, Debug)]
pub struct Hitbox {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tag {
    Player,
    Spawn,
    Spring,
    Balloon,
    FallFloor,
    Smoke,
    Fruit,
    FlyFruit,
    Lifeup,
    FakeWall,
    Key,
    Chest,
    Platform,
    Message,
    BigChest,
    Orb,
    Flag,
    RoomTitle,
}

#[derive(Clone, Debug)]
pub struct Hair {
    nodes: [(f64, f64, f64); 5],
}

impl Hair {
    fn new(x: f64, y: f64) -> Hair {
        Hair { nodes: std::array::from_fn(|i| (x, y, 1f64.max(2f64.min(3.0 - i as f64)))) }
    }

    fn draw(&mut self, e: &mut Pico, x: f64, y: f64, facing: i32, djump: i32, frames: i32) {
        let c = if djump == 1 {
            8
        } else if djump == 2 {
            7 + ((frames / 3) % 2) * 4
        } else {
            12
        };
        let mut last = v2(x + 4.0 - (facing * 2) as f64, y + if e.btn(DOWN) { 4.0 } else { 3.0 });
        for h in &mut self.nodes {
            h.0 += (last.x - h.0) / 1.5;
            h.1 += (last.y + 0.5 - h.1) / 1.5;
            e.circfill(h.0, h.1, h.2, c as f64);
            last = v2(h.0, h.1);
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlayerD {
    pub p_jump: bool,
    pub p_dash: bool,
    pub grace: i32,
    pub jbuffer: i32,
    pub djump: i32,
    pub dash_time: i32,
    pub dash_effect_time: i32,
    pub dash_target: V2,
    pub dash_accel: V2,
    pub spr_off: f64,
    pub was_on_ground: bool,
    pub hair: Hair,
}

#[derive(Clone, Debug)]
pub enum Kind {
    Player(Box<PlayerD>),
    Spawn { target: V2, state: i32, delay: i32, hair: Hair },
    Spring { hide_in: i32, hide_for: i32, delay: i32 },
    Balloon { offset: f64, start: f64, timer: f64 },
    FallFloor { state: i32, delay: i32 },
    Smoke,
    Fruit { start: f64, off: f64 },
    FlyFruit { start: f64, fly: bool, step: f64, sfx_delay: f64 },
    Lifeup { duration: i32, flash: f64 },
    FakeWall,
    Key,
    Chest { start: f64, timer: f64 },
    Platform { dir: f64, last: f64 },
    Message { last: f64, index: f64 },
    BigChest { state: i32, timer: f64, particles: Vec<[f64; 4]> },
    Orb,
    Flag { score: f64, show: bool },
    RoomTitle { delay: f64 },
}

impl Kind {
    pub fn tag(&self) -> Tag {
        match self {
            Kind::Player(_) => Tag::Player,
            Kind::Spawn { .. } => Tag::Spawn,
            Kind::Spring { .. } => Tag::Spring,
            Kind::Balloon { .. } => Tag::Balloon,
            Kind::FallFloor { .. } => Tag::FallFloor,
            Kind::Smoke => Tag::Smoke,
            Kind::Fruit { .. } => Tag::Fruit,
            Kind::FlyFruit { .. } => Tag::FlyFruit,
            Kind::Lifeup { .. } => Tag::Lifeup,
            Kind::FakeWall => Tag::FakeWall,
            Kind::Key => Tag::Key,
            Kind::Chest { .. } => Tag::Chest,
            Kind::Platform { .. } => Tag::Platform,
            Kind::Message { .. } => Tag::Message,
            Kind::BigChest { .. } => Tag::BigChest,
            Kind::Orb => Tag::Orb,
            Kind::Flag { .. } => Tag::Flag,
            Kind::RoomTitle { .. } => Tag::RoomTitle,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Obj {
    /// Identity, for `each`.
    pub id: u64,
    pub kind: Kind,
    pub dead: bool,
    pub collideable: bool,
    pub solids: bool,
    pub spr: f64,
    pub flip_x: bool,
    pub flip_y: bool,
    pub x: f64,
    pub y: f64,
    pub hitbox: Hitbox,
    pub spd: V2,
    pub rem: V2,
}

struct Cloud {
    x: f64,
    y: f64,
    spd: f64,
    w: f64,
}

struct Particle {
    x: f64,
    y: f64,
    s: f64,
    spd: f64,
    off: f64,
    c: f64,
}

struct DeadParticle {
    x: f64,
    y: f64,
    t: i32,
    spd: V2,
}

pub struct Classic {
    pub e: Pico,
    pub room: (i32, i32),
    pub objects: Vec<Obj>,
    pub freeze: i32,
    shake: i32,
    pub screen_shake: bool,
    will_restart: bool,
    delay_restart: i32,
    pub got_fruit: HashSet<i32>,
    has_dashed: bool,
    sfx_timer: i32,
    has_key: bool,
    pause_player: bool,
    flash_bg: bool,
    music_timer: i32,
    new_bg: bool,
    pub frames: i32,
    pub seconds: i32,
    pub minutes: i32,
    pub deaths: i32,
    pub max_djump: i32,
    start_game: bool,
    start_game_flash: i32,
    clouds: Vec<Cloud>,
    particles: Vec<Particle>,
    dead_particles: Vec<DeadParticle>,
    next_id: u64,
}

/// Mutable access to one kind's fields of object `$i`.
macro_rules! data {
    ($g:expr, $i:expr, $k:ident { $($f:ident),* }) => {
        let Kind::$k { $($f),* , .. } = &mut $g.objects[$i].kind else { unreachable!() };
    };
}

impl Classic {
    pub fn new(seed: u64) -> Classic {
        let mut e = Pico::new(seed);
        let clouds = (0..=16)
            .map(|_| Cloud { x: e.rnd(128.0), y: e.rnd(128.0), spd: 1.0 + e.rnd(4.0), w: 32.0 + e.rnd(32.0) })
            .collect();
        let particles = (0..=24)
            .map(|_| Particle {
                x: e.rnd(128.0),
                y: e.rnd(128.0),
                s: flr(e.rnd(5.0) / 4.0) as f64,
                spd: 0.25 + e.rnd(5.0),
                off: e.rnd(1.0),
                c: (6 + flr(0.5 + e.rnd(1.0))) as f64,
            })
            .collect();
        let mut g = Classic {
            e,
            room: (0, 0),
            objects: Vec::new(),
            freeze: 0,
            shake: 0,
            screen_shake: true,
            will_restart: false,
            delay_restart: 0,
            got_fruit: HashSet::new(),
            has_dashed: false,
            sfx_timer: 0,
            has_key: false,
            pause_player: false,
            flash_bg: false,
            music_timer: 0,
            new_bg: false,
            frames: 0,
            seconds: 0,
            minutes: 0,
            deaths: 0,
            max_djump: 1,
            start_game: false,
            start_game_flash: 0,
            clouds,
            particles,
            dead_particles: Vec::new(),
            next_id: 0,
        };
        g.title_screen();
        g
    }

    fn title_screen(&mut self) {
        self.got_fruit.clear();
        self.frames = 0;
        self.deaths = 0;
        self.max_djump = 1;
        self.start_game = false;
        self.start_game_flash = 0;
        self.e.music(40);
        self.load_room(7, 3);
    }

    fn begin_game(&mut self) {
        self.frames = 0;
        self.seconds = 0;
        self.minutes = 0;
        self.music_timer = 0;
        self.start_game = false;
        self.e.music(0);
        self.load_room(0, 0);
    }

    pub fn level_index(&self) -> i32 {
        self.room.0 % 8 + self.room.1 * 8
    }

    pub fn is_title(&self) -> bool {
        self.level_index() == 31
    }

    pub fn player(&self) -> Option<&Obj> {
        self.objects.iter().find(|o| !o.dead && o.kind.tag() == Tag::Player)
    }

    // ================================================================ objects

    /// Call `f` on each live object the way PICO-8's `foreach`/`all()` walks
    /// a table that changes under it: keep a position and the last object
    /// visited; step on only if that object is still at the position (so a
    /// deletion doesn't skip the next one), and visit objects added during
    /// the walk. When a room loads mid-walk, the walk carries on at the same
    /// position in the new room's objects, exactly as in the original.
    fn each(&mut self, mut f: impl FnMut(&mut Classic, usize)) {
        let nth = |g: &Classic, n: usize| g.objects.iter().enumerate().filter(|(_, o)| !o.dead).nth(n).map(|(i, _)| i);
        let (mut n, mut last) = (0usize, None::<u64>);
        loop {
            if let Some(p) = nth(self, n) {
                if Some(self.objects[p].id) == last {
                    n += 1;
                }
            }
            let Some(p) = nth(self, n) else { break };
            last = Some(self.objects[p].id);
            f(self, p);
        }
    }

    fn psfx(&mut self, n: u8) {
        if self.sfx_timer <= 0 {
            self.e.sfx(n);
        }
    }

    fn init_object(&mut self, kind: Kind, x: f64, y: f64, tile: Option<i32>) -> usize {
        self.next_id += 1;
        let mut o = Obj {
            id: self.next_id,
            kind,
            dead: false,
            collideable: true,
            solids: true,
            spr: 0.0,
            flip_x: false,
            flip_y: false,
            x,
            y,
            hitbox: Hitbox { x: 0, y: 0, w: 8, h: 8 },
            spd: V2::default(),
            rem: V2::default(),
        };
        if let Some(t) = tile {
            o.spr = t as f64;
        }
        let max_djump = self.max_djump;
        let fruit_count = self.got_fruit.len() as f64;
        let e = &mut self.e;
        match &mut o.kind {
            Kind::Player(p) => {
                o.spr = 1.0;
                p.djump = max_djump;
                o.hitbox = Hitbox { x: 1, y: 3, w: 6, h: 5 };
            }
            Kind::Spawn { target, state, delay, hair } => {
                o.spr = 3.0;
                *target = v2(o.x, o.y);
                o.y = 128.0;
                o.spd.y = -4.0;
                *state = 0;
                *delay = 0;
                o.solids = false;
                *hair = Hair::new(o.x, o.y);
                e.sfx(4);
            }
            Kind::Balloon { offset, start, .. } => {
                *offset = e.rnd(1.0);
                *start = o.y;
                o.hitbox = Hitbox { x: -1, y: -1, w: 10, h: 10 };
            }
            Kind::Smoke => {
                o.spr = 29.0;
                o.spd.y = -0.1;
                o.spd.x = 0.3 + e.rnd(0.2);
                o.x += -1.0 + e.rnd(2.0);
                o.y += -1.0 + e.rnd(2.0);
                o.flip_x = e.rnd(1.0) < 0.5;
                o.flip_y = e.rnd(1.0) < 0.5;
                o.solids = false;
            }
            Kind::Fruit { start, off } => {
                o.spr = 26.0;
                *start = o.y;
                *off = 0.0;
            }
            Kind::FlyFruit { start, .. } => {
                *start = o.y;
                o.solids = false;
            }
            Kind::Lifeup { duration, flash } => {
                o.spd.y = -0.25;
                *duration = 30;
                o.x -= 2.0;
                o.y -= 4.0;
                *flash = 0.0;
                o.solids = false;
            }
            Kind::Chest { start, timer } => {
                o.x -= 4.0;
                *start = o.x;
                *timer = 20.0;
            }
            Kind::Platform { last, .. } => {
                o.x -= 4.0;
                o.solids = false;
                o.hitbox.w = 16;
                *last = o.x;
            }
            Kind::BigChest { .. } => o.hitbox.w = 16,
            Kind::Orb => {
                o.spd.y = -4.0;
                o.solids = false;
            }
            Kind::Flag { score, .. } => {
                o.x += 5.0;
                *score = fruit_count;
            }
            _ => {}
        }
        self.objects.push(o);
        self.objects.len() - 1
    }

    fn destroy_object(&mut self, i: usize) {
        self.objects[i].dead = true;
    }

    fn kill_player(&mut self, i: usize) {
        self.sfx_timer = 12;
        self.e.sfx(0);
        self.deaths += 1;
        self.shake = 10;
        self.destroy_object(i);
        log::debug!("death {} in room {:?} at ({}, {})", self.deaths, self.room, self.objects[i].x, self.objects[i].y);
        self.dead_particles.clear();
        let (x, y) = (self.objects[i].x, self.objects[i].y);
        for dir in 0..=7 {
            let angle = dir as f64 / 8.0;
            self.dead_particles.push(DeadParticle { x: x + 4.0, y: y + 4.0, t: 10, spd: v2(sin(angle) * 3.0, cos(angle) * 3.0) });
        }
        self.restart_room();
    }

    fn break_spring(&mut self, i: usize) {
        data!(self, i, Spring { hide_in });
        *hide_in = 15;
    }

    fn break_fall_floor(&mut self, i: usize) {
        let Kind::FallFloor { state, delay } = &mut self.objects[i].kind else { unreachable!() };
        if *state == 0 {
            *state = 1;
            *delay = 15; // how long until it falls
            self.psfx(15);
            let (x, y) = (self.objects[i].x, self.objects[i].y);
            self.init_object(Kind::Smoke, x, y, None);
            if let Some(hit) = self.collide(i, Tag::Spring, 0, -1) {
                self.break_spring(hit);
            }
        }
    }

    // ---------------------------------------------------------- collision

    pub fn collide(&self, i: usize, tag: Tag, ox: i32, oy: i32) -> Option<usize> {
        let me = &self.objects[i];
        let (mx, my) = (me.x + (me.hitbox.x + ox) as f64, me.y + (me.hitbox.y + oy) as f64);
        self.objects.iter().enumerate().position(|(j, o)| {
            j != i
                && !o.dead
                && o.collideable
                && o.kind.tag() == tag
                && o.x + (o.hitbox.x + o.hitbox.w) as f64 > mx
                && o.y + (o.hitbox.y + o.hitbox.h) as f64 > my
                && o.x + (o.hitbox.x as f64) < mx + me.hitbox.w as f64
                && o.y + (o.hitbox.y as f64) < my + me.hitbox.h as f64
        })
    }

    fn check(&self, i: usize, tag: Tag, ox: i32, oy: i32) -> bool {
        self.collide(i, tag, ox, oy).is_some()
    }

    pub fn is_solid(&self, i: usize, ox: i32, oy: i32) -> bool {
        if oy > 0 && !self.check(i, Tag::Platform, ox, 0) && self.check(i, Tag::Platform, ox, oy) {
            return true;
        }
        let o = &self.objects[i];
        self.solid_at(o.x + (o.hitbox.x + ox) as f64, o.y + (o.hitbox.y + oy) as f64, o.hitbox.w as f64, o.hitbox.h as f64)
            || self.check(i, Tag::FallFloor, ox, oy)
            || self.check(i, Tag::FakeWall, ox, oy)
    }

    fn is_ice(&self, i: usize, ox: i32, oy: i32) -> bool {
        let o = &self.objects[i];
        self.ice_at(o.x + (o.hitbox.x + ox) as f64, o.y + (o.hitbox.y + oy) as f64, o.hitbox.w as f64, o.hitbox.h as f64)
    }

    fn move_obj(&mut self, i: usize, ox: f64, oy: f64) {
        let o = &mut self.objects[i];
        o.rem.x += ox;
        let amount = flr(o.rem.x + 0.5);
        o.rem.x -= amount as f64;
        self.move_x(i, amount, 0);

        let o = &mut self.objects[i];
        o.rem.y += oy;
        let amount = flr(o.rem.y + 0.5);
        o.rem.y -= amount as f64;
        self.move_y(i, amount);
    }

    fn move_x(&mut self, i: usize, amount: i32, start: i32) {
        if self.objects[i].solids {
            let step = amount.signum();
            for _ in start..=amount.abs() {
                if !self.is_solid(i, step, 0) {
                    self.objects[i].x += step as f64;
                } else {
                    let o = &mut self.objects[i];
                    o.spd.x = 0.0;
                    o.rem.x = 0.0;
                    break;
                }
            }
        } else {
            self.objects[i].x += amount as f64;
        }
    }

    fn move_y(&mut self, i: usize, amount: i32) {
        if self.objects[i].solids {
            let step = amount.signum();
            for _ in 0..=amount.abs() {
                if !self.is_solid(i, 0, step) {
                    self.objects[i].y += step as f64;
                } else {
                    let o = &mut self.objects[i];
                    o.spd.y = 0.0;
                    o.rem.y = 0.0;
                    break;
                }
            }
        } else {
            self.objects[i].y += amount as f64;
        }
    }

    // ================================================================ updates

    fn update_obj(&mut self, i: usize) {
        match self.objects[i].kind.tag() {
            Tag::Player => self.update_player(i),
            Tag::Spawn => self.update_spawn(i),
            Tag::Spring => self.update_spring(i),
            Tag::Balloon => self.update_balloon(i),
            Tag::FallFloor => self.update_fall_floor(i),
            Tag::Smoke => {
                let o = &mut self.objects[i];
                o.spr += 0.2;
                if o.spr >= 32.0 {
                    self.destroy_object(i);
                }
            }
            Tag::Fruit => self.update_fruit(i),
            Tag::FlyFruit => self.update_fly_fruit(i),
            Tag::Lifeup => {
                data!(self, i, Lifeup { duration });
                *duration -= 1;
                if *duration <= 0 {
                    self.destroy_object(i);
                }
            }
            Tag::FakeWall => self.update_fake_wall(i),
            Tag::Key => self.update_key(i),
            Tag::Chest => self.update_chest(i),
            Tag::Platform => self.update_platform(i),
            _ => {}
        }
    }

    fn update_player(&mut self, i: usize) {
        if self.pause_player {
            return;
        }
        // Nothing else touches the player during its own update, so work on
        // a copy and put it back at the end (keeping `dead`, which a death or
        // a room change may have set meanwhile).
        let mut o = self.objects[i].clone();
        let Kind::Player(mut p) = o.kind.clone() else { unreachable!() };
        let e_btn = |g: &Classic, b| g.e.btn(b);
        let input = if e_btn(self, RIGHT) {
            1
        } else if e_btn(self, LEFT) {
            -1
        } else {
            0
        };

        // spikes collide
        if self.spikes_at(o.x + o.hitbox.x as f64, o.y + o.hitbox.y as f64, o.hitbox.w, o.hitbox.h, o.spd.x, o.spd.y) {
            self.kill_player(i);
        }

        // bottom death
        if o.y > 128.0 {
            self.kill_player(i);
        }

        let on_ground = self.is_solid(i, 0, 1);
        let on_ice = self.is_ice(i, 0, 1);

        // smoke particles
        if on_ground && !p.was_on_ground {
            self.init_object(Kind::Smoke, o.x, o.y + 4.0, None);
        }

        let jump = e_btn(self, JUMP) && !p.p_jump;
        p.p_jump = e_btn(self, JUMP);
        if jump {
            p.jbuffer = 4;
        } else if p.jbuffer > 0 {
            p.jbuffer -= 1;
        }

        let dash = e_btn(self, DASH) && !p.p_dash;
        p.p_dash = e_btn(self, DASH);

        if on_ground {
            p.grace = 6;
            if p.djump < self.max_djump {
                self.psfx(54);
                p.djump = self.max_djump;
            }
        } else if p.grace > 0 {
            p.grace -= 1;
        }

        p.dash_effect_time -= 1;
        if p.dash_time > 0 {
            self.init_object(Kind::Smoke, o.x, o.y, None);
            p.dash_time -= 1;
            o.spd.x = appr(o.spd.x, p.dash_target.x, p.dash_accel.x);
            o.spd.y = appr(o.spd.y, p.dash_target.y, p.dash_accel.y);
        } else {
            // move
            let maxrun = 1.0;
            let mut accel = 0.6;
            let deccel = 0.15;

            if !on_ground {
                accel = 0.4;
            } else if on_ice {
                accel = 0.05;
            }

            if o.spd.x.abs() > maxrun {
                o.spd.x = appr(o.spd.x, sign(o.spd.x) as f64 * maxrun, deccel);
            } else {
                o.spd.x = appr(o.spd.x, input as f64 * maxrun, accel);
            }

            // facing
            if o.spd.x != 0.0 {
                o.flip_x = o.spd.x < 0.0;
            }

            // gravity
            let mut maxfall = 2.0;
            let mut gravity = 0.21;

            if o.spd.y.abs() <= 0.15 {
                gravity *= 0.5;
            }

            // wall slide
            if input != 0 && self.is_solid(i, input, 0) && !self.is_ice(i, input, 0) {
                maxfall = 0.4;
                if self.e.rnd(10.0) < 2.0 {
                    self.init_object(Kind::Smoke, o.x + (input * 6) as f64, o.y, None);
                }
            }

            if !on_ground {
                o.spd.y = appr(o.spd.y, maxfall, gravity);
            }

            // jump
            if p.jbuffer > 0 {
                if p.grace > 0 {
                    // normal jump
                    self.psfx(1);
                    p.jbuffer = 0;
                    p.grace = 0;
                    o.spd.y = -2.0;
                    self.init_object(Kind::Smoke, o.x, o.y + 4.0, None);
                } else {
                    // wall jump
                    let wall_dir = if self.is_solid(i, -3, 0) {
                        -1
                    } else if self.is_solid(i, 3, 0) {
                        1
                    } else {
                        0
                    };
                    if wall_dir != 0 {
                        self.psfx(2);
                        p.jbuffer = 0;
                        o.spd.y = -2.0;
                        o.spd.x = (-wall_dir) as f64 * (maxrun + 1.0);
                        if self.is_ice(i, wall_dir * 3, 0) {
                            self.init_object(Kind::Smoke, o.x + (wall_dir * 6) as f64, o.y, None);
                        }
                    }
                }
            }

            // dash
            let d_full = 5.0;
            let d_half = d_full * 0.70710678118;

            if p.djump > 0 && dash {
                self.init_object(Kind::Smoke, o.x, o.y, None);
                p.djump -= 1;
                p.dash_time = 4;
                self.has_dashed = true;
                p.dash_effect_time = 10;

                let v_input = if e_btn(self, UP) {
                    -1
                } else if e_btn(self, DOWN) {
                    1
                } else {
                    0
                };
                log::debug!("dash ({input}, {v_input}) from ({}, {})", o.x, o.y);
                if input != 0 {
                    if v_input != 0 {
                        o.spd.x = input as f64 * d_half;
                        o.spd.y = v_input as f64 * d_half;
                    } else {
                        o.spd.x = input as f64 * d_full;
                        o.spd.y = 0.0;
                    }
                } else if v_input != 0 {
                    o.spd.x = 0.0;
                    o.spd.y = v_input as f64 * d_full;
                } else {
                    // no direction: a weak dash the way the player faces
                    o.spd.x = if o.flip_x { -1.0 } else { 1.0 };
                    o.spd.y = 0.0;
                }

                self.psfx(3);
                self.freeze = 2;
                self.shake = 6;
                p.dash_target.x = 2.0 * sign(o.spd.x) as f64;
                p.dash_target.y = 2.0 * sign(o.spd.y) as f64;
                p.dash_accel.x = 1.5;
                p.dash_accel.y = 1.5;

                if o.spd.y < 0.0 {
                    p.dash_target.y *= 0.75;
                }
                if o.spd.y != 0.0 {
                    p.dash_accel.x *= 0.70710678118;
                }
                if o.spd.x != 0.0 {
                    p.dash_accel.y *= 0.70710678118;
                }
            } else if dash && p.djump <= 0 {
                self.psfx(9);
                self.init_object(Kind::Smoke, o.x, o.y, None);
            }
        }

        // animation
        p.spr_off += 0.25;
        if !on_ground {
            o.spr = if self.is_solid(i, input, 0) { 5.0 } else { 3.0 };
        } else if e_btn(self, DOWN) {
            o.spr = 6.0;
        } else if e_btn(self, UP) {
            o.spr = 7.0;
        } else if o.spd.x == 0.0 || (!e_btn(self, LEFT) && !e_btn(self, RIGHT)) {
            o.spr = 1.0;
        } else {
            o.spr = 1.0 + p.spr_off % 4.0;
        }

        // write back before a room change can read the object list
        p.was_on_ground = on_ground;
        o.kind = Kind::Player(p);
        o.dead = self.objects[i].dead;
        self.objects[i] = o;

        // next level
        if self.objects[i].y < -4.0 && self.level_index() < 30 {
            self.next_room();
        }
    }

    fn update_spawn(&mut self, i: usize) {
        let shake;
        {
            let o = &mut self.objects[i];
            let Kind::Spawn { target, state, delay, .. } = &mut o.kind else { unreachable!() };
            shake = match *state {
                // jumping up
                0 => {
                    if o.y < target.y + 16.0 {
                        *state = 1;
                        *delay = 3;
                    }
                    false
                }
                // falling
                1 => {
                    o.spd.y += 0.5;
                    if o.spd.y > 0.0 && *delay > 0 {
                        o.spd.y = 0.0;
                        *delay -= 1;
                    }
                    if o.spd.y > 0.0 && o.y > target.y {
                        o.y = target.y;
                        o.spd = V2::default();
                        *state = 2;
                        *delay = 5;
                        true
                    } else {
                        false
                    }
                }
                // landing
                _ => {
                    *delay -= 1;
                    o.spr = 6.0;
                    if *delay < 0 {
                        let (x, y) = (o.x, o.y);
                        self.destroy_object(i);
                        let j = self.init_object(Kind::Player(Box::new(new_player(Hair::new(x, y)))), x, y, None);
                        log::debug!("player spawned in room {:?} as #{j}", self.room);
                    }
                    false
                }
            };
        }
        if shake {
            self.shake = 5;
            let (x, y) = (self.objects[i].x, self.objects[i].y);
            self.init_object(Kind::Smoke, x, y + 4.0, None);
            self.e.sfx(5);
        }
    }

    fn update_spring(&mut self, i: usize) {
        data!(self, i, Spring { hide_for, delay });
        if *hide_for > 0 {
            *hide_for -= 1;
            if *hide_for <= 0 {
                *delay = 0;
                self.objects[i].spr = 18.0;
            }
        } else if self.objects[i].spr == 18.0 {
            if let Some(h) = self.collide(i, Tag::Player, 0, 0).filter(|&h| self.objects[h].spd.y >= 0.0) {
                let (x, y) = (self.objects[i].x, self.objects[i].y);
                self.objects[i].spr = 19.0;
                let max_djump = self.max_djump;
                let hit = &mut self.objects[h];
                hit.y = y - 4.0;
                hit.spd.x *= 0.2;
                hit.spd.y = -3.0;
                if let Kind::Player(p) = &mut hit.kind {
                    p.djump = max_djump;
                }
                data!(self, i, Spring { delay });
                *delay = 10;
                self.init_object(Kind::Smoke, x, y, None);

                // breakable below us
                if let Some(below) = self.collide(i, Tag::FallFloor, 0, 1) {
                    self.break_fall_floor(below);
                }

                self.psfx(8);
            }
        } else {
            data!(self, i, Spring { delay });
            if *delay > 0 {
                *delay -= 1;
                if *delay <= 0 {
                    self.objects[i].spr = 18.0;
                }
            }
        }

        // begin hiding
        data!(self, i, Spring { hide_in, hide_for });
        if *hide_in > 0 {
            *hide_in -= 1;
            if *hide_in <= 0 {
                *hide_for = 60;
                self.objects[i].spr = 0.0;
            }
        }
    }

    fn update_balloon(&mut self, i: usize) {
        if self.objects[i].spr == 22.0 {
            {
                let o = &mut self.objects[i];
                let Kind::Balloon { offset, start, .. } = &mut o.kind else { unreachable!() };
                *offset += 0.01;
                o.y = *start + sin(*offset) * 2.0;
            }
            if let Some(h) = self.collide(i, Tag::Player, 0, 0) {
                let max_djump = self.max_djump;
                if let Kind::Player(p) = &mut self.objects[h].kind {
                    if p.djump < max_djump {
                        p.djump = max_djump;
                        self.psfx(6);
                        let (x, y) = (self.objects[i].x, self.objects[i].y);
                        self.init_object(Kind::Smoke, x, y, None);
                        self.objects[i].spr = 0.0;
                        data!(self, i, Balloon { timer });
                        *timer = 60.0;
                    }
                }
            }
        } else {
            data!(self, i, Balloon { timer });
            if *timer > 0.0 {
                *timer -= 1.0;
            } else {
                self.psfx(7);
                let (x, y) = (self.objects[i].x, self.objects[i].y);
                self.init_object(Kind::Smoke, x, y, None);
                self.objects[i].spr = 22.0;
            }
        }
    }

    fn update_fall_floor(&mut self, i: usize) {
        let Kind::FallFloor { state, .. } = self.objects[i].kind else { unreachable!() };
        match state {
            0 => {
                if self.check(i, Tag::Player, 0, -1) || self.check(i, Tag::Player, -1, 0) || self.check(i, Tag::Player, 1, 0) {
                    self.break_fall_floor(i);
                }
            }
            1 => {
                data!(self, i, FallFloor { state, delay });
                *delay -= 1;
                if *delay <= 0 {
                    *state = 2;
                    *delay = 60; // how long it hides for
                    self.objects[i].collideable = false;
                }
            }
            _ => {
                data!(self, i, FallFloor { delay });
                *delay -= 1;
                if *delay <= 0 && !self.check(i, Tag::Player, 0, 0) {
                    self.psfx(7);
                    data!(self, i, FallFloor { state });
                    *state = 0;
                    self.objects[i].collideable = true;
                    let (x, y) = (self.objects[i].x, self.objects[i].y);
                    self.init_object(Kind::Smoke, x, y, None);
                }
            }
        }
    }

    /// A strawberry touched by the player: restore the dash, count it, show
    /// "1000".
    fn collect_fruit(&mut self, i: usize, h: usize) {
        let max_djump = self.max_djump;
        if let Kind::Player(p) = &mut self.objects[h].kind {
            p.djump = max_djump;
        }
        self.sfx_timer = 20;
        self.e.sfx(13);
        self.got_fruit.insert(1 + self.level_index());
        log::debug!("strawberry in level {} ({} total)", self.level_index(), self.got_fruit.len());
        let (x, y) = (self.objects[i].x, self.objects[i].y);
        self.init_object(Kind::Lifeup { duration: 0, flash: 0.0 }, x, y, None);
        self.destroy_object(i);
    }

    fn update_fruit(&mut self, i: usize) {
        if let Some(h) = self.collide(i, Tag::Player, 0, 0) {
            self.collect_fruit(i, h);
        }
        let o = &mut self.objects[i];
        let Kind::Fruit { start, off } = &mut o.kind else { unreachable!() };
        *off += 1.0;
        o.y = *start + sin(*off / 40.0) * 2.5;
    }

    fn update_fly_fruit(&mut self, i: usize) {
        let has_dashed = self.has_dashed;
        let mut play = false;
        let mut gone = false;
        {
            let o = &mut self.objects[i];
            let Kind::FlyFruit { fly, step, sfx_delay, .. } = &mut o.kind else { unreachable!() };
            // fly away
            if *fly {
                if *sfx_delay > 0.0 {
                    *sfx_delay -= 1.0;
                    if *sfx_delay <= 0.0 {
                        play = true;
                    }
                }
                o.spd.y = appr(o.spd.y, -3.5, 0.25);
                if o.y < -16.0 {
                    gone = true;
                }
            }
            // wait
            else {
                if has_dashed {
                    *fly = true;
                }
                *step += 0.05;
                o.spd.y = sin(*step) * 0.5;
            }
        }
        if play {
            self.sfx_timer = 20;
            self.e.sfx(14);
        }
        if gone {
            self.destroy_object(i);
        }
        // collect
        if let Some(h) = self.collide(i, Tag::Player, 0, 0) {
            self.collect_fruit(i, h);
        }
    }

    fn update_fake_wall(&mut self, i: usize) {
        self.objects[i].hitbox = Hitbox { x: -1, y: -1, w: 18, h: 18 };
        if let Some(h) = self.collide(i, Tag::Player, 0, 0) {
            let hit = &mut self.objects[h];
            let Kind::Player(p) = &mut hit.kind else { unreachable!() };
            if p.dash_effect_time > 0 {
                p.dash_time = -1;
                hit.spd.x = -sign(hit.spd.x) as f64 * 1.5;
                hit.spd.y = -1.5;
                self.sfx_timer = 20;
                self.e.sfx(16);
                self.destroy_object(i);
                let (x, y) = (self.objects[i].x, self.objects[i].y);
                self.init_object(Kind::Smoke, x, y, None);
                self.init_object(Kind::Smoke, x + 8.0, y, None);
                self.init_object(Kind::Smoke, x, y + 8.0, None);
                self.init_object(Kind::Smoke, x + 8.0, y + 8.0, None);
                self.init_object(Kind::Fruit { start: 0.0, off: 0.0 }, x + 4.0, y + 4.0, None);
            }
        }
        self.objects[i].hitbox = Hitbox { x: 0, y: 0, w: 16, h: 16 };
    }

    fn update_key(&mut self, i: usize) {
        let frames = self.frames;
        let o = &mut self.objects[i];
        let was = flr(o.spr);
        o.spr = 9.0 + (sin(frames as f64 / 30.0) + 0.5) * 1.0;
        let current = flr(o.spr);
        if current == 10 && current != was {
            o.flip_x = !o.flip_x;
        }
        if self.check(i, Tag::Player, 0, 0) {
            self.e.sfx(23);
            self.sfx_timer = 10;
            self.destroy_object(i);
            self.has_key = true;
        }
    }

    fn update_chest(&mut self, i: usize) {
        if self.has_key {
            let r = self.e.rnd(3.0);
            let o = &mut self.objects[i];
            let Kind::Chest { start, timer } = &mut o.kind else { unreachable!() };
            *timer -= 1.0;
            o.x = *start - 1.0 + r;
            if *timer <= 0.0 {
                self.sfx_timer = 20;
                self.e.sfx(16);
                let (x, y) = (self.objects[i].x, self.objects[i].y);
                self.init_object(Kind::Fruit { start: 0.0, off: 0.0 }, x, y - 4.0, None);
                self.destroy_object(i);
            }
        }
    }

    fn update_platform(&mut self, i: usize) {
        {
            let o = &mut self.objects[i];
            let Kind::Platform { dir, .. } = o.kind else { unreachable!() };
            o.spd.x = dir * 0.65;
            if o.x < -16.0 {
                o.x = 128.0;
            }
            if o.x > 128.0 {
                o.x = -16.0;
            }
        }
        if !self.check(i, Tag::Player, 0, 0) {
            if let Some(h) = self.collide(i, Tag::Player, 0, -1) {
                let Kind::Platform { last, .. } = self.objects[i].kind else { unreachable!() };
                let amount = (self.objects[i].x - last) as i32;
                self.move_x(h, amount, 1);
            }
        }
        let x = self.objects[i].x;
        data!(self, i, Platform { last });
        *last = x;
    }

    // ================================================================ draws

    fn draw_object(&mut self, i: usize) {
        match self.objects[i].kind.tag() {
            Tag::Player => {
                let o = &mut self.objects[i];
                // clamp in screen
                if o.x < -1.0 || o.x > 121.0 {
                    o.x = o.x.clamp(-1.0, 121.0);
                    o.spd.x = 0.0;
                }
                let (x, y, facing) = (o.x, o.y, if o.flip_x { -1 } else { 1 });
                let Kind::Player(p) = &mut o.kind else { unreachable!() };
                let djump = p.djump;
                p.hair.draw(&mut self.e, x, y, facing, djump, self.frames);
                self.draw_player(i, djump);
            }
            Tag::Spawn => {
                let o = &mut self.objects[i];
                let (x, y) = (o.x, o.y);
                let Kind::Spawn { hair, .. } = &mut o.kind else { unreachable!() };
                hair.draw(&mut self.e, x, y, 1, self.max_djump, self.frames);
                self.draw_player(i, self.max_djump);
            }
            Tag::Balloon => {
                let o = &self.objects[i];
                if o.spr == 22.0 {
                    let Kind::Balloon { offset, .. } = o.kind else { unreachable!() };
                    let (x, y, spr) = (o.x, o.y, o.spr);
                    self.e.spr(13.0 + (offset * 8.0) % 3.0, x, y + 6.0, false, false);
                    self.e.spr(spr, x, y, false, false);
                }
            }
            Tag::FallFloor => {
                let o = &self.objects[i];
                let Kind::FallFloor { state, delay } = o.kind else { unreachable!() };
                if state != 2 {
                    let s = if state != 1 { 23 } else { 23 + (15 - delay) / 5 };
                    self.e.spr(s as f64, o.x, o.y, false, false);
                }
            }
            Tag::FlyFruit => {
                let o = &self.objects[i];
                let Kind::FlyFruit { start, fly, step, .. } = o.kind else { unreachable!() };
                let mut off = 0.0f64;
                if !fly {
                    let dir = sin(step);
                    if dir < 0.0 {
                        off = 1.0 + 0f64.max(sign(o.y - start) as f64);
                    }
                } else {
                    off = (off + 0.25) % 3.0;
                }
                let (x, y, spr) = (o.x, o.y, o.spr);
                self.e.spr(45.0 + off, x - 6.0, y - 2.0, true, false);
                self.e.spr(spr, x, y, false, false);
                self.e.spr(45.0 + off, x + 6.0, y - 2.0, false, false);
            }
            Tag::Lifeup => {
                let o = &mut self.objects[i];
                let Kind::Lifeup { flash, .. } = &mut o.kind else { unreachable!() };
                *flash += 0.5;
                let (x, y, c) = (o.x, o.y, 7.0 + *flash % 2.0);
                self.e.print("1000", x - 2.0, y, c);
            }
            Tag::FakeWall => {
                let (x, y) = (self.objects[i].x, self.objects[i].y);
                self.e.spr(64.0, x, y, false, false);
                self.e.spr(65.0, x + 8.0, y, false, false);
                self.e.spr(80.0, x, y + 8.0, false, false);
                self.e.spr(81.0, x + 8.0, y + 8.0, false, false);
            }
            Tag::Platform => {
                let (x, y) = (self.objects[i].x, self.objects[i].y);
                self.e.spr(11.0, x, y - 1.0, false, false);
                self.e.spr(12.0, x + 8.0, y - 1.0, false, false);
            }
            Tag::Message => self.draw_message(i),
            Tag::BigChest => self.draw_big_chest(i),
            Tag::Orb => self.draw_orb(i),
            Tag::Flag => self.draw_flag(i),
            Tag::RoomTitle => self.draw_room_title(i),
            _ => {
                let o = &self.objects[i];
                if o.spr > 0.0 {
                    let (s, x, y, fx, fy) = (o.spr, o.x, o.y, o.flip_x, o.flip_y);
                    self.e.spr(s, x, y, fx, fy);
                }
            }
        }
    }

    fn draw_player(&mut self, i: usize, djump: i32) {
        let push = if djump == 2 {
            if (self.frames / 3) % 2 == 0 {
                10 * 16
            } else {
                9 * 16
            }
        } else if djump == 0 {
            8 * 16
        } else {
            0
        };
        let o = &self.objects[i];
        let (s, x, y, fx, fy) = (o.spr + push as f64, o.x, o.y, o.flip_x, o.flip_y);
        self.e.spr(s, x, y, fx, fy);
    }

    fn draw_message(&mut self, i: usize) {
        let text: Vec<char> = "-- ceres mountain --#this memorial to those# perished on the climb".chars().collect();
        if self.check(i, Tag::Player, 4, 0) {
            let mut tick = false;
            let idx;
            {
                data!(self, i, Message { last, index });
                if (*index as usize) < text.len() {
                    *index += 0.5;
                    if *index >= *last + 1.0 {
                        *last += 1.0;
                        tick = true;
                    }
                }
                idx = *index;
            }
            if tick {
                self.e.sfx(35);
            }
            let mut off = v2(8.0, 96.0);
            let mut k = 0;
            while (k as f64) < idx && k < text.len() {
                if text[k] != '#' {
                    self.e.rectfill(off.x - 2.0, off.y - 2.0, off.x + 7.0, off.y + 6.0, 7.0);
                    self.e.print(&text[k].to_string(), off.x, off.y, 0.0);
                    off.x += 5.0;
                } else {
                    off.x = 8.0;
                    off.y += 7.0;
                }
                k += 1;
            }
        } else {
            data!(self, i, Message { last, index });
            *index = 0.0;
            *last = 0.0;
        }
    }

    fn draw_big_chest(&mut self, i: usize) {
        let (x, y) = (self.objects[i].x, self.objects[i].y);
        let Kind::BigChest { state, .. } = self.objects[i].kind else { unreachable!() };
        if state == 0 {
            if let Some(h) = self.collide(i, Tag::Player, 0, 8).filter(|&h| self.is_solid(h, 0, 1)) {
                self.e.music(-1);
                self.e.sfx(37);
                self.pause_player = true;
                self.objects[h].spd = V2::default();
                data!(self, i, BigChest { state, timer, particles });
                *state = 1;
                *timer = 60.0;
                particles.clear();
                self.init_object(Kind::Smoke, x, y, None);
                self.init_object(Kind::Smoke, x + 8.0, y, None);
                log::debug!("big chest opened");
            }
            self.e.spr(96.0, x, y, false, false);
            self.e.spr(97.0, x + 8.0, y, false, false);
        } else if state == 1 {
            self.shake = 5;
            self.flash_bg = true;
            let done;
            {
                data!(self, i, BigChest { timer, particles });
                *timer -= 1.0;
                let add = *timer <= 45.0 && particles.len() < 50;
                if add {
                    let fresh = [1.0 + self.e.rnd(14.0), 0.0, 32.0 + self.e.rnd(32.0), 8.0 + self.e.rnd(8.0)];
                    data!(self, i, BigChest { particles });
                    particles.push(fresh);
                }
                data!(self, i, BigChest { state, timer, particles });
                done = *timer < 0.0;
                if done {
                    *state = 2;
                    particles.clear();
                }
            }
            if done {
                self.flash_bg = false;
                self.new_bg = true;
                self.init_object(Kind::Orb, x + 4.0, y + 4.0, None);
                self.pause_player = false;
            }
            data!(self, i, BigChest { particles });
            let mut ps = std::mem::take(particles);
            for p in &mut ps {
                p[1] += p[3];
                self.e.rectfill(x + p[0], y + 8.0 - p[1], x + p[0] + 1.0, (y + 8.0 - p[1] + p[2]).min(y + 8.0), 7.0);
            }
            data!(self, i, BigChest { particles });
            *particles = ps;
        }
        self.e.spr(112.0, x, y + 8.0, false, false);
        self.e.spr(113.0, x + 8.0, y + 8.0, false, false);
    }

    fn draw_orb(&mut self, i: usize) {
        let o = &mut self.objects[i];
        o.spd.y = appr(o.spd.y, 0.0, 0.5);
        let still = o.spd.y == 0.0;
        if let Some(h) = self.collide(i, Tag::Player, 0, 0).filter(|_| still) {
            self.music_timer = 45;
            self.e.sfx(51);
            self.freeze = 10;
            self.shake = 10;
            self.destroy_object(i);
            self.max_djump = 2;
            if let Kind::Player(p) = &mut self.objects[h].kind {
                p.djump = 2;
            }
            log::debug!("orb taken: two dashes");
        }
        let (x, y) = (self.objects[i].x, self.objects[i].y);
        self.e.spr(102.0, x, y, false, false);
        let off = self.frames as f64 / 30.0;
        for k in 0..=7 {
            let a = off + k as f64 / 8.0;
            self.e.circfill(x + 4.0 + cos(a) * 8.0, y + 4.0 + sin(a) * 8.0, 1.0, 7.0);
        }
    }

    fn draw_flag(&mut self, i: usize) {
        let spr = 118.0 + (self.frames as f64 / 5.0) % 3.0;
        self.objects[i].spr = spr;
        let (x, y) = (self.objects[i].x, self.objects[i].y);
        self.e.spr(spr, x, y, false, false);
        let Kind::Flag { score, show } = self.objects[i].kind else { unreachable!() };
        if show {
            self.e.rectfill(32.0, 2.0, 96.0, 31.0, 0.0);
            self.e.spr(26.0, 55.0, 6.0, false, false);
            self.e.print(&format!("x{score}"), 64.0, 9.0, 7.0);
            self.draw_time(49, 16);
            let d = format!("deaths:{}", self.deaths);
            self.e.print(&d, 48.0, 24.0, 7.0);
        } else if self.check(i, Tag::Player, 0, 0) {
            self.e.sfx(55);
            self.sfx_timer = 30;
            data!(self, i, Flag { show });
            *show = true;
            log::info!("summit flag: {} berries, {} deaths, {}:{:02}", score, self.deaths, self.minutes, self.seconds);
        }
    }

    fn draw_room_title(&mut self, i: usize) {
        data!(self, i, RoomTitle { delay });
        *delay -= 1.0;
        let d = *delay;
        if d < -30.0 {
            self.destroy_object(i);
        } else if d < 0.0 {
            self.e.rectfill(24.0, 58.0, 104.0, 70.0, 0.0);
            if self.room == (3, 1) {
                self.e.print("old site", 48.0, 62.0, 7.0);
            } else if self.level_index() == 30 {
                self.e.print("summit", 52.0, 62.0, 7.0);
            } else {
                let level = (1 + self.level_index()) * 100;
                let x = 52 + if level < 1000 { 2 } else { 0 };
                self.e.print(&format!("{level} m"), x as f64, 62.0, 7.0);
            }
            self.draw_time(4, 4);
        }
    }

    // ================================================================ rooms

    fn restart_room(&mut self) {
        self.will_restart = true;
        self.delay_restart = 15;
    }

    fn next_room(&mut self) {
        match self.room {
            (2, 1) | (4, 2) | (5, 3) => self.e.music(30),
            (3, 1) => self.e.music(20),
            _ => {}
        }
        if self.room.0 == 7 {
            self.load_room(0, self.room.1 + 1);
        } else {
            self.load_room(self.room.0 + 1, self.room.1);
        }
    }

    pub fn load_room(&mut self, x: i32, y: i32) {
        self.has_dashed = false;
        self.has_key = false;

        // remove existing objects
        for o in &mut self.objects {
            o.dead = true;
        }

        // current room
        self.room = (x, y);
        log::debug!("room {:?} level {}", self.room, self.level_index());

        // entities
        for tx in 0..=15 {
            for ty in 0..=15 {
                let tile = self.e.mget(x * 16 + tx, y * 16 + ty);
                let (px, py) = ((tx * 8) as f64, (ty * 8) as f64);
                if tile == 11 || tile == 12 {
                    let j = self.init_object(Kind::Platform { dir: 0.0, last: 0.0 }, px, py, None);
                    data!(self, j, Platform { dir });
                    *dir = if tile == 11 { -1.0 } else { 1.0 };
                    continue;
                }
                let kind = match tile {
                    1 => Some(Kind::Spawn { target: V2::default(), state: 0, delay: 0, hair: Hair::new(0.0, 0.0) }),
                    18 => Some(Kind::Spring { hide_in: 0, hide_for: 0, delay: 0 }),
                    22 => Some(Kind::Balloon { offset: 0.0, start: 0.0, timer: 0.0 }),
                    23 => Some(Kind::FallFloor { state: 0, delay: 0 }),
                    86 => Some(Kind::Message { last: 0.0, index: 0.0 }),
                    96 => Some(Kind::BigChest { state: 0, timer: 0.0, particles: Vec::new() }),
                    118 => Some(Kind::Flag { score: 0.0, show: false }),
                    _ if !self.got_fruit.contains(&(1 + self.level_index())) => match tile {
                        26 => Some(Kind::Fruit { start: 0.0, off: 0.0 }),
                        28 => Some(Kind::FlyFruit { start: 0.0, fly: false, step: 0.5, sfx_delay: 8.0 }),
                        64 => Some(Kind::FakeWall),
                        8 => Some(Kind::Key),
                        20 => Some(Kind::Chest { start: 0.0, timer: 0.0 }),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(k) = kind {
                    self.init_object(k, px, py, Some(tile));
                }
            }
        }

        if !self.is_title() {
            self.init_object(Kind::RoomTitle { delay: 5.0 }, 0.0, 0.0, None);
        }
    }

    // ================================================================ update

    pub fn update(&mut self) {
        self.frames = (self.frames + 1) % 30;
        if self.frames == 0 && self.level_index() < 30 {
            self.seconds = (self.seconds + 1) % 60;
            if self.seconds == 0 {
                self.minutes += 1;
            }
        }

        if self.music_timer > 0 {
            self.music_timer -= 1;
            if self.music_timer <= 0 {
                self.e.music(10);
            }
        }

        if self.sfx_timer > 0 {
            self.sfx_timer -= 1;
        }

        // cancel if freeze
        if self.freeze > 0 {
            self.freeze -= 1;
            return;
        }

        // screenshake
        if self.shake > 0 {
            self.shake -= 1;
            self.e.camera(0.0, 0.0);
            if self.shake > 0 && self.screen_shake {
                let (sx, sy) = (-2.0 + self.e.rnd(5.0), -2.0 + self.e.rnd(5.0));
                self.e.camera(sx, sy);
            }
        }

        // restart (soon)
        if self.will_restart && self.delay_restart > 0 {
            self.delay_restart -= 1;
            if self.delay_restart <= 0 {
                self.will_restart = false;
                self.load_room(self.room.0, self.room.1);
            }
        }

        // update each object
        self.objects.retain(|o| !o.dead);
        self.each(|g, i| {
            let spd = g.objects[i].spd;
            g.move_obj(i, spd.x, spd.y);
            g.update_obj(i);
        });
        self.objects.retain(|o| !o.dead);

        // start game
        if self.is_title() {
            if !self.start_game && (self.e.btn(JUMP) || self.e.btn(DASH)) {
                self.e.music(-1);
                self.start_game_flash = 50;
                self.start_game = true;
                self.e.sfx(38);
            }
            if self.start_game {
                self.start_game_flash -= 1;
                if self.start_game_flash <= -30 {
                    self.begin_game();
                }
            }
        }
    }

    // ================================================================ draw

    pub fn draw(&mut self) {
        // reset all palette values
        self.e.pal_reset();

        // start game flash
        if self.start_game {
            let mut c = 10;
            if self.start_game_flash > 10 {
                if self.frames % 10 < 5 {
                    c = 7;
                }
            } else if self.start_game_flash > 5 {
                c = 2;
            } else if self.start_game_flash > 0 {
                c = 1;
            } else {
                c = 0;
            }
            if c < 10 {
                for a in [6, 12, 13, 5, 1, 7] {
                    self.e.pal[a] = c;
                }
            }
        }

        // clear screen
        let mut bg_col = 0;
        if self.flash_bg {
            bg_col = self.frames / 5;
        } else if self.new_bg {
            bg_col = 2;
        }
        self.e.rectfill(0.0, 0.0, 128.0, 128.0, bg_col as f64);

        // clouds
        if !self.is_title() {
            let col = if self.new_bg { 14.0 } else { 1.0 };
            for k in 0..self.clouds.len() {
                let c = &mut self.clouds[k];
                c.x += c.spd;
                let (x, y, w) = (c.x, c.y, c.w);
                self.e.rectfill(x, y, x + w, y + 4.0 + (1.0 - w / 64.0) * 12.0, col);
                if x > 128.0 {
                    let ny = self.e.rnd(128.0 - 8.0);
                    let c = &mut self.clouds[k];
                    c.x = -c.w;
                    c.y = ny;
                }
            }
        }

        let (rx, ry) = (self.room.0 * 16, self.room.1 * 16);
        // draw bg terrain
        self.e.map(rx, ry, 0, 0, 16, 16, 2);

        // platforms / big chest
        self.each(|g, i| {
            if matches!(g.objects[i].kind.tag(), Tag::Platform | Tag::BigChest) {
                g.draw_object(i);
            }
        });

        // draw terrain
        let off = if self.is_title() { -4 } else { 0 };
        self.e.map(rx, ry, off, 0, 16, 16, 1);

        // draw objects
        self.each(|g, i| {
            if !matches!(g.objects[i].kind.tag(), Tag::Platform | Tag::BigChest) {
                g.draw_object(i);
            }
        });

        // draw fg terrain
        self.e.map(rx, ry, 0, 0, 16, 16, 3);

        // particles
        for k in 0..self.particles.len() {
            let p = &mut self.particles[k];
            p.x += p.spd;
            p.y += sin(p.off);
            p.off += 0.05f64.min(p.spd / 32.0);
            let (x, y, s, c) = (p.x, p.y, p.s, p.c);
            self.e.rectfill(x, y, x + s, y + s, c);
            if x > 128.0 + 4.0 {
                let ny = self.e.rnd(128.0);
                let p = &mut self.particles[k];
                p.x = -4.0;
                p.y = ny;
            }
        }

        // dead particles
        let mut k = self.dead_particles.len();
        while k > 0 {
            k -= 1;
            let p = &mut self.dead_particles[k];
            p.x += p.spd.x;
            p.y += p.spd.y;
            p.t -= 1;
            let (x, y, t) = (p.x, p.y, p.t);
            if t <= 0 {
                self.dead_particles.remove(k);
            }
            let r = (t / 5) as f64;
            self.e.rectfill(x - r, y - r, x + r, y + r, (14 + t % 2) as f64);
        }

        // draw outside of the screen for screenshake
        self.e.rectfill(-5.0, -5.0, -1.0, 133.0, 0.0);
        self.e.rectfill(-5.0, -5.0, 133.0, -1.0, 0.0);
        self.e.rectfill(-5.0, 128.0, 133.0, 133.0, 0.0);
        self.e.rectfill(128.0, -5.0, 133.0, 133.0, 0.0);

        if self.is_title() {
            self.e.print("z+x", 58.0, 80.0, 5.0);
            self.e.print("maddy thorson", 38.0, 96.0, 5.0);
            self.e.print("noel berry", 44.0, 102.0, 5.0);
        }

        if self.level_index() == 30 {
            if let Some(p) = self.player() {
                let diff = 24f64.min(40.0 - (p.x + 4.0 - 64.0).abs());
                self.e.rectfill(0.0, 0.0, diff, 128.0, 0.0);
                self.e.rectfill(128.0 - diff, 0.0, 128.0, 128.0, 0.0);
            }
        }
    }

    fn draw_time(&mut self, x: i32, y: i32) {
        let s = self.seconds;
        let m = self.minutes % 60;
        let h = self.minutes / 60;
        let (x, y) = (x as f64, y as f64);
        self.e.rectfill(x, y, x + 32.0, y + 6.0, 0.0);
        self.e.print(&format!("{h:02}:{m:02}:{s:02}"), x + 1.0, y + 1.0, 7.0);
    }

    // ================================================================ util

    fn solid_at(&self, x: f64, y: f64, w: f64, h: f64) -> bool {
        self.tile_flag_at(x, y, w, h, 0)
    }

    fn ice_at(&self, x: f64, y: f64, w: f64, h: f64) -> bool {
        self.tile_flag_at(x, y, w, h, 4)
    }

    /// Tiles under a box, clipped to the room. The C# compares an integer
    /// index against a float bound, which flooring reproduces.
    fn tiles(x: f64, y: f64, w: f64, h: f64) -> impl Iterator<Item = (i32, i32)> {
        let (i0, i1) = (0.max(flr(x / 8.0)), flr(15f64.min((x + w - 1.0) / 8.0)));
        let (j0, j1) = (0.max(flr(y / 8.0)), flr(15f64.min((y + h - 1.0) / 8.0)));
        (i0..=i1).flat_map(move |i| (j0..=j1).map(move |j| (i, j)))
    }

    fn tile_flag_at(&self, x: f64, y: f64, w: f64, h: f64, flag: u32) -> bool {
        Self::tiles(x, y, w, h).any(|(i, j)| self.e.fget(self.tile_at(i, j), flag))
    }

    fn tile_at(&self, x: i32, y: i32) -> i32 {
        self.e.mget(self.room.0 * 16 + x, self.room.1 * 16 + y)
    }

    fn spikes_at(&self, x: f64, y: f64, w: i32, h: i32, xspd: f64, yspd: f64) -> bool {
        let (wf, hf) = (w as f64, h as f64);
        Self::tiles(x, y, wf, hf).any(|(i, j)| {
            let tile = self.tile_at(i, j);
            (tile == 17 && (modulo(y + hf - 1.0, 8.0) >= 6.0 || y + hf == (j * 8 + 8) as f64) && yspd >= 0.0)
                || (tile == 27 && modulo(y, 8.0) <= 2.0 && yspd <= 0.0)
                || (tile == 43 && modulo(x, 8.0) <= 2.0 && xspd <= 0.0)
                || (tile == 59 && (modulo(x + wf - 1.0, 8.0) >= 6.0 || x + wf == (i * 8 + 8) as f64) && xspd >= 0.0)
        })
    }
}

fn new_player(hair: Hair) -> PlayerD {
    PlayerD {
        p_jump: false,
        p_dash: false,
        grace: 0,
        jbuffer: 0,
        djump: 0,
        dash_time: 0,
        dash_effect_time: 0,
        dash_target: V2::default(),
        dash_accel: V2::default(),
        spr_off: 0.0,
        was_on_ground: false,
        hair,
    }
}

fn appr(val: f64, target: f64, amount: f64) -> f64 {
    if val > target {
        (val - amount).max(target)
    } else {
        (val + amount).min(target)
    }
}
