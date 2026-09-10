-- Senkron edilen servislerin TEK kaynagi.
--
-- Neden ayri bir dosya: bu list GenericObserver ve PatchBuilder icinde iki kez
-- elle yazilmisti. Birine svc eklenip digerine eklenmediginde object agacta
-- gorunuyor ama degisiklikleri izlenmiyordu — sessiz ve tesisi zor bir failure.
--
-- Sabit UUID'ler: servislerin kimligi core yeniden basladiginda da ayni kalmali,
-- yoksa her acilisita agacin kokleri degisir ve tum subItem agac yeniden yazilir.

local SyncConfig = require(script.Parent.Parent.Core.SyncConfig)

local Services = {}

Services.UUIDS = {
	Workspace           = "00000000-0000-4000-8000-000000000001",
	ReplicatedStorage   = "00000000-0000-4000-8000-000000000002",
	ReplicatedFirst     = "00000000-0000-4000-8000-000000000003",
	ServerStorage       = "00000000-0000-4000-8000-000000000004",
	ServerScriptService = "00000000-0000-4000-8000-000000000005",
	StarterPlayer       = "00000000-0000-4000-8000-000000000006",
	StarterGui          = "00000000-0000-4000-8000-000000000007",
	StarterPack         = "00000000-0000-4000-8000-000000000008",
	Lighting            = "00000000-0000-4000-8000-000000000009",
	SoundService        = "00000000-0000-4000-8000-00000000000a",
	Teams               = "00000000-0000-4000-8000-00000000000b",
	TextChatService     = "00000000-0000-4000-8000-00000000000c",
	LocalizationService = "00000000-0000-4000-8000-00000000000d",
	MaterialService     = "00000000-0000-4000-8000-00000000000e",
}

-- Sirali list: agacin kok siralamasi her acilista ayni olsun.
--
-- Players, Chat ve TestService bilerek disarida. Players calisma aninda dolan
-- bir svc; icerigi yazarin urunu degil, oyuncularin. Chat eski sohbet
-- sistemi, yerini TextChatService aldi. TestService yalnizca test kosmak icin.
local NAMES = {
	"Workspace",
	"ReplicatedStorage",
	"ReplicatedFirst",
	"ServerStorage",
	"ServerScriptService",
	"StarterPlayer",
	"StarterGui",
	"StarterPack",
	"Lighting",
	"SoundService",
	"Teams",
	-- Modern sohbet kurulumu: TextChannel, TextChatCommand, pencere ayarlari.
	"TextChatService",
	-- Ceviri tablolarinin durdugu yer.
	"LocalizationService",
	-- Ozel MaterialVariant tanimlari.
	"MaterialService",
}

--- Senkron edilecek servisleri dondurur.
--- GetService bir svc bu Studio surumunde yoksa failure atiyor; o yuzden her
--- cagri korumali ve eksik svc sessizce atlaniyor.
function Services.List(): { Instance }
	local list = {}
	-- Kullanici syncix.toml'da kendi listesini verdiyse o gecerli.
	local requested = SyncConfig.ServiceList() or NAMES
	for _, ad in ipairs(requested) do
		local ok, svc = pcall(function()
			return game:GetService(ad)
		end)
		if ok and svc then
			table.insert(list, svc)
		end
	end
	return list
end

return Services
