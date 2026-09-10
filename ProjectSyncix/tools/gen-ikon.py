# Generates the sidebar icon (PNG).
#
# Why PNG and not SVG: vsce refuses to publish extensions that contain
# user-provided SVG images ("Due to security concerns, vsce will not publish
# extensions that contain user-provided SVG images"), so the package ships a PNG.
#
# The background is TRANSPARENT: VS Code uses the icon as a mask and tints it
# for the current theme.

import zlib, struct, os

SCALE, BOX = 8, 24
B = BOX * SCALE
COLOR = (0xC5, 0xC5, 0xC5)


def s(v):
    return v * SCALE


# Rounded rectangles: (x, y, width, height, radius)
RECTANGLES = [(3, 7.6, 10, 2.8, 1.4), (11, 13.6, 10, 2.8, 1.4)]
# Triangles
TRIANGLES = [
    ((12.2, 4.8), (18.6, 9.0), (12.2, 13.2)),
    ((11.8, 10.8), (5.4, 15.0), (11.8, 19.2)),
]


def in_rectangle(px, py, rect):
    x, y, w, h, r = (s(v) for v in rect)
    if not (x <= px <= x + w and y <= py <= y + h):
        return False
    for cx, cy in ((x + r, y + r), (x + w - r, y + r), (x + r, y + h - r), (x + w - r, y + h - r)):
        if (px < x + r or px > x + w - r) and (py < y + r or py > y + h - r):
            if abs(px - cx) <= r and abs(py - cy) <= r:
                return ((px - cx) ** 2 + (py - cy) ** 2) ** 0.5 <= r
    return True


def in_triangle(px, py, tri):
    (ax, ay), (bx, by), (cx, cy) = ((s(x), s(y)) for x, y in tri)
    d1 = (px - bx) * (ay - by) - (ax - bx) * (py - by)
    d2 = (px - cx) * (by - cy) - (bx - cx) * (py - cy)
    d3 = (px - ax) * (cy - ay) - (cx - ax) * (py - ay)
    negative = (d1 < 0) or (d2 < 0) or (d3 < 0)
    positive = (d1 > 0) or (d2 > 0) or (d3 > 0)
    return not (negative and positive)


large = bytearray(B * B * 4)
for y in range(B):
    for x in range(B):
        i = (y * B + x) * 4
        px, py = x + 0.5, y + 0.5
        filled = any(in_rectangle(px, py, rect) for rect in RECTANGLES) or any(
            in_triangle(px, py, tri) for tri in TRIANGLES
        )
        if filled:
            large[i], large[i + 1], large[i + 2], large[i + 3] = COLOR[0], COLOR[1], COLOR[2], 255

# Scale down. Alpha-weighted average for transparency: otherwise transparent
# pixels would muddy the colour at the edges.
SIZE = BOX * 2
small = bytearray(SIZE * SIZE * 4)
step = B // SIZE
area = step * step
for y in range(SIZE):
    for x in range(SIZE):
        alpha = 0
        for dy in range(step):
            base = ((y * step + dy) * B + x * step) * 4
            for dx in range(step):
                alpha += large[base + dx * 4 + 3]
        j = (y * SIZE + x) * 4
        small[j], small[j + 1], small[j + 2] = COLOR
        small[j + 3] = alpha // area

raw = bytearray()
for y in range(SIZE):
    raw.append(0)
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
    "vscode-extension", "resources", "icon.png",
)
open(target, "wb").write(png)
print("  wrote: %s (%dx%d)" % (os.path.basename(target), SIZE, SIZE))
