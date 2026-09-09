# Roblox API dokumunden Syncix icin property tablosu uretir.
#
# Neden: property listeleri elle yaziliyordu ve yalnizca 12 sinifi kapsiyordu.
# Model, Humanoid, ParticleEmitter gibi her sey yalnizca yapisal olarak senkron
# oluyordu. Bu betik dokumu suzup eklentiye gomulecek tabloyu uretir.
#
# Kullanim:
#   V=$(curl -s https://setup.rbxcdn.com/versionQTStudio)
#   curl -s -o dump.json "https://setup.rbxcdn.com/$V-API-Dump.json"
#   python tools/gen-properties.py dump.json

import json, io, os, sys

KOK = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DUMP = sys.argv[1] if len(sys.argv) > 1 else os.path.join(KOK, "dump.json")

d = json.load(io.open(DUMP, encoding="utf-8"))

# Syncix'in tasiyabildigi tipler.
PRIMITIF = {"string", "bool", "int", "int64", "float", "double"}
# Kapsami modelin gercekten tasiyabildigi tiplerle sinirli tutmak,
# "gonderildi ama karsi tarafta kayboldu" durumunu onluyor. Buraya bir tip
# eklemeden once model.rs, PatchBuilder ve PatchExecutor'in onu tasiyabildigini
# dogrula.
VERI_TIPI = {
    "Vector3", "Vector2", "Color3", "UDim", "UDim2", "CFrame",
    "NumberRange", "BrickColor",
    # Asset referanslari: MeshId, TextureID, SoundId, AnimationId, Image,
    # Decal.Texture, ShirtTemplate. Bunlar disarida kaldigi surece Toolbox'tan
    # surukledigin bir model editorde neye benzedigi bilinmeden duruyordu.
    "Content",
    # Parcacik/gradyan renk ve seffaflik egrileri: ParticleEmitter.Color,
    # Beam.Transparency, UIGradient.Color. Bunlar olmadan gorsel efektler
    # editorden kurulamiyordu.
    "ColorSequence", "NumberSequence",
    # 9-slice UI (ImageLabel.SliceCenter) ve yazi tipi.
    "Rect", "Font",
    # Ozel yogunluk/surtunme/esneklik.
    "PhysicalProperties",
}

# Oyun icerigi olmayan siniflar: Studio ayarlari, motor ic yapilari.
# Studio sinifinin property adlarinda tirnak bile var ("TODO" Color gibi).
SINIF_KARA_LISTE = {
    "Studio", "StudioService", "DebuggerManager", "LuaSettings",
    "NetworkSettings", "PhysicsSettings", "RenderSettings", "Stats",
    "GlobalSettings", "UserSettings", "AnalysticsSettings",
    "Terrain",
}

# Senkron edilmeyen property'ler: turetilmis, gurultulu ya da ayri kanaldan giden.
KARA_LISTE = {
    "Parent", "Name", "ClassName",
    "Archivable", "RobloxLocked",
    "AssemblyLinearVelocity", "AssemblyAngularVelocity",
    "Velocity", "RotVelocity",
    "CenterOfMass", "AssemblyCenterOfMass", "AssemblyMass", "Mass",
    "ResizeIncrement", "ResizeableFaces",
    "Source",
    "Contents",
}


def uygun_mu(m):
    if m.get("MemberType") != "Property":
        return None
    etiketler = set(m.get("Tags") or [])
    if etiketler & {"Deprecated", "ReadOnly", "NotScriptable", "Hidden"}:
        return None

    guv = m.get("Security")
    if isinstance(guv, dict):
        if guv.get("Read") != "None" or guv.get("Write") != "None":
            return None
    elif guv not in (None, "None"):
        return None

    ad = m["Name"]
    if ad in KARA_LISTE:
        return None
    # Savunma: adinda tirnak ya da ters bolu olan property yok sayilir.
    if chr(34) in ad or chr(92) in ad:
        return None

    vt = m.get("ValueType") or {}
    kat, tip = vt.get("Category"), vt.get("Name")

    if kat == "Primitive" and tip in PRIMITIF:
        return "p"
    if kat == "DataType" and tip in VERI_TIPI:
        return "d"
    if kat == "Enum":
        return "e"
    if kat == "Class":
        return "r"
    return None


siniflar = {}
for c in d["Classes"]:
    if c["Name"] in SINIF_KARA_LISTE:
        continue
    # NotCreatable siniflar ELENMEZ: BasePart, GuiObject, PVInstance gibi soyut
    # ust siniflar olusturulamaz ama alt siniflarin property'lerini onlar tasir.
    # Elenirse Part, BasePart'tan gelen Anchored/Size/Color'i kaybeder.
    etiketler = set(c.get("Tags") or [])
    if "Deprecated" in etiketler:
        continue

    props = {}
    for m in c.get("Members", []):
        t = uygun_mu(m)
        if t:
            props[m["Name"]] = t

    # Property'si olmayan siniflar da yaziliyor. Sebep: kalitim zinciri onlarin
    # uzerinden geciyor. Part'in ust sinifi FormFactorPart, onun kendine ait
    # uygun property'si yok; atlanirsa zincir orada kirilir ve Part, BasePart'tan
    # gelen Anchored/Size/Color/Material'in hepsini kaybeder.
    siniflar[c["Name"]] = (c.get("Superclass"), props)

# Kalitim: her sinif yalnizca KENDI property'lerini tutar; ust siniftakiler
# calisma aninda birlestirilir. Tabloyu kucuk tutan sey bu.
satirlar = []
for ad in sorted(siniflar):
    ust, props = siniflar[ad]
    icerik = ",".join('["%s"]="%s"' % (p, t) for p, t in sorted(props.items()))
    ustu = ('"%s"' % ust) if ust and ust != "<<<ROOT>>>" else "nil"
    satirlar.append('\t["%s"]={u=%s,p={%s}},' % (ad, ustu, icerik))

cikti = """-- OTOMATIK URETILDI - ELLE DUZENLEMEYIN
--
-- Kaynak : Roblox Studio API dokumu (%s)
-- Ureten : tools/gen-properties.py
--
-- Neden var: property listeleri elle yaziliyordu ve yalnizca 12 sinifi
-- kapsiyordu. Model, Humanoid, ParticleEmitter gibi her sey yalnizca yapisal
-- olarak senkron oluyor, tek bir ayari bile gitmiyordu.
--
-- Bicim: ["Sinif"] = { u = ustSinif, p = { ["Property"] = tipKodu } }
--   p = ilkel (string/bool/sayi)   d = veri tipi (Vector3, CFrame, ...)
--   e = enum (metin olarak)        r = baska bir instance'a referans
--
-- Ust siniftaki property'ler calisma aninda birlestirilir (bkz. PatchBuilder).

return {
%s
}
""" % (d.get("Version", "?"), "\n".join(satirlar))

hedef = os.path.join(KOK, "studio-plugin", "src", "Observer", "PropertyTable.lua")
io.open(hedef, "w", encoding="utf-8").write(cikti)

print("sinif: %d" % len(siniflar))
print("property: %d" % sum(len(p) for _, p in siniflar.values()))
print("dosya: %.0f KB" % (os.path.getsize(hedef) / 1024.0))
