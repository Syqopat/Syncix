local HttpService = game:GetService("HttpService")
local RunService = game:GetService("RunService")

-- The version is semver now: major.minor must match the core (see ConnectionManager).
local SYNCIX_VERSION = "0.1.3"

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

    -- The `plugin` global is not reliably available in ModuleScripts.
    -- It is passed explicitly from here for the approval dialog and settings storage.
    -- It is wrapped in a table: ServiceContainer checks service.OnInit when registering,
    -- and looking up a member that does not exist on an Instance throws an error.
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

-- Run ONLY in the edit session.
--
-- When Play is pressed, this Studio version also runs plugins in the game's SERVER and CLIENT
-- sessions (built-in plugins behave the same
-- way). Those copies have nothing to do: there is no edit tree to sync,
-- and HTTP is off on the client side anyway.
--
-- The gate must be at the VERY TOP. The first attempt put it INSIDE startSyncix, but
-- the HTTP check ran before it: during Play the client session
-- kept printing the "Allow HTTP Requests" warning.
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
