-- GamePassConfig.lua
-- Oyunun Gamepass Yapılandırması ve Ayarları
-- 9 Robuxluk "Meyve / Sebze Çalma (Hırsızlık)" Gamepass'i

local GamePassConfig = {
    -- -------------------------------------------------------------
    -- 9 ROBUX HIRSIZLIK GAMEPASS'İ
    -- Roblox Creator Hub (Dashboard) üzerinden oluşturduğunuz 9 Robuxluk
    -- Gamepass ID'nizi aşağıdaki alana yazabilirsiniz:
    -- -------------------------------------------------------------
    STEAL_GAMEPASS_ID = 0, -- Kendi Gamepass ID'nizi buraya yazın (Örn: 123456789)
    STEAL_PRICE_ROBUX = 9,
    STEAL_NAME = "Meyve & Sebze Çalma Gamepass",
    STEAL_DESC = "Başkalarının tarlalarına sızıp olgunlaşmış meyve ve sebzelerini çalabilmenizi sağlar!",

    -- Geliştirici ve Studio Test Modu (Robux harcamadan test etme imkanı)
    DEVELOPER_FREE_TEST = true,
}

return GamePassConfig
