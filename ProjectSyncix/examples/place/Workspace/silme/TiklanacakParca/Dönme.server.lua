local tiklanacakParca = script.Parent
local clickDetector = tiklanacakParca:WaitForChild("ClickDetector")
local donecekParca = game.Workspace.silme:WaitForChild("allah")

local donduruLuyor = false -- Dönüş durumunu takip eden değişken

local function tiklamaAksiyonu()
	-- Tıklandığında durumu tersine çevir (True ise False, False ise True yap)
	donduruLuyor = not donduruLuyor

	-- Eğer dönme başlatıldıysa döngüyü çalıştır
	if donduruLuyor then
		task.spawn(function()
			while donduruLuyor do
				task.wait() -- wait() yerine Roblox'ta güncel standart olan task.wait() önerilir
				donecekParca.CFrame = donecekParca.CFrame * CFrame.fromEulerAnglesXYZ(0.02, 0.02, -0.07)
			end
		end)
	end
end

clickDetector.MouseClick:Connect(tiklamaAksiyonu)