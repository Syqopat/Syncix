"""Motion: animations built from curves and sampled densely, so they move like a body.

A few keyframes that move every joint together look mechanical. Bodies move in
overlapping waves: the hips lead, the torso follows, the shoulders swing against the
hips, the head steadies itself, the hands trail behind. Motion describes each joint axis
as a function of the phase p (0..1 over the animation) — waves, smooth key curves, gentle
noise — lets one channel lag behind another, keeps the feet on the ground, and bakes it
all into an ordinary Animation (a key every 1/fps s) that exports and previews as usual.

    from motion import Motion, wave, keys, noise, lag
    m = Motion("Walk", rig, length=1.0, loop=True)
    m.pair("hip", (wave(30), 0, 0), offset=0.5)          # legs alternate: left half a cycle later
    m.pair("shoulder", (lag(wave(-24), 0.05), 0, 6), offset=0.5)
    m.set("waist", (-4, wave(8, phase=0.25), 0))
    m.plant()                                            # feet stay on the ground
    anim = m.bake()

Every channel value is a number or a function of p. Rotations are degrees in character
axes (see rig.py), moves are studs.
"""
import math
import random

from anim import Animation, ease
from cframe import CFrame

# Stepped look: poses held and snapped, a few per second (animation "on twos/threes"),
# the deliberately choppy old-Roblox style of games like asymmetric horror titles.
# None bakes smooth 30 fps keys. animkit.py's --fps sets it for a whole run.
STEP_FPS = 12


# ---------------------------------------------------------------------- curve builders
def const(v):
    return lambda p: v


def _fn(v):
    return v if callable(v) else const(float(v))


def wave(amp, phase=0.0, bias=0.0, freq=1):
    """bias + amp * sin(2π (freq·p − phase)). freq must be whole for a loop to close."""
    return lambda p: bias + amp * math.sin(2 * math.pi * (freq * p - phase))


def keys(points, loop=True):
    """A curve through (p, value[, ease]) points. The ease of a point shapes the segment
    that leaves it: "smooth" (Catmull-Rom, the default: arcs and follow-through for
    free), "linear", "in", "out", "inout", or "hold" (stays until the next point).
    Use "in" into a hit so it lands hard, "out" after it so it settles."""
    pts = sorted((float(p[0]), float(p[1]), p[2] if len(p) > 2 else "smooth") for p in points)
    if loop:
        ext = [(pts[-2][0] - 1, pts[-2][1], pts[-2][2]), (pts[-1][0] - 1, pts[-1][1], pts[-1][2])] + pts + \
              [(pts[0][0] + 1, pts[0][1], pts[0][2]), (pts[1][0] + 1, pts[1][1], pts[1][2])]
    else:
        ext = [(pts[0][0] - 1, pts[0][1], "smooth")] + pts + [(pts[-1][0] + 1, pts[-1][1], "smooth")]

    def f(p):
        if loop:
            p = p % 1.0
        else:
            if p <= pts[0][0]:
                return pts[0][1]
            if p >= pts[-1][0]:
                return pts[-1][1]
        for i in range(1, len(ext) - 2):
            (p1, v1, e1), (p2, v2, _) = ext[i], ext[i + 1]
            if p1 <= p <= p2:
                t = (p - p1) / (p2 - p1) if p2 > p1 else 1.0
                if e1 == "hold":
                    return v1
                if e1 != "smooth":
                    style, direction = ("Linear", "InOut") if e1 == "linear" else ("Cubic", {"in": "In", "out": "Out", "inout": "InOut"}[e1])
                    return v1 + (v2 - v1) * ease(style, direction, t)
                v0, v3 = ext[i - 1][1], ext[i + 2][1]
                return 0.5 * (2 * v1 + (-v0 + v2) * t + (2 * v0 - 5 * v1 + 4 * v2 - v3) * t * t
                              + (-v0 + 3 * v1 - 3 * v2 + v3) * t * t * t)
        return pts[-1][1]
    return f


def noise(amp, seed=0, detail=3):
    """Slow, organic wobble that still closes the loop (whole-number frequencies with
    random phases). Use a few degrees on heads, chests and idle hands."""
    rng = random.Random(seed)
    comps = [(k, rng.uniform(0, 2 * math.pi), rng.uniform(0.5, 1.0) / k) for k in range(1, detail + 1)]
    norm = sum(w for _, _, w in comps)
    return lambda p: amp * sum(w * math.sin(2 * math.pi * k * p + ph) for k, ph, w in comps) / norm


def lag(f, dp):
    """The same curve dp of the cycle later: overlap, a hand trailing its elbow."""
    f = _fn(f)
    return lambda p: f(p - dp)


def add(*fs):
    fs = [_fn(f) for f in fs]
    return lambda p: sum(f(p) for f in fs)


def scale(f, k):
    f = _fn(f)
    return lambda p: k * f(p)


def mirror_fn(axes):
    """Rotation channels for the other side of the body: y and z flip."""
    x, y, z = (_fn(a) for a in axes)
    return (x, lambda p: -y(p), lambda p: -z(p))


# ---------------------------------------------------------------------- Motion
class Motion:
    def __init__(self, name, rig, length, loop=False, priority="Action", fps=30):
        self.name, self.rig, self.length, self.loop, self.priority, self.fps = name, rig, float(length), loop, priority, fps
        self.channels = {}
        self.markers = []
        self.ground = None
        self.airborne = []

    def set(self, key, rot=(0, 0, 0), move=(0, 0, 0)):
        """Channels for one joint (role or joint name). A joint the rig lacks is skipped,
        except that a waist on a rig without one (R6) is added to the root."""
        self.channels[key] = ([_fn(a) for a in rot], [_fn(a) for a in move])
        return self

    def pair(self, role, rot, offset=0.0, move=(0, 0, 0)):
        """The right side gets `rot`; the left side the mirror of it, `offset` of the cycle
        later (0.5 for alternating legs and arms, 0 for both at once)."""
        self.set(f"{role}.R", rot, move)
        lx, ly, lz = mirror_fn(rot)
        # The move is shifted by the same offset as the rotation: a leg lifted as it swings
        # forward must lift when IT swings forward, not when the other one does.
        mx, my, mz = (_fn(a) for a in move)
        self.set(f"{role}.L", (lag(lx, offset), lag(ly, offset), lag(lz, offset)),
                 (lag(lambda p: -mx(p), offset), lag(my, offset), lag(mz, offset)))
        return self

    def plant(self, airborne=(), mode="plant"):
        """Keep the body on the ground: each frame the root is moved so the lowest point of
        the body sits where the feet do at rest. mode "plant" also pulls a floating body
        down (walks, idles); "push" only stops sinking (jumps). During `airborne` windows
        ((start, end) in p) only sinking is prevented, so a run can leave the ground."""
        self.ground, self.airborne = mode, list(airborne)
        return self

    def marker(self, p, name, value=""):
        self.markers.append((p, name, value))
        return self

    # ------------------------------------------------------------------ baking
    def _lowest(self, world):
        low = math.inf
        for name, cf in world.items():
            part = self.rig.parts.get(name)
            if not part or name == "HumanoidRootPart":
                continue
            sx, sy, sz = (v / 2 for v in part.size)
            for x in (-sx, sx):
                for y in (-sy, sy):
                    for z in (-sz, sz):
                        low = min(low, cf.point((x, y, z))[1])
        return low

    def _poses_at(self, p):
        rig = self.rig
        poses = {}
        waist_extra = None
        for key, (rot, move) in self.channels.items():
            r = tuple(f(p) for f in rot)
            m = tuple(f(p) for f in move)
            if rig.has(key):
                poses[rig.joint(key).name] = {"rot": r, "move": m}
            elif key == "waist":
                waist_extra = r  # R6: the torso turns and bends with the root
        root_name = rig.joint("root").name if rig.has("root") else None
        if waist_extra and root_name:
            cur = poses.get(root_name, {"rot": (0, 0, 0), "move": (0, 0, 0)})
            poses[root_name] = {"rot": tuple(a + b * 0.8 for a, b in zip(cur["rot"], waist_extra)), "move": cur["move"]}
        return poses, root_name

    def bake(self, step=None):
        """The sampled Animation. Smooth: a linear key every 1/fps s (the curves carry the
        easing). Stepped (step = poses per second, default STEP_FPS): a key every 1/step s
        with Constant easing, so each pose is held and the next one snaps in."""
        step = STEP_FPS if step is None else step
        a = Animation(self.name, self.rig, self.length, self.loop, self.priority)
        rest_low = self._lowest(self.rig.solve({}))
        rate = step if step else self.fps
        easing = "constant" if step else "linear"
        frames = max(2, round(self.length * rate))
        for i in range(frames + 1):
            p = i / frames
            if self.loop and i == frames:
                p = 0.0  # the last key repeats the first exactly
            poses, root_name = self._poses_at(p)
            if self.ground and root_name:
                transforms = {j: self.rig.to_transform(j, CFrame.degrees(*v["rot"]), v["move"]) for j, v in poses.items()}
                gap = rest_low - self._lowest(self.rig.solve(transforms))
                flying = any(s <= p <= e for s, e in self.airborne)
                if self.ground == "push" or flying:
                    gap = max(gap, 0.0)
                cur = poses.get(root_name, {"rot": (0, 0, 0), "move": (0, 0, 0)})
                poses[root_name] = {"rot": cur["rot"], "move": (cur["move"][0], cur["move"][1] + gap, cur["move"][2])}
            a.key(i / frames * self.length, poses, easing=easing)
        for p, name, value in self.markers:
            # Markers sit on the nearest baked key.
            t = round(p * frames) / frames * self.length
            a.marker(t, name, value)
        return a
