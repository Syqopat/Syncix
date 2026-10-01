--!modules Network/BatchQueue
--!nolint
-- The queue between Studio's signals and the network: what it merges, what it holds
-- back, and the object it names when something changes every frame.

print("BatchQueue")

local function propertyPatch(uuid: string, property: string, value: any): any
	return {
		event_type = "PROPERTY_UPDATE",
		version = "v1",
		data = { syncix_id = uuid, property = property, value = value },
	}
end

local function queueWith(instance: any?): any
	local queue = BatchQueue.new()
	local cache = {
		GetInstance = function(_, _uuid: string)
			return instance
		end,
	}
	queue:OnStart(fakeContainer({
		ConnectionManager = { Send = function() end },
		Metrics = nil,
		RuntimeCache = cache,
	}))
	return queue
end

it("keeps only the last value of one property", function()
	local queue = queueWith(nil)
	queue:Enqueue(propertyPatch("a", "Size", 1))
	queue:Enqueue(propertyPatch("a", "Size", 2))
	queue:Enqueue(propertyPatch("a", "Size", 3))

	expectEqual(#queue.queue, 1, "three changes to one property are one message")
	expectEqual(queue.queue[1].data.value, 3, "the last value is the one that goes")
	local stats = queue:GetStats()
	expectEqual(stats.queued, 3)
	expectEqual(stats.coalesced, 2)
end)

it("does not merge different properties or different objects", function()
	local queue = queueWith(nil)
	queue:Enqueue(propertyPatch("a", "Size", 1))
	queue:Enqueue(propertyPatch("a", "Color", 1))
	queue:Enqueue(propertyPatch("b", "Size", 1))
	expectEqual(#queue.queue, 3)
	expectEqual(queue:GetStats().coalesced, 0)
end)

it("leaves a create alone: it is not a value that can be replaced", function()
	local queue = queueWith(nil)
	for _ = 1, 3 do
		queue:Enqueue({
			event_type = "CREATE",
			version = "v1",
			data = { syncix_id = "a", class_name = "Part", name = "Box" },
		})
	end
	expectEqual(#queue.queue, 3, "every create has to reach the core")
end)

-- The incident behind this: a WireframeHandleAdornment recoloured itself every frame
-- and piled up millions of patches. It was throttled, but nothing said WHICH object was
-- doing it, so there was nothing to act on.
it("names the object that changes a property every frame", function()
	local part = FakeInstance.new("WireframeHandleAdornment", "DebugDraw")
	local workspace = fakeService("Workspace")
	FakeInstance.adopt(workspace, part)

	local queue = queueWith(part)
	WARNINGS = {}
	for index = 1, 120 do
		queue:Enqueue(propertyPatch("a", "Color3", index))
	end

	expectEqual(queue:GetStats().floods, 1, "the flood is counted once")
	expectEqual(#WARNINGS, 1, "and reported once, not every frame")
	local said = WARNINGS[1]
	expect(string.find(said, "DebugDraw", 1, true) ~= nil, "the object has to be named: " .. said)
	expect(
		string.find(said, "WireframeHandleAdornment", 1, true) ~= nil,
		"with its class, which is what goes in syncix.toml: " .. said
	)
	expect(string.find(said, "ignore_classes", 1, true) ~= nil, "and the cure: " .. said)
end)

it("says nothing about ordinary editing", function()
	local queue = queueWith(nil)
	WARNINGS = {}
	for index = 1, 20 do
		queue:Enqueue(propertyPatch("a", "Size", index))
	end
	expectEqual(#WARNINGS, 0)
	expectEqual(queue:GetStats().floods, 0)
end)
