# Contributing

Issues and pull requests are welcome. This page is what you need to get a
change through CI and, if you are cutting one, a release. Security problems
go through private reporting instead: see [SECURITY.md](SECURITY.md).

## Building

Stable Rust is all you need on Windows and macOS:

```sh
cargo run --release -p ferrite          # game and launcher
cargo run --release -p ferrite-server   # dedicated server
```

On Linux, install the window, input and audio headers first (Debian and
Ubuntu names; CI uses these on Ubuntu 22.04):

```sh
sudo apt-get install libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev libudev-dev pkg-config
```

The README's "Run from source" section lists the game's command-line flags.

## Checks CI enforces

Every push and pull request runs these; run them before you push:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --release
cargo deny check            # advisories, bans, licences, sources (needs cargo install cargo-deny)
```

Tests run on Windows, Linux and macOS, and CI also smoke-tests the packaged
server (`ferrite-server --bench 120 --bots 12`). If you touch
`vendor/miniquad`, also run its mouse-button test (the command is in
`vendor/miniquad/FERRITE-PATCH.md`).

Formatting follows `rustfmt.toml` (max width 140); clippy settings are in
`clippy.toml`. A new dependency must have a licence `deny.toml` allows and
come from crates.io. Do not add an advisory to the `ignore` list without a
comment saying why it does not affect FERRITE.

## Code style

- Let rustfmt format it; do not fight it.
- Comments say why, not what, in plain English. A reader should learn the
  reason for a number or a branch, not a restatement of the code.
- The server is the authority. Never accept a position, a hit or a score from
  the network; take inputs and decide on the server.
- Anything decoded from the network is size-limited and must not panic on bad
  input.

## Protocol changes

Clients and servers must agree exactly on the wire format and on the map
(the client predicts movement against its own copy of the collision). Any
change to `PState`, `NetPlayer`, `Feed`, `C2S` / `S2C`, `Snapshot`, or to map
collision or generation needs `PROTO` bumped in
`crates/shared/src/proto.rs`, with a one-line doc comment saying what
changed. Mention it in the release notes ("everyone in a match needs vX.Y.Z,
including the dedicated server"), and redeploy the dedicated server
(`deploy/`) with the release.

## Maps

- **Balance by symmetry.** Playable geometry (anything with collision, and
  spawns and objectives) is mirrored under a half turn, so both teams get the
  same routes and cover.
- **Fingerprints.** `crates/shared/src/map.rs` pins every map's terrain,
  collision, trees, objectives and crates with a fingerprint test. If you
  change a layout on purpose, update the fingerprint in the same change and
  say why in the commit; an unexpected failure means you changed a map you did
  not mean to.
- **Triangle budgets.** The scene modules in `crates/client/src/scene/` have
  tests that bound each map's mesh size. Profile on a modest machine
  (`F3` shows fps) before raising a budget, and say what you measured.
- Map behaviour tests (walkability, mirroring, clear spawns, navigation) live
  in `crates/server/tests/`; the map guides in `docs/maps/` describe them.

## Checking visuals

Tests do not tell you whether something looks right. Two ways to look:

```sh
# a real bot match, spectated, saving one frame at 20 s
cargo run --release -p ferrite -- --spectate --shot f.png --shot-at 20 --quit-at 21

# deterministic renders with no server and no saved settings
cargo run -p ferrite --example asset_review -- players target/players.png
cargo run -p ferrite --example asset_review -- relay target/relay.png
```

The README's "Verification" section lists the `asset_review` subjects, and
`docs/maps/landmarks.md` has the camera views for each landmark. Put
before/after images in the pull request for any visual change.

## Assets

The MIT licence covers the code only. Music, artwork and other assets are
listed one by one, with owner and licence, in
`crates/client/assets/LICENSE.md`. If you contribute art, music, sound or
images, state who made them and the licence you offer them under, and add
them to that file. Do not submit anything you do not have the right to
licence to the project.

## Releases

For version X.Y.Z:

1. Bump `version` under `[workspace.package]` in `Cargo.toml`, and run a
   build so `Cargo.lock` follows. CI refuses a tag that does not match the
   workspace version, and the server's Docker build uses `--locked`.
2. In `README.md`: the title (`# FERRITE X.Y.Z`), a `## vX.Y` section at the
   top of the changes, and the version in the macOS `curl` download URL.
3. In `packaging/README.txt`: a `NEW IN vX.Y` section at the top.
4. Write `docs/release-notes/vX.Y.Z.md`.
5. Commit, tag `vX.Y.Z` and push the tag.

CI builds and tests all three platforms and publishes the release (with
`SHA256SUMS`, which the updater checks) only after every platform and the
lint job pass. The release is created with generated notes; replace them:

```sh
gh release edit vX.Y.Z --notes-file docs/release-notes/vX.Y.Z.md
```

If the protocol changed, redeploy the dedicated server with the release.
