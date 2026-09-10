-- SettingsPanel
-- The Syncix button on Studio's toolbar and the settings/status panel.
--
-- The main job here is PORT SELECTION. The plugin normally scans 8080-8089 and connects
-- to the first core that answers. With two projects open at once this can connect to the
-- WRONG project. When a number is typed into the port field, scanning stops and only that
-- port is tried; if Syncix is not there, it does not connect. That way "this Studio window
-- connects to that project" can be stated for certain.
--
-- The default is deliberately "Automatic", not 8080: if the requested port is taken the
-- core may move to 8081 by itself; with a fixed 8080 in the field you would never
-- connect in that case.

local Store = require(script.Parent.Store)

local SettingsPanel = {}
SettingsPanel.__index = SettingsPanel

-- Palette.
--
-- There are two background tones: the panel's back is DARK, cards are one tone lighter.
-- Separating with tone rather than lines produces less noise in a small panel.
-- The blue is the same as the logo's (#4C8DF5) — the panel, the icon and the store listing
-- rest on one colour.
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

-- Spacing scale. Chosen from here instead of typing pixels by hand, so the whole panel
-- shares one rhythm.
local SPACING = { narrow = 6, medium = 10, wide = 14 }

-- Toolbar icon.
--
-- The only way to put an icon on a Roblox plugin button is to upload the image to Roblox
-- as an asset: a local file cannot be used. Its source is
-- vscode-extension/resources/logo.png; the same mark is used for the sidebar icon and
-- the store listing.
--
-- First it was Roblox's built-in ROBUX icon (nothing to do with the product), then an empty
-- string was tried and Studio treated it as "failed to load" and showed a diamond-shaped
-- placeholder.
local ICON = "rbxassetid://73929349055328"

function SettingsPanel.new()
	return setmetatable({}, SettingsPanel)
end

function SettingsPanel:OnStart(container)
	self.plugin = container:Get("Plugin").ref
	self.connectionManager = container:Get("ConnectionManager")
	self.activityLog = container:Get("ActivityLog")
	-- The panel has two views: "projects" (which cores are running) and
	-- "activity" (what Syncix changed). Activity is the default, because that was what was really
	-- missing: changes were applied completely silently.
	self.view = "activity"

	if not self.plugin then
		return
	end

	-- The toolbar is kept COMPLETELY OUTSIDE the panel and everything runs in pcall.
	--
	-- This is called from inside StartAll; an error here stops the whole plugin
	-- from starting. Sync never working because of an icon attempt is
	-- unacceptable; on failure only the button is missing.
	--
	-- Icon: first it was Roblox's built-in ROBUX icon (nothing to do with the product),
	-- then an empty string was tried and Studio treated it as "failed to load" and showed a
	-- diamond-shaped placeholder. A call without arguments is tried first; if this Studio version
	-- does not support it, it falls back to the empty-string version.
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
		-- Even if the icon fails to load the button must still be created; sync does not depend on the icon.
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
-- A SMALL DESIGN SYSTEM
--
-- The old panel placed every element at hand-picked pixel positions (y = 10, 32, 96, 154...).
-- That had two problems: when an element grew, the ones below overlapped it, and
-- because widths were fixed they overflowed in a narrow panel (330 + 74 = 404 pixels,
-- while the panel's narrow state is 380).
--
-- Now the vertical flow uses UIListLayout and horizontal placement uses RATIOS. There is no
-- hand-written Y position anywhere; elements report their own heights and the layout
-- handles the ordering.
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

--- Card that groups content. One tone lighter than the panel's back.
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

--- Horizontal row. Widths are given as RATIOS so it does not overflow in a narrow panel.
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

-- The INSIDE of list rows uses absolute placement: every row has a fixed
-- height and its three fields (direction, title, timestamp) must line up.
-- The flow-based `label` of the upper part does not suit this, so there is
-- a positioned counterpart.
--
-- For a while these two DID NOT EXIST: the signature of `label` was changed but the eight
-- calls inside the list were not updated. UDim2 values went into the colour
-- parameter and the panel threw an error on every refresh.
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

	-- Upper part: cards in a vertical flow.
	-- Thanks to AutomaticSize, when the status text grows the card grows and
	-- the ones below move down by themselves.
	local parentNode = Instance.new("Frame")
	parentNode.BackgroundTransparency = 1
	parentNode.Size = UDim2.new(1, 0, 0, 0)
	parentNode.AutomaticSize = Enum.AutomaticSize.Y
	parentNode.Parent = frame
	innerPadding(parentNode, SPACING.wide)
	verticalFlow(parentNode, SPACING.medium)

	-- Identity line: the blue square in the logo + name.
	local identity = row(parentNode, 18, 1)
	-- The logo itself. If it fails to load (no asset access), the blue square behind it
	-- stays visible; the panel does not depend on the icon.
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
	local titleLabel = label(identity, "SYNCIX", COLOR.ink, 12, true)
	titleLabel.AutomaticSize = Enum.AutomaticSize.None
	titleLabel.Size = UDim2.new(1, -22, 1, 0)
	titleLabel.TextYAlignment = Enum.TextYAlignment.Center
	titleLabel.LayoutOrder = 2

	-- STATUS
	title(parentNode, "Status", 2)
	local statusCard = card(parentNode, 3)
	local statusRow = Instance.new("Frame")
	statusRow.BackgroundTransparency = 1
	statusRow.Size = UDim2.new(1, 0, 0, 0)
	statusRow.AutomaticSize = Enum.AutomaticSize.Y
	statusRow.Parent = statusCard

	-- Coloured dot: separating the status colour from the text colour keeps the text white
	-- when connected, with only the dot green.
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

	-- CONNECTION
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

	-- SYNC
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

	-- TABS
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

	-- LIST: fills remaining height.
	self.list = Instance.new("ScrollingFrame")
	self.list.BackgroundColor3 = COLOR.card
	self.list.BorderSizePixel = 0
	self.list.ScrollBarThickness = 5
	self.list.ScrollBarImageColor3 = COLOR.stroke
	self.list.CanvasSize = UDim2.new(0, 0, 0, 0)
	self.list.Parent = frame
	corner(self.list, 8)

	-- The list's position follows the upper part's REAL height.
	-- Writing a fixed number made the list overlap it
	-- whenever the status text grew.
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

	-- Keep the status live while the panel is open.
	--
	-- Refresh runs in pcall: an error here used to kill the task and the panel
	-- NEVER updated again. The result was misleading — Output said "Connected"
	-- while the panel said "Not connected", because the code writing it was no longer
	-- running. The error is reported once and the loop continues.
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
		Store.Set(self.plugin, "syncix_port", 0) -- 0 = automatic
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
		-- When connected the text stays WHITE and only the dot is green. Painting the whole block
		-- green made it harder to read.
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

	-- Show conflicts in the status line: a silently overwritten change must not
	-- disappear without the user knowing.
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

--- Recent changes feed.
---
--- Rojo's patch visualizer presents a large diff for approval on connect; we
--- work continuously and two-way, so asking for approval would be unusable.
--- Instead we show afterwards what came in and what went out. Conflicting
--- changes are marked red.
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
			subItem = subItem .. "   [CONFLICT]"
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

	-- Tab views
	if self.tabFlow then
		self.tabFlow.BackgroundColor3 = (self.view == "activity") and COLOR.blue or COLOR.box
		self.projectTab.BackgroundColor3 = (self.view == "projects") and COLOR.blue or COLOR.box
	end

	if self.view == "activity" then
		self:FillFlow()
		return
	end

	-- The project scan probes TEN ports one by one; since the panel refreshes every
	-- 2 seconds, that meant five HTTP requests per second and made the
	-- connection churn (Output kept printing Connected -> Disconnected).
	-- The result is cached and refreshed at most every 10 seconds.
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
