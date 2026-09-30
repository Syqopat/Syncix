local door = script.Parent
local clickDetector = door:WaitForChild("ClickDetector")
local TweenService = game:GetService("TweenService")

local isOpen = false
local isTweening = false

-- Kapının kapalı (başlangıç) konumu
local closedCFrame = door.CFrame

-- Mesafeleri belirleyin
local upDistance = door.Size.Y * 1.2 -- Kapının yüksekliği kadar yukarı (X ekseni)
local forwardDistance = door.Size.Z * 1.5 -- Kapının kalınlığı/uzunluğu kadar ileri (Z ekseni)

-- Kapının hem yukarı hem de kendi baktığı yöne (ileri) doğru kayması
-- CFrame.new(X, Y, Z): Y yukarı, -Z genellikle Roblox'ta ileri kabul edilir
local openCFrame = closedCFrame * CFrame.new(0, upDistance, -forwardDistance)

local tweenInfo = TweenInfo.new(
	1, -- Açılma/Kapanma süresi (saniye)
	Enum.EasingStyle.Quad, -- Yumuşak hareket stili
	Enum.EasingDirection.Out
)

clickDetector.MouseClick:Connect(function()
	if isTweening then return end -- Hareket bitmeden tekrar tıklamayı engeller
	isTweening = true

	-- Hedef konumu belirle
	local targetCFrame = isOpen and closedCFrame or openCFrame
	local tween = TweenService:Create(door, tweenInfo, {CFrame = targetCFrame})

	tween:Play()

	-- Ses efekti çalmak istersen buraya ekleyebilirsin
	-- local sound = door:FindFirstChild("DoorSound")
	-- if sound then sound:Play() end

	tween.Completed:Wait() -- Hareket bitene kadar bekle

	isOpen = not isOpen
	isTweening = false
end)