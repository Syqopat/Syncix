# Marketplace ikonunu (128x128 PNG) uretir.
#
# Neden elle: bu ortamda Pillow yok ve `convert` komutu Windows'ta ImageMagick
# degil, dosya sistemi donusturucusu — calistirmak tehlikeli. PNG yazmak zaten
# zlib + birkac satir baslik; disaridan araca gerek yok.
#
# Cizim logo.svg ile AYNI geometriyi kullaniyor. Kenar yumusatma icin 4 kat
# buyuk cizilip kutu ortalamasiyla kuculuyor.

import zlib, struct, os

OLCEK = 4
BOY = 128
B = BOY * OLCEK

ZEMIN = (0x1E, 0x21, 0x28)
BEYAZ = (0xFF, 0xFF, 0xFF)
MAVI = (0x4C, 0x8D, 0xF5)

KOSE = 28 * OLCEK
KALINLIK = 9 * OLCEK / 2.0


def nokta_cizgi_uzakligi(px, py, x1, y1, x2, y2):
    """Noktanin bir DOGRU PARCASINA uzakligi. Yuvarlak uc/kose bedava geliyor:
    parcaya uzaklik esigi, ucu yarim daire yapar."""
    dx, dy = x2 - x1, y2 - y1
    uzunluk2 = dx * dx + dy * dy
    if uzunluk2 == 0:
        t = 0.0
    else:
        t = ((px - x1) * dx + (py - y1) * dy) / uzunluk2
        t = 0.0 if t < 0 else (1.0 if t > 1 else t)
    ux, uy = x1 + t * dx - px, y1 + t * dy - py
    return (ux * ux + uy * uy) ** 0.5


# logo.svg ile ayni koordinatlar, OLCEK ile buyutulmus.
def s(v):
    return v * OLCEK


BEYAZ_PARCALAR = [
    (s(30), s(52), s(74), s(52)),
    (s(64), s(40), s(76), s(52)),
    (s(76), s(52), s(64), s(64)),
]
MAVI_PARCALAR = [
    (s(98), s(80), s(54), s(80)),
    (s(64), s(68), s(52), s(80)),
    (s(52), s(80), s(64), s(92)),
]


def kose_disinda(x, y):
    """Yuvarlatilmis dikdortgenin disinda mi?"""
    for cx, cy in ((KOSE, KOSE), (B - KOSE, KOSE), (KOSE, B - KOSE), (B - KOSE, B - KOSE)):
        if (x < KOSE or x > B - KOSE) and (y < KOSE or y > B - KOSE):
            if abs(x - cx) <= KOSE and abs(y - cy) <= KOSE:
                if ((x - cx) ** 2 + (y - cy) ** 2) ** 0.5 > KOSE:
                    return True
    return False


# Buyuk tuvali ciz
buyuk = bytearray(B * B * 4)
for y in range(B):
    satir = y * B * 4
    for x in range(B):
        i = satir + x * 4
        if kose_disinda(x + 0.5, y + 0.5):
            continue  # seffaf kalir
        r, g, b = ZEMIN
        for parcalar, renk in ((BEYAZ_PARCALAR, BEYAZ), (MAVI_PARCALAR, MAVI)):
            for (x1, y1, x2, y2) in parcalar:
                if nokta_cizgi_uzakligi(x + 0.5, y + 0.5, x1, y1, x2, y2) <= KALINLIK:
                    r, g, b = renk
                    break
        buyuk[i] = r
        buyuk[i + 1] = g
        buyuk[i + 2] = b
        buyuk[i + 3] = 255

# Kutu ortalamasiyla kucult (kenar yumusatma buradan geliyor)
kucuk = bytearray(BOY * BOY * 4)
alan = OLCEK * OLCEK
for y in range(BOY):
    for x in range(BOY):
        tr = tg = tb = ta = 0
        for dy in range(OLCEK):
            taban = ((y * OLCEK + dy) * B + x * OLCEK) * 4
            for dx in range(OLCEK):
                i = taban + dx * 4
                a = buyuk[i + 3]
                tr += buyuk[i] * a
                tg += buyuk[i + 1] * a
                tb += buyuk[i + 2] * a
                ta += a
        j = (y * BOY + x) * 4
        if ta:
            kucuk[j] = tr // ta
            kucuk[j + 1] = tg // ta
            kucuk[j + 2] = tb // ta
        kucuk[j + 3] = ta // alan

# PNG yaz
ham = bytearray()
for y in range(BOY):
    ham.append(0)  # filtre yok
    ham += kucuk[y * BOY * 4:(y + 1) * BOY * 4]


def parca(tur, veri):
    g = tur + veri
    return struct.pack(">I", len(veri)) + g + struct.pack(">I", zlib.crc32(g) & 0xFFFFFFFF)


png = (
    b"\x89PNG\r\n\x1a\n"
    + parca(b"IHDR", struct.pack(">IIBBBBB", BOY, BOY, 8, 6, 0, 0, 0))
    + parca(b"IDAT", zlib.compress(bytes(ham), 9))
    + parca(b"IEND", b"")
)

hedef = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "vscode-extension", "resources", "logo.png",
)
open(hedef, "wb").write(png)
print("  yazildi: %s (%d bayt)" % (os.path.basename(hedef), len(png)))
