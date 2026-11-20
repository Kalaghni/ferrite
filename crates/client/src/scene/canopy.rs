//! Canopy, the art pass.
//!
//! A botanical reserve in the roots and lower boughs of a colossal fig. The
//! collision is read from the shared layout (`ferrite_shared::map::canopy`),
//! so what is drawn is what is stood on; the high crown above it is scenery.
use super::hash01;
use super::slag_cathedral::{basis, beam};
use crate::gfx::{rgb, shade, Col, Mesh};
use ferrite_shared::glam::{vec3, Vec3};
use ferrite_shared::map::{canopy as layout, Block, BlockKind, World};

pub const ID: u8 = layout::MAP_ID;

/// Warm umber bark, the pale cut wood of walkways, and the bark lip.
const BARK: Col = [0.36, 0.26, 0.18, 0.0];
const BARK_DARK: Col = [0.25, 0.18, 0.13, 0.0];
const PLANK: Col = [0.62, 0.52, 0.38, 0.0];

/// Draw one of Canopy's tree blocks (trunks, roots, boughs, lips). Other
/// kinds are drawn by the common block code.
pub fn block(m: &mut Mesh, _w: &World, b: &Block, i: usize) {
    debug_assert_eq!(b.kind, BlockKind::Trunk);
    let j = 0.9 + 0.2 * hash01(i as u32, 0xCA70);
    let (min, max) = (b.min, b.max);
    let flat = max.y - min.y < 1.0 && (max.x - min.x).max(max.z - min.z) > 3.0;
    if flat && b.tint == 2 {
        // A bough slab: bark below, a walkway of pale planks on top.
        m.cuboid(min, vec3(max.x, max.y - 0.08, max.z), shade(BARK, j));
        let inset = 0.35;
        m.cuboid(vec3(min.x + inset, max.y - 0.08, min.z + inset), vec3(max.x - inset, max.y, max.z - inset), shade(PLANK, j));
        return;
    }
    // Trunks, roots and lips: dark bark, and on anything tall the
    // strangler's lattice of roots grown down over it.
    m.cuboid(min, max, shade(BARK_DARK, j * 1.05));
    if max.y - min.y > 3.0 {
        ribs(m, min, max, i as u32);
    }
}

/// How far a rib stands proud of the collision box: little enough that
/// what is seen is what is hit.
const RIB: f32 = 0.22;

/// Root ribs down every side of a tall bark block: irregular in width and
/// spacing, some stopping short, the corners wrapped by the heaviest.
fn ribs(m: &mut Mesh, min: Vec3, max: Vec3, seed: u32) {
    let h = max.y - min.y;
    // Each face: an origin corner, the direction along it, its length and
    // its outward normal.
    let faces = [
        (vec3(min.x, 0.0, max.z), Vec3::X, max.x - min.x, Vec3::Z),
        (vec3(max.x, 0.0, min.z), Vec3::NEG_X, max.x - min.x, Vec3::NEG_Z),
        (vec3(max.x, 0.0, max.z), Vec3::NEG_Z, max.z - min.z, Vec3::X),
        (vec3(min.x, 0.0, min.z), Vec3::Z, max.z - min.z, Vec3::NEG_X),
    ];
    for (f, &(o, along, len, out)) in faces.iter().enumerate() {
        let mut u = 0.0;
        let mut k = 0u32;
        while u < len {
            let id = seed.wrapping_mul(31).wrapping_add(f as u32 * 1000 + k);
            let corner = u == 0.0;
            let width = if corner { 1.4 } else { 0.5 + 1.1 * hash01(id, 1) };
            let w = width.min(len - u);
            let gap = 0.6 + 1.8 * hash01(id, 2);
            if w > 0.25 {
                // Most ribs run the full height; some hang from the top and
                // stop short, or rise from the ground and fade out.
                let r = hash01(id, 3);
                let (y0, y1) = if corner || r < 0.55 {
                    (min.y, max.y)
                } else if r < 0.8 {
                    (min.y + h * (0.2 + 0.5 * hash01(id, 4)), max.y)
                } else {
                    (min.y, min.y + h * (0.3 + 0.5 * hash01(id, 4)))
                };
                let a = o + along * u;
                let b = a + along * w;
                let proud = if corner { RIB } else { RIB * (0.5 + 0.5 * hash01(id, 5)) };
                let lo = a.min(b).min(a + out * proud);
                let hi = a.max(b).max(b + out * proud);
                let c = shade(BARK, 0.86 + 0.28 * hash01(id, 6));
                m.cuboid(vec3(lo.x, y0, lo.z), vec3(hi.x, y1, hi.z), c);
            }
            u += w + gap;
            k += 1;
        }
    }
}

/// Faded turquoise for the reserve's fittings, dull brass for its pulleys.
const FITTING: Col = [0.24, 0.52, 0.50, 0.0];
const BRASS: Col = [0.62, 0.50, 0.26, 0.0];
const CABLE: Col = [0.14, 0.13, 0.12, 0.0];
/// The guide runs this far above a rider's feet: the pulley over the sling.
pub const GUIDE_LIFT: f32 = 2.45;

/// The seedlines' fixed guides: a cable over each rider's path, a pulley at
/// every bend, and at each dock a post with a turquoise collar and a
/// painted landing.
pub fn build(m: &mut Mesh, w: &World) {
    // Remixes keep their seedlines (and riders need the guides), but the
    // shade cranks belong to Canopy itself.
    if w.map == ID {
        cranks(m, w);
    } else if w.lines.is_empty() {
        return;
    }
    let lift = Vec3::Y * GUIDE_LIFT;
    for line in &w.lines {
        let pts: Vec<Vec3> = line.pts.iter().map(|&p| p + lift).collect();
        for k in 0..pts.len() - 1 {
            beam(m, pts[k], pts[k + 1], 0.07, 0.07, CABLE);
        }
        for &p in &pts[1..pts.len() - 1] {
            m.cuboid(p - vec3(0.22, 0.22, 0.22), p + vec3(0.22, 0.22, 0.22), BRASS);
        }
        for &d in [line.pts[0], line.pts[line.pts.len() - 1]].iter() {
            // The dock: a post beside the landing up past the cable head,
            // and the landing itself painted on the floor.
            let side = vec3(0.9, 0.0, 0.0);
            let top = d + lift + Vec3::Y * 0.6;
            m.cuboid(d + side - vec3(0.09, 0.0, 0.09), top + side + vec3(0.09, 0.0, 0.09), shade(FITTING, 0.9));
            m.cuboid(top + side - vec3(0.16, 0.5, 0.16), top + side + vec3(0.16, 0.0, 0.16), FITTING);
            beam(m, top + side - Vec3::Y * 0.2, d + lift + Vec3::Y * 0.4, 0.06, 0.06, shade(FITTING, 0.8));
            let r = 0.8;
            m.quad(d + vec3(-r, 0.02, -r), d + vec3(-r, 0.02, r), d + vec3(r, 0.02, r), d + vec3(r, 0.02, -r), FITTING);
        }
    }
}

/// The sling of someone riding a seedline: a strap from the harness up to
/// the pulley block running on the guide.
pub fn sling(m: &mut Mesh, feet: Vec3) {
    let waist = feet + Vec3::Y * 1.0;
    let block = feet + Vec3::Y * GUIDE_LIFT;
    beam(m, waist, block - Vec3::Y * 0.15, 0.08, 0.03, [0.86, 0.66, 0.20, 0.0]);
    m.cuboid(block - vec3(0.14, 0.15, 0.14), block + vec3(0.14, 0.15, 0.14), BRASS);
    m.cuboid(waist - vec3(0.2, 0.06, 0.16), waist + vec3(0.2, 0.06, 0.16), [0.86, 0.66, 0.20, 0.0]);
}

/// Cloth: off-white and faded turquoise in broad vertical stripes.
const CLOTH: [Col; 2] = [[0.86, 0.84, 0.76, 0.0], [0.34, 0.58, 0.55, 0.0]];

/// The shade curtains: a drum over each panel, and the cloth let down from
/// it as far as its bank is, rippling when the crown gust blows.
pub fn shades(m: &mut Mesh, w: &World, time: f32) {
    if w.map != ID {
        return;
    }
    let gust = ferrite_shared::events::canopy_gust(w.clock).strength;
    for k in 0..2 {
        let down = w.shades[k].clamp(0.0, 1.0);
        for (x, z0, z1) in layout::shade_panels(k) {
            let top = layout::SHADE_TOP;
            m.cuboid(vec3(x - 0.22, top - 0.02, z0 - 0.3), vec3(x + 0.22, top + 0.42, z1 + 0.3), shade(BARK_DARK, 1.2));
            let roll = 0.32 + 0.18 * (1.0 - down);
            m.cuboid(vec3(x - roll * 0.5, top - roll * 0.5, z0), vec3(x + roll * 0.5, top + roll * 0.5, z1), CLOTH[0]);
            if down < 0.02 {
                continue;
            }
            let bottom = top - layout::SHADE_DROP * down;
            let n = 8;
            for i in 0..n {
                let (za, zb) = (z0 + (z1 - z0) * i as f32 / n as f32, z0 + (z1 - z0) * (i + 1) as f32 / n as f32);
                // A ripple across the cloth, stronger low down and in the wind.
                let wave = |z: f32, y: f32| {
                    let low = (top - y) / layout::SHADE_DROP;
                    (0.05 + 0.35 * gust) * low * ((z * 0.9 + time * (1.4 + 3.0 * gust)).sin())
                };
                let c = shade(CLOTH[i % 2], 0.95 + 0.1 * hash01(i as u32, k as u32));
                let a = vec3(x + wave(za, top), top, za);
                let b = vec3(x + wave(zb, top), top, zb);
                let cc = vec3(x + wave(zb, bottom), bottom, zb);
                let d = vec3(x + wave(za, bottom), bottom, za);
                m.quad(a, b, cc, d, c);
                m.quad(d, cc, b, a, shade(c, 0.85));
            }
        }
    }
}

/// The shade cranks: a big wheel on a bracket on the inward face of each
/// trunk, its rim painted turquoise, and the drive shaft from it up the
/// bark and across to both curtain drums of its bank; and a post at each
/// end of every drum.
pub fn cranks(m: &mut Mesh, w: &World) {
    let top = layout::SHADE_TOP + 0.2;
    for k in 0..2 {
        let c = layout::crank(w, k);
        let s = if k == 0 { 1.0 } else { -1.0 };
        // Bracket, wheel (a turquoise rim round a dark hub), handle.
        m.cuboid(c + vec3(-0.75 * s - 0.12, -0.7, -0.14), c + vec3(-0.75 * s + 0.12, 0.7, 0.14), shade(BRASS, 0.8));
        m.cuboid(c + vec3(-0.12, -0.6, -0.6), c + vec3(0.12, 0.6, 0.6), FITTING);
        m.cuboid(c + vec3(-0.14, -0.42, -0.42), c + vec3(0.14, 0.42, 0.42), shade(BARK_DARK, 0.9));
        m.cuboid(c + vec3(-0.18, -0.12, -0.12), c + vec3(0.18, 0.12, 0.12), BRASS);
        m.cuboid(c + vec3(-0.05, 0.35, -0.06), c + vec3(0.35 * s, 0.47, 0.06), BRASS);
        // The drive: up the bark, across to the curtain line, along it.
        let x = layout::SHADE_X * s;
        let up = vec3(c.x - 0.55 * s, top, c.z);
        beam(m, c + vec3(-0.55 * s, 0.0, 0.0), up, 0.12, 0.12, shade(BRASS, 0.85));
        beam(m, up, vec3(x, top, c.z), 0.12, 0.12, shade(BRASS, 0.85));
        let panels = layout::shade_panels(k);
        let (lo, hi) = (panels.iter().map(|p| p.1).fold(f32::MAX, f32::min), panels.iter().map(|p| p.2).fold(f32::MIN, f32::max));
        beam(m, vec3(x, top, lo), vec3(x, top, hi), 0.1, 0.1, shade(BRASS, 0.85));
        m.cuboid(vec3(x - 0.2, top - 0.2, c.z - 0.2), vec3(x + 0.2, top + 0.2, c.z + 0.2), BRASS);
        // Posts carrying the drums.
        for (_, z0, z1) in panels {
            for z in [z0 - 0.45, z1 + 0.45] {
                let g = w.height(x, z);
                m.cuboid(vec3(x - 0.16, g - 0.2, z - 0.16), vec3(x + 0.16, top + 0.45, z + 0.16), shade(BARK_DARK, 1.1));
                m.cuboid(vec3(x - 0.22, top + 0.3, z - 0.22), vec3(x + 0.22, top + 0.55, z + 0.22), FITTING);
            }
        }
    }
}

/// A tapering, faceted limb from `a` (radius `ra`) to `b` (radius `rb`).
fn limb(m: &mut Mesh, a: Vec3, ra: f32, b: Vec3, rb: f32, col: Col, seed: u32) {
    const N: usize = 7;
    let (g, len) = basis(a, b);
    let ring = |k: usize, r: f32, z: f32| {
        let t = (k % N) as f32 / N as f32 * std::f32::consts::TAU + seed as f32;
        g.transform_point3(vec3(t.cos() * r, t.sin() * r, z))
    };
    let (z0, z1) = (-len * 0.5, len * 0.5);
    for k in 0..N {
        let j = 0.86 + 0.24 * hash01(k as u32, seed);
        m.quad(ring(k, ra, z0), ring(k + 1, ra, z0), ring(k + 1, rb, z1), ring(k, rb, z1), shade(col, j));
    }
    // A cap, so a limb's end is never seen hollow.
    for k in 0..N {
        m.tri(ring(k + 1, rb, z1), ring(k, rb, z1), b, shade(col, 0.8));
    }
}

/// Where each main trunk stands on the ground plan: the layout's `TRUNKS`
/// and their mirrors.
/// Whether the great fig stands here: Canopy itself, or a remix in its mood
/// that still has a trunk at the foot of every limb of the crown.
pub fn has_fig(w: &World) -> bool {
    if w.map == ID {
        return true;
    }
    if w.map < ferrite_shared::map::doc::FIRST_CUSTOM || ferrite_shared::map::doc::look_of(w.map) != ID {
        return false;
    }
    trunk_feet().iter().all(|&(x, z)| {
        w.blocks.iter().any(|b| {
            b.kind == BlockKind::Trunk
                && b.max.y > layout::TRUNK_TOP - 4.0
                && (b.min.x..=b.max.x).contains(&x)
                && (b.min.z..=b.max.z).contains(&z)
        })
    })
}

fn trunk_feet() -> Vec<(f32, f32)> {
    let mut v = Vec::new();
    for &(x, z, _, _) in layout::TRUNKS.iter() {
        v.push((x, z));
        v.push((-x, -z));
    }
    v
}

/// Leaf colours for the reserve's ordinary trees (by tint): rainforest
/// greens, one with a flush of new red growth.
pub const LEAF: [[f32; 3]; 5] = [[0.20, 0.42, 0.22], [0.30, 0.48, 0.20], [0.15, 0.34, 0.24], [0.42, 0.50, 0.18], [0.52, 0.24, 0.18]];

/// Leaves of the high crown: sunlit tops warm green, the underside the
/// cool shadowed green of the storm light.
const CROWN_TOP: [[f32; 3]; 3] = [[0.30, 0.47, 0.22], [0.25, 0.43, 0.24], [0.36, 0.50, 0.24]];
const CROWN_UNDER: [[f32; 3]; 2] = [[0.13, 0.25, 0.19], [0.16, 0.29, 0.20]];

/// A cluster of leaves: a rounded sunlit top over a cooler underside.
fn leaves(m: &mut Mesh, c: Vec3, r: f32, id: u32) {
    let rot = hash01(id, 4) * std::f32::consts::TAU;
    let top = CROWN_TOP[(hash01(id, 6) * 3.0) as usize % 3];
    let under = CROWN_UNDER[(hash01(id, 7) * 2.0) as usize % 2];
    super::octa(m, c, vec3(r, r * (0.5 + 0.2 * hash01(id, 8)), r), rot, rgb(top[0], top[1], top[2]));
    super::octa(
        m,
        c + vec3(2.0 * (hash01(id, 9) - 0.5), -r * 0.45, 2.0 * (hash01(id, 10) - 0.5)),
        vec3(r * 0.92, r * 0.35, r * 0.92),
        rot + 0.6,
        rgb(under[0], under[1], under[2]),
    );
}

/// The Hollow Crown: the tree above anything a player can reach. Each main
/// trunk climbs on past its collision and spreads into great limbs; a ring
/// of arching limbs holds open a well of sky over C; and the leaf roof,
/// highest round that opening and drooping toward its edge, spreads some
/// 164 by 144 m. It is scenery: it casts no shadow (that would put the
/// whole field in shade) and nothing collides with it.
pub fn crown(m: &mut Mesh, w: &World) {
    if !has_fig(w) {
        return;
    }
    let feet = trunk_feet();
    let mut heads = Vec::new();
    for (k, &(x, z)) in feet.iter().enumerate() {
        let out = vec3(x, 0.0, z).normalize_or_zero();
        let foot = vec3(x, layout::TRUNK_TOP - 2.0, z);
        let head = foot + out * 6.0 + Vec3::Y * 16.0;
        limb(m, foot, 6.6, head, 4.4, BARK, 40 + k as u32);
        heads.push(head);
        // Great limbs spreading out and up from each trunk head.
        let base = out.z.atan2(out.x);
        let spread = [(-1.25f32, 44.0f32, 10.0f32), (-0.5, 52.0, 14.0), (0.35, 50.0, 12.0), (1.1, 42.0, 9.0)];
        for (j, (da, reach, rise)) in spread.into_iter().enumerate() {
            let a = base + da + 0.12 * (hash01(k as u32, j as u32) - 0.5);
            let dir = vec3(a.cos(), 0.0, a.sin());
            let elbow = head + dir * reach * 0.55 + Vec3::Y * rise;
            let tip = head + dir * reach + Vec3::Y * (rise + 6.0);
            limb(m, head - Vec3::Y * 2.0, 3.0, elbow, 1.9, shade(BARK, 1.05), 60 + (k * 4 + j) as u32);
            limb(m, elbow, 1.9, tip, 0.8, shade(BARK, 1.1), 90 + (k * 4 + j) as u32);
            leaves(m, tip + Vec3::Y * 2.0, 9.0, 300 + (k * 4 + j) as u32);
            // A pair of lesser branches off the elbow.
            for (q, side) in [-0.7f32, 0.7].into_iter().enumerate() {
                let b = a + side;
                let end = elbow + vec3(b.cos(), 0.0, b.sin()) * 14.0 + Vec3::Y * 8.0;
                limb(m, elbow, 1.0, end, 0.4, shade(BARK, 1.12), 130 + (k * 8 + j * 2 + q) as u32);
                leaves(m, end + Vec3::Y * 1.5, 6.5, 400 + (k * 8 + j * 2 + q) as u32);
            }
        }
    }
    // Arches between neighbouring trunk heads, round the opening.
    let mut order = heads.clone();
    order.sort_by(|a, b| a.z.atan2(a.x).total_cmp(&b.z.atan2(b.x)));
    for k in 0..order.len() {
        let (a, b) = (order[k], order[(k + 1) % order.len()]);
        let mid = (a + b) * 0.5;
        let lift = mid + vec3(mid.x, 0.0, mid.z).normalize_or_zero() * 4.0 + Vec3::Y * 9.0;
        limb(m, a, 2.8, lift, 2.2, shade(BARK, 0.95), 200 + k as u32);
        limb(m, lift, 2.2, b, 2.8, shade(BARK, 0.95), 210 + k as u32);
    }
    // The leaf roof: two layers of faceted clusters over an ellipse with a
    // ragged hole of sky over the root well.
    let (rx, rz) = (82.0f32, 72.0f32);
    let cell = 10.0;
    let (nx, nz) = ((rx / cell) as i32 + 1, (rz / cell) as i32 + 1);
    for ix in -nx..=nx {
        for iz in -nz..=nz {
            let id = ((ix + 40) * 131 + (iz + 40)) as u32;
            let x = ix as f32 * cell + (hash01(id, 1) - 0.5) * cell * 0.8;
            let z = iz as f32 * cell + (hash01(id, 2) - 0.5) * cell * 0.8;
            let e = ((x / rx).powi(2) + (z / rz).powi(2)).sqrt();
            let ang = z.atan2(x);
            let hole = 0.3 + 0.05 * (ang * 3.0).sin() + 0.03 * (ang * 7.0 + 1.0).sin();
            if e > 1.0 || e < hole {
                continue;
            }
            // Highest just outside the hole, a broad dome, then a droop
            // over the rim; lumpy everywhere.
            let t = ((e - hole) / (1.0 - hole)).clamp(0.0, 1.0);
            let rise = 6.0 * (1.0 - (t * 5.0).min(1.0));
            let lump = 10.0 * (hash01(id, 3) - 0.5) + 6.0 * ((x * 0.07).sin() * (z * 0.09).cos());
            let y = 80.0 - 18.0 * t - 16.0 * t.powi(3) - rise + lump;
            let r = 10.0 + 7.0 * hash01(id, 5).powi(2);
            leaves(m, vec3(x, y, z), r, id);
        }
    }
}
