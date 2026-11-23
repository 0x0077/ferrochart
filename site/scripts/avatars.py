#!/usr/bin/env python3
"""Generates the wallet avatars of the memecoin examples (public/avatars/):
original, deterministic identicons, 64 × 64 PNG. Run from site/:
    python3 scripts/avatars.py
Needs Pillow. The output is committed; rerunning gives the same files."""

import colorsys
import hashlib
from pathlib import Path

from PIL import Image

OUT = Path(__file__).resolve().parent.parent / 'public' / 'avatars'
COUNT = 16
GRID = 5  # pattern cells per side, mirrored around the middle column
PAD = 1  # empty cells around it, so a round crop keeps the pattern
SIDE = 63  # a multiple of the 7 cells


def avatar(n: int) -> Image.Image:
    h = hashlib.sha256(f'ferrochart-wallet-{n}'.encode()).digest()
    hue = (n / COUNT + h[0] / 2560) % 1.0
    fg = tuple(round(c * 255) for c in colorsys.hls_to_rgb(hue, 0.58, 0.75))
    bg = tuple(round(c * 255) for c in colorsys.hls_to_rgb((hue + 0.5) % 1.0, 0.16, 0.35))
    cells = GRID + 2 * PAD
    img = Image.new('RGB', (cells, cells), bg)
    bits = int.from_bytes(h[1:9], 'big')
    half = (GRID + 1) // 2
    for y in range(GRID):
        for x in range(half):
            if bits >> (y * half + x) & 1:
                img.putpixel((PAD + x, PAD + y), fg)
                img.putpixel((PAD + GRID - 1 - x, PAD + y), fg)
    return img.resize((SIDE, SIDE), Image.NEAREST)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for n in range(1, COUNT + 1):
        avatar(n).save(OUT / f'w{n:02}.png', optimize=True)


if __name__ == '__main__':
    main()
