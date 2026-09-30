-- LiquidSim: the liquid inside this tank.
--
-- Not an effect painted on a cylinder. Every speck is carried through a velocity field,
-- the one a stirred tank really has: the blades throw liquid outwards, it climbs the
-- wall, turns in at the surface and sinks down the middle. Two of those loops, one
-- above the blades and one below, with everything turning at the same time.
--
-- The script lives inside the model it drives and runs with RunContext = Client. Move
-- the model, rename it, copy it: it keeps working, and nothing is sent to the server.
-- None of it runs in Studio's edit mode; press Play.

local RunService = game:GetService("RunService")

-- The tank is whatever this script sits in. No paths through the Workspace.
local lab = script.Parent

local bulk = lab:WaitForChild("Bulk", 10)
local marker = lab:WaitForChild("Marker", 10)
local stirrer = lab:FindFirstChild("Stirrer")

if not (bulk and marker) then
	warn("[LiquidSim] Bulk or Marker is missing from " .. lab:GetFullName() .. "; stopping.")
	return
end

-- The funnel is found by what it is, not by what it is called: the first mesh in the
-- tank. Rename it, regroup it, re-import it, it still turns.
local vortexMesh
for _, d in ipairs(lab:GetDescendants()) do
	if d:IsA("MeshPart") then
		vortexMesh = d
		break
	end
end

-- Tank numbers, read off the model so the geometry stays the one source of truth.
local CENTRE = Vector3.new(bulk.Position.X, 0, bulk.Position.Z)
local R = bulk.Size.Y / 2 - 0.25      -- a cylinder's diameter is its Y and Z size
local BOT = marker.Position.Y
local TOP = bulk.Position.Y + bulk.Size.X / 2
local IMPELLER = BOT + 2.4            -- the height where the two loops meet

local FLOW = 2.6        -- studs a second in the circulation loops
local SPIN = 3.1        -- radians a second at the blades
local BUOYANCY = 2.2    -- extra rise for bubbles

-- Bubbles are billboards, not spheres: a drawn shell with a hollow middle reads as a
-- bubble from every angle and costs one quad. Fewer of them than the old neon balls,
-- because each one now carries its own little gui.
local BUBBLE_IMAGE = "rbxassetid://90555701622472"
local COUNT = 150       -- the liquid itself
local BUBBLES = 60      -- the ones that rise and pop

local COLD = Color3.fromRGB(18, 184, 232)
local HOT = Color3.fromRGB(190, 250, 255)
local ORE = Color3.fromRGB(255, 176, 64)

local rng = Random.new(20260920)

local folder = Instance.new("Folder")
folder.Name = "LiquidParticles"
folder.Parent = workspace

local anchor = Instance.new("Part")
anchor.Size = Vector3.one * 0.2
anchor.Transparency = 1
anchor.Anchored = true
anchor.CanCollide = false
anchor.CanQuery = false
anchor.CanTouch = false
anchor.CastShadow = false

local parts, rad, ang, hei, phase, kind = {}, {}, {}, {}, {}, {}

local function place(i, atBottom)
	rad[i] = math.sqrt(rng:NextNumber()) * R
	ang[i] = rng:NextNumber() * math.pi * 2
	hei[i] = atBottom and (BOT + rng:NextNumber() * 0.6) or (BOT + rng:NextNumber() * (TOP - BOT))
	phase[i] = rng:NextNumber() * math.pi * 2
end

local function makeBubble(diameter, colour, transparency)
	local part = anchor:Clone()
	local billboard = Instance.new("BillboardGui")
	billboard.Size = UDim2.fromScale(diameter, diameter)   -- scale is studs here
	billboard.AlwaysOnTop = false
	billboard.LightInfluence = 0
	billboard.Parent = part

	local image = Instance.new("ImageLabel")
	image.Size = UDim2.fromScale(1, 1)
	image.BackgroundTransparency = 1
	image.Image = BUBBLE_IMAGE
	image.ImageColor3 = colour
	image.ImageTransparency = transparency
	image.Parent = billboard
	return part
end

for i = 1, COUNT + BUBBLES do
	kind[i] = (i > COUNT) and "bubble" or (rng:NextNumber() < 0.14 and "ore" or "drop")
	local part
	if kind[i] == "bubble" then
		part = makeBubble(rng:NextNumber(0.5, 0.95), HOT, 0.08)
	elseif kind[i] == "ore" then
		-- Ore dust stays solid: it is a grain, not a shell, and the contrast against
		-- the bubbles is what makes the liquid look like it is carrying something.
		part = anchor:Clone()
		part.Shape = Enum.PartType.Ball
		part.Material = Enum.Material.Neon
		part.Size = Vector3.one * rng:NextNumber(0.28, 0.46)
		part.Color = ORE
	else
		part = makeBubble(rng:NextNumber(0.35, 0.7), COLD:Lerp(HOT, rng:NextNumber() ^ 2), 0.2)
	end
	part.Parent = folder
	parts[i] = part
	place(i, kind[i] == "bubble")
end

-- The velocity field: radius and height in, radial and vertical speed out.
local function flowAt(r, y)
	local rn = math.clamp(r / R, 0, 1)
	local ur, uy
	if y >= IMPELLER then
		local zn = math.clamp((y - IMPELLER) / (TOP - IMPELLER), 0, 1)
		ur = math.cos(math.pi * zn) * math.sin(math.pi * rn)
		uy = -math.sin(math.pi * zn) * math.cos(math.pi * rn)
	else
		local zn = math.clamp((IMPELLER - y) / (IMPELLER - BOT), 0, 1)
		ur = math.cos(math.pi * zn) * math.sin(math.pi * rn)
		uy = math.sin(math.pi * zn) * math.cos(math.pi * rn)
	end
	return ur * FLOW, uy * FLOW
end

-- Swirl: nearly solid-body near the shaft, held back by the wall. That difference is
-- what makes the specks shear past each other instead of turning as one block.
local function swirlAt(r)
	local rn = math.clamp(r / R, 0, 1)
	return SPIN * (0.35 + 0.65 * (1 - rn * rn)) * (0.35 + 0.65 * math.min(rn * 3, 1))
end

-- What each moving piece started as. A standing cylinder's own X axis runs along its
-- length, so a tilt has to be applied in world space: doing it in local space rolled
-- the liquid end over end and pushed it out through the glass.
local bulkHome = bulk.CFrame
local bulkSpin = bulkHome - bulkHome.Position

local stirHome = {}
if stirrer then
	for _, p in ipairs(stirrer:GetDescendants()) do
		if p:IsA("BasePart") then
			stirHome[p] = p.CFrame
		end
	end
end

local meshHome, meshSpin
if vortexMesh then
	meshHome = vortexMesh.CFrame
	meshSpin = meshHome - meshHome.Position
end

local t = 0

RunService.RenderStepped:Connect(function(dt)
	dt = math.min(dt, 1 / 20)   -- a frozen frame must not throw the liquid out of the tank
	t += dt

	for i = 1, COUNT + BUBBLES do
		local r, y = rad[i], hei[i]
		local ur, uy = flowAt(r, y)

		-- Turbulence: every speck carries its own offset, so neighbours drift apart.
		local wob = phase[i]
		ur += math.sin(t * 2.3 + wob) * 0.45
		uy += math.cos(t * 1.9 + wob * 1.7) * 0.4

		if kind[i] == "bubble" then
			uy += BUOYANCY
		elseif kind[i] == "ore" then
			uy -= 0.8            -- ore is heavy: it lags the flow and sinks back
		end

		r += ur * dt
		y += uy * dt
		local a = ang[i] + swirlAt(r) * dt

		if r < 0.12 then         -- through the middle and out the other side
			r = 0.12 - r
			a += math.pi
		elseif r > R then
			r = R - (r - R) * 0.5
		end

		if kind[i] == "bubble" and y >= TOP - 0.15 then
			place(i, true)       -- popped at the surface, a new one starts at the floor
			r, y, a = rad[i], hei[i], ang[i]
		else
			y = math.clamp(y, BOT + 0.1, TOP - 0.12)
		end

		rad[i], hei[i], ang[i] = r, y, a
		parts[i].Position = CENTRE + Vector3.new(r * math.sin(a), y, r * math.cos(a))
	end

	local tiltX = math.sin(t * 1.6) * 0.02
	local tiltZ = math.cos(t * 1.25) * 0.018

	bulk.CFrame = CFrame.new(bulkHome.Position) * CFrame.Angles(tiltX, 0, tiltZ) * bulkSpin

	local turn = CFrame.new(CENTRE.X, 0, CENTRE.Z) * CFrame.Angles(0, t * SPIN, 0)
		* CFrame.new(-CENTRE.X, 0, -CENTRE.Z)
	for p, home in pairs(stirHome) do
		p.CFrame = turn * home
	end

	if vortexMesh then
		vortexMesh.CFrame = CFrame.new(meshHome.Position)
			* CFrame.Angles(tiltX * 0.6, t * SPIN * 1.1, tiltZ * 0.6) * meshSpin
	end

	local glow = bulk:FindFirstChildOfClass("PointLight")
	if glow then
		glow.Brightness = 2.0 + math.sin(t * 4.1) * 0.5
	end
end)
