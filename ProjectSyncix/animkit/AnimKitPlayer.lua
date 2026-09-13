--!strict
-- AnimKitPlayer: plays animkit's Lua exports on any Motor6D rig (R15, R6, custom) by
-- writing Motor6D.Transform every frame. Nothing has to be uploaded, so a change to an
-- animation can be tried with Play straight away.
--
-- Use it for code-driven animation (NPCs, creatures, quick tests). For the player's own
-- character in a finished game, prefer uploaded animations played through the Animator:
-- they blend with walk/idle and replicate by themselves.
--
-- It writes after Roblox's Animator each frame (RunService.Stepped), so on the joints it
-- drives it wins over any Animator track; joints it does not drive are left alone.
--
--   local AnimKitPlayer = require(path.to.AnimKitPlayer)
--   local track = AnimKitPlayer.new(character, require(path.to.WaveData))
--   track.MarkerReached = function(name, value) print(name, value) end
--   track:Play()            -- track:Play(0.5) plays at half speed
--   track:Stop()

local RunService = game:GetService("RunService")

local Player = {}
Player.__index = Player

local function ease(style: string, direction: string, t: number): number
	if style == "Constant" then
		return if t < 1 then 0 else 1
	elseif style == "Linear" then
		return t
	elseif style == "Bounce" or style == "Elastic" then
		return 1 - (1 - t) ^ 3
	elseif direction == "In" then
		return t ^ 3
	elseif direction == "Out" then
		return 1 - (1 - t) ^ 3
	end
	return if t < 0.5 then 4 * t ^ 3 else 1 - (-2 * t + 2) ^ 3 / 2
end

-- A pose's easing leads to the next pose, as in Roblox's KeyframeSequences.
local function sample(track, t: number): CFrame
	local first = track[1]
	if t <= first.Time then
		return first.CFrame
	end
	for i = 1, #track - 1 do
		local a, b = track[i], track[i + 1]
		if t <= b.Time then
			local span = b.Time - a.Time
			local alpha = if span > 0 then (t - a.Time) / span else 1
			return a.CFrame:Lerp(b.CFrame, ease(a.Easing, a.Direction, alpha))
		end
	end
	return track[#track].CFrame
end

function Player.new(rig: Instance, data)
	local self = setmetatable({}, Player)
	self.Data = data
	-- Motor6D joints, or AnimationConstraints on Rig Builder's newer R15 (it has no
	-- Motor6D); both are posed through their Transform. A Motor6D wins if a rig has both.
	self.Motors = {} :: { [string]: any }
	for _, d in ipairs(rig:GetDescendants()) do
		if d:IsA("Motor6D") or (d:IsA("AnimationConstraint") and not self.Motors[d.Name]) then
			self.Motors[d.Name] = d
		end
	end
	for joint in pairs(data.Tracks) do
		if not self.Motors[joint] then
			warn(string.format("[AnimKit] %s has no Motor6D named %s; that track is skipped.", rig:GetFullName(), joint))
		end
	end
	self.Time = 0
	self.Speed = 1
	self.Playing = false
	self.MarkerReached = nil :: ((string, string) -> ())?
	self.Stopped = nil :: (() -> ())?
	return self
end

function Player:_apply()
	for joint, track in pairs(self.Data.Tracks) do
		local motor = self.Motors[joint]
		if motor then
			motor.Transform = sample(track, self.Time)
		end
	end
end

function Player:_markers(from: number, to: number)
	if not self.MarkerReached then
		return
	end
	for _, m in ipairs(self.Data.Markers) do
		if m.Time > from and m.Time <= to then
			task.spawn(self.MarkerReached, m.Name, m.Value)
		end
	end
end

function Player:Play(speed: number?)
	self.Speed = speed or 1
	if self.Playing then
		return
	end
	self.Playing = true
	self:_markers(-1, 0)
	self.Connection = RunService.Stepped:Connect(function(_, dt)
		local length = self.Data.Length
		local before = self.Time
		self.Time += dt * self.Speed
		if self.Time >= length then
			if self.Data.Loop then
				self:_markers(before, length)
				self.Time -= length
				before = -1
			else
				self.Time = length
				self:_markers(before, length)
				self:_apply()
				self:Stop(true)
				return
			end
		end
		self:_markers(before, self.Time)
		self:_apply()
	end)
end

-- keepPose: leave the joints in the last pose instead of returning them to rest.
function Player:Stop(keepPose: boolean?)
	if self.Connection then
		self.Connection:Disconnect()
		self.Connection = nil
	end
	self.Playing = false
	if not keepPose then
		for joint in pairs(self.Data.Tracks) do
			local motor = self.Motors[joint]
			if motor then
				motor.Transform = CFrame.identity
			end
		end
	end
	self.Time = 0
	if self.Stopped then
		task.spawn(self.Stopped)
	end
end

return Player
