"""A character rig read from the place: parts, Motor6D joints, and how to pose them.

Any Motor6D rig works: R15, R6, or a custom creature. The rig is read from Studio through
Syncix (the running core's HTTP API, or the synced files on disk) and can be saved as a
small JSON snapshot, so animations can be written and previewed with Studio closed.

Poses are written in the CHARACTER's rest axes, not in each joint's own frame. The
character faces -Z; +X is its right, +Y up. A rotation about +X swings a hanging limb
forward, about +Z moves a right-side limb outward (a left-side one inward), about +Y
turns it to the character's left. to_transform() converts such a rotation into the
Motor6D.Transform / Pose.CFrame of that joint, whatever orientation its C0 has (R6
joints are turned oddly; custom rigs can be anything).
"""
import json
import os
import re
import urllib.parse
import urllib.request

from cframe import CFrame

# Joint roles shared by humanoid rigs. Presets use roles, so one walk cycle drives R15 and
# R6 alike; a role the rig lacks (R6 has no elbows or knees) is simply skipped.
ROLE_NAMES = {
    "root": ["root", "rootjoint"],
    "waist": ["waist"],
    "neck": ["neck"],
    "shoulder.R": ["rightshoulder"], "shoulder.L": ["leftshoulder"],
    "elbow.R": ["rightelbow"], "elbow.L": ["leftelbow"],
    "wrist.R": ["rightwrist"], "wrist.L": ["leftwrist"],
    "hip.R": ["righthip"], "hip.L": ["lefthip"],
    "knee.R": ["rightknee"], "knee.L": ["leftknee"],
    "ankle.R": ["rightankle"], "ankle.L": ["leftankle"],
}


def _norm(name):
    return re.sub(r"[\s_\-]", "", name).lower()


class Part:
    def __init__(self, name, size, cframe):
        self.name, self.size, self.cframe = name, tuple(size), cframe


class Joint:
    def __init__(self, name, part0, part1, c0, c1):
        self.name, self.part0, self.part1, self.c0, self.c1 = name, part0, part1, c0, c1


class Rig:
    def __init__(self, name, parts, joints, roles=None):
        self.name = name
        self.parts = {p.name: p for p in parts}
        self.joints = list(joints)
        self.by_part1 = {j.part1: j for j in self.joints}
        self.children = {}
        for j in self.joints:
            self.children.setdefault(j.part0, []).append(j)
        driven = {j.part1 for j in self.joints}
        roots = [p for p in self.children if p not in driven]
        if not roots:
            raise ValueError(f"{name}: no root part (every part is driven by a joint)")
        self.root = "HumanoidRootPart" if "HumanoidRootPart" in roots else roots[0]
        self.roles = roles or self._guess_roles()

    # ------------------------------------------------------------------ naming
    @property
    def kind(self):
        names = {_norm(j.name) for j in self.joints}
        if {"rightelbow", "leftknee", "waist"} <= names:
            return "R15"
        if {"rightshoulder", "lefthip", "neck"} <= names and "rightelbow" not in names:
            return "R6"
        return "custom"

    def _guess_roles(self):
        roles = {}
        for role, names in ROLE_NAMES.items():
            for j in self.joints:
                if _norm(j.name) in names:
                    roles[role] = j.name
                    break
        return roles

    def joint(self, key):
        """A joint by role ("elbow.R"), Motor6D name ("RightElbow", "Right Shoulder") or
        the name of the part it moves ("RightLowerArm"). Case and spaces are ignored."""
        if key in self.roles:
            key = self.roles[key]
        k = _norm(key)
        for j in self.joints:
            if _norm(j.name) == k or _norm(j.part1) == k:
                return j
        known = [j.name for j in self.joints] + list(self.roles)
        from difflib import get_close_matches
        near = get_close_matches(key, known, n=3)
        hint = f" Did you mean {', '.join(near)}?" if near else f" Joints: {', '.join(known)}"
        raise KeyError(f"{self.name} has no joint '{key}'.{hint}")

    def has(self, key):
        try:
            self.joint(key)
            return True
        except KeyError:
            return False

    # ------------------------------------------------------------------ posing
    def joint_frame_rest(self, j):
        """The joint's frame in the world at rest: Part0.CFrame * C0."""
        return self.parts[j.part0].cframe * j.c0

    def to_transform(self, key, rotation=None, offset=(0.0, 0.0, 0.0)):
        """The Pose.CFrame for a rotation (and a move, for the root) given in character axes."""
        j = self.joint(key)
        frame = self.joint_frame_rest(j).rotation()
        rot = rotation or CFrame()
        local = frame.inverse() * rot.rotation() * frame
        dx, dy, dz = frame.inverse().vector(offset)
        return CFrame((dx, dy, dz), local.r)

    def solve(self, transforms):
        """World CFrame of every part for {joint name: Transform}; missing joints rest."""
        world = {self.root: self.parts[self.root].cframe}
        stack = [self.root]
        while stack:
            part0 = stack.pop()
            for j in self.children.get(part0, []):
                t = transforms.get(j.name, CFrame())
                world[j.part1] = world[part0] * j.c0 * t * j.c1.inverse()
                stack.append(j.part1)
        return world

    def pose_tree(self):
        """(part name, [children]) from the root, the nesting a KeyframeSequence's Poses use."""
        def build(part):
            return (part, [build(j.part1) for j in self.children.get(part, [])])
        return build(self.root)

    def rest_error(self):
        """How far the rest pose computed from joints is from the parts' real positions.
        Near 0 means the rig was read correctly."""
        return self.rest_report()[0]

    def rest_report(self):
        """(error, part): the rest error and the part where it is largest."""
        solved = self.solve({})
        return max(
            (max(abs(a - b) for a, b in zip(solved[name].p, part.cframe.p)), name)
            for name, part in self.parts.items() if name in solved
        )

    # ------------------------------------------------------------------ snapshot
    def to_json(self):
        return {
            "name": self.name,
            "root": self.root,
            "roles": self.roles,
            "parts": [{"name": p.name, "size": p.size, "cframe": p.cframe.components()} for p in self.parts.values()],
            "joints": [{"name": j.name, "part0": j.part0, "part1": j.part1,
                        "c0": j.c0.components(), "c1": j.c1.components()} for j in self.joints],
        }

    def save(self, path):
        os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
        with open(path, "w", encoding="utf-8") as fh:
            json.dump(self.to_json(), fh, indent=1)

    @staticmethod
    def load(path):
        """A saved snapshot, or "R15" / "R6" for the built-in rigs."""
        if not os.path.exists(path) and path.upper() in ("R15", "R6"):
            return builtin(path)
        with open(path, encoding="utf-8") as fh:
            d = json.load(fh)
        parts = [Part(p["name"], p["size"], CFrame.from_components(p["cframe"])) for p in d["parts"]]
        joints = [Joint(j["name"], j["part0"], j["part1"], CFrame.from_components(j["c0"]),
                        CFrame.from_components(j["c1"])) for j in d["joints"]]
        return Rig(d["name"], parts, joints, d.get("roles"))


# ---------------------------------------------------------------------- built-in rigs
# Animations can be written without capturing a rig from Studio:
#  - R6 joints are the same in every place (Roblox builds each R6 character with these
#    C0/C1 values), so the R6 rig below is exact.
#  - R15 joint frames are not turned (C0/C1 are pure offsets), so an exported R15
#    animation depends only on the rotations you write; the proportions below are those
#    of a block rig and only change how the preview is drawn. Capture the real rig when
#    the preview must match a custom body exactly.
_TURN_ROOT = (-1, 0, 0, 0, 0, 1, 0, 1, 0)
_TURN_R = (0, 0, 1, 0, 1, 0, -1, 0, 0)
_TURN_L = (0, 0, -1, 0, 1, 0, 1, 0, 0)
_R6_JOINTS = [
    ("RootJoint", "HumanoidRootPart", "Torso", (0, 0, 0) + _TURN_ROOT, (0, 0, 0) + _TURN_ROOT),
    ("Neck", "Torso", "Head", (0, 1, 0) + _TURN_ROOT, (0, -0.5, 0) + _TURN_ROOT),
    ("Right Shoulder", "Torso", "Right Arm", (1, 0.5, 0) + _TURN_R, (-0.5, 0.5, 0) + _TURN_R),
    ("Left Shoulder", "Torso", "Left Arm", (-1, 0.5, 0) + _TURN_L, (0.5, 0.5, 0) + _TURN_L),
    ("Right Hip", "Torso", "Right Leg", (1, -1, 0) + _TURN_R, (0.5, 1, 0) + _TURN_R),
    ("Left Hip", "Torso", "Left Leg", (-1, -1, 0) + _TURN_L, (-0.5, 1, 0) + _TURN_L),
]
_R6_SIZES = {"HumanoidRootPart": (2, 2, 1), "Torso": (2, 2, 1), "Head": (2, 1, 1), "Right Arm": (1, 2, 1),
             "Left Arm": (1, 2, 1), "Right Leg": (1, 2, 1), "Left Leg": (1, 2, 1)}


def _r15_layout():
    """(parts {name: (size, centre)}, joints [(name, part0, part1, pivot)]) of a block R15."""
    parts = {"HumanoidRootPart": ((2, 2, 1), (0, 3.1, 0)), "LowerTorso": ((2, 0.4, 1), (0, 2.9, 0)),
             "UpperTorso": ((2, 1.6, 1), (0, 3.9, 0)), "Head": ((1.2, 1.2, 1.2), (0, 5.3, 0))}
    joints = [("Root", "HumanoidRootPart", "LowerTorso", (0, 2.9, 0)),
              ("Waist", "LowerTorso", "UpperTorso", (0, 3.1, 0)),
              ("Neck", "UpperTorso", "Head", (0, 4.7, 0))]
    for side, s in (("Right", 1), ("Left", -1)):
        parts.update({f"{side}UpperArm": ((1, 1.2, 1), (1.5 * s, 4.1, 0)), f"{side}LowerArm": ((1, 1.2, 1), (1.5 * s, 2.9, 0)),
                      f"{side}Hand": ((1, 0.3, 1), (1.5 * s, 2.15, 0)), f"{side}UpperLeg": ((1, 1.2, 1), (0.5 * s, 2.1, 0)),
                      f"{side}LowerLeg": ((1, 1.2, 1), (0.5 * s, 0.9, 0)), f"{side}Foot": ((1, 0.3, 1), (0.5 * s, 0.15, 0))})
        joints += [(f"{side}Shoulder", "UpperTorso", f"{side}UpperArm", (1.5 * s, 4.6, 0)),
                   (f"{side}Elbow", f"{side}UpperArm", f"{side}LowerArm", (1.5 * s, 3.5, 0)),
                   (f"{side}Wrist", f"{side}LowerArm", f"{side}Hand", (1.5 * s, 2.3, 0)),
                   (f"{side}Hip", "LowerTorso", f"{side}UpperLeg", (0.5 * s, 2.7, 0)),
                   (f"{side}Knee", f"{side}UpperLeg", f"{side}LowerLeg", (0.5 * s, 1.5, 0)),
                   (f"{side}Ankle", f"{side}LowerLeg", f"{side}Foot", (0.5 * s, 0.3, 0))]
    return parts, joints


def builtin(kind):
    """The built-in "R15" or "R6" rig (see the note above)."""
    kind = kind.upper()
    if kind == "R6":
        world = {"HumanoidRootPart": CFrame.new(0, 3, 0)}
        joints = []
        for name, p0, p1, c0, c1 in _R6_JOINTS:
            j = Joint(name, p0, p1, CFrame.from_components(c0), CFrame.from_components(c1))
            world[p1] = world[p0] * j.c0 * j.c1.inverse()
            joints.append(j)
        return Rig("R6", [Part(n, _R6_SIZES[n], world[n]) for n in _R6_SIZES], joints)
    if kind == "R15":
        parts, layout = _r15_layout()
        cfs = {n: CFrame(c) for n, (_, c) in parts.items()}
        joints = [Joint(name, p0, p1, cfs[p0].inverse() * CFrame(pivot), cfs[p1].inverse() * CFrame(pivot))
                  for name, p0, p1, pivot in layout]
        return Rig("R15", [Part(n, s, cfs[n]) for n, (s, _) in parts.items()], joints)
    raise KeyError(f"No built-in rig '{kind}'. Built-in: R15, R6")


# ---------------------------------------------------------------------- reading from Syncix
def _value(prop):
    """A Syncix property value as plain data: {"Vector3": {...}} -> (x, y, z), and so on."""
    if not isinstance(prop, dict) or len(prop) != 1:
        return prop
    kind, v = next(iter(prop.items()))
    if kind == "Vector3":
        return (v["x"], v["y"], v["z"])
    if kind == "CFrame":
        return CFrame(v["pos"], v["rot"])
    if kind == "Ref":
        return v if isinstance(v, str) else (v or {}).get("id")
    return v


def _get(port, path):
    with urllib.request.urlopen(f"http://127.0.0.1:{port}{path}", timeout=10) as r:
        return json.loads(r.read().decode("utf-8"))


def _build(name, nodes):
    """nodes: [(id, name, class, properties, parent id)] of everything inside the rig model.

    Two kinds of joint are read:
      Motor6D              Part0/Part1 and C0/C1, the classic rig.
      AnimationConstraint  Rig Builder's newer R15: no Motor6D at all. The joint is the
                           two Attachments it links; Attachment0's CFrame on its part is
                           the C0, Attachment1's on its part the C1, and its Transform
                           plays the same role, so the rest of animkit does not change.
    A part driven by a Motor6D ignores any AnimationConstraint on it, so no joint counts twice.
    """
    by_id = {i: (n, cls, props, parent) for i, n, cls, props, parent in nodes}

    def resolve(ref):
        if not ref:
            return None
        if ref in by_id:
            return ref
        return next((k for k in by_id if k.startswith(ref)), None)

    # A joint that cannot be read used to be skipped without a word; the capture then
    # looked fine but half the body hung loose. Every skip is reported with its reason.
    warnings = []
    parts, motors, constraints = [], [], []
    for i, n, cls, props, parent in nodes:
        if cls in ("Motor6D", "Motor"):
            p0, p1 = resolve(_value(props.get("Part0"))), resolve(_value(props.get("Part1")))
            missing = [k for k, ok in (("Part0", p0), ("Part1", p1), ("C0", "C0" in props), ("C1", "C1" in props)) if not ok]
            if missing:
                warnings.append(f"joint {n} ({cls}) skipped: no {', '.join(missing)}"
                                + (" inside the model" if {"Part0", "Part1"} & set(missing) else ""))
                continue
            motors.append(Joint(n, by_id[p0][0], by_id[p1][0], _value(props["C0"]), _value(props["C1"])))
        elif cls == "AnimationConstraint":
            a0, a1 = resolve(_value(props.get("Attachment0"))), resolve(_value(props.get("Attachment1")))
            if not (a0 and a1):
                warnings.append(f"joint {n} (AnimationConstraint) skipped: its attachments are not in the model")
                continue
            (_, _, p0props, p0parent), (_, _, p1props, p1parent) = by_id[a0], by_id[a1]
            if p0parent in by_id and p1parent in by_id and "CFrame" in p0props and "CFrame" in p1props:
                constraints.append(Joint(n, by_id[p0parent][0], by_id[p1parent][0],
                                         _value(p0props["CFrame"]), _value(p1props["CFrame"])))
            else:
                warnings.append(f"joint {n} (AnimationConstraint) skipped: an attachment has no part or CFrame")
        elif "Size" in props and "CFrame" in props:
            parts.append(Part(n, _value(props["Size"]), _value(props["CFrame"])))
    # Parts are known by name, so two parts with one name overwrite each other: a folder
    # holding several rigs (a pack of dummies) reads as one broken rig.
    seen = {}
    for p in parts:
        seen[p.name] = seen.get(p.name, 0) + 1
    twice = sorted(n for n, c in seen.items() if c > 1)
    if twice:
        warnings.append(f"several parts are named {', '.join(twice)}: this looks like more than one rig "
                        "(capture the rig model itself, not the folder holding it)")
    driven = {j.part1 for j in motors}
    joints = motors + [j for j in constraints if j.part1 not in driven]
    if not joints:
        raise ValueError(f"{name}: no Motor6D or AnimationConstraint joint was found"
                         + "".join(f"\n  - {w}" for w in warnings))
    rig = Rig(name, parts, joints)
    loose = sorted(p for p in rig.parts if p not in rig.by_part1 and p != rig.root)
    if loose:
        warnings.append(f"not joined to the rig (they will not move): {', '.join(loose)}")
    rig.warnings = warnings
    return rig


def from_syncix(target, port=8080):
    """Reads a rig model through the running Syncix core: `target` as the CLI takes it
    ("Workspace.Rig", a name, or a short id)."""
    root = _get(port, "/object?target=" + urllib.parse.quote(target))
    if "error" in root:
        raise KeyError(root["error"])
    tree = _get(port, "/tree")
    kids = {}
    for n in tree:
        kids.setdefault(n.get("parentId"), []).append(n)
    nodes, stack = [], [root["syncix_id"]]
    while stack:
        for n in kids.get(stack.pop(), []):
            stack.append(n["id"])
            obj = _get(port, "/object?target=" + n["id"])
            nodes.append((n["id"], n["name"], n["className"], obj.get("properties", {}), n.get("parentId")))
    return _build(root.get("name", target), nodes)


def from_folder(folder):
    """Reads a rig from its synced folder (src/Workspace/<Rig>), no running core needed."""
    nodes = []
    for dirpath, _, files in os.walk(folder):
        for f in files:
            if f.endswith(".json") and not f.endswith(".meta.json"):
                with open(os.path.join(dirpath, f), encoding="utf-8") as fh:
                    d = json.load(fh)
                if "syncix_id" in d:
                    nodes.append((d["syncix_id"], d["name"], d["class_name"], d.get("properties", {}), d.get("parent")))
    return _build(os.path.basename(os.path.normpath(folder)), nodes)
