//! Slag Cathedral: cut steel, refractory masonry, suspended heat and ash.
//! Large footprints come from shared collision; added details are cladding,
//! surface markings, thin rails or machinery well above the playable routes.
use super::{hash01, mix3, smooth};
use crate::gfx::{rgb, shade, Col, Light, Mesh};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::{slag_cathedral as layout, vnoise, Block, BlockKind, World, SPREAD};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

#[path = "slag_cathedral_art.rs"]
pub(crate) mod art;

pub const ID: u8 = 6;
pub const FOG: [f32; 4] = [0.235, 0.205, 0.235, 0.0040];
pub const SUN: Vec3 = Vec3::new(0.56, 0.40, 0.34);
const STEEL: Col = [0.23, 0.255, 0.28, 0.0];
const EDGE: Col = [0.40, 0.435, 0.45, 0.0];
const DARK: Col = [0.095, 0.11, 0.13, 0.0];
const RUST: Col = [0.47, 0.245, 0.15, 0.0];
const BRASS: Col = [0.69, 0.46, 0.21, 0.0];
const HOT: Col = [1.0, 0.29, 0.045, 1.0];
const WHITE_HOT: Col = [1.0, 0.73, 0.25, 1.0];
const GLASS: Col = [0.16, 0.42, 0.43, 0.18];
const WARNING: Col = [0.83, 0.54, 0.16, 0.0];

pub(crate) fn basis(a: Vec3, b: Vec3) -> (Mat4, f32) {
    let d = b - a;
    let len = d.length().max(0.0001);
    let z = d / len;
    let guide = if z.y.abs() > 0.95 { Vec3::X } else { Vec3::Y };
    let x = guide.cross(z).normalize_or_zero();
    let y = z.cross(x);
    (
        Mat4::from_cols(
            x.extend(0.0),
            y.extend(0.0),
            z.extend(0.0),
            ((a + b) * 0.5).extend(1.0),
        ),
        len,
    )
}
pub(crate) fn beam(m: &mut Mesh, a: Vec3, b: Vec3, width: f32, depth: f32, col: Col) {
    let (g, len) = basis(a, b);
    m.obox(
        &g,
        Vec3::ZERO,
        vec3(width * 0.5, depth * 0.5, len * 0.5),
        col,
    );
}
pub(crate) fn girder(m: &mut Mesh, a: Vec3, b: Vec3, width: f32, depth: f32, col: Col) {
    let (g, len) = basis(a, b);
    m.obox(
        &g,
        Vec3::ZERO,
        vec3(width * 0.09, depth * 0.5, len * 0.5),
        shade(col, 0.8),
    );
    for sign in [-1.0f32, 1.0] {
        m.obox(
            &g,
            vec3(0.0, sign * depth * 0.43, 0.0),
            vec3(width * 0.5, depth * 0.07, len * 0.5),
            col,
        );
    }
}
pub(crate) fn pipe(m: &mut Mesh, a: Vec3, b: Vec3, radius: f32, col: Col) {
    let (g, len) = basis(a, b);
    m.tube(&g, Vec3::ZERO, radius, 0.0, len * 0.5, 12, col);
}
fn flange(m: &mut Mesh, c: Vec3, axis: Vec3, radius: f32) {
    let (g, _) = basis(c - axis * 0.1, c + axis * 0.1);
    m.tube(&g, Vec3::ZERO, radius * 1.35, 0.0, 0.095, 12, EDGE);
    m.tube(
        &g,
        Vec3::ZERO,
        radius * 1.41,
        radius * 1.08,
        0.025,
        12,
        DARK,
    );
    for k in 0..8 {
        let a = k as f32 * TAU / 8.0;
        m.tube(
            &g,
            vec3(a.cos(), a.sin(), 0.0) * (radius * 1.17),
            0.055,
            0.0,
            0.125,
            6,
            BRASS,
        );
    }
}
fn pipeline(m: &mut Mesh, points: &[Vec3], radius: f32, col: Col) {
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        pipe(m, a, b, radius, col);
        let axis = (b - a).normalize();
        let n = ((b - a).length() / 7.0).ceil().max(1.0) as usize;
        for k in 0..=n {
            flange(m, a.lerp(b, k as f32 / n as f32), axis, radius);
        }
    }
}
fn wheel(m: &mut Mesh, g: Mat4, radius: f32) {
    m.tube(&g, Vec3::ZERO, radius, radius * 0.84, 0.05, 12, WARNING);
    m.tube(&g, Vec3::ZERO, radius * 0.2, 0.0, 0.10, 8, BRASS);
    for k in 0..5 {
        let a = k as f32 * TAU / 5.0;
        beam(
            m,
            g.transform_point3(Vec3::ZERO),
            g.transform_point3(vec3(a.cos(), a.sin(), 0.0) * radius * 0.9),
            0.055,
            0.055,
            WARNING,
        );
    }
}

pub fn ground(w: &World, c: Vec3, n: Vec3, jitter: f32) -> Col {
    let ash = vnoise(c.x / 16.0, c.z / 16.0, w.seed as u32 ^ 0x51A6);
    let col = mix3(
        [0.20, 0.205, 0.22],
        [0.36, 0.29, 0.255],
        smooth(0.25, 0.85, ash),
    );
    let col = mix3(col, [0.28, 0.275, 0.30], smooth(0.02, 0.2, 1.0 - n.y));
    let col = art::ground_tint(col, c);
    let wear = if c.z.abs() < 26.0 * SPREAD && c.x.abs() < 91.0 * SPREAD {
        1.04
    } else {
        0.91
    };
    let k = (0.9 + 0.18 * jitter) * wear;
    rgb(col[0] * k, col[1] * k, col[2] * k)
}

/// Cladding stays inside each authoritative box, with tiny surface seams.
pub fn block(m: &mut Mesh, b: &Block, index: usize) {
    if art::owns(b) {
        return;
    }
    let size = b.max - b.min;
    let center = (b.min + b.max) * 0.5;
    let j = 0.91 + hash01(index as u32, 0xAC) * 0.14;
    let base = match b.kind {
        BlockKind::Crate | BlockKind::Trunk => return,
        BlockKind::Concrete => rgb(0.37 * j, 0.345 * j, 0.32 * j),
        BlockKind::Rock => rgb(0.285 * j, 0.27 * j, 0.29 * j),
        BlockKind::Step => shade(STEEL, 1.12 * j),
        BlockKind::Sandbag => rgb(0.39 * j, 0.35 * j, 0.28 * j),
        BlockKind::Container => shade(if b.tint % 2 == 0 { RUST } else { STEEL }, j),
        BlockKind::Metal => shade(STEEL, j * (0.92 + (b.tint % 3) as f32 * 0.06)),
    };
    m.cuboid(b.min, b.max, base);
    if size.y < 0.5 {
        // Gallery floors and stairs: antislip steel with a legible tread edge.
        let y = b.max.y + 0.012;
        let n = (size.x / 0.55).ceil().max(1.0) as usize;
        for k in 0..n {
            let x = b.min.x + size.x * (k as f32 + 0.5) / n as f32;
            m.cuboid(
                vec3(x - 0.018, y, b.min.z + 0.03),
                vec3(x + 0.018, y + 0.009, b.max.z - 0.03),
                DARK,
            );
        }
        return;
    }
    let e = 0.055f32.min(size.x * 0.1).min(size.z * 0.1);
    // Corner posts stand proud of the faces, like the seams below: flush
    // with them they z-fight the block and flicker as the view moves.
    let o = 0.012;
    for (x0, x1) in [(b.min.x - o, b.min.x + 2.0 * e), (b.max.x - 2.0 * e, b.max.x + o)] {
        for (z0, z1) in [(b.min.z - o, b.min.z + 2.0 * e), (b.max.z - 2.0 * e, b.max.z + o)] {
            m.cuboid(
                vec3(x0, b.min.y + 0.03, z0),
                vec3(x1, b.max.y - 0.03, z1),
                shade(base, 0.73),
            );
        }
    }
    if size.y < 8.0 && size.x > 1.0 && size.z > 1.0 {
        // Refractory courses or welded panel seams, with solid silhouettes.
        let levels = (size.y / 0.65).floor() as usize;
        for k in 1..=levels {
            let y = b.min.y + k as f32 * size.y / (levels + 1) as f32;
            m.cuboid(
                vec3(b.min.x + 0.012, y, b.min.z - 0.012),
                vec3(b.max.x - 0.012, y + 0.035, b.min.z + 0.018),
                DARK,
            );
            m.cuboid(
                vec3(b.min.x + 0.012, y, b.max.z - 0.018),
                vec3(b.max.x - 0.012, y + 0.035, b.max.z + 0.012),
                DARK,
            );
        }
        let y = center.y;
        for sign in [-1.0f32, 1.0] {
            let z = if sign < 0.0 {
                b.min.z - 0.015
            } else {
                b.max.z + 0.015
            };
            for x in [b.min.x + size.x * 0.15, b.max.x - size.x * 0.15] {
                m.cuboid(
                    vec3(x - 0.045, y - 0.045, z - 0.01),
                    vec3(x + 0.045, y + 0.045, z + 0.01),
                    BRASS,
                );
            }
        }
    }
}

fn lancet(m: &mut Mesh, g: Mat4, width: f32, height: f32, glass: Col) {
    let p = [
        vec3(-width * 0.5, 0.0, 0.0),
        vec3(width * 0.5, 0.0, 0.0),
        vec3(width * 0.5, height * 0.65, 0.0),
        vec3(width * 0.28, height * 0.87, 0.0),
        vec3(0.0, height, 0.0),
        vec3(-width * 0.28, height * 0.87, 0.0),
        vec3(-width * 0.5, height * 0.65, 0.0),
    ];
    let center = g.transform_point3(vec3(0.0, height * 0.45, -0.015));
    for k in 0..p.len() {
        let a = g.transform_point3(p[k]);
        let b = g.transform_point3(p[(k + 1) % p.len()]);
        // Dark leading; the coloured panes are set just in front of it.
        m.tri(center, a, b, shade(DARK, 0.7));
        beam(m, a, b, 0.17, 0.15, EDGE);
    }
    art::glazing(m, &g, width, height, glass[0] > glass[2]);
    for fraction in [0.18f32, 0.40, 0.63] {
        beam(
            m,
            g.transform_point3(vec3(-width * 0.48, height * fraction, 0.03)),
            g.transform_point3(vec3(width * 0.48, height * fraction, 0.03)),
            0.11,
            0.12,
            STEEL,
        );
    }
    for x in [-width * 0.17, width * 0.17] {
        beam(
            m,
            g.transform_point3(vec3(x, 0.0, 0.04)),
            g.transform_point3(vec3(x, height * 0.88, 0.04)),
            0.095,
            0.12,
            BRASS,
        );
    }
    // An industrial diamond instead of religious iconography.
    let cy = height * 0.77;
    for (a, b) in [
        (
            vec3(-width * 0.2, cy, 0.05),
            vec3(0.0, cy + width * 0.22, 0.05),
        ),
        (
            vec3(0.0, cy + width * 0.22, 0.05),
            vec3(width * 0.2, cy, 0.05),
        ),
        (
            vec3(width * 0.2, cy, 0.05),
            vec3(0.0, cy - width * 0.22, 0.05),
        ),
        (
            vec3(0.0, cy - width * 0.22, 0.05),
            vec3(-width * 0.2, cy, 0.05),
        ),
    ] {
        beam(
            m,
            g.transform_point3(a),
            g.transform_point3(b),
            0.11,
            0.12,
            WARNING,
        );
    }
}

pub(crate) fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 14],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'N' => [17, 25, 25, 21, 19, 19, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        'J' => [7, 2, 2, 2, 2, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'W' => [17, 17, 17, 21, 21, 27, 17],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 30, 1, 1, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        _ => [0; 7],
    }
}
fn sign(m: &mut Mesh, g: Mat4, text: &str, pixel: f32) {
    let width = text.len() as f32 * pixel * 6.0;
    m.obox(
        &g,
        vec3(0.0, 3.5 * pixel, -0.12),
        vec3(width * 0.5 + pixel, 4.5 * pixel, 0.12),
        DARK,
    );
    for (i, c) in text.chars().enumerate() {
        for (row, bits) in glyph(c).into_iter().enumerate() {
            for bit in 0..5 {
                if bits & (1 << (4 - bit)) != 0 {
                    m.obox(
                        &g,
                        vec3(
                            -width * 0.5 + (i as f32 * 6.0 + bit as f32 + 0.5) * pixel,
                            (6.5 - row as f32) * pixel,
                            0.018,
                        ),
                        vec3(pixel * 0.42, pixel * 0.42, 0.018),
                        [0.75, 0.79, 0.76, 0.04],
                    );
                }
            }
        }
    }
    m.obox(
        &g,
        vec3(0.0, -pixel * 0.4, 0.015),
        vec3(width * 0.5, pixel * 0.13, 0.02),
        WARNING,
    );
}

fn nave(m: &mut Mesh) {
    let eave = layout::NAVE_EAVE_Y;
    let ridge = layout::NAVE_RIDGE_Y;
    let arch = [
        (-18.0, eave),
        (-15.0, eave + 1.2),
        (-11.0, ridge - 2.3),
        (-6.0, ridge - 0.65),
        (0.0, ridge),
        (6.0, ridge - 0.65),
        (11.0, ridge - 2.3),
        (15.0, eave + 1.2),
        (18.0, eave),
    ];
    for &x in &layout::NAVE_RIB_X {
        for side in [-1.0f32, 1.0] {
            let z = side * layout::NAVE_RIB_Z;
            // Riveted column plates sit on the authoritative steel column.
            for y in [2.0f32, 6.0, 10.0, 14.0, 18.0, 22.0] {
                m.cuboid(
                    vec3(x - 0.79, y - 0.15, z - 0.89),
                    vec3(x + 0.79, y + 0.15, z + 0.89),
                    RUST,
                );
                for dx in [-0.61f32, 0.61] {
                    m.cuboid(
                        vec3(x + dx - 0.065, y - 0.065, z - side * 0.925 - 0.025),
                        vec3(x + dx + 0.065, y + 0.065, z - side * 0.925 + 0.025),
                        BRASS,
                    );
                }
            }
            girder(
                m,
                vec3(x, 8.5, z),
                vec3(x, 14.5, z - side * 3.2),
                0.45,
                0.65,
                EDGE,
            );
            girder(
                m,
                vec3(x, 19.0, z),
                vec3(x, 23.8, z - side * 4.4),
                0.5,
                0.75,
                STEEL,
            );
            m.cuboid(
                vec3(x - 1.35, 0.12, z - 1.45),
                vec3(x + 1.35, 0.28, z + 1.45),
                DARK,
            );
            m.cuboid(
                vec3(x - 1.0, 1.08, z - 1.0),
                vec3(x + 1.0, 1.18, z + 1.0),
                EDGE,
            );
        }
        for pair in arch.windows(2) {
            girder(
                m,
                vec3(x, pair[0].1, pair[0].0),
                vec3(x, pair[1].1, pair[1].0),
                0.65,
                1.0,
                EDGE,
            );
            beam(
                m,
                vec3(x, pair[0].1 - 0.75, pair[0].0),
                vec3(x, pair[1].1 - 0.75, pair[1].0),
                0.11,
                0.2,
                RUST,
            );
        }
        // Suspended central ridge keys and a vertical web emphasize the vault.
        girder(
            m,
            vec3(x, ridge - 3.1, 0.0),
            vec3(x, ridge + 0.25, 0.0),
            0.28,
            0.35,
            BRASS,
        );
        for sign in [-1.0f32, 1.0] {
            beam(
                m,
                vec3(x, ridge - 3.1, 0.0),
                vec3(x, ridge - 1.3, sign * 7.0),
                0.13,
                0.2,
                STEEL,
            );
        }
    }
    for &(z, y) in &arch {
        girder(
            m,
            vec3(-34.0, y + 0.3, z),
            vec3(34.0, y + 0.3, z),
            0.32,
            0.42,
            STEEL,
        );
    }
    // Corrugated roof strips deliberately leave a long ridge lantern open.
    for side in [-1.0f32, 1.0] {
        for bay in 0..5 {
            let x0 = layout::NAVE_RIB_X[bay];
            let x1 = layout::NAVE_RIB_X[bay + 1];
            for j in 0..3 {
                let (z0, y0) = arch[j];
                let (z1, y1) = arch[j + 1];
                let a = vec3(x0, y0 + 0.35, z0 * side);
                let b = vec3(x1, y0 + 0.35, z0 * side);
                let c = vec3(x1, y1 + 0.35, z1 * side);
                let d = vec3(x0, y1 + 0.35, z1 * side);
                m.quad(a, b, c, d, shade(STEEL, 0.78 + bay as f32 * 0.045));
                m.quad(d, c, b, a, shade(DARK, 1.18));
                for k in 1..6 {
                    let t = k as f32 / 6.0;
                    beam(m, a.lerp(b, t), d.lerp(c, t), 0.05, 0.07, EDGE);
                }
            }
            // Diagonal bracing in the clerestory, all safely overhead.
            let z = 18.0 * side;
            beam(m, vec3(x0, 17.0, z), vec3(x1, 23.0, z), 0.18, 0.26, RUST);
            beam(m, vec3(x0, 23.0, z), vec3(x1, 17.0, z), 0.18, 0.26, RUST);
            for exterior in [false, true] {
                let face_z = side * if exterior { 22.64 } else { 21.36 };
                let facing = if exterior { side } else { -side };
                let g = Mat4::from_translation(vec3((x0 + x1) * 0.5, 10.8, face_z))
                    * Mat4::from_rotation_y(if facing > 0.0 { 0.0 } else { PI });
                lancet(
                    m,
                    g,
                    5.0,
                    12.0,
                    if bay % 2 == 0 {
                        GLASS
                    } else {
                        [0.53, 0.31, 0.13, 0.12]
                    },
                );
            }
        }
    }
    // Surface-mounted pilasters and cornices articulate the solid wall
    // headers. Both facade and nave faces remain backed by collision.
    for side in [-1.0f32, 1.0] {
        for exterior in [false, true] {
            let z = side * if exterior { 22.70 } else { 21.30 };
            for y in [9.0f32, 16.0, 24.0] {
                beam(m, vec3(-34.0, y, z), vec3(34.0, y, z), 0.20, 0.24, EDGE);
            }
            for x in layout::NAVE_RIB_X {
                m.cuboid(
                    vec3(x - 0.25, 9.0, z - 0.13),
                    vec3(x + 0.25, 24.0, z + 0.13),
                    STEEL,
                );
                for y in [9.3f32, 16.0, 23.7] {
                    m.cuboid(
                        vec3(x - 0.52, y - 0.16, z - 0.17),
                        vec3(x + 0.52, y + 0.16, z + 0.17),
                        RUST,
                    );
                }
            }
        }
    }
    // Two open portal gables, with a rose-shaped pressure-gauge lantern.
    for side in [-1.0f32, 1.0] {
        let x = side * 30.0;
        let g =
            Mat4::from_translation(vec3(x, 25.4, 0.0)) * Mat4::from_rotation_y(side * FRAC_PI_2);
        m.tube(&g, Vec3::ZERO, 2.15, 1.92, 0.18, 24, EDGE);
        m.tube(&g, Vec3::ZERO, 1.88, 0.0, 0.025, 16, GLASS);
        for k in 0..8 {
            let a = k as f32 * TAU / 8.0;
            beam(
                m,
                g.transform_point3(Vec3::ZERO),
                g.transform_point3(vec3(a.cos(), a.sin(), 0.0) * 1.9),
                0.1,
                0.16,
                BRASS,
            );
        }
        m.tube(&g, vec3(0.0, 0.0, 0.2), 0.38, 0.20, 0.1, 8, WARNING);
        let sign_g = Mat4::from_translation(vec3(x + side * 0.5, 10.8, 0.0))
            * Mat4::from_rotation_y(side * FRAC_PI_2);
        sign(m, sign_g, "SLAG CATHEDRAL", 0.27);
        let inside_sign = Mat4::from_translation(vec3(x + side * 0.24, 10.8, 0.0))
            * Mat4::from_rotation_y(-side * FRAC_PI_2);
        sign(m, inside_sign, "SLAG CATHEDRAL", 0.27);
        for z in [-12.0f32, -6.0, 6.0, 12.0] {
            beam(
                m,
                vec3(x, 12.9, z),
                vec3(x, 23.3, z * 0.6),
                0.14,
                0.22,
                STEEL,
            );
        }
    }
}

fn galleries(m: &mut Mesh) {
    for side in [-1.0f32, 1.0] {
        let z = side * layout::GALLERY_Z;
        let y = layout::GALLERY_Y;
        for outer in [false, true] {
            let rz = side
                * if outer {
                    layout::GALLERY_RAIL_OUTER_Z
                } else {
                    layout::GALLERY_RAIL_INNER_Z
                };
            let outer_span = [(-layout::GALLERY_HALF_X, layout::GALLERY_HALF_X)];
            let spans: &[(f32, f32)] = if outer {
                &outer_span
            } else {
                &layout::GALLERY_INNER_RAIL_SPANS
            };
            for &(x0, x1) in spans {
                for height in layout::GALLERY_RAIL_HEIGHTS {
                    beam(
                        m,
                        vec3(x0, height, rz),
                        vec3(x1, height, rz),
                        0.12,
                        0.12,
                        if height > y + 0.75 { WARNING } else { STEEL },
                    );
                }
                let posts = ((x1 - x0) / 3.0).ceil() as usize;
                for k in 0..=posts {
                    let x = x0 + (x1 - x0) * k as f32 / posts as f32;
                    beam(m, vec3(x, y, rz), vec3(x, y + 1.1, rz), 0.055, 0.055, EDGE);
                }
            }
        }
        // Galvanized grate crosspieces and long bearing strips at the same top.
        for k in -51..=51 {
            let x = k as f32 * 0.5;
            m.cuboid(
                vec3(x - 0.035, y + 0.013, z - 1.80),
                vec3(x + 0.035, y + 0.024, z + 1.80),
                DARK,
            );
        }
        for offset in [-1.3f32, -0.65, 0.0, 0.65, 1.3] {
            beam(
                m,
                vec3(-26.0, y + 0.02, z + offset),
                vec3(26.0, y + 0.02, z + offset),
                0.035,
                0.018,
                EDGE,
            );
        }
        for end in [-1.0f32, 1.0] {
            for edge in [-1.0f32, 1.0] {
                let rz = z + edge * (layout::STAIR_HALF_Z + 0.02);
                beam(
                    m,
                    vec3(end * layout::STAIR_START_X, 1.05, rz),
                    vec3(end * layout::GALLERY_HALF_X, y + 1.05, rz),
                    0.055,
                    0.055,
                    WARNING,
                );
                for k in 0..=4 {
                    let t = k as f32 / 4.0;
                    let x = end
                        * (layout::STAIR_START_X
                            + (layout::GALLERY_HALF_X - layout::STAIR_START_X) * t);
                    beam(
                        m,
                        vec3(x, y * t, rz),
                        vec3(x, y * t + 1.05, rz),
                        0.055,
                        0.055,
                        STEEL,
                    );
                }
            }
        }
    }
}

fn furnace(m: &mut Mesh, x: f32, z: f32, index: usize) {
    let h = layout::FURNACE_HEIGHT;
    // Shaft faces retain the square collision envelope. Hot recessed louvres,
    // plate overlaps and massive collars break up its uninterrupted verticals.
    for y in [7.4f32, 14.0, 21.0, 28.0, 35.0] {
        for side in [-1.0f32, 1.0] {
            m.cuboid(
                vec3(x - 4.22, y - 0.14, z + side * 4.1 - 0.14),
                vec3(x + 4.22, y + 0.14, z + side * 4.1 + 0.14),
                EDGE,
            );
            m.cuboid(
                vec3(x + side * 4.1 - 0.14, y - 0.14, z - 4.22),
                vec3(x + side * 4.1 + 0.14, y + 0.14, z + 4.22),
                EDGE,
            );
        }
    }
    for face in 0..4 {
        let angle = face as f32 * FRAC_PI_2;
        let n = vec3(angle.sin(), 0.0, angle.cos());
        let g = Mat4::from_translation(vec3(x, 0.0, z) + n * 4.115) * Mat4::from_rotation_y(angle);
        for y in [10.5f32, 17.5, 24.5, 31.5] {
            m.obox(&g, vec3(0.0, y, 0.005), vec3(2.35, 1.35, 0.05), DARK);
            m.obox(&g, vec3(0.0, y, 0.065), vec3(2.1, 1.1, 0.015), HOT);
            for k in -2..=2 {
                let panel = g
                    * Mat4::from_translation(vec3(0.0, y + k as f32 * 0.42, 0.14))
                    * Mat4::from_rotation_x(-0.3);
                m.obox(&panel, Vec3::ZERO, vec3(2.22, 0.095, 0.16), STEEL);
            }
            for sx in [-2.38f32, 2.38] {
                m.obox(&g, vec3(sx, y, 0.13), vec3(0.11, 1.42, 0.1), RUST);
            }
        }
        // Refractory plinth vents are only a skin on its authoritative base.
        for k in -2..=2 {
            let at =
                Mat4::from_translation(vec3(x, 2.0, z) + n * 4.815) * Mat4::from_rotation_y(angle);
            m.obox(
                &at,
                vec3(k as f32 * 1.25, 0.0, 0.0),
                vec3(0.40, 0.92, 0.025),
                DARK,
            );
            m.obox(
                &at,
                vec3(k as f32 * 1.25, 0.0, 0.035),
                vec3(0.10, 0.79, 0.015),
                HOT,
            );
        }
    }
    // A square maintenance crown and four flared exhaust stacks.
    for side in [-1.0f32, 1.0] {
        beam(
            m,
            vec3(x - 4.45, h + 0.3, z + side * 4.45),
            vec3(x + 4.45, h + 0.3, z + side * 4.45),
            0.42,
            0.42,
            STEEL,
        );
        beam(
            m,
            vec3(x + side * 4.45, h + 0.3, z - 4.45),
            vec3(x + side * 4.45, h + 0.3, z + 4.45),
            0.42,
            0.42,
            STEEL,
        );
    }
    for (dx, dz) in [(-2.3f32, -2.3), (2.3, -2.3), (-2.3, 2.3), (2.3, 2.3)] {
        let g = Mat4::from_translation(vec3(x + dx, h + 1.65, z + dz))
            * Mat4::from_rotation_x(FRAC_PI_2);
        m.tube(&g, Vec3::ZERO, 0.87, 0.65, 1.65, 12, DARK);
        m.tube(&g, vec3(0.0, 0.0, -1.7), 1.05, 0.65, 0.18, 12, RUST);
        m.tube(&g, vec3(0.0, 0.0, -1.9), 0.64, 0.0, 0.025, 12, HOT);
    }
    let plaque = Mat4::from_translation(vec3(x, 5.0, z - 4.84)) * Mat4::from_rotation_y(PI);
    sign(
        m,
        plaque,
        if index == 0 {
            "FURNACE 01"
        } else {
            "FURNACE 02"
        },
        0.105,
    );
    pipeline(
        m,
        &[
            vec3(x - 3.8, 8.3, z),
            vec3(x - 3.8, 12.5, z),
            vec3(x + 3.8, 12.5, z),
        ],
        0.28,
        RUST,
    );
}

fn crucible(m: &mut Mesh) {
    // Crane rails run the length of the nave; a twin-girder bridge spans it.
    for z in [-9.5f32, 9.5] {
        girder(
            m,
            vec3(-29.0, 21.0, z),
            vec3(29.0, 21.0, z),
            0.55,
            0.9,
            RUST,
        );
        for x in [-24.0f32, -12.0, 0.0, 12.0, 24.0] {
            beam(m, vec3(x, 21.0, z), vec3(x, 25.0, z), 0.13, 0.15, DARK);
        }
    }
    for x in [-1.35f32, 1.35] {
        girder(
            m,
            vec3(x, 20.4, -11.0),
            vec3(x, 20.4, 11.0),
            0.70,
            1.15,
            WARNING,
        );
        for z in [-10.2f32, 10.2] {
            pipe(
                m,
                vec3(x - 0.5, 20.1, z),
                vec3(x + 0.5, 20.1, z),
                0.43,
                DARK,
            );
        }
    }
    m.cuboid(vec3(-1.9, 19.5, -1.6), vec3(1.9, 20.5, 1.6), STEEL);
    sign(
        m,
        Mat4::from_translation(vec3(0.0, 19.0, -1.7)) * Mat4::from_rotation_y(PI),
        "CRUCIBLE 06",
        0.085,
    );
    for x in [-3.8f32, 3.8] {
        pipe(m, vec3(x, 13.5, 0.0), vec3(x, 19.65, 0.0), 0.065, DARK);
        pipe(m, vec3(x, 13.5, -0.12), vec3(x, 19.65, -0.12), 0.065, EDGE);
        pipe(
            m,
            vec3(x - 0.35, 13.5, 0.0),
            vec3(x + 0.35, 13.5, 0.0),
            0.30,
            RUST,
        );
        beam(
            m,
            vec3(x, 13.5, 0.0),
            vec3(x.signum() * 2.9, 12.0, 0.0),
            0.45,
            0.50,
            STEEL,
        );
    }
    const N: usize = 16;
    let at = |i: usize, r: f32, y: f32| {
        let a = i as f32 * TAU / N as f32;
        vec3(a.cos() * r, y, a.sin() * r)
    };
    for k in 0..N {
        let j = 0.84 + hash01(k as u32, 19) * 0.2;
        m.quad(
            at(k, 1.8, 8.7),
            at(k, 3.3, 13.6),
            at(k + 1, 3.3, 13.6),
            at(k + 1, 1.8, 8.7),
            shade(STEEL, j),
        );
        m.quad(
            at(k, 3.3, 13.6),
            at(k, 2.94, 13.55),
            at(k + 1, 2.94, 13.55),
            at(k + 1, 3.3, 13.6),
            EDGE,
        );
        m.quad(
            at(k, 2.94, 13.55),
            at(k, 2.4, 11.7),
            at(k + 1, 2.4, 11.7),
            at(k + 1, 2.94, 13.55),
            DARK,
        );
        m.tri(
            vec3(0.0, 12.45, 0.0),
            at(k + 1, 2.75, 12.45),
            at(k, 2.75, 12.45),
            if k % 3 == 0 { WHITE_HOT } else { HOT },
        );
        if k % 2 == 0 {
            beam(m, at(k, 1.82, 8.8), at(k, 3.33, 13.45), 0.11, 0.13, RUST);
        }
    }
    for y in [9.6f32, 11.2, 13.3] {
        let r = 1.8 + (y - 8.7) / 4.9 * 1.5;
        for k in 0..N {
            beam(
                m,
                at(k, r + 0.055, y),
                at(k + 1, r + 0.055, y),
                0.14,
                0.22,
                RUST,
            );
        }
    }
    // Baked warm bounce below the suspended vessel, visibly a floor stain.
    for k in 0..20 {
        let a = k as f32 * TAU / 20.0;
        let b = (k + 1) as f32 * TAU / 20.0;
        m.tri(
            vec3(0.0, 0.018, 0.0),
            vec3(b.cos() * 4.2, 0.018, b.sin() * 4.2),
            vec3(a.cos() * 4.2, 0.018, a.sin() * 4.2),
            [0.33, 0.19, 0.12, 0.12],
        );
    }
}

fn floor_and_services(m: &mut Mesh, w: &World) {
    for ix in -17i32..17 {
        for iz in -7i32..7 {
            let x = ix as f32 * 2.0;
            let z = iz as f32 * 3.0;
            let c = shade(
                [0.23, 0.235, 0.25, 0.0],
                0.87 + hash01((ix + 17) as u32, (iz + 7) as u32) * 0.2,
            );
            m.quad(
                vec3(x + 0.025, 0.010, z + 0.025),
                vec3(x + 0.025, 0.010, z + 2.975),
                vec3(x + 1.975, 0.010, z + 2.975),
                vec3(x + 1.975, 0.010, z + 0.025),
                c,
            );
        }
    }
    // Open lava channels: the molten spans burn, the grated bridges between
    // them do not. Every footprint is read from the shared hazard layout.
    let hw = layout::LAVA_HALF_W;
    let spans = layout::LAVA_SPANS;
    let (x0, x1) = (spans[0].0, spans[spans.len() - 1].1);
    for z in [-layout::LAVA_Z, layout::LAVA_Z] {
        // Scorched refractory bed and low kerbs along the whole run.
        m.cuboid(
            vec3(x0 - 0.3, 0.012, z - hw - 0.3),
            vec3(x1 + 0.3, 0.020, z + hw + 0.3),
            DARK,
        );
        for side in [-1.0f32, 1.0] {
            let kz = z + side * (hw + 0.11);
            m.cuboid(
                vec3(x0 - 0.3, 0.020, kz - 0.11),
                vec3(x1 + 0.3, 0.075, kz + 0.11),
                shade(RUST, 0.62),
            );
            m.cuboid(
                vec3(x0 - 0.3, 0.075, kz - 0.035),
                vec3(x1 + 0.3, 0.082, kz + 0.035),
                WARNING,
            );
        }
        for (s, &(a, b)) in spans.iter().enumerate() {
            // Molten surface: a crusted edge, a hot body and a brighter core.
            m.cuboid(
                vec3(a, 0.020, z - hw),
                vec3(b, 0.034, z + hw),
                shade(HOT, 0.55),
            );
            let n = ((b - a) / 0.9).ceil() as usize;
            for k in 0..n {
                let (p, q) = (
                    a + (b - a) * k as f32 / n as f32,
                    a + (b - a) * (k + 1) as f32 / n as f32,
                );
                let j = hash01(k as u32, s as u32 * 7 + (z > 0.0) as u32);
                let (w0, w1) = (hw * (0.62 + j * 0.25), hw * (0.18 + j * 0.2));
                let drift = (j - 0.5) * 0.18;
                m.quad(
                    vec3(p, 0.036, z + drift - w0),
                    vec3(p, 0.036, z + drift + w0),
                    vec3(q, 0.036, z + drift + w0),
                    vec3(q, 0.036, z + drift - w0),
                    shade(HOT, 0.86 + j * 0.14),
                );
                if k % 3 != 2 {
                    m.quad(
                        vec3(p, 0.038, z - drift - w1),
                        vec3(p, 0.038, z - drift + w1),
                        vec3(q, 0.038, z - drift + w1),
                        vec3(q, 0.038, z - drift - w1),
                        WHITE_HOT,
                    );
                }
            }
            // A dark lip where the melt disappears under each bridge.
            for end in [a, b] {
                m.cuboid(
                    vec3(end - 0.07, 0.020, z - hw - 0.22),
                    vec3(end + 0.07, 0.085, z + hw + 0.22),
                    DARK,
                );
            }
        }
        // Grated steel bridges over the safe stretches.
        for pair in spans.windows(2) {
            let (a, b) = (pair[0].1 + 0.07, pair[1].0 - 0.07);
            m.cuboid(
                vec3(a, 0.020, z - hw - 0.22),
                vec3(b, 0.040, z + hw + 0.22),
                shade(STEEL, 0.9),
            );
            let n = ((b - a) / 0.4).round() as usize;
            for k in 0..=n {
                let x = a + (b - a) * k as f32 / n as f32;
                m.cuboid(
                    vec3(x - 0.03, 0.040, z - hw - 0.2),
                    vec3(x + 0.03, 0.050, z + hw + 0.2),
                    DARK,
                );
            }
            for side in [-1.0f32, 1.0] {
                let ez = z + side * (hw + 0.17);
                m.cuboid(
                    vec3(a, 0.040, ez - 0.05),
                    vec3(b, 0.056, ez + 0.05),
                    WARNING,
                );
            }
        }
    }
    for side in [-1.0f32, 1.0] {
        let z = side * layout::SERVICE_Z;
        let y = layout::SERVICE_ROOF_Y;
        for x in [-42.0f32, -30.0, -18.0, -6.0, 6.0, 18.0, 30.0, 42.0] {
            girder(
                m,
                vec3(x, y - 0.3, z - 2.8),
                vec3(x, y - 0.3, z + 2.8),
                0.20,
                0.35,
                EDGE,
            );
            m.cuboid(
                vec3(x - 0.55, y + 0.01, z - 2.9),
                vec3(x + 0.55, y + 0.06, z + 2.9),
                shade(STEEL, 0.85),
            );
        }
        pipeline(
            m,
            &[vec3(-46.0, 8.0, side * 21.5), vec3(46.0, 8.0, side * 21.5)],
            0.48,
            shade(RUST, 0.8),
        );
        pipeline(
            m,
            &[
                vec3(-40.0, 10.0, side * 20.7),
                vec3(40.0, 10.0, side * 20.7),
            ],
            0.21,
            EDGE,
        );
        for x in [-23.0f32, 23.0] {
            let g = Mat4::from_translation(vec3(x, 8.0, side * 22.2))
                * Mat4::from_rotation_y(if side < 0.0 { PI } else { 0.0 });
            wheel(m, g, 0.50);
        }
        // Hanging work lights keep the covered service lane readable.
        for x in [-36.0f32, -12.0, 12.0, 36.0] {
            m.cuboid(
                vec3(x - 0.7, y - 0.13, z - 0.2),
                vec3(x + 0.7, y - 0.08, z + 0.2),
                [0.67, 0.79, 0.75, 0.65],
            );
        }
    }
    for (x, z) in layout::FURNACE_ANCHORS {
        let side = z.signum();
        pipeline(
            m,
            &[
                vec3(x + 3.1, 9.5, z),
                vec3(x + 3.1, 9.5, side * 23.0),
                vec3(x + 3.1, 15.0, side * 23.0),
                vec3(x + 3.1, 15.0, side * 18.0),
            ],
            0.72,
            STEEL,
        );
    }
    // Small scattered flakes stay below step height and outside the nave.
    for k in 0..220u32 {
        let x = (hash01(k, 0xA5) * 2.0 - 1.0) * 111.0 * SPREAD;
        let z = (hash01(k, 0x5A) * 2.0 - 1.0) * 61.0 * SPREAD;
        if x.abs() < 36.0 && z.abs() < 24.0 {
            continue;
        }
        if w.blocks.iter().any(|b| {
            x > b.min.x - 0.1 && x < b.max.x + 0.1 && z > b.min.z - 0.1 && z < b.max.z + 0.1
        }) {
            continue;
        }
        let y = w.height(x, z) + 0.018;
        let g = Mat4::from_translation(vec3(x, y, z)) * Mat4::from_rotation_y(hash01(k, 3) * TAU);
        m.obox(
            &g,
            Vec3::ZERO,
            vec3(0.08 + hash01(k, 4) * 0.16, 0.025, 0.06 + hash01(k, 5) * 0.1),
            if k % 3 == 0 { RUST } else { DARK },
        );
    }
}

pub fn build(m: &mut Mesh, w: &World) {
    floor_and_services(m, w);
    nave(m);
    galleries(m);
    for (index, &(x, z)) in layout::FURNACE_ANCHORS.iter().enumerate() {
        furnace(m, x, z, index);
    }
    crucible(m);
    art::build(m, w);
}

pub fn sky() -> Mesh {
    let mut m = Mesh::default();
    let r = 420.0;
    let at = |e: f32, a: f32| vec3(e.cos() * a.cos(), e.sin(), e.cos() * a.sin()) * r;
    let color = |e: f32, a: f32| {
        let t = smooth(0.0, 1.1, e.max(0.0));
        let dusk = mix3([0.40, 0.25, 0.20], [0.105, 0.13, 0.20], t);
        let heat = (a - 0.7).cos().max(0.0).powi(4) * (1.0 - t) * 0.22;
        [
            dusk[0] + heat,
            dusk[1] + heat * 0.30,
            dusk[2] + heat * 0.06,
            1.0,
        ]
    };
    for row in 0..15 {
        let e0 = -0.25 + 1.82 * row as f32 / 15.0;
        let e1 = -0.25 + 1.82 * (row + 1) as f32 / 15.0;
        for k in 0..48 {
            let a = k as f32 * TAU / 48.0;
            let b = (k + 1) as f32 * TAU / 48.0;
            m.quad_c(
                at(e0, a),
                at(e0, b),
                at(e1, b),
                at(e1, a),
                [color(e0, a), color(e0, b), color(e1, b), color(e1, a)],
            );
        }
    }
    art::heavens(&mut m);
    art::strata(&mut m);
    art::skyline(&mut m);
    m
}

pub fn lights(w: &World, eye: Vec3, existing: [Light; 4]) -> [Light; 4] {
    let candidates = [
        Light {
            pos: vec3(0.0, 9.0, 0.0),
            radius: 22.0,
            col: [1.0, 0.31, 0.07],
            power: 1.1,
        },
        Light {
            pos: vec3(-18.0, 4.0, -26.9),
            radius: 20.0,
            col: [1.0, 0.26, 0.04],
            power: 1.0,
        },
        Light {
            pos: vec3(18.0, 4.0, 26.9),
            radius: 20.0,
            col: [1.0, 0.26, 0.04],
            power: 1.0,
        },
        Light {
            pos: vec3(0.0, 4.4, 39.0),
            radius: 18.0,
            col: [0.35, 0.63, 0.66],
            power: 0.55,
        },
        Light {
            pos: vec3(0.0, 4.4, -39.0),
            radius: 18.0,
            col: [0.35, 0.63, 0.66],
            power: 0.55,
        },
    ];
    let mut nearest: Vec<Light> = candidates.into_iter().chain(art::lights()).collect();
    nearest.sort_by(|a, b| {
        (a.pos - eye)
            .length_squared()
            .total_cmp(&(b.pos - eye).length_squared())
    });
    let mut result = existing;
    let mut n = 0;
    for light in &mut result {
        if light.power <= 0.0 || light.radius <= 0.0 {
            *light = nearest[n];
            n += 1;
        }
    }
    let _ = w;
    result
}

/// Deterministic smoke, floating ash and furnace cinders; no gameplay state.
pub fn atmosphere(alpha: &mut Mesh, add: &mut Mesh, w: &World, time: f32) {
    for (f, &(x, z)) in layout::FURNACE_ANCHORS.iter().enumerate() {
        for k in 0..18u32 {
            let seed = k + f as u32 * 29;
            let age = (time * 0.11 + hash01(seed, 22)).fract();
            let p = vec3(
                x + age * 8.0 + (seed as f32 * 1.7).sin() * age,
                layout::FURNACE_HEIGHT + 2.8 + age * 19.0,
                z + age * 4.0,
            );
            let radius = 1.2 + age * 4.0;
            let opacity = (1.0 - age) * 0.13;
            let c = [0.12, 0.105, 0.13, opacity];
            alpha.quad(
                p + vec3(-radius, -radius, 0.0),
                p + vec3(radius, -radius, 0.0),
                p + vec3(radius, radius, 0.0),
                p + vec3(-radius, radius, 0.0),
                c,
            );
            alpha.quad(
                p + vec3(0.0, -radius, -radius),
                p + vec3(0.0, -radius, radius),
                p + vec3(0.0, radius, radius),
                p + vec3(0.0, radius, -radius),
                c,
            );
        }
        for k in 0..30u32 {
            let seed = k + f as u32 * 31;
            let age = (time * (0.13 + hash01(seed, 17) * 0.1) + hash01(seed, 21)).fract();
            let p = vec3(
                x + (hash01(seed, 9) - 0.5) * 7.0 + age * 1.5,
                layout::FURNACE_HEIGHT + 2.6 + age * 11.0,
                z + (hash01(seed, 10) - 0.5) * 7.0,
            );
            let s = 0.025 + hash01(seed, 11) * 0.05;
            let c = [1.0, 0.37 + hash01(seed, 12) * 0.3, 0.07, (1.0 - age) * 0.8];
            add.quad(
                p + vec3(-s, -s * 2.0, 0.0),
                p + vec3(s, -s * 2.0, 0.0),
                p + vec3(s, s * 2.0, 0.0),
                p + vec3(-s, s * 2.0, 0.0),
                c,
            );
        }
    }
    // Lava channels: bright crusts drifting down the melt and rising sparks.
    for (c, z) in [-layout::LAVA_Z, layout::LAVA_Z].into_iter().enumerate() {
        for (s, &(a, b)) in layout::LAVA_SPANS.iter().enumerate() {
            let len = b - a;
            let seed = (c * 4 + s) as u32;
            for k in 0..(len / 1.6).ceil() as u32 {
                let flow = (time * 0.05 * (0.7 + hash01(k, 40 + seed) * 0.6)
                    + hash01(k, 50 + seed))
                .fract();
                let half = 0.25 + hash01(k, 60 + seed) * 0.35;
                let x = a + half + flow * (len - half * 2.0);
                let hz = layout::LAVA_HALF_W * (0.3 + hash01(k, 61 + seed) * 0.4);
                let pulse = 0.5 + 0.5 * (time * 1.7 + k as f32 * 2.1 + seed as f32).sin();
                // Fade in and out at the ends so crusts never pop.
                let fade = (flow * (1.0 - flow) * 8.0).min(1.0);
                add.quad(
                    vec3(x - half, 0.041, z - hz),
                    vec3(x - half, 0.041, z + hz),
                    vec3(x + half, 0.041, z + hz),
                    vec3(x + half, 0.041, z - hz),
                    [1.0, 0.55, 0.14, (0.18 + 0.22 * pulse) * fade],
                );
            }
            for k in 0..(len / 2.5).ceil() as u32 {
                let age =
                    (time * (0.35 + hash01(k, 80 + seed) * 0.3) + hash01(k, 81 + seed)).fract();
                let p = vec3(
                    a + hash01(k, 82 + seed) * len,
                    0.06 + age * (0.7 + hash01(k, 83 + seed) * 1.1),
                    z + (hash01(k, 84 + seed) - 0.5) * layout::LAVA_HALF_W * 1.6,
                );
                let r = 0.012 + hash01(k, 85 + seed) * 0.02;
                add.quad(
                    p + vec3(-r, -r * 2.0, 0.0),
                    p + vec3(r, -r * 2.0, 0.0),
                    p + vec3(r, r * 2.0, 0.0),
                    p + vec3(-r, r * 2.0, 0.0),
                    [1.0, 0.45, 0.08, (1.0 - age) * 0.85],
                );
            }
        }
    }
    art::atmosphere(alpha, add, w, time);
    for k in 0..110u32 {
        let age = (time * 0.04 + hash01(k, 70)).fract();
        let x = (hash01(k, 71) * 2.0 - 1.0) * 108.0 * SPREAD + age * 5.0;
        let z = (hash01(k, 72) * 2.0 - 1.0) * 58.0 * SPREAD;
        let p = vec3(
            x,
            w.height(x, z) + 0.8 + (1.0 - age) * (5.0 + hash01(k, 73) * 9.0),
            z,
        );
        let s = 0.012 + hash01(k, 74) * 0.028;
        alpha.quad(
            p + vec3(-s, 0.0, -s),
            p + vec3(s, 0.0, -s),
            p + vec3(s, 0.01, s),
            p + vec3(-s, 0.01, s),
            [0.59, 0.55, 0.53, 0.3],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_shared::map::{HALF_X, HALF_Z};
    #[test]
    fn cathedral_detail_is_finite_and_stays_within_its_landmark_budget() {
        let w = World::with_map(0xFE221, ID);
        let mut mesh = Mesh::default();
        build(&mut mesh, &w);
        eprintln!("Slag static detail: {} triangles", mesh.triangles());
        assert!(
            mesh.triangles() < 320_000,
            "static cathedral triangle budget"
        );
        let mut vertices = 0;
        let mut maximum_y = 0.0f32;
        for (v, indices) in mesh.finish() {
            vertices += v.len();
            assert!(indices.iter().all(|&i| (i as usize) < v.len()));
            for v in v {
                let p = Vec3::from_array(v.pos);
                assert!(p.is_finite() && Vec3::from_array(v.nrm).is_finite());
                assert!(
                    p.x.abs() < HALF_X && p.z.abs() < HALF_Z && p.y >= -8.0 && p.y < 48.0,
                    "cathedral detail outside its bounds: {p:?}"
                );
                maximum_y = maximum_y.max(p.y);
            }
        }
        assert!(
            maximum_y > 40.0,
            "furnace crowns should rise above the nave"
        );
        assert!(
            vertices < 680_000,
            "static detail budget: {vertices} vertices"
        );
    }
    #[test]
    fn animated_ash_and_furnace_effects_keep_finite_bounded_meshes() {
        let w = World::with_map(0xFE221, ID);
        for time in [0.0f32, 1.0, 17.0, 93.0] {
            let (mut alpha, mut add) = (Mesh::default(), Mesh::default());
            atmosphere(&mut alpha, &mut add, &w, time);
            assert!(
                alpha.triangles() + add.triangles() < 2_000 && add.triangles() > 150,
                "atmosphere triangle budget"
            );
            for (vertices, indices) in alpha.finish().into_iter().chain(add.finish()) {
                assert!(indices.iter().all(|&i| (i as usize) < vertices.len()));
                for vertex in vertices {
                    let p = Vec3::from_array(vertex.pos);
                    assert!(p.is_finite() && Vec3::from_array(vertex.nrm).is_finite());
                    assert!(p.x.abs() < HALF_X && p.z.abs() < HALF_Z && p.y > -8.0 && p.y < 65.0);
                    assert!(vertex.col.into_iter().all(f32::is_finite));
                }
            }
        }
    }
    #[test]
    fn the_sky_is_finite_and_the_bells_are_drawn_once() {
        let mut vertices = 0;
        for (v, indices) in sky().finish() {
            vertices += v.len();
            assert!(indices.iter().all(|&i| (i as usize) < v.len()));
            for v in v {
                assert!(Vec3::from_array(v.pos).is_finite());
                assert!(v
                    .col
                    .into_iter()
                    .all(|c| c.is_finite() && (0.0..=1.0).contains(&c)));
            }
        }
        assert!(
            vertices > 3_000 && vertices < 20_000,
            "sky budget: {vertices} vertices"
        );
        // Exactly the flask and body of each great bell are skinned by the
        // art pass instead of the generic block renderer.
        let w = World::with_map(0xFE221, ID);
        assert_eq!(w.blocks.iter().filter(|b| art::owns(b)).count(), 4);
        // Every character used on a sign has a glyph.
        for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-".chars() {
            assert!(glyph(c).iter().any(|&row| row != 0), "no glyph for {c}");
        }
    }
    #[test]
    fn yard_dressing_follows_the_spread_out_collision() {
        let near = |b: &Block, x: f32, z: f32| {
            let c = (b.min + b.max) * 0.5;
            (c.x - x).abs() < 0.05 && (c.z - z).abs() < 0.05
        };
        for seed in [0xFE221u64, 1, 6, 8, 12, 77, 0xBEEF] {
            let w = World::with_map(seed, ID);
            let (sx, sz) = art::STOCKYARD;
            assert!(
                w.blocks.iter().any(|b| near(b, sx, sz) && b.min.y > 5.0),
                "stockyard lamps and sign hang from the gantry beam"
            );
            assert!(w.blocks.iter().any(|b| near(b, art::GATE_WALL_X, 0.0)
                && b.kind == BlockKind::Concrete));
            assert!((art::COURT.0 - layout::POINTS[1].0).abs() < 0.01);
            for x in layout::PIPE_YARD_X {
                assert!(w.blocks.iter().any(|b| near(b, x, layout::PIPE_YARD_Z)));
            }
            // The railway runs out through both portals, stops short of the
            // spawn ground, and nothing standing on the ground crosses it.
            let runs = art::rail_runs(&w);
            assert!(runs[0].0 == 0.0 && runs[0].1 > layout::NAVE_HALF_X + 1.0, "{runs:?}");
            assert!(runs.iter().all(|&(a, b)| a < b && b <= art::RAIL_LIMIT));
            for &(a, b) in &runs {
                for s in [-1.0f32, 1.0] {
                    let (x0, x1) = ((s * a).min(s * b) - 0.45, (s * a).max(s * b) + 0.45);
                    assert!(!w.blocks.iter().any(|blk| {
                        let c = (blk.min + blk.max) * 0.5;
                        blk.min.y < w.height(c.x, c.z) + 0.5
                            && blk.min.z < 1.2
                            && blk.max.z > -1.2
                            && blk.max.x > x0
                            && blk.min.x < x1
                    }), "a building stands on the ladle railway: {runs:?}");
                }
            }
        }
    }
    #[test]
    fn landmark_glow_preserves_active_combat_lights() {
        let w = World::with_map(0xFE221, ID);
        let shot = Light {
            pos: vec3(1.0, 2.0, 3.0),
            radius: 7.0,
            col: [1.0, 0.7, 0.3],
            power: 2.0,
        };
        let result = lights(
            &w,
            Vec3::ZERO,
            [shot, Light::default(), Light::default(), Light::default()],
        );
        assert_eq!(result[0].pos, shot.pos);
        assert_eq!(result[0].power, shot.power);
        assert!(result[1..].iter().all(|l| l.radius > 0.0 && l.power > 0.0));
    }
}
