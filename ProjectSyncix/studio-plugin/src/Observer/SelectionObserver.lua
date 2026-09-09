--!strict
-- SelectionObserver
--
-- Studio'daki secimi editore, editordeki secimi Studio'ya tasir.
--
-- Neden ayri bir gozlemci: secim projenin ICERIGI degil, anlik bir durum.
-- Diger gozlemciler gibi model'e ve diske yazilmamali. Her tiklamada bir dosya
-- degisseydi surum kontrolu gurultuye bogulurdu. Bu yuzden kendi kanalindan,
-- model'e hic dokunmadan gidiyor.

local Selection = game:GetService("Selection")
local RunService = game:GetService("RunService")

local Ayarlar = require(script.Parent.Parent.Core.Ayarlar)

local SelectionObserver = {}
SelectionObserver.__index = SelectionObserver

function SelectionObserver.new()
	local self = setmetatable({}, SelectionObserver)
	-- Editorden gelen secimi uygularken Studio yine SelectionChanged atiyor.
	-- Bu bayrak olmasa o sinyal editore geri gonderilir ve iki taraf birbirini
	-- surekli tetiklerdi.
	self.uygularken = false
	return self
end

function SelectionObserver:OnStart(container)
	self.cache = container:Get("RuntimeCache")
	self.connectionManager = container:Get("ConnectionManager")

	Selection.SelectionChanged:Connect(function()
		self:StudiodanGonder()
	end)
end

--- Studio'da secilenleri editore bildirir.
function SelectionObserver:StudiodanGonder()
	if self.uygularken then return end
	if RunService:IsRunning() then return end
	if not Ayarlar.StudiodanGonder() then return end
	if not self.connectionManager then return end

	local kimlikler = {}
	for _, obje in ipairs(Selection:Get()) do
		-- UUID'si olmayan obje senkron disi (ornegin Camera); atlanir.
		local uuid = obje:GetAttribute("__syncix_id")
		if uuid then
			table.insert(kimlikler, tostring(uuid))
		end
	end

	self.connectionManager:Send({
		event_type = "SELECTION",
		version = "v1",
		data = { ids = kimlikler, source = "studio" },
	})
end

--- Editorden gelen secimi Studio'da uygular.
function SelectionObserver:Uygula(kimlikler: { string })
	if not self.cache then return end

	local objeler = {}
	for _, uuid in ipairs(kimlikler or {}) do
		local obje = self.cache:GetInstance(uuid)
		-- Silinmis ya da hic gelmemis bir kimlik sessizce atlanir: yarim bir
		-- secim, hic secim yapmamaktan iyi.
		if obje and obje.Parent then
			table.insert(objeler, obje)
		end
	end

	self.uygularken = true
	pcall(function()
		Selection:Set(objeler)
	end)
	-- SelectionChanged deferred gelebiliyor; bayragi bir kare sonra birakiyoruz
	-- ki kendi yazdigimiz secimi geri gondermeyelim.
	task.defer(function()
		self.uygularken = false
	end)
end

return SelectionObserver
