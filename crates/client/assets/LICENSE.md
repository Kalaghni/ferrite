# Licence for the files in this folder

The MIT licence in the repository root covers FERRITE's source code. It does
**not** cover the files in this folder, except where a file below says so.
They are here so the game can be built, and every file except this one and
`slag-cathedral.txt` is embedded in the game executable.

Every file in the folder is listed below. A file added here needs an entry,
with its owner and licence, in the same change.

| File | What it is | Owner | Licence |
|---|---|---|---|
| `liquid-plasma.ogg` | "Liquid Plasma", launcher music | breadKernel | All rights reserved (music terms below) |
| `submariner.ogg` | "Submariner", launcher music | breadKernel | All rights reserved (music terms below) |
| `barbed-wire.ogg` | "Barbed Wire", launcher music | breadKernel | All rights reserved (music terms below) |
| `foundrite-lobby.ogg` | "Foundrite Lobby", lobby music | breadKernel | All rights reserved (music terms below) |
| `slag-cathedral.ogg` | "Slag Cathedral", the Slag Cathedral map score | Supplied by the FERRITE project owner (see `slag-cathedral.txt`) | All rights reserved (music terms below) |
| `slag-cathedral.txt` | Provenance and encoding notes for `slag-cathedral.ogg` | the FERRITE authors | MIT, with the source code |
| `launcher-background.png`, `lobby-background.png` | Key art: launcher and lobby backgrounds | the FERRITE authors | All rights reserved (artwork terms below) |
| `ferrite-wordmark.png` | FERRITE logo | the FERRITE authors | All rights reserved (artwork terms below) |
| `emblem-ember.png`, `emblem-cobalt.png` | Team emblems | the FERRITE authors | All rights reserved (artwork terms below) |
| `icon-16.rgba`, `icon-32.rgba`, `icon-64.rgba` | Window icon (raw RGBA, 16/32/64 px), made from the artwork | the FERRITE authors | All rights reserved (artwork terms below) |
| `Quantico-Regular.ttf`, `Quantico-Bold.ttf` | Quantico typeface, unmodified | Matthew Desmond | SIL Open Font License 1.1 |
| `Quantico-OFL.txt` | Full text of the font licence | - | Must ship with the font |
| `LICENSE.md` | This file | the FERRITE authors | MIT, with the source code |

## Music - all rights reserved

Copyright (c) breadKernel ("Liquid Plasma", "Submariner", "Barbed Wire",
"Foundrite Lobby") and the owner of "Slag Cathedral". All rights reserved.

You may play these tracks as part of FERRITE, and keep them in a copy or fork
of this repository so that the game builds. You may not otherwise copy,
redistribute, remix, sample, sell or publish them, or use them in any other
project, video or product, without written permission from the owner.

## Artwork - all rights reserved

Copyright (c) the FERRITE authors. All rights reserved. The key art, logo,
team emblems and window icon may be used as part of FERRITE and in forks of
this repository, and not otherwise without permission. The FERRITE name and
logo may not be used in a way that suggests a modified build is the official
game.

## Typeface - SIL Open Font License

Quantico is copyright (c) 2011 Matthew Desmond, with Reserved Font Name
Quantico, licensed under the SIL Open Font License 1.1. The full text is in
`Quantico-OFL.txt`; a copy, `packaging/Quantico-OFL.txt`, ships beside the
executable in every release. The font may be embedded and redistributed with
the game. A modified version may not be called Quantico.

## Elsewhere in the repository

These non-code files outside this folder are not MIT either:

| File | What it is | Owner | Licence |
|---|---|---|---|
| `packaging/ferrite.ico` | Windows executable and installer icon, made from the artwork | the FERRITE authors | All rights reserved (artwork terms above) |
| `packaging/Quantico-OFL.txt` | Copy of `Quantico-OFL.txt`, shipped with releases | - | Licence text |
| `docs/launcher.png`, `docs/screenshot.png` | Screenshots of the game | the FERRITE authors | No licence granted; all rights reserved. `launcher.png` shows the key art and logo. |
| `docs/maps/relay/*.jpg`, `docs/maps/slag-cathedral/*.jpg` | Map views exported from FERRITE's renderer | the FERRITE authors | No licence granted; all rights reserved |

The vendored crates in `vendor/` keep their own MIT / Apache-2.0 licences;
see `vendor/README.md`. Other Rust dependencies come from crates.io under the
licences allowed in `deny.toml`.

## Contributing art, music or other assets

Assets are not covered by the MIT licence, so a contribution of art, music,
sound or images must say who made it and under what licence it is offered,
and add a row to the table above. Do not submit anything you cannot licence
to the project.
