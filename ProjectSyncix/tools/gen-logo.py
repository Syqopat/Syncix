# Generates the Marketplace icon (128x128 PNG).
#
# Why by hand: there is no Pillow in this environment, and on Windows `convert`
# is not ImageMagick but a file-system converter — dangerous to run. Writing a PNG
# is just zlib and a few header bytes anyway; no external tool is needed.
#
# The drawing uses the SAME geometry as logo.svg. For anti-aliasing it is drawn at
# 4x size and scaled down with a box average.

import zlib, struct, os

SCALE = 4
SIZE = 128
B = SIZE * SCALE

BACKGROUND = (0x1E, 0x21, 0x28)
WHITE = (0xFF, 0xFF, 0xFF)
BLUE = (0x4C, 0x8D, 0xF5)

CORNER = 28 * SCALE
HALF_WIDTH = 9 * SCALE / 2.0


def distance_to_segment(px, py, x1, y1, x2, y2):
    """Distance from a point to a LINE SEGMENT. Round caps and joins come for free:
    a distance threshold around a segment makes its ends semicircles."""
    dx, dy = x2 - x1, y2 - y1
    length2 = dx * dx + dy * dy
    if length2 == 0:
        t = 0.0
    else:
        t = ((px - x1) * dx + (py - y1) * dy) / length2
        t = 0.0 if t < 0 else (1.0 if t > 1 else t)
    ux, uy = x1 + t * dx - px, y1 + t * dy - py
    return (ux * ux + uy * uy) ** 0.5


# The same coordinates as logo.svg, scaled by SCALE.
def s(v):
    return v * SCALE


WHITE_SEGMENTS = [
    (s(30), s(52), s(74), s(52)),
    (s(64), s(40), s(76), s(52)),
    (s(76), s(52), s(64), s(64)),
]
BLUE_SEGMENTS = [
    (s(98), s(80), s(54), s(80)),
    (s(64), s(68), s(52), s(80)),
    (s(52), s(80), s(64), s(92)),
]


def outside_corner(x, y):
    """Is the point outside the rounded rectangle?"""
    for cx, cy in ((CORNER, CORNER), (B - CORNER, CORNER), (CORNER, B - CORNER), (B - CORNER, B - CORNER)):
        if (x < CORNER or x > B - CORNER) and (y < CORNER or y > B - CORNER):
            if abs(x - cx) <= CORNER and abs(y - cy) <= CORNER:
                if ((x - cx) ** 2 + (y - cy) ** 2) ** 0.5 > CORNER:
                    return True
    return False


# Draw the large canvas
large = bytearray(B * B * 4)
for y in range(B):
    row = y * B * 4
    for x in range(B):
        i = row + x * 4
        if outside_corner(x + 0.5, y + 0.5):
            continue  # stays transparent
        r, g, b = BACKGROUND
        for segments, color in ((WHITE_SEGMENTS, WHITE), (BLUE_SEGMENTS, BLUE)):
            for (x1, y1, x2, y2) in segments:
                if distance_to_segment(x + 0.5, y + 0.5, x1, y1, x2, y2) <= HALF_WIDTH:
                    r, g, b = color
                    break
        large[i] = r
        large[i + 1] = g
        large[i + 2] = b
        large[i + 3] = 255

# Scale down with a box average (this is where the anti-aliasing comes from)
small = bytearray(SIZE * SIZE * 4)
area = SCALE * SCALE
for y in range(SIZE):
    for x in range(SIZE):
        tr = tg = tb = ta = 0
        for dy in range(SCALE):
            base = ((y * SCALE + dy) * B + x * SCALE) * 4
            for dx in range(SCALE):
                i = base + dx * 4
                a = large[i + 3]
                tr += large[i] * a
                tg += large[i + 1] * a
                tb += large[i + 2] * a
                ta += a
        j = (y * SIZE + x) * 4
        if ta:
            small[j] = tr // ta
            small[j + 1] = tg // ta
            small[j + 2] = tb // ta
        small[j + 3] = ta // area

# Write the PNG
raw = bytearray()
for y in range(SIZE):
    raw.append(0)  # no filter
    raw += small[y * SIZE * 4:(y + 1) * SIZE * 4]


def chunk(kind, data):
    body = kind + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)


png = (
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
    + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    + chunk(b"IEND", b"")
)

target = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "vscode-extension", "resources", "logo.png",
)
open(target, "wb").write(png)
print("  wrote: %s (%d bytes)" % (os.path.basename(target), len(png)))
