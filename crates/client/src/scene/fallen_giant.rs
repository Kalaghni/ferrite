//! A military heavy-lifter that ploughed into Ridgeline's saddle and broke
//! in three. The hull walls, roof, buckled frames, cargo and the wreckage
//! strewn outside are drawn over shared collision. Everything else - the
//! twisted crown, wings, tail, nose, hanging skin and the debris along the
//! furrow - stays above head height or below the knee, so nothing a player
//! sees at body height is missing from collision (see the tests below).
use super::hash01;
use super::slag_cathedral::art::{cross, soft, stencil, wall};
use super::slag_cathedral::{basis, beam};
use crate::gfx::{shade, Col, Mesh};
use ferrite_shared::glam::{vec3, Mat4, Quat, Vec3};
use ferrite_shared::map::{fallen_giant as layout, Block, BlockKind, World};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Map id, as the other landmark scenes export it.
pub const ID: u8 = layout::MAP_ID;

const SKIN: Col = [0.34, 0.38, 0.34, 0.0];
const LIGHT: Col = [0.47, 0.50, 0.44, 0.0];
const INSIDE: Col = [0.22, 0.26, 0.25, 0.0];
const FRAME: Col = [0.39, 0.42, 0.40, 0.0];
const DARK: Col = [0.065, 0.085, 0.09, 0.0];
const RUST: Col = [0.41, 0.23, 0.14, 0.0];
const WHITE: Col = [0.72, 0.73, 0.65, 0.0];
const YELLOW: Col = [0.83, 0.55, 0.16, 0.0];
/// Bare aluminium where the skin tore.
const RAW: Col = [0.68, 0.68, 0.63, 0.0];
/// Burnt black.
const CHAR: Col = [0.045, 0.04, 0.035, 0.0];
const INSULATION: Col = [0.60, 0.53, 0.34, 0.0];
const DECK: Col = [0.29, 0.30, 0.29, 0.0];
const CARGO: Col = [0.32, 0.31, 0.25, 0.0];
const BOX: Col = [0.55, 0.27, 0.15, 0.0];
const TYRE: Col = [0.06, 0.06, 0.065, 0.0];
const BARK: Col = [0.30, 0.22, 0.15, 0.0];
const SPLINTER: Col = [0.72, 0.62, 0.43, 0.0];
const SEAT: Col = [0.24, 0.29, 0.22, 0.0];

/// The hull broke at |x| = BREAK, above the buckled frames.
const BREAK: f32 = layout::ROOF_TEAR[0];
/// Crown profile across the hull: (z, rise above the crown's base).
const PROFILE: [(f32, f32); 7] = [(-6.42, 0.0), (-5.25, 1.8), (-2.8, 3.0), (0.0, 3.35), (2.8, 3.0), (5.25, 1.8), (6.42, 0.0)];
const CROWN: f32 = layout::ROOF_HEIGHT + 0.39;
/// Crown bays run between these x stations; two meet each break and tear.
const STATIONS: [f32; 21] =
    [-42.0, -38.0, -34.0, -30.0, -26.0, -22.0, -17.0, -12.0, -8.0, -4.0, 0.0, 4.0, 8.0, 12.0, 17.0, 22.0, 26.0, 30.0, 34.0, 38.0, 42.0];

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn hump(v: f32, centre: f32, half: f32) -> f32 {
    1.0 - smoothstep(0.0, half, (v - centre).abs())
}
fn mix(a: Col, b: Col, t: f32) -> Col {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3]]
}
fn lerp(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

/// How burnt the skin is at (x, y) in hull space: both breaks burned, the
/// forward one worst, and the fire climbed.
fn soot(x: f32, y: f32) -> f32 {
    let near = 1.0 - ((x.abs() - BREAK).abs() / 11.0).min(1.0).powf(1.5);
    let climb = 0.5 + 0.5 * (y / 11.0).clamp(0.0, 1.0);
    (near * climb * if x > 0.0 { 1.0 } else { 0.8 }).clamp(0.0, 1.0)
}
fn sooted(col: Col, x: f32, y: f32, jitter: f32) -> Col {
    mix(col, CHAR, soot(x, y) * (0.55 + 0.45 * jitter))
}

fn origin(w: &World) -> Vec3 {
    vec3(0.0, layout::floor(w), 0.0)
}
fn at(w: &World, x: f32, y: f32, z: f32) -> Vec3 {
    origin(w) + vec3(x, y, z)
}
fn box_at(m: &mut Mesh, w: &World, c: Vec3, half: Vec3, col: Col) {
    m.obox(&Mat4::from_translation(origin(w)), c, half, col);
}
fn plate(m: &mut Mesh, points: [Vec3; 4], thick: f32, col: Col) {
    let n = (points[1] - points[0]).cross(points[3] - points[0]).normalize_or_zero() * thick;
    m.quad(points[0], points[1], points[2], points[3], col);
    m.quad(points[3] - n, points[2] - n, points[1] - n, points[0] - n, shade(col, 0.6));
    for i in 0..4 {
        let j = (i + 1) % 4;
        m.quad(points[i], points[i] - n, points[j] - n, points[j], FRAME);
    }
}
/// A thin round rod, cheaper than a pipe.
fn rod(m: &mut Mesh, a: Vec3, b: Vec3, r: f32, col: Col) {
    let (g, len) = basis(a, b);
    m.tube(&g, Vec3::ZERO, r, 0.0, len * 0.5, 4, col);
}
/// A quad dented in or out at its centre, so it catches light unevenly.
fn crumple(m: &mut Mesh, p: [Vec3; 4], push: f32, col: Col) {
    let n = (p[1] - p[0]).cross(p[3] - p[0]).normalize_or_zero();
    let c = (p[0] + p[1] + p[2] + p[3]) * 0.25 + n * push;
    for i in 0..4 {
        m.tri(p[i], p[(i + 1) % 4], c, shade(col, 0.82 + 0.12 * i as f32));
    }
}
/// Ragged teeth along a -> b, pointing along `dir`, as if the sheet beyond
/// had been ripped away.
fn teeth(m: &mut Mesh, a: Vec3, b: Vec3, dir: Vec3, reach: f32, n: u32, seed: u32, col: Col) {
    for k in 0..n {
        let (t0, t1) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
        let tip = lerp(a, b, (t0 + t1) * 0.5 + (hash01(seed, k) - 0.5) * 0.4 / n as f32);
        let len = reach * (0.35 + 0.65 * hash01(seed ^ 0x55, k));
        m.tri(lerp(a, b, t0), lerp(a, b, t1), tip + dir * len, if k % 3 == 0 { RAW } else { col });
    }
}

/// A skin panel with a ragged hole torn through it. `q` runs round the
/// panel; `size` is how much of it the hole takes. Bright metal shows along
/// alternate torn edges.
fn holed(m: &mut Mesh, q: [Vec3; 4], size: f32, seed: u32, col: Col) {
    let at = |u: f32, v: f32| lerp(lerp(q[0], q[1], u), lerp(q[3], q[2], u), v);
    const RIM: [(f32, f32); 8] = [(0.0, 0.0), (0.5, 0.0), (1.0, 0.0), (1.0, 0.5), (1.0, 1.0), (0.5, 1.0), (0.0, 1.0), (0.0, 0.5)];
    let cx = 0.5 + (hash01(seed, 20) - 0.5) * 0.2;
    let cy = 0.5 + (hash01(seed, 21) - 0.5) * 0.2;
    let hole: Vec<Vec3> = RIM
        .iter()
        .enumerate()
        .map(|(k, &(u, v))| {
            let s = size * (0.45 + 0.55 * hash01(seed, k as u32));
            at(cx + (u - cx) * s, cy + (v - cy) * s)
        })
        .collect();
    for k in 0..8 {
        let j = (k + 1) % 8;
        let (a, b) = (at(RIM[k].0, RIM[k].1), at(RIM[j].0, RIM[j].1));
        m.quad(a, b, hole[j], hole[k], shade(col, 0.92 + 0.12 * hash01(seed, 30 + k as u32)));
        if k % 2 == 0 {
            let lip = lerp(hole[k], hole[j], 0.5) + (lerp(a, b, 0.5) - lerp(hole[k], hole[j], 0.5)) * 0.25;
            m.tri(hole[k], hole[j], lip, RAW);
        }
    }
}

/// Above the shared collision the two end sections are twisted about their
/// breaks (yaw, roll, pitch), easing in over 4 m of crumpled crown.
fn bend(p: Vec3) -> Vec3 {
    let s = smoothstep(BREAK - 2.0, BREAK + 2.0, p.x.abs());
    if s <= 0.0 {
        return p;
    }
    let (yaw, roll, pitch) = if p.x > 0.0 { (0.09, 0.07, -0.03) } else { (-0.06, -0.06, 0.03) };
    let pivot = vec3(BREAK * p.x.signum(), layout::ROOF_HEIGHT, 0.0);
    let q = Quat::from_rotation_y(yaw * s) * Quat::from_rotation_x(roll * s) * Quat::from_rotation_z(pitch * s);
    pivot + q * (p - pivot)
}
/// The crown buckled upward across each break, highest along the spine.
fn crown(w: &World, x: f32, i: usize, lift: f32) -> Vec3 {
    let (z, r) = PROFILE[i];
    let buckle = hump(x.abs(), BREAK + 0.5, 3.5) * (0.4 + r / 3.35) * if x > 0.0 { 1.3 } else { 0.9 };
    origin(w) + bend(vec3(x, CROWN + r + lift + buckle, z))
}
/// Whether the roof is torn open over (x, z).
fn torn(x: f32, z: f32) -> bool {
    let [t0, t1] = layout::ROOF_TEAR;
    x.abs() > t0 && x.abs() < t1 && z * x.signum() > -layout::ROOF_TEAR_EDGE
}

// ------------------------------------------------------------------ blocks

pub fn block(m: &mut Mesh, w: &World, b: &Block) {
    match (b.kind, b.tint) {
        (BlockKind::Trunk, _) => stump(m, w, b),
        (_, layout::CARGO_TINT) => cargo(m, b),
        (_, layout::FRAME_TINT) if b.max.y - b.min.y > 3.0 => bulkhead(m, b),
        (_, layout::FRAME_TINT) => heap(m, b),
        (_, layout::DEBRIS_TINT) => wreckage(m, w, b),
        _ if b.max.y - b.min.y > 1.0 => hull_wall(m, b),
        _ => m.cuboid(b.min, b.max, INSIDE),
    }
}

/// A hull wall: dark lining behind a skin of panels on the global 4 m bay
/// grid. Low panels are dented and scorched but stay flush; above the
/// tears some are ripped off, exposing frames, or peeled outward.
fn hull_wall(m: &mut Mesh, b: &Block) {
    m.cuboid(b.min, b.max, INSIDE);
    let g = b.max.y - layout::ROOF_HEIGHT;
    let side = ((b.min.z + b.max.z) * 0.5).signum();
    let z = if side < 0.0 { b.min.z - 0.025 } else { b.max.z + 0.025 };
    let rows: &[f32] = if b.min.y > g + 1.0 { &[4.0, 6.0, 8.0] } else { &[-0.6, 1.5, 4.0, 6.0, 8.0] };
    let mut xs = vec![b.min.x];
    let mut k = ((b.min.x + 42.0) / 4.0).floor() + 1.0;
    while -42.0 + k * 4.0 < b.max.x - 0.5 {
        xs.push(-42.0 + k * 4.0);
        k += 1.0;
    }
    xs.push(b.max.x);
    let out = Vec3::Z * side;
    // Painted names stay readable on intact panels.
    let lettered = |x0: f32, x1: f32, y0: f32| y0 >= 4.0 && ((x1 > 20.5 && x0 < 35.5) || (x1 > -35.0 && x0 < -27.0));
    for c in xs.windows(2) {
        for r in rows.windows(2) {
            let (x0, x1, y0, y1) = (c[0], c[1], g + r[0], g + r[1]);
            let key = ((x0 + 60.0) * 4.0) as u32 * 8 + (r[0] + 1.0) as u32 + if side > 0.0 { 4096 } else { 0 };
            let h = hash01(key, 91);
            let xm = (x0 + x1) * 0.5;
            let col = sooted(shade(SKIN, 0.84 + hash01(key, 3) * 0.22), xm, r[1], hash01(key, 5));
            let p = |x: f32, y: f32| vec3(x, y, z);
            let quad = if side > 0.0 { [p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)] } else { [p(x1, y0), p(x0, y0), p(x0, y1), p(x1, y1)] };
            let high = r[0] >= 4.0 && !lettered(x0, x1, r[0]);
            if high && (h < 0.3 || soot(xm, r[1]) * hash01(key, 12) > 0.45) {
                // Skin gone: insulation, a frame and two stringers.
                m.quad(
                    quad[0] - out * 0.01,
                    quad[1] - out * 0.01,
                    quad[2] - out * 0.01,
                    quad[3] - out * 0.01,
                    mix(INSULATION, CHAR, soot(xm, r[1])),
                );
                beam(m, p(xm, y0) + out * 0.03, p(xm, y1) + out * 0.03, 0.12, 0.06, FRAME);
                for t in [0.33, 0.66] {
                    let y = y0 + (y1 - y0) * t;
                    beam(m, p(x0, y) + out * 0.03, p(x1, y) + out * 0.03, 0.06, 0.05, shade(FRAME, 0.8));
                }
                let peeled = h < 0.13 || (h > 0.3 && hash01(key, 13) < 0.4);
                if !peeled {
                    holed(m, quad.map(|v| v + out * 0.05), 0.55 + 0.4 * hash01(key, 14), key, col);
                } else {
                    teeth(m, p(x0, y1) + out * 0.02, p(x1, y1) + out * 0.02, -Vec3::Y, 0.45, 4, key, col);
                }
                if peeled {
                    // Peeled outward from its top edge, high above head height.
                    let a = 0.55 + hash01(key, 8) * 0.5;
                    let drop = (y1 - y0) * 0.92;
                    let low = |x: f32| p(x, y1 - drop * a.cos()) + out * (drop * a.sin());
                    let flap = if side > 0.0 { [p(x0, y1), p(x1, y1), low(x1), low(x0)] } else { [p(x1, y1), p(x0, y1), low(x0), low(x1)] };
                    m.quad(flap[0], flap[1], flap[2], flap[3], col);
                    m.quad(flap[3], flap[2], flap[1], flap[0], shade(INSIDE, 0.8));
                    teeth(m, flap[3], flap[2], (flap[3] - flap[0]).normalize(), 0.5, 4, key ^ 7, col);
                }
            } else if h < 0.45 {
                crumple(m, quad, -0.04 - 0.06 * hash01(key, 9), col);
            } else {
                m.quad(quad[0], quad[1], quad[2], quad[3], col);
            }
        }
    }
}

/// A buckled frame across the hold, floor to roof: crushed skin on both
/// faces, a heavy post on its open edge and struts bent by the impact.
fn bulkhead(m: &mut Mesh, b: &Block) {
    m.cuboid(b.min, b.max, INSIDE);
    let x = (b.min.x + b.max.x) * 0.5;
    let g = b.min.y;
    let open = if x < 0.0 { b.max.z } else { b.min.z };
    let wall = if x < 0.0 { b.min.z } else { b.max.z };
    let seed = if x < 0.0 { 3 } else { 11 };
    let (nz, ny) = (4usize, 5usize);
    for face in [-1.0f32, 1.0] {
        let fx = if face < 0.0 { b.min.x - 0.012 } else { b.max.x + 0.012 };
        let fkey = seed * 1000 + if face > 0.0 { 500 } else { 0 };
        // A grid of crushed sheets whose inner corners wander, so the folds
        // run at odd angles rather than in a quilt.
        let grid = |i: usize, j: usize| {
            let (mut z, mut y) = (b.min.z + (b.max.z - b.min.z) * i as f32 / nz as f32, layout::ROOF_HEIGHT * j as f32 / ny as f32);
            if i > 0 && i < nz && j > 0 && j < ny {
                z += (hash01(fkey + (i * 10 + j) as u32, 1) - 0.5) * 0.9;
                y += (hash01(fkey + (i * 10 + j) as u32, 2) - 0.5) * 0.9;
            }
            vec3(fx, g + y, z)
        };
        for i in 0..nz {
            for j in 0..ny {
                let key = fkey + (i * 10 + j) as u32;
                let q = [grid(i, j), grid(i + 1, j), grid(i + 1, j + 1), grid(i, j + 1)];
                let q = if face > 0.0 { q } else { [q[1], q[0], q[3], q[2]] };
                let h = hash01(key, 4);
                let base = if h < 0.18 {
                    RAW
                } else if h < 0.3 {
                    INSULATION
                } else if h < 0.42 {
                    CHAR
                } else {
                    SKIN
                };
                let y = layout::ROOF_HEIGHT * (j as f32 + 0.5) / ny as f32;
                let col = sooted(shade(base, 0.75 + 0.35 * hash01(key, 5)), x, y, h);
                let push = if (i + j) % 2 == 0 { -0.18 } else { -0.05 } * (0.5 + hash01(key, 6));
                crumple(m, q, push, col);
            }
        }
        // Struts bent by the impact, flush on the face.
        let f = |z: f32, y: f32| vec3(fx + face * 0.04, g + y, z);
        let kink = lerp(f(open, 0.0), f(wall, 0.0), 0.45) + Vec3::Y * 3.4;
        beam(m, f(open, 0.4), kink, 0.16, 0.08, FRAME);
        beam(m, kink, f(wall, 7.4), 0.16, 0.08, FRAME);
        for y in [2.1, 6.1] {
            beam(m, f(open, y), f(wall, y + 0.35 * face), 0.12, 0.06, shade(FRAME, 0.8));
        }
    }
    // Heavy post along the open edge, with the torn web beside it.
    let post = open - (open - wall).signum() * 0.18;
    beam(m, vec3(x, g, post), vec3(x, g + layout::ROOF_HEIGHT, post), 0.62, 0.34, shade(FRAME, 0.9));
    let edge = |y: f32| vec3(x, g + y, open + (open - wall).signum() * 0.012);
    for k in 0..4 {
        let y = 0.6 + k as f32 * 1.9;
        m.tri(edge(y) + Vec3::X * 0.3, edge(y + 1.2) + Vec3::X * 0.3, edge(y + 0.5) - Vec3::X * 0.3, RAW);
    }
}

/// The roof skin that fell through the tear: a crushed heap the height of
/// its collision, ribs across the top.
fn heap(m: &mut Mesh, b: &Block) {
    let (lo, hi) = (b.min, b.max);
    let seed = if lo.x < 0.0 { 5 } else { 9 };
    m.cuboid(lo + vec3(0.1, 0.0, 0.1), hi - vec3(0.1, 0.12, 0.1), CHAR);
    // Top: a 3 x 2 grid of crushed sheets, every corner at or under the top.
    let (nx, nz) = (3, 2);
    let pt = |i: usize, j: usize| {
        let x = lo.x + (hi.x - lo.x) * i as f32 / nx as f32;
        let z = lo.z + (hi.z - lo.z) * j as f32 / nz as f32;
        let dip = if i == 0 || j == 0 || i == nx || j == nz { 0.25 } else { 0.0 };
        vec3(x, hi.y - dip - 0.25 * hash01(seed * 31 + i as u32, j as u32), z)
    };
    for i in 0..nx {
        for j in 0..nz {
            let key = seed * 7 + (i * 3 + j) as u32;
            let col = mix(if hash01(key, 2) < 0.5 { SKIN } else { RAW }, CHAR, 0.3 + 0.4 * hash01(key, 3));
            crumple(m, [pt(i, j), pt(i, j + 1), pt(i + 1, j + 1), pt(i + 1, j)], -0.15, col);
        }
    }
    // Sides: crushed skin hanging over the edges of the pile.
    let corners = [vec3(lo.x, 0.0, lo.z), vec3(hi.x, 0.0, lo.z), vec3(hi.x, 0.0, hi.z), vec3(lo.x, 0.0, hi.z)];
    for k in 0..4 {
        let (a, c) = (corners[k], corners[(k + 1) % 4]);
        let top = hi.y - 0.3;
        let q = [vec3(a.x, lo.y + 0.3, a.z), vec3(c.x, lo.y + 0.3, c.z), vec3(c.x, top, c.z), vec3(a.x, top, a.z)];
        crumple(m, q, -0.12, mix(SKIN, CHAR, 0.35 + 0.15 * k as f32));
    }
    // Two bent ribs lying across the heap.
    for (t, col) in [(0.3, FRAME), (0.72, shade(FRAME, 0.7))] {
        let x = lo.x + (hi.x - lo.x) * t;
        let mid = vec3(x + 0.2, hi.y - 0.06, (lo.z + hi.z) * 0.5);
        beam(m, vec3(x, hi.y - 0.35, lo.z + 0.15), mid, 0.12, 0.12, col);
        beam(m, mid, vec3(x + 0.5, hi.y - 0.3, hi.z - 0.15), 0.12, 0.12, col);
    }
}

fn cargo(m: &mut Mesh, b: &Block) {
    let h = b.max.y - b.min.y;
    if h > 2.0 {
        // A toppled container, ribbed along its length.
        m.cuboid(b.min, b.max, shade(BOX, 0.85));
        let n = 7;
        for i in 0..n {
            let y = b.min.y + h * (i as f32 + 0.5) / n as f32;
            for z in [b.min.z - 0.03, b.max.z + 0.03] {
                m.cuboid(vec3(b.min.x + 0.15, y - 0.06, z - 0.02), vec3(b.max.x - 0.15, y + 0.06, z + 0.02), shade(BOX, 0.65));
            }
        }
        for x in [b.min.x - 0.02, b.max.x + 0.02] {
            m.cuboid(vec3(x - 0.02, b.min.y, b.min.z), vec3(x + 0.02, b.max.y, b.max.z), DARK);
        }
        return;
    }
    m.cuboid(b.min, b.max, CARGO);
    for i in 0..4 {
        let x = b.min.x + (b.max.x - b.min.x) * (i as f32 + 0.5) / 4.0;
        m.cuboid(vec3(x - 0.045, b.min.y, b.min.z - 0.03), vec3(x + 0.045, b.max.y + 0.02, b.max.z + 0.03), DARK);
    }
    m.cuboid(vec3(b.min.x + 0.2, b.max.y, b.min.z + 0.15), vec3(b.max.x - 0.4, b.max.y + 0.03, b.max.z - 0.2), shade(CARGO, 0.75));
}

/// Which outside piece a debris block is, and whether it is the authored
/// (aft) copy or its mirror.
fn piece(b: &Block) -> Option<(layout::Piece, bool)> {
    let c = (b.min + b.max) * 0.5;
    layout::PIECES.into_iter().find_map(|p| {
        [(1.0, true), (-1.0, false)]
            .into_iter()
            .find_map(|(s, aft)| ((c.x - s * p.x).abs() < 0.01 && (c.z - s * p.z).abs() < 0.01).then_some((p, aft)))
    })
}

fn wreckage(m: &mut Mesh, w: &World, b: &Block) {
    let Some((p, aft)) = piece(b) else {
        m.cuboid(b.min, b.max, SKIN);
        return;
    };
    let c = (b.min + b.max) * 0.5;
    let ground = b.min.y + 0.4;
    if p.hx == layout::BOGIE.hx {
        // A main gear bogie: two tyres on a snapped oleo.
        let r = 0.97;
        for k in [-1.0f32, 1.0] {
            let g = Mat4::from_translation(vec3(c.x, ground + r, c.z + k * 0.36));
            m.tube(&g, Vec3::ZERO, r, 0.42, 0.17, 10, TYRE);
            m.tube(&g, Vec3::ZERO, 0.43, 0.0, 0.14, 6, FRAME);
        }
        beam(m, vec3(c.x, ground + r, c.z - 0.7), vec3(c.x, ground + r, c.z + 0.7), 0.22, 0.22, DARK);
        let dir = if aft { 1.0 } else { -1.0 };
        let top = vec3(c.x + dir * 1.05, b.max.y - 0.12, c.z);
        beam(m, vec3(c.x, ground + r, c.z), top, 0.34, 0.34, FRAME);
        teeth(m, top + vec3(-0.17, 0.0, -0.17), top + vec3(0.17, 0.0, 0.17), Vec3::Y, 0.1, 3, 77, RAW);
    } else if p.hx == layout::NACELLE.hx {
        // A torn-off engine, half buried and burnt out.
        let r = p.hz;
        let g = Mat4::from_translation(vec3(c.x, b.max.y - r, c.z)) * Mat4::from_rotation_y(FRAC_PI_2);
        let shell = mix(SKIN, CHAR, if aft { 0.7 } else { 0.45 });
        m.tube(&g, Vec3::ZERO, r, r - 0.18, p.hx - 0.25, 10, shell);
        m.tube(&g, vec3(0.0, 0.0, p.hx - 0.3), r + 0.05, r - 0.25, 0.15, 10, if aft { CHAR } else { LIGHT });
        m.tube(&g, vec3(0.0, 0.0, -p.hx + 0.2), r - 0.4, r - 0.75, 0.2, 10, DARK);
        // Bent fan blades and the spinner.
        let hub = g.transform_point3(vec3(0.0, 0.0, p.hx - 0.7));
        for k in 0..9 {
            let a = k as f32 * TAU / 9.0;
            let q = |rad: f32, ang: f32, z: f32| g.transform_point3(vec3(ang.cos() * rad, ang.sin() * rad, p.hx - 0.7 + z));
            m.tri(hub, q(r - 0.25, a, 0.0), q(r - 0.35, a + 0.3, -0.35 * hash01(k, 3)), if k % 2 == 0 { RAW } else { FRAME });
        }
        m.tube(&g, vec3(0.0, 0.0, p.hx - 0.65), 0.45, 0.0, 0.3, 6, LIGHT);
        // Cowling panels torn half away, flush with the shell.
        for k in 0..3 {
            let a = 0.4 + k as f32 * 0.9;
            let q = |rad: f32, ang: f32, z: f32| g.transform_point3(vec3(ang.cos() * rad, ang.sin() * rad, z));
            let z0 = -1.5 + k as f32 * 1.1;
            teeth(m, q(r + 0.01, a, z0), q(r + 0.01, a, z0 + 1.0), (q(r, a + 0.4, z0) - q(r, a, z0)).normalize(), 0.5, 3, k + 40, RAW);
        }
    } else {
        // A burst cargo container, its roof crushed and one door ajar.
        let (lo, hi) = (b.min + vec3(0.0, 0.4, 0.0), b.max);
        m.cuboid(lo, hi - vec3(0.0, 0.25, 0.0), shade(BOX, if aft { 0.8 } else { 0.7 }));
        for i in 0..9 {
            let x = lo.x + 0.25 + (hi.x - lo.x - 0.5) * i as f32 / 8.0;
            for z in [lo.z - 0.03, hi.z + 0.03] {
                m.cuboid(vec3(x - 0.06, lo.y, z - 0.02), vec3(x + 0.06, hi.y - 0.3, z + 0.02), shade(BOX, 0.6));
            }
        }
        let y = hi.y - 0.25;
        let q = |x: f32, z: f32, dy: f32| vec3(x, y + dy, z);
        crumple(m, [q(lo.x, lo.z, 0.0), q(lo.x, hi.z, 0.0), q(hi.x, hi.z, -0.1), q(hi.x, lo.z, 0.1)], 0.24, mix(BOX, CHAR, 0.4));
        let end = if aft { hi.x } else { lo.x };
        let dir = if aft { 1.0 } else { -1.0 };
        let hinge = vec3(end, lo.y, lo.z);
        plate(
            m,
            [
                hinge,
                hinge + vec3(0.0, hi.y - lo.y - 0.3, 0.0),
                hinge + vec3(dir * 0.25, hi.y - lo.y - 0.35, 1.2),
                hinge + vec3(dir * 0.25, 0.0, 1.2),
            ],
            0.05,
            shade(BOX, 0.7),
        );
        // Its load spilled on the ground: low cases below the knee.
        for k in 0..4 {
            let x = end + dir * (0.7 + k as f32 * 0.55);
            let z = c.z + (hash01(k, 21) - 0.5) * 2.0;
            let gy = w.height(x, z);
            m.cuboid(
                vec3(x - 0.3, gy - 0.1, z - 0.25),
                vec3(x + 0.3, gy + 0.32, z + 0.25),
                if k % 2 == 0 { CARGO } else { shade(CARGO, 0.75) },
            );
        }
    }
}

/// A pine snapped by the slide, its top lying beside it pointing the way
/// the aircraft travelled.
fn stump(m: &mut Mesh, w: &World, b: &Block) {
    let c = (b.min + b.max) * 0.5;
    let (base, top) = (b.min.y + 0.6, b.max.y);
    let (tube, _) = basis(vec3(c.x, base - 0.3, c.z), vec3(c.x, top - 0.3, c.z));
    m.tube(&tube, Vec3::ZERO, 0.25, 0.0, (top - base) * 0.5, 4, BARK);
    let seed = (c.x.abs() * 10.0) as u32;
    for k in 0..6 {
        let a = k as f32 * TAU / 6.0;
        let ring = |r: f32, ang: f32| vec3(c.x + ang.cos() * r, top - 0.32, c.z + ang.sin() * r);
        let tip = vec3(c.x + a.cos() * 0.12, top - 0.02 - 0.25 * hash01(seed, k), c.z + a.sin() * 0.12);
        m.tri(ring(0.25, a), ring(0.25, a + TAU / 6.0), tip, if k % 2 == 0 { SPLINTER } else { BARK });
    }
    // The fallen top: a log below the knee.
    let len = 4.0 + 2.0 * hash01(seed, 9);
    let swing = (hash01(seed, 10) - 0.5) * 0.8;
    let end = vec3(c.x + len * swing.cos(), 0.0, c.z + len * swing.sin());
    let (ax, az) = (c.x + 0.45 * swing.cos(), c.z + 0.45 * swing.sin());
    let a = vec3(ax, w.height(ax, az) + 0.12, az);
    let e = vec3(end.x, w.height(end.x, end.z) + 0.1, end.z);
    rod(m, a, e, 0.17, BARK);
    for k in 1..4 {
        let p = lerp(a, e, k as f32 / 4.0);
        rod(m, p, p + vec3(0.3, -0.1, (hash01(seed, k) - 0.5) * 1.2), 0.04, shade(BARK, 0.8));
    }
}

// ------------------------------------------------------------- the hull

fn cargo_shell(m: &mut Mesh, w: &World) {
    for (b, xs) in STATIONS.windows(2).enumerate() {
        let (x0, x1) = (xs[0] + 0.025, xs[1] - 0.025);
        let xm = (x0 + x1) * 0.5;
        let key = b as u32;
        let folded = xm.abs() > BREAK - 4.0 && xm.abs() < layout::ROOF_TEAR[1];
        for i in 0..6 {
            let zc = (PROFILE[i].0 + PROFILE[i + 1].0) * 0.5;
            let h = hash01(key, i as u32 + 40);
            let col = sooted(shade(SKIN, 0.82 + hash01(key, i as u32) * 0.22), xm, 9.0 + PROFILE[i].1, h);
            if torn(xm, zc) {
                exposed_crown(m, w, x0, x1, i, key * 8 + i as u32);
            } else if !folded && h < 0.13 {
                // A ragged hole punched through the skin, frames beneath.
                exposed_crown(m, w, x0, x1, i, key * 8 + i as u32);
                let q = [crown(w, x0, i, 0.02), crown(w, x1, i, 0.02), crown(w, x1, i + 1, 0.02), crown(w, x0, i + 1, 0.02)];
                holed(m, q, 0.5 + 0.45 * hash01(key, i as u32 + 60), key * 8 + i as u32, col);
            } else if folded {
                // Accordion folds across the break.
                for s in 0..3 {
                    let (ta, tb) = (x0 + (x1 - x0) * s as f32 / 3.0, x0 + (x1 - x0) * (s + 1) as f32 / 3.0);
                    let lift = (hash01(key * 13 + i as u32, s) - 0.3) * 1.1;
                    let q = [crown(w, ta, i, 0.0), crown(w, tb, i, 0.0), crown(w, tb, i + 1, 0.0), crown(w, ta, i + 1, 0.0)];
                    crumple(m, q, lift, mix(col, CHAR, 0.2 * s as f32));
                }
            } else if h < 0.26 {
                // Peeled up from its upper edge.
                let (hinge, free) = if PROFILE[i + 1].1 > PROFILE[i].1 { (i + 1, i) } else { (i, i + 1) };
                exposed_crown(m, w, x0, x1, i, key * 8 + i as u32);
                let up = Vec3::Y * 0.8 + Vec3::Z * zc.signum() * 0.5;
                let q = [crown(w, x0, hinge, 0.0), crown(w, x1, hinge, 0.0), crown(w, x1, free, 0.0) + up, crown(w, x0, free, 0.0) + up];
                plate(m, q, 0.04, col);
                teeth(m, q[3], q[2], (q[3] - q[0]).normalize(), 0.45, 4, key * 9 + i as u32, col);
            } else {
                let q = [crown(w, x0, i, 0.0), crown(w, x1, i, 0.0), crown(w, x1, i + 1, 0.0), crown(w, x0, i + 1, 0.0)];
                plate(m, q, 0.06, col);
            }
        }
        // Where the end sections twist away, a skirt of skin still joins
        // the crown to the straight hull walls.
        if xs[0].abs().max(xs[1].abs()) > BREAK - 2.0 {
            for (i, z) in [(0, -6.42f32), (6, 6.42)] {
                let flat = |x: f32| at(w, x, CROWN, z);
                let (a, b2) = (flat(x0), flat(x1));
                let (c, d) = (crown(w, x1, i, 0.0), crown(w, x0, i, 0.0));
                if (c - b2).length() + (d - a).length() > 0.05 {
                    let col = sooted(SKIN, xm, 8.5, 0.5);
                    m.quad(a, b2, c, d, col);
                    m.quad(d, c, b2, a, shade(col, 0.7));
                }
            }
        }
        for side in [-1.0f32, 1.0] {
            // Flush seams between bays.
            beam(m, at(w, x0, 4.2, side * 6.39), at(w, x0, 7.95, side * 6.39), 0.05, 0.02, FRAME);
        }
    }
    // A ragged rim and a bare frame ring close the crown's aft end.
    for i in 0..6 {
        let (a, b) = (crown(w, -41.95, i, 0.0), crown(w, -41.95, i + 1, 0.0));
        beam(m, a, b, 0.14, 0.18, FRAME);
        teeth(m, a, b, Vec3::NEG_X, 0.7, 3, 300 + i as u32, SKIN);
    }
    interior(m, w);
    // Earth shoved up against the hull's flanks as it slid, below the
    // knee so it never hides or blocks anyone.
    for side in [-1.0f32, 1.0] {
        let mut x = -layout::HALF_LENGTH + 0.5;
        while x < layout::HALF_LENGTH - 1.0 {
            let x1 = x + 2.0;
            if !layout::SIDE_DOORS.iter().any(|d| x1 > d - layout::DOOR_HALF_WIDTH - 0.3 && x < d + layout::DOOR_HALF_WIDTH + 0.3) {
                let k = ((x + 50.0) * 2.0) as u32 + (side > 0.0) as u32 * 1000;
                let (h0, h1) = (0.22 + 0.2 * hash01(k, 1), 0.22 + 0.2 * hash01(k + 4, 1));
                let (r0, r1) = (1.0 + 0.6 * hash01(k, 2), 1.0 + 0.6 * hash01(k + 4, 2));
                let p = |x: f32, y: f32, out: f32| at(w, x, y, side * (layout::HALF_WIDTH + layout::WALL_THICKNESS + 0.03 + out));
                let col = mix([0.33, 0.26, 0.18, 0.0], [0.22, 0.17, 0.12, 0.0], hash01(k, 3));
                let q = [p(x, h0, 0.0), p(x1, h1, 0.0), p(x1, 0.0, r1), p(x, 0.0, r0)];
                if side > 0.0 {
                    m.quad(q[0], q[3], q[2], q[1], col);
                } else {
                    m.quad(q[0], q[1], q[2], q[3], col);
                }
            }
            x = x1;
        }
    }
    for side in [-1.0f32, 1.0] {
        let g = wall(at(w, 28.0, 5.6, side * 6.43), if side < 0.0 { PI } else { 0.0 });
        stencil(m, &g, "FERRITE AIRLIFT", 0.145, WHITE);
        let g = wall(at(w, -31.0, 4.9, side * 6.43), if side < 0.0 { PI } else { 0.0 });
        stencil(m, &g, "FG 042", 0.20, WHITE);
        // The long pale belly band, scorched where the hull burned.
        for (x0, x1) in [(-41.8, -23.1), (-16.9, -3.1), (3.1, 16.9), (23.1, 41.8)] {
            let p = |x, y| at(w, x, y, side * 6.44);
            let col = sooted(WHITE, (x0 + x1) * 0.5, 2.6, 0.9);
            m.quad(p(x0, 2.5), p(x1, 2.5), p(x1, 2.8), p(x0, 2.8), col);
        }
    }
}

/// A crown facet with its skin gone: frames and stringers over the dark
/// space beneath, ragged edges all round.
fn exposed_crown(m: &mut Mesh, w: &World, x0: f32, x1: f32, i: usize, seed: u32) {
    for t in [0.0, 0.5, 1.0] {
        let x = x0 + (x1 - x0) * t;
        beam(m, crown(w, x, i, -0.08), crown(w, x, i + 1, -0.08), 0.12, 0.16, if t == 0.5 { shade(FRAME, 0.7) } else { FRAME });
    }
    for k in [i, i + 1] {
        beam(m, crown(w, x0, k, -0.1), crown(w, x1, k, -0.1), 0.08, 0.1, shade(FRAME, 0.85));
    }
    let along = (crown(w, x1, i, 0.0) - crown(w, x0, i, 0.0)).normalize();
    teeth(m, crown(w, x0, i, 0.0), crown(w, x0, i + 1, 0.0), along, 0.6, 3, seed, SKIN);
    teeth(m, crown(w, x1, i + 1, 0.0), crown(w, x1, i, 0.0), -along, 0.6, 3, seed ^ 99, SKIN);
}

/// The hold: ribs, seats, deck, cabling, the torn roof seen from below and
/// soot round both breaks.
fn interior(m: &mut Mesh, w: &World) {
    let [t0, t1] = layout::ROOF_TEAR;
    for i in 0..=20 {
        let x = -40.0 + i as f32 * 4.0;
        let in_door = layout::SIDE_DOORS.iter().any(|d| (x - d).abs() < layout::DOOR_HALF_WIDTH + 0.25);
        for side in [-1.0f32, 1.0] {
            let y0 = if in_door { layout::DOOR_HEIGHT + 0.12 } else { 0.1 };
            if x.abs() > BREAK - 4.5 && x.abs() < t1 + 1.0 && !in_door {
                // Frames buckled by the break.
                let kink = at(w, x + 0.2 * x.signum(), 4.2, side * 5.78);
                beam(m, at(w, x, y0, side * 5.90), kink, 0.13, 0.13, FRAME);
                beam(m, kink, at(w, x, 7.82, side * 5.90), 0.13, 0.13, FRAME);
            } else {
                beam(m, at(w, x, y0, side * 5.90), at(w, x, 7.82, side * 5.90), 0.13, 0.13, FRAME);
            }
            if torn(x, side * 3.0) {
                // The roof frame over the tear snapped and hangs.
                let stub = at(w, x, 7.80, side * 4.6);
                beam(m, at(w, x, 7.80, side * 5.9), stub, 0.14, 0.14, FRAME);
                beam(m, stub, stub + vec3(0.4 * x.signum(), -2.1, -side * 0.9), 0.12, 0.12, RAW);
            } else {
                beam(m, at(w, x, 7.80, side * 5.9), at(w, x, 7.80, 0.0), 0.14, 0.14, FRAME);
            }
            if !in_door {
                box_at(m, w, vec3(x, 3.3, side * 5.84), vec3(0.20, 0.085, 0.05), DARK);
                // Folded troop seats hug the walls.
                box_at(m, w, vec3(x, 1.55, side * 5.86), vec3(0.65, 0.40, 0.04), SEAT);
            }
        }
    }
    // Cable runs, snapped at both frames and over the tears.
    for side in [-1.0f32, 1.0] {
        for (a, b) in [(-41.8, -10.4), (-9.6, 9.6), (10.4, 41.8)] {
            rod(m, at(w, a, 6.8, side * 5.86), at(w, b, 6.8, side * 5.86), 0.05, DARK);
        }
        let z = side * 4.65;
        let spans: Vec<(f32, f32)> = if torn(-14.0, z) || torn(14.0, z) {
            let s = if torn(-14.0, z) { -1.0 } else { 1.0 };
            if s < 0.0 {
                vec![(-41.8, -t1), (-t0, 41.8)]
            } else {
                vec![(-41.8, t0), (t1, 41.8)]
            }
        } else {
            vec![(-41.8, 41.8)]
        };
        for (a, b) in spans {
            rod(m, at(w, a, 7.45, z), at(w, b, 7.45, z), 0.045, YELLOW);
            for e in [a, b] {
                if e.abs() > t0 - 0.5 && e.abs() < t1 + 0.5 {
                    let dir = if (e - a).abs() < 0.01 { 1.0 } else { -1.0 };
                    rod(m, at(w, e, 7.45, z), at(w, e + dir * 0.6, 5.2, z - side * 0.3), 0.045, YELLOW);
                }
            }
        }
        for x in [-32.0f32, 32.0] {
            let g = wall(at(w, x, 5.5, side * 5.97), if side < 0.0 { 0.0 } else { PI });
            stencil(m, &g, "EXIT", 0.13, [0.32, 0.56, 0.38, 0.18]);
        }
        // Soot climbing the walls round both breaks, between the tears.
        for (k, x) in [8.5f32, 11.0, 13.5, -8.5, -11.0, -13.5].into_iter().enumerate() {
            let k = k as u32;
            let h = hash01(k, (side > 0.0) as u32 + 7);
            let base = at(w, x, 0.6 + h * 1.5, side * 5.975);
            let top = base + vec3(0.6 * (h - 0.5), 4.5 + 2.0 * h, 0.0);
            let soot_col = mix(INSIDE, CHAR, 0.7);
            m.tri(base - Vec3::X * (0.8 + h), base + Vec3::X * (0.8 + h), top + Vec3::X * (1.4 + h), soot_col);
            m.tri(base - Vec3::X * (0.8 + h), top + Vec3::X * (1.4 + h), top - Vec3::X * (1.2 + h), soot_col);
        }
    }
    // Ragged edges of the torn roof, seen from below, and wiring hanging
    // from it, all well above head height.
    for s in [-1.0f32, 1.0] {
        let edge = -s * layout::ROOF_TEAR_EDGE;
        let far = -s * (layout::HALF_WIDTH + layout::WALL_THICKNESS);
        let y = layout::ROOF_HEIGHT - 0.01;
        teeth(
            m,
            at(w, s * t0, y, edge),
            at(w, s * t1, y, edge),
            vec3(0.0, -0.8, -s * 0.5).normalize(),
            0.9,
            7,
            500 + (s > 0.0) as u32,
            INSIDE,
        );
        teeth(m, at(w, s * t0, y, far), at(w, s * t0, y, edge), vec3(s * 0.4, -0.9, 0.0).normalize(), 0.8, 6, 510, INSIDE);
        teeth(m, at(w, s * t1, y, edge), at(w, s * t1, y, far), vec3(-s * 0.4, -0.9, 0.0).normalize(), 0.8, 6, 520, INSIDE);
        for k in 0..3 {
            let x = s * (t0 + 1.0 + k as f32 * 1.6);
            let z = edge + s * 0.1;
            rod(m, at(w, x, y, z), at(w, x + 0.3, 4.2 + 0.4 * k as f32, z - s * 0.4), 0.02, DARK);
        }
    }
    deck(m, w);
}

/// A metal cargo deck laid over the furrow floor: tiles, roller tracks and
/// tie-downs, torn up and missing round the breaks.
fn deck(m: &mut Mesh, w: &World) {
    let rows = 6;
    let half = layout::HALF_WIDTH - 0.04;
    for i in 0..21 {
        let (x0, x1) = (-42.0 + i as f32 * 4.0 + 0.02, -42.0 + (i + 1) as f32 * 4.0 - 0.02);
        let xm = (x0 + x1) * 0.5;
        for j in 0..rows {
            let (z0, z1) = (-half + 2.0 * half * j as f32 / rows as f32 + 0.02, -half + 2.0 * half * (j + 1) as f32 / rows as f32 - 0.02);
            let key = (i * 8 + j) as u32;
            let h = hash01(key, 61);
            let near_break = xm.abs() > 5.0 && xm.abs() < 17.0;
            if (near_break && h < 0.18) || h < 0.03 {
                continue; // torn out: the furrow floor shows through
            }
            let col = mix(shade(DECK, 0.85 + 0.25 * hash01(key, 62)), CHAR, soot(xm, 0.0) * 0.6);
            let lift = if near_break && h < 0.45 { 0.12 + 0.2 * hash01(key, 63) } else { 0.0 };
            let p = |x: f32, z: f32, up: f32| at(w, x, 0.03 + up, z);
            m.quad(p(x0, z0, 0.0), p(x0, z1, 0.0), p(x1, z1, lift), p(x1, z0, lift), col);
            if lift > 0.0 {
                m.quad(p(x1, z0, 0.0), p(x1, z0, lift), p(x1, z1, lift), p(x1, z1, 0.0), DARK);
                teeth(m, p(x1, z0, lift), p(x1, z1, lift), Vec3::X, 0.25, 3, key, RAW);
            }
        }
    }
    for z in [-3.4f32, -1.75, 1.75, 3.4] {
        beam(m, at(w, -41.5, 0.05, z), at(w, 41.5, 0.05, z), 0.06, 0.03, DARK);
        for x in -10..=10 {
            let c = at(w, x as f32 * 4.0, 0.07, z);
            m.quad(c + vec3(-0.14, 0.0, -0.09), c + vec3(-0.14, 0.0, 0.09), c + vec3(0.14, 0.0, 0.09), c + vec3(0.14, 0.0, -0.09), FRAME);
        }
    }
    // Loose straps and spilled small cases, all below the knee.
    for k in 0..10 {
        let x = -38.0 + k as f32 * 8.1 + hash01(k, 70) * 3.0;
        let z = (hash01(k, 71) - 0.5) * 9.0;
        if (x.abs() - layout::BULKHEAD_X).abs() < 1.0 || x.abs() < 2.0 {
            continue;
        }
        let a = hash01(k, 72) * PI;
        let d = vec3(a.cos(), 0.0, a.sin());
        let c = at(w, x, 0.06, z);
        m.quad(
            c - d * 1.2 - d.cross(Vec3::Y) * 0.04,
            c - d * 1.2 + d.cross(Vec3::Y) * 0.04,
            c + d * 1.2 + d.cross(Vec3::Y) * 0.04,
            c + d * 1.2 - d.cross(Vec3::Y) * 0.04,
            YELLOW,
        );
        if k % 3 == 0 {
            box_at(m, w, vec3(x + 0.8, 0.2, z), vec3(0.3, 0.2, 0.25), shade(CARGO, 0.9));
        }
    }
}

fn nose(m: &mut Mesh, w: &World) {
    // The lower nose tore away, leaving a clear ground-level entrance into
    // the hold. The surviving flight deck overhangs it like a broken brow,
    // crumpled where it meets the hull and turned with the forward section.
    let n = |x: f32, y: f32, z: f32| origin(w) + bend(vec3(x, y, z));
    let rings = [(41.9, 6.42, 8.40, 11.70), (47.0, 5.1, 6.0, 10.8), (51.0, 3.4, 5.4, 9.0), (54.0, 0.8, 6.0, 7.7)];
    for (r, p) in rings.windows(2).enumerate() {
        for side in [-1.0f32, 1.0] {
            let (a, b) = (p[0], p[1]);
            let key = r as u32 * 2 + (side > 0.0) as u32;
            let col = sooted(shade(SKIN, 0.9), 45.0, a.2 + 2.0, hash01(key, 1));
            let low = [n(a.0, a.2, side * a.1), n(b.0, b.2, side * b.1), n(b.0, b.3, side * b.1 * 0.65), n(a.0, a.3, side * a.1 * 0.65)];
            if r == 0 {
                crumple(m, low, 0.3, col);
            } else if r == 1 && side < 0.0 {
                // A hole punched in the cheek, ragged and burnt, the frames
                // and a dark flight deck behind it.
                let inner = low.map(|p| p + vec3(0.0, 0.0, 0.35));
                m.quad(inner[0], inner[1], inner[2], inner[3], DARK);
                for t in [0.3, 0.7] {
                    beam(m, lerp(low[0], low[1], t), lerp(low[3], low[2], t), 0.12, 0.12, FRAME);
                }
                holed(m, low, 0.9, 600, mix(col, CHAR, 0.5));
            } else {
                plate(m, low, 0.1, col);
            }
            plate(
                m,
                [n(a.0, a.3, side * a.1 * 0.65), n(b.0, b.3, side * b.1 * 0.65), n(b.0, b.3, 0.0), n(a.0, a.3, 0.0)],
                0.1,
                mix(LIGHT, CHAR, 0.25),
            );
            teeth(m, low[0], low[1], -Vec3::Y, 0.55, 4, 610 + key, RUST);
        }
    }
    // Cockpit windows: steep, framed, and one blown out.
    for side in [-1.0f32, 1.0] {
        for k in 0..3 {
            let x0 = 45.2 + k as f32 * 1.75;
            let z0 = (4.6 - (x0 - 45.2) * 0.25) * side;
            let q =
                [n(x0, 8.25, z0), n(x0 + 1.45, 7.85, z0 - side * 0.45), n(x0 + 1.45, 9.0, z0 - side * 1.1), n(x0, 9.6, z0 - side * 0.65)];
            if side < 0.0 && k == 1 {
                plate(m, q, 0.035, DARK);
                teeth(m, q[3], q[2], (q[0] - q[3]).normalize(), 0.35, 3, 620, RAW);
            } else {
                plate(m, q, 0.035, [0.08, 0.16, 0.19, 0.08]);
            }
        }
        beam(m, n(42.0, 5.0, side * 5.2), n(48.0, 6.6, side * 3.4), 0.18, 0.18, FRAME);
    }
}

fn tail(m: &mut Mesh, w: &World) {
    // The aft section's tall fin leans over and its tip tore away.
    let lean = |p: Vec3| {
        let pivot = vec3(p.x, 10.2, 0.0);
        let r = Quat::from_rotation_x(0.13);
        origin(w) + bend(pivot + r * (p - pivot))
    };
    for side in [-1.0f32, 1.0] {
        let z = side * 0.52;
        let col = sooted(shade(SKIN, if side < 0.0 { 0.85 } else { 1.05 }), -36.0, 12.0, 0.4);
        let q =
            [lean(vec3(-42.0, 10.2, z)), lean(vec3(-41.2, 30.8, z * 0.5)), lean(vec3(-30.0, 27.6, z * 0.5)), lean(vec3(-25.0, 10.2, z))];
        plate(m, q, 0.08, col);
        teeth(m, q[1], q[2], (q[1] - q[0]).normalize(), 1.8, 6, 700 + (side > 0.0) as u32, col);
        let g = Mat4::from_translation(lean(vec3(-35.6, 20.3, z + side * 0.06))) * Mat4::from_rotation_x(0.13);
        let g = g * Mat4::from_rotation_y(if side < 0.0 { PI } else { 0.0 });
        stencil(m, &g, "042", 0.37, WHITE);
        let p = |x, y| lean(vec3(x, y, z + side * 0.08));
        m.quad(p(-40.2, 25.5), p(-31.8, 23.3), p(-31.8, 24.3), p(-40.2, 26.5), YELLOW);
        beam(m, lean(vec3(-41.2, 10.4, side * 0.58)), lean(vec3(-40.6, 28.6, side * 0.34)), 0.11, 0.11, DARK);
    }
    // Elevators: the starboard one intact, the port one snapped at half
    // span and drooping, its outer half gone.
    for side in [-1.0f32, 1.0] {
        let tp = |x: f32, y: f32, out: f32| {
            let (y, out) = if side < 0.0 && out > 1.0 {
                let (s, c) = (-0.42f32).sin_cos();
                (15.0 + (y - 15.0) * c + (out - 1.0) * s, 1.0 + (out - 1.0) * c - (y - 15.0) * s)
            } else {
                (y, out)
            };
            lean(vec3(x, y, side * out))
        };
        let reach = if side < 0.0 { 9.0 } else { 17.5 };
        let q = [
            tp(-39.0, 15.0, 0.8),
            tp(-40.0 - 2.5 * reach / 17.5, 15.7, reach),
            tp(-41.5 - 6.5 * reach / 17.5, 15.0, reach),
            tp(-44.0, 14.4, 0.8),
        ];
        plate(m, q, 0.35, SKIN);
        if side < 0.0 {
            teeth(m, q[1], q[2], (q[1] - q[0]).normalize(), 0.9, 5, 720, RAW);
            beam(m, tp(-40.3, 15.2, 1.0), tp(-40.6, 15.2, reach + 1.0), 0.18, 0.12, FRAME);
        }
    }
    // The broken aft rim exposes longitudinal stringers.
    for side in [-1.0f32, 1.0] {
        for k in 0..6 {
            let y = 4.2 + k as f32 * 1.1;
            beam(
                m,
                at(w, -41.9, y, side * 6.20),
                at(w, -45.0 - hash01(k, 9) * 1.6, y + 0.3, side * (6.0 - k as f32 * 0.2)),
                0.10,
                0.10,
                FRAME,
            );
        }
    }
}

fn engine(m: &mut Mesh, g: Mat4, burnt: bool) {
    let body = if burnt { mix(SKIN, CHAR, 0.6) } else { SKIN };
    m.tube(&g, Vec3::ZERO, 2.15, 1.83, 3.7, 12, body);
    m.tube(&g, vec3(0.0, 0.0, 3.55), 2.24, 1.82, 0.18, 12, if burnt { CHAR } else { LIGHT });
    m.tube(&g, vec3(0.0, 0.0, -3.7), 1.97, 1.3, 0.2, 12, DARK);
    let center = g.transform_point3(vec3(0.0, 0.0, 2.7));
    for k in 0..12 {
        let a = k as f32 * TAU / 12.0;
        let p = |rad: f32, angle: f32| g.transform_point3(vec3(angle.cos() * rad, angle.sin() * rad, 2.7));
        m.tri(center, p(1.7, a), p(1.65, a + 0.32), shade(FRAME, 0.75));
    }
    m.tube(&g, vec3(0.0, 0.0, 2.9), 0.45, 0.0, 0.50, 8, DARK);
    if burnt {
        for k in 0..5 {
            let a = k as f32 * 0.6;
            let p = |r: f32, zz: f32| g.transform_point3(vec3(a.cos() * r, a.sin() * r, zz));
            m.tri(p(2.2, 3.8), p(2.5, 5.0), p(1.95, 2.7), RUST);
        }
    }
}

/// One wing in hull space. Planform coordinates are x, the height of the
/// level wing and the distance out from the centreline; `dihedral` swings
/// the whole wing about its root, `kink` bends the outer part down.
struct Wing {
    side: f32,
    dihedral: f32,
    reach: f32,
    kink: Option<(f32, f32)>,
}

impl Wing {
    fn at(&self, x: f32, y: f32, out: f32) -> Vec3 {
        let (mut y, mut out) = (y, out);
        if let Some((t, a)) = self.kink {
            let (ky, kz) = (9.1 - 2.4 * t, 6.0 + 40.0 * t);
            if out > kz {
                let (dy, dz) = (y - ky, out - kz);
                let (s, c) = (-a).sin_cos();
                y = ky + dy * c + dz * s;
                out = kz + dz * c - dy * s;
            }
        }
        let (dy, dz) = (y - 9.1, out - 6.4);
        let (s, c) = self.dihedral.sin_cos();
        vec3(x, 9.1 + dy * c + dz * s, self.side * (6.4 + dz * c - dy * s))
    }
    /// Leading (f = 0) to trailing (f = 1) edge at span fraction t.
    fn chord(&self, t: f32, f: f32) -> Vec3 {
        let (xl, yl) = (5.0 - 26.0 * t, 9.7 - 2.7 * t);
        let (xt, yt) = (-10.5 - 24.5 * t, 8.5 - 2.1 * t);
        self.at(xl + (xt - xl) * f, yl + (yt - yl) * f, 6.0 + 40.0 * t)
    }
}

fn wings(m: &mut Mesh, w: &World) {
    // The port wing folded up at the root as the hull rolled, and its outer
    // third tore away; the starboard wing's tip crumpled down. Both lost
    // their outboard engines - the starboard one lies beneath it.
    let o = origin(w);
    for wing in [
        Wing { side: -1.0, dihedral: 0.35, reach: 0.6, kink: None },
        Wing { side: 1.0, dihedral: 0.15, reach: 1.0, kink: Some((0.74, 0.42)) },
    ] {
        let port = wing.side < 0.0;
        let stations: Vec<f32> =
            [0.0, 0.12, 0.225, 0.36, 0.5, 0.6, 0.74, 0.87, 1.0].into_iter().filter(|&t| t <= wing.reach + 1e-4).collect();
        for (si, ts) in stations.windows(2).enumerate() {
            for (fi, fs) in [(0.0f32, 0.5f32), (0.5, 1.0)].into_iter().enumerate() {
                let key = si as u32 * 4 + fi as u32 + if port { 0 } else { 100 };
                let h = hash01(key, 33);
                let mut q = [wing.chord(ts[0], fs.0), wing.chord(ts[1], fs.0), wing.chord(ts[1], fs.1), wing.chord(ts[0], fs.1)];
                if (q[1] - q[0]).cross(q[3] - q[0]).y < 0.0 {
                    q = [q[1], q[0], q[3], q[2]];
                }
                let q = q.map(|p| o + p);
                let burnt = port && ts[0] < 0.4;
                let col = mix(shade(SKIN, 0.85 + 0.2 * hash01(key, 34)), CHAR, if burnt { 0.45 } else { 0.0 });
                if h < 0.12 && si > 0 {
                    // Skin gone: ribs and spar.
                    for t in [0.0, 1.0] {
                        let (a, b) = (lerp(q[0], q[1], t), lerp(q[3], q[2], t));
                        beam(m, a, b, 0.1, 0.25, FRAME);
                    }
                    beam(m, lerp(q[0], q[3], 0.5), lerp(q[1], q[2], 0.5), 0.2, 0.3, shade(FRAME, 0.8));
                } else if !port && fi == 1 && (0.36..0.6).contains(&ts[0]) {
                    // A flap hanging off its hinges.
                    let droop = Vec3::NEG_Y * 1.3 + Vec3::NEG_X * 0.2;
                    plate(m, [q[0], q[1], q[2] + droop, q[3] + droop * 0.7], 0.1, shade(col, 0.9));
                } else if !port && ts[0] >= 0.74 {
                    crumple(m, q, 0.35, col);
                    crumple(m, [q[3], q[2], q[1], q[0]], 0.2, shade(col, 0.6));
                } else {
                    plate(m, q, 0.42, col);
                }
            }
        }
        // The torn end of the port wing: ragged skin and snapped spars.
        if port {
            let (a, b) = (o + wing.chord(wing.reach, 0.0), o + wing.chord(wing.reach, 1.0));
            let out = (o + wing.chord(wing.reach, 0.5) - (o + wing.chord(wing.reach - 0.05, 0.5))).normalize();
            teeth(m, a, b, out, 1.4, 7, 800, SKIN);
            teeth(m, b - Vec3::Y * 0.35, a - Vec3::Y * 0.35, out, 1.0, 6, 801, CHAR);
            for f in [0.22, 0.68] {
                let root = o + wing.chord(wing.reach, f);
                beam(m, root - out * 0.5, root + out * 1.2 + Vec3::Y * 0.2, 0.22, 0.4, RAW);
            }
        }
        // Faded stripe along the inner wing.
        let s = |t: f32, f: f32| o + wing.chord(t, f) + Vec3::Y * 0.47;
        let stripe = [s(0.5, 0.3), s(0.55, 0.3), s(0.55, 0.62), s(0.5, 0.62)];
        if wing.reach > 0.55 {
            m.quad(stripe[0], stripe[1], stripe[2], stripe[3], YELLOW);
        }
        // Inboard engine on its pylon; the outboard pylon is a torn stub.
        let hang = |x: f32, y: f32, out: f32| o + wing.at(x, y, out);
        let tilt = Mat4::from_rotation_x(-wing.side * wing.dihedral);
        beam(m, hang(-4.5, 8.3, 15.0), hang(-5.5, 10.2, 15.0), 0.75, 1.1, FRAME);
        let g = Mat4::from_translation(hang(-5.5, 7.2, 15.0)) * tilt * Mat4::from_rotation_y(FRAC_PI_2);
        engine(m, g, port);
        if port {
            // Soot streaked back over the wing from the burnt engine.
            let t = 0.225;
            let q = [s(t - 0.03, 0.45), s(t + 0.03, 0.45), s(t + 0.05, 1.0), s(t - 0.05, 1.0)];
            m.quad(q[0], q[1], q[2], q[3], CHAR);
        }
        let stub = hang(-14.0, 7.6, 26.0);
        beam(m, hang(-14.0, 8.9, 26.0), stub, 0.6, 0.9, shade(FRAME, 0.8));
        teeth(m, stub + Vec3::X * 0.45, stub - Vec3::X * 0.45, Vec3::NEG_Y, 0.5, 4, 820 + port as u32, RAW);
        rod(m, stub, stub + vec3(0.3, -1.6, 0.2), 0.03, DARK);
    }
}

/// Wreckage too low to hide anyone, strewn along the furrow: torn skin,
/// tipped seats and splinters of aluminium.
fn strewn(m: &mut Mesh, w: &World) {
    let lie = |x: f32, z: f32, lift: f32| vec3(x, w.height(x, z) + lift, z);
    // Torn skin panels lying flat.
    let panels: [(f32, f32, f32, f32, f32); 8] = [
        (-52.0, -2.5, 2.6, 1.3, 0.3),
        (-63.0, 3.0, 2.0, 1.1, 1.9),
        (-71.0, -1.0, 3.0, 1.4, 0.7),
        (-86.0, 2.0, 1.8, 1.0, 2.6),
        (-24.0, -16.0, 2.4, 1.2, 1.2),
        (48.0, 3.0, 2.2, 1.2, 2.2),
        (-14.0, 14.5, 1.6, 0.9, 0.4),
        (30.0, -14.0, 2.0, 1.0, 1.0),
    ];
    for (k, (x, z, l, wd, yaw)) in panels.into_iter().enumerate() {
        let (d, e) = (vec3(yaw.cos(), 0.0, yaw.sin()), vec3(-yaw.sin(), 0.0, yaw.cos()));
        let p = |u: f32, v: f32, up: f32| {
            let q = vec3(x, 0.0, z) + d * u + e * v;
            lie(q.x, q.z, 0.07 + up)
        };
        // A 2 x 2 grid draped over the ground, so it never bridges a hollow.
        let col = if k % 3 == 0 { mix(SKIN, CHAR, 0.5) } else { shade(SKIN, 0.9) };
        let lift = |i: usize, j: usize| 0.12 * hash01(900 + k as u32, (i * 3 + j) as u32);
        let v = |i: usize, j: usize| p(-l + l * i as f32, -wd + wd * j as f32, lift(i, j));
        for i in 0..2 {
            for j in 0..2 {
                crumple(m, [v(i, j), v(i, j + 1), v(i + 1, j + 1), v(i + 1, j)], 0.04, col);
            }
        }
        teeth(m, v(2, 0), v(2, 2), d, 0.45, 3, 900 + k as u32, RAW);
    }
    // Rows of troop seats, thrown out and lying on their backs.
    for (k, (x, z, yaw)) in [(-58.0f32, -1.0f32, 0.4f32), (-76.0, 2.5, 1.2), (52.0, -2.0, 2.3)].into_iter().enumerate() {
        // Bedded in at its lowest corner, so no end floats over a slope.
        let low = [(-1.0f32, -0.3f32), (1.0, -0.3), (-1.0, 0.3), (1.0, 0.3)]
            .into_iter()
            .map(|(u, v)| w.height(x + u * yaw.cos() + v * yaw.sin(), z - u * yaw.sin() + v * yaw.cos()))
            .fold(f32::MAX, f32::min);
        let g = Mat4::from_translation(vec3(x, low, z)) * Mat4::from_rotation_y(yaw);
        m.obox(&g, vec3(0.0, 0.1, 0.0), vec3(1.0, 0.1, 0.3), FRAME);
        for s in 0..3 {
            let sx = -0.65 + s as f32 * 0.65;
            m.obox(&g, vec3(sx, 0.24, -0.05), vec3(0.28, 0.07, 0.24), if k == 1 { mix(SEAT, CHAR, 0.6) } else { SEAT });
        }
    }
    // A trail of small fragments, densest behind the tail.
    for k in 0..44u32 {
        let t = hash01(k, 950);
        let aft = k % 4 != 0;
        let x = if aft { -44.0 - t * 52.0 } else { 44.0 + t * 20.0 };
        let z = (hash01(k, 951) - 0.5) * (6.0 + 10.0 * t);
        let a = hash01(k, 952) * TAU;
        let r = 0.25 + 0.45 * hash01(k, 953);
        let c = lie(x, z, 0.06);
        let p = |ang: f32, rr: f32| c + vec3(ang.cos() * rr, 0.06 * hash01(k, 954), ang.sin() * rr);
        m.tri(
            p(a, r),
            p(a + 2.2, r * 0.7),
            p(a + 4.1, r * 0.9),
            if k % 5 == 0 {
                RAW
            } else if k % 3 == 0 {
                CHAR
            } else {
                shade(SKIN, 0.8)
            },
        );
    }
}

pub fn build(m: &mut Mesh, w: &World) {
    if w.map != layout::MAP_ID {
        return;
    }
    cargo_shell(m, w);
    nose(m, w);
    tail(m, w);
    wings(m, w);
    strewn(m, w);
}

// ------------------------------------------------------------ the ground

/// How strongly (x, z) is churned earth, and how strongly scorched.
fn scar(x: f32, z: f32) -> (f32, f32) {
    let (ax, az) = (x.abs(), z.abs());
    // The slide path runs far behind the tail; ahead of the nose the earth
    // piled into the two bow waves.
    let length = if x < 0.0 { 100.0 } else { 62.0 };
    let furrow = hump(az, 0.0, 6.5) * smoothstep(42.0, 47.0, ax) * (1.0 - smoothstep(length - 22.0, length, ax));
    let waves = hump(az, 10.5, 6.5) * hump(ax, 58.0, 9.0) * 0.9;
    let flanks = (1.0 - smoothstep(7.0, 15.0, az)) * (1.0 - smoothstep(44.0, 50.0, ax)) * 0.85;
    let earth = furrow.max(waves).max(flanks);
    let mut burn: f32 = 0.0;
    for (px, pz, r) in [
        (layout::NACELLE.x, layout::NACELLE.z, 7.0),
        (-layout::NACELLE.x, -layout::NACELLE.z, 5.5),
        (BREAK, 8.0, 7.0),
        (BREAK, -8.0, 7.5),
        (-BREAK, -8.0, 5.0),
        (-6.0, -16.0, 6.0),
    ] {
        let d = ((x - px).powi(2) + (z - pz).powi(2)).sqrt();
        burn = burn.max(1.0 - smoothstep(r * 0.3, r, d));
    }
    (earth, burn)
}

/// Ridgeline's ground colour round the crash: torn earth along the furrow,
/// streaked by the slide, and char round everything that burned.
pub fn ground_tint(col: [f32; 3], c: Vec3) -> [f32; 3] {
    let (earth, burn) = scar(c.x, c.z);
    if earth <= 0.0 && burn <= 0.0 {
        return col;
    }
    let streak = 0.5 + 0.5 * (c.z * 2.7 + (c.x * 0.13).sin() * 1.5).sin();
    let soil = mix([0.34, 0.26, 0.18, 0.0], [0.2, 0.15, 0.11, 0.0], streak);
    let out = mix([col[0], col[1], col[2], 0.0], soil, earth);
    let out = mix(out, [0.09, 0.08, 0.07, 0.0], burn * 0.85);
    [out[0], out[1], out[2]]
}

/// No grass grows on the torn and burnt ground.
pub fn bare(x: f32, z: f32) -> bool {
    let (earth, burn) = scar(x, z);
    earth > 0.45 || burn > 0.4
}

/// The wind gust cycle is shared with the positional rattle's seven-second
/// cadence. No gameplay state is driven by this local ambient clock.
pub fn gust(time: f32) -> f32 {
    (time * TAU / 7.0).sin()
}

pub fn atmosphere(m: &mut Mesh, alpha: &mut Mesh, add: &mut Mesh, w: &World, time: f32) {
    if w.map != layout::MAP_ID {
        return;
    }
    for (i, (x, y, z)) in [(12.0f32, 6.3f32, 6.52f32), (-11.0, 6.5, -6.52)].into_iter().enumerate() {
        let flap = gust(time + i as f32 * 1.4) * 0.12 + (time * 5.1 + i as f32).sin() * 0.025;
        let side = z.signum();
        let g = Mat4::from_translation(at(w, x, y, z)) * Mat4::from_rotation_x(flap * side);
        m.obox(&g, vec3(0.0, 0.70, side * 0.05), vec3(1.12, 0.70, 0.045), LIGHT);
        for xx in [-0.8, 0.0, 0.8] {
            m.obox(&g, vec3(xx, 0.7, side * 0.10), vec3(0.025, 0.6, 0.025), FRAME);
        }
    }
    // Two surviving emergency fixtures, including one deep in the hold.
    for x in [-12.0f32, 31.0] {
        let p = at(w, x, 6.5, -5.68);
        m.cuboid(p - vec3(0.28, 0.15, 0.18), p + vec3(0.28, 0.15, 0.18), DARK);
        let phase = (time * 1.6 + x * 0.02).rem_euclid(1.0);
        let flash = if phase < 0.12 || (0.22..0.32).contains(&phase) { 1.0 } else { 0.08 };
        let col = [1.0, 0.14, 0.045, flash * 0.9];
        add.cuboid(p - vec3(0.12, 0.10, 0.20), p + vec3(0.12, 0.10, 0.20), col);
        if flash > 0.5 {
            cross(add, p, 0.65, 0.65, [1.0, 0.12, 0.035, 0.48]);
        }
    }
    // Thin smoke still rising from both fallen engines and the burnt one on
    // the port wing. It starts above head height and is mirrored at ground
    // level, so it hides nobody on one side only.
    let o = origin(w);
    let wing = Wing { side: -1.0, dihedral: 0.35, reach: 0.6, kink: None };
    let sources = [
        vec3(layout::NACELLE.x, w.height(layout::NACELLE.x, layout::NACELLE.z) + 3.4, layout::NACELLE.z),
        vec3(-layout::NACELLE.x, w.height(-layout::NACELLE.x, -layout::NACELLE.z) + 3.4, -layout::NACELLE.z),
        o + wing.at(-8.5, 8.6, 15.0),
    ];
    for (i, s) in sources.into_iter().enumerate() {
        for k in 0..2 {
            let t = (time * 0.07 + k as f32 * 0.5 + i as f32 * 0.31).rem_euclid(1.0);
            let drift = vec3(1.5 + gust(time) * 0.6, 0.0, 0.8) * t * 4.0;
            let size = 0.8 + t * 2.2;
            let p = s + Vec3::Y * (size * 0.6 + t * 9.0) + drift;
            soft(alpha, p, size, size, [0.2, 0.19, 0.18, 0.32 * (1.0 - t)]);
        }
    }
    // Sunlight falling through the roof tears into the hold.
    for s in [-1.0f32, 1.0] {
        let p = at(w, s * 14.5, 4.2, -s * 2.6);
        cross(add, p, 1.6, 3.9, [1.0, 0.93, 0.75, 0.05]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid(mesh: Mesh, budget: usize) {
        assert!(mesh.triangles() < budget, "{} triangles, budget {budget}", mesh.triangles());
        for (v, i) in mesh.finish() {
            assert!(i.iter().all(|&n| (n as usize) < v.len()));
            assert!(v.iter().all(|v| v.pos.iter().chain(v.nrm.iter()).chain(v.col.iter()).all(|f| f.is_finite())));
        }
    }

    fn wreck_mesh(w: &World) -> Mesh {
        let mut m = Mesh::default();
        build(&mut m, w);
        for b in w.blocks.iter().filter(|b| layout::owns(b)) {
            block(&mut m, w, b);
        }
        m
    }

    #[test]
    fn cargo_aircraft_has_finite_bounded_silhouette_and_moving_ambient_parts() {
        for seed in [0, 17, 0xFE221, u64::MAX] {
            let w = World::with_map(seed, layout::MAP_ID);
            // One budget for the whole wreck: the art above and below body
            // height plus everything drawn over its collision blocks. Before
            // the crash damage the two came to about 20,600 triangles; now
            // about 21,300.
            let m = wreck_mesh(&w);
            assert!(m.triangles() > 2500);
            let mut verts = m.v.iter().chain(m.chunks.iter().flat_map(|(v, _)| v.iter()));
            assert!(verts.any(|v| v.pos[1] > layout::floor(&w) + 30.0), "the tall tail must still read from afar");
            valid(m, 28000);
            let mut poses = Vec::new();
            for t in [0.0, 0.4, 2.3, 10000.0] {
                let (mut m, mut alpha, mut add) = (Mesh::default(), Mesh::default(), Mesh::default());
                atmosphere(&mut m, &mut alpha, &mut add, &w, t);
                poses.push(m.v.iter().map(|v| v.pos).collect::<Vec<_>>());
                valid(m, 500);
                valid(alpha, 100);
                valid(add, 500);
            }
            assert_ne!(poses[0], poses[1]);
        }
    }

    /// Anything drawn between knee and jump height, over ground a player
    /// can stand on, must lie on or within 0.3 m of real collision. Higher
    /// parts are out of reach; lower ones are stepped over.
    #[test]
    fn nothing_at_body_height_lacks_collision() {
        for seed in [0, 17, 0xFE221] {
            let w = World::with_map(seed, layout::MAP_ID);
            let near: Vec<&Block> =
                w.blocks.iter().filter(|b| b.max.x > -110.0 && b.min.x < 110.0 && b.max.z > -60.0 && b.min.z < 60.0).collect();
            let stand = |p: Vec3| {
                let ground = w.height(p.x, p.z);
                near.iter()
                    .filter(|b| {
                        p.x > b.min.x && p.x < b.max.x && p.z > b.min.z && p.z < b.max.z && b.max.y <= p.y + 0.01 && b.max.y - ground < 2.7
                    })
                    .fold(ground, |s, b| s.max(b.max.y))
            };
            let solid = |p: Vec3| near.iter().any(|b| (p - p.clamp(b.min, b.max)).length() < 0.3);
            let mut checked = 0;
            for (v, i) in wreck_mesh(&w).finish() {
                let pos = |n: u16| Vec3::from(v[n as usize].pos);
                for t in i.chunks(3) {
                    let (a, b, c) = (pos(t[0]), pos(t[1]), pos(t[2]));
                    for p in [a, b, c, (a + b + c) / 3.0, (a + b) * 0.5, (b + c) * 0.5, (c + a) * 0.5] {
                        let rel = p.y - stand(p);
                        if rel > 0.45 && rel < 3.2 {
                            checked += 1;
                            assert!(solid(p), "seed {seed:x}: visual-only geometry at body height, {rel:.2} m up at {p:?}");
                        }
                    }
                }
            }
            assert!(checked > 1000);
        }
    }

    #[test]
    fn scar_tint_and_bare_ground_follow_the_furrow() {
        let grass = [0.3, 0.45, 0.2];
        assert_eq!(ground_tint(grass, vec3(0.0, 0.0, 80.0)), grass);
        assert_ne!(ground_tint(grass, vec3(-70.0, 0.0, 0.0)), grass);
        assert!(bare(-70.0, 0.0) && bare(layout::NACELLE.x, layout::NACELLE.z) && !bare(0.0, 60.0));
    }
}
