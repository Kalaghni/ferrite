# FERRITE 0.2.2 — Rust Conquest FPS

Original low-poly multiplayer FPS with a headless, authoritative UDP server and
Macroquad graphical clients. This release compiles and has actual Linux runtime
evidence. It remains an experimental LAN/VPN prototype.

## Run

Install current stable Rust. In separate terminals at the project root:

```sh
cargo run --locked --bin ferrite-server
cargo run --locked --bin ferrite-client -- 127.0.0.1:4000 PlayerOne
cargo run --locked --bin ferrite-client -- 127.0.0.1:4000 PlayerTwo
```

WASD moves, Shift sprints, hold left mouse to fire, and Escape releases the mouse.
Click the window to capture the mouse. Teams are assigned automatically.
For a trusted LAN, bind the server to `0.0.0.0:4000` and use its LAN IP in the
client command. UDP/4000 is the default port. Run matching client/server builds.

## What is implemented

- Shared 20 Hz horizontal movement and cover collision.
- Server input acknowledgements, local prediction, unacknowledged-command replay,
  smoothed corrections, and 120 ms delayed remote-player interpolation.
- Authoritative ray/AABB hitscan, occluding cover, 25 body / 75 head damage,
  kills/deaths, hit feedback, and four-second respawns.
- Three contested capture points, conquest scoring, round victory and reset.
- Eight server bots and up to 24 human sessions.
- Opt-in real-client telemetry and seeded network-impairment testing.

## Verification

See `MEASUREMENTS.md` for measured results and limits, `HANDOFF.md` for the next
engineering gate, and `verification/` for the raw output packaged with this release.
The source includes 17 shared-simulation tests and a client telemetry correlation
test. The GitHub Actions workflow checks Linux and Windows when the project is
pushed; execution in this workspace is Linux only.

Unix:

```sh
./scripts/check.sh
```

Windows PowerShell (each command should succeed):

```powershell
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --bins
python scripts/smoke.py --spawn
```

The UDP smoke harness starts and stops its own real server with `--spawn`.
To test an existing server instead: `python scripts/smoke.py 127.0.0.1 4000`.

## Measure a real client

Set a different output path for each client. Telemetry is disabled by default.

```powershell
$env:FERRITE_METRICS_PATH = "player-one.jsonl"
cargo run --locked --bin ferrite-client -- 127.0.0.1:4000 PlayerOne
```

```sh
FERRITE_METRICS_PATH=player-one.jsonl cargo run --locked --bin ferrite-client -- 127.0.0.1:4000 PlayerOne
```

JSONL records contain replay-aligned position error in metres, whether a correction
exceeds 1 cm, pending inputs, discontinuities, and matched fire-command-to-HUD
timings. Human hit events carry the source command sequence. Older v2 events
without that optional field are accepted but cannot produce matched latency.

The graphical tools require Linux, Xvfb, xdotool, ImageMagick `import`, and working
software OpenGL. They run two actual graphical clients and apply XTest controls:

```sh
cargo build --locked --bins
python3 scripts/graphical_soak.py --duration 600 --output verification/soak --require-combat
python3 scripts/client_metrics_lab.py
python3 scripts/summarize_metrics.py verification/soak
```

Use `--xvfb` and `--xdotool` if those executables are outside PATH. The matrix uses
0/50/100/200 ms **added RTT**, split equally between uplink and downlink, and
independent 0/2/5% loss on each leg. Seeds fix loss decisions; OS scheduling and
rendering mean repeated measured timings will vary. `scripts/network_lab.py` is
an additional headless clear-lane probe, with an analytic one-axis predictor;
its results are separate from the graphical client's telemetry.

## Remaining limits

- UDP source address identifies a session. No authentication, encryption,
  session tokens, NAT rebinding, or Internet-facing abuse protection. LAN/VPN only.
- No input-clock synchronization: the server can repeat a held command or
  coalesce incoming commands. Prediction can drift under loss and jitter.
- No shot rewind. Moving-target and high-ping combat fairness need more work.
- Combat events last one snapshot window. A dropped snapshot can lose feedback.
- Snapshot JSON is not optimized for bandwidth or IP fragmentation.
- Bots have simple steering and may get stuck on cover. Movement is horizontal;
  there is no jumping, vehicles, destruction, sound, or finished art.
- Software-rendered, scripted Linux checks do not establish subjective game feel,
  Windows runtime correctness, monitor latency, or production-scale capacity.

License: MIT. No EA code or assets are included.
