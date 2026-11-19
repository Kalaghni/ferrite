//! Cover with some shape to it. The sim sees every piece of cover as a box;
//! this draws what the box stands for: corrugated shipping containers,
//! sandbag walls laid bag by bag, plank crates nailed into frames, cast
//! concrete with worn edges and weathering, jersey barriers, steel columns,
//! corrugated roofs, and round tanks and chimneys. Everything stays inside
//! its box, give or take a centimetre or two, so what is seen is what stops
//! a bullet.

use super::hash01;
use crate::gfx::{rgb, shade, Col, Mesh};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::{Block, BlockKind, World};
use std::f32::consts::{FRAC_PI_2, TAU};

const CONTAINER: [[f32; 3]; 4] = [[0.66, 0.27, 0.16], [0.18, 0.46, 0.48], [0.80, 0.60, 0.18], [0.34, 0.38, 0.47]];
const TANK: [[f32; 3]; 3] = [[0.70, 0.68, 0.62], [0.44, 0.52, 0.46], [0.58, 0.40, 0.28]];
const LENS: Col = [1.0, 0.93, 0.78, 0.6];
const HAZARD: Col = [0.85, 0.38, 0.12, 0.0];

/// A flat quad with one normal and a colour per corner.
fn face(m: &mut Mesh, p: [Vec3; 4], n: Vec3, c: [Col; 4]) {
    m.quad_nc(p, [n; 4], c);
}

/// Shade at height `y` along stops `(y, k)` given in rising y.
fn tone_at(stops: &[(f32, f32)], y: f32) -> f32 {
    if y <= stops[0].0 {
        return stops[0].1;
    }
    for s in stops.windows(2) {
        if y <= s[1].0 {
            let t = (y - s[0].0) / (s[1].0 - s[0].0).max(1e-4);
            return s[0].1 + (s[1].1 - s[0].1) * t;
        }
    }
    stops[stops.len() - 1].1
}

/// Weathering for something standing on the ground at `ground` with its top
/// at `top`: soaked and splashed dark at the foot, clean in the middle, a
/// little stained just under the top.
fn weather(ground: f32, top: f32) -> Vec<(f32, f32)> {
    let mut s = vec![(ground - 0.3, 0.66), (ground + 0.06, 0.72), (ground + 0.8, 0.95), (top - 0.4, 1.0), (top - 0.1, 0.9)];
    s.sort_by(|p, q| p.0.total_cmp(&q.0));
    s
}

/// The rectangle `min`..`max` seen from above, its corners cut back by `ch`.
fn octagon(min: Vec3, max: Vec3, ch: f32) -> Vec<(f32, f32)> {
    let (x0, x1, z0, z1) = (min.x, max.x, min.z, max.z);
    vec![(x0 + ch, z0), (x1 - ch, z0), (x1, z0 + ch), (x1, z1 - ch), (x1 - ch, z1), (x0 + ch, z1), (x0, z1 - ch), (x0, z0 + ch)]
}

/// An upright prism over the convex outline `ring` (x, z) from `y0` to
/// `y1`: its walls cut into bands at the `stops` and shaded along them,
/// capped top and bottom.
fn prism(m: &mut Mesh, ring: &[(f32, f32)], y0: f32, y1: f32, col: Col, stops: &[(f32, f32)]) {
    let n = ring.len();
    let c = ring.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
    let c = (c.0 / n as f32, c.1 / n as f32);
    let mut ys = vec![y0];
    ys.extend(stops.iter().map(|s| s.0).filter(|&y| y > y0 + 0.02 && y < y1 - 0.02));
    ys.push(y1);
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        let mut nn = vec3(b.1 - a.1, 0.0, a.0 - b.0).normalize_or_zero();
        if nn.x * ((a.0 + b.0) * 0.5 - c.0) + nn.z * ((a.1 + b.1) * 0.5 - c.1) < 0.0 {
            nn = -nn;
        }
        for w in ys.windows(2) {
            let (ka, kb) = (shade(col, tone_at(stops, w[0])), shade(col, tone_at(stops, w[1])));
            face(m, [vec3(a.0, w[0], a.1), vec3(b.0, w[0], b.1), vec3(b.0, w[1], b.1), vec3(a.0, w[1], a.1)], nn, [ka, ka, kb, kb]);
        }
    }
    for (y, up) in [(y1, 1.0f32), (y0, -1.0)] {
        let k = shade(col, tone_at(stops, y));
        let mid = vec3(c.0, y, c.1);
        for i in 0..n {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            m.tri_n(mid, vec3(a.0, y, a.1), vec3(b.0, y, b.1), vec3(0.0, up, 0.0), k);
        }
    }
}

/// A convex profile `(across, y)` swept straight along x (`along_x`) or z
/// from `a0` to `a1`, centred across on `mid`; shaded by height along
/// `stops` and capped at both ends.
#[allow(clippy::too_many_arguments)]
fn sweep(m: &mut Mesh, along_x: bool, a0: f32, a1: f32, mid: f32, prof: &[(f32, f32)], col: Col, stops: &[(f32, f32)]) {
    let at = |a: f32, p: (f32, f32)| if along_x { vec3(a, p.1, mid + p.0) } else { vec3(mid + p.0, p.1, a) };
    let dir = if along_x { Vec3::X } else { Vec3::Z };
    let n = prof.len();
    let c = prof.iter().fold((0.0, 0.0), |s, p| (s.0 + p.0, s.1 + p.1));
    let c = (c.0 / n as f32, c.1 / n as f32);
    let k = |y: f32| shade(col, tone_at(stops, y));
    for i in 0..n {
        let (p, q) = (prof[i], prof[(i + 1) % n]);
        let (mut nu, mut ny) = (q.1 - p.1, p.0 - q.0);
        if nu * ((p.0 + q.0) * 0.5 - c.0) + ny * ((p.1 + q.1) * 0.5 - c.1) < 0.0 {
            (nu, ny) = (-nu, -ny);
        }
        let nn = if along_x { vec3(0.0, ny, nu) } else { vec3(nu, ny, 0.0) }.normalize_or_zero();
        face(m, [at(a0, p), at(a0, q), at(a1, q), at(a1, p)], nn, [k(p.1), k(q.1), k(q.1), k(p.1)]);
    }
    for (a, s) in [(a0, -1.0f32), (a1, 1.0)] {
        let mid = at(a, c);
        for i in 0..n {
            let (p, q) = (prof[i], prof[(i + 1) % n]);
            m.tri_n(mid, at(a, p), at(a, q), dir * s, k(c.1));
        }
    }
}

/// Upright cylinder from `y0` to `y1`.
fn column(m: &mut Mesh, x: f32, z: f32, y0: f32, y1: f32, r: f32, segs: usize, col: Col) {
    let g = Mat4::from_translation(vec3(x, (y0 + y1) * 0.5, z)) * Mat4::from_rotation_x(-FRAC_PI_2);
    m.tube(&g, Vec3::ZERO, r, 0.0, (y1 - y0) * 0.5, segs, col);
}

// ------------------------------------------------------------- concrete

pub fn concrete(m: &mut Mesh, w: &World, b: &Block, j: f32) {
    let k = j * (0.92 + 0.05 * b.tint as f32);
    let col = rgb(0.63 * k, 0.61 * k, 0.57 * k);
    let size = b.max - b.min;
    let (thin, long) = (size.x.min(size.z), size.x.max(size.z));
    let c = (b.min + b.max) * 0.5;
    let ground = w.height(c.x, c.z).max(b.min.y);
    if b.tint == 3 && thin <= 0.7 && b.max.y - ground <= 1.4 && long >= 3.0 {
        barrier(m, b, ground, col);
        return;
    }
    let stops = weather(ground, b.max.y);
    prism(m, &octagon(b.min, b.max, (thin * 0.2).min(0.07)), b.min.y, b.max.y, col, &stops);
    // The weathered coping along the top.
    m.cuboid(vec3(b.min.x - 0.04, b.max.y - 0.12, b.min.z - 0.04), b.max + vec3(0.04, 0.02, 0.04), rgb(0.5 * k, 0.48 * k, 0.45 * k));
    // A plinth course at the foot of a wall.
    if size.y > 1.5 && b.min.y < ground + 0.05 {
        let g = b.min.y + 0.6;
        let plinth = rgb(0.55 * k, 0.53 * k, 0.5 * k);
        let stops = [(g, 0.8), (g + 0.35, 0.95)];
        prism(m, &octagon(b.min - vec3(0.06, 0.0, 0.06), b.max + vec3(0.06, 0.0, 0.06), 0.08), g, g + 0.35, plinth, &stops);
    }
}

/// A run of jersey barriers: a wide foot, a sloped face and a narrow top,
/// cast in two-metre lengths.
fn barrier(m: &mut Mesh, b: &Block, ground: f32, col: Col) {
    let along_x = b.max.x - b.min.x > b.max.z - b.min.z;
    let (a0, a1) = if along_x { (b.min.x, b.max.x) } else { (b.min.z, b.max.z) };
    let (c0, c1) = if along_x { (b.min.z, b.max.z) } else { (b.min.x, b.max.x) };
    let (mid, t) = ((c0 + c1) * 0.5, (c1 - c0) * 0.5);
    let (knee, brk, top) = (ground + 0.075, ground + 0.27, b.max.y);
    let prof = [(-t, b.min.y), (t, b.min.y), (t, knee), (t * 0.8, brk), (t * 0.38, top), (-t * 0.38, top), (-t * 0.8, brk), (-t, knee)];
    let stops = [(ground - 0.3, 0.68), (ground + 0.05, 0.74), (ground + 0.5, 0.95), (top, 1.03)];
    let n = ((a1 - a0) / 2.0).round().max(1.0) as usize;
    let len = (a1 - a0) / n as f32;
    for i in 0..n {
        let (s, e) = (a0 + i as f32 * len + 0.02, a0 + (i + 1) as f32 * len - 0.02);
        sweep(m, along_x, s, e, mid, &prof, col, &stops);
    }
}

// ------------------------------------------------------------ containers

/// A shipping container: corner posts and castings, rails top and bottom,
/// corrugated walls, and doors with locking bars at one end.
pub fn container(m: &mut Mesh, b: &Block, j: f32, i: usize) {
    let c = CONTAINER[b.tint as usize % 4];
    let col = rgb(c[0] * j, c[1] * j, c[2] * j);
    let frame = shade(col, 0.8);
    let cast = shade(col, 0.55);
    let size = b.max - b.min;
    let long_x = size.x > size.z;
    // Local frame: x along the length, z across, y up, from the centre.
    let (hl, hw, hh) = if long_x { (size.x * 0.5, size.z * 0.5, size.y * 0.5) } else { (size.z * 0.5, size.x * 0.5, size.y * 0.5) };
    let flip = if hash01(i as u32 / 2, 61) < 0.5 { 1.0 } else { -1.0 };
    let turn = if long_x { 0.0 } else { FRAC_PI_2 };
    let g = Mat4::from_translation((b.min + b.max) * 0.5) * Mat4::from_rotation_y(turn) * Mat4::from_scale(vec3(flip, 1.0, 1.0));
    let (post, rail, e) = (0.09, 0.075, 0.07);
    // Walls and roof, set back behind the frame.
    m.obox(&g, vec3(0.0, -0.015, 0.0), vec3(hl - e, hh - 0.015, hw - e), shade(col, 1.04));
    for sx in [-1.0f32, 1.0] {
        for sz in [-1.0f32, 1.0] {
            m.obox(&g, vec3(sx * (hl - post), 0.0, sz * (hw - post)), vec3(post, hh - 0.15, post), frame);
            for sy in [-1.0f32, 1.0] {
                m.obox(&g, vec3(sx * (hl - 0.1), sy * (hh - 0.075), sz * (hw - 0.1)), vec3(0.1, 0.075, 0.1), cast);
            }
        }
    }
    for sy in [-1.0f32, 1.0] {
        let y = sy * (hh - rail);
        for sz in [-1.0f32, 1.0] {
            m.obox(&g, vec3(0.0, y, sz * (hw - 0.05)), vec3(hl - 0.2, rail, 0.05), frame);
        }
        for sx in [-1.0f32, 1.0] {
            m.obox(&g, vec3(sx * (hl - 0.05), y, 0.0), vec3(0.05, rail, hw - 0.2), frame);
        }
    }
    // Corrugation: trapezoid ribs between the rails, darker toward the foot.
    let (y0, y1) = (-hh + 2.0 * rail, hh - 2.0 * rail);
    let (lo, hi) = (shade(col, 0.8), shade(col, 1.0));
    let rib = |m: &mut Mesh, along: f32, side: Vec3, across: Vec3| {
        // `side` is the wall's outward normal, `across` runs along the wall.
        let dist = side.abs().dot(vec3(hl, 0.0, hw));
        let (back, front) = (side * (dist - e), side * (dist - 0.008));
        let p = |depth: Vec3, off: f32, y: f32| g.transform_point3(depth + across * (along + off) + vec3(0.0, y, 0.0));
        let slope = |s: f32| g.transform_vector3((side * 0.04 - across * s * (e - 0.008)).normalize()).normalize();
        let nf = g.transform_vector3(side).normalize();
        face(m, [p(back, -0.09, y0), p(front, -0.05, y0), p(front, -0.05, y1), p(back, -0.09, y1)], slope(1.0), [lo, lo, hi, hi]);
        face(
            m,
            [p(front, -0.05, y0), p(front, 0.05, y0), p(front, 0.05, y1), p(front, -0.05, y1)],
            nf,
            [shade(lo, 1.05), shade(lo, 1.05), shade(hi, 1.05), shade(hi, 1.05)],
        );
        face(m, [p(front, 0.05, y0), p(back, 0.09, y0), p(back, 0.09, y1), p(front, 0.05, y1)], slope(-1.0), [lo, lo, hi, hi]);
    };
    let pitch = 0.28;
    for sz in [-1.0f32, 1.0] {
        let n = ((hl - 0.22) * 2.0 / pitch).floor() as i32;
        for k in 0..n {
            let x = -(n - 1) as f32 * pitch * 0.5 + k as f32 * pitch;
            rib(m, x, vec3(0.0, 0.0, sz), Vec3::X);
        }
    }
    let n = ((hw - 0.22) * 2.0 / pitch).floor() as i32;
    for k in 0..n {
        let z = -(n - 1) as f32 * pitch * 0.5 + k as f32 * pitch;
        rib(m, z, -Vec3::X, Vec3::Z);
    }
    // Doors at +x: two leaves, four locking bars with handles.
    let leaf_x = hl - (e + 0.045) * 0.5;
    let half_door = (hw - 0.2) * 0.5 - 0.006;
    for sz in [-1.0f32, 1.0] {
        m.obox(&g, vec3(leaf_x, 0.0, sz * (half_door + 0.006)), vec3((e - 0.045) * 0.5, (y1 - y0) * 0.5, half_door), shade(col, 0.97));
    }
    let bar_col = shade(col, 0.6);
    for f in [-0.8f32, -0.3, 0.3, 0.8] {
        let z = f * (hw - 0.2);
        m.obox(&g, vec3(hl - 0.025, 0.0, z), vec3(0.018, (y1 - y0) * 0.5, 0.018), bar_col);
        m.obox(&g, vec3(hl - 0.022, -0.2, z + 0.07 * f.signum()), vec3(0.012, 0.016, 0.07), bar_col);
        for y in [y0 + 0.05, y1 - 0.05] {
            m.obox(&g, vec3(hl - 0.03, y, z), vec3(0.02, 0.03, 0.035), cast);
        }
    }
}

// -------------------------------------------------------------- sandbags

/// A sandbag wall, laid in staggered courses two bags thick, on a dark fill
/// so the gaps between bags do not show daylight.
pub fn sandbags(m: &mut Mesh, w: &World, b: &Block, j: f32, i: usize) {
    let along_x = b.max.x - b.min.x > b.max.z - b.min.z;
    let (a0, a1) = if along_x { (b.min.x, b.max.x) } else { (b.min.z, b.max.z) };
    let (c0, c1) = if along_x { (b.min.z, b.max.z) } else { (b.min.x, b.max.x) };
    let base = if b.tint == 1 { [0.47, 0.47, 0.31] } else { [0.62, 0.55, 0.39] };
    let fill = rgb(base[0] * 0.55, base[1] * 0.55, base[2] * 0.55);
    m.cuboid(b.min + vec3(0.05, 0.0, 0.05), b.max - vec3(0.05, 0.06, 0.05), fill);
    let n = ((a1 - a0) / 0.62).round().max(1.0) as usize;
    let bl = (a1 - a0) / n as f32;
    let across = if c1 - c0 > 0.6 { 2 } else { 1 };
    let bw = (c1 - c0) / across as f32;
    let bh = 0.2;
    let at = |a: f32, c: f32, y: f32| if along_x { vec3(a, y, c) } else { vec3(c, y, a) };
    let mut row = 0;
    loop {
        let top = b.max.y - row as f32 * bh;
        let y = top - bh * 0.5;
        if top < b.min.y + 0.05 {
            break;
        }
        // Each course: whole bags, or half bags at the ends to break the joints.
        let mut spans: Vec<(f32, f32)> = Vec::new();
        if row % 2 == 0 {
            spans.extend((0..n).map(|k| (a0 + k as f32 * bl, a0 + (k + 1) as f32 * bl)));
        } else {
            spans.push((a0, a0 + bl * 0.5));
            spans.extend((0..n - 1).map(|k| (a0 + bl * (0.5 + k as f32), a0 + bl * (1.5 + k as f32))));
            spans.push((a1 - bl * 0.5, a1));
        }
        let mut any = false;
        for (k, (s, e)) in spans.into_iter().enumerate() {
            for q in 0..across {
                let c = c0 + (q as f32 + 0.5) * bw;
                let mid = at((s + e) * 0.5, c, y);
                if top < w.height(mid.x, mid.z) - 0.02 {
                    continue; // buried
                }
                any = true;
                let h = hash01((i as u32 / 2) * 977 + row * 31 + k as u32 * 3 + q as u32, 47);
                let tone = j * (0.86 + 0.2 * h);
                let col = rgb(base[0] * tone, base[1] * tone, base[2] * tone);
                // Bags squash into their neighbours, each settled a little differently.
                let sag = (hash01(h.to_bits(), 5) - 0.5) * 0.02;
                bag(m, mid + vec3(0.0, sag, 0.0), along_x, (e - s) * 0.5 + 0.004, bw * 0.5 + 0.006, bh * 0.5 + 0.02, col);
            }
        }
        if !any && row > 0 {
            break;
        }
        row += 1;
    }
}

/// One filled bag: a soft pillow, its ends folded flat.
fn bag(m: &mut Mesh, c: Vec3, along_x: bool, hl: f32, hw: f32, hh: f32, col: Col) {
    // Twelve points round, a full oval, smooth-shaded.
    const RING: [(f32, f32); 12] = [
        (0.98, 0.3),
        (0.78, 0.74),
        (0.3, 0.98),
        (-0.3, 0.98),
        (-0.78, 0.74),
        (-0.98, 0.3),
        (-0.98, -0.3),
        (-0.78, -0.74),
        (-0.3, -0.98),
        (0.3, -0.98),
        (0.78, -0.74),
        (0.98, -0.3),
    ];
    // Stations along the bag: (position, width scale, height scale).
    const AT: [(f32, f32, f32); 4] = [(-1.0, 0.9, 0.62), (-0.8, 1.0, 1.0), (0.8, 1.0, 1.0), (1.0, 0.9, 0.62)];
    let world = |a: f32, u: f32, v: f32| if along_x { c + vec3(a, v, u) } else { c + vec3(u, v, a) };
    let normal = |u: f32, v: f32, a: f32| {
        let n = vec3(u / hw, v / hh, 0.0).normalize_or_zero();
        if along_x {
            vec3(a, n.y, n.x).normalize_or_zero()
        } else {
            vec3(n.x, n.y, a).normalize_or_zero()
        }
    };
    let ring = |s: (f32, f32, f32)| RING.map(|(u, v)| (s.0 * hl, u * hw * s.1, v * hh * s.2));
    let dark = shade(col, 0.82);
    for st in AT.windows(2) {
        let (r0, r1) = (ring(st[0]), ring(st[1]));
        let (lean0, lean1) = (if st[0].1 < 1.0 { st[0].0 * 0.8 } else { 0.0 }, if st[1].1 < 1.0 { st[1].0 * 0.8 } else { 0.0 });
        for k in 0..RING.len() {
            let l = (k + 1) % RING.len();
            let p = [
                world(r0[k].0, r0[k].1, r0[k].2),
                world(r0[l].0, r0[l].1, r0[l].2),
                world(r1[l].0, r1[l].1, r1[l].2),
                world(r1[k].0, r1[k].1, r1[k].2),
            ];
            let n = [
                normal(r0[k].1, r0[k].2, lean0),
                normal(r0[l].1, r0[l].2, lean0),
                normal(r1[l].1, r1[l].2, lean1),
                normal(r1[k].1, r1[k].2, lean1),
            ];
            // The underside sits in shadow against the course below.
            let cs = |v: f32| if v < -0.01 { dark } else { col };
            m.quad_nc(p, n, [cs(r0[k].2), cs(r0[l].2), cs(r1[l].2), cs(r1[k].2)]);
        }
    }
    for st in [AT[0], AT[3]] {
        let r = ring(st);
        let mid = world(st.0 * hl * 1.02, 0.0, 0.0);
        let n = if along_x { vec3(st.0, 0.0, 0.0) } else { vec3(0.0, 0.0, st.0) };
        for k in 0..RING.len() {
            let l = (k + 1) % RING.len();
            m.tri_n(mid, world(r[k].0, r[k].1, r[k].2), world(r[l].0, r[l].1, r[l].2), n, shade(col, 0.92));
        }
    }
}

// ----------------------------------------------------------------- crates

/// A wooden crate: planks behind a nailed frame, a diagonal brace on every
/// side and a planked lid.
pub fn crate_box(m: &mut Mesh, b: &Block) {
    let k = 0.9 + 0.07 * b.tint as f32;
    let wood = rgb(0.70 * k, 0.50 * k, 0.26 * k);
    let frame = rgb(0.56 * k, 0.39 * k, 0.2 * k);
    let gap = rgb(0.22 * k, 0.15 * k, 0.08 * k);
    let (f, t) = (0.03, 0.085);
    let foot = b.min.y + 0.6;
    let (lo, hi) = (b.min + vec3(f, 0.0, f), b.max - vec3(f, 0.0, f));
    let lid = b.max.y - 0.018;
    // The dark inside that the plank joints show.
    m.cuboid(lo + vec3(0.012, 0.0, 0.012), vec3(hi.x - 0.012, lid - 0.01, hi.z - 0.012), gap);
    let seed = ((b.min.x * 13.0) as i32 ^ (b.min.z * 7.0) as i32) as u32;
    let jit = |n: u32| shade(wood, 0.9 + 0.18 * hash01(seed, n));
    // Side planks, laid horizontally with thin joints between.
    let rows = (((lid - foot) / 0.17).round() as usize).max(3);
    let ph = (lid - foot) / rows as f32;
    let sides: [(Vec3, f32); 4] = [(Vec3::X, hi.x), (-Vec3::X, lo.x), (Vec3::Z, hi.z), (-Vec3::Z, lo.z)];
    for (si, (n, at)) in sides.into_iter().enumerate() {
        let quad = |m: &mut Mesh, y0: f32, y1: f32, col: Col| {
            let p = if n.x != 0.0 {
                [vec3(at, y0, lo.z), vec3(at, y0, hi.z), vec3(at, y1, hi.z), vec3(at, y1, lo.z)]
            } else {
                [vec3(lo.x, y0, at), vec3(hi.x, y0, at), vec3(hi.x, y1, at), vec3(lo.x, y1, at)]
            };
            face(m, p, n, [col; 4]);
        };
        quad(m, b.min.y, foot + 0.006, gap);
        for r in 0..rows {
            let (y0, y1) = (foot + r as f32 * ph + 0.006, foot + (r + 1) as f32 * ph - 0.006);
            quad(m, y0, y1, jit(si as u32 * 16 + r as u32));
        }
        // Frame: top and bottom rails between the corner boards.
        let (w0, w1) = if n.x != 0.0 { (b.min.z + t + f, b.max.z - t - f) } else { (b.min.x + t + f, b.max.x - t - f) };
        for (y0, y1) in [(foot, foot + t), (b.max.y - t, b.max.y)] {
            let (mn, mx) = if n.x != 0.0 {
                (vec3(at.min(at + n.x * f), y0, w0), vec3(at.max(at + n.x * f), y1, w1))
            } else {
                (vec3(w0, y0, at.min(at + n.z * f)), vec3(w1, y1, at.max(at + n.z * f)))
            };
            m.cuboid(mn, mx, frame);
        }
        // The brace, corner to corner across the planks, behind the rails.
        let (span, rise) = (w1 - w0, b.max.y - t - (foot + t));
        let ang = rise.atan2(span) * if si % 2 == 0 { 1.0 } else { -1.0 };
        let centre = if n.x != 0.0 {
            vec3(at + n.x * f * 0.4, foot + t + rise * 0.5, (w0 + w1) * 0.5)
        } else {
            vec3((w0 + w1) * 0.5, foot + t + rise * 0.5, at + n.z * f * 0.4)
        };
        let turn = if n.x != 0.0 { FRAC_PI_2 } else { 0.0 };
        let g = Mat4::from_translation(centre) * Mat4::from_rotation_y(turn) * Mat4::from_rotation_z(ang);
        m.obox(&g, Vec3::ZERO, vec3((span * span + rise * rise).sqrt() * 0.5, t * 0.45, f * 0.4), frame);
    }
    // Corner boards, full height.
    for (cx, cz) in [(b.min.x, b.min.z), (b.max.x, b.min.z), (b.min.x, b.max.z), (b.max.x, b.max.z)] {
        let (sx, sz) = (if cx > lo.x { -1.0 } else { 1.0 }, if cz > lo.z { -1.0 } else { 1.0 });
        let other = vec3(cx + sx * (t + f), b.max.y, cz + sz * (t + f));
        m.cuboid(vec3(cx.min(other.x), foot, cz.min(other.z)), vec3(cx.max(other.x), b.max.y, cz.max(other.z)), frame);
    }
    // Lid planks.
    let along_x = hi.x - lo.x >= hi.z - lo.z;
    let (s0, s1) = if along_x { (lo.z, hi.z) } else { (lo.x, hi.x) };
    let np = 5;
    let pw = (s1 - s0) / np as f32;
    for p in 0..np {
        let (q0, q1) = (s0 + p as f32 * pw + 0.006, s0 + (p + 1) as f32 * pw - 0.006);
        let pts = if along_x {
            [vec3(lo.x, lid, q0), vec3(lo.x, lid, q1), vec3(hi.x, lid, q1), vec3(hi.x, lid, q0)]
        } else {
            [vec3(q0, lid, lo.z), vec3(q1, lid, lo.z), vec3(q1, lid, hi.z), vec3(q0, lid, hi.z)]
        };
        face(m, pts, Vec3::Y, [shade(jit(90 + p as u32), 1.06); 4]);
    }
}

// ------------------------------------------------------------------ steel

/// What a steel block is part of, worked out from how blocks sit together.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Steel {
    Plain,
    /// The shell of a round tank of this radius (the tank is several
    /// crossed boxes; the others draw nothing).
    Tank(f32),
    Hidden,
    Chimney,
    /// The collar round a chimney's top.
    Collar,
    /// A light mast, and the lamp head on it.
    Pole,
    Lamp,
    /// A steel column: drawn as an H-beam.
    Beam,
    /// A flat roof of corrugated sheet.
    Roof,
    /// A breastwork plate with stiffeners.
    Plate,
}

/// The part each block plays; `Plain` for everything not steel.
pub fn steel_roles(w: &World) -> Vec<Steel> {
    let bl = &w.blocks;
    let metal = |b: &Block| b.kind == BlockKind::Metal && b.dest.is_none();
    let half = |b: &Block| (b.max - b.min) * 0.5;
    let centre = |b: &Block| (b.min + b.max) * 0.5;
    let same_axis = |p: &Block, q: &Block| (centre(p).x - centre(q).x).abs() < 0.02 && (centre(p).z - centre(q).z).abs() < 0.02;
    let near = |a: f32, b: f32| (a - b).abs() < 0.02;
    let mut role = vec![Steel::Plain; bl.len()];
    for (i, b) in bl.iter().enumerate() {
        if !metal(b) || role[i] != Steel::Plain {
            continue;
        }
        let h = half(b);
        // Tanks: a slab r by 0.42r, the same turned, a 0.72r core and a
        // 0.3r hatch on top.
        if h.x > 1.5 && near(h.z, h.x * 0.42) {
            let r = h.x;
            let find = |hx: f32, hz: f32, top: bool| {
                bl.iter().position(|q| {
                    metal(q) && same_axis(b, q) && near(half(q).x, hx) && near(half(q).z, hz) && (!top || near(q.min.y, b.max.y))
                })
            };
            if let (Some(p), Some(q), Some(s)) = (find(r * 0.42, r, false), find(r * 0.72, r * 0.72, false), find(r * 0.3, r * 0.3, true)) {
                role[i] = Steel::Tank(r);
                for k in [p, q, s] {
                    role[k] = Steel::Hidden;
                }
                continue;
            }
        }
        let tall = b.max.y - b.min.y;
        if near(h.x, h.z) && h.x > 0.4 && h.x < 1.3 && tall > 8.0 {
            role[i] = Steel::Chimney;
            if let Some(k) = bl.iter().position(|q| metal(q) && same_axis(b, q) && near(q.min.y, b.max.y)) {
                role[k] = Steel::Collar;
            }
            continue;
        }
        if h.x <= 0.21 && h.z <= 0.21 && tall > 5.0 && b.tint == 1 {
            role[i] = Steel::Pole;
            if let Some(k) = bl.iter().position(|q| metal(q) && same_axis(b, q) && near(q.min.y, b.max.y)) {
                role[k] = Steel::Lamp;
            }
            continue;
        }
        if role[i] != Steel::Plain {
            continue;
        }
        role[i] = if h.x <= 0.26 && h.z <= 0.26 && tall > 1.5 {
            Steel::Beam
        } else if tall <= 0.8 && h.x >= 1.0 && h.z >= 1.0 {
            Steel::Roof
        } else if h.x.min(h.z) <= 0.18 && tall <= 1.6 && h.x.max(h.z) >= 0.5 {
            Steel::Plate
        } else {
            Steel::Plain
        };
    }
    role
}

pub fn steel(m: &mut Mesh, w: &World, b: &Block, j: f32, role: Steel) {
    let k = j * (0.9 + 0.2 * b.tint as f32);
    let col = rgb(0.33 * k, 0.31 * k, 0.32 * k);
    let c = (b.min + b.max) * 0.5;
    let h = (b.max - b.min) * 0.5;
    let ground = w.height(c.x, c.z).max(b.min.y);
    match role {
        Steel::Hidden => {}
        Steel::Tank(r) => tank(m, w, b, r, j),
        Steel::Chimney => {
            let r = h.x * 0.97;
            column(m, c.x, c.z, b.min.y, b.max.y, r, 10, col);
            let band = shade(col, 0.75);
            let mut y = ground + 2.5;
            while y < b.max.y - 2.0 {
                column(m, c.x, c.z, y - 0.06, y + 0.06, r + 0.035, 10, band);
                y += 2.5;
            }
            column(m, c.x, c.z, b.max.y - 1.6, b.max.y - 0.8, r + 0.03, 10, HAZARD);
        }
        Steel::Collar => {
            column(m, c.x, c.z, b.min.y, b.max.y, h.x * 0.97, 10, shade(col, 0.8));
            column(m, c.x, c.z, b.max.y - 0.02, b.max.y + 0.01, h.x * 0.6, 10, rgb(0.08, 0.08, 0.08));
        }
        Steel::Pole => {
            column(m, c.x, c.z, b.min.y, b.max.y, h.x * 0.65, 6, col);
            column(m, c.x, c.z, ground - 0.05, ground + 0.06, h.x, 6, shade(col, 0.8));
            column(m, c.x, c.z, ground + 0.06, ground + 0.5, h.x * 0.8, 6, shade(col, 0.9));
        }
        Steel::Lamp => {
            let stops = [(b.min.y, 0.8), (b.max.y, 1.05)];
            prism(m, &octagon(b.min, b.max, h.x.min(h.z) * 0.5), b.min.y + 0.03, b.max.y, col, &stops);
            let (lo, hi) = (b.min + vec3(0.06, 0.0, 0.06), b.max - vec3(0.06, 0.0, 0.06));
            let y = b.min.y + 0.03;
            face(
                m,
                [vec3(lo.x, y - 0.005, lo.z), vec3(hi.x, y - 0.005, lo.z), vec3(hi.x, y - 0.005, hi.z), vec3(lo.x, y - 0.005, hi.z)],
                -Vec3::Y,
                [LENS; 4],
            );
        }
        Steel::Beam => {
            // Flanges across the longer way, a web between them.
            let wide_x = h.x >= h.z;
            let (fl, web) = (0.2 * h.x.min(h.z), 0.12 * h.x.min(h.z));
            let (y0, y1) = (b.min.y, b.max.y - 0.03);
            for s in [-1.0f32, 1.0] {
                let (mn, mx) = if wide_x {
                    (vec3(b.min.x, y0, c.z + s * h.z - fl), vec3(b.max.x, y1, c.z + s * h.z))
                } else {
                    (vec3(c.x + s * h.x - fl, y0, b.min.z), vec3(c.x + s * h.x, y1, b.max.z))
                };
                m.cuboid(mn.min(mx), mn.max(mx), col);
            }
            let (mn, mx) = if wide_x {
                (vec3(c.x - web, y0, b.min.z + fl), vec3(c.x + web, y1, b.max.z - fl))
            } else {
                (vec3(b.min.x + fl, y0, c.z - web), vec3(b.max.x - fl, y1, c.z + web))
            };
            m.cuboid(mn, mx, shade(col, 0.85));
            m.cuboid(vec3(b.min.x, b.max.y - 0.03, b.min.z), b.max, shade(col, 1.1));
            m.cuboid(
                vec3(b.min.x - 0.05, ground - 0.1, b.min.z - 0.05),
                vec3(b.max.x + 0.05, ground + 0.03, b.max.z + 0.05),
                shade(col, 0.75),
            );
        }
        Steel::Roof => roof(m, b, col),
        Steel::Plate => plate(m, w, b, col),
        Steel::Plain => {
            let stops = weather(ground, b.max.y);
            let size = b.max - b.min;
            prism(m, &octagon(b.min, b.max, (size.x.min(size.z) * 0.15).min(0.04)), b.min.y, b.max.y, col, &stops);
            if size.y > 8.0 {
                // Hazard band on a tall stack.
                m.cuboid(vec3(b.min.x - 0.05, b.max.y - 1.6, b.min.z - 0.05), vec3(b.max.x + 0.05, b.max.y - 0.8, b.max.z + 0.05), HAZARD);
            } else if size.y > 1.5 {
                // Standing seams down the cladding.
                for (n, len) in [(Vec3::X, size.z), (Vec3::Z, size.x)] {
                    if len < 1.6 {
                        continue;
                    }
                    let seams = (len / 0.9).floor() as i32;
                    for s in [-1.0f32, 1.0] {
                        for q in 1..seams {
                            let a = -len * 0.5 + q as f32 * len / seams as f32;
                            let at = c + n * s * (if n.x != 0.0 { h.x } else { h.z });
                            let off = if n.x != 0.0 { vec3(0.0, 0.0, a) } else { vec3(a, 0.0, 0.0) };
                            let p = at + off;
                            let e = vec3(0.02, 0.0, 0.02) + n.abs() * 0.005;
                            m.cuboid(vec3(p.x - e.x, ground, p.z - e.z), vec3(p.x + e.x, b.max.y - 0.05, p.z + e.z), shade(col, 0.88));
                        }
                    }
                }
            }
        }
    }
}

/// A flat roof of corrugated sheet on a deep fascia.
fn roof(m: &mut Mesh, b: &Block, col: Col) {
    let sheet = shade(col, 1.12);
    let top = b.max.y;
    let base = top - 0.05;
    let stops = [(b.min.y, 0.62), (base - 0.01, 0.78), (base, 0.92)];
    prism(m, &octagon(b.min, vec3(b.max.x, base, b.max.z), 0.0), b.min.y, base, sheet, &stops);
    // Ribs run the short way, so they show along the long eaves.
    let long_x = b.max.x - b.min.x >= b.max.z - b.min.z;
    let (a0, a1) = if long_x { (b.min.x, b.max.x) } else { (b.min.z, b.max.z) };
    let (s0, s1) = if long_x { (b.min.z, b.max.z) } else { (b.min.x, b.max.x) };
    let pitch = 0.3;
    let n = ((a1 - a0) / pitch).floor() as i32;
    let start = (a0 + a1) * 0.5 - (n - 1) as f32 * pitch * 0.5;
    // Rib profile (along, up): a trapezoid standing on the sheet.
    let prof = [(-0.07, base), (0.07, base), (0.035, top), (-0.035, top)];
    for q in 0..n {
        let a = start + q as f32 * pitch;
        sweep(m, !long_x, s0, s1, a, &prof, sheet, &[(base, 0.92), (top, 1.04)]);
    }
}

/// A steel breastwork plate: stiffeners on both faces and a lip along the top.
fn plate(m: &mut Mesh, w: &World, b: &Block, col: Col) {
    let along_x = b.max.x - b.min.x > b.max.z - b.min.z;
    let (a0, a1) = if along_x { (b.min.x, b.max.x) } else { (b.min.z, b.max.z) };
    let (c0, c1) = if along_x { (b.min.z, b.max.z) } else { (b.min.x, b.max.x) };
    let mid = (c0 + c1) * 0.5;
    let t = (c1 - c0) * 0.5;
    let at = |a: f32, c: f32, y: f32| if along_x { vec3(a, y, c) } else { vec3(c, y, a) };
    let ground = w.height((b.min.x + b.max.x) * 0.5, (b.min.z + b.max.z) * 0.5).max(b.min.y);
    let stops = weather(ground, b.max.y);
    let plate = [(-t * 0.45, b.min.y), (t * 0.45, b.min.y), (t * 0.45, b.max.y - 0.06), (-t * 0.45, b.max.y - 0.06)];
    sweep(m, along_x, a0, a1, mid, &plate, col, &stops);
    let lip = [(-t, b.max.y - 0.06), (t, b.max.y - 0.06), (t, b.max.y), (-t, b.max.y)];
    sweep(m, along_x, a0, a1, mid, &lip, shade(col, 1.1), &[(b.min.y, 1.0)]);
    let n = ((a1 - a0) / 0.7).floor().max(1.0) as i32;
    for q in 0..=n {
        let a = a0 + 0.04 + (a1 - a0 - 0.08) * q as f32 / n as f32;
        let (p0, p1) = (at(a - 0.03, c0, ground - 0.1), at(a + 0.03, c1, b.max.y - 0.06));
        m.cuboid(p0.min(p1), p0.max(p1), shade(col, 0.85));
    }
}

/// A round storage tank on a concrete footing: welded courses, a shallow
/// cone roof with a hatch, and a caged ladder up one side.
fn tank(m: &mut Mesh, w: &World, b: &Block, r: f32, j: f32) {
    let tint = TANK[b.tint as usize % TANK.len()];
    let col = rgb(tint[0] * j, tint[1] * j, tint[2] * j);
    let c = (b.min + b.max) * 0.5;
    let ground = w.height(c.x, c.z);
    let top = b.max.y;
    let rr = r * 0.93;
    let segs = 16;
    column(m, c.x, c.z, b.min.y, top, rr, segs, col);
    column(m, c.x, c.z, ground - 0.3, ground + 0.4, rr + 0.012, segs, shade(col, 0.62));
    column(m, c.x, c.z, ground - 0.4, ground + 0.12, rr + 0.14, segs, rgb(0.55 * j, 0.53 * j, 0.5 * j));
    let seam = shade(col, 0.82);
    let mut y = ground + 1.4;
    while y < top - 0.4 {
        column(m, c.x, c.z, y - 0.03, y + 0.03, rr + 0.018, segs, seam);
        y += 1.4;
    }
    // Cone roof up to the hatch.
    let (rise, inner) = (0.32, r * 0.3);
    let n = segs * 2;
    let roof = shade(col, 1.08);
    for k in 0..n {
        let (a0, a1) = (k as f32 / n as f32 * TAU, (k + 1) as f32 / n as f32 * TAU);
        let rim = |a: f32, rad: f32, y: f32| vec3(c.x + a.cos() * rad, y, c.z + a.sin() * rad);
        let nrm = |a: f32| vec3(a.cos() * rise, rr - inner, a.sin() * rise).normalize();
        m.quad_n(
            [rim(a0, rr, top), rim(a1, rr, top), rim(a1, inner, top + rise), rim(a0, inner, top + rise)],
            [nrm(a0), nrm(a1), nrm(a1), nrm(a0)],
            roof,
        );
    }
    column(m, c.x, c.z, top + rise - 0.05, top + 0.6, r * 0.24, 8, shade(col, 0.9));
    column(m, c.x, c.z, top + 0.55, top + 0.62, r * 0.27, 8, shade(col, 0.7));
    // Ladder on the side the slab reaches out furthest (+x or -x).
    let side = if hash01((c.x.abs() * 10.0) as u32, (c.z.abs() * 10.0) as u32) < 0.5 { 1.0 } else { -1.0 };
    let lx = c.x + side * (rr + 0.12);
    let steel = rgb(0.3 * j, 0.3 * j, 0.31 * j);
    for dz in [-0.22f32, 0.22] {
        m.cuboid(vec3(lx - 0.025, ground, c.z + dz - 0.025), vec3(lx + 0.025, top + 0.3, c.z + dz + 0.025), steel);
    }
    let mut y = ground + 0.3;
    while y < top + 0.2 {
        m.cuboid(vec3(lx - 0.018, y - 0.018, c.z - 0.22), vec3(lx + 0.018, y + 0.018, c.z + 0.22), steel);
        y += 0.3;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_shared::map::MAPS;

    /// Every tank found is whole: one shell and its three hidden parts.
    #[test]
    fn tanks_are_found_whole() {
        let mut tanks = 0;
        for map in 0..MAPS.len() as u8 {
            let w = World::with_map(0xFE221, map);
            let roles = steel_roles(&w);
            let shells = roles.iter().filter(|r| matches!(r, Steel::Tank(_))).count();
            let hidden = roles.iter().filter(|r| **r == Steel::Hidden).count();
            assert_eq!(hidden, shells * 3, "map {map}");
            tanks += shells;
        }
        assert!(tanks > 0, "no tanks found on any map");
    }

    /// The props stay inside their blocks, give or take a few centimetres.
    #[test]
    fn props_stay_inside_their_blocks() {
        for map in 0..MAPS.len() as u8 {
            let w = World::with_map(0xFE221, map);
            for (i, b) in w.blocks.iter().enumerate() {
                let mut m = Mesh::default();
                match b.kind {
                    BlockKind::Container => container(&mut m, b, 1.0, i),
                    BlockKind::Sandbag => sandbags(&mut m, &w, b, 1.0, i),
                    BlockKind::Crate => crate_box(&mut m, b),
                    _ => continue,
                }
                let slack = 0.03;
                for v in m.finish().into_iter().flat_map(|(v, _)| v) {
                    let p = Vec3::from(v.pos);
                    assert!(
                        p.cmpge(b.min - Vec3::splat(slack)).all() && p.cmple(b.max + Vec3::splat(slack)).all(),
                        "{:?} {p} outside {:?}..{:?}",
                        b.kind,
                        b.min,
                        b.max
                    );
                }
            }
        }
    }
}
