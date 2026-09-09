--!strict
-- PatchBuilder
-- Observer'dan gelen ham değişiklikleri alır ve standartlaştırılmış Patch (Yama) nesnelerine çevirir.
-- Ağa gidecek JSON verisini hazırlar.

local HttpService = game:GetService("HttpService")
local CollectionService = game:GetService("CollectionService")
local PlaceKimligi = require(script.Parent.Parent.Core.PlaceKimligi)

local PatchBuilder = {}
PatchBuilder.__index = PatchBuilder

function PatchBuilder.new()
    local self = setmetatable({}, PatchBuilder)
    return self
end

function PatchBuilder:OnStart(container)
    self.cache = container:Get("RuntimeCache")
end

-- Tek bir property değişikliğini Patch'e çevirir.
function PatchBuilder:BuildPropertyPatch(uuid: string, propertyName: string, newValue: any): any
    local patch = {
        event_type = "PROPERTY_UPDATE",
        version = "v1",
        data = {
            syncix_id = uuid,
            property = propertyName,
            value = self:SerializeValue(newValue)
        }
    }
    return patch
end

-- Yeni yaratılmış bir nesne için LifecyclePatch üretir.
function PatchBuilder:BuildLifecyclePatch(uuid: string, instance: Instance, eventType: string): any
    -- Parent UUID'sini bul (hiyerarşinin VS Code tarafında doğru kurulması için)
    local parentUuid = nil
    if instance.Parent then
        parentUuid = instance.Parent:GetAttribute("__syncix_id")
    end

    local patch = {
        event_type = eventType, -- "CREATE" veya "DESTROY"
        version = "v1",
        data = {
            syncix_id = uuid,
            class_name = instance.ClassName,
            name = instance.Name,
            parent = parentUuid
        }
    }

    -- Script ise kaynak kodunu da ekle (yaratılırken editöre gelsin)
    if eventType == "CREATE" and instance:IsA("LuaSourceContainer") then
        local ok, src = pcall(function() return (instance :: any).Source end)
        if ok then
            patch.data.source = src
        end
    end

    -- Attribute'ları ve property'leri ekle (CREATE'te)
    if eventType == "CREATE" then
        local attrs = self:SerializeAttributes(instance)
        if attrs then
            patch.data.attributes = attrs
        end
        local etiketler = self:SerializeTags(instance)
        if etiketler then
            patch.data.tags = etiketler
        end
        local props = self:SerializeProperties(instance)
        if props then
            patch.data.properties = props
        end
    end

    return patch
end

-- Bir objenin Attribute'larını serileştirir (__syncix_id hariç). Boşsa nil döner.
function PatchBuilder:SerializeAttributes(instance: Instance): any
    local attrs = {}
    local count = 0
    for name, value in pairs(instance:GetAttributes()) do
        -- __syncix_id ve __syncix_place Syncix'in KENDI defterleri; icerik degil.
        -- Diske yazilirlarsa place kimligi dosyalara sizar ve baska bir place'e
        -- kopyalanabilir hale gelir — kimligin tek isi ayirt etmek oldugu icin
        -- bu onu ise yaramaz kilardi.
        if name ~= "__syncix_id" and name ~= "__syncix_place" then
            local sv = self:SerializeValue(value)
            if sv ~= nil then
                attrs[name] = sv
                count += 1
            end
        end
    end
    if count == 0 then return nil end
    return attrs
end

-- Bir objenin CollectionService etiketlerini dondurur. Etiketi yoksa nil.
--
-- Etiketler property degil: Roblox'ta instance uzerinde bir alan olarak
-- durmuyorlar, CollectionService'te tutuluyorlar. Bu yuzden property tablosu
-- onlari asla goremezdi ve etiketle calisan oyunlarin mantigi editorde
-- tamamen gorunmezdi.
function PatchBuilder:SerializeTags(instance: Instance): any
    local ok, etiketler = pcall(function()
        return CollectionService:GetTags(instance)
    end)
    if not ok or not etiketler or #etiketler == 0 then
        return nil
    end
    -- Siralanmis gonderiliyor: ayni etiket kumesi her seferinde ayni listeyi
    -- uretsin ki gereksiz "degisti" yamasi cikmasin.
    table.sort(etiketler)
    return etiketler
end

-- Tek bir Attribute değişikliği için ATTRIBUTE_UPDATE patch üretir.
function PatchBuilder:BuildAttributePatch(uuid: string, attrName: string, value: any): any
    return {
        event_type = "ATTRIBUTE_UPDATE",
        version = "v1",
        data = {
            syncix_id = uuid,
            name = attrName,
            value = self:SerializeValue(value)
        }
    }
end

-- Bir objenin ebeveyni değiştiğinde (taşıma) REPARENT patch üretir.
function PatchBuilder:BuildReparentPatch(uuid: string, parentUuid: string): any
    return {
        event_type = "REPARENT",
        version = "v1",
        data = {
            syncix_id = uuid,
            parent = parentUuid
        }
    }
end

-- Gelen verinin tipine göre uygun serileştirme yapar (Vector3, Color3, Enum, string, number vb.)
function PatchBuilder:SerializeValue(value: any): any
    local t = typeof(value)
    if t == "Vector3" then
        return { Vector3 = { x = value.X, y = value.Y, z = value.Z } }
    elseif t == "Color3" then
        return { Color3 = { r = value.R, g = value.G, b = value.B } }
    elseif t == "EnumItem" then
        return tostring(value) -- "Enum.Material.Plastic" (string olarak)
    elseif t == "UDim2" then
        return { UDim2 = { xs = value.X.Scale, xo = value.X.Offset, ys = value.Y.Scale, yo = value.Y.Offset } }
    elseif t == "Vector2" then
        return { Vector2 = { x = value.X, y = value.Y } }
    elseif t == "UDim" then
        return { UDim = { scale = value.Scale, offset = value.Offset } }
    elseif t == "CFrame" then
        -- Konum + 3x3 donme matrisi. Orientation tek basina yetmez: Euler
        -- acilariyla ifade edilemeyen yonelimler var.
        local x, y, z,
              r00, r01, r02,
              r10, r11, r12,
              r20, r21, r22 = value:GetComponents()
        return {
            CFrame = {
                pos = { x, y, z },
                rot = { r00, r01, r02, r10, r11, r12, r20, r21, r22 },
            },
        }
    elseif t == "NumberRange" then
        return { NumberRange = { min = value.Min, max = value.Max } }
    elseif t == "BrickColor" then
        -- Duz metin olarak gonderilemez: karsi tarafta
        -- `part.BrickColor = "Really red"` atamasi sessizce basarisiz oluyor.
        return { BrickColor = value.Name }
    elseif t == "Content" then
        -- Roblox'un yeni Content tipi. Eskiler (Decal.Texture, SoundId) hala
        -- duz metin dondugu icin asagidaki string dalindan geciyor.
        return { Content = value.Uri or "" }
    elseif t == "ColorSequence" then
        local noktalar = {}
        for _, k in ipairs(value.Keypoints) do
            table.insert(noktalar, {
                t = k.Time,
                r = k.Value.R,
                g = k.Value.G,
                b = k.Value.B,
            })
        end
        return { ColorSequence = noktalar }
    elseif t == "NumberSequence" then
        local noktalar = {}
        for _, k in ipairs(value.Keypoints) do
            -- Envelope Roblox'un rastgelelik payi; atlanirsa parcacik efekti
            -- duzlesir, o yuzden tasiniyor.
            table.insert(noktalar, { t = k.Time, v = k.Value, envelope = k.Envelope })
        end
        return { NumberSequence = noktalar }
    elseif t == "Rect" then
        return {
            Rect = {
                min = { value.Min.X, value.Min.Y },
                max = { value.Max.X, value.Max.Y },
            },
        }
    elseif t == "Font" then
        return {
            Font = {
                family = value.Family,
                weight = tostring(value.Weight),
                style = tostring(value.Style),
            },
        }
    elseif t == "PhysicalProperties" then
        return {
            PhysicalProperties = {
                density = value.Density,
                friction = value.Friction,
                elasticity = value.Elasticity,
                frictionWeight = value.FrictionWeight,
                elasticityWeight = value.ElasticityWeight,
            },
        }
    elseif t == "Instance" then
        -- Baska bir objeye referans: hedefin UUID'si tasinir.
        -- Metin olarak gonderilseydi karsi taraf onu duz metin sanip
        -- instance'a cevirmezdi; bu yuzden ayri bir sarmalayici var.
        local hedefId = value:GetAttribute("__syncix_id")
        if hedefId then
            return { Ref = tostring(hedefId) }
        end
        return nil
    elseif t == "string" or t == "number" or t == "boolean" then
        return value
    end
    -- Bos referans (ObjectValue.Value = nil gibi) bilgi tasir: "baglanti yok".
    if value == nil then
        return { Ref = "" }
    end
    -- Desteklenmeyen tipler için
    return nil
end

-- Sınıflara göre otomatik senkronlanan property listeleri.
local PropertyTable = require(script.Parent.PropertyTable)
local Services = require(script.Parent.Services)

-- Sinif basina property listesi, kalitim birlestirilerek ve ONBELLEKLENEREK.
--
-- Eskiden bu listeler elle yaziliyordu ve yalnizca 12 sinifi kapsiyordu; Model,
-- Humanoid, ParticleEmitter gibi her sey yalnizca yapisal olarak senkron oluyor,
-- tek bir ayari bile gitmiyordu. Artik liste Roblox API dokumundan uretiliyor
-- (tools/gen-properties.py) ve 248 sinifi, 1263 property'yi kapsiyor.
--
-- Onbellek onemli: kalitim zinciri her instance icin degil, her SINIF icin bir
-- kez cozulur.
local sinifOnbellegi = {}

local function sinifPropertyleri(className: string)
	local onbellek = sinifOnbellegi[className]
	if onbellek then
		return onbellek
	end

	local birlesik = {}
	local mevcut = className
	local koruma = 0
	while mevcut and koruma < 64 do
		local kayit = PropertyTable[mevcut]
		if kayit then
			for ad, tip in pairs(kayit.p) do
				-- Alt sinif ust sinifi EZER: ayni ad iki yerde varsa alt sinifin
				-- tanimi gecerlidir.
				if birlesik[ad] == nil then
					birlesik[ad] = tip
				end
			end
			mevcut = kayit.u
		else
			break
		end
		koruma += 1
	end

	sinifOnbellegi[className] = birlesik
	return birlesik
end



-- Bir instance'ın izlenen tüm property'lerini serileştirir (property adı -> değer). Boşsa nil.
function PatchBuilder:SerializeProperties(instance: Instance): any
    -- LocalizationTable'in "Contents" diye bir property'si YOKTUR; ilk denemede
    -- oyle varsayilmisti ve Studio "Contents is not a valid member" dedi.
    -- Dogru API GetEntries()/SetEntries(). Ceviri girdilerini JSON metnine
    -- cevirip sanki bir property'ymis gibi tasiyoruz; boylece mevcut String
    -- property yolu ve .csv dosya bicimi oldugu gibi calisiyor.
    if instance:IsA("LocalizationTable") then
        local ok, json = pcall(function()
            local HttpService = game:GetService("HttpService")
            return HttpService:JSONEncode((instance :: any):GetEntries())
        end)
        if ok and json then
            return { Contents = json }
        end
        return nil
    end

    local liste = sinifPropertyleri(instance.ClassName)
    local props = {}
    local count = 0
    for ad in pairs(liste) do
        local ok, val = pcall(function() return (instance :: any)[ad] end)
        if ok then
            local sv = self:SerializeValue(val)
            if sv ~= nil then
                props[ad] = sv
                count += 1
            end
        end
    end
    if count == 0 then return nil end
    return props
end

-- Bir property adının bu instance için otomatik izlenip izlenmediğini döndürür.
function PatchBuilder:IsWatchedProperty(instance: Instance, propName: string): boolean
    return sinifPropertyleri(instance.ClassName)[propName] ~= nil
end

-- Tüm ağacı dolaşıp FULL_SYNC JSON nesnesi üretir
function PatchBuilder:BuildFullTreeSnapshot(): any
    local servicesToSync = Services.List()
    
    local instances = {}

    -- 1. Önce servislerin kendilerini kök node olarak ekle.
    -- Servisler de UUID alır; böylece hiyerarşi uçtan uca UUID ile taşınır.
    for _, service in ipairs(servicesToSync) do
        local serviceUuid = service:GetAttribute("__syncix_id")
        if not serviceUuid then
            serviceUuid = HttpService:GenerateGUID(false)
            service:SetAttribute("__syncix_id", serviceUuid)
        end
        if self.cache then
            self.cache:CacheInstance(serviceUuid, service)
        end
        -- Servisin KENDİ property'leri de taşınır. Uzun süre yalnızca çocukları
        -- gönderiliyordu; Lighting.ClockTime, Lighting.Ambient, Workspace.Gravity
        -- gibi bir oyunun görünüşünü belirleyen ayarlar editörde hiç görünmüyordu.
        table.insert(instances, {
            syncix_id = serviceUuid,
            class_name = service.ClassName,
            name = service.Name,
            properties = self:SerializeProperties(service)
            -- parent alanı yok: kök node
        })
    end

    local function serializeNode(instance: Instance)
        local uuid = instance:GetAttribute("__syncix_id")
        if not uuid then return end

        local parentUuid = nil
        if instance.Parent and instance.Parent:GetAttribute("__syncix_id") then
            parentUuid = instance.Parent:GetAttribute("__syncix_id")
        end

        local nodeData = {
            syncix_id = uuid,
            class_name = instance.ClassName,
            name = instance.Name,
            parent = parentUuid
        }
        
        -- Serialize properties (genis kapsam)
        pcall(function()
            local props = self:SerializeProperties(instance)
            if props then
                nodeData.properties = props
            end
            if instance:IsA("LuaSourceContainer") then
                -- Script kaynak kodunu bootstrap'ta gönder ki editörde .lua olarak açılabilsin
                nodeData.source = (instance :: any).Source
            end
            -- Attribute'ları bootstrap'ta gönder
            local attrs = self:SerializeAttributes(instance)
            if attrs then
                nodeData.attributes = attrs
            end
            local etiketler = self:SerializeTags(instance)
            if etiketler then
                nodeData.tags = etiketler
            end
        end)
        
        table.insert(instances, nodeData)
    end
    
    -- Iterate through all configured services
    for _, service in ipairs(servicesToSync) do
        for _, descendant in ipairs(service:GetDescendants()) do
            serializeNode(descendant)
        end
    end
    
    local snapshotPatch = {
        event_type = "FULL_SYNC",
        version = "v1",
        data = {
            instances = instances,
            place_key = PlaceKimligi.Al(),
            place_id = tostring(game.PlaceId),
            place_name = game.Name,
        }
    }
    
    return snapshotPatch
end

return PatchBuilder
