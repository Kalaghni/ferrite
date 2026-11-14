//! Paint follows the flat faces of the machined parts, including moving
//! magazines and actions. No textures or extra draw calls are needed.

use crate::gfx::{rgb, shade, Col, Mesh};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::sim::{WeaponPattern, WeaponSkinDef};

pub(super) fn receiver(
    m: &mut Mesh,
    g: &Mat4,
    center: Vec3,
    half: Vec3,
    col: Col,
    finish: &WeaponSkinDef,
) {
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
    let point = |i: usize, z: f32| g.transform_point3(center + vec3(ring[i].0, ring[i].1, z));
    let front = g.transform_point3(center - Vec3::Z * half.z);
    let back = g.transform_point3(center + Vec3::Z * half.z);
    for i in 0..8 {
        let j = (i + 1) % 8;
        m.quad(
            point(i, -half.z),
            point(j, -half.z),
            point(j, half.z),
            point(i, half.z),
            col,
        );
        m.tri(front, point(j, -half.z), point(i, -half.z), col);
        m.tri(back, point(i, half.z), point(j, half.z), col);
    }
    let Some(ink) = finish.pattern_color else {
        return;
    };
    // Keep small hardware and the bevel itself free of pattern noise.
    if half.z < 0.04 || half.y < 0.02 || finish.pattern == WeaponPattern::Plain {
        return;
    }
    let ink = rgb(ink[0], ink[1], ink[2]);
    let faint = [
        ink[0] * 0.55 + col[0] * 0.45,
        ink[1] * 0.55 + col[1] * 0.45,
        ink[2] * 0.55 + col[2] * 0.45,
        0.0,
    ];
    let salt = (center.z * 1000.0).round() as i32 as u32 ^ (half.z * 1000.0) as u32;
    // A fraction of a millimetre avoids z fighting without floating decals.
    for side in [-1.0f32, 1.0] {
        let origin = center
            + vec3(
                side * (half.x + 0.00018),
                -half.y + bevel + 0.001,
                -half.z + 0.002,
            );
        let along = Vec3::Z * (half.z * 2.0 - 0.004);
        let up = Vec3::Y * ((half.y - bevel) * 2.0 - 0.002);
        pattern(
            m,
            g,
            origin,
            along,
            up,
            side > 0.0,
            finish.pattern,
            ink,
            faint,
            salt,
        );
    }
    let origin = center + vec3(-half.x + bevel + 0.001, half.y + 0.00018, -half.z + 0.002);
    pattern(
        m,
        g,
        origin,
        Vec3::Z * (half.z * 2.0 - 0.004),
        Vec3::X * ((half.x - bevel) * 2.0 - 0.002),
        false,
        finish.pattern,
        shade(ink, 0.93),
        faint,
        salt,
    );
}

fn hash(x: u32, y: u32, salt: u32) -> u32 {
    let mut n = x.wrapping_mul(0x85ebca6b) ^ y.wrapping_mul(0xc2b2ae35) ^ salt;
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb352d);
    n ^ (n >> 15)
}

#[allow(clippy::too_many_arguments)]
fn pattern(
    m: &mut Mesh,
    g: &Mat4,
    origin: Vec3,
    along: Vec3,
    up: Vec3,
    reverse: bool,
    kind: WeaponPattern,
    ink: Col,
    faint: Col,
    salt: u32,
) {
    let mut polygon = |p: &[(f32, f32)], color: Col| {
        let at = |i: usize| g.transform_point3(origin + along * p[i].0 + up * p[i].1);
        for k in 1..p.len() - 1 {
            if reverse {
                m.tri(at(0), at(k + 1), at(k), color);
            } else {
                m.tri(at(0), at(k), at(k + 1), color);
            }
        }
    };
    match kind {
        WeaponPattern::Plain => {}
        WeaponPattern::Digital => {
            // Quantised patches retain their shape on narrow stocks and
            // broad receivers; the hash is stable across clients and poses.
            for x in 0..13 {
                for y in 0..4 {
                    let h = hash(x, y, salt);
                    if h % 7 > 2 {
                        continue;
                    }
                    let (u, v) = (x as f32 / 13.0, y as f32 / 4.0);
                    let w = 1.0 / 13.0;
                    let x1 = (u + w).min(1.0);
                    polygon(
                        &[(u, v), (x1, v), (x1, v + 0.25), (u, v + 0.25)],
                        if h & 16 == 0 { ink } else { faint },
                    );
                }
            }
        }
        WeaponPattern::Bands => {
            for k in -1..4 {
                let u = k as f32 * 0.30;
                let band = [
                    (u - 0.10, 0.0),
                    (u + 0.015, 0.0),
                    (u + 0.265, 1.0),
                    (u + 0.15, 1.0),
                ];
                let clipped = clip_u(&clip_u(&band, 0.0, true), 1.0, false);
                if clipped.len() >= 3 {
                    polygon(&clipped, ink);
                }
            }
        }
    }
}

/// Clip slanted bands to the actual flat face, never beyond the bevel.
fn clip_u(poly: &[(f32, f32)], edge: f32, keep_greater: bool) -> Vec<(f32, f32)> {
    let inside = |p: (f32, f32)| {
        if keep_greater {
            p.0 >= edge
        } else {
            p.0 <= edge
        }
    };
    let mut out = Vec::new();
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if inside(a) {
            out.push(a);
        }
        if inside(a) != inside(b) {
            let t = (edge - a.0) / (b.0 - a.0);
            out.push((edge, a.1 + (b.1 - a.1) * t));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_shared::sim::WEAPON_SKINS;

    #[test]
    fn paint_stays_on_the_part_with_outward_facing_normals() {
        let half = vec3(0.04, 0.05, 0.2);
        let mut factory = Mesh::default();
        receiver(
            &mut factory,
            &Mat4::IDENTITY,
            Vec3::ZERO,
            half,
            rgb(0.3, 0.3, 0.3),
            &WEAPON_SKINS[0],
        );
        for finish in &WEAPON_SKINS[1..] {
            let mut mesh = Mesh::default();
            receiver(
                &mut mesh,
                &Mat4::IDENTITY,
                Vec3::ZERO,
                half,
                rgb(0.3, 0.3, 0.3),
                finish,
            );
            assert!(
                mesh.v.len() > factory.v.len(),
                "{} has no pattern",
                finish.name
            );
            for v in &mesh.v[factory.v.len()..] {
                let p = Vec3::from(v.pos);
                let n = Vec3::from(v.nrm);
                assert!(p.is_finite() && n.is_finite());
                assert!(
                    p.abs().cmple(half + Vec3::splat(0.0002)).all(),
                    "{} paint beyond part: {p:?}",
                    finish.name
                );
                assert!(n.dot(p) > 0.03, "{} has inward paint at {p:?}", finish.name);
                assert!(n.length() > 0.99);
            }
        }
    }
}
