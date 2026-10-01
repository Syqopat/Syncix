"""All three components must carry the same version.

The Studio plugin refuses a core whose major.minor differs, so a release where one
file was forgotten installs and then refuses to connect, with the reason only in
Studio's Output. This is the check that stops that from reaching a release.
"""

import io
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

SOURCES = [
    (
        "packages/core-engine/Cargo.toml",
        r'^version\s*=\s*"([^"]+)"',
    ),
    (
        "packages/vscode-extension/package.json",
        r'"version"\s*:\s*"([^"]+)"',
    ),
    (
        "packages/studio-plugin/src/Network/Protocol.lua",
        r'VERSION\s*=\s*"([^"]+)"',
    ),
]


def main():
    found = {}
    for relative, pattern in SOURCES:
        path = os.path.join(ROOT, relative)
        text = io.open(path, encoding="utf-8").read()
        match = re.search(pattern, text, re.M)
        if not match:
            print("no version found in " + relative)
            return 1
        found[relative] = match.group(1)

    distinct = set(found.values())
    for relative, version in found.items():
        print("  %-50s %s" % (relative, version))
    if len(distinct) != 1:
        print("RESULT: the versions differ: %s" % ", ".join(sorted(distinct)))
        return 1
    print("RESULT: every component is %s" % distinct.pop())
    return 0


if __name__ == "__main__":
    sys.exit(main())
