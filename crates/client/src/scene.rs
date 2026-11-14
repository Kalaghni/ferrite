//! Everything that turns game state into low-poly geometry.

use ferrite_shared::sim::weapon_def;
use crate::gfx::{rgb, shade, Col, Mesh};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::*;
use ferrite_shared::proto::flag;
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
    let [grass, dry, dirt] = map_def(w.map).ground;
    let patch = vnoise(c.x / 24.0, c.z / 24.0, w.seed as u32 ^ 7);
    let mut col = mix3(grass, dry, smooth(0.3, 0.75, patch));
    let mut near = f32::MAX;
    for p in w.points.iter().chain(w.bases.iter()) {
        near = near.min(((c.x - p.x).powi(2) + (c.z - p.z).powi(2)).sqrt());
    }
    col = mix3(col, dirt, 1.0 - smooth(7.0, 16.0, near));
    col = mix3(col, [0.47, 0.40, 0.35], smooth(0.06, 0.22, 1.0 - n.y));
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

fn octa(m: &mut Mesh, c: Vec3, r: Vec3, rot: f32, col: Col) {
    let (s, co) = rot.sin_cos();
    let h = |a: f32, b: f32| c + vec3((a * co - b * s) * r.x, 0.0, (a * s + b * co) * r.z);
    const N: usize = 7;
    let top = c + vec3(0.0, r.y, 0.0);
    let bot = c - vec3(0.0, r.y * 0.7, 0.0);
    let ring = |k: usize, rad: f32, y: f32, tw: f32| {
        let a = (k % N) as f32 / N as f32 * std::f32::consts::TAU + tw;
        h(a.cos() * rad, a.sin() * rad) + vec3(0.0, y, 0.0)
    };
    for k in 0..N {
        let j = 0.9 + 0.2 * hash01(k as u32, (r.x * 97.0) as u32);
        let (u0, u1) = (ring(k, 0.72, r.y * 0.5, 0.2), ring(k + 1, 0.72, r.y * 0.5, 0.2));
        let (m0, m1) = (ring(k, 1.0, 0.0, 0.0), ring(k + 1, 1.0, 0.0, 0.0));
        let (l0, l1) = (ring(k, 0.6, -r.y * 0.45, -0.2), ring(k + 1, 0.6, -r.y * 0.45, -0.2));
        m.tri(u0, top, u1, shade(col, 1.06 * j));
        m.quad(m0, u0, u1, m1, shade(col, 0.96 * j));
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
        m.tri(p0, top, p1, shade(col, 0.9 + 0.2 * hash01(k as u32, (r * 100.0) as u32)));
        m.tri(p1, base + vec3(0.0, h * 0.12, 0.0), p0, shade(col, 0.6));
    }
}

pub fn build_world(w: &World) -> Mesh {
    let mut m = Mesh::default();
    let seed = w.seed as u32;
    let hp = |ix: usize, iz: usize| {
        vec3(TER_X0 + ix as f32 * CELL, w.heights[iz * (TNX + 1) + ix], TER_Z0 + iz as f32 * CELL)
    };
    for iz in 0..TNZ {
        for ix in 0..TNX {
            let (p00, p10, p01, p11) = (hp(ix, iz), hp(ix + 1, iz), hp(ix, iz + 1), hp(ix + 1, iz + 1));
            for (k, (a, b, c)) in [(p00, p01, p10), (p11, p10, p01)].into_iter().enumerate() {
                let n = (b - a).cross(c - a).normalize();
                let cen = (a + b + c) / 3.0;
                let j = hash01((ix * 2 + k) as u32, iz as u32 ^ seed);
                m.tri(a, b, c, ground_col(w, cen, n, j));
            }
        }
    }
    const CONTAINER: [[f32; 3]; 4] = [[0.66, 0.27, 0.16], [0.18, 0.46, 0.48], [0.80, 0.60, 0.18], [0.34, 0.38, 0.47]];
    for (i, b) in w.blocks.iter().enumerate() {
        let j = 0.9 + 0.2 * hash01(i as u32, 91);
        match b.kind {
            BlockKind::Crate => {}
            BlockKind::Concrete => {
                let k = j * (0.92 + 0.05 * b.tint as f32);
                m.cuboid(b.min, b.max, rgb(0.63 * k, 0.61 * k, 0.57 * k));
                // weathered cap
                m.cuboid(vec3(b.min.x - 0.04, b.max.y - 0.12, b.min.z - 0.04), b.max + vec3(0.04, 0.02, 0.04), rgb(0.5 * k, 0.48 * k, 0.45 * k));
                let g = b.min.y + 0.6;
                m.cuboid(vec3(b.min.x - 0.06, g, b.min.z - 0.06), vec3(b.max.x + 0.06, g + 0.35, b.max.z + 0.06), rgb(0.55 * k, 0.53 * k, 0.5 * k));
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
                    m.cuboid(vec3(b.min.x - 0.05, b.max.y - 1.6, b.min.z - 0.05), vec3(b.max.x + 0.05, b.max.y - 0.8, b.max.z + 0.05), rgb(0.85, 0.38, 0.12));
                }
            }
            BlockKind::Trunk => {}
        }
    }
    const LEAF: [[f32; 3]; 5] =
        [[0.74, 0.33, 0.13], [0.86, 0.57, 0.16], [0.63, 0.20, 0.15], [0.42, 0.47, 0.20], [0.24, 0.40, 0.25]];
    for (i, t) in w.trees.iter().enumerate() {
        let r = hash01(i as u32 / 2, 17) * std::f32::consts::TAU;
        let trunk = rgb(0.34, 0.25, 0.18);
        let th = t.height * 0.5;
        taper(&mut m, t.base + vec3(-0.3, -0.5, -0.3), t.base + vec3(0.3, th * 0.35, 0.3), 1.0, 0.7, Vec3::ZERO, Vec3::ZERO, trunk);
        taper(&mut m, t.base + vec3(-0.3, th * 0.35, -0.3), t.base + vec3(0.3, th, 0.3), 0.7, 0.5, Vec3::ZERO, vec3(r.cos(), 0.0, r.sin()) * 0.12, shade(trunk, 1.1));
        if t.pine {
            let c = shade(rgb(0.17, 0.33, 0.25), 0.85 + 0.1 * t.tint as f32);
            for k in 0..4 {
                let f = k as f32 / 4.0;
                let base = t.base + vec3(0.0, t.height * (0.25 + 0.25 * f), 0.0);
                pyramid(&mut m, base, t.radius * (1.0 - 0.24 * f), t.height * 0.36, r + f, shade(c, 1.0 + 0.1 * f));
            }
        } else {
            let lc = LEAF[t.tint as usize % 5];
            let c = rgb(lc[0], lc[1], lc[2]);
            let cen = t.base + vec3(0.0, t.height * 0.72, 0.0);
            octa(&mut m, cen, vec3(t.radius, t.height * 0.34, t.radius), r, c);
            octa(&mut m, cen + vec3(r.cos(), -0.8, r.sin()) * t.radius * 0.55, vec3(t.radius * 0.6, t.height * 0.2, t.radius * 0.6), r * 2.0, shade(c, 0.88));
            octa(&mut m, cen + vec3(-r.sin(), 0.5, r.cos()) * t.radius * 0.5, vec3(t.radius * 0.5, t.height * 0.17, t.radius * 0.5), r * 3.0, shade(c, 1.08));
            // a branch reaching to the side blob
            let b0 = t.base + vec3(0.0, th * 0.8, 0.0);
            let b1 = cen + vec3(r.cos(), -0.8, r.sin()) * t.radius * 0.5;
            let side = vec3(-r.sin(), 0.0, r.cos()) * 0.07;
            m.quad(b0 - side, b0 + side, b1 + side, b1 - side, trunk);
            m.quad(b0 + side, b0 - side, b1 - side, b1 + side, shade(trunk, 0.8));
        }
    }
    // Grass tufts and pebbles: cheap detail that breaks up the ground.
    let free = |x: f32, z: f32| !w.blocks.iter().any(|b| x > b.min.x - 0.2 && x < b.max.x + 0.2 && z > b.min.z - 0.2 && z < b.max.z + 0.2);
    for k in 0..26000u32 {
        let x = (hash01(k, seed ^ 0x11) * 2.0 - 1.0) * (HALF_X + 6.0);
        let z = (hash01(k, seed ^ 0x22) * 2.0 - 1.0) * (HALF_Z + 6.0);
        let n = w.terrain_normal(x, z);
        if n.y < 0.93 || !free(x, z) {
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
            taper(&mut m, base + vec3(-s, -0.04, -s * 0.8), base + vec3(s, s * 0.8, s * 0.8), 1.0, 0.6, Vec3::ZERO, Vec3::ZERO, rgb(g, g * 0.95, g * 0.9));
            continue;
        }
        let gc = ground_col(w, base, n, hash01(k, 4));
        let col = if hash01(k, 6) < 0.3 { rgb(gc[0] * 1.12, gc[1] * 1.05, gc[2] * 0.9) } else { rgb(gc[0] * 0.82, gc[1] * 0.98, gc[2] * 0.74) };
        let blades = 3 + (hash01(k, 7) * 3.0) as u32;
        for b in 0..blades {
            let a = hash01(k, 20 + b) * std::f32::consts::TAU;
            let hgt = 0.12 + 0.2 * hash01(k, 30 + b);
            let off = vec3(a.cos(), 0.0, a.sin()) * 0.12 * hash01(k, 40 + b);
            let wd = vec3(-a.sin(), 0.0, a.cos()) * 0.02;
            let tip = base + off + vec3(a.cos() * 0.08, hgt, a.sin() * 0.08);
            m.tri_n(base + off - wd, base + off + wd, tip, n, shade(col, 0.85 + 0.3 * hash01(k, 50 + b)));
        }
    }
    m
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

#[derive(Clone, Copy, Default)]
pub struct PlayerView {
    pub id: u8,
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub flags: u8,
    pub flags2: u8,
    pub hp: u8,
    pub team: u8,
    pub kit: u8,
    /// Family of the primary carried (0 rifle, 1 shotgun, 2 long rifle).
    pub gun: u8,
    pub phase: f32,
    pub moving: f32,
    /// Smoothed 0..1 pose weights.
    pub crouch: f32,
    pub air: f32,
    pub slide: f32,
    pub reload: f32,
    /// 1 on the frame a shot is fired, decaying.
    pub fire: f32,
    /// 1 on the frame damage is taken, decaying.
    pub hurt: f32,
    /// Velocity in the soldier's own frame (forward, right), m/s.
    pub vel: (f32, f32),
    /// 0 = on their feet, 1 = flat on the ground.
    pub fall: f32,
    /// How far a body has sunk into the ground before it is removed.
    pub sink: f32,
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

/// A soldier, posed from their movement: stride follows the direction of
/// travel, the torso leans into a run, knees fold for a crouch or tuck for
/// a jump, and the weapon drops for a sprint or a reload. `blob` draws a
/// simple ground shadow for when real shadows are off.
pub fn soldier(m: &mut Mesh, alpha: &mut Mesh, w: &World, p: &PlayerView, time: f32, blob: bool) {
    let tc = team_col(p.team);
    let shield = p.flags & flag::SHIELD != 0;
    let glow = if p.flags & flag::SURGE != 0 { 0.35 + 0.25 * (time * 9.0).sin() } else { 0.0 };
    // Taking a hit flashes the whole figure toward white.
    let h = p.hurt * 0.6;
    let tint = |c: [f32; 3], e: f32| -> Col { [c[0] + (1.0 - c[0]) * h, c[1] + (1.0 - c[1]) * h, c[2] + (1.0 - c[2]) * h, e.max(h * 0.5)] };
    let main = tint(tc, glow);
    let dark = tint([tc[0] * 0.42, tc[1] * 0.42, tc[2] * 0.45], glow * 0.5);
    let metal = tint([0.16, 0.16, 0.18], 0.0);
    let skin = tint([0.86, 0.78, 0.66], 0.0);
    let glove = tint([0.2, 0.19, 0.18], 0.0);

    let sl = p.slide;
    let c = p.crouch.max(sl);
    let kneel = p.crouch * (1.0 - sl);
    let air = p.air * (1.0 - c);
    let speed = (p.vel.0 * p.vel.0 + p.vel.1 * p.vel.1).sqrt();
    let sprint = ((speed - 6.4) / 1.6).clamp(0.0, 1.0) * (1.0 - c);
    let fall = p.fall * p.fall * (3.0 - 2.0 * p.fall);
    let root = Mat4::from_translation(p.pos + vec3(0.0, 0.17 * fall - p.sink, 0.0)) * Mat4::from_rotation_y(p.yaw) * Mat4::from_rotation_x(-fall * 1.5);
    let bulk = if p.kit == 1 { 1.18 } else { 1.0 };

    // ---- legs ----
    // Stride along the direction of travel, so strafing sidesteps.
    let la = -p.vel.1.atan2(p.vel.0.abs().max(0.4)) * p.moving.min(1.0) * (1.0 - c);
    let back = if p.vel.0 < -0.5 { -1.0 } else { 1.0 };
    let swing = (p.phase * 2.2).sin() * (0.72 + 0.22 * sprint) * p.moving * (1.0 - sl) * (1.0 - 0.55 * kneel) * (1.0 - 0.6 * air) * (1.0 - fall) * back;
    let hip_y = 0.78 - 0.44 * c;
    for (side, s, lead) in [(0.14f32, swing, 1.0f32), (-0.14, -swing, 0.0)] {
        // Kneel: lead knee up, rear knee down. Slide: legs thrown forward.
        let thigh = s * (1.0 - 0.5 * kneel) - kneel * (0.55 + 0.9 * lead) - air * (0.3 + 0.35 * lead) - sl * (1.3 + 0.2 * lead) + fall * 0.1 * (lead - 0.5);
        let knee_a = (s.max(0.0) * 0.9 + 0.05) * (1.0 - c) + kneel * (2.0 - 0.5 * lead) + air * (0.5 + 0.7 * (1.0 - lead)) + sl * (0.2 + 0.9 * (1.0 - lead));
        let hip = root * Mat4::from_translation(vec3(side, hip_y, 0.0)) * Mat4::from_rotation_y(la) * Mat4::from_rotation_x(thigh);
        m.obox(&hip, vec3(0.0, -0.19, 0.0), vec3(0.11, 0.2, 0.125), dark);
        let knee = hip * Mat4::from_translation(vec3(0.0, -0.38, 0.0)) * Mat4::from_rotation_x(knee_a);
        m.obox(&knee, vec3(0.0, -0.16, 0.0), vec3(0.095, 0.17, 0.11), shade(dark, 0.85));
        m.obox(&knee, vec3(0.0, -0.01, 0.07), vec3(0.08, 0.06, 0.05), metal);
        m.obox(&knee, vec3(0.0, -0.35, 0.04), vec3(0.105, 0.055, 0.16), metal);
    }

    // ---- torso ----
    let idle = (time * 1.7 + p.id as f32).sin() * 0.008 * (1.0 - fall);
    let bob = (p.phase * 4.4).sin().abs() * (0.04 + 0.025 * sprint) * p.moving * (1.0 - sl);
    let lean = (0.07 * (p.vel.0 / 5.6).clamp(-1.0, 1.0) + 0.16 * sprint) * (1.0 - sl) + 0.14 * kneel - 0.6 * sl + 0.1 * air - 0.05 * p.fire;
    let roll = (p.vel.1 / 5.6).clamp(-1.0, 1.0) * 0.07 * (1.0 - c);
    let body = root
        * Mat4::from_translation(vec3(0.0, bob + idle - 0.5 * c + 0.84, 0.0))
        * Mat4::from_rotation_x(lean)
        * Mat4::from_rotation_z(roll)
        * Mat4::from_translation(vec3(0.0, -0.84, 0.0));
    m.obox(&body, vec3(0.0, 1.12, 0.0), vec3(0.27 * bulk, 0.34, 0.16 * bulk), main);
    m.obox(&body, vec3(0.0, 0.84, 0.0), vec3(0.28 * bulk, 0.06, 0.17 * bulk), metal);
    m.obox(&body, vec3(0.0, 1.2, 0.165 * bulk), vec3(0.2 * bulk, 0.2, 0.025), shade(main, 0.8));
    for sx in [-1.0f32, 1.0] {
        m.obox(&body, vec3(sx * 0.13, 0.95, 0.19 * bulk), vec3(0.06, 0.07, 0.04), metal);
        m.obox(&body, vec3(sx * 0.31 * bulk, 1.4, 0.0), vec3(0.09, 0.07, 0.13), shade(main, 1.1));
    }
    m.obox(&body, vec3(0.0, 1.48, 0.0), vec3(0.07, 0.04, 0.07), shade(skin, 0.85));
    match p.kit {
        1 => m.obox(&body, vec3(0.0, 1.2, -0.24), vec3(0.22, 0.26, 0.09), metal),
        2 => m.obox(&body, vec3(0.0, 1.3, -0.2), vec3(0.2, 0.3, 0.04), dark),
        _ => m.obox(&body, vec3(0.0, 1.15, -0.21), vec3(0.16, 0.18, 0.06), dark),
    }

    // ---- head: stays level while the body leans ----
    let head = body * Mat4::from_translation(vec3(0.0, 1.5, 0.0)) * Mat4::from_rotation_x(-p.pitch * 0.6 * (1.0 - fall) - lean * 0.8);
    m.obox(&head, vec3(0.0, 0.14, 0.0), vec3(0.15, 0.15, 0.15), skin);
    m.obox(&head, vec3(0.0, 0.25, -0.01), vec3(0.18, 0.09, 0.19), main);
    m.obox(&head, vec3(0.0, 0.15, 0.155), vec3(0.12, 0.04, 0.01), metal);
    m.obox(&head, vec3(0.0, 0.2, 0.2), vec3(0.17, 0.02, 0.05), shade(main, 0.85));
    for sx in [-1.0f32, 1.0] {
        m.obox(&head, vec3(sx * 0.165, 0.13, -0.02), vec3(0.02, 0.07, 0.08), shade(main, 0.9));
    }

    // ---- arms and weapon ----
    // The muzzle follows the aim, drops for a sprint or a reload, and kicks
    // when a shot goes off.
    let relaxed = (0.5 * sprint + 0.3 * air) * (1.0 - p.fire) + 0.55 * p.reload + 1.1 * fall;
    let aim = -p.pitch * (1.0 - 0.6 * sprint) * (1.0 - fall) - lean + relaxed - 0.07 * p.fire;
    let arms = body
        * Mat4::from_translation(vec3(0.0, 1.36, -0.045 * p.fire))
        * Mat4::from_rotation_x(aim)
        * Mat4::from_rotation_y(0.45 * sprint * (1.0 - p.fire) + 0.25 * p.reload);
    let sidearm = p.flags & flag::SIDEARM != 0;
    let glen = if sidearm { 0.12 } else { [0.36, 0.3, 0.52][p.gun as usize % 3] };
    let gx = -0.2;
    m.obox(&arms, vec3(gx, -0.08, 0.4 + glen * 0.5), vec3(0.04, 0.06, glen), metal);
    m.obox(&arms, vec3(gx, -0.2, 0.42), vec3(0.03, 0.08, 0.05), metal);
    m.obox(&arms, vec3(gx, 0.0, 0.5), vec3(0.012, 0.02, 0.05), metal);
    if !sidearm {
        m.obox(&arms, vec3(gx, -0.07, 0.22), vec3(0.035, 0.05, 0.1), shade(metal, 0.8));
        m.obox(&arms, vec3(gx, -0.02, 0.4 + glen * 1.5), vec3(0.016, 0.016, glen * 0.5), shade(metal, 0.7));
        if p.reload < 0.5 {
            m.obox(&arms, vec3(gx, -0.2, 0.56), vec3(0.025, 0.08, 0.04), shade(metal, 0.7));
        }
    }
    // Right arm: shoulder, elbow, grip.
    let r_sh = vec3(-0.31 * bulk, 0.02, 0.0);
    let r_hand = vec3(gx, -0.17, 0.42);
    let r_el = vec3(-0.36 * bulk, -0.24, 0.14);
    limb(m, &arms, r_sh, r_el, 0.075, false, dark);
    limb(m, &arms, r_el, r_hand, 0.065, false, shade(dark, 0.9));
    m.obox(&arms, r_hand, vec3(0.05, 0.055, 0.05), glove);
    // Left arm: supports the fore-end, or comes off it to work the magazine.
    let l_sh = vec3(0.31 * bulk, 0.02, 0.0);
    let support = if sidearm { vec3(gx + 0.05, -0.2, 0.42) } else { vec3(gx + 0.02, -0.14, 0.4 + glen * 1.1) };
    let l_hand = support + (vec3(gx + 0.04, -0.3, 0.5) - support) * p.reload;
    let l_el = vec3(0.24 * bulk, -0.26, 0.2);
    limb(m, &arms, l_sh, l_el, 0.075, false, dark);
    limb(m, &arms, l_el, l_hand, 0.065, false, shade(dark, 0.9));
    m.obox(&arms, l_hand, vec3(0.05, 0.05, 0.055), glove);

    if shield {
        let a = 0.25 + 0.15 * (time * 12.0).sin();
        alpha.cuboid(p.pos + vec3(-0.5, 0.0, -0.5), p.pos + vec3(0.5, 1.95 - 0.5 * c, 0.5), [1.0, 1.0, 1.0, a * 0.25]);
    }
    if blob {
        let gy = if p.flags & flag::GROUND != 0 { p.pos.y } else { w.height(p.pos.x, p.pos.z) } + 0.04;
        let r = 0.55;
        alpha.quad(
            vec3(p.pos.x - r, gy, p.pos.z - r),
            vec3(p.pos.x - r, gy, p.pos.z + r),
            vec3(p.pos.x + r, gy, p.pos.z + r),
            vec3(p.pos.x + r, gy, p.pos.z - r),
            [0.05, 0.04, 0.06, 0.4],
        );
    }
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

/// Objective beacons: flag pole, ground ring and a light pillar readable
/// from across the map.
pub fn beacons(m: &mut Mesh, alpha: &mut Mesh, add: &mut Mesh, w: &World, owners: &[(u8, f32, bool)], time: f32) {
    for (k, &(owner, progress, contested)) in owners.iter().enumerate() {
        let c = w.points[k];
        let oc = team_col(owner);
        // progress colour: who is currently winning the tug
        let lead = if progress > 0.02 { 1 } else if progress < -0.02 { 0 } else { NEUTRAL };
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
            add.quad_c(c - d + vec3(0.0, 8.0, 0.0), c + d + vec3(0.0, 8.0, 0.0), c + d + vec3(0.0, 60.0, 0.0), c - d + vec3(0.0, 60.0, 0.0), [lo, lo, hi, hi]);
        }
    }
}

pub const SIGHT_NAMES: [&str; 6] = ["Iron sights", "Red dot", "Holo", "2.5x optic", "4x scope", "8x scope"];
pub const IRON: u8 = 0;
pub const RED_DOT: u8 = 1;
pub const HOLO: u8 = 2;
pub const OPTIC: u8 = 3;
pub const SCOPE4: u8 = 4;
pub const SCOPE8: u8 = 5;

/// Sights each weapon can mount, first entry is the default.
pub fn sights_for(wid: u8) -> &'static [u8] {
    if wid == 9 {
        return &[OPTIC, SCOPE4, RED_DOT, IRON]; // the Ranger ships with glass
    }
    match weapon_def(wid).arch {
        0 => &[IRON, RED_DOT, HOLO, OPTIC],
        1 => &[IRON, RED_DOT, HOLO],
        2 => &[SCOPE4, SCOPE8, OPTIC, RED_DOT, IRON],
        _ => &[IRON, RED_DOT],
    }
}

/// Vertical field of view when aiming through a sight.
pub fn sight_fov(wid: u8, sight: u8) -> f32 {
    let iron = match weapon_def(wid).arch {
        0 => 48.0,
        1 => 58.0,
        2 => 46.0,
        _ => 56.0,
    };
    match sight {
        RED_DOT => iron - 2.0,
        HOLO => 42.0,
        OPTIC => 27.0,
        SCOPE4 => 22.0,
        SCOPE8 => 11.0,
        _ => iron,
    }
}

/// Top rail of each weapon in gun space: (height, z, scale).
fn rail(wid: u8) -> (f32, f32, f32) {
    match weapon_def(wid).arch {
        0 => (0.066, -0.02, 1.3),
        1 => (0.063, -0.1, 1.3),
        2 => (0.045, -0.12, 1.3),
        _ => (0.051, -0.03, 0.8),
    }
}

/// Height of the sight picture above the rail (what lines up with the eye).
fn sight_center(sight: u8) -> f32 {
    match sight {
        RED_DOT => 0.04,
        HOLO => 0.042,
        OPTIC => 0.036,
        _ => 0.0,
    }
}

fn optic(m: &mut Mesh, g: &Mat4, wid: u8, sight: u8, accent: Col, dark: Col, metal: Col) {
    let (ry, rz, s) = rail(wid);
    const N: usize = 28;
    let at = |c: [f32; 3]| vec3(c[0] * s, ry + c[1] * s, rz + c[2] * s);
    let bx = |m: &mut Mesh, c: [f32; 3], h: [f32; 3], col: Col| m.obox(g, at(c), vec3(h[0], h[1], h[2]) * s, col);
    let tb = |m: &mut Mesh, c: [f32; 3], r0: f32, r1: f32, hl: f32, col: Col| m.tube(g, at(c), r0 * s, r1 * s, hl * s, N, col);
    // Picatinny-style mount shared by every optic.
    let mount = |m: &mut Mesh, len: f32| {
        bx(m, [0.0, 0.004, 0.0], [0.013, 0.004, len], dark);
        for k in -2i32..=2 {
            bx(m, [0.0, 0.004, k as f32 * len * 0.42], [0.017, 0.005, len * 0.09], metal);
        }
    };
    match sight {
        RED_DOT => {
            mount(m, 0.034);
            bx(m, [0.0, 0.012, -0.02], [0.012, 0.006, 0.02], dark);
            tb(m, [0.0, 0.04, -0.02], 0.0295, 0.026, 0.024, dark);
            tb(m, [0.0, 0.04, -0.046], 0.031, 0.026, 0.003, metal);
            tb(m, [0.0, 0.04, 0.006], 0.031, 0.026, 0.003, metal);
            // brightness dial and battery cap
            m.tube(&(*g * Mat4::from_translation(at([0.034, 0.04, -0.02])) * Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2)), Vec3::ZERO, 0.011 * s, 0.0, 0.006 * s, 16, accent);
            m.tube(&(*g * Mat4::from_translation(at([0.0, 0.073, -0.02])) * Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2)), Vec3::ZERO, 0.008 * s, 0.0, 0.004 * s, 16, metal);
        }
        HOLO => {
            mount(m, 0.046);
            bx(m, [0.0, 0.012, 0.0], [0.02, 0.006, 0.046], dark);
            bx(m, [0.0, 0.014, 0.03], [0.022, 0.006, 0.018], dark);
            // chamfered hood: uprights, top, and angled shoulders
            for x in [-0.027f32, 0.027] {
                bx(m, [x, 0.04, -0.012], [0.0028, 0.026, 0.026], metal);
                let sh = *g * Mat4::from_translation(at([x * 0.84, 0.0695, -0.012])) * Mat4::from_rotation_z(-x.signum() * 0.8);
                m.obox(&sh, Vec3::ZERO, vec3(0.008, 0.0026, 0.026) * s, metal);
            }
            bx(m, [0.0, 0.0745, -0.012], [0.019, 0.0028, 0.026], metal);
            bx(m, [0.0, 0.014, -0.036], [0.027, 0.002, 0.002], metal);
            bx(m, [0.0, 0.079, 0.004], [0.01, 0.003, 0.006], accent);
            for z in [0.02f32, 0.034] {
                m.tube(&(*g * Mat4::from_translation(at([-0.024, 0.022, z])) * Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2)), Vec3::ZERO, 0.005 * s, 0.0, 0.004 * s, 12, accent);
            }
        }
        OPTIC => {
            mount(m, 0.05);
            bx(m, [0.0, 0.012, 0.0], [0.011, 0.006, 0.03], dark);
            tb(m, [0.0, 0.036, 0.0], 0.0235, 0.0205, 0.07, dark);
            tb(m, [0.0, 0.036, -0.082], 0.029, 0.0235, 0.014, dark);
            tb(m, [0.0, 0.036, -0.097], 0.0305, 0.0245, 0.003, metal);
            tb(m, [0.0, 0.036, 0.078], 0.0265, 0.0205, 0.01, metal);
            tb(m, [0.0, 0.036, 0.03], 0.0255, 0.0235, 0.004, accent);
            m.tube(&(*g * Mat4::from_translation(at([0.0, 0.064, -0.01])) * Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2)), Vec3::ZERO, 0.01 * s, 0.0, 0.006 * s, 16, metal);
            m.tube(&(*g * Mat4::from_translation(at([0.028, 0.036, -0.01])) * Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2)), Vec3::ZERO, 0.01 * s, 0.0, 0.006 * s, 16, metal);
        }
        _ => {
            // Rifle scope (seen from the hip; aiming shows the scope picture).
            let long = if sight == SCOPE8 { 0.2 } else { 0.15 };
            for z in [-0.07f32, 0.07] {
                bx(m, [0.0, 0.012, z], [0.012, 0.012, 0.012], metal);
                tb(m, [0.0, 0.04, z], 0.0225, 0.019, 0.01, metal);
            }
            tb(m, [0.0, 0.04, 0.0], 0.019, 0.016, long, dark);
            tb(m, [0.0, 0.04, -long - 0.03], 0.03, 0.025, 0.04, dark);
            tb(m, [0.0, 0.04, -long - 0.072], 0.0315, 0.026, 0.003, accent);
            tb(m, [0.0, 0.04, long + 0.02], 0.025, 0.02, 0.03, dark);
            tb(m, [0.0, 0.04, long + 0.052], 0.0265, 0.02, 0.003, metal);
            m.tube(&(*g * Mat4::from_translation(at([0.0, 0.064, 0.0])) * Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2)), Vec3::ZERO, 0.011 * s, 0.0, 0.008 * s, 16, metal);
            m.tube(&(*g * Mat4::from_translation(at([0.025, 0.04, 0.0])) * Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2)), Vec3::ZERO, 0.011 * s, 0.0, 0.008 * s, 16, metal);
        }
    }
}

pub struct ViewModel {
    pub sight: u8,
    /// Weapon family (0 rifle, 1 shotgun, 2 long rifle, 4 handgun).
    pub kit: u8,
    /// The exact weapon, for the details that set it apart.
    pub wid: u8,
    pub team: u8,
    pub ads: f32,
    pub recoil: f32,
    pub bob: f32,
    pub bob_amt: f32,
    /// Reload progress 0..1, negative when not reloading.
    pub reload: f32,
    /// 1 as the weapon starts coming up, 0 once it is ready.
    pub draw: f32,
    /// 1 just after a shot, 0 when the next round is ready (pump, bolt).
    pub cycle: f32,
    pub sway: (f32, f32),
    pub flash: f32,
    pub sprint: f32,
    /// Landing dip, strafe lean (-1..1), slide, airborne, climbing: 0..1.
    pub land: f32,
    pub strafe: f32,
    pub slide: f32,
    pub air: f32,
    pub mantle: f32,
    pub time: f32,
    /// Changes every shot; picks the muzzle flash shape.
    pub seq: u32,
}

/// First-person weapon and arms, built in camera space (camera looks down
/// -Z, +X is right, +Y is up).
pub fn viewmodel(m: &mut Mesh, add: &mut Mesh, v: &ViewModel) {
    use std::f32::consts::PI;
    let tc = team_col(v.team);
    let accent = rgb(tc[0], tc[1], tc[2]);
    let metal = rgb(0.2, 0.2, 0.23);
    let dark = rgb(0.11, 0.11, 0.13);
    // The self-loaders wear polymer where the older guns wear wood.
    let wood = if matches!(v.wid, 7 | 9 | 14) { rgb(0.17, 0.18, 0.17) } else { rgb(0.45, 0.3, 0.17) };
    let sleeve = rgb(tc[0] * 0.45, tc[1] * 0.45, tc[2] * 0.48);
    let cuff = rgb(tc[0] * 0.3, tc[1] * 0.3, tc[2] * 0.33);
    let glove = rgb(0.42, 0.34, 0.25);
    let hip = if v.kit == 4 { vec3(0.15, -0.13, -0.5) } else { vec3(0.19, -0.165, -0.7) };
    let iron = v.sight == IRON;
    let scope = v.sight >= SCOPE4;
    let aim = if iron || scope {
        vec3(0.0, [-0.115, -0.1, -0.095, 0.0, -0.085][v.kit as usize % 5], -0.5)
    } else {
        // bring the optic's window up to the eye, a little closer in
        let (ry, _, s) = rail(v.kit);
        vec3(0.0, -(ry + sight_center(v.sight) * s), if v.kit == 4 { -0.34 } else { -0.47 })
    };
    let ease = |t: f32| t * t * (3.0 - 2.0 * t);
    let seg = |t: f32, a: f32, b: f32| ease(((t - a) / (b - a)).clamp(0.0, 1.0));

    // Reload choreography: tilt in, magazine out, magazine in, settle.
    let r = v.reload;
    let reloading = r >= 0.0;
    let (tilt, mag_out, to_mag, seat) = if reloading {
        (
            seg(r, 0.0, 0.14) * (1.0 - seg(r, 0.84, 1.0)),
            seg(r, 0.14, 0.32) * (1.0 - seg(r, 0.5, 0.68)),
            seg(r, 0.04, 0.18) * (1.0 - seg(r, 0.72, 0.9)),
            seg(r, 0.62, 0.68) * (1.0 - seg(r, 0.68, 0.78)),
        )
    } else {
        (0.0, 0.0, 0.0, 0.0)
    };
    // Working the action between shots.
    let cyc = if reloading { 0.0 } else { (seg(1.0 - v.cycle, 0.22, 0.72) * PI).sin() };
    let pump = if matches!(v.wid, 1 | 13) { cyc } else { 0.0 };
    let bolt = if matches!(v.wid, 2 | 15) { cyc } else { 0.0 };
    let draw = ease(v.draw.clamp(0.0, 1.0));

    let k = 1.0 - v.ads * 0.85;
    let mut pos = hip + (aim - hip) * v.ads;
    pos += vec3((v.bob).sin() * 0.012, (v.bob * 2.0).sin().abs() * -0.012, 0.0) * v.bob_amt * (1.0 + 0.6 * v.sprint) * k;
    pos += vec3(v.sway.0, v.sway.1, 0.0) * k;
    pos += vec3((v.time * 0.9).sin() * 0.0018, (v.time * 1.7).sin() * 0.003, 0.0) * k;
    pos.z += v.recoil * 0.07 + pump * 0.012;
    pos.x -= tilt * 0.05;
    pos.y -= -tilt * 0.06 + v.sprint * 0.06 + draw * 0.34 + v.land * 0.05 * k + v.mantle * 0.3 - seat * 0.014 - v.air * 0.012 * k;
    pos.x += (v.slide * 0.03 - bolt * 0.012) * k;
    let rot = Mat4::from_rotation_x(v.recoil * 0.09 + tilt * 0.36 - v.sprint * 0.25 - draw * 0.9 - v.mantle * 0.8 - v.land * 0.05 * k + pump * 0.03)
        * Mat4::from_rotation_y(v.sprint * 0.5 + tilt * 0.22 + draw * 0.3 + bolt * 0.04)
        * Mat4::from_rotation_z(-tilt * 0.55 - v.strafe * 0.05 * k + v.slide * 0.28 * k - bolt * 0.16 * k);
    let g = Mat4::from_translation(pos) * rot;

    // Where the hands go, in gun space, and the magazine if there is one.
    let (grip, support, mut mag_c, mut mag_h) = match v.kit {
        1 => (vec3(0.0, -0.06, 0.1), vec3(0.0, -0.085, -0.32), Vec3::ZERO, Vec3::ZERO),
        2 => (vec3(0.0, -0.065, 0.13), vec3(0.0, -0.06, -0.3), vec3(0.0, -0.065, -0.06), vec3(0.016, 0.028, 0.04)),
        4 => (vec3(0.0, -0.085, 0.02), vec3(-0.03, -0.1, 0.005), vec3(0.0, -0.085, 0.022), vec3(0.013, 0.05, 0.02)),
        _ => (vec3(0.0, -0.085, 0.1), vec3(0.0, -0.05, -0.36), vec3(0.0, -0.1, -0.06), vec3(0.022, 0.07, 0.04)),
    };
    match v.wid {
        6 => (mag_c, mag_h) = (vec3(0.0, -0.095, -0.1), vec3(0.05, 0.05, 0.065)), // belt box
        8 => (mag_c, mag_h) = (vec3(0.0, -0.13, -0.06), vec3(0.014, 0.1, 0.024)), // stick magazine
        9 => mag_h = vec3(0.018, 0.05, 0.045),
        10 | 16 => mag_h = Vec3::ZERO, // revolver, break-action
        11 => (mag_c, mag_h) = (vec3(0.0, -0.11, 0.022), vec3(0.013, 0.08, 0.02)),
        _ => {}
    }
    let mag_off = vec3(0.0, -0.3 * mag_out, 0.04 * mag_out);
    let slide_back = vec3(0.0, 0.0, pump * 0.085);
    let muzzle_z;
    match v.kit {
        1 => {
            m.obox(&g, vec3(0.0, 0.0, -0.05), vec3(0.04, 0.05, 0.2), metal);
            for x in [-0.021f32, 0.021] {
                m.tube(&g, vec3(x, 0.012, -0.42), 0.021, 0.015, 0.2, 24, dark);
            }
            m.obox(&g, vec3(0.0, 0.012, -0.42), vec3(0.006, 0.012, 0.2), dark);
            m.obox(&g, vec3(0.0, 0.012, -0.6), vec3(0.045, 0.006, 0.012), metal);
            // pump: slides back toward you between shots
            m.obox(&g, vec3(0.0, -0.045, -0.32) + slide_back, vec3(0.035, 0.03, 0.1), wood);
            for z in [-0.38f32, -0.32, -0.26] {
                m.obox(&g, vec3(0.0, -0.045, z) + slide_back, vec3(0.037, 0.026, 0.006), shade(wood, 0.75));
            }
            m.tube(&g, vec3(0.0, -0.04, -0.38), 0.012, 0.0, 0.2, 12, metal);
            m.obox(&g, vec3(0.0, -0.03, 0.22), vec3(0.03, 0.05, 0.1), wood);
            m.obox(&g, vec3(0.0, -0.045, 0.34), vec3(0.03, 0.065, 0.03), shade(wood, 0.8));
            m.obox(&g, vec3(0.0, -0.065, 0.06), vec3(0.006, 0.02, 0.05), dark);
            m.obox(&g, vec3(0.0, 0.055, -0.1), vec3(0.042, 0.008, 0.06), accent);
            m.obox(&g, vec3(0.042, 0.01, -0.02), vec3(0.003, 0.018, 0.05), dark);
            if iron {
                m.obox(&g, vec3(0.0, 0.045, -0.6), vec3(0.005, 0.012, 0.005), accent);
                m.obox(&g, vec3(-0.012, 0.07, 0.1), vec3(0.004, 0.008, 0.005), dark);
                m.obox(&g, vec3(0.012, 0.07, 0.1), vec3(0.004, 0.008, 0.005), dark);
            }
            match v.wid {
                13 => {
                    // Slug gun: heat shield and a tall rifle front sight.
                    m.obox(&g, vec3(0.0, 0.04, -0.4), vec3(0.03, 0.005, 0.13), dark);
                    m.obox(&g, vec3(0.0, 0.06, -0.58), vec3(0.004, 0.016, 0.006), accent);
                }
                14 => {
                    // Drum magazine under the receiver.
                    m.tube(&g, vec3(0.0, -0.085, -0.06), 0.062, 0.0, 0.034, 16, dark);
                    m.tube(&g, vec3(0.0, -0.085, -0.06), 0.03, 0.0, 0.037, 12, metal);
                }
                _ => {}
            }
            muzzle_z = -0.64;
        }
        4 => {
            // Sidearm: slide (snaps back with each shot), frame, grip.
            let kick = vec3(0.0, 0.0, v.recoil * 0.035);
            m.obox(&g, vec3(0.0, 0.025, -0.12) + kick, vec3(0.022, 0.026, 0.13), metal);
            for z in [-0.02f32, -0.035, -0.05] {
                m.obox(&g, vec3(0.0, 0.025, z) + kick, vec3(0.0235, 0.022, 0.004), dark);
            }
            m.obox(&g, vec3(0.0, -0.012, -0.1), vec3(0.02, 0.014, 0.11), dark);
            m.obox(&(g * Mat4::from_rotation_x(-0.25)), vec3(0.0, -0.075, 0.0), vec3(0.02, 0.06, 0.03), dark);
            m.obox(&g, vec3(0.0, -0.035, -0.07), vec3(0.008, 0.012, 0.035), metal);
            m.obox(&g, vec3(0.0, -0.055, -0.07), vec3(0.006, 0.004, 0.04), metal);
            m.tube(&g, vec3(0.0, 0.03, -0.252), 0.012, 0.007, 0.006, 20, dark);
            if iron {
                m.obox(&g, vec3(0.0, 0.056, -0.23) + kick, vec3(0.005, 0.008, 0.006), dark);
                m.obox(&g, vec3(-0.013, 0.056, -0.01) + kick, vec3(0.004, 0.008, 0.006), dark);
                m.obox(&g, vec3(0.013, 0.056, -0.01) + kick, vec3(0.004, 0.008, 0.006), dark);
            }
            m.obox(&g, vec3(0.0, 0.025, 0.0) + kick, vec3(0.024, 0.02, 0.012), accent);
            muzzle_z = match v.wid {
                10 => {
                    // Revolver: cylinder, long barrel with a rib, wooden grip.
                    m.tube(&g, vec3(0.0, 0.012, -0.035), 0.034, 0.0, 0.04, 12, shade(metal, 1.25));
                    m.tube(&g, vec3(0.0, 0.03, -0.31), 0.014, 0.008, 0.07, 20, metal);
                    m.obox(&g, vec3(0.0, 0.05, -0.3), vec3(0.005, 0.006, 0.075), dark);
                    m.obox(&(g * Mat4::from_rotation_x(-0.25)), vec3(0.0, -0.08, 0.004), vec3(0.023, 0.062, 0.033), rgb(0.45, 0.3, 0.17));
                    -0.39
                }
                11 => {
                    // Machine pistol: compensator and a stubby fore grip.
                    m.obox(&g, vec3(0.0, 0.028, -0.275), vec3(0.025, 0.028, 0.022), dark);
                    m.obox(&g, vec3(0.0, -0.05, -0.17), vec3(0.014, 0.03, 0.016), dark);
                    -0.3
                }
                16 => {
                    // Sawn-off: two fat barrels side by side, wooden grip.
                    for x in [-0.016f32, 0.016] {
                        m.tube(&g, vec3(x, 0.03, -0.2), 0.016, 0.011, 0.12, 20, shade(metal, 1.2));
                    }
                    m.obox(&g, vec3(0.0, 0.03, -0.1), vec3(0.034, 0.02, 0.03), dark);
                    m.obox(&(g * Mat4::from_rotation_x(-0.25)), vec3(0.0, -0.08, 0.004), vec3(0.023, 0.062, 0.033), rgb(0.45, 0.3, 0.17));
                    -0.33
                }
                _ => -0.27,
            };
        }
        2 => {
            m.obox(&g, vec3(0.0, 0.0, -0.1), vec3(0.03, 0.045, 0.3), wood);
            m.obox(&g, vec3(0.0, 0.022, -0.02), vec3(0.024, 0.026, 0.15), metal);
            m.tube(&g, vec3(0.0, 0.015, -0.6), 0.014, 0.008, 0.28, 24, dark);
            m.tube(&g, vec3(0.0, 0.015, -0.86), 0.02, 0.008, 0.03, 24, metal);
            m.tube(&g, vec3(0.0, 0.015, -0.36), 0.019, 0.0, 0.04, 24, metal);
            // bolt: lifts and draws back between shots
            let bz = 0.07 + bolt * 0.075;
            m.tube(&g, vec3(0.0, 0.022, bz), 0.011, 0.0, 0.05, 12, shade(metal, 1.3));
            let handle = g * Mat4::from_translation(vec3(0.024, 0.022, bz + 0.03)) * Mat4::from_rotation_z(-0.9 + bolt * 0.9);
            m.obox(&handle, vec3(0.022, 0.0, 0.0), vec3(0.022, 0.005, 0.005), metal);
            m.obox(&handle, vec3(0.046, 0.0, 0.0), vec3(0.009, 0.009, 0.009), dark);
            m.obox(&g, vec3(0.0, -0.055, 0.03), vec3(0.006, 0.016, 0.04), dark);
            if iron {
                m.obox(&g, vec3(0.0, 0.05, -0.85), vec3(0.005, 0.02, 0.005), dark);
                m.obox(&g, vec3(-0.014, 0.058, 0.1), vec3(0.004, 0.013, 0.005), dark);
                m.obox(&g, vec3(0.014, 0.058, 0.1), vec3(0.004, 0.013, 0.005), dark);
            }
            m.obox(&g, vec3(0.0, -0.03, 0.25), vec3(0.028, 0.055, 0.1), wood);
            m.obox(&g, vec3(0.0, -0.035, 0.37), vec3(0.03, 0.065, 0.02), dark);
            muzzle_z = if v.wid == 15 {
                // Heavy rifle: thick barrel and a slab of muzzle brake.
                m.tube(&g, vec3(0.0, 0.015, -0.58), 0.02, 0.008, 0.27, 24, dark);
                m.obox(&g, vec3(0.0, 0.015, -0.92), vec3(0.04, 0.022, 0.04), metal);
                for z in [-0.935f32, -0.905] {
                    m.obox(&g, vec3(0.0, 0.015, z), vec3(0.042, 0.012, 0.006), dark);
                }
                -0.97
            } else {
                -0.9
            };
        }
        _ => {
            m.obox(&g, vec3(0.0, 0.0, -0.08), vec3(0.032, 0.05, 0.24), metal);
            if v.wid == 8 {
                // Submachine gun: a short barrel inside a fat suppressor.
                m.tube(&g, vec3(0.0, 0.012, -0.4), 0.027, 0.007, 0.085, 24, dark);
                m.tube(&g, vec3(0.0, 0.012, -0.325), 0.029, 0.0, 0.008, 24, metal);
            } else {
                m.tube(&g, vec3(0.0, 0.012, -0.44), 0.014, 0.007, 0.14, 24, dark);
                m.tube(&g, vec3(0.0, 0.012, -0.36), 0.024, 0.0, 0.06, 24, metal);
                for z in [-0.4f32, -0.36, -0.32] {
                    m.tube(&g, vec3(0.0, 0.012, z), 0.0255, 0.0, 0.006, 24, dark);
                }
                m.tube(&g, vec3(0.0, 0.012, -0.57), 0.019, 0.007, 0.025, 24, metal);
            }
            match v.wid {
                5 => {
                    // Battle rifle: wooden handguard and stock, longer barrel.
                    m.obox(&g, vec3(0.0, -0.004, -0.37), vec3(0.03, 0.034, 0.08), wood);
                    m.obox(&g, vec3(0.0, -0.02, 0.245), vec3(0.03, 0.047, 0.093), wood);
                    m.tube(&g, vec3(0.0, 0.012, -0.64), 0.012, 0.007, 0.05, 24, dark);
                }
                12 => {
                    // Burst rifle: team-coloured handguard and a long flash hider.
                    m.obox(&g, vec3(0.0, 0.034, -0.37), vec3(0.028, 0.01, 0.09), accent);
                    m.obox(&g, vec3(0.0, -0.012, -0.37), vec3(0.03, 0.02, 0.09), dark);
                    m.tube(&g, vec3(0.0, 0.012, -0.63), 0.016, 0.007, 0.04, 24, dark);
                }
                6 => {
                    // Machine gun: heavy barrel shroud, carry handle, folded bipod.
                    m.tube(&g, vec3(0.0, 0.012, -0.45), 0.022, 0.007, 0.1, 24, metal);
                    m.obox(&g, vec3(0.0, 0.075, -0.2), vec3(0.006, 0.006, 0.07), dark);
                    for x in [-0.022f32, 0.022] {
                        m.obox(&g, vec3(x, -0.022, -0.47), vec3(0.005, 0.005, 0.1), dark);
                    }
                }
                _ => {}
            }
            // grip, trigger guard, stock
            m.obox(&(g * Mat4::from_rotation_x(-0.3)), vec3(0.0, -0.09, 0.07), vec3(0.02, 0.05, 0.026), dark);
            m.obox(&g, vec3(0.0, -0.062, 0.04), vec3(0.005, 0.004, 0.045), dark);
            m.obox(&g, vec3(0.0, -0.02, 0.24), vec3(0.028, 0.045, 0.09), metal);
            m.obox(&g, vec3(0.0, -0.03, 0.34), vec3(0.03, 0.06, 0.02), dark);
            // ejection port and charging handle
            m.obox(&g, vec3(0.0325, 0.012, -0.03), vec3(0.002, 0.014, 0.04), dark);
            m.obox(&g, vec3(0.0, 0.052, 0.13 + v.recoil * 0.03), vec3(0.014, 0.006, 0.02), dark);
            m.obox(&g, vec3(0.0, 0.058, -0.02), vec3(0.034, 0.008, 0.1), accent);
            if iron {
                m.obox(&g, vec3(0.0, 0.075, -0.27), vec3(0.006, 0.012, 0.006), dark);
                m.obox(&g, vec3(-0.018, 0.075, 0.08), vec3(0.005, 0.012, 0.006), dark);
                m.obox(&g, vec3(0.018, 0.075, 0.08), vec3(0.005, 0.012, 0.006), dark);
            }
            muzzle_z = match v.wid {
                5 => -0.7,
                12 => -0.68,
                8 => -0.49,
                _ => -0.6,
            };
        }
    }
    if mag_h != Vec3::ZERO && mag_out < 0.97 {
        m.obox(&g, mag_c + mag_off, mag_h, dark);
        m.obox(&g, mag_c + mag_off - vec3(0.0, mag_h.y, 0.0), vec3(mag_h.x * 1.1, 0.006, mag_h.z * 1.05), metal);
    }
    if !iron {
        optic(m, &g, v.kit, v.sight, accent, dark, metal);
    }

    // ---- arms: elbows hang in camera space, hands stay on the weapon ----
    let follow = (pos - hip) * 0.55;
    let r_el = vec3(0.36, -0.56, -0.16) + follow;
    let l_el = vec3(-0.16, -0.56, -0.3) + follow;
    // Right hand holds the grip, or reaches up to work the bolt.
    let r_gun = grip + (vec3(0.07, 0.03, 0.1 + bolt * 0.07) - grip) * bolt.min(1.0);
    let r_hand = g.transform_point3(r_gun);
    // Left hand supports the fore-end, rides the pump, or handles the magazine.
    let l_gun = if v.kit == 1 && reloading {
        // feeding shells one at a time
        let push = (r * 4.0 * PI).sin().abs();
        let port = vec3(0.0, -0.075 - 0.07 * (1.0 - push), -0.06);
        support + (port - support) * to_mag
    } else {
        let at_mag = mag_c + mag_off - vec3(0.0, mag_h.y + 0.02, 0.0);
        support + slide_back + (at_mag - support) * to_mag
    };
    let l_hand = g.transform_point3(l_gun);
    let cam = Mat4::IDENTITY;
    for (el, hand, thumb_x) in [(r_el, r_hand, -1.0f32), (l_el, l_hand, 1.0)] {
        let d = (hand - el).normalize_or_zero();
        let wrist = hand - d * 0.075;
        limb(m, &cam, el, wrist, 0.043, true, sleeve);
        limb(m, &cam, wrist - d * 0.035, wrist, 0.047, true, cuff);
        limb(m, &cam, wrist, hand + d * 0.03, 0.036, false, glove);
        // thumb wrapped over the top
        let side = d.cross(Vec3::Y).normalize_or_zero();
        m.obox(&cam, hand + side * (0.034 * thumb_x) + vec3(0.0, 0.022, 0.0), vec3(0.012, 0.013, 0.03), shade(glove, 1.2));
    }
    if v.kit == 1 && reloading && to_mag > 0.6 {
        // the shell going in
        m.obox(&g, l_gun + vec3(0.0, 0.03, -0.01), vec3(0.011, 0.011, 0.028), rgb(0.75, 0.2, 0.15));
    }

    if v.flash > 0.0 {
        let c = g.transform_point3(vec3(0.0, 0.015, muzzle_z));
        let s = (0.012 + 0.02 * v.flash) * if v.kit == 1 { 1.5 } else { 1.0 };
        let col = [1.0, 0.8, 0.4, v.flash];
        let a0 = hash01(v.seq, 5) * PI;
        add.quad(c + vec3(-s, -s, 0.0) * 0.7, c + vec3(s, -s, 0.0) * 0.7, c + vec3(s, s, 0.0) * 0.7, c + vec3(-s, s, 0.0) * 0.7, [1.0, 0.95, 0.75, v.flash]);
        for j in 0..3u32 {
            let a = a0 + j as f32 * PI / 3.0;
            let (u, w2) = (vec3(a.cos(), a.sin(), 0.0), vec3(-a.sin(), a.cos(), 0.0));
            let len = s * (1.7 + 0.8 * hash01(v.seq, 9 + j));
            add.quad(c - u * len, c - w2 * s * 0.35, c + u * len, c + w2 * s * 0.35, col);
        }
    }
}
