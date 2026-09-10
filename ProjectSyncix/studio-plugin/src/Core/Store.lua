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

local GAME_ATTR = "__syncix_settings"

local function readPlace()
	local ok, raw = pcall(function()
		return game:GetAttribute(GAME_ATTR)
	end)
	if not ok or type(raw) ~= "string" or raw == "" then
		return {}
	end
	local okDecode, tbl = pcall(function()
		return HttpService:JSONDecode(raw)
	end)
	if okDecode and type(tbl) == "table" then
		return tbl
	end
	return {}
end

local function writePlace(tbl)
	pcall(function()
		game:SetAttribute(GAME_ATTR, HttpService:JSONEncode(tbl))
	end)
end

function Store.Get(pluginRef, keyName, defaultValue)
	if pluginRef then
		local ok, datum = pcall(function()
			return pluginRef:GetSetting(keyName)
		end)
		if ok and datum ~= nil then
			return datum
		end
	end

	local tbl = readPlace()
	if tbl[keyName] ~= nil then
		return tbl[keyName]
	end
	return defaultValue
end

function Store.Set(pluginRef, keyName, datum)
	if pluginRef then
		pcall(function()
			pluginRef:SetSetting(keyName, datum)
		end)
	end
	local tbl = readPlace()
	tbl[keyName] = datum
	writePlace(tbl)
end

return Store
