//! Monuments: three giants at the edges of the oldest battlefields - the
//! Broken Viaduct across Foundry Valley, the Titan Excavator over the Quarry
//! and the Great Dam behind Spillway.
//!
//! They are scenery only. Nothing here collides or is known to the server:
//! everything over the playable field is at least `CLEARANCE` above the
//! ground, and everything at player height stands beyond the fence. Their
//! ambient motion and sound cues are pure functions of the world seed and
//! the `time` passed in, so every client sees and hears the same thing.

use super::hash01;
use super::slag_cathedral::{basis, beam};
use crate::gfx::{shade, Col, Mesh};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::{Tree, World, HALF_X, HALF_Z};

#[path = "monuments_dam.rs"]
mod dam;
#[path = "monuments_excavator.rs"]
mod excavator;
#[path = "monuments_viaduct.rs"]
mod viaduct;

/// Minimum height of any monument part above the ground inside the fence
/// (checked by the tests below).
#[cfg(test)]
const CLEARANCE: f32 = 5.0;

/// A sound a monument makes at a place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    /// The viaduct's hanging train and its carriage, settling in the wind.
    Creak,
    /// The excavator's hoist cables and slewing gear.
    Groan,
    /// The dam's warning sirens under test.
    Siren,
}

/// The cues that start in `(from, to]` of the visual clock, and where.
pub fn cues(w: &World, from: f32, to: f32) -> Vec<(Cue, Vec3)> {
    let mut out = Vec::new();
    if !(from.is_finite() && to.is_finite()) || to <= from || to - from > 5.0 {
        return Vec::new(); // a clock jump (a new round, the killcam) starts nothing
    }
    match w.map {
        viaduct::MAP => viaduct::cues(w, from, to, &mut out),
        excavator::MAP => excavator::cues(w, from, to, &mut out),
        dam::MAP => dam::cues(w, from, to, &mut out),
        _ => {}
    }
    out.sort_by(|a: &(f32, Cue, Vec3), b| a.0.total_cmp(&b.0));
    out.into_iter().map(|(_, cue, at)| (cue, at)).collect()
}

/// True where a monument stands, so trees there are not drawn through it.
pub fn hides_tree(w: &World, t: &Tree) -> bool {
    hides(w, t.base.x, t.base.z, t.radius)
}

/// True if a plant of radius `r` at (x, z) would stand inside a monument.
/// Never inside the fence: trees there have colliding trunks.
pub fn hides(w: &World, x: f32, z: f32, r: f32) -> bool {
    if on_field(vec3(x, 0.0, z)) {
        return false;
    }
    match w.map {
        viaduct::MAP => viaduct::covers(x, z, r),
        excavator::MAP => excavator::covers(x, z, r),
        dam::MAP => dam::covers(w, x, z, r),
        _ => false,
    }
}

pub fn build(m: &mut Mesh, w: &World) {
    match w.map {
        viaduct::MAP => viaduct::build(m, w),
        excavator::MAP => excavator::build(m, w),
        dam::MAP => dam::build(m, w),
        _ => {}
    }
}

/// Everything about the monuments that moves, rebuilt each frame.
pub fn atmosphere(m: &mut Mesh, alpha: &mut Mesh, add: &mut Mesh, w: &World, time: f32) {
    let time = if time.is_finite() { time.max(0.0) } else { 0.0 };
    match w.map {
        viaduct::MAP => viaduct::atmosphere(m, alpha, add, w, time),
        excavator::MAP => excavator::atmosphere(m, alpha, add, w, time),
        dam::MAP => dam::atmosphere(m, alpha, add, w, time),
        _ => {}
    }
}

/// Camera for each monument's review view in asset_review: (eye, target).
#[allow(dead_code)] // the asset_review example's cameras
pub fn review_view(w: &World, view: &str) -> Option<(Vec3, Vec3)> {
    match (w.map, view) {
        (viaduct::MAP, _) => viaduct::view(w, view),
        (excavator::MAP, _) => excavator::view(w, view),
        (dam::MAP, _) => dam::view(w, view),
        _ => None,
    }
}

// ------------------------------------------------------------- helpers ----

/// A member's colour, varied a little so a lattice is not one flat tone.
fn jit(col: Col, k: u32) -> Col {
    shade(col, 0.86 + 0.26 * hash01(k, 0x3F1))
}

/// Square bar from `a` to `b`.
fn bar(m: &mut Mesh, a: Vec3, b: Vec3, width: f32, col: Col) {
    beam(m, a, b, width, width, col);
}

/// A round shaft or wheel along `a` -> `b`, cut into `segs` facets (before
/// the renderer's `ROUND` doubling).
fn drum(m: &mut Mesh, a: Vec3, b: Vec3, radius: f32, segs: usize, col: Col) {
    let (g, len) = basis(a, b);
    m.tube(&g, Vec3::ZERO, radius, 0.0, len * 0.5, segs, col);
}

/// Lowest and highest ground over a disc, sampled on a coarse grid.
fn ground_range(w: &World, c: Vec3, r: f32) -> (f32, f32) {
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for i in -4..=4 {
        for j in -4..=4 {
            let (x, z) = (c.x + r * i as f32 / 4.0, c.z + r * j as f32 / 4.0);
            let h = w.height(x, z);
            lo = lo.min(h);
            hi = hi.max(h);
        }
    }
    (lo, hi)
}

/// A box in the frame `g`, from local corner `a` to local corner `b`.
fn block(m: &mut Mesh, g: &Mat4, a: Vec3, b: Vec3, col: Col) {
    m.obox(g, (a + b) * 0.5, (b - a).abs() * 0.5, col);
}

/// Inside the fence, with a margin.
fn on_field(p: Vec3) -> bool {
    p.x.abs() < HALF_X + 1.0 && p.z.abs() < HALF_Z + 1.0
}

/// Smoothstep.
fn ease(e0: f32, e1: f32, x: f32) -> f32 {
    super::smooth(e0, e1, x)
}

/// A random-looking number in [0, 1) from the world seed and an event key.
fn roll(w: &World, key: u32, salt: u32) -> f32 {
    hash01(((w.seed ^ (w.seed >> 32)) as u32) ^ key.wrapping_mul(0x9E37_79B9), salt)
}

/// Cue times: every `period` seconds from `offset`, those for which `keep`
/// says yes, falling in `(from, to]`.
fn ticks(from: f32, to: f32, period: f32, offset: f32, mut keep: impl FnMut(u32) -> bool) -> Vec<f32> {
    let first = ((from - offset) / period).floor().max(0.0) as u32;
    let last = ((to - offset) / period).floor().max(0.0) as u32;
    (first..=last)
        .filter_map(|k| {
            let t = offset + k as f32 * period;
            (t > from && t <= to && keep(k)).then_some(t)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn finite(mesh: &Mesh) {
        for (v, i) in mesh.chunks.iter().map(|(v, i)| (v, i)).chain(std::iter::once((&mesh.v, &mesh.i))) {
            assert!(i.iter().all(|&k| (k as usize) < v.len()));
            for v in v {
                assert!(v.pos.iter().chain(v.nrm.iter()).chain(v.col.iter()).all(|x| x.is_finite()), "non-finite vertex {:?}", v.pos);
            }
        }
    }

    /// Every vertex over the playable field is out of reach.
    pub(super) fn out_of_reach(w: &World, mesh: &Mesh, what: &str) {
        for v in mesh.chunks.iter().flat_map(|(v, _)| v.iter()).chain(mesh.v.iter()) {
            let p = Vec3::from_array(v.pos);
            if on_field(p) {
                let g = w.height(p.x, p.z);
                assert!(p.y > g + CLEARANCE, "{what} reaches down to {:.1} m above the field at {p:?}", p.y - g);
            }
        }
    }

    fn bounds(mesh: &Mesh) -> (Vec3, Vec3) {
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for v in mesh.chunks.iter().flat_map(|(v, _)| v.iter()).chain(mesh.v.iter()) {
            lo = lo.min(Vec3::from_array(v.pos));
            hi = hi.max(Vec3::from_array(v.pos));
        }
        (lo, hi)
    }

    /// Static budgets, set from the triangle profile: each monument costs
    /// a few percent of its map's static world (0.9 - 1.4 million).
    const STATIC: [(u8, usize, usize); 3] = [(0, 8_000, 32_000), (1, 800, 6_000), (4, 6_000, 24_000)];
    /// Per-frame budgets for the moving parts, alongside landmark machinery.
    const DYNAMIC: [(u8, usize); 3] = [(0, 3_500), (1, 7_000), (4, 2_500)];

    #[test]
    fn monuments_are_finite_bounded_and_out_of_reach() {
        for seed in [0xFE221u64, 7, 0xDEAD_BEEF] {
            for (id, lo, hi) in STATIC {
                let w = World::with_map(seed, id);
                let mut m = Mesh::default();
                build(&mut m, &w);
                finite(&m);
                let n = m.triangles();
                assert!(n > lo && n < hi, "map {id} monument has {n} triangles, budget {lo}..{hi}");
                out_of_reach(&w, &m, "static monument");
                let (_, mut top) = bounds(&m);
                for t in [0.0, 3.3, 17.0, 33.0, 61.5, 95.0, 240.0, 1234.5, 86_000.0] {
                    let (mut m, mut alpha, mut add) = (Mesh::default(), Mesh::default(), Mesh::default());
                    atmosphere(&mut m, &mut alpha, &mut add, &w, t);
                    top = top.max(bounds(&m).1);
                    for mesh in [&m, &alpha, &add] {
                        finite(mesh);
                        out_of_reach(&w, mesh, "moving monument part");
                    }
                    let budget = DYNAMIC.iter().find(|d| d.0 == id).unwrap().1;
                    let n = m.triangles() + alpha.triangles() + add.triangles();
                    assert!(n > 0 && n < budget, "map {id} moving parts: {n} triangles, budget {budget}");
                }
                // A dominant silhouette: well above anything else on the map.
                let field = w.points.iter().map(|p| p.y).fold(f32::MIN, f32::max);
                assert!(top.y > field + 45.0, "map {id} monument should tower over the field ({} m)", top.y - field);
            }
        }
    }

    #[test]
    fn only_scenery_trees_beyond_the_fence_give_way() {
        for id in [0u8, 1, 4] {
            let w = World::with_map(0xFE221, id);
            let hidden = w.trees.iter().filter(|t| hides_tree(&w, t)).count();
            assert!(hidden > 0, "map {id}: the monument should clear the slope it stands on");
            for t in w.trees.iter().filter(|t| on_field(t.base)) {
                assert!(!hides_tree(&w, t), "map {id}: a tree on the field at {:?} would vanish", t.base);
            }
        }
    }

    #[test]
    fn other_maps_have_no_monument() {
        for id in [2, 3, 5, 6] {
            let w = World::with_map(0xFE221, id);
            let (mut m, mut alpha, mut add) = (Mesh::default(), Mesh::default(), Mesh::default());
            build(&mut m, &w);
            atmosphere(&mut m, &mut alpha, &mut add, &w, 10.0);
            assert_eq!(m.triangles() + alpha.triangles() + add.triangles(), 0);
            assert!(cues(&w, 0.0, 4.0).is_empty());
        }
    }

    #[test]
    fn motion_is_bounded_deterministic_and_alive() {
        // How far any moving vertex may travel: the hanging engine swings a
        // little, the excavator's boom tip sweeps a few degrees of a 190 m
        // radius, the dam's gantry crawls the length of its hoist bridge.
        for (id, reach) in [(0u8, 8.0f32), (1, 30.0), (4, 100.0)] {
            let w = World::with_map(0xFE221, id);
            let frame = |t: f32| {
                let (mut m, mut alpha, mut add) = (Mesh::default(), Mesh::default(), Mesh::default());
                atmosphere(&mut m, &mut alpha, &mut add, &w, t);
                m.v.iter().chain(alpha.v.iter()).map(|v| Vec3::from_array(v.pos)).collect::<Vec<_>>()
            };
            let first = frame(12.0);
            assert_eq!(first, frame(12.0), "map {id} motion must be a function of time alone");
            let mut moved = false;
            for t in [12.5, 20.0, 47.0, 300.0, 5000.0] {
                let other = frame(t);
                if other.len() == first.len() {
                    // Matching vertices never wander more than the swing allows.
                    let far = first.iter().zip(&other).map(|(a, b)| a.distance(*b)).fold(0.0, f32::max);
                    assert!(far < reach, "map {id} part moved {far} m");
                    moved |= far > 0.05;
                } else {
                    moved = true;
                }
            }
            assert!(moved, "map {id} monument never moves");
        }
    }

    #[test]
    fn cues_are_deterministic_occasional_and_placed_at_the_monument() {
        for (id, cue) in [(0u8, Cue::Creak), (1, Cue::Groan), (4, Cue::Siren)] {
            let w = World::with_map(0xFE221, id);
            let mut all = Vec::new();
            let mut t = 0.0;
            while t < 600.0 {
                all.extend(cues(&w, t, t + 1.0 / 60.0 * 7.0));
                t += 1.0 / 60.0 * 7.0;
            }
            assert!(!all.is_empty(), "map {id} is silent for ten minutes");
            let first = (0..6000).map(|k| k as f32 * 0.1).find(|&s| !cues(&w, s, s + 0.1).is_empty()).unwrap_or(0.0);
            eprintln!("Map {id} review: first {cue:?} at {first:.1} s");
            assert!(all.len() < 200, "map {id} cues {} times in ten minutes", all.len());
            for (c, p) in &all {
                assert_eq!(*c, cue);
                assert!(p.is_finite() && p.y > w.height(p.x, p.z) + CLEARANCE, "cue at {p:?} is not on the monument");
            }
            // Splitting the clock differently gives the same cues.
            let coarse: Vec<_> = (0..600).flat_map(|s| cues(&w, s as f32, s as f32 + 1.0)).collect();
            assert_eq!(coarse, all);
            assert!(cues(&w, 0.0, 100.0).is_empty(), "a clock jump must not fire a burst of cues");
        }
    }
}
