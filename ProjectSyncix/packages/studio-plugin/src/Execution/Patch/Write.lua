--[[
	Writing one property onto one instance: the decode, the echo expectation, the
	spelling correction and the warning that says what Studio refused.
]]

local Colour = require(script.Parent.Colour)
local Decode = require(script.Parent.Decode)
local Names = require(script.Parent.Names)

local colorFromText = Colour.colorFromText
local isColor3Property = Colour.isColor3Property
local decodeContent = Decode.decodeContent
local decodeColorSequence = Decode.decodeColorSequence
local decodeNumberSequence = Decode.decodeNumberSequence
local decodeFont = Decode.decodeFont
local decodeEnum = Decode.decodeEnum
local decodeUDim2 = Decode.decodeUDim2
local decodeVector2 = Decode.decodeVector2
local correctedName = Names.correctedName
local refusalHint = Names.refusalHint

local PatchExecutor = {}

function PatchExecutor:ApplyPropertyValue(instance: Instance, propName: string, propValue: any)
    -- A mesh is not a property that can be set: the part is created from it (see
    -- meshPartFrom). Writing it would only raise an error every time.
    if (propName == "MeshId" or propName == "MeshContent") and instance:IsA("MeshPart") then
        return
    end
    -- Turned into a Color3 before the echo note below, so the note holds the value
    -- Studio will actually report back.
    if type(propValue) == "string" then
        local color = colorFromText(propValue)
        if color and isColor3Property(instance, propName) then
            propValue = color
        end
    end

    -- Echo protection: BEFORE applying, an "I am writing this value" note is made.
    -- Roblox's property signals are deferred, so a timing-based lock
    -- was not enough; the observer compares the incoming value with this note and filters our own write.
    if self.echoGuard then
        local uuid = instance:GetAttribute("__syncix_id")
        if uuid then
            local ok, resolved = pcall(function()
                return self:DecodeValue(propValue)
            end)
            local expectedVal = ok and resolved or propValue
            if propName == "CFrame" and typeof(expectedVal) == "Vector3" then
                expectedVal = CFrame.new(expectedVal)
            end
            self.echoGuard:Expect(uuid, propName, expectedVal)
        end
    end

    -- Errors must not be swallowed silently: they led to undiagnosable "value not applied" problems.
    local ok, err = pcall(function()
        if propName == "Contents" and instance:IsA("LocalizationTable") then
            -- Translation entries are written with SetEntries, not as a property.
            local HttpService = game:GetService("HttpService")
            local inputs = HttpService:JSONDecode(propValue)
            ;(instance :: any):SetEntries(inputs)
        elseif propName == "Source" and instance:IsA("LuaSourceContainer") then
            -- UpdateSourceAsync is the way Roblox asks plugins to change a script: it also
            -- works for a script open in the editor and under Team Create's Collaborative
            -- Editing, where setting Source directly may be refused or overwritten by the
            -- shared document. The direct write stays as the fallback.
            local ScriptEditorService = game:GetService("ScriptEditorService")
            local updated = pcall(function()
                ScriptEditorService:UpdateSourceAsync(instance, function()
                    return propValue
                end)
            end)
            if not updated then
                instance.Source = propValue
                pcall(function()
                    local doc = ScriptEditorService:FindScriptDocumentAsync(instance)
                    if doc then
                        doc:EditTextAsync(propValue, 1, 1, doc:GetLineCount(), doc:GetLineLength(doc:GetLineCount()) + 1)
                    end
                end)
            end
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
            -- IMPORTANT: the type is decided by the VALUE, NOT by the property NAME.
            -- "Size"/"Position" used to be taken for UDim2 every time, so a Part's
            -- Vector3 position could not be resolved and was silently never applied (objects stayed at 0,0,0).
            if propValue.Vector3 then
                if propName == "CFrame" then
                    instance.CFrame = CFrame.new(propValue.Vector3.x, propValue.Vector3.y, propValue.Vector3.z)
                else
                    instance[propName] = Vector3.new(propValue.Vector3.x, propValue.Vector3.y, propValue.Vector3.z)
                end
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
                local str = propValue.String
                local enumVal = decodeEnum(str)
                if enumVal ~= nil then
                    local ok2 = pcall(function()
                        instance[propName] = enumVal
                    end)
                    if not ok2 then
                        local shortName = string.gsub(str, "^Enum%..+%.", "")
                        local ok3 = pcall(function()
                            instance[propName] = shortName
                        end)
                        if not ok3 then
                            instance[propName] = str
                        end
                    end
                else
                    instance[propName] = str
                end
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
            -- UDim2 in text form for GUI ("0.5,0,0.5,0")
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
                local ok2 = pcall(function()
                    instance[propName] = enumVal
                end)
                if not ok2 then
                    if type(propValue) == "string" then
                        local shortName = string.gsub(propValue, "^Enum%..+%.", "")
                        local ok3 = pcall(function()
                            instance[propName] = shortName
                        end)
                        if not ok3 then
                            instance[propName] = propValue
                        end
                    else
                        instance[propName] = propValue
                    end
                end
            elseif propName == "CFrame" and typeof(propValue) == "Vector3" then
                instance.CFrame = CFrame.new(propValue)
            else
                instance[propName] = propValue
            end
        end
    end)

    if ok then
        -- If the write succeeded, the new values of derived properties are expected too.
        self:_AwaitReferences(instance, propName)

        -- Activity log: so the user can see what changed and is warned about conflicts.
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
        -- Retried once under the right name; the retry cannot correct again because
        -- the right name is its own spelling.
        local realName = correctedName(instance, propName)
        if realName then
            self:ApplyPropertyValue(instance, realName, propValue)
            return
        end
        warn(string.format(
            "[Syncix] Could not apply property: %s.%s -> %s%s",
            instance:GetFullName(),
            tostring(propName),
            tostring(err),
            refusalHint(instance, propName, propValue)
        ))
    end
end

return PatchExecutor.ApplyPropertyValue
