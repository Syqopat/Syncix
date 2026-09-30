--!strict
-- Syncix Service Container
-- Dependency Injection container for Clean Architecture.
-- No module calls another directly with require(); everything is injected from here.

local ServiceContainer = {}
ServiceContainer.__index = ServiceContainer

function ServiceContainer.new()
    local self = setmetatable({}, ServiceContainer)
    self.services = {}
    return self
end

-- Registers a service in the container.
-- name: the service's string name (e.g. "ConnectionManager")
-- service: the service table/object
function ServiceContainer:Register(name: string, service: any)
    if self.services[name] then
        warn("[Syncix] Service already registered: " .. name)
        return
    end
    self.services[name] = service
    
    -- If the service has an "OnInit" function, call it and pass the container
    if type(service.OnInit) == "function" then
        service:OnInit(self)
    end
end

-- Returns a registered service.
function ServiceContainer:Get(name: string): any
    local service = self.services[name]
    if not service then
        error("[Syncix] Service not found: " .. name)
    end
    return service
end

-- Calls every service's "OnStart" function.
-- This starts the system after all dependencies are resolved.
function ServiceContainer:StartAll()
    for name, service in pairs(self.services) do
        if type(service.OnStart) == "function" then
            service:OnStart(self)
        end
    end
end

return ServiceContainer
