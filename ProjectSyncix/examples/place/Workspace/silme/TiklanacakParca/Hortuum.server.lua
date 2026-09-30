local TweenService = game:GetService("TweenService")

-- Buton tanımlaması
local button = script.Parent

-- YENİ YOL: workspace -> silme -> homtur
local silmeKlasoru = workspace:WaitForChild("silme")
local part = workspace.silme:WaitForChild("homtur")

-- Part'ın başlangıç (orijinal) pozisyonu ve boyutu
local originalCFrame = part.CFrame
local originalSize = part.Size

-- Hortum/Uzaklaşma Efekti Parametreleri (50 stud yukarı, 30 stud ileri ve 3 tam tur dönme)
local offsetPosition = Vector3.new(0, 50, -30) 
local targetCFrame = originalCFrame * CFrame.new(offsetPosition) * CFrame.Angles(0, math.rad(1080), 0)

-- Hareket Ayarları
local tweenInfo = TweenInfo.new(
	2.5, -- 2.5 saniyede gitsin/gelsin
	Enum.EasingStyle.Quad,
	Enum.EasingDirection.Out
)

local isTransformed = false
local isTweening = false

button.MouseButton1Click:Connect(function()
	if isTweening then return end
	isTweening = true

	if not isTransformed then
		-- 1. TIKLAMA: Döne döne uzaklaş
		local goal = {
			CFrame = targetCFrame,
			Size = originalSize * 0.2,
			Transparency = 0.5
		}

		local tween = TweenService:Create(part, tweenInfo, goal)
		tween:Play()

		tween.Completed:Connect(function()
			isTransformed = true
			isTweening = false
		end)

	else
		-- 2. TIKLAMA: Eski yerine dön
		local goal = {
			CFrame = originalCFrame,
			Size = originalSize,
			Transparency = 0
		}

		local tween = TweenService:Create(part, tweenInfo, goal)
		tween:Play()

		tween.Completed:Connect(function()
			isTransformed = false
			isTweening = false
		end)
	end
end)