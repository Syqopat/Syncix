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

-- Stores an instance in the cache.
function RuntimeCache:CacheInstance(uuid: string, instance: Instance)
    self.uuidToInstance[uuid] = instance
    self.instanceToUuid[instance] = uuid
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
function RuntimeCache:GetInstance(uuid: string): Instance?
    return self.uuidToInstance[uuid]
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
