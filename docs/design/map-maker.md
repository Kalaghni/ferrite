# FERRITE Map Maker - design and plan

Working name: **FORGE** (MAP MAKER on the launcher, `ferrite --forge`, or `ferrite --edit <map>`).

## Status (2026-11-19)

Answers taken for section 10: the editor lives in the game client, maps keep half-turn symmetry, no mesh import yet, and sharing is passing `.fmap` files around.

**Phase 0 is done** (`crates/shared/src/map/doc.rs`):
- **Format and loading:** `MapDoc` is JSON on disk (one block per line) and bincode on the wire. `validate()` enforces every cap, and the loader is fuzzed.
- **Building:** `MapDoc::build` makes the `World`, generating the east half in half-turn symmetry.
- **Exact round trip:** `MapDoc::from_world` writes a built map out. Foundry, Depot and Relay come back with the same fingerprint and nav grid.
- **Folding:** `MapDoc::fold` keeps the west half of a written-out map. Foundry and Depot fold. Relay does not, because its mast is only mirrored left to right.
- **Custom map ids:** they start at 128 (`doc::install`), and `World::with_map` and `map_def` know them. A custom map's sky, fog and foliage come from `look.mood` (`doc::look_of`).
- **Missing maps:** a client that joins a custom map it does not have is told so instead of building the wrong world.
- **No PROTO change.**

**Phase 1 has started** (`crates/client/src/forge.rs`):
- **Opening:** the start screen (section 11) lists the maps folder; `ferrite --edit NAME` or a `.fmap` path opens one directly, or a new map if it names none.
- **Camera:** fly with WASD and Q/E while holding right mouse to look; the wheel sets speed.
- **Tools:**
  - Select: click a block.
  - Place: eight kinds, a ghost of the block and its turned copy, R to turn, T for tint.
  - Objectives: A, B and the west base.
- **Editing the selection:** move with the arrow keys and PgUp/PgDn, Shift to resize, R, T, Ctrl+D, Delete.
- **History and files:** Ctrl+Z/Y undo, Ctrl+S save (atomic), and an autosave every minute to `FILE.fmap.autosave`.
- **Playtest:** F5 plays the map with bots, and leaving the match returns to the editor.
- **Other entry points:**
  - `ferrite --export-map NAME FILE` writes a built-in map as a starting point; it is folded when it can be.
  - `ferrite --host --map-file FILE` hosts a document.
- **Capture aid:** `--edit-select N`.

**AI tooling has started** (`crates/shared/src/map/ops.rs`, `crates/forge-mcp`, `crates/client/src/forge_link.rs`): see section 11.

**Not yet:**
- The FORGE launcher page: done (MAP MAKER and the start screen, section 11).
- The plan view.
- Box-drag brush, multi-select and mouse-drag move.
- Trees, ladders and seedlines in the editor (the format has them).
- The terrain tools (phase 3): sculpting is done (section 11); painting the ground palette per spot, regions (water, lava) and scatter are not.
- Dedicated-server `maps/` and transfer to remote clients (phase 5): remote clients cannot join a custom map yet.
- Bespoke art: Depot written out loses its signs and building detail.

## 1. Where we start from (what the code does today)

- **Maps are code.** `World::with_map(seed, id)` switches on the map id. Each map's `build_*` places AABB `Block`s, trees, features, tunnels, ladders and seedlines. The terrain comes from `shape()`/`full_height()` sampled onto a 2 m grid (241 x 169). Client and server each regenerate the same `World` from `(seed, map: u8)`, and the protocol only ever sends that `u8`.
- **The art is code too.** `scene::build_world` draws generic blocks by `BlockKind`/tint, and bespoke modules (`scene/canopy.rs`, monuments, landmarks...) add the hand-made art. `Gpu::with_scenery` takes static visual-only geometry.
- **Collision is axis-aligned boxes only.** `World::raycast`, the movement sim and the nav grid (`Nav::build_mask`) all assume AABBs. This is the biggest constraint on the editor.
- **Things we can reuse:**
  - The cine free-fly camera (`cine.rs`).
  - Top-down plans drawn from any `World` (`plan.rs`).
  - Offscreen capture (`Gpu::capture`, as in `asset_review`).
  - Chunked transfer of player icons over the protocol (`S2C::Icon`), which is a template for sending maps.
  - `net::Guard` rate limits.
  - The 16-slot lobby vote.
  - Point symmetry helpers (`sym`, `sym_abs`).

**The core move:** add a *data* map format alongside the code maps. Built-ins stay code; custom maps are documents that build a `World` the same way on client and server.

## 2. The map document (`.fmap`)

Store maps as JSON (serde, already a dependency; add `serde_json`), so they diff well and are easy to hand-fix. On the wire they go as bincode.

```
MapDoc {
  format: u16,                    // schema version, migrate on load
  meta:   { name, blurb, author, created, revision },
  look:   { ground: [[f32;3];3], leaf: [[f32;3];5], sun, fog, sky, tone },  // presets + overrides
  symmetry: PointHalfTurn | MirrorX | None,   // default PointHalfTurn, as every FERRITE map is
  terrain: {
    base: { preset: u8 /* reuse a built-in shape() */, seed: u32 },
    delta: Vec<i16>,              // 241x169 centimetre offsets, RLE on disk
    paint: Vec<u8>,               // ground palette index per cell
  },
  points: [Vec2; 5], bases: [Vec2; 2], flag_points: [u8; 2],
  placements: Vec<Placement>,     // prefab instances (section 3)
  brushes: Vec<Brush>,            // raw boxes: min, max, kind, tint, dest group
  trees: Vec<Vec2>,
  regions: Vec<Region>,           // water / lava / kill / no-bot, polygon + level
  ladders: Vec<Ladder>, lines: Vec<Vec<Vec3>>, tunnels: Vec<TunnelDoc>,
  events: Vec<Event>,             // declarative timers (phase 6)
}
Placement { asset: AssetId, pos: Vec3, turn: u8 /* quarter turns */, yaw_fine: f32 /* visual only */,
            scale: f32, tint: u8, dest: Option<u8>, mirrored: bool }
```

- **Symmetry is authored once.** You only author your side; the loader generates the other half. The editor shows the generated half as a live ghost, and placements marked `mirrored: false` stay on the centre line.
- **Integrity:** the document is identified by a content hash (sha256 of its canonical bincode). Clients cache by hash, so a renamed copy is still the same map.
- **No code, ever.** A document is pure data. Events are declarative ("every 120 s, for 24 s, toggle dest group 3 / raise region 2"), with no scripting.
- **Loading is strict:**
  - Reject NaN and out-of-bounds coordinates, and anything outside `HALF_X`/`HALF_Z`.
  - Caps: 4096 blocks, 2048 trees, 64 regions, 16 ladders, 16 lines and 1 MB compressed.
  - A hostile server must not be able to crash or stall a client.
- **Building the world:** `World::from_doc(&MapDoc, seed)` builds the heights (the base preset plus the delta), expands placements and brushes into blocks, and mirrors everything. `World::with_map` gains a `MapSource::{Builtin(u8), Custom(Arc<MapDoc>)}` path. Built-ins don't change.

## 3. Asset library

An asset is a **prefab**: collision plus a picture.

```
Asset { id: "ferrite/container_40", name, category, tags,
        collide: Vec<LocalBox>,           // AABBs in the prefab's frame
        art: Art,                         // how it is drawn
        footprint, tri_budget, thumb }
Art = Generic(BlockKind, tint)            // drawn by the existing block styling
    | Builtin(fn id)                      // a registered Rust art function (container, crane, sandbags...)
    | Composite(Vec<(AssetId, offset, turn)>)  // user-made group
    | Mesh(hash)                          // imported visual mesh (phase 6)
```

1. **Built-in library.** Mine the existing maps:
   - Split the cluster builders in `build_structures`, `build_depot`, `build_quarry` and the rest into named prefabs: container stacks, warehouse, boiler hall, bunker, sandbag runs, culvert, gantry, rock knuckles, crates.
   - Register their art functions in a registry (`scene::art::REGISTRY`), so editor maps look as good as the shipped ones.
   - Expected first cut: about 40 prefabs in 8 categories (Structures, Cover, Industrial, Natural, Vegetation, Props, Gameplay, Landmarks).
2. **Gameplay assets.** Objective markers, base pads, ladders, seedline docks and destructible crate groups. These carry rules, not just boxes.
3. **User prefabs.** Select any group in the editor and choose "Save as prefab". The group is stored as a `Composite` in `~/ferrite/assets/user/*.fasset`.
4. **Imported visuals (later).**
   - glTF/OBJ as visual-only meshes, with the author drawing the collision boxes in the editor.
   - Hard limits: 20k triangles per asset and vertex colours only (the renderer has no textures).
   - Wireframe overlay mode to compare what you see with what you hit.

**Browser UI:** a left dock with category tabs, a search box, favourites and recently used. Each asset has a thumbnail rendered once offscreen with `Gpu::capture` and cached by hash. Drag one into the world or press a hotkey to place it.

**Map packaging:** a map embeds every asset that isn't built in (composites and meshes) inside its `.fmap`, so it is always self-contained.

## 4. The editor

**Shell:**
- A new launcher page, FORGE, with three choices: new map from a preset, open, or import.
- `ferrite --edit file.fmap` opens a map directly.
- It runs inside the client, reusing `Gpu`, `scene`, `plan.rs` and `ui.rs`, with no new window system. Panels are drawn with the existing `ui.rs` look so the editor feels like part of the game. Adding egui is possible but fights the raw miniquad pipelines.

**Views:**
- **Fly:** WASD plus right mouse, as in the cine camera; Shift and Alt change speed.
- **Plan:** an orthographic top-down view built on `plan.rs`.
- **Split:** both views at once.
- **Walk:** a soldier body with real movement, for checking jumps, mantles and step heights.

**Tools** (keys in brackets):

| Tool | What it does |
| --- | --- |
| Select (Q) | Click, box-select, outliner; picking reuses `World::raycast`, extended to return the placement id |
| Move (W) | Grid snap (2 / 1 / 0.5 / 0.25 m), surface snap, stack snap onto block tops |
| Turn (E) | Quarter turns for anything that collides; a free yaw for visual-only parts |
| Box brush (B) | Drag out a footprint, then drag up the height; pick kind and tint |
| Place (P) | Stamp from the library; scatter mode for trees and rocks |
| Terrain (T) | Raise, lower, flatten, smooth, ramp, noise; paint ground palette |
| Regions (R) | Draw a polygon for water, lava, kill or no-bot; set the level |
| Objectives (O) | Drag A-E and the bases; capture radius drawn on the ground |
| Paths (L) | Ladders (two points) and seedlines (a polyline with docks) |
| Measure (M) | Distance, height and line of sight between two points |

**Always on:**
- **Undo and redo:** a command stack where every edit is an inverse-able op. It also drives autosave every 60 s to `.fmap.autosave`.
- **Symmetry ghost:** the mirrored half drawn translucent and kept live.
- **Overlays:**
  - Nav grid (blocked cells).
  - Spawn radius.
  - Capture zones.
  - Sightline heatmap.
  - Height contours.
  - Triangle budget bar.
- **Properties panel:** fields for the selection.
- **Outliner:** layers that can be hidden or locked.

**Rebuilding while you edit:**
- Edits change the `MapDoc`, then rebuild only the dirty part.
- Block meshes are rebuilt per chunk; the world mesh would be split into 32 m tiles for the editor.
- The terrain is rebuilt per dirty tile.
- Nav is rebuilt in the background. It takes 75-90 ms, so never on the frame.

**Playtest (F5):**
1. Starts the embedded server with the in-memory document and bots, in the chosen mode.
2. Esc returns you to the editor at the same camera position.
3. Edits made while hosting take effect at the next round start.

## 5. The map doctor (validation)

Runs live as a warnings list and blocks publishing on errors:

- **Fairness:** symmetry holds (generated, but check the centre-line items), and both bases are the same distance from each objective (path length on the nav, within 5%).
- **Reachability:**
  - Every objective, base and flag point can be reached on the ground. This is the generalised test from `crates/server/tests/canopy.rs`.
  - No spawn is inside a block.
  - Every ladder top lands on walkable ground.
- **Sightlines:** report lines of sight longer than 80 m that cross the midline or run base to base. These are the sniper lanes, which was a lesson from Fallen Giant.
- **Budgets:** blocks, triangles (static and scenery), nav build time and document size, with green/amber/red bars.
- **Bot soak (button):** runs `ferrite-server --bench 600 --mode X` on the document in the background and reports captures, rounds and stalemates per mode.

## 6. Multiplayer distribution

- **Server side:**
  - The server loads `maps/*.fmap` next to its executable.
  - Rotation and vote lists take names or hashes.
  - Custom maps get session ids 128-254 in the existing `map: u8` fields, so no widening is needed.
- **Protocol (PROTO bump):**
  - `S2C::Welcome` and lobby snapshots carry `map_hash` for custom maps.
  - The client looks in `cache/maps/<hash>.fmap`. If it's missing, it sends `C2S::MapReq(hash)` and gets `S2C::MapPart { hash, at, total, bytes }` chunks, like icons do.
  - Transfers are rate-limited under `net::Guard` and capped at 1 MB compressed.
  - The client shows "downloading map" in the lobby.
- **Integrity:** the client re-hashes what it received and rejects a mismatch.
- **Lobby:** the vote shows the custom map's name and author, with an auto-drawn plan from `plan.rs` (already map-generic).

## 7. Sharing

- **v1:** export and import `.fmap` files; dropping one on the launcher installs it (as icons work today).
- **Later:** a small map browser served from josh-docker over HTTP, with a list, a download and plan thumbnails. Uploads would need moderation, and the developer decides if and when.

## 8. Phases

| # | Phase | Delivers | Size |
| --- | --- | --- | --- |
| 0 | Format and loader | `MapDoc`, serde, validation caps, `World::from_doc`, mirroring | M |
| 1 | Editor MVP | FORGE page, fly and plan views, box brush, select/move/snap/turn, objectives and bases, symmetry ghost, undo, save/load, F5 playtest | L |
| 2 | Asset library | Art registry, about 40 prefabs from existing maps, browser with thumbnails, user composites | L |
| 3 | Terrain and look | Sculpt and paint, regions (water/lava), mood presets, trees and scatter | M |
| 4 | Map doctor | Reachability, symmetry, sightlines, budgets, background bot soak | M |
| 5 | Online | PROTO bump, hash, chunked transfer, server `maps/` folder, custom maps in rotation and vote | M |
| 6 | Advanced | Ladders, seedlines, tunnels, destructible groups, declarative timed events, glTF visual import | L |
| 7 | Sharing hub | Map browser on josh-docker | M (optional) |

**Phase 0 acceptance test:** snapshot Foundry Valley's generated blocks, trees and heights into a `MapDoc`. Rebuilding from it must give an identical `World` and an identical nav grid. Without its bespoke art, that map is then proof that the document path carries a full map.

**Tests to add:**
- Document round-trip (JSON and bincode).
- Loader fuzz (random bytes never panic).
- `from_doc` symmetry.
- The map doctor on every built-in map, which should pass.
- Transfer of a 1 MB map over a lossy socket.

## 9. Being critical: risks and limits

- **Box-only collision is the real ceiling.** Rotated collision would mean OBB support in the raycast, movement sim and nav, which is a big, protocol-sensitive change. Recommendation: stay with quarter turns for collision and allow free rotation for visuals only. Show a "collision differs from art" overlay so authors aren't fooled.
- **Custom maps will look plainer than the shipped ones.** The shipped maps' beauty is bespoke code: the Hollow Crown, monuments, Fallen Giant. Prefabs carry some of it; landmarks don't carry over. Phase 2 is what decides whether custom maps look like FERRITE, so it deserves the most art time.
- **The 2 m terrain grid** limits fine sculpting. Cliffs come out as 2 m steps, so blocks have to stand in as cliffs.
- **Nav rebuild cost** (about 80 ms) already causes a hitch at round changes. Background rebuilding is needed for the editor, and it would also fix the existing hitch.
- **Built-in maps can't be edited faithfully.** "Fork into a document" is possible for blocks and terrain, but loses the bespoke art and mechanics (gust, floods). Label it clearly or don't offer it.
- **Security:** a server now sends data the client parses, so strict caps and a fuzzed loader are mandatory. There is no scripting by design.
- **Scope:** phases 0-2 are a usable, shareable editor for local play; everything after is optional. Ship phases 0-2 before committing to phase 6.

## 10. Questions for the developer

1. Should the editor be inside the game client (recommended) or a separate tool?
2. Must custom maps keep half-turn symmetry (recommended), or may authors choose a mirror or no symmetry?
3. Is importing outside meshes (glTF) wanted, given the vertex-colour, low-poly look?
4. Should there be any online sharing beyond passing `.fmap` files around?

## 11. AI: the FORGE MCP server and generated assets

The direction (2026-11-19): AI comes in through an MCP server that any assistant can drive, and through assets it can generate and change. No chat panel inside the editor.

**Edits as data** (`map::ops`): every change is an `Op` (place, add, move, reshape, restyle, turn, copy, remove, clear, prefab, tree, objective, base, rename, look). A batch applies in order and lands whole or not at all, validated once at the end. The editor's tools, the MCP server and the coming command palette all speak it.

**The live link** (`forge_link.rs`): FORGE listens on `127.0.0.1:47817` (loopback only) for newline-separated JSON: `status`, `doc`, `apply`, `save`, `undo`/`redo`, `view` (camera, selection, a note) and `shot` (a picture from any camera, the user's view put back after). It is polled once a frame and never blocks. Every applied batch is one undo step, and the panel shows LINKED while a tool is connected.

**The MCP server** (`ferrite-forge-mcp`, stdio): when FORGE is open on the map, tools go through the link, so edits appear on screen. Otherwise they work on the `.fmap` file directly. The tools:

| Tool | What it does |
|---|---|
| `forge_status` | Is FORGE open, on what, unsaved or not, where the camera is |
| `list_maps` | The maps folder, newest first: names, files, ages, sizes, which is open, which have newer autosaves |
| `create_map` | New map in the maps folder: flat, on a built-in map's ground, or a copy of a built-in map to remix (folded to half-turn where it allows) |
| `describe_map` | Layout and blocks with ids, filterable by region and kind |
| `plan_image` | Top-down PNG of the built map; with `heat`, where bots died in the last soak |
| `edit_map` | A batch of ops; prefabs by name |
| `analyze_map` | The map doctor (section 5), first cut: routes, cover and openness per objective, sight between objectives, spawn exposure, mirror twins, and floating, buried or overlapping blocks |
| `soak_test` | A headless bot match (120 s of 10 v 10 runs in about 1 s): kills, ranges, early deaths, captures, hot spots and a heat plan |
| `screenshot` / `show_user` | See the 3D view, and point the user's camera at something |
| `save_map`, `undo` | Through FORGE |
| `list_prefabs`, `get_prefab`, `save_prefab` | The asset library |

To register it with Claude Code: `claude mcp add ferrite-forge -- C:/path/to/ferrite-forge-mcp.exe`.

**Generated assets, in order of what the engine can carry:**
1. **Prefabs (now):** named groups of blocks and trees in `prefabs/*.fprefab` next to the map, or `FERRITE_PREFABS`. An assistant can compose one from a description (a bunker, a gatehouse), capture one from blocks already placed, then turn, restyle, resize and stamp it. Prefabs fit the box-only collision exactly, so what you see is what you collide with.
2. **Looks (now, through `look`):** ground palettes and borrowed moods. Next: named material variants per kind, beyond the four tints.
3. **Meshes (phase 6):** text-to-3D models as visual-only meshes, with collision fitted automatically as a few boxes and shown by the "collision differs from art" overlay. They need the renderer to load meshes, a vertex-colour pass to match the low-poly look, and phase 5 transfer to reach other players. Until then a generated "model" should be a prefab.

**The library drawer** (`forge_library.rs`, `forge_thumbs.rs`, `map::library`): the library is `prefabs/` next to the game (or `FERRITE_PREFABS`), its sub-folders are folders and its `.fprefab` files are models, plus a built-in set (BLOCKS: one of each kind; COVER, STRUCTURES, NATURE: 13 models). FORGE shows it as a drawer on the right (L toggles it):
- Folder cards preview the first four models inside; model cards show a picture, name, footprint and block count, and a green dot for your own.
- Pictures are drawn by a small software renderer (flat-lit faces, dark edges, soft shadow), six a frame; the hovered card turns slowly.
- Drag a card into the world to place it (a ghost and its turned copy follow the mouse, R turns it, it lands on whatever block is under the mouse), or click it to stamp it with every click on the world until Esc.
- OPEN FOLDER opens the library in the file browser; the drawer rereads the folder every 2 s, so models saved by the MCP server, or folders made by hand, appear on their own.
- The MCP tools `list_prefabs`, `get_prefab`, `save_prefab` (with `folder`) and the `prefab` edit op use the same library.
- Capture aids: `--edit-folder NAME`, `--edit-stamp NAME@X,Y`.

**Maps and the start screen** (`forge_home.rs`, `map::maps`): maps live in `maps/` next to the game (or `FERRITE_MAPS`), one `.fmap` each, unsaved work beside it as `.fmap.autosave`.
- **Getting in:** MAP MAKER on the launcher's first page, `ferrite --forge`, or `ferrite --edit NAME` (a name in the maps folder, a new map there if it names none, or any `.fmap` path).
- **The start screen:** a card per map, newest first, with its plan (drawn from the document on a worker thread, objectives and bases marked), its name, when it was edited and its block count; hovering shows the file. Maps with an autosave newer than the file are badged UNSAVED WORK. The folder is reread whenever it changes, so maps made by the MCP server or copied in appear on their own. OPEN FOLDER, BACK (Esc), NEW MAP (Ctrl+N).
- **New map:** a name, and a start: BLANK (flat), or any built-in map's ground, or everything on it to remix (not for maps with tunnels yet). Copies are folded to their west half where the map allows, so edits land on both sides.
- **Unsaved work:** opening a map whose autosave is newer asks: RESTORE THEM (the map opens with that work, unsaved), OPEN LAST SAVE (the autosave is dropped), or CANCEL. Saving drops the autosave; leaving with unsaved changes asks once, then keeps them in the autosave.
- **In FORGE:** Esc or Ctrl+O goes back to the start screen, Ctrl+N to its new-map panel, Ctrl+Shift+S saves as a new map in the folder under a typed name.
- **MCP:** every `path` also takes a map's name in the folder; `create_map` writes there unless given a path.
- Capture aids: `--forge`, `--forge-new`, `--edit-restore`.

**Terrain** (`map::terrain`, the `sculpt` op, FORGE's TERRAIN tool on key 4): a round brush on the 2 m height grid.
- **Modes:** RAISE and LOWER (metres at the centre, soft to the rim), FLATTEN (towards the height where the stroke began, flat over the inner half), SMOOTH (towards the mean of the ground within 4 m), RAMP (a straight slope between two points, the brush radius either side) and NOISE (seeded lumps).
- **In FORGE:** hold and drag on the ground (RAMP: drag from foot to top). Tab changes mode, [ and ] the size, - and = the strength. The brush shows as two rings, and its turned twin in half-turn maps. A whole stroke is one undo step and Esc takes it back. While a stroke runs the world is rebuilt on a worker thread (a full rebuild is 150-250 ms, mostly the ground mesh), so the frame rate holds and the ground catches up several times a second; every other edit now rebuilds off the frame too.
- **Symmetry:** in half-turn maps each stroke is made at its turned spot as well, taking the stronger of the two where they meet, so the ground stays the same under a half turn.
- **What stands on the ground follows it:** trees at its foot, blocks footed in it (foot at or up to 4 m under the lowest ground beneath them), and blocks stacked on those. Floating blocks stay put. Later ops in the same batch stand on the new ground.
- **Stored** as the preset's heights plus a per-point delta in centimetres (up to 200 m), written back after each stroke; the preset's heights are cached.
- **MCP:** `edit_map` takes `sculpt {mode, at, radius, amount?, height?, to?, heights?, scale?, seed?}`. `analyze_map` gains a GROUND section (what was sculpted, the height range, each objective against the ground around it) and warns about sculpted slopes over about 37 degrees (anyone walks up any slope, so they are not walls), objectives in a hollow or on a rise of over 3 m, and capture zones spanning over 4 m of height.
- **Not yet:** painting the ground palette per spot (the format has one palette per map), water and lava regions, and finer than 2 m (cliffs are still rock blocks).
- Capture aid: `--edit-brush MODE@X,Y`.

**Next for this track:** multi-select in FORGE and "save selection to library", search in the drawer, the command palette sending typed requests to an assistant, and suggestions shown as ghost diffs to accept or reject.
