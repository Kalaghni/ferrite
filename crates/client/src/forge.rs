//! FORGE, the map maker: edit a map document (`.fmap`) in the game's own
//! renderer, then play it with bots. See docs/design/map-maker.md.
//!
//! This first cut edits blocks and objectives. The document is the only
//! state: every edit changes it, the world is rebuilt from it a moment
//! later, and undo keeps whole copies of it (they are small).

use crate::forge_home::Field;
use crate::forge_library::{Drag, Shelf};
use crate::forge_link::Link;
use crate::gfx::{self, Col, Mesh};
use crate::{scene, ui};
use ferrite_shared::glam::{vec3, vec4, Mat4, Vec3};
use ferrite_shared::map::doc::{self, Brush, MapDoc, Symmetry, TreeDoc};
use ferrite_shared::map::ops::{self, Op, Outcome};
use ferrite_shared::map::terrain::{Mode, Stroke};
use ferrite_shared::map::{library, maps};
use ferrite_shared::map::{BlockKind, World, HALF_X, HALF_Z};
use macroquad::prelude as mq;
use mq::KeyCode as K;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

pub enum Exit {
    /// The window is closing.
    Quit,
    /// Back to the start screen, its "new map" panel open if `new`.
    Home { new: bool },
    /// Play the map, installed as this map id, then come back.
    Playtest(u8),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Select,
    Place,
    Objectives,
    Terrain,
}

/// A world built on the worker thread, for the document as of `edits`.
struct Built {
    world: World,
    mesh: Mesh,
    scenery: Mesh,
    sources: Vec<(usize, bool)>,
    edits: u64,
}

/// A brush stroke on the ground, from press to release.
struct Sculpting {
    /// Where it started, and the ground's height there (what FLATTEN
    /// levels to).
    from: [f32; 2],
    height: f32,
    /// When the brush last bit.
    last: f64,
    /// The document before the stroke: one undo step for all of it.
    before: MapDoc,
    /// What it does (the right button works the opposite of the mode), and
    /// the button that holds it.
    mode: Mode,
    button: mq::MouseButton,
}

/// A block or tree being dragged over the ground with the select tool.
struct Moving {
    what: Picked,
    before: MapDoc,
    /// Where the mouse went down, and whether it has moved far enough to
    /// count as a drag (a plain click only selects).
    press: (f32, f32),
    started: bool,
    /// The height of the plane the mouse drags on, and where on it the
    /// thing was grabbed, as seen (its turned copy, if that was clicked).
    plane: f32,
    grab: Vec3,
}

/// Something on the tool panel a click can do.
#[derive(Clone, Copy, PartialEq)]
enum Act {
    Tool(Tool),
    Mode(usize),
    Size(bool),
    Strength(bool),
    Kind,
    Grid,
    Tint,
    Marker,
}

/// A clickable spot on the panel: x, y, width, height.
type Spot = (f32, f32, f32, f32);

/// The terrain modes as their chips on the panel say them.
const MODE_CHIPS: [&str; 6] = ["RAISE", "LOWER", "FLAT", "SMOOTH", "RAMP", "NOISE"];

/// What the right button (or Shift with the left) does in each terrain
/// mode: the opposite, where there is one.
fn opposite(m: Mode) -> Option<Mode> {
    match m {
        Mode::Raise => Some(Mode::Lower),
        Mode::Lower => Some(Mode::Raise),
        Mode::Smooth => Some(Mode::Noise),
        Mode::Noise => Some(Mode::Smooth),
        Mode::Flatten => Some(Mode::Smooth),
        Mode::Ramp => None,
    }
}

/// What the place tool stamps: a kind, its name and a starting size.
const KINDS: [(BlockKind, &str, [f32; 3]); 8] = library::BLOCKS;
const GRIDS: [f32; 4] = [2.0, 1.0, 0.5, 0.25];
/// The tool panel's size, without the selection line.
const PANEL: (f32, f32) = (330.0, 246.0);
/// Brush sizes, and how hard it works (times its usual rate).
const BRUSH: (f32, f32) = (2.0, 60.0);
const STRENGTH: (f32, f32) = (0.25, 4.0);
const MARKERS: [&str; 3] = ["OBJECTIVE A", "OBJECTIVE B", "WEST BASE"];
/// How long after the last edit the world is rebuilt.
const SETTLE: f64 = 0.15;
const SELECT: Col = [1.0, 0.62, 0.25, 1.0];
const GHOST: Col = [0.35, 0.75, 1.0, 1.0];
const COPY: Col = [0.55, 0.45, 0.4, 1.0];

pub struct Editor {
    path: PathBuf,
    doc: MapDoc,
    /// The document as last saved, to tell whether there are changes.
    saved: MapDoc,
    world: World,
    gpu: Option<gfx::Gpu>,
    /// The brush (and whether its turned copy) behind each world block.
    sources: Vec<(usize, bool)>,
    shadows: u32,
    eye: Vec3,
    yaw: f32,
    pitch: f32,
    speed: f32,
    tool: Tool,
    kind: usize,
    /// Footprint of the next placed block, before any turn.
    size: [f32; 3],
    turned: bool,
    tint: u8,
    grid: usize,
    /// The selected brush, and whether it was picked by its turned copy:
    /// moves then go the other way so it follows the mouse.
    sel: Option<(usize, bool)>,
    /// The selected tree, the same way (never with a brush selected).
    tree: Option<(usize, bool)>,
    marker: usize,
    undo: Vec<MapDoc>,
    redo: Vec<MapDoc>,
    /// When the document last changed and the world has not caught up.
    stale: Option<f64>,
    note: Option<(String, f64)>,
    leave_armed: bool,
    /// When unsaved work was last written to the autosave file.
    autosaved: f64,
    looking: bool,
    last_mouse: (f32, f32),
    /// Tools (the MCP server) reading and editing the map; see `forge_link`.
    link: Option<Link>,
    /// A picture a tool asked for, taken on the next frame.
    shot: Option<Shot>,
    /// The library drawer: models to drag in.
    shelf: Shelf,
    /// Capture aid: where the mouse is taken to be.
    pointer: Option<(f32, f32)>,
    /// The name being typed for Save As.
    naming: Option<Field>,
    /// Edits so far, to tell whether a finished build is still current.
    edits: u64,
    /// The world being built on the worker thread.
    building: Option<std::sync::mpsc::Receiver<Result<Built, String>>>,
    /// The terrain brush: mode, radius (m) and strength.
    mode: usize,
    radius: f32,
    strength: f32,
    sculpting: Option<Sculpting>,
    moving: Option<Moving>,
}

/// What a click in the world picked, and whether by its turned copy.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Picked {
    Block(usize, bool),
    Tree(usize, bool),
}

/// A screenshot a tool is waiting for.
struct Shot {
    conn: usize,
    id: Value,
    path: PathBuf,
    /// Where to look from, and at: the editor's own camera if `None`.
    view: Option<(Vec3, Vec3)>,
    hud: bool,
}

/// A name the map format accepts, from a file name.
fn name_from(path: &std::path::Path) -> String {
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_uppercase()).unwrap_or_default();
    let name: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() || c == ' ' { c } else { ' ' }).take(32).collect();
    let name = name.trim().to_string();
    if name.is_empty() {
        "UNTITLED".into()
    } else {
        name
    }
}

impl Editor {
    /// Open a map document, or start a new one if the file does not exist.
    /// `restore` brings back the unsaved work in its autosave.
    pub fn open(path: PathBuf, shadows: u32, restore: bool) -> Result<Editor, String> {
        let saved = match std::fs::read_to_string(&path) {
            Ok(text) => MapDoc::from_json(&text)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => MapDoc::blank(&name_from(&path)),
            Err(e) => return Err(e.to_string()),
        };
        let doc = if restore { maps::read(&maps::autosave_of(&path)).map_err(|e| format!("the autosave: {e}"))? } else { saved.clone() };
        let fresh = !path.exists();
        let mut ed = Editor {
            path,
            saved,
            world: World::with_map(0, 0),
            doc,
            gpu: None,
            sources: Vec::new(),
            shadows,
            // Over the west base, looking east along the field.
            eye: vec3(-150.0, 30.0, -40.0),
            yaw: 0.35,
            pitch: -0.3,
            speed: 24.0,
            tool: Tool::Place,
            kind: 0,
            size: KINDS[0].2,
            turned: false,
            tint: 0,
            grid: 1,
            sel: None,
            tree: None,
            marker: 0,
            undo: Vec::new(),
            redo: Vec::new(),
            stale: None,
            note: None,
            leave_armed: false,
            autosaved: 0.0,
            looking: false,
            last_mouse: (0.0, 0.0),
            link: Link::open(),
            shot: None,
            shelf: Shelf::new(),
            pointer: None,
            naming: None,
            edits: 0,
            building: None,
            mode: 0,
            radius: 10.0,
            strength: 1.0,
            sculpting: None,
            moving: None,
        };
        ed.rebuild();
        if restore {
            ed.say("BROUGHT BACK YOUR UNSAVED WORK - CTRL+S KEEPS IT", 0.0);
        } else if fresh {
            ed.say("NEW MAP - SAVE WITH CTRL+S", 0.0);
        }
        Ok(ed)
    }

    fn say(&mut self, s: &str, now: f64) {
        self.note = Some((s.to_string(), now));
    }

    /// Capture aid: start with brush `j` selected.
    pub fn select(&mut self, j: usize) {
        if j < self.doc.brushes.len() {
            self.choose(Some(Picked::Block(j, false)));
            self.tool = Tool::Select;
        }
    }

    /// Capture aid: open the library at `folder`.
    pub fn show_folder(&mut self, folder: &str) {
        self.shelf.show(folder);
    }

    /// Show a message (a playtest's ending, say) when the editor next runs.
    pub fn tell(&mut self, s: String) {
        self.note = Some((s, 0.0));
    }

    /// Build the world from the document now, on this thread.
    fn rebuild(&mut self) {
        let doc = Arc::new(self.doc.clone());
        doc::preview(doc.clone());
        match doc.build(0xFE221, doc::EDITING) {
            Ok(w) => self.world = w,
            Err(e) => {
                // Edits are checked before they land, so this is a bug.
                self.note = Some((format!("CANNOT BUILD: {e}"), mq::get_time()));
                return;
            }
        }
        self.sources = self.doc.sources();
        if let Some(old) = self.gpu.take() {
            old.destroy();
        }
        self.gpu = Some(crate::world_gpu(&self.world, self.shadows));
        self.stale = None;
        self.building = None;
    }

    /// Start building the world from the document on a worker thread; the
    /// ground mesh takes a good part of a second's frames to make.
    fn start_build(&mut self) {
        let doc = Arc::new(self.doc.clone());
        doc::preview(doc.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        let edits = self.edits;
        let spawned = std::thread::Builder::new().name("forge-build".into()).spawn(move || {
            let built = doc.build(0xFE221, doc::EDITING).map(|world| {
                let mesh = scene::build_world(&world);
                let scenery = scene::build_scenery(&world);
                Built { sources: doc.sources(), world, mesh, scenery, edits }
            });
            let _ = tx.send(built);
        });
        match spawned {
            Ok(_) => self.building = Some(rx),
            Err(_) => self.rebuild(),
        }
    }

    /// Take a world the worker has finished.
    fn finish_build(&mut self) {
        let Some(rx) = &self.building else { return };
        let built = match rx.try_recv() {
            Ok(b) => b,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err("the build stopped".into()),
        };
        self.building = None;
        match built {
            Ok(b) => {
                if let Some(old) = self.gpu.take() {
                    old.destroy();
                }
                self.gpu = Some(crate::world_gpu_from(&b.world, b.mesh, b.scenery, self.shadows));
                self.world = b.world;
                self.sources = b.sources;
                // Edits made while it was building need another.
                if b.edits == self.edits {
                    self.stale = None;
                }
            }
            Err(e) => self.note = Some((format!("CANNOT BUILD: {e}"), mq::get_time())),
        }
    }

    /// The document changed: the world is to be built again.
    fn touch(&mut self, now: f64) {
        self.stale = Some(now);
        self.edits += 1;
    }

    /// Change the document; refused (and undone) if the result is not a
    /// valid map.
    fn edit(&mut self, now: f64, f: impl FnOnce(&mut MapDoc)) {
        let before = self.doc.clone();
        f(&mut self.doc);
        if self.doc == before {
            return;
        }
        if let Err(e) = self.doc.validate() {
            self.doc = before;
            self.say(&e.to_uppercase(), now);
            return;
        }
        self.undo.push(before);
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.touch(now);
        self.leave_armed = false;
    }

    /// Apply a batch of edits as one undo step.
    fn apply(&mut self, now: f64, batch: &[Op]) -> Result<Outcome, String> {
        let before = self.doc.clone();
        let w = &self.world;
        let out = ops::apply(&mut self.doc, batch, &|x, z| w.height(x, z))?;
        if self.doc != before {
            self.undo.push(before);
            if self.undo.len() > 200 {
                self.undo.remove(0);
            }
            self.redo.clear();
            self.touch(now);
            self.leave_armed = false;
            self.keep_selection();
        }
        Ok(out)
    }

    fn step_history(&mut self, now: f64, back: bool, steps: usize) -> usize {
        let mut n = 0;
        for _ in 0..steps {
            let (from, to) = if back { (&mut self.undo, &mut self.redo) } else { (&mut self.redo, &mut self.undo) };
            let Some(d) = from.pop() else { break };
            to.push(std::mem::replace(&mut self.doc, d));
            n += 1;
        }
        if n > 0 {
            self.touch(now);
            self.keep_selection();
        }
        n
    }

    /// Drop a selection the document no longer has.
    fn keep_selection(&mut self) {
        self.sel = self.sel.filter(|s| s.0 < self.doc.brushes.len());
        self.tree = self.tree.filter(|s| s.0 < self.doc.trees.len());
    }

    /// Select what the mouse is over: a block, or a tree.
    fn choose(&mut self, picked: Option<Picked>) {
        (self.sel, self.tree) = match picked {
            Some(Picked::Block(j, copy)) => (Some((j, copy)), None),
            Some(Picked::Tree(j, copy)) => (None, Some((j, copy))),
            None => (None, None),
        };
    }

    /// Where tree `t` stands, or its turned copy.
    fn tree_spot(t: &TreeDoc, copy: bool) -> Vec3 {
        let p = Vec3::from(t.at);
        if copy {
            vec3(-p.x, p.y, -p.z)
        } else {
            p
        }
    }

    /// The nearest tree (or turned copy) the ray `o + d s` passes through
    /// before `limit`.
    fn tree_hit(&self, o: Vec3, d: Vec3, limit: f32) -> Option<(f32, usize, bool)> {
        tree_hit(&self.doc, o, d, limit)
    }

    /// Where a model dropped now would land: its origin on the ground, and
    /// how far it is lifted (onto the block under the mouse).
    fn stamp_spot(&self) -> Option<([f32; 2], f32)> {
        let (o, d) = self.mouse_ray();
        let hit = self.world.raycast(o, d, 800.0, 0)?;
        let p = o + d * hit.t;
        let (x, z) = (self.snap(p.x).clamp(-HALF_X, HALF_X), self.snap(p.z).clamp(-HALF_Z, HALF_Z));
        let on_block = hit.block.is_some() && hit.normal.y > 0.5;
        let lift = if on_block { (p.y - self.world.height(x, z)).max(0.0) } else { 0.0 };
        Some(([x, z], lift))
    }

    /// The boxes a library model would stamp at the mouse now (trees as
    /// their trunks), for the ghost.
    fn stamp_ghost(&self, key: u64) -> Vec<(Vec3, Vec3)> {
        let (Some(e), Some((at, lift))) = (self.shelf.entry(key), self.stamp_spot()) else { return Vec::new() };
        let w = &self.world;
        let (blocks, trees) = ops::stamp(&e.prefab, at, self.shelf.turn, lift, &|x, z| w.height(x, z));
        let mut out: Vec<(Vec3, Vec3)> = blocks.iter().map(|b| (Vec3::from(b.min), Vec3::from(b.max))).collect();
        for t in trees {
            let p = Vec3::from(t.at);
            out.push((p - vec3(t.radius * 0.5, 0.0, t.radius * 0.5), p + vec3(t.radius * 0.5, t.height, t.radius * 0.5)));
        }
        out
    }

    /// Put library model `key` down at the mouse, as one undo step.
    fn place_model(&mut self, now: f64, key: u64) {
        let Some(e) = self.shelf.entry(key) else { return };
        let Some((at, lift)) = self.stamp_spot() else { return };
        let name = e.prefab.name.clone();
        let mut prefab = e.prefab.clone();
        // A lone block takes the current tint, as the place tool's do, and
        // a lone crate can be blown up.
        let lone = prefab.blocks.len() == 1 && prefab.trees.is_empty();
        if lone {
            prefab.blocks[0].tint = self.tint;
        }
        let lone_crate = lone && prefab.blocks[0].kind == BlockKind::Crate;
        match self.apply(now, &[Op::Prefab { prefab, at, quarters: self.shelf.turn, lift }]) {
            Ok(out) => {
                if let [j] = out.added[..] {
                    if lone_crate {
                        let slot = self.free_slot();
                        self.doc.brushes[j].dest = slot;
                    }
                    self.choose(Some(Picked::Block(j, false)));
                }
                let n = out.added.len();
                self.say(&format!("PLACED {name}{}", if n > 1 { format!(" ({n} BLOCKS)") } else { String::new() }), now);
            }
            Err(e) => self.say(&e.to_uppercase(), now),
        }
    }

    /// Look from `eye` at `at`.
    fn look_at(&mut self, eye: Vec3, at: Vec3) {
        let d = (at - eye).normalize_or_zero();
        if d == Vec3::ZERO {
            return;
        }
        self.eye = eye;
        self.yaw = d.z.atan2(d.x);
        self.pitch = d.y.clamp(-1.0, 1.0).asin().clamp(-1.55, 1.55);
    }

    fn status(&self) -> Value {
        json!({
            "ok": true,
            "path": self.path.display().to_string(),
            "name": self.doc.meta.name,
            "dirty": self.dirty(),
            "brushes": self.doc.brushes.len(),
            "blocks": self.world.blocks.len(),
            "undo": self.undo.len(),
            "redo": self.redo.len(),
            "eye": self.eye.to_array(),
            "facing": self.dir().to_array(),
            "selected": self.sel.map(|s| s.0),
            "selected_tree": self.tree.map(|s| s.0),
            "trees": self.doc.trees.len(),
        })
    }

    /// Answer one tool request, or `None` when the answer comes later.
    fn handle(&mut self, body: &Value, now: f64, conn: usize) -> Option<Value> {
        let v3 = |v: &Value| -> Option<Vec3> {
            let a = v.as_array()?;
            Some(vec3(a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32))
        };
        let view = |b: &Value| Some((v3(&b["eye"])?, v3(&b["at"])?));
        let cmd = body["cmd"].as_str().unwrap_or("");
        Some(match cmd {
            "status" => self.status(),
            "doc" => {
                let mut v = self.status();
                v["doc"] = serde_json::to_value(&self.doc).unwrap_or(Value::Null);
                v
            }
            "apply" => {
                let batch: Vec<Op> = match serde_json::from_value(body["ops"].clone()) {
                    Ok(b) => b,
                    Err(e) => return Some(json!({ "ok": false, "error": format!("bad ops: {e}") })),
                };
                match self.apply(now, &batch) {
                    Ok(out) => {
                        if let Some(note) = body["note"].as_str() {
                            self.say(&note.to_uppercase(), now);
                        }
                        json!({ "ok": true, "outcome": out })
                    }
                    Err(e) => json!({ "ok": false, "error": e }),
                }
            }
            "save" => {
                self.save(now);
                json!({ "ok": !self.dirty(), "path": self.path.display().to_string() })
            }
            "undo" | "redo" => {
                let steps = body["steps"].as_u64().unwrap_or(1).min(200) as usize;
                let n = self.step_history(now, cmd == "undo", steps);
                json!({ "ok": true, "steps": n, "brushes": self.doc.brushes.len() })
            }
            "view" => {
                if let Some((eye, at)) = view(body) {
                    self.look_at(eye, at);
                }
                if let Some(j) = body["select"].as_u64() {
                    self.select(j as usize);
                } else if body["select"].is_null() && body.get("select").is_some() {
                    self.choose(None);
                }
                if let Some(note) = body["note"].as_str() {
                    self.say(&note.to_uppercase(), now);
                }
                self.status()
            }
            "shot" => {
                let Some(path) = body["path"].as_str() else {
                    return Some(json!({ "ok": false, "error": "shot needs a path" }));
                };
                let id = body.get("id").cloned().unwrap_or(Value::Null);
                self.shot = Some(Shot { conn, id, path: path.into(), view: view(body), hud: body["hud"].as_bool().unwrap_or(false) });
                return None;
            }
            "bad" => json!({ "ok": false, "error": body["error"] }),
            other => json!({ "ok": false, "error": format!("unknown command {other:?}") }),
        })
    }

    /// Answer whatever tools have asked since the last frame.
    fn serve(&mut self, now: f64) {
        let Some(mut link) = self.link.take() else { return };
        for req in link.poll() {
            let id = req.body.get("id").cloned().unwrap_or(Value::Null);
            if let Some(mut r) = self.handle(&req.body, now, req.conn) {
                r["id"] = id;
                link.reply(req.conn, &r);
            }
        }
        if self.shot.is_none() {
            link.sweep();
        }
        self.link = Some(link);
    }

    /// Take the picture a tool asked for: this frame, from its camera.
    fn take_shot(&mut self, art: &ui::Art, now: f64) {
        let Some(shot) = self.shot.take() else { return };
        if self.stale.is_some() {
            self.rebuild();
        }
        let keep = (self.eye, self.yaw, self.pitch, self.looking);
        if let Some((eye, at)) = shot.view {
            self.look_at(eye, at);
        }
        // No cursor marks or ghosts in the picture.
        self.looking = true;
        self.draw(art, now, shot.hud);
        let img = mq::get_screen_data();
        (self.eye, self.yaw, self.pitch, self.looking) = keep;
        if let Some(dir) = shot.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let saved = std::panic::catch_unwind(|| img.export_png(&shot.path.display().to_string())).is_ok();
        let mut r = if saved {
            json!({ "ok": true, "path": shot.path.display().to_string(), "width": img.width, "height": img.height })
        } else {
            json!({ "ok": false, "error": "could not save the picture" })
        };
        r["id"] = shot.id;
        if let Some(link) = self.link.as_mut() {
            link.reply(shot.conn, &r);
        }
    }

    fn dirty(&self) -> bool {
        self.doc != self.saved
    }

    fn save(&mut self, now: f64) -> bool {
        self.doc.meta.revision += 1;
        match maps::write(&self.path, &self.doc) {
            Ok(()) => {
                self.saved = self.doc.clone();
                // Saved: the autosave is behind it now.
                let _ = std::fs::remove_file(maps::autosave_of(&self.path));
                let msg = format!("SAVED {}", self.path.display());
                self.say(&msg, now);
                true
            }
            Err(e) => {
                self.doc.meta.revision -= 1;
                self.say(&format!("COULD NOT SAVE: {e}"), now);
                false
            }
        }
    }

    /// Save as a new map called `name` in the maps folder, and go on
    /// editing that one.
    fn save_as(&mut self, now: f64, name: &str) {
        let name = maps::clean_name(name);
        if name.is_empty() {
            self.say("A MAP NEEDS A NAME", now);
            return;
        }
        let path = maps::fresh_path(&maps::default_root(), &name);
        let old = (std::mem::replace(&mut self.path, path), std::mem::replace(&mut self.doc.meta.name, name.clone()));
        if self.save(now) {
            // Undo goes back through edits, not the name.
            for d in self.undo.iter_mut().chain(self.redo.iter_mut()) {
                d.meta.name = name.clone();
            }
            self.say(&format!("SAVED AS {name}  -  {}", self.path.display()), now);
        } else {
            (self.path, self.doc.meta.name) = old;
        }
    }

    /// Leave for `exit`, unless there is unsaved work and this is the first
    /// try: then say so. Unsaved work left behind stays in the autosave.
    fn leave(&mut self, now: f64, exit: Exit) -> Option<Exit> {
        if self.dirty() && !self.leave_armed {
            self.leave_armed = true;
            self.say("UNSAVED CHANGES - CTRL+S SAVES, AGAIN TO LEAVE (THEY STAY IN THE AUTOSAVE)", now);
            return None;
        }
        if self.dirty() {
            let _ = std::fs::write(maps::autosave_of(&self.path), self.doc.to_json());
        }
        Some(exit)
    }

    fn dir(&self) -> Vec3 {
        vec3(self.yaw.cos() * self.pitch.cos(), self.pitch.sin(), self.yaw.sin() * self.pitch.cos())
    }

    fn proj(&self) -> Mat4 {
        let aspect = mq::screen_width() / mq::screen_height().max(1.0);
        Mat4::perspective_rh_gl(70f32.to_radians(), aspect, 0.08, 900.0)
    }

    /// The mouse, or where a capture says it is.
    fn cursor(&self) -> (f32, f32) {
        self.pointer.unwrap_or_else(mq::mouse_position)
    }

    /// Capture aid: the terrain brush in mode `name` with the mouse at `at`.
    pub fn brush_preview(&mut self, name: &str, at: (f32, f32)) {
        self.tool = Tool::Terrain;
        self.mode = Mode::ALL.iter().position(|m| m.name().eq_ignore_ascii_case(name)).unwrap_or(0);
        self.pointer = Some(at);
    }

    /// Capture aid: stamp library model `name` with the mouse at `at`.
    pub fn stamp_preview(&mut self, name: &str, at: (f32, f32)) {
        self.shelf.armed = self.shelf.lib.find(name).map(|e| e.key);
        self.pointer = Some(at);
    }

    /// The ray under the mouse.
    fn mouse_ray(&self) -> (Vec3, Vec3) {
        let (mx, my) = self.cursor();
        let (w, h) = (mq::screen_width(), mq::screen_height());
        let inv = (self.proj() * Mat4::look_to_rh(self.eye, self.dir(), Vec3::Y)).inverse();
        let ndc = (2.0 * mx / w - 1.0, 1.0 - 2.0 * my / h);
        let a = inv * vec4(ndc.0, ndc.1, -1.0, 1.0);
        let b = inv * vec4(ndc.0, ndc.1, 1.0, 1.0);
        let (a, b) = (a.truncate() / a.w, b.truncate() / b.w);
        (a, (b - a).normalize())
    }

    fn snap(&self, v: f32) -> f32 {
        let g = GRIDS[self.grid];
        (v / g).round() * g
    }

    /// The footprint of the block the place tool would stamp now.
    fn ghost(&self) -> Option<(Vec3, Vec3)> {
        let (o, d) = self.mouse_ray();
        let hit = self.world.raycast(o, d, 800.0, 0)?;
        let p = o + d * hit.t;
        let [mut sx, sy, mut sz] = self.size;
        if self.turned {
            std::mem::swap(&mut sx, &mut sz);
        }
        // Stand on what was hit, or beside it when its side was hit.
        let c = p + hit.normal * if hit.normal.y > 0.5 { 0.0 } else { sx.max(sz) * 0.5 };
        let (cx, cz) = (self.snap(c.x), self.snap(c.z));
        let on_block = hit.block.is_some() && hit.normal.y > 0.5;
        let (y0, y1) = if on_block {
            (p.y, p.y + sy)
        } else {
            // On the ground: sunk to the lowest corner, as the built maps do.
            let w = &self.world;
            let g = w.height(cx, cz);
            let low = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
                .iter()
                .map(|(a, b)| w.height(cx + a * sx * 0.5, cz + b * sz * 0.5))
                .fold(g, f32::min);
            (low - 0.4, g + sy)
        };
        Some((vec3(cx - sx * 0.5, y0, cz - sz * 0.5), vec3(cx + sx * 0.5, y1, cz + sz * 0.5)))
    }

    fn pick(&self) -> Option<Picked> {
        let (o, d) = self.mouse_ray();
        let hit = self.world.raycast(o, d, 800.0, 0);
        let block = hit.as_ref().and_then(|h| Some((h.t, *self.sources.get(h.block?)?)));
        // A tree counts if it is reached before the ground or a block is
        // (a little past, since the trunk stands in the ground).
        let tree = self.tree_hit(o, d, hit.as_ref().map_or(800.0, |h| h.t + 0.5));
        match (block, tree) {
            (Some((tb, _)), Some((tt, j, copy))) if tt <= tb + 0.5 => Some(Picked::Tree(j, copy)),
            (Some((_, (j, copy))), _) => Some(Picked::Block(j, copy)),
            (None, Some((_, j, copy))) => Some(Picked::Tree(j, copy)),
            (None, None) => None,
        }
    }

    /// Where the mouse meets the ground, inside the battlefield.
    fn ground_at_mouse(&self) -> Option<(f32, f32)> {
        let (o, d) = self.mouse_ray();
        let hit = self.world.raycast(o, d, 800.0, 0)?;
        let p = o + d * hit.t;
        Some((self.snap(p.x).clamp(-HALF_X, HALF_X), self.snap(p.z).clamp(-HALF_Z, HALF_Z)))
    }

    /// Run the editor until the map is played or the editor is left.
    pub async fn run(&mut self, art: &ui::Art, shot: Option<(&str, f64)>) -> Exit {
        mq::set_cursor_grab(false);
        mq::show_mouse(true);
        if self.gpu.is_none() {
            self.rebuild();
        }
        let start = mq::get_time();
        let mut shot_done = false;
        self.last_mouse = mq::mouse_position();
        loop {
            let now = mq::get_time();
            let dt = mq::get_frame_time().min(0.1);
            if let Some(exit) = self.input(now, dt) {
                if let Some(g) = self.gpu.take() {
                    g.destroy();
                }
                mq::set_cursor_grab(false);
                mq::show_mouse(true);
                return exit;
            }
            self.serve(now);
            self.shelf.refresh(now);
            self.shelf.prepare(now, self.cursor());
            self.finish_build();
            let due = self.stale.is_some_and(|t| now - t > SETTLE || self.sculpting.is_some());
            if due && self.building.is_none() {
                self.start_build();
            }
            // Unsaved work goes to FILE.fmap.autosave every minute.
            if self.dirty() && now - self.autosaved > 60.0 {
                self.autosaved = now;
                let _ = std::fs::write(maps::autosave_of(&self.path), self.doc.to_json());
            }
            if self.shot.is_some() {
                self.take_shot(art, now);
            } else {
                self.draw(art, now, true);
            }
            if let Some((path, at)) = shot {
                if !shot_done && now - start >= at {
                    shot_done = true;
                    mq::get_screen_data().export_png(path);
                }
                if now - start >= at + 0.5 {
                    return Exit::Quit;
                }
            }
            mq::next_frame().await;
        }
    }

    fn input(&mut self, now: f64, dt: f32) -> Option<Exit> {
        let ctrl = mq::is_key_down(K::LeftControl) || mq::is_key_down(K::RightControl);
        let shift = mq::is_key_down(K::LeftShift) || mq::is_key_down(K::RightShift);
        let pressed = mq::is_key_pressed;

        // Typing a name for Save As takes every key.
        if let Some(f) = self.naming.as_mut() {
            f.typing(now);
            if pressed(K::Escape) {
                self.naming = None;
            } else if pressed(K::Enter) || pressed(K::KpEnter) {
                let name = f.text.clone();
                self.naming = None;
                self.save_as(now, &name);
            }
            if mq::is_quit_requested() {
                if self.dirty() {
                    let _ = std::fs::write(maps::autosave_of(&self.path), self.doc.to_json());
                }
                return Some(Exit::Quit);
            }
            self.last_mouse = mq::mouse_position();
            return None;
        }

        // Look while the middle button is held, or the right one unless the
        // terrain brush has it (there it works the opposite way).
        let m = mq::mouse_position();
        let right_sculpts = self.tool == Tool::Terrain && opposite(Mode::ALL[self.mode]).is_some();
        let look = mq::is_mouse_button_down(mq::MouseButton::Middle)
            || (mq::is_mouse_button_down(mq::MouseButton::Right) && !right_sculpts && !self.looking_blocked());
        if look != self.looking {
            self.looking = look;
            mq::set_cursor_grab(look);
            mq::show_mouse(!look);
        } else if look {
            let (dx, dy) = ((m.0 - self.last_mouse.0).clamp(-300.0, 300.0), (m.1 - self.last_mouse.1).clamp(-300.0, 300.0));
            self.yaw += dx * 0.0035;
            self.pitch = (self.pitch - dy * 0.0035).clamp(-1.55, 1.55);
        }
        self.last_mouse = m;

        // Fly.
        let wheel = mq::mouse_wheel().1;
        let alt = mq::is_key_down(K::LeftAlt) || mq::is_key_down(K::RightAlt);
        if wheel != 0.0 && !self.shelf.wheel(m.0, m.1, wheel) {
            if self.tool == Tool::Terrain && ctrl {
                self.act(Act::Size(wheel > 0.0));
            } else if self.tool == Tool::Terrain && alt {
                self.act(Act::Strength(wheel > 0.0));
            } else {
                self.speed = (self.speed * if wheel > 0.0 { 1.25 } else { 0.8 }).clamp(3.0, 160.0);
            }
        }
        if !ctrl {
            let fwd = self.dir();
            let flat = vec3(fwd.x, 0.0, fwd.z).normalize_or_zero();
            let right = flat.cross(Vec3::Y);
            let mut v = Vec3::ZERO;
            for (k, d) in [(K::W, fwd), (K::S, -fwd), (K::D, right), (K::A, -right), (K::E, Vec3::Y), (K::Q, -Vec3::Y)] {
                if mq::is_key_down(k) {
                    v += d;
                }
            }
            let fast = if shift { 3.0 } else { 1.0 };
            self.eye += v.normalize_or_zero() * self.speed * fast * dt;
            self.eye = self.eye.clamp(vec3(-260.0, -20.0, -190.0), vec3(260.0, 260.0, 190.0));
        }

        if pressed(K::Escape) {
            if let Some(mv) = self.moving.take() {
                // The drag is taken back.
                if mv.started {
                    self.doc = mv.before;
                    self.touch(now);
                    self.say("MOVE CANCELLED", now);
                }
            } else if let Some(s) = self.sculpting.take() {
                // The stroke is taken back whole.
                self.doc = s.before;
                self.touch(now);
                self.say("STROKE CANCELLED", now);
            } else if self.shelf.cancel() {
                // Stopped carrying or stamping a model.
            } else if self.sel.is_some() || self.tree.is_some() {
                self.choose(None);
            } else {
                return self.leave(now, Exit::Home { new: false });
            }
        }
        if mq::is_quit_requested() {
            if self.dirty() {
                let _ = std::fs::write(maps::autosave_of(&self.path), self.doc.to_json());
            }
            return Some(Exit::Quit);
        }
        if ctrl && pressed(K::O) {
            return self.leave(now, Exit::Home { new: false });
        }
        if ctrl && pressed(K::N) {
            return self.leave(now, Exit::Home { new: true });
        }
        if ctrl && shift && pressed(K::S) {
            self.naming = Some(Field::new(&self.doc.meta.name));
            return None;
        }
        if ctrl && pressed(K::S) {
            self.save(now);
        }
        if ctrl && pressed(K::Z) {
            self.step_history(now, true, 1);
        }
        if ctrl && pressed(K::Y) {
            self.step_history(now, false, 1);
        }
        if pressed(K::F5) {
            return match doc::install(self.doc.clone()) {
                Ok(id) => Some(Exit::Playtest(id)),
                Err(e) => {
                    self.say(&e.to_uppercase(), now);
                    None
                }
            };
        }

        for (k, t) in [(K::Key1, Tool::Select), (K::Key2, Tool::Place), (K::Key3, Tool::Objectives), (K::Key4, Tool::Terrain)] {
            if pressed(k) {
                self.act(Act::Tool(t));
            }
        }
        if self.tool == Tool::Terrain {
            if pressed(K::LeftBracket) || pressed(K::RightBracket) {
                self.act(Act::Size(pressed(K::RightBracket)));
            }
            if pressed(K::Minus) || pressed(K::Equal) {
                self.act(Act::Strength(pressed(K::Equal)));
            }
        }
        if pressed(K::L) && !ctrl {
            self.shelf.open = !self.shelf.open;
        }
        // R turns a model being carried or stamped before anything else.
        let stamping = self.shelf.stamping().is_some();
        if stamping && pressed(K::R) {
            self.shelf.turn = (self.shelf.turn + 1) % 4;
        }
        let turn_key = pressed(K::R) && !stamping;
        if pressed(K::G) {
            self.grid = (self.grid + 1) % GRIDS.len();
        }
        if pressed(K::Tab) {
            let step = if shift { KINDS.len() - 1 } else { 1 };
            match self.tool {
                Tool::Objectives => self.marker = (self.marker + if shift { 2 } else { 1 }) % MARKERS.len(),
                Tool::Terrain => self.mode = (self.mode + if shift { Mode::ALL.len() - 1 } else { 1 }) % Mode::ALL.len(),
                _ => {
                    self.kind = (self.kind + step) % KINDS.len();
                    self.size = KINDS[self.kind].2;
                }
            }
        }

        // The selection: move, size, turn, tint, copy, delete.
        if let Some((j, copy)) = self.sel {
            let g = GRIDS[self.grid];
            // Arrow keys go along the axis nearest where the camera faces.
            let f = self.dir();
            let (ax, az) = if f.x.abs() > f.z.abs() { (f.x.signum(), 0.0) } else { (0.0, f.z.signum()) };
            let fwd = vec3(ax, 0.0, az);
            let right = fwd.cross(Vec3::Y);
            let mut step = Vec3::ZERO;
            for (k, d) in
                [(K::Up, fwd), (K::Down, -fwd), (K::Right, right), (K::Left, -right), (K::PageUp, Vec3::Y), (K::PageDown, -Vec3::Y)]
            {
                if pressed(k) {
                    step += d * g;
                }
            }
            if copy {
                step = vec3(-step.x, step.y, -step.z);
            }
            if step != Vec3::ZERO {
                self.edit(now, |d| {
                    let b = &mut d.brushes[j];
                    if shift {
                        // Grow or shrink from the min corner.
                        for a in 0..3 {
                            b.max[a] = (b.max[a] + step[a].abs() * step[a].signum()).max(b.min[a] + 0.1);
                        }
                    } else {
                        for a in 0..3 {
                            b.min[a] += step[a];
                            b.max[a] += step[a];
                        }
                    }
                });
            }
            if turn_key {
                self.edit(now, |d| {
                    let b = &mut d.brushes[j];
                    let (cx, cz) = ((b.min[0] + b.max[0]) * 0.5, (b.min[2] + b.max[2]) * 0.5);
                    let (hx, hz) = ((b.max[0] - b.min[0]) * 0.5, (b.max[2] - b.min[2]) * 0.5);
                    (b.min[0], b.max[0], b.min[2], b.max[2]) = (cx - hz, cx + hz, cz - hx, cz + hx);
                });
            }
            if pressed(K::T) {
                self.edit(now, |d| d.brushes[j].tint = (d.brushes[j].tint + 1) % 4);
            }
            if ctrl && pressed(K::D) {
                let mut b = self.doc.brushes[j].clone();
                let w = b.max[0] - b.min[0];
                b.min[0] += w + g;
                b.max[0] += w + g;
                let n = self.doc.brushes.len();
                self.edit(now, |d| d.brushes.push(b));
                if self.doc.brushes.len() > n {
                    self.sel = Some((n, false));
                }
            }
            if pressed(K::Delete) || pressed(K::Backspace) {
                self.edit(now, |d| {
                    d.brushes.remove(j);
                });
                self.sel = None;
            }
        } else if let Some((j, copy)) = self.tree {
            self.edit_tree(now, j, copy, shift, ctrl, turn_key);
        } else if self.tool == Tool::Place && turn_key {
            self.turned = !self.turned;
        } else if self.tool == Tool::Place && pressed(K::T) {
            self.tint = (self.tint + 1) % 4;
        }

        // Clicks.
        let (mx, my) = mq::mouse_position();
        let (panel_h, spots) = self.panel(false, (mx, my));
        let over_panel = mx < 16.0 + PANEL.0 && my < 16.0 + panel_h;
        let over_ui = over_panel || self.shelf.over(mx, my);
        if let Some(key) = self.shelf.mouse((mx, my)) {
            // A model dragged out of the library and let go over the world.
            self.place_model(now, key);
        }
        if over_panel && mq::is_mouse_button_pressed(mq::MouseButton::Left) {
            if let Some(&(_, a)) = spots.iter().find(|(r, _)| mx >= r.0 && mx < r.0 + r.2 && my >= r.1 && my < r.1 + r.3) {
                self.act(a);
            }
        }
        self.sculpt(now);
        self.drag(now);
        // The terrain brush's opposite, on the right button.
        let right = mq::is_mouse_button_pressed(mq::MouseButton::Right);
        if self.tool == Tool::Terrain && right && !over_ui && self.shelf.armed.is_none() && self.sculpting.is_none() {
            if let Some(o) = opposite(Mode::ALL[self.mode]) {
                self.start_stroke(now, o, mq::MouseButton::Right);
            }
        }
        if mq::is_mouse_button_pressed(mq::MouseButton::Left) && !over_ui {
            if let Some(key) = self.shelf.armed {
                self.place_model(now, key);
                return None;
            }
            match self.tool {
                Tool::Select => {
                    let picked = self.pick();
                    self.choose(picked);
                    // Hold and drag to move it.
                    self.moving = picked.and_then(|p| self.grab(p, (mx, my)));
                }
                Tool::Place => {
                    if let Some((min, max)) = self.ghost() {
                        let kind = KINDS[self.kind].0;
                        let dest = (kind == BlockKind::Crate).then(|| self.free_slot()).flatten();
                        let n = self.doc.brushes.len();
                        let tint = self.tint;
                        self.edit(now, |d| d.brushes.push(Brush { min: min.to_array(), max: max.to_array(), kind, tint, dest }));
                        if self.doc.brushes.len() > n {
                            self.choose(Some(Picked::Block(n, false)));
                        }
                    }
                }
                Tool::Terrain if self.sculpting.is_none() => {
                    // Shift works the opposite way, for a trackpad.
                    let mode = Mode::ALL[self.mode];
                    let mode = if shift { opposite(mode).unwrap_or(mode) } else { mode };
                    self.start_stroke(now, mode, mq::MouseButton::Left);
                }
                Tool::Terrain => {}
                Tool::Objectives => {
                    if let Some((x, z)) = self.ground_at_mouse() {
                        let half = self.doc.symmetry == Symmetry::HalfTurn;
                        // A click on the east half moves the west original.
                        let p = if half && x > 0.0 { [-x, -z] } else { [x, z] };
                        let k = self.marker;
                        self.edit(now, |d| match k {
                            0 | 1 => d.points[k] = p,
                            _ => d.bases[0] = p,
                        });
                    }
                }
            }
        }
        None
    }

    /// Begin a brush stroke in `mode` at the mouse, held by `button`.
    fn start_stroke(&mut self, now: f64, mode: Mode, button: mq::MouseButton) {
        if let Some(p) = self.brush_at() {
            let height = self.world.height(p[0], p[1]);
            self.sculpting = Some(Sculpting { from: p, height, last: now, before: self.doc.clone(), mode, button });
        }
    }

    /// While the terrain brush has the right button, it does not look.
    fn looking_blocked(&self) -> bool {
        self.sculpting.as_ref().is_some_and(|s| s.button == mq::MouseButton::Right)
    }

    /// Do what a panel click (or its key) asks.
    fn act(&mut self, a: Act) {
        let grow = |v: f32, up: bool, (lo, hi): (f32, f32)| (v * if up { 1.25 } else { 0.8 }).clamp(lo, hi);
        match a {
            Act::Tool(t) => {
                self.tool = t;
                self.shelf.armed = None;
                self.moving = None;
                if t == Tool::Terrain {
                    self.choose(None);
                }
            }
            Act::Mode(i) => self.mode = i.min(Mode::ALL.len() - 1),
            Act::Size(up) => self.radius = grow(self.radius, up, BRUSH).round().max(BRUSH.0),
            Act::Strength(up) => self.strength = grow(self.strength, up, STRENGTH),
            Act::Kind => {
                self.kind = (self.kind + 1) % KINDS.len();
                self.size = KINDS[self.kind].2;
            }
            Act::Grid => self.grid = (self.grid + 1) % GRIDS.len(),
            Act::Tint => self.tint = (self.tint + 1) % 4,
            Act::Marker => self.marker = (self.marker + 1) % MARKERS.len(),
        }
    }

    /// Where a block (its foot) or a tree stands, as seen: its turned copy's
    /// spot if that is what was picked.
    fn seen_at(&self, what: Picked) -> Option<Vec3> {
        self.seen_at_in(&self.doc, what)
    }

    /// Where the mouse ray meets the level plane at height `y`.
    fn on_plane(&self, y: f32) -> Option<Vec3> {
        let (o, d) = self.mouse_ray();
        if d.y.abs() < 1e-4 {
            return None;
        }
        let s = (y - o.y) / d.y;
        (s > 0.0 && s < 900.0).then(|| o + d * s)
    }

    /// Take hold of what was clicked, ready to drag it.
    fn grab(&self, what: Picked, press: (f32, f32)) -> Option<Moving> {
        let at = self.seen_at(what)?;
        let hit = self.on_plane(at.y)?;
        Some(Moving { what, before: self.doc.clone(), press, started: false, plane: at.y, grab: vec3(at.x - hit.x, 0.0, at.z - hit.z) })
    }

    /// Drag the held block or tree over the ground: it keeps its height
    /// over the ground (if it stands on it) and moves by whole grid steps.
    /// Letting go makes the whole drag one undo step.
    fn drag(&mut self, now: f64) {
        let Some(mv) = &self.moving else { return };
        let held = mq::is_mouse_button_down(mq::MouseButton::Left);
        let (mx, my) = self.cursor();
        if !mv.started && held && ((mx - mv.press.0).abs() + (my - mv.press.1).abs()) > 6.0 {
            if let Some(m) = &mut self.moving {
                m.started = true;
            }
        }
        let Some(mv) = &self.moving else { return };
        if mv.started && held {
            let (what, plane, grab) = (mv.what, mv.plane, mv.grab);
            let before = mv.before.clone();
            if let (Some(hit), Some(start)) = (self.on_plane(plane), self.seen_at_in(&before, what)) {
                let want = hit + grab;
                let g = GRIDS[self.grid];
                let snap = |v: f32| (v / g).round() * g;
                let (mut dx, mut dz) = (snap(want.x - start.x), snap(want.z - start.z));
                let copy = matches!(what, Picked::Block(_, true) | Picked::Tree(_, true));
                if copy {
                    (dx, dz) = (-dx, -dz);
                }
                let mut d = before;
                let w = &self.world;
                match what {
                    Picked::Block(j, _) => {
                        let b = &mut d.brushes[j];
                        let (cx, cz) = ((b.min[0] + b.max[0]) * 0.5, (b.min[2] + b.max[2]) * 0.5);
                        let (nx, nz) = ((cx + dx).clamp(-HALF_X, HALF_X), (cz + dz).clamp(-HALF_Z, HALF_Z));
                        let (dx, dz) = (nx - cx, nz - cz);
                        // A block standing on the ground follows it.
                        let footed = b.min[1] < w.height(cx, cz) + 0.6;
                        let dy = if footed { w.height(nx, nz) - w.height(cx, cz) } else { 0.0 };
                        for (a, v) in [(0, dx), (1, dy), (2, dz)] {
                            b.min[a] += v;
                            b.max[a] += v;
                        }
                    }
                    Picked::Tree(j, _) => {
                        let t = &mut d.trees[j];
                        let over = t.at[1] - w.height(t.at[0], t.at[2]);
                        let (nx, nz) = ((t.at[0] + dx).clamp(-HALF_X, HALF_X), (t.at[2] + dz).clamp(-HALF_Z, HALF_Z));
                        t.at = [nx, w.height(nx, nz) + over, nz];
                    }
                }
                if d != self.doc {
                    self.doc = d;
                    self.touch(now);
                }
            }
            return;
        }
        if held {
            return;
        }
        // Let go.
        let Some(mv) = self.moving.take() else { return };
        if !mv.started || self.doc == mv.before {
            return;
        }
        if let Err(e) = self.doc.validate() {
            self.doc = mv.before;
            self.touch(now);
            self.say(&e.to_uppercase(), now);
            return;
        }
        self.undo.push(mv.before);
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.leave_armed = false;
    }

    /// `seen_at`, in another copy of the document.
    fn seen_at_in(&self, d: &MapDoc, what: Picked) -> Option<Vec3> {
        let flip = |p: Vec3, copy: bool| if copy { vec3(-p.x, p.y, -p.z) } else { p };
        match what {
            Picked::Block(j, copy) => {
                let b = d.brushes.get(j)?;
                Some(flip(vec3((b.min[0] + b.max[0]) * 0.5, b.min[1], (b.min[2] + b.max[2]) * 0.5), copy))
            }
            Picked::Tree(j, copy) => Some(Self::tree_spot(d.trees.get(j)?, copy)),
        }
    }

    /// Where the terrain brush is: the spot under the mouse.
    fn brush_at(&self) -> Option<[f32; 2]> {
        let (o, d) = self.mouse_ray();
        let hit = self.world.raycast(o, d, 800.0, 0)?;
        let p = o + d * hit.t;
        Some([p.x.clamp(-HALF_X, HALF_X), p.z.clamp(-HALF_Z, HALF_Z)])
    }

    /// Work the terrain brush while the button is held; end the stroke when
    /// it is let go.
    fn sculpt(&mut self, now: f64) {
        let Some(s) = &self.sculpting else { return };
        let (from, height, since, mode) = (s.from, s.height, now - s.last, s.mode);
        let held = mq::is_mouse_button_down(s.button);
        let at = self.brush_at();
        let k = self.strength;
        // Most modes bite about 30 times a second; a ramp once, at the end.
        let stroke = match (mode, held, at) {
            (Mode::Ramp, false, Some(p)) => Some(Stroke { to: Some(p), ..Stroke::new(Mode::Ramp, from, (self.radius * 0.5).max(1.0)) }),
            (Mode::Ramp, _, _) | (_, _, None) => None,
            (_, _, Some(p)) if since >= 1.0 / 30.0 => {
                let dt = since.min(0.1) as f32;
                let mut st = Stroke::new(mode, p, self.radius);
                st.amount = Some(match mode {
                    Mode::Raise | Mode::Lower => 4.0 * k * dt,
                    Mode::Flatten => (5.0 * k * dt).min(1.0),
                    Mode::Smooth => (4.0 * k * dt).min(1.0),
                    _ => 1.5 * k * dt,
                });
                st.height = Some(height);
                st.scale = Some((self.radius * 0.4).clamp(2.0, 80.0));
                st.seed = (from[0].to_bits() ^ from[1].to_bits().rotate_left(16)) & 0xffff;
                Some(st)
            }
            _ => None,
        };
        if let Some(st) = stroke {
            if let Some(s) = &mut self.sculpting {
                s.last = now;
            }
            let w = &self.world;
            match ops::apply(&mut self.doc, &[Op::Sculpt(st)], &|x, z| w.height(x, z)) {
                Ok(out) if out.ground > 0 => self.touch(now),
                Ok(_) => {}
                Err(e) => {
                    self.say(&e.to_uppercase(), now);
                }
            }
        }
        if held {
            return;
        }
        // Let go: the whole stroke is one undo step.
        let Some(s) = self.sculpting.take() else { return };
        if self.doc == s.before {
            return;
        }
        let blocks = s.before.brushes.iter().zip(&self.doc.brushes).filter(|(a, b)| a.min[1] != b.min[1]).count();
        let trees = s.before.trees.iter().zip(&self.doc.trees).filter(|(a, b)| a.at[1] != b.at[1]).count();
        self.undo.push(s.before);
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.leave_armed = false;
        if blocks + trees > 0 {
            self.say(&format!("{blocks} BLOCKS AND {trees} TREES MOVED WITH THE GROUND"), now);
        }
    }

    /// The brush's ring on the ground (and its turned twin), or a ramp's
    /// line while it is dragged out.
    fn draw_brush(&self, m: &mut Mesh) {
        let Some(p) = self.brush_at() else { return };
        let mode = Mode::ALL[self.mode];
        let half = self.doc.symmetry == Symmetry::HalfTurn;
        let w = &self.world;
        let on = |x: f32, z: f32| vec3(x, w.height(x, z) + 0.2, z);
        let bar = |m: &mut Mesh, a: Vec3, b: Vec3, col: Col| {
            let t = 0.09;
            let n = ((b - a).length() / 1.5).ceil().max(1.0) as usize;
            for i in 0..n {
                let (u, v) = (a.lerp(b, i as f32 / n as f32), a.lerp(b, (i + 1) as f32 / n as f32));
                m.cuboid(u.min(v) - Vec3::splat(t), u.max(v) + Vec3::splat(t), col);
            }
        };
        let ring = |m: &mut Mesh, c: [f32; 2], r: f32, col: Col| {
            let n = 48;
            for i in 0..n {
                let (a0, a1) = (i as f32 / n as f32 * std::f32::consts::TAU, (i + 1) as f32 / n as f32 * std::f32::consts::TAU);
                bar(m, on(c[0] + r * a0.cos(), c[1] + r * a0.sin()), on(c[0] + r * a1.cos(), c[1] + r * a1.sin()), col);
            }
        };
        let col: Col = match mode {
            Mode::Lower => [1.0, 0.55, 0.35, 1.0],
            Mode::Raise => [0.45, 0.95, 0.6, 1.0],
            _ => GHOST,
        };
        match (&self.sculpting, mode) {
            (Some(s), Mode::Ramp) => {
                let r = (self.radius * 0.5).max(1.0);
                for (a, b, c) in [(s.from, p, col), ([-s.from[0], -s.from[1]], [-p[0], -p[1]], COPY)] {
                    if c == COPY && !half {
                        continue;
                    }
                    bar(m, on(a[0], a[1]), on(b[0], b[1]), c);
                    ring(m, a, r, c);
                    ring(m, b, r, c);
                }
            }
            _ => {
                let r = if mode == Mode::Ramp { (self.radius * 0.5).max(1.0) } else { self.radius };
                ring(m, p, r, col);
                if mode != Mode::Ramp {
                    ring(m, p, r * 0.5, col);
                }
                if half {
                    ring(m, [-p[0], -p[1]], r, COPY);
                }
            }
        }
    }

    /// Keys on a selected tree: arrows move it over the ground (PgUp/PgDn
    /// lift it), Shift+Up/Down make it taller or shorter, Shift+Left/Right
    /// its crown narrower or wider, R swaps pine and broadleaf, T its
    /// colour, Ctrl+D a copy beside it, Delete removes it.
    fn edit_tree(&mut self, now: f64, j: usize, copy: bool, shift: bool, ctrl: bool, turn_key: bool) {
        let pressed = mq::is_key_pressed;
        let g = GRIDS[self.grid];
        let f = self.dir();
        let (ax, az) = if f.x.abs() > f.z.abs() { (f.x.signum(), 0.0) } else { (0.0, f.z.signum()) };
        let fwd = vec3(ax, 0.0, az);
        let right = fwd.cross(Vec3::Y);
        let keys = [(K::Up, fwd), (K::Down, -fwd), (K::Right, right), (K::Left, -right), (K::PageUp, Vec3::Y), (K::PageDown, -Vec3::Y)];
        if shift {
            let (mut tall, mut wide) = (0.0, 0.0);
            for (k, dt, dw) in [(K::Up, 1.0, 0.0), (K::Down, -1.0, 0.0), (K::Right, 0.0, 0.5), (K::Left, 0.0, -0.5)] {
                if pressed(k) {
                    tall += dt * g.max(0.5);
                    wide += dw * g.max(0.5);
                }
            }
            if tall != 0.0 || wide != 0.0 {
                self.edit(now, |d| {
                    let t = &mut d.trees[j];
                    t.height = (t.height + tall).clamp(1.0, 60.0);
                    t.radius = (t.radius + wide).clamp(0.2, 12.0);
                });
            }
        } else {
            let mut step = Vec3::ZERO;
            for (k, d) in keys {
                if pressed(k) {
                    step += d * g;
                }
            }
            if copy {
                step = vec3(-step.x, step.y, -step.z);
            }
            if step != Vec3::ZERO {
                let w = &self.world;
                let t = &self.doc.trees[j];
                // It keeps its height over the ground as it moves.
                let over = t.at[1] - w.height(t.at[0], t.at[2]);
                let (x, z) = ((t.at[0] + step.x).clamp(-HALF_X, HALF_X), (t.at[2] + step.z).clamp(-HALF_Z, HALF_Z));
                let y = w.height(x, z) + over + step.y;
                self.edit(now, |d| d.trees[j].at = [x, y, z]);
            }
        }
        if turn_key {
            self.edit(now, |d| d.trees[j].pine = !d.trees[j].pine);
        }
        if pressed(K::T) {
            self.edit(now, |d| d.trees[j].tint = (d.trees[j].tint + 1) % 5);
        }
        if ctrl && pressed(K::D) {
            let mut t = self.doc.trees[j].clone();
            let gap = t.radius * 2.0 + g;
            t.at[0] = (t.at[0] + gap).clamp(-HALF_X, HALF_X);
            t.at[1] = self.world.height(t.at[0], t.at[2]);
            let n = self.doc.trees.len();
            self.edit(now, |d| d.trees.push(t));
            if self.doc.trees.len() > n {
                self.choose(Some(Picked::Tree(n, false)));
            }
        }
        if pressed(K::Delete) || pressed(K::Backspace) {
            self.edit(now, |d| {
                d.trees.remove(j);
            });
            self.choose(None);
        }
    }

    /// The tool panel, laid out: its height and the spots a click works on.
    /// Draws it too when `draw` is set (over a plate already drawn).
    fn panel(&self, draw: bool, mouse: (f32, f32)) -> (f32, Vec<(Spot, Act)>) {
        use ui::color as th;
        let (x, y, pw) = (16.0, 16.0, PANEL.0);
        let mut spots: Vec<(Spot, Act)> = Vec::new();
        let hot = |r: Spot| mouse.0 >= r.0 && mouse.0 < r.0 + r.2 && mouse.1 >= r.1 && mouse.1 < r.1 + r.3;
        let hover = |r: Spot| {
            if draw && hot(r) {
                mq::draw_rectangle(r.0, r.1, r.2, r.3, ui::fade(th::WHITE, 0.08));
            }
        };
        // A small square button with a sign on it.
        let button = |r: Spot, sign: &str| {
            if draw {
                ui::plate(
                    r.0,
                    r.1,
                    r.2,
                    r.3,
                    if hot(r) { th::PANEL_RAISED } else { th::PANEL },
                    if hot(r) { th::COBALT } else { th::STROKE },
                );
                ui::text_c(sign, r.0 + r.2 * 0.5, r.1 + r.3 - 6.0, 18.0, th::WHITE);
            }
        };
        if draw {
            let star = if self.dirty() { " *" } else { "" };
            ui::text(&format!("FORGE  {}{star}", self.doc.meta.name), x + 14.0, y + 30.0, 22.0, th::WHITE);
            if self.link.as_ref().is_some_and(|l| l.linked()) {
                ui::text("LINKED", x + pw - 76.0, y + 30.0, 16.0, th::EMBER);
            }
        }
        let mut ty = y + 62.0;
        let tools = [(Tool::Select, "SELECT"), (Tool::Place, "PLACE"), (Tool::Objectives, "OBJECTIVES"), (Tool::Terrain, "TERRAIN")];
        for (k, (t, name)) in tools.iter().enumerate() {
            let r = (x + 6.0, ty - 19.0, pw - 12.0, 24.0);
            hover(r);
            if draw {
                let on = self.tool == *t;
                ui::text(&format!("{}  {name}", k + 1), x + 14.0, ty, 18.0, if on { th::EMBER } else { th::MUTED });
            }
            spots.push((r, Act::Tool(*t)));
            ty += 24.0;
        }
        ty += 8.0;
        match self.tool {
            Tool::Terrain => {
                // The modes as chips, then size and strength with - and +.
                let n = Mode::ALL.len() as f32;
                let cw = (pw - 28.0 - (n - 1.0) * 4.0) / n;
                for (i, chip) in MODE_CHIPS.iter().enumerate() {
                    let r = (x + 14.0 + i as f32 * (cw + 4.0), ty - 18.0, cw, 26.0);
                    if draw {
                        let on = i == self.mode;
                        let fill = if on {
                            th::EMBER
                        } else if hot(r) {
                            th::PANEL_RAISED
                        } else {
                            th::PANEL
                        };
                        ui::plate(r.0, r.1, r.2, r.3, fill, if on { th::EMBER } else { th::STROKE });
                        ui::text_c(chip, r.0 + r.2 * 0.5, r.1 + 18.0, 12.0, if on { th::INK } else { th::WHITE });
                    }
                    spots.push((r, Act::Mode(i)));
                }
                ty += 34.0;
                let rows = [
                    ("SIZE", "[ ]  CTRL+WHEEL", format!("{:.0} m", self.radius), Act::Size(false), Act::Size(true)),
                    ("STRENGTH", "- =  ALT+WHEEL", format!("x{:.2}", self.strength), Act::Strength(false), Act::Strength(true)),
                ];
                for (label, keys, value, down, up) in rows {
                    if draw {
                        ui::text(label, x + 14.0, ty, 16.0, th::MUTED);
                        ui::text(&value, x + 112.0, ty, 18.0, th::WHITE);
                        ui::text(keys, x + 112.0 + 62.0, ty, 12.0, th::QUIET);
                    }
                    for (k, (act, sign)) in [(down, "-"), (up, "+")].into_iter().enumerate() {
                        let r = (x + pw - 72.0 + k as f32 * 30.0, ty - 19.0, 26.0, 24.0);
                        button(r, sign);
                        spots.push((r, act));
                    }
                    ty += 28.0;
                }
                if draw {
                    let m = Mode::ALL[self.mode];
                    let line = match opposite(m) {
                        Some(o) => format!("LEFT MOUSE {}   RIGHT MOUSE {}", m.name(), o.name()),
                        None => "DRAG FROM THE FOOT TO THE TOP".to_string(),
                    };
                    ui::text(&line, x + 14.0, ty, 14.0, th::QUIET);
                }
                ty += 24.0;
            }
            Tool::Objectives => {
                let r = (x + 6.0, ty - 19.0, pw - 12.0, 24.0);
                hover(r);
                if draw {
                    ui::text(&format!("TAB  {}", MARKERS[self.marker]), x + 14.0, ty, 18.0, th::WHITE);
                }
                spots.push((r, Act::Marker));
                ty += 24.0;
            }
            _ => {
                let r = (x + 6.0, ty - 19.0, pw - 12.0, 24.0);
                hover(r);
                if draw {
                    let [mut a, b, mut c] = self.size;
                    if self.turned {
                        std::mem::swap(&mut a, &mut c);
                    }
                    ui::text(&format!("TAB  {}  {a:.1} x {c:.1} x {b:.1} m", KINDS[self.kind].1), x + 14.0, ty, 18.0, th::WHITE);
                }
                spots.push((r, Act::Kind));
                ty += 24.0;
            }
        }
        if self.tool != Tool::Terrain {
            let half = (pw - 12.0) * 0.5;
            for (k, (act, text)) in [(Act::Grid, format!("G  GRID {} m", GRIDS[self.grid])), (Act::Tint, format!("T  TINT {}", self.tint))]
                .into_iter()
                .enumerate()
            {
                let r = (x + 6.0 + k as f32 * half, ty - 19.0, half, 24.0);
                hover(r);
                if draw {
                    ui::text(&text, r.0 + 8.0, ty, 18.0, th::MUTED);
                }
                spots.push((r, act));
            }
            ty += 24.0;
        }
        if draw {
            ui::text(&format!("{} / {} BLOCKS", self.world.blocks.len(), doc::MAX_BLOCKS), x + 14.0, ty, 18.0, th::MUTED);
        }
        ty += 24.0;
        let selected = self
            .sel
            .and_then(|(j, _)| self.doc.brushes.get(j))
            .map(|b| {
                let sz = [b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]];
                format!("SELECTED {:?}  {:.2} x {:.2} x {:.2}", b.kind, sz[0], sz[2], sz[1]).to_uppercase()
            })
            .or_else(|| {
                self.tree.and_then(|(j, _)| self.doc.trees.get(j)).map(|t| {
                    let kind = if t.pine { "PINE" } else { "BROADLEAF" };
                    format!("SELECTED {kind}  {:.1} m TALL  {:.1} m CROWN", t.height, t.radius)
                })
            });
        if let Some(line) = selected {
            if draw {
                ui::text(&line, x + 14.0, ty, 16.0, th::EMBER);
            }
            ty += 24.0;
        }
        (ty - y + 8.0, spots)
    }

    /// A destruction slot no crate uses yet.
    fn free_slot(&self) -> Option<u8> {
        let max = if self.doc.symmetry == Symmetry::HalfTurn { 32 } else { 64 };
        (0..max).find(|s| self.doc.brushes.iter().all(|b| b.dest != Some(*s)))
    }

    fn draw(&mut self, art: &ui::Art, now: f64, hud: bool) {
        let (sw, sh) = (mq::screen_width(), mq::screen_height());
        let dir = self.dir();
        let proj = self.proj();
        let vp = proj * Mat4::look_to_rh(self.eye, dir, Vec3::Y);
        let sky_vp = proj * Mat4::look_to_rh(Vec3::ZERO, dir, Vec3::Y);
        let (mut opaque, mut alpha, mut add) = (Mesh::default(), Mesh::default(), Mesh::default());
        let w = &self.world;
        scene::crates(&mut opaque, w, 0);
        let owners = [(ferrite_shared::NEUTRAL, 0.0, false); 5];
        scene::beacons(&mut opaque, &mut alpha, &mut add, w, &owners, now as f32, 0);
        // The bases: a post at each.
        for (k, b) in w.bases.iter().enumerate() {
            let c: Col = if k == 0 { [0.35, 0.6, 1.0, 1.0] } else { [1.0, 0.45, 0.3, 1.0] };
            add.cuboid(*b - vec3(0.3, 0.0, 0.3), *b + vec3(0.3, 14.0, 0.3), c);
        }
        // The selection, and its turned copy dimmer.
        if let Some((j, _)) = self.sel {
            for (i, &(src, copy)) in self.sources.iter().enumerate() {
                if src == j && self.stale.is_none() {
                    let b = &w.blocks[i];
                    outline(&mut opaque, b.min, b.max, if copy { COPY } else { SELECT });
                }
            }
            if self.stale.is_some() {
                // The world has not caught up: show where it is going.
                if let Some(b) = self.doc.brushes.get(j) {
                    outline(&mut opaque, Vec3::from(b.min), Vec3::from(b.max), SELECT);
                }
            }
        }
        if let Some((j, _)) = self.tree {
            if let Some(t) = self.doc.trees.get(j) {
                let half = self.doc.symmetry == Symmetry::HalfTurn;
                for copy in [false, true] {
                    if copy && !half {
                        continue;
                    }
                    let p = Self::tree_spot(t, copy);
                    let r = t.radius.max(0.6);
                    outline(&mut opaque, p - vec3(r, 0.0, r), p + vec3(r, t.height, r), if copy { COPY } else { SELECT });
                }
            }
        }
        let (mx, my) = self.cursor();
        let over_panel = mx < 16.0 + PANEL.0 && my < 16.0 + self.panel(false, (mx, my)).0;
        let in_world = !self.looking && !self.shelf.over(mx, my) && !over_panel;
        // What a click would select, outlined faintly.
        if self.tool == Tool::Select && in_world && self.moving.is_none() && self.shelf.armed.is_none() {
            let faint: Col = [0.8, 0.85, 0.95, 1.0];
            match self.pick() {
                Some(Picked::Block(j, _)) if self.sel.map(|s| s.0) != Some(j) && self.stale.is_none() => {
                    for (i, &(src, _)) in self.sources.iter().enumerate() {
                        if src == j {
                            let b = &w.blocks[i];
                            outline(&mut opaque, b.min, b.max, faint);
                        }
                    }
                }
                Some(Picked::Tree(j, copy)) if self.tree.map(|s| s.0) != Some(j) => {
                    if let Some(t) = self.doc.trees.get(j) {
                        let p = Self::tree_spot(t, copy);
                        let r = t.radius.max(0.6);
                        outline(&mut opaque, p - vec3(r, 0.0, r), p + vec3(r, t.height, r), faint);
                    }
                }
                _ => {}
            }
        }
        if let Some(key) = self.shelf.stamping().filter(|_| in_world) {
            let half = self.doc.symmetry == Symmetry::HalfTurn;
            for (min, max) in self.stamp_ghost(key) {
                outline(&mut opaque, min, max, GHOST);
                if half {
                    let t = |p: Vec3| vec3(-p.x, p.y, -p.z);
                    outline(&mut opaque, t(max).min(t(min)), t(max).max(t(min)), COPY);
                }
            }
        } else if self.tool == Tool::Terrain && in_world {
            self.draw_brush(&mut opaque);
        } else if self.tool == Tool::Place && !self.looking {
            if let Some((min, max)) = self.ghost() {
                outline(&mut opaque, min, max, GHOST);
                if self.doc.symmetry == Symmetry::HalfTurn {
                    let t = |p: Vec3| vec3(-p.x, p.y, -p.z);
                    outline(&mut opaque, t(max).min(t(min)), t(max).max(t(min)), COPY);
                }
            }
        }
        let map = w.map;
        let empty = Mesh::default();
        let frame = gfx::Frame {
            vp,
            sky_vp,
            cam: self.eye,
            sun: scene::sun_for(map).normalize(),
            fog: scene::fog_for(map),
            tone: scene::tone_for(map),
            time: now as f32,
            lights: [gfx::Light::default(); 4],
            opaque: &opaque,
            alpha: &alpha,
            add: &add,
            view_vp: Mat4::IDENTITY,
            view_sun: Vec3::Y,
            view_lit: 0.0,
            view_opaque: &empty,
            view_add: &empty,
        };
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.render(&frame);
        }
        if hud {
            self.hud(art, sw, sh, now);
        }
    }

    fn hud(&self, _art: &ui::Art, sw: f32, sh: f32, now: f64) {
        use ui::color as th;
        let picked = self.sel.is_some() || self.tree.is_some();
        let mouse = self.cursor();
        let (ph, _) = self.panel(false, mouse);
        ui::plate(16.0, 16.0, PANEL.0, ph, th::GLASS, th::STROKE);
        self.panel(true, mouse);
        // Keys along the bottom.
        let help = match (self.tool, picked) {
            _ if self.tree.is_some() => {
                "ARROWS MOVE   PGUP/PGDN RAISE   SHIFT+UP/DOWN TALLER   SHIFT+LEFT/RIGHT CROWN   R PINE   T COLOUR   CTRL+D COPY   DEL REMOVE"
            }
            (_, true) => "DRAG OR ARROWS MOVE   PGUP/PGDN RAISE   SHIFT+ARROWS SIZE   R TURN   T TINT   CTRL+D COPY   DEL REMOVE   ESC DESELECT",
            (Tool::Select, false) => "CLICK A BLOCK OR A TREE TO SELECT IT   DRAG IT TO MOVE IT",
            (Tool::Place, false) => "CLICK TO PLACE   TAB KIND   R TURN   T TINT",
            (Tool::Objectives, false) => "CLICK THE GROUND TO MOVE IT   TAB CHOOSE",
            (Tool::Terrain, false) if Mode::ALL[self.mode] == Mode::Ramp => {
                "DRAG FROM THE FOOT OF THE RAMP TO ITS TOP   TAB MODE   [ ] WIDTH   ESC CANCELS"
            }
            (Tool::Terrain, false) => {
                "LEFT DRAG SCULPTS   RIGHT DRAG (OR SHIFT) THE OPPOSITE   TAB MODE   CTRL+WHEEL SIZE   ALT+WHEEL STRENGTH   ESC TAKES IT BACK"
            }
        };
        mq::draw_rectangle(0.0, sh - 64.0, sw, 64.0, th::GLASS);
        ui::text(help, 16.0, sh - 38.0, 17.0, th::WHITE);
        ui::text(
            if self.tool == Tool::Terrain {
                "MIDDLE MOUSE LOOK   WASD+Q/E FLY   WHEEL SPEED   L LIBRARY   CTRL+Z/Y UNDO   CTRL+S SAVE (+SHIFT AS)   F5 PLAY   ESC MAPS"
            } else {
                "RIGHT OR MIDDLE MOUSE LOOK   WASD+Q/E FLY   WHEEL SPEED   L LIBRARY   CTRL+Z/Y UNDO   CTRL+S SAVE (+SHIFT AS)   F5 PLAY   ESC MAPS"
            },
            16.0,
            sh - 14.0,
            17.0,
            th::MUTED,
        );
        if let Some((msg, at)) = &self.note {
            if *at == 0.0 || now - at < 4.0 {
                // Centred in the room between the tool panel and the drawer.
                let (l, r) = (16.0 + PANEL.0 + 16.0, self.shelf.left().unwrap_or(sw) - 16.0);
                let (cx, room) = if r - l > 300.0 { ((l + r) / 2.0, r - l) } else { (sw / 2.0, sw - 32.0) };
                let size = if ui::width(msg, 22.0) <= room { 22.0 } else { 17.0 };
                let w = ui::width(msg, size).min(room) + 28.0;
                ui::plate(cx - w / 2.0, 22.0, w, 34.0, th::GLASS_STRONG, th::STROKE);
                ui::text_c(&crate::forge_library::fit(msg, size, room), cx, 46.0, size, th::WHITE);
            }
        }
        if let Some(f) = &self.naming {
            let w = (sw - 40.0).min(560.0);
            let (x, y) = ((sw - w) / 2.0, sh * 0.32);
            mq::draw_rectangle(0.0, 0.0, sw, sh, ui::fade(th::INK, 0.55));
            ui::plate(x, y, w, 150.0, th::PANEL, th::STROKE);
            ui::text("SAVE AS", x + 20.0, y + 36.0, 22.0, th::WHITE);
            f.draw(x + 20.0, y + 52.0, w - 40.0, "NAME", now);
            ui::text("ENTER SAVES A NEW MAP IN THE MAPS FOLDER   ESC CANCELS", x + 20.0, y + 126.0, 14.0, th::QUIET);
            return;
        }
        let (mx, my) = self.cursor();
        self.shelf.draw((mx, my));
        let carrying = matches!(self.shelf.drag, Drag::Carry(_));
        if !self.looking && !self.shelf.over(mx, my) && !carrying {
            // A crosshair on the mouse, over the world.
            mq::draw_circle_lines(mx, my, 4.0, 1.5, th::WHITE);
        }
    }
}

/// The nearest tree (or turned copy) the ray `o + d s` passes through
/// before `limit`: its trunk and crown taken as one upright cylinder.
fn tree_hit(doc: &MapDoc, o: Vec3, d: Vec3, limit: f32) -> Option<(f32, usize, bool)> {
    let half = doc.symmetry == Symmetry::HalfTurn;
    let mut best: Option<(f32, usize, bool)> = None;
    for (j, t) in doc.trees.iter().enumerate() {
        for copy in [false, true] {
            if copy && !half {
                continue;
            }
            let p = Editor::tree_spot(t, copy);
            let r = (t.radius * 0.9).max(0.7);
            let (y0, y1) = (p.y - 0.3, p.y + t.height);
            // Where the ray passes closest to the trunk's line, and
            // where it crosses the crown's middle.
            let flat = vec3(d.x, 0.0, d.z);
            let mut tries = Vec::with_capacity(2);
            if flat.length_squared() > 1e-6 {
                tries.push((p - o).dot(flat) / flat.length_squared());
            }
            if d.y.abs() > 1e-4 {
                tries.push((y1 - t.radius * 0.5 - o.y) / d.y);
            }
            for s in tries {
                let q = o + d * s;
                let near = vec3(q.x - p.x, 0.0, q.z - p.z).length() < r;
                if s > 0.0 && s < limit && near && (y0..=y1).contains(&q.y) && best.is_none_or(|b| s < b.0) {
                    best = Some((s, j, copy));
                }
            }
        }
    }
    best
}

/// A box's twelve edges as thin bars, lit by nothing (alpha is the
/// emissive mix).
fn outline(m: &mut Mesh, min: Vec3, max: Vec3, col: Col) {
    let t = 0.05;
    let (a, b) = (min - Vec3::splat(t), max + Vec3::splat(t));
    for (x, z) in [(a.x, a.z), (b.x, a.z), (a.x, b.z), (b.x, b.z)] {
        m.cuboid(vec3(x - t, a.y, z - t), vec3(x + t, b.y, z + t), col);
    }
    for y in [a.y, b.y] {
        for z in [a.z, b.z] {
            m.cuboid(vec3(a.x, y - t, z - t), vec3(b.x, y + t, z + t), col);
        }
        for x in [a.x, b.x] {
            m.cuboid(vec3(x - t, y - t, a.z), vec3(x + t, y + t, b.z), col);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_right_button_undoes_what_the_left_does() {
        assert_eq!(opposite(Mode::Raise), Some(Mode::Lower));
        assert_eq!(opposite(Mode::Lower), Some(Mode::Raise));
        assert_eq!(opposite(Mode::Smooth), Some(Mode::Noise));
        assert_eq!(opposite(Mode::Noise), Some(Mode::Smooth));
        assert_eq!(opposite(Mode::Ramp), None);
        // Every mode has a chip, in the same order.
        assert_eq!(MODE_CHIPS.len(), Mode::ALL.len());
        for (chip, m) in MODE_CHIPS.iter().zip(Mode::ALL) {
            assert!(m.name().starts_with(&chip[..3]), "{chip} vs {}", m.name());
        }
    }

    #[test]
    fn a_click_on_a_tree_or_its_turned_copy_picks_it() {
        let mut d = MapDoc::blank("TREES");
        d.trees.push(TreeDoc { at: [-40.0, 0.0, 10.0], height: 9.0, radius: 2.5, tint: 0, pine: false });
        d.trees.push(TreeDoc { at: [-60.0, 0.0, -20.0], height: 7.0, radius: 2.0, tint: 0, pine: true });
        // Level, at the first tree's crown, from the west.
        let hit = tree_hit(&d, vec3(-80.0, 6.0, 10.0), vec3(1.0, 0.0, 0.0), 800.0);
        assert_eq!(hit.map(|h| (h.1, h.2)), Some((0, false)));
        assert!((hit.unwrap().0 - 40.0).abs() < 3.0);
        // Straight down on the second tree's turned copy.
        let hit = tree_hit(&d, vec3(60.0, 50.0, 20.0), vec3(0.0, -1.0, 0.0), 800.0);
        assert_eq!(hit.map(|h| (h.1, h.2)), Some((1, true)));
        // Past the trees, beside them, or behind the ground: nothing.
        assert_eq!(tree_hit(&d, vec3(-80.0, 6.0, 30.0), vec3(1.0, 0.0, 0.0), 800.0), None);
        assert_eq!(tree_hit(&d, vec3(-80.0, 6.0, 10.0), vec3(1.0, 0.0, 0.0), 20.0), None);
    }
}
