# resources

Bu klasordeki gorseller ve neden bu bicimde olduklari.

## icon.svg — VS Code kenar cubugu ikonu

Kasten SADE tutuluyor: tek satir, yorum yok, XML bildirimi yok, dosya
dogrudan `<svg` ile basliyor.

Sebebi denemeyle bulundu. Iki kez gorunmez kaldi:

1. Ilk surum `stroke="currentColor"` kullaniyordu. VS Code bu ikonu tek renge
   indirirken cizgi ozelliklerini tasimiyor, `currentColor` da bir CSS
   baglamindan geldigi ve kenar cubugunda o baglam olmadigi icin cozulemiyor.
   Sonuc: ikonun yeri bos gorunuyor, yalnizca uzerine gelince ipucu cikiyor.

2. Ikinci surum dolgu kullaniyordu ama dosyanin BASINDA uzun bir XML yorumu
   vardi. Yine gorunmedi. Bu yuzden dosyada artik hicbir yorum yok.

Kural: dolgu kullan (cizgi degil), rengi acikca yaz (#C5C5C5 VS Code'un
standart ikon grisi), dosyayi `<svg` ile baslat.

Isaret logoyla ayni: ust ok saga (editore), alt ok Studio'ya.

## logo.svg / logo.png — Marketplace ve README logosu

Ayni isaretin renkli, zeminli surumu. PNG `tools/gen-logo.py` ile uretiliyor;
bu ortamda Pillow yok ve Windows'ta `convert` ImageMagick degil, o yuzden PNG
dogrudan zlib ile yaziliyor ve kenar yumusatma icin 4 kat buyuk cizilip
kuculuyor.

Zemin rengi Studio panelindeki arka planla ayni: eklenti, panel ve magaza
girdisi tek bir palete dayaniyor.

## Studio arac cubugu ikonu

Roblox plugin dugmesine yerel dosya konulamiyor; gorsel Roblox'a asset olarak
yuklenmeli. Yuklenen surum: `rbxassetid://73929349055328`
(bkz. studio-plugin/src/Core/SettingsPanel.lua, IKON).
