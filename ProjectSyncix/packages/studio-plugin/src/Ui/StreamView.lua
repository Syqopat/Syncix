--!strict
-- StreamView
--
-- What Syncix just did. Changes flow continuously in both directions and used to be
-- applied in silence: the only way to notice was to spot it in the Explorer, which is
-- how objects once dropped to 0,0,0 for days without anyone seeing.

local Theme = require(script.Parent.Theme)
local Widgets = require(script.Parent.Widgets)

local StreamView = {}
StreamView.__index = StreamView

local SHOWN = 40

function StreamView.new(activityLog)
	return setmetatable({ activityLog = activityLog, rows = {} }, StreamView)
end

function StreamView:Build(parent: Instance)
	local column = Widgets.column(parent, Theme.space.small)
	self.root = column

	local header = Widgets.row(column, 18, 1)
	Widgets.label(header, "LIVE", {
		font = Theme.font.bold,
		size = Theme.size.small,
		colour = Theme.subText(),
		size2 = UDim2.new(0.5, 0, 1, 0),
	})
	self.countLabel = Widgets.label(header, "", {
		size = Theme.size.small,
		colour = Theme.subText(),
		align = Enum.TextXAlignment.Right,
		size2 = UDim2.new(0.5, 0, 1, 0),
		position = UDim2.new(0.5, 0, 0, 0),
	})

	local holder = Widgets.card(column, 2)
	holder.Size = UDim2.new(1, 0, 1, -24)
	holder.AutomaticSize = Enum.AutomaticSize.None
	holder.ClipsDescendants = true

	self.list = Widgets.scroll(holder)
	local layout = Instance.new("UIListLayout")
	layout.SortOrder = Enum.SortOrder.LayoutOrder
	layout.Padding = UDim.new(0, 1)
	layout.Parent = self.list

	self.empty = Widgets.label(holder, "Nothing has moved yet.", {
		colour = Theme.subText(),
		align = Enum.TextXAlignment.Center,
	})

	return column
end

local function arrow(direction: string): string
	return direction == "out" and "\u{2192}" or "\u{2190}"
end

--[[
	One line per change: which way it went, what was touched, and the value. A conflict
	-- the same property changed on both sides within seconds -- is the one thing here
	worth a colour.
]]
function StreamView:Refresh()
	if not self.root then
		return
	end

	local entries = self.activityLog:Recent(SHOWN)
	self.empty.Visible = #entries == 0
	self.empty.TextColor3 = Theme.subText()

	local summary = self.activityLog:Summary()
	self.countLabel.Text = string.format("%d in  ·  %d out  ·  %d conflicts",
		summary.incoming or 0, summary.outgoing or 0, summary.conflict or 0)
	self.countLabel.TextColor3 = Theme.subText()

	for index = 1, SHOWN do
		local entry = entries[index]
		local row = self.rows[index]

		if entry and not row then
			local frame = Widgets.row(self.list, 20, index)
			row = {
				frame = frame,
				direction = Widgets.label(frame, "", {
					size = Theme.size.small,
					size2 = UDim2.new(0, 14, 1, 0),
				}),
				target = Widgets.label(frame, "", {
					size = Theme.size.small,
					size2 = UDim2.new(0.55, -14, 1, 0),
					position = UDim2.new(0, 14, 0, 0),
				}),
				value = Widgets.label(frame, "", {
					size = Theme.size.small,
					size2 = UDim2.new(0.45, 0, 1, 0),
					position = UDim2.new(0.55, 0, 0, 0),
					align = Enum.TextXAlignment.Right,
				}),
			}
			self.rows[index] = row
		end

		if not row then
			continue
		end

		if not entry then
			row.frame.Visible = false
			continue
		end

		row.frame.Visible = true
		row.direction.Text = arrow(entry.direction)
		row.direction.TextColor3 = entry.direction == "out" and Theme.accent or Theme.ok
		row.target.Text = entry.field and (entry.target .. "." .. entry.field) or entry.target
		row.target.TextColor3 = entry.conflict and Theme.bad or Theme.text()
		row.value.Text = entry.conflict and ("conflict  " .. tostring(entry.datum))
			or tostring(entry.datum)
		row.value.TextColor3 = entry.conflict and Theme.bad or Theme.subText()
	end
end

return StreamView
