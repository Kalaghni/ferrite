//! A top-down plan of a built map, as a picture: shaded ground, every
//! block by kind, trees, objectives and bases, and optionally where
//! people died in a soak.

use crate::png;
use ferrite_shared::map::{BlockKind, World, HALF_X, HALF_Z};

/// Metres a pixel.
pub const SCALE: f32 = 0.75;

pub fn size() -> (usize, usize) {
    ((2.0 * HALF_X / SCALE).round() as usize, (2.0 * HALF_Z / SCALE).round() as usize)
}

fn kind_col(k: BlockKind) -> [f32; 3] {
    match k {
        BlockKind::Concrete => [0.66, 0.66, 0.63],
        BlockKind::Container => [0.80, 0.42, 0.20],
        BlockKind::Rock => [0.48, 0.44, 0.40],
        BlockKind::Crate => [0.78, 0.60, 0.32],
        BlockKind::Sandbag => [0.80, 0.72, 0.48],
        BlockKind::Metal => [0.52, 0.60, 0.70],
        BlockKind::Trunk => [0.42, 0.28, 0.16],
        BlockKind::Step => [0.72, 0.72, 0.72],
    }
}

/// 5 x 7 letters for the objectives and bases.
fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        _ => [0; 7],
    }
}

struct Canvas {
    w: usize,
    h: usize,
    px: Vec<[f32; 3]>,
}

impl Canvas {
    fn at(&self, x: f32, z: f32) -> (f32, f32) {
        ((x + HALF_X) / SCALE, (z + HALF_Z) / SCALE)
    }
    fn set(&mut self, x: i32, y: i32, c: [f32; 3], a: f32) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            let p = &mut self.px[y as usize * self.w + x as usize];
            for k in 0..3 {
                p[k] += (c[k] - p[k]) * a;
            }
        }
    }
    fn ring(&mut self, x: f32, z: f32, r: f32, c: [f32; 3]) {
        let (cx, cy) = self.at(x, z);
        let rp = r / SCALE;
        let n = (rp * 8.0) as usize + 16;
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            for t in [-0.5f32, 0.5] {
                let (px, py) = (cx + a.cos() * (rp + t), cy + a.sin() * (rp + t));
                self.set(px as i32, py as i32, c, 0.9);
            }
        }
    }
    fn text(&mut self, s: &str, x: f32, z: f32, c: [f32; 3]) {
        let (cx, cy) = self.at(x, z);
        let scale = 2;
        let wide = s.len() as i32 * 6 * scale;
        let (x0, y0) = (cx as i32 - wide / 2, cy as i32 - 7 * scale / 2);
        for (k, ch) in s.chars().enumerate() {
            let g = glyph(ch);
            for (row, bits) in g.iter().enumerate() {
                for col in 0..5 {
                    if bits >> (4 - col) & 1 == 1 {
                        for dy in 0..scale {
                            for dx in 0..scale {
                                let (px, py) = (x0 + (k as i32 * 6 + col) * scale + dx, y0 + row as i32 * scale + dy);
                                // A dark edge so letters read on anything.
                                for (ox, oy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                                    self.set(px + ox, py + oy, [0.05, 0.05, 0.05], 0.6);
                                }
                            }
                        }
                    }
                }
            }
            for (row, bits) in g.iter().enumerate() {
                for col in 0..5 {
                    if bits >> (4 - col) & 1 == 1 {
                        for dy in 0..scale {
                            for dx in 0..scale {
                                self.set(x0 + (k as i32 * 6 + col) * scale + dx, y0 + row as i32 * scale + dy, c, 1.0);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The plan as a PNG. `heat` is where deaths happened, `[x, z]`.
pub fn render(w: &World, ground: [[f32; 3]; 3], heat: &[[f32; 2]]) -> Vec<u8> {
    let (cw, ch) = size();
    let mut c = Canvas { w: cw, h: ch, px: vec![[0.0; 3]; cw * ch] };
    // Ground: the map's grass colour, lit from the north west, lighter
    // the higher it is.
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    let mut hs = vec![0.0f32; cw * ch];
    for py in 0..ch {
        for px in 0..cw {
            let (x, z) = (-HALF_X + (px as f32 + 0.5) * SCALE, -HALF_Z + (py as f32 + 0.5) * SCALE);
            let h = w.height(x, z);
            hs[py * cw + px] = h;
            lo = lo.min(h);
            hi = hi.max(h);
        }
    }
    let grass = ground[0];
    for py in 0..ch {
        for px in 0..cw {
            let h = hs[py * cw + px];
            let hx = hs[py * cw + (px + 1).min(cw - 1)] - hs[py * cw + px.saturating_sub(1)];
            let hz = hs[(py + 1).min(ch - 1) * cw + px] - hs[py.saturating_sub(1) * cw + px];
            let shade = (1.0 - (hx + hz) * 0.9).clamp(0.55, 1.35);
            let up = if hi > lo { (h - lo) / (hi - lo) } else { 0.5 };
            let k = shade * (0.7 + 0.45 * up);
            let (x, z) = (-HALF_X + (px as f32 + 0.5) * SCALE, -HALF_Z + (py as f32 + 0.5) * SCALE);
            let col = if w.lava_fixed(x, z, 0.0) { [0.95, 0.35, 0.08] } else { grass.map(|g| (g * k).min(1.0)) };
            c.px[py * cw + px] = col;
        }
    }
    // A 25 m grid, the centre lines stronger.
    for py in 0..ch {
        for px in 0..cw {
            let (x, z) = (-HALF_X + (px as f32 + 0.5) * SCALE, -HALF_Z + (py as f32 + 0.5) * SCALE);
            let on = |v: f32, step: f32| (v / step).round() * step - v;
            if on(x, 25.0).abs() < SCALE * 0.5 || on(z, 25.0).abs() < SCALE * 0.5 {
                let strong = x.abs() < SCALE * 0.5 || z.abs() < SCALE * 0.5;
                c.set(px as i32, py as i32, [1.0, 1.0, 1.0], if strong { 0.22 } else { 0.08 });
            }
        }
    }
    // Trees under the blocks.
    for t in &w.trees {
        let (cx, cy) = c.at(t.base.x, t.base.z);
        let r = (t.radius * 0.7 / SCALE).max(1.0);
        for dy in -(r as i32)..=(r as i32) {
            for dx in -(r as i32)..=(r as i32) {
                if (dx * dx + dy * dy) as f32 <= r * r {
                    c.set(cx as i32 + dx, cy as i32 + dy, [0.12, 0.28, 0.12], 0.75);
                }
            }
        }
    }
    // Blocks, lowest first so roofs land on top; taller is lighter.
    let mut order: Vec<usize> = (0..w.blocks.len()).collect();
    order.sort_by(|&a, &b| w.blocks[a].max.y.total_cmp(&w.blocks[b].max.y));
    for i in order {
        let b = &w.blocks[i];
        let g = w.height((b.min.x + b.max.x) * 0.5, (b.min.z + b.max.z) * 0.5);
        let tall = ((b.max.y - g) / 6.0).clamp(0.0, 1.0);
        let base = kind_col(b.kind).map(|v| (v * (0.75 + 0.45 * tall)).min(1.0));
        let (x0, y0) = c.at(b.min.x, b.min.z);
        let (x1, y1) = c.at(b.max.x, b.max.z);
        let (x0, y0, x1, y1) =
            (x0.floor() as i32, y0.floor() as i32, (x1.ceil() as i32 - 1).max(x0 as i32), (y1.ceil() as i32 - 1).max(y0 as i32));
        for py in y0..=y1 {
            for px in x0..=x1 {
                let edge = px == x0 || px == x1 || py == y0 || py == y1;
                c.set(px, py, if edge { base.map(|v| v * 0.45) } else { base }, 1.0);
            }
        }
    }
    // Deaths, as a glow: yellow where most.
    if !heat.is_empty() {
        let mut field = vec![0.0f32; cw * ch];
        let r = (5.0 / SCALE) as i32;
        for p in heat {
            let (cx, cy) = c.at(p[0], p[1]);
            for dy in -r..=r {
                for dx in -r..=r {
                    let (x, y) = (cx as i32 + dx, cy as i32 + dy);
                    if x >= 0 && y >= 0 && (x as usize) < cw && (y as usize) < ch {
                        let d2 = (dx * dx + dy * dy) as f32 / (r * r) as f32;
                        if d2 < 1.0 {
                            field[y as usize * cw + x as usize] += (1.0 - d2).powi(2);
                        }
                    }
                }
            }
        }
        let top = field.iter().cloned().fold(0.0, f32::max).max(1e-3);
        for (i, f) in field.iter().enumerate() {
            let t = (f / top).sqrt();
            if t > 0.04 {
                let col = [1.0, 0.15 + 0.85 * t, 0.1 * t];
                let p = &mut c.px[i];
                for k in 0..3 {
                    p[k] += (col[k] - p[k]) * (0.25 + 0.6 * t);
                }
            }
        }
    }
    // Objectives and bases.
    for (k, p) in w.points.iter().enumerate() {
        c.ring(p.x, p.z, 12.0, [1.0, 1.0, 1.0]);
        c.text(&((b'A' + k as u8) as char).to_string(), p.x, p.z, [1.0, 1.0, 1.0]);
    }
    for (k, b) in w.bases.iter().enumerate() {
        let col = if k == 0 { [0.35, 0.6, 1.0] } else { [1.0, 0.42, 0.3] };
        c.ring(b.x, b.z, 20.0, col);
        c.text(if k == 0 { "W" } else { "E" }, b.x, b.z, col);
    }
    c.text("N", 0.0, -HALF_Z + 6.0, [1.0, 1.0, 1.0]);
    let rgb: Vec<u8> = c.px.iter().flat_map(|p| p.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)).collect();
    png::encode(cw, ch, &rgb)
}
