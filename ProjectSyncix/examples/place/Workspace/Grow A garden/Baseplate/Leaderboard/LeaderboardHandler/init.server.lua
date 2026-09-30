local US = game:GetService("UserService")
local DS = game:GetService("DataStoreService")
local P = game:GetService("Players")
local Data = DS:GetOrderedDataStore("Sheckles_LeaderboardRESET")

local ScrollingFrame = script.Parent.View.SurfaceGui.Frame.Frame.Frame1.ScrollingFrame
local RefreshText = script.Parent.View.SurfaceGui.Frame.Frame.Frame2.TextLabel1

local red = Color3.fromHSV(0, 1, 0.933333)
local green = Color3.fromHSV(0.308028, 1, 0.929412)

local ListSize = 50
local updRate = 60

function Format(Num)
	return string.format("%02i", Num)
end

function Convert(Seconds)
	local Minutes = (Seconds - Seconds%60)/60
	Seconds = Seconds - Minutes*60
	local Hours = (Minutes - Minutes%60)/60
	Minutes = Minutes - Hours*60
	return Format(Hours)..":"..Format(Minutes)..":"..Format(Seconds)
end

while true do
	for i, v in pairs(ScrollingFrame:GetChildren()) do 
		if v:IsA("Frame") then v:Destroy() end
	end

	local Pages
	local s,e = pcall(function()
		Pages = Data:GetSortedAsync(false, ListSize)
	end)
	if s then
		local TopList = Pages:GetCurrentPage()
		for i, v in pairs(TopList) do
			local UserId = v["key"]
			local value = v["value"]
			if not value then continue end

			local Template = script.Template:Clone()
			local Rank = Template.Frame.Placement
			local UN = Template.Frame.Username
			local AMT = Template.Value
			local Pfp = Template.Frame.ImageLabel.ProfilePicture

			Template.LayoutOrder = i
			AMT.Text = require(game.ReplicatedStorage.Comma_Module).Comma(value).."¢"
			Rank.Text = "#" .. i
			
			local color = green:Lerp(red, (i / 50)) -- green:Lerp(red, (i / #TopList))
			AMT.TextColor3 = color
			UN.TextColor3 = color
			Rank.TextColor3 = color
			Pfp.Parent.ProfilePictureBorder.BackgroundColor3 = color

			local UserInfo = P:GetNameFromUserIdAsync(UserId)
			local PfpImg,ir = P:GetUserThumbnailAsync(UserId,Enum.ThumbnailType.HeadShot,Enum.ThumbnailSize.Size100x100)
			if not UserInfo or not PfpImg then continue end
			UN.Text = "@"..UserInfo
			Pfp.Image = PfpImg

			Template.Name = UserInfo
			Template.Parent = ScrollingFrame
		end
	end

	local count = updRate
	task.spawn(function()
		for i = 1, count do
			count -= 1
			RefreshText.Text = string.format("Refreshes in: %s",Convert(count))
			wait(1)
		end
	end)
	
	task.wait(updRate)
end