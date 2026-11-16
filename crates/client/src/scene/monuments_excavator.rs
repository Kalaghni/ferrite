//! Quarry: the Titan Excavator. A walking dragline far too big for the pit
//! squats on the south ridge beyond the fence. Its lattice boom reaches 170
//! metres out over the terraces, and its bucket - big enough to swallow a
//! haul truck - hangs on its hoist ropes high above the pit floor, north of
//! C. The tub and the spoil around it are static; the whole revolving house,
//! boom and bucket are rebuilt each frame because the machine occasionally
//! slews a few degrees and the bucket sways on its ropes.
use super::{bar, block, drum, ease, ground_range, jit, roll, ticks, Cue};
use crate::gfx::{shade, Col, Mesh};
use crate::scene::slag_cathedral::art::{soft, stencil, wall};
use crate::scene::slag_cathedral::beam;
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::World;
use std::f32::consts::{FRAC_PI_2, TAU};

pub const MAP: u8 = 1;

const PAINT: Col = [0.80, 0.60, 0.20, 0.0];
const PAINT_DARK: Col = [0.52, 0.38, 0.14, 0.0];
const STEEL: Col = [0.26, 0.26, 0.25, 0.0];
const DARK: Col = [0.10, 0.10, 0.11, 0.0];
const ROOF: Col = [0.48, 0.46, 0.42, 0.0];
const GLASS: Col = [0.18, 0.26, 0.30, 0.08];
const WORN: Col = [0.56, 0.54, 0.50, 0.0];
const ROPE: Col = [0.14, 0.13, 0.12, 0.0];
const STRIPE: Col = [0.09, 0.09, 0.09, 0.0];
const SPOIL: Col = [0.60, 0.54, 0.44, 0.0];

/// Centre of the tub on the south ridge.
const TUB: (f32, f32) = (0.0, -146.0);
const TUB_R: f32 = 20.0;
/// Boom foot, in the revolving frame.
const FOOT: Vec3 = Vec3::new(0.0, 5.0, 20.0);
const BOOM_LEN: f32 = 172.0;
const BOOM_RISE: f32 = 0.56;
/// The bucket hangs this far above the ground below it.
const BUCKET_CLEAR: f32 = 33.0;
/// One slew cycle; in most the house turns out a few degrees and back.
const SLEW_PERIOD: f32 = 80.0;

fn tub_top(w: &World) -> f32 {
    ground_range(w, vec3(TUB.0, 0.0, TUB.1), 31.0).1 + 1.2
}
fn boom_dir() -> Vec3 {
    vec3(0.0, BOOM_RISE.sin(), BOOM_RISE.cos())
}
/// "Up" across the boom, in the revolving frame.
fn boom_up() -> Vec3 {
    vec3(0.0, BOOM_RISE.cos(), -BOOM_RISE.sin())
}
fn tip() -> Vec3 {
    FOOT + boom_dir() * BOOM_LEN
}
/// Where the house points (radians from +z) at `time`.
fn slew(w: &World, time: f32) -> f32 {
    let cycle = (time / SLEW_PERIOD).floor() as u32;
    let phase = time.rem_euclid(SLEW_PERIOD);
    if roll(w, cycle, 0x51E) >= 0.6 {
        return 0.0;
    }
    let r = roll(w, cycle, 0x51F) * 2.0 - 1.0;
    let target = (0.035 + 0.05 * r.abs()) * r.signum();
    target * ease(8.0, 20.0, phase) * (1.0 - ease(50.0, 62.0, phase))
}
fn frame(w: &World, angle: f32) -> Mat4 {
    Mat4::from_translation(vec3(TUB.0, tub_top(w), TUB.1)) * Mat4::from_rotation_y(angle)
}

pub fn covers(x: f32, z: f32, r: f32) -> bool {
    let d = ((x - TUB.0).powi(2) + (z - TUB.1).powi(2)).sqrt();
    d < 38.0 + r || spoil().iter().any(|&(sx, sz, sr, _)| ((x - sx).powi(2) + (z - sz).powi(2)).sqrt() < sr + r)
}

#[allow(dead_code)] // the asset_review example's cameras
pub fn view(w: &World, view: &str) -> Option<(Vec3, Vec3)> {
    let top = tub_top(w);
    Some(match view {
        // From the pit floor north of C, looking up the boom at the machine.
        "excavator" => (vec3(24.0, w.height(24.0, 64.0) + 1.8, 64.0), vec3(0.0, top + 30.0, -70.0)),
        // Under the bucket.
        "bucket" => (vec3(-26.0, w.height(-26.0, -14.0) + 1.8, -14.0), vec3(0.0, w.height(0.0, 24.0) + BUCKET_CLEAR + 4.0, 20.0)),
        // The machine itself, from the west ridge.
        "dragline" => (vec3(-95.0, top + 24.0, -96.0), vec3(0.0, top + 22.0, -130.0)),
        _ => return None,
    })
}

/// Spoil heaps around the machine: (x, z, radius, height).
fn spoil() -> [(f32, f32, f32, f32); 4] {
    [(-44.0, -138.0, 14.0, 9.0), (38.0, -150.0, 12.0, 7.0), (52.0, -128.0, 9.0, 5.0), (-30.0, -162.0, 10.0, 6.0)]
}

pub fn build(m: &mut Mesh, w: &World) {
    let top = tub_top(w);
    let c = vec3(TUB.0, 0.0, TUB.1);
    let (lo, _) = ground_range(w, c, TUB_R);
    // The tub: a broad steel drum the whole machine turns on.
    drum(m, vec3(c.x, lo - 2.0, c.z), vec3(c.x, top - 1.2, c.z), TUB_R, 16, jit(STEEL, 1));
    drum(m, vec3(c.x, top - 1.2, c.z), vec3(c.x, top, c.z), TUB_R - 1.5, 16, DARK);
    // Roller path and its rack.
    drum(m, vec3(c.x, top - 1.6, c.z), vec3(c.x, top - 1.1, c.z), TUB_R + 0.4, 16, PAINT_DARK);
    for (k, (sx, sz, r, h)) in spoil().into_iter().enumerate() {
        heap(m, w, vec3(sx, 0.0, sz), r, h, k as u32);
    }
    // A haul truck parked by the tub, for scale, and the trailing power
    // cable snaking off to its substation.
    let g = w.height(34.0, -122.0);
    haul_truck(m, &(Mat4::from_translation(vec3(34.0, g, -122.0)) * Mat4::from_rotation_y(0.4)));
    let mut p = vec3(-TUB_R + 2.0, top - 2.0, TUB.1 + 4.0);
    for k in 0..9 {
        let x = -TUB_R - 4.0 - k as f32 * 6.0;
        let z = TUB.1 + 4.0 + (k as f32 * 0.9).sin() * 5.0;
        let q = vec3(x, w.height(x, z) + 0.25, z);
        beam(m, p, q, 0.5, 0.5, DARK);
        p = q;
    }
    let s = vec3(-80.0, w.height(-80.0, -138.0), -138.0);
    m.cuboid(s + vec3(-4.0, -1.0, -3.0), s + vec3(4.0, 3.5, 3.0), [0.45, 0.47, 0.44, 0.0]);
    for x in [-2.0f32, 2.0] {
        bar(m, s + vec3(x, 3.5, 0.0), s + vec3(x, 9.0, 0.0), 0.25, STEEL);
    }
    bar(m, s + vec3(-3.0, 8.6, 0.0), s + vec3(3.0, 8.6, 0.0), 0.2, STEEL);
}

/// A faceted heap of spoil, sunk into the ground at its rim.
fn heap(m: &mut Mesh, w: &World, c: Vec3, r: f32, h: f32, seed: u32) {
    const N: usize = 9;
    let base = ground_range(w, c, r * 0.5).1;
    let top = vec3(c.x, base + h, c.z);
    let ring = |k: usize, f: f32, y: f32| {
        let a = k as f32 / N as f32 * TAU + seed as f32;
        let j = 0.85 + 0.3 * crate::scene::hash01(k as u32 % N as u32, seed + 70);
        let (x, z) = (c.x + a.cos() * r * f * j, c.z + a.sin() * r * f * j);
        vec3(x, if y < 0.0 { w.height(x, z) - 0.6 } else { base + y }, z)
    };
    for k in 0..N {
        m.quad(
            ring(k, 1.0, -1.0),
            ring(k, 0.5, h * 0.7),
            ring(k + 1, 0.5, h * 0.7),
            ring(k + 1, 1.0, -1.0),
            jit(SPOIL, seed * 13 + k as u32),
        );
        m.tri(ring(k, 0.5, h * 0.7), top, ring(k + 1, 0.5, h * 0.7), jit(shade(SPOIL, 1.08), seed * 17 + k as u32));
    }
}

/// An ordinary 8 m mining truck: the scale everything else is measured by.
fn haul_truck(m: &mut Mesh, g: &Mat4) {
    block(m, g, vec3(-2.6, 1.2, -4.2), vec3(2.6, 2.0, 4.2), DARK);
    let p = |x: f32, y: f32, z: f32| g.transform_point3(vec3(x, y, z));
    m.corners(
        [
            p(-2.8, 2.0, -4.3),
            p(2.8, 2.0, -4.3),
            p(-3.0, 4.6, -4.6),
            p(3.0, 4.6, -4.6),
            p(-2.8, 2.0, 2.2),
            p(2.8, 2.0, 2.2),
            p(-3.0, 4.2, 2.8),
            p(3.0, 4.2, 2.8),
        ],
        PAINT,
    );
    block(m, g, vec3(-2.6, 2.0, 2.4), vec3(-0.2, 4.0, 4.2), PAINT);
    block(m, g, vec3(-2.5, 3.0, 4.15), vec3(-0.3, 3.8, 4.25), GLASS);
    for (x, z) in [(-2.3f32, -2.6f32), (2.3, -2.6), (-2.3, 3.0), (2.3, 3.0)] {
        drum(m, p(x - 0.6, 1.2, z), p(x + 0.6, 1.2, z), 1.2, 5, DARK);
    }
}

// ------------------------------------------------------------- the house --

fn house(m: &mut Mesh, g: &Mat4) {
    // Revolving frame and machinery house.
    block(m, g, vec3(-21.0, 0.0, -30.0), vec3(21.0, 2.5, 18.0), STEEL);
    block(m, g, vec3(-18.0, 2.5, -26.0), vec3(18.0, 22.0, 10.0), PAINT);
    block(m, g, vec3(-18.4, 22.0, -26.4), vec3(18.4, 23.0, 10.4), ROOF);
    block(m, g, vec3(-18.08, 2.5, -26.08), vec3(18.08, 4.0, 10.08), PAINT_DARK);
    // Counterweight with hazard stripes.
    block(m, g, vec3(-16.0, 2.5, -32.5), vec3(16.0, 15.0, -26.0), PAINT_DARK);
    for k in 0..8 {
        let x = -15.0 + k as f32 * 4.0;
        let f = *g * Mat4::from_translation(vec3(x, 13.2, -32.55)) * Mat4::from_rotation_z(0.7);
        m.obox(&f, Vec3::ZERO, vec3(0.5, 2.0, 0.05), STRIPE);
    }
    // Window bands and the name on both flanks.
    for side in [-1.0f32, 1.0] {
        let x = side * 18.06;
        for k in 0..6 {
            let z = -23.0 + k as f32 * 5.6;
            block(m, g, vec3(x - 0.04, 19.0, z), vec3(x + 0.04, 20.6, z + 3.8), GLASS);
        }
        for k in 0..12 {
            let z = -25.0 + k as f32 * 3.0;
            block(m, g, vec3(x - 0.05, 4.0, z), vec3(x + 0.05, 22.0, z + 0.25), PAINT_DARK);
        }
        let flank = *g * wall(vec3(x * 1.003, 11.6, -8.0), side * FRAC_PI_2);
        stencil(m, &flank, "TITAN", 0.85, STRIPE);
        // Walking shoes on their cam arms, raised clear of the ground.
        block(m, g, vec3(side * 22.0, -1.6, -22.0), vec3(side * 30.0, 2.6, 12.0), STEEL);
        block(m, g, vec3(side * 22.2, 2.6, -20.0), vec3(side * 29.8, 3.2, 10.0), PAINT_DARK);
        drum(m, g.transform_point3(vec3(side * 18.2, 6.5, -5.0)), g.transform_point3(vec3(side * 20.2, 6.5, -5.0)), 3.6, 8, PAINT_DARK);
        bar(m, g.transform_point3(vec3(side * 20.0, 6.5, -5.0)), g.transform_point3(vec3(side * 26.0, 2.6, -11.0)), 1.6, STEEL);
        bar(m, g.transform_point3(vec3(side * 20.0, 6.5, -5.0)), g.transform_point3(vec3(side * 26.0, 2.6, 2.0)), 1.6, STEEL);
    }
    // Operator's cab out on the front corner, with lit glass.
    block(m, g, vec3(-17.5, 8.0, 10.0), vec3(-9.5, 15.0, 15.5), PAINT);
    block(m, g, vec3(-17.2, 11.0, 15.5), vec3(-9.8, 14.3, 15.6), [0.62, 0.70, 0.66, 0.35]);
    block(m, g, vec3(-17.8, 15.0, 9.8), vec3(-9.2, 15.6, 15.9), ROOF);
    // Roof plant: exhaust stacks and cooling housings.
    for (x, z) in [(-10.0f32, -18.0f32), (-4.0, -18.0)] {
        drum(m, g.transform_point3(vec3(x, 23.0, z)), g.transform_point3(vec3(x, 28.0, z)), 0.8, 6, DARK);
    }
    for (x, z) in [(6.0f32, -20.0f32), (6.0, -11.0), (-12.0, 2.0)] {
        block(m, g, vec3(x - 3.0, 23.0, z - 3.0), vec3(x + 3.0, 25.6, z + 3.0), ROOF);
        block(m, g, vec3(x - 2.4, 25.6, z - 2.4), vec3(x + 2.4, 25.9, z + 2.4), DARK);
    }
    // A-frame: two legs each side to a crosshead high above the roof.
    let apex = |s: f32| vec3(s * 5.0, 54.0, -8.0);
    for s in [-1.0f32, 1.0] {
        for z in [6.0f32, -22.0] {
            bar(m, g.transform_point3(vec3(s * 15.0, 23.0, z)), g.transform_point3(apex(s)), 1.7, jit(PAINT, (z + 30.0) as u32));
        }
        for y in [32.0f32, 42.0] {
            let t = (y - 23.0) / 31.0;
            let a = vec3(s * 15.0, 23.0, 6.0).lerp(apex(s), t);
            let b = vec3(s * 15.0, 23.0, -22.0).lerp(apex(s), t);
            bar(m, g.transform_point3(a), g.transform_point3(b), 0.7, PAINT_DARK);
        }
        // Backstays to the rear of the house.
        beam(m, g.transform_point3(apex(s)), g.transform_point3(vec3(s * 12.0, 23.0, -26.0)), 0.3, 0.3, ROPE);
    }
    bar(m, g.transform_point3(apex(-1.0)), g.transform_point3(apex(1.0)), 1.6, PAINT_DARK);
    for s in [-1.0f32, 1.0] {
        drum(
            m,
            g.transform_point3(apex(s) + vec3(-s * 1.2, 0.0, 0.0)),
            g.transform_point3(apex(s) + vec3(-s * 2.2, 0.0, 0.0)),
            1.8,
            8,
            DARK,
        );
    }
}

/// One point on a boom chord: `t` along the boom, `sx`/`sy` choose the chord.
fn chord(t: f32, sx: f32, sy: f32) -> Vec3 {
    let width = 6.5 + (2.2 - 6.5) * t;
    let depth = if t < 0.12 { 1.0 + (5.0 - 1.0) * t / 0.12 } else { 5.0 + (1.8 - 5.0) * (t - 0.12) / 0.88 };
    FOOT + boom_dir() * (BOOM_LEN * t) + Vec3::X * (sx * width) + boom_up() * (sy * depth)
}

fn boom(m: &mut Mesh, g: &Mat4) {
    const BAYS: usize = 20;
    let p = |t: f32, sx: f32, sy: f32| g.transform_point3(chord(t, sx, sy));
    let corners = [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    for b in 0..BAYS {
        let (t0, t1) = (b as f32 / BAYS as f32, (b + 1) as f32 / BAYS as f32);
        for (k, &(sx, sy)) in corners.iter().enumerate() {
            bar(m, p(t0, sx, sy), p(t1, sx, sy), 0.9 - 0.4 * t0, jit(PAINT, b as u32 * 4 + k as u32));
            // One diagonal on each face, alternating so the faces zig-zag.
            let (nx, ny) = corners[(k + 1) % 4];
            let (a, c) = if b % 2 == 0 { (p(t0, sx, sy), p(t1, nx, ny)) } else { (p(t1, sx, sy), p(t0, nx, ny)) };
            bar(m, a, c, 0.36, PAINT_DARK);
        }
        if b % 2 == 0 {
            for k in 0..4 {
                let ((sx, sy), (nx, ny)) = (corners[k], corners[(k + 1) % 4]);
                bar(m, p(t0, sx, sy), p(t0, nx, ny), 0.4, PAINT_DARK);
            }
        }
    }
    // Foot hinges and the tip sheaves.
    for s in [-1.0f32, 1.0] {
        drum(m, g.transform_point3(FOOT + vec3(s * 5.4, 0.0, 0.0)), g.transform_point3(FOOT + vec3(s * 7.6, 0.0, 0.0)), 1.5, 6, DARK);
    }
    let t = tip();
    block(
        m,
        &(*g * Mat4::from_translation(t) * Mat4::from_rotation_x(-BOOM_RISE)),
        vec3(-2.6, -2.0, -3.0),
        vec3(2.6, 2.0, 1.0),
        PAINT_DARK,
    );
    for x in [-1.3f32, 1.3] {
        drum(m, g.transform_point3(t + vec3(x - 0.35, 0.0, 1.2)), g.transform_point3(t + vec3(x + 0.35, 0.0, 1.2)), 2.4, 8, DARK);
    }
    // Suspension pendants from the A-frame to the boom, and the hoist
    // ropes running up along the boom to the tip.
    for s in [-1.0f32, 1.0] {
        let apex = vec3(s * 4.0, 54.0, -8.0);
        for t in [0.42f32, 0.7, 1.0] {
            beam(m, g.transform_point3(apex), p(t, s, 1.0), 0.32, 0.32, ROPE);
        }
        let start = vec3(s * 1.2, 17.0, 10.0);
        let rise = chord(0.04, 0.0, 1.0) + vec3(s * 1.2, 1.4, 0.0);
        beam(m, g.transform_point3(start), g.transform_point3(rise), 0.22, 0.22, ROPE);
        beam(m, g.transform_point3(rise), g.transform_point3(t + vec3(s * 1.3, 2.4, 1.2)), 0.22, 0.22, ROPE);
    }
}

/// The bucket, mouth toward the machine (-z), bottom at y = 0 of `g`.
fn bucket(m: &mut Mesh, g: &Mat4) {
    block(m, g, vec3(-5.0, 0.0, -6.8), vec3(5.0, 0.7, 7.5), STEEL);
    for s in [-1.0f32, 1.0] {
        let p = |x: f32, y: f32, z: f32| g.transform_point3(vec3(s * x, y, z));
        // Side walls sweep down toward the mouth.
        m.corners(
            [
                p(4.4, 0.0, -7.2),
                p(5.0, 0.0, -7.2),
                p(4.4, 4.6, -7.2),
                p(5.0, 4.6, -7.2),
                p(4.4, 0.0, 7.5),
                p(5.0, 0.0, 7.5),
                p(4.4, 7.0, 7.5),
                p(5.0, 7.0, 7.5),
            ],
            jit(STEEL, 3 + s as u32),
        );
        // Wear plates and trunnions for the hoist chains.
        block(m, g, vec3(s * 5.0, 1.0, -5.0), vec3(s * 5.25, 3.0, 6.0), WORN);
        drum(m, p(5.0, 4.6, 1.0), p(5.9, 4.6, 1.0), 0.7, 6, DARK);
        // The arch across the mouth.
        bar(m, p(4.7, 4.6, -6.6), p(0.0, 9.2, -6.6), 1.0, PAINT_DARK);
    }
    block(m, g, vec3(-5.0, 0.0, 7.0), vec3(5.0, 7.0, 7.6), STEEL);
    block(m, g, vec3(-5.4, -0.2, -7.9), vec3(5.4, 1.1, -6.6), WORN);
    for k in 0..5 {
        let x = -4.0 + k as f32 * 2.0;
        let p = |dx: f32, y: f32, z: f32| g.transform_point3(vec3(x + dx, y, z));
        m.corners(
            [
                p(-0.45, -0.1, -7.9),
                p(0.45, -0.1, -7.9),
                p(-0.45, 0.9, -7.9),
                p(0.45, 0.9, -7.9),
                p(-0.2, 0.1, -9.6),
                p(0.2, 0.1, -9.6),
                p(-0.2, 0.5, -9.6),
                p(0.2, 0.5, -9.6),
            ],
            WORN,
        );
    }
    // A load of rock still in it, and a dark interior.
    block(m, g, vec3(-4.4, 0.7, -5.0), vec3(4.4, 1.2, 7.0), DARK);
    for k in 0..5u32 {
        let c = vec3(-3.0 + k as f32 * 1.5, 1.6, 2.0 + (k * 3 % 5) as f32 - 2.0);
        let r = 0.9 + 0.5 * crate::scene::hash01(k, 0xB0);
        block(m, g, c - Vec3::splat(r), c + vec3(r, r * 0.8, r), jit(SPOIL, k));
    }
}

pub fn atmosphere(m: &mut Mesh, alpha: &mut Mesh, add: &mut Mesh, w: &World, time: f32) {
    let angle = slew(w, time);
    let g = frame(w, angle);
    house(m, &g);
    boom(m, &g);
    // The bucket swings under the tip: a slow pendulum, kicked sideways
    // while the house slews, pulled a little back toward the machine by
    // its drag ropes.
    let turning = (slew(w, time) - slew(w, time - 1.0)) * 60.0;
    let sway = vec3((time * 0.37 + roll(w, 2, 0xB5) * TAU).sin() * 1.4 - turning * 3.0, 0.0, (time * 0.29).sin() * 0.7);
    let tip_w = g.transform_point3(tip());
    let back = g.transform_vector3(vec3(0.0, 0.0, -9.0));
    let mut hang = vec3(tip_w.x, 0.0, tip_w.z) + back;
    let floor = w.height(hang.x, hang.z);
    let bottom = floor.max(w.height(0.0, 24.0)) + BUCKET_CLEAR;
    hang += g.transform_vector3(sway);
    hang.y = bottom;
    let yaw = angle + (time * 0.21).sin() * 0.05;
    let bg = Mat4::from_translation(hang) * Mat4::from_rotation_y(yaw) * Mat4::from_rotation_x(sway.z * 0.01);
    bucket(m, &bg);
    // Hoist chains to the spreader, ropes up to the sheaves.
    let spreader = bg.transform_point3(vec3(0.0, 15.0, 1.0));
    block(m, &(bg * Mat4::from_translation(vec3(0.0, 15.0, 1.0))), vec3(-6.0, -0.4, -0.4), vec3(6.0, 0.4, 0.4), DARK);
    for s in [-1.0f32, 1.0] {
        beam(m, bg.transform_point3(vec3(s * 5.4, 4.6, 1.0)), bg.transform_point3(vec3(s * 5.6, 15.0, 1.0)), 0.3, 0.3, ROPE);
        beam(
            m,
            spreader + g.transform_vector3(vec3(s * 1.3, 0.0, 0.0)),
            tip_w + g.transform_vector3(vec3(s * 1.3, -2.0, 1.2)),
            0.26,
            0.26,
            ROPE,
        );
    }
    // Drag chains meet at the hitch; the drag rope sags back to the fairlead.
    let hitch = bg.transform_point3(vec3(0.0, 1.0, -15.0));
    for s in [-1.0f32, 1.0] {
        beam(m, bg.transform_point3(vec3(s * 5.2, 1.0, -7.0)), hitch, 0.3, 0.3, ROPE);
    }
    let fairlead = g.transform_point3(vec3(0.0, 3.0, 18.5));
    let mut prev = hitch;
    for k in 1..=8 {
        let t = k as f32 / 8.0;
        let mut q = hitch.lerp(fairlead, t);
        q.y -= 4.0 * t * (1.0 - t) * 4.0;
        beam(m, prev, q, 0.32, 0.32, ROPE);
        prev = q;
    }
    // Aircraft warning lamps on the A-frame and at the boom tip.
    let blink = (time / 1.5).fract() < 0.2;
    for p in [g.transform_point3(vec3(0.0, 55.5, -8.0)), tip_w + Vec3::Y * 3.0] {
        let c = if blink { 0.9 } else { 0.12 };
        soft(add, p, 1.6, 1.6, [1.0, 0.15, 0.05, c]);
    }
    // Dust shaken from the bucket drifts down over the pit.
    let seed = (w.seed ^ (w.seed >> 32)) as u32;
    for k in 0..10u32 {
        let age = (time * (0.05 + 0.02 * crate::scene::hash01(k, 0xD1)) + crate::scene::hash01(k, seed ^ 0xD2)).fract();
        let p =
            hang + vec3((crate::scene::hash01(k, 0xD3) - 0.5) * 9.0, -age * 18.0, (crate::scene::hash01(k, 0xD4) - 0.5) * 12.0 + age * 3.0);
        soft(alpha, p, 1.2 + age * 3.5, 1.0 + age * 3.0, [0.82, 0.76, 0.64, 0.22 * (1.0 - age) * age.sqrt()]);
    }
}

/// The ropes groan when the house starts to slew, and now and then as the
/// bucket swings.
pub fn cues(w: &World, from: f32, to: f32, out: &mut Vec<(f32, Cue, Vec3)>) {
    let foot = frame(w, 0.0).transform_point3(FOOT + Vec3::Y * 10.0);
    for start in [8.0f32, 50.0] {
        for t in ticks(from, to, SLEW_PERIOD, start, |k| roll(w, k, 0x51E) < 0.6) {
            out.push((t, Cue::Groan, foot));
        }
    }
    let tip_w = frame(w, 0.0).transform_point3(tip());
    for t in ticks(from, to, 23.0, 5.0, |k| roll(w, k, 0x6A0) < 0.4) {
        out.push((t, Cue::Groan, tip_w));
    }
}
