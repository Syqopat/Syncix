"""Runs the Studio plugin's Luau tests with the luau CLI.

Why a tool and not plain `luau tests/x.lua`: the plugin's modules are written for
Roblox. They reach for the global `script` and require their neighbours through it
(`require(script.Parent.Services)`), and the CLI's require gives each module its own
globals, so neither can be supplied from the outside.

So the test is built as ONE chunk: the harness (the fake Roblox API and the assertions),
then each module the test names, then the test itself. Every
`local Name = require(...)` line in a module becomes `local Name = FAKE.Name`, which the
harness and the test fill in. Nothing in the module's own code is changed, so what runs
is the shipped code.

Usage:
    python tools/luau-test.py                      # every *.test.lua
    python tools/luau-test.py tests/batchqueue.test.lua
"""

import glob
import io
import os
import re
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PLUGIN = os.path.join(ROOT, "packages", "studio-plugin")
SRC = os.path.join(PLUGIN, "src")
TESTS = os.path.join(PLUGIN, "tests")

REQUIRE_LINE = re.compile(r"^(\s*)local\s+(\w+)\s*=\s*require\(([^)]*)\)\s*$", re.M)
MODULE_HEADER = re.compile(r"^--!modules\s+(.*)$", re.M)


def read(path):
    return io.open(path, encoding="utf-8").read()


def module_source(relative):
    """One module, with its requires turned into FAKE lookups."""
    path = os.path.join(SRC, relative + ".lua")
    if not os.path.exists(path):
        raise SystemExit("no such module: " + path)
    text = read(path)

    def swap(match):
        indent, name, _target = match.group(1), match.group(2), match.group(3)
        return '%slocal %s = FAKE["%s"] or error("the test gave no %s")' % (
            indent,
            name,
            name,
            name,
        )

    return REQUIRE_LINE.sub(swap, text)


def build(test_path):
    test_text = read(test_path)
    header = MODULE_HEADER.search(test_text)
    modules = header.group(1).split() if header else []

    parts = [read(os.path.join(TESTS, "harness.lua"))]
    for relative in modules:
        name = os.path.basename(relative)
        parts.append(
            "-- ---- %s ----\nlocal %s = (function()\n%s\nend)()\nFAKE[\"%s\"] = %s\n"
            % (relative, name, module_source(relative), name, name)
        )
    parts.append(test_text)
    parts.append("\nreturn __report()\n")
    return "\n".join(parts)


def run(test_path):
    source = build(test_path)
    handle, temp = tempfile.mkstemp(suffix=".lua", prefix="syncix-luau-test-")
    os.close(handle)
    io.open(temp, "w", encoding="utf-8", newline="\n").write(source)
    try:
        done = subprocess.run(["luau", temp], capture_output=True, text=True, cwd=ROOT)
    finally:
        os.remove(temp)

    name = os.path.relpath(test_path, PLUGIN).replace(os.sep, "/")
    print("== %s" % name)
    output = (done.stdout or "") + (done.stderr or "")
    print(output.rstrip())
    return done.returncode == 0


def main(argv):
    wanted = argv or sorted(glob.glob(os.path.join(TESTS, "*.test.lua")))
    if not wanted:
        print("no tests found under " + TESTS)
        return 1
    failed = 0
    for path in wanted:
        if not os.path.isabs(path):
            path = os.path.join(ROOT, path)
        if not run(path):
            failed += 1
    print("RESULT: %d file(s) failed" % failed if failed else "RESULT: all Luau tests pass")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
