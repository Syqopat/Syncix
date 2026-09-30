-- AdminServerManager (ServerScriptService)
-- Yetkili geliştiriciler (ErimYancar, 0Ben_ege0, Rakunkee ve Studio testçisi) için admin yetkilerini yönetir.

local Players = game:GetService("Players")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local RunService = game:GetService("RunService")
local Workspace = game:GetService("Workspace")

local fruitVisualHelperModule = ReplicatedStorage:WaitForChild("FruitVisualHelper", 15)
local FruitVisualHelper = require(fruitVisualHelperModule)

local DEVELOPERS = {
    ["erimyancar"] = true,
    ["0ben_ege0"] = true,
    ["rakunkee"] = true,
}

local function isDeveloper(player)
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

-- RemoteEvents
local AdminActionEvent = ReplicatedStorage:FindFirstChild("AdminActionEvent")
if not AdminActionEvent then
    AdminActionEvent = Instance.new("RemoteEvent")
    AdminActionEvent.Name = "AdminActionEvent"
    AdminActionEvent.Parent = ReplicatedStorage
end

local DataUpdateEvent = ReplicatedStorage:FindFirstChild("DataUpdateEvent")
if not DataUpdateEvent then
    DataUpdateEvent = Instance.new("RemoteEvent")
    DataUpdateEvent.Name = "DataUpdateEvent"
    DataUpdateEvent.Parent = ReplicatedStorage
end

local function getPlayerCurrencyValue(player)
    local leaderstats = player:FindFirstChild("leaderstats") or player:FindFirstChild("PlayerData")
    if leaderstats then
        return leaderstats:FindFirstChild("Sheckles") or leaderstats:FindFirstChild("Money")
    end
    return nil
end

AdminActionEvent.OnServerEvent:Connect(function(player, action, arg1, arg2)
    if not isDeveloper(player) then
        warn(string.format("[AdminServerManager] Yetkisiz erişim denemesi: %s", player.Name))
        return
    end

    local currencyVal = getPlayerCurrencyValue(player)

    if action == "SetMoney" then
        local amount = tonumber(arg1) or 0
        if currencyVal then
            currencyVal.Value = amount
            DataUpdateEvent:FireClient(player, "Sheckles", amount)
            print(string.format("[Admin] %s parasını %.2e olarak ayarladı.", player.Name, amount))
        end

    elseif action == "AddMoney" then
        local amount = tonumber(arg1) or 0
        if currencyVal then
            currencyVal.Value = currencyVal.Value + amount
            DataUpdateEvent:FireClient(player, "Sheckles", currencyVal.Value)
            print(string.format("[Admin] %s hesabına %.2e ekledi.", player.Name, amount))
        end

    elseif action == "SubtractMoney" then
        local amount = tonumber(arg1) or 0
        if currencyVal then
            currencyVal.Value = math.max(0, currencyVal.Value - amount)
            DataUpdateEvent:FireClient(player, "Sheckles", currencyVal.Value)
            print(string.format("[Admin] %s hesabından %.2e eksiltti.", player.Name, amount))
        end

    elseif action == "SetSpeed" then
        local speed = tonumber(arg1) or 16
        local char = player.Character
        local hum = char and char:FindFirstChildOfClass("Humanoid")
        if hum then
            hum.WalkSpeed = speed
            print(string.format("[Admin] %s yürüme hızını %d yaptı.", player.Name, speed))
        end

    elseif action == "SetFlySpeed" then
        local speed = tonumber(arg1) or 60
        player:SetAttribute("AdminFlySpeed", speed)
        print(string.format("[Admin] %s uçma hızını %d yaptı.", player.Name, speed))

    elseif action == "SetGrowthMultiplier" then
        local mult = tonumber(arg1) or 1
        Workspace:SetAttribute("DevGrowthMultiplier", mult)
        print(string.format("[Admin] %s bitki büyüme hızını %.1fx yaptı.", player.Name, mult))

    elseif action == "InstaGrowAll" then
        Workspace:SetAttribute("DevInstaGrowTrigger", os.clock())
        print(string.format("[Admin] %s tüm bitkileri anında olgunlaştırdı!", player.Name))

    elseif action == "ToggleStealPass" then
        local cur = player:GetAttribute("HasStealPass") == true
        player:SetAttribute("HasStealPass", not cur)
        local GameEvents = ReplicatedStorage:FindFirstChild("GameEvents")
        local notifRE = GameEvents and GameEvents:FindFirstChild("Notification_RE")
        if notifRE then
            local stateStr = not cur and "AÇIK (Artık çalabilirsin!)" or "KAPALI"
            notifRE:FireClient(player, "🥷 Hırsızlık Pass Testi", "Hırsızlık yetkisi: " .. stateStr, not cur and Color3.fromRGB(80, 235, 110) or Color3.fromRGB(240, 70, 70))
        end
        print(string.format("[Admin] %s Hırsızlık Pass test durumunu %s yaptı.", player.Name, tostring(not cur)))

    elseif action == "Heal" then
        local char = player.Character
        local hum = char and char:FindFirstChildOfClass("Humanoid")
        if hum then
            hum.Health = hum.MaxHealth
        end

    elseif action == "Teleport" then
        local dest = tostring(arg1)
        local char = player.Character
        local root = char and char:FindFirstChild("HumanoidRootPart")
        if not root then return end

        local targetCF = nil
        if dest == "Shop" then
            targetCF = CFrame.lookAt(Vector3.new(0, 128.5, 0), Vector3.new(12, 128.5, 0))
        elseif dest:find("Island") then
            local farmFolder = Workspace:FindFirstChild("Farm")
            local farm = farmFolder and farmFolder:FindFirstChild("Farm_" .. dest)
            local sp = farm and farm:FindFirstChild("Spawn_Point", true)
            if sp and sp:IsA("BasePart") then
                targetCF = CFrame.new(sp.Position + Vector3.new(0, 3.5, 0))
            else
                local islands = Workspace:FindFirstChild("Islands")
                local isl = (islands and islands:FindFirstChild(dest)) or Workspace:FindFirstChild(dest)
                if isl then
                    targetCF = CFrame.new(isl:GetPivot().Position + Vector3.new(0, 5, 0))
                end
            end
        end

        if targetCF then
            char:PivotTo(targetCF)
            TeleportEvent:FireClient(player, targetCF)
        end

    elseif action == "GiveAllSeeds" then
        local seedsFolder = ReplicatedStorage:FindFirstChild("Seeds")
        local backpack = player:FindFirstChildOfClass("Backpack")
        local starterGear = player:FindFirstChildOfClass("StarterGear")
        local ALL_15_SEEDS = {
            "Carrot Seed", "Potato Seed", "Strawberry Seed", "Blueberry Seed",
            "Apple Tree Seed", "Mango Tree Seed", "Frostleaf Seed", "Cherry Tree Seed",
            "Banana Tree Seed", "Dragonfruit Bush Seed", "Starfruit Plant Seed",
            "Coconut Palm Seed", "Crystal Shrub Seed", "Nebula Vine Seed", "Celestial Tree Seed"
        }
        if backpack then
            local count = 0
            for _, sName in ipairs(ALL_15_SEEDS) do
                if not backpack:FindFirstChild(sName) then
                    local tool = seedsFolder and (seedsFolder:FindFirstChild(sName) or seedsFolder:FindFirstChild(sName:gsub(" Tree", ""):gsub(" Bush", ""):gsub(" Plant", ""):gsub(" Palm", "")))
                    if tool then
                        tool = tool:Clone()
                    else
                        tool = Instance.new("Tool")
                        tool.Name = sName
                        tool:SetAttribute("Seed", sName:gsub(" Seed", ""))
                        local h = Instance.new("Part")
                        h.Name = "Handle"
                        h.Size = Vector3.new(0.8, 0.8, 0.8)
                        h.CanCollide = false
                        h.Anchored = false
                        h.Parent = tool
                    end
                    tool:SetAttribute("Quantity", 99)
                    tool.Parent = backpack
                    if starterGear and not starterGear:FindFirstChild(sName) then
                        tool:Clone().Parent = starterGear
                    end
                    count = count + 1
                end
            end
            print(string.format("[Admin] %s oyuncusuna 15 bitki tohumu verildi.", player.Name))
        end

    elseif action == "GiveSeed" then
        local seedName = tostring(arg1)
        local qty = tonumber(arg2) or 99
        local seedsFolder = ReplicatedStorage:FindFirstChild("Seeds")
        local backpack = player:FindFirstChildOfClass("Backpack")
        local starterGear = player:FindFirstChildOfClass("StarterGear")
        if backpack then
            local fullName = seedName
            if not fullName:find("Seed") then fullName = fullName .. " Seed" end
            local tool = seedsFolder and (seedsFolder:FindFirstChild(fullName) or seedsFolder:FindFirstChild(seedName))
            if not tool and seedsFolder then
                for _, t in ipairs(seedsFolder:GetChildren()) do
                    if t:GetAttribute("Seed") == seedName or t.Name:lower():find(seedName:lower()) then
                        tool = t
                        break
                    end
                end
            end
            if tool then
                tool = tool:Clone()
            else
                tool = Instance.new("Tool")
                tool.Name = fullName
                tool:SetAttribute("Seed", seedName)
                local h = Instance.new("Part")
                h.Name = "Handle"
                h.Size = Vector3.new(0.8, 0.8, 0.8)
                h.CanCollide = false
                h.Anchored = false
                h.Parent = tool
            end
            tool:SetAttribute("Quantity", qty)
            tool.Parent = backpack
            if starterGear and not starterGear:FindFirstChild(tool.Name) then
                tool:Clone().Parent = starterGear
            end
            print(string.format("[Admin] %s tohumu (%s) oyuncusuna verildi.", tool.Name, player.Name))
        end

    elseif action == "GiveCrop" then
        local cropName = tostring(arg1)
        local backpack = player:FindFirstChildOfClass("Backpack")
        if backpack then
            local cleanCrop = FruitVisualHelper.getCanonicalPlantName(cropName)
            local rollWeight = tonumber(arg2) or (math.round((1.0 + math.random() * 2.5) * 100) / 100)
            local rollVariant = "Normal"
            local roll = math.random(1, 100)
            if roll <= 4 then rollVariant = "Rainbow"
            elseif roll <= 14 then rollVariant = "Gold" end

            local val = math.floor(25 * (rollWeight / 1.5))
            if rollVariant == "Gold" then val = val * 2.5
            elseif rollVariant == "Rainbow" then val = val * 5 end

            local cropTool = FruitVisualHelper.createCropTool(cleanCrop, cleanCrop, rollWeight, rollVariant, val)
            cropTool.Parent = backpack
            print(string.format("[Admin] %s ürünü (%s) oyuncusuna verildi.", cropTool.Name, player.Name))
        end

    elseif action == "ClearSeeds" then
        local backpack = player:FindFirstChildOfClass("Backpack")
        if backpack then
            for _, item in ipairs(backpack:GetChildren()) do
                if item:IsA("Tool") and (item:GetAttribute("Seed") or item.Name:find("Seed")) then
                    item:Destroy()
                end
            end
        end
        local char = player.Character
        if char then
            for _, item in ipairs(char:GetChildren()) do
                if item:IsA("Tool") and (item:GetAttribute("Seed") or item.Name:find("Seed")) then
                    item:Destroy()
                end
            end
        end
        print(string.format("[Admin] %s oyuncusunun envanterindeki tohumlar temizlendi.", player.Name))
    end
end)

-- Oyuncu katıldığında admin olup olmadığını doğrula ve haber ver
local function checkAdminStatus(player)
    if isDeveloper(player) then
        player:SetAttribute("IsDeveloperAdmin", true)
        print(string.format("[AdminServerManager] Geliştirici yetkisi tanındı: %s", player.Name))
    end
end

Players.PlayerAdded:Connect(checkAdminStatus)
for _, p in ipairs(Players:GetPlayers()) do
    task.spawn(checkAdminStatus, p)
end