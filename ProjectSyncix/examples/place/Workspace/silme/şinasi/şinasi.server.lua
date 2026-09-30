local part = script.Parent
local clickDetector = part:WaitForChild("ClickDetector")
local RunService = game:GetService("RunService")
local TweenService = game:GetService("TweenService")

local isMoving = false
local originalPosition = part.Position
local moveConnection = nil

local moveSpeed = 12 -- İleri gitme hızı
local jumpHeight = 3 -- Zıplama yüksekliği (Kaç birim yukarı zıplayacağı)
local jumpSpeed = 10 -- Zıplama hızı/sıklığı

local currentForwardDistance = 0

clickDetector.MouseClick:Connect(function()
	if not isMoving then
		isMoving = true
		currentForwardDistance = 0

		if moveConnection then moveConnection:Disconnect() end

		moveConnection = RunService.Heartbeat:Connect(function(deltaTime)
			if isMoving then
				-- İleri gitme mesafesini artır
				currentForwardDistance = currentForwardDistance + (moveSpeed * deltaTime)

				-- İleri yönlü CFrame hesabı (-LookVector bakılan yönün tersiyse kullanılır)
				local forwardOffset = -part.CFrame.LookVector * currentForwardDistance

				-- Yerin altına girmeden ve göğe uçmadan SADECE zıplama hesabı
				local bounceHeight = math.abs(math.sin(tick() * jumpSpeed)) * jumpHeight

				-- Yeni konumu tam olarak ayarla: Başlangıç + İleri Gitme + Zıplama
				part.Position = originalPosition + forwardOffset + Vector3.new(0, bounceHeight, 0)
			end
		end)
	else
		-- Durdur ve başlangıç konumuna geri döndür
		isMoving = false
		if moveConnection then
			moveConnection:Disconnect()
			moveConnection = nil
		end

		local tweenInfo = TweenInfo.new(1, Enum.EasingStyle.Quad, Enum.EasingDirection.Out)
		local tween = TweenService:Create(part, tweenInfo, {Position = originalPosition})
		tween:Play()
	end
end)