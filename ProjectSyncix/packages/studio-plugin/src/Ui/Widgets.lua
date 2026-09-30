--!strict
-- Widgets
--
-- The pieces every view is built from. They exist so no view has to place an element
-- at a hand-picked y position: a column lays its children out, and each child reports
-- its own height.

local Theme = require(script.Parent.Theme)

local Widgets = {}

local function corner(parent: Instance, radius: number?)
	local item = Instance.new("UICorner")
	item.CornerRadius = UDim.new(0, radius or Theme.corner)
	item.Parent = parent
	return item
end

local function stroke(parent: Instance, colour: Color3?)
	local item = Instance.new("UIStroke")
	item.Color = colour or Theme.border()
	item.Thickness = 1
	item.ApplyStrokeMode = Enum.ApplyStrokeMode.Border
	item.Parent = parent
	return item
end

Widgets.corner = corner
Widgets.stroke = stroke

function Widgets.padding(parent: Instance, amount: number?)
	local pad = Instance.new("UIPadding")
	local value = UDim.new(0, amount or Theme.space.medium)
	pad.PaddingTop = value
	pad.PaddingBottom = value
	pad.PaddingLeft = value
	pad.PaddingRight = value
	pad.Parent = parent
	return pad
end

--[[
	A vertical stack. Children are ordered by LayoutOrder and the stack grows with
	them, so a view never has to know how tall its contents are.
]]
function Widgets.column(parent: Instance, gap: number?): Frame
	local frame = Instance.new("Frame")
	frame.BackgroundTransparency = 1
	frame.BorderSizePixel = 0
	frame.Size = UDim2.new(1, 0, 0, 0)
	frame.AutomaticSize = Enum.AutomaticSize.Y

	local layout = Instance.new("UIListLayout")
	layout.FillDirection = Enum.FillDirection.Vertical
	layout.SortOrder = Enum.SortOrder.LayoutOrder
	layout.Padding = UDim.new(0, gap or Theme.space.small)
	layout.Parent = frame

	frame.Parent = parent
	return frame
end

function Widgets.row(parent: Instance, height: number?, order: number?): Frame
	local frame = Instance.new("Frame")
	frame.BackgroundTransparency = 1
	frame.BorderSizePixel = 0
	frame.Size = UDim2.new(1, 0, 0, height or Theme.rowHeight)
	frame.LayoutOrder = order or 1
	frame.Parent = parent
	return frame
end

function Widgets.card(parent: Instance, order: number?): Frame
	local frame = Instance.new("Frame")
	frame.BackgroundColor3 = Theme.card()
	frame.BorderSizePixel = 0
	frame.Size = UDim2.new(1, 0, 0, 0)
	frame.AutomaticSize = Enum.AutomaticSize.Y
	frame.LayoutOrder = order or 1
	corner(frame)
	stroke(frame)
	frame.Parent = parent
	return frame
end

function Widgets.label(parent: Instance, text: string, options: { [string]: any }?): TextLabel
	local opts = options or {}
	local item = Instance.new("TextLabel")
	item.BackgroundTransparency = 1
	item.BorderSizePixel = 0
	item.Text = text
	item.TextColor3 = opts.colour or Theme.text()
	item.Font = opts.font or Theme.font.body
	item.TextSize = opts.size or Theme.size.body
	item.TextXAlignment = opts.align or Enum.TextXAlignment.Left
	item.TextYAlignment = Enum.TextYAlignment.Center
	item.TextTruncate = Enum.TextTruncate.AtEnd
	item.RichText = opts.rich == true
	item.Size = opts.size2 or UDim2.new(1, 0, 1, 0)
	item.Position = opts.position or UDim2.new(0, 0, 0, 0)
	item.LayoutOrder = opts.order or 1
	item.Parent = parent
	return item
end

--[[
	A button that follows Studio's own button colours, with the accent as an option for
	the one action a view wants to lead with.
]]
function Widgets.button(parent: Instance, text: string, options: { [string]: any }?): TextButton
	local opts = options or {}
	local primary = opts.primary == true

	local item = Instance.new("TextButton")
	item.AutoButtonColor = false
	item.Text = text
	item.Font = Theme.font.medium
	item.TextSize = Theme.size.body
	item.TextColor3 = primary and Color3.new(1, 1, 1) or Theme.text()
	item.BackgroundColor3 = primary and Theme.accent or Theme.colour("Button")
	item.BorderSizePixel = 0
	item.Size = opts.size or UDim2.new(1, 0, 0, Theme.rowHeight)
	item.Position = opts.position or UDim2.new(0, 0, 0, 0)
	item.LayoutOrder = opts.order or 1
	corner(item)
	if not primary then
		stroke(item)
	end

	local resting = item.BackgroundColor3
	item.MouseEnter:Connect(function()
		item.BackgroundColor3 = primary and Theme.accentPressed or Theme.colour("Button", "Hover")
	end)
	item.MouseLeave:Connect(function()
		item.BackgroundColor3 = resting
	end)

	item.Parent = parent
	return item
end

function Widgets.field(parent: Instance, placeholder: string, options: { [string]: any }?): TextBox
	local opts = options or {}
	local item = Instance.new("TextBox")
	item.PlaceholderText = placeholder
	item.Text = opts.text or ""
	item.ClearTextOnFocus = false
	item.Font = Theme.font.body
	item.TextSize = Theme.size.body
	item.TextColor3 = Theme.text()
	item.PlaceholderColor3 = Theme.subText()
	item.BackgroundColor3 = Theme.field()
	item.BorderSizePixel = 0
	item.TextXAlignment = Enum.TextXAlignment.Left
	item.Size = opts.size or UDim2.new(1, 0, 0, Theme.rowHeight)
	item.Position = opts.position or UDim2.new(0, 0, 0, 0)
	item.LayoutOrder = opts.order or 1
	corner(item)
	stroke(item)

	local pad = Instance.new("UIPadding")
	pad.PaddingLeft = UDim.new(0, Theme.space.small)
	pad.PaddingRight = UDim.new(0, Theme.space.small)
	pad.Parent = item

	item.Parent = parent
	return item
end

--[[
	The coloured dot in front of a state line. A dot plus a word says more at a glance
	than a word alone, and it is the only place colour carries meaning here.
]]
function Widgets.dot(parent: Instance, colour: Color3, position: UDim2?): Frame
	local item = Instance.new("Frame")
	item.Size = UDim2.new(0, 8, 0, 8)
	item.Position = position or UDim2.new(0, 0, 0.5, -4)
	item.BackgroundColor3 = colour
	item.BorderSizePixel = 0
	corner(item, 4)
	item.Parent = parent
	return item
end

function Widgets.divider(parent: Instance, order: number?): Frame
	local item = Instance.new("Frame")
	item.Size = UDim2.new(1, 0, 0, 1)
	item.BackgroundColor3 = Theme.border()
	item.BorderSizePixel = 0
	item.LayoutOrder = order or 1
	item.Parent = parent
	return item
end

function Widgets.scroll(parent: Instance): ScrollingFrame
	local item = Instance.new("ScrollingFrame")
	item.Size = UDim2.new(1, 0, 1, 0)
	item.BackgroundTransparency = 1
	item.BorderSizePixel = 0
	item.ScrollBarThickness = 6
	item.ScrollBarImageColor3 = Theme.subText()
	item.CanvasSize = UDim2.new(0, 0, 0, 0)
	item.AutomaticCanvasSize = Enum.AutomaticSize.Y
	item.Parent = parent
	return item
end

return Widgets
