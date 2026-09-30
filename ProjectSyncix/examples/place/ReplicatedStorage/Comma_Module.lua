local CommaModule = {}

local SUFFIXES = {
    { 1e33, "Dc" },
    { 1e30, "No" },
    { 1e27, "Oc" },
    { 1e24, "Sp" },
    { 1e21, "Sx" },
    { 1e18, "Qi" },
    { 1e15, "Qa" },
    { 1e12, "T" },
    { 1e9, "B" },
    { 1e6, "M" },
    { 1e3, "k" },
}

function CommaModule.Comma(value)
    local n = tonumber(value)
    if not n then
        return tostring(value)
    end

    -- Abbreviate large numbers like 20 Sx
    if n >= 1e6 then
        for _, s in ipairs(SUFFIXES) do
            if n >= s[1] then
                local formatted = string.format("%.2f", n / s[1])
                -- Remove trailing .00
                formatted = formatted:gsub("%.00$", ""):gsub("(%..-)0+$", "%1")
                return formatted .. " " .. s[2]
            end
        end
    end

    -- Under 1M, format with comma separators
    local formatted = string.format("%.0f", n)
    local k
    while true do
        formatted, k = string.gsub(formatted, "^(-?%d+)(%d%d%d)", "%1,%2")
        if k == 0 then break end
    end
    return formatted
end

return CommaModule
