//! Optic attachments, connected iron sights and their shared aim axis.

use crate::gfx::{shade, Col, Mesh};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::sim::weapon_def;

pub const SIGHT_NAMES: [&str; 6] = [
    "Iron sights",
    "Red dot",
    "Holo",
    "2.5x optic",
    "4x scope",
    "8x scope",
];
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
pub(super) fn rail(wid: u8) -> (f32, f32, f32) {
    match weapon_def(wid).arch {
        0 => (0.066, -0.02, 1.3),
        1 => (0.063, -0.1, 1.3),
        2 => (0.052, -0.12, 1.3),
        _ => (0.051, -0.03, 0.8),
    }
}

/// Height of the sight picture above the rail (what lines up with the eye).
pub(super) fn sight_center(sight: u8) -> f32 {
    match sight {
        RED_DOT => 0.04,
        HOLO => 0.042,
        OPTIC => 0.036,
        SCOPE4 | SCOPE8 => 0.04,
        _ => 0.0,
    }
}

/// Height at which the eye meets the front post or optic aperture.
pub(super) fn sight_height(wid: u8, sight: u8) -> f32 {
    if sight != IRON {
        let (ry, _, scale) = rail(wid);
        return ry + sight_center(sight) * scale;
    }
    match weapon_def(wid).arch {
        1 => 0.078,
        2 => 0.071,
        4 => 0.064,
        _ => 0.087,
    }
}

/// A factory attachment on a weapon's own rail, without the weapon.
/// The muzzle faces -Z and apertures remain open for the HUD reticle.
#[allow(dead_code)] // Entry point for the standalone asset review example.
pub fn sight_model(m: &mut Mesh, wid: u8, sight: u8, g: Mat4) {
    let dark = [0.095, 0.105, 0.12, 0.0];
    let metal = [0.25, 0.27, 0.30, 0.0];
    let accent = [0.15, 0.46, 0.96, 0.0];
    if sight == IRON {
        let (ry, rz, s) = rail(wid);
        let tip = (sight_height(wid, IRON) - ry) / s;
        let frame =
            g * Mat4::from_translation(vec3(0.0, ry, rz)) * Mat4::from_scale(Vec3::splat(s));
        m.obox(
            &frame,
            vec3(0.0, -0.003, 0.0),
            vec3(0.023, 0.003, 0.17),
            dark,
        );
        front_sight(m, &frame, -0.15, 0.0, tip, accent, dark);
        rear_sight(m, &frame, 0.14, 0.0, tip, 0.014, dark);
    } else {
        optic(m, &g, wid, sight, accent, dark, metal);
    }
}

/// A point on the common optical axis in canonical weapon coordinates.
pub fn sight_axis(wid: u8, sight: u8) -> Vec3 {
    vec3(0.0, sight_height(wid, sight), rail(wid).1)
}

/// A flat-sided tapered support, with all pieces joined along their edges.
fn pedestal(m: &mut Mesh, g: &Mat4, center: Vec3, bottom: Vec3, top: Vec3, col: Col) {
    let mut p = [Vec3::ZERO; 8];
    for (i, v) in p.iter_mut().enumerate() {
        let h = if i & 2 == 0 { bottom } else { top };
        *v = g.transform_point3(
            center
                + vec3(
                    if i & 1 == 0 { -h.x } else { h.x },
                    if i & 2 == 0 { -bottom.y } else { top.y },
                    if i & 4 == 0 { -h.z } else { h.z },
                ),
        );
    }
    m.corners(p, col);
}

/// The blue bead is a small insert in a continuous, protected front blade.
pub(super) fn front_sight(
    m: &mut Mesh,
    g: &Mat4,
    z: f32,
    base: f32,
    tip: f32,
    accent: Col,
    dark: Col,
) {
    m.obox(
        g,
        vec3(0.0, base + 0.002, z),
        vec3(0.014, 0.004, 0.012),
        dark,
    );
    let bottom = base + 0.003;
    let upper = tip - 0.002;
    let half = ((upper - bottom) * 0.5).max(0.001);
    pedestal(
        m,
        g,
        vec3(0.0, (bottom + upper) * 0.5, z),
        vec3(0.007, half, 0.007),
        vec3(0.003, half, 0.004),
        dark,
    );
    m.obox(
        g,
        vec3(0.0, tip - 0.0015, z),
        vec3(0.0024, 0.0015, 0.0035),
        accent,
    );
    // Ears shield the blade while leaving the centre line completely clear.
    let wing_base = base + 0.004;
    let wing_top = tip + 0.003;
    for side in [-1.0f32, 1.0] {
        let wing = *g * Mat4::from_translation(vec3(side * 0.011, (wing_base + wing_top) * 0.5, z));
        pedestal(
            m,
            &wing,
            Vec3::ZERO,
            vec3(0.003, (wing_top - wing_base) * 0.5, 0.008),
            vec3(0.002, (wing_top - wing_base) * 0.5, 0.005),
            shade(dark, 1.2),
        );
    }
}

pub(super) fn rear_sight(
    m: &mut Mesh,
    g: &Mat4,
    z: f32,
    base: f32,
    tip: f32,
    width: f32,
    dark: Col,
) {
    m.obox(
        g,
        vec3(0.0, base + 0.002, z),
        vec3(width + 0.006, 0.004, 0.01),
        dark,
    );
    let half = ((tip - base) * 0.5).max(0.001);
    for side in [-1.0f32, 1.0] {
        pedestal(
            m,
            g,
            vec3(side * width, (base + tip) * 0.5, z),
            vec3(0.006, half, 0.009),
            vec3(0.003, half, 0.005),
            shade(dark, 1.15),
        );
    }
    // The shelf defines a crisp U-notch; its top stays below the aim line.
    let notch_floor = (tip - 0.009).max(base + 0.004);
    m.obox(
        g,
        vec3(0.0, (base + notch_floor) * 0.5, z),
        vec3(width, (notch_floor - base) * 0.5, 0.005),
        dark,
    );
}

/// Polygonal hollow extrusion. Its bore stays empty through bevels and collars.
#[allow(clippy::too_many_arguments)]
fn shell(
    m: &mut Mesh,
    g: &Mat4,
    cy: f32,
    z0: f32,
    z1: f32,
    r0: f32,
    r1: f32,
    bore: f32,
    n: usize,
    col: Col,
) {
    let (bore0, bore1) = (bore.min(r0 - 0.003), bore.min(r1 - 0.003));
    let point = |angle: f32, r: f32, z: f32| {
        g.transform_point3(vec3(angle.cos() * r, cy + angle.sin() * r, z))
    };
    for k in 0..n {
        let a = std::f32::consts::TAU * k as f32 / n as f32 + std::f32::consts::PI / n as f32;
        let b = a + std::f32::consts::TAU / n as f32;
        m.quad(
            point(a, r0, z0),
            point(b, r0, z0),
            point(b, r1, z1),
            point(a, r1, z1),
            col,
        );
        m.quad(
            point(b, bore0, z0),
            point(a, bore0, z0),
            point(a, bore1, z1),
            point(b, bore1, z1),
            shade(col, 0.48),
        );
        m.quad(
            point(b, bore0, z0),
            point(b, r0, z0),
            point(a, r0, z0),
            point(a, bore0, z0),
            col,
        );
        m.quad(
            point(a, bore1, z1),
            point(a, r1, z1),
            point(b, r1, z1),
            point(b, bore1, z1),
            shade(col, 0.85),
        );
    }
}

fn frame(m: &mut Mesh, g: &Mat4, cy: f32, z0: f32, z1: f32, col: Col) {
    let outline = |x: f32, y: f32, bevel: f32| {
        [
            (-x + bevel, -y),
            (x - bevel, -y),
            (x, -y + bevel),
            (x, y - bevel),
            (x - bevel, y),
            (-x + bevel, y),
            (-x, y - bevel),
            (-x, -y + bevel),
        ]
    };
    let outer = outline(0.034, 0.034, 0.01);
    let inner = outline(0.027, 0.026, 0.008);
    let point = |p: (f32, f32), z: f32| g.transform_point3(vec3(p.0, cy + p.1, z));
    for i in 0..8 {
        let j = (i + 1) % 8;
        m.quad(
            point(outer[i], z0),
            point(outer[j], z0),
            point(outer[j], z1),
            point(outer[i], z1),
            col,
        );
        m.quad(
            point(inner[j], z0),
            point(inner[i], z0),
            point(inner[i], z1),
            point(inner[j], z1),
            shade(col, 0.55),
        );
        m.quad(
            point(inner[j], z0),
            point(outer[j], z0),
            point(outer[i], z0),
            point(inner[i], z0),
            col,
        );
        m.quad(
            point(inner[i], z1),
            point(outer[i], z1),
            point(outer[j], z1),
            point(inner[j], z1),
            shade(col, 0.8),
        );
    }
}

fn knob(m: &mut Mesh, g: &Mat4, c: Vec3, r: f32, half: f32, vertical: bool, col: Col) {
    let at = *g
        * Mat4::from_translation(c)
        * if vertical {
            Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2)
        } else {
            Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2)
        };
    m.tube(&at, Vec3::ZERO, r, 0.0, half, 12, col);
}

fn mount(m: &mut Mesh, g: &Mat4, len: f32, dark: Col, metal: Col) {
    m.obox(g, vec3(0.0, 0.004, 0.0), vec3(0.019, 0.004, len), dark);
    // Two jaws grip the sides of the weapon's rail and touch its top plate.
    for side in [-1.0f32, 1.0] {
        pedestal(
            m,
            g,
            vec3(side * 0.02, 0.002, 0.0),
            vec3(0.004, 0.003, len),
            vec3(0.003, 0.003, len),
            metal,
        );
    }
    for z in [-len * 0.64, len * 0.64] {
        knob(m, g, vec3(0.025, 0.005, z), 0.0045, 0.003, false, metal);
        m.obox(g, vec3(0.0285, 0.005, z), vec3(0.0007, 0.0008, 0.003), dark);
    }
    // Captive throw lever shares its pivot with the right mounting jaw.
    m.obox(
        g,
        vec3(0.027, 0.003, 0.0),
        vec3(0.003, 0.0025, len * 0.36),
        dark,
    );
}

pub(super) fn optic(
    m: &mut Mesh,
    g: &Mat4,
    wid: u8,
    sight: u8,
    accent: Col,
    dark: Col,
    metal: Col,
) {
    let (ry, rz, s) = rail(wid);
    let g = *g * Mat4::from_translation(vec3(0.0, ry, rz)) * Mat4::from_scale(Vec3::splat(s));
    let cy = sight_center(sight);
    match sight {
        RED_DOT => {
            mount(m, &g, 0.038, dark, metal);
            pedestal(
                m,
                &g,
                vec3(0.0, 0.01, -0.01),
                vec3(0.015, 0.004, 0.027),
                vec3(0.013, 0.004, 0.025),
                dark,
            );
            shell(m, &g, cy, -0.036, 0.018, 0.031, 0.029, 0.024, 10, dark);
            shell(m, &g, cy, -0.049, -0.036, 0.035, 0.031, 0.025, 10, metal);
            shell(m, &g, cy, 0.018, 0.024, 0.031, 0.032, 0.024, 10, metal);
            knob(m, &g, vec3(0.034, cy, -0.009), 0.013, 0.009, false, dark);
            knob(m, &g, vec3(0.044, cy, -0.009), 0.0115, 0.002, false, metal);
            m.obox(
                &g,
                vec3(0.0465, cy, -0.009),
                vec3(0.0006, 0.001, 0.006),
                dark,
            );
            knob(
                m,
                &g,
                vec3(0.0, cy + 0.032, -0.012),
                0.007,
                0.004,
                true,
                metal,
            );
            m.obox(
                &g,
                vec3(0.0, cy + 0.036, -0.012),
                vec3(0.003, 0.001, 0.002),
                accent,
            );
        }
        HOLO => {
            mount(m, &g, 0.05, dark, metal);
            pedestal(
                m,
                &g,
                vec3(0.0, 0.013, 0.012),
                vec3(0.028, 0.006, 0.048),
                vec3(0.026, 0.006, 0.046),
                dark,
            );
            frame(m, &g, cy, -0.035, 0.009, metal);
            // Battery tray and sealed electronics sit behind the open hood.
            m.obox(&g, vec3(0.0, 0.017, 0.039), vec3(0.027, 0.008, 0.021), dark);
            m.obox(
                &g,
                vec3(0.0, cy + 0.035, -0.007),
                vec3(0.012, 0.002, 0.012),
                dark,
            );
            m.obox(
                &g,
                vec3(0.0, cy + 0.037, -0.007),
                vec3(0.008, 0.0008, 0.005),
                accent,
            );
            for z in [0.029, 0.047] {
                knob(m, &g, vec3(-0.028, 0.018, z), 0.005, 0.003, false, metal);
                m.obox(
                    &g,
                    vec3(-0.0315, 0.018, z),
                    vec3(0.0005, 0.0007, 0.0025),
                    dark,
                );
            }
        }
        OPTIC => {
            mount(m, &g, 0.052, dark, metal);
            pedestal(
                m,
                &g,
                vec3(0.0, 0.012, 0.0),
                vec3(0.014, 0.005, 0.04),
                vec3(0.012, 0.005, 0.036),
                metal,
            );
            shell(m, &g, cy, -0.064, 0.065, 0.026, 0.024, 0.018, 8, dark);
            shell(m, &g, cy, -0.093, -0.064, 0.032, 0.026, 0.022, 8, dark);
            shell(m, &g, cy, -0.1, -0.093, 0.033, 0.032, 0.022, 8, metal);
            shell(m, &g, cy, 0.065, 0.088, 0.027, 0.028, 0.02, 10, dark);
            for z in [0.069, 0.078, 0.087] {
                shell(m, &g, cy, z, z + 0.003, 0.0285, 0.0285, 0.02, 10, metal);
            }
            knob(
                m,
                &g,
                vec3(0.0, cy + 0.024, -0.014),
                0.0105,
                0.006,
                true,
                metal,
            );
            knob(m, &g, vec3(0.025, cy, -0.014), 0.0095, 0.006, false, metal);
            m.obox(
                &g,
                vec3(-0.024, cy + 0.005, 0.005),
                vec3(0.0015, 0.004, 0.016),
                accent,
            );
        }
        SCOPE4 | SCOPE8 => {
            let high = sight == SCOPE8;
            let body = if high { 0.18 } else { 0.13 };
            let nose = if high { -0.255 } else { -0.20 };
            let bell = if high { 0.041 } else { 0.034 };
            mount(m, &g, 0.105, dark, metal);
            shell(m, &g, cy, -body, body, 0.019, 0.019, 0.014, 12, dark);
            shell(
                m,
                &g,
                cy,
                nose,
                -body,
                bell,
                0.021,
                if high { 0.031 } else { 0.025 },
                12,
                dark,
            );
            shell(
                m,
                &g,
                cy,
                nose - 0.012,
                nose,
                bell + 0.002,
                bell,
                if high { 0.032 } else { 0.026 },
                12,
                metal,
            );
            shell(m, &g, cy, body, body + 0.034, 0.025, 0.028, 0.019, 12, dark);
            shell(
                m,
                &g,
                cy,
                body + 0.034,
                body + 0.047,
                0.028,
                0.027,
                0.019,
                12,
                dark,
            );
            for z in [body + 0.008, body + 0.019, body + 0.03] {
                shell(m, &g, cy, z, z + 0.003, 0.028, 0.028, 0.019, 12, metal);
            }
            for z in [-0.079, 0.079] {
                pedestal(
                    m,
                    &g,
                    vec3(0.0, 0.015, z),
                    vec3(0.015, 0.008, 0.014),
                    vec3(0.012, 0.008, 0.012),
                    metal,
                );
                shell(
                    m,
                    &g,
                    cy,
                    z - 0.007,
                    z + 0.007,
                    0.0235,
                    0.0235,
                    0.019,
                    12,
                    metal,
                );
                for side in [-1.0f32, 1.0] {
                    knob(m, &g, vec3(side * 0.024, cy, z), 0.0035, 0.003, false, dark);
                }
            }
            let turret = if high { 0.014 } else { 0.011 };
            knob(
                m,
                &g,
                vec3(0.0, cy + 0.024, 0.0),
                turret,
                if high { 0.01 } else { 0.007 },
                true,
                metal,
            );
            knob(m, &g, vec3(0.026, cy, 0.0), 0.011, 0.009, false, metal);
            m.obox(
                &g,
                vec3(0.0, cy + if high { 0.035 } else { 0.032 }, 0.0),
                vec3(0.005, 0.001, 0.002),
                dark,
            );
            // An exterior index stripe never crosses the optical bore.
            m.obox(
                &g,
                vec3(
                    0.0,
                    cy + (bell + 0.001) * (std::f32::consts::PI / 12.0).cos() + 0.0004,
                    nose - 0.006,
                ),
                vec3(0.006, 0.0008, 0.003),
                accent,
            );
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_shared::sim;

    fn ray_hits_triangle(origin: Vec3, direction: Vec3, a: Vec3, b: Vec3, c: Vec3) -> bool {
        let (ab, ac) = (b - a, c - a);
        let h = direction.cross(ac);
        let det = ab.dot(h);
        if det.abs() < 1e-8 {
            return false;
        }
        let inv = 1.0 / det;
        let s = origin - a;
        let u = inv * s.dot(h);
        if !(0.0..=1.0).contains(&u) {
            return false;
        }
        let q = s.cross(ab);
        let v = inv * direction.dot(q);
        if v < 0.0 || u + v > 1.0 {
            return false;
        }
        inv * ac.dot(q) > 0.0
    }

    #[test]
    fn every_allowed_attachment_is_finite_and_leaves_its_aim_axis_clear() {
        let mut largest = 0;
        for wid in 0..sim::WEAPONS.len() as u8 {
            if matches!(wid, sim::GRENADE_WEAPON | sim::MELEE_WEAPON) {
                continue;
            }
            for &sight in sights_for(wid) {
                let mut mesh = Mesh::default();
                sight_model(&mut mesh, wid, sight, Mat4::IDENTITY);
                assert!(!mesh.v.is_empty());
                assert!(mesh.chunks.is_empty());
                assert!(mesh.i.iter().all(|&i| (i as usize) < mesh.v.len()));
                assert!(mesh.v.iter().all(|v| v
                    .pos
                    .iter()
                    .chain(&v.nrm)
                    .chain(&v.col)
                    .all(|x| x.is_finite())));
                largest = largest.max(mesh.v.len());
                assert!(
                    mesh.v.len() < 4000,
                    "{} {} attachment exceeds budget: {}",
                    weapon_def(wid).name,
                    SIGHT_NAMES[sight as usize],
                    mesh.v.len()
                );
                if sight == IRON {
                    continue;
                }
                let axis = sight_axis(wid, sight);
                for x in [-0.003, 0.0, 0.003] {
                    for y in [-0.003, 0.0, 0.003] {
                        let origin = axis + vec3(x, y, 1.0);
                        for tri in mesh.i.chunks_exact(3) {
                            let a = Vec3::from(mesh.v[tri[0] as usize].pos);
                            let b = Vec3::from(mesh.v[tri[1] as usize].pos);
                            let c = Vec3::from(mesh.v[tri[2] as usize].pos);
                            assert!(
                                !ray_hits_triangle(origin, -Vec3::Z, a, b, c),
                                "{} {} blocks its view line at {x},{y}",
                                weapon_def(wid).name,
                                SIGHT_NAMES[sight as usize]
                            );
                        }
                    }
                }
            }
        }
        eprintln!("largest attachment: {largest} vertices");
    }
}
