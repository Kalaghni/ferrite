#!/bin/sh
set -eu
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --bin ferrite-server
python3 scripts/smoke.py --spawn
printf '\nAll build, unit, and UDP checks passed.\n'
