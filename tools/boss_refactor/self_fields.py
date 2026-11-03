#!/usr/bin/env python3
"""Lot L9b: literal offsets of the boss's own fields (`x.u8(0x..)`, `sub8(x, 0x.., k)`, `for off in [0x..]`, init
tuples `(0x.., v)`) become the names of `Enemy` / `Obj` / `Boss` (and of the template `+0xa0..` with TEMPLATE=1).
Unlike `common_fields.py`, only the receiver `x` (the boss itself) is rewritten, and the prefix of a call is never
taken for an offset (`x.set8(off, 0)` stays). Prints what remains.

Usage: [TEMPLATE=1] [SPEC=spec.py] self_fields.py apply|check FILE...

Any other receiver (`shot.set_u8(x.b, 0x8, v)`) takes the fields common to all objects (`Obj::X`...). SPEC: a Python
file defining `SPEC = {(receiver, offset, width): name}` (receiver "x" for the boss itself; width 8, 16, 32, 'bit' or
'link') for the fields that the boss's own record or another record names.
"""
import os
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
os.environ["BOSS_FIELDS"] = "1"
from common_fields import E, WIDTH, METHODS  # noqa: E402

E = dict(E)
E.update({
    (0x6f, 8): "Enemy::AI_SPARE_ROW",
    (0x57, 8): "Boss::HIT_FLAGS",
    (0x3b, 8): "Enemy::GROUND_MARK",
    (0x3f, 8): "Enemy::PRIORITY",
    (0x82, 8): "Enemy::WEAK_TO_B",
    (0x83, 8): "Enemy::WEAK_TO_C",
    (0x09, 8): "Boss::X_LOW",
    (0x0d, 8): "Boss::HEIGHT_LOW",
    (0x12, 16): "Enemy::Y_FRACTION",
    (0x2d, 8): "Enemy::HEADING",
    (0x32, 16): "Enemy::ANIM_END",
})
if os.environ.get("TEMPLATE"):
    E.update({
        (0xa0, 8): "Boss::ANIM_KIND", (0xa1, 8): "Boss::ANIM_LAST", (0xa2, 32): "Boss::SAVED_STATE",
        (0xa6, 16): "Boss::TIMER", (0xa8, 16): "Boss::TIMER2", (0xaa, 8): "Boss::PHASE", (0xab, 8): "Boss::PHASES",
        (0xad, 8): "Boss::INVULNERABLE", (0xbc, 16): "Boss::MARK_X",
    })
OBJ = {
    (0x00, 8): "Obj::ACTIVE", (0x02, 8): "Obj::KIND", (0x03, 8): "Obj::VARIANT", (0x04, 8): "Obj::STATE",
    (0x08, 16): "Obj::X", (0x08, 32): "Obj::X", (0x0c, 16): "Obj::Z", (0x0c, 32): "Obj::Z", (0x10, 16): "Obj::Y",
    (0x10, 32): "Obj::Y", (0x14, 16): "Obj::VX", (0x16, 16): "Obj::AX", (0x18, 16): "Obj::VZ", (0x1a, 16): "Obj::AZ",
    (0x1c, 16): "Obj::VY", (0x1e, 16): "Obj::AY", (0x28, 8): "Obj::CLASS", (0x40, 8): "Obj::HIDDEN",
}
SPEC = {}
if os.environ.get("SPEC"):
    g = {}
    exec(open(os.environ["SPEC"]).read(), g)
    SPEC = g["SPEC"]
for (r, o, w), nm in SPEC.items():
    if r == "x":
        E[(o, w)] = nm
FN_WIDTH = {"sub8": 8, "tst8": 8, "cmp8": 8, "addq8": 8, "sub16": 16, "tst16": 16, "cmp16": 16, "expired": 16}
LIT = r"(0x[0-9a-fA-F_]+|\d+)\b(?!\s*[+\-*])"
CALL = re.compile(rf"\bx\.({METHODS})\({LIT}")
OTHER = re.compile(rf"\b(\w+)\.({METHODS})\((x\.b|b), {LIT}")
FN = re.compile(rf"\b({'|'.join(FN_WIDTH)})\(x, {LIT}")


def fix(path, apply):
    src = open(path).read().split("\n")
    out, left = [], []
    for_list = None  # width of the `for off in [..]` list being read
    init_list = False
    for n, line in enumerate(src, 1):
        code, sep, comment = line.partition("//")

        def name(lit, w):
            return E.get((int(lit.replace("_", ""), 0), w))

        def call(m):
            meth, lit = m.groups()
            nm = name(lit, WIDTH[meth])
            if nm is None:
                left.append(f"{path}:{n}: {line.strip()}")
                return m.group(0)
            return f"x.{meth}({nm}"

        def fn(m):
            f, lit = m.groups()
            nm = name(lit, FN_WIDTH[f])
            if nm is None:
                left.append(f"{path}:{n}: {line.strip()}")
                return m.group(0)
            return f"{f}(x, {nm}"

        def other(m):
            recv, meth, bus, lit = m.groups()
            if recv in ("x", "self"):
                return m.group(0)
            key = (int(lit.replace("_", ""), 0), WIDTH[meth])
            nm = SPEC.get((recv,) + key) or OBJ.get(key)
            if nm is None:
                left.append(f"{path}:{n}: {line.strip()}")
                return m.group(0)
            return f"{recv}.{meth}({bus}, {nm}"

        code = OTHER.sub(other, code)
        code = CALL.sub(call, code)
        code = FN.sub(fn, code)
        m = re.search(r"for (\w+) in \[([^\]]*)\]", code)
        if m and re.fullmatch(r"[\s0-9a-fA-Fx_u32,]*", m.group(2)):
            var = m.group(1)
            body = "\n".join(src[n:n + 4])
            wm = re.search(rf"x\.(set8|set16|set32|u8|u16|u32)\({var}\b", body)
            if wm:
                w = WIDTH[wm.group(1)]
                items = [s.strip() for s in m.group(2).split(",") if s.strip()]
                names = [name(re.sub(r"u32$", "", i), w) for i in items]
                if all(names):
                    code = code.replace(m.group(0), f"for {var} in [{', '.join(names)}]")
                else:
                    left.append(f"{path}:{n}: {line.strip()} (for)")
        if "for (off, v) in [" in code:
            init_list = True
        if init_list:
            def tup(m):
                nm = name(m.group(1), 8)
                if nm is None:
                    left.append(f"{path}:{n}: {line.strip()} (init)")
                    return m.group(0)
                return f"({nm}, "
            code = re.sub(r"\((0x[0-9a-fA-F]+), ", tup, code)
            if "] {" in code:
                init_list = False
        out.append(code + sep + comment)
    if apply:
        open(path, "w").write("\n".join(out))
    return left


if __name__ == "__main__":
    apply = sys.argv[1] == "apply"
    for p in sys.argv[2:]:
        for l in fix(p, apply):
            print(l)
