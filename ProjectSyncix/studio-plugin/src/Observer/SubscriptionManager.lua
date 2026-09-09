--!strict
-- SubscriptionManager
-- RBXScriptSignal nesnelerini (Eventleri) toplu halde yönetir, Memory Leak oluşmasını engeller.

local SubscriptionManager = {}
SubscriptionManager.__index = SubscriptionManager

function SubscriptionManager.new()
    local self = setmetatable({}, SubscriptionManager)
    
    -- UUID bazında tüm bağlantıları (Connection) tutar
    self.connections = {}
    
    return self
end

function SubscriptionManager:OnInit(container)
    -- İhtiyaç duyulursa diğer servisler çekilir
end

-- Bir instance için yeni bir event aboneliği oluşturur.
function SubscriptionManager:Subscribe(uuid: string, signal: RBXScriptSignal, callback: (...any) -> ())
    local connection = signal:Connect(callback)
    
    if not self.connections[uuid] then
        self.connections[uuid] = {}
    end
    
    table.insert(self.connections[uuid], connection)
end

-- Bir instance'ın tüm aboneliklerini temizler (Yok edildiğinde memory leak olmaması için).
function SubscriptionManager:UnsubscribeAll(uuid: string)
    local instanceConnections = self.connections[uuid]
    if instanceConnections then
        for _, connection in ipairs(instanceConnections) do
            connection:Disconnect()
        end
        self.connections[uuid] = nil
    end
end

-- Tüm sistemdeki abonelikleri siler (Plugin kapanırken).
function SubscriptionManager:Shutdown()
    for uuid, _ in pairs(self.connections) do
        self:UnsubscribeAll(uuid)
    end
end

return SubscriptionManager
