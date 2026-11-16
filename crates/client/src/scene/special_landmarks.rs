//! Off-field landmarks: the relay's cellular backbone and the foundry's volcano.
//! Their moving parts are ambient scenery; neither changes collision or damage.
use super::slag_cathedral::art::{annulus, cross, soft, stencil, wall};
use super::slag_cathedral::{beam, pipe};
use super::{hash01, smooth};
use crate::gfx::{shade, Col, Mesh};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::World;
use std::f32::consts::{PI, TAU};

pub(crate) const MAST_XZ: (f32, f32) = (0.0, 126.0);
pub(crate) const VOLCANO_XZ: (f32, f32) = (0.0, -160.0);
pub(crate) const ERUPTION_PERIOD: f32 = 96.0;
const STEEL: Col = [0.36, 0.39, 0.42, 0.0];
const WHITE: Col = [0.84, 0.85, 0.80, 0.0];
const RED: Col = [0.71, 0.19, 0.13, 0.0];
const DARK: Col = [0.10, 0.12, 0.14, 0.0];
const HOT: Col = [0.95, 0.21, 0.025, 0.9];

pub(crate) fn mast_origin(w: &World) -> Vec3 {
    vec3(MAST_XZ.0, w.height(MAST_XZ.0, MAST_XZ.1), MAST_XZ.1)
}
pub(crate) fn volcano_origin(w: &World) -> Vec3 {
    vec3(VOLCANO_XZ.0, w.height(VOLCANO_XZ.0, VOLCANO_XZ.1), VOLCANO_XZ.1)
}

/// Each 96-second ambient window has a 40% chance of an eruption. Smoke
/// thickens first, then a lava fountain launches bombs, then the plume cools.
/// Based only on the world seed and visual clock; it never affects players.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Eruption {
    pub warning: f32,
    pub fountain: f32,
    pub ash: f32,
    pub age: f32,
    pub cycle: u32,
}
pub(crate) fn eruption(seed: u64, time: f32) -> Eruption {
    let time = if time.is_finite() { time.max(0.0) } else { 0.0 };
    let cycle = (time / ERUPTION_PERIOD).floor() as u32;
    let phase = time.rem_euclid(ERUPTION_PERIOD);
    let rolled = hash01((seed ^ (seed >> 32)) as u32 ^ cycle.wrapping_mul(0x9E37), 0xCADA);
    let mut out = Eruption { warning: 0.0, fountain: 0.0, ash: 0.0, age: phase - 26.0, cycle };
    if rolled >= 0.40 {
        return out;
    }
    out.warning = smooth(16.0, 26.0, phase) * (1.0 - smooth(44.0, 62.0, phase));
    out.fountain = smooth(26.0, 29.0, phase) * (1.0 - smooth(39.0, 46.0, phase));
    out.ash = smooth(21.0, 36.0, phase) * (1.0 - smooth(57.0, 82.0, phase));
    out
}

pub fn build(m: &mut Mesh, w: &World) {
    match w.map {
        5 => cellular_mast(m, w),
        6 => volcano(m, w),
        _ => {}
    }
}

fn cellular_mast(m: &mut Mesh, w: &World) {
    let root = mast_origin(w);
    let at = |x: f32, y: f32, z: f32| root + vec3(x, y, z);
    let half = |y: f32| 4.3 - y * 0.057;
    // Four tapering legs with individually braced bays, white and oxide red
    // above the tree line. This is the whole station's unmistakable backbone.
    for sx in [-1.0f32, 1.0] {
        for sz in [-1.0f32, 1.0] {
            m.cuboid(at(sx * 4.3 - 0.8, -1.0, sz * 4.3 - 0.8), at(sx * 4.3 + 0.8, 1.0, sz * 4.3 + 0.8), [0.45, 0.44, 0.40, 0.0]);
            for bay in 0..10 {
                let y0 = bay as f32 * 5.7;
                let y1 = y0 + 5.7;
                let paint = if bay % 3 == 1 { RED } else { WHITE };
                let p0 = at(sx * half(y0), y0, sz * half(y0));
                let p1 = at(sx * half(y1), y1, sz * half(y1));
                beam(m, p0, p1, 0.30, 0.30, paint);
                for axis in [0, 1] {
                    let q0 = if axis == 0 { at(-sx * half(y0), y0, sz * half(y0)) } else { at(sx * half(y0), y0, -sz * half(y0)) };
                    let q1 = if axis == 0 { at(-sx * half(y1), y1, sz * half(y1)) } else { at(sx * half(y1), y1, -sz * half(y1)) };
                    beam(m, p0, q1, 0.095, 0.095, shade(paint, 0.8));
                    if sx > 0.0 && sz > 0.0 {
                        beam(m, p0, q0, 0.16, 0.16, STEEL);
                    }
                }
            }
        }
    }
    // Ladder and a dark cable tray disappear into the lattice.
    for k in 0..113 {
        let y = k as f32 * 0.49 + 0.5;
        beam(m, at(-0.4, y, -0.5), at(0.4, y, -0.5), 0.045, 0.045, STEEL);
    }
    for x in [-0.43f32, 0.43] {
        beam(m, at(x, 0.4, -0.5), at(x, 56.0, -0.5), 0.075, 0.075, STEEL);
    }
    m.cuboid(at(0.8, 0.4, -0.55), at(1.15, 57.0, -0.30), DARK);
    // Three tiers of directional cellular panels, each visibly joined to
    // its service ring and carrying a paired radio unit behind it.
    for (tier, y) in [39.0f32, 48.0, 57.0].into_iter().enumerate() {
        annulus(m, at(0.0, y - 2.25, 0.0), 0.8, 3.9, 20, DARK);
        for k in 0..6 {
            let a = k as f32 * TAU / 6.0 + tier as f32 * 0.17;
            let radial = vec3(a.sin(), 0.0, a.cos());
            let f = Mat4::from_translation(at(0.0, y, 0.0) + radial * 3.5) * Mat4::from_rotation_y(a);
            beam(m, at(0.0, y - 1.25, 0.0), at(0.0, y - 1.25, 0.0) + radial * 3.45, 0.14, 0.14, STEEL);
            m.obox(&f, Vec3::ZERO, vec3(0.43, 1.85, 0.19), WHITE);
            m.obox(&f, vec3(0.0, 0.0, -0.25), vec3(0.32, 1.5, 0.09), STEEL);
            m.obox(&f, vec3(0.0, -0.55, -0.48), vec3(0.27, 0.48, 0.15), DARK);
            for seam in [-0.82f32, 0.82] {
                m.obox(&f, vec3(0.0, seam, 0.197), vec3(0.39, 0.022, 0.008), shade(WHITE, 0.7));
            }
            let a1 = a + TAU / 6.0;
            let p = |angle: f32, h: f32| at(angle.sin() * 3.8, h, angle.cos() * 3.8);
            beam(m, p(a, y - 2.2), p(a, y - 1.1), 0.055, 0.055, STEEL);
            beam(m, p(a, y - 1.1), p(a1, y - 1.1), 0.06, 0.06, STEEL);
        }
    }
    pipe(m, at(0.0, 55.0, 0.0), at(0.0, 64.0, 0.0), 0.19, STEEL);
    pipe(m, at(0.0, 63.5, 0.0), at(0.0, 67.0, 0.0), 0.055, WHITE);
    // A small maintenance dish and its support bracket on the lower mast.
    let dish = Mat4::from_translation(at(-3.3, 29.0, -3.0)) * Mat4::from_rotation_y(PI);
    m.tube(&dish, Vec3::ZERO, 1.8, 1.5, 0.18, 18, WHITE);
    beam(m, at(-1.0, 28.8, -1.0), at(-3.3, 29.0, -3.0), 0.2, 0.2, STEEL);
    pipe(m, at(-3.3, 29.0, -3.0), at(-3.3, 29.0, -4.3), 0.07, STEEL);
    for a in [0.0f32, TAU / 3.0, TAU * 2.0 / 3.0] {
        let end = at(a.cos() * 1.6 - 3.3, 29.0 + a.sin() * 1.6, -3.0);
        beam(m, end, at(-3.3, 29.0, -4.3), 0.035, 0.035, STEEL);
    }
    let hut = at(10.5, 0.0, 1.5);
    m.cuboid(hut, hut + vec3(6.0, 3.8, 5.0), [0.42, 0.43, 0.42, 0.0]);
    m.cuboid(hut + vec3(-0.3, 3.8, -0.3), hut + vec3(6.3, 4.05, 5.3), DARK);
    m.cuboid(hut + vec3(0.5, 0.1, -0.035), hut + vec3(2.1, 2.8, 0.0), STEEL);
    stencil(m, &wall(hut + vec3(4.0, 2.3, -0.045), PI), "BACKBONE", 0.09, WHITE);
}

// The outermost ring is planted into the surrounding ridge; all other rings
// form a closed, faceted cone and the inside of its sunken caldera.
const CONE: [(f32, f32); 7] = [(47.0, -2.0), (39.0, 6.0), (27.0, 20.0), (17.0, 35.0), (10.0, 48.0), (6.8, 47.2), (5.8, 43.2)];
fn cone_point(root: Vec3, ring: usize, angle: f32) -> Vec3 {
    let (r, h) = CONE[ring];
    let jag = (angle * 7.0 + 0.3).sin() * 0.032 + (angle * 11.0).cos() * 0.018;
    root + vec3(angle.cos() * r * (1.0 + jag), h + (angle * 9.0).cos() * if ring == 0 { 0.0 } else { 0.55 }, angle.sin() * r * (1.0 + jag))
}
fn volcano(m: &mut Mesh, w: &World) {
    let root = volcano_origin(w);
    const N: usize = 48;
    for ring in 0..CONE.len() - 1 {
        for k in 0..N {
            let a0 = k as f32 * TAU / N as f32;
            let a1 = (k + 1) as f32 * TAU / N as f32;
            let p = |r, a| {
                let mut p = cone_point(root, r, a);
                if r == 0 {
                    p.y = w.height(p.x, p.z) - 1.0;
                }
                p
            };
            let c = if ring >= 4 { [0.19, 0.115, 0.10, 0.0] } else { [0.25, 0.225, 0.24, 0.0] };
            m.quad(p(ring, a0), p(ring + 1, a0), p(ring + 1, a1), p(ring, a1), shade(c, 0.8 + hash01(k as u32, ring as u32 + 414) * 0.4));
        }
    }
    // Caldera pool and narrow glowing seams pouring over the near rim.
    let lava = root + Vec3::Y * 43.22;
    for k in 0..32 {
        let a = k as f32 * TAU / 32.0;
        let b = (k + 1) as f32 * TAU / 32.0;
        m.tri(lava, lava + vec3(a.cos(), 0.0, a.sin()) * 5.8, lava + vec3(b.cos(), 0.0, b.sin()) * 5.8, HOT);
    }
    for (seam, angle) in [0.53f32, 1.25, 1.84, 2.40, 4.15].into_iter().enumerate() {
        for ring in 1..5 {
            let a = angle + (ring as f32 * 1.8 + seam as f32).sin() * 0.05;
            let b = angle + ((ring + 1) as f32 * 1.8 + seam as f32).sin() * 0.05;
            let width = if ring > 2 { 0.018 } else { 0.012 };
            m.quad(
                cone_point(root, ring, a - width) + Vec3::Y * 0.12,
                cone_point(root, ring + 1, b - width) + Vec3::Y * 0.12,
                cone_point(root, ring + 1, b + width) + Vec3::Y * 0.12,
                cone_point(root, ring, a + width) + Vec3::Y * 0.12,
                shade(HOT, 0.65),
            );
        }
    }
}

pub fn atmosphere(m: &mut Mesh, alpha: &mut Mesh, add: &mut Mesh, w: &World, time: f32) {
    if w.map == 5 {
        let root = mast_origin(w);
        let pulse = if time.rem_euclid(2.0) < 0.18 { 1.0 } else { 0.08 };
        for y in [26.0f32, 45.0, 64.0] {
            let p = root + Vec3::Y * y;
            cross(add, p, 0.23, 0.23, [1.0, 0.10, 0.035, pulse]);
            soft(add, p, 1.2, 1.2, [1.0, 0.08, 0.025, pulse * 0.65]);
        }
        // A rotating microwave scanner below the upper antenna crowns.
        let g = Mat4::from_translation(root + vec3(0.0, 32.0, 0.0)) * Mat4::from_rotation_y(time * 0.35);
        m.obox(&g, vec3(0.0, 0.0, 3.4), vec3(1.7, 0.45, 0.14), WHITE);
        m.obox(&g, vec3(0.0, 0.0, 1.7), vec3(0.12, 0.13, 1.7), STEEL);
    } else if w.map == 6 {
        volcano_atmosphere(m, alpha, add, w, time.max(0.0));
    }
}

fn volcano_atmosphere(m: &mut Mesh, alpha: &mut Mesh, add: &mut Mesh, w: &World, time: f32) {
    let root = volcano_origin(w);
    let vent = root + Vec3::Y * 47.0;
    let state = eruption(w.seed, time);
    let seed = (w.seed ^ (w.seed >> 32)) as u32;
    // Even a dormant cone breathes. During a warning the column thickens;
    // after the blast it spreads into a large, dark anvil of ash aloft.
    for k in 0..42u32 {
        let age = (time * (0.035 + hash01(k, 744) * 0.014) + hash01(k, seed ^ 745)).fract();
        let h = age * (29.0 + 39.0 * state.ash);
        let width = 2.0 + age * (8.0 + state.ash * 13.0);
        let p = vent + vec3(age * (15.0 + 12.0 * state.ash) + (k as f32 * 1.9).sin() * age * 2.7, h, age * -7.0);
        let opacity = (0.13 + state.warning * 0.14 + state.ash * 0.18) * (1.0 - age).powf(0.6);
        soft(alpha, p, width, width * 0.85, [0.12, 0.105, 0.12, opacity]);
    }
    let flicker = 0.8 + 0.2 * (time * 8.3).sin();
    soft(
        add,
        vent - Vec3::Y * 1.0,
        5.8 + state.fountain * 3.0,
        2.0 + state.fountain * 6.0,
        [1.0, 0.22, 0.025, (0.22 + state.warning * 0.45) * flicker],
    );
    if state.fountain > 0.001 {
        // A pulsing fountain emerges from the crater before the larger bombs
        // arc outward. Every bomb and its tail stays on the distant cone.
        for k in 0..12u32 {
            let a = k as f32 * TAU / 12.0;
            let wave = 0.75 + 0.25 * (time * 4.8 + k as f32 * 1.3).sin();
            let top = vent + vec3(a.cos() * 1.8, 8.0 + 17.0 * state.fountain * wave, a.sin() * 1.8);
            beam(m, vent + vec3(a.cos(), 0.0, a.sin()) * 2.4, top, 0.35 + state.fountain * 0.32, 0.35, [1.0, 0.36, 0.045, 1.0]);
        }
        for k in 0..40u32 {
            let life = 4.8 + hash01(k, 751) * 2.0;
            let age = (time / life + hash01(k, seed ^ state.cycle ^ 752)).fract();
            let secs = age * life;
            let a = hash01(k, 753) * TAU;
            let reach = 2.5 + hash01(k, 754) * 2.8;
            let speed = 16.0 + hash01(k, 755) * 8.0;
            let p = vent + vec3(a.cos() * reach * secs, speed * secs - 4.8 * secs * secs, a.sin() * reach * secs);
            if p.y < root.y + 20.0 {
                continue;
            }
            let size = (0.20 + hash01(k, 756) * 0.34) * state.fountain;
            let velocity = vec3(a.cos() * reach, speed - 9.6 * secs, a.sin() * reach);
            let tail = p - velocity.normalize_or_zero() * (1.2 + age * 1.8);
            beam(m, tail, p, size * 0.65, size * 0.65, [0.8, 0.14, 0.015, 0.95]);
            m.cuboid(p - Vec3::splat(size), p + Vec3::splat(size), [1.0, 0.61, 0.10, 1.0]);
            soft(add, p, size * 3.0, size * 3.0, [1.0, 0.28, 0.025, state.fountain * (1.0 - age) * 0.85]);
        }
    }
    // The plume rains fine incandescent fragments after its main fountain.
    for k in 0..55u32 {
        let age = (time * 0.10 + hash01(k, seed ^ 761)).fract();
        let a = hash01(k, 762) * TAU;
        let spread = 3.0 + age * (8.0 + hash01(k, 763) * 15.0);
        let p = vent + vec3(a.cos() * spread + age * 12.0, 5.0 + (1.0 - age) * 32.0, a.sin() * spread);
        cross(add, p, 0.10, 0.24, [1.0, 0.30, 0.04, state.ash * (1.0 - age) * 0.8]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finite(mesh: &Mesh) {
        for vertices in mesh.chunks.iter().map(|(v, _)| v).chain(std::iter::once(&mesh.v)) {
            for v in vertices {
                assert!(v.pos.iter().chain(v.nrm.iter()).chain(v.col.iter()).all(|x| x.is_finite()));
            }
        }
    }

    #[test]
    fn distant_landmarks_and_all_eruption_stages_are_finite_and_bounded() {
        for id in [5, 6] {
            let w = World::with_map(0xFE221, id);
            let mut static_mesh = Mesh::default();
            build(&mut static_mesh, &w);
            finite(&static_mesh);
            assert!(static_mesh.triangles() > 500 && static_mesh.triangles() < 14000);
            for vertices in static_mesh.chunks.iter().map(|(v, _)| v).chain(std::iter::once(&static_mesh.v)) {
                for v in vertices {
                    assert!(if id == 5 { v.pos[2] > 110.0 } else { v.pos[2] < -110.0 }, "landmark enters playable field: {:?}", v.pos);
                }
            }
            for time in [0.0, 22.0, 32.0, 45.0, 62.0, 128.0, 224.0, 10001.0] {
                let (mut m, mut a, mut glow) = (Mesh::default(), Mesh::default(), Mesh::default());
                atmosphere(&mut m, &mut a, &mut glow, &w, time);
                for mesh in [&m, &a, &glow] {
                    finite(mesh);
                }
                assert!(m.triangles() + a.triangles() + glow.triangles() < 5000);
            }
        }
    }

    #[test]
    fn eruption_has_a_real_chance_and_warning_blast_and_cooldown() {
        let seed = 0xFE221;
        let active = (0..64).filter(|&cycle| eruption(seed, cycle as f32 * ERUPTION_PERIOD + 33.0).fountain > 0.5).collect::<Vec<_>>();
        assert!(!active.is_empty() && active.len() < 64, "need both erupting and dormant windows");
        let base = active[0] as f32 * ERUPTION_PERIOD;
        let warning = eruption(seed, base + 23.0);
        assert!(warning.warning > 0.0 && warning.fountain == 0.0);
        let blast = eruption(seed, base + 33.0);
        assert!(blast.fountain > 0.9 && blast.ash > 0.0);
        let cooling = eruption(seed, base + 60.0);
        assert!(cooling.ash > 0.5 && cooling.fountain == 0.0);
        assert_eq!(eruption(seed, base + 33.0), blast);
        eprintln!("Volcano review blast time: {}", base + 33.0);
    }
}
