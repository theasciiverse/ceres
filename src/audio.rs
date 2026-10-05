//! Sound, all synthesised here; Ceres ships no audio files.
//!
//! Each effect is written like a PICO-8 sfx: a row of steps, each a pitch,
//! a waveform and a volume, played at a fixed speed. They are our own
//! sounds, made for the moments the game asks for by PICO-8 sfx number.
//! Music is not written yet; music requests are logged and ignored.
//!
//! Without an audio device, or with `--mute`, every call does nothing.

use rodio::buffer::SamplesBuffer;
use rodio::{MixerDeviceSink, Source};
use std::collections::HashMap;
use std::num::NonZero;

const SR: u32 = 44_100;
const SRF: f32 = SR as f32;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Wave {
    Tri,
    Square,
    Pulse,
    Saw,
    Noise,
}
use Wave::*;

/// One step: MIDI pitch (fractions slide), waveform, volume 0-1.
type Step = (f32, Wave, f32);

pub struct Audio {
    sink: Option<MixerDeviceSink>,
    bank: HashMap<u8, SamplesBuffer>,
    pub muted: bool,
}

impl Audio {
    pub fn new(muted: bool) -> Audio {
        let sink = match rodio::DeviceSinkBuilder::open_default_sink() {
            Ok(mut s) => {
                s.log_on_drop(false);
                Some(s)
            }
            Err(e) => {
                log::warn!("no audio: {e}");
                None
            }
        };
        let bank = if sink.is_some() { SFX.iter().map(|&n| (n, buffer(render(n)))).collect() } else { HashMap::new() };
        Audio { sink, bank, muted }
    }

    pub fn available(&self) -> bool {
        self.sink.is_some()
    }

    pub fn sfx(&mut self, n: u8) {
        if self.muted {
            return;
        }
        let (Some(sink), Some(b)) = (&self.sink, self.bank.get(&n)) else { return };
        sink.mixer().add(b.clone().amplify(0.35));
    }

    pub fn music(&mut self, n: i32) {
        log::debug!("music {n} (not written yet)");
    }
}

fn buffer(samples: Vec<f32>) -> SamplesBuffer {
    SamplesBuffer::new(NonZero::new(1).unwrap(), NonZero::new(SR).unwrap(), samples)
}

const SFX: [u8; 21] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 13, 14, 15, 16, 23, 35, 37, 38, 51, 54, 55];

/// Steps from `a` to `b` in `n` even pitch moves, volume from `v0` to `v1`.
fn slide(a: f32, b: f32, n: usize, w: Wave, v0: f32, v1: f32) -> Vec<Step> {
    (0..n)
        .map(|i| {
            let t = if n > 1 { i as f32 / (n - 1) as f32 } else { 0.0 };
            (a + (b - a) * t, w, v0 + (v1 - v0) * t)
        })
        .collect()
}

fn notes(ps: &[f32], w: Wave, v: f32, each: usize) -> Vec<Step> {
    ps.iter().flat_map(|&p| std::iter::repeat_n((p, w, v), each)).collect()
}

fn fade(mut s: Vec<Step>, from: f32) -> Vec<Step> {
    let n = s.len() as f32;
    for (i, st) in s.iter_mut().enumerate() {
        let t = i as f32 / n;
        if t > from {
            st.2 *= 1.0 - (t - from) / (1.0 - from);
        }
    }
    s
}

/// The effect for PICO-8 sfx number `n`: step length in seconds, and one
/// or more layers of steps played together.
fn score(n: u8) -> (f32, Vec<Vec<Step>>) {
    match n {
        // death
        0 => (0.012, vec![slide(76.0, 40.0, 14, Square, 0.7, 0.3), slide(50.0, 30.0, 14, Noise, 0.6, 0.0)]),
        // jump
        1 => (0.011, vec![slide(62.0, 74.0, 7, Square, 0.55, 0.15)]),
        // wall jump
        2 => (0.011, vec![[vec![(40.0, Noise, 0.4)], slide(64.0, 78.0, 7, Pulse, 0.55, 0.15)].concat()]),
        // dash
        3 => (0.012, vec![slide(60.0, 80.0, 6, Noise, 0.8, 0.5), slide(79.0, 55.0, 6, Saw, 0.4, 0.0)]),
        // spawn: shooting up out of the ground
        4 => (0.014, vec![slide(48.0, 86.0, 18, Tri, 0.6, 0.2)]),
        // landing from the spawn jump
        5 => (0.012, vec![slide(40.0, 30.0, 4, Noise, 0.7, 0.2), slide(45.0, 36.0, 4, Tri, 0.7, 0.0)]),
        // balloon popped
        6 => (0.03, vec![[notes(&[79.0, 86.0], Square, 0.5, 1), slide(91.0, 91.0, 4, Square, 0.4, 0.0)].concat()]),
        // something comes back (balloon, crumbled floor)
        7 => (0.03, vec![fade(notes(&[72.0, 79.0], Tri, 0.35, 2), 0.5)]),
        // spring
        8 => (0.012, vec![[slide(55.0, 82.0, 9, Square, 0.6, 0.4), slide(82.0, 84.0, 6, Tri, 0.4, 0.0)].concat()]),
        // no dash left
        9 => (0.02, vec![notes(&[41.0, 39.0], Pulse, 0.25, 2)]),
        // strawberry
        13 => (0.035, vec![fade([notes(&[72.0, 76.0, 79.0, 84.0], Square, 0.45, 1), notes(&[88.0], Square, 0.45, 5)].concat(), 0.5)]),
        // flying strawberry takes off
        14 => (0.025, vec![fade(notes(&[84.0, 88.0, 84.0, 88.0, 86.0, 90.0, 86.0, 90.0], Pulse, 0.35, 1), 0.5)]),
        // floor starts to crumble
        15 => (0.02, vec![[notes(&[38.0], Noise, 0.6, 1), notes(&[36.0], Noise, 0.1, 1), notes(&[40.0], Noise, 0.5, 1), notes(&[36.0], Noise, 0.1, 1), notes(&[38.0], Noise, 0.35, 2)].concat()]),
        // wall smashed, chest opens
        16 => (0.015, vec![slide(70.0, 40.0, 10, Noise, 0.9, 0.3), slide(64.0, 48.0, 10, Square, 0.4, 0.0)]),
        // key
        23 => (0.03, vec![fade(notes(&[84.0, 91.0, 96.0, 91.0, 96.0, 103.0], Tri, 0.4, 1), 0.6)]),
        // message letter
        35 => (0.01, vec![vec![(86.0, Tri, 0.25), (86.0, Tri, 0.1)]]),
        // the big chest opens: a long swell
        37 => (0.03, vec![fade(slide(48.0, 96.0, 64, Tri, 0.2, 0.6), 0.8)]),
        // start game
        38 => (0.04, vec![fade([notes(&[72.0, 76.0, 79.0], Square, 0.5, 1), slide(84.0, 84.0, 12, Square, 0.5, 0.5)].concat(), 0.4)]),
        // the orb: two dashes now
        51 => (0.03, vec![fade(notes(&[60.0, 67.0, 72.0, 76.0, 79.0, 84.0, 88.0, 91.0, 96.0], Square, 0.45, 1), 0.7)]),
        // dash restored on landing
        54 => (0.015, vec![fade(notes(&[88.0, 93.0], Tri, 0.35, 2), 0.5)]),
        // summit flag
        55 => (0.06, vec![fade([notes(&[72.0, 76.0, 79.0, 84.0], Square, 0.45, 1), notes(&[84.0], Square, 0.45, 8)].concat(), 0.4)]),
        _ => (0.01, Vec::new()),
    }
}

fn render(n: u8) -> Vec<f32> {
    let (len, layers) = score(n);
    let per = (len * SRF) as usize;
    let mut out = vec![0.0f32; per * layers.iter().map(Vec::len).max().unwrap_or(0)];
    for steps in &layers {
        let (mut ph, mut noise, mut lp, mut hold, mut prev_vol) = (0.0f32, 0x1234_5678u32, 0.0f32, 0.0f32, 0.0f32);
        for (k, &(pitch, w, vol)) in steps.iter().enumerate() {
            let next = steps.get(k + 1).map(|s| s.0).unwrap_or(pitch);
            for s in 0..per {
                let t = s as f32 / per as f32;
                let p = pitch + (next - pitch) * t * 0.5;
                let f = 440.0 * 2f32.powf((p - 69.0) / 12.0);
                ph = (ph + f / SRF).fract();
                // ramp volume changes over 64 samples against clicks
                let v = prev_vol + (vol - prev_vol) * (s as f32 / 64.0).min(1.0);
                let x = match w {
                    Tri => 1.0 - 4.0 * (ph - 0.5).abs(),
                    Square => 0.6 * if ph < 0.5 { 1.0 } else { -1.0 },
                    Pulse => 0.6 * if ph < 0.25 { 1.0 } else { -1.0 },
                    Saw => (ph * 2.0 - 1.0) * 0.6,
                    Noise => {
                        // sample-and-hold noise at the pitch rate, softened
                        if ph < f / SRF {
                            noise ^= noise << 13;
                            noise ^= noise >> 17;
                            noise ^= noise << 5;
                            hold = (noise as f32 / u32::MAX as f32) * 2.0 - 1.0;
                        }
                        lp += (hold - lp) * 0.5;
                        lp
                    }
                };
                out[k * per + s] += x * v;
            }
            prev_vol = vol;
        }
    }
    // short fade out against clicks
    let n = out.len();
    for (i, s) in out.iter_mut().enumerate().skip(n.saturating_sub(200)) {
        *s *= (n - i) as f32 / 200.0;
    }
    out
}
