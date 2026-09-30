--[[
	Sending to the core: one POST, the split for a message Studio refuses to post in one
	piece, and the retry for what could not go out.
]]

local HttpService = game:GetService("HttpService")
local Protocol = require(script.Parent.Protocol)

local MAX_POST_BYTES = Protocol.MAX_POST_BYTES

local ConnectionManager = {}

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
	if #json > MAX_POST_BYTES then
		-- Sent whole, Studio refuses it; that refusal used to look like a lost connection,
		-- and a place whose tree passed 1 MB could never finish connecting.
		local parts = self:_Split(payload)
		if parts then
			for _, part in ipairs(parts) do
				self:Send(part)
			end
		else
			warn(string.format(
				"[Syncix] A %s message of %d KB is over Studio's 1024 KB limit and could not be split; it was not sent.",
				tostring(payload.event_type),
				math.floor(#json / 1024)
			))
		end
		return
	end
	local url = self.serverUrl .. "/sync/push"

	local headers = self:AuthHeaders()
	headers["Content-Type"] = "application/json"

	task.spawn(function()
		local success, err = pcall(function()
			local response = HttpService:RequestAsync({
				Url = url,
				Method = "POST",
				Headers = headers,
				Body = json,
			})
			if not response.Success then
				error(string.format("HTTP %d %s", response.StatusCode, tostring(response.Body)), 0)
			end
		end)

		if success then
			if self.metrics then self.metrics:IncrementSuccessfulRequests() end
			return
		end
		if self.metrics then self.metrics:IncrementFailedRequests() end
		local message = tostring(err)
		if string.find(message, "too large", 1, true) then
			-- Resending would fail the same way forever.
			warn("[Syncix] A message was too large for Studio to send and was dropped: " .. message)
		elseif string.find(message, "exceeded limit", 1, true) then
			-- Studio's request limit is not a lost connection; reconnecting would resend
			-- the whole tree and make even more requests.
			if self.retryQueue then self.retryQueue:EnqueueFailed(payload) end
			self:_ResendLater()
		else
			warn("[Syncix] Could not send packet, queued for retry. Error: " .. message)
			if self.retryQueue then self.retryQueue:EnqueueFailed(payload) end
			if self.state == "Connected" then
				self:HandleDisconnect()
			end
		end
	end)
end

-- Splits a message too big for one POST. A FULL_SYNC goes out in parts the core puts
-- back together, but only to a core that says it can ("full_sync_parts"): an older one
-- would take every part for the whole tree and drop the rest. A batch of patches is
-- halved. Returns nil when the message cannot be split.
function ConnectionManager:_Split(payload: any): { any }?
	local data = payload.data
	if type(data) ~= "table" or data.part_id ~= nil then
		return nil
	end

	if payload.event_type == "FULL_SYNC" and type(data.instances) == "table" then
		local features = self.serverInfo and self.serverInfo.features
		if type(features) ~= "table" or not table.find(features, "full_sync_parts") then
			return nil
		end
		local budget = MAX_POST_BYTES - 64 * 1024
		local groups, current, size = {}, {}, 0
		for _, instance in ipairs(data.instances) do
			local bytes = #HttpService:JSONEncode(instance) + 1
			if bytes > budget then
				-- A single instance that big (a script of hundreds of KB) cannot travel;
				-- the rest of the tree still goes.
				warn(string.format("[Syncix] %s is too large to sync (%d KB).", tostring(instance.name), math.floor(bytes / 1024)))
			else
				if size + bytes > budget and #current > 0 then
					table.insert(groups, current)
					current, size = {}, 0
				end
				table.insert(current, instance)
				size += bytes
			end
		end
		if #current > 0 then
			table.insert(groups, current)
		end
		local partId = HttpService:GenerateGUID(false)
		local parts = {}
		for index, group in ipairs(groups) do
			local partData = table.clone(data)
			partData.instances = group
			partData.part_id = partId
			partData.part_index = index
			partData.part_count = #groups
			table.insert(parts, { event_type = payload.event_type, version = payload.version, data = partData })
		end
		return if #parts > 0 then parts else nil
	end

	local patches = data.patches
	if type(patches) == "table" and #patches > 1 then
		local half = math.floor(#patches / 2)
		local first, second = table.clone(data), table.clone(data)
		first.patches = table.move(patches, 1, half, 1, {})
		second.patches = table.move(patches, half + 1, #patches, 1, {})
		return {
			{ event_type = payload.event_type, version = payload.version, data = first },
			{ event_type = payload.event_type, version = payload.version, data = second },
		}
	end
	return nil
end

-- After Studio's request limit was hit: resend what failed a few seconds later, one at
-- a time.
function ConnectionManager:_ResendLater()
	if self._resendScheduled then
		return
	end
	self._resendScheduled = true
	warn("[Syncix] Studio's HTTP request limit was reached; retrying in 5 seconds.")
	task.delay(5, function()
		self._resendScheduled = false
		if self.state ~= "Connected" or not self.retryQueue then
			return
		end
		for _, pending in ipairs(self.retryQueue:Flush()) do
			self:Send(pending)
			task.wait(0.2)
		end
	end)
end

return {
	Send = ConnectionManager.Send,
	Split = ConnectionManager._Split,
	ResendLater = ConnectionManager._ResendLater,
}
