local buton = script.Parent
local clickDetector = buton:WaitForChild("ClickDetector")
local RunService = game:GetService("RunService")

-- Workspace içindeki Hortum objesini bul
local hortum = workspace.silme:WaitForChild("homtur")

local isRotating = false
local rotateConnection = nil

-- Dönme hızı (Değeri artırırsan daha hızlı döner)
local rotateSpeed = 999999999999

clickDetector.MouseClick:Connect(function()
	if not isRotating then
		-- DÖNMEYİ BAŞLAT
		isRotating = true

		-- Her karede (frame) hortumu biraz döndür
		rotateConnection = RunService.Heartbeat:Connect(function(deltaTime)
			if isRotating and hortum then
				-- Y ekseninde kendi etrafında döndürme
				hortum.CFrame = hortum.CFrame * CFrame.Angles(0, math.rad(rotateSpeed), 0)
			end
		end)
	else
		-- DÖNMEYİ DURDUR
		isRotating = false
		if rotateConnection then
			rotateConnection:Disconnect()
			rotateConnection = nil
		end
	end
end)