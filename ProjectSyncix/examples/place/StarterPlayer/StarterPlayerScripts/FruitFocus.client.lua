-- FruitFocus: the fruit you are about to pick is the one that lights up.
--
-- The server tags every ripe fruit "HarvestFruit" and gives it a prompt with Roblox's
-- own interface switched off. This picks the nearest tagged fruit, draws a thin black
-- outline around it and floats one word above it. Only that fruit's prompt is enabled,
-- so pressing E takes the apple you are looking at, not whichever one the engine found
-- first — and the word appears over the fruit instead of over the trunk.
--
-- It lives on the player, not in the plants: there are dozens of plants and one player,
-- and this is about what this player is looking at.

local CollectionService = game:GetService("CollectionService")
local Players = game:GetService("Players")
local RunService = game:GetService("RunService")

local TAG = "HarvestFruit"
local REACH = 14          -- studs: how far a fruit can be and still be pickable
local RECHECK = 0.1       -- seconds between scans; fruit does not move fast

local player = Players.LocalPlayer

-- The outline: no fill, one thin black line. A filled highlight washed the fruit out.
local highlight = Instance.new("Highlight")
highlight.Name = "FruitHighlight"
highlight.FillTransparency = 1
highlight.OutlineColor = Color3.fromRGB(0, 0, 0)
highlight.OutlineTransparency = 0
highlight.DepthMode = Enum.HighlightDepthMode.AlwaysOnTop
highlight.Enabled = false
highlight.Parent = workspace

-- The word, floating over the fruit itself.
local billboard = Instance.new("BillboardGui")
billboard.Name = "FruitCollectLabel"
billboard.Size = UDim2.fromOffset(118, 30)
billboard.StudsOffsetWorldSpace = Vector3.new(0, 1.1, 0)
billboard.AlwaysOnTop = true
billboard.LightInfluence = 0
billboard.MaxDistance = REACH + 6
billboard.Enabled = false
billboard.Parent = workspace

-- The key sits in its own little box, the way a keycap looks, and the word next to it.
local keyBox = Instance.new("Frame")
keyBox.Size = UDim2.fromOffset(26, 26)
keyBox.Position = UDim2.fromOffset(0, 2)
keyBox.BackgroundColor3 = Color3.fromRGB(16, 16, 18)
keyBox.BorderSizePixel = 0
keyBox.Parent = billboard

local keyCorner = Instance.new("UICorner")
keyCorner.CornerRadius = UDim.new(0, 6)
keyCorner.Parent = keyBox

local keyStroke = Instance.new("UIStroke")
keyStroke.Color = Color3.fromRGB(255, 255, 255)
keyStroke.Thickness = 1.5
keyStroke.Transparency = 0.3
keyStroke.Parent = keyBox

local keyLabel = Instance.new("TextLabel")
keyLabel.Size = UDim2.fromScale(1, 1)
keyLabel.BackgroundTransparency = 1
keyLabel.Text = "E"
keyLabel.TextColor3 = Color3.fromRGB(255, 255, 255)
keyLabel.TextScaled = true
keyLabel.Font = Enum.Font.GothamBold
keyLabel.Parent = keyBox

local keyPadding = Instance.new("UIPadding")
keyPadding.PaddingTop = UDim.new(0, 5)
keyPadding.PaddingBottom = UDim.new(0, 5)
keyPadding.Parent = keyLabel

-- One word. No plant name, no weight: the player is deciding whether to pick, not
-- reading a label.
local label = Instance.new("TextLabel")
label.Size = UDim2.fromOffset(84, 26)
label.Position = UDim2.fromOffset(34, 2)
label.BackgroundTransparency = 1
label.Text = "Collect"
label.TextColor3 = Color3.fromRGB(255, 255, 255)
label.TextStrokeColor3 = Color3.fromRGB(0, 0, 0)
label.TextStrokeTransparency = 0
label.TextXAlignment = Enum.TextXAlignment.Left
label.TextScaled = true
label.Font = Enum.Font.GothamBold
label.Parent = billboard

local constraint = Instance.new("UITextSizeConstraint")
constraint.MaxTextSize = 18
constraint.Parent = label

local focused = nil

local function promptOf(fruit)
	return fruit and fruit:FindFirstChild("CollectPrompt", true)
end

local function anchorOf(fruit)
	return fruit.PrimaryPart or fruit:FindFirstChildWhichIsA("BasePart", true)
end

local function setFocus(fruit)
	if focused == fruit then return end

	local leaving = promptOf(focused)
	if leaving then
		leaving.Enabled = false
	end

	focused = fruit

	if not fruit then
		highlight.Enabled = false
		highlight.Adornee = nil
		billboard.Enabled = false
		billboard.Adornee = nil
		return
	end

	local prompt = promptOf(fruit)
	if prompt then
		prompt.Enabled = true
		-- The prompt carries "Apple · 2.3 kg"; the weight belongs on the label too,
		-- because that is what the player is deciding on.
	end

	highlight.Adornee = fruit
	highlight.Enabled = true

	local anchor = anchorOf(fruit)
	if anchor and anchor.Transparency < 0.9 then
		billboard.Adornee = anchor
		billboard.Enabled = true
	else
		billboard.Enabled = false
		billboard.Adornee = nil
	end
end

-- Every tagged fruit starts silent; only the focused one speaks.
local function silence(fruit)
	local prompt = promptOf(fruit)
	if prompt then
		prompt.Enabled = false
	end
end

for _, fruit in ipairs(CollectionService:GetTagged(TAG)) do
	silence(fruit)
end
CollectionService:GetInstanceAddedSignal(TAG):Connect(silence)
CollectionService:GetInstanceRemovedSignal(TAG):Connect(function(fruit)
	if focused == fruit then
		setFocus(nil)
	end
end)

local clock = 0

RunService.Heartbeat:Connect(function(dt)
	clock += dt
	if clock < RECHECK then return end
	clock = 0

	local character = player.Character
	local root = character and character:FindFirstChild("HumanoidRootPart")
	if not root then
		setFocus(nil)
		return
	end

	local best, bestDistance = nil, REACH
	for _, fruit in ipairs(CollectionService:GetTagged(TAG)) do
		if fruit.Parent and fruit:IsA("Model") then
			local anchor = anchorOf(fruit)
			-- Toplanmış (gizlenmiş) bir meyve hâlâ sahnede durur; ona ne çizgi ne de
			-- yazı gider.
			if anchor and anchor.Transparency < 0.9 then
				local distance = (anchor.Position - root.Position).Magnitude
				if distance < bestDistance then
					best, bestDistance = fruit, distance
				end
			end
		end
	end

	setFocus(best)
end)
