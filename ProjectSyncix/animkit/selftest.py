"""animkit's own checks, on synthetic R15 and R6 rigs (no Studio needed).

    python selftest.py [out_dir]

Builds both rigs (the R6 one with turned joint frames, as real R6 joints are), writes a
wave and a walk cycle for each, checks them, exports KeyframeSequence XML and Lua, and
draws previews. Exit code 0 when every check passes.
"""
import os
import sys
import xml.etree.ElementTree as ET

from anim import Animation, sym
from cframe import CFrame, rot_y
from preview import render
from rig import Joint, Part, Rig

OUT = sys.argv[1] if len(sys.argv) > 1 else "out"
failures = []


def check(cond, msg):
    print(("PASS " if cond else "FAIL ") + msg)
    if not cond:
        failures.append(msg)


def build(name, parts, joints):
    """parts: {name: (size, centre)}; joints: [(name, part0, part1, pivot, frame rotation)]."""
    cfs = {n: CFrame(c) for n, (_, c) in parts.items()}
    js = []
    for jn, p0, p1, pivot, rot in joints:
        J = CFrame(pivot) * rot
        js.append(Joint(jn, p0, p1, cfs[p0].inverse() * J, cfs[p1].inverse() * J))
    return Rig(name, [Part(n, s, cfs[n]) for n, (s, _) in parts.items()], js)


def r15():
    I = CFrame()
    parts = {"HumanoidRootPart": ((2, 2, 1), (0, 3.1, 0)), "LowerTorso": ((2, 0.4, 1), (0, 2.9, 0)),
             "UpperTorso": ((2, 1.6, 1), (0, 3.9, 0)), "Head": ((1.2, 1.2, 1.2), (0, 5.3, 0))}
    joints = [("Root", "HumanoidRootPart", "LowerTorso", (0, 2.9, 0), I),
              ("Waist", "LowerTorso", "UpperTorso", (0, 3.1, 0), I),
              ("Neck", "UpperTorso", "Head", (0, 4.7, 0), I)]
    for side, s in (("Right", 1), ("Left", -1)):
        parts.update({f"{side}UpperArm": ((1, 1.2, 1), (1.5 * s, 4.1, 0)), f"{side}LowerArm": ((1, 1.2, 1), (1.5 * s, 2.9, 0)),
                      f"{side}Hand": ((1, 0.3, 1), (1.5 * s, 2.15, 0)), f"{side}UpperLeg": ((1, 1.2, 1), (0.5 * s, 2.1, 0)),
                      f"{side}LowerLeg": ((1, 1.2, 1), (0.5 * s, 0.9, 0)), f"{side}Foot": ((1, 0.3, 1), (0.5 * s, 0.15, 0))})
        joints += [(f"{side}Shoulder", "UpperTorso", f"{side}UpperArm", (1.5 * s, 4.6, 0), I),
                   (f"{side}Elbow", f"{side}UpperArm", f"{side}LowerArm", (1.5 * s, 3.5, 0), I),
                   (f"{side}Wrist", f"{side}LowerArm", f"{side}Hand", (1.5 * s, 2.3, 0), I),
                   (f"{side}Hip", "LowerTorso", f"{side}UpperLeg", (0.5 * s, 2.7, 0), I),
                   (f"{side}Knee", f"{side}UpperLeg", f"{side}LowerLeg", (0.5 * s, 1.5, 0), I),
                   (f"{side}Ankle", f"{side}LowerLeg", f"{side}Foot", (0.5 * s, 0.3, 0), I)]
    return build("SynthR15", parts, joints)


def r6():
    parts = {"HumanoidRootPart": ((2, 2, 1), (0, 3, 0)), "Torso": ((2, 2, 1), (0, 3, 0)), "Head": ((2, 1, 1), (0, 4.5, 0)),
             "Right Arm": ((1, 2, 1), (1.5, 3, 0)), "Left Arm": ((1, 2, 1), (-1.5, 3, 0)),
             "Right Leg": ((1, 2, 1), (0.5, 1, 0)), "Left Leg": ((1, 2, 1), (-0.5, 1, 0))}
    turned = CFrame.from_components((0, 0, 0, -1, 0, 0, 0, 0, 1, 0, 1, 0))  # R6's RootJoint frame
    joints = [("RootJoint", "HumanoidRootPart", "Torso", (0, 3, 0), turned),
              ("Neck", "Torso", "Head", (0, 4, 0), turned),
              ("Right Shoulder", "Torso", "Right Arm", (1.5, 3.5, 0), rot_y(1.5708)),
              ("Left Shoulder", "Torso", "Left Arm", (-1.5, 3.5, 0), rot_y(-1.5708)),
              ("Right Hip", "Torso", "Right Leg", (0.5, 2, 0), rot_y(1.5708)),
              ("Left Hip", "Torso", "Left Leg", (-0.5, 2, 0), rot_y(-1.5708))]
    return build("SynthR6", parts, joints)


def wave(rig):
    a = Animation("Wave", rig, length=1.2, loop=True, priority="Action")
    a.key(0.0, {"shoulder.R": (0, 0, 15)})
    a.key(0.3, {"shoulder.R": (0, 0, 150), "elbow.R": (30, 0, 0)}, easing="cubic", direction="out")
    a.key(0.6, {"shoulder.R": (0, 0, 130), "elbow.R": (70, 0, 0)})
    a.key(0.9, {"shoulder.R": (0, 0, 150), "elbow.R": (30, 0, 0)})
    a.key(1.2, {"shoulder.R": (0, 0, 15), "elbow.R": (0, 0, 0)})
    return a


def walk(rig):
    a = Animation("Walk", rig, length=0.8, loop=True, priority="Movement")
    contact = {**sym("hip", (30, 0, 0), left=(-25, 0, 0)), **sym("shoulder", (-25, 0, 5), left=(25, 0, -5)),
               "knee.R": (-5, 0, 0), "knee.L": (-35, 0, 0), "root": {"move": (0, -0.1, 0)}}
    passing = {**sym("hip", (0, 0, 0)), **sym("shoulder", (0, 0, 5)), "knee.R": (-40, 0, 0),
               "knee.L": (-10, 0, 0), "root": {"move": (0, 0.1, 0)}}
    other = {**sym("hip", (-25, 0, 0), left=(30, 0, 0)), **sym("shoulder", (25, 0, 5), left=(-25, 0, -5)),
             "knee.R": (-35, 0, 0), "knee.L": (-5, 0, 0), "root": {"move": (0, -0.1, 0)}}
    a.key(0.0, contact).key(0.2, passing).key(0.4, other)
    a.key(0.6, {**passing, "knee.R": (-10, 0, 0), "knee.L": (-40, 0, 0)}).key(0.8, contact)
    a.marker(0.0, "Step", "R").marker(0.4, "Step", "L")
    return a


os.makedirs(OUT, exist_ok=True)
for rig in (r15(), r6()):
    # Saved so `animkit.py make <rig.json> all` can be tried without Studio.
    rig.save(os.path.join(OUT, f"{rig.name}.json"))
    check(Rig.load(os.path.join(OUT, f"{rig.name}.json")).rest_error() < 1e-6, f"{rig.name}: snapshot round trip")
    check(rig.rest_error() < 1e-6, f"{rig.name}: rest pose rebuilt from joints")
    check(rig.kind == ("R15" if rig.name.endswith("R15") else "R6"), f"{rig.name}: kind {rig.kind}")
    for anim in (wave(rig), walk(rig)):
        problems = anim.validate()
        check(not problems, f"{rig.name}/{anim.name}: validate -> {problems or 'ok'}")
        if rig.kind == "R6":
            check(any("has no" in n for n in anim.notes()), f"{rig.name}/{anim.name}: missing joints noted, not failed")
        path = os.path.join(OUT, f"{rig.name}_{anim.name}.rbxmx")
        anim.export_rbxmx(path)
        root = ET.parse(path).getroot()
        kfs = root.find("Item/Item")
        keyframes = kfs.findall("Item[@class='Keyframe']")
        check(kfs.get("class") == "KeyframeSequence" and len(keyframes) == len(anim.keys),
              f"{rig.name}/{anim.name}: {len(keyframes)} keyframes exported")
        first_pose = keyframes[0].find("Item[@class='Pose']/Properties/string[@name='Name']").text
        check(first_pose == "HumanoidRootPart", f"{rig.name}/{anim.name}: pose tree starts at {first_pose}")
        anim.export_lua(os.path.join(OUT, f"{rig.name}_{anim.name}.lua"))
        render(anim, os.path.join(OUT, f"{rig.name}_{anim.name}.png"), extra_per_gap=0)

# Built-in rigs: animations without capturing anything from Studio.
from rig import builtin  # noqa: E402

for kind in ("R15", "R6"):
    rig = Rig.load(kind)
    check(rig.kind == kind and rig.rest_error() < 1e-6, f"built-in {kind}: loads, rest pose consistent")
    if kind == "R6":
        # Roblox's R6 layout: arms beside the torso, legs under it, head on top.
        p = {n: tuple(round(v, 4) for v in part.cframe.p) for n, part in rig.parts.items()}
        check(p["Right Arm"] == (1.5, 3, 0) and p["Left Leg"] == (-0.5, 1, 0) and p["Head"] == (0, 4.5, 0),
              f"built-in R6 parts where Roblox puts them: {p['Right Arm']}, {p['Left Leg']}, {p['Head']}")
    w = wave(rig)
    check(not w.validate(), f"built-in {kind}: wave validates")

# Posing sanity on the real maths: a raised right arm ends up above the shoulder on both rigs.
for rig in (r15(), r6()):
    a = Animation("Raise", rig, 1)
    a.key(0, {"shoulder.R": (0, 0, 170)})
    hand = "RightHand" if "RightHand" in rig.parts else "Right Arm"
    y = rig.solve(a.sample(0))[hand].p[1]
    shoulder = rig.joint_frame_rest(rig.joint("shoulder.R")).p[1]
    check(y > shoulder + 0.3, f"{rig.name}: right arm raised above the shoulder (y {y:.2f} > {shoulder:.2f})")

bad = Animation("Bad", r15(), 1)
bad.key(0, {"elbow.R": (-60, 0, 0), "knee.L": (40, 0, 0)})
problems = bad.validate()
check(any("elbow.R" in p for p in problems) and any("knee.L" in p for p in problems),
      f"backwards elbow and knee are caught: {problems}")

print("\n" + ("ALL PASSED" if not failures else f"{len(failures)} FAILED"))
sys.exit(1 if failures else 0)
