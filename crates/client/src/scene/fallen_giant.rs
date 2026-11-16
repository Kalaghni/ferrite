//! A military heavy-lifter torn open across Ridgeline's saddle.
//! The cargo shell is read from shared collision. Wings and the high tail
//! make one recognizable aircraft; its open ends and six side tears stay clear.
use super::hash01;
use super::slag_cathedral::art::{cross, stencil, wall};
use super::slag_cathedral::{beam, pipe};
use crate::gfx::{shade, Col, Mesh};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::{fallen_giant as layout, Block, World};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Map id, as the other landmark scenes export it.
pub const ID: u8 = layout::MAP_ID;

const SKIN: Col = [0.34, 0.38, 0.34, 0.0];
const LIGHT: Col = [0.47, 0.50, 0.44, 0.0];
const INSIDE: Col = [0.22, 0.26, 0.25, 0.0];
const FRAME: Col = [0.39, 0.42, 0.40, 0.0];
const DARK: Col = [0.065, 0.085, 0.09, 0.0];
const RUST: Col = [0.41, 0.23, 0.14, 0.0];
const WHITE: Col = [0.72, 0.73, 0.65, 0.0];
const YELLOW: Col = [0.83, 0.55, 0.16, 0.0];

fn origin(w: &World) -> Vec3 {
    vec3(0.0, layout::floor(w), 0.0)
}
fn at(w: &World, x: f32, y: f32, z: f32) -> Vec3 {
    origin(w) + vec3(x, y, z)
}
fn box_at(m: &mut Mesh, w: &World, c: Vec3, half: Vec3, col: Col) {
    m.obox(&Mat4::from_translation(origin(w)), c, half, col);
}
fn plate(m: &mut Mesh, points: [Vec3; 4], thick: f32, col: Col) {
    let n = (points[1] - points[0])
        .cross(points[3] - points[0])
        .normalize_or_zero()
        * thick;
    m.quad(points[0], points[1], points[2], points[3], col);
    m.quad(
        points[3] - n,
        points[2] - n,
        points[1] - n,
        points[0] - n,
        shade(col, 0.6),
    );
    for i in 0..4 {
        let j = (i + 1) % 4;
        m.quad(points[i], points[i] - n, points[j] - n, points[j], FRAME);
    }
}

pub fn block(m: &mut Mesh, b: &Block) {
    if b.tint == layout::CARGO_TINT {
        m.cuboid(b.min, b.max, [0.32, 0.31, 0.25, 0.0]);
        for i in 0..4 {
            let x = b.min.x + (b.max.x - b.min.x) * (i as f32 + 0.5) / 4.0;
            m.cuboid(
                vec3(x - 0.045, b.min.y, b.min.z - 0.03),
                vec3(x + 0.045, b.max.y + 0.02, b.max.z + 0.03),
                DARK,
            );
        }
        return;
    }
    m.cuboid(b.min, b.max, INSIDE);
    // Thin outer skin preserves the exact collision silhouette, including
    // the complete doorway rectangles cut into the shared side walls.
    if b.max.y - b.min.y > 1.0 {
        let side = ((b.min.z + b.max.z) * 0.5).signum();
        let z = if side < 0.0 {
            b.min.z - 0.025
        } else {
            b.max.z + 0.025
        };
        let col = shade(SKIN, 0.9 + hash01((b.min.x.abs() * 19.0) as u32, 7) * 0.15);
        let x0 = b.min.x;
        let x1 = b.max.x;
        m.quad(
            vec3(x0, b.min.y, z),
            vec3(x1, b.min.y, z),
            vec3(x1, b.max.y, z),
            vec3(x0, b.max.y, z),
            col,
        );
        m.quad(
            vec3(x0, b.max.y, z),
            vec3(x1, b.max.y, z),
            vec3(x1, b.min.y, z),
            vec3(x0, b.min.y, z),
            col,
        );
    }
}

fn cargo_shell(m: &mut Mesh, w: &World) {
    let half = layout::HALF_LENGTH;
    let roof = layout::ROOF_HEIGHT + 0.39;
    // A broad, faceted crown above the box-section cargo floor. Individual
    // skin bays carry alternate faded paint and separated riveting seams.
    let profile = [
        (-6.42, roof),
        (-5.25, roof + 1.8),
        (-2.8, roof + 3.0),
        (0.0, roof + 3.35),
        (2.8, roof + 3.0),
        (5.25, roof + 1.8),
        (6.42, roof),
    ];
    for bay in 0..21 {
        let x0 = -half + bay as f32 * 4.0 + 0.025;
        let x1 = x0 + 3.95;
        for (i, p) in profile.windows(2).enumerate() {
            let col = shade(SKIN, 0.82 + hash01(bay, i as u32) * 0.22);
            plate(
                m,
                [
                    at(w, x0, p[0].1, p[0].0),
                    at(w, x1, p[0].1, p[0].0),
                    at(w, x1, p[1].1, p[1].0),
                    at(w, x0, p[1].1, p[1].0),
                ],
                0.06,
                col,
            );
        }
        for side in [-1.0f32, 1.0] {
            // Flush seams, small fasteners and fading maintenance numbering.
            pipe(
                m,
                at(w, x0, 4.2, side * 6.39),
                at(w, x0, 7.95, side * 6.39),
                0.025,
                FRAME,
            );
            for y in [4.35, 5.7, 7.7] {
                box_at(
                    m,
                    w,
                    vec3(x0 + 0.13, y, side * 6.40),
                    vec3(0.04, 0.04, 0.025),
                    DARK,
                );
            }
        }
    }
    // Interior ribs stop over the six side doors. The floor remains the
    // levelled ground, so players can enter every breach without jumping.
    for i in 0..=20 {
        let x = -40.0 + i as f32 * 4.0;
        let in_door = layout::SIDE_DOORS
            .iter()
            .any(|d| (x - d).abs() < layout::DOOR_HALF_WIDTH + 0.25);
        for side in [-1.0f32, 1.0] {
            let y0 = if in_door {
                layout::DOOR_HEIGHT + 0.12
            } else {
                0.1
            };
            beam(
                m,
                at(w, x, y0, side * 5.90),
                at(w, x, 7.82, side * 5.90),
                0.13,
                0.13,
                FRAME,
            );
            beam(
                m,
                at(w, x, 7.80, side * 5.9),
                at(w, x, 7.80, 0.0),
                0.14,
                0.14,
                FRAME,
            );
            if !in_door {
                for y in [1.6, 3.3] {
                    box_at(m, w, vec3(x, y, side * 5.81), vec3(0.20, 0.085, 0.08), DARK);
                }
                // Folded troop seats hug the walls, outside the clear lane.
                box_at(
                    m,
                    w,
                    vec3(x, 1.55, side * 5.72),
                    vec3(0.65, 0.44, 0.10),
                    [0.24, 0.29, 0.22, 0.0],
                );
            }
        }
    }
    for side in [-1.0f32, 1.0] {
        pipe(
            m,
            at(w, -41.8, 6.8, side * 5.88),
            at(w, 41.8, 6.8, side * 5.88),
            0.055,
            DARK,
        );
        pipe(
            m,
            at(w, -41.8, 7.45, side * 4.65),
            at(w, 41.8, 7.45, side * 4.65),
            0.045,
            YELLOW,
        );
        for x in [-32.0f32, -10.0, 10.0, 32.0] {
            let g = wall(
                at(w, x, 5.5, side * 5.97),
                if side < 0.0 { 0.0 } else { PI },
            );
            stencil(m, &g, "EXIT", 0.13, [0.32, 0.56, 0.38, 0.18]);
        }
        // Torn edging has short triangular teeth above head height only.
        for x in layout::SIDE_DOORS {
            for k in 0..8 {
                let xx = x - 2.9 + k as f32 * 0.78;
                let y = 4.1 + hash01(k, x.abs() as u32) * 0.42;
                let z = side * 6.41;
                m.tri(
                    at(w, xx, 4.03, z),
                    at(w, xx + 0.5, 4.04, z),
                    at(w, xx + 0.2, y, z + side * 0.17),
                    LIGHT,
                );
                m.tri(
                    at(w, xx + 0.2, y, z + side * 0.17),
                    at(w, xx + 0.5, 4.04, z),
                    at(w, xx, 4.03, z),
                    RUST,
                );
            }
        }
    }
    // Longitudinal deck strips and flush tie-downs give a cargo-hold scale.
    for z in [-3.4f32, -1.75, 1.75, 3.4] {
        beam(
            m,
            at(w, -41.5, 0.035, z),
            at(w, 41.5, 0.035, z),
            0.045,
            0.02,
            DARK,
        );
        for x in -10..=10 {
            box_at(
                m,
                w,
                vec3(x as f32 * 4.0, 0.045, z),
                vec3(0.14, 0.015, 0.09),
                FRAME,
            );
        }
    }
    for side in [-1.0f32, 1.0] {
        let g = wall(
            at(w, 10.0, 5.6, side * 6.43),
            if side < 0.0 { PI } else { 0.0 },
        );
        stencil(m, &g, "FERRITE AIRLIFT", 0.145, WHITE);
        let g = wall(
            at(w, -31.0, 4.9, side * 6.43),
            if side < 0.0 { PI } else { 0.0 },
        );
        stencil(m, &g, "FG 042", 0.20, WHITE);
        // The long pale belly band is segmented around the broken doors.
        for (x0, x1) in [(-41.8, -23.1), (-16.9, -3.1), (3.1, 16.9), (23.1, 41.8)] {
            let p = |x, y| at(w, x, y, side * 6.44);
            m.quad(p(x0, 2.5), p(x1, 2.5), p(x1, 2.8), p(x0, 2.8), WHITE);
            m.quad(p(x0, 2.8), p(x1, 2.8), p(x1, 2.5), p(x0, 2.5), WHITE);
        }
    }
}

fn nose(m: &mut Mesh, w: &World) {
    // The lower nose tore away, leaving a clear ground-level entrance into
    // the hold. The surviving flight deck overhangs it like a broken brow.
    let rings = [
        (41.9, 6.42, 8.40, 11.70),
        (47.0, 5.1, 6.0, 10.8),
        (51.0, 3.4, 5.4, 9.0),
        (54.0, 0.8, 6.0, 7.7),
    ];
    for p in rings.windows(2) {
        for side in [-1.0f32, 1.0] {
            let (a, b) = (p[0], p[1]);
            plate(
                m,
                [
                    at(w, a.0, a.2, side * a.1),
                    at(w, b.0, b.2, side * b.1),
                    at(w, b.0, b.3, side * b.1 * 0.65),
                    at(w, a.0, a.3, side * a.1 * 0.65),
                ],
                0.1,
                shade(SKIN, 0.9),
            );
            plate(
                m,
                [
                    at(w, a.0, a.3, side * a.1 * 0.65),
                    at(w, b.0, b.3, side * b.1 * 0.65),
                    at(w, b.0, b.3, 0.0),
                    at(w, a.0, a.3, 0.0),
                ],
                0.1,
                LIGHT,
            );
        }
    }
    // Cockpit windows are steep, widely separated and visibly framed.
    for side in [-1.0f32, 1.0] {
        for k in 0..3 {
            let x0 = 45.2 + k as f32 * 1.75;
            let z0 = (4.6 - (x0 - 45.2) * 0.25) * side;
            plate(
                m,
                [
                    at(w, x0, 8.25, z0),
                    at(w, x0 + 1.45, 7.85, z0 - side * 0.45),
                    at(w, x0 + 1.45, 9.0, z0 - side * 1.1),
                    at(w, x0, 9.6, z0 - side * 0.65),
                ],
                0.035,
                [0.08, 0.16, 0.19, 0.08],
            );
        }
        beam(
            m,
            at(w, 42.0, 4.2, side * 5.2),
            at(w, 48.0, 6.4, side * 3.4),
            0.18,
            0.18,
            FRAME,
        );
        for k in 0..7 {
            let x = 42.0 + k as f32 * 0.8;
            let y = 5.3 + (x - 42.0) * 0.1;
            m.tri(
                at(w, x, y, side * 4.8),
                at(w, x + 0.5, y + 0.5, side * 4.7),
                at(w, x + 0.6, y - 0.2, side * 4.6),
                RUST,
            );
        }
    }
}

fn tail(m: &mut Mesh, w: &World) {
    // A tall, swept cargo-aircraft fin, distinctly different from the nose.
    for side in [-1.0f32, 1.0] {
        let z = side * 0.52;
        plate(
            m,
            [
                at(w, -42.0, 10.2, z),
                at(w, -41.0, 32.0, z * 0.45),
                at(w, -29.0, 28.0, z * 0.45),
                at(w, -25.0, 10.2, z),
            ],
            0.08,
            shade(SKIN, if side < 0.0 { 0.85 } else { 1.05 }),
        );
        let g = wall(
            at(w, -35.6, 22.8, z + side * 0.06),
            if side < 0.0 { PI } else { 0.0 },
        );
        stencil(m, &g, "042", 0.37, WHITE);
        let p = |x, y| at(w, x, y, z + side * 0.08);
        m.quad(
            p(-40.2, 27.5),
            p(-30.8, 24.3),
            p(-30.8, 25.3),
            p(-40.2, 28.5),
            YELLOW,
        );
        m.quad(
            p(-40.2, 28.5),
            p(-30.8, 25.3),
            p(-30.8, 24.3),
            p(-40.2, 27.5),
            YELLOW,
        );
        // Elevator span, ragged tip and a low remnant of the rear fuselage.
        plate(
            m,
            [
                at(w, -39.0, 15.0, side * 0.8),
                at(w, -42.5, 15.7, side * 17.5),
                at(w, -48.0, 15.0, side * 17.5),
                at(w, -44.0, 14.4, side * 0.8),
            ],
            0.35,
            SKIN,
        );
        beam(
            m,
            at(w, -41.2, 10.4, side * 0.58),
            at(w, -40.4, 30.2, side * 0.34),
            0.11,
            0.11,
            DARK,
        );
    }
    // The broken aft rim exposes longitudinal stringers rather than a cap
    // across the open ground-level cargo entrance.
    for side in [-1.0f32, 1.0] {
        for k in 0..6 {
            let y = 4.2 + k as f32 * 1.1;
            beam(
                m,
                at(w, -41.9, y, side * 6.20),
                at(
                    w,
                    -45.0 - hash01(k, 9) * 1.6,
                    y + 0.3,
                    side * (6.0 - k as f32 * 0.2),
                ),
                0.10,
                0.10,
                FRAME,
            );
        }
    }
}

fn engine(m: &mut Mesh, w: &World, x: f32, y: f32, z: f32, damaged: bool) {
    let g = Mat4::from_translation(at(w, x, y, z)) * Mat4::from_rotation_y(FRAC_PI_2);
    m.tube(&g, Vec3::ZERO, 2.15, 1.83, 3.7, 16, SKIN);
    m.tube(&g, vec3(0.0, 0.0, 3.55), 2.24, 1.82, 0.18, 16, LIGHT);
    m.tube(&g, vec3(0.0, 0.0, -3.7), 1.97, 1.3, 0.2, 16, DARK);
    let center = g.transform_point3(vec3(0.0, 0.0, 2.7));
    for k in 0..12 {
        let a = k as f32 * TAU / 12.0;
        let p = |rad: f32, angle: f32| {
            g.transform_point3(vec3(angle.cos() * rad, angle.sin() * rad, 2.7))
        };
        m.tri(center, p(1.7, a), p(1.65, a + 0.32), shade(FRAME, 0.75));
    }
    m.tube(&g, vec3(0.0, 0.0, 2.9), 0.45, 0.0, 0.50, 10, DARK);
    if damaged {
        for k in 0..5 {
            let a = k as f32 * 0.6;
            let p = |r: f32, zz: f32| g.transform_point3(vec3(a.cos() * r, a.sin() * r, zz));
            m.tri(p(2.2, 3.8), p(2.5, 5.0), p(1.95, 2.7), RUST);
        }
    }
}

fn wings(m: &mut Mesh, w: &World) {
    for side in [-1.0f32, 1.0] {
        // Both wings sweep back. The port tip is snapped and driven into
        // forest ground; the starboard wing still bridges a stand of pines.
        let tip_y = if side < 0.0 { 1.6 } else { 7.0 };
        let tip_z = if side < 0.0 { 42.0 } else { 46.0 };
        let p = |x, y, z| at(w, x, y, z * side);
        plate(
            m,
            [
                p(5.0, 9.7, 6.0),
                p(-21.0, tip_y, tip_z),
                p(-35.0, tip_y - 0.6, tip_z),
                p(-10.5, 8.5, 6.0),
            ],
            0.65,
            SKIN,
        );
        plate(
            m,
            [
                p(-10.0, 8.5, 7.0),
                p(-34.0, tip_y - 0.7, tip_z),
                p(-36.0, tip_y - 1.2, tip_z - 1.2),
                p(-12.8, 8.1, 7.0),
            ],
            0.20,
            shade(SKIN, 0.68),
        );
        for k in 1..5 {
            let t = k as f32 / 5.0;
            let z = 6.0 + (tip_z - 6.0) * t;
            let y = 9.1 + (tip_y - 9.1) * t;
            let x = 3.0 - 26.0 * t;
            beam(
                m,
                p(x, y + 0.06, z),
                p(x - 13.0, y - 0.6, z),
                0.065,
                0.065,
                FRAME,
            );
        }
        for (x, z, y) in [(-5.5, 15.0, 7.2), (-15.0, 26.0, 6.5)] {
            beam(
                m,
                p(x + 1.0, y + 1.1, z),
                p(x, y + 3.0, z),
                0.75,
                1.1,
                FRAME,
            );
            engine(m, w, x, y, z * side, side < 0.0 && z > 20.0);
        }
        // Faded broad wing stripe and interrupted trailing-edge hinges.
        plate(
            m,
            [
                p(-15.0, 8.9, 26.0),
                p(-16.4, 8.65, 28.0),
                p(-26.5, 8.1, 28.0),
                p(-25.2, 8.4, 26.0),
            ],
            0.025,
            YELLOW,
        );
        // Two snapped-off main landing legs remain folded beside the hull.
        let gear = at(w, -28.0, 2.1, side * 8.1);
        beam(
            m,
            gear,
            gear + vec3(3.5, -0.7, side * 1.5),
            0.45,
            0.45,
            FRAME,
        );
        let g = Mat4::from_translation(gear + vec3(3.2, -0.4, side * 1.5));
        m.tube(&g, Vec3::ZERO, 1.0, 0.3, 0.45, 12, DARK);
        m.tube(&g, Vec3::ZERO, 0.4, 0.0, 0.47, 10, FRAME);
    }
}

pub fn build(m: &mut Mesh, w: &World) {
    if w.map != layout::MAP_ID {
        return;
    }
    cargo_shell(m, w);
    nose(m, w);
    tail(m, w);
    wings(m, w);
}

/// The wind gust cycle is shared with the positional rattle's seven-second
/// cadence. No gameplay state is driven by this local ambient clock.
pub fn gust(time: f32) -> f32 {
    (time * TAU / 7.0).sin()
}

pub fn atmosphere(m: &mut Mesh, _alpha: &mut Mesh, add: &mut Mesh, w: &World, time: f32) {
    if w.map != layout::MAP_ID {
        return;
    }
    for (i, (x, y, z)) in [(12.0f32, 6.3f32, 6.52f32), (-11.0, 6.5, -6.52)]
        .into_iter()
        .enumerate()
    {
        let flap = gust(time + i as f32 * 1.4) * 0.12 + (time * 5.1 + i as f32).sin() * 0.025;
        let side = z.signum();
        let g = Mat4::from_translation(at(w, x, y, z)) * Mat4::from_rotation_x(flap * side);
        m.obox(
            &g,
            vec3(0.0, 0.70, side * 0.05),
            vec3(1.12, 0.70, 0.045),
            LIGHT,
        );
        for xx in [-0.8, 0.0, 0.8] {
            m.obox(
                &g,
                vec3(xx, 0.7, side * 0.10),
                vec3(0.025, 0.6, 0.025),
                FRAME,
            );
        }
    }
    // Two surviving emergency fixtures, including one deep in the hold.
    for x in [-12.0f32, 31.0] {
        let p = at(w, x, 6.5, -5.68);
        m.cuboid(p - vec3(0.28, 0.15, 0.18), p + vec3(0.28, 0.15, 0.18), DARK);
        let phase = (time * 1.6 + x * 0.02).rem_euclid(1.0);
        let flash = if phase < 0.12 || (0.22..0.32).contains(&phase) {
            1.0
        } else {
            0.08
        };
        let col = [1.0, 0.14, 0.045, flash * 0.9];
        add.cuboid(p - vec3(0.12, 0.10, 0.20), p + vec3(0.12, 0.10, 0.20), col);
        if flash > 0.5 {
            cross(add, p, 0.65, 0.65, [1.0, 0.12, 0.035, 0.48]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn valid(mesh: Mesh, budget: usize) {
        assert!(mesh.triangles() < budget, "{} triangles, budget {budget}", mesh.triangles());
        for (v, i) in mesh.finish() {
            assert!(i.iter().all(|&n| (n as usize) < v.len()));
            assert!(v.iter().all(|v| v
                .pos
                .iter()
                .chain(v.nrm.iter())
                .chain(v.col.iter())
                .all(|f| f.is_finite())));
        }
    }
    #[test]
    fn cargo_aircraft_has_finite_bounded_silhouette_and_moving_ambient_parts() {
        for seed in [0, 17, 0xFE221, u64::MAX] {
            let w = World::with_map(seed, layout::MAP_ID);
            let mut m = Mesh::default();
            build(&mut m, &w);
            assert!(m.triangles() > 2500);
            assert!(m.v.iter().any(|v| v.pos[1] > layout::floor(&w) + 30.0));
            valid(m, 28000);
            let mut poses = Vec::new();
            for t in [0.0, 0.4, 2.3, 10000.0] {
                let (mut m, mut alpha, mut add) =
                    (Mesh::default(), Mesh::default(), Mesh::default());
                atmosphere(&mut m, &mut alpha, &mut add, &w, t);
                poses.push(m.v.iter().map(|v| v.pos).collect::<Vec<_>>());
                valid(m, 500);
                valid(alpha, 100);
                valid(add, 500);
            }
            assert_ne!(poses[0], poses[1]);
        }
    }
}
