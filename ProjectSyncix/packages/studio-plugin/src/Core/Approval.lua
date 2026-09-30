-- Approval
-- User approval on first connection (trust on first use).
--
-- Why it exists: the core could drive Studio over 127.0.0.1 without any identity check.
-- Any program on the machine could open a server on 8080 and change objects
-- in Studio. Instead of a password or token, a simpler and clearer way was
-- chosen: Studio shows which PROJECT FOLDER wants to connect and asks for permission once.
-- Approved folders are stored and not asked again.

local HttpService = game:GetService("HttpService")

local Approval = {}

local SETTING_PREFIX = "syncix_approval_"

-- TWO-LAYER PERSISTENCE
--
-- 1. plugin:SetSetting — works for plugins installed from the Creator Store.
--    But when the .rbxm file is dropped straight into the Plugins folder (our
--    distribution form), the plugin has no registered identity, so SETTINGS ARE NOT WRITTEN TO DISK.
--    Measured: after restarting Studio the permission was reset and the InstalledPlugins
--    folder was never created.
--
-- 2. A game attribute — as a fallback the decision is written into the place itself. The `game`
--    object is outside the services the observer watches, so it does not leak into sync and
--    does not appear in files. It persists when the place is saved.
--
-- Reading tries both, writing writes to both.
local GAME_ATTR = "__syncix_approved_roots"

local function keyName(root: string): string
	return SETTING_PREFIX .. string.lower(root)
end

local function readPlaceRecords(): { [string]: boolean }
	local ok, raw = pcall(function()
		return game:GetAttribute(GAME_ATTR)
	end)
	if not ok or type(raw) ~= "string" or raw == "" then
		return {}
	end
	local okDecode, tbl = pcall(function()
		return HttpService:JSONDecode(raw)
	end)
	if okDecode and type(tbl) == "table" then
		return tbl
	end
	return {}
end

local function writePlaceRecords(entries)
	pcall(function()
		game:SetAttribute(GAME_ATTR, HttpService:JSONEncode(entries))
	end)
end

-- A decision given earlier: true (allowed), false (denied), nil (never asked)
function Approval.GetStoredDecision(pluginRef, root: string)
	if not root or root == "" then
		return nil
	end
	local k = keyName(root)

	if pluginRef then
		local ok, datum = pcall(function()
			return pluginRef:GetSetting(k)
		end)
		if ok and type(datum) == "boolean" then
			return datum
		end
	end

	local entries = readPlaceRecords()
	local datum = entries[k]
	if type(datum) == "boolean" then
		return datum
	end

	return nil
end

function Approval.Store(pluginRef, root: string, permission: boolean)
	if not root or root == "" then
		return
	end
	local k = keyName(root)

	if pluginRef then
		pcall(function()
			pluginRef:SetSetting(k, permission)
		end)
	end

	local entries = readPlaceRecords()
	entries[k] = permission
	writePlaceRecords(entries)

	-- Read-back check: if no layer persisted, the user must know,
	-- or they will take being asked on every Studio start for a bug.
	if Approval.GetStoredDecision(pluginRef, root) == nil then
		warn(
			"[Syncix] The decision could not be stored permanently; you will be asked on every Studio start.\n" ..
			"  Save the place (Ctrl+S) to store the decision with it."
		)
	end
end

-- The window id must be unique on every call: calling
-- CreateDockWidgetPluginGui a second time with the same id throws an error.
local windowCounter = 0

-- Shows the approval window and waits until the user decides.
--
-- The return value has three states, and that MATTERS:
--   "allow"  the user approved
--   "deny"   the user denied (stored permanently)
--   "error"  the window could not open -> NOT A DECISION; not stored, retried later
-- Otherwise a single UI glitch would lock sync permanently.
function Approval.Ask(pluginRef, info): string
	local projectInfo = tostring(info.project or "Unknown project")
	local root = tostring(info.root or "")
	local port = tostring(info.port or "?")

	windowCounter += 1

	local ok, result = pcall(function()
		local widgetInfo = DockWidgetPluginGuiInfo.new(
			Enum.InitialDockState.Float,
			true,  -- open initially
			true,  -- override the restored window state
			460, 210,
			460, 210
		)
		local gui = pluginRef:CreateDockWidgetPluginGui(
			"SyncixBaglantiOnayi_" .. tostring(windowCounter),
			widgetInfo
		)
		gui.Title = "Syncix - Connection permission"

		local frame = Instance.new("Frame")
		frame.Size = UDim2.new(1, 0, 1, 0)
		frame.BackgroundColor3 = Color3.fromRGB(30, 33, 40)
		frame.BorderSizePixel = 0
		frame.Parent = gui

		local title = Instance.new("TextLabel")
		title.Size = UDim2.new(1, -24, 0, 28)
		title.Position = UDim2.new(0, 12, 0, 12)
		title.BackgroundTransparency = 1
		title.TextColor3 = Color3.fromRGB(255, 255, 255)
		title.TextXAlignment = Enum.TextXAlignment.Left
		title.Font = Enum.Font.GothamBold
		title.TextSize = 16
		title.Text = "This project wants to modify Studio"
		title.Parent = frame

		local detail = Instance.new("TextLabel")
		detail.Size = UDim2.new(1, -24, 0, 92)
		detail.Position = UDim2.new(0, 12, 0, 44)
		detail.BackgroundTransparency = 1
		detail.TextColor3 = Color3.fromRGB(210, 214, 222)
		detail.TextXAlignment = Enum.TextXAlignment.Left
		detail.TextYAlignment = Enum.TextYAlignment.Top
		detail.TextWrapped = true
		detail.Font = Enum.Font.Gotham
		detail.TextSize = 13
		detail.Text = string.format(
			"Project: %s\nFolder: %s\nPort: %s\n\nIf you do not recognise this folder, deny it. If you allow it, this project can create, modify and delete instances in the Explorer.",
			projectInfo, root, port
		)
		detail.Parent = frame

		local function button(text, x, color)
			local b = Instance.new("TextButton")
			b.Size = UDim2.new(0, 200, 0, 34)
			b.Position = UDim2.new(0, x, 1, -46)
			b.BackgroundColor3 = color
			b.BorderSizePixel = 0
			b.TextColor3 = Color3.fromRGB(255, 255, 255)
			b.Font = Enum.Font.GothamBold
			b.TextSize = 14
			b.Text = text
			b.Parent = frame
			local corner = Instance.new("UICorner")
			corner.CornerRadius = UDim.new(0, 6)
			corner.Parent = b
			return b
		end

		local permissionButton = button("Allow", 12, Color3.fromRGB(46, 160, 87))
		local denyButton = button("Deny", 236, Color3.fromRGB(180, 60, 60))

		-- The window may have been restored closed by Studio; make sure it is open.
		gui.Enabled = true

		local decision = nil
		local wasClosed = false
		local openedAt = os.clock()

		permissionButton.Activated:Connect(function() decision = true end)
		denyButton.Activated:Connect(function() decision = false end)

		-- Closing the window DOES NOT COUNT AS A DENIAL.
		--
		-- It used to, and that caused this bug: while the DockWidgetPluginGui was being created,
		-- Enabled was false for a moment, which was read as "denied" before the user pressed
		-- anything and blocked the connection permanently.
		-- Now closing means "undecided": not stored, asked again on the next attempt.
		gui:GetPropertyChangedSignal("Enabled"):Connect(function()
			-- The first second is left for Studio restoring its own window state.
			if not gui.Enabled and decision == nil and (os.clock() - openedAt) > 1 then
				wasClosed = true
			end
		end)

		-- Wait at most 2 minutes; if the user is not there, the connection attempt
		-- must not hang forever.
		while decision == nil and not wasClosed and (os.clock() - openedAt) < 120 do
			task.wait(0.1)
		end

		gui.Enabled = false
		gui:Destroy()

		if decision == nil then
			return nil -- decision verilmedi
		end
		return decision
	end)

	if not ok then
		-- The window could not open. This is NOT A DENIAL: it is not stored permanently,
		-- and the connection asks again on the next attempt.
		warn(
			"[Syncix] Could not open the approval window: " .. tostring(result) ..
			"\n  Not connected; will retry shortly."
		)
		return "error"
	end

	if result == true then
		return "allow"
	end
	if result == false then
		return "deny"
	end

	-- nil: the window was closed or timed out. No decision was made;
	-- it is not stored or blacklisted, and it is asked again on the next attempt.
	print("[Syncix] Permission not granted (window closed). You will be asked again later.")
	return "error"
end

return Approval
