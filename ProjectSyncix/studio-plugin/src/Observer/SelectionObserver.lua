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

local SyncConfig = require(script.Parent.Parent.Core.SyncConfig)

local SelectionObserver = {}
SelectionObserver.__index = SelectionObserver

function SelectionObserver.new()
	local self = setmetatable({}, SelectionObserver)
	-- Editorden incoming secimi applying Studio yine SelectionChanged atiyor.
	-- Bu bayrak olmasa o sinyal editore geri gonderilir ve iki taraf birbirini
	-- surekli tetiklerdi.
	self.applying = false
	return self
end

function SelectionObserver:OnStart(container)
	self.cache = container:Get("RuntimeCache")
	self.connectionManager = container:Get("ConnectionManager")

	Selection.SelectionChanged:Connect(function()
		self:SendFromStudio()
	end)
end

--- Studio'da secilenleri editore bildirir.
function SelectionObserver:SendFromStudio()
	if self.applying then return end
	if RunService:IsRunning() then return end
	if not SyncConfig.SendFromStudio() then return end
	if not self.connectionManager then return end

	local identities = {}
	for _, object in ipairs(Selection:Get()) do
		-- UUID'si olmayan object senkron disi (ornegin Camera); atlanir.
		local uuid = object:GetAttribute("__syncix_id")
		if uuid then
			table.insert(identities, tostring(uuid))
		end
	end

	self.connectionManager:Send({
		event_type = "SELECTION",
		version = "v1",
		data = { ids = identities, source = "studio" },
	})
end

--- Editorden incoming secimi Studio'da uygular.
function SelectionObserver:Apply(identities: { string })
	if not self.cache then return end

	local objects = {}
	for _, uuid in ipairs(identities or {}) do
		local object = self.cache:GetInstance(uuid)
		-- Silinmis ya da hic gelmemis bir identity sessizce atlanir: yarim bir
		-- secim, hic secim yapmamaktan iyi.
		if object and object.Parent then
			table.insert(objects, object)
		end
	end

	self.applying = true
	pcall(function()
		Selection:Set(objects)
	end)
	-- SelectionChanged deferred gelebiliyor; bayragi bir kare sonra birakiyoruz
	-- ki kendi yazdigimiz secimi geri gondermeyelim.
	task.defer(function()
		self.applying = false
	end)
end

return SelectionObserver
