-- Ultimate PlantingScript (Grow A Garden 1-e-1)
local UserInputService = game:GetService("UserInputService")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local Players = game:GetService("Players")
local Workspace = game:GetService("Workspace")

local gameEvents = ReplicatedStorage:WaitForChild("GameEvents", 10)
local plantRemoteEvent = gameEvents and gameEvents:WaitForChild("Plant_RE", 10)

local tool = script.Parent
local localPlayer = Players.LocalPlayer

local isEquipped = false
local canPlant = true

tool.Equipped:Connect(function()
    isEquipped = true
end)

tool.Unequipped:Connect(function()
    isEquipped = false
end)

local function getFarmFromPartOrPosition(part, pos)
    local farmFolder = Workspace:FindFirstChild("Farm")
    if not farmFolder then return nil end

    if part then
        local cur = part
        while cur and cur ~= Workspace do
            if cur.Parent and cur.Parent.Name == "Farm" then
                return cur
            end
            cur = cur.Parent
        end
    end

    if pos then
        for _, farm in ipairs(farmFolder:GetChildren()) do
            local pl = farm:FindFirstChild("Important") and farm.Important:FindFirstChild("Plant_Locations")
            local cp1 = pl and pl:FindFirstChild("Can_Plant1")
            if cp1 and (cp1.Position - pos).Magnitude < 45 then
                return farm
            end
        end
    end

    local assignedIsland = localPlayer:GetAttribute("AssignedIsland")
    if assignedIsland and farmFolder:FindFirstChild("Farm_" .. assignedIsland) then
        return farmFolder["Farm_" .. assignedIsland]
    end

    return nil
end

local function handlePlanting(targetPart, hitPosition)
    if not isEquipped or not canPlant then return end

    local mouse = localPlayer:GetMouse()
    if not hitPosition and mouse then
        hitPosition = mouse.Hit.Position
    end
    if not targetPart and mouse then
        targetPart = mouse.Target
    end
    if not hitPosition then return end

    local farm = getFarmFromPartOrPosition(targetPart, hitPosition)
    if not farm then return end

    local important = farm:FindFirstChild("Important")
    local dataFolder = important and important:FindFirstChild("Data")
    local plotOwner = dataFolder and dataFolder:FindFirstChild("Owner") and dataFolder.Owner.Value or "None"
    local assignedIsland = localPlayer:GetAttribute("AssignedIsland")
    local farmIslandName = string.gsub(farm.Name, "Farm_", "")

    local isMyFarm = (plotOwner == localPlayer.Name)
        or (assignedIsland and assignedIsland == farmIslandName)
        or (plotOwner == "None")

    if isMyFarm then
        canPlant = false
        local sName = tool:GetAttribute("Seed") or string.gsub(tool.Name, " Seed.*", "")
        local partName = (targetPart and targetPart.Name) or "Can_Plant1"

        if plantRemoteEvent then
            plantRemoteEvent:FireServer(hitPosition, sName, partName)
            print("[Grow A Garden] Tohum başarıyla ekildi: " .. sName .. " -> " .. farm.Name)
        end

        task.wait(0.2)
        canPlant = true
    else
        print("[Grow A Garden] Bu tarla " .. tostring(plotOwner) .. " adlı oyuncuya ait! Kendi çiftliğinize ekmelisiniz.")
    end
end

-- 1. Native Roblox Tool Activation (Fires on Click / Tap anywhere while tool equipped)
tool.Activated:Connect(function()
    local mouse = localPlayer:GetMouse()
    if mouse then
        handlePlanting(mouse.Target, mouse.Hit.Position)
    end
end)

-- 2. Mouse Button 1 Backup
local mouse = localPlayer:GetMouse()
if mouse then
    mouse.Button1Down:Connect(function()
        if isEquipped and mouse.Target then
            handlePlanting(mouse.Target, mouse.Hit.Position)
        end
    end)
end

-- 3. Touch Support for mobile / touchscreens
if UserInputService.TouchEnabled then
    local currentCamera = Workspace.CurrentCamera
    UserInputService.TouchTapInWorld:Connect(function(touchPosition, processed)
        if processed or not isEquipped then return end
        local cameraRay = currentCamera:ViewportPointToRay(touchPosition.X, touchPosition.Y)
        local worldRay = Ray.new(cameraRay.Origin, cameraRay.Direction * 500)
        local hitPart, hitPosition = Workspace:FindPartOnRay(worldRay)
        handlePlanting(hitPart, hitPosition)
    end)
end
