//! Map documents (`.fmap`): a battlefield as data rather than code.
//!
//! The built-in maps are built by code from `(seed, map id)`. A document
//! holds everything such a build produces - the ground, the collision
//! boxes, trees, objectives, ladders and seedlines - so the map maker can
//! write one and client and server can each build the same [`World`] from
//! it. A document is pure data: nothing in it is run.
//!
//! On disk a document is JSON, so it diffs well and can be fixed by hand;
//! on the wire it is bincode. Both paths go through [`MapDoc::validate`],
//! which bounds every count and coordinate, because a document may come
//! from someone else's server.
//!
//! Every FERRITE map is the same under a half turn about the centre. A
//! document in [`Symmetry::HalfTurn`] holds only the west half's things;
//! the east half is generated when the world is built.

use super::{
    full_height, BlockKind, Feature, FeatureKind, Ladder, MapDef, Seedline, Tree, World, MAPS, MAX_CRATES, TER_X0, TER_Z0, TNX, TNZ,
};
use super::{Block, HALF_X, HALF_Z};
use bincode::Options;
use glam::{vec3, Vec3};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// The schema version written by this build. Older documents are migrated
/// on load; newer ones are refused.
pub const FORMAT: u16 = 1;
/// Map ids at and above this are custom maps for this session; see
/// [`install`]. Built-in ids stay below it.
pub const FIRST_CUSTOM: u8 = 128;
/// Limits on what a document may hold, so a hostile one cannot stall or
/// exhaust whoever loads it.
pub const MAX_BLOCKS: usize = 4096;
pub const MAX_TREES: usize = 4096;
pub const MAX_FEATURES: usize = 64;
pub const MAX_LADDERS: usize = 32;
pub const MAX_LINES: usize = 16;
pub const MAX_LINE_POINTS: usize = 64;
pub const MAX_JSON: usize = 8 << 20;
pub const MAX_WIRE: u64 = 2 << 20;
/// How far below and above the ground's zero anything may reach.
pub const Y_RANGE: (f32, f32) = (-200.0, 400.0);

const CELLS: usize = (TNX + 1) * (TNZ + 1);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MapDoc {
    pub format: u16,
    pub meta: Meta,
    pub look: Look,
    pub symmetry: Symmetry,
    pub terrain: Terrain,
    /// Objectives A to E and the two bases, on the ground. In half-turn
    /// symmetry only A, B and the west base are used; C is the centre.
    pub points: [[f32; 2]; 5],
    pub bases: [[f32; 2]; 2],
    /// Collision boxes, each drawn by the styling for its kind.
    pub brushes: Vec<Brush>,
    #[serde(default)]
    pub trees: Vec<TreeDoc>,
    #[serde(default)]
    pub features: Vec<FeatureDoc>,
    #[serde(default)]
    pub ladders: Vec<LadderDoc>,
    /// Seedlines, each the points a rider's feet follow from dock to dock.
    #[serde(default)]
    pub lines: Vec<Vec<[f32; 3]>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Meta {
    pub name: String,
    #[serde(default)]
    pub blurb: String,
    #[serde(default)]
    pub author: String,
    /// Bumped on every save.
    #[serde(default)]
    pub revision: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Look {
    /// Ground palette: grass, dry patches, packed dirt around objectives.
    pub ground: [[f32; 3]; 3],
    /// The built-in map whose sky, sun, fog and foliage to borrow.
    #[serde(default)]
    pub mood: u8,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Symmetry {
    /// The east half is the west half turned about the centre.
    HalfTurn,
    /// Everything is written out; nothing is generated.
    None,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Terrain {
    /// The built-in map whose ground to start from, or flat at zero.
    pub preset: Option<u8>,
    /// The seed that ground was shaped with.
    #[serde(default)]
    pub seed: u32,
    /// Centimetres added to each grid height, west to east and then north
    /// to south, as runs of `(count, centimetres)`. Empty is no change.
    #[serde(default)]
    pub delta: Vec<(u32, i16)>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Brush {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub kind: BlockKind,
    #[serde(default)]
    pub tint: u8,
    /// Destruction slot: a crate, or a group a grenade can bring down.
    #[serde(default)]
    pub dest: Option<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TreeDoc {
    pub at: [f32; 3],
    pub height: f32,
    pub radius: f32,
    #[serde(default)]
    pub tint: u8,
    #[serde(default)]
    pub pine: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FeatureDoc {
    pub kind: FeatureKind,
    pub bit: u8,
    pub at: [f32; 3],
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LadderDoc {
    pub at: [f32; 3],
    /// Horizontal direction a climber faces, into the rungs.
    pub face: [f32; 3],
    pub top: f32,
}

fn v3(a: [f32; 3]) -> Vec3 {
    Vec3::from_array(a)
}

/// The half turn about the map's centre.
fn turn(p: Vec3) -> Vec3 {
    vec3(-p.x, p.y, -p.z)
}

impl MapDoc {
    /// An empty map on flat ground with the standard objective layout.
    pub fn blank(name: &str) -> MapDoc {
        MapDoc {
            format: FORMAT,
            meta: Meta { name: name.to_string(), blurb: String::new(), author: String::new(), revision: 0 },
            look: Look { ground: MAPS[0].ground, mood: 0 },
            symmetry: Symmetry::HalfTurn,
            terrain: Terrain { preset: None, seed: 0, delta: Vec::new() },
            points: super::POINTS_XZ.map(|(x, z)| [x, z]),
            bases: super::BASES_XZ.map(|(x, z)| [x, z]),
            brushes: Vec::new(),
            trees: Vec::new(),
            features: Vec::new(),
            ladders: Vec::new(),
            lines: Vec::new(),
        }
    }

    /// Everything a built world holds, written out with no symmetry. Maps
    /// with underground tunnels cannot be written yet. Bespoke art and
    /// map-specific rules (floods, lava, gusts) do not carry over: they
    /// belong to the built-in map's id, not to its data.
    pub fn from_world(w: &World) -> Result<MapDoc, String> {
        if !w.tunnels.is_empty() {
            return Err("maps with tunnels cannot be written as documents yet".into());
        }
        let def = super::map_def(w.map);
        let s = (w.seed ^ (w.seed >> 32)) as u32;
        let mut doc = MapDoc::blank(def.name);
        doc.meta.blurb = def.blurb.to_string();
        doc.look = Look { ground: def.ground, mood: w.map };
        doc.symmetry = Symmetry::None;
        let base = base_heights(Some(w.map), s);
        let cm: Vec<i16> = w.heights.iter().zip(&base).map(|(h, b)| ((h - b) * 100.0).round() as i16).collect();
        doc.terrain = Terrain { preset: Some(w.map), seed: s, delta: if cm.iter().all(|&c| c == 0) { Vec::new() } else { runs(&cm) } };
        doc.points = w.points.map(|p| [p.x, p.z]);
        doc.bases = w.bases.map(|p| [p.x, p.z]);
        doc.brushes = w
            .blocks
            .iter()
            .map(|b| Brush { min: b.min.to_array(), max: b.max.to_array(), kind: b.kind, tint: b.tint, dest: b.dest })
            .collect();
        doc.trees = w
            .trees
            .iter()
            .map(|t| TreeDoc { at: t.base.to_array(), height: t.height, radius: t.radius, tint: t.tint, pine: t.pine })
            .collect();
        doc.features = w.features.iter().map(|f| FeatureDoc { kind: f.kind, bit: f.bit, at: f.pos.to_array() }).collect();
        doc.ladders = w.ladders.iter().map(|l| LadderDoc { at: l.at.to_array(), face: l.face.to_array(), top: l.top }).collect();
        doc.lines = w.lines.iter().map(|l| l.pts.iter().map(|p| p.to_array()).collect()).collect();
        Ok(doc)
    }

    /// JSON laid out to read and diff well: anything that fits on a line
    /// (a block, a tree, a point) is written on one.
    pub fn to_json(&self) -> String {
        // Through text, so each f32 keeps its shortest spelling.
        let flat = serde_json::to_string(self).expect("a map document always serialises");
        let v: serde_json::Value = serde_json::from_str(&flat).expect("its own JSON parses");
        let mut out = String::new();
        lay_out(&v, 0, &mut out);
        out.push('\n');
        out
    }

    pub fn from_json(text: &str) -> Result<MapDoc, String> {
        if text.len() > MAX_JSON {
            return Err(format!("map file is over {} MB", MAX_JSON >> 20));
        }
        let mut doc: MapDoc = serde_json::from_str(text).map_err(|e| format!("not a map file: {e}"))?;
        doc.migrate()?;
        doc.validate()?;
        Ok(doc)
    }

    fn wire() -> impl Options {
        bincode::DefaultOptions::new().with_limit(MAX_WIRE)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        Self::wire().serialize(self).expect("a map document always serialises")
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<MapDoc, String> {
        let mut doc: MapDoc = Self::wire().deserialize(bytes).map_err(|e| format!("bad map data: {e}"))?;
        doc.migrate()?;
        doc.validate()?;
        Ok(doc)
    }

    fn migrate(&mut self) -> Result<(), String> {
        match self.format {
            FORMAT => Ok(()),
            f if f > FORMAT => Err(format!("this map needs a newer FERRITE (format {f})")),
            f => Err(format!("unknown map format {f}")),
        }
    }

    /// Check every count, name and coordinate against the limits.
    pub fn validate(&self) -> Result<(), String> {
        if self.format != FORMAT {
            return Err(format!("map format {} is not {FORMAT}", self.format));
        }
        let text = |what: &str, s: &str, max: usize, empty_ok: bool| -> Result<(), String> {
            if !empty_ok && s.trim().is_empty() {
                return Err(format!("{what} is empty"));
            }
            if s.len() > max || !s.chars().all(|c| c.is_ascii_graphic() || c == ' ') {
                return Err(format!("{what} must be at most {max} plain characters"));
            }
            Ok(())
        };
        text("map name", &self.meta.name, 32, false)?;
        text("blurb", &self.meta.blurb, 120, true)?;
        text("author", &self.meta.author, 32, true)?;
        if self.look.ground.iter().flatten().any(|c| !(0.0..=1.0).contains(c)) {
            return Err("ground colours must be between 0 and 1".into());
        }
        if self.look.mood as usize >= MAPS.len() {
            return Err("unknown mood".into());
        }
        if self.terrain.preset.is_some_and(|p| p as usize >= MAPS.len()) {
            return Err("unknown terrain preset".into());
        }
        if !self.terrain.delta.is_empty() {
            let mut n = 0u64;
            for &(count, cm) in &self.terrain.delta {
                if cm.unsigned_abs() > 20_000 {
                    return Err("terrain moved more than 200 m".into());
                }
                n += count as u64;
            }
            if n != CELLS as u64 {
                return Err(format!("terrain has {n} heights, not {CELLS}"));
            }
        }
        let flat = |p: &[f32; 2]| p[0].is_finite() && p[1].is_finite() && p[0].abs() <= HALF_X && p[1].abs() <= HALF_Z;
        if !self.points.iter().chain(&self.bases).all(flat) {
            return Err("objectives and bases must be on the battlefield".into());
        }
        let inside = |p: &[f32; 3]| {
            p.iter().all(|v| v.is_finite())
                && (TER_X0..=-TER_X0).contains(&p[0])
                && (TER_Z0..=-TER_Z0).contains(&p[2])
                && (Y_RANGE.0..=Y_RANGE.1).contains(&p[1])
        };
        let count = |what: &str, n: usize, max: usize| if n > max { Err(format!("too many {what}: {n} (at most {max})")) } else { Ok(()) };
        let half = self.symmetry == Symmetry::HalfTurn;
        // A half-turn document's items are doubled when built.
        let k = if half { 2 } else { 1 };
        count("blocks", self.brushes.len() * k, MAX_BLOCKS)?;
        count("trees", self.trees.len() * k, MAX_TREES)?;
        count("features", self.features.len() * k, MAX_FEATURES)?;
        count("ladders", self.ladders.len() * k, MAX_LADDERS)?;
        count("seedlines", self.lines.len() * k, MAX_LINES)?;
        let slots = if half { MAX_CRATES / 2 } else { MAX_CRATES };
        for b in &self.brushes {
            if !inside(&b.min) || !inside(&b.max) || (0..3).any(|a| b.max[a] - b.min[a] < 0.01) {
                return Err("a block is outside the map or has no size".into());
            }
            if b.dest.is_some_and(|d| d as usize >= slots) {
                return Err(format!("destruction groups go up to {}", slots - 1));
            }
        }
        for t in &self.trees {
            if !inside(&t.at) || !(1.0..=60.0).contains(&t.height) || !(0.2..=12.0).contains(&t.radius) {
                return Err("a tree is outside the map or the wrong size".into());
            }
        }
        for f in &self.features {
            if !inside(&f.at) || f.bit as usize >= slots {
                return Err("a breakable feature is outside the map or its group is too high".into());
            }
        }
        for l in &self.ladders {
            let face = v3(l.face);
            if !inside(&l.at)
                || !l.top.is_finite()
                || l.top <= l.at[1]
                || l.top - l.at[1] > 80.0
                || face.y.abs() > 0.01
                || (face.length() - 1.0).abs() > 0.01
            {
                return Err("a ladder is outside the map, upside down or not facing level".into());
            }
        }
        for line in &self.lines {
            if !(2..=MAX_LINE_POINTS).contains(&line.len()) || !line.iter().all(inside) {
                return Err("a seedline is outside the map or has too few or too many points".into());
            }
        }
        Ok(())
    }

    /// Where the objectives and bases are, with the east half generated in
    /// half-turn symmetry.
    pub fn layout(&self) -> ([[f32; 2]; 5], [[f32; 2]; 2]) {
        let (mut points, mut bases) = (self.points, self.bases);
        if self.symmetry == Symmetry::HalfTurn {
            points[2] = [0.0, 0.0];
            points[3] = [-points[1][0], -points[1][1]];
            points[4] = [-points[0][0], -points[0][1]];
            bases[1] = [-bases[0][0], -bases[0][1]];
        }
        (points, bases)
    }

    /// Turn a written-out document into a half-turn one, keeping the west
    /// half of every pair, when everything in it does pair up (as on every
    /// built-in map). Returns whether it did; if not, nothing changes.
    pub fn fold(&mut self) -> bool {
        if self.symmetry != Symmetry::None {
            return false;
        }
        match self.folded() {
            Some(d) => {
                *self = d;
                true
            }
            None => false,
        }
    }

    fn folded(&self) -> Option<MapDoc> {
        let close = |a: Vec3, b: Vec3| (a - b).abs().max_element() < 2e-3;
        let p2 = |p: [f32; 2]| vec3(p[0], 0.0, p[1]);
        let (pts, bases) = (&self.points, &self.bases);
        if !close(p2(pts[2]), Vec3::ZERO)
            || !close(p2(pts[3]), turn(p2(pts[1])))
            || !close(p2(pts[4]), turn(p2(pts[0])))
            || !close(p2(bases[1]), turn(p2(bases[0])))
        {
            return None;
        }
        // Pair each item with its turned twin; `keep` says which of a pair
        // stays (the west one).
        fn pair<T>(items: &[T], at: impl Fn(&T) -> Vec3, twin: impl Fn(&T, &T) -> bool) -> Option<Vec<(usize, Option<usize>)>> {
            let mut used = vec![false; items.len()];
            let mut out = Vec::new();
            for i in 0..items.len() {
                if used[i] {
                    continue;
                }
                used[i] = true;
                if twin(&items[i], &items[i]) {
                    out.push((i, None));
                    continue;
                }
                let j = (i + 1..items.len()).find(|&j| !used[j] && twin(&items[i], &items[j]))?;
                used[j] = true;
                let west = |k: usize| {
                    let p = at(&items[k]);
                    (p.x, p.z)
                };
                out.push(if west(i) <= west(j) { (i, Some(j)) } else { (j, Some(i)) });
            }
            Some(out)
        }
        let boxes = |b: &Brush| {
            let (a, c) = (turn(v3(b.min)), turn(v3(b.max)));
            (a.min(c), a.max(c))
        };
        let brushes = pair(
            &self.brushes,
            |b| (v3(b.min) + v3(b.max)) * 0.5,
            |a, b| {
                let (min, max) = boxes(a);
                a.kind == b.kind
                    && a.tint == b.tint
                    && a.dest.is_some() == b.dest.is_some()
                    && close(min, v3(b.min))
                    && close(max, v3(b.max))
            },
        )?;
        // Destruction slots: each pair becomes one authored group.
        let mut group = std::collections::HashMap::new();
        let mut next = 0u8;
        for &(i, j) in &brushes {
            if let Some(d) = self.brushes[i].dest {
                if group.contains_key(&d) {
                    continue;
                }
                let twin = j.and_then(|j| self.brushes[j].dest);
                if twin.is_some_and(|t| group.contains_key(&t)) {
                    return None;
                }
                group.insert(d, (next, false));
                if let Some(t) = twin.filter(|&t| t != d) {
                    group.insert(t, (next, true));
                }
                next += 1;
            }
        }
        let features = pair(&self.features, |f| v3(f.at), |a, b| a.kind == b.kind && close(turn(v3(a.at)), v3(b.at)))?;
        for &(i, j) in &features {
            let g = group.get(&self.features[i].bit);
            if let Some(j) = j {
                let h = group.get(&self.features[j].bit);
                if g.map(|g| g.0) != h.map(|h| h.0) || g.is_none_or(|g| g.1) {
                    return None;
                }
            }
        }
        // Brushes whose groups only features use are not allowed for: any
        // feature must share a group with a brush.
        if self.features.iter().any(|f| !group.contains_key(&f.bit)) {
            return None;
        }
        let trees = pair(
            &self.trees,
            |t| v3(t.at),
            |a, b| a.height == b.height && a.radius == b.radius && a.pine == b.pine && close(turn(v3(a.at)), v3(b.at)),
        )?;
        let ladders = pair(
            &self.ladders,
            |l| v3(l.at),
            |a, b| a.top == b.top && close(turn(v3(a.at)), v3(b.at)) && close(turn(v3(a.face)), v3(b.face)),
        )?;
        let lines =
            pair(&self.lines, |l| v3(l[0]), |a, b| a.len() == b.len() && a.iter().zip(b).all(|(p, q)| close(turn(v3(*p)), v3(*q))))?;
        // Only pairs fold: an item that is its own twin must be a block.
        if [&trees, &ladders, &lines, &features].iter().any(|v| v.iter().any(|(_, j)| j.is_none())) {
            return None;
        }
        let mut d = self.clone();
        d.symmetry = Symmetry::HalfTurn;
        d.brushes = brushes
            .iter()
            .map(|&(i, _)| {
                let mut b = self.brushes[i].clone();
                b.dest = b.dest.map(|g| group[&g].0);
                b
            })
            .collect();
        d.features = features
            .iter()
            .map(|&(i, _)| {
                let mut f = self.features[i].clone();
                f.bit = group[&f.bit].0;
                f
            })
            .collect();
        d.trees = trees.iter().map(|&(i, _)| self.trees[i].clone()).collect();
        d.ladders = ladders.iter().map(|&(i, _)| self.ladders[i].clone()).collect();
        d.lines = lines.iter().map(|&(i, _)| self.lines[i].clone()).collect();
        d.validate().ok()?;
        Some(d)
    }

    /// Which brush each block of the built world comes from, and whether
    /// it is the turned copy, in the order the blocks are built.
    pub fn sources(&self) -> Vec<(usize, bool)> {
        let half = self.symmetry == Symmetry::HalfTurn;
        let mut out = Vec::with_capacity(self.brushes.len() * 2);
        for (j, b) in self.brushes.iter().enumerate() {
            out.push((j, false));
            let (min, max) = (v3(b.min), v3(b.max));
            let (a, c) = (turn(min), turn(max));
            // A block centred on the middle is its own copy.
            if half && ((a.min(c) - min).abs().max_element() > 1e-3 || (a.max(c) - max).abs().max_element() > 1e-3) {
                out.push((j, true));
            }
        }
        out
    }

    /// Build the world this document describes. `seed` is the session's;
    /// nothing in the build depends on it, but the world carries it.
    pub fn build(self: &Arc<Self>, seed: u64, map: u8) -> Result<World, String> {
        self.validate()?;
        let heights = self.heights();
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
            tunnels: Vec::new(),
            clock: 0.0,
            ladders: Vec::new(),
            lines: Vec::new(),
            shades: [0.0; 2],
            doc: Some(self.clone()),
        };
        let half = self.symmetry == Symmetry::HalfTurn;
        let (points, bases) = self.layout();
        for (i, p) in points.iter().enumerate() {
            w.points[i] = vec3(p[0], w.height(p[0], p[1]), p[1]);
        }
        for (i, p) in bases.iter().enumerate() {
            w.bases[i] = vec3(p[0], w.height(p[0], p[1]), p[1]);
        }
        // In half-turn symmetry each authored group g becomes slots 2g
        // (as written) and 2g + 1 (its turned copy).
        let slot = |d: Option<u8>, copy: bool| if half { d.map(|g| g * 2 + copy as u8) } else { d };
        for (j, copy) in self.sources() {
            let b = &self.brushes[j];
            let (mut min, mut max) = (v3(b.min), v3(b.max));
            if copy {
                (min, max) = (turn(max).min(turn(min)), turn(max).max(turn(min)));
            }
            w.blocks.push(Block { min, max, kind: b.kind, tint: b.tint, dest: slot(b.dest, copy) });
        }
        for t in &self.trees {
            let base = v3(t.at);
            w.trees.push(Tree { base, height: t.height, radius: t.radius, tint: t.tint, pine: t.pine });
            if half && (base.x.abs() > 1e-3 || base.z.abs() > 1e-3) {
                w.trees.push(Tree { base: turn(base), height: t.height, radius: t.radius, tint: t.tint, pine: t.pine });
            }
        }
        for f in &self.features {
            w.features.push(Feature { kind: f.kind, bit: slot(Some(f.bit), false).unwrap(), pos: v3(f.at) });
            if half {
                w.features.push(Feature { kind: f.kind, bit: slot(Some(f.bit), true).unwrap(), pos: turn(v3(f.at)) });
            }
        }
        for l in &self.ladders {
            let (at, face) = (v3(l.at), v3(l.face).normalize());
            w.ladders.push(Ladder { at, face, top: l.top });
            if half {
                w.ladders.push(Ladder { at: turn(at), face: turn(face), top: l.top });
            }
        }
        for line in &self.lines {
            let pts: Vec<Vec3> = line.iter().copied().map(v3).collect();
            let turned = pts.iter().copied().map(turn).collect();
            w.lines.push(Seedline::new(pts));
            if half {
                w.lines.push(Seedline::new(turned));
            }
        }
        let top = w.blocks.iter().filter_map(|b| b.dest).chain(w.features.iter().map(|f| f.bit)).max();
        w.n_crates = top.map_or(0, |t| t as usize + 1);
        Ok(w)
    }
}

/// The grid heights of a built-in map's ground, or flat ground.
/// The ground before any change: a built-in map's, or flat. Kept for the
/// last few terrains asked for, since the map maker asks for the same one
/// on every stroke.
fn base_cached(preset: Option<u8>, s: u32) -> Arc<Vec<f32>> {
    type Cache = Vec<((Option<u8>, u32), Arc<Vec<f32>>)>;
    static CACHE: std::sync::Mutex<Cache> = std::sync::Mutex::new(Vec::new());
    let key = (preset, s);
    if let Some((_, h)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(k, _)| *k == key) {
        return h.clone();
    }
    let h = Arc::new(base_heights(preset, s));
    let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    c.retain(|(k, _)| *k != key);
    c.insert(0, (key, h.clone()));
    c.truncate(4);
    h
}

impl MapDoc {
    /// The ground's height at every grid point (`terrain::GX` across, west
    /// to east, then north to south): the preset's plus the delta.
    pub fn heights(&self) -> Vec<f32> {
        let mut h = base_cached(self.terrain.preset, self.terrain.seed).as_ref().clone();
        let mut i = 0;
        for &(count, cm) in &self.terrain.delta {
            let end = (i + count as usize).min(h.len());
            for v in &mut h[i..end] {
                *v += cm as f32 / 100.0;
            }
            i = end;
        }
        h
    }

    /// Make the ground `h` (as [`MapDoc::heights`] gives it), to the
    /// centimetre, within what a document can hold.
    pub fn set_heights(&mut self, h: &[f32]) {
        let base = base_cached(self.terrain.preset, self.terrain.seed);
        let cm: Vec<i16> = base
            .iter()
            .zip(h)
            .map(|(b, v)| {
                let v = v.clamp(Y_RANGE.0 + 1.0, Y_RANGE.1 - 1.0);
                ((v - b) * 100.0).round().clamp(-20_000.0, 20_000.0) as i16
            })
            .collect();
        self.terrain.delta = if cm.iter().all(|&c| c == 0) { Vec::new() } else { runs(&cm) };
    }
}

fn base_heights(preset: Option<u8>, s: u32) -> Vec<f32> {
    let mut out = Vec::with_capacity(CELLS);
    for iz in 0..=TNZ {
        for ix in 0..=TNX {
            let x = TER_X0 + ix as f32 * super::CELL;
            let z = TER_Z0 + iz as f32 * super::CELL;
            out.push(preset.map_or(0.0, |p| full_height(x, z, s, p, &[])));
        }
    }
    out
}

fn lay_out(v: &serde_json::Value, depth: usize, out: &mut String) {
    use serde_json::Value;
    let flat = v.to_string();
    if flat.len() + depth * 2 <= 110 {
        out.push_str(&flat);
        return;
    }
    let pad = "  ".repeat(depth + 1);
    match v {
        Value::Array(items) => {
            out.push('[');
            for (k, item) in items.iter().enumerate() {
                out.push_str(if k == 0 { "\n" } else { ",\n" });
                out.push_str(&pad);
                lay_out(item, depth + 1, out);
            }
            out.push('\n');
            out.push_str(&"  ".repeat(depth));
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            for (k, (key, item)) in map.iter().enumerate() {
                out.push_str(if k == 0 { "\n" } else { ",\n" });
                out.push_str(&pad);
                out.push_str(&Value::String(key.clone()).to_string());
                out.push_str(": ");
                lay_out(item, depth + 1, out);
            }
            out.push('\n');
            out.push_str(&"  ".repeat(depth));
            out.push('}');
        }
        _ => out.push_str(&flat),
    }
}

fn runs(v: &[i16]) -> Vec<(u32, i16)> {
    let mut out: Vec<(u32, i16)> = Vec::new();
    for &c in v {
        match out.last_mut() {
            Some((n, last)) if *last == c => *n += 1,
            _ => out.push((1, c)),
        }
    }
    out
}

/// Custom maps known to this process, by session map id. The embedded
/// server and the client share them, so a map hosted from a document is
/// built the same on both.
static INSTALLED: RwLock<Vec<Installed>> = RwLock::new(Vec::new());

struct Installed {
    id: u8,
    doc: Arc<MapDoc>,
    /// Its name, palette and layout for everything that lists maps. Leaked:
    /// there are never more than 127 in a run.
    def: &'static MapDef,
}

/// Make a document available as a map id (`FIRST_CUSTOM` and up) for
/// [`World::with_map`]. Installing the same document again returns its id.
pub fn install(doc: MapDoc) -> Result<u8, String> {
    doc.validate()?;
    let mut list = INSTALLED.write().unwrap_or_else(|e| e.into_inner());
    if let Some(i) = list.iter().find(|i| *i.doc == doc) {
        return Ok(i.id);
    }
    let id = (FIRST_CUSTOM..255).find(|id| list.iter().all(|i| i.id != *id)).ok_or("too many custom maps")?;
    let (points, bases) = doc.layout();
    let def = Box::leak(Box::new(MapDef {
        name: Box::leak(doc.meta.name.clone().into_boxed_str()),
        blurb: Box::leak(doc.meta.blurb.clone().into_boxed_str()),
        points: points.map(|[x, z]| (x, z)),
        bases: bases.map(|[x, z]| (x, z)),
        ground: doc.look.ground,
    }));
    list.push(Installed { id, doc: Arc::new(doc), def });
    Ok(id)
}

/// The map id the map maker shows the map being edited as. It is never
/// given out by [`install`] or sent to a server.
pub const EDITING: u8 = 255;

/// Show `doc` as map [`EDITING`], replacing what was there.
pub fn preview(doc: Arc<MapDoc>) {
    let mut list = INSTALLED.write().unwrap_or_else(|e| e.into_inner());
    let (points, bases) = doc.layout();
    let (points, bases) = (points.map(|[x, z]| (x, z)), bases.map(|[x, z]| (x, z)));
    let old = list.iter().position(|i| i.id == EDITING).map(|k| list.swap_remove(k));
    // Leak a new name and layout only when they change.
    let def = match old {
        Some(i)
            if i.def.name == doc.meta.name
                && i.def.blurb == doc.meta.blurb
                && i.def.points == points
                && i.def.bases == bases
                && i.def.ground == doc.look.ground =>
        {
            i.def
        }
        _ => Box::leak(Box::new(MapDef {
            name: Box::leak(doc.meta.name.clone().into_boxed_str()),
            blurb: Box::leak(doc.meta.blurb.clone().into_boxed_str()),
            points,
            bases,
            ground: doc.look.ground,
        })),
    };
    list.push(Installed { id: EDITING, doc, def });
}

/// The installed document behind a custom map id.
pub fn installed(id: u8) -> Option<Arc<MapDoc>> {
    INSTALLED.read().unwrap_or_else(|e| e.into_inner()).iter().find(|i| i.id == id).map(|i| i.doc.clone())
}

pub(super) fn def(id: u8) -> Option<&'static MapDef> {
    INSTALLED.read().unwrap_or_else(|e| e.into_inner()).iter().find(|i| i.id == id).map(|i| i.def)
}

/// The built-in map whose sky, sun, fog and foliage map `id` wears: its
/// own, or the mood a custom map chose.
pub fn look_of(id: u8) -> u8 {
    if id >= FIRST_CUSTOM {
        installed(id).map_or(0, |d| d.look.mood)
    } else {
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{tests::fingerprint, Nav};

    #[test]
    fn built_in_maps_round_trip_through_a_document() {
        // Maps whose rules live only in their data: everything a player
        // collides with, walks on or captures must come back identical.
        for map in [0u8, 3, 5] {
            for seed in [0xFE221u64, 0xD091] {
                let w = World::with_map(seed, map);
                let doc = MapDoc::from_world(&w).unwrap();
                let back = Arc::new(MapDoc::from_json(&doc.to_json()).unwrap());
                assert_eq!(*back, doc);
                let b = back.build(seed, map).unwrap();
                assert_eq!(fingerprint(&b), fingerprint(&w), "map {map} seed {seed:x}");
                assert_eq!(b.ladders.len(), w.ladders.len());
                assert_eq!(Nav::build_mask(&b, 0).blocked, Nav::build_mask(&w, 0).blocked, "map {map} nav");
                let wire = MapDoc::from_bytes(&doc.to_bytes()).unwrap();
                assert_eq!(wire, doc);
            }
        }
    }

    #[test]
    fn built_in_maps_fold_to_half_turn() {
        let key = |b: &Block| {
            let q = |v: Vec3| (v * 100.0).round().to_array().map(|x| x as i32);
            (q(b.min), q(b.max), b.kind as u8, b.tint, b.dest.is_some())
        };
        // Relay's great mast stands alone on the north flank: it is not
        // the same under a half turn, so it stays written out.
        let mut relay = MapDoc::from_world(&World::with_map(0xFE221, 5)).unwrap();
        assert!(!relay.fold());
        assert_eq!(relay.symmetry, Symmetry::None);
        for map in [0u8, 3] {
            let w = World::with_map(0xFE221, map);
            let mut doc = MapDoc::from_world(&w).unwrap();
            assert!(doc.fold(), "map {map} did not fold");
            assert_eq!(doc.symmetry, Symmetry::HalfTurn);
            assert!(doc.brushes.len() * 2 >= w.blocks.len());
            let b = Arc::new(MapDoc::from_json(&doc.to_json()).unwrap()).build(1, FIRST_CUSTOM).unwrap();
            let mut x: Vec<_> = w.blocks.iter().map(key).collect();
            let mut y: Vec<_> = b.blocks.iter().map(key).collect();
            x.sort();
            y.sort();
            assert_eq!(x, y, "map {map} blocks");
            assert_eq!(b.trees.len(), w.trees.len());
            assert_eq!(b.ladders.len(), w.ladders.len());
            assert_eq!(b.n_crates, w.n_crates, "map {map} slots");
            assert_eq!(b.points, w.points);
            assert_eq!(Nav::build_mask(&b, 0).blocked, Nav::build_mask(&w, 0).blocked, "map {map} nav");
        }
        // A lone block off the centre has no twin, so nothing folds.
        let mut d = sample();
        d.symmetry = Symmetry::None;
        d.points = [[-108.0, 21.0], [-54.0, -36.0], [0.0, 0.0], [54.0, 36.0], [108.0, -21.0]];
        d.bases[1] = [159.0, 0.0];
        let before = d.clone();
        assert!(!d.fold());
        assert_eq!(d, before);
    }

    fn sample() -> MapDoc {
        let mut d = MapDoc::blank("TEST YARD");
        d.brushes.push(Brush { min: [-60.0, -0.5, 10.0], max: [-54.0, 2.6, 12.5], kind: BlockKind::Container, tint: 1, dest: None });
        d.brushes.push(Brush { min: [-1.0, -0.5, -1.0], max: [1.0, 1.2, 1.0], kind: BlockKind::Concrete, tint: 0, dest: None });
        d.brushes.push(Brush { min: [-40.0, 0.0, -5.0], max: [-39.0, 1.0, -4.0], kind: BlockKind::Crate, tint: 0, dest: Some(3) });
        d.trees.push(TreeDoc { at: [-80.0, 0.0, 40.0], height: 7.0, radius: 2.0, tint: 2, pine: true });
        d.ladders.push(LadderDoc { at: [-30.0, 0.0, 20.0], face: [1.0, 0.0, 0.0], top: 6.0 });
        d.lines.push(vec![[-20.0, 0.0, 0.0], [-20.0, 3.0, 0.0], [-10.0, 6.0, 5.0], [-10.0, 0.0, 5.0]]);
        d
    }

    #[test]
    fn half_turn_documents_build_both_halves() {
        let w = Arc::new(sample()).build(7, FIRST_CUSTOM).unwrap();
        // The centred block is its own copy.
        assert_eq!(w.blocks.len(), 5);
        let c = &w.blocks[1];
        assert_eq!((c.min, c.max), (vec3(54.0, -0.5, -12.5), vec3(60.0, 2.6, -10.0)));
        let crates: Vec<_> = w.blocks.iter().filter_map(|b| b.dest).collect();
        assert_eq!(crates, vec![6, 7]);
        assert_eq!(w.n_crates, 8);
        assert_eq!(w.trees.len(), 2);
        assert_eq!(w.trees[1].base, vec3(80.0, 0.0, -40.0));
        assert_eq!(w.ladders[1].face, vec3(-1.0, 0.0, 0.0));
        assert_eq!(w.lines.len(), 2);
        assert_eq!(w.lines[1].pts[2], vec3(10.0, 6.0, -5.0));
        assert_eq!(w.points[4], vec3(108.0, 0.0, -21.0));
        assert_eq!(w.bases[1], vec3(159.0, 0.0, 0.0));
        // Flat ground everywhere, and the nav grid builds.
        assert!(w.heights.iter().all(|&h| h == 0.0));
        let nav = Nav::build_mask(&w, 0);
        assert!(nav.blocked.iter().any(|&b| b));
    }

    #[test]
    fn terrain_edits_apply() {
        let mut d = sample();
        let mut cm = vec![0i16; CELLS];
        cm[5] = 150;
        cm[CELLS - 1] = -20;
        d.terrain.delta = runs(&cm);
        let w = Arc::new(MapDoc::from_json(&d.to_json()).unwrap()).build(1, FIRST_CUSTOM).unwrap();
        assert_eq!(w.heights[5], 1.5);
        assert_eq!(w.heights[CELLS - 1], -0.2);
        assert_eq!(w.heights[4], 0.0);
    }

    #[test]
    fn bad_documents_are_refused() {
        type Spoil = Box<dyn Fn(&mut MapDoc)>;
        let bad: Vec<(&str, Spoil)> = vec![
            ("nan", Box::new(|d| d.brushes[0].min[0] = f32::NAN)),
            ("outside", Box::new(|d| d.brushes[0].max[2] = 500.0)),
            ("no size", Box::new(|d| d.brushes[0].max[1] = d.brushes[0].min[1])),
            ("dest", Box::new(|d| d.brushes[2].dest = Some(40))),
            ("point", Box::new(|d| d.points[0][0] = 300.0)),
            ("name", Box::new(|d| d.meta.name = String::new())),
            ("long name", Box::new(|d| d.meta.name = "X".repeat(40))),
            ("control chars", Box::new(|d| d.meta.blurb = "a\u{7}b".into())),
            ("delta length", Box::new(|d| d.terrain.delta = vec![(10, 5)])),
            ("delta size", Box::new(|d| d.terrain.delta = vec![(CELLS as u32, 30_000)])),
            ("preset", Box::new(|d| d.terrain.preset = Some(99))),
            ("ladder", Box::new(|d| d.ladders[0].top = -1.0)),
            ("line", Box::new(|d| d.lines[0].truncate(1))),
            ("tree", Box::new(|d| d.trees[0].height = 500.0)),
            ("too many", Box::new(|d| d.brushes = vec![d.brushes[0].clone(); MAX_BLOCKS])),
            ("format", Box::new(|d| d.format = FORMAT + 1)),
        ];
        for (what, f) in bad {
            let mut d = sample();
            f(&mut d);
            assert!(MapDoc::from_json(&d.to_json()).is_err(), "{what} was accepted from JSON");
            if what != "nan" {
                assert!(MapDoc::from_bytes(&d.to_bytes()).is_err(), "{what} was accepted from bytes");
            }
            assert!(install(d).is_err(), "{what} was installed");
        }
    }

    #[test]
    fn damaged_data_never_panics() {
        let good = sample().to_bytes();
        let json = sample().to_json().into_bytes();
        let mut r = crate::rng::Rng::new(0xF0F0);
        for _ in 0..3000 {
            for src in [&good, &json] {
                let mut b = src.clone();
                for _ in 0..1 + r.below(6) {
                    let i = r.below(b.len() as u32) as usize;
                    match r.below(3) {
                        0 => b[i] = r.below(256) as u8,
                        1 => b.truncate(i),
                        _ => b.insert(i, r.below(256) as u8),
                    }
                    if b.is_empty() {
                        break;
                    }
                }
                if let Ok(d) = MapDoc::from_bytes(&b) {
                    Arc::new(d).build(1, FIRST_CUSTOM).unwrap();
                }
                if let Ok(d) = MapDoc::from_json(&String::from_utf8_lossy(&b)) {
                    Arc::new(d).build(1, FIRST_CUSTOM).unwrap();
                }
            }
        }
        // Huge lengths up front must not allocate their way to a crash.
        let mut b = vec![0xFF; 64];
        b[..2].copy_from_slice(&FORMAT.to_le_bytes());
        assert!(MapDoc::from_bytes(&b).is_err());
    }

    #[test]
    fn installed_maps_build_through_with_map() {
        let id = install(sample()).unwrap();
        assert!(id >= FIRST_CUSTOM);
        assert_eq!(install(sample()).unwrap(), id);
        let w = World::with_map(5, id);
        assert_eq!(w.map, id);
        assert_eq!(w.blocks.len(), 5);
        assert_eq!(w.doc.as_ref().unwrap().meta.name, "TEST YARD");
        assert_eq!(super::super::map_def(id).name, "TEST YARD");
        assert_eq!(super::super::map_def(id).points[4], (108.0, -21.0));
        assert_eq!(look_of(id), 0);
    }
}
