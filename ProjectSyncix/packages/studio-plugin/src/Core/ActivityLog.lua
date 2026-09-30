-- ActivityLog
-- Activity log that makes what Syncix does visible.
--
-- WHY IT EXISTS:
-- Rojo has a "patch visualizer" because Rojo works one-way and, on connect,
-- presents one large diff for approval. Our situation is different and actually
-- worse: changes flow continuously and were applied COMPLETELY SILENTLY. The user
-- only noticed something had changed if they happened to see it — that is exactly what
-- happened with the Position bug: objects dropped to 0,0,0 and nobody noticed for days.
--
-- Copying Rojo's approval dialog does not suit us: in a continuous, two-way flow
-- approving every change would be unusable. Instead, an ACTIVITY LOG:
-- it does not block, but shows afterwards what came in and what went out.
--
-- Second job: CONFLICT WARNING. When the user and Syncix changed the same property
-- within a short time, one of them was silently overwritten. Now it is flagged.

local ActivityLog = {}
ActivityLog.__index = ActivityLog

-- Maximum number of entries kept in memory. The panel only shows the last few dozen;
-- growing without limit would leak memory in long sessions.
local MAX_ENTRIES = 80

-- If there are both outgoing and incoming changes within this time, it counts as a conflict.
local CONFLICT_WINDOW = 3

function ActivityLog.new()
	local self = setmetatable({}, ActivityLog)
	self.entries = {}
	-- (uuid|property) -> { datum = ..., timestamp = ... }  the last OUTGOING changes
	self.lastOutgoing = {}
	self.conflictCount = 0
	return self
end

local function keyName(uuid, field)
	return tostring(uuid) .. "|" .. tostring(field)
end

local function shortValue(datum): string
	local t = typeof(datum)
	if t == "string" then
		if #datum > 40 then
			return string.sub(datum, 1, 37) .. "..."
		end
		return datum
	elseif t == "Vector3" then
		return string.format("%.4g, %.4g, %.4g", datum.X, datum.Y, datum.Z)
	elseif t == "Color3" then
		return string.format("#%02x%02x%02x",
			math.floor(datum.R * 255 + 0.5),
			math.floor(datum.G * 255 + 0.5),
			math.floor(datum.B * 255 + 0.5))
	elseif t == "nil" then
		return "(removed)"
	end
	return tostring(datum)
end

function ActivityLog:_Add(entry)
	table.insert(self.entries, 1, entry) -- newest first
	if #self.entries > MAX_ENTRIES then
		table.remove(self.entries)
	end
end

--- A change made in Studio and SENT to the core.
function ActivityLog:Outbound(pass: string, targetName: string, field: string?, datum: any, uuid: string?)
	if uuid and field then
		self.lastOutgoing[keyName(uuid, field)] = { datum = datum, timestamp = os.clock() }
	end
	self:_Add({
		direction = "out",
		pass = pass,
		target = targetName,
		field = field,
		datum = shortValue(datum),
		timestamp = os.clock(),
		conflict = false,
	})
end

--- A change that CAME from the core and was applied in Studio.
--- If the same field was sent from Studio a moment ago with a different value,
--- this is a conflict: the user's change is being overwritten.
function ActivityLog:Inbound(pass: string, targetName: string, field: string?, datum: any, uuid: string?)
	local conflict = false

	if uuid and field then
		local previous = self.lastOutgoing[keyName(uuid, field)]
		if previous and (os.clock() - previous.timestamp) < CONFLICT_WINDOW then
			local ok, isEqual = pcall(function()
				return previous.datum == datum
			end)
			if not (ok and isEqual) then
				conflict = true
				self.conflictCount += 1
				warn(string.format(
					"[Syncix] Conflict on %s.%s — you set '%s', then '%s' arrived from the editor and overwrote it.",
					tostring(targetName), tostring(field),
					shortValue(previous.datum), shortValue(datum)
				))
			end
		end
	end

	self:_Add({
		direction = "in",
		pass = pass,
		target = targetName,
		field = field,
		datum = shortValue(datum),
		timestamp = os.clock(),
		conflict = conflict,
	})
end

--- The newest entries, for showing in the panel.
function ActivityLog:Recent(itemCount: number)
	local output = {}
	for i = 1, math.min(itemCount, #self.entries) do
		table.insert(output, self.entries[i])
	end
	return output
end

function ActivityLog:Summary()
	local incoming, outgoing = 0, 0
	for _, k in ipairs(self.entries) do
		if k.direction == "in" then incoming += 1 else outgoing += 1 end
	end
	return {
		total = #self.entries,
		incoming = incoming,
		outgoing = outgoing,
		conflict = self.conflictCount,
	}
end

return ActivityLog
