-- DialogModule.lua
local DialogModule = {}
DialogModule.__index = DialogModule

local tweenService = game:GetService("TweenService")
local runService = game:GetService('RunService')
local userInputService = game:GetService('UserInputService')
local collectionService = game:GetService("CollectionService")

local TICK_SOUND = script.sounds.tick
local END_TICK_SOUND = script.sounds.tick2
local DIALOG_RESPONSES_UI = game.Players.LocalPlayer:WaitForChild("PlayerGui"):WaitForChild("dialog"):WaitForChild("dialogResponses")

-- Constructor
function DialogModule.new(npcName, npc, prompt, animation)
	local self = setmetatable({}, DialogModule)
	self.npcName = npcName
	self.npc = npc
	self.dialogs = {} -- Array to store dialog options
	self.responses = {} -- Array to store response options
	self.dialogOption = 1
	self.npcGui = self.npc:WaitForChild("Head"):WaitForChild("gui")
	self.active = false
	self.talking = false
	self.prompt = prompt
	
	local template = DIALOG_RESPONSES_UI:FindFirstChild("template")
	if template then
		for i = 1,9 do
			local newResponseButton = template:Clone()
			newResponseButton.Parent = DIALOG_RESPONSES_UI
			newResponseButton.Name = i
		end
		template:Destroy()
	end
	
	local eventSignal = Instance.new("BindableEvent")
	self.responded = eventSignal.Event -- Expose the event to connect to
	self.fireResponded = eventSignal -- Keep a reference to the BindableEvent
		
	-- tween variables
	self.animNameText = tweenService:Create(self.npcGui.name, TweenInfo.new(.3),{TextTransparency = 1})
	self.animNameStroke = tweenService:Create(self.npcGui.name.UIStroke, TweenInfo.new(.3),{Transparency = 1})
	self.animArrowText = tweenService:Create(self.npcGui.arrow, TweenInfo.new(.3),{TextTransparency = 1})
	self.animArrowStroke = tweenService:Create(self.npcGui.arrow.UIStroke, TweenInfo.new(.3),{Transparency = 1})
	self.animDialogText = tweenService:Create(self.npcGui.dialog, TweenInfo.new(.3),{TextTransparency = 1})
	self.animDialogStroke = tweenService:Create(self.npcGui.dialog.UIStroke, TweenInfo.new(.3),{Transparency = 1})
	
	-- animate
	if animation ~= nil then
		local newAnimation = Instance.new("Animation")
		newAnimation.AnimationId = animation
		local newAnimLoaded = npc:WaitForChild("Humanoid"):LoadAnimation(newAnimation)
		newAnimLoaded:Play()
	end
	
	-- Connections
	local frameCount = 0
	local heartbeatConnection = runService.Heartbeat:Connect(function()
		frameCount += 1
		if self.talking then
			self.npcGui.StudsOffset = Vector3.new(0,1.6,0)
		else
			self.npcGui.StudsOffset = Vector3.new(0,math.sin(frameCount/25)/6 + 1.55,0)
		end
	end)
	local shownConnection = prompt.PromptShown:Connect(function()
		self.npcGui.AlwaysOnTop = true
	end)
	local hiddenConnection = prompt.PromptHidden:Connect(function()
		if self.talking then return end
		self.npcGui.AlwaysOnTop = false
	end)
	self.connections = {heartbeatConnection}--,shownConnection,hiddenConnection}
	
	return self
end

-- Add dialog to the NPC
function DialogModule:addDialog(dialogText, responseOptions)
	table.insert(self.dialogs, {text = dialogText, responses = responseOptions})
end

-- Sort dialogs alphabetically or by custom function
function DialogModule:sortDialogs(sortFunc)
	table.sort(self.dialogs, sortFunc or function(a, b) return a.text < b.text end)
end

-- Display the dialog when proximity prompt is triggered
function DialogModule:triggerDialog(player, questionNumber)
	self:showGui()
	
	if #self.dialogs == 0 then
		warn("No dialogs available for NPC: " .. self.npcName)
		return
	end

	local dialogNum = questionNumber or self.dialogOption
	local dialog = self.dialogs[dialogNum] -- Show the first dialog (can be updated for other logic)
	
	tweenService:Create(game.Workspace.CurrentCamera, TweenInfo.new(0.5, Enum.EasingStyle.Quad, Enum.EasingDirection.Out), {FieldOfView = 65}):Play()
	
	task.spawn(function()
		self.talking = true
		local dialogObject = self.npcGui.dialog
		dialogObject.Visible = true
		dialogObject.Text = ""
		local currenttext = ""
		local skip = false
		local arrow = 0
		for i, letter in string.split(dialog.text,"") do
			currenttext = currenttext .. letter
			if letter == "<" then skip = true end
			if letter == ">" then skip = false arrow += 1 continue end
			if arrow == 2 then arrow = 0 end
			if skip then continue end
			dialogObject.Text = currenttext .. if arrow == 1 then "</font>" else ""
			TICK_SOUND:Play()
			task.wait(0.02)
		end
		dialogObject.Text = dialog.text
		self.talking = false

		-- inputs
		local keyboardInputs = {
			Enum.KeyCode.One,
			Enum.KeyCode.Two,
			Enum.KeyCode.Three,
			Enum.KeyCode.Four,
			Enum.KeyCode.Five,
			Enum.KeyCode.Six,
			Enum.KeyCode.Seven,
			Enum.KeyCode.Eight,
			Enum.KeyCode.Nine,
		}

		-- Show responses
		local uiResponses = DIALOG_RESPONSES_UI
		local responseNum = nil
		for i, response in ipairs(dialog.responses) do
			local option = uiResponses[i] 
			option.text.Text = "<font color='rgb(255,220,127)'>" .. i .. ".)</font> [''" .. response .. "'']"
			
			-- calculate x size
			local plaintext = i..".) [''"..response:gsub("%b<>", "").."'']"
			
			option.Size = UDim2.fromScale(option.Size.X.Scale,.4)
			
			option.text.Position = UDim2.new(0.02,0,0.5,0)
			option.Visible = true
			tweenService:Create(option,TweenInfo.new(0.1,Enum.EasingStyle.Quad,Enum.EasingDirection.Out),{Size = UDim2.new(option.Size.X.Scale,0,0.35,0)}):Play()

			local enterCon = option.MouseEnter:Connect(function()
				tweenService:Create(option,TweenInfo.new(0.3,Enum.EasingStyle.Quad,Enum.EasingDirection.Out),{Size = UDim2.new(option.Size.X.Scale + (option.Size.X.Scale * .05), 0,0.4,0)}):Play()
				tweenService:Create(option.text,TweenInfo.new(0.3,Enum.EasingStyle.Quad,Enum.EasingDirection.Out),{Position = UDim2.new(0.06,0,0.5,0)}):Play()
				END_TICK_SOUND:Play()
			end)

			local leaveCon = option.MouseLeave:Connect(function()
				tweenService:Create(option,TweenInfo.new(0.3,Enum.EasingStyle.Quad,Enum.EasingDirection.Out),{Size = UDim2.new(option.Size.X.Scale, 0,0.35,0)}):Play()
				tweenService:Create(option.text,TweenInfo.new(0.3,Enum.EasingStyle.Quad,Enum.EasingDirection.Out),{Position = UDim2.new(0.02,0,0.5,0)}):Play()
			end)

			local chooseCon = option.MouseButton1Down:Connect(function() -- Return response
				if not self.active then return end
				self.active = false
				responseNum = i
				self.fireResponded:Fire(i, dialogNum)
				TICK_SOUND:Play()
			end)
			
			local numberpressCon = userInputService.InputBegan:Connect(function(input, gameprocessed)
				if gameprocessed then return end
				if input.UserInputType == Enum.UserInputType.Keyboard then
					local numberinput = table.find(keyboardInputs, input.KeyCode)
					if (numberinput ~= nil and numberinput == i) then
						if not self.active then return end
						self.active = false
						responseNum = i
						self.fireResponded:Fire(i, dialogNum)
						TICK_SOUND:Play()
					end
				end
			end)

			coroutine.wrap(function()
				-- unconnectAllConnections
				repeat task.wait() until responseNum ~= nil
				enterCon:Disconnect()
				leaveCon:Disconnect()
				chooseCon:Disconnect()
				numberpressCon:Disconnect()
				option.Visible = false
			end)()

			END_TICK_SOUND:Play()

			task.wait(0.2)
		end

		self.active = true

		local range = 10
		while self.active do
			local distance = (player.Character.PrimaryPart.Position - self.npc.Torso.Position).Magnitude
			if distance > range then
				self:hideGui()
				responseNum = 0
				break
			end
			task.wait()
		end
	end)
end

function DialogModule:showGui()
	turnProximityPromptsOn(false)
	--self.npcGui.AlwaysOnTop = true
	
	self.animNameText:Play()
	self.animNameStroke:Play()
	self.animArrowText:Play()
	self.animArrowStroke:Play()

	self.animDialogText:Cancel()
	self.animDialogStroke:Cancel()
	
	self.npcGui.dialog.TextTransparency = 0
	self.npcGui.dialog.UIStroke.Transparency = 0
	
	coroutine.wrap(function()
		task.wait(0.3)
		
		if self.npcGui.name.TextTransparency ~= 1 then return end -- check if already chose an opiton
		self.npcGui.name.Visible = false
		self.npcGui.arrow.Visible = false
	end)()
end

function DialogModule:hideGui(exitQuip, notActuallyAnExitQuip)
	self.active = false
	self.talking = true
	notActuallyAnExitQuip = notActuallyAnExitQuip or false
	turnProximityPromptsOn(not notActuallyAnExitQuip)
	
	self.talking = false
	
	if notActuallyAnExitQuip then
		tweenService:Create(game.Workspace.CurrentCamera, TweenInfo.new(0.5, Enum.EasingStyle.Quad, Enum.EasingDirection.Out), {FieldOfView = 65}):Play()
	else
		tweenService:Create(game.Workspace.CurrentCamera, TweenInfo.new(0.5, Enum.EasingStyle.Quad, Enum.EasingDirection.Out), {FieldOfView = 70}):Play()
	end
	
	-- hide player response options
	local playerReponseOptions = DIALOG_RESPONSES_UI
	for i, option in playerReponseOptions:GetChildren() do
		if not option:IsA("GuiButton") then continue end
		option.Visible = false
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
		for i, letter in string.split(exitQuip,"") do
			if dialogObject.Text ~= currenttext and skip == 0 then warn("other dialog happening") break end
			currenttext = currenttext .. letter
			if letter == "<" then skip = true end
			if letter == ">" then skip = false arrow += 1 continue end
			if arrow == 2 then arrow = 0 end
			if skip then continue end
			dialogObject.Text = currenttext .. if arrow == 1 then "</font>" else ""
			TICK_SOUND:Play()
			task.wait(0.02)
		end
			
		dialogObject.Text = exitQuip
		if notActuallyAnExitQuip then return end
	end
	
	task.spawn(function()
		if exitQuip then
			wait(2)
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
		--self.npcGui.AlwaysOnTop = false
		turnProximityPromptsOn(true)
	end)
end

function DialogModule:nextOption()
	self.dialogOption += 1
	if #self.dialogs < self.dialogOption then warn("No next dialog option for, " .. self.npcName) self.dialogOption -= 1 end
	return self.dialogOption
end

function turnProximityPromptsOn(yes)
	for i, prompt in collectionService:GetTagged("NPCprompt") do
		if prompt:IsA("ProximityPrompt") then
			prompt.Enabled = yes
		end
	end
end

return DialogModule