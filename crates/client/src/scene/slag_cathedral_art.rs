//! Slag Cathedral, the art pass.
//!
//! Works No. 6 cast bells. Its sixth great bell cracked as it cooled and was
//! left standing in the south-west yard; the seventh never left its mould in
//! the north-east. The furnaces cannot be let out, so the works still runs:
//! metal is tapped into overhead launders and falls into the floor channels,
//! the steam organ on the nave walls sounds to nobody, the control cabins are
//! lit, and somebody still tends a lantern at the foot of the cracked bell.
//!
//! Everything here is dressing. Anything a player can touch is either flush
//! with the floor, well overhead, or skinned onto authoritative collision
//! read from the shared layout.
use super::*;
use ferrite_shared::map::{BASES_XZ, BASE_SPAWN_RADIUS, SPREAD};

/// The yards beyond the nave were first laid out closer in; each moved out
/// with the spread, keeping its shape. These are their first-authored centres
/// carried to where the shared layout now builds them.
/// The west stockyard gantry (over objective A).
pub(super) const STOCKYARD: (f32, f32) = (-78.0 + layout::STOCKYARD_SHIFT.0, -18.0 + layout::STOCKYARD_SHIFT.1);
/// The west boiler court (objective B).
pub(super) const COURT: (f32, f32) = (-40.0 + layout::COURT_SHIFT.0, 22.0 + layout::COURT_SHIFT.1);
/// The west gate wall behind the spawn.
pub(super) const GATE_WALL_X: f32 = -115.5 + layout::BASE_SHIFT.0;
/// The ladle railway never runs into the spawn ground.
pub(super) const RAIL_LIMIT: f32 = -(BASES_XZ[0].0 + BASE_SPAWN_RADIUS + 2.0);

const BRONZE: Col = [0.47, 0.33, 0.16, 0.0];
const NEW_BRONZE: Col = [0.80, 0.56, 0.25, 0.12];
const VERDIGRIS: Col = [0.24, 0.41, 0.36, 0.0];
const TIMBER: Col = [0.30, 0.22, 0.15, 0.0];
const SAND: Col = [0.085, 0.08, 0.085, 0.0];
const CLAY: Col = [0.34, 0.285, 0.25, 0.0];
const LAMP: Col = [1.0, 0.78, 0.46, 0.92];
const COOL_LAMP: Col = [0.72, 0.90, 0.95, 0.9];
const PAINT: Col = [0.62, 0.44, 0.15, 0.0];
const WHITEWASH: Col = [0.70, 0.68, 0.62, 0.0];
const CHALK: Col = [0.80, 0.80, 0.76, 0.3];
const SHADOW: Col = [0.05, 0.045, 0.05, 0.0];
/// (radius, height) of the great bells, lip to crown.
const BELL_PROFILE: [(f32, f32); 9] = [
    (2.12, 0.25),
    (2.02, 0.6),
    (1.78, 1.6),
    (1.6, 2.7),
    (1.5, 3.7),
    (1.34, 4.2),
    (0.95, 4.55),
    (0.45, 4.72),
    (0.0, 4.76),
];

/// The small bells are squatter than the great ones, so they still read as
/// bells at arm's length.
const HAND_BELL: [(f32, f32); 8] = [
    (0.31, 0.0),
    (0.285, 0.05),
    (0.21, 0.2),
    (0.175, 0.34),
    (0.155, 0.42),
    (0.1, 0.475),
    (0.045, 0.5),
    (0.0, 0.505),
];

pub(crate) fn mixc(a: Col, b: Col, t: f32) -> Col {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}
/// Axis-aligned box from any two opposite corners (mirrored coordinates welcome).
pub(crate) fn cube(m: &mut Mesh, a: Vec3, b: Vec3, col: Col) {
    m.cuboid(a.min(b), a.max(b), col);
}
/// Upward-facing rectangle.
pub(crate) fn flat(m: &mut Mesh, x0: f32, z0: f32, x1: f32, z1: f32, y: f32, col: Col) {
    m.quad(
        vec3(x0, y, z0),
        vec3(x0, y, z1),
        vec3(x1, y, z1),
        vec3(x1, y, z0),
        col,
    );
}
/// Rectangle in the XY plane of `g`, facing its +Z.
pub(crate) fn panel(m: &mut Mesh, g: &Mat4, c: Vec3, hw: f32, hh: f32, col: Col) {
    let p = |x: f32, y: f32| g.transform_point3(c + vec3(x, y, 0.0));
    m.quad(p(-hw, -hh), p(hw, -hh), p(hw, hh), p(-hw, hh), col);
}
/// Painted lettering, two triangles a pixel: centred on `g`, baseline at y = 0.
pub(crate) fn stencil(m: &mut Mesh, g: &Mat4, text: &str, pixel: f32, col: Col) {
    let width = text.len() as f32 * 6.0 * pixel - pixel;
    for (i, c) in text.chars().enumerate() {
        for (row, bits) in glyph(c).into_iter().enumerate() {
            for bit in 0..5 {
                if bits & (1 << (4 - bit)) != 0 {
                    panel(
                        m,
                        g,
                        vec3(
                            -width * 0.5 + (i as f32 * 6.0 + bit as f32 + 0.5) * pixel,
                            (6.5 - row as f32) * pixel,
                            0.0,
                        ),
                        pixel * 0.47,
                        pixel * 0.47,
                        col,
                    );
                }
            }
        }
    }
}
/// A vertical surface at `at` whose +Z faces (sin yaw, 0, cos yaw).
pub(crate) fn wall(at: Vec3, yaw: f32) -> Mat4 {
    Mat4::from_translation(at) * Mat4::from_rotation_y(yaw)
}
/// The floor at (x, z), read by someone looking along yaw 0 = -Z, PI = +Z.
pub(crate) fn floor(x: f32, z: f32, yaw: f32) -> Mat4 {
    Mat4::from_translation(vec3(x, 0.022, z))
        * Mat4::from_rotation_y(yaw)
        * Mat4::from_rotation_x(-FRAC_PI_2)
}
/// Smooth surface of revolution about +Y through `base`.
pub(crate) fn lathe(
    m: &mut Mesh,
    base: Vec3,
    scale: f32,
    profile: &[(f32, f32)],
    segs: usize,
    col: impl Fn(usize, usize) -> Col,
) {
    for (i, pair) in profile.windows(2).enumerate() {
        let ((r0, y0), (r1, y1)) = (pair[0], pair[1]);
        let (dr, dy) = (r1 - r0, y1 - y0);
        let len = (dr * dr + dy * dy).sqrt().max(1e-5);
        let (nr, ny) = (dy / len, -dr / len);
        for k in 0..segs {
            let a0 = k as f32 / segs as f32 * TAU;
            let a1 = (k + 1) as f32 / segs as f32 * TAU;
            let (d0, d1) = (vec3(a0.cos(), 0.0, a0.sin()), vec3(a1.cos(), 0.0, a1.sin()));
            let p = |d: Vec3, r: f32, y: f32| base + (d * r + Vec3::Y * y) * scale;
            let n = |d: Vec3| (d * nr + Vec3::Y * ny).normalize_or_zero();
            m.quad_n(
                [p(d1, r0, y0), p(d0, r0, y0), p(d0, r1, y1), p(d1, r1, y1)],
                [n(d1), n(d0), n(d0), n(d1)],
                col(i, k),
            );
        }
    }
}
pub(crate) fn annulus(m: &mut Mesh, c: Vec3, r0: f32, r1: f32, n: usize, col: Col) {
    for k in 0..n {
        let a0 = k as f32 / n as f32 * TAU;
        let a1 = (k + 1) as f32 / n as f32 * TAU;
        let p = |a: f32, r: f32| c + vec3(a.cos() * r, 0.0, a.sin() * r);
        m.quad(p(a0, r0), p(a1, r0), p(a1, r1), p(a0, r1), col);
    }
}
pub(crate) fn spire(m: &mut Mesh, c: Vec3, half: f32, h: f32, col: Col) {
    let top = c + Vec3::Y * h;
    let p = [
        c + vec3(-half, 0.0, -half),
        c + vec3(half, 0.0, -half),
        c + vec3(half, 0.0, half),
        c + vec3(-half, 0.0, half),
    ];
    for k in 0..4 {
        m.tri(p[k], top, p[(k + 1) % 4], shade(col, 0.82 + 0.1 * k as f32));
    }
}
pub(crate) fn chain(m: &mut Mesh, a: Vec3, b: Vec3) {
    beam(m, a, b, 0.035, 0.035, DARK);
    let n = ((b - a).length() / 0.45).ceil().max(1.0) as usize;
    for k in 0..n {
        let p = a.lerp(b, (k as f32 + 0.5) / n as f32);
        let wide = if k % 2 == 0 {
            vec3(0.065, 0.05, 0.03)
        } else {
            vec3(0.03, 0.05, 0.065)
        };
        m.cuboid(p - wide, p + wide, shade(STEEL, 0.75));
    }
}
fn bell_radius(y: f32) -> f32 {
    for pair in BELL_PROFILE.windows(2) {
        let ((r0, y0), (r1, y1)) = (pair[0], pair[1]);
        if y <= y1 {
            return r0 + (r1 - r0) * ((y - y0) / (y1 - y0)).clamp(0.0, 1.0);
        }
    }
    0.0
}
/// A small bell `height` tall with a hanging loop on its crown.
fn hand_bell(m: &mut Mesh, foot: Vec3, height: f32, col: impl Fn(usize, usize) -> Col) {
    let scale = height / 0.5;
    let loop_col = col(6, 0);
    lathe(m, foot, scale, &HAND_BELL, 10, col);
    let g = Mat4::from_translation(foot + Vec3::Y * (0.54 * scale));
    m.tube(
        &g,
        Vec3::ZERO,
        0.06 * scale,
        0.035 * scale,
        0.018 * scale,
        8,
        loop_col,
    );
}
/// One bell of the given size, its lip resting at `foot`.
fn bell(m: &mut Mesh, foot: Vec3, scale: f32, segs: usize, col: impl Fn(usize, usize) -> Col) {
    lathe(
        m,
        foot - Vec3::Y * (0.25 * scale),
        scale,
        &BELL_PROFILE,
        segs,
        col,
    );
}

// ------------------------------------------------------------------ ground

/// Stains that say what happened where: soot round the furnaces, ore dust in
/// the stockyards, pale flux in the boiler courts, black moulding sand in the
/// bell yards and a dark ballast bed under the ladle railway.
pub(super) fn ground_tint(mut col: [f32; 3], c: Vec3) -> [f32; 3] {
    let near = |x: f32, z: f32, r0: f32, r1: f32| {
        let d = |px: f32, pz: f32| ((c.x - px).powi(2) + (c.z - pz).powi(2)).sqrt();
        1.0 - smooth(r0, r1, d(x, z).min(d(-x, -z)))
    };
    let (fx, fz) = layout::FURNACE_ANCHORS[0];
    col = mix3(col, [0.11, 0.10, 0.11], near(fx, fz, 5.0, 16.0) * 0.75);
    col = mix3(col, [0.47, 0.25, 0.15], near(STOCKYARD.0, STOCKYARD.1, 6.0, 20.0) * 0.5);
    col = mix3(col, [0.50, 0.47, 0.42], near(COURT.0, COURT.1, 5.0, 14.0) * 0.4);
    let (bx, bz) = layout::BELL_YARD;
    col = mix3(col, [0.09, 0.085, 0.09], near(bx, bz, 4.0, 14.0) * 0.82);
    // The ballast bed runs to the railway's furthest reach; where a works
    // building cut the line short, the old bed still shows beyond it.
    if c.z.abs() < 2.2 && c.x.abs() < RAIL_LIMIT + 2.0 {
        col = mix3(col, [0.17, 0.16, 0.165], 0.6);
    }
    col
}

/// Collision boxes this module draws itself: the bells and their flasks.
pub(super) fn owns(b: &Block) -> bool {
    let (bx, bz) = layout::BELL_YARD;
    let c = (b.min + b.max) * 0.5;
    let half = (b.max.x - b.min.x) * 0.5;
    c.x * c.z > 0.0
        && (c.x.abs() - bx.abs()).abs() < 0.05
        && (c.z.abs() - bz.abs()).abs() < 0.05
        && ((half - layout::BELL_FLASK_HALF).abs() < 0.05
            || (half - layout::BELL_BODY_HALF).abs() < 0.05)
}

// ------------------------------------------------------------------ static

pub(super) fn build(m: &mut Mesh, w: &World) {
    rails(m, w);
    floor_markings(m);
    for s in [1.0f32, -1.0] {
        launder(m, s);
    }
    buttresses(m);
    organ(m);
    cabins(m);
    lamps(m, w);
    hoists(m);
    crucible_tap(m);
    bell_yards(m, w);
    mould_beds(m);
    furnace_crowns(m);
    signage(m, w);
    discoveries(m);
    puddles(m, w);
    guy_wires(m);
}

/// The stretches of ladle railway in the west half, as distances out from
/// the turntable: the first runs through the nave, the last stops short of
/// the spawn ground, and any works building standing on the line breaks it.
/// The layout is a half-turn mirror, so the same runs serve the east half.
pub(super) fn rail_runs(w: &World) -> Vec<(f32, f32)> {
    let mut cuts: Vec<(f32, f32)> = w
        .blocks
        .iter()
        .filter(|b| {
            let c = (b.min + b.max) * 0.5;
            b.max.x < -layout::NAVE_HALF_X - 1.0
                && b.min.z < 1.4
                && b.max.z > -1.4
                && b.min.y < w.height(c.x, c.z) + 0.5
        })
        .map(|b| (-b.max.x - 1.0, -b.min.x + 1.0))
        .collect();
    cuts.sort_by(|p, q| p.0.total_cmp(&q.0));
    let mut runs = Vec::new();
    let mut at = 0.0f32;
    for (a, b) in cuts {
        if a.min(RAIL_LIMIT) - at > 3.0 {
            runs.push((at, a.min(RAIL_LIMIT)));
        }
        at = at.max(b);
    }
    if RAIL_LIMIT - at > 3.0 {
        runs.push((at, RAIL_LIMIT));
    }
    runs
}

/// A buffer stop at `x`, its face toward the track on the `-dir` side.
fn buffer_stop(m: &mut Mesh, x: f32, dir: f32) {
    cube(m, vec3(x, 0.012, -1.05), vec3(x + dir * 0.4, 0.14, 1.05), WARNING);
    for z in [-0.6f32, 0.0, 0.6] {
        cube(
            m,
            vec3(x - dir * 0.02, 0.03, z - 0.12),
            vec3(x + dir * 0.42, 0.145, z + 0.12),
            DARK,
        );
    }
}

/// The ladle railway: one straight line from gate to gate through both
/// portals, with a turntable beneath the crucible. Where a works building
/// was put up across it the line is stopped either side and carries on.
fn rails(m: &mut Mesh, w: &World) {
    for (a, b) in rail_runs(w) {
        // The run through the nave is one piece; the others come in mirrored pairs.
        let pieces: &[(f32, f32)] = if a == 0.0 { &[(-b, b)] } else { &[(-b, -a), (a, b)] };
        for &(x0, x1) in pieces {
            for z in [-0.72f32, 0.72] {
                m.cuboid(vec3(x0, 0.012, z - 0.09), vec3(x1, 0.03, z + 0.09), DARK);
                m.cuboid(
                    vec3(x0, 0.03, z - 0.045),
                    vec3(x1, 0.085, z + 0.045),
                    shade(EDGE, 1.05),
                );
            }
        }
        for s in [-1.0f32, 1.0] {
            buffer_stop(m, s * b, s);
            if a > 0.0 {
                buffer_stop(m, s * a, -s);
            }
        }
        // Sleepers every 1.2m, on the same pitch the whole length of the line.
        let first = (a / 1.2).ceil() as i32;
        let last = ((b - 0.3) / 1.2).floor() as i32;
        for i in first.max(0)..=last {
            let d = i as f32 * 1.2;
            if d < layout::NAVE_HALF_X + 0.6 || d < a + 0.3 {
                continue;
            }
            for (s, k) in [(-1.0f32, 2 * i as u32), (1.0, 2 * i as u32 + 1)] {
                let x = s * d;
                let j = hash01(k, 77);
                m.cuboid(
                    vec3(x - 0.13, 0.008, -1.18 - j * 0.08),
                    vec3(x + 0.13, 0.05, 1.18 + j * 0.06),
                    shade(TIMBER, 0.65 + j * 0.3),
                );
            }
        }
    }
    // Inside the nave the track is set flush in chequer plate.
    flat(
        m,
        -layout::NAVE_HALF_X,
        -0.62,
        layout::NAVE_HALF_X,
        0.62,
        0.013,
        shade(DARK, 1.15),
    );
    let c = vec3(0.0, 0.02, 0.0);
    annulus(m, c, 4.35, 4.47, 32, DARK);
    annulus(m, c + Vec3::Y * 0.002, 4.47, 4.78, 32, shade(EDGE, 0.9));
    annulus(m, c, 4.78, 4.9, 32, DARK);
    for k in 0..16 {
        let a = (k as f32 + 0.5) / 16.0 * TAU;
        let p = vec3(a.cos() * 4.62, 0.024, a.sin() * 4.62);
        m.cuboid(p - vec3(0.05, 0.0, 0.05), p + vec3(0.05, 0.02, 0.05), BRASS);
    }
}

fn floor_markings(m: &mut Mesh) {
    const BAYS: [&str; 5] = ["BAY 1", "BAY 2", "BAY 3", "BAY 4", "BAY 5"];
    for (bay, pair) in layout::NAVE_RIB_X.windows(2).enumerate() {
        let x = (pair[0] + pair[1]) * 0.5;
        stencil(m, &floor(x, 11.25, PI), BAYS[bay], 0.11, PAINT);
        stencil(m, &floor(x, -11.25, 0.0), BAYS[bay], 0.11, PAINT);
    }
    stencil(m, &floor(-21.75, 6.65, PI), "HOT METAL", 0.1, PAINT);
    stencil(m, &floor(21.75, -6.65, 0.0), "HOT METAL", 0.1, PAINT);
    stencil(m, &floor(21.75, 6.65, PI), "KEEP CLEAR", 0.1, PAINT);
    stencil(m, &floor(-21.75, -6.65, 0.0), "KEEP CLEAR", 0.1, PAINT);
    // Hazard chevrons on both approaches to every bridge.
    let hw = layout::LAVA_HALF_W;
    for z in [-layout::LAVA_Z, layout::LAVA_Z] {
        for pair in layout::LAVA_SPANS.windows(2) {
            let (a, b) = (pair[0].1, pair[1].0);
            for side in [-1.0f32, 1.0] {
                let (z0, z1) = (z + side * (hw + 0.42), z + side * (hw + 0.92));
                let n = ((b - a) / 0.6) as usize;
                for k in (0..n).step_by(2) {
                    let x = a + k as f32 * 0.6;
                    m.quad(
                        vec3(x, 0.021, z0),
                        vec3(x + 0.3, 0.021, z1),
                        vec3(x + 0.9, 0.021, z1),
                        vec3(x + 0.6, 0.021, z0),
                        PAINT,
                    );
                }
            }
        }
    }
}

/// An open refractory trough carrying molten metal from `a` down to `b`.
fn trough(m: &mut Mesh, a: Vec3, b: Vec3) {
    let (g, len) = basis(a, b);
    let h = len * 0.5;
    m.obox(&g, vec3(0.0, -0.2, 0.0), vec3(0.46, 0.05, h), DARK);
    for sx in [-1.0f32, 1.0] {
        m.obox(
            &g,
            vec3(sx * 0.42, 0.0, 0.0),
            vec3(0.06, 0.24, h),
            shade(RUST, 0.7),
        );
        m.obox(&g, vec3(sx * 0.47, 0.22, 0.0), vec3(0.09, 0.035, h), EDGE);
    }
    m.obox(&g, vec3(0.0, -0.03, 0.0), vec3(0.36, 0.015, h), HOT);
    m.obox(&g, vec3(0.0, -0.012, 0.0), vec3(0.13, 0.008, h), WHITE_HOT);
    let joints = (len / 3.2).ceil().max(1.0) as usize;
    for k in 0..=joints {
        let z = -h + len * k as f32 / joints as f32;
        m.obox(&g, vec3(0.0, -0.06, z), vec3(0.56, 0.24, 0.06), STEEL);
        m.obox(&g, vec3(0.0, -0.3, z), vec3(0.6, 0.04, 0.09), RUST);
    }
}

/// Tap, launder and the static core of a lavafall. Authored for the south
/// furnace; `s = -1` gives the half-turn mirror for the north one.
fn launder(m: &mut Mesh, s: f32) {
    let q = |x: f32, y: f32, z: f32| vec3(x * s, y, z * s);
    let (fx, fz) = layout::FURNACE_ANCHORS[0];
    let (lx, lz) = layout::LAVAFALL;
    let face = fx - layout::FURNACE_SHAFT_HALF;
    // The taphole: a hooded, white-hot mouth low on the shaft's outer face.
    cube(
        m,
        q(face - 0.55, 7.05, fz - 0.8),
        q(face, 8.35, fz + 0.8),
        DARK,
    );
    cube(
        m,
        q(face - 0.6, 7.2, fz - 0.42),
        q(face - 0.52, 7.95, fz + 0.42),
        WHITE_HOT,
    );
    cube(
        m,
        q(face - 0.75, 8.35, fz - 0.95),
        q(face + 0.02, 8.5, fz + 0.95),
        RUST,
    );
    trough(m, q(face - 0.5, 7.5, fz), q(lx + 0.5, 7.45, fz));
    // Distribution pot on a corbel off the furnace plinth.
    cube(
        m,
        q(lx - 0.8, 7.0, fz - 0.8),
        q(lx + 0.8, 7.9, fz + 0.8),
        shade(RUST, 0.6),
    );
    cube(
        m,
        q(lx - 0.62, 7.86, fz - 0.62),
        q(lx + 0.62, 7.91, fz + 0.62),
        HOT,
    );
    cube(
        m,
        q(lx - 0.3, 7.9, fz - 0.3),
        q(lx + 0.3, 7.92, fz + 0.3),
        WHITE_HOT,
    );
    for dz in [-0.6f32, 0.6] {
        beam(
            m,
            q(lx - 0.7, 7.0, fz + dz),
            q(fx - layout::FURNACE_HALF, 5.6, fz + dz),
            0.16,
            0.22,
            STEEL,
        );
    }
    let (a, b) = (q(lx, 7.4, fz + 0.7), q(lx, 6.2, lz - 0.7));
    trough(m, a, b);
    let at = |z: f32| a.y + (b.y - a.y) * ((z - (fz + 0.7)) / (lz - 0.7 - (fz + 0.7)));
    // A lintel across the wall doorway carries it into the nave.
    beam(
        m,
        q(-26.0, at(-22.0) - 0.42, -22.0),
        q(-21.0, at(-22.0) - 0.42, -22.0),
        0.34,
        0.3,
        STEEL,
    );
    // Posts off both gallery rails, then hangers from the crane rail.
    for (z, foot) in [
        (-layout::GALLERY_RAIL_OUTER_Z, layout::GALLERY_Y + 1.0),
        (-layout::GALLERY_RAIL_INNER_Z, layout::GALLERY_Y + 1.0),
    ] {
        for dx in [-0.52f32, 0.52] {
            beam(
                m,
                q(lx + dx, foot, z),
                q(lx + dx, at(z) - 0.25, z),
                0.07,
                0.07,
                EDGE,
            );
        }
    }
    for dx in [-0.52f32, 0.52] {
        beam(
            m,
            q(lx + dx, at(-10.4) + 0.2, -10.4),
            q(lx + dx, 20.6, -9.5),
            0.05,
            0.05,
            DARK,
        );
    }
    // Spout, and the solid heart of the fall into the channel.
    let lip = q(lx, 5.8, lz - 0.08);
    let (g, len) = basis(b, lip);
    m.obox(&g, vec3(0.0, -0.16, 0.0), vec3(0.34, 0.05, len * 0.5), DARK);
    for sx in [-1.0f32, 1.0] {
        m.obox(
            &g,
            vec3(sx * 0.3, 0.0, 0.0),
            vec3(0.05, 0.2, len * 0.5),
            shade(RUST, 0.7),
        );
    }
    m.obox(
        &g,
        vec3(0.0, -0.05, 0.0),
        vec3(0.24, 0.012, len * 0.5),
        WHITE_HOT,
    );
    cube(
        m,
        q(lx - 0.065, 0.03, lz - 0.065),
        q(lx + 0.065, 5.8, lz + 0.065),
        WHITE_HOT,
    );
    cube(
        m,
        q(lx - 0.12, 0.03, lz - 0.12),
        q(lx + 0.12, 2.2, lz + 0.12),
        HOT,
    );
    cube(
        m,
        q(lx - 0.6, 0.039, lz - 0.5),
        q(lx + 0.6, 0.043, lz + 0.5),
        WHITE_HOT,
    );
}

/// Lattice flying buttresses from the clerestory down to the cloister roofs.
fn buttresses(m: &mut Mesh) {
    for &x in &layout::NAVE_RIB_X {
        for side in [-1.0f32, 1.0] {
            if layout::FURNACE_ANCHORS
                .iter()
                .any(|&(fx, fz)| (fx - x).abs() < 7.0 && fz * side > 0.0)
            {
                continue;
            }
            let zw = side * (layout::NAVE_HALF_Z + 0.72);
            let zf = side * (layout::SERVICE_Z - layout::SERVICE_HALF_Z + 0.9);
            let (top, foot) = (vec3(x, 21.6, zw), vec3(x, layout::SERVICE_ROOF_Y + 0.7, zf));
            girder(m, top, foot, 0.5, 0.85, STEEL);
            let (a0, ctrl, a1) = (
                vec3(x, 12.6, zw),
                vec3(x, 6.3, side * 27.5),
                foot - Vec3::Y * 0.5,
            );
            let arc =
                |t: f32| a0 * ((1.0 - t) * (1.0 - t)) + ctrl * (2.0 * t * (1.0 - t)) + a1 * (t * t);
            for k in 0..6 {
                beam(
                    m,
                    arc(k as f32 / 6.0),
                    arc((k + 1) as f32 / 6.0),
                    0.26,
                    0.34,
                    RUST,
                );
            }
            for k in 1..6 {
                let t = k as f32 / 6.0;
                beam(m, arc(t), top.lerp(foot, t * 0.92), 0.11, 0.14, EDGE);
            }
            cube(
                m,
                vec3(x - 0.42, 12.0, zw - side * 0.1),
                vec3(x + 0.42, 22.3, zw + side * 0.1),
                shade(RUST, 0.8),
            );
            // A pinnacle where the thrust lands on the cloister roof.
            let base = vec3(x, layout::SERVICE_ROOF_Y, zf + side * 0.45);
            m.cuboid(
                base - vec3(0.5, 0.0, 0.5),
                base + vec3(0.5, 2.5, 0.5),
                shade(STEEL, 0.95),
            );
            m.cuboid(
                base + vec3(-0.58, 2.5, -0.58),
                base + vec3(0.58, 2.7, 0.58),
                RUST,
            );
            spire(m, base + Vec3::Y * 2.7, 0.5, 2.6, EDGE);
            m.cuboid(
                base + vec3(-0.07, 5.2, -0.07),
                base + vec3(0.07, 5.5, 0.07),
                BRASS,
            );
        }
    }
}

/// Where organ pipe `i` of a rank stands: (x, z, top y, radius).
fn organ_pipe(side: f32, flank: f32, i: usize) -> (f32, f32, f32, f32) {
    let f = i as f32;
    (
        flank * (3.05 + f * 0.4),
        side * 21.05,
        20.6 - f * 1.3 - f * f * 0.05,
        0.19 - f * 0.014,
    )
}
const ORGAN_FOOT: f32 = 10.05;

/// The steam organ: four ranks of graded pipes flanking the central window
/// on each wall, standing on wind chests fed from the wall mains.
fn organ(m: &mut Mesh) {
    for side in [-1.0f32, 1.0] {
        for flank in [-1.0f32, 1.0] {
            for i in 0..7 {
                let (x, z, top, r) = organ_pipe(side, flank, i);
                let g = Mat4::from_translation(vec3(x, (top + ORGAN_FOOT) * 0.5, z))
                    * Mat4::from_rotation_x(FRAC_PI_2);
                let col = if i % 2 == 0 {
                    shade(BRASS, 0.9)
                } else {
                    shade(EDGE, 1.1)
                };
                m.tube(
                    &g,
                    Vec3::ZERO,
                    r,
                    r * 0.72,
                    (top - ORGAN_FOOT) * 0.5,
                    8,
                    col,
                );
                // The mouth, cut toward the nave.
                let zm = z - side * (r + 0.004);
                cube(
                    m,
                    vec3(x - r * 0.7, 10.72, zm),
                    vec3(x + r * 0.7, 10.95, zm - side * 0.012),
                    SHADOW,
                );
                cube(
                    m,
                    vec3(x - r * 0.8, 10.95, zm),
                    vec3(x + r * 0.8, 11.0, zm - side * 0.03),
                    BRASS,
                );
            }
            let (x0, x1) = (flank * 2.75, flank * 5.8);
            cube(
                m,
                vec3(x0, 9.3, side * 20.62),
                vec3(x1, ORGAN_FOOT, side * 21.32),
                shade(RUST, 0.72),
            );
            cube(
                m,
                vec3(x0, 9.62, side * 20.6),
                vec3(x1, 9.72, side * 20.62),
                BRASS,
            );
            pipe(
                m,
                vec3(flank * 4.25, 8.35, side * 21.35),
                vec3(flank * 4.25, 9.3, side * 21.0),
                0.15,
                RUST,
            );
        }
        // One word, cast in brass, under the window between the ranks.
        let g = wall(
            vec3(0.0, 9.42, side * 21.26),
            if side > 0.0 { PI } else { 0.0 },
        );
        m.obox(
            &g,
            vec3(0.0, 0.2, -0.03),
            vec3(1.25, 0.32, 0.03),
            shade(BRASS, 0.75),
        );
        stencil(m, &g, "CHOIR", 0.06, SHADOW);
    }
}

/// Lit control cabins hung round a rib column above each gallery.
fn cabins(m: &mut Mesh) {
    for (cx, side, label, cat_inside) in [
        (-18.0f32, 1.0f32, "BAY 2 CONTROL", true),
        (18.0, -1.0, "BAY 4 CONTROL", false),
    ] {
        let (zf, zb) = (side * 15.9, side * 20.9);
        let (y0, y1) = (5.9f32, 8.4f32);
        cube(
            m,
            vec3(cx - 2.7, y0, zf),
            vec3(cx + 2.7, y0 + 0.15, zb),
            DARK,
        );
        cube(
            m,
            vec3(cx - 2.86, y1 - 0.1, zf - side * 0.18),
            vec3(cx + 2.86, y1 + 0.06, zb),
            shade(RUST, 0.75),
        );
        // Solid dado and fascia with a continuous glazed band between.
        for (lo, hi) in [(y0 + 0.15, 6.9f32), (8.0, y1 - 0.1)] {
            cube(
                m,
                vec3(cx - 2.7, lo, zf),
                vec3(cx + 2.7, hi, zf + side * 0.08),
                STEEL,
            );
            for e in [-1.0f32, 1.0] {
                cube(
                    m,
                    vec3(cx + e * 2.7, lo, zf),
                    vec3(cx + e * 2.62, hi, zb),
                    STEEL,
                );
            }
        }
        cube(
            m,
            vec3(cx - 2.7, y0 + 0.15, zb),
            vec3(cx + 2.7, y1 - 0.1, zb - side * 0.08),
            shade(STEEL, 0.85),
        );
        cube(
            m,
            vec3(cx - 2.64, 6.9, zf + side * 0.03),
            vec3(cx + 2.64, 8.0, zf + side * 0.05),
            LAMP,
        );
        for e in [-1.0f32, 1.0] {
            cube(
                m,
                vec3(cx + e * 2.67, 6.9, zf + side * 0.06),
                vec3(cx + e * 2.65, 8.0, zb - side * 0.1),
                LAMP,
            );
            for k in 0..=4 {
                let z = zf + (zb - zf) * k as f32 / 4.0;
                cube(
                    m,
                    vec3(cx + e * 2.71, 6.9, z - 0.04),
                    vec3(cx + e * 2.63, 8.0, z + 0.04),
                    DARK,
                );
            }
        }
        for k in 0..=5 {
            let x = cx - 2.7 + 5.4 * k as f32 / 5.0;
            cube(
                m,
                vec3(x - 0.045, 6.9, zf - side * 0.01),
                vec3(x + 0.045, 8.0, zf + side * 0.07),
                DARK,
            );
        }
        // What the lit glass shows from the floor: a coat and hat on a hook,
        // a desk lamp and, in one cabin, the works cat.
        let zs = zf - side * 0.006;
        let sil = |m: &mut Mesh, x0: f32, y0: f32, x1: f32, y1: f32| {
            cube(
                m,
                vec3(cx + x0, y0, zs),
                vec3(cx + x1, y1, zs + side * 0.02),
                SHADOW,
            );
        };
        sil(m, 1.2, 6.95, 1.56, 7.7);
        sil(m, 1.14, 7.5, 1.62, 7.62);
        sil(m, 1.22, 7.74, 1.54, 7.8);
        sil(m, 1.27, 7.8, 1.49, 7.9);
        sil(m, -1.52, 6.9, -1.46, 7.38);
        sil(m, -1.7, 7.38, -1.4, 7.47);
        if cat_inside {
            sil(m, -0.36, 6.9, -0.06, 7.16);
            sil(m, -0.3, 7.16, -0.1, 7.33);
            sil(m, -0.3, 7.33, -0.25, 7.4);
            sil(m, -0.15, 7.33, -0.1, 7.4);
            sil(m, -0.06, 6.9, 0.2, 6.95);
        }
        for e in [-1.0f32, 1.0] {
            beam(
                m,
                vec3(cx + e * 2.4, y0, side * 16.4),
                vec3(cx + e * 0.8, 4.5, side * 17.5),
                0.14,
                0.2,
                STEEL,
            );
        }
        cube(
            m,
            vec3(cx - 0.35, y0 - 0.08, side * 16.3),
            vec3(cx + 0.35, y0, side * 16.7),
            LAMP,
        );
        // Access ladder from the gallery, outside its rail.
        for z in [side * 17.3, side * 17.68] {
            beam(
                m,
                vec3(cx - 2.95, layout::GALLERY_Y, z),
                vec3(cx - 2.95, 6.5, z),
                0.05,
                0.05,
                EDGE,
            );
        }
        for k in 0..9 {
            let y = layout::GALLERY_Y + 0.25 + k as f32 * 0.3;
            beam(
                m,
                vec3(cx - 2.95, y, side * 17.3),
                vec3(cx - 2.95, y, side * 17.68),
                0.035,
                0.035,
                STEEL,
            );
        }
        let g = wall(
            vec3(cx, 8.07, zf - side * 0.012),
            if side > 0.0 { PI } else { 0.0 },
        );
        stencil(m, &g, label, 0.028, [0.92, 0.86, 0.7, 0.55]);
    }
}

fn lamps(m: &mut Mesh, w: &World) {
    // High-bay pendants above the crane's travel.
    for &x in &layout::NAVE_RIB_X {
        for z in [-6.5f32, 6.5] {
            beam(m, vec3(x, 28.9, z), vec3(x, 23.5, z), 0.05, 0.05, DARK);
            lathe(
                m,
                vec3(x, 23.22, z),
                1.0,
                &[(0.58, 0.0), (0.2, 0.22), (0.1, 0.3)],
                8,
                |_, k| shade(DARK, 1.0 + (k % 2) as f32 * 0.25),
            );
            annulus(m, vec3(x, 23.235, z), 0.0, 0.46, 8, LAMP);
        }
    }
    // Caged bulkhead lamps on every column, and strips under the galleries.
    for &x in &layout::NAVE_RIB_X {
        for side in [-1.0f32, 1.0] {
            let z = side * (layout::NAVE_RIB_Z - layout::RIB_COLUMN_HALF_Z);
            cube(
                m,
                vec3(x - 0.13, 2.45, z),
                vec3(x + 0.13, 2.82, z - side * 0.1),
                LAMP,
            );
            cube(
                m,
                vec3(x - 0.2, 2.82, z),
                vec3(x + 0.2, 2.88, z - side * 0.17),
                DARK,
            );
            cube(
                m,
                vec3(x - 0.2, 2.4, z),
                vec3(x + 0.2, 2.45, z - side * 0.13),
                DARK,
            );
            cube(
                m,
                vec3(x - 0.025, 2.88, z),
                vec3(x + 0.025, 3.6, z - side * 0.03),
                DARK,
            );
        }
    }
    for side in [-1.0f32, 1.0] {
        for k in 0..8 {
            let x = -21.0 + k as f32 * 6.0;
            let z = side * layout::GALLERY_Z;
            m.cuboid(
                vec3(x - 0.45, layout::GALLERY_BOTTOM - 0.06, z - 0.06),
                vec3(x + 0.45, layout::GALLERY_BOTTOM, z + 0.06),
                shade(LAMP, 0.92),
            );
        }
    }
    // Cold-white yard floods: pipe racks, stockyard gantries, bell gantries.
    for x in layout::PIPE_YARD_X {
        for s in [1.0f32, -1.0] {
            let (px, pz) = (x * s, layout::PIPE_YARD_Z * s);
            let g = w.height(px, pz) + layout::PIPE_RACK_Y;
            for dz in [-2.05f32, 2.05] {
                m.cuboid(
                    vec3(px - 0.3, g - 0.16, pz + dz - 0.2),
                    vec3(px + 0.3, g - 0.04, pz + dz + 0.2),
                    COOL_LAMP,
                );
                m.cuboid(
                    vec3(px - 0.36, g - 0.04, pz + dz - 0.26),
                    vec3(px + 0.36, g, pz + dz + 0.26),
                    DARK,
                );
            }
        }
    }
    for s in [1.0f32, -1.0] {
        let (gx, gz) = (STOCKYARD.0 * s, STOCKYARD.1 * s);
        let g = w.height(gx, gz) + 6.0;
        for dx in [-4.4f32, 0.0, 4.4] {
            m.cuboid(
                vec3(gx + dx - 0.35, g - 0.12, gz - 0.2),
                vec3(gx + dx + 0.35, g - 0.02, gz + 0.2),
                COOL_LAMP,
            );
        }
        let (bx, bz) = (layout::BELL_YARD.0 * s, layout::BELL_YARD.1 * s);
        let g = w.height(bx, bz) + layout::BELL_GANTRY_Y;
        for dx in [-2.4f32, 2.4] {
            m.cuboid(
                vec3(bx + dx - 0.3, g - 0.12, bz - 0.22),
                vec3(bx + dx + 0.3, g - 0.02, bz + 0.22),
                COOL_LAMP,
            );
        }
    }
}

/// The suspended crucible's underside: a riveted floor and the weeping tap.
fn crucible_tap(m: &mut Mesh) {
    let c = vec3(0.0, 8.7, 0.0);
    for k in 0..16 {
        let a0 = k as f32 / 16.0 * TAU;
        let a1 = (k + 1) as f32 / 16.0 * TAU;
        let p = |a: f32, r: f32| c + vec3(a.cos() * r, 0.0, a.sin() * r);
        m.tri_n(
            c,
            p(a0, 1.8),
            p(a1, 1.8),
            -Vec3::Y,
            shade(STEEL, 0.7 + 0.1 * (k % 2) as f32),
        );
        let rivet = p(a0, 1.5) - Vec3::Y * 0.02;
        m.cuboid(rivet - Vec3::splat(0.04), rivet + Vec3::splat(0.04), BRASS);
    }
    let g = Mat4::from_translation(c - Vec3::Y * 0.12) * Mat4::from_rotation_x(FRAC_PI_2);
    m.tube(&g, Vec3::ZERO, 0.42, 0.0, 0.12, 12, RUST);
    m.tube(&g, vec3(0.0, 0.0, 0.2), 0.2, 0.09, 0.1, 10, DARK);
    m.tube(&g, vec3(0.0, 0.0, 0.3), 0.09, 0.0, 0.012, 8, WHITE_HOT);
}

/// Chain hoists parked along the crane rails, hooks well above head height.
fn hoists(m: &mut Mesh) {
    for (k, (x, z)) in [
        (-14.0f32, -9.5f32),
        (11.0, -9.5),
        (25.0, -9.5),
        (14.0, 9.5),
        (-11.0, 9.5),
        (-25.0, 9.5),
    ]
    .into_iter()
    .enumerate()
    {
        let hook = 6.3 + hash01(k as u32, 91) * 3.4;
        m.cuboid(
            vec3(x - 0.45, 20.15, z - 0.32),
            vec3(x + 0.45, 20.56, z + 0.32),
            WARNING,
        );
        chain(m, vec3(x, 20.15, z), vec3(x, hook, z));
        m.cuboid(
            vec3(x - 0.16, hook - 0.36, z - 0.1),
            vec3(x + 0.16, hook, z + 0.1),
            STEEL,
        );
        let g = Mat4::from_translation(vec3(x, hook - 0.58, z));
        m.tube(&g, Vec3::ZERO, 0.21, 0.14, 0.04, 10, BRASS);
    }
}

/// The two great bells. West: No. 6, verdigrised and cracked, shored in
/// timber cribbing and still chained to its gantry. East: No. 7, never broken
/// out of its mould, the flask's seams still glowing.
fn bell_yards(m: &mut Mesh, w: &World) {
    let (yx, yz) = layout::BELL_YARD;
    let fh = layout::BELL_FLASK_HALF;
    let top = layout::BELL_FLASK_H;
    for (s, cracked) in [(1.0f32, true), (-1.0, false)] {
        let c = vec3(yx * s, w.height(yx * s, yz * s), yz * s);
        if cracked {
            // Timber cribbing: corner posts and three courses with open gaps.
            for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let p = c + vec3(sx * (fh - 0.17), 0.0, sz * (fh - 0.17));
                m.cuboid(
                    p - vec3(0.17, 0.6, 0.17),
                    p + vec3(0.17, top, 0.17),
                    shade(TIMBER, 0.8),
                );
            }
            for (k, y) in [0.0f32, 0.65, 1.3].into_iter().enumerate() {
                let j = 0.85 + 0.12 * k as f32;
                for sz in [-1.0f32, 1.0] {
                    let z = c.z + sz * (fh - 0.13);
                    m.cuboid(
                        vec3(c.x - fh, c.y + y, z - 0.13),
                        vec3(c.x + fh, c.y + y + 0.3, z + 0.13),
                        shade(TIMBER, j),
                    );
                    let x = c.x + sz * (fh - 0.13);
                    m.cuboid(
                        vec3(x - 0.13, c.y + y, c.z - fh),
                        vec3(x + 0.13, c.y + y + 0.3, c.z + fh),
                        shade(TIMBER, j * 0.92),
                    );
                }
            }
            m.cuboid(c + vec3(-fh, -0.6, -fh), c + vec3(fh, 0.02, fh), SAND);
            bell(m, c, 1.0, 20, |ring, seg| {
                let j = hash01(seg as u32, ring as u32 * 3 + 11);
                let streak = hash01(seg as u32, 19);
                mixc(BRONZE, VERDIGRIS, (0.25 + 0.45 * j + 0.3 * streak).min(1.0))
            });
            // The flaw: a black, branching crack from lip to waist.
            let face = 2.03f32;
            let mut prev: Option<(Vec3, f32)> = None;
            for k in 0..9 {
                let y = 0.3 + k as f32 * 0.42;
                let a = face + (hash01(k, 31) - 0.5) * 0.16;
                let r = bell_radius(y) + 0.015;
                let p = c + vec3(a.cos() * r, y, a.sin() * r);
                let half = 0.075 * (1.0 - k as f32 / 9.5);
                if let Some((q, h0)) = prev {
                    let t = vec3(-a.sin(), 0.0, a.cos());
                    m.quad(
                        q - t * h0,
                        q + t * h0,
                        p + t * half,
                        p - t * half,
                        [0.015, 0.015, 0.02, 0.0],
                    );
                    if k == 4 {
                        let b = face + 0.2;
                        let e = c + vec3(b.cos() * (r - 0.03), y + 0.5, b.sin() * (r - 0.03));
                        m.quad(
                            q - t * 0.03,
                            q + t * 0.03,
                            e + t * 0.008,
                            e - t * 0.008,
                            [0.015, 0.015, 0.02, 0.0],
                        );
                    }
                }
                prev = Some((p, half));
            }
            // Still slung from the gantry it was lifted on.
            for e in [-1.0f32, 1.0] {
                chain(
                    m,
                    c + vec3(e * 1.25, layout::BELL_GANTRY_Y, 0.0),
                    c + vec3(e * 0.28, 5.02, 0.0),
                );
            }
            // Six failed trial casts lined up on the cribbing, and the lantern.
            for k in 0..6 {
                let p = c + vec3(-1.5 + k as f32 * 0.42, top, fh - 0.13);
                hand_bell(m, p, 0.3 + 0.03 * (k % 3) as f32, |_, seg| {
                    mixc(BRONZE, VERDIGRIS, 0.2 + 0.5 * hash01(seg as u32, k))
                });
            }
            let l = c + vec3(1.72, top, fh - 0.13);
            m.cuboid(
                l + vec3(-0.08, 0.0, -0.08),
                l + vec3(0.08, 0.03, 0.08),
                DARK,
            );
            m.cuboid(
                l + vec3(-0.055, 0.03, -0.055),
                l + vec3(0.055, 0.2, 0.055),
                LAMP,
            );
            m.cuboid(
                l + vec3(-0.09, 0.2, -0.09),
                l + vec3(0.09, 0.24, 0.09),
                DARK,
            );
            let g = wall(c + vec3(0.0, 0.72, fh + 0.012), 0.0);
            m.obox(
                &g,
                vec3(0.0, 0.1, -0.005),
                vec3(0.62, 0.2, 0.005),
                shade(BRASS, 0.8),
            );
            stencil(
                m,
                &wall(c + vec3(0.0, 0.83, fh + 0.02), 0.0),
                "NO 6",
                0.022,
                SHADOW,
            );
            stencil(
                m,
                &wall(c + vec3(0.0, 0.65, fh + 0.02), 0.0),
                "CAST FLAWED",
                0.016,
                SHADOW,
            );
        } else {
            // A banded moulding flask packed with black sand, seams still hot.
            m.cuboid(c + vec3(-fh, -0.6, -fh), c + vec3(fh, top, fh), SAND);
            for y in [0.25f32, 0.85, 1.42] {
                m.cuboid(
                    c + vec3(-fh - 0.03, y, -fh - 0.03),
                    c + vec3(fh + 0.03, y + 0.14, fh + 0.03),
                    shade(EDGE, 0.8),
                );
            }
            for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let p = c + vec3(sx * fh, 0.0, sz * fh);
                m.cuboid(
                    p - vec3(0.1, 0.0, 0.1),
                    p + vec3(0.1, top + 0.03, 0.1),
                    RUST,
                );
            }
            for face in 0..4 {
                let g = wall(c, face as f32 * FRAC_PI_2)
                    * Mat4::from_translation(vec3(0.0, 0.0, fh + 0.008));
                for k in 0..3u32 {
                    let mut x = -1.7 + k as f32 * 1.25 + hash01(k, face) * 0.5;
                    let mut y = 0.42 + hash01(k, face + 9) * 0.25;
                    for j in 0..4u32 {
                        let (nx, ny) = (
                            x + 0.12 + hash01(j, k + face * 5) * 0.2,
                            y + (hash01(j, k * 7 + face) - 0.4) * 0.3,
                        );
                        let (p0, p1) = (vec3(x, y, 0.0), vec3(nx, ny.clamp(0.42, 1.38), 0.0));
                        let t = vec3(0.0, 0.016, 0.0);
                        let tp = |v: Vec3| g.transform_point3(v);
                        m.quad(
                            tp(p0 - t),
                            tp(p1 - t),
                            tp(p1 + t),
                            tp(p0 + t),
                            if j % 2 == 0 { HOT } else { WHITE_HOT },
                        );
                        (x, y) = (nx, ny.clamp(0.42, 1.38));
                    }
                }
            }
            // The cope still clings to the bell; one slab has fallen away.
            bell(m, c, 1.04, 20, |ring, seg| {
                if (2..=3).contains(&ring) && (5..=7).contains(&seg) {
                    shade(NEW_BRONZE, 0.9 + 0.1 * (seg % 2) as f32)
                } else {
                    shade(CLAY, 0.85 + 0.25 * hash01(seg as u32, ring as u32 + 40))
                }
            });
            for y in [2.0f32, 3.3] {
                let r = bell_radius(y) * 1.04 + 0.03;
                lathe(
                    m,
                    c + Vec3::Y * y,
                    1.0,
                    &[(r, 0.0), (r, 0.12)],
                    20,
                    |_, k| {
                        if (5..=7).contains(&k) {
                            [0.0; 4]
                        } else {
                            shade(EDGE, 0.85)
                        }
                    },
                );
            }
            // Pouring cups on the flask, each with a skin of hot metal.
            for dx in [-1.75f32, 1.75] {
                let p = c + vec3(dx, top, 1.75);
                lathe(m, p, 1.0, &[(0.2, 0.0), (0.3, 0.3)], 10, |_, _| {
                    shade(CLAY, 0.8)
                });
                annulus(m, p + Vec3::Y * 0.24, 0.0, 0.26, 10, HOT);
            }
            for e in [-1.0f32, 1.0] {
                let x = c.x + e * 1.25;
                chain(
                    m,
                    vec3(x, c.y + layout::BELL_GANTRY_Y, c.z),
                    vec3(x, c.y + 6.3, c.z),
                );
                let g = Mat4::from_translation(vec3(x, c.y + 6.08, c.z));
                m.tube(&g, Vec3::ZERO, 0.22, 0.15, 0.045, 10, BRASS);
            }
            let g = wall(c + vec3(0.0, 0.5, -fh - 0.012), PI);
            m.obox(
                &g,
                vec3(0.0, 0.1, -0.005),
                vec3(0.62, 0.2, 0.005),
                shade(BRASS, 0.9),
            );
            stencil(
                m,
                &wall(c + vec3(0.0, 0.61, -fh - 0.02), PI),
                "NO 7",
                0.022,
                SHADOW,
            );
            stencil(
                m,
                &wall(c + vec3(0.0, 0.43, -fh - 0.02), PI),
                "DO NOT STRIKE",
                0.014,
                SHADOW,
            );
        }
        // Crown loops, common to both.
        for k in 0..4 {
            let g = Mat4::from_translation(c + Vec3::Y * 4.84)
                * Mat4::from_rotation_y(k as f32 * FRAC_PI_2);
            m.tube(
                &g,
                vec3(0.3, 0.0, 0.0),
                0.24,
                0.14,
                0.07,
                8,
                if cracked { BRONZE } else { CLAY },
            );
        }
        // The gantry's name board, on both faces of the beam.
        let beam_y = c.y + layout::BELL_GANTRY_Y + 0.13;
        stencil(
            m,
            &wall(vec3(c.x, beam_y, c.z + 0.412), 0.0),
            "BELL YARD",
            0.05,
            WHITEWASH,
        );
        stencil(
            m,
            &wall(vec3(c.x, beam_y, c.z - 0.412), PI),
            "BELL YARD",
            0.05,
            WHITEWASH,
        );
        // Spilled moulding sand, trodden outward from the flask.
        for k in 0..14u32 {
            let a = hash01(k, 61) * TAU;
            let r = fh + 0.4 + hash01(k, 62) * 3.2;
            let (x, z) = (c.x + a.cos() * r, c.z + a.sin() * r);
            let h = 0.3 + hash01(k, 63) * 0.5;
            flat(
                m,
                x - h,
                z - h * 0.7,
                x + h,
                z + h * 0.7,
                w.height(x, z) + 0.016,
                shade(SAND, 1.2),
            );
        }
    }
}

/// Every mould bed and press gets its working detail: three small bells
/// cooling in a row (white-hot beside the crucible, cold in the yards), a
/// brass pressure gauge, a hand lever and a lit control pedestal.
fn mould_beds(m: &mut Mesh) {
    for (i, &(ax, az)) in layout::MACHINE_ANCHORS.iter().enumerate() {
        for flip in [1.0f32, -1.0] {
            let (cx, cz) = (ax * flip, az * flip);
            let heat = [1.0f32, 0.75, 0.0, 0.0, 0.0][i];
            for (k, dz) in [-0.78f32, 0.0, 0.78].into_iter().enumerate() {
                let foot = vec3(cx + flip * 0.48, 0.95, cz + dz);
                annulus(m, foot + Vec3::Y * 0.004, 0.25, 0.34, 10, DARK);
                if i == 3 && k == 1 {
                    continue; // one mould in the yard was never filled
                }
                let h = (heat - k as f32 * 0.27).max(0.0);
                hand_bell(m, foot, 0.46, |ring, seg| {
                    let cold = shade(BRONZE, 0.9 + 0.2 * hash01(seg as u32, (i * 3 + k) as u32));
                    // Hot metal cools from the thin lip inward: the crown stays brightest.
                    let glow = (h * (0.72 + 0.05 * ring as f32)).min(1.0);
                    mixc(
                        cold,
                        [1.0, 0.36 + 0.34 * glow, 0.06 + 0.2 * glow, 0.9],
                        glow,
                    )
                });
            }
            let xa = cx - flip * 1.0;
            for fz in [-1.0f32, 1.0] {
                let g = wall(
                    vec3(xa, 1.72, cz + fz * 1.012),
                    if fz > 0.0 { 0.0 } else { PI },
                );
                m.tube(&g, Vec3::ZERO, 0.25, 0.2, 0.03, 12, BRASS);
                panel(
                    m,
                    &g,
                    vec3(0.0, 0.0, 0.004),
                    0.2,
                    0.2,
                    [0.80, 0.78, 0.70, 0.12],
                );
                let needle =
                    g * Mat4::from_rotation_z(-1.9 + heat * 2.6 + hash01(i as u32, 5) * 0.5);
                m.obox(
                    &needle,
                    vec3(0.0, 0.07, 0.012),
                    vec3(0.012, 0.09, 0.004),
                    [0.8, 0.1, 0.08, 0.35],
                );
                let tag = format!("M-{}{}", if flip > 0.0 { "W" } else { "E" }, i + 1);
                stencil(
                    m,
                    &wall(
                        vec3(cx, 0.42, cz + fz * 1.212),
                        if fz > 0.0 { 0.0 } else { PI },
                    ),
                    &tag,
                    0.03,
                    shade(BRASS, 0.9),
                );
            }
            beam(
                m,
                vec3(xa, 2.3, cz),
                vec3(xa + flip * 0.28, 2.78, cz),
                0.05,
                0.05,
                EDGE,
            );
            let knob = vec3(xa + flip * 0.3, 2.82, cz);
            m.cuboid(
                knob - Vec3::splat(0.06),
                knob + Vec3::splat(0.06),
                [0.7, 0.1, 0.08, 0.0],
            );
            let xb = cx + flip * 1.5;
            for (k, col) in [
                [0.2f32, 0.9, 0.4, 0.85],
                [1.0, 0.25, 0.12, 0.85],
                [1.0, 0.7, 0.2, 0.85],
            ]
            .into_iter()
            .enumerate()
            {
                let z = cz - 0.95 + k as f32 * 0.3;
                m.cuboid(
                    vec3(xb - 0.1, 1.25, z),
                    vec3(xb + 0.1, 1.285, z + 0.16),
                    col,
                );
            }
        }
    }
}

/// Maintenance catwalks and caged ladders on the furnace shafts.
fn furnace_crowns(m: &mut Mesh) {
    let s = layout::FURNACE_SHAFT_HALF;
    for (x, z) in layout::FURNACE_ANCHORS {
        let (y, o) = (21.3f32, s + 1.1);
        for side in [-1.0f32, 1.0] {
            cube(
                m,
                vec3(x - o, y, z + side * s),
                vec3(x + o, y + 0.1, z + side * o),
                shade(STEEL, 0.9),
            );
            cube(
                m,
                vec3(x + side * s, y, z - s),
                vec3(x + side * o, y + 0.1, z + s),
                shade(STEEL, 0.9),
            );
            for h in [0.55f32, 1.1] {
                let col = if h > 1.0 { WARNING } else { STEEL };
                beam(
                    m,
                    vec3(x - o, y + h, z + side * o),
                    vec3(x + o, y + h, z + side * o),
                    0.07,
                    0.07,
                    col,
                );
                beam(
                    m,
                    vec3(x + side * o, y + h, z - o),
                    vec3(x + side * o, y + h, z + o),
                    0.07,
                    0.07,
                    col,
                );
            }
            for k in 0..=4 {
                let t = -o + 2.0 * o * k as f32 / 4.0;
                beam(
                    m,
                    vec3(x + t, y, z + side * o),
                    vec3(x + t, y + 1.1, z + side * o),
                    0.05,
                    0.05,
                    EDGE,
                );
                beam(
                    m,
                    vec3(x + side * o, y, z + t),
                    vec3(x + side * o, y + 1.1, z + t),
                    0.05,
                    0.05,
                    EDGE,
                );
                beam(
                    m,
                    vec3(x + t, y, z + side * (o - 0.1)),
                    vec3(x + t, y - 0.9, z + side * s),
                    0.07,
                    0.09,
                    RUST,
                );
            }
        }
        // The ladder climbs the face turned away from the nave.
        let out = z.signum();
        let zl = z + out * (s + 0.13);
        for dx in [3.08f32, 3.52] {
            beam(
                m,
                vec3(x + dx, layout::FURNACE_BASE_Y, zl),
                vec3(x + dx, y + 1.1, zl),
                0.05,
                0.05,
                EDGE,
            );
        }
        for k in 0..36 {
            let ry = layout::FURNACE_BASE_Y + 0.3 + k as f32 * 0.4;
            beam(
                m,
                vec3(x + 3.08, ry, zl),
                vec3(x + 3.52, ry, zl),
                0.035,
                0.035,
                STEEL,
            );
        }
        for k in 0..7 {
            let ry = layout::FURNACE_BASE_Y + 2.2 + k as f32 * 2.0;
            let g = Mat4::from_translation(vec3(x + 3.3, ry, zl + out * 0.34))
                * Mat4::from_rotation_x(FRAC_PI_2);
            m.tube(&g, Vec3::ZERO, 0.4, 0.36, 0.03, 10, WARNING);
        }
    }
}

fn signage(m: &mut Mesh, w: &World) {
    for (s, yard, gate) in [
        (1.0f32, "WEST STOCKYARD", "WEST GATE"),
        (-1.0, "EAST STOCKYARD", "EAST GATE"),
    ] {
        // Stockyard gantry lintel, both faces.
        let (gx, gz) = (STOCKYARD.0 * s, STOCKYARD.1 * s);
        let y = w.height(gx, gz) + 6.09;
        stencil(
            m,
            &wall(vec3(gx, y, gz + 0.612), 0.0),
            yard,
            0.068,
            WHITEWASH,
        );
        stencil(
            m,
            &wall(vec3(gx, y, gz - 0.612), PI),
            yard,
            0.068,
            WHITEWASH,
        );
        // The gate wall behind each spawn, facing the yard.
        let wx = GATE_WALL_X * s;
        let y = w.height(wx, 0.0);
        let g = |dy: f32| wall(vec3(wx + s * 0.612, y + dy, 0.0), s * FRAC_PI_2);
        stencil(m, &g(2.55), "WORKS 6", 0.19, WHITEWASH);
        stencil(m, &g(1.7), gate, 0.08, PAINT);
        m.obox(&g(1.45), Vec3::ZERO, vec3(3.6, 0.04, 0.004), PAINT);
    }
    // "Days without incident", by each portal, beside the open lava.
    for s in [1.0f32, -1.0] {
        let g = wall(vec3(-33.085 * s, 2.3, 11.8 * s), s * FRAC_PI_2);
        m.obox(&g, vec3(0.0, 0.0, 0.006), vec3(0.72, 0.52, 0.006), DARK);
        for (dx, dy, hx, hy) in [
            (0.0f32, 0.5f32, 0.72f32, 0.02f32),
            (0.0, -0.5, 0.72, 0.02),
            (-0.7, 0.0, 0.02, 0.5),
            (0.7, 0.0, 0.02, 0.5),
        ] {
            m.obox(&g, vec3(dx, dy, 0.014), vec3(hx, hy, 0.004), WARNING);
        }
        let t = |dy: f32| g * Mat4::from_translation(vec3(0.0, dy, 0.016));
        stencil(m, &t(0.27), "DAYS WITHOUT", 0.017, WHITEWASH);
        stencil(m, &t(0.1), "INCIDENT", 0.017, WHITEWASH);
        m.obox(&g, vec3(0.0, -0.2, 0.014), vec3(0.5, 0.2, 0.004), SHADOW);
        stencil(m, &t(-0.35), "000", 0.045, [1.0, 0.24, 0.1, 0.95]);
    }
}

fn cat(m: &mut Mesh, at: Vec3, yaw: f32) {
    let fur: Col = [0.74, 0.43, 0.16, 0.0];
    let dark = shade(fur, 0.6);
    let pale: Col = [0.86, 0.72, 0.52, 0.0];
    // Curled asleep: a low round body, banded like a tabby.
    lathe(
        m,
        at,
        1.0,
        &[
            (0.0, 0.0),
            (0.2, 0.0),
            (0.23, 0.05),
            (0.2, 0.12),
            (0.12, 0.165),
            (0.0, 0.175),
        ],
        12,
        |ring, seg| {
            if ring >= 2 && seg % 3 == 0 {
                dark
            } else {
                shade(fur, 0.92 + 0.08 * (seg % 2) as f32)
            }
        },
    );
    // Head tucked against the flank, ears up.
    let dir = vec3(yaw.cos(), 0.0, yaw.sin());
    let side = vec3(-yaw.sin(), 0.0, yaw.cos());
    let head = at + dir * 0.2 + Vec3::Y * 0.02;
    lathe(
        m,
        head,
        1.0,
        &[
            (0.0, 0.0),
            (0.075, 0.02),
            (0.09, 0.07),
            (0.07, 0.12),
            (0.0, 0.14),
        ],
        8,
        |_, _| fur,
    );
    for e in [-1.0f32, 1.0] {
        let b = head + side * (e * 0.045) + Vec3::Y * 0.12;
        m.tri(
            b - side * 0.028 - dir * 0.01,
            b + Vec3::Y * 0.06,
            b + side * 0.028 - dir * 0.01,
            dark,
        );
        m.tri(
            b + side * 0.028 + dir * 0.01,
            b + Vec3::Y * 0.06,
            b - side * 0.028 + dir * 0.01,
            pale,
        );
    }
    let nose = head + dir * 0.088 + Vec3::Y * 0.06;
    m.cuboid(
        nose - Vec3::splat(0.012),
        nose + Vec3::splat(0.012),
        [0.75, 0.4, 0.4, 0.0],
    );
    // Tail wrapped round to the nose, tip pale.
    for k in 0..7 {
        let a = yaw + 0.55 + k as f32 * 0.42;
        let p = at + vec3(a.cos() * 0.255, 0.035, a.sin() * 0.255);
        let h = 0.034 - k as f32 * 0.002;
        m.cuboid(
            p - vec3(h * 1.5, h, h * 1.5),
            p + vec3(h * 1.5, h, h * 1.5),
            if k == 0 { pale } else { dark },
        );
    }
}

/// Small things to be found by anyone who stops to look.
fn discoveries(m: &mut Mesh) {
    // The works cat, asleep on a warm control pedestal beside the hot bells.
    let (ax, az) = layout::MACHINE_ANCHORS[1];
    cat(m, vec3(ax + 1.5, 1.25, az + 0.8), 2.4);

    // Shift C's board on the north wall outside: eight hooks, seven helmets.
    let g = wall(vec3(-6.0, 1.75, layout::NAVE_HALF_Z + 0.615), 0.0);
    m.obox(
        &g,
        vec3(0.0, 0.0, 0.02),
        vec3(1.72, 0.86, 0.02),
        shade(TIMBER, 0.75),
    );
    m.obox(
        &g,
        vec3(0.0, 0.68, 0.045),
        vec3(1.5, 0.11, 0.006),
        shade(BRASS, 0.85),
    );
    stencil(
        m,
        &(g * Mat4::from_translation(vec3(0.0, 0.6, 0.054))),
        "SHIFT C",
        0.024,
        SHADOW,
    );
    let hats: [Col; 4] = [
        [0.82, 0.80, 0.74, 0.0],
        WARNING,
        [0.78, 0.36, 0.12, 0.0],
        WARNING,
    ];
    for k in 0..8 {
        let x = -1.4 + k as f32 * 0.4;
        m.obox(&g, vec3(x, 0.3, 0.07), vec3(0.012, 0.012, 0.035), EDGE);
        m.obox(
            &g,
            vec3(x, -0.02, 0.043),
            vec3(0.11, 0.035, 0.003),
            shade(BRASS, 0.9),
        );
        if k == 5 {
            continue; // one hook is bare, and its brass tag polished bright
        }
        let p = g.transform_point3(vec3(x, 0.14, 0.17));
        let col = hats[k % 4];
        lathe(
            m,
            p,
            1.0,
            &[
                (0.17, 0.0),
                (0.135, 0.02),
                (0.125, 0.08),
                (0.075, 0.14),
                (0.0, 0.155),
            ],
            8,
            |_, seg| shade(col, 0.85 + 0.15 * (seg % 2) as f32),
        );
    }
    m.obox(&g, vec3(0.0, -0.46, 0.13), vec3(1.5, 0.02, 0.1), TIMBER);
    for x in [-1.05f32, -0.35, 0.45, 1.1] {
        m.obox(&g, vec3(x, -0.39, 0.13), vec3(0.03, 0.05, 0.03), LAMP);
        m.obox(&g, vec3(x, -0.335, 0.13), vec3(0.04, 0.006, 0.04), DARK);
    }
    hand_bell(
        m,
        g.transform_point3(vec3(0.05, -0.44, 0.13)),
        0.14,
        |_, _| NEW_BRONZE,
    );

    // A chalk tally of pours on a pier inside, and a doodled bell with a crack.
    let g = wall(vec3(6.0, 1.55, layout::NAVE_HALF_Z - 0.615), PI);
    for group in 0..4 {
        let x0 = -0.75 + group as f32 * 0.34;
        for k in 0..4 {
            m.obox(
                &g,
                vec3(x0 + k as f32 * 0.05, 0.0, 0.004),
                vec3(0.007, 0.075, 0.002),
                CHALK,
            );
        }
        if group < 3 {
            let slash = g
                * Mat4::from_translation(vec3(x0 + 0.075, 0.0, 0.006))
                * Mat4::from_rotation_z(1.05);
            m.obox(&slash, Vec3::ZERO, vec3(0.007, 0.13, 0.002), CHALK);
        }
    }
    let stroke = |m: &mut Mesh, a: (f32, f32), b: (f32, f32)| {
        let (p, q) = (vec3(a.0, a.1, 0.004), vec3(b.0, b.1, 0.004));
        let n = (q - p).normalize_or_zero();
        let t = vec3(-n.y, n.x, 0.0) * 0.008;
        let tp = |v: Vec3| g.transform_point3(v);
        m.quad(tp(p - t), tp(q - t), tp(q + t), tp(p + t), CHALK);
    };
    for (a, b) in [
        ((0.86, -0.1), (0.92, 0.12)),
        ((0.92, 0.12), (1.0, 0.2)),
        ((1.0, 0.2), (1.08, 0.12)),
        ((1.08, 0.12), (1.14, -0.1)),
        ((0.82, -0.1), (1.18, -0.1)),
        ((1.0, -0.1), (1.0, -0.16)),
        ((1.06, -0.1), (1.03, -0.01)),
        ((1.03, -0.01), (1.07, 0.05)),
        ((1.07, 0.05), (1.04, 0.13)),
    ] {
        stroke(m, a, b);
    }

    // Somebody's dinner on the north gallery: pail, flask, mug, the paper.
    let d = vec3(9.5, layout::GALLERY_Y, layout::GALLERY_Z + 1.35);
    m.cuboid(
        d + vec3(-0.14, 0.0, -0.09),
        d + vec3(0.14, 0.17, 0.09),
        [0.22, 0.32, 0.42, 0.0],
    );
    m.cuboid(d + vec3(-0.15, 0.17, -0.1), d + vec3(0.15, 0.2, 0.1), EDGE);
    beam(
        m,
        d + vec3(-0.1, 0.2, 0.0),
        d + vec3(0.1, 0.2, 0.0),
        0.02,
        0.05,
        DARK,
    );
    let g = Mat4::from_translation(d + vec3(0.32, 0.14, 0.02)) * Mat4::from_rotation_x(FRAC_PI_2);
    m.tube(&g, Vec3::ZERO, 0.045, 0.0, 0.14, 8, [0.6, 0.12, 0.1, 0.0]);
    m.tube(&g, vec3(0.0, 0.0, -0.15), 0.035, 0.0, 0.02, 8, EDGE);
    let g = Mat4::from_translation(d + vec3(0.5, 0.05, -0.04)) * Mat4::from_rotation_x(FRAC_PI_2);
    m.tube(&g, Vec3::ZERO, 0.04, 0.032, 0.05, 8, WHITEWASH);
    flat(
        m,
        d.x - 0.62,
        d.z - 0.13,
        d.x - 0.26,
        d.z + 0.12,
        d.y + 0.03,
        [0.74, 0.72, 0.66, 0.0],
    );

    // A nest in the crook of a rib brace, three eggs in it.
    let n = vec3(6.0, 14.95, 14.85);
    lathe(
        m,
        n,
        1.0,
        &[(0.0, 0.0), (0.17, 0.03), (0.23, 0.11), (0.2, 0.15)],
        8,
        |_, k| shade(TIMBER, 1.0 + 0.3 * hash01(k as u32, 3)),
    );
    for (dx, dz) in [(-0.05f32, 0.0f32), (0.05, 0.03), (0.0, -0.06)] {
        let e = n + vec3(dx, 0.07, dz);
        m.cuboid(
            e - vec3(0.03, 0.035, 0.03),
            e + vec3(0.03, 0.035, 0.03),
            [0.78, 0.82, 0.76, 0.0],
        );
    }
}

/// Leaded glass for a lancet: a flame rising through cool glass in the amber
/// windows, and its mirror in aqua and indigo in the teal ones.
pub(super) fn glazing(m: &mut Mesh, g: &Mat4, width: f32, height: f32, warm: bool) {
    const COLS: usize = 6;
    const ROWS: usize = 14;
    let half_at = |y: f32| {
        let t = y / height;
        width
            * if t <= 0.65 {
                0.5
            } else if t <= 0.87 {
                0.5 - (t - 0.65) / 0.22 * 0.22
            } else {
                0.28 * (1.0 - (t - 0.87) / 0.13)
            }
    };
    for row in 0..ROWS {
        for col in 0..COLS {
            let (y0, y1) = (
                row as f32 / ROWS as f32 * height,
                (row + 1) as f32 / ROWS as f32 * height,
            );
            let (x0, x1) = (
                -width * 0.5 + col as f32 / COLS as f32 * width,
                -width * 0.5 + (col + 1) as f32 / COLS as f32 * width,
            );
            let yc = (y0 + y1) * 0.5;
            let lim = half_at(yc) - 0.08;
            let (cx0, cx1) = (x0.max(-lim), x1.min(lim));
            if cx1 - cx0 < 0.12 {
                continue;
            }
            let u = ((x0 + x1) * 0.5 / width).abs() * 2.0;
            let v = yc / height;
            let n = hash01((row * COLS + col) as u32, 400 + warm as u32);
            // A teardrop: widest low down, drawing to a point near the head.
            let d = u * (1.2 + v * 1.6) + (v - 0.22).abs() * 0.9 + n * 0.3;
            let c: [f32; 3] = match (warm, d) {
                (true, d) if d < 0.42 => [1.0, 0.86, 0.5],
                (true, d) if d < 0.8 => [1.0, 0.55, 0.15],
                (true, d) if d < 1.2 => [0.74, 0.2, 0.1],
                (true, _) => [0.24, 0.12, 0.2],
                (false, d) if d < 0.42 => [0.72, 0.97, 0.9],
                (false, d) if d < 0.8 => [0.2, 0.66, 0.66],
                (false, d) if d < 1.2 => [0.1, 0.33, 0.48],
                (false, _) => [0.1, 0.13, 0.32],
            };
            let k = 0.82 + 0.3 * hash01((row * COLS + col) as u32, 411);
            panel(
                m,
                g,
                vec3((cx0 + cx1) * 0.5, yc, 0.006),
                (cx1 - cx0) * 0.5 - 0.035,
                (y1 - y0) * 0.5 - 0.035,
                [c[0] * k, c[1] * k, c[2] * k, 0.4],
            );
        }
    }
}

/// Standing water on the apron, holding the colour of the sky.
fn puddles(m: &mut Mesh, w: &World) {
    for k in 0..90u32 {
        let x = (hash01(k, 500) * 2.0 - 1.0) * 108.0 * SPREAD;
        let z = (hash01(k, 501) * 2.0 - 1.0) * 45.0 * SPREAD;
        let r = 0.6 + hash01(k, 502) * 1.6;
        if (x.abs() < 37.0 && z.abs() < 25.0) || z.abs() < 1.8 {
            continue;
        }
        if w.points
            .iter()
            .chain(w.bases.iter())
            .any(|p| (p.x - x).powi(2) + (p.z - z).powi(2) < 64.0)
        {
            continue;
        }
        if w.blocks
            .iter()
            .any(|b| x + r > b.min.x && x - r < b.max.x && z + r > b.min.z && z - r < b.max.z)
        {
            continue;
        }
        let y = w.height(x, z);
        if [(r, 0.0), (-r, 0.0), (0.0, r), (0.0, -r)]
            .iter()
            .any(|&(dx, dz)| (w.height(x + dx, z + dz) - y).abs() > 0.01)
        {
            continue;
        }
        let c = vec3(x, y + 0.011, z);
        let edge = |j: u32, k2: f32| {
            let a = j as f32 / 9.0 * TAU;
            let rr = r * k2 * (0.55 + 0.45 * hash01(k * 16 + j % 9, 503));
            c + vec3(a.cos() * rr, 0.0, a.sin() * rr * 0.7)
        };
        for j in 0..9 {
            // A bright heart and a darker, shallower margin.
            m.tri_n(
                c,
                edge(j + 1, 0.7),
                edge(j, 0.7),
                Vec3::Y,
                [0.42, 0.25, 0.2, 0.55],
            );
            m.quad_n(
                [
                    edge(j, 0.7),
                    edge(j + 1, 0.7),
                    edge(j + 1, 1.0),
                    edge(j, 1.0),
                ],
                [Vec3::Y; 4],
                [0.24, 0.17, 0.17, 0.3],
            );
        }
    }
}

/// Guy cables from each furnace crown to the nave's ridge ends, hung with
/// warning markers.
fn guy_wires(m: &mut Mesh) {
    for (x, z) in layout::FURNACE_ANCHORS {
        let from = vec3(x, layout::FURNACE_HEIGHT + 0.4, z - z.signum() * 4.3);
        for end in [-1.0f32, 1.0] {
            let to = vec3(end * 33.4, layout::NAVE_RIDGE_Y + 0.6, 0.0);
            let sag = (to - from).length() * 0.045;
            let at = |t: f32| from.lerp(to, t) - Vec3::Y * (sag * 4.0 * t * (1.0 - t));
            for k in 0..12 {
                beam(
                    m,
                    at(k as f32 / 12.0),
                    at((k + 1) as f32 / 12.0),
                    0.045,
                    0.045,
                    DARK,
                );
            }
            for t in [0.3f32, 0.5, 0.7] {
                let p = at(t);
                m.cuboid(p - Vec3::splat(0.17), p + Vec3::splat(0.17), WARNING);
            }
        }
    }
}

// --------------------------------------------------------------------- sky

pub(crate) fn sky_basis(d: Vec3) -> (Vec3, Vec3) {
    let rt = d.cross(Vec3::Y).normalize_or_zero();
    (rt, rt.cross(d))
}
pub(crate) fn billboard(m: &mut Mesh, d: Vec3, dist: f32, hw: f32, hh: f32, col: Col) {
    let (rt, up) = sky_basis(d);
    let c = d * dist;
    m.quad(
        c - rt * hw - up * hh,
        c + rt * hw - up * hh,
        c + rt * hw + up * hh,
        c - rt * hw + up * hh,
        col,
    );
}
pub(crate) fn sky_disc(m: &mut Mesh, d: Vec3, dist: f32, size: f32, col: Col) {
    let (rt, up) = sky_basis(d);
    let c = d * dist;
    for k in 0..24 {
        let a0 = k as f32 / 24.0 * TAU;
        let a1 = (k + 1) as f32 / 24.0 * TAU;
        m.tri(
            c,
            c + (rt * a0.cos() + up * a0.sin()) * size,
            c + (rt * a1.cos() + up * a1.sin()) * size,
            col,
        );
    }
}
/// A disc whose colour fades to nothing at its rim.
pub(crate) fn sky_glow(m: &mut Mesh, d: Vec3, dist: f32, size: f32, core: Col) {
    let (rt, up) = sky_basis(d);
    let c = d * dist;
    let rim = [core[0], core[1], core[2], 0.0];
    for k in 0..24 {
        let a0 = k as f32 / 24.0 * TAU;
        let a1 = (k + 1) as f32 / 24.0 * TAU;
        let (p0, p1) = (
            c + (rt * a0.cos() + up * a0.sin()) * size,
            c + (rt * a1.cos() + up * a1.sin()) * size,
        );
        m.quad_c(c, p0, p1, c, [core, rim, rim, core]);
    }
}
pub(crate) fn sky_dir(e: f32, a: f32) -> Vec3 {
    vec3(e.cos() * a.cos(), e.sin(), e.cos() * a.sin())
}

/// First stars, a veiled moon and a smoke-reddened sun, drawn behind the
/// kiln strata.
pub(super) fn heavens(m: &mut Mesh) {
    for k in 0..110u32 {
        let e = 0.6 + hash01(k, 201) * 0.92;
        let s = 0.4 + hash01(k, 203) * 0.7;
        billboard(
            m,
            sky_dir(e, hash01(k, 202) * TAU),
            412.0,
            s,
            s,
            [0.86, 0.88, 1.0, 0.2 + hash01(k, 204) * 0.5],
        );
    }
    let moon = sky_dir(0.5, 3.6);
    sky_glow(m, moon, 410.0, 58.0, [0.62, 0.64, 0.8, 0.16]);
    sky_disc(m, moon, 408.0, 17.0, [0.80, 0.77, 0.78, 0.66]);
    for (de, da, size) in [
        (0.012f32, -0.014f32, 3.6f32),
        (-0.014, 0.008, 2.6),
        (0.002, 0.02, 1.8),
    ] {
        sky_disc(
            m,
            sky_dir(0.5 + de, 3.6 + da),
            407.0,
            size,
            [0.64, 0.61, 0.65, 0.5],
        );
    }
    let sun = SUN.normalize();
    sky_glow(m, sun, 404.0, 190.0, [1.0, 0.36, 0.12, 0.2]);
    sky_glow(m, sun, 403.0, 70.0, [1.0, 0.5, 0.2, 0.42]);
    sky_disc(m, sun, 402.0, 15.0, [1.0, 0.80, 0.56, 1.0]);
    // Two slivers of smoke drawn across the disc.
    let (el, az) = (sun.y.asin(), sun.z.atan2(sun.x));
    billboard(
        m,
        sky_dir(el - 0.014, az + 0.03),
        398.0,
        44.0,
        1.5,
        [0.22, 0.13, 0.14, 0.82],
    );
    billboard(
        m,
        sky_dir(el + 0.02, az - 0.05),
        398.0,
        30.0,
        1.0,
        [0.24, 0.14, 0.14, 0.6],
    );
}

/// Kiln smoke lying in long soft bands, thickest at the middle of each.
pub(super) fn strata(m: &mut Mesh) {
    for k in 0..16u32 {
        let d = sky_dir(0.14 + hash01(k, 9) * 0.42, hash01(k, 8) * TAU);
        let (rt, up) = sky_basis(d);
        let c = d * 388.0;
        let hw = 45.0 + hash01(k, 10) * 95.0;
        let hh = 1.8 + hash01(k, 11) * 3.4;
        for (layer, (dy, sx, al)) in [
            (0.0f32, 1.0f32, 0.5f32),
            (hh * 1.3, 0.7, 0.3),
            (-hh * 1.2, 0.55, 0.26),
        ]
        .into_iter()
        .enumerate()
        {
            let cc = c + up * dy + rt * (layer as f32 * hw * 0.18);
            let (w2, h2) = (hw * sx, hh * (1.0 - 0.25 * layer as f32));
            let core = [0.17, 0.14, 0.18, al];
            let rim = [0.17, 0.14, 0.18, 0.0];
            m.quad_c(
                cc - rt * w2 - up * h2,
                cc - up * h2,
                cc + up * h2,
                cc - rt * w2 + up * h2,
                [rim, core, core, rim],
            );
            m.quad_c(
                cc - up * h2,
                cc + rt * w2 - up * h2,
                cc + rt * w2 + up * h2,
                cc + up * h2,
                [core, rim, rim, core],
            );
        }
    }
}

/// The country beyond the ridge: other works, their stacks and lamps and
/// smoke, and a sister cathedral standing on the nave's axis at each end.
pub(super) fn skyline(m: &mut Mesh) {
    let r = 372.0;
    let haze: Col = [0.345, 0.215, 0.185, 1.0];
    // Haze swallows the foot of everything out there, so each silhouette
    // fades toward the horizon instead of ending on a hard edge.
    let column = |m: &mut Mesh, a: f32, hw: f32, e0: f32, e1: f32, col: Col| {
        let foot = [col[0], col[1], col[2], if e0 < 0.0 { 0.0 } else { col[3] }];
        m.quad_c(
            sky_dir(e0.max(0.02), a - hw) * r,
            sky_dir(e0.max(0.02), a + hw) * r,
            sky_dir(e1, a + hw * 0.8) * r,
            sky_dir(e1, a - hw * 0.8) * r,
            [foot, foot, col, col],
        );
    };
    for k in 0..26u32 {
        let a = (k as f32 + 0.15 + hash01(k, 211) * 0.7) / 26.0 * TAU;
        if a.cos().abs() > 0.975 {
            continue; // keep the axis clear for the sister works
        }
        let top = 0.15 + hash01(k, 212) * 0.11;
        let hw = 0.003 + hash01(k, 213) * 0.004;
        let col = shade(haze, 0.8 + 0.2 * hash01(k, 214));
        column(m, a, hw, -0.4, top, col);
        column(m, a, hw * 1.4, top - 0.004, top + 0.003, shade(col, 0.8));
        billboard(
            m,
            sky_dir(top + 0.007, a),
            r - 2.0,
            0.9,
            0.9,
            [1.0, 0.16, 0.07, 0.95],
        );
        if k % 2 == 0 {
            for j in 0..4 {
                let t = j as f32;
                billboard(
                    m,
                    sky_dir(top + 0.014 + t * 0.018, a + 0.006 + 0.007 * t * t),
                    r - 4.0,
                    3.0 + t * 2.6,
                    2.2 + t * 1.5,
                    [0.15, 0.12, 0.14, 0.34 - t * 0.07],
                );
            }
        }
    }
    for a in [0.0f32, PI] {
        let body = shade(haze, 0.86);
        column(m, a, 0.075, -0.4, 0.215, body);
        m.tri(
            sky_dir(0.215, a - 0.06) * r,
            sky_dir(0.215, a + 0.06) * r,
            sky_dir(0.285, a) * r,
            body,
        );
        billboard(
            m,
            sky_dir(0.165, a),
            r - 2.0,
            5.0,
            14.0,
            [1.0, 0.45, 0.14, 0.6],
        );
        m.tri(
            sky_dir(0.203, a - 0.0135) * (r - 2.0),
            sky_dir(0.203, a + 0.0135) * (r - 2.0),
            sky_dir(0.226, a) * (r - 2.0),
            [1.0, 0.45, 0.14, 0.6],
        );
        for t in [-0.105f32, 0.105] {
            column(m, a + t, 0.012, -0.4, 0.32, shade(haze, 0.8));
            billboard(
                m,
                sky_dir(0.328, a + t),
                r - 2.0,
                1.0,
                1.0,
                [1.0, 0.16, 0.07, 0.95],
            );
            for j in 0..4 {
                billboard(
                    m,
                    sky_dir(0.19 + j as f32 * 0.032, a + t),
                    r - 2.0,
                    1.9,
                    0.9,
                    [1.0, 0.42, 0.12, 0.7],
                );
            }
        }
    }
}

// ------------------------------------------------------------------ lights

/// Extra glow sources; the nearest few to the camera are used.
pub(super) fn lights() -> Vec<Light> {
    let mut out = Vec::new();
    let (lx, lz) = layout::LAVAFALL;
    let s = layout::LAVA_SPANS;
    let far = (s[3].0 + s[3].1) * 0.5;
    let (bx, bz) = layout::BELL_YARD;
    for k in [1.0f32, -1.0] {
        out.push(Light {
            pos: vec3(lx * k, 2.2, lz * k),
            radius: 17.0,
            col: [1.0, 0.36, 0.07],
            power: 1.25,
        });
        out.push(Light {
            pos: vec3(far * k, 1.1, -layout::LAVA_Z * k),
            radius: 13.0,
            col: [1.0, 0.34, 0.06],
            power: 0.9,
        });
        out.push(Light {
            pos: vec3(-18.0 * k, 5.0, 15.5 * k),
            radius: 10.0,
            col: [1.0, 0.74, 0.42],
            power: 0.7,
        });
    }
    out.push(Light {
        pos: vec3(bx, 7.0, bz + 1.5),
        radius: 14.0,
        col: [0.62, 0.80, 0.86],
        power: 0.75,
    });
    out.push(Light {
        pos: vec3(-bx, 3.0, -bz - 3.2),
        radius: 12.0,
        col: [1.0, 0.42, 0.1],
        power: 0.8,
    });
    out
}

// -------------------------------------------------------------- atmosphere

pub(crate) fn cross(mesh: &mut Mesh, p: Vec3, hw: f32, hh: f32, col: Col) {
    mesh.quad(
        p + vec3(-hw, -hh, 0.0),
        p + vec3(hw, -hh, 0.0),
        p + vec3(hw, hh, 0.0),
        p + vec3(-hw, hh, 0.0),
        col,
    );
    mesh.quad(
        p + vec3(0.0, -hh, -hw),
        p + vec3(0.0, -hh, hw),
        p + vec3(0.0, hh, hw),
        p + vec3(0.0, hh, -hw),
        col,
    );
}

/// Crossed sprites whose colour fades to nothing at the rim.
pub(crate) fn soft(mesh: &mut Mesh, p: Vec3, hw: f32, hh: f32, col: Col) {
    let rim = [col[0], col[1], col[2], 0.0];
    for axis in [Vec3::X, Vec3::Z] {
        let (r, u) = (axis * hw, Vec3::Y * hh);
        let corners = [p - r - u, p + r - u, p + r + u, p - r + u];
        for e in 0..4 {
            mesh.quad_c(p, corners[e], corners[(e + 1) % 4], p, [col, rim, rim, col]);
        }
    }
}

/// Everything that moves: lavafalls, the crucible's slow drip, the organ's
/// steam, birds round the lantern, beacons and flares on the furnaces, and
/// coloured light falling through the north windows.
pub(super) fn atmosphere(alpha: &mut Mesh, add: &mut Mesh, w: &World, time: f32) {
    let (lx, lz) = layout::LAVAFALL;
    for (f, k) in [1.0f32, -1.0].into_iter().enumerate() {
        let foot = vec3(lx * k, 0.05, lz * k);
        let f = f as u32;
        let flicker = 0.5 + 0.5 * (time * 9.0 + f as f32 * 2.0).sin();
        soft(
            add,
            foot + Vec3::Y * 2.9,
            0.55,
            3.4,
            [1.0, 0.5, 0.12, 0.42 + 0.12 * flicker],
        );
        for j in 0..7u32 {
            let age = (time * 1.35 + j as f32 / 7.0 + hash01(j, 300 + f) * 0.1).fract();
            let p = foot
                + vec3(
                    (hash01(j, 301 + f) - 0.5) * 0.08,
                    5.75 * (1.0 - age * age),
                    0.0,
                );
            let s = 0.1 + hash01(j, 302 + f) * 0.08;
            cross(add, p, s, s * (1.6 + age * 2.0), [1.0, 0.82, 0.42, 0.55]);
        }
        for j in 0..10u32 {
            let age = (time * (1.1 + hash01(j, 310 + f) * 0.8) + hash01(j, 311 + f)).fract();
            let a = hash01(j, 312 + f) * TAU;
            let r = age * (0.35 + hash01(j, 313 + f) * 0.9);
            let p = foot
                + vec3(
                    a.cos() * r,
                    0.08 + age * (1.0 - age) * 4.0 * (0.35 + hash01(j, 314 + f) * 0.7),
                    a.sin() * r,
                );
            let s = 0.018 + hash01(j, 315 + f) * 0.022;
            cross(add, p, s, s * 1.8, [1.0, 0.6, 0.15, (1.0 - age) * 0.9]);
        }
        add.quad(
            foot + vec3(-1.5, 0.0, -1.5),
            foot + vec3(-1.5, 0.0, 1.5),
            foot + vec3(1.5, 0.0, 1.5),
            foot + vec3(1.5, 0.0, -1.5),
            [1.0, 0.4, 0.1, 0.16 + 0.06 * flicker],
        );
        for j in 0..3u32 {
            let age = (time * 0.22 + j as f32 / 3.0).fract();
            let p = foot + vec3(age * 0.8, 0.6 + age * 4.5, 0.0);
            soft(
                alpha,
                p,
                0.6 + age * 1.3,
                0.6 + age * 1.3,
                [0.2, 0.15, 0.13, (1.0 - age) * 0.22],
            );
        }
    }

    // The crucible weeps: a bead gathers, falls and bursts on the turntable.
    let phase = (time / 6.5).fract();
    let lip = vec3(0.0, 8.62, 0.0);
    if phase > 0.8 {
        let s = 0.03 + (phase - 0.8) * 0.4;
        cross(add, lip, s, s * 1.3, [1.0, 0.75, 0.35, 0.8]);
    } else if phase < 0.11 {
        let t = phase / 0.11;
        cross(
            add,
            lip - Vec3::Y * (8.55 * t * t),
            0.07,
            0.07 + t * 0.2,
            [1.0, 0.8, 0.4, 0.9],
        );
    } else if phase < 0.24 {
        let t = (phase - 0.11) / 0.13;
        for j in 0..8u32 {
            let a = j as f32 / 8.0 * TAU + 0.3;
            let p = vec3(
                a.cos() * t * 0.9,
                0.06 + t * (1.0 - t) * 1.6,
                a.sin() * t * 0.9,
            );
            cross(add, p, 0.025, 0.04, [1.0, 0.6, 0.18, 1.0 - t]);
        }
    }

    // The organ plays a slow, wandering figure in steam.
    let beat = time * 1.5;
    let (bar, age) = (beat.floor() as u32, beat.fract());
    for (r, (side, flank)) in [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)]
        .into_iter()
        .enumerate()
    {
        for i in 0..7 {
            if hash01(bar.wrapping_mul(13) + (r * 7 + i) as u32, 320) > 0.2 {
                continue;
            }
            let (x, z, top, radius) = organ_pipe(side, flank, i);
            let p = vec3(x, top + 0.2 + age * 1.5, z - side * age * 0.35);
            let s = radius * 1.4 + age * 0.55;
            soft(
                alpha,
                p,
                s * 1.3,
                s * 1.3,
                [0.80, 0.80, 0.84, (1.0 - age) * 0.42],
            );
        }
    }

    // Birds wheel round the open ridge lantern.
    for k in 0..9u32 {
        let dirn = if k % 3 == 0 { -1.0 } else { 1.0 };
        let a = dirn * time * (0.22 + hash01(k, 330) * 0.16) + hash01(k, 331) * TAU;
        let r = 9.0 + hash01(k, 332) * 13.0;
        let p = vec3(
            a.cos() * r * 1.5,
            32.5 + hash01(k, 333) * 5.0 + (time * 0.4 + k as f32).sin() * 1.2,
            a.sin() * r,
        );
        let fwd = vec3(-a.sin() * 1.5, 0.0, a.cos()).normalize_or_zero() * dirn;
        let right = fwd.cross(Vec3::Y);
        let flap = (time * (7.0 + hash01(k, 334) * 3.0) + k as f32).sin();
        let col = [0.045, 0.045, 0.055, 0.9];
        for e in [-1.0f32, 1.0] {
            alpha.tri(
                p + fwd * 0.2,
                p - fwd * 0.16,
                p + right * (e * 0.5) + Vec3::Y * (flap * 0.28) - fwd * 0.05,
                col,
            );
        }
    }

    // Furnace crowns: red obstruction beacons and a flare on every stack.
    for (f, &(x, z)) in layout::FURNACE_ANCHORS.iter().enumerate() {
        let h = layout::FURNACE_HEIGHT;
        let on = (time * 0.75 + f as f32 * 0.5).fract();
        let glow = if on < 0.3 { 1.0 - on / 0.3 * 0.4 } else { 0.0 };
        for (dx, dz) in [
            (-4.45f32, -4.45f32),
            (4.45, -4.45),
            (-4.45, 4.45),
            (4.45, 4.45),
        ] {
            let p = vec3(x + dx, h + 0.75, z + dz);
            cross(add, p, 0.11, 0.11, [1.0, 0.12, 0.06, 0.35 + 0.65 * glow]);
            if glow > 0.0 {
                soft(add, p, 0.8, 0.8, [1.0, 0.1, 0.05, 0.5 * glow]);
            }
        }
        for (j, (dx, dz)) in [(-2.3f32, -2.3f32), (2.3, -2.3), (-2.3, 2.3), (2.3, 2.3)]
            .into_iter()
            .enumerate()
        {
            let seed = (f * 4 + j) as f32;
            let lick =
                1.3 + 0.5 * (time * 6.0 + seed * 1.7).sin() + 0.3 * (time * 13.0 + seed).sin();
            let base = vec3(x + dx, h + 3.45, z + dz);
            let tip = base + vec3(0.25 * (time * 3.0 + seed).sin(), lick, 0.0);
            let col = [1.0, 0.52, 0.14, 0.5];
            add.tri(
                base + vec3(-0.5, 0.0, 0.0),
                base + vec3(0.5, 0.0, 0.0),
                tip,
                col,
            );
            add.tri(
                base + vec3(0.0, 0.0, -0.5),
                base + vec3(0.0, 0.0, 0.5),
                tip,
                col,
            );
        }
    }

    // Evening sun through three north windows: slanting shafts, drifting
    // motes, and a pool of coloured light on the casting floor.
    let sun = SUN.normalize();
    for bay in [2usize, 3, 4] {
        let xc = (layout::NAVE_RIB_X[bay] + layout::NAVE_RIB_X[bay + 1]) * 0.5;
        let tint: [f32; 3] = if bay % 2 == 0 {
            [0.34, 0.86, 0.86]
        } else {
            [1.0, 0.62, 0.25]
        };
        let breathe = 0.85 + 0.15 * (time * 0.3 + bay as f32).sin();
        let z = layout::NAVE_HALF_Z - 0.75;
        let corner = |dx: f32, y: f32| vec3(xc + dx, y, z);
        let land = |p: Vec3| p - sun * ((p.y - 0.05) / sun.y);
        let (near, far) = (
            [tint[0], tint[1], tint[2], 0.032 * breathe],
            [tint[0], tint[1], tint[2], 0.004],
        );
        let win = [
            corner(-2.1, 11.4),
            corner(2.1, 11.4),
            corner(2.1, 21.4),
            corner(-2.1, 21.4),
        ];
        // Thin parallel sheets read as depth where a single slab would not.
        for i in 0..6 {
            let dx = -2.1 + i as f32 * 0.84;
            let (a, b) = (corner(dx, 11.4), corner(dx, 21.4));
            add.quad_c(a, b, land(b), land(a), [near, near, far, far]);
        }
        for i in 0..4 {
            let y = 11.4 + i as f32 * 3.33;
            let (a, b) = (corner(-2.1, y), corner(2.1, y));
            add.quad_c(a, b, land(b), land(a), [near, near, far, far]);
        }
        add.quad(
            land(win[0]),
            land(win[3]),
            land(win[2]),
            land(win[1]),
            [tint[0], tint[1], tint[2], 0.085 * breathe],
        );
        for j in 0..14u32 {
            let seed = bay as u32 * 40 + j;
            let p0 = corner(
                (hash01(seed, 340) - 0.5) * 4.0,
                11.6 + hash01(seed, 341) * 9.6,
            );
            let t = (hash01(seed, 342) + time * 0.012 * (0.5 + hash01(seed, 343))).fract();
            let p = p0.lerp(land(p0), t)
                + vec3(
                    (time * 0.5 + seed as f32).sin() * 0.12,
                    (time * 0.37 + seed as f32).cos() * 0.1,
                    0.0,
                );
            cross(add, p, 0.022, 0.022, [tint[0], tint[1], tint[2], 0.55]);
        }
    }

    // The lantern at the cracked bell gutters; No. 7's flask still spits.
    let (bx, bz) = layout::BELL_YARD;
    let fh = layout::BELL_FLASK_HALF;
    let lantern = vec3(
        bx + 1.72,
        w.height(bx, bz) + layout::BELL_FLASK_H + 0.12,
        bz + fh - 0.13,
    );
    let gutter = 0.6 + 0.25 * (time * 11.0).sin() + 0.15 * (time * 4.3).sin();
    soft(add, lantern, 0.5, 0.5, [1.0, 0.7, 0.35, 0.45 * gutter]);
    let seven = vec3(-bx, w.height(-bx, -bz), -bz);
    for j in 0..8u32 {
        let age = (time * (0.3 + hash01(j, 350) * 0.3) + hash01(j, 351)).fract();
        let a = hash01(j, 352) * TAU;
        let p = seven + vec3(a.cos() * (fh + 0.1), 0.5 + age * 2.4, a.sin() * (fh + 0.1));
        cross(add, p, 0.02, 0.035, [1.0, 0.5, 0.12, (1.0 - age) * 0.8]);
    }
}
