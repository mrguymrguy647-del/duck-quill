# Phase 6 working notes

Numbers and facts gathered along the way, kept here so the reports can quote them.

## Painting speed before tiles are shared (0.5.0 code, commit d24971b)

Measured on the development machine (4 cores, Linux), release build, best of three runs.
`cargo test -p quill_engine --release --test big_canvas -- --ignored --nocapture`:
a 4000 x 3000 picture with 20 layers.

| What | Before (ms) |
|---|---|
| make 20 painted layers | 1471.3 |
| long stroke, size 60, no selection | 92.8 |
| long stroke, size 60, pencil (grain) | 188.5 |
| long stroke, size 60, four-way symmetry | 324.7 |
| blur 8 px, no selection | 647.7 |
| select a big ellipse | 154.4 |
| feather it by 30 px | 137.2 |
| its outline (marching ants) | 41.8 |
| long stroke, size 60, inside the selection | 77.8 |
| paint bucket inside the selection | 33.1 |
| blur 8 px inside the selection | 639.9 |
| copy the selection | 44.3 |
| the whole picture, all 20 layers combined | 782.3 |
| a copy of the document (as autosave would) | 120.9 |

`quill render` of the gallery pieces (best of three): sunset-pond-v3 1.98 s,
forest-at-dawn 2.92 s, duck-portrait-v2 0.81 s.

Run-to-run noise on this machine is about 10 to 15 percent.

## Other measurements

- `cargo build --release -p quill_cli` from nothing: about 36 s; as one file that runs on any
  Linux (`--target x86_64-unknown-linux-musl`): works, 5.9 MB.

## Step 1: before and after, with the real programs

The 0.5.0 `quill` and today's, run on the same scripts (the scripts are the tests'
own: `TILES` and the others in `crates/cli/tests/render.rs`).

| # | What was tried | 0.5.0 | Now |
|---|---|---|---|
| 1 | Getting `quill` on Linux | build it (36 s alone, minutes with the whole workspace) | download `quill-linux-x86_64.tar.gz` (25 MB, one file that needs nothing installed); a test fails if `quill` ever depends on a window library |
| 2 | `quill export` on a script of three tiles, no animations | `error: this script has no animations` | `Exported: 3 stills`, `tiles.png` and `tiles.json` in the tileset format |
| 3 | A script with `speckle` | `unknown variant speckle` | a 16 x 16 grass tile; `render --tile`: "opposite edges meet" |
| 4 | A script with `skin_box` | `unknown variant skin_box` | 12 regions listed by `check`, `skin.regions.json` beside `skin.png` |
| 5 | `check` on the script of tiles | 10 warnings, 9 of them false | 1 warning, the true one (`"spare" is never used`) |
| 5 | `render` on tiles shown by one-frame animations | 6 warnings, 5 false | 1 warning, the true one |
| 6 | `pixels` with keys `g1`, `g2` | `` `colors` keys are single characters `` | paints; a bad cell: `row 2 has "zz" at column 4` |

Also found and fixed while testing these: on a pixel canvas, `check` lost which commands
painted each layer (and could call a painted layer empty in every frame).
