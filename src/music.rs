//! The soundtrack: original tunes, synthesised live, one endless source.
//!
//! The game asks for music by PICO-8 pattern number, at the same moments as
//! the original: 40 the title, 0 the first climb, 30 the windy passages
//! between sections, 20 the Old Site, 10 from the orb to the summit, -1
//! silence. Each tune here is our own: a lead line, a broken-chord
//! accompaniment, a bass and drums, written as text one sixteenth note per
//! token (`a4` a note, `.` hold, `-` rest).

use rodio::Source;
use std::f32::consts::TAU;
use std::num::NonZero;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SR: u32 = 44_100;
const SRF: f32 = SR as f32;

#[derive(Clone, Copy, PartialEq)]
enum Wave {
    Tri,
    Square,
    Pulse,
}

#[derive(Clone, Copy, PartialEq)]
enum Bass {
    /// Root on beat one, fifth on beat three, held.
    Long,
    /// Eighth notes, root and octave.
    Drive,
    /// One low note per bar.
    Drone,
}

struct Tune {
    name: &'static str,
    bpm: f32,
    /// Chord tones per bar, low to high; the bass takes the first.
    chords: &'static [&'static str],
    lead: &'static str,
    lead_wave: Wave,
    bass: Bass,
    /// Sixteen steps: `k` kick, `s` snare, `h` hat, `.` nothing.
    drums: &'static str,
    /// Accompaniment loudness (0 for none).
    arp: f32,
    wind: bool,
}

const TUNES: [Tune; 5] = [
    Tune {
        name: "title",
        bpm: 80.0,
        chords: &["a3 c4 e4", "f3 a3 c4", "c4 e4 g4", "g3 b3 d4", "a3 c4 e4", "f3 a3 c4", "c4 e4 g4", "g3 b3 d4"],
        lead: "e5 . . . c5 . . . a4 . . . b4 . c5 . \
               a4 . . . . . . . - - - - c5 . a4 . \
               g4 . . . e4 . . . g4 . . . c5 . . . \
               b4 . . . . . . . d5 . . . b4 . g4 . \
               e5 . . . a5 . . . g5 . e5 . c5 . . . \
               d5 . c5 . a4 . . . . . . . - - - - \
               e5 . . . g5 . . . e5 . d5 . c5 . . . \
               d5 . . . . . . . b4 . . . - - - -",
        lead_wave: Wave::Tri,
        bass: Bass::Long,
        drums: "",
        arp: 0.06,
        wind: false,
    },
    Tune {
        name: "climb",
        bpm: 116.0,
        chords: &["a3 c4 e4", "f3 a3 c4", "c4 e4 g4", "g3 b3 d4", "a3 c4 e4", "f3 a3 c4", "g3 b3 d4", "e3 g#3 b3"],
        lead: "a4 . c5 . e5 . . . d5 . c5 . b4 . c5 . \
               a4 . . . . . . . f4 . a4 . c5 . . . \
               g4 . . . c5 . . . e5 . . . d5 . c5 . \
               b4 . . . . . . . g4 . a4 . b4 . d5 . \
               e5 . . . . . e5 . d5 . c5 . d5 . e5 . \
               f5 . . . e5 . . . c5 . . . a4 . . . \
               d5 . . . b4 . d5 . g5 . . . f5 . d5 . \
               e5 . . . . . . . g#4 . . . b4 . . .",
        lead_wave: Wave::Pulse,
        bass: Bass::Drive,
        drums: "k . h . s . h . k . k h s . h h",
        arp: 0.05,
        wind: false,
    },
    Tune {
        name: "old site",
        bpm: 96.0,
        chords: &["d4 f4 a4", "a#3 d4 f4", "c4 e4 g4", "a3 c4 e4", "d4 f4 a4", "a#3 d4 f4", "g3 a#3 d4", "a3 c#4 e4"],
        lead: "d5 . . . . . . . a4 . . . f4 . . . \
               d5 . . . f5 . . . e5 . d5 . . . . . \
               c5 . . . e5 . . . g5 . . . e5 . . . \
               a4 . . . . . . . . . . . - - - - \
               f5 . . . . . e5 . d5 . . . a4 . . . \
               a#4 . . . d5 . . . f5 . . . a#5 . . . \
               g5 . . . f5 . . . d5 . . . a#4 . . . \
               c#5 . . . . . . . e5 . . . a4 . . .",
        lead_wave: Wave::Tri,
        bass: Bass::Long,
        drums: "k . . . h . . . . . k . h . . .",
        arp: 0.05,
        wind: false,
    },
    Tune {
        name: "wind",
        bpm: 60.0,
        chords: &["e3 b3 e4", "c3 g3 e4"],
        lead: "",
        lead_wave: Wave::Tri,
        bass: Bass::Drone,
        drums: "",
        arp: 0.0,
        wind: true,
    },
    Tune {
        name: "summit",
        bpm: 132.0,
        chords: &["c4 e4 g4", "g3 b3 d4", "a3 c4 e4", "f3 a3 c4", "c4 e4 g4", "g3 b3 d4", "f3 a3 c4", "g3 b3 d4"],
        lead: "c5 . e5 . g5 . . . c6 . . . g5 . e5 . \
               d5 . . . g5 . . . b5 . . . a5 . g5 . \
               a5 . . . e5 . . . c5 . e5 . a5 . . . \
               f5 . . . a5 . . . c6 . . . a5 . f5 . \
               e5 . g5 . c6 . . . g5 . e5 . c5 . e5 . \
               d5 . . . b4 . d5 . g5 . . . f5 . d5 . \
               c5 . . . a4 . c5 . f5 . . . a5 . . . \
               g5 . . . . . . . d5 . . . b4 . . .",
        lead_wave: Wave::Square,
        bass: Bass::Drive,
        drums: "k . h . s . h k k . h . s . h h",
        arp: 0.05,
        wind: false,
    },
];

/// PICO-8 music number to tune.
fn tune_for(n: i32) -> Option<usize> {
    match n {
        40 => Some(0),
        0 => Some(1),
        20 => Some(2),
        30 => Some(3),
        10 => Some(4),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tok {
    Note(f32),
    Hold,
    Rest,
}

fn midi(s: &str) -> Option<f32> {
    let b = s.as_bytes();
    let base = match b.first()? {
        b'c' => 0,
        b'd' => 2,
        b'e' => 4,
        b'f' => 5,
        b'g' => 7,
        b'a' => 9,
        b'b' => 11,
        _ => return None,
    };
    let (sharp, rest) = if b.get(1) == Some(&b'#') { (1, &s[2..]) } else { (0, &s[1..]) };
    let oct: i32 = rest.parse().ok()?;
    Some(((oct + 1) * 12 + base + sharp) as f32)
}

fn tokens(s: &str) -> Vec<Tok> {
    s.split_whitespace()
        .map(|t| match t {
            "." => Tok::Hold,
            "-" => Tok::Rest,
            n => Tok::Note(midi(n).unwrap_or_else(|| panic!("bad note {n:?}"))),
        })
        .collect()
}

fn hz(m: f32) -> f32 {
    440.0 * 2f32.powf((m - 69.0) / 12.0)
}

/// One tune, parsed.
struct Score {
    step_len: usize,
    lead: Vec<Tok>,
    chords: Vec<Vec<f32>>,
    drums: Vec<u8>,
}

impl Score {
    fn new(t: &Tune) -> Score {
        Score {
            step_len: (60.0 / t.bpm / 4.0 * SRF) as usize,
            lead: tokens(t.lead),
            chords: t.chords.iter().map(|c| c.split_whitespace().map(|n| midi(n).expect("chord note")).collect()).collect(),
            drums: t.drums.bytes().filter(|b| !b.is_ascii_whitespace()).collect(),
        }
    }

    fn steps(&self) -> usize {
        self.chords.len() * 16
    }
}

#[derive(Clone, Copy, Default)]
struct Voice {
    freq: f32,
    phase: f32,
    t: f32,
    on: bool,
    off_t: f32,
    off_level: f32,
}

impl Voice {
    fn start(&mut self, freq: f32) {
        self.freq = freq;
        self.t = 0.0;
        self.on = true;
    }

    fn stop(&mut self, attack: f32, decay: f32, sustain: f32) {
        if self.on {
            self.off_level = self.level(attack, decay, sustain);
            self.on = false;
            self.off_t = 0.0;
        }
    }

    fn level(&self, attack: f32, decay: f32, sustain: f32) -> f32 {
        (self.t / attack).min(1.0) * (sustain + (1.0 - sustain) * (-self.t / decay).exp())
    }

    /// Advance one sample; envelope with release 60 ms.
    fn env(&mut self, attack: f32, decay: f32, sustain: f32) -> f32 {
        let dt = 1.0 / SRF;
        if self.on {
            self.t += dt;
            self.level(attack, decay, sustain)
        } else {
            self.off_t += dt;
            self.off_level * (-self.off_t / 0.06).exp()
        }
    }

    fn osc(&mut self, wave: Wave, vibrato: f32) -> f32 {
        let f = self.freq * (1.0 + vibrato * (self.t * TAU * 5.5).sin() * (self.t * 4.0 - 0.6).clamp(0.0, 1.0));
        self.phase = (self.phase + f / SRF).fract();
        let p = self.phase;
        match wave {
            Wave::Tri => 1.0 - 4.0 * (p - 0.5).abs(),
            Wave::Square => 0.7 * if p < 0.5 { 1.0 } else { -1.0 },
            Wave::Pulse => 0.7 * if p < 0.25 { 1.0 } else { -1.0 },
        }
    }
}

#[derive(Default)]
pub struct Ctrl {
    /// The tune wanted, if any.
    want: Option<usize>,
    pub muted: bool,
}

/// The endless music source.
pub struct Band {
    ctrl: Arc<Mutex<Ctrl>>,
    scores: Vec<Score>,
    cur: Option<usize>,
    want: Option<usize>,
    muted: bool,
    gain: f32,
    step: usize,
    sample: usize,
    lead: Voice,
    arp: Voice,
    bass: Voice,
    kick_t: f32,
    snare_t: f32,
    hat_t: f32,
    noise: u32,
    wind_lp: f32,
    wind_t: f32,
    poll: usize,
}

impl Band {
    pub fn new(ctrl: Arc<Mutex<Ctrl>>) -> Band {
        Band {
            ctrl,
            scores: TUNES.iter().map(Score::new).collect(),
            cur: None,
            want: None,
            muted: false,
            gain: 0.0,
            step: 0,
            sample: 0,
            lead: Voice::default(),
            arp: Voice::default(),
            bass: Voice::default(),
            kick_t: 9.0,
            snare_t: 9.0,
            hat_t: 9.0,
            noise: 0x2545_f491,
            wind_lp: 0.0,
            wind_t: 0.0,
            poll: 0,
        }
    }

    fn rand(&mut self) -> f32 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        self.noise as f32 / u32::MAX as f32 * 2.0 - 1.0
    }

    /// Start the notes of the current step.
    fn on_step(&mut self, ti: usize) {
        let tune = &TUNES[ti];
        let sc = &self.scores[ti];
        let s = self.step % sc.steps();
        let (bar, beat) = (s / 16, s % 16);
        let chord = sc.chords[bar].clone();
        if !sc.lead.is_empty() {
            match sc.lead[s % sc.lead.len()] {
                Tok::Note(m) => self.lead.start(hz(m)),
                Tok::Rest => self.lead.stop(0.005, 0.12, 0.55),
                Tok::Hold => {}
            }
        }
        if tune.arp > 0.0 && beat % 2 == 0 {
            // up and down the chord, an octave up, in eighths
            let up: Vec<f32> = chord.iter().map(|m| m + 12.0).collect();
            let cycle = [0, 1, 2, 1];
            self.arp.start(hz(up[cycle[(beat / 2) % 4] % up.len()]));
        }
        let root = chord[0] - 12.0;
        match tune.bass {
            Bass::Long if beat == 0 => self.bass.start(hz(root)),
            Bass::Long if beat == 8 => self.bass.start(hz(root + 7.0)),
            Bass::Drive if beat % 2 == 0 => self.bass.start(hz(if beat % 4 == 2 { root + 12.0 } else { root })),
            Bass::Drone if beat == 0 => self.bass.start(hz(root - 12.0)),
            _ => {}
        }
        if !sc.drums.is_empty() {
            match sc.drums[beat % sc.drums.len()] {
                b'k' => self.kick_t = 0.0,
                b's' => self.snare_t = 0.0,
                b'h' => self.hat_t = 0.0,
                _ => {}
            }
        }
    }

    fn sample_of(&mut self, ti: usize) -> f32 {
        if self.sample == 0 {
            self.on_step(ti);
        }
        let tune = &TUNES[ti];
        let dt = 1.0 / SRF;
        let mut out = 0.0;
        // lead
        let e = self.lead.env(0.005, 0.12, 0.55);
        out += self.lead.osc(tune.lead_wave, 0.006) * e * 0.20;
        // accompaniment: plucked
        let e = self.arp.env(0.002, 0.07, 0.0);
        out += self.arp.osc(Wave::Pulse, 0.0) * e * tune.arp;
        // bass
        let e = self.bass.env(0.004, 0.25, if tune.bass == Bass::Drone { 0.9 } else { 0.5 });
        out += self.bass.osc(Wave::Tri, 0.0) * e * if tune.bass == Bass::Drone { 0.18 } else { 0.30 };
        // drums
        if self.kick_t < 0.15 {
            let f = 45.0 + 110.0 * (-self.kick_t * 30.0).exp();
            out += (self.kick_t * f * TAU).sin() * (-self.kick_t * 22.0).exp() * 0.45;
            self.kick_t += dt;
        }
        if self.snare_t < 0.15 {
            let n = self.rand();
            out += n * (-self.snare_t * 28.0).exp() * 0.16;
            self.snare_t += dt;
        }
        if self.hat_t < 0.05 {
            let n = self.rand();
            out += n * (-self.hat_t * 90.0).exp() * 0.06;
            self.hat_t += dt;
        }
        // wind: noise through a lowpass that opens and closes slowly
        if tune.wind {
            self.wind_t += dt;
            let gust = 0.5 + 0.5 * (self.wind_t * 0.23 * TAU).sin() * (self.wind_t * 0.071 * TAU).sin();
            let cutoff = 0.004 + 0.03 * gust;
            let n = self.rand();
            self.wind_lp += (n - self.wind_lp) * cutoff;
            out += self.wind_lp * (0.6 + 1.6 * gust);
        }
        self.sample += 1;
        if self.sample >= self.scores[ti].step_len {
            self.sample = 0;
            self.step += 1;
        }
        out
    }
}

impl Iterator for Band {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.poll == 0 {
            if let Ok(c) = self.ctrl.lock() {
                self.want = c.want;
                self.muted = c.muted;
            }
            self.poll = 512;
        }
        self.poll -= 1;
        // fade out over ~0.5 s before changing tune or stopping
        let target = if self.muted || self.want != self.cur { 0.0 } else { 1.0 };
        let rate = 1.0 / (0.5 * SRF);
        self.gain = if self.gain < target { (self.gain + rate * 4.0).min(target) } else { (self.gain - rate).max(target) };
        if self.gain == 0.0 && self.want != self.cur {
            self.cur = self.want;
            self.step = 0;
            self.sample = 0;
            self.lead = Voice::default();
            self.arp = Voice::default();
            self.bass = Voice::default();
            if let Some(t) = self.cur {
                log::debug!("music now {}", TUNES[t].name);
            }
        }
        let Some(ti) = self.cur else { return Some(0.0) };
        let s = self.sample_of(ti);
        Some((s * self.gain * 0.5).clamp(-1.0, 1.0))
    }
}

impl Source for Band {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> rodio::ChannelCount {
        NonZero::new(1).unwrap()
    }
    fn sample_rate(&self) -> rodio::SampleRate {
        NonZero::new(SR).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// Ask for PICO-8 music pattern `n` (-1 or unknown: silence).
pub fn request(ctrl: &Mutex<Ctrl>, n: i32) {
    if let Ok(mut c) = ctrl.lock() {
        c.want = tune_for(n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tune_is_whole_bars_and_stays_in_range() {
        for (i, t) in TUNES.iter().enumerate() {
            let sc = Score::new(t);
            if !sc.lead.is_empty() {
                assert_eq!(sc.lead.len(), sc.steps(), "{}: lead is {} steps for {} bars", t.name, sc.lead.len(), t.chords.len());
            }
            assert!(sc.drums.is_empty() || sc.drums.len() == 16, "{}: drums", t.name);
            let ctrl = Arc::new(Mutex::new(Ctrl { want: Some(i), muted: false }));
            let mut band = Band::new(ctrl);
            let peak = (0..SR as usize * 6).map(|_| band.next().unwrap().abs()).fold(0.0, f32::max);
            assert!(peak > 0.05 && peak < 1.0, "{}: peak {peak}", t.name);
        }
    }
}
