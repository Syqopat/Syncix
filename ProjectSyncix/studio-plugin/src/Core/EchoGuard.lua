-- EchoGuard
-- Keeps changes applied by Syncix itself from being sent back to the core.
--
-- PROBLEM:
-- Echo suppression used to rely on the `isLocked` flag in CommandDispatcher:
-- Lock() before applying a patch, Unlock() right after. But in Roblox, property
-- signals (Changed, AttributeChanged, AncestryChanged) are DEFERRED: the
-- observer's function fires AFTER Unlock() has run. By then the lock was open,
-- so the change was sent back to the core as an echo.
--
-- Measured: when the core sent 40 property commands to Studio, 64 changes entered
-- the plugin's queue — everything we sent came back. No infinite loop formed
-- (the value was the same, so it stopped on the second round), but traffic doubled
-- and the disk writer was triggered for nothing.
--
-- FIX:
-- Look at the VALUE, not the timing. Right before a patch is applied, a note says
-- "I expect this property of this object to be set to this value". When the observer
-- fires and the incoming value equals the expected one, it is our own write and
-- is not sent. A note is used once and expires quickly, so
-- the user's REAL changes are never swallowed.

local EchoGuard = {}
EchoGuard.__index = EchoGuard

-- An expectation that is not consumed within this time is dropped. Deferred signals arrive
-- in the same frame or the next; 2 seconds is a more than safe upper bound.
local TTL = 2

function EchoGuard.new()
	local self = setmetatable({}, EchoGuard)
	self.expectedList = {}
	self.lastPrune = os.clock()
	return self
end

local function keyName(uuid: string, field: string): string
	return tostring(uuid) .. "|" .. tostring(field)
end

function EchoGuard:Prune()
	local now = os.clock()
	if now - self.lastPrune < 5 then
		return
	end
	self.lastPrune = now
	for k, entry in pairs(self.expectedList) do
		if now - entry.timestamp > TTL then
			self.expectedList[k] = nil
		end
	end
end

-- "I am writing this value" note. Called RIGHT BEFORE a patch is applied.
function EchoGuard:Expect(uuid: string, field: string, datum: any)
	if not uuid or not field then return end
	self.expectedList[keyName(uuid, field)] = { datum = datum, timestamp = os.clock() }
	self:Prune()
end

-- Is a change coming from the observer our own write?
-- If so the note is consumed and true is returned (do not send).
function EchoGuard:Consume(uuid: string, field: string, datum: any): boolean
	local k = keyName(uuid, field)
	local entry = self.expectedList[k]
	if not entry then
		return false
	end

	if os.clock() - entry.timestamp > TTL then
		self.expectedList[k] = nil
		return false
	end

	-- Roblox types (Vector3, Color3, UDim2, CFrame) and primitives compare correctly
	-- with ==. The comparison runs in pcall so unexpected types cannot throw.
	local ok, isEqual = pcall(function()
		return entry.datum == datum
	end)

	if ok and isEqual then
		self.expectedList[k] = nil
		return true
	end

	-- The value differs: the user really changed it. Drop the expectation and send.
	self.expectedList[k] = nil
	return false
end

return EchoGuard
