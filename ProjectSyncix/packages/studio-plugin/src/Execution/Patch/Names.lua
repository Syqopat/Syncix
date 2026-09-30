--[[
	Property names Roblox refused: the right spelling for one that differs only in case,
	and the "did you mean" hint for one that does not exist at all.
]]

local PropertyTable = require(script.Parent.Parent.Parent.Observer.PropertyTable)
local Suggest = require(script.Parent.Parent.Parent.Core.Suggest)

local Names = {}

-- Property names in the wrong case.
--
-- Roblox member names are case-sensitive: a file or command that said "size" failed
-- with "size is not a valid member of Part". The name Roblox knows is looked up
-- case-insensitively in the generated table, walking up the superclasses.

-- Members tools/gen-properties.py leaves out of the table on purpose, which a file or
-- command can still misspell the same way.
local UNLISTED_MEMBERS = {
    Instance = { "Name", "Parent", "Archivable" },
    LuaSourceContainer = { "Source" },
}

-- className -> { lowercased name -> name Roblox knows }. Built the first time a write
-- to that class fails, so correctly spelt writes never pay for it.
local caseIndex = {}
-- "Class.wrongName" -> true once the correction has been reported.
local caseWarned = {}

local function caseIndexFor(className: string): { [string]: string }
    local index = caseIndex[className]
    if index then
        return index
    end
    index = {}
    local function add(realName: string)
        local key = string.lower(realName)
        -- A subclass is walked first, so its spelling wins.
        if index[key] == nil then
            index[key] = realName
        end
    end
    local current = className
    local guard = 0
    while current and guard < 64 do
        local extras = UNLISTED_MEMBERS[current]
        if extras then
            for _, realName in ipairs(extras) do
                add(realName)
            end
        end
        local entry = PropertyTable[current]
        if not entry then
            break
        end
        for realName in pairs(entry.p) do
            add(realName)
        end
        current = entry.u
        guard += 1
    end
    caseIndex[className] = index
    return index
end

-- The correctly spelt name for a write that failed, or nil when the name was not the
-- problem. Warns once per class and wrong name.
local function correctedName(instance: Instance, propName: any): string?
    if type(propName) ~= "string" then
        return nil
    end
    local className = instance.ClassName
    local realName = caseIndexFor(className)[string.lower(propName)]
    if not realName or realName == propName then
        return nil
    end
    -- Only a name that is not a member at all is corrected; a real member that
    -- merely differs in case from another keeps its own error.
    local isMember = pcall(function()
        return (instance :: any)[propName]
    end)
    if isMember then
        return nil
    end
    local key = className .. "." .. propName
    if not caseWarned[key] then
        caseWarned[key] = true
        warn(string.format(
            "[Syncix] %s has no property \"%s\"; applied it as \"%s\". Fix the name in the file or command.",
            className,
            propName,
            realName
        ))
    end
    return realName
end

-- " Did you mean X?" for a write Studio refused: the closest property when the name is
-- not a member, the closest item when the property holds an enum. A file edited by hand
-- or an AI reaches Studio without the terminal's checks, so this is where it hears of a
-- slip. Empty when nothing is close.
local function refusalHint(instance: Instance, propName: any, propValue: any): string
    if type(propName) ~= "string" then
        return ""
    end
    local isMember, current = pcall(function()
        return (instance :: any)[propName]
    end)
    local names = {}
    local typed = propName
    if not isMember then
        for _, realName in pairs(caseIndexFor(instance.ClassName)) do
            table.insert(names, realName)
        end
    elseif typeof(current) == "EnumItem" and type(propValue) == "string" then
        typed = string.gsub(propValue, "^Enum%..+%.", "")
        for _, item in ipairs(current.EnumType:GetEnumItems()) do
            table.insert(names, item.Name)
        end
    end
    local sentence = Suggest.didYouMean(Suggest.closest(typed, names))
    return if sentence then " " .. sentence else ""
end

Names.correctedName = correctedName
Names.refusalHint = refusalHint

return Names
