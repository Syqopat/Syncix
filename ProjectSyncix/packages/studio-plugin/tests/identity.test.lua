--!modules Core/RuntimeCache
--!nolint
-- Which object an identity points at.
--
-- The bug behind these: a Model copied with Ctrl+D carries the original's identity in
-- its attributes. Two live objects under one identity meant `syncix set` moved the copy
-- instead of the original, and a `pull` said everything was consistent because the tree
-- is read from the attributes.

print("RuntimeCache identity")

-- The cache looks up the real holder of an identity through the services list.
FAKE["Services"] = {
	List = function()
		return { game:GetChildren()[1] }
	end,
}

local function world(): (any, any, any)
	game = FakeInstance.new("DataModel", "game")
	local workspace = fakeService("Workspace", "ws")
	local original = FakeInstance.new("Model", "Farm")
	FakeInstance.adopt(workspace, original)
	local copy = FakeInstance.new("Model", "Farm")
	FakeInstance.adopt(workspace, copy)
	return workspace, original, copy
end

it("hands back the object filed under an identity", function()
	local _, original = world()
	original:SetAttribute("__syncix_id", "one")
	local cache = RuntimeCache.new()
	cache:CacheInstance("one", original)
	expectEqual(cache:GetInstance("one"), original)
	expectEqual(cache:GetUuid(original), "one")
end)

it("never leaves one object reachable by two identities", function()
	local _, original = world()
	local cache = RuntimeCache.new()
	cache:CacheInstance("one", original)
	cache:CacheInstance("two", original)
	expectEqual(cache:GetUuid(original), "two")
	expectEqual(cache:GetInstance("one"), nil, "the old entry has to go")
end)

-- The repair: the cache says identity "one" is the copy, but the copy carries "two" and
-- the original carries "one". Asking for "one" must give the ORIGINAL.
it("repairs a mix-up and sends the command to the right object", function()
	local _, original, copy = world()
	original:SetAttribute("__syncix_id", "one")
	copy:SetAttribute("__syncix_id", "two")

	local cache = RuntimeCache.new()
	cache:CacheInstance("one", copy) -- wrong, as an older plugin left it
	WARNINGS = {}

	expectEqual(cache:GetInstance("one"), original, "the object carrying the identity wins")
	expectEqual(cache:GetInstance("two"), copy, "and the other goes back under its own")
	expectEqual(#WARNINGS, 1, "the repair is reported, not silent")
end)

it("leaves an object alone while its identity is still being settled", function()
	local _, original = world()
	local cache = RuntimeCache.new()
	cache:CacheInstance("one", original) -- the attribute has not been written yet
	expectEqual(cache:GetInstance("one"), original)
end)

it("forgets an object when it is destroyed", function()
	local _, original = world()
	original:SetAttribute("__syncix_id", "one")
	local cache = RuntimeCache.new()
	cache:CacheInstance("one", original)
	cache:MarkDirty("one")
	cache:Remove("one")
	expectEqual(cache:GetInstance("one"), nil)
	expectEqual(cache:GetUuid(original), nil)
	expectEqual(#cache:GetDirtyUuids(), 0)
end)

it("reports only what changed since the last send", function()
	local cache = RuntimeCache.new()
	cache:MarkDirty("a")
	cache:MarkDirty("b")
	expectEqual(#cache:GetDirtyUuids(), 2)
	cache:ClearDirty("a")
	expectEqual(#cache:GetDirtyUuids(), 1)
end)
