-- Saved by UniversalSynSaveInstance (Join to Copy Games) https://discord.gg/wx4ThpAsmw

local l_Parent_0 = script.Parent;
local l_Torso_0 = l_Parent_0:WaitForChild("Torso");
local v2 = l_Torso_0:WaitForChild("Right Shoulder");
local v3 = l_Torso_0:WaitForChild("Left Shoulder");
local v4 = l_Torso_0:WaitForChild("Right Hip");
local v5 = l_Torso_0:WaitForChild("Left Hip");
local _ = l_Torso_0:WaitForChild("Neck");
local l_Humanoid_0 = l_Parent_0:WaitForChild("Humanoid");
local v8 = "Standing";
local l_status_0, l_result_0 = pcall(function() --[[ Line: 15 ]]
    return UserSettings():IsUserFeatureEnabled("UserAnimateScaleRun");
end);
local v11 = l_status_0 and l_result_0;
local function _() --[[ Line: 18 ]] --[[ Name: getRigScale ]]
    -- upvalues: v11 (copy), l_Parent_0 (copy)
    if v11 then
        return l_Parent_0:GetScale();
    else
        return 1;
    end;
end;
local v13 = "";
local v14 = nil;
local v15 = nil;
local v16 = nil;
local v17 = 1;
local v18 = {};
local v19 = {
    idle = {
        {
            id = "http://www.roblox.com/asset/?id=180435571", 
            weight = 9
        }, 
        {
            id = "http://www.roblox.com/asset/?id=180435792", 
            weight = 1
        }
    }, 
    walk = {
        {
            id = "http://www.roblox.com/asset/?id=180426354", 
            weight = 10
        }
    }, 
    run = {
        {
            id = "run.xml", 
            weight = 10
        }
    }, 
    jump = {
        {
            id = "http://www.roblox.com/asset/?id=125750702", 
            weight = 10
        }
    }, 
    fall = {
        {
            id = "http://www.roblox.com/asset/?id=180436148", 
            weight = 10
        }
    }, 
    climb = {
        {
            id = "http://www.roblox.com/asset/?id=180436334", 
            weight = 10
        }
    }, 
    sit = {
        {
            id = "http://www.roblox.com/asset/?id=178130996", 
            weight = 10
        }
    }, 
    toolnone = {
        {
            id = "http://www.roblox.com/asset/?id=182393478", 
            weight = 10
        }
    }, 
    toolslash = {
        {
            id = "http://www.roblox.com/asset/?id=129967390", 
            weight = 10
        }
    }, 
    toollunge = {
        {
            id = "http://www.roblox.com/asset/?id=129967478", 
            weight = 10
        }
    }, 
    wave = {
        {
            id = "http://www.roblox.com/asset/?id=128777973", 
            weight = 10
        }
    }, 
    point = {
        {
            id = "http://www.roblox.com/asset/?id=128853357", 
            weight = 10
        }
    }, 
    dance1 = {
        {
            id = "http://www.roblox.com/asset/?id=182435998", 
            weight = 10
        }, 
        {
            id = "http://www.roblox.com/asset/?id=182491037", 
            weight = 10
        }, 
        {
            id = "http://www.roblox.com/asset/?id=182491065", 
            weight = 10
        }
    }, 
    dance2 = {
        {
            id = "http://www.roblox.com/asset/?id=182436842", 
            weight = 10
        }, 
        {
            id = "http://www.roblox.com/asset/?id=182491248", 
            weight = 10
        }, 
        {
            id = "http://www.roblox.com/asset/?id=182491277", 
            weight = 10
        }
    }, 
    dance3 = {
        {
            id = "http://www.roblox.com/asset/?id=182436935", 
            weight = 10
        }, 
        {
            id = "http://www.roblox.com/asset/?id=182491368", 
            weight = 10
        }, 
        {
            id = "http://www.roblox.com/asset/?id=182491423", 
            weight = 10
        }
    }, 
    laugh = {
        {
            id = "http://www.roblox.com/asset/?id=129423131", 
            weight = 10
        }
    }, 
    cheer = {
        {
            id = "http://www.roblox.com/asset/?id=129423030", 
            weight = 10
        }
    }
};
local v20 = {
    "dance1", 
    "dance2", 
    "dance3"
};
local v21 = {
    wave = false, 
    point = false, 
    dance1 = true, 
    dance2 = true, 
    dance3 = true, 
    laugh = false, 
    cheer = false
};
configureAnimationSet = function(v22, v23) --[[ Line: 98 ]] --[[ Name: configureAnimationSet ]]
    -- upvalues: v18 (copy)
    if v18[v22] ~= nil then
        for _, v25 in pairs(v18[v22].connections) do
            v25:disconnect();
        end;
    end;
    v18[v22] = {};
    v18[v22].count = 0;
    v18[v22].totalWeight = 0;
    v18[v22].connections = {};
    local l_script_FirstChild_0 = script:FindFirstChild(v22);
    if l_script_FirstChild_0 ~= nil then
        table.insert(v18[v22].connections, l_script_FirstChild_0.ChildAdded:connect(function(_) --[[ Line: 112 ]]
            -- upvalues: v22 (copy), v23 (copy)
            configureAnimationSet(v22, v23);
        end));
        table.insert(v18[v22].connections, l_script_FirstChild_0.ChildRemoved:connect(function(_) --[[ Line: 113 ]]
            -- upvalues: v22 (copy), v23 (copy)
            configureAnimationSet(v22, v23);
        end));
        local v29 = 1;
        for _, v31 in pairs(l_script_FirstChild_0:GetChildren()) do
            if v31:IsA("Animation") then
                table.insert(v18[v22].connections, v31.Changed:connect(function(_) --[[ Line: 117 ]]
                    -- upvalues: v22 (copy), v23 (copy)
                    configureAnimationSet(v22, v23);
                end));
                v18[v22][v29] = {};
                v18[v22][v29].anim = v31;
                local l_Weight_0 = v31:FindFirstChild("Weight");
                if l_Weight_0 == nil then
                    v18[v22][v29].weight = 1;
                else
                    v18[v22][v29].weight = l_Weight_0.Value;
                end;
                v18[v22].count = v18[v22].count + 1;
                v18[v22].totalWeight = v18[v22].totalWeight + v18[v22][v29].weight;
                v29 = v29 + 1;
            end;
        end;
    end;
    if v18[v22].count <= 0 then
        for v34, v35 in pairs(v23) do
            v18[v22][v34] = {};
            v18[v22][v34].anim = Instance.new("Animation");
            v18[v22][v34].anim.Name = v22;
            v18[v22][v34].anim.AnimationId = v35.id;
            v18[v22][v34].weight = v35.weight;
            v18[v22].count = v18[v22].count + 1;
            v18[v22].totalWeight = v18[v22].totalWeight + v35.weight;
        end;
    end;
end;
scriptChildModified = function(v36) --[[ Line: 148 ]] --[[ Name: scriptChildModified ]]
    -- upvalues: v19 (copy)
    local v37 = v19[v36.Name];
    if v37 ~= nil then
        configureAnimationSet(v36.Name, v37);
    end;
end;
script.ChildAdded:connect(scriptChildModified);
script.ChildRemoved:connect(scriptChildModified);
local v38 = if l_Humanoid_0 then l_Humanoid_0:FindFirstChildOfClass("Animator") else nil;
if v38 then
    local l_v38_PlayingAnimationTracks_0 = v38:GetPlayingAnimationTracks();
    for _, v41 in ipairs(l_v38_PlayingAnimationTracks_0) do
        v41:Stop(0);
        v41:Destroy();
    end;
end;
for v42, v43 in pairs(v19) do
    configureAnimationSet(v42, v43);
end;
local v44 = "None";
local v45 = 0;
local v46 = 0;
stopAllAnimations = function() --[[ Line: 189 ]] --[[ Name: stopAllAnimations ]]
    -- upvalues: v13 (ref), v21 (copy), v14 (ref), v16 (ref), v15 (ref)
    local l_v13_0 = v13;
    if v21[l_v13_0] ~= nil and v21[l_v13_0] == false then
        l_v13_0 = "idle";
    end;
    v13 = "";
    v14 = nil;
    if v16 ~= nil then
        v16:disconnect();
    end;
    if v15 ~= nil then
        v15:Stop();
        v15:Destroy();
        v15 = nil;
    end;
    return l_v13_0;
end;
setAnimationSpeed = function(v48) --[[ Line: 211 ]] --[[ Name: setAnimationSpeed ]]
    -- upvalues: v17 (ref), v15 (ref)
    if v48 ~= v17 then
        v17 = v48;
        v15:AdjustSpeed(v17);
    end;
end;
keyFrameReachedFunc = function(v49) --[[ Line: 218 ]] --[[ Name: keyFrameReachedFunc ]]
    -- upvalues: v13 (ref), v21 (copy), v17 (ref), l_Humanoid_0 (copy)
    if v49 == "End" then
        local l_v13_1 = v13;
        if v21[l_v13_1] ~= nil and v21[l_v13_1] == false then
            l_v13_1 = "idle";
        end;
        local l_v17_0 = v17;
        playAnimation(l_v13_1, 0, l_Humanoid_0);
        setAnimationSpeed(l_v17_0);
    end;
end;
playAnimation = function(v52, v53, v54) --[[ Line: 234 ]] --[[ Name: playAnimation ]]
    -- upvalues: v18 (copy), v14 (ref), v15 (ref), v17 (ref), v13 (ref), v16 (ref)
    local v55 = math.random(1, v18[v52].totalWeight);
    local _ = v55;
    local v57 = 1;
    while v18[v52][v57].weight < v55 do
        v55 = v55 - v18[v52][v57].weight;
        v57 = v57 + 1;
    end;
    local l_anim_0 = v18[v52][v57].anim;
    if l_anim_0 ~= v14 then
        if v15 ~= nil then
            v15:Stop(v53);
            v15:Destroy();
        end;
        v17 = 1;
        v15 = v54:LoadAnimation(l_anim_0);
        v15.Priority = Enum.AnimationPriority.Core;
        v15:Play(v53);
        v13 = v52;
        v14 = l_anim_0;
        if v16 ~= nil then
            v16:disconnect();
        end;
        v16 = v15.KeyframeReached:connect(keyFrameReachedFunc);
    end;
end;
local v59 = "";
local v60 = nil;
local v61 = nil;
local v62 = nil;
toolKeyFrameReachedFunc = function(v63) --[[ Line: 282 ]] --[[ Name: toolKeyFrameReachedFunc ]]
    -- upvalues: v59 (ref), l_Humanoid_0 (copy)
    if v63 == "End" then
        playToolAnimation(v59, 0, l_Humanoid_0);
    end;
end;
playToolAnimation = function(v64, v65, v66, v67) --[[ Line: 289 ]] --[[ Name: playToolAnimation ]]
    -- upvalues: v18 (copy), v61 (ref), v60 (ref), v59 (ref), v62 (ref)
    local v68 = math.random(1, v18[v64].totalWeight);
    local _ = v68;
    local v70 = 1;
    while v18[v64][v70].weight < v68 do
        v68 = v68 - v18[v64][v70].weight;
        v70 = v70 + 1;
    end;
    local l_anim_1 = v18[v64][v70].anim;
    if v61 ~= l_anim_1 then
        if v60 ~= nil then
            v60:Stop();
            v60:Destroy();
            v65 = 0;
        end;
        v60 = v66:LoadAnimation(l_anim_1);
        if v67 then
            v60.Priority = v67;
        end;
        v60:Play(v65);
        v59 = v64;
        v61 = l_anim_1;
        v62 = v60.KeyframeReached:connect(toolKeyFrameReachedFunc);
    end;
end;
stopToolAnimations = function() --[[ Line: 323 ]] --[[ Name: stopToolAnimations ]]
    -- upvalues: v59 (ref), v62 (ref), v61 (ref), v60 (ref)
    local l_v59_0 = v59;
    if v62 ~= nil then
        v62:disconnect();
    end;
    v59 = "";
    v61 = nil;
    if v60 ~= nil then
        v60:Stop();
        v60:Destroy();
        v60 = nil;
    end;
    return l_v59_0;
end;
onRunning = function(v73) --[[ Line: 346 ]] --[[ Name: onRunning ]]
    -- upvalues: v11 (copy), l_Parent_0 (copy), l_Humanoid_0 (copy), v14 (ref), v8 (ref), v21 (copy), v13 (ref)
    v73 = v73 / if v11 then l_Parent_0:GetScale() else 1;
    if v73 > 0.01 then
        playAnimation("walk", 0.1, l_Humanoid_0);
        if v14 and v14.AnimationId == "http://www.roblox.com/asset/?id=180426354" then
            setAnimationSpeed(v73 / 14.5);
        end;
        v8 = "Running";
        return;
    else
        if v21[v13] == nil then
            playAnimation("idle", 0.1, l_Humanoid_0);
            v8 = "Standing";
        end;
        return;
    end;
end;
onDied = function() --[[ Line: 363 ]] --[[ Name: onDied ]]
    -- upvalues: v8 (ref)
    v8 = "Dead";
end;
onJumping = function() --[[ Line: 367 ]] --[[ Name: onJumping ]]
    -- upvalues: l_Humanoid_0 (copy), v46 (ref), v8 (ref)
    playAnimation("jump", 0.1, l_Humanoid_0);
    v46 = 0.3;
    v8 = "Jumping";
end;
onClimbing = function(v74) --[[ Line: 373 ]] --[[ Name: onClimbing ]]
    -- upvalues: v11 (copy), l_Parent_0 (copy), l_Humanoid_0 (copy), v8 (ref)
    v74 = v74 / if v11 then l_Parent_0:GetScale() else 1;
    playAnimation("climb", 0.1, l_Humanoid_0);
    setAnimationSpeed(v74 / 12);
    v8 = "Climbing";
end;
onGettingUp = function() --[[ Line: 381 ]] --[[ Name: onGettingUp ]]
    -- upvalues: v8 (ref)
    v8 = "GettingUp";
end;
onFreeFall = function() --[[ Line: 385 ]] --[[ Name: onFreeFall ]]
    -- upvalues: v46 (ref), l_Humanoid_0 (copy), v8 (ref)
    if v46 <= 0 then
        playAnimation("fall", 0.3, l_Humanoid_0);
    end;
    v8 = "FreeFall";
end;
onFallingDown = function() --[[ Line: 392 ]] --[[ Name: onFallingDown ]]
    -- upvalues: v8 (ref)
    v8 = "FallingDown";
end;
onSeated = function() --[[ Line: 396 ]] --[[ Name: onSeated ]]
    -- upvalues: v8 (ref)
    v8 = "Seated";
end;
onPlatformStanding = function() --[[ Line: 400 ]] --[[ Name: onPlatformStanding ]]
    -- upvalues: v8 (ref)
    v8 = "PlatformStanding";
end;
onSwimming = function(v75) --[[ Line: 404 ]] --[[ Name: onSwimming ]]
    -- upvalues: v8 (ref)
    if v75 > 0 then
        v8 = "Running";
        return;
    else
        v8 = "Standing";
        return;
    end;
end;
getTool = function() --[[ Line: 412 ]] --[[ Name: getTool ]]
    -- upvalues: l_Parent_0 (copy)
    for _, v77 in ipairs(l_Parent_0:GetChildren()) do
        if v77.className == "Tool" then
            return v77;
        end;
    end;
    return nil;
end;
getToolAnim = function(v78) --[[ Line: 419 ]] --[[ Name: getToolAnim ]]
    for _, v80 in ipairs(v78:GetChildren()) do
        if v80.Name == "toolanim" and v80.className == "StringValue" then
            return v80;
        end;
    end;
    return nil;
end;
animateTool = function() --[[ Line: 428 ]] --[[ Name: animateTool ]]
    -- upvalues: v44 (ref), l_Humanoid_0 (copy)
    if v44 == "None" then
        playToolAnimation("toolnone", 0.1, l_Humanoid_0, Enum.AnimationPriority.Idle);
        return;
    elseif v44 == "Slash" then
        playToolAnimation("toolslash", 0, l_Humanoid_0, Enum.AnimationPriority.Action);
        return;
    elseif v44 == "Lunge" then
        playToolAnimation("toollunge", 0, l_Humanoid_0, Enum.AnimationPriority.Action);
        return;
    else
        return;
    end;
end;
moveSit = function() --[[ Line: 446 ]] --[[ Name: moveSit ]]
    -- upvalues: v2 (copy), v3 (copy), v4 (copy), v5 (copy)
    v2.MaxVelocity = 0.15;
    v3.MaxVelocity = 0.15;
    v2:SetDesiredAngle(1.57);
    v3:SetDesiredAngle(-1.57);
    v4:SetDesiredAngle(1.57);
    v5:SetDesiredAngle(-1.57);
end;
local v81 = 0;
move = function(v82) --[[ Line: 457 ]] --[[ Name: move ]]
    -- upvalues: v81 (ref), v46 (ref), v8 (ref), l_Humanoid_0 (copy), v2 (copy), v3 (copy), v4 (copy), v5 (copy), v44 (ref), v45 (ref), v61 (ref)
    local v83 = 1;
    local v84 = 1;
    local v85 = v82 - v81;
    v81 = v82;
    local v86 = false;
    if v46 > 0 then
        v46 = v46 - v85;
    end;
    if v8 == "FreeFall" and v46 <= 0 then
        playAnimation("fall", 0.3, l_Humanoid_0);
    elseif v8 == "Seated" then
        playAnimation("sit", 0.5, l_Humanoid_0);
        return;
    elseif v8 == "Running" then
        playAnimation("walk", 0.1, l_Humanoid_0);
    elseif v8 == "Dead" or v8 == "GettingUp" or v8 == "FallingDown" or v8 == "Seated" or v8 == "PlatformStanding" then
        stopAllAnimations();
        v83 = 0.1;
        v84 = 1;
        v86 = true;
    end;
    if v86 then
        local v87 = v83 * math.sin(v82 * v84);
        v2:SetDesiredAngle(v87 + 0);
        v3:SetDesiredAngle(v87 - 0);
        v4:SetDesiredAngle(-v87);
        v5:SetDesiredAngle(-v87);
    end;
    local v88 = getTool();
    if v88 and v88:FindFirstChild("Handle") then
        local v89 = getToolAnim(v88);
        if v89 then
            v44 = v89.Value;
            v89.Parent = nil;
            v45 = v82 + 0.3;
        end;
        if v45 < v82 then
            v45 = 0;
            v44 = "None";
        end;
        animateTool();
        return;
    else
        stopToolAnimations();
        v44 = "None";
        v61 = nil;
        v45 = 0;
        return;
    end;
end;
l_Humanoid_0.Died:connect(onDied);
l_Humanoid_0.Running:connect(onRunning);
l_Humanoid_0.Jumping:connect(onJumping);
l_Humanoid_0.Climbing:connect(onClimbing);
l_Humanoid_0.GettingUp:connect(onGettingUp);
l_Humanoid_0.FreeFalling:connect(onFreeFall);
l_Humanoid_0.FallingDown:connect(onFallingDown);
l_Humanoid_0.Seated:connect(onSeated);
l_Humanoid_0.PlatformStanding:connect(onPlatformStanding);
l_Humanoid_0.Swimming:connect(onSwimming);
game:GetService("Players").LocalPlayer.Chatted:connect(function(v90) --[[ Line: 533 ]]
    -- upvalues: v20 (copy), v8 (ref), v21 (copy), l_Humanoid_0 (copy)
    local v91 = "";
    if v90 == "/e dance" then
        v91 = v20[math.random(1, #v20)];
    elseif string.sub(v90, 1, 3) == "/e " then
        v91 = string.sub(v90, 4);
    elseif string.sub(v90, 1, 7) == "/emote " then
        v91 = string.sub(v90, 8);
    end;
    if v8 == "Standing" and v21[v91] ~= nil then
        playAnimation(v91, 0.1, l_Humanoid_0);
    end;
end);
script:WaitForChild("PlayEmote").OnInvoke = function(v92) --[[ Line: 550 ]]
    -- upvalues: v8 (ref), v21 (copy), l_Humanoid_0 (copy), v15 (ref)
    if v8 ~= "Standing" then
        return;
    elseif v21[v92] ~= nil then
        playAnimation(v92, 0.1, l_Humanoid_0);
        return true, v15;
    else
        return false;
    end;
end;
playAnimation("idle", 0.1, l_Humanoid_0);
v8 = "Standing";
while l_Parent_0.Parent ~= nil do
    local _, v94 = wait(0.1);
    move(v94);
end;