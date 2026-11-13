//! Procedural battlefields: five control points in a staggered line between
//! two bases. Generated identically on client and server from a seed and a
//! map id using only integer hashing and basic arithmetic. Every layout is
//! symmetric under a 180-degree rotation so neither team has a terrain
//! advantage.

use crate::rng::Rng;
use glam::{vec2, vec3, Vec2, Vec3};
use std::collections::BinaryHeap;

pub const HALF_X: f32 = 118.0;
pub const HALF_Z: f32 = 68.0;
pub const CELL: f32 = 2.0;
pub const TNX: usize = 180;
pub const TNZ: usize = 132;
pub const TER_X0: f32 = -180.0;
pub const TER_Z0: f32 = -132.0;

pub const POINTS_XZ: [(f32, f32); 5] =
    [(-72.0, 14.0), (-36.0, -24.0), (0.0, 0.0), (36.0, 24.0), (72.0, -14.0)];
pub const BASES_XZ: [(f32, f32); 2] = [(-106.0, 0.0), (106.0, 0.0)];
pub const MAX_CRATES: usize = 64;

/// One selectable battlefield. All maps share the same footprint; they
/// differ in where the objectives sit, the shape of the ground and what is
/// built on it.
pub struct MapDef {
    pub name: &'static str,
    pub blurb: &'static str,
    pub points: [(f32, f32); 5],
    pub bases: [(f32, f32); 2],
    /// Ground palette: grass, dry patches, packed dirt around objectives.
    pub ground: [[f32; 3]; 3],
}

pub static MAPS: [MapDef; 4] = [
    MapDef {
        name: "FOUNDRY VALLEY",
        blurb: "Rolling valley, ruined foundry in the middle",
        points: POINTS_XZ,
        bases: BASES_XZ,
        ground: [[0.40, 0.47, 0.22], [0.62, 0.52, 0.25], [0.55, 0.46, 0.35]],
    },
    MapDef {
        name: "QUARRY",
        blurb: "Terraced pit - the centre point sits on the floor",
        points: [(-78.0, -20.0), (-42.0, 18.0), (0.0, 0.0), (42.0, -18.0), (78.0, 20.0)],
        bases: BASES_XZ,
        ground: [[0.53, 0.48, 0.37], [0.64, 0.57, 0.44], [0.50, 0.47, 0.44]],
    },
    MapDef {
        name: "RIDGELINE",
        blurb: "Pine forest split by a rocky ridge with three passes",
        points: [(-74.0, 22.0), (-34.0, -30.0), (0.0, 0.0), (34.0, 30.0), (74.0, -22.0)],
        bases: BASES_XZ,
        ground: [[0.29, 0.43, 0.22], [0.44, 0.49, 0.24], [0.48, 0.42, 0.32]],
    },
    MapDef {
        name: "DEPOT",
        blurb: "Flat container yard - close quarters, short sightlines",
        points: [(-74.0, -8.0), (-38.0, 10.0), (0.0, 0.0), (38.0, -10.0), (74.0, 8.0)],
        bases: BASES_XZ,
        ground: [[0.37, 0.38, 0.36], [0.46, 0.45, 0.41], [0.42, 0.41, 0.40]],
    },
];

pub fn map_def(id: u8) -> &'static MapDef {
    &MAPS[id as usize % MAPS.len()]
}

/// Look a map up by number ("2") or by name or unambiguous prefix
/// ("quarry", "ridge"), ignoring case.
pub fn map_by_name(s: &str) -> Option<u8> {
    let s = s.trim().to_ascii_lowercase();
    if s.is_empty() {
        return None;
    }
    if let Ok(n) = s.parse::<usize>() {
        return (n < MAPS.len()).then_some(n as u8);
    }
    let squash = |t: &str| t.to_ascii_lowercase().replace([' ', '-', '_'], "");
    let want = squash(&s);
    let hits: Vec<usize> = (0..MAPS.len()).filter(|&i| squash(MAPS[i].name).contains(&want)).collect();
    (hits.len() == 1).then(|| hits[0] as u8)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlockKind {
    Concrete,
    Container,
    Rock,
    Crate,
    Sandbag,
    Metal,
    Trunk,
    Step,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub min: Vec3,
    pub max: Vec3,
    pub kind: BlockKind,
    pub tint: u8,
    /// Destructible crate index (bit in the destruction mask).
    pub dest: Option<u8>,
}

#[derive(Clone, Debug)]
pub struct Tree {
    pub base: Vec3,
    pub height: f32,
    pub radius: f32,
    pub tint: u8,
    pub pine: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    pub t: f32,
    pub normal: Vec3,
    pub block: Option<usize>,
}

pub struct World {
    pub seed: u64,
    pub map: u8,
    pub heights: Vec<f32>,
    pub blocks: Vec<Block>,
    pub trees: Vec<Tree>,
    pub points: [Vec3; 5],
    pub bases: [Vec3; 2],
    pub n_crates: usize,
}

fn hash(x: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x85EB_CA6B)
        ^ (z as u32).wrapping_mul(0xC2B2_AE35)
        ^ seed.wrapping_mul(0x27D4_EB2F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / 16_777_216.0
}

pub fn vnoise(x: f32, z: f32, seed: u32) -> f32 {
    let xf = x.floor();
    let zf = z.floor();
    let (xi, zi) = (xf as i32, zf as i32);
    let (fx, fz) = (x - xf, z - zf);
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sz = fz * fz * (3.0 - 2.0 * fz);
    let a = hash(xi, zi, seed);
    let b = hash(xi + 1, zi, seed);
    let c = hash(xi, zi + 1, seed);
    let d = hash(xi + 1, zi + 1, seed);
    let ab = a + (b - a) * sx;
    let cd = c + (d - c) * sx;
    ab + (cd - ab) * sz
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn raw_height(x: f32, z: f32, s: u32) -> f32 {
    let n = |x: f32, z: f32| {
        (vnoise(x / 40.0 + 100.0, z / 40.0 + 100.0, s) - 0.5) * 8.0
            + (vnoise(x / 13.0 + 50.0, z / 13.0 + 50.0, s ^ 0x9E37) - 0.5) * 1.8
    };
    n(x, z) + n(-x, -z)
}

/// The lie of the land before objectives are levelled. Each map's shape is
/// built from terms that are unchanged by (x, z) -> (-x, -z).
fn shape(x: f32, z: f32, s: u32, map: u8) -> f32 {
    match map % MAPS.len() as u8 {
        1 => {
            // Quarry: an oval pit cut in three terraces down to a flat floor.
            let d = (x * x * 0.55 + z * z).sqrt();
            let t = 1.0 - smooth(17.0, 47.0, d);
            let f = t * 3.0;
            let q = (f.floor() + smooth(0.0, 0.4, f - f.floor())) / 3.0;
            raw_height(x, z, s) * 0.4 + 3.0 - q.min(1.0) * 9.5
        }
        2 => {
            // Ridgeline: a spine running corner to corner, cut by a saddle
            // in the middle and a low pass toward each end.
            let across = (x - 0.35 * z) / 1.0595;
            let along = (0.35 * x + z) / 1.0595;
            let ridge = 1.0 - smooth(4.0, 21.0, across.abs());
            let gap = |c: f32, w: f32| 1.0 - smooth(w * 0.4, w, (along - c).abs());
            let cut = (gap(0.0, 17.0) * 0.6 + gap(RIDGE_PASS, 13.0) + gap(-RIDGE_PASS, 13.0)).min(1.0);
            raw_height(x, z, s) * 0.55 + ridge * 10.5 * (1.0 - cut)
        }
        3 => raw_height(x, z, s) * 0.16,
        _ => raw_height(x, z, s),
    }
}

/// Distance along the Ridgeline spine of its two outer passes.
const RIDGE_PASS: f32 = 44.0;

fn full_height(x: f32, z: f32, s: u32, map: u8) -> f32 {
    let def = map_def(map);
    let mut h = shape(x, z, s, map);
    // The original valley settles objectives halfway to sea level; the other
    // maps level them where the ground already is (a pit floor stays a floor).
    let settle = if map % MAPS.len() as u8 == 0 { 0.5 } else { 1.0 };
    let mut flat = |cx: f32, cz: f32, r: f32| {
        let d = ((x - cx).powi(2) + (z - cz).powi(2)).sqrt();
        let w = 1.0 - smooth(r * 0.55, r * 1.4, d);
        if w > 0.0 {
            let target = shape(cx, cz, s, map) * settle;
            h += (target - h) * w;
        }
    };
    for (cx, cz) in def.points {
        flat(cx, cz, 13.0);
    }
    for (cx, cz) in def.bases {
        flat(cx, cz, 20.0);
    }
    // Ridge walls enclosing the playable valley.
    let ex = (x.abs() - (HALF_X + 2.0)).max(0.0);
    let ez = (z.abs() - (HALF_Z + 2.0)).max(0.0);
    let e = (ex.max(ez) / 26.0).min(1.6);
    if e > 0.0 {
        let rough = vnoise(x / 9.0, z / 9.0, s ^ 0x51) + vnoise(-x / 9.0, -z / 9.0, s ^ 0x51);
        h += smooth(0.0, 1.0, e) * 16.0 + e * 5.0 * rough;
    }
    h
}

impl World {
    /// The original map, Foundry Valley.
    pub fn new(seed: u64) -> World {
        World::with_map(seed, 0)
    }

    pub fn with_map(seed: u64, map: u8) -> World {
        let map = map % MAPS.len() as u8;
        let def = map_def(map);
        let s = (seed ^ (seed >> 32)) as u32;
        let mut heights = Vec::with_capacity((TNX + 1) * (TNZ + 1));
        for iz in 0..=TNZ {
            for ix in 0..=TNX {
                let x = TER_X0 + ix as f32 * CELL;
                let z = TER_Z0 + iz as f32 * CELL;
                heights.push(full_height(x, z, s, map));
            }
        }
        let mut w = World {
            seed,
            map,
            heights,
            blocks: Vec::new(),
            trees: Vec::new(),
            points: [Vec3::ZERO; 5],
            bases: [Vec3::ZERO; 2],
            n_crates: 0,
        };
        for (i, (x, z)) in def.points.iter().enumerate() {
            w.points[i] = vec3(*x, w.height(*x, *z), *z);
        }
        for (i, (x, z)) in def.bases.iter().enumerate() {
            w.bases[i] = vec3(*x, w.height(*x, *z), *z);
        }
        let mut rng = Rng::new(seed ^ 0xFE44_17E0 ^ ((map as u64) << 40));
        match map {
            1 => w.build_quarry(&mut rng),
            2 => w.build_ridgeline(&mut rng),
            3 => w.build_depot(&mut rng),
            _ => w.build_structures(&mut rng),
        }
        w
    }

    /// Terrain height, matching the rendered triangle mesh exactly.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        let gx = ((x - TER_X0) / CELL).clamp(0.0, TNX as f32 - 0.001);
        let gz = ((z - TER_Z0) / CELL).clamp(0.0, TNZ as f32 - 0.001);
        let (ix, iz) = (gx as usize, gz as usize);
        let (fx, fz) = (gx - ix as f32, gz - iz as f32);
        let i = iz * (TNX + 1) + ix;
        let h00 = self.heights[i];
        let h10 = self.heights[i + 1];
        let h01 = self.heights[i + TNX + 1];
        let h11 = self.heights[i + TNX + 2];
        if fx + fz < 1.0 {
            h00 + (h10 - h00) * fx + (h01 - h00) * fz
        } else {
            h11 + (h01 - h11) * (1.0 - fx) + (h10 - h11) * (1.0 - fz)
        }
    }

    pub fn terrain_normal(&self, x: f32, z: f32) -> Vec3 {
        let e = 0.6;
        let dx = self.height(x + e, z) - self.height(x - e, z);
        let dz = self.height(x, z + e) - self.height(x, z - e);
        vec3(-dx, 2.0 * e, -dz).normalize()
    }

    #[inline]
    pub fn solid(&self, i: usize, dmask: u64) -> bool {
        match self.blocks[i].dest {
            Some(d) => dmask & (1u64 << d) == 0,
            None => true,
        }
    }

    /// Nearest hit of a ray against terrain and solid blocks.
    pub fn raycast(&self, o: Vec3, d: Vec3, max_t: f32, dmask: u64) -> Option<RayHit> {
        let mut best: Option<RayHit> = None;
        let mut best_t = max_t;
        for (i, b) in self.blocks.iter().enumerate() {
            if !self.solid(i, dmask) {
                continue;
            }
            if let Some((t, n)) = ray_aabb(o, d, b.min, b.max) {
                if t < best_t {
                    best_t = t;
                    best = Some(RayHit { t, normal: n, block: Some(i) });
                }
            }
        }
        // March the heightfield, then bisect the crossing.
        let step = 1.0;
        let mut t0 = 0.0f32;
        if o.y < self.height(o.x, o.z) {
            return best;
        }
        while t0 < best_t {
            let t1 = (t0 + step).min(best_t);
            let p = o + d * t1;
            if p.y > 60.0 && d.y >= 0.0 {
                break;
            }
            if p.y < self.height(p.x, p.z) {
                let (mut lo, mut hi) = (t0, t1);
                for _ in 0..7 {
                    let mid = (lo + hi) * 0.5;
                    let q = o + d * mid;
                    if q.y < self.height(q.x, q.z) {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                let q = o + d * lo;
                return Some(RayHit { t: lo, normal: self.terrain_normal(q.x, q.z), block: None });
            }
            if t1 >= best_t {
                break;
            }
            t0 = t1;
        }
        best
    }

    pub fn los(&self, a: Vec3, b: Vec3, dmask: u64) -> bool {
        let d = b - a;
        let len = d.length();
        if len < 1e-4 {
            return true;
        }
        self.raycast(a, d / len, len, dmask).is_none()
    }

    // ---------------------------------------------------------------- build

    fn add(&mut self, cx: f32, cz: f32, hx: f32, hz: f32, y0: f32, h: f32, kind: BlockKind, tint: u8) {
        let g = self.height(cx, cz);
        let mut low = g;
        for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            low = low.min(self.height(cx + sx * hx, cz + sz * hz));
        }
        let min_y = if y0 > 0.0 { g + y0 } else { low - 0.6 };
        let dest = if kind == BlockKind::Crate && self.n_crates < MAX_CRATES {
            self.n_crates += 1;
            Some((self.n_crates - 1) as u8)
        } else {
            None
        };
        self.blocks.push(Block {
            min: vec3(cx - hx, min_y, cz - hz),
            max: vec3(cx + hx, g + y0 + h, cz + hz),
            kind,
            tint,
            dest,
        });
    }

    /// Place a block and its 180-degree mirror.
    fn sym(&mut self, cx: f32, cz: f32, hx: f32, hz: f32, y0: f32, h: f32, kind: BlockKind, tint: u8) {
        self.add(cx, cz, hx, hz, y0, h, kind, tint);
        self.add(-cx, -cz, hx, hz, y0, h, kind, tint);
    }

    fn footprint_free(&self, cx: f32, cz: f32, r: f32) -> bool {
        !self.blocks.iter().any(|b| {
            cx + r > b.min.x && cx - r < b.max.x && cz + r > b.min.z && cz - r < b.max.z
        })
    }

    fn near_objective(&self, cx: f32, cz: f32, r_point: f32, r_base: f32) -> bool {
        let near = |p: Vec3, r: f32| (cx - p.x).powi(2) + (cz - p.z).powi(2) < r * r;
        self.points.iter().any(|&p| near(p, r_point)) || self.bases.iter().any(|&p| near(p, r_base))
    }

    /// Broken perimeter wall. `sides` selects which edges to emit:
    /// bit0 west, bit1 east, bit2 north(-z), bit3 south(+z).
    #[allow(clippy::too_many_arguments)]
    fn ruin(&mut self, rng: &mut Rng, cx: f32, cz: f32, w: f32, d: f32, h: f32, x_range: (f32, f32), sides: u8) {
        let seg = 2.6;
        let t = 0.28;
        let (x0, x1) = (cx - w * 0.5, cx + w * 0.5);
        let (z0, z1) = (cz - d * 0.5, cz + d * 0.5);
        let run = |me: &mut World, rng: &mut Rng, along_x: bool, fixed: f32, a: f32, b: f32| {
            let n = ((b - a) / seg).round().max(1.0) as i32;
            let len = (b - a) / n as f32;
            for i in 0..n {
                let c = a + (i as f32 + 0.5) * len;
                let r = rng.f32();
                if r < 0.26 {
                    continue; // doorway / collapsed section
                }
                let hh = if r < 0.45 { 1.1 } else { h * rng.range(0.7, 1.0) };
                let tint = rng.below(4) as u8;
                if along_x {
                    me.sym(c, fixed, len * 0.5, t, 0.0, hh, BlockKind::Concrete, tint);
                } else {
                    me.sym(fixed, c, t, len * 0.5, 0.0, hh, BlockKind::Concrete, tint);
                }
            }
        };
        let (xa, xb) = (x0.max(x_range.0), x1.min(x_range.1));
        if sides & 1 != 0 {
            run(self, rng, false, x0, z0, z1);
        }
        if sides & 2 != 0 {
            run(self, rng, false, x1, z0, z1);
        }
        if sides & 4 != 0 {
            run(self, rng, true, z0, xa, xb);
        }
        if sides & 8 != 0 {
            run(self, rng, true, z1, xa, xb);
        }
    }

    fn crates(&mut self, rng: &mut Rng, cx: f32, cz: f32, n: usize) {
        for _ in 0..n {
            for _ in 0..6 {
                let x = cx + rng.range(-2.4, 2.4);
                let z = cz + rng.range(-2.4, 2.4);
                if self.footprint_free(x, z, 0.75) && self.footprint_free(-x, -z, 0.75) {
                    let s = rng.range(0.5, 0.62);
                    let t = rng.below(4) as u8;
                    self.sym(x, z, s, s, 0.0, s * 2.0, BlockKind::Crate, t);
                    break;
                }
            }
        }
    }

    fn rock(&mut self, rng: &mut Rng, cx: f32, cz: f32, size: f32) {
        let n = 1 + rng.below(3);
        for _ in 0..n {
            let x = cx + rng.range(-0.6, 0.6) * size;
            let z = cz + rng.range(-0.6, 0.6) * size;
            let hx = rng.range(0.5, 1.0) * size;
            let hz = rng.range(0.5, 1.0) * size;
            let h = rng.range(0.7, 1.7) * size;
            self.sym(x, z, hx, hz, 0.0, h, BlockKind::Rock, rng.below(4) as u8);
        }
    }

    /// Block at absolute heights, for things that span uneven ground.
    #[allow(clippy::too_many_arguments)]
    fn add_abs(&mut self, cx: f32, cz: f32, hx: f32, hz: f32, y0: f32, y1: f32, kind: BlockKind, tint: u8) {
        self.blocks.push(Block { min: vec3(cx - hx, y0, cz - hz), max: vec3(cx + hx, y1, cz + hz), kind, tint, dest: None });
    }

    #[allow(clippy::too_many_arguments)]
    fn sym_abs(&mut self, cx: f32, cz: f32, hx: f32, hz: f32, y0: f32, y1: f32, kind: BlockKind, tint: u8) {
        self.add_abs(cx, cz, hx, hz, y0, y1, kind, tint);
        self.add_abs(-cx, -cz, hx, hz, y0, y1, kind, tint);
    }

    /// Room for something of radius `r` here and at the mirrored spot.
    fn clear_both(&self, x: f32, z: f32, r: f32) -> bool {
        self.footprint_free(x, z, r) && self.footprint_free(-x, -z, r)
    }

    /// The two bases are built the same on every map (west one authored,
    /// east one mirrored).
    fn build_bases(&mut self) {
        use BlockKind::*;
        self.sym(-92.0, -11.0, 0.3, 5.0, 0.0, 3.2, Concrete, 0);
        self.sym(-92.0, 11.0, 0.3, 5.0, 0.0, 3.2, Concrete, 1);
        self.sym(-100.0, -17.0, 3.0, 1.25, 0.0, 2.6, Container, 0);
        self.sym(-100.0, 17.0, 3.0, 1.25, 0.0, 2.6, Container, 2);
        self.sym(-114.0, 0.0, 1.5, 4.0, 0.0, 4.2, Metal, 0);
        self.sym(-114.0, 0.0, 0.25, 0.25, 4.2, 5.0, Metal, 1);
    }

    /// A tree and its mirror. Trunks collide, canopies are scenery.
    fn plant(&mut self, rng: &mut Rng, x: f32, z: f32, collide: bool, pine: f32) {
        let height = rng.range(5.0, 9.5);
        let radius = rng.range(1.5, 2.6);
        let tint = rng.below(5) as u8;
        let pine = rng.chance(pine);
        for (px, pz) in [(x, z), (-x, -z)] {
            let base = vec3(px, self.height(px, pz), pz);
            self.trees.push(Tree { base, height, radius, tint, pine });
        }
        if collide {
            self.sym(x, z, 0.28, 0.28, 0.0, height * 0.45, BlockKind::Trunk, 0);
        }
    }

    /// Trees on the battlefield (`inside`) and on the slopes around it.
    fn forest(&mut self, rng: &mut Rng, inside: usize, outside: usize, pine: f32) {
        for _ in 0..inside {
            let x = rng.range(-HALF_X + 2.0, -1.5);
            let z = rng.range(-HALF_Z + 2.0, HALF_Z - 2.0);
            if self.near_objective(x, z, 13.0, 16.0) || !self.clear_both(x, z, 1.6) {
                continue;
            }
            self.plant(rng, x, z, true, pine);
        }
        for _ in 0..outside {
            let x = rng.range(TER_X0 + 8.0, 0.0);
            let z = rng.range(TER_Z0 + 8.0, -TER_Z0 - 8.0);
            if x.abs() < HALF_X + 4.0 && z.abs() < HALF_Z + 4.0 {
                continue;
            }
            self.plant(rng, x, z, false, pine);
        }
    }

    fn boulders(&mut self, rng: &mut Rng, n: usize, lo: f32, hi: f32) {
        for _ in 0..n {
            let x = rng.range(-HALF_X + 6.0, -2.5);
            let z = rng.range(-HALF_Z + 4.0, HALF_Z - 4.0);
            if self.near_objective(x, z, 12.5, 17.0) || !self.clear_both(x, z, 3.0) {
                continue;
            }
            let size = rng.range(lo, hi);
            self.rock(rng, x, z, size);
        }
    }

    /// Scattered cover along the route base -> A -> B -> C (and its mirror).
    fn lane_cover(&mut self, rng: &mut Rng, per_seg: usize, spread: f32) {
        use BlockKind::*;
        let chain = [self.bases[0], self.points[0], self.points[1], self.points[2]];
        for seg in chain.windows(2) {
            let (a, b) = (vec2(seg[0].x, seg[0].z), vec2(seg[1].x, seg[1].z));
            let dir = (b - a).normalize();
            let perp = vec2(-dir.y, dir.x);
            for k in 0..per_seg {
                let t = 0.2 + 0.62 * (k as f32 + 0.5) / per_seg as f32;
                let p = a + (b - a) * t + perp * rng.range(-spread, spread);
                if p.x > -3.5 || p.y.abs() > HALF_Z - 4.0 || self.near_objective(p.x, p.y, 12.0, 15.0) || !self.clear_both(p.x, p.y, 2.6) {
                    continue;
                }
                match rng.below(4) {
                    0 => self.rock(rng, p.x, p.y, 1.2),
                    1 => self.sym(p.x, p.y, 0.45, 2.4, 0.0, 1.1, Sandbag, 0),
                    2 => self.crates(rng, p.x, p.y, 3),
                    _ => self.sym(p.x, p.y, 2.2, 0.3, 0.0, rng.range(1.2, 2.4), Concrete, 3),
                }
            }
        }
    }

    /// Raised slab at the map centre with steps up its west and east faces.
    fn centre_platform(&mut self) {
        use BlockKind::*;
        self.add(0.0, 0.0, 2.6, 2.6, 0.0, 2.45, Concrete, 2);
        for i in 0..7 {
            let x = -2.6 - 0.3 - 0.6 * (6 - i) as f32;
            self.sym(x, 0.0, 0.3, 1.1, 0.0, 0.35 * (i + 1) as f32, Step, (i % 2) as u8);
        }
        self.sym(1.6, 1.6, 0.25, 0.25, 2.45, 1.0, Concrete, 1);
    }

    // ------------------------------------------------------------ Quarry

    fn build_quarry(&mut self, rng: &mut Rng) {
        use BlockKind::*;
        self.build_bases();

        // --- A / E: site office, a walled yard up on the rim ---
        self.ruin(rng, -78.0, -20.0, 12.0, 9.0, 3.2, (-200.0, 200.0), 0b1111);
        self.crates(rng, -78.0, -20.0, 4);
        self.sym(-69.5, -24.0, 0.45, 2.6, 0.0, 1.1, Sandbag, 0);
        self.sym(-74.0, -12.5, 2.6, 0.45, 0.0, 1.1, Sandbag, 1);

        // --- B / D: loading terrace with spoil containers and a conveyor ---
        self.sym(-47.0, 23.0, 3.0, 1.25, 0.0, 2.6, Container, 2);
        self.sym(-36.5, 25.5, 1.25, 3.0, 0.0, 2.6, Container, 0);
        self.sym(-47.5, 12.0, 1.25, 3.0, 0.0, 2.6, Container, 3);
        self.sym(-36.0, 11.5, 2.4, 0.45, 0.0, 1.1, Sandbag, 0);
        self.crates(rng, -42.0, 18.0, 5);
        let belt = self.height(-50.0, 32.0) + 5.5;
        for k in 0..4 {
            let x = -50.0 + 6.0 * k as f32;
            self.sym_abs(x, 32.0, 0.3, 0.3, self.height(x, 32.0) - 0.6, belt, Metal, 0);
        }
        self.sym_abs(-41.0, 32.0, 9.8, 0.55, belt, belt + 0.5, Metal, 1);

        // --- C: the crusher, down on the pit floor ---
        self.centre_platform();
        self.sym(-8.5, 6.5, 1.4, 1.4, 0.0, 7.5, Metal, 0);
        self.sym(-8.5, 6.5, 1.7, 1.7, 7.5, 0.5, Metal, 1);
        self.sym(-11.5, -4.5, 0.45, 2.4, 0.0, 1.1, Sandbag, 1);
        self.sym(-3.5, -10.0, 2.4, 0.45, 0.0, 1.1, Sandbag, 0);
        self.crates(rng, -6.0, -4.5, 4);

        // --- Cut stone and spoil on the terraces ---
        for _ in 0..34 {
            let x = rng.range(-HALF_X + 10.0, -4.0);
            let z = rng.range(-HALF_Z + 6.0, HALF_Z - 6.0);
            if self.near_objective(x, z, 12.0, 17.0) || !self.clear_both(x, z, 2.8) {
                continue;
            }
            match rng.below(3) {
                0 => {
                    let (hx, hz, h) = (rng.range(0.8, 1.7), rng.range(0.8, 1.7), rng.range(0.9, 2.3));
                    self.sym(x, z, hx, hz, 0.0, h, Concrete, rng.below(4) as u8);
                }
                1 => self.rock(rng, x, z, 1.7),
                _ => self.crates(rng, x, z, 3),
            }
        }
        self.lane_cover(rng, 4, 11.0);
        self.boulders(rng, 50, 0.8, 1.8);
        self.forest(rng, 20, 150, 0.7);
    }

    // --------------------------------------------------------- Ridgeline

    fn build_ridgeline(&mut self, rng: &mut Rng) {
        use BlockKind::*;
        self.build_bases();

        // --- A / E: walled outpost at the forest edge ---
        self.ruin(rng, -74.0, 22.0, 13.0, 10.0, 3.4, (-200.0, 200.0), 0b1111);
        self.crates(rng, -74.0, 22.0, 4);
        self.sym(-64.5, 18.0, 0.45, 2.6, 0.0, 1.1, Sandbag, 0);
        self.sym(-70.0, 30.5, 2.6, 0.45, 0.0, 1.1, Sandbag, 1);

        // --- B / D: logging camp below the outer pass ---
        self.ruin(rng, -38.0, -34.0, 9.0, 7.0, 2.8, (-200.0, 200.0), 0b1101);
        self.sym(-27.5, -26.0, 3.0, 1.25, 0.0, 2.6, Container, 1);
        self.sym(-40.0, -23.0, 2.4, 0.45, 0.0, 1.1, Sandbag, 0);
        self.sym(-26.5, -35.0, 0.45, 2.4, 0.0, 1.1, Sandbag, 1);
        self.crates(rng, -32.5, -28.0, 5);

        // --- C: watch post on the saddle (self-symmetric) ---
        self.ruin(rng, 0.0, 0.0, 18.0, 13.0, 3.6, (-200.0, 0.0), 0b1101);
        self.add(0.0, 0.0, 1.6, 1.6, 0.0, 5.0, Concrete, 2);
        self.add(0.0, 0.0, 2.0, 2.0, 5.0, 0.4, Concrete, 1);
        self.sym(-5.5, 3.5, 0.45, 2.2, 0.0, 1.1, Sandbag, 1);
        self.crates(rng, -5.0, -3.5, 3);

        // --- The crest: a wall of rock everywhere but the passes ---
        let (ux, uz) = (0.35 / 1.0595, 1.0 / 1.0595);
        let mut along = 15.0;
        while along < 76.0 {
            let (x, z) = (ux * along + rng.range(-1.2, 1.2), uz * along + rng.range(-1.0, 1.0));
            let size = rng.range(1.6, 2.4);
            if (along - RIDGE_PASS).abs() > 8.5 && z.abs() < HALF_Z - 3.0 && !self.near_objective(x, z, 13.0, 16.0) {
                self.rock(rng, x, z, size);
            }
            along += 4.2;
        }

        self.lane_cover(rng, 5, 12.0);
        self.boulders(rng, 26, 0.7, 1.5);
        self.forest(rng, 200, 190, 0.75);
    }

    // ------------------------------------------------------------- Depot

    fn build_depot(&mut self, rng: &mut Rng) {
        use BlockKind::*;
        self.build_bases();

        // --- A / E: warehouse shell ---
        self.ruin(rng, -74.0, -8.0, 16.0, 12.0, 4.0, (-200.0, 200.0), 0b1111);
        self.crates(rng, -74.0, -8.0, 5);
        self.sym(-63.0, -3.0, 0.45, 2.6, 0.0, 1.1, Sandbag, 0);

        // --- B / D: loading dock, a raised slab with steps at one end ---
        self.sym(-38.0, 16.5, 5.0, 2.0, 0.0, 1.1, Concrete, 1);
        for i in 0..3 {
            let x = -43.3 - 0.6 * (2 - i) as f32;
            self.sym(x, 16.5, 0.3, 1.2, 0.0, 0.3 * (i + 1) as f32, Step, (i % 2) as u8);
        }
        self.sym(-33.0, 3.5, 2.4, 0.45, 0.0, 1.1, Sandbag, 0);
        self.sym(-45.5, 6.5, 0.45, 2.4, 0.0, 1.1, Sandbag, 1);
        self.crates(rng, -38.0, 9.5, 5);

        // --- C: gantry crane over the central crossing (self-symmetric) ---
        let top = self.height(0.0, 0.0) + 9.0;
        for (x, z) in [(-7.0, -5.0), (-7.0, 5.0)] {
            self.sym_abs(x, z, 0.5, 0.5, self.height(x, z) - 0.6, top, Metal, 0);
        }
        self.sym_abs(0.0, -5.0, 7.8, 0.6, top, top + 0.8, Metal, 1);
        self.add_abs(0.0, 0.0, 0.8, 5.6, top + 0.8, top + 1.4, Metal, 1);
        self.sym(-4.2, 2.4, 3.0, 1.25, 0.0, 2.6, Container, 1);
        self.sym(-11.5, 0.0, 0.45, 2.4, 0.0, 1.1, Sandbag, 1);
        self.crates(rng, -5.5, -4.5, 4);

        // --- Container rows: loose grid with gaps, some stacked two high ---
        let mut gx = -98.0;
        while gx < -6.0 {
            let mut gz = -58.5;
            while gz < 59.0 {
                let (x, z) = (gx + rng.range(-1.0, 1.0), gz + rng.range(-0.8, 0.8));
                let roll = rng.f32();
                let long_x = rng.chance(0.5);
                let tint = rng.below(4) as u8;
                gz += 9.0;
                if roll < 0.42 || self.near_objective(x, z, 11.0, 17.0) || !self.clear_both(x, z, 3.6) {
                    continue;
                }
                let (hx, hz) = if long_x { (3.0, 1.25) } else { (1.25, 3.0) };
                self.sym(x, z, hx, hz, 0.0, 2.6, Container, tint);
                if roll > 0.86 {
                    self.sym(x, z, hx, hz, 2.6, 2.6, Container, (tint + 1) % 4);
                }
            }
            gx += 10.0;
        }

        // --- Barriers and light masts in the lanes ---
        for _ in 0..40 {
            let x = rng.range(-HALF_X + 8.0, -3.5);
            let z = rng.range(-HALF_Z + 4.0, HALF_Z - 4.0);
            let kind = rng.below(4);
            if self.near_objective(x, z, 10.0, 16.0) || !self.clear_both(x, z, 2.6) {
                continue;
            }
            match kind {
                0 => {
                    self.sym(x, z, 0.16, 0.16, 0.0, 7.0, Metal, 1);
                    self.sym(x, z, 0.55, 0.2, 7.0, 0.25, Metal, 0);
                }
                1 => self.sym(x, z, 0.3, 2.0, 0.0, 1.1, Concrete, 3),
                _ => self.sym(x, z, 2.0, 0.3, 0.0, 1.1, Concrete, 3),
            }
        }
        self.crates(rng, -56.0, 4.0, 3);
        self.crates(rng, -20.0, -6.0, 3);
        self.boulders(rng, 6, 0.7, 1.2);
        self.forest(rng, 10, 150, 0.3);
    }

    // ---------------------------------------------------- Foundry Valley

    fn build_structures(&mut self, rng: &mut Rng) {
        use BlockKind::*;
        self.build_bases();

        // --- A / E: walled outpost ---
        self.ruin(rng, -72.0, 14.0, 13.0, 10.0, 3.4, (-200.0, 200.0), 0b1111);
        self.crates(rng, -72.0, 14.0, 4);
        self.sym(-62.5, 10.0, 0.45, 2.6, 0.0, 1.1, Sandbag, 0);
        self.sym(-66.0, 22.5, 2.6, 0.45, 0.0, 1.1, Sandbag, 1);

        // --- B / D: container yard ---
        self.sym(-41.0, -20.5, 3.0, 1.25, 0.0, 2.6, Container, 0);
        self.sym(-32.0, -30.0, 1.25, 3.0, 0.0, 2.6, Container, 1);
        self.sym(-30.5, -19.5, 3.0, 1.25, 0.0, 2.6, Container, 2);
        self.sym(-30.5, -19.5, 3.0, 1.25, 2.6, 2.6, Container, 3);
        self.sym(-43.0, -30.5, 1.25, 3.0, 0.0, 2.6, Container, 3);
        self.sym(-36.5, -14.5, 2.4, 0.45, 0.0, 1.1, Sandbag, 0);
        self.crates(rng, -36.5, -25.0, 5);

        // --- C: the Foundry (self-symmetric) ---
        self.ruin(rng, 0.0, 0.0, 22.0, 17.0, 4.2, (-200.0, 0.0), 0b1101);
        self.add(0.0, 0.0, 2.6, 2.6, 0.0, 2.45, Concrete, 2);
        for i in 0..7 {
            let x = -2.6 - 0.3 - 0.6 * (6 - i) as f32;
            self.sym(x, 0.0, 0.3, 1.1, 0.0, 0.35 * (i + 1) as f32, Step, (i % 2) as u8);
        }
        self.sym(-7.5, 5.5, 0.7, 0.7, 0.0, 11.0, Metal, 0);
        self.sym(-7.5, 5.5, 1.0, 1.0, 11.0, 0.5, Metal, 1);
        self.sym(1.6, 1.6, 0.25, 0.25, 2.45, 1.0, Concrete, 1);
        self.crates(rng, -6.0, -4.5, 3);
        self.sym(-13.5, -3.0, 0.45, 2.4, 0.0, 1.1, Sandbag, 1);

        // --- Cover along the lanes between objectives ---
        let chain = [BASES_XZ[0], POINTS_XZ[0], POINTS_XZ[1], POINTS_XZ[2]];
        for seg in chain.windows(2) {
            let (a, b) = (vec2(seg[0].0, seg[0].1), vec2(seg[1].0, seg[1].1));
            let dir = (b - a).normalize();
            let perp = vec2(-dir.y, dir.x);
            for k in 0..5 {
                let t = 0.24 + 0.13 * k as f32;
                let p = a + (b - a) * t + perp * rng.range(-13.0, 13.0);
                if self.near_objective(p.x, p.y, 12.0, 15.0) || !self.footprint_free(p.x, p.y, 2.5) {
                    continue;
                }
                match rng.below(4) {
                    0 => self.rock(rng, p.x, p.y, 1.2),
                    1 => self.sym(p.x, p.y, 0.45, 2.4, 0.0, 1.1, Sandbag, 0),
                    2 => self.crates(rng, p.x, p.y, 3),
                    _ => self.sym(p.x, p.y, 2.2, 0.3, 0.0, rng.range(1.2, 2.4), Concrete, 3),
                }
            }
        }

        // --- Scattered rocks ---
        for _ in 0..46 {
            let x = rng.range(-HALF_X + 6.0, -1.0);
            let z = rng.range(-HALF_Z + 4.0, HALF_Z - 4.0);
            if self.near_objective(x, z, 12.5, 17.0) || !self.footprint_free(x, z, 3.0) {
                continue;
            }
            let size = rng.range(0.7, 1.6);
            self.rock(rng, x, z, size);
        }

        // --- Trees: trunks collide, canopies are scenery ---
        let tree = |me: &mut World, rng: &mut Rng, x: f32, z: f32, collide: bool| {
            let height = rng.range(5.0, 9.5);
            let radius = rng.range(1.5, 2.6);
            let tint = rng.below(5) as u8;
            let pine = rng.chance(0.4);
            for (px, pz) in [(x, z), (-x, -z)] {
                let base = vec3(px, me.height(px, pz), pz);
                me.trees.push(Tree { base, height, radius, tint, pine });
            }
            if collide {
                me.sym(x, z, 0.28, 0.28, 0.0, height * 0.45, Trunk, 0);
            }
        };
        for _ in 0..90 {
            let x = rng.range(-HALF_X + 2.0, -1.5);
            let z = rng.range(-HALF_Z + 2.0, HALF_Z - 2.0);
            if self.near_objective(x, z, 13.0, 16.0) || !self.footprint_free(x, z, 1.6) {
                continue;
            }
            tree(self, rng, x, z, true);
        }
        for _ in 0..170 {
            let x = rng.range(TER_X0 + 8.0, 0.0);
            let z = rng.range(TER_Z0 + 8.0, -TER_Z0 - 8.0);
            if x.abs() < HALF_X + 4.0 && z.abs() < HALF_Z + 4.0 {
                continue;
            }
            tree(self, rng, x, z, false);
        }
    }
}

/// Slab test. Returns entry distance and face normal; None if the origin is
/// inside the box or the ray misses.
pub fn ray_aabb(o: Vec3, d: Vec3, min: Vec3, max: Vec3) -> Option<(f32, Vec3)> {
    let mut tmin = 0.0f32;
    let mut tmax = f32::MAX;
    let mut axis = 3usize;
    let mut sign = 0.0f32;
    for a in 0..3 {
        if d[a].abs() < 1e-8 {
            if o[a] < min[a] || o[a] > max[a] {
                return None;
            }
        } else {
            let inv = 1.0 / d[a];
            let mut t1 = (min[a] - o[a]) * inv;
            let mut t2 = (max[a] - o[a]) * inv;
            let mut s = -1.0;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
                s = 1.0;
            }
            if t1 > tmin {
                tmin = t1;
                axis = a;
                sign = s;
            }
            tmax = tmax.min(t2);
            if tmin > tmax {
                return None;
            }
        }
    }
    if axis == 3 {
        return None;
    }
    let mut n = Vec3::ZERO;
    n[axis] = sign;
    Some((tmin, n))
}

// ------------------------------------------------------------------ nav

pub const NAV_CELL: f32 = 1.5;
/// Goals: the five points, then the two bases.
pub const NAV_GOALS: usize = 7;
pub const NAV_LANES: usize = 3;
const DIRS: [(i32, i32); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];

/// Precomputed flow fields: for every goal and lane preference, each cell
/// stores the direction of the cheapest route. Bots just read the field.
pub struct Nav {
    pub w: usize,
    pub h: usize,
    pub blocked: Vec<bool>,
    fields: Vec<Vec<u8>>,
}

impl Nav {
    pub fn build(world: &World) -> Nav {
        let w = (2.0 * HALF_X / NAV_CELL).ceil() as usize;
        let h = (2.0 * HALF_Z / NAV_CELL).ceil() as usize;
        let mut blocked = vec![false; w * h];
        for cz in 0..h {
            for cx in 0..w {
                let x = -HALF_X + (cx as f32 + 0.5) * NAV_CELL;
                let z = -HALF_Z + (cz as f32 + 0.5) * NAV_CELL;
                let g = world.height(x, z);
                let r = 0.6;
                blocked[cz * w + cx] = world.blocks.iter().any(|b| {
                    x + r > b.min.x
                        && x - r < b.max.x
                        && z + r > b.min.z
                        && z - r < b.max.z
                        && b.max.y - g > 0.45
                        && b.min.y - g < 1.9
                });
            }
        }
        let mut near = vec![false; w * h];
        for cz in 0..h as i32 {
            for cx in 0..w as i32 {
                near[cz as usize * w + cx as usize] = DIRS.iter().any(|&(dx, dz)| {
                    let (nx, nz) = (cx + dx, cz + dz);
                    nx >= 0 && nz >= 0 && nx < w as i32 && nz < h as i32 && blocked[nz as usize * w + nx as usize]
                });
            }
        }
        let mut nav = Nav { w, h, blocked, fields: Vec::new() };
        for goal in 0..NAV_GOALS {
            let gp = if goal < 5 { world.points[goal] } else { world.bases[goal - 5] };
            for lane in 0..NAV_LANES {
                let f = nav.field(gp.x, gp.z, lane, &near);
                nav.fields.push(f);
            }
        }
        nav
    }

    pub fn cell(&self, x: f32, z: f32) -> (usize, usize) {
        let cx = (((x + HALF_X) / NAV_CELL) as i32).clamp(0, self.w as i32 - 1);
        let cz = (((z + HALF_Z) / NAV_CELL) as i32).clamp(0, self.h as i32 - 1);
        (cx as usize, cz as usize)
    }

    pub fn center(&self, cx: usize, cz: usize) -> Vec2 {
        vec2(-HALF_X + (cx as f32 + 0.5) * NAV_CELL, -HALF_Z + (cz as f32 + 0.5) * NAV_CELL)
    }

    pub fn is_blocked(&self, x: f32, z: f32) -> bool {
        if x.abs() > HALF_X - 1.0 || z.abs() > HALF_Z - 1.0 {
            return true;
        }
        let (cx, cz) = self.cell(x, z);
        self.blocked[cz * self.w + cx]
    }

    fn field(&self, gx: f32, gz: f32, lane: usize, near: &[bool]) -> Vec<u8> {
        let (w, h) = (self.w, self.h);
        let mut dist = vec![u32::MAX; w * h];
        let mut heap = BinaryHeap::new();
        let (sx, sz) = self.cell(gx, gz);
        dist[sz * w + sx] = 0;
        heap.push((std::cmp::Reverse(0u32), sx, sz));
        while let Some((std::cmp::Reverse(d), cx, cz)) = heap.pop() {
            if d > dist[cz * w + cx] {
                continue;
            }
            for (k, &(dx, dz)) in DIRS.iter().enumerate() {
                let (nx, nz) = (cx as i32 + dx, cz as i32 + dz);
                if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
                    continue;
                }
                let (nx, nz) = (nx as usize, nz as usize);
                let ni = nz * w + nx;
                if k >= 4 && (self.blocked[cz * w + nx] || self.blocked[nz * w + cx]) {
                    continue; // no cutting corners
                }
                let z = -HALF_Z + (nz as f32 + 0.5) * NAV_CELL;
                let lane_cost = match lane {
                    1 => 1.0 + ((20.0 - z).max(0.0) / 40.0),
                    2 => 1.0 + ((z + 20.0).max(0.0) / 40.0),
                    _ => 1.0,
                };
                let mut c = if k >= 4 { 141.0 } else { 100.0 } * lane_cost;
                if self.blocked[ni] {
                    c *= 150.0;
                } else if near[ni] {
                    c *= 1.7;
                }
                let nd = d.saturating_add(c as u32);
                if nd < dist[ni] {
                    dist[ni] = nd;
                    heap.push((std::cmp::Reverse(nd), nx, nz));
                }
            }
        }
        let mut field = vec![8u8; w * h];
        for cz in 0..h {
            for cx in 0..w {
                let mut best = dist[cz * w + cx];
                for (k, &(dx, dz)) in DIRS.iter().enumerate() {
                    let (nx, nz) = (cx as i32 + dx, cz as i32 + dz);
                    if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
                        continue;
                    }
                    let (nx, nz) = (nx as usize, nz as usize);
                    if k >= 4 && (self.blocked[cz * w + nx] || self.blocked[nz * w + cx]) {
                        continue;
                    }
                    if dist[nz * w + nx] < best {
                        best = dist[nz * w + nx];
                        field[cz * w + cx] = k as u8;
                    }
                }
            }
        }
        field
    }

    /// Unit XZ direction to walk from (x, z) toward `goal`. Looks a few
    /// cells ahead along the field so paths are smooth, not 8-way staircase.
    pub fn dir(&self, goal: usize, lane: usize, x: f32, z: f32) -> Vec2 {
        let f = &self.fields[goal * NAV_LANES + lane % NAV_LANES];
        let (mut cx, mut cz) = self.cell(x, z);
        for _ in 0..3 {
            let k = f[cz * self.w + cx];
            if k >= 8 {
                break;
            }
            cx = (cx as i32 + DIRS[k as usize].0) as usize;
            cz = (cz as i32 + DIRS[k as usize].1) as usize;
        }
        (self.center(cx, cz) - vec2(x, z)).normalize_or_zero()
    }
}
