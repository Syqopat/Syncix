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
# Rates measured from a reference asym movement set: the idle is almost a held pose
# (the reference changes pose twice a second), the walk steps at 12, the run at 20.
FPS_IDLE, FPS_WALK, FPS_RUN, FPS_JUMP, FPS_FALL = 5, 12, 20, 20, 12


def _no_knees(rig):
    return not rig.has("knee.R")


def idle(rig):
    """A held, turned stance: the body angled away, the feet and the head back to the front,
    arms loose. Only the breath moves it: on the in-breath the shoulders rise (a lift, not a
    rotation). No sway, no swinging arms."""
    m = Motion("Idle", rig, 2.0, loop=True, priority="Idle")
    breath = keys([(0, 0), (0.4, 1), (0.55, 1), (1, 0)])
    m.set("root", rot=(0, -18, 0))
    m.pair("hip", (-3, -16, 0), offset=0.0)                 # the legs turn back so the feet face ahead
    m.set("neck", (add(-6, scale_(breath, -1.0)), 15, -5), move=(0, scale_(breath, 0.02), 0))  # slight head tilt
    m.set("shoulder.R", (4, -12, 3), move=(0, scale_(breath, 0.07), 0))
    m.set("shoulder.L", (-6, 14, 4), move=(0, scale_(breath, 0.07), 0))
    m.pair("elbow", (10, 0, 0), offset=0.0)
    if rig.has("waist"):
        m.set("waist", (scale_(breath, 1.5), 0, 0), move=(0, scale_(breath, 0.02), 0))
    return m.bake(step=FPS_IDLE)


def walk(rig):
    """A relaxed walk: hips rolling, shoulders swinging against them, head level."""
    m = Motion("Walk", rig, 1.1, loop=True, priority="Movement")
    # Legs swing forward ~26 and back ~34. Without knees (R6) the leg is also slid at the
    # hip, forward and up while it swings through, which reads as a bent knee.
    m.pair("hip", (wave(30, bias=-4), wave(4, phase=0.25), 0),
           move=(0, keys([(0, 0.26), (0.25, 0.06), (0.5, 0), (0.75, 0.06)]) if _no_knees(rig) else 0,
                 wave(-0.4) if _no_knees(rig) else 0), offset=0.5)
    m.pair("knee", (keys([(0, -30), (0.25, -8), (0.5, -6), (0.75, -20)]), 0, 0), offset=0.5)
    m.pair("ankle", (keys([(0, 5), (0.25, -8), (0.5, 10), (0.75, 4)]), 0, 0), offset=0.5)
    # Arms: a relaxed swing, turning inward as they come forward.
    m.pair("shoulder", (lag(wave(-20, bias=2), 0.05), lag(wave(-10), 0.05), 4), offset=0.5)
    m.pair("elbow", (add(15, lag(wave(6), 0.08)), 0, 0), offset=0.5)
    m.set("root", rot=(-3, wave(8), 0))
    m.set("neck", (-2, wave(-7), 0))                          # the head keeps looking ahead
    if rig.has("waist"):
        m.set("waist", (0, wave(-3), 0))
    m.plant()
    m.marker(0.0, "Step", "R").marker(0.5, "Step", "L")
    return m.bake(step=FPS_WALK)


def run(rig):
    """The sprint: leaning in, big strides, arms swinging wide and bent, feet leaving the ground."""
    m = Motion("Run", rig, 0.56, loop=True, priority="Movement")
    # A cartoon sprint: the leg reaches ~38 forward and kicks up to ~75 behind. Without
    # knees (R6) it is slid at the hip, forward and a little up in front, up behind, so the
    # kick reads as a heel coming up. (A bigger lift in front read as a kick.)
    no_knees = _no_knees(rig)
    # With knees (R15) the heel comes up from the knee, so the thigh only goes ~35 back.
    kick = -75 if no_knees else -35
    m.pair("hip", (keys([(0, 38), (0.2, 5), (0.5, kick), (0.75, -8)]), wave(4, phase=0.25), 0),
           move=(0, keys([(0, 0.55), (0.2, 0.12), (0.5, 0.45), (0.75, 0.15)]) if no_knees else 0,
                 keys([(0, -0.65), (0.5, 0.7)]) if no_knees else 0), offset=0.5)
    m.pair("knee", (keys([(0, -20), (0.25, -40), (0.5, -115), (0.75, -70)]), 0, 0), offset=0.5)
    m.pair("ankle", (keys([(0, -15), (0.25, 15), (0.5, 20), (0.75, -20)]), 0, 0), offset=0.5)
    # Arms: pumped up in front, back behind, turning as they go.
    m.pair("shoulder", (keys([(0, -30), (0.5, 95)]), lag(wave(22), 0.05), lag(wave(4, bias=6), 0.05)), offset=0.5)
    m.pair("elbow", (add(70, lag(wave(20), 0.08)), 0, 0), offset=0.5)
    # The lean comes from the root alone. (An earlier run leaned the root AND the waist,
    # -10 and -16, and folded the body; a single lean keeps it one straight line.)
    m.set("root", rot=(-14, wave(-10), 0))
    m.set("neck", (8, wave(10), 0))                           # chin up, looking ahead
    if rig.has("waist"):
        m.set("waist", (0, wave(-4), 0))
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
