--!nolint
-- The fake Roblox API the plugin's modules need, and a very small test framework.
--
-- Loaded first by tools/luau-test.py; the modules under test are appended after it, so
-- what they see here is all they get. Only what the tests actually touch is faked.

FAKE = {}

local failures = 0
local passed = 0

function it(what: string, body: () -> ())
	local ok, err = pcall(body)
	if ok then
		passed += 1
		print("  ok    " .. what)
	else
		failures += 1
		print("  FAIL  " .. what)
		print("        " .. tostring(err))
	end
end

function expect(value: any, message: string?)
	if not value then
		error(message or "expected a value, got " .. tostring(value), 2)
	end
end

function expectEqual(actual: any, wanted: any, message: string?)
	if actual ~= wanted then
		error(
			string.format(
				"%s\n        wanted: %s\n        got:    %s",
				message or "not equal",
				tostring(wanted),
				tostring(actual)
			),
			2
		)
	end
end

function __report(): number
	print(string.format("  %d passed, %d failed", passed, failures))
	if failures > 0 then
		error(string.format("%d test(s) failed", failures), 0)
	end
	return 0
end

-- ---------------------------------------------------------------------------
-- Fake Roblox API
-- ---------------------------------------------------------------------------

WARNINGS = {}

function warn(...)
	local parts = {}
	for index = 1, select("#", ...) do
		parts[index] = tostring(select(index, ...))
	end
	table.insert(WARNINGS, table.concat(parts, " "))
end

--- A fake Instance: attributes, a parent, children and a class name. Enough for the
--- identity and queue logic, which is what the tests here are about.
local Fake = {}
Fake.__index = Fake

function Fake.new(className: string, name: string): any
	return setmetatable({
		ClassName = className,
		Name = name,
		Parent = nil,
		children = {},
		attributes = {},
	}, Fake)
end

function Fake:GetAttribute(key: string): any
	return self.attributes[key]
end

function Fake:SetAttribute(key: string, value: any)
	self.attributes[key] = value
end

function Fake:IsA(className: string): boolean
	return self.ClassName == className
end

function Fake:GetChildren(): { any }
	return self.children
end

function Fake:GetDescendants(): { any }
	local out = {}
	for _, child in ipairs(self.children) do
		table.insert(out, child)
		for _, deeper in ipairs(child:GetDescendants()) do
			table.insert(out, deeper)
		end
	end
	return out
end

function Fake:IsDescendantOf(other: any): boolean
	local above = self.Parent
	while above do
		if above == other then
			return true
		end
		above = above.Parent
	end
	return false
end

function Fake:GetFullName(): string
	local names = { self.Name }
	local above = self.Parent
	while above do
		table.insert(names, 1, above.Name)
		above = above.Parent
	end
	return table.concat(names, ".")
end

--- Puts `child` under `parent` the way Studio would.
function Fake.adopt(parent: any, child: any)
	child.Parent = parent
	table.insert(parent.children, child)
end

FakeInstance = Fake

-- `game`, and the one service the tests need under it.
game = Fake.new("DataModel", "game")

function fakeService(name: string, uuid: string?): any
	local service = Fake.new(name, name)
	if uuid then
		service:SetAttribute("__syncix_id", uuid)
	end
	Fake.adopt(game, service)
	return service
end

--- A dependency container like the plugin's own, filled by the test.
function fakeContainer(entries: { [string]: any }): any
	return {
		Get = function(_, name: string)
			return entries[name]
		end,
	}
end
