//! Who the player is: a random id made on first launch and kept in the
//! settings file, and the small icon shown beside their callsign. An icon
//! is 16 x 16 RGBA - one of the built-in emblems, or any PNG shrunk to fit.

use ferrite_shared::proto::{ICON_BYTES, ICON_PX};
use macroquad::prelude::{FilterMode, Image, Texture2D};

/// A fresh random id (a version 4 UUID). The randomness is the operating
/// system's, by way of the standard library's hash seeds.
pub fn new_uuid() -> [u8; 16] {
    use std::hash::{BuildHasher, Hasher};
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let mut id = [0u8; 16];
    for (k, half) in id.chunks_mut(8).enumerate() {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(nanos);
        h.write_u32(std::process::id());
        h.write_usize(k);
        half.copy_from_slice(&h.finish().to_le_bytes());
    }
    id[6] = (id[6] & 0x0F) | 0x40;
    id[8] = (id[8] & 0x3F) | 0x80;
    id
}

/// Built-in emblems. Kind 0 is "no icon"; kinds 1.. index this list.
pub const SHAPES: [&str; 9] = ["DISC", "RING", "DIAMOND", "CHEVRONS", "CROSS", "STAR", "SHIELD", "BARS", "HEX"];
/// Icon kind for a picture of the player's own.
pub const CUSTOM: u8 = 255;

pub const COLOURS: [(&str, [u8; 3]); 8] = [
    ("EMBER", [255, 117, 76]),
    ("COBALT", [105, 173, 255]),
    ("WHITE", [246, 243, 250]),
    ("GREEN", [129, 224, 186]),
    ("GOLD", [255, 210, 138]),
    ("ROSE", [255, 104, 124]),
    ("VIOLET", [186, 140, 255]),
    ("TEAL", [92, 214, 220]),
];

/// Is the point (x, y), in -1..1 with +y down, inside emblem `shape`?
fn inside(shape: usize, x: f32, y: f32) -> bool {
    let r = (x * x + y * y).sqrt();
    match shape {
        0 => r < 0.74,
        1 => (0.46..0.8).contains(&r),
        2 => x.abs() + y.abs() < 0.86,
        3 => {
            // Two chevrons pointing up.
            let v = y + 0.9 * (0.75 - x.abs());
            x.abs() < 0.75 && ((0.1..0.42).contains(&v) || (0.72..1.04).contains(&v))
        }
        4 => (x.abs() < 0.24 || y.abs() < 0.24) && x.abs().max(y.abs()) < 0.78,
        5 => {
            // Five points: the radius swings between the tips and the notches.
            let a = (-y).atan2(x) - std::f32::consts::FRAC_PI_2;
            let t = (a.rem_euclid(std::f32::consts::TAU / 5.0) / (std::f32::consts::TAU / 5.0) - 0.5).abs() * 2.0;
            r < 0.36 + 0.5 * t.powf(1.6)
        }
        6 => {
            // Flat top, straight sides, pointed foot.
            if y < 0.1 {
                y > -0.76 && x.abs() < 0.66
            } else {
                x.abs() < 0.66 * (1.0 - (y - 0.1) / 0.74)
            }
        }
        7 => x.abs() < 0.72 && [-0.52f32, 0.0, 0.52].iter().any(|c| (y - c).abs() < 0.15),
        _ => {
            let (ax, ay) = (x.abs(), y.abs());
            ax < 0.72 && ay * 0.866 + ax * 0.5 < 0.72
        }
    }
}

/// A built-in emblem: the shape in `colour` on a dark tile.
pub fn preset(shape: u8, colour: u8) -> Vec<u8> {
    let c = COLOURS[colour as usize % COLOURS.len()].1;
    let shape = shape as usize % SHAPES.len();
    let n = ICON_PX as f32;
    let mut out = Vec::with_capacity(ICON_BYTES);
    for py in 0..ICON_PX {
        for px in 0..ICON_PX {
            // 4 x 4 samples per pixel, so edges come out smooth.
            let (mut cover, mut tile) = (0.0f32, 0.0f32);
            for sy in 0..4 {
                for sx in 0..4 {
                    let x = ((px as f32 + (sx as f32 + 0.5) / 4.0) / n) * 2.0 - 1.0;
                    let y = ((py as f32 + (sy as f32 + 0.5) / 4.0) / n) * 2.0 - 1.0;
                    // The tile has rounded corners.
                    let (cx, cy) = ((x.abs() - 0.72).max(0.0), (y.abs() - 0.72).max(0.0));
                    if (cx * cx + cy * cy).sqrt() < 0.28 {
                        tile += 1.0 / 16.0;
                    }
                    if inside(shape, x, y) {
                        cover += 1.0 / 16.0;
                    }
                }
            }
            let ink = [12.0, 16.0, 32.0];
            for k in 0..3 {
                out.push((ink[k] + (c[k] as f32 - ink[k]) * cover) as u8);
            }
            out.push((tile * (0.86 + 0.14 * cover) * 255.0) as u8);
        }
    }
    out
}

/// Any picture the game can decode (PNG), cropped to a square about its
/// middle and shrunk to icon size. `None` if it is not a picture.
pub fn from_image_bytes(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.is_empty() || bytes.len() > 16 * 1024 * 1024 {
        return None;
    }
    let img = Image::from_file_with_format(bytes, None).ok()?;
    let (w, h) = (img.width as usize, img.height as usize);
    if w == 0 || h == 0 || img.bytes.len() < w * h * 4 {
        return None;
    }
    let side = w.min(h);
    let (ox, oy) = ((w - side) / 2, (h - side) / 2);
    let cell = side as f32 / ICON_PX as f32;
    let mut out = Vec::with_capacity(ICON_BYTES);
    for py in 0..ICON_PX {
        for px in 0..ICON_PX {
            // Average an 8 x 8 grid of samples over the cell, weighting
            // colour by opacity so clear pixels do not darken the edges.
            let mut acc = [0.0f32; 4];
            for sy in 0..8 {
                for sx in 0..8 {
                    let x = (ox + (((px as f32 + (sx as f32 + 0.5) / 8.0) * cell) as usize).min(side - 1)).min(w - 1);
                    let y = (oy + (((py as f32 + (sy as f32 + 0.5) / 8.0) * cell) as usize).min(side - 1)).min(h - 1);
                    let p = &img.bytes[(y * w + x) * 4..(y * w + x) * 4 + 4];
                    let a = p[3] as f32 / 255.0;
                    for k in 0..3 {
                        acc[k] += p[k] as f32 * a;
                    }
                    acc[3] += a;
                }
            }
            for k in 0..3 {
                out.push(if acc[3] > 0.0 { (acc[k] / acc[3]).round().clamp(0.0, 255.0) as u8 } else { 0 });
            }
            out.push((acc[3] / 64.0 * 255.0).round() as u8);
        }
    }
    Some(out)
}

/// An icon ready to draw. `None` unless `rgba` is exactly an icon's worth.
pub fn texture(rgba: &[u8]) -> Option<Texture2D> {
    if rgba.len() != ICON_BYTES {
        return None;
    }
    let t = Texture2D::from_rgba8(ICON_PX as u16, ICON_PX as u16, rgba);
    t.set_filter(FilterMode::Nearest);
    Some(t)
}

/// An icon as one line of text for the settings file, and back.
pub fn to_hex(rgba: &[u8]) -> String {
    rgba.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn from_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim().as_bytes();
    if s.len() != ICON_BYTES * 2 {
        return None;
    }
    s.chunks(2).map(|p| u8::from_str_radix(std::str::from_utf8(p).ok()?, 16).ok()).collect()
}
