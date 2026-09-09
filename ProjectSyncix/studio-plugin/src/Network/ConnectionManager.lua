-- ConnectionManager
-- HTTP Polling, Push, Timeout ve Exponential Backoff mantığını yöneten State Machine.
-- Durumlar: Disconnected, Discovering, Connecting, Connected, Reconnecting, Blocked
--
-- Release için buraya üç şey eklendi:
--   1. Port keşfi. Port artık sabit 8080 değil; core doluysa sıradakine geçiyor.
--      Eklenti dosya okuyamadığı için aralığı /health ile tarar.
--   2. Sürüm uyum kontrolü. Eski eklenti + yeni core sessizce garip davranıyordu.
--   3. İlk bağlantı onayı. Hangi proje klasörünün bağlandığı kullanıcıya gösterilir.

local HttpService = game:GetService("HttpService")
local Ayarlar = require(script.Parent.Parent.Core.Ayarlar)
local PlaceKimligi = require(script.Parent.Parent.Core.PlaceKimligi)

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
	self.reddedilen = {}

	-- Kullanici senkronu elle duraklatti mi?
	-- Duraklatma yalnizca kullanicinin acik istegiyle olur; aglar koptugunda
	-- kullanilan yol Reconnecting'dir, bu ayri bir durumdur.
	self.duraklatildi = false

	-- Elle sabitlenmiş port. nil ise 8080-8089 aralığı taranır.
	-- Sabitlenmişse YALNIZCA o port denenir: iki proje açıkken hangi projeye
	-- bağlanılacağını kesinleştirmenin tek yolu bu.
	self.manuelPort = nil

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
	-- Yerine: baglanti kurulunca hangi proje klasorune baglanildigi Output'a ve
	-- panele YAZILIYOR. Kullanici neye baglandigini goruyor, ama akis durmuyor.
	-- Kapiyi geri acmak isteyen Syncix panelinden "Baglanti izni sor"u acabilir.
	self.izinSor = Store.Get(self.plugin, "syncix_izin_sor", false) == true

	local kayitli = Store.Get(self.plugin, "syncix_port", 0)
	if type(kayitli) == "number" and kayitli > 0 then
		self.manuelPort = kayitli
		print(string.format("[Syncix] Saved port setting: %d (only this port will be tried)", kayitli))
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
local function healthOku(port: number)
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
local function surumUyumlu(a: string?, b: string?): boolean
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
	self.manuelPort = port
	-- Port değişince eski reddetmeler anlamını yitirir.
	self.reddedilen = {}
end

function ConnectionManager:GetManualPort(): number?
	return self.manuelPort
end

-- Kullanıcı ayarı değiştirdiğinde beklemeden yeniden bağlan.
function ConnectionManager:ForceReconnect()
	self.serverUrl = nil
	self.serverInfo = nil
	self.currentRetryWait = self.baseRetryWait
	self:SetState("Disconnected")
	self:Connect()
end

-- Panelde göstermek için: aralıktaki tüm core'ları listeler (izin/sürüm süzgeci yok).
function ConnectionManager:TaraTumPortlar()
	local bulunanlar = {}
	for port = PORT_BASLANGIC, PORT_BASLANGIC + PORT_ARALIK - 1 do
		local bilgi = healthOku(port)
		if bilgi then
			table.insert(bulunanlar, bilgi)
		end
	end
	-- Elle yazılan port aralık dışında olabilir; o da listelenmeli.
	if self.manuelPort and (self.manuelPort < PORT_BASLANGIC or self.manuelPort >= PORT_BASLANGIC + PORT_ARALIK) then
		local bilgi = healthOku(self.manuelPort)
		if bilgi then
			table.insert(bulunanlar, bilgi)
		end
	end
	return bulunanlar
end

function ConnectionManager:Discover()
	-- Port sabitlenmişse tarama yapılmaz; yalnızca o port denenir.
	local ilk, son
	if self.manuelPort then
		ilk, son = self.manuelPort, self.manuelPort
	else
		ilk, son = PORT_BASLANGIC, PORT_BASLANGIC + PORT_ARALIK - 1
	end

	for port = ilk, son do
		local bilgi = healthOku(port)
		if bilgi then
			local root = tostring(bilgi.root or "")

			if self.reddedilen[root] then
				-- Bu oturumda zaten reddedildi, atla.
				continue
			end

			-- BASKA bir place'e bagli core'u atla.
			--
			-- Eklenti port tararken buldugu ILK saglikli core'a baglaniyordu.
			-- Iki proje ayni anda acikken bu, yanlis projeye baglanmak demekti:
			-- core hemen place catismasi verip senkronu askiya aliyor ve
			-- kullanici "neden calismiyor" diye bakakaliyordu. Artik kendi
			-- place'imize bagli olan ya da hic baglanmamis (bos) klasoru
			-- seciyoruz.
			local benimPlace = PlaceKimligi.Al()
			if bilgi.bound_place ~= nil
				and benimPlace ~= ""
				and tostring(bilgi.bound_place) ~= benimPlace
			then
				continue
			end

			-- Sürüm kapısı: uyumsuzsa bağlanma, sebebini açıkça söyle.
			if not surumUyumlu(bilgi.version, PLUGIN_VERSION) then
				warn(string.format(
					"[Syncix] Version mismatch. Plugin: %s, core: %s (port %d).\n" ..
					"  The same major.minor version is required. Update the VS Code extension and the Studio plugin.",
					PLUGIN_VERSION, tostring(bilgi.version), port
				))
				continue
			end

			if bilgi.protocol ~= nil and bilgi.protocol ~= PLUGIN_PROTOCOL then
				warn(string.format(
					"[Syncix] Protocol mismatch. Plugin: %d, core: %s. An update is required.",
					PLUGIN_PROTOCOL, tostring(bilgi.protocol)
				))
				continue
			end

			-- İzin kapısı yalnızca açıkça istenmişse çalışır (bkz. self.izinSor).
			local izinli = true
			-- Izin kapisi artik syncix.toml'dan geliyor; panel ayari yalnizca
			-- core'a hic baglanilamadigi durumda gecerli.
			if self.izinSor or Ayarlar.IzinSor() then
				izinli = Approval.GetStoredDecision(self.plugin, root)
			end

			if izinli == nil then
				print(string.format("[Syncix] A new project wants to connect: %s", root))
				local karar = Approval.Ask(self.plugin, bilgi)

				if karar == "error" then
					-- Pencere açılamadı: karar verilmedi. Saklamıyoruz ve kara
					-- listeye almıyoruz ki bir arayüz aksaklığı senkronu
					-- kalıcı olarak kilitlemesin; sonraki denemede tekrar sorulur.
					continue
				end

				izinli = (karar == "allow")
				Approval.Store(self.plugin, root, izinli)
			end

			if not izinli then
				self.reddedilen[root] = true
				warn(string.format(
					"[Syncix] Connection refused: %s\n" ..
					"  To change your mind, reset the Studio plugin settings.",
					root
				))
				continue
			end

			return bilgi
		end
	end

	if self.manuelPort then
		warn(string.format(
			"[Syncix] No Syncix core found on port %d.\n" ..
			"  Start one there:  syncix serve %d\n" ..
			"  Or set the port back to Automatic in the Syncix panel.",
			self.manuelPort, self.manuelPort
		))
	end
	return nil
end

--- Senkronu elle duraklatir/devam ettirir.
---
--- Devam ederken TAM YENIDEN SENKRON yapilir: duraklatma sirasinda hem Studio
--- hem editor tarafinda degisiklik olmus olabilir ve hangisinin daha yeni
--- oldugunu bilmiyoruz. Studio'nun anlik goruntusunu yeniden gondermek iki
--- tarafi tek adimda ayni noktaya getirir.
function ConnectionManager:SetPaused(duraklat: boolean)
	if self.duraklatildi == duraklat then
		return
	end
	self.duraklatildi = duraklat

	if duraklat then
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
	return self.duraklatildi
end

function ConnectionManager:Connect()
	if self.duraklatildi then return end
	if self.state == "Connected" then return end

	self:SetState("Discovering")

	task.spawn(function()
		local bilgi = self:Discover()
		if not bilgi then
			self:HandleDisconnect()
			return
		end

		self.serverUrl = bilgi.url
		self.serverInfo = bilgi

		-- syncix.toml'daki ayarlar core uzerinden geliyor. Eklentinin bunlari
		-- kendi icinde saklamamasi bilincli: iki ayri ayar seti olsaydi hangisinin
		-- gecerli oldugu belirsizlesirdi. Tek dogruluk kaynagi syncix.toml.
		-- Klasor baska bir place'e bagliysa kullanici bunu Studio'da gormeli;
		-- terminale bakmiyor olabilir ve sessiz kalirsa "neden senkron olmuyor"
		-- sorusunun cevabi hicbir yerde yazmaz.
		if bilgi.place_conflict then
			warn(string.format(
				"[Syncix] This folder belongs to a DIFFERENT place. Sync is on hold so nothing gets mixed.\n" ..
				"  folder is bound to : %s\n" ..
				"  this place         : %s (%s)\n" ..
				"  Decide in a terminal:\n" ..
				"    syncix bind --studio   this place is right, rewrite the folder from it\n" ..
				"    syncix bind --disk     the folder is right, load it into this place",
				tostring(bilgi.place_conflict.klasorun_place),
				tostring(bilgi.place_conflict.gelen_place),
				tostring(bilgi.place_conflict.gelen_ad)
			))
		end

		Ayarlar.Uygula(bilgi.config)
		if bilgi.config then
			print(string.format(
				"[Syncix] Settings from syncix.toml — mode: %s, play: %s, undo: %s",
				tostring(bilgi.config.mode),
				tostring(bilgi.config.play_mode),
				tostring(bilgi.config.undo)
			))
		end

		self:SetState("Connected")
		self.currentRetryWait = self.baseRetryWait

		print(string.format(
			"[Syncix] Connected: %s (folder: %s, port %d, core %s)",
			tostring(bilgi.project), tostring(bilgi.root), bilgi.port, tostring(bilgi.version)
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
	if self.duraklatildi then
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
	if self.duraklatildi then
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
-- bunu doğrulamanın tek yolu Studio'da elle obje sürüklemekti.
function ConnectionManager:StartMetricsReporting()
	task.spawn(function()
		while self.state == "Connected" do
			task.wait(5)
			if self.state ~= "Connected" or not self.batchQueue then
				break
			end
			local sayac = self.batchQueue:GetStats()
			-- Akis ozeti de bildirilir: eklenti bellegindeki gunlugu disaridan
			-- gorebilmenin tek yolu bu. `syncix status` bu sayilari gosterir,
			-- ozellikle cakisma sayisi sessiz veri kaybinin erken uyarisidir.
			local akis = self.activityLog and self.activityLog:Ozet() or nil
			self:Send({
				event_type = "PLUGIN_METRICS",
				version = "v1",
				data = {
					queued = sayac.queued,
					coalesced = sayac.coalesced,
					plugin_version = PLUGIN_VERSION,
					activity_total = akis and akis.toplam or 0,
					activity_in = akis and akis.gelen or 0,
					activity_out = akis and akis.giden or 0,
					conflicts = akis and akis.cakisma or 0,
				},
			})
		end
	end)
end

return ConnectionManager
