local textLabel = script.Parent
local screenGui = script:FindFirstAncestorWhichIsA("ScreenGui")
local referenceResolution = Vector2.new(1920, 1080)
local referenceTextSize = 30

local function updateTextSize()
	local currentSize = screenGui.AbsoluteSize
	local scaleFactor = math.min(currentSize.X / referenceResolution.X, currentSize.Y / referenceResolution.Y)
	local newTextSize = math.max(9, referenceTextSize * scaleFactor)
	textLabel.TextSize = newTextSize
end 

screenGui:GetPropertyChangedSignal("AbsoluteSize"):Connect(updateTextSize) 

updateTextSize()