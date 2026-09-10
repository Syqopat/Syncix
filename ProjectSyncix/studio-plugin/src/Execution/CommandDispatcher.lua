--!strict
-- CommandDispatcher
-- Rust Core'dan gelen (Ağdan okunan) paketleri analiz edip ilgili Executor'a (Yürütücüye) gönderir.
-- Bu yapı ileride "Undo/Redo" (Command Pattern) altyapısının temelini oluşturur.

local ChangeHistoryService = game:GetService("ChangeHistoryService")
local RunService = game:GetService("RunService")
local Ayarlar = require(script.Parent.Parent.Core.Ayarlar)

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
--- Position hatasinda tam olarak bu yasandi: objeler 0,0,0'a dustu ve geri
--- donusu yoktu. Artik her senkron grubu tek bir geri alinabilir adim.
---
--- API cagrisi pcall icinde: ChangeHistoryService bazi Studio durumlarinda
--- (ornegin oyun calisirken) recording acmayi reddediyor; o durumda degisiklik
--- yine de uygulanmali, sadece geri alinamaz olmali.
function CommandDispatcher:GeriAlinabilir(ad: string, is: () -> ())
    -- Geri al kapaliysa kayit acilmaz; is yine de yapilir.
    if not Ayarlar.GeriAlAcik() then
        local ok, hata = pcall(is)
        if not ok then
            warn("[Syncix] Sync step failed: " .. tostring(hata))
        end
        return
    end

    local kayitId = nil
    pcall(function()
        kayitId = ChangeHistoryService:TryBeginRecording(ad, ad)
    end)

    local ok, hata = pcall(is)

    if kayitId then
        pcall(function()
            ChangeHistoryService:FinishRecording(
                kayitId,
                ok and Enum.FinishRecordingOperation.Commit
                    or Enum.FinishRecordingOperation.Cancel
            )
        end)
    end

    if not ok then
        warn("[Syncix] Sync step failed: " .. tostring(hata))
    end
end

--- Oyun calisirken gelen degisiklikleri biriktirir, Play bitince uygular.
---
--- ONEMLI: bu kuyruk gunumuz Studio surumunde HIC DEVREYE GIRMIYOR ve bu
--- gercek bir testle olculdu.
---
--- Play'e basildiginda Studio eklentileri oyunun sunucu ve istemci
--- oturumlarinda yeniden baslatiyor; o kopyalar init.server.lua'daki IsEdit
--- kapisinda duruyor. Geriye core'a bagli tek ornek kaliyor: DUZENLEME
--- oturumundaki. Onun agaci calismadigi icin RunService:IsRunning() onun
--- icin hep false, yani asagidaki dal hicbir zaman secilmiyor.
---
--- Peki neden duruyor: Studio Play sirasinda ayri bir oturum kopyasi
--- kullaniyor, yani editorden gelen degisiklik duzenleme agacina yaziliyor ve
--- Stop'a basildiginda oldugu gibi duruyor. Amaclanan davranis zaten
--- saglaniyor — bu kod onun yedegi. Studio bu izolasyonu degistirirse
--- devreye girer.
---
--- Olculen: Play sirasinda Transparency 0.7 gonderildi. Studio'nun agacinda
--- deger 0.7 olarak gorundu (kuyruga alinmadi), Output'ta "Play mode ended"
--- satiri cikmadi, ve kullanici Play sirasinda parcayi opak, Stop sonrasi
--- saydam gordu.
function CommandDispatcher:_OyunBittiginde()
    if self._playDinleyici then return end
    self._playDinleyici = true

    -- Heartbeat uzerinden kenar tespiti. RunService'in calisma durumu icin
    -- guvenilir bir property sinyali yok; Heartbeat ise duzenleme halinde de
    -- calisiyor, yani Play bittigi anda burasi haberdar oluyor.
    local oncekiCalisiyor = true
    RunService.Heartbeat:Connect(function()
        local calisiyor = RunService:IsRunning()
        local yeniDurdu = oncekiCalisiyor and not calisiyor
        oncekiCalisiyor = calisiyor
        if not yeniDurdu then return end

        local kuyruk = self._playKuyrugu
        if not kuyruk or #kuyruk == 0 then return end
        self._playKuyrugu = {}

        print(string.format(
            "[Syncix] Play mode ended; applying %d change(s) that were held back.",
            #kuyruk
        ))
        for _, bekleyen in ipairs(kuyruk) do
            self:Dispatch(bekleyen)
        end
    end)
end

function CommandDispatcher:Dispatch(payload: any)
    if not payload or not payload.event_type then return end

    -- Oyun calisirken degisiklik uygulanmaz; Play bitince sirayla islenir.
    -- FULL_SYNC_REQUEST istisna: agaci okumak Studio'yu degistirmez ve core'un
    -- dogrulama yolunu Play boyunca kapatmanin bir sebebi yok.
    if RunService:IsRunning() and payload.event_type ~= "FULL_SYNC_REQUEST" then
        local davranis = Ayarlar.PlayDavranisi()
        if davranis == "ignore" then
            return
        elseif davranis ~= "apply" then
            -- Varsayilan: kuyruga al, Play bitince uygula.
            self._playKuyrugu = self._playKuyrugu or {}
            table.insert(self._playKuyrugu, payload)
            self:_OyunBittiginde()
            return
        end
        -- "apply": kullanici bilerek istedi; Play bitince Studio oturumla
        -- birlikte atacagi icin degisiklik kaybolabilir.
    end

    -- Senkron duraklatilmissa gelen degisiklikler UYGULANMAZ.
    -- Yalnizca gondermeyi durdurmak yetmezdi: editorden gelen yamalar Studio'yu
    -- degistirmeye devam ederdi ve "duraklattim" diyen kullanici yine de
    -- yerinin degistigini gorurdu.
    if self.connectionManager and self.connectionManager:IsPaused() then
        return
    end

    -- studio_to_disk modunda Studio yalnizca kaynak; core'dan gelen hicbir
    -- degisiklik uygulanmaz. FULL_SYNC_REQUEST istisna: agaci okumak Studio'yu
    -- degistirmiyor ve o modda zaten tek is akisi bu.
    if not Ayarlar.StudiyaUygula() and payload.event_type ~= "FULL_SYNC_REQUEST" then
        return
    end
    
    self:Lock()
    
    if payload.event_type == "COMPOSITE_UPDATE" then
        self:GeriAlinabilir("Syncix sync", function()
            self:ExecuteTransaction(payload.data.patches)
        end)
    elseif payload.event_type == "PUSH_UPDATE" then
        self:GeriAlinabilir("Syncix sync", function()
            self.patchExecutor:ApplyFullNode(payload.data)
        end)
    elseif payload.event_type == "FULL_SYNC_REQUEST" then
        -- Core agacin tamamini yeniden istiyor. Bu, dogrulamanin tek guvenilir
        -- yoludur: core'un modeli komut gonderilirken zaten guncellendigi icin
        -- modeli okumak komutun Studio'ya ULASTIGINI kanitlamaz.
        if self.patchBuilder and self.connectionManager then
            local anlikGoruntu = self.patchBuilder:BuildFullTreeSnapshot()
            self.connectionManager:Send(anlikGoruntu)
            print("[Syncix] FULL_SYNC request served; the tree was resent.")
        end
    end
    
    self:Unlock()
end

-- Patch uygulama.
--
-- ÖNEMLİ TASARIM KARARI (rollback kaldırıldı):
-- Eskiden tüm patch'ler tek bir pcall içinde uygulanıyor, herhangi biri hata verince
-- ROLLBACK çalışıp uygulanmış değerleri eski haline geri yazıyordu. Bu, yeni oluşturulan
-- objelerde konumun 0,0,0'a dönmesine ve ardından Studio'nun bu eski değeri echo olarak
-- göndererek Core'daki DOĞRU değeri ezmesine yol açıyordu.
--
-- Artık: Core tek doğruluk kaynağıdır. Her patch bağımsız uygulanır; biri başarısız olursa
-- yalnızca o patch atlanır ve uyarı basılır. Rollback yapılmaz — çünkü geri almak,
-- Core ile Studio'yu birbirinden ayırır (Core yeni değeri bilir, Studio eskiye döner).
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
