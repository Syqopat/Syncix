--!strict
-- BatchQueue
-- Instead of sending events immediately, collects them and sends one packet (composite patch) at the end of each Heartbeat.
-- This greatly reduces network traffic.

local BatchQueue = {}
BatchQueue.__index = BatchQueue

function BatchQueue.new()
    local self = setmetatable({}, BatchQueue)
    self.queue = {}
    -- Coalescing index: "uuid|property" -> position in the queue.
    -- While dragging or using a colour picker the same property changes dozens of times a second;
    -- only the LAST value is kept in the queue, and intermediate values never reach the network.
    self.propIndex = {}
    -- Measurement counters: the only way to verify that coalescing really works.
    -- queued    = total changes that entered the queue
    -- coalesced = changes that overwrote an existing entry and never reached the network
    self.stats = { queued = 0, coalesced = 0 }
    return self
end

-- Counters reported to the core (plugin_queued / plugin_coalesced in /health).
function BatchQueue:GetStats()
    return { queued = self.stats.queued, coalesced = self.stats.coalesced }
end

-- Can a patch be coalesced? (intermediate values of the same object + property can be dropped)
local function coalesceKey(patch: any): string?
    local d = patch and patch.data
    if not d or not d.syncix_id then return nil end
    if patch.event_type == "PROPERTY_UPDATE" and d.property then
        -- Source (script code) can be coalesced too: the last version is enough
        return tostring(d.syncix_id) .. "|p|" .. tostring(d.property)
    elseif patch.event_type == "ATTRIBUTE_UPDATE" and d.name then
        return tostring(d.syncix_id) .. "|a|" .. tostring(d.name)
    elseif patch.event_type == "REPARENT" then
        return tostring(d.syncix_id) .. "|reparent"
    end
    return nil
end

function BatchQueue:OnStart(container)
    self.connectionManager = container:Get("ConnectionManager")
    self.metrics = container:Get("Metrics")
end

-- Adds a new patch. If a patch for the same object+property is already pending, it is
-- REPLACED with the new one (intermediate values never reach the network).
function BatchQueue:Enqueue(patch: any)
    self.stats.queued += 1

    local key = coalesceKey(patch)
    if key then
        local existingIndex = self.propIndex[key]
        if existingIndex and self.queue[existingIndex] then
            self.queue[existingIndex] = patch -- keep only the last value
            self.stats.coalesced += 1
            if self.metrics then
                self.metrics:IncrementPatchCount(1)
            end
            return
        end
        self.propIndex[key] = #self.queue + 1
    end

    table.insert(self.queue, patch)

    if self.metrics then
        self.metrics:SetQueueLength(#self.queue)
        self.metrics:IncrementPatchCount(1)
    end
end

-- Forwards every patch in the queue to ConnectionManager as one "CompositePatch" and clears the queue.
function BatchQueue:Flush()
    if #self.queue == 0 then return end
    
    local compositePatch = {
        event_type = "COMPOSITE_UPDATE",
        version = "v1",
        data = {
            patches = self.queue
        }
    }
    
    if self.connectionManager then
        self.connectionManager:Send(compositePatch)
    end
    
    -- Empty the queue (the coalescing index must be reset too)
    self.queue = {}
    self.propIndex = {}
    if self.metrics then
        self.metrics:SetQueueLength(0)
    end
end

return BatchQueue
