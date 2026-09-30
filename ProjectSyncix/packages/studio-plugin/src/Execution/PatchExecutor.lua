local Workspace = game:GetService("Workspace")
local CollectionService = game:GetService("CollectionService")

local Creation = require(script.Parent.Patch.Creation)
local Decode = require(script.Parent.Patch.Decode)
local applyPropertyValue = require(script.Parent.Patch.Write)

local wireText = Creation.wireText
local newInstanceFor = Creation.newInstanceFor
local smoothNewPart = Creation.smoothNewPart

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

        local success, newInst = pcall(function()
            return newInstanceFor(className, {
                name = name,
                mesh_id = wireText(props, "MeshId") or wireText(props, "MeshContent"),
                collision_fidelity = wireText(props, "CollisionFidelity"),
                render_fidelity = wireText(props, "RenderFidelity"),
            })
        end)
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
            return newInstanceFor(patch.data.class_name, patch.data)
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

--- Wire value -> Roblox value (see Patch/Decode.lua).
function PatchExecutor:DecodeValue(propValue: any): any
    return Decode.Value(self, propValue)
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

--- Writes one property (see Patch/Write.lua).
function PatchExecutor:ApplyPropertyValue(instance: Instance, propName: string, propValue: any)
    return applyPropertyValue(self, instance, propName, propValue)
end


return PatchExecutor
