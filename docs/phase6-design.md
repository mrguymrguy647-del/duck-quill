# Phase 6 design: Duck Quill 0.6.0

Three things: the six issues an agent hit while building a game are fixed, animation becomes
something the owner switches on and draws with, and Duck Quill installs from one `setup.exe`
and updates itself. Everything made before keeps working.

Stops for the owner: after this design, after Step 3 (animation proven), after Step 5 (the
installer proven on a clean Windows machine).

## What I checked in the code first

The phase's brief described 0.5.0 from memory. Four things were different in the code:

- **Every agent doc already says `-p quill_cli`** (`SKILL.md`, `AI_GUIDE.md`, `QUICKSTART.md`,
  `README.md`). Built alone from nothing, `quill` takes 36 seconds here; the whole workspace
  pulls in about 290 libraries instead of 40. What's missing is the ready-made download and a guard.
- **The shortcut sheet is its own hand-written list**, not the list the menus use. Command
  search needs one real list, so the menus are rebuilt on it (Step 4).
- **Layers don't share pixels yet.** Each layer owns its 256-pixel tiles outright. Phase 5 left
  sharing as "if needed"; animation needs it (below).
- **The repository is already renamed**: `art-app` redirects to `duck-quill`, and 0.5.0 is on
  its Releases page. The updater asks about `duck-quill`.

One limit of this session: it can only push to the branch `claude/stoic-cannon-vyu6qr`, so
that branch plays the part of `phase-6`. It was restarted from today's `main`.

## Step 1: the six fixes

Each one gets a test that fails on today's code first.

| # | What happened | The cause in the code | The fix |
|---|---|---|---|
| 1 | The first build took 8 minutes | Nothing wrong in the docs; the agent built the whole workspace, window and all. Nothing ready-made exists for Linux | Each release also publishes `quill-linux-x86_64.tar.gz` (`quill`, the skill folder, the docs and examples an agent reads; built to run on any Linux). `SKILL.md` shows the one-line download first and the 36-second build second. A test fails if `quill` ever depends on a window library |
| 2 | `export` refuses a script without animations | `export.rs` stops at once when `animations` is empty | Without animations, `export` paints each `define`d symbol on its own frame (all of them, or those in a top-level `"stills"` list) and writes one PNG plus a JSON shaped exactly like `quill atlas`'s. `--stills` does the same for a script that also has animations |
| 3 | No speckle fill | There is no such command; `scatter` paints brush dabs, not pixels | A `speckle` command (below) |
| 4 | A skin's layout written twice | Scripts have no way to name a rectangle | `region` and `skin_box` commands (below), written to a JSON beside the PNG |
| 5 | False "palette colour never used" | Two causes. A colour only counts once something *paints* with it, so colours inside symbols that the frame shown doesn't use are blamed: `render` of an animated script looks at one frame only, and a script that is only `define`s paints nothing at all (my test of that shape: 10 warnings, 9 false). | A colour counts as used when any command anywhere in the script names it, in a symbol or not, including a `pixels` `colors` map, a colour swap and a keyframe. A colour nothing names is still reported. A script made for stills no longer calls its symbols "never used" or its layer "empty" |
| 6 | 52 colours at most in `pixels` | `colors` keys must be exactly one character | Keys may be longer. All keys in one command have the same length, and each row is read that many characters at a time (`..` is see-through for two-character keys). One-character scripts read exactly as before. A bad row's error says the row and the column |

**`speckle`**, for grass, dirt, stone and leaves:

```json
{ "cmd": "speckle", "rect": [0, 0, 16, 16], "seed": 7, "clump": 2,
  "colors": { "grass": 6, "grass_dark": 3, "grass_light": 1 } }
```

Each pixel of the rectangle (the whole canvas if none is given) gets one of the colours, chosen
by weight. With `clump`, neighbouring pixels gather into blobs about that many pixels across
instead of single dots. The choice uses whole-number arithmetic from the script's own random
generator, so one seed gives the same pixels on every machine. Blobs wrap round the rectangle's
edges, so a tile that is speckled edge to edge repeats without a seam (`render --tile` agrees).
It paints whole canvas pixels, so it also works on a canvas that is painted big and shrunk.

**`region` and `skin_box`**, so a layout is written once:

```json
{ "cmd": "region", "name": "hat_icon", "rect": [48, 0, 8, 8] }
{ "cmd": "skin_box", "name": "head", "at": [0, 0], "size": [8, 8, 8] }
```

`skin_box` unwraps a box `[width, height, depth]` the Minecraft way from its top-left corner and
names six regions: `head.top`, `head.bottom`, `head.right`, `head.front`, `head.left`,
`head.back`. Neither command paints. A region's name can be used wherever a rectangle is asked
for (`{ "cmd": "speckle", "region": "head.top", ... }`). `render` and `export` write
`<name>.regions.json` beside the PNG (name, x, y, width, height of each, and each box's size),
`check` lists them and warns when two overlap or one leaves the canvas, and
`docs/sprite-sheet-format.md` documents the file for Duck Forge.

Whatever the owner reports from testing 0.5.0 joins this step.

## Step 2: animation as a switch

### The switch

**View → Animation tools** and the same tick box in a new **Settings** window (File → Settings).
Remembered between sessions, off the first time. Off: no timeline, no Animation menu, exactly
today's drawing app. On: a timeline at the bottom and an **Animation** menu. Switching it off
changes nothing in the picture: the frames stay, and the canvas shows the frame you were on.

A picture with frames opened while the tools are off shows a thin bar under the menus:
"This picture has 8 frames. **Turn on animation tools**   ×". File → New → **New animation**
turns them on by itself.

### The timeline

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ |◀  ▶ Play  ▶|   12 fps  [x] Loop   [x] Onion skin: 1 before, 1 after ▁▃▅    │
│ ┌────┐┌────┐┌────┐╔════╗┌────┐┌────┐┌────┐┌────┐                             │
│ │ 🦆 ││ 🦆 ││ 🦆 │║ 🦆 ║│ 🦆 ││ 🦆 ││ 🦆 ││ 🦆 │  [+ Frame] [Duplicate] [Delete]│
│ └─1──┘└─2──┘└─3×2┘╚═4══╝└─5──┘└─6──┘└─7──┘└─8──┘                             │
│ ├──────── idle ────────┤├──────────── run ──────────┤                        │
└──────────────────────────────────────────────────────────────────────────────┘
```

- A small picture of every frame; click to go there, drag to reorder, right-click for
  duplicate, delete and **hold** (a frame shown 2, 3, … times as long; "×2" under it).
- Play / pause (`Space` is taken by panning, so the button, and `Enter` when nothing else
  wants it), previous and next frame on `,` and `.`, fps, loop.
- Onion skin: how many frames before and after (0 to 3), a strength slider, red for before
  and blue for after, as now.
- **Named stretches** ("idle", "run") under the frames, for a sprite with several animations
  in one picture. Play plays the stretch you're in. A picture with none is one animation.

It replaces today's Animation panel. That panel's play, step and onion-skin code and its
tests are kept and moved under it.

### Drawing on frames

Every tool works on the frame shown, pen pressure included, because a frame is not a special
thing: **each layer is either still or animated**, set by a small film-strip mark on its row in
the Layers panel.

- A **still** layer has one drawing, seen on every frame (a background, a guide).
- An **animated** layer has a drawing per frame. Going to another frame swaps which drawing
  the layer shows. Nothing is copied.

New animation starts with a still "Background" and an animated "Layer 1". Changing a layer
from still to animated puts its drawing on every frame; the other way keeps the frame shown
(and says so first, since the other frames' drawings go; it's one undo step).

### How frames are stored, and the risks

**Memory.** Tiles become shared: a layer holds a reference to each tile, and a tile is copied
only at the moment something paints on it. So a duplicated frame costs nothing until it's
drawn on, a blank frame costs nothing, a still layer exists once, and undo keeps references
too. Twenty-four frames of 1024 × 1024 with a still background cost the background once plus
only the tiles that differ between frames. A test builds exactly that picture and fails if it
uses more than a small multiple of one frame; the numbers go in the Step 3 report. (This also
makes the autosave copy nearly free, which Phase 5 wanted.)

**Undo.** One history for the whole picture. Each undo step remembers which frame it was made
on; undoing it first takes you to that frame, then undoes it, so you always see what changed.
Adding, duplicating, deleting, reordering and holding a frame, and switching a layer between
still and animated, are undo steps like any other.

**The file.** A picture with one frame is saved in today's format (file version 1),
unchanged, so 0.5.0 opens everything that isn't animated. A picture with frames is saved as
**file version 2**: the same archive, plus an `animation` section in `document.json` (fps,
loop, each frame's hold, the named stretches, which layers are animated) and the animated
layers' tiles in one shared pool, each stored once however many frames use it.

*What 0.5.0 does with it:* it says "This picture was saved by a newer version of Duck Quill
(file version 2). Please update Duck Quill to open it." The brief's first wish was that 0.5.0
opens the first frame. I'm choosing the message on purpose: if 0.5.0 opened frame 1 and you
pressed Ctrl+S, the other frames would be gone without a word. A CI job builds the real 0.5.0
`quill` from its tag and checks both halves: it opens a 0.6.0 one-frame picture pixel for
pixel, and it refuses a 0.6.0 animation with that message and no crash.

Old files (`.quill` and `.artapp`, version 1) open as one frame. Fixtures saved by today's
code are committed before anything changes, with their expected pixels.

**Autosave and crash recovery** save the document, and the frames are now inside the
document, so every frame is covered. A test draws on three frames, lets autosave run, restores
the copy and compares every frame.

**Selections, transform, symmetry, references.** The selection belongs to the picture, so it
stays put as you step through frames (handy for redrawing one area on each). While a
transform is in progress the frame can't change, just as undo and the menus wait today:
Enter or Esc first. Symmetry is a
setting of the tool and doesn't know about frames. References belong to the picture and show
on every frame. Each gets a test on frame 1 and on a later frame.

**Speed.** Going to another frame swaps the animated layers' drawings and redraws only their
tiles; 1024 × 1024 is 16 tiles. Frame pictures for the timeline and the onion skin are kept
and redone only for a frame that was drawn on. A timing test (24 frames, 1024 × 1024, four
layers) measures a step, a second of playback and a save; the numbers go in the report, and I
expect a step well under one screen refresh.

### Where the owner and agents meet

- `quill info walk.quill` lists frames, holds, stretches, fps, and which layers are animated.
- `quill check walk.quill` gives the motion summary scripts already get: each frame's painted
  box, jumps, empty frames, a loop whose ends don't meet.
- `quill critique walk.quill --frames 0-7`, `quill render walk.quill --frame 3`,
  `--frame-sheet` and `--onion` work as they do for scripts.
- `quill export walk.quill --sheet --gif --frames` (and `--mp4`): the same sheet and JSON
  Duck Forge reads. A held frame is its number repeated in the JSON's `frames` list, so the
  sheet format doesn't change.
- **To improve a frame**, an agent runs a small script onto it:
  `quill apply fix.json --to walk.quill --frame 3 --layer "Duck"`. It uses the same code as
  File → Play strokes on an open picture.
- **Script animation into frames:** `quill bake duck-animated.json` writes
  `duck-animated.quill` with one frame per script frame, each animation a named stretch;
  layers nothing moves on become still layers. In the app, a script animation opens in the
  same timeline, locked, with a bar: "These frames come from a script. **Edit frames by
  hand**". Clicking it unlocks them as a new unsaved picture. The script is never touched.

### Sprite templates

File → New → **New pixel sprite or tile**: 16, 32 or 64 pixels; tick the animations wanted
(idle 4 frames at 6 fps, run 8 at 12; jump 6 and attack 5 if time allows). The picture opens
with those named stretches, a still guide layer showing the ground line and the pivot
(left out of exports), and the tools on. File → Export animation → Sprite sheet gives the PNG
and JSON straight away.

## Step 4: installer, updates, Home, command search

### The installer

**Inno Setup**, as suggested: free, one script file in the repository, per-user installs
without an admin prompt are built in, and so are file associations, the PATH tick box, quiet
installs and a proper uninstaller. `release.yml` builds `DuckQuill-<version>-setup.exe`.

- Installs to `%LOCALAPPDATA%\Programs\Duck Quill`: `DuckQuill.exe`, `quill.exe`,
  `art-cli.exe`, the skill folder, the read-me. No admin prompt.
- Start menu entry; desktop icon as a tick box; "Launch Duck Quill" on the last page.
- **Registry, all under the current user (HKCU), nothing system-wide:** `.quill` and
  `.artapp` → "Duck Quill picture" with the duck icon and "open with DuckQuill.exe"; the
  uninstall entry Windows Settings → Apps reads; with the PATH tick box, the install folder
  added to the user's `Path`. PNG gets only an "Open with → Duck Quill" entry, never the
  default (first thing cut if time runs short).
- Uninstall removes what it installed and its registry entries. `%APPDATA%\Duck Quill`
  (settings, presets, autosaves) and your pictures are never touched. Installing over an older
  version leaves that folder alone too.
- **One Duck Quill at a time:** a second start hands its file to the one already running and
  closes; the running one opens it (asking "Save changes?" first if needed) and comes to the
  front.

The zip stays as the portable download. `run.bat` stays for building from source.
`README.md` and `READ ME FIRST.txt` lead with the installer.

### Updates

- At start (a tick box in Settings) and from Help → Check for updates, Duck Quill asks
  GitHub "what is the newest release of duck-quill?". It sends nothing about you: no name, no
  files, no id; GitHub sees an internet address and "Duck Quill 0.6.0", as it would for any
  download. It runs in the background, so starting never waits, and says nothing if it fails.
- Newer version: a banner, "Duck Quill 0.6.1 is out: **Update**". Update downloads the setup
  and the release's checksum file, works out the setup's SHA-256, and **refuses to run it if
  the two differ** (the download is deleted and the banner says so). Then it keeps your work,
  runs the setup with only a small progress window, and Duck Quill reopens with the same
  picture.
- A failed or cancelled update changes nothing: the old version is only replaced by a setup
  that passed the check.
- A portable (zip) copy gets "Download" instead, which opens the Releases page.
- `release.yml` gets a signing step that runs only when a certificate secret exists.

Two things to be plain about. **The checksum** proves the setup arrived whole and is the one
published with that release. It can't prove who published it; only a code-signing certificate
does that. **"Windows protected your PC"** will keep appearing on first install until you
buy one (a yearly cost); I'll leave the hook and the instructions.

### Home screen

```
┌─────────────────────────────────────────────────────────────────────┐
│  🦆 Duck Quill                       Duck Quill 0.6.1 is out: Update │
│                                                                     │
│  [ New picture ] [ New animation ] [ New pixel sprite or tile ] [ Open… ]
│                                                                     │
│  Recent                                                             │
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐                        │
│  │        │ │        │ │        │ │        │                        │
│  └─pond───┘ └─duck───┘ └─walk 8f┘ └─tiles──┘                        │
│                                                                     │
│  Not saved last time                                                │
│  ┌────────┐  "Untitled", 12 minutes ago   [Restore] [Throw away]    │
│  └────────┘                                                         │
│                                          [x] Show this at start     │
└─────────────────────────────────────────────────────────────────────┘
```

Shown at start (unless a file was opened by double-click) and from File → Home. Recent
pictures are large thumbnails, made when a picture is saved and kept in the settings folder.
Autosaved copies appear here instead of in today's pop-up.

### Command search (Ctrl+K)

```
┌───────────────────────────────────────────┐
│ 🔍 onion                                  │
├───────────────────────────────────────────┤
│ Animation: Onion skin on / off            │
│ Settings: Animation tools                 │
│ View: Animation tools                     │
└───────────────────────────────────────────┘
```

One list in the code holds every command: its menu, its name, its shortcut, and when it's
available. The menus are drawn from that list, the shortcut sheet is printed from it, the
keys are read from it, and Ctrl+K searches it. Tools, panels and settings are entries too. So
a feature added later is in the search the moment it's in a menu, and a test checks that
every entry turns up in the search under its own name.

## New parts the app would depend on

Only in the app, never in `quill`: a small web client for the update check and download, and
a SHA-256 library for the checksum (`ureq` and `sha2`, both widely used). Inno Setup on the
build machine. Say if you'd rather not add any of these.

## What I'd cut or change in the brief

1. **0.5.0 gets a clear message for animations, not the first frame** (the silent-loss reason
   above).
2. **"Update saves all open work"**: I'd keep unsaved changes as an autosave copy and bring
   them back, still marked unsaved, after the update, instead of writing over your file because
   an update happened. Your saved file only changes when you press Save.
3. **Several animations live in one picture** as named stretches of one timeline, so one
   sprite makes one sheet with idle and run rows.
4. **One timeline**, also for script animations (locked until "Edit frames by hand"), instead
   of the old panel beside a new one.
5. **`Space` stays panning**, so play/pause is the button and `Enter`.
6. **Fix 1's doc change is already done**; the work is the download and the guard.
7. If time runs short I cut in the brief's order: MP4 of hand animations, jump and attack
   templates, "what's new" on Home, PNG "Open with".

## What worries me

- **I can't run Windows here.** The installer, associations, the single running copy and the
  updater are built and tested on GitHub's Windows machines (Step 5 runs them on a clean one),
  and I'll read those logs, but your PC is the real test. Each stop comes with a short list to
  try.
- **Sharing tiles touches the engine's core.** Every existing test, the old-file fixtures and
  the benchmark must come out the same before any animation code goes on top; that's the first
  commit of Step 2.
- **Rebuilding the menus on one list** touches every menu. It's done in its own commits with
  the shortcut tests kept.
- **The pen.** Pressure on frames uses the same path as today, but only you can feel it.

## One question

**Did testing 0.5.0 on Windows turn up anything?** Pen, shortcuts, lasso, copy and paste,
crash recovery, anything annoying or slow. Whatever you list joins Step 1.
