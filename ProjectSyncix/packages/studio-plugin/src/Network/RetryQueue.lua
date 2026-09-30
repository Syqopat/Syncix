--!strict
-- RetryQueue
-- Keeps HTTP packets that could not be sent in an in-memory queue.
-- Resends them in order when the server reconnects, so failed packets are not lost.

local RetryQueue = {}
RetryQueue.__index = RetryQueue

function RetryQueue.new()
    local self = setmetatable({}, RetryQueue)
    self.queue = {}
    return self
end

function RetryQueue:OnStart(container)
    self.metrics = container:Get("Metrics")
end

-- Queues a packet that could not be sent.
function RetryQueue:EnqueueFailed(payload: any)
    table.insert(self.queue, payload)
    
    if self.metrics then
        self.metrics.data.retryCount += 1
    end
end

-- Checks whether there are packets in the queue.
function RetryQueue:HasPending(): boolean
    return #self.queue > 0
end

-- Returns every packet in the queue and clears it.
function RetryQueue:Flush(): {any}
    local oldQueue = self.queue
    self.queue = {}
    return oldQueue
end

return RetryQueue
