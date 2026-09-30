--!strict
-- BatchQueue
-- Instead of sending events immediately, collects them and sends one packet (composite patch) at the end of each Heartbeat.
-- This greatly reduces network traffic.

local BatchQueue = {}
BatchQueue.__index = BatchQueue

-- Rate limits. Something that changes a property every frame (a script recolouring its
-- debug drawing, a spinning part) made one request per Heartbeat, ~60 a second, and
-- Studio's HTTP limit stopped the sync. A single edit still leaves on the next frame.
-- At most one packet per MIN_FLUSH_INTERVAL seconds:
local MIN_FLUSH_INTERVAL = 0.1
-- A property sent less than HOT_INTERVAL seconds ago waits (only its last value is kept),
-- so a property changing every frame goes out twice a second instead of 60 times:
local HOT_INTERVAL = 0.5

function BatchQueue.new()
    local self = setmetatable({}, BatchQueue)
    self.queue = {}
    self.lastFlush = 0
    -- "uuid|property" -> os.clock() of its last send; pruned now and then.
    self.lastSent = {}
    self.lastPrune = 0
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

-- Forwards the queue to ConnectionManager as one "CompositePatch". Patches of a property
-- sent less than HOT_INTERVAL ago stay in the queue for a later flush.
function BatchQueue:Flush()
    if #self.queue == 0 then return end
    local now = os.clock()
    if now - self.lastFlush < MIN_FLUSH_INTERVAL then return end

    local sending, held, heldIndex = {}, {}, {}
    for _, patch in ipairs(self.queue) do
        local key = coalesceKey(patch)
        local sentAt = key and self.lastSent[key]
        if sentAt and now - sentAt < HOT_INTERVAL then
            table.insert(held, patch)
            heldIndex[key :: string] = #held
        else
            table.insert(sending, patch)
            if key then
                self.lastSent[key] = now
            end
        end
    end
    self.queue = held
    self.propIndex = heldIndex
    if self.metrics then
        self.metrics:SetQueueLength(#held)
    end

    if now - self.lastPrune > 5 then
        self.lastPrune = now
        for key, sentAt in pairs(self.lastSent) do
            if now - sentAt >= HOT_INTERVAL then
                self.lastSent[key] = nil
            end
        end
    end

    if #sending == 0 then return end
    self.lastFlush = now
    if self.connectionManager then
        self.connectionManager:Send({
            event_type = "COMPOSITE_UPDATE",
            version = "v1",
            data = {
                patches = sending
            }
        })
    end
end

return BatchQueue
