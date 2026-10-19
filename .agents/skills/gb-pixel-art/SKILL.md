---
name: gb-pixel-art
description: Create, edit or check Game Boy (DMG) sprite and tile PNGs for rust-boy examples — recolour or redraw frames (e.g. "put a shirt and jeans on the character"), turn an image-model picture into valid 2-bit pixel art, check the palette and size, preview animation frames. Use for any change to the .png art in examples/<name>/.
---

# Game Boy pixel art

The art is tiny (an 8×8 tile is 64 pixels) and every pixel is an index 0–3, so edit it **as indices, pixel by
pixel**, with `scripts/gbsprite.py` (Python 3 + Pillow). Image models can help with ideas, but their output is
never valid as it is: it must go through `import` and then be fixed by hand.

## Hardware rules (DMG)

- **4 shades per palette.** A pixel stores an index 0–3; the palette register maps it to a shade. Background
  tiles use `BGP`; sprites use `OBP0` or `OBP1`.
- **Sprites: index 0 is transparent**, so a sprite has **3 visible shades** (1, 2, 3). So every part of a
  character shares those 3 indices: skin, hair, clothes, outline. Areas next to each other need different
  indices to read, and the outline is usually 3.
- **The palette is per sprite, not per pixel.** Each OBJ (one 8×8 or 8×16 piece) chooses OBP0 or OBP1. You cannot
  give a shirt its own palette inside one sprite. Colours (a blue for "jeans") do not exist on the DMG; only the
  shade does.
- **Tiles are 8×8.** All sprites are 8×8 or all are 8×16 (one LCDC bit for the whole screen). A 16×16 character is
  two 8×16 sprites side by side = 4 tiles per frame.
- **Limits:** 40 sprites in OAM, **at most 10 on one scanline** (a 16×16 character costs 2 on each of its lines),
  the library allows 256 sprite tiles (a 32-frame 16×16 character uses 128 of them).

## Asset conventions in this repo

- PNG, opaque RGBA (or indexed). Width and height are multiples of 8, and of 16 in height for 8×16 sprites. No
  semi-transparent pixels.
- **Index = order from lightest to darkest.** The lightest colour (white, the background) is index 0. An image has
  at most 4 colours.
- The library's default palette is `255,255,255 / 170,170,170 / 85,85,85 / 0,0,0` with exact matching. Art in
  other shades loads with `PngOptions::new([...its four rgb...])` or `.mapping(PngColorMapping::Nearest)`.
  `gbsprite.py info` prints which applies. **Keep an asset's own palette when you edit it**: the script does.
- Vertical strips of 8×16 sprites load with `.tile_order(TileOrder::Sprite8x16)`.
- A wide character is split into columns: `char-l.png` | `char-r.png` are the left and right 8-pixel halves.
  Frame N is rows `16N … 16N+15` in **both** files. Always pass the columns together, left to right.

### Known assets

- `examples/olive-empire/char-l.png`, `char-r.png`: 32 frames of a 16×16 character, palette
  `255,255,255 / 207,207,207 / 104,104,104 / 0,0,1`. Roles: 1 = skin (light grey), 2 = hair and shoes,
  3 = outline and eyes.

## Art rules

1. **Keep the silhouette and the pose**: the outline (3) and the transparent pixels around it stay where they are,
   unless the change is to the shape itself.
2. **One role, one index, in every frame.** Before you edit, write down the role map (e.g. "shirt = 2, jeans = 3,
   hands = 1") and say it to the user. With 3 visible shades, two roles must share one: choose the pairs that
   never touch (hair and shirt are separated by the face).
3. **No anti-aliasing, no gradients, no dithering** (unless asked). Every pixel is exactly one of the 4 colours.
4. **A change to a character is a change to all its frames.** Edit one frame first, show it, and after the user
   approves, repeat it on every frame. For each frame, check where the body parts are, because the pose changes
   between frames.
5. **Never overwrite the originals without asking.** `paint` and `import` write `<name>.edited.png`. Use
   `--in-place` only when the user has said so.

## Tool: `scripts/gbsprite.py`

Run from the repo root; `S=.agents/skills/gb-pixel-art/scripts/gbsprite.py`.

```bash
python3 $S check   char-l.png char-r.png               # sizes, ≤4 colours, alpha; exit 1 on a problem
python3 $S show    char-l.png char-r.png --frame 0     # the frame as text: . 1 2 3, '|' between columns
python3 $S preview /tmp/sheet.png char-l.png char-r.png  # every frame on a grid, ×6 (look at it)
python3 $S export  /tmp/f0.png char-l.png char-r.png --frame 0 --scale 16   # one frame, scaled up
python3 $S paint   f0.txt char-l.png char-r.png --frame 0                   # text -> frame 0
python3 $S import  model.png char-l.png char-r.png --frame 0                # picture -> frame 0
```

`--frame-height` (default 16) changes the frame size; `--default-palette` uses the library's shades.

## Workflow A: exact edit (preferred)

1. `check`, then `show --frame N`, and read the frame: find the head, torso, arms, legs and feet rows.
2. Write the new frame as text in a file, with the same lines and widths, keeping `|`. Comments after `#` are
   allowed. Change only the pixels of the edit, and note each changed row's role in a comment.
3. `paint` it, `check` the `.edited.png` files, then `export` the before and after frames and **look at them**
   (read the PNGs) before showing them to the user. Say what changed and the role map.

## Workflow B: with an image model (Codex `$imagegen`, Gemini Nano Banana, …)

Claude cannot generate images; Codex can. Use a model for a new design or a big change, not for a recolour.

1. `export /tmp/fN.png ... --frame N --scale 16`: a 256×256 picture of a 16×16 frame.
2. Prompt the model with that image, for example:

   > Edit this 16×16 Game Boy sprite, shown scaled up 16× (each 16×16 block of the image is one pixel). Put a
   > shirt and jeans on the character. Keep the exact pose, outline and size. Use only these 4 flat colours:
   > #FFFFFF (background), #CFCFCF, #686868, #000001; no other colours, no anti-aliasing, no gradients, no
   > shading. Keep the background #FFFFFF. Output a 256×256 PNG with the same 16-pixel grid.

3. `import model.png ... --frame N`: each pixel becomes the most common index in its block. The output is printed
   as text.
4. Fix the printed frame by hand (Workflow A, steps 2–3). Expect stray pixels, a moved outline and roles in the
   wrong index.

## Using the art in the game

Loading the edited PNGs directly, for example (palette of the olive-empire character):

```rust
let options = PngOptions::new([[255; 3], [207; 3], [104; 3], [0, 0, 1]])
    .tile_order(TileOrder::Sprite8x16);
let left = TileSource::from_png(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/olive-empire/char-l.png"), &options)?;
```

After an art change that reaches an example's output, the example's snapshot changes too (see `CLAUDE.md`).
