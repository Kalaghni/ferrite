//! Everything that turns game state into low-poly geometry.

use crate::gfx::{rgb, shade, Col, Mesh};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::*;
use ferrite_shared::{NEUTRAL, POINT_RADIUS};

pub const TEAM_COL: [[f32; 3]; 2] = [[0.96, 0.43, 0.12], [0.15, 0.46, 0.96]];
pub const NEUTRAL_COL: [f32; 3] = [0.82, 0.82, 0.78];
pub const FOG: [f32; 4] = [0.85, 0.78, 0.67, 0.0046];
pub const SUN: Vec3 = Vec3::new(0.52, 0.58, 0.40);

pub fn team_col(t: u8) -> [f32; 3] {
    if t < 2 {
        TEAM_COL[t as usize]
    } else {
        NEUTRAL_COL
    }
}

fn hash01(a: u32, b: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 13;
    (h & 0xFFFF) as f32 / 65535.0
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Ground colour at `c`: the map's palette, worn to dirt around its
/// objectives and to rock on slopes.
fn ground_col(w: &World, c: Vec3, n: Vec3, jitter: f32) -> Col {
    if w.map == slag_cathedral::ID {
        return slag_cathedral::ground(w, c, n, jitter);
    }
    let [grass, dry, dirt] = map_def(w.map).ground;
    let patch = vnoise(c.x / 24.0, c.z / 24.0, w.seed as u32 ^ 7);
    let mut col = mix3(grass, dry, smooth(0.3, 0.75, patch));
    let mut near = f32::MAX;
    for p in w.points.iter().chain(w.bases.iter()) {
        near = near.min(((c.x - p.x).powi(2) + (c.z - p.z).powi(2)).sqrt());
    }
    col = mix3(col, dirt, 1.0 - smooth(7.0, 16.0, near));
    col = mix3(col, [0.47, 0.40, 0.35], smooth(0.06, 0.22, 1.0 - n.y));
    if w.map == relay::ID {
        col = relay::ground_tint(w, col, c, n);
    }
    if w.map == fallen_giant::ID {
        col = fallen_giant::ground_tint(col, c);
    }
    let k = 0.9 + 0.2 * jitter;
    rgb(col[0] * k, col[1] * k, col[2] * k)
}

#[allow(clippy::too_many_arguments)]
fn taper(m: &mut Mesh, min: Vec3, max: Vec3, bot: f32, top: f32, skew_bot: Vec3, skew: Vec3, col: Col) {
    let c = (min + max) * 0.5;
    let h = (max - min) * 0.5;
    let mut p = [Vec3::ZERO; 8];
    for (k, q) in p.iter_mut().enumerate() {
        let sx = if k & 1 != 0 { 1.0 } else { -1.0 };
        let sz = if k & 4 != 0 { 1.0 } else { -1.0 };
        *q = if k & 2 != 0 {
            vec3(c.x + sx * h.x * top + skew.x, max.y, c.z + sz * h.z * top + skew.z)
        } else {
            vec3(c.x + sx * h.x * bot + skew_bot.x, min.y, c.z + sz * h.z * bot + skew_bot.z)
        };
    }
    m.corners(p, col);
}

/// A length of trunk: eight-sided, open at both ends (the ground and the
/// crown hide them).
#[allow(clippy::too_many_arguments)]
fn bole(m: &mut Mesh, a: Vec3, ra: f32, b: Vec3, rb: f32, rot: f32, col: Col, seed: u32) {
    const N: usize = 8;
    let at = |k: usize, c: Vec3, r: f32| {
        let ang = rot + (k % N) as f32 / N as f32 * std::f32::consts::TAU;
        c + vec3(ang.cos() * r, 0.0, ang.sin() * r)
    };
    for k in 0..N {
        let j = 0.92 + 0.16 * hash01(k as u32, seed);
        m.quad(at(k, a, ra), at(k, b, rb), at(k + 1, b, rb), at(k + 1, a, ra), shade(col, j));
    }
}

fn octa(m: &mut Mesh, c: Vec3, r: Vec3, rot: f32, col: Col) {
    let (s, co) = rot.sin_cos();
    let h = |a: f32, b: f32| c + vec3((a * co - b * s) * r.x, 0.0, (a * s + b * co) * r.z);
    const N: usize = 10;
    let top = c + vec3(0.0, r.y, 0.0);
    let bot = c - vec3(0.0, r.y * 0.7, 0.0);
    let ring = |k: usize, rad: f32, y: f32, tw: f32| {
        let a = (k % N) as f32 / N as f32 * std::f32::consts::TAU + tw;
        h(a.cos() * rad, a.sin() * rad) + vec3(0.0, y, 0.0)
    };
    for k in 0..N {
        let j = 0.9 + 0.2 * hash01(k as u32, (r.x * 97.0) as u32);
        let (u0, u1) = (ring(k, 0.62, r.y * 0.62, 0.2), ring(k + 1, 0.62, r.y * 0.62, 0.2));
        let (s0, s1) = (ring(k, 0.93, r.y * 0.28, 0.1), ring(k + 1, 0.93, r.y * 0.28, 0.1));
        let (m0, m1) = (ring(k, 1.0, -r.y * 0.08, 0.0), ring(k + 1, 1.0, -r.y * 0.08, 0.0));
        let (l0, l1) = (ring(k, 0.6, -r.y * 0.45, -0.2), ring(k + 1, 0.6, -r.y * 0.45, -0.2));
        m.tri(u0, top, u1, shade(col, 1.08 * j));
        m.quad(s0, u0, u1, s1, shade(col, 1.0 * j));
        m.quad(m0, s0, s1, m1, shade(col, 0.93 * j));
        m.quad(l0, m0, m1, l1, shade(col, 0.84 * j));
        m.tri(l1, bot, l0, shade(col, 0.72));
    }
}

fn pyramid(m: &mut Mesh, base: Vec3, r: f32, h: f32, rot: f32, col: Col) {
    let n = 8;
    let top = base + vec3(0.0, h, 0.0);
    for k in 0..n {
        let a0 = rot + k as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = rot + (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
        let p0 = base + vec3(a0.cos() * r, 0.0, a0.sin() * r);
        let p1 = base + vec3(a1.cos() * r, 0.0, a1.sin() * r);
        // A shoulder part-way up: the boughs droop out from the stem
        // instead of running straight to the tip.
        let q0 = base + vec3(a0.cos() * r * 0.46, h * 0.42, a0.sin() * r * 0.46);
        let q1 = base + vec3(a1.cos() * r * 0.46, h * 0.42, a1.sin() * r * 0.46);
        let j = 0.9 + 0.2 * hash01(k as u32, (r * 100.0) as u32);
        m.tri(q0, top, q1, shade(col, 1.04 * j));
        m.quad(p0, q0, q1, p1, shade(col, 0.96 * j));
        m.tri(p1, base + vec3(0.0, h * 0.12, 0.0), p0, shade(col, 0.6));
    }
}

/// Blocks whose visible faces lie in one plane and overlap z-fight: which
/// face wins flips as the view moves. Each block is shaded with a per-block
/// jitter, so the fight shows as patches jumping between two shades. Group
/// such blocks and return, per block, the index whose shade it takes (the
/// lowest in its group), so the faces that fight look the same.
pub fn shade_groups(w: &World) -> Vec<usize> {
    let b = &w.blocks;
    let mut root: Vec<usize> = (0..b.len()).collect();
    fn find(r: &mut [usize], mut i: usize) -> usize {
        while r[i] != i {
            r[i] = r[r[i]];
            i = r[i];
        }
        i
    }
    for i in 0..b.len() {
        for j in i + 1..b.len() {
            let (p, q) = (&b[i], &b[j]);
            if p.min.cmpgt(q.max).any() || q.min.cmpgt(p.max).any() {
                continue; // apart: no shared face
            }
            let shares = (0..3).any(|ax| {
                let (o1, o2) = ([1, 2, 0][ax], [2, 0, 1][ax]);
                let lo1 = p.min[o1].max(q.min[o1]);
                let lo2 = p.min[o2].max(q.min[o2]);
                if p.max[o1].min(q.max[o1]) - lo1 <= 0.01 || p.max[o2].min(q.max[o2]) - lo2 <= 0.01 {
                    return false;
                }
                let low = (p.min[ax] - q.min[ax]).abs() < 0.002;
                let high = (p.max[ax] - q.max[ax]).abs() < 0.002;
                // Bottoms sunk into the ground are never seen; counting them
                // would put every block standing on flat ground in one group.
                let buried = ax == 1 && {
                    let c = (p.min.max(q.min) + p.max.min(q.max)) * 0.5;
                    p.min.y <= w.height(c.x, c.z) + 0.05
                };
                high || (low && !buried)
            });
            if shares {
                let (a, c) = (find(&mut root, i), find(&mut root, j));
                root[a.max(c)] = a.min(c);
            }
        }
    }
    (0..b.len()).map(|i| find(&mut root, i)).collect()
}

/// Triangles each part of a map adds, in drawing order: one row per scene
/// module. The triangle_profile example prints these for every map.
pub type Profile = Vec<(&'static str, usize)>;

/// Add `n` triangles to `name`'s row (rows keep the order first seen).
pub fn tally(p: &mut Profile, name: &'static str, n: usize) {
    match p.iter_mut().find(|row| row.0 == name) {
        Some(row) => row.1 += n,
        None => p.push((name, n)),
    }
}

/// Counts what a section added to the mesh since the last mark.
struct Meter(usize);
impl Meter {
    fn mark(&mut self, p: &mut Profile, m: &Mesh, name: &'static str) {
        let now = m.triangles();
        tally(p, name, now - self.0);
        self.0 = now;
    }
}

pub fn build_world(w: &World) -> Mesh {
    build_world_profiled(w).0
}

/// The static world mesh, and how many of its triangles each module drew.
pub fn build_world_profiled(w: &World) -> (Mesh, Profile) {
    let mut m = Mesh::default();
    let mut prof = Profile::new();
    let mut meter = Meter(0);
    let seed = w.seed as u32;
    // The ground is drawn at twice the resolution the sim walks on. The new
    // vertices follow a curve through the old ones, so slopes round off, but
    // never leave the walked surface by more than `SMOOTH`: feet, grass and
    // walls stay where the sim put them.
    const SMOOTH: f32 = 0.05;
    let smooth = if w.map == slag_cathedral::ID { 0.0 } else { SMOOTH }; // a cast floor stays flat
    let hg = |ix: isize, iz: isize| w.heights[iz.clamp(0, TNZ as isize) as usize * (TNX + 1) + ix.clamp(0, TNX as isize) as usize];
    let cr = |a: f32, b: f32, c: f32, d: f32| (-a + 9.0 * b + 9.0 * c - d) / 16.0;
    let (fnx, fnz) = (TNX * 2, TNZ * 2);
    let mut fine = Vec::with_capacity((fnx + 1) * (fnz + 1));
    for fz in 0..=fnz {
        for fx in 0..=fnx {
            let (ix, iz) = ((fx / 2) as isize, (fz / 2) as isize);
            let along_x = |z: isize| cr(hg(ix - 1, z), hg(ix, z), hg(ix + 1, z), hg(ix + 2, z));
            let along_z = |x: isize| cr(hg(x, iz - 1), hg(x, iz), hg(x, iz + 1), hg(x, iz + 2));
            // (curved, walked): the walked surface splits each cell along
            // its (x+1, z)-(x, z+1) diagonal.
            let (curved, walked) = match (fx % 2, fz % 2) {
                (0, 0) => (hg(ix, iz), hg(ix, iz)),
                (1, 0) => (along_x(iz), (hg(ix, iz) + hg(ix + 1, iz)) * 0.5),
                (0, 1) => (along_z(ix), (hg(ix, iz) + hg(ix, iz + 1)) * 0.5),
                _ => (cr(along_x(iz - 1), along_x(iz), along_x(iz + 1), along_x(iz + 2)), (hg(ix + 1, iz) + hg(ix, iz + 1)) * 0.5),
            };
            fine.push(walked + (curved - walked).clamp(-smooth, smooth));
        }
    }
    let hp = |fx: usize, fz: usize| vec3(TER_X0 + fx as f32 * CELL * 0.5, fine[fz * (fnx + 1) + fx], TER_Z0 + fz as f32 * CELL * 0.5);
    for iz in 0..fnz {
        for ix in 0..fnx {
            let (p00, p10, p01, p11) = (hp(ix, iz), hp(ix + 1, iz), hp(ix, iz + 1), hp(ix + 1, iz + 1));
            for (k, (a, b, c)) in [(p00, p01, p10), (p11, p10, p01)].into_iter().enumerate() {
                let n = (b - a).cross(c - a).normalize();
                let cen = (a + b + c) / 3.0;
                // Cells spanning a tunnel portal give way to its rock collar.
                if w.tunnel_carves(cen, a.y.min(b.y).min(c.y), a.y.max(b.y).max(c.y)) {
                    continue;
                }
                let j = hash01((ix * 2 + k) as u32, iz as u32 ^ seed);
                m.tri(a, b, c, ground_col(w, cen, n, j));
            }
        }
    }
    meter.mark(&mut prof, &m, "terrain");
    const CONTAINER: [[f32; 3]; 4] = [[0.66, 0.27, 0.16], [0.18, 0.46, 0.48], [0.80, 0.60, 0.18], [0.34, 0.38, 0.47]];
    let groups = shade_groups(w);
    for (i, b) in w.blocks.iter().enumerate() {
        // Breakable route pieces and cabinets are drawn from the current mask
        // every frame, just like crates; none can survive in the static mesh.
        if b.dest.is_some() {
            continue;
        }
        if w.map == fallen_giant::ID && ferrite_shared::map::fallen_giant::owns(b) {
            fallen_giant::block(&mut m, w, b);
            meter.mark(&mut prof, &m, "fallen giant");
            continue;
        }
        let g = groups[i]; // shade of the group, so coplanar faces match
        if w.map == slag_cathedral::ID {
            slag_cathedral::block(&mut m, b, g);
            meter.mark(&mut prof, &m, "slag cathedral");
            continue;
        }
        if w.map == relay::ID && relay::owns(b) {
            continue;
        }
        let j = 0.9 + 0.2 * hash01(g as u32, 91);
        match b.kind {
            BlockKind::Crate => {}
            BlockKind::Concrete => {
                let k = j * (0.92 + 0.05 * b.tint as f32);
                m.cuboid(b.min, b.max, rgb(0.63 * k, 0.61 * k, 0.57 * k));
                // weathered cap
                m.cuboid(
                    vec3(b.min.x - 0.04, b.max.y - 0.12, b.min.z - 0.04),
                    b.max + vec3(0.04, 0.02, 0.04),
                    rgb(0.5 * k, 0.48 * k, 0.45 * k),
                );
                let g = b.min.y + 0.6;
                m.cuboid(
                    vec3(b.min.x - 0.06, g, b.min.z - 0.06),
                    vec3(b.max.x + 0.06, g + 0.35, b.max.z + 0.06),
                    rgb(0.55 * k, 0.53 * k, 0.5 * k),
                );
            }
            BlockKind::Step => {
                let k = 0.9 + 0.08 * b.tint as f32;
                m.cuboid(b.min, b.max, rgb(0.56 * k, 0.54 * k, 0.5 * k));
            }
            BlockKind::Container => {
                let c = CONTAINER[b.tint as usize % 4];
                m.cuboid(b.min, b.max, rgb(c[0] * j, c[1] * j, c[2] * j));
                let d = rgb(c[0] * 0.55, c[1] * 0.55, c[2] * 0.55);
                let e = 0.06;
                let long_x = b.max.x - b.min.x > b.max.z - b.min.z;
                for t in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
                    if long_x {
                        let x = b.min.x + (b.max.x - b.min.x) * t;
                        m.cuboid(vec3(x - 0.09, b.min.y, b.min.z - e), vec3(x + 0.09, b.max.y + e, b.max.z + e), d);
                    } else {
                        let z = b.min.z + (b.max.z - b.min.z) * t;
                        m.cuboid(vec3(b.min.x - e, b.min.y, z - 0.09), vec3(b.max.x + e, b.max.y + e, z + 0.09), d);
                    }
                }
            }
            BlockKind::Rock => {
                let k = j * (0.9 + 0.06 * b.tint as f32);
                let skew = vec3(hash01(i as u32, 3) - 0.5, 0.0, hash01(i as u32, 4) - 0.5) * 0.5;
                let c = rgb(0.46 * k, 0.43 * k, 0.41 * k);
                let y1 = b.min.y + (b.max.y - b.min.y) * 0.45;
                let y2 = b.min.y + (b.max.y - b.min.y) * 0.8;
                taper(&mut m, b.min, vec3(b.max.x, y1, b.max.z), 0.9, 1.0, Vec3::ZERO, skew * 0.3, c);
                taper(&mut m, vec3(b.min.x, y1, b.min.z), vec3(b.max.x, y2, b.max.z), 1.0, 0.8, skew * 0.3, skew * 0.7, shade(c, 1.05));
                taper(&mut m, vec3(b.min.x, y2, b.min.z), b.max, 0.8, 0.45, skew * 0.7, skew, shade(c, 1.1));
            }
            BlockKind::Sandbag => {
                let mid = (b.min.y + b.max.y) * 0.5 + 0.25;
                m.cuboid(b.min, vec3(b.max.x, mid, b.max.z), rgb(0.62 * j, 0.55 * j, 0.39 * j));
                m.cuboid(vec3(b.min.x + 0.08, mid, b.min.z + 0.08), b.max - vec3(0.08, 0.0, 0.08), rgb(0.68 * j, 0.6 * j, 0.42 * j));
            }
            BlockKind::Metal => {
                let k = j * (0.9 + 0.2 * b.tint as f32);
                m.cuboid(b.min, b.max, rgb(0.33 * k, 0.31 * k, 0.32 * k));
                if b.max.y - b.min.y > 8.0 {
                    // chimney hazard band
                    m.cuboid(
                        vec3(b.min.x - 0.05, b.max.y - 1.6, b.min.z - 0.05),
                        vec3(b.max.x + 0.05, b.max.y - 0.8, b.max.z + 0.05),
                        rgb(0.85, 0.38, 0.12),
                    );
                }
            }
            BlockKind::Trunk => {}
        }
        meter.mark(&mut prof, &m, "blocks");
    }
    if w.map == slag_cathedral::ID {
        slag_cathedral::build(&mut m, w);
        meter.mark(&mut prof, &m, "slag cathedral");
        special_landmarks::build(&mut m, w);
        meter.mark(&mut prof, &m, "relay mast / volcano");
        return (m, prof);
    }
    for (i, t) in w.trees.iter().enumerate() {
        if !monuments::hides_tree(w, t) {
            tree(&mut m, t, i);
        }
    }
    meter.mark(&mut prof, &m, "trees");
    build_ground_cover(&mut m, w);
    meter.mark(&mut prof, &m, "ground cover");
    flora::grow(&mut m, w);
    meter.mark(&mut prof, &m, "flora");
    if w.map == relay::ID {
        relay::build(&mut m, w);
        meter.mark(&mut prof, &m, "relay");
    }
    landmarks::build(&mut m, w);
    meter.mark(&mut prof, &m, "landmarks");
    tunnels::build(&mut m, w);
    meter.mark(&mut prof, &m, "tunnels");
    fallen_giant::build(&mut m, w);
    meter.mark(&mut prof, &m, "fallen giant");
    special_landmarks::build(&mut m, w);
    meter.mark(&mut prof, &m, "relay mast / volcano");
    monuments::build(&mut m, w);
    meter.mark(&mut prof, &m, "monuments");
    (m, prof)
}

/// Leaf colours a broadleaf tree's `tint` picks from.
pub const LEAF: [[f32; 3]; 5] = [[0.74, 0.33, 0.13], [0.86, 0.57, 0.16], [0.63, 0.20, 0.15], [0.42, 0.47, 0.20], [0.24, 0.40, 0.25]];

/// One tree. `i` is its index: mirrored pairs share a turn.
fn tree(m: &mut Mesh, t: &Tree, i: usize) {
    {
        let r = hash01(i as u32 / 2, 17) * std::f32::consts::TAU;
        let trunk = rgb(0.34, 0.25, 0.18);
        let th = t.height * 0.5;
        let lean = vec3(r.cos(), 0.0, r.sin()) * 0.12;
        let (foot, knee, head) = (t.base - vec3(0.0, 0.5, 0.0), t.base + vec3(0.0, th * 0.35, 0.0), t.base + vec3(0.0, th, 0.0) + lean);
        bole(m, foot, 0.36, knee, 0.25, r, trunk, i as u32);
        bole(m, knee, 0.25, head, 0.18, r, shade(trunk, 1.1), i as u32 + 7);
        if t.pine {
            let c = shade(rgb(0.17, 0.33, 0.25), 0.85 + 0.1 * t.tint as f32);
            for k in 0..4 {
                let f = k as f32 / 4.0;
                let base = t.base + vec3(0.0, t.height * (0.25 + 0.25 * f), 0.0);
                pyramid(m, base, t.radius * (1.0 - 0.24 * f), t.height * 0.36, r + f, shade(c, 1.0 + 0.1 * f));
            }
        } else {
            let lc = LEAF[t.tint as usize % 5];
            let c = rgb(lc[0], lc[1], lc[2]);
            let cen = t.base + vec3(0.0, t.height * 0.72, 0.0);
            octa(m, cen, vec3(t.radius, t.height * 0.34, t.radius), r, c);
            octa(
                m,
                cen + vec3(r.cos(), -0.8, r.sin()) * t.radius * 0.55,
                vec3(t.radius * 0.6, t.height * 0.2, t.radius * 0.6),
                r * 2.0,
                shade(c, 0.88),
            );
            octa(
                m,
                cen + vec3(-r.sin(), 0.5, r.cos()) * t.radius * 0.5,
                vec3(t.radius * 0.5, t.height * 0.17, t.radius * 0.5),
                r * 3.0,
                shade(c, 1.08),
            );
            // a branch reaching to the side blob
            let b0 = t.base + vec3(0.0, th * 0.8, 0.0);
            let b1 = cen + vec3(r.cos(), -0.8, r.sin()) * t.radius * 0.5;
            let side = vec3(-r.sin(), 0.0, r.cos()) * 0.07;
            m.quad(b0 - side, b0 + side, b1 + side, b1 - side, trunk);
            m.quad(b0 + side, b0 - side, b1 - side, b1 + side, shade(trunk, 0.8));
        }
    }
}

/// Grass tufts and pebbles: cheap detail that breaks up the ground.
fn build_ground_cover(m: &mut Mesh, w: &World) {
    let seed = w.seed as u32;
    let free = |x: f32, z: f32| !w.blocks.iter().any(|b| x > b.min.x - 0.2 && x < b.max.x + 0.2 && z > b.min.z - 0.2 && z < b.max.z + 0.2);
    // Same density as on the original 236 x 136 m footprint.
    let tufts = (26000.0 * (HALF_X + 6.0) * (HALF_Z + 6.0) / (124.0 * 74.0)) as u32;
    for k in 0..tufts {
        let x = (hash01(k, seed ^ 0x11) * 2.0 - 1.0) * (HALF_X + 6.0);
        let z = (hash01(k, seed ^ 0x22) * 2.0 - 1.0) * (HALF_Z + 6.0);
        let n = w.terrain_normal(x, z);
        if n.y < 0.93 || !free(x, z) || (w.map == fallen_giant::ID && fallen_giant::bare(x, z)) {
            continue;
        }
        let mut near = f32::MAX;
        for p in w.points.iter().chain(w.bases.iter()) {
            near = near.min(((x - p.x).powi(2) + (z - p.z).powi(2)).sqrt());
        }
        if near < 9.5 && hash01(k, 3) < 0.85 {
            continue; // trampled ground around objectives
        }
        let base = vec3(x, w.height(x, z), z);
        if k % 10 == 0 {
            let s = 0.07 + 0.13 * hash01(k, 5);
            let g = 0.4 + 0.14 * hash01(k, 8);
            taper(
                m,
                base + vec3(-s, -0.04, -s * 0.8),
                base + vec3(s, s * 0.8, s * 0.8),
                1.0,
                0.6,
                Vec3::ZERO,
                Vec3::ZERO,
                rgb(g, g * 0.95, g * 0.9),
            );
            continue;
        }
        let gc = ground_col(w, base, n, hash01(k, 4));
        let col =
            if hash01(k, 6) < 0.3 { rgb(gc[0] * 1.12, gc[1] * 1.05, gc[2] * 0.9) } else { rgb(gc[0] * 0.82, gc[1] * 0.98, gc[2] * 0.74) };
        let blades = 3 + (hash01(k, 7) * 3.0) as u32;
        for b in 0..blades {
            let a = hash01(k, 20 + b) * std::f32::consts::TAU;
            let hgt = 0.12 + 0.2 * hash01(k, 30 + b);
            let off = vec3(a.cos(), 0.0, a.sin()) * 0.12 * hash01(k, 40 + b);
            let wd = vec3(-a.sin(), 0.0, a.cos()) * 0.02;
            let tip = base + off + vec3(a.cos() * 0.08, hgt, a.sin() * 0.08);
            // Two pieces, so the blade bends over instead of standing as a spike.
            let (root, c) = (base + off, shade(col, 0.85 + 0.3 * hash01(k, 50 + b)));
            let mid = root + vec3(a.cos() * 0.022, hgt * 0.58, a.sin() * 0.022) + wd * 0.55;
            m.tri_w([root - wd, root + wd, mid], n, [0.0, 0.0, hgt * 0.14], c);
            m.tri_w([root - wd, mid, tip], n, [0.0, hgt * 0.14, hgt * 0.42], shade(c, 1.07));
        }
    }
}

/// Each map has its own hour: Relay and Slag Cathedral carry theirs with
/// their art, the rest take a `flora::Mood`.
pub fn fog_for(map: u8) -> [f32; 4] {
    match map {
        slag_cathedral::ID => slag_cathedral::FOG,
        relay::ID => relay::FOG,
        _ => flora::mood(map).map_or(FOG, |m| m.fog),
    }
}
pub fn sun_for(map: u8) -> Vec3 {
    match map {
        slag_cathedral::ID => slag_cathedral::SUN,
        relay::ID => relay::SUN,
        _ => flora::mood(map).map_or(SUN, |m| m.sun),
    }
}
/// Colour of the sun and of the two ambients.
pub fn tone_for(map: u8) -> crate::gfx::Tone {
    flora::mood(map).map_or(crate::gfx::TONE, |m| m.tone)
}
// The asset_review example compiles this module without the game loop.
#[allow(unused_imports)]
pub use flora::ambience;
pub fn shadow_bounds(map: u8) -> (Vec3, Vec3) {
    (vec3(-HALF_X - 8.0, -12.0, -HALF_Z - 8.0), vec3(HALF_X + 8.0, if map == slag_cathedral::ID { 52.0 } else { 34.0 }, HALF_Z + 8.0))
}
pub fn build_sky_for(map: u8) -> Mesh {
    match map {
        slag_cathedral::ID => slag_cathedral::sky(),
        relay::ID => relay::sky(),
        _ => flora::mood(map).map_or_else(build_sky, flora::sky),
    }
}
pub fn map_lights(w: &World, eye: Vec3, existing: [crate::gfx::Light; 4]) -> [crate::gfx::Light; 4] {
    match w.map {
        slag_cathedral::ID => slag_cathedral::lights(w, eye, existing),
        relay::ID => relay::lights(w, eye, existing),
        1 | 2 => tunnels::lights(w, eye, existing),
        _ => existing,
    }
}

pub fn build_sky() -> Mesh {
    let mut m = Mesh::default();
    let r = 420.0;
    let zen = [0.30, 0.55, 0.72];
    let hor = [FOG[0], FOG[1], FOG[2]];
    let col_at = |el: f32| {
        let t = smooth(0.0, 1.0, (el / 1.1).clamp(0.0, 1.0)).powf(0.7);
        let c = mix3(hor, zen, t);
        [c[0], c[1], c[2], 1.0]
    };
    let (rings, segs) = (14, 48);
    let pt = |el: f32, az: f32| vec3(el.cos() * az.cos(), el.sin(), el.cos() * az.sin()) * r;
    for ri in 0..rings {
        let e0 = -0.25 + (1.57 + 0.25) * ri as f32 / rings as f32;
        let e1 = -0.25 + (1.57 + 0.25) * (ri + 1) as f32 / rings as f32;
        for s in 0..segs {
            let a0 = s as f32 / segs as f32 * std::f32::consts::TAU;
            let a1 = (s + 1) as f32 / segs as f32 * std::f32::consts::TAU;
            m.quad_c(pt(e0, a0), pt(e0, a1), pt(e1, a1), pt(e1, a0), [col_at(e0), col_at(e0), col_at(e1), col_at(e1)]);
        }
    }
    // Sun: hard-edged disc with a soft halo.
    let s = SUN.normalize();
    let c = s * 400.0;
    let rt = s.cross(Vec3::Y).normalize();
    let up = rt.cross(s);
    for (size, col) in [(70.0, [1.0, 0.9, 0.65, 0.16]), (40.0, [1.0, 0.92, 0.7, 0.25]), (17.0, [1.0, 0.97, 0.86, 1.0])] {
        let n = 20;
        for k in 0..n {
            let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
            let a1 = (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
            m.tri(c, c + (rt * a0.cos() + up * a0.sin()) * size, c + (rt * a1.cos() + up * a1.sin()) * size, col);
        }
    }
    // Slab clouds.
    for k in 0..16u32 {
        let az = hash01(k, 1) * std::f32::consts::TAU;
        let el = 0.16 + hash01(k, 2) * 0.45;
        let p = pt(el, az) * 0.9;
        let sx = 40.0 + hash01(k, 3) * 60.0;
        let sz = 18.0 + hash01(k, 4) * 26.0;
        m.cuboid(p - vec3(sx, 3.0, sz), p + vec3(sx, 3.0, sz), [1.0, 0.96, 0.9, 0.82]);
        m.cuboid(p + vec3(-sx * 0.5, 3.0, -sz * 0.6), p + vec3(sx * 0.6, 8.0, sz * 0.5), [1.0, 0.97, 0.93, 0.82]);
    }
    m
}

pub fn crates(m: &mut Mesh, w: &World, dmask: u64) {
    for (i, b) in w.blocks.iter().enumerate() {
        if b.kind != BlockKind::Crate || !w.solid(i, dmask) {
            continue;
        }
        let k = 0.9 + 0.07 * b.tint as f32;
        m.cuboid(b.min, b.max, rgb(0.70 * k, 0.50 * k, 0.26 * k));
        let d = rgb(0.45 * k, 0.31 * k, 0.16 * k);
        let e = 0.03;
        let t = 0.09;
        for y in [b.min.y + 0.6, b.max.y - t] {
            m.cuboid(vec3(b.min.x - e, y, b.min.z - e), vec3(b.max.x + e, y + t + e, b.max.z + e), d);
        }
        for (cx, cz) in [(b.min.x, b.min.z), (b.max.x, b.min.z), (b.min.x, b.max.z), (b.max.x, b.max.z)] {
            m.cuboid(vec3(cx - t * 0.5, b.min.y + 0.6, cz - t * 0.5), vec3(cx + t * 0.5, b.max.y, cz + t * 0.5), d);
        }
    }
}

/// Colour of each kind of pickup (`proto::pickup::*`).
pub const PICKUP_COL: [[f32; 3]; 5] = [[0.35, 0.95, 0.45], [0.35, 0.65, 1.0], [0.95, 0.80, 0.30], [1.0, 0.55, 0.15], [0.95, 0.25, 0.85]];

/// Something to walk over: a small case that turns and bobs, marked with
/// what it is, under a short beam so it can be found in the grass.
pub fn pickup(m: &mut Mesh, add: &mut Mesh, pos: Vec3, kind: u8, time: f32, seed: f32) {
    use ferrite_shared::proto::pickup as pk;
    let c = PICKUP_COL[kind as usize % PICKUP_COL.len()];
    let col = rgb(c[0], c[1], c[2]);
    let bob = (time * 2.4 + seed).sin() * 0.08;
    let g = Mat4::from_translation(pos + vec3(0.0, 0.55 + bob, 0.0)) * Mat4::from_rotation_y(time * 1.6 + seed);
    let case = rgb(0.16, 0.17, 0.19);
    match kind {
        pk::HEALTH => {
            m.obox(&g, Vec3::ZERO, vec3(0.26, 0.2, 0.16), rgb(0.9, 0.9, 0.88));
            for z in [-0.165, 0.165] {
                m.obox(&g, vec3(0.0, 0.0, z), vec3(0.15, 0.045, 0.006), col);
                m.obox(&g, vec3(0.0, 0.0, z), vec3(0.045, 0.15, 0.006), col);
            }
        }
        pk::ARMOR => {
            // A plate carrier: chest plate over a darker back.
            m.obox(&g, Vec3::ZERO, vec3(0.24, 0.27, 0.08), case);
            m.obox(&g, vec3(0.0, 0.02, 0.085), vec3(0.19, 0.2, 0.02), col);
            m.obox(&g, vec3(0.0, 0.02, -0.085), vec3(0.19, 0.2, 0.02), col);
            for x in [-0.17, 0.17] {
                m.obox(&g, vec3(x, 0.3, 0.0), vec3(0.05, 0.05, 0.07), case);
            }
        }
        pk::AMMO => {
            m.obox(&g, Vec3::ZERO, vec3(0.28, 0.17, 0.17), rgb(0.25, 0.3, 0.2));
            m.obox(&g, vec3(0.0, 0.19, 0.0), vec3(0.29, 0.025, 0.18), col);
            m.obox(&g, vec3(0.0, 0.24, 0.0), vec3(0.1, 0.03, 0.03), case);
        }
        pk::OVERDRIVE => {
            // A chevron pointing the way: go.
            for (k, y) in [-0.14f32, 0.06, 0.26].into_iter().enumerate() {
                let s = 1.0 - k as f32 * 0.12;
                m.obox(&(g * Mat4::from_rotation_z(0.6)), vec3(-0.1 * s + y * 0.56, y * 0.82, 0.0), vec3(0.17 * s, 0.045, 0.05), col);
                m.obox(&(g * Mat4::from_rotation_z(-0.6)), vec3(0.1 * s - y * 0.56, y * 0.82, 0.0), vec3(0.17 * s, 0.045, 0.05), col);
            }
        }
        _ => {
            // A charged cell.
            m.tube(&(g * Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2)), Vec3::ZERO, 0.15, 0.15, 0.26, 10, case);
            m.tube(&(g * Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2)), Vec3::ZERO, 0.165, 0.165, 0.12, 10, col);
            m.obox(&g, vec3(0.0, 0.3, 0.0), vec3(0.06, 0.04, 0.06), rgb(0.8, 0.8, 0.82));
        }
    }
    let pulse = 0.75 + 0.25 * (time * 5.0 + seed).sin();
    for (dx, dz) in [(0.32, 0.0), (0.0, 0.32)] {
        let d = vec3(dx, 0.0, dz);
        let lo = [c[0], c[1], c[2], 0.42 * pulse];
        let hi = [c[0], c[1], c[2], 0.0];
        add.quad_c(
            pos - d + vec3(0.0, 0.05, 0.0),
            pos + d + vec3(0.0, 0.05, 0.0),
            pos + d + vec3(0.0, 5.0, 0.0),
            pos - d + vec3(0.0, 5.0, 0.0),
            [lo, lo, hi, hi],
        );
    }
}

/// A capture-the-flag flag: on its stand, on the ground where it fell, or
/// on a carrier's back (`carried`: its pole leans and it sits higher).
pub fn flag(m: &mut Mesh, add: &mut Mesh, pos: Vec3, team: u8, carried: bool, time: f32) {
    let c = team_col(team);
    let col = rgb(c[0], c[1], c[2]);
    let base = pos + vec3(0.0, if carried { 0.9 } else { 0.0 }, 0.0);
    let g = Mat4::from_translation(base) * Mat4::from_rotation_z(if carried { 0.16 } else { 0.0 });
    let h = if carried { 1.9 } else { 3.4 };
    m.obox(&g, vec3(0.0, h * 0.5, 0.0), vec3(0.035, h * 0.5, 0.035), rgb(0.86, 0.86, 0.84));
    if !carried {
        m.obox(&g, vec3(0.0, 0.06, 0.0), vec3(0.3, 0.06, 0.3), rgb(0.28, 0.28, 0.3));
    }
    // The cloth, in three strips so it can ripple.
    let (top, drop, len) = (h, 0.75, 1.25);
    for k in 0..3 {
        let x0 = 0.035 + len * k as f32 / 3.0;
        let x1 = 0.035 + len * (k + 1) as f32 / 3.0;
        let wave = |x: f32| (time * 5.0 + x * 4.0).sin() * 0.11 * x;
        let p = |x: f32, y: f32| g.transform_point3(vec3(x, y, wave(x)));
        let shade = 1.0 - 0.1 * k as f32;
        let cc = [col[0] * shade, col[1] * shade, col[2] * shade, col[3]];
        m.quad(p(x0, top), p(x1, top), p(x1, top - drop), p(x0, top - drop), cc);
        m.quad(p(x1, top), p(x0, top), p(x0, top - drop), p(x1, top - drop), cc);
    }
    if !carried {
        for (dx, dz) in [(0.5, 0.0), (0.0, 0.5)] {
            let d = vec3(dx, 0.0, dz);
            let lo = [c[0], c[1], c[2], 0.3];
            let hi = [c[0], c[1], c[2], 0.0];
            add.quad_c(
                pos - d + vec3(0.0, 3.4, 0.0),
                pos + d + vec3(0.0, 3.4, 0.0),
                pos + d + vec3(0.0, 46.0, 0.0),
                pos - d + vec3(0.0, 46.0, 0.0),
                [lo, lo, hi, hi],
            );
        }
    }
}

/// Objective beacons: flag pole, ground ring and a light pillar readable
/// from across the map. An owner of 255 hides that objective (modes that
/// do not use it).
pub fn beacons(m: &mut Mesh, alpha: &mut Mesh, add: &mut Mesh, w: &World, owners: &[(u8, f32, bool)], time: f32, dmask: u64) {
    beacons_profiled(m, alpha, add, w, owners, time, dmask, &mut Profile::new());
}

/// `beacons`, also counting the triangles each module adds this frame.
#[allow(clippy::too_many_arguments)]
pub fn beacons_profiled(
    m: &mut Mesh,
    alpha: &mut Mesh,
    add: &mut Mesh,
    w: &World,
    owners: &[(u8, f32, bool)],
    time: f32,
    dmask: u64,
    prof: &mut Profile,
) {
    let count = |m: &Mesh, alpha: &Mesh, add: &Mesh| m.triangles() + alpha.triangles() + add.triangles();
    let mut last = count(m, alpha, add);
    let mut mark = |m: &Mesh, alpha: &Mesh, add: &Mesh, name: &'static str| {
        let now = count(m, alpha, add);
        tally(prof, name, now - last);
        last = now;
    };
    if w.map == slag_cathedral::ID {
        slag_cathedral::atmosphere(alpha, add, w, time);
        mark(m, alpha, add, "slag cathedral");
    }
    if w.map == relay::ID {
        relay::atmosphere(m, alpha, add, w, time);
        mark(m, alpha, add, "relay");
    }
    landmarks::features(m, alpha, add, w, dmask, time);
    mark(m, alpha, add, "landmark features");
    landmarks::atmosphere(m, alpha, add, w, time, dmask);
    mark(m, alpha, add, "landmarks");
    special_landmarks::atmosphere(m, alpha, add, w, time);
    mark(m, alpha, add, "relay mast / volcano");
    fallen_giant::atmosphere(m, alpha, add, w, time);
    mark(m, alpha, add, "fallen giant");
    monuments::atmosphere(m, alpha, add, w, time);
    mark(m, alpha, add, "monuments");
    for (k, &(owner, progress, contested)) in owners.iter().enumerate() {
        if owner == 255 {
            continue;
        }
        let c = w.points[k];
        let oc = team_col(owner);
        // progress colour: who is currently winning the tug
        let lead = if progress > 0.02 {
            1
        } else if progress < -0.02 {
            0
        } else {
            NEUTRAL
        };
        let pc = team_col(lead);
        let pulse = if contested { 0.6 + 0.4 * (time * 7.0).sin() } else { 1.0 };
        m.cuboid(c + vec3(-0.09, 0.0, -0.09), c + vec3(0.09, 8.0, 0.09), rgb(0.9, 0.9, 0.88));
        m.cuboid(c + vec3(-0.5, 0.0, -0.5), c + vec3(0.5, 0.25, 0.5), rgb(0.3, 0.3, 0.32));
        let wave = (time * 2.0 + k as f32).sin() * 0.12;
        m.quad(
            c + vec3(0.09, 7.9, 0.0),
            c + vec3(2.0, 7.8 + wave, 0.35),
            c + vec3(2.0, 6.7 + wave, 0.35),
            c + vec3(0.09, 6.8, 0.0),
            [oc[0], oc[1], oc[2], 0.55],
        );
        // fill gauge on the pole
        let fill = progress.abs().clamp(0.0, 1.0) * 6.0;
        m.cuboid(c + vec3(-0.16, 0.3, -0.16), c + vec3(0.16, 0.3 + fill, 0.16), [pc[0], pc[1], pc[2], 0.7]);
        let segs = 40;
        for s in 0..segs {
            let a0 = s as f32 / segs as f32 * std::f32::consts::TAU;
            let a1 = (s + 1) as f32 / segs as f32 * std::f32::consts::TAU;
            let p = |a: f32, r: f32| {
                let (x, z) = (c.x + a.cos() * r, c.z + a.sin() * r);
                vec3(x, w.height(x, z) + 0.08, z)
            };
            let col = [oc[0], oc[1], oc[2], 0.6 * pulse];
            alpha.quad(p(a0, POINT_RADIUS - 0.4), p(a0, POINT_RADIUS), p(a1, POINT_RADIUS), p(a1, POINT_RADIUS - 0.4), col);
        }
        for (dx, dz) in [(0.8, 0.0), (0.0, 0.8)] {
            let d = vec3(dx, 0.0, dz);
            let lo = [oc[0], oc[1], oc[2], 0.22 * pulse];
            let hi = [oc[0], oc[1], oc[2], 0.0];
            add.quad_c(
                c - d + vec3(0.0, 8.0, 0.0),
                c + d + vec3(0.0, 8.0, 0.0),
                c + d + vec3(0.0, 60.0, 0.0),
                c - d + vec3(0.0, 60.0, 0.0),
                [lo, lo, hi, hi],
            );
        }
    }
    mark(m, alpha, add, "objectives");
}

/// Box stretched from `a` to `b` (a forearm, a thigh...).
fn limb(m: &mut Mesh, g: &Mat4, a: Vec3, b: Vec3, r: f32, round: bool, col: Col) {
    let d = b - a;
    let len = d.length();
    if len < 1e-4 {
        return;
    }
    let z = d / len;
    let up = if z.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let x = up.cross(z).normalize();
    let y = z.cross(x);
    let basis = Mat4::from_cols(x.extend(0.0), y.extend(0.0), z.extend(0.0), ((a + b) * 0.5).extend(1.0));
    if round {
        m.tube(&(*g * basis), Vec3::ZERO, r, 0.0, len * 0.5, 12, col);
    } else {
        m.obox(&(*g * basis), Vec3::ZERO, vec3(r, r, len * 0.5), col);
    }
}

#[path = "scene/players.rs"]
mod players;
#[path = "scene/weapons.rs"]
mod weapons;
pub use players::*;
pub use weapons::*;

#[path = "scene/fallen_giant.rs"]
mod fallen_giant;
#[path = "scene/flora.rs"]
mod flora;
#[path = "scene/landmarks.rs"]
mod landmarks;
#[path = "scene/monuments.rs"]
pub mod monuments;
#[path = "scene/relay.rs"]
mod relay;
#[path = "scene/slag_cathedral.rs"]
mod slag_cathedral;
#[path = "scene/special_landmarks.rs"]
mod special_landmarks;
#[path = "scene/tunnels.rs"]
mod tunnels;
