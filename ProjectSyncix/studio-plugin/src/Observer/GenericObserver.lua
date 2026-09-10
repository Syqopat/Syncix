local Workspace = game:GetService("Workspace")
local HttpService = game:GetService("HttpService")
local RunService = game:GetService("RunService")

local Services = require(script.Parent.Services)
local SyncConfig = require(script.Parent.Parent.Core.SyncConfig)
local SERVICE_UUIDS = Services.UUIDS

local Players = game:GetService("Players")

--- Bu object, senkron dışı bırakılması gereken bir OYUNCU karakterinin parçası mı?
---
--- Eskiden içinde Humanoid olan HER Model elenirdi. Amaç doğruydu — player
--- karakterleri Studio'da kendiliğinden belirip kaybolan geçici objects, onları
--- diske yazmak anlamsız — ama kapsam çok genişti: yazarın Workspace'e elle
--- koyduğu NPC'ler, içlerindeki script'ler ve kıyafetleri de eleniyordu. Bir
--- simulator'da NPC'ler oyunun kendisi; editörde hiç görünmüyorlardı.
---
--- Ayrım şu: eleyeceğimiz şey "Humanoid içeriyor" değil, "Players servisine
--- bağlı bir oyuncunun karakteri". Yazarın koyduğu NPC ise sıradan içerik.
local function isPlayerCharacter(inst: Instance): boolean
    local model = inst:FindFirstAncestorOfClass("Model")
        or (inst:IsA("Model") and inst :: Model)
    if not model then
        return false
    end

    -- Oyuncu karakteri: adı bağlı bir oyuncunun adıyla eşleşir ve Workspace'in
    -- doğrudan çocuğudur. Roblox karakterleri tam olarak böyle yerleştiriyor.
    local ok, player = pcall(function()
        return Players:GetPlayerFromCharacter(model)
    end)
    if ok and player then
        return true
    end

    -- Play sırasında oluşan geçici karakterler; oyun çalışmıyorken böyle bir
    -- durum zaten yok.
    if RunService:IsRunning() and model:FindFirstChildOfClass("Humanoid") then
        return true
    end

    return false
end

local GenericObserver = {}
GenericObserver.__index = GenericObserver

function GenericObserver.new()
    local self = setmetatable({}, GenericObserver)
    self.isSyncing = false
    self.tracked = {}
    self.parentOf = {}
    return self
end

function GenericObserver:OnStart(container)
    self.cache = container:Get("RuntimeCache")
    self.subscriptions = container:Get("SubscriptionManager")
    self.patchBuilder = container:Get("PatchBuilder")
    self.batchQueue = container:Get("BatchQueue")
    self.commandDispatcher = container:Get("CommandDispatcher")
    self.echoGuard = container:Get("EchoGuard")
    self.activityLog = container:Get("ActivityLog")

    local servicesToSync = Services.List()

    for _, service in ipairs(servicesToSync) do
        local serviceUuid = SERVICE_UUIDS[service.Name] or service:GetAttribute("__syncix_id")
        if not serviceUuid then
            serviceUuid = HttpService:GenerateGUID(false)
        end
        service:SetAttribute("__syncix_id", serviceUuid)
        self.cache:CacheInstance(serviceUuid, service)

        self.subscriptions:Subscribe(service.Name .. "_ROOT", service.DescendantAdded, function(descendant)
            self:HandleInstanceAdded(descendant, false)
        end)
        
        for _, descendant in ipairs(service:GetDescendants()) do
            self:HandleInstanceAdded(descendant, true)
        end
    end
end

function GenericObserver:HandleInstanceAdded(instance: Instance, isBootstrap: boolean)
    if RunService:IsRunning() then return end
    if instance:IsA("Terrain") or instance:IsA("Camera") then return end
    -- Kullanicinin disladigi siniflar hic izlenmez: yalnizca gonderimi
    -- susturmak yetmez, object yine de onbellege girip UUID alirdi.
    if not SyncConfig.ClassAllowed(instance.ClassName) then return end
    if isPlayerCharacter(instance) then return end

    if self.tracked[instance] then return end
    self.tracked[instance] = true

    local uuid = instance:GetAttribute("__syncix_id")
    
    if not uuid then
        uuid = HttpService:GenerateGUID(false)
        instance:SetAttribute("__syncix_id", uuid)
        
        if not isBootstrap then
            local createPatch = self.patchBuilder:BuildLifecyclePatch(uuid, instance, "CREATE")
            if self.activityLog then
                self.activityLog:Outbound("create", instance.Name, nil, instance.ClassName, uuid)
            end
            self.batchQueue:Enqueue(createPatch)
        end
    end
    
    self.cache:CacheInstance(uuid, instance)

    if instance.Parent then
        self.parentOf[instance] = instance.Parent:GetAttribute("__syncix_id")
    end

    self.subscriptions:Subscribe(uuid, instance.Changed, function(propertyName)
        self:HandlePropertyChanged(instance, uuid, propertyName)
    end)

    self.subscriptions:Subscribe(uuid, instance.AttributeChanged, function(attrName)
        self:HandleAttributeChanged(instance, uuid, attrName)
    end)
    
    self.subscriptions:Subscribe(uuid, instance.Destroying, function()
        self:HandleInstanceDestroyed(instance, uuid)
    end)

    self.subscriptions:Subscribe(uuid, instance.AncestryChanged, function()
        if not instance:IsDescendantOf(game) then
            self:HandleInstanceDestroyed(instance, uuid)
        else
            self:HandleReparent(instance, uuid)
        end
    end)
end

function GenericObserver:HandleReparent(instance: Instance, uuid: string)
    if RunService:IsRunning() then return end
    if not SyncConfig.SendFromStudio() then return end
    if isPlayerCharacter(instance) then return end
    local parent = instance.Parent
    if not parent then return end

    local parentUuid = parent:GetAttribute("__syncix_id")
    if not parentUuid then return end

    if self.parentOf[instance] == parentUuid then return end
    self.parentOf[instance] = parentUuid

    if self.commandDispatcher and self.commandDispatcher:IsLocked() then return end
    if self.echoGuard and self.echoGuard:Consume(uuid, "__parent", parentUuid) then return end

    local patch = self.patchBuilder:BuildReparentPatch(uuid, parentUuid)
    self.batchQueue:Enqueue(patch)
end

function GenericObserver:HandlePropertyChanged(instance: Instance, uuid: string, propertyName: string)
    if RunService:IsRunning() then return end
    if isPlayerCharacter(instance) then return end
    if self.commandDispatcher and self.commandDispatcher:IsLocked() then return end
    
    if not SyncConfig.SendFromStudio() then return end
    if not SyncConfig.PropertyAllowed(propertyName) then return end

    if propertyName == "Name" or self.patchBuilder:IsWatchedProperty(instance, propertyName) then
        local newValue = (instance :: any)[propertyName]
        -- Bu değeri az önce Syncix'in kendisi yazdıysa geri gönderme.
        if self.echoGuard and self.echoGuard:Consume(uuid, propertyName, newValue) then return end
        local patch = self.patchBuilder:BuildPropertyPatch(uuid, propertyName, newValue)

        if patch then
            if self.activityLog then
                self.activityLog:Outbound("property", instance.Name, propertyName, newValue, uuid)
            end
            self.batchQueue:Enqueue(patch)
        end
    elseif propertyName == "Source" and instance:IsA("LuaSourceContainer") then
        local newValue = (instance :: any).Source
        if self.echoGuard and self.echoGuard:Consume(uuid, "Source", newValue) then return end
        local patch = self.patchBuilder:BuildPropertyPatch(uuid, "Source", newValue)
        if patch then
            self.batchQueue:Enqueue(patch)
        end
    end
end

function GenericObserver:HandleAttributeChanged(instance: Instance, uuid: string, attrName: string)
    if RunService:IsRunning() then return end
    if not SyncConfig.SendFromStudio() then return end
    if isPlayerCharacter(instance) then return end
    -- Syncix'in kendi defterleri content degil; degistiklerinde yama uretilmez.
    if attrName == "__syncix_id" or attrName == "__syncix_place" then return end
    if self.commandDispatcher and self.commandDispatcher:IsLocked() then return end

    local value = instance:GetAttribute(attrName)
    if self.echoGuard and self.echoGuard:Consume(uuid, "@" .. attrName, value) then return end
    local patch = self.patchBuilder:BuildAttributePatch(uuid, attrName, value)
    if patch then
        if self.activityLog then
            self.activityLog:Outbound("attribute", instance.Name, attrName, value, uuid)
        end
        self.batchQueue:Enqueue(patch)
    end
end

function GenericObserver:HandleInstanceDestroyed(instance: Instance, uuid: string)
    if RunService:IsRunning() then return end
    if not self.tracked[instance] then return end
    self.tracked[instance] = nil
    self.parentOf[instance] = nil

    if self.commandDispatcher and self.commandDispatcher:IsLocked() then
        self.subscriptions:UnsubscribeAll(uuid)
        return
    end

    local destroyPatch = self.patchBuilder:BuildLifecyclePatch(uuid, instance, "DESTROY")
    if self.activityLog then
        self.activityLog:Outbound("silme", instance.Name, nil, nil, uuid)
    end
    self.batchQueue:Enqueue(destroyPatch)
    self.subscriptions:UnsubscribeAll(uuid)
end

return GenericObserver
