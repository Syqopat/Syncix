-- "Did you mean ...?" for names Studio refused: a property that is not a member, an
-- enum item that does not exist. Mirrors core-engine/src/suggest.rs, so the terminal and
-- the Output window offer the same spellings.

local Suggest = {}

-- Edit distance, case-insensitive; swapping two neighbours ("Szie") counts one.
function Suggest.distance(a: string, b: string): number
    a, b = string.lower(a), string.lower(b)
    local la, lb = #a, #b
    local d: { [number]: { [number]: number } } = {}
    for i = 0, la do
        d[i] = { [0] = i }
    end
    for j = 0, lb do
        d[0][j] = j
    end
    for i = 1, la do
        local ca = string.byte(a, i)
        for j = 1, lb do
            local cb = string.byte(b, j)
            local cost = if ca == cb then 0 else 1
            local best = math.min(d[i - 1][j] + 1, d[i][j - 1] + 1, d[i - 1][j - 1] + cost)
            if i > 1 and j > 1 and ca == string.byte(b, j - 1) and string.byte(a, i - 1) == cb then
                best = math.min(best, d[i - 2][j - 2] + 1)
            end
            d[i][j] = best
        end
    end
    return d[la][lb]
end

-- How far a spelling may be and still be offered: one slip in a short word, more in a long one.
local function allowed(length: number): number
    if length <= 2 then
        return 0
    elseif length <= 4 then
        return 1
    elseif length <= 8 then
        return 2
    end
    return 3
end

-- The closest candidates, best first, at most three; a candidate that starts with what
-- was typed counts after every near spelling.
function Suggest.closest(typed: string, candidates: { string }): { string }
    local limit = allowed(#typed)
    local lower = string.lower(typed)
    local scored = {}
    local seen = {}
    for _, candidate in ipairs(candidates) do
        local key = string.lower(candidate)
        if not seen[key] then
            local score = nil
            if math.abs(#candidate - #typed) <= limit then
                local distance = Suggest.distance(typed, candidate)
                if distance <= limit then
                    score = distance
                end
            end
            if score == nil and #typed >= 4 and string.sub(key, 1, #lower) == lower then
                score = limit + 1
            end
            if score ~= nil then
                seen[key] = true
                table.insert(scored, { score = score, name = candidate })
            end
        end
    end
    table.sort(scored, function(x, y)
        if x.score ~= y.score then
            return x.score < y.score
        end
        if #x.name ~= #y.name then
            return #x.name < #y.name
        end
        return x.name < y.name
    end)
    local out = {}
    for _, entry in ipairs(scored) do
        if entry.score ~= scored[1].score or #out >= 3 then
            break
        end
        table.insert(out, entry.name)
    end
    return out
end

-- "Did you mean X?", "Did you mean X or Y?", "Did you mean X, Y or Z?"; nil for none.
function Suggest.didYouMean(options: { string }): string?
    if #options == 0 then
        return nil
    elseif #options == 1 then
        return "Did you mean " .. options[1] .. "?"
    end
    return "Did you mean " .. table.concat(options, ", ", 1, #options - 1) .. " or " .. options[#options] .. "?"
end

return Suggest
