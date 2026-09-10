--!strict
-- CommandDispatcher
-- Analyses packets coming from the Rust core over the network and sends them to the matching executor.
-- This structure is the foundation for a future undo/redo (command pattern) system.

local ChangeHistoryService = game:GetService("ChangeHistoryService")
local RunService = game:GetService("RunService")
local SyncConfig = require(script.Parent.Parent.Core.SyncConfig)

local CommandDispatcher = {}
CommandDispatcher.__index = CommandDispatcher

function CommandDispatcher.new()
    local self = setmetatable({}, CommandDispatcher)
    
    -- Lock that keeps the observer from reacting while Syncix-originated updates are applied.
    self.isLocked = false 
    
    return self
end

function CommandDispatcher:OnStart(container)
    self.patchExecutor = container:Get("PatchExecutor")
    self.patchBuilder = container:Get("PatchBuilder")
    self.connectionManager = container:Get("ConnectionManager")
end

function CommandDispatcher:IsLocked(): boolean
    return self.isLocked
end

function CommandDispatcher:Lock()
    self.isLocked = true
end

function CommandDispatcher:Unlock()
    self.isLocked = false
end

--- Applies a group of changes by recording it on Studio's UNDO stack.
---
--- Why it is needed: Syncix's changes were outside Studio's undo stack,
--- so Ctrl+Z did nothing when a bad sync arrived.
--- That is exactly what happened with the Position bug: objects dropped to 0,0,0 with no
--- way back. Now every sync group is one undoable step.
---
--- The API call runs in pcall: ChangeHistoryService refuses to start recording in some
--- Studio states (e.g. while the game is running); the change must still be applied
--- then, just not undoably.
function CommandDispatcher:IsUndoable(actionName: string, applyChanges: () -> ())
    -- With undo off no recording is opened; the work is still done.
    if not SyncConfig.UndoEnabled() then
        local ok, failure = pcall(applyChanges)
        if not ok then
            warn("[Syncix] Sync step failed: " .. tostring(failure))
        end
        return
    end

    local entryId = nil
    pcall(function()
        entryId = ChangeHistoryService:TryBeginRecording(actionName, actionName)
    end)

    local ok, failure = pcall(applyChanges)

    if entryId then
        pcall(function()
            ChangeHistoryService:FinishRecording(
                entryId,
                ok and Enum.FinishRecordingOperation.Commit
                    or Enum.FinishRecordingOperation.Cancel
            )
        end)
    end

    if not ok then
        warn("[Syncix] Sync step failed: " .. tostring(failure))
    end
end

--- Holds changes that arrive while the game runs and applies them when Play ends.
---
--- IMPORTANT: this backlog NEVER KICKS IN on current Studio versions, and that was
--- measured with a real test.
---
--- When Play is pressed, Studio restarts plugins in the game's server and client
--- sessions; those copies stop at the IsEdit gate in init.server.lua.
--- The only instance still connected to the core is the one in the EDIT
--- session. Its tree is not running, so RunService:IsRunning() is always false for it,
--- and the branch below is never taken.
---
--- So why keep it: during Play Studio uses a separate copy of the session,
--- so a change from the editor is written to the edit tree and stays as it is
--- when Stop is pressed. The intended behaviour is already
--- there — this code is its fallback. If Studio ever changes that isolation,
--- it kicks in.
---
--- Measured: Transparency 0.7 was sent during Play. In Studio's tree the
--- value showed 0.7 (it was not queued), no "Play mode ended" line appeared
--- in Output, and the user saw the part opaque during Play and transparent
--- after Stop.
function CommandDispatcher:_OnPlayEnded()
    if self._playListener then return end
    self._playListener = true

    -- Edge detection via Heartbeat. There is no reliable property signal for RunService's
    -- running state; Heartbeat, however, runs in edit mode too,
    -- so this learns the moment Play ends.
    local wasRunning = true
    RunService.Heartbeat:Connect(function()
        local isRunning = RunService:IsRunning()
        local nowStopped = wasRunning and not isRunning
        wasRunning = isRunning
        if not nowStopped then return end

        local backlog = self._playQueue
        if not backlog or #backlog == 0 then return end
        self._playQueue = {}

        print(string.format(
            "[Syncix] Play mode ended; applying %d change(s) that were held back.",
            #backlog
        ))
        for _, pendingItem in ipairs(backlog) do
            self:Dispatch(pendingItem)
        end
    end)
end

function CommandDispatcher:Dispatch(payload: any)
    if not payload or not payload.event_type then return end

    -- No changes are applied while the game runs; they are processed in order when Play ends.
    -- FULL_SYNC_REQUEST is the exception: reading the tree does not change Studio, and there is
    -- no reason to shut down the core's verification path for the whole of Play.
    if RunService:IsRunning() and payload.event_type ~= "FULL_SYNC_REQUEST" then
        local behavior = SyncConfig.PlayBehavior()
        if behavior == "ignore" then
            return
        elseif behavior ~= "apply" then
            -- Varsayilan: kuyruga al, Play bitince applyFn.
            self._playQueue = self._playQueue or {}
            table.insert(self._playQueue, payload)
            self:_OnPlayEnded()
            return
        end
        -- "apply": the user asked for it knowingly; since Studio discards the session
        -- when Play ends, the change may be lost.
    end

    -- When sync is paused, incoming changes are NOT APPLIED.
    -- Stopping only the sending would not be enough: patches from the editor would keep
    -- changing Studio, and a user who had paused would still see
    -- their place change.
    if self.connectionManager and self.connectionManager:IsPaused() then
        return
    end

    -- In studio_to_disk mode Studio is only the source; no change from the core
    -- is applied. FULL_SYNC_REQUEST is the exception: reading the tree does not
    -- change Studio, and it is the only workflow in that mode anyway.
    if not SyncConfig.ApplyToStudio() and payload.event_type ~= "FULL_SYNC_REQUEST" then
        return
    end
    
    self:Lock()
    
    if payload.event_type == "COMPOSITE_UPDATE" then
        self:IsUndoable("Syncix sync", function()
            self:ExecuteTransaction(payload.data.patches)
        end)
    elseif payload.event_type == "PUSH_UPDATE" then
        self:IsUndoable("Syncix sync", function()
            self.patchExecutor:ApplyFullNode(payload.data)
        end)
    elseif payload.event_type == "FULL_SYNC_REQUEST" then
        -- The core asks for the whole tree again. This is the only reliable way to
        -- verify: the core's model is already updated while the command is sent, so
        -- reading the model does not prove that the command REACHED Studio.
        if self.patchBuilder and self.connectionManager then
            local snapshot = self.patchBuilder:BuildFullTreeSnapshot()
            self.connectionManager:Send(snapshot)
            print("[Syncix] FULL_SYNC request served; the tree was resent.")
        end
    end
    
    self:Unlock()
end

-- Applying patches.
--
-- IMPORTANT DESIGN DECISION (rollback removed):
-- All patches used to be applied inside one pcall; when any failed,
-- a ROLLBACK wrote the applied values back to their old state. For newly created
-- objects that put the position back to 0,0,0, and Studio then sent that old value as an
-- echo, overwriting the CORRECT value in the core.
--
-- Now: the core is the single source of truth. Each patch is applied independently; if one fails
-- only that patch is skipped and a warning is printed. There is no rollback — because undoing
-- separates the core from Studio (the core knows the new value, Studio goes back to the old one).
function CommandDispatcher:ExecuteTransaction(patches: any)
    if type(patches) ~= "table" then return end

    for _, patch in ipairs(patches) do
        local ok, err = pcall(function()
            self.patchExecutor:ApplyPatch(patch)
        end)

        if not ok then
            local pType = (patch and patch.event_type) or "?"
            local pProp = (patch and patch.data and patch.data.property) or ""
            warn(string.format(
                "[Syncix] Could not apply patch (%s %s): %s",
                tostring(pType),
                tostring(pProp),
                tostring(err)
            ))
        end
    end
end

return CommandDispatcher
