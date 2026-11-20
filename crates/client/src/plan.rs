//! Top-down plans of the battlefields for the map pickers (the launcher's
//! Host & Play page and the lobby's vote). Each plan is drawn from the map
//! data itself - ground, structures, forest, water, lava and tunnels - so a
//! new map gets one with no extra art. They are rasterised once per run on a
//! worker thread and kept as textures; until a plan arrives its card shows
//! the map's ground palette instead.
//!
//! Every map has two plans. The thumbnail is coarse and bold so the maps
//! tell apart at a glance: the ground palette, forest as dark masses,
//! buildings merged into a few solid shapes (the tallest brightest), and
//! water, lava and tunnels in their own colours. The full plan, for large
//! views, adds single trees, contour lines and finer building shapes.

use ferrite_shared::map::doc::MapDoc;
use ferrite_shared::map::{map_def, spillway, BlockKind, MapDef, World, HALF_X, HALF_Z, MAPS};
use ferrite_shared::{mode, FLAG_POINT, NUM_POINTS, POINT_NAMES};
use macroquad::prelude::*;
use std::cell::RefCell;
use std::sync::{mpsc, Arc};

/// Width over height of every plan: all maps share one footprint.
pub const ASPECT: f32 = HALF_X / HALF_Z;

/// Seed the plans are drawn with. Seeds only roughen the terrain; where
/// everything stands is the same in every match.
const SEED: u64 = 0x00F3_2217_E000;

/// Plans narrower than this on screen use the thumbnail.
const THUMB_BELOW: f32 = 200.0;

/// How a plan is drawn.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Detail {
    Thumb,
    Full,
}

impl Detail {
    /// Pixels per metre.
    fn px(self) -> f32 {
        match self {
            Detail::Thumb => 0.5,
            Detail::Full => 1.5,
        }
    }
}

/// A plan as pixels: `w` x `h`, RGBA.
pub struct Raster {
    pub w: u16,
    pub h: u16,
    pub rgba: Vec<u8>,
}

struct Plans {
    rx: Option<mpsc::Receiver<(u8, [Raster; 2])>>,
    /// Thumbnail and full plan per map.
    tex: Vec<Option<[Texture2D; 2]>>,
    started: bool,
}

thread_local! {
    static PLANS: RefCell<Plans> = const { RefCell::new(Plans { rx: None, tex: Vec::new(), started: false }) };
}

/// Start drawing the plans in the background, if that has not begun, and
/// take any that are finished. Cheap; call it every frame a picker is up.
fn pump() {
    PLANS.with(|p| {
        let mut p = p.borrow_mut();
        if !p.started {
            p.started = true;
            p.tex = vec![None; MAPS.len()];
            let (tx, rx) = mpsc::channel();
            let spawned = std::thread::Builder::new().name("map-plans".into()).spawn(move || {
                for id in 0..MAPS.len() as u8 {
                    let world = World::with_map(SEED, id);
                    let plans = [raster_world(&world, Detail::Thumb), raster_world(&world, Detail::Full)];
                    if tx.send((id, plans)).is_err() {
                        return;
                    }
                }
            });
            if spawned.is_ok() {
                p.rx = Some(rx);
            }
        }
        let mut done = Vec::new();
        if let Some(rx) = &p.rx {
            while let Ok(plan) = rx.try_recv() {
                done.push(plan);
            }
        }
        for (id, [thumb, full]) in done {
            let tex = |r: &Raster| {
                let t = Texture2D::from_rgba8(r.w, r.h, &r.rgba);
                t.set_filter(FilterMode::Linear);
                t
            };
            if let Some(slot) = p.tex.get_mut(id as usize) {
                *slot = Some([tex(&thumb), tex(&full)]);
            }
        }
    });
}

fn texture(id: u8, detail: Detail) -> Option<Texture2D> {
    pump();
    PLANS.with(|p| p.borrow().tex.get(id as usize).cloned().flatten().map(|t| t[(detail == Detail::Full) as usize].clone()))
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn scale(a: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

/// `c` with its colour pushed away from grey by `k`.
fn saturate(c: [f32; 3], k: f32) -> [f32; 3] {
    let l = 0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2];
    [l + (c[0] - l) * k, l + (c[1] - l) * k, l + (c[2] - l) * k]
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Grow (`grow`) or shrink a mask by a square of radius `r` pixels.
fn morph(m: &[bool], w: usize, h: usize, r: usize, grow: bool) -> Vec<bool> {
    if r == 0 {
        return m.to_vec();
    }
    let pass = |src: &[bool], horizontal: bool| {
        let mut out = vec![false; w * h];
        for j in 0..h {
            for i in 0..w {
                let (c, n) = if horizontal { (i, w) } else { (j, h) };
                let range = c.saturating_sub(r)..(c + r + 1).min(n);
                let at = |k: usize| if horizontal { src[j * w + k] } else { src[k * w + i] };
                out[j * w + i] = if grow { range.clone().any(at) } else { range.clone().all(at) };
            }
        }
        out
    };
    pass(&pass(m, true), false)
}

/// The plan of a map document, for the map maker's list of maps. Builds
/// the world first: call it off the frame loop.
pub fn raster_doc(doc: &MapDoc, detail: Detail) -> Result<Raster, String> {
    let (points, bases) = doc.layout();
    let def =
        MapDef { name: "", blurb: "", points: points.map(|[x, z]| (x, z)), bases: bases.map(|[x, z]| (x, z)), ground: doc.look.ground };
    // Built as a map id with no special rules for the plan to draw.
    let world = Arc::new(doc.clone()).build(SEED, 0)?;
    Ok(raster_with(&world, &def, detail))
}

/// The plan of `world`: north (-z) up, west (-x, EMBER's base) left.
fn raster_world(world: &World, detail: Detail) -> Raster {
    raster_with(world, map_def(world.map), detail)
}

fn raster_with(world: &World, def: &MapDef, detail: Detail) -> Raster {
    let id = world.map;
    let px = detail.px();
    let thumb = detail == Detail::Thumb;
    let (w, h) = ((2.0 * HALF_X * px).round() as usize, (2.0 * HALF_Z * px).round() as usize);
    let at = |i: usize, j: usize| (-HALF_X + (i as f32 + 0.5) / px, -HALF_Z + (j as f32 + 0.5) / px);
    let to_px = |x: f32, z: f32| ((x + HALF_X) * px, (z + HALF_Z) * px);
    let [grass, dry, dirt] = def.ground;
    let night = [0.082, 0.094, 0.165];
    let light = Vec3::new(-0.55, 0.75, -0.4).normalize();

    let heights: Vec<f32> = (0..w * h)
        .map(|k| {
            let (x, z) = at(k % w, k / w);
            world.height(x, z)
        })
        .collect();
    // The lie of the land: low ground grassy, high ground dry.
    let mut sorted = heights.clone();
    sorted.sort_by(f32::total_cmp);
    let (lo, hi) = (sorted[sorted.len() * 15 / 100], sorted[sorted.len() * 85 / 100]);

    // Forest, as a density: in the thumbnail it darkens the ground in
    // masses instead of a scatter of single trees.
    let mut forest = vec![0.0f32; w * h];
    if thumb {
        for t in &world.trees {
            let (i, j) = to_px(t.base.x, t.base.z);
            if i >= 0.0 && j >= 0.0 && (i as usize) < w && (j as usize) < h {
                forest[j as usize * w + i as usize] += 1.0;
            }
        }
        // Spread each tree over about 12 m around it.
        let r = (6.0 * px).ceil() as isize;
        for _ in 0..2 {
            let mut out = vec![0.0f32; w * h];
            for j in 0..h as isize {
                for i in 0..w as isize {
                    let mut sum = 0.0;
                    for d in -r..=r {
                        let k = (i + d).clamp(0, w as isize - 1);
                        sum += forest[j as usize * w + k as usize];
                    }
                    out[j as usize * w + i as usize] = sum / (2 * r + 1) as f32;
                }
            }
            for j in 0..h as isize {
                for i in 0..w as isize {
                    let mut sum = 0.0;
                    for d in -r..=r {
                        let k = (j + d).clamp(0, h as isize - 1);
                        sum += out[k as usize * w + i as usize];
                    }
                    forest[j as usize * w + i as usize] = sum / (2 * r + 1) as f32;
                }
            }
        }
    }
    // Trees per square metre in a thick wood, near enough.
    let wood = 1.0 / 60.0 / (px * px);
    let canopy = scale(mix(grass, [0.07, 0.2, 0.12], 0.45), 0.55);

    let mut img = vec![[0.0f32; 3]; w * h];
    for j in 0..h {
        for i in 0..w {
            let (x, z) = at(i, j);
            let k = j * w + i;
            let n = world.terrain_normal(x, z);
            let rise = ((heights[k] - lo) / (hi - lo).max(0.5)).clamp(0.0, 1.0);
            let mut c = mix(grass, dry, 0.1 + 0.55 * rise + (1.0 - n.y) * 4.0);
            // Packed dirt around the objectives and at the bases.
            let near = def.points.iter().map(|&(px, pz)| ((x - px).hypot(z - pz) - 6.0) / 12.0).fold(1.0f32, f32::min);
            let home = def.bases.iter().map(|&(bx, bz)| ((x - bx).hypot(z - bz) - 14.0) / 10.0).fold(1.0f32, f32::min);
            c = mix(c, dirt, 1.0 - near.min(home));
            c = saturate(c, 1.35);
            c = scale(c, 0.7 + 0.42 * n.dot(light).max(0.0));
            if thumb {
                c = mix(c, canopy, smooth(0.45, 1.0, forest[k] / wood) * 0.85);
            } else {
                let band = |v: f32| (v / 3.0).floor();
                let hb = band(heights[k]);
                if (i + 1 < w && band(heights[k + 1]) != hb) || (j + 1 < h && band(heights[k + w]) != hb) {
                    c = mix(c, [0.72, 0.78, 0.9], 0.16);
                }
            }
            // A little darker and cooler than the real thing, to sit on glass.
            c = mix(scale(c, 0.82), night, 0.12);
            if world.lava_fixed(x, z, 0.0) {
                c = if thumb {
                    [1.0, 0.45, 0.12]
                } else {
                    mix([1.0, 0.42, 0.16], [1.0, 0.7, 0.35], ((x * 0.3).sin() * (z * 0.3).cos()).abs())
                };
            } else if id == spillway::MAP_ID {
                if let Some(y) = spillway::water_surface(world, x, z, spillway::FULL) {
                    if heights[k] < y {
                        c = [0.25, 0.55, 0.85];
                    }
                }
            }
            img[k] = c;
        }
    }

    // Single trees, in the full plan only.
    if !thumb {
        for t in &world.trees {
            let r = (t.radius * 0.9).max(1.1) * px;
            let (cx, cz) = to_px(t.base.x, t.base.z);
            let dark = scale(mix(grass, [0.1, 0.22, 0.16], if t.pine { 0.55 } else { 0.25 }), 0.5);
            for j in ((cz - r).floor().max(0.0) as usize)..((cz + r).ceil().min(h as f32) as usize) {
                for i in ((cx - r).floor().max(0.0) as usize)..((cx + r).ceil().min(w as f32) as usize) {
                    let d = (i as f32 + 0.5 - cx).hypot(j as f32 + 0.5 - cz);
                    if d < r {
                        img[j * w + i] = mix(img[j * w + i], dark, 0.75 * (1.0 - (d / r).powi(4)));
                    }
                }
            }
        }
    }

    // Structures: everything 3 m or more across, as a mask with the height
    // it stands to. Close gaps (so a cluster becomes one shape), then open
    // (so thin walls and specks go), and drop what is left too small.
    // Landmarks - anything standing 12 m or more, however slim, such as
    // Relay's mast and towers or Slag Cathedral's nave - always stay, and
    // stand out.
    let blocks: Vec<_> = world
        .blocks
        .iter()
        .filter(|b| !matches!(b.kind, BlockKind::Trunk | BlockKind::Crate))
        .map(|b| (b, b.max.y - world.height((b.min.x + b.max.x) * 0.5, (b.min.z + b.max.z) * 0.5)))
        .collect();
    let landmark_at = 12.0;
    let mut solid = vec![false; w * h];
    let mut tall = vec![0.0f32; w * h];
    for &(b, up) in &blocks {
        let (dx, dz) = (b.max.x - b.min.x, b.max.z - b.min.z);
        if dx.max(dz) < 3.0 && !(up >= landmark_at && dx.min(dz) >= 0.4) {
            continue;
        }
        let (i0, j0) = to_px(b.min.x, b.min.z);
        let (i1, j1) = to_px(b.max.x, b.max.z);
        let span = |a: f32, b: f32, n: usize| {
            let lo = a.round().clamp(0.0, n as f32) as usize;
            (lo, (b.round().clamp(0.0, n as f32) as usize).max(lo + 1).min(n))
        };
        let ((i0, i1), (j0, j1)) = (span(i0, i1, w), span(j0, j1, h));
        for j in j0..j1 {
            for i in i0..i1 {
                solid[j * w + i] = true;
                tall[j * w + i] = tall[j * w + i].max(up);
            }
        }
    }
    let (close_m, open_m, least_m2) = if thumb { (6.0, 3.0, 100.0) } else { (5.0, 2.0, 30.0) };
    let r = |m: f32| (m * px * 0.5).round() as usize;
    let mut shape = morph(&morph(&solid, w, h, r(close_m), true), w, h, r(close_m), false);
    shape = morph(&morph(&shape, w, h, r(open_m).max(1), false), w, h, r(open_m).max(1), true);
    // Drop what is too small to see.
    let least = (least_m2 * px * px).ceil() as usize;
    let mut seen = vec![false; w * h];
    for start in 0..w * h {
        if !shape[start] || seen[start] {
            continue;
        }
        let mut part = vec![start];
        seen[start] = true;
        let mut k = 0;
        while k < part.len() {
            let c = part[k];
            let (i, j) = (c % w, c / w);
            let next = [(i > 0).then(|| c - 1), (i + 1 < w).then(|| c + 1), (j > 0).then(|| c - w), (j + 1 < h).then(|| c + w)];
            for n in next.into_iter().flatten() {
                if shape[n] && !seen[n] {
                    seen[n] = true;
                    part.push(n);
                }
            }
            k += 1;
        }
        if part.len() < least {
            for c in part {
                shape[c] = false;
            }
        }
    }
    // Heights for the filled-in parts: the tallest thing nearby.
    let reach = r(close_m) as isize + 1;
    let height_at = |k: usize| {
        if tall[k] > 0.0 {
            return tall[k];
        }
        let (i, j) = ((k % w) as isize, (k / w) as isize);
        let mut best = 0.0f32;
        for dj in -reach..=reach {
            for di in -reach..=reach {
                let (a, b) = (i + di, j + dj);
                if a >= 0 && b >= 0 && (a as usize) < w && (b as usize) < h {
                    best = best.max(tall[b as usize * w + a as usize]);
                }
            }
        }
        best
    };
    // Landmarks go back in, grown to stay visible at any size.
    let high: Vec<bool> = tall.iter().map(|&t| t >= landmark_at).collect();
    let high = morph(&high, w, h, 2, true);
    let heights_of: Vec<f32> = (0..w * h).map(|k| if shape[k] || high[k] { height_at(k) } else { 0.0 }).collect();
    let landmark: Vec<bool> = (0..w * h).map(|k| high[k] || (shape[k] && heights_of[k] >= landmark_at)).collect();
    for k in 0..w * h {
        shape[k] |= landmark[k];
    }
    // Buildings are dark against the ground, with a pale rim; landmarks are
    // bright, with a dark rim. Both cast a shadow to the south-east.
    let rim = morph(&shape, w, h, 1, true);
    let rim_high = morph(&landmark, w, h, 1, true);
    let shadow = r(3.0).max(1);
    for j in 0..h {
        for i in 0..w {
            let k = j * w + i;
            if shape[k] {
                img[k] = if landmark[k] {
                    mix([0.86, 0.84, 0.8], [1.0, 0.95, 0.82], smooth(landmark_at, landmark_at * 1.8, heights_of[k]))
                } else {
                    mix([0.15, 0.16, 0.21], [0.27, 0.28, 0.34], smooth(2.0, landmark_at, heights_of[k]))
                };
                continue;
            }
            if rim_high[k] {
                img[k] = scale(img[k], 0.35);
            } else if rim[k] {
                img[k] = mix(img[k], [0.9, 0.92, 0.96], 0.4);
            } else if i >= shadow && j >= shadow && shape[k - shadow - shadow * w] {
                img[k] = scale(img[k], 0.7);
            }
        }
    }

    // Tunnels: their roofed lengths, dashed.
    let dash = 5.0;
    let lw = if thumb { 1.0 } else { 1.2 * px };
    for t in &world.tunnels {
        for s in 0..t.pts.len().saturating_sub(1) {
            let (a, b) = (t.pts[s], t.pts[s + 1]);
            if t.s[s] < t.portals[0] || t.s[s + 1] > t.portals[1] {
                continue;
            }
            let steps = ((b - a).length() * px * 3.0).ceil().max(1.0) as usize;
            for q in 0..=steps {
                let f = q as f32 / steps as f32;
                if ((t.s[s] + (t.s[s + 1] - t.s[s]) * f) / dash) as i64 % 2 == 1 {
                    continue;
                }
                let p = a + (b - a) * f;
                let (cx, cz) = to_px(p.x, p.z);
                for j in ((cz - lw).floor().max(0.0) as usize)..((cz + lw).ceil().min(h as f32) as usize) {
                    for i in ((cx - lw).floor().max(0.0) as usize)..((cx + lw).ceil().min(w as f32) as usize) {
                        img[j * w + i] = [1.0, 0.78, 0.4];
                    }
                }
            }
        }
    }

    let mut rgba = Vec::with_capacity(w * h * 4);
    for c in img {
        rgba.extend(c.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8));
        rgba.push(255);
    }
    Raster { w: w as u16, h: h as u16, rgba }
}

/// What to mark on a plan.
#[derive(Clone, Copy)]
pub struct Marks {
    /// Show it from the far side: COBALT's base on the left.
    pub flip: bool,
    /// The objectives of this game mode; `None` marks all five, dimmed.
    pub mode: Option<u8>,
    pub alpha: f32,
}

fn rgb(c: [f32; 3], a: f32) -> Color {
    Color::new(c[0], c[1], c[2], a)
}

/// The map's ground palette in three bands, while its plan is being drawn.
fn palette(id: u8, x: f32, y: f32, w: f32, h: f32, a: f32) {
    let night = [0.082, 0.094, 0.165];
    for (k, c) in map_def(id).ground.iter().enumerate() {
        draw_rectangle(x, y + h * k as f32 / 3.0, w, h / 3.0 + 0.5, rgb(mix(scale(*c, 0.6), night, 0.3), a));
    }
}

/// A narrow slice through the middle of map `id`'s thumbnail, for a list
/// too tight for a plan: the ground, and whatever stands at the centre.
pub fn swatch(id: u8, x: f32, y: f32, w: f32, h: f32, a: f32) {
    match texture(id, Detail::Thumb) {
        Some(t) => {
            let (tw, th) = (t.width(), t.height());
            let sw = (th * w / h.max(1.0)).min(tw);
            let src = Rect::new((tw - sw) / 2.0, 0.0, sw, th);
            let params = DrawTextureParams { dest_size: Some(vec2(w, h)), source: Some(src), ..Default::default() };
            draw_texture_ex(&t, x, y, Color::new(1.0, 1.0, 1.0, a), params);
        }
        None => palette(id, x, y, w, h, a),
    }
}

/// Draw the plan of map `id` into `w` x `h` at (x, y), with the bases and
/// the objectives `marks` asks for.
pub fn draw(id: u8, x: f32, y: f32, w: f32, h: f32, marks: Marks) {
    let def = map_def(id);
    let a = marks.alpha;
    let detail = if w < THUMB_BELOW { Detail::Thumb } else { Detail::Full };
    match texture(id, detail) {
        Some(t) => draw_texture_ex(
            &t,
            x,
            y,
            Color::new(1.0, 1.0, 1.0, a),
            DrawTextureParams { dest_size: Some(vec2(w, h)), flip_x: marks.flip, flip_y: marks.flip, ..Default::default() },
        ),
        None => palette(id, x, y, w, h, a),
    }
    let f = if marks.flip { -1.0 } else { 1.0 };
    let to = |(px, pz): (f32, f32)| (x + (px * f + HALF_X) / (2.0 * HALF_X) * w, y + (pz * f + HALF_Z) / (2.0 * HALF_Z) * h);
    let r = (w * 0.021).clamp(1.6, 9.0);
    let ring = (r * 0.4).clamp(1.0, 1.5);
    let dark = Color::new(0.047, 0.063, 0.125, 0.92 * a);
    for (t, b) in def.bases.iter().enumerate() {
        let (bx, by) = to(*b);
        let s = r * 1.15;
        draw_rectangle(bx - s - ring, by - s - ring, 2.0 * (s + ring), 2.0 * (s + ring), dark);
        draw_rectangle(bx - s, by - s, 2.0 * s, 2.0 * s, rgb(crate::scene::team_col(t as u8), a));
    }
    let letters = w >= 150.0;
    for k in 0..NUM_POINTS {
        let (used, flag) = match marks.mode {
            None => (true, None),
            Some(mode::CONQUEST) => (true, None),
            Some(mode::KOTH) => (k == 2, None),
            Some(mode::CTF) => (FLAG_POINT.contains(&k), FLAG_POINT.iter().position(|&p| p == k)),
            Some(_) => (false, None),
        };
        if !used {
            continue;
        }
        let (px, py) = to(def.points[k]);
        let dim = if marks.mode.is_none() { 0.75 } else { 1.0 };
        match flag {
            Some(team) => {
                draw_poly(px, py, 3, r * 1.1 + ring * 1.4, 90.0, dark);
                draw_poly(px, py, 3, r * 1.1, 90.0, rgb(crate::scene::team_col(team as u8), a));
            }
            None => {
                draw_circle(px, py, r + ring, dark);
                draw_circle(px, py, r, rgb(crate::scene::NEUTRAL_COL, a * dim));
                if letters {
                    let size = (r * 1.6).clamp(10.0, 15.0);
                    crate::ui::text_c(POINT_NAMES[k], px, py + size * 0.36, size, Color::new(0.047, 0.063, 0.125, a));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_map_has_plans_with_ground_and_buildings() {
        for id in 0..MAPS.len() as u8 {
            let world = World::with_map(SEED, id);
            for detail in [Detail::Thumb, Detail::Full] {
                let Raster { w, h, rgba } = raster_world(&world, detail);
                assert_eq!(rgba.len(), w as usize * h as usize * 4);
                assert!((w as f32 / h as f32 - ASPECT).abs() < 0.02);
                // Not one flat colour, and something built on it.
                let first = &rgba[..3];
                let differ = rgba.chunks(4).filter(|p| &p[..3] != first).count();
                assert!(differ > rgba.len() / 8, "{} {detail:?} looks blank", MAPS[id as usize].name);
                let built = rgba.chunks(4).filter(|p| p[..3] == [38, 41, 54] || p[..3] == [219, 214, 204]).count();
                assert!(built > 10, "{} {detail:?} shows no structures", MAPS[id as usize].name);
            }
        }
    }
}
