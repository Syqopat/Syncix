local Workspace = game:GetService("Workspace")
local CollectionService = game:GetService("CollectionService")

-- Upper bound on instances waiting for a parent, so a parent that never arrives
-- cannot grow the queue without limit.
local MAX_WAITING = 2000

local PatchExecutor = {}
PatchExecutor.__index = PatchExecutor

function PatchExecutor.new()
    local self = setmetatable({}, PatchExecutor)
    -- parent uuid -> list of functions that finish applying a child once it exists.
    --
    -- A child whose parent is not in Studio yet used to be put into Workspace. The
    -- core never learnt of it, so Studio and the sync folder silently disagreed
    -- (an imported Atmosphere or Sky ended up in Workspace). Now the child waits
    -- for its parent instead of landing somewhere it does not belong.
    self.waitingForParent = {}
    self.waitingCount = 0
    return self
end

-- Queues `apply` until the instance `parentId` is created by a later patch.
function PatchExecutor:WaitForParent(parentId: string, childName: string, apply: () -> ())
    if self.waitingCount >= MAX_WAITING then
        warn(string.format("[Syncix] %s was not created: its parent is not in Studio.", tostring(childName)))
        return
    end
    local queue = self.waitingForParent[parentId]
    if not queue then
        queue = {}
        self.waitingForParent[parentId] = queue
        warn(string.format(
            "[Syncix] %s is waiting for its parent (%s), which is not in Studio yet.",
            tostring(childName),
            string.sub(parentId, 1, 8)
        ))
    end
    table.insert(queue, apply)
    self.waitingCount += 1
end

-- Runs everything that waited for `uuid`, now that it exists.
function PatchExecutor:ReleaseChildren(uuid: string)
    local queue = self.waitingForParent[uuid]
    if not queue then
        return
    end
    self.waitingForParent[uuid] = nil
    self.waitingCount -= #queue
    for _, apply in ipairs(queue) do
        apply()
    end
end

function PatchExecutor:OnStart(container)
    self.cache = container:Get("RuntimeCache")
    self.echoGuard = container:Get("EchoGuard")
    self.activityLog = container:Get("ActivityLog")
    self.subscriptions = container:Get("SubscriptionManager")
    self.genericObserver = container:Get("GenericObserver")
    self.selectionObserver = container:Get("SelectionObserver")
end

function PatchExecutor:ApplyFullNode(nodeData: any)
    local uuid = nodeData.syncix_id
    local className = nodeData.class_name
    local name = nodeData.name
    local props = nodeData.properties
    local parentId = nodeData.parent
    
    local hasParent = parentId ~= nil and parentId ~= ""
    local targetParent = hasParent and self.cache:GetInstance(parentId) or nil

    local instance = self.cache:GetInstance(uuid)

    if not instance then
        if hasParent and not targetParent then
            self:WaitForParent(parentId, name, function()
                self:ApplyFullNode(nodeData)
            end)
            return
        end
        if not targetParent then
            -- Only services sit at the top, and they exist already; anything else
            -- without a parent has no place to go.
            warn(string.format("[Syncix] %s (%s) has no parent and was not created.", tostring(name), tostring(className)))
            return
        end

        local success, newInst = pcall(function() return Instance.new(className) end)
        if not success or not newInst then
            return
        end
        
        instance = newInst
        instance.Name = name
        pcall(function()
            instance:SetAttribute("__syncix_id", uuid)
        end)
        
        self.cache:CacheInstance(uuid, instance)
        
        local parentSuccess = pcall(function()
            instance.Parent = targetParent 
        end)

        if not parentSuccess then
            pcall(function() instance:Destroy() end)
            return
        end

        self.genericObserver:HandleInstanceAdded(instance)
    else
        instance.Name = name
        -- An unknown parent leaves the instance where it is rather than moving it
        -- somewhere it does not belong.
        if targetParent and instance.Parent ~= targetParent then
            pcall(function()
                instance.Parent = targetParent
            end)
        end
    end

    if props then
        for propName, propValue in pairs(props) do
            self:ApplyPropertyValue(instance, propName, propValue)
        end
    end

    self:ReleaseChildren(uuid)
end

function PatchExecutor:ApplyPatch(patch: any)
    local uuid = patch.data and (patch.data.syncix_id or patch.data.id)

    if patch.event_type == "CREATE" then
        if uuid and self.cache:GetInstance(uuid) then
            return
        end

        local parentId = patch.data.parent
        local targetParent = parentId and self.cache:GetInstance(parentId) or nil
        if not targetParent then
            if parentId then
                self:WaitForParent(parentId, patch.data.name or patch.data.class_name, function()
                    self:ApplyPatch(patch)
                end)
            else
                warn(string.format("[Syncix] %s has no parent and was not created.", tostring(patch.data.name)))
            end
            return
        end

        local ok, newInst = pcall(function()
            return Instance.new(patch.data.class_name)
        end)
        if not ok or not newInst then
            return
        end

        newInst.Name = patch.data.name or patch.data.class_name
        if uuid then
            pcall(function()
                newInst:SetAttribute("__syncix_id", uuid)
            end)
            self.cache:CacheInstance(uuid, newInst)
        end

        pcall(function()
            newInst.Parent = targetParent
        end)

        if self.activityLog then
            self.activityLog:Inbound("create", newInst.Name, nil, newInst.ClassName, uuid)
        end
        if uuid then
            self:ReleaseChildren(uuid)
        end
        return
    end

    -- SELECTION is NOT tied to an instance: it carries no uuid, because which objects
    -- are selected is global state. So it must be handled before both "exit if no uuid"
    -- and instance resolution. The first attempt put it one line BELOW the uuid
    -- check, and selection was silently dropped.
    if patch.event_type == "SELECTION_UPDATE" then
        if self.selectionObserver then
            self.selectionObserver:Apply(patch.data and patch.data.ids or {})
        end
        return
    end

    if not uuid then return end
    local instance = self.cache:GetInstance(uuid)
    if not instance then return end

    if patch.event_type == "PROPERTY_UPDATE" then
        self:ApplyPropertyValue(instance, patch.data.property, patch.data.value)
    elseif patch.event_type == "RENAME_INSTANCE" or patch.event_type == "RENAME" then
        local newName = patch.data.newName or patch.data.name or patch.data.value
        if newName then
            if self.echoGuard then self.echoGuard:Expect(uuid, "Name", newName) end
            if self.activityLog then
                self.activityLog:Inbound("rename", instance.Name, "Name", newName, uuid)
            end
            pcall(function()
                instance.Name = newName
            end)
        end
    elseif patch.event_type == "ATTRIBUTE_UPDATE" then
        local name = patch.data.name
        if name then
            local resolved = self:DecodeValue(patch.data.value)
            if self.echoGuard then self.echoGuard:Expect(uuid, "@" .. name, resolved) end
            pcall(function()
                instance:SetAttribute(name, resolved)
            end)
        end
    elseif patch.event_type == "TAGS_UPDATE" then
        -- The tag list arrives as a whole. Tracking single adds and removes
        -- would need separate state on both sides; instead
        -- the current set is brought to the requested set.
        local requested = {}
        for _, t in ipairs(patch.data.tags or {}) do
            requested[t] = true
        end
        pcall(function()
            for _, current in ipairs(CollectionService:GetTags(instance)) do
                if not requested[current] then
                    CollectionService:RemoveTag(instance, current)
                end
            end
            for t in pairs(requested) do
                if not CollectionService:HasTag(instance, t) then
                    CollectionService:AddTag(instance, t)
                end
            end
        end)
    elseif patch.event_type == "REPARENT" then
        local newParentUuid = patch.data.parent
        if newParentUuid then
            local pInst = self.cache:GetInstance(newParentUuid)
            if pInst then
                if self.echoGuard then self.echoGuard:Expect(uuid, "__parent", newParentUuid) end
                pcall(function()
                    instance.Parent = pInst
                end)
            end
        end
    elseif patch.event_type == "DESTROY" then
        if self.activityLog then
            self.activityLog:Inbound("delete", instance.Name, nil, nil, uuid)
        end
        pcall(function()
            instance:Destroy()
        end)
        self.cache:Remove(uuid)
    end
end

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

-- Linked (derived) properties.
--
-- In Roblox writing one property changes its siblings too: writing Position also changes
-- CFrame, Orientation and Rotation, and each produces its own Changed signal.
-- Echo protection only expected the property we wrote, so these derived
-- signals went back to the core as echoes.
--
-- Measured: 40 Position commands -> 39 "Orientation" updates came back.
-- Position itself was filtered correctly; only the derived ones leaked.
local LINKED = {
    Position    = { "CFrame", "Orientation", "Rotation" },
    CFrame      = { "Position", "Orientation", "Rotation" },
    Orientation = { "CFrame", "Position", "Rotation" },
    Rotation    = { "CFrame", "Position", "Orientation" },
    Size        = { "CFrame" },
}

-- AFTER writing, records the CURRENT values Roblox computed for the linked properties
-- as expectations. Values are recorded exactly, so real changes the user makes
-- later differ and are not filtered.
function PatchExecutor:_AwaitReferences(instance: Instance, propName: string)
    if not self.echoGuard then return end

    local siblings = LINKED[propName]
    if not siblings then return end

    local uuid = instance:GetAttribute("__syncix_id")
    if not uuid then return end

    for _, fieldName in ipairs(siblings) do
        pcall(function()
            local latest = (instance :: any)[fieldName]
            if latest ~= nil then
                self.echoGuard:Expect(uuid, fieldName, latest)
            end
        end)
    end
end

function PatchExecutor:ApplyPropertyValue(instance: Instance, propName: string, propValue: any)
    -- Echo protection: BEFORE applying, an "I am writing this value" note is made.
    -- Roblox's property signals are deferred, so a timing-based lock
    -- was not enough; the observer compares the incoming value with this note and filters our own write.
    if self.echoGuard then
        local uuid = instance:GetAttribute("__syncix_id")
        if uuid then
            local ok, resolved = pcall(function()
                return self:DecodeValue(propValue)
            end)
            local expectedVal = ok and resolved or propValue
            if propName == "CFrame" and typeof(expectedVal) == "Vector3" then
                expectedVal = CFrame.new(expectedVal)
            end
            self.echoGuard:Expect(uuid, propName, expectedVal)
        end
    end

    -- Errors must not be swallowed silently: they led to undiagnosable "value not applied" problems.
    local ok, err = pcall(function()
        if propName == "Contents" and instance:IsA("LocalizationTable") then
            -- Translation entries are written with SetEntries, not as a property.
            local HttpService = game:GetService("HttpService")
            local inputs = HttpService:JSONDecode(propValue)
            ;(instance :: any):SetEntries(inputs)
        elseif propName == "Source" and instance:IsA("LuaSourceContainer") then
            instance.Source = propValue
            pcall(function()
                local ScriptEditorService = game:GetService("ScriptEditorService")
                local doc = ScriptEditorService:FindScriptDocumentAsync(instance)
                if doc then
                    doc:EditTextAsync(propValue, 1, 1, doc:GetLineCount(), doc:GetLineLength(doc:GetLineCount()) + 1)
                end
            end)
        elseif propName == "Parent" then
            if type(propValue) == "string" and propValue ~= "" then
                local pInst = self.cache:GetInstance(propValue)
                if pInst then
                    instance.Parent = pInst
                end
            elseif propValue == "" or propValue == "Workspace" then
                instance.Parent = Workspace
            end
        elseif type(propValue) == "table" then
            -- IMPORTANT: the type is decided by the VALUE, NOT by the property NAME.
            -- "Size"/"Position" used to be taken for UDim2 every time, so a Part's
            -- Vector3 position could not be resolved and was silently never applied (objects stayed at 0,0,0).
            if propValue.Vector3 then
                if propName == "CFrame" then
                    instance.CFrame = CFrame.new(propValue.Vector3.x, propValue.Vector3.y, propValue.Vector3.z)
                else
                    instance[propName] = Vector3.new(propValue.Vector3.x, propValue.Vector3.y, propValue.Vector3.z)
                end
            elseif propValue.Color3 then
                instance[propName] = Color3.new(propValue.Color3.r, propValue.Color3.g, propValue.Color3.b)
            elseif propValue.UDim2 then
                instance[propName] = UDim2.new(propValue.UDim2.xs, propValue.UDim2.xo, propValue.UDim2.ys, propValue.UDim2.yo)
            elseif propValue.Vector2 then
                instance[propName] = Vector2.new(propValue.Vector2.x, propValue.Vector2.y)
            elseif propValue.UDim then
                instance[propName] = UDim.new(propValue.UDim.scale, propValue.UDim.offset)
            elseif propValue.CFrame then
                local p, r = propValue.CFrame.pos, propValue.CFrame.rot
                instance[propName] = CFrame.new(
                    p[1], p[2], p[3],
                    r[1], r[2], r[3],
                    r[4], r[5], r[6],
                    r[7], r[8], r[9]
                )
            elseif propValue.NumberRange then
                instance[propName] = NumberRange.new(propValue.NumberRange.min, propValue.NumberRange.max)
            elseif propValue.Number ~= nil then
                instance[propName] = propValue.Number
            elseif propValue.String ~= nil then
                local str = propValue.String
                local enumVal = decodeEnum(str)
                if enumVal ~= nil then
                    local ok2 = pcall(function()
                        instance[propName] = enumVal
                    end)
                    if not ok2 then
                        local shortName = string.gsub(str, "^Enum%..+%.", "")
                        local ok3 = pcall(function()
                            instance[propName] = shortName
                        end)
                        if not ok3 then
                            instance[propName] = str
                        end
                    end
                else
                    instance[propName] = str
                end
            elseif propValue.Boolean ~= nil then
                instance[propName] = propValue.Boolean
            elseif propValue.BrickColor then
                instance[propName] = BrickColor.new(propValue.BrickColor)
            elseif propValue.Content ~= nil then
                instance[propName] = decodeContent(propValue.Content)
            elseif propValue.ColorSequence then
                instance[propName] = decodeColorSequence(propValue.ColorSequence)
            elseif propValue.NumberSequence then
                instance[propName] = decodeNumberSequence(propValue.NumberSequence)
            elseif propValue.Rect then
                local r = propValue.Rect
                instance[propName] = Rect.new(r.min[1], r.min[2], r.max[1], r.max[2])
            elseif propValue.Font then
                instance[propName] = decodeFont(propValue.Font)
            elseif propValue.PhysicalProperties then
                local p = propValue.PhysicalProperties
                instance[propName] = PhysicalProperties.new(
                    p.density, p.friction, p.elasticity,
                    p.frictionWeight, p.elasticityWeight
                )
            elseif propValue.Ref ~= nil then
                instance[propName] = (propValue.Ref ~= "")
                    and self.cache:GetInstance(propValue.Ref)
                    or nil
            else
                error("unsupported table value for property: " .. propName)
            end
        elseif type(propValue) == "string" and (propName == "Size" or propName == "Position") and instance:IsA("GuiObject") then
            -- UDim2 in text form for GUI ("0.5,0,0.5,0")
            local udim = decodeUDim2(propValue)
            if udim then
                instance[propName] = udim
            end
        elseif propName == "AnchorPoint" then
            local vec2 = decodeVector2(propValue)
            if vec2 then
                instance[propName] = vec2
            end
        else
            local enumVal = decodeEnum(propValue)
            if enumVal ~= nil then
                local ok2 = pcall(function()
                    instance[propName] = enumVal
                end)
                if not ok2 then
                    if type(propValue) == "string" then
                        local shortName = string.gsub(propValue, "^Enum%..+%.", "")
                        local ok3 = pcall(function()
                            instance[propName] = shortName
                        end)
                        if not ok3 then
                            instance[propName] = propValue
                        end
                    else
                        instance[propName] = propValue
                    end
                end
            elseif propName == "CFrame" and typeof(propValue) == "Vector3" then
                instance.CFrame = CFrame.new(propValue)
            else
                instance[propName] = propValue
            end
        end
    end)

    if ok then
        -- If the write succeeded, the new values of derived properties are expected too.
        self:_AwaitReferences(instance, propName)

        -- Activity log: so the user can see what changed and is warned about conflicts.
        if self.activityLog then
            local applied = nil
            pcall(function()
                applied = (instance :: any)[propName]
            end)
            self.activityLog:Inbound(
                "property",
                instance.Name,
                propName,
                applied,
                instance:GetAttribute("__syncix_id")
            )
        end
    else
        warn(string.format(
            "[Syncix] Could not apply property: %s.%s -> %s",
            instance:GetFullName(),
            tostring(propName),
            tostring(err)
        ))
    end
end

return PatchExecutor
