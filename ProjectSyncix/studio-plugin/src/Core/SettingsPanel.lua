-- SettingsPanel
-- Studio araç çubuğundaki Syncix düğmesi ve ayar/durum paneli.
--
-- Buradaki asıl iş PORT SEÇİMİ. Eklenti normalde 8080-8089 aralığını tarayıp first
-- cevap veren core'a bağlanır. İki projectInfo aynı anda açıksa bu YANLIŞ projeye
-- bağlanabilir. Port alanına bir sayı yazıldığında tarama kapanır ve yalnızca o
-- port denenir; orada Syncix yoksa bağlanılmaz. Böylece "bu Studio penceresi şu
-- projeye bağlansın" kesin olarak söylenebilir.
--
-- Varsayılan bilerek "Automatic"tir, 8080 değildir: core requested port doluysa
-- kendiliğinden 8081'e geçebiliyor; alanda sabit 8080 yazsaydı o durumda hiç
-- bağlanamazdınız.

local Store = require(script.Parent.Store)

local SettingsPanel = {}
SettingsPanel.__index = SettingsPanel

-- Palet.
--
-- Iki zemin tonu var: panelin arkasi KOYU, kartlar bir ton isOpen. Ayrimi
-- cizgiyle degil tonla yapmak, kucuk bir panelde daha az gurultu uretiyor.
-- Mavi, logodaki maviyle ayni (#4C8DF5) — panel, ikon ve magaza girdisi
-- tek bir renge dayaniyor.
local COLOR = {
	bg     = Color3.fromRGB(22, 24, 29),
	card     = Color3.fromRGB(30, 33, 40),
	box     = Color3.fromRGB(41, 45, 54),
	stroke    = Color3.fromRGB(52, 57, 68),
	ink     = Color3.fromRGB(232, 234, 240),
	muted    = Color3.fromRGB(138, 146, 166),
	green    = Color3.fromRGB(58, 176, 106),
	yellow     = Color3.fromRGB(214, 162, 54),
	red  = Color3.fromRGB(214, 88, 88),
	blue     = Color3.fromRGB(76, 141, 245),
}

-- Bosluk olcegi. Elle piksel yazmak yerine buradan seciliyor; panelin her
-- yerinde ayni ritim olusuyor.
local SPACING = { narrow = 6, medium = 10, wide = 14 }

-- Arac cubugu ikonu.
--
-- Roblox plugin dugmesine ikon koymanin tek yolu, gorseli Roblox'a asset
-- olarak yuklemek: yerel bir dosya kullanilamiyor. Kaynagi
-- vscode-extension/resources/logo.png; ayni marker kenar cubugu ikonunda ve
-- magaza girdisinde de kullaniliyor.
--
-- Once Roblox'un yerlesik ROBUX ikonu vardi (urunle ilgisi yoktu), sonra bos
-- dize denendi ve Studio onu "yuklenemedi" sayip baklava seklinde bir yer
-- tutucu gosterdi.
local ICON = "rbxassetid://73929349055328"

function SettingsPanel.new()
	return setmetatable({}, SettingsPanel)
end

function SettingsPanel:OnStart(container)
	self.plugin = container:Get("Plugin").ref
	self.connectionManager = container:Get("ConnectionManager")
	self.activityLog = container:Get("ActivityLog")
	-- Panelde iki view var: "projeler" (hangi core'lar isRunning) ve
	-- "akis" (Syncix ne degistirdi). Varsayilan flow, cunku asil eksik oydu:
	-- degisiklikler tamamen sessiz uygulaniyordu.
	self.view = "activity"

	if not self.plugin then
		return
	end

	-- Arac cubugu, panelin TAMAMEN DISINDA tutuluyor ve her sey pcall icinde.
	--
	-- Burasi StartAll icinden cagriliyor; buradaki bir failure butun eklentiyi
	-- baslatmadan dusuruyor. Ikon denemesi yuzunden senkronun hic calismamasi
	-- kabul edilemez; basarisizlikta yalnizca button eksik kalir.
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
		self.button = toolbar:CreateButton("Syncix", "Syncix status and port settings", ICON)
	end) then
		-- Ikon yuklenemezse button yine de olusmali; senkron ikona bagli degil.
		pcall(function()
			self.button = toolbar:CreateButton("Syncix", "Syncix status and port settings", "")
		end)
	end
	if not self.button then
		warn("[Syncix] Toolbar button could not be created; sync still works.")
		return
	end

	self.button.ClickableWhenViewportHidden = true

	self.button.Click:Connect(function()
		self:Toggle()
	end)
end

-- ---------------------------------------------------------------------------
-- KUCUK BIR TASARIM DUZENI
--
-- Eski panel her ogeyi elle piksel konumuna koyuyordu (y = 10, 32, 96, 154...).
-- Iki sorunu vardi: bir oge buyudugunde altindakiler ustune biniyordu ve
-- genislikler sabit oldugu icin narrow panelde tasiyordu (330 + 74 = 404 piksel,
-- panelin narrow hali 380).
--
-- Artik dikey flow UIListLayout ile, yatay yerlesim ORANLA yapiliyor. Hicbir
-- yerde elle Y konumu yok; ogeler kendi boylarini soyluyor, duzen siralamayi
-- hallediyor.
-- ---------------------------------------------------------------------------

local function corner(parentNode, radius)
	local k = Instance.new("UICorner")
	k.CornerRadius = UDim.new(0, radius or 6)
	k.Parent = parentNode
	return k
end

local function verticalFlow(parentNode, gap)
	local d = Instance.new("UIListLayout")
	d.FillDirection = Enum.FillDirection.Vertical
	d.SortOrder = Enum.SortOrder.LayoutOrder
	d.Padding = UDim.new(0, gap or SPACING.medium)
	d.Parent = parentNode
	return d
end

local function innerPadding(parentNode, datum)
	local b = Instance.new("UIPadding")
	local u = UDim.new(0, datum)
	b.PaddingTop = u
	b.PaddingBottom = u
	b.PaddingLeft = u
	b.PaddingRight = u
	b.Parent = parentNode
	return b
end

local function label(parentNode, text, color, size, bold)
	local l = Instance.new("TextLabel")
	l.BackgroundTransparency = 1
	l.Size = UDim2.new(1, 0, 0, 0)
	l.AutomaticSize = Enum.AutomaticSize.Y
	l.TextColor3 = color or COLOR.ink
	l.TextXAlignment = Enum.TextXAlignment.Left
	l.TextYAlignment = Enum.TextYAlignment.Top
	l.TextWrapped = true
	l.Font = bold and Enum.Font.GothamBold or Enum.Font.Gotham
	l.TextSize = size or 13
	l.Text = text
	l.Parent = parentNode
	return l
end

--- Bolum basligi: kucuk, buyuk harf, muted. Iceriginin onune gecmemeli.
local function title(parentNode, text, order)
	local l = label(parentNode, string.upper(text), COLOR.muted, 11, true)
	l.LayoutOrder = order
	return l
end

--- Icerigi gruplayan card. Panelin arkasindan bir ton isOpen.
local function card(parentNode, order)
	local k = Instance.new("Frame")
	k.BackgroundColor3 = COLOR.card
	k.BorderSizePixel = 0
	k.Size = UDim2.new(1, 0, 0, 0)
	k.AutomaticSize = Enum.AutomaticSize.Y
	k.LayoutOrder = order
	k.Parent = parentNode
	corner(k, 8)
	innerPadding(k, SPACING.medium)
	verticalFlow(k, SPACING.narrow)
	return k
end

--- Yatay row. Genislikler ORANLA veriliyor ki narrow panelde tasmasin.
local function row(parentNode, height, order)
	local r = Instance.new("Frame")
	r.BackgroundTransparency = 1
	r.Size = UDim2.new(1, 0, 0, height)
	r.LayoutOrder = order
	r.Parent = parentNode
	local d = Instance.new("UIListLayout")
	d.FillDirection = Enum.FillDirection.Horizontal
	d.SortOrder = Enum.SortOrder.LayoutOrder
	d.Padding = UDim.new(0, SPACING.narrow)
	d.Parent = r
	return r
end

-- Liste satirlarinin ICI mutlak yerlesim kullaniyor: her row sabit
-- yukseklikte ve icindeki uc field (direction, title, timestamp) hizali durmali.
-- Ust bolumdeki flow tabanli `etiket` bunun icin uygun degil, o yuzden
-- konumlu bir es var.
--
-- Bu ikisi bir sure YOKTU: `etiket`in imzasini degistirdim ama list
-- icindeki sekiz cagriyi guncellemeyi unuttum. UDim2 degerleri color
-- parametresine gitti ve panel her yenilenmede failure verdi.
local function boxLabel(parentNode, text, size, position, color, bold)
	local l = Instance.new("TextLabel")
	l.Size = size
	l.Position = position
	l.BackgroundTransparency = 1
	l.TextColor3 = color or COLOR.ink
	l.TextXAlignment = Enum.TextXAlignment.Left
	l.TextYAlignment = Enum.TextYAlignment.Top
	l.TextWrapped = true
	l.Font = bold and Enum.Font.GothamBold or Enum.Font.Gotham
	l.TextSize = bold and 12 or 11
	l.Text = text
	l.Parent = parentNode
	return l
end

local function boxButton(parentNode, text, size, position, color)
	local b = Instance.new("TextButton")
	b.Size = size
	b.Position = position
	b.BackgroundColor3 = color
	b.BorderSizePixel = 0
	b.AutoButtonColor = true
	b.TextColor3 = COLOR.ink
	b.Font = Enum.Font.GothamMedium
	b.TextSize = 11
	b.Text = text
	b.Parent = parentNode
	corner(b, 5)
	return b
end

local function makeButton(parentNode, text, ratio, color, order)
	local b = Instance.new("TextButton")
	b.Size = UDim2.new(ratio, -SPACING.narrow, 1, 0)
	b.BackgroundColor3 = color
	b.BorderSizePixel = 0
	b.AutoButtonColor = true
	b.TextColor3 = COLOR.ink
	b.Font = Enum.Font.GothamMedium
	b.TextSize = 12
	b.Text = text
	b.LayoutOrder = order
	b.Parent = parentNode
	corner(b, 6)
	return b
end

function SettingsPanel:Toggle()
	if self.gui then
		self.gui.Enabled = not self.gui.Enabled
		if self.gui.Enabled then
			self:Refresh()
		end
		return
	end

	local info = DockWidgetPluginGuiInfo.new(
		Enum.InitialDockState.Float,
		true, true,
		420, 520,
		360, 420
	)
	self.gui = self.plugin:CreateDockWidgetPluginGui("SyncixPanel", info)
	self.gui.Title = "Syncix"

	local frame = Instance.new("Frame")
	frame.Size = UDim2.new(1, 0, 1, 0)
	frame.BackgroundColor3 = COLOR.bg
	frame.BorderSizePixel = 0
	frame.Parent = self.gui

	-- Ust bolum: kartlar dikey akista.
	-- AutomaticSize sayesinde durum yazisi uzayinca card da uzuyor ve
	-- altindakiler kendiliginden asagi kayiyor.
	local parentNode = Instance.new("Frame")
	parentNode.BackgroundTransparency = 1
	parentNode.Size = UDim2.new(1, 0, 0, 0)
	parentNode.AutomaticSize = Enum.AutomaticSize.Y
	parentNode.Parent = frame
	innerPadding(parentNode, SPACING.wide)
	verticalFlow(parentNode, SPACING.medium)

	-- Kimlik satiri: logodaki blue kare + ad.
	local identity = row(parentNode, 18, 1)
	-- Logonun kendisi. Yuklenemezse (asset erisimi yoksa) arkasindaki blue
	-- kare gorunur kalir; panel ikona bagli degil.
	local marker = Instance.new("ImageLabel")
	marker.Size = UDim2.new(0, 16, 0, 16)
	marker.BackgroundColor3 = COLOR.blue
	marker.BackgroundTransparency = 0
	marker.BorderSizePixel = 0
	marker.Image = ICON
	marker.ScaleType = Enum.ScaleType.Fit
	marker.LayoutOrder = 1
	marker.Parent = identity
	corner(marker, 4)
	local ad = label(identity, "SYNCIX", COLOR.ink, 12, true)
	ad.AutomaticSize = Enum.AutomaticSize.None
	ad.Size = UDim2.new(1, -22, 1, 0)
	ad.TextYAlignment = Enum.TextYAlignment.Center
	ad.LayoutOrder = 2

	-- DURUM
	title(parentNode, "Status", 2)
	local statusCard = card(parentNode, 3)
	local statusRow = Instance.new("Frame")
	statusRow.BackgroundTransparency = 1
	statusRow.Size = UDim2.new(1, 0, 0, 0)
	statusRow.AutomaticSize = Enum.AutomaticSize.Y
	statusRow.Parent = statusCard

	-- Renkli nokta: durum rengini yazinin renginden ayirmak, "bagli" halinde
	-- metnin beyaz kalip yalnizca noktanin green olmasini sagliyor.
	self.statusDot = Instance.new("Frame")
	self.statusDot.Size = UDim2.new(0, 8, 0, 8)
	self.statusDot.Position = UDim2.new(0, 0, 0, 4)
	self.statusDot.BackgroundColor3 = COLOR.muted
	self.statusDot.BorderSizePixel = 0
	self.statusDot.Parent = statusRow
	corner(self.statusDot, 4)

	self.statusText = label(statusRow, "...", COLOR.ink, 12)
	self.statusText.Position = UDim2.new(0, 16, 0, 0)
	self.statusText.Size = UDim2.new(1, -16, 0, 0)

	-- BAGLANTI
	title(parentNode, "Connection", 4)
	local connectionCard = card(parentNode, 5)
	label(
		connectionCard,
		"Leave the port empty and Syncix finds the running core itself (8080-8089). Type a port to pin this window to one project.",
		COLOR.muted, 11
	)

	local portRow = row(connectionCard, 28, 2)
	self.portBox = Instance.new("TextBox")
	self.portBox.Size = UDim2.new(0.32, -SPACING.narrow, 1, 0)
	self.portBox.BackgroundColor3 = COLOR.box
	self.portBox.BorderSizePixel = 0
	self.portBox.TextColor3 = COLOR.ink
	self.portBox.PlaceholderText = "Automatic"
	self.portBox.PlaceholderColor3 = COLOR.muted
	self.portBox.Font = Enum.Font.Code
	self.portBox.TextSize = 13
	self.portBox.ClearTextOnFocus = false
	self.portBox.Text = ""
	self.portBox.LayoutOrder = 1
	self.portBox.Parent = portRow
	corner(self.portBox, 6)

	local applyFn = makeButton(portRow, "Apply", 0.38, COLOR.blue, 2)
	local cleanup = makeButton(portRow, "Automatic", 0.30, COLOR.box, 3)

	-- SENKRON
	title(parentNode, "Sync", 6)
	local syncRow = row(parentNode, 30, 7)
	self.pauseButton = makeButton(syncRow, "...", 0.5, COLOR.box, 1)
	self.permissionButton = makeButton(syncRow, "...", 0.5, COLOR.box, 2)

	self.pauseButton.Activated:Connect(function()
		self.connectionManager:SetPaused(not self.connectionManager:IsPaused())
		self:Refresh()
	end)
	self.permissionButton.Activated:Connect(function()
		local fresh = not self.connectionManager.askPermission
		self.connectionManager.askPermission = fresh
		Store.Set(self.plugin, "syncix_ask_permission", fresh)
		print(string.format(
			"[Syncix] Ask for connection permission: %s",
			fresh and "ON (every new project must be approved)" or "OFF"
		))
		self:Refresh()
	end)

	-- SEKMELER
	local tabRow = row(parentNode, 26, 8)
	self.tabFlow = makeButton(tabRow, "Recent changes", 0.5, COLOR.box, 1)
	self.projectTab = makeButton(tabRow, "Projects", 0.5, COLOR.box, 2)
	self.tabFlow.Activated:Connect(function()
		self.view = "activity"
		self:Refresh()
	end)
	self.projectTab.Activated:Connect(function()
		self.view = "projects"
		self:Refresh()
	end)

	-- LISTE: kalan yuksekligi doldurur.
	self.list = Instance.new("ScrollingFrame")
	self.list.BackgroundColor3 = COLOR.card
	self.list.BorderSizePixel = 0
	self.list.ScrollBarThickness = 5
	self.list.ScrollBarImageColor3 = COLOR.stroke
	self.list.CanvasSize = UDim2.new(0, 0, 0, 0)
	self.list.Parent = frame
	corner(self.list, 8)

	-- Listenin yeri parentNode bolumun GERCEK yuksekligine gore ayarlanir.
	-- Sabit bir numValue yazmak, durum yazisi uzadiginda listenin ustune
	-- binmesine yol aciyordu.
	local function layoutList()
		local y = parentNode.AbsoluteSize.Y
		self.list.Position = UDim2.new(0, SPACING.wide, 0, y)
		self.list.Size = UDim2.new(1, -SPACING.wide * 2, 1, -y - SPACING.wide)
	end
	parentNode:GetPropertyChangedSignal("AbsoluteSize"):Connect(layoutList)
	layoutList()

	applyFn.Activated:Connect(function()
		self:ApplyPort(self.portBox.Text)
	end)
	cleanup.Activated:Connect(function()
		self.portBox.Text = ""
		self:ApplyPort("")
	end)

	-- Panel acikken durumu canli tut.
	--
	-- Refresh pcall icinde: burada olusan bir failure task'i olduruyordu ve panel
	-- bir daha HIC guncellenmiyordu. Sonuc yaniltiyordu — Output "Connected"
	-- derken panelde "Not connected" yaziyordu, cunku yazan kod artik
	-- calismiyordu. Hata bir kez bildirilir, dongu devam eder.
	task.spawn(function()
		local errorReported = false
		while self.gui do
			if self.gui.Enabled then
				local ok, failure = pcall(function()
					self:Refresh()
				end)
				if not ok and not errorReported then
					errorReported = true
					warn("[Syncix] Panel refresh failed: " .. tostring(failure))
				end
			end
			task.wait(2)
		end
	end)

	self:Refresh()
end

function SettingsPanel:ApplyPort(text: string)
	local clean = string.gsub(text or "", "%s", "")

	if clean == "" then
		Store.Set(self.plugin, "syncix_port", 0) -- 0 = otomatik
		self.connectionManager:SetManualPort(nil)
		print("[Syncix] Port: automatic. Looking for a running core...")
	else
		local numValue = tonumber(clean)
		if not numValue or numValue < 1 or numValue > 65535 or numValue ~= math.floor(numValue) then
			warn("[Syncix] Invalid port: " .. clean .. " (must be a whole number between 1 and 65535)")
			return
		end
		Store.Set(self.plugin, "syncix_port", numValue)
		self.connectionManager:SetManualPort(numValue)
		print(string.format("[Syncix] Port pinned to %d. Only this port will be tried.", numValue))
	end

	self.connectionManager:ForceReconnect()
	self:Refresh()
end

function SettingsPanel:Refresh()
	if not self.statusText then return end

	local cm = self.connectionManager
	local info = cm.serverInfo
	local manual = cm:GetManualPort()

	local portText = manual and tostring(manual) or "Automatic"
	if self.portBox and not self.portBox:IsFocused() then
		self.portBox.Text = manual and tostring(manual) or ""
	end

	local statusColor = COLOR.muted

	if cm:IsPaused() then
		statusColor = COLOR.yellow
		self.statusText.TextColor3 = COLOR.yellow
		self.statusText.Text =
			"Sync PAUSED\nNothing is sent to or applied from the editor.\n"
			.. "Press Resume sync to re-sync the full tree."
	elseif cm.state == "Connected" and info then
		-- Bagliyken ink BEYAZ kaliyor, yalnizca nokta green. Butun blogu
		-- yesile boyamak okunurlugu dusuruyordu.
		statusColor = COLOR.green
		self.statusText.TextColor3 = COLOR.ink
		self.statusText.Text = string.format(
			"Connected  •  port setting: %s\nProject: %s\nFolder: %s\nPort: %s   Core: %s",
			portText,
			tostring(info.project),
			tostring(info.root),
			tostring(info.port),
			tostring(info.version)
		)
	else
		self.statusText.TextColor3 = COLOR.muted
		self.statusText.Text = string.format(
			"Not connected  •  port setting: %s\nState: %s\n%s",
			portText,
			tostring(cm.state),
			manual and ("Only port " .. manual .. " is being tried.")
				or "Scanning ports 8080-8089."
		)
	end

	-- Cakisma varsa durum satirinda goster: sessizce ezilmis bir degisiklik
	-- kullanicinin haberi olmadan kaybolmasin.
	if self.activityLog then
		local summary = self.activityLog:Summary()
		if summary.conflict > 0 then
			statusColor = COLOR.yellow
			self.statusText.Text = self.statusText.Text
				.. string.format("\n%d conflict(s) — see the Recent changes tab", summary.conflict)
			self.statusText.TextColor3 = COLOR.yellow
		end
	end

	if self.statusDot then
		self.statusDot.BackgroundColor3 = statusColor
	end

	if self.pauseButton then
		local stopped = self.connectionManager:IsPaused()
		self.pauseButton.Text = stopped and "Resume sync" or "Pause sync"
		self.pauseButton.BackgroundColor3 = stopped and COLOR.yellow or COLOR.box
	end

	if self.permissionButton then
		local isOpen = self.connectionManager.askPermission == true
		self.permissionButton.Text = isOpen and "Ask permission: ON" or "Ask permission: OFF"
		self.permissionButton.BackgroundColor3 = isOpen and COLOR.green or COLOR.box
	end

	self:FillList()
end

--- Recent degisiklikler akisi.
---
--- Rojo'nun patch visualizer'i bagliniverince buyuk bir farki onaya sunar; biz
--- surekli ve cift yonlu calistigimiz icin onay istemek kullanilamaz olurdu.
--- Bunun yerine ne gelip ne gittigini geriye donuk gosteriyoruz. Cakisan
--- degisiklikler red isaretlenir.
function SettingsPanel:FillFlow()
	local entries = self.activityLog:Recent(40)

	if #entries == 0 then
		boxLabel(self.list, "No changes yet.\nChange something in Studio, or send a command from the editor.",
			UDim2.new(1, -16, 0, 40), UDim2.new(0, 8, 0, 8), COLOR.muted)
		self.list.CanvasSize = UDim2.new(0, 0, 0, 56)
		return
	end

	local now = os.clock()
	local y = 6
	for _, k in ipairs(entries) do
		local row = Instance.new("Frame")
		row.Size = UDim2.new(1, -12, 0, 34)
		row.Position = UDim2.new(0, 6, 0, y)
		row.BackgroundColor3 = k.conflict and Color3.fromRGB(70, 32, 32) or COLOR.bg
		row.BorderSizePixel = 0
		row.Parent = self.list
		local sk = Instance.new("UICorner")
		sk.CornerRadius = UDim.new(0, 4)
		sk.Parent = row

		local directionMark = (k.direction == "in") and "<-" or "->"
		local directionColor = k.conflict and COLOR.red
			or ((k.direction == "in") and COLOR.blue or COLOR.green)

		local ok = boxLabel(row, directionMark, UDim2.new(0, 24, 0, 16), UDim2.new(0, 8, 0, 4), directionColor, true)
		ok.TextXAlignment = Enum.TextXAlignment.Center

		local title = k.target
		if k.field then
			title = title .. "." .. k.field
		end
		boxLabel(row, title, UDim2.new(1, -110, 0, 16), UDim2.new(0, 36, 0, 3), COLOR.ink, true)

		local subItem = k.pass
		if k.datum and k.datum ~= "" then
			subItem = subItem .. "  =  " .. k.datum
		end
		if k.conflict then
			subItem = subItem .. "   [CAKISMA]"
		end
		boxLabel(row, subItem, UDim2.new(1, -110, 0, 14), UDim2.new(0, 36, 0, 18), k.conflict and COLOR.red or COLOR.muted)

		local elapsed = math.max(0, math.floor(now - k.timestamp))
		local timeText = (elapsed < 60) and (elapsed .. "s ago")
			or (math.floor(elapsed / 60) .. "m ago")
		local z = boxLabel(row, timeText, UDim2.new(0, 66, 0, 14), UDim2.new(1, -72, 0, 10), COLOR.muted)
		z.TextXAlignment = Enum.TextXAlignment.Right

		y += 38
	end
	self.list.CanvasSize = UDim2.new(0, 0, 0, y)
end

function SettingsPanel:FillList()
	if not self.list then return end

	for _, c in ipairs(self.list:GetChildren()) do
		if not c:IsA("UICorner") then
			c:Destroy()
		end
	end

	-- Sekme gorunumleri
	if self.tabFlow then
		self.tabFlow.BackgroundColor3 = (self.view == "activity") and COLOR.blue or COLOR.box
		self.projectTab.BackgroundColor3 = (self.view == "projects") and COLOR.blue or COLOR.box
	end

	if self.view == "activity" then
		self:FillFlow()
		return
	end

	-- Proje taramasi ON portu tek tek yokluyor; panel her 2 saniyede bir
	-- yenilendigi icin bu, saniyede bes HTTP istegi demekti ve baglantiyi
	-- calkantiya sokuyordu (Output'ta surekli Connected -> Disconnected).
	-- Sonuc onbelleklenip en fazla 10 saniyede bir tazeleniyor.
	local scanNow = os.clock()
	if not self.projectCache or (scanNow - (self.projectCacheTime or 0)) > 10 then
		self.projectCache = self.connectionManager:ScanAllPorts()
		self.projectCacheTime = scanNow
	end
	local foundList = self.projectCache
	if #foundList == 0 then
		boxLabel(self.list, "No running core found.\nOpen the project in VS Code, or run: syncix up",
			UDim2.new(1, -16, 0, 40), UDim2.new(0, 8, 0, 8), COLOR.muted)
		self.list.CanvasSize = UDim2.new(0, 0, 0, 56)
		return
	end

	local y = 6
	for _, b in ipairs(foundList) do
		local row = Instance.new("Frame")
		row.Size = UDim2.new(1, -12, 0, 52)
		row.Position = UDim2.new(0, 6, 0, y)
		row.BackgroundColor3 = COLOR.bg
		row.BorderSizePixel = 0
		row.Parent = self.list
		local sk = Instance.new("UICorner")
		sk.CornerRadius = UDim.new(0, 4)
		sk.Parent = row

		boxLabel(row, string.format("%s   (port %d)", tostring(b.project), b.port),
			UDim2.new(1, -80, 0, 18), UDim2.new(0, 8, 0, 6), COLOR.ink, true)
		boxLabel(row, tostring(b.root),
			UDim2.new(1, -80, 0, 24), UDim2.new(0, 8, 0, 24), COLOR.muted)

		local pick = boxButton(row, "Select", UDim2.new(0, 56, 0, 24), UDim2.new(1, -64, 0, 14), COLOR.blue)
		local port = b.port
		pick.Activated:Connect(function()
			self.portBox.Text = tostring(port)
			self:ApplyPort(tostring(port))
		end)

		y += 58
	end
	self.list.CanvasSize = UDim2.new(0, 0, 0, y)
end

return SettingsPanel
