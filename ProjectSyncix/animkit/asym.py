"""ASYM locomotion: idle, walk, run, jump and fall in the low-FPS "blocky" style of
Roblox asymmetrical (asym) games.

The technique, as the Roblox developer community describes it: animate smoothly, then
keep one frame in three at 30 fps (10 poses a second) with Constant easing, so every pose
is held and the next snaps in. Fewer keys look blockier. The poses are pushed further
than a smooth animation would need, because each one stays on screen longer.

Built with Motion (curves, overlap, planted feet) and baked at FPS poses a second. Works
on R15 and R6 (R6: the waist goes to the root, no knees or elbows, feet kept down).
"""
from motion import Motion, add, keys, lag, noise, wave

FPS = 10


def idle(rig):
    """Weight on the right leg, slow breaths, arms loose, the head settling now and then."""
    m = Motion("Idle", rig, 2.4, loop=True, priority="Idle")
    breathe = wave(2.5, freq=2)
    m.set("root", rot=(0, -6, wave(1.2, phase=0.1)), move=(wave(0.05, phase=0.1), 0, 0))
    m.set("waist", (add(-4, breathe), add(5, noise(2, seed=21)), wave(-1, phase=0.1)))
    m.set("neck", (add(3, lag(breathe, 0.06)), add(-7, noise(4, seed=22)), noise(2, seed=23)))
    m.pair("shoulder", (add(4, lag(breathe, 0.08)), 0, add(8, noise(1.5, seed=24))), offset=0.12)
    m.pair("elbow", (add(16, lag(wave(5, freq=2), 0.12)), 0, 0), offset=0.12)
    m.set("hip.R", (6, 0, 5)).set("knee.R", (-10, 0, 0)).set("ankle.R", (4, 0, 0))
    m.set("hip.L", (-3, 0, -5)).set("knee.L", (-4, 0, 0))
    m.plant()
    return m.bake(step=FPS)


def walk(rig):
    """A relaxed walk: hips rolling, shoulders swinging against them, head level."""
    m = Motion("Walk", rig, 1.0, loop=True, priority="Movement")
    m.pair("hip", (add(3, wave(30)), 0, 2), offset=0.5)
    m.pair("knee", (keys([(0, -6), (0.15, -18), (0.35, -5), (0.55, -30), (0.72, -60), (0.9, -24)]), 0, 0), offset=0.5)
    m.pair("ankle", (keys([(0, -12), (0.2, 6), (0.45, 14), (0.6, -5), (0.8, -10)]), 0, 0), offset=0.5)
    m.pair("shoulder", (lag(wave(-26), 0.05), 0, 7), offset=0.5)
    m.pair("elbow", (add(18, lag(wave(10, freq=2), 0.1)), 0, 0), offset=0.5)
    m.set("root", rot=(0, wave(-7), wave(2, phase=0.25)))
    m.set("waist", (add(-3, wave(1.5, freq=2)), wave(10, phase=0.02), 0))
    m.set("neck", (3, wave(-4, phase=0.02), 0))
    m.plant()
    m.marker(0.0, "Step", "R").marker(0.5, "Step", "L")
    return m.bake(step=FPS)


def run(rig):
    """The sprint: leaning in, big strides, arms swinging wide and bent, feet leaving the ground."""
    m = Motion("Run", rig, 0.7, loop=True, priority="Movement")
    m.pair("hip", (keys([(0, 52), (0.13, 32), (0.3, -12), (0.45, -34), (0.62, 8), (0.82, 64)]), 0, 2), offset=0.5)
    m.pair("knee", (keys([(0, -16), (0.1, -30), (0.3, -42), (0.5, -100), (0.68, -120), (0.85, -58)]), 0, 0), offset=0.5)
    m.pair("ankle", (keys([(0, -18), (0.15, 10), (0.35, 26), (0.55, -10), (0.8, -26)]), 0, 0), offset=0.5)
    m.pair("shoulder", (lag(wave(-70, bias=8), 0.04), 0, lag(wave(10, bias=14, phase=0.25), 0.04)), offset=0.5)
    m.pair("elbow", (add(80, lag(wave(22), 0.08)), 0, 0), offset=0.5)
    m.set("root", rot=(-8, wave(-9), 0))
    m.set("waist", (add(-14, wave(3, freq=2, phase=0.2)), wave(14, phase=0.03), 0))
    m.set("neck", (add(12, wave(-2, freq=2)), wave(-6, phase=0.03), 0))
    m.plant(airborne=[(0.2, 0.42), (0.7, 0.92)])
    m.marker(0.0, "Step", "R").marker(0.5, "Step", "L")
    return m.bake(step=FPS)


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
    return m.bake(step=FPS)


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
    return m.bake(step=FPS)


ASYM = {"idle": idle, "walk": walk, "run": run, "jump": jump, "fall": fall}
