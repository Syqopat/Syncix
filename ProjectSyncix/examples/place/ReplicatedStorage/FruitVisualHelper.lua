-- FruitVisualHelper.lua
-- Ağaçta Büyüyen Meyveler ile Eldeki / Envanterdeki Meyve Eşyalarını (Tool) 1-e-1 BİREBİR AYNI Kılan Merkezi Görsel Yardımcı
-- Workspace["Grow A garden"], ReplicatedStorage.Fruit_Spawn ve ServerStorage.Collectables kaynaklarını entegre eder

local ReplicatedStorage = game:GetService("ReplicatedStorage")
local ServerStorage = game:GetService("ServerStorage")
local Workspace = game:GetService("Workspace")

local FruitVisualHelper = {}

-- -------------------------------------------------------------
-- 1. Kanonik Bitki İsmi Çözümleyici
-- -------------------------------------------------------------
function FruitVisualHelper.getCanonicalPlantName(raw)
    if not raw then return "Carrot" end
    local s = tostring(raw):lower()
    s = s:gsub("%s*seed.*", "")
    s = s:gsub("%s*tree.*", "")
    s = s:gsub("%s*bush.*", "")
    s = s:gsub("%s*plant.*", "")
    s = s:gsub("%s*palm.*", "")
    s = s:gsub("%s*x%d+", "")
    s = s:gsub("[^%a]", "")

    if s:find("carrot") or s:find("havuc") then return "Carrot"
    elseif s:find("potato") or s:find("patates") then return "Potato"
    elseif s:find("straw") or s:find("cilek") then return "Strawberry"
    elseif s:find("blue") or s:find("mersin") or s:find("yaban") then return "Blueberry"
    elseif s:find("apple") or s:find("elma") then return "Apple Tree"
    elseif s:find("mango") then return "Mango Tree"
    elseif s:find("frost") or s:find("buz") then return "Frostleaf"
    elseif s:find("cher") or s:find("kiraz") or s:find("sakura") then return "Cherry Tree"
    elseif s:find("banana") or s:find("muz") then return "Banana Tree"
    elseif s:find("dragon") or s:find("ejder") or s:find("pitaya") then return "Dragonfruit Bush"
    elseif s:find("star") or s:find("yildiz") then return "Starfruit Plant"
    elseif s:find("coco") or s:find("palm") or s:find("ceviz") then return "Coconut Palm"
    elseif s:find("cryst") or s:find("kristal") then return "Crystal Shrub"
    elseif s:find("nebula") or s:find("vine") or s:find("sarmasik") then return "Nebula Vine"
    elseif s:find("celest") or s:find("goksel") or s:find("eden") then return "Celestial Tree"
    end

    return "Carrot"
end

-- -------------------------------------------------------------
-- 2. Meyve İsim Eşleme Tablosu
-- -------------------------------------------------------------
local FRUIT_SPAWN_MAP = {
    ["Apple Tree"] = "Apple",
    ["Banana Tree"] = "Banana",
    ["Coconut Palm"] = "Coconut",
    ["Mango Tree"] = "Mango",
    ["Dragonfruit Bush"] = "Dragon Fruit",
    ["Celestial Tree"] = "Eden Fruit",
    ["Cherry Tree"] = "Cherry Blossom",
    ["Blueberry"] = "Blueberry",
    ["Strawberry"] = "Strawberry",
    ["Carrot"] = "Carrot",
    ["Potato"] = "Potato",
    ["Frostleaf"] = "Frostleaf",
    ["Starfruit Plant"] = "Starfruit",
    ["Crystal Shrub"] = "Crystal",
    ["Nebula Vine"] = "Nebula",
}

function FruitVisualHelper.getFruitSpawnName(canonical)
    return FRUIT_SPAWN_MAP[canonical] or canonical
end

-- -------------------------------------------------------------
-- 3. Yüksek Kaliteli Çok Parçalı 3D Usulü (Procedural) Meyve Üretici
--    (Fruit_Spawn'da hazır modeli bulunmayan veya özel türler için)
-- -------------------------------------------------------------
function FruitVisualHelper.createProceduralFruit(canonical, scale)
    local m = Instance.new("Model")
    m.Name = canonical .. "_FruitModel"
    scale = scale or 1.0

    local function makePart(parent, name, shape, size, color, mat, cf, transp)
        local p = Instance.new("Part")
        p.Name = name or "Part"
        if shape then p.Shape = shape end
        p.Size = (size or Vector3.new(1, 1, 1)) * scale
        p.Color = color or Color3.fromRGB(200, 200, 200)
        p.Material = mat or Enum.Material.SmoothPlastic
        p.Anchored = false
        p.CanCollide = false
        p.Massless = true
        p.CastShadow = true
        p.Transparency = transp or 0
        if cf then p.CFrame = cf end
        p.Parent = parent
        return p
    end

    -- =========================================================
    -- CARROT (Havuç)
    -- =========================================================
    if canonical == "Carrot" then
        local body = makePart(m, "CarrotBody", Enum.PartType.Cylinder, Vector3.new(1.6, 0.75, 0.75), Color3.fromRGB(245, 115, 15), Enum.Material.SmoothPlastic, CFrame.Angles(0, 0, math.rad(90)))
        m.PrimaryPart = body
        local tip = makePart(m, "CarrotTip", Enum.PartType.Wedge, Vector3.new(0.5, 0.7, 0.5), Color3.fromRGB(235, 100, 10), Enum.Material.SmoothPlastic, body.CFrame * CFrame.new(0, -1.0, 0) * CFrame.Angles(math.rad(180), 0, 0))
        local crown = makePart(m, "LeafCrown", Enum.PartType.Block, Vector3.new(0.35, 0.4, 0.35), Color3.fromRGB(55, 165, 40), Enum.Material.Grass, body.CFrame * CFrame.new(0, 0.9, 0))
        for i = 1, 4 do
            local ang = math.rad(i * 90)
            makePart(m, "Leaf_" .. i, Enum.PartType.Block, Vector3.new(0.18, 0.9, 0.4), Color3.fromRGB(45, 175, 35), Enum.Material.Grass, crown.CFrame * CFrame.Angles(0, ang, math.rad(30)) * CFrame.new(0, 0.45, 0.2))
        end

    -- =========================================================
    -- POTATO (Patates)
    -- =========================================================
    elseif canonical == "Potato" then
        local tuber = makePart(m, "PotatoTuber", Enum.PartType.Ball, Vector3.new(1.3, 1.0, 1.5), Color3.fromRGB(155, 115, 68), Enum.Material.Slate)
        m.PrimaryPart = tuber
        makePart(m, "TuberEnd", Enum.PartType.Ball, Vector3.new(0.9, 0.8, 0.9), Color3.fromRGB(145, 105, 60), Enum.Material.Slate, tuber.CFrame * CFrame.new(0.4, 0.1, 0.4))
        for i = 1, 5 do
            local ang = math.rad(i * 72)
            makePart(m, "Eye_" .. i, Enum.PartType.Ball, Vector3.new(0.2, 0.2, 0.2), Color3.fromRGB(105, 75, 45), Enum.Material.Slate, tuber.CFrame * CFrame.Angles(0, ang, 0) * CFrame.new(0.5, 0.15, 0))
        end

    -- =========================================================
    -- CHERRY (Kiraz - İkili Saplı Çift Kiraz)
    -- =========================================================
    elseif canonical == "Cherry Tree" then
        local c1 = makePart(m, "Cherry1", Enum.PartType.Ball, Vector3.new(0.9, 0.9, 0.9), Color3.fromRGB(205, 15, 40), Enum.Material.SmoothPlastic)
        m.PrimaryPart = c1
        local c2 = makePart(m, "Cherry2", Enum.PartType.Ball, Vector3.new(0.85, 0.85, 0.85), Color3.fromRGB(220, 20, 50), Enum.Material.SmoothPlastic, c1.CFrame * CFrame.new(0.8, -0.15, 0.3))
        local stem1 = makePart(m, "Stem1", Enum.PartType.Cylinder, Vector3.new(1.1, 0.12, 0.12), Color3.fromRGB(85, 135, 45), Enum.Material.SmoothPlastic, c1.CFrame * CFrame.new(0.35, 0.6, 0.1) * CFrame.Angles(0, 0, math.rad(-30)))
        local stem2 = makePart(m, "Stem2", Enum.PartType.Cylinder, Vector3.new(1.1, 0.12, 0.12), Color3.fromRGB(85, 135, 45), Enum.Material.SmoothPlastic, c2.CFrame * CFrame.new(-0.25, 0.6, -0.1) * CFrame.Angles(0, 0, math.rad(30)))
        makePart(m, "Leaf", Enum.PartType.Block, Vector3.new(0.12, 0.3, 0.6), Color3.fromRGB(60, 160, 40), Enum.Material.Grass, c1.CFrame * CFrame.new(0.5, 1.15, 0.1) * CFrame.Angles(math.rad(25), 0, math.rad(45)))

    -- =========================================================
    -- STARFRUIT (Yıldız Meyvesi)
    -- =========================================================
    elseif canonical == "Starfruit Plant" then
        local core = makePart(m, "StarCore", Enum.PartType.Cylinder, Vector3.new(1.5, 0.65, 0.65), Color3.fromRGB(255, 215, 20), Enum.Material.SmoothPlastic, CFrame.Angles(0, 0, math.rad(90)))
        m.PrimaryPart = core
        for i = 1, 5 do
            local ang = math.rad(i * 72)
            makePart(m, "StarRidge_" .. i, Enum.PartType.Wedge, Vector3.new(0.22, 1.4, 0.65), Color3.fromRGB(255, 235, 45), Enum.Material.SmoothPlastic, core.CFrame * CFrame.Angles(ang, 0, 0) * CFrame.new(0, 0, 0.45))
        end
        makePart(m, "Stem", Enum.PartType.Cylinder, Vector3.new(0.45, 0.15, 0.15), Color3.fromRGB(90, 150, 45), Enum.Material.SmoothPlastic, core.CFrame * CFrame.new(0, 0.85, 0))

    -- =========================================================
    -- FROSTLEAF (Buz Kristali Meyvesi)
    -- =========================================================
    elseif canonical == "Frostleaf" then
        local iceCore = makePart(m, "IceCore", Enum.PartType.Ball, Vector3.new(1.1, 1.3, 1.1), Color3.fromRGB(170, 235, 255), Enum.Material.Ice)
        m.PrimaryPart = iceCore
        for i = 1, 6 do
            local ang = math.rad(i * 60)
            makePart(m, "IceShard_" .. i, Enum.PartType.Wedge, Vector3.new(0.3, 0.9, 0.45), Color3.fromRGB(215, 245, 255), Enum.Material.Glass, iceCore.CFrame * CFrame.Angles(0, ang, math.rad(25)) * CFrame.new(0, 0.35, 0.45), 0.15)
        end

    -- =========================================================
    -- CRYSTAL SHRUB (Kristal Meyvesi)
    -- =========================================================
    elseif canonical == "Crystal Shrub" then
        local gemCore = makePart(m, "GemCore", Enum.PartType.Block, Vector3.new(1.0, 1.6, 1.0), Color3.fromRGB(210, 85, 255), Enum.Material.Neon, CFrame.Angles(math.rad(15), math.rad(25), 0))
        m.PrimaryPart = gemCore
        for i = 1, 4 do
            local ang = math.rad(i * 90 + 45)
            makePart(m, "Facet_" .. i, Enum.PartType.Wedge, Vector3.new(0.4, 1.2, 0.5), Color3.fromRGB(235, 140, 255), Enum.Material.Neon, gemCore.CFrame * CFrame.Angles(0, ang, math.rad(20)) * CFrame.new(0, 0.2, 0.4), 0.1)
        end

    -- =========================================================
    -- NEBULA VINE (Nebula Meyvesi)
    -- =========================================================
    elseif canonical == "Nebula Vine" then
        local cosmicOrb = makePart(m, "CosmicOrb", Enum.PartType.Ball, Vector3.new(1.3, 1.3, 1.3), Color3.fromRGB(185, 45, 230), Enum.Material.Neon)
        m.PrimaryPart = cosmicOrb
        makePart(m, "Ring", Enum.PartType.Cylinder, Vector3.new(0.18, 2.3, 2.3), Color3.fromRGB(120, 210, 255), Enum.Material.Glass, cosmicOrb.CFrame * CFrame.Angles(math.rad(45), 0, math.rad(30)), 0.2)

    -- =========================================================
    -- CELESTIAL TREE (Galaxy Portal Meyvesi - Halkasız)
    -- =========================================================
    elseif canonical == "Celestial Tree" then
        local c_obsidian = Color3.fromRGB(12, 16, 18)
        local c_emerald = Color3.fromRGB(45, 255, 115)
        local c_mint = Color3.fromRGB(150, 255, 205)

        local voidCore = makePart(m, "Portal_VoidCore", Enum.PartType.Ball, Vector3.new(1.2, 1.2, 1.2), c_obsidian, Enum.Material.SmoothPlastic)
        m.PrimaryPart = voidCore

        makePart(m, "Portal_Singularity", Enum.PartType.Ball, Vector3.new(0.55, 0.55, 0.55), c_mint, Enum.Material.Neon, voidCore.CFrame)

        -- 3D Galaxy Vortex Arms (Halkasız)
        for arm = 0, 1 do
            local baseRot = arm * math.pi
            for s = 1, 8 do
                local t = s / 8
                local ang = baseRot + t * math.pi * 1.5
                local yOff = (0.5 - t) * 0.95
                local rad = 0.65
                local pPos = voidCore.CFrame * CFrame.new(math.cos(ang) * rad, yOff, math.sin(ang) * rad)
                makePart(m, "ArmNode_" .. arm .. "_" .. s, Enum.PartType.Ball, Vector3.new(0.24, 0.24, 0.24), (s % 2 == 0 and c_emerald or c_mint), Enum.Material.Neon, pPos)
            end
        end

        makePart(m, "Portal_FlareX", Enum.PartType.Block, Vector3.new(1.4, 0.15, 0.15), c_emerald, Enum.Material.Neon, voidCore.CFrame)
        makePart(m, "Portal_FlareY", Enum.PartType.Block, Vector3.new(0.15, 1.4, 0.15), c_emerald, Enum.Material.Neon, voidCore.CFrame)

        local stem = makePart(m, "Stem", Enum.PartType.Cylinder, Vector3.new(0.4, 0.12, 0.12), c_obsidian, Enum.Material.SmoothPlastic, voidCore.CFrame * CFrame.new(0, 0.72, 0))
        makePart(m, "StemLeaf", Enum.PartType.Block, Vector3.new(0.1, 0.25, 0.4), c_emerald, Enum.Material.Neon, stem.CFrame * CFrame.new(0.15, -0.05, 0))

    -- =========================================================
    -- GENERIC FALLBACK
    -- =========================================================
    else
        local ball = makePart(m, "FruitBody", Enum.PartType.Ball, Vector3.new(1.1, 1.1, 1.1), Color3.fromRGB(245, 175, 40), Enum.Material.SmoothPlastic)
        m.PrimaryPart = ball
        makePart(m, "Stem", Enum.PartType.Cylinder, Vector3.new(0.4, 0.15, 0.15), Color3.fromRGB(90, 150, 45), Enum.Material.SmoothPlastic, ball.CFrame * CFrame.new(0, 0.65, 0))
    end

    return m
end

-- -------------------------------------------------------------
-- 4. Birebir Özdeş 3D Meyve Modeli Çıkarıcı
--    (Workspace["Grow A garden"] -> Fruit_Spawn -> Collectables -> Procedural)
-- -------------------------------------------------------------
function FruitVisualHelper.getFruitModel(seedName, scale, variant, isForHand)
    local canonical = FruitVisualHelper.getCanonicalPlantName(seedName)
    local fruitName = FruitVisualHelper.getFruitSpawnName(canonical)
    scale = scale or 1.0

    local fruitModel = nil

    -- 0. Öncelik: ReplicatedStorage.PlantAssets.Fruits (yeni low-poly meyveler). Ağaçtaki
    -- meyve ile eldeki eşya aynı modelden geldiği için ikisi birebir aynı görünür.
    local plantAssets = ReplicatedStorage:FindFirstChild("PlantAssets")
    local assetFruits = plantAssets and plantAssets:FindFirstChild("Fruits")
    local assetFruit = assetFruits and (assetFruits:FindFirstChild(canonical) or assetFruits:FindFirstChild(fruitName))
    if assetFruit then
        fruitModel = assetFruit:Clone()
    end

    -- 1. Öncelik: game.Workspace["Grow A garden"] altındaki özel meyve modelleri
    local gagWorkspace = (not fruitModel) and Workspace:FindFirstChild("Grow A garden")
    if gagWorkspace then
        local wsModel = gagWorkspace:FindFirstChild(fruitName) or gagWorkspace:FindFirstChild(fruitName .. " Fruit") or gagWorkspace:FindFirstChild(canonical)
        if wsModel and (wsModel:IsA("Model") or wsModel:IsA("BasePart")) then
            fruitModel = wsModel:Clone()
        end
    end

    -- 2. Öncelik: ReplicatedStorage.Fruit_Spawn altındaki otantik Grow A Garden modelleri
    if not fruitModel then
        local fruitSpawnFolder = ReplicatedStorage:FindFirstChild("Fruit_Spawn")
        if fruitSpawnFolder then
            local template = fruitSpawnFolder:FindFirstChild(fruitName)
            if not template and canonical == "Cherry Tree" then
                template = fruitSpawnFolder:FindFirstChild("Cherry OLD") or fruitSpawnFolder:FindFirstChild("Cherry Blossom")
            end
            if template then
                fruitModel = template:Clone()
            end
        end
    end

    -- 3. Öncelik: ServerStorage.Collectables altındaki meyve klasörleri
    if not fruitModel then
        local collectablesFolder = ServerStorage:FindFirstChild("Collectables")
        if collectablesFolder then
            local col = collectablesFolder:FindFirstChild(fruitName) or collectablesFolder:FindFirstChild(canonical)
            if col then
                local fruitsChild = col:FindFirstChild("Fruits") or col:FindFirstChild("Fruit_Spawn")
                if fruitsChild then
                    local firstFruit = fruitsChild:FindFirstChildWhichIsA("Model") or fruitsChild:FindFirstChildWhichIsA("BasePart")
                    if firstFruit then
                        fruitModel = firstFruit:Clone()
                    end
                end
            end
        end
    end

    -- 4. Öncelik: Yüksek kaliteli usulü 3D model üretici
    if not fruitModel then
        -- 1.0: ölçek aşağıda modelin tamamına uygulanacak
        fruitModel = FruitVisualHelper.createProceduralFruit(canonical, 1.0)
    end

    -- Tek bir BasePart geldiyse Model içine al
    if fruitModel:IsA("BasePart") then
        local container = Instance.new("Model")
        container.Name = canonical .. "_Fruit"
        fruitModel.Parent = container
        container.PrimaryPart = fruitModel
        fruitModel = container
    end

    -- Modelin PrimaryPart'ını garantiye al
    if not fruitModel.PrimaryPart then
        local p = fruitModel:FindFirstChild("1") or fruitModel:FindFirstChildWhichIsA("BasePart")
        if p then fruitModel.PrimaryPart = p end
    end

    -- Tüm parçaları düzenle (Şeffaflık düzeltmesi, ölçeklendirme, varyant boyama)
    for _, part in ipairs(fruitModel:GetDescendants()) do
        if part:IsA("BasePart") then
            -- Fruit_Spawn modelleri şeffaf (transparency 1.0) başladığı için görünür yap.
            -- Taban/kök parçaları hariç: onlar modelin pivotu olsun diye görünmez
            -- duruyor, görünür yapılınca meyvenin altında beyaz bir plaka beliriyordu.
            local isAnchorPart = part.Name == "PrimaryPart" or part.Name == "Root"
                or part.Name == "Handle" or part.Name == "Marker"
            if not isAnchorPart and part.Size.Magnitude > 0.08 then
                part.Transparency = 0
            end
            part.CastShadow = true

            -- Elde tutma veya ağaçta asılı kalma durumuna göre fizik ayarı
            if isForHand then
                part.Anchored = false
                part.CanCollide = false
                part.Massless = true
            else
                part.Anchored = true
                part.CanCollide = false
            end

            -- Varyant efektleri (Altın ve Gökkuşağı)
            if variant == "Gold" then
                part.Color = Color3.fromRGB(255, 215, 0)
                part.Material = Enum.Material.Metal
            elseif variant == "Rainbow" then
                part.Material = Enum.Material.Neon
            end
        end
    end

    -- Ağırlık ölçeklendirmesi modelin TAMAMINA uygulanır. Parçaların Size'ını tek tek
    -- çarpmak aralarındaki mesafeyi sabit bıraktığı için meyveyi dağıtıyordu: gövde
    -- büyürken sap ve yaprak yerinde kalıyor, arada boşluk açılıyordu.
    if scale ~= 1.0 then
        pcall(function()
            fruitModel:ScaleTo(scale)
        end)
    end

    if variant == "Rainbow" and fruitModel.PrimaryPart then
        local sp = Instance.new("Sparkles")
        sp.SparkleColor = Color3.fromRGB(255, 120, 255)
        sp.Parent = fruitModel.PrimaryPart
    end

    return fruitModel
end

-- Ağırlıktan boyuta: kütle hacimle arttığı için küp kök. Üst sınır yok; 100 kg'lık bir
-- meyve gerçekten devasa görünür, sadece çok nadir çıkar.
function FruitVisualHelper.weightToScale(weight)
    weight = math.max(tonumber(weight) or 1, 0.05)
    return (weight / 1.5) ^ (1 / 3) * 1.15
end

-- -------------------------------------------------------------
-- 5. Elde Tutulan Meyve Eşyası (Tool) Üretici
--    (Ağaçtaki meyve modelinin BİREBİR AYNISINI Tool içine weld eder)
-- -------------------------------------------------------------
function FruitVisualHelper.createCropTool(displayName, cleanSeed, weight, variant, reward, index)
    local canonical = FruitVisualHelper.getCanonicalPlantName(cleanSeed)
    local weightScale = FruitVisualHelper.weightToScale(weight)

    local toolName = index and string.format("%s #%d [%.1f kg]", displayName, index, weight)
        or string.format("%s [%.1f kg]", displayName, weight)

    if variant and variant ~= "Normal" and not toolName:find(variant) then
        toolName = "✨ " .. variant .. " " .. toolName
    end

    local cropTool = Instance.new("Tool")
    cropTool.Name = toolName
    cropTool:SetAttribute("Item_String", canonical)
    cropTool:SetAttribute("Weight", weight)
    cropTool:SetAttribute("Variant", variant or "Normal")
    cropTool:SetAttribute("Value", reward or 25)

    -- Tool'un kök Handle parçası (Görünmez, hafif)
    local handle = Instance.new("Part")
    handle.Name = "Handle"
    handle.Size = Vector3.new(0.6, 0.6, 0.6)
    handle.Transparency = 1
    handle.Anchored = false
    handle.CanCollide = false
    handle.Massless = true
    handle.Parent = cropTool

    -- Ağaçta görünen meyve modelinin BİREBİR AYNISINI al
    local fruitVisual = FruitVisualHelper.getFruitModel(canonical, weightScale, variant, true)

    -- Meyve modellerinin pivotu tabanındadır (tarlada toprağa oturması için). Elde
    -- taşınırken taban değil, meyvenin ORTASI ele gelmeli; yoksa meyve elin yukarısında
    -- havada durur. Kutusunun merkezini bulup ele hizalıyoruz, birazcık da aşağı.
    do
        local boxCF, boxSize = fruitVisual:GetBoundingBox()
        local pivot = fruitVisual:GetPivot()
        local centreOffset = pivot:PointToObjectSpace(boxCF.Position)
        local target = handle.CFrame * CFrame.new(0, -boxSize.Y * 0.18, 0)
        fruitVisual:PivotTo(target * CFrame.new(-centreOffset))
    end

    -- Modelin tüm parçalarını Handle parçasına rigid weld et
    for _, part in ipairs(fruitVisual:GetDescendants()) do
        if part:IsA("BasePart") then
            -- Tool'un tek bir Handle'ı olmalı; modelden gelen aynı adlı parça kavrama
            -- noktasını şaşırtıyor.
            if part.Name == "Handle" then
                part.Name = "FruitRoot"
            end
            part.Anchored = false
            part.CanCollide = false
            part.Massless = true

            local weld = Instance.new("WeldConstraint")
            weld.Part0 = handle
            weld.Part1 = part
            weld.Parent = part

            part.Parent = cropTool
        end
    end

    fruitVisual:Destroy()

    return cropTool
end

-- -------------------------------------------------------------
-- 6. Ağaca Meyve Bağlayıcı
--    (Ağacın Fruit_Spawn noktasına veya dallarına 1-e-1 aynı modeli takar)
-- -------------------------------------------------------------
function FruitVisualHelper.attachFruitToTree(plantModel, cleanSeed, scale, variant)
    local canonical = FruitVisualHelper.getCanonicalPlantName(cleanSeed)
    local fruit = FruitVisualHelper.getFruitModel(canonical, scale or 1.0, variant or "Normal", false)
    fruit.Name = "Fruit"

    -- Ağaç üzerinde meyve spawn noktası ara
    local spawnPart = nil
    local fruitSpawnFolder = plantModel:FindFirstChild("Fruit_Spawn", true)
    if fruitSpawnFolder then
        spawnPart = fruitSpawnFolder:FindFirstChildWhichIsA("BasePart", true)
    end

    if not spawnPart then
        spawnPart = plantModel:FindFirstChild("FruitPoint", true) or plantModel:FindFirstChild("FruitPart", true)
    end

    if spawnPart and fruit.PrimaryPart then
        fruit:PivotTo(spawnPart.CFrame)
    elseif fruit.PrimaryPart then
        local cf, sz = plantModel:GetBoundingBox()
        fruit:PivotTo(CFrame.new(cf.Position.X + (math.random(-10, 10) * 0.08), cf.Position.Y + (sz.Y * 0.32), cf.Position.Z + (math.random(-10, 10) * 0.08)))
    end

    fruit.Parent = plantModel
    return fruit
end

return FruitVisualHelper
