--!strict
-- SelectionObserver
--
-- Carries Studio's selection to the editor, and the editor's selection to Studio.
--
-- Why a separate observer: selection is not project CONTENT but momentary state.
-- Unlike the other observers it must not be written to the model or to disk. If a file
-- changed on every click, version control would drown in noise. So it travels on its own
-- channel, without touching the model at all.

local Selection = game:GetService("Selection")
local RunService = game:GetService("RunService")

local SyncConfig = require(script.Parent.Parent.Core.SyncConfig)

local SelectionObserver = {}
SelectionObserver.__index = SelectionObserver

function SelectionObserver.new()
	local self = setmetatable({}, SelectionObserver)
	-- While applying a selection from the editor, Studio fires SelectionChanged too.
	-- Without this flag that signal would be sent back to the editor and the two sides
	-- would keep triggering each other.
	self.applying = false
	return self
end

function SelectionObserver:OnStart(container)
	self.cache = container:Get("RuntimeCache")
	self.connectionManager = container:Get("ConnectionManager")

	Selection.SelectionChanged:Connect(function()
		self:SendFromStudio()
	end)
end

--- Reports what is selected in Studio to the editor.
function SelectionObserver:SendFromStudio()
	if self.applying then return end
	if RunService:IsRunning() then return end
	if not SyncConfig.SendFromStudio() then return end
	if not self.connectionManager then return end

	local identities = {}
	for _, object in ipairs(Selection:Get()) do
		-- Objects without a UUID are outside sync (e.g. Camera); they are skipped.
		local uuid = object:GetAttribute("__syncix_id")
		if uuid then
			table.insert(identities, tostring(uuid))
		end
	end

	self.connectionManager:Send({
		event_type = "SELECTION",
		version = "v1",
		data = { ids = identities, source = "studio" },
	})
end

--- Applies a selection from the editor in Studio.
function SelectionObserver:Apply(identities: { string })
	if not self.cache then return end

	local objects = {}
	for _, uuid in ipairs(identities or {}) do
		local object = self.cache:GetInstance(uuid)
		-- An identity that was deleted or never arrived is skipped silently: a partial
		-- selection is better than none.
		if object and object.Parent then
			table.insert(objects, object)
		end
	end

	self.applying = true
	pcall(function()
		Selection:Set(objects)
	end)
	-- SelectionChanged may arrive deferred; the flag is released a frame later
	-- so the selection we wrote ourselves is not sent back.
	task.defer(function()
		self.applying = false
	end)
end

return SelectionObserver
