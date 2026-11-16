# Spillway - The Gate Deck and the Flood

Spillway's channel runs below the Great Dam. In about half of the 90-second windows the dam's sirens are tested, and every test is followed by a release: the outlet gates lift, water bursts out of the banks at C and runs both ways along the channel, rises to 1.9 m, and drains away again. C no longer sits on the channel floor. It stands on the old floodgate's deck, raised over the water on piers, so the fight for the middle goes on through the flood.

## The deck

The deck (`crates/shared/src/map/spillway.rs`) is a 20 m square concrete slab centred on C, at roughly the height of the bank tops: 3 to 4 m above the channel floor depending on the seed, so it always stands at least 1.1 m clear of a full flood. It rests on two pier walls along the channel and four corner columns, with steel cross-girders beneath.

- **Bridges** run north and south from the deck, 3.2 m wide, to the top of each bank, carried over the outfalls on a causeway. Galvanised two-beam rails like Slag Cathedral's line both sides wherever the bank drops away beside them.
- **Stairs** drop east and west from the deck to the channel floor, 2.8 m wide, in 0.3 m risers with yellow nosings, railed where the drop is a metre or more.
- **Rails** run round the deck's edge, broken only where the bridges and stairs land. They are thin beams, not invisible walls: you can shoot and see through them, but not walk off the deck.
- **Cover** on the deck is mirrored under the half turn: a concrete wall, a sandbag line, a valve cabinet and a winch on each side.
- **The floodgate** that used to block the channel now hangs lifted overhead in its portal frame: four legs, girders, the ribbed "GATE C" leaf and the yellow winding house on top. The leaf's underside is 3.3 m above the deck, so nothing overhead is closer than that.

All of it is static collision, mirrored under a half turn. The old gate's blocks are reserved in the build where they always were, then replaced by the deck after the scatter, so the random cover and trees elsewhere on the map stay where they were.

## The outfalls

Each bank at C is faced with a concrete headwall carrying two culvert mouths, symmetric about the bridge. A headstock with a gearbox and handwheel stands on the terrace above each one. The culverts' interiors are solid (you cannot walk into them); the steel sluice gates in their mouths are art only. When the gates open they lift in their guides with `Flood.gates`, and white water bursts out from under them, falls to the channel and throws up spray. At the dam, the middle floodgate lifts at the same moment and the reservoir thunders down its bay of the chute into a churning stilling basin.

## The water

Everything about the flood is a pure function of the world seed and the world clock (`ferrite_shared::events::flood`), so the server and every client agree where the water is:

| Seconds into a tested window | What happens |
| --- | --- |
| 30 to 44 | Sirens |
| 35 | Gates start to open (a heavy clank at both outfalls) |
| 39 | Water reaches the channel at C and runs outward at 8 m/s, rising over 9 s |
| 72 | Gates close (another clank) and the water starts to fall |
| 84 | Drained; the floor stays dark and wet, and puddles dry out over about 25 s |

The client draws the surface as one translucent sheet that follows `water_surface`, with its leading edge at `reach`, foam where it laps the banks, piers and stair feet, a churning white front while the surge is running, and streaks of foam carried outward from C. Nothing is drawn when the channel is dry. A rushing-water sound plays at C while the water runs, under the combat sounds.

Water deeper than 0.3 m (`WADE`) does 30 damage per second (`WATER_DPS`), so staying in a full channel kills in a little over three seconds. Wading at the edges, standing on the stairs above the water, and anything on the deck or bridges is safe.

## Capture and bots

On Spillway, C counts only players on the deck: within the 20 m square and between 0.6 m below and 7 m above its surface. Players in the channel underneath do not count. The HUD's capture panel uses the same rule (`World::on_point`), and so does every other map, where the old disc rule is unchanged.

Bot navigation reads the walkways as ground at their own height. Moves between the deck and the channel floor are only allowed where they meet at the same height, so routes go over the bridges or up the stairs, and the rails keep bots on the deck. While the siren is sounding or water is out, bots follow a second set of flow fields that treats the channel as impassable, and a bot caught in the water heads for the nearest dry ground first. A bot under the deck is steered out from under it rather than counting as arrived.

## Review

```powershell
cargo build --release -p ferrite --example asset_review
target/release/examples/asset_review spillway platform.png platform 100
target/release/examples/asset_review spillway flood.png platform 240
target/release/examples/asset_review spillway outfall.png outfall 221
target/release/examples/asset_review spillway channel.png channel 240
target/release/examples/asset_review spillway deck.png deck 240
target/release/examples/asset_review spillway release.png gates 240
target/release/examples/asset_review spillway drying.png channel 268
```

With seed `0xFE221`, windows 2, 7, 8 and 11 are tested; second 221 has the gates opening, 240 a full flood and 268 a draining channel.

`crates/server/tests/spillway.rs` checks the deck stands above a full flood with head room under the gate, that the bridges and stairs can be walked with the real movement code, that water hurts and kills in the channel but not on the deck, bridges or shallows, that capture counts the deck and not the channel, that the structure is mirrored, and that bots take C both dry and in a flood without walking into water.
