-- ConnectionManager
-- HTTP Polling, Push, Timeout ve Exponential Backoff mantığını yöneten State Machine.
-- Durumlar: Disconnected, Discovering, Connecting, Connected, Reconnecting, Blocked
--
-- Release için buraya üç şey eklendi:
--   1. Port keşfi. Port artık sabit 8080 değil; core doluysa sıradakine geçiyor.
--      Eklenti dosya okuyamadığı için aralığı /health ile tarar.
--   2. Sürüm uyum kontrolü. Eski eklenti + fresh core sessizce garip davranıyordu.
--   3. İlk bağlantı onayı. Hangi projectInfo klasörünün bağlandığı kullanıcıya gösterilir.

local HttpService = game:GetService("HttpService")
local SyncConfig = require(script.Parent.Parent.Core.SyncConfig)
local PlaceIdentity = require(script.Parent.Parent.Core.PlaceIdentity)
local Store = require(script.Parent.Parent.Core.Store)
local Approval = require(script.Parent.Parent.Core.Approval)

-- Bu dort sabit core ile ESLESMEK ZORUNDA. Karsiliklari:
--   PLUGIN_VERSION  <-> core-engine/Cargo.toml  version
--   PLUGIN_PROTOCOL <-> project.rs  PROTOCOL_VERSION
--   PORT_START  <-> project.rs  DEFAULT_PORT
--   PORT_RANGE     <-> project.rs  PORT_SCAN_SPAN
local PLUGIN_VERSION = "0.1.1"
local PLUGIN_PROTOCOL = 1
local PORT_START = 8080
local PORT_RANGE = 10

local ConnectionManager = {}
ConnectionManager.__index = ConnectionManager

function ConnectionManager.new()
	local self = setmetatable({}, ConnectionManager)

	self.serverUrl = nil        -- keşiften sonra dolar
	self.serverInfo = nil       -- /health cevabı: project, root, port, version
	self.state = "Disconnected"

	-- Exponential Backoff için ayarlar
	self.baseRetryWait = 1.0
	self.maxRetryWait = 30.0
	self.currentRetryWait = 1.0

	-- Reddedilen köklerin tekrar tekrar sorulmaması için oturum içi hafıza
	self.rejected = {}

	-- Kullanici senkronu elle duraklatti mi?
	-- Duraklatma yalnizca kullanicinin isOpen istegiyle olur; aglar koptugunda
	-- kullanilan yol Reconnecting'dir, bu ayri bir durumdur.
	self.wasPaused = false

	-- Elle sabitlenmiş port. nil ise 8080-8089 aralığı taranır.
	-- Sabitlenmişse YALNIZCA o port denenir: iki projectInfo açıkken hangi projeye
	-- bağlanılacağını kesinleştirmenin tek yolu bu.
	self.manualPort = nil

	return self
end

function ConnectionManager:OnStart(container)
	self.retryQueue = container:Get("RetryQueue")
	self.metrics = container:Get("Metrics")
	self.commandDispatcher = container:Get("CommandDispatcher")
	self.patchBuilder = container:Get("PatchBuilder")
	self.batchQueue = container:Get("BatchQueue")
	self.activityLog = container:Get("ActivityLog")
	-- `plugin` global'i ModuleScript'lerde güvenilir değil; ana script'ten
	-- açıkça geçiriliyor. Onay penceresi ve ayar saklama buna bağlı.
	self.plugin = container:Get("Plugin").ref

	-- Baglanti izni kapisi.
	--
	-- Varsayilan KAPALI. Sebep: plugin:SetSetting yerel kurulan eklentilerde diske
	-- yazilmiyor (olculdu), dolayisiyla "hatirla" calismiyordu ve her Studio
	-- acilisinda pencere cikip senkronu bekletiyordu. Guvenlik degeri, engellemenin
	-- maliyetini karsilamiyordu.
	--
	-- Yerine: baglanti kurulunca hangi projectInfo klasorune baglanildigi Output'a ve
	-- panele YAZILIYOR. Kullanici neye baglandigini goruyor, ama flow durmuyor.
	-- Kapiyi geri acmak isteyen Syncix panelinden "Baglanti izni sor"u acabilir.
	self.askPermission = Store.Get(self.plugin, "syncix_ask_permission", false) == true

	local saved = Store.Get(self.plugin, "syncix_port", 0)
	if type(saved) == "number" and saved > 0 then
		self.manualPort = saved
		print(string.format("[Syncix] Saved port setting: %d (only this port will be tried)", saved))
	end

	self:Connect()
end

function ConnectionManager:SetState(newState: string)
	if self.state ~= newState then
		print(string.format("[Syncix] %s -> %s", self.state, newState))
		self.state = newState
	end
end

-- Tek bir portu yoklar. Cevap Syncix core'undan geliyorsa bilgiyi döndürür.
local function readHealth(port: number)
	local url = string.format("http://127.0.0.1:%d/health", port)
	local ok, response = pcall(function()
		return HttpService:RequestAsync({ Url = url, Method = "GET" })
	end)
	if not ok or not response or not response.Success then
		return nil
	end
	local decoded
	local okDecode = pcall(function()
		decoded = HttpService:JSONDecode(response.Body)
	end)
	if not okDecode or type(decoded) ~= "table" then
		return nil
	end
	-- Portta başka bir program olabilir; Syncix imzası aranır.
	if decoded.status == nil then
		return nil
	end
	decoded.port = decoded.port or port
	decoded.url = string.format("http://127.0.0.1:%d", port)
	return decoded
end

-- major.minor karşılaştırır; yama farkı sorun değildir.
local function versionCompatible(a: string?, b: string?): boolean
	if type(a) ~= "string" or type(b) ~= "string" then
		return false
	end
	local aMajor, aMinor = string.match(a, "^(%d+)%.(%d+)")
	local bMajor, bMinor = string.match(b, "^(%d+)%.(%d+)")
	if not aMajor or not bMajor then
		return false
	end
	return aMajor == bMajor and aMinor == bMinor
end

-- Elle port ayarlama (SettingsPanel'den çağrılır).
function ConnectionManager:SetManualPort(port: number?)
	self.manualPort = port
	-- Port değişince eski reddetmeler anlamını yitirir.
	self.rejected = {}
end

function ConnectionManager:GetManualPort(): number?
	return self.manualPort
end

-- Kullanıcı ayarı değiştirdiğinde beklemeden yeniden bağlan.
function ConnectionManager:ForceReconnect()
	self.serverUrl = nil
	self.serverInfo = nil
	self.currentRetryWait = self.baseRetryWait
	self:SetState("Disconnected")
	self:Connect()
end

-- Panelde göstermek için: aralıktaki tüm core'ları listeler (permission/sürüm süzgeci yok).
function ConnectionManager:ScanAllPorts()
	local foundList = {}
	for port = PORT_START, PORT_START + PORT_RANGE - 1 do
		local info = readHealth(port)
		if info then
			table.insert(foundList, info)
		end
	end
	-- Elle yazılan port aralık dışında olabilir; o da listelenmeli.
	if self.manualPort and (self.manualPort < PORT_START or self.manualPort >= PORT_START + PORT_RANGE) then
		local info = readHealth(self.manualPort)
		if info then
			table.insert(foundList, info)
		end
	end
	return foundList
end

function ConnectionManager:Discover()
	-- Port sabitlenmişse tarama yapılmaz; yalnızca o port denenir.
	local first, last
	if self.manualPort then
		first, last = self.manualPort, self.manualPort
	else
		first, last = PORT_START, PORT_START + PORT_RANGE - 1
	end

	for port = first, last do
		local info = readHealth(port)
		if info then
			local root = tostring(info.root or "")

			if self.rejected[root] then
				-- Bu oturumda zaten reddedildi, atla.
				continue
			end

			-- BASKA bir place'e bagli core'u atla.
			--
			-- Eklenti port tararken buldugu ILK saglikli core'a baglaniyordu.
			-- Iki projectInfo ayni anda acikken bu, yanlis projeye baglanmak demekti:
			-- core hemen place catismasi verip senkronu askiya aliyor ve
			-- kullanici "neden calismiyor" diye bakakaliyordu. Artik kendi
			-- place'imize bagli olan ya da hic baglanmamis (bos) klasoru
			-- seciyoruz.
			local myPlace = PlaceIdentity.Resolve()
			if info.bound_place ~= nil
				and myPlace ~= ""
				and tostring(info.bound_place) ~= myPlace
			then
				continue
			end

			-- Sürüm kapısı: uyumsuzsa bağlanma, sebebini açıkça söyle.
			if not versionCompatible(info.version, PLUGIN_VERSION) then
				warn(string.format(
					"[Syncix] Version mismatch. Plugin: %s, core: %s (port %d).\n" ..
					"  The same major.minor version is required. Update the VS Code extension and the Studio plugin.",
					PLUGIN_VERSION, tostring(info.version), port
				))
				continue
			end

			if info.protocol ~= nil and info.protocol ~= PLUGIN_PROTOCOL then
				warn(string.format(
					"[Syncix] Protocol mismatch. Plugin: %d, core: %s. An update is required.",
					PLUGIN_PROTOCOL, tostring(info.protocol)
				))
				continue
			end

			-- İzin kapısı yalnızca açıkça istenmişse çalışır (bkz. self.askPermission).
			local isAllowed = true
			-- Izin kapisi artik syncix.toml'dan geliyor; panel ayari yalnizca
			-- core'a hic baglanilamadigi durumda gecerli.
			if self.askPermission or SyncConfig.AskPermission() then
				isAllowed = Approval.GetStoredDecision(self.plugin, root)
			end

			if isAllowed == nil then
				print(string.format("[Syncix] A new project wants to connect: %s", root))
				local decision = Approval.Ask(self.plugin, info)

				if decision == "error" then
					-- Pencere açılamadı: decision verilmedi. Saklamıyoruz ve kara
					-- listeye almıyoruz ki bir arayüz aksaklığı senkronu
					-- kalıcı olarak kilitlemesin; sonraki denemede tekrar sorulur.
					continue
				end

				isAllowed = (decision == "allow")
				Approval.Store(self.plugin, root, isAllowed)
			end

			if not isAllowed then
				self.rejected[root] = true
				warn(string.format(
					"[Syncix] Connection refused: %s\n" ..
					"  To change your mind, reset the Studio plugin settings.",
					root
				))
				continue
			end

			return info
		end
	end

	if self.manualPort then
		warn(string.format(
			"[Syncix] No Syncix core found on port %d.\n" ..
			"  Start one there:  syncix serve %d\n" ..
			"  Or set the port back to Automatic in the Syncix panel.",
			self.manualPort, self.manualPort
		))
	end
	return nil
end

--- Senkronu elle duraklatir/devam ettirir.
---
--- Devam ederken TAM YENIDEN SENKRON yapilir: duraklatma sirasinda hem Studio
--- hem editor tarafinda degisiklik olmus olabilir ve hangisinin daha fresh
--- oldugunu bilmiyoruz. Studio'nun anlik goruntusunu yeniden gondermek iki
--- tarafi tek adimda ayni noktaya getirir.
function ConnectionManager:SetPaused(pause: boolean)
	if self.wasPaused == pause then
		return
	end
	self.wasPaused = pause

	if pause then
		self:SetState("Paused")
		print("[Syncix] Sync paused. Nothing is sent to or applied from the editor.")
	else
		print("[Syncix] Sync resumed. Re-syncing the full tree...")
		self.currentRetryWait = self.baseRetryWait
		self:SetState("Disconnected")
		self:Connect()
	end
end

function ConnectionManager:IsPaused(): boolean
	return self.wasPaused
end

function ConnectionManager:Connect()
	if self.wasPaused then return end
	if self.state == "Connected" then return end

	self:SetState("Discovering")

	task.spawn(function()
		local info = self:Discover()
		if not info then
			self:HandleDisconnect()
			return
		end

		self.serverUrl = info.url
		self.serverInfo = info

		-- syncix.toml'daki ayarlar core uzerinden geliyor. Eklentinin bunlari
		-- kendi icinde saklamamasi bilincli: iki ayri ayar seti olsaydi hangisinin
		-- gecerli oldugu belirsizlesirdi. Tek dogruluk kaynagi syncix.toml.
		-- Klasor baska bir place'e bagliysa kullanici bunu Studio'da gormeli;
		-- terminale bakmiyor olabilir ve sessiz kalirsa "neden senkron olmuyor"
		-- sorusunun cevabi hicbir yerde yazmaz.
		if info.place_conflict then
			warn(string.format(
				"[Syncix] This folder belongs to a DIFFERENT place. Sync is on hold so nothing gets mixed.\n" ..
				"  folder is bound to : %s\n" ..
				"  this place         : %s (%s)\n" ..
				"  Decide in a terminal:\n" ..
				"    syncix bind --studio   this place is right, rewrite the folder from it\n" ..
				"    syncix bind --disk     the folder is right, load it into this place",
				tostring(info.place_conflict.folder_place),
				tostring(info.place_conflict.incoming_place),
				tostring(info.place_conflict.incoming_name)
			))
		end

		SyncConfig.Apply(info.config)
		if info.config then
			print(string.format(
				"[Syncix] Settings from syncix.toml — mode: %s, play: %s, undo: %s",
				tostring(info.config.mode),
				tostring(info.config.play_mode),
				tostring(info.config.undo)
			))
		end

		self:SetState("Connected")
		self.currentRetryWait = self.baseRetryWait

		print(string.format(
			"[Syncix] Connected: %s (folder: %s, port %d, core %s)",
			tostring(info.project), tostring(info.root), info.port, tostring(info.version)
		))

		-- Eski gönderilemeyen paketleri yolla
		if self.retryQueue and self.retryQueue:HasPending() then
			local pending = self.retryQueue:Flush()
			for _, payload in ipairs(pending) do
				self:Send(payload)
			end
		end

		-- Bootstrap / FULL_SYNC (Sıfırdan Senkronizasyon)
		if self.patchBuilder then
			local fullSyncPatch = self.patchBuilder:BuildFullTreeSnapshot()
			self:Send(fullSyncPatch)
			print("[Syncix] Bootstrap FULL_SYNC sent.")
		end

		self:StartPolling()
		self:StartMetricsReporting()
	end)
end

function ConnectionManager:PingServer(): (boolean, number)
	if not self.serverUrl then
		return false, 0
	end
	local start = os.clock()
	local success, _ = pcall(function()
		return HttpService:RequestAsync({
			Url = self.serverUrl .. "/health",
			Method = "GET"
		})
	end)
	local latencyMs = math.floor((os.clock() - start) * 1000)

	if success and self.metrics then
		self.metrics:RecordLatency(latencyMs)
	end

	return success, latencyMs
end

function ConnectionManager:HandleDisconnect()
	-- Kullanici duraklatmissa yeniden baglanma dongusu calismamali.
	if self.wasPaused then
		return
	end
	self:SetState("Reconnecting")

	warn(string.format("[Syncix] No connection. Retrying in %d second(s).", self.currentRetryWait))
	task.wait(self.currentRetryWait)

	self.currentRetryWait = math.min(self.currentRetryWait * 2, self.maxRetryWait)

	self:Connect()
end

function ConnectionManager:Send(payload: any)
	-- Duraklatilmisken hicbir sey gonderilmez. Kuyruga da alinmaz: duraklatma
	-- bittiginde tam yeniden senkron yapiliyor, birikmis eski paketleri sonradan
	-- gondermek o senkronu bozardi.
	if self.wasPaused then
		return
	end
	if not self.serverUrl then
		if self.retryQueue then self.retryQueue:EnqueueFailed(payload) end
		return
	end

	local json = HttpService:JSONEncode(payload)
	local url = self.serverUrl .. "/sync/push"

	task.spawn(function()
		local success, _ = pcall(function()
			HttpService:PostAsync(url, json, Enum.HttpContentType.ApplicationJson)
		end)

		if success then
			if self.metrics then self.metrics:IncrementSuccessfulRequests() end
		else
			warn("[Syncix] Could not send packet, queued for retry.")
			if self.metrics then self.metrics:IncrementFailedRequests() end
			if self.retryQueue then self.retryQueue:EnqueueFailed(payload) end

			if self.state == "Connected" then
				self:HandleDisconnect()
			end
		end
	end)
end

-- Long-Polling döngüsü
function ConnectionManager:StartPolling()
	task.spawn(function()
		while self.state == "Connected" do
			local success, response = pcall(function()
				return HttpService:RequestAsync({
					Url = self.serverUrl .. "/sync/poll",
					Method = "GET"
				})
			end)

			if success and response.Success then
				local body = response.Body
				if body and body ~= "null" then
					local payload = HttpService:JSONDecode(body)
					if payload and self.commandDispatcher then
						self.commandDispatcher:Dispatch(payload)
					end
				end
			else
				self:HandleDisconnect()
				break
			end

			task.wait(0.1)
		end
	end)
end

-- BatchQueue sayaçlarını düzenli olarak core'a bildirir.
-- Birleştirmenin gerçekten çalışıp çalışmadığı ancak böyle ölçülebilir; eskiden
-- bunu doğrulamanın tek yolu Studio'da elle object sürüklemekti.
function ConnectionManager:StartMetricsReporting()
	task.spawn(function()
		while self.state == "Connected" do
			task.wait(5)
			if self.state ~= "Connected" or not self.batchQueue then
				break
			end
			local counter = self.batchQueue:GetStats()
			-- Akis ozeti de bildirilir: eklenti bellegindeki gunlugu disaridan
			-- gorebilmenin tek yolu bu. `syncix status` bu sayilari gosterir,
			-- ozellikle conflict sayisi sessiz veri kaybinin erken uyarisidir.
			local flow = self.activityLog and self.activityLog:Summary() or nil
			self:Send({
				event_type = "PLUGIN_METRICS",
				version = "v1",
				data = {
					queued = counter.queued,
					coalesced = counter.coalesced,
					plugin_version = PLUGIN_VERSION,
					activity_total = flow and flow.total or 0,
					activity_in = flow and flow.incoming or 0,
					activity_out = flow and flow.outgoing or 0,
					conflicts = flow and flow.conflict or 0,
				},
			})
		end
	end)
end

return ConnectionManager
