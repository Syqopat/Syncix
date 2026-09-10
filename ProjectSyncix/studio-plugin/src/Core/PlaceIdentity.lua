--!strict
-- This place's persistent identity. SINGLE SOURCE.
--
-- Why one file: the identity is needed both for port scanning (ConnectionManager) and for
-- sending the tree (PatchBuilder). Computed separately in two places, one would fall
-- behind when the other changed, and the two sides would take the same place for DIFFERENT ones.
-- Exactly this bug happened with the service list.

local RunService = game:GetService("RunService")
local HttpService = game:GetService("HttpService")

local PlaceIdentity = {}

--- Returns the identity.
---
--- game.PlaceId is tried first. The reason matters: the first version wrote the identity to
--- the Workspace as an attribute, but attributes are SAVED with the place.
--- When the user closed Studio without saving, the identity was lost; when the place was
--- opened again a new identity was generated and Syncix took it for "another place"
--- and stopped sync. PlaceId does not depend on saving.
---
--- The attribute remains only as a fallback for places without a PlaceId (never saved,
--- entirely local).
function PlaceIdentity.Resolve(): string
	local id = game.PlaceId
	if id ~= nil and id ~= 0 then
		return "place:" .. tostring(id)
	end

	local Workspace = game:GetService("Workspace")
	local identity = Workspace:GetAttribute("__syncix_place")
	if type(identity) ~= "string" or identity == "" then
		identity = "local:" .. HttpService:GenerateGUID(false)
		-- In an unsaved place this attribute is not persistent either; it only
		-- keeps things consistent within the same session.
		Workspace:SetAttribute("__syncix_place", identity)
	end
	return identity
end

--- Short description to show to the user.
function PlaceIdentity.Describe(): string
	if game.PlaceId ~= 0 then
		return string.format("%s (placeId %d)", game.Name, game.PlaceId)
	end
	return game.Name .. " (unsaved place)"
end

return PlaceIdentity
