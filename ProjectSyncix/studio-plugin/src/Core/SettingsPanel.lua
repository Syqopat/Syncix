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

local RENK = {
	arka     = Color3.fromRGB(30, 33, 40),
	kutu     = Color3.fromRGB(46, 50, 60),
	yazi     = Color3.fromRGB(235, 237, 242),
	soluk    = Color3.fromRGB(150, 156, 168),
	yesil    = Color3.fromRGB(46, 160, 87),
	sari     = Color3.fromRGB(200, 150, 40),
	kirmizi  = Color3.fromRGB(190, 70, 70),
	mavi     = Color3.fromRGB(60, 120, 200),
}

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

	local toolbar = self.plugin:CreateToolbar("Syncix")
	-- Ikon BILEREK bos.
	--
	-- Onceden buraya Roblox'un yerlesik ROBUX ikonu konmustu; arac cubugunda
	-- Syncix'in yaninda para simgesi duruyordu ve urunle hicbir ilgisi yoktu.
	-- Kendi ikonumuzu koymak icin gorseli Roblox'a asset olarak yuklemek
	-- gerekiyor (rbxassetid), bu da yayinlama islemi ve hesap sahibinin karari.
	-- Yanlis bir ikondan iyisi, yalnizca ad gostermek.
	self.dugme = toolbar:CreateButton("Syncix", "Syncix status and port settings", "")
	self.dugme.ClickableWhenViewportHidden = true

	self.dugme.Click:Connect(function()
		self:AcKapa()
	end)
end

local function etiket(ust, metin, boyut, konum, renk, kalin)
	local l = Instance.new("TextLabel")
	l.Size = boyut
	l.Position = konum
	l.BackgroundTransparency = 1
	l.TextColor3 = renk or RENK.yazi
	l.TextXAlignment = Enum.TextXAlignment.Left
	l.TextYAlignment = Enum.TextYAlignment.Top
	l.TextWrapped = true
	l.Font = kalin and Enum.Font.GothamBold or Enum.Font.Gotham
	l.TextSize = kalin and 14 or 13
	l.Text = metin
	l.Parent = ust
	return l
end

local function dugmeYap(ust, metin, boyut, konum, renk)
	local b = Instance.new("TextButton")
	b.Size = boyut
	b.Position = konum
	b.BackgroundColor3 = renk
	b.BorderSizePixel = 0
	b.TextColor3 = Color3.fromRGB(255, 255, 255)
	b.Font = Enum.Font.GothamBold
	b.TextSize = 13
	b.Text = metin
	b.Parent = ust
	local k = Instance.new("UICorner")
	k.CornerRadius = UDim.new(0, 5)
	k.Parent = b
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
		420, 420,
		380, 360
	)
	self.gui = self.plugin:CreateDockWidgetPluginGui("SyncixPanel", bilgi)
	self.gui.Title = "Syncix"

	local cerceve = Instance.new("Frame")
	cerceve.Size = UDim2.new(1, 0, 1, 0)
	cerceve.BackgroundColor3 = RENK.arka
	cerceve.BorderSizePixel = 0
	cerceve.Parent = self.gui

	-- Ust kenardaki ince mavi cizgi. Tek isi panelin Syncix'e ait oldugunu
	-- bir bakista belli etmek; logodaki mavi ile ayni renk.
	local seritCizgi = Instance.new("Frame")
	seritCizgi.Size = UDim2.new(1, 0, 0, 2)
	seritCizgi.Position = UDim2.new(0, 0, 0, 0)
	seritCizgi.BackgroundColor3 = RENK.mavi
	seritCizgi.BorderSizePixel = 0
	seritCizgi.ZIndex = 5
	seritCizgi.Parent = cerceve

	-- Durum
	etiket(cerceve, "Status", UDim2.new(1, -24, 0, 20), UDim2.new(0, 12, 0, 10), RENK.soluk, true)
	self.durumYazi = etiket(cerceve, "...", UDim2.new(1, -24, 0, 56), UDim2.new(0, 12, 0, 32), RENK.yazi)

	-- Port
	etiket(cerceve, "Port", UDim2.new(1, -24, 0, 20), UDim2.new(0, 12, 0, 96), RENK.soluk, true)
	etiket(
		cerceve,
		"Leave empty and Syncix finds the running core itself (8080-8089).\nType a port to pin this window to one project.",
		UDim2.new(1, -24, 0, 34), UDim2.new(0, 12, 0, 116), RENK.soluk
	)

	self.portKutu = Instance.new("TextBox")
	self.portKutu.Size = UDim2.new(0, 150, 0, 30)
	self.portKutu.Position = UDim2.new(0, 12, 0, 154)
	self.portKutu.BackgroundColor3 = RENK.kutu
	self.portKutu.BorderSizePixel = 0
	self.portKutu.TextColor3 = RENK.yazi
	self.portKutu.PlaceholderText = "Automatic"
	self.portKutu.PlaceholderColor3 = RENK.soluk
	self.portKutu.Font = Enum.Font.Code
	self.portKutu.TextSize = 14
	self.portKutu.ClearTextOnFocus = false
	self.portKutu.Text = ""
	self.portKutu.Parent = cerceve
	local pk = Instance.new("UICorner")
	pk.CornerRadius = UDim.new(0, 5)
	pk.Parent = self.portKutu

	local uygula = dugmeYap(cerceve, "Apply and connect", UDim2.new(0, 150, 0, 30), UDim2.new(0, 172, 0, 154), RENK.yesil)
	local temizle = dugmeYap(cerceve, "Automatic", UDim2.new(0, 74, 0, 30), UDim2.new(0, 330, 0, 154), RENK.mavi)

	-- Duraklat / Devam
	self.duraklatDugme = dugmeYap(cerceve, "...", UDim2.new(0, 150, 0, 30), UDim2.new(0, 12, 0, 190), RENK.kutu)
	self.duraklatDugme.Activated:Connect(function()
		self.connectionManager:SetPaused(not self.connectionManager:IsPaused())
		self:Yenile()
	end)

	-- Guvenlik
	self.izinDugme = dugmeYap(cerceve, "...", UDim2.new(0, 232, 0, 24), UDim2.new(0, 172, 0, 193), RENK.mavi)
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

	-- Gorunum sekmeleri
	self.sekmeAkis = dugmeYap(cerceve, "Recent changes", UDim2.new(0, 150, 0, 26), UDim2.new(0, 12, 0, 226), RENK.kutu)
	self.sekmeProje = dugmeYap(cerceve, "Discovered projects", UDim2.new(0, 150, 0, 26), UDim2.new(0, 170, 0, 226), RENK.kutu)
	self.sekmeAkis.Activated:Connect(function()
		self.gorunum = "activity"
		self:Yenile()
	end)
	self.sekmeProje.Activated:Connect(function()
		self.gorunum = "projects"
		self:Yenile()
	end)
	self.liste = Instance.new("ScrollingFrame")
	self.liste.Size = UDim2.new(1, -24, 1, -270)
	self.liste.Position = UDim2.new(0, 12, 0, 248)
	self.liste.BackgroundColor3 = RENK.kutu
	self.liste.BorderSizePixel = 0
	self.liste.ScrollBarThickness = 6
	self.liste.CanvasSize = UDim2.new(0, 0, 0, 0)
	self.liste.Parent = cerceve
	local lk = Instance.new("UICorner")
	lk.CornerRadius = UDim.new(0, 5)
	lk.Parent = self.liste

	uygula.Activated:Connect(function()
		self:PortUygula(self.portKutu.Text)
	end)
	temizle.Activated:Connect(function()
		self.portKutu.Text = ""
		self:PortUygula("")
	end)

	-- Panel açıkken durumu canlı tut.
	task.spawn(function()
		while self.gui do
			if self.gui.Enabled then
				self:Yenile()
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

	if cm:IsPaused() then
		self.durumYazi.TextColor3 = RENK.sari
		self.durumYazi.Text =
			"Sync PAUSED\nNothing is sent to or applied from the editor.\n"
			.. "Press Resume sync to re-sync the full tree."
	elseif cm.state == "Connected" and bilgi then
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
			self.durumYazi.Text = self.durumYazi.Text
				.. string.format("\n%d conflict(s) — see the Recent changes tab", ozet.cakisma)
			self.durumYazi.TextColor3 = RENK.sari
		end
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
		etiket(self.liste, "No changes yet.\nChange something in Studio, or send a command from the editor.",
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

		local ok = etiket(satir, yonIsareti, UDim2.new(0, 24, 0, 16), UDim2.new(0, 8, 0, 4), yonRenk, true)
		ok.TextXAlignment = Enum.TextXAlignment.Center

		local baslik = k.hedef
		if k.alan then
			baslik = baslik .. "." .. k.alan
		end
		etiket(satir, baslik, UDim2.new(1, -110, 0, 16), UDim2.new(0, 36, 0, 3), RENK.yazi, true)

		local alt = k.tur
		if k.deger and k.deger ~= "" then
			alt = alt .. "  =  " .. k.deger
		end
		if k.cakisma then
			alt = alt .. "   [CAKISMA]"
		end
		etiket(satir, alt, UDim2.new(1, -110, 0, 14), UDim2.new(0, 36, 0, 18), k.cakisma and RENK.kirmizi or RENK.soluk)

		local gecen = math.max(0, math.floor(simdi - k.zaman))
		local zamanMetni = (gecen < 60) and (gecen .. "s ago")
			or (math.floor(gecen / 60) .. "m ago")
		local z = etiket(satir, zamanMetni, UDim2.new(0, 66, 0, 14), UDim2.new(1, -72, 0, 10), RENK.soluk)
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

	-- Proje taramasi ag istegi yapiyor; yalnizca o sekme acikken calistirilir.
	local bulunanlar = self.connectionManager:TaraTumPortlar()
	if #bulunanlar == 0 then
		etiket(self.liste, "No running core found.\nOpen the project in VS Code, or run: syncix up",
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

		etiket(satir, string.format("%s   (port %d)", tostring(b.project), b.port),
			UDim2.new(1, -80, 0, 18), UDim2.new(0, 8, 0, 6), RENK.yazi, true)
		etiket(satir, tostring(b.root),
			UDim2.new(1, -80, 0, 24), UDim2.new(0, 8, 0, 24), RENK.soluk)

		local sec = dugmeYap(satir, "Select", UDim2.new(0, 56, 0, 24), UDim2.new(1, -64, 0, 14), RENK.mavi)
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
