local tiklanacakParca = script.Parent
local clickDetector = tiklanacakParca:WaitForChild("ClickDetector")
local donecekParca = game.Workspace.silme:WaitForChild("allah")

local donduruLuyor = false

local function renkDegistir()
	donduruLuyor = not donduruLuyor

	if donduruLuyor then
		task.spawn(function()
			while donduruLuyor do
				-- Hızlı hızlı rastgele renkler üretir
				donecekParca.Color = Color3.fromRGB(math.random(0, 255), math.random(0, 255), math.random(0, 255))
				task.wait(0.1) -- Renk değişim hızı (küçülttükçe hızlanır)
			end

			-- Durdurulduğunda rengi #FFFF00 (Sarı) yapar
			donecekParca.Color = Color3.fromHex("#FFFF00")
		end)
	else
		-- Eğer döngü kapandıysa rengi garantiye almak için doğrudan ayarlar
		donecekParca.Color = Color3.fromHex("#FFFF00")
	end
end

clickDetector.MouseClick:Connect(renkDegistir)