-- GamePassManager.server.lua
-- 9 Robuxluk "Meyve / Sebze Çalma" Gamepass Satın Alım Dinleyicisi ve Yetkilendirici

local MarketplaceService = game:GetService("MarketplaceService")
local Players = game:GetService("Players")
local ReplicatedStorage = game:GetService("ReplicatedStorage")

local GamePassService = require(ReplicatedStorage:WaitForChild("GamePassService", 10))

-- -------------------------------------------------------------
-- 1. Satın Alım Tamamlandığında Anında Yetki Ver
-- -------------------------------------------------------------
MarketplaceService.PromptGamePassPurchaseFinished:Connect(function(player, purchasedPassId, purchaseSuccess)
    if not player then return end

    if purchaseSuccess and (purchasedPassId == GamePassService.STEAL_PASS_ID or GamePassService.STEAL_PASS_ID == 0) then
        player:SetAttribute("HasStealPass", true)
        print(string.format("[GamePassManager] 🎉 %s 9 Robuxluk Hırsızlık Gamepass'ini satın aldı! Çalma yetkisi verildi.", player.Name))

        local GameEvents = ReplicatedStorage:FindFirstChild("GameEvents")
        local notifRE = GameEvents and GameEvents:FindFirstChild("Notification_RE")
        if notifRE then
            notifRE:FireClient(player, "🎉 Hırsızlık Gamepass Aktif!", "Tebrikler! 9 Robuxluk Hırsızlık Gamepass'iniz aktif edildi. Artık tüm bahçelerden ürün çalabilirsiniz!", Color3.fromRGB(80, 245, 120))
        end
    end
end)

-- -------------------------------------------------------------
-- 2. Oyuncu Girişinde Sahipliği Tara ve Attribute Olarak İşle
-- -------------------------------------------------------------
local function onPlayerAdded(player)
    task.spawn(function()
        local can = GamePassService.canSteal(player)
        if can then
            player:SetAttribute("HasStealPass", true)
            print(string.format("[GamePassManager] ✅ %s için Hırsızlık yetkisi onaylandı.", player.Name))
        end
    end)
end

Players.PlayerAdded:Connect(onPlayerAdded)
for _, p in ipairs(Players:GetPlayers()) do
    onPlayerAdded(p)
end

print("[GamePassManager] ✅ 9 Robuxluk Hırsızlık (Çalma) Gamepass servisi aktif!")
