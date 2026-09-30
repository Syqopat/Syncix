-- PlantingController.client.lua
-- Grow A Garden 1-e-1 Kesintisiz Fare Tıklama ile Tohum Ekiş Kontrolcüsü

local Players = game:GetService("Players")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local UserInputService = game:GetService("UserInputService")
local Workspace = game:GetService("Workspace")
local SoundService = game:GetService("SoundService")

local localPlayer = Players.LocalPlayer
local mouse = localPlayer:GetMouse()

local gameEvents = ReplicatedStorage:WaitForChild("GameEvents", 15)
local plantRemoteEvent = gameEvents and gameEvents:WaitForChild("Plant_RE", 15)

local lastPlantTime = 0
local COOLDOWN = 0.2

local function getEquippedSeedTool()
    local char = localPlayer.Character
    if not char then return nil end
    local tool = char:FindFirstChildOfClass("Tool")
    if not tool then return nil end

    -- Hasat edilmiş ürün (Crop) tohum değildir!
    if tool:GetAttribute("Item_String") or tool:GetAttribute("Weight") then
        return nil
    end

    local seedName = tool:GetAttribute("Seed")
    if not seedName and tool.Name:find("Seed") then
        seedName = string.gsub(tool.Name, "%s*Seed.*", "")
        seedName = string.gsub(seedName, "%s*X%d+", "")
        seedName = string.gsub(seedName, "%s*x%d+", "")
    end

    if not seedName or seedName == "" then
        return nil
    end

    return tool, seedName
end

local function playPlantSound(pos)
    local s = Instance.new("Sound")
    s.SoundId = "rbxassetid://9114223175"
    s.Volume = 0.65
    s.PlaybackSpeed = 1.0 + math.random(-10, 10) / 100
    s.Parent = SoundService
    s:Play()
    game:GetService("Debris"):AddItem(s, 2)
end

local function isHitInMyFarm(targetPart, hitPosition)
    local farmFolder = Workspace:FindFirstChild("Farm")
    if not farmFolder then return true end

    local assigned = localPlayer:GetAttribute("AssignedIsland")
    local myFarm = nil
    if assigned and farmFolder:FindFirstChild("Farm_" .. assigned) then
        myFarm = farmFolder["Farm_" .. assigned]
    else
        for _, f in ipairs(farmFolder:GetChildren()) do
            local data = f:FindFirstChild("Important") and f.Important:FindFirstChild("Data")
            if data and data:FindFirstChild("Owner") and data.Owner.Value == localPlayer.Name then
                myFarm = f
                break
            end
        end
    end

    if not myFarm then return true end

    -- 1. Hedef parça doğrudan oyuncunun tarlasının içinde mi?
    if targetPart and targetPart:IsDescendantOf(myFarm) then
        return true
    end

    -- 2. Tıklanan nokta tarlanın toprak veya sınır alanı içinde mi?
    if hitPosition then
        local plantLocations = myFarm:FindFirstChild("Important") and myFarm.Important:FindFirstChild("Plant_Locations")
        if plantLocations then
            for _, plot in ipairs(plantLocations:GetChildren()) do
                if plot:IsA("BasePart") then
                    local rel = plot.CFrame:PointToObjectSpace(hitPosition)
                    local half = plot.Size / 2 + Vector3.new(4, 15, 4)
                    if math.abs(rel.X) <= half.X and math.abs(rel.Z) <= half.Z then
                        return true
                    end
                end
            end
        end

        local cf, sz = myFarm:GetBoundingBox()
        local rel = cf:PointToObjectSpace(hitPosition)
        local half = sz / 2 + Vector3.new(3, 15, 3)
        if math.abs(rel.X) <= half.X and math.abs(rel.Z) <= half.Z then
            return true
        end
    end

    return false
end

local function tryPlantAt(targetPart, hitPosition)
    local now = os.clock()
    if now - lastPlantTime < COOLDOWN then return end

    local tool, seedName = getEquippedSeedTool()
    if not tool or not seedName then return end

    if not hitPosition and mouse then
        hitPosition = mouse.Hit.Position
    end
    if not targetPart and mouse then
        targetPart = mouse.Target
    end
    if not hitPosition then return end

    -- KİŞİSEL BAHÇE KURALI: Başkasının bahçesine bitki dikilemez!
    if not isHitInMyFarm(targetPart, hitPosition) then
        return
    end

    lastPlantTime = now

    local partName = (targetPart and targetPart.Name) or "Can_Plant1"

    -- Anında lokal toprak parçacık efekti ve ses (Responsive Client Feedback)
    task.spawn(function()
        local groundY = (targetPart and targetPart:IsA("BasePart") and (targetPart.Position.Y + targetPart.Size.Y / 2)) or 125.2
        playPlantSound(hitPosition)

        local vfxPart = Instance.new("Part")
        vfxPart.Size = Vector3.new(0.5, 0.5, 0.5)
        vfxPart.Position = Vector3.new(hitPosition.X, groundY + 0.1, hitPosition.Z)
        vfxPart.Anchored = true
        vfxPart.CanCollide = false
        vfxPart.Transparency = 1
        vfxPart.Parent = Workspace

        local emitter = Instance.new("ParticleEmitter")
        emitter.Texture = "rbxassetid://243098098"
        emitter.Color = ColorSequence.new(Color3.fromRGB(130, 85, 45))
        emitter.Rate = 0
        emitter.Speed = NumberRange.new(5, 10)
        emitter.Lifetime = NumberRange.new(0.3, 0.6)
        emitter.SpreadAngle = Vector2.new(45, 45)
        emitter.Parent = vfxPart
        emitter:Emit(25)

        task.wait(1.0)
        vfxPart:Destroy()
    end)

    if plantRemoteEvent then
        plantRemoteEvent:FireServer(hitPosition, seedName, partName)
        print(string.format("[PlantingController] 🌱 %s tohumu ekildi! (%.1f, %.1f, %.1f)", seedName, hitPosition.X, hitPosition.Y, hitPosition.Z))
    else
        warn("[PlantingController] Plant_RE RemoteEvent bulunamadı!")
    end
end

-- 1. Fare Sol Tık ve Dokunma Girişi
UserInputService.InputBegan:Connect(function(input, processed)
    if processed then return end
    if input.UserInputType == Enum.UserInputType.MouseButton1 or input.UserInputType == Enum.UserInputType.Touch then
        local tool, _ = getEquippedSeedTool()
        if tool and mouse then
            tryPlantAt(mouse.Target, mouse.Hit.Position)
        end
    end
end)

-- 2. Mouse Button 1 Backup
if mouse then
    mouse.Button1Down:Connect(function()
        local tool, _ = getEquippedSeedTool()
        if tool and mouse.Target then
            tryPlantAt(mouse.Target, mouse.Hit.Position)
        end
    end)
end

-- 3. Tool.Activated desteği
local function bindTool(tool)
    if not tool:IsA("Tool") then return end
    tool.Activated:Connect(function()
        if mouse then
            tryPlantAt(mouse.Target, mouse.Hit.Position)
        end
    end)
end

local function onCharacter(char)
    if not char then return end
    char.ChildAdded:Connect(function(child)
        if child:IsA("Tool") then
            bindTool(child)
        end
    end)
    local curTool = char:FindFirstChildOfClass("Tool")
    if curTool then
        bindTool(curTool)
    end
end

if localPlayer.Character then
    onCharacter(localPlayer.Character)
end
localPlayer.CharacterAdded:Connect(onCharacter)

print("[PlantingController] Grow A Garden fare tıklama ekiş kontrolcüsü aktif!")
