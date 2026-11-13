# FERRITE 0.2.2 — Verified baseline and measurement handoff

## Goal and current scope

Original Rust low-poly Conquest FPS. Keep the focus on correct multiplayer infantry movement and gunplay. The latest request was to measure client prediction error, reconciliation frequency, and perceived hit-registration latency. Actual graphical-client measurements are in MEASUREMENTS.md.

## What was fixed

- v0.2.1 already contained the undefined HUD `i` fix.
- Corrected camera field-of-view degrees/radians, camera-relative strafing, and horizontal mouse-look direction.
- Corrected first-join team view after the initial command overwrites server spawn yaw.
- Fixed all strict Clippy warnings, formatted the code, and included Cargo.lock.
- Added opt-in client JSONL metrics and optional human-shot input-sequence correlation in combat events.
- Extended real UDP integration and added graphical gameplay, deterministic RTT/loss proxies, and measurement summaries.
- CI checks Linux and Windows, formatting, compile, tests, Clippy, and real UDP integration when pushed. No remote CI execution is claimed here.

## Evidence actually obtained

- Linux Rust/Cargo 1.100.0: cargo fmt --all -- --check; cargo check --locked --all-targets; cargo test --locked --all-targets; cargo clippy --locked --all-targets -- -D warnings — PASS.
- 17 shared simulation/protocol tests and 1 client timing/correlation test — PASS.
- python3 scripts/smoke.py --spawn — PASS against a real server: teams, objectives, movement, acknowledgements, stale input state, stopping and invalid packets.
- Two actual graphical Macroquad clients: 600.35 seconds, 5987 observed snapshots, tick 12000, real keyboard/mouse gameplay, player kill, and human respawn — PASS.
- Twelve real graphical-client scenarios: 0/50/100/200 ms added RTT crossed with 0/2/5% independent loss per leg — PASS. Raw client metrics, proxy counts and server events are packaged.
- Separate headless movement matrix: all twelve cases — PASS. Its analytic predictor is not a replacement for real-client measurements.
- Native Windows runtime and subjective human game-feel assessment have not been performed.

## Measurement contract

Prediction error is the replay-aligned position delta before visual smoothing. Corrections exceed 1 cm; snapshot replays occur near 10 Hz even with no position correction. Hit timing matches the source command and stops at HUD draw submission, excluding device/display latency. Confirmed hits use stationary targets in an open lane. Each short matrix case has only a few hit samples; do not represent its maximum as a reliable production P95. Unknown or older shot sequences produce null latency rather than guessed correlation.

## Reproduce

1. Install current stable Rust; use the included lockfile.
2. Run ./scripts/check.sh (Unix), or the individual locked commands in README on Windows.
3. Enable FERRITE_METRICS_PATH separately for each client when collecting manual gameplay telemetry.
4. On Linux with Xvfb, xdotool, ImageMagick and software OpenGL, run scripts/client_metrics_lab.py and scripts/graphical_soak.py --duration 600 --output verification/soak --require-combat.
5. Re-run scripts/summarize_metrics.py on the resulting directories.

## Remaining engineering risks and next gate

- No rigorous input-clock alignment. The server repeats held commands and can coalesce new ones; prioritize synchronized command consumption and input redundancy.
- Combat events are transient for one snapshot window. Add bounded replay/acknowledgement so packet loss cannot silently remove feedback.
- No lag-compensated rewind. Bound history and rewind time, then test moving targets and fairness at realistic RTT/jitter/loss.
- UDP address is identity. No authentication, encryption, session tokens, NAT rebinding or Internet-facing abuse protection. LAN/VPN only.
- JSON snapshots can fragment; bots have simple steering; movement is horizontal; art/audio are placeholders.
- Preserve the baseline, measure changes against it, and keep Windows/native GPU and human playtesting as explicit follow-up gates. Do not add vehicles or destruction before resolving core netcode behaviour.

Quality bar: actual compiled and runtime evidence with definitions and sample counts.
