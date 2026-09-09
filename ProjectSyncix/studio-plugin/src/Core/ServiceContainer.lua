--!strict
-- Syncix Service Container
-- Dependency Injection container for Clean Architecture.
-- Hiçbir modül birbirini require() ile doğrudan çağırmaz, her şey buradan enjekte edilir.

local ServiceContainer = {}
ServiceContainer.__index = ServiceContainer

function ServiceContainer.new()
    local self = setmetatable({}, ServiceContainer)
    self.services = {}
    return self
end

-- Bir servisi konteynıra kaydeder.
-- name: Servisin string adı (Örn: "ConnectionManager")
-- service: Servis tablosu/nesnesi
function ServiceContainer:Register(name: string, service: any)
    if self.services[name] then
        warn("[Syncix] Service already registered: " .. name)
        return
    end
    self.services[name] = service
    
    -- Eğer servisin "OnInit" fonksiyonu varsa onu çağırıp Container'ı pasla
    if type(service.OnInit) == "function" then
        service:OnInit(self)
    end
end

-- Kayıtlı bir servisi döndürür.
function ServiceContainer:Get(name: string): any
    local service = self.services[name]
    if not service then
        error("[Syncix] Service not found: " .. name)
    end
    return service
end

-- Tüm servislerin "OnStart" fonksiyonunu çağırır. 
-- Bu, tüm bağımlılıklar çözüldükten sonra sistemi başlatmak içindir.
function ServiceContainer:StartAll()
    for name, service in pairs(self.services) do
        if type(service.OnStart) == "function" then
            service:OnStart(self)
        end
    end
end

return ServiceContainer
