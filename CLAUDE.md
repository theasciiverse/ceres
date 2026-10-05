# Ceres

Asciiverse restoration of **Celeste Classic** (Maddy Thorson and Noel Berry,
PICO-8, 2015), the precision mountain-climbing platformer that grew into
Celeste (2018). Rust + Ratatui, half-block pixels.

## Name

A tribute to David's niece, Ceres Aurora, and a name that sits right next to
Celeste: the same opening sound, and both belong to the heavens. Ceres is the
Roman goddess of grain and harvest, and the largest object in the asteroid
belt, now classed as a dwarf planet.

Name check (2026-10-04): *Ceres* (Jötunn Games / Iceberg Interactive, 2015)
is a space fleet-combat game on Steam, and *Surviving Ceres* (2025) a survival
base-builder. Neither is a platformer; the name is kept deliberately.

An original platformer of our own is planned as a sibling; its name and
concept are still to come.

Renaming is required, not just tasteful: "Celeste" is the name of a commercial
game. For a while this project was called Aurora; the ski game that held
that name first is now Skare (`games/skare/`).

## Sources (checked 2026-10-04)

- **github.com/NoelFB/Celeste**, Noel Berry's official repo, MIT licence
  ("Copyright (c) 2018 Noel Berry").
  - `Source/PICO-8/Classic.cs` (1782 lines): Celeste Classic in C#, "as close
    to 1-1 with the original LUA code as it can". Player (run, jump, dash,
    wall jump, hair), springs, balloons, crumbling floors, strawberries and
    flying strawberries, keys and chests, moving platforms, fake walls,
    messages, the big chest and orb, the flag, room titles.
  - `Source/PICO-8/Emulator.cs`: the PICO-8 stand-in, including the level
    tilemap as hex data (128 tiles wide). So the levels are in the MIT repo.
  - `Source/PICO-8/Graphics/`: `atlas.png`, `consolebg.png`, `font.png`,
    `logo.png`.
  - No audio: music and effects are calls into the full game's audio
    (`Audio.Play("event:/classic/sfx" + n)`), and no sound files are in the repo.
  - The README: "the MIT License only applies to the code in this repo and
    does not include the actual commercial Celeste game or assets." The PICO-8
    readme: "The PICO-8 Name and Logo is owned by Lexaloffle Games LLP".
- The original cart and its Lua: lexaloffle.com/bbs/?tid=2145.
- Prior ports: a ZX Spectrum Next port (github.com/dcrespo3d/CelesteNext)
  states it was made with Noel Berry's permission; ccleste
  (github.com/lemon32767/ccleste) is a C port for 3DS and PC.

**What is clear:** the code and the level data are MIT. **What is not:**
whether the `Graphics/` sprite sheet counts as code in the repo or as the
game's assets under the README note. Options: ask Noel Berry (others have),
or draw our own sprites at the same 8x8 scale. Sound will be ours either way
(synthesised, as in Panzer).

## Why it fits the terminal

A PICO-8 screen is 128x128 pixels. In half-block pixels (`▀`, two pixels per
cell, as in Panzer and Calvary) that is exactly 128 columns by 64 rows: every
PICO-8 pixel gets its own half cell, no scaling. Each room is 16x16 tiles of
8x8 pixels.

## The hard part: input

Celeste Classic reads its buttons once per 30 Hz frame and lives on press
edges: a 4-frame jump buffer, 6 frames of coyote grace, and a dash whose
direction is whatever is held on the frame dash goes down. (There is no
variable jump height: the Lua has no jump cut on release; an earlier draft
of this file said otherwise.) A terminal without the kitty keyboard
protocol only sends key repeats, so holds and releases are guesses. Ceres
uses exact presses and releases when the terminal reports them (kitty, foot,
WezTerm, Ghostty and others), latches a press until the next frame reads it
so a sub-frame tap still counts, takes a gamepad via gilrs, and otherwise
says plainly at start-up that it will feel wrong before letting you play
with the Panzer repeat heuristic.

## Layout

- `src/classic.rs`: the game, ported from `Classic.cs` and checked line by
  line against the original Lua (where they differ, the Lua wins; the list
  is in the module doc).
- `src/pico.rs`: the PICO-8 stand-in (screen, sprites, flags, map, `rnd`,
  turn-based `sin`/`cos`, floored `%`). Repaints the title logo as CERES.
- `src/render.rs`: half-block renderer, 1:1 at 128x64 cells, whole-number
  zoom above, nearest-pixel shrink below. `src/font.rs`: our own 3x5 font.
- `src/input.rs`, `src/audio.rs` (synthesised effects), `src/music.rs`
  (the soundtrack), `src/main.rs`.
- `data/map.bin`: the 128x64 tilemap decoded from `Emulator.cs`; identical
  byte for byte to the original cart's `__map__`/`__gfx__` data.
- `assets/atlas.png`: Noel Berry's sprite sheet, a stand-in (see Art).
- `tests/tas-any/`: the community any% TAS, one file per level (fixtures).
- `tools/lua-reference/`: runs the original Lua headlessly for frame diffs.

## Verification (2026-10-05)

`cargo test --release` replays the community any% TAS
(github.com/CelesteClassic/ClassicTAS) through all 31 levels; every level is
beaten. Beyond that, the player's position, speed and sub-pixel remainder
were diffed against the original Lua cart run headlessly
(`tools/lua-reference/`) on every frame of all 31 levels: identical, 2,998
frames, largest difference 0. The simulation uses `f64` like picolove's
Lua, which the TAS was made on (real PICO-8 is 16.16 fixed point; not
modelled).

## What the licences cover (checked 2026-10-05)

- Code and levels: MIT. The level map is not an asset file but a string
  constant in `Emulator.cs`, source code in Noel Berry's repo, so it falls
  under the repo's MIT licence like `Classic.cs`. The README's exclusion is
  "the actual commercial Celeste game or assets".
- The original PICO-8 cart on the Lexaloffle BBS (tid=2145) is marked "No
  License", so the MIT repo is the route to the levels, and it covers them.
- The sprite sheet (`Graphics/atlas.png`, an image file) is the one item
  that may count as an asset; see below.

## Art

`assets/atlas.png` is still Noel Berry's sprite sheet from the MIT repo,
whose README says the licence covers the code, not "the actual commercial
Celeste game or assets". It is a stand-in until either Noel Berry says yes
(others have asked; CelesteNext was made with his permission) or we draw our
own 8x8 sprites into the same sheet layout (only the PNG changes). The title
logo is already ours (CERES, painted in `pico.rs`), and so is the font.

## Next

1. Art: drawing our own sheet in `src/art.rs`, slot by slot over the stand-in.
   Done: the climber (Ceres: blond, plum dress; hair blond with a dash,
   red when spent, white and gold flashing with two), and the cherries
   (the collectible, plain and winged).
2. Gamepad play-test; listen through the soundtrack and tune it.

## Status

Playable, 2026-10-05: all 31 levels, title, berries, keys and chests,
balloons, springs, crumbling floors, moving platforms, the big chest and the
second dash, the summit flag with time and deaths; synthesised sound
effects and an original synthesised soundtrack (title, climb, wind, Old
Site, summit, cued where the original changes music); kitty-protocol and
gamepad input; square-pixel correction from the font's real cell shape
(`a` in the menu, `--sharp` to start sharp); pause menu.
Run: `cargo run --release` (`--level N`, `--no-shake`, `--mute`;
session log with `RUST_LOG=debug ceres 2>ceres.log`). Own git repo, private
at github.com/theasciiverse/ceres.
