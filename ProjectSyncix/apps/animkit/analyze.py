"""Measures a KeyframeSequence used as a style reference, so its feel can be matched:
pose rate, holds, easing, how far each joint moves in character axes, root motion.

    python analyze.py <folder of a synced KeyframeSequence> [...] [--rig R6|R15|rig.json]

It reads the files Syncix wrote for the KeyframeSequence (Keyframe and Pose JSON), so the
reference in Studio is left untouched. It reports ranges and timing only, not the
keyframes themselves: the point is to match a style, not to copy someone's animation.
"""
import argparse
import collections
import json
import os
import statistics
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from cframe import CFrame  # noqa: E402
from rig import Rig  # noqa: E402


def _load(path):
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def _prop(d, key):
    v = d.get("properties", {}).get(key)
    if not isinstance(v, dict) or not v:
        return None
    return next(iter(v.values()))


def _enum(value):
    return (value or "").split(".")[-1]


def read_kfs(folder):
    """{name, loop, priority, keys: [(time, {part: {cf, style, dir, weight}})], markers}"""
    init = next((f for f in os.listdir(folder) if f.startswith("init.") and f.endswith(".json")), None)
    meta = _load(os.path.join(folder, init)) if init else {}
    keys, markers = [], []
    for entry in os.listdir(folder):
        path = os.path.join(folder, entry)
        if os.path.isdir(path):
            kfile = os.path.join(path, "init.keyframe.json")
        elif entry.endswith(".keyframe.json"):
            kfile = path
        else:
            continue
        if not os.path.exists(kfile):
            continue
        k = _load(kfile)
        t = float(_prop(k, "Time") or 0.0)
        poses = {}
        if os.path.isdir(path):
            for dirpath, _, files in os.walk(path):
                for f in files:
                    full = os.path.join(dirpath, f)
                    if f.endswith(".pose.json"):
                        p = _load(full)
                        cf = _prop(p, "CFrame")
                        poses[p["name"]] = {
                            "cf": CFrame(cf["pos"], cf["rot"]) if cf else CFrame(),
                            "style": _enum(_prop(p, "EasingStyle")),
                            "dir": _enum(_prop(p, "EasingDirection")),
                            "weight": _prop(p, "Weight"),
                        }
                    elif f.endswith(".keyframemarker.json"):
                        markers.append((t, _load(full).get("name")))
        keys.append((t, poses))
    keys.sort(key=lambda x: x[0])
    return {"name": meta.get("name", os.path.basename(folder)), "loop": _prop(meta, "Loop"),
            "priority": _enum(_prop(meta, "Priority")), "keys": keys, "markers": sorted(markers)}


def in_character_axes(rig, part, cf):
    """(joint, (x, y, z) degrees, move) for a Pose, in the character axes animkit writes in."""
    j = rig.by_part1.get(part)
    if not j:
        return None
    frame = rig.joint_frame_rest(j).rotation()
    rot = frame * cf.rotation() * frame.inverse()
    return j.name, rot.to_euler_degrees(), frame.vector(cf.p)


def report(kfs, rig):
    keys = kfs["keys"]
    times = [t for t, _ in keys]
    length = times[-1] if times else 0.0
    gaps = [b - a for a, b in zip(times, times[1:]) if b - a > 1e-6]
    print(f"\n=== {kfs['name']}  length {length:.2f}s  keyframes {len(keys)}  loop {kfs['loop']}  priority {kfs['priority']}")
    if gaps:
        med = statistics.median(gaps)
        print(f"    pose rate ~{1 / med:.0f}/s (median gap {med:.3f}s, shortest {min(gaps):.3f}s, longest {max(gaps):.3f}s)")
    styles = collections.Counter()
    for _, poses in keys:
        for pose in poses.values():
            if pose["weight"] not in (0, 0.0):
                styles[f"{pose['style']}/{pose['dir']}"] += 1
    print(f"    easing: {', '.join(f'{k} x{v}' for k, v in styles.most_common())}")
    if kfs["markers"]:
        print(f"    markers: {', '.join(f'{n}@{t:.2f}' for t, n in kfs['markers'])}")

    per_joint = collections.defaultdict(list)
    for t, poses in keys:
        for part, pose in poses.items():
            if pose["weight"] in (0, 0.0):
                continue
            conv = in_character_axes(rig, part, pose["cf"])
            if conv:
                name, deg, move = conv
                per_joint[name].append((t, deg, move))
    print(f"    {'joint':<15} {'x (fwd/back)':>16} {'y (turn)':>16} {'z (side)':>16}   root move (studs)")
    for name, samples in sorted(per_joint.items()):
        cols = []
        for axis in range(3):
            vals = [(s[1][axis], s[0]) for s in samples]
            lo, hi = min(vals), max(vals)
            cols.append(f"{lo[0]:>6.0f}..{hi[0]:<5.0f}")
        mv = ""
        moves = [s[2] for s in samples]
        if any(abs(c) > 1e-3 for m in moves for c in m):
            rng = [max(m[i] for m in moves) - min(m[i] for m in moves) for i in range(3)]
            mv = f"x {rng[0]:.2f}  y {rng[1]:.2f}  z {rng[2]:.2f}"
        print(f"    {name:<15} {cols[0]:>16} {cols[1]:>16} {cols[2]:>16}   {mv}")


def main(argv=None):
    ap = argparse.ArgumentParser(prog="analyze")
    ap.add_argument("folders", nargs="+")
    ap.add_argument("--rig", default="R6")
    args = ap.parse_args(argv)
    rig = Rig.load(args.rig)
    for folder in args.folders:
        report(read_kfs(folder), rig)


if __name__ == "__main__":
    main()
