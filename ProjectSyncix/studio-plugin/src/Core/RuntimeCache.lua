--!strict
-- RuntimeCache
-- Hızlı erişim için UUID <-> Instance eşleşmelerini ve O(1) arama yapılarını tutar.

local RuntimeCache = {}
RuntimeCache.__index = RuntimeCache

function RuntimeCache.new()
    local self = setmetatable({}, RuntimeCache)
    
    -- UUID'den Instance'a hızlı erişim
    self.uuidToInstance = {}
    
    -- Instance'dan UUID'ye hızlı erişim
    self.instanceToUuid = {}
    
    -- "Kirli" (Değişmiş ama henüz gönderilmemiş) nesneleri tutar
    self.dirtyStates = {}
    
    return self
end

function RuntimeCache:OnInit(container)
    -- İhtiyaç duyulursa diğer servisler buradan çekilir
end

-- Bir instance'ı önbelleğe kaydeder.
function RuntimeCache:CacheInstance(uuid: string, instance: Instance)
    self.uuidToInstance[uuid] = instance
    self.instanceToUuid[instance] = uuid
end

-- Bir instance'ı önbellekten çıkarır (DESTROY sonrası).
function RuntimeCache:Remove(uuid: string)
    local instance = self.uuidToInstance[uuid]
    if instance then
        self.instanceToUuid[instance] = nil
    end
    self.uuidToInstance[uuid] = nil
    self.dirtyStates[uuid] = nil
end

-- UUID vererek Instance döndürür.
function RuntimeCache:GetInstance(uuid: string): Instance?
    return self.uuidToInstance[uuid]
end

-- Instance vererek UUID döndürür.
function RuntimeCache:GetUuid(instance: Instance): string?
    return self.instanceToUuid[instance]
end

-- Bir instance'ın "kirli" (Dirty) olarak işaretlenmesi (ağa gönderilecekler için)
function RuntimeCache:MarkDirty(uuid: string)
    self.dirtyStates[uuid] = true
end

-- Gönderimden sonra "kirli" bayrağını temizler.
function RuntimeCache:ClearDirty(uuid: string)
    self.dirtyStates[uuid] = nil
end

-- Tüm "kirli" (değişmiş) nesnelerin UUID'lerini döndürür.
function RuntimeCache:GetDirtyUuids()
    local uuids = {}
    for uuid, _ in pairs(self.dirtyStates) do
        table.insert(uuids, uuid)
    end
    return uuids
end

return RuntimeCache
