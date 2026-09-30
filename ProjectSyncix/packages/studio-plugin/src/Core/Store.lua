-- Store
-- Two-layer persistent store for plugin settings.
--
-- 1. plugin:SetSetting — works for plugins installed from the Creator Store.
--    When the .rbxm file is dropped straight into the Plugins folder (our distribution form)
--    the plugin has no registered identity, so it is NOT WRITTEN TO DISK; measured.
-- 2. A game attribute — fallback layer. `game` is outside the services the observer watches,
--    so it does not leak into sync; it persists when the place is saved.

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
