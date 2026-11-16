//! Weapon finishes. Paint follows the flat faces of the machined parts,
//! including moving magazines and actions. Patterns are thin polygons laid
//! a fraction of a millimetre above each face, so no textures or extra
//! draw calls are needed. Materials read through colour alone: polished
//! metal is lighter towards the top of a part and darker underneath, its
//! chamfers can carry a polished or worn edge colour, and a glowing paint
//! uses the vertex alpha (emissive) channel.

use crate::gfx::{rgb, shade, Col, Mesh};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::sim::{WeaponPattern, WeaponSkinDef};
use std::f32::consts::TAU;

/// What a part is, which decides how much of a finish it wears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Part {
    /// Receiver, slide and other machined metal: the whole finish.
    Body,
    /// Stock, handguard and fore-end in the finish's furniture colour.
    Furniture,
    /// Grips, magazines and other dark parts.
    Detail,
}

/// The colour a pattern polygon is painted in.
#[derive(Clone, Copy, PartialEq)]
enum Paint {
    Ink,
    Alt,
    /// The part's own colour, scaled.
    Tint(f32),
}

/// Height of one paint layer above the face. Overlapping polygons of
/// different colours go on different layers so they never z-fight.
const LAYER: f32 = 0.00018;

/// Whether a pattern is painted on this kind of part. Camouflage covers
/// everything; metal treatments stay on the metal.
fn covers(kind: WeaponPattern, part: Part) -> bool {
    use WeaponPattern::*;
    match part {
        Part::Body => kind != Plain,
        Part::Furniture => !matches!(kind, Plain | Damascus),
        Part::Detail => matches!(kind, Digital | Blotch | Splinter | Tiger | Weave),
    }
}

/// Patterns where the face itself is painted in the glowing colour and
/// dark plates are laid over it, leaving glowing seams.
fn plated(kind: WeaponPattern) -> bool {
    matches!(kind, WeaponPattern::Magma | WeaponPattern::Hex)
}

/// A bevelled box: the shape of every machined part, painted in a finish.
#[allow(clippy::too_many_arguments)]
pub(super) fn receiver(m: &mut Mesh, g: &Mat4, center: Vec3, half: Vec3, col: Col, part: Part, finish: &WeaponSkinDef, detailed: bool) {
    let bevel = half.x.min(half.y) * 0.3;
    let ring = [
        (-half.x + bevel, -half.y),
        (half.x - bevel, -half.y),
        (half.x, -half.y + bevel),
        (half.x, half.y - bevel),
        (half.x - bevel, half.y),
        (-half.x + bevel, half.y),
        (-half.x, half.y - bevel),
        (-half.x, -half.y + bevel),
    ];
    let sheen = finish.sheen
        * match part {
            Part::Body => 1.0,
            Part::Furniture => 0.5,
            Part::Detail => 0.0,
        };
    // Polished metal: lighter towards the top of the part, darker under it.
    let lit = |c: Col, y: f32| -> Col {
        if sheen <= 0.0 {
            return c;
        }
        let t = ((y + half.y) / (2.0 * half.y)).clamp(0.0, 1.0);
        let k = 1.0 - 0.3 * sheen + 0.5 * sheen * t;
        let gleam = 0.07 * sheen * t * t * t;
        [c[0] * k + gleam, c[1] * k + gleam, c[2] * k + gleam, c[3]]
    };
    let pattern_on = detailed && finish.pattern_color.is_some() && covers(finish.pattern, part) && half.z >= 0.04 && half.y >= 0.02;
    let glow = |c: [f32; 3], a: f32| [c[0], c[1], c[2], a];
    let ink = finish.pattern_color.map_or(col, |c| glow(c, finish.glow));
    let alt = finish.pattern_alt.map_or(shade(ink, 0.6), |c| glow(c, finish.glow * 0.5));
    let plates = pattern_on && plated(finish.pattern);
    let edge = finish.edge.filter(|_| part == Part::Body).map(|c| rgb(c[0], c[1], c[2]));
    // The ends of each part sit a little darker, like recesses.
    let cap = if finish.body.is_some() { shade(col, 0.82) } else { col };
    let point = |i: usize, z: f32| g.transform_point3(center + vec3(ring[i].0, ring[i].1, z));
    let front = g.transform_point3(center - Vec3::Z * half.z);
    let back = g.transform_point3(center + Vec3::Z * half.z);
    for i in 0..8 {
        let j = (i + 1) % 8;
        let face = match i {
            // Upper chamfers catch the light; lower ones are half worn.
            3 | 5 => edge.unwrap_or(shade(col, 1.0 + 0.3 * sheen)),
            1 | 7 => edge.map_or(col, |e| mix(e, col, 0.55)),
            2 | 4 | 6 if plates => ink,
            _ => col,
        };
        let (ci, cj) = (lit(face, ring[i].1), lit(face, ring[j].1));
        m.quad_c(point(i, -half.z), point(j, -half.z), point(j, half.z), point(i, half.z), [ci, cj, cj, ci]);
        let (ki, kj, k0) = (lit(cap, ring[i].1), lit(cap, ring[j].1), lit(cap, 0.0));
        m.tri_c(front, point(j, -half.z), point(i, -half.z), [k0, kj, ki]);
        m.tri_c(back, point(i, half.z), point(j, half.z), [k0, ki, kj]);
    }
    if !pattern_on {
        return;
    }
    // Plates run to the very edge so the seams glow evenly; other paint
    // keeps clear of the chamfers.
    let inset = if plates { 0.0 } else { 0.001 };
    let salt = (center.z * 1000.0).round() as i32 as u32 ^ (half.z * 1000.0) as u32 ^ (half.y * 7919.0) as u32;
    let (side_h, top_h) = ((half.y - bevel) * 2.0 - inset * 2.0, (half.x - bevel) * 2.0 - inset * 2.0);
    let faces = [
        (vec3(half.x, -half.y + bevel + inset, 0.0), Vec3::Y, Vec3::X, side_h),
        (vec3(-half.x, -half.y + bevel + inset, 0.0), Vec3::Y, -Vec3::X, side_h),
        (vec3(-half.x + bevel + inset, half.y, 0.0), Vec3::X, Vec3::Y, top_h),
    ];
    let w = half.z * 2.0 - inset * 4.0;
    for (n, &(corner, up, normal, h)) in faces.iter().enumerate() {
        let origin = corner - Vec3::Z * (half.z - inset * 2.0);
        // Triangles laid along +Z then `up` face this way; flip if needed.
        let reverse = Vec3::Z.cross(up).dot(normal) < 0.0;
        let mut emit = |poly: &[(f32, f32)], paint: Paint, layer: u8| {
            let mut clipped = [(0.0, 0.0); 24];
            let count = clip(poly, w, h, &mut clipped);
            let poly = &clipped[..count];
            if count < 3 {
                return;
            }
            let area = signed_area(poly);
            if area.abs() < 1e-9 {
                return;
            }
            let base = match paint {
                Paint::Ink => ink,
                Paint::Alt => alt,
                Paint::Tint(k) => shade(col, k),
            };
            let lift = normal * (LAYER * (1 + layer) as f32);
            let at = |p: (f32, f32)| {
                let local = origin + Vec3::Z * p.0 + up * p.1 + lift;
                (g.transform_point3(center + local), lit(base, local.y))
            };
            let flip = (area < 0.0) != reverse;
            for k in 1..count - 1 {
                let (a, b, c) = (poly[0], poly[k], poly[k + 1]);
                // Skip slivers left by clipping and any folded corner.
                if signed_area(&[a, b, c]) * area.signum() <= 1e-10 {
                    continue;
                }
                let (a, b, c) = (at(a), at(b), at(c));
                if flip {
                    m.tri_c(a.0, c.0, b.0, [a.1, c.1, b.1]);
                } else {
                    m.tri_c(a.0, b.0, c.0, [a.1, b.1, c.1]);
                }
            }
        };
        paint(finish.pattern, part, w, h, salt ^ (n as u32).wrapping_mul(0x9e37_79b9), &mut emit);
    }
}

fn mix(a: Col, b: Col, t: f32) -> Col {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t]
}

fn hash(x: u32, y: u32, salt: u32) -> u32 {
    let mut n = x.wrapping_mul(0x85eb_ca6b) ^ y.wrapping_mul(0xc2b2_ae35) ^ salt;
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb_352d);
    n ^ (n >> 15)
}

/// A stable random number in 0..1 for a lattice cell.
fn rnd(x: i32, y: i32, salt: u32) -> f32 {
    hash(x as u32, y as u32, salt) as f32 / 4_294_967_296.0
}

/// Smooth value noise in 0..1, for blotchy pixel fields.
fn noise(x: f32, y: f32, salt: u32) -> f32 {
    let (i, j) = (x.floor() as i32, y.floor() as i32);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (fx, fy) = (s(x - x.floor()), s(y - y.floor()));
    let row = |j: i32| rnd(i, j, salt) + (rnd(i + 1, j, salt) - rnd(i, j, salt)) * fx;
    row(j) + (row(j + 1) - row(j)) * fy
}

fn signed_area(p: &[(f32, f32)]) -> f32 {
    let mut a = 0.0;
    for i in 0..p.len() {
        let (u, v) = (p[i], p[(i + 1) % p.len()]);
        a += u.0 * v.1 - v.0 * u.1;
    }
    a * 0.5
}

/// Clip a convex polygon to the face, 0..w along and 0..h up.
fn clip(poly: &[(f32, f32)], w: f32, h: f32, out: &mut [(f32, f32); 24]) -> usize {
    let mut cur = [(0.0, 0.0); 24];
    let mut n = poly.len().min(16);
    cur[..n].copy_from_slice(&poly[..n]);
    for edge in 0..4 {
        let value = |p: (f32, f32)| match edge {
            0 => p.0,
            1 => w - p.0,
            2 => p.1,
            _ => h - p.1,
        };
        let mut k = 0;
        for i in 0..n {
            let (p, q) = (cur[i], cur[(i + 1) % n]);
            let (vp, vq) = (value(p), value(q));
            if vp >= 0.0 && k < 24 {
                out[k] = p;
                k += 1;
            }
            if (vp >= 0.0) != (vq >= 0.0) && k < 24 {
                let t = vp / (vp - vq);
                out[k] = (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t);
                k += 1;
            }
        }
        n = k;
        if n < 3 {
            return 0;
        }
        cur[..n].copy_from_slice(&out[..n]);
    }
    n
}

type Emit<'a> = dyn FnMut(&[(f32, f32)], Paint, u8) + 'a;

/// A rounded, slightly irregular seven-sided blot.
#[allow(clippy::too_many_arguments)]
fn blot(emit: &mut Emit, c: (f32, f32), rx: f32, ry: f32, tilt: f32, seed: (i32, i32, u32), paint: Paint, layer: u8) {
    let (sin, cos) = tilt.sin_cos();
    let mut p = [(0.0, 0.0); 7];
    for (k, q) in p.iter_mut().enumerate() {
        let a = k as f32 * TAU / 7.0;
        // Radii within 0.85..1 keep the outline convex.
        let r = 0.85 + 0.15 * rnd(seed.0 * 7 + k as i32, seed.1, seed.2);
        let (dx, dy) = (a.cos() * rx * r, a.sin() * ry * r);
        *q = (c.0 + dx * cos - dy * sin, c.1 + dx * sin + dy * cos);
    }
    emit(&p, paint, layer);
}

/// A line of even thickness through `pts`, one quad per segment.
fn strip(emit: &mut Emit, pts: &[(f32, f32)], thick: f32, paint: Paint, layer: u8) {
    let side = |i: usize| {
        let (a, b) = (pts[i.saturating_sub(1)], pts[(i + 1).min(pts.len() - 1)]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        (-dy / len * thick * 0.5, dx / len * thick * 0.5)
    };
    for i in 0..pts.len() - 1 {
        let (a, b, na, nb) = (pts[i], pts[i + 1], side(i), side(i + 1));
        emit(&[(a.0 - na.0, a.1 - na.1), (b.0 - nb.0, b.1 - nb.1), (b.0 + nb.0, b.1 + nb.1), (a.0 + na.0, a.1 + na.1)], paint, layer);
    }
}

/// An engraved line just inside the edge of the face.
fn border(emit: &mut Emit, w: f32, h: f32, inset: f32, t: f32, paint: Paint) {
    if w < (inset + t) * 2.0 || h < (inset + t) * 2.0 {
        return;
    }
    let (x0, x1, y0, y1) = (inset, w - inset, inset, h - inset);
    let rect = |a: f32, b: f32, c: f32, d: f32| [(a, b), (c, b), (c, d), (a, d)];
    emit(&rect(x0, y0, x1, y0 + t), paint, 0);
    emit(&rect(x0, y1 - t, x1, y1), paint, 0);
    emit(&rect(x0, y0 + t, x0 + t, y1 - t), paint, 0);
    emit(&rect(x1 - t, y0 + t, x1, y1 - t), paint, 0);
}

/// A jittered grid split into triangles: `pick` paints each one.
fn facets(emit: &mut Emit, w: f32, h: f32, cell: (f32, f32), salt: u32, mut pick: impl FnMut(&mut Emit, [(f32, f32); 3], f32, f32)) {
    let pt = |i: i32, j: i32| {
        ((i as f32 + (rnd(i, j, salt ^ 11) - 0.5) * 0.6) * cell.0, (j as f32 + (rnd(i, j, salt ^ 12) - 0.5) * 0.6) * cell.1)
    };
    for j in -1..=(h / cell.1) as i32 {
        for i in -1..=(w / cell.0) as i32 {
            let (a, b, c, d) = (pt(i, j), pt(i + 1, j), pt(i + 1, j + 1), pt(i, j + 1));
            let tris = if rnd(i, j, salt ^ 13) < 0.5 { [[a, b, c], [a, c, d]] } else { [[a, b, d], [b, c, d]] };
            for (t, tri) in tris.into_iter().enumerate() {
                pick(emit, tri, rnd(i * 2 + t as i32, j, salt ^ 14), rnd(i * 2 + t as i32, j, salt ^ 15));
            }
        }
    }
}

/// Lay one finish's pattern over a face `w` long (muzzle at 0) and `h` high.
fn paint(kind: WeaponPattern, part: Part, w: f32, h: f32, salt: u32, emit: &mut Emit) {
    match kind {
        WeaponPattern::Plain => {}
        WeaponPattern::Digital => {
            // Square pixels, merged into runs along each row; the noise
            // makes them cluster into blotches rather than static.
            let c = 0.0105;
            let (nx, ny) = ((w / c).ceil() as i32, (h / c).ceil() as i32);
            let pick = |i: i32, j: i32| {
                let v = noise(i as f32 / 3.2, j as f32 / 2.6, salt) + (rnd(i, j, salt ^ 0x51) - 0.5) * 0.32;
                if v > 0.6 {
                    1
                } else if v < 0.37 {
                    2
                } else {
                    0
                }
            };
            for j in 0..ny {
                let mut i = 0;
                while i < nx {
                    let p = pick(i, j);
                    let mut k = i + 1;
                    while k < nx && pick(k, j) == p {
                        k += 1;
                    }
                    if p != 0 {
                        let (x0, x1, y0, y1) = (i as f32 * c, k as f32 * c, j as f32 * c, (j + 1) as f32 * c);
                        emit(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], if p == 1 { Paint::Ink } else { Paint::Alt }, 0);
                    }
                    i = k;
                }
            }
        }
        WeaponPattern::Patina => {
            // Verdigris runs down from the top edge.
            let (mut x, mut k) = (0.002, 0);
            while x < w {
                if rnd(k, 0, salt) < 0.55 {
                    let dw = 0.006 + 0.007 * rnd(k, 1, salt);
                    let len = h * (0.25 + 0.6 * rnd(k, 2, salt));
                    let tip = h - len;
                    let poly = [(x, h + 0.001), (x + dw, h + 0.001), (x + dw, tip + dw * 0.7), (x + dw * 0.5, tip), (x, tip + dw * 0.7)];
                    emit(&poly, Paint::Ink, 0);
                }
                x += 0.02 + 0.016 * rnd(k, 3, salt);
                k += 1;
            }
            // Crust gathers along the bottom edge.
            let (mut x, mut k) = (0.0, 0);
            while x < w + 0.02 {
                let rx = 0.01 + 0.012 * rnd(k, 4, salt);
                let ry = (0.006 + 0.008 * rnd(k, 5, salt)).min(h * 0.4);
                blot(emit, (x, h * 0.04), rx, ry, 0.0, (k, 6, salt), Paint::Ink, 0);
                x += 0.022 + 0.022 * rnd(k, 7, salt);
                k += 1;
            }
            // Dark pits of oxide.
            let pits = ((w * h * 2000.0) as i32).min(10);
            for k in 0..pits {
                let (x, y) = (rnd(k, 8, salt) * w, rnd(k, 9, salt) * h);
                let s = 0.002 + 0.002 * rnd(k, 10, salt);
                emit(&[(x - s, y), (x, y - s), (x + s, y), (x, y + s)], Paint::Alt, 1);
            }
        }
        WeaponPattern::Blotch => {
            // Light leaf colour lowest, brown over it, black branches on top.
            let (sx, sy) = (0.046, 0.032);
            for j in -1..=(h / sy) as i32 + 1 {
                for i in -1..=(w / sx) as i32 + 1 {
                    let r = rnd(i, j, salt);
                    let (paint, layer, scale) = if r < 0.2 {
                        (Paint::Tint(1.55), 0, 0.9)
                    } else if r < 0.6 {
                        (Paint::Ink, 1, 1.0)
                    } else if r < 0.84 {
                        (Paint::Alt, 2, 0.6)
                    } else {
                        continue;
                    };
                    let cx = (i as f32 + 0.5 * (j & 1) as f32 + (rnd(i, j, salt ^ 1) - 0.5) * 0.6) * sx;
                    let cy = (j as f32 + (rnd(i, j, salt ^ 2) - 0.5) * 0.6) * sy;
                    let rx = sx * (0.5 + 0.35 * rnd(i, j, salt ^ 3)) * scale;
                    let ry = sy * (0.5 + 0.35 * rnd(i, j, salt ^ 4)) * scale * if layer == 2 { 0.6 } else { 1.0 };
                    let tilt = (rnd(i, j, salt ^ 5) - 0.5) * 1.2;
                    blot(emit, (cx, cy), rx, ry, tilt, (i, j, salt ^ 6), paint, layer);
                }
            }
        }
        WeaponPattern::Splinter => {
            // Slanted shards on a sheared grid, then short rain marks.
            let (sx, sy, skew) = (0.052, 0.03, 0.9);
            let pt = |i: i32, j: i32| {
                (
                    i as f32 * sx + j as f32 * sy * skew + (rnd(i, j, salt ^ 21) - 0.5) * sx * 0.5,
                    j as f32 * sy + (rnd(i, j, salt ^ 22) - 0.5) * sy * 0.5,
                )
            };
            let nj = (h / sy).ceil() as i32 + 1;
            let i0 = -((nj as f32 * sy * skew / sx).ceil() as i32) - 1;
            for j in -1..nj {
                for i in i0..=(w / sx) as i32 + 1 {
                    let (a, b, c, d) = (pt(i, j), pt(i + 1, j), pt(i + 1, j + 1), pt(i, j + 1));
                    for (t, tri) in [[a, b, c], [a, c, d]].iter().enumerate() {
                        let r = rnd(i * 2 + t as i32, j, salt ^ 23);
                        if r < 0.3 {
                            emit(tri, Paint::Ink, 0);
                        } else if r < 0.56 {
                            emit(tri, Paint::Alt, 0);
                        }
                    }
                }
            }
            let (mut x, mut k) = (0.003, 0);
            while x < w {
                if rnd(k, 7, salt) < 0.5 {
                    let y = rnd(k, 8, salt) * h;
                    let len = 0.006 + 0.006 * rnd(k, 9, salt);
                    emit(&[(x, y), (x + 0.0014, y), (x + 0.0026, y + len), (x + 0.0012, y + len)], Paint::Ink, 1);
                }
                x += 0.012;
                k += 1;
            }
        }
        WeaponPattern::Chevron => {
            // Chevrons at 45 degrees, pointing at the muzzle: black on the
            // painted metal, yellow on black furniture.
            let paint = if part == Part::Body { Paint::Ink } else { Paint::Alt };
            let (s, t, mid) = (0.046, 0.019, h * 0.5);
            for k in -2..((w + h) / s) as i32 + 2 {
                let x0 = k as f32 * s - mid;
                emit(&[(x0, mid), (x0 + t, mid), (x0 + t + mid, h), (x0 + mid, h)], paint, 0);
                emit(&[(x0, mid), (x0 + mid, 0.0), (x0 + t + mid, 0.0), (x0 + t, mid)], paint, 0);
            }
        }
        WeaponPattern::Tiger => {
            // Tapered stripes reaching in from alternate edges, leaning back.
            let s = 0.034;
            for k in -2..(w / s) as i32 + 2 {
                let from_top = (k & 1 == 0) != (rnd(k, 0, salt) < 0.2);
                let x0 = (k as f32 + (rnd(k, 1, salt) - 0.5) * 0.5) * s;
                let len = h * (0.5 + 0.45 * rnd(k, 2, salt));
                let bw = 0.012 + 0.01 * rnd(k, 3, salt);
                let lean = (0.3 + 0.35 * rnd(k, 4, salt)) * len;
                stripe(emit, x0, bw, len, lean, from_top, h, Paint::Ink, 0);
                // A thinner, shorter stripe between the main ones.
                if rnd(k, 5, salt) < 0.7 {
                    stripe(emit, x0 + s * 0.5, bw * 0.55, len * 0.5, lean * 0.5, !from_top, h, Paint::Alt, 1);
                }
            }
        }
        WeaponPattern::Magma => {
            // Plates of cooled rock; the glowing face shows in the cracks.
            facets(emit, w, h, (0.044, 0.034), salt, |emit, tri, r, v| {
                let c = ((tri[0].0 + tri[1].0 + tri[2].0) / 3.0, (tri[0].1 + tri[1].1 + tri[2].1) / 3.0);
                let shrunk = tri.map(|p| (c.0 + (p.0 - c.0) * 0.92, c.1 + (p.1 - c.1) * 0.92));
                let paint = if r < 0.06 { Paint::Alt } else { Paint::Tint(0.7 + 1.1 * v) };
                emit(&shrunk, paint, 0);
            });
        }
        WeaponPattern::Damascus => {
            // Wavy layers of light and dark steel folded into the blade.
            let (sp, seg) = (0.015, 0.025);
            let phase = rnd(0, 0, salt) * TAU;
            let segs = (w / seg).ceil() as usize + 1;
            for k in -1..=(h / sp) as i32 + 1 {
                let kf = k as f32;
                let y = |x: f32| (kf + 0.5) * sp + 0.0042 * (x * 62.0 + phase + kf * 0.5).sin();
                let th = |x: f32| 0.0032 + 0.0012 * (x * 31.0 + kf * 1.3).sin();
                let paint = if k & 1 == 0 { Paint::Ink } else { Paint::Alt };
                for s in 0..segs {
                    let (x0, x1) = (s as f32 * seg, (s + 1) as f32 * seg);
                    let (y0, y1, t0, t1) = (y(x0), y(x1), th(x0) * 0.5, th(x1) * 0.5);
                    emit(&[(x0, y0 - t0), (x1, y1 - t1), (x1, y1 + t1), (x0, y0 + t0)], paint, (k & 1) as u8);
                }
            }
        }
        WeaponPattern::Weave => {
            // Two-by-two twill: each lit tow is split into two facets so
            // the weave catches the light.
            let c = 0.014;
            let (nx, ny) = ((w / c).ceil() as i32, (h / c).ceil() as i32);
            for j in 0..ny {
                let mut i = -(j % 4);
                while i < nx {
                    let (x0, x1, y0, y1) = (i as f32 * c, (i + 2) as f32 * c, j as f32 * c, (j + 1) as f32 * c);
                    emit(&[(x0, y0), (x1, y0), (x0, y1)], Paint::Ink, 0);
                    emit(&[(x1, y0), (x1, y1), (x0, y1)], Paint::Alt, 0);
                    i += 4;
                }
            }
        }
        WeaponPattern::Hex => {
            // Honeycomb plates; the glowing face shows between them, and a
            // few cells are left open.
            let r = 0.019;
            let (dx, dy) = (1.5 * r, 3f32.sqrt() * r);
            for i in -1..=(w / dx) as i32 + 1 {
                for j in -1..=(h / dy) as i32 + 1 {
                    let c = (i as f32 * dx, j as f32 * dy + if i & 1 == 1 { dy * 0.5 } else { 0.0 });
                    let q = rnd(i, j, salt);
                    let paint = if q < 0.04 {
                        continue;
                    } else if q < 0.17 {
                        Paint::Alt
                    } else {
                        Paint::Tint(0.85 + 0.4 * rnd(i, j, salt ^ 1))
                    };
                    let hex: [(f32, f32); 6] = std::array::from_fn(|k| {
                        let a = k as f32 * TAU / 6.0;
                        (c.0 + a.cos() * r * 0.935, c.1 + a.sin() * r * 0.935)
                    });
                    emit(&hex, paint, 0);
                }
            }
        }
        WeaponPattern::Inlay => {
            border(emit, w, h, 0.003, 0.0016, Paint::Ink);
            if part != Part::Body {
                return;
            }
            // Brushed streaks along the metal.
            for n in 0..3 {
                let y = h * (0.22 + 0.28 * n as f32) + (rnd(n, 0, salt) - 0.5) * 0.004;
                let (x0, x1) = (w * 0.3 * rnd(n, 1, salt), w * (0.7 + 0.3 * rnd(n, 2, salt)));
                emit(&[(x0, y), (x1, y), (x1, y + 0.0008), (x0, y + 0.0008)], Paint::Tint(1.22), 1);
            }
            // A pearl panel with a diamond at each end.
            if w > 0.08 && h > 0.035 {
                let (x0, x1, y0, y1) = (w * 0.3, w * 0.7, h * 0.3, h * 0.7);
                let ch = (y1 - y0) * 0.32;
                let octagon = |g: f32| {
                    let (x0, x1, y0, y1, ch) = (x0 - g, x1 + g, y0 - g, y1 + g, ch + g * 0.4);
                    [(x0 + ch, y0), (x1 - ch, y0), (x1, y0 + ch), (x1, y1 - ch), (x1 - ch, y1), (x0 + ch, y1), (x0, y1 - ch), (x0, y0 + ch)]
                };
                // Set in an engraved rim.
                emit(&octagon(0.0016), Paint::Ink, 2);
                emit(&octagon(0.0), Paint::Alt, 3);
                for x in [x0 - 0.008, x1 + 0.008] {
                    let (y, s) = (h * 0.5, 0.0035);
                    emit(&[(x - s, y), (x, y - s), (x + s, y), (x, y + s)], Paint::Ink, 2);
                }
            }
        }
        WeaponPattern::Shards => {
            // Ice broken into facets of white, pale and deep blue.
            facets(emit, w, h, (0.034, 0.026), salt, |emit, tri, r, _| {
                let paint = if r < 0.2 {
                    Paint::Ink
                } else if r < 0.38 {
                    Paint::Alt
                } else if r < 0.6 {
                    Paint::Tint(1.14)
                } else if r < 0.76 {
                    Paint::Tint(0.84)
                } else {
                    return;
                };
                emit(&tri, paint, 0);
            });
        }
        WeaponPattern::Scroll => {
            border(emit, w, h, 0.0028, 0.0016, Paint::Ink);
            if part == Part::Detail || h < 0.02 {
                return;
            }
            // A vine waving along the face, a curl in each bend.
            let lam = 0.1;
            let amp = (h * 0.5 - 0.009).clamp(0.0, 0.011);
            let stem = |x: f32| h * 0.5 + amp * (x * TAU / lam).sin();
            let (x0, x1) = (0.008, w - 0.008);
            let n = ((x1 - x0) / (lam / 6.0)).ceil().max(1.0) as usize;
            let pts: Vec<(f32, f32)> = (0..=n).map(|k| x0 + (x1 - x0) * k as f32 / n as f32).map(|x| (x, stem(x))).collect();
            strip(emit, &pts, 0.0024, Paint::Ink, 1);
            // Curls only on the metal, to keep big stocks inexpensive.
            if amp < 0.004 || part != Part::Body {
                return;
            }
            let r0 = (amp * 0.85).min(0.01);
            let mut k = 0;
            loop {
                let x = lam * 0.25 + k as f32 * lam * 0.5;
                if x > x1 - r0 {
                    break;
                }
                // Hang each curl inside the bend, winding forwards.
                let up = if k & 1 == 0 { 1.0 } else { -1.0 };
                let c = (x + r0 * 0.3, stem(x) - up * r0);
                let spiral: Vec<(f32, f32)> = (0..6)
                    .map(|s| {
                        let t = s as f32 / 5.0;
                        let a = TAU * 0.25 * up - up * t * TAU * 1.15;
                        let r = r0 * (1.0 - 0.62 * t);
                        (c.0 + a.cos() * r, c.1 + a.sin() * r)
                    })
                    .collect();
                strip(emit, &spiral, 0.002, Paint::Alt, 2);
                k += 1;
            }
        }
    }
}

/// One tiger stripe: a tapered blade from an edge, slightly bowed.
#[allow(clippy::too_many_arguments)]
fn stripe(emit: &mut Emit, x0: f32, bw: f32, len: f32, lean: f32, from_top: bool, h: f32, paint: Paint, layer: u8) {
    let (y0, dir) = if from_top { (h + 0.001, -1.0) } else { (-0.001, 1.0) };
    let tip = (x0 + bw * 0.5 + lean, y0 + dir * len);
    let bulge = bw * 0.25;
    let right = ((x0 + bw + tip.0) * 0.5 + bulge, y0 + dir * len * 0.5);
    let left = ((x0 + tip.0) * 0.5 - bulge, y0 + dir * len * 0.5);
    emit(&[(x0, y0), (x0 + bw, y0), right, tip, left], paint, layer);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_shared::sim::WEAPON_SKINS;

    #[test]
    fn paint_stays_on_the_part_with_outward_facing_normals() {
        let half = vec3(0.04, 0.05, 0.2);
        let col = rgb(0.3, 0.3, 0.3);
        for part in [Part::Body, Part::Furniture, Part::Detail] {
            let mut factory = Mesh::default();
            receiver(&mut factory, &Mat4::IDENTITY, Vec3::ZERO, half, col, part, &WEAPON_SKINS[0], true);
            for finish in &WEAPON_SKINS[1..] {
                let mut mesh = Mesh::default();
                receiver(&mut mesh, &Mat4::IDENTITY, Vec3::ZERO, half, col, part, finish, true);
                if part == Part::Body {
                    assert!(mesh.v.len() > factory.v.len(), "{} has no pattern", finish.name);
                }
                for v in &mesh.v[factory.v.len()..] {
                    let p = Vec3::from(v.pos);
                    let n = Vec3::from(v.nrm);
                    assert!(p.is_finite() && n.is_finite());
                    assert!(p.abs().cmple(half + Vec3::splat(0.001)).all(), "{} paint beyond part: {p:?}", finish.name);
                    assert!(n.dot(p) > 0.03, "{} has inward paint at {p:?}", finish.name);
                    assert!(n.length() > 0.99);
                    assert!(v.col.iter().all(|c| c.is_finite() && *c >= 0.0), "{} has an invalid colour", finish.name);
                }
                // Without detail (a distant soldier) only the colours remain.
                let mut plain = Mesh::default();
                receiver(&mut plain, &Mat4::IDENTITY, Vec3::ZERO, half, col, part, finish, false);
                assert_eq!(plain.v.len(), factory.v.len(), "{} draws a pattern when it should not", finish.name);
            }
        }
    }
}
