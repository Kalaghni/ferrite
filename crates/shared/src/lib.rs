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
