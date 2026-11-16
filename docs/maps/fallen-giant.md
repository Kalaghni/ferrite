# Ridgeline — The Fallen Giant

A massive military cargo aircraft lies across Ridgeline's central saddle, replacing the old watch post at C. Its torn cargo hold supplies covered routes through the wreck, while the tail and broken wings make the crash visible from the surrounding forest.

## Routes and cover

The main fuselage runs along X across the saddle. Both fractured ends remain open, with three broad tears on each side so neither team must enter through a single choke point. C remains on the cargo floor and can be reached along or across the hold without jumping or crouching.

The cargo floor and approaches are level terrain. Shared collision defines the roof, intact hull sections and cargo cover. The nose and tall tail have different visual silhouettes; the playable cover and openings are mirrored under a half turn. The existing rock shelters, ranger lodges and wind turbines remain elsewhere on the map.

## Atmosphere

An emergency beacon still pulses inside the wreck. Loose skin panels move in gusts, and a quiet positional metal rattle can be heard nearby. These ambient effects respect the effects volume and do not affect movement or damage.

## Review

```powershell
cargo build --release -p ferrite --example asset_review
target/release/examples/asset_review ridgeline aircraft.png wreck 10
target/release/examples/asset_review ridgeline cargo.png cargo 10
target/release/examples/asset_review ridgeline tail.png tail 10
```

The fourth argument freezes wind and beacon animation. A custom six-number eye/target camera remains available for other views.

Ridgeline's collision and terrain changed in v0.11.0 (protocol **18**), so everyone in a match needs v0.11.0, including the dedicated server.
