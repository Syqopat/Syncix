local Workspace = game:GetService("Workspace")
local CollectionService = game:GetService("CollectionService")

local PatchExecutor = {}
PatchExecutor.__index = PatchExecutor

function PatchExecutor.new()
    local self = setmetatable({}, PatchExecutor)
    return self
end

function PatchExecutor:OnStart(container)
    self.cache = container:Get("RuntimeCache")
    self.echoGuard = container:Get("EchoGuard")
    self.activityLog = container:Get("ActivityLog")
    self.subscriptions = container:Get("SubscriptionManager")
    self.genericObserver = container:Get("GenericObserver")
    self.selectionObserver = container:Get("SelectionObserver")
end

function PatchExecutor:ApplyFullNode(nodeData: any)
    local uuid = nodeData.syncix_id
    local className = nodeData.class_name
    local name = nodeData.name
    local props = nodeData.properties
    local parentId = nodeData.parent
    
    local targetParent = Workspace
    if parentId and parentId ~= "" then
        local pInst = self.cache:GetInstance(parentId)
        if pInst then
            targetParent = pInst
        end
    end
    
    local instance = self.cache:GetInstance(uuid)
    
    if not instance then
        local success, newInst = pcall(function() return Instance.new(className) end)
        if not success or not newInst then
            return
        end
        
        instance = newInst
        instance.Name = name
        pcall(function()
            instance:SetAttribute("__syncix_id", uuid)
        end)
        
        self.cache:CacheInstance(uuid, instance)
        
        local parentSuccess = pcall(function()
            instance.Parent = targetParent 
        end)

        if not parentSuccess then
            pcall(function() instance:Destroy() end)
            return
        end
        
        self.genericObserver:HandleInstanceAdded(instance)
    else
        instance.Name = name
        if instance.Parent ~= targetParent then
            pcall(function()
                instance.Parent = targetParent
            end)
        end
    end
    
    if props then
        for propName, propValue in pairs(props) do
            self:ApplyPropertyValue(instance, propName, propValue)
        end
    end
end

function PatchExecutor:ApplyPatch(patch: any)
    local uuid = patch.data and (patch.data.syncix_id or patch.data.id)

    if patch.event_type == "CREATE" then
        if uuid and self.cache:GetInstance(uuid) then
            return
        end

        local ok, newInst = pcall(function()
            return Instance.new(patch.data.class_name)
        end)
        if not ok or not newInst then
            return
        end

        newInst.Name = patch.data.name or patch.data.class_name
        if uuid then
            pcall(function()
                newInst:SetAttribute("__syncix_id", uuid)
            end)
            self.cache:CacheInstance(uuid, newInst)
        end

        local targetParent = Workspace
        if patch.data.parent then
            local pInst = self.cache:GetInstance(patch.data.parent)
            if pInst then
                targetParent = pInst
            end
        end
        pcall(function()
            newInst.Parent = targetParent
        end)

        if self.activityLog then
            self.activityLog:Inbound("create", newInst.Name, nil, newInst.ClassName, uuid)
        end
        return
    end

    -- SECIM bir instance'a bagli DEGIL: uuid tasimiyor, cunku hangi objelerin
    -- secili oldugu global bir durum. Bu yuzden hem "uuid yoksa cik" hem de
    -- instance cozumlemesinden ONCE ele alinmali. Ilk denememde uuid
    -- kontrolunun bir row ALTINA koymustum ve secim sessizce dusuyordu.
    if patch.event_type == "SELECTION_UPDATE" then
        if self.selectionObserver then
            self.selectionObserver:Apply(patch.data and patch.data.ids or {})
        end
        return
    end

    if not uuid then return end
    local instance = self.cache:GetInstance(uuid)
    if not instance then return end

    if patch.event_type == "PROPERTY_UPDATE" then
        self:ApplyPropertyValue(instance, patch.data.property, patch.data.value)
    elseif patch.event_type == "RENAME_INSTANCE" or patch.event_type == "RENAME" then
        local newName = patch.data.newName or patch.data.name or patch.data.value
        if newName then
            if self.echoGuard then self.echoGuard:Expect(uuid, "Name", newName) end
            if self.activityLog then
                self.activityLog:Inbound("rename", instance.Name, "Name", newName, uuid)
            end
            pcall(function()
                instance.Name = newName
            end)
        end
    elseif patch.event_type == "ATTRIBUTE_UPDATE" then
        local name = patch.data.name
        if name then
            local resolved = self:DecodeValue(patch.data.value)
            if self.echoGuard then self.echoGuard:Expect(uuid, "@" .. name, resolved) end
            pcall(function()
                instance:SetAttribute(name, resolved)
            end)
        end
    elseif patch.event_type == "TAGS_UPDATE" then
        -- Etiket listesi butun halinde geliyor. Tek tek ekleme/silme takip
        -- etmek iki tarafta ayri durum tutmayi gerektirirdi; onun yerine
        -- current lookupSet requested kumeye getiriliyor.
        local requested = {}
        for _, t in ipairs(patch.data.tags or {}) do
            requested[t] = true
        end
        pcall(function()
            for _, current in ipairs(CollectionService:GetTags(instance)) do
                if not requested[current] then
                    CollectionService:RemoveTag(instance, current)
                end
            end
            for t in pairs(requested) do
                if not CollectionService:HasTag(instance, t) then
                    CollectionService:AddTag(instance, t)
                end
            end
        end)
    elseif patch.event_type == "REPARENT" then
        local newParentUuid = patch.data.parent
        if newParentUuid then
            local pInst = self.cache:GetInstance(newParentUuid)
            if pInst then
                if self.echoGuard then self.echoGuard:Expect(uuid, "__parent", newParentUuid) end
                pcall(function()
                    instance.Parent = pInst
                end)
            end
        end
    elseif patch.event_type == "DESTROY" then
        if self.activityLog then
            self.activityLog:Inbound("silme", instance.Name, nil, nil, uuid)
        end
        pcall(function()
            instance:Destroy()
        end)
        self.cache:Remove(uuid)
    end
end

--- Asset referansi. Eski property'ler (Decal.Texture, SoundId) duz text
--- kabul ediyor; fresh Content tipli olanlar etmiyor. Once Content olarak
--- denenir, o surum yoksa metne dusulur.
local function decodeContent(uri: string): any
    local ok, content = pcall(function()
        return (Content :: any).fromUri(uri)
    end)
    if ok and content then
        return content
    end
    return uri
end

local function decodeColorSequence(points: any): ColorSequence?
    if type(points) ~= "table" or #points == 0 then
        return nil
    end
    local keyNames = {}
    for _, k in ipairs(points) do
        table.insert(keyNames, ColorSequenceKeypoint.new(k.t, Color3.new(k.r, k.g, k.b)))
    end
    -- Roblox first noktanin 0, sonuncusunun 1 olmasini sart kosuyor ve
    -- siralanmamis listeyi reddediyor.
    table.sort(keyNames, function(a, b) return a.Time < b.Time end)
    local ok, array = pcall(ColorSequence.new, keyNames)
    return ok and array or nil
end

local function decodeNumberSequence(points: any): NumberSequence?
    if type(points) ~= "table" or #points == 0 then
        return nil
    end
    local keyNames = {}
    for _, k in ipairs(points) do
        table.insert(keyNames, NumberSequenceKeypoint.new(k.t, k.v, k.envelope or 0))
    end
    table.sort(keyNames, function(a, b) return a.Time < b.Time end)
    local ok, array = pcall(NumberSequence.new, keyNames)
    return ok and array or nil
end

--- Yazi tipi. Aile bir asset URI'si, kalinlik ve stil enum.
--- Metinden enum'a cevrim basarisiz olursa varsayilana dusulur; yanlis bir
--- datum atamaktansa Roblox'un varsayilani dogru behavior.
local function decodeFont(f: any): Font?
    local ok, ink = pcall(function()
        local weight = Enum.FontWeight.Regular
        local style = Enum.FontStyle.Normal
        for _, w in ipairs(Enum.FontWeight:GetEnumItems()) do
            if tostring(w) == f.weight then weight = w break end
        end
        for _, st in ipairs(Enum.FontStyle:GetEnumItems()) do
            if tostring(st) == f.style then style = st break end
        end
        return Font.new(f.family, weight, style)
    end)
    return ok and ink or nil
end

local function decodeEnum(propValue: any): any
    if type(propValue) == "string" and string.sub(propValue, 1, 5) == "Enum." then
        local parts = string.split(propValue, ".")
        if #parts == 3 then
            local ok, ev = pcall(function()
                return (Enum :: any)[parts[2]][parts[3]]
            end)
            if ok then return ev end
        end
    end
    return nil
end

local function decodeUDim2(val: any): any
    if type(val) == "string" then
        local parts = string.split(val, ",")
        if #parts == 4 then
            local sx, ox, sy, oy = tonumber(parts[1]), tonumber(parts[2]), tonumber(parts[3]), tonumber(parts[4])
            if sx and ox and sy and oy then
                return UDim2.new(sx, ox, sy, oy)
            end
        end
    elseif type(val) == "table" and val.UDim2 then
        return UDim2.new(val.UDim2.xs or 0, val.UDim2.xo or 0, val.UDim2.ys or 0, val.UDim2.yo or 0)
    end
    return nil
end

local function decodeVector2(val: any): any
    if type(val) == "string" then
        local parts = string.split(val, ",")
        if #parts == 2 then
            local x, y = tonumber(parts[1]), tonumber(parts[2])
            if x and y then
                return Vector2.new(x, y)
            end
        end
    elseif type(val) == "table" and val.Vector2 then
        return Vector2.new(val.Vector2.x or 0, val.Vector2.y or 0)
    end
    return nil
end

function PatchExecutor:DecodeValue(propValue: any): any
    if type(propValue) == "table" then
        if propValue.Vector3 then
            return Vector3.new(propValue.Vector3.x, propValue.Vector3.y, propValue.Vector3.z)
        elseif propValue.Color3 then
            return Color3.new(propValue.Color3.r, propValue.Color3.g, propValue.Color3.b)
        elseif propValue.UDim2 then
            return UDim2.new(propValue.UDim2.xs, propValue.UDim2.xo, propValue.UDim2.ys, propValue.UDim2.yo)
        elseif propValue.Vector2 then
            return Vector2.new(propValue.Vector2.x, propValue.Vector2.y)
        elseif propValue.UDim then
            return UDim.new(propValue.UDim.scale, propValue.UDim.offset)
        elseif propValue.CFrame then
            local p, r = propValue.CFrame.pos, propValue.CFrame.rot
            return CFrame.new(
                p[1], p[2], p[3],
                r[1], r[2], r[3],
                r[4], r[5], r[6],
                r[7], r[8], r[9]
            )
        elseif propValue.NumberRange then
            return NumberRange.new(propValue.NumberRange.min, propValue.NumberRange.max)
        elseif propValue.Number ~= nil then
            -- Eski bir core serde'nin etiketli bicimini gonderiyor olabilir
            -- ({"Number":0.5}). Yeni core'lar duz gonderiyor; bu dal yalnizca
            -- surum farkina karsi duruyor.
            return propValue.Number
        elseif propValue.String ~= nil then
            return propValue.String
        elseif propValue.Boolean ~= nil then
            return propValue.Boolean
        elseif propValue.BrickColor then
            return BrickColor.new(propValue.BrickColor)
        elseif propValue.Content ~= nil then
            return decodeContent(propValue.Content)
        elseif propValue.ColorSequence then
            return decodeColorSequence(propValue.ColorSequence)
        elseif propValue.NumberSequence then
            return decodeNumberSequence(propValue.NumberSequence)
        elseif propValue.Rect then
            local r = propValue.Rect
            return Rect.new(r.min[1], r.min[2], r.max[1], r.max[2])
        elseif propValue.Font then
            return decodeFont(propValue.Font)
        elseif propValue.PhysicalProperties then
            local p = propValue.PhysicalProperties
            return PhysicalProperties.new(
                p.density, p.friction, p.elasticity,
                p.frictionWeight, p.elasticityWeight
            )
        elseif propValue.Ref ~= nil then
            -- Bos dize "baglanti yok" demektir; nil dondurmek dogru behavior.
            if propValue.Ref == "" then
                return nil
            end
            return self.cache and self.cache:GetInstance(propValue.Ref) or nil
        end
        return nil
    end
    local enumVal = decodeEnum(propValue)
    if enumVal ~= nil then return enumVal end
    return propValue
end

-- Bağlaşık (türetilmiş) property'ler.
--
-- Roblox'ta bir property'yi yazmak kardeşlerini de değiştirir: Position yazıldığında
-- CFrame, Orientation ve Rotation da değişir ve her biri ayrı bir Changed sinyali
-- üretir. Echo koruması yalnızca yazdığımız property'yi beklediği için bu türetilmiş
-- sinyaller echo olarak core'a geri gidiyordu.
--
-- Ölçüm: 40 Position komutu -> 39 itemCount "Orientation" güncellemesi geri geldi.
-- Position'ın kendisi doğru şekilde eleniyordu, sızan yalnızca türetilmişlerdi.
local LINKED = {
    Position    = { "CFrame", "Orientation", "Rotation" },
    CFrame      = { "Position", "Orientation", "Rotation" },
    Orientation = { "CFrame", "Position", "Rotation" },
    Rotation    = { "CFrame", "Position", "Orientation" },
    Size        = { "CFrame" },
}

-- Yazımdan SONRA, bağlaşık property'lerin Roblox'un hesapladığı GÜNCEL değerlerini
-- beklenti olarak kaydeder. Değer birebir kaydedildiği için kullanıcının daha sonra
-- yaptığı gerçek değişiklikler farklı olur ve elenmez.
function PatchExecutor:_AwaitReferences(instance: Instance, propName: string)
    if not self.echoGuard then return end

    local siblings = LINKED[propName]
    if not siblings then return end

    local uuid = instance:GetAttribute("__syncix_id")
    if not uuid then return end

    for _, ad in ipairs(siblings) do
        pcall(function()
            local latest = (instance :: any)[ad]
            if latest ~= nil then
                self.echoGuard:Expect(uuid, ad, latest)
            end
        end)
    end
end

function PatchExecutor:ApplyPropertyValue(instance: Instance, propName: string, propValue: any)
    -- Echo koruması: uygulamadan ÖNCE "bu değeri ben yazıyorum" notu düşülür.
    -- Roblox'un property sinyalleri deferred olduğu için zamanlama tabanlı kilit
    -- yetmiyordu; gözlemci incoming değeri bu notla karşılaştırıp kendi yazımızı eler.
    if self.echoGuard then
        local uuid = instance:GetAttribute("__syncix_id")
        if uuid then
            local ok, resolved = pcall(function()
                return self:DecodeValue(propValue)
            end)
            self.echoGuard:Expect(uuid, propName, ok and resolved or propValue)
        end
    end

    -- Hatalar sessizce yutulmamalı: teşhis edilemeyen "değer uygulanmadı" sorunlarına yol açıyordu.
    local ok, err = pcall(function()
        if propName == "Contents" and instance:IsA("LocalizationTable") then
            -- Ceviri girdileri property ile degil SetEntries ile yazilir.
            local HttpService = game:GetService("HttpService")
            local inputs = HttpService:JSONDecode(propValue)
            ;(instance :: any):SetEntries(inputs)
        elseif propName == "Source" and instance:IsA("LuaSourceContainer") then
            instance.Source = propValue
            pcall(function()
                local ScriptEditorService = game:GetService("ScriptEditorService")
                local doc = ScriptEditorService:FindScriptDocumentAsync(instance)
                if doc then
                    doc:EditTextAsync(propValue, 1, 1, doc:GetLineCount(), doc:GetLineLength(doc:GetLineCount()) + 1)
                end
            end)
        elseif propName == "Parent" then
            if type(propValue) == "string" and propValue ~= "" then
                local pInst = self.cache:GetInstance(propValue)
                if pInst then
                    instance.Parent = pInst
                end
            elseif propValue == "" or propValue == "Workspace" then
                instance.Parent = Workspace
            end
        elseif type(propValue) == "table" then
            -- ÖNEMLİ: Tip kararı DEĞERE göre verilir, property ADINA göre DEĞİL.
            -- Eskiden "Size"/"Position" her timestamp UDim2 sanılıyordu; bu yüzden bir Part'ın
            -- Vector3 konumu çözümlenemiyor ve sessizce hiç uygulanmıyordu (objects 0,0,0'da kalıyordu).
            if propValue.Vector3 then
                instance[propName] = Vector3.new(propValue.Vector3.x, propValue.Vector3.y, propValue.Vector3.z)
            elseif propValue.Color3 then
                instance[propName] = Color3.new(propValue.Color3.r, propValue.Color3.g, propValue.Color3.b)
            elseif propValue.UDim2 then
                instance[propName] = UDim2.new(propValue.UDim2.xs, propValue.UDim2.xo, propValue.UDim2.ys, propValue.UDim2.yo)
            elseif propValue.Vector2 then
                instance[propName] = Vector2.new(propValue.Vector2.x, propValue.Vector2.y)
            elseif propValue.UDim then
                instance[propName] = UDim.new(propValue.UDim.scale, propValue.UDim.offset)
            elseif propValue.CFrame then
                local p, r = propValue.CFrame.pos, propValue.CFrame.rot
                instance[propName] = CFrame.new(
                    p[1], p[2], p[3],
                    r[1], r[2], r[3],
                    r[4], r[5], r[6],
                    r[7], r[8], r[9]
                )
            elseif propValue.NumberRange then
                instance[propName] = NumberRange.new(propValue.NumberRange.min, propValue.NumberRange.max)
            elseif propValue.Number ~= nil then
                instance[propName] = propValue.Number
            elseif propValue.String ~= nil then
                instance[propName] = propValue.String
            elseif propValue.Boolean ~= nil then
                instance[propName] = propValue.Boolean
            elseif propValue.BrickColor then
                instance[propName] = BrickColor.new(propValue.BrickColor)
            elseif propValue.Content ~= nil then
                instance[propName] = decodeContent(propValue.Content)
            elseif propValue.ColorSequence then
                instance[propName] = decodeColorSequence(propValue.ColorSequence)
            elseif propValue.NumberSequence then
                instance[propName] = decodeNumberSequence(propValue.NumberSequence)
            elseif propValue.Rect then
                local r = propValue.Rect
                instance[propName] = Rect.new(r.min[1], r.min[2], r.max[1], r.max[2])
            elseif propValue.Font then
                instance[propName] = decodeFont(propValue.Font)
            elseif propValue.PhysicalProperties then
                local p = propValue.PhysicalProperties
                instance[propName] = PhysicalProperties.new(
                    p.density, p.friction, p.elasticity,
                    p.frictionWeight, p.elasticityWeight
                )
            elseif propValue.Ref ~= nil then
                instance[propName] = (propValue.Ref ~= "")
                    and self.cache:GetInstance(propValue.Ref)
                    or nil
            else
                error("unsupported table value for property: " .. propName)
            end
        elseif type(propValue) == "string" and (propName == "Size" or propName == "Position") and instance:IsA("GuiObject") then
            -- GUI için text biçimli UDim2 ("0.5,0,0.5,0")
            local udim = decodeUDim2(propValue)
            if udim then
                instance[propName] = udim
            end
        elseif propName == "AnchorPoint" then
            local vec2 = decodeVector2(propValue)
            if vec2 then
                instance[propName] = vec2
            end
        else
            local enumVal = decodeEnum(propValue)
            if enumVal ~= nil then
                instance[propName] = enumVal
            else
                instance[propName] = propValue
            end
        end
    end)

    if ok then
        -- Yazım başarılıysa türetilmiş property'lerin fresh değerleri de beklenir.
        self:_AwaitReferences(instance, propName)

        -- Akış günlüğü: kullanıcı ne değiştiğini görebilsin ve çakışma varsa uyarılsın.
        if self.activityLog then
            local applied = nil
            pcall(function()
                applied = (instance :: any)[propName]
            end)
            self.activityLog:Inbound(
                "property",
                instance.Name,
                propName,
                applied,
                instance:GetAttribute("__syncix_id")
            )
        end
    else
        warn(string.format(
            "[Syncix] Could not apply property: %s.%s -> %s",
            instance:GetFullName(),
            tostring(propName),
            tostring(err)
        ))
    end
end

return PatchExecutor
