-- The SINGLE source of the synced services.
--
-- Why a separate file: this list was written by hand twice, in GenericObserver and
-- PatchBuilder. When a service was added to one and not the other, the object showed
-- in the tree but its changes were not observed — a silent failure that was hard to diagnose.
--
-- Fixed UUIDs: a service's identity must stay the same when the core restarts,
-- otherwise the tree's roots change on every start and the whole subtree is rewritten.

local SyncConfig = require(script.Parent.Parent.Core.SyncConfig)

local Services = {}

Services.UUIDS = {
	Workspace           = "00000000-0000-4000-8000-000000000001",
	ReplicatedStorage   = "00000000-0000-4000-8000-000000000002",
	ReplicatedFirst     = "00000000-0000-4000-8000-000000000003",
	ServerStorage       = "00000000-0000-4000-8000-000000000004",
	ServerScriptService = "00000000-0000-4000-8000-000000000005",
	StarterPlayer       = "00000000-0000-4000-8000-000000000006",
	StarterGui          = "00000000-0000-4000-8000-000000000007",
	StarterPack         = "00000000-0000-4000-8000-000000000008",
	Lighting            = "00000000-0000-4000-8000-000000000009",
	SoundService        = "00000000-0000-4000-8000-00000000000a",
	Teams               = "00000000-0000-4000-8000-00000000000b",
	TextChatService     = "00000000-0000-4000-8000-00000000000c",
	LocalizationService = "00000000-0000-4000-8000-00000000000d",
	MaterialService     = "00000000-0000-4000-8000-00000000000e",
}

-- Ordered list: the tree's root order is the same on every start.
--
-- Players, Chat and TestService are left out on purpose. Players fills at runtime;
-- its contents belong to the players, not to the author. Chat is the old chat
-- system, replaced by TextChatService. TestService is only for running tests.
local NAMES = {
	"Workspace",
	"ReplicatedStorage",
	"ReplicatedFirst",
	"ServerStorage",
	"ServerScriptService",
	"StarterPlayer",
	"StarterGui",
	"StarterPack",
	"Lighting",
	"SoundService",
	"Teams",
	-- Modern chat setup: TextChannel, TextChatCommand, window settings.
	"TextChatService",
	-- Where translation tables live.
	"LocalizationService",
	-- Custom MaterialVariant definitions.
	"MaterialService",
}

--- Returns the services to sync.
--- GetService throws when a service does not exist in this Studio version, so every
--- call is guarded and a missing service is skipped silently.
function Services.List(): { Instance }
	local list = {}
	-- If the user gave their own list in syncix.toml, that one applies.
	local requested = SyncConfig.ServiceList() or NAMES
	for _, fieldName in ipairs(requested) do
		local ok, svc = pcall(function()
			return game:GetService(fieldName)
		end)
		if ok and svc then
			table.insert(list, svc)
		end
	end
	return list
end

return Services
