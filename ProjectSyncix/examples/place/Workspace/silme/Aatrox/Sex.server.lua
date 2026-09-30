local donecekParca = script.Parent
local baslangicCFrame = donecekParca.CFrame

local zaman = 0
local hiz = 0.2 -- İleri-geri gitme hızı
local mesafe = 1

while true do
	task.wait()
	zaman = zaman + hiz

	-- math.sin kullanarak ileri-geri (Z ekseni) hareket hesaplanır
	local ileriGeri = math.sin(zaman) * mesafe

	donecekParca.CFrame = baslangicCFrame * CFrame.new(0, 0, ileriGeri)
end