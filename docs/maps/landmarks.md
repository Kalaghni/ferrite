# Working landmarks

Future concepts for the next map pass are saved in [Future map landmarks](future-landmarks.md): the Broken Viaduct, Titan Excavator, Fallen Giant and Great Dam, including the user's reference links and ambient details.

The map expansion gives the four oldest battlefields authored places to fight through, large silhouettes to navigate by, and visible machinery. Playable additions are mirrored under a half turn, preserving the same routes and cover for both teams. Their doors and tunnel approaches are levelled and reserved before random buildings, cover and trees are placed.

| Battlefield | Playable destinations | Landmark and motion |
| --- | --- | --- |
| Foundry Valley | Complete boiler workshops with four doors, windows, interior work surfaces and a covered service culvert | Paired tall stacks, drifting exhaust and a working flywheel |
| Quarry | Site workshops and long timber-supported mine adits with rails and two exits | An 18-metre headframe, winding wheels, moving skips and ore dust |
| Ridgeline | Ranger lodges, rock passages and the Fallen Giant's walkable cargo hold across C | Huge cargo-aircraft wreck, tail above the pines, emergency beacon, rattling panels and wind turbines |
| Depot | Dispatch warehouses and open-ended container passages | A 52-metre-wide gantry, 27-metre supporting towers, traveling trolley, suspended container spreader and warning beacons |
| Relay | Existing climbable signal towers and listening station | One giant cellular backbone beyond the boundary, with antenna crowns, service rings, scanner and aviation lights |
| Slag Cathedral | Existing casting nave, furnace courts and galleries | A distant volcanic caldera with lava seams, smoke and occasional eruptions |

The volcano has a seeded 40% eruption chance in each 96-second visual cycle. An active cycle builds warning smoke, launches a lava fountain and arcing volcanic bombs, and cools into ash. It sits beyond the playable boundary and causes no damage. Its visual clock, like the other ambient animations, is local to each client.

## Explosive changes

The four priority maps have designated weak wall panels, service walkway sections and machine controls. Hazard paint and signs identify them. Ordinary gunfire leaves these features intact; a nearby unobstructed grenade blast can trigger their discrete state change.

- A breached panel removes its collision and opens an alternate path.
- A collapsed walkway removes its deck and supporting pieces. Anyone standing on it falls under the normal player physics.
- A disabled control stops the associated machinery and changes its visible status.

The server owns all transitions. The persistent destruction mask is included in every snapshot, so joining late or losing a cosmetic debris event does not restore a broken feature. Collision and navigation update together. Mirrored features have independent state; a new round restores the initial layout. Current development builds use protocol 18, including the Fallen Giant's changed Ridgeline collision, and require matching client/server builds.

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

`workshop`, `interior`, `passage`, `landmark`, `breach` and `walkway` provide close views on the original four maps. Existing Cathedral views remain available. A six-number `eyeX,eyeY,eyeZ,targetX,targetY,targetZ` camera can be supplied instead. The fourth argument freezes animation, including wind; seed `0xFE221` at second 33 shows an active eruption. The fifth accepts `broken` for all structural features or a decimal/hexadecimal destruction mask for an individual state.
