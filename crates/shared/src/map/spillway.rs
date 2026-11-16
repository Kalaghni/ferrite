//! Spillway: drained waterworks along an east-west channel, below the Great
//! Dam. When the dam's sirens are tested its outlet gates open and water
//! surges out into the channel at C, spreads along it both ways, holds a
//! while and drains (`crate::events::flood`). Deep water sweeps players off
//! their feet: it hurts, and kills those who stay in it.

use super::World;
use crate::events::{smooth, Flood};

pub const MAP_ID: u8 = 4;
/// Full depth of the flood over the middle of the channel's floor.
pub const DEPTH: f32 = 1.9;
/// The water thins out to nothing this far along the channel either way
/// from C, well short of the bases.
pub const FLOOD_X: f32 = 98.0;
/// The channel is never wider than this either side of its centre line.
pub const FLOOD_HALF_Z: f32 = 26.0;
/// Water this shallow round someone's feet does them no harm.
pub const WADE: f32 = 0.3;
/// Health a strong flood takes per second from someone standing in it.
pub const WATER_DPS: f32 = 30.0;

/// The channel's floor along its centre line at `x`, smoothed so the water
/// surface runs evenly over the bumps.
pub fn channel_floor(w: &World, x: f32) -> f32 {
    (w.height(x - 3.0, 0.0) + w.height(x, 0.0) + w.height(x + 3.0, 0.0)) / 3.0
}

/// Height of the water surface over (x, z) in flood `f`, if the water has
/// reached that far along the channel. Says nothing about whether the ground
/// there is under it (see `World::water`).
pub fn water_surface(w: &World, x: f32, z: f32, f: Flood) -> Option<f32> {
    if f.level <= 0.0 || z.abs() > FLOOD_HALF_Z {
        return None;
    }
    let ax = x.abs();
    // Shallow at the leading edge of the surge and towards the ends.
    let depth = DEPTH * f.level * smooth(-2.0, 10.0, f.reach - ax) * (1.0 - smooth(FLOOD_X - 24.0, FLOOD_X, ax));
    (depth > 0.02).then(|| channel_floor(w, x) + depth)
}
