local Product = 3289231671
local Farm = script.Parent.Parent.Parent.Parent

local MarketPlace = game:GetService("MarketplaceService")
local SSS = game:GetService("ServerScriptService")
script.Parent.Triggered:Connect(function(player)
	if not player:FindFirstChild("FarmBought") then
		local obj = Instance.new("ObjectValue")
		obj.Value = Farm
		obj.Name = "FarmBought"
		obj.Parent = player
	else
		player.FarmBought.Value = Farm
	end
	MarketPlace:PromptProductPurchase(player, Product, false, Enum.CurrencyType.Robux)
end)

SSS.GrowAllBought.Event:Connect(function(player)
	local objVal = player:FindFirstChild("FarmBought")
	if objVal and objVal.Value == Farm then
		for i, v in Farm.Important.Plants_Physical:GetDescendants() do
			if v.Name == "Grow_Rate" then
				v.Value = 1000
			end
		end
	end
end)