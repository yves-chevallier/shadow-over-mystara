#!/usr/bin/env python3
"""Turns the `mod g { pub const NAME: u32 = 0x..; }` field constants of a boss into the fields of `Enemy` / `Boss`
(same name at the same offset) and of a `record!` of the boss (the others). Usage: brec.py PATH Struct [TEMPLATE]
(run from families/). PATH: a directory (main file mod.rs) or a single file. TEMPLATE=1: the boss uses the template
fields of `Boss` (`+0xa0`..).
"""
import re
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from common_fields import E, BOSS, WIDTH, METHODS  # noqa: E402

TEMPLATE = {
    (0xa0, 8): "Boss::ANIM_KIND", (0xa1, 8): "Boss::ANIM_LAST", (0xa2, 32): "Boss::SAVED_STATE",
    (0xa6, 16): "Boss::TIMER", (0xa8, 16): "Boss::TIMER2", (0xaa, 8): "Boss::PHASE", (0xab, 8): "Boss::PHASES",
    (0xad, 8): "Boss::INVULNERABLE", (0xbc, 16): "Boss::MARK_X",
}
ALIAS = {"SAVED_X": "X_SAVED"}

root = Path(sys.argv[1])
struct = sys.argv[2]
template = len(sys.argv) > 3 and sys.argv[3] == "1"
files = sorted(root.glob("*.rs")) if root.is_dir() else [root]
main = root / "mod.rs" if root.is_dir() else root
texts = {f: f.read_text() for f in files}
src = texts[main]
m = re.search(r"\n(?:/// [^\n]*\n)*(?:pub(?:\(crate\))? )?mod g \{\n(.*?)\n\}\n(use g::\*;\n)?", src, re.S)
if not m:
    sys.exit(f"{main}: no mod g")
block = m.group(1).split("\n")
entries = []
docs = []
for l in block:
    s = l.strip()
    if s.startswith("///"):
        docs.append(s)
        continue
    d = re.match(r"pub const (\w+): u32 = (0x[0-9a-fA-F]+|\d+);", s)
    if d:
        entries.append((docs, d.group(1), int(d.group(2), 0)))
    elif s:
        sys.exit(f"{main}: unexpected line in mod g: {l}")
    docs = []

all_text = "\n".join(texts.values())
repl = {}
fields = []
for docs, name, v in entries:
    widths = Counter(WIDTH[w] for w in re.findall(rf"\.({METHODS})\((?:[a-z_][\w.]*, *)?{name}\b", all_text))
    for fn8 in ("sub8(x, ", "tst8(x, ", "cmp8(x, ", "addq8(x, "):
        widths[8] += all_text.count(fn8 + name)
    for fn16 in ("sub16(x, ", "tst16(x, ", "cmp16(x, ", "expired(x, "):
        widths[16] += all_text.count(fn16 + name)
    w = widths.most_common(1)[0][0] if widths else 8
    w = 8 if w in ("bit", "link") else w
    common = None
    for table in ([TEMPLATE] if template else []) + [BOSS, E]:
        c = table.get((v, w))
        if c and c.split("::")[1] == ALIAS.get(name, name):
            common = c
            break
    if common:
        repl[name] = common
    else:
        repl[name] = f"{struct}::{name}"
        fields.append((docs, name, v, {8: "u8", 16: "u16", 32: "u32"}[w]))

lines = [f"/// Fields of this boss beyond those of `Enemy` and `Boss` (`record!`). A meaning guessed from the code alone is",
         "/// marked (h).",
         "#[derive(Clone, Copy, PartialEq, Eq, Debug)]",
         f"pub struct {struct}(pub Obj);",
         "",
         f"impl {struct} {{",
         "    pub fn at(self, off: u32) -> u32 {",
         "        self.0.at(off)",
         "    }",
         "}",
         "",
         "record! {",
         f"    impl {struct} {{"]
for docs, name, v, t in sorted(fields, key=lambda f: f[2]):
    lines += ["        " + d for d in docs]
    lo = name.lower()
    lines.append(f"        {lo}, set_{lo}, {name}: {t} @ {v:#x};")
lines += ["    }", "}", ""]
new_main = src[:m.start()] + "\n" + "\n".join(lines) + src[m.end():]
if "use crate::record::record;" not in new_main:
    new_main = re.sub(r"(\nuse [^\n]*;\n)", r"\1use crate::record::record;\n", new_main, count=1)
if not re.search(r"use crate::objects::(Obj|\{[^}]*Obj)", new_main):
    new_main = re.sub(r"(\nuse [^\n]*;\n)", r"\1use crate::objects::Obj;\n", new_main, count=1)
texts[main] = new_main
for f, t in texts.items():
    out = []
    for line in t.split("\n"):
        if re.search(r"^\s+\w+, set_\w+, \w+: u(8|16|32) @", line):
            out.append(line)
            continue
        for name, r in repl.items():
            line = re.sub(rf"(?<![\w:.]){name}\b(?!\s*:)", r, line)
        out.append(line)
    t = "\n".join(out)
    if t != f.read_text():
        f.write_text(t)
for name, r in repl.items():
    print(f"{name} -> {r}")
