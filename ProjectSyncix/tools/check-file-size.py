"""No product file over 500 lines.

A 3000-line main.rs and a 2400-line CLI were the reason nobody could find anything in
this repository. The limit is a limit on the product code; generated data and tests are
exempt, because splitting a generated table or a test file buys nothing.
"""

import io
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LIMIT = 500

LOOK_IN = [
    ("packages/core-engine/src", (".rs",)),
    ("packages/studio-plugin/src", (".lua", ".luau")),
    ("packages/vscode-extension/src", (".ts",)),
]

# Why each of these is allowed to be long.
EXEMPT = {
    # Generated from Roblox's own API dump by tools/gen-properties.py. It is data, not
    # code: splitting it would only spread one table over several files.
    "packages/studio-plugin/src/Observer/PropertyTable.lua": "generated data",
}


def main():
    too_long = []
    for folder, suffixes in LOOK_IN:
        base = os.path.join(ROOT, folder)
        for current, _dirs, files in os.walk(base):
            for name in files:
                if not name.endswith(suffixes):
                    continue
                path = os.path.join(current, name)
                relative = os.path.relpath(path, ROOT).replace(os.sep, "/")
                if relative in EXEMPT:
                    continue
                # Tests live beside the code they test; their length is their own.
                if "/tests/" in relative or relative.endswith(("_tests.rs", ".test.ts")):
                    continue
                lines = io.open(path, encoding="utf-8", errors="replace").read().count("\n") + 1
                if lines > LIMIT:
                    too_long.append((relative, lines))

    for relative, lines in sorted(too_long, key=lambda row: -row[1]):
        print("  %-60s %d lines" % (relative, lines))
    if too_long:
        print("RESULT: %d file(s) over %d lines" % (len(too_long), LIMIT))
        return 1
    print("RESULT: every product file is at most %d lines" % LIMIT)
    for relative, why in sorted(EXEMPT.items()):
        print("  exempt: %s (%s)" % (relative, why))
    return 0


if __name__ == "__main__":
    sys.exit(main())
