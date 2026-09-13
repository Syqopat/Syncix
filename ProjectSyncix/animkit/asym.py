"""ASYM locomotion: idle, walk, run, jump and fall in the low-FPS "blocky" style of
Roblox asymmetrical (asym) games.

The technique, as the Roblox developer community describes it: animate smoothly, then
keep one frame in three at 30 fps (10 poses a second) with Constant easing, so every pose
is held and the next snaps in. Fewer keys look blockier. The poses are pushed further
than a smooth animation would need, because each one stays on screen longer.

Built with Motion (curves, overlap, planted feet) and baked at FPS poses a second. Works
on R15 and R6 (R6: the waist goes to the root, no knees or elbows, feet kept down).
"""
from motion import Motion, add, keys, lag, noise, scale as scale_, wave

# 15 poses a second: "on twos" at 30 fps. Still visibly stepped, but less jerky than 10.
FPS = 15

# Cycle lengths are matched to the controller's speeds (walk 12, sprint 24 studs/s) and a
# Roblox leg of ~1.75 studs: a long cycle with small strides makes the feet slide.


# Poses per second per animation: slow motion needs few poses to read smoothly, fast
# motion needs more or the limbs jump too far between poses.
FPS_IDLE, FPS_WALK, FPS_RUN, FPS_JUMP, FPS_FALL = 10, 15, 20, 20, 12


def idle(rig):
    """Standing still and breathing. Only the breath moves: on the in-breath the shoulders
    rise (a small lift, not a rotation) and the chest comes up a touch; the body does not
    sway and the arms do not swing. (An earlier idle leaned forward and mixed sway, noise
    and a weight shift, so hips and head seemed to move on their own.)"""
    m = Motion("Idle", rig, 3.0, loop=True, priority="Idle")
    # Breath: in over 40% of the cycle, a short hold, then out.
    breath = keys([(0, 0), (0.4, 1), (0.52, 1), (1, 0)])
    m.pair("shoulder", (0, 0, 4), move=(0, scale_(breath, 0.07), 0), offset=0.0)
    m.pair("elbow", (8, 0, 0), offset=0.0)
    if rig.has("waist"):
        m.set("waist", (scale_(breath, 1.5), 0, 0), move=(0, scale_(breath, 0.02), 0))
    m.set("neck", (scale_(breath, -1.0), 0, 0), move=(0, scale_(breath, 0.02), 0))
    return m.bake(step=FPS_IDLE)


def walk(rig):
    """A relaxed walk: hips rolling, shoulders swinging against them, head level."""
    m = Motion("Walk", rig, 0.72, loop=True, priority="Movement")
    m.pair("hip", (add(3, wave(34)), 0, 2), offset=0.5)
    m.pair("knee", (keys([(0, -6), (0.15, -20), (0.35, -5), (0.55, -32), (0.72, -66), (0.9, -26)]), 0, 0), offset=0.5)
    m.pair("ankle", (keys([(0, -14), (0.2, 6), (0.45, 16), (0.6, -5), (0.8, -12)]), 0, 0), offset=0.5)
    m.pair("shoulder", (lag(wave(-34), 0.05), 0, 8), offset=0.5)
    m.pair("elbow", (add(20, lag(wave(12, freq=2), 0.1)), 0, 0), offset=0.5)
    m.set("root", rot=(0, wave(-8), wave(2.5, phase=0.25)))
    m.set("waist", (add(-4, wave(2, freq=2)), wave(12, phase=0.02), 0))
    m.set("neck", (3, wave(-5, phase=0.02), 0))
    m.plant()
    m.marker(0.0, "Step", "R").marker(0.5, "Step", "L")
    return m.bake(step=FPS_WALK)


def run(rig):
    """The sprint: leaning in, big strides, arms swinging wide and bent, feet leaving the ground."""
    m = Motion("Run", rig, 0.5, loop=True, priority="Movement")
    m.pair("hip", (keys([(0, 60), (0.13, 36), (0.3, -16), (0.45, -40), (0.62, 10), (0.82, 70)]), 0, 2), offset=0.5)
    m.pair("knee", (keys([(0, -16), (0.1, -32), (0.3, -46), (0.5, -105), (0.68, -125), (0.85, -60)]), 0, 0), offset=0.5)
    m.pair("ankle", (keys([(0, -20), (0.15, 10), (0.35, 28), (0.55, -10), (0.8, -28)]), 0, 0), offset=0.5)
    m.pair("shoulder", (lag(wave(-80, bias=10), 0.04), 0, lag(wave(10, bias=14, phase=0.25), 0.04)), offset=0.5)
    m.pair("elbow", (add(82, lag(wave(24), 0.08)), 0, 0), offset=0.5)
    # Upright: only a slight lean in the chest. (The root and waist leaning together,
    # -10 and -16, bent the whole body over and looked silly.)
    m.set("root", rot=(0, wave(-8), 0))
    m.set("waist", (add(-5, wave(2, freq=2, phase=0.2)), wave(14, phase=0.03), 0))
    m.set("neck", (add(3, wave(-2, freq=2)), wave(-6, phase=0.03), 0))
    m.plant(airborne=[(0.2, 0.42), (0.7, 0.92)])
    m.marker(0.0, "Step", "R").marker(0.5, "Step", "L")
    return m.bake(step=FPS_RUN)


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
    return m.bake(step=FPS_JUMP)


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
    return m.bake(step=FPS_FALL)


ASYM = {"idle": idle, "walk": walk, "run": run, "jump": jump, "fall": fall}
