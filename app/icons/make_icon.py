"""Draws the app's icon (icon.ico, icon.png): a glowing ring, violet into blue, around a glowing infinity,
on a dark disc. The small sizes (16 to 32 pixels) bolder, so it holds there. Ours
(GPL-3.0-or-later), drawn with Pillow.

usage: python -I make_icon.py <folder to write into>
"""
import sys

from PIL import Image, ImageChops, ImageDraw, ImageFilter

SIZE = 1024
UNIT = SIZE / 64  # the logo's own grid is 64 by 64
DISC = (13, 16, 22, 255)
# violet into blue through a softer blue: the app's backglass and playfield colours
STOPS = [(0.0, (180, 140, 255)), (0.5, (127, 168, 255)), (1.0, (76, 194, 255))]

# the infinity: four cubic curves around (32, 38), drawn smaller about the logo's middle
LOOP = [
    ((32, 38), (38, 28), (52, 28), (52, 38)),
    ((52, 38), (52, 48), (38, 48), (32, 38)),
    ((32, 38), (26, 28), (12, 28), (12, 38)),
    ((12, 38), (12, 48), (26, 48), (32, 38)),
]
LOOP_SCALE = 0.86
RING_RADIUS = 26
RING_WIDTH = 4.57


def cubic(p0, p1, p2, p3, steps=160):
    for i in range(steps + 1):
        t = i / steps
        u = 1 - t
        yield tuple(u * u * u * a + 3 * u * u * t * b + 3 * u * t * t * c + t * t * t * d for a, b, c, d in zip(p0, p1, p2, p3))


def gradient(x0, x1):
    """The colours from left (x0) to right (x1), in the logo's units, as a whole picture."""
    row = Image.new("RGB", (SIZE, 1))
    for px in range(SIZE):
        f = min(1.0, max(0.0, (px / UNIT - x0) / (x1 - x0)))
        for (a, ca), (b, cb) in zip(STOPS, STOPS[1:]):
            if a <= f <= b:
                k = (f - a) / (b - a)
                row.putpixel((px, 0), tuple(round(ca[i] + (cb[i] - ca[i]) * k) for i in range(3)))
                break
    return row.resize((SIZE, SIZE), Image.NEAREST).convert("RGBA")


def ring_mask(width):
    mask = Image.new("L", (SIZE, SIZE), 0)
    draw = ImageDraw.Draw(mask)
    c, r = 32 * UNIT, RING_RADIUS * UNIT
    outer, inner = r + width * UNIT / 2, r - width * UNIT / 2
    draw.ellipse((c - outer, c - outer, c + outer, c + outer), fill=255)
    draw.ellipse((c - inner, c - inner, c + inner, c + inner), fill=0)
    return mask


def loop_mask(width):
    mask = Image.new("L", (SIZE, SIZE), 0)
    draw = ImageDraw.Draw(mask)
    points = [((32 + (x - 32) * LOOP_SCALE) * UNIT, (32 + (y - 38) * LOOP_SCALE) * UNIT) for curve in LOOP for x, y in cubic(*curve)]
    w = width * LOOP_SCALE * UNIT
    draw.line(points, fill=255, width=round(w), joint="curve")
    for x, y in points[:: max(1, len(points) // 400)]:
        draw.ellipse((x - w / 2, y - w / 2, x + w / 2, y + w / 2), fill=255)
    return mask


def paint(img, colour, mask, opacity=1.0, blur=0.0):
    """`colour` (a picture) laid over `img` through `mask`, softened by `blur` logo units."""
    if blur:
        mask = mask.filter(ImageFilter.GaussianBlur(blur * UNIT))
    if opacity < 1:
        mask = mask.point(lambda v: round(v * opacity))
    layer = colour.copy()
    layer.putalpha(ImageChops.multiply(layer.getchannel("A"), mask))
    img.alpha_composite(layer)


def draw(bold):
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    disc = Image.new("L", (SIZE, SIZE), 0)
    c, r = 32 * UNIT, RING_RADIUS * UNIT
    ImageDraw.Draw(disc).ellipse((c - r, c - r, c + r, c + r), fill=255)
    paint(img, Image.new("RGBA", (SIZE, SIZE), DISC), disc)
    ring_colours, loop_colours = gradient(3, 61), gradient(32 + (12 - 32) * LOOP_SCALE, 32 + (52 - 32) * LOOP_SCALE)
    white = Image.new("RGBA", (SIZE, SIZE), (255, 255, 255, 255))
    k = 1.45 if bold else 1.0
    # the ring: its glow, itself, a white line along it
    paint(img, ring_colours, ring_mask(RING_WIDTH * k + 1.5), 0.5, 1.2)
    paint(img, ring_colours, ring_mask(RING_WIDTH * k))
    if not bold:
        paint(img, white, ring_mask(1.1), 0.4)
    # the infinity: its glow, itself, a white line along it
    paint(img, loop_colours, loop_mask(9 * k), 0.55, 2.2 * LOOP_SCALE)
    paint(img, loop_colours, loop_mask(5.6 * k))
    if not bold:
        paint(img, white, loop_mask(1.5), 0.55)
    return img


def main():
    out = sys.argv[1]
    full, bold = draw(False), draw(True)
    full.resize((512, 512), Image.LANCZOS).save(f"{out}/icon.png")
    small = [bold.resize((s, s), Image.LANCZOS) for s in (16, 20, 24, 32)]
    large = [full.resize((s, s), Image.LANCZOS) for s in (40, 48, 64, 128)]
    full.resize((256, 256), Image.LANCZOS).save(f"{out}/icon.ico", sizes=[(s, s) for s in (16, 20, 24, 32, 40, 48, 64, 128, 256)], append_images=small + large)
    print("icon.ico and icon.png written")


if __name__ == "__main__":
    main()
