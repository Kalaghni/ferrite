//! Canopy, the art pass.
//!
//! A botanical reserve in the roots and lower boughs of a colossal fig. The
//! collision is read from the shared layout (`ferrite_shared::map::canopy`),
//! so what is drawn is what is stood on; the high crown above it is scenery.
use super::hash01;
use super::slag_cathedral::beam;
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
    // Trunks, roots and lips: bark with darker furrows.
    m.cuboid(min, max, shade(BARK, j));
    let h = max.y - min.y;
    if h > 3.0 {
        let long_x = max.x - min.x > max.z - min.z;
        let n = (((if long_x { max.x - min.x } else { max.z - min.z }) / 2.2) as usize).max(2);
        for k in 1..n {
            let t = k as f32 / n as f32;
            let e = 0.05;
            let c = shade(BARK_DARK, 0.9 + 0.2 * hash01(i as u32, k as u32));
            if long_x {
                let x = min.x + (max.x - min.x) * t;
                m.cuboid(vec3(x - 0.12, min.y, min.z - e), vec3(x + 0.12, max.y, max.z + e), c);
            } else {
                let z = min.z + (max.z - min.z) * t;
                m.cuboid(vec3(min.x - e, min.y, z - 0.12), vec3(max.x + e, max.y, z + 0.12), c);
            }
        }
    }
    let _ = rgb;
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
    if w.map != ID {
        return;
    }
    cranks(m, w);
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
