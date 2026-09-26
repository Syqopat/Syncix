--[[
	COIN SIMULATOR — player HUD
	Written from the editor, synced into Studio by Syncix.
	Shows money, backpack fill and a one-line hint.
--]]

local Players = game:GetService("Players")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local LocalizationService = game:GetService("LocalizationService")

-- ============ LOCALISATION ============
-- Strings come from the GameStrings table in ReplicatedStorage, which the
-- editor sees as GameStrings.csv — a file Excel or Sheets can open.
-- Adding a language means adding a column; the code does not change.
local strings = {}
do
	local table_ = ReplicatedStorage:FindFirstChild("GameStrings")
	local locale = "en"
	pcall(function()
		locale = string.sub(LocalizationService.RobloxLocaleId, 1, 2)
	end)

	if table_ and table_:IsA("LocalizationTable") then
		for _, entry in ipairs(table_:GetEntries()) do
			-- Fall back to the source text when the player's language is
			-- missing, so a gap in the table shows readable English rather
			-- than an empty label.
			strings[entry.Key] = entry.Values[locale] or entry.Values["en"] or entry.Source
		end
	end
end

local function T(key, fallback)
	return strings[key] or fallback
end

local player = Players.LocalPlayer
local stats = player:WaitForChild("leaderstats")
local money = stats:WaitForChild("Money")
local bag = stats:WaitForChild("Bag")

-- ============ INTERFACE ============
local gui = Instance.new("ScreenGui")
gui.Name = "SimulatorHud"
gui.ResetOnSpawn = false
gui.Parent = player:WaitForChild("PlayerGui")

local panel = Instance.new("Frame")
panel.Name = "Panel"
panel.Size = UDim2.new(0, 260, 0, 96)
panel.Position = UDim2.new(0, 16, 0, 16)
panel.BackgroundColor3 = Color3.fromRGB(20, 24, 32)
panel.BackgroundTransparency = 0.15
panel.BorderSizePixel = 0
panel.Parent = gui

local panelCorner = Instance.new("UICorner")
panelCorner.CornerRadius = UDim.new(0, 10)
panelCorner.Parent = panel

-- Money row
local moneyLabel = Instance.new("TextLabel")
moneyLabel.Name = "Money"
moneyLabel.Size = UDim2.new(1, -20, 0, 28)
moneyLabel.Position = UDim2.new(0, 10, 0, 8)
moneyLabel.BackgroundTransparency = 1
moneyLabel.TextColor3 = Color3.fromRGB(255, 209, 0)
moneyLabel.TextScaled = true
moneyLabel.Font = Enum.Font.GothamBold
moneyLabel.TextXAlignment = Enum.TextXAlignment.Left
moneyLabel.Text = T("money", "Money") .. ": 0"
moneyLabel.Parent = panel

-- Backpack row
local bagLabel = Instance.new("TextLabel")
bagLabel.Name = "Bag"
bagLabel.Size = UDim2.new(1, -20, 0, 22)
bagLabel.Position = UDim2.new(0, 10, 0, 40)
bagLabel.BackgroundTransparency = 1
bagLabel.TextColor3 = Color3.fromRGB(235, 235, 235)
bagLabel.TextScaled = true
bagLabel.Font = Enum.Font.Gotham
bagLabel.TextXAlignment = Enum.TextXAlignment.Left
bagLabel.Text = T("bag", "Bag") .. ": 0"
bagLabel.Parent = panel

-- Fill bar
local barTrack = Instance.new("Frame")
barTrack.Name = "BarTrack"
barTrack.Size = UDim2.new(1, -20, 0, 12)
barTrack.Position = UDim2.new(0, 10, 0, 68)
barTrack.BackgroundColor3 = Color3.fromRGB(45, 50, 60)
barTrack.BorderSizePixel = 0
barTrack.Parent = panel

local trackCorner = Instance.new("UICorner")
trackCorner.CornerRadius = UDim.new(0, 6)
trackCorner.Parent = barTrack

local barFill = Instance.new("Frame")
barFill.Name = "BarFill"
barFill.Size = UDim2.new(0, 0, 1, 0)
barFill.BackgroundColor3 = Color3.fromRGB(46, 204, 113)
barFill.BorderSizePixel = 0
barFill.Parent = barTrack

local fillCorner = Instance.new("UICorner")
fillCorner.CornerRadius = UDim.new(0, 6)
fillCorner.Parent = barFill

-- Hint line
local hint = Instance.new("TextLabel")
hint.Name = "Hint"
hint.Size = UDim2.new(0, 320, 0, 26)
hint.Position = UDim2.new(0, 16, 0, 120)
hint.BackgroundTransparency = 1
hint.TextColor3 = Color3.fromRGB(255, 255, 255)
hint.TextStrokeTransparency = 0.4
hint.TextScaled = true
hint.Font = Enum.Font.GothamMedium
hint.TextXAlignment = Enum.TextXAlignment.Left
hint.Text = T("start", "Collect coins, then sell them on the green pad!")
hint.Parent = gui

-- ============ UPDATE ============
-- The client is not told the capacity; the highest value seen so far is a
-- good enough reference for a progress bar.
local estimatedCapacity = 10

local function update()
	moneyLabel.Text = T("money", "Money") .. ": " .. money.Value

	if bag.Value > estimatedCapacity then
		estimatedCapacity = bag.Value
	end

	bagLabel.Text = T("bag", "Bag") .. ": " .. bag.Value .. " / " .. estimatedCapacity

	local ratio = 0
	if estimatedCapacity > 0 then
		ratio = math.clamp(bag.Value / estimatedCapacity, 0, 1)
	end
	barFill.Size = UDim2.new(ratio, 0, 1, 0)

	if ratio >= 1 then
		barFill.BackgroundColor3 = Color3.fromRGB(231, 76, 60)
		hint.Text = T("bagFull", "Bag full! Go to the green pad and sell.")
	elseif bag.Value > 0 then
		barFill.BackgroundColor3 = Color3.fromRGB(46, 204, 113)
		hint.Text = T("collecting", "Collecting coins... sell them on the green pad.")
	else
		barFill.BackgroundColor3 = Color3.fromRGB(46, 204, 113)
		hint.Text = T("start", "Collect coins, then sell them on the green pad!")
	end
end

money.Changed:Connect(update)
bag.Changed:Connect(update)
update()

-- ============ WELCOME BANNER ============
-- The text lives in the WelcomeMessage StringValue in ReplicatedStorage, which
-- the editor sees as WelcomeMessage.txt — plain text, no code involved.
task.spawn(function()
	local message = ReplicatedStorage:FindFirstChild("WelcomeMessage")
	if not message or not message:IsA("StringValue") then
		return
	end

	local banner = Instance.new("TextLabel")
	banner.Name = "Welcome"
	banner.Size = UDim2.new(0, 520, 0, 40)
	banner.Position = UDim2.new(0.5, -260, 0, 24)
	banner.BackgroundColor3 = Color3.fromRGB(20, 24, 32)
	banner.BackgroundTransparency = 0.2
	banner.BorderSizePixel = 0
	banner.TextColor3 = Color3.fromRGB(255, 209, 0)
	banner.TextScaled = true
	banner.Font = Enum.Font.GothamBold
	banner.Text = message.Value
	banner.Parent = gui

	local bannerCorner = Instance.new("UICorner")
	bannerCorner.CornerRadius = UDim.new(0, 8)
	bannerCorner.Parent = banner

	-- Edit the text in the editor and the banner follows immediately.
	message:GetPropertyChangedSignal("Value"):Connect(function()
		banner.Text = message.Value
	end)

	task.wait(6)
	for i = 0, 20 do
		banner.BackgroundTransparency = 0.2 + (i / 25)
		banner.TextTransparency = i / 20
		task.wait(0.05)
	end
	banner:Destroy()
end)
