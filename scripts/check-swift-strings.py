#!/usr/bin/env python3
"""Check that every Vietnamese text in the Swift code has an English entry.

Vietnamese is the source language of the code, so each Vietnamese string literal is also the lookup
key in swift/Resources/en.lproj/Localizable.strings. This script lists the literals that have no
entry (and entries that no literal uses). Interpolations "\\(...)" count as %@ / %lld.
Exit code 1 when something is missing.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VI = re.compile(r"[àáảãạăằắẳẵặâầấẩẫậèéẻẽẹêềếểễệìíỉĩịòóỏõọôồốổỗộơờớởỡợùúủũụưừứửữựỳýỷỹỵđ]", re.I)


def literals(src: str):
    """Yield the string literals of a Swift file, with "\\(...)" replaced by %@ (nesting-aware)."""
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if c == "/" and src[i : i + 2] == "//":
            while i < n and src[i] != "\n":
                i += 1
        elif c == '"':
            if src[i : i + 3] == '"""':
                j = src.find('"""', i + 3)
                i = n if j < 0 else j + 3
                continue
            out, i = [], i + 1
            while i < n and src[i] != '"':
                if src[i] == "\\" and src[i + 1] == "(":
                    depth, i = 1, i + 2
                    while i < n and depth:
                        if src[i] == "(":
                            depth += 1
                        elif src[i] == ")":
                            depth -= 1
                        elif src[i] == '"':  # a string inside the interpolation
                            i += 1
                            while src[i] != '"':
                                i += 2 if src[i] == "\\" else 1
                        i += 1
                    out.append("%@")
                elif src[i] == "\\":
                    out.append(src[i : i + 2])
                    i += 2
                else:
                    out.append(src[i])
                    i += 1
            i += 1
            yield "".join(out)
        else:
            i += 1


def unescape(s: str) -> str:
    return s.replace('\\"', '"').replace("\\n", "\n").replace("\\\\", "\\")


def norm(s: str) -> str:
    return re.sub(r"%(lld|lf|d)", "%@", s)


def main() -> int:
    strings = (ROOT / "swift/Resources/en.lproj/Localizable.strings").read_text(encoding="utf-8")
    entries = {}
    for m in re.finditer(r'^"((?:[^"\\]|\\.)*)"\s*=\s*"((?:[^"\\]|\\.)*)";', strings, re.M):
        entries[norm(unescape(m.group(1)))] = unescape(m.group(2))

    used = set()
    missing = []
    for f in sorted((ROOT / "swift/Sources").glob("*.swift")):
        for lit in literals(f.read_text(encoding="utf-8")):
            key = norm(unescape(lit))
            if not VI.search(key) and key not in entries:
                continue  # plain ASCII text that needs no entry (names, symbols)
            used.add(key)
            if key not in entries and f.name != "Language.swift":
                missing.append((f.name, key))

    # Texts passed through L() / LocalizedStringKey with a format are listed in the code too.
    unused = sorted(k for k in entries if k not in used)
    for name, key in missing:
        print(f"MISSING  {name}: {key}")
    for key in unused:
        print(f"UNUSED   {key}")
    print(f"{len(entries)} entries, {len(missing)} missing, {len(unused)} unused")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
