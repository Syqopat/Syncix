local ReplicatedStorage = game:GetService("ReplicatedStorage")
local SoundService = game:GetService("SoundService")
local DialogModule = require(ReplicatedStorage:WaitForChild("DialogModule"))

local player = game.Players.LocalPlayer
local npc = script.Parent
local prompt = npc:WaitForChild("ProximityPrompt")

local tweenService = game:GetService("TweenService")
local userInputService = game:GetService("UserInputService")
local collectionService = game:GetService("CollectionService")
local soundsFolder = ReplicatedStorage:WaitForChild("DialogModule"):WaitForChild("sounds")
local SANS_SOUND = soundsFolder:FindFirstChild("sans dialogue sound effect") or soundsFolder:WaitForChild("tick")
SANS_SOUND.Volume = 0.35
local TICK_SOUND = SANS_SOUND
local END_TICK_SOUND = soundsFolder:WaitForChild("tick2")
local DIALOG_RESPONSES_UI = player:WaitForChild("PlayerGui"):WaitForChild("dialog"):WaitForChild("dialogResponses")

local lastSoundTime = 0
local function playDialogueSound()
	local now = os.clock()
	if now - lastSoundTime >= 0.055 then
		lastSoundTime = now
		SoundService:PlayLocalSound(SANS_SOUND)
	end
end

DialogModule.triggerDialog = function(self, playerRef, questionNumber)
	self:showGui()
	if #self.dialogs == 0 then return end
	local dialogNum = questionNumber or self.dialogOption
	local dialog = self.dialogs[dialogNum]
	tweenService:Create(game.Workspace.CurrentCamera, TweenInfo.new(0.4, Enum.EasingStyle.Quad, Enum.EasingDirection.Out), {FieldOfView = 65}):Play()
	task.spawn(function()
		self.talking = true
		local dialogObject = self.npcGui.dialog
		dialogObject.Visible = true
		dialogObject.Text = ""
		local currenttext = ""
		local skip = false
		local arrow = 0
		for i, letter in string.split(dialog.text, "") do
			currenttext = currenttext .. letter
			if letter == "<" then skip = true end
			if letter == ">" then skip = false arrow += 1 continue end
			if arrow == 2 then arrow = 0 end
			if skip then continue end
			dialogObject.Text = currenttext .. (if arrow == 1 then "</font>" else "")
			if letter ~= " " then
				playDialogueSound()
			end
			task.wait(0.012)
		end
		dialogObject.Text = dialog.text
		self.talking = false

		local keyboardInputs = {
			Enum.KeyCode.One, Enum.KeyCode.Two, Enum.KeyCode.Three,
			Enum.KeyCode.Four, Enum.KeyCode.Five, Enum.KeyCode.Six,
			Enum.KeyCode.Seven, Enum.KeyCode.Eight, Enum.KeyCode.Nine
		}
		local uiResponses = DIALOG_RESPONSES_UI
		local responseNum = nil
		for i, response in ipairs(dialog.responses) do
			local option = uiResponses[tostring(i)] or uiResponses[i]
			option.text.Text = "<font color='rgb(255,220,127)'>" .. i .. ".)</font> [''" .. response .. "'']"
			option.Size = UDim2.fromScale(option.Size.X.Scale, 0.4)
			option.text.Position = UDim2.new(0.02, 0, 0.5, 0)
			option.Visible = true
			tweenService:Create(option, TweenInfo.new(0.1, Enum.EasingStyle.Quad, Enum.EasingDirection.Out), {Size = UDim2.new(option.Size.X.Scale, 0, 0.35, 0)}):Play()

			local enterCon = option.MouseEnter:Connect(function()
				tweenService:Create(option, TweenInfo.new(0.2, Enum.EasingStyle.Quad, Enum.EasingDirection.Out), {Size = UDim2.new(option.Size.X.Scale + (option.Size.X.Scale * 0.05), 0, 0.4, 0)}):Play()
				tweenService:Create(option.text, TweenInfo.new(0.2, Enum.EasingStyle.Quad, Enum.EasingDirection.Out), {Position = UDim2.new(0.06, 0, 0.5, 0)}):Play()
				SoundService:PlayLocalSound(END_TICK_SOUND)
			end)
			local leaveCon = option.MouseLeave:Connect(function()
				tweenService:Create(option, TweenInfo.new(0.2, Enum.EasingStyle.Quad, Enum.EasingDirection.Out), {Size = UDim2.new(option.Size.X.Scale, 0, 0.35, 0)}):Play()
				tweenService:Create(option.text, TweenInfo.new(0.2, Enum.EasingStyle.Quad, Enum.EasingDirection.Out), {Position = UDim2.new(0.02, 0, 0.5, 0)}):Play()
			end)
			local chooseCon = option.MouseButton1Down:Connect(function()
				if not self.active then return end
				self.active = false
				responseNum = i
				self.fireResponded:Fire(i, dialogNum)
				SoundService:PlayLocalSound(TICK_SOUND)
			end)
			local numberpressCon = userInputService.InputBegan:Connect(function(input, gameprocessed)
				if gameprocessed then return end
				if input.UserInputType == Enum.UserInputType.Keyboard then
					local numberinput = table.find(keyboardInputs, input.KeyCode)
					if numberinput ~= nil and numberinput == i then
						if not self.active then return end
						self.active = false
						responseNum = i
						self.fireResponded:Fire(i, dialogNum)
						SoundService:PlayLocalSound(TICK_SOUND)
					end
				end
			end)
			coroutine.wrap(function()
				repeat task.wait() until responseNum ~= nil
				enterCon:Disconnect()
				leaveCon:Disconnect()
				chooseCon:Disconnect()
				numberpressCon:Disconnect()
				option.Visible = false
			end)()
			SoundService:PlayLocalSound(END_TICK_SOUND)
			task.wait(0.05)
		end
		self.active = true
		local range = 10
		while self.active do
			local charRoot = playerRef.Character and (playerRef.Character.PrimaryPart or playerRef.Character:FindFirstChild("HumanoidRootPart") or playerRef.Character:FindFirstChild("Torso"))
			local npcRoot = self.npc:FindFirstChild("HumanoidRootPart") or self.npc:FindFirstChild("UpperTorso") or self.npc:FindFirstChild("Torso") or self.npc.PrimaryPart or self.npc:FindFirstChild("Head")
			if charRoot and npcRoot then
				local distance = (charRoot.Position - npcRoot.Position).Magnitude
				if distance > range then
					self:hideGui()
					responseNum = 0
					break
				end
			end
			task.wait()
		end
	end)
end

DialogModule.hideGui = function(self, exitQuip, notActuallyAnExitQuip)
	self.active = false
	self.talking = true
	notActuallyAnExitQuip = notActuallyAnExitQuip or false
	for _, p in collectionService:GetTagged("NPCprompt") do
		if p:IsA("ProximityPrompt") then
			p.Enabled = not notActuallyAnExitQuip
		end
	end
	self.talking = false
	if notActuallyAnExitQuip then
		tweenService:Create(game.Workspace.CurrentCamera, TweenInfo.new(0.4, Enum.EasingStyle.Quad, Enum.EasingDirection.Out), {FieldOfView = 65}):Play()
	else
		tweenService:Create(game.Workspace.CurrentCamera, TweenInfo.new(0.4, Enum.EasingStyle.Quad, Enum.EasingDirection.Out), {FieldOfView = 70}):Play()
	end
	for _, option in DIALOG_RESPONSES_UI:GetChildren() do
		if option:IsA("GuiButton") then
			option.Visible = false
		end
	end
	local dialogObject = self.npcGui.dialog
	if exitQuip then
		dialogObject.TextTransparency = 0
		dialogObject.UIStroke.Transparency = 0
		self.npcGui.name.TextTransparency = 1
		self.npcGui.name.UIStroke.Transparency = 1
		self.npcGui.arrow.TextTransparency = 1
		self.npcGui.arrow.UIStroke.Transparency = 1
		local currenttext = ""
		dialogObject.Text = ""
		dialogObject.Visible = true
		local skip = false
		local arrow = 0
		for i, letter in string.split(exitQuip, "") do
			if dialogObject.Text ~= currenttext and skip == 0 then break end
			currenttext = currenttext .. letter
			if letter == "<" then skip = true end
			if letter == ">" then skip = false arrow += 1 continue end
			if arrow == 2 then arrow = 0 end
			if skip then continue end
			dialogObject.Text = currenttext .. (if arrow == 1 then "</font>" else "")
			if letter ~= " " then
				playDialogueSound()
			end
			task.wait(0.012)
		end
		dialogObject.Text = exitQuip
		if notActuallyAnExitQuip then return end
	end
	task.spawn(function()
		if exitQuip then
			task.wait(1.0)
			if dialogObject.Text ~= exitQuip then return end
		end
		if self.npcGui.name.TextTransparency ~= 1 then
			self.animNameText:Cancel()
			self.animNameStroke:Cancel()
			self.animArrowText:Cancel()
			self.animArrowStroke:Cancel()
		end
		self.npcGui.name.TextTransparency = 0
		self.npcGui.name.UIStroke.Transparency = 0
		self.npcGui.arrow.TextTransparency = 0
		self.npcGui.arrow.UIStroke.Transparency = 0
		self.npcGui.name.Visible = true
		self.npcGui.arrow.Visible = true
		self.animDialogText:Play()
		self.animDialogStroke:Play()
		for _, p in collectionService:GetTagged("NPCprompt") do
			if p:IsA("ProximityPrompt") then
				p.Enabled = true
			end
		end
	end)
end

local head = npc:WaitForChild("Head")
if not head:FindFirstChild("gui") then
	local templateGui = ReplicatedStorage:WaitForChild("DialogModule"):WaitForChild("gui")
	local newGui = templateGui:Clone()
	if newGui:FindFirstChild("name") then
		newGui.name.Text = "Seed Merchant"
	end
	newGui.Parent = head
end

if not npc:FindFirstChild("Torso") then
	local root = npc:FindFirstChild("HumanoidRootPart") or npc:FindFirstChild("UpperTorso") or head
	local torsoPart = Instance.new("Part")
	torsoPart.Name = "Torso"
	torsoPart.Transparency = 1
	torsoPart.CanCollide = false
	torsoPart.CanTouch = false
	torsoPart.CanQuery = false
	torsoPart.Massless = true
	torsoPart.Size = Vector3.new(1, 1, 1)
	torsoPart.CFrame = root.CFrame
	local weld = Instance.new("WeldConstraint")
	weld.Part0 = torsoPart
	weld.Part1 = root
	weld.Parent = torsoPart
	torsoPart.Parent = npc
end

local dialogObject = DialogModule.new("Seed Merchant", npc, prompt)

dialogObject:addDialog(
	"Welcome farmer! I have fresh seeds for your garden. What would you like to know?",
	{
		"Which seed makes the most profit?",
		"How do I plant seeds?",
		"Just browsing for now."
	}
)

prompt.Triggered:Connect(function(triggerPlayer)
	if triggerPlayer == player then
		dialogObject:triggerDialog(player, 1)
	end
end)

dialogObject.responded:Connect(function(responseNum, dialogNum)
	if dialogNum == 1 then
		if responseNum == 1 then
			dialogObject:hideGui("Carrots grow very fast, but golden wheat brings much more profit!")
		elseif responseNum == 2 then
			dialogObject:hideGui("Just hold the seed in your hand and click on an empty farmland patch!")
		elseif responseNum == 3 then
			dialogObject:hideGui("Alright, good luck with your farm. Come back anytime!")
		end
	end
end)
