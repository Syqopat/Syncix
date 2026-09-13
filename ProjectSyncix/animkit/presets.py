"""Ready-made animations for humanoid rigs (R15, R6, and custom rigs whose joints have
humanoid roles). Each preset is a function (rig, **options) -> Animation, written with the
twelve principles in mind: anticipation before a big move, a fast action, follow-through
and a slower recovery; arcs, not straight lines; something always slightly moving.

Axes (rig.py): +x swings a hanging limb forward (for the waist and neck: leans back);
+z moves a right limb outward (for the waist: leans left); +y turns to the left.
Elbows bend with +x, knees with -x. sym() mirrors a pose onto the left side.

Heights are derived from the rig: a crouch lowers the root by exactly what the bent legs
lose, so feet stay on the ground on any rig size. Rigs without knees (R6) keep straight
legs and do not sink.
"""
import math

from anim import Animation, sym


# ---------------------------------------------------------------------- rig measurements
def _dist(rig, a, b):
    pa = rig.joint_frame_rest(rig.joint(a)).p
    pb = rig.joint_frame_rest(rig.joint(b)).p
    return math.dist(pa, pb)


def leg_lengths(rig):
    """(thigh, shin) in studs, from joint pivots; (0, 0) when the rig has no knees."""
    if not (rig.has("hip.R") and rig.has("knee.R") and rig.has("ankle.R")):
        return 0.0, 0.0
    return _dist(rig, "hip.R", "knee.R"), _dist(rig, "knee.R", "ankle.R")


def crouch(rig, bend):
    """Hip, knee and ankle angles and root drop for a crouch with thighs `bend` degrees
    forward, feet flat and under the hips."""
    thigh, shin = leg_lengths(rig)
    if not thigh:
        return {"root": {"move": (0, 0, 0)}}
    a = math.radians(bend)
    drop = (thigh + shin) * (1 - math.cos(a))
    pose = {**sym("hip", (bend, 0, 0)), **sym("knee", (-2 * bend, 0, 0)), **sym("ankle", (bend, 0, 0))}
    pose["root"] = {"move": (0, -drop, 0)}
    return pose


def foot(hip, knee, share):
    """Ankle angle that keeps the foot roughly level (`share` of the leg's tilt undone),
    within the ankle's range."""
    return max(-40.0, min(40.0, -(hip + knee) * share))


def merge(*poses):
    out = {}
    for p in poses:
        out.update(p)
    return out


GUARD = {**sym("shoulder", (45, 0, 12)), **sym("elbow", (120, 0, 0)), "waist": (-5, 0, 0)}


# ---------------------------------------------------------------------- locomotion
def idle(rig, length=2.4):
    """Breathing and a slow sway; loops."""
    a = Animation("Idle", rig, length, loop=True, priority="Idle")
    rest = merge(sym("shoulder", (0, 0, 4)), sym("elbow", (6, 0, 0)), {"waist": (0, 0, 0), "neck": (0, 0, 0)},
                 {"root": {"move": (0, 0, 0)}})
    inhale = merge(sym("shoulder", (2, 0, 6)), sym("elbow", (9, 0, 0)), {"waist": (3, 2, 1), "neck": (-2, -2, 0)},
                   {"root": {"move": (0, -0.03, 0)}})
    a.key(0, rest).key(length / 2, inhale, easing="cubic").key(length, rest)
    return a


def walk(rig, length=0.9, stride=30):
    """A walk cycle; `stride` is the thigh swing in degrees. Markers "Step" on each contact."""
    a = Animation("Walk", rig, length, loop=True, priority="Movement")
    s, q = stride, length / 4

    def phase(lead, back, lead_knee, back_knee, bob, twist):
        return merge({"hip.R": (lead, 0, 0), "hip.L": (back, 0, 0), "knee.R": (lead_knee, 0, 0), "knee.L": (back_knee, 0, 0),
                      "ankle.R": (foot(lead, lead_knee, 0.6), 0, 0), "ankle.L": (foot(back, back_knee, 0.6), 0, 0),
                      "shoulder.R": (-lead * 0.8, 0, 5), "shoulder.L": (-back * 0.8, 0, -5),
                      "elbow.R": (15 + max(0, -lead) * 0.4, 0, 0), "elbow.L": (15 + max(0, -back) * 0.4, 0, 0),
                      "waist": (-3, twist, 0), "root": {"move": (0, bob, 0), "rot": (0, -twist * 0.5, 0)}})

    contact_r = phase(s, -s * 0.8, -5, -30, -0.08, 6)
    pass_r = phase(0, 0, -10, -45, 0.08, 0)
    contact_l = phase(-s * 0.8, s, -30, -5, -0.08, -6)
    pass_l = phase(0, 0, -45, -10, 0.08, 0)
    a.key(0, contact_r).key(q, pass_l).key(2 * q, contact_l).key(3 * q, pass_r).key(length, contact_r)
    a.marker(0, "Step", "R").marker(2 * q, "Step", "L")
    return a


def run(rig, length=0.6, stride=50):
    """A run cycle: bigger swings, bent arms, forward lean, more bounce."""
    a = Animation("Run", rig, length, loop=True, priority="Movement")
    s, q = stride, length / 4

    def phase(lead, back, lead_knee, back_knee, bob):
        return merge({"hip.R": (lead, 0, 0), "hip.L": (back, 0, 0), "knee.R": (lead_knee, 0, 0), "knee.L": (back_knee, 0, 0),
                      "ankle.R": (foot(lead, lead_knee, 0.5), 0, 0), "ankle.L": (foot(back, back_knee, 0.5), 0, 0),
                      "shoulder.R": (-lead * 1.0, 0, 8), "shoulder.L": (-back * 1.0, 0, -8),
                      "elbow.R": (85, 0, 0), "elbow.L": (85, 0, 0), "waist": (-14, 0, 0), "neck": (8, 0, 0),
                      "root": {"move": (0, bob, 0)}})

    contact_r = phase(s, -s * 0.6, -20, -70, -0.2)
    flight_l = phase(-s * 0.2, s * 0.6, -90, -40, 0.25)
    contact_l = phase(-s * 0.6, s, -70, -20, -0.2)
    flight_r = phase(s * 0.6, -s * 0.2, -40, -90, 0.25)
    a.key(0, contact_r).key(q, flight_l).key(2 * q, contact_l).key(3 * q, flight_r).key(length, contact_r)
    a.marker(0, "Step", "R").marker(2 * q, "Step", "L")
    return a


def jump(rig):
    """Anticipation crouch, take-off, tuck; ends in the fall pose. Marker "TakeOff"."""
    a = Animation("Jump", rig, 0.7, priority="Action")
    stand = merge(crouch(rig, 0), sym("shoulder", (0, 0, 4)))
    load = merge(crouch(rig, 40), sym("shoulder", (-35, 0, 8)), sym("elbow", (20, 0, 0)), {"waist": (-15, 0, 0)})
    push = merge(crouch(rig, 0), sym("shoulder", (150, 0, 15)), sym("elbow", (10, 0, 0)), {"waist": (5, 0, 0)},
                 sym("ankle", (-25, 0, 0)), {"root": {"move": (0, 0.3, 0)}})
    tuck = merge(sym("hip", (45, 0, 0)), sym("knee", (-70, 0, 0)), sym("shoulder", (60, 0, 35)), sym("elbow", (30, 0, 0)),
                 {"waist": (-5, 0, 0), "root": {"move": (0, 0.3, 0)}})
    a.key(0, stand).key(0.15, load, direction="out").key(0.28, push, direction="out").key(0.7, tuck)
    a.marker(0.28, "TakeOff")
    return a


def fall(rig, length=0.8):
    """Arms up and slightly flapping; loops while falling."""
    a = Animation("Fall", rig, length, loop=True, priority="Movement")
    up = merge(sym("shoulder", (20, 0, 80)), sym("elbow", (20, 0, 0)), sym("hip", (15, 0, 5)), sym("knee", (-25, 0, 0)))
    flap = merge(sym("shoulder", (25, 0, 95)), sym("elbow", (10, 0, 0)), sym("hip", (20, 0, 5)), sym("knee", (-35, 0, 0)))
    a.key(0, up).key(length / 2, flap).key(length, up)
    return a


def land(rig):
    """Impact crouch and recovery. Marker "Land" on the impact."""
    a = Animation("Land", rig, 0.45, priority="Action")
    a.key(0, merge(sym("hip", (15, 0, 0)), sym("knee", (-25, 0, 0)), sym("shoulder", (20, 0, 60))))
    a.key(0.08, merge(crouch(rig, 45), sym("shoulder", (35, 0, 30)), sym("elbow", (30, 0, 0)), {"waist": (-18, 0, 0)}), direction="out")
    a.key(0.45, merge(crouch(rig, 0), sym("shoulder", (0, 0, 4)), sym("elbow", (6, 0, 0)), {"waist": (0, 0, 0)}))
    a.marker(0.08, "Land")
    return a


# ---------------------------------------------------------------------- combat
def jab(rig, side="R"):
    """A quick straight punch from guard. Marker "Hit" when the arm is fully out."""
    o = "L" if side == "R" else "R"
    turn = 18 if side == "R" else -18
    a = Animation("Jab", rig, 0.45, priority="Action")
    windup = merge(GUARD, {f"shoulder.{side}": (35, 0, 15 if side == "R" else -15), "waist": (-5, -turn * 0.4, 0)})
    hit = merge(GUARD, {f"shoulder.{side}": (88, 0, 5 if side == "R" else -5), f"elbow.{side}": (5, 0, 0),
                        f"shoulder.{o}": (50, 0, 14 if o == "R" else -14), "waist": (-8, turn, 0), "neck": (0, -turn * 0.5, 0)})
    a.key(0, GUARD).key(0.07, windup, direction="in").key(0.15, hit, easing="linear").key(0.45, GUARD)
    a.marker(0.15, "Hit")
    return a


def uppercut(rig, side="R"):
    """Dip, then drive up from the legs. Marker "Hit"."""
    turn = 25 if side == "R" else -25
    a = Animation("Uppercut", rig, 0.65, priority="Action")
    dip = merge(GUARD, crouch(rig, 30), {f"shoulder.{side}": (5, 0, 10 if side == "R" else -10), f"elbow.{side}": (95, 0, 0),
                                         "waist": (-18, -turn * 0.5, 0)})
    hit = merge(GUARD, crouch(rig, 0), {f"shoulder.{side}": (150, 0, 5 if side == "R" else -5), f"elbow.{side}": (70, 0, 0),
                                         "waist": (12, turn, 0), "root": {"move": (0, 0.15, 0)}})
    a.key(0, GUARD).key(0.18, dip, direction="out").key(0.28, hit, easing="linear").key(0.65, merge(GUARD, crouch(rig, 0)))
    a.marker(0.28, "Hit")
    return a


def kick(rig, side="R"):
    """Front kick: chamber, extend, retract. Marker "Hit" at full extension."""
    o = "L" if side == "R" else "R"
    a = Animation("Kick", rig, 0.75, priority="Action")
    stand = merge(GUARD, crouch(rig, 0))
    chamber = merge(GUARD, {f"hip.{side}": (85, 0, 0), f"knee.{side}": (-110, 0, 0), f"hip.{o}": (5, 0, 0), f"knee.{o}": (-10, 0, 0),
                            "waist": (5, 0, 0)})
    hit = merge(GUARD, {f"hip.{side}": (90, 0, 0), f"knee.{side}": (-5, 0, 0), f"ankle.{side}": (-20, 0, 0),
                        f"hip.{o}": (5, 0, 0), f"knee.{o}": (-12, 0, 0), "waist": (18, 0, 0)})
    a.key(0, stand).key(0.18, chamber, direction="out").key(0.3, hit, easing="linear").key(0.45, chamber).key(0.75, stand)
    a.marker(0.3, "Hit")
    return a


def slash(rig, side="R"):
    """A diagonal sword swing with the right (or left) arm: wind-up over the shoulder,
    fast strike across the body, follow-through, recovery. Marker "Hit" mid-swing."""
    s = 1 if side == "R" else -1
    arm = f"shoulder.{side}"
    elbow = f"elbow.{side}"
    a = Animation("Slash", rig, 0.8, priority="Action")
    ready = {arm: (40, 0, 20 * s), elbow: (40, 0, 0), "waist": (0, 0, 0)}
    windup = {arm: (140, 0, 55 * s), elbow: (70, 0, 0), "waist": (6, -30 * s, 0), "neck": (0, 15 * s, 0)}
    strike = {arm: (70, 0, -15 * s), elbow: (10, 0, 0), "waist": (-12, 25 * s, 0), "neck": (0, -10 * s, 0)}
    through = {arm: (20, 0, -30 * s), elbow: (25, 0, 0), "waist": (-15, 35 * s, 0), "neck": (0, -15 * s, 0)}
    a.key(0, ready).key(0.22, windup, direction="out").key(0.34, strike, easing="linear").key(0.44, through, direction="out")
    a.key(0.8, merge(ready, {"neck": (0, 0, 0)}))
    a.marker(0.34, "Hit")
    return a


def block(rig, length=1.0):
    """Arms raised in front of the face, braced; loops while holding block."""
    a = Animation("Block", rig, length, loop=True, priority="Action")
    hold = merge(sym("shoulder", (80, 0, -10)), sym("elbow", (115, 0, 0)), crouch(rig, 12), {"waist": (-8, 0, 0), "neck": (-10, 0, 0)})
    brace = merge(hold, {"waist": (-10, 0, 0)}, crouch(rig, 15))
    a.key(0, hold).key(length / 2, brace).key(length, hold)
    return a


def hit_react(rig):
    """Taking a hit from the front: snap back, arms fling, recover."""
    a = Animation("HitReact", rig, 0.55, priority="Action2")
    rest = merge(sym("shoulder", (0, 0, 4)), {"waist": (0, 0, 0), "neck": (0, 0, 0)})
    snap = merge(sym("shoulder", (-20, 0, 35)), sym("elbow", (30, 0, 0)), {"waist": (22, 0, 0), "neck": (18, 0, 0)},
                 {"root": {"move": (0, 0, 0.3)}})
    a.key(0, rest).key(0.08, snap, direction="out").key(0.55, rest)
    return a


def death(rig):
    """Knees buckle, the body falls back onto the ground and stays."""
    a = Animation("Death", rig, 1.3, priority="Action4")
    thigh, shin = leg_lengths(rig)
    hip_height = rig.joint_frame_rest(rig.joint("root")).p[1] - min(p.cframe.p[1] - p.size[1] / 2 for p in rig.parts.values())
    a.key(0, merge(sym("shoulder", (0, 0, 4)), {"waist": (0, 0, 0), "root": {"move": (0, 0, 0)}}))
    a.key(0.35, merge(crouch(rig, 35), sym("shoulder", (10, 0, 20)), {"waist": (-15, 0, 0), "neck": (-20, 0, 0)}), direction="out")
    a.key(1.0, merge(sym("hip", (10, 0, 8)), sym("knee", (-20, 0, 0)), sym("shoulder", (20, 0, 60)), sym("elbow", (20, 0, 0)),
                     {"waist": (5, 0, 0), "neck": (10, 0, 0),
                      "root": {"rot": (85, 0, 0), "move": (0, -(hip_height - 0.6), 0.4)}}), easing="bounce", direction="out")
    a.key(1.3, merge(sym("hip", (10, 0, 8)), sym("knee", (-20, 0, 0)), sym("shoulder", (20, 0, 65)), sym("elbow", (15, 0, 0)),
                     {"waist": (5, 0, 0), "neck": (12, 0, 0),
                      "root": {"rot": (88, 0, 0), "move": (0, -(hip_height - 0.55), 0.4)}}))
    return a


# ---------------------------------------------------------------------- emotes
def wave(rig, side="R"):
    """A friendly wave; loops."""
    s = 1 if side == "R" else -1
    arm, elbow = f"shoulder.{side}", f"elbow.{side}"
    a = Animation("Wave", rig, 1.2, loop=True, priority="Action")
    a.key(0, {arm: (0, 0, 15 * s), elbow: (0, 0, 0), "neck": (0, 0, 0)})
    a.key(0.3, {arm: (10, 0, 150 * s), elbow: (30, 0, 0), "neck": (0, 8 * s, 0)}, direction="out")
    a.key(0.6, {arm: (10, 0, 135 * s), elbow: (70, 0, 0)})
    a.key(0.9, {arm: (10, 0, 150 * s), elbow: (30, 0, 0)})
    a.key(1.2, {arm: (0, 0, 15 * s), elbow: (0, 0, 0), "neck": (0, 0, 0)})
    return a


def cheer(rig, length=0.8):
    """Both fists pumping overhead with a little hop; loops."""
    a = Animation("Cheer", rig, length, loop=True, priority="Action")
    up = merge(sym("shoulder", (15, 0, 160)), sym("elbow", (20, 0, 0)), crouch(rig, 0), {"neck": (15, 0, 0), "root": {"move": (0, 0.25, 0)}})
    down = merge(sym("shoulder", (25, 0, 125)), sym("elbow", (70, 0, 0)), crouch(rig, 18), {"neck": (5, 0, 0)})
    a.key(0, down).key(length / 2, up, direction="out").key(length, down)
    return a


PRESETS = {
    "idle": idle, "walk": walk, "run": run, "jump": jump, "fall": fall, "land": land,
    "jab": jab, "uppercut": uppercut, "kick": kick, "slash": slash, "block": block,
    "hit_react": hit_react, "death": death, "wave": wave, "cheer": cheer,
}
