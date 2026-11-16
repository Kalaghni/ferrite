//! Spillway's gate deck at C and the flood that pours into the channel
//! after a siren test at the Great Dam.
//!
//! The deck, its piers, bridges, stairs, rails, cover, the lifted floodgate
//! in its frame and the two outfalls are collision from
//! `ferrite_shared::map::spillway`; this module draws them. Everything that
//! moves - the sluice gates lifting in the outfalls, water gushing out, the
//! surge running along the channel and draining away, the wet floor it
//! leaves - is a pure function of the world seed and the world clock
//! (`ferrite_shared::events::flood`), the same one the server drowns players
//! by, so every client sees the water where it hurts.

use super::hash01;
use super::slag_cathedral::art::{soft, stencil, wall};
use super::slag_cathedral::{beam, girder};
use crate::gfx::{shade, Col, Mesh};
use ferrite_shared::events::{self, Flood, GATES_CLOSE, GATES_OPEN, SIREN_PERIOD, WATER_DRAINED};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::spillway::{self as layout, Layout};
use ferrite_shared::map::{Block, BlockKind, World};
use std::f32::consts::PI;

pub const ID: u8 = layout::MAP_ID;

const CONCRETE: Col = [0.62, 0.61, 0.57, 0.0];
const CONCRETE_DARK: Col = [0.47, 0.47, 0.45, 0.0];
/// Concrete the floods keep wet, below the high-water mark.
const WET: Col = [0.38, 0.39, 0.36, 0.0];
const STEEL: Col = [0.27, 0.30, 0.32, 0.0];
const GALV: Col = [0.55, 0.57, 0.56, 0.0];
const YELLOW: Col = [0.83, 0.64, 0.18, 0.0];
const RUST: Col = [0.44, 0.27, 0.17, 0.0];
const GREEN: Col = [0.30, 0.38, 0.33, 0.0];
const DARK: Col = [0.06, 0.07, 0.08, 0.0];
const SAND: Col = [0.62, 0.55, 0.39, 0.0];

/// Which of the world's blocks this module draws (the structure at C).
/// Empty on other maps.
pub fn owned(w: &World) -> Vec<bool> {
    if w.map != ID {
        return Vec::new();
    }
    let parts = layout::structure(w);
    w.blocks.iter().map(|b| b.dest.is_none() && parts.iter().any(|p| p.min == b.min && p.max == b.max)).collect()
}

// ------------------------------------------------------------------ static

pub fn build(m: &mut Mesh, w: &World) {
    if w.map != ID {
        return;
    }
    let l = layout::layout(w);
    let parts = layout::structure(w);
    for (i, b) in parts.iter().enumerate() {
        part(m, &l, &parts, b, i as u32);
    }
    under_deck(m, &l);
    outfalls(m, w, &l);
    markings(m, &l);
}

/// The high-water mark the floods leave on everything in the channel.
fn high_water(l: &Layout) -> f32 {
    l.floor + layout::DEPTH + 0.08
}

fn part(m: &mut Mesh, l: &Layout, parts: &[Block], b: &Block, i: u32) {
    let size = b.max - b.min;
    let tone = 0.94 + 0.08 * hash01(i / 2, 0x5B1);
    match b.kind {
        BlockKind::Concrete => {
            m.cuboid(b.min, b.max, shade(CONCRETE, tone * (0.97 + 0.03 * b.tint as f32)));
            if size.y > 0.6 {
                // A darker cap, and the wet band below the high-water mark.
                let e = 0.03;
                m.cuboid(vec3(b.min.x - e, b.max.y - 0.14, b.min.z - e), b.max + vec3(e, 0.01, e), shade(CONCRETE_DARK, tone));
                let mark = high_water(l).min(b.max.y - 0.2);
                if b.min.y < mark && size.y > 1.2 {
                    m.cuboid(b.min - vec3(e, 0.0, e), vec3(b.max.x + e, mark, b.max.z + e), shade(WET, tone));
                }
            } else {
                // A slab: steel edge angles along its sides.
                let (e, y0, y1) = (0.025, b.max.y - 0.12, b.max.y + 0.004);
                m.cuboid(vec3(b.min.x - e, y0, b.min.z - e), vec3(b.max.x + e, y1, b.min.z + 0.05), STEEL);
                m.cuboid(vec3(b.min.x - e, y0, b.max.z - 0.05), vec3(b.max.x + e, y1, b.max.z + e), STEEL);
                m.cuboid(vec3(b.min.x - e, y0, b.min.z - e), vec3(b.min.x + 0.05, y1, b.max.z + e), STEEL);
                m.cuboid(vec3(b.max.x - 0.05, y0, b.min.z - e), vec3(b.max.x + e, y1, b.max.z + e), STEEL);
                // Expansion joints across the top.
                let n = ((size.x.max(size.z)) / 2.5) as usize;
                for k in 1..n {
                    let t = k as f32 / n as f32;
                    if size.x >= size.z {
                        let x = b.min.x + size.x * t;
                        m.cuboid(vec3(x - 0.02, b.max.y, b.min.z), vec3(x + 0.02, b.max.y + 0.003, b.max.z), CONCRETE_DARK);
                    } else {
                        let z = b.min.z + size.z * t;
                        m.cuboid(vec3(b.min.x, b.max.y, z - 0.02), vec3(b.max.x, b.max.y + 0.003, z + 0.02), CONCRETE_DARK);
                    }
                }
            }
        }
        BlockKind::Step => {
            m.cuboid(b.min, b.max, shade(CONCRETE, tone * (0.92 + 0.06 * b.tint as f32)));
            // A yellow nosing on the edge that faces down the stair.
            let c = (b.min + b.max) * 0.5;
            let x = if c.x > 0.0 { b.max.x - 0.07 } else { b.min.x + 0.07 };
            m.cuboid(vec3(x - 0.07, b.max.y - 0.03, b.min.z + 0.05), vec3(x + 0.07, b.max.y + 0.006, b.max.z - 0.05), YELLOW);
            if b.min.y < high_water(l) {
                m.cuboid(b.min - vec3(0.01, 0.0, 0.01), vec3(b.max.x + 0.01, high_water(l).min(b.max.y - 0.05), b.max.z + 0.01), WET);
            }
        }
        BlockKind::Sandbag => {
            let mid = (b.min.y + b.max.y) * 0.5 + 0.2;
            m.cuboid(b.min, vec3(b.max.x, mid, b.max.z), shade(SAND, tone));
            m.cuboid(vec3(b.min.x + 0.08, mid, b.min.z + 0.08), b.max - vec3(0.08, 0.0, 0.08), shade(SAND, tone * 1.08));
        }
        BlockKind::Metal => metal(m, l, parts, b, i),
        _ => m.cuboid(b.min, b.max, STEEL),
    }
}

fn metal(m: &mut Mesh, l: &Layout, parts: &[Block], b: &Block, i: u32) {
    let size = b.max - b.min;
    let c = (b.min + b.max) * 0.5;
    if size.y < 0.12 {
        // A rail beam; the top one of a pair carries the posts.
        m.cuboid(b.min, b.max, GALV);
        let lower = parts.iter().any(|q| q.min.x == b.min.x && q.max.z == b.max.z && (b.min.y - q.min.y - 0.5).abs() < 0.01);
        if lower {
            let along_x = size.x > size.z;
            let len = if along_x { size.x } else { size.z };
            let n = (len / 2.0).ceil().max(1.0) as usize;
            let foot = b.min.y - 0.95;
            for k in 0..=n {
                let t = k as f32 / n as f32;
                let p = if along_x { vec3(b.min.x + size.x * t, 0.0, c.z) } else { vec3(c.x, 0.0, b.min.z + size.z * t) };
                m.cuboid(vec3(p.x - 0.035, foot, p.z - 0.035), vec3(p.x + 0.035, b.max.y + 0.02, p.z + 0.035), GALV);
            }
            // A toe board along the foot of the rail.
            m.cuboid(vec3(b.min.x, foot, b.min.z), vec3(b.max.x, foot + 0.1, b.max.z), YELLOW);
        }
        return;
    }
    let deck = l.deck;
    if b.min.y >= deck + layout::GATE_CLEAR - 0.01 {
        if (b.min.y - (deck + layout::GATE_CLEAR)).abs() < 0.01 {
            gate_leaf(m, b);
        } else if b.min.y > layout::gear_bottom(deck) + 0.1 {
            winding_house(m, b);
        } else {
            // The frame's girders.
            let (a, z) = if size.x > size.z {
                (vec3(b.min.x, c.y, c.z), vec3(b.max.x, c.y, c.z))
            } else {
                (vec3(c.x, c.y, b.min.z), vec3(c.x, c.y, b.max.z))
            };
            girder(m, a, z, 0.5, size.y, STEEL);
        }
        return;
    }
    if size.x < 0.6 && size.z < 0.6 {
        // A leg of the gate frame, with a yellow foot plate.
        girder(m, vec3(c.x, b.min.y, c.z), vec3(c.x, b.max.y, c.z), 0.5, 0.5, STEEL);
        m.cuboid(vec3(b.min.x - 0.12, b.min.y, b.min.z - 0.12), vec3(b.max.x + 0.12, b.min.y + 0.06, b.max.z + 0.12), YELLOW);
        return;
    }
    if (b.min.y - l.terrace).abs() < 0.01 {
        headstock(m, b, i);
        return;
    }
    // Machinery on the deck: a valve cabinet, or a winch.
    if size.y > 1.4 {
        m.cuboid(b.min, b.max, shade(GREEN, 0.95 + 0.1 * hash01(i, 0x77)));
        m.cuboid(vec3(b.min.x - 0.04, b.max.y - 0.08, b.min.z - 0.04), b.max + vec3(0.04, 0.03, 0.04), STEEL);
        // Gauges and a wheel on each long face.
        for s in [-1.0f32, 1.0] {
            let z = if s > 0.0 { b.max.z + 0.02 } else { b.min.z - 0.02 };
            m.cuboid(vec3(c.x - 0.5, c.y + 0.2, z - 0.02), vec3(c.x - 0.2, c.y + 0.5, z + 0.02), [0.85, 0.85, 0.8, 0.0]);
            let g = Mat4::from_translation(vec3(c.x + 0.3, c.y, z + s * 0.08));
            m.tube(&g, Vec3::ZERO, 0.3, 0.24, 0.03, 8, [0.62, 0.12, 0.08, 0.0]);
        }
        beam(m, vec3(b.min.x, b.min.y + 0.4, c.z), vec3(b.min.x - 0.6, b.min.y + 0.4, c.z), 0.22, 0.22, RUST);
    } else {
        let top = b.min.y + 0.35;
        m.cuboid(b.min, vec3(b.max.x, top, b.max.z), STEEL);
        let r = (b.max.y - top) * 0.5;
        let g = Mat4::from_translation(vec3(c.x, top + r, c.z)) * Mat4::from_rotation_y(PI * 0.5);
        m.tube(&g, Vec3::ZERO, r, 0.0, size.x * 0.5 - 0.15, 8, YELLOW);
        for s in [-1.0f32, 1.0] {
            m.cuboid(
                vec3(c.x + s * (size.x * 0.5 - 0.15) - 0.08, b.min.y, b.min.z),
                vec3(c.x + s * (size.x * 0.5 - 0.15) + 0.08, b.max.y, b.max.z),
                STEEL,
            );
        }
        // Rope wound on the drum.
        m.tube(&g, Vec3::ZERO, r + 0.04, 0.0, size.x * 0.3, 8, [0.2, 0.2, 0.19, 0.0]);
    }
}

/// The old floodgate, lifted clear: a ribbed steel leaf across the channel.
fn gate_leaf(m: &mut Mesh, b: &Block) {
    let c = (b.min + b.max) * 0.5;
    m.cuboid(b.min, b.max, STEEL);
    let size = b.max - b.min;
    for k in 0..4 {
        let y = b.min.y + 0.2 + k as f32 * (size.y - 0.4) / 3.0;
        for s in [-1.0f32, 1.0] {
            let x = c.x + s * (size.x * 0.5 + 0.06);
            m.cuboid(vec3(x - 0.06, y - 0.08, b.min.z + 0.1), vec3(x + 0.06, y + 0.08, b.max.z - 0.1), shade(STEEL, 0.8));
        }
    }
    for j in 0..5u32 {
        let z = b.min.z + 1.2 + (size.z - 2.4) * j as f32 / 4.0 + (hash01(j, 0x6A) - 0.5);
        for s in [-1.0f32, 1.0] {
            let x = c.x + s * (size.x * 0.5 + 0.005);
            m.cuboid(vec3(x - 0.01, b.min.y + 0.1, z - 0.3), vec3(x + 0.01, b.max.y - 0.4, z + 0.3), RUST);
        }
    }
    for s in [-1.0f32, 1.0] {
        let g = wall(vec3(c.x + s * (size.x * 0.5 + 0.13), c.y - 0.4, c.z), s * PI * 0.5);
        stencil(m, &g, "GATE C", 0.11, YELLOW);
    }
    // The leaf hangs on chains from the winding house.
    for z in [-3.0f32, 3.0] {
        beam(m, vec3(c.x, b.max.y, c.z + z), vec3(c.x, b.max.y + 0.7, c.z + z), 0.08, 0.08, DARK);
    }
}

fn winding_house(m: &mut Mesh, b: &Block) {
    let c = (b.min + b.max) * 0.5;
    m.cuboid(b.min, b.max, YELLOW);
    m.cuboid(vec3(b.min.x - 0.12, b.max.y, b.min.z - 0.12), vec3(b.max.x + 0.12, b.max.y + 0.12, b.max.z + 0.12), STEEL);
    for s in [-1.0f32, 1.0] {
        let x = if s > 0.0 { b.max.x + 0.01 } else { b.min.x - 0.01 };
        m.cuboid(vec3(x - 0.01, c.y - 0.15, c.z - 0.6), vec3(x + 0.01, c.y + 0.3, c.z + 0.6), DARK);
    }
    // A shaft out each end to the chain wheels.
    let g = Mat4::from_translation(vec3(c.x, c.y, c.z));
    m.tube(&g, Vec3::ZERO, 0.12, 0.0, (b.max.z - b.min.z) * 0.5 + 0.5, 6, STEEL);
}

/// A sluice gate's headstock on the terrace: a gearbox, a handwheel, and
/// the housing the gate's rising stem slides into.
fn headstock(m: &mut Mesh, b: &Block, i: u32) {
    let c = (b.min + b.max) * 0.5;
    let toward = if c.z > 0.0 { -1.0 } else { 1.0 }; // the channel side
    m.cuboid(vec3(b.min.x, b.min.y, b.min.z), vec3(b.max.x, b.min.y + 0.25, b.max.z), STEEL);
    m.cuboid(
        vec3(b.min.x + 0.2, b.min.y + 0.25, b.min.z + 0.05),
        vec3(b.max.x - 0.2, b.max.y, b.max.z - 0.05),
        shade(YELLOW, 0.95 + 0.1 * hash01(i, 3)),
    );
    let back = vec3(c.x, b.min.y + 0.75, c.z - toward * (b.max.z - b.min.z) * 0.5 - toward * 0.12);
    let g = Mat4::from_translation(back);
    m.tube(&g, Vec3::ZERO, 0.42, 0.34, 0.03, 8, [0.62, 0.12, 0.08, 0.0]);
    beam(m, back, back + vec3(0.0, 0.0, toward * 0.15), 0.06, 0.06, STEEL);
    // The stem housing: a tube up out of the gearbox.
    m.cuboid(vec3(c.x - 0.09, b.max.y, c.z - 0.09), vec3(c.x + 0.09, b.max.y + 0.9, c.z + 0.09), STEEL);
}

/// Steel cross-girders under the deck and under each bridge span.
fn under_deck(m: &mut Mesh, l: &Layout) {
    let y = l.deck - layout::SLAB - 0.17;
    let d = layout::DECK_HALF - 0.2;
    for k in 0..7 {
        let x = -9.0 + 3.0 * k as f32;
        girder(m, vec3(x, y, -d), vec3(x, y, d), 0.3, 0.34, STEEL);
    }
    for z in [-5.0f32, 5.0] {
        girder(m, vec3(-d, y - 0.02, z), vec3(d, y - 0.02, z), 0.32, 0.3, shade(STEEL, 0.9));
    }
    for s in [-1.0f32, 1.0] {
        for x in [-1.1f32, 1.1] {
            girder(m, vec3(s * x, y, s * layout::DECK_HALF), vec3(s * x, y, s * l.headwall), 0.3, 0.34, STEEL);
        }
    }
}

/// The culvert mouths' dark throats, the gate guides on the headwall face
/// and the lettering over them.
fn outfalls(m: &mut Mesh, w: &World, l: &Layout) {
    for s in [-1.0f32, 1.0] {
        let face = s * l.headwall;
        let back = s * (l.headwall + layout::HEADWALL_T);
        for x in [-layout::CULVERT_X, layout::CULVERT_X] {
            let x = s * x;
            let hw = layout::CULVERT_HALF;
            let bottom = w.height(x, face) - 0.6;
            // The throat: the fill behind, painted out to black.
            m.cuboid(vec3(x - hw, bottom, back - s * 0.02), vec3(x + hw, l.floor + layout::CULVERT_H, back + s * 0.01), DARK);
            // Gate guides up the face and over the terrace edge, and the
            // beam across their tops.
            let top = l.terrace + 1.9;
            for g in [-1.0f32, 1.0] {
                let gx = x + g * (hw + 0.12);
                m.cuboid(vec3(gx - 0.12, bottom, face - s * 0.32), vec3(gx + 0.12, top, face - s * 0.02), STEEL);
            }
            let (z0, z1) = if s > 0.0 { (face - 0.32, face + 1.6) } else { (face - 1.6, face + 0.32) };
            m.cuboid(vec3(x - hw - 0.3, top, z0), vec3(x + hw + 0.3, top + 0.25, z1), STEEL);
        }
        let g = wall(vec3(0.0, l.floor + layout::CULVERT_H + 0.35, face - s * 0.01), if s > 0.0 { PI } else { 0.0 });
        stencil(m, &g, if s > 0.0 { "OUTFALL 1" } else { "OUTFALL 2" }, 0.075, shade(CONCRETE, 1.25));
    }
}

/// Paint on the deck: hazard strips at the bridge and stair heads.
fn markings(m: &mut Mesh, l: &Layout) {
    let y = l.deck + 0.006;
    let edge = layout::DECK_HALF - 0.6;
    for s in [-1.0f32, 1.0] {
        for k in 0..7 {
            let t = -1.4 + k as f32 * 0.4;
            let col = if k % 2 == 0 { YELLOW } else { DARK };
            m.cuboid(vec3(t, y - 0.004, s * edge - 0.15), vec3(t + 0.4, y, s * edge + 0.15), col);
        }
        for k in 0..8 {
            let t = -1.2 + k as f32 * 0.3;
            let col = if k % 2 == 0 { YELLOW } else { DARK };
            m.cuboid(vec3(s * edge - 0.15, y - 0.004, t), vec3(s * edge + 0.15, y, t + 0.3), col);
        }
    }
}

// -------------------------------------------------------------- per frame

/// The water surface across the channel at `x`, or below the floor if
/// there is none.
fn level(w: &World, l: &Layout, x: f32, f: Flood) -> f32 {
    layout::water_surface(w, x, 0.0, f).unwrap_or(l.floor - 1.0)
}

/// Seconds into the current siren window.
fn phase(time: f32) -> f32 {
    time.max(0.0).rem_euclid(SIREN_PERIOD)
}

pub fn atmosphere(m: &mut Mesh, alpha: &mut Mesh, add: &mut Mesh, w: &World, time: f32) {
    if w.map != ID {
        return;
    }
    let f = events::flood(w.seed, time);
    let l = layout::layout(w);
    sluice_gates(m, alpha, add, w, &l, f, time);
    if f.level > 0.0 {
        water(alpha, add, w, &l, f, time);
    }
    // While it drains and for a while after, the channel stays wet.
    let p = phase(time);
    let tested = events::siren_tested(w.seed, events::siren_cycle(time));
    if tested && p > GATES_CLOSE && p < WATER_DRAINED + 24.0 {
        wet_floor(alpha, w, &l, f, p);
    }
}

/// The four sluice gates: steel leaves that lift in their guides as the
/// flood is released and drop again when it stops.
fn sluice_gates(m: &mut Mesh, alpha: &mut Mesh, add: &mut Mesh, w: &World, l: &Layout, f: Flood, time: f32) {
    let lift = f.gates * (layout::CULVERT_H + 0.2);
    let hw = layout::CULVERT_HALF + 0.08;
    for s in [-1.0f32, 1.0] {
        let face = s * l.headwall;
        for x in [-layout::CULVERT_X, layout::CULVERT_X] {
            let x = s * x;
            let y0 = l.floor - 0.1 + lift;
            let y1 = l.floor + layout::CULVERT_H + 0.15 + lift;
            let z = face - s * 0.17;
            m.cuboid(vec3(x - hw, y0, z - 0.07), vec3(x + hw, y1, z + 0.07), shade(STEEL, 1.1));
            for k in 0..3 {
                let y = y0 + 0.3 + k as f32 * (y1 - y0 - 0.6) / 2.0;
                m.cuboid(vec3(x - hw, y - 0.06, z - s * 0.07 - s * 0.08), vec3(x + hw, y + 0.06, z - s * 0.07), shade(STEEL, 0.8));
            }
            m.cuboid(vec3(x - 0.5, y1 - 0.5, z - s * 0.08 - 0.005), vec3(x + 0.5, y1 - 0.2, z - s * 0.08 + 0.005), YELLOW);
            // The rising stem, up through the guide beam to the headstock.
            let stem = vec3(x, y1, face + s * 1.25);
            beam(m, vec3(x, y1, z), vec3(x, y1 + 0.2, z), 0.12, 0.12, STEEL);
            beam(m, stem, vec3(x, l.terrace + 2.05 + lift, face + s * 1.25), 0.08, 0.08, GALV);
            if f.gates > 0.02 {
                gush(alpha, add, w, l, f, x, s, lift.min(layout::CULVERT_H), time);
            }
        }
    }
}

/// White water bursting from under a rising gate and falling into the
/// channel, with spray where it lands. `s` is the bank (+1 north).
#[allow(clippy::too_many_arguments)]
fn gush(alpha: &mut Mesh, add: &mut Mesh, w: &World, l: &Layout, f: Flood, x: f32, s: f32, open: f32, time: f32) {
    let face = s * l.headwall;
    let hw = layout::CULVERT_HALF - 0.1;
    let surface = level(w, l, x, f);
    // Once the gates start to close the jet weakens with them.
    let strength = f.gates * (0.6 + 0.4 * (1.0 - events::smooth(GATES_CLOSE - 2.0, GATES_CLOSE + 4.0, phase(time))));
    let v = 3.5 + 5.0 * strength;
    let top = l.floor + open * 0.92;
    let mut prev: Option<(Vec3, f32)> = None;
    for k in 0..=8 {
        let t = k as f32 * 0.09;
        let y = top - 4.9 * t * t;
        let landed = y < surface.max(l.floor + 0.05);
        let p = vec3(x, y.max(surface.max(l.floor + 0.05)), face - s * (0.3 + v * t));
        let spread = hw + t * 1.6;
        if let Some((q, qs)) = prev {
            let streak = |d: f32| 0.55 + 0.25 * (d * 2.3 - time * 14.0).sin();
            let ca = [0.86, 0.92, 0.94, streak(k as f32 - 1.0) * strength];
            let cb = [0.86, 0.92, 0.94, streak(k as f32) * strength];
            alpha.quad_c(
                q + vec3(-qs, 0.0, 0.0),
                q + vec3(qs, 0.0, 0.0),
                p + vec3(spread, 0.0, 0.0),
                p + vec3(-spread, 0.0, 0.0),
                [ca, ca, cb, cb],
            );
            // The face of the jet, down to the floor or the water.
            let low = surface.max(l.floor);
            alpha.quad_c(
                q + vec3(-qs, 0.0, 0.0),
                p + vec3(-spread, 0.0, 0.0),
                vec3(p.x - spread, low, p.z),
                vec3(q.x - qs, low, q.z),
                [ca, cb, [0.8, 0.88, 0.9, 0.3 * strength], [0.8, 0.88, 0.9, 0.3 * strength]],
            );
        }
        prev = Some((p, spread));
        if landed {
            // Spray and boil where it comes down.
            let seed = (x * 7.0) as i32 as u32 ^ (s > 0.0) as u32;
            for j in 0..5u32 {
                let age = (time * 0.9 + hash01(j, seed)).fract();
                let o = vec3((hash01(j, seed ^ 7) - 0.5) * 2.5, 0.3 + age * 2.2, -s * (hash01(j, seed ^ 9) - 0.5) * 2.0);
                soft(alpha, p + o, 0.8 + age * 1.6, 0.6 + age * 1.2, [0.9, 0.94, 0.96, 0.32 * (1.0 - age) * strength]);
            }
            soft(add, p + Vec3::Y * 0.4, 2.2, 0.9, [0.5, 0.55, 0.58, 0.18 * strength]);
            break;
        }
    }
}

/// The surge: a sheet of muddy water filling the channel out to its
/// leading edge, foam along the front and the banks, and streaks running
/// outward from C.
fn water(alpha: &mut Mesh, add: &mut Mesh, w: &World, l: &Layout, f: Flood, time: f32) {
    const STEP: f32 = 2.5;
    const STRIPS: usize = 6;
    let reach = (f.reach + 2.0).min(layout::FLOOD_X);
    let n = (reach / STEP).ceil() as i32;
    // Where the water meets the bank: the surface is level across the
    // channel, so it is where the ground rises through it.
    let shore = |x: f32, s: f32| {
        let mut z = 0.0;
        while z < layout::FLOOD_HALF_Z && w.height(x, z) < s && w.height(x, -z) < s {
            z += 0.75;
        }
        z + 0.75
    };
    let surface = |x: f32| layout::water_surface(w, x, 0.0, f);
    let col = |x: f32, z: f32, s: f32| -> Col {
        let depth = (s - w.height(x, z)).max(0.0);
        let deep = events::smooth(0.0, 1.4, depth);
        let ripple = 0.04 * (x * 0.31 + z * 0.17 - time * 1.7).sin();
        [0.40 - 0.16 * deep + ripple, 0.43 - 0.12 * deep + ripple, 0.37 - 0.05 * deep + ripple, 0.55 + 0.33 * deep]
    };
    for i in -n..n {
        let (x0, x1) = (i as f32 * STEP, (i + 1) as f32 * STEP);
        let (Some(s0), Some(s1)) = (surface(x0), surface(x1)) else {
            continue;
        };
        let (z0, z1) = (shore(x0, s0), shore(x1, s1));
        let wave = |x: f32, z: f32| 0.035 * ((x * 0.7 + time * 2.4).sin() + (z * 0.9 - time * 1.9).sin());
        for k in 0..STRIPS {
            let (a, b) = (-1.0 + 2.0 * k as f32 / STRIPS as f32, -1.0 + 2.0 * (k + 1) as f32 / STRIPS as f32);
            let p = |x: f32, s: f32, z: f32| vec3(x, s + wave(x, z), z);
            let (za0, zb0, za1, zb1) = (a * z0, b * z0, a * z1, b * z1);
            alpha.quad_c(
                p(x0, s0, za0),
                p(x0, s0, zb0),
                p(x1, s1, zb1),
                p(x1, s1, za1),
                [col(x0, za0, s0), col(x0, zb0, s0), col(x1, zb1, s1), col(x1, za1, s1)],
            );
        }
        // Foam where it laps the banks.
        for side in [-1.0f32, 1.0] {
            let fz = |z: f32| side * (z - 0.9);
            let a = 0.3 + 0.12 * ((x0 * 0.5 + time * 3.0).sin());
            let c = [0.88, 0.9, 0.86, a * f.level];
            alpha.quad(
                vec3(x0, s0 + 0.04, fz(z0) - 0.5),
                vec3(x0, s0 + 0.04, fz(z0) + 0.5),
                vec3(x1, s1 + 0.04, fz(z1) + 0.5),
                vec3(x1, s1 + 0.04, fz(z1) - 0.5),
                c,
            );
        }
    }
    // The front: a churning wall of foam while the surge is still running.
    if f.reach < layout::FLOOD_X - 20.0 {
        for side in [-1.0f32, 1.0] {
            let x = side * (f.reach - 1.0);
            if let Some(s) = layout::water_surface(w, x - side * 3.0, 0.0, f) {
                let zs = shore(x, s + 0.3);
                for k in 0..10 {
                    let (za, zb) = (-zs + 2.0 * zs * k as f32 / 10.0, -zs + 2.0 * zs * (k + 1) as f32 / 10.0);
                    let chop = |z: f32| 0.5 + 0.4 * (z * 1.7 + time * 6.0).sin();
                    let c = [0.92, 0.93, 0.9, 0.7];
                    let back = x - side * 2.5;
                    alpha.quad_c(
                        vec3(back, s + 0.05, za),
                        vec3(back, s + 0.05, zb),
                        vec3(x, s + chop(zb), zb),
                        vec3(x, s + chop(za), za),
                        [[0.9, 0.92, 0.9, 0.2], [0.9, 0.92, 0.9, 0.2], c, c],
                    );
                }
                soft(alpha, vec3(x, s + 0.8, 0.0), 2.0, 1.2, [0.9, 0.93, 0.95, 0.25]);
            }
        }
    }
    // Streaks of foam carried outward from C, both ways.
    let seed = (w.seed ^ (w.seed >> 32)) as u32;
    for k in 0..40u32 {
        let side = if k % 2 == 0 { 1.0 } else { -1.0 };
        let speed = 4.0 + 3.0 * hash01(k, seed ^ 0x51);
        let life = 18.0;
        let age = (time / life + hash01(k, seed ^ 0x52)).fract() * life;
        let x = side * (layout::DECK_HALF + 2.0 + age * speed);
        if x.abs() > f.reach - 3.0 || x.abs() > layout::FLOOD_X - 10.0 {
            continue;
        }
        let Some(s) = surface(x) else { continue };
        let z = (hash01(k, seed ^ 0x53) - 0.5) * 2.0 * (shore(x, s) - 2.0).max(1.0);
        let len = 2.0 + 3.0 * hash01(k, seed ^ 0x54);
        let a = 0.2 * f.level * events::smooth(0.0, 2.0, age) * (1.0 - events::smooth(life - 4.0, life, age));
        let (c, tail) = ([0.9, 0.92, 0.88, a], [0.9, 0.92, 0.88, 0.0]);
        let y = s + 0.05;
        alpha.quad_c(
            vec3(x - side * len, y, z - 0.08),
            vec3(x - side * len, y, z + 0.08),
            vec3(x, y, z + 0.16),
            vec3(x, y, z - 0.16),
            [tail, tail, c, c],
        );
    }
    // Water piling against the piers, columns and stair feet.
    let s = level(w, l, 0.0, f);
    if s > l.floor + 0.3 {
        let a = 0.3 * f.level;
        for (x, z) in [(3.6, 5.0), (-3.6, 5.0), (3.6, -5.0), (-3.6, -5.0)] {
            soft(alpha, vec3(x, s + 0.2, z), 1.0, 0.35, [0.9, 0.93, 0.92, a]);
        }
        for sx in [-1.0f32, 1.0] {
            soft(alpha, vec3(sx * (layout::DECK_HALF + 1.5), s + 0.2, 0.0), 1.6, 0.35, [0.9, 0.93, 0.92, a]);
        }
        // Light glancing off the surface near C.
        soft(add, vec3(0.0, s + 0.1, 12.0), 6.0, 0.4, [0.35, 0.4, 0.42, 0.08 * f.level]);
        soft(add, vec3(0.0, s + 0.1, -12.0), 6.0, 0.4, [0.35, 0.4, 0.42, 0.08 * f.level]);
    }
}

/// Dark, wet ground where the water has drained off, and puddles left in
/// the low places, drying out after the flood.
fn wet_floor(alpha: &mut Mesh, w: &World, l: &Layout, f: Flood, p: f32) {
    let dry = 1.0 - events::smooth(WATER_DRAINED - 4.0, WATER_DRAINED + 24.0, p);
    let full = level(w, l, 0.0, layout::FULL);
    const STEP: f32 = 4.0;
    let n = ((layout::FLOOD_X - 8.0) / STEP) as i32;
    for i in -n..n {
        let (x0, x1) = (i as f32 * STEP, (i + 1) as f32 * STEP);
        let fade = |x: f32| 1.0 - events::smooth(layout::FLOOD_X - 30.0, layout::FLOOD_X - 8.0, x.abs());
        for j in -6..6 {
            let (z0, z1) = (j as f32 * STEP, (j + 1) as f32 * STEP);
            let corner = |x: f32, z: f32| {
                let g = w.height(x, z);
                let left = full - g;
                // Only where the water was, and has gone.
                let gone = layout::water_surface(w, x, z, f).is_none_or(|s| s < g);
                let a = if left > 0.0 && gone { 0.45 * dry * fade(x) * events::smooth(0.0, 0.6, left) } else { 0.0 };
                (vec3(x, g + 0.09, z), [0.13, 0.14, 0.13, a])
            };
            let c = [corner(x0, z0), corner(x0, z1), corner(x1, z1), corner(x1, z0)];
            if c.iter().all(|v| v.1[3] <= 0.0) {
                continue;
            }
            alpha.quad_c(c[0].0, c[1].0, c[2].0, c[3].0, [c[0].1, c[1].1, c[2].1, c[3].1]);
        }
    }
    // Puddles, longest-lived in the middle of the channel.
    let seed = (w.seed ^ (w.seed >> 32)) as u32;
    for k in 0..14u32 {
        let x = (hash01(k, seed ^ 0x61) - 0.5) * 150.0;
        let z = (hash01(k, seed ^ 0x62) - 0.5) * 18.0;
        if (x.abs() < layout::DECK_HALF + 8.0 && z.abs() < layout::DECK_HALF + 1.0) || layout::water_surface(w, x, z, f).is_some() {
            continue;
        }
        let r = (0.8 + 1.8 * hash01(k, seed ^ 0x63)) * (0.4 + 0.6 * dry);
        let y = w.height(x, z) + 0.1;
        // Still water shows the overcast sky.
        let c = [0.52, 0.58, 0.6, 0.6 * dry.sqrt()];
        let pts: Vec<Vec3> = (0..7)
            .map(|j| {
                let a = j as f32 / 7.0 * std::f32::consts::TAU;
                let rr = r * (0.75 + 0.35 * hash01(k * 7 + j, seed ^ 0x64));
                vec3(x + a.cos() * rr, y, z + a.sin() * rr * 0.7)
            })
            .collect();
        for j in 0..7 {
            alpha.tri(vec3(x, y, z), pts[(j + 1) % 7], pts[j], c);
        }
    }
}

// ------------------------------------------------------------------ sound

/// A sound the flood makes at a place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cue {
    /// The sluice gates' gear taking up the load as they start to lift
    /// (and again as they drop shut).
    Clank,
    /// Water roaring out of the culverts and along the channel; a few
    /// seconds long, cued again before it ends while the water runs.
    Rush,
}

/// How often the rushing water is cued again while it runs.
pub const RUSH_EVERY: f32 = 2.0;

/// The cues that start in `(from, to]` of the world clock: what, where, and
/// how loud (0..1, before distance).
pub fn cues(w: &World, from: f32, to: f32) -> Vec<(Cue, Vec3, f32)> {
    let mut out = Vec::new();
    if w.map != ID || !(from.is_finite() && to.is_finite()) || to <= from || to - from > 5.0 {
        return out; // a clock jump (a new round, the killcam) starts nothing
    }
    let l = layout::layout(w);
    // The gear clanks at both outfalls as the gates start to move.
    for c in events::siren_cycle(from)..=events::siren_cycle(to) {
        if !events::siren_tested(w.seed, c) {
            continue;
        }
        for edge in [GATES_OPEN, GATES_CLOSE] {
            let at = c as f32 * SIREN_PERIOD + edge;
            if at > from && at <= to {
                for s in [-1.0f32, 1.0] {
                    out.push((Cue::Clank, vec3(0.0, l.terrace + 1.5, s * l.headwall), 1.0));
                }
            }
        }
    }
    let first = (from / RUSH_EVERY).floor() as i64 + 1;
    let last = (to / RUSH_EVERY).floor() as i64;
    for k in first..=last {
        let t = k as f32 * RUSH_EVERY;
        let f = events::flood(w.seed, t);
        let loud = (f.gates * 0.8 + f.level * 0.4).min(1.0);
        if loud > 0.05 {
            out.push((Cue::Rush, vec3(0.0, l.floor + 1.0, 0.0), loud));
        }
    }
    out
}

// ----------------------------------------------------------------- review

/// Cameras for the asset_review example: the deck from the north bank, the
/// north outfall from the channel, and the deck from the west stair foot.
#[allow(dead_code)]
pub fn view(w: &World, view: &str) -> Option<(Vec3, Vec3)> {
    if w.map != ID {
        return None;
    }
    let l = layout::layout(w);
    let y = l.deck;
    Some(match view {
        "platform" => (vec3(-26.0, y + 9.0, 34.0), vec3(0.0, y, 0.0)),
        "outfall" => (vec3(-9.0, l.floor + 2.2, 5.0), vec3(4.0, l.floor + 1.5, l.headwall)),
        "channel" => (vec3(-40.0, l.floor + 3.5, -14.0), vec3(0.0, y - 1.0, 0.0)),
        "deck" => (vec3(-1.0, y + 1.7, 8.5), vec3(3.0, y + 0.5, -6.0)),
        _ => return None,
    })
}
