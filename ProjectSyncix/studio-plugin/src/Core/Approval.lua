-- Approval
-- İlk bağlantıda kullanıcı onayı (trust on first use).
--
-- Neden var: core, 127.0.0.1 üzerinde kimlik doğrulaması olmadan Studio'yu sürebiliyordu.
-- Makinedeki herhangi bir program 8080'e bir sunucu açıp Studio'daki objeleri
-- değiştirebilirdi. Şifre/token girmek yerine daha basit ve daha anlaşılır bir yol
-- seçildi: Studio, hangi PROJE KLASÖRÜNÜN bağlanmak istediğini gösterip bir kez izin ister.
-- Onaylanan klasörler saklanır, bir daha sorulmaz.

local HttpService = game:GetService("HttpService")

local Approval = {}

local SETTING_PREFIX = "syncix_onay_"

-- İKİ KATMANLI KALICILIK
--
-- 1. plugin:SetSetting — Creator Store'dan kurulmuş plugin'lerde çalışır.
--    Ama .rbxm dosyası doğrudan Plugins klasörüne bırakıldığında (bizim dağıtım
--    biçimimiz) plugin'in kayıtlı bir kimliği olmadığı için AYARLAR DİSKE YAZILMIYOR.
--    Ölçüldü: Studio yeniden başlatıldığında izin sıfırlanıyor, InstalledPlugins
--    klasörü hiç oluşmuyor.
--
-- 2. game niteliği — yedek olarak karar place'in kendisine yazılır. `game` nesnesi
--    gözlemcinin izlediği servislerin dışında olduğu için senkrona sızmaz ve
--    dosyalarda görünmez. Place kaydedildiğinde kalıcı olur.
--
-- Okuma ikisini de dener, yazma ikisine de yazar.
local GAME_ATTR = "__syncix_onaylanan_kokler"

local function anahtar(root: string): string
	return SETTING_PREFIX .. string.lower(root)
end

local function placeKayitlariniOku(): { [string]: boolean }
	local ok, ham = pcall(function()
		return game:GetAttribute(GAME_ATTR)
	end)
	if not ok or type(ham) ~= "string" or ham == "" then
		return {}
	end
	local okDecode, tablo = pcall(function()
		return HttpService:JSONDecode(ham)
	end)
	if okDecode and type(tablo) == "table" then
		return tablo
	end
	return {}
end

local function placeKayitlariniYaz(kayitlar)
	pcall(function()
		game:SetAttribute(GAME_ATTR, HttpService:JSONEncode(kayitlar))
	end)
end

-- Daha önce verilmiş karar: true (izin), false (red), nil (hiç sorulmadı)
function Approval.GetStoredDecision(pluginRef, root: string)
	if not root or root == "" then
		return nil
	end
	local k = anahtar(root)

	if pluginRef then
		local ok, deger = pcall(function()
			return pluginRef:GetSetting(k)
		end)
		if ok and type(deger) == "boolean" then
			return deger
		end
	end

	local kayitlar = placeKayitlariniOku()
	local deger = kayitlar[k]
	if type(deger) == "boolean" then
		return deger
	end

	return nil
end

function Approval.Store(pluginRef, root: string, izin: boolean)
	if not root or root == "" then
		return
	end
	local k = anahtar(root)

	if pluginRef then
		pcall(function()
			pluginRef:SetSetting(k, izin)
		end)
	end

	local kayitlar = placeKayitlariniOku()
	kayitlar[k] = izin
	placeKayitlariniYaz(kayitlar)

	-- Geri okuma denetimi: hiçbir katman kalıcı olmadıysa kullanıcı bunu bilmeli,
	-- yoksa her Studio açılışında tekrar sorulmasını hata sanır.
	if Approval.GetStoredDecision(pluginRef, root) == nil then
		warn(
			"[Syncix] The decision could not be stored permanently; you will be asked on every Studio start.\n" ..
			"  Save the place (Ctrl+S) to store the decision with it."
		)
	end
end

-- Pencere kimliği her çağrıda benzersiz olmalı: aynı kimlikle ikinci kez
-- CreateDockWidgetPluginGui çağırmak hata verir.
local pencereSayaci = 0

-- Onay penceresini gösterir ve kullanıcı karar verene kadar bekler.
--
-- Dönüş üç durumludur ve bu ÖNEMLİ:
--   "allow"  kullanıcı onayladı
--   "deny"   kullanıcı reddetti (kalıcı olarak saklanır)
--   "error"  pencere açılamadı  -> KARAR DEĞİLDİR, saklanmaz, sonra tekrar denenir
-- Aksi halde arayüzdeki tek bir aksaklık senkronu kalıcı olarak kilitlerdi.
function Approval.Ask(pluginRef, bilgi): string
	local proje = tostring(bilgi.project or "Unknown project")
	local root = tostring(bilgi.root or "")
	local port = tostring(bilgi.port or "?")

	pencereSayaci += 1

	local ok, sonuc = pcall(function()
		local widgetInfo = DockWidgetPluginGuiInfo.new(
			Enum.InitialDockState.Float,
			true,  -- başlangıçta açık
			true,  -- kullanıcı geçmişini ezme
			460, 210,
			460, 210
		)
		local gui = pluginRef:CreateDockWidgetPluginGui(
			"SyncixBaglantiOnayi_" .. tostring(pencereSayaci),
			widgetInfo
		)
		gui.Title = "Syncix - Connection permission"

		local cerceve = Instance.new("Frame")
		cerceve.Size = UDim2.new(1, 0, 1, 0)
		cerceve.BackgroundColor3 = Color3.fromRGB(30, 33, 40)
		cerceve.BorderSizePixel = 0
		cerceve.Parent = gui

		local baslik = Instance.new("TextLabel")
		baslik.Size = UDim2.new(1, -24, 0, 28)
		baslik.Position = UDim2.new(0, 12, 0, 12)
		baslik.BackgroundTransparency = 1
		baslik.TextColor3 = Color3.fromRGB(255, 255, 255)
		baslik.TextXAlignment = Enum.TextXAlignment.Left
		baslik.Font = Enum.Font.GothamBold
		baslik.TextSize = 16
		baslik.Text = "This project wants to modify Studio"
		baslik.Parent = cerceve

		local detay = Instance.new("TextLabel")
		detay.Size = UDim2.new(1, -24, 0, 92)
		detay.Position = UDim2.new(0, 12, 0, 44)
		detay.BackgroundTransparency = 1
		detay.TextColor3 = Color3.fromRGB(210, 214, 222)
		detay.TextXAlignment = Enum.TextXAlignment.Left
		detay.TextYAlignment = Enum.TextYAlignment.Top
		detay.TextWrapped = true
		detay.Font = Enum.Font.Gotham
		detay.TextSize = 13
		detay.Text = string.format(
			"Project: %s\nFolder: %s\nPort: %s\n\nIf you do not recognise this folder, deny it. If you allow it, this project can create, modify and delete instances in the Explorer.",
			proje, root, port
		)
		detay.Parent = cerceve

		local function dugme(metin, x, renk)
			local b = Instance.new("TextButton")
			b.Size = UDim2.new(0, 200, 0, 34)
			b.Position = UDim2.new(0, x, 1, -46)
			b.BackgroundColor3 = renk
			b.BorderSizePixel = 0
			b.TextColor3 = Color3.fromRGB(255, 255, 255)
			b.Font = Enum.Font.GothamBold
			b.TextSize = 14
			b.Text = metin
			b.Parent = cerceve
			local kose = Instance.new("UICorner")
			kose.CornerRadius = UDim.new(0, 6)
			kose.Parent = b
			return b
		end

		local izinDugme = dugme("Allow", 12, Color3.fromRGB(46, 160, 87))
		local redDugme = dugme("Deny", 236, Color3.fromRGB(180, 60, 60))

		-- Pencere Studio tarafından kapalı durumda geri yüklenmiş olabilir; açık olduğundan emin ol.
		gui.Enabled = true

		local karar = nil
		local kapatildi = false
		local acilisAni = os.clock()

		izinDugme.Activated:Connect(function() karar = true end)
		redDugme.Activated:Connect(function() karar = false end)

		-- Pencerenin kapatılması REDDETME SAYILMAZ.
		--
		-- Eskiden sayılıyordu ve şu hataya yol açtı: DockWidgetPluginGui oluşturulurken
		-- Enabled bir an false oluyor, bu da kullanıcı hiçbir şeye basmadan "reddedildi"
		-- olarak yorumlanıp bağlantıyı kalıcı olarak engelliyordu.
		-- Artık kapatma "kararsız" demektir: saklanmaz, bir sonraki denemede tekrar sorulur.
		gui:GetPropertyChangedSignal("Enabled"):Connect(function()
			-- İlk saniye Studio'nun kendi pencere durumu geri yüklemesine ayrılmıştır.
			if not gui.Enabled and karar == nil and (os.clock() - acilisAni) > 1 then
				kapatildi = true
			end
		end)

		-- En fazla 2 dakika bekle; kullanıcı orada değilse bağlantı denemesi
		-- sonsuza kadar askıda kalmasın.
		while karar == nil and not kapatildi and (os.clock() - acilisAni) < 120 do
			task.wait(0.1)
		end

		gui.Enabled = false
		gui:Destroy()

		if karar == nil then
			return nil -- karar verilmedi
		end
		return karar
	end)

	if not ok then
		-- Pencere açılamadı. Bu bir RED DEĞİLDİR: kalıcı olarak saklanmaz,
		-- bağlantı bir sonraki denemede yeniden sorulur.
		warn(
			"[Syncix] Could not open the approval window: " .. tostring(sonuc) ..
			"\n  Not connected; will retry shortly."
		)
		return "error"
	end

	if sonuc == true then
		return "allow"
	end
	if sonuc == false then
		return "deny"
	end

	-- nil: pencere kapatıldı ya da zaman aşımına uğradı. Karar verilmedi,
	-- saklanmaz ve kara listeye alınmaz; bir sonraki denemede tekrar sorulur.
	print("[Syncix] Permission not granted (window closed). You will be asked again later.")
	return "error"
end

return Approval
