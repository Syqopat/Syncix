-- Eklentinin calisma ayarlari.
--
-- Tek dogruluk kaynagi syncix.toml. Core bu ayarlari /health cevabinda
-- gonderiyor, eklenti baglanirken okuyup buraya yaziyor.
--
-- Neden eklenti kendi ayarini tutmuyor: iki ayri ayar seti olsaydi (biri
-- panelde, biri dosyada) hangisinin gecerli oldugu belirsizlesirdi. Kullanici
-- dosyada "disk_to_studio" yazip Studio'da hala iki yonlu behavior gorurse
-- bunun sebebini bulmasi cok zor olur. Bu yuzden dosya kazanir.
--
-- Tek istisna Pause/Resume: o bir ayar degil, anlik bir durum. Panelde kaliyor.

local SyncConfig = {}

-- Baglanti kurulana kadar gecerli olan varsayilanlar. Core baglandigi anda
-- Apply() bunlarin uzerine yaziyor.
local current = {
	mode = "two_way",
	play_mode = "queue",
	ask_permission = false,
	undo = true,
	services = {},
	ignore_classes = {},
	ignore_properties = {},
}

-- Liste yerine lookupSet: her property degisikliginde list taramak pahali olurdu.
local ignoredClasses = {}
local ignoredProperties = {}

local function toSet(list: { string }?): { [string]: boolean }
	local lookupSet = {}
	for _, ad in ipairs(list or {}) do
		lookupSet[ad] = true
	end
	return lookupSet
end

--- Core'dan incoming ayarlari uygular. Alan eksikse current datum korunur:
--- eski bir core'a baglanildiginda ayarlarin sifirlanmasi yanlis olur.
function SyncConfig.Apply(incoming: any)
	if type(incoming) ~= "table" then
		return
	end
	for keyName, datum in pairs(incoming) do
		if current[keyName] ~= nil then
			current[keyName] = datum
		end
	end
	ignoredClasses = toSet(current.ignore_classes)
	ignoredProperties = toSet(current.ignore_properties)
end

function SyncConfig.Mode(): string
	return current.mode
end

--- Studio'da olan bir degisiklik core'a gonderilsin mi?
--- disk_to_studio modunda Studio yalnizca alici; gozlemci hic konusmamali.
function SyncConfig.SendFromStudio(): boolean
	return current.mode == "two_way" or current.mode == "studio_to_disk"
end

--- Core'dan incoming bir degisiklik Studio'ya uygulansin mi?
function SyncConfig.ApplyToStudio(): boolean
	return current.mode == "two_way" or current.mode == "disk_to_studio"
end

function SyncConfig.PlayBehavior(): string
	return current.play_mode
end

function SyncConfig.UndoEnabled(): boolean
	return current.undo ~= false
end

function SyncConfig.AskPermission(): boolean
	return current.ask_permission == true
end

--- Bos list "varsayilani kullan" demek; kullanicinin hicbir svc
--- istemedigi anlamina gelmez. Bos birakmak en sik durum oldugu icin
--- bunu "hicbiri" saymak, ayari acan herkesin senkronu kirmasi olurdu.
function SyncConfig.ServiceList(): { string }?
	if #current.services == 0 then
		return nil
	end
	return current.services
end

function SyncConfig.ClassAllowed(cls: string): boolean
	return not ignoredClasses[cls]
end

function SyncConfig.PropertyAllowed(ad: string): boolean
	return not ignoredProperties[ad]
end

return SyncConfig
