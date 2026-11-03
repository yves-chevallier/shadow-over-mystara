#!/usr/bin/env python3
"""Lot L9b: work RAM reads and writes at a literal A5 offset (`x.a5_u8(0x50)`, `x.set_a5_u16(0x51e, v)`,
`set_abs8(x, 0xff8043, v)`) become the named variables of `boss_common::vars` (`var::BOSS_FIGHT.get(x.b)`,
`var::CAMERA_MODE.set(x.b, v)`). Only the offsets of the table below, at the width of the variable; the rest is printed.

Usage: a5vars.py apply|check FILE...
"""
import re
import sys

A5 = 0xff8000
# A5 offset -> (name, width)
VARS = {
    0x2e: ("SUB_STEP", 8), 0x33: ("BLINK_SUSPENDED", 8), 0x38: ("STAGE_CLOCK", 32), 0x3c: ("DISPLAY_PANELS", 8),
    0x3d: ("PAUSE", 8), 0x42: ("PLAYERS_FROZEN", 8), 0x43: ("INPUT_MASK", 8), 0x4f: ("BOSS_DOWN", 8),
    0x50: ("BOSS_FIGHT", 8), 0x5c: ("NO_VICTORY", 8), 0x66: ("NO_WALK_IN", 8), 0x6e: ("FRAME_COUNTER", 16),
    0x0a: ("ALT_CAMERA_EDGES", 8), 0x48e: ("FADE_PAGES", 8), 0x48f: ("FADE_STEPS", 8), 0x490: ("FADE_DELAY", 16),
    0x4ba: ("SCROLL_Y_PLANE1", 16), 0x4be: ("SHAKE", 16), 0x4ff: ("SCENE_STEP", 8), 0x500: ("LANDING_CUE", 8),
    0x51e: ("CAMERA_MODE", 16), 0x35: ("PLAYERS_ALIVE", 8), 0x5a: ("SCENE_LEVEL", 8),
    -0x557c: ("DEFEAT_NAME", 8), -0x557b: ("DEFEAT_FIRST", 8), -0x557a: ("DEFEAT_BONUS", 8),
    -0x55c8: ("SCENE_SEQUENCE", 8), -0x713a: ("END_COUNTDOWN", 8),
}
W = {"8": 8, "16": 16, "32": 32}


def args_end(s, i):
    """Index of the parenthesis closing the call whose arguments start at `i`."""
    depth = 1
    while depth:
        c = s[i]
        depth += c == "("
        depth -= c == ")"
        i += 1
    return i - 1


def fix(text, left, path):
    out = []
    pos = 0
    pat = re.compile(r"x\.(set_)?a5_u(8|16|32)\((0x[0-9a-fA-F]+|\d+)\b|set_abs8\(x, (0x[0-9a-fA-F]+)\b")
    for m in pat.finditer(text):
        if m.start() < pos:
            continue
        if m.group(4):
            off, w, setter = int(m.group(4), 16) - A5, 8, True
        else:
            off, w, setter = int(m.group(3), 0), W[m.group(2)], bool(m.group(1))
        v = VARS.get(off)
        line = text.count("\n", 0, m.start()) + 1
        if v is None or v[1] != w:
            left.append(f"{path}:{line}: {m.group(0)}")
            continue
        end = args_end(text, m.end())
        if setter:
            rest = text[m.end():end]
            assert rest.startswith(", "), (path, line, rest)
            new = f"var::{v[0]}.set(x.b, {rest[2:]})"
        else:
            assert text[m.end():end].strip() == "", (path, line)
            new = f"var::{v[0]}.get(x.b)"
        out.append(text[pos:m.start()] + new)
        pos = end + 1
    out.append(text[pos:])
    return "".join(out)


if __name__ == "__main__":
    apply = sys.argv[1] == "apply"
    for p in sys.argv[2:]:
        left = []
        t = open(p).read()
        n = fix(t, left, p)
        if apply and n != t:
            open(p, "w").write(n)
        for l in left:
            print(l)
