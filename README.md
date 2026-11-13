# FERRITE 0.3

![A COBALT soldier advancing on point B through the valley](docs/screenshot.png)

An original low-poly multiplayer FPS in Rust. Two teams of twelve, EMBER and COBALT, fight over five control points strung across a valley. Only the front line can be contested, so the battle has a shape: it masses, breaks, and moves.

v0.3 is a from-scratch rewrite. Nothing from the v0.2 prototype is reused (it remains in git history).

## Download

Windows 64-bit builds of the client and the dedicated server are on the [releases page](../../releases). Unzip and run `ferrite.exe`; nothing else to install.

## Run from source

Needs stable Rust. No assets, no setup - sound and geometry are generated in code.

```sh
cargo run --release -p ferrite
```

The game opens on a home screen. **Host and play** starts a match inside the game (24 fighters, bots fill every empty slot). **Join a server** takes an address and a port (default 27500) and remembers servers you have joined. Click the window to deploy.

Skip the home screen, or run a dedicated server:

```sh
cargo run --release -p ferrite -- --host                      # host straight away
cargo run --release -p ferrite -- --connect 192.168.1.20      # join a host (UDP 27500)
cargo run --release -p ferrite-server                         # dedicated server
```

Each human replaces a bot; teams are balanced by human count. Allow UDP 27500 through the host's firewall.

| Key | Action | Key | Action |
|---|---|---|---|
| WASD | move | LMB | fire |
| Shift | sprint | RMB | aim down sights |
| Space | jump | R | reload |
| Ctrl / C | crouch (hold) | G | frag grenade |
| Q / F / wheel | swap primary and sidearm | T | change sight |
| 1 / 2 / 3 | kit for next deploy | | |
| Tab | scoreboard | F3 | fps / ping / prediction stats |
| Esc | release mouse | Q | leave match (only while mouse is released) |
| - / = | mouse sensitivity | | |

Useful flags: `--name`, `--bots N` (players per team), `--skill 0.6` (bot aim), `--tickets N`, `--seed N` (different valley), `--fullscreen`, `--volume 0..1`, `--spectate` (watch the bots fight).

## The game

- **One front.** Points A-E must be taken in order from your side. You can only push a point that touches ground you hold, so both teams always know where the fight is - and flanking means going around it, not ignoring it.
- **Numbers matter.** Capture speed scales with how many more of you are on the point (up to 3). Lone wolves stall; squads take ground.
- **Surge.** Capturing a point gives the whole team eight seconds of faster movement and half-time reloads. A capture is the moment to push the next point, not to rest.
- **Waves and rubber band.** Reinforcements arrive in waves, and the more ground a team holds the longer its waves take. Winning stretches you thin; losing teams hit back together.
- **Tickets.** Every death costs one; holding more points bleeds the enemy. First team to zero loses, then the round resets.
- **Crouch.** Half speed, a much smaller target (you fit behind sandbags and crates), and 40% tighter spread. You cannot stand up under something.
- **Sidearms.** Every kit carries a 12-round semi-automatic pistol. Drawing it takes a quarter of a second - far quicker than a reload - and you move slightly faster with it out.
- **Sights.** Press T to cycle the sight on the weapon in hand: iron sights, red dot, holo, 2.5x optic, and 4x / 8x scopes on the Longrifle. Each has its own model, reticle and zoom; your choice is remembered per weapon.
- **Three kits.** Vanguard (carbine, two frags), Breacher (scattergun, 130 HP, slightly faster), Marksman (longrifle; anyone it hits is marked through walls for the whole team for six seconds).
- **Readable battlefield.** Team-coloured soldiers and tracers, light pillars over every objective, a front-line bar, minimap that shows enemies who fire or are marked, damage direction indicators.
- **Breakable cover.** Wooden crates shatter under fire and grenades, for everyone, authoritatively.

## Architecture and trade-offs

```
crates/shared   deterministic sim (movement, collision, weapons), map generation, wire protocol
crates/server   game rules, bots, UDP front end; also a library the client embeds to host
crates/client   window, renderer, prediction, effects, synthesised audio, HUD
```

- **Server authoritative, client predicted.** Clients send only intent (buttons and view angles) at 60 Hz. The server runs the same `sim::step` the client used to predict, so with no loss prediction is bit-exact, and after loss the client replays its unacknowledged commands on top of the next snapshot. Position is never accepted from the network.
- **Trust boundary.** Commands are bounds-checked and sequence-checked, and a per-player budget caps how much simulated time a client can claim (a flood of 600 commands moves you under 3 m, not 80). Fire rate, ammo and hits are all decided server-side. Packet decoding is size-limited and cannot panic.
- **Lag compensation.** Hitscan is resolved against where targets were on the shooter's screen, using a per-player position history, capped at 0.5 s of rewind.
- **Loss tolerance.** Every command rides in three consecutive packets. Kills, captures and round state use an acknowledged feed that is resent until confirmed; tracers and hit sparks are sent once because losing one costs nothing.
- **Snapshots.** 30 Hz, fixed-int bincode, about 550 bytes for 24 players (measured ~16 kB/s per client), under one MTU. Remote players are interpolated 100 ms in the past.
- **Bots are clients.** A bot produces the same command struct as a human and goes through the same validation. They navigate with precomputed flow fields (three lane preferences per objective), follow a shared team push order, and have reaction time and aim error that settles while tracking.
- **Rendering.** macroquad provides the window, input, audio and 2D HUD; the 3D scene uses raw miniquad pipelines with a custom flat-shading and fog shader. Chosen over Bevy for a 15-second rebuild and a small, stable API surface; the cost is no shadow maps or post-processing (blob shadows instead).
- **Simple by choice.** Axis-aligned collision boxes on a heightfield; no player-player collision (keeps prediction exact); UDP source address is the session identity with no encryption or authentication - LAN/VPN only, not Internet-hardened.

## Verification

```sh
cargo test --workspace                                            # 7 integration tests
cargo run --release -p ferrite-server -- --bench 900              # headless 15-minute bot war + timing
cargo run --release -p ferrite -- --spectate                      # watch a match
cargo run --release -p ferrite -- --auto --quit-at 30             # scripted client, prints session stats
```

The tests cover: prediction equals server state exactly over 900 random commands; replay after delayed delivery converges; command-flood rate limiting; NaN/out-of-range/replayed input and malformed packets; a five-minute 12v12 soak asserting territory stays contiguous, nobody leaves the map or falls through terrain, and snapshots stay under 1400 bytes; and a real UDP join / drive / acknowledge / leave round trip.

Every client prints a line on exit with frame times, ping, snapshot count, bandwidth and the number of prediction corrections over 1 cm. `--shot file.png --shot-at SECS` saves a frame.

Note on the ping readout: acknowledgements ride on 30 Hz snapshots, so loopback shows about 15 ms. It is an upper bound on round trip, not a precise measurement.

License: MIT.
