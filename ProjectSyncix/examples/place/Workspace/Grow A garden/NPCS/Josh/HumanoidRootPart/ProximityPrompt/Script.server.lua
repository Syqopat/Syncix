-- Saved by UniversalSynSaveInstance (Join to Copy Games) https://discord.gg/wx4ThpAsmw

local v0 = nil;
local l_Parent_0 = script.Parent.Parent.Parent;
local l_TweenService_0 = game:GetService("TweenService");
local l_ReplicatedStorage_0 = game:GetService("ReplicatedStorage");
local v4 = TweenInfo.new(0.2, Enum.EasingStyle.Sine, Enum.EasingDirection.Out, 0, false, 0);
local v5 = true;
local v6 = require(game.ReplicatedStorage.Top_Text);
local _ = script.Parent.Parent.Exit_Detection;
local l_LocalPlayer_0 = game.Players.LocalPlayer;
local v9 = false;
local v10 = false;
local v11 = require(game.ReplicatedStorage.NPC_MOD);
local v12 = require(l_ReplicatedStorage_0.Modules.GuiController);
local v13 = nil;
cancelYes = function() --[[ Line: 19 ]] --[[ Name: cancelYes ]]
    -- upvalues: v10 (ref), v5 (ref), v6 (copy), l_Parent_0 (copy), l_LocalPlayer_0 (copy), v0 (ref), l_TweenService_0 (copy), v4 (copy), v11 (copy), v13 (ref), v9 (ref)
    v10 = true;
    v5 = true;
    v6.TakeAwayResponses(l_Parent_0, l_LocalPlayer_0);
    if v0 then
        l_TweenService_0:Create(v0, v4, {
            OutlineTransparency = 1
        }):Play();
        game.Debris:AddItem(v0, v4.Time);
    end;
    v11.End_Speak(game.Players.LocalPlayer);
    v13 = nil;
    v9 = false;
    task.wait(0.5);
    script.Parent.Enabled = true;
end;
local _ = {};
yes = function() --[[ Line: 39 ]] --[[ Name: yes ]]
    -- upvalues: v5 (ref), v9 (ref), v13 (ref), v11 (copy), v0 (ref), l_TweenService_0 (copy), v4 (copy), v10 (ref), v6 (copy), l_Parent_0 (copy), v12 (copy), l_LocalPlayer_0 (copy)
    for _, v16 in pairs(script.Parent.Parent.Parent.Head:GetChildren()) do
        if v16:IsA("BillboardGui") then
            v16.Enabled = false;
        end;
    end;
    if v5 == true then
        v5 = false;
        v9 = true;
        v13 = game.Players.LocalPlayer;
        v11.Start_Speak(game.Players.LocalPlayer);
        if v0 and v0 ~= nil then
            l_TweenService_0:Create(v0, v4, {
                OutlineTransparency = 1
            }):Play();
            game.Debris:AddItem(v0, v4.Time);
        end;
        v10 = false;
        script.Parent.Enabled = false;
        if v0 then
            l_TweenService_0:Create(v0, v4, {
                OutlineTransparency = 1
            }):Play();
            game.Debris:AddItem(v0, v4.Time);
        end;
        local _ = v6.NpcText(l_Parent_0, "Here are the seeds that are in stock", true);
        local v18 = 0;
        local v19 = time();
        while v18 < 0.8374507866387546 and v10 == false do
            task.wait();
            v18 = time() - v19;
        end;
        v12:Open(l_LocalPlayer_0.PlayerGui.Seed_Shop);
        if v10 then
            return cancelYes();
        else
            local v20 = 0;
            local v21 = time();
            while v20 < 0.5874507866387546 and v10 == false do
                task.wait();
                v20 = time() - v21;
            end;
            if v10 then
                return cancelYes();
            else
                v6.TakeAwayResponses(script.Parent.Parent.Parent, game.Players.LocalPlayer);
                v5 = true;
                v9 = false;
                script.Parent.Enabled = true;
                v11.End_Speak(game.Players.LocalPlayer);
            end;
        end;
    end;
end;
script.Parent.Triggered:Connect(function() --[[ Line: 90 ]]
    -- upvalues: v11 (copy), l_LocalPlayer_0 (copy)
    if v11.Can_Speak(l_LocalPlayer_0) == true then
        yes(l_LocalPlayer_0);
    end;
end);
local l_Click_0 = game.SoundService.Click;
script.Parent.PromptButtonHoldBegan:Connect(function() --[[ Line: 97 ]]
    -- upvalues: l_Click_0 (copy)
    l_Click_0.PlaybackSpeed = 1 + math.random(-15, 15) / 100;
    l_Click_0.Playing = true;
    l_Click_0.TimePosition = 0;
end);
local l_Position_0 = script.Parent.Parent.Position;
task.spawn(function() --[[ Line: 103 ]]
    -- upvalues: v13 (ref), l_Position_0 (copy), v5 (ref), v9 (ref), v6 (copy), l_LocalPlayer_0 (copy), l_Parent_0 (copy), v11 (copy)
    while true do
        task.wait(1);
        if v13 and v13.Character then
            local l_PrimaryPart_0 = v13.Character.PrimaryPart;
            if l_PrimaryPart_0 and (l_PrimaryPart_0.Position - l_Position_0).Magnitude > 10 and v5 == false and v9 == true then
                v6.RemovePlayerSideFrame(l_LocalPlayer_0);
                v6.NpcText(l_Parent_0, "...", true);
                v11.End_Speak(game.Players.LocalPlayer);
                cancelYes();
            end;
        end;
    end;
end);
script.Parent.PromptShown:Connect(function() --[[ Line: 124 ]]
    -- upvalues: v0 (ref), l_Parent_0 (copy), l_TweenService_0 (copy), v4 (copy)
    v0 = Instance.new("Highlight");
    v0.DepthMode = Enum.HighlightDepthMode.Occluded;
    v0.FillTransparency = 1;
    v0.OutlineTransparency = 1;
    v0.Adornee = l_Parent_0;
    v0.Parent = l_Parent_0;
    l_TweenService_0:Create(v0, v4, {
        OutlineTransparency = 0
    }):Play();
end);
script.Parent.PromptHidden:Connect(function() --[[ Line: 135 ]]
    -- upvalues: v0 (ref), l_TweenService_0 (copy), v4 (copy)
    if v0 then
        l_TweenService_0:Create(v0, v4, {
            OutlineTransparency = 1
        }):Play();
        game.Debris:AddItem(v0, v4.Time);
    end;
end);