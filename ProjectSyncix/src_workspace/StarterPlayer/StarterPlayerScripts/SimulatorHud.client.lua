--[[
	COIN SIMULATOR - Oyuncu Arayuzu (HUD)
	Syncix ile editorden yazildi.
	Ekranda canta doluluk orani, para ve kisa yonlendirme gosterir.
--]]


-- ============ CEVIRI ============
-- Metinler ReplicatedStorage'daki OyunCevirileri tablosundan gelir ve editorde
-- OyunCevirileri.csv dosyasi olarak Excel/Sheets ile duzenlenebilir.
-- Yeni bir dil eklemek icin CSV'ye sutun eklemek yeterli, koda dokunmaya gerek yok.
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local LocalizationService = game:GetService("LocalizationService")

local ceviriler = {}
do
	local tablo = ReplicatedStorage:FindFirstChild("OyunCevirileri")
	local dil = "tr"
	pcall(function()
		dil = string.sub(LocalizationService.RobloxLocaleId, 1, 2)
	end)

	if tablo and tablo:IsA("LocalizationTable") then
		for _, girdi in ipairs(tablo:GetEntries()) do
			-- Oyuncunun dili yoksa kaynak metne duseriz; boylece eksik ceviri
			-- bos ekran degil, anlasilir bir metin uretir.
			ceviriler[girdi.Key] = girdi.Values[dil] or girdi.Values["tr"] or girdi.Source
		end
	end
end

local function T(anahtar, varsayilan)
	return ceviriler[anahtar] or varsayilan
end

local Players = game:GetService("Players")
local RunService = game:GetService("RunService")

local player = Players.LocalPlayer
local stats = player:WaitForChild("leaderstats")
local para = stats:WaitForChild("Para")
local canta = stats:WaitForChild("Canta")

-- ============ ARAYUZ ============
local gui = Instance.new("ScreenGui")
gui.Name = "SimulatorHud"
gui.ResetOnSpawn = false
gui.Parent = player:WaitForChild("PlayerGui")

local cerceve = Instance.new("Frame")
cerceve.Name = "Panel"
cerceve.Size = UDim2.new(0, 260, 0, 96)
cerceve.Position = UDim2.new(0, 16, 0, 16)
cerceve.BackgroundColor3 = Color3.fromRGB(20, 24, 32)
cerceve.BackgroundTransparency = 0.15
cerceve.BorderSizePixel = 0
cerceve.Parent = gui

local kose = Instance.new("UICorner")
kose.CornerRadius = UDim.new(0, 10)
kose.Parent = cerceve

-- Para satiri
local paraYazi = Instance.new("TextLabel")
paraYazi.Name = "Para"
paraYazi.Size = UDim2.new(1, -20, 0, 28)
paraYazi.Position = UDim2.new(0, 10, 0, 8)
paraYazi.BackgroundTransparency = 1
paraYazi.TextColor3 = Color3.fromRGB(255, 209, 0)
paraYazi.TextScaled = true
paraYazi.Font = Enum.Font.GothamBold
paraYazi.TextXAlignment = Enum.TextXAlignment.Left
paraYazi.Text = T("para", "Para") .. ": 0"
paraYazi.Parent = cerceve

-- Canta satiri
local cantaYazi = Instance.new("TextLabel")
cantaYazi.Name = "Canta"
cantaYazi.Size = UDim2.new(1, -20, 0, 22)
cantaYazi.Position = UDim2.new(0, 10, 0, 40)
cantaYazi.BackgroundTransparency = 1
cantaYazi.TextColor3 = Color3.fromRGB(235, 235, 235)
cantaYazi.TextScaled = true
cantaYazi.Font = Enum.Font.Gotham
cantaYazi.TextXAlignment = Enum.TextXAlignment.Left
cantaYazi.Text = T("canta", "Çanta") .. ": 0"
cantaYazi.Parent = cerceve

-- Doluluk cubugu
local cubukArka = Instance.new("Frame")
cubukArka.Name = "CubukArka"
cubukArka.Size = UDim2.new(1, -20, 0, 12)
cubukArka.Position = UDim2.new(0, 10, 0, 68)
cubukArka.BackgroundColor3 = Color3.fromRGB(45, 50, 60)
cubukArka.BorderSizePixel = 0
cubukArka.Parent = cerceve

local kose2 = Instance.new("UICorner")
kose2.CornerRadius = UDim.new(0, 6)
kose2.Parent = cubukArka

local cubukDolu = Instance.new("Frame")
cubukDolu.Name = "CubukDolu"
cubukDolu.Size = UDim2.new(0, 0, 1, 0)
cubukDolu.BackgroundColor3 = Color3.fromRGB(46, 204, 113)
cubukDolu.BorderSizePixel = 0
cubukDolu.Parent = cubukArka

local kose3 = Instance.new("UICorner")
kose3.CornerRadius = UDim.new(0, 6)
kose3.Parent = cubukDolu

-- Yonlendirme yazisi
local ipucu = Instance.new("TextLabel")
ipucu.Name = "Ipucu"
ipucu.Size = UDim2.new(0, 320, 0, 26)
ipucu.Position = UDim2.new(0, 16, 0, 120)
ipucu.BackgroundTransparency = 1
ipucu.TextColor3 = Color3.fromRGB(255, 255, 255)
ipucu.TextStrokeTransparency = 0.4
ipucu.TextScaled = true
ipucu.Font = Enum.Font.GothamMedium
ipucu.TextXAlignment = Enum.TextXAlignment.Left
ipucu.Text = T("basla", "Coin topla, sonra yeşil alanda sat!")
ipucu.Parent = gui

-- ============ GUNCELLEME ============
-- Kapasiteyi sunucudan bilmiyoruz; gorunen en yuksek degeri referans aliriz.
local tahminiKapasite = 10

local function guncelle()
	paraYazi.Text = T("para", "Para") .. ": " .. para.Value

	if canta.Value > tahminiKapasite then
		tahminiKapasite = canta.Value
	end

	cantaYazi.Text = T("canta", "Çanta") .. ": " .. canta.Value .. " / " .. tahminiKapasite

	local oran = 0
	if tahminiKapasite > 0 then
		oran = math.clamp(canta.Value / tahminiKapasite, 0, 1)
	end
	cubukDolu.Size = UDim2.new(oran, 0, 1, 0)

	if oran >= 1 then
		cubukDolu.BackgroundColor3 = Color3.fromRGB(231, 76, 60)
		ipucu.Text = T("dolu", "Çanta dolu! Yeşil alana git ve sat.")
	elseif canta.Value > 0 then
		cubukDolu.BackgroundColor3 = Color3.fromRGB(46, 204, 113)
		ipucu.Text = T("topluyor", "Coin topluyorsun... Satmak için yeşil alan.")
	else
		cubukDolu.BackgroundColor3 = Color3.fromRGB(46, 204, 113)
		ipucu.Text = T("basla", "Coin topla, sonra yeşil alanda sat!")
	end
end

para.Changed:Connect(guncelle)
canta.Changed:Connect(guncelle)
guncelle()

-- ============ KARSILAMA MESAJI ============
-- Metin ReplicatedStorage'daki HosgeldinMesaji StringValue'sundan gelir ve
-- editorde HosgeldinMesaji.txt dosyasi olarak duz metin halinde duzenlenebilir.
-- Metni degistirmek icin oyuna dokunmaya gerek yok.
local ReplicatedStorage = game:GetService("ReplicatedStorage")

task.spawn(function()
	local mesajDegeri = ReplicatedStorage:FindFirstChild("HosgeldinMesaji")
	if not mesajDegeri or not mesajDegeri:IsA("StringValue") then
		return
	end

	local karsilama = Instance.new("TextLabel")
	karsilama.Name = "Karsilama"
	karsilama.Size = UDim2.new(0, 520, 0, 40)
	karsilama.Position = UDim2.new(0.5, -260, 0, 24)
	karsilama.BackgroundColor3 = Color3.fromRGB(20, 24, 32)
	karsilama.BackgroundTransparency = 0.2
	karsilama.BorderSizePixel = 0
	karsilama.TextColor3 = Color3.fromRGB(255, 209, 0)
	karsilama.TextScaled = true
	karsilama.Font = Enum.Font.GothamBold
	karsilama.Text = mesajDegeri.Value
	karsilama.Parent = gui

	local kose = Instance.new("UICorner")
	kose.CornerRadius = UDim.new(0, 8)
	kose.Parent = karsilama

	-- Metin editorde degisirse ekranda da aninda degissin.
	mesajDegeri:GetPropertyChangedSignal("Value"):Connect(function()
		karsilama.Text = mesajDegeri.Value
	end)

	task.wait(6)
	for i = 0, 20 do
		karsilama.BackgroundTransparency = 0.2 + (i / 25)
		karsilama.TextTransparency = i / 20
		task.wait(0.05)
	end
	karsilama:Destroy()
end)
