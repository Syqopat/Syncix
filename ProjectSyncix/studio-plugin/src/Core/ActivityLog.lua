-- ActivityLog
-- Syncix'in ne yaptığını görünür kılan akış günlüğü.
--
-- NEDEN VAR:
-- Rojo'da "patch visualizer" var çünkü Rojo tek yönlü çalışıyor ve bağlandığında
-- büyük bir farkı tek seferde onaya sunuyor. Bizde durum farklı ve aslında daha
-- kötü: değişiklikler sürekli akıyor ve TAMAMEN SESSİZ uygulanıyordu. Kullanıcı
-- yerinde bir şeyin değiştiğini ancak gözüyle fark ederse anlıyordu — Position
-- hatasında tam olarak bu oldu, objects 0,0,0'a düştü ve kimse günlerce görmedi.
--
-- Rojo'nun onay diyaloğunu kopyalamak bize uymaz: sürekli ve çift yönlü bir akışta
-- her değişikliği onaylatmak kullanılamaz hale gelir. Onun yerine AKIŞ GÜNLÜĞÜ:
-- engellemez, ama ne gelip ne gittiğini geriye dönük gösterir.
--
-- İkinci iş: ÇAKIŞMA UYARISI. Aynı property'yi kısa süre içinde hem kullanıcı hem
-- Syncix değiştirirse biri sessizce eziliyordu. Artık işaretleniyor.

local ActivityLog = {}
ActivityLog.__index = ActivityLog

-- Bellekte tutulan en fazla kayıt. Panel zaten last birkaç onu gösteriyor;
-- sınırsız büyümek uzun oturumlarda bellek sızıntısı olurdu.
local MAX_ENTRIES = 80

-- Bu süre içinde hem outgoing hem incoming değişiklik varsa çakışma sayılır.
local CONFLICT_WINDOW = 3

function ActivityLog.new()
	local self = setmetatable({}, ActivityLog)
	self.entries = {}
	-- (uuid|property) -> { datum = ..., timestamp = ... }  last GİDEN değişiklikler
	self.lastOutgoing = {}
	self.conflictCount = 0
	return self
end

local function keyName(uuid, field)
	return tostring(uuid) .. "|" .. tostring(field)
end

local function shortValue(datum): string
	local t = typeof(datum)
	if t == "string" then
		if #datum > 40 then
			return string.sub(datum, 1, 37) .. "..."
		end
		return datum
	elseif t == "Vector3" then
		return string.format("%.4g, %.4g, %.4g", datum.X, datum.Y, datum.Z)
	elseif t == "Color3" then
		return string.format("#%02x%02x%02x",
			math.floor(datum.R * 255 + 0.5),
			math.floor(datum.G * 255 + 0.5),
			math.floor(datum.B * 255 + 0.5))
	elseif t == "nil" then
		return "(removed)"
	end
	return tostring(datum)
end

function ActivityLog:_Add(entry)
	table.insert(self.entries, 1, entry) -- en yenisi başta
	if #self.entries > MAX_ENTRIES then
		table.remove(self.entries)
	end
end

--- Studio'da olan ve core'a GÖNDERİLEN bir değişiklik.
function ActivityLog:Outbound(pass: string, targetName: string, field: string?, datum: any, uuid: string?)
	if uuid and field then
		self.lastOutgoing[keyName(uuid, field)] = { datum = datum, timestamp = os.clock() }
	end
	self:_Add({
		direction = "out",
		pass = pass,
		target = targetName,
		field = field,
		datum = shortValue(datum),
		timestamp = os.clock(),
		conflict = false,
	})
end

--- Core'dan GELEN ve Studio'ya applied bir değişiklik.
--- Kısa süre önce aynı field Studio'dan gönderilmişse ve değer farklıysa
--- bu bir çakışmadır: kullanıcının değişikliği eziliyor demektir.
function ActivityLog:Inbound(pass: string, targetName: string, field: string?, datum: any, uuid: string?)
	local conflict = false

	if uuid and field then
		local previous = self.lastOutgoing[keyName(uuid, field)]
		if previous and (os.clock() - previous.timestamp) < CONFLICT_WINDOW then
			local ok, isEqual = pcall(function()
				return previous.datum == datum
			end)
			if not (ok and isEqual) then
				conflict = true
				self.conflictCount += 1
				warn(string.format(
					"[Syncix] Conflict on %s.%s — you set '%s', then '%s' arrived from the editor and overwrote it.",
					tostring(targetName), tostring(field),
					shortValue(previous.datum), shortValue(datum)
				))
			end
		end
	end

	self:_Add({
		direction = "in",
		pass = pass,
		target = targetName,
		field = field,
		datum = shortValue(datum),
		timestamp = os.clock(),
		conflict = conflict,
	})
end

--- Panelde göstermek için en fresh kayıtlar.
function ActivityLog:Recent(itemCount: number)
	local output = {}
	for i = 1, math.min(itemCount, #self.entries) do
		table.insert(output, self.entries[i])
	end
	return output
end

function ActivityLog:Summary()
	local incoming, outgoing = 0, 0
	for _, k in ipairs(self.entries) do
		if k.direction == "in" then incoming += 1 else outgoing += 1 end
	end
	return {
		total = #self.entries,
		incoming = incoming,
		outgoing = outgoing,
		conflict = self.conflictCount,
	}
end

return ActivityLog
