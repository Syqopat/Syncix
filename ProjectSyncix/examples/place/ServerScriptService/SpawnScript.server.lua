local Players = game:GetService("Players")
local spawnLocation = game.Workspace:WaitForChild("SpawnLocation")

Players.PlayerAdded:Connect(function(player)
    player.CharacterAdded:Connect(function(character)
        if player.Name == "0ben_ege0" then
            task.wait(0.5) -- Karakterin tam yüklenmesini beklemek için
            character:PivotTo(spawnLocation.CFrame + Vector3.new(0, 3, 0))
        end
    end)
end)