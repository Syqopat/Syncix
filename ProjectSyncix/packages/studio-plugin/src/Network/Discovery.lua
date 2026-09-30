--[[
	Finding the core: which port answers, whether that core belongs to this place and
	whether its version can be talked to.
]]

local HttpService = game:GetService("HttpService")
local SyncConfig = require(script.Parent.Parent.Core.SyncConfig)
local PlaceIdentity = require(script.Parent.Parent.Core.PlaceIdentity)
local Store = require(script.Parent.Parent.Core.Store)
local Approval = require(script.Parent.Parent.Core.Approval)
local Protocol = require(script.Parent.Protocol)

local PLUGIN_VERSION = Protocol.VERSION
local PLUGIN_PROTOCOL = Protocol.PROTOCOL
local PORT_START = Protocol.PORT_START
local PORT_RANGE = Protocol.PORT_RANGE

local ConnectionManager = {}

-- Probes a single port. Returns the info if the answer comes from a Syncix core.
local function readHealth(port: number)
	local url = string.format("http://127.0.0.1:%d/health", port)
	local ok, response = pcall(function()
		return HttpService:RequestAsync({ Url = url, Method = "GET" })
	end)
	if not ok or not response or not response.Success then
		return nil
	end
	local decoded
	local okDecode = pcall(function()
		decoded = HttpService:JSONDecode(response.Body)
	end)
	if not okDecode or type(decoded) ~= "table" then
		return nil
	end
	-- Another program may be on the port; the Syncix signature is checked.
	if decoded.status == nil then
		return nil
	end
	decoded.port = decoded.port or port
	decoded.url = string.format("http://127.0.0.1:%d", port)
	return decoded
end

-- Compares major.minor; a patch difference is fine.
local function versionCompatible(a: string?, b: string?): boolean
	if type(a) ~= "string" or type(b) ~= "string" then
		return false
	end
	local aMajor, aMinor = string.match(a, "^(%d+)%.(%d+)")
	local bMajor, bMinor = string.match(b, "^(%d+)%.(%d+)")
	if not aMajor or not bMajor then
		return false
	end
	return aMajor == bMajor and aMinor == bMinor
end

-- For the panel: lists every core in the range (no permission or version filter).
function ConnectionManager:ScanAllPorts()
	local foundList = {}
	for port = PORT_START, PORT_START + PORT_RANGE - 1 do
		local info = readHealth(port)
		if info then
			table.insert(foundList, info)
		end
	end
	-- A hand-typed port may be outside the range; it must be listed too.
	if self.manualPort and (self.manualPort < PORT_START or self.manualPort >= PORT_START + PORT_RANGE) then
		local info = readHealth(self.manualPort)
		if info then
			table.insert(foundList, info)
		end
	end
	return foundList
end

function ConnectionManager:Discover()
	-- When the port is pinned there is no scan; only that port is tried.
	local first, last
	if self.manualPort then
		first, last = self.manualPort, self.manualPort
	else
		first, last = PORT_START, PORT_START + PORT_RANGE - 1
	end

	for port = first, last do
		local info = readHealth(port)
		if info then
			local root = tostring(info.root or "")

			if self.rejected[root] then
				-- Already denied in this session; skip it.
				continue
			end

			-- Skip a core bound to ANOTHER place.
			--
			-- While scanning ports the plugin used to connect to the FIRST healthy core it found.
			-- With two projects open at once that meant connecting to the wrong project:
			-- the core immediately raised a place conflict and suspended sync, and
			-- the user was left wondering why it did not work. Now a folder bound
			-- to our own place, or a folder never bound (empty),
			-- is chosen.
			local myPlace = PlaceIdentity.Resolve()
			if info.bound_place ~= nil
				and myPlace ~= ""
				and tostring(info.bound_place) ~= myPlace
			then
				continue
			end

			-- Version gate: if incompatible, do not connect, and say why plainly.
			if not versionCompatible(info.version, PLUGIN_VERSION) then
				warn(string.format(
					"[Syncix] Version mismatch. Plugin: %s, core: %s (port %d).\n" ..
					"  The same major.minor version is required. Update the VS Code extension and the Studio plugin.",
					PLUGIN_VERSION, tostring(info.version), port
				))
				continue
			end

			if info.protocol ~= nil and info.protocol ~= PLUGIN_PROTOCOL then
				warn(string.format(
					"[Syncix] Protocol mismatch. Plugin: %d, core: %s. An update is required.",
					PLUGIN_PROTOCOL, tostring(info.protocol)
				))
				continue
			end

			-- The permission gate only runs when explicitly asked for (see self.askPermission).
			local isAllowed = true
			-- The permission gate now comes from syncix.toml; the panel setting only
			-- applies when the core could not be reached at all.
			if self.askPermission or SyncConfig.AskPermission() then
				isAllowed = Approval.GetStoredDecision(self.plugin, root)
			end

			if isAllowed == nil then
				print(string.format("[Syncix] A new project wants to connect: %s", root))
				local decision = Approval.Ask(self.plugin, info)

				if decision == "error" then
					-- The window could not open: no decision was made. It is not stored or
					-- blacklisted, so a UI glitch cannot lock sync
					-- permanently; it is asked again on the next attempt.
					continue
				end

				isAllowed = (decision == "allow")
				Approval.Store(self.plugin, root, isAllowed)
			end

			if not isAllowed then
				self.rejected[root] = true
				warn(string.format(
					"[Syncix] Connection refused: %s\n" ..
					"  To change your mind, reset the Studio plugin settings.",
					root
				))
				continue
			end

			return info
		end
	end

	if self.manualPort then
		warn(string.format(
			"[Syncix] No Syncix core found on port %d.\n" ..
			"  Start one there:  syncix serve %d\n" ..
			"  Or set the port back to Automatic in the Syncix panel.",
			self.manualPort, self.manualPort
		))
	end
	return nil
end

return {
	readHealth = readHealth,
	versionCompatible = versionCompatible,
	ScanAllPorts = ConnectionManager.ScanAllPorts,
	Discover = ConnectionManager.Discover,
}
