local Workspace = game:GetService("Workspace")
local CollectionService = game:GetService("CollectionService")

-- The generated class -> property table, used only to correct a property name
-- given in the wrong case (see correctedName). PatchBuilder loads it anyway.
local PropertyTable = require(script.Parent.Parent.Observer.PropertyTable)

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
    -- uuid -> true for every instance parked above. They go out with a FULL_SYNC so the
    -- core keeps them: Studio has received them but not built them yet.
    self.waitingIds = {}
    return self
end

-- Queues `apply` until the instance `parentId` is created by a later patch.
function PatchExecutor:WaitForParent(parentId: string, childName: string, childId: string?, apply: () -> ())
    if self.waitingCount >= MAX_WAITING then
        self:ReportNotCreated(childId, childName, nil, "its parent is not in Studio")
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
    table.insert(queue, { id = childId, apply = apply })
    self.waitingCount += 1
    if childId then
        self.waitingIds[childId] = true
    end
end

-- The instances parked until their parent exists.
function PatchExecutor:WaitingIds(): { string }
    local ids = {}
    for id in pairs(self.waitingIds) do
        table.insert(ids, id)
    end
    return ids
end

-- Runs everything that waited for `uuid`, now that it exists.
function PatchExecutor:ReleaseChildren(uuid: string)
    local queue = self.waitingForParent[uuid]
    if not queue then
        return
    end
    self.waitingForParent[uuid] = nil
    self.waitingCount -= #queue
    for _, entry in ipairs(queue) do
        if entry.id then
            self.waitingIds[entry.id] = nil
        end
        -- The whole queue is already off the books. An error from one child used to skip
        -- its siblings: never built, yet listed in waitingIds, so every FULL_SYNC asked the
        -- core to keep them.
        local ok, failure = pcall(entry.apply)
        if not ok then
            warn(string.format(
                "[Syncix] A child of %s could not be built: %s",
                string.sub(uuid, 1, 8),
                tostring(failure)
            ))
        end
    end
end

-- Forgets everything parked under `uuid`, and what was parked under those in turn: none
-- of it can be built now. The core drops the whole subtree when told about `uuid`; with
-- only one level forgotten, grandchildren stayed in waitingIds and in waitingCount for
-- good.
function PatchExecutor:_DropWaiting(uuid: string)
    local queue = self.waitingForParent[uuid]
    if not queue then
        return
    end
    -- Cleared before recursing, so a cycle in bad data still ends.
    self.waitingForParent[uuid] = nil
    self.waitingCount -= #queue
    for _, entry in ipairs(queue) do
        if entry.id then
            self.waitingIds[entry.id] = nil
            self:_DropWaiting(entry.id)
        end
    end
end

function PatchExecutor:OnStart(container)
    self.cache = container:Get("RuntimeCache")
    self.echoGuard = container:Get("EchoGuard")
    self.activityLog = container:Get("ActivityLog")
    self.subscriptions = container:Get("SubscriptionManager")
    self.genericObserver = container:Get("GenericObserver")
    self.selectionObserver = container:Get("SelectionObserver")
    self.batchQueue = container:Get("BatchQueue")
end

-- A class Studio cannot create (BubbleChatConfiguration, StarterPlayerScripts, ...)
-- exists once under its parent. If that one has no identity yet, it is the
-- instance meant, and it is adopted rather than reported missing.
function PatchExecutor:AdoptExisting(parent: Instance, className: string, uuid: string): Instance?
    local existing = parent:FindFirstChildOfClass(className)
    if existing and not self.cache:GetUuid(existing) then
        pcall(function()
            existing:SetAttribute("__syncix_id", uuid)
        end)
        self.cache:CacheInstance(uuid, existing)
        return existing
    end
    return nil
end

-- Tells the core an instance could not be created, the way a deletion in Studio
-- is told. The core drops it with its subtree and moves its files to the trash.
-- Before this the core kept a phantom Studio never had: an imported
-- BubbleChatConfiguration copy stayed in the model, and its UIGradient waited
-- for a parent that would never come.
function PatchExecutor:ReportNotCreated(uuid: string?, name: any, className: any, reason: string)
    warn(string.format(
        "[Syncix] %s (%s) could not be created in Studio: %s. It was removed from the sync (syncix trash).",
        tostring(name),
        tostring(className),
        reason
    ))
    if not uuid or uuid == "" then
        return
    end
    -- Its children waited for it; the core removes them together with it.
    self.waitingIds[uuid] = nil
    self:_DropWaiting(uuid)
    if self.batchQueue then
        self.batchQueue:Enqueue({
            event_type = "DESTROY",
            version = "v1",
            data = { syncix_id = uuid, class_name = className, name = name },
        })
    end
end

-- Instance.new("Part") still gives Roblox's legacy surfaces (Studs on top, Inlet
-- underneath), unlike a part inserted by hand in Studio, so every part Syncix
-- created from a file or a command came out studded. Called right after
-- Instance.new, before any property is applied, so a file or the core that names
-- a surface still has the last word.
local function smoothNewPart(instance: Instance)
    if instance:IsA("BasePart") then
        pcall(function()
            local part = instance :: any
            part.TopSurface = Enum.SurfaceType.Smooth
            part.BottomSurface = Enum.SurfaceType.Smooth
        end)
    end
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
            self:WaitForParent(parentId, name, uuid, function()
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
            newInst = self:AdoptExisting(targetParent, className, uuid)
            if not newInst then
                self:ReportNotCreated(uuid, name, className, "Studio cannot create this class")
                return
            end
        else
            smoothNewPart(newInst)
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
            self.cache:Remove(uuid)
            self:ReportNotCreated(uuid, name, className, "Studio refused its parent")
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
                self:WaitForParent(parentId, patch.data.name or patch.data.class_name, uuid, function()
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
            newInst = uuid and self:AdoptExisting(targetParent, patch.data.class_name, uuid) or nil
            if not newInst then
                self:ReportNotCreated(uuid, patch.data.name, patch.data.class_name, "Studio cannot create this class")
                return
            end
        else
            smoothNewPart(newInst)
        end

        newInst.Name = patch.data.name or patch.data.class_name
        if uuid then
            pcall(function()
                newInst:SetAttribute("__syncix_id", uuid)
            end)
            self.cache:CacheInstance(uuid, newInst)
        end

        local parented = pcall(function()
            newInst.Parent = targetParent
        end)
        if not parented then
            pcall(function() newInst:Destroy() end)
            if uuid then
                self.cache:Remove(uuid)
            end
            self:ReportNotCreated(uuid, patch.data.name, patch.data.class_name, "Studio refused its parent")
            return
        end

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

function PatchExecutor:ApplyPropertyValue(instance: Instance, propName: string, propValue: any)
    -- Turned into a Color3 before the echo note below, so the note holds the value
    -- Studio will actually report back.
    if type(propValue) == "string" then
        local color = colorFromText(propValue)
        if color and isColor3Property(instance, propName) then
            propValue = color
        end
    end

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
        -- Retried once under the right name; the retry cannot correct again because
        -- the right name is its own spelling.
        local realName = correctedName(instance, propName)
        if realName then
            self:ApplyPropertyValue(instance, realName, propValue)
            return
        end
        warn(string.format(
            "[Syncix] Could not apply property: %s.%s -> %s",
            instance:GetFullName(),
            tostring(propName),
            tostring(err)
        ))
    end
end

return PatchExecutor
