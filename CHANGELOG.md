# Changelog

## 0.2.2 — Compiled verification and client measurements

- Compile, format, test, and strict-clippy verification with a locked dependency set.
- Correct degree/radian mismatch in the camera field of view.
- Correct camera-relative strafe and horizontal mouse-look directions.
- Initialize each team's view toward the map center after joining.
- Add opt-in real-client JSONL telemetry for prediction, corrections, and hit feedback.
- Attach an optional source input sequence to human combat events for exact timing correlation.
- Reject false latency pairing when an event has no matching command.
- Extend UDP integration to check stale-command state, stopping, and malformed packets.
- Add real graphical client endurance, combat, seeded RTT/loss, and summary tools.
- Check formatting rather than rewriting it in CI; include UDP integration and a Windows job.
- Package actual measurements and their limits with the source.

## 0.2.1 — Client compile diagnostic hotfix

- Use `enumerate()` for A/B/C objective HUD labels, fixing undefined `i` (E0425).

## 0.2 — Unverified engineering candidate

- Versioned protocol, fixed-step movement, prediction/reconciliation and interpolation.
- 3D hitscan, body/head damage, event feedback, contested capture and timed respawns.
- Initial shared tests, smoke script, and CI workflow.
