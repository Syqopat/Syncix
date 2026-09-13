"""Draws an animation as a contact sheet PNG: front and side view of the rig at each
keyframe (and optional in-betweens), so an AI that cannot open Studio can still look at
what it made and fix it.

Front view: the character faces you (its right hand is on your left).
Side view: seen from the character's right; it faces right.
Every part is drawn as the silhouette of its box from that side. Right limbs are red,
left limbs blue, the rest dark; the grey line is the ground at rest.
"""
from png import Canvas

RIGHT, LEFT, BODY = (214, 69, 65), (52, 120, 214), (60, 63, 86)
GROUND, BG, TEXT = (190, 196, 206), (246, 247, 250), (43, 45, 66)


def _color(name):
    n = name.lower().replace(" ", "")
    if n.startswith("right") or n.endswith(".r"):
        return RIGHT
    if n.startswith("left") or n.endswith(".l"):
        return LEFT
    return BODY


def _darker(c):
    return tuple(int(v * 0.6) for v in c)


def _hull(points):
    """Convex hull, counter-clockwise (monotone chain)."""
    pts = sorted(set(points))
    if len(pts) <= 2:
        return pts

    def cross(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])

    lower, upper = [], []
    for p in pts:
        while len(lower) >= 2 and cross(lower[-2], lower[-1], p) <= 0:
            lower.pop()
        lower.append(p)
    for p in reversed(pts):
        while len(upper) >= 2 and cross(upper[-2], upper[-1], p) <= 0:
            upper.pop()
        upper.append(p)
    return lower[:-1] + upper[:-1]


def _corners(part, cf):
    sx, sy, sz = (v / 2 for v in part.size)
    return [cf.point((x, y, z)) for x in (-sx, sx) for y in (-sy, sy) for z in (-sz, sz)]


def render(animation, path, extra_per_gap=0, cell=220, label=True):
    """Writes the sheet; returns the times drawn. extra_per_gap in-betweens per key gap."""
    rig = animation.rig
    times = []
    keys = [k.time for k in animation.keys] or [0.0]
    for t0, t1 in zip(keys, keys[1:]):
        times += [t0 + (t1 - t0) * i / (extra_per_gap + 1) for i in range(extra_per_gap + 1)]
    times.append(keys[-1])

    rest = rig.solve({})
    ground = min(p.cframe.p[1] - p.size[1] / 2 for p in rig.parts.values())
    top = max(p.cframe.p[1] + p.size[1] / 2 for p in rig.parts.values())
    scale = (cell * 0.62) / max(top - ground, 1.0)
    center = rest[rig.root].p

    sheet = Canvas(len(times) * cell, 2 * cell + 28, BG)
    if label:
        sheet.text(6, 6, f"{animation.name.upper()}  {animation.length:.2f}S  {'LOOP' if animation.loop else ''}", TEXT, 2)

    for col, t in enumerate(times):
        world = rig.solve(animation.sample(t))
        for row, view in enumerate(("front", "side")):
            ox, oy = col * cell, 28 + row * cell
            base_y = oy + cell - 24

            def project(p):
                h = -(p[0] - center[0]) if view == "front" else -(p[2] - center[2])
                return (ox + cell / 2 + h * scale, base_y - (p[1] - ground) * scale)

            # Far parts first. Front view: the viewer is at -Z, so a larger z is farther.
            # Side view: the viewer is at +X, so a smaller x is farther.
            def depth(item):
                cf = item[1]
                return -cf.p[2] if view == "front" else cf.p[0]

            sheet.line(ox + 10, base_y, ox + cell - 10, base_y, GROUND, 2)
            for name, cf in sorted(((n, c) for n, c in world.items() if n in rig.parts and n != "HumanoidRootPart"), key=depth):
                hull = _hull([tuple(round(v, 1) for v in project(p)) for p in _corners(rig.parts[name], cf)])
                color = _color(name)
                sheet.polygon(hull, color, _darker(color))
            if label:
                sheet.text(ox + 6, oy + 4, f"{t:.2f}S {'FRONT' if view == 'front' else 'SIDE'}", TEXT, 1)
        sheet.line(ox, 28, ox, 2 * cell + 28, GROUND, 1)
    sheet.save(path)
    return times
