-- TopBarController.client.lua
-- Hızlı, Kesintisiz ve %100 Güvenilir Üst Bar Kontrolcüsü (Ada TP, Shop TP & DevMenu)

local Players = game:GetService("Players")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local TweenService = game:GetService("TweenService")
local Workspace = game:GetService("Workspace")

local localPlayer = Players.LocalPlayer

-- GUI Elemanları (Hemen alınıyor, hiçbir gecikme yok)
local container = script.Parent
local islandBtn = container:WaitForChild("IslandButton", 5)
local shopBtn = container:WaitForChild("ShopButton", 5)
local devMenuBtn = container:WaitForChild("DevMenuButton", 5)

local topBarGui = container.Parent
local devFrame = topBarGui and topBarGui:WaitForChild("DevMenuFrame", 5)

-- RemoteEvents (Asenkron veya anlık bağlanır, ana akışı ASLA kilitlemez)
local TeleportEvent = ReplicatedStorage:FindFirstChild("TeleportEvent")
local ClientTeleport = ReplicatedStorage:FindFirstChild("ClientTeleport")

-- Dükkanın Önü (Island Main Dükkanlar)
local SHOP_CF = CFrame.lookAt(Vector3.new(0, 127.5, -5), Vector3.new(10, 127.5, 5))

-- Karakteri güvenli ve anında taşıma fonksiyonu
local function applyTeleport(targetCF)
    if not targetCF then return end
    local char = localPlayer.Character
    if not char then return end
    local hrp = char:FindFirstChild("HumanoidRootPart")
    if hrp then
        hrp.AssemblyLinearVelocity = Vector3.zero
        hrp.AssemblyAngularVelocity = Vector3.zero
    end
    char:PivotTo(targetCF)
end

-- İstemci tarafı sunucudan gelen ışınlanma yanıtlarını dinler
local function setupRemoteListeners()
    if TeleportEvent and TeleportEvent:IsA("RemoteEvent") then
        TeleportEvent.OnClientEvent:Connect(function(actionOrCF, cf)
            local targetCF = (typeof(actionOrCF) == "CFrame" and actionOrCF) or cf
            if targetCF and typeof(targetCF) == "CFrame" then
                applyTeleport(targetCF)
            end
        end)
    end

    if ClientTeleport and ClientTeleport:IsA("RemoteEvent") then
        ClientTeleport.OnClientEvent:Connect(function(targetCF)
            if targetCF and typeof(targetCF) == "CFrame" then
                applyTeleport(targetCF)
            end
        end)
    end
end

setupRemoteListeners()

-- Eğer RemoteEvent henüz yüklenmediyse arka planda bekle ve bağla (Asenkron)
task.spawn(function()
    if not TeleportEvent then
        TeleportEvent = ReplicatedStorage:WaitForChild("TeleportEvent", 10)
        setupRemoteListeners()
    end
    if not ClientTeleport then
        ClientTeleport = ReplicatedStorage:WaitForChild("ClientTeleport", 10)
        setupRemoteListeners()
    end
end)

-- Oyuncunun kendi adasının koordinatını bul
local function getMyFarmCFrame()
    local farmFolder = Workspace:FindFirstChild("Farm")
    if not farmFolder then return nil end

    local assigned = localPlayer:GetAttribute("AssignedIsland")
    local farm = nil

    if assigned and farmFolder:FindFirstChild("Farm_" .. assigned) then
        farm = farmFolder["Farm_" .. assigned]
    else
        for _, f in ipairs(farmFolder:GetChildren()) do
            local data = f:FindFirstChild("Important") and f.Important:FindFirstChild("Data")
            if data and data:FindFirstChild("Owner") and data.Owner.Value == localPlayer.Name then
                farm = f
                break
            end
        end
    end

    if farm then
        local sp = farm:FindFirstChild("Spawn_Point", true)
        if sp and sp:IsA("BasePart") then
            return CFrame.new(sp.Position + Vector3.new(0, 3.5, 0))
        end
        local cp1 = farm:FindFirstChild("Important") and farm.Important:FindFirstChild("Plant_Locations") and farm.Important.Plant_Locations:FindFirstChild("Can_Plant1")
        if cp1 and cp1:IsA("BasePart") then
            return CFrame.new(cp1.Position + Vector3.new(0, 4.0, 0))
        end
        local cf = farm:GetBoundingBox()
        return CFrame.new(cf.Position + Vector3.new(0, 8.0, 0))
    end

    return nil
end

-- Buton tıklama animasyonu
local function animateClick(btn)
    if not btn then return end
    local origSize = btn.Size
    btn.Size = UDim2.new(origSize.X.Scale, origSize.X.Offset - 6, origSize.Y.Scale, origSize.Y.Offset - 4)
    task.delay(0.1, function()
        if btn then btn.Size = origSize end
    end)
end

-- =============================================================
-- TOAST BİLDİRİM SİSTEMİ (Notification_RE Dinleyicisi)
-- =============================================================
local toastContainer = topBarGui and topBarGui:FindFirstChild("ToastContainer")
if topBarGui and not toastContainer then
    toastContainer = Instance.new("Frame")
    toastContainer.Name = "ToastContainer"
    toastContainer.AnchorPoint = Vector2.new(0.5, 0)
    toastContainer.Position = UDim2.new(0.5, 0, 0, 68)
    toastContainer.Size = UDim2.new(0, 420, 0, 0)
    toastContainer.AutomaticSize = Enum.AutomaticSize.Y
    toastContainer.BackgroundTransparency = 1
    toastContainer.Parent = topBarGui

    local toastList = Instance.new("UIListLayout")
    toastList.FillDirection = Enum.FillDirection.Vertical
    toastList.HorizontalAlignment = Enum.HorizontalAlignment.Center
    toastList.VerticalAlignment = Enum.VerticalAlignment.Top
    toastList.Padding = UDim.new(0, 8)
    toastList.SortOrder = Enum.SortOrder.LayoutOrder
    toastList.Parent = toastContainer
end

local function showToast(title, message, accentColor)
    if not toastContainer then return end
    accentColor = accentColor or Color3.fromRGB(255, 215, 0)

    local toast = Instance.new("Frame")
    toast.Name = "ToastCard"
    toast.Size = UDim2.new(1, 0, 0, 64)
    toast.BackgroundColor3 = Color3.fromRGB(20, 24, 34)
    toast.BackgroundTransparency = 0.12
    toast.ClipsDescendants = true
    toast.Parent = toastContainer

    local corner = Instance.new("UICorner")
    corner.CornerRadius = UDim.new(0, 10)
    corner.Parent = toast

    local stroke = Instance.new("UIStroke")
    stroke.Thickness = 1.5
    stroke.Color = accentColor
    stroke.Transparency = 0.35
    stroke.Parent = toast

    local bar = Instance.new("Frame")
    bar.Size = UDim2.new(0, 6, 1, 0)
    bar.BackgroundColor3 = accentColor
    bar.BorderSizePixel = 0
    bar.Parent = toast
    local barCorner = Instance.new("UICorner")
    barCorner.CornerRadius = UDim.new(0, 4)
    barCorner.Parent = bar

    local titleLbl = Instance.new("TextLabel")
    titleLbl.Position = UDim2.new(0, 16, 0, 6)
    titleLbl.Size = UDim2.new(1, -24, 0, 20)
    titleLbl.BackgroundTransparency = 1
    titleLbl.Font = Enum.Font.FredokaOne
    titleLbl.Text = title or "Bildirim"
    titleLbl.TextColor3 = accentColor
    titleLbl.TextSize = 14
    titleLbl.TextXAlignment = Enum.TextXAlignment.Left
    titleLbl.Parent = toast

    local descLbl = Instance.new("TextLabel")
    descLbl.Position = UDim2.new(0, 16, 0, 26)
    descLbl.Size = UDim2.new(1, -24, 0, 32)
    descLbl.BackgroundTransparency = 1
    descLbl.Font = Enum.Font.GothamMedium
    descLbl.Text = message or ""
    descLbl.TextColor3 = Color3.fromRGB(240, 240, 245)
    descLbl.TextSize = 12
    descLbl.TextWrapped = true
    descLbl.TextXAlignment = Enum.TextXAlignment.Left
    descLbl.TextYAlignment = Enum.TextYAlignment.Top
    descLbl.Parent = toast

    -- Yumuşak Giriş Animasyonu
    toast.Position = UDim2.new(0, 0, 0, -15)
    toast.BackgroundTransparency = 1
    titleLbl.TextTransparency = 1
    descLbl.TextTransparency = 1
    bar.BackgroundTransparency = 1

    TweenService:Create(toast, TweenInfo.new(0.25, Enum.EasingStyle.Back, Enum.EasingDirection.Out), {
        Position = UDim2.new(0, 0, 0, 0),
        BackgroundTransparency = 0.12
    }):Play()
    TweenService:Create(titleLbl, TweenInfo.new(0.25), { TextTransparency = 0 }):Play()
    TweenService:Create(descLbl, TweenInfo.new(0.25), { TextTransparency = 0 }):Play()
    TweenService:Create(bar, TweenInfo.new(0.25), { BackgroundTransparency = 0 }):Play()

    -- 4 saniye sonra kapanış
    task.delay(4.0, function()
        if toast and toast.Parent then
            local outTween = TweenService:Create(toast, TweenInfo.new(0.3, Enum.EasingStyle.Quad, Enum.EasingDirection.In), {
                Position = UDim2.new(0, 0, 0, -20),
                BackgroundTransparency = 1
            })
            TweenService:Create(titleLbl, TweenInfo.new(0.3), { TextTransparency = 1 }):Play()
            TweenService:Create(descLbl, TweenInfo.new(0.3), { TextTransparency = 1 }):Play()
            TweenService:Create(bar, TweenInfo.new(0.3), { BackgroundTransparency = 1 }):Play()
            outTween:Play()
            outTween.Completed:Connect(function()
                toast:Destroy()
            end)
        end
    end)
end

-- Sunucudan gelen bildirimleri dinle
task.spawn(function()
    local gameEvents = ReplicatedStorage:WaitForChild("GameEvents", 10)
    local notifRE = gameEvents and gameEvents:WaitForChild("Notification_RE", 10)
    if notifRE and notifRE:IsA("RemoteEvent") then
        notifRE.OnClientEvent:Connect(function(title, message, color)
            showToast(title, message, color)
        end)
    end
end)

-- =============================================================
-- BUTON DÜZENİ VE ÇALMA GAMEPASS BUTONU
-- =============================================================
if container then
    container.AutomaticSize = Enum.AutomaticSize.X
    local listLayout = container:FindFirstChildOfClass("UIListLayout")
    if listLayout then
        listLayout.SortOrder = Enum.SortOrder.LayoutOrder
    end
end

if islandBtn then islandBtn.LayoutOrder = 1 end
if shopBtn then shopBtn.LayoutOrder = 2 end
if devMenuBtn then devMenuBtn.LayoutOrder = 4 end

-- The steal gamepass button that used to be built here was removed: the row holds
-- only the buttons that exist in StarterGui.

-- 1. ADA IŞINLANMA BUTONU
local islandDebounce = false
if islandBtn then
    islandBtn.MouseButton1Click:Connect(function()
        if islandDebounce then return end
        islandDebounce = true
        animateClick(islandBtn)

        -- 1. Sunucuya bildir
        if TeleportEvent then
            TeleportEvent:FireServer("Island")
        end

        -- 2. İstemcide anında ada koordinatına geç
        local myCF = getMyFarmCFrame()
        if myCF then
            applyTeleport(myCF)
        end

        task.wait(0.5)
        islandDebounce = false
    end)
end

-- 2. SHOP (MARKET) IŞINLANMA BUTONU
local shopDebounce = false
if shopBtn then
    shopBtn.MouseButton1Click:Connect(function()
        if shopDebounce then return end
        shopDebounce = true
        animateClick(shopBtn)

        -- 1. Sunucuya bildir
        if TeleportEvent then
            TeleportEvent:FireServer("Shop")
        end

        -- 2. İstemcide anında dükkan konumuna geç
        applyTeleport(SHOP_CF)

        task.wait(0.5)
        shopDebounce = false
    end)
end

-- 4. DEV MENU BUTONU VE KONTROLLER
-- =============================================================
-- DEV MENU GORUNUMU (ust bar ile ayni dil: studlu zemin, siyah kenar, kare kose)
-- =============================================================
local STUDS_IMAGE = "rbxassetid://71015713698716"

-- Bir butonun yazisi artik butonun kendisinde degil, icindeki Label'da duruyor:
-- ImageButton'in Text ozelligi yok.
local function setBtnText(btn, text)
	if not btn then return end
	local lbl = btn:FindFirstChild("Label")
	if lbl and lbl:IsA("TextLabel") then
		lbl.Text = text
	elseif btn:IsA("TextButton") then
		btn.Text = text
	end
end

-- Tema butonu: doku butonun kendisinde, yazi ustte, kenar tam siyah ve kose keskin.
local function makeThemedButton(parent, text, colour, textSize)
	local b = Instance.new("ImageButton")
	b.BackgroundColor3 = colour
	b.BorderSizePixel = 0
	b.Image = STUDS_IMAGE
	b.ImageColor3 = colour
	b.ScaleType = Enum.ScaleType.Tile
	b.TileSize = UDim2.fromOffset(38, 38)
	b.AutoButtonColor = true
	b.Parent = parent

	local lbl = Instance.new("TextLabel")
	lbl.Name = "Label"
	lbl.Size = UDim2.fromScale(1, 1)
	lbl.BackgroundTransparency = 1
	lbl.Text = text
	lbl.TextColor3 = Color3.fromRGB(255, 255, 255)
	lbl.TextSize = textSize or 12
	lbl.Font = Enum.Font.GothamBold
	lbl.TextStrokeTransparency = 0.45
	lbl.Parent = b

	local stroke = Instance.new("UIStroke")
	stroke.Color = Color3.new(0, 0, 0)
	stroke.Thickness = 2
	stroke.ApplyStrokeMode = Enum.ApplyStrokeMode.Border
	stroke.LineJoinMode = Enum.LineJoinMode.Miter
	stroke.Parent = b

	return b
end

if devMenuBtn and devFrame then
    devMenuBtn.MouseButton1Click:Connect(function()
        animateClick(devMenuBtn)
        devFrame.Visible = not devFrame.Visible
    end)

    local closeBtn = devFrame:FindFirstChild("CloseButton")
    if closeBtn then
        closeBtn.MouseButton1Click:Connect(function()
            devFrame.Visible = false
        end)
    end

    -- GameEvents ve Admin Events arka planda bağlanır
    task.spawn(function()
        local gameEvents = ReplicatedStorage:WaitForChild("GameEvents", 10)
        local devGiveSeed = gameEvents and gameEvents:WaitForChild("DevGiveSeed", 10)
        local AdminActionEvent = ReplicatedStorage:FindFirstChild("AdminActionEvent")

        local giveAllBtn = devFrame:FindFirstChild("GiveAllSeedsButton")
        if giveAllBtn and devGiveSeed then
            giveAllBtn.MouseButton1Click:Connect(function()
                devGiveSeed:FireServer("ALL")
                setBtnText(giveAllBtn, "TUM TOHUMLAR VERILDI")
                task.delay(1.5, function()
                    if giveAllBtn then setBtnText(giveAllBtn, "TUM TOHUMLARI VER (40x)") end
                end)
            end)
        end

        local giveMoneyBtn = devFrame:FindFirstChild("GiveMoneyButton")
        if giveMoneyBtn and devGiveSeed then
            giveMoneyBtn.MouseButton1Click:Connect(function()
                devGiveSeed:FireServer("MONEY")
                setBtnText(giveMoneyBtn, "+100.000 SHECKLES VERILDI")
                task.delay(1.5, function()
                    if giveMoneyBtn then setBtnText(giveMoneyBtn, "+10.000 SHECKLES") end
                end)
            end)
        end

        -- Hız & Büyüme & Pass Kontrol Konteyneri
        local speedContainer = devFrame:FindFirstChild("SpeedControlsContainer")
        if not speedContainer then
            speedContainer = Instance.new("Frame")
            speedContainer.Name = "SpeedControlsContainer"
            speedContainer.Size = UDim2.new(1, -30, 0, 195)
            speedContainer.Position = UDim2.new(0, 15, 0, 175)
            speedContainer.BackgroundTransparency = 1
            speedContainer.Parent = devFrame

            local listLayout = Instance.new("UIListLayout")
            listLayout.Padding = UDim.new(0, 6)
            listLayout.SortOrder = Enum.SortOrder.LayoutOrder
            listLayout.Parent = speedContainer

            -- Başlık 1: Oyuncu Hızı
            local spdLbl = Instance.new("TextLabel")
            spdLbl.Size = UDim2.new(1, 0, 0, 16)
            spdLbl.BackgroundTransparency = 1
            spdLbl.Text = "OYUNCU HIZI"
            spdLbl.TextColor3 = Color3.fromRGB(255, 215, 60)
            spdLbl.Font = Enum.Font.FredokaOne
            spdLbl.TextSize = 11
            spdLbl.TextXAlignment = Enum.TextXAlignment.Left
            spdLbl.LayoutOrder = 1
            spdLbl.Parent = speedContainer

            local spdRow = Instance.new("Frame")
            spdRow.Size = UDim2.new(1, 0, 0, 28)
            spdRow.BackgroundTransparency = 1
            spdRow.LayoutOrder = 2
            spdRow.Parent = speedContainer

            local spdGrid = Instance.new("UIGridLayout")
            spdGrid.CellSize = UDim2.new(0, 98, 0, 28)
            spdGrid.CellPadding = UDim2.new(0, 6, 0, 0)
            spdGrid.Parent = spdRow

            local function makeMiniBtn(parent, text, color, cb)
                local b = makeThemedButton(parent, text, color, 11)
                b.MouseButton1Click:Connect(cb)
                return b
            end

            local function applySpeed(val)
                local char = localPlayer.Character
                local hum = char and char:FindFirstChildOfClass("Humanoid")
                if hum then hum.WalkSpeed = val end
                if AdminActionEvent then AdminActionEvent:FireServer("SetSpeed", val) end
            end

            makeMiniBtn(spdRow, "Normal 16", Color3.fromRGB(35, 40, 52), function() applySpeed(16) end)
            makeMiniBtn(spdRow, "Hizli 45", Color3.fromRGB(45, 80, 120), function() applySpeed(45) end)
            makeMiniBtn(spdRow, "Flas 120", Color3.fromRGB(160, 90, 30), function() applySpeed(120) end)
            makeMiniBtn(spdRow, "Sonic 250", Color3.fromRGB(180, 45, 45), function() applySpeed(250) end)

            -- Başlık 2: Bitki Büyüme Hızı
            local growLbl = Instance.new("TextLabel")
            growLbl.Size = UDim2.new(1, 0, 0, 16)
            growLbl.BackgroundTransparency = 1
            growLbl.Text = "BITKI BUYUME HIZI"
            growLbl.TextColor3 = Color3.fromRGB(130, 230, 130)
            growLbl.Font = Enum.Font.FredokaOne
            growLbl.TextSize = 11
            growLbl.TextXAlignment = Enum.TextXAlignment.Left
            growLbl.LayoutOrder = 3
            growLbl.Parent = speedContainer

            local growRow = Instance.new("Frame")
            growRow.Size = UDim2.new(1, 0, 0, 28)
            growRow.BackgroundTransparency = 1
            growRow.LayoutOrder = 4
            growRow.Parent = speedContainer

            local growGrid = Instance.new("UIGridLayout")
            growGrid.CellSize = UDim2.new(0, 98, 0, 28)
            growGrid.CellPadding = UDim2.new(0, 6, 0, 0)
            growGrid.Parent = growRow

            makeMiniBtn(growRow, "Aninda 0s", Color3.fromRGB(200, 140, 20), function()
                if AdminActionEvent then AdminActionEvent:FireServer("SetGrowthMultiplier", 999) end
                if devGiveSeed then devGiveSeed:FireServer("GROW_SPEED", 999) end
            end)
            makeMiniBtn(growRow, "10x Hiz", Color3.fromRGB(40, 150, 70), function()
                if AdminActionEvent then AdminActionEvent:FireServer("SetGrowthMultiplier", 10) end
                if devGiveSeed then devGiveSeed:FireServer("GROW_SPEED", 10) end
            end)
            makeMiniBtn(growRow, "1x Normal", Color3.fromRGB(50, 60, 75), function()
                if AdminActionEvent then AdminActionEvent:FireServer("SetGrowthMultiplier", 1) end
                if devGiveSeed then devGiveSeed:FireServer("GROW_SPEED", 1) end
            end)
            makeMiniBtn(growRow, "Tumunu Buyut", Color3.fromRGB(150, 50, 140), function()
                if AdminActionEvent then AdminActionEvent:FireServer("InstaGrowAll") end
                if devGiveSeed then devGiveSeed:FireServer("INSTA_GROW") end
            end)

            -- Başlık 3: 9 Robuxluk Çalma Pass Testi
            local stealLbl = Instance.new("TextLabel")
            stealLbl.Size = UDim2.new(1, 0, 0, 16)
            stealLbl.BackgroundTransparency = 1
            stealLbl.Text = "HIRSIZLIK PASS TESTI"
            stealLbl.TextColor3 = Color3.fromRGB(200, 130, 255)
            stealLbl.Font = Enum.Font.FredokaOne
            stealLbl.TextSize = 11
            stealLbl.TextXAlignment = Enum.TextXAlignment.Left
            stealLbl.LayoutOrder = 5
            stealLbl.Parent = speedContainer

            local stealDevRow = Instance.new("Frame")
            stealDevRow.Size = UDim2.new(1, 0, 0, 28)
            stealDevRow.BackgroundTransparency = 1
            stealDevRow.LayoutOrder = 6
            stealDevRow.Parent = speedContainer

            local devStealToggleBtn = Instance.new("TextButton")
            devStealToggleBtn.Size = UDim2.new(1, 0, 1, 0)
            devStealToggleBtn.Font = Enum.Font.FredokaOne
            devStealToggleBtn.TextSize = 12
            devStealToggleBtn.TextColor3 = Color3.fromRGB(255, 255, 255)
            devStealToggleBtn.Parent = stealDevRow
            local toggleCorner = Instance.new("UICorner")
            toggleCorner.CornerRadius = UDim.new(0, 6)
            toggleCorner.Parent = devStealToggleBtn

            local function syncDevStealBtn()
                local hasPass = localPlayer:GetAttribute("HasStealPass") == true
                if hasPass then
                    devStealToggleBtn.Text = "Hirsizlik Pass: ACIK"
                    devStealToggleBtn.BackgroundColor3 = Color3.fromRGB(35, 145, 75)
                else
                    devStealToggleBtn.Text = "Hirsizlik Pass: KAPALI"
                    devStealToggleBtn.BackgroundColor3 = Color3.fromRGB(160, 45, 45)
                end
            end
            syncDevStealBtn()
            localPlayer:GetAttributeChangedSignal("HasStealPass"):Connect(syncDevStealBtn)

            devStealToggleBtn.MouseButton1Click:Connect(function()
                if AdminActionEvent then
                    AdminActionEvent:FireServer("ToggleStealPass")
                end
            end)
        end

        local scroll = devFrame:FindFirstChild("SeedsScroll")
        if scroll and devGiveSeed then
            for _, child in ipairs(scroll:GetChildren()) do
                if child:IsA("TextButton") then
                    child.MouseButton1Click:Connect(function()
                        local sName = string.gsub(child.Name, "Btn", "")
                        devGiveSeed:FireServer("SEED", sName)
                        child.Text = "Alindi"
                        task.delay(1.0, function()
                            if child then child.Text = "+ " .. sName end
                        end)
                    end)
                end
            end
        end
    end)
end

print("[TopBarController] TopBar ve DevMenu hazir")

-- =============================================================
-- HOVER ANIMASYONU (satırdaki her butona, sonradan eklenenler dahil)
-- =============================================================
-- Buyume UIScale ile yapiliyor: UIListLayout butonun Size'ina bakar, UIScale'e bakmaz,
-- yani buton buyurken komsulari yerinden oynamiyor.
local HOVER_SCALE = 1.07
local HOVER_TIME = TweenInfo.new(0.12, Enum.EasingStyle.Quad, Enum.EasingDirection.Out)

local function addHover(btn)
	if not btn:IsA("GuiButton") then return end
	if btn:GetAttribute("HoverReady") then return end
	btn:SetAttribute("HoverReady", true)

	local scale = btn:FindFirstChildOfClass("UIScale")
	if not scale then
		scale = Instance.new("UIScale")
		scale.Parent = btn
	end

	local function tweenTo(value)
		TweenService:Create(scale, HOVER_TIME, {Scale = value}):Play()
	end

	btn.MouseEnter:Connect(function() tweenTo(HOVER_SCALE) end)
	btn.MouseLeave:Connect(function() tweenTo(1) end)
	-- Basarken hafif iceri cokuyor, birakinca hover boyutuna donuyor.
	btn.MouseButton1Down:Connect(function() tweenTo(0.96) end)
	btn.MouseButton1Up:Connect(function() tweenTo(HOVER_SCALE) end)
end

if container then
	for _, child in ipairs(container:GetChildren()) do
		addHover(child)
	end
	container.ChildAdded:Connect(addHover)
end
