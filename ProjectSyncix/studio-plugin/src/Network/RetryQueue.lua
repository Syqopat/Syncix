--!strict
-- RetryQueue
-- Ağa gönderilemeyen HTTP paketlerini bellek kuyruğuna alır.
-- Sunucu yeniden bağlandığında sırayla tekrar gönderir. Hatalı paketlerin kaybolmasını önler.

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

-- Gönderilemeyen bir paketi kuyruğa atar.
function RetryQueue:EnqueueFailed(payload: any)
    table.insert(self.queue, payload)
    
    if self.metrics then
        self.metrics.data.retryCount += 1
    end
end

-- Kuyrukta paket olup olmadığını kontrol eder.
function RetryQueue:HasPending(): boolean
    return #self.queue > 0
end

-- Kuyruktaki tüm paketleri döndürür ve kuyruğu temizler.
function RetryQueue:Flush(): {any}
    local oldQueue = self.queue
    self.queue = {}
    return oldQueue
end

return RetryQueue
