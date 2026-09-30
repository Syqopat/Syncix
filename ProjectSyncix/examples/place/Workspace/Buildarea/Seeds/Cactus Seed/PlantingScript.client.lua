-- Bulletproof PlantingScript
local UserInputService = game:GetService("UserInputService")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local Players = game:GetService("Players")
local Workspace = game:GetService("Workspace")

local gameEvents = ReplicatedStorage:WaitForChild("GameEvents", 15)
local plantRemoteEvent = gameEvents and gameEvents:WaitForChild("Plant_RE", 15)

local tool = script.Parent
local localPlayer = Players.LocalPlayer

local isEquipped = false
local canPlant = true

tool.Equipped:Connect(function() isEquipped = true end)
tool.Unequipped:Connect(function() isEquipped = false end)

local function plant()
    if not isEquipped or not canPlant or not plantRemoteEvent then return end
    local mouse = localPlayer:GetMouse()
    if not mouse or not mouse.Hit then return end

    canPlant = false
    local sName = tool:GetAttribute("Seed") or string.gsub(tool.Name, "%s*Seed.*", "")
    local partName = (mouse.Target and mouse.Target.Name) or "Can_Plant1"
    plantRemoteEvent:FireServer(mouse.Hit.Position, sName, partName)
    task.wait(0.25)
    canPlant = true
end

tool.Activated:Connect(plant)
local mouse = localPlayer:GetMouse()
if mouse then
    mouse.Button1Down:Connect(function()
        if isEquipped then plant() end
    end)
end
