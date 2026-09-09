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
-- Zamanlamaya değil DEĞERE bakmak. Bir patch uygulanmadan hemen önce "bu obje için
-- bu property'nin şu değere ayarlanmasını bekliyorum" diye not düşülür. Gözlemci
-- tetiklendiğinde gelen değer beklenen değerle aynıysa bu bizim kendi yazımızdır,
-- gönderilmez. Kayıt bir kez kullanılır ve kısa sürede zaman aşımına uğrar; yani
-- kullanıcının GERÇEK değişiklikleri asla yutulmaz.

local EchoGuard = {}
EchoGuard.__index = EchoGuard

-- Beklenti bu süre içinde tüketilmezse düşer. Deferred sinyaller aynı kare içinde
-- ya da bir sonrakinde gelir; 2 saniye fazlasıyla güvenli bir üst sınırdır.
local YASAM_SURESI = 2

function EchoGuard.new()
	local self = setmetatable({}, EchoGuard)
	self.beklenenler = {}
	self.sonTemizlik = os.clock()
	return self
end

local function anahtar(uuid: string, alan: string): string
	return tostring(uuid) .. "|" .. tostring(alan)
end

function EchoGuard:Temizle()
	local simdi = os.clock()
	if simdi - self.sonTemizlik < 5 then
		return
	end
	self.sonTemizlik = simdi
	for k, kayit in pairs(self.beklenenler) do
		if simdi - kayit.zaman > YASAM_SURESI then
			self.beklenenler[k] = nil
		end
	end
end

-- "Bu değeri ben yazıyorum" notu. Patch uygulanmadan HEMEN ÖNCE çağrılır.
function EchoGuard:Expect(uuid: string, alan: string, deger: any)
	if not uuid or not alan then return end
	self.beklenenler[anahtar(uuid, alan)] = { deger = deger, zaman = os.clock() }
	self:Temizle()
end

-- Gözlemciden gelen değişiklik bizim kendi yazımız mı?
-- Öyleyse kayıt tüketilir ve true döner (gönderme).
function EchoGuard:Consume(uuid: string, alan: string, deger: any): boolean
	local k = anahtar(uuid, alan)
	local kayit = self.beklenenler[k]
	if not kayit then
		return false
	end

	if os.clock() - kayit.zaman > YASAM_SURESI then
		self.beklenenler[k] = nil
		return false
	end

	-- Roblox tipleri (Vector3, Color3, UDim2, CFrame) ve ilkel tipler == ile
	-- doğru karşılaştırılır. Eşitlik pcall içinde: beklenmedik tipler hata vermesin.
	local ok, esit = pcall(function()
		return kayit.deger == deger
	end)

	if ok and esit then
		self.beklenenler[k] = nil
		return true
	end

	-- Değer farklı: kullanıcı gerçekten değiştirmiş. Beklentiyi düşür ve gönder.
	self.beklenenler[k] = nil
	return false
end

return EchoGuard
