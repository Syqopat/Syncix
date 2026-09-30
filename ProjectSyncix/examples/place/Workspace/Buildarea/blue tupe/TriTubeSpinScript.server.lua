    local RunService = game:GetService("RunService")
    
    local function setupRotor(tubeName, speedMultiplier)
        local tube = script.Parent:FindFirstChild(tubeName)
        if not tube then return nil end
        local rotor = tube:FindFirstChild("RotorAssembly")
        if not rotor then return nil end
        
        local shaft = rotor:FindFirstChild("Shaft")
        local hub = rotor:FindFirstChild("Hub")
        local vRing = rotor:FindFirstChild("VortexRing")
        local blades = {}
        for i = 1, 4 do
            local b = rotor:FindFirstChild("Blade_" .. i)
            if b then table.insert(blades, b) end
        end
        if not shaft or not hub then return nil end
        
        return {
            centerPos = shaft.Position,
            hubY = hub.Position.Y,
            blades = blades,
            vRing = vRing,
            speed = speedMultiplier,
            bLen = #blades > 0 and blades[1].Size.X or 1.2,
            angle = 0
        }
    end
    
    local rotors = {
        setupRotor("Main_Tube", 500),
        setupRotor("Left_Tube", 380),
        setupRotor("Right_Tube", 260)
    }
    
    RunService.Heartbeat:Connect(function(dt)
        for _, r in ipairs(rotors) do
            if r then
                r.angle = (r.angle + dt * r.speed) % 360
                local rad = math.rad(r.angle)
                
                for idx, blade in ipairs(r.blades) do
                    local offsetAng = (idx - 1) * (math.pi / 2) + rad
                    blade.CFrame = CFrame.new(r.centerPos.X, r.hubY, r.centerPos.Z)
                        * CFrame.Angles(0, offsetAng, 0)
                        * CFrame.Angles(math.rad(24), 0, 0)
                        * CFrame.new(0, 0, r.bLen / 2 + 0.25)
                end
                
                if r.vRing then
                    r.vRing.CFrame = CFrame.new(r.centerPos.X, r.hubY, r.centerPos.Z)
                        * CFrame.Angles(0, rad * 1.5, math.rad(90))
                end
            end
        end
    end)
