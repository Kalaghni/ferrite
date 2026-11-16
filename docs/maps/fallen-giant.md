# Ridgeline - The Fallen Giant

A massive military cargo aircraft ploughed into Ridgeline's central saddle and broke in three, replacing the old watch post at C. Its torn cargo hold supplies covered routes through the wreck, while the leaning tail, the folded wing and the long furrow behind it make the crash visible from the surrounding forest.

## The crash

The aircraft came in from the west (-x), touched down well short of the saddle and slid east. Its furrow runs about 50 m back from the tail, streaked dark earth littered with torn skin, tipped troop seats and fragments, past pines snapped at the trunk with their tops lying the way the aircraft travelled. Ahead of the nose the slide pushed two bow waves of earth to either side of the end lane.

The hull lies in its own furrow, its deck cut 1.8 m below the natural saddle, with earth shoved up against both flanks. It broke above both buckled frames (x = +/-12): the forward and aft sections are twisted and pitched off the centre line, the crown buckles upward and is burnt black at each break, and the roof is torn open beside each one. Skin panels are holed, ripped away over bare frames and insulation, or peeled outward high on the walls. The nose has a hole punched in one cheek and a window blown out.

The port wing folded up at the root as the hull rolled, and its outer third tore away; its inboard engine is burnt out and still smoking. The starboard wing's tip crumpled downward and a flap hangs off its hinges. Both outboard engines are gone - the starboard one lies half buried beneath its wing. The fin leans over with its tip torn off, and the port elevator snapped at half span. A main gear bogie and a burst cargo container lie beside the hull.

## Routes and cover

The main fuselage runs along X across the saddle. Both broken ends remain open, with three broad tears on each side so neither team must enter through a single choke point. C remains on the cargo deck and can be reached along or across the hold without jumping or crouching.

There is no straight line along the hold. A buckled frame stands floor to roof across each break: the aft frame blocks all but a 4.5 m gap on the +z side, the forward one all but the same gap on the -z side, so the route through the hold zigzags and neither broken end can see C. Under each roof tear the fallen skin forms a low heap, leaving a nook against the frame. Cargo cases sit at each end, a toppled container stands in each end section and two pallets give C crouching cover against the centre tears.

The cargo deck and every opening are level, with two metres of level apron outside each side tear and a long even grade out of each broken end (under 26 degrees anywhere past either end, against the old pad's 36 degree banks). Where the furrow cuts through the ridge spine its flanks stay below 36 degrees.

## Fairness

Everything a player can touch is shared collision mirrored under a half turn ((x, z) -> (-x, -z)): hull, roof, frames, heaps, cargo, the bogie, nacelle and container (each pair drawn differently but with identical boxes, each on a levelled pad) and the snapped trunks. The terrain - furrow, flank banks and bow waves - is mirrored too. Art that is not collision stays below the knee (logs, seats, panels and fragments on the ground, earth against the hull) or above jump height (crown, wings, tail, nose, peeled flaps, hanging wiring). A client test checks every visible triangle of the wreck between 0.45 m and 3.2 m above standable ground lies within 0.3 m of collision. The smoke rises from both nacelles, so it is mirrored at ground level too.

## Atmosphere

An emergency beacon still pulses inside the wreck. Loose skin panels move in gusts, and a quiet positional metal rattle can be heard nearby. Thin smoke drifts up from both fallen engines and the burnt one on the port wing, and sunlight falls through the roof tears into the hold. These ambient effects respect the effects volume and do not affect movement or damage.

## Review

```powershell
cargo build --release -p ferrite --example asset_review
target/release/examples/asset_review ridgeline aircraft.png wreck 10
target/release/examples/asset_review ridgeline cargo.png cargo 10
target/release/examples/asset_review ridgeline tail.png tail 10
target/release/examples/asset_review ridgeline furrow.png -115,40,45,-30,4,0 10
target/release/examples/asset_review ridgeline tear.png -21,6.2,1.5,-12,10.5,-3 10
```

The fourth argument freezes wind and beacon animation. A custom six-number eye/target camera remains available for other views.

Ridgeline's collision and terrain changed with the crash damage, so it rides on the next protocol bump: everyone in a match needs the same build, including the dedicated server.
