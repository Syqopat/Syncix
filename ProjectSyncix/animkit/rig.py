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
        solved = self.solve({})
        return max(
            max(abs(a - b) for a, b in zip(solved[name].p, part.cframe.p))
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
        with open(path, encoding="utf-8") as fh:
            d = json.load(fh)
        parts = [Part(p["name"], p["size"], CFrame.from_components(p["cframe"])) for p in d["parts"]]
        joints = [Joint(j["name"], j["part0"], j["part1"], CFrame.from_components(j["c0"]),
                        CFrame.from_components(j["c1"])) for j in d["joints"]]
        return Rig(d["name"], parts, joints, d.get("roles"))


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
    """nodes: [(id, name, class, properties)] of everything inside the rig model."""
    names = {i: n for i, n, _, _ in nodes}
    parts, joints = [], []
    for i, n, cls, props in nodes:
        if cls in ("Motor6D", "Motor"):
            p0, p1 = _value(props.get("Part0")), _value(props.get("Part1"))
            p0 = names.get(p0) or next((v for k, v in names.items() if p0 and k.startswith(p0)), None)
            p1 = names.get(p1) or next((v for k, v in names.items() if p1 and k.startswith(p1)), None)
            if p0 and p1 and "C0" in props and "C1" in props:
                joints.append(Joint(n, p0, p1, _value(props["C0"]), _value(props["C1"])))
        elif "Size" in props and "CFrame" in props:
            parts.append(Part(n, _value(props["Size"]), _value(props["CFrame"])))
    if not joints:
        raise ValueError(f"{name}: no Motor6D with Part0, Part1, C0 and C1 was found")
    return Rig(name, parts, joints)


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
            nodes.append((n["id"], n["name"], n["className"], obj.get("properties", {})))
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
                    nodes.append((d["syncix_id"], d["name"], d["class_name"], d.get("properties", {})))
    return _build(os.path.basename(os.path.normpath(folder)), nodes)
