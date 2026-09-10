local HttpService = game:GetService("HttpService")
local RunService = game:GetService("RunService")

-- Sürüm artık semver: core ile major.minor eşleşmesi aranıyor (bkz. ConnectionManager).
local SYNCIX_VERSION = "0.1.0"

local function startSyncix()
    print("[Syncix] Starting Studio runtime (version " .. SYNCIX_VERSION .. ")...")

    local CorePath = script.Core
    local NetworkPath = script.Network
    local ObserverPath = script.Observer
    local ExecutionPath = script.Execution

    local ServiceContainer = require(CorePath.ServiceContainer)
    local RuntimeCache = require(CorePath.RuntimeCache)
    local EchoGuard = require(CorePath.EchoGuard)
    local ActivityLog = require(CorePath.ActivityLog)
    local SettingsPanel = require(CorePath.SettingsPanel)
    local Metrics = require(CorePath.Metrics)

    local ConnectionManager = require(NetworkPath.ConnectionManager)
    local BatchQueue = require(NetworkPath.BatchQueue)
    local RetryQueue = require(NetworkPath.RetryQueue)

    local SubscriptionManager = require(ObserverPath.SubscriptionManager)
    local GenericObserver = require(ObserverPath.GenericObserver)
    local SelectionObserver = require(ObserverPath.SelectionObserver)
    local PatchBuilder = require(ObserverPath.PatchBuilder)

    local CommandDispatcher = require(ExecutionPath.CommandDispatcher)
    local PatchExecutor = require(ExecutionPath.PatchExecutor)

    local container = ServiceContainer.new()

    -- `plugin` global'i ModuleScript'lerde güvenilir biçimde bulunmuyor.
    -- Onay penceresi ve ayar saklama için buradan açıkça geçiriliyor.
    -- Tabloya sarılıyor: ServiceContainer kayıt sırasında service.OnInit'e bakıyor,
    -- bir Instance üzerinde olmayan üyeyi aramak hata verir.
    container:Register("Plugin", { ref = plugin })

    container:Register("RuntimeCache", RuntimeCache.new())
    container:Register("EchoGuard", EchoGuard.new())
    container:Register("ActivityLog", ActivityLog.new())
    container:Register("Metrics", Metrics.new())
    container:Register("RetryQueue", RetryQueue.new())
    container:Register("PatchBuilder", PatchBuilder.new())
    container:Register("PatchExecutor", PatchExecutor.new())
    container:Register("CommandDispatcher", CommandDispatcher.new())
    container:Register("ConnectionManager", ConnectionManager.new())
    container:Register("BatchQueue", BatchQueue.new())
    container:Register("SettingsPanel", SettingsPanel.new())
    container:Register("SubscriptionManager", SubscriptionManager.new())
    container:Register("GenericObserver", GenericObserver.new())
    container:Register("SelectionObserver", SelectionObserver.new())

    container:StartAll()

    local batchQueue = container:Get("BatchQueue")
    RunService.Heartbeat:Connect(function()
        batchQueue:Flush()
    end)

    print("[Syncix] All services started and wired up.")
end

-- YALNIZCA duzenleme oturumunda calis.
--
-- Bu Studio surumu Play'e basildiginda eklentileri oyunun SUNUCU ve ISTEMCI
-- oturumlarinda da calistiriyor (yerlesik eklentiler de ayni sekilde
-- davraniyor). O kopyalarin yapacagi bir is yok: senkron edilecek bir duzenleme
-- agaci yok, istemci tarafinda HTTP zaten kapali.
--
-- Kapi EN BASTA olmali. Ilk denememde startSyncix'in ICINE koymustum, ama
-- HTTP denetimi ondan once calisiyor: Play sirasinda istemci oturumu
-- "Allow HTTP Requests" uyarisini basmaya devam ediyordu.
if not RunService:IsEdit() then
    return
end

if not HttpService.HttpEnabled then
    warn("[Syncix] Please enable Game Settings > Security > Allow HTTP Requests.")
    task.spawn(function()
        while not HttpService.HttpEnabled do
            task.wait(1)
        end
        print("[Syncix] HTTP requests enabled. Starting automatically...")
        startSyncix()
    end)
else
    startSyncix()
end
