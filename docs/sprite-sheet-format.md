# Sprite sheets: the PNG and its JSON

`quill export duck.json`, for a script with animations, writes `duck.png` and `duck.json` (and
the app's **File → Export animation** does the same). This page says what's in them, so another program, such as Duck
Forge, can load them.

## The picture

One PNG with a see-through background. Every frame is the same size (the script's canvas,
times `scale`). The frames sit in a grid: **one row per animation**, that animation's frames
left to right from frame 0, every cell the frame size, and shorter rows padded on the right with
empty cells. Frames are **numbered across the grid, row by row, from 0**: the frame in row `r`
and column `c` is number `r × columns + c`. Cutting the picture into cells of the frame size, in
reading order, gives exactly that numbering (`sprite.frames(w, h)` in Duck Forge).

## The JSON

```json
{
  "image": "duck.png",
  "frame": { "width": 128, "height": 128 },
  "columns": 8,
  "rows": 2,
  "scale": 1,
  "pivot": [64.0, 116.0],
  "animations": {
    "idle": { "row": 0, "frames": [0, 1, 2, 3], "fps": 6.0, "loop": true },
    "walk": { "row": 1, "frames": [8, 9, 10, 11, 12, 13, 14, 15], "fps": 12.0, "loop": true }
  }
}
```

| Field | |
|---|---|
| `image` | The sheet's file name, next to the JSON. |
| `frame` | One cell's size in pixels (`width`, `height`), after scaling. |
| `columns`, `rows` | The grid: `columns` is the longest animation's frame count; `rows` is the number of animations. The picture is `columns × width` by `rows × height`. |
| `scale` | How many times the frames were enlarged with sharp pixels (1 = as painted). Pixel art is usually exported at 1 for a game and at 8 for people to look at. |
| `pivot` | `[x, y]` in frame pixels (after scaling): the character's anchor point. Draw the frame so this point lands where the character stands. Default: the bottom centre of the frame, so a character stands on its feet and switching animations never moves it. The script sets it with a top-level `"pivot": [x, y]`. |
| `animations` | By name, in the order the script wrote them. `row` is its row; `frames` are its frame numbers in the grid, in play order; `fps` how fast; `loop` whether the last frame leads back to the first. |

Playing an animation: for each number in `frames`, show cell number `n` for `1 / fps` seconds;
at the end, start again if `loop`, otherwise stay on the last frame. In Duck Forge, that's
`Animation::new(frames, fps)` with the frame list.

## Other exports from the same script

- `--frames`: every frame as its own PNG, `duck-walk-003.png` (animation name, then the frame
  number with three digits).
- `--gif`: `duck-walk.gif`, looping if the animation loops, at its fps.
- `--mp4`: `duck-walk.mp4` through `ffmpeg`, on the `--bg` colour (a video has no see-through
  background; the paper colour if none is given). Without `ffmpeg`, the frames are written and
  the exact command to run is printed.
- `--scale 8` enlarges all of them with sharp pixels; `--animation walk` exports one animation.

## Tilesets

`quill atlas grass.json water.json sand.json --name ground` paints each script (one tile or
sprite each, at full size on a see-through background) and packs them into `ground.png` with
`ground.json` beside it. Tiles are named after their script files and stay in the order given.

```json
{
  "image": "ground.png",
  "scale": 1,
  "padding": 1,
  "tile": { "width": 16, "height": 16 },
  "columns": 2,
  "rows": 2,
  "tiles": {
    "grass": { "index": 0, "x": 1, "y": 1, "width": 16, "height": 16 },
    "water": { "index": 1, "x": 18, "y": 1, "width": 16, "height": 16 },
    "sand":  { "index": 2, "x": 1, "y": 18, "width": 16, "height": 16 }
  }
}
```

| Field | |
|---|---|
| `image` | The PNG's file name, next to the JSON. |
| `scale` | How many times the tiles were enlarged with sharp pixels (`--scale`). |
| `padding` | See-through pixels between tiles and round the edge (`--padding`, default 0). With 0 and tiles of one size, the picture is an exact grid. |
| `tile`, `columns`, `rows` | Only when every tile is the same size: the grid, so a game can also cut the picture by the tile size (tile number `n` is in row `n / columns`, column `n % columns`). |
| `tiles` | By name: `index` (the order given), and `x`, `y`, `width`, `height` of its rectangle in the PNG. Tiles of different sizes are packed in rows (`--columns` per row), each row as tall as its tallest tile. |

A script with animations gives its first frame (use `quill export` for all its frames).

**Many tiles in one script.** `quill export blocks.json`, for a script with no `animations`,
writes the same two files (`blocks.png`, `blocks.json`) from one script: every symbol it
`define`s is a tile, named after the symbol, or only those in its `"stills"` list
(`docs/script-format.md`, "Stills"). The JSON is exactly the one above.

## Regions

A script can name rectangles of its picture (`region`, and `skin_box` for the six faces of an
unwrapped box: `docs/script-format.md`, "Regions"). When it does, `quill render skin.json`
writes `skin.regions.json` beside `skin.png`, and `quill export` writes it beside the sheet:

```json
{
  "image": "skin.png",
  "width": 64,
  "height": 32,
  "scale": 1,
  "regions": {
    "head.top":    { "x": 8,  "y": 0, "width": 8, "height": 8 },
    "head.bottom": { "x": 16, "y": 0, "width": 8, "height": 8 },
    "head.right":  { "x": 0,  "y": 8, "width": 8, "height": 8 },
    "head.front":  { "x": 8,  "y": 8, "width": 8, "height": 8 },
    "head.left":   { "x": 16, "y": 8, "width": 8, "height": 8 },
    "head.back":   { "x": 24, "y": 8, "width": 8, "height": 8 },
    "hat_icon":    { "x": 48, "y": 0, "width": 8, "height": 8 }
  },
  "boxes": {
    "head": { "x": 0, "y": 0, "size": [8, 8, 8],
              "faces": ["head.top", "head.bottom", "head.right", "head.front", "head.left", "head.back"] }
  }
}
```

| Field | |
|---|---|
| `image`, `width`, `height` | The picture the regions are in, and its size in pixels. |
| `scale` | How many times the picture was enlarged (`--scale`). Every number below is already multiplied by it. |
| `frame` | Only beside a sheet of frames or stills: one cell's `width` and `height`. The regions are then inside **each** cell, measured from that cell's top-left corner. |
| `regions` | By name, in the order the script declared them: `x`, `y` (top-left corner, y down), `width`, `height`, in the picture's pixels. |
| `boxes` | Only if the script has `skin_box`es. By name: `x`, `y` of the unwrapped shape's top-left corner, `size` as `[width, height, depth]`, and `faces`, the names of its six regions in the order top, bottom, right, front, left, back. |

For a box, the texture coordinates of a face are its region divided by the picture's size:
`u0 = x / width`, `v0 = y / height`, `u1 = (x + region width) / width`,
`v1 = (y + region height) / height`. "right" and "left" are the character's own right and left.
