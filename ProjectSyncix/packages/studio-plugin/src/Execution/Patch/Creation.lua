--[[
	Making the instance itself: a MeshPart from its mesh, and the surfaces a part gets
	when Instance.new creates it.
]]

local Creation = {}

-- Instance.new("Part") still gives Roblox's legacy surfaces (Studs on top, Inlet
-- underneath), unlike a part inserted by hand in Studio, so every part Syncix
-- created from a file or a command came out studded. Called right after
-- Instance.new, before any property is applied, so a file or the core that names
-- a surface still has the last word.
-- Roblox does not let a plugin write MeshPart.MeshId: a mesh is what the part is MADE of,
-- and the only way to make one is InsertService:CreateMeshPartAsync. Without this a model
-- brought in from files arrived as grey boxes. The call yields and can fail (no network,
-- an asset that is not ours); then a plain MeshPart is created and the mesh is named in
-- the warning, so it is clear which one to bring in by hand.
local function meshPartFrom(data: any): Instance?
    local meshId = data and data.mesh_id
    if type(meshId) ~= "string" or meshId == "" then
        return nil
    end
    local function fidelity(enumType, value: any, fallback: EnumItem): EnumItem
        local name = type(value) == "string" and value:match("([^.]+)$") or nil
        return (name and (enumType :: any)[name]) or fallback
    end
    local collision = fidelity(Enum.CollisionFidelity, data.collision_fidelity, Enum.CollisionFidelity.Default)
    local render = fidelity(Enum.RenderFidelity, data.render_fidelity, Enum.RenderFidelity.Automatic)

    local ok, part = pcall(function()
        return game:GetService("InsertService"):CreateMeshPartAsync(meshId, collision, render)
    end)
    if ok and part then
        return part
    end
    warn(string.format(
        "[Syncix] The mesh %s could not be loaded, so %s was created as a plain MeshPart. %s",
        meshId, tostring(data.name), tostring(part)
    ))
    return nil
end

--- The text inside a wire value ({ Content = "rbxassetid://1" }, { String = "..." } or a
--- plain string), for the few values that are needed BEFORE an object exists.
local function wireText(properties: any, key: string): string?
    local value = properties and properties[key]
    if type(value) == "string" then
        return value
    end
    if type(value) == "table" then
        local text = value.Content or value.String
        return type(text) == "string" and text or nil
    end
    return nil
end

--- Instance.new, except that a MeshPart is created from its mesh when one came with it.
local function newInstanceFor(className: string, data: any): Instance
    if className == "MeshPart" then
        local part = meshPartFrom(data)
        if part then
            return part
        end
    end
    return Instance.new(className)
end

local function smoothNewPart(instance: Instance)
    if instance:IsA("BasePart") then
        pcall(function()
            local part = instance :: any
            part.TopSurface = Enum.SurfaceType.Smooth
            part.BottomSurface = Enum.SurfaceType.Smooth
        end)
    end
end

Creation.wireText = wireText
Creation.newInstanceFor = newInstanceFor
Creation.smoothNewPart = smoothNewPart

return Creation
