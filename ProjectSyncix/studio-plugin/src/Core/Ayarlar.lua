-- Eklentinin calisma ayarlari.
--
-- Tek dogruluk kaynagi syncix.toml. Core bu ayarlari /health cevabinda
-- gonderiyor, eklenti baglanirken okuyup buraya yaziyor.
--
-- Neden eklenti kendi ayarini tutmuyor: iki ayri ayar seti olsaydi (biri
-- panelde, biri dosyada) hangisinin gecerli oldugu belirsizlesirdi. Kullanici
-- dosyada "disk_to_studio" yazip Studio'da hala iki yonlu davranis gorurse
-- bunun sebebini bulmasi cok zor olur. Bu yuzden dosya kazanir.
--
-- Tek istisna Pause/Resume: o bir ayar degil, anlik bir durum. Panelde kaliyor.

local Ayarlar = {}

-- Baglanti kurulana kadar gecerli olan varsayilanlar. Core baglandigi anda
-- Uygula() bunlarin uzerine yaziyor.
local mevcut = {
	mode = "two_way",
	play_mode = "queue",
	ask_permission = false,
	undo = true,
	services = {},
	ignore_classes = {},
	ignore_properties = {},
}

-- Liste yerine kume: her property degisikliginde liste taramak pahali olurdu.
local sinifDisla = {}
local propertyDisla = {}

local function kumeYap(liste: { string }?): { [string]: boolean }
	local kume = {}
	for _, ad in ipairs(liste or {}) do
		kume[ad] = true
	end
	return kume
end

--- Core'dan gelen ayarlari uygular. Alan eksikse mevcut deger korunur:
--- eski bir core'a baglanildiginda ayarlarin sifirlanmasi yanlis olur.
function Ayarlar.Uygula(gelen: any)
	if type(gelen) ~= "table" then
		return
	end
	for anahtar, deger in pairs(gelen) do
		if mevcut[anahtar] ~= nil then
			mevcut[anahtar] = deger
		end
	end
	sinifDisla = kumeYap(mevcut.ignore_classes)
	propertyDisla = kumeYap(mevcut.ignore_properties)
end

function Ayarlar.Mod(): string
	return mevcut.mode
end

--- Studio'da olan bir degisiklik core'a gonderilsin mi?
--- disk_to_studio modunda Studio yalnizca alici; gozlemci hic konusmamali.
function Ayarlar.StudiodanGonder(): boolean
	return mevcut.mode == "two_way" or mevcut.mode == "studio_to_disk"
end

--- Core'dan gelen bir degisiklik Studio'ya uygulansin mi?
function Ayarlar.StudiyaUygula(): boolean
	return mevcut.mode == "two_way" or mevcut.mode == "disk_to_studio"
end

function Ayarlar.PlayDavranisi(): string
	return mevcut.play_mode
end

function Ayarlar.GeriAlAcik(): boolean
	return mevcut.undo ~= false
end

function Ayarlar.IzinSor(): boolean
	return mevcut.ask_permission == true
end

--- Bos liste "varsayilani kullan" demek; kullanicinin hicbir servis
--- istemedigi anlamina gelmez. Bos birakmak en sik durum oldugu icin
--- bunu "hicbiri" saymak, ayari acan herkesin senkronu kirmasi olurdu.
function Ayarlar.Servisler(): { string }?
	if #mevcut.services == 0 then
		return nil
	end
	return mevcut.services
end

function Ayarlar.SinifIzinli(sinif: string): boolean
	return not sinifDisla[sinif]
end

function Ayarlar.PropertyIzinli(ad: string): boolean
	return not propertyDisla[ad]
end

return Ayarlar
