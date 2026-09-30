local tiklanacakParca = script.Parent
local clickDetector = tiklanacakParca:WaitForChild("ClickDetector")
local donecekParca = game.Workspace:WaitForChild("allah")

local donduruLuyor = false

local function saydamlikDegistir()
	donduruLuyor = not donduruLuyor

	if donduruLuyor then
		task.spawn(function()
			local saydamlik = 0
			local artiyor = true

			while donduruLuyor do
				-- Şeffaflığı yavaşça artır ve azalt
				if artiyor then
					saydamlik = saydamlik + 0.06
					if saydamlik >= 0.6 then
						artiyor = false
					end
				else
					saydamlik = saydamlik - 0.06
					if saydamlik <= 0 then
						artiyor = true
					end
				end

				donecekParca.Transparency = saydamlik
				task.wait(0.01) -- Hızı ayarlamak için süreyi değiştirebilirsiniz
			end

			-- Durdurulduğunda parçayı tamamen görünür yap
			donecekParca.Transparency = 0
		end)
	else
		-- Durdurulduğunda şeffaflığı sıfırla (tam opak yap)
		donecekParca.Transparency = 0
	end
end

clickDetector.MouseClick:Connect(saydamlikDegistir)