--!strict
-- PatchBuilder
-- Takes raw changes from the Observer and turns them into standardised patch objects.
-- Prepares the JSON data that goes over the network.

local HttpService = game:GetService("HttpService")
local CollectionService = game:GetService("CollectionService")
local PlaceIdentity = require(script.Parent.Parent.Core.PlaceIdentity)

local PatchBuilder = {}
PatchBuilder.__index = PatchBuilder

function PatchBuilder.new()
    local self = setmetatable({}, PatchBuilder)
    return self
end

function PatchBuilder:OnStart(container)
    self.cache = container:Get("RuntimeCache")
end

-- Turns a single property change into a patch.
function PatchBuilder:BuildPropertyPatch(uuid: string, propertyName: string, newValue: any): any
    local patch = {
        event_type = "PROPERTY_UPDATE",
        version = "v1",
        data = {
            syncix_id = uuid,
            property = propertyName,
            value = self:SerializeValue(newValue)
        }
    }
    return patch
end

-- Produces a LifecyclePatch for a newly created object.
function PatchBuilder:BuildLifecyclePatch(uuid: string, instance: Instance, eventType: string): any
    -- Find the parent UUID (so the hierarchy is built correctly on the VS Code side)
    local parentUuid = nil
    if instance.Parent then
        parentUuid = instance.Parent:GetAttribute("__syncix_id")
    end

    local patch = {
        event_type = eventType, -- "CREATE" or "DESTROY"
        version = "v1",
        data = {
            syncix_id = uuid,
            class_name = instance.ClassName,
            name = instance.Name,
            parent = parentUuid
        }
    }

    -- For a script, add its source too (so it reaches the editor on creation)
    if eventType == "CREATE" and instance:IsA("LuaSourceContainer") then
        local ok, src = pcall(function() return (instance :: any).Source end)
        if ok then
            patch.data.source = src
        end
    end

    -- Add attributes and properties (on CREATE)
    if eventType == "CREATE" then
        local attrs = self:SerializeAttributes(instance)
        if attrs then
            patch.data.attributes = attrs
        end
        local labels = self:SerializeTags(instance)
        if labels then
            patch.data.tags = labels
        end
        local props = self:SerializeProperties(instance)
        if props then
            patch.data.properties = props
        end
    end

    return patch
end

-- Serialises an object's attributes (__syncix_id excluded). Returns nil when empty.
function PatchBuilder:SerializeAttributes(instance: Instance): any
    local attrs = {}
    local count = 0
    for name, value in pairs(instance:GetAttributes()) do
        -- __syncix_id and __syncix_place are Syncix's OWN bookkeeping, not content.
        -- Written to disk, the place identity would leak into files and could be copied
        -- to another place — and since the identity's only job is to tell places apart,
        -- that would make it useless.
        if name ~= "__syncix_id" and name ~= "__syncix_place" then
            local sv = self:SerializeValue(value)
            if sv ~= nil then
                attrs[name] = sv
                count += 1
            end
        end
    end
    if count == 0 then return nil end
    return attrs
end

-- Returns an object's CollectionService tags. nil when it has none.
--
-- Tags are not properties: in Roblox they do not live on the instance as a field;
-- CollectionService keeps them. So the property table
-- could never see them, and the logic of tag-based games was completely invisible
-- in the editor.
function PatchBuilder:SerializeTags(instance: Instance): any
    local ok, labels = pcall(function()
        return CollectionService:GetTags(instance)
    end)
    if not ok or not labels or #labels == 0 then
        return nil
    end
    -- Sent sorted: the same set of tags should produce the same list every time
    -- so no needless "changed" patch comes out.
    table.sort(labels)
    return labels
end

-- Produces an ATTRIBUTE_UPDATE patch for a single attribute change.
function PatchBuilder:BuildAttributePatch(uuid: string, attrName: string, value: any): any
    return {
        event_type = "ATTRIBUTE_UPDATE",
        version = "v1",
        data = {
            syncix_id = uuid,
            name = attrName,
            value = self:SerializeValue(value)
        }
    }
end

-- Produces a REPARENT patch when an object's parent changes (a move).
function PatchBuilder:BuildReparentPatch(uuid: string, parentUuid: string): any
    return {
        event_type = "REPARENT",
        version = "v1",
        data = {
            syncix_id = uuid,
            parent = parentUuid
        }
    }
end

-- Serialises according to the incoming value's type (Vector3, Color3, Enum, string, number, ...)
function PatchBuilder:SerializeValue(value: any): any
    local t = typeof(value)
    if t == "Vector3" then
        return { Vector3 = { x = value.X, y = value.Y, z = value.Z } }
    elseif t == "Color3" then
        return { Color3 = { r = value.R, g = value.G, b = value.B } }
    elseif t == "EnumItem" then
        return tostring(value) -- "Enum.Material.Plastic" (as a string)
    elseif t == "UDim2" then
        return { UDim2 = { xs = value.X.Scale, xo = value.X.Offset, ys = value.Y.Scale, yo = value.Y.Offset } }
    elseif t == "Vector2" then
        return { Vector2 = { x = value.X, y = value.Y } }
    elseif t == "UDim" then
        return { UDim = { scale = value.Scale, offset = value.Offset } }
    elseif t == "CFrame" then
        -- Position + 3x3 rotation matrix. Orientation alone is not enough: there are
        -- orientations Euler angles cannot express.
        local x, y, z,
              r00, r01, r02,
              r10, r11, r12,
              r20, r21, r22 = value:GetComponents()
        return {
            CFrame = {
                pos = { x, y, z },
                rot = { r00, r01, r02, r10, r11, r12, r20, r21, r22 },
            },
        }
    elseif t == "NumberRange" then
        return { NumberRange = { min = value.Min, max = value.Max } }
    elseif t == "BrickColor" then
        -- It cannot be sent as plain text: on the other side
        -- `part.BrickColor = "Really red"` silently fails.
        return { BrickColor = value.Name }
    elseif t == "Content" then
        -- Roblox's newer Content type. Older ones (Decal.Texture, SoundId) still
        -- return plain text, so they go through the string branch below.
        return { Content = value.Uri or "" }
    elseif t == "ColorSequence" then
        local points = {}
        for _, k in ipairs(value.Keypoints) do
            table.insert(points, {
                t = k.Time,
                r = k.Value.R,
                g = k.Value.G,
                b = k.Value.B,
            })
        end
        return { ColorSequence = points }
    elseif t == "NumberSequence" then
        local points = {}
        for _, k in ipairs(value.Keypoints) do
            -- Envelope Roblox'un rastgelelik payi; atlanirsa parcacik efekti
            -- duzlesir, o yuzden tasiniyor.
            table.insert(points, { t = k.Time, v = k.Value, envelope = k.Envelope })
        end
        return { NumberSequence = points }
    elseif t == "Rect" then
        return {
            Rect = {
                min = { value.Min.X, value.Min.Y },
                max = { value.Max.X, value.Max.Y },
            },
        }
    elseif t == "Font" then
        return {
            Font = {
                family = value.Family,
                weight = tostring(value.Weight),
                style = tostring(value.Style),
            },
        }
    elseif t == "PhysicalProperties" then
        return {
            PhysicalProperties = {
                density = value.Density,
                friction = value.Friction,
                elasticity = value.Elasticity,
                frictionWeight = value.FrictionWeight,
                elasticityWeight = value.ElasticityWeight,
            },
        }
    elseif t == "Instance" then
        -- Reference to another object: the target's UUID is carried.
        -- Sent as text, the other side would take it for plain text and
        -- never turn it into an instance; hence the separate wrapper.
        local targetId = value:GetAttribute("__syncix_id")
        if targetId then
            return { Ref = tostring(targetId) }
        end
        return nil
    elseif t == "string" or t == "number" or t == "boolean" then
        return value
    end
    -- An empty reference (such as ObjectValue.Value = nil) carries information: "no link".
    if value == nil then
        return { Ref = "" }
    end
    -- For unsupported types
    return nil
end

-- Property lists synced automatically per class.
local PropertyTable = require(script.Parent.PropertyTable)
local Services = require(script.Parent.Services)

-- Property list per class, with inheritance merged and CACHED.
--
-- These lists used to be written by hand and covered only 12 classes; Model,
-- Humanoid, ParticleEmitter and everything else synced only structurally,
-- not a single setting went across. Now the list is generated from Roblox's API dump
-- (tools/gen-properties.py) and covers every class in the dump.
--
-- The cache matters: the inheritance chain is resolved once per CLASS, not once
-- per instance.
local classCache = {}

local function classProperties(className: string)
	local memo = classCache[className]
	if memo then
		return memo
	end

	local merged = {}
	local current = className
	local guard = 0
	while current and guard < 64 do
		local entry = PropertyTable[current]
		if entry then
			for fieldName, kind in pairs(entry.p) do
				-- A subclass OVERRIDES its parent class: when the same name appears in both,
				-- the subclass's definition wins.
				if merged[fieldName] == nil then
					merged[fieldName] = kind
				end
			end
			current = entry.u
		else
			break
		end
		guard += 1
	end

	classCache[className] = merged
	return merged
end



-- Serialises every watched property of an instance (property name -> value). nil when empty.
function PatchBuilder:SerializeProperties(instance: Instance): any
    -- LocalizationTable has NO property called "Contents"; the first attempt assumed
    -- so and Studio said "Contents is not a valid member".
    -- The right API is GetEntries()/SetEntries(). The translation entries are turned into
    -- a JSON string and carried as if they were a property; that way the existing String
    -- property path and the .csv file format work as they are.
    if instance:IsA("LocalizationTable") then
        local ok, json = pcall(function()
            local HttpService = game:GetService("HttpService")
            return HttpService:JSONEncode((instance :: any):GetEntries())
        end)
        if ok and json then
            return { Contents = json }
        end
        return nil
    end

    local list = classProperties(instance.ClassName)
    local props = {}
    local count = 0
    for fieldName in pairs(list) do
        local ok, val = pcall(function() return (instance :: any)[fieldName] end)
        if ok then
            local sv = self:SerializeValue(val)
            if sv ~= nil then
                props[fieldName] = sv
                count += 1
            end
        end
    end
    if count == 0 then return nil end
    return props
end

-- Returns whether a property name is watched automatically for this instance.
function PatchBuilder:IsWatchedProperty(instance: Instance, propName: string): boolean
    return classProperties(instance.ClassName)[propName] ~= nil
end

-- Walks the whole tree and produces the FULL_SYNC JSON object
function PatchBuilder:BuildFullTreeSnapshot(): any
    local servicesToSync = Services.List()
    
    local instances = {}

    -- 1. First add the services themselves as root nodes.
    -- Services get UUIDs too, so the hierarchy is carried by UUID end to end.
    for _, service in ipairs(servicesToSync) do
        local serviceUuid = service:GetAttribute("__syncix_id")
        if not serviceUuid then
            serviceUuid = HttpService:GenerateGUID(false)
            service:SetAttribute("__syncix_id", serviceUuid)
        end
        if self.cache then
            self.cache:CacheInstance(serviceUuid, service)
        end
        -- The service's OWN properties are carried too. For a long time only its children
        -- were sent; settings that define how a game looks, such as Lighting.ClockTime,
        -- Lighting.Ambient and Workspace.Gravity, never showed up in the editor.
        table.insert(instances, {
            syncix_id = serviceUuid,
            class_name = service.ClassName,
            name = service.Name,
            properties = self:SerializeProperties(service)
            -- no parent field: root node
        })
    end

    local function serializeNode(instance: Instance)
        local uuid = instance:GetAttribute("__syncix_id")
        if not uuid then return end

        local parentUuid = nil
        if instance.Parent and instance.Parent:GetAttribute("__syncix_id") then
            parentUuid = instance.Parent:GetAttribute("__syncix_id")
        end

        local nodeData = {
            syncix_id = uuid,
            class_name = instance.ClassName,
            name = instance.Name,
            parent = parentUuid
        }
        
        -- Serialize properties (wide kapsam)
        pcall(function()
            local props = self:SerializeProperties(instance)
            if props then
                nodeData.properties = props
            end
            if instance:IsA("LuaSourceContainer") then
                -- Send script source in the bootstrap so it can be opened as .lua in the editor
                nodeData.source = (instance :: any).Source
            end
            -- Send attributes in the bootstrap
            local attrs = self:SerializeAttributes(instance)
            if attrs then
                nodeData.attributes = attrs
            end
            local labels = self:SerializeTags(instance)
            if labels then
                nodeData.tags = labels
            end
        end)
        
        table.insert(instances, nodeData)
    end
    
    -- Iterate through all configured services
    for _, service in ipairs(servicesToSync) do
        for _, descendant in ipairs(service:GetDescendants()) do
            serializeNode(descendant)
        end
    end
    
    local snapshotPatch = {
        event_type = "FULL_SYNC",
        version = "v1",
        data = {
            instances = instances,
            place_key = PlaceIdentity.Resolve(),
            place_id = tostring(game.PlaceId),
            place_name = game.Name,
        }
    }
    
    return snapshotPatch
end

return PatchBuilder
