"""ASYM locomotion: idle, walk, run, jump and fall in the low-FPS "blocky" style of
Roblox asymmetrical (asym) games.

The technique, as the Roblox developer community describes it: animate smoothly, then
keep one frame in three at 30 fps (10 poses a second) with Constant easing, so every pose
is held and the next snaps in. Fewer keys look blockier. The poses are pushed further
than a smooth animation would need, because each one stays on screen longer.

Built with Motion (curves, overlap, planted feet) and baked at FPS poses a second. Works
on R15 and R6 (R6: the waist goes to the root, no knees or elbows, feet kept down).
"""
import math

from motion import Motion, add, keys, lag, noise, scale as scale_, wave

# 15 poses a second: "on twos" at 30 fps. Still visibly stepped, but less jerky than 10.
FPS = 15

# Cycle lengths are matched to the controller's speeds (walk 12, sprint 24 studs/s) and a
# Roblox leg of ~1.75 studs: a long cycle with small strides makes the feet slide.


# Poses per second per animation: slow motion needs few poses to read smoothly, fast
# motion needs more or the limbs jump too far between poses.
# Rates measured from a reference asym movement set: the idle is almost a held pose
# (the reference changes pose twice a second), the walk steps at 12, the run at 20.
FPS_IDLE, FPS_WALK, FPS_RUN, FPS_JUMP, FPS_FALL = 8, 12, 20, 20, 12

# How one pose leads to the next. "constant" is the classic choppy look, but with few
# poses a fast arm jumps across the screen; "linear" keeps the same poses and rhythm and
# glides between them. Set back to "constant" for the fully stepped style.
BLEND = "linear"


def _no_knees(rig):
    return not rig.has("knee.R")


def _leg(rig):
    """(thigh, shin) in studs; for a leg without a knee (R6), (hip pivot to sole, 0)."""
    if _no_knees(rig):
        j = rig.joint("hip.R")
        leg = rig.parts[j.part1]
        return rig.joint_frame_rest(j).p[1] - (leg.cframe.p[1] - leg.size[1] / 2), 0.0
    from presets import leg_lengths
    return leg_lengths(rig)


def foot_path(stride, lift, stance, heel=1.0):
    """Where the foot is, relative to its hip, over one step cycle: (forward, up) in studs.

    The wheel of a walk: while the foot is on the ground (the first `stance` of the cycle)
    it stays planted, so relative to the hip it slides straight back from +stride in front
    to -stride behind while the body passes over it. Then it leaves the ground, comes up
    toward the body and swings forward along an arc to land in front again. heel < 1
    lifts it sooner after it leaves the ground (the heel coming up behind in a run)."""
    def f(p):
        p %= 1.0
        if p < stance:
            return stride * (1 - 2 * p / stance), 0.0
        t = (p - stance) / (1 - stance)
        s = t * t * (3 - 2 * t)
        return -stride + 2 * stride * s, lift * math.sin(math.pi * t ** heel)
    return f


def leg_cycle(m, rig, stride, lift, stance, heel=1.0, lean=0.0, inward=0.0):
    """Drives both legs from foot_path (left half a cycle after right) by solving the leg
    for the foot position every frame, instead of writing hip angles by hand. stride and
    lift are fractions of the leg's length, so every rig gets the same shape.

    With a knee (R15): two-bone IK, the knee bends forward, the ankle keeps the foot level.
    Without (R6): the leg turns toward the foot and slides up at the hip while it is in the
    air, which reads as a bent knee.

    lean: how far the root leans forward (degrees); the legs turn back by as much, so the
    path stays level with the ground and not with the tilted body. inward: degrees the leg
    turns toward the body's centre line, a little while planted and more while lifted, so
    the feet step close to one line instead of apart."""
    thigh, shin = _leg(rig)
    reach = thigh + shin
    path = foot_path(stride * reach, lift * reach, stance, heel)
    cache = {}

    def solve(p):
        key = round(p % 1.0, 5)
        if key not in cache:
            fwd, up = path(p)
            tuck = -inward * (0.5 + 0.5 * up / (lift * reach)) if lift else -inward
            if not shin:
                hip = math.degrees(math.asin(max(-0.95, min(0.95, fwd / thigh))))
                cache[key] = (hip + lean, 0.0, 0.0, up, tuck)
            else:
                ty = -reach * 0.97 + up                      # a straight leg would lock the knee
                d = min(math.hypot(fwd, ty), reach - 1e-3)
                alpha = math.atan2(fwd, -ty)                  # from straight down toward the foot
                beta = math.acos(max(-1.0, min(1.0, (thigh * thigh + d * d - shin * shin) / (2 * thigh * d))))
                inner = math.acos(max(-1.0, min(1.0, (thigh * thigh + shin * shin - d * d) / (2 * thigh * shin))))
                hip = math.degrees(alpha + beta) + lean
                knee = -math.degrees(math.pi - inner)
                cache[key] = (hip, knee, max(-40.0, min(40.0, lean - hip - knee)), 0.0, tuck)
        return cache[key]

    m.pair("hip", (lambda p: solve(p)[0], 0, lambda p: solve(p)[4]),
           move=(0, lambda p: solve(p)[3], 0), offset=0.5)
    if shin:
        m.pair("knee", (lambda p: solve(p)[1], 0, 0), offset=0.5)
        m.pair("ankle", (lambda p: solve(p)[2], 0, 0), offset=0.5)


def idle(rig):
    """A held, turned stance: the body angled away, the feet and the head back to the front,
    arms loose. Only the breath moves it: on the in-breath the shoulders rise (a lift, not a
    rotation). No sway, no swinging arms."""
    m = Motion("Idle", rig, 3.2, loop=True, priority="Idle")
    # Breath: in over 35% of the cycle, a short hold, out, then a moment of rest.
    breath = keys([(0, 0), (0.35, 1), (0.5, 1), (0.85, 0), (1, 0)])
    m.set("root", rot=(0, -18, 0))                           # the body stays put: no sway
    m.pair("hip", (-3, -16, 0), offset=0.0)                  # the legs turn back so the feet face ahead
    # The head never turns (a turn read as looking at something off to the side). It only
    # drifts with the breath: up a few degrees and a touch to the left, then back.
    m.set("neck", (add(-6, scale_(breath, 2.5)), 15, add(-5, scale_(breath, 1.5))),
          move=(0, scale_(breath, 0.03), 0))
    # The arms follow the chest: out a touch and bending a little on the in-breath.
    m.set("shoulder.R", (4, -12, add(3, scale_(breath, 2))), move=(0, scale_(breath, 0.1), 0))
    m.set("shoulder.L", (-6, 14, add(4, scale_(breath, 2))), move=(0, scale_(breath, 0.1), 0))
    m.pair("elbow", (add(10, scale_(breath, 5)), 0, 0), offset=0.0)
    if rig.has("waist"):
        m.set("waist", (scale_(breath, 2.5), 0, 0), move=(0, scale_(breath, 0.03), 0))
    return m.bake(step=FPS_IDLE, blend=BLEND)


def walk(rig):
    """A relaxed walk: hips rolling, shoulders swinging against them, head level."""
    m = Motion("Walk", rig, 1.1, loop=True, priority="Movement")
    # The feet follow a wheel: planted for 55% of the cycle (both down for a moment at each
    # step), then lifted toward the body and swung forward along an arc.
    leg_cycle(m, rig, stride=0.45, lift=0.18, stance=0.55, lean=3, inward=3)
    # Arms: a relaxed swing, turning inward as they come forward.
    m.pair("shoulder", (lag(wave(-20, bias=2), 0.05), lag(wave(-10), 0.05), 4), offset=0.5)
    m.pair("elbow", (add(15, lag(wave(6), 0.08)), 0, 0), offset=0.5)
    m.set("root", rot=(-3, wave(8), 0))
    m.set("neck", (-2, wave(-7), 0))                          # the head keeps looking ahead
    if rig.has("waist"):
        m.set("waist", (0, wave(-3), 0))
    m.plant()
    m.marker(0.0, "Step", "R").marker(0.5, "Step", "L")
    return m.bake(step=FPS_WALK, blend=BLEND)


def run(rig):
    """The sprint: leaning in, big strides, arms swinging wide and bent, feet leaving the ground."""
    m = Motion("Run", rig, 0.56, loop=True, priority="Movement")
    # A bigger, faster wheel: each foot is down for only 35% of the cycle (so both are in
    # the air twice per cycle), and it comes up high behind right after it leaves the ground.
    leg_cycle(m, rig, stride=0.6, lift=0.45, stance=0.35, heel=0.7, lean=14, inward=4)
    # Arms: pumped up in front, back behind, turning as they go.
    m.pair("shoulder", (keys([(0, -30), (0.5, 95)]), lag(wave(22), 0.05), lag(wave(4, bias=6), 0.05)), offset=0.5)
    m.pair("elbow", (add(70, lag(wave(20), 0.08)), 0, 0), offset=0.5)
    # The lean comes from the root alone. (An earlier run leaned the root AND the waist,
    # -10 and -16, and folded the body; a single lean keeps it one straight line.)
    m.set("root", rot=(-14, wave(-10), 0))
    m.set("neck", (8, wave(10), 0))                           # chin up, looking ahead
    if rig.has("waist"):
        m.set("waist", (0, wave(-4), 0))
    # Both feet are up between one foot leaving (0.35) and the other landing (0.5), and again
    # between 0.85 and 1.0.
    m.plant(airborne=[(0.35, 0.5), (0.85, 1.0)])
    m.marker(0.0, "Step", "R").marker(0.5, "Step", "L")
    return m.bake(step=FPS_RUN, blend=BLEND)


def jump(rig):
    """Take-off with the arms thrown up, then the knees tucked; held until the fall takes over."""
    m = Motion("Jump", rig, 0.4, loop=False, priority="Action")
    m.pair("shoulder", (keys([(0, 10), (0.3, 155, "out"), (1, 70)], loop=False), 0,
                        keys([(0, 8), (0.3, 25), (1, 40)], loop=False)), offset=0.0)
    m.pair("elbow", (keys([(0, 20), (0.3, 12), (1, 45)], loop=False), 0, 0), offset=0.0)
    m.set("hip.R", (keys([(0, 5), (0.3, -8), (1, 55)], loop=False), 0, 3))
    m.set("hip.L", (keys([(0, 5), (0.3, -12), (1, 30)], loop=False), 0, -3))
    m.set("knee.R", (keys([(0, -10), (0.3, -4), (1, -85)], loop=False), 0, 0))
    m.set("knee.L", (keys([(0, -10), (0.3, -6), (1, -50)], loop=False), 0, 0))
    m.pair("ankle", (keys([(0, 0), (0.3, -28), (1, -10)], loop=False), 0, 0), offset=0.0)
    m.set("waist", (keys([(0, -4), (0.3, 6), (1, -8)], loop=False), 0, 0))
    m.set("neck", (keys([(0, 0), (0.3, 10), (1, -4)], loop=False), 0, 0))
    m.plant(mode="push")
    return m.bake(step=FPS_JUMP, blend=BLEND)


def fall(rig):
    """Falling: arms up and out, flailing a little, legs bent and uneven."""
    m = Motion("Fall", rig, 0.6, loop=True, priority="Movement")
    m.pair("shoulder", (keys([(0, 20), (0.5, 35), (1, 20)]), 0, keys([(0, 100), (0.5, 120), (1, 100)])), offset=0.25)
    m.pair("elbow", (keys([(0, 30), (0.5, 15), (1, 30)]), 0, 0), offset=0.3)
    m.set("hip.R", (keys([(0, 25), (0.5, 35), (1, 25)]), 0, 5)).set("knee.R", (keys([(0, -35), (0.5, -50), (1, -35)]), 0, 0))
    m.set("hip.L", (keys([(0, 10), (0.5, 4), (1, 10)]), 0, -5)).set("knee.L", (keys([(0, -25), (0.5, -15), (1, -25)]), 0, 0))
    m.set("waist", (6, noise(4, seed=25), 0))
    m.set("neck", (-6, noise(5, seed=26), 0))
    m.plant(mode="push")
    return m.bake(step=FPS_FALL, blend=BLEND)


ASYM = {"idle": idle, "walk": walk, "run": run, "jump": jump, "fall": fall}
