--!strict
-- SettingsView
--
-- The one setting that belongs in Studio rather than in syncix.toml -- which core this
-- window talks to -- and the way to send feedback.
--
-- Everything else about a project (sync mode, trash, ignored classes) is read from
-- syncix.toml through the core. Two places to change one setting is how "I changed it
-- and nothing happened" begins.

local GuiService = game:GetService("GuiService")

local Theme = require(script.Parent.Theme)
local Widgets = require(script.Parent.Widgets)

local SettingsView = {}
SettingsView.__index = SettingsView

local FEEDBACK_URL = "https://github.com/Syqopat/Syncix/issues/new/choose"

function SettingsView.new(connection, store, pluginRef)
	return setmetatable({ connection = connection, store = store, plugin = pluginRef }, SettingsView)
end

function SettingsView:Build(parent: Instance)
	local column = Widgets.column(parent, Theme.space.medium)
	self.root = column

	-- Port
	local portCard = Widgets.card(column, 1)
	local port = Widgets.column(portCard, Theme.space.small)
	Widgets.padding(port)
	Widgets.label(Widgets.row(port, 18, 1), "CORE PORT", {
		font = Theme.font.bold,
		size = Theme.size.small,
		colour = Theme.subText(),
	})
	Widgets.label(Widgets.row(port, 30, 2), "Automatic scans 8080-8089 and takes the first core that answers. Set a port when two projects are open and this window has to talk to one of them.", {
		size = Theme.size.small,
		colour = Theme.subText(),
		size2 = UDim2.new(1, 0, 1, 0),
	})

	local portRow = Widgets.row(port, Theme.rowHeight, 3)
	self.portField = Widgets.field(portRow, "Automatic", {
		size = UDim2.new(0.42, -3, 1, 0),
	})
	self.applyButton = Widgets.button(portRow, "Use port", {
		primary = true,
		size = UDim2.new(0.29, -3, 1, 0),
		position = UDim2.new(0.42, 3, 0, 0),
	})
	self.autoButton = Widgets.button(portRow, "Automatic", {
		size = UDim2.new(0.29, 0, 1, 0),
		position = UDim2.new(0.71, 3, 0, 0),
	})
	self.portNote = Widgets.label(Widgets.row(port, 18, 4), "", {
		size = Theme.size.small,
		colour = Theme.subText(),
	})

	self.applyButton.MouseButton1Click:Connect(function()
		self:ApplyPort(self.portField.Text)
	end)
	self.autoButton.MouseButton1Click:Connect(function()
		self.portField.Text = ""
		self:ApplyPort("")
	end)

	-- Feedback
	local feedbackCard = Widgets.card(column, 2)
	local feedback = Widgets.column(feedbackCard, Theme.space.small)
	Widgets.padding(feedback)
	Widgets.label(Widgets.row(feedback, 18, 1), "FEEDBACK", {
		font = Theme.font.bold,
		size = Theme.size.small,
		colour = Theme.subText(),
	})
	Widgets.label(Widgets.row(feedback, 30, 2), "Something broken, missing or slow? The form opens in your browser and nothing is sent from Studio.", {
		size = Theme.size.small,
		colour = Theme.subText(),
		size2 = UDim2.new(1, 0, 1, 0),
	})
	self.feedbackButton = Widgets.button(feedback, "Open the feedback form", {
		primary = true,
		order = 3,
	})
	self.feedbackLink = Widgets.field(feedback, FEEDBACK_URL, {
		text = FEEDBACK_URL,
		order = 4,
	})
	self.feedbackLink.TextEditable = false
	self.feedbackNote = Widgets.label(Widgets.row(feedback, 18, 5), "", {
		size = Theme.size.small,
		colour = Theme.subText(),
	})

	self.feedbackButton.MouseButton1Click:Connect(function()
		self:OpenFeedback()
	end)

	return column
end

--[[
	Studio has no promise that a plugin may open a browser, so the attempt is wrapped
	and the URL stays on screen to copy when the attempt is refused.
]]
function SettingsView:OpenFeedback()
	local opened = pcall(function()
		GuiService:OpenBrowserWindow(FEEDBACK_URL)
	end)
	if not opened and self.plugin then
		opened = pcall(function()
			self.plugin:OpenWikiPage(FEEDBACK_URL)
		end)
	end
	self.feedbackNote.Text = opened and "Opened in your browser."
		or "Studio would not open it — copy the link below."
	self.feedbackNote.TextColor3 = opened and Theme.ok or Theme.warn
	self.feedbackLink:CaptureFocus()
end

function SettingsView:ApplyPort(text: string)
	local trimmed = (text or ""):match("^%s*(.-)%s*$") or ""

	if trimmed == "" then
		self.connection:SetManualPort(nil)
		self.portNote.Text = "Automatic: the first core that answers."
		self.portNote.TextColor3 = Theme.subText()
	else
		local port = tonumber(trimmed)
		if not port or port < 1 or port > 65535 or port % 1 ~= 0 then
			self.portNote.Text = "A port is a whole number between 1 and 65535."
			self.portNote.TextColor3 = Theme.bad
			return
		end
		self.connection:SetManualPort(port)
		self.portNote.Text = string.format("Only port %d will be tried.", port)
		self.portNote.TextColor3 = Theme.subText()
	end

	self.connection:ForceReconnect()
end

function SettingsView:Refresh()
	if not self.root then
		return
	end
	local manual = self.connection:GetManualPort()
	if not self.portField:IsFocused() then
		self.portField.Text = manual and tostring(manual) or ""
	end
	if self.portNote.Text == "" then
		self.portNote.Text = manual
			and string.format("Only port %d will be tried.", manual)
			or "Automatic: the first core that answers."
		self.portNote.TextColor3 = Theme.subText()
	end
end

return SettingsView
