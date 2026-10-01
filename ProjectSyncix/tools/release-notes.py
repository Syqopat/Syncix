"""Prints one version's section of CHANGELOG.md, for the release page body.

The release notes and the changelog used to be written separately, so a release page
could say less than the repository did. The changelog is the source now: a version with
no section in it is an error, not an empty release page.

New entries are drafted from the commit messages with git-cliff (see cliff.toml) and
then edited; this script only reads what is committed.

Usage:
    python tools/release-notes.py v0.1.7
"""

import io
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def section(version):
    text = io.open(os.path.join(ROOT, "CHANGELOG.md"), encoding="utf-8").read()
    wanted = version.lstrip("v")
    heading = re.compile(r"^## \[%s\].*$" % re.escape(wanted), re.M)
    start = heading.search(text)
    if not start:
        raise SystemExit(
            "CHANGELOG.md has no section for %s. Add one before tagging." % wanted
        )
    following = re.compile(r"^## \[", re.M).search(text, start.end())
    body = text[start.end() : following.start() if following else len(text)]
    return body.strip()


def main(argv):
    if len(argv) != 1:
        raise SystemExit("Usage: python tools/release-notes.py <version>")
    print(section(argv[0]))
    print()
    print("---")
    print()
    print(
        "**Install:** download the `.vsix`, then in VS Code open the Extensions panel,\n"
        "use the `...` menu and pick **Install from VSIX...**. The Studio plugin and the\n"
        "core engine ship inside it; nothing else to download.\n"
        "\n"
        "`SyncixPlugin.rbxm` and `syncix-core.exe` are the same files from inside that\n"
        "package, for anyone who wants only one of them."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
