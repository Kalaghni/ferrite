# FERRITE 0.2.2 — Measured client prediction and hit feedback

Tested November 12, 2026 (America/Toronto), on Linux with Rust/Cargo 1.100.0.
Two actual Macroquad graphical clients connected through seeded UDP impairment proxies to the real 20 Hz authoritative server. Xvfb, software OpenGL, and XTest supplied rendering and controls. Each matrix scenario lasted approximately 35 seconds and included movement, a player-versus-player body-shot kill, and a respawn. These are debug builds.

## Real graphical-client measurements

| Added RTT (ms) | Loss per leg | Prediction error mean / P95 / max (m) | Corrections/s while moving | Replays/s | Hit feedback median / max (ms) | Feedback seen / server hits | First-trigger feedback (ms) |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 0 | 0% | 0.005 / 0.000 / 0.400 | 0.12 | 10.03 | 75 / 107 | 4/4 | 102 |
| 0 | 2% | 0.085 / 0.400 / 0.400 | 2.08 | 9.85 | 57 / 71 | 4/4 | 50 |
| 0 | 5% | 0.183 / 0.400 / 1.200 | 3.93 | 9.60 | 55 / 159 | 4/4 | 91 |
| 50 | 0% | 0.005 / 0.000 / 0.400 | 0.12 | 10.03 | 98 / 137 | 4/4 | 137 |
| 50 | 2% | 0.162 / 0.400 / 0.800 | 3.71 | 9.81 | 126 / 154 | 4/4 | 215 |
| 50 | 5% | 0.035 / 0.400 / 0.800 | 0.72 | 9.53 | 103 / 154 | 3/4 | 263 |
| 100 | 0% | 0.014 / 0.000 / 0.400 | 0.36 | 10.03 | 165 / 230 | 4/4 | 230 |
| 100 | 2% | 0.116 / 0.400 / 0.800 | 2.72 | 9.81 | 163 / 167 | 4/4 | 159 |
| 100 | 5% | 0.096 / 0.400 / 0.800 | 2.22 | 9.50 | 145 / 198 | 4/4 | 198 |
| 200 | 0% | 0.081 / 0.400 / 0.400 | 2.04 | 10.03 | 259 / 303 | 4/4 | 303 |
| 200 | 2% | 0.048 / 0.400 / 0.400 | 1.19 | 9.88 | 248 / 298 | 4/4 | 237 |
| 200 | 5% | 0.050 / 0.400 / 0.800 | 0.96 | 9.45 | 314 / 338 | 2/4 | 338 |

**Observed feedback defect:** the 50 ms/5% case lost one of four human hit confirmations, and the 200 ms/5% case lost two of four. In the latter case, the missing events were carried in server snapshots at ticks 524 and 530; neither tick appears in the shooter’s received-snapshot telemetry. All four authoritative 25-damage events were present at the direct observer. See `feedback-loss-proof.json` in that scenario. These small samples demonstrate a loss mode; they do not estimate its long-run probability.

## Metric definitions

- **Prediction error:** distance between the client’s existing predicted position and the authoritative position after replaying its still-unacknowledged inputs. Measured before camera smoothing, at each received snapshot. Health changes and teleports are excluded from the movement distribution. This is replay-aligned error, not the distance to a stale server position.
- **Correction frequency:** movement snapshots with error greater than 0.01 m, divided by client time classified as moving. Both clients’ exposures are combined. Normal snapshot replays are counted separately; they occur even when position error is zero.
- **Hit feedback:** local fire-input sampling to submission of the corresponding HUD feedback draw. The server includes the exact fire-command sequence in the hit event. Unmatched events have no latency value. **First-trigger feedback** additionally includes the wait from button-press sampling to the first command that produces a visible hit confirmation.
- **Feedback seen/server hits:** unique local HUD events compared with human-fired events observed by a separate direct telemetry socket. Missing confirmations indicate lost feedback in this test, rather than a failed authoritative damage calculation.
- RTT is **added round-trip delay**, split equally between each direction. Loss is independently applied to each leg, not an end-to-end combined loss rate. Seeded packet decisions are reproducible; frame timing, scheduling, command phase, and observed results vary between runs.

## Sample sizes and practical interpretation

Each case has 158–169 moving snapshot samples, but only 2–4 matched hit samples. Hit maximum is reported instead of implying a reliable tail estimate; with four hits the nearest-rank P95 equals the maximum. A single first-trigger sample per burst is not a production latency estimate.

The 0.4 m error step corresponds to one 50 ms walking tick at 8 m/s. The code repeats the latest held input and can coalesce arriving commands; it has no rigorous client/server input-clock alignment. Those are plausible sources of measured corrections. This is an inference from the timing design and measurements, not an isolated causal experiment. Loss cases are not expected to rank monotonically in short runs.

Hit tests use a stationary target in a clear lane. These results do not measure moving-target fairness, miss rates caused by interpolation, or lag-compensated rewind. Remote actors are visually delayed by 120 ms and the server still tests current positions. Input-device polling, GPU presentation, compositor delay, monitor scanout, and subjective perceived responsiveness are outside the HUD-submission clock. Software rendering and concurrent test load also affect timings.

## Endurance and correctness evidence

- Two graphical clients plus a direct telemetry observer stayed connected for **600.35 seconds**; the observer processed **5987 snapshots** and reached server tick **12000**. All monitored processes remained alive until test cleanup.
- Both graphical clients moved through actual keyboard input. Player-versus-player kill and human respawn assertions passed. The run observed 1405 combat events. Screenshots, server logs and client JSONL are in `verification/soak/`.
- Formatting, locked all-target compile, **18 Rust tests**, strict Clippy, and real two-socket UDP integration passed. Tests include cover occlusion, body/head damage, stale-input rejection, and exact latency-command correlation. Native Windows execution was not available; a Windows CI job is included for the next repository push.
- A separate earlier headless matrix also passed all 12 RTT/loss cases. Its one-axis analytic predictor is documented separately in `verification/network-lab.json`; use the actual graphical-client table above for this request.

## Reproduce

```sh
cargo build --locked --bins
python3 scripts/client_metrics_lab.py
python3 scripts/graphical_soak.py --duration 600 --output verification/soak --require-combat
python3 scripts/summarize_metrics.py verification/soak verification/client-matrix/rtt-200-loss-5
```

For a normal client on Windows, set `$env:FERRITE_METRICS_PATH = "player-one.jsonl"` before launching it. Use a distinct file for each process. Telemetry is off by default. See README for prerequisites and other platforms.

Next priorities: align input clocks and command consumption, make combat-event delivery loss-tolerant, then add bounded lag-compensated shot rewind. Preserve these measurements as the baseline before changing netcode.
