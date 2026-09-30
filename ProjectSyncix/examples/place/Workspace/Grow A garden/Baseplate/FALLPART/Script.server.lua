-- FALLPART Respawn Handler
-- Oyuncu boşluğa düştüğünde kendi adasına geri döndürür

local Players = game:GetService("Players")
local Workspace = game:GetService("Workspace")

local farmFolder = Workspace:WaitForChild("Farm", 15)

local function getPlayerFarm(player)
    if not farmFolder then return nil end
    local farmName = player:GetAttribute("FarmName")
    if farmName and farmFolder:FindFirstChild(farmName) then
        return farmFolder[farmName]
    end
    for _, f in ipairs(farmFolder:GetChildren()) do
        local d = f:FindFirstChild("Important") and f.Important:FindFirstChild("Data")
        if d and d:FindFirstChild("Owner") and d.Owner.Value == player.Name then
            return f
        end
    end
    return farmFolder:GetChildren()[1]
end

script.Parent.Touched:Connect(function(part)
    local char = part.Parent
    local plr = Players:GetPlayerFromCharacter(char)
    if plr and char and char.PrimaryPart then
        local farm = getPlayerFarm(plr)
        if farm then
            local sp = farm:FindFirstChild("Spawn_Point", true)
            if sp and sp:IsA("BasePart") then
                char:PivotTo(sp.CFrame + Vector3.new(0, 3.5, 0))
                local hrp = char:FindFirstChild("HumanoidRootPart")
                if hrp then
                    hrp.AssemblyLinearVelocity = Vector3.zero
                    hrp.AssemblyAngularVelocity = Vector3.zero
                end
            end
        end
    end
end)