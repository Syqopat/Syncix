--!strict
-- RuntimeCache
-- Keeps UUID <-> Instance mappings and O(1) lookup structures for fast access.

local RuntimeCache = {}
RuntimeCache.__index = RuntimeCache

function RuntimeCache.new()
    local self = setmetatable({}, RuntimeCache)
    
    -- Fast lookup from UUID to Instance
    self.uuidToInstance = {}
    
    -- Fast lookup from Instance to UUID
    self.instanceToUuid = {}
    
    -- Keeps "dirty" objects (changed but not sent yet)
    self.dirtyStates = {}
    
    return self
end

function RuntimeCache:OnInit(container)
    -- Other services are fetched from here when needed
end

-- Stores an instance in the cache. An instance filed under another identity before
-- leaves that entry, so one object is never reachable by two identities.
function RuntimeCache:CacheInstance(uuid: string, instance: Instance)
    local previous = self.instanceToUuid[instance]
    if previous and previous ~= uuid and self.uuidToInstance[previous] == instance then
        self.uuidToInstance[previous] = nil
    end
    self.uuidToInstance[uuid] = instance
    self.instanceToUuid[instance] = uuid
end

local function identityOf(instance: Instance): any
    local ok, value = pcall(function()
        return instance:GetAttribute("__syncix_id")
    end)
    return ok and value or nil
end

local function isLive(instance: Instance): boolean
    local ok, live = pcall(function()
        return instance:IsDescendantOf(game)
    end)
    return ok and live
end

-- The live object that carries `uuid` in its __syncix_id, searched in the synced services.
function RuntimeCache:_FindByIdentity(uuid: string): Instance?
    local Services = require(script.Parent.Parent.Observer.Services)
    for _, service in ipairs(Services.List()) do
        if identityOf(service) == uuid then
            return service
        end
        for _, d in ipairs(service:GetDescendants()) do
            if identityOf(d) == uuid then
                return d
            end
        end
    end
    return nil
end

-- Removes an instance from the cache (after DESTROY).
function RuntimeCache:Remove(uuid: string)
    local instance = self.uuidToInstance[uuid]
    if instance then
        self.instanceToUuid[instance] = nil
    end
    self.uuidToInstance[uuid] = nil
    self.dirtyStates[uuid] = nil
end

-- Returns the Instance for a UUID.
--
-- The entry is checked against the object's own __syncix_id. The two could drift apart
-- (a Ctrl+D copy filed under its original's identity by an older plugin), and then a
-- command meant for one object moved another while `pull` reported everything
-- consistent, because the tree is read from the attributes. On a mismatch the object
-- that really carries the identity is looked up and the cache repaired; the object
-- filed wrongly goes back under its own identity.
function RuntimeCache:GetInstance(uuid: string): Instance?
    local instance = self.uuidToInstance[uuid]
    if instance == nil then
        return nil
    end
    local carried = identityOf(instance)
    if carried == nil or carried == uuid or not isLive(instance) then
        return instance
    end
    local holder = self:_FindByIdentity(uuid)
    if holder == nil then
        -- Nobody else carries it: this is the object (its attribute is being settled,
        -- see GenericObserver.HandleIdentityChanged).
        return instance
    end
    self:CacheInstance(uuid, holder)
    local other = self.uuidToInstance[carried]
    if other == nil or not isLive(other) then
        self:CacheInstance(carried, instance)
    end
    warn(string.format(
        "[Syncix] Repaired an identity mix-up: %s now points at %s, not %s.",
        uuid, holder:GetFullName(), instance:GetFullName()
    ))
    return holder
end

-- Returns the UUID for an Instance.
function RuntimeCache:GetUuid(instance: Instance): string?
    return self.instanceToUuid[instance]
end

-- Marks an instance as "dirty" (to be sent over the network)
function RuntimeCache:MarkDirty(uuid: string)
    self.dirtyStates[uuid] = true
end

-- Clears the "dirty" flag after sending.
function RuntimeCache:ClearDirty(uuid: string)
    self.dirtyStates[uuid] = nil
end

-- Returns the UUIDs of all "dirty" (changed) objects.
function RuntimeCache:GetDirtyUuids()
    local uuids = {}
    for uuid, _ in pairs(self.dirtyStates) do
        table.insert(uuids, uuid)
    end
    return uuids
end

return RuntimeCache
