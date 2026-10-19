#!/usr/bin/env python3
"""Inspect and edit Game Boy sprite PNGs as 2-bit pixel indices.

A sprite sheet is one or more PNG columns joined left to right (e.g. char-l.png | char-r.png,
the two 8-pixel halves of a 16x16 character). Frame N is rows N*H .. N*H+H-1 of every column
(H = --frame-height, 16 by default). Pixels are shown and written as indices:
'.' = 0 (lightest, transparent for sprites), '1', '2', '3' (darkest).

The palette is read from the images: their colours sorted light to dark are indices 0..3.

Commands:
  info    IMG...                         size, palette, frame count, problems
  check   IMG...                         like info; exit 1 on any problem
  show    IMG... [--frame N]             frames as index text (one '|' between columns)
  preview OUT.png IMG... [--scale S]     all frames on a grid, scaled up
  export  OUT.png IMG... --frame N       one frame, scaled up (input for an image model)
  paint   TEXT IMG... --frame N          write index text (as printed by show) into a frame
  import  SRC.png IMG... --frame N       a (model-made) picture of a frame -> quantized into a frame

paint/import write <name>.edited.png next to each column, or the columns themselves with
--in-place. Needs Pillow.
"""

import argparse
import sys
from pathlib import Path

from PIL import Image

DEFAULT_PALETTE = [(255, 255, 255), (170, 170, 170), (85, 85, 85), (0, 0, 0)]
SYMBOLS = ".123"


def luma(rgb):
    r, g, b = rgb[:3]
    return 0.299 * r + 0.587 * g + 0.114 * b


def load(paths):
    return [Image.open(p).convert("RGBA") for p in paths]


def problems(path, image):
    """Problems of one column, as text lines."""
    out = []
    w, h = image.size
    if w % 8 or h % 8:
        out.append(f"{path}: size {w}x{h} is not a multiple of 8")
    alphas = {a for (_, (_, _, _, a)) in image.getcolors(1 << 16) or []}
    if alphas - {0, 255}:
        out.append(f"{path}: semi-transparent pixels (alpha {sorted(alphas - {0, 255})})")
    colours = opaque_colours([image])
    if len(colours) > 4:
        out.append(f"{path}: {len(colours)} colours, at most 4 allowed")
    return out


def opaque_colours(images):
    colours = set()
    for image in images:
        for _, (r, g, b, a) in image.getcolors(1 << 16) or []:
            if a == 255:
                colours.add((r, g, b))
    return colours


def palette_of(images, default=False):
    """Colours for indices 0..3: the images' colours light to dark, padded with defaults."""
    if default:
        return list(DEFAULT_PALETTE)
    colours = sorted(opaque_colours(images), key=luma, reverse=True)
    if len(colours) > 4:
        sys.exit(f"error: {len(colours)} colours in the images, at most 4 allowed (run check)")
    if len(colours) == 4:
        return colours
    # Too few colours to know the roles: fall back to the default shades for the missing ones.
    pal = list(DEFAULT_PALETTE)
    for c in colours:
        pal[min(range(4), key=lambda i: abs(luma(DEFAULT_PALETTE[i]) - luma(c)))] = c
    return pal


def index_of(pixel, pal):
    if pixel[3] == 0:
        return 0
    rgb = pixel[:3]
    if rgb in pal:
        return pal.index(rgb)
    return min(range(4), key=lambda i: abs(luma(pal[i]) - luma(rgb)))


def frame_count(images, fh):
    return min(im.size[1] for im in images) // fh


def frame_rows(images, pal, frame, fh):
    """Index text of one frame: a list of rows, columns separated by '|'."""
    rows = []
    for y in range(frame * fh, frame * fh + fh):
        parts = []
        for im in images:
            parts.append("".join(SYMBOLS[index_of(im.getpixel((x, y)), pal)] for x in range(im.size[0])))
        rows.append("|".join(parts))
    return rows


def frame_image(images, pal, frame, fh):
    width = sum(im.size[0] for im in images)
    out = Image.new("RGB", (width, fh))
    x0 = 0
    for im in images:
        for y in range(fh):
            for x in range(im.size[0]):
                out.putpixel((x0 + x, y), pal[index_of(im.getpixel((x, frame * fh + y)), pal)])
        x0 += im.size[0]
    return out


def write_frame(images, paths, pal, frame, fh, grid, in_place):
    """Write a grid (rows of indices across all columns) into a frame and save the columns."""
    widths = [im.size[0] for im in images]
    total = sum(widths)
    if len(grid) != fh or any(len(row) != total for row in grid):
        sys.exit(f"error: frame must be {fh} rows of {total} pixels, got "
                 f"{len(grid)} rows of {sorted({len(r) for r in grid})}")
    x0 = 0
    for im, path, w in zip(images, paths, widths):
        for y in range(fh):
            for x in range(w):
                im.putpixel((x, frame * fh + y), pal[grid[y][x0 + x]] + (255,))
        x0 += w
        target = Path(path) if in_place else Path(path).with_suffix(".edited.png")
        im.save(target)
        print(f"wrote {target}")


def cmd_info(args, strict):
    images = load(args.images)
    found = []
    for path, im in zip(args.images, images):
        found += problems(path, im)
        print(f"{path}: {im.size[0]}x{im.size[1]}, {im.size[0] // 8 * (im.size[1] // 8)} tiles")
    if not found:
        pal = palette_of(images)
        print("palette (index: rgb):", ", ".join(f"{SYMBOLS[i]}: {c}" for i, c in enumerate(pal)))
        print(f"frames of height {args.frame_height}: {frame_count(images, args.frame_height)}")
        if pal != DEFAULT_PALETTE:
            print("not the library default palette: load with PngOptions::new([...these rgb...]) "
                  "or .mapping(PngColorMapping::Nearest)")
    for line in found:
        print("problem:", line)
    if strict and found:
        sys.exit(1)


def cmd_show(args):
    images = load(args.images)
    pal = palette_of(images, args.default_palette)
    frames = [args.frame] if args.frame is not None else range(frame_count(images, args.frame_height))
    for f in frames:
        print(f"== frame {f}")
        print("\n".join(frame_rows(images, pal, f, args.frame_height)))


def cmd_preview(args):
    images = load(args.images)
    pal = palette_of(images, args.default_palette)
    fh, n = args.frame_height, frame_count(images, args.frame_height)
    fw = sum(im.size[0] for im in images)
    per_row = args.per_row
    gap = 2
    sheet = Image.new("RGB", (per_row * (fw + gap) + gap, -(-n // per_row) * (fh + gap) + gap), (255, 0, 255))
    for f in range(n):
        sheet.paste(frame_image(images, pal, f, fh), (gap + (f % per_row) * (fw + gap), gap + (f // per_row) * (fh + gap)))
    sheet.resize((sheet.size[0] * args.scale, sheet.size[1] * args.scale), Image.NEAREST).save(args.out)
    print(f"wrote {args.out} ({n} frames, magenta = outside the frames)")


def cmd_export(args):
    images = load(args.images)
    pal = palette_of(images, args.default_palette)
    im = frame_image(images, pal, args.frame, args.frame_height)
    im.resize((im.size[0] * args.scale, im.size[1] * args.scale), Image.NEAREST).save(args.out)
    print(f"wrote {args.out} ({im.size[0]}x{im.size[1]} frame, x{args.scale})")


def cmd_paint(args):
    images = load(args.images)
    pal = palette_of(images, args.default_palette)
    text = Path(args.text).read_text() if args.text != "-" else sys.stdin.read()
    grid = []
    for line in text.splitlines():
        line = line.split("#")[0].strip().replace("|", "")
        if not line or line.startswith("=="):
            continue
        if set(line) - set(SYMBOLS):
            sys.exit(f"error: unexpected characters in {line!r} (use . 1 2 3 and |)")
        grid.append([SYMBOLS.index(c) for c in line])
    write_frame(images, args.images, pal, args.frame, args.frame_height, grid, args.in_place)


def cmd_import(args):
    """Each target pixel takes the most common index of its block in the source picture."""
    images = load(args.images)
    pal = palette_of(images, args.default_palette)
    fw, fh = sum(im.size[0] for im in images), args.frame_height
    src = Image.open(args.source).convert("RGBA")
    sw, sh = src.size
    grid = []
    for y in range(fh):
        row = []
        for x in range(fw):
            votes = [0, 0, 0, 0]
            for sy in range(y * sh // fh, max(y * sh // fh + 1, (y + 1) * sh // fh)):
                for sx in range(x * sw // fw, max(x * sw // fw + 1, (x + 1) * sw // fw)):
                    votes[index_of(src.getpixel((sx, sy)), pal)] += 1
            row.append(votes.index(max(votes)))
        grid.append(row)
    print("\n".join("".join(SYMBOLS[i] for i in row) for row in grid))
    write_frame(images, args.images, pal, args.frame, fh, grid, args.in_place)


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--frame-height", type=int, default=16)
    p.add_argument("--default-palette", action="store_true",
                   help="use the library default shades instead of the images' colours")
    sub = p.add_subparsers(dest="cmd", required=True)
    for name in ("info", "check"):
        s = sub.add_parser(name)
        s.add_argument("images", nargs="+")
    s = sub.add_parser("show")
    s.add_argument("images", nargs="+")
    s.add_argument("--frame", type=int)
    s = sub.add_parser("preview")
    s.add_argument("out")
    s.add_argument("images", nargs="+")
    s.add_argument("--scale", type=int, default=6)
    s.add_argument("--per-row", type=int, default=8)
    s = sub.add_parser("export")
    s.add_argument("out")
    s.add_argument("images", nargs="+")
    s.add_argument("--frame", type=int, required=True)
    s.add_argument("--scale", type=int, default=16)
    for name, first in (("paint", "text"), ("import", "source")):
        s = sub.add_parser(name)
        s.add_argument(first)
        s.add_argument("images", nargs="+")
        s.add_argument("--frame", type=int, required=True)
        s.add_argument("--in-place", action="store_true")
    args = p.parse_args()
    if args.cmd in ("info", "check"):
        cmd_info(args, args.cmd == "check")
    else:
        {"show": cmd_show, "preview": cmd_preview, "export": cmd_export,
         "paint": cmd_paint, "import": cmd_import}[args.cmd](args)


if __name__ == "__main__":
    main()
