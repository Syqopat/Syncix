"""Splits one long Rust module into submodules without touching the code inside.

Reviewer feedback named the two-thousand-line files; this is the tool that took them
apart. It moves whole items (a function with its doc comment, a struct, a const) into
sibling files, marks them pub(crate) so the parent can still reach them, and leaves a
mod.rs that re-exports everything, so no call site has to change.

    python tools/split-module.py --list packages/core-engine/src/layout.rs
    python tools/split-module.py packages/core-engine/src/layout.rs \
        paths=write_path,file_name write=write_full_tree,write_node
"""
import io
import os
import re
import sys

START = re.compile(r"^(pub |pub\(crate\) |pub\(super\) )?(unsafe )?(async fn|fn|const|static|struct|enum|impl|type|trait) ")
TEST_MOD = re.compile(r"^mod (\w+) \{$")
DOC = ("///", "//!", "#[", "// ")


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


def items(lines):
    found, i = [], 0
    while i < len(lines):
        if START.match(lines[i]):
            start, j = i, i - 1
            while j >= 0 and lines[j].startswith(DOC):
                start, j = j, j - 1
            depth, k, opened = 0, i, False
            while k < len(lines):
                depth += brace_delta(lines[k])
                opened = opened or depth > 0
                done = lines[k].rstrip().endswith("}") or lines[k].rstrip().endswith(";")
                if depth <= 0 and done and (opened or lines[k].rstrip().endswith(";")):
                    break
                k += 1
            name = re.sub(r"[^A-Za-z0-9_].*$", "", re.sub(START, "", lines[i]))
            found.append({"name": name, "start": start, "decl": i, "end": k})
            i = k + 1
        else:
            i += 1
    return found


def test_modules(lines):
    """Every `#[cfg(test)] mod x { ... }` block, so tests do not keep a module long."""
    found, i = [], 0
    while i < len(lines) - 1:
        if lines[i] == "#[cfg(test)]" and TEST_MOD.match(lines[i + 1]):
            name = TEST_MOD.match(lines[i + 1]).group(1)
            k = i + 1
            depth = 0
            while k < len(lines):
                depth += brace_delta(lines[k])
                if depth <= 0:
                    break
                k += 1
            found.append({"name": name, "start": i, "end": k})
            i = k + 1
        else:
            i += 1
    return found


def main():
    args = sys.argv[1:]
    listing = args and args[0] == "--list"
    if listing:
        args = args[1:]
    if not args:
        print(__doc__)
        return 1

    path = args[0]
    lines = io.open(path, encoding="utf-8").read().split("\n")
    found = items(lines)

    if listing:
        for item in found:
            print(f"{item['name']:34} {item['start'] + 1:5}-{item['end'] + 1:5}"
                  f"  ({item['end'] - item['start'] + 1})")
        print(f"{len(found)} item, {len(lines)} satir")
        return 0

    plan = {}
    for pair in args[1:]:
        module, names = pair.split("=", 1)
        for name in names.split(","):
            plan[name] = module
    unknown = [n for n in plan if n not in {i["name"] for i in found}]
    if unknown:
        print("bu isimler dosyada yok:", ", ".join(unknown))
        return 1

    folder = path[: -len(".rs")]
    os.makedirs(folder, exist_ok=True)
    buckets, taken = {}, set()
    for item in found:
        module = plan.get(item["name"])
        if not module:
            continue
        body = lines[item["start"]:item["end"] + 1]
        d = item["decl"] - item["start"]
        if not body[d].startswith(("pub ", "pub(crate) ", "pub(super) ")):
            body[d] = "pub(crate) " + body[d]
        buckets.setdefault(module, []).append("\n".join(body))
        taken.update(range(item["start"], item["end"] + 1))

    for module, chunks in buckets.items():
        out = (f"//! Split out of {os.path.basename(path)}.\n\n#[allow(unused_imports)]\n"
               f"use super::*;\n\n" + "\n\n".join(chunks) + "\n")
        io.open(os.path.join(folder, f"{module}.rs"), "w", encoding="utf-8", newline="\n").write(out)
        print(f"{folder}/{module}.rs: {out.count(chr(10))} satir")

    kept = "\n".join(line for n, line in enumerate(lines) if n not in taken)
    decls = "\n".join(f"mod {m};" for m in sorted(buckets)) + "\n\n" + \
            "\n".join(f"pub(crate) use {m}::*;" for m in sorted(buckets))
    first = next((n for n, line in enumerate(kept.split("\n")) if line.startswith("use ")), 0)
    body = kept.split("\n")
    body.insert(first, decls + "\n")
    kept = re.sub(r"\n{4,}", "\n\n\n", "\n".join(body))
    io.open(os.path.join(folder, "mod.rs"), "w", encoding="utf-8", newline="\n").write(kept + "\n")
    os.remove(path)
    print(f"{folder}/mod.rs: {kept.count(chr(10))} satir")
    return 0


if __name__ == "__main__":
    sys.exit(main())
