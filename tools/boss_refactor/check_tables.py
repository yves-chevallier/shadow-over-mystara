#!/usr/bin/env python3
"""Checks that every boss table that the game reads (`"boss_N/..."`, `"boss/..."` strings of families/boss*) is declared
in extract/src/bosses.rs, and that every declared table is read. Run from the repository root."""
import glob
import re

decl = open("port/extract/src/bosses.rs").read()
declared = set(re.findall(r'(?:prog|data|rank|upto|pointed)\((?:rom, )?s, "([^"]+)"', decl))
declared |= set(re.findall(r'(?:prog|data)\("([^"]+)"', decl))
for m in re.finditer(r'decision\(rom, s, "([^"]+)", &\[([^\]]*)\]', decl):
    base, k = m.group(1), len(m.group(2).split(","))
    declared.add(base + "_jumps")
    declared.update([base + "_picks"] if k == 1 else [f"{base}_picks_{i}" for i in range(k)])
read = set()
for f in glob.glob("port/ddsom-game/src/families/boss*/*.rs") + glob.glob("port/ddsom-game/src/families/boss*.rs"):
    read |= set(re.findall(r'"(boss(?:_[0-9]+)?/[^"]+)"', open(f).read()))
print("read but not declared:", sorted(read - declared))
print("declared but not read:", sorted(declared - read))
