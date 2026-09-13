# animkit

Character animation for Roblox, written as code. Plain Python 3, no dependencies.

An AI (or a person) describes keyframes in the character's own axes; animkit turns them
into Roblox KeyframeSequences, checks them, and draws every keyframe from the front and
the side as a PNG, so whoever wrote the animation can see it without opening Studio.

It works on any Motor6D rig: R15, R6 or a custom creature. The rig is read from the
place through Syncix, and ready-made animations are written with joint *roles*, so one
walk cycle drives R15 and R6 alike.

It lives beside Syncix rather than inside it on purpose: animation authoring changes
fast and is tried in seconds in Python, and a mistake here must not reach the sync
engine. When it settles it can become a `syncix anim` command.

## Files

| File | What |
|---|---|
| `animkit.py` | Command line: `capture`, `capture-folder`, `presets`, `make` |
| `rig.py` | Reads a rig (Syncix HTTP API or synced files), roles, posing, snapshots |
| `anim.py` | `Animation`: keys, easing, markers, mirroring, validation, KeyframeSequence and Lua export |
| `presets.py` | idle, walk, run, jump, fall, land, jab, uppercut, kick, slash, block, hit_react, death, wave, cheer |
| `preview.py` | Front + side contact sheet PNG |
| `cframe.py`, `png.py` | CFrame maths and a PNG writer, standard library only |
| `AnimKitPlayer.lua` | Plays the Lua export in game without uploading anything |
| `selftest.py` | Checks on synthetic R15 and R6 rigs |

## Use

    python animkit.py capture Workspace.R15Rig rigs/R15.json
    python animkit.py make rigs/R15.json walk run jab --out out/anims
    syncix import out/anims/Animations.rbxmx ServerStorage

A KeyframeSequence plays in Studio through `KeyframeSequenceProvider:RegisterKeyframeSequence`;
a live game needs it uploaded (right-click > Save to Roblox) by the place's owner.

## Axes

The character faces -Z. In a pose `(x, y, z)` degrees: +x swings a hanging limb forward
(for the waist and neck: leans back), +y turns to the character's left, +z moves a
right-side limb outward. Elbows bend with +x, knees with -x. `sym()` mirrors onto the
left side.
