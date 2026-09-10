-- SettingsPanel
-- Studio araç çubuğundaki Syncix düğmesi ve ayar/durum paneli.
--
-- Buradaki asıl iş PORT SEÇİMİ. Eklenti normalde 8080-8089 aralığını tarayıp ilk
-- cevap veren core'a bağlanır. İki proje aynı anda açıksa bu YANLIŞ projeye
-- bağlanabilir. Port alanına bir sayı yazıldığında tarama kapanır ve yalnızca o
-- port denenir; orada Syncix yoksa bağlanılmaz. Böylece "bu Studio penceresi şu
-- projeye bağlansın" kesin olarak söylenebilir.
--
-- Varsayılan bilerek "Automatic"tir, 8080 değildir: core istenen port doluysa
-- kendiliğinden 8081'e geçebiliyor; alanda sabit 8080 yazsaydı o durumda hiç
-- bağlanamazdınız.

local Store = require(script.Parent.Store)

local SettingsPanel = {}
SettingsPanel.__index = SettingsPanel

-- Palet.
--
-- Iki zemin tonu var: panelin arkasi KOYU, kartlar bir ton acik. Ayrimi
-- cizgiyle degil tonla yapmak, kucuk bir panelde daha az gurultu uretiyor.
-- Mavi, logodaki maviyle ayni (#4C8DF5) — panel, ikon ve magaza girdisi
-- tek bir renge dayaniyor.
local RENK = {
	arka     = Color3.fromRGB(22, 24, 29),
	kart     = Color3.fromRGB(30, 33, 40),
	kutu     = Color3.fromRGB(41, 45, 54),
	cizgi    = Color3.fromRGB(52, 57, 68),
	yazi     = Color3.fromRGB(232, 234, 240),
	soluk    = Color3.fromRGB(138, 146, 166),
	yesil    = Color3.fromRGB(58, 176, 106),
	sari     = Color3.fromRGB(214, 162, 54),
	kirmizi  = Color3.fromRGB(214, 88, 88),
	mavi     = Color3.fromRGB(76, 141, 245),
}

-- Bosluk olcegi. Elle piksel yazmak yerine buradan seciliyor; panelin her
-- yerinde ayni ritim olusuyor.
local BOSLUK = { dar = 6, orta = 10, genis = 14 }

-- Arac cubugu ikonu.
--
-- Roblox plugin dugmesine ikon koymanin tek yolu, gorseli Roblox'a asset
-- olarak yuklemek: yerel bir dosya kullanilamiyor. Kaynagi
-- vscode-extension/resources/logo.png; ayni isaret kenar cubugu ikonunda ve
-- magaza girdisinde de kullaniliyor.
--
-- Once Roblox'un yerlesik ROBUX ikonu vardi (urunle ilgisi yoktu), sonra bos
-- dize denendi ve Studio onu "yuklenemedi" sayip baklava seklinde bir yer
-- tutucu gosterdi.
local IKON = "rbxassetid://128567176637407"

function SettingsPanel.new()
	return setmetatable({}, SettingsPanel)
end

function SettingsPanel:OnStart(container)
	self.plugin = container:Get("Plugin").ref
	self.connectionManager = container:Get("ConnectionManager")
	self.activityLog = container:Get("ActivityLog")
	-- Panelde iki gorunum var: "projeler" (hangi core'lar calisiyor) ve
	-- "akis" (Syncix ne degistirdi). Varsayilan akis, cunku asil eksik oydu:
	-- degisiklikler tamamen sessiz uygulaniyordu.
	self.gorunum = "activity"

	if not self.plugin then
		return
	end

	-- Arac cubugu, panelin TAMAMEN DISINDA tutuluyor ve her sey pcall icinde.
	--
	-- Burasi StartAll icinden cagriliyor; buradaki bir hata butun eklentiyi
	-- baslatmadan dusuruyor. Ikon denemesi yuzunden senkronun hic calismamasi
	-- kabul edilemez; basarisizlikta yalnizca dugme eksik kalir.
	--
	-- Ikon: once Roblox'un yerlesik ROBUX ikonu vardi (urunle ilgisi yoktu),
	-- sonra bos dize denendi ve Studio onu "yuklenemedi" sayip baklava seklinde
	-- yer tutucu gosterdi. Once argumansiz cagri deneniyor; bu Studio surumunde
	-- desteklenmiyorsa bos dizeli surume dusuluyor.
	local toolbar
	if not pcall(function()
		toolbar = self.plugin:CreateToolbar("Syncix")
	end) or not toolbar then
		warn("[Syncix] Toolbar could not be created; sync still works.")
		return
	end

	if not pcall(function()
		self.dugme = toolbar:CreateButton("Syncix", "Syncix status and port settings", IKON)
	end) then
		-- Ikon yuklenemezse dugme yine de olusmali; senkron ikona bagli degil.
		pcall(function()
			self.dugme = toolbar:CreateButton("Syncix", "Syncix status and port settings", "")
		end)
	end
	if not self.dugme then
		warn("[Syncix] Toolbar button could not be created; sync still works.")
		return
	end

	self.dugme.ClickableWhenViewportHidden = true

	self.dugme.Click:Connect(function()
		self:AcKapa()
	end)
end

-- ---------------------------------------------------------------------------
-- KUCUK BIR TASARIM DUZENI
--
-- Eski panel her ogeyi elle piksel konumuna koyuyordu (y = 10, 32, 96, 154...).
-- Iki sorunu vardi: bir oge buyudugunde altindakiler ustune biniyordu ve
-- genislikler sabit oldugu icin dar panelde tasiyordu (330 + 74 = 404 piksel,
-- panelin dar hali 380).
--
-- Artik dikey akis UIListLayout ile, yatay yerlesim ORANLA yapiliyor. Hicbir
-- yerde elle Y konumu yok; ogeler kendi boylarini soyluyor, duzen siralamayi
-- hallediyor.
-- ---------------------------------------------------------------------------

local function kose(ust, yaricap)
	local k = Instance.new("UICorner")
	k.CornerRadius = UDim.new(0, yaricap or 6)
	k.Parent = ust
	return k
end

local function dikeyAkis(ust, aralik)
	local d = Instance.new("UIListLayout")
	d.FillDirection = Enum.FillDirection.Vertical
	d.SortOrder = Enum.SortOrder.LayoutOrder
	d.Padding = UDim.new(0, aralik or BOSLUK.orta)
	d.Parent = ust
	return d
end

local function icBosluk(ust, deger)
	local b = Instance.new("UIPadding")
	local u = UDim.new(0, deger)
	b.PaddingTop = u
	b.PaddingBottom = u
	b.PaddingLeft = u
	b.PaddingRight = u
	b.Parent = ust
	return b
end

local function etiket(ust, metin, renk, boyut, kalin)
	local l = Instance.new("TextLabel")
	l.BackgroundTransparency = 1
	l.Size = UDim2.new(1, 0, 0, 0)
	l.AutomaticSize = Enum.AutomaticSize.Y
	l.TextColor3 = renk or RENK.yazi
	l.TextXAlignment = Enum.TextXAlignment.Left
	l.TextYAlignment = Enum.TextYAlignment.Top
	l.TextWrapped = true
	l.Font = kalin and Enum.Font.GothamBold or Enum.Font.Gotham
	l.TextSize = boyut or 13
	l.Text = metin
	l.Parent = ust
	return l
end

--- Bolum basligi: kucuk, buyuk harf, soluk. Iceriginin onune gecmemeli.
local function baslik(ust, metin, sira)
	local l = etiket(ust, string.upper(metin), RENK.soluk, 11, true)
	l.LayoutOrder = sira
	return l
end

--- Icerigi gruplayan kart. Panelin arkasindan bir ton acik.
local function kart(ust, sira)
	local k = Instance.new("Frame")
	k.BackgroundColor3 = RENK.kart
	k.BorderSizePixel = 0
	k.Size = UDim2.new(1, 0, 0, 0)
	k.AutomaticSize = Enum.AutomaticSize.Y
	k.LayoutOrder = sira
	k.Parent = ust
	kose(k, 8)
	icBosluk(k, BOSLUK.orta)
	dikeyAkis(k, BOSLUK.dar)
	return k
end

--- Yatay satir. Genislikler ORANLA veriliyor ki dar panelde tasmasin.
local function satir(ust, yukseklik, sira)
	local r = Instance.new("Frame")
	r.BackgroundTransparency = 1
	r.Size = UDim2.new(1, 0, 0, yukseklik)
	r.LayoutOrder = sira
	r.Parent = ust
	local d = Instance.new("UIListLayout")
	d.FillDirection = Enum.FillDirection.Horizontal
	d.SortOrder = Enum.SortOrder.LayoutOrder
	d.Padding = UDim.new(0, BOSLUK.dar)
	d.Parent = r
	return r
end

-- Liste satirlarinin ICI mutlak yerlesim kullaniyor: her satir sabit
-- yukseklikte ve icindeki uc alan (yon, baslik, zaman) hizali durmali.
-- Ust bolumdeki akis tabanli `etiket` bunun icin uygun degil, o yuzden
-- konumlu bir es var.
--
-- Bu ikisi bir sure YOKTU: `etiket`in imzasini degistirdim ama liste
-- icindeki sekiz cagriyi guncellemeyi unuttum. UDim2 degerleri renk
-- parametresine gitti ve panel her yenilenmede hata verdi.
local function kutuEtiket(ust, metin, boyut, konum, renk, kalin)
	local l = Instance.new("TextLabel")
	l.Size = boyut
	l.Position = konum
	l.BackgroundTransparency = 1
	l.TextColor3 = renk or RENK.yazi
	l.TextXAlignment = Enum.TextXAlignment.Left
	l.TextYAlignment = Enum.TextYAlignment.Top
	l.TextWrapped = true
	l.Font = kalin and Enum.Font.GothamBold or Enum.Font.Gotham
	l.TextSize = kalin and 12 or 11
	l.Text = metin
	l.Parent = ust
	return l
end

local function kutuDugme(ust, metin, boyut, konum, renk)
	local b = Instance.new("TextButton")
	b.Size = boyut
	b.Position = konum
	b.BackgroundColor3 = renk
	b.BorderSizePixel = 0
	b.AutoButtonColor = true
	b.TextColor3 = RENK.yazi
	b.Font = Enum.Font.GothamMedium
	b.TextSize = 11
	b.Text = metin
	b.Parent = ust
	kose(b, 5)
	return b
end

local function dugmeYap(ust, metin, oran, renk, sira)
	local b = Instance.new("TextButton")
	b.Size = UDim2.new(oran, -BOSLUK.dar, 1, 0)
	b.BackgroundColor3 = renk
	b.BorderSizePixel = 0
	b.AutoButtonColor = true
	b.TextColor3 = RENK.yazi
	b.Font = Enum.Font.GothamMedium
	b.TextSize = 12
	b.Text = metin
	b.LayoutOrder = sira
	b.Parent = ust
	kose(b, 6)
	return b
end

function SettingsPanel:AcKapa()
	if self.gui then
		self.gui.Enabled = not self.gui.Enabled
		if self.gui.Enabled then
			self:Yenile()
		end
		return
	end

	local bilgi = DockWidgetPluginGuiInfo.new(
		Enum.InitialDockState.Float,
		true, true,
		420, 520,
		360, 420
	)
	self.gui = self.plugin:CreateDockWidgetPluginGui("SyncixPanel", bilgi)
	self.gui.Title = "Syncix"

	local cerceve = Instance.new("Frame")
	cerceve.Size = UDim2.new(1, 0, 1, 0)
	cerceve.BackgroundColor3 = RENK.arka
	cerceve.BorderSizePixel = 0
	cerceve.Parent = self.gui

	-- Ust bolum: kartlar dikey akista.
	-- AutomaticSize sayesinde durum yazisi uzayinca kart da uzuyor ve
	-- altindakiler kendiliginden asagi kayiyor.
	local ust = Instance.new("Frame")
	ust.BackgroundTransparency = 1
	ust.Size = UDim2.new(1, 0, 0, 0)
	ust.AutomaticSize = Enum.AutomaticSize.Y
	ust.Parent = cerceve
	icBosluk(ust, BOSLUK.genis)
	dikeyAkis(ust, BOSLUK.orta)

	-- Kimlik satiri: logodaki mavi kare + ad.
	local kimlik = satir(ust, 18, 1)
	-- Logonun kendisi. Yuklenemezse (asset erisimi yoksa) arkasindaki mavi
	-- kare gorunur kalir; panel ikona bagli degil.
	local isaret = Instance.new("ImageLabel")
	isaret.Size = UDim2.new(0, 16, 0, 16)
	isaret.BackgroundColor3 = RENK.mavi
	isaret.BackgroundTransparency = 0
	isaret.BorderSizePixel = 0
	isaret.Image = IKON
	isaret.ScaleType = Enum.ScaleType.Fit
	isaret.LayoutOrder = 1
	isaret.Parent = kimlik
	kose(isaret, 4)
	local ad = etiket(kimlik, "SYNCIX", RENK.yazi, 12, true)
	ad.AutomaticSize = Enum.AutomaticSize.None
	ad.Size = UDim2.new(1, -22, 1, 0)
	ad.TextYAlignment = Enum.TextYAlignment.Center
	ad.LayoutOrder = 2

	-- DURUM
	baslik(ust, "Status", 2)
	local durumKart = kart(ust, 3)
	local durumSatir = Instance.new("Frame")
	durumSatir.BackgroundTransparency = 1
	durumSatir.Size = UDim2.new(1, 0, 0, 0)
	durumSatir.AutomaticSize = Enum.AutomaticSize.Y
	durumSatir.Parent = durumKart

	-- Renkli nokta: durum rengini yazinin renginden ayirmak, "bagli" halinde
	-- metnin beyaz kalip yalnizca noktanin yesil olmasini sagliyor.
	self.durumNokta = Instance.new("Frame")
	self.durumNokta.Size = UDim2.new(0, 8, 0, 8)
	self.durumNokta.Position = UDim2.new(0, 0, 0, 4)
	self.durumNokta.BackgroundColor3 = RENK.soluk
	self.durumNokta.BorderSizePixel = 0
	self.durumNokta.Parent = durumSatir
	kose(self.durumNokta, 4)

	self.durumYazi = etiket(durumSatir, "...", RENK.yazi, 12)
	self.durumYazi.Position = UDim2.new(0, 16, 0, 0)
	self.durumYazi.Size = UDim2.new(1, -16, 0, 0)

	-- BAGLANTI
	baslik(ust, "Connection", 4)
	local baglantiKart = kart(ust, 5)
	etiket(
		baglantiKart,
		"Leave the port empty and Syncix finds the running core itself (8080-8089). Type a port to pin this window to one project.",
		RENK.soluk, 11
	)

	local portSatir = satir(baglantiKart, 28, 2)
	self.portKutu = Instance.new("TextBox")
	self.portKutu.Size = UDim2.new(0.32, -BOSLUK.dar, 1, 0)
	self.portKutu.BackgroundColor3 = RENK.kutu
	self.portKutu.BorderSizePixel = 0
	self.portKutu.TextColor3 = RENK.yazi
	self.portKutu.PlaceholderText = "Automatic"
	self.portKutu.PlaceholderColor3 = RENK.soluk
	self.portKutu.Font = Enum.Font.Code
	self.portKutu.TextSize = 13
	self.portKutu.ClearTextOnFocus = false
	self.portKutu.Text = ""
	self.portKutu.LayoutOrder = 1
	self.portKutu.Parent = portSatir
	kose(self.portKutu, 6)

	local uygula = dugmeYap(portSatir, "Apply", 0.38, RENK.mavi, 2)
	local temizle = dugmeYap(portSatir, "Automatic", 0.30, RENK.kutu, 3)

	-- SENKRON
	baslik(ust, "Sync", 6)
	local senkronSatir = satir(ust, 30, 7)
	self.duraklatDugme = dugmeYap(senkronSatir, "...", 0.5, RENK.kutu, 1)
	self.izinDugme = dugmeYap(senkronSatir, "...", 0.5, RENK.kutu, 2)

	self.duraklatDugme.Activated:Connect(function()
		self.connectionManager:SetPaused(not self.connectionManager:IsPaused())
		self:Yenile()
	end)
	self.izinDugme.Activated:Connect(function()
		local yeni = not self.connectionManager.izinSor
		self.connectionManager.izinSor = yeni
		Store.Set(self.plugin, "syncix_izin_sor", yeni)
		print(string.format(
			"[Syncix] Ask for connection permission: %s",
			yeni and "ON (every new project must be approved)" or "OFF"
		))
		self:Yenile()
	end)

	-- SEKMELER
	local sekmeSatir = satir(ust, 26, 8)
	self.sekmeAkis = dugmeYap(sekmeSatir, "Recent changes", 0.5, RENK.kutu, 1)
	self.sekmeProje = dugmeYap(sekmeSatir, "Projects", 0.5, RENK.kutu, 2)
	self.sekmeAkis.Activated:Connect(function()
		self.gorunum = "activity"
		self:Yenile()
	end)
	self.sekmeProje.Activated:Connect(function()
		self.gorunum = "projects"
		self:Yenile()
	end)

	-- LISTE: kalan yuksekligi doldurur.
	self.liste = Instance.new("ScrollingFrame")
	self.liste.BackgroundColor3 = RENK.kart
	self.liste.BorderSizePixel = 0
	self.liste.ScrollBarThickness = 5
	self.liste.ScrollBarImageColor3 = RENK.cizgi
	self.liste.CanvasSize = UDim2.new(0, 0, 0, 0)
	self.liste.Parent = cerceve
	kose(self.liste, 8)

	-- Listenin yeri ust bolumun GERCEK yuksekligine gore ayarlanir.
	-- Sabit bir sayi yazmak, durum yazisi uzadiginda listenin ustune
	-- binmesine yol aciyordu.
	local function listeyiYerlestir()
		local y = ust.AbsoluteSize.Y
		self.liste.Position = UDim2.new(0, BOSLUK.genis, 0, y)
		self.liste.Size = UDim2.new(1, -BOSLUK.genis * 2, 1, -y - BOSLUK.genis)
	end
	ust:GetPropertyChangedSignal("AbsoluteSize"):Connect(listeyiYerlestir)
	listeyiYerlestir()

	uygula.Activated:Connect(function()
		self:PortUygula(self.portKutu.Text)
	end)
	temizle.Activated:Connect(function()
		self.portKutu.Text = ""
		self:PortUygula("")
	end)

	-- Panel acikken durumu canli tut.
	--
	-- Yenile pcall icinde: burada olusan bir hata task'i olduruyordu ve panel
	-- bir daha HIC guncellenmiyordu. Sonuc yaniltiyordu — Output "Connected"
	-- derken panelde "Not connected" yaziyordu, cunku yazan kod artik
	-- calismiyordu. Hata bir kez bildirilir, dongu devam eder.
	task.spawn(function()
		local hataBildirildi = false
		while self.gui do
			if self.gui.Enabled then
				local ok, hata = pcall(function()
					self:Yenile()
				end)
				if not ok and not hataBildirildi then
					hataBildirildi = true
					warn("[Syncix] Panel refresh failed: " .. tostring(hata))
				end
			end
			task.wait(2)
		end
	end)

	self:Yenile()
end

function SettingsPanel:PortUygula(metin: string)
	local temiz = string.gsub(metin or "", "%s", "")

	if temiz == "" then
		Store.Set(self.plugin, "syncix_port", 0) -- 0 = otomatik
		self.connectionManager:SetManualPort(nil)
		print("[Syncix] Port: automatic. Looking for a running core...")
	else
		local sayi = tonumber(temiz)
		if not sayi or sayi < 1 or sayi > 65535 or sayi ~= math.floor(sayi) then
			warn("[Syncix] Invalid port: " .. temiz .. " (must be a whole number between 1 and 65535)")
			return
		end
		Store.Set(self.plugin, "syncix_port", sayi)
		self.connectionManager:SetManualPort(sayi)
		print(string.format("[Syncix] Port pinned to %d. Only this port will be tried.", sayi))
	end

	self.connectionManager:ForceReconnect()
	self:Yenile()
end

function SettingsPanel:Yenile()
	if not self.durumYazi then return end

	local cm = self.connectionManager
	local bilgi = cm.serverInfo
	local manuel = cm:GetManualPort()

	local portMetni = manuel and tostring(manuel) or "Automatic"
	if self.portKutu and not self.portKutu:IsFocused() then
		self.portKutu.Text = manuel and tostring(manuel) or ""
	end

	local durumRengi = RENK.soluk

	if cm:IsPaused() then
		durumRengi = RENK.sari
		self.durumYazi.TextColor3 = RENK.sari
		self.durumYazi.Text =
			"Sync PAUSED\nNothing is sent to or applied from the editor.\n"
			.. "Press Resume sync to re-sync the full tree."
	elseif cm.state == "Connected" and bilgi then
		-- Bagliyken yazi BEYAZ kaliyor, yalnizca nokta yesil. Butun blogu
		-- yesile boyamak okunurlugu dusuruyordu.
		durumRengi = RENK.yesil
		self.durumYazi.TextColor3 = RENK.yazi
		self.durumYazi.Text = string.format(
			"Connected  •  port setting: %s\nProject: %s\nFolder: %s\nPort: %s   Core: %s",
			portMetni,
			tostring(bilgi.project),
			tostring(bilgi.root),
			tostring(bilgi.port),
			tostring(bilgi.version)
		)
	else
		self.durumYazi.TextColor3 = RENK.soluk
		self.durumYazi.Text = string.format(
			"Not connected  •  port setting: %s\nState: %s\n%s",
			portMetni,
			tostring(cm.state),
			manuel and ("Only port " .. manuel .. " is being tried.")
				or "Scanning ports 8080-8089."
		)
	end

	-- Cakisma varsa durum satirinda goster: sessizce ezilmis bir degisiklik
	-- kullanicinin haberi olmadan kaybolmasin.
	if self.activityLog then
		local ozet = self.activityLog:Ozet()
		if ozet.cakisma > 0 then
			durumRengi = RENK.sari
			self.durumYazi.Text = self.durumYazi.Text
				.. string.format("\n%d conflict(s) — see the Recent changes tab", ozet.cakisma)
			self.durumYazi.TextColor3 = RENK.sari
		end
	end

	if self.durumNokta then
		self.durumNokta.BackgroundColor3 = durumRengi
	end

	if self.duraklatDugme then
		local durdu = self.connectionManager:IsPaused()
		self.duraklatDugme.Text = durdu and "Resume sync" or "Pause sync"
		self.duraklatDugme.BackgroundColor3 = durdu and RENK.sari or RENK.kutu
	end

	if self.izinDugme then
		local acik = self.connectionManager.izinSor == true
		self.izinDugme.Text = acik and "Ask permission: ON" or "Ask permission: OFF"
		self.izinDugme.BackgroundColor3 = acik and RENK.yesil or RENK.kutu
	end

	self:ListeyiDoldur()
end

--- Son degisiklikler akisi.
---
--- Rojo'nun patch visualizer'i bagliniverince buyuk bir farki onaya sunar; biz
--- surekli ve cift yonlu calistigimiz icin onay istemek kullanilamaz olurdu.
--- Bunun yerine ne gelip ne gittigini geriye donuk gosteriyoruz. Cakisan
--- degisiklikler kirmizi isaretlenir.
function SettingsPanel:AkisiDoldur()
	local kayitlar = self.activityLog:Son(40)

	if #kayitlar == 0 then
		kutuEtiket(self.liste, "No changes yet.\nChange something in Studio, or send a command from the editor.",
			UDim2.new(1, -16, 0, 40), UDim2.new(0, 8, 0, 8), RENK.soluk)
		self.liste.CanvasSize = UDim2.new(0, 0, 0, 56)
		return
	end

	local simdi = os.clock()
	local y = 6
	for _, k in ipairs(kayitlar) do
		local satir = Instance.new("Frame")
		satir.Size = UDim2.new(1, -12, 0, 34)
		satir.Position = UDim2.new(0, 6, 0, y)
		satir.BackgroundColor3 = k.cakisma and Color3.fromRGB(70, 32, 32) or RENK.arka
		satir.BorderSizePixel = 0
		satir.Parent = self.liste
		local sk = Instance.new("UICorner")
		sk.CornerRadius = UDim.new(0, 4)
		sk.Parent = satir

		local yonIsareti = (k.yon == "in") and "<-" or "->"
		local yonRenk = k.cakisma and RENK.kirmizi
			or ((k.yon == "in") and RENK.mavi or RENK.yesil)

		local ok = kutuEtiket(satir, yonIsareti, UDim2.new(0, 24, 0, 16), UDim2.new(0, 8, 0, 4), yonRenk, true)
		ok.TextXAlignment = Enum.TextXAlignment.Center

		local baslik = k.hedef
		if k.alan then
			baslik = baslik .. "." .. k.alan
		end
		kutuEtiket(satir, baslik, UDim2.new(1, -110, 0, 16), UDim2.new(0, 36, 0, 3), RENK.yazi, true)

		local alt = k.tur
		if k.deger and k.deger ~= "" then
			alt = alt .. "  =  " .. k.deger
		end
		if k.cakisma then
			alt = alt .. "   [CAKISMA]"
		end
		kutuEtiket(satir, alt, UDim2.new(1, -110, 0, 14), UDim2.new(0, 36, 0, 18), k.cakisma and RENK.kirmizi or RENK.soluk)

		local gecen = math.max(0, math.floor(simdi - k.zaman))
		local zamanMetni = (gecen < 60) and (gecen .. "s ago")
			or (math.floor(gecen / 60) .. "m ago")
		local z = kutuEtiket(satir, zamanMetni, UDim2.new(0, 66, 0, 14), UDim2.new(1, -72, 0, 10), RENK.soluk)
		z.TextXAlignment = Enum.TextXAlignment.Right

		y += 38
	end
	self.liste.CanvasSize = UDim2.new(0, 0, 0, y)
end

function SettingsPanel:ListeyiDoldur()
	if not self.liste then return end

	for _, c in ipairs(self.liste:GetChildren()) do
		if not c:IsA("UICorner") then
			c:Destroy()
		end
	end

	-- Sekme gorunumleri
	if self.sekmeAkis then
		self.sekmeAkis.BackgroundColor3 = (self.gorunum == "activity") and RENK.mavi or RENK.kutu
		self.sekmeProje.BackgroundColor3 = (self.gorunum == "projects") and RENK.mavi or RENK.kutu
	end

	if self.gorunum == "activity" then
		self:AkisiDoldur()
		return
	end

	-- Proje taramasi ON portu tek tek yokluyor; panel her 2 saniyede bir
	-- yenilendigi icin bu, saniyede bes HTTP istegi demekti ve baglantiyi
	-- calkantiya sokuyordu (Output'ta surekli Connected -> Disconnected).
	-- Sonuc onbelleklenip en fazla 10 saniyede bir tazeleniyor.
	local simdiTara = os.clock()
	if not self.projeOnbellek or (simdiTara - (self.projeOnbellekZaman or 0)) > 10 then
		self.projeOnbellek = self.connectionManager:TaraTumPortlar()
		self.projeOnbellekZaman = simdiTara
	end
	local bulunanlar = self.projeOnbellek
	if #bulunanlar == 0 then
		kutuEtiket(self.liste, "No running core found.\nOpen the project in VS Code, or run: syncix up",
			UDim2.new(1, -16, 0, 40), UDim2.new(0, 8, 0, 8), RENK.soluk)
		self.liste.CanvasSize = UDim2.new(0, 0, 0, 56)
		return
	end

	local y = 6
	for _, b in ipairs(bulunanlar) do
		local satir = Instance.new("Frame")
		satir.Size = UDim2.new(1, -12, 0, 52)
		satir.Position = UDim2.new(0, 6, 0, y)
		satir.BackgroundColor3 = RENK.arka
		satir.BorderSizePixel = 0
		satir.Parent = self.liste
		local sk = Instance.new("UICorner")
		sk.CornerRadius = UDim.new(0, 4)
		sk.Parent = satir

		kutuEtiket(satir, string.format("%s   (port %d)", tostring(b.project), b.port),
			UDim2.new(1, -80, 0, 18), UDim2.new(0, 8, 0, 6), RENK.yazi, true)
		kutuEtiket(satir, tostring(b.root),
			UDim2.new(1, -80, 0, 24), UDim2.new(0, 8, 0, 24), RENK.soluk)

		local sec = kutuDugme(satir, "Select", UDim2.new(0, 56, 0, 24), UDim2.new(1, -64, 0, 14), RENK.mavi)
		local port = b.port
		sec.Activated:Connect(function()
			self.portKutu.Text = tostring(port)
			self:PortUygula(tostring(port))
		end)

		y += 58
	end
	self.liste.CanvasSize = UDim2.new(0, 0, 0, y)
end

return SettingsPanel
