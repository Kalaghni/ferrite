//! Signs with some character. Places get an enamel nameboard in their own
//! colours: raised lettering, a pinstripe, an emblem badge, bolts, two
//! gooseneck lamps over it, hung a touch off level. Small warnings get a
//! safety plate: yellow and black with hazard stripes, or red for a breach.

use super::hash01;
use super::slag_cathedral::glyph;
use crate::gfx::{rgb, shade, Col, Mesh};
use ferrite_shared::glam::{vec3, Mat4, Vec3};

const TRIM: Col = [0.16, 0.17, 0.17, 0.0];
const BULB: Col = [1.0, 0.86, 0.6, 0.9];
const SHADE_COL: Col = [0.12, 0.14, 0.14, 0.0];
const YELLOW: Col = [0.95, 0.74, 0.12, 0.0];
const INK: Col = [0.06, 0.06, 0.06, 0.0];

/// Seven-by-seven emblems, bit 6 the leftmost.
const GEAR: [u8; 7] = [0b0010100, 0b0111110, 0b1110111, 0b0100010, 0b1110111, 0b0111110, 0b0010100];
const PICK: [u8; 7] = [0b0111110, 0b1001001, 0b0001000, 0b0001000, 0b0001000, 0b0001000, 0b0011100];
const PINE: [u8; 7] = [0b0001000, 0b0011100, 0b0111110, 0b0011100, 0b0111110, 0b1111111, 0b0001000];
const ARROW: [u8; 7] = [0b1001000, 0b0100100, 0b0010010, 0b0001001, 0b0010010, 0b0100100, 0b1001000];
const ARCH: [u8; 7] = [0b0011100, 0b0111110, 0b1100011, 0b1000001, 0b1000001, 0b1000001, 0b1111111];
/// Hazard stripes for the ends of a safety plate, four wide.
const STRIPES: [u8; 7] = [0b1100, 0b0110, 0b0011, 0b1001, 0b1100, 0b0110, 0b0011];

/// The look of a place's board: face, lettering, emblem.
fn livery(text: &str) -> (Col, Col, Option<[u8; 7]>) {
    let cream = rgb(0.94, 0.88, 0.70);
    match text {
        "VALLEY WORKS" | "VALLEY" => (rgb(0.13, 0.32, 0.22), cream, Some(GEAR)),
        "STONE WORKS" | "PIT 04" | "MINE 04" => (rgb(0.46, 0.15, 0.12), cream, Some(PICK)),
        "INCLINE" => (rgb(0.46, 0.15, 0.12), cream, None),
        "RANGER STATION" => (rgb(0.30, 0.20, 0.12), rgb(0.95, 0.78, 0.36), Some(PINE)),
        "FREIGHT CONTROL" | "FERRITE FREIGHT" => (rgb(0.10, 0.17, 0.33), rgb(0.95, 0.95, 0.92), Some(ARROW)),
        "SERVICE TUNNEL" | "TRANSIT 07" => (rgb(0.16, 0.30, 0.33), cream, Some(ARCH)),
        _ => {
            const FACES: [[f32; 3]; 4] = [[0.13, 0.32, 0.22], [0.46, 0.15, 0.12], [0.10, 0.17, 0.33], [0.16, 0.30, 0.33]];
            let c = FACES[text.bytes().map(u32::from).sum::<u32>() as usize % FACES.len()];
            (rgb(c[0], c[1], c[2]), cream, None)
        }
    }
}

/// Scale2x: each dot becomes four, with the corners on a diagonal filled
/// or cut, so slopes and curves read smooth instead of stepped.
fn scale2x(rows: &[u8], cols: u32) -> Vec<u32> {
    let at = |r: i32, c: i32| {
        r >= 0 && c >= 0 && (r as usize) < rows.len() && (c as u32) < cols && rows[r as usize] & (1 << (cols - 1 - c as u32)) != 0
    };
    let mut out = vec![0u32; rows.len() * 2];
    let w = cols * 2;
    for r in 0..rows.len() as i32 {
        for c in 0..cols as i32 {
            let p = at(r, c);
            let (a, b, cc, d) = (at(r - 1, c), at(r, c + 1), at(r, c - 1), at(r + 1, c));
            let e = [
                if cc == a && cc != d && a != b { a } else { p },
                if a == b && a != cc && b != d { b } else { p },
                if d == cc && d != b && cc != a { cc } else { p },
                if b == d && b != a && d != cc { d } else { p },
            ];
            for (k, on) in e.into_iter().enumerate() {
                if on {
                    let (rr, col) = (r as usize * 2 + k / 2, c as u32 * 2 + (k % 2) as u32);
                    out[rr] |= 1 << (w - 1 - col);
                }
            }
        }
    }
    out
}

/// A grid of set bits, smoothed (`scale2x`) and drawn as rectangles:
/// runs across each row, stacked where rows below repeat them. Raised
/// blocks when `depth` > 0, flat otherwise. Bit `cols - 1` is the
/// leftmost; the grid's bottom-left corner is at `at`.
fn bits(m: &mut Mesh, f: &Mat4, rows: &[u8], cols: u32, at: Vec3, pixel: f32, depth: f32, col: Col) {
    let fine = scale2x(rows, cols);
    let (cols, pixel) = (cols * 2, pixel * 0.5);
    let runs = |row: u32| {
        let mut v = Vec::new();
        let mut b = 0;
        while b < cols {
            if row & (1 << (cols - 1 - b)) == 0 {
                b += 1;
                continue;
            }
            let s = b;
            while b < cols && row & (1 << (cols - 1 - b)) != 0 {
                b += 1;
            }
            v.push((s, b));
        }
        v
    };
    let n = fine.len();
    // (start, end, first row, rows) of rectangles still growing downward.
    let mut open: Vec<(u32, u32, usize, usize)> = Vec::new();
    let emit = |m: &mut Mesh, (s, e, r0, k): (u32, u32, usize, usize)| {
        let x = at.x + (s + e) as f32 * 0.5 * pixel;
        let y = at.y + (n as f32 - r0 as f32 - k as f32 * 0.5) * pixel;
        rect(m, f, vec3(x, y, at.z), (e - s) as f32 * 0.5 * pixel, k as f32 * 0.5 * pixel, depth, col);
    };
    for r in 0..=n {
        let now = if r < n { runs(fine[r]) } else { Vec::new() };
        let mut next = Vec::new();
        for o in open.drain(..) {
            if now.contains(&(o.0, o.1)) {
                next.push((o.0, o.1, o.2, o.3 + 1));
            } else {
                emit(m, o);
            }
        }
        for (s, e) in now {
            if !next.iter().any(|o| o.0 == s && o.1 == e) {
                next.push((s, e, r, 1));
            }
        }
        open = next;
    }
}

/// A rectangle on the face at `c`: a block standing `depth` proud, or flat
/// when `depth` is 0. No back (it sits on the board) and no top: signs hang
/// above eye level, so the top is the one side never seen.
fn rect(m: &mut Mesh, f: &Mat4, c: Vec3, hw: f32, hh: f32, depth: f32, col: Col) {
    let p = |x: f32, y: f32, z: f32| f.transform_point3(c + vec3(x, y, z));
    let n = |v: Vec3| f.transform_vector3(v).normalize_or_zero();
    let d = depth;
    m.quad_nc([p(-hw, -hh, d), p(hw, -hh, d), p(hw, hh, d), p(-hw, hh, d)], [n(Vec3::Z); 4], [col; 4]);
    if d > 0.0 {
        let side = shade(col, 0.85);
        m.quad_nc([p(-hw, -hh, 0.0), p(hw, -hh, 0.0), p(hw, -hh, d), p(-hw, -hh, d)], [n(-Vec3::Y); 4], [side; 4]);
        for s in [-1.0f32, 1.0] {
            m.quad_nc([p(s * hw, -hh, 0.0), p(s * hw, hh, 0.0), p(s * hw, hh, d), p(s * hw, -hh, d)], [n(Vec3::X * s); 4], [side; 4]);
        }
    }
}

/// `text` in the 5 x 7 face, centred on x = 0 with its baseline at `at.y`.
fn letters(m: &mut Mesh, f: &Mat4, text: &str, at: Vec3, pixel: f32, depth: f32, col: Col) {
    let width = text.chars().count() as f32 * 6.0 * pixel - pixel;
    for (i, c) in text.chars().enumerate() {
        let x = at.x - width * 0.5 + i as f32 * 6.0 * pixel;
        bits(m, f, &glyph(c), 5, vec3(x, at.y, at.z), pixel, depth, col);
    }
}

/// A slab facing +z in `f`, its front face shaded from `top` to `bottom`.
fn slab(m: &mut Mesh, f: &Mat4, c: Vec3, h: Vec3, top: Col, bottom: Col) {
    let p = |x: f32, y: f32, z: f32| f.transform_point3(c + vec3(x * h.x, y * h.y, z * h.z));
    let n = |v: Vec3| f.transform_vector3(v).normalize_or_zero();
    m.quad_nc([p(-1.0, -1.0, 1.0), p(1.0, -1.0, 1.0), p(1.0, 1.0, 1.0), p(-1.0, 1.0, 1.0)], [n(Vec3::Z); 4], [bottom, bottom, top, top]);
    let side = shade(bottom, 0.8);
    m.quad_nc([p(-1.0, 1.0, -1.0), p(1.0, 1.0, -1.0), p(1.0, 1.0, 1.0), p(-1.0, 1.0, 1.0)], [n(Vec3::Y); 4], [top; 4]);
    m.quad_nc([p(-1.0, -1.0, -1.0), p(1.0, -1.0, -1.0), p(1.0, -1.0, 1.0), p(-1.0, -1.0, 1.0)], [n(-Vec3::Y); 4], [side; 4]);
    for s in [-1.0f32, 1.0] {
        m.quad_nc([p(s, -1.0, -1.0), p(s, 1.0, -1.0), p(s, 1.0, 1.0), p(s, -1.0, 1.0)], [n(Vec3::X * s); 4], [side, side, top, side]);
    }
}

/// A sign with its lettering centred on `f`'s origin, baseline at y = 0,
/// facing +z. Warnings (`warn`) and very small labels get a safety plate.
pub(super) fn draw(m: &mut Mesh, f: &Mat4, text: &str, pixel: f32, warn: bool) {
    if warn || pixel < 0.04 {
        plate(m, f, text, pixel, warn);
    } else {
        board(m, f, text, pixel);
    }
}

fn board(m: &mut Mesh, f: &Mat4, text: &str, p: f32) {
    let (face, ink, emblem) = livery(text);
    let seed = text.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    // Hung by hand: a touch off level, a dot above the line it is given.
    let f = *f
        * Mat4::from_translation(vec3(0.0, 4.5 * p, 0.0))
        * Mat4::from_rotation_z((hash01(seed, 77) - 0.5) * 0.03)
        * Mat4::from_translation(vec3(0.0, -3.5 * p, 0.0));
    let tw = text.chars().count() as f32 * 6.0 * p - p;
    let badge = if emblem.is_some() { 13.0 * p } else { 0.0 };
    let (hw, hh, cy) = ((tw + badge) * 0.5 + 3.0 * p, 6.0 * p, 3.5 * p);
    let shift = badge * 0.5;
    // Trim behind, the enamel face (sun-faded at the top, grimy below).
    m.obox(&f, vec3(0.0, cy, -0.04), vec3(hw + 0.7 * p, hh + 0.7 * p, 0.02), TRIM);
    slab(m, &f, vec3(0.0, cy, -0.01), vec3(hw, hh, 0.01), shade(face, 1.12), shade(face, 0.82));
    // Pinstripe round the edge.
    let (sx, sy, t) = (hw - 1.2 * p, hh - 1.2 * p, 0.18 * p);
    for s in [-1.0f32, 1.0] {
        m.obox(&f, vec3(0.0, cy + s * sy, 0.002), vec3(sx + t, t, 0.002), ink);
        m.obox(&f, vec3(s * sx, cy, 0.002), vec3(t, sy - t, 0.002), ink);
    }
    // Painted lettering with a sign-writer's drop shadow.
    letters(m, &f, text, vec3(shift + 0.3 * p, -0.3 * p, 0.003), p, 0.0, shade(face, 0.45));
    letters(m, &f, text, vec3(shift, 0.0, 0.006), p, 0.0, ink);
    // The emblem: a disc in the lettering colour, the picture raised on it.
    if let Some(icon) = emblem {
        let cx = shift - tw * 0.5 - 7.0 * p;
        let disc = f * Mat4::from_translation(vec3(cx, cy, 0.006));
        m.tube(&disc, Vec3::ZERO, 5.0 * p, 0.0, 0.006, 8, ink);
        bits(m, &f, &icon, 7, vec3(cx - 3.5 * p, cy - 3.5 * p, 0.016), p, 0.0, face);
    }
    // Bolts in the corners.
    for (x, y) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        rect(m, &f, vec3(x * (hw - 0.55 * p), cy + y * (hh - 0.55 * p), 0.0), 0.28 * p, 0.28 * p, 0.008, TRIM);
    }
    // Brackets back to the wall, and a gooseneck lamp over each end.
    let top = cy + hh + 0.7 * p;
    for s in [-1.0f32, 1.0] {
        let x = s * (hw * 0.62);
        m.obox(&f, vec3(x, top - 0.05, -0.2), vec3(0.025, 0.025, 0.15), TRIM);
        if p >= 0.06 {
            let reach = (hh * 1.1).clamp(0.35, 0.6);
            let rise = reach * 0.45;
            let (a, b, c) = (vec3(x, top - 0.02, -0.03), vec3(x, top + rise, 0.02), vec3(x, top + rise * 0.8, reach));
            m.obox(&seg(&f, a, b), Vec3::ZERO, vec3(0.022, 0.022, (b - a).length() * 0.5 + 0.02), TRIM);
            m.obox(&seg(&f, b, c), Vec3::ZERO, vec3(0.022, 0.022, (c - b).length() * 0.5 + 0.02), TRIM);
            // A dished shade over a warm bulb, tipped toward the board.
            let lamp = f * Mat4::from_translation(c) * Mat4::from_rotation_x(0.45);
            let down = lamp * Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2);
            m.tube(&down, Vec3::ZERO, 0.15, 0.0, 0.06, 4, SHADE_COL);
            m.obox(&lamp, vec3(0.0, -0.08, 0.0), vec3(0.055, 0.03, 0.055), BULB);
        }
    }
}

/// A frame whose local z runs from `a` to `b`, centred between them.
fn seg(f: &Mat4, a: Vec3, b: Vec3) -> Mat4 {
    let d = (b - a).normalize_or_zero();
    *f * Mat4::from_translation((a + b) * 0.5) * Mat4::from_quat(ferrite_shared::glam::Quat::from_rotation_arc(Vec3::Z, d))
}

fn plate(m: &mut Mesh, f: &Mat4, text: &str, p: f32, warn: bool) {
    let breach = text == "BREACH";
    let (face, ink) = if breach { (rgb(0.72, 0.12, 0.09), rgb(0.95, 0.93, 0.88)) } else { (YELLOW, INK) };
    let tw = text.chars().count() as f32 * 6.0 * p - p;
    let stripes = warn && text.chars().count() > 4;
    let ends = if stripes { 6.0 * p } else { 0.0 };
    let (hw, hh, cy) = (tw * 0.5 + 2.0 * p + ends, 5.5 * p, 3.5 * p);
    m.obox(f, vec3(0.0, cy, -0.02), vec3(hw + 0.5 * p, hh + 0.5 * p, 0.012), INK);
    slab(m, f, vec3(0.0, cy, -0.004), vec3(hw, hh, 0.004), shade(face, 1.05), shade(face, 0.86));
    letters(m, f, text, vec3(0.0, 0.0, 0.004), p, 0.0, ink);
    if stripes {
        for s in [-1.0f32, 1.0] {
            let x = s * (hw - 3.0 * p) - 2.0 * p;
            bits(m, f, &STRIPES, 4, vec3(x, 0.0, 0.004), p, 0.0, INK);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Signs stay cheap enough for the landmark and tunnel budgets, and
    /// every vertex is finite.
    #[test]
    fn signs_are_bounded() {
        for (t, p, warn, limit) in [
            ("VALLEY WORKS", 0.07, false, 1_200),
            ("FERRITE FREIGHT", 0.13, false, 1_300),
            ("HARD HATS", 0.05, true, 400),
            ("BREACH", 0.025, false, 300),
        ] {
            let mut m = Mesh::default();
            draw(&mut m, &Mat4::IDENTITY, t, p, warn);
            assert!(m.triangles() < limit, "{t}: {} triangles", m.triangles());
            assert!(m.v.iter().all(|v| v.pos.iter().chain(v.nrm.iter()).all(|x| x.is_finite())));
        }
    }

    /// Scale2x rounds a block's outer corners and fills in the steps of a
    /// diagonal stroke.
    #[test]
    fn scale2x_smooths_corners_and_diagonals() {
        assert_eq!(scale2x(&[0b11, 0b11], 2), vec![0b0110, 0b1111, 0b1111, 0b0110]);
        let d = scale2x(&[0b100, 0b010, 0b001], 3);
        let set: u32 = d.iter().map(|r| r.count_ones()).sum();
        assert!(set > 12, "a three-dot diagonal should gain filled corners, got {d:?}");
    }
}
