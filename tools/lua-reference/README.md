# Lua reference tracer

Runs the original Celeste Classic Lua (from the TAS tool's cart,
github.com/CelesteClassic/ClassicTAS, `Celeste/CelesteTAS/celeste.p8`, a copy
is in `reference/celeste-classic/celeste-tas-cart.p8`) headlessly, with a
minimal PICO-8 shim and the same TAS input feed as `src/tests.rs`, and prints
the player's state every frame. Used to check the Rust port frame for frame.

- `cart.lua`: the cart's Lua with PICO-8 shorthand (`+=`, `!=`, `if (c) x`)
  rewritten as plain Lua.
- `cartmap.bin`, `cartflags.bin`: the map and sprite flags read from the
  cart's `__map__`, `__gfx__` and `__gff__` sections (identical to
  `data/map.bin`; one flag differs, on a title-logo tile).

Needs a Lua 5.3 (`texlua` from TeX Live works). From this folder:

    texlua ref.lua 1 ../../tests/tas-any 400 > lua1.txt
    CERES_TRACE_DIR=$PWD cargo test --release write_traces -- --ignored

then compare `rust1.txt` with `lua1.txt`. Checked 2026-10-05: all 31
levels identical on every frame except the last (where the TAS cart sets
`beat_level` instead of loading the next room).
