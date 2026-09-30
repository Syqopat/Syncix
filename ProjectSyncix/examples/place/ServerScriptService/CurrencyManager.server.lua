-- CurrencyManager (ServerScriptService)
-- Sol alttaki para göstergesini ve ErimYancar, 0Ben_ege0, Rakunkee için 20 SX (20 Sextillion) başlangıç parasını yönetir.

local Players = game:GetService("Players")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local RunService = game:GetService("RunService")

-- 20 SX = 20 * 10^21 (20 Sextillion)
local TWENTY_SX = 20 * (10 ^ 21)

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

-- Appear_Effect TextLabel güvencesi (Sheckles_UI için)
local function ensureAppearEffect()
    local appearEffect = ReplicatedStorage:FindFirstChild("Appear_Effect")
    if not appearEffect then
        appearEffect = Instance.new("TextLabel")
        appearEffect.Name = "Appear_Effect"
        appearEffect.Size = UDim2.new(0, 160, 0, 40)
        appearEffect.BackgroundTransparency = 1
        appearEffect.Font = Enum.Font.FredokaOne
        appearEffect.TextScaled = true
        appearEffect.TextColor3 = Color3.fromRGB(255, 230, 80)
        appearEffect.TextStrokeColor3 = Color3.fromRGB(30, 30, 30)
        appearEffect.TextStrokeTransparency = 0.2
        appearEffect.Text = "+0¢"
        appearEffect.Parent = ReplicatedStorage
    end
end

ensureAppearEffect()

-- RemoteEvent senkronizasyonu
local DataUpdateEvent = ReplicatedStorage:FindFirstChild("DataUpdateEvent")
if not DataUpdateEvent then
    DataUpdateEvent = Instance.new("RemoteEvent")
    DataUpdateEvent.Name = "DataUpdateEvent"
    DataUpdateEvent.Parent = ReplicatedStorage
end

local function setupPlayerCurrency(player)
    -- leaderstats yerine PlayerData kullanarak sağ üstteki gereksiz Roblox tablosunu engelleriz
    -- Ancak standart erişim için hem leaderstats hem PlayerData desteklenir
    local dataFolder = player:FindFirstChild("PlayerData")
    if not dataFolder then
        dataFolder = Instance.new("Folder")
        dataFolder.Name = "PlayerData"
        dataFolder.Parent = player
    end

    local sheckles = dataFolder:FindFirstChild("Sheckles")
    if not sheckles then
        sheckles = Instance.new("NumberValue")
        sheckles.Name = "Sheckles"
        sheckles.Parent = dataFolder
    end

    -- Geliştiricilere (ErimYancar, 0Ben_ege0, Rakunkee ve Studio testine) 20 SX para tanımla
    if isDeveloper(player) then
        sheckles.Value = TWENTY_SX
        print(string.format("[CurrencyManager] %s geliştiricisine 20 SX para tanımlandı!", player.Name))
    else
        if sheckles.Value == 0 then
            sheckles.Value = 1000
        end
    end

    -- leaderstats uyumluluğu (eğer dış scriptler ararsa)
    local leaderstats = player:FindFirstChild("leaderstats")
    if not leaderstats then
        leaderstats = Instance.new("Folder")
        leaderstats.Name = "leaderstats"
        leaderstats.Parent = player
    end
    local lsSheckles = leaderstats:FindFirstChild("Sheckles")
    if not lsSheckles then
        lsSheckles = Instance.new("NumberValue")
        lsSheckles.Name = "Sheckles"
        lsSheckles.Value = sheckles.Value
        lsSheckles.Parent = leaderstats
    end

    sheckles.Changed:Connect(function(newVal)
        if lsSheckles.Value ~= newVal then
            lsSheckles.Value = newVal
        end
        DataUpdateEvent:FireClient(player, "Sheckles", newVal)
    end)

    lsSheckles.Changed:Connect(function(newVal)
        if sheckles.Value ~= newVal then
            sheckles.Value = newVal
        end
    end)

    -- Client'a ilk veriyi yolla
    task.delay(0.3, function()
        if player:IsDescendantOf(Players) then
            DataUpdateEvent:FireClient(player, "Sheckles", sheckles.Value)
        end
    end)
end

Players.PlayerAdded:Connect(setupPlayerCurrency)
for _, p in ipairs(Players:GetPlayers()) do
    task.spawn(setupPlayerCurrency, p)
end
