--!strict
-- Bu place'in kalici kimligi. TEK KAYNAK.
--
-- Neden tek dosya: kimlik hem port taramasinda (ConnectionManager) hem agac
-- gonderiminde (PatchBuilder) lazim. Iki yerde ayri ayri hesaplansaydi biri
-- degistiginde digeri geride kalir ve iki taraf ayni place'i FARKLI sanardi.
-- Servis listesinde tam olarak bu hata yasanmisti.

local RunService = game:GetService("RunService")
local HttpService = game:GetService("HttpService")

local PlaceKimligi = {}

--- Kimligi dondurur.
---
--- Once game.PlaceId denenir. Sebep onemli: ilk surumde kimligi Workspace'e
--- attribute olarak yaziyordum, ama attribute place ile birlikte KAYDEDILIYOR.
--- Kullanici Studio'yu kaydetmeden kapatinca kimlik kayboluyor, place tekrar
--- acildiginda yeni bir kimlik uretiliyor ve Syncix bunu "baska bir place"
--- sanip senkronu durduruyordu. PlaceId ise kaydetmeye bagli degil.
---
--- Attribute yalnizca PlaceId'si olmayan (hic kaydedilmemis, tamamen yerel)
--- place'ler icin yedek olarak kaliyor.
function PlaceKimligi.Al(): string
	local id = game.PlaceId
	if id ~= nil and id ~= 0 then
		return "place:" .. tostring(id)
	end

	local Workspace = game:GetService("Workspace")
	local kimlik = Workspace:GetAttribute("__syncix_place")
	if type(kimlik) ~= "string" or kimlik == "" then
		kimlik = "local:" .. HttpService:GenerateGUID(false)
		-- Kaydedilmemis place'te bu attribute de kalici degil; yalnizca
		-- ayni oturum icinde tutarlilik sagliyor.
		Workspace:SetAttribute("__syncix_place", kimlik)
	end
	return kimlik
end

--- Kullaniciya gosterilecek kisa aciklama.
function PlaceKimligi.Aciklama(): string
	if game.PlaceId ~= 0 then
		return string.format("%s (placeId %d)", game.Name, game.PlaceId)
	end
	return game.Name .. " (unsaved place)"
end

return PlaceKimligi
