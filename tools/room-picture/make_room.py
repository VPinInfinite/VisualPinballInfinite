"""Makes the player's default room picture.

    python -I tools/room-picture/make_room.py <rostock_laage_airport_1k.hdr> <VPX's EnvMap.webp> <out.png>

VPX draws a table that names no room picture (environment) of its own, or one it
doesn't hold, with its built-in `EnvMap.webp`, an airport hall, which dates from
2014, before VPX's GPL rule, and can't be taken. Ours is Poly Haven's
Rostock-Laage Airport (CC0, by Greg Zaal; the 1k Radiance file, see README.md),
made into a picture like VPX's:

- 512 x 256, an ordinary 8-bit sRGB picture, as VPX's is;
- its highlights rolled off softly rather than cut;
- graded above and below the horizon so the light it casts on parts facing up and
  on parts facing down is what VPX's room casts (VPX's is brighter above, its
  floor dark wood);
- its average colour that of VPX's room (VPX's is near neutral).

VPX's picture is only measured (its light up and down, its average colour), never
copied. Laid out as VPX reads an environment: across, the direction around the
table; down, from straight up to straight down.
"""
import sys

import numpy as np
from PIL import Image


def read_hdr(path):
    """A Radiance picture (new-style run-length scanlines) as linear RGB."""
    data = open(path, "rb").read()
    pos = 0
    header = []
    while True:
        end = data.index(b"\n", pos)
        line = data[pos:end]
        pos = end + 1
        if line.startswith(b"-Y") or line.startswith(b"+Y"):
            break
        header.append(line)
    if b"FORMAT=32-bit_rle_rgbe" not in b"\n".join(header):
        sys.exit("not a Radiance RGBE picture")
    parts = line.split()
    if parts[0] != b"-Y" or parts[2] != b"+X":
        sys.exit("unexpected picture orientation: " + line.decode(errors="replace"))
    h, w = int(parts[1]), int(parts[3])
    out = np.zeros((h, w, 4), np.uint8)
    for y in range(h):
        if data[pos] != 2 or data[pos + 1] != 2 or (data[pos + 2] << 8 | data[pos + 3]) != w:
            sys.exit("old-style scanlines aren't read")
        pos += 4
        for c in range(4):
            x = 0
            while x < w:
                n = data[pos]
                pos += 1
                if n > 128:
                    n -= 128
                    out[y, x:x + n, c] = data[pos]
                    pos += 1
                else:
                    out[y, x:x + n, c] = np.frombuffer(data, np.uint8, n, pos)
                    pos += n
                x += n
    m = out[..., :3].astype(np.float64)
    e = out[..., 3].astype(np.int32)
    f = np.where(e > 0, np.ldexp(1.0, e - 136), 0.0)
    return (m + 0.5) * f[..., None]


def srgb_to_linear(v):
    return np.where(v <= 0.04045, v / 12.92, ((v + 0.055) / 1.055) ** 2.4)


def linear_to_srgb(v):
    return np.where(v <= 0.0031308, v * 12.92, 1.055 * np.power(np.maximum(v, 0), 1 / 2.4) - 0.055)


def luminance(rgb):
    return rgb[..., 0] * 0.2126 + rgb[..., 1] * 0.7152 + rgb[..., 2] * 0.0722


def weights(h, w):
    """Each texel's share of the sphere."""
    wr = np.sin((np.arange(h) + 0.5) / h * np.pi)
    return np.repeat(wr[:, None], w, axis=1) / (wr.sum() * w)


def irradiance(lin):
    """The light on parts facing up and facing down (the top row straight up)."""
    h, w = lin.shape[:2]
    theta = (np.arange(h) + 0.5) / h * np.pi
    dw = np.sin(theta) * (np.pi / h) * (2 * np.pi / w)
    lum = luminance(lin).sum(axis=1)
    c = np.cos(theta)
    return {"up": (lum * np.maximum(c, 0) * dw).sum() / np.pi, "down": (lum * np.maximum(-c, 0) * dw).sum() / np.pi}


def shoulder(x, t=0.6):
    """Straight up to t, then rolling off softly towards 1."""
    over = np.maximum(x - t, 0)
    return np.where(x <= t, x, t + (1 - t) * (1 - np.exp(-over / (1 - t))))


def main():
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    hdr_path, vpx_path, out_path = sys.argv[1:]
    hdr = read_hdr(hdr_path)
    h, w = hdr.shape[:2]
    if (w, h) != (1024, 512):
        sys.exit(f"expected the 1k picture (1024 x 512), got {w} x {h}")
    small = hdr.reshape(h // 2, 2, w // 2, 2, 3).mean(axis=(1, 3))  # 512 x 256, in linear light
    vpx = srgb_to_linear(np.asarray(Image.open(vpx_path).convert("RGB"), np.float64) / 255)
    wt_vpx = weights(*vpx.shape[:2])
    wt = weights(*small.shape[:2])
    want = irradiance(vpx)
    vpx_rgb = np.array([(vpx[..., c] * wt_vpx).sum() for c in range(3)])
    # one gain above the horizon and one below, blended across it (20 degrees each way)
    rows = small.shape[0]
    lat = 90 - (np.arange(rows) + 0.5) / rows * 180
    s = np.clip((lat + 20) / 40, 0, 1)
    s = s * s * (3 - 2 * s)
    up, down, balance = 1.0, 1.0, np.ones(3)
    for _ in range(80):
        gain = (down + (up - down) * s)[:, None, None]
        out = shoulder(small * balance * gain)
        got = irradiance(out)
        up *= want["up"] / got["up"]
        down *= want["down"] / got["down"]
        rgb = np.array([(out[..., c] * wt).sum() for c in range(3)])
        balance *= (vpx_rgb / luminance(vpx_rgb)) / (rgb / luminance(rgb))
    gain = (down + (up - down) * s)[:, None, None]
    srgb = np.clip(linear_to_srgb(shoulder(small * balance * gain)), 0, 1)
    picture = (srgb * 255 + 0.5).astype(np.uint8)
    Image.fromarray(picture, "RGB").save(out_path, optimize=True)
    ours = srgb_to_linear(picture / 255)
    got = irradiance(ours)
    print(f"gain above the horizon {up:.4f}, below {down:.4f}; white balance {np.round(balance, 4)}")
    print(f"light on parts facing up: VPX's room {want['up']:.4f}, ours {got['up']:.4f}; facing down: {want['down']:.4f}, {got['down']:.4f}")
    print("average colour (sRGB): VPX's room", np.round(linear_to_srgb(vpx_rgb) * 255, 1),
          "ours", np.round(linear_to_srgb(np.array([(ours[..., c] * wt).sum() for c in range(3)])) * 255, 1))


main()
