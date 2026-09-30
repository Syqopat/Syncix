"""A tiny RGB canvas that saves PNG with the standard library only (zlib + struct).

Enough for animation previews: thick lines, filled circles, rectangles and a small
built-in bitmap font for labels. No numpy, no Pillow: it must run wherever Python does.
"""
import struct
import zlib

# 5x7 glyphs, one string of 7 rows per character; '#' is ink.
_FONT = {
    "0": ".###.#...##..###.#.###..##...#.###.", "1": "..#...##....#....#....#....#...###.",
    "2": ".###.#...#....#...#...#...#....#####", "3": "#####...#...#.....#....##...#.###.",
    "4": "...#...##..#.#.#..#.#####...#....#.", "5": "######....####.....#....##...#.###.",
    "6": "..##..#...#....####.#...##...#.###.", "7": "#####....#...#...#...#....#....#...",
    "8": ".###.#...##...#.###.#...##...#.###.", "9": ".###.#...##...#.####....#...#..##..",
    ".": "..........................##...##..", ":": ".....##...##.........##...##.......",
    "-": "...............###................", "s": "..........###.#.....###.....#####..",
    " ": "...................................", "/": "....#....#...#...#...#....#....#...",
    "+": ".......#....#..#####..#....#.......", "%": "##...##..#...#...#...#...#..##...##",
}
_LETTERS = {
    "A": ".###.#...##...#######...##...##...#", "B": "####.#...##...#####.#...##...#####.",
    "C": ".###.#...##....#....#....#...#.###.", "D": "####.#...##...##...##...##...#####.",
    "E": "######....#....####.#....#....#####", "F": "######....#....####.#....#....#....",
    "G": ".###.#...##....#.####...##...#.####", "H": "#...##...##...#######...##...##...#",
    "I": ".###...#....#....#....#....#...###.", "J": "..###...#....#....#....##..#..##..",
    "K": "#...##..#.#.#..##...#.#..#..#.#...#", "L": "#....#....#....#....#....#....#####",
    "M": "#...###.###.#.##...##...##...##...#", "N": "#...###..##.#.##..###...##...##...#",
    "O": ".###.#...##...##...##...##...#.###.", "P": "####.#...##...#####.#....#....#....",
    "Q": ".###.#...##...##...##.#.##..#..##.#", "R": "####.#...##...#####.#.#..#..#.#...#",
    "S": ".#####....#.....###.....#....#####.", "T": "#####..#....#....#....#....#....#..",
    "U": "#...##...##...##...##...##...#.###.", "V": "#...##...##...##...##...#.#.#...#..",
    "W": "#...##...##...##.#.##.#.##.#.#.#.#.", "X": "#...##...#.#.#...#...#.#.#...##...#",
    "Y": "#...##...#.#.#...#....#....#....#..", "Z": "#####....#...#...#...#...#....#####",
}
_FONT.update(_LETTERS)


class Canvas:
    def __init__(self, width, height, background=(255, 255, 255)):
        self.w, self.h = width, height
        self.px = bytearray(bytes(background) * (width * height))

    def set(self, x, y, color):
        x, y = int(x), int(y)
        if 0 <= x < self.w and 0 <= y < self.h:
            i = (y * self.w + x) * 3
            self.px[i:i + 3] = bytes(color)

    def disc(self, cx, cy, radius, color):
        r = max(0, int(radius))
        for dy in range(-r, r + 1):
            for dx in range(-r, r + 1):
                if dx * dx + dy * dy <= r * r + r:
                    self.set(cx + dx, cy + dy, color)

    def line(self, x0, y0, x1, y1, color, width=1):
        """A thick line: discs stamped along a Bresenham path."""
        x0, y0, x1, y1 = int(round(x0)), int(round(y0)), int(round(x1)), int(round(y1))
        dx, dy = abs(x1 - x0), -abs(y1 - y0)
        sx, sy = (1 if x0 < x1 else -1), (1 if y0 < y1 else -1)
        err = dx + dy
        half = max(0, (width - 1) // 2)
        while True:
            if half:
                self.disc(x0, y0, half, color)
            else:
                self.set(x0, y0, color)
            if x0 == x1 and y0 == y1:
                break
            e2 = 2 * err
            if e2 >= dy:
                err += dy
                x0 += sx
            if e2 <= dx:
                err += dx
                y0 += sy

    def polygon(self, points, color, outline=None):
        """A filled convex polygon (scanline), with an optional outline."""
        if len(points) < 3:
            return
        ys = [p[1] for p in points]
        for y in range(int(min(ys)), int(max(ys)) + 1):
            yc = y + 0.5
            xs = []
            for (x0, y0), (x1, y1) in zip(points, points[1:] + points[:1]):
                if (y0 <= yc < y1) or (y1 <= yc < y0):
                    xs.append(x0 + (yc - y0) * (x1 - x0) / (y1 - y0))
            xs.sort()
            for a, b in zip(xs[0::2], xs[1::2]):
                for x in range(int(round(a)), int(round(b)) + 1):
                    self.set(x, y, color)
        if outline:
            for (x0, y0), (x1, y1) in zip(points, points[1:] + points[:1]):
                self.line(x0, y0, x1, y1, outline)

    def rect(self, x0, y0, x1, y1, color, fill=False):
        if fill:
            for y in range(int(min(y0, y1)), int(max(y0, y1)) + 1):
                for x in range(int(min(x0, x1)), int(max(x0, x1)) + 1):
                    self.set(x, y, color)
        else:
            for a, b, c, d in ((x0, y0, x1, y0), (x1, y0, x1, y1), (x1, y1, x0, y1), (x0, y1, x0, y0)):
                self.line(a, b, c, d, color)

    def text(self, x, y, s, color=(0, 0, 0), scale=2):
        """Upper-case letters, digits and . : - s / + % ; unknown characters are skipped."""
        for ch in str(s):
            glyph = _FONT.get(ch) or _FONT.get(ch.upper())
            if glyph:
                glyph = glyph.ljust(35, ".")
                for row in range(7):
                    for col in range(5):
                        if glyph[row * 5 + col] == "#":
                            for sy in range(scale):
                                for sx in range(scale):
                                    self.set(x + col * scale + sx, y + row * scale + sy, color)
            x += 6 * scale

    def save(self, path):
        raw = b"".join(b"\x00" + bytes(self.px[y * self.w * 3:(y + 1) * self.w * 3]) for y in range(self.h))

        def chunk(tag, data):
            body = tag + data
            return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

        with open(path, "wb") as fh:
            fh.write(b"\x89PNG\r\n\x1a\n")
            fh.write(chunk(b"IHDR", struct.pack(">IIBBBBB", self.w, self.h, 8, 2, 0, 0, 0)))
            fh.write(chunk(b"IDAT", zlib.compress(raw, 6)))
            fh.write(chunk(b"IEND", b""))
