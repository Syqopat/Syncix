--!strict
-- Bu place'in kalici kimligi. TEK KAYNAK.
--
-- Neden tek dosya: identity hem port taramasinda (ConnectionManager) hem agac
-- gonderiminde (PatchBuilder) lazim. Iki yerde ayri ayri hesaplansaydi biri
-- degistiginde digeri geride kalir ve iki taraf ayni place'i FARKLI sanardi.
-- Servis listesinde tam olarak bu failure yasanmisti.

local RunService = game:GetService("RunService")
local HttpService = game:GetService("HttpService")

local PlaceIdentity = {}

--- Kimligi dondurur.
---
--- Once game.PlaceId denenir. Sebep onemli: first surumde kimligi Workspace'e
--- attribute olarak yaziyordum, ama attribute place ile birlikte KAYDEDILIYOR.
--- Kullanici Studio'yu kaydetmeden kapatinca identity kayboluyor, place tekrar
--- acildiginda fresh bir identity uretiliyor ve Syncix bunu "baska bir place"
--- sanip senkronu durduruyordu. PlaceId ise kaydetmeye bagli degil.
---
--- Attribute yalnizca PlaceId'si olmayan (hic kaydedilmemis, tamamen yerel)
--- place'ler icin yedek olarak kaliyor.
function PlaceIdentity.Resolve(): string
	local id = game.PlaceId
	if id ~= nil and id ~= 0 then
		return "place:" .. tostring(id)
	end

	local Workspace = game:GetService("Workspace")
	local identity = Workspace:GetAttribute("__syncix_place")
	if type(identity) ~= "string" or identity == "" then
		identity = "local:" .. HttpService:GenerateGUID(false)
		-- Kaydedilmemis place'te bu attribute de kalici degil; yalnizca
		-- ayni oturum icinde tutarlilik sagliyor.
		Workspace:SetAttribute("__syncix_place", identity)
	end
	return identity
end

--- Kullaniciya gosterilecek kisa aciklama.
function PlaceIdentity.Describe(): string
	if game.PlaceId ~= 0 then
		return string.format("%s (placeId %d)", game.Name, game.PlaceId)
	end
	return game.Name .. " (unsaved place)"
end

return PlaceIdentity
