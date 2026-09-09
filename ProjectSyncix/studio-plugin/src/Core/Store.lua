-- Store
-- Eklenti ayarları için iki katmanlı kalıcı depo.
--
-- 1. plugin:SetSetting — Creator Store'dan kurulmuş eklentilerde çalışır.
--    .rbxm dosyası doğrudan Plugins klasörüne bırakıldığında (bizim dağıtım biçimimiz)
--    eklentinin kayıtlı kimliği olmadığı için diske YAZILMIYOR; ölçüldü.
-- 2. game niteliği — yedek katman. `game` gözlemcinin izlediği servislerin dışında
--    olduğu için senkrona sızmaz; place kaydedildiğinde kalıcı olur.

local HttpService = game:GetService("HttpService")

local Store = {}

local GAME_ATTR = "__syncix_ayarlar"

local function placeOku()
	local ok, ham = pcall(function()
		return game:GetAttribute(GAME_ATTR)
	end)
	if not ok or type(ham) ~= "string" or ham == "" then
		return {}
	end
	local okDecode, tablo = pcall(function()
		return HttpService:JSONDecode(ham)
	end)
	if okDecode and type(tablo) == "table" then
		return tablo
	end
	return {}
end

local function placeYaz(tablo)
	pcall(function()
		game:SetAttribute(GAME_ATTR, HttpService:JSONEncode(tablo))
	end)
end

function Store.Get(pluginRef, anahtar, varsayilan)
	if pluginRef then
		local ok, deger = pcall(function()
			return pluginRef:GetSetting(anahtar)
		end)
		if ok and deger ~= nil then
			return deger
		end
	end

	local tablo = placeOku()
	if tablo[anahtar] ~= nil then
		return tablo[anahtar]
	end
	return varsayilan
end

function Store.Set(pluginRef, anahtar, deger)
	if pluginRef then
		pcall(function()
			pluginRef:SetSetting(anahtar, deger)
		end)
	end
	local tablo = placeOku()
	tablo[anahtar] = deger
	placeYaz(tablo)
end

return Store
