#!/bin/sh
# Luau sözdizimi denetimi.
#
# luau CLI'da "yalnızca derle" bayrağı yok, dosya çalıştırılıyor. Roblox API'si
# (game, Instance, plugin) burada bulunmadığı için her dosya çalışma zamanında
# hata verir; bu BEKLENEN durumdur ve yok sayılır.
#
# Ayırt etme kuralı: luau her iki hatayı da "dosya:satır: mesaj" + "stacktrace:"
# biçiminde basar. Fark şu: DERLEME (sözdizimi) hatasında stacktrace BOŞtur,
# çünkü hiç kod çalışmamıştır. Çalışma zamanı hatasında en az bir çerçeve vardır.
#
# Kullanım:  sh tools/luau-check.sh dosya1.lua dosya2.lua ...
#            sh tools/luau-check.sh $(find studio-plugin/src -name "*.lua")

HATA=0

for f in "$@"; do
	CIKTI=$(luau "$f" 2>&1)

	if [ -z "$CIKTI" ]; then
		echo "OK                $f"
		continue
	fi

	# "stacktrace:" satırından SONRAKİ boş olmayan satır sayısı
	CERCEVE=$(echo "$CIKTI" | sed -n '/^stacktrace:/,$p' | tail -n +2 | grep -c '[^[:space:]]')

	if [ "$CERCEVE" -eq 0 ]; then
		echo "SOZDIZIMI HATASI  $f"
		echo "$CIKTI" | head -2 | sed 's/^/    /'
		HATA=1
	else
		echo "OK                $f"
	fi
done

if [ "$HATA" -ne 0 ]; then
	echo "SONUC: sozdizimi hatasi var"
else
	echo "SONUC: tum dosyalar temiz"
fi

exit $HATA
