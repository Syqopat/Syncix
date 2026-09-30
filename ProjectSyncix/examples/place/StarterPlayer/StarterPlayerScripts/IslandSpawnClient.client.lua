-- IslandSpawnClient.client.lua
-- Oyuncunun doğduğunda ve respawn olduğunda KESİNLİKLE kendi adasında olmasını sağlayan istemci güvencesi

local Players = game:GetService("Players")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local Workspace = game:GetService("Workspace")
local RunService = game:GetService("RunService")

local player = Players.LocalPlayer
local TeleportEvent = ReplicatedStorage:WaitForChild("TeleportEvent", 15)
local ClientTeleport = ReplicatedStorage:WaitForChild("ClientTeleport", 15)

local function getMySpawnTarget()
    local farmFolder = Workspace:FindFirstChild("Farm")
    if not farmFolder then return nil, nil end

    local assigned = player:GetAttribute("AssignedIsland")
    local farm = nil
    if assigned and farmFolder:FindFirstChild("Farm_" .. assigned) then
        farm = farmFolder["Farm_" .. assigned]
    else
        for _, f in ipairs(farmFolder:GetChildren()) do
            local data = f:FindFirstChild("Important") and f.Important:FindFirstChild("Data")
            if data and data:FindFirstChild("Owner") and data.Owner.Value == player.Name then
                farm = f
                break
            end
        end
    end

    if farm then
        local sp = farm:FindFirstChild("Spawn_Point", true)
        if sp and sp:IsA("BasePart") then
            return CFrame.new(sp.Position + Vector3.new(0, 3.5, 0)), sp.Position
        end
        local cf, sz = farm:GetBoundingBox()
        return CFrame.new(cf.Position + Vector3.new(0, 5.0, 0)), cf.Position
    end

    return nil, nil
end

local SHOP_POS = Vector3.new(0, 127.5, -5)

local function checkAndEnforceSpawn(newChar)
    local char = newChar or player.Character
    if not char then
        char = player.CharacterAdded:Wait()
    end
    local hrp = char:WaitForChild("HumanoidRootPart", 10)
    if not hrp then return end

    for _, delayTime in ipairs({0.1, 0.4, 1.0, 2.0}) do
        task.wait(delayTime)
        if not char or not char.Parent or not hrp or not hrp.Parent then break end

        -- Eğer oyuncu isteyerek Shop'a ışınlandıysa geri çekme
        if (hrp.Position - SHOP_POS).Magnitude < 40 then
            break
        end

        local targetCF, targetPos = getMySpawnTarget()
        if targetCF and targetPos then
            local dist = (hrp.Position - targetPos).Magnitude
            -- Eğer oyuncu kendi adasının merkezinden 35 stud'dan uzaksa veya gökyüzündeyse kendi adasına çek
            if dist > 35 or hrp.Position.Y > 200 or hrp.Position.Y < -175 then
                hrp.AssemblyLinearVelocity = Vector3.zero
                hrp.AssemblyAngularVelocity = Vector3.zero
                hrp.CFrame = targetCF
                char:PivotTo(targetCF)

                if TeleportEvent then
                    TeleportEvent:FireServer("Island")
                end
            end
        else
            if TeleportEvent then
                TeleportEvent:FireServer("Island")
            end
        end
    end
end

player.CharacterAdded:Connect(checkAndEnforceSpawn)
if player.Character then
    task.spawn(checkAndEnforceSpawn, player.Character)
end

if ClientTeleport then
    ClientTeleport.OnClientEvent:Connect(function(targetCF)
        if targetCF and typeof(targetCF) == "CFrame" and player.Character then
            local hrp = player.Character:FindFirstChild("HumanoidRootPart")
            if hrp then
                hrp.AssemblyLinearVelocity = Vector3.zero
                hrp.AssemblyAngularVelocity = Vector3.zero
                hrp.CFrame = targetCF
                player.Character:PivotTo(targetCF)
            end
        end
    end)
end

-- Sürekli boşluk kontrolü (İstemci tarafında sıfır gecikmeli kurtarma)
local VOID_Y_THRESHOLD = -175
local lastClientVoidTeleport = 0

RunService.Heartbeat:Connect(function()
    local char = player.Character
    if not char or not char.Parent then return end
    local hrp = char:FindFirstChild("HumanoidRootPart")
    local hum = char:FindFirstChildOfClass("Humanoid")
    if not hrp or not hum or hum.Health <= 0 then return end

    if hrp.Position.Y < VOID_Y_THRESHOLD then
        local now = os.clock()
        if now - lastClientVoidTeleport > 0.8 then
            lastClientVoidTeleport = now

            -- Hızları yerel olarak sıfırla ve serbest düşüş animasyonunu sonlandır
            hrp.AssemblyLinearVelocity = Vector3.zero
            hrp.AssemblyAngularVelocity = Vector3.zero
            hum:ChangeState(Enum.HumanoidStateType.GettingUp)

            -- İstemci tarafında sıfır gecikmeyle adaya konumlandır
            local targetCF, _ = getMySpawnTarget()
            if targetCF then
                hrp.CFrame = targetCF
                char:PivotTo(targetCF)
            end

            -- Sunucuya güvenli teleport ve can tazeleme bildir
            if TeleportEvent then
                TeleportEvent:FireServer("VoidFall")
            end
        end
    end
end)

