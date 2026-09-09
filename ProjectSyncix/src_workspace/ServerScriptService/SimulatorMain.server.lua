--[[
	COIN SIMULATOR - Ana Sunucu Scripti
	Syncix ile editorden yazildi.

	Oyun dongusu:
	  1. Bolgelerde coin topla  -> sirt cantana girer
	  2. Canta dolunca Satis Alani'na git -> coinler paraya donusur
	  3. Para ile yukseltme al  -> daha buyuk canta / daha hizli toplama
	  4. Bolge 2'yi ac (500 para) -> 5 kat degerli coinler
--]]

local Players = game:GetService("Players")
local RunService = game:GetService("RunService")

local simulator = workspace:WaitForChild("Simulator")
local bolgeler = simulator:WaitForChild("Bolgeler")
local yukseltmeler = simulator:WaitForChild("Yukseltmeler")
local satisAlani = simulator:WaitForChild("SatisAlani")

-- ============ AYARLAR ============
local AYARLAR = {
	baslangicCanta = 10,
	cantaArtis = 10,
	cantaFiyatCarpan = 1.6,
	cantaBaslangicFiyat = 50,

	baslangicHiz = 1,
	hizArtis = 1,
	hizFiyatCarpan = 2.0,
	hizBaslangicFiyat = 100,

	bolge2Fiyat = 500,

	bolgeler = {
		{ zeminAdi = "Bolge1Zemin", coinSayisi = 25, deger = 1, renk = Color3.fromRGB(255, 209, 0), kilit = false },
		{ zeminAdi = "Bolge2Zemin", coinSayisi = 25, deger = 5, renk = Color3.fromRGB(120, 220, 255), kilit = true },
	},
}

-- ============ ALAN ETIKETLERI ============
-- Her pad'in ustunde ne ise yaradigini yazan tabela olusturur.
local function etiketOlustur(parca, baslik, altYazi, renk)
	if not parca then return nil end

	local mevcut = parca:FindFirstChild("SyncixEtiket")
	if mevcut then mevcut:Destroy() end

	local bb = Instance.new("BillboardGui")
	bb.Name = "SyncixEtiket"
	bb.Size = UDim2.new(0, 220, 0, 70)
	bb.StudsOffset = Vector3.new(0, 5, 0)
	bb.AlwaysOnTop = true
	bb.Parent = parca

	local ust = Instance.new("TextLabel")
	ust.Size = UDim2.new(1, 0, 0.55, 0)
	ust.BackgroundTransparency = 1
	ust.TextColor3 = renk
	ust.TextStrokeTransparency = 0.3
	ust.TextScaled = true
	ust.Font = Enum.Font.GothamBold
	ust.Text = baslik
	ust.Parent = bb

	local alt = Instance.new("TextLabel")
	alt.Name = "AltYazi"
	alt.Size = UDim2.new(1, 0, 0.45, 0)
	alt.Position = UDim2.new(0, 0, 0.55, 0)
	alt.BackgroundTransparency = 1
	alt.TextColor3 = Color3.fromRGB(235, 235, 235)
	alt.TextStrokeTransparency = 0.4
	alt.TextScaled = true
	alt.Font = Enum.Font.Gotham
	alt.Text = altYazi
	alt.Parent = bb

	return alt
end

-- ============ OYUNCU VERISI ============
local oyuncuVeri = {}

local function veriAl(player)
	return oyuncuVeri[player.UserId]
end

Players.PlayerAdded:Connect(function(player)
	oyuncuVeri[player.UserId] = {
		cantaKapasite = AYARLAR.baslangicCanta,
		toplamaHizi = AYARLAR.baslangicHiz,
		cantaFiyat = AYARLAR.cantaBaslangicFiyat,
		hizFiyat = AYARLAR.hizBaslangicFiyat,
		bolge2Acik = false,
		sonToplama = 0,
	}

	local stats = Instance.new("Folder")
	stats.Name = "leaderstats"
	stats.Parent = player

	local para = Instance.new("IntValue")
	para.Name = "Para"
	para.Value = 0
	para.Parent = stats

	local canta = Instance.new("IntValue")
	canta.Name = "Canta"
	canta.Value = 0
	canta.Parent = stats
end)

Players.PlayerRemoving:Connect(function(player)
	oyuncuVeri[player.UserId] = nil
end)

-- ============ COIN SISTEMI ============
local function coinOlustur(bolgeKlasoru, zemin, ayar)
	local coin = Instance.new("Part")
	coin.Name = "Coin"
	-- Roblox'ta Cylinder'in ekseni X'tir: kalinlik X'te, cap Y/Z'de olmali.
	-- (2,2,0.4) verilirse eliptik bir varil cikiyor, madeni para gorunumu icin (0.4,2,2) gerekir.
	coin.Size = Vector3.new(0.4, 2, 2)
	coin.Shape = Enum.PartType.Cylinder
	coin.Material = Enum.Material.Neon
	coin.Color = ayar.renk
	coin.Anchored = true
	coin.CanCollide = false

	-- Zemin uzerinde rastgele konum
	local alan = zemin.Size.X / 2 - 4
	local x = zemin.Position.X + math.random(-alan, alan)
	local z = zemin.Position.Z + math.random(-alan, alan)
	coin.CFrame = CFrame.new(x, zemin.Position.Y + 3, z)

	local isik = Instance.new("PointLight")
	isik.Brightness = 3
	isik.Range = 8
	isik.Color = ayar.renk
	isik.Parent = coin

	coin:SetAttribute("Deger", ayar.deger)
	coin.Parent = bolgeKlasoru
	return coin
end

local function coinTopla(player, coin, ayar)
	local veri = veriAl(player)
	if not veri then return end

	local stats = player:FindFirstChild("leaderstats")
	if not stats then return end

	local canta = stats:FindFirstChild("Canta")
	if not canta then return end

	-- Canta doluysa toplama
	if canta.Value >= veri.cantaKapasite then
		return
	end

	-- Toplama hizi siniri (saniyede en fazla 'toplamaHizi' kadar)
	local simdi = tick()
	if simdi - veri.sonToplama < (1 / (veri.toplamaHizi * 4)) then
		return
	end
	veri.sonToplama = simdi

	local deger = coin:GetAttribute("Deger") or 1
	canta.Value = math.min(canta.Value + deger, veri.cantaKapasite)

	-- Coin'i gizle, sonra yeni yerde geri getir
	coin.Transparency = 1
	local isik = coin:FindFirstChildWhichIsA("Light")
	if isik then isik.Enabled = false end

	task.delay(3, function()
		if coin and coin.Parent then
			local zemin = bolgeler:FindFirstChild(ayar.zeminAdi)
			if zemin then
				local alan = zemin.Size.X / 2 - 4
				local x = zemin.Position.X + math.random(-alan, alan)
				local z = zemin.Position.Z + math.random(-alan, alan)
				coin.CFrame = CFrame.new(x, zemin.Position.Y + 3, z)
			end
			coin.Transparency = 0
			if isik then isik.Enabled = true end
		end
	end)
end

-- Bolgeleri kur
local bolgeKlasorleri = {}

for _, ayar in ipairs(AYARLAR.bolgeler) do
	local zemin = bolgeler:FindFirstChild(ayar.zeminAdi)
	if zemin then
		local klasor = Instance.new("Folder")
		klasor.Name = ayar.zeminAdi .. "_Coinler"
		klasor.Parent = bolgeler

		for _ = 1, ayar.coinSayisi do
			local coin = coinOlustur(klasor, zemin, ayar)
			coin.Touched:Connect(function(hit)
				local player = Players:GetPlayerFromCharacter(hit.Parent)
				if not player then return end
				if coin.Transparency > 0 then return end

				-- Kilitli bolge kontrolu
				if ayar.kilit then
					local veri = veriAl(player)
					if not veri or not veri.bolge2Acik then
						return
					end
				end

				coinTopla(player, coin, ayar)
			end)
		end

		bolgeKlasorleri[ayar.zeminAdi] = klasor
	end
end

-- Coinleri dondur (tek dongu, tum coinler)
RunService.Heartbeat:Connect(function(dt)
	for _, klasor in pairs(bolgeKlasorleri) do
		for _, coin in ipairs(klasor:GetChildren()) do
			if coin:IsA("BasePart") and coin.Transparency < 1 then
				-- Y ekseninde donunce para "sergileniyor" gibi doner
				coin.CFrame = coin.CFrame * CFrame.Angles(0, math.rad(120 * dt), 0)
			end
		end
	end
end)

-- ============ SATIS ALANI ============
local satisBekleme = {}

satisAlani.Touched:Connect(function(hit)
	local player = Players:GetPlayerFromCharacter(hit.Parent)
	if not player then return end

	local simdi = tick()
	if satisBekleme[player.UserId] and simdi - satisBekleme[player.UserId] < 0.5 then
		return
	end
	satisBekleme[player.UserId] = simdi

	local stats = player:FindFirstChild("leaderstats")
	if not stats then return end

	local canta = stats:FindFirstChild("Canta")
	local para = stats:FindFirstChild("Para")
	if not canta or not para or canta.Value <= 0 then return end

	para.Value += canta.Value
	canta.Value = 0
end)

-- ============ YUKSELTMELER ============
local yukseltmeBekleme = {}

local function yukseltmeKur(pad, tur)
	if not pad then return end

	pad.Touched:Connect(function(hit)
		local player = Players:GetPlayerFromCharacter(hit.Parent)
		if not player then return end

		local simdi = tick()
		if yukseltmeBekleme[player.UserId] and simdi - yukseltmeBekleme[player.UserId] < 1 then
			return
		end
		yukseltmeBekleme[player.UserId] = simdi

		local veri = veriAl(player)
		local stats = player:FindFirstChild("leaderstats")
		if not veri or not stats then return end

		local para = stats:FindFirstChild("Para")
		if not para then return end

		local altYazi = pad:FindFirstChild("SyncixEtiket")
		altYazi = altYazi and altYazi:FindFirstChild("AltYazi")

		if tur == "canta" then
			if para.Value >= veri.cantaFiyat then
				para.Value -= veri.cantaFiyat
				veri.cantaKapasite += AYARLAR.cantaArtis
				veri.cantaFiyat = math.floor(veri.cantaFiyat * AYARLAR.cantaFiyatCarpan)
				if altYazi then
					altYazi.Text = "Canta: " .. veri.cantaKapasite .. " | Fiyat: " .. veri.cantaFiyat
				end
			elseif altYazi then
				altYazi.Text = "Yetersiz para (" .. veri.cantaFiyat .. ")"
			end
		elseif tur == "hiz" then
			if para.Value >= veri.hizFiyat then
				para.Value -= veri.hizFiyat
				veri.toplamaHizi += AYARLAR.hizArtis
				veri.hizFiyat = math.floor(veri.hizFiyat * AYARLAR.hizFiyatCarpan)
				if altYazi then
					altYazi.Text = "Hiz: " .. veri.toplamaHizi .. " | Fiyat: " .. veri.hizFiyat
				end
			elseif altYazi then
				altYazi.Text = "Yetersiz para (" .. veri.hizFiyat .. ")"
			end
		end
	end)
end

yukseltmeKur(yukseltmeler:FindFirstChild("CantaYukseltme"), "canta")
yukseltmeKur(yukseltmeler:FindFirstChild("HizYukseltme"), "hiz")

-- Tabelalar
etiketOlustur(satisAlani, "SATIS ALANI", "Coinlerini paraya cevir", Color3.fromRGB(46, 204, 113))
etiketOlustur(
	yukseltmeler:FindFirstChild("CantaYukseltme"),
	"CANTA +" .. AYARLAR.cantaArtis,
	"Fiyat: " .. AYARLAR.cantaBaslangicFiyat,
	Color3.fromRGB(230, 126, 34)
)
etiketOlustur(
	yukseltmeler:FindFirstChild("HizYukseltme"),
	"HIZ +" .. AYARLAR.hizArtis,
	"Fiyat: " .. AYARLAR.hizBaslangicFiyat,
	Color3.fromRGB(155, 89, 182)
)
etiketOlustur(
	bolgeler:FindFirstChild("Bolge2Zemin"),
	"BOLGE 2",
	AYARLAR.bolge2Fiyat .. " para (5x deger)",
	Color3.fromRGB(120, 220, 255)
)

-- ============ BOLGE 2 KILIDI ============
local bolge2Zemin = bolgeler:FindFirstChild("Bolge2Zemin")
if bolge2Zemin then
	bolge2Zemin.Touched:Connect(function(hit)
		local player = Players:GetPlayerFromCharacter(hit.Parent)
		if not player then return end

		local veri = veriAl(player)
		local stats = player:FindFirstChild("leaderstats")
		if not veri or not stats or veri.bolge2Acik then return end

		local para = stats:FindFirstChild("Para")
		if para and para.Value >= AYARLAR.bolge2Fiyat then
			para.Value -= AYARLAR.bolge2Fiyat
			veri.bolge2Acik = true
		end
	end)
end

print("[Simulator] Oyun hazir. Bolge sayisi: " .. #AYARLAR.bolgeler)
