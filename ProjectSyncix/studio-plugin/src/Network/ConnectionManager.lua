-- ConnectionManager
-- State machine handling HTTP polling, push, timeouts and exponential backoff.
-- States: Disconnected, Discovering, Connecting, Connected, Reconnecting, Blocked
--
-- Three things were added here for the release:
--   1. Port discovery. The port is no longer fixed at 8080; if taken, the core moves to the next one.
--      The plugin cannot read files, so it scans the range through /health.
--   2. Version compatibility check. An old plugin with a new core misbehaved without a word.
--   3. First-connection approval. The user is shown which project folder connected.

local HttpService = game:GetService("HttpService")
local SyncConfig = require(script.Parent.Parent.Core.SyncConfig)
local PlaceIdentity = require(script.Parent.Parent.Core.PlaceIdentity)
local Store = require(script.Parent.Parent.Core.Store)
local Approval = require(script.Parent.Parent.Core.Approval)

-- These four constants MUST MATCH the core. Their counterparts:
--   PLUGIN_VERSION  <-> core-engine/Cargo.toml  version
--   PLUGIN_PROTOCOL <-> project.rs  PROTOCOL_VERSION
--   PORT_START  <-> project.rs  DEFAULT_PORT
--   PORT_RANGE     <-> project.rs  PORT_SCAN_SPAN
local PLUGIN_VERSION = "0.1.1"
local PLUGIN_PROTOCOL = 1
local PORT_START = 8080
local PORT_RANGE = 10

local ConnectionManager = {}
ConnectionManager.__index = ConnectionManager

function ConnectionManager.new()
	local self = setmetatable({}, ConnectionManager)

	self.serverUrl = nil        -- set after discovery
	self.serverInfo = nil       -- /health reply: project, root, port, version
	self.state = "Disconnected"

	-- Exponential backoff settings
	self.baseRetryWait = 1.0
	self.maxRetryWait = 30.0
	self.currentRetryWait = 1.0

	-- Per-session memory so denied roots are not asked again and again
	self.rejected = {}

	-- Did the user pause sync by hand?
	-- Pausing only happens by the user's explicit request; when the network drops
	-- the path is Reconnecting, which is a separate state.
	self.wasPaused = false

	-- Port pinned by hand. When nil the range 8080-8089 is scanned.
	-- When pinned, ONLY that port is tried: with two projects open, this is the only way
	-- to be certain which project it connects to.
	self.manualPort = nil

	return self
end

function ConnectionManager:OnStart(container)
	self.retryQueue = container:Get("RetryQueue")
	self.metrics = container:Get("Metrics")
	self.commandDispatcher = container:Get("CommandDispatcher")
	self.patchBuilder = container:Get("PatchBuilder")
	self.batchQueue = container:Get("BatchQueue")
	self.activityLog = container:Get("ActivityLog")
	-- The `plugin` global is not reliable in ModuleScripts; it is passed
	-- explicitly from the main script. The approval dialog and settings storage depend on it.
	self.plugin = container:Get("Plugin").ref

	-- Connection permission gate.
	--
	-- OFF by default. Reason: plugin:SetSetting is not written to disk for locally
	-- installed plugins (measured), so "remember" did not work and on every Studio
	-- start the window popped up and held sync. The security value did not cover the
	-- cost of blocking.
	--
	-- Instead: once connected, the project folder it connected to is WRITTEN to Output and
	-- to the panel. The user sees what they are connected to, but the flow does not stop.
	-- Anyone who wants the gate back can turn the permission prompt on in the Syncix panel.
	self.askPermission = Store.Get(self.plugin, "syncix_ask_permission", false) == true

	local saved = Store.Get(self.plugin, "syncix_port", 0)
	if type(saved) == "number" and saved > 0 then
		self.manualPort = saved
		print(string.format("[Syncix] Saved port setting: %d (only this port will be tried)", saved))
	end

	self:Connect()
end

function ConnectionManager:SetState(newState: string)
	if self.state ~= newState then
		print(string.format("[Syncix] %s -> %s", self.state, newState))
		self.state = newState
	end
end

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

-- Sets the port by hand (called from SettingsPanel).
function ConnectionManager:SetManualPort(port: number?)
	self.manualPort = port
	-- When the port changes, earlier denials lose their meaning.
	self.rejected = {}
end

function ConnectionManager:GetManualPort(): number?
	return self.manualPort
end

-- When the user changes the setting, reconnect without waiting.
function ConnectionManager:ForceReconnect()
	self.serverUrl = nil
	self.serverInfo = nil
	self.currentRetryWait = self.baseRetryWait
	self:SetState("Disconnected")
	self:Connect()
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

--- Pauses or resumes sync by hand.
---
--- Resuming does a FULL RESYNC: while paused there may have been changes on both the
--- Studio and the editor side, and we do not know which is
--- newer. Resending Studio's snapshot brings both
--- sides to the same point in one step.
function ConnectionManager:SetPaused(pause: boolean)
	if self.wasPaused == pause then
		return
	end
	self.wasPaused = pause

	if pause then
		self:SetState("Paused")
		print("[Syncix] Sync paused. Nothing is sent to or applied from the editor.")
	else
		print("[Syncix] Sync resumed. Re-syncing the full tree...")
		self.currentRetryWait = self.baseRetryWait
		self:SetState("Disconnected")
		self:Connect()
	end
end

function ConnectionManager:IsPaused(): boolean
	return self.wasPaused
end

function ConnectionManager:Connect()
	if self.wasPaused then return end
	if self.state == "Connected" then return end

	self:SetState("Discovering")

	task.spawn(function()
		local info = self:Discover()
		if not info then
			self:HandleDisconnect()
			return
		end

		self.serverUrl = info.url
		self.serverInfo = info

		-- The settings in syncix.toml arrive through the core. The plugin deliberately does not
		-- keep them itself: with two separate sets it would be unclear which one
		-- applies. The single source of truth is syncix.toml.
		-- If the folder is bound to another place the user must see it in Studio;
		-- they may not be looking at the terminal, and if it stayed silent the answer to
		-- "why is it not syncing" would be written nowhere.
		if info.place_conflict then
			warn(string.format(
				"[Syncix] This folder belongs to a DIFFERENT place. Sync is on hold so nothing gets mixed.\n" ..
				"  folder is bound to : %s\n" ..
				"  this place         : %s (%s)\n" ..
				"  Decide in a terminal:\n" ..
				"    syncix bind --studio   this place is right, rewrite the folder from it\n" ..
				"    syncix bind --disk     the folder is right, load it into this place",
				tostring(info.place_conflict.folder_place),
				tostring(info.place_conflict.incoming_place),
				tostring(info.place_conflict.incoming_name)
			))
		end

		SyncConfig.Apply(info.config)
		if info.config then
			print(string.format(
				"[Syncix] Settings from syncix.toml — mode: %s, play: %s, undo: %s",
				tostring(info.config.mode),
				tostring(info.config.play_mode),
				tostring(info.config.undo)
			))
		end

		self:SetState("Connected")
		self.currentRetryWait = self.baseRetryWait

		print(string.format(
			"[Syncix] Connected: %s (folder: %s, port %d, core %s)",
			tostring(info.project), tostring(info.root), info.port, tostring(info.version)
		))

		-- Send old packets that could not be sent
		if self.retryQueue and self.retryQueue:HasPending() then
			local pending = self.retryQueue:Flush()
			for _, payload in ipairs(pending) do
				self:Send(payload)
			end
		end

		-- Bootstrap / FULL_SYNC (sync from scratch)
		if self.patchBuilder then
			local fullSyncPatch = self.patchBuilder:BuildFullTreeSnapshot()
			self:Send(fullSyncPatch)
			print("[Syncix] Bootstrap FULL_SYNC sent.")
		end

		self:StartPolling()
		self:StartMetricsReporting()
	end)
end

function ConnectionManager:PingServer(): (boolean, number)
	if not self.serverUrl then
		return false, 0
	end
	local start = os.clock()
	local success, _ = pcall(function()
		return HttpService:RequestAsync({
			Url = self.serverUrl .. "/health",
			Method = "GET"
		})
	end)
	local latencyMs = math.floor((os.clock() - start) * 1000)

	if success and self.metrics then
		self.metrics:RecordLatency(latencyMs)
	end

	return success, latencyMs
end

function ConnectionManager:HandleDisconnect()
	-- If the user paused, the reconnect loop must not run.
	if self.wasPaused then
		return
	end
	self:SetState("Reconnecting")

	warn(string.format("[Syncix] No connection. Retrying in %d second(s).", self.currentRetryWait))
	task.wait(self.currentRetryWait)

	self.currentRetryWait = math.min(self.currentRetryWait * 2, self.maxRetryWait)

	self:Connect()
end

function ConnectionManager:Send(payload: any)
	-- Nothing is sent while paused. Nothing is queued either: when the pause
	-- ends a full resync runs, and sending stale packets afterwards
	-- would spoil that sync.
	if self.wasPaused then
		return
	end
	if not self.serverUrl then
		if self.retryQueue then self.retryQueue:EnqueueFailed(payload) end
		return
	end

	local json = HttpService:JSONEncode(payload)
	local url = self.serverUrl .. "/sync/push"

	task.spawn(function()
		local success, _ = pcall(function()
			HttpService:PostAsync(url, json, Enum.HttpContentType.ApplicationJson)
		end)

		if success then
			if self.metrics then self.metrics:IncrementSuccessfulRequests() end
		else
			warn("[Syncix] Could not send packet, queued for retry.")
			if self.metrics then self.metrics:IncrementFailedRequests() end
			if self.retryQueue then self.retryQueue:EnqueueFailed(payload) end

			if self.state == "Connected" then
				self:HandleDisconnect()
			end
		end
	end)
end

-- Long-polling loop
function ConnectionManager:StartPolling()
	task.spawn(function()
		while self.state == "Connected" do
			local success, response = pcall(function()
				return HttpService:RequestAsync({
					Url = self.serverUrl .. "/sync/poll",
					Method = "GET"
				})
			end)

			if success and response.Success then
				local body = response.Body
				if body and body ~= "null" then
					local payload = HttpService:JSONDecode(body)
					if payload and self.commandDispatcher then
						self.commandDispatcher:Dispatch(payload)
					end
				end
			else
				self:HandleDisconnect()
				break
			end

			task.wait(0.1)
		end
	end)
end

-- Reports BatchQueue counters to the core regularly.
-- Only this way can we measure whether coalescing really works; the only way to
-- verify it used to be dragging objects around in Studio by hand.
function ConnectionManager:StartMetricsReporting()
	task.spawn(function()
		while self.state == "Connected" do
			task.wait(5)
			if self.state ~= "Connected" or not self.batchQueue then
				break
			end
			local counter = self.batchQueue:GetStats()
			-- The activity summary is reported too: the only way to see the log in the plugin's
			-- memory from outside. `syncix status` shows these numbers;
			-- the conflict count in particular is an early warning of silent data loss.
			local flow = self.activityLog and self.activityLog:Summary() or nil
			self:Send({
				event_type = "PLUGIN_METRICS",
				version = "v1",
				data = {
					queued = counter.queued,
					coalesced = counter.coalesced,
					plugin_version = PLUGIN_VERSION,
					activity_total = flow and flow.total or 0,
					activity_in = flow and flow.incoming or 0,
					activity_out = flow and flow.outgoing or 0,
					conflicts = flow and flow.conflict or 0,
				},
			})
		end
	end)
end

return ConnectionManager
