-- Saved by UniversalSynSaveInstance (Join to Copy Games) https://discord.gg/wx4ThpAsmw

local l_TweenService_0 = game:GetService("TweenService");
local l_ReplicatedStorage_0 = game:GetService("ReplicatedStorage");
local v2 = TweenInfo.new(0.2, Enum.EasingStyle.Sine, Enum.EasingDirection.Out, 0, false, 0);
local l_Debris_0 = game:GetService("Debris");
local l_Parent_0 = script.Parent.Parent.Parent;
local l_LocalPlayer_0 = game.Players.LocalPlayer;
local v7 = require(game.ReplicatedStorage.Top_Text);
local v8 = require(game.ReplicatedStorage.NPC_MOD);
local _ = require(game.ReplicatedStorage.Item_Module);
local v10 = require(game.ReplicatedStorage.Comma_Module);
local v11 = nil;
local v12 = false;
local v13 = true;
local v14 = false;
local v15 = nil;
local function v18(v16) --[[ Line: 29 ]] --[[ Name: waitForDuration ]]
    -- upvalues: v14 (ref)
    local v17 = time();
    while true do
        if time() - v17 < v16 then
            task.wait();
            if v14 then
                return false;
            end;
        else
            return true;
        end;
    end;
end;
local function v19() --[[ Line: 41 ]] --[[ Name: cancelYes ]]
    -- upvalues: v14 (ref), v13 (ref), v7 (copy), l_Parent_0 (copy), l_LocalPlayer_0 (copy), v11 (ref), l_TweenService_0 (copy), v2 (copy), l_Debris_0 (copy), v8 (copy), v15 (ref)
    v14 = true;
    v13 = true;
    v7.TakeAwayResponses(l_Parent_0, l_LocalPlayer_0);
    if v11 then
        l_TweenService_0:Create(v11, v2, {
            OutlineTransparency = 1
        }):Play();
        l_Debris_0:AddItem(v11, v2.Time);
    end;
    v8.End_Speak(l_LocalPlayer_0);
    v15 = nil;
    task.wait(0.7);
    script.Parent.Enabled = true;
end;
local function v25(v20, v21) --[[ Line: 57 ]] --[[ Name: getSellableTools ]]
    v21 = v21 or false;
    local v22 = {};
    if v20 then
        for _, v24 in pairs(v20:GetChildren()) do
            if v24:IsA("Tool") and v24:FindFirstChild("Item_String") and (v24:GetAttribute("Favorite") ~= true or v21) then
                table.insert(v22, v24);
            end;
        end;
    end;
    return v22;
end;
local v26 = require(l_ReplicatedStorage_0.Modules.CalculateToolValue);
local function v35(player, v28, v29, v30) --[[ Line: 84 ]] --[[ Name: processSale ]]
    -- upvalues: v26 (copy)
    local v31 = 0;
    for _, v33 in v28 do
        local v34 = v26(v33);
        if v29 then
            v31 = v31 + v34;
        end;
	end;
	if player:GetAttribute("VIP") then
		v31 = v31 * 2
	end
    if v30 and v31 > 0 then
        v30:FireServer();
    end;
    return v31;
end;
local function v77() --[[ Line: 102 ]] --[[ Name: yes ]]
    -- upvalues: v13 (ref), v12 (ref), v15 (ref), l_LocalPlayer_0 (copy), v8 (copy), v11 (ref), l_TweenService_0 (copy), v2 (copy), l_Debris_0 (copy), v14 (ref), v7 (copy), l_Parent_0 (copy), v18 (copy), v19 (copy), v25 (copy), v26 (copy), v10 (copy), v35 (copy)
    if not v13 then
        return;
    else
        v13 = false;
        v12 = true;
        v15 = l_LocalPlayer_0;
        v8.Start_Speak(l_LocalPlayer_0);
        if v11 then
            l_TweenService_0:Create(v11, v2, {
                OutlineTransparency = 1
            }):Play();
            l_Debris_0:AddItem(v11, v2.Time);
        end;
        v14 = false;
        script.Parent.Enabled = false;
        v7.NpcText(l_Parent_0, "Got anything to sell?", false);
        if not v18(math.random(1, 1.5)) then
            return v19();
        else
            local v36 = {
                "I want to sell my inventory", 
                "I want to sell this", 
                "How much is this worth?", 
                "Nevermind"
            };
            if workspace:GetAttribute("InTutorial") then
                v36 = {
                    "I want to sell my inventory"
                };
            end;
            local v37 = v7.ShowChoices(l_LocalPlayer_0, v36);
            if v14 then
                return v19();
            else
                local v38 = "";
                local v39 = "";
                local v40 = {};
                for _, v42 in pairs(v37) do
                    local l_Frame_0 = v42:FindFirstChild("Frame");
                    if l_Frame_0 then
                        local l_ImageButton_0 = l_Frame_0:FindFirstChild("ImageButton");
                        if l_ImageButton_0 then
                            table.insert(v40, (l_ImageButton_0.MouseButton1Click:Connect(function() --[[ Line: 152 ]]
                                -- upvalues: v39 (ref), l_Frame_0 (copy), v38 (ref)
                                v39 = l_Frame_0.Frame.Text_Element:GetAttribute("Text");
                                if v39 == "I want to sell my inventory" then
                                    v38 = "Sell Inventory";
                                    return;
                                elseif v39 == "I want to sell this" then
                                    v38 = "Sell This";
                                    return;
                                elseif v39 == "How much is this worth?" then
                                    v38 = "How";
                                    return;
                                elseif v39 == "Tell me about this" then
                                    v38 = "Tell me";
                                    return;
                                else
                                    v38 = "Nevermind";
                                    return;
                                end;
                            end)));
                        end;
                    end;
                end;
                while true do
                    if v38 == "" then
                        task.wait();
                        if v14 then
                            for _, v46 in pairs(v40) do
                                v46:Disconnect();
                            end;
                            return v19();
                        end;
                    else
                        for _, v48 in pairs(v40) do
                            v48:Disconnect();
                        end;
                        if v14 then
                            return v19();
                        else
                            v7.RemovePlayerSideFrame(l_LocalPlayer_0);
                            if l_LocalPlayer_0.Character then
                                v7.PlayerResponse(l_LocalPlayer_0.Character, v39, true);
                                if not v18((math.sqrt(string.len(v39) * 0.07))) then
                                    return v19();
                                else
                                    if v38 == "Sell Inventory" then
                                        local v49 = v25(l_LocalPlayer_0.Character);
                                        local v50 = v25(l_LocalPlayer_0.Backpack);
                                        local v51 = {};
                                        for _, v53 in pairs(v49) do
                                            table.insert(v51, v53);
                                        end;
                                        for _, v55 in pairs(v50) do
                                            table.insert(v51, v55);
                                        end;
                                        local v56 = 0;
                                        for _, v58 in pairs(v51) do
                                            v56 = v56 + v26(v58);
                                        end;
                                        if v56 > 0 then
                                            local v59 = "Here is <font color='#FFFF00'>" .. v10.Comma((tostring(v56))) .. "\194\162</font>";
                                            v7.NpcText(l_Parent_0, v59, true);
                                            v35(l_LocalPlayer_0, v51, true, game.ReplicatedStorage.GameEvents.Sell_Inventory);
                                            v18(math.random(0.3, 0.4));
                                        else
                                            v7.NpcText(l_Parent_0, "Nothing to buy...", true);
                                        end;
                                    elseif v38 == "Sell This" then
                                        local v60 = v25(l_LocalPlayer_0.Character);
                                        local v61 = 0;
                                        for _, v63 in pairs(v60) do
                                            v61 = v61 + math.round((v26(v63)));
                                        end;
                                        if v61 > 0 then
                                            local v64 = "Here is <font color='#FFFF00'>" .. v10.Comma((tostring(v61))) .. "\194\162</font>";
                                            v7.NpcText(l_Parent_0, v64, true);
                                            v35(l_LocalPlayer_0, v60, true, game.ReplicatedStorage.GameEvents.Sell_Item);
                                            v18(math.random(0.3, 0.4));
                                        else
                                            v7.NpcText(l_Parent_0, "Nothing to buy...", true);
                                        end;
                                    elseif v38 == "How" then
                                        local v65 = v25(l_LocalPlayer_0.Character, true);
                                        local v66 = 0;
                                        for _, v68 in pairs(v65) do
                                            v66 = v66 + v26(v68);
                                        end;
                                        if v66 > 0 then
                                            local v69 = "I'd value it at <font color='#FFFF00'>" .. v10.Comma((tostring(v66))) .. "\194\162</font>";
                                            v7.NpcText(l_Parent_0, v69, true);
                                            v18(math.random(0.7, 0.8));
                                        else
                                            v7.NpcText(l_Parent_0, "I don't see anything...", true);
                                        end;
                                    elseif v38 == "Tell me" then
                                        local l_Character_0 = l_LocalPlayer_0.Character;
                                        local v71 = false;
                                        if l_Character_0 then
                                            for _, v73 in pairs(l_Character_0:GetChildren()) do
                                                if v73:IsA("Tool") then
                                                    local l_Name_0 = v73.Name;
                                                    local v75 = string.sub(l_Name_0:lower(), 1, 1);
                                                    local v76 = (not (v75 ~= "a" and v75 ~= "e" and v75 ~= "i" and v75 ~= "o") or v75 == "u") and "an" or "a";
                                                    v7.NpcText(l_Parent_0, "You're holding " .. v76 .. " " .. l_Name_0 .. ".", true);
                                                    v71 = true;
                                                    v18(math.random(0.7, 0.8));
                                                    break;
                                                end;
                                            end;
                                        end;
                                        if not v71 then
                                            v7.NpcText(l_Parent_0, "You're not holding anything right now.", true);
                                        end;
                                    else
                                        v7.NpcText(l_Parent_0, "Goodbye!", true);
                                    end;
                                    v12 = false;
                                    if not v18(math.random(0.4, 0.5)) then
                                        return v19();
                                    else
                                        task.wait(0.5);
                                        v7.TakeAwayResponses(l_Parent_0, l_LocalPlayer_0);
                                        v18(0.5);
                                        v13 = true;
                                        script.Parent.Enabled = true;
                                        v8.End_Speak(l_LocalPlayer_0);
                                        return;
                                    end;
                                end;
                            else
                                return v19();
                            end;
                        end;
                    end;
                end;
            end;
        end;
    end;
end;
script.Parent.Triggered:Connect(function() --[[ Line: 300 ]]
    -- upvalues: v8 (copy), l_LocalPlayer_0 (copy), v77 (copy)
    if v8.Can_Speak(l_LocalPlayer_0) then
        v77();
    end;
end);
local l_Click_0 = game.SoundService.Click;
script.Parent.PromptButtonHoldBegan:Connect(function() --[[ Line: 308 ]]
    -- upvalues: l_Click_0 (copy)
    l_Click_0.PlaybackSpeed = 1 + math.random(-15, 15) / 100;
    l_Click_0.Playing = true;
    l_Click_0.TimePosition = 0;
end);
local l_Position_0 = script.Parent.Parent.Position;
task.spawn(function() --[[ Line: 316 ]]
    -- upvalues: v15 (ref), l_Position_0 (copy), v13 (ref), v12 (ref), v7 (copy), l_LocalPlayer_0 (copy), l_Parent_0 (copy), v8 (copy), v19 (copy)
    while true do
        task.wait(0.1);
        if v15 and v15.Character and v15.Character.PrimaryPart and (v15.Character.PrimaryPart.Position - l_Position_0).Magnitude > 17 and not v13 and v12 then
            v7.RemovePlayerSideFrame(l_LocalPlayer_0);
            v7.NpcText(l_Parent_0, "Goodbye!", true);
            v8.End_Speak(l_LocalPlayer_0);
            v19();
        end;
    end;
end);
script.Parent.PromptShown:Connect(function() --[[ Line: 333 ]]
    -- upvalues: v11 (ref), l_Parent_0 (copy), l_TweenService_0 (copy), v2 (copy)
    v11 = Instance.new("Highlight", l_Parent_0);
    v11.DepthMode = Enum.HighlightDepthMode.Occluded;
    v11.FillTransparency = 1;
    v11.OutlineTransparency = 1;
    v11.Adornee = l_Parent_0;
    l_TweenService_0:Create(v11, v2, {
        OutlineTransparency = 0
    }):Play();
end);
script.Parent.PromptHidden:Connect(function() --[[ Line: 344 ]]
    -- upvalues: v11 (ref), l_TweenService_0 (copy), v2 (copy), l_Debris_0 (copy)
    if v11 then
        l_TweenService_0:Create(v11, v2, {
            OutlineTransparency = 1
        }):Play();
        l_Debris_0:AddItem(v11, v2.Time);
    end;
end);