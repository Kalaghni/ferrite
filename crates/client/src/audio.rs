//! All sound is synthesised at startup - the game ships no asset files.

use ferrite_shared::glam::Vec3;
use ferrite_shared::rng::Rng;
use macroquad::audio::{load_sound_from_bytes, play_sound, PlaySoundParams, Sound};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sfx {
    Carbine,
    Scatter,
    Long,
    Hit,
    Head,
    Kill,
    Boom,
    CapUp,
    CapDown,
    Hurt,
    Reload,
    Step,
    Crate,
    Throw,
    Spawn,
    Pistol,
    /// A round passing close by.
    Whizz,
    Slide,
    Count,
}

const SR: f32 = 44100.0;

fn wav(samples: &[f32], gain: (f32, f32)) -> Vec<u8> {
    let n = samples.len() as u32;
    let mut b = Vec::with_capacity(44 + samples.len() * 4);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + n * 4).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&(SR as u32).to_le_bytes());
    b.extend_from_slice(&(SR as u32 * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(n * 4).to_le_bytes());
    for s in samples {
        let l = ((s * gain.0).clamp(-1.0, 1.0) * 30000.0) as i16;
        let r = ((s * gain.1).clamp(-1.0, 1.0) * 30000.0) as i16;
        b.extend_from_slice(&l.to_le_bytes());
        b.extend_from_slice(&r.to_le_bytes());
    }
    b
}

/// Noise burst through a one-pole low-pass plus a pitched thump.
fn gun(dur: f32, decay: f32, lp: f32, thump_hz: f32, thump: f32, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let n = (dur * SR) as usize;
    let mut y = 0.0;
    let mut ph = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            let noise = rng.range(-1.0, 1.0);
            y += (noise - y) * lp;
            let hz = thump_hz * (1.0 - 0.6 * (t / dur));
            ph += hz / SR * std::f32::consts::TAU;
            let crack = if t < 0.004 { noise * 0.8 } else { 0.0 };
            (y * (-t / decay).exp() * 1.4 + ph.sin() * thump * (-t / (decay * 0.8)).exp() + crack).tanh() * 0.9
        })
        .collect()
}

fn tones(notes: &[(f32, f32, f32)], decay: f32, shape: f32) -> Vec<f32> {
    let total = notes.iter().map(|n| n.1 + n.2).fold(0.0, f32::max) + decay * 3.0;
    let n = (total * SR) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            let mut s = 0.0;
            for &(hz, start, _) in notes {
                if t >= start {
                    let lt = t - start;
                    let w = (lt * hz * std::f32::consts::TAU).sin();
                    let w2 = (lt * hz * 2.0 * std::f32::consts::TAU).sin() * shape;
                    s += (w + w2) * (-lt / decay).exp() * (lt * 400.0).min(1.0);
                }
            }
            s * 0.35
        })
        .collect()
}

/// A supersonic crack and falling hiss, for near misses.
fn whizz() -> Vec<f32> {
    let mut rng = Rng::new(21);
    let n = (0.16 * SR) as usize;
    let mut y = 0.0;
    let mut ph = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            let f = t / 0.16;
            y += (rng.range(-1.0, 1.0) - y) * (0.5 - 0.35 * f);
            ph += (3200.0 - 2300.0 * f) / SR * std::f32::consts::TAU;
            let env = (t * 900.0).min(1.0) * (1.0 - f).powi(2);
            (y * 0.9 + ph.sin() * 0.25) * env
        })
        .collect()
}

/// Gravel under a sliding body.
fn scrape() -> Vec<f32> {
    let mut rng = Rng::new(22);
    let n = (0.55 * SR) as usize;
    let mut y = 0.0;
    (0..n)
        .map(|i| {
            let f = i as f32 / n as f32;
            y += (rng.range(-1.0, 1.0) - y) * 0.22;
            y * (f * 30.0).min(1.0) * (1.0 - f).powf(1.5) * 0.9
        })
        .collect()
}

/// Soft filtered noise with a gentle attack: no click at the start.
fn noise(dur: f32, decay: f32, lp: f32, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let n = (dur * SR) as usize;
    let mut y = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            y += (rng.range(-1.0, 1.0) - y) * lp;
            y * (t * 160.0).min(1.0) * (-t / decay).exp() * 1.3
        })
        .collect()
}

/// A boot on dirt: a dull thud with a little grit, nothing sharp.
fn footstep() -> Vec<f32> {
    let mut rng = Rng::new(6);
    let n = (0.16 * SR) as usize;
    let (mut y, mut y2) = (0.0f32, 0.0f32);
    let mut ph = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            let w = rng.range(-1.0, 1.0);
            y += (w - y) * 0.035;
            y2 += (y - y2) * 0.2;
            ph += (95.0 - 260.0 * t).max(45.0) / SR * std::f32::consts::TAU;
            let attack = (t * 120.0).min(1.0);
            (y2 * 3.2 * (-t / 0.035).exp() + ph.sin() * 0.5 * (-t / 0.03).exp()) * attack
        })
        .collect()
}

pub fn shot_sfx(wid: u8) -> Sfx {
    match ferrite_shared::sim::weapon_def(wid).arch {
        0 => Sfx::Carbine,
        1 => Sfx::Scatter,
        2 => Sfx::Long,
        _ => Sfx::Pistol,
    }
}

/// Each sound is loaded three times - centred, left-heavy and right-heavy -
/// because the mixer has no pan control. Picking one by bearing is enough
/// to tell which side a shot came from.
const PANS: [(f32, f32); 3] = [(0.85, 0.85), (1.0, 0.3), (0.3, 1.0)];

pub struct Audio {
    sounds: Vec<[Option<Sound>; 3]>,
    /// The listener's right-hand direction, for stereo placement.
    pub right: Vec3,
    budget: i32,
    pub volume: f32,
    pub ok: bool,
}

impl Audio {
    pub async fn new(volume: f32) -> Audio {
        let mut defs: Vec<(Sfx, Vec<f32>)> = vec![
            (Sfx::Carbine, gun(0.28, 0.045, 0.55, 150.0, 0.7, 1)),
            (Sfx::Scatter, gun(0.5, 0.09, 0.3, 90.0, 0.9, 2)),
            (Sfx::Long, gun(0.75, 0.12, 0.7, 120.0, 0.8, 3)),
            (Sfx::Pistol, gun(0.2, 0.03, 0.75, 230.0, 0.5, 9)),
            (Sfx::Hit, tones(&[(1250.0, 0.0, 0.02)], 0.018, 0.3)),
            (Sfx::Head, tones(&[(1900.0, 0.0, 0.02), (2500.0, 0.03, 0.02)], 0.03, 0.2)),
            (Sfx::Kill, tones(&[(880.0, 0.0, 0.05), (1320.0, 0.07, 0.1)], 0.09, 0.25)),
            (Sfx::Boom, gun(1.3, 0.3, 0.06, 55.0, 1.0, 4)),
            (Sfx::CapUp, tones(&[(523.0, 0.0, 0.1), (659.0, 0.11, 0.1), (784.0, 0.22, 0.1), (1047.0, 0.33, 0.2)], 0.16, 0.3)),
            (Sfx::CapDown, tones(&[(440.0, 0.0, 0.14), (349.0, 0.16, 0.14), (262.0, 0.32, 0.25)], 0.2, 0.5)),
            (Sfx::Hurt, gun(0.2, 0.05, 0.12, 85.0, 1.0, 5)),
            (Sfx::Step, footstep()),
            (Sfx::Crate, noise(0.45, 0.09, 0.35, 7)),
            (Sfx::Throw, noise(0.22, 0.08, 0.04, 8)),
            (Sfx::Spawn, tones(&[(330.0, 0.0, 0.1), (495.0, 0.08, 0.2)], 0.14, 0.2)),
            (Sfx::Whizz, whizz()),
            (Sfx::Slide, scrape()),
        ];
        // Reload: two metallic clicks.
        let mut reload = vec![0.0; (0.5 * SR) as usize];
        for (at, seed) in [(0.02, 11u64), (0.36, 12)] {
            let click = gun(0.06, 0.012, 0.8, 900.0, 0.4, seed);
            let o = (at * SR) as usize;
            for (i, s) in click.iter().enumerate() {
                reload[o + i] += s * 0.6;
            }
        }
        defs.push((Sfx::Reload, reload));
        let mut sounds: Vec<[Option<Sound>; 3]> = (0..Sfx::Count as usize).map(|_| [None, None, None]).collect();
        let mut ok = true;
        // Only sounds that come from a place in the world need the panned copies.
        let placed = [Sfx::Carbine, Sfx::Scatter, Sfx::Long, Sfx::Pistol, Sfx::Boom, Sfx::Step, Sfx::Crate, Sfx::Throw, Sfx::Whizz];
        for (id, samples) in defs {
            for (k, gain) in PANS.iter().enumerate() {
                if k > 0 && !placed.contains(&id) {
                    continue;
                }
                match load_sound_from_bytes(&wav(&samples, *gain)).await {
                    Ok(s) => sounds[id as usize][k] = Some(s),
                    Err(_) => ok = false,
                }
            }
        }
        Audio { sounds, right: Vec3::X, budget: 0, volume, ok }
    }

    pub fn begin_frame(&mut self) {
        self.budget = 7;
    }

    pub fn play(&mut self, s: Sfx, vol: f32) {
        self.play_pan(s, vol, 0);
    }

    fn play_pan(&mut self, s: Sfx, vol: f32, pan: usize) {
        let v = vol * self.volume;
        if v < 0.012 || self.budget <= 0 {
            return;
        }
        let set = &self.sounds[s as usize];
        if let Some(snd) = set[pan].as_ref().or(set[0].as_ref()) {
            self.budget -= 1;
            play_sound(snd, PlaySoundParams { looped: false, volume: v.min(1.0) });
        }
    }

    /// Distance-attenuated playback relative to the listener.
    pub fn play_at(&mut self, s: Sfx, pos: Vec3, listener: Vec3, vol: f32, reach: f32) {
        let to = pos - listener;
        let d = to.length();
        let side = if d > 0.5 { to.dot(self.right) / d } else { 0.0 };
        let pan = if side < -0.28 {
            1
        } else if side > 0.28 {
            2
        } else {
            0
        };
        self.play_pan(s, vol / (1.0 + (d / reach) * (d / reach)), pan);
    }
}
