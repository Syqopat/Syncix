"""Splits one long `impl` block into several files.

`impl DataModel` was 566 lines in one file. Rust lets a type's methods live in any
module of the same crate, so the methods move into sibling files, each with its own
`impl` block, and nothing at the call sites changes.

    python tools/split-impl.py packages/core-engine/src/model.rs DataModel \
        edit=upsert_instance,remove_instance lookup=find_by_name,resolve_target
"""
import io
import os
import re
import sys


def brace_delta(line):
    delta, i, in_s, in_c, esc = 0, 0, False, False, False
    while i < len(line):
        ch = line[i]
        if in_s:
            if esc:
                esc = False
            elif ch == "\\":
                esc = True
            elif ch == '"':
                in_s = False
        elif in_c:
            if esc:
                esc = False
            elif ch == "\\":
                esc = True
            elif ch == "'":
                in_c = False
        elif ch == '"':
            in_s = True
        elif ch == "/" and i + 1 < len(line) and line[i + 1] == "/":
            break
        elif ch == "{":
            delta += 1
        elif ch == "}":
            delta -= 1
        i += 1
    return delta


def methods(lines, start, end):
    """Every method of the impl block, doc comment included."""
    found, i = [], start
    while i < end:
        if re.match(r"^    (pub )?(async )?fn ", lines[i]):
            head, j = i, i - 1
            while j >= 0 and (lines[j].lstrip().startswith(("///", "//", "#["))):
                head, j = j, j - 1
            depth, k, opened = 0, i, False
            while k < end:
                depth += brace_delta(lines[k])
                opened = opened or depth > 0
                if opened and depth <= 0:
                    break
                k += 1
            name = re.sub(r"[^A-Za-z0-9_].*$", "", re.sub(r"^    (pub )?(async )?fn ", "", lines[i]))
            found.append({"name": name, "start": head, "end": k})
            i = k + 1
        else:
            i += 1
    return found


def main():
    if len(sys.argv) < 4:
        print(__doc__)
        return 1

    path, type_name = sys.argv[1], sys.argv[2]
    lines = io.open(path, encoding="utf-8").read().split("\n")

    # the impl block to split: the longest one for this type
    blocks = []
    for i, line in enumerate(lines):
        if line.startswith(f"impl {type_name} "):
            depth, k, opened = 0, i, False
            while k < len(lines):
                depth += brace_delta(lines[k])
                opened = opened or depth > 0
                if opened and depth <= 0:
                    break
                k += 1
            blocks.append((i, k))
    if not blocks:
        print(f"impl {type_name} bulunamadi")
        return 1
    start, end = max(blocks, key=lambda b: b[1] - b[0])

    plan = {}
    for pair in sys.argv[3:]:
        group, names = pair.split("=", 1)
        for name in names.split(","):
            plan[name] = group

    found = methods(lines, start + 1, end)
    missing = [n for n in plan if n not in {m["name"] for m in found}]
    if missing:
        print("bu metotlar impl icinde yok:", ", ".join(missing))
        return 1

    folder = path[: -len(".rs")]
    os.makedirs(folder, exist_ok=True)
    taken, buckets = set(), {}
    for method in found:
        group = plan.get(method["name"])
        if not group:
            continue
        buckets.setdefault(group, []).append("\n".join(lines[method["start"]:method["end"] + 1]))
        taken.update(range(method["start"], method["end"] + 1))

    for group, chunks in buckets.items():
        text = (f"//! {type_name} methods split out of {os.path.basename(path)}.\n\n"
                f"#[allow(unused_imports)]\nuse super::*;\n\nimpl {type_name} {{\n"
                + "\n\n".join(chunks) + "\n}\n")
        io.open(os.path.join(folder, f"{group}.rs"), "w", encoding="utf-8", newline="\n").write(text)
        print(f"{folder}/{group}.rs: {text.count(chr(10))} satir")

    kept = "\n".join(line for n, line in enumerate(lines) if n not in taken)
    decls = "\n".join(f"mod {g};" for g in sorted(buckets))
    first_use = next((n for n, l in enumerate(kept.split("\n")) if l.startswith("use ")), 0)
    body = kept.split("\n")
    body.insert(first_use, decls + "\n")
    kept = re.sub(r"\n{4,}", "\n\n\n", "\n".join(body))
    io.open(os.path.join(folder, "mod.rs"), "w", encoding="utf-8", newline="\n").write(kept + "\n")
    os.remove(path)
    print(f"{folder}/mod.rs: {kept.count(chr(10))} satir")
    return 0


if __name__ == "__main__":
    sys.exit(main())
