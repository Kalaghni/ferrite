//! Loading and playing sounds.

#![allow(warnings)]

mod error;

pub use error::Error;

#[cfg(target_os = "android")]
#[path = "opensles_snd.rs"]
mod snd;

#[cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"))]
#[path = "alsa_snd.rs"]
mod snd;

#[cfg(any(target_os = "macos", target_os = "ios"))]
#[path = "coreaudio_snd.rs"]
mod snd;

#[cfg(target_os = "windows")]
#[path = "wasapi_snd.rs"]
mod snd;

#[cfg(target_arch = "wasm32")]
#[path = "web_snd.rs"]
mod snd;

#[cfg(not(target_arch = "wasm32"))]
mod mixer;

pub use snd::{AudioContext, Playback, Sound};

#[cfg(not(target_arch = "wasm32"))]
pub use mixer::{decode, stash};

// FERRITE: choosing the output device (Windows only so far; elsewhere the
// system default is always used and the list is empty).
#[cfg(target_os = "windows")]
pub use snd::{output_devices, set_output_device};

/// Active output devices as (id, name).
#[cfg(not(target_os = "windows"))]
pub fn output_devices() -> Vec<(String, String)> {
    Vec::new()
}

/// Play through the device with this id; `None` follows the system default.
#[cfg(not(target_os = "windows"))]
pub fn set_output_device(_id: Option<String>) {}

pub struct PlaySoundParams {
    pub looped: bool,
    pub volume: f32,
}

impl Default for PlaySoundParams {
    fn default() -> PlaySoundParams {
        PlaySoundParams {
            looped: false,
            volume: 1.,
        }
    }
}
