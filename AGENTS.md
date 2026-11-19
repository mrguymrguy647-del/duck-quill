# Duck Quill, for AI agents

Duck Quill is a painting program written in Rust: layers, brushes, pixel
art and short animations. A **painting script** (a JSON file) describes a
picture step by step, and `quill` paints it into a PNG and a `.quill` file
(the layers), with no window. `DuckQuill` is the same engine with a
window, for people. Duck Mold starts it to paint one face of a 3D model.

## Painting with it

Read `docs/QUICKSTART.md` first: one page, with a first script and the
loop. `docs/AI_GUIDE.md` has the workflow and techniques,
`docs/script-format.md` every command, and `skills/duck-quill/SKILL.md`
is the same know-how as a Claude skill. `quill` has no guide command:
`quill help` lists every command and option.

```bash
cargo build --release -p quill_cli     # once; the program is target/release/quill
quill help                             # every command and option
quill render --help                    # one command's options (any command)
quill check examples/two-ducks.json    # mistakes, and where everything landed
quill render examples/two-ducks.json --preview   # a quick look: out/two-ducks-preview.png
quill views examples/two-ducks.json    # seven views of the picture on one sheet
quill critique examples/two-ducks.json # about fifteen findings in numbers
quill info examples/two-ducks.json     # the layers, and what painted each one
quill presets                          # the brush presets a script can name
quill export examples/duck-animated.json --gif   # an animation as a GIF
```

The rules:

- **`quill` isn't on the PATH.** Give its full path,
  `target/release/quill` (`quill.exe` on Windows). Pictures go to `out/`
  in the folder you run it from, or to `--out <folder>`.
- **Check after every change** with `quill check`, and read the numbers
  before the picture. Boxes read `x left..right, y top..bottom`; x goes
  right, y goes down. Errors name the command, counted from 1.
- **Look with `quill render --preview`** when you can read images.
  Small steps beat one huge script.
- Exit codes: 0 worked (warnings too), 1 a problem in the script or a
  file, 2 a wrong command line.
- The same script always paints the same pixels.

## Working on the code

A Rust workspace (Rust 1.97, edition 2024):

| Crate | What it is |
|---|---|
| `crates/engine` | `quill_engine`: the painting engine. Plain Rust, no window code |
| `crates/cli` | `quill_cli`: the `quill` program (and `art-cli`, its old name) |
| `crates/app` | `quill_app`: the `DuckQuill` program, with its window (egui) |

The app and `quill` use the engine; the engine never uses them.

```bash
cargo build --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p quill_cli --bin quill -- help   # run quill from the source
cargo run -p quill_app                       # start the app (it needs a screen)
```

**Before a commit:** the tests, `cargo fmt --all` and clippy with
`-D warnings`, all clean. CI (`.github/workflows/ci.yml`) runs the
build, the tests and clippy on Windows and Linux.

Stored results that tests compare with, and how to accept a change you
meant:

- The reference pictures in `crates/engine/tests/reference`: every pixel
  must match on Linux. If a change is meant to change pictures:
  `ARTAPP_UPDATE_REFERENCES=1 cargo test -p quill_engine --test reference`.
  Look at every picture that changed before you keep it.
- The docs are tested too. `cargo test -p quill_cli --test skill` checks
  that `skills/duck-quill/SKILL.md` names only commands, options and
  files that exist, and its examples paint; `--test agents` does the same
  for this file. The engine's tests check every script example in
  `docs/script-format.md`, `docs/AI_GUIDE.md` and `docs/QUICKSTART.md`.

**With no internet:** once `cargo fetch` has run,
`cargo build --offline --release -p quill_cli` builds `quill` with
nothing downloaded (and `cargo test --offline --workspace` tests).

Other things to know:

- A change to the script format changes `docs/script-format.md`, and
  the guide and skill where they mention it.
- `docs/decisions.md` says why things are as they are: a short numbered
  entry per decision, newest at the bottom.
- Words for people, in messages and docs, are plain English with no
  jargon.
- Old names still work and must keep working: `art-cli`, `.artapp`
  pictures and `"format": "artapp-script"` (`crates/engine/tests/old_files.rs`).
- The version number is written once, in `Cargo.toml`.
- Tests read text files with `\r\n` turned into `\n`: on Windows, Git may
  check files out with Windows line endings.
- Duck Mold starts the app on a PNG, with options:
  `DuckQuill face.png --paper #C8A064 --guide face-guide.png`. It watches
  the PNG: Save writes `face.png` and the layers in `face.quill`. The
  details are in README.md, "PNG pictures, and painting for Duck Mold".
