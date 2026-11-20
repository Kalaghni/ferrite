//! Shaping the ground of a map document with a round brush: raise, lower,
//! flatten, smooth, ramp and roughen.
//!
//! The ground is a grid of heights every `CELL` (2 m) across the whole
//! terrain. A document keeps it as a preset's heights plus a delta; a
//! stroke changes the heights and the delta is written again. In half-turn
//! symmetry every stroke is also made at the turned spot, so the ground
//! stays the same under a half turn, as the blocks do.
//!
//! Players and bots walk up any ground, however steep: a slope never stops
//! anyone. Cliffs that should stop people are rock blocks.

use super::doc::MapDoc;
use super::{CELL, TER_X0, TER_Z0, TNX, TNZ};
use serde::{Deserialize, Serialize};

/// Grid points across and down.
pub const GX: usize = TNX + 1;
pub const GZ: usize = TNZ + 1;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Lift the ground by `amount` metres at the centre, less towards the rim.
    Raise,
    /// Sink it the same way.
    Lower,
    /// Bring it towards `height` (default: the ground at the centre), with
    /// `amount` the share of the way to go (0 to 1, default 1).
    Flatten,
    /// Even it out; `amount` is how much (0 to 1, default 0.5).
    Smooth,
    /// A straight slope from `at` to `to`, `radius` either side, rising from
    /// the ground at each end (or `heights`).
    Ramp,
    /// Roughen it by up to `amount` metres, in lumps `scale` metres across.
    Noise,
}

impl Mode {
    pub const ALL: [Mode; 6] = [Mode::Raise, Mode::Lower, Mode::Flatten, Mode::Smooth, Mode::Ramp, Mode::Noise];

    pub fn name(self) -> &'static str {
        match self {
            Mode::Raise => "RAISE",
            Mode::Lower => "LOWER",
            Mode::Flatten => "FLATTEN",
            Mode::Smooth => "SMOOTH",
            Mode::Ramp => "RAMP",
            Mode::Noise => "NOISE",
        }
    }
}

/// One stroke of the brush.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Stroke {
    pub mode: Mode,
    pub at: [f32; 2],
    pub radius: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heights: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f32>,
    #[serde(default)]
    pub seed: u32,
}

impl Stroke {
    pub fn new(mode: Mode, at: [f32; 2], radius: f32) -> Stroke {
        Stroke { mode, at, radius, amount: None, height: None, to: None, heights: None, scale: None, seed: 0 }
    }
}

/// The height of grid `h` at (x, z), as the world reads it: two triangles a
/// cell.
pub fn sample(h: &[f32], x: f32, z: f32) -> f32 {
    let gx = ((x - TER_X0) / CELL).clamp(0.0, TNX as f32 - 0.001);
    let gz = ((z - TER_Z0) / CELL).clamp(0.0, TNZ as f32 - 0.001);
    let (ix, iz) = (gx as usize, gz as usize);
    let (fx, fz) = (gx - ix as f32, gz - iz as f32);
    let i = iz * GX + ix;
    let (h00, h10, h01, h11) = (h[i], h[i + 1], h[i + GX], h[i + GX + 1]);
    if fx + fz < 1.0 {
        h00 + (h10 - h00) * fx + (h01 - h00) * fz
    } else {
        h11 + (h01 - h11) * (1.0 - fx) + (h10 - h11) * (1.0 - fz)
    }
}

/// Where grid point `i` is.
pub fn point(i: usize) -> (f32, f32) {
    (TER_X0 + (i % GX) as f32 * CELL, TER_Z0 + (i / GX) as f32 * CELL)
}

/// 1 at the centre to 0 at `r`, smoothly.
fn soft(d: f32, r: f32) -> f32 {
    if d >= r {
        0.0
    } else {
        0.5 * (1.0 + (std::f32::consts::PI * d / r).cos())
    }
}

/// 1 over the inner half, then easing to 0 at `r`: for flat tops.
fn plateau(d: f32, r: f32) -> f32 {
    let inner = r * 0.5;
    if d <= inner {
        1.0
    } else {
        soft(d - inner, r - inner)
    }
}

fn hash(x: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (z as u32).wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

/// Smooth value noise in -1..1, lumps about 1 apart.
fn noise(x: f32, z: f32, seed: u32) -> f32 {
    let (ix, iz) = (x.floor() as i32, z.floor() as i32);
    let (fx, fz) = (x - ix as f32, z - iz as f32);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sz) = (s(fx), s(fz));
    let a = hash(ix, iz, seed) + (hash(ix + 1, iz, seed) - hash(ix, iz, seed)) * sx;
    let b = hash(ix, iz + 1, seed) + (hash(ix + 1, iz + 1, seed) - hash(ix, iz + 1, seed)) * sx;
    (a + (b - a) * sz) * 2.0 - 1.0
}

/// What a stroke does at one spot: how strongly, and to what.
#[derive(Clone, Copy)]
enum Effect {
    /// Add this many metres.
    Add(f32),
    /// Move towards this height.
    Toward(f32),
}

/// A stroke checked and with its defaults filled in.
struct Ready {
    mode: Mode,
    at: [f32; 2],
    r: f32,
    amount: f32,
    height: f32,
    to: [f32; 2],
    ends: [f32; 2],
    scale: f32,
    seed: u32,
}

fn ready(s: &Stroke, h: &[f32]) -> Result<Ready, String> {
    let fine = |v: f32| v.is_finite();
    let inside = |p: [f32; 2]| fine(p[0]) && fine(p[1]) && p[0].abs() <= -TER_X0 && p[1].abs() <= -TER_Z0;
    if !inside(s.at) {
        return Err("the brush is off the terrain".into());
    }
    if !fine(s.radius) || !(1.0..=80.0).contains(&s.radius) {
        return Err("radius is 1 to 80 m".into());
    }
    let (default, most) = match s.mode {
        Mode::Raise | Mode::Lower => (1.0, 40.0),
        Mode::Flatten => (1.0, 1.0),
        Mode::Smooth => (0.5, 1.0),
        Mode::Ramp => (1.0, 1.0),
        Mode::Noise => (1.0, 20.0),
    };
    let amount = s.amount.unwrap_or(default);
    if !fine(amount) || !(0.0..=most).contains(&amount) {
        return Err(format!("amount for {} is 0 to {most}", s.mode.name().to_lowercase()));
    }
    let height = s.height.unwrap_or_else(|| sample(h, s.at[0], s.at[1]));
    if !fine(height) || !(-150.0..=300.0).contains(&height) {
        return Err("height is -150 to 300 m".into());
    }
    let to = match (s.mode, s.to) {
        (Mode::Ramp, None) => return Err("a ramp needs `to`, where it ends".into()),
        (_, Some(t)) if !inside(t) => return Err("`to` is off the terrain".into()),
        (_, t) => t.unwrap_or(s.at),
    };
    let ends = s.heights.unwrap_or_else(|| [sample(h, s.at[0], s.at[1]), sample(h, to[0], to[1])]);
    if !ends.iter().all(|e| fine(*e) && (-150.0..=300.0).contains(e)) {
        return Err("ramp heights are -150 to 300 m".into());
    }
    let scale = s.scale.unwrap_or(8.0);
    if !fine(scale) || !(2.0..=80.0).contains(&scale) {
        return Err("scale is 2 to 80 m".into());
    }
    Ok(Ready { mode: s.mode, at: s.at, r: s.radius, amount, height, to, ends, scale, seed: s.seed })
}

impl Ready {
    /// The stroke's weight and effect at (x, z); `h0` is the ground before.
    fn at(&self, x: f32, z: f32, h0: &[f32]) -> (f32, Effect) {
        let d = (x - self.at[0]).hypot(z - self.at[1]);
        match self.mode {
            Mode::Raise => {
                let w = soft(d, self.r);
                (w, Effect::Add(self.amount * w))
            }
            Mode::Lower => {
                let w = soft(d, self.r);
                (w, Effect::Add(-self.amount * w))
            }
            Mode::Flatten => (plateau(d, self.r) * self.amount, Effect::Toward(self.height)),
            Mode::Smooth => {
                let w = soft(d, self.r) * self.amount;
                if w <= 0.0 {
                    return (0.0, Effect::Add(0.0));
                }
                // The mean of the ground within two cells.
                let mut sum = 0.0;
                for (dx, dz) in (-2..=2).flat_map(|a| (-2..=2).map(move |b| (a, b))) {
                    sum += sample(h0, x + dx as f32 * CELL, z + dz as f32 * CELL);
                }
                (w, Effect::Toward(sum / 25.0))
            }
            Mode::Ramp => {
                let (ax, az, bx, bz) = (self.at[0], self.at[1], self.to[0], self.to[1]);
                let (vx, vz) = (bx - ax, bz - az);
                let len2 = (vx * vx + vz * vz).max(1e-6);
                let t = (((x - ax) * vx + (z - az) * vz) / len2).clamp(0.0, 1.0);
                let (px, pz) = (ax + vx * t, az + vz * t);
                let w = plateau((x - px).hypot(z - pz), self.r) * self.amount;
                (w, Effect::Toward(self.ends[0] + (self.ends[1] - self.ends[0]) * t))
            }
            Mode::Noise => {
                let w = soft(d, self.r);
                let n = 0.65 * noise(x / self.scale, z / self.scale, self.seed)
                    + 0.35 * noise(x / self.scale * 2.1 + 17.3, z / self.scale * 2.1 - 4.1, self.seed ^ 0x9e37);
                (w, Effect::Add(self.amount * w * n))
            }
        }
    }
}

/// Make stroke `s` on height grid `h`. With `half`, the turned stroke is
/// made too and the result is the same under a half turn. Returns how many
/// grid points moved.
pub fn stroke(s: &Stroke, h: &mut [f32], half: bool) -> Result<usize, String> {
    let r = ready(s, h)?;
    let h0 = h.to_vec();
    // Only points near the stroke (or its turned copy) can change.
    let reach = r.r + 2.0 * CELL;
    let (lo, hi) =
        ([r.at[0].min(r.to[0]) - reach, r.at[1].min(r.to[1]) - reach], [r.at[0].max(r.to[0]) + reach, r.at[1].max(r.to[1]) + reach]);
    let near = |x: f32, z: f32| x >= lo[0] && x <= hi[0] && z >= lo[1] && z <= hi[1];
    let mut moved = 0;
    for (i, v) in h.iter_mut().enumerate() {
        let (x, z) = point(i);
        let (mine, turned) = (near(x, z), half && near(-x, -z));
        if !mine && !turned {
            continue;
        }
        let a = if mine { r.at(x, z, &h0) } else { (0.0, Effect::Add(0.0)) };
        let b = if turned { r.at(-x, -z, &h0) } else { (0.0, Effect::Add(0.0)) };
        let (w, e) = if b.0 > a.0 { b } else { a };
        if w <= 0.0 {
            continue;
        }
        let old = *v;
        *v = match e {
            Effect::Add(dh) => old + dh,
            Effect::Toward(t) => old + (t - old) * w.min(1.0),
        };
        if (*v - old).abs() > 0.005 {
            moved += 1;
        }
    }
    Ok(moved)
}

/// The lowest ground under a footprint (its corners and middle).
pub fn low(h: &[f32], x0: f32, z0: f32, x1: f32, z1: f32) -> f32 {
    [(x0, z0), (x1, z0), (x0, z1), (x1, z1), ((x0 + x1) * 0.5, (z0 + z1) * 0.5)]
        .iter()
        .map(|&(x, z)| sample(h, x, z))
        .fold(f32::MAX, f32::min)
}

/// Keep what stood on the ground standing on it after the ground moved
/// from `old` to `new`: trees at its foot, blocks footed in it (and what
/// is stacked on them) moved up or down with it. Returns the ids of the
/// blocks moved, and how many trees.
pub fn reseat(d: &mut MapDoc, old: &[f32], new: &[f32]) -> (Vec<usize>, usize) {
    let mut trees = 0;
    for t in &mut d.trees {
        let (x, z) = (t.at[0], t.at[2]);
        let (o, n) = (sample(old, x, z), sample(new, x, z));
        if (t.at[1] - o).abs() < 0.35 && (n - o).abs() > 0.005 {
            t.at[1] += n - o;
            trees += 1;
        }
    }
    // Blocks with their foot at or in the ground move by how much the
    // lowest ground under them moved.
    let mut shift: Vec<Option<f32>> = vec![None; d.brushes.len()];
    let tops: Vec<f32> = d.brushes.iter().map(|b| b.max[1]).collect();
    for (k, b) in d.brushes.iter().enumerate() {
        let (lo_old, lo_new) = (low(old, b.min[0], b.min[2], b.max[0], b.max[2]), low(new, b.min[0], b.min[2], b.max[0], b.max[2]));
        let footed = b.min[1] <= lo_old + 0.05 && b.min[1] >= lo_old - 4.0;
        if footed && (lo_new - lo_old).abs() > 0.005 {
            shift[k] = Some(lo_new - lo_old);
        }
    }
    // Then what stands on a moved block, a few storeys up.
    for _ in 0..6 {
        let mut more = false;
        for k in 0..d.brushes.len() {
            if shift[k].is_some() {
                continue;
            }
            let b = &d.brushes[k];
            let under = (0..d.brushes.len()).find(|&j| {
                let m = &d.brushes[j];
                shift[j].is_some()
                    && (b.min[1] - tops[j]).abs() < 0.06
                    && b.min[0] < m.max[0]
                    && b.max[0] > m.min[0]
                    && b.min[2] < m.max[2]
                    && b.max[2] > m.min[2]
            });
            if let Some(j) = under {
                shift[k] = shift[j];
                more = true;
            }
        }
        if !more {
            break;
        }
    }
    let mut moved = Vec::new();
    for (k, s) in shift.iter().enumerate() {
        if let Some(dy) = s {
            d.brushes[k].min[1] += dy;
            d.brushes[k].max[1] += dy;
            moved.push(k);
        }
    }
    (moved, trees)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::doc::{Brush, Symmetry, TreeDoc};
    use crate::map::BlockKind;

    fn flat() -> MapDoc {
        MapDoc::blank("TEST")
    }

    fn turned_alike(h: &[f32]) -> bool {
        (0..h.len()).all(|i| (h[i] - h[h.len() - 1 - i]).abs() < 1e-3)
    }

    #[test]
    fn raising_lifts_the_centre_most_and_its_turned_twin_too() {
        let d = flat();
        let mut h = d.heights();
        let n = stroke(&Stroke { amount: Some(4.0), ..Stroke::new(Mode::Raise, [-60.0, 20.0], 10.0) }, &mut h, true).unwrap();
        assert!(n > 40, "{n}");
        assert!((sample(&h, -60.0, 20.0) - 4.0).abs() < 0.01);
        assert!((sample(&h, 60.0, -20.0) - 4.0).abs() < 0.01, "the turned spot rises too");
        assert!(sample(&h, -60.0, 26.0) < 4.0 && sample(&h, -60.0, 26.0) > 0.0);
        assert_eq!(sample(&h, -60.0, 40.0), 0.0);
        assert!(turned_alike(&h));
        // Without symmetry only the stroke moves.
        let mut h = d.heights();
        stroke(&Stroke { amount: Some(4.0), ..Stroke::new(Mode::Raise, [-60.0, 20.0], 10.0) }, &mut h, false).unwrap();
        assert_eq!(sample(&h, 60.0, -20.0), 0.0);
    }

    #[test]
    fn a_stroke_across_the_centre_is_not_made_twice() {
        let mut h = flat().heights();
        stroke(&Stroke { amount: Some(3.0), ..Stroke::new(Mode::Raise, [0.0, 0.0], 12.0) }, &mut h, true).unwrap();
        assert!((sample(&h, 0.0, 0.0) - 3.0).abs() < 0.01);
        assert!(turned_alike(&h));
    }

    #[test]
    fn flatten_smooth_ramp_and_noise_do_what_they_say() {
        let mut h = flat().heights();
        stroke(&Stroke { amount: Some(6.0), ..Stroke::new(Mode::Raise, [-80.0, 0.0], 14.0) }, &mut h, true).unwrap();
        // Flatten to 2 m: a level top across the inner half.
        stroke(&Stroke { height: Some(2.0), ..Stroke::new(Mode::Flatten, [-80.0, 0.0], 12.0) }, &mut h, true).unwrap();
        for dx in [-4.0, 0.0, 4.0] {
            assert!((sample(&h, -80.0 + dx, 0.0) - 2.0).abs() < 0.01);
        }
        // Smoothing pulls a spike down.
        let mut s = flat().heights();
        stroke(&Stroke { amount: Some(8.0), ..Stroke::new(Mode::Raise, [-100.0, 30.0], 3.0) }, &mut s, false).unwrap();
        let before = sample(&s, -100.0, 30.0);
        stroke(&Stroke { amount: Some(1.0), ..Stroke::new(Mode::Smooth, [-100.0, 30.0], 10.0) }, &mut s, false).unwrap();
        assert!(sample(&s, -100.0, 30.0) < before * 0.6);
        // A ramp from 0 m to 5 m, halfway at 2.5 m.
        let mut r = flat().heights();
        let ramp = Stroke { to: Some([-100.0, 0.0]), heights: Some([0.0, 5.0]), ..Stroke::new(Mode::Ramp, [-140.0, 0.0], 6.0) };
        stroke(&ramp, &mut r, true).unwrap();
        assert!((sample(&r, -120.0, 0.0) - 2.5).abs() < 0.05);
        assert!((sample(&r, -100.0, 0.0) - 5.0).abs() < 0.05);
        assert!((sample(&r, 120.0, 0.0) - 2.5).abs() < 0.05, "and its twin");
        assert!(stroke(&Stroke::new(Mode::Ramp, [0.0, 0.0], 5.0), &mut r, true).is_err(), "a ramp needs an end");
        // Noise is lumpy, bounded, and the same every time.
        let (mut a, mut b) = (flat().heights(), flat().heights());
        let n = Stroke { amount: Some(2.0), seed: 7, ..Stroke::new(Mode::Noise, [-60.0, -40.0], 20.0) };
        stroke(&n, &mut a, true).unwrap();
        stroke(&n, &mut b, true).unwrap();
        assert_eq!(a, b);
        assert!(a.iter().any(|v| *v > 0.2) && a.iter().any(|v| *v < -0.2) && a.iter().all(|v| v.abs() <= 2.0));
    }

    #[test]
    fn bad_strokes_are_refused() {
        let mut h = flat().heights();
        assert!(stroke(&Stroke::new(Mode::Raise, [999.0, 0.0], 5.0), &mut h, true).is_err());
        assert!(stroke(&Stroke::new(Mode::Raise, [0.0, 0.0], 500.0), &mut h, true).is_err());
        assert!(stroke(&Stroke { amount: Some(f32::NAN), ..Stroke::new(Mode::Raise, [0.0, 0.0], 5.0) }, &mut h, true).is_err());
        assert!(stroke(&Stroke { amount: Some(2.0), ..Stroke::new(Mode::Smooth, [0.0, 0.0], 5.0) }, &mut h, true).is_err());
    }

    #[test]
    fn heights_survive_the_document_and_things_on_the_ground_follow_it() {
        let mut d = flat();
        assert_eq!(d.symmetry, Symmetry::HalfTurn);
        // A crate on the ground with a crate on it, a floating platform,
        // and a tree.
        d.brushes.push(Brush { min: [-61.0, -0.4, 19.0], max: [-59.0, 1.1, 21.0], kind: BlockKind::Crate, tint: 0, dest: None });
        d.brushes.push(Brush { min: [-61.0, 1.1, 19.0], max: [-59.0, 2.2, 21.0], kind: BlockKind::Crate, tint: 0, dest: None });
        d.brushes.push(Brush { min: [-62.0, 6.0, 18.0], max: [-58.0, 6.3, 22.0], kind: BlockKind::Metal, tint: 0, dest: None });
        d.trees.push(TreeDoc { at: [-56.0, 0.0, 20.0], height: 8.0, radius: 2.0, tint: 0, pine: true });
        let old = d.heights();
        let mut new = old.clone();
        stroke(&Stroke { amount: Some(3.0), ..Stroke::new(Mode::Raise, [-60.0, 20.0], 16.0) }, &mut new, true).unwrap();
        d.set_heights(&new);
        assert_eq!(
            d.heights().iter().map(|v| (v * 100.0).round()).collect::<Vec<_>>(),
            new.iter().map(|v| (v * 100.0).round()).collect::<Vec<_>>()
        );
        d.validate().unwrap();
        let (moved, trees) = reseat(&mut d, &old, &new);
        assert_eq!(moved, vec![0, 1], "the crates move, the platform in the air does not");
        let lift = low(&new, -61.0, 19.0, -59.0, 21.0);
        assert!((d.brushes[0].min[1] - (lift - 0.4)).abs() < 0.01);
        assert!((d.brushes[1].min[1] - d.brushes[0].max[1]).abs() < 0.01, "the stack stays stacked");
        assert_eq!(d.brushes[2].min[1], 6.0);
        assert_eq!(trees, 1);
        assert!((d.trees[0].at[1] - sample(&new, -56.0, 20.0)).abs() < 0.01);
        // Flat again: no delta kept at all.
        d.set_heights(&old);
        assert!(d.terrain.delta.is_empty());
    }

    #[test]
    fn a_sculpt_op_lands_as_one_batch_and_later_ops_stand_on_the_new_ground() {
        use crate::map::ops::{apply, Op};
        let mut d = flat();
        let ops: Vec<Op> = serde_json::from_str(
            r#"[{"op":"sculpt","mode":"raise","at":[-80,10],"radius":12,"amount":5},
                {"op":"place","kind":"crate","at":[-80,10],"size":[1,1,1]}]"#,
        )
        .unwrap();
        let out = apply(&mut d, &ops, &|_, _| 0.0).unwrap();
        assert!(out.ground > 50);
        // The crate stands on the raised ground, not the flat ground the
        // caller knew.
        assert!((d.brushes[0].max[1] - 6.0).abs() < 0.05, "{:?}", d.brushes[0]);
        // A bad stroke changes nothing.
        let before = d.clone();
        let bad: Vec<Op> = serde_json::from_str(r#"[{"op":"sculpt","mode":"ramp","at":[0,0],"radius":5}]"#).unwrap();
        assert!(apply(&mut d, &bad, &|_, _| 0.0).is_err());
        assert_eq!(d, before);
    }
}
