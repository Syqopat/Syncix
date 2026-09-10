# Finds "used but not defined" problems in the plugin sources.
#
# Why it is needed: the syntax check does NOT catch these. The file compiles as
# valid Lua; the error only shows up when Studio RUNS that line. While cutting a
# block of code, first the Store/Approval require lines and then constants such as
# PLUGIN_VERSION were deleted by accident. Both made the plugin crash while Studio
# loaded it, and the cause was visible only in Output.
#
# Checks:
#   1. If one of the plugin's OWN module names (file names) is used, it must have
#      been required.
#   2. If an UPPER_CASE constant is used, it must be defined as a local.
#   3. A call to a local function must not pass more arguments than it declares.
#
# Comments and string literals are removed first to avoid false alarms.

import io
import os
import re
import sys
import glob

NAME = r"[A-Za-z_]\w*"

ROOT = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "studio-plugin",
    "src",
)


def strip_code(raw):
    """Removes comments and string literals; only code is left."""
    lines = [re.sub(r"--.*$", "", x) for x in raw.splitlines()]
    code = "\n".join(lines)
    # Strings are replaced with a PLACEHOLDER, not with blanks.
    #
    # With blanks, a call such as `label(a, "x", b)` became `label(a,  , b)`;
    # the argument counter skipped the middle one, counted 2 instead of 3, and
    # missed calls with too many arguments.
    # The placeholder is lowercase so it never trips the upper-case constant check.
    code = re.sub(r'"[^"\n]*"', "stringliteral", code)
    code = re.sub(r"'[^'\n]*'", "stringliteral", code)
    return code


def split_top_level(text):
    """Splits on top-level commas; commas INSIDE brackets do not count.

    UDim2.new(0, 24, 0, 16) is a single argument; a plain split(",") would cut it
    into four pieces and miscount every call.
    """
    parts, depth, start = [], 0, 0
    for i, ch in enumerate(text):
        if ch in "({[":
            depth += 1
        elif ch in ")}]":
            depth -= 1
        elif ch == "," and depth == 0:
            parts.append(text[start:i])
            start = i + 1
    parts.append(text[start:])
    return [p for p in parts if p.strip()]


def check():
    files = sorted(glob.glob(os.path.join(ROOT, "**", "*.lua"), recursive=True))
    if not files:
        print("WARNING: no .lua files found under " + ROOT)
        return 1

    modules = {
        os.path.splitext(os.path.basename(f))[0]
        for f in files
        if not os.path.basename(f).startswith("init.")
    }

    problems = 0
    for path in files:
        code = strip_code(io.open(path, encoding="utf-8").read())
        defined = set(re.findall(r"local[ ]+(" + NAME + r")", code))
        shown = os.path.relpath(path, ROOT).replace(os.sep, "/")

        # 1. Missing require
        for module in sorted(modules - defined):
            if re.search(r"(?<![A-Za-z_.])" + module + r"[ ]*[.:][ ]*[A-Za-z_]", code):
                print("  %-44s %s is used but never required" % (shown, module))
                problems += 1

        # 2. Undefined constant
        # Anything after a dot or colon is FIELD access, not a constant (Services.UUIDS).
        constants = set(re.findall(r"(?<![A-Za-z_.:])([A-Z][A-Z_]{2,})(?![A-Za-z_])", code))
        for constant in sorted(constants - defined):
            print("  %-44s %s is used but never defined" % (shown, constant))
            problems += 1

    # 3. ARGUMENT COUNT of local functions
    #
    # In the panel, the signatures of `label` and `makeButton` were changed but eight
    # calls kept the old form. Lua silently swallows extra arguments and makes
    # missing ones nil; the syntax is clean, and the error only appears when that
    # line runs, as "Color3 expected, got UDim2".
    for path in files:
        code = strip_code(io.open(path, encoding="utf-8").read())
        shown = os.path.relpath(path, ROOT).replace(os.sep, "/")

        for name, params in re.findall(
            r"local[ ]function[ ]+(" + NAME + r")[ ]*\(([^)]*)\)", code
        ):
            expected = len([x for x in params.split(",") if x.strip()])
            for call in re.findall(
                r"(?<![A-Za-z_.:])" + name + r"[ ]*\(([^()]*(?:\([^()]*\)[^()]*)*)\)", code
            ):
                if call.strip() == params.strip():
                    continue  # the definition itself
                given = len(split_top_level(call))
                if given > expected:
                    print(
                        "  %-44s %s(): declares %d parameters, called with %d arguments"
                        % (shown, name, expected, given)
                    )
                    problems += 1

    if problems:
        print("RESULT: %d problem(s)" % problems)
        return 1
    print("RESULT: clean (%d files)" % len(files))
    return 0


if __name__ == "__main__":
    sys.exit(check())
