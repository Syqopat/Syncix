-- IslandManager.server.lua
-- Kesin Oyuncu Ada Atama, Yerel Roblox Team Doğuş (Spawn) ve Işınlanma Yöneticisi

local Players = game:GetService("Players")
local Workspace = game:GetService("Workspace")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local Teams = game:GetService("Teams")
local StarterPack = game:GetService("StarterPack")
local RunService = game:GetService("RunService")

local farmFolder = Workspace:WaitForChild("Farm", 15)

-- RemoteEvents (Mevcut statik dosyaları kullan veya oluştur)
local TeleportEvent = ReplicatedStorage:FindFirstChild("TeleportEvent")
if not TeleportEvent then
    TeleportEvent = Instance.new("RemoteEvent")
    TeleportEvent.Name = "TeleportEvent"
    TeleportEvent.Parent = ReplicatedStorage
end

local ClientTeleport = ReplicatedStorage:FindFirstChild("ClientTeleport")
if not ClientTeleport then
    ClientTeleport = Instance.new("RemoteEvent")
    ClientTeleport.Name = "ClientTeleport"
    ClientTeleport.Parent = ReplicatedStorage
end

-- Gökyüzündeki hatalı SpawnLocation'ı tamamen yok et
local defaultSpawn = Workspace:FindFirstChild("SpawnLocation")
if defaultSpawn then
    if defaultSpawn:IsA("SpawnLocation") then
        defaultSpawn.Enabled = false
        defaultSpawn.Neutral = false
    end
    defaultSpawn:Destroy()
end

-- Boşluğa (void) düşen oyuncuların parçalarının motor seviyesinde silinmesini ve ölmesini engelle
pcall(function()
    Workspace.FallenPartsDestroyHeight = -5000
end)

-- 8 Ada ve Özel Takım Renkleri
local ISLAND_CONFIG = {
    { name = "Island_East_1",  color = BrickColor.new("Bright red") },
    { name = "Island_East_2",  color = BrickColor.new("Bright blue") },
    { name = "Island_North_1", color = BrickColor.new("Bright yellow") },
    { name = "Island_North_2", color = BrickColor.new("Bright green") },
    { name = "Island_South_1", color = BrickColor.new("Bright orange") },
    { name = "Island_South_2", color = BrickColor.new("Dark green") },
    { name = "Island_West_1",  color = BrickColor.new("Bright violet") },
    { name = "Island_West_2",  color = BrickColor.new("Dark stone grey") },
}

local islandTeams = {}

-- 1. Takımları ve Ada SpawnLocation'larını Yapılandır
for _, cfg in ipairs(ISLAND_CONFIG) do
    local team = Teams:FindFirstChild(cfg.name)
    if not team then
        team = Instance.new("Team")
        team.Name = cfg.name
        team.TeamColor = cfg.color
        team.AutoAssignable = false
        team.Parent = Teams
    else
        team.TeamColor = cfg.color
        team.AutoAssignable = false
    end
    islandTeams[cfg.name] = team

    -- İlgili adanın Spawn_Point'ini bu takıma bağla
    if farmFolder then
        local farm = farmFolder:FindFirstChild("Farm_" .. cfg.name)
        if farm then
            local sp = farm:FindFirstChild("Spawn_Point", true)
            if sp and sp:IsA("SpawnLocation") then
                sp.Enabled = true
                sp.Neutral = false
                sp.TeamColor = cfg.color
            end
        end
    end
end

-- 2. Sunucu Açılışında Tüm Sahiplikleri Temizle
if farmFolder then
    for _, farm in ipairs(farmFolder:GetChildren()) do
        local data = farm:FindFirstChild("Important") and farm.Important:FindFirstChild("Data")
        if data and data:FindFirstChild("Owner") then
            local cur = data.Owner.Value
            if cur ~= "None" and cur ~= "" and not Players:FindFirstChild(cur) then
                data.Owner.Value = "None"
            end
        end
    end
end

-- Bir tarlanın doğuş noktasını en güvenli şekilde hesapla
local function getFarmSpawnCFrame(farm)
    if not farm then return CFrame.new(0, 127.5, 0) end

    -- 1. Tarladaki Spawn_Point kontrolü
    local sp = farm:FindFirstChild("Spawn_Point", true)
    if sp and sp:IsA("BasePart") then
        return CFrame.new(sp.Position + Vector3.new(0, 3.5, 0))
    end

    -- 2. Tarladaki Can_Plant1 kontrolü
    local plantLocations = farm:FindFirstChild("Important") and farm.Important:FindFirstChild("Plant_Locations")
    if plantLocations then
        local cp1 = plantLocations:FindFirstChild("Can_Plant1")
        if cp1 and cp1:IsA("BasePart") then
            return CFrame.new(cp1.Position + Vector3.new(0, 4.0, 0))
        end
    end

    -- 3. BoundingBox
    local cf, sz = farm:GetBoundingBox()
    return CFrame.new(cf.Position + Vector3.new(0, 8.0, 0))
end

-- Oyuncuya boş bir ada tahsis et ve Takımını ata
local function assignIslandToPlayer(player)
    if not farmFolder then return nil end

    local assignedFarm = nil

    -- 1. Zaten adası atanmışsa
    local currentAssigned = player:GetAttribute("AssignedIsland")
    if currentAssigned and farmFolder:FindFirstChild("Farm_" .. currentAssigned) then
        assignedFarm = farmFolder["Farm_" .. currentAssigned]
    end

    -- 2. Daha önceden oyuncu adına kayıtlı tarlayı bul
    if not assignedFarm then
        for _, farm in ipairs(farmFolder:GetChildren()) do
            local data = farm:FindFirstChild("Important") and farm.Important:FindFirstChild("Data")
            if data and data:FindFirstChild("Owner") and data.Owner.Value == player.Name then
                assignedFarm = farm
                break
            end
        end
    end

    -- 3. Boş olan ilk tarlayı oyuncuya bağla
    if not assignedFarm then
        for _, farm in ipairs(farmFolder:GetChildren()) do
            local data = farm:FindFirstChild("Important") and farm.Important:FindFirstChild("Data")
            if data and data:FindFirstChild("Owner") and (data.Owner.Value == "None" or data.Owner.Value == "") then
                assignedFarm = farm
                break
            end
        end
    end

    -- 4. Tümü doluysa ilk adayı fallback olarak ver
    if not assignedFarm then
        assignedFarm = farmFolder:GetChildren()[1]
    end

    if assignedFarm then
        local islName = string.gsub(assignedFarm.Name, "Farm_", "")
        player:SetAttribute("AssignedIsland", islName)
        player:SetAttribute("FarmName", assignedFarm.Name)

        local data = assignedFarm:FindFirstChild("Important") and assignedFarm.Important:FindFirstChild("Data")
        if data and data:FindFirstChild("Owner") then
            data.Owner.Value = player.Name
        end

        if islandTeams[islName] then
            player.Team = islandTeams[islName]
        end

        -- Roblox'un yerel C++ respawner'ına bu adayı kesin hedef olarak tanıt
        local sp = assignedFarm:FindFirstChild("Spawn_Point", true)
        if sp and sp:IsA("SpawnLocation") then
            sp.Enabled = true
            player.RespawnLocation = sp
        end

        return assignedFarm
    end

    return nil
end

-- Karakteri fizik motoruna karşı kesin şekilde ışınla ve sabitle
local function robustTeleport(player, targetCF)
    local char = player.Character
    if not char then return end
    local hrp = char:FindFirstChild("HumanoidRootPart")
    if not hrp then return end

    if not char.PrimaryPart then
        char.PrimaryPart = hrp
    end

    local hum = char:FindFirstChildOfClass("Humanoid")
    if hum and hum.Health > 0 then
        hum.Health = hum.MaxHealth
        hum:ChangeState(Enum.HumanoidStateType.GettingUp)
    end

    hrp.AssemblyLinearVelocity = Vector3.zero
    hrp.AssemblyAngularVelocity = Vector3.zero
    hrp.CFrame = targetCF
    char:PivotTo(targetCF)

    -- İstemciye haber ver
    TeleportEvent:FireClient(player, "Teleport", targetCF)
    ClientTeleport:FireClient(player, targetCF)

    -- Roblox fizik motorunun geri çekmesini engellemek için sabitle
    task.spawn(function()
        for _, delayTime in ipairs({0.05, 0.15, 0.35, 0.7}) do
            task.wait(delayTime)
            if char and char.Parent and hrp and hrp.Parent then
                hrp.AssemblyLinearVelocity = Vector3.zero
                hrp.AssemblyAngularVelocity = Vector3.zero
                hrp.CFrame = targetCF
                char:PivotTo(targetCF)
            end
        end
    end)
end

local ALL_15_SEEDS = {
    { name = "Carrot Seed",           seed = "Carrot",           qty = 20, color = Color3.fromRGB(245, 115, 15) },
    { name = "Potato Seed",           seed = "Potato",           qty = 15, color = Color3.fromRGB(150, 110, 65) },
    { name = "Strawberry Seed",       seed = "Strawberry",       qty = 15, color = Color3.fromRGB(225, 25, 40) },
    { name = "Blueberry Seed",        seed = "Blueberry",        qty = 15, color = Color3.fromRGB(38, 70, 180) },
    { name = "Apple Tree Seed",       seed = "Apple Tree",       qty = 10, color = Color3.fromRGB(220, 20, 25) },
    { name = "Mango Tree Seed",       seed = "Mango Tree",       qty = 10, color = Color3.fromRGB(255, 175, 35) },
    { name = "Frostleaf Seed",        seed = "Frostleaf",        qty = 10, color = Color3.fromRGB(160, 230, 255) },
    { name = "Cherry Tree Seed",      seed = "Cherry Tree",      qty = 10, color = Color3.fromRGB(190, 10, 30) },
    { name = "Banana Tree Seed",      seed = "Banana Tree",      qty = 10, color = Color3.fromRGB(255, 215, 30) },
    { name = "Dragonfruit Bush Seed", seed = "Dragonfruit Bush", qty = 10, color = Color3.fromRGB(240, 30, 120) },
    { name = "Starfruit Plant Seed",  seed = "Starfruit Plant",  qty = 10, color = Color3.fromRGB(255, 215, 25) },
    { name = "Coconut Palm Seed",     seed = "Coconut Palm",     qty = 10, color = Color3.fromRGB(115, 75, 45) },
    { name = "Crystal Shrub Seed",    seed = "Crystal Shrub",    qty = 5,  color = Color3.fromRGB(185, 75, 255) },
    { name = "Nebula Vine Seed",      seed = "Nebula Vine",      qty = 5,  color = Color3.fromRGB(195, 50, 230) },
    { name = "Celestial Tree Seed",   seed = "Celestial Tree",   qty = 5,  color = Color3.fromRGB(255, 245, 180) },
}

local function createSeedTool(data)
    local tool = Instance.new("Tool")
    tool.Name = string.format("%s X%d", data.name, data.qty)
    tool.ToolTip = string.format("%s (x%d)", data.seed, data.qty)
    tool:SetAttribute("Seed", data.seed)
    tool:SetAttribute("Quantity", data.qty)
    tool.CanBeDropped = false

    local handle = Instance.new("Part")
    handle.Name = "Handle"
    handle.Size = Vector3.new(0.8, 0.8, 0.8)
    handle.Color = data.color
    handle.Material = Enum.Material.Wood
    handle.CanCollide = false
    handle.Anchored = false
    handle.Parent = tool

    return tool
end

-- StarterPack'i 15 kanonik tohumla senkronize et, eski tohumları (Corn, Tomato, Watermelon vb.) temizle
local function setupStarterPack()
    for _, child in ipairs(StarterPack:GetChildren()) do
        if child:IsA("Tool") then
            local isCanon = false
            for _, data in ipairs(ALL_15_SEEDS) do
                if child:GetAttribute("Seed") == data.seed or child.Name:find(data.seed) then
                    isCanon = true
                    break
                end
            end
            if not isCanon then
                child:Destroy()
            end
        end
    end

    for _, data in ipairs(ALL_15_SEEDS) do
        local exists = false
        for _, child in ipairs(StarterPack:GetChildren()) do
            if child:IsA("Tool") and (child:GetAttribute("Seed") == data.seed or child.Name:find(data.seed)) then
                exists = true
                child:SetAttribute("Seed", data.seed)
                child:SetAttribute("Quantity", data.qty)
                child.Name = string.format("%s X%d", data.name, data.qty)
                break
            end
        end
        if not exists then
            local t = createSeedTool(data)
            t.Parent = StarterPack
        end
    end
end

setupStarterPack()

local function ensurePlayerHasAll15Seeds(player)
    local backpack = player:FindFirstChildOfClass("Backpack")
    local starterGear = player:FindFirstChildOfClass("StarterGear")
    local char = player.Character

    -- 1. Listede olmayan eski tohumları (Corn, Tomato, Watermelon vb.) temizle
    local function cleanObsolete(container)
        if not container then return end
        for _, item in ipairs(container:GetChildren()) do
            if item:IsA("Tool") and not item:GetAttribute("Item_String") then
                local isCanon = false
                for _, data in ipairs(ALL_15_SEEDS) do
                    if item:GetAttribute("Seed") == data.seed or item.Name:find(data.seed) then
                        isCanon = true
                        break
                    end
                end
                if not isCanon and item.Name:find("Seed") then
                    item:Destroy()
                end
            end
        end
    end

    cleanObsolete(backpack)
    cleanObsolete(char)
    cleanObsolete(starterGear)

    if not backpack then return end

    -- 2. 15 tohumun tamamının oyuncuda eksiksiz bulunmasını sağla
    for _, data in ipairs(ALL_15_SEEDS) do
        local existingTool = nil
        if char then
            for _, item in ipairs(char:GetChildren()) do
                if item:IsA("Tool") and (item:GetAttribute("Seed") == data.seed or item.Name:find(data.seed)) then
                    existingTool = item
                    break
                end
            end
        end
        if not existingTool and backpack then
            for _, item in ipairs(backpack:GetChildren()) do
                if item:IsA("Tool") and (item:GetAttribute("Seed") == data.seed or item.Name:find(data.seed)) then
                    existingTool = item
                    break
                end
            end
        end

        if existingTool then
            if not existingTool:GetAttribute("Seed") then
                existingTool:SetAttribute("Seed", data.seed)
            end
            local currentQty = existingTool:GetAttribute("Quantity")
            if not currentQty or currentQty <= 0 then
                existingTool:SetAttribute("Quantity", data.qty)
                local baseName = existingTool.Name:gsub("%s*X%d+", ""):gsub("%s*x%d+", "")
                existingTool.Name = string.format("%s X%d", baseName, data.qty)
            end
        else
            local newTool = createSeedTool(data)
            newTool.Parent = backpack

            if starterGear and not starterGear:FindFirstChild(newTool.Name) then
                newTool:Clone().Parent = starterGear
            end
        end
    end
end


-- Oyuncu Katıldığında & Karakter Doğduğunda
local function onCharacter(player, char)
    local farm = assignIslandToPlayer(player)
    if farm then
        local sp = farm:FindFirstChild("Spawn_Point", true)
        if sp and sp:IsA("SpawnLocation") then
            sp.Enabled = true
            sp.Neutral = false
            player.RespawnLocation = sp
        end
        local targetCF = getFarmSpawnCFrame(farm)

        task.spawn(function()
            local hrp = char:WaitForChild("HumanoidRootPart", 10)
            if hrp then
                if not char.PrimaryPart then char.PrimaryPart = hrp end
                robustTeleport(player, targetCF)
            end
        end)
    end
    task.defer(function()
        ensurePlayerHasAll15Seeds(player)
    end)
end

Players.PlayerAdded:Connect(function(player)
    -- Karakter doğmadan ÖNCE ada ve Takım ata (böylece Roblox yerel spawner doğrudan o adaya doğurur)
    assignIslandToPlayer(player)

    player.CharacterAdded:Connect(function(char)
        onCharacter(player, char)
    end)

    if player.Character then
        task.spawn(onCharacter, player, player.Character)
    end
end)

local VOID_Y_THRESHOLD = -175
local lastVoidTeleport = {}

local function handleVoidRescue(player)
    local now = os.clock()
    if lastVoidTeleport[player] and (now - lastVoidTeleport[player] < 1.0) then
        return
    end
    lastVoidTeleport[player] = now

    local char = player.Character
    if not char then return end
    local hrp = char:FindFirstChild("HumanoidRootPart")
    local hum = char:FindFirstChildOfClass("Humanoid")
    if not hrp then return end

    if hum and hum.Health > 0 then
        hum.Health = hum.MaxHealth
        hum:ChangeState(Enum.HumanoidStateType.GettingUp)
    end

    local farm = assignIslandToPlayer(player)
    if farm then
        local targetCF = getFarmSpawnCFrame(farm)
        robustTeleport(player, targetCF)
        print(string.format("[IslandManager] 🪂 %s aşağı düştü, ölmeden kendi adasına (%s) ışınlandı.", player.Name, farm.Name))
    end
end

-- Sürekli boşluk kontrolü: Herhangi bir oyuncu haritadan aşağı düşerse ölmeden adasına ışınlanır
RunService.Heartbeat:Connect(function()
    for _, player in ipairs(Players:GetPlayers()) do
        local char = player.Character
        if char and char.Parent then
            local hrp = char:FindFirstChild("HumanoidRootPart")
            local hum = char:FindFirstChildOfClass("Humanoid")
            if hrp and hum and hum.Health > 0 then
                if hrp.Position.Y < VOID_Y_THRESHOLD then
                    handleVoidRescue(player)
                end
            end
        end
    end
end)

Players.PlayerRemoving:Connect(function(player)
    lastVoidTeleport[player] = nil
    local assigned = player:GetAttribute("AssignedIsland")
    if assigned and farmFolder and farmFolder:FindFirstChild("Farm_" .. assigned) then
        local farm = farmFolder["Farm_" .. assigned]
        local data = farm:FindFirstChild("Important") and farm.Important:FindFirstChild("Data")
        if data and data:FindFirstChild("Owner") and data.Owner.Value == player.Name then
            data.Owner.Value = "None"
            print(string.format("[IslandManager] 🚪 %s oyundan ayrıldı, %s serbest bırakıldı.", player.Name, farm.Name))
        end
    end
end)

-- Zaten sunucuda olan oyuncular için başlatma
for _, p in ipairs(Players:GetPlayers()) do
    local farm = assignIslandToPlayer(p)
    if p.Character then
        task.spawn(onCharacter, p, p.Character)
    end
end

-- SHOP Işınlanma Hedefi (Island Main Dükkanların Önü)
local SHOP_CF = CFrame.lookAt(Vector3.new(0, 127.5, -5), Vector3.new(10, 127.5, 5))

-- Işınlanma Eventi Dinleyicisi
TeleportEvent.OnServerEvent:Connect(function(player, action)
    if action == "Island" or action == "VoidFall" then
        handleVoidRescue(player)
    elseif action == "Shop" then
        robustTeleport(player, SHOP_CF)
    end
end)

print("[IslandManager] ✅ Herkesin kendi adasında doğması ve ışınlanma butonları garantilendi!")
