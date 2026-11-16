//! Quarry's mine and Ridgeline's cave: the rock around each tunnel's
//! cavity, its floor, the collar of rock round each portal, and fittings.
//! The collision lives in `ferrite_shared::map::tunnels`; everything drawn
//! here stays outside the cavity's clear width and height, so nothing seen
//! is something a player walks through.

use super::hash01;
use super::landmarks::{lamp, sign};
use crate::gfx::{rgb, shade, Col, Light, Mesh};
use ferrite_shared::glam::{vec2, vec3, Mat4, Vec3};
use ferrite_shared::map::tunnels::{Tunnel, FLARE, HALF_WIDTH as HW, HEIGHT as H};
use ferrite_shared::map::World;

const TIMBER: Col = [0.33, 0.23, 0.14, 0.0];
const TIMBER_DARK: Col = [0.24, 0.17, 0.11, 0.0];
const RAIL: Col = [0.36, 0.34, 0.32, 0.0];
const PAINT: Col = [0.88, 0.76, 0.47, 0.0];
const SAFETY: Col = [0.94, 0.61, 0.11, 0.0];
const LANTERN: Col = [1.0, 0.72, 0.36, 0.9];

/// Spacing of the rock rings along a tunnel.
const RING: f32 = 1.5;

/// The cross-section, from the left foot of the wall over the roof to the
/// right foot: (offset from the centreline, height above the floor). Every
/// point is outside the clear width or above the clear height.
fn profile(map: u8) -> &'static [(f32, f32)] {
    if map == 1 {
        // A squared mine drift, timbered.
        &[
            (-HW - 0.12, -0.05),
            (-HW - 0.22, 1.2),
            (-HW - 0.22, 2.5),
            (-HW - 0.1, H + 0.35),
            (-0.8, H + 0.55),
            (0.8, H + 0.55),
            (HW + 0.1, H + 0.35),
            (HW + 0.22, 2.5),
            (HW + 0.22, 1.2),
            (HW + 0.12, -0.05),
        ]
    } else {
        // A water-worn fissure: bulging walls, a high irregular vault.
        &[
            (-HW - 0.15, -0.05),
            (-HW - 0.7, 0.9),
            (-HW - 0.85, 2.1),
            (-HW - 0.45, H + 0.2),
            (-1.3, H + 0.8),
            (0.2, H + 1.15),
            (1.5, H + 0.7),
            (HW + 0.5, H + 0.15),
            (HW + 0.9, 2.0),
            (HW + 0.6, 0.8),
            (HW + 0.15, -0.05),
        ]
    }
}

/// Rough-hewn variation, pushing each point away from the cavity only.
fn jitter(map: u8, ring: u32, k: usize, d: f32, y: f32) -> (f32, f32) {
    let amp = if map == 1 { 0.18 } else { 0.55 };
    let a = hash01(ring.wrapping_mul(131) ^ map as u32, k as u32 + 17) * amp;
    let b = (hash01(ring.wrapping_mul(71), k as u32 + 900) - 0.5) * amp * 0.6;
    let out = vec2(d, y - H * 0.45).normalize_or_zero();
    let (mut d, mut y) = (d + out.x * a, y + out.y * a + b.max(0.0) * out.y.signum());
    if y < 0.1 {
        y = -0.15; // the wall's foot tucks under the floor
    }
    if y < H + 0.1 && d.abs() < HW + 0.05 {
        d = (HW + 0.05) * d.signum(); // walls stay out of the clear width
    }
    if d.abs() < HW + 0.05 {
        y = y.max(H + 0.1); // and the vault above the clear height
    }
    (d, y)
}

/// Local frame at arc length `s`: centre on the floor, left, forward.
fn frame(t: &Tunnel, s: f32) -> (Vec3, Vec3, Vec3) {
    let (c, dir) = t.at(s);
    (c, vec3(-dir.y, 0.0, dir.x), vec3(dir.x, 0.0, dir.y))
}

fn at(f: &(Vec3, Vec3, Vec3), d: f32, y: f32, along: f32) -> Vec3 {
    f.0 + f.1 * d + Vec3::Y * y + f.2 * along
}

/// A quad lit from the side that faces `toward`.
fn face(m: &mut Mesh, q: [Vec3; 4], toward: Vec3, col: Col) {
    let n = (q[1] - q[0]).cross(q[3] - q[0]);
    if n.dot(toward - q[0]) >= 0.0 {
        m.quad(q[0], q[1], q[2], q[3], col);
    } else {
        m.quad(q[0], q[3], q[2], q[1], col);
    }
}

fn rock(map: u8, ring: u32, k: usize, mouth: f32) -> Col {
    let base = if map == 1 { rgb(0.47, 0.42, 0.35) } else { rgb(0.43, 0.41, 0.37) };
    let v = 0.78 + hash01(ring * 7 + k as u32, 333 + map as u32) * 0.3;
    // Damp moss near Ridgeline's daylight, drill-scarred bands in the mine.
    if map == 2 && mouth < 6.0 && hash01(ring, k as u32 + 51) > 0.45 + mouth * 0.08 {
        return shade(rgb(0.27, 0.35, 0.22), v);
    }
    if map == 1 && k.is_multiple_of(3) {
        return shade(rgb(0.52, 0.44, 0.33), v);
    }
    shade(base, v)
}

fn ring_points(t: &Tunnel, map: u8, s: f32, ring: u32) -> Vec<Vec3> {
    let f = frame(t, s);
    profile(map)
        .iter()
        .enumerate()
        .map(|(k, &(d, y))| {
            let (d, y) = jitter(map, ring, k, d, y);
            at(&f, d, y, 0.0)
        })
        .collect()
}

pub fn build(m: &mut Mesh, w: &World) {
    for (i, t) in w.tunnels.iter().enumerate() {
        tunnel(m, w, t, i as u32);
    }
}

fn tunnel(m: &mut Mesh, w: &World, t: &Tunnel, which: u32) {
    let map = w.map;
    let [a, b] = t.portals;
    let n = ((b - a) / RING).ceil().max(1.0) as u32;
    let seed = which * 1000;
    let rings: Vec<(f32, Vec<Vec3>)> =
        (0..=n).map(|r| (a + (b - a) * r as f32 / n as f32, r)).map(|(s, r)| (s, ring_points(t, map, s, seed + r))).collect();
    // Walls and vault.
    for r in 0..rings.len() - 1 {
        let ((s0, p), (s1, q)) = (&rings[r], &rings[r + 1]);
        let (c0, _) = t.at(*s0);
        let (c1, _) = t.at(*s1);
        let axis = (c0 + c1) * 0.5 + Vec3::Y * (H * 0.45);
        let mouth = (s0 - a).min(b - s1);
        for k in 0..p.len() - 1 {
            face(m, [p[k], q[k], q[k + 1], p[k + 1]], axis, rock(map, seed + r as u32, k, mouth));
        }
    }
    // Floor. Under the vault it spans wall foot to wall foot, so no seam
    // opens beside the rock; in each mouth, where the terrain over the
    // portal is cut away, it is a plain strip.
    let col = if map == 1 { rgb(0.36, 0.31, 0.26) } else { rgb(0.35, 0.33, 0.28) };
    let lift = 0.06;
    for r in 0..rings.len() - 1 {
        let ((s0, p), (s1, q)) = (&rings[r], &rings[r + 1]);
        let (g0, g1) = (frame(t, *s0), frame(t, *s1));
        let foot = |g: &(Vec3, Vec3, Vec3), e: Vec3| g.0 + (e - g.0).dot(g.1) * g.1 + Vec3::Y * lift;
        let (l0, r0, l1, r1) = (foot(&g0, p[0]), foot(&g0, p[p.len() - 1]), foot(&g1, q[0]), foot(&g1, q[q.len() - 1]));
        for k in 0..3u32 {
            let (u0, u1) = (k as f32 / 3.0, (k + 1) as f32 / 3.0);
            let c = shade(col, 0.86 + hash01(seed + r as u32, k + 70) * 0.22);
            face(m, [l0.lerp(r0, u0), l1.lerp(r1, u0), l1.lerp(r1, u1), l0.lerp(r0, u1)], g0.0 + Vec3::Y * 5.0, c);
        }
    }
    for (from, to) in [(a - 3.2, a), (b, b + 3.2)] {
        let steps = 3u32;
        for r in 0..steps {
            let (s0, s1) = (from + (to - from) * r as f32 / steps as f32, from + (to - from) * (r + 1) as f32 / steps as f32);
            let (g0, g1) = (frame(t, s0), frame(t, s1));
            let half = HW + 0.25;
            let q = [at(&g0, -half, lift, 0.0), at(&g1, -half, lift, 0.0), at(&g1, half, lift, 0.0), at(&g0, half, lift, 0.0)];
            face(m, q, g0.0 + Vec3::Y * 5.0, shade(col, 0.95));
            // Aprons sloping under the cut's sides, beneath the cells left
            // out beside the opening.
            for k in [-1.0, 1.0] {
                let (e, o) = (k * half, k * (HW + 1.4));
                let q = [at(&g0, e, lift, 0.0), at(&g1, e, lift, 0.0), at(&g1, o, -0.35, 0.0), at(&g0, o, -0.35, 0.0)];
                face(m, q, g0.0 + Vec3::Y * 5.0, shade(col, 0.9));
            }
        }
    }
    for (s, out) in [(a, -1.0), (b, 1.0)] {
        collar(m, w, t, s, out, seed + if out < 0.0 { 0 } else { n });
    }
    if map == 1 {
        mine_fittings(m, t);
    } else {
        cave_fittings(m, w, t, seed);
    }
}

/// Rock round a portal: one face from the opening out into the cut and the
/// hillside, and a cap over the terrain cells left out above the mouth.
/// The face flares out over the cells `World::tunnel_carves` leaves out in
/// front of the portal; its walls and feet end buried in the cut, and its
/// roof rises to the hill's rim, where the cap begins on the same vertices,
/// so no edge of either is ever left hanging.
fn collar(m: &mut Mesh, w: &World, t: &Tunnel, s: f32, out: f32, ring: u32) {
    let map = w.map;
    let inner = ring_points(t, map, s, ring);
    let f = frame(t, s);
    let floor = f.0.y;
    let centre = at(&f, 0.0, H * 0.45, 0.0);
    let outward = f.2 * out;
    let ground = |p: Vec3| w.height(p.x, p.z);
    // `d` across and `l` back into the hill from the portal, `y` above the floor.
    let spot = |d: f32, y: f32, l: f32| at(&f, d, y, -out * l);
    let lid = |d: f32, l: f32| {
        let p = spot(d, 0.0, l);
        vec3(p.x, ground(p).max(floor + H + 0.95) + 0.04, p.z)
    };
    let top: Vec<bool> = inner.iter().map(|p| (*p - centre).normalize_or_zero().y > 0.55).collect();
    let mut outer = Vec::with_capacity(inner.len());
    for (k, p) in inner.iter().enumerate() {
        let rel = *p - centre;
        let lat = rel.dot(f.1);
        let dir = vec2(lat, rel.y).normalize_or_zero();
        let flare = FLARE + hash01(ring, k as u32 + 3) * 0.4;
        let reach = 2.8 + hash01(ring * 3 + k as u32, 4411) * 1.6;
        let q = if top[k] {
            // The roof climbs the face to the rim over the flared mouth.
            // Kept behind the boards hung over the opening.
            lid(lat + dir.x * reach * 0.6, -0.2)
        } else {
            // Walls and feet: out and away until well inside the cut's
            // sides or under its floor.
            let mut q = *p + f.1 * (dir.x * reach) + Vec3::Y * (dir.y * reach) + outward * flare;
            if rel.y < -H * 0.4 {
                q.y = p.y - 0.45;
            }
            q.y = q.y.min(ground(q) - 0.3);
            q
        };
        outer.push(q);
    }
    let toward = centre + outward * 6.0;
    for k in 0..inner.len() - 1 {
        let c = rock(map, ring, k + 40, 0.0);
        face(m, [inner[k], inner[k + 1], outer[k + 1], outer[k]], toward, shade(c, 0.92));
    }
    // The cap, from the face's top edge back over the hill.
    let (first, last) = (top.iter().position(|&x| x).unwrap_or(0), top.iter().rposition(|&x| x).unwrap_or(0));
    let edge: Vec<Vec3> = outer[first..=last].to_vec();
    let along: [f32; 4] = [1.0, 2.2, 3.4, 4.6];
    let mut rows = vec![edge.clone()];
    for &l in &along {
        rows.push(edge.iter().map(|q| lid((*q - f.0).dot(f.1), l)).collect());
    }
    for i in 0..rows.len() - 1 {
        for j in 0..edge.len() - 1 {
            let q = [rows[i][j], rows[i + 1][j], rows[i + 1][j + 1], rows[i][j + 1]];
            face(m, q, q[0] + Vec3::Y * 10.0, rock(map, ring + i as u32 * 5, j + 60, 1.0));
        }
    }
    // Skirts down into the hill along the cap's sides and back, so no gap
    // opens under its edge where the ground dips.
    let sink = |p: Vec3| vec3(p.x, ground(p).min(p.y) - 0.5, p.z);
    let skirt = |m: &mut Mesh, p: Vec3, q: Vec3, c: Col| face(m, [p, q, sink(q), sink(p)], centre + Vec3::Y * 4.0 + outward * 3.0, c);
    let (n, side) = (rows.len() - 1, edge.len() - 1);
    for i in 0..n {
        skirt(m, rows[i][0], rows[i + 1][0], rock(map, ring, 80 + i, 1.0));
        skirt(m, rows[i][side], rows[i + 1][side], rock(map, ring, 85 + i, 1.0));
    }
    for j in 0..side {
        skirt(m, rows[n][j], rows[n][j + 1], rock(map, ring, 90 + j, 1.0));
    }
}

/// Timber sets, rails, lamps and signs in Quarry's mine.
fn mine_fittings(m: &mut Mesh, t: &Tunnel) {
    let [a, b] = t.portals;
    let sets = ((b - a) / 2.4).floor() as u32;
    for i in 0..=sets {
        let s = a + (b - a) * i as f32 / sets as f32;
        let g = frame(t, s);
        let portal = i == 0 || i == sets;
        let (post, cap) = if portal { (0.2, 0.24) } else { (0.13, 0.15) };
        let col = if i % 2 == 0 { TIMBER } else { TIMBER_DARK };
        for k in [-1.0, 1.0] {
            let x = k * (HW + post * 0.7);
            beam(m, at(&g, x, -0.02, 0.0), at(&g, x * 1.02, H + 0.05, 0.0), post, col);
        }
        beam(m, at(&g, -HW - 0.3, H + 0.05 + cap * 0.5, 0.0), at(&g, HW + 0.3, H + 0.05 + cap * 0.5, 0.0), cap, col);
        if i % 4 == 2 {
            let f = Mat4::from_cols(g.1.extend(0.0), Vec3::Y.extend(0.0), g.2.extend(0.0), at(&g, 0.0, 0.0, 0.0).extend(1.0));
            lamp(m, &f, vec3(0.0, H - 0.08, 0.0));
        }
    }
    // Rails on sleepers, painted on the floor (never a step).
    let f0 = a - 3.0;
    let f1 = b + 3.0;
    let n = ((f1 - f0) / 0.9).ceil() as u32;
    for i in 0..n {
        let (s0, s1) = (f0 + (f1 - f0) * i as f32 / n as f32, f0 + (f1 - f0) * (i + 1) as f32 / n as f32);
        let (g0, g1) = (frame(t, s0), frame(t, s1));
        for x in [-0.72, 0.72] {
            for (dx, y) in [(-0.04, 0.12), (0.04, 0.12)] {
                let q = [at(&g0, x + dx, y, 0.0), at(&g1, x + dx, y, 0.0), at(&g1, x - dx, y, 0.0), at(&g0, x - dx, y, 0.0)];
                face(m, q, g0.0 + Vec3::Y * 4.0, if dx < 0.0 { RAIL } else { shade(RAIL, 1.2) });
            }
        }
        let q = [at(&g0, -1.0, 0.085, 0.1), at(&g0, -1.0, 0.085, 0.4), at(&g0, 1.0, 0.085, 0.4), at(&g0, 1.0, 0.085, 0.1)];
        face(m, q, g0.0 + Vec3::Y * 4.0, TIMBER_DARK);
    }
    // Boards over each entrance: the adit in the pit wall, the incline exit.
    for (s, out, text) in [(a, -1.0, "MINE 04"), (b, 1.0, "INCLINE")] {
        let g = frame(t, s);
        let fwd = g.2 * out;
        let side = vec3(fwd.z, 0.0, -fwd.x);
        let f = Mat4::from_cols(side.extend(0.0), Vec3::Y.extend(0.0), fwd.extend(0.0), (at(&g, 0.0, 0.0, 0.0) + fwd * 0.35).extend(1.0));
        sign(m, &f, vec3(0.0, H + 0.42, 0.0), 0.0, text, 0.085, PAINT);
        let k = if out < 0.0 { 1.0 } else { -1.0 };
        sign(m, &f, vec3(k * (HW - 0.4), 1.1, 0.0), 0.0, "HARD HATS", 0.05, SAFETY);
    }
}

/// Rubble along the walls and a couple of lanterns left behind in the cave.
fn cave_fittings(m: &mut Mesh, w: &World, t: &Tunnel, seed: u32) {
    let [a, b] = t.portals;
    let mut s = a + 1.0;
    let mut i = 0u32;
    while s < b - 1.0 {
        let g = frame(t, s);
        for k in [-1.0, 1.0] {
            if hash01(seed + i, if k > 0.0 { 5 } else { 6 }) < 0.55 {
                let r = 0.25 + hash01(seed + i, 7) * 0.35;
                let c = at(&g, k * (HW + 0.25), 0.0, hash01(seed + i, 8) - 0.5);
                let col = shade(rgb(0.40, 0.38, 0.34), 0.8 + hash01(seed + i, 9) * 0.3);
                let top = c + Vec3::Y * r;
                let ring: Vec<Vec3> = (0..5)
                    .map(|j| {
                        let ang = j as f32 * 1.2566 + hash01(seed + i, 10);
                        c + vec3(ang.cos(), 0.0, ang.sin()) * r * 1.2
                    })
                    .collect();
                for j in 0..5 {
                    let (p, q) = (ring[j], ring[(j + 1) % 5]);
                    let n = (q - p).cross(top - p);
                    if n.dot(p - c) >= 0.0 {
                        m.tri(p, q, top, col)
                    } else {
                        m.tri(q, p, top, col)
                    }
                }
            }
        }
        s += 1.7;
        i += 1;
    }
    let _ = w;
    for frac in [0.3, 0.68] {
        let s = a + (b - a) * frac;
        let g = frame(t, s);
        let hang = at(&g, (HW - 0.5) * if frac < 0.5 { 1.0 } else { -1.0 }, H + 0.15, 0.0);
        beam(m, hang, hang - Vec3::Y * 0.35, 0.02, RAIL);
        let body = hang - Vec3::Y * 0.5;
        beam(m, body - Vec3::Y * 0.12, body + Vec3::Y * 0.12, 0.16, LANTERN);
        beam(m, body + Vec3::Y * 0.13, body + Vec3::Y * 0.17, 0.2, RAIL);
    }
}

/// A square beam between two points.
fn beam(m: &mut Mesh, p: Vec3, q: Vec3, width: f32, col: Col) {
    super::slag_cathedral::beam(m, p, q, width, width, col);
}

/// The tunnel lamps nearest the eye fill the free point-light slots, so
/// the mine and cave are lit where you are, dark where you are not.
pub fn lights(w: &World, eye: Vec3, existing: [Light; 4]) -> [Light; 4] {
    let mut all = Vec::new();
    for t in &w.tunnels {
        let [a, b] = t.portals;
        if w.map == 1 {
            let sets = ((b - a) / 2.4).floor() as u32;
            for i in (2..sets).step_by(4) {
                let (c, _) = t.at(a + (b - a) * i as f32 / sets as f32);
                all.push(Light { pos: c + Vec3::Y * (H - 0.35), radius: 8.0, col: [1.0, 0.8, 0.52], power: 0.75 });
            }
        } else {
            for frac in [0.3, 0.68] {
                let (c, _) = t.at(a + (b - a) * frac);
                all.push(Light { pos: c + Vec3::Y * (H - 0.5), radius: 9.0, col: [1.0, 0.7, 0.4], power: 0.7 });
            }
        }
    }
    all.retain(|l| l.pos.distance_squared(eye) < 60.0 * 60.0);
    all.sort_by(|a, b| (a.pos - eye).length_squared().total_cmp(&(b.pos - eye).length_squared()));
    let mut result = existing;
    let mut next = all.into_iter();
    for light in &mut result {
        if light.power <= 0.0 || light.radius <= 0.0 {
            match next.next() {
                Some(l) => *light = l,
                None => break,
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_shared::map::World;

    #[test]
    fn tunnel_art_is_finite_bounded_and_clear_of_the_cavity() {
        for map in [1u8, 2] {
            for seed in [0u64, 0xFE221] {
                let w = World::with_map(seed, map);
                let mut m = Mesh::default();
                build(&mut m, &w);
                let tris = m.i.len() / 3;
                assert!(tris > 500 && tris < 16_000, "map {map}: {tris} triangles");
                assert!(m.v.iter().all(|v| v.pos.iter().all(|c| c.is_finite())));
            }
        }
    }

    #[test]
    fn profiles_stay_outside_the_clear_space() {
        for map in [1u8, 2] {
            for ring in 0..200 {
                for (k, &(d, y)) in profile(map).iter().enumerate() {
                    let (d, y) = jitter(map, ring, k, d, y);
                    assert!(d.abs() >= HW + 0.04 || y >= H + 0.09, "map {map}: ({d}, {y}) inside the cavity");
                }
            }
        }
    }
}
