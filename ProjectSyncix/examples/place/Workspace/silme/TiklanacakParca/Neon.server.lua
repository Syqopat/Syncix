local tiklanacakParca = script.Parent -- Veya butonun olduğu yol
local clickDetector = tiklanacakParca:WaitForChild("ClickDetector")
local donecekParca = game.Workspace.silme:WaitForChild("allah")

local donduruLuyor = false

local function materyalDegistir()
	donduruLuyor = not donduruLuyor

	if donduruLuyor then
		task.spawn(function()
			while donduruLuyor do
				-- Materyali Neon yap
				donecekParca.Material = Enum.Material.Neon
				task.wait(0.05) -- Hızını burayı küçülterek/büyülterek ayarlayabilirsiniz

				-- Eğer döngü devam ediyorsa Metal yap
				if donduruLuyor then
					donecekParca.Material = Enum.Material.Metal
					task.wait(0.1)
				end
			end

			-- Durdurulduğunda varsayılan materyale geri dönmek isterseniz:
			donecekParca.Material = Enum.Material.Metal
		end)
	end
end

clickDetector.MouseClick:Connect(materyalDegistir)