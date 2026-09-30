-- GamePassService.lua
-- 9 Robuxluk "Meyve / Sebze Çalma (Hırsızlık)" Gamepass Sistemi
-- Hem İstemci hem Sunucu tarafından kullanılabilen ortak servis modülü

local MarketplaceService = game:GetService("MarketplaceService")
local Players = game:GetService("Players")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local RunService = game:GetService("RunService")

local GamePassConfig = require(ReplicatedStorage:WaitForChild("GamePassConfig", 10))

local GamePassService = {}
GamePassService.STEAL_PASS_ID = GamePassConfig.STEAL_GAMEPASS_ID
GamePassService.STEAL_PRICE = GamePassConfig.STEAL_PRICE_ROBUX

local DEVELOPERS = {
    ["erimyancar"] = true,
    ["0ben_ege0"] = true,
    ["rakunkee"] = true,
}

local function isDeveloper(player)
    if not player then return false end
    if RunService:IsStudio() then return true end
    local name = player.Name:lower()
    local dName = player.DisplayName:lower()
    for dev, _ in pairs(DEVELOPERS) do
        if name:find(dev) or dName:find(dev) then
            return true
        end
    end
    return false
end

-- -------------------------------------------------------------
-- 1. Oyuncunun Çalma Yetkisini Kontrol Et (Cache'li & Hızlı)
-- -------------------------------------------------------------
function GamePassService.canSteal(player)
    if not player then return false end

    -- 1. Yetki attribute'u açık mı? (Satın alanlara veya teste tabi olanlara verilir)
    if player:GetAttribute("HasStealPass") == true then
        return true
    end

    -- 2. Geliştirici veya Studio Test Modu
    if GamePassConfig.DEVELOPER_FREE_TEST and isDeveloper(player) then
        return true
    end

    -- 3. Gamepass ID girilmişse (ID > 0) Roblox MarketplaceService kontrolü
    if GamePassService.STEAL_PASS_ID and GamePassService.STEAL_PASS_ID > 0 then
        local success, hasPass = pcall(function()
            return MarketplaceService:UserOwnsGamePassAsync(player.UserId, GamePassService.STEAL_PASS_ID)
        end)
        if success and hasPass then
            player:SetAttribute("HasStealPass", true)
            return true
        end
    end

    return false
end

-- -------------------------------------------------------------
-- 2. Gamepass Satın Alma Penceresini Aç & Bildir
-- -------------------------------------------------------------
function GamePassService.promptStealPass(player)
    if not player then return end

    -- İstemciye bildirim yolla
    local GameEvents = ReplicatedStorage:FindFirstChild("GameEvents")
    local notifRE = GameEvents and GameEvents:FindFirstChild("Notification_RE")
    if notifRE and RunService:IsServer() then
        notifRE:FireClient(player, "🔒 Hırsızlık Gamepass Gerekli", "Başkalarının ürünlerini çalmak için 'Hırsızlık Gamepass' (9 Robux) satın almalısın!", Color3.fromRGB(255, 195, 45))
    end

    -- Roblox Purchase Prompt aç
    if GamePassService.STEAL_PASS_ID and GamePassService.STEAL_PASS_ID > 0 then
        pcall(function()
            MarketplaceService:PromptGamePassPurchase(player, GamePassService.STEAL_PASS_ID)
        end)
    else
        -- Eğer kullanıcı henüz kendi ID'sini girmemişse uyar
        print(string.format("[GamePassService] ℹ️ %s için 9 Robuxluk Çalma Gamepass satın alma isteği tetiklendi. (ID henüz 0 ise test için HasStealPass attribute'u verilebilir)", player.Name))
    end
end

-- -------------------------------------------------------------
-- 3. Çalma Olayında Hırsıza ve Kurbana Bildirim Gönder
-- -------------------------------------------------------------
function GamePassService.notifySteal(thief, victim, cropName, weight)
    local GameEvents = ReplicatedStorage:FindFirstChild("GameEvents")
    local notifRE = GameEvents and GameEvents:FindFirstChild("Notification_RE")
    if not notifRE or not RunService:IsServer() then return end

    local wStr = weight and string.format(" (%.1f kg)", weight) or ""

    if thief and thief:IsA("Player") then
        local victimName = (victim and victim:IsA("Player")) and victim.Name or "Biri"
        notifRE:FireClient(thief, "🥷 Hırsızlık Başarılı!", string.format("%s adlı oyuncunun %s%s ürününü başarıyla çaldın!", victimName, cropName, wStr), Color3.fromRGB(80, 235, 110))
    end

    if victim and victim:IsA("Player") and victim ~= thief then
        local thiefName = (thief and thief:IsA("Player")) and thief.Name or "Biri"
        notifRE:FireClient(victim, "⚠️ Tarlana Hırsız Girdi!", string.format("%s tarlana sızıp %s%s ürününü çaldı!", thiefName, cropName, wStr), Color3.fromRGB(245, 65, 65))
    end
end

return GamePassService
