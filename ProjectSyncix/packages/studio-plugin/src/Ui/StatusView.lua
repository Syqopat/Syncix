--!strict
-- StatusView
--
-- The answer to "is it connected, and to what". One card for the connection, one for
-- what the core reports about the project, and the two actions that belong here.

local Theme = require(script.Parent.Theme)
local Widgets = require(script.Parent.Widgets)

local StatusView = {}
StatusView.__index = StatusView

local STATE_WORDS = {
	Connected = "Connected",
	Connecting = "Connecting",
	Disconnected = "Not connected",
	Paused = "Paused",
}

local function stateColour(state: string, paused: boolean): Color3
	if paused then
		return Theme.warn
	end
	if state == "Connected" then
		return Theme.ok
	end
	if state == "Connecting" then
		return Theme.warn
	end
	return Theme.bad
end

function StatusView.new(connection, pluginVersion: string)
	return setmetatable({ connection = connection, pluginVersion = pluginVersion }, StatusView)
end

function StatusView:Build(parent: Instance)
	local column = Widgets.column(parent, Theme.space.medium)
	self.root = column

	local connectionCard = Widgets.card(column, 1)
	local body = Widgets.column(connectionCard, Theme.space.small)
	Widgets.padding(body)

	local stateRow = Widgets.row(body, 20, 1)
	self.dot = Widgets.dot(stateRow, Theme.bad)
	self.stateLabel = Widgets.label(stateRow, "…", {
		font = Theme.font.bold,
		size = Theme.size.title,
		size2 = UDim2.new(1, -16, 1, 0),
		position = UDim2.new(0, 16, 0, 0),
	})

	self.projectLabel = Widgets.label(Widgets.row(body, 18, 2), "", {
		colour = Theme.subText(),
	})
	self.portLabel = Widgets.label(Widgets.row(body, 18, 3), "", {
		colour = Theme.subText(),
		size = Theme.size.small,
	})

	local actions = Widgets.row(body, Theme.rowHeight, 4)
	self.reconnectButton = Widgets.button(actions, "Reconnect", {
		primary = true,
		size = UDim2.new(0.5, -3, 1, 0),
	})
	self.pauseButton = Widgets.button(actions, "Pause", {
		size = UDim2.new(0.5, -3, 1, 0),
		position = UDim2.new(0.5, 3, 0, 0),
	})

	self.reconnectButton.MouseButton1Click:Connect(function()
		self.connection:ForceReconnect()
		self:Refresh()
	end)
	self.pauseButton.MouseButton1Click:Connect(function()
		self.connection:SetPaused(not self.connection:IsPaused())
		self:Refresh()
	end)

	-- What the core says about the project it serves. It is read-only on purpose:
	-- syncix.toml is the single source of truth, and two places to change one setting
	-- is how "I set it and nothing happened" starts.
	local projectCard = Widgets.card(column, 2)
	local details = Widgets.column(projectCard, Theme.space.tight)
	Widgets.padding(details)
	Widgets.label(Widgets.row(details, 18, 1), "PROJECT", {
		font = Theme.font.bold,
		size = Theme.size.small,
		colour = Theme.subText(),
	})
	self.detailLabels = {}
	for index, name in ipairs({ "Sync mode", "Objects", "Queue", "Conflicts", "Versions" }) do
		local row = Widgets.row(details, 18, index + 1)
		Widgets.label(row, name, {
			colour = Theme.subText(),
			size = Theme.size.small,
			size2 = UDim2.new(0.45, 0, 1, 0),
		})
		self.detailLabels[name] = Widgets.label(row, "-", {
			size = Theme.size.small,
			size2 = UDim2.new(0.55, 0, 1, 0),
			position = UDim2.new(0.45, 0, 0, 0),
		})
	end

	return column
end

function StatusView:Refresh()
	if not self.root then
		return
	end

	local connection = self.connection
	local info = connection.serverInfo
	local paused = connection:IsPaused()
	local state = paused and "Paused" or (connection.state or "Disconnected")

	self.dot.BackgroundColor3 = stateColour(connection.state or "", paused)
	self.stateLabel.Text = STATE_WORDS[state] or state
	self.stateLabel.TextColor3 = Theme.text()
	self.pauseButton.Text = paused and "Resume" or "Pause"

	self.projectLabel.Text = info and (info.project or "?") or "No core found"
	self.projectLabel.TextColor3 = Theme.subText()
	self.portLabel.Text = info
		and string.format("port %d  ·  core %s  ·  up %ds", info.port or 0,
			tostring(info.version), math.floor(info.uptime_seconds or 0))
		or string.format("scanning 8080-8089  ·  plugin %s", self.pluginVersion)
	self.portLabel.TextColor3 = Theme.subText()

	local function set(name: string, value: string)
		local label = self.detailLabels[name]
		if label then
			label.Text = value
			label.TextColor3 = Theme.text()
		end
	end

	if info then
		local config = info.config or {}
		set("Sync mode", tostring(config.mode or "?"))
		set("Objects", tostring(info.object_count or info.objects or "-"))
		set("Queue", string.format("%d queued  ·  %d merged",
			info.plugin_queued or 0, info.plugin_coalesced or 0))
		set("Conflicts", tostring(info.conflicts or 0))
		set("Versions", string.format("core %s  ·  plugin %s",
			tostring(info.version), self.pluginVersion))
	else
		for _, name in ipairs({ "Sync mode", "Objects", "Queue", "Conflicts" }) do
			set(name, "-")
		end
		set("Versions", string.format("plugin %s", self.pluginVersion))
	end
end

return StatusView
