--!strict
-- Metrics
-- Ağ performansı, kuyruk uzunluğu ve ping verilerini toplar.
-- İleride Profiler veya Live Debugger eklentilerine bağlanması için açık uçludur.

local Metrics = {}
Metrics.__index = Metrics

function Metrics.new()
    local self = setmetatable({}, Metrics)
    
    self.data = {
        ping = 0,
        avgLatency = 0,
        maxLatency = 0,
        minLatency = 99999,
        queueLength = 0,
        retryCount = 0,
        failedRequests = 0,
        successfulRequests = 0,
        patchCount = 0
    }
    
    return self
end

function Metrics:OnInit(container)
    self.container = container
end

function Metrics:RecordLatency(latencyMs: number)
    self.data.ping = latencyMs
    
    if latencyMs > self.data.maxLatency then
        self.data.maxLatency = latencyMs
    end
    if latencyMs < self.data.minLatency then
        self.data.minLatency = latencyMs
    end
    
    -- Basit hareketli ortalama (Simple Moving Average)
    if self.data.avgLatency == 0 then
        self.data.avgLatency = latencyMs
    else
        self.data.avgLatency = (self.data.avgLatency * 0.9) + (latencyMs * 0.1)
    end
end

function Metrics:IncrementFailedRequests()
    self.data.failedRequests += 1
end

function Metrics:IncrementSuccessfulRequests()
    self.data.successfulRequests += 1
end

function Metrics:SetQueueLength(length: number)
    self.data.queueLength = length
end

function Metrics:IncrementPatchCount(amount: number)
    self.data.patchCount += amount
end

-- Metriklerin dökümü (Debug amaçlı)
function Metrics:Dump(): { [string]: number }
    return self.data
end

return Metrics
