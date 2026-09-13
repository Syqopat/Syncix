"""animkit command line.

    python animkit.py capture <target> <rig.json> [--port 8080]
        Reads a rig model through the running Syncix core ("Workspace.R15Rig") and saves
        a snapshot, so animations can be made with Studio closed.
    python animkit.py capture-folder <src/Workspace/R15Rig> <rig.json>
        The same, from the synced files.
    python animkit.py presets
        Lists the ready-made animations.
    python animkit.py make <rig.json> <what>... [--out tools/out/anims] [--no-preview] [--in-betweens N]
        <what> is a preset name ("walk"), "all" (every preset), or a .py file defining
        build(rig) -> Animation (or a list of them). For each animation it prints the
        problems validate() finds and writes:
          <out>/<Name>.rbxmx      KeyframeSequence (in a Folder "Animations")
          <out>/lua/<Name>.lua    data for AnimKitPlayer (no upload needed)
          <out>/preview/<Name>.png  front + side contact sheet: LOOK AT IT
        and <out>/Animations.rbxmx with all of them, for a single `syncix import`.
"""
import argparse
import importlib.util
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from anim import Animation, write_rbxmx  # noqa: E402
from preview import render  # noqa: E402
from presets import PRESETS  # noqa: E402
from rig import Rig, from_folder, from_syncix  # noqa: E402


def _load_script(path, rig):
    spec = importlib.util.spec_from_file_location(os.path.splitext(os.path.basename(path))[0], path)
    module = importlib.util.module_from_spec(spec)
    sys.path.insert(0, os.path.dirname(os.path.abspath(path)))
    spec.loader.exec_module(module)
    if not hasattr(module, "build"):
        raise SystemExit(f"{path} has no build(rig) function")
    made = module.build(rig)
    return made if isinstance(made, list) else [made]


def cmd_capture(args):
    rig = from_syncix(args.target, args.port) if args.cmd == "capture" else from_folder(args.target)
    rig.save(args.out)
    err = rig.rest_error()
    print(f"{rig.name}: {rig.kind} rig, {len(rig.parts)} parts, {len(rig.joints)} joints, root {rig.root}")
    print(f"roles: {', '.join(f'{k}={v}' for k, v in sorted(rig.roles.items())) or '(none: custom rig, use joint names)'}")
    print(f"rest pose check: {'ok' if err < 0.01 else f'OFF BY {err:.3f} studs - the rig was not read correctly'}")
    print(f"saved {args.out}")


def cmd_make(args):
    rig = Rig.load(args.rig)
    animations = []
    for what in args.what:
        if what == "all":
            animations += [f(rig) for f in PRESETS.values()]
        elif what in PRESETS:
            animations.append(PRESETS[what](rig))
        elif what.endswith(".py"):
            animations += _load_script(what, rig)
        else:
            from difflib import get_close_matches
            near = get_close_matches(what, list(PRESETS) + ["all"], n=3)
            raise SystemExit(f"Unknown animation '{what}'." + (f" Did you mean {', '.join(near)}?" if near else ""))
    os.makedirs(os.path.join(args.out, "lua"), exist_ok=True)
    os.makedirs(os.path.join(args.out, "preview"), exist_ok=True)
    total_problems = 0
    for a in animations:
        if not isinstance(a, Animation):
            raise SystemExit(f"build() returned {type(a).__name__}, not an Animation")
        problems = a.validate()
        total_problems += len(problems)
        status = "ok" if not problems else f"{len(problems)} problem(s)"
        print(f"{a.name:<12} {a.length:.2f}s {'loop' if a.loop else '    '} {a.priority:<9} {len(a.keys)} keys  {status}")
        for p in problems:
            print(f"    - {p}")
        for n in a.notes():
            print(f"    (note: {n})")
        a.export_rbxmx(os.path.join(args.out, f"{a.name}.rbxmx"))
        a.export_lua(os.path.join(args.out, "lua", f"{a.name}.lua"))
        if not args.no_preview:
            render(a, os.path.join(args.out, "preview", f"{a.name}.png"), extra_per_gap=args.in_betweens)
    combined = write_rbxmx(os.path.join(args.out, "Animations.rbxmx"), animations)
    print(f"\n{len(animations)} animation(s) -> {args.out}  (all in one: {combined})")
    if total_problems:
        print(f"{total_problems} problem(s) above: fix them, then look at the previews.")


def main(argv=None):
    p = argparse.ArgumentParser(prog="animkit", description="Animations for Roblox rigs, made from code.")
    sub = p.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("capture")
    c.add_argument("target")
    c.add_argument("out")
    c.add_argument("--port", type=int, default=8080)
    cf = sub.add_parser("capture-folder")
    cf.add_argument("target")
    cf.add_argument("out")
    sub.add_parser("presets")
    m = sub.add_parser("make")
    m.add_argument("rig")
    m.add_argument("what", nargs="+")
    m.add_argument("--out", default=os.path.join("tools", "out", "anims"))
    m.add_argument("--no-preview", action="store_true")
    m.add_argument("--in-betweens", type=int, default=0)
    args = p.parse_args(argv)
    if args.cmd in ("capture", "capture-folder"):
        cmd_capture(args)
    elif args.cmd == "presets":
        for name, f in PRESETS.items():
            print(f"{name:<10} {(f.__doc__ or '').strip().splitlines()[0]}")
    else:
        cmd_make(args)


if __name__ == "__main__":
    main()
