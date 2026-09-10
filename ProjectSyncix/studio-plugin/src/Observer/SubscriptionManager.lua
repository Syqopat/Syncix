--!strict
-- SubscriptionManager
-- Manages RBXScriptSignal connections (events) in bulk and prevents memory leaks.

local SubscriptionManager = {}
SubscriptionManager.__index = SubscriptionManager

function SubscriptionManager.new()
    local self = setmetatable({}, SubscriptionManager)
    
    -- Keeps every connection per UUID
    self.connections = {}
    
    return self
end

function SubscriptionManager:OnInit(container)
    -- Other services are fetched when needed
end

-- Creates a new event subscription for an instance.
function SubscriptionManager:Subscribe(uuid: string, signal: RBXScriptSignal, callback: (...any) -> ())
    local connection = signal:Connect(callback)
    
    if not self.connections[uuid] then
        self.connections[uuid] = {}
    end
    
    table.insert(self.connections[uuid], connection)
end

-- Clears every subscription of an instance (so nothing leaks when it is destroyed).
function SubscriptionManager:UnsubscribeAll(uuid: string)
    local instanceConnections = self.connections[uuid]
    if instanceConnections then
        for _, connection in ipairs(instanceConnections) do
            connection:Disconnect()
        end
        self.connections[uuid] = nil
    end
end

-- Removes every subscription in the system (when the plugin shuts down).
function SubscriptionManager:Shutdown()
    for uuid, _ in pairs(self.connections) do
        self:UnsubscribeAll(uuid)
    end
end

return SubscriptionManager
