-- ActivityLog
-- Syncix'in ne yaptığını görünür kılan akış günlüğü.
--
-- NEDEN VAR:
-- Rojo'da "patch visualizer" var çünkü Rojo tek yönlü çalışıyor ve bağlandığında
-- büyük bir farkı tek seferde onaya sunuyor. Bizde durum farklı ve aslında daha
-- kötü: değişiklikler sürekli akıyor ve TAMAMEN SESSİZ uygulanıyordu. Kullanıcı
-- yerinde bir şeyin değiştiğini ancak gözüyle fark ederse anlıyordu — Position
-- hatasında tam olarak bu oldu, objeler 0,0,0'a düştü ve kimse günlerce görmedi.
--
-- Rojo'nun onay diyaloğunu kopyalamak bize uymaz: sürekli ve çift yönlü bir akışta
-- her değişikliği onaylatmak kullanılamaz hale gelir. Onun yerine AKIŞ GÜNLÜĞÜ:
-- engellemez, ama ne gelip ne gittiğini geriye dönük gösterir.
--
-- İkinci iş: ÇAKIŞMA UYARISI. Aynı property'yi kısa süre içinde hem kullanıcı hem
-- Syncix değiştirirse biri sessizce eziliyordu. Artık işaretleniyor.

local ActivityLog = {}
ActivityLog.__index = ActivityLog

-- Bellekte tutulan en fazla kayıt. Panel zaten son birkaç onu gösteriyor;
-- sınırsız büyümek uzun oturumlarda bellek sızıntısı olurdu.
local AZAMI_KAYIT = 80

-- Bu süre içinde hem giden hem gelen değişiklik varsa çakışma sayılır.
local CAKISMA_PENCERESI = 3

function ActivityLog.new()
	local self = setmetatable({}, ActivityLog)
	self.kayitlar = {}
	-- (uuid|property) -> { deger = ..., zaman = ... }  son GİDEN değişiklikler
	self.sonGiden = {}
	self.cakismaSayisi = 0
	return self
end

local function anahtar(uuid, alan)
	return tostring(uuid) .. "|" .. tostring(alan)
end

local function kisaDeger(deger): string
	local t = typeof(deger)
	if t == "string" then
		if #deger > 40 then
			return string.sub(deger, 1, 37) .. "..."
		end
		return deger
	elseif t == "Vector3" then
		return string.format("%.4g, %.4g, %.4g", deger.X, deger.Y, deger.Z)
	elseif t == "Color3" then
		return string.format("#%02x%02x%02x",
			math.floor(deger.R * 255 + 0.5),
			math.floor(deger.G * 255 + 0.5),
			math.floor(deger.B * 255 + 0.5))
	elseif t == "nil" then
		return "(removed)"
	end
	return tostring(deger)
end

function ActivityLog:_Ekle(kayit)
	table.insert(self.kayitlar, 1, kayit) -- en yenisi başta
	if #self.kayitlar > AZAMI_KAYIT then
		table.remove(self.kayitlar)
	end
end

--- Studio'da olan ve core'a GÖNDERİLEN bir değişiklik.
function ActivityLog:Giden(tur: string, hedefAdi: string, alan: string?, deger: any, uuid: string?)
	if uuid and alan then
		self.sonGiden[anahtar(uuid, alan)] = { deger = deger, zaman = os.clock() }
	end
	self:_Ekle({
		yon = "out",
		tur = tur,
		hedef = hedefAdi,
		alan = alan,
		deger = kisaDeger(deger),
		zaman = os.clock(),
		cakisma = false,
	})
end

--- Core'dan GELEN ve Studio'ya uygulanan bir değişiklik.
--- Kısa süre önce aynı alan Studio'dan gönderilmişse ve değer farklıysa
--- bu bir çakışmadır: kullanıcının değişikliği eziliyor demektir.
function ActivityLog:Gelen(tur: string, hedefAdi: string, alan: string?, deger: any, uuid: string?)
	local cakisma = false

	if uuid and alan then
		local onceki = self.sonGiden[anahtar(uuid, alan)]
		if onceki and (os.clock() - onceki.zaman) < CAKISMA_PENCERESI then
			local ok, esit = pcall(function()
				return onceki.deger == deger
			end)
			if not (ok and esit) then
				cakisma = true
				self.cakismaSayisi += 1
				warn(string.format(
					"[Syncix] Conflict on %s.%s — you set '%s', then '%s' arrived from the editor and overwrote it.",
					tostring(hedefAdi), tostring(alan),
					kisaDeger(onceki.deger), kisaDeger(deger)
				))
			end
		end
	end

	self:_Ekle({
		yon = "in",
		tur = tur,
		hedef = hedefAdi,
		alan = alan,
		deger = kisaDeger(deger),
		zaman = os.clock(),
		cakisma = cakisma,
	})
end

--- Panelde göstermek için en yeni kayıtlar.
function ActivityLog:Son(adet: number)
	local cikti = {}
	for i = 1, math.min(adet, #self.kayitlar) do
		table.insert(cikti, self.kayitlar[i])
	end
	return cikti
end

function ActivityLog:Ozet()
	local gelen, giden = 0, 0
	for _, k in ipairs(self.kayitlar) do
		if k.yon == "in" then gelen += 1 else giden += 1 end
	end
	return {
		toplam = #self.kayitlar,
		gelen = gelen,
		giden = giden,
		cakisma = self.cakismaSayisi,
	}
end

return ActivityLog
