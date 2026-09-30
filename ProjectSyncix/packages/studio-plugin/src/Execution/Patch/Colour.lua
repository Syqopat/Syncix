--[[
	Colours written as text, and the check that the property really wants a Color3.
]]

local Colour = {}

-- Colour given as text.
--
-- A file or command may write a colour the way people type it ("#ff8800",
-- "255, 136, 0", "rgb(255, 136, 0)"), and Roblox only takes a Color3, so the
-- assignment was rejected. Three numbers are 0-1 unless one is above 1, which means
-- the 0-255 scale; "1, 1, 1" is white either way.
local function colorFromTriple(text: string): Color3?
    local a, b, c = string.match(text, "^([^,]+),([^,]+),([^,]+)$")
    if not a then
        return nil
    end
    local r = tonumber(string.match(a, "^%s*(.-)%s*$"))
    local g = tonumber(string.match(b, "^%s*(.-)%s*$"))
    local bl = tonumber(string.match(c, "^%s*(.-)%s*$"))
    if not (r and g and bl) or r < 0 or g < 0 or bl < 0 then
        return nil
    end
    if r > 1 or g > 1 or bl > 1 then
        if r > 255 or g > 255 or bl > 255 then
            return nil
        end
        return Color3.fromRGB(r, g, bl)
    end
    return Color3.new(r, g, bl)
end

local function colorFromText(text: string): Color3?
    local hex = string.match(text, "^%s*#(%x+)%s*$")
    if hex then
        if #hex == 6 then
            return Color3.fromRGB(
                tonumber(string.sub(hex, 1, 2), 16) :: number,
                tonumber(string.sub(hex, 3, 4), 16) :: number,
                tonumber(string.sub(hex, 5, 6), 16) :: number
            )
        elseif #hex == 3 then
            -- "#f80" is shorthand for "#ff8800": each digit is doubled.
            return Color3.fromRGB(
                (tonumber(string.sub(hex, 1, 1), 16) :: number) * 17,
                (tonumber(string.sub(hex, 2, 2), 16) :: number) * 17,
                (tonumber(string.sub(hex, 3, 3), 16) :: number) * 17
            )
        end
        return nil
    end
    local inner = string.match(text, "^%s*[Rr][Gg][Bb]%s*%((.*)%)%s*$")
    return colorFromTriple(inner or text)
end

-- True when the property currently holds a Color3. Only asked once the text parsed as a
-- colour, so ordinary strings never pay for the extra property read; it also keeps
-- "1, 2, 3" meant for a Vector3 away from the colour path.
local function isColor3Property(instance: Instance, propName: string): boolean
    local ok, current = pcall(function()
        return (instance :: any)[propName]
    end)
    return ok and typeof(current) == "Color3"
end

Colour.colorFromText = colorFromText
Colour.isColor3Property = isColor3Property

return Colour
