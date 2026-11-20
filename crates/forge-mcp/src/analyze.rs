//! Reading a map the way the game does: routes on the bots' nav grid,
//! sight lines, cover, mistakes in the blocks, and a bot match run
//! headless to see where the fighting goes.

use ferrite_server::{Config, Game};
use ferrite_shared::map::doc::{self, MapDoc, Symmetry};
use ferrite_shared::map::terrain::{self, GX, GZ};
use ferrite_shared::map::{BlockKind, Nav, World, BASE_SPAWN_RADIUS, CELL, HALF_X, HALF_Z, MAPS};
use ferrite_shared::proto::Feed;
use ferrite_shared::sim::EYE;
use ferrite_shared::{mode_def, vec3, Vec3, TICK_HZ};
use std::fmt::Write;
use std::sync::Arc;

pub const SEED: u64 = 0xFE221;
const LETTERS: [char; 5] = ['A', 'B', 'C', 'D', 'E'];

/// The world a document builds, shown as the map being edited.
pub fn world_of(d: &MapDoc) -> Result<World, String> {
    let a = Arc::new(d.clone());
    doc::preview(a.clone());
    a.build(SEED, doc::EDITING)
}

fn kind_name(k: BlockKind) -> &'static str {
    match k {
        BlockKind::Concrete => "concrete",
        BlockKind::Container => "container",
        BlockKind::Rock => "rock",
        BlockKind::Crate => "crate",
        BlockKind::Sandbag => "sandbag",
        BlockKind::Metal => "metal",
        BlockKind::Trunk => "trunk",
        BlockKind::Step => "step",
    }
}

fn near_point(w: &World, x: f32, z: f32, r: f32) -> String {
    let mut best = (r, None);
    for (k, p) in w.points.iter().enumerate() {
        let d = ((p.x - x).powi(2) + (p.z - z).powi(2)).sqrt();
        if d < best.0 {
            best = (d, Some(k));
        }
    }
    match best.1 {
        Some(k) => format!(" near {}", LETTERS[k]),
        None => String::new(),
    }
}

/// The document in words: layout, counts, then blocks with their ids.
pub fn describe(d: &MapDoc, w: &World, region: Option<[f32; 4]>, kind: Option<BlockKind>, limit: usize) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "{}{}", d.meta.name, if d.meta.blurb.is_empty() { String::new() } else { format!(" - {}", d.meta.blurb) });
    match d.symmetry {
        Symmetry::HalfTurn => {
            let _ = writeln!(s, "symmetry: half_turn. Every block, tree and ladder is copied turned half round the centre (x,z) -> (-x,-z). Build one half (the west is usual); objectives A, B and the west base are the originals of E, D and the east base.");
        }
        Symmetry::None => {
            let _ = writeln!(s, "symmetry: none. Everything is written out; keep the halves fair by hand.");
        }
    }
    let mood = MAPS.get(d.look.mood as usize).map(|m| m.name).unwrap_or("?");
    let terrain = match d.terrain.preset {
        Some(p) => format!("the ground of {}", MAPS.get(p as usize).map(|m| m.name).unwrap_or("?")),
        None => "flat ground".into(),
    };
    let _ = writeln!(
        s,
        "look: sky and light of {mood} (mood {}); {terrain}{}",
        d.look.mood,
        if d.terrain.delta.is_empty() { "" } else { ", reshaped" }
    );
    let pts: Vec<String> =
        w.points.iter().enumerate().map(|(k, p)| format!("{} ({:.0}, {:.0}) y {:.1}", LETTERS[k], p.x, p.z, p.y)).collect();
    let _ = writeln!(s, "objectives: {}", pts.join(" | "));
    let _ = writeln!(
        s,
        "bases: west ({:.0}, {:.0}), east ({:.0}, {:.0}); spawn ring {BASE_SPAWN_RADIUS} m",
        w.bases[0].x, w.bases[0].z, w.bases[1].x, w.bases[1].z
    );
    let mut kinds = std::collections::BTreeMap::new();
    for b in &d.brushes {
        *kinds.entry(kind_name(b.kind)).or_insert(0) += 1;
    }
    let by: Vec<String> = kinds.iter().map(|(k, n)| format!("{k} {n}")).collect();
    let _ = writeln!(
        s,
        "blocks: {} in the document, {} built of {} allowed ({}); trees {}; ladders {}",
        d.brushes.len(),
        w.blocks.len(),
        doc::MAX_BLOCKS,
        if by.is_empty() { "none".into() } else { by.join(", ") },
        w.trees.len(),
        w.ladders.len()
    );
    let mut shown = 0;
    let mut skipped = 0;
    let _ = writeln!(s, "\nid  kind  tint  min [x y z] -> max [x y z]  size x*y*z (y is height above its ground)");
    for (i, b) in d.brushes.iter().enumerate() {
        let (cx, cz) = ((b.min[0] + b.max[0]) * 0.5, (b.min[2] + b.max[2]) * 0.5);
        if region.is_some_and(|r| cx < r[0].min(r[2]) || cx > r[0].max(r[2]) || cz < r[1].min(r[3]) || cz > r[1].max(r[3])) {
            continue;
        }
        if kind.is_some_and(|k| k != b.kind) {
            continue;
        }
        if shown >= limit {
            skipped += 1;
            continue;
        }
        shown += 1;
        let g = w.height(cx, cz);
        let _ = writeln!(
            s,
            "{i}  {}  {}  [{:.2} {:.2} {:.2}] -> [{:.2} {:.2} {:.2}]  {:.2}*{:.2}*{:.2}{}",
            kind_name(b.kind),
            b.tint,
            b.min[0],
            b.min[1],
            b.min[2],
            b.max[0],
            b.max[1],
            b.max[2],
            b.max[0] - b.min[0],
            b.max[1] - g,
            b.max[2] - b.min[2],
            near_point(w, cx, cz, 25.0)
        );
    }
    if shown == 0 {
        let _ = writeln!(s, "(no blocks match)");
    }
    if skipped > 0 {
        let _ = writeln!(s, "... and {skipped} more (narrow with region or kind, or raise limit)");
    }
    s
}

/// Eye-height sight distance from `from` along `dir`, to 150 m.
fn sight(w: &World, from: Vec3, dir: Vec3) -> f32 {
    w.raycast(from, dir, 150.0, 0).map(|h| h.t).unwrap_or(150.0)
}

fn eye_at(w: &World, x: f32, z: f32) -> Vec3 {
    vec3(x, w.walk_height(x, z) + EYE, z)
}

/// The map doctor: what is wrong or lopsided, and what the ground is like
/// at each objective.
pub fn analyze(d: &MapDoc, w: &World) -> String {
    let mut s = String::new();
    let mut warn: Vec<String> = Vec::new();
    let nav = Nav::build(w);
    let sources = d.sources();
    let brush_of = |i: usize| sources.get(i).map(|s| s.0).unwrap_or(i);

    // Routes from each base to each objective along the bots' lanes.
    let _ = writeln!(s, "ROUTES (walked on the bots' nav grid, three lanes each; metres)");
    let _ = writeln!(s, "point  from west  from east  difference");
    for goal in 0..5 {
        let mut len = [0.0f32; 2];
        for base in 0..2 {
            let mut total = 0.0;
            let mut ok = 0;
            for lane in 0..3 {
                let (mut px, mut pz) = (w.bases[base].x, w.bases[base].z);
                let target = w.points[goal];
                let mut walked = 0.0;
                let mut reached = false;
                for _ in 0..1600 {
                    if ((px - target.x).powi(2) + (pz - target.z).powi(2)).sqrt() < 6.5 {
                        reached = true;
                        break;
                    }
                    let dv = nav.dir(goal, lane, px, pz);
                    px += dv.x * 0.6;
                    pz += dv.y * 0.6;
                    walked += 0.6;
                }
                if reached {
                    total += walked;
                    ok += 1;
                } else {
                    warn.push(format!(
                        "No route from the {} base to {} on lane {lane}: stuck at ({px:.0}, {pz:.0}). Something walls it off.",
                        ["west", "east"][base],
                        LETTERS[goal]
                    ));
                }
            }
            len[base] = if ok > 0 { total / ok as f32 } else { f32::NAN };
        }
        let gap = (len[0] - len[1]).abs() / len[0].max(len[1]) * 100.0;
        let _ = writeln!(s, "{}      {:>6.0}     {:>6.0}     {:>4.0}%", LETTERS[goal], len[0], len[1], gap);
    }
    // Fairness. A half-turn document is fair by construction; a written
    // out one is checked block by block for its turned twin. (Route
    // lengths above are for reading only: the bots' lanes are not mirror
    // exact, so they differ a little even on a fair map.)
    if d.symmetry == Symmetry::None {
        let twin = |b: &ferrite_shared::map::Block| {
            let (min, max) = (vec3(-b.max.x, b.min.y, -b.max.z), vec3(-b.min.x, b.max.y, -b.min.z));
            w.blocks.iter().any(|o| o.kind == b.kind && (o.min - min).abs().max_element() < 0.3 && (o.max - max).abs().max_element() < 0.3)
        };
        let mut lone: Vec<usize> = (0..w.blocks.len()).filter(|&i| !twin(&w.blocks[i])).map(brush_of).collect();
        if !lone.is_empty() {
            let n = lone.len();
            lone.truncate(20);
            let list: Vec<String> = lone.iter().map(|i| i.to_string()).collect();
            warn.push(format!(
                "{n} blocks have no twin on the other side (turned half round the centre), so the sides differ: {}.",
                list.join(", ")
            ));
        }
        let pts = (0..5).all(|k| (w.points[k].x + w.points[4 - k].x).abs() < 1.0 && (w.points[k].z + w.points[4 - k].z).abs() < 1.0);
        let bases = (w.bases[0].x + w.bases[1].x).abs() < 1.0 && (w.bases[0].z + w.bases[1].z).abs() < 1.0;
        if !pts || !bases {
            warn.push("The objectives or bases are not mirrored about the centre: one side has the shorter walk.".into());
        }
    }

    // Each objective: cover close by, and how open it is.
    let _ = writeln!(s, "\nOBJECTIVES (cover within 20 m; sight lines at eye height in 72 directions)");
    let _ = writeln!(s, "point  low cover  full cover  mean sight  open past 80 m  longest");
    for (k, p) in w.points.iter().enumerate() {
        let (mut low, mut full) = (0, 0);
        for b in &w.blocks {
            let (cx, cz) = ((b.min.x + b.max.x) * 0.5, (b.min.z + b.max.z) * 0.5);
            if ((cx - p.x).powi(2) + (cz - p.z).powi(2)).sqrt() > 20.0 {
                continue;
            }
            let h = b.max.y - w.walk_height(cx, cz).min(b.max.y);
            if h >= 1.7 {
                full += 1;
            } else if h >= 0.8 {
                low += 1;
            }
        }
        let eye = eye_at(w, p.x, p.z);
        let mut sum = 0.0;
        let mut open = 0;
        let mut longest = (0.0f32, 0.0f32);
        for i in 0..72 {
            let a = i as f32 / 72.0 * std::f32::consts::TAU;
            let t = sight(w, eye, vec3(a.cos(), 0.0, a.sin()));
            sum += t;
            if t > 80.0 {
                open += 1;
            }
            if t > longest.0 {
                longest = (t, a.to_degrees());
            }
        }
        let open_pc = open as f32 / 72.0 * 100.0;
        let _ = writeln!(
            s,
            "{}      {:>4}       {:>4}        {:>5.0} m      {:>4.0}%        {:.0} m",
            LETTERS[k],
            low,
            full,
            sum / 72.0,
            open_pc,
            longest.0
        );
        if low + full < 3 {
            warn.push(format!("{} has almost no cover within 20 m ({} blocks): whoever holds it is in the open.", LETTERS[k], low + full));
        }
        if open_pc > 60.0 {
            warn.push(format!("{} is open past 80 m in {open_pc:.0}% of directions: long-range weapons will own it.", LETTERS[k]));
        }
    }

    // Who can see whom.
    let _ = writeln!(s, "\nSIGHT BETWEEN OBJECTIVES (eye to eye)");
    let mut pairs = Vec::new();
    for a in 0..5 {
        for b in a + 1..5 {
            let (pa, pb) = (eye_at(w, w.points[a].x, w.points[a].z), eye_at(w, w.points[b].x, w.points[b].z));
            if w.los(pa, pb, 0) && !w.foliage(pa, pb) {
                pairs.push(format!("{}-{} ({:.0} m)", LETTERS[a], LETTERS[b], (pa - pb).length()));
            }
        }
    }
    let _ = writeln!(s, "{}", if pairs.is_empty() { "none".into() } else { pairs.join(", ") });

    // Spawn exposure: can the far side's objectives see into a base?
    let _ = writeln!(s, "\nSPAWN EXPOSURE (share of 24 spots in each base ring seen from the enemy's objectives)");
    for base in 0..2 {
        let bp = w.bases[base];
        let enemy: Vec<usize> = if base == 0 { vec![2, 3, 4] } else { vec![0, 1, 2] };
        let mut seen = 0;
        for i in 0..24 {
            let a = i as f32 / 24.0 * std::f32::consts::TAU;
            let r = 6.0 + (i % 3) as f32 * 6.0;
            let spot = eye_at(w, bp.x + a.cos() * r, bp.z + a.sin() * r);
            if enemy.iter().any(|&k| {
                let e = eye_at(w, w.points[k].x, w.points[k].z);
                (e - spot).length() < 120.0 && w.los(e, spot, 0) && !w.foliage(e, spot)
            }) {
                seen += 1;
            }
        }
        let pc = seen as f32 / 24.0 * 100.0;
        let _ = writeln!(s, "{} base: {pc:.0}%", ["west", "east"][base]);
        if pc > 15.0 {
            warn.push(format!(
                "The {} base is in view of the enemy's objectives at {pc:.0}% of its spawn spots: add a wall or berm.",
                ["west", "east"][base]
            ));
        }
    }

    // Blocks that are probably mistakes.
    let mut floating = Vec::new();
    let mut buried = Vec::new();
    for (i, b) in w.blocks.iter().enumerate() {
        let (cx, cz) = ((b.min.x + b.max.x) * 0.5, (b.min.z + b.max.z) * 0.5);
        let g = w.height(cx, cz);
        if b.min.y > g + 0.3 {
            // Held up by something under it or beside it (a lintel by its
            // wall, a roof by its posts) that reaches from lower down.
            let e = 0.05;
            let held = w.blocks.iter().enumerate().any(|(j, o)| {
                j != i
                    && o.min.y < b.min.y - e
                    && o.max.y >= b.min.y - 0.35
                    && o.min.x <= b.max.x + e
                    && o.max.x >= b.min.x - e
                    && o.min.z <= b.max.z + e
                    && o.max.z >= b.min.z - e
            });
            if !held {
                floating.push(brush_of(i));
            }
        }
        let low = [(b.min.x, b.min.z), (b.max.x, b.min.z), (b.min.x, b.max.z), (b.max.x, b.max.z)]
            .iter()
            .map(|&(x, z)| w.height(x, z))
            .fold(g, f32::min);
        if b.max.y < low - 0.02 {
            buried.push(brush_of(i));
        }
    }
    let mut overlaps = 0;
    let mut overlap_ids = Vec::new();
    for i in 0..w.blocks.len() {
        for j in i + 1..w.blocks.len() {
            let (a, b) = (&w.blocks[i], &w.blocks[j]);
            let ov = |lo: f32, hi: f32, lo2: f32, hi2: f32| (hi.min(hi2) - lo.max(lo2)).max(0.0);
            let v =
                ov(a.min.x, a.max.x, b.min.x, b.max.x) * ov(a.min.y, a.max.y, b.min.y, b.max.y) * ov(a.min.z, a.max.z, b.min.z, b.max.z);
            if v > 1.0 {
                overlaps += 1;
                if overlap_ids.len() < 12 {
                    overlap_ids.push(format!("{}+{}", brush_of(i), brush_of(j)));
                }
            }
        }
    }
    let ids = |v: &mut Vec<usize>| {
        v.sort_unstable();
        v.dedup();
        v.iter().take(20).map(|i| i.to_string()).collect::<Vec<_>>().join(", ")
    };
    if !floating.is_empty() {
        warn.push(format!("Floating blocks (nothing under them): {}. Lower them or give them a support.", ids(&mut floating)));
    }
    if !buried.is_empty() {
        warn.push(format!("Blocks buried under the ground: {}.", ids(&mut buried)));
    }
    // How much of each base's spawn ring is taken up by blocks.
    for (k, bp) in w.bases.iter().enumerate() {
        let (mut all, mut taken) = (0, 0);
        let mut r = 4.0;
        while r <= BASE_SPAWN_RADIUS {
            let n = (r * 1.2) as usize;
            for i in 0..n {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                let (x, z) = (bp.x + a.cos() * r, bp.z + a.sin() * r);
                all += 1;
                if nav.is_blocked(x, z) {
                    taken += 1;
                }
            }
            r += 2.0;
        }
        let pc = taken as f32 / all.max(1) as f32 * 100.0;
        if pc > 40.0 {
            warn.push(format!(
                "{pc:.0}% of the {} base's spawn ring is under blocks: spawns will bunch in what is left.",
                ["west", "east"][k]
            ));
        }
    }
    if overlaps > 0 {
        warn.push(format!(
            "{overlaps} pairs of blocks overlap by over 1 m^3 (fine for a rock pile, often a slip): {}.",
            overlap_ids.join(", ")
        ));
    }
    ground(d, w, &mut s, &mut warn);
    let used = w.blocks.len() as f32 / doc::MAX_BLOCKS as f32;
    if used > 0.85 {
        warn.push(format!("{:.0}% of the block budget is used.", used * 100.0));
    }

    let mut head = String::new();
    let _ = writeln!(head, "MAP DOCTOR: {}  ({} blocks built, {:.0}% of the budget)\n", d.meta.name, w.blocks.len(), used * 100.0);
    if warn.is_empty() {
        let _ = writeln!(head, "No problems found.\n");
    } else {
        let _ = writeln!(head, "{} things to look at (block ids are document ids):", warn.len());
        for x in &warn {
            let _ = writeln!(head, "- {x}");
        }
        head.push('\n');
    }
    head + &s
}

/// The ground: what was sculpted, slopes too steep to mean what they look
/// like, and objectives in hollows or on rises.
fn ground(d: &MapDoc, w: &World, s: &mut String, warn: &mut Vec<String>) {
    let h = d.heights();
    let mut plain = d.clone();
    plain.terrain.delta.clear();
    let base = plain.heights();
    let moved = |i: usize| (h[i] - base[i]).abs() > 0.05;
    let inside = |x: f32, z: f32| x.abs() <= HALF_X && z.abs() <= HALF_Z;
    let (mut lo, mut hi, mut shaped) = (f32::MAX, f32::MIN, 0usize);
    for (i, v) in h.iter().enumerate() {
        let (x, z) = terrain::point(i);
        if inside(x, z) {
            lo = lo.min(*v);
            hi = hi.max(*v);
            if moved(i) {
                shaped += 1;
            }
        }
    }
    let from = match d.terrain.preset {
        Some(p) => format!("{}'s ground", MAPS.get(p as usize).map_or("?", |m| m.name)),
        None => "flat ground".into(),
    };
    let _ = writeln!(s, "\nGROUND");
    let _ = writeln!(
        s,
        "{from}{}; {:.0} to {:.0} m across the battlefield",
        if shaped == 0 { ", not sculpted".to_string() } else { format!(", sculpted over {:.0} m^2", shaped as f32 * CELL * CELL) },
        lo,
        hi
    );

    // Sculpted slopes over about 37 degrees: they read as cliffs, but the
    // game lets everyone walk up any ground.
    let mut steep: Vec<(f32, f32, f32)> = Vec::new();
    for iz in 0..GZ - 1 {
        for ix in 0..GX - 1 {
            let i = iz * GX + ix;
            let (x, z) = terrain::point(i);
            if !inside(x, z) || !(moved(i) || moved(i + 1) || moved(i + GX)) {
                continue;
            }
            let rise = (h[i + 1] - h[i]).abs().max((h[i + GX] - h[i]).abs());
            if rise > 1.5 {
                steep.push((x + CELL * 0.5, z + CELL * 0.5, rise / CELL));
            }
        }
    }
    if !steep.is_empty() {
        // A few places, each the steepest within 20 m.
        steep.sort_by(|a, b| b.2.total_cmp(&a.2));
        let mut spots: Vec<(f32, f32, f32)> = Vec::new();
        for p in &steep {
            if spots.iter().all(|q| (q.0 - p.0).hypot(q.1 - p.1) > 20.0) {
                spots.push(*p);
            }
        }
        let list: Vec<String> = spots
            .iter()
            .take(5)
            .map(|(x, z, g)| format!("({x:.0}, {z:.0}){} {:.0} deg", near_point(w, *x, *z, 25.0), g.atan().to_degrees()))
            .collect();
        warn.push(format!(
            "Steep sculpted ground at {}{}: players and bots walk up any slope, so it will not stop anyone. Use rock blocks where a cliff should.",
            list.join(", "),
            if spots.len() > 5 { format!(" and {} more", spots.len() - 5) } else { String::new() }
        ));
    }

    // Objectives in a hollow or on a rise, and capture zones on a slope.
    let _ = writeln!(s, "point  ground  vs around (12-24 m)  height across the zone (8 m)");
    for (k, p) in w.points.iter().enumerate() {
        let at = w.height(p.x, p.z);
        let (mut sum, mut n) = (0.0, 0);
        for r in [12.0f32, 18.0, 24.0] {
            for a in 0..24 {
                let t = a as f32 / 24.0 * std::f32::consts::TAU;
                sum += w.height(p.x + r * t.cos(), p.z + r * t.sin());
                n += 1;
            }
        }
        let around = sum / n as f32;
        let (mut zlo, mut zhi) = (at, at);
        for a in 0..16 {
            let t = a as f32 / 16.0 * std::f32::consts::TAU;
            for r in [4.0f32, 8.0] {
                let v = w.height(p.x + r * t.cos(), p.z + r * t.sin());
                zlo = zlo.min(v);
                zhi = zhi.max(v);
            }
        }
        let _ = writeln!(s, "{}      {at:6.1}  {:+6.1}               {:.1}", LETTERS[k], at - around, zhi - zlo);
        if at - around < -3.0 {
            warn.push(format!(
                "{} sits in a hollow {:.0} m below the ground around it: attackers look down into it.",
                LETTERS[k],
                around - at
            ));
        } else if at - around > 3.0 {
            warn.push(format!(
                "{} sits on a rise {:.0} m above the ground around it: holders see over everything and are seen from everywhere.",
                LETTERS[k],
                at - around
            ));
        }
        if zhi - zlo > 4.0 {
            warn.push(format!("{}'s capture zone spans {:.0} m of height: part of it is hidden from the rest.", LETTERS[k], zhi - zlo));
        }
    }
}

/// What a headless bot match on the map showed.
pub struct Soak {
    pub text: String,
    pub deaths: Vec<[f32; 2]>,
}

pub fn soak(d: &MapDoc, mode: u8, secs: u32, per_team: usize) -> Result<Soak, String> {
    let id = doc::install(d.clone())?;
    let mut g = Game::new(Config {
        port: 0,
        per_team,
        seed: SEED,
        map: id,
        rotation: Vec::new(),
        lobby_secs: 0,
        tickets: 5000,
        bot_skill: 1.0,
        log: false,
        discover: false,
        mode,
        score_limit: 100_000,
        time_limit: 100_000,
        ..Default::default()
    });
    let started = std::time::Instant::now();
    let mut seen = g.feed.len();
    let mut alive: Vec<bool> = g.players.iter().map(|p| p.alive).collect();
    let mut deployed: Vec<u32> = vec![0; g.players.len()];
    let mut deaths = Vec::new();
    let mut ranges = Vec::new();
    let (mut kills, mut caps) = ([0u32; 2], [0u32; 2]);
    let mut caps_at = [0u32; 5];
    let mut quick = 0;
    let mut spawns = 0;
    let ticks = secs * TICK_HZ;
    for _ in 0..ticks {
        // Who is about to be killed is where they stand now.
        let before: Vec<Vec3> = g.players.iter().map(|p| p.st.pos).collect();
        g.tick();
        for (i, p) in g.players.iter().enumerate() {
            if i >= alive.len() {
                alive.push(p.alive);
                deployed.push(g.tick);
                continue;
            }
            if p.alive && !alive[i] {
                deployed[i] = g.tick;
                spawns += 1;
            }
            alive[i] = p.alive;
        }
        for (_, f) in g.feed.since(seen) {
            match *f {
                Feed::Kill { killer, victim, .. } => {
                    let v = victim as usize;
                    let Some(&vp) = before.get(v) else { continue };
                    deaths.push([vp.x, vp.z]);
                    if let Some(team) = g.players.get(killer as usize).map(|p| p.team) {
                        if (team as usize) < 2 && killer != victim {
                            kills[team as usize] += 1;
                            ranges.push((before[killer as usize] - vp).length());
                        }
                    }
                    if g.tick.saturating_sub(deployed.get(v).copied().unwrap_or(0)) < 5 * TICK_HZ {
                        quick += 1;
                    }
                }
                Feed::Capture { point, team } => {
                    if (team as usize) < 2 {
                        caps[team as usize] += 1;
                    }
                    if (point as usize) < 5 {
                        caps_at[point as usize] += 1;
                    }
                }
                _ => {}
            }
        }
        seen = g.feed.len();
    }
    let w = &g.world;
    let mut s = String::new();
    let total = deaths.len();
    let _ = writeln!(
        s,
        "SOAK: {} s of {} with {per_team} bots a side on {} (ran in {:.1} s)",
        secs,
        mode_def(mode).name,
        d.meta.name,
        started.elapsed().as_secs_f32()
    );
    let _ = writeln!(s, "deaths {total}; kills west {} / east {}; captures west {} / east {}", kills[0], kills[1], caps[0], caps[1]);
    ranges.sort_by(f32::total_cmp);
    if !ranges.is_empty() {
        let med = ranges[ranges.len() / 2];
        let far = ranges.iter().filter(|&&r| r > 50.0).count() as f32 / ranges.len() as f32 * 100.0;
        let close = ranges.iter().filter(|&&r| r < 10.0).count() as f32 / ranges.len() as f32 * 100.0;
        let _ = writeln!(
            s,
            "kill range: median {med:.0} m; {close:.0}% under 10 m; {far:.0}% over 50 m; longest {:.0} m",
            ranges[ranges.len() - 1]
        );
    }
    if spawns > 0 {
        let _ = writeln!(s, "deaths within 5 s of deploying: {quick} ({:.0}% of {spawns} deploys)", quick as f32 / spawns as f32 * 100.0);
    }
    let caps_txt: Vec<String> = (0..5).map(|k| format!("{} {}", LETTERS[k], caps_at[k])).collect();
    let _ = writeln!(s, "captures by objective: {}", caps_txt.join(", "));
    let mut near = [0u32; 5];
    for dp in &deaths {
        for (k, p) in w.points.iter().enumerate() {
            if ((p.x - dp[0]).powi(2) + (p.z - dp[1]).powi(2)).sqrt() < 20.0 {
                near[k] += 1;
            }
        }
    }
    let near_txt: Vec<String> = (0..5).map(|k| format!("{} {}", LETTERS[k], near[k])).collect();
    let _ = writeln!(s, "deaths within 20 m of each objective: {}", near_txt.join(", "));
    // Hot spots on a 15 m grid.
    let mut cells = std::collections::HashMap::new();
    for dp in &deaths {
        *cells.entry(((dp[0] / 15.0).floor() as i32, (dp[1] / 15.0).floor() as i32)).or_insert(0u32) += 1;
    }
    let mut hot: Vec<_> = cells.into_iter().collect();
    hot.sort_by_key(|h| std::cmp::Reverse(h.1));
    let _ = writeln!(s, "hot spots (15 m squares, by centre):");
    for ((cx, cz), n) in hot.iter().take(8) {
        let (x, z) = ((*cx as f32 + 0.5) * 15.0, (*cz as f32 + 0.5) * 15.0);
        let _ =
            writeln!(s, "  ({x:.0}, {z:.0}){}: {n} deaths ({:.0}%)", near_point(w, x, z, 30.0), *n as f32 / total.max(1) as f32 * 100.0);
    }
    if mode == ferrite_shared::mode::CONQUEST && caps[0] + caps[1] == 0 {
        let _ = writeln!(s, "warning: nobody captured anything - check the routes in analyze_map.");
    }
    Ok(Soak { text: s, deaths })
}
