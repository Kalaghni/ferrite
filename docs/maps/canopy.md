# Canopy - The Hollow Crown

Canopy is a botanical reserve in the roots and lower boughs of a colossal strangler fig, centred on C. Seed collectors once crossed its branches on powered harvesting slings; those lines now carry soldiers over the fight in the hollow beneath the trunk. A storm front is coming in, and every two minutes a gust comes through the crown and furls the reserve's shade cloth.

The map was designed in collaboration with Codex (the creative brief, the mechanics and their numbers) and built in the engine by Claude. Its brief was to give the map play that exists nowhere else in FERRITE, not just a new look: a connected upper route, rideable lines, cloth that hides without protecting, and a clock that takes that cover away.

## The tree

The layout (`crates/shared/src/map/canopy.rs`) is authored for the west half and mirrored under the half turn, like every map.

- **Trunks.** Four main trunks (two authored, two mirrored) rise from the ground to 36 m around the root well at C. Root knuckles, a base root shield and an outer buttress pierced by two 6 m passages break up the ground lanes.
- **The bough circuit.** A continuous ring of walkable boughs at 18 m, 4 m wide, joins all four trunks. An inward branch reaches over the well from each side and ends in a 2 m drop opening onto open ground. Rails run along the open edges and bark lips cap the closed corners.
- **Ways up.**
  - Behind B (and mirrored behind D), a two-flight stair climbs from the collector terrace to a landing at 12 m and on to the circuit at 18 m. It has 0.3 m risers and is carried on root masses.
  - On the circuit's west leg (and mirrored on the east), a neutral ladder runs from the ground to a hatch in the bough.
- **Ground.** All five objectives and both bases stay reachable on the ground, and nothing at 18 m blocks a ground route.

Trunks and roots are drawn as dark bark wrapped in an irregular lattice of root ribs, never more than 0.22 m proud of what collides.

## The Hollow Crown

Above anything a player can reach, each trunk climbs on into great limbs, arches ring a well of open sky over C, and a leaf roof about 164 by 144 m spreads out to a droop at its rim. It is render-only scenery (`scene/canopy.rs`, `crown`), uploaded with `Gpu::with_scenery` so that it **casts no shadow**: the crown in the shadow pass would put the whole field in shade.

The mood is a bright afternoon before a tropical squall. Light is warm, the fog is green-grey and the crown's underside is cool. The reserve's own trees and the forest beyond the fence grow rainforest greens.

## Seedlines

Six powered lines: three are authored and three mirrored.

| Line | From | To |
| --- | --- | --- |
| Court line | The collector court behind B (ground dock) | The circuit's north-west corner (-42, 24) |
| North-west span | The north-west corner | The bough north of the north trunk (0, 48) |
| North-east span | The bough north of the north trunk | The circuit's north-east corner (42, 24) |

Their mirrors give D its own court line and run the two spans round the south of the tree, so the spans make a ring of lines over the circuit.

**Riding.**
- Stand on a turquoise dock pad and press **Use** (default **E**) to take the line you are facing. Where two lines share a dock, the one heading most nearly your way is taken.
- **Forward** reels you along at up to 14 m/s and **back** returns at up to 8 m/s. Let go and the sling slows to a hang, **crouch** brakes hard, and **jump** lets go with a little lift.
- The first and last legs (the dock climbs) are taken at 4 m/s.

**While riding.**
- Taking a line draws your sidearm. You can shoot it, but you cannot swap, reload, throw or melee.
- Riders hang in a yellow sling under a brass pulley block. Their motor whines while they move and falls quiet when they hang.

Riding is part of the predicted movement (`sim::ride`, new `PState` fields), so it is protocol **21**.

## Shade curtains and the crown gust

Two banks of striped shade cloth hang across the root well on the plane x = -18 and its mirror. Each bank has two 16 m panels, let down from a drum at 7.4 m.

- **Cranks.** Hold **Use** for 1.2 s at a bank's crank wheel, on the inward face of its trunk, to let the bank down or roll it up. A brass drive runs from the wheel up the bark to the drums. A bank then rests for 6 s.
- **Cloth stops sight, never bullets.** Bots cannot see through it (`World::screened`) whatever you do. Bodies, rounds and grenades pass straight through.
- **The crown gust** (`events::canopy_gust`) blows every 120 s:

| Seconds into the window | What happens |
| --- | --- |
| 24 | Wind rising: on-screen warning, the cranks lock |
| 30 | Every curtain furls, whoever wants it down |
| 54 | The wind is through; the cranks unlock |

The gust never hurts, moves or slows anyone. It changes what can be seen, and when a push under cover is possible. The bank state is server-authoritative and sent in each snapshot (`Snapshot::shades`). Clients roll the cloth toward it and ripple it in the wind.

## Flags

In CTF, a flag dropped well above the ground (on a bough, the stair, or a seedline) falls to the nearest open ground below. Nothing solid stands where it lands, so a flag is never stranded somewhere only a person can reach, and bots fight for it on the ground.

## Review

```powershell
cargo build --release -p ferrite --example asset_review
target/release/examples/asset_review canopy key.png "-72,11,12,0,24,8"
target/release/examples/asset_review canopy well.png "-10,3.2,-6,0,60,0"
target/release/examples/asset_review canopy bough.png "-42,20,24,40,26,30"
$env:REVIEW_SHADES="down"; target/release/examples/asset_review canopy shades.png "-32,9,16,-8,3,16"
```

`crates/server/tests/canopy.rs` covers:
- **Seedlines:** a rider reaches the boughs, every line runs both ways, and hanging, reversing and jumping off all work. Riders keep their sidearm, and Use away from a dock does nothing.
- **Traversal:** the stair and the ladder climb to the circuit, every objective and base can be reached on the ground, and the boughs are mirrored.
- **Shades and gust:** a held crank lets its bank down and screens sight but not fire, cranks rest after use and lock in the gust, and the gust keeps its timetable.
- **Flags:** flags dropped on the boughs, the stair and along every seedline land on open ground.
