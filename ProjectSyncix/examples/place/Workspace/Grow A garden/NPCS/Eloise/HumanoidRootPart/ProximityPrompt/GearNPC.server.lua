-- Saved by UniversalSynSaveInstance (Join to Copy Games) https://discord.gg/wx4ThpAsmw

local _ = game:GetService("ReplicatedStorage");
local l_ReplicatedStorage_1 = game:GetService("ReplicatedStorage");
local _ = l_ReplicatedStorage_1:WaitForChild("GameEvents");
local v3 = nil;
local l_Parent_0 = script.Parent.Parent.Parent;
local l_TweenService_0 = game:GetService("TweenService");
local v6 = TweenInfo.new(0.2, Enum.EasingStyle.Sine, Enum.EasingDirection.Out, 0, false, 0);
local v7 = true;
local v8 = require(game.ReplicatedStorage.Top_Text);
local _ = script.Parent.Parent.Exit_Detection;
local l_LocalPlayer_0 = game.Players.LocalPlayer;
local v11 = false;
local v12 = false;
local v13 = require(game.ReplicatedStorage.NPC_MOD);
local v14 = require(l_ReplicatedStorage_1.Modules.GuiController);
local v15 = nil;
cancelYes = function() --[[ Line: 22 ]] --[[ Name: cancelYes ]]
    -- upvalues: v12 (ref), v7 (ref), v8 (copy), l_Parent_0 (copy), l_LocalPlayer_0 (copy), v3 (ref), l_TweenService_0 (copy), v6 (copy), v13 (copy), v15 (ref), v11 (ref)
    v12 = true;
    v7 = true;
    v8.TakeAwayResponses(l_Parent_0, l_LocalPlayer_0);
    if v3 then
        l_TweenService_0:Create(v3, v6, {
            OutlineTransparency = 1
        }):Play();
        game.Debris:AddItem(v3, v6.Time);
    end;
    v13.End_Speak(game.Players.LocalPlayer);
    v15 = nil;
    v11 = false;
    task.wait(0.5);
    script.Parent.Enabled = true;
end;
local _ = {};
yes = function() --[[ Line: 42 ]] --[[ Name: yes ]]
    -- upvalues: v7 (ref), v11 (ref), v15 (ref), v13 (copy), v3 (ref), l_TweenService_0 (copy), v6 (copy), v12 (ref), v8 (copy), l_Parent_0 (copy), l_LocalPlayer_0 (copy), v14 (copy)
    for _, v18 in pairs(script.Parent.Parent.Parent.Head:GetChildren()) do
        if v18:IsA("BillboardGui") then
            v18.Enabled = false;
        end;
    end;
    if v7 == true then
        v7 = false;
        v11 = true;
        v15 = game.Players.LocalPlayer;
        v13.Start_Speak(game.Players.LocalPlayer);
        if v3 and v3 ~= nil then
            l_TweenService_0:Create(v3, v6, {
                OutlineTransparency = 1
            }):Play();
            game.Debris:AddItem(v3, v6.Time);
        end;
        v12 = false;
        script.Parent.Enabled = false;
        if v3 then
            l_TweenService_0:Create(v3, v6, {
                OutlineTransparency = 1
            }):Play();
            game.Debris:AddItem(v3, v6.Time);
        end;
        v8.NpcText(l_Parent_0, "Welcome to my stand!", false);
        task.wait(1);
        local v19 = v8.ShowChoices(l_LocalPlayer_0, {
            "Show me the gear shop", 
            "Show me daily quests", 
            "Nevermind"
        });
        if v12 then
            return cancelYes();
        else
            local v20 = "";
            local v21 = "";
            local v22 = {};
            for _, v24 in pairs(v19) do
                local l_Frame_0 = v24:FindFirstChild("Frame");
                if l_Frame_0 then
                    local l_ImageButton_0 = l_Frame_0:FindFirstChild("ImageButton");
                    if l_ImageButton_0 then
                        table.insert(v22, (l_ImageButton_0.MouseButton1Click:Connect(function() --[[ Line: 90 ]]
                            -- upvalues: v21 (ref), l_Frame_0 (copy), v20 (ref)
                            v21 = l_Frame_0.Frame.Text_Element:GetAttribute("Text");
                            if v21 == "Show me the gear shop" then
                                v20 = "Gear";
                                return;
                            elseif v21 == "Show me daily quests" then
                                v20 = "Daily Quest";
                                return;
                            elseif v21 == "Show me pet eggs" then
                                v20 = "Show";
                                return;
                            else
                                v20 = "Nevermind";
                                return;
                            end;
                        end)));
                    end;
                end;
            end;
            while true do
                if v20 == "" then
                    task.wait();
                    if v12 then
                        for _, v28 in pairs(v22) do
                            v28:Disconnect();
                        end;
                        return cancelYes();
                    end;
                else
                    for _, lol in pairs(v22) do
                        lol:Disconnect();
                    end;
                    if v12 then
                        return cancelYes();
                    else
                        v8.RemovePlayerSideFrame(l_LocalPlayer_0);
                        if l_LocalPlayer_0.Character then
                            v8.PlayerResponse(l_LocalPlayer_0.Character, v21, true);
                            if v20 == "Gear" then
                                task.wait(0.5);
                                local _ = v8.NpcText(l_Parent_0, "Here is the gear shop", true);
                                local v32 = 0;
                                local v33 = time();
                                while v32 < 0.46243556529821417 and v12 == false do
                                    task.wait();
                                    v32 = time() - v33;
                                end;
                                v14:Open(l_LocalPlayer_0.PlayerGui.Gear_Shop);
                                if v12 then
                                    return cancelYes();
                                else
                                    local v34 = 0;
                                    local v35 = time();
                                    while v34 < 0.21243556529821417 and v12 == false do
                                        task.wait();
                                        v34 = time() - v35;
                                    end;
                                    if v12 then
                                        return cancelYes();
                                    else
                                        v8.TakeAwayResponses(script.Parent.Parent.Parent, game.Players.LocalPlayer);
                                        v7 = true;
                                        v11 = false;
                                        script.Parent.Enabled = true;
                                        v13.End_Speak(game.Players.LocalPlayer);
                                    end;
                                end;
                            elseif v20 == "Daily Quest" then
                                v14:Open(l_LocalPlayer_0.PlayerGui.DailyQuests_UI);
                            else
                                v8.NpcText(l_Parent_0, "...", true);
                            end;
                            v11 = false;
                            task.wait(0.5);
                            v8.TakeAwayResponses(l_Parent_0, l_LocalPlayer_0);
                            task.wait(0.5);
                            v7 = true;
                            script.Parent.Enabled = true;
                            v13.End_Speak(l_LocalPlayer_0);
                            --[[ close >= 2 ]]
                            break;
                        else
                            return cancelYes();
                        end;
                    end;
                end;
            end;
        end;
    end;
end;
script.Parent.Triggered:Connect(function() --[[ Line: 178 ]]
    -- upvalues: v13 (copy), l_LocalPlayer_0 (copy)
    if v13.Can_Speak(l_LocalPlayer_0) == true then
        yes(l_LocalPlayer_0);
    end;
end);
local l_Click_0 = game.SoundService.Click;
script.Parent.PromptButtonHoldBegan:Connect(function() --[[ Line: 185 ]]
    -- upvalues: l_Click_0 (copy)
    l_Click_0.PlaybackSpeed = 1 + math.random(-15, 15) / 100;
    l_Click_0.Playing = true;
    l_Click_0.TimePosition = 0;
end);
local l_Position_0 = script.Parent.Parent.Position;
task.spawn(function() --[[ Line: 191 ]]
    -- upvalues: v15 (ref), l_Position_0 (copy), v7 (ref), v11 (ref), v8 (copy), l_LocalPlayer_0 (copy), l_Parent_0 (copy), v13 (copy)
    while true do
        task.wait(1);
        if v15 and v15.Character then
            local l_PrimaryPart_0 = v15.Character.PrimaryPart;
            if l_PrimaryPart_0 and (l_PrimaryPart_0.Position - l_Position_0).Magnitude > 10 and v7 == false and v11 == true then
                v8.RemovePlayerSideFrame(l_LocalPlayer_0);
                v8.NpcText(l_Parent_0, "...", true);
                v13.End_Speak(game.Players.LocalPlayer);
                cancelYes();
            end;
        end;
    end;
end);
script.Parent.PromptShown:Connect(function() --[[ Line: 212 ]]
    -- upvalues: v3 (ref), l_Parent_0 (copy), l_TweenService_0 (copy), v6 (copy)
    v3 = Instance.new("Highlight");
    v3.DepthMode = Enum.HighlightDepthMode.Occluded;
    v3.FillTransparency = 1;
    v3.OutlineTransparency = 1;
    v3.Adornee = l_Parent_0;
    v3.Parent = l_Parent_0;
    l_TweenService_0:Create(v3, v6, {
        OutlineTransparency = 0
    }):Play();
end);
script.Parent.PromptHidden:Connect(function() --[[ Line: 223 ]]
    -- upvalues: v3 (ref), l_TweenService_0 (copy), v6 (copy)
    if v3 then
        l_TweenService_0:Create(v3, v6, {
            OutlineTransparency = 1
        }):Play();
        game.Debris:AddItem(v3, v6.Time);
    end;
end);