# Vendored crates

Two dependencies are kept here as patched copies. The workspace `Cargo.toml`
points crates.io at them with `[patch.crates-io]`, so every crate that
depends on them (macroquad included) builds against these copies.

| Folder | Upstream | Version | Licence | What FERRITE changed |
|---|---|---|---|---|
| `miniquad/` | [not-fl3/miniquad](https://github.com/not-fl3/miniquad), from crates.io | 0.4.11 | MIT or Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`, shipped upstream) | Mouse 4 / Mouse 5 support on Windows, X11, Wayland, macOS and web. Details in `miniquad/FERRITE-PATCH.md`. |
| `quad-snd/` | [not-fl3/quad-snd](https://github.com/not-fl3/quad-snd), from crates.io | 0.2.8 | MIT or Apache-2.0 (see below) | Output device selection (Windows; other platforms use the system default), and decoding a sound on a worker thread so loading a long track does not stall a frame. Changes are marked `FERRITE:` in `src/lib.rs`, `src/mixer.rs` and `src/wasapi_snd.rs`. |

Both copies are the crates.io source with registry metadata, lockfiles,
examples and CI files left out.

## quad-snd licence files

quad-snd declares `license = "MIT/Apache-2.0"` in its `Cargo.toml`, but
neither the crates.io package nor the upstream repository includes a licence
text. `quad-snd/LICENSE-MIT` and `quad-snd/LICENSE-APACHE` were added by
FERRITE: they are the standard texts of the two licences it declares, with
the copyright holder taken from the crate's `authors` field (Fedor
Logachev). They do not change the terms; they record them next to the code.

## Updating a vendored crate

1. Copy the new version's source from crates.io over the folder, keeping the
   licence files.
2. Re-apply FERRITE's changes (search for `FERRITE` in the old copy, and read
   `miniquad/FERRITE-PATCH.md`).
3. Update the version in the table above.
4. Run the full CI checks (see `CONTRIBUTING.md`), including the miniquad
   mouse-button test: `rustc --edition=2018 --test vendor/miniquad/src/event.rs -o target/mouse-button-tests && target/mouse-button-tests`.
