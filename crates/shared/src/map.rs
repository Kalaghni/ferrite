//! Procedural battlefields: five control points in a staggered line between
//! two bases. Generated identically on client and server from a seed and a
//! map id using only integer hashing and basic arithmetic. Every layout is
//! symmetric under a 180-degree rotation so neither team has a terrain
//! advantage.

pub mod canopy;
pub mod fallen_giant;
pub mod landmarks;
pub mod relay;
pub mod slag_cathedral;
pub mod spillway;
pub mod tunnels;

use crate::rng::Rng;
use glam::{vec2, vec3, Vec2, Vec3};
use std::collections::BinaryHeap;

/// Every battlefield was first laid out on a 236 x 136 m footprint and then
/// spread out by this factor: objectives, bases and the lie of the land move
/// outward, while each building keeps its size and its place beside the
/// objective it belongs to (see [`shift`]).
pub const SPREAD: f32 = 1.5;
pub const HALF_X: f32 = 177.0;
pub const HALF_Z: f32 = 102.0;
pub const CELL: f32 = 2.0;
pub const TNX: usize = 240;
pub const TNZ: usize = 168;
pub const TER_X0: f32 = -240.0;
pub const TER_Z0: f32 = -168.0;

pub const POINTS_XZ: [(f32, f32); 5] = [(-108.0, 21.0), (-54.0, -36.0), (0.0, 0.0), (54.0, 36.0), (108.0, -21.0)];
pub const BASES_XZ: [(f32, f32); 2] = [(-159.0, 0.0), (159.0, 0.0)];
/// Players spawn this far from the centre of their base, at most.
pub const BASE_SPAWN_RADIUS: f32 = 20.0;

/// How far a cluster first authored around `(x, z)` on the original
/// footprint moves when the map is spread out. Adding it to every
/// coordinate in the cluster keeps the cluster's shape.
pub const fn shift(x: f32, z: f32) -> (f32, f32) {
    (x * (SPREAD - 1.0), z * (SPREAD - 1.0))
}
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

pub static MAPS: [MapDef; 8] = [
    MapDef {
        name: "FOUNDRY VALLEY",
        blurb: "Autumn steelworks, boiler hall and a covered service culvert",
        points: POINTS_XZ,
        bases: BASES_XZ,
        ground: [[0.40, 0.47, 0.22], [0.62, 0.52, 0.25], [0.55, 0.46, 0.35]],
    },
    MapDef {
        name: "QUARRY",
        blurb: "Terraced pit, timbered mine adits and towering winding gear",
        points: [(-117.0, -30.0), (-63.0, 27.0), (0.0, 0.0), (63.0, -27.0), (117.0, 30.0)],
        bases: BASES_XZ,
        ground: [[0.53, 0.48, 0.37], [0.64, 0.57, 0.44], [0.50, 0.47, 0.44]],
    },
    MapDef {
        name: "RIDGELINE",
        blurb: "A fallen cargo giant spans the pine saddle; caves and ranger lodges on the flanks",
        points: [(-111.0, 33.0), (-51.0, -45.0), (0.0, 0.0), (51.0, 45.0), (111.0, -33.0)],
        bases: BASES_XZ,
        ground: [[0.29, 0.43, 0.22], [0.44, 0.49, 0.24], [0.48, 0.42, 0.32]],
    },
    MapDef {
        name: "DEPOT",
        blurb: "Container passages and warehouses beneath a giant yard crane",
        points: [(-111.0, -12.0), (-57.0, 15.0), (0.0, 0.0), (57.0, -15.0), (111.0, 12.0)],
        bases: BASES_XZ,
        ground: [[0.37, 0.38, 0.36], [0.46, 0.45, 0.41], [0.42, 0.41, 0.40]],
    },
    MapDef {
        name: "SPILLWAY",
        blurb: "Waterworks under a dam - a gate deck over a channel that floods",
        points: [(-114.0, -27.0), (-54.0, 30.0), (0.0, 0.0), (54.0, -30.0), (114.0, 27.0)],
        bases: BASES_XZ,
        ground: [[0.43, 0.47, 0.40], [0.57, 0.55, 0.46], [0.49, 0.50, 0.47]],
    },
    MapDef {
        name: "RELAY",
        blurb: "Highland signal station - climb the towers, hold the exchange",
        points: [(-117.0, 24.0), (-57.0, -39.0), (0.0, 0.0), (57.0, 39.0), (117.0, -24.0)],
        bases: BASES_XZ,
        ground: [[0.37, 0.43, 0.39], [0.54, 0.53, 0.47], [0.46, 0.46, 0.44]],
    },
    MapDef {
        name: "SLAG CATHEDRAL",
        blurb: "Monumental casting nave, furnace courts and steel galleries",
        points: slag_cathedral::POINTS,
        bases: BASES_XZ,
        ground: [[0.28, 0.29, 0.29], [0.42, 0.37, 0.31], [0.37, 0.36, 0.34]],
    },
    MapDef {
        name: "CANOPY",
        blurb: "Fight through a hollow giant fig - ride the seedlines, pull the shade down",
        points: canopy::POINTS,
        bases: BASES_XZ,
        ground: [[0.19, 0.39, 0.25], [0.56, 0.47, 0.31], [0.29, 0.24, 0.16]],
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
    /// Destruction group: one crate or a blast-sensitive landmark feature.
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

/// Blast-sensitive map features share the same replicated destruction bits
/// as crates. A bit may remove a whole group of structural collision blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureKind {
    BreachPanel,
    Walkway,
    Machinery,
}

/// A climbable ladder. A player facing it who stands in front of the rungs
/// and pushes forward climbs; see `sim::step` for the rest.
#[derive(Clone, Copy, Debug)]
pub struct Ladder {
    /// Middle of the rung plane at the ladder's foot.
    pub at: Vec3,
    /// Horizontal unit direction a climber faces, into the rungs.
    pub face: Vec3,
    /// Height of the deck the ladder arrives at.
    pub top: f32,
}

/// A powered sling on a fixed guide: a rider's feet follow `pts` from the
/// first dock to the last. The first and last legs are the short climbs
/// on and off at the docks; see `sim::ride`.
#[derive(Clone, Debug)]
pub struct Seedline {
    pub pts: Vec<Vec3>,
    /// Distance along the line to each point.
    pub at: Vec<f32>,
}

impl Seedline {
    pub fn new(pts: Vec<Vec3>) -> Seedline {
        let mut at = vec![0.0];
        for k in 1..pts.len() {
            at.push(at[k - 1] + pts[k].distance(pts[k - 1]));
        }
        Seedline { pts, at }
    }

    pub fn len(&self) -> f32 {
        *self.at.last().unwrap_or(&0.0)
    }

    pub fn is_empty(&self) -> bool {
        self.pts.len() < 2
    }

    /// Feet position, unit direction of travel (first to last) and leg
    /// index at distance `s`.
    pub fn sample(&self, s: f32) -> (Vec3, Vec3, usize) {
        let s = s.clamp(0.0, self.len());
        let mut k = 0;
        while k + 2 < self.pts.len() && s > self.at[k + 1] {
            k += 1;
        }
        let (a, b) = (self.pts[k], self.pts[k + 1]);
        let span = (self.at[k + 1] - self.at[k]).max(1e-4);
        let t = ((s - self.at[k]) / span).clamp(0.0, 1.0);
        (a + (b - a) * t, (b - a).normalize_or_zero(), k)
    }

    /// Whether leg `k` is a dock climb (the first or the last).
    pub fn dock_leg(&self, k: usize) -> bool {
        k == 0 || k + 2 == self.pts.len()
    }
}

#[derive(Clone, Debug)]
pub struct Feature {
    pub kind: FeatureKind,
    pub bit: u8,
    /// An exposed weak point a grenade can reach from the ground.
    pub pos: Vec3,
}

pub struct World {
    pub seed: u64,
    pub map: u8,
    pub heights: Vec<f32>,
    pub blocks: Vec<Block>,
    pub trees: Vec<Tree>,
    pub points: [Vec3; 5],
    pub bases: [Vec3; 2],
    pub features: Vec<Feature>,
    /// Destruction slots, including both crates and grouped map features.
    pub n_crates: usize,
    /// Underground routes; see `tunnels`.
    pub tunnels: Vec<tunnels::Tunnel>,
    /// Seconds on the world clock (`crate::world_clock`): when timed hazards
    /// (`crate::events`) are live. The server sets it every tick and clients
    /// every frame. Movement never reads it, so prediction is unaffected.
    pub clock: f32,
    /// Ladders a player can climb (Relay's mast).
    pub ladders: Vec<Ladder>,
    /// Seedlines a player can ride (Canopy's harvest slings).
    pub lines: Vec<Seedline>,
    /// How far Canopy's two shade banks are let down (0 up, 1 down). The
    /// server sets it every tick and clients from each snapshot.
    pub shades: [f32; 2],
}

fn hash(x: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x85EB_CA6B) ^ (z as u32).wrapping_mul(0xC2B2_AE35) ^ seed.wrapping_mul(0x27D4_EB2F);
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
        (vnoise(x / 40.0 + 100.0, z / 40.0 + 100.0, s) - 0.5) * 8.0 + (vnoise(x / 13.0 + 50.0, z / 13.0 + 50.0, s ^ 0x9E37) - 0.5) * 1.8
    };
    n(x, z) + n(-x, -z)
}

/// The lie of the land before objectives are levelled. Each map's shape is
/// built from terms that are unchanged by (x, z) -> (-x, -z).
fn shape(x: f32, z: f32, s: u32, map: u8) -> f32 {
    // Canopy is authored in final metres.
    if map % MAPS.len() as u8 == canopy::MAP_ID {
        return canopy::shape(x, z, raw_height(x, z, s) * 0.03);
    }
    // Every feature was designed on the original footprint; sample it there.
    let (x, z) = (x / SPREAD, z / SPREAD);
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
        4 => {
            // A drained east-west channel with broad, walkable banks. The
            // banks are terrain ramps rather than collision-box stairways.
            let channel = 1.0 - smooth(8.0, 25.0, z.abs());
            raw_height(x, z, s) * 0.17 + 1.6 - 4.0 * channel
        }
        5 => {
            // The relay stands on a low central plateau, with a shallow
            // outer route on both sides and wind-worn hills beyond it.
            let plateau = 1.0 - smooth(16.0, 48.0, (x * x * 0.3 + z * z).sqrt());
            raw_height(x, z, s) * 0.42 + 3.4 * plateau
        }
        6 => {
            // Level industrial apron; only the outer flanks regain terrain.
            let edge = smooth(48.0, 64.0, z.abs()).max(smooth(112.0, 145.0, x.abs()));
            raw_height(x, z, s) * 0.2 * edge
        }
        _ => raw_height(x, z, s),
    }
}

/// Distance along the Ridgeline spine of its two outer passes.
const RIDGE_PASS: f32 = 44.0;

fn full_height(x: f32, z: f32, s: u32, map: u8, tunnels: &[tunnels::Tunnel]) -> f32 {
    let def = map_def(map);
    let mut h = shape(x, z, s, map);
    // The original valley settles objectives halfway to sea level; the other
    // maps level them where the ground already is (a pit floor stays a floor).
    let settle = if map.is_multiple_of(MAPS.len() as u8) { 0.5 } else { 1.0 };
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
        flat(cx, cz, BASE_SPAWN_RADIUS + 8.0);
    }
    // Relay's towers stand on levelled sites so their stairs meet the ground.
    if map % MAPS.len() as u8 == relay::MAP_ID {
        for (tx, tz, _, _) in relay::TOWERS {
            flat(tx, tz, relay::SITE_RADIUS);
            flat(-tx, -tz, relay::SITE_RADIUS);
        }
        let (dx, dz) = relay::DISH;
        flat(dx, dz, 6.0);
        flat(-dx, -dz, 6.0);
        h = relay::settle(x, z, h, |cx, cz| shape(cx, cz, s, map));
    }
    h = landmarks::settle(x, z, map % MAPS.len() as u8, h, |cx, cz| shape(cx, cz, s, map));
    if map % MAPS.len() as u8 == fallen_giant::MAP_ID {
        h = fallen_giant::settle(x, z, h, |cx, cz| shape(cx, cz, s, map));
    }
    h = tunnels::sculpt(x, z, h, tunnels);
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
        // Tunnel floors follow the ground as it would be without them.
        let tunnels = tunnels::layout(map, |x, z| full_height(x, z, s, map, &[]));
        let mut heights = Vec::with_capacity((TNX + 1) * (TNZ + 1));
        for iz in 0..=TNZ {
            for ix in 0..=TNX {
                let x = TER_X0 + ix as f32 * CELL;
                let z = TER_Z0 + iz as f32 * CELL;
                heights.push(full_height(x, z, s, map, &tunnels));
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
            features: Vec::new(),
            n_crates: 0,
            tunnels,
            clock: 0.0,
            ladders: Vec::new(),
            lines: Vec::new(),
            shades: [0.0; 2],
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
            4 => w.build_spillway(&mut rng),
            5 => w.build_relay(&mut rng),
            slag_cathedral::MAP_ID => slag_cathedral::build(&mut w, &mut rng),
            canopy::MAP_ID => canopy::build(&mut w, &mut rng),
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

    /// Whether (x, z) is over lava that is always molten (Slag Cathedral's
    /// casting channels), grown by `margin`. Navigation and map building use
    /// this; the runner's lava comes and goes.
    pub fn lava_fixed(&self, x: f32, z: f32, margin: f32) -> bool {
        self.map == slag_cathedral::MAP_ID && slag_cathedral::in_lava(x, z, margin)
    }

    /// Whether (x, z) is over molten lava now (see `clock`), grown by `margin`.
    pub fn lava(&self, x: f32, z: f32, margin: f32) -> bool {
        self.lava_fixed(x, z, margin)
            || (self.map == slag_cathedral::MAP_ID
                && slag_cathedral::in_runner(x, z, margin, crate::events::lava_flow(self.seed, self.clock).front))
    }

    /// Whether someone whose feet are at `pos` is standing in lava.
    pub fn burning(&self, pos: Vec3) -> bool {
        self.lava(pos.x, pos.z, 0.0) && pos.y - self.height(pos.x, pos.z) < slag_cathedral::LAVA_REACH
    }

    /// Height of flood water over (x, z) now, if there is any there.
    pub fn water(&self, x: f32, z: f32) -> Option<f32> {
        if self.map != spillway::MAP_ID {
            return None;
        }
        let y = spillway::water_surface(self, x, z, crate::events::flood(self.seed, self.clock))?;
        (self.height(x, z) < y).then_some(y)
    }

    /// Whether someone whose feet are at `pos` is in flood water.
    pub fn drowning(&self, pos: Vec3) -> bool {
        self.water(pos.x, pos.z).is_some_and(|y| pos.y < y - spillway::WADE)
    }

    /// What the ground at `pos` is doing to whoever stands there: damage per
    /// second and the kill-feed id it is reported under.
    pub fn hazard(&self, pos: Vec3) -> Option<(f32, u8)> {
        if self.burning(pos) {
            Some((slag_cathedral::LAVA_DPS, crate::sim::LAVA_WEAPON))
        } else if self.drowning(pos) {
            Some((spillway::WATER_DPS, crate::sim::WATER_WEAPON))
        } else {
            None
        }
    }

    /// Whether feet at `p`, grown by `margin` across the ground, would be
    /// somewhere a bot should not walk or an item should not lie now: in
    /// molten lava, or below flood water deeper than a wade (a bridge over
    /// the flood is fine).
    pub fn danger(&self, p: Vec3, margin: f32) -> bool {
        if self.lava(p.x, p.z, margin) {
            return true;
        }
        if self.map != spillway::MAP_ID {
            return false;
        }
        [(0.0, 0.0), (margin, 0.0), (-margin, 0.0), (0.0, margin), (0.0, -margin)]
            .iter()
            .any(|&(dx, dz)| self.water(p.x + dx, p.z + dz).is_some_and(|y| p.y < y - spillway::WADE))
    }

    /// Whether Spillway's channel is flooding or about to (from the siren
    /// until it has drained): while it is, bots route round the water.
    pub fn flooding(&self) -> bool {
        if self.map != spillway::MAP_ID {
            return false;
        }
        let f = crate::events::flood(self.seed, self.clock);
        crate::events::siren(self.seed, self.clock).is_some() || f.gates > 0.0 || f.level > 0.0
    }

    /// The surface someone walking at (x, z) stands on: the ground, or a
    /// walkway over it (Spillway's gate deck, its bridges and stairs).
    pub fn walk_height(&self, x: f32, z: f32) -> f32 {
        if self.map == spillway::MAP_ID {
            if let Some(y) = spillway::walkway(self, x, z) {
                return y;
            }
        }
        self.height(x, z)
    }

    /// Whether someone whose feet are at `pos` counts toward holding
    /// objective `k`. Spillway's C counts the gate deck, not the channel
    /// floor under it.
    pub fn on_point(&self, k: usize, pos: Vec3) -> bool {
        let d = pos - self.points[k];
        if d.x * d.x + d.z * d.z >= crate::POINT_RADIUS * crate::POINT_RADIUS {
            return false;
        }
        if self.map == spillway::MAP_ID && k == crate::NUM_POINTS / 2 {
            spillway::on_deck(self, pos)
        } else if self.map == canopy::MAP_ID {
            canopy::on_pad(self, k, pos)
        } else {
            d.y.abs() < 7.0
        }
    }

    /// The direction a walker at `pos` can go when it wants `want`: out from
    /// under Spillway's deck (the bots' flow field describes the deck), and
    /// along a tunnel underground. Unchanged anywhere else.
    pub fn walk_steer(&self, pos: Vec3, want: Vec2) -> Vec2 {
        if self.map == spillway::MAP_ID {
            return spillway::steer(self, pos, want);
        }
        self.tunnel_steer(pos, want)
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
        if self.underground(o) {
            return best;
        }
        while t0 < best_t {
            let t1 = (t0 + step).min(best_t);
            let p = o + d * t1;
            if p.y > 60.0 && d.y >= 0.0 {
                break;
            }
            if self.underground(p) {
                let (mut lo, mut hi) = (t0, t1);
                for _ in 0..7 {
                    let mid = (lo + hi) * 0.5;
                    let q = o + d * mid;
                    if self.underground(q) {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                let q = o + d * lo;
                // Under the surface the hit is a tunnel's wall or roof.
                let normal = if q.y < self.height(q.x, q.z) - 0.3 { -d } else { self.terrain_normal(q.x, q.z) };
                return Some(RayHit { t: lo, normal, block: None });
            }
            if t1 >= best_t {
                break;
            }
            t0 = t1;
        }
        best
    }

    /// Whether shade cloth hangs across the sight line a-b. Like leaves it
    /// stops sight and not bullets.
    pub fn screened(&self, a: Vec3, b: Vec3) -> bool {
        self.map == canopy::MAP_ID && canopy::screened(self.shades, a, b)
    }

    pub fn los(&self, a: Vec3, b: Vec3, dmask: u64) -> bool {
        let d = b - a;
        let len = d.length();
        if len < 1e-4 {
            return true;
        }
        self.raycast(a, d / len, len, dmask).is_none()
    }

    /// Whether a sight line from `a` to `b` passes through the leaves of a
    /// tree. Canopies stop sight, not bullets, so this is for whoever has
    /// to see a target first (bots) and not for hit tests. The volumes are
    /// the solid core of what the client draws, and the first and last
    /// 1.2 m of the line are not counted, so someone standing under a pine
    /// is not hidden from a soldier in the open.
    pub fn foliage(&self, a: Vec3, b: Vec3) -> bool {
        const STEP: f32 = 0.35;
        const SKIP: f32 = 1.2;
        let d = b - a;
        let len = d.length();
        if len < SKIP * 2.0 + STEP {
            return false;
        }
        let dir = d / len;
        let (lo, hi) = (a.min(b), a.max(b));
        for t in &self.trees {
            let r = t.radius;
            if t.base.x + r < lo.x || t.base.x - r > hi.x || t.base.z + r < lo.z || t.base.z - r > hi.z {
                continue;
            }
            if t.base.y + t.height < lo.y || t.base.y + t.height * 0.25 > hi.y {
                continue;
            }
            // The stretch of the line within the canopy's footprint.
            let (ox, oz) = (a.x - t.base.x, a.z - t.base.z);
            let qa = dir.x * dir.x + dir.z * dir.z;
            let (s0, s1) = if qa < 1e-6 {
                if ox * ox + oz * oz > r * r {
                    continue;
                }
                (0.0, len)
            } else {
                let qb = ox * dir.x + oz * dir.z;
                let disc = qb * qb - qa * (ox * ox + oz * oz - r * r);
                if disc <= 0.0 {
                    continue;
                }
                let root = disc.sqrt();
                ((-qb - root) / qa, (-qb + root) / qa)
            };
            let (s0, s1) = (s0.max(SKIP), s1.min(len - SKIP));
            let mut inside = 0;
            let mut s = s0;
            while s <= s1 {
                let p = a + dir * s;
                let y = (p.y - t.base.y) / t.height;
                let rr = vec2(p.x - t.base.x, p.z - t.base.z).length() / r;
                let leaves = if t.pine {
                    // Four stacked tiers read as one cone from 0.28 to 0.78.
                    (0.28..0.78).contains(&y) && rr < 0.85 * (0.8 - y) / 0.52
                } else {
                    let e = (y - 0.72) / (0.34 * 0.8);
                    let h = rr / 0.85;
                    h * h + e * e < 1.0
                };
                if leaves {
                    inside += 1;
                    // About 0.7 m of leaves hides a soldier.
                    if inside >= 2 {
                        return true;
                    }
                }
                s += STEP;
            }
        }
        false
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
        self.blocks.push(Block { min: vec3(cx - hx, min_y, cz - hz), max: vec3(cx + hx, g + y0 + h, cz + hz), kind, tint, dest });
    }

    /// Place a block and its 180-degree mirror.
    fn sym(&mut self, cx: f32, cz: f32, hx: f32, hz: f32, y0: f32, h: f32, kind: BlockKind, tint: u8) {
        self.add(cx, cz, hx, hz, y0, h, kind, tint);
        self.add(-cx, -cz, hx, hz, y0, h, kind, tint);
    }

    fn footprint_free(&self, cx: f32, cz: f32, r: f32) -> bool {
        !landmarks::reserved(self.map, cx, cz, r, r)
            && !fallen_giant::reserved(self.map, cx, cz, r, r)
            && !relay::reserved(self.map, cx, cz, r, r)
            && !canopy::reserved(self.map, cx, cz, r, r)
            && !self.tunnel_reserved(cx, cz, r)
            && !self.blocks.iter().any(|b| cx + r > b.min.x && cx - r < b.max.x && cz + r > b.min.z && cz - r < b.max.z)
    }

    fn near_objective(&self, cx: f32, cz: f32, r_point: f32, r_base: f32) -> bool {
        let near = |p: Vec3, r: f32| (cx - p.x).powi(2) + (cz - p.z).powi(2) < r * r;
        // Bases are measured from the edge of the spawn ground.
        let r_base = r_base + BASE_SPAWN_RADIUS - 8.0;
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
    /// east one mirrored). Everything stands outside the spawn ground except
    /// the command post at its back.
    fn build_bases(&mut self) {
        use BlockKind::*;
        let bx = BASES_XZ[0].0;
        // Command post and its mast.
        self.sym(bx - 12.0, 0.0, 1.5, 4.0, 0.0, 4.2, Metal, 0);
        self.sym(bx - 12.0, 0.0, 0.25, 0.25, 4.2, 5.0, Metal, 1);
        // Blast walls either side of the way out.
        self.sym(bx + 23.0, -13.0, 0.3, 5.0, 0.0, 3.2, Concrete, 0);
        self.sym(bx + 23.0, 13.0, 0.3, 5.0, 0.0, 3.2, Concrete, 1);
        // Supply containers on the flanks of the spawn ground.
        self.sym(bx + 8.0, -24.0, 3.0, 1.25, 0.0, 2.6, Container, 0);
        self.sym(bx + 8.0, 24.0, 3.0, 1.25, 0.0, 2.6, Container, 2);
        self.sym(bx - 5.0, -23.5, 1.25, 3.0, 0.0, 2.6, Container, 3);
        self.sym(bx - 5.0, 23.5, 1.25, 3.0, 0.0, 2.6, Container, 1);
        // Sandbagged firing steps covering the exits.
        self.sym(bx + 25.0, -27.0, 2.4, 0.45, 0.0, 1.1, Sandbag, 0);
        self.sym(bx + 25.0, 27.0, 2.4, 0.45, 0.0, 1.1, Sandbag, 1);
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

    // ------------------------------------------------------ structure kit

    /// Lowest ground under a footprint.
    fn ground_under(&self, cx: f32, cz: f32, hx: f32, hz: f32) -> f32 {
        let mut low = self.height(cx, cz);
        for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0), (0.0, -1.0), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)] {
            low = low.min(self.height(cx + sx * hx, cz + sz * hz));
        }
        low
    }

    /// Rise of the ground across a footprint.
    fn slope_under(&self, cx: f32, cz: f32, hx: f32, hz: f32) -> f32 {
        let mut high = self.height(cx, cz);
        for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            high = high.max(self.height(cx + sx * hx, cz + sz * hz));
        }
        high - self.ground_under(cx, cz, hx, hz)
    }

    /// Nothing solid within the rectangle, here or at the mirrored spot.
    fn rect_free(&self, cx: f32, cz: f32, hx: f32, hz: f32) -> bool {
        if landmarks::reserved(self.map, cx, cz, hx, hz) || self.tunnel_reserved(cx, cz, hx.max(hz)) {
            return false;
        }
        if fallen_giant::reserved(self.map, cx, cz, hx, hz) || relay::reserved(self.map, cx, cz, hx, hz) {
            return false;
        }
        let free =
            |x: f32, z: f32| !self.blocks.iter().any(|b| x + hx > b.min.x && x - hx < b.max.x && z + hz > b.min.z && z - hz < b.max.z);
        free(cx, cz) && free(-cx, -cz)
    }

    /// Distance from (x, z) to the walking line base -> A -> B -> C.
    fn lane_distance(&self, x: f32, z: f32) -> f32 {
        let chain = [self.bases[0], self.points[0], self.points[1], self.points[2]];
        let p = vec2(x, z);
        chain
            .windows(2)
            .map(|s| {
                let (a, b) = (vec2(s[0].x, s[0].z), vec2(s[1].x, s[1].z));
                let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
                p.distance(a + (b - a) * t)
            })
            .fold(f32::MAX, f32::min)
    }

    /// Lines whose dressing scattered structures must not stand in, as
    /// west-authored segments: Relay's power and signal cables and the
    /// exchange flag line, and Slag Cathedral's ladle railway.
    fn corridors(&self) -> Vec<(Vec2, Vec2)> {
        let v = |(x, z): (f32, f32)| vec2(x, z);
        match self.map {
            relay::MAP_ID => {
                let shed = (-82.5 + shift(-78.0, 16.0).0, 18.5 + shift(-78.0, 16.0).1);
                let switchgear = (-8.5, -7.0);
                let [inner, outer] = relay::GANTRIES;
                let [array, exchange] = relay::TOWERS.map(|(x, z, _, _)| (x, z));
                vec![
                    (v(shed), v(outer)),
                    (v(outer), v(inner)),
                    (v(inner), v(switchgear)),
                    (v(exchange), v(inner)),
                    (v(exchange), v(array)),
                    (v(exchange), v(relay::DISH)),
                    (v(array), v(relay::DISH)),
                ]
            }
            slag_cathedral::MAP_ID => {
                let end = BASES_XZ[0].0 + BASE_SPAWN_RADIUS;
                let mut lines = vec![(vec2(end, 0.0), vec2(-slag_cathedral::NAVE_HALF_X, 0.0))];
                // Keep the lava runner clear (it is mirrored onto the north
                // side too, which only leaves that strip open as well).
                lines.extend(slag_cathedral::RUNNER.windows(2).map(|w| (v(w[0]), v(w[1]))));
                lines
            }
            _ => Vec::new(),
        }
    }

    /// Whether (x, z) or its mirror lies within `d` of a corridor.
    fn in_corridor(&self, x: f32, z: f32, d: f32) -> bool {
        self.corridors().iter().any(|&(a, b)| {
            [vec2(x, z), vec2(-x, -z)].iter().any(|&p| {
                let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
                p.distance(a + (b - a) * t) < d
            })
        })
    }

    /// Whether a structure of half size (hx, hz) can stand at (x, z) in the
    /// west half: inside the battlefield, on reasonably level ground, off the
    /// main walking line, clear of objectives, spawns and other structures.
    fn site_ok(&self, x: f32, z: f32, hx: f32, hz: f32, pad: f32) -> bool {
        let r = hx.max(hz);
        x + hx < -3.0
            && x - hx > -HALF_X + 6.0
            && (z.abs() + hz) < HALF_Z - 6.0
            && !self.near_objective(x, z, 16.0 + r, 12.0 + r)
            && self.lane_distance(x, z) > 6.0 + r
            && self.slope_under(x, z, hx, hz) < 1.3
            && !self.lava_fixed(x, z, r + 2.0)
            && !self.in_corridor(x, z, r + 3.0)
            && self.rect_free(x, z, hx + pad, hz + pad)
    }

    /// One wall with openings, along x (`along_x`) or z, at `fixed`, from
    /// `a` to `b`. Doors run from the floor; windows have a sill.
    #[allow(clippy::too_many_arguments)]
    fn wall_with_openings(&mut self, along_x: bool, fixed: f32, a: f32, b: f32, low: f32, g: f32, top: f32, door: bool, tint: u8) {
        use BlockKind::*;
        let t = 0.22;
        let len = b - a;
        // (start, end, sill, head); a door's sill is below the floor.
        let mut holes: Vec<(f32, f32, f32, f32)> = Vec::new();
        if door {
            let c = (a + b) * 0.5;
            holes.push((c - 1.2, c + 1.2, low, g + 2.45));
        }
        if len > 7.5 {
            for c in [a + len * 0.2, b - len * 0.2] {
                holes.push((c - 0.65, c + 0.65, g + 1.05, g + 2.1));
            }
        }
        holes.sort_by(|p, q| p.0.total_cmp(&q.0));
        let piece = |me: &mut World, s: f32, e: f32, y0: f32, y1: f32| {
            if e - s < 0.05 || y1 - y0 < 0.05 {
                return;
            }
            let c = (s + e) * 0.5;
            if along_x {
                me.sym_abs(c, fixed, (e - s) * 0.5, t, y0, y1, Concrete, tint);
            } else {
                me.sym_abs(fixed, c, t, (e - s) * 0.5, y0, y1, Concrete, tint);
            }
        };
        let mut at = a;
        for (s, e, sill, head) in holes {
            piece(self, at, s, low, top);
            piece(self, s, e, low, sill);
            piece(self, s, e, head, top);
            at = e;
        }
        piece(self, at, b, low, top);
    }

    /// A flat-roofed building with a doorway in each side named in `doors`
    /// (bit0 west, bit1 east, bit2 north (-z), bit3 south (+z)) and windows
    /// in its longer walls. Placed with its mirror.
    #[allow(clippy::too_many_arguments)]
    fn building(&mut self, cx: f32, cz: f32, hx: f32, hz: f32, h: f32, doors: u8, tint: u8, roof_kind: BlockKind) {
        let g = self.height(cx, cz);
        let low = self.ground_under(cx, cz, hx, hz) - 0.6;
        let top = g + h;
        let t = 0.22;
        self.wall_with_openings(true, cz - hz + t, cx - hx, cx + hx, low, g, top, doors & 4 != 0, tint);
        self.wall_with_openings(true, cz + hz - t, cx - hx, cx + hx, low, g, top, doors & 8 != 0, tint);
        self.wall_with_openings(false, cx - hx + t, cz - hz + 2.0 * t, cz + hz - 2.0 * t, low, g, top, doors & 1 != 0, tint);
        self.wall_with_openings(false, cx + hx - t, cz - hz + 2.0 * t, cz + hz - 2.0 * t, low, g, top, doors & 2 != 0, tint);
        self.sym_abs(cx, cz, hx + 0.25, hz + 0.25, top, top + 0.3, roof_kind, (tint + 1) % 4);
    }

    /// An open-sided shed: a roof on posts. Shelter from above, not cover.
    fn shed(&mut self, cx: f32, cz: f32, hx: f32, hz: f32, h: f32, tint: u8) {
        use BlockKind::*;
        let g = self.height(cx, cz);
        let top = g + h;
        let nx = (hx / 3.5).ceil().max(1.0) as i32;
        for i in 0..=nx {
            let x = cx - hx + 2.0 * hx * i as f32 / nx as f32;
            for z in [cz - hz, cz + hz] {
                self.sym_abs(x, z, 0.18, 0.18, self.height(x, z) - 0.6, top, Metal, 0);
            }
        }
        self.sym_abs(cx, cz, hx + 0.4, hz + 0.6, top, top + 0.28, Metal, tint);
    }

    /// A lookout: a slab `h` metres up, a stair down one side (`dir` is the
    /// x direction the stair leaves in) and waist-high plates on the far edges.
    fn lookout(&mut self, cx: f32, cz: f32, h: f32, dir: f32, tint: u8) {
        use BlockKind::*;
        let half = 2.2;
        let g = self.height(cx, cz);
        let low = self.ground_under(cx, cz, half + 4.0, half) - 0.6;
        let top = g + h;
        self.sym_abs(cx, cz, half, half, low, top, Concrete, tint);
        let rise = 0.3;
        let n = (h / rise).ceil() as usize;
        let tread = 0.55;
        for i in 0..n {
            // The first step stands farthest out.
            let x = cx + dir * (half + tread * (n - i) as f32 - tread * 0.5);
            let y = (top - rise * (n - 1 - i) as f32).min(top);
            self.sym_abs(x, cz, tread * 0.5, 1.0, low, y, Step, (i % 2) as u8);
        }
        // Plates on the three sides away from the stair.
        let pt = 0.14;
        self.sym_abs(cx - dir * (half - pt), cz, pt, half, top, top + 0.95, Metal, 1);
        self.sym_abs(cx, cz - half + pt, half, pt, top, top + 0.95, Metal, 1);
        self.sym_abs(cx, cz + half - pt, half, pt, top, top + 0.95, Metal, 1);
    }

    /// A round tank, approximated by two crossed slabs and a core.
    fn tank(&mut self, cx: f32, cz: f32, r: f32, h: f32, tint: u8) {
        use BlockKind::*;
        self.sym(cx, cz, r, r * 0.42, 0.0, h, Metal, tint);
        self.sym(cx, cz, r * 0.42, r, 0.0, h, Metal, tint);
        self.sym(cx, cz, r * 0.72, r * 0.72, 0.0, h, Metal, tint);
        self.sym(cx, cz, r * 0.3, r * 0.3, h, 0.6, Metal, (tint + 1) % 4);
    }

    /// Up to `n` structures dropped where `site_ok` allows, each picked by
    /// `make` from a size returned by `size`.
    /// `turn` lets the footprint be turned a quarter.
    fn scatter(
        &mut self,
        rng: &mut Rng,
        n: usize,
        turn: bool,
        size: impl Fn(&mut Rng) -> (f32, f32),
        mut make: impl FnMut(&mut World, &mut Rng, f32, f32, f32, f32),
    ) {
        for _ in 0..n {
            for _ in 0..60 {
                let (mut hx, mut hz) = size(rng);
                if turn && rng.chance(0.5) {
                    std::mem::swap(&mut hx, &mut hz);
                }
                let x = rng.range(-HALF_X + 8.0, -6.0);
                let z = rng.range(-HALF_Z + 8.0, HALF_Z - 8.0);
                if self.site_ok(x, z, hx, hz, 4.5) {
                    make(self, rng, x, z, hx, hz);
                    break;
                }
            }
        }
    }

    /// Scattered roofed buildings, each with at least two doors.
    fn scatter_buildings(&mut self, rng: &mut Rng, n: usize, sizes: &[(f32, f32, f32)], roof: BlockKind) {
        let pick = sizes.to_vec();
        let heights = std::cell::Cell::new(3.4f32);
        self.scatter(
            rng,
            n,
            true,
            |rng| {
                let (hx, hz, h) = pick[rng.below(pick.len() as u32) as usize];
                heights.set(h);
                (hx, hz)
            },
            |me, rng, x, z, hx, hz| {
                let mut doors = rng.below(16) as u8;
                while doors.count_ones() < 2 {
                    doors |= 1 << rng.below(4);
                }
                let tint = rng.below(4) as u8;
                me.building(x, z, hx, hz, heights.get(), doors, tint, roof);
            },
        );
    }

    fn scatter_lookouts(&mut self, rng: &mut Rng, n: usize, h: f32) {
        // The footprint covers the slab and its stair; the stair runs along x.
        let run = 0.55 * (h / 0.3).ceil();
        self.scatter(
            rng,
            n,
            false,
            |_| (2.2 + run * 0.5 + 0.3, 2.2),
            |me, rng, x, z, _, _| {
                let dir = if rng.chance(0.5) { 1.0 } else { -1.0 };
                me.lookout(x - dir * run * 0.5, z, h, dir, rng.below(3) as u8);
            },
        );
    }

    fn scatter_sheds(&mut self, rng: &mut Rng, n: usize) {
        self.scatter(
            rng,
            n,
            true,
            |rng| (rng.range(4.5, 7.5), rng.range(2.8, 3.8)),
            |me, rng, x, z, hx, hz| {
                let h = rng.range(3.6, 4.6);
                me.shed(x, z, hx, hz, h, rng.below(3) as u8);
                me.crates(rng, x, z, 2);
            },
        );
    }

    fn scatter_tanks(&mut self, rng: &mut Rng, n: usize) {
        self.scatter(
            rng,
            n,
            true,
            |rng| {
                let r = rng.range(2.0, 3.4);
                (r, r)
            },
            |me, rng, x, z, r, _| {
                let h = rng.range(3.5, 6.5);
                me.tank(x, z, r, h, rng.below(3) as u8);
            },
        );
    }

    /// Roofless walled compounds: broken walls round an empty yard.
    fn scatter_ruins(&mut self, rng: &mut Rng, n: usize) {
        self.scatter(
            rng,
            n,
            true,
            |rng| (rng.range(5.0, 8.0), rng.range(4.0, 6.5)),
            |me, rng, x, z, hx, hz| {
                let h = rng.range(2.4, 3.4);
                me.ruin(rng, x, z, hx * 2.0, hz * 2.0, h, (-HALF_X, -1.0), 0b1111);
            },
        );
    }

    // ------------------------------------------------------------ Quarry

    fn build_quarry(&mut self, rng: &mut Rng) {
        use BlockKind::*;
        self.build_bases();
        landmarks::build(self);

        // --- A / E: site office, a walled yard up on the rim ---
        let (ox, oz) = shift(-78.0, -20.0);
        self.ruin(rng, -78.0 + ox, -20.0 + oz, 12.0, 9.0, 3.2, (-300.0, 300.0), 0b1111);
        self.crates(rng, -78.0 + ox, -20.0 + oz, 4);
        self.sym(-69.5 + ox, -24.0 + oz, 0.45, 2.6, 0.0, 1.1, Sandbag, 0);
        self.sym(-74.0 + ox, -12.5 + oz, 2.6, 0.45, 0.0, 1.1, Sandbag, 1);

        // --- B / D: loading terrace with spoil containers and a conveyor ---
        let (ox, oz) = shift(-42.0, 18.0);
        self.sym(-47.0 + ox, 23.0 + oz, 3.0, 1.25, 0.0, 2.6, Container, 2);
        self.sym(-36.5 + ox, 25.5 + oz, 1.25, 3.0, 0.0, 2.6, Container, 0);
        self.sym(-47.5 + ox, 12.0 + oz, 1.25, 3.0, 0.0, 2.6, Container, 3);
        self.sym(-36.0 + ox, 11.5 + oz, 2.4, 0.45, 0.0, 1.1, Sandbag, 0);
        self.crates(rng, -42.0 + ox, 18.0 + oz, 5);
        let belt = self.height(-50.0 + ox, 32.0 + oz) + 5.5;
        for k in 0..4 {
            let x = -50.0 + ox + 6.0 * k as f32;
            self.sym_abs(x, 32.0 + oz, 0.3, 0.3, self.height(x, 32.0 + oz) - 0.6, belt, Metal, 0);
        }
        self.sym_abs(-41.0 + ox, 32.0 + oz, 9.8, 0.55, belt, belt + 0.5, Metal, 1);

        // --- C: the crusher, down on the pit floor ---
        self.centre_platform();
        self.sym(-8.5, 6.5, 1.4, 1.4, 0.0, 7.5, Metal, 0);
        self.sym(-8.5, 6.5, 1.7, 1.7, 7.5, 0.5, Metal, 1);
        self.sym(-11.5, -4.5, 0.45, 2.4, 0.0, 1.1, Sandbag, 1);
        self.sym(-3.5, -10.0, 2.4, 0.45, 0.0, 1.1, Sandbag, 0);
        self.crates(rng, -6.0, -4.5, 4);

        // --- The works: site offices and stores, machine sheds, lookouts
        // over the pit and fuel tanks, between the terraces ---
        self.scatter_buildings(rng, 6, &[(4.0, 3.0, 3.4), (5.5, 3.5, 3.6), (3.2, 2.6, 3.2)], Metal);
        self.scatter_sheds(rng, 4);
        self.scatter_lookouts(rng, 3, 3.6);
        self.scatter_tanks(rng, 3);

        // --- Cut stone and spoil on the terraces ---
        for _ in 0..76 {
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
        self.lane_cover(rng, 6, 14.0);
        self.boulders(rng, 110, 0.8, 1.8);
        self.forest(rng, 45, 210, 0.7);
    }

    // --------------------------------------------------------- Ridgeline

    fn build_ridgeline(&mut self, rng: &mut Rng) {
        use BlockKind::*;
        self.build_bases();
        landmarks::build(self);

        // --- A / E: walled outpost at the forest edge ---
        let (ox, oz) = shift(-74.0, 22.0);
        self.ruin(rng, -74.0 + ox, 22.0 + oz, 13.0, 10.0, 3.4, (-300.0, 300.0), 0b1111);
        self.crates(rng, -74.0 + ox, 22.0 + oz, 4);
        self.sym(-64.5 + ox, 18.0 + oz, 0.45, 2.6, 0.0, 1.1, Sandbag, 0);
        self.sym(-70.0 + ox, 30.5 + oz, 2.6, 0.45, 0.0, 1.1, Sandbag, 1);

        // --- B / D: logging camp below the outer pass ---
        let (ox, oz) = shift(-34.0, -30.0);
        self.ruin(rng, -38.0 + ox, -34.0 + oz, 9.0, 7.0, 2.8, (-300.0, 300.0), 0b1101);
        self.sym(-27.5 + ox, -26.0 + oz, 3.0, 1.25, 0.0, 2.6, Container, 1);
        self.sym(-40.0 + ox, -23.0 + oz, 2.4, 0.45, 0.0, 1.1, Sandbag, 0);
        self.sym(-26.5 + ox, -35.0 + oz, 0.45, 2.4, 0.0, 1.1, Sandbag, 1);
        self.crates(rng, -32.5 + ox, -28.0 + oz, 5);

        // --- C: the Fallen Giant, a cargo aircraft across the saddle ---
        fallen_giant::build(self);

        // --- The crest: a wall of rock everywhere but the passes ---
        let (ux, uz) = (0.35 / 1.0595, 1.0 / 1.0595);
        let pass = RIDGE_PASS * SPREAD;
        let mut along = 15.0 * SPREAD;
        while along < 76.0 * SPREAD {
            let (x, z) = (ux * along + rng.range(-1.2, 1.2), uz * along + rng.range(-1.0, 1.0));
            let size = rng.range(1.6, 2.4);
            if (along - pass).abs() > 8.5 * SPREAD
                && z.abs() < HALF_Z - 3.0
                && !self.near_objective(x, z, 13.0, 16.0)
                && self.clear_both(x, z, 3.6)
            {
                self.rock(rng, x, z, size);
            }
            along += 4.2;
        }

        // --- Forest cabins, hunters' lookouts and old walled pens ---
        self.scatter_buildings(rng, 7, &[(3.2, 2.6, 3.2), (4.0, 3.0, 3.4)], Concrete);
        self.scatter_lookouts(rng, 4, 4.2);
        self.scatter_ruins(rng, 3);

        self.lane_cover(rng, 7, 15.0);
        self.boulders(rng, 60, 0.7, 1.5);
        self.forest(rng, 450, 260, 0.75);
    }

    // ------------------------------------------------------------- Depot

    fn build_depot(&mut self, rng: &mut Rng) {
        use BlockKind::*;
        self.build_bases();
        landmarks::build(self);

        // --- A / E: warehouse shell ---
        let (ox, oz) = shift(-74.0, -8.0);
        self.ruin(rng, -74.0 + ox, -8.0 + oz, 16.0, 12.0, 4.0, (-300.0, 300.0), 0b1111);
        self.crates(rng, -74.0 + ox, -8.0 + oz, 5);
        self.sym(-63.0 + ox, -3.0 + oz, 0.45, 2.6, 0.0, 1.1, Sandbag, 0);

        // --- B / D: loading dock, a raised slab with steps at one end ---
        let (ox, oz) = shift(-38.0, 10.0);
        self.sym(-38.0 + ox, 16.5 + oz, 5.0, 2.0, 0.0, 1.1, Concrete, 1);
        for i in 0..3 {
            let x = -43.3 + ox - 0.6 * (2 - i) as f32;
            self.sym(x, 16.5 + oz, 0.3, 1.2, 0.0, 0.3 * (i + 1) as f32, Step, (i % 2) as u8);
        }
        self.sym(-33.0 + ox, 3.5 + oz, 2.4, 0.45, 0.0, 1.1, Sandbag, 0);
        self.sym(-45.5 + ox, 6.5 + oz, 0.45, 2.4, 0.0, 1.1, Sandbag, 1);
        self.crates(rng, -38.0 + ox, 9.5 + oz, 5);

        // --- C: container cover below the yard-wide gantry ---
        self.sym(-4.2, 2.4, 3.0, 1.25, 0.0, 2.6, Container, 1);
        self.sym(-11.5, 0.0, 0.45, 2.4, 0.0, 1.1, Sandbag, 1);
        self.crates(rng, -5.5, -4.5, 4);

        // --- Warehouses and yard offices, before the containers fill in ---
        self.scatter_buildings(rng, 4, &[(9.0, 5.5, 5.2), (7.5, 5.0, 4.8)], Metal);
        self.scatter_buildings(rng, 4, &[(4.0, 3.0, 3.4), (3.4, 2.8, 3.2)], Concrete);
        self.scatter_lookouts(rng, 2, 3.6);

        // --- Container rows: loose grid with gaps, some stacked two high ---
        let mut gx = -HALF_X + 22.0;
        while gx < -6.0 {
            let mut gz = -HALF_Z + 10.0;
            while gz < HALF_Z - 9.0 {
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
        for _ in 0..90 {
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
        self.crates(rng, -84.0, 6.0, 3);
        self.crates(rng, -30.0, -9.0, 3);
        self.boulders(rng, 12, 0.7, 1.2);
        self.forest(rng, 20, 210, 0.3);
    }

    // ---------------------------------------------------------- Spillway

    fn build_spillway(&mut self, rng: &mut Rng) {
        use BlockKind::*;
        self.build_bases();

        // A / E: the settling yard. Open-ended concrete cells break up the
        // first approach without turning the objective into a sealed room.
        let (ox, oz) = shift(-76.0, -18.0);
        self.sym(-82.5 + ox, -18.0 + oz, 0.45, 5.0, 0.0, 2.8, Concrete, 1);
        self.sym(-76.0 + ox, -23.0 + oz, 6.9, 0.4, 0.0, 1.15, Concrete, 2);
        self.sym(-70.0 + ox, -19.8 + oz, 0.4, 3.2, 0.0, 1.15, Concrete, 2);
        self.sym(-77.5 + ox, -13.0 + oz, 3.0, 0.4, 0.0, 1.15, Concrete, 1);
        self.sym(-72.0 + ox, -15.8 + oz, 1.2, 0.8, 0.0, 1.5, Metal, 1);
        self.crates(rng, -79.0 + ox, -18.5 + oz, 3);

        // B / D: a pump hall with a doorway on every side. The girders
        // stay above players' heads, so the hall works for all nav lanes.
        let (ox, oz) = shift(-36.0, 20.0);
        for x in [-40.8, -31.2] {
            self.sym(x + ox, 14.0 + oz, 2.2, 0.4, 0.0, 3.7, Concrete, 0);
            self.sym(x + ox, 26.0 + oz, 2.2, 0.4, 0.0, 3.7, Concrete, 1);
        }
        for z in [15.8, 24.2] {
            self.sym(-43.0 + ox, z + oz, 0.4, 1.8, 0.0, 3.7, Concrete, 1);
            self.sym(-29.0 + ox, z + oz, 0.4, 1.8, 0.0, 3.7, Concrete, 0);
        }
        let roof = self.height(-36.0 + ox, 20.0 + oz) + 4.4;
        self.sym_abs(-36.0 + ox, 14.0 + oz, 7.6, 0.35, roof, roof + 0.45, Metal, 1);
        self.sym_abs(-36.0 + ox, 26.0 + oz, 7.6, 0.35, roof, roof + 0.45, Metal, 1);
        for x in [-40.0, -32.0] {
            self.sym(x + ox, 23.3 + oz, 1.1, 1.0, 0.0, 1.5, Metal, 0);
            self.sym(x + ox, 23.3 + oz, 0.35, 0.35, 1.5, 0.5, Metal, 1);
            self.sym_abs(x + ox, 20.0 + oz, 0.22, 6.1, roof + 0.45, roof + 0.8, Metal, 0);
        }
        self.crates(rng, -33.5 + ox, 17.5 + oz, 3);

        // C: the gate deck over the channel, above the flood, built last
        // (see `spillway`). A low wall, sandbags and crates on the channel
        // floor under it.
        let stand_ins = spillway::reserve(self);
        self.sym(-6.0, -3.6, 0.4, 2.4, 0.0, 1.05, Concrete, 2);
        self.sym(-11.0, 4.5, 2.1, 0.4, 0.0, 1.05, Sandbag, 0);
        self.crates(rng, -9.0, -6.5, 3);

        // Broken retaining walls expose ramps between the service roads
        // and channel. Gaps separate each short wall instead of forcing
        // everyone through the floodgate.
        for x in [-87.0f32, -58.0, -30.0] {
            for z in [-13.8f32 * SPREAD, 13.8 * SPREAD] {
                self.sym(x, z, 5.5, 0.4, 0.0, 2.0, Concrete, 2);
                self.sym(x, z, 0.6, 0.65, 2.0, 0.35, Concrete, 1);
            }
        }
        // Raised service pipe racks distinguish the outer flank routes.
        for z in [-42.0f32 * SPREAD, 42.0 * SPREAD] {
            let rack = self.height(-72.0, z) + 4.8;
            for x in [-90.0f32, -78.0, -66.0, -54.0] {
                self.sym_abs(x, z, 0.22, 0.22, self.height(x, z) - 0.6, rack + 0.6, Metal, 0);
            }
            self.sym_abs(-72.0, z, 18.5, 0.45, rack, rack + 0.55, Metal, 1);
        }

        // Pump houses, valve huts, settling tanks, filter-bed sheds, walled
        // sludge pens and lookouts over the channel fill the ground between
        // the yards.
        self.scatter_buildings(rng, 5, &[(5.0, 3.5, 4.0), (6.0, 4.0, 4.4)], Concrete);
        self.scatter_buildings(rng, 3, &[(2.6, 2.2, 2.9)], Metal);
        self.scatter_tanks(rng, 6);
        self.scatter_lookouts(rng, 2, 3.6);
        self.scatter_sheds(rng, 3);
        self.scatter_ruins(rng, 3);

        self.lane_cover(rng, 7, 13.0);
        self.boulders(rng, 26, 0.7, 1.3);
        self.forest(rng, 40, 205, 0.45);
        spillway::build(self, stand_ins);
    }

    // ------------------------------------------------------------- Relay

    fn build_relay(&mut self, rng: &mut Rng) {
        use BlockKind::*;
        self.build_bases();

        // A / E: forward generator shelters, with a clear central yard.
        let (ox, oz) = shift(-78.0, 16.0);
        self.ruin(rng, -78.0 + ox, 16.0 + oz, 14.0, 11.0, 3.1, (-300.0, 300.0), 0b1111);
        self.sym(-82.5 + ox, 18.5 + oz, 1.3, 1.0, 0.0, 1.45, Metal, 0);
        self.sym(-72.0 + ox, 10.5 + oz, 2.4, 0.4, 0.0, 1.1, Sandbag, 1);
        self.crates(rng, -76.0 + ox, 18.5 + oz, 3);

        // B / D: communications cabins and an antenna array. Low
        // equipment bays provide cover without filling the objective.
        let (ox, oz) = shift(-38.0, -26.0);
        self.sym(-43.5 + ox, -31.5 + oz, 4.0, 1.5, 0.0, 2.7, Container, 3);
        self.sym(-31.0 + ox, -29.0 + oz, 1.2, 2.5, 0.0, 1.55, Metal, 1);
        self.sym(-38.0 + ox, -19.5 + oz, 2.7, 0.4, 0.0, 1.1, Sandbag, 0);
        self.sym(-45.0 + ox, -23.0 + oz, 0.4, 2.4, 0.0, 1.1, Concrete, 1);
        self.crates(rng, -39.0 + ox, -28.5 + oz, 4);
        let (x, z, flights, rot) = relay::TOWERS[0];
        relay::tower(self, x, z, flights, rot);

        // C: the relay exchange. Two climbable towers frame an open
        // switchyard, so the centre reads at a distance from every lane.
        let (x, z, flights, rot) = relay::TOWERS[1];
        relay::tower(self, x, z, flights, rot);
        self.sym(-8.5, -7.0, 2.0, 1.0, 0.0, 1.4, Metal, 0);
        self.sym(-4.5, 5.0, 2.1, 0.4, 0.0, 1.0, Concrete, 2);
        self.sym(-4.0, -4.0, 0.4, 2.0, 0.0, 1.0, Sandbag, 1);
        self.crates(rng, -10.0, -4.0, 3);

        // A great listening dish stands on a pedestal in each half; the dish
        // itself is overhead, so only the pedestal is cover.
        let (dx, dz) = relay::DISH;
        self.sym(dx, dz, relay::DISH_PEDESTAL_HALF, relay::DISH_PEDESTAL_HALF, 0.0, relay::DISH_PEDESTAL_H, Concrete, 3);

        // BACKBONE, the great cellular mast, stands on the north flank and
        // is climbed by ladder to a caged rest platform and a top deck.
        relay::mast(self);

        // Cable gantries run from the exchange to the ridge-side cabins.
        // The two five-metre service arches can be walked underneath.
        for (x, z) in relay::GANTRIES {
            let top = self.height(x, z) + 5.5;
            for dz in [-3.8f32, 3.8] {
                self.sym_abs(x, z + dz, 0.3, 0.3, self.height(x, z + dz) - 0.6, top, Metal, 0);
            }
            self.sym_abs(x, z, 0.4, 4.3, top, top + 0.5, Metal, 1);
        }

        // Low rock shelves along the edges split long sightlines. The
        // centre and outer flanks are deliberately left as terrain ramps.
        for (x, z) in [(-61.0f32, -43.0f32), (-21.0, 37.0), (-56.0, 39.0), (-14.0, -44.0)] {
            self.rock(rng, x * SPREAD, z * SPREAD, 2.3);
        }

        // Equipment huts, fuel tanks and a pair of survey lookouts dot the
        // plateau between the station's buildings.
        self.scatter_buildings(rng, 6, &[(3.0, 2.5, 3.0), (4.2, 3.0, 3.3)], Metal);
        self.scatter_tanks(rng, 3);
        self.scatter_lookouts(rng, 2, 3.6);
        self.scatter_ruins(rng, 2);

        self.lane_cover(rng, 7, 15.0);
        self.boulders(rng, 62, 0.8, 1.6);
        self.forest(rng, 140, 260, 0.95);
    }

    // ---------------------------------------------------- Foundry Valley

    fn build_structures(&mut self, rng: &mut Rng) {
        use BlockKind::*;
        self.build_bases();
        landmarks::build(self);

        // --- A / E: walled outpost ---
        let (ox, oz) = shift(-72.0, 14.0);
        self.ruin(rng, -72.0 + ox, 14.0 + oz, 13.0, 10.0, 3.4, (-300.0, 300.0), 0b1111);
        self.crates(rng, -72.0 + ox, 14.0 + oz, 4);
        self.sym(-62.5 + ox, 10.0 + oz, 0.45, 2.6, 0.0, 1.1, Sandbag, 0);
        self.sym(-66.0 + ox, 22.5 + oz, 2.6, 0.45, 0.0, 1.1, Sandbag, 1);

        // --- B / D: container yard ---
        let (ox, oz) = shift(-36.0, -24.0);
        self.sym(-41.0 + ox, -20.5 + oz, 3.0, 1.25, 0.0, 2.6, Container, 0);
        self.sym(-32.0 + ox, -30.0 + oz, 1.25, 3.0, 0.0, 2.6, Container, 1);
        self.sym(-30.5 + ox, -19.5 + oz, 3.0, 1.25, 0.0, 2.6, Container, 2);
        self.sym(-30.5 + ox, -19.5 + oz, 3.0, 1.25, 2.6, 2.6, Container, 3);
        self.sym(-43.0 + ox, -30.5 + oz, 1.25, 3.0, 0.0, 2.6, Container, 3);
        self.sym(-36.5 + ox, -14.5 + oz, 2.4, 0.45, 0.0, 1.1, Sandbag, 0);
        self.crates(rng, -36.5 + ox, -25.0 + oz, 5);

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

        // --- The valley's farms and workshops: houses and barns, ruined
        // walled yards, water-tower lookouts and the foundry's old tanks ---
        self.scatter_buildings(rng, 5, &[(4.0, 3.2, 3.4), (5.0, 3.5, 3.6)], Concrete);
        self.scatter_buildings(rng, 3, &[(7.0, 4.5, 4.8)], Metal);
        self.scatter_ruins(rng, 4);
        self.scatter_lookouts(rng, 2, 4.2);
        self.scatter_tanks(rng, 2);

        self.lane_cover(rng, 7, 18.0);
        self.boulders(rng, 100, 0.7, 1.6);
        self.forest(rng, 200, 240, 0.4);
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
    /// Where a map has walkways over the ground (Spillway's gate deck), the
    /// surface walked on in each cell and which cells are walkway. A route
    /// never steps between a walkway and anything a metre above or below
    /// it, so walkways are only joined where they meet the ground or each
    /// other. Empty on other maps.
    ground: Vec<f32>,
    raised: Vec<bool>,
    /// Spillway: routes that keep out of the channel where a full flood
    /// would be deep, used while it floods. Empty on other maps.
    wet: Vec<Vec<u8>>,
}

fn near_blocked(blocked: &[bool], w: usize, h: usize) -> Vec<bool> {
    let mut near = vec![false; w * h];
    for cz in 0..h as i32 {
        for cx in 0..w as i32 {
            near[cz as usize * w + cx as usize] = DIRS.iter().any(|&(dx, dz)| {
                let (nx, nz) = (cx + dx, cz + dz);
                nx >= 0 && nz >= 0 && nx < w as i32 && nz < h as i32 && blocked[nz as usize * w + nx as usize]
            });
        }
    }
    near
}

impl Nav {
    pub fn build(world: &World) -> Nav {
        Self::build_mask(world, 0)
    }

    /// Rebuild routes against the authoritative destroyed-feature state.
    pub fn build_mask(world: &World, dmask: u64) -> Nav {
        let w = (2.0 * HALF_X / NAV_CELL).ceil() as usize;
        let h = (2.0 * HALF_Z / NAV_CELL).ceil() as usize;
        let mut blocked = vec![false; w * h];
        let mut ground = vec![0.0; w * h];
        let mut raised = vec![false; w * h];
        for cz in 0..h {
            for cx in 0..w {
                let x = -HALF_X + (cx as f32 + 0.5) * NAV_CELL;
                let z = -HALF_Z + (cz as f32 + 0.5) * NAV_CELL;
                ground[cz * w + cx] = world.walk_height(x, z);
                raised[cz * w + cx] = world.map == spillway::MAP_ID && spillway::walkway(world, x, z).is_some();
                blocked[cz * w + cx] = world.lava_fixed(x, z, 0.9); // bots take the bridges
            }
        }
        // A structural blast rebuilds navigation during a server tick.
        // Rasterize each solid footprint instead of testing every collider
        // against every cell. Keep the exact overlap/headroom rules below.
        let r = 0.6;
        let range = |lo: f32, hi: f32, half: f32, count: usize| {
            let first = (((lo - r + half) / NAV_CELL - 0.5).floor() as i32).clamp(0, count as i32 - 1);
            let last = (((hi + r + half) / NAV_CELL - 0.5).ceil() as i32).clamp(0, count as i32 - 1);
            first as usize..=last as usize
        };
        for (i, b) in world.blocks.iter().enumerate() {
            if !world.solid(i, dmask) {
                continue;
            }
            let xs = range(b.min.x, b.max.x, HALF_X, w);
            for cz in range(b.min.z, b.max.z, HALF_Z, h) {
                let z = -HALF_Z + (cz as f32 + 0.5) * NAV_CELL;
                if z + r <= b.min.z || z - r >= b.max.z {
                    continue;
                }
                for cx in xs.clone() {
                    let cell = cz * w + cx;
                    if blocked[cell] {
                        continue;
                    }
                    let x = -HALF_X + (cx as f32 + 0.5) * NAV_CELL;
                    let g = ground[cell];
                    if x + r > b.min.x && x - r < b.max.x && b.max.y - g > 0.45 && b.min.y - g < 1.9 {
                        blocked[cell] = true;
                    }
                }
            }
        }
        let near = near_blocked(&blocked, w, h);
        let walkways = raised.iter().any(|&r| r);
        let (ground, raised) = if walkways { (ground, raised) } else { (Vec::new(), Vec::new()) };
        let mut nav = Nav { w, h, blocked, fields: Vec::new(), ground, raised, wet: Vec::new() };
        let changed_features = world.features.iter().any(|f| dmask & (1u64 << f.bit) != 0);
        let workers = if changed_features { std::thread::available_parallelism().map_or(1, |n| n.get().min(4)) } else { 1 };
        let compute = |job: usize| {
            let goal = job / NAV_LANES;
            let gp = if goal < 5 { world.points[goal] } else { world.bases[goal - 5] };
            nav.field(gp.x, gp.z, job % NAV_LANES, &near)
        };
        if workers == 1 {
            nav.fields = (0..NAV_GOALS * NAV_LANES).map(compute).collect();
        } else {
            // Each goal/lane field is independent. Bound the thread count
            // during a blast and collect by index, preserving the exact same
            // routes regardless of worker scheduling or machine core count.
            let mut fields = vec![Vec::new(); NAV_GOALS * NAV_LANES];
            std::thread::scope(|scope| {
                let jobs: Vec<_> = (0..workers)
                    .map(|worker| {
                        let compute = &compute;
                        scope.spawn(move || {
                            (worker..NAV_GOALS * NAV_LANES).step_by(workers).map(|job| (job, compute(job))).collect::<Vec<_>>()
                        })
                    })
                    .collect();
                for worker in jobs {
                    for (job, field) in worker.join().expect("navigation worker") {
                        fields[job] = field;
                    }
                }
            });
            nav.fields = fields;
        }
        if world.map == spillway::MAP_ID {
            // Flood routes: the channel is as good as a wall where a full
            // flood would be deep, so bots cross by the deck and keep to the
            // banks; one caught in the water takes the shortest way out.
            let wet: Vec<bool> = (0..w * h)
                .map(|cell| {
                    let c = nav.center(cell % w, cell / w);
                    nav.blocked[cell]
                        || (!nav.raised.get(cell).copied().unwrap_or(false)
                            && spillway::water_surface(world, c.x, c.y, spillway::FULL)
                                .is_some_and(|s| world.height(c.x, c.y) < s - spillway::WADE))
                })
                .collect();
            let near = near_blocked(&wet, w, h);
            nav.wet = (0..NAV_GOALS * NAV_LANES)
                .map(|job| {
                    let goal = job / NAV_LANES;
                    let gp = if goal < 5 { world.points[goal] } else { world.bases[goal - 5] };
                    nav.field_on(&wet, gp.x, gp.z, job % NAV_LANES, &near)
                })
                .collect();
        }
        nav
    }

    /// Whether a route may not step between cells `a` and `b`: one is a
    /// walkway and the other is more than a metre above or below it.
    fn cliff(&self, a: usize, b: usize) -> bool {
        !self.raised.is_empty() && (self.raised[a] || self.raised[b]) && (self.ground[a] - self.ground[b]).abs() > 1.0
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
        self.field_on(&self.blocked, gx, gz, lane, near)
    }

    fn field_on(&self, blocked: &[bool], gx: f32, gz: f32, lane: usize, near: &[bool]) -> Vec<u8> {
        let (w, h) = (self.w, self.h);
        // No stepping off a walkway, nor cutting a corner past its edge.
        let apart = |c: usize, nx: usize, nz: usize, k: usize| {
            let (cx, cz) = (c % w, c / w);
            self.cliff(c, nz * w + nx) || (k >= 4 && (self.cliff(c, cz * w + nx) || self.cliff(c, nz * w + cx)))
        };
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
                if k >= 4 && (blocked[cz * w + nx] || blocked[nz * w + cx]) {
                    continue; // no cutting corners
                }
                if apart(cz * w + cx, nx, nz, k) {
                    continue;
                }
                let z = -HALF_Z + (nz as f32 + 0.5) * NAV_CELL;
                let lane_cost = match lane {
                    1 => 1.0 + ((20.0 - z).max(0.0) / 40.0),
                    2 => 1.0 + ((z + 20.0).max(0.0) / 40.0),
                    _ => 1.0,
                };
                let mut c = if k >= 4 { 141.0 } else { 100.0 } * lane_cost;
                if blocked[ni] {
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
                    if k >= 4 && (blocked[cz * w + nx] || blocked[nz * w + cx]) {
                        continue;
                    }
                    if apart(cz * w + cx, nx, nz, k) {
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
        self.dir_in(false, goal, lane, x, z)
    }

    /// As `dir`, on the routes round flood water while `flooding` (see
    /// `World::flooding`).
    pub fn dir_in(&self, flooding: bool, goal: usize, lane: usize, x: f32, z: f32) -> Vec2 {
        let set = if flooding && !self.wet.is_empty() { &self.wet } else { &self.fields };
        let f = &set[goal * NAV_LANES + lane % NAV_LANES];
        let (mut cx, mut cz) = self.cell(x, z);
        let layer = |cx: usize, cz: usize| self.raised.get(cz * self.w + cx).copied().unwrap_or(false);
        let start = layer(cx, cz);
        for _ in 0..3 {
            let k = f[cz * self.w + cx];
            if k >= 8 {
                break;
            }
            cx = (cx as i32 + DIRS[k as usize].0) as usize;
            cz = (cz as i32 + DIRS[k as usize].1) as usize;
            // Aim at the first cell on or off a walkway, not past it: a
            // straight line beyond would cut across its edge.
            if layer(cx, cz) != start {
                break;
            }
        }
        (self.center(cx, cz) - vec2(x, z)).normalize_or_zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canopies_hide_and_trunks_do_not_blind() {
        let w = World::new(0xFE221);
        let mut checked = 0;
        for t in w.trees.iter().filter(|t| t.base.x.abs() < 60.0 && t.base.z.abs() < 40.0).take(20) {
            let side = vec3(t.radius * 3.0, 0.0, 0.0);
            // Straight through the middle of the leaves.
            let y = t.base.y + t.height * if t.pine { 0.42 } else { 0.72 };
            let mid = vec3(t.base.x, y, t.base.z) + vec3(0.0, 0.0, t.radius * 0.3);
            assert!(w.foliage(mid - side, mid + side), "tree at {:?}", t.base);
            // Well above the crown.
            let over = mid + vec3(0.0, t.height, 0.0);
            assert!(!w.foliage(over - side, over + side));
            checked += 1;
        }
        assert!(checked > 5);
        // Someone standing right under a tree is not hidden by it.
        let t = w.trees.iter().find(|t| t.pine && t.base.x.abs() < 60.0).unwrap();
        let foot = t.base + vec3(0.9, 1.0, 0.0);
        assert!(!w.foliage(foot + vec3(30.0, 0.6, 0.0), foot));
    }

    // Captured after the authored landmark sites and Ridgeline cargo wreck
    // (map 2 recaptured for the crash furrow, frames and strewn wreckage).
    // Millimetre quantisation tolerates tiny floating-point differences
    // between targets, but covers the complete terrain, collision geometry,
    // trees, objectives and crate assignment of every map.
    const FINGERPRINTS_FE221: [u64; 7] = [
        0xe5a9186a98463522,
        0x3b49936209c2b08f,
        0x3445e849b5f38255,
        0x57c62f6f2f7b39da,
        0xcb1a2501a121ed4e,
        0xa5240978f9ddea9d,
        0xa4522d55ba21aae2,
    ];
    const FINGERPRINTS_D091: [u64; 7] = [
        0x56fcf92cda86bcfc,
        0x38841009e290d9e0,
        0xe88f753d78825fa3,
        0x57246f9b0a5621e4,
        0x565ba0334f8c8517,
        0xea78cb32cce6e12b,
        0xa89dded116c53bbc,
    ];

    fn fingerprint(w: &World) -> u64 {
        let mut h = 0xcbf29ce484222325u64;
        let mut add = |v: u32| {
            for b in v.to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
        };
        let mm = |v: f32| (v * 1000.0).round() as i32 as u32;
        add(w.n_crates as u32);
        for &v in &w.heights {
            add(mm(v));
        }
        for b in &w.blocks {
            for v in b.min.to_array().into_iter().chain(b.max.to_array()) {
                add(mm(v));
            }
            add(b.kind as u32);
            add(b.tint as u32);
            add(b.dest.map_or(64, |n| n as u32));
        }
        for feature in &w.features {
            add(feature.kind as u32);
            add(feature.bit as u32);
            for v in feature.pos.to_array() {
                add(mm(v));
            }
        }
        for t in &w.trees {
            for v in t.base.to_array().into_iter().chain([t.height, t.radius]) {
                add(mm(v));
            }
            add(t.tint as u32);
            add(t.pine as u32);
        }
        for p in w.points.iter().chain(w.bases.iter()) {
            for v in p.to_array() {
                add(mm(v));
            }
        }
        h
    }

    #[test]
    fn parallel_navigation_fields_match_serial_routes_after_a_breach() {
        for map in 0..4 {
            let world = World::with_map(0xFE221, map);
            let nav = Nav::build_mask(&world, 1u64 << world.features[0].bit);
            let near: Vec<bool> = (0..nav.w * nav.h)
                .map(|cell| {
                    let (cx, cz) = ((cell % nav.w) as i32, (cell / nav.w) as i32);
                    DIRS.iter().any(|&(dx, dz)| {
                        let (nx, nz) = (cx + dx, cz + dz);
                        nx >= 0 && nz >= 0 && nx < nav.w as i32 && nz < nav.h as i32 && nav.blocked[nz as usize * nav.w + nx as usize]
                    })
                })
                .collect();
            for job in 0..NAV_GOALS * NAV_LANES {
                let goal = job / NAV_LANES;
                let gp = if goal < 5 { world.points[goal] } else { world.bases[goal - 5] };
                assert!(
                    nav.fields[job] == nav.field(gp.x, gp.z, job % NAV_LANES, &near),
                    "map {map}: parallel navigation changed goal/lane {job}"
                );
            }
        }
    }

    #[test]
    fn navigation_rasterization_matches_collision_sampling() {
        for seed in [17, 0xFE221] {
            for map in 0..MAPS.len() as u8 {
                let world = World::with_map(seed, map);
                let features = world.features.iter().fold(0u64, |mask, f| mask | 1u64 << f.bit);
                let mut masks = vec![0, features, u64::MAX];
                masks.sort_unstable();
                masks.dedup();
                for mask in masks {
                    let nav = Nav::build_mask(&world, mask);
                    for cz in 0..nav.h {
                        for cx in 0..nav.w {
                            let p = nav.center(cx, cz);
                            let g = world.walk_height(p.x, p.y);
                            let reference = world.lava(p.x, p.y, 0.9)
                                || world.blocks.iter().enumerate().any(|(i, b)| {
                                    world.solid(i, mask)
                                        && p.x + 0.6 > b.min.x
                                        && p.x - 0.6 < b.max.x
                                        && p.y + 0.6 > b.min.z
                                        && p.y - 0.6 < b.max.z
                                        && b.max.y - g > 0.45
                                        && b.min.y - g < 1.9
                                });
                            assert_eq!(
                                nav.blocked[cz * nav.w + cx],
                                reference,
                                "map {map}, seed {seed:x}, mask {mask:x}: collision differs at {p:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn map_geometry_is_unchanged() {
        // Report every changed map at once, with its new value.
        let mut changed = Vec::new();
        for (seed, expected) in [(0xFE221u64, FINGERPRINTS_FE221), (0xD091, FINGERPRINTS_D091)] {
            for (id, hash) in expected.into_iter().enumerate() {
                let now = fingerprint(&World::with_map(seed, id as u8));
                if now != hash {
                    changed.push(format!("{} (map {id}) for seed {seed:x}: now {now:#x}", MAPS[id].name));
                }
            }
        }
        assert!(
            changed.is_empty(),
            "map geometry changed:
{}",
            changed.join(
                "
"
            )
        );
    }

    #[test]
    fn every_map_is_spread_out() {
        for (id, def) in MAPS.iter().enumerate() {
            let pts = def.points;
            for w in pts.windows(2) {
                let d = ((w[0].0 - w[1].0).powi(2) + (w[0].1 - w[1].1).powi(2)).sqrt();
                assert!(d > 55.0, "{}: neighbouring objectives only {d:.1} m apart", def.name);
            }
            assert!(def.bases[0].0 < -150.0 && def.bases[1].0 > 150.0, "{}: bases too close", def.name);
            for &(x, z) in pts.iter().chain(def.bases.iter()) {
                assert!(x.abs() < HALF_X - 15.0 && z.abs() < HALF_Z - 15.0, "{}: ({x}, {z}) is near the edge", def.name);
            }
            // The terrain runs well past the battlefield on every side.
            const { assert!(TER_X0 < -HALF_X - 50.0 && TER_Z0 < -HALF_Z - 50.0) };
            let _ = id;
        }
    }

    #[test]
    fn every_map_has_buildings_between_the_objectives() {
        for seed in [0xFE221, 0xD091, 17] {
            for id in 0..MAPS.len() as u8 {
                let w = World::with_map(seed, id);
                let name = map_def(id).name;
                // A roof: a broad slab well clear of the ground beneath it.
                let roofs: Vec<&Block> = w
                    .blocks
                    .iter()
                    .filter(|b| {
                        let (cx, cz) = ((b.min.x + b.max.x) * 0.5, (b.min.z + b.max.z) * 0.5);
                        b.kind != BlockKind::Container
                            && (b.max.x - b.min.x) * (b.max.z - b.min.z) > 12.0
                            && b.max.y - b.min.y < 0.6
                            && b.min.y - w.height(cx, cz) > 2.5
                    })
                    .collect();
                assert!(roofs.len() >= 8, "{name}, seed {seed:x}: only {} roofs", roofs.len());
                // Roofed buildings have headroom and a way in.
                for r in &roofs {
                    let (cx, cz) = ((r.min.x + r.max.x) * 0.5, (r.min.z + r.max.z) * 0.5);
                    assert!(r.min.y - w.height(cx, cz) > 2.2, "{name}: low roof at ({cx}, {cz})");
                }
            }
        }
    }

    #[test]
    fn spawn_ground_stays_open() {
        for seed in [0xFE221, 0xD091, 17] {
            for id in 0..MAPS.len() as u8 {
                let w = World::with_map(seed, id);
                let nav = Nav::build(&w);
                for base in w.bases {
                    let (mut open, mut all) = (0, 0);
                    for ring in 1..=10 {
                        let r = BASE_SPAWN_RADIUS * ring as f32 / 10.0;
                        for k in 0..24 {
                            let a = k as f32 / 24.0 * std::f32::consts::TAU;
                            all += 1;
                            if !nav.is_blocked(base.x + a.cos() * r, base.z + a.sin() * r) {
                                open += 1;
                            }
                        }
                    }
                    assert!(open * 10 >= all * 8, "{}, seed {seed:x}: only {open} of {all} spawn spots open", map_def(id).name);
                }
            }
        }
    }

    #[test]
    fn new_maps_are_named_mirrored_and_navigable_across_seeds() {
        assert_eq!(map_by_name("spillway"), Some(4));
        assert_eq!(map_by_name("relay"), Some(5));
        assert_eq!(map_by_name("5"), Some(5));
        for seed in [0xFE221, 17, 0xD091] {
            for id in [4, 5] {
                let w = World::with_map(seed, id);
                let name = map_def(id).name;
                assert!(w.n_crates <= MAX_CRATES);
                assert!(w.heights.iter().all(|v| v.is_finite()));
                assert_eq!(fingerprint(&w), fingerprint(&World::with_map(seed, id)), "{name} is not deterministic");
                for b in &w.blocks {
                    assert!(b.min.is_finite() && b.max.is_finite() && b.min.cmplt(b.max).all(), "{name}: invalid block");
                    // Relay's single great mast stands on x = 0, as far from
                    // one base as the other: it is mirrored across that line
                    // (which swaps the bases) rather than by the half turn.
                    let mast = id == relay::MAP_ID && relay::mast_owns(b);
                    let mirrored = |q: &Block| {
                        let (min_z, max_z) = if mast { (b.min.z, b.max.z) } else { (-b.max.z, -b.min.z) };
                        (q.min.x + b.max.x).abs() < 0.002
                            && (q.max.x + b.min.x).abs() < 0.002
                            && (q.min.z - min_z).abs() < 0.002
                            && (q.max.z - max_z).abs() < 0.002
                            && (q.min.y - b.min.y).abs() < 0.05
                            && (q.max.y - b.max.y).abs() < 0.05
                            && q.kind == b.kind
                            && q.tint == b.tint
                            && q.dest.is_some() == b.dest.is_some()
                    };
                    assert!(w.blocks.iter().any(mirrored), "{name}: unmatched {:?} at {:?}", b.kind, b.min);
                }
                if id == relay::MAP_ID {
                    // On the centre line, so equidistant from both bases.
                    const { assert!(relay::BACKBONE.0 == 0.0) };
                    assert!(w.blocks.iter().filter(|b| relay::mast_owns(b)).count() > 100, "{name}: the mast has no collision");
                }
                let nav = Nav::build(&w);
                for goal in 0..NAV_GOALS {
                    let target = if goal < 5 { w.points[goal] } else { w.bases[goal - 5] };
                    for start in w.bases.iter().chain(w.points.iter()) {
                        for lane in 0..NAV_LANES {
                            let mut p = vec2(start.x, start.z);
                            for _ in 0..1600 {
                                if p.distance(vec2(target.x, target.z)) < 6.5 {
                                    break;
                                }
                                p += nav.dir(goal, lane, p.x, p.y) * 0.6;
                            }
                            assert!(
                                p.distance(vec2(target.x, target.z)) < 6.5,
                                "{name}, seed {seed:x}: no lane {lane} route from {start:?} to goal {goal}; stopped at {p:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}
