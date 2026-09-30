-- SeedData.lua
-- 15 Orijinal Bitki / Ağaç Türü ve Özellikleri (Tip A: Kalıcı Ağaç, Tip B: Çiçek / Çalı)
-- Boyutlarına göre orantılı büyüme: Küçük bitkiler hızlı, büyük ağaçlar daha yavaş

local SeedData = {
    ["Carrot"] = {
        SeedName = "Carrot Seed",
        DisplayName = "Carrot",
        Type = "Flower", -- Tip B: Hasatta yok olur, 3-5 adet verir
        GrowthTime = 18,
        RegrowTime = nil,
        MinHarvest = 3,
        MaxHarvest = 5,
        Price = 15,
        SellPerKg = 4,
        MarketRate = 100,
        StockMin = 5,
        StockMax = 20,
        LayoutOrder = 1,
    },
    ["Potato"] = {
        SeedName = "Potato Seed",
        DisplayName = "Potato",
        Type = "Flower", -- Tip B: Hasatta yok olur, 3-5 adet verir
        GrowthTime = 24,
        RegrowTime = nil,
        MinHarvest = 3,
        MaxHarvest = 5,
        Price = 30,
        SellPerKg = 9,
        MarketRate = 100,
        StockMin = 4,
        StockMax = 15,
        LayoutOrder = 2,
    },
    ["Strawberry"] = {
        SeedName = "Strawberry Seed",
        DisplayName = "Strawberry",
        Type = "Flower", -- Tip B: Hasatta yok olur, 3-5 adet verir
        GrowthTime = 32,
        RegrowTime = nil,
        MinHarvest = 3,
        MaxHarvest = 5,
        Price = 50,
        SellPerKg = 15,
        MarketRate = 100,
        StockMin = 3,
        StockMax = 12,
        LayoutOrder = 3,
    },
    ["Blueberry"] = {
        SeedName = "Blueberry Seed",
        DisplayName = "Blueberry",
        Type = "Bush", -- Tip B: Hasatta yok olur, 3-5 adet verir
        GrowthTime = 45,
        RegrowTime = nil,
        MinHarvest = 3,
        MaxHarvest = 5,
        Price = 80,
        SellPerKg = 25,
        MarketRate = 100,
        StockMin = 3,
        StockMax = 10,
        LayoutOrder = 4,
    },
    ["Apple Tree"] = {
        SeedName = "Apple Tree Seed",
        DisplayName = "Apple Tree",
        Type = "Tree", -- Tip A: Kalıcı Ağaç, 1 adet verir, regrow süresi vardır
        GrowthTime = 75,
        RegrowTime = 90,
        MinHarvest = 1,
        MaxHarvest = 1,
        Price = 150,
        SellPerKg = 45,
        MarketRate = 100,
        StockMin = 2,
        StockMax = 8,
        LayoutOrder = 5,
    },
    ["Mango Tree"] = {
        SeedName = "Mango Tree Seed",
        DisplayName = "Mango Tree",
        Type = "Tree", -- Tip A: Kalıcı Ağaç, 1 adet verir, regrow süresi vardır
        GrowthTime = 105,
        RegrowTime = 130,
        MinHarvest = 1,
        MaxHarvest = 1,
        Price = 350,
        SellPerKg = 110,
        MarketRate = 80,
        StockMin = 2,
        StockMax = 6,
        LayoutOrder = 6,
    },
    ["Frostleaf"] = {
        SeedName = "Frostleaf Seed",
        DisplayName = "Frostleaf",
        Type = "Bush", -- Tip B: Hasatta yok olur, 3-5 adet verir
        GrowthTime = 55,
        RegrowTime = nil,
        MinHarvest = 3,
        MaxHarvest = 5,
        Price = 700,
        SellPerKg = 220,
        MarketRate = 60,
        StockMin = 2,
        StockMax = 6,
        LayoutOrder = 7,
    },
    ["Cherry Tree"] = {
        SeedName = "Cherry Tree Seed",
        DisplayName = "Cherry Tree",
        Type = "Tree", -- Tip A: Kalıcı Ağaç, 1 adet verir, regrow süresi vardır
        GrowthTime = 140,
        RegrowTime = 170,
        MinHarvest = 1,
        MaxHarvest = 1,
        Price = 1200,
        SellPerKg = 380,
        MarketRate = 100,
        StockMin = 2,
        StockMax = 6,
        LayoutOrder = 8,
    },
    ["Banana Tree"] = {
        SeedName = "Banana Tree Seed",
        DisplayName = "Banana Tree",
        Type = "Tree", -- Tip A: Kalıcı Ağaç, 1 adet verir, regrow süresi vardır
        GrowthTime = 200,
        RegrowTime = 240,
        MinHarvest = 1,
        MaxHarvest = 1,
        Price = 2500,
        SellPerKg = 800,
        MarketRate = 40,
        StockMin = 1,
        StockMax = 4,
        LayoutOrder = 9,
    },
    ["Dragonfruit Bush"] = {
        SeedName = "Dragonfruit Bush Seed",
        DisplayName = "Dragonfruit Bush",
        Type = "Bush", -- Tip B: Hasatta yok olur, 3-5 adet verir
        GrowthTime = 260,
        RegrowTime = nil,
        MinHarvest = 3,
        MaxHarvest = 5,
        Price = 8000,
        SellPerKg = 2600,
        MarketRate = 15,
        StockMin = 1,
        StockMax = 3,
        LayoutOrder = 10,
    },
    ["Starfruit Plant"] = {
        SeedName = "Starfruit Plant Seed",
        DisplayName = "Starfruit Plant",
        Type = "Tree", -- Tip A: Kalıcı Ağaç, 1 adet verir, regrow süresi vardır
        GrowthTime = 340,
        RegrowTime = 400,
        MinHarvest = 1,
        MaxHarvest = 1,
        Price = 18000,
        SellPerKg = 5800,
        MarketRate = 10,
        StockMin = 1,
        StockMax = 3,
        LayoutOrder = 11,
    },
    ["Coconut Palm"] = {
        SeedName = "Coconut Palm Seed",
        DisplayName = "Coconut Palm",
        Type = "Tree", -- Tip A: Kalıcı Ağaç, 1 adet verir, regrow süresi vardır
        GrowthTime = 480,
        RegrowTime = 560,
        MinHarvest = 1,
        MaxHarvest = 1,
        Price = 45000,
        SellPerKg = 14500,
        MarketRate = 4,
        StockMin = 1,
        StockMax = 2,
        LayoutOrder = 12,
    },
    ["Crystal Shrub"] = {
        SeedName = "Crystal Shrub Seed",
        DisplayName = "Crystal Shrub",
        Type = "Bush", -- Tip B: Hasatta yok olur, 3-5 adet verir
        GrowthTime = 650,
        RegrowTime = nil,
        MinHarvest = 3,
        MaxHarvest = 5,
        Price = 100000,
        SellPerKg = 32000,
        MarketRate = 2,
        StockMin = 1,
        StockMax = 2,
        LayoutOrder = 13,
    },
    ["Nebula Vine"] = {
        SeedName = "Nebula Vine Seed",
        DisplayName = "Nebula Vine",
        Type = "Bush", -- Tip B: Hasatta yok olur, 3-5 adet verir
        GrowthTime = 850,
        RegrowTime = nil,
        MinHarvest = 3,
        MaxHarvest = 5,
        Price = 300000,
        SellPerKg = 95000,
        MarketRate = 0.5,
        StockMin = 1,
        StockMax = 1,
        LayoutOrder = 14,
    },
    ["Celestial Tree"] = {
        SeedName = "Celestial Tree Seed",
        DisplayName = "Celestial Tree",
        Type = "Tree", -- Tip A: Kalıcı Ağaç, 1 adet verir, regrow süresi vardır
        GrowthTime = 1200,
        RegrowTime = 1500,
        MinHarvest = 1,
        MaxHarvest = 1,
        Price = 1000000,
        SellPerKg = 320000,
        MarketRate = 0.1,
        StockMin = 1,
        StockMax = 1,
        LayoutOrder = 15,
    },
}

-- Kullanım Kolaylığı İçin Eş Anlamlı (Alias) Eşlemeler
local aliases = {
    ["Apple"] = "Apple Tree",
    ["Mango"] = "Mango Tree",
    ["Cherry"] = "Cherry Tree",
    ["Banana"] = "Banana Tree",
    ["Dragon Fruit"] = "Dragonfruit Bush",
    ["Dragonfruit"] = "Dragonfruit Bush",
    ["Starfruit"] = "Starfruit Plant",
    ["Coconut"] = "Coconut Palm",
    ["Palm"] = "Coconut Palm",
}

for alias, target in pairs(aliases) do
    if SeedData[target] then
        SeedData[alias] = SeedData[target]
    end
end

-- Tohum İsimlerine Göre de Eşleme Sağla
for key, data in pairs(SeedData) do
    if not SeedData[data.SeedName] then
        SeedData[data.SeedName] = data
    end
end

return SeedData