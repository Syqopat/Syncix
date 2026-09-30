"""Moves the `#[cfg(test)] mod x { ... }` blocks of a module into <module>/tests/.

A module that carries seven hundred lines of tests reads as a long file even when the
code in it is short. The tests keep working: a child module still sees its parent's
private items.

    python tools/move-tests.py packages/core-engine/src/layout/mod.rs
"""
import io
import os
import re
import sys

HEAD = re.compile(r"^mod (\w+) \{$")


def brace_delta(line):
    """Counts braces outside strings, chars and comments.

    A naive count broke on a test whose fixture JSON contained a brace: the module
    looked unfinished and the next one was swallowed into the same file.
    """
    delta, i, in_string, in_char, escaped = 0, 0, False, False, False
    while i < len(line):
        ch = line[i]
        if in_string:
            if escaped:
                escaped = False
            elif ch == "\\":
                escaped = True
            elif ch == '"':
                in_string = False
        elif in_char:
            if escaped:
                escaped = False
            elif ch == "\\":
                escaped = True
            elif ch == "'":
                in_char = False
        elif ch == '"':
            in_string = True
        elif ch == "'" and i + 2 < len(line) and (line[i + 2] == "'" or line[i + 1] == "\\"):
            in_char = True
        elif ch == "/" and i + 1 < len(line) and line[i + 1] == "/":
            break
        elif ch == "{":
            delta += 1
        elif ch == "}":
            delta -= 1
        i += 1
    return delta


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return 1

    path = sys.argv[1]
    lines = io.open(path, encoding="utf-8").read().split("\n")
    folder = os.path.dirname(path)
    crate_path = folder.replace("\\", "/").split("src/", 1)[-1].replace("/", "::")

    blocks, i = [], 0
    while i < len(lines) - 1:
        if lines[i] == "#[cfg(test)]" and HEAD.match(lines[i + 1]):
            name = HEAD.match(lines[i + 1]).group(1)
            depth, k = 0, i + 1
            while k < len(lines):
                depth += brace_delta(lines[k])
                if depth <= 0:
                    break
                k += 1
            blocks.append((name, i, k))
            i = k + 1
        else:
            i += 1

    if not blocks:
        print("test modulu yok")
        return 0

    os.makedirs(os.path.join(folder, "tests"), exist_ok=True)
    taken = set()
    for name, start, end in blocks:
        inner = [l[4:] if l.startswith("    ") else l for l in lines[start + 2:end]]
        text = "\n".join(inner).replace("use super::*;", f"use crate::{crate_path}::*;")
        io.open(os.path.join(folder, "tests", f"{name}.rs"), "w", encoding="utf-8",
                newline="\n").write(f"//! {name.replace('_', ' ')}.\n\n{text}\n")
        taken.update(range(start, end + 1))
        print(f"{folder}/tests/{name}.rs")

    io.open(os.path.join(folder, "tests", "mod.rs"), "w", encoding="utf-8", newline="\n").write(
        "//! Tests of this module, kept beside it rather than inside it.\n\n"
        + "\n".join(f"mod {name};" for name, _, _ in blocks) + "\n")

    kept = [line for n, line in enumerate(lines) if n not in taken]
    body = re.sub(r"\n{4,}", "\n\n\n", "\n".join(kept)).rstrip()
    body += "\n\n#[cfg(test)]\nmod tests;\n"
    io.open(path, "w", encoding="utf-8", newline="\n").write(body)
    print(f"{path}: {body.count(chr(10))} satir")
    return 0


if __name__ == "__main__":
    sys.exit(main())
