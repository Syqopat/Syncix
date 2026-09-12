--!strict
-- CommandDispatcher
-- Analyses packets coming from the Rust core over the network and sends them to the matching executor.
-- This structure is the foundation for a future undo/redo (command pattern) system.

local ChangeHistoryService = game:GetService("ChangeHistoryService")
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

-- Play mode: when a playtest starts, Studio runs it in a separate copy of the place and
-- restarts plugins there; those copies stop at the IsEdit gate in init.server.lua. The
-- copy connected to the core stays in the edit session, so changes from the editor land
-- in the edit session and survive Stop; the running test sees them after a restart.
-- A play-mode queue used to sit here. Measured, it never took effect (the edit session
-- never reports RunService:IsRunning()), and it was removed together with play_mode.

--- Handles one message from the core and remembers how far it got.
---
--- appliedSeq is the number of the last message handled here: applied, dropped because
--- sync is paused, or parked until its parent exists (those are listed separately, see
--- PatchExecutor:WaitingIds). It goes out with every FULL_SYNC, so the core can tell a
--- create Studio has not received yet (keep it) from one Studio refused or deleted (let
--- it go). Before, a FULL_SYNC in the middle of a large import dropped everything still
--- on its way, which then came back later as duplicates.
function CommandDispatcher:Dispatch(payload: any)
    if type(payload) ~= "table" or not payload.event_type then return end
    local handled = true
    local ok, failure = pcall(function()
        handled = self:_Apply(payload) ~= false
    end)
    if not ok then
        -- A poll reply carries up to 64 messages. Uncaught, one error ended the polling
        -- loop for good, dropped the rest of the reply and left the lock on, so the
        -- observer went quiet too. The message still counts as handled: Studio's tree
        -- shows whatever part of it landed.
        self:Unlock()
        warn(string.format(
            "[Syncix] Could not handle %s from the core: %s",
            tostring(payload.event_type),
            tostring(failure)
        ))
    end

    local data = payload.data
    if type(data) == "table" and type(data._seq) == "number" then
        if data._epoch ~= self.appliedEpoch then
            -- Another core process: its numbering starts over.
            self.appliedEpoch = data._epoch
            self.appliedSeq = 0
            self.seqFrozen = false
        end
        if not handled then
            -- Dropped because sync is paused. The applied count is a high-water mark and
            -- cannot leave gaps, so it stops here until the next FULL_SYNC goes out; the
            -- core then keeps what was dropped and sends it again. Counting it as applied
            -- sent the editor's work made during a pause to the trash on resume.
            self.seqFrozen = true
        elseif not self.seqFrozen then
            self.appliedSeq = math.max(self.appliedSeq or 0, data._seq)
        end
    end
end

function CommandDispatcher:_Apply(payload: any)

    -- When sync is paused, incoming changes are NOT APPLIED.
    -- Stopping only the sending would not be enough: patches from the editor would keep
    -- changing Studio, and a user who had paused would still see
    -- their place change.
    if self.connectionManager and self.connectionManager:IsPaused() then
        return false
    end

    -- In studio_to_disk mode Studio is only the source; no change from the core
    -- is applied. FULL_SYNC_REQUEST is the exception: reading the tree does not
    -- change Studio, and it is the only workflow in that mode anyway.
    if not SyncConfig.ApplyToStudio() and payload.event_type ~= "FULL_SYNC_REQUEST" then
        -- Deliberately never applied in this mode: counted as handled, or the core would
        -- keep sending it again.
        return true
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
    return true
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
