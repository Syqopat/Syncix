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
local Store = require(script.Parent.Parent.Core.Store)
local Protocol = require(script.Parent.Protocol)
local Discovery = require(script.Parent.Discovery)
local Outbox = require(script.Parent.Outbox)

local PLUGIN_VERSION = Protocol.VERSION

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

-- For the panel: lists every core in the range (no permission or version filter).
function ConnectionManager:ScanAllPorts()
	return Discovery.ScanAllPorts(self)
end

--- Finds the core that serves this place (see Network/Discovery.lua).
function ConnectionManager:Discover()
	return Discovery.Discover(self)
end

-- The headers every authenticated call needs.
function ConnectionManager:AuthHeaders(): { [string]: string }
	return { ["X-Syncix-Token"] = self.accessToken or "" }
end

-- Sets the port by hand (called from the settings view).
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

	-- Every attempt gets a number; an attempt (or a retry wait) that finds a newer number
	-- has been replaced and stops. Pressing reconnect during a discovery or a retry wait
	-- used to leave the old one running too, and parallel loops with interleaved backoff
	-- sequences piled up in Output.
	self.connectGeneration = (self.connectGeneration or 0) + 1
	local generation = self.connectGeneration

	self:SetState("Discovering")

	task.spawn(function()
		local info = self:Discover()
		if self.connectGeneration ~= generation then
			return
		end
		if not info then
			self:HandleDisconnect()
			return
		end

		self.serverUrl = info.url
		self.serverInfo = info
		-- Every call except /health carries the project token. The plugin cannot read
		-- files, so /health is where it gets it.
		self.accessToken = info.token

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
				"[Syncix] Settings from syncix.toml — mode: %s, undo: %s",
				tostring(info.config.mode),
				tostring(info.config.undo)
			))
		end

		self:SetState("Connected")
		self.currentRetryWait = self.baseRetryWait

		print(string.format(
			"[Syncix] Connected: %s (folder: %s, port %d, core %s)",
			tostring(info.project), tostring(info.root), info.port, tostring(info.version)
		))

		-- Packets that failed while the connection was down are not resent: the FULL_SYNC
		-- below carries Studio's whole tree, which already holds everything they said.
		-- Resending them all at once hit Studio's request limit (46 in a row, measured)
		-- and turned one failure into a reconnect storm.
		if self.retryQueue and self.retryQueue:HasPending() then
			self.retryQueue:Flush()
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
	local success, err = pcall(function()
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
	-- A failed send and a failed poll can both report the same loss; one retry loop is enough.
	if self.state == "Reconnecting" then
		return
	end
	self:SetState("Reconnecting")
	local generation = self.connectGeneration

	warn(string.format("[Syncix] No connection. Retrying in %d second(s).", self.currentRetryWait))
	task.wait(self.currentRetryWait)

	-- Reconnected by hand (or paused) while waiting: that attempt carries on, not this one.
	if self.connectGeneration ~= generation or self.wasPaused then
		return
	end

	self.currentRetryWait = math.min(self.currentRetryWait * 2, self.maxRetryWait)

	self:Connect()
end

--- Sends one message to the core (see Network/Outbox.lua).
function ConnectionManager:Send(payload: any)
	return Outbox.Send(self, payload)
end

function ConnectionManager:_Split(payload: any): { any }?
	return Outbox.Split(self, payload)
end

function ConnectionManager:_ResendLater()
	return Outbox.ResendLater(self)
end

-- Long-polling loop
function ConnectionManager:StartPolling()
	-- Each poll fetches up to this many messages at once. One message per request made a
	-- large import cost one request per command, enough to hit Studio's HTTP limit; every
	-- failed request forces a reconnect and a full resend of the tree.
	local POLL_BATCH = 64
	-- A reconnect can start a new loop while an old one still waits on its request. Two
	-- loops dispatch out of order, and the applied count sent with a FULL_SYNC would then
	-- cover messages Studio has not applied. Only the newest loop may run and dispatch;
	-- whatever an old loop drops is sent again after the next FULL_SYNC.
	self.pollGeneration = (self.pollGeneration or 0) + 1
	local generation = self.pollGeneration
	local function isCurrent(): boolean
		return self.state == "Connected" and self.pollGeneration == generation
	end
	task.spawn(function()
		while isCurrent() do
			local success, response = pcall(function()
				return HttpService:RequestAsync({
					Url = self.serverUrl .. "/sync/poll?batch=" .. POLL_BATCH,
					Method = "GET",
					Headers = self:AuthHeaders(),
				})
			end)
			if self.pollGeneration ~= generation then
				break
			end

			if success and response.Success then
				local body = response.Body
				if body and body ~= "null" then
					local decoded
					local okDecode = pcall(function()
						decoded = HttpService:JSONDecode(body)
					end)
					if not okDecode or type(decoded) ~= "table" then
						-- Uncaught, this error ended the polling loop while the state still
						-- said Connected: nothing more arrived and nothing said why. The reply
						-- may have held messages, so it is handled like a failed request:
						-- reconnect, and the FULL_SYNC that follows settles them.
						warn("[Syncix] Could not read the core's reply; reconnecting.")
						self:HandleDisconnect()
						break
					end
					if self.commandDispatcher then
						-- A batching core answers with a list, an older one with one message.
						if decoded.event_type ~= nil then
							self.commandDispatcher:Dispatch(decoded)
						else
							for _, payload in ipairs(decoded) do
								-- Disconnected or superseded mid-reply: the rest is left
								-- undispatched and counts as not applied.
								if not isCurrent() then
									break
								end
								self.commandDispatcher:Dispatch(payload)
							end
						end
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
