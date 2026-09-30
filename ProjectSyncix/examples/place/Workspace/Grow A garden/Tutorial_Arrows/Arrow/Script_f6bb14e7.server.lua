-- Saved by UniversalSynSaveInstance (Join to Copy Games) https://discord.gg/wx4ThpAsmw

local v0 = require(game.ReplicatedStorage.Arrow_Module);
local v1 = true;
script.Parent.Touched:Connect(function(v2) --[[ Line: 3 ]]
    -- upvalues: v1 (ref), v0 (copy)
    if v2.Parent.Name == script.Parent.Target_Player.Value and v1 == true then
        v1 = false;
        v0.Remove_Arrow(script.Parent);
    end;
end);