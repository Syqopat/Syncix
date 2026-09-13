"""An asymmetric-horror animation set (survivors and a killer), written for this kit and
inspired by the genre's conventions: the survivor is tense and alert, the killer heavy
and menacing, explosive when it attacks. Original work; no animation data from any game.

Every animation is a Motion (curves, overlap, planted feet), so it reads as a body with
weight rather than a puppet. Works on R15 and R6 (R6 has no waist, knees or elbows: the
waist goes to the root, knees and elbows are left out, the ground lock keeps the feet down).

Timing notes, in the spirit of the style:
  - idles are long (3 s) and never symmetric, so the loop is hard to spot
  - a survivor's sprint is fast (0.56 s) with a hard forward lean and pumping arms
  - the killer's walk is slow (1.4 s) with a heavy downward bob; its chase leans in
  - the killer's swing: long wind-up (~30%), a strike of two or three frames, a big
    follow-through and a slow recovery, so the hit is readable and dodgeable
"""
from motion import Motion, add, keys, lag, noise, wave


# ================================================================= survivors
def survivor_idle(rig):
    """Tense, alert: weight shifting, quick breaths, glancing around, hands ready."""
    m = Motion("SurvivorIdle", rig, 3.2, loop=True, priority="Idle")
    breathe = wave(2.2, freq=3)
    m.set("root", rot=(0, add(noise(3, seed=1), 0), wave(1.5, phase=0.1)), move=(wave(0.06, phase=0.1), 0, 0))
    m.set("waist", (add(-6, breathe), noise(5, seed=2), wave(-1.5, phase=0.1)))
    # Head: small glances with holds, then a quick look back over the shoulder.
    m.set("neck", (add(4, lag(breathe, 0.03)),
                   keys([(0, 0), (0.18, 0, "hold"), (0.26, 28, "out"), (0.42, 28, "hold"), (0.5, -8, "out"),
                         (0.7, -8, "hold"), (0.78, -35, "out"), (0.88, -35, "hold"), (0.95, 0)]),
                   noise(3, seed=3)))
    m.pair("shoulder", (add(8, lag(breathe, 0.05)), 0, add(9, noise(2, seed=4))), offset=0.13)
    m.pair("elbow", (add(28, noise(4, seed=5)), 0, 0), offset=0.21)
    m.pair("wrist", (add(-5, noise(4, seed=6)), 0, 0), offset=0.3)
    m.set("hip.R", (8, 0, 3)).set("hip.L", (4, 0, -6))
    m.set("knee.R", (-12, 0, 0)).set("knee.L", (-6, 0, 0))
    m.plant()
    return m.bake()


def survivor_walk(rig):
    """A careful walk: slight crouch and lean, hips rolling, shoulders countering, head steady."""
    m = Motion("SurvivorWalk", rig, 1.0, loop=True, priority="Movement")
    step = wave(28, phase=0.0)
    m.pair("hip", (add(4, step), 0, 2), offset=0.5)
    m.pair("knee", (keys([(0, -8), (0.15, -18), (0.35, -6), (0.55, -30), (0.72, -58), (0.9, -25)]), 0, 0), offset=0.5)
    m.pair("ankle", (keys([(0, -12), (0.2, 6), (0.45, 14), (0.6, -4), (0.8, -10)]), 0, 0), offset=0.5)
    m.pair("shoulder", (lag(wave(-20), 0.06), 0, 7), offset=0.5)
    m.pair("elbow", (add(22, lag(wave(8, freq=2), 0.1)), 0, 0), offset=0.5)
    m.set("root", rot=(0, wave(-6, phase=0.0), wave(2, phase=0.25)))
    m.set("waist", (add(-7, wave(1.5, freq=2)), wave(9, phase=0.02), 0))
    m.set("neck", (5, wave(-3, phase=0.02), 0))  # counters the chest so the eyes stay level
    m.plant()
    m.marker(0.0, "Step", "R").marker(0.5, "Step", "L")
    return m.bake()


def survivor_sprint(rig):
    """Panic sprint: hard lean, knees high, arms pumping, feet leaving the ground."""
    m = Motion("SurvivorSprint", rig, 0.56, loop=True, priority="Movement")
    m.pair("hip", (keys([(0, 48), (0.12, 30), (0.3, -12), (0.45, -30), (0.62, 10), (0.82, 62)]), 0, 1), offset=0.5)
    m.pair("knee", (keys([(0, -18), (0.1, -30), (0.3, -40), (0.5, -95), (0.68, -118), (0.85, -60)]), 0, 0), offset=0.5)
    m.pair("ankle", (keys([(0, -18), (0.15, 10), (0.35, 25), (0.55, -10), (0.8, -25)]), 0, 0), offset=0.5)
    m.pair("shoulder", (lag(wave(-58, bias=6), 0.04), 0, 9), offset=0.5)
    m.pair("elbow", (add(92, lag(wave(18), 0.08)), 0, 0), offset=0.5)
    m.set("root", rot=(-10, wave(-8), 0))
    m.set("waist", (add(-16, wave(3, freq=2, phase=0.2)), wave(12, phase=0.03), 0))
    m.set("neck", (add(14, wave(-2, freq=2)), wave(-5, phase=0.03), 0))
    m.plant(airborne=[(0.2, 0.42), (0.7, 0.92)])
    m.marker(0.0, "Step", "R").marker(0.5, "Step", "L")
    return m.bake()


def survivor_exhausted(rig):
    """Out of stamina: bent over, hands on the knees, heaving breaths."""
    m = Motion("SurvivorExhausted", rig, 2.2, loop=True, priority="Movement")
    heave = wave(5, freq=3)
    m.set("waist", (add(-40, heave), noise(3, seed=7), 0))
    m.set("neck", (add(22, lag(heave, 0.04)), noise(6, seed=8), 0))
    m.pair("shoulder", (add(55, lag(heave, 0.06)), 0, -6), offset=0.0)
    m.pair("elbow", (add(25, lag(scale_(heave, 0.5), 0.1)), 0, 0), offset=0.0)
    m.pair("hip", (38, 0, 6), offset=0.0)
    m.pair("knee", (-55, 0, 0), offset=0.0)
    m.pair("ankle", (20, 0, 0), offset=0.0)
    m.set("root", rot=(add(-8, lag(scale_(heave, 0.4), 0.02)), 0, 0))
    m.plant()
    return m.bake()


def scale_(f, k):
    return lambda p: k * f(p)


def survivor_injured_walk(rig):
    """Limping on a hurt right leg, left hand holding the side, head down."""
    m = Motion("SurvivorInjured", rig, 1.3, loop=True, priority="Movement")
    # The right leg barely bends and is on the ground for less of the cycle.
    m.set("hip.R", (keys([(0, 18), (0.3, -6), (0.45, -14), (0.7, 4), (0.85, 20)]), 0, 4))
    m.set("knee.R", (keys([(0, -6), (0.45, -8), (0.7, -22), (0.9, -8)]), 0, 0))
    m.set("hip.L", (keys([(0, -18), (0.2, -8), (0.4, 24), (0.6, 30), (0.85, -10)]), 0, -3))
    m.set("knee.L", (keys([(0, -40), (0.25, -60), (0.45, -15), (0.7, -10), (0.9, -30)]), 0, 0))
    m.set("root", rot=(0, keys([(0, -4), (0.45, 6), (0.9, -4)]), keys([(0, 3), (0.45, -5), (0.9, 3)])))
    m.set("waist", (add(-14, keys([(0, -2), (0.4, 4), (0.8, -2)])), 0, keys([(0, -8), (0.45, 4), (0.9, -8)])))
    m.set("neck", (-12, noise(4, seed=9), 5))
    m.set("shoulder.L", (30, 0, -38)).set("elbow.L", (100, 0, 0))            # hand pressed to the right side
    m.set("shoulder.R", (lag(wave(-10), 0.1), 0, 16)).set("elbow.R", (18, 0, 0))
    m.plant()
    return m.bake()


def survivor_hit(rig):
    """Hit from the front: snap back off balance, arms flung, stumble, recover."""
    m = Motion("SurvivorHit", rig, 0.8, loop=False, priority="Action2")
    back = keys([(0, 0), (0.1, 26, "out"), (0.35, 12), (0.6, -6), (1, 0)], loop=False)
    m.set("root", rot=(lag(back, 0.02), keys([(0, 0), (0.12, -14), (1, 0)], loop=False), 0),
          move=(0, 0, keys([(0, 0), (0.12, 0.7, "out"), (0.5, 0.9), (1, 0.9)], loop=False)))
    m.set("waist", (back, 0, keys([(0, 0), (0.15, 8), (1, 0)], loop=False)))
    m.set("neck", (lag(back, 0.05), 0, 0))
    m.pair("shoulder", (keys([(0, 5), (0.12, -30, "out"), (0.4, 20), (1, 5)], loop=False), 0,
                        keys([(0, 8), (0.12, 45, "out"), (0.45, 15), (1, 8)], loop=False)), offset=0.03)
    m.pair("elbow", (keys([(0, 20), (0.15, 50), (0.5, 15), (1, 20)], loop=False), 0, 0), offset=0.05)
    m.pair("knee", (keys([(0, -5), (0.2, -30), (0.5, -15), (1, -5)], loop=False), 0, 0), offset=0.1)
    m.plant(mode="push")
    return m.bake()


def survivor_taunt(rig):
    """"Come on then": beckoning with both hands, head tilting, a bounce in the knees."""
    m = Motion("SurvivorTaunt", rig, 2.0, loop=True, priority="Action")
    beck = keys([(0, 30), (0.12, 90, "in"), (0.25, 30), (0.5, 30), (0.62, 90, "in"), (0.75, 30)])
    m.pair("shoulder", (60, 0, 20), offset=0.0)
    m.pair("elbow", (beck, 0, 0), offset=0.04)
    m.set("neck", (wave(-5, freq=2), wave(10, freq=1), wave(12, freq=1, phase=0.1)))
    m.set("waist", (add(6, wave(3, freq=2)), wave(8), 0))
    m.pair("knee", (add(-16, wave(10, freq=2)), 0, 0), offset=0.0)
    m.pair("hip", (add(8, wave(5, freq=2)), 0, 6), offset=0.0)
    m.plant()
    return m.bake()


# ================================================================= killer
def killer_idle(rig):
    """Menacing: slow deep breaths, hunched, head slowly tilting, weapon hand heavy."""
    m = Motion("KillerIdle", rig, 3.6, loop=True, priority="Idle")
    breathe = wave(3, freq=2)
    m.set("waist", (add(-14, breathe), noise(6, seed=10), 0))
    m.set("neck", (add(6, lag(breathe, 0.05)), keys([(0, -10), (0.3, 18), (0.55, 18, "hold"), (0.7, -5), (1, -10)]),
                   keys([(0, 0), (0.3, 14), (0.6, 10), (0.8, -6), (1, 0)])))
    m.set("shoulder.R", (add(12, lag(breathe, 0.08)), 0, 18)).set("elbow.R", (35, 0, 0))   # weapon arm
    m.set("shoulder.L", (add(4, lag(noise(6, seed=11), 0.1)), 0, -14)).set("elbow.L", (add(20, noise(10, seed=12)), 0, 0))
    m.set("root", rot=(0, noise(4, seed=13), wave(2)), move=(wave(0.08), 0, 0))
    m.set("hip.R", (6, 0, 10)).set("hip.L", (-4, 0, -10))
    m.set("knee.R", (-14, 0, 0)).set("knee.L", (-8, 0, 0))
    m.plant()
    return m.bake()


def killer_walk(rig):
    """Slow and heavy: long strides, a deep bob, the weapon arm swinging with its weight."""
    m = Motion("KillerWalk", rig, 1.4, loop=True, priority="Movement")
    m.pair("hip", (add(2, wave(32)), 0, 3), offset=0.5)
    m.pair("knee", (keys([(0, -6), (0.12, -24), (0.35, -8), (0.55, -30), (0.75, -55), (0.92, -20)]), 0, 0), offset=0.5)
    m.pair("ankle", (keys([(0, -10), (0.2, 8), (0.45, 12), (0.65, -6), (0.85, -8)]), 0, 0), offset=0.5)
    m.set("shoulder.R", (lag(wave(-14, bias=10), 0.1), 0, 18)).set("elbow.R", (add(30, lag(wave(10), 0.18)), 0, 0))
    m.set("shoulder.L", (lag(wave(18), 0.08), 0, -10)).set("elbow.L", (add(22, lag(wave(-8), 0.15)), 0, 0))
    m.set("root", rot=(0, wave(-7), wave(4, phase=0.25)))
    m.set("waist", (add(-12, wave(2, freq=2, phase=0.1)), wave(10, phase=0.03), wave(-2, phase=0.25)))
    m.set("neck", (add(8, wave(-2, freq=2, phase=0.1)), wave(-6, phase=0.03), 0))
    m.plant()
    m.marker(0.0, "Step", "R").marker(0.5, "Step", "L")
    return m.bake()


def killer_chase(rig):
    """The chase: leaning in, weapon cocked back, the free hand reaching ahead."""
    m = Motion("KillerChase", rig, 0.7, loop=True, priority="Movement")
    m.pair("hip", (keys([(0, 50), (0.15, 28), (0.35, -14), (0.5, -28), (0.68, 12), (0.85, 60)]), 0, 2), offset=0.5)
    m.pair("knee", (keys([(0, -20), (0.12, -32), (0.32, -38), (0.52, -90), (0.7, -112), (0.87, -55)]), 0, 0), offset=0.5)
    m.pair("ankle", (keys([(0, -15), (0.18, 10), (0.38, 22), (0.58, -8), (0.8, -22)]), 0, 0), offset=0.5)
    m.set("shoulder.R", (add(-25, lag(wave(12), 0.1)), 0, 28)).set("elbow.R", (add(95, lag(wave(10), 0.15)), 0, 0))
    m.set("shoulder.L", (add(35, lag(wave(35), 0.05)), 0, -8)).set("elbow.L", (add(35, lag(wave(-15), 0.12)), 0, 0))
    m.set("root", rot=(-14, wave(-10), 0))
    m.set("waist", (add(-18, wave(3, freq=2)), wave(14, phase=0.03), 0))
    m.set("neck", (add(22, wave(-3, freq=2)), wave(-8, phase=0.03), 0))
    m.plant(airborne=[(0.22, 0.4), (0.72, 0.9)])
    m.marker(0.0, "Step", "R").marker(0.5, "Step", "L")
    return m.bake()


def killer_swing(rig):
    """A two-handed-weight overhead swing with the right arm: long wind-up, a strike of a
    few frames stepping in, big follow-through, slow recovery. Marker "Hit" on contact."""
    m = Motion("KillerSwing", rig, 1.0, loop=False, priority="Action")
    # p: 0 ready | 0.3 wound up | 0.38 contact | 0.5 follow-through | 1 back to ready
    # The waist does most of the turn and the root a third of it: both at full strength
    # turned the body ~95 degrees, sideways to its target.
    twist = keys([(0, 0), (0.3, -35, "in"), (0.38, 22, "out"), (0.5, 35), (1, 0)], loop=False)
    m.set("waist", (keys([(0, -10), (0.3, 8, "in"), (0.38, -22, "out"), (0.5, -26), (1, -10)], loop=False), twist, 0))
    m.set("root", rot=(0, lag(scale_(twist, 0.35), 0.02), 0),
          move=(0, 0, keys([(0, 0), (0.3, 0.3, "in"), (0.4, -0.7, "out"), (1, 0)], loop=False)))
    m.set("shoulder.R", (keys([(0, 20), (0.3, 165, "in"), (0.38, 55, "out"), (0.5, 15), (1, 20)], loop=False), 0,
                         keys([(0, 20), (0.3, 55, "in"), (0.38, -10, "out"), (0.5, -35), (1, 20)], loop=False)))
    m.set("elbow.R", (keys([(0, 35), (0.3, 80, "in"), (0.38, 8, "out"), (0.55, 25), (1, 35)], loop=False), 0, 0))
    m.set("shoulder.L", (keys([(0, 10), (0.3, -25), (0.4, 40, "out"), (1, 10)], loop=False), 0, -20))
    m.set("elbow.L", (keys([(0, 20), (0.3, 50), (0.45, 30), (1, 20)], loop=False), 0, 0))
    m.set("neck", (keys([(0, 6), (0.3, -6), (0.4, 16), (1, 6)], loop=False), lag(twist, 0.05) if False else
                   keys([(0, 0), (0.3, 20), (0.4, -18), (1, 0)], loop=False), 0))
    m.set("hip.R", (keys([(0, 5), (0.3, -12), (0.4, 30, "out"), (1, 5)], loop=False), 0, 4))
    m.set("knee.R", (keys([(0, -10), (0.3, -8), (0.4, -38, "out"), (1, -10)], loop=False), 0, 0))
    m.set("hip.L", (keys([(0, -3), (0.35, -5), (0.42, -18), (1, -3)], loop=False), 0, -6))
    m.set("knee.L", (keys([(0, -8), (0.35, -10), (0.42, -24), (1, -8)], loop=False), 0, 0))
    m.plant()
    m.marker(0.38, "Hit")
    return m.bake()


def killer_stunned(rig):
    """Stunned: head snaps back, arms drop, the body wobbles, then gathers itself."""
    m = Motion("KillerStunned", rig, 1.6, loop=False, priority="Action2")
    snap = keys([(0, 0), (0.08, 30, "out"), (0.3, 18), (0.7, 12), (1, 0)], loop=False)
    m.set("neck", (snap, keys([(0, 0), (0.1, -15), (0.4, 12), (0.7, -8), (1, 0)], loop=False), 0))
    m.set("waist", (keys([(0, -10), (0.08, 14, "out"), (0.4, -22), (0.75, -18), (1, -10)], loop=False),
                    keys([(0, 0), (0.2, 10), (0.5, -10), (0.8, 5), (1, 0)], loop=False), 0))
    m.set("root", rot=(0, 0, keys([(0, 0), (0.2, 6), (0.45, -7), (0.7, 4), (1, 0)], loop=False)),
          move=(0, 0, keys([(0, 0), (0.1, 0.5, "out"), (1, 0.5)], loop=False)))
    m.pair("shoulder", (keys([(0, 15), (0.1, -10), (0.5, 0), (1, 15)], loop=False), 0,
                        keys([(0, 15), (0.1, 30), (0.5, 6), (1, 15)], loop=False)), offset=0.02)
    m.pair("elbow", (keys([(0, 30), (0.15, 5), (0.6, 10), (1, 30)], loop=False), 0, 0), offset=0.04)
    m.pair("knee", (keys([(0, -10), (0.2, -35), (0.5, -28), (1, -10)], loop=False), 0, 0), offset=0.06)
    m.pair("hip", (keys([(0, 5), (0.2, 22), (0.5, 18), (1, 5)], loop=False), 0, 4), offset=0.06)
    m.plant()
    return m.bake()


HORROR = {
    "survivor_idle": survivor_idle, "survivor_walk": survivor_walk, "survivor_sprint": survivor_sprint,
    "survivor_exhausted": survivor_exhausted, "survivor_injured": survivor_injured_walk, "survivor_hit": survivor_hit,
    "survivor_taunt": survivor_taunt, "killer_idle": killer_idle, "killer_walk": killer_walk,
    "killer_chase": killer_chase, "killer_swing": killer_swing, "killer_stunned": killer_stunned,
}
