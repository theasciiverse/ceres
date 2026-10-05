# Celeste Classic any% TAS inputs (test fixtures)

Per-level input files from the community TAS project
github.com/CelesteClassic/ClassicTAS (`Celeste/TAS files/Any%/`), made with
its picolove-based TAS tool. Used only by `src/tests.rs` to replay every
level through Ceres's port and check that it completes, frame for frame.
Not shipped in the game.

Format: `[seed,seed,...]` (balloon/chest RNG seeds, in object order), then
one comma-separated number per frame: a bitmask of PICO-8 buttons
(1 left, 2 right, 4 up, 8 down, 16 jump, 32 dash). Inputs start on the frame
the player leaves the spawn animation.
