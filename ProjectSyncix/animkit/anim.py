"""Animations written as keyframes in character axes, exported for Roblox.

    from anim import Animation, sym
    a = Animation("Wave", rig, length=1.2, loop=True, priority="Action")
    a.key(0.0, {"shoulder.R": (0, 0, 20)})
    a.key(0.3, {"shoulder.R": (0, 0, 150), "elbow.R": (40, 0, 0)}, easing="cubic")
    a.key(1.2, {"shoulder.R": (0, 0, 20)})
    a.export_rbxmx("out/Wave.rbxmx")     # KeyframeSequence for Studio
    a.export_lua("out/Wave.lua")         # data for AnimKitPlayer (no upload needed)

A value is (x, y, z) degrees about the character's axes (see rig.py): +x swings a hanging
limb forward, +z moves a right limb outward, +y turns towards the character's left.
The root also takes {"rot": (x, y, z), "move": (dx, dy, dz)} in studs.
sym("shoulder", (40, 0, 10)) keys both sides, the left one mirrored.
"""
import math

from cframe import CFrame, lerp

EASING_STYLES = {"linear": "Linear", "constant": "Constant", "cubic": "Cubic", "elastic": "Elastic", "bounce": "Bounce"}
EASING_DIRECTIONS = {"in": "In", "out": "Out", "inout": "InOut"}
PRIORITIES = ["Core", "Idle", "Movement", "Action", "Action2", "Action3", "Action4"]

# Rough human limits in character axes, (min, max) degrees per axis, for the checks in
# validate(). They catch the classic AI mistakes: elbows and knees bent backwards, heads
# turned 180 degrees. Mirrored for the left side.
LIMITS = {
    "elbow": {"x": (-5, 150), "y": (-30, 30), "z": (-20, 20)},
    "knee": {"x": (-150, 5), "y": (-10, 10), "z": (-10, 10)},
    # For the neck and waist -x bends forward (a chin to the chest, a bow), +x back.
    "neck": {"x": (-60, 50), "y": (-80, 80), "z": (-40, 40)},
    "waist": {"x": (-75, 40), "y": (-60, 60), "z": (-35, 35)},
    "shoulder": {"x": (-80, 180), "y": (-90, 90), "z": (-40, 180)},
    "hip": {"x": (-40, 120), "y": (-45, 45), "z": (-15, 70)},
    "wrist": {"x": (-80, 80), "y": (-30, 30), "z": (-60, 60)},
    "ankle": {"x": (-45, 45), "y": (-20, 20), "z": (-20, 20)},
}


def mirror(value):
    """The same pose on the other side of the body: rotations about Y and Z flip."""
    if isinstance(value, dict):
        out = dict(value)
        if "rot" in out:
            out["rot"] = mirror(out["rot"])
        if "move" in out:
            dx, dy, dz = out["move"]
            out["move"] = (-dx, dy, dz)
        return out
    x, y, z = value
    return (x, -y, -z)


def sym(role, value, left=None):
    """{"<role>.R": value, "<role>.L": mirrored value}; pass left= to override that side."""
    return {f"{role}.R": value, f"{role}.L": left if left is not None else mirror(value)}


def ease(style, direction, t):
    if style == "Constant":
        return 0.0 if t < 1 else 1.0
    if style == "Linear":
        return t
    if style == "Bounce" or style == "Elastic":  # previews approximate both with a cubic out
        return 1 - (1 - t) ** 3
    if direction == "In":
        return t ** 3
    if direction == "Out":
        return 1 - (1 - t) ** 3
    return 4 * t ** 3 if t < 0.5 else 1 - (-2 * t + 2) ** 3 / 2


class Key:
    def __init__(self, time, poses, easing, direction):
        self.time, self.poses, self.easing, self.direction = time, poses, easing, direction


class Animation:
    def __init__(self, name, rig, length, loop=False, priority="Action"):
        if priority not in PRIORITIES:
            raise ValueError(f"priority must be one of {PRIORITIES}")
        self.name, self.rig, self.length, self.loop, self.priority = name, rig, float(length), loop, priority
        self.keys = []
        self.markers = []
        self.skipped = set()

    # ------------------------------------------------------------------ writing
    def key(self, time, poses=None, easing="cubic", direction="inout", **named):
        """A keyframe at `time` seconds. poses: {role or joint name: value}. Joints the rig
        lacks (an elbow on R6) are skipped and listed by validate()."""
        style = EASING_STYLES[easing.lower()]
        dirn = EASING_DIRECTIONS[direction.lower()]
        merged = dict(poses or {})
        merged.update(named)
        resolved = {}
        for key, value in merged.items():
            if not self.rig.has(key):
                self.skipped.add(key)
                continue
            joint = self.rig.joint(key)
            rot, move = (value.get("rot", (0, 0, 0)), value.get("move", (0, 0, 0))) if isinstance(value, dict) else (value, (0, 0, 0))
            resolved[joint.name] = {"role": self._role_of(joint.name), "deg": tuple(rot), "move": tuple(move),
                                    "transform": self.rig.to_transform(joint.name, CFrame.degrees(*rot), move)}
        self.keys = [k for k in self.keys if abs(k.time - time) > 1e-6]
        self.keys.append(Key(float(time), resolved, style, dirn))
        self.keys.sort(key=lambda k: k.time)
        return self

    def marker(self, time, name, value=""):
        """A KeyframeMarker: scripts hear it through AnimationTrack:GetMarkerReachedSignal(name).
        Put one on the frame a punch lands ("Hit"), a foot touches ("Step"), a sword swings."""
        self.markers.append((float(time), name, value))
        return self

    def _role_of(self, joint_name):
        for role, name in self.rig.roles.items():
            if name == joint_name:
                return role
        return None

    # ------------------------------------------------------------------ sampling
    def tracks(self):
        """{joint name: [(time, key, pose)]}; a joint keyed later but not at 0 starts at rest."""
        out = {}
        for k in self.keys:
            for joint, pose in k.poses.items():
                out.setdefault(joint, []).append((k.time, k, pose))
        for joint, track in out.items():
            if track[0][0] > 1e-6:
                rest = {"deg": (0, 0, 0), "move": (0, 0, 0), "transform": CFrame(), "role": self._role_of(joint)}
                track.insert(0, (0.0, Key(0.0, {}, "Cubic", "InOut"), rest))
        return out

    def sample(self, t):
        """{joint: Transform} at time t, eased like Roblox: a pose's easing leads to the next."""
        result = {}
        for joint, track in self.tracks().items():
            if t <= track[0][0]:
                result[joint] = track[0][2]["transform"]
                continue
            for (t0, k0, p0), (t1, _, p1) in zip(track, track[1:]):
                if t0 <= t <= t1:
                    alpha = ease(k0.easing, k0.direction, (t - t0) / (t1 - t0) if t1 > t0 else 1.0)
                    result[joint] = lerp(p0["transform"], p1["transform"], alpha)
                    break
            else:
                result[joint] = track[-1][2]["transform"]
        return result

    # ------------------------------------------------------------------ checks
    def notes(self):
        """Things worth knowing that are not mistakes: joints this rig does not have (an R6
        rig has no elbows, knees or waist), so those keys were left out."""
        if not self.skipped:
            return []
        return [f"{self.rig.kind} rig has no {', '.join(sorted(self.skipped))}; those keys were left out"]

    def validate(self):
        """Problems worth fixing before anyone sees the animation; [] when none."""
        problems = []
        if not self.keys:
            return ["no keyframes"]
        # One line per (role, axis) with the worst value: baked animations have a key every
        # frame, and a line per frame would bury the message.
        worst = {}
        for k in self.keys:
            if k.time < -1e-6 or k.time > self.length + 1e-6:
                problems.append(f"key at {k.time:.2f}s is outside 0..{self.length:.2f}s")
            for joint, pose in k.poses.items():
                role = pose["role"]
                if not role:
                    continue
                base, side = (role.split(".") + [None])[:2]
                limits = LIMITS.get(base)
                if not limits:
                    continue
                deg = pose["deg"] if side != "L" else mirror(pose["deg"])
                for axis, value in zip("xyz", deg):
                    lo, hi = limits[axis]
                    over = max(lo - value, value - hi)
                    if over > 0.5 and over > worst.get((role, axis), (0,))[0]:
                        worst[(role, axis)] = (over, k.time, value, lo, hi)
        for (role, axis), (_, t, value, lo, hi) in sorted(worst.items()):
            problems.append(f"{t:.2f}s {role} {axis}={value:.0f} is outside {lo}..{hi} degrees")
        if self.loop:
            first, last = self.sample(0.0), self.sample(self.length)
            for joint in first:
                if not first[joint].is_close(last.get(joint, CFrame()), 1e-3):
                    problems.append(f"loop does not close: {joint} differs between 0s and {self.length:.2f}s")
        return problems

    # ------------------------------------------------------------------ export
    def _pose_xml(self, node, key, indent, ref):
        part, children = node
        joint = self.rig.by_part1.get(part)
        pose = key.poses.get(joint.name) if joint else None
        inner = [self._pose_xml(c, key, indent + 1, ref) for c in children]
        inner = [x for x in inner if x]
        if pose is None and not inner:
            return ""
        ref[0] += 1
        pad = "\t" * indent
        cf = pose["transform"] if pose else CFrame()
        comps = cf.components()
        names = ["X", "Y", "Z"] + [f"R{i // 3}{i % 3}" for i in range(9)]
        cf_xml = "".join(f"<{n}>{v:.6g}</{n}>" for n, v in zip(names, comps))
        weight = 1 if pose else 0  # a Pose only there to hold its children leaves the joint alone
        return "\n".join([
            f'{pad}<Item class="Pose" referent="RBX{ref[0]}">',
            f"{pad}\t<Properties>",
            f'{pad}\t\t<string name="Name">{part}</string>',
            f'{pad}\t\t<CoordinateFrame name="CFrame">{cf_xml}</CoordinateFrame>',
            f'{pad}\t\t<string name="EasingStyle">Enum.PoseEasingStyle.{key.easing}</string>',
            f'{pad}\t\t<string name="EasingDirection">Enum.PoseEasingDirection.{key.direction}</string>',
            f'{pad}\t\t<float name="Weight">{weight}</float>',
            f"{pad}\t</Properties>",
            *inner,
            f"{pad}</Item>",
        ])

    def to_rbxmx_item(self, indent=1, ref=None):
        ref = ref or [0]
        ref[0] += 1
        pad = "\t" * indent
        out = [f'{pad}<Item class="KeyframeSequence" referent="RBX{ref[0]}">', f"{pad}\t<Properties>",
               f'{pad}\t\t<string name="Name">{self.name}</string>',
               f'{pad}\t\t<bool name="Loop">{"true" if self.loop else "false"}</bool>',
               f'{pad}\t\t<string name="Priority">Enum.AnimationPriority.{self.priority}</string>',
               f"{pad}\t</Properties>"]
        tree = self.rig.pose_tree()
        for k in self.keys:
            ref[0] += 1
            out += [f'{pad}\t<Item class="Keyframe" referent="RBX{ref[0]}">', f"{pad}\t\t<Properties>",
                    f'{pad}\t\t\t<string name="Name">Keyframe</string>',
                    f'{pad}\t\t\t<float name="Time">{k.time:.4g}</float>', f"{pad}\t\t</Properties>"]
            pose = self._pose_xml(tree, k, indent + 2, ref)
            if pose:
                out.append(pose)
            for t, name, value in self.markers:
                if abs(t - k.time) < 1e-6:
                    ref[0] += 1
                    out += [f'{pad}\t\t<Item class="KeyframeMarker" referent="RBX{ref[0]}">',
                            f'{pad}\t\t\t<Properties><string name="Name">{name}</string>'
                            f'<string name="Value">{value}</string></Properties>',
                            f"{pad}\t\t</Item>"]
            out.append(f"{pad}\t</Item>")
        out.append(f"{pad}</Item>")
        missing = [t for t, _, _ in self.markers if not any(abs(t - k.time) < 1e-6 for k in self.keys)]
        if missing:
            raise ValueError(f"{self.name}: markers must sit on a keyframe; add a key at {missing}")
        return "\n".join(out)

    def export_rbxmx(self, path, folder="Animations"):
        """One KeyframeSequence inside a Folder, ready for `syncix import`."""
        write_rbxmx(path, [self], folder)

    def export_lua(self, path):
        """A ModuleScript with the keyframes as CFrame data, played by AnimKitPlayer
        without uploading anything. Joint names are the Motor6D names."""
        lines = [f"-- Generated by animkit from {self.name}. Edit the generator, not this file.",
                 "return {", f'\tName = "{self.name}",', f"\tLength = {self.length:g},",
                 f"\tLoop = {'true' if self.loop else 'false'},", f'\tPriority = "{self.priority}",',
                 "\tMarkers = {"]
        lines += [f'\t\t{{ Time = {t:g}, Name = "{n}", Value = "{v}" }},' for t, n, v in self.markers]
        lines += ["\t},", "\tTracks = {"]
        for joint, track in self.tracks().items():
            lines.append(f'\t\t["{joint}"] = {{')
            for t, k, pose in track:
                comps = ", ".join(f"{v:.5g}" for v in pose["transform"].components())
                lines.append(f'\t\t\t{{ Time = {t:g}, Easing = "{k.easing}", Direction = "{k.direction}", CFrame = CFrame.new({comps}) }},')
            lines.append("\t\t},")
        lines += ["\t},", "}", ""]
        with open(path, "w", encoding="utf-8") as fh:
            fh.write("\n".join(lines))


def write_rbxmx(path, animations, folder="Animations"):
    """Several KeyframeSequences in one Folder, one import."""
    import os
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    ref = [1]
    items = "\n".join(a.to_rbxmx_item(2, ref) for a in animations)
    body = (f'<roblox version="4">\n\t<Item class="Folder" referent="RBX1">\n\t\t<Properties>'
            f'<string name="Name">{folder}</string></Properties>\n{items}\n\t</Item>\n</roblox>\n')
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(body)
    return path


def frames_for(length, fps=30):
    """Times of every frame, for code that wants to sample the whole animation."""
    return [i / fps for i in range(int(math.floor(length * fps)) + 1)]
