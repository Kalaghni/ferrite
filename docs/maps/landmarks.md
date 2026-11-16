# Working landmarks

The user's original concepts are saved in [Future map landmarks](future-landmarks.md): the Broken Viaduct, Titan Excavator, Fallen Giant and Great Dam, including reference links and ambient details. All four are built; the three scenery-only giants are described under [Monuments](#monuments).

The map expansion gives the four oldest battlefields authored places to fight through, large silhouettes to navigate by, and visible machinery. Playable additions are mirrored under a half turn, preserving the same routes and cover for both teams. Their doors and tunnel approaches are levelled and reserved before random buildings, cover and trees are placed.

| Battlefield | Playable destinations | Landmark and motion |
| --- | --- | --- |
| Foundry Valley | Complete boiler workshops with four doors, windows, interior work surfaces and a covered service culvert | Paired tall stacks, drifting exhaust and a working flywheel; the Broken Viaduct overhead |
| Quarry | Site workshops and an underground mine: an adit in the terrace wall, a timbered drift with rails and lamps under the second terrace and the headframe, and a long incline up to daylight | An 18-metre headframe, winding wheels, moving skips and ore dust; the Titan Excavator over the pit |
| Ridgeline | Ranger lodges, a bending natural cave through the ridge and the Fallen Giant's walkable cargo hold across C | Huge cargo-aircraft wreck, tail above the pines, emergency beacon, rattling panels and wind turbines |
| Depot | Dispatch warehouses and open-ended container passages | A 52-metre-wide gantry, 27-metre supporting towers, traveling trolley, suspended container spreader and warning beacons |
| Spillway | Pump halls, and the gate deck at C over a channel that floods ([spillway.md](spillway.md)) | The Great Dam beyond the north fence |
| Relay | Existing climbable signal towers and listening station | One giant cellular backbone beyond the boundary, with antenna crowns, service rings, scanner and aviation lights |
| Slag Cathedral | Existing casting nave, furnace courts and galleries | A distant volcanic caldera with lava seams, smoke and occasional eruptions |

The volcano has a seeded 40% eruption chance in each 96-second visual cycle. An active cycle builds warning smoke, launches a lava fountain and arcing volcanic bombs, and cools into ash. It sits beyond the playable boundary and causes no damage. It runs, like the machinery, the wind and the other map events, on the world clock: the server's tick, which every client follows from its snapshots, so all players - including anyone who joins mid-round - see the same eruption at the same moment.

## Monuments

Three giants give Foundry Valley, Quarry and Spillway a skyline. They live in `crates/client/src/scene/monuments*.rs` and are scenery only: nothing collides, the server knows nothing about them, and the protocol is unchanged. Everything over the playable field is at least 5 m above the ground (tested on several seeds), and everything at player height stands beyond the fence. Each is centred on x = 0, so both teams see it the same way. Trees and brush beyond the fence give way where a monument stands; trees inside the fence are never hidden, because their trunks collide.

| Battlefield | Monument | Where | Motion and sound |
| --- | --- | --- | --- |
| Foundry Valley | The Broken Viaduct: a riveted lattice arch 232 m between footings, carrying the ore line 44 m over C, with braced trestle towers and stone abutments into the hills | Along x = 0 from ridge to ridge. The north approach span (z 127 to 145) has fallen onto the slope beyond the fence | The hanging engine and its tender sway with the wind; a signal lamp dangles under the lip and blinks red. The tender creaks at the end of most swings |
| Quarry | The Titan Excavator: a walking dragline with a 172 m lattice boom, A-frame, pendants, walking shoes and a bucket bigger than the haul truck parked by its tub | Tub on the south ridge at z = -146; the boom reaches north over the terraces and the bucket hangs about 33 m above the pit floor north of C | In most 80-second windows the house slews a few degrees out and back; the bucket swings on its ropes and sheds dust. Ropes groan when it slews and now and then as it swings |
| Spillway | The Great Dam: a 66 m concrete gravity dam with five steel floodgates, a hoist bridge with a travelling maintenance gantry, two powerhouses with penstocks, and rock shoulders holding the reservoir | Beyond the north fence, z 113 to 137, facing the overcast sun | Water weeps from the gate seals down the chute; spray rises off the stilling basin; turbine-hall lights pulse. In about half of the 90-second windows the sirens are tested for 14 seconds, with turning amber beacons; then the middle gate lifts and the reservoir pours down its bay of the chute until the release ends |

All motion and every sound cue is a pure function of the world seed and the visual clock passed to `atmosphere`, so clients sharing a clock see and hear the same thing. The sounds are synthesised like the Fallen Giant's rattle and play positionally at the effects volume.

## Explosive changes

The four priority maps have designated weak wall panels, service walkway sections and machine controls. Hazard paint and signs identify them. Ordinary gunfire leaves these features intact; a nearby unobstructed grenade blast can trigger their discrete state change.

- A breached panel removes its collision and opens an alternate path.
- A collapsed walkway removes its deck and supporting pieces. Anyone standing on it falls under the normal player physics.
- A disabled control stops the associated machinery and changes its visible status.

The server owns all transitions. The persistent destruction mask is included in every snapshot, so joining late or losing a cosmetic debris event does not restore a broken feature. Collision and navigation update together. Mirrored features have independent state; a new round restores the initial layout. v0.11.0 uses protocol 18, including the Fallen Giant's changed Ridgeline collision, so clients and servers must match.

## Review

Build the direct renderer exporter with `cargo build --release -p ferrite --example asset_review`. It writes a PNG after 24 frames and does not require a server or saved settings.

```powershell
target/release/examples/asset_review depot depot.png landmark 10
target/release/examples/asset_review quarry mine.png passage 10
target/release/examples/asset_review foundry workshop.png workshop 10
target/release/examples/asset_review ridgeline lodge.png interior 10
target/release/examples/asset_review relay mast.png mast 10
target/release/examples/asset_review slag volcano.png volcano 33
target/release/examples/asset_review foundry intact.png breach 10
target/release/examples/asset_review foundry breached.png breach 10 broken
target/release/examples/asset_review depot collapsed.png walkway 10 broken
target/release/examples/asset_review ridgeline aircraft.png wreck 10
target/release/examples/asset_review ridgeline hold.png cargo 10
```

Monument views:

```powershell
target/release/examples/asset_review foundry viaduct.png viaduct 12
target/release/examples/asset_review foundry train.png train 12
target/release/examples/asset_review foundry crossing.png crossing 12
target/release/examples/asset_review quarry excavator.png excavator 12
target/release/examples/asset_review quarry bucket.png bucket 12
target/release/examples/asset_review quarry dragline.png dragline 12
target/release/examples/asset_review spillway dam.png dam 214
target/release/examples/asset_review spillway gates.png gates 214
target/release/examples/asset_review spillway crest.png crest 214
```

With seed `0xFE221`, second 214 falls inside a siren test, and second 240 shows the middle gate lifted during the release. Views of the gate deck and the flood are in [spillway.md](spillway.md).

`workshop`, `interior`, `passage`, `landmark`, `breach` and `walkway` provide close views on the original four maps; on Quarry and Ridgeline `portal`, `tunnel` and `exit` look at the underground route. Existing Cathedral views remain available. A six-number `eyeX,eyeY,eyeZ,targetX,targetY,targetZ` camera can be supplied instead. The fourth argument freezes animation, including wind; seed `0xFE221` at second 33 shows an active eruption. The fifth accepts `broken` for all structural features or a decimal/hexadecimal destruction mask for an individual state.

## Triangle profile

`cargo run --release -p ferrite --example triangle_profile` prints, for every map, the static world triangles by scene module (terrain, blocks, trees, ground cover, flora, landmarks, the Fallen Giant, Relay, the relay mast and volcano, monuments, Slag Cathedral) and the peak per-frame triangles each module rebuilds (machinery, beacons, smoke, crates, objectives, flora ambience). It needs no window or GPU. Use it before raising a budget: the monument tests in `scene/monuments.rs` take their limits from it.

With seed `0xFE221`, the static worlds run from 0.6 to 1.44 million triangles, mostly terrain, ground cover and flora. Each monument costs a small share of that:

| Monument | Static | Per frame (peak) | Test budget (static / per frame) |
| --- | --- | --- | --- |
| Broken Viaduct | 16,000 | 3,108 | 8,000-32,000 / 3,500 |
| Titan Excavator | 1,128 | 5,356 | 800-6,000 / 7,000 |
| Great Dam | 9,486 | 872 | 6,000-24,000 / 2,500 |

The excavator's house, boom and bucket are rebuilt every frame because they slew and sway, so most of its cost is per frame. The dam stands on and floods slopes that used to carry backdrop forest, so Spillway's static total went down.

## Underground tunnels

Quarry's mine and Ridgeline's cave are real tunnels (`crates/shared/src/map/tunnels.rs`). The heightfield stays the visible surface - the hill over the tunnel - and the tunnel adds a cavity under it:

- Inside the cavity's footprint, anyone at floor height stands on the tunnel floor, whatever the terrain above does; a flat roof stops jumps, and ground that rises above the feet next to the cavity is rock that stops bodies. Bullets and sight lines treat the cavity as air and the rest of the ground as solid, so nothing reaches the tunnel through the hill.
- Each route is a Catmull-Rom curve through authored points, resampled every 0.75 m and mirrored under a half turn. Under the roof the floor sinks until the natural ground gives at least 1.6 m of rock above the roof, the slope is held under 30%, and only the remaining shortfall is made up with a low mound. Each end is an open cut carved down to the floor.
- The renderer leaves out the terrain cells that cross a portal and closes the gap with a rock collar and a cap over the mouth; the drawn walls and vault stay outside the clear width (2.3 m either side) and height (3.3 m).
- Bots follow the surface flow field; underground, their movement is turned along the tunnel.
- Quarry's adit opens from the first terrace into the face of the second, runs round under the second terrace and the headframe, and climbs a 17-metre incline to daylight. Ridgeline's cave bends through the ridge between the saddle and the southern pass, so it is never one straight sight line.

`crates/server/tests/tunnels.rs` walks every tunnel both ways for four seeds, checks the cover and slopes, and checks that walls, the roof and the hill above stop walkers, jumps and shots.
