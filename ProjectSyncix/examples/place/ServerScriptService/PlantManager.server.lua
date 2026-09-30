-- PlantManager.server.lua
-- 15 Bitki Türü, Tip A (Kalıcı Ağaç - Regrow & Ağaçta Büyüyen Meyve) & Tip B (Tükenen - 3-5 Adet)
-- Kişisel Bahçe Güvenliği (Başkasının arazisine ekim ve hırsızlık engelleme) & Ağaç Gövdeleri Dokunulabilir (CanCollide = true)

local Players = game:GetService("Players")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local ServerStorage = game:GetService("ServerStorage")
local Workspace = game:GetService("Workspace")
local TweenService = game:GetService("TweenService")
local Debris = game:GetService("Debris")
local CollectionService = game:GetService("CollectionService")

-- RemoteEvents & Data
local GameEvents = ReplicatedStorage:WaitForChild("GameEvents", 15)
local plantRemoteEvent = GameEvents:WaitForChild("Plant_RE", 15)
local devGiveSeed = GameEvents:WaitForChild("DevGiveSeed", 15)

local seedDataModule = ReplicatedStorage:WaitForChild("Data", 15):WaitForChild("SeedData", 15)
local seedData = require(seedDataModule)

local seedsFolder = ReplicatedStorage:WaitForChild("Seeds", 15)
local fruitVisualHelperModule = ReplicatedStorage:WaitForChild("FruitVisualHelper", 15)
local FruitVisualHelper = require(fruitVisualHelperModule)
local gamePassServiceModule = ReplicatedStorage:WaitForChild("GamePassService", 10)
local GamePassService = require(gamePassServiceModule)
local collectablesFolder = ServerStorage:FindFirstChild("Collectables")
local fruitSpawnFolder = ReplicatedStorage:FindFirstChild("Fruit_Spawn")

-- -------------------------------------------------------------
-- 0. Kanonik Bitki Adı Çözümleyici (Büyük/küçük harf ve boşluk hatalarını engeller)
-- -------------------------------------------------------------
local function getCanonicalPlantName(raw)
    return FruitVisualHelper.getCanonicalPlantName(raw)
end

-- -------------------------------------------------------------
-- 1. 15 Bitki İçin Otantik, Canlı & Dokunulabilir 3D Model Fabrikası
-- -------------------------------------------------------------
local function buildProceduralPlant(seedName)
    local canonical = getCanonicalPlantName(seedName)
    seedName = canonical

    local m = Instance.new("Model")
    m.Name = canonical .. "_Plant"

    -- Helper to make parts cleanly
    local function makePart(parent, name, shape, size, color, mat, cf, canCollide, transp)
        local p = Instance.new("Part")
        p.Name = name or "Part"
        if shape then p.Shape = shape end
        p.Size = size or Vector3.new(1, 1, 1)
        p.Color = color or Color3.fromRGB(150, 150, 150)
        p.Material = mat or Enum.Material.Plastic
        p.Anchored = true
        p.CanCollide = (canCollide == true)
        p.Transparency = transp or 0
        if cf then p.CFrame = cf end
        p.Parent = parent
        return p
    end

    -- =========================================================
    -- 1. CARROT (Havuç - Tip B)
    -- =========================================================
    if seedName == "Carrot" then
        local mound = makePart(m, "Mound", Enum.PartType.Cylinder, Vector3.new(0.35, 2.2, 2.2), Color3.fromRGB(75, 45, 25), Enum.Material.Slate, CFrame.Angles(0, 0, math.rad(90)), false)
        m.PrimaryPart = mound

        local root = makePart(m, "Fruit", Enum.PartType.Cylinder, Vector3.new(1.6, 0.9, 0.9), Color3.fromRGB(245, 115, 15), Enum.Material.SmoothPlastic, mound.CFrame * CFrame.Angles(0, 0, math.rad(90)) * CFrame.new(0, 0.5, 0), false)

        local crown = makePart(m, "Stem", Enum.PartType.Block, Vector3.new(0.4, 0.6, 0.4), Color3.fromRGB(60, 160, 45), Enum.Material.Grass, root.CFrame * CFrame.new(0, 0.8, 0), false)

        for i = 1, 5 do
            local ang = math.rad(i * 72 + 15)
            makePart(m, "Foliage", Enum.PartType.Block, Vector3.new(0.25, 1.6, 0.7), Color3.fromRGB(50, 175, 45), Enum.Material.Grass, crown.CFrame * CFrame.Angles(0, ang, math.rad(28)) * CFrame.new(0, 0.8, 0.45), false)
        end

        local down = Instance.new("NumberValue", m)
        down.Name = "Plant_Down"
        down.Value = 0.5

    -- =========================================================
    -- 2. POTATO (Patates - Tip B)
    -- =========================================================
    elseif seedName == "Potato" then
        local mound = makePart(m, "Mound", Enum.PartType.Cylinder, Vector3.new(0.4, 2.4, 2.4), Color3.fromRGB(70, 48, 28), Enum.Material.Slate, CFrame.Angles(0, 0, math.rad(90)), false)
        m.PrimaryPart = mound

        local stem = makePart(m, "Stem", Enum.PartType.Block, Vector3.new(0.5, 1.8, 0.5), Color3.fromRGB(65, 140, 45), Enum.Material.Grass, mound.CFrame * CFrame.new(0, 0.9, 0), false)

        for i = 1, 3 do
            local ang = math.rad(i * 120)
            local branch = makePart(m, "StemBranch", Enum.PartType.Block, Vector3.new(0.3, 1.4, 0.3), Color3.fromRGB(60, 135, 40), Enum.Material.Grass, stem.CFrame * CFrame.Angles(0, ang, math.rad(30)) * CFrame.new(0, 0.7, 0.4), false)
            makePart(m, "Leaves", Enum.PartType.Block, Vector3.new(1.2, 0.25, 1.2), Color3.fromRGB(52, 160, 38), Enum.Material.Grass, branch.CFrame * CFrame.new(0, 0.7, 0), false)
        end

        makePart(m, "Flower", Enum.PartType.Ball, Vector3.new(0.5, 0.5, 0.5), Color3.fromRGB(255, 255, 230), Enum.Material.SmoothPlastic, stem.CFrame * CFrame.new(0, 1.4, 0), false)

        local fruitModel = Instance.new("Model", m)
        fruitModel.Name = "Fruit"
        for i = 1, 3 do
            local ang = math.rad(i * 120 + 30)
            local tub = makePart(fruitModel, "Tuber_" .. i, Enum.PartType.Ball, Vector3.new(1.1, 0.85, 1.3), Color3.fromRGB(155, 115, 68), Enum.Material.Slate, mound.CFrame * CFrame.Angles(0, ang, 0) * CFrame.new(0.55, 0.2, 0), false)
            if i == 1 then fruitModel.PrimaryPart = tub end
        end

        local down = Instance.new("NumberValue", m)
        down.Name = "Plant_Down"
        down.Value = 0.55

    -- =========================================================
    -- 3. STRAWBERRY (Çilek - Tip B)
    -- =========================================================
    elseif seedName == "Strawberry" then
        local stem = makePart(m, "Stem", Enum.PartType.Block, Vector3.new(0.4, 1.0, 0.4), Color3.fromRGB(55, 145, 40), Enum.Material.Grass, nil, false)
        m.PrimaryPart = stem

        for i = 1, 5 do
            local ang = math.rad(i * 72)
            makePart(m, "Leaves", Enum.PartType.Ball, Vector3.new(1.6, 0.6, 1.6), Color3.fromRGB(45, 142, 35), Enum.Material.Grass, stem.CFrame * CFrame.Angles(0, ang, math.rad(15)) * CFrame.new(0.7, 0.35, 0), false)
        end

        for i = 1, 2 do
            local ang = math.rad(i * 180 + 40)
            makePart(m, "Blossom", Enum.PartType.Ball, Vector3.new(0.45, 0.45, 0.45), Color3.fromRGB(255, 255, 240), Enum.Material.SmoothPlastic, stem.CFrame * CFrame.Angles(0, ang, 0) * CFrame.new(0.6, 0.7, 0), false)
        end

        local fruitModel = Instance.new("Model", m)
        fruitModel.Name = "Fruit"

        local mainBerry = makePart(fruitModel, "MainBerry", Enum.PartType.Ball, Vector3.new(1.3, 1.5, 1.3), Color3.fromRGB(225, 25, 35), Enum.Material.SmoothPlastic, stem.CFrame * CFrame.new(0.65, 0.4, 0.4), false)
        fruitModel.PrimaryPart = mainBerry

        makePart(fruitModel, "Calyx", Enum.PartType.Block, Vector3.new(0.7, 0.15, 0.7), Color3.fromRGB(40, 160, 40), Enum.Material.Grass, mainBerry.CFrame * CFrame.new(0, 0.75, 0), false)

        local down = Instance.new("NumberValue", m)
        down.Name = "Plant_Down"
        down.Value = 0.3

    -- =========================================================
    -- 4. BLUEBERRY (Yaban Mersini - Tip B)
    -- =========================================================
    elseif seedName == "Blueberry" then
        local stem = makePart(m, "Stem", Enum.PartType.Block, Vector3.new(0.5, 2.0, 0.5), Color3.fromRGB(85, 65, 45), Enum.Material.Wood, nil, false)
        m.PrimaryPart = stem

        for i = 1, 4 do
            local ang = math.rad(i * 90)
            local twig = makePart(m, "Twig", Enum.PartType.Block, Vector3.new(0.35, 1.5, 0.35), Color3.fromRGB(80, 60, 40), Enum.Material.Wood, stem.CFrame * CFrame.Angles(0, ang, math.rad(25)) * CFrame.new(0, 0.8, 0.4), false)
            makePart(m, "LeafCloud", Enum.PartType.Ball, Vector3.new(1.9, 1.5, 1.9), Color3.fromRGB(35, 120, 48), Enum.Material.Grass, twig.CFrame * CFrame.new(0, 0.9, 0), false)
        end

        local fruitModel = Instance.new("Model", m)
        fruitModel.Name = "Fruit"

        local mainBerry = makePart(fruitModel, "MainBerry", Enum.PartType.Ball, Vector3.new(1.1, 1.1, 1.1), Color3.fromRGB(38, 70, 180), Enum.Material.SmoothPlastic, stem.CFrame * CFrame.new(0.75, 1.3, 0.5), false)
        fruitModel.PrimaryPart = mainBerry

        for i = 1, 3 do
            local ang = math.rad(i * 120)
            makePart(fruitModel, "Berry_" .. i, Enum.PartType.Ball, Vector3.new(0.85, 0.85, 0.85), Color3.fromRGB(32, 60, 160), Enum.Material.SmoothPlastic, mainBerry.CFrame * CFrame.Angles(0, ang, 0) * CFrame.new(0.5, -0.2, 0), false)
        end

        local down = Instance.new("NumberValue", m)
        down.Name = "Plant_Down"
        down.Value = 0.35

    -- =========================================================
    -- 5. APPLE TREE (Elma Ağacı - Tip A Kalıcı Ağaç)
    -- =========================================================
    elseif seedName == "Apple Tree" or seedName == "Apple" then
        local lowerTrunk = makePart(m, "Trunk", Enum.PartType.Block, Vector3.new(1.9, 4.0, 1.9), Color3.fromRGB(90, 58, 35), Enum.Material.Wood, nil, true)
        m.PrimaryPart = lowerTrunk

        for i = 1, 4 do
            local ang = math.rad(i * 90)
            makePart(m, "Root_" .. i, Enum.PartType.Wedge, Vector3.new(0.8, 1.2, 1.6), Color3.fromRGB(82, 52, 30), Enum.Material.Wood, lowerTrunk.CFrame * CFrame.Angles(0, ang, 0) * CFrame.new(0, -1.4, 1.35) * CFrame.Angles(0, math.rad(180), 0), true)
        end

        local upperTrunk = makePart(m, "UpperTrunk", Enum.PartType.Block, Vector3.new(1.6, 3.5, 1.6), Color3.fromRGB(88, 55, 32), Enum.Material.Wood, lowerTrunk.CFrame * CFrame.new(0, 3.2, 0) * CFrame.Angles(math.rad(4), 0, math.rad(-3)), true)

        local br1 = makePart(m, "Branch_1", Enum.PartType.Block, Vector3.new(0.9, 3.0, 0.9), Color3.fromRGB(84, 52, 30), Enum.Material.Wood, upperTrunk.CFrame * CFrame.Angles(math.rad(35), math.rad(45), math.rad(15)) * CFrame.new(0, 1.4, 0), false)
        local br2 = makePart(m, "Branch_2", Enum.PartType.Block, Vector3.new(0.9, 2.8, 0.9), Color3.fromRGB(84, 52, 30), Enum.Material.Wood, upperTrunk.CFrame * CFrame.Angles(math.rad(-30), math.rad(135), math.rad(-20)) * CFrame.new(0, 1.3, 0), false)
        local br3 = makePart(m, "Branch_3", Enum.PartType.Block, Vector3.new(0.8, 2.5, 0.8), Color3.fromRGB(84, 52, 30), Enum.Material.Wood, upperTrunk.CFrame * CFrame.Angles(math.rad(15), math.rad(-80), math.rad(10)) * CFrame.new(0, 1.2, 0), false)

        makePart(m, "CanopyCenter", Enum.PartType.Ball, Vector3.new(5.6, 4.4, 5.6), Color3.fromRGB(38, 120, 35), Enum.Material.Grass, upperTrunk.CFrame * CFrame.new(0, 3.2, 0), false)
        makePart(m, "CanopyPuff1", Enum.PartType.Ball, Vector3.new(4.2, 3.5, 4.2), Color3.fromRGB(55, 145, 45), Enum.Material.Grass, br1.CFrame * CFrame.new(0, 1.8, 0), false)
        makePart(m, "CanopyPuff2", Enum.PartType.Ball, Vector3.new(4.0, 3.4, 4.0), Color3.fromRGB(32, 105, 30), Enum.Material.Grass, br2.CFrame * CFrame.new(0, 1.7, 0), false)
        makePart(m, "CanopyPuff3", Enum.PartType.Ball, Vector3.new(3.8, 3.2, 3.8), Color3.fromRGB(48, 135, 40), Enum.Material.Grass, br3.CFrame * CFrame.new(0, 1.6, 0), false)
        makePart(m, "CanopyTop", Enum.PartType.Ball, Vector3.new(3.6, 3.0, 3.6), Color3.fromRGB(62, 155, 50), Enum.Material.Grass, upperTrunk.CFrame * CFrame.new(0, 5.0, 0), false)

        for i = 1, 3 do
            local ang = math.rad(i * 120 + 25)
            makePart(m, "DecoApple_" .. i, Enum.PartType.Ball, Vector3.new(0.7, 0.7, 0.7), Color3.fromRGB(210, 25, 30), Enum.Material.SmoothPlastic, upperTrunk.CFrame * CFrame.Angles(0, ang, 0) * CFrame.new(2.4, 2.8, 0), false)
        end

        local fruitModel = Instance.new("Model", m)
        fruitModel.Name = "Fruit"

        local appleBody = makePart(fruitModel, "FruitPart", Enum.PartType.Ball, Vector3.new(1.6, 1.5, 1.6), Color3.fromRGB(220, 20, 25), Enum.Material.SmoothPlastic, br1.CFrame * CFrame.new(0.4, 0.5, 1.4), false)
        fruitModel.PrimaryPart = appleBody

        makePart(fruitModel, "FruitStem", Enum.PartType.Cylinder, Vector3.new(0.5, 0.18, 0.18), Color3.fromRGB(68, 42, 22), Enum.Material.Wood, appleBody.CFrame * CFrame.Angles(0, 0, math.rad(90)) * CFrame.new(0, 0.85, 0), false)
        makePart(fruitModel, "FruitLeaf", Enum.PartType.Block, Vector3.new(0.4, 0.1, 0.65), Color3.fromRGB(45, 140, 30), Enum.Material.Grass, appleBody.CFrame * CFrame.new(0.2, 0.9, 0) * CFrame.Angles(0, math.rad(30), math.rad(20)), false)

        local down = Instance.new("NumberValue", m)
        down.Name = "Plant_Down"
        down.Value = 0.5

    -- =========================================================
    -- 6. MANGO TREE (Mango Ağacı - Tip A Kalıcı Ağaç)
    -- =========================================================
    elseif seedName == "Mango Tree" or seedName == "Mango" then
        local trunk = makePart(m, "Trunk", Enum.PartType.Block, Vector3.new(2.2, 5.0, 2.2), Color3.fromRGB(88, 55, 32), Enum.Material.Wood, nil, true)
        m.PrimaryPart = trunk

        for i = 1, 4 do
            local ang = math.rad(i * 90 + 45)
            makePart(m, "Root_" .. i, Enum.PartType.Wedge, Vector3.new(1.0, 1.5, 1.8), Color3.fromRGB(80, 48, 28), Enum.Material.Wood, trunk.CFrame * CFrame.Angles(0, ang, 0) * CFrame.new(0, -1.75, 1.55) * CFrame.Angles(0, math.rad(180), 0), true)
        end

        local upperTrunk = makePart(m, "UpperTrunk", Enum.PartType.Block, Vector3.new(1.8, 3.8, 1.8), Color3.fromRGB(84, 52, 30), Enum.Material.Wood, trunk.CFrame * CFrame.new(0, 3.8, 0), true)

        local limb1 = makePart(m, "Limb_1", Enum.PartType.Block, Vector3.new(1.1, 3.5, 1.1), Color3.fromRGB(80, 48, 28), Enum.Material.Wood, upperTrunk.CFrame * CFrame.Angles(math.rad(40), math.rad(30), 0) * CFrame.new(0, 1.6, 0), false)
        local limb2 = makePart(m, "Limb_2", Enum.PartType.Block, Vector3.new(1.1, 3.5, 1.1), Color3.fromRGB(80, 48, 28), Enum.Material.Wood, upperTrunk.CFrame * CFrame.Angles(math.rad(-35), math.rad(150), 0) * CFrame.new(0, 1.6, 0), false)
        local limb3 = makePart(m, "Limb_3", Enum.PartType.Block, Vector3.new(1.0, 3.2, 1.0), Color3.fromRGB(80, 48, 28), Enum.Material.Wood, upperTrunk.CFrame * CFrame.Angles(math.rad(10), math.rad(-90), math.rad(35)) * CFrame.new(0, 1.5, 0), false)

        makePart(m, "CanopyBase", Enum.PartType.Ball, Vector3.new(7.5, 4.5, 7.5), Color3.fromRGB(25, 95, 30), Enum.Material.Grass, upperTrunk.CFrame * CFrame.new(0, 3.6, 0), false)
        makePart(m, "CanopyPuff1", Enum.PartType.Ball, Vector3.new(5.2, 3.8, 5.2), Color3.fromRGB(38, 120, 38), Enum.Material.Grass, limb1.CFrame * CFrame.new(0, 2.0, 0), false)
        makePart(m, "CanopyPuff2", Enum.PartType.Ball, Vector3.new(5.0, 3.8, 5.0), Color3.fromRGB(22, 88, 26), Enum.Material.Grass, limb2.CFrame * CFrame.new(0, 2.0, 0), false)
        makePart(m, "CanopyPuff3", Enum.PartType.Ball, Vector3.new(4.8, 3.6, 4.8), Color3.fromRGB(35, 115, 35), Enum.Material.Grass, limb3.CFrame * CFrame.new(0, 1.8, 0), false)
        makePart(m, "CanopyCrown", Enum.PartType.Ball, Vector3.new(4.5, 3.2, 4.5), Color3.fromRGB(48, 135, 45), Enum.Material.Grass, upperTrunk.CFrame * CFrame.new(0, 5.6, 0), false)

        local fruitModel = Instance.new("Model", m)
        fruitModel.Name = "Fruit"

        local mangoBody = makePart(fruitModel, "MangoBody", Enum.PartType.Ball, Vector3.new(1.4, 2.1, 1.3), Color3.fromRGB(255, 175, 35), Enum.Material.SmoothPlastic, limb1.CFrame * CFrame.new(0.5, 0.4, 1.5), false)
        fruitModel.PrimaryPart = mangoBody

        makePart(fruitModel, "MangoCheek", Enum.PartType.Ball, Vector3.new(1.2, 1.6, 0.75), Color3.fromRGB(225, 65, 40), Enum.Material.SmoothPlastic, mangoBody.CFrame * CFrame.new(0.25, 0.1, 0.35), false)

        makePart(fruitModel, "MangoStalk", Enum.PartType.Cylinder, Vector3.new(0.8, 0.18, 0.18), Color3.fromRGB(80, 110, 45), Enum.Material.Wood, mangoBody.CFrame * CFrame.Angles(0, 0, math.rad(90)) * CFrame.new(0, 1.2, 0), false)

        makePart(fruitModel, "MangoLeaf", Enum.PartType.Block, Vector3.new(0.5, 0.1, 0.9), Color3.fromRGB(30, 110, 35), Enum.Material.Grass, mangoBody.CFrame * CFrame.new(0.3, 1.3, 0) * CFrame.Angles(0, math.rad(45), math.rad(15)), false)

        local down = Instance.new("NumberValue", m)
        down.Name = "Plant_Down"
        down.Value = 0.5

    -- =========================================================
    -- 7. FROSTLEAF (Buz Yaprağı - Tip B)
    -- =========================================================
    elseif seedName == "Frostleaf" then
        local snowBase = makePart(m, "SnowBase", Enum.PartType.Ball, Vector3.new(3.0, 0.8, 3.0), Color3.fromRGB(225, 242, 255), Enum.Material.Ice, nil, false)
        m.PrimaryPart = snowBase

        local core = makePart(m, "CoreIce", Enum.PartType.Cylinder, Vector3.new(1.8, 0.8, 0.8), Color3.fromRGB(180, 225, 255), Enum.Material.Glass, snowBase.CFrame * CFrame.Angles(0, 0, math.rad(90)) * CFrame.new(0, 0.6, 0), false, 0.2)

        for i = 1, 6 do
            local ang = math.rad(i * 60)
            makePart(m, "Fern_" .. i, Enum.PartType.Wedge, Vector3.new(0.4, 2.4, 0.9), Color3.fromRGB(165, 220, 255), Enum.Material.Glass, core.CFrame * CFrame.Angles(0, ang, math.rad(35)) * CFrame.new(0, 0.9, 0.8), false, 0.25)
        end

        local fruitModel = Instance.new("Model", m)
        fruitModel.Name = "Fruit"

        local frostOrb = makePart(fruitModel, "FrostOrb", Enum.PartType.Ball, Vector3.new(1.4, 1.4, 1.4), Color3.fromRGB(130, 210, 255), Enum.Material.Neon, core.CFrame * CFrame.new(0, 1.1, 0), false)
        fruitModel.PrimaryPart = frostOrb

        local sp = Instance.new("Sparkles", frostOrb)
        sp.SparkleColor = Color3.fromRGB(180, 240, 255)

        local down = Instance.new("NumberValue", m)
        down.Name = "Plant_Down"
        down.Value = 0.3

    -- =========================================================
    -- 8. CHERRY TREE (Kiraz Ağacı / Sakura - Tip A Kalıcı Ağaç)
    -- =========================================================
    elseif seedName == "Cherry Tree" or seedName == "Cherry" then
        local trunk = makePart(m, "Trunk", Enum.PartType.Block, Vector3.new(2.2, 5.5, 2.2), Color3.fromRGB(58, 38, 28), Enum.Material.Wood, nil, true)
        m.PrimaryPart = trunk

        -- 4 Geniş Payanda Kök (Toprağa sıkı tutunan organik kökler)
        for i = 1, 4 do
            local ang = math.rad(i * 90 + 25)
            makePart(m, "Root_" .. i, Enum.PartType.Wedge, Vector3.new(1.1, 1.8, 2.2), Color3.fromRGB(52, 34, 25), Enum.Material.Wood, trunk.CFrame * CFrame.Angles(0, ang, 0) * CFrame.new(0, -1.9, 1.8) * CFrame.Angles(0, math.rad(180), 0), true)
        end

        -- Kıvrımlı Üst Gövde
        local upperTrunk = makePart(m, "UpperTrunk", Enum.PartType.Block, Vector3.new(1.8, 4.0, 1.8), Color3.fromRGB(54, 35, 26), Enum.Material.Wood, trunk.CFrame * CFrame.new(0, 4.2, 0) * CFrame.Angles(math.rad(8), 0, math.rad(6)), true)

        -- 3 Yöne Yayılan Organik Dallar
        local limb1 = makePart(m, "Limb_1", Enum.PartType.Block, Vector3.new(1.1, 3.6, 1.1), Color3.fromRGB(50, 32, 24), Enum.Material.Wood, upperTrunk.CFrame * CFrame.Angles(math.rad(32), math.rad(45), 0) * CFrame.new(0, 1.8, 0), true)
        local limb2 = makePart(m, "Limb_2", Enum.PartType.Block, Vector3.new(1.1, 3.4, 1.1), Color3.fromRGB(50, 32, 24), Enum.Material.Wood, upperTrunk.CFrame * CFrame.Angles(math.rad(-28), math.rad(140), 0) * CFrame.new(0, 1.7, 0), true)
        local limb3 = makePart(m, "Limb_3", Enum.PartType.Block, Vector3.new(0.9, 3.0, 0.9), Color3.fromRGB(50, 32, 24), Enum.Material.Wood, upperTrunk.CFrame * CFrame.Angles(math.rad(15), math.rad(-75), 0) * CFrame.new(0, 1.5, 0), true)

        -- Çok Katmanlı Pastel Pembe Sakura Çiçek Bulutları
        local blossomCenter = makePart(m, "BlossomCenter", Enum.PartType.Ball, Vector3.new(7.0, 5.2, 7.0), Color3.fromRGB(255, 185, 210), Enum.Material.Grass, upperTrunk.CFrame * CFrame.new(0, 4.0, 0), false)
        makePart(m, "BlossomPuff1", Enum.PartType.Ball, Vector3.new(5.2, 4.0, 5.2), Color3.fromRGB(255, 160, 190), Enum.Material.Grass, limb1.CFrame * CFrame.new(0, 2.0, 0), false)
        makePart(m, "BlossomPuff2", Enum.PartType.Ball, Vector3.new(4.8, 3.8, 4.8), Color3.fromRGB(245, 140, 175), Enum.Material.Grass, limb2.CFrame * CFrame.new(0, 1.9, 0), false)
        makePart(m, "BlossomPuff3", Enum.PartType.Ball, Vector3.new(4.2, 3.5, 4.2), Color3.fromRGB(255, 175, 200), Enum.Material.Grass, limb3.CFrame * CFrame.new(0, 1.6, 0), false)
        makePart(m, "BlossomTop", Enum.PartType.Ball, Vector3.new(4.5, 3.4, 4.5), Color3.fromRGB(255, 210, 230), Enum.Material.Grass, upperTrunk.CFrame * CFrame.new(0, 6.0, 0), false)

        -- Dökülen Sakura Yaprakları Efekti (ParticleEmitter)
        local petalEmitter = Instance.new("ParticleEmitter")
        petalEmitter.Name = "SakuraPetals"
        petalEmitter.Texture = "rbxassetid://243098098"
        petalEmitter.Color = ColorSequence.new(Color3.fromRGB(255, 185, 210), Color3.fromRGB(255, 140, 175))
        petalEmitter.Rate = 5
        petalEmitter.Speed = NumberRange.new(1, 3)
        petalEmitter.Lifetime = NumberRange.new(3, 5)
        petalEmitter.SpreadAngle = Vector2.new(45, 45)
        petalEmitter.Acceleration = Vector3.new(0, -1.5, 0)
        petalEmitter.Parent = blossomCenter

        -- Ağaçta Asılı Kiraz Meyvesi (FruitVisualHelper ile BİREBİR AYNI model)
        local fruitModel = FruitVisualHelper.getFruitModel("Cherry Tree", 1.0, "Normal", false)
        fruitModel.Name = "Fruit"
        if fruitModel.PrimaryPart then
            fruitModel:PivotTo(limb1.CFrame * CFrame.new(0.5, 0.4, 1.4))
        end
        fruitModel.Parent = m

        local down = Instance.new("NumberValue", m)
        down.Name = "Plant_Down"
        down.Value = 0.5

    -- =========================================================
    -- 9. BANANA TREE (Muz Ağacı - Tip A Kalıcı Ağaç)
    -- =========================================================
    elseif seedName == "Banana Tree" or seedName == "Banana" then
        local trunk = makePart(m, "Trunk", Enum.PartType.Block, Vector3.new(1.9, 6.5, 1.9), Color3.fromRGB(120, 145, 55), Enum.Material.SmoothPlastic, nil, true)
        m.PrimaryPart = trunk

        for i = 1, 3 do
            makePart(m, "Ring_" .. i, Enum.PartType.Cylinder, Vector3.new(0.2, 2.1, 2.1), Color3.fromRGB(95, 120, 42), Enum.Material.SmoothPlastic, trunk.CFrame * CFrame.Angles(0, 0, math.rad(90)) * CFrame.new(0, (i - 2) * 1.8, 0), false)
        end

        for i = 1, 7 do
            local ang = math.rad(i * (360 / 7))
            local frond = makePart(m, "Frond_" .. i, Enum.PartType.Block, Vector3.new(1.8, 0.3, 4.8), Color3.fromRGB(42, 138, 38), Enum.Material.Grass, trunk.CFrame * CFrame.new(0, 3.2, 0) * CFrame.Angles(0, ang, 0) * CFrame.Angles(math.rad(25), 0, 0) * CFrame.new(0, 0, 2.2), false)
        end

        local fruitModel = Instance.new("Model", m)
        fruitModel.Name = "Fruit"

        local stalk = makePart(fruitModel, "BananaStalk", Enum.PartType.Cylinder, Vector3.new(1.8, 0.35, 0.35), Color3.fromRGB(80, 110, 45), Enum.Material.Wood, trunk.CFrame * CFrame.new(1.3, 2.4, 0) * CFrame.Angles(0, 0, math.rad(15)), false)
        fruitModel.PrimaryPart = stalk

        makePart(fruitModel, "BananaBell", Enum.PartType.Ball, Vector3.new(1.0, 1.6, 1.0), Color3.fromRGB(115, 30, 65), Enum.Material.SmoothPlastic, stalk.CFrame * CFrame.new(0, -1.2, 0), false)

        for i = 1, 6 do
            local ang = math.rad(i * 60)
            makePart(fruitModel, "Banana_" .. i, Enum.PartType.Cylinder, Vector3.new(1.5, 0.45, 0.45), Color3.fromRGB(255, 215, 30), Enum.Material.SmoothPlastic, stalk.CFrame * CFrame.new(0, -0.4, 0) * CFrame.Angles(0, ang, math.rad(45)) * CFrame.new(0, 0, 0.6), false)
        end

        local down = Instance.new("NumberValue", m)
        down.Name = "Plant_Down"
        down.Value = 0.5

    -- =========================================================
    -- 10. DRAGONFRUIT BUSH (Ejder Meyvesi - Tip B)
    -- =========================================================
    elseif seedName == "Dragonfruit Bush" or seedName == "Dragon Fruit" or seedName == "Dragonfruit" then
        local stem = makePart(m, "Stem", Enum.PartType.Block, Vector3.new(1.0, 2.6, 1.0), Color3.fromRGB(50, 140, 60), Enum.Material.SmoothPlastic, nil, false)
        m.PrimaryPart = stem

        for i = 1, 4 do
            local ang = math.rad(i * 90 + 20)
            local pad = makePart(m, "CactusPad_" .. i, Enum.PartType.Block, Vector3.new(0.7, 2.2, 0.7), Color3.fromRGB(45, 130, 55), Enum.Material.SmoothPlastic, stem.CFrame * CFrame.Angles(0, ang, math.rad(30)) * CFrame.new(0, 1.0, 0.8), false)
            makePart(m, "Rib_" .. i, Enum.PartType.Block, Vector3.new(0.2, 2.0, 1.1), Color3.fromRGB(60, 155, 70), Enum.Material.SmoothPlastic, pad.CFrame, false)
        end

        local fruitModel = Instance.new("Model", m)
        fruitModel.Name = "Fruit"

        local mainFruit = makePart(fruitModel, "MainFruit", Enum.PartType.Ball, Vector3.new(1.5, 1.9, 1.5), Color3.fromRGB(240, 30, 120), Enum.Material.SmoothPlastic, stem.CFrame * CFrame.new(0.8, 1.8, 0.6), false)
        fruitModel.PrimaryPart = mainFruit

        for i = 1, 5 do
            local ang = math.rad(i * 72)
            makePart(fruitModel, "Bract_" .. i, Enum.PartType.Wedge, Vector3.new(0.35, 0.7, 0.5), Color3.fromRGB(90, 195, 40), Enum.Material.SmoothPlastic, mainFruit.CFrame * CFrame.Angles(0, ang, math.rad(25)) * CFrame.new(0, 0.4, 0.75), false)
        end

        local down = Instance.new("NumberValue", m)
        down.Name = "Plant_Down"
        down.Value = 0.4

    -- =========================================================
    -- 11. STARFRUIT PLANT (Yıldız Meyvesi - Tip A Kalıcı Ağaç)
    -- =========================================================
    elseif seedName == "Starfruit Plant" or seedName == "Starfruit" then
        local trunk = makePart(m, "Trunk", Enum.PartType.Block, Vector3.new(1.6, 5.0, 1.6), Color3.fromRGB(95, 70, 45), Enum.Material.Wood, nil, true)
        m.PrimaryPart = trunk

        local upperTrunk = makePart(m, "UpperTrunk", Enum.PartType.Block, Vector3.new(1.3, 3.2, 1.3), Color3.fromRGB(90, 65, 40), Enum.Material.Wood, trunk.CFrame * CFrame.new(0, 3.5, 0), true)

        local br1 = makePart(m, "Branch_1", Enum.PartType.Block, Vector3.new(0.8, 2.6, 0.8), Color3.fromRGB(85, 60, 38), Enum.Material.Wood, upperTrunk.CFrame * CFrame.Angles(math.rad(35), math.rad(45), 0) * CFrame.new(0, 1.2, 0), false)
        local br2 = makePart(m, "Branch_2", Enum.PartType.Block, Vector3.new(0.8, 2.6, 0.8), Color3.fromRGB(85, 60, 38), Enum.Material.Wood, upperTrunk.CFrame * CFrame.Angles(math.rad(-30), math.rad(135), 0) * CFrame.new(0, 1.2, 0), false)

        makePart(m, "Canopy1", Enum.PartType.Ball, Vector3.new(5.0, 3.8, 5.0), Color3.fromRGB(60, 155, 48), Enum.Material.Grass, upperTrunk.CFrame * CFrame.new(0, 2.8, 0), false)
        makePart(m, "Canopy2", Enum.PartType.Ball, Vector3.new(3.8, 3.0, 3.8), Color3.fromRGB(72, 170, 58), Enum.Material.Grass, br1.CFrame * CFrame.new(0, 1.4, 0), false)
        makePart(m, "Canopy3", Enum.PartType.Ball, Vector3.new(3.6, 2.8, 3.6), Color3.fromRGB(52, 142, 42), Enum.Material.Grass, br2.CFrame * CFrame.new(0, 1.4, 0), false)

        local fruitModel = Instance.new("Model", m)
        fruitModel.Name = "Fruit"

        local core = makePart(fruitModel, "StarCore", Enum.PartType.Cylinder, Vector3.new(1.8, 0.8, 0.8), Color3.fromRGB(255, 215, 25), Enum.Material.SmoothPlastic, br1.CFrame * CFrame.Angles(0, 0, math.rad(90)) * CFrame.new(0, 0.4, 1.2), false)
        fruitModel.PrimaryPart = core

        for i = 1, 5 do
            local ang = math.rad(i * 72)
            makePart(fruitModel, "Fin_" .. i, Enum.PartType.Wedge, Vector3.new(0.25, 1.6, 0.75), Color3.fromRGB(255, 230, 50), Enum.Material.SmoothPlastic, core.CFrame * CFrame.Angles(ang, 0, 0) * CFrame.new(0, 0, 0.45), false)
        end

        local down = Instance.new("NumberValue", m)
        down.Name = "Plant_Down"
        down.Value = 0.5

    -- =========================================================
    -- 12. COCONUT PALM (Hindistan Cevizi - Tip A Kalıcı Ağaç)
    -- =========================================================
    elseif seedName == "Coconut Palm" or seedName == "Coconut" or seedName == "Palm" then
        local trunk1 = makePart(m, "Trunk", Enum.PartType.Block, Vector3.new(1.9, 4.0, 1.9), Color3.fromRGB(115, 85, 55), Enum.Material.Wood, nil, true)
        m.PrimaryPart = trunk1

        for i = 1, 4 do
            local ang = math.rad(i * 90)
            makePart(m, "Root_" .. i, Enum.PartType.Wedge, Vector3.new(0.8, 1.0, 1.5), Color3.fromRGB(105, 75, 48), Enum.Material.Wood, trunk1.CFrame * CFrame.Angles(0, ang, 0) * CFrame.new(0, -1.4, 1.3) * CFrame.Angles(0, math.rad(180), 0), true)
        end

        local trunk2 = makePart(m, "TrunkMid", Enum.PartType.Block, Vector3.new(1.7, 3.8, 1.7), Color3.fromRGB(112, 82, 52), Enum.Material.Wood, trunk1.CFrame * CFrame.new(0, 3.4, 0.2) * CFrame.Angles(math.rad(6), 0, 0), true)
        local trunk3 = makePart(m, "TrunkTop", Enum.PartType.Block, Vector3.new(1.5, 3.5, 1.5), Color3.fromRGB(108, 78, 48), Enum.Material.Wood, trunk2.CFrame * CFrame.new(0, 3.2, 0.3) * CFrame.Angles(math.rad(8), 0, 0), true)

        for i = 1, 8 do
            local ang = math.rad(i * 45)
            makePart(m, "Frond_" .. i, Enum.PartType.Block, Vector3.new(1.6, 0.25, 5.2), Color3.fromRGB(38, 135, 40), Enum.Material.Grass, trunk3.CFrame * CFrame.new(0, 1.8, 0) * CFrame.Angles(0, ang, 0) * CFrame.Angles(math.rad(28), 0, 0) * CFrame.new(0, 0, 2.4), false)
        end

        local fruitModel = Instance.new("Model", m)
        fruitModel.Name = "Fruit"

        local c1 = makePart(fruitModel, "Coconut_1", Enum.PartType.Ball, Vector3.new(1.4, 1.6, 1.4), Color3.fromRGB(90, 55, 30), Enum.Material.Slate, trunk3.CFrame * CFrame.new(0.6, 1.3, 0.5), false)
        fruitModel.PrimaryPart = c1

        makePart(fruitModel, "Coconut_2", Enum.PartType.Ball, Vector3.new(1.3, 1.5, 1.3), Color3.fromRGB(85, 50, 28), Enum.Material.Slate, trunk3.CFrame * CFrame.new(-0.5, 1.2, 0.6), false)
        makePart(fruitModel, "Coconut_3", Enum.PartType.Ball, Vector3.new(1.35, 1.55, 1.35), Color3.fromRGB(95, 58, 32), Enum.Material.Slate, trunk3.CFrame * CFrame.new(0.1, 1.2, -0.7), false)

        local down = Instance.new("NumberValue", m)
        down.Name = "Plant_Down"
        down.Value = 0.5

    -- =========================================================
    -- 13. CRYSTAL SHRUB (Kristal Çalı - Tip B)
    -- =========================================================
    elseif seedName == "Crystal Shrub" then
        local rockBase = makePart(m, "RockBase", Enum.PartType.Ball, Vector3.new(3.2, 1.2, 3.2), Color3.fromRGB(65, 60, 75), Enum.Material.Slate, nil, false)
        m.PrimaryPart = rockBase

        for i = 1, 7 do
            local ang = math.rad(i * (360 / 7))
            makePart(m, "Spike_" .. i, Enum.PartType.Wedge, Vector3.new(0.6, 2.6, 0.9), Color3.fromRGB(185, 75, 255), Enum.Material.Neon, rockBase.CFrame * CFrame.Angles(0, ang, math.rad(28)) * CFrame.new(0, 1.0, 0.9), false, 0.15)
        end

        local fruitModel = Instance.new("Model", m)
        fruitModel.Name = "Fruit"

        local coreCrystal = makePart(fruitModel, "CoreCrystal", Enum.PartType.Block, Vector3.new(1.4, 2.2, 1.4), Color3.fromRGB(220, 110, 255), Enum.Material.Neon, rockBase.CFrame * CFrame.new(0, 1.4, 0) * CFrame.Angles(math.rad(15), math.rad(25), 0), false)
        fruitModel.PrimaryPart = coreCrystal

        local sp = Instance.new("Sparkles", coreCrystal)
        sp.SparkleColor = Color3.fromRGB(255, 150, 255)

        local down = Instance.new("NumberValue", m)
        down.Name = "Plant_Down"
        down.Value = 0.4

    -- =========================================================
    -- 14. NEBULA VINE (Nebula Sarmaşığı - Tip B)
    -- =========================================================
    elseif seedName == "Nebula Vine" then
        local stem = makePart(m, "Stem", Enum.PartType.Cylinder, Vector3.new(3.5, 0.6, 0.6), Color3.fromRGB(45, 20, 70), Enum.Material.SmoothPlastic, CFrame.Angles(0, 0, math.rad(90)) * CFrame.new(0, 1.7, 0), false)
        m.PrimaryPart = stem

        for i = 1, 5 do
            local h = i * 0.6
            local ang = math.rad(i * 72)
            makePart(m, "VineCoil_" .. i, Enum.PartType.Block, Vector3.new(0.5, 0.5, 1.6), Color3.fromRGB(75, 30, 110), Enum.Material.SmoothPlastic, stem.CFrame * CFrame.new(0, h - 1.5, 0) * CFrame.Angles(0, ang, math.rad(25)), false)
        end

        local fruitModel = Instance.new("Model", m)
        fruitModel.Name = "Fruit"

        local nebulaOrb = makePart(fruitModel, "NebulaOrb", Enum.PartType.Ball, Vector3.new(1.6, 1.6, 1.6), Color3.fromRGB(195, 50, 230), Enum.Material.Neon, stem.CFrame * CFrame.new(0, 1.2, 0), false)
        fruitModel.PrimaryPart = nebulaOrb

        local sp = Instance.new("Sparkles", nebulaOrb)
        sp.SparkleColor = Color3.fromRGB(140, 80, 255)

        local down = Instance.new("NumberValue", m)
        down.Name = "Plant_Down"
        down.Value = 0.4

    -- =========================================================
    -- 15. CELESTIAL TREE (Göksel Ağaç - Cosmic Yggdrasil)
    -- =========================================================
    elseif seedName == "Celestial Tree" or seedName == "Celestial" then
        local c_obsidian = Color3.fromRGB(15, 20, 18)
        local c_emerald = Color3.fromRGB(45, 255, 115)
        local c_mint = Color3.fromRGB(150, 255, 205)
        local c_lime = Color3.fromRGB(165, 255, 85)
        local c_nebula_deep = Color3.fromRGB(12, 32, 22)

        local trunk = makePart(m, "Trunk", Enum.PartType.Block, Vector3.new(2.4, 7.5, 2.4), c_obsidian, Enum.Material.Wood, nil, true)
        m.PrimaryPart = trunk

        -- Roots with glowing veins
        for i = 1, 6 do
            local ang = math.rad(i * 60 + 20)
            local rootPart = makePart(m, "Root_" .. i, Enum.PartType.Wedge, Vector3.new(1.1, 1.8, 2.2), c_obsidian, Enum.Material.Slate, trunk.CFrame * CFrame.Angles(0, ang, 0) * CFrame.new(0, -2.5, 1.9) * CFrame.Angles(0, math.rad(180), 0), true)
            makePart(m, "RootVein_" .. i, Enum.PartType.Block, Vector3.new(0.25, 1.6, 0.25), c_emerald, Enum.Material.Neon, rootPart.CFrame * CFrame.new(0, 0.2, 0), false)
        end

        -- Luminescent trunk energy conduits
        for i = 1, 3 do
            local ang = math.rad(i * 120)
            makePart(m, "TrunkConduit_" .. i, Enum.PartType.Block, Vector3.new(0.3, 6.8, 0.3), c_emerald, Enum.Material.Neon, trunk.CFrame * CFrame.Angles(0, ang, 0) * CFrame.new(0, 0, 1.25), false)
        end

        local limb1 = makePart(m, "Limb_1", Enum.PartType.Block, Vector3.new(1.2, 3.8, 1.2), c_obsidian, Enum.Material.Wood, trunk.CFrame * CFrame.new(0, 2.6, 0) * CFrame.Angles(math.rad(35), math.rad(45), 0) * CFrame.new(0, 1.6, 0), false)
        local limb2 = makePart(m, "Limb_2", Enum.PartType.Block, Vector3.new(1.2, 3.8, 1.2), c_obsidian, Enum.Material.Wood, trunk.CFrame * CFrame.new(0, 2.6, 0) * CFrame.Angles(math.rad(-30), math.rad(140), 0) * CFrame.new(0, 1.6, 0), false)
        local limb3 = makePart(m, "Limb_3", Enum.PartType.Block, Vector3.new(1.1, 3.5, 1.1), c_obsidian, Enum.Material.Wood, trunk.CFrame * CFrame.new(0, 2.6, 0) * CFrame.Angles(math.rad(15), math.rad(-80), 0) * CFrame.new(0, 1.5, 0), false)

        -- Volumetric Nebula Clouds & Glowing Aurora Crowns
        makePart(m, "NebulaDeep_1", Enum.PartType.Ball, Vector3.new(6.8, 4.8, 6.8), c_nebula_deep, Enum.Material.SmoothPlastic, trunk.CFrame * CFrame.new(0, 5.5, 0), false)
        makePart(m, "NebulaAurora_1", Enum.PartType.Ball, Vector3.new(5.6, 3.8, 5.6), c_emerald, Enum.Material.Neon, trunk.CFrame * CFrame.new(0, 5.9, 0), false)

        makePart(m, "NebulaDeep_2", Enum.PartType.Ball, Vector3.new(5.0, 3.8, 5.0), c_nebula_deep, Enum.Material.SmoothPlastic, limb1.CFrame * CFrame.new(0, 2.2, 0), false)
        makePart(m, "NebulaAurora_2", Enum.PartType.Ball, Vector3.new(4.2, 3.0, 4.2), c_lime, Enum.Material.Neon, limb1.CFrame * CFrame.new(0, 2.5, 0), false)

        makePart(m, "NebulaDeep_3", Enum.PartType.Ball, Vector3.new(4.8, 3.6, 4.8), c_nebula_deep, Enum.Material.SmoothPlastic, limb2.CFrame * CFrame.new(0, 2.2, 0), false)
        makePart(m, "NebulaAurora_3", Enum.PartType.Ball, Vector3.new(4.0, 2.8, 4.0), c_emerald, Enum.Material.Neon, limb2.CFrame * CFrame.new(0, 2.5, 0), false)

        makePart(m, "NebulaDeep_4", Enum.PartType.Ball, Vector3.new(4.6, 3.4, 4.6), c_nebula_deep, Enum.Material.SmoothPlastic, limb3.CFrame * CFrame.new(0, 2.0, 0), false)
        makePart(m, "NebulaAurora_4", Enum.PartType.Ball, Vector3.new(3.8, 2.6, 3.8), c_lime, Enum.Material.Neon, limb3.CFrame * CFrame.new(0, 2.3, 0), false)

        -- Constellation Stars
        makePart(m, "ApexStar", Enum.PartType.Ball, Vector3.new(0.9, 0.9, 0.9), Color3.fromRGB(255, 255, 255), Enum.Material.Neon, trunk.CFrame * CFrame.new(0, 7.8, 0), false)
        makePart(m, "ApexFlareX", Enum.PartType.Block, Vector3.new(1.8, 0.18, 0.18), c_mint, Enum.Material.Neon, trunk.CFrame * CFrame.new(0, 7.8, 0), false)
        makePart(m, "ApexFlareY", Enum.PartType.Block, Vector3.new(0.18, 1.8, 0.18), c_mint, Enum.Material.Neon, trunk.CFrame * CFrame.new(0, 7.8, 0), false)

        local fruitModel = Instance.new("Model", m)
        fruitModel.Name = "Fruit"

        -- Ringless Galaxy Portal Fruit
        local voidCore = makePart(fruitModel, "Portal_VoidCore", Enum.PartType.Ball, Vector3.new(1.3, 1.3, 1.3), c_obsidian, Enum.Material.SmoothPlastic, trunk.CFrame * CFrame.new(1.6, 4.2, 1.2), false)
        fruitModel.PrimaryPart = voidCore
        makePart(fruitModel, "Portal_Singularity", Enum.PartType.Ball, Vector3.new(0.6, 0.6, 0.6), c_mint, Enum.Material.Neon, voidCore.CFrame, false)
        makePart(fruitModel, "Portal_FlareX", Enum.PartType.Block, Vector3.new(1.5, 0.16, 0.16), c_emerald, Enum.Material.Neon, voidCore.CFrame, false)
        makePart(fruitModel, "Portal_FlareY", Enum.PartType.Block, Vector3.new(0.16, 1.5, 0.16), c_emerald, Enum.Material.Neon, voidCore.CFrame, false)

        local sp = Instance.new("Sparkles", voidCore)
        sp.SparkleColor = c_mint

        local down = Instance.new("NumberValue", m)
        down.Name = "Plant_Down"
        down.Value = 0.6

    -- Fallback generic
    else
        local stem = makePart(m, "Stem", Enum.PartType.Block, Vector3.new(0.5, 2.0, 0.5), Color3.fromRGB(70, 150, 50), Enum.Material.Grass, nil, false)
        m.PrimaryPart = stem
        makePart(m, "Fruit", Enum.PartType.Ball, Vector3.new(1.2, 1.2, 1.2), Color3.fromRGB(255, 180, 50), Enum.Material.SmoothPlastic, stem.CFrame * CFrame.new(0, 1.2, 0), false)
        local down = Instance.new("NumberValue", m)
        down.Name = "Plant_Down"
        down.Value = 0.5
    end

    return m
end

local COLLECTABLES_MAP = {
    ["Apple Tree"] = "Apple",
    ["Banana Tree"] = "Banana",
    ["Coconut Palm"] = "Coconut",
    ["Mango Tree"] = "Mango",
    ["Dragonfruit Bush"] = "Dragon Fruit",
    ["Celestial Tree"] = "Eden Fruit",
    ["Blueberry"] = "Blueberry",
    ["Strawberry"] = "Strawberry",
    ["Carrot"] = "Carrot",
}

local function getPlantModel(seedName)
    local canonical = getCanonicalPlantName(seedName)
    local cName = COLLECTABLES_MAP[canonical] or FruitVisualHelper.getFruitSpawnName(canonical)

    local plant = nil

    -- 0. Öncelik: ReplicatedStorage.PlantAssets.Plants (tools/gen_plants.py ile üretilen
    -- yeni low-poly modeller). Burada bir model varsa eski kaynaklara hiç bakılmaz;
    -- böylece 15 bitkinin görünümü tek bir yerden yönetilir.
    local plantAssets = ReplicatedStorage:FindFirstChild("PlantAssets")
    local assetPlants = plantAssets and plantAssets:FindFirstChild("Plants")
    local assetTemplate = assetPlants and assetPlants:FindFirstChild(canonical)
    if assetTemplate then
        plant = assetTemplate:Clone()
    end

    -- 1. Öncelik: game.Workspace["Grow A garden"] altındaki hazır modeller ve şablonlar
    local gagWorkspace = (not plant) and Workspace:FindFirstChild("Grow A garden")
    if gagWorkspace then
        local candidate = gagWorkspace:FindFirstChild(canonical)
            or gagWorkspace:FindFirstChild(cName)
            or (canonical == "Cherry Tree" and gagWorkspace:FindFirstChild("Cherry"))
            or (canonical == "Celestial Tree" and (gagWorkspace:FindFirstChild("eden fruit tree") or gagWorkspace:FindFirstChild("eden")))
            or (canonical == "Carrot" and gagWorkspace:FindFirstChild("Carrot"))

        if not candidate then
            for _, folderName in ipairs({"Trees", "Plants", "Flowers", "Decor", "MapDecorations", "Baseplate"}) do
                local subFolder = gagWorkspace:FindFirstChild(folderName)
                if subFolder then
                    local found = subFolder:FindFirstChild(canonical)
                        or subFolder:FindFirstChild(cName)
                        or (canonical == "Cherry Tree" and subFolder:FindFirstChild("Cherry"))
                        or (canonical == "Celestial Tree" and (subFolder:FindFirstChild("eden fruit tree") or subFolder:FindFirstChild("eden")))
                    if found then
                        candidate = found
                        break
                    end
                end
            end
        end

        if candidate and (candidate:IsA("Model") or candidate:IsA("BasePart")) then
            plant = candidate:Clone()
            if plant:IsA("BasePart") then
                local m = Instance.new("Model")
                plant.Parent = m
                m.PrimaryPart = plant
                plant = m
            end
        end
    end

    -- 2. Öncelik: ServerStorage.Collectables altındaki modeller (Apple, Banana, Coconut, Mango, Dragon Fruit, Eden Fruit, Blueberry, Strawberry, Carrot)
    if not plant and cName and collectablesFolder and collectablesFolder:FindFirstChild(cName) then
        local template = collectablesFolder:FindFirstChild(cName)
        plant = template:Clone()
    end

    -- 3. Öncelik: Yüksek kaliteli usulü (procedural) ağaç & bitki fabrikası
    if not plant then
        plant = buildProceduralPlant(canonical)
    end

    plant.Name = canonical .. "_Plant"

    -- PrimaryPart güvencesi (Modelin tabanına veya ana gövdesine hizala)
    if not plant.PrimaryPart then
        local p = plant:FindFirstChild("Root") or plant:FindFirstChild("1") or plant:FindFirstChild("Base") or plant:FindFirstChild("Trunk") or plant:FindFirstChildWhichIsA("BasePart", true)
        if p then plant.PrimaryPart = p end
    end

    -- Parçaların Anchored ve CanCollide ayarları (Trunk dokunulabilir/tırmanılabilir, yaprak/taç geçilebilir)
    for _, d in ipairs(plant:GetDescendants()) do
        if d:IsA("BasePart") then
            d.Anchored = true
            local dName = d.Name:lower()
            if dName:find("trunk") or dName:find("stem") or dName:find("wood") or dName:find("root") or d.Name == "1" or d.Name == "Base" then
                d.CanCollide = true
            else
                d.CanCollide = false
            end
        end
    end

    -- Meyve Modeli (Fruit) Entegrasyonu: Ağaçtaki meyve eldeki ile 1-e-1 BİREBİR AYNI olmalıdır
    local existingFruit = plant:FindFirstChild("Fruit", true)
    if not existingFruit then
        FruitVisualHelper.attachFruitToTree(plant, canonical, 1.0, "Normal")
    end

    return plant
end


-- -------------------------------------------------------------
-- 2. Oyuncunun Tarlasını Bulma & Bahçe Güvenlik Doğrulaması
-- -------------------------------------------------------------
local function getPlayerFarm(player)
    if not player then return nil end
    local farmFolder = Workspace:FindFirstChild("Farm")
    if not farmFolder then return nil end

    local assignedIsland = player:GetAttribute("AssignedIsland")
    if assignedIsland and farmFolder:FindFirstChild("Farm_" .. assignedIsland) then
        return farmFolder["Farm_" .. assignedIsland]
    end

    local farmName = player:GetAttribute("FarmName")
    if farmName and farmFolder:FindFirstChild(farmName) then
        return farmFolder[farmName]
    end

    for _, farm in ipairs(farmFolder:GetChildren()) do
        local data = farm:FindFirstChild("Important") and farm.Important:FindFirstChild("Data")
        if data and data:FindFirstChild("Owner") and data.Owner.Value == player.Name then
            return farm
        end
    end

    return nil
end

local function isPositionInFarm(pos, farm)
    if not farm or not pos then return false end

    local plantLocations = farm:FindFirstChild("Important") and farm.Important:FindFirstChild("Plant_Locations")
    if plantLocations then
        for _, plot in ipairs(plantLocations:GetChildren()) do
            if plot:IsA("BasePart") then
                local rel = plot.CFrame:PointToObjectSpace(pos)
                local half = plot.Size / 2 + Vector3.new(4, 15, 4)
                if math.abs(rel.X) <= half.X and math.abs(rel.Z) <= half.Z then
                    return true, plot
                end
            end
        end
    end

    local cf, sz = farm:GetBoundingBox()
    local rel = cf:PointToObjectSpace(pos)
    local half = sz / 2 + Vector3.new(3, 15, 3)
    if math.abs(rel.X) <= half.X and math.abs(rel.Z) <= half.Z then
        return true, nil
    end

    return false, nil
end

-- Meyve Büyütme / Gizleme Fonksiyonları (Ağaç Regrow İçin)
-- Meyve büyürken biçimi bozulmamalı: parçaları tek tek büyütmek aralarındaki mesafeyi
-- sabit bıraktığı için meyveyi dağıtır. Model:ScaleTo() hepsini pivota göre birlikte
-- ölçekler, yani meyve sadece büyür.
local function setFruitScale(plant, scale)
    local fruit = plant:FindFirstChild("Fruit", true)
    if not fruit then return end

    if fruit:IsA("BasePart") then
        local origSize = fruit:GetAttribute("OrigSize")
        if not origSize then
            origSize = fruit.Size
            fruit:SetAttribute("OrigSize", origSize)
        end
        fruit.Size = origSize * scale
        fruit.Transparency = 0
        return
    end

    if fruit:IsA("Model") then
        local base = fruit:GetAttribute("BaseScale")
        if not base then
            base = fruit:GetScale()
            fruit:SetAttribute("BaseScale", base)
        end
        pcall(function()
            fruit:ScaleTo(base * scale)
        end)
        for _, p in ipairs(fruit:GetDescendants()) do
            if p:IsA("BasePart") then
                p.Transparency = 0
            end
        end
    end
end

local function hideFruit(plant)
    local fruit = plant:FindFirstChild("Fruit", true)
    if not fruit then return end
    if fruit:IsA("BasePart") then
        fruit.Transparency = 1
    elseif fruit:IsA("Model") then
        for _, p in ipairs(fruit:GetDescendants()) do
            if p:IsA("BasePart") then
                p.Transparency = 1
            end
        end
    end
end

-- Ağırlık dağılımı: alt sınır var, üst sınır yok. Her katına çıkışta olasılık hızla
-- düşer, yani 2 kg sık, 10 kg seyrek, 40 kg efsane. "En ağır meyve" diye bir tavan
-- olmadığı için oyuncu her hasatta daha büyüğünü umabilir.
local WEIGHT_MIN = 0.8
local WEIGHT_TAIL = 2.2   -- büyüdükçe ne kadar hızlı seyrekleştiği; büyük = daha seyrek

local function rollWeight(minWeight)
    local floorW = minWeight or WEIGHT_MIN
    local u = math.random()
    if u < 1e-6 then u = 1e-6 end
    return math.round(floorW * (u ^ (-1 / WEIGHT_TAIL)) * 100) / 100
end

-- Envantere Gidecek Otantik Meyve / Bitki Eşyası (Tool) Üretici
-- Ağaçta yetişen meyvenin BİREBİR AYNISINI Tool içine weld eder (FruitVisualHelper)
local function createHarvestTool(displayName, cleanSeed, weight, variant, reward, index)
    return FruitVisualHelper.createCropTool(displayName, cleanSeed, weight, variant, reward, index)
end


local function consumeSeed(player, cleanSeed)
    if player:GetAttribute("InfiniteSeeds") == true then return end
    local char = player.Character
    local backpack = player:FindFirstChildOfClass("Backpack")

    local function normalize(str)
        if not str then return "" end
        local s = str:lower()
        s = s:gsub("%s*seed.*", "")
        s = s:gsub("%s*tree.*", "")
        s = s:gsub("%s*bush.*", "")
        s = s:gsub("%s*plant.*", "")
        s = s:gsub("%s*palm.*", "")
        s = s:gsub("%s*x%d+", "")
        s = s:gsub("%s+", "")
        return s
    end

    local targetNorm = normalize(cleanSeed)

    local function isMatchingTool(t)
        if not t or not t:IsA("Tool") then return false end
        local attrSeed = t:GetAttribute("Seed")
        if attrSeed and normalize(attrSeed) == targetNorm then return true end
        if normalize(t.Name) == targetNorm then return true end
        if t.Name:lower():find(targetNorm, 1, true) then return true end
        return false
    end

    local toolToConsume = nil
    -- 1. Öncelikle oyuncunun elinde (Character) tuttuğu tool'u kontrol et
    if char then
        local held = char:FindFirstChildOfClass("Tool")
        if held and isMatchingTool(held) then
            toolToConsume = held
        end
    end

    -- 2. Eğer elde bulunamadıysa Backpack'i tara
    if not toolToConsume and backpack then
        for _, t in ipairs(backpack:GetChildren()) do
            if isMatchingTool(t) then
                toolToConsume = t
                break
            end
        end
    end

    -- 3. Eğer elde hala yoksa, elde tutulan herhangi bir Seed tool'unu tüket
    if not toolToConsume and char then
        local held = char:FindFirstChildOfClass("Tool")
        if held and (held:GetAttribute("Seed") or held.Name:find("Seed")) then
            toolToConsume = held
        end
    end

    if toolToConsume then
        local qty = toolToConsume:GetAttribute("Quantity")
        if not qty or type(qty) ~= "number" then
            toolToConsume:Destroy()
            print(string.format("[PlantManager] 🗑️ %s tohumu kullanıldı ve envanterden silindi.", toolToConsume.Name))
        elseif qty > 1 then
            local newQty = qty - 1
            toolToConsume:SetAttribute("Quantity", newQty)
            local baseName = toolToConsume.Name:gsub("%s*X%d+", ""):gsub("%s*x%d+", "")
            toolToConsume.Name = string.format("%s X%d", baseName, newQty)
            toolToConsume.ToolTip = string.format("%s (x%d)", baseName, newQty)
            print(string.format("[PlantManager] 📉 %s tohumu azaldı: %d", toolToConsume.Name, newQty))
        else
            toolToConsume:Destroy()
            print(string.format("[PlantManager] 🗑️ %s tohumu son adedi kullanıldı ve silindi.", toolToConsume.Name))
        end
    else
        warn(string.format("[PlantManager] ⚠️ %s tohumu tüketilecek tool bulunamadı!", cleanSeed))
    end
end

-- -------------------------------------------------------------
-- 3. Tohum Ekiş & Hasat Döngüsü (Tip A & Tip B)
-- -------------------------------------------------------------
local playerPlantDebounce = {}

plantRemoteEvent.OnServerEvent:Connect(function(player, hitPosition, rawSeedName, hitPartName)
    local now = os.clock()
    if playerPlantDebounce[player] and now - playerPlantDebounce[player] < 0.18 then
        return
    end
    playerPlantDebounce[player] = now

    local success, err = pcall(function()
        if not player or not hitPosition or not rawSeedName then return end

        local cleanSeed = getCanonicalPlantName(rawSeedName)
        local info = seedData[cleanSeed] or seedData[cleanSeed .. " Tree"] or seedData[cleanSeed .. " Bush"] or seedData["Carrot"]
        local isTree = (info and info.Type == "Tree")
        local baseGrowthTime = (info and info.GrowthTime) or 12
        local regrowTime = (info and info.RegrowTime) or 80
        local minHarvest = (info and info.MinHarvest) or (isTree and 1 or 3)
        local maxHarvest = (info and info.MaxHarvest) or (isTree and 1 or 5)
        local basePrice = (info and info.Price) or 20

        local farm = getPlayerFarm(player)
        if not farm then
            warn(string.format("[PlantManager] ❌ %s için atanmış bir bahçe bulunamadı!", player.Name))
            return
        end

        -- KİŞİSEL BAHÇE KURALI: Yalnızca kendi bahçesine ekebilir!
        local inFarm = isPositionInFarm(hitPosition, farm)
        if not inFarm then
            warn(string.format("[PlantManager] ❌ %s yalnızca kendine atanan bahçeye ekim yapabilir!", player.Name))
            return
        end

        local important = farm:FindFirstChild("Important") or Instance.new("Folder", farm)
        important.Name = "Important"

        local plantsPhysical = important:FindFirstChild("Plants_Physical") or Instance.new("Folder", important)
        plantsPhysical.Name = "Plants_Physical"

        -- Y Yüksekliği Tespiti
        local groundY = 125.2
        local plantLocations = important:FindFirstChild("Plant_Locations")
        if plantLocations then
            local rayParams = RaycastParams.new()
            rayParams.FilterDescendantsInstances = {plantLocations}
            rayParams.FilterType = Enum.RaycastFilterType.Include
            local rayRes = Workspace:Raycast(Vector3.new(hitPosition.X, 150, hitPosition.Z), Vector3.new(0, -60, 0), rayParams)
            if rayRes then
                groundY = rayRes.Position.Y
            else
                local plotPart = plantLocations:FindFirstChild("Can_Plant1") or plantLocations:FindFirstChild("Can_Plant2")
                if plotPart and plotPart:IsA("BasePart") then
                    groundY = plotPart.Position.Y + (plotPart.Size.Y / 2)
                end
            end
        end

        local plant = getPlantModel(cleanSeed)
        if not plant then return end

        plant.Name = cleanSeed .. "_Planted"

        if not plant.PrimaryPart then
            plant.PrimaryPart = plant:FindFirstChildWhichIsA("BasePart", true)
        end

        -- Ağaç gövdeleri DOKUNULABİLİR (CanCollide = true), yaprak/meyveler geçilebilir
        for _, d in ipairs(plant:GetDescendants()) do
            if d:IsA("BasePart") then
                d.Anchored = true
                if isTree and (d.Name == "Trunk" or d.Name == "Stem" or d.Name == "Wood" or d.Name:lower():find("trunk")) then
                    d.CanCollide = true
                else
                    d.CanCollide = false
                end
            end
        end

        local downOffset = 0
        local pdVal = plant:FindFirstChild("Plant_Down")
        if pdVal and pdVal:IsA("ValueBase") then
            downOffset = tonumber(pdVal.Value) or 0
        end

        local yRot = math.rad(math.random(-180, 180))
        local plantCF = CFrame.new(hitPosition.X, groundY - downOffset + 0.05, hitPosition.Z) * CFrame.Angles(0, yRot, 0)
        plant:PivotTo(plantCF)

        -- Rastgele ağaç ve bitki boyutu varyasyonu (Kimisi küçük, kimisi orta, kimisi devasa)
        local targetScale = 1.0
        if isTree then
            -- Ağaçlar için: 0.75x ile 1.35x arası (Genç bodur ağaçtan ulu görkemli ağaca)
            targetScale = math.round((0.75 + (math.random() * 0.60)) * 100) / 100
        else
            -- Çalı ve küçük bitkiler için: 0.85x ile 1.25x arası
            targetScale = math.round((0.85 + (math.random() * 0.40)) * 100) / 100
        end
        plant:SetAttribute("PlantScale", targetScale)

        plant:SetAttribute("Owner", player.Name)
        plant:SetAttribute("FarmName", farm.Name)
        plant:SetAttribute("DisplayName", (info and info.DisplayName) or cleanSeed)
        plant:SetAttribute("SeedName", cleanSeed)
        plant:SetAttribute("SeedType", isTree and "Tree" or "Crop")
        plant:SetAttribute("GrowthStartTime", os.clock())
        plant:SetAttribute("GrowthDuration", baseGrowthTime)
        plant:SetAttribute("IsGrowing", true)
        plant:SetAttribute("IsRegrowing", false)
        plant:SetAttribute("IsMature", false)
        plant:SetAttribute("GrowthProgress", 0)
        plant.Parent = plantsPhysical

        consumeSeed(player, cleanSeed)

        -- Toprak VFX
        task.spawn(function()
            local vfxPart = Instance.new("Part")
            vfxPart.Size = Vector3.new(0.5, 0.5, 0.5)
            vfxPart.Position = Vector3.new(hitPosition.X, groundY + 0.1, hitPosition.Z)
            vfxPart.Anchored = true
            vfxPart.CanCollide = false
            vfxPart.Transparency = 1
            vfxPart.Parent = Workspace:FindFirstChild("Dirt_VFX") or Workspace

            local emitter = Instance.new("ParticleEmitter")
            emitter.Texture = "rbxassetid://243098098"
            emitter.Color = ColorSequence.new(Color3.fromRGB(120, 80, 45))
            emitter.Rate = 0
            emitter.Speed = NumberRange.new(4, 8)
            emitter.Lifetime = NumberRange.new(0.3, 0.6)
            emitter.SpreadAngle = Vector2.new(60, 60)
            emitter.Parent = vfxPart
            emitter:Emit(25)
            task.wait(1)
            vfxPart:Destroy()
        end)

        -- ---------------------------------------------------------
        -- Büyüme & Hasat Yönetici Döngüsü (Tip A Kalıcı & Tip B Tükenen)
        -- ---------------------------------------------------------
        task.spawn(function()
            local function growCycle(duration)
                pcall(function()
                    plant:ScaleTo(0.20 * targetScale)
                    plant:PivotTo(plantCF)
                end)

                local startTime = os.clock()
                local lastInsta = Workspace:GetAttribute("DevInstaGrowTrigger") or 0

                while true do
                    task.wait(0.25)
                    if not plant or not plant.Parent then return false end

                    local devMult = tonumber(Workspace:GetAttribute("DevGrowthMultiplier")) or 1.0
                    if devMult >= 100 then break end

                    local currentInsta = Workspace:GetAttribute("DevInstaGrowTrigger") or 0
                    if currentInsta > lastInsta then break end

                    local elapsed = os.clock() - startTime
                    local effectiveDur = math.max(0.2, duration / devMult)
                    local progress = math.clamp(elapsed / effectiveDur, 0, 1)
                    local currentScale = (0.20 + (0.80 * progress)) * targetScale
                    plant:SetAttribute("GrowthProgress", math.floor(progress * 100))

                    pcall(function()
                        plant:ScaleTo(currentScale)
                        plant:PivotTo(plantCF)
                    end)

                    if progress >= 1 then break end
                end

                pcall(function()
                    plant:ScaleTo(targetScale)
                    plant:PivotTo(plantCF)
                end)
                plant:SetAttribute("IsGrowing", false)
                plant:SetAttribute("IsMature", true)
                plant:SetAttribute("GrowthProgress", 100)
                return true
            end

            -- İlk Büyüme
            if not growCycle(baseGrowthTime) then return end

            -- ---------------------------------------------------------
            -- HASAT: HER MEYVE AYRI
            -- Ağacın tek bir düğmesi yok. Her meyve kendi modelidir, kendi kilosunu
            -- taşır, bilye boyundan olgun boyuna kendi büyür ve ancak olgunlaşınca
            -- kendi "Collect" düğmesi açılır. Oyuncu ağaca değil, istediği meyveye
            -- bakarak toplar; meyvesi olmayan ağaçta toplanacak bir şey yoktur.
            -- ---------------------------------------------------------
            -- Bir meyvenin tutamağı en büyük parçasıdır: gövdesi. İlk bulunan parça
            -- sap ya da yaprak olabilir; o zaman hem yazı hem uzaklık ölçümü meyvenin
            -- kenarına takılır.
            local function biggestPart(item)
                local best, bestVolume = nil, -1
                for _, p in ipairs(item:GetDescendants()) do
                    if p:IsA("BasePart") then
                        local v = p.Size.X * p.Size.Y * p.Size.Z
                        if v > bestVolume then
                            best, bestVolume = p, v
                        end
                    end
                end
                return best
            end

            local fruitContainer = plant:FindFirstChild("Fruit", true)
            local fruitModels = {}
            if fruitContainer then
                for _, child in ipairs(fruitContainer:GetChildren()) do
                    if child:IsA("Model") then
                        child.PrimaryPart = biggestPart(child)
                        table.insert(fruitModels, child)
                    end
                end
                -- Eski modellerde meyveler tek bir gövdedir: o zaman tek meyve sayılır.
                if #fruitModels == 0 and fruitContainer:IsA("Model") then
                    table.insert(fruitModels, fruitContainer)
                end
            end

            if #fruitModels == 0 then
                plant:SetAttribute("IsMature", true)
                return
            end

            -- Ağaç büyürken üstünde meyve olmaz; meyveler ağaç tam boyuna ulaştıktan
            -- sonra tek tek tomurcuktan belirir.
            for _, fruitModel in ipairs(fruitModels) do
                for _, p in ipairs(fruitModel:GetDescendants()) do
                    if p:IsA("BasePart") then
                        p.Transparency = 1
                        p.CanQuery = false
                    end
                end
            end

            local remaining = #fruitModels
            local displayNameBase = (info and info.DisplayName) or cleanSeed

            local function setFruitVisible(fruitModel, visible)
                for _, p in ipairs(fruitModel:GetDescendants()) do
                    if p:IsA("BasePart") then
                        p.Transparency = visible and 0 or 1
                        p.CanQuery = visible
                    end
                end
            end

            -- Meyvenin kendi merkezine göre ölçeklenmesi.
            -- Model:ScaleTo() burada işe yaramıyor: ağacın büyürken kullandığı ScaleTo
            -- ile iç içe geçiyor ve meyve kendi yerinde küçüleceğine ağacın kökündeki
            -- pivota doğru kayıyor (elmalar gövdenin dibinde havada asılı kalıyordu).
            -- Bunun yerine her parça, ağaç tam büyüdükten SONRA kendi boyutunu ve
            -- meyvenin merkezine olan uzaklığını bir kez hatırlıyor; ölçekleme artık
            -- bu iki sayının çarpımı, yani düz aritmetik.
            local function captureFruit(fruitModel)
                local anchor = fruitModel.PrimaryPart
                    or fruitModel:FindFirstChildWhichIsA("BasePart", true)
                if not anchor then return nil end
                local centre = anchor.Position
                fruitModel:SetAttribute("CentreX", centre.X)
                fruitModel:SetAttribute("CentreY", centre.Y)
                fruitModel:SetAttribute("CentreZ", centre.Z)
                for _, p in ipairs(fruitModel:GetDescendants()) do
                    if p:IsA("BasePart") then
                        p:SetAttribute("OrigSize", p.Size)
                        p:SetAttribute("OrigOffset", p.Position - centre)
                    end
                end
                return centre
            end

            -- Tacın küreleri: meyvenin yeniden doğabileceği yüzey. Modeli kuran üreteç
            -- de meyveleri bu kürelerin kabuğuna yerleştiriyor; burada aynı kural
            -- çalışma anında uygulanıyor.
            local canopyBalls = {}
            for _, p in ipairs(plant:GetDescendants()) do
                if p:IsA("Part") and p.Shape == Enum.PartType.Ball then
                    local name = p.Name:lower()
                    if name:find("canopy") or name:find("bush") or name:find("crown")
                        or name:find("leaf") or name:find("leaves") then
                        table.insert(canopyBalls, p)
                    end
                end
            end

            local function fruitCentreOf(fruitModel)
                local cx = fruitModel:GetAttribute("CentreX")
                if not cx then return nil end
                return Vector3.new(cx, fruitModel:GetAttribute("CentreY"),
                    fruitModel:GetAttribute("CentreZ"))
            end

            -- Yeni bir meyve yeri: bir küre seç, kabuğunda bir nokta al, yaprakların
            -- derinine düşerse ya da başka bir meyveye çok yakınsa yeniden dene.
            local function pickSpot(fruitModel, others)
                if #canopyBalls == 0 then return nil end
                for _ = 1, 30 do
                    local ball = canopyBalls[math.random(1, #canopyBalls)]
                    local radius = ball.Size.X / 2
                    local theta = math.random() * math.pi * 2
                    local height = -0.85 + math.random() * 1.3      -- aşağı doğru ağırlıklı
                    local horiz = math.sqrt(math.max(0, 1 - height * height))
                    local depth = 0.74 + math.random() * 0.26       -- kürenin kabuğu

                    local candidate = ball.Position + Vector3.new(
                        math.cos(theta) * horiz * radius * depth,
                        height * radius * depth * 0.85 - 0.3,
                        math.sin(theta) * horiz * radius * depth)

                    local ok = true
                    for _, other in ipairs(canopyBalls) do
                        if other ~= ball then
                            local d = (candidate - other.Position).Magnitude
                            if d < other.Size.X / 2 * 0.74 then
                                ok = false
                                break
                            end
                        end
                    end

                    if ok then
                        for _, other in ipairs(others) do
                            if other ~= fruitModel then
                                local centre = fruitCentreOf(other)
                                if centre and (candidate - centre).Magnitude < 1.4 then
                                    ok = false
                                    break
                                end
                            end
                        end
                    end

                    if ok then return candidate end
                end
                return nil
            end

            local function moveFruitTo(fruitModel, centre)
                if not centre then return end
                fruitModel:SetAttribute("CentreX", centre.X)
                fruitModel:SetAttribute("CentreY", centre.Y)
                fruitModel:SetAttribute("CentreZ", centre.Z)
            end

            local function scaleFruit(fruitModel, factor)
                local cx = fruitModel:GetAttribute("CentreX")
                if not cx then return end
                local centre = Vector3.new(cx, fruitModel:GetAttribute("CentreY"),
                    fruitModel:GetAttribute("CentreZ"))
                for _, p in ipairs(fruitModel:GetDescendants()) do
                    if p:IsA("BasePart") then
                        -- Ölçüsü kaydedilmemiş bir parça varsa (model sonradan
                        -- değiştiyse) burada kaydedilir; yoksa o parça tam boyunda
                        -- kalır ve meyvenin yanında yapayalnız durur.
                        local size = p:GetAttribute("OrigSize")
                        local offset = p:GetAttribute("OrigOffset")
                        if not (size and offset) then
                            size = p.Size / math.max(factor, 0.001)
                            offset = (p.Position - centre) / math.max(factor, 0.001)
                            p:SetAttribute("OrigSize", size)
                            p:SetAttribute("OrigOffset", offset)
                        end

                        -- Meyve ağacın parçasıdır: yere düşmez, çarpışmaz.
                        p.Anchored = true
                        p.CanCollide = false
                        p.Size = size * factor
                        p.CFrame = CFrame.new(centre + offset * factor) * (p.CFrame - p.CFrame.Position)
                    end
                end
            end

            -- Bir meyvenin ömrü: büyü, olgunlaş, toplanmayı bekle, (ağaçsa) baştan başla.
            local function runFruit(fruitModel, index)
                local firstRound = true
                while plant and plant.Parent do
                    -- İkinci ve sonraki meyveler aynı deliğe değil, tacın başka bir
                    -- yerine gelir; ama rastgelelik tacın kabuğuyla sınırlı, yani
                    -- havada ya da gövdenin dibinde bitmez.
                    if not firstRound then
                        moveFruitTo(fruitModel, pickSpot(fruitModel, fruitModels))
                    end
                    firstRound = false

                    local roll = math.random(1, 100)
                    local variant, variantMult = "Normal", 1.0
                    if roll <= 3 then variant, variantMult = "Rainbow", 5.0
                    elseif roll <= 12 then variant, variantMult = "Gold", 2.5 end

                    local weight = rollWeight()
                    local ripeScale = FruitVisualHelper.weightToScale(weight)
                    local growTime = isTree and regrowTime or math.max(4, baseGrowthTime * 0.35)

                    -- Bilye boyunda başla ve olgun boyuna kadar büyü. Önce küçült,
                    -- sonra göster: ters sırada meyve bir kare boyunca eski boyunda ve
                    -- eski yerinde görünüyor.
                    -- Ağaçta meyve uzaktan görülür, bilye boyu başlangıç iyi durur;
                    -- çilek ya da patateste aynı oran neredeyse görünmez olur, bitki
                    -- "boş yaprak" gibi görünür. Küçük bitkiler daha iri başlar.
                    local budScale = isTree and 0.12 or 0.33
                    scaleFruit(fruitModel, budScale * ripeScale)
                    setFruitVisible(fruitModel, true)
                    fruitModel:SetAttribute("Ripe", false)

                    local startTime = os.clock()
                    local lastInsta = Workspace:GetAttribute("DevInstaGrowTrigger") or 0
                    while plant and plant.Parent do
                        task.wait(0.25)
                        local devMult = tonumber(Workspace:GetAttribute("DevGrowthMultiplier")) or 1.0
                        local currentInsta = Workspace:GetAttribute("DevInstaGrowTrigger") or 0
                        if devMult >= 100 or currentInsta > lastInsta then break end

                        local effective = math.max(0.5, growTime / devMult)
                        local progress = math.clamp((os.clock() - startTime) / effective, 0, 1)
                        scaleFruit(fruitModel, (budScale + (1 - budScale) * progress) * ripeScale)
                        if progress >= 1 then break end
                    end
                    if not (plant and plant.Parent) then return end

                    scaleFruit(fruitModel, ripeScale)
                    fruitModel:SetAttribute("Ripe", true)

                    -- Olgunlaştı: artık toplanabilir. Düğme meyvenin kendi üstünde.
                    local displayName = displayNameBase
                    if variant ~= "Normal" then displayName = "✨ " .. variant .. " " .. displayName end

                    local anchorPart = fruitModel.PrimaryPart
                        or fruitModel:FindFirstChildWhichIsA("BasePart", true)
                    if not anchorPart then return end

                    local prompt = Instance.new("ProximityPrompt")
                    prompt.Name = "CollectPrompt"
                    prompt.ActionText = "Collect"
                    prompt.ObjectText = string.format("%s · %.1f kg", displayName, weight)
                    prompt.KeyboardKeyCode = Enum.KeyCode.E
                    -- Roblox'un kendi düğme arayüzü kapalı: yazıyı istemci meyvenin
                    -- üstüne kendi koyuyor (FruitFocus).
                    prompt.Style = Enum.ProximityPromptStyle.Custom
                    prompt.MaxActivationDistance = 12
                    prompt.HoldDuration = 0
                    prompt.RequiresLineOfSight = false
                    prompt.Parent = anchorPart

                    -- İstemci en yakın meyveyi bu etiketten bulup çevresini çizer.
                    CollectionService:AddTag(fruitModel, "HarvestFruit")

                    local collected = false
                    local connection
                    connection = prompt.Triggered:Connect(function(harvestingPlayer)
                        if collected or not harvestingPlayer then return end

                        local isThief = harvestingPlayer.Name ~= player.Name
                        if isThief and not GamePassService.canSteal(harvestingPlayer) then
                            GamePassService.promptStealPass(harvestingPlayer)
                            return
                        end

                        collected = true
                        CollectionService:RemoveTag(fruitModel, "HarvestFruit")
                        prompt:Destroy()
                        if connection then connection:Disconnect() end

                        if isThief then
                            GamePassService.notifySteal(harvestingPlayer, player, displayName, weight)
                        end

                        local reward = math.floor(basePrice * (weight / 1.5) * variantMult * 2.2)
                        if reward < 15 then reward = 15 end

                        local backpack = harvestingPlayer:FindFirstChildOfClass("Backpack")
                        if backpack then
                            local cropTool = createHarvestTool(displayName, cleanSeed, weight,
                                variant, reward, index)
                            cropTool.Parent = backpack
                        end

                        local sparkles = Instance.new("Sparkles")
                        sparkles.SparkleColor = isTree and Color3.fromRGB(255, 220, 60)
                            or Color3.fromRGB(100, 255, 100)
                        sparkles.Parent = anchorPart
                        Debris:AddItem(sparkles, 1.0)

                        setFruitVisible(fruitModel, false)
                        fruitModel:SetAttribute("Ripe", false)
                    end)

                    while not collected and plant and plant.Parent do
                        task.wait(0.2)
                    end
                    if not (plant and plant.Parent) then return end

                    if not isTree then
                        -- Tip B (çalı/çiçek): meyve geri gelmez. Hepsi toplanınca bitki
                        -- tükenir ve toprak boşalır.
                        remaining = remaining - 1
                        if remaining <= 0 then
                            task.wait(0.2)
                            if plant and plant.Parent then
                                plant:Destroy()
                            end
                        end
                        return
                    end
                    -- Tip A (ağaç): meyve yeniden büyümeye başlar, döngü baştan.
                end
            end

            for _, fruitModel in ipairs(fruitModels) do
                captureFruit(fruitModel)
            end

            for index, fruitModel in ipairs(fruitModels) do
                task.spawn(function()
                    -- Meyveler aynı anda olgunlaşmasın diye hafif kaydırma.
                    task.wait((index - 1) * 0.35)
                    runFruit(fruitModel, index)
                end)
            end
        end)
    end)

    if not success then
        warn("[PlantManager] Hata oluştu:", err)
    end
end)

-- -------------------------------------------------------------
-- 4. DevPanel / Admin Hızlı Tohum & Hızlandırma Eventleri
-- -------------------------------------------------------------
devGiveSeed.OnServerEvent:Connect(function(player, action, param)
    local backpack = player:FindFirstChildOfClass("Backpack")
    local starterGear = player:FindFirstChildOfClass("StarterGear")

    if action == "GROW_SPEED" then
        local mult = tonumber(param) or 1
        Workspace:SetAttribute("DevGrowthMultiplier", mult)
        print(string.format("[PlantManager] ⚡ Büyüme Hızı Çarpanı ayarlandı: %sx", tostring(mult)))

    elseif action == "INSTA_GROW" then
        local trig = (Workspace:GetAttribute("DevInstaGrowTrigger") or 0) + 1
        Workspace:SetAttribute("DevInstaGrowTrigger", trig)
        print("[PlantManager] 🌟 Tüm tarladaki bitkiler anında büyütüldü!")

    elseif action == "MONEY" then
        local leaderstats = player:FindFirstChild("leaderstats") or player:FindFirstChild("PlayerData")
        local sheckles = leaderstats and (leaderstats:FindFirstChild("Sheckles") or leaderstats:FindFirstChild("Para") or leaderstats:FindFirstChild("Coins"))
        if sheckles then
            sheckles.Value = sheckles.Value + 100000
        end

    elseif action == "ALL" then
        local ALL_15 = {
            "Carrot Seed", "Potato Seed", "Strawberry Seed", "Blueberry Seed",
            "Apple Tree Seed", "Mango Tree Seed", "Frostleaf Seed", "Cherry Tree Seed",
            "Banana Tree Seed", "Dragonfruit Bush Seed", "Starfruit Plant Seed",
            "Coconut Palm Seed", "Crystal Shrub Seed", "Nebula Vine Seed", "Celestial Tree Seed"
        }

        for _, seedName in ipairs(ALL_15) do
            local cleanName = seedName:gsub(" Seed", "")
            local tool = Instance.new("Tool")
            tool.Name = seedName .. " X40"
            tool:SetAttribute("Seed", cleanName)
            tool:SetAttribute("Quantity", 40)
            tool.ToolTip = string.format("%s (x40)", cleanName)

            local handle = Instance.new("Part")
            handle.Name = "Handle"
            handle.Size = Vector3.new(0.8, 0.8, 0.8)
            handle.Color = Color3.fromRGB(130, 85, 45)
            handle.Material = Enum.Material.Wood
            handle.CanCollide = false
            handle.Anchored = false
            handle.Parent = tool

            if backpack then tool:Clone().Parent = backpack end
            if starterGear then tool.Parent = starterGear end
        end

    elseif action == "SEED" then
        local seedName = tostring(param)
        local cleanName = seedName:gsub(" Seed", "")
        local tool = Instance.new("Tool")
        tool.Name = seedName .. " Seed X10"
        tool:SetAttribute("Seed", cleanName)
        tool:SetAttribute("Quantity", 10)
        tool.ToolTip = string.format("%s (x10)", cleanName)

        local handle = Instance.new("Part")
        handle.Name = "Handle"
        handle.Size = Vector3.new(0.8, 0.8, 0.8)
        handle.Color = Color3.fromRGB(130, 85, 45)
        handle.Material = Enum.Material.Wood
        handle.CanCollide = false
        handle.Anchored = false
        handle.Parent = tool

        if backpack then tool:Clone().Parent = backpack end
        if starterGear then tool.Parent = starterGear end
    end
end)

print("[PlantManager] ✅ 15 Bitki, Kalıcı Ağaçlar (Dokunulabilir Trunk & Ağaçta Büyüyen Meyve) ve Kişisel Bahçe İzolasyonu aktif!")
