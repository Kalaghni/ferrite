//! Pictures of library models for the map maker's grid: a small software
//! renderer, so a card can be drawn (and turned while hovered) without
//! touching the game's 3D pipelines. Flat-lit faces with dark edges, a
//! soft shadow, and a clear background so the card shows through.

use ferrite_shared::glam::{vec3, Vec3};
use ferrite_shared::map::ops::Prefab;
use ferrite_shared::map::BlockKind;

const CONTAINER: [[f32; 3]; 4] = [[0.66, 0.27, 0.16], [0.18, 0.46, 0.48], [0.80, 0.60, 0.18], [0.34, 0.38, 0.47]];

fn colour(k: BlockKind, tint: u8) -> [f32; 3] {
    let j: f32 = [1.0, 0.9, 1.08, 0.82][tint as usize % 4];
    let c: [f32; 3] = match k {
        BlockKind::Container => return CONTAINER[tint as usize % 4],
        BlockKind::Concrete => [0.68, 0.67, 0.63],
        BlockKind::Sandbag => [0.76, 0.67, 0.46],
        BlockKind::Crate => [0.72, 0.53, 0.29],
        BlockKind::Metal => [0.50, 0.57, 0.66],
        BlockKind::Rock => [0.52, 0.49, 0.46],
        BlockKind::Step => [0.64, 0.64, 0.62],
        BlockKind::Trunk => [0.45, 0.31, 0.19],
    };
    c.map(|v| (v * j).min(1.0))
}

struct Tri {
    p: [Vec3; 3],
    col: [f32; 3],
    /// Faces that share an id are drawn without an edge between them.
    id: u32,
}

fn quad(out: &mut Vec<Tri>, a: Vec3, b: Vec3, c: Vec3, d: Vec3, col: [f32; 3], id: u32) {
    out.push(Tri { p: [a, b, c], col, id });
    out.push(Tri { p: [a, c, d], col, id });
}

fn cuboid(out: &mut Vec<Tri>, lo: Vec3, hi: Vec3, col: [f32; 3], id: &mut u32) {
    let v = |x: bool, y: bool, z: bool| vec3(if x { hi.x } else { lo.x }, if y { hi.y } else { lo.y }, if z { hi.z } else { lo.z });
    let faces = [
        [v(false, true, false), v(true, true, false), v(true, true, true), v(false, true, true)],
        [v(false, false, false), v(false, false, true), v(true, false, true), v(true, false, false)],
        [v(false, false, false), v(true, false, false), v(true, true, false), v(false, true, false)],
        [v(false, false, true), v(false, true, true), v(true, true, true), v(true, false, true)],
        [v(false, false, false), v(false, true, false), v(false, true, true), v(false, false, true)],
        [v(true, false, false), v(true, false, true), v(true, true, true), v(true, true, false)],
    ];
    for f in faces {
        quad(out, f[0], f[1], f[2], f[3], col, *id);
        *id += 1;
    }
}

/// The model as triangles, in metres about its origin.
fn tris(p: &Prefab) -> Vec<Tri> {
    let mut out = Vec::new();
    let mut id = 1;
    for b in &p.blocks {
        cuboid(&mut out, Vec3::from(b.min), Vec3::from(b.max), colour(b.kind, b.tint), &mut id);
    }
    for t in &p.trees {
        let base = vec3(t.at[0], 0.0, t.at[1]);
        let w = (t.radius * 0.12).clamp(0.15, 0.4);
        cuboid(&mut out, base - vec3(w, 0.0, w), base + vec3(w, t.height * 0.4, w), [0.42, 0.29, 0.18], &mut id);
        let ring: Vec<Vec3> = (0..6)
            .map(|k| {
                let a = k as f32 / 6.0 * std::f32::consts::TAU + 0.3;
                vec3(a.cos() * t.radius, 0.0, a.sin() * t.radius)
            })
            .collect();
        let (col, lo, mid, top) = if t.pine {
            ([0.20, 0.42, 0.26], t.height * 0.22, t.height * 0.22, t.height)
        } else {
            ([0.40, 0.60, 0.26], t.height * 0.3, t.height * 0.62, t.height)
        };
        for k in 0..6 {
            let (a, b) = (ring[k], ring[(k + 1) % 6]);
            let (am, bm) = (base + a + vec3(0.0, mid, 0.0), base + b + vec3(0.0, mid, 0.0));
            out.push(Tri { p: [am, bm, base + vec3(0.0, top, 0.0)], col, id });
            id += 1;
            out.push(Tri { p: [am, base + vec3(0.0, lo, 0.0), bm], col: col.map(|c| c * 0.85), id });
            id += 1;
        }
    }
    out
}

/// A `size` x `size` RGBA picture of the model seen from `yaw` (radians
/// round it), as bytes, top row first.
pub fn render(p: &Prefab, size: usize, yaw: f32) -> Vec<u8> {
    const SS: usize = 2;
    let n = size * SS;
    let tris = tris(p);
    let pitch: f32 = 0.6;
    let fwd = vec3(-pitch.cos() * yaw.sin(), -pitch.sin(), -pitch.cos() * yaw.cos());
    let right = fwd.cross(Vec3::Y).normalize();
    let up = right.cross(fwd);
    let light = vec3(-0.45, 0.85, -0.3).normalize();
    // Fit the model, with a margin.
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for t in &tris {
        for v in t.p {
            let s = [v.dot(right), v.dot(up)];
            lo = [lo[0].min(s[0]), lo[1].min(s[1])];
            hi = [hi[0].max(s[0]), hi[1].max(s[1])];
        }
    }
    if tris.is_empty() {
        return vec![0; size * size * 4];
    }
    let span = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(0.5);
    let scale = n as f32 * 0.78 / span;
    let mid = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5];
    let to_px = |v: Vec3| {
        let (x, y) = (v.dot(right) - mid[0], v.dot(up) - mid[1]);
        (n as f32 * 0.5 + x * scale, n as f32 * 0.52 - y * scale, v.dot(fwd))
    };
    let mut depth = vec![f32::MAX; n * n];
    let mut col = vec![[0.0f32; 3]; n * n];
    let mut ids = vec![0u32; n * n];
    for t in &tris {
        // Shade by the side facing the camera: winding is not trusted.
        let mut nrm = (t.p[1] - t.p[0]).cross(t.p[2] - t.p[0]).normalize_or_zero();
        if nrm.dot(fwd) > 0.0 {
            nrm = -nrm;
        }
        let lit = 0.58 + 0.42 * nrm.dot(light).max(0.0) + 0.1 * nrm.y.max(0.0);
        let c = t.col.map(|v| (v * lit).min(1.0));
        let s = t.p.map(to_px);
        let x0 = s.iter().map(|v| v.0).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let x1 = (s.iter().map(|v| v.0).fold(f32::MIN, f32::max).ceil() as usize).min(n - 1);
        let y0 = s.iter().map(|v| v.1).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let y1 = (s.iter().map(|v| v.1).fold(f32::MIN, f32::max).ceil() as usize).min(n - 1);
        let area = (s[1].0 - s[0].0) * (s[2].1 - s[0].1) - (s[2].0 - s[0].0) * (s[1].1 - s[0].1);
        if area.abs() < 1e-6 || x0 > x1 || y0 > y1 {
            continue;
        }
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let e = |a: (f32, f32, f32), b: (f32, f32, f32)| (b.0 - a.0) * (py - a.1) - (b.1 - a.1) * (px - a.0);
                let (w0, w1, w2) = (e(s[1], s[2]) / area, e(s[2], s[0]) / area, e(s[0], s[1]) / area);
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let z = w0 * s[0].2 + w1 * s[1].2 + w2 * s[2].2;
                let k = y * n + x;
                if z < depth[k] {
                    depth[k] = z;
                    col[k] = c;
                    ids[k] = t.id;
                }
            }
        }
    }
    // Dark edges where one face meets another or the background.
    let mut rgba = vec![[0.0f32; 4]; n * n];
    for y in 0..n {
        for x in 0..n {
            let k = y * n + x;
            if ids[k] == 0 {
                continue;
            }
            let edge = [(1i32, 0i32), (0, 1), (-1, 0), (0, -1)].iter().any(|&(dx, dy)| {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx < 0 || ny < 0 || nx >= n as i32 || ny >= n as i32 {
                    return true;
                }
                let j = ny as usize * n + nx as usize;
                // The edge goes on the nearer face's side.
                ids[j] != ids[k] && (ids[j] == 0 || depth[j] >= depth[k])
            });
            let c = if edge { col[k].map(|v| v * 0.62) } else { col[k] };
            rgba[k] = [c[0], c[1], c[2], 1.0];
        }
    }
    // A soft shadow under the footprint.
    let foot = to_px(Vec3::ZERO);
    let reach = (hi[0] - lo[0]) * scale * 0.5;
    let (rx, ry) = (reach * 0.95, reach * 0.95 * pitch.sin());
    for y in 0..n {
        for x in 0..n {
            let k = y * n + x;
            if rgba[k][3] > 0.0 {
                continue;
            }
            let (dx, dy) = ((x as f32 - foot.0) / rx.max(1.0), (y as f32 - foot.1) / ry.max(1.0));
            let d2 = dx * dx + dy * dy;
            if d2 < 1.0 {
                rgba[k] = [0.0, 0.0, 0.0, 0.32 * (1.0 - d2).powi(2)];
            }
        }
    }
    // Down to size, averaging colour by coverage.
    let mut out = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let mut acc = [0.0f32; 4];
            for sy in 0..SS {
                for sx in 0..SS {
                    let c = rgba[(y * SS + sy) * n + x * SS + sx];
                    for i in 0..3 {
                        acc[i] += c[i] * c[3];
                    }
                    acc[3] += c[3];
                }
            }
            let a = acc[3] / (SS * SS) as f32;
            let k = (y * size + x) * 4;
            for i in 0..3 {
                out[k + i] = if acc[3] > 0.0 { (acc[i] / acc[3] * 255.0).round() as u8 } else { 0 };
            }
            out[k + 3] = (a * 255.0).round() as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    /// Writes every built-in model's card to a contact sheet for review:
    /// `cargo test -p ferrite thumbs_sheet -- --ignored`, then open
    /// target/thumbs.png.
    #[test]
    #[ignore]
    fn thumbs_sheet() {
        use ferrite_shared::map::library::builtin;
        let models = builtin();
        let (size, cols) = (112usize, 7usize);
        let rows = models.len().div_ceil(cols);
        let (w, h) = (cols * size, rows * size);
        let mut sheet: Vec<u8> = [38u8, 43, 66, 255].iter().copied().cycle().take(w * h * 4).collect();
        for (i, (_, p)) in models.iter().enumerate() {
            let img = super::render(p, size, 0.75);
            let (ox, oy) = ((i % cols) * size, (i / cols) * size);
            for y in 0..size {
                for x in 0..size {
                    let s = (y * size + x) * 4;
                    // export_png wants the bottom row first.
                    let d = ((h - 1 - (oy + y)) * w + ox + x) * 4;
                    let a = img[s + 3] as f32 / 255.0;
                    let bg = [0.15f32, 0.17, 0.26];
                    for c in 0..3 {
                        sheet[d + c] = (img[s + c] as f32 * a + bg[c] * 255.0 * (1.0 - a)) as u8;
                    }
                    sheet[d + 3] = 255;
                }
            }
        }
        let img = macroquad::prelude::Image { bytes: sheet, width: w as u16, height: h as u16 };
        img.export_png(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/thumbs.png"));
    }
}
