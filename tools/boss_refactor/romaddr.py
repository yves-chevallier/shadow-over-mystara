#!/usr/bin/env python3
"""Constants of the ROM range (0x400-0x3fffff) of a family: `const NAME: u32 = 0x..` becomes a `RomAddr` and its uses
`NAME.0`. Usage: romaddr.py FAMILY_PATH (a directory or a file), run from families/."""
import re
import sys
from pathlib import Path

REACTIONS = {0x56816, 0x56a0e, 0x58d0a, 0x58f2e, 0x5acfc, 0x5ae8e, 0x5cbe8, 0x5cd14, 0x5ea5c, 0x5ecb8, 0x6090c,
             0x60abe, 0x62aa2, 0x62d76, 0x6518a, 0x68542, 0x68724, 0x6a206, 0x6a3a0, 0x6bb4c, 0x6bd3a, 0x6d0f0,
             0x6d284, 0x6e68c, 0x6e824, 0x6fe22, 0x70018, 0x715d4, 0x740c0, 0x742fa, 0x77aec, 0x78a26}
DECL = re.compile(r"^(\s*(?:pub(?:\([a-z]+\))? )?const (\w+)): u32 = (0x[0-9a-fA-F_]+);")

root = Path(sys.argv[1])
files = sorted(root.glob("*.rs")) if root.is_dir() else [root]
names = {}
texts = {f: f.read_text() for f in files}
for f, t in texts.items():
    lines = t.split("\n")
    for i, l in enumerate(lines):
        m = DECL.match(l)
        if not m:
            continue
        v = int(m.group(3).replace("_", ""), 16)
        if 0x400 <= v < 0x400000:
            names[m.group(2)] = v
            lines[i] = f"{m.group(1)}: RomAddr = RomAddr({v:#x});"
    texts[f] = "\n".join(lines)
for f, t in texts.items():
    for n in names:
        t = re.sub(rf"(?<![\w.]){n}\b(?!\s*[:=(.])", f"{n}.0", t)
        t = re.sub(rf"const {n}\.0:", f"const {n}:", t)
    if t != f.read_text():
        if "RomAddr" in t and "use crate::data::RomAddr" not in t and "data::{" not in t:
            t = re.sub(r"(\nuse [^\n]*;\n)", r"\1use crate::data::RomAddr;\n", t, count=1)
        f.write_text(t)
print(" ".join(f"{n}={v:#x}" for n, v in names.items()))
