--!strict
-- Panel
--
-- The Syncix button on Studio's toolbar and the window behind it: three tabs, one
-- refresh loop, Studio's own colours.
--
-- The toolbar is created outside the window and every call is wrapped: this runs from
-- StartAll, and an error here would stop the whole plugin. Sync must never fail
-- because an icon did not load.

local Theme = require(script.Parent.Theme)
local Widgets = require(script.Parent.Widgets)
local StatusView = require(script.Parent.StatusView)
local StreamView = require(script.Parent.StreamView)
local SettingsView = require(script.Parent.SettingsView)

local Panel = {}
Panel.__index = Panel

-- A plugin button can only wear an uploaded asset; the mark is the one from the
-- extension's logo, so the panel, the icon and the store listing share it.
local ICON = "rbxassetid://73929349055328"
local REFRESH_SECONDS = 1

local TABS = { "Status", "Live", "Settings" }

function Panel.new()
	return setmetatable({ tabButtons = {}, views = {} }, Panel)
end

function Panel:OnStart(container)
	self.plugin = container:Get("Plugin").ref
	self.connection = container:Get("ConnectionManager")
	self.activityLog = container:Get("ActivityLog")
	local okVersion, versionEntry = pcall(function()
		return container:Get("Version")
	end)
	self.version = (okVersion and versionEntry and versionEntry.value) or "0.1.7"

	if not self.plugin then
		return
	end

	local toolbar
	if not pcall(function()
		toolbar = self.plugin:CreateToolbar("Syncix")
	end) or not toolbar then
		warn("[Syncix] Toolbar could not be created; sync still works.")
		return
	end

	if not pcall(function()
		self.button = toolbar:CreateButton("Syncix", "Syncix status, live changes and settings", ICON)
	end) then
		pcall(function()
			self.button = toolbar:CreateButton("Syncix", "Syncix status, live changes and settings", "")
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

function Panel:Toggle()
	if not self.widget then
		self:Create()
	end
	if not self.widget then
		return
	end
	self.widget.Enabled = not self.widget.Enabled
	if self.button then
		self.button:SetActive(self.widget.Enabled)
	end
	if self.widget.Enabled then
		self:Refresh()
	end
end

function Panel:Create()
	local info = DockWidgetPluginGuiInfo.new(Enum.InitialDockState.Right, false, false, 340, 460, 300, 360)
	local ok = pcall(function()
		self.widget = self.plugin:CreateDockWidgetPluginGui("SyncixPanel", info)
	end)
	if not ok or not self.widget then
		warn("[Syncix] The panel could not be created; sync still works.")
		return
	end

	self.widget.Title = "Syncix"
	self.widget.ZIndexBehavior = Enum.ZIndexBehavior.Sibling

	self.background = Instance.new("Frame")
	self.background.Size = UDim2.new(1, 0, 1, 0)
	self.background.BackgroundColor3 = Theme.background()
	self.background.BorderSizePixel = 0
	self.background.Parent = self.widget

	local shell = Instance.new("Frame")
	shell.Size = UDim2.new(1, 0, 1, 0)
	shell.BackgroundTransparency = 1
	shell.Parent = self.background
	Widgets.padding(shell, Theme.space.medium)

	self:BuildTabs(shell)

	self.body = Instance.new("Frame")
	self.body.Size = UDim2.new(1, 0, 1, -(Theme.rowHeight + Theme.space.medium))
	self.body.Position = UDim2.new(0, 0, 0, Theme.rowHeight + Theme.space.medium)
	self.body.BackgroundTransparency = 1
	self.body.Parent = shell

	local scroll = Widgets.scroll(self.body)

	self.views.Status = StatusView.new(self.connection, self.version)
	self.views.Live = StreamView.new(self.activityLog)
	self.views.Settings = SettingsView.new(self.connection, nil, self.plugin)

	self.roots = {
		Status = self.views.Status:Build(scroll),
		Live = self.views.Live:Build(scroll),
		Settings = self.views.Settings:Build(scroll),
	}

	self:Show("Status")
	self:StartRefreshing()

	-- Studio's theme can change while the panel is open; repainting is cheaper than
	-- keeping a second palette for the light theme.
	Theme.onChanged(function()
		self:Repaint()
	end)
end

function Panel:BuildTabs(parent: Instance)
	local bar = Instance.new("Frame")
	bar.Size = UDim2.new(1, 0, 0, Theme.rowHeight)
	bar.BackgroundTransparency = 1
	bar.Parent = parent

	local width = 1 / #TABS
	for index, name in ipairs(TABS) do
		local button = Widgets.button(bar, name, {
			size = UDim2.new(width, -4, 1, 0),
			position = UDim2.new(width * (index - 1), index == 1 and 0 or 4, 0, 0),
		})
		button.MouseButton1Click:Connect(function()
			self:Show(name)
		end)
		self.tabButtons[name] = button
	end
end

function Panel:Show(name: string)
	self.current = name
	for tab, root in pairs(self.roots or {}) do
		root.Visible = tab == name
	end
	for tab, button in pairs(self.tabButtons) do
		local active = tab == name
		button.BackgroundColor3 = active and Theme.accent or Theme.colour("Button")
		button.TextColor3 = active and Color3.new(1, 1, 1) or Theme.text()
	end
	self:Refresh()
end

function Panel:Refresh()
	local view = self.views[self.current or "Status"]
	if view then
		view:Refresh()
	end
end

--[[
	The panel polls instead of subscribing: the numbers it shows (uptime, queue length,
	the log) change on their own, and one refresh a second while the window is open is
	cheaper than keeping listeners on every one of them.
]]
function Panel:StartRefreshing()
	task.spawn(function()
		while self.widget do
			if self.widget.Enabled then
				local ok, err = pcall(function()
					self:Refresh()
				end)
				if not ok then
					warn("[Syncix] Panel refresh failed: " .. tostring(err))
				end
			end
			task.wait(REFRESH_SECONDS)
		end
	end)
end

function Panel:Repaint()
	if not self.widget then
		return
	end
	self.background.BackgroundColor3 = Theme.background()
	self:Show(self.current or "Status")
end

return Panel
