--!strict
-- Theme
--
-- Colours come from Studio itself, so the panel matches the editor around it in both
-- the light and the dark theme instead of carrying its own dark palette. Only the
-- accent is ours.

local Theme = {}

local ACCENT = Color3.fromRGB(76, 141, 245)
local ACCENT_PRESSED = Color3.fromRGB(58, 118, 216)

local OK = Color3.fromRGB(58, 176, 106)
local WARN = Color3.fromRGB(214, 162, 54)
local BAD = Color3.fromRGB(214, 88, 88)

Theme.accent = ACCENT
Theme.accentPressed = ACCENT_PRESSED
Theme.ok = OK
Theme.warn = WARN
Theme.bad = BAD

-- One rhythm for the whole panel: no hand-typed pixel gaps.
Theme.space = { tight = 4, small = 6, medium = 10, large = 16 }
Theme.corner = 4
Theme.rowHeight = 26

Theme.font = {
	body = Enum.Font.Gotham,
	medium = Enum.Font.GothamMedium,
	bold = Enum.Font.GothamBold,
}
Theme.size = { small = 11, body = 13, title = 15 }

local function studioTheme()
	local ok, value = pcall(function()
		return settings().Studio.Theme
	end)
	return ok and value or nil
end

local FALLBACK = {
	MainBackground = Color3.fromRGB(46, 46, 46),
	Tab = Color3.fromRGB(53, 53, 53),
	Border = Color3.fromRGB(34, 34, 34),
	MainText = Color3.fromRGB(245, 245, 245),
	SubText = Color3.fromRGB(170, 170, 170),
	Button = Color3.fromRGB(60, 60, 60),
	InputFieldBackground = Color3.fromRGB(37, 37, 37),
}

--[[
	One Studio colour, with an optional modifier ("Hover", "Pressed", "Disabled").
	Falls back to a fixed dark palette if this Studio build refuses the call, so a
	theme lookup can never stop the panel from drawing.
]]
function Theme.colour(name: string, modifier: string?): Color3
	local theme = studioTheme()
	if theme then
		local ok, value = pcall(function()
			local guide = (Enum.StudioStyleGuideColor :: any)[name]
			if modifier then
				return theme:GetColor(guide, (Enum.StudioStyleGuideModifier :: any)[modifier])
			end
			return theme:GetColor(guide)
		end)
		if ok and typeof(value) == "Color3" then
			return value
		end
	end
	return FALLBACK[name] or FALLBACK.MainBackground
end

function Theme.background(): Color3
	return Theme.colour("MainBackground")
end

function Theme.card(): Color3
	return Theme.colour("Tab")
end

function Theme.border(): Color3
	return Theme.colour("Border")
end

function Theme.text(): Color3
	return Theme.colour("MainText")
end

function Theme.subText(): Color3
	return Theme.colour("SubText")
end

function Theme.field(): Color3
	return Theme.colour("InputFieldBackground")
end

--[[
	Calls back whenever the user switches Studio's theme, so a panel that is already
	open repaints instead of staying dark on a light editor.
]]
function Theme.onChanged(callback: () -> ()): RBXScriptConnection?
	local ok, connection = pcall(function()
		return settings().Studio.ThemeChanged:Connect(callback)
	end)
	return ok and connection or nil
end

return Theme
