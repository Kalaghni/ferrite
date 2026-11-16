//! Sound effects are synthesised at startup. Launcher, lobby and map music
//! are embedded and decoded on background workers.

use ferrite_shared::glam::Vec3;
use ferrite_shared::rng::Rng;
use macroquad::audio::{load_sound_from_bytes, play_sound, set_sound_volume, PlaySoundParams, Sound};

/// Songs that rotate on the launcher (and in a match, if that is switched
/// on): the title shown as "now playing", and the file.
///
/// To add one: put the .ogg (44.1 kHz stereo) in `assets/` and add a line.
/// Players can add their own without rebuilding, by putting .ogg or .wav
/// files in a `music` folder next to the executable.
static SONGS: &[(&str, &[u8])] = &[
    ("Liquid Plasma - breadKernel", include_bytes!("../assets/liquid-plasma.ogg")),
    ("Submariner - breadKernel", include_bytes!("../assets/submariner.ogg")),
    ("Barbed Wire - breadKernel", include_bytes!("../assets/barbed-wire.ogg")),
];

/// The lobby has its own track.
static LOBBY_SONG: (&str, &[u8]) = ("Foundrite Lobby - breadKernel", include_bytes!("../assets/foundrite-lobby.ogg"));

/// User-supplied score, reserved for the Slag Cathedral battlefield.
static CATHEDRAL_SONG: (&str, &[u8]) = ("Slag Cathedral", include_bytes!("../assets/slag-cathedral.ogg"));

#[derive(Clone)]
enum Source {
    Built(&'static [u8]),
    File(std::path::PathBuf),
}

struct Song {
    title: String,
    src: Source,
}

/// Decoded on a worker thread: a token for `load_sound_from_bytes` and the
/// length in seconds, or `None` if the file could not be read.
type Decoded = Option<(Vec<u8>, f32)>;

fn decode_in_background(src: &Source) -> std::sync::mpsc::Receiver<Decoded> {
    let (tx, rx) = std::sync::mpsc::channel();
    let src = src.clone();
    std::thread::spawn(move || {
        let pcm = match &src {
            Source::Built(b) => quad_snd::decode(b),
            Source::File(p) => std::fs::read(p).ok().and_then(|b| quad_snd::decode(&b)),
        };
        let _ = tx.send(pcm.map(|pcm| {
            let secs = pcm.len() as f32 / 2.0 / 44100.0;
            (quad_snd::stash(pcm), secs)
        }));
    });
    rx
}

#[derive(Default)]
struct Deck {
    sound: Option<Sound>,
    loading: Option<std::sync::mpsc::Receiver<Decoded>>,
    /// Length of the loaded track, and how much of it is left to play.
    secs: f32,
    left: f32,
    level: f32,
    sent: f32,
    playing: bool,
}

impl Deck {
    /// Take a finished decode. `Some(false)` means the file was unreadable.
    async fn poll(&mut self) -> Option<bool> {
        let got = match self.loading.as_ref()?.try_recv() {
            Ok(d) => d,
            Err(std::sync::mpsc::TryRecvError::Empty) => return None,
            Err(_) => None,
        };
        self.loading = None;
        let (token, secs) = match got {
            Some(x) => x,
            None => return Some(false),
        };
        match load_sound_from_bytes(&token).await {
            Ok(s) => {
                self.sound = Some(s);
                self.secs = secs;
                self.playing = false;
                Some(true)
            }
            Err(_) => Some(false),
        }
    }

    /// Fade toward audible or silent, start the track when first wanted,
    /// and return true on the frame it finishes (never, when looped).
    fn run(&mut self, want: bool, gain: f32, looped: bool, dt: f32) -> bool {
        let target = if want { 1.0 } else { 0.0 };
        let step = dt * if target > self.level { 0.9 } else { 0.45 };
        self.level += (target - self.level).clamp(-step, step);
        let Some(s) = &self.sound else { return false };
        let v = gain * self.level * self.level;
        if !self.playing {
            if want {
                self.playing = true;
                self.left = self.secs;
                self.sent = v;
                play_sound(s, PlaySoundParams { looped, volume: v });
            }
            return false;
        }
        if (v - self.sent).abs() > 0.002 || (v == 0.0 && self.sent != 0.0) {
            self.sent = v;
            set_sound_volume(s, v);
        }
        if !looped {
            self.left -= dt;
            if self.left <= 0.0 {
                self.playing = false;
                return true;
            }
        }
        false
    }
}

/// Where the player is, as far as the music is concerned.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    Launcher,
    Lobby(u8),
    Match(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MusicTarget {
    Silence,
    Rotation,
    Lobby,
    Cathedral,
}

impl Scene {
    fn target(self, on: bool, in_match: bool) -> MusicTarget {
        if !on || matches!(self, Self::Match(_)) && !in_match {
            return MusicTarget::Silence;
        }
        let cathedral = ferrite_shared::map::slag_cathedral::MAP_ID;
        match self {
            Self::Launcher => MusicTarget::Rotation,
            Self::Lobby(map) | Self::Match(map) if map == cathedral => MusicTarget::Cathedral,
            Self::Lobby(_) => MusicTarget::Lobby,
            Self::Match(_) => MusicTarget::Rotation,
        }
    }
}

/// Music: a rotation of songs on the launcher (and in a match, if the
/// player wants it there) and a track of its own for the lobby. Songs are
/// decoded in the background one at a time, as their turn comes.
pub struct Music {
    enabled: bool,
    /// The player's switches and volume (0..1) from the settings.
    pub on: bool,
    pub in_match: bool,
    pub vol: f32,
    songs: Vec<Song>,
    cur: usize,
    deck: Deck,
    lobby: Deck,
    cathedral: Deck,
    target: MusicTarget,
}

impl Music {
    pub fn silent() -> Music {
        Music {
            enabled: false,
            on: false,
            in_match: false,
            vol: 0.0,
            songs: Vec::new(),
            cur: 0,
            deck: Deck::default(),
            lobby: Deck::default(),
            cathedral: Deck::default(),
            target: MusicTarget::Silence,
        }
    }

    pub fn new(vol: f32) -> Music {
        let mut songs: Vec<Song> = SONGS.iter().map(|(t, b)| Song { title: t.to_string(), src: Source::Built(b) }).collect();
        // The player's own songs: a `music` folder next to the executable.
        let dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("music")));
        if let Some(Ok(rd)) = dir.map(std::fs::read_dir) {
            let mut files: Vec<_> = rd.flatten().map(|e| e.path()).collect();
            files.sort();
            for p in files {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                if ext == "ogg" || ext == "wav" {
                    let title = p.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown").to_string();
                    songs.push(Song { title, src: Source::File(p) });
                }
            }
        }
        let lobby = Deck { loading: Some(decode_in_background(&Source::Built(LOBBY_SONG.1))), ..Default::default() };
        Music {
            enabled: true,
            on: true,
            in_match: false,
            vol,
            songs,
            cur: 0,
            deck: Deck::default(),
            lobby,
            cathedral: Deck::default(),
            target: MusicTarget::Rotation,
        }
    }

    pub fn available(&self) -> bool {
        self.enabled
    }

    /// More than one song in the rotation: there is something to skip to.
    pub fn can_skip(&self) -> bool {
        self.enabled && self.target == MusicTarget::Rotation && self.songs.len() > 1
    }

    /// Move on to the next song in the rotation.
    pub fn skip(&mut self) {
        if !self.can_skip() || self.deck.loading.is_some() {
            return;
        }
        if let Some(s) = self.deck.sound.take() {
            macroquad::audio::stop_sound(&s);
        }
        self.deck.playing = false;
        self.cur = (self.cur + 1) % self.songs.len();
    }

    /// Title of the track that can be heard right now.
    pub fn now_playing(&self) -> Option<&str> {
        if !self.enabled || !self.on || self.vol <= 0.0 {
            None
        } else if self.cathedral.playing && self.cathedral.level > 0.05 {
            Some(CATHEDRAL_SONG.0)
        } else if self.deck.playing && self.deck.level > 0.05 {
            self.songs.get(self.cur).map(|s| s.title.as_str())
        } else if self.lobby.playing && self.lobby.level > 0.05 {
            Some(LOBBY_SONG.0)
        } else {
            None
        }
    }

    /// Call every frame.
    pub async fn update(&mut self, scene: Scene, dt: f32) {
        if !self.enabled {
            return;
        }
        self.target = scene.target(self.on, self.in_match);
        let want_songs = self.target == MusicTarget::Rotation && !self.songs.is_empty();
        let want_lobby = self.target == MusicTarget::Lobby;
        let want_cathedral = self.target == MusicTarget::Cathedral;
        // Under a firefight the music sits further back.
        let gain = self.vol * 0.3 * if matches!(scene, Scene::Match(_)) { 0.6 } else { 1.0 };

        self.lobby.poll().await;
        self.lobby.run(want_lobby, gain, true, dt);
        if want_cathedral && self.cathedral.sound.is_none() && self.cathedral.loading.is_none() {
            self.cathedral.loading = Some(decode_in_background(&Source::Built(CATHEDRAL_SONG.1)));
        }
        self.cathedral.poll().await;
        self.cathedral.run(want_cathedral, gain, true, dt);

        if self.deck.poll().await == Some(false) {
            // Unreadable file: take it out of the rotation.
            if self.cur < self.songs.len() {
                self.songs.remove(self.cur);
            }
            if self.cur >= self.songs.len() {
                self.cur = 0;
            }
        }
        if want_songs && self.deck.sound.is_none() && self.deck.loading.is_none() {
            if let Some(song) = self.songs.get(self.cur) {
                self.deck.loading = Some(decode_in_background(&song.src));
            }
        }
        let ended = self.deck.run(want_songs, gain, false, dt);
        if ended && self.songs.len() > 1 {
            // Free this one and let the next be decoded.
            self.deck.sound = None;
            self.cur = (self.cur + 1) % self.songs.len();
        }
    }
}

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
    /// Wind knocking a loose aircraft skin panel against its frame.
    MetalRattle,
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

/// Light, irregular impacts with inharmonic sheet-metal resonances.
fn metal_rattle() -> Vec<f32> {
    let mut rng = Rng::new(0x00FA_11EA);
    let mut filtered = 0.0;
    (0..(1.4 * SR) as usize)
        .map(|i| {
            let t = i as f32 / SR;
            filtered += (rng.range(-1.0, 1.0) - filtered) * 0.65;
            let mut sample = 0.0;
            for (at, gain) in [(0.0, 0.8), (0.11, 0.55), (0.29, 0.7), (0.52, 0.35), (0.81, 0.2)] {
                let age = t - at;
                if age < 0.0 {
                    continue;
                }
                let attack = (age * 1800.0).min(1.0);
                let ring = (age * 431.0 * std::f32::consts::TAU).sin()
                    + (age * 937.0 * std::f32::consts::TAU).sin() * 0.45
                    + (age * 1693.0 * std::f32::consts::TAU).sin() * 0.23;
                sample += gain * attack * (ring * (-age / 0.085).exp() * 0.2 + filtered * (-age / 0.018).exp() * 0.35);
            }
            sample.tanh() * 0.6
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
            (Sfx::MetalRattle, metal_rattle()),
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
        let placed = [
            Sfx::Carbine,
            Sfx::Scatter,
            Sfx::Long,
            Sfx::Pistol,
            Sfx::Boom,
            Sfx::Step,
            Sfx::Crate,
            Sfx::Throw,
            Sfx::Whizz,
            Sfx::MetalRattle,
        ];
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

#[cfg(test)]
mod music_tests {
    use super::*;

    #[test]
    fn aircraft_rattle_has_finite_audible_taps_and_a_quiet_tail() {
        let pcm = metal_rattle();
        assert_eq!(pcm.len(), (SR * 1.4) as usize);
        assert!(pcm.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        assert!(pcm.iter().take(4410).any(|s| s.abs() > 0.05));
        assert!(pcm.iter().rev().take(4410).all(|s| s.abs() < 0.002));
    }

    #[test]
    fn cathedral_score_is_map_specific_and_honours_music_switches() {
        use ferrite_shared::map::slag_cathedral::MAP_ID;
        assert_eq!(Scene::Lobby(MAP_ID).target(true, false), MusicTarget::Cathedral);
        assert_eq!(Scene::Match(MAP_ID).target(true, true), MusicTarget::Cathedral);
        assert_eq!(Scene::Match(MAP_ID).target(true, false), MusicTarget::Silence);
        for map in 0..MAP_ID {
            assert_eq!(Scene::Lobby(map).target(true, true), MusicTarget::Lobby);
            assert_eq!(Scene::Match(map).target(true, true), MusicTarget::Rotation);
        }
        assert_eq!(Scene::Launcher.target(true, true), MusicTarget::Rotation);
        for scene in [Scene::Launcher, Scene::Lobby(MAP_ID), Scene::Match(MAP_ID)] {
            assert_eq!(scene.target(false, true), MusicTarget::Silence);
        }
    }

    #[test]
    fn embedded_cathedral_score_decodes_at_the_games_sample_rate() {
        let pcm = quad_snd::decode(CATHEDRAL_SONG.1).expect("map score must decode");
        assert_eq!(pcm.len() % 2, 0, "stereo PCM must have complete frames");
        let seconds = pcm.len() as f32 / (44100.0 * 2.0);
        assert!((seconds - 154.2857).abs() < 0.1, "track duration changed: {seconds}");
        assert!(pcm.iter().all(|sample| sample.is_finite()));
        assert!(pcm.iter().any(|sample| sample.abs() > 0.001), "map score is silent");
    }
}
