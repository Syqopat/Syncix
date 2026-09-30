-- Sheckles_UI TextLabel Controller (Client)
-- Sol alttaki para göstergesini (20 Sx ¢ vb.) yönetir ve günceller.

local Players = game:GetService("Players")
local StarterGui = game:GetService("StarterGui")
local TweenService = game:GetService("TweenService")
local Debris = game:GetService("Debris")
local SoundService = game:GetService("SoundService")
local ReplicatedStorage = game:GetService("ReplicatedStorage")

-- Sağ üstteki gereksiz Roblox leaderboard tablosunu gizle (Sadece sol alttaki para görünsün)
pcall(function()
    StarterGui:SetCoreGuiEnabled(Enum.CoreGuiType.PlayerList, false)
end)

local player = Players.LocalPlayer
local label = script.Parent
local valObject = label:FindFirstChild("val")

local SUFFIXES = {
    { 1e33, "Dc" },
    { 1e30, "No" },
    { 1e27, "Oc" },
    { 1e24, "Sp" },
    { 1e21, "Sx" },
    { 1e18, "Qi" },
    { 1e15, "Qa" },
    { 1e12, "T" },
    { 1e9, "B" },
    { 1e6, "M" },
    { 1e3, "k" },
}

local function formatNumber(n)
    n = tonumber(n) or 0
    if n >= 1e6 then
        for _, s in ipairs(SUFFIXES) do
            if n >= s[1] then
                local val = n / s[1]
                local str = string.format("%.2f", val):gsub("%.00$", ""):gsub("(%..-)0+$", "%1")
                return str .. " " .. s[2]
            end
        end
    end

    -- 1M altı virgüllü sayı formatı
    local formatted = string.format("%.0f", n)
    local k
    while true do
        formatted, k = string.gsub(formatted, "^(-?%d+)(%d%d%d)", "%1,%2")
        if k == 0 then break end
    end
    return formatted
end

local lastValue = 0

local function showPopupEffect(diff, isGain)
    local appearEffect = ReplicatedStorage:FindFirstChild("Appear_Effect")
    if not appearEffect then return end

    local popup = appearEffect:Clone()
    popup.Parent = label.Parent
    popup.Position = label.Position
    popup.TextColor3 = isGain and Color3.fromRGB(255, 230, 50) or Color3.fromRGB(240, 50, 50)
    popup.Text = (isGain and "+" or "-") .. formatNumber(diff) .. "¢"

    local tweenInfo = TweenInfo.new(0.9, Enum.EasingStyle.Quad, Enum.EasingDirection.Out)
    local targetPos = popup.Position - UDim2.new(0, 0, 0.08, 0)

    local moveTween = TweenService:Create(popup, tweenInfo, { Position = targetPos })
    local fadeTween = TweenService:Create(popup, tweenInfo, { TextTransparency = 1, TextStrokeTransparency = 1 })

    moveTween:Play()
    fadeTween:Play()
    Debris:AddItem(popup, 1.0)

    local cashSound = SoundService:FindFirstChild("Cash Register")
    if cashSound then
        cashSound.TimePosition = 0
        cashSound.PlaybackSpeed = 1.0 + math.random(-10, 10) / 100
        cashSound:Play()
    end
end

local function updateDisplay(newValue, playEffect)
    newValue = tonumber(newValue) or 0

    if valObject and valObject:IsA("NumberValue") then
        valObject.Value = newValue
    end

    label.Text = formatNumber(newValue) .. "¢"
    label.Visible = true
    if label.Parent and label.Parent:IsA("ScreenGui") then
        label.Parent.Enabled = true
    end

    if playEffect and lastValue ~= newValue then
        local diff = math.abs(newValue - lastValue)
        if diff > 0 then
            showPopupEffect(diff, newValue > lastValue)
        end
    end

    lastValue = newValue
end

-- 1. leaderstats üzerinden değer takibi
task.spawn(function()
    local leaderstats = player:WaitForChild("leaderstats", 15)
    if leaderstats then
        local sheckles = leaderstats:WaitForChild("Sheckles", 15) or leaderstats:WaitForChild("Money", 5)
        if sheckles then
            updateDisplay(sheckles.Value, false)
            sheckles.Changed:Connect(function(newVal)
                updateDisplay(newVal, true)
            end)
        end
    end
end)

-- 2. DataUpdateEvent RemoteEvent takibi (Server senkronizasyonu)
local DataUpdateEvent = ReplicatedStorage:WaitForChild("DataUpdateEvent", 15)
if DataUpdateEvent then
    DataUpdateEvent.OnClientEvent:Connect(function(path, val)
        if path == "Sheckles" or path == "Money" then
            updateDisplay(val, true)
        end
    end)
end

-- Başlangıçta doğrudan kontrol et
if valObject and valObject.Value > 0 then
    updateDisplay(valObject.Value, false)
end