-- AdminPanelController (StarterPlayerScripts)
-- Geliştiriciler (ErimYancar, 0Ben_ege0, Rakunkee ve Studio testçisi) için test & admin paneli.
-- Para artırma/azaltma/ayarlama, uçma (fly), hız ve ışınlanma özelliklerini içerir.

local Players = game:GetService("Players")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local UserInputService = game:GetService("UserInputService")
local RunService = game:GetService("RunService")
local StarterGui = game:GetService("StarterGui")
local TweenService = game:GetService("TweenService")

local player = Players.LocalPlayer
local camera = workspace.CurrentCamera

-- Sağ üstteki gereksiz Roblox leaderboard tablosunu gizle (Sadece sol alttaki para görünsün)
pcall(function()
    StarterGui:SetCoreGuiEnabled(Enum.CoreGuiType.PlayerList, false)
end)

local DEVELOPERS = {
    ["erimyancar"] = true,
    ["0ben_ege0"] = true,
    ["rakunkee"] = true,
}

local function isDeveloper()
    if RunService:IsStudio() then return true end
    local name = player.Name:lower()
    local dName = player.DisplayName:lower()
    for dev, _ in pairs(DEVELOPERS) do
        if name:find(dev) or dName:find(dev) then
            return true
        end
    end
    return player:GetAttribute("IsDeveloperAdmin") == true
end

if not isDeveloper() then
    -- Yetkili değilse admin paneli hiç oluşturulmaz
    return
end

local AdminActionEvent = ReplicatedStorage:WaitForChild("AdminActionEvent", 10)
local DataUpdateEvent = ReplicatedStorage:FindFirstChild("DataUpdateEvent")

-- -------------------------------------------------------------
-- Sayı Formatlayıcı ve Ayrıştırıcı (20sx, 100k, vb.)
-- -------------------------------------------------------------
local SUFFIXES = {
    { 1e33, "Dc" }, { 1e30, "No" }, { 1e27, "Oc" }, { 1e24, "Sp" },
    { 1e21, "Sx" }, { 1e18, "Qi" }, { 1e15, "Qa" }, { 1e12, "T" },
    { 1e9, "B" },   { 1e6, "M" },   { 1e3, "k" },
}

local function formatAmount(n)
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
    local formatted = string.format("%.0f", n)
    local k
    while true do
        formatted, k = string.gsub(formatted, "^(-?%d+)(%d%d%d)", "%1,%2")
        if k == 0 then break end
    end
    return formatted
end

local function parseInputAmount(text)
    if not text then return 0 end
    text = text:gsub("%s+", ""):gsub(",", ""):lower()
    local mults = {
        sx = 1e21, qi = 1e18, qa = 1e15, t = 1e12, b = 1e9, m = 1e6, k = 1e3
    }
    for s, m in pairs(mults) do
        if text:sub(-#s) == s then
            local numStr = text:sub(1, -#s - 1)
            local val = tonumber(numStr)
            if val then return val * m end
        end
    end
    return tonumber(text) or 0
end

-- -------------------------------------------------------------
-- Uçma (Fly) Sistemi
-- -------------------------------------------------------------
local isFlying = false
local flySpeed = 45
local bodyGyro = nil
local bodyVelocity = nil

local function stopFlying()
    isFlying = false
    local char = player.Character
    local hum = char and char:FindFirstChildOfClass("Humanoid")
    if hum then
        hum.PlatformStand = false
    end
    if bodyGyro then bodyGyro:Destroy(); bodyGyro = nil end
    if bodyVelocity then bodyVelocity:Destroy(); bodyVelocity = nil end
end

local function startFlying()
    local char = player.Character
    local root = char and char:FindFirstChild("HumanoidRootPart")
    local hum = char and char:FindFirstChildOfClass("Humanoid")
    if not root or not hum then return end

    isFlying = true
    hum.PlatformStand = true

    bodyGyro = Instance.new("BodyGyro")
    bodyGyro.P = 9e4
    bodyGyro.MaxTorque = Vector3.new(9e9, 9e9, 9e9)
    bodyGyro.CFrame = root.CFrame
    bodyGyro.Parent = root

    bodyVelocity = Instance.new("BodyVelocity")
    bodyVelocity.Velocity = Vector3.zero
    bodyVelocity.MaxForce = Vector3.new(9e9, 9e9, 9e9)
    bodyVelocity.Parent = root
end

local function toggleFlight()
    if isFlying then
        stopFlying()
    else
        startFlying()
    end
end

RunService.RenderStepped:Connect(function()
    if not isFlying or not bodyVelocity or not bodyGyro then return end
    local char = player.Character
    local root = char and char:FindFirstChild("HumanoidRootPart")
    if not root then return end

    local camCF = camera.CFrame
    bodyGyro.CFrame = camCF

    local moveDir = Vector3.zero
    if UserInputService:IsKeyDown(Enum.KeyCode.W) then
        moveDir = moveDir + camCF.LookVector
    end
    if UserInputService:IsKeyDown(Enum.KeyCode.S) then
        moveDir = moveDir - camCF.LookVector
    end
    if UserInputService:IsKeyDown(Enum.KeyCode.A) then
        moveDir = moveDir - camCF.RightVector
    end
    if UserInputService:IsKeyDown(Enum.KeyCode.D) then
        moveDir = moveDir + camCF.RightVector
    end
    if UserInputService:IsKeyDown(Enum.KeyCode.Space) then
        moveDir = moveDir + Vector3.new(0, 1, 0)
    end
    if UserInputService:IsKeyDown(Enum.KeyCode.LeftShift) then
        moveDir = moveDir - Vector3.new(0, 1, 0)
    end

    if moveDir.Magnitude > 0 then
        bodyVelocity.Velocity = moveDir.Unit * flySpeed
    else
        bodyVelocity.Velocity = Vector3.zero
    end
end)

player.CharacterAdded:Connect(function()
    stopFlying()
end)

-- -------------------------------------------------------------
-- Admin Panel UI Yapısı
-- -------------------------------------------------------------
local playerGui = player:WaitForChild("PlayerGui")

local adminScreenGui = Instance.new("ScreenGui")
adminScreenGui.Name = "DeveloperAdminGui"
adminScreenGui.ResetOnSpawn = false
adminScreenGui.ZIndexBehavior = Enum.ZIndexBehavior.Sibling
adminScreenGui.Parent = playerGui

-- The DEV PANEL badge button was removed; the panel opens with F2 (see the key
-- handler further down).

-- Ana Panel Penceresi
local mainFrame = Instance.new("Frame")
mainFrame.Name = "AdminMainFrame"
mainFrame.Size = UDim2.new(0, 420, 0, 520)
mainFrame.Position = UDim2.new(0.5, -210, 0.5, -260)
mainFrame.BackgroundColor3 = Color3.fromRGB(18, 20, 26)
mainFrame.BorderSizePixel = 0
mainFrame.Visible = false
mainFrame.ClipsDescendants = true
mainFrame.Parent = adminScreenGui

local mainCorner = Instance.new("UICorner")
mainCorner.CornerRadius = UDim.new(0, 12)
mainCorner.Parent = mainFrame

local mainStroke = Instance.new("UIStroke")
mainStroke.Color = Color3.fromRGB(60, 65, 80)
mainStroke.Thickness = 1.5
mainStroke.Parent = mainFrame

-- Başlık Barı
local titleBar = Instance.new("Frame")
titleBar.Name = "TitleBar"
titleBar.Size = UDim2.new(1, 0, 0, 44)
titleBar.BackgroundColor3 = Color3.fromRGB(26, 28, 38)
titleBar.BorderSizePixel = 0
titleBar.Parent = mainFrame

local titleCorner = Instance.new("UICorner")
titleCorner.CornerRadius = UDim.new(0, 12)
titleCorner.Parent = titleBar

local titleLabel = Instance.new("TextLabel")
titleLabel.Size = UDim2.new(1, -50, 1, 0)
titleLabel.Position = UDim2.new(0, 15, 0, 0)
titleLabel.BackgroundTransparency = 1
titleLabel.Text = "🛠️ DEVELOPER TEST & ADMIN PANEL"
titleLabel.TextColor3 = Color3.fromRGB(255, 255, 255)
titleLabel.Font = Enum.Font.FredokaOne
titleLabel.TextSize = 15
titleLabel.TextXAlignment = Enum.TextXAlignment.Left
titleLabel.Parent = titleBar

local closeBtn = Instance.new("TextButton")
closeBtn.Name = "CloseButton"
closeBtn.Size = UDim2.new(0, 32, 0, 32)
closeBtn.Position = UDim2.new(1, -38, 0, 6)
closeBtn.BackgroundColor3 = Color3.fromRGB(40, 42, 54)
closeBtn.TextColor3 = Color3.fromRGB(255, 100, 100)
closeBtn.Text = "✕"
closeBtn.Font = Enum.Font.FredokaOne
closeBtn.TextSize = 16
closeBtn.Parent = titleBar

local closeCorner = Instance.new("UICorner")
closeCorner.CornerRadius = UDim.new(0, 6)
closeCorner.Parent = closeBtn

-- İçerik Kaydırma Alanı
local contentScroll = Instance.new("ScrollingFrame")
contentScroll.Name = "ContentScroll"
contentScroll.Size = UDim2.new(1, -20, 1, -54)
contentScroll.Position = UDim2.new(0, 10, 0, 48)
contentScroll.BackgroundTransparency = 1
contentScroll.BorderSizePixel = 0
contentScroll.ScrollBarThickness = 5
contentScroll.ScrollBarImageColor3 = Color3.fromRGB(80, 85, 105)
contentScroll.AutomaticCanvasSize = Enum.AutomaticSize.Y
contentScroll.CanvasSize = UDim2.new(0, 0, 0, 0)
contentScroll.Parent = mainFrame

local contentLayout = Instance.new("UIListLayout")
contentLayout.Padding = UDim.new(0, 12)
contentLayout.SortOrder = Enum.SortOrder.LayoutOrder
contentLayout.Parent = contentScroll

local function createSectionHeader(title, order)
    local lbl = Instance.new("TextLabel")
    lbl.Size = UDim2.new(1, 0, 0, 24)
    lbl.BackgroundTransparency = 1
    lbl.Text = title
    lbl.TextColor3 = Color3.fromRGB(255, 215, 60)
    lbl.Font = Enum.Font.FredokaOne
    lbl.TextSize = 15
    lbl.TextXAlignment = Enum.TextXAlignment.Left
    lbl.LayoutOrder = order
    lbl.Parent = contentScroll
    return lbl
end

-- =============================================================
-- BÖLÜM 1: PARA YÖNETİMİ & TEST
-- =============================================================
createSectionHeader("💰 SOL ALTTAKİ PARA DÜZENLEME & TEST", 10)

-- Canlı Bakiye Göstergesi
local liveBalanceBox = Instance.new("Frame")
liveBalanceBox.Size = UDim2.new(1, 0, 0, 36)
liveBalanceBox.BackgroundColor3 = Color3.fromRGB(26, 28, 38)
liveBalanceBox.LayoutOrder = 11
liveBalanceBox.Parent = contentScroll

local lbCorner = Instance.new("UICorner")
lbCorner.CornerRadius = UDim.new(0, 6)
lbCorner.Parent = liveBalanceBox

local liveBalanceLabel = Instance.new("TextLabel")
liveBalanceLabel.Size = UDim2.new(1, -20, 1, 0)
liveBalanceLabel.Position = UDim2.new(0, 10, 0, 0)
liveBalanceLabel.BackgroundTransparency = 1
liveBalanceLabel.Text = "Sol Alttaki Bakiye: Yükleniyor..."
liveBalanceLabel.TextColor3 = Color3.fromRGB(130, 230, 130)
liveBalanceLabel.Font = Enum.Font.GothamBold
liveBalanceLabel.TextSize = 13
liveBalanceLabel.TextXAlignment = Enum.TextXAlignment.Left
liveBalanceLabel.Parent = liveBalanceBox

-- Bakiye güncelleme dinleyicisi
local function refreshBalanceDisplay()
    local leaderstats = player:FindFirstChild("leaderstats") or player:FindFirstChild("PlayerData")
    local val = leaderstats and (leaderstats:FindFirstChild("Sheckles") or leaderstats:FindFirstChild("Money"))
    local current = val and val.Value or 0
    liveBalanceLabel.Text = "Sol Alttaki Bakiye: " .. formatAmount(current) .. " ¢ (" .. tostring(current) .. ")"
end

task.spawn(function()
    while true do
        refreshBalanceDisplay()
        task.wait(1.0)
    end
end)

-- Hızlı Butonlar Izgarası
local quickBtnFrame = Instance.new("Frame")
quickBtnFrame.Size = UDim2.new(1, 0, 0, 80)
quickBtnFrame.BackgroundTransparency = 1
quickBtnFrame.LayoutOrder = 12
quickBtnFrame.Parent = contentScroll

local grid = Instance.new("UIGridLayout")
grid.CellSize = UDim2.new(0, 125, 0, 34)
grid.CellPadding = UDim2.new(0, 8, 0, 8)
grid.SortOrder = Enum.SortOrder.LayoutOrder
grid.Parent = quickBtnFrame

local function makeBtn(parent, text, color, cb)
    local btn = Instance.new("TextButton")
    btn.BackgroundColor3 = color
    btn.TextColor3 = Color3.fromRGB(255, 255, 255)
    btn.Font = Enum.Font.FredokaOne
    btn.TextSize = 13
    btn.Text = text
    btn.Parent = parent

    local c = Instance.new("UICorner")
    c.CornerRadius = UDim.new(0, 6)
    c.Parent = btn

    btn.MouseButton1Click:Connect(cb)
    return btn
end

makeBtn(quickBtnFrame, "✨ 20 SX Yap", Color3.fromRGB(45, 140, 70), function()
    AdminActionEvent:FireServer("SetMoney", 20 * (10 ^ 21))
end)

makeBtn(quickBtnFrame, "+100 k", Color3.fromRGB(40, 110, 180), function()
    AdminActionEvent:FireServer("AddMoney", 100000)
end)

makeBtn(quickBtnFrame, "-100 k", Color3.fromRGB(180, 60, 60), function()
    AdminActionEvent:FireServer("SubtractMoney", 100000)
end)

makeBtn(quickBtnFrame, "+1 Sx", Color3.fromRGB(50, 120, 190), function()
    AdminActionEvent:FireServer("AddMoney", 1e21)
end)

makeBtn(quickBtnFrame, "-1 Sx", Color3.fromRGB(190, 50, 50), function()
    AdminActionEvent:FireServer("SubtractMoney", 1e21)
end)

makeBtn(quickBtnFrame, "🗑️ Parayı Sıfırla", Color3.fromRGB(100, 40, 40), function()
    AdminActionEvent:FireServer("SetMoney", 0)
end)

-- Özel Miktar Giriş Kutusu (TextBox)
local inputRow = Instance.new("Frame")
inputRow.Size = UDim2.new(1, 0, 0, 36)
inputRow.BackgroundTransparency = 1
inputRow.LayoutOrder = 13
inputRow.Parent = contentScroll

local amountBox = Instance.new("TextBox")
amountBox.Size = UDim2.new(0, 160, 1, 0)
amountBox.BackgroundColor3 = Color3.fromRGB(26, 28, 38)
amountBox.TextColor3 = Color3.fromRGB(255, 255, 255)
amountBox.PlaceholderText = "Miktar (örn: 20sx, 50m, 5000)"
amountBox.PlaceholderColor3 = Color3.fromRGB(120, 125, 145)
amountBox.Font = Enum.Font.Gotham
amountBox.TextSize = 12
amountBox.ClearTextOnFocus = false
amountBox.Parent = inputRow

local abCorner = Instance.new("UICorner")
abCorner.CornerRadius = UDim.new(0, 6)
abCorner.Parent = amountBox

makeBtn(inputRow, "Ayarla", Color3.fromRGB(55, 130, 200), function()
    local val = parseInputAmount(amountBox.Text)
    AdminActionEvent:FireServer("SetMoney", val)
end).Position = UDim2.new(0, 168, 0, 0)
inputRow:GetChildren()[#inputRow:GetChildren()].Size = UDim2.new(0, 70, 1, 0)

makeBtn(inputRow, "+ Ekle", Color3.fromRGB(45, 140, 70), function()
    local val = parseInputAmount(amountBox.Text)
    AdminActionEvent:FireServer("AddMoney", val)
end).Position = UDim2.new(0, 244, 0, 0)
inputRow:GetChildren()[#inputRow:GetChildren()].Size = UDim2.new(0, 70, 1, 0)

makeBtn(inputRow, "- Çıkar", Color3.fromRGB(170, 50, 50), function()
    local val = parseInputAmount(amountBox.Text)
    AdminActionEvent:FireServer("SubtractMoney", val)
end).Position = UDim2.new(0, 320, 0, 0)
inputRow:GetChildren()[#inputRow:GetChildren()].Size = UDim2.new(0, 70, 1, 0)

-- =============================================================
-- BÖLÜM 2: UÇMA (FLY) & HAREKET KONTROLÜ
-- =============================================================
createSectionHeader("✈️ UÇMA (FLY) & HAREKET KONTROLÜ", 20)

local flyBtn = nil
flyBtn = makeBtn(contentScroll, "✈️ Uçma Modu: KAPALI (Kısayol: F)", Color3.fromRGB(50, 60, 80), function()
    toggleFlight()
    if isFlying then
        flyBtn.Text = "🚀 Uçma Modu: AÇIK (Kısayol: F)"
        flyBtn.BackgroundColor3 = Color3.fromRGB(40, 150, 70)
    else
        flyBtn.Text = "✈️ Uçma Modu: KAPALI (Kısayol: F)"
        flyBtn.BackgroundColor3 = Color3.fromRGB(50, 60, 80)
    end
end)
flyBtn.Size = UDim2.new(1, 0, 0, 38)
flyBtn.LayoutOrder = 21

-- Hızlı Yürüme Hız Butonları (Grid)
local speedRow = Instance.new("Frame")
speedRow.Size = UDim2.new(1, 0, 0, 72)
speedRow.BackgroundTransparency = 1
speedRow.LayoutOrder = 22
speedRow.Parent = contentScroll

local sGrid = Instance.new("UIGridLayout")
sGrid.CellSize = UDim2.new(0, 125, 0, 32)
sGrid.CellPadding = UDim2.new(0, 8, 0, 8)
sGrid.Parent = speedRow

makeBtn(speedRow, "🚶 Normal (16)", Color3.fromRGB(35, 40, 52), function()
    AdminActionEvent:FireServer("SetSpeed", 16)
end)

makeBtn(speedRow, "⚡ Hızlı (45)", Color3.fromRGB(45, 80, 120), function()
    AdminActionEvent:FireServer("SetSpeed", 45)
end)

makeBtn(speedRow, "🏃 Koşu (75)", Color3.fromRGB(40, 120, 160), function()
    AdminActionEvent:FireServer("SetSpeed", 75)
end)

makeBtn(speedRow, "🔥 Flaş (120)", Color3.fromRGB(160, 90, 30), function()
    AdminActionEvent:FireServer("SetSpeed", 120)
end)

makeBtn(speedRow, "🚀 Sonic (250)", Color3.fromRGB(180, 50, 50), function()
    AdminActionEvent:FireServer("SetSpeed", 250)
end)

makeBtn(speedRow, "💖 Canı Doldur", Color3.fromRGB(160, 45, 80), function()
    AdminActionEvent:FireServer("Heal")
end)

-- Özel Hız Giriş Satırı (İstediğin Hızı Yazma)
local customSpeedRow = Instance.new("Frame")
customSpeedRow.Size = UDim2.new(1, 0, 0, 34)
customSpeedRow.BackgroundTransparency = 1
customSpeedRow.LayoutOrder = 23
customSpeedRow.Parent = contentScroll

local speedInputBox = Instance.new("TextBox")
speedInputBox.Size = UDim2.new(1, -90, 1, 0)
speedInputBox.BackgroundColor3 = Color3.fromRGB(26, 28, 38)
speedInputBox.TextColor3 = Color3.fromRGB(255, 255, 255)
speedInputBox.PlaceholderText = "Özel Hız Girin (örn: 100, 300, 500)"
speedInputBox.PlaceholderColor3 = Color3.fromRGB(120, 125, 145)
speedInputBox.Font = Enum.Font.Gotham
speedInputBox.TextSize = 12
speedInputBox.ClearTextOnFocus = false
speedInputBox.Parent = customSpeedRow

local sibCorner = Instance.new("UICorner")
sibCorner.CornerRadius = UDim.new(0, 6)
sibCorner.Parent = speedInputBox

local setSpeedBtn = makeBtn(customSpeedRow, "Ayarla", Color3.fromRGB(50, 130, 200), function()
    local spd = tonumber(speedInputBox.Text) or 16
    AdminActionEvent:FireServer("SetSpeed", spd)
    flySpeed = spd
    speedInputBox.Text = ""
end)
setSpeedBtn.Position = UDim2.new(1, -82, 0, 0)
setSpeedBtn.Size = UDim2.new(0, 82, 1, 0)

-- =============================================================
-- BÖLÜM 2.5: 🌱 BİTKİ BÜYÜME HIZI (GROWTH SPEED & DEV)
-- =============================================================
createSectionHeader("🌱 BİTKİ BÜYÜME HIZI & ANINDA OLGUNLAŞTIRMA", 24)

local growSpeedGrid = Instance.new("Frame")
growSpeedGrid.Size = UDim2.new(1, 0, 0, 72)
growSpeedGrid.BackgroundTransparency = 1
growSpeedGrid.LayoutOrder = 24
growSpeedGrid.Parent = contentScroll

local gsGrid = Instance.new("UIGridLayout")
gsGrid.CellSize = UDim2.new(0, 125, 0, 32)
gsGrid.CellPadding = UDim2.new(0, 8, 0, 8)
gsGrid.Parent = growSpeedGrid

makeBtn(growSpeedGrid, "⚡ ANINDA (0.1s)", Color3.fromRGB(200, 140, 20), function()
    AdminActionEvent:FireServer("SetGrowthMultiplier", 999)
end)

makeBtn(growSpeedGrid, "🚀 10x Büyüme", Color3.fromRGB(40, 150, 80), function()
    AdminActionEvent:FireServer("SetGrowthMultiplier", 10)
end)

makeBtn(growSpeedGrid, "⏩ 5x Büyüme", Color3.fromRGB(45, 110, 170), function()
    AdminActionEvent:FireServer("SetGrowthMultiplier", 5)
end)

makeBtn(growSpeedGrid, "🌱 1x Normal", Color3.fromRGB(50, 60, 75), function()
    AdminActionEvent:FireServer("SetGrowthMultiplier", 1)
end)

makeBtn(growSpeedGrid, "🌟 Tümünü Büyüt", Color3.fromRGB(160, 60, 140), function()
    AdminActionEvent:FireServer("InstaGrowAll")
end)

makeBtn(growSpeedGrid, "🔥 50x Süper", Color3.fromRGB(190, 50, 40), function()
    AdminActionEvent:FireServer("SetGrowthMultiplier", 50)
end)

-- =============================================================
-- BÖLÜM 3: TOHUM & ENVANTER YÖNETİMİ
-- =============================================================
createSectionHeader("🌱 TOHUM & ENVANTER YÖNETİMİ (SEEDS)", 25)

local seedActionsRow = Instance.new("Frame")
seedActionsRow.Size = UDim2.new(1, 0, 0, 36)
seedActionsRow.BackgroundTransparency = 1
seedActionsRow.LayoutOrder = 26
seedActionsRow.Parent = contentScroll

local seedActLayout = Instance.new("UIListLayout")
seedActLayout.FillDirection = Enum.FillDirection.Horizontal
seedActLayout.Padding = UDim.new(0, 8)
seedActLayout.Parent = seedActionsRow

local giveAllSeedsBtn = nil
giveAllSeedsBtn = makeBtn(seedActionsRow, "🌟 TEK TUŞLA TÜM TOHUM LARI VER (40x)", Color3.fromRGB(35, 150, 75), function()
    AdminActionEvent:FireServer("GiveAllSeeds")
    giveAllSeedsBtn.Text = "✅ 40 Tohum Envantere Eklendi!"
    task.delay(1.5, function()
        if giveAllSeedsBtn and giveAllSeedsBtn.Parent then
            giveAllSeedsBtn.Text = "🌟 TEK TUŞLA TÜM TOHUM LARI VER (40x)"
        end
    end)
end)
giveAllSeedsBtn.Size = UDim2.new(1, -110, 1, 0)

local clearSeedsBtn = nil
clearSeedsBtn = makeBtn(seedActionsRow, "🗑️ Temizle", Color3.fromRGB(150, 45, 45), function()
    AdminActionEvent:FireServer("ClearSeeds")
    clearSeedsBtn.Text = "✅ Temiz!"
    task.delay(1.5, function()
        if clearSeedsBtn and clearSeedsBtn.Parent then
            clearSeedsBtn.Text = "🗑️ Temizle"
        end
    end)
end)
clearSeedsBtn.Size = UDim2.new(0, 102, 1, 0)

-- Tohum Arama Kutusu
-- Sekme Filtreleme Çubuğu (Tümü, Tohumlar, Bitkiler)
local filterTabsFrame = Instance.new("Frame")
filterTabsFrame.Size = UDim2.new(1, 0, 0, 32)
filterTabsFrame.BackgroundTransparency = 1
filterTabsFrame.LayoutOrder = 27
filterTabsFrame.Parent = contentScroll

local ftLayout = Instance.new("UIListLayout")
ftLayout.FillDirection = Enum.FillDirection.Horizontal
ftLayout.Padding = UDim.new(0, 6)
ftLayout.HorizontalAlignment = Enum.HorizontalAlignment.Left
ftLayout.Parent = filterTabsFrame

local currentFilter = "ALL" -- "ALL", "SEED", "CROP"
local tabButtons = {}

local function createTabBtn(text, filterKey)
    local tBtn = Instance.new("TextButton")
    tBtn.Size = UDim2.new(0, 115, 1, 0)
    tBtn.BackgroundColor3 = (currentFilter == filterKey) and Color3.fromRGB(45, 140, 75) or Color3.fromRGB(28, 34, 46)
    tBtn.Text = text
    tBtn.TextColor3 = Color3.fromRGB(255, 255, 255)
    tBtn.Font = Enum.Font.GothamBold
    tBtn.TextSize = 11
    tBtn.AutoButtonColor = true
    tBtn.Parent = filterTabsFrame

    local tc = Instance.new("UICorner")
    tc.CornerRadius = UDim.new(0, 6)
    tc.Parent = tBtn

    local ts = Instance.new("UIStroke")
    ts.Color = (currentFilter == filterKey) and Color3.fromRGB(70, 220, 120) or Color3.fromRGB(45, 55, 70)
    ts.Thickness = 1.2
    ts.Parent = tBtn

    tabButtons[filterKey] = { btn = tBtn, stroke = ts }
    return tBtn
end

local tabAll = createTabBtn("🌐 Tümü (Hepsi)", "ALL")
local tabSeeds = createTabBtn("🌱 Tohumlar (Seeds)", "SEED")
local tabCrops = createTabBtn("🍎 Bitkiler (Crops)", "CROP")

-- Tohum & Ürün Arama Kutusu
local seedSearchBox = Instance.new("TextBox")
seedSearchBox.Size = UDim2.new(1, 0, 0, 34)
seedSearchBox.BackgroundColor3 = Color3.fromRGB(26, 28, 38)
seedSearchBox.TextColor3 = Color3.fromRGB(255, 255, 255)
seedSearchBox.PlaceholderText = "🔍 Ara... (örn: Carrot, Apple, Cherry, Seed)"
seedSearchBox.PlaceholderColor3 = Color3.fromRGB(120, 125, 145)
seedSearchBox.Font = Enum.Font.Gotham
seedSearchBox.TextSize = 12
seedSearchBox.ClearTextOnFocus = false
seedSearchBox.LayoutOrder = 28
seedSearchBox.Parent = contentScroll

local sbCorner = Instance.new("UICorner")
sbCorner.CornerRadius = UDim.new(0, 6)
sbCorner.Parent = seedSearchBox

-- Izgara Konteyneri
local seedGridContainer = Instance.new("Frame")
seedGridContainer.Size = UDim2.new(1, 0, 0, 0)
seedGridContainer.AutomaticSize = Enum.AutomaticSize.Y
seedGridContainer.BackgroundTransparency = 1
seedGridContainer.LayoutOrder = 29
seedGridContainer.Parent = contentScroll

local seedGrid = Instance.new("UIGridLayout")
seedGrid.CellSize = UDim2.new(0, 125, 0, 32)
seedGrid.CellPadding = UDim2.new(0, 8, 0, 8)
seedGrid.SortOrder = Enum.SortOrder.LayoutOrder
seedGrid.Parent = seedGridContainer

local SEED_ICONS = {
    ["Carrot"] = "🥕", ["Potato"] = "🥔", ["Strawberry"] = "🍓", ["Blueberry"] = "🫐",
    ["Apple Tree"] = "🍎", ["Mango Tree"] = "🥭", ["Frostleaf"] = "❄️", ["Cherry Tree"] = "🌸",
    ["Banana Tree"] = "🍌", ["Dragonfruit Bush"] = "🐉", ["Starfruit Plant"] = "⭐", ["Coconut Palm"] = "🥥",
    ["Crystal Shrub"] = "🔮", ["Nebula Vine"] = "🌌", ["Celestial Tree"] = "🌟",
    ["Apple"] = "🍎", ["Banana"] = "🍌", ["Cherry"] = "🌸", ["Coconut"] = "🥥",
    ["Dragon Fruit"] = "🐉", ["Mango"] = "🥭", ["Starfruit"] = "⭐",
}

local itemDataList = {}

local function filterButtons()
    local query = seedSearchBox.Text:lower():gsub("%s+", "")
    for _, item in ipairs(itemDataList) do
        local btn = item.btn
        local matchTab = (currentFilter == "ALL") or (currentFilter == item.category)
        local matchQuery = (query == "") or item.searchKey:find(query, 1, true)
        btn.Visible = (matchTab and matchQuery)
    end
end

local function setFilter(newFilter)
    currentFilter = newFilter
    for key, data in pairs(tabButtons) do
        if key == currentFilter then
            data.btn.BackgroundColor3 = Color3.fromRGB(45, 140, 75)
            data.stroke.Color = Color3.fromRGB(70, 220, 120)
        else
            data.btn.BackgroundColor3 = Color3.fromRGB(28, 34, 46)
            data.stroke.Color = Color3.fromRGB(45, 55, 70)
        end
    end
    filterButtons()
end

tabAll.MouseButton1Click:Connect(function() setFilter("ALL") end)
tabSeeds.MouseButton1Click:Connect(function() setFilter("SEED") end)
tabCrops.MouseButton1Click:Connect(function() setFilter("CROP") end)

local function populateButtons()
    for _, item in ipairs(itemDataList) do
        if item.btn and item.btn.Parent then
            item.btn:Destroy()
        end
    end
    table.clear(itemDataList)

    local canonList = {
        { name = "Carrot",           seedName = "Carrot Seed",           cropName = "Carrot",           order = 1 },
        { name = "Potato",           seedName = "Potato Seed",           cropName = "Potato",           order = 2 },
        { name = "Strawberry",       seedName = "Strawberry Seed",       cropName = "Strawberry",       order = 3 },
        { name = "Blueberry",        seedName = "Blueberry Seed",        cropName = "Blueberry",        order = 4 },
        { name = "Apple Tree",       seedName = "Apple Tree Seed",       cropName = "Apple",            order = 5 },
        { name = "Mango Tree",       seedName = "Mango Tree Seed",       cropName = "Mango",            order = 6 },
        { name = "Frostleaf",        seedName = "Frostleaf Seed",        cropName = "Frostleaf",        order = 7 },
        { name = "Cherry Tree",      seedName = "Cherry Tree Seed",      cropName = "Cherry",           order = 8 },
        { name = "Banana Tree",      seedName = "Banana Tree Seed",      cropName = "Banana",           order = 9 },
        { name = "Dragonfruit Bush", seedName = "Dragonfruit Bush Seed", cropName = "Dragonfruit",      order = 10 },
        { name = "Starfruit Plant",  seedName = "Starfruit Plant Seed",  cropName = "Starfruit",        order = 11 },
        { name = "Coconut Palm",     seedName = "Coconut Palm Seed",     cropName = "Coconut",          order = 12 },
        { name = "Crystal Shrub",    seedName = "Crystal Shrub Seed",    cropName = "Crystal Shrub",    order = 13 },
        { name = "Nebula Vine",      seedName = "Nebula Vine Seed",      cropName = "Nebula Vine",      order = 14 },
        { name = "Celestial Tree",   seedName = "Celestial Tree Seed",   cropName = "Celestial Tree",   order = 15 },
    }

    -- 1. TOHUMLAR (SEEDS)
    for _, info in ipairs(canonList) do
        local icon = SEED_ICONS[info.name] or "🌱"
        local label = string.format("%s %s", icon, info.seedName)
        local btn = makeBtn(seedGridContainer, label, Color3.fromRGB(30, 42, 58), function()
            AdminActionEvent:FireServer("GiveSeed", info.seedName, 20)
            btn.Text = "✅ Alındı!"
            btn.BackgroundColor3 = Color3.fromRGB(40, 150, 75)
            task.delay(0.85, function()
                if btn and btn.Parent then
                    btn.Text = label
                    btn.BackgroundColor3 = Color3.fromRGB(30, 42, 58)
                end
            end)
        end)
        btn.LayoutOrder = info.order * 2 - 1
        btn.Name = "Seed_" .. info.name

        table.insert(itemDataList, {
            btn = btn,
            category = "SEED",
            searchKey = (info.seedName .. " " .. info.name .. " tohum seed"):lower():gsub("%s+", "")
        })
    end

    -- 2. BİTKİLER / HASAT EDİLMİŞ ÜRÜNLER (CROPS)
    for _, info in ipairs(canonList) do
        local icon = SEED_ICONS[info.name] or "🍎"
        local label = string.format("%s %s", icon, info.cropName)
        local btn = makeBtn(seedGridContainer, label, Color3.fromRGB(52, 34, 46), function()
            AdminActionEvent:FireServer("GiveCrop", info.cropName)
            btn.Text = "✅ Hasat Alındı!"
            btn.BackgroundColor3 = Color3.fromRGB(160, 60, 80)
            task.delay(0.85, function()
                if btn and btn.Parent then
                    btn.Text = label
                    btn.BackgroundColor3 = Color3.fromRGB(52, 34, 46)
                end
            end)
        end)
        btn.LayoutOrder = info.order * 2
        btn.Name = "Crop_" .. info.cropName

        table.insert(itemDataList, {
            btn = btn,
            category = "CROP",
            searchKey = (info.cropName .. " " .. info.name .. " urun meyve bitki hasat crop"):lower():gsub("%s+", "")
        })
    end

    filterButtons()
end

populateButtons()

seedSearchBox:GetPropertyChangedSignal("Text"):Connect(filterButtons)

-- =============================================================
-- BÖLÜM 4: IŞINLANMA (TELEPORT)
-- =============================================================
createSectionHeader("📍 IŞINLANMA (TELEPORT)", 30)

local tpGrid = Instance.new("Frame")
tpGrid.Size = UDim2.new(1, 0, 0, 120)
tpGrid.BackgroundTransparency = 1
tpGrid.LayoutOrder = 31
tpGrid.Parent = contentScroll

local tGrid = Instance.new("UIGridLayout")
tGrid.CellSize = UDim2.new(0, 125, 0, 32)
tGrid.CellPadding = UDim2.new(0, 8, 0, 8)
tGrid.Parent = tpGrid

makeBtn(tpGrid, "🏛️ Merkez (Shop)", Color3.fromRGB(80, 50, 120), function()
    AdminActionEvent:FireServer("Teleport", "Shop")
end)

for i = 1, 8 do
    local islNames = {
        "Island_East_1", "Island_East_2", "Island_West_1", "Island_West_2",
        "Island_South_1", "Island_South_2", "Island_North_1", "Island_North_2"
    }
    local islTarget = islNames[i] or ("Island_" .. i)
    makeBtn(tpGrid, "Ada " .. i, Color3.fromRGB(36, 42, 56), function()
        AdminActionEvent:FireServer("Teleport", islTarget)
    end)
end

-- -------------------------------------------------------------
-- Panel Açma / Kapatma Olayları & Klavye Kısayolları
-- -------------------------------------------------------------
local function togglePanel()
    mainFrame.Visible = not mainFrame.Visible
    if mainFrame.Visible then
        refreshBalanceDisplay()
    end
end

closeBtn.MouseButton1Click:Connect(togglePanel)

UserInputService.InputBegan:Connect(function(input, gameProcessed)
    if gameProcessed then return end
    if input.KeyCode == Enum.KeyCode.F2 or input.KeyCode == Enum.KeyCode.Quote then
        togglePanel()
    elseif input.KeyCode == Enum.KeyCode.F then
        toggleFlight()
        if flyBtn then
            if isFlying then
                flyBtn.Text = "🚀 Uçma Modu: AÇIK (Kısayol: F)"
                flyBtn.BackgroundColor3 = Color3.fromRGB(40, 150, 70)
            else
                flyBtn.Text = "✈️ Uçma Modu: KAPALI (Kısayol: F)"
                flyBtn.BackgroundColor3 = Color3.fromRGB(50, 60, 80)
            end
        end
    end
end)

print("[AdminPanelController] Developer test ve admin paneli yüklendi! (Kısayol: F2 veya ekranın sağ üstündeki ⚡ DEV PANEL)")
