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
