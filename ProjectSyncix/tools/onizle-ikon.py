# icon.svg'nin geometrisini PNG'ye cizer.
#
# Neden: kenar cubugu ikonu bir kez "seffaf goruniyor" diye geri geldi ve
# sebebini ancak kurup bakinca anladik. Bu betik, kurmadan once seklin
# gercekten dolu ve dogru olup olmadigini gostermek icin.
#
# icon.svg ile AYNI koordinatlar; degistirirsen ikisini birden guncelle.

import zlib, struct, os

OLCEK, KUTU = 6, 24
B = KUTU * OLCEK
ZEMIN = (0x25, 0x25, 0x26)   # VS Code kenar cubugu grisi
RENK = (0xC5, 0xC5, 0xC5)


def s(v):
    return v * OLCEK


# Yuvarlatilmis dikdortgen: (x, y, genislik, yukseklik, yaricap)
DIKDORTGENLER = [(3, 7.6, 10, 2.8, 1.4), (11, 13.6, 10, 2.8, 1.4)]
# Ucgenler
UCGENLER = [
    ((12.2, 4.8), (18.6, 9.0), (12.2, 13.2)),
    ((11.8, 10.8), (5.4, 15.0), (11.8, 19.2)),
]


def dikdortgende(px, py, d):
    x, y, g, h, r = (s(v) for v in d)
    if not (x <= px <= x + g and y <= py <= y + h):
        return False
    for cx, cy in ((x + r, y + r), (x + g - r, y + r), (x + r, y + h - r), (x + g - r, y + h - r)):
        if (px < x + r or px > x + g - r) and (py < y + r or py > y + h - r):
            if abs(px - cx) <= r and abs(py - cy) <= r:
                return ((px - cx) ** 2 + (py - cy) ** 2) ** 0.5 <= r
    return True


def ucgende(px, py, u):
    (ax, ay), (bx, by), (cx, cy) = ((s(x), s(y)) for x, y in u)
    d1 = (px - bx) * (ay - by) - (ax - bx) * (py - by)
    d2 = (px - cx) * (by - cy) - (bx - cx) * (py - cy)
    d3 = (px - ax) * (cy - ay) - (cx - ax) * (py - ay)
    negatif = (d1 < 0) or (d2 < 0) or (d3 < 0)
    pozitif = (d1 > 0) or (d2 > 0) or (d3 > 0)
    return not (negatif and pozitif)


buyuk = bytearray(B * B * 3)
for y in range(B):
    for x in range(B):
        i = (y * B + x) * 3
        px, py = x + 0.5, y + 0.5
        dolu = any(dikdortgende(px, py, d) for d in DIKDORTGENLER) or any(
            ucgende(px, py, u) for u in UCGENLER
        )
        r, g, b = RENK if dolu else ZEMIN
        buyuk[i], buyuk[i + 1], buyuk[i + 2] = r, g, b

# Kucult (kenar yumusatma)
BOY = KUTU * 4
kucuk = bytearray(BOY * BOY * 3)
adim = B // BOY
alan = adim * adim
for y in range(BOY):
    for x in range(BOY):
        tr = tg = tb = 0
        for dy in range(adim):
            taban = ((y * adim + dy) * B + x * adim) * 3
            for dx in range(adim):
                i = taban + dx * 3
                tr += buyuk[i]; tg += buyuk[i + 1]; tb += buyuk[i + 2]
        j = (y * BOY + x) * 3
        kucuk[j], kucuk[j + 1], kucuk[j + 2] = tr // alan, tg // alan, tb // alan

ham = bytearray()
for y in range(BOY):
    ham.append(0)
    ham += kucuk[y * BOY * 3:(y + 1) * BOY * 3]


def parca(tur, veri):
    g = tur + veri
    return struct.pack(">I", len(veri)) + g + struct.pack(">I", zlib.crc32(g) & 0xFFFFFFFF)


png = (
    b"\x89PNG\r\n\x1a\n"
    + parca(b"IHDR", struct.pack(">IIBBBBB", BOY, BOY, 8, 2, 0, 0, 0))
    + parca(b"IDAT", zlib.compress(bytes(ham), 9))
    + parca(b"IEND", b"")
)

hedef = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "ikon-onizleme.png")
open(hedef, "wb").write(png)
print("  yazildi: %s (%dx%d)" % (os.path.basename(hedef), BOY, BOY))
