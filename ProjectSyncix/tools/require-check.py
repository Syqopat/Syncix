# Eklenti kaynaklarinda "kullaniliyor ama tanimli degil" durumlarini bulur.
#
# Neden gerekli: sozdizimi denetimi bunu YAKALAMIYOR. Dosya gecerli Lua olarak
# derleniyor; hata ancak Studio'da o satir CALISINCA ortaya cikiyor. Bir kod
# blogunu keserken once Store/Approval require satirlarini, sonra da
# PLUGIN_VERSION gibi sabitleri yanlislikla sildim. Ikisi de eklentinin
# Studio'da yuklenirken patlamasina yol acti ve sebep yalnizca Output'ta
# gorunuyordu.
#
# Iki denetim:
#   1. Eklentinin KENDI modul adlari (dosya adlari) kullaniliyorsa require
#      edilmis olmali.
#   2. BUYUK_HARFLI sabitler kullaniliyorsa local olarak tanimli olmali.
#
# Yanlis alarmi onlemek icin yorumlar ve metin sabitleri once siliniyor.

import io
import os
import re
import sys
import glob

KOK = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "studio-plugin",
    "src",
)


def kodu_ayikla(ham):
    """Yorumlari ve metin sabitlerini siler; geriye yalnizca kod kalir."""
    satirlar = [re.sub(r"--.*$", "", x) for x in ham.splitlines()]
    kod = '\n'.join(satirlar)
    kod = re.sub('"[^"\\n]*"', " ", kod)
    kod = re.sub("'[^'\\n]*'", " ", kod)
    return kod


def denetle():
    dosyalar = sorted(glob.glob(os.path.join(KOK, "**", "*.lua"), recursive=True))
    if not dosyalar:
        print("UYARI: hic .lua dosyasi bulunamadi: " + KOK)
        return 1

    moduller = {
        os.path.splitext(os.path.basename(f))[0]
        for f in dosyalar
        if not os.path.basename(f).startswith("init.")
    }

    sorun = 0
    for yol in dosyalar:
        kod = kodu_ayikla(io.open(yol, encoding="utf-8").read())
        tanimli = set(re.findall(r"local[ ]+([A-Za-z_]" + '\\w' + r"*)", kod))
        gosterim = os.path.relpath(yol, KOK).replace(os.sep, "/")

        # 1. Eksik require
        for m in sorted(moduller - tanimli):
            if re.search(r"(?<![A-Za-z_.])" + m + r"[ ]*[.:][ ]*[A-Za-z_]", kod):
                print("  %-44s %s kullaniliyor ama require edilmemis" % (gosterim, m))
                sorun += 1

        # 2. Tanimsiz sabit
        # Nokta/iki nokta sonrasi ALAN erisimidir, sabit degil (Services.UUIDS).
        sabitler = set(re.findall(r"(?<![A-Za-z_.:])([A-Z][A-Z_]{2,})(?![A-Za-z_])", kod))
        for sabit in sorted(sabitler - tanimli):
            print("  %-44s %s kullaniliyor ama tanimli degil" % (gosterim, sabit))
            sorun += 1

    if sorun:
        print("SONUC: %d sorun" % sorun)
        return 1
    print("SONUC: temiz (%d dosya)" % len(dosyalar))
    return 0


if __name__ == "__main__":
    sys.exit(denetle())
