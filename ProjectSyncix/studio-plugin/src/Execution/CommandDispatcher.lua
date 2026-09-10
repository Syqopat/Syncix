--!strict
-- CommandDispatcher
-- Rust Core'dan incoming (Ağdan okunan) paketleri analiz edip ilgili Executor'a (Yürütücüye) gönderir.
-- Bu yapı ileride "Undo/Redo" (Command Pattern) altyapısının temelini oluşturur.

local ChangeHistoryService = game:GetService("ChangeHistoryService")
local RunService = game:GetService("RunService")
local SyncConfig = require(script.Parent.Parent.Core.SyncConfig)

local CommandDispatcher = {}
CommandDispatcher.__index = CommandDispatcher

function CommandDispatcher.new()
    local self = setmetatable({}, CommandDispatcher)
    
    -- Syncix kaynaklı güncellemeler uygulanırken Observer'ın tepki vermesini engelleyen kilit.
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

--- Bir degisiklik grubunu Studio'nun GERI AL yiginina kaydederek uygular.
---
--- Neden gerekli: Syncix'in yaptigi degisiklikler Studio'nun undo yigininin
--- disindaydi, yani kotu bir senkron geldiginde Ctrl+Z ise yaramiyordu.
--- Position hatasinda tam olarak bu yasandi: objects 0,0,0'a dustu ve geri
--- donusu yoktu. Artik her senkron grubu tek bir geri alinabilir adim.
---
--- API cagrisi pcall icinde: ChangeHistoryService bazi Studio durumlarinda
--- (ornegin oyun calisirken) recording acmayi reddediyor; o durumda degisiklik
--- yine de uygulanmali, sadece geri alinamaz olmali.
function CommandDispatcher:IsUndoable(ad: string, is: () -> ())
    -- Geri al kapaliysa entry acilmaz; is yine de yapilir.
    if not SyncConfig.UndoEnabled() then
        local ok, failure = pcall(is)
        if not ok then
            warn("[Syncix] Sync step failed: " .. tostring(failure))
        end
        return
    end

    local entryId = nil
    pcall(function()
        entryId = ChangeHistoryService:TryBeginRecording(ad, ad)
    end)

    local ok, failure = pcall(is)

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

--- Oyun calisirken incoming degisiklikleri biriktirir, Play bitince uygular.
---
--- ONEMLI: bu backlog gunumuz Studio surumunde HIC DEVREYE GIRMIYOR ve bu
--- gercek bir testle olculdu.
---
--- Play'e basildiginda Studio eklentileri oyunun sunucu ve istemci
--- oturumlarinda yeniden baslatiyor; o kopyalar init.server.lua'daki IsEdit
--- kapisinda duruyor. Geriye core'a bagli tek ornek kaliyor: DUZENLEME
--- oturumundaki. Onun agaci calismadigi icin RunService:IsRunning() onun
--- icin hep false, yani asagidaki dal hicbir timestamp secilmiyor.
---
--- Peki neden duruyor: Studio Play sirasinda ayri bir oturum kopyasi
--- kullaniyor, yani editorden incoming degisiklik duzenleme agacina yaziliyor ve
--- Stop'a basildiginda oldugu gibi duruyor. Amaclanan behavior zaten
--- saglaniyor — bu kod onun yedegi. Studio bu izolasyonu degistirirse
--- devreye girer.
---
--- Olculen: Play sirasinda Transparency 0.7 gonderildi. Studio'nun agacinda
--- datum 0.7 olarak gorundu (kuyruga alinmadi), Output'ta "Play mode ended"
--- satiri cikmadi, ve kullanici Play sirasinda parcayi opak, Stop sonrasi
--- saydam gordu.
function CommandDispatcher:_OnPlayEnded()
    if self._playListener then return end
    self._playListener = true

    -- Heartbeat uzerinden kenar tespiti. RunService'in calisma durumu icin
    -- guvenilir bir property sinyali yok; Heartbeat ise duzenleme halinde de
    -- isRunning, yani Play bittigi anda burasi haberdar oluyor.
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

    -- Oyun calisirken degisiklik uygulanmaz; Play bitince sirayla islenir.
    -- FULL_SYNC_REQUEST istisna: agaci okumak Studio'yu degistirmez ve core'un
    -- dogrulama yolunu Play boyunca kapatmanin bir sebebi yok.
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
        -- "apply": kullanici bilerek istedi; Play bitince Studio oturumla
        -- birlikte atacagi icin degisiklik kaybolabilir.
    end

    -- Senkron duraklatilmissa incoming degisiklikler UYGULANMAZ.
    -- Yalnizca gondermeyi durdurmak yetmezdi: editorden incoming yamalar Studio'yu
    -- degistirmeye devam ederdi ve "duraklattim" diyen kullanici yine de
    -- yerinin degistigini gorurdu.
    if self.connectionManager and self.connectionManager:IsPaused() then
        return
    end

    -- studio_to_disk modunda Studio yalnizca kaynak; core'dan incoming hicbir
    -- degisiklik uygulanmaz. FULL_SYNC_REQUEST istisna: agaci okumak Studio'yu
    -- degistirmiyor ve o modda zaten tek is akisi bu.
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
        -- Core agacin tamamini yeniden istiyor. Bu, dogrulamanin tek guvenilir
        -- yoludur: core'un modeli komut gonderilirken zaten guncellendigi icin
        -- modeli okumak komutun Studio'ya ULASTIGINI kanitlamaz.
        if self.patchBuilder and self.connectionManager then
            local snapshot = self.patchBuilder:BuildFullTreeSnapshot()
            self.connectionManager:Send(snapshot)
            print("[Syncix] FULL_SYNC request served; the tree was resent.")
        end
    end
    
    self:Unlock()
end

-- Patch uygulama.
--
-- ÖNEMLİ TASARIM KARARI (rollback kaldırıldı):
-- Eskiden tüm patch'ler tek bir pcall içinde uygulanıyor, herhangi biri failure verince
-- ROLLBACK çalışıp uygulanmış değerleri eski haline geri yazıyordu. Bu, fresh oluşturulan
-- objelerde konumun 0,0,0'a dönmesine ve ardından Studio'nun bu eski değeri echo olarak
-- göndererek Core'daki DOĞRU değeri ezmesine yol açıyordu.
--
-- Artık: Core tek doğruluk kaynağıdır. Her patch bağımsız uygulanır; biri başarısız olursa
-- yalnızca o patch atlanır ve uyarı basılır. Rollback yapılmaz — çünkü geri almak,
-- Core ile Studio'yu birbirinden ayırır (Core fresh değeri bilir, Studio eskiye döner).
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
