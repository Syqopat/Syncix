--[[
	COIN SIMULATOR — server
	Written from the editor, synced into Studio by Syncix.

	Game loop:
	  1. Collect coins in a zone  -> they go into your backpack
	  2. Backpack full? Walk to the sell area -> coins become money
	  3. Spend money on upgrades  -> bigger backpack / faster pickup
	  4. Unlock Zone 2 (500 money) -> coins worth 5x
--]]

local Players = game:GetService("Players")
local RunService = game:GetService("RunService")

local simulator = workspace:WaitForChild("Simulator")
local zones = simulator:WaitForChild("Zones")
local upgrades = simulator:WaitForChild("Upgrades")
local sellArea = simulator:WaitForChild("SellArea")

-- ============ TUNING ============
local CONFIG = {
	startingCapacity = 10,
	capacityStep = 10,
	capacityPriceScale = 1.6,
	capacityBasePrice = 50,

	startingSpeed = 1,
	speedStep = 1,
	speedPriceScale = 2.0,
	speedBasePrice = 100,

	zone2Price = 500,

	zones = {
		{ floorName = "Zone1Floor", coinCount = 25, value = 1, color = Color3.fromRGB(255, 209, 0), locked = false },
		{ floorName = "Zone2Floor", coinCount = 25, value = 5, color = Color3.fromRGB(120, 220, 255), locked = true },
	},
}

-- ============ PAD SIGNS ============
-- Every pad gets a floating sign saying what it does.
local function createSign(part, title, subtitle, color)
	if not part then return nil end

	local existing = part:FindFirstChild("SyncixSign")
	if existing then existing:Destroy() end

	local billboard = Instance.new("BillboardGui")
	billboard.Name = "SyncixSign"
	billboard.Size = UDim2.new(0, 220, 0, 70)
	billboard.StudsOffset = Vector3.new(0, 5, 0)
	billboard.AlwaysOnTop = true
	billboard.Parent = part

	local heading = Instance.new("TextLabel")
	heading.Size = UDim2.new(1, 0, 0.55, 0)
	heading.BackgroundTransparency = 1
	heading.TextColor3 = color
	heading.TextStrokeTransparency = 0.3
	heading.TextScaled = true
	heading.Font = Enum.Font.GothamBold
	heading.Text = title
	heading.Parent = billboard

	local caption = Instance.new("TextLabel")
	caption.Name = "Caption"
	caption.Size = UDim2.new(1, 0, 0.45, 0)
	caption.Position = UDim2.new(0, 0, 0.55, 0)
	caption.BackgroundTransparency = 1
	caption.TextColor3 = Color3.fromRGB(235, 235, 235)
	caption.TextStrokeTransparency = 0.4
	caption.TextScaled = true
	caption.Font = Enum.Font.Gotham
	caption.Text = subtitle
	caption.Parent = billboard

	return caption
end

-- ============ PLAYER STATE ============
local playerState = {}

local function stateOf(player)
	return playerState[player.UserId]
end

Players.PlayerAdded:Connect(function(player)
	playerState[player.UserId] = {
		capacity = CONFIG.startingCapacity,
		pickupSpeed = CONFIG.startingSpeed,
		capacityPrice = CONFIG.capacityBasePrice,
		speedPrice = CONFIG.speedBasePrice,
		zone2Unlocked = false,
		lastPickup = 0,
	}

	local stats = Instance.new("Folder")
	stats.Name = "leaderstats"
	stats.Parent = player

	local money = Instance.new("IntValue")
	money.Name = "Money"
	money.Value = 0
	money.Parent = stats

	local bag = Instance.new("IntValue")
	bag.Name = "Bag"
	bag.Value = 0
	bag.Parent = stats
end)

Players.PlayerRemoving:Connect(function(player)
	playerState[player.UserId] = nil
end)

-- ============ COINS ============
local function createCoin(container, floor, zone)
	local coin = Instance.new("Part")
	coin.Name = "Coin"
	-- A Cylinder's axis in Roblox is X: thickness on X, diameter on Y/Z.
	-- (2,2,0.4) gives an elliptical barrel; a coin needs (0.4,2,2).
	coin.Size = Vector3.new(0.4, 2, 2)
	coin.Shape = Enum.PartType.Cylinder
	coin.Material = Enum.Material.Neon
	coin.Color = zone.color
	coin.Anchored = true
	coin.CanCollide = false

	-- Random spot on the floor
	local span = floor.Size.X / 2 - 4
	local x = floor.Position.X + math.random(-span, span)
	local z = floor.Position.Z + math.random(-span, span)
	coin.CFrame = CFrame.new(x, floor.Position.Y + 3, z)

	local glow = Instance.new("PointLight")
	glow.Brightness = 3
	glow.Range = 8
	glow.Color = zone.color
	glow.Parent = coin

	coin:SetAttribute("Value", zone.value)
	coin.Parent = container
	return coin
end

local function collectCoin(player, coin, zone)
	local state = stateOf(player)
	if not state then return end

	local stats = player:FindFirstChild("leaderstats")
	if not stats then return end

	local bag = stats:FindFirstChild("Bag")
	if not bag then return end

	-- Full backpack: nothing to do until the player sells.
	if bag.Value >= state.capacity then
		return
	end

	-- Pickup rate limit (at most 'pickupSpeed' pickups per second).
	local now = tick()
	if now - state.lastPickup < (1 / (state.pickupSpeed * 4)) then
		return
	end
	state.lastPickup = now

	local value = coin:GetAttribute("Value") or 1
	bag.Value = math.min(bag.Value + value, state.capacity)

	-- Hide the coin, then bring it back somewhere else.
	coin.Transparency = 1
	local glow = coin:FindFirstChildWhichIsA("Light")
	if glow then glow.Enabled = false end

	task.delay(3, function()
		if coin and coin.Parent then
			local floor = zones:FindFirstChild(zone.floorName)
			if floor then
				local span = floor.Size.X / 2 - 4
				local x = floor.Position.X + math.random(-span, span)
				local z = floor.Position.Z + math.random(-span, span)
				coin.CFrame = CFrame.new(x, floor.Position.Y + 3, z)
			end
			coin.Transparency = 0
			if glow then glow.Enabled = true end
		end
	end)
end

-- Build the zones
local coinContainers = {}

for _, zone in ipairs(CONFIG.zones) do
	local floor = zones:FindFirstChild(zone.floorName)
	if floor then
		local container = Instance.new("Folder")
		container.Name = zone.floorName .. "_Coins"
		container.Parent = zones

		for _ = 1, zone.coinCount do
			local coin = createCoin(container, floor, zone)
			coin.Touched:Connect(function(hit)
				local player = Players:GetPlayerFromCharacter(hit.Parent)
				if not player then return end
				if coin.Transparency > 0 then return end

				-- Locked zone: only players who paid may collect here.
				if zone.locked then
					local state = stateOf(player)
					if not state or not state.zone2Unlocked then
						return
					end
				end

				collectCoin(player, coin, zone)
			end)
		end

		coinContainers[zone.floorName] = container
	end
end

-- Spin every coin from a single loop rather than one loop per coin.
RunService.Heartbeat:Connect(function(dt)
	for _, container in pairs(coinContainers) do
		for _, coin in ipairs(container:GetChildren()) do
			if coin:IsA("BasePart") and coin.Transparency < 1 then
				-- Spinning on Y makes the coin read as "on display".
				coin.CFrame = coin.CFrame * CFrame.Angles(0, math.rad(120 * dt), 0)
			end
		end
	end
end)

-- ============ SELL AREA ============
local sellCooldown = {}

sellArea.Touched:Connect(function(hit)
	local player = Players:GetPlayerFromCharacter(hit.Parent)
	if not player then return end

	local now = tick()
	if sellCooldown[player.UserId] and now - sellCooldown[player.UserId] < 0.5 then
		return
	end
	sellCooldown[player.UserId] = now

	local stats = player:FindFirstChild("leaderstats")
	if not stats then return end

	local bag = stats:FindFirstChild("Bag")
	local money = stats:FindFirstChild("Money")
	if not bag or not money or bag.Value <= 0 then return end

	money.Value += bag.Value
	bag.Value = 0
end)

-- ============ UPGRADES ============
local upgradeCooldown = {}

local function setUpUpgrade(pad, kind)
	if not pad then return end

	pad.Touched:Connect(function(hit)
		local player = Players:GetPlayerFromCharacter(hit.Parent)
		if not player then return end

		local now = tick()
		if upgradeCooldown[player.UserId] and now - upgradeCooldown[player.UserId] < 1 then
			return
		end
		upgradeCooldown[player.UserId] = now

		local state = stateOf(player)
		local stats = player:FindFirstChild("leaderstats")
		if not state or not stats then return end

		local money = stats:FindFirstChild("Money")
		if not money then return end

		local caption = pad:FindFirstChild("SyncixSign")
		caption = caption and caption:FindFirstChild("Caption")

		if kind == "capacity" then
			if money.Value >= state.capacityPrice then
				money.Value -= state.capacityPrice
				state.capacity += CONFIG.capacityStep
				state.capacityPrice = math.floor(state.capacityPrice * CONFIG.capacityPriceScale)
				if caption then
					caption.Text = "Bag: " .. state.capacity .. " | Price: " .. state.capacityPrice
				end
			elseif caption then
				caption.Text = "Not enough money (" .. state.capacityPrice .. ")"
			end
		elseif kind == "speed" then
			if money.Value >= state.speedPrice then
				money.Value -= state.speedPrice
				state.pickupSpeed += CONFIG.speedStep
				state.speedPrice = math.floor(state.speedPrice * CONFIG.speedPriceScale)
				if caption then
					caption.Text = "Speed: " .. state.pickupSpeed .. " | Price: " .. state.speedPrice
				end
			elseif caption then
				caption.Text = "Not enough money (" .. state.speedPrice .. ")"
			end
		end
	end)
end

setUpUpgrade(upgrades:FindFirstChild("BackpackUpgrade"), "capacity")
setUpUpgrade(upgrades:FindFirstChild("SpeedUpgrade"), "speed")

-- Signs
createSign(sellArea, "SELL AREA", "Turn your coins into money", Color3.fromRGB(46, 204, 113))
createSign(
	upgrades:FindFirstChild("BackpackUpgrade"),
	"BAG +" .. CONFIG.capacityStep,
	"Price: " .. CONFIG.capacityBasePrice,
	Color3.fromRGB(230, 126, 34)
)
createSign(
	upgrades:FindFirstChild("SpeedUpgrade"),
	"SPEED +" .. CONFIG.speedStep,
	"Price: " .. CONFIG.speedBasePrice,
	Color3.fromRGB(155, 89, 182)
)
createSign(
	zones:FindFirstChild("Zone2Floor"),
	"ZONE 2",
	CONFIG.zone2Price .. " money (5x value)",
	Color3.fromRGB(120, 220, 255)
)

-- ============ ZONE 2 UNLOCK ============
local zone2Floor = zones:FindFirstChild("Zone2Floor")
if zone2Floor then
	zone2Floor.Touched:Connect(function(hit)
		local player = Players:GetPlayerFromCharacter(hit.Parent)
		if not player then return end

		local state = stateOf(player)
		local stats = player:FindFirstChild("leaderstats")
		if not state or not stats or state.zone2Unlocked then return end

		local money = stats:FindFirstChild("Money")
		if money and money.Value >= CONFIG.zone2Price then
			money.Value -= CONFIG.zone2Price
			state.zone2Unlocked = true
		end
	end)
end

print("[Simulator] Ready. Zones: " .. #CONFIG.zones)
