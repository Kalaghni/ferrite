//! FORGE MCP server: lets an AI assistant read, edit, judge and play-test
//! FERRITE maps, live in the open map maker or on `.fmap` files.
//!
//! It speaks the Model Context Protocol over stdio (newline-separated
//! JSON-RPC). When FORGE is open, reads and edits go through it, so every
//! change shows up on screen as an ordinary undo step; otherwise they go
//! straight to the file.

mod analyze;
mod live;
mod plan;
mod png;

use ferrite_shared::map::doc::MapDoc;
use ferrite_shared::map::library::{self, Library};
use ferrite_shared::map::maps::{self, Start};
use ferrite_shared::map::ops::{self, Op, Prefab};
use ferrite_shared::map::{BlockKind, MAPS};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

const INSTRUCTIONS: &str = r#"FERRITE map maker (FORGE). Maps are documents of axis-aligned boxes ("blocks") on a heightfield, played 12 v 12 with bots.

Space: metres, y up. x runs west (-177) to east (177), z north (-102) to south (102). The west base is at (-159, 0), the east at (159, 0); objectives A..E run west to east (default A(-108,21) B(-54,-36) C(0,0) D(54,36) E(108,-21)). Points are given as [x, z].

Symmetry: maps are usually half_turn - every block is copied to (-x, -z) when built, so build the west half only (x < 0) and the east appears. A block across x = 0 gets a copy overlapping it; keep centre pieces centred on (0,0) or clear of the line.

Ground: a 2 m grid of heights, shaped with the sculpt op (raise, lower, flatten, smooth, ramp, noise). Anyone walks up any slope, so steep ground is not a wall: cliffs that must stop players are rock blocks. Read the ground with plan_image and the GROUND part of analyze_map.

Players: eye 1.62 m, step up 0.5 m freely, mantle ledges up to about 1.9 m, walls 3.2 m and up cannot be climbed. Waist cover 1.0-1.2 m hides a crouched player; 1.8 m and up is full cover. Blocks are boxes only: no rotation except quarter turns.

Block kinds and typical sizes [x, y, z]: container [6, 2.6, 2.5], concrete wall [6, 3, 0.6], sandbag [4, 1.1, 0.9], crate [1.1, 1.1, 1.1] (crates can be destroyed), metal [3, 3, 3], rock [4, 2.5, 3.5], step [1.2, 0.3, 1.2], trunk (a post) [0.5, 4, 0.5]. Tint 0-3 varies the colour.

The library holds reusable models ("prefabs"): named groups of blocks and trees (a bunker, a tower, a wall run) in folders. Built in: BLOCKS (one of each kind), COVER, STRUCTURES and NATURE; the developer's own live in folders on disk beside them. FORGE shows the library as a grid of cards the developer drags into the world, and it rereads it every couple of seconds, so a model you save appears there at once. Make one with save_prefab (from blocks you design, or by capturing blocks already in the map) into a fitting folder; stamp any model with the edit op {"op":"prefab","name":...,"at":[x,z],"quarters":0-3}.

Maps live in the maps folder (list_maps): FORGE's start screen shows each as a card, newest first, and picks up new files on its own. Name a map there by its name or file name wherever a tool takes `path`; create_map puts new maps there unless given a path.

Workflow: forge_status -> describe_map / plan_image to see the map -> edit_map (batches are all-or-nothing) -> analyze_map for routes, cover, sight lines and mistakes -> soak_test to watch bots fight on it -> screenshot to look at it in 3D. In live mode edits are undoable on screen and nothing is saved until save_map. Use show_user to fly the developer's camera to what you are talking about."#;

fn tools() -> Value {
    let path = json!({ "type": "string", "description": "A map: its name in the maps folder (see list_maps) or a .fmap path. Leave out to use the map open in FORGE." });
    let pt = json!({ "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 });
    let v3 = json!({ "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 });
    json!([
        {
            "name": "forge_status",
            "description": "Whether FORGE is open, on which map, with unsaved changes or not, and where its camera is.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "list_maps",
            "description": "The maps in the maps folder, newest first: name, file, when last changed, block count, symmetry, and whether it is open in FORGE or has unsaved work in an autosave.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "create_map",
            "description": "Start a new map: empty on flat ground, empty on a built-in map's terrain, or a copy of a built-in map to remix (folded to its west half where it is the same under a half turn, so edits land on both sides). It goes in the maps folder, where FORGE's start screen shows it, unless path says otherwise.",
            "inputSchema": { "type": "object", "required": ["name"], "properties": {
                "path": { "type": "string", "description": "A .fmap path to write instead of a new file in the maps folder." },
                "name": { "type": "string", "description": "Up to 32 plain characters." },
                "terrain": { "type": "integer", "description": "Built-in map id whose ground to start from (see forge_status for the list); leave out for flat." },
                "mood": { "type": "integer", "description": "Built-in map id whose sky, light and foliage to borrow." },
                "copy_of": { "type": "integer", "description": "Start from a full copy of a built-in map (its blocks, trees and ground, written out with no symmetry). Its bespoke art and special rules do not carry over." }
            } }
        },
        {
            "name": "describe_map",
            "description": "The map in words: symmetry, objectives, bases, counts, and its blocks with ids, kinds, corners and sizes. Ids are what edit_map uses.",
            "inputSchema": { "type": "object", "properties": {
                "path": path,
                "region": { "type": "array", "items": { "type": "number" }, "minItems": 4, "maxItems": 4, "description": "[x0, z0, x1, z1]: only blocks centred inside." },
                "kind": { "type": "string" },
                "limit": { "type": "integer", "description": "Most blocks to list (default 250)." }
            } }
        },
        {
            "name": "plan_image",
            "description": "A top-down picture of the whole built map (both halves): shaded ground, blocks coloured by kind (lighter is taller), trees, objectives A-E (white rings, 12 m) and bases W/E (20 m rings). North is up, 0.75 m a pixel, 25 m grid. With heat, it glows where bots died in the last soak_test.",
            "inputSchema": { "type": "object", "properties": { "path": path, "heat": { "type": "boolean" } } }
        },
        {
            "name": "edit_map",
            "description": concat!(
                "Apply a batch of edits in order; all land or none do. Ids are document ids as they stand at each op (see describe_map). Ops (field `op`):\n",
                "place {kind, at:[x,z], size:[x,y,z], tint?, lift?} - stands a block on the ground, foot sunk to the lowest ground under it; lift raises it (onto a roof).\n",
                "add {kind, min:[x,y,z], max:[x,y,z], tint?} - exact corners.\n",
                "move {ids, by:[x,y,z]}; reshape {id, min, max}; restyle {ids, kind?, tint?}; turn {ids, quarters?, about?:[x,z]} (clockwise from above);\n",
                "copy {ids, by} - copies added at the end; remove {ids}; clear {from:[x,z], to:[x,z], kind?} - removes blocks centred in the rectangle;\n",
                "prefab {name, at:[x,z], quarters?, lift?} - stamp a saved prefab; tree {at, height, radius, pine?};\n",
                "objective {point: 0-4 for A-E, at}; base {team: 0 west / 1 east, at}; rename {name?, blurb?}; look {mood?, ground?: [[r,g,b] x3]}.\n",
                "sculpt {mode, at:[x,z], radius (1-80 m), amount?, height?, to?, heights?, scale?, seed?} - shape the ground with a round brush, soft at the rim:\n",
                "  raise / lower: amount m at the centre (default 1); flatten: towards height (default the ground at `at`), amount 0-1 of the way (default 1), flat over the inner half;\n",
                "  smooth: amount 0-1 (default 0.5); ramp: from `at` to `to`, radius either side, rising from the ground at each end or `heights`: [a, b];\n",
                "  noise: lumps up to amount m (default 1), scale m across (default 8), seed. Mirrored in half-turn maps. Blocks footed in the ground, what is stacked on them, and trees move with it; later ops in the batch stand on the new ground.\n",
                "Returns the ids of added blocks."
            ),
            "inputSchema": { "type": "object", "required": ["ops"], "properties": {
                "path": path,
                "ops": { "type": "array", "items": { "type": "object" } },
                "note": { "type": "string", "description": "Shown to the developer in FORGE, briefly." }
            } }
        },
        {
            "name": "analyze_map",
            "description": "The map doctor: routes from both bases to every objective on the bots' nav grid, fairness between the sides, cover and openness at each objective, sight lines between objectives, how exposed each base's spawns are, and blocks that look like mistakes (floating, buried, overlapping, in a spawn ring).",
            "inputSchema": { "type": "object", "properties": { "path": path } }
        },
        {
            "name": "soak_test",
            "description": "Play the map headless with bots and report: kills and captures per side, kill ranges, deaths soon after deploying, captures per objective, and hot spots. Returns a heat plan of where people died. 120 s takes a few seconds.",
            "inputSchema": { "type": "object", "properties": {
                "path": path,
                "mode": { "type": "string", "description": "conquest (default), tdm, ctf or stronghold." },
                "seconds": { "type": "integer", "description": "Match time to simulate, 10-600 (default 120)." },
                "per_team": { "type": "integer", "description": "Bots a side, 1-12 (default 10)." }
            } }
        },
        {
            "name": "screenshot",
            "description": "A picture from FORGE's 3D view, as the game renders it. Give eye and at to place the camera for the shot (the developer's own view is put back after); leave out to see what the developer sees. Needs FORGE open.",
            "inputSchema": { "type": "object", "properties": {
                "eye": v3, "at": v3,
                "hud": { "type": "boolean", "description": "Include the editor's panels (default false)." }
            } }
        },
        {
            "name": "show_user",
            "description": "Move the developer's FORGE camera, select a block and/or show them a short note - to point at what you are talking about.",
            "inputSchema": { "type": "object", "properties": {
                "eye": v3, "at": v3,
                "select": { "type": ["integer", "null"], "description": "Block id to select; null clears the selection." },
                "note": { "type": "string" }
            } }
        },
        {
            "name": "save_map",
            "description": "Save the map open in FORGE to its file.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "undo",
            "description": "Undo (or with redo: true, redo) edits in FORGE.",
            "inputSchema": { "type": "object", "properties": { "steps": { "type": "integer" }, "redo": { "type": "boolean" } } }
        },
        {
            "name": "list_prefabs",
            "description": "The model library by folder: each model's name, what it is, block count, footprint and height, and whether it is built in. Give folder to see only that folder and those under it.",
            "inputSchema": { "type": "object", "properties": { "folder": { "type": "string" } } }
        },
        {
            "name": "get_prefab",
            "description": "One model's blocks and trees, to read, or to change and save as a new model. name may be NAME or FOLDER/NAME.",
            "inputSchema": { "type": "object", "required": ["name"], "properties": { "name": { "type": "string" } } }
        },
        {
            "name": "save_prefab",
            "description": "Save a model to the library, into folder (made if need be; nest with /, e.g. COVER/WALLS). Either give blocks (offsets from its origin: x and z across the ground, y up from the ground, so a block standing on the ground has min y 0; centre the footprint on the origin) or ids of blocks in a map to capture. Saving a name into the same folder again replaces it. It appears in FORGE's library within a couple of seconds.",
            "inputSchema": { "type": "object", "required": ["name"], "properties": {
                "path": path,
                "name": { "type": "string" },
                "folder": { "type": "string", "description": "Library folder, e.g. COVER or STRUCTURES/TOWERS. Default: MINE." },
                "about": { "type": "string" },
                "blocks": { "type": "array", "items": { "type": "object", "properties": {
                    "kind": { "type": "string" }, "min": v3, "max": v3, "tint": { "type": "integer" }
                } } },
                "trees": { "type": "array", "items": { "type": "object", "properties": {
                    "at": pt, "height": { "type": "number" }, "radius": { "type": "number" }, "pine": { "type": "boolean" }
                } } },
                "ids": { "type": "array", "items": { "type": "integer" } }
            } }
        }
    ])
}

/// Where a tool's map is: in the open editor, or a file.
enum Target {
    Live(PathBuf),
    File(PathBuf),
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// A map named by path, or by name in the maps folder.
fn map_path(s: &str) -> Result<PathBuf, String> {
    maps::resolve(&maps::default_root(), s)
        .ok_or_else(|| format!("no map called {s:?} in the maps folder (see list_maps), and it is not a .fmap path"))
}

fn target(args: &Value) -> Result<Target, String> {
    let st = live::status();
    let open = st.as_ref().and_then(|s| s["path"].as_str()).map(PathBuf::from);
    match args["path"].as_str() {
        Some(p) => {
            let p = map_path(p)?;
            Ok(match open {
                Some(o) if same_file(&o, &p) => Target::Live(o),
                _ => Target::File(p),
            })
        }
        None => open
            .map(Target::Live)
            .ok_or_else(|| "FORGE is not open and no `path` was given: pass a .fmap path, or open one with `ferrite --edit FILE`.".into()),
    }
}

fn load(t: &Target) -> Result<MapDoc, String> {
    match t {
        Target::Live(_) => {
            let v = live::request(json!({ "cmd": "doc" }), Duration::from_secs(10))?;
            serde_json::from_value(v["doc"].clone()).map_err(|e| format!("FORGE sent a bad map: {e}"))
        }
        Target::File(p) => {
            let text = std::fs::read_to_string(p).map_err(|e| format!("cannot read {}: {e}", p.display()))?;
            MapDoc::from_json(&text)
        }
    }
}

fn write_file(p: &Path, d: &MapDoc) -> Result<(), String> {
    let tmp = p.with_extension("fmap.tmp");
    std::fs::write(&tmp, d.to_json()).and_then(|_| std::fs::rename(&tmp, p)).map_err(|e| format!("cannot write {}: {e}", p.display()))
}

fn library() -> Library {
    Library::load(&library::default_root())
}

fn find_prefab(lib: &Library, name: &str) -> Result<Prefab, String> {
    lib.find(name).map(|e| e.prefab.clone()).ok_or_else(|| format!("no model {name:?} in the library (see list_prefabs)"))
}

fn text(s: impl Into<String>) -> Value {
    json!({ "type": "text", "text": s.into() })
}

fn image(png: &[u8]) -> Value {
    json!({ "type": "image", "data": png::base64(png), "mimeType": "image/png" })
}

#[derive(Default)]
struct Server {
    /// Where bots died in the last soak, and on which map.
    heat: Option<(String, Vec<[f32; 2]>)>,
    shots: u32,
}

fn kind_of(s: &str) -> Result<BlockKind, String> {
    serde_json::from_value(json!(s.to_ascii_lowercase()))
        .map_err(|_| format!("unknown kind {s:?}: concrete, container, rock, crate, sandbag, metal, trunk or step"))
}

impl Server {
    fn call(&mut self, name: &str, args: &Value) -> Result<Vec<Value>, String> {
        match name {
            "forge_status" => {
                let maps: Vec<String> = MAPS.iter().enumerate().map(|(i, m)| format!("{i} {}", m.name)).collect();
                let builtin = format!("Built-in maps (ids for terrain and mood): {}", maps.join(", "));
                Ok(vec![text(match live::status() {
                    Some(v) => format!(
                        "FORGE is open on {} ({}){}. {} blocks in the document, {} built. Undo steps: {}. Camera at {}, facing {}. Selected: {}.\n{builtin}",
                        v["name"].as_str().unwrap_or("?"),
                        v["path"].as_str().unwrap_or("?"),
                        if v["dirty"].as_bool().unwrap_or(false) { " with unsaved changes" } else { "" },
                        v["brushes"],
                        v["blocks"],
                        v["undo"],
                        v["eye"],
                        v["facing"],
                        v["selected"]
                    ),
                    None => format!("FORGE is not open. Tools still work on maps by name or path (see list_maps); `ferrite --edit NAME` opens one to watch.\n{builtin}"),
                })])
            }
            "create_map" => {
                let name = args["name"].as_str().unwrap_or("UNTITLED");
                let id = |v: &Value| v.as_u64().map(|i| u8::try_from(i).map_err(|_| "unknown built-in map".to_string())).transpose();
                let start = match (id(&args["copy_of"])?, id(&args["terrain"])?) {
                    (Some(c), _) => Start::Copy(c),
                    (None, Some(t)) => Start::Ground(t),
                    (None, None) => Start::Blank,
                };
                let mut d = maps::starter(name, start)?;
                if let Some(m) = id(&args["mood"])? {
                    if m as usize >= MAPS.len() {
                        return Err("unknown mood".into());
                    }
                    d.look.mood = m;
                }
                let p = match args["path"].as_str() {
                    Some(p) => PathBuf::from(p),
                    None => maps::fresh_path(&maps::default_root(), &d.meta.name),
                };
                if p.exists() {
                    return Err(format!("{} already exists", p.display()));
                }
                maps::write(&p, &d).map_err(|e| format!("cannot write {}: {e}", p.display()))?;
                let half = if d.symmetry == ferrite_shared::map::doc::Symmetry::HalfTurn {
                    "mirrored: build the west half"
                } else {
                    "every block written out"
                };
                Ok(vec![text(format!(
                    "Created {} at {} ({} blocks, {half}). It is on FORGE's start screen; `ferrite --edit \"{}\"` opens it directly.",
                    d.meta.name,
                    p.display(),
                    d.brushes.len(),
                    d.meta.name
                ))])
            }
            "list_maps" => {
                let root = maps::default_root();
                let list = maps::scan(&root);
                let open = live::status();
                let open_path = open.as_ref().and_then(|v| v["path"].as_str()).map(PathBuf::from);
                let mut out = format!("Maps folder: {} ({} map{})\n", root.display(), list.len(), if list.len() == 1 { "" } else { "s" });
                if list.is_empty() {
                    out.push_str("Empty. create_map makes one here.\n");
                }
                for m in &list {
                    let file = m.path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                    let mut line = match &m.doc {
                        Ok(s) => format!(
                            "  {} - {file} - changed {}, {} blocks, {} trees, {}, revision {}",
                            s.name,
                            maps::ago(m.modified),
                            s.blocks,
                            s.trees,
                            if s.mirrored { "mirrored" } else { "no symmetry" },
                            s.revision
                        ),
                        Err(e) => format!("  {file} - cannot be read: {e}"),
                    };
                    if let Ok(s) = &m.doc {
                        if !s.blurb.is_empty() {
                            line.push_str(&format!(" - \"{}\"", s.blurb));
                        }
                    }
                    if open_path.as_ref().is_some_and(|o| same_file(o, &m.path)) {
                        let dirty = open.as_ref().is_some_and(|v| v["dirty"].as_bool().unwrap_or(false));
                        line.push_str(if dirty { " [OPEN IN FORGE, unsaved changes]" } else { " [OPEN IN FORGE]" });
                    }
                    if let Some(a) = m.autosave {
                        line.push_str(&format!(" [autosave from {} is newer than the file]", maps::ago(a)));
                    }
                    out.push_str(&line);
                    out.push('\n');
                }
                if let Some(o) = open_path.filter(|o| !list.iter().any(|m| same_file(o, &m.path))) {
                    out.push_str(&format!("FORGE has a map open from outside the folder: {}\n", o.display()));
                }
                Ok(vec![text(out)])
            }
            "describe_map" => {
                let t = target(args)?;
                let d = load(&t)?;
                let w = analyze::world_of(&d)?;
                let region = args["region"].as_array().and_then(|a| {
                    let v: Vec<f32> = a.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect();
                    (v.len() == 4).then(|| [v[0], v[1], v[2], v[3]])
                });
                let kind = args["kind"].as_str().map(kind_of).transpose()?;
                let limit = args["limit"].as_u64().unwrap_or(250) as usize;
                let where_ = match &t {
                    Target::Live(p) => format!("(open in FORGE: {})\n", p.display()),
                    Target::File(p) => format!("(file: {})\n", p.display()),
                };
                Ok(vec![text(where_ + &analyze::describe(&d, &w, region, kind, limit))])
            }
            "plan_image" => {
                let t = target(args)?;
                let d = load(&t)?;
                let w = analyze::world_of(&d)?;
                let heat = match (&self.heat, args["heat"].as_bool().unwrap_or(false)) {
                    (Some((n, h)), true) if *n == d.meta.name => h.as_slice(),
                    (_, true) => return Err("no soak_test has run on this map yet".into()),
                    _ => &[],
                };
                let png = plan::render(&w, d.look.ground, heat);
                let (pw, ph) = plan::size();
                Ok(vec![
                    image(&png),
                    text(format!(
                        "{} plan, {pw} x {ph} px, {} m a pixel: pixel (px, py) is x = {:.1} + px * {s}, z = {:.1} + py * {s}.",
                        d.meta.name,
                        plan::SCALE,
                        -ferrite_shared::map::HALF_X,
                        -ferrite_shared::map::HALF_Z,
                        s = plan::SCALE
                    )),
                ])
            }
            "edit_map" => {
                let t = target(args)?;
                let lib = library();
                let mut raw = args["ops"].as_array().cloned().ok_or("ops must be a list")?;
                // Prefabs by name become their contents.
                for op in &mut raw {
                    if op["op"] == "prefab" && op.get("prefab").is_none() {
                        let name = op["name"].as_str().ok_or("a prefab op needs a name")?.to_string();
                        op["prefab"] = serde_json::to_value(find_prefab(&lib, &name)?).map_err(|e| e.to_string())?;
                    }
                }
                let batch: Vec<Op> = serde_json::from_value(Value::Array(raw.clone())).map_err(|e| format!("bad ops: {e}"))?;
                match &t {
                    Target::Live(_) => {
                        let mut req = json!({ "cmd": "apply", "ops": raw });
                        if let Some(n) = args["note"].as_str() {
                            req["note"] = json!(n);
                        }
                        let v = live::request(req, Duration::from_secs(10))?;
                        Ok(vec![text(format!("Applied in FORGE (one undo step, not saved yet): {}", v["outcome"]))])
                    }
                    Target::File(p) => {
                        let mut d = load(&t)?;
                        let w = analyze::world_of(&d)?;
                        let out = ops::apply(&mut d, &batch, &|x, z| w.height(x, z))?;
                        write_file(p, &d)?;
                        Ok(vec![text(format!("Applied and saved to {}: {}", p.display(), serde_json::to_string(&out).unwrap_or_default()))])
                    }
                }
            }
            "analyze_map" => {
                let t = target(args)?;
                let d = load(&t)?;
                let w = analyze::world_of(&d)?;
                Ok(vec![text(analyze::analyze(&d, &w))])
            }
            "soak_test" => {
                let t = target(args)?;
                let d = load(&t)?;
                let mode = ferrite_shared::mode_by_name(args["mode"].as_str().unwrap_or("conquest"))
                    .ok_or("mode: conquest, tdm, ctf or stronghold")?;
                let secs = args["seconds"].as_u64().unwrap_or(120).clamp(10, 600) as u32;
                let per_team = args["per_team"].as_u64().unwrap_or(10).clamp(1, 12) as usize;
                let r = analyze::soak(&d, mode, secs, per_team)?;
                let w = analyze::world_of(&d)?;
                let png = plan::render(&w, d.look.ground, &r.deaths);
                self.heat = Some((d.meta.name.clone(), r.deaths));
                Ok(vec![text(r.text), image(&png)])
            }
            "screenshot" => {
                self.shots += 1;
                let dir = std::env::temp_dir().join("ferrite-forge-shots");
                let file = dir.join(format!("shot-{}-{}.png", std::process::id(), self.shots));
                let mut req = json!({ "cmd": "shot", "path": file.display().to_string(), "hud": args["hud"].as_bool().unwrap_or(false) });
                if args.get("eye").is_some() && args.get("at").is_some() {
                    req["eye"] = args["eye"].clone();
                    req["at"] = args["at"].clone();
                }
                let v = live::request(req, Duration::from_secs(15))?;
                let bytes = std::fs::read(&file).map_err(|e| format!("FORGE saved no picture: {e}"))?;
                let _ = std::fs::remove_file(&file);
                Ok(vec![image(&bytes), text(format!("{} x {} from FORGE.", v["width"], v["height"]))])
            }
            "show_user" => {
                let mut req = json!({ "cmd": "view" });
                for k in ["eye", "at", "note", "select"] {
                    if let Some(v) = args.get(k) {
                        req[k] = v.clone();
                    }
                }
                live::request(req, Duration::from_secs(5))?;
                Ok(vec![text("Done.")])
            }
            "save_map" => {
                let v = live::request(json!({ "cmd": "save" }), Duration::from_secs(10))?;
                Ok(vec![text(format!("Saved {}.", v["path"].as_str().unwrap_or("?")))])
            }
            "undo" => {
                let redo = args["redo"].as_bool().unwrap_or(false);
                let v = live::request(
                    json!({ "cmd": if redo { "redo" } else { "undo" }, "steps": args["steps"].as_u64().unwrap_or(1) }),
                    Duration::from_secs(5),
                )?;
                Ok(vec![text(format!("{} {} step(s); {} blocks now.", if redo { "Redid" } else { "Undid" }, v["steps"], v["brushes"]))])
            }
            "list_prefabs" => {
                let lib = library();
                let only = library::clean_folder(args["folder"].as_str().unwrap_or(""));
                let mut out = format!("Library at {} (built-in models are marked *):\n", lib.root.display());
                let mut folders: Vec<&str> = vec![""];
                folders.extend(lib.folders.iter().map(|f| f.as_str()));
                for f in folders {
                    if !only.is_empty() && f != only && !f.starts_with(&format!("{only}/")) {
                        continue;
                    }
                    let items: Vec<_> = lib.entries.iter().filter(|e| e.folder == f).collect();
                    if items.is_empty() && f.is_empty() {
                        continue;
                    }
                    out.push_str(&format!("\n{}/\n", if f.is_empty() { "(top)" } else { f }));
                    if items.is_empty() {
                        out.push_str("  (empty)\n");
                    }
                    for e in items {
                        let s = e.size();
                        let what = match (e.prefab.blocks.len(), e.prefab.trees.len()) {
                            (b, 0) => format!("{b} blocks"),
                            (0, t) => format!("{t} trees"),
                            (b, t) => format!("{b} blocks, {t} trees"),
                        };
                        out.push_str(&format!(
                            "  {}{} - {} ({what}, {:.1} x {:.1} m, {:.1} m tall)\n",
                            e.prefab.name,
                            if e.file.is_none() { " *" } else { "" },
                            if e.prefab.about.is_empty() { "no description" } else { &e.prefab.about },
                            s[0],
                            s[1],
                            s[2]
                        ));
                    }
                }
                Ok(vec![text(out)])
            }
            "get_prefab" => {
                let lib = library();
                let name = args["name"].as_str().ok_or("name is needed")?;
                let e = lib.find(name).ok_or_else(|| format!("no model {name:?} in the library"))?;
                Ok(vec![text(format!(
                    "folder: {}\n{}",
                    if e.folder.is_empty() { "(top)" } else { &e.folder },
                    serde_json::to_string_pretty(&e.prefab).unwrap_or_default()
                ))])
            }
            "save_prefab" => {
                let name = args["name"].as_str().ok_or("name is needed")?.trim().to_uppercase();
                if name.is_empty() || name.len() > 40 {
                    return Err("a prefab name is 1 to 40 characters".into());
                }
                let mut p = if let Some(ids) = args["ids"].as_array() {
                    let ids: Vec<usize> = ids.iter().filter_map(|v| v.as_u64().map(|x| x as usize)).collect();
                    let d = load(&target(args)?)?;
                    let w = analyze::world_of(&d)?;
                    ops::capture(&d, &ids, &name, &|x, z| w.height(x, z))?
                } else {
                    let blocks = serde_json::from_value(args["blocks"].clone()).map_err(|e| format!("bad blocks: {e}"))?;
                    let trees = if args["trees"].is_array() {
                        serde_json::from_value(args["trees"].clone()).map_err(|e| format!("bad trees: {e}"))?
                    } else {
                        Vec::new()
                    };
                    Prefab { name: name.clone(), about: String::new(), blocks, trees }
                };
                if (p.blocks.is_empty() && p.trees.is_empty()) || p.blocks.len() > 256 {
                    return Err("a prefab holds up to 256 blocks, and at least one block or tree".into());
                }
                for b in &p.blocks {
                    if (0..3).any(|a| {
                        !b.min[a].is_finite()
                            || !b.max[a].is_finite()
                            || b.max[a] - b.min[a] < 0.01
                            || b.min[a].abs() > 60.0
                            || b.max[a].abs() > 60.0
                    }) {
                        return Err("prefab blocks must have size and stay within 60 m of the origin".into());
                    }
                }
                p.about = args["about"].as_str().unwrap_or_default().to_string();
                if p.trees.len() > 64 {
                    return Err("a prefab holds at most 64 trees".into());
                }
                let folder = library::clean_folder(args["folder"].as_str().unwrap_or("MINE"));
                let folder = if folder.is_empty() { "MINE".to_string() } else { folder };
                let f = library().save(&folder, &p)?;
                let live = if live::status().is_some() { " FORGE's library shows it within a couple of seconds." } else { "" };
                Ok(vec![text(format!("Saved {name} ({} blocks) to {folder} ({}).{live}", p.blocks.len(), f.display()))])
            }
            other => Err(format!("no tool {other:?}")),
        }
    }
}

fn main() {
    let mut server = Server::default();
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let _ = writeln!(out, "{}", json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } }));
                let _ = out.flush();
                continue;
            }
        };
        let Some(id) = msg.get("id").cloned() else {
            continue; // a notification
        };
        let params = &msg["params"];
        let result = match msg["method"].as_str().unwrap_or("") {
            "initialize" => Ok(json!({
                "protocolVersion": params["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "ferrite-forge", "version": env!("CARGO_PKG_VERSION") },
                "instructions": INSTRUCTIONS,
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => {
                let name = params["name"].as_str().unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                // A panic in a tool is that tool's error, not the server's end.
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| server.call(name, &args)))
                    .unwrap_or_else(|_| Err(format!("{name} failed inside (a bug)")));
                Ok(match r {
                    Ok(content) => json!({ "content": content, "isError": false }),
                    Err(e) => json!({ "content": [text(e)], "isError": true }),
                })
            }
            m => Err(json!({ "code": -32601, "message": format!("no method {m}") })),
        };
        let reply = match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": e }),
        };
        let _ = writeln!(out, "{reply}");
        let _ = out.flush();
    }
}
