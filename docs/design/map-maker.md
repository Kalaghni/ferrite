# FERRITE Map Maker - design and plan

Working name: **FORGE** (a launcher page and `ferrite --edit <file>`).

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
