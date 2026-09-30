local Workspace = game:GetService("Workspace")

local function ensureModel(parent, name)
    local existing = parent:FindFirstChild(name)
    if existing and existing:IsA("Model") then
        return existing
    end

    if existing ~= nil then
        existing:Destroy()
    end

    local model = Instance.new("Model")
    model.Name = name
    model.Parent = parent
    return model
end

local function makePart(parent, name, size, color, material, cframe)
    local part = Instance.new("Part")
    part.Name = name
    part.Size = size or Vector3.new(1, 1, 1)
    part.Color = color or Color3.fromRGB(255, 255, 255)
    part.Material = material or Enum.Material.SmoothPlastic

    if cframe then
        part.CFrame = cframe
    end

    part.Parent = parent
    return part
end

local function attachRoot(model, size, cframe)
    local root = makePart(model, "Root", size or Vector3.new(0.6, 0.1, 0.6), Color3.fromRGB(255, 255, 255), Enum.Material.SmoothPlastic, cframe or CFrame.new())
    root.Transparency = 1
    root.CanCollide = false
    model.PrimaryPart = root
    return root
end

local function getRightMostX()
    local maxX = -math.huge

    for _, descendant in ipairs(Workspace:GetDescendants()) do
        if descendant:IsA("BasePart") then
            local x = descendant.Position.X + descendant.Size.X * 0.5
            maxX = math.max(maxX, x)
        elseif descendant:IsA("Model") and descendant.PrimaryPart then
            local pos = descendant:GetPivot().Position
            local x = pos.X + descendant:GetExtentsSize().X * 0.5
            maxX = math.max(maxX, x)
        end
    end

    return maxX == -math.huge and 0 or maxX
end

local function buildAppleTree()
    local model = Instance.new("Model")
    model.Name = "Apple Tree"
    attachRoot(model, Vector3.new(0.75, 0.1, 0.75), CFrame.new())

    local trunk = makePart(model, "Trunk", Vector3.new(1.1, 4.0, 1.1), Color3.fromRGB(96, 62, 38), Enum.Material.Wood, CFrame.new(0, 2.0, 0))
    local branchA = makePart(model, "BranchA", Vector3.new(0.7, 2.6, 0.7), Color3.fromRGB(90, 58, 35), Enum.Material.Wood, trunk.CFrame * CFrame.Angles(0, math.rad(30), math.rad(20)) * CFrame.new(0.8, 1.4, 0.4))
    local branchB = makePart(model, "BranchB", Vector3.new(0.7, 2.6, 0.7), Color3.fromRGB(90, 58, 35), Enum.Material.Wood, trunk.CFrame * CFrame.Angles(0, math.rad(150), math.rad(-18)) * CFrame.new(-0.8, 1.4, -0.2))

    for i, color in ipairs({Color3.fromRGB(45, 158, 46), Color3.fromRGB(62, 175, 54), Color3.fromRGB(38, 139, 42)}) do
        local offset = CFrame.new((i % 2 == 0 and -1.2 or 1.2), 3.8 + i * 0.5, i * 0.3)
        makePart(model, "Canopy" .. i, Vector3.new(2.8, 1.8, 2.8), color, Enum.Material.Grass, trunk.CFrame * offset)
    end

    local fruitGroup = Instance.new("Model")
    fruitGroup.Name = "Fruit"
    fruitGroup.Parent = model

    for i = 1, 5 do
        local angle = math.rad((i - 1) * 72)
        local fruit = makePart(fruitGroup, "Apple" .. i, Vector3.new(0.8, 0.8, 0.8), Color3.fromRGB(227, 36, 42), Enum.Material.SmoothPlastic, trunk.CFrame * CFrame.Angles(0, angle, 0) * CFrame.new(1.8, 2.1, 0.7))
        local leaf = makePart(fruitGroup, "Leaf" .. i, Vector3.new(0.5, 0.12, 0.7), Color3.fromRGB(55, 165, 48), Enum.Material.Grass, fruit.CFrame * CFrame.new(0.15, 0.55, 0))
    end

    return model
end

local function buildBlueberryBush()
    local model = Instance.new("Model")
    model.Name = "Blueberry Bush"
    attachRoot(model, Vector3.new(0.7, 0.1, 0.7), CFrame.new())

    local trunk = makePart(model, "Trunk", Vector3.new(0.4, 1.8, 0.4), Color3.fromRGB(88, 66, 42), Enum.Material.Wood, CFrame.new(0, 1.1, 0))
    for i = 1, 4 do
        local angle = math.rad((i - 1) * 90)
        makePart(model, "Branch" .. i, Vector3.new(0.35, 1.4, 0.35), Color3.fromRGB(82, 60, 38), Enum.Material.Wood, trunk.CFrame * CFrame.Angles(0, angle, math.rad(28)) * CFrame.new(0, 0.8, 0.6))
        makePart(model, "LeafCluster" .. i, Vector3.new(1.8, 1.0, 1.8), Color3.fromRGB(42, 136, 48), Enum.Material.Grass, trunk.CFrame * CFrame.Angles(0, angle, 0) * CFrame.new(0.8, 1.5, 0.5))
    end

    local fruitGroup = Instance.new("Model")
    fruitGroup.Name = "Fruit"
    fruitGroup.Parent = model
    for i = 1, 6 do
        local angle = math.rad((i - 1) * 60)
        makePart(fruitGroup, "Berry" .. i, Vector3.new(0.8, 0.8, 0.8), Color3.fromRGB(52, 74, 190), Enum.Material.SmoothPlastic, trunk.CFrame * CFrame.Angles(0, angle, 0) * CFrame.new(0.9 + (i % 2) * 0.2, 1.5, 0.4 + (i % 3) * 0.2))
    end

    return model
end

local function buildCarrotPlant()
    local model = Instance.new("Model")
    model.Name = "Carrot Plant"
    attachRoot(model, Vector3.new(0.6, 0.1, 0.6), CFrame.new())

    local crown = makePart(model, "Crown", Vector3.new(2.2, 0.8, 2.2), Color3.fromRGB(107, 71, 42), Enum.Material.Slate, CFrame.new(0, 0.5, 0))
    local stem = makePart(model, "Stem", Vector3.new(0.35, 1.5, 0.35), Color3.fromRGB(63, 164, 53), Enum.Material.Grass, CFrame.new(0, 1.5, 0))
    for i = 1, 5 do
        local angle = math.rad((i - 1) * 72)
        makePart(model, "Leaf" .. i, Vector3.new(0.45, 1.5, 0.9), Color3.fromRGB(59, 172, 56), Enum.Material.Grass, stem.CFrame * CFrame.Angles(0, angle, math.rad(26)) * CFrame.new(0, 0.8, 0.4))
    end

    local rootFruit = makePart(model, "RootFruit", Vector3.new(1.0, 0.9, 1.0), Color3.fromRGB(236, 133, 43), Enum.Material.SmoothPlastic, stem.CFrame * CFrame.new(0, 0.7, 0.3))
    rootFruit.Shape = Enum.PartType.Cylinder
    rootFruit.CFrame = rootFruit.CFrame * CFrame.Angles(0, 0, math.rad(90))

    return model
end

local function buildBananaTree()
    local model = Instance.new("Model")
    model.Name = "Banana Tree"
    attachRoot(model, Vector3.new(0.75, 0.1, 0.75), CFrame.new())

    local trunk = makePart(model, "Trunk", Vector3.new(0.9, 3.5, 0.9), Color3.fromRGB(95, 68, 41), Enum.Material.Wood, CFrame.new(0, 1.8, 0))
    for i = 1, 3 do
        local angle = math.rad((i - 1) * 120)
        makePart(model, "Leaf" .. i, Vector3.new(0.5, 2.3, 1.6), Color3.fromRGB(112, 180, 60), Enum.Material.Grass, trunk.CFrame * CFrame.Angles(0, angle, math.rad(24)) * CFrame.new(0.8, 2.0, 0.2))
    end

    local fruitGroup = Instance.new("Model")
    fruitGroup.Name = "Fruit"
    fruitGroup.Parent = model
    for i = 1, 5 do
        local angle = math.rad((i - 1) * 45)
        local bunch = makePart(fruitGroup, "Bunch" .. i, Vector3.new(0.35, 1.0, 0.7), Color3.fromRGB(246, 212, 75), Enum.Material.SmoothPlastic, trunk.CFrame * CFrame.Angles(0, angle, 0) * CFrame.new(1.6, 2.1, 0.5))
        bunch.CFrame = bunch.CFrame * CFrame.Angles(0, 0, math.rad(90))
    end

    return model
end

local function buildCoconutPalm()
    local model = Instance.new("Model")
    model.Name = "Coconut Palm"
    attachRoot(model, Vector3.new(0.7, 0.1, 0.7), CFrame.new())

    local trunk = makePart(model, "Trunk", Vector3.new(0.8, 4.2, 0.8), Color3.fromRGB(112, 79, 44), Enum.Material.Wood, CFrame.new(0, 2.1, 0))
    for i = 1, 6 do
        local angle = math.rad((i - 1) * 60)
        makePart(model, "Leaf" .. i, Vector3.new(0.45, 2.2, 1.8), Color3.fromRGB(87, 178, 76), Enum.Material.Grass, trunk.CFrame * CFrame.Angles(0, angle, math.rad(32)) * CFrame.new(0.9, 2.0, 0.2))
    end

    local fruitGroup = Instance.new("Model")
    fruitGroup.Name = "Fruit"
    fruitGroup.Parent = model
    for i = 1, 3 do
        local angle = math.rad((i - 1) * 120)
        makePart(fruitGroup, "Coconut" .. i, Vector3.new(0.9, 0.9, 0.9), Color3.fromRGB(122, 79, 42), Enum.Material.SmoothPlastic, trunk.CFrame * CFrame.Angles(0, angle, 0) * CFrame.new(1.2, 1.1, 0.7))
    end

    return model
end

local function buildCherryTree()
    local model = Instance.new("Model")
    model.Name = "Cherry Tree"
    attachRoot(model, Vector3.new(0.75, 0.1, 0.75), CFrame.new())

    local trunk = makePart(model, "Trunk", Vector3.new(1.0, 3.4, 1.0), Color3.fromRGB(98, 68, 40), Enum.Material.Wood, CFrame.new(0, 1.7, 0))
    makePart(model, "BranchA", Vector3.new(0.6, 2.2, 0.6), Color3.fromRGB(90, 60, 35), Enum.Material.Wood, trunk.CFrame * CFrame.Angles(0, math.rad(25), math.rad(22)) * CFrame.new(0.7, 1.3, 0.2))
    makePart(model, "BranchB", Vector3.new(0.6, 2.0, 0.6), Color3.fromRGB(90, 60, 35), Enum.Material.Wood, trunk.CFrame * CFrame.Angles(0, math.rad(135), math.rad(-18)) * CFrame.new(-0.7, 1.2, -0.3))

    for i, color in ipairs({Color3.fromRGB(124, 206, 112), Color3.fromRGB(110, 195, 104), Color3.fromRGB(138, 212, 110)}) do
        local offset = CFrame.new((i % 2 == 0 and -1.1 or 1.1), 3.2 + i * 0.4, i * 0.3)
        makePart(model, "Canopy" .. i, Vector3.new(2.8, 2.0, 2.8), color, Enum.Material.Grass, trunk.CFrame * offset)
    end

    local fruitGroup = Instance.new("Model")
    fruitGroup.Name = "Fruit"
    fruitGroup.Parent = model
    for i = 1, 6 do
        local angle = math.rad((i - 1) * 60)
        makePart(fruitGroup, "Cherry" .. i, Vector3.new(0.7, 0.7, 0.7), Color3.fromRGB(220, 38, 46), Enum.Material.SmoothPlastic, trunk.CFrame * CFrame.Angles(0, angle, 0) * CFrame.new(1.5, 2.3, 0.6))
    end

    return model
end

local function buildFrostleaf()
    local model = Instance.new("Model")
    model.Name = "Frostleaf"
    attachRoot(model, Vector3.new(0.65, 0.1, 0.65), CFrame.new())

    local stem = makePart(model, "Stem", Vector3.new(0.45, 2.2, 0.45), Color3.fromRGB(101, 196, 214), Enum.Material.SmoothPlastic, CFrame.new(0, 1.4, 0))
    for i = 1, 3 do
        local angle = math.rad((i - 1) * 120)
        makePart(model, "Petal" .. i, Vector3.new(0.7, 2.0, 0.45), Color3.fromRGB(165, 228, 255), Enum.Material.Neon, stem.CFrame * CFrame.Angles(0, angle, math.rad(16)) * CFrame.new(0, 0.9, 0.8))
    end

    local fruitGroup = Instance.new("Model")
    fruitGroup.Name = "Fruit"
    fruitGroup.Parent = model
    for i = 1, 3 do
        local angle = math.rad((i - 1) * 120)
        makePart(fruitGroup, "Shard" .. i, Vector3.new(0.35, 1.0, 0.35), Color3.fromRGB(205, 238, 255), Enum.Material.Neon, stem.CFrame * CFrame.Angles(0, angle, 0) * CFrame.new(0.8, 0.8, 0.2))
    end

    return model
end

local function buildStrawberryPlant()
    local model = Instance.new("Model")
    model.Name = "Strawberry Plant"
    attachRoot(model, Vector3.new(0.65, 0.1, 0.65), CFrame.new())

    local stem = makePart(model, "Stem", Vector3.new(0.42, 1.25, 0.42), Color3.fromRGB(62, 150, 48), Enum.Material.Grass, CFrame.new(0, 1.2, 0))
    for i = 1, 5 do
        local angle = math.rad((i - 1) * 72)
        makePart(model, "Leaf" .. i, Vector3.new(1.5, 0.5, 1.5), Color3.fromRGB(48, 150, 44), Enum.Material.Grass, stem.CFrame * CFrame.Angles(0, angle, math.rad(16)) * CFrame.new(0.7, 0.5, 0))
    end

    local fruitGroup = Instance.new("Model")
    fruitGroup.Name = "Fruit"
    fruitGroup.Parent = model
    for i = 1, 3 do
        local offset = {
            CFrame.new(0.7, 1.0, 0.2),
            CFrame.new(-0.6, 0.8, 0.2),
            CFrame.new(0.2, 0.9, -0.8),
        }
        makePart(fruitGroup, "Berry" .. i, Vector3.new(1.0, 1.0, 1.0), Color3.fromRGB(220, 38, 48), Enum.Material.SmoothPlastic, offset[i])
    end

    return model
end

local function buildFruitItem(name, color)
    local model = Instance.new("Model")
    model.Name = name
    attachRoot(model, Vector3.new(0.5, 0.1, 0.5), CFrame.new())

    local body = makePart(model, "Body", Vector3.new(1.1, 1.1, 1.1), color, Enum.Material.SmoothPlastic, CFrame.new(0, 0.8, 0))

    if name == "Apple" then
        body.Shape = Enum.PartType.Ball
        makePart(model, "Stem", Vector3.new(0.12, 0.55, 0.12), Color3.fromRGB(80, 56, 26), Enum.Material.Wood, body.CFrame * CFrame.new(0, 0.7, 0))
        makePart(model, "Leaf", Vector3.new(0.5, 0.12, 0.75), Color3.fromRGB(44, 160, 42), Enum.Material.Grass, body.CFrame * CFrame.new(0.15, 0.9, 0))
    elseif name == "Banana" then
        body.Size = Vector3.new(1.4, 0.7, 0.6)
        body.CFrame = CFrame.new(0, 0.8, 0)
        makePart(model, "Tip", Vector3.new(0.24, 0.2, 0.24), Color3.fromRGB(246, 214, 82), Enum.Material.SmoothPlastic, body.CFrame * CFrame.new(0.55, 0, 0))
    elseif name == "Blueberry" then
        body.Size = Vector3.new(0.9, 0.9, 0.9)
        makePart(model, "Berry2", Vector3.new(0.7, 0.7, 0.7), Color3.fromRGB(62, 87, 210), Enum.Material.SmoothPlastic, body.CFrame * CFrame.new(-0.55, -0.1, 0.2))
    elseif name == "Carrot" then
        body.Shape = Enum.PartType.Cylinder
        body.Size = Vector3.new(1.1, 1.1, 1.1)
        body.CFrame = CFrame.new(0, 0.8, 0) * CFrame.Angles(0, 0, math.rad(90))
        makePart(model, "Top", Vector3.new(0.42, 0.18, 0.42), Color3.fromRGB(62, 175, 60), Enum.Material.Grass, body.CFrame * CFrame.new(0, 0.7, 0))
    elseif name == "Cherry" then
        body.Size = Vector3.new(0.8, 0.8, 0.8)
        makePart(model, "Stem", Vector3.new(0.08, 0.5, 0.08), Color3.fromRGB(68, 156, 52), Enum.Material.Grass, body.CFrame * CFrame.new(0, 0.6, 0))
    elseif name == "Coconut" then
        body.Shape = Enum.PartType.Ball
        body.Size = Vector3.new(1.0, 1.0, 1.0)
        body.CFrame = CFrame.new(0, 0.8, 0)
    elseif name == "Crystal" then
        body.Shape = Enum.PartType.Cylinder
        body.Size = Vector3.new(0.8, 1.2, 0.8)
        body.CFrame = CFrame.new(0, 0.9, 0) * CFrame.Angles(0, 0, math.rad(90))
    elseif name == "Dragonfruit" then
        body.Shape = Enum.PartType.Ball
        body.Size = Vector3.new(1.1, 1.1, 1.1)
        body.CFrame = CFrame.new(0, 0.8, 0)
    elseif name == "Mango" then
        body.Shape = Enum.PartType.Ball
        body.Size = Vector3.new(1.1, 1.1, 1.1)
        body.CFrame = CFrame.new(0, 0.8, 0)
    elseif name == "Potato" then
        body.Shape = Enum.PartType.Ball
        body.Size = Vector3.new(1.1, 0.9, 1.1)
        body.CFrame = CFrame.new(0, 0.8, 0)
    elseif name == "Starfruit" then
        body.Shape = Enum.PartType.Ball
        body.Size = Vector3.new(1.0, 1.0, 1.0)
        body.CFrame = CFrame.new(0, 0.8, 0)
    elseif name == "Strawberry" then
        body.Size = Vector3.new(1.1, 1.2, 1.1)
        makePart(model, "Leaf1", Vector3.new(0.6, 0.14, 0.7), Color3.fromRGB(50, 160, 44), Enum.Material.Grass, body.CFrame * CFrame.new(0.15, 0.8, 0))
    end

    return model
end

local function buildPlantSet(startX)
    local plants = {
        buildAppleTree,
        buildBlueberryBush,
        buildCarrotPlant,
        buildBananaTree,
        buildCoconutPalm,
        buildCherryTree,
        buildFrostleaf,
        buildStrawberryPlant,
    }

    local plantsFolder = ensureModel(Workspace, "Plants_New")
    local fruitsFolder = ensureModel(Workspace, "Fruits_New")

    local offsetZ = 0
    for i, plantBuilder in ipairs(plants) do
        local model = plantBuilder()
        model.Parent = plantsFolder
        local x = startX + ((i - 1) % 4) * 12
        local z = offsetZ + math.floor((i - 1) / 4) * 12
        model:PivotTo(CFrame.new(x, 0, z))
    end

    local fruits = {"Apple", "Banana", "Blueberry", "Carrot", "Cherry", "Coconut", "Crystal", "Dragonfruit", "Mango", "Potato", "Starfruit", "Strawberry"}
    local fruitColors = {
        Color3.fromRGB(229, 36, 42),
        Color3.fromRGB(247, 212, 80),
        Color3.fromRGB(54, 76, 200),
        Color3.fromRGB(242, 136, 46),
        Color3.fromRGB(220, 38, 46),
        Color3.fromRGB(124, 81, 44),
        Color3.fromRGB(130, 216, 255),
        Color3.fromRGB(218, 98, 144),
        Color3.fromRGB(243, 183, 62),
        Color3.fromRGB(164, 127, 90),
        Color3.fromRGB(255, 221, 109),
        Color3.fromRGB(221, 46, 58),
    }

    for i, fruitName in ipairs(fruits) do
        local fruit = buildFruitItem(fruitName, fruitColors[i])
        fruit.Parent = fruitsFolder
        local x = startX + 14 + ((i - 1) % 4) * 10
        local z = 8 + math.floor((i - 1) / 4) * 11
        fruit:PivotTo(CFrame.new(x, 0, z))
    end

    return plantsFolder, fruitsFolder
end

local function main()
    local rightX = getRightMostX()
    local startX = rightX + 16
    buildPlantSet(startX)
end

main()
