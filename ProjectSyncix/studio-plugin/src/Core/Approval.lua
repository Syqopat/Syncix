-- Approval
-- İlk bağlantıda kullanıcı onayı (trust on first use).
--
-- Neden var: core, 127.0.0.1 üzerinde identity doğrulaması olmadan Studio'yu sürebiliyordu.
-- Makinedeki herhangi bir program 8080'e bir sunucu açıp Studio'daki objeleri
-- değiştirebilirdi. Şifre/token girmek yerine daha basit ve daha anlaşılır bir yol
-- seçildi: Studio, hangi PROJE KLASÖRÜNÜN bağlanmak istediğini gösterip bir kez permission ister.
-- Onaylanan klasörler saklanır, bir daha sorulmaz.

local HttpService = game:GetService("HttpService")

local Approval = {}

local SETTING_PREFIX = "syncix_approval_"

-- İKİ KATMANLI KALICILIK
--
-- 1. plugin:SetSetting — Creator Store'dan kurulmuş plugin'lerde çalışır.
--    Ama .rbxm dosyası doğrudan Plugins klasörüne bırakıldığında (bizim dağıtım
--    biçimimiz) plugin'in kayıtlı bir kimliği olmadığı için AYARLAR DİSKE YAZILMIYOR.
--    Ölçüldü: Studio yeniden başlatıldığında permission sıfırlanıyor, InstalledPlugins
--    klasörü hiç oluşmuyor.
--
-- 2. game niteliği — yedek olarak decision place'in kendisine yazılır. `game` nesnesi
--    gözlemcinin izlediği servislerin dışında olduğu için senkrona sızmaz ve
--    dosyalarda görünmez. Place kaydedildiğinde kalıcı olur.
--
-- Okuma ikisini de dener, yazma ikisine de yazar.
local GAME_ATTR = "__syncix_approved_roots"

local function keyName(root: string): string
	return SETTING_PREFIX .. string.lower(root)
end

local function readPlaceRecords(): { [string]: boolean }
	local ok, raw = pcall(function()
		return game:GetAttribute(GAME_ATTR)
	end)
	if not ok or type(raw) ~= "string" or raw == "" then
		return {}
	end
	local okDecode, tbl = pcall(function()
		return HttpService:JSONDecode(raw)
	end)
	if okDecode and type(tbl) == "table" then
		return tbl
	end
	return {}
end

local function writePlaceRecords(entries)
	pcall(function()
		game:SetAttribute(GAME_ATTR, HttpService:JSONEncode(entries))
	end)
end

-- Daha önce verilmiş decision: true (permission), false (red), nil (hiç sorulmadı)
function Approval.GetStoredDecision(pluginRef, root: string)
	if not root or root == "" then
		return nil
	end
	local k = keyName(root)

	if pluginRef then
		local ok, datum = pcall(function()
			return pluginRef:GetSetting(k)
		end)
		if ok and type(datum) == "boolean" then
			return datum
		end
	end

	local entries = readPlaceRecords()
	local datum = entries[k]
	if type(datum) == "boolean" then
		return datum
	end

	return nil
end

function Approval.Store(pluginRef, root: string, permission: boolean)
	if not root or root == "" then
		return
	end
	local k = keyName(root)

	if pluginRef then
		pcall(function()
			pluginRef:SetSetting(k, permission)
		end)
	end

	local entries = readPlaceRecords()
	entries[k] = permission
	writePlaceRecords(entries)

	-- Geri okuma denetimi: hiçbir katman kalıcı olmadıysa kullanıcı bunu bilmeli,
	-- yoksa her Studio açılışında tekrar sorulmasını failure sanır.
	if Approval.GetStoredDecision(pluginRef, root) == nil then
		warn(
			"[Syncix] The decision could not be stored permanently; you will be asked on every Studio start.\n" ..
			"  Save the place (Ctrl+S) to store the decision with it."
		)
	end
end

-- Pencere kimliği her çağrıda benzersiz olmalı: aynı kimlikle ikinci kez
-- CreateDockWidgetPluginGui çağırmak failure verir.
local windowCounter = 0

-- Onay penceresini gösterir ve kullanıcı decision verene kadar bekler.
--
-- Dönüş üç durumludur ve bu ÖNEMLİ:
--   "allow"  kullanıcı onayladı
--   "deny"   kullanıcı reddetti (kalıcı olarak saklanır)
--   "error"  pencere açılamadı  -> KARAR DEĞİLDİR, saklanmaz, sonra tekrar denenir
-- Aksi halde arayüzdeki tek bir aksaklık senkronu kalıcı olarak kilitlerdi.
function Approval.Ask(pluginRef, info): string
	local projectInfo = tostring(info.project or "Unknown project")
	local root = tostring(info.root or "")
	local port = tostring(info.port or "?")

	windowCounter += 1

	local ok, result = pcall(function()
		local widgetInfo = DockWidgetPluginGuiInfo.new(
			Enum.InitialDockState.Float,
			true,  -- başlangıçta açık
			true,  -- kullanıcı geçmişini ezme
			460, 210,
			460, 210
		)
		local gui = pluginRef:CreateDockWidgetPluginGui(
			"SyncixBaglantiOnayi_" .. tostring(windowCounter),
			widgetInfo
		)
		gui.Title = "Syncix - Connection permission"

		local frame = Instance.new("Frame")
		frame.Size = UDim2.new(1, 0, 1, 0)
		frame.BackgroundColor3 = Color3.fromRGB(30, 33, 40)
		frame.BorderSizePixel = 0
		frame.Parent = gui

		local title = Instance.new("TextLabel")
		title.Size = UDim2.new(1, -24, 0, 28)
		title.Position = UDim2.new(0, 12, 0, 12)
		title.BackgroundTransparency = 1
		title.TextColor3 = Color3.fromRGB(255, 255, 255)
		title.TextXAlignment = Enum.TextXAlignment.Left
		title.Font = Enum.Font.GothamBold
		title.TextSize = 16
		title.Text = "This project wants to modify Studio"
		title.Parent = frame

		local detail = Instance.new("TextLabel")
		detail.Size = UDim2.new(1, -24, 0, 92)
		detail.Position = UDim2.new(0, 12, 0, 44)
		detail.BackgroundTransparency = 1
		detail.TextColor3 = Color3.fromRGB(210, 214, 222)
		detail.TextXAlignment = Enum.TextXAlignment.Left
		detail.TextYAlignment = Enum.TextYAlignment.Top
		detail.TextWrapped = true
		detail.Font = Enum.Font.Gotham
		detail.TextSize = 13
		detail.Text = string.format(
			"Project: %s\nFolder: %s\nPort: %s\n\nIf you do not recognise this folder, deny it. If you allow it, this project can create, modify and delete instances in the Explorer.",
			projectInfo, root, port
		)
		detail.Parent = frame

		local function button(text, x, color)
			local b = Instance.new("TextButton")
			b.Size = UDim2.new(0, 200, 0, 34)
			b.Position = UDim2.new(0, x, 1, -46)
			b.BackgroundColor3 = color
			b.BorderSizePixel = 0
			b.TextColor3 = Color3.fromRGB(255, 255, 255)
			b.Font = Enum.Font.GothamBold
			b.TextSize = 14
			b.Text = text
			b.Parent = frame
			local corner = Instance.new("UICorner")
			corner.CornerRadius = UDim.new(0, 6)
			corner.Parent = b
			return b
		end

		local permissionButton = button("Allow", 12, Color3.fromRGB(46, 160, 87))
		local denyButton = button("Deny", 236, Color3.fromRGB(180, 60, 60))

		-- Pencere Studio tarafından kapalı durumda geri yüklenmiş olabilir; açık olduğundan emin ol.
		gui.Enabled = true

		local decision = nil
		local wasClosed = false
		local openedAt = os.clock()

		permissionButton.Activated:Connect(function() decision = true end)
		denyButton.Activated:Connect(function() decision = false end)

		-- Pencerenin kapatılması REDDETME SAYILMAZ.
		--
		-- Eskiden sayılıyordu ve şu hataya yol açtı: DockWidgetPluginGui oluşturulurken
		-- Enabled bir an false oluyor, bu da kullanıcı hiçbir şeye basmadan "reddedildi"
		-- olarak yorumlanıp bağlantıyı kalıcı olarak engelliyordu.
		-- Artık kapatma "kararsız" demektir: saklanmaz, bir sonraki denemede tekrar sorulur.
		gui:GetPropertyChangedSignal("Enabled"):Connect(function()
			-- İlk saniye Studio'nun kendi pencere durumu geri yüklemesine ayrılmıştır.
			if not gui.Enabled and decision == nil and (os.clock() - openedAt) > 1 then
				wasClosed = true
			end
		end)

		-- En fazla 2 dakika bekle; kullanıcı orada değilse bağlantı denemesi
		-- sonsuza kadar askıda kalmasın.
		while decision == nil and not wasClosed and (os.clock() - openedAt) < 120 do
			task.wait(0.1)
		end

		gui.Enabled = false
		gui:Destroy()

		if decision == nil then
			return nil -- decision verilmedi
		end
		return decision
	end)

	if not ok then
		-- Pencere açılamadı. Bu bir RED DEĞİLDİR: kalıcı olarak saklanmaz,
		-- bağlantı bir sonraki denemede yeniden sorulur.
		warn(
			"[Syncix] Could not open the approval window: " .. tostring(result) ..
			"\n  Not connected; will retry shortly."
		)
		return "error"
	end

	if result == true then
		return "allow"
	end
	if result == false then
		return "deny"
	end

	-- nil: pencere kapatıldı ya da timestamp aşımına uğradı. Karar verilmedi,
	-- saklanmaz ve kara listeye alınmaz; bir sonraki denemede tekrar sorulur.
	print("[Syncix] Permission not granted (window closed). You will be asked again later.")
	return "error"
end

return Approval
