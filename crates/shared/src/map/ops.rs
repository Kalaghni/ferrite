//! Edits to a map document as data: what the map maker's tools, its MCP
//! server and any script all speak.
//!
//! A batch of [`Op`]s applies in order and lands whole or not at all: the
//! document is validated once at the end, and a batch that leaves an
//! invalid map changes nothing. Brush ids are indices into
//! `MapDoc::brushes` as they stand when that op runs.
//!
//! Coordinates are metres: x runs west to east (-177 to 177), z north to
//! south (-102 to 102), y is up. `at` is a spot on the ground `[x, z]`.

use super::doc::{Brush, MapDoc, Symmetry, TreeDoc};
use super::terrain::{self, Stroke};
use super::BlockKind;
use serde::{Deserialize, Serialize};

/// The loopback port the map maker listens on for tools (see the client's
/// `forge_link`).
pub const BRIDGE_PORT: u16 = 47817;

/// A group of blocks kept as one asset: a bunker, a tower, a wall run.
/// Offsets are from the prefab's origin, with y measured from the ground.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Prefab {
    pub name: String,
    #[serde(default)]
    pub about: String,
    pub blocks: Vec<PrefabBlock>,
    #[serde(default)]
    pub trees: Vec<PrefabTree>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PrefabBlock {
    pub kind: BlockKind,
    pub min: [f32; 3],
    pub max: [f32; 3],
    #[serde(default)]
    pub tint: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PrefabTree {
    pub at: [f32; 2],
    pub height: f32,
    pub radius: f32,
    #[serde(default)]
    pub pine: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    /// Shape the ground with a round brush (see `terrain`). Blocks footed
    /// in the ground and trees on it move with it.
    Sculpt(Stroke),
    /// A block standing on the ground at `at`, `size` = [x, y, z]. Its
    /// foot is sunk to the lowest ground under it, as the built maps do;
    /// `lift` raises it off the ground (onto a roof, say).
    Place {
        kind: BlockKind,
        at: [f32; 2],
        size: [f32; 3],
        #[serde(default)]
        tint: u8,
        #[serde(default)]
        lift: f32,
    },
    /// A block by its corners, exactly.
    Add {
        kind: BlockKind,
        min: [f32; 3],
        max: [f32; 3],
        #[serde(default)]
        tint: u8,
    },
    Move {
        ids: Vec<usize>,
        by: [f32; 3],
    },
    /// New corners for a block.
    Reshape {
        id: usize,
        min: [f32; 3],
        max: [f32; 3],
    },
    /// Change what a block is made of, or its tint.
    Restyle {
        ids: Vec<usize>,
        #[serde(default)]
        kind: Option<BlockKind>,
        #[serde(default)]
        tint: Option<u8>,
    },
    /// Quarter turns (clockwise seen from above) about the blocks' joint
    /// centre, or about `about`.
    Turn {
        ids: Vec<usize>,
        #[serde(default = "one")]
        quarters: u8,
        #[serde(default)]
        about: Option<[f32; 2]>,
    },
    /// Copies of blocks, moved by `by`. The copies are added at the end.
    Copy {
        ids: Vec<usize>,
        by: [f32; 3],
    },
    Remove {
        ids: Vec<usize>,
    },
    /// Remove every block whose footprint centre is in the rectangle.
    Clear {
        from: [f32; 2],
        to: [f32; 2],
        #[serde(default)]
        kind: Option<BlockKind>,
    },
    /// Stamp a prefab with its origin at `at`. A prefab named in a tool
    /// call is resolved to its contents before it gets here.
    Prefab {
        prefab: Prefab,
        at: [f32; 2],
        #[serde(default)]
        quarters: u8,
        #[serde(default)]
        lift: f32,
    },
    Tree {
        at: [f32; 2],
        height: f32,
        radius: f32,
        #[serde(default)]
        pine: bool,
    },
    /// Move objective `point` (0 = A ... 4 = E). In half-turn symmetry
    /// A and B are the originals and D and E their copies; moving a copy
    /// moves its original. C stays at the centre.
    Objective {
        point: usize,
        at: [f32; 2],
    },
    /// Move base `team` (0 west, 1 east), the same way.
    Base {
        team: usize,
        at: [f32; 2],
    },
    Rename {
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        blurb: Option<String>,
    },
    /// Borrow a built-in map's sky, light and foliage, or set the ground
    /// palette (grass, dry patches, packed dirt; each RGB 0 to 1).
    Look {
        #[serde(default)]
        mood: Option<u8>,
        #[serde(default)]
        ground: Option<[[f32; 3]; 3]>,
    },
}

fn one() -> u8 {
    1
}

/// What a batch did.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    /// Ids of the blocks it added, in the document after the batch.
    pub added: Vec<usize>,
    pub removed: usize,
    pub changed: usize,
    pub blocks: usize,
    /// Ground grid points (2 m apart) that sculpting moved.
    #[serde(default)]
    pub ground: usize,
}

/// A clockwise quarter turn of an offset, seen from above (y up, z south).
fn quarter(x: f32, z: f32, n: u8) -> (f32, f32) {
    match n % 4 {
        0 => (x, z),
        1 => (-z, x),
        2 => (-x, -z),
        _ => (z, -x),
    }
}

fn turn_box(min: [f32; 3], max: [f32; 3], n: u8, cx: f32, cz: f32) -> ([f32; 3], [f32; 3]) {
    let (ax, az) = quarter(min[0] - cx, min[2] - cz, n);
    let (bx, bz) = quarter(max[0] - cx, max[2] - cz, n);
    ([cx + ax.min(bx), min[1], cz + az.min(bz)], [cx + ax.max(bx), max[1], cz + az.max(bz)])
}

/// The lowest ground under a footprint, and the ground at its middle.
fn footing(ground: &dyn Fn(f32, f32) -> f32, x0: f32, z0: f32, x1: f32, z1: f32) -> (f32, f32) {
    let mid = ground((x0 + x1) * 0.5, (z0 + z1) * 0.5);
    let low = [(x0, z0), (x1, z0), (x0, z1), (x1, z1)].iter().map(|&(x, z)| ground(x, z)).fold(mid, f32::min);
    (low, mid)
}

fn check_ids(doc: &MapDoc, ids: &[usize]) -> Result<(), String> {
    match ids.iter().find(|&&i| i >= doc.brushes.len()) {
        Some(i) => Err(format!("no block {i}: the map has {}", doc.brushes.len())),
        None => Ok(()),
    }
}

/// Apply `ops` in order. `ground(x, z)` is the terrain height, for ops
/// that stand things on it. On any error the document is left as it was.
pub fn apply(doc: &mut MapDoc, ops: &[Op], ground: &dyn Fn(f32, f32) -> f32) -> Result<Outcome, String> {
    let mut d = doc.clone();
    let mut out = Outcome::default();
    // Added ids shift when blocks before them are removed: track them.
    let mut added: Vec<usize> = Vec::new();
    let half = d.symmetry == Symmetry::HalfTurn;
    // Once the batch has sculpted, later ops stand things on the new ground.
    let mut grid: Option<Vec<f32>> = None;
    let outer = ground;
    for (n, op) in ops.iter().enumerate() {
        let fail = |e: String| format!("op {n}: {e}");
        let mut sculpted = None;
        let reshaped;
        let ground: &dyn Fn(f32, f32) -> f32 = match &grid {
            Some(h) => {
                reshaped = move |x: f32, z: f32| terrain::sample(h, x, z);
                &reshaped
            }
            None => outer,
        };
        match op {
            Op::Sculpt(s) => {
                let old = grid.clone().unwrap_or_else(|| d.heights());
                let mut new = old.clone();
                out.ground += terrain::stroke(s, &mut new, half).map_err(fail)?;
                d.set_heights(&new);
                // As stored: to the centimetre.
                let new = d.heights();
                out.changed += terrain::reseat(&mut d, &old, &new).0.len();
                sculpted = Some(new);
            }
            Op::Place { kind, at, size, tint, lift } => {
                if size.iter().any(|s| !s.is_finite() || *s <= 0.0) {
                    return Err(fail("size must be positive".into()));
                }
                let (x0, z0, x1, z1) = (at[0] - size[0] * 0.5, at[1] - size[2] * 0.5, at[0] + size[0] * 0.5, at[1] + size[2] * 0.5);
                let (low, mid) = footing(ground, x0, z0, x1, z1);
                let (y0, y1) = if *lift > 0.0 { (mid + lift, mid + lift + size[1]) } else { (low - 0.4, mid + size[1]) };
                added.push(d.brushes.len());
                d.brushes.push(Brush { min: [x0, y0, z0], max: [x1, y1, z1], kind: *kind, tint: *tint, dest: None });
            }
            Op::Add { kind, min, max, tint } => {
                added.push(d.brushes.len());
                d.brushes.push(Brush { min: *min, max: *max, kind: *kind, tint: *tint, dest: None });
            }
            Op::Move { ids, by } => {
                check_ids(&d, ids).map_err(fail)?;
                for &i in ids {
                    let b = &mut d.brushes[i];
                    for a in 0..3 {
                        b.min[a] += by[a];
                        b.max[a] += by[a];
                    }
                }
                out.changed += ids.len();
            }
            Op::Reshape { id, min, max } => {
                check_ids(&d, &[*id]).map_err(fail)?;
                let b = &mut d.brushes[*id];
                (b.min, b.max) = (*min, *max);
                out.changed += 1;
            }
            Op::Restyle { ids, kind, tint } => {
                check_ids(&d, ids).map_err(fail)?;
                for &i in ids {
                    let b = &mut d.brushes[i];
                    if let Some(k) = kind {
                        b.kind = *k;
                    }
                    if let Some(t) = tint {
                        b.tint = t % 4;
                    }
                }
                out.changed += ids.len();
            }
            Op::Turn { ids, quarters, about } => {
                check_ids(&d, ids).map_err(fail)?;
                if ids.is_empty() {
                    continue;
                }
                let (cx, cz) = match about {
                    Some(p) => (p[0], p[1]),
                    None => {
                        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
                        for &i in ids {
                            let b = &d.brushes[i];
                            lo = [lo[0].min(b.min[0]), lo[1].min(b.min[2])];
                            hi = [hi[0].max(b.max[0]), hi[1].max(b.max[2])];
                        }
                        ((lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5)
                    }
                };
                for &i in ids {
                    let b = &mut d.brushes[i];
                    (b.min, b.max) = turn_box(b.min, b.max, *quarters, cx, cz);
                }
                out.changed += ids.len();
            }
            Op::Copy { ids, by } => {
                check_ids(&d, ids).map_err(fail)?;
                for &i in ids {
                    let mut b = d.brushes[i].clone();
                    for a in 0..3 {
                        b.min[a] += by[a];
                        b.max[a] += by[a];
                    }
                    // A copy is its own thing, not part of the original's
                    // destruction group.
                    b.dest = None;
                    added.push(d.brushes.len());
                    d.brushes.push(b);
                }
            }
            Op::Remove { ids } => {
                check_ids(&d, ids).map_err(fail)?;
                let mut ids = ids.clone();
                ids.sort_unstable();
                ids.dedup();
                for &i in ids.iter().rev() {
                    d.brushes.remove(i);
                    added.retain(|&a| a != i);
                    for a in &mut added {
                        if *a > i {
                            *a -= 1;
                        }
                    }
                }
                out.removed += ids.len();
            }
            Op::Clear { from, to, kind } => {
                let (x0, x1) = (from[0].min(to[0]), from[0].max(to[0]));
                let (z0, z1) = (from[1].min(to[1]), from[1].max(to[1]));
                let ids: Vec<usize> = d
                    .brushes
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| {
                        let (cx, cz) = ((b.min[0] + b.max[0]) * 0.5, (b.min[2] + b.max[2]) * 0.5);
                        (x0..=x1).contains(&cx) && (z0..=z1).contains(&cz) && kind.is_none_or(|k| b.kind == k)
                    })
                    .map(|(i, _)| i)
                    .collect();
                for &i in ids.iter().rev() {
                    d.brushes.remove(i);
                    added.retain(|&a| a != i);
                    for a in &mut added {
                        if *a > i {
                            *a -= 1;
                        }
                    }
                }
                out.removed += ids.len();
            }
            Op::Prefab { prefab, at, quarters, lift } => {
                let (blocks, trees) = stamp(prefab, *at, *quarters, *lift, ground);
                for b in blocks {
                    added.push(d.brushes.len());
                    d.brushes.push(b);
                }
                d.trees.extend(trees);
            }
            Op::Tree { at, height, radius, pine } => {
                d.trees.push(TreeDoc { at: [at[0], ground(at[0], at[1]), at[1]], height: *height, radius: *radius, tint: 0, pine: *pine });
            }
            Op::Objective { point, at } => {
                let k = match (*point, half) {
                    (p, _) if p > 4 => return Err(fail("objectives are 0 (A) to 4 (E)".into())),
                    (2, true) => return Err(fail("C is the centre in a half-turn map".into())),
                    (p, true) if p > 2 => 4 - p,
                    (p, _) => p,
                };
                let flip = k != *point;
                d.points[k] = if flip { [-at[0], -at[1]] } else { *at };
                out.changed += 1;
            }
            Op::Base { team, at } => {
                let k = match (*team, half) {
                    (t, _) if t > 1 => return Err(fail("bases are 0 (west) and 1 (east)".into())),
                    (_, true) => 0,
                    (t, false) => t,
                };
                d.bases[k] = if k != *team { [-at[0], -at[1]] } else { *at };
                out.changed += 1;
            }
            Op::Rename { name, blurb } => {
                if let Some(s) = name {
                    d.meta.name = s.to_uppercase();
                }
                if let Some(s) = blurb {
                    d.meta.blurb = s.clone();
                }
            }
            Op::Look { mood, ground } => {
                if let Some(m) = mood {
                    d.look.mood = *m;
                }
                if let Some(g) = ground {
                    d.look.ground = *g;
                }
            }
        }
        if sculpted.is_some() {
            grid = sculpted;
        }
    }
    d.validate()?;
    out.added = added;
    out.blocks = d.brushes.len();
    *doc = d;
    Ok(out)
}

/// The blocks and trees a prefab stamps with its origin at `at`, turned
/// `quarters` clockwise and raised `lift` off the ground. Blocks that
/// start at the ground reach down to it wherever it falls away under them.
pub fn stamp(prefab: &Prefab, at: [f32; 2], quarters: u8, lift: f32, ground: &dyn Fn(f32, f32) -> f32) -> (Vec<Brush>, Vec<TreeDoc>) {
    let g0 = ground(at[0], at[1]) + lift;
    let blocks = prefab
        .blocks
        .iter()
        .map(|p| {
            let (min, max) = turn_box(p.min, p.max, quarters, 0.0, 0.0);
            let (x0, z0, x1, z1) = (at[0] + min[0], at[1] + min[2], at[0] + max[0], at[1] + max[2]);
            let mut y0 = g0 + min[1];
            if min[1] <= 0.01 && lift <= 0.0 {
                y0 = y0.min(footing(ground, x0, z0, x1, z1).0 - 0.4);
            }
            Brush { min: [x0, y0, z0], max: [x1, g0 + max[1], z1], kind: p.kind, tint: p.tint, dest: None }
        })
        .collect();
    let trees = prefab
        .trees
        .iter()
        .map(|t| {
            let (x, z) = quarter(t.at[0], t.at[1], quarters);
            let (x, z) = (at[0] + x, at[1] + z);
            TreeDoc { at: [x, ground(x, z) + lift.max(0.0), z], height: t.height, radius: t.radius, tint: 0, pine: t.pine }
        })
        .collect();
    (blocks, trees)
}

/// The blocks `ids` as a prefab about their joint footprint centre, with
/// heights from the ground there.
pub fn capture(doc: &MapDoc, ids: &[usize], name: &str, ground: &dyn Fn(f32, f32) -> f32) -> Result<Prefab, String> {
    check_ids(doc, ids)?;
    if ids.is_empty() {
        return Err("no blocks to keep".into());
    }
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for &i in ids {
        let b = &doc.brushes[i];
        lo = [lo[0].min(b.min[0]), lo[1].min(b.min[2])];
        hi = [hi[0].max(b.max[0]), hi[1].max(b.max[2])];
    }
    let (cx, cz) = (((lo[0] + hi[0]) * 0.5).round(), ((lo[1] + hi[1]) * 0.5).round());
    let g = ground(cx, cz);
    let blocks = ids
        .iter()
        .map(|&i| {
            let b = &doc.brushes[i];
            // A foot sunk into the ground is kept at ground level: placing
            // it sinks it again to suit wherever it lands.
            let y0 = if b.min[1] < g + 0.05 { 0.0 } else { b.min[1] - g };
            PrefabBlock {
                kind: b.kind,
                min: [b.min[0] - cx, y0, b.min[2] - cz],
                max: [b.max[0] - cx, b.max[1] - g, b.max[2] - cz],
                tint: b.tint,
            }
        })
        .collect();
    Ok(Prefab { name: name.to_string(), about: String::new(), blocks, trees: Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(_: f32, _: f32) -> f32 {
        0.0
    }

    #[test]
    fn batches_land_whole_or_not_at_all() {
        let mut d = MapDoc::blank("TEST");
        let place = Op::Place { kind: BlockKind::Concrete, at: [-100.0, 10.0], size: [6.0, 3.0, 0.6], tint: 0, lift: 0.0 };
        let out = apply(&mut d, &[place.clone(), place.clone()], &flat).unwrap();
        assert_eq!(out.added, vec![0, 1]);
        assert_eq!(d.brushes[0].min, [-103.0, -0.4, 9.7]);
        assert_eq!(d.brushes[0].max, [-97.0, 3.0, 10.3]);
        // A bad op later in the batch undoes the good ones before it.
        let before = d.clone();
        let bad = Op::Move { ids: vec![0], by: [0.0, 0.0, 0.0] };
        let err = apply(&mut d, &[place, bad, Op::Remove { ids: vec![7] }], &flat).unwrap_err();
        assert!(err.starts_with("op 2"), "{err}");
        assert_eq!(d, before);
        // Leaving the map is refused at the end.
        assert!(apply(&mut d, &[Op::Move { ids: vec![0], by: [500.0, 0.0, 0.0] }], &flat).is_err());
        assert_eq!(d, before);
    }

    #[test]
    fn turns_copies_and_removals_keep_ids_straight() {
        let mut d = MapDoc::blank("TEST");
        let add = |x: f32| Op::Add { kind: BlockKind::Crate, min: [x, 0.0, 0.0], max: [x + 4.0, 1.0, 1.0], tint: 0 };
        apply(&mut d, &[add(-120.0), add(-110.0)], &flat).unwrap();
        let out = apply(&mut d, &[Op::Copy { ids: vec![1], by: [0.0, 0.0, 5.0] }, Op::Remove { ids: vec![0] }], &flat).unwrap();
        assert_eq!(out.added, vec![1]);
        assert_eq!(d.brushes[1].min, [-110.0, 0.0, 5.0]);
        apply(&mut d, &[Op::Turn { ids: vec![0], quarters: 1, about: None }], &flat).unwrap();
        let b = &d.brushes[0];
        assert_eq!((b.max[0] - b.min[0], b.max[2] - b.min[2]), (1.0, 4.0));
        assert_eq!(((b.min[0] + b.max[0]) * 0.5, (b.min[2] + b.max[2]) * 0.5), (-108.0, 0.5));
    }

    #[test]
    fn prefabs_round_trip_through_capture() {
        let mut d = MapDoc::blank("TEST");
        let ops = [
            Op::Place { kind: BlockKind::Concrete, at: [-90.0, 20.0], size: [6.0, 3.0, 0.6], tint: 1, lift: 0.0 },
            Op::Place { kind: BlockKind::Sandbag, at: [-90.0, 22.0], size: [4.0, 1.1, 0.9], tint: 0, lift: 0.0 },
        ];
        apply(&mut d, &ops, &flat).unwrap();
        let p = capture(&d, &[0, 1], "WALL", &flat).unwrap();
        apply(&mut d, &[Op::Prefab { prefab: p, at: [-60.0, 0.0], quarters: 2, lift: 0.0 }], &flat).unwrap();
        assert_eq!(d.brushes.len(), 4);
        // Turned half round: the sandbags are now north of the wall.
        assert!(d.brushes[3].min[2] < d.brushes[2].min[2]);
        assert_eq!(d.brushes[2].min[1], -0.4);
    }

    #[test]
    fn half_turn_objectives_move_their_originals() {
        let mut d = MapDoc::blank("TEST");
        apply(&mut d, &[Op::Objective { point: 3, at: [60.0, 30.0] }, Op::Base { team: 1, at: [150.0, 5.0] }], &flat).unwrap();
        assert_eq!(d.points[1], [-60.0, -30.0]);
        assert_eq!(d.bases[0], [-150.0, -5.0]);
        assert!(apply(&mut d, &[Op::Objective { point: 2, at: [5.0, 5.0] }], &flat).is_err());
    }
}
