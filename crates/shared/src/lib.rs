//! Shared deterministic simulation, map generation and wire protocol.
//! Both the authoritative server and the predicting client link this crate,
//! so a command stepped on either side produces the same result.

pub mod map;
pub mod proto;
pub mod rng;
pub mod sim;

pub use glam;
pub use glam::{vec2, vec3, Mat4, Vec2, Vec3};

pub const TICK_HZ: u32 = 60;
pub const DT: f32 = 1.0 / 60.0;
pub const NUM_POINTS: usize = 5;
pub const POINT_RADIUS: f32 = 9.0;
pub const NEUTRAL: u8 = 2;
pub const TEAM_NAMES: [&str; 2] = ["EMBER", "COBALT"];
pub const POINT_NAMES: [&str; 5] = ["A", "B", "C", "D", "E"];

/// Game modes. Ids travel over the wire: only ever append.
pub mod mode {
    pub const CONQUEST: u8 = 0;
    pub const TDM: u8 = 1;
    pub const CTF: u8 = 2;
    pub const KOTH: u8 = 3;
}

pub struct ModeDef {
    pub name: &'static str,
    /// What `--mode` calls it.
    pub key: &'static str,
    pub pace: &'static str,
    pub blurb: &'static str,
    /// Shown as the round starts.
    pub brief: &'static str,
}

pub static MODES: [ModeDef; 4] = [
    ModeDef { name: "CONQUEST", key: "conquest", pace: "LONG", blurb: "Five points taken in order along one front line. Tickets decide it.", brief: "TAKE THE LINE" },
    ModeDef { name: "TEAM DEATHMATCH", key: "tdm", pace: "FAST", blurb: "No objectives. Forward spawns, quick redeploys, first side to the kill target.", brief: "FIRST TO THE KILL TARGET" },
    ModeDef { name: "CAPTURE THE FLAG", key: "ctf", pace: "SLOW", blurb: "Take their flag from B or D and carry it back to your own. Three captures win.", brief: "BRING THEIR FLAG HOME" },
    ModeDef { name: "STRONGHOLD", key: "stronghold", pace: "MEDIUM", blurb: "One point in the middle. Every second you hold it scores.", brief: "HOLD THE CENTRE" },
];

pub fn mode_def(m: u8) -> &'static ModeDef {
    &MODES[m as usize % MODES.len()]
}

pub fn mode_by_name(s: &str) -> Option<u8> {
    let s = s.trim().to_ascii_lowercase();
    if let Ok(n) = s.parse::<usize>() {
        return (n < MODES.len()).then_some(n as u8);
    }
    let alias = match s.as_str() {
        "koth" | "hill" => "stronghold",
        "deathmatch" | "team-deathmatch" => "tdm",
        "flag" | "flags" => "ctf",
        other => other,
    };
    MODES.iter().position(|m| m.key == alias || m.name.to_ascii_lowercase() == alias).map(|i| i as u8)
}

/// Which team's flag stands at which objective in capture the flag.
pub const FLAG_POINT: [usize; 2] = [1, NUM_POINTS - 2];

/// Front-line rule: a team may only push a point that touches territory it
/// already holds (its base counts as territory behind its first point).
pub fn eligible(owners: &[u8; NUM_POINTS], team: u8, k: usize) -> bool {
    if team == 0 {
        k == 0 || owners[k - 1] == 0
    } else {
        k == NUM_POINTS - 1 || owners[k + 1] == 1
    }
}

pub fn wrap_angle(a: f32) -> f32 {
    let mut a = a % std::f32::consts::TAU;
    if a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    } else if a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}
