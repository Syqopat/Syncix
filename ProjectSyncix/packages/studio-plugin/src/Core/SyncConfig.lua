-- The plugin's runtime settings.
--
-- The single source of truth is syncix.toml. The core sends these settings in its /health
-- response; the plugin reads them on connect and writes them here.
--
-- Why the plugin keeps no settings of its own: with two separate sets of settings (one
-- in the panel, one in the file) it would be unclear which applies. A user who
-- wrote "disk_to_studio" in the file and still saw two-way behaviour in Studio
-- would have a very hard time finding out why. So the file wins.
--
-- The one exception is Pause/Resume: that is not a setting but momentary state. It stays in the panel.

local SyncConfig = {}

-- Defaults that apply until a connection is made. As soon as the core connects,
-- Apply() overwrites them.
local current = {
	mode = "two_way",
	ask_permission = false,
	undo = true,
	services = {},
	ignore_classes = {},
	ignore_properties = {},
}

-- A set instead of a list: scanning a list on every property change would be expensive.
local ignoredClasses = {}
local ignoredProperties = {}

local function toSet(list: { string }?): { [string]: boolean }
	local lookupSet = {}
	for _, fieldName in ipairs(list or {}) do
		lookupSet[fieldName] = true
	end
	return lookupSet
end

--- Applies the settings from the core. A missing field keeps its current value:
--- resetting settings when connecting to an older core would be wrong.
function SyncConfig.Apply(incoming: any)
	if type(incoming) ~= "table" then
		return
	end
	for keyName, datum in pairs(incoming) do
		if current[keyName] ~= nil then
			current[keyName] = datum
		end
	end
	ignoredClasses = toSet(current.ignore_classes)
	ignoredProperties = toSet(current.ignore_properties)
end

function SyncConfig.Mode(): string
	return current.mode
end

--- Should a change made in Studio be sent to the core?
--- In disk_to_studio mode Studio only receives; the observer must not speak at all.
function SyncConfig.SendFromStudio(): boolean
	return current.mode == "two_way" or current.mode == "studio_to_disk"
end

--- Should a change from the core be applied in Studio?
function SyncConfig.ApplyToStudio(): boolean
	return current.mode == "two_way" or current.mode == "disk_to_studio"
end

function SyncConfig.UndoEnabled(): boolean
	return current.undo ~= false
end

function SyncConfig.AskPermission(): boolean
	return current.ask_permission == true
end

--- An empty list means "use the default"; it does not mean the user wants no
--- services. Leaving it empty is the most common case, so treating it
--- as "none" would break sync for everyone who opens the setting.
function SyncConfig.ServiceList(): { string }?
	if #current.services == 0 then
		return nil
	end
	return current.services
end

function SyncConfig.ClassAllowed(cls: string): boolean
	return not ignoredClasses[cls]
end

function SyncConfig.PropertyAllowed(fieldName: string): boolean
	return not ignoredProperties[fieldName]
end

return SyncConfig
