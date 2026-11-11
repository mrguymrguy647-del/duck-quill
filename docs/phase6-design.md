# Phase 6 design: Duck Quill 0.6.0

Three things: the six issues an agent hit while building a game are fixed, animation becomes
something the owner switches on and draws with, and Duck Quill installs from one `setup.exe`
and updates itself.

Stops for the owner: after this design, after Step 3 (animation proven), after Step 5 (the
installer proven on a clean Windows machine).

## What I checked in the code first

The brief described 0.5.0 from memory. Four things were different in the code:

- **Every agent doc already tells agents to build only `quill`** (`cargo build --release
  -p quill_cli` in `SKILL.md`, `AI_GUIDE.md`, `QUICKSTART.md` and `README.md`). Built alone from
  nothing, `quill` takes about half a minute here and needs about 45 libraries; the app needs
  about 300. So the 8 minutes most likely came from building everything. What's missing is a
  ready-made download and a check that `quill` stays small.
- **The shortcut sheet is its own hand-written list**, not the list the menus use. Command
  search needs one real list, so the menus are rebuilt on it (Step 4).
- **Layers don't share pixels.** Each layer owns its own blocks of pixels. Phase 5 left
  sharing as "if needed"; animation needs it (below).
- **The repository is already renamed** `duck-quill` (the old address still works), and 0.5.0
  is on its Releases page. The updater asks about `duck-quill`.

The work is on the branch `claude/stoic-cannon-vyu6qr`, not one called `phase-6`: it's the
only branch this session is allowed to save to. The pull request at the end goes from it to
`main`.

## Step 1: the six fixes

Each one gets a test that fails on today's code first. Before any of it, pictures, scripts
and `quill` outputs made by today's code are committed as fixtures, with their expected pixels.

| # | What happened | The cause in the code | The fix |
|---|---|---|---|
| 1 | The first build took 8 minutes | The docs are right; most likely the whole project was built, window and all. Nothing ready-made exists for Linux | Each release also publishes `quill-linux-x86_64.tar.gz`: `quill` as one file that runs on any Linux (tried here: it works), with the skill folder, docs and examples. `SKILL.md` shows the one-line download first and the half-minute build second. A test fails if `quill` ever comes to depend on a window library |
| 2 | `export` refuses a script without animations | `export.rs` stops at once when `animations` is empty | Without animations, `export` paints each `define`d symbol on its own frame (all of them, or those in a top-level `"stills"` list) and writes one PNG plus a JSON shaped exactly like `quill atlas`'s. `--stills` does the same for a script that also has animations |
| 3 | No speckle fill | There is no such command; `scatter` paints brush dabs, not pixels | A `speckle` command (below) |
| 4 | A skin's layout written twice | Scripts have no way to name a rectangle | `region` and `skin_box` commands (below), written to a JSON beside the PNG |
| 5 | False "palette colour never used" | A colour only counts once something *paints* with it. `render` of an animated script looks at one frame, so colours of symbols used in other frames are blamed; a script that is only `define`s paints nothing, so every colour is blamed (in my test of that shape, every warning but one was false). `check` already counts all frames | A colour counts as used when any command anywhere in the script names it, inside a symbol or not, including a `pixels` `colors` map, a colour swap and a keyframe. A colour nothing names is still reported. A script made for stills no longer calls its symbols "never used" or its layer "empty" |
| 6 | 52 colours at most in `pixels` | `colors` keys must be exactly one character | Keys may be longer. All keys in one command have the same length and each row is read that many characters at a time (`..`, or two spaces, is see-through for two-character keys). One-character scripts read exactly as before. A bad row's error names the row and the column (today: the row only) |

**`speckle`**, for grass, dirt, stone and leaves:

```json
{ "cmd": "speckle", "rect": [0, 0, 16, 16], "seed": 7, "clump": 2,
  "colors": { "grass": 6, "grass_dark": 3, "grass_light": 1 } }
```

Each pixel of the rectangle (the whole canvas if none is given) gets one of the colours, chosen
by weight. With `clump`, neighbouring pixels gather into blobs about that many pixels across
instead of single dots. It uses the scripts' existing repeatable random numbers and only whole
numbers, so one seed gives the same pixels on every machine. Blobs wrap round the rectangle's
edges, so a tile speckled edge to edge repeats without a seam (`render --tile` agrees).

**`region` and `skin_box`**, so a layout is written once:

```json
{ "cmd": "region", "name": "hat_icon", "rect": [48, 0, 8, 8] }
{ "cmd": "skin_box", "name": "head", "at": [0, 0], "size": [8, 8, 8] }
```

`skin_box` unwraps a box `[width, height, depth]` the Minecraft way from its top-left corner and
names six regions: `head.top`, `head.bottom`, `head.right`, `head.front`, `head.left`,
`head.back`. Neither command paints. A region's name can be used where a rectangle is asked
for (`{ "cmd": "speckle", "region": "head.top", ... }`). `render` and `export` write
`<name>.regions.json` beside the PNG (name, x, y, width, height of each, and each box's size),
`check` lists them and warns when two overlap or one leaves the canvas, and
`docs/sprite-sheet-format.md` documents the file for Duck Forge.

Whatever the owner reports from testing 0.5.0 joins this step.

## Step 2: animation as a switch

### The switch

**View → Animation tools**, and the same tick box in a new **Settings** window (File →
Settings). Remembered between sessions, off the first time. Off: no timeline and no Animation
menu, a plain drawing app. On: a timeline at the bottom and an **Animation** menu. Switching it
off changes nothing in the picture: the frames stay, and the canvas shows the frame you were on.

A picture with frames opened while the tools are off shows a thin bar under the menus:
"This picture has 8 frames. **Turn on animation tools**   ×". File → **New animation** turns
them on by itself.

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
- Previous frame `,` next frame `.` play or pause `/` (three keys side by side, all free
  today; `Space` stays panning). Starting to draw pauses playback.
- Onion skin: how many frames before and after (0 to 3), a strength slider, red for before
  and blue for after, as now.
- **Named stretches** ("idle", "run") under the frames, for a sprite with several animations
  in one picture. Each stretch has its own fps and loop, and Play plays the stretch you're in.
  A picture with none is one animation, with the fps and loop shown at the top.

This is a new panel. From today's Animation panel it keeps the timing rules and the three
exports with their tests; the rest of that panel showed finished pictures you couldn't draw on.

### Drawing on frames

Every tool works on the frame shown, pen pressure included, because **each layer is either
still or animated**, shown by a small film-strip mark on its row in the Layers panel.

- A **still** layer has one drawing, seen on every frame (a background, a guide).
- An **animated** layer has a drawing per frame. Going to another frame changes which drawing
  the layer shows. Nothing is copied.

The rules, so nothing is a surprise:

- New animation starts with a still "Background" and an animated "Layer 1".
- Adding the first extra frame to a plain picture makes the layer you're on animated; the
  others stay still until you click their mark.
- In a picture with frames, a new layer is animated (empty on every frame) and a paste lands
  on the frame shown.
- Still → animated puts the drawing on every frame. Animated → still keeps the frame shown and
  warns first that the other frames' drawings on that layer go (one undo step).
- A layer's name, opacity, blend mode, clipping and alpha lock belong to the layer, on every frame.
- Changing frame follows today's rule for undo: it waits for a stroke to end, finishes text
  being typed, and isn't available during a transform, an unfinished polygon lasso or a
  script being played (Enter or Esc first).
- File → Export PNG exports the frame shown. File → Export strokes exports the strokes made on
  the frame shown, and says so.

**Drawing pixel sprites by hand** needs something the app lacks today: every brush has a soft
edge at least a pixel wide, so a 16-pixel sprite would come out blurred. So this step adds a
**Pixel pencil** and **Pixel eraser** preset (whole pixels, hard edge, no see-through), a fill
that stops exactly at pixel edges, and a pixel grid when zoomed in. Scripts already draw this
way on pixel canvases; the app's tools get the same code.

### How frames are stored, and the risks

**Memory.** Layers start sharing their blocks of pixels (each block is 256 × 256): a block is
copied only at the moment something paints on it. So a duplicated frame costs almost no memory
until it's drawn on, a blank frame costs none, a still layer exists once, and undo shares
blocks too. Twenty-four frames of 1024 × 1024 with a still background cost the background once
plus only the blocks that differ between frames. The timeline's frame pictures are small, and
the onion skin is kept only for the frames beside the one shown. A test builds that picture
and fails if it uses more than a small multiple of one frame, counting those too; the numbers
go in the Step 3 report. (Sharing also makes the autosave copy almost free.)

**Undo.** One history for the whole picture. Each undo step is tied to the drawing it changed
(which layer, which frame), so it's put back in the right place whatever is on screen; and
undo first takes you to that frame, so you see what changed. Adding, duplicating, deleting,
reordering and holding a frame, and switching a layer between still and animated, are undo
steps like any other.

**The file.** A picture that was never animated is saved in today's format (file version 1),
unchanged, so 0.5.0 opens it. A picture with animation in it (frames, or an animated layer) is
saved as **file version 2**: the same kind of file, with an animation section added (fps,
loop, each frame's hold, the named stretches, the pivot, which layers are animated) and each
block of an animated layer stored once however many frames show it.

*What 0.5.0 does with a version 2 file:* it says "This picture was saved by a newer version of
Duck Quill (file version 2). Please update Duck Quill to open it." The brief's first wish was
that 0.5.0 opens the first frame. I'm choosing the message on purpose: if 0.5.0 opened frame 1
and you pressed Ctrl+S, the other frames would be gone without a word. On every push, GitHub's
test machines build the real 0.5.0 `quill` and check both halves: it opens a 0.6.0 plain
picture pixel for pixel, and it refuses a 0.6.0 animation with that message and no crash.

Old files (`.quill` and `.artapp`) open as one frame. A script that opens a picture
(`"open": "walk.quill"`) gets its first frame, or the one named by `"frame"`.

**Autosave and crash recovery** save the whole picture, and the frames are part of the
picture, so every frame is covered. A test draws on three frames, lets autosave run, restores
the copy and compares every frame.

**Selections, transform, symmetry, references.** The selection belongs to the picture, so it
stays put as you step through frames (handy for redrawing one area on each). A transform
moves the drawing of the frame shown, and the frame can't change until it's put down.
Symmetry is a setting of the tool and works the same on every frame. References belong to the
picture and show on every frame. Each gets a test on the first frame and on a later one.

**Speed.** Going to another frame redraws only the animated layers' blocks; 1024 × 1024 is 16
of them. A frame's small picture is redone when its drawing changes; a change to a still layer,
a layer setting or the paper redoes them all, a few at a time between screen refreshes. A
timing test (24 frames, 1024 × 1024, four layers) measures a step, a second of playback and a
save. The target is smooth playback at 24 frames a second; the report has the numbers either way.

### Where the owner and agents meet

Today `quill` can only `info` a `.quill`; everything below is new.

- `quill info walk.quill` lists frames, holds, stretches, fps, and which layers are animated.
- `quill check walk.quill`: each frame's painted box, jumps between frames, empty frames, a
  loop whose ends don't meet.
- `quill critique walk.quill --frames 0-7`: the findings that need no script (values, colour,
  readability at small size, composition). A picture has no written intent, so the focal-point
  findings are left out, and it says so.
- `quill render walk.quill --frame 3`, `--frame-sheet`, `--onion`: frames as pictures to look at.
- `quill export walk.quill --sheet --gif --frames` (and `--mp4`): the same sheet and JSON
  Duck Forge reads, one row per stretch with its own fps and loop. A held frame is its number
  repeated in the JSON's `frames` list, so the sheet format doesn't change.
- **To improve a frame**, an agent runs a small script onto it:
  `quill apply fix.json --to walk.quill --frame 3 --layer "Duck"`. The app's Play strokes does
  this for ordinary pictures but refuses pixel canvases today; `apply` will paint crisp whole
  pixels straight onto the frame (a script that paints big and shrinks is refused with a clear
  message).
- **Script animation into frames:** `quill bake duck-animated.json` writes
  `duck-animated.quill`: one frame per script frame, each animation a named stretch, layers
  matched by name across animations, and layers nothing moves on become still. In the app, a
  script animation opens in the same timeline with drawing switched off and a bar: "These
  frames come from a script. **Edit frames by hand**". Clicking it makes them a new unsaved
  picture you can draw on. The script is never touched.

### Sprite templates

File → **New pixel sprite or tile**: 16, 32 or 64 pixels; tick the animations wanted (idle 4
frames at 6 fps, run 8 at 12; jump 6 and attack 5 if time allows). The picture opens with
those stretches, the Pixel pencil chosen, the tools on, and a still guide layer (left out of
exports) showing the ground line and the **pivot**: the point where the character stands,
which the game uses to place it. Animation → Set pivot moves it. File → Export animation →
Sprite sheet gives the PNG and JSON straight away.

## Step 3: prove animation (STOP)

As the brief says: tests drive the app's own commands as the owner (tools on, an 8-frame duck
walk over a still background with onion skin, GIF and sheet exported, tools off and on, close
and reopen), then as an agent (`quill apply` on one frame, export again). The report has a
screenshot of each step, the GIF, the memory and speed numbers, what a fresh reviewer found,
and an honest list of what would still confuse a first-time animator.

## Step 4: installer, updates, Home, command search

### The installer

**Inno Setup**, as suggested: free, one script file in the repository, and it already does
per-user installs without an admin prompt, file associations, a PATH tick box, quiet installs
and a proper uninstaller. `release.yml` builds `DuckQuill-<version>-setup.exe`.

- Installs to `%LOCALAPPDATA%\Programs\Duck Quill`: `DuckQuill.exe`, `quill.exe`,
  `art-cli.exe`, the skill folder, the read-me. No admin prompt.
- Start menu entry; desktop icon as a tick box; "Launch Duck Quill" on the last page.
- **Registry: only the part that belongs to your Windows account** (called HKCU), nothing
  system-wide. `.quill` and `.artapp` become "Duck Quill picture" with the duck icon, opened by
  `DuckQuill.exe`; the entry Windows Settings → Apps reads to uninstall; and, with the PATH
  tick box, the install folder added to your account's `Path`. PNG gets only an "Open with →
  Duck Quill" entry, never the default (the first thing cut if time runs short).
- Uninstall removes what it installed and its registry entries. `%APPDATA%\Duck Quill`
  (settings, presets, autosaves) and your pictures are never touched. Installing over an older
  version leaves that folder alone too.
- **A double-clicked file opens in the Duck Quill already running** (asking "Save changes?"
  first if needed), which comes to the front. Today each double-click starts another copy; to
  keep two pictures side by side there's a new File → New window.
- **Moving from 0.5.0 is by hand, once:** 0.5.0 has no updater. Download the setup, run it, and
  delete the old unzipped folder and its desktop shortcut. Settings, presets and recent files
  carry over, because both keep them in the same place.

The zip stays as the portable download. `run.bat` stays for building from source.
`README.md` and `READ ME FIRST.txt` lead with the installer.

### Updates

- At start (a tick box in Settings) and from Help → Check for updates, Duck Quill asks
  GitHub "what is the newest release of duck-quill?". It sends nothing about you: no name, no
  files, no id; GitHub sees an internet address and "Duck Quill 0.6.0", as for any download.
  It runs in the background, so starting never waits. At start a failed check says nothing;
  from the Help menu it answers: "You have the newest version" or "Couldn't check".
- Newer version: a banner, "Duck Quill 0.6.1 is out: **Update**". Update downloads the setup
  and the release's list of fingerprints (SHA-256 checksums), works out the setup's own
  fingerprint, and **refuses to run it if they differ**: the download is deleted and the
  banner says so. Then it keeps your work, runs the setup with a small progress window, and
  Duck Quill reopens with the same picture.
- **If it fails:** up to the moment the setup starts, nothing on your PC has changed. While
  the setup swaps the program files (a few seconds), it keeps the old ones aside and puts them
  back if anything goes wrong, and shows what went wrong. If an agent is using `quill.exe` at
  that moment, the update waits and says so. Step 5 makes a setup fail on purpose to test this.
- A portable (zip) copy gets "Download" instead, which opens the Releases page.
- `release.yml` publishes the checksum file with every release, keeps the release hidden until
  every file is uploaded (so a check never sees half a release), and gets a signing step that
  runs only when a certificate has been added to the repository's secrets.

Two things to be plain about. **The checksum** proves the setup arrived whole and is the one
published with that release. It can't prove who published it; only a code-signing certificate
does that. **"Windows protected your PC"** will keep appearing on first install until you buy
one (a yearly cost); the signing step and instructions will be ready for when you do.

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

## Step 5: prove the install (STOP)

On a clean Windows machine at GitHub, as the brief lists: quiet install; Start menu entry,
file association and uninstall entry checked; a `.quill` opened through its association; an
update from a fake older version with settings surviving; a setup with a wrong checksum
refused; a setup that fails halfway leaving the old version working; uninstall leaving the
owner's files. The setup is attached to that run as a download. The report has screenshots of
the installer pages, Home, the update banner and command search, and steps to try it on your PC.

## Step 6: wrap up

Version 0.6.0; the docs and `SKILL.md` updated and kept short; a fresh agent given only
`SKILL.md` renders, checks, critiques and exports a picture, a speckled tile and a hand-made
animation; full tests, the benchmark and the gallery checks; the pull request. The release
itself is yours to press.

## New parts the app would depend on

For information, since the brief implies them. Only in the app, never in `quill`: a web client
for the update check and download, which brings its own encryption code for https (`ureq`
with `rustls`), and a SHA-256 library for the checksum (`sha2`). All widely used. Inno Setup is
needed only on GitHub's build machine.

## What I'd cut or change in the brief

1. **0.5.0 gets a clear message for animations, not the first frame** (the silent-loss reason
   above).
2. **"Update saves all open work"**: I'd keep unsaved changes as an autosave copy and bring
   them back, still marked unsaved, after the update, not write over your file because an
   update happened. Your saved file only changes when you press Save.
3. **Several animations live in one picture** as named stretches of one timeline, so one
   sprite makes one sheet with an idle row and a run row.
4. **One timeline**, also for script animations, not the old panel beside a new one.
5. **Added: pixel tools for the app** (Pixel pencil, hard fill, grid). The brief doesn't ask
   for them, but hand-drawn 16-pixel sprites aren't possible without them.
6. **Added: File → New window**, so opening files in the running app doesn't take away
   having two pictures open.
7. **Fix 1's doc change is already done**; the work is the download and the check.
8. If time runs short I cut in the brief's order: MP4 of hand animations, jump and attack
   templates, "what's new" on Home, PNG "Open with".

## What worries me

- **I can't run Windows here.** The installer, associations, the single running copy and the
  updater are built and tested on GitHub's Windows machines, and I'll read those logs, but
  your PC is the real test. Each stop comes with a short list to try.
- **Sharing blocks of pixels touches the engine's core**, and about 220 places in the code
  reach straight into a layer's pixels. Sharing goes in first, on its own, and every existing
  test, the old-file fixtures and the benchmark must come out identical before any animation
  code goes on top. After every frame change the screen and the layer thumbnails must show
  exactly that frame; tests compare pixels after stepping, not just the data.
- **Rebuilding the menus on one list** touches every menu, so the app will not be literally
  "today's app" even with animation off. It's done in its own commits with the shortcut tests kept.
- **The pen.** Pressure on frames uses the same path as today, but only you can feel it.

## One question

**Did testing 0.5.0 on Windows turn up anything?** Pen, shortcuts, lasso, copy and paste,
crash recovery, anything annoying or slow. Whatever you list joins Step 1.

## The owner's answers (design review)

Approved as written, with the four changes to the brief, and three additions:

1. **Painting speed is measured before and after tiles are shared**: a long brush stroke on
   a large canvas and the existing timing test and gallery renders. It must not get slower.
   The numbers go in the Step 3 report (the "before" is in `docs/phase6-notes.md`).
2. **"The old program files are put back if the setup fails halfway" is built and proven in
   Step 5**, not assumed of Inno Setup.
3. **The updater ignores pre-releases and drafts, and never installs a version older than or
   equal to the one running.**

Testing 0.5.0 on Windows: no answer yet. Anything reported later joins Step 1.
