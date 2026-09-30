--[[
	Wire value -> Roblox value. Every type the core can send arrives as plain JSON;
	this is where it becomes a Color3, a Font, an EnumItem or an instance reference.
]]

local Decode = {}

local PatchExecutor = {}

--- Asset reference. Older properties (Decal.Texture, SoundId) accept plain text;
--- newer Content-typed ones do not. Content is tried first,
--- falling back to text when that version does not exist.
local function decodeContent(uri: string): any
    local ok, content = pcall(function()
        return (Content :: any).fromUri(uri)
    end)
    if ok and content then
        return content
    end
    return uri
end

local function decodeColorSequence(points: any): ColorSequence?
    if type(points) ~= "table" or #points == 0 then
        return nil
    end
    local keyNames = {}
    for _, k in ipairs(points) do
        table.insert(keyNames, ColorSequenceKeypoint.new(k.t, Color3.new(k.r, k.g, k.b)))
    end
    -- Roblox requires the first point at 0 and the last at 1, and
    -- rejects an unsorted list.
    table.sort(keyNames, function(a, b) return a.Time < b.Time end)
    local ok, array = pcall(ColorSequence.new, keyNames)
    return ok and array or nil
end

local function decodeNumberSequence(points: any): NumberSequence?
    if type(points) ~= "table" or #points == 0 then
        return nil
    end
    local keyNames = {}
    for _, k in ipairs(points) do
        table.insert(keyNames, NumberSequenceKeypoint.new(k.t, k.v, k.envelope or 0))
    end
    table.sort(keyNames, function(a, b) return a.Time < b.Time end)
    local ok, array = pcall(NumberSequence.new, keyNames)
    return ok and array or nil
end

--- Font. The family is an asset URI; weight and style are enums.
--- If converting text to an enum fails, the default is used; rather than assign a
--- wrong value, Roblox's default is the right behaviour.
local function decodeFont(f: any): Font?
    local ok, ink = pcall(function()
        local weight = Enum.FontWeight.Regular
        local style = Enum.FontStyle.Normal
        for _, w in ipairs(Enum.FontWeight:GetEnumItems()) do
            if tostring(w) == f.weight then weight = w break end
        end
        for _, st in ipairs(Enum.FontStyle:GetEnumItems()) do
            if tostring(st) == f.style then style = st break end
        end
        return Font.new(f.family, weight, style)
    end)
    return ok and ink or nil
end

local function decodeEnum(propValue: any): any
    if typeof(propValue) == "EnumItem" then
        return propValue
    end
    if type(propValue) == "string" and string.sub(propValue, 1, 5) == "Enum." then
        local parts = string.split(propValue, ".")
        if #parts == 3 then
            local ok, ev = pcall(function()
                return (Enum :: any)[parts[2]][parts[3]]
            end)
            if ok and ev ~= nil then return ev end
        elseif #parts > 3 then
            local ok, ev = pcall(function()
                return (Enum :: any)[parts[2]][parts[#parts]]
            end)
            if ok and ev ~= nil then return ev end
        end
        return string.gsub(propValue, "^Enum%..+%.", "")
    end
    return nil
end

local function decodeUDim2(val: any): any
    if type(val) == "string" then
        local parts = string.split(val, ",")
        if #parts == 4 then
            local sx, ox, sy, oy = tonumber(parts[1]), tonumber(parts[2]), tonumber(parts[3]), tonumber(parts[4])
            if sx and ox and sy and oy then
                return UDim2.new(sx, ox, sy, oy)
            end
        end
    elseif type(val) == "table" and val.UDim2 then
        return UDim2.new(val.UDim2.xs or 0, val.UDim2.xo or 0, val.UDim2.ys or 0, val.UDim2.yo or 0)
    end
    return nil
end

local function decodeVector2(val: any): any
    if type(val) == "string" then
        local parts = string.split(val, ",")
        if #parts == 2 then
            local x, y = tonumber(parts[1]), tonumber(parts[2])
            if x and y then
                return Vector2.new(x, y)
            end
        end
    elseif type(val) == "table" and val.Vector2 then
        return Vector2.new(val.Vector2.x or 0, val.Vector2.y or 0)
    end
    return nil
end

function PatchExecutor:DecodeValue(propValue: any): any
    if type(propValue) == "table" then
        if propValue.Vector3 then
            return Vector3.new(propValue.Vector3.x, propValue.Vector3.y, propValue.Vector3.z)
        elseif propValue.Color3 then
            return Color3.new(propValue.Color3.r, propValue.Color3.g, propValue.Color3.b)
        elseif propValue.UDim2 then
            return UDim2.new(propValue.UDim2.xs, propValue.UDim2.xo, propValue.UDim2.ys, propValue.UDim2.yo)
        elseif propValue.Vector2 then
            return Vector2.new(propValue.Vector2.x, propValue.Vector2.y)
        elseif propValue.UDim then
            return UDim.new(propValue.UDim.scale, propValue.UDim.offset)
        elseif propValue.CFrame then
            local p, r = propValue.CFrame.pos, propValue.CFrame.rot
            return CFrame.new(
                p[1], p[2], p[3],
                r[1], r[2], r[3],
                r[4], r[5], r[6],
                r[7], r[8], r[9]
            )
        elseif propValue.NumberRange then
            return NumberRange.new(propValue.NumberRange.min, propValue.NumberRange.max)
        elseif propValue.Number ~= nil then
            -- An older core may be sending serde's tagged form
            -- ({"Number":0.5}). Newer cores send the plain form; this branch only
            -- guards against a version mismatch.
            return propValue.Number
        elseif propValue.String ~= nil then
            local ev = decodeEnum(propValue.String)
            if ev ~= nil and typeof(ev) == "EnumItem" then
                return ev
            end
            return propValue.String
        elseif propValue.Boolean ~= nil then
            return propValue.Boolean
        elseif propValue.BrickColor then
            return BrickColor.new(propValue.BrickColor)
        elseif propValue.Content ~= nil then
            return decodeContent(propValue.Content)
        elseif propValue.ColorSequence then
            return decodeColorSequence(propValue.ColorSequence)
        elseif propValue.NumberSequence then
            return decodeNumberSequence(propValue.NumberSequence)
        elseif propValue.Rect then
            local r = propValue.Rect
            return Rect.new(r.min[1], r.min[2], r.max[1], r.max[2])
        elseif propValue.Font then
            return decodeFont(propValue.Font)
        elseif propValue.PhysicalProperties then
            local p = propValue.PhysicalProperties
            return PhysicalProperties.new(
                p.density, p.friction, p.elasticity,
                p.frictionWeight, p.elasticityWeight
            )
        elseif propValue.Ref ~= nil then
            -- An empty string means "no reference"; returning nil is the right behaviour.
            if propValue.Ref == "" then
                return nil
            end
            return self.cache and self.cache:GetInstance(propValue.Ref) or nil
        end
        return nil
    end
    local enumVal = decodeEnum(propValue)
    if enumVal ~= nil then return enumVal end
    return propValue
end

Decode.decodeContent = decodeContent
Decode.decodeColorSequence = decodeColorSequence
Decode.decodeNumberSequence = decodeNumberSequence
Decode.decodeFont = decodeFont
Decode.decodeEnum = decodeEnum
Decode.decodeUDim2 = decodeUDim2
Decode.decodeVector2 = decodeVector2
Decode.Value = PatchExecutor.DecodeValue

return Decode
