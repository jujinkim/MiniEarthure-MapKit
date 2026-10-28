#!/usr/bin/env python3
"""Reproduce the original MIT storefront sign atlas (godot/textures/sign_atlas.png).

512 signs (8 categories x 64) of 64x20 px in a 16x32 grid (drawn at 2x and
downsampled), from fixed seeds:
background, border, an optional pictogram and 2-6 glyphs of an invented,
language-neutral script. No font, text or external image is used.
"""
import argparse
import math
import random
from pathlib import Path

from PIL import Image, ImageDraw

COLUMNS, ROWS, WIDTH, HEIGHT = 16, 32, 128, 40  # Drawing size; saved at half.
# Category rows of 64 signs: (backgrounds, ink colours, pictogram).
CATEGORIES = [
    ("food", [(196, 40, 32), (226, 110, 30), (240, 180, 40), (150, 30, 40)], [(255, 248, 225), (40, 20, 10)], "bowl"),
    ("cafe", [(92, 60, 40), (40, 90, 60), (230, 220, 200), (30, 30, 30)], [(250, 240, 220), (200, 150, 90)], "cup"),
    ("shop", [(30, 110, 200), (230, 60, 120), (40, 160, 120), (250, 200, 30)], [(255, 255, 255), (20, 20, 40)], "bag"),
    ("clinic", [(245, 245, 245), (220, 236, 245), (230, 245, 235), (255, 255, 255)], [(200, 30, 40), (20, 120, 80)], "cross"),
    ("finance", [(20, 40, 90), (10, 70, 120), (240, 240, 240), (40, 40, 50)], [(240, 200, 80), (255, 255, 255)], "coin"),
    ("beauty", [(230, 120, 170), (150, 70, 160), (250, 220, 230), (40, 30, 50)], [(255, 255, 255), (120, 30, 90)], "star"),
    ("academy", [(30, 90, 170), (40, 140, 70), (250, 250, 240), (220, 120, 30)], [(255, 255, 255), (20, 40, 90)], "book"),
    ("store", [(20, 150, 90), (230, 80, 30), (30, 70, 160), (250, 250, 250)], [(255, 255, 255), (230, 40, 40)], "stripe"),
]


def glyph(draw, rng, x, y, size, ink):
    """One glyph of the invented script: 3-5 strokes between neighbouring
    points of a 3x4 lattice, so glyphs read as letters rather than scratches."""
    step_x, step_y = size * 0.4, size * 0.3
    width = max(3, size // 5)
    strokes, count = set(), rng.randint(3, 5)
    while len(strokes) < count:
        i, j = rng.randint(0, 2), rng.randint(0, 3)
        di, dj = rng.choice([(1, 0), (0, 1), (1, 1), (1, -1), (0, 2), (2, 0)])
        if 0 <= i + di <= 2 and 0 <= j + dj <= 3: strokes.add((i, j, i + di, j + dj))
    for i, j, k, l in sorted(strokes):
        draw.line([(x + i * step_x, y + j * step_y), (x + k * step_x, y + l * step_y)], fill=ink, width=width)
    if rng.random() < 0.2:
        cx, cy, r = x + rng.randint(0, 2) * step_x, y + rng.randint(0, 3) * step_y, size * 0.1
        draw.ellipse([cx - r, cy - r, cx + r, cy + r], fill=ink)


def pictogram(draw, kind, x, y, size, ink):
    s = size
    if kind == "bowl":
        draw.pieslice([x, y + s * 0.2, x + s, y + s * 1.1], 0, 180, fill=ink)
    elif kind == "cup":
        draw.rectangle([x + s * 0.15, y + s * 0.25, x + s * 0.7, y + s * 0.85], fill=ink)
        draw.arc([x + s * 0.55, y + s * 0.35, x + s * 0.95, y + s * 0.7], -90, 90, fill=ink, width=3)
    elif kind == "bag":
        draw.rectangle([x + s * 0.15, y + s * 0.35, x + s * 0.85, y + s * 0.95], fill=ink)
        draw.arc([x + s * 0.3, y + s * 0.05, x + s * 0.7, y + s * 0.55], 180, 360, fill=ink, width=3)
    elif kind == "cross":
        draw.rectangle([x + s * 0.38, y + s * 0.1, x + s * 0.62, y + s * 0.9], fill=ink)
        draw.rectangle([x + s * 0.1, y + s * 0.38, x + s * 0.9, y + s * 0.62], fill=ink)
    elif kind == "coin":
        draw.ellipse([x + s * 0.1, y + s * 0.1, x + s * 0.9, y + s * 0.9], outline=ink, width=4)
        draw.line([x + s * 0.5, y + s * 0.25, x + s * 0.5, y + s * 0.75], fill=ink, width=3)
    elif kind == "star":
        cx, cy, r = x + s / 2, y + s / 2, s * 0.45
        points = [(cx + (r if i % 2 == 0 else r * 0.45) * math.cos(math.pi / 2 + i * math.pi / 5),
                   cy - (r if i % 2 == 0 else r * 0.45) * math.sin(math.pi / 2 + i * math.pi / 5)) for i in range(10)]
        draw.polygon(points, fill=ink)
    elif kind == "book":
        draw.polygon([(x + s * 0.1, y + s * 0.2), (x + s * 0.5, y + s * 0.3), (x + s * 0.9, y + s * 0.2),
                      (x + s * 0.9, y + s * 0.85), (x + s * 0.5, y + s * 0.95), (x + s * 0.1, y + s * 0.85)], outline=ink, width=3)
        draw.line([x + s * 0.5, y + s * 0.3, x + s * 0.5, y + s * 0.95], fill=ink, width=2)
    else:
        for i in range(3):
            draw.rectangle([x + i * s * 0.33, y, x + i * s * 0.33 + s * 0.2, y + s], fill=ink)


def sign(rng, category):
    name, backgrounds, inks, icon = category
    image = Image.new("RGB", (WIDTH, HEIGHT), rng.choice(backgrounds))
    draw = ImageDraw.Draw(image)
    background = image.getpixel((0, 0))
    ink = max(inks, key=lambda c: sum(abs(a - b) for a, b in zip(c, background)) + rng.random() * 120)
    style = rng.randint(0, 3)
    if style == 1: draw.rectangle([1, 1, WIDTH - 2, HEIGHT - 2], outline=ink, width=2)
    elif style == 2: draw.rectangle([0, HEIGHT - 6, WIDTH, HEIGHT], fill=ink)
    elif style == 3: draw.rectangle([0, 0, 10, HEIGHT], fill=ink)
    x = 16 if style == 3 else 8
    if rng.random() < 0.7:
        pictogram(draw, icon, x, 6, 28, ink)
        x += 34
    count = rng.randint(2, 5)
    size = min(24, (WIDTH - x - 6) // count)
    for i in range(count):
        glyph(draw, rng, x + i * size + 2, (HEIGHT - (size - 6) * 0.9) / 2, size - 6, ink)
    return image


def build(path):
    atlas = Image.new("RGB", (COLUMNS * WIDTH, ROWS * HEIGHT))
    for index in range(COLUMNS * ROWS):
        category = CATEGORIES[index // 64]
        rng = random.Random(f"miniearthure-sign-{index}")
        atlas.paste(sign(rng, category), ((index % COLUMNS) * WIDTH, (index // COLUMNS) * HEIGHT))
    atlas.resize((COLUMNS * WIDTH // 2, ROWS * HEIGHT // 2), Image.LANCZOS).save(path, optimize=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, nargs="?", default=Path(__file__).resolve().parents[1] / "godot/textures/sign_atlas.png")
    build(parser.parse_args().output)
