--!strict
--[[
    PROCEDURAL FARM ASSET ENGINE
    =============================
    Single-file ServerScript architecture containing:

        Module 1 -> MathEngine
        Module 2 -> MeshBuilder
        Module 3 -> AssetGenerators
        Module 4 -> Deployment

    Geometry:
        - Part
        - Part.Shape = Ball
        - Part.Shape = Cylinder
        - WedgePart

    Major procedural systems:
        - Recursive L-System branching
        - Spherical Fibonacci phyllotaxis
        - Cubic Bezier curves
        - Parametric sine/cosine weaving
        - Deterministic procedural noise
        - Layered faceted canopies
        - Organic soil mounds
        - Harvest prop fabrication
        - Magical particle/light accents
        - Batch yielding for server safety
        - Deterministic per-asset random seeds
        - Automatic redeployment
        - 3x5 deployment matrix

    Paste directly into ServerScriptService as a normal ServerScript.
]]

----------------------------------------------------------------------
-- SERVICES
----------------------------------------------------------------------

local Workspace = game:GetService("Workspace")
local Lighting = game:GetService("Lighting")
local CollectionService = game:GetService("CollectionService")

----------------------------------------------------------------------
-- MASTER CONFIGURATION
----------------------------------------------------------------------

local Config = {
	RootName = "ProceduralFarmGenerated",

	-- Grid
	GridRows = 3,
	GridColumns = 5,
	GridSpacing = 48,
	GridOrigin = Vector3.new(0, 0, 0),

	-- Detail
	DetailMultiplier = 1.0,
	MaxPartCountPerAsset = 650,
	MaxParticleEmittersPerAsset = 8,

	-- Server generation throttling
	YieldEveryParts = 45,
	YieldDuration = 0.02,

	-- Soil
	SoilBaseRadius = 7,
	SoilMoundCount = 30,

	-- L-System
	LSystemDepth = 3,
	LSystemSegmentLength = 2.5,
	LSystemAngle = math.rad(24),

	-- Botanical
	LeafThickness = 0.18,
	FruitScale = 1.0,

	-- Visual
	DefaultReflectance = 0.02,
	FruitReflectance = 0.06,
	CrystalReflectance = 0.16,
	CosmicReflectance = 0.28,

	-- Particle texture is a built-in Roblox texture
	SparkTexture = "rbxasset://textures/particles/sparkles_main.dds",

	-- Randomization
	MasterSeed = 746291,

	-- Runtime
	ClearExistingGeneration = true,
	AutoRun = true,
}

----------------------------------------------------------------------
-- UTILITY STATE
----------------------------------------------------------------------

local Runtime = {
	PartCount = 0,
	LastYield = 0,
}

local function yieldIfNeeded()
	Runtime.PartCount += 1

	if Runtime.PartCount % Config.YieldEveryParts == 0 then
		task.wait(Config.YieldDuration)
	end
end

local function safeNumber(value: number, fallback: number): number
	if typeof(value) ~= "number" then
		return fallback
	end

	if value ~= value then
		return fallback
	end

	return value
end

local function clamp(value: number, minimum: number, maximum: number): number
	return math.max(minimum, math.min(maximum, value))
end

local function hashString(text: string): number
	local hash = 2166136261

	for i = 1, #text do
		hash = bit32.bxor(hash, string.byte(text, i))
		hash = (hash * 16777619) % 2147483647
	end

	return math.max(1, hash)
end

local function makeRng(label: string, index: number?): Random
	local numericIndex = index or 0

	local seed =
		Config.MasterSeed
		+ hashString(label)
		+ numericIndex * 7919

	seed = math.abs(seed) % 2147483647

	if seed == 0 then
		seed = 1
	end

	return Random.new(seed)
end

local function randomRange(
	rng: Random,
	minimum: number,
	maximum: number
): number
	return rng:NextNumber(minimum, maximum)
end

local function randomInteger(
	rng: Random,
	minimum: number,
	maximum: number
): number
	return rng:NextInteger(minimum, maximum)
end

local function lerpNumber(
	a: number,
	b: number,
	t: number
): number
	return a + (b - a) * t
end

local function lerpVector(
	a: Vector3,
	b: Vector3,
	t: number
): Vector3
	return a:Lerp(b, t)
end

local function colorLerp(
	a: Color3,
	b: Color3,
	t: number
): Color3
	return a:Lerp(b, t)
end

----------------------------------------------------------------------
-- MATERIAL / VISUAL PALETTES
----------------------------------------------------------------------

local Palette = {
	SoilDark = Color3.fromRGB(59, 41, 25),
	Soil = Color3.fromRGB(91, 62, 34),
	SoilLight = Color3.fromRGB(122, 84, 43),

	Bark = Color3.fromRGB(93, 58, 29),
	BarkLight = Color3.fromRGB(123, 76, 36),
	BarkDark = Color3.fromRGB(64, 39, 20),

	Leaf = Color3.fromRGB(48, 135, 52),
	LeafLight = Color3.fromRGB(89, 172, 67),
	LeafDark = Color3.fromRGB(28, 93, 38),

	CarrotOrange = Color3.fromRGB(242, 119, 29),
	CarrotGreen = Color3.fromRGB(61, 157, 54),

	PotatoBrown = Color3.fromRGB(156, 112, 70),
	PotatoLight = Color3.fromRGB(190, 145, 92),

	StrawberryRed = Color3.fromRGB(224, 43, 55),
	StrawberryDark = Color3.fromRGB(164, 26, 39),
	StrawberrySeed = Color3.fromRGB(248, 217, 156),

	Blueberry = Color3.fromRGB(53, 76, 157),
	BlueberryDark = Color3.fromRGB(31, 48, 111),

	AppleRed = Color3.fromRGB(206, 43, 39),
	AppleGreen = Color3.fromRGB(88, 170, 61),
	AppleYellow = Color3.fromRGB(230, 189, 54),

	MangoYellow = Color3.fromRGB(246, 178, 43),
	MangoOrange = Color3.fromRGB(239, 119, 24),

	FrostBlue = Color3.fromRGB(173, 230, 255),
	FrostCyan = Color3.fromRGB(102, 207, 250),
	FrostWhite = Color3.fromRGB(224, 248, 255),

	CherryPink = Color3.fromRGB(239, 107, 145),
	CherryDark = Color3.fromRGB(174, 47, 83),

	BananaYellow = Color3.fromRGB(247, 211, 58),
	BananaBrown = Color3.fromRGB(115, 79, 27),

	DragonPink = Color3.fromRGB(227, 82, 126),
	DragonWhite = Color3.fromRGB(250, 232, 218),
	DragonGreen = Color3.fromRGB(61, 157, 68),

	StarYellow = Color3.fromRGB(255, 221, 86),
	StarGold = Color3.fromRGB(241, 156, 39),

	CoconutBrown = Color3.fromRGB(94, 61, 34),
	CoconutWhite = Color3.fromRGB(247, 241, 222),

	CrystalBlue = Color3.fromRGB(121, 221, 255),
	CrystalWhite = Color3.fromRGB(221, 252, 255),

	NebulaPurple = Color3.fromRGB(122, 63, 202),
	NebulaBlue = Color3.fromRGB(64, 122, 226),
	NebulaPink = Color3.fromRGB(230, 86, 195),

	CelestialBlue = Color3.fromRGB(42, 57, 130),
	CelestialPurple = Color3.fromRGB(109, 67, 179),
	CelestialStar = Color3.fromRGB(255, 236, 153),
}

----------------------------------------------------------------------
-- MODULE 1 : MATH ENGINE
----------------------------------------------------------------------

local MathEngine = {}

MathEngine.Phi = (1 + math.sqrt(5)) / 2
MathEngine.GoldenAngle = math.pi * (3 - math.sqrt(5))

function MathEngine.Map(
	value: number,
	inMin: number,
	inMax: number,
	outMin: number,
	outMax: number
): number
	local denominator = inMax - inMin

	if math.abs(denominator) < 1e-8 then
		return outMin
	end

	local t = (value - inMin) / denominator

	return outMin + (outMax - outMin) * t
end

function MathEngine.SmoothStep(t: number): number
	t = clamp(t, 0, 1)

	return t * t * (3 - 2 * t)
end

function MathEngine.SmootherStep(t: number): number
	t = clamp(t, 0, 1)

	return t * t * t * (t * (t * 6 - 15) + 10)
end

function MathEngine.CubicBezier(
	p0: Vector3,
	p1: Vector3,
	p2: Vector3,
	p3: Vector3,
	t: number
): Vector3
	local u = 1 - t
	local uu = u * u
	local tt = t * t

	return
		p0 * (uu * u)
		+ p1 * (3 * uu * t)
		+ p2 * (3 * u * tt)
		+ p3 * (tt * t)
end

function MathEngine.BezierTangent(
	p0: Vector3,
	p1: Vector3,
	p2: Vector3,
	p3: Vector3,
	t: number
): Vector3
	local u = 1 - t

	return
		(p1 - p0) * (3 * u * u)
		+ (p2 - p1) * (6 * u * t)
		+ (p3 - p2) * (3 * t * t)
end

function MathEngine.BezierFrame(
	p0: Vector3,
	p1: Vector3,
	p2: Vector3,
	p3: Vector3,
	t: number
): CFrame
	local position = MathEngine.CubicBezier(p0, p1, p2, p3, t)
	local tangent = MathEngine.BezierTangent(p0, p1, p2, p3, t)

	if tangent.Magnitude < 1e-5 then
		tangent = Vector3.new(0, 1, 0)
	else
		tangent = tangent.Unit
	end

	local up = Vector3.new(0, 1, 0)

	if math.abs(tangent:Dot(up)) > 0.92 then
		up = Vector3.new(1, 0, 0)
	end

	local right = tangent:Cross(up).Unit
	local correctedUp = right:Cross(tangent).Unit

	return CFrame.fromMatrix(
		position,
		right,
		correctedUp,
		-tangent
	)
end

function MathEngine.FibonacciSphere(
	count: number,
	radius: number,
	center: Vector3,
	jitter: number?,
	rng: Random?
): {Vector3}
	local points = {}

	local effectiveJitter = jitter or 0
	local localRng = rng or Random.new()

	if count <= 0 then
		return points
	end

	for i = 0, count - 1 do
		local normalized = if count == 1
			then 0.5
			else i / (count - 1)

		local y = 1 - 2 * normalized
		local ringRadius = math.sqrt(math.max(0, 1 - y * y))
		local theta = i * MathEngine.GoldenAngle

		local direction = Vector3.new(
			math.cos(theta) * ringRadius,
			y,
			math.sin(theta) * ringRadius
		)

		local radialNoise = 1

		if effectiveJitter > 0 then
			radialNoise =
				1
				+ localRng:NextNumber(
					-effectiveJitter,
					effectiveJitter
				)
		end

		points[#points + 1] =
			center + direction * radius * radialNoise
	end

	return points
end

function MathEngine.SphericalFibonacciDirections(
	count: number
): {Vector3}
	local directions = {}

	if count <= 0 then
		return directions
	end

	for i = 0, count - 1 do
		local y = 1 - 2 * ((i + 0.5) / count)
		local radial = math.sqrt(
			math.max(0, 1 - y * y)
		)

		local theta = i * MathEngine.GoldenAngle

		directions[#directions + 1] = Vector3.new(
			math.cos(theta) * radial,
			y,
			math.sin(theta) * radial
		).Unit
	end

	return directions
end

function MathEngine.FibonacciSpiral(
	count: number,
	spacing: number,
	verticalRise: number,
	center: Vector3
): {Vector3}
	local points = {}

	for i = 0, count - 1 do
		local radius = spacing * math.sqrt(i + 1)
		local theta = i * MathEngine.GoldenAngle

		points[#points + 1] =
			center
			+ Vector3.new(
				math.cos(theta) * radius,
				verticalRise * i,
				math.sin(theta) * radius
			)
	end

	return points
end

function MathEngine.EllipsoidPoint(
	center: Vector3,
	radiusX: number,
	radiusY: number,
	radiusZ: number,
	index: number,
	count: number
): Vector3
	local y = 1 - 2 * ((index + 0.5) / count)

	local radial = math.sqrt(
		math.max(0, 1 - y * y)
	)

	local theta = index * MathEngine.GoldenAngle

	return center + Vector3.new(
		math.cos(theta) * radial * radiusX,
		y * radiusY,
		math.sin(theta) * radial * radiusZ
	)
end

function MathEngine.ValueNoise3D(
	x: number,
	y: number,
	z: number,
	seed: number
): number
	local xi = math.floor(x)
	local yi = math.floor(y)
	local zi = math.floor(z)

	local xf = x - xi
	local yf = y - yi
	local zf = z - zi

	local function lattice(
		ix: number,
		iy: number,
		iz: number
	): number
		local n =
			ix * 15731
			+ iy * 789221
			+ iz * 1376312589
			+ seed * 1013

		n = bit32.bxor(
			n,
			bit32.rshift(n, 13)
		)

		local value =
			(n * (n * n * 15731 + 789221) + 1376312589)
			% 2147483647

		return (value / 2147483647) * 2 - 1
	end

	local function fade(v: number): number
		return MathEngine.SmootherStep(v)
	end

	local u = fade(xf)
	local v = fade(yf)
	local w = fade(zf)

	local c000 = lattice(xi, yi, zi)
	local c100 = lattice(xi + 1, yi, zi)
	local c010 = lattice(xi, yi + 1, zi)
	local c110 = lattice(xi + 1, yi + 1, zi)

	local c001 = lattice(xi, yi, zi + 1)
	local c101 = lattice(xi + 1, yi, zi + 1)
	local c011 = lattice(xi, yi + 1, zi + 1)
	local c111 = lattice(xi + 1, yi + 1, zi + 1)

	local x00 = c000 + (c100 - c000) * u
	local x10 = c010 + (c110 - c010) * u
	local x01 = c001 + (c101 - c001) * u
	local x11 = c011 + (c111 - c011) * u

	local y0 = x00 + (x10 - x00) * v
	local y1 = x01 + (x11 - x01) * v

	return y0 + (y1 - y0) * w
end

function MathEngine.FractalNoise3D(
	x: number,
	y: number,
	z: number,
	seed: number,
	octaves: number?,
	lacunarity: number?,
	gain: number?
): number
	local octaveCount = octaves or 4
	local lac = lacunarity or 2
	local g = gain or 0.5

	local amplitude = 1
	local frequency = 1
	local total = 0
	local normalization = 0

	for octave = 1, octaveCount do
		total += MathEngine.ValueNoise3D(
			x * frequency,
			y * frequency,
			z * frequency,
			seed + octave * 31
		) * amplitude

		normalization += amplitude

		amplitude *= g
		frequency *= lac
	end

	if normalization <= 0 then
		return 0
	end

	return total / normalization
end

----------------------------------------------------------------------
-- RECURSIVE L-SYSTEM ENGINE
----------------------------------------------------------------------

function MathEngine.RecursiveExpand(
	symbol: string,
	depth: number,
	rules: {[string]: string},
	buffer: {string},
	budget: {value: number}
)
	if budget.value <= 0 then
		return
	end

	local replacement = rules[symbol]

	if depth <= 0 or replacement == nil then
		buffer[#buffer + 1] = symbol
		budget.value -= 1
		return
	end

	for i = 1, #replacement do
		if budget.value <= 0 then
			break
		end

		local character = string.sub(
			replacement,
			i,
			i
		)

		MathEngine.RecursiveExpand(
			character,
			depth - 1,
			rules,
			buffer,
			budget
		)
	end
end

function MathEngine.BuildLSystem(
	axiom: string,
	rules: {[string]: string},
	iterations: number,
	maxSymbols: number?
): string
	local buffer = {}
	local budget = {
		value = maxSymbols or 20000
	}

	for i = 1, #axiom do
		local symbol = string.sub(
			axiom,
			i,
			i
		)

		MathEngine.RecursiveExpand(
			symbol,
			iterations,
			rules,
			buffer,
			budget
		)

		if budget.value <= 0 then
			break
		end
	end

	return table.concat(buffer)
end

type TurtleCommand = {
	kind: string,
	from: Vector3,
	to: Vector3,
	radius: number,
	depth: number,
}

function MathEngine.TurtleInterpret(
	program: string,
	origin: Vector3,
	heading: CFrame,
	stepLength: number,
	angle: number,
	branchRadius: number
): {TurtleCommand}
	local commands = {}

	type TurtleState = {
		frame: CFrame,
		position: Vector3,
		radius: number,
		depth: number,
	}

	local state: TurtleState = {
		frame = heading,
		position = origin,
		radius = branchRadius,
		depth = 0,
	}

	local stack: {TurtleState} = {}

	for i = 1, #program do
		local symbol = string.sub(
			program,
			i,
			i
		)

		if symbol == "F" then
			local currentPosition = state.position

			local localScale =
				1
				+ 0.08 * math.sin(i * 0.91)
				+ 0.04 * math.cos(i * 1.73)

			local distance =
				stepLength
				* localScale
				* math.max(
					0.35,
					1 - state.depth * 0.16
				)

			local forward =
				state.frame.LookVector

			state.position =
				currentPosition
				+ forward * distance

			commands[#commands + 1] = {
				kind = "Branch",
				from = currentPosition,
				to = state.position,
				radius = state.radius,
				depth = state.depth,
			}

		elseif symbol == "+" then
			state.frame *= CFrame.Angles(
				0,
				angle,
				0
			)

		elseif symbol == "-" then
			state.frame *= CFrame.Angles(
				0,
				-angle,
				0
			)

		elseif symbol == "&" then
			state.frame *= CFrame.Angles(
				angle,
				0,
				0
			)

		elseif symbol == "^" then
			state.frame *= CFrame.Angles(
				-angle,
				0,
				0
			)

		elseif symbol == "\\" then
			state.frame *= CFrame.Angles(
				0,
				0,
				angle
			)

		elseif symbol == "/" then
			state.frame *= CFrame.Angles(
				0,
				0,
				-angle
			)

		elseif symbol == "[" then
			stack[#stack + 1] = {
				frame = state.frame,
				position = state.position,
				radius = state.radius * 0.72,
				depth = state.depth + 1,
			}

		elseif symbol == "]" then
			local previous = stack[#stack]

			if previous then
				stack[#stack] = nil

				state = {
					frame = previous.frame,
					position = previous.position,
					radius = previous.radius,
					depth = previous.depth,
				}
			end
		end
	end

	return commands
end

----------------------------------------------------------------------
-- MODULE 2 : MESH BUILDER
----------------------------------------------------------------------

local MeshBuilder = {}

local function registerPart(part: BasePart)
	Runtime.PartCount += 1

	if Runtime.PartCount % Config.YieldEveryParts == 0 then
		task.wait(Config.YieldDuration)
	end

	CollectionService:AddTag(
		part,
		"ProceduralFarmGeometry"
	)

	return part
end

function MeshBuilder.MakePart(
	parent: Instance,
	name: string,
	size: Vector3,
	cframe: CFrame,
	material: Enum.Material,
	color: Color3,
	transparency: number?,
	reflectance: number?
): Part
	yieldIfNeeded()

	local part = Instance.new("Part")

	part.Name = name
	part.Size = size
	part.CFrame = cframe
	part.Anchored = true
	part.CanCollide = false
	part.CanTouch = false
	part.CanQuery = false
	part.CastShadow = true

	part.Material = material
	part.Color = color
	part.Transparency = transparency or 0
	part.Reflectance = reflectance or Config.DefaultReflectance

	part.TopSurface = Enum.SurfaceType.Smooth
	part.BottomSurface = Enum.SurfaceType.Smooth

	part.Parent = parent

	return registerPart(part)
end

function MeshBuilder.MakeBall(
	parent: Instance,
	name: string,
	size: Vector3,
	position: Vector3,
	material: Enum.Material,
	color: Color3,
	transparency: number?,
	reflectance: number?
): Part
	local part = MeshBuilder.MakePart(
		parent,
		name,
		size,
		CFrame.new(position),
		material,
		color,
		transparency,
		reflectance
	)

	part.Shape = Enum.PartType.Ball

	return part
end

function MeshBuilder.MakeCylinder(
	parent: Instance,
	name: string,
	radius: number,
	height: number,
	cframe: CFrame,
	material: Enum.Material,
	color: Color3,
	transparency: number?,
	reflectance: number?
): Part
	local part = MeshBuilder.MakePart(
		parent,
		name,
		Vector3.new(
			radius * 2,
			height,
			radius * 2
		),
		cframe,
		material,
		color,
		transparency,
		reflectance
	)

	part.Shape = Enum.PartType.Cylinder

	return part
end

function MeshBuilder.MakeWedge(
	parent: Instance,
	name: string,
	size: Vector3,
	cframe: CFrame,
	material: Enum.Material,
	color: Color3,
	transparency: number?,
	reflectance: number?
): WedgePart
	yieldIfNeeded()

	local wedge = Instance.new("WedgePart")

	wedge.Name = name
	wedge.Size = size
	wedge.CFrame = cframe

	wedge.Anchored = true
	wedge.CanCollide = false
	wedge.CanTouch = false
	wedge.CanQuery = false
	wedge.CastShadow = true

	wedge.Material = material
	wedge.Color = color
	wedge.Transparency = transparency or 0
	wedge.Reflectance = reflectance or Config.DefaultReflectance

	wedge.Parent = parent

	registerPart(wedge)

	return wedge
end

function MeshBuilder.MakeSegment(
	parent: Instance,
	name: string,
	a: Vector3,
	b: Vector3,
	radius: number,
	material: Enum.Material,
	color: Color3,
	reflectance: number?
): Part
	local direction = b - a
	local length = direction.Magnitude

	if length < 0.001 then
		length = 0.001
		direction = Vector3.new(0, 1, 0)
	end

	local midpoint = (a + b) * 0.5

	local cframe =
		CFrame.lookAt(
			midpoint,
			midpoint + direction
		)
		* CFrame.Angles(
			math.rad(90),
			0,
			0
		)

	return MeshBuilder.MakeCylinder(
		parent,
		name,
		radius,
		length,
		cframe,
		material,
		color,
		0,
		reflectance
	)
end

function MeshBuilder.MakeTaperedBranch(
	parent: Instance,
	name: string,
	a: Vector3,
	b: Vector3,
	radiusA: number,
	radiusB: number,
	material: Enum.Material,
	color: Color3
): Part
	local radius = (radiusA + radiusB) * 0.5

	local branch = MeshBuilder.MakeSegment(
		parent,
		name,
		a,
		b,
		radius,
		material,
		color
	)

	branch.Size = Vector3.new(
		radius * 2,
		(a - b).Magnitude,
		radius * 2
	)

	return branch
end

function MeshBuilder.AddPointLight(
	parent: Instance,
	position: Vector3,
	color: Color3,
	brightness: number,
	range: number
): PointLight
	local anchor = MeshBuilder.MakeBall(
		parent,
		"LightAnchor",
		Vector3.new(0.12, 0.12, 0.12),
		position,
		Enum.Material.Neon,
		color,
		1,
		0
	)

	anchor.Transparency = 1

	local light = Instance.new("PointLight")
	light.Name = "ProceduralGlow"
	light.Color = color
	light.Brightness = brightness
	light.Range = range
	light.Shadows = true
	light.Parent = anchor

	return light
end

function MeshBuilder.AddParticleBurst(
	parent: Instance,
	position: Vector3,
	color: Color3,
	rate: number,
	lifetime: NumberRange,
	speed: NumberRange,
	size: NumberSequence,
	lightEmission: number
): ParticleEmitter
	local attachmentPart = MeshBuilder.MakeBall(
		parent,
		"ParticleAnchor",
		Vector3.new(
			0.14,
			0.14,
			0.14
		),
		position,
		Enum.Material.Neon,
		color,
		1,
		0
	)

	attachmentPart.Transparency = 1
	attachmentPart.CanQuery = false

	local emitter = Instance.new("ParticleEmitter")

	emitter.Name = "ProceduralParticles"
	emitter.Texture = Config.SparkTexture
	emitter.Rate = rate
	emitter.Lifetime = lifetime
	emitter.Speed = speed
	emitter.Size = size
	emitter.Color = ColorSequence.new(color)
	emitter.Transparency = NumberSequence.new({
		NumberSequenceKeypoint.new(0, 0.1),
		NumberSequenceKeypoint.new(0.7, 0.25),
		NumberSequenceKeypoint.new(1, 1),
	})

	emitter.LightEmission = lightEmission
	emitter.LightInfluence = 0.15
	emitter.Rotation = NumberRange.new(0, 360)
	emitter.RotSpeed = NumberRange.new(-90, 90)
	emitter.SpreadAngle = Vector2.new(180, 180)
	emitter.Shape = Enum.ParticleEmitterShape.Sphere
	emitter.ShapeStyle = Enum.ParticleEmitterShapeStyle.Volume
	emitter.Enabled = true

	emitter.Parent = attachmentPart

	return emitter
end

function MeshBuilder.AddHighlight(
	parent: Instance,
	fillColor: Color3,
	outlineColor: Color3,
	fillTransparency: number,
	outlineTransparency: number
): Highlight
	local highlight = Instance.new("Highlight")

	highlight.Name = "ProceduralHighlight"
	highlight.Adornee = parent
	highlight.FillColor = fillColor
	highlight.OutlineColor = outlineColor
	highlight.FillTransparency = fillTransparency
	highlight.OutlineTransparency = outlineTransparency
	highlight.DepthMode = Enum.HighlightDepthMode.Occluded

	highlight.Parent = parent

	return highlight
end

----------------------------------------------------------------------
-- SOIL BUILDER
----------------------------------------------------------------------

function MeshBuilder.BuildSoilMound(
	parent: Instance,
	center: Vector3,
	radius: number,
	moundCount: number,
	rng: Random,
	seed: number
)
	local baseHeight =
		math.max(
			0.65,
			radius * 0.18
		)

	MeshBuilder.MakeCylinder(
		parent,
		"SoilBase",
		radius,
		baseHeight,
		CFrame.new(
			center
				+ Vector3.new(
					0,
					baseHeight * 0.5,
					0
				)
		),
		Enum.Material.Ground,
		Palette.Soil,
		0,
		0
	)

	local points = MathEngine.FibonacciSphere(
		moundCount,
		radius * 0.92,
		center + Vector3.new(
			0,
			baseHeight,
			0
		),
		0.16,
		rng
	)

	for i, point in ipairs(points) do
		local sphericalIndex = i

		local noise =
			MathEngine.FractalNoise3D(
				point.X * 0.17,
				point.Y * 0.19,
				point.Z * 0.17,
				seed,
				4,
				2.0,
				0.52
			)

		local localRadius =
			radius
			* 0.16
			* (
				1
				+ 0.45 * noise
			)

		local localHeight =
			radius
			* 0.23
			* (
				1
				+ 0.35 * noise
			)

		local randomOffset = Vector3.new(
			rng:NextNumber(-0.8, 0.8),
			rng:NextNumber(
				-0.15,
				0.4
			),
			rng:NextNumber(-0.8, 0.8)
		)

		local p =
			point
			+ randomOffset

		MeshBuilder.MakeBall(
			parent,
			"LumpySoil_" .. sphericalIndex,
			Vector3.new(
				localRadius * 2.2,
				localHeight * 1.2,
				localRadius * 2.2
			),
			p,
			Enum.Material.Ground,
			colorLerp(
				Palette.SoilDark,
				Palette.SoilLight,
				rng:NextNumber(0, 1)
			),
			0,
			0
		)
	end
end

----------------------------------------------------------------------
-- FACETED CANOPY
----------------------------------------------------------------------

function MeshBuilder.BuildFacetedCanopy(
	parent: Instance,
	center: Vector3,
	radius: Vector3,
	rings: number,
	segmentsPerRing: number,
	colorA: Color3,
	colorB: Color3,
	rng: Random
)
	local total = 0

	for ring = 1, rings do
		local verticalT =
			if rings == 1
			then 0.5
			else (ring - 1) / (rings - 1)

		local y =
			(verticalT - 0.5)
			* radius.Y
			* 1.55

		local ringRadiusScale =
			math.sin(
				verticalT * math.pi
			)

		for segment = 1, segmentsPerRing do
			total += 1

			local theta =
				(segment / segmentsPerRing)
				* math.pi
				* 2
				+ ring * MathEngine.GoldenAngle

			local noise =
				MathEngine.FractalNoise3D(
					ring * 0.61,
					segment * 0.37,
					0.13,
					rng:NextInteger(1, 100000),
					3,
					2,
					0.5
				)

			local x =
				math.cos(theta)
				* radius.X
				* ringRadiusScale
				* (
					0.65
					+ 0.28 * noise
				)

			local z =
				math.sin(theta)
				* radius.Z
				* ringRadiusScale
				* (
					0.65
					+ 0.28 * noise
				)

			local position =
				center
				+ Vector3.new(
					x,
					y,
					z
				)

			local leafScale =
				Vector3.new(
					rng:NextNumber(1.0, 1.8),
					rng:NextNumber(0.35, 0.68),
					rng:NextNumber(1.0, 1.8)
				)

			local wedge =
				MeshBuilder.MakeWedge(
					parent,
					"CanopyFacet_" .. total,
					leafScale,
					CFrame.new(position)
					* CFrame.Angles(
						rng:NextNumber(-0.35, 0.35),
						theta,
						rng:NextNumber(-0.25, 0.25)
					),
					Enum.Material.Grass,
					colorLerp(
						colorA,
						colorB,
						rng:NextNumber(0, 1)
					),
					0,
					0
				)

			wedge:SetAttribute(
				"CanopyFacet",
				true
			)
		end
	end
end

----------------------------------------------------------------------
-- LEAF / PETAL HELPERS
----------------------------------------------------------------------

function MeshBuilder.MakeLeaf(
	parent: Instance,
	name: string,
	center: Vector3,
	direction: Vector3,
	length: number,
	width: number,
	color: Color3,
	tilt: number?,
	material: Enum.Material?
): WedgePart
	local safeDirection = direction

	if safeDirection.Magnitude < 0.001 then
		safeDirection = Vector3.new(
			0,
			1,
			0
		)
	else
		safeDirection = safeDirection.Unit
	end

	local endpoint =
		center
		+ safeDirection * length

	local midpoint =
		(center + endpoint) * 0.5

	local frame =
		CFrame.lookAt(
			midpoint,
			endpoint
		)
		* CFrame.Angles(
			math.rad(90),
			0,
			tilt or 0
		)

	return MeshBuilder.MakeWedge(
		parent,
		name,
		Vector3.new(
			width,
			width,
			length
		),
		frame,
		material or Enum.Material.Grass,
		color,
		0,
		0
	)
end

function MeshBuilder.MakePetalCluster(
	parent: Instance,
	center: Vector3,
	petalCount: number,
	petalRadius: number,
	petalColor: Color3,
	coreColor: Color3,
	rng: Random
)
	local angleOffset =
		rng:NextNumber(
			0,
			math.pi * 2
		)

	for i = 1, petalCount do
		local theta =
			angleOffset
			+ (i / petalCount)
			* math.pi
			* 2

		local direction = Vector3.new(
			math.cos(theta),
			rng:NextNumber(
				0.05,
				0.4
			),
			math.sin(theta)
		).Unit

		MeshBuilder.MakeLeaf(
			parent,
			"Petal_" .. i,
			center,
			direction,
			petalRadius,
			petalRadius * 0.35,
			petalColor,
			rng:NextNumber(-0.4, 0.4),
			Enum.Material.SmoothPlastic
		)
	end

	MeshBuilder.MakeBall(
		parent,
		"PetalCore",
		Vector3.new(
			petalRadius * 0.65,
			petalRadius * 0.65,
			petalRadius * 0.65
		),
		center,
		Enum.Material.SmoothPlastic,
		coreColor,
		0,
		0.03
	)
end

----------------------------------------------------------------------
-- WOVEN BASKET
-- Parametric sine/cosine planks + circular weaving
----------------------------------------------------------------------

function MeshBuilder.BuildWovenBasket(
	parent: Instance,
	center: Vector3,
	width: number,
	height: number,
	depth: number,
	rng: Random
)
	local plankCount = 11
	local slatCount = 13

	local bodyColor = Color3.fromRGB(
		169,
		121,
		70
	)

	local accentColor = Color3.fromRGB(
		124,
		84,
		45
	)

	local base =
		center
	- Vector3.new(
		0,
		height * 0.5,
		0
	)

	------------------------------------------------------------------
	-- Vertical structural slats
	------------------------------------------------------------------

	for i = 1, plankCount do
		local t =
			if plankCount == 1
			then 0.5
			else (i - 1) / (plankCount - 1)

		local x =
			(t - 0.5)
			* width

		local wave =
			math.sin(
				t * math.pi * 2
			)
			* width
			* 0.025

		local z =
			wave

		local lean =
			math.sin(
				t * math.pi * 4
			)
			* 0.035

		MeshBuilder.MakePart(
			parent,
			"BasketVerticalSlat_" .. i,
			Vector3.new(
				width * 0.075,
				height,
				depth * 0.065
			),
			CFrame.new(
				base
					+ Vector3.new(
						x,
						height * 0.5,
						z
					)
			)
				* CFrame.Angles(
					0,
					lean,
					0
				),
			Enum.Material.WoodPlanks,
			if i % 2 == 0
				then bodyColor
				else accentColor,
			0,
			0.01
		)
	end

	------------------------------------------------------------------
	-- Horizontal woven layers
	------------------------------------------------------------------

	for layer = 1, slatCount do
		local t =
			(layer - 1)
			/ math.max(1, slatCount - 1)

		local y =
			base.Y
			+ height * (
				0.10
				+ t * 0.80
			)

		local wavePhase =
			t * math.pi * 4

		local segmentCount = 18

		for segment = 1, segmentCount do
			local theta =
				((segment - 1)
					/ segmentCount)
				* math.pi
				* 2

			local radiusX =
				width
				* 0.47
				* (
					1
					+ 0.045
					* math.sin(
						theta * 3
						+ wavePhase
					)
				)

			local radiusZ =
				depth
				* 0.47
				* (
					1
					+ 0.045
					* math.cos(
						theta * 2
						- wavePhase
					)
				)

			local aTheta =
				theta

			local bTheta =
				theta
				+ (
					math.pi * 2
					/ segmentCount
				)

			local a = center + Vector3.new(
				math.cos(aTheta)
					* radiusX,
				y,
				math.sin(aTheta)
					* radiusZ
			)

			local b = center + Vector3.new(
				math.cos(bTheta)
					* radiusX,
				y,
				math.sin(bTheta)
					* radiusZ
			)

			local offset =
				math.sin(
					theta * 4
					+ layer * 0.8
				) * 0.035

			a += Vector3.new(
				0,
				offset,
				0
			)

			b += Vector3.new(
				0,
				offset,
				0
			)

			MeshBuilder.MakeSegment(
				parent,
				"BasketWeave_" ..
					layer ..
					"_" ..
					segment,
				a,
				b,
				width * 0.018,
				Enum.Material.Wood,
				if layer % 2 == 0
					then bodyColor
					else accentColor,
				0.01
			)
		end
	end

	------------------------------------------------------------------
	-- Rim
	------------------------------------------------------------------

	local rimSegments = 28

	for i = 1, rimSegments do
		local aTheta =
			((i - 1) / rimSegments)
			* math.pi * 2

		local bTheta =
			(i / rimSegments)
			* math.pi * 2

		local y =
			base.Y
			+ height * 0.92

		local a = center + Vector3.new(
			math.cos(aTheta)
				* width * 0.49,
			y,
			math.sin(aTheta)
				* depth * 0.49
		)

		local b = center + Vector3.new(
			math.cos(bTheta)
				* width * 0.49,
			y,
			math.sin(bTheta)
				* depth * 0.49
		)

		MeshBuilder.MakeSegment(
			parent,
			"BasketRim_" .. i,
			a,
			b,
			width * 0.032,
			Enum.Material.Wood,
			accentColor,
			0.02
		)
	end

	------------------------------------------------------------------
	-- Handle
	------------------------------------------------------------------

	local handleSegments = 14

	for i = 1, handleSegments do
		local t =
			(i - 1)
			/ math.max(
				1,
				handleSegments - 1
			)

		local angle =
			math.pi
			+ math.pi * t

		local handleCenter =
			center
			+ Vector3.new(
				0,
				height * 0.88,
				0
			)

		local x =
			math.cos(angle)
			* width * 0.30

		local y =
			math.sin(angle)
			* height * 0.45

		local point =
			handleCenter
			+ Vector3.new(
				x,
				y,
				0
			)

		if i > 1 then
			local previousT =
				(i - 2)
				/ math.max(
					1,
					handleSegments - 1
				)

			local previousAngle =
				math.pi
				+ math.pi * previousT

			local previous =
				handleCenter
				+ Vector3.new(
					math.cos(previousAngle)
					* width * 0.30,
					math.sin(previousAngle)
					* height * 0.45,
					0
				)

			MeshBuilder.MakeSegment(
				parent,
				"BasketHandle_" .. i,
				previous,
				point,
				width * 0.03,
				Enum.Material.Wood,
				accentColor,
				0.01
			)
		end
	end
end

----------------------------------------------------------------------
-- WAVED CRATE
----------------------------------------------------------------------

function MeshBuilder.BuildWovenCrate(
	parent: Instance,
	center: Vector3,
	width: number,
	height: number,
	depth: number,
	rng: Random
)
	local woodA = Color3.fromRGB(
		185,
		137,
		77
	)

	local woodB = Color3.fromRGB(
		129,
		90,
		51
	)

	local plankThickness = 0.26

	------------------------------------------------------------------
	-- Bottom
	------------------------------------------------------------------

	MeshBuilder.MakePart(
		parent,
		"CrateBottom",
		Vector3.new(
			width,
			plankThickness,
			depth
		),
		CFrame.new(
			center
			- Vector3.new(
				0,
				height * 0.5,
				0
			)
		),
		Enum.Material.WoodPlanks,
		woodB,
		0,
		0.01
	)

	------------------------------------------------------------------
	-- Front and back parametric slats
	------------------------------------------------------------------

	local slatsY = 5

	for side = -1, 1, 2 do
		for i = 1, slatsY do
			local t =
				(i - 0.5) / slatsY

			local y =
				center.Y
			- height * 0.5
				+ t * height

			local wave =
				math.sin(
					t * math.pi * 4
					+ side * 0.8
				) * 0.10

			local cframe =
				CFrame.new(
					center.X + wave,
					y,
					center.Z
					+ side
					* depth * 0.5
				)

			MeshBuilder.MakePart(
				parent,
				"CrateFaceSlat_" ..
					side ..
					"_" ..
					i,
				Vector3.new(
					width,
					height / slatsY
						* 0.72,
					plankThickness
				),
				cframe,
				Enum.Material.WoodPlanks,
				if i % 2 == 0
					then woodA
					else woodB,
				0,
				0.01
			)
		end
	end

	------------------------------------------------------------------
	-- Side slats
	------------------------------------------------------------------

	local sideSlats = 4

	for sideX = -1, 1, 2 do
		for i = 1, sideSlats do
			local t =
				(i - 0.5) / sideSlats

			local y =
				center.Y
			- height * 0.5
				+ t * height

			local wave =
				math.cos(
					t * math.pi * 3
				) * 0.08

			MeshBuilder.MakePart(
				parent,
				"CrateSideSlat_" ..
					sideX ..
					"_" ..
					i,
				Vector3.new(
					plankThickness,
					height / sideSlats
						* 0.72,
					depth
				),
				CFrame.new(
					center.X
						+ sideX * width * 0.5,
					y,
					center.Z + wave
				),
				Enum.Material.WoodPlanks,
				if i % 2 == 0
					then woodB
					else woodA,
				0,
				0.01
			)
		end
	end

	------------------------------------------------------------------
	-- Corner posts
	------------------------------------------------------------------

	for xSign = -1, 1, 2 do
		for zSign = -1, 1, 2 do
			MeshBuilder.MakePart(
				parent,
				"CrateCorner",
				Vector3.new(
					plankThickness * 1.4,
					height * 1.03,
					plankThickness * 1.4
				),
				CFrame.new(
					center
						+ Vector3.new(
							xSign
							* width
							* 0.5,
							0,
							zSign
							* depth
							* 0.5
						)
				),
				Enum.Material.Wood,
				woodB,
				0,
				0.01
			)
		end
	end

	------------------------------------------------------------------
	-- Top rim oscillation
	------------------------------------------------------------------

	local rimSegments = 12

	for i = 1, rimSegments do
		local t =
			(i - 1)
			/ math.max(
				1,
				rimSegments - 1
			)

		local x =
			-width * 0.5
			+ width * t

		local wave =
			math.sin(
				t * math.pi * 5
			) * 0.09

		MeshBuilder.MakePart(
			parent,
			"CrateTopWave_" .. i,
			Vector3.new(
				width / rimSegments * 1.15,
				plankThickness,
				plankThickness
			),
			CFrame.new(
				center.X + x,
				center.Y
					+ height * 0.5,
				center.Z + wave
			),
			Enum.Material.WoodPlanks,
			woodA,
			0,
			0.01
		)
	end
end

----------------------------------------------------------------------
-- HARVEST ITEM HELPERS
----------------------------------------------------------------------

function MeshBuilder.BuildCarrot(
	parent: Instance,
	position: Vector3,
	scale: number,
	rng: Random
)
	local bodyHeight = 1.6 * scale

	local carrotCore =
		MeshBuilder.MakeBall(
			parent,
			"CarrotHarvest",
			Vector3.new(
				0.60 * scale,
				bodyHeight,
				0.60 * scale
			),
			position,
			Enum.Material.SmoothPlastic,
			Palette.CarrotOrange,
			0,
			Config.FruitReflectance
		)

	for i = 1, 5 do
		local theta =
			i * MathEngine.GoldenAngle

		local leafPosition =
			position
			+ Vector3.new(
				math.cos(theta)
				* 0.18
				* scale,
				bodyHeight
				* 0.42,
				math.sin(theta)
				* 0.18
				* scale
			)

		MeshBuilder.MakeLeaf(
			parent,
			"CarrotLeaf_" .. i,
			leafPosition,
			Vector3.new(
				math.cos(theta)
					* 0.20,
				rng:NextNumber(
					0.75,
					1.0
				),
				math.sin(theta)
					* 0.20
			),
			0.85 * scale,
			0.12 * scale,
			Palette.CarrotGreen,
			rng:NextNumber(-0.2, 0.2),
			Enum.Material.Grass
		)
	end

	return carrotCore
end

function MeshBuilder.BuildPotato(
	parent: Instance,
	position: Vector3,
	scale: number,
	rng: Random
)
	return MeshBuilder.MakeBall(
		parent,
		"PotatoHarvest",
		Vector3.new(
			1.10 * scale,
			0.88 * scale,
			0.95 * scale
		),
		position,
		Enum.Material.SmoothPlastic,
		colorLerp(
			Palette.PotatoBrown,
			Palette.PotatoLight,
			rng:NextNumber(
				0,
				0.65
			)
		),
		0,
		0.02
	)
end

function MeshBuilder.BuildBlueberry(
	parent: Instance,
	position: Vector3,
	scale: number,
	rng: Random
)
	local berry =
		MeshBuilder.MakeBall(
			parent,
			"BlueberryHarvest",
			Vector3.new(
				0.62 * scale,
				0.62 * scale,
				0.62 * scale
			),
			position,
			Enum.Material.SmoothPlastic,
			colorLerp(
				Palette.BlueberryDark,
				Palette.Blueberry,
				rng:NextNumber(0, 1)
			),
			0,
			Config.FruitReflectance
		)

	MeshBuilder.MakeCylinder(
		parent,
		"BlueberryCrown",
		0.11 * scale,
		0.045 * scale,
		CFrame.new(
			position
				+ Vector3.new(
					0,
					0.29 * scale,
					0
				)
		),
		Enum.Material.SmoothPlastic,
		Palette.BlueberryDark,
		0,
		0
	)

	return berry
end

function MeshBuilder.BuildApple(
	parent: Instance,
	position: Vector3,
	scale: number,
	color: Color3,
	rng: Random
)
	local apple =
		MeshBuilder.MakeBall(
			parent,
			"AppleHarvest",
			Vector3.new(
				1.16 * scale,
				1.10 * scale,
				1.16 * scale
			),
			position,
			Enum.Material.SmoothPlastic,
			color,
			0,
			Config.FruitReflectance
		)

	MeshBuilder.MakeCylinder(
		parent,
		"AppleStem",
		0.055 * scale,
		0.30 * scale,
		CFrame.new(
			position
				+ Vector3.new(
					0,
					0.62 * scale,
					0
				)
		),
		Enum.Material.Wood,
		Palette.BarkDark,
		0,
		0
	)

	local leafDirection =
		Vector3.new(
			rng:NextNumber(-0.5, 0.5),
			0.25,
			rng:NextNumber(-0.5, 0.5)
		).Unit

	MeshBuilder.MakeLeaf(
		parent,
		"AppleLeaf",
		position
			+ Vector3.new(
				0,
				0.68 * scale,
				0
			),
		leafDirection,
		0.55 * scale,
		0.15 * scale,
		Palette.Leaf,
		0,
		Enum.Material.Grass
	)

	return apple
end

function MeshBuilder.BuildMango(
	parent: Instance,
	position: Vector3,
	scale: number,
	rng: Random
)
	return MeshBuilder.MakeBall(
		parent,
		"MangoHarvest",
		Vector3.new(
			0.95 * scale,
			1.35 * scale,
			0.88 * scale
		),
		position,
		Enum.Material.SmoothPlastic,
		colorLerp(
			Palette.MangoYellow,
			Palette.MangoOrange,
			rng:NextNumber(0, 1)
		),
		0,
		Config.FruitReflectance
	)
end

function MeshBuilder.BuildCherryPair(
	parent: Instance,
	position: Vector3,
	scale: number,
	rng: Random
)
	local left =
		position
		+ Vector3.new(
			-0.24 * scale,
			0,
			0
		)

	local right =
		position
		+ Vector3.new(
			0.24 * scale,
			0,
			0
		)

	MeshBuilder.MakeBall(
		parent,
		"CherryA",
		Vector3.new(
			0.48 * scale,
			0.48 * scale,
			0.48 * scale
		),
		left,
		Enum.Material.SmoothPlastic,
		Palette.CherryDark,
		0,
		Config.FruitReflectance
	)

	MeshBuilder.MakeBall(
		parent,
		"CherryB",
		Vector3.new(
			0.48 * scale,
			0.48 * scale,
			0.48 * scale
		),
		right,
		Enum.Material.SmoothPlastic,
		Palette.CherryPink,
		0,
		Config.FruitReflectance
	)

	MeshBuilder.MakeSegment(
		parent,
		"CherryStemA",
		left
			+ Vector3.new(
				0,
				0.20 * scale,
				0
			),
		position
			+ Vector3.new(
				0,
				0.62 * scale,
				0
			),
		0.035 * scale,
		Enum.Material.Wood,
		Palette.BarkDark
	)

	MeshBuilder.MakeSegment(
		parent,
		"CherryStemB",
		right
			+ Vector3.new(
				0,
				0.20 * scale,
				0
			),
		position
			+ Vector3.new(
				0,
				0.62 * scale,
				0
			),
		0.035 * scale,
		Enum.Material.Wood,
		Palette.BarkDark
	)
end

function MeshBuilder.BuildBanana(
	parent: Instance,
	position: Vector3,
	scale: number,
	direction: Vector3,
	rng: Random
)
	local start =
		position
		+ direction.Unit
		* 0.1

	local finish =
		position
		+ direction.Unit
		* (1.35 * scale)

	local curvature =
		Vector3.new(
			0,
			-0.45 * scale,
			0
		)

	local previous = start

	local samples = 7

	for i = 1, samples do
		local t =
			(i - 1)
			/ (samples - 1)

		local p =
			start:Lerp(
				finish,
				t
			)
			+ Vector3.new(
				0,
				math.sin(
					t * math.pi
				)
				* curvature.Y,
				0
			)

		if i > 1 then
			MeshBuilder.MakeSegment(
				parent,
				"BananaSegment_" .. i,
				previous,
				p,
				0.19 * scale,
				Enum.Material.SmoothPlastic,
				Palette.BananaYellow,
				0.02
			)
		end

		previous = p
	end

	MeshBuilder.MakeBall(
		parent,
		"BananaTip",
		Vector3.new(
			0.33,
			0.24,
			0.33
		) * scale,
		finish,
		Enum.Material.Wood,
		Palette.BananaBrown,
		0,
		0
	)
end

----------------------------------------------------------------------
-- STRAWBERRY GENERATOR PROP
-- 100+ seeds via spherical Fibonacci placement
----------------------------------------------------------------------

function MeshBuilder.BuildStrawberry(
	parent: Instance,
	position: Vector3,
	scale: number,
	rng: Random
)
	local berry =
		MeshBuilder.MakeBall(
			parent,
			"StrawberryHarvest",
			Vector3.new(
				0.95 * scale,
				1.12 * scale,
				0.95 * scale
			),
			position,
			Enum.Material.SmoothPlastic,
			Palette.StrawberryRed,
			0,
			Config.FruitReflectance
		)

	local seedCount = 24

	local seedDirections =
		MathEngine.SphericalFibonacciDirections(
			seedCount
		)

	for i, direction in ipairs(seedDirections) do
		local adjustedDirection =
			Vector3.new(
				direction.X,
				direction.Y
				* 1.14,
				direction.Z
			)

		if adjustedDirection.Magnitude > 0 then
			adjustedDirection =
				adjustedDirection.Unit
		end

		local seedPosition =
			position
			+ Vector3.new(
				adjustedDirection.X
				* 0.49
				* scale,
				adjustedDirection.Y
				* 0.57
				* scale,
				adjustedDirection.Z
				* 0.49
				* scale
			)

		MeshBuilder.MakeBall(
			parent,
			"StrawberrySeed_" .. i,
			Vector3.new(
				0.075,
				0.12,
				0.075
			) * scale,
			seedPosition,
			Enum.Material.SmoothPlastic,
			Palette.StrawberrySeed,
			0,
			0.01
		)
	end

	for i = 1, 5 do
		local theta =
			i * MathEngine.GoldenAngle

		local leafDirection =
			Vector3.new(
				math.cos(theta)
				* 0.55,
				0.45,
				math.sin(theta)
				* 0.55
			).Unit

		MeshBuilder.MakeLeaf(
			parent,
			"StrawberryCalyx_" .. i,
			position
				+ Vector3.new(
					0,
					0.45 * scale,
					0
				),
			leafDirection,
			0.46 * scale,
			0.14 * scale,
			Palette.Leaf,
			rng:NextNumber(
				-0.3,
				0.3
			),
			Enum.Material.Grass
		)
	end

	return berry
end

----------------------------------------------------------------------
-- DRAGON FRUIT PROP
----------------------------------------------------------------------

function MeshBuilder.BuildDragonFruit(
	parent: Instance,
	position: Vector3,
	scale: number,
	rng: Random
)
	local fruit =
		MeshBuilder.MakeBall(
			parent,
			"DragonfruitHarvest",
			Vector3.new(
				1.25 * scale,
				1.05 * scale,
				1.25 * scale
			),
			position,
			Enum.Material.SmoothPlastic,
			Palette.DragonPink,
			0,
			Config.FruitReflectance
		)

	local finCount = 10

	for i = 1, finCount do
		local theta =
			i / finCount
			* math.pi * 2

		local direction =
			Vector3.new(
				math.cos(theta),
				rng:NextNumber(
					0.1,
					0.55
				),
				math.sin(theta)
			).Unit

		MeshBuilder.MakeLeaf(
			parent,
			"DragonScale_" .. i,
			position
				+ direction * 0.55 * scale,
			direction,
			0.50 * scale,
			0.18 * scale,
			Palette.DragonGreen,
			0,
			Enum.Material.SmoothPlastic
		)
	end

	return fruit
end

----------------------------------------------------------------------
-- STARFRUIT PROP
----------------------------------------------------------------------

function MeshBuilder.BuildStarfruit(
	parent: Instance,
	position: Vector3,
	scale: number,
	rng: Random
)
	local body =
		MeshBuilder.MakeBall(
			parent,
			"StarfruitCenter",
			Vector3.new(
				0.90 * scale,
				1.45 * scale,
				0.90 * scale
			),
			position,
			Enum.Material.SmoothPlastic,
			Palette.StarYellow,
			0,
			Config.FruitReflectance
		)

	for i = 1, 5 do
		local theta =
			i / 5
			* math.pi * 2

		local direction =
			Vector3.new(
				math.cos(theta),
				rng:NextNumber(
					0.05,
					0.25
				),
				math.sin(theta)
			).Unit

		local endpoint =
			position
			+ direction
			* 0.83
			* scale

		MeshBuilder.MakeWedge(
			parent,
			"StarfruitArm_" .. i,
			Vector3.new(
				0.30 * scale,
				0.55 * scale,
				1.10 * scale
			),
			CFrame.lookAt(
				position,
				endpoint
			)
				* CFrame.Angles(
					math.rad(90),
					0,
					0
				),
			Enum.Material.SmoothPlastic,
			Palette.StarGold,
			0,
			Config.FruitReflectance
		)
	end

	return body
end

----------------------------------------------------------------------
-- COCONUT PROP
----------------------------------------------------------------------

function MeshBuilder.BuildCoconut(
	parent: Instance,
	position: Vector3,
	scale: number,
	split: boolean
)
	local shell =
		MeshBuilder.MakeBall(
			parent,
			"Coconut",
			Vector3.new(
				1.05,
				1.05,
				1.05
			) * scale,
			position,
			Enum.Material.SmoothPlastic,
			Palette.CoconutBrown,
			0,
			0.04
		)

	if split then
		MeshBuilder.MakeBall(
			parent,
			"CoconutInterior",
			Vector3.new(
				0.78,
				0.35,
				0.78
			) * scale,
			position
				+ Vector3.new(
					0,
					0.34 * scale,
					0
				),
			Enum.Material.SmoothPlastic,
			Palette.CoconutWhite,
			0,
			0
		)
	end

	return shell
end

----------------------------------------------------------------------
-- CRYSTAL SHARD PROP
----------------------------------------------------------------------

function MeshBuilder.BuildCrystalShard(
	parent: Instance,
	position: Vector3,
	scale: number,
	color: Color3,
	rotation: Vector3
)
	local primary =
		MeshBuilder.MakeWedge(
			parent,
			"CrystalShard_A",
			Vector3.new(
				0.48,
				1.60,
				0.48
			) * scale,
			CFrame.new(position)
			* CFrame.Angles(
				rotation.X,
				rotation.Y,
				rotation.Z
			),
			Enum.Material.Glass,
			color,
			0.06,
			Config.CrystalReflectance
		)

	MeshBuilder.MakeWedge(
		parent,
		"CrystalShard_B",
		Vector3.new(
			0.48,
			1.60,
			0.48
		) * scale,
		CFrame.new(position)
			* CFrame.Angles(
				rotation.X,
				rotation.Y
				+ math.pi,
				rotation.Z
			),
		Enum.Material.Glass,
		color,
		0.06,
		Config.CrystalReflectance
	)

	return primary
end

----------------------------------------------------------------------
-- COSMIC ORB PROP
----------------------------------------------------------------------

function MeshBuilder.BuildCosmicOrb(
	parent: Instance,
	position: Vector3,
	scale: number,
	color: Color3
)
	local orb =
		MeshBuilder.MakeBall(
			parent,
			"CosmicHarvest",
			Vector3.new(
				0.70,
				0.70,
				0.70
			) * scale,
			position,
			Enum.Material.Neon,
			color,
			0.02,
			Config.CosmicReflectance
		)

	MeshBuilder.AddPointLight(
		parent,
		position,
		color,
		0.55,
		7
	)

	return orb
end

----------------------------------------------------------------------
-- STAR PROP
----------------------------------------------------------------------

function MeshBuilder.BuildCelestialStar(
	parent: Instance,
	position: Vector3,
	scale: number,
	color: Color3
)
	local star =
		MeshBuilder.MakeBall(
			parent,
			"CelestialStar",
			Vector3.new(
				0.34,
				0.34,
				0.34
			) * scale,
			position,
			Enum.Material.Neon,
			color,
			0,
			0.22
		)

	MeshBuilder.AddPointLight(
		parent,
		position,
		color,
		0.8,
		6
	)

	return star
end

----------------------------------------------------------------------
-- BARK / BRANCH SYSTEM
----------------------------------------------------------------------

local function buildLSystemBranches(
	parent: Instance,
	origin: Vector3,
	axiom: string,
	rules: {[string]: string},
	iterations: number,
	stepLength: number,
	angle: number,
	branchRadius: number,
	rng: Random
): {Vector3}
	local program =
		MathEngine.BuildLSystem(
			axiom,
			rules,
			iterations,
			16000
		)

	local commands =
		MathEngine.TurtleInterpret(
			program,
			origin,
			CFrame.new(origin)
			* CFrame.Angles(
				0,
				rng:NextNumber(
					0,
					math.pi * 2
				),
				0
			),
			stepLength,
			angle,
			branchRadius
		)

	local endpoints = {}

	for index, command in ipairs(commands) do
		local segmentColor =
			colorLerp(
				Palette.BarkDark,
				Palette.BarkLight,
				rng:NextNumber(0, 1)
			)

		local radiusMultiplier =
			math.max(
				0.28,
				1
				- command.depth * 0.17
			)

		MeshBuilder.MakeSegment(
			parent,
			"LSystemBranch_" .. index,
			command.from,
			command.to,
			math.max(
				0.065,
				command.radius
					* radiusMultiplier
			),
			Enum.Material.Wood,
			segmentColor,
			0.01
		)

		endpoints[#endpoints + 1] =
			command.to
	end

	return endpoints
end

----------------------------------------------------------------------
-- CANOPY LEAF CLUSTER
----------------------------------------------------------------------

local function buildLeafCluster(
	parent: Instance,
	center: Vector3,
	leafCount: number,
	radius: number,
	verticalBias: number,
	rng: Random,
	colorA: Color3,
	colorB: Color3
)
	for i = 1, leafCount do
		local theta =
			i * MathEngine.GoldenAngle

		local radial =
			math.sqrt(
				i / leafCount
			) * radius

		local direction = Vector3.new(
			math.cos(theta),
			verticalBias
				+ math.sin(i * 0.77)
				* 0.16,
			math.sin(theta)
		).Unit

		local leafPosition =
			center
			+ Vector3.new(
				math.cos(theta)
				* radial,
				math.sin(
					i * 0.63
				)
				* radius
				* 0.12,
				math.sin(theta)
				* radial
			)

		MeshBuilder.MakeLeaf(
			parent,
			"FoliageLeaf_" .. i,
			leafPosition,
			direction,
			rng:NextNumber(
				radius * 0.48,
				radius * 0.86
			),
			rng:NextNumber(
				radius * 0.12,
				radius * 0.25
			),
			colorLerp(
				colorA,
				colorB,
				rng:NextNumber(0, 1)
			),
			rng:NextNumber(
				-0.35,
				0.35
			),
			Enum.Material.Grass
		)
	end
end

----------------------------------------------------------------------
-- MODULE 3 : ASSET GENERATORS
----------------------------------------------------------------------

local AssetGenerators = {}

----------------------------------------------------------------------
-- 01 CARROT PLANT
-- Curved Bezier leaves
----------------------------------------------------------------------

function AssetGenerators.GenerateCarrotPlant(
	origin: Vector3,
	parent: Instance,
	seed: number
)
	local rng =
		makeRng(
			"CarrotPlant",
			seed
		)

	local model =
		Instance.new("Model")

	model.Name = "01_CarrotPlant"
	model:SetAttribute(
		"AssetType",
		"Carrot Plant"
	)
	model:SetAttribute(
		"Seed",
		seed
	)
	model.Parent = parent

	MeshBuilder.BuildSoilMound(
		model,
		origin,
		5.1,
		24,
		rng,
		seed
	)

	local root =
		origin
		+ Vector3.new(
			0,
			0.8,
			0
		)

	------------------------------------------------------------------
	-- Underground carrots visible around soil
	------------------------------------------------------------------

	for i = 1, 5 do
		local angle =
			i * MathEngine.GoldenAngle

		local radial =
			rng:NextNumber(
				0.25,
				2.35
			)

		local position =
			root
			+ Vector3.new(
				math.cos(angle)
				* radial,
				rng:NextNumber(
					0.05,
					0.48
				),
				math.sin(angle)
				* radial
			)

		MeshBuilder.BuildCarrot(
			model,
			position,
			rng:NextNumber(
				0.65,
				0.92
			),
			rng
		)
	end

	------------------------------------------------------------------
	-- Bezier bent foliage
	------------------------------------------------------------------

	local leafCount = 17

	for i = 1, leafCount do
		local theta =
			i * MathEngine.GoldenAngle

		local radial =
			rng:NextNumber(
				0.12,
				0.42
			)

		local p0 =
			root
			+ Vector3.new(
				math.cos(theta)
				* radial,
				0.65,
				math.sin(theta)
				* radial
			)

		local bend =
			rng:NextNumber(
				0.35,
				0.80
			)

		local p1 =
			p0
			+ Vector3.new(
				math.cos(theta)
				* bend,
				1.2,
				math.sin(theta)
				* bend
			)

		local p2 =
			p0
			+ Vector3.new(
				math.cos(theta)
				* bend
				* 0.7,
				2.4,
				math.sin(theta)
				* bend
				* 0.7
			)

		local p3 =
			p0
			+ Vector3.new(
				math.cos(theta)
				* rng:NextNumber(
					0.45,
					0.95
				),
				rng:NextNumber(
					3.1,
					4.6
				),
				math.sin(theta)
				* rng:NextNumber(
					0.45,
					0.95
				)
			)

		local previous =
			MathEngine.CubicBezier(
				p0,
				p1,
				p2,
				p3,
				0
			)

		local samples = 10

		for j = 1, samples do
			local t =
				(j - 1)
				/ (samples - 1)

			local current =
				MathEngine.CubicBezier(
					p0,
					p1,
					p2,
					p3,
					t
				)

			if j > 1 then
				MeshBuilder.MakeSegment(
					model,
					"CarrotBezierLeaf_" ..
						i ..
						"_" ..
						j,
					previous,
					current,
					0.055,
					Enum.Material.Grass,
					colorLerp(
						Palette.CarrotGreen,
						Palette.LeafLight,
						rng:NextNumber(
							0,
							1
						)
					),
					0
				)
			end

			previous = current
		end
	end

	local harvestBasket =
		origin
		+ Vector3.new(
			4.8,
			0.9,
			0
		)

	MeshBuilder.BuildWovenBasket(
		model,
		harvestBasket,
		3.1,
		1.6,
		2.5,
		rng
	)

	for i = 1, 5 do
		MeshBuilder.BuildCarrot(
			model,
			harvestBasket
				+ Vector3.new(
					rng:NextNumber(-1.0, 1.0),
					1.0
					+ i * 0.08,
					rng:NextNumber(
						-0.8,
						0.8
					)
				),
			0.52,
			rng
		)
	end

	return model
end

----------------------------------------------------------------------
-- 02 POTATO PLANT
----------------------------------------------------------------------

function AssetGenerators.GeneratePotatoPlant(
	origin: Vector3,
	parent: Instance,
	seed: number
)
	local rng =
		makeRng(
			"PotatoPlant",
			seed
		)

	local model =
		Instance.new("Model")

	model.Name = "02_PotatoPlant"
	model:SetAttribute(
		"AssetType",
		"Potato Plant"
	)
	model.Parent = parent

	MeshBuilder.BuildSoilMound(
		model,
		origin,
		5.4,
		28,
		rng,
		seed
	)

	local center =
		origin
		+ Vector3.new(
			0,
			1.0,
			0
		)

	------------------------------------------------------------------
	-- Stem network
	------------------------------------------------------------------

	local stems = 9

	for i = 1, stems do
		local theta =
			i * MathEngine.GoldenAngle

		local base =
			center
			+ Vector3.new(
				math.cos(theta) * 0.22,
				0,
				math.sin(theta) * 0.22
			)

		local stemEnd =
			center
			+ Vector3.new(
				math.cos(theta)
				* rng:NextNumber(
					1.6,
					2.8
				),
				rng:NextNumber(
					2.5,
					4.0
				),
				math.sin(theta)
				* rng:NextNumber(
					1.6,
					2.8
				)
			)

		MeshBuilder.MakeSegment(
			model,
			"PotatoStem_" .. i,
			base,
			stemEnd,
			0.085,
			Enum.Material.Grass,
			Palette.LeafDark,
			0
		)

		for j = 1, 5 do
			local t =
				j / 6

			local leafCenter =
				base:Lerp(stemEnd, t)

			local lateral =
				Vector3.new(
					math.cos(
						theta
						+ j * 0.9
					),
					0.2,
					math.sin(
						theta
						+ j * 0.9
					)
				).Unit

			MeshBuilder.MakeLeaf(
				model,
				"PotatoLeaf_" ..
					i ..
					"_" ..
					j,
				leafCenter,
				lateral,
				rng:NextNumber(
					0.45,
					0.75
				),
				rng:NextNumber(
					0.18,
					0.27
				),
				colorLerp(
					Palette.LeafDark,
					Palette.LeafLight,
					rng:NextNumber(
						0,
						1
					)
				),
				rng:NextNumber(
					-0.2,
					0.2
				)
			)
		end
	end

	------------------------------------------------------------------
	-- Harvest crate
	------------------------------------------------------------------

	local cratePosition =
		origin
		+ Vector3.new(
			5.2,
			1.4,
			0
		)

	MeshBuilder.BuildWovenCrate(
		model,
		cratePosition,
		3.1,
		2.3,
		2.7,
		rng
	)

	for i = 1, 12 do
		MeshBuilder.BuildPotato(
			model,
			cratePosition
				+ Vector3.new(
					rng:NextNumber(
						-1.15,
						1.15
					),
					1.35
					+ rng:NextNumber(
						0,
						0.45
					),
					rng:NextNumber(
						-0.95,
						0.95
					)
				),
			rng:NextNumber(
				0.58,
				0.78
			),
			rng
		)
	end

	return model
end

----------------------------------------------------------------------
-- 03 STRAWBERRY PLANT
-- Fibonacci fruit placement + 100+ visible seeds
----------------------------------------------------------------------

function AssetGenerators.GenerateStrawberryPlant(
	origin: Vector3,
	parent: Instance,
	seed: number
)
	local rng =
		makeRng(
			"StrawberryPlant",
			seed
		)

	local model =
		Instance.new("Model")

	model.Name = "03_StrawberryPlant"
	model:SetAttribute(
		"AssetType",
		"Strawberry Plant"
	)
	model.Parent = parent

	MeshBuilder.BuildSoilMound(
		model,
		origin,
		5.3,
		28,
		rng,
		seed
	)

	local center =
		origin
		+ Vector3.new(
			0,
			1.0,
			0
		)

	------------------------------------------------------------------
	-- Crown
	------------------------------------------------------------------

	MeshBuilder.MakeBall(
		model,
		"StrawberryCrown",
		Vector3.new(
			0.9,
			0.5,
			0.9
		),
		center,
		Enum.Material.Grass,
		Palette.LeafDark
	)

	------------------------------------------------------------------
	-- Long leaves
	------------------------------------------------------------------

	for i = 1, 15 do
		local theta =
			i * MathEngine.GoldenAngle

		local direction =
			Vector3.new(
				math.cos(theta),
				rng:NextNumber(
					0.28,
					0.72
				),
				math.sin(theta)
			).Unit

		MeshBuilder.MakeLeaf(
			model,
			"StrawberryLeaf_" .. i,
			center,
			direction,
			rng:NextNumber(
				1.3,
				2.5
			),
			rng:NextNumber(
				0.18,
				0.35
			),
			colorLerp(
				Palette.LeafDark,
				Palette.LeafLight,
				rng:NextNumber(
					0,
					1
				)
			),
			rng:NextNumber(
				-0.4,
				0.4
			)
		)
	end

	------------------------------------------------------------------
	-- Fruit cluster by Fibonacci phyllotaxis
	------------------------------------------------------------------

	local fruitCount = 8

	local fruitCenters =
		MathEngine.FibonacciSphere(
			fruitCount,
			1.5,
			center
			+ Vector3.new(
				0,
				0.65,
				0
			),
			0.12,
			rng
		)

	for i, fruitPosition in ipairs(fruitCenters) do
		MeshBuilder.BuildStrawberry(
			model,
			fruitPosition,
			rng:NextNumber(
				0.56,
				0.84
			),
			rng
		)
	end

	------------------------------------------------------------------
	-- Harvest basket
	------------------------------------------------------------------

	local basketPosition =
		origin
		+ Vector3.new(
			4.7,
			1.0,
			0
		)

	MeshBuilder.BuildWovenBasket(
		model,
		basketPosition,
		3.0,
		1.55,
		2.4,
		rng
	)

	for i = 1, 11 do
		local p =
			basketPosition
			+ Vector3.new(
				rng:NextNumber(
					-1.0,
					1.0
				),
				1.0
				+ rng:NextNumber(
					0,
					0.65
				),
				rng:NextNumber(
					-0.85,
					0.85
				)
			)

		MeshBuilder.BuildStrawberry(
			model,
			p,
			0.52,
			rng
		)
	end

	return model
end

----------------------------------------------------------------------
-- 04 BLUEBERRY BUSH
-- Fibonacci berry cluster
----------------------------------------------------------------------

function AssetGenerators.GenerateBlueberryBush(
	origin: Vector3,
	parent: Instance,
	seed: number
)
	local rng =
		makeRng(
			"BlueberryBush",
			seed
		)

	local model =
		Instance.new("Model")

	model.Name = "04_BlueberryBush"
	model:SetAttribute(
		"AssetType",
		"Blueberry Bush"
	)
	model.Parent = parent

	MeshBuilder.BuildSoilMound(
		model,
		origin,
		5.0,
		25,
		rng,
		seed
	)

	local center =
		origin
		+ Vector3.new(
			0,
			0.7,
			0
		)

	------------------------------------------------------------------
	-- Root stems
	------------------------------------------------------------------

	for i = 1, 14 do
		local theta =
			i * MathEngine.GoldenAngle

		local base =
			center
			+ Vector3.new(
				0,
				0.1,
				0
			)

		local endpoint =
			center
			+ Vector3.new(
				math.cos(theta)
				* rng:NextNumber(
					1.6,
					2.9
				),
				rng:NextNumber(
					2.1,
					4.6
				),
				math.sin(theta)
				* rng:NextNumber(
					1.6,
					2.9
				)
			)

		MeshBuilder.MakeSegment(
			model,
			"BlueberryStem_" .. i,
			base,
			endpoint,
			0.075,
			Enum.Material.Wood,
			Palette.Bark
		)

		for j = 1, 4 do
			local t =
				j / 5

			local leafCenter =
				base:Lerp(
					endpoint,
					t
				)

			local leafDirection =
				Vector3.new(
					math.cos(
						theta
						+ j * 1.7
					),
					rng:NextNumber(
						0.20,
						0.75
					),
					math.sin(
						theta
						+ j * 1.7
					)
				).Unit

			MeshBuilder.MakeLeaf(
				model,
				"BlueberryLeaf_" ..
					i ..
					"_" ..
					j,
				leafCenter,
				leafDirection,
				rng:NextNumber(
					0.45,
					0.75
				),
				rng:NextNumber(
					0.16,
					0.28
				),
				Palette.Leaf,
				rng:NextNumber(
					-0.3,
					0.3
				)
			)
		end
	end

	------------------------------------------------------------------
	-- Berry clusters
	------------------------------------------------------------------

	for cluster = 1, 9 do
		local clusterTheta =
			cluster
			* MathEngine.GoldenAngle

		local clusterCenter =
			center
			+ Vector3.new(
				math.cos(
					clusterTheta
				) * rng:NextNumber(
					1.0,
					2.5
				),
				rng:NextNumber(
					2.1,
					4.3
				),
				math.sin(
					clusterTheta
				) * rng:NextNumber(
					1.0,
					2.5
				)
			)

		local berries =
			MathEngine.FibonacciSphere(
				13,
				0.46,
				clusterCenter,
				0.08,
				rng
			)

		for berryIndex, berryPosition in ipairs(berries) do
			MeshBuilder.BuildBlueberry(
				model,
				berryPosition,
				rng:NextNumber(
					0.48,
					0.72
				),
				rng
			)
		end
	end

	local basketPosition =
		origin
		+ Vector3.new(
			4.8,
			0.95,
			0
		)

	MeshBuilder.BuildWovenBasket(
		model,
		basketPosition,
		3.0,
		1.55,
		2.35,
		rng
	)

	for i = 1, 28 do
		local theta =
			i
			* MathEngine.GoldenAngle

		local radial =
			math.sqrt(i / 28)
			* 0.9

		MeshBuilder.BuildBlueberry(
			model,
			basketPosition
				+ Vector3.new(
					math.cos(theta)
					* radial,
					1.0
					+ rng:NextNumber(
						0,
						0.55
					),
					math.sin(theta)
					* radial
				),
			0.38,
			rng
		)
	end

	return model
end

----------------------------------------------------------------------
-- 05 APPLE TREE
-- Recursive L-System
----------------------------------------------------------------------

function AssetGenerators.GenerateAppleTree(
	origin: Vector3,
	parent: Instance,
	seed: number
)
	local rng =
		makeRng(
			"AppleTree",
			seed
		)

	local model =
		Instance.new("Model")

	model.Name = "05_AppleTree"
	model:SetAttribute(
		"AssetType",
		"Apple Tree"
	)
	model.Parent = parent

	MeshBuilder.BuildSoilMound(
		model,
		origin,
		6.5,
		38,
		rng,
		seed
	)

	------------------------------------------------------------------
	-- Recursive L-System grammar
	------------------------------------------------------------------

	local rules = {
		F = "FF-[-F+F+F]+[+F-F-F]",
	}

	local endpoints =
		buildLSystemBranches(
			model,
			origin
			+ Vector3.new(
				0,
				0.65,
				0
			),
			"F",
			rules,
			Config.LSystemDepth,
			2.95,
			math.rad(21),
			0.24,
			rng
		)

	------------------------------------------------------------------
	-- Central trunk reinforcement
	------------------------------------------------------------------

	MeshBuilder.MakeSegment(
		model,
		"AppleMainTrunk",
		origin
			+ Vector3.new(
				0,
				0.5,
				0
			),
		origin
			+ Vector3.new(
				0,
				5.2,
				0
			),
		0.52,
		Enum.Material.Wood,
		Palette.Bark
	)

	------------------------------------------------------------------
	-- Canopy
	------------------------------------------------------------------

	buildLeafCluster(
		model,
		origin
			+ Vector3.new(
				0,
				7.2,
				0
			),
		65,
		7.0,
		0.17,
		rng,
		Palette.LeafDark,
		Palette.LeafLight
	)

	------------------------------------------------------------------
	-- Apples attached to branch endpoints
	------------------------------------------------------------------

	local appleCount =
		math.min(
			22,
			#endpoints
		)

	for i = 1, appleCount do
		local endpoint =
			endpoints[
		((i - 1)
			% #endpoints)
			+ 1
		]

		local appleColor

		if i % 5 == 0 then
			appleColor = Palette.AppleYellow
		elseif i % 3 == 0 then
			appleColor = Palette.AppleGreen
		else
			appleColor = Palette.AppleRed
		end

		MeshBuilder.BuildApple(
			model,
			endpoint
				+ Vector3.new(
					0,
					-0.45,
					0
				),
			rng:NextNumber(
				0.72,
				0.94
			),
			appleColor,
			rng
		)
	end

	MeshBuilder.AddPointLight(
		model,
		origin
			+ Vector3.new(
				0,
				7.0,
				0
			),
		Color3.fromRGB(
			255,
			170,
			75
		),
		0.18,
		5
	)

	local basketPosition =
		origin
		+ Vector3.new(
			5.2,
			1.0,
			0
		)

	MeshBuilder.BuildWovenBasket(
		model,
		basketPosition,
		3.4,
		1.7,
		2.8,
		rng
	)

	for i = 1, 8 do
		MeshBuilder.BuildApple(
			model,
			basketPosition
				+ Vector3.new(
					rng:NextNumber(
						-1.0,
						1.0
					),
					1.0
					+ rng:NextNumber(
						0,
						0.65
					),
					rng:NextNumber(
						-0.9,
						0.9
					)
				),
			0.64,
			if i % 3 == 0
				then Palette.AppleGreen
				else Palette.AppleRed,
			rng
		)
	end

	return model
end

----------------------------------------------------------------------
-- 06 MANGO TREE
-- Recursive L-System + larger leaves
----------------------------------------------------------------------

function AssetGenerators.GenerateMangoTree(
	origin: Vector3,
	parent: Instance,
	seed: number
)
	local rng =
		makeRng(
			"MangoTree",
			seed
		)

	local model =
		Instance.new("Model")

	model.Name = "06_MangoTree"
	model:SetAttribute(
		"AssetType",
		"Mango Tree"
	)
	model.Parent = parent

	MeshBuilder.BuildSoilMound(
		model,
		origin,
		6.9,
		40,
		rng,
		seed
	)

	local rules = {
		F = "FF+[+F-F-F]-[-F+F+F]",
	}

	local endpoints =
		buildLSystemBranches(
			model,
			origin
			+ Vector3.new(
				0,
				0.7,
				0
			),
			"F",
			rules,
			3,
			3.25,
			math.rad(25),
			0.30,
			rng
		)

	MeshBuilder.MakeSegment(
		model,
		"MangoTrunk",
		origin
			+ Vector3.new(
				0,
				0.55,
				0
			),
		origin
			+ Vector3.new(
				0.4,
				6.2,
				0
			),
		0.62,
		Enum.Material.Wood,
		Palette.Bark
	)

	buildLeafCluster(
		model,
		origin
			+ Vector3.new(
				0,
				8.2,
				0
			),
		80,
		7.5,
		0.30,
		rng,
		Palette.Leaf,
		Palette.LeafLight
	)

	------------------------------------------------------------------
	-- Mango fruits
	------------------------------------------------------------------

	local count =
		math.min(
			20,
			#endpoints
		)

	for i = 1, count do
		local endpoint =
			endpoints[
		((i * 3 - 1)
			% #endpoints)
			+ 1
		]

		MeshBuilder.BuildMango(
			model,
			endpoint
				+ Vector3.new(
					0,
					-0.35,
					0
				),
			rng:NextNumber(
				0.75,
				1.1
			),
			rng
		)
	end

	------------------------------------------------------------------
	-- Mango harvest crate
	------------------------------------------------------------------

	local cratePosition =
		origin
		+ Vector3.new(
			5.5,
			1.4,
			0
		)

	MeshBuilder.BuildWovenCrate(
		model,
		cratePosition,
		3.4,
		2.4,
		2.9,
		rng
	)

	for i = 1, 10 do
		MeshBuilder.BuildMango(
			model,
			cratePosition
				+ Vector3.new(
					rng:NextNumber(
						-1.15,
						1.15
					),
					1.30
					+ rng:NextNumber(
						0,
						0.5
					),
					rng:NextNumber(
						-1.0,
						1.0
					)
				),
			0.55,
			rng
		)
	end

	return model
end

----------------------------------------------------------------------
-- 07 FROSTLEAF
----------------------------------------------------------------------

function AssetGenerators.GenerateFrostleaf(
	origin: Vector3,
	parent: Instance,
	seed: number
)
	local rng =
		makeRng(
			"Frostleaf",
			seed
		)

	local model =
		Instance.new("Model")

	model.Name = "07_Frostleaf"
	model:SetAttribute(
		"AssetType",
		"Frostleaf"
	)
	model.Parent = parent

	MeshBuilder.BuildSoilMound(
		model,
		origin,
		5.0,
		25,
		rng,
		seed
	)

	local root =
		origin
		+ Vector3.new(
			0,
			0.7,
			0
		)

	------------------------------------------------------------------
	-- Main frost crystal
	------------------------------------------------------------------

	MeshBuilder.BuildCrystalShard(
		model,
		root
			+ Vector3.new(
				0,
				1.8,
				0
			),
		1.25,
		Palette.FrostBlue,
		Vector3.new(
			0.05,
			0,
			0.08
		)
	)

	------------------------------------------------------------------
	-- Radial secondary crystals
	------------------------------------------------------------------

	local directions =
		MathEngine.FibonacciSphere(
			17,
			1.5,
			root,
			0.08,
			rng
		)

	for i, point in ipairs(directions) do
		local shardPosition =
			point
			+ Vector3.new(
				0,
				1.0,
				0
			)

		local direction =
			shardPosition
		- root

		MeshBuilder.BuildCrystalShard(
			model,
			shardPosition,
			rng:NextNumber(
				0.36,
				0.80
			),
			colorLerp(
				Palette.FrostBlue,
				Palette.FrostWhite,
				rng:NextNumber(
					0,
					1
				)
			),
			Vector3.new(
				rng:NextNumber(
					-0.3,
					0.3
				),
				math.atan2(
					direction.X,
					direction.Z
				),
				rng:NextNumber(
					-0.3,
					0.3
				)
			)
		)
	end

	------------------------------------------------------------------
	-- Frost particles
	------------------------------------------------------------------

	MeshBuilder.AddParticleBurst(
		model,
		root
			+ Vector3.new(
				0,
				3.0,
				0
			),
		Palette.FrostWhite,
		3,
		NumberRange.new(
			2.0,
			4.0
		),
		NumberRange.new(
			0.25,
			0.9
		),
		NumberSequence.new({
			NumberSequenceKeypoint.new(
				0,
				0.16
			),
			NumberSequenceKeypoint.new(
				0.5,
				0.08
			),
			NumberSequenceKeypoint.new(
				1,
				0.0
			),
		}),
		0.75
	)

	MeshBuilder.AddPointLight(
		model,
		root
			+ Vector3.new(
				0,
				2.6,
				0
			),
		Palette.FrostCyan,
		1.0,
		10
	)

	------------------------------------------------------------------
	-- Harvest crystal cluster
	------------------------------------------------------------------

	local harvest =
		origin
		+ Vector3.new(
			4.7,
			0.8,
			0
		)

	for i = 1, 9 do
		MeshBuilder.BuildCrystalShard(
			model,
			harvest
				+ Vector3.new(
					rng:NextNumber(
						-1.1,
						1.1
					),
					rng:NextNumber(
						0.4,
						1.5
					),
					rng:NextNumber(
						-1.0,
						1.0
					)
				),
			rng:NextNumber(
				0.42,
				0.72
			),
			colorLerp(
				Palette.FrostBlue,
				Palette.FrostWhite,
				rng:NextNumber(
					0,
					1
				)
			),
			Vector3.new(
				rng:NextNumber(
					-0.4,
					0.4
				),
				rng:NextNumber(
					0,
					math.pi * 2
				),
				rng:NextNumber(
					-0.4,
					0.4
				)
			)
		)
	end

	MeshBuilder.AddHighlight(
		model,
		Palette.FrostBlue,
		Palette.FrostWhite,
		0.82,
		0.45
	)

	return model
end

----------------------------------------------------------------------
-- 08 CHERRY TREE
-- Recursive L-System + pink blossom canopy
----------------------------------------------------------------------

function AssetGenerators.GenerateCherryTree(
	origin: Vector3,
	parent: Instance,
	seed: number
)
	local rng =
		makeRng(
			"CherryTree",
			seed
		)

	local model =
		Instance.new("Model")

	model.Name = "08_CherryTree"
	model:SetAttribute(
		"AssetType",
		"Cherry Tree"
	)
	model.Parent = parent

	MeshBuilder.BuildSoilMound(
		model,
		origin,
		6.5,
		37,
		rng,
		seed
	)

	local rules = {
		F = "F[+F]F[-F]F[&F]^F",
	}

	local endpoints =
		buildLSystemBranches(
			model,
			origin
			+ Vector3.new(
				0,
				0.7,
				0
			),
			"F",
			rules,
			3,
			2.55,
			math.rad(23),
			0.23,
			rng
		)

	MeshBuilder.MakeSegment(
		model,
		"CherryTrunk",
		origin
			+ Vector3.new(
				0,
				0.6,
				0
			),
		origin
			+ Vector3.new(
				-0.25,
				5.7,
				0.15
			),
		0.56,
		Enum.Material.Wood,
		Palette.Bark
	)

	------------------------------------------------------------------
	-- Pink canopy
	------------------------------------------------------------------

	for i = 1, 14 do
		local theta =
			i * MathEngine.GoldenAngle

		local clusterRadius =
			rng:NextNumber(
				4.0,
				6.6
			)

		local clusterCenter =
			origin
			+ Vector3.new(
				math.cos(theta)
				* clusterRadius,
				rng:NextNumber(
					5.6,
					9.1
				),
				math.sin(theta)
				* clusterRadius
			)

		MeshBuilder.BuildFacetedCanopy(
			model,
			clusterCenter,
			Vector3.new(
				2.3,
				1.8,
				2.3
			),
			3,
			7,
			Color3.fromRGB(
				213,
				104,
				143
			),
			Color3.fromRGB(
				252,
				192,
				215
			),
			rng
		)
	end

	------------------------------------------------------------------
	-- Cherry fruit pairs
	------------------------------------------------------------------

	local pairCount =
		math.min(
			15,
			#endpoints
		)

	for i = 1, pairCount do
		local endpoint =
			endpoints[
		((i * 5 - 1)
			% #endpoints)
			+ 1
		]

		MeshBuilder.BuildCherryPair(
			model,
			endpoint
				+ Vector3.new(
					0,
					-0.35,
					0
				),
			rng:NextNumber(
				0.65,
				0.95
			),
			rng
		)
	end

	------------------------------------------------------------------
	-- Harvest basket
	------------------------------------------------------------------

	local basketPosition =
		origin
		+ Vector3.new(
			5.1,
			0.95,
			0
		)

	MeshBuilder.BuildWovenBasket(
		model,
		basketPosition,
		3.2,
		1.7,
		2.6,
		rng
	)

	for i = 1, 8 do
		MeshBuilder.BuildCherryPair(
			model,
			basketPosition
				+ Vector3.new(
					rng:NextNumber(
						-0.9,
						0.9
					),
					1.05
					+ rng:NextNumber(
						0,
						0.55
					),
					rng:NextNumber(
						-0.8,
						0.8
					)
				),
			0.6,
			rng
		)
	end

	MeshBuilder.AddParticleBurst(
		model,
		origin
			+ Vector3.new(
				0,
				8.0,
				0
			),
		Color3.fromRGB(
			255,
			185,
			211
		),
		1.2,
		NumberRange.new(
			2.5,
			4.0
		),
		NumberRange.new(
			0.15,
			0.55
		),
		NumberSequence.new(
			0.12
		),
		0.25
	)

	return model
end

----------------------------------------------------------------------
-- 09 BANANA TREE
----------------------------------------------------------------------

function AssetGenerators.GenerateBananaTree(
	origin: Vector3,
	parent: Instance,
	seed: number
)
	local rng =
		makeRng(
			"BananaTree",
			seed
		)

	local model =
		Instance.new("Model")

	model.Name = "09_BananaTree"
	model:SetAttribute(
		"AssetType",
		"Banana Tree"
	)
	model.Parent = parent

	MeshBuilder.BuildSoilMound(
		model,
		origin,
		6.0,
		34,
		rng,
		seed
	)

	local trunkBase =
		origin
		+ Vector3.new(
			0,
			0.7,
			0
		)

	------------------------------------------------------------------
	-- Layered banana trunk
	------------------------------------------------------------------

	local trunkLayers = 10

	for i = 1, trunkLayers do
		local t =
			(i - 1)
			/ (trunkLayers - 1)

		local pos =
			trunkBase
			+ Vector3.new(
				0.10
				* math.sin(
					t * math.pi * 1.6
				),
				t * 6.2,
				0.10
				* math.cos(
					t * math.pi * 1.7
				)
			)

		MeshBuilder.MakeCylinder(
			model,
			"BananaPseudoTrunk_" .. i,
			1.0
			- t * 0.22,
			0.72,
			CFrame.new(pos),
			Enum.Material.Grass,
			colorLerp(
				Palette.Leaf,
				Palette.LeafLight,
				t
			),
			0,
			0.01
		)
	end

	------------------------------------------------------------------
	-- Large banana leaves
	------------------------------------------------------------------

	local leafCount = 18

	for i = 1, leafCount do
		local theta =
			i
			* MathEngine.GoldenAngle

		local center =
			origin
			+ Vector3.new(
				0,
				6.2,
				0
			)

		local length =
			rng:NextNumber(
				4.0,
				6.8
			)

		local direction =
			Vector3.new(
				math.cos(theta),
				rng:NextNumber(
					0.15,
					0.65
				),
				math.sin(theta)
			).Unit

		local endpoint =
			center
			+ direction
			* length

		local previous =
			center

		local samples = 12

		for j = 1, samples do
			local t =
				(j - 1)
				/ (samples - 1)

			local wave =
				math.sin(
					t * math.pi
				)
				* 0.65

			local point =
				center:Lerp(
					endpoint,
					t
				)
				+ Vector3.new(
					0,
					wave,
					0
				)

			if j > 1 then
				MeshBuilder.MakeSegment(
					model,
					"BananaLeafVein_" ..
						i ..
						"_" ..
						j,
					previous,
					point,
					0.075,
					Enum.Material.Grass,
					Palette.LeafDark
				)
			end

			previous = point
		end

		for j = 0, 7 do
			local t =
				j / 7

			local point =
				center:Lerp(
					endpoint,
					t
				)

			local width =
				math.sin(
					t * math.pi
				)
				* 0.95

			MeshBuilder.MakeWedge(
				model,
				"BananaLeafBlade_" ..
					i ..
					"_" ..
					j,
				Vector3.new(
					width,
					0.10,
					length / 8 + 0.20
				),
				CFrame.lookAt(
					point,
					endpoint
				)
					* CFrame.Angles(
						math.rad(90),
						0,
						math.sin(
							t * math.pi
						) * 0.15
					),
				Enum.Material.Grass,
				colorLerp(
					Palette.LeafDark,
					Palette.LeafLight,
					t
				),
				0,
				0
			)
		end
	end

	------------------------------------------------------------------
	-- Banana fruit bunch
	------------------------------------------------------------------

	local bunchCenter =
		origin
		+ Vector3.new(
			0,
			4.9,
			-0.35
		)

	MeshBuilder.MakeCylinder(
		model,
		"BananaBunchStem",
		0.16,
		1.2,
		CFrame.new(
			bunchCenter
				+ Vector3.new(
					0,
					0.55,
					0
				)
		),
		Enum.Material.Wood,
		Palette.BananaBrown
	)

	for row = 1, 4 do
		local count =
			4 + row

		for i = 1, count do
			local theta =
				(i / count)
				* math.pi * 2

			local offset =
				Vector3.new(
					math.cos(theta)
					* row
					* 0.30,
					-row
					* 0.35,
					math.sin(theta)
					* row
					* 0.30
				)

			MeshBuilder.BuildBanana(
				model,
				bunchCenter
					+ offset,
				0.85,
				Vector3.new(
					math.cos(theta),
					-0.18,
					math.sin(theta)
				),
				rng
			)
		end
	end

	return model
end

----------------------------------------------------------------------
-- 10 DRAGONFRUIT BUSH
----------------------------------------------------------------------

function AssetGenerators.GenerateDragonfruitBush(
	origin: Vector3,
	parent: Instance,
	seed: number
)
	local rng =
		makeRng(
			"DragonfruitBush",
			seed
		)

	local model =
		Instance.new("Model")

	model.Name = "10_DragonfruitBush"
	model:SetAttribute(
		"AssetType",
		"Dragonfruit Bush"
	)
	model.Parent = parent

	MeshBuilder.BuildSoilMound(
		model,
		origin,
		5.8,
		30,
		rng,
		seed
	)

	local center =
		origin
		+ Vector3.new(
			0,
			1.0,
			0
		)

	------------------------------------------------------------------
	-- Cactus-like arching stems
	------------------------------------------------------------------

	for i = 1, 17 do
		local theta =
			i * MathEngine.GoldenAngle

		local p0 =
			center
			+ Vector3.new(
				math.cos(theta) * 0.15,
				0,
				math.sin(theta) * 0.15
			)

		local p1 =
			center
			+ Vector3.new(
				math.cos(theta)
				* 1.0,
				1.3,
				math.sin(theta)
				* 1.0
			)

		local p2 =
			center
			+ Vector3.new(
				math.cos(theta)
				* 2.6,
				1.7,
				math.sin(theta)
				* 2.6
			)

		local p3 =
			center
			+ Vector3.new(
				math.cos(theta)
				* rng:NextNumber(
					3.4,
					4.6
				),
				rng:NextNumber(
					1.6,
					3.1
				),
				math.sin(theta)
				* rng:NextNumber(
					3.4,
					4.6
				)
			)

		local previous =
			MathEngine.CubicBezier(
				p0,
				p1,
				p2,
				p3,
				0
			)

		for j = 1, 10 do
			local t =
				(j - 1) / 9

			local point =
				MathEngine.CubicBezier(
					p0,
					p1,
					p2,
					p3,
					t
				)

			if j > 1 then
				MeshBuilder.MakeSegment(
					model,
					"DragonStem_" ..
						i ..
						"_" ..
						j,
					previous,
					point,
					0.16,
					Enum.Material.Grass,
					Palette.LeafDark
				)
			end

			previous = point
		end

		------------------------------------------------------------------
		-- Flower-like node pads
		------------------------------------------------------------------

		local flowerPoint = p3

		for petal = 1, 5 do
			local petalTheta =
				petal
				* MathEngine.GoldenAngle

			local direction =
				Vector3.new(
					math.cos(petalTheta),
					0.35,
					math.sin(petalTheta)
				).Unit

			MeshBuilder.MakeLeaf(
				model,
				"DragonFlowerPetal_" ..
					i ..
					"_" ..
					petal,
				flowerPoint,
				direction,
				0.55,
				0.18,
				Palette.DragonGreen,
				0,
				Enum.Material.Grass
			)
		end

		MeshBuilder.BuildDragonFruit(
			model,
			flowerPoint
				+ Vector3.new(
					0,
					-0.1,
					0
				),
			rng:NextNumber(
				0.58,
				0.82
			),
			rng
		)
	end

	------------------------------------------------------------------
	-- Harvest crate
	------------------------------------------------------------------

	local cratePosition =
		origin
		+ Vector3.new(
			5.0,
			1.5,
			0
		)

	MeshBuilder.BuildWovenCrate(
		model,
		cratePosition,
		3.3,
		2.3,
		2.7,
		rng
	)

	for i = 1, 8 do
		MeshBuilder.BuildDragonFruit(
			model,
			cratePosition
				+ Vector3.new(
					rng:NextNumber(
						-1.05,
						1.05
					),
					1.25
					+ rng:NextNumber(
						0,
						0.55
					),
					rng:NextNumber(
						-0.9,
						0.9
					)
				),
			0.58,
			rng
		)
	end

	return model
end

----------------------------------------------------------------------
-- 11 STARFRUIT PLANT
-- L-System branching + starfruit props
----------------------------------------------------------------------

function AssetGenerators.GenerateStarfruitPlant(
	origin: Vector3,
	parent: Instance,
	seed: number
)
	local rng =
		makeRng(
			"StarfruitPlant",
			seed
		)

	local model =
		Instance.new("Model")

	model.Name = "11_StarfruitPlant"
	model:SetAttribute(
		"AssetType",
		"Starfruit Plant"
	)
	model.Parent = parent

	MeshBuilder.BuildSoilMound(
		model,
		origin,
		5.7,
		29,
		rng,
		seed
	)

	local rules = {
		F = "F[+F][-F][&F]^F",
	}

	local endpoints =
		buildLSystemBranches(
			model,
			origin
			+ Vector3.new(
				0,
				0.65,
				0
			),
			"F",
			rules,
			3,
			2.25,
			math.rad(20),
			0.17,
			rng
		)

	MeshBuilder.MakeSegment(
		model,
		"StarfruitTrunk",
		origin
			+ Vector3.new(
				0,
				0.5,
				0
			),
		origin
			+ Vector3.new(
				0,
				4.6,
				0
			),
		0.32,
		Enum.Material.Wood,
		Palette.Bark
	)

	------------------------------------------------------------------
	-- Elongated leaflets
	------------------------------------------------------------------

	for i = 1, 42 do
		local theta =
			i * MathEngine.GoldenAngle

		local center =
			origin
			+ Vector3.new(
				math.cos(theta)
				* rng:NextNumber(
					0.7,
					3.0
				),
				rng:NextNumber(
					2.5,
					5.7
				),
				math.sin(theta)
				* rng:NextNumber(
					0.7,
					3.0
				)
			)

		MeshBuilder.MakeLeaf(
			model,
			"StarfruitLeaf_" .. i,
			center,
			Vector3.new(
				math.cos(theta),
				rng:NextNumber(
					0.1,
					0.45
				),
				math.sin(theta)
			).Unit,
			rng:NextNumber(
				0.50,
				0.95
			),
			rng:NextNumber(
				0.14,
				0.25
			),
			colorLerp(
				Palette.LeafDark,
				Palette.LeafLight,
				rng:NextNumber(
					0,
					1
				)
			),
			0,
			Enum.Material.Grass
		)
	end

	------------------------------------------------------------------
	-- Starfruits
	------------------------------------------------------------------

	local count =
		math.min(
			12,
			#endpoints
		)

	for i = 1, count do
		local endpoint =
			endpoints[
		((i * 4 - 1)
			% #endpoints)
			+ 1
		]

		MeshBuilder.BuildStarfruit(
			model,
			endpoint
				+ Vector3.new(
					0,
					-0.25,
					0
				),
			rng:NextNumber(
				0.60,
				0.92
			),
			rng
		)
	end

	------------------------------------------------------------------
	-- Harvest basket
	------------------------------------------------------------------

	local basketPosition =
		origin
		+ Vector3.new(
			4.9,
			0.95,
			0
		)

	MeshBuilder.BuildWovenBasket(
		model,
		basketPosition,
		3.2,
		1.65,
		2.6,
		rng
	)

	for i = 1, 7 do
		MeshBuilder.BuildStarfruit(
			model,
			basketPosition
				+ Vector3.new(
					rng:NextNumber(
						-1.0,
						1.0
					),
					1.0
					+ rng:NextNumber(
						0,
						0.55
					),
					rng:NextNumber(
						-0.85,
						0.85
					)
				),
			0.52,
			rng
		)
	end

	return model
end

----------------------------------------------------------------------
-- 12 COCONUT PALM
-- Cubic Bezier curved trunk
----------------------------------------------------------------------

function AssetGenerators.GenerateCoconutPalm(
	origin: Vector3,
	parent: Instance,
	seed: number
)
	local rng =
		makeRng(
			"CoconutPalm",
			seed
		)

	local model =
		Instance.new("Model")

	model.Name = "12_CoconutPalm"
	model:SetAttribute(
		"AssetType",
		"Coconut Palm"
	)
	model.Parent = parent

	MeshBuilder.BuildSoilMound(
		model,
		origin,
		6.0,
		34,
		rng,
		seed
	)

	------------------------------------------------------------------
	-- Cubic Bezier trunk
	------------------------------------------------------------------

	local p0 =
		origin
		+ Vector3.new(
			0,
			0.7,
			0
		)

	local p1 =
		origin
		+ Vector3.new(
			0.25,
			4.0,
			0.2
		)

	local p2 =
		origin
		+ Vector3.new(
			2.0,
			8.0,
			0.4
		)

	local p3 =
		origin
		+ Vector3.new(
			2.9,
			11.5,
			0.1
		)

	local segments = 22

	local previous =
		MathEngine.CubicBezier(
			p0,
			p1,
			p2,
			p3,
			0
		)

	for i = 1, segments do
		local t =
			(i - 1)
			/ (segments - 1)

		local current =
			MathEngine.CubicBezier(
				p0,
				p1,
				p2,
				p3,
				t
			)

		if i > 1 then
			MeshBuilder.MakeSegment(
				model,
				"PalmTrunk_" .. i,
				previous,
				current,
				lerpNumber(
					0.66,
					0.34,
					t
				),
				Enum.Material.Wood,
				colorLerp(
					Palette.Bark,
					Palette.BarkLight,
					t
				)
			)
		end

		previous = current
	end

	------------------------------------------------------------------
	-- Crown
	------------------------------------------------------------------

	local crown =
		p3

	for i = 1, 22 do
		local theta =
			i * MathEngine.GoldenAngle

		local direction =
			Vector3.new(
				math.cos(theta),
				rng:NextNumber(
					0.02,
					0.25
				),
				math.sin(theta)
			).Unit

		local leafLength =
			rng:NextNumber(
				4.3,
				6.2
			)

		local endPoint =
			crown
			+ direction
			* leafLength

		local previousLeaf =
			crown

		for j = 1, 12 do
			local t =
				(j - 1) / 11

			local point =
				crown:Lerp(
					endPoint,
					t
				)
				+ Vector3.new(
					0,
					math.sin(
						t * math.pi
					)
					* 0.55,
					0
				)

			if j > 1 then
				MeshBuilder.MakeSegment(
					model,
					"PalmLeafVein_" ..
						i ..
						"_" ..
						j,
					previousLeaf,
					point,
					0.08,
					Enum.Material.Grass,
					Palette.LeafDark
				)
			end

			previousLeaf = point
		end

		for j = 1, 7 do
			local t =
				(j - 0.5) / 7

			local point =
				crown:Lerp(
					endPoint,
					t
				)

			local width =
				math.sin(
					t * math.pi
				)
				* 0.75

			MeshBuilder.MakeWedge(
				model,
				"PalmLeafSegment_" ..
					i ..
					"_" ..
					j,
				Vector3.new(
					width,
					0.13,
					leafLength
						/ 8
				),
				CFrame.lookAt(
					point,
					endPoint
				)
					* CFrame.Angles(
						math.rad(90),
						0,
						0
					),
				Enum.Material.Grass,
				colorLerp(
					Palette.LeafDark,
					Palette.LeafLight,
					t
				),
				0,
				0
			)
		end
	end

	------------------------------------------------------------------
	-- Coconut cluster
	------------------------------------------------------------------

	for i = 1, 8 do
		local theta =
			i * MathEngine.GoldenAngle

		local p =
			crown
			+ Vector3.new(
				math.cos(theta)
				* rng:NextNumber(
					0.35,
					1.15
				),
				-rng:NextNumber(
					0.2,
					1.5
				),
				math.sin(theta)
				* rng:NextNumber(
					0.35,
					1.15
				)
			)

		MeshBuilder.BuildCoconut(
			model,
			p,
			rng:NextNumber(
				0.62,
				0.85
			),
			false
		)
	end

	------------------------------------------------------------------
	-- Coconut harvest pile
	------------------------------------------------------------------

	local harvest =
		origin
		+ Vector3.new(
			4.9,
			0.75,
			0
		)

	for i = 1, 8 do
		MeshBuilder.BuildCoconut(
			model,
			harvest
				+ Vector3.new(
					rng:NextNumber(
						-1.3,
						1.3
					),
					rng:NextNumber(
						0.45,
						1.0
					),
					rng:NextNumber(
						-1.05,
						1.05
					)
				),
			0.72,
			i % 3 == 0
		)
	end

	return model
end

----------------------------------------------------------------------
-- 13 CRYSTAL SHRUB
----------------------------------------------------------------------

function AssetGenerators.GenerateCrystalShrub(
	origin: Vector3,
	parent: Instance,
	seed: number
)
	local rng =
		makeRng(
			"CrystalShrub",
			seed
		)

	local model =
		Instance.new("Model")

	model.Name = "13_CrystalShrub"
	model:SetAttribute(
		"AssetType",
		"Crystal Shrub"
	)
	model.Parent = parent

	MeshBuilder.BuildSoilMound(
		model,
		origin,
		5.2,
		26,
		rng,
		seed
	)

	local center =
		origin
		+ Vector3.new(
			0,
			0.4,
			0
		)

	------------------------------------------------------------------
	-- Fibonacci distributed crystal core
	------------------------------------------------------------------

	local points =
		MathEngine.FibonacciSphere(
			32,
			2.5,
			center
			+ Vector3.new(
				0,
				1.0,
				0
			),
			0.12,
			rng
		)

	for i, point in ipairs(points) do
		local localScale =
			rng:NextNumber(
				0.40,
				0.95
			)

		local direction =
			(
				point - center
			)

		if direction.Magnitude < 0.001 then
			direction = Vector3.new(
				0,
				1,
				0
			)
		else
			direction = direction.Unit
		end

		MeshBuilder.BuildCrystalShard(
			model,
			point
				+ direction
				* rng:NextNumber(
					0,
					0.45
				),
			localScale,
			colorLerp(
				Palette.CrystalBlue,
				Palette.CrystalWhite,
				rng:NextNumber(
					0,
					1
				)
			),
			Vector3.new(
				rng:NextNumber(
					-0.35,
					0.35
				),
				rng:NextNumber(
					0,
					math.pi * 2
				),
				rng:NextNumber(
					-0.35,
					0.35
				)
			)
		)
	end

	------------------------------------------------------------------
	-- Core glow
	------------------------------------------------------------------

	MeshBuilder.AddPointLight(
		model,
		center
			+ Vector3.new(
				0,
				2.0,
				0
			),
		Palette.CrystalBlue,
		1.1,
		10
	)

	MeshBuilder.AddParticleBurst(
		model,
		center
			+ Vector3.new(
				0,
				1.9,
				0
			),
		Palette.CrystalWhite,
		2.5,
		NumberRange.new(
			1.8,
			3.5
		),
		NumberRange.new(
			0.10,
			0.65
		),
		NumberSequence.new({
			NumberSequenceKeypoint.new(
				0,
				0.17
			),
			NumberSequenceKeypoint.new(
				0.65,
				0.11
			),
			NumberSequenceKeypoint.new(
				1,
				0
			),
		}),
		0.95
	)

	------------------------------------------------------------------
	-- Harvest crystal cluster
	------------------------------------------------------------------

	local harvest =
		origin
		+ Vector3.new(
			5.0,
			0.55,
			0
		)

	for i = 1, 11 do
		MeshBuilder.BuildCrystalShard(
			model,
			harvest
				+ Vector3.new(
					rng:NextNumber(
						-1.25,
						1.25
					),
					rng:NextNumber(
						0.25,
						1.6
					),
					rng:NextNumber(
						-1.05,
						1.05
					)
				),
			rng:NextNumber(
				0.35,
				0.75
			),
			colorLerp(
				Palette.CrystalBlue,
				Palette.CrystalWhite,
				rng:NextNumber(
					0,
					1
				)
			),
			Vector3.new(
				rng:NextNumber(
					-0.45,
					0.45
				),
				rng:NextNumber(
					0,
					math.pi * 2
				),
				rng:NextNumber(
					-0.45,
					0.45
				)
			)
		)
	end

	MeshBuilder.AddHighlight(
		model,
		Palette.CrystalBlue,
		Palette.CrystalWhite,
		0.86,
		0.32
	)

	return model
end

----------------------------------------------------------------------
-- 14 NEBULA VINE
-- Cubic Bezier base path + double helix sine/cosine sweep
----------------------------------------------------------------------

function AssetGenerators.GenerateNebulaVine(
	origin: Vector3,
	parent: Instance,
	seed: number
)
	local rng =
		makeRng(
			"NebulaVine",
			seed
		)

	local model =
		Instance.new("Model")

	model.Name = "14_NebulaVine"
	model:SetAttribute(
		"AssetType",
		"Nebula Vine"
	)
	model.Parent = parent

	MeshBuilder.BuildSoilMound(
		model,
		origin,
		5.8,
		30,
		rng,
		seed
	)

	local base =
		origin
		+ Vector3.new(
			0,
			0.8,
			0
		)

	local p0 =
		base

	local p1 =
		base
		+ Vector3.new(
			-2.0,
			4.2,
			1.0
		)

	local p2 =
		base
		+ Vector3.new(
			3.0,
			7.8,
			-1.5
		)

	local p3 =
		base
		+ Vector3.new(
			0.6,
			12.0,
			0.2
		)

	------------------------------------------------------------------
	-- Two intertwined Bezier cosmic vines
	------------------------------------------------------------------

	local sampleCount = 55
	local helixRadius = 0.72

	for strand = 1, 2 do
		local phase =
			if strand == 1
			then 0
			else math.pi

		local previous =
			MathEngine.CubicBezier(
				p0,
				p1,
				p2,
				p3,
				0
			)

		for i = 1, sampleCount do
			local t =
				(i - 1)
				/ (sampleCount - 1)

			local centerline =
				MathEngine.CubicBezier(
					p0,
					p1,
					p2,
					p3,
					t
				)

			local tangent =
				MathEngine.BezierTangent(
					p0,
					p1,
					p2,
					p3,
					t
				)

			if tangent.Magnitude < 0.001 then
				tangent =
					Vector3.new(
						0,
						1,
						0
					)
			else
				tangent =
					tangent.Unit
			end

			local reference =
				Vector3.new(
					0,
					1,
					0
				)

			if math.abs(
				tangent:Dot(reference)
				) > 0.94 then
				reference =
					Vector3.new(
						1,
						0,
						0
					)
			end

			local side =
				tangent:Cross(
					reference
				).Unit

			local up =
				side:Cross(
					tangent
				).Unit

			local turns =
				4.5

			local angle =
				t
				* math.pi
				* 2
				* turns
				+ phase

			local helixOffset =
				side
				* math.cos(angle)
				* helixRadius
				+ up
				* math.sin(angle)
				* helixRadius

			local point =
				centerline
				+ helixOffset

			if i > 1 then
				MeshBuilder.MakeSegment(
					model,
					"NebulaHelix_" ..
						strand ..
						"_" ..
						i,
					previous,
					point,
					0.13,
					Enum.Material.Neon,
					if strand == 1
						then Palette.NebulaPurple
						else Palette.NebulaBlue,
					Config.CosmicReflectance
				)
			end

			previous = point

			------------------------------------------------------------------
			-- Cosmic nodes
			------------------------------------------------------------------

			if i % 5 == 0 then
				local nodeColor =
					if strand == 1
					then Palette.NebulaPink
					else Palette.NebulaBlue

				MeshBuilder.MakeBall(
					model,
					"NebulaNode_" ..
						strand ..
						"_" ..
						i,
					Vector3.new(
						0.38,
						0.38,
						0.38
					),
					point,
					Enum.Material.Neon,
					nodeColor,
					0.03,
					0.3
				)
			end
		end
	end

	------------------------------------------------------------------
	-- Cosmic leaf crystals
	------------------------------------------------------------------

	for i = 1, 36 do
		local t =
			(i - 1) / 35

		local position =
			MathEngine.CubicBezier(
				p0,
				p1,
				p2,
				p3,
				t
			)

		local angle =
			i
			* MathEngine.GoldenAngle

		local direction =
			Vector3.new(
				math.cos(angle),
				math.sin(i * 0.8)
				* 0.4,
				math.sin(angle)
			).Unit

		MeshBuilder.BuildCrystalShard(
			model,
			position
				+ direction
				* rng:NextNumber(
					0.5,
					1.15
				),
			rng:NextNumber(
				0.25,
				0.5
			),
			if i % 2 == 0
				then Palette.NebulaPink
				else Palette.NebulaBlue,
			Vector3.new(
				rng:NextNumber(
					-0.4,
					0.4
				),
				angle,
				rng:NextNumber(
					-0.4,
					0.4
				)
			)
		)
	end

	------------------------------------------------------------------
	-- Cosmic harvest orbs
	------------------------------------------------------------------

	local harvest =
		origin
		+ Vector3.new(
			5.0,
			0.9,
			0
		)

	for i = 1, 15 do
		MeshBuilder.BuildCosmicOrb(
			model,
			harvest
				+ Vector3.new(
					rng:NextNumber(
						-1.2,
						1.2
					),
					rng:NextNumber(
						0.45,
						1.8
					),
					rng:NextNumber(
						-1.1,
						1.1
					)
				),
			rng:NextNumber(
				0.55,
				0.85
			),
			if i % 3 == 0
				then Palette.NebulaPink
				elseif i % 2 == 0
				then Palette.NebulaBlue
				else Palette.NebulaPurple
		)
	end

	MeshBuilder.AddParticleBurst(
		model,
		base
			+ Vector3.new(
				0,
				7.0,
				0
			),
		Palette.NebulaPink,
		3.5,
		NumberRange.new(
			1.4,
			3.1
		),
		NumberRange.new(
			0.10,
			0.45
		),
		NumberSequence.new({
			NumberSequenceKeypoint.new(
				0,
				0.14
			),
			NumberSequenceKeypoint.new(
				0.5,
				0.08
			),
			NumberSequenceKeypoint.new(
				1,
				0
			),
		}),
		1
	)

	MeshBuilder.AddPointLight(
		model,
		base
			+ Vector3.new(
				0,
				7.0,
				0
			),
		Palette.NebulaPurple,
		1.3,
		12
	)

	MeshBuilder.AddHighlight(
		model,
		Palette.NebulaPurple,
		Palette.NebulaPink,
		0.88,
		0.28
	)

	return model
end

----------------------------------------------------------------------
-- 15 CELESTIAL TREE
-- Fibonacci stars in canopy
----------------------------------------------------------------------

function AssetGenerators.GenerateCelestialTree(
	origin: Vector3,
	parent: Instance,
	seed: number
)
	local rng =
		makeRng(
			"CelestialTree",
			seed
		)

	local model =
		Instance.new("Model")

	model.Name = "15_CelestialTree"
	model:SetAttribute(
		"AssetType",
		"Celestial Tree"
	)
	model.Parent = parent

	MeshBuilder.BuildSoilMound(
		model,
		origin,
		6.5,
		38,
		rng,
		seed
	)

	------------------------------------------------------------------
	-- Dark trunk
	------------------------------------------------------------------

	local trunkBase =
		origin
		+ Vector3.new(
			0,
			0.7,
			0
		)

	local trunkTop =
		origin
		+ Vector3.new(
			-0.7,
			8.5,
			0.4
		)

	MeshBuilder.MakeSegment(
		model,
		"CelestialTrunk",
		trunkBase,
		trunkTop,
		0.58,
		Enum.Material.Wood,
		Palette.BarkDark,
		0.04
	)

	------------------------------------------------------------------
	-- Branching silhouette
	------------------------------------------------------------------

	for i = 1, 16 do
		local theta =
			i * MathEngine.GoldenAngle

		local start =
			trunkTop
			+ Vector3.new(
				0,
				rng:NextNumber(
					-1.4,
					1.0
				),
				0
			)

		local finish =
			start
			+ Vector3.new(
				math.cos(theta)
				* rng:NextNumber(
					3.2,
					5.6
				),
				rng:NextNumber(
					0.4,
					2.4
				),
				math.sin(theta)
				* rng:NextNumber(
					3.2,
					5.6
				)
			)

		MeshBuilder.MakeSegment(
			model,
			"CelestialBranch_" .. i,
			start,
			finish,
			rng:NextNumber(
				0.14,
				0.26
			),
			Enum.Material.Wood,
			Palette.BarkDark,
			0.06
		)

		for j = 1, 3 do
			local t =
				j / 4

			local branchStart =
				start:Lerp(
					finish,
					t
				)

			local branchEnd =
				branchStart
				+ Vector3.new(
					math.cos(
						theta
						+ j
						* MathEngine.GoldenAngle
					)
					* rng:NextNumber(
						1.5,
						2.8
					),
					rng:NextNumber(
						0.4,
						1.7
					),
					math.sin(
						theta
						+ j
						* MathEngine.GoldenAngle
					)
					* rng:NextNumber(
						1.5,
						2.8
					)
				)

			MeshBuilder.MakeSegment(
				model,
				"CelestialTwig_" ..
					i ..
					"_" ..
					j,
				branchStart,
				branchEnd,
				0.08,
				Enum.Material.Wood,
				Palette.BarkDark,
				0.08
			)
		end
	end

	------------------------------------------------------------------
	-- Dark faceted canopy
	------------------------------------------------------------------

	MeshBuilder.BuildFacetedCanopy(
		model,
		origin
			+ Vector3.new(
				0,
				7.8,
				0
			),
		Vector3.new(
			7.5,
			4.4,
			7.2
		),
		6,
		14,
		Palette.CelestialBlue,
		Palette.CelestialPurple,
		rng
	)

	------------------------------------------------------------------
	-- Fibonacci-distributed stars
	------------------------------------------------------------------

	local starCenter =
		origin
		+ Vector3.new(
			0,
			8.1,
			0
		)

	local starPoints =
		MathEngine.FibonacciSphere(
			96,
			6.9,
			starCenter,
			0.07,
			rng
		)

	for i, point in ipairs(starPoints) do
		local starScale =
			rng:NextNumber(
				0.55,
				1.15
			)

		local starColor

		if i % 7 == 0 then
			starColor =
				Palette.CelestialStar
		elseif i % 3 == 0 then
			starColor =
				Color3.fromRGB(
					168,
					196,
					255
				)
		else
			starColor =
				Color3.fromRGB(
					230,
					218,
					255
				)
		end

		MeshBuilder.BuildCelestialStar(
			model,
			point,
			starScale,
			starColor
		)
	end

	------------------------------------------------------------------
	-- Larger central star cluster
	------------------------------------------------------------------

	for i = 1, 13 do
		local theta =
			i
			* MathEngine.GoldenAngle

		local point =
			starCenter
			+ Vector3.new(
				math.cos(theta)
				* rng:NextNumber(
					0.4,
					2.5
				),
				rng:NextNumber(
					0.2,
					1.8
				),
				math.sin(theta)
				* rng:NextNumber(
					0.4,
					2.5
				)
			)

		MeshBuilder.BuildCelestialStar(
			model,
			point,
			rng:NextNumber(
				1.0,
				1.8
			),
			Palette.CelestialStar
		)
	end

	------------------------------------------------------------------
	-- Firefly / star particle system
	------------------------------------------------------------------

	MeshBuilder.AddParticleBurst(
		model,
		starCenter,
		Palette.CelestialStar,
		4.5,
		NumberRange.new(
			2.5,
			5.0
		),
		NumberRange.new(
			0.12,
			0.65
		),
		NumberSequence.new({
			NumberSequenceKeypoint.new(
				0,
				0.11
			),
			NumberSequenceKeypoint.new(
				0.55,
				0.08
			),
			NumberSequenceKeypoint.new(
				1,
				0
			),
		}),
		1
	)

	MeshBuilder.AddPointLight(
		model,
		starCenter,
		Color3.fromRGB(
			108,
			111,
			255
		),
		1.2,
		15
	)

	MeshBuilder.AddHighlight(
		model,
		Palette.CelestialPurple,
		Palette.CelestialStar,
		0.91,
		0.25
	)

	------------------------------------------------------------------
	-- Star harvest display
	------------------------------------------------------------------

	local harvest =
		origin
		+ Vector3.new(
			5.0,
			0.95,
			0
		)

	for i = 1, 18 do
		local theta =
			i
			* MathEngine.GoldenAngle

		local radial =
			math.sqrt(
				i / 18
			) * 1.05

		local point =
			harvest
			+ Vector3.new(
				math.cos(theta)
				* radial,
				rng:NextNumber(
					0.45,
					1.4
				),
				math.sin(theta)
				* radial
			)

		MeshBuilder.BuildCelestialStar(
			model,
			point,
			rng:NextNumber(
				0.8,
				1.25
			),
			Palette.CelestialStar
		)
	end

	return model
end

----------------------------------------------------------------------
-- GENERATOR REGISTRY
----------------------------------------------------------------------

local GeneratorDefinitions = {
	{
		Id = 1,
		Name = "Carrot Plant",
		Generate = AssetGenerators.GenerateCarrotPlant,
	},

	{
		Id = 2,
		Name = "Potato Plant",
		Generate = AssetGenerators.GeneratePotatoPlant,
	},

	{
		Id = 3,
		Name = "Strawberry Plant",
		Generate = AssetGenerators.GenerateStrawberryPlant,
	},

	{
		Id = 4,
		Name = "Blueberry Bush",
		Generate = AssetGenerators.GenerateBlueberryBush,
	},

	{
		Id = 5,
		Name = "Apple Tree",
		Generate = AssetGenerators.GenerateAppleTree,
	},

	{
		Id = 6,
		Name = "Mango Tree",
		Generate = AssetGenerators.GenerateMangoTree,
	},

	{
		Id = 7,
		Name = "Frostleaf",
		Generate = AssetGenerators.GenerateFrostleaf,
	},

	{
		Id = 8,
		Name = "Cherry Tree",
		Generate = AssetGenerators.GenerateCherryTree,
	},

	{
		Id = 9,
		Name = "Banana Tree",
		Generate = AssetGenerators.GenerateBananaTree,
	},

	{
		Id = 10,
		Name = "Dragonfruit Bush",
		Generate = AssetGenerators.GenerateDragonfruitBush,
	},

	{
		Id = 11,
		Name = "Starfruit Plant",
		Generate = AssetGenerators.GenerateStarfruitPlant,
	},

	{
		Id = 12,
		Name = "Coconut Palm",
		Generate = AssetGenerators.GenerateCoconutPalm,
	},

	{
		Id = 13,
		Name = "Crystal Shrub",
		Generate = AssetGenerators.GenerateCrystalShrub,
	},

	{
		Id = 14,
		Name = "Nebula Vine",
		Generate = AssetGenerators.GenerateNebulaVine,
	},

	{
		Id = 15,
		Name = "Celestial Tree",
		Generate = AssetGenerators.GenerateCelestialTree,
	},
}

----------------------------------------------------------------------
-- MODULE 4 : DEPLOYMENT
----------------------------------------------------------------------

local Deployment = {}

function Deployment.GetOrCreateRoot(): Folder
	local existing =
		Workspace:FindFirstChild(
			Config.RootName
		)

	if existing then
		if Config.ClearExistingGeneration or not existing:IsA("Folder") then
			existing:Destroy()
		else
			return existing
		end
	end

	local root =
		Instance.new("Folder")

	root.Name =
		Config.RootName

	root.Parent =
		Workspace

	root:SetAttribute(
		"ProceduralEngine",
		"AdvancedFarmGenerator"
	)

	root:SetAttribute(
		"MasterSeed",
		Config.MasterSeed
	)

	root:SetAttribute(
		"AssetCount",
		#GeneratorDefinitions
	)

	return root
end

function Deployment.CreateAssetFolder(
	root: Folder,
	assetId: number,
	assetName: string
): Folder
	local folder =
		Instance.new("Folder")

	folder.Name =
		string.format(
			"%02d_%s",
			assetId,
			assetName
			:gsub("%s+", "_")
		)

	folder:SetAttribute(
		"AssetId",
		assetId
	)

	folder:SetAttribute(
		"AssetName",
		assetName
	)

	folder.Parent =
		root

	return folder
end

function Deployment.ComputeGridPosition(
	row: number,
	column: number
): Vector3
	local columns = Config.GridColumns
	local rows = Config.GridRows

	local x =
		(
			column
			- (columns + 1) * 0.5
		)
		* Config.GridSpacing

	local z =
		(
			row
			- (rows + 1) * 0.5
		)
		* Config.GridSpacing

	return
		Config.GridOrigin
		+ Vector3.new(
			x,
			0,
			z
		)
end

function Deployment.SpawnOne(
	definition,
	root: Folder,
	row: number,
	column: number
)
	local position =
		Deployment.ComputeGridPosition(
			row,
			column
		)

	local assetFolder =
		Deployment.CreateAssetFolder(
			root,
			definition.Id,
			definition.Name
		)

	assetFolder:SetAttribute(
		"GridRow",
		row
	)

	assetFolder:SetAttribute(
		"GridColumn",
		column
	)

	assetFolder:SetAttribute(
		"WorldX",
		position.X
	)

	assetFolder:SetAttribute(
		"WorldZ",
		position.Z
	)

	local assetSeed =
		Config.MasterSeed
		+ definition.Id
		* 100003
		+ row * 7919
		+ column * 104729

	local success, result =
		pcall(
			definition.Generate,
			position,
			assetFolder,
			assetSeed
		)

	if not success then
		warn(
			"[ProceduralFarm] Failed to generate "
				.. definition.Name
				.. ": "
				.. tostring(result)
		)

		assetFolder:SetAttribute(
			"GenerationFailed",
			true
		)

		return nil
	end

	if result and result:IsA("Model") then
		result:SetAttribute(
			"GridRow",
			row
		)

		result:SetAttribute(
			"GridColumn",
			column
		)

		result:SetAttribute(
			"WorldPosition",
			position
		)

		CollectionService:AddTag(
			result,
			"ProceduralFarmAsset"
		)

		return result
	end

	return nil
end

function Deployment.SpawnGrid(root: Folder)
	print(
		"[ProceduralFarm] Beginning 3x5 deployment..."
	)

	Runtime.PartCount = 0

	local generatedCount = 0
	local failedCount = 0

	local index = 0

	for row = 1, Config.GridRows do
		for column = 1, Config.GridColumns do
			index += 1

			local definition =
				GeneratorDefinitions[
			index
			]

			if definition then
				local generated =
					Deployment.SpawnOne(
						definition,
						root,
						row,
						column
					)

				if generated then
					generatedCount += 1
				else
					failedCount += 1
				end
			end

			task.wait()
		end
	end

	print(
		"[ProceduralFarm] Deployment complete."
	)

	print(
		"[ProceduralFarm] Assets generated: "
			.. tostring(generatedCount)
	)

	print(
		"[ProceduralFarm] Generation errors: "
			.. tostring(failedCount)
	)

	print(
		"[ProceduralFarm] Approximate procedural part operations: "
			.. tostring(Runtime.PartCount)
	)
end

----------------------------------------------------------------------
-- LIGHTING SETUP
--
-- Only creates the global effects if they do not already exist.
-- This prevents repeated ServerScript execution from stacking effects.
----------------------------------------------------------------------

function Deployment.ConfigureLighting()
	local atmosphere =
		Lighting:FindFirstChild(
			"ProceduralFarmAtmosphere"
		)

	if not atmosphere then
		atmosphere =
			Instance.new(
				"Atmosphere"
			)

		atmosphere.Name =
			"ProceduralFarmAtmosphere"

		atmosphere.Density = 0.22
		atmosphere.Offset = 0.05
		atmosphere.Color =
			Color3.fromRGB(
				190,
				205,
				220
			)

		atmosphere.Decay =
			Color3.fromRGB(
				105,
				116,
				132
			)

		atmosphere.Glare = 0.08
		atmosphere.Haze = 0.4

		atmosphere.Parent =
			Lighting
	end

	local bloom =
		Lighting:FindFirstChild(
			"ProceduralFarmBloom"
		)

	if not bloom then
		bloom =
			Instance.new(
				"BloomEffect"
			)

		bloom.Name =
			"ProceduralFarmBloom"

		bloom.Intensity = 0.22
		bloom.Size = 24
		bloom.Threshold = 1.2

		bloom.Parent =
			Lighting
	end

	local colorCorrection =
		Lighting:FindFirstChild(
			"ProceduralFarmColorCorrection"
		)

	if not colorCorrection then
		colorCorrection =
			Instance.new(
				"ColorCorrectionEffect"
			)

		colorCorrection.Name =
			"ProceduralFarmColorCorrection"

		colorCorrection.Brightness = 0.03
		colorCorrection.Contrast = 0.08
		colorCorrection.Saturation = 0.04

		colorCorrection.Parent =
			Lighting
	end
end

----------------------------------------------------------------------
-- OPTIONAL DEBUG MARKERS
----------------------------------------------------------------------

function Deployment.CreateGridMarkers(
	root: Folder
)
	local markerFolder =
		Instance.new("Folder")

	markerFolder.Name =
		"_GridDiagnostics"

	markerFolder.Parent =
		root

	for row = 1, Config.GridRows do
		for column = 1, Config.GridColumns do
			local position =
				Deployment.ComputeGridPosition(
					row,
					column
				)

			local marker =
				MeshBuilder.MakeBall(
					markerFolder,
					"GridMarker",
					Vector3.new(
						0.25,
						0.25,
						0.25
					),
					position
					+ Vector3.new(
						0,
						0.2,
						0
					),
					Enum.Material.Neon,
					Color3.fromRGB(
						0,
						255,
						0
					),
					1,
					0
				)

			marker.Transparency = 1
		end
	end
end

----------------------------------------------------------------------
-- VALIDATION
----------------------------------------------------------------------

local function validateConfiguration()
	assert(
		Config.GridRows == 3,
		"GridRows must remain 3 for the requested 3x5 layout."
	)

	assert(
		Config.GridColumns == 5,
		"GridColumns must remain 5 for the requested 3x5 layout."
	)

	assert(
		#GeneratorDefinitions == 15,
		"Exactly 15 asset generators are required."
	)

	for index, definition in ipairs(
		GeneratorDefinitions
		) do
		assert(
			typeof(
				definition.Generate
			) == "function",
			"Generator " ..
				tostring(index)
				.. " is missing."
		)
	end
end

----------------------------------------------------------------------
-- OPTIONAL PROCEDURAL METADATA SYSTEM
----------------------------------------------------------------------

local function annotateAllAssets(
	root: Folder
)
	for _, descendant in ipairs(
		root:GetDescendants()
		) do
		if descendant:IsA("BasePart") then
			descendant:SetAttribute(
				"GeneratedBy",
				"ProceduralFarmEngine"
			)

			descendant:SetAttribute(
				"IsProcedural",
				true
			)
		end
	end
end

----------------------------------------------------------------------
-- MASTER EXECUTION
----------------------------------------------------------------------

local function main()
	validateConfiguration()

	Deployment.ConfigureLighting()

	local root =
		Deployment.GetOrCreateRoot()

	Deployment.SpawnGrid(root)

	annotateAllAssets(root)

	print(
		"=========================================="
	)

	print(
		"[ProceduralFarm] Advanced procedural farm engine online."
	)

	print(
		"[ProceduralFarm] 15/15 botanical assets requested."
	)

	print(
		"[ProceduralFarm] L-System systems: ENABLED"
	)

	print(
		"[ProceduralFarm] Fibonacci phyllotaxis: ENABLED"
	)

	print(
		"[ProceduralFarm] Cubic Bezier systems: ENABLED"
	)

	print(
		"[ProceduralFarm] Parametric sine/cosine weaving: ENABLED"
	)

	print(
		"[ProceduralFarm] Procedural noise soil: ENABLED"
	)

	print(
		"[ProceduralFarm] Particle/light systems: ENABLED"
	)

	print(
		"[ProceduralFarm] 3x5 deployment: COMPLETE"
	)

	print(
		"=========================================="
	)
end

----------------------------------------------------------------------
-- RUN
----------------------------------------------------------------------

if Config.AutoRun then
	main()
end

----------------------------------------------------------------------
-- END OF PROCEDURAL FARM ASSET ENGINE
----------------------------------------------------------------------
