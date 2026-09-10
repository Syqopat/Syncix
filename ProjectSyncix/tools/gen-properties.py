# Generates the property table Syncix embeds in the Studio plugin, from Roblox's API dump.
#
# Why: property lists used to be written by hand and covered only 12 classes.
# Model, Humanoid, ParticleEmitter and everything else synced only structurally.
# This script filters the dump and produces the table embedded in the plugin.
#
# Usage:
#   V=$(curl -s https://setup.rbxcdn.com/versionQTStudio)
#   curl -s -o dump.json "https://setup.rbxcdn.com/$V-API-Dump.json"
#   python tools/gen-properties.py dump.json

import json, io, os, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DUMP = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "dump.json")

dump = json.load(io.open(DUMP, encoding="utf-8"))

# Types Syncix can carry.
PRIMITIVES = {"string", "bool", "int", "int64", "float", "double"}
# Keeping the scope to types the model can really carry prevents
# "sent, but lost on the other side". Before adding a type here, check that
# model.rs, PatchBuilder and PatchExecutor can all carry it.
DATA_TYPES = {
    "Vector3", "Vector2", "Color3", "UDim", "UDim2", "CFrame",
    "NumberRange", "BrickColor",
    # Asset references: MeshId, TextureID, SoundId, AnimationId, Image,
    # Decal.Texture, ShirtTemplate. While these were left out, a model dragged in
    # from the Toolbox showed up in the editor without anything saying what it looked like.
    "Content",
    # Particle/gradient colour and transparency curves: ParticleEmitter.Color,
    # Beam.Transparency, UIGradient.Color. Without them visual effects could not
    # be set up from the editor.
    "ColorSequence", "NumberSequence",
    # 9-slice UI (ImageLabel.SliceCenter) and fonts.
    "Rect", "Font",
    # Custom density/friction/elasticity.
    "PhysicalProperties",
}

# Classes that are not game content: Studio settings, engine internals.
# The Studio class even has quotes in its property names ("TODO" Color, for one).
CLASS_BLOCKLIST = {
    "Studio", "StudioService", "DebuggerManager", "LuaSettings",
    "NetworkSettings", "PhysicsSettings", "RenderSettings", "Stats",
    "GlobalSettings", "UserSettings", "AnalysticsSettings",
    "Terrain",
}

# Properties that are not synced: derived, noisy, or carried on a separate channel.
PROPERTY_BLOCKLIST = {
    "Parent", "Name", "ClassName",
    "Archivable", "RobloxLocked",
    "AssemblyLinearVelocity", "AssemblyAngularVelocity",
    "Velocity", "RotVelocity",
    "CenterOfMass", "AssemblyCenterOfMass", "AssemblyMass", "Mass",
    "ResizeIncrement", "ResizeableFaces",
    "Source",
    "Contents",
}


def type_code(member):
    if member.get("MemberType") != "Property":
        return None
    tags = set(member.get("Tags") or [])
    if tags & {"Deprecated", "ReadOnly", "NotScriptable", "Hidden"}:
        return None

    security = member.get("Security")
    if isinstance(security, dict):
        if security.get("Read") != "None" or security.get("Write") != "None":
            return None
    elif security not in (None, "None"):
        return None

    name = member["Name"]
    if name in PROPERTY_BLOCKLIST:
        return None
    # Defensive: properties with a quote or backslash in their name are ignored.
    if chr(34) in name or chr(92) in name:
        return None

    value_type = member.get("ValueType") or {}
    category, type_name = value_type.get("Category"), value_type.get("Name")

    if category == "Primitive" and type_name in PRIMITIVES:
        return "p"
    if category == "DataType" and type_name in DATA_TYPES:
        return "d"
    if category == "Enum":
        return "e"
    if category == "Class":
        return "r"
    return None


classes = {}
for cls in dump["Classes"]:
    if cls["Name"] in CLASS_BLOCKLIST:
        continue
    # NotCreatable classes are NOT dropped: abstract superclasses such as BasePart,
    # GuiObject and PVInstance cannot be created, but they carry their subclasses'
    # properties. Dropping them would cost Part the Anchored/Size/Color it gets from BasePart.
    tags = set(cls.get("Tags") or [])
    if "Deprecated" in tags:
        continue

    props = {}
    for member in cls.get("Members", []):
        code = type_code(member)
        if code:
            props[member["Name"]] = code

    # Classes without properties are written too, because the inheritance chain
    # passes through them. Part's superclass is FormFactorPart, which has no
    # eligible property of its own; skipping it breaks the chain there, and Part
    # loses every Anchored/Size/Color/Material it gets from BasePart.
    classes[cls["Name"]] = (cls.get("Superclass"), props)

# Inheritance: each class keeps only its OWN properties; superclass properties are
# merged at runtime. That is what keeps the table small.
rows = []
for name in sorted(classes):
    superclass, props = classes[name]
    body = ",".join('["%s"]="%s"' % (p, t) for p, t in sorted(props.items()))
    parent = ('"%s"' % superclass) if superclass and superclass != "<<<ROOT>>>" else "nil"
    rows.append('\t["%s"]={u=%s,p={%s}},' % (name, parent, body))

output = """-- AUTO-GENERATED - DO NOT EDIT BY HAND
--
-- Source    : Roblox Studio API dump (%s)
-- Generator : tools/gen-properties.py
--
-- Why it exists: property lists used to be written by hand and covered only 12
-- classes. Model, Humanoid, ParticleEmitter and everything else synced only
-- structurally; not a single setting went across.
--
-- Format: ["Class"] = { u = superclass, p = { ["Property"] = typeCode } }
--   p = primitive (string/bool/number)   d = data type (Vector3, CFrame, ...)
--   e = enum (as text)                   r = reference to another instance
--
-- Properties of superclasses are merged at runtime (see PatchBuilder).

return {
%s
}
""" % (dump.get("Version", "?"), "\n".join(rows))

target = os.path.join(ROOT, "studio-plugin", "src", "Observer", "PropertyTable.lua")
io.open(target, "w", encoding="utf-8").write(output)

print("classes: %d" % len(classes))
print("properties: %d" % sum(len(p) for _, p in classes.values()))
print("file: %.0f KB" % (os.path.getsize(target) / 1024.0))
