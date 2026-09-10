-- EchoGuard
-- Syncix'in kendi uyguladığı değişikliklerin core'a geri gönderilmesini engeller.
--
-- SORUN:
-- Eskiden echo engelleme CommandDispatcher'daki `isLocked` bayrağına dayanıyordu:
-- patch uygulanmadan önce Lock(), hemen sonra Unlock(). Ama Roblox'ta property
-- sinyalleri (Changed, AttributeChanged, AncestryChanged) DEFERRED çalışır; yani
-- gözlemcinin fonksiyonu Unlock() çalıştıktan SONRA tetiklenir. O anda kilit açık
-- olduğu için değişiklik echo olarak core'a geri gönderiliyordu.
--
-- Ölçüm: core'dan Studio'ya 40 property komutu gönderildiğinde eklentinin kuyruğuna
-- 64 değişiklik girdi, yani gönderdiğimiz her şey geri geliyordu. Sonsuz döngü
-- oluşmuyordu (değer aynı olduğu için ikinci turda duruyor) ama trafik iki katına
-- çıkıyor ve disk yazıcısı boşuna tetikleniyordu.
--
-- ÇÖZÜM:
-- Zamanlamaya değil DEĞERE bakmak. Bir patch uygulanmadan hemen önce "bu object için
-- bu property'nin şu değere ayarlanmasını bekliyorum" diye not düşülür. Gözlemci
-- tetiklendiğinde incoming değer beklenen değerle aynıysa bu bizim kendi yazımızdır,
-- gönderilmez. Kayıt bir kez kullanılır ve kısa sürede timestamp aşımına uğrar; yani
-- kullanıcının GERÇEK değişiklikleri asla yutulmaz.

local EchoGuard = {}
EchoGuard.__index = EchoGuard

-- Beklenti bu süre içinde tüketilmezse düşer. Deferred sinyaller aynı kare içinde
-- ya da bir sonrakinde gelir; 2 saniye fazlasıyla güvenli bir üst sınırdır.
local TTL = 2

function EchoGuard.new()
	local self = setmetatable({}, EchoGuard)
	self.expectedList = {}
	self.lastPrune = os.clock()
	return self
end

local function keyName(uuid: string, field: string): string
	return tostring(uuid) .. "|" .. tostring(field)
end

function EchoGuard:Prune()
	local now = os.clock()
	if now - self.lastPrune < 5 then
		return
	end
	self.lastPrune = now
	for k, entry in pairs(self.expectedList) do
		if now - entry.timestamp > TTL then
			self.expectedList[k] = nil
		end
	end
end

-- "Bu değeri ben yazıyorum" notu. Patch uygulanmadan HEMEN ÖNCE çağrılır.
function EchoGuard:Expect(uuid: string, field: string, datum: any)
	if not uuid or not field then return end
	self.expectedList[keyName(uuid, field)] = { datum = datum, timestamp = os.clock() }
	self:Prune()
end

-- Gözlemciden incoming değişiklik bizim kendi yazımız mı?
-- Öyleyse kayıt tüketilir ve true döner (gönderme).
function EchoGuard:Consume(uuid: string, field: string, datum: any): boolean
	local k = keyName(uuid, field)
	local entry = self.expectedList[k]
	if not entry then
		return false
	end

	if os.clock() - entry.timestamp > TTL then
		self.expectedList[k] = nil
		return false
	end

	-- Roblox tipleri (Vector3, Color3, UDim2, CFrame) ve ilkel tipler == ile
	-- doğru karşılaştırılır. Eşitlik pcall içinde: beklenmedik tipler failure vermesin.
	local ok, isEqual = pcall(function()
		return entry.datum == datum
	end)

	if ok and isEqual then
		self.expectedList[k] = nil
		return true
	end

	-- Değer farklı: kullanıcı gerçekten değiştirmiş. Beklentiyi düşür ve gönder.
	self.expectedList[k] = nil
	return false
end

return EchoGuard
