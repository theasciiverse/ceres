# Ceres

**3000 METRES. ONE GIRL. ONE DASH.**

At the foot of Ceres Mountain stands a memorial to those who perished on the
climb. Ceres starts up anyway: thirty-one screens of ice, spikes and wind, a
hundred metres at a time. Jump, dash in eight directions, cling to the rock
and kick off again; bounce off springs, grab balloons, race the crumbling
snow, hunt the cherries, and find the golden sun-seed that gives you a second
dash. You will fall a lot. The summit tells you exactly how many times.

Super-tight platforming, true to the frame: all 31 levels play exactly as in
the original, checked frame by frame against it, with all-new art and music.

In 2015 Maddy Thorson and Noel Berry made Celeste Classic for the PICO-8 in
four days, filling the cart almost to the last byte (8186 of 8192 code
tokens, the whole sprite sheet and map, 63 of 64 sounds). It grew into the
acclaimed Celeste (2018). Ceres restores it for the terminal: their code and
levels, new art and music.

## Play

With [av](https://asciiverse.sh), the Asciiverse client:

```
av install ceres
av play ceres
```

Or download the archive for your system from this repository's Releases,
unpack it and run `./ceres` (`ceres.exe` on Windows).

To build it yourself: `cargo run --release`.

Needs a terminal that reports key releases (the kitty keyboard protocol:
kitty, foot, WezTerm, Ghostty, Alacritty, iTerm2 and others), or a gamepad.

## Keys

| Key | |
|---|---|
| arrows, W A S D | move |
| Z, C, N, Space | jump |
| X, V, M | dash (eight ways) |
| Esc, P | menu |
| R / Q | restart / quit |
| K / A / F8 | screen shake / pixel shape / sound |

## Options

`--level N` (1 to 31; 31 is the summit), `--no-shake`, `--sharp` (exact 1:1
pixels), `--mute`.

Session log for bug reports: `RUST_LOG=debug ceres 2>ceres.log`

## Credits

Celeste Classic is by Maddy Thorson and Noel Berry (PICO-8, 2015). The code
and levels come from Noel Berry's C# port (github.com/NoelFB/Celeste, MIT
licence), checked against the original Lua. The art and music are new.
