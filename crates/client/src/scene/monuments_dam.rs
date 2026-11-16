//! Spillway: the Great Dam. A concrete gravity dam 60 metres high closes the
//! valley beyond the north fence, holding back the reservoir the drained
//! waterworks once ran from. Five giant steel floodgates crown its central
//! spillway under a high hoist bridge, where a maintenance gantry runs on
//! rails. Powerhouses squat at its toe, rock shoulders anchor both ends, and
//! water still trickles through the sealed gates down the chute.
use super::{bar, block, drum, ease, ground_range, jit, roll, ticks, Cue};
use crate::gfx::{shade, Col, Mesh};
use crate::scene::hash01;
use crate::scene::slag_cathedral::art::{soft, stencil, wall};
use crate::scene::slag_cathedral::{beam, girder};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::World;
use std::f32::consts::{PI, TAU};

pub const MAP: u8 = 4;

const CONCRETE: Col = [0.64, 0.63, 0.59, 0.0];
const CONCRETE_DARK: Col = [0.47, 0.47, 0.45, 0.0];
const STAIN: Col = [0.36, 0.37, 0.35, 0.0];
const GATE: Col = [0.30, 0.34, 0.36, 0.0];
const GATE_RUST: Col = [0.44, 0.27, 0.17, 0.0];
const STEEL: Col = [0.22, 0.25, 0.27, 0.0];
const YELLOW: Col = [0.83, 0.64, 0.18, 0.0];
const ROCK: Col = [0.46, 0.46, 0.43, 0.0];
const WATER: Col = [0.25, 0.33, 0.36, 0.0];
const GLASS: Col = [0.20, 0.26, 0.28, 0.12];
const DARK: Col = [0.08, 0.09, 0.10, 0.0];

/// The dam runs along x between its rock shoulders.
const HALF: f32 = 128.0;
/// Toe of the downstream face, its crest edge, and the upstream face.
const Z_TOE: f32 = 113.0;
const Z_CREST: f32 = 127.0;
const Z_UP: f32 = 137.0;
/// Crest height above the field by the fence.
const HEIGHT: f32 = 66.0;
/// The spillway: five bays of this clear width between six piers.
const BAYS: usize = 5;
const BAY: f32 = 15.0;
const PIER: f32 = 4.0;
/// Gate sill below the crest, and the hoist bridge above it.
const SILL: f32 = 18.0;
const BRIDGE: f32 = 22.0;
/// One siren window; in about half of them the sirens are tested.
const SIREN_PERIOD: f32 = 90.0;
const SIREN: (f32, f32) = (30.0, 44.0);

fn base(w: &World) -> f32 {
    w.height(0.0, 96.0)
}
fn crest(w: &World) -> f32 {
    base(w) + HEIGHT
}
/// The downstream face: z of the face at height y.
fn face_z(w: &World, y: f32) -> f32 {
    let slope = (Z_CREST - Z_TOE) / (HEIGHT - 4.0);
    Z_CREST - (crest(w) - y) * slope
}
fn spill_half() -> f32 {
    (BAYS as f32 * BAY + (BAYS + 1) as f32 * PIER) * 0.5
}
fn pier_x(k: usize) -> f32 {
    -spill_half() + PIER * 0.5 + k as f32 * (BAY + PIER)
}
fn bay_x(k: usize) -> f32 {
    pier_x(k) + (BAY + PIER) * 0.5
}
/// The spillway chute, upstream sill to the flip bucket: (z, y) pairs.
fn chute(w: &World) -> Vec<(f32, f32)> {
    let (h, b) = (crest(w), base(w));
    let mut p = vec![(Z_CREST + 2.5, h - SILL), (Z_CREST - 0.5, h - SILL - 0.6), (Z_CREST - 3.0, h - SILL - 2.4)];
    for y in [h - 27.0, h - 40.0, b + 12.0, b + 6.0] {
        p.push((face_z(w, y) - 0.4, y));
    }
    p.extend([(Z_TOE - 3.0, b + 2.6), (Z_TOE - 6.5, b + 3.0), (Z_TOE - 8.0, b + 4.2)]);
    p
}
/// Height of the rock shoulder at (x, z) above the ground there (0 = none).
fn shoulder(w: &World, x: f32, z: f32) -> f32 {
    let ax = x.abs();
    let along = ease(100.0, 126.0, ax) * (1.0 - ease(150.0, 206.0, ax));
    let back = ease(103.0, 126.0, z);
    let rough = (hash01((x * 0.37) as i32 as u32, (z * 0.41) as i32 as u32 ^ 0x5E) - 0.5) * 7.0;
    let lift = (crest(w) + 15.0 - w.height(x, z) + rough) * along * back;
    lift.max(0.0)
}

/// The dam, its toe works and the flooded valley behind it, plus wherever a
/// rock shoulder rises over the slope.
pub fn covers(w: &World, x: f32, z: f32, r: f32) -> bool {
    (z > 101.0 - r && x.abs() < HALF + r) || (z > 101.0 && x.abs() < 210.0 && shoulder(w, x, z) > 1.5)
}

#[allow(dead_code)] // the asset_review example's cameras
pub fn view(w: &World, view: &str) -> Option<(Vec3, Vec3)> {
    let h = crest(w);
    Some(match view {
        // From the west base, across the channel: the whole wall.
        "dam" => (vec3(-150.0, w.height(-150.0, -10.0) + 2.0, -10.0), vec3(0.0, h - 6.0, 120.0)),
        // At the fence below the gates, looking up the chute.
        "gates" => (vec3(14.0, w.height(14.0, 96.0) + 1.8, 96.0), vec3(0.0, h - 6.0, 128.0)),
        // The crest, the hoist bridge and the gantry from the east shoulder.
        "crest" => (vec3(90.0, h + 20.0, 104.0), vec3(0.0, h + 10.0, 130.0)),
        _ => return None,
    })
}

pub fn build(m: &mut Mesh, w: &World) {
    let (h, b) = (crest(w), base(w));
    monoliths(m, w);
    spillway(m, w);
    // Crest road, parapets and lamp posts along its whole length.
    m.cuboid(vec3(-HALF, h - 0.6, Z_CREST), vec3(HALF, h, Z_UP), shade(CONCRETE_DARK, 0.95));
    for z in [Z_CREST + 0.25, Z_UP - 0.25] {
        for (x0, x1) in [(-HALF, -spill_half()), (spill_half(), HALF)] {
            m.cuboid(vec3(x0, h, z - 0.25), vec3(x1, h + 1.2, z + 0.25), CONCRETE);
        }
    }
    for k in 0..11 {
        let x = -120.0 + k as f32 * 24.0;
        if x.abs() < spill_half() + 2.0 {
            continue;
        }
        let p = vec3(x, h + 1.2, Z_CREST + 0.25);
        bar(m, p, p + Vec3::Y * 5.0, 0.18, STEEL);
        bar(m, p + vec3(0.0, 5.0, 0.0), p + vec3(0.0, 5.0, -1.2), 0.14, STEEL);
        m.cuboid(p + vec3(-0.25, 4.6, -1.5), p + vec3(0.25, 4.85, -0.9), [1.0, 0.86, 0.6, 0.7]);
    }
    // Siren masts on the crest either side of the gates.
    for x in [-70.0f32, 70.0] {
        let p = vec3(x, h, Z_CREST + 3.0);
        bar(m, p, p + Vec3::Y * 11.0, 0.45, STEEL);
        m.cuboid(p + vec3(-1.4, 11.0, -1.4), p + vec3(1.4, 11.4, 1.4), STEEL);
        for (k, yaw) in [0.0f32, PI * 0.5, PI, PI * 1.5].into_iter().enumerate() {
            let g = Mat4::from_translation(p + Vec3::Y * 12.4) * Mat4::from_rotation_y(yaw + 0.4);
            drum(
                m,
                g.transform_point3(vec3(0.0, 0.0, 0.3)),
                g.transform_point3(vec3(0.0, 0.0, 1.9)),
                0.55 + 0.1 * k as f32 % 2.0,
                6,
                YELLOW,
            );
            drum(m, g.transform_point3(vec3(0.0, 0.0, 1.9)), g.transform_point3(vec3(0.0, 0.0, 2.0)), 0.75, 6, DARK);
        }
        m.cuboid(p + vec3(-0.7, 11.4, -0.7), p + vec3(0.7, 13.6, 0.7), YELLOW);
    }
    for side in [-1.0f32, 1.0] {
        powerhouse(m, w, side);
        rock_shoulder(m, w, side);
    }
    // The reservoir behind the wall, held up between the shoulders.
    m.quad(
        vec3(-HALF - 6.0, h - 2.5, Z_UP),
        vec3(-HALF - 6.0, h - 2.5, 168.0),
        vec3(HALF + 6.0, h - 2.5, 168.0),
        vec3(HALF + 6.0, h - 2.5, Z_UP),
        WATER,
    );
    // Stilling basin under the chute: an apron with baffle blocks.
    let (lo, _) = ground_range(w, vec3(0.0, 0.0, 108.0), 6.0);
    m.cuboid(vec3(-spill_half(), lo - 2.0, 104.0), vec3(spill_half(), b + 1.6, Z_TOE), CONCRETE_DARK);
    for k in 0..9 {
        let x = -spill_half() + 6.0 + k as f32 * (spill_half() * 2.0 - 12.0) / 8.0;
        m.cuboid(vec3(x - 1.4, b + 1.6, 106.0), vec3(x + 1.4, b + 3.2, 108.0), CONCRETE);
    }
    for x in [-spill_half() - 1.5, spill_half() + 1.5] {
        m.cuboid(vec3(x - 1.5, lo - 2.0, 104.0), vec3(x + 1.5, b + 7.0, Z_TOE + 2.0), CONCRETE);
    }
}

/// The plain blocks either side of the spillway: a battered face poured in
/// lifts, with contraction joints, gallery vents and drain stains.
fn monoliths(m: &mut Mesh, w: &World) {
    let (h, b) = (crest(w), base(w));
    // Sixteen-metre blocks from each shoulder in to the spillway piers.
    let mut spans = Vec::new();
    for side in [-1.0f32, 1.0] {
        let mut x = spill_half();
        while x < HALF - 0.1 {
            let next = (x + 16.0).min(HALF);
            spans.push(if side > 0.0 { (x, next) } else { (-next, -x) });
            x = next;
        }
    }
    for (k, &(xa, xb)) in spans.iter().enumerate() {
        let k = k as u32;
        let (lo, _) = ground_range(w, vec3((xa + xb) * 0.5, 0.0, Z_TOE + 4.0), 9.0);
        let bottom = lo.min(b) - 4.0;
        let tone = 0.92 + 0.12 * hash01(k, 0xDA);
        const LIFTS: usize = 12;
        for i in 0..LIFTS {
            let y0 = bottom + (h - bottom) * i as f32 / LIFTS as f32;
            let y1 = bottom + (h - bottom) * (i + 1) as f32 / LIFTS as f32;
            let col = shade(CONCRETE, tone * (0.95 + 0.07 * hash01(k * 31 + i as u32, 0xDB)));
            let (z0, z1) = (face_z(w, y0), face_z(w, y1));
            m.quad(vec3(xb, y0, z0), vec3(xa, y0, z0), vec3(xa, y1, z1), vec3(xb, y1, z1), col);
        }
        // Joint, gallery vent and a drain stain on the face.
        let on_face = |x: f32, y: f32| vec3(x, y, face_z(w, y) - 0.06);
        m.quad(on_face(xa + 0.25, bottom), on_face(xa - 0.25, bottom), on_face(xa - 0.25, h), on_face(xa + 0.25, h), CONCRETE_DARK);
        let (vx, vy) = ((xa + xb) * 0.5, h - 24.0);
        m.quad(on_face(vx + 0.9, vy), on_face(vx - 0.9, vy), on_face(vx - 0.9, vy + 1.6), on_face(vx + 0.9, vy + 1.6), DARK);
        let sx = xa + (xb - xa) * (0.2 + 0.6 * hash01(k, 0xDC));
        let sy = h - 3.0 - 30.0 * hash01(k, 0xDD);
        m.quad(on_face(sx + 0.8, sy - 14.0), on_face(sx - 0.8, sy - 14.0), on_face(sx - 1.2, sy), on_face(sx + 1.2, sy), STAIN);
        // The crest block, and the back of it above the water.
        m.cuboid(vec3(xa, h - 3.0, Z_CREST - 0.01), vec3(xb, h - 0.6, Z_UP), shade(CONCRETE, tone));
    }
}

fn spillway(m: &mut Mesh, w: &World) {
    let (h, b) = (crest(w), base(w));
    let profile = chute(w);
    let half = spill_half();
    // The chute surface across all five bays, darker where it stays wet.
    for (i, pair) in profile.windows(2).enumerate() {
        let ((za, ya), (zb, yb)) = (pair[0], pair[1]);
        let col = shade(CONCRETE_DARK, 0.96 + 0.06 * hash01(i as u32, 0xC4));
        m.quad(vec3(half, ya, za), vec3(-half, ya, za), vec3(-half, yb, zb), vec3(half, yb, zb), col);
        // Training walls along both sides.
        for s in [-1.0f32, 1.0] {
            let x = s * (half + 1.0);
            let p = [vec3(x - 1.0, ya, za), vec3(x + 1.0, ya, za), vec3(x - 1.0, ya + 4.5, za), vec3(x + 1.0, ya + 4.5, za)];
            let q = [vec3(x - 1.0, yb, zb), vec3(x + 1.0, yb, zb), vec3(x - 1.0, yb + 4.5, zb), vec3(x + 1.0, yb + 4.5, zb)];
            m.corners([q[0], q[1], q[2], q[3], p[0], p[1], p[2], p[3]], CONCRETE);
        }
    }
    // Under the chute, the dam body fills down to the ground.
    let bottom = ground_range(w, vec3(0.0, 0.0, Z_TOE), 20.0).0 - 4.0;
    m.quad(
        vec3(half, bottom, Z_TOE - 8.0),
        vec3(-half, bottom, Z_TOE - 8.0),
        vec3(-half, b + 4.2, Z_TOE - 8.0),
        vec3(half, b + 4.2, Z_TOE - 8.0),
        CONCRETE,
    );
    // Wet streaks under each gate.
    for k in 0..BAYS {
        let x = bay_x(k);
        for (i, pair) in profile.windows(2).enumerate().skip(1) {
            let ((za, ya), (zb, yb)) = (pair[0], pair[1]);
            let wide = 1.6 + 0.8 * hash01(k as u32, i as u32);
            let off = (hash01(k as u32 + 9, 0x57) - 0.5) * 6.0;
            m.quad(
                vec3(x + off + wide, ya + 0.05, za - 0.05),
                vec3(x + off - wide, ya + 0.05, za - 0.05),
                vec3(x + off - wide * 1.2, yb + 0.05, zb - 0.05),
                vec3(x + off + wide * 1.2, yb + 0.05, zb - 0.05),
                STAIN,
            );
        }
    }
    // Piers rising into hoist towers, the floodgates between them, and the
    // hoist bridge high over everything.
    for k in 0..=BAYS {
        let x = pier_x(k);
        let col = jit(CONCRETE, 70 + k as u32);
        m.cuboid(vec3(x - PIER * 0.5, h - 30.0, Z_CREST - 4.0), vec3(x + PIER * 0.5, h + 1.0, Z_UP), col);
        // Rounded downstream nose.
        drum(m, vec3(x, h - 28.0, Z_CREST - 4.0), vec3(x, h + 1.0, Z_CREST - 4.0), PIER * 0.5, 5, col);
        m.cuboid(vec3(x - PIER * 0.5, h + 1.0, Z_CREST + 0.5), vec3(x + PIER * 0.5, h + BRIDGE, Z_CREST + 5.5), col);
        // Gate slots.
        for s in [-1.0f32, 1.0] {
            m.cuboid(
                vec3(x + s * PIER * 0.5 - 0.3, h - SILL, Z_CREST + 1.4),
                vec3(x + s * PIER * 0.5 + 0.3, h + BRIDGE - 1.0, Z_CREST + 2.6),
                DARK,
            );
        }
    }
    for k in 0..BAYS {
        let x = bay_x(k);
        let g = Mat4::from_translation(vec3(x, h - SILL, Z_CREST + 2.0));
        let hw = BAY * 0.5 + 0.2;
        block(m, &g, vec3(-hw, 0.0, -0.5), vec3(hw, SILL - 0.6, 0.5), jit(GATE, k as u32));
        for j in 0..6 {
            let y = 1.2 + j as f32 * 3.0;
            block(m, &g, vec3(-hw + 0.3, y, -0.95), vec3(hw - 0.3, y + 0.7, -0.5), shade(GATE, 0.8));
        }
        for xs in [-4.5f32, 0.0, 4.5] {
            block(m, &g, vec3(xs - 0.25, 0.4, -0.9), vec3(xs + 0.25, SILL - 1.0, -0.5), shade(GATE, 0.85));
        }
        // Rust weeping from the bolted seams.
        for j in 0..3u32 {
            let xs = -5.5 + 5.5 * j as f32 + (hash01(k as u32, j) - 0.5) * 2.0;
            let top = 4.0 + 10.0 * hash01(k as u32 + 3, j);
            block(m, &g, vec3(xs - 0.35, top - 6.0, -0.98), vec3(xs + 0.35, top, -0.93), GATE_RUST);
        }
        stencil(
            m,
            &(g * Mat4::from_translation(vec3(0.0, SILL - 3.6, -0.99)) * Mat4::from_rotation_y(PI)),
            &format!("GATE {}", BAYS - k),
            0.22,
            YELLOW,
        );
        // The road deck behind the gates.
        m.cuboid(vec3(x - BAY * 0.5, h - 0.8, Z_CREST + 5.5), vec3(x + BAY * 0.5, h, Z_UP), CONCRETE_DARK);
    }
    for z in [Z_CREST + 5.6, Z_UP - 0.2] {
        m.cuboid(vec3(-half, h, z - 0.15), vec3(half, h + 1.1, z + 0.15), STEEL);
    }
    // Hoist bridge: two deep girders on the towers, hoist houses above each gate.
    let top = h + BRIDGE;
    for z in [Z_CREST + 0.9, Z_CREST + 5.1] {
        girder(m, vec3(-half - 2.0, top + 1.2, z), vec3(half + 2.0, top + 1.2, z), 0.9, 2.6, STEEL);
    }
    m.cuboid(vec3(-half - 2.0, top + 2.5, Z_CREST + 0.4), vec3(half + 2.0, top + 2.8, Z_CREST + 5.6), shade(STEEL, 0.8));
    for k in 0..BAYS {
        let x = bay_x(k);
        m.cuboid(vec3(x - 3.2, top + 2.8, Z_CREST + 1.0), vec3(x + 3.2, top + 6.0, Z_CREST + 5.0), YELLOW);
        m.cuboid(vec3(x - 3.4, top + 6.0, Z_CREST + 0.8), vec3(x + 3.4, top + 6.3, Z_CREST + 5.2), STEEL);
        for s in [-1.5f32, 1.5] {
            beam(m, vec3(x + s, top, Z_CREST + 2.0), vec3(x + s, h + 1.0, Z_CREST + 2.0), 0.12, 0.12, DARK);
        }
    }
    // Gantry rails on the bridge deck.
    for z in [Z_CREST + 1.0, Z_CREST + 5.0] {
        m.cuboid(vec3(-half - 2.0, top + 2.8, z - 0.15), vec3(half + 2.0, top + 3.1, z + 0.15), DARK);
    }
    let sign = wall(vec3(0.0, top - 2.2, Z_CREST + 0.38), PI);
    m.obox(&sign, vec3(0.0, 0.75, -0.05), vec3(8.0, 1.2, 0.05), DARK);
    stencil(m, &sign, "SPILLWAY NO 2", 0.22, shade(CONCRETE, 1.2));
}

fn powerhouse(m: &mut Mesh, w: &World, side: f32) {
    let (h, b) = (crest(w), base(w));
    let (x0, x1) = if side > 0.0 { (60.0, 100.0) } else { (-100.0, -60.0) };
    let (lo, _) = ground_range(w, vec3((x0 + x1) * 0.5, 0.0, 109.0), 20.0);
    let roof = b + 16.0;
    m.cuboid(vec3(x0, lo - 2.0, 104.5), vec3(x1, roof, face_z(w, roof) + 1.0), jit(CONCRETE, 90 + side as u32));
    m.cuboid(vec3(x0 - 0.3, roof, 104.2), vec3(x1 + 0.3, roof + 0.7, face_z(w, roof + 0.7) + 1.0), CONCRETE_DARK);
    // Tall windows, lit faintly from the turbine hall inside.
    let n = 7;
    for k in 0..n {
        let x = x0 + 3.0 + k as f32 * (x1 - x0 - 6.0) / (n - 1) as f32;
        m.cuboid(vec3(x - 1.5, b + 4.0, 104.42), vec3(x + 1.5, b + 13.0, 104.5), GLASS);
        for y in [b + 7.0, b + 10.0] {
            m.cuboid(vec3(x - 1.5, y, 104.38), vec3(x + 1.5, y + 0.2, 104.45), STEEL);
        }
        m.cuboid(vec3(x - 1.9, b + 3.6, 104.3), vec3(x + 1.9, b + 4.0, 104.5), CONCRETE_DARK);
    }
    let sign = wall(vec3((x0 + x1) * 0.5, b + 13.6, 104.45), PI);
    stencil(m, &sign, if side > 0.0 { "UNIT 2" } else { "UNIT 1" }, 0.3, CONCRETE_DARK);
    // Penstocks from the intakes high on the face down into the roof.
    for k in 0..3 {
        let x = x0 + 8.0 + k as f32 * 12.0;
        let y_in = h - 22.0;
        let a = vec3(x, y_in, face_z(w, y_in) - 1.6);
        let c = vec3(x, roof + 0.5, face_z(w, roof) - 3.0);
        drum(m, a, c, 1.9, 7, jit(GATE_RUST, k));
        for t in [0.25f32, 0.5, 0.75] {
            let p = a.lerp(c, t);
            let d = (c - a).normalize() * 0.25;
            drum(m, p - d, p + d, 2.2, 7, STEEL);
        }
        m.cuboid(vec3(x - 3.0, y_in - 3.0, face_z(w, y_in) - 2.2), vec3(x + 3.0, y_in + 3.5, face_z(w, y_in) + 1.0), CONCRETE_DARK);
    }
    // Transformers on the roof.
    for k in 0..3 {
        let x = x0 + 7.0 + k as f32 * 12.0;
        m.cuboid(vec3(x - 2.0, roof + 0.7, 105.5), vec3(x + 2.0, roof + 4.0, 108.5), [0.32, 0.36, 0.34, 0.0]);
        for dx in [-1.2f32, 0.0, 1.2] {
            bar(m, vec3(x + dx, roof + 4.0, 107.0), vec3(x + dx, roof + 5.6, 107.0), 0.25, [0.55, 0.40, 0.30, 0.0]);
        }
    }
}

/// Faceted rock buttresses the dam ends bite into, rising above the crest.
fn rock_shoulder(m: &mut Mesh, w: &World, side: f32) {
    const NX: usize = 16;
    const NZ: usize = 12;
    let (xa, xb, za, zb) = (100.0f32, 210.0f32, 104.0f32, 168.0f32);
    let at = |i: usize, j: usize| {
        let x = side * (xa + (xb - xa) * i as f32 / NX as f32);
        let z = za + (zb - za) * j as f32 / NZ as f32;
        let lift = shoulder(w, x, z);
        vec3(x, w.height(x, z) + if lift > 0.05 { lift } else { -1.0 }, z)
    };
    for i in 0..NX {
        for j in 0..NZ {
            let (p00, p10, p01, p11) = (at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1));
            let k = (i * NZ + j) as u32;
            // Ledges hold moss; steep faces show banded rock.
            let tint = |a: Vec3, b: Vec3, c: Vec3, k: u32| {
                let n = (b - a).cross(c - a).normalize_or_zero();
                let y = (a.y + b.y + c.y) / 3.0;
                let band = if ((y - crest(w)) / 5.0).floor() as i32 % 2 == 0 { 1.0 } else { 0.9 };
                let rock = shade(ROCK, band);
                let col = if n.y.abs() > 0.82 { [0.38, 0.44, 0.33, 0.0] } else { rock };
                jit(col, k)
            };
            let (a, b) = if side > 0.0 { ([p00, p01, p10], [p11, p10, p01]) } else { ([p00, p10, p01], [p11, p01, p10]) };
            m.tri(a[0], a[1], a[2], tint(a[0], a[1], a[2], k * 2));
            m.tri(b[0], b[1], b[2], tint(b[0], b[1], b[2], k * 2 + 1));
        }
    }
}

/// Seconds into a siren test, if one is running.
fn siren(w: &World, time: f32) -> Option<f32> {
    let cycle = (time / SIREN_PERIOD).floor() as u32;
    let phase = time.rem_euclid(SIREN_PERIOD);
    (roll(w, cycle, 0x5124) < 0.5 && phase >= SIREN.0 && phase < SIREN.1).then_some(phase - SIREN.0)
}

pub fn atmosphere(m: &mut Mesh, alpha: &mut Mesh, add: &mut Mesh, w: &World, time: f32) {
    let (h, b) = (crest(w), base(w));
    let profile = chute(w);
    // Water weeping from the bottom seals of every gate runs down the chute
    // in thin, pulsing threads, and spits from the side seals.
    for k in 0..BAYS {
        let x0 = bay_x(k);
        for s in 0..3u32 {
            let x = x0 + (hash01(k as u32 * 3 + s, 0xA1) - 0.5) * (BAY - 3.0);
            let wide = 0.25 + 0.35 * hash01(k as u32 * 3 + s, 0xA2);
            let speed = 5.0 + 3.0 * hash01(k as u32 * 3 + s, 0xA3);
            let mut along = 0.0;
            for pair in profile.windows(2) {
                let ((za, ya), (zb, yb)) = (pair[0], pair[1]);
                let len = ((zb - za).powi(2) + (yb - ya).powi(2)).sqrt();
                let flow = |d: f32| 0.16 + 0.22 * (0.5 + 0.5 * (d * 0.35 - time * speed).sin());
                let (ca, cb) = ([0.78, 0.88, 0.92, flow(along)], [0.78, 0.88, 0.92, flow(along + len)]);
                let (wa, wb) = (wide * (1.0 + along * 0.012), wide * (1.0 + (along + len) * 0.012));
                alpha.quad_c(
                    vec3(x + wa, ya + 0.12, za - 0.1),
                    vec3(x - wa, ya + 0.12, za - 0.1),
                    vec3(x - wb, yb + 0.12, zb - 0.1),
                    vec3(x + wb, yb + 0.12, zb - 0.1),
                    [ca, ca, cb, cb],
                );
                along += len;
            }
        }
        // Side-seal jets arc out and fall onto the chute.
        for s in [-1.0f32, 1.0] {
            let start = vec3(x0 + s * (BAY * 0.5 - 0.3), h - SILL + 2.0 + 6.0 * hash01(k as u32, 0xA4 + s as u32), Z_CREST + 1.4);
            let mut prev = start;
            for j in 1..=5 {
                let t = j as f32 * 0.25;
                let p = start + vec3(-s * 0.6 * t, -4.9 * t * t, -2.2 * t);
                let a = 0.32 * (0.7 + 0.3 * (time * 9.0 + j as f32 + k as f32).sin());
                alpha.quad_c(prev + Vec3::X * 0.12, prev - Vec3::X * 0.12, p - Vec3::X * 0.2, p + Vec3::X * 0.2, [[0.82, 0.9, 0.95, a]; 4]);
                prev = p;
            }
        }
    }
    // A shallow pool in the stilling basin and spray rising off it.
    let half = spill_half();
    alpha.quad(
        vec3(half, b + 1.7, 104.2),
        vec3(-half, b + 1.7, 104.2),
        vec3(-half, b + 1.7, Z_TOE - 0.5),
        vec3(half, b + 1.7, Z_TOE - 0.5),
        [0.36, 0.46, 0.48, 0.55],
    );
    let seed = (w.seed ^ (w.seed >> 32)) as u32;
    for k in 0..12u32 {
        let age = (time * 0.12 + hash01(k, seed ^ 0xA7)).fract();
        let x = (hash01(k, 0xA8) - 0.5) * half * 1.8;
        let p = vec3(x, b + 2.5 + age * 9.0, Z_TOE - 1.0 - age * 1.5);
        soft(alpha, p, 1.6 + age * 3.5, 1.5 + age * 3.5, [0.86, 0.9, 0.92, 0.2 * (1.0 - age)]);
    }
    // The maintenance gantry crawls along the hoist bridge, its hook swaying.
    let top = h + BRIDGE + 3.1;
    let gx = (time * 0.021 + roll(w, 3, 0x6A7) * TAU).sin() * (half - 6.0);
    for z in [Z_CREST - 2.5, Z_CREST + 8.5] {
        for s in [-1.0f32, 1.0] {
            bar(m, vec3(gx + s * 4.0, top, z.clamp(Z_CREST + 1.0, Z_CREST + 5.0)), vec3(gx + s * 4.0, top + 6.0, z), 0.55, YELLOW);
        }
    }
    for s in [-1.0f32, 1.0] {
        girder(m, vec3(gx + s * 4.0, top + 6.6, Z_CREST - 3.5), vec3(gx + s * 4.0, top + 6.6, Z_CREST + 9.5), 0.7, 1.4, YELLOW);
    }
    let trolley_z = Z_CREST + 3.0 - 5.5 * (0.5 + 0.5 * (time * 0.05).sin());
    m.cuboid(vec3(gx - 4.6, top + 7.3, trolley_z - 1.6), vec3(gx + 4.6, top + 9.2, trolley_z + 1.6), shade(YELLOW, 0.85));
    let swing = (time * 0.9).sin() * 0.35;
    let hook = vec3(gx + swing, h + 6.0, trolley_z - 0.2);
    beam(m, vec3(gx, top + 7.3, trolley_z), hook, 0.08, 0.08, DARK);
    m.cuboid(hook - vec3(0.4, 0.9, 0.3), hook + vec3(0.4, 0.0, 0.3), YELLOW);
    // Warning lamps: red on the hoist bridge always; during a siren test
    // the amber beacons on the siren masts flash and turn.
    let blink = (time / 1.8).fract() < 0.25;
    for x in [-half - 2.0, half + 2.0] {
        soft(add, vec3(x, top + 1.0, Z_CREST + 0.2), 1.4, 1.4, [1.0, 0.12, 0.05, if blink { 0.8 } else { 0.1 }]);
    }
    let test = siren(w, time);
    for x in [-70.0f32, 70.0] {
        let p = vec3(x, h + 14.2, Z_CREST + 3.0);
        let lamp = if test.is_some() { [1.0, 0.62, 0.1, 1.0] } else { [0.35, 0.24, 0.08, 0.0] };
        m.cuboid(p - vec3(0.55, 0.5, 0.55), p + vec3(0.55, 0.5, 0.55), lamp);
        if let Some(t) = test {
            // A rotating beacon: brightest when its lens faces the field.
            let a = t * 4.2;
            let toward = 0.5 + 0.5 * (-a.cos());
            let fade = ease(0.0, 0.6, t) * (1.0 - ease(SIREN.1 - SIREN.0 - 0.8, SIREN.1 - SIREN.0, t));
            soft(add, p + vec3(a.sin(), 0.0, -a.cos()) * 0.9, 2.5, 2.5, [1.0, 0.6, 0.1, (0.25 + 0.75 * toward) * fade]);
            soft(add, p, 9.0, 6.0, [1.0, 0.5, 0.08, 0.22 * toward * fade]);
        }
    }
    // The turbine hall hums: its status lights pulse slowly behind the glass.
    for side in [-1.0f32, 1.0] {
        for k in 0..3 {
            let x = side * (66.0 + k as f32 * 13.0);
            let pulse = 0.5 + 0.5 * (time * 1.3 + k as f32 * 2.1 + side).sin();
            soft(add, vec3(x, b + 6.0, 104.3), 0.9, 0.9, [0.4, 1.0, 0.6, 0.25 + 0.3 * pulse]);
        }
    }
}

/// A siren test starts in about half of the 90-second windows.
pub fn cues(w: &World, from: f32, to: f32, out: &mut Vec<(f32, Cue, Vec3)>) {
    for t in ticks(from, to, SIREN_PERIOD, SIREN.0, |k| roll(w, k, 0x5124) < 0.5) {
        out.push((t, Cue::Siren, vec3(0.0, crest(w) + 12.0, Z_CREST + 3.0)));
    }
}
