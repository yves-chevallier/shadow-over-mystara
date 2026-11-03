#!/usr/bin/env python3
"""Replaces literal offsets of the common enemy fields by the names of `Enemy` / `Obj` in the given files.

Only calls `recv.meth(off ...)` / `recv.meth(bus, off ...)` whose offset is in the map at that width; the rest is
printed for manual work.
"""
import re
import sys

# (offset, width) -> name; width 8/16/32, 'bit' for set_bit/bit, 'link' for linked
E = {
    (0x00, 8): "Enemy::ACTIVE",
    (0x01, 8): "Enemy::DISPLAY", (0x01, 'bit'): "Enemy::DISPLAY",
    (0x02, 8): "Enemy::KIND",
    (0x03, 8): "Enemy::VARIANT",
    (0x04, 8): "Enemy::STATE", (0x04, 16): "Enemy::STATE_WORD", (0x04, 32): "Enemy::STATE_LONG",
    (0x05, 8): "Enemy::SUB",
    (0x06, 8): "Enemy::STEP", (0x06, 16): "Enemy::STEP_WORD",
    (0x07, 8): "Enemy::SUB_STEP",
    (0x08, 16): "Obj::X", (0x08, 32): "Obj::X", (0x08, 8): "Obj::X",
    (0x0a, 16): "Enemy::X_SAVED",
    (0x0c, 16): "Obj::Z", (0x0c, 32): "Obj::Z", (0x0c, 8): "Obj::Z",
    (0x10, 16): "Obj::Y", (0x10, 32): "Obj::Y", (0x10, 8): "Obj::Y",
    (0x14, 16): "Enemy::VX", (0x14, 32): "Enemy::VX_LONG", (0x14, 8): "Enemy::VX",
    (0x16, 16): "Enemy::AX", (0x16, 32): "Enemy::AX_LONG", (0x16, 8): "Enemy::AX",
    (0x18, 16): "Enemy::VZ", (0x18, 32): "Enemy::VZ_LONG", (0x18, 8): "Enemy::VZ",
    (0x1a, 16): "Enemy::GRAVITY",
    (0x1c, 16): "Enemy::VY", (0x1c, 32): "Enemy::VY_LONG", (0x1c, 8): "Enemy::VY",
    (0x1e, 16): "Enemy::AY",
    (0x28, 8): "Enemy::CLASS",
    (0x29, 8): "Enemy::BLOCKED",
    (0x2a, 8): "Enemy::OWN_PALETTE", (0x2a, 16): "Enemy::PARTS",
    (0x2b, 8): "Enemy::PALETTE",
    (0x2c, 8): "Enemy::FLIP",
    (0x2d, 8): "Enemy::HEADING",
    (0x2e, 8): "Enemy::BOX_SET",
    (0x2f, 8): "Enemy::GROUP",
    (0x20, 32): "Obj::ANIM_NEXT",
    (0x30, 16): "Obj::ANIM_TIMER",
    (0x34, 32): "Obj::FRAME",
    (0x38, 8): "Enemy::FRAME_TAG",
    (0x40, 8): "Obj::HIDDEN",
    (0x4c, 32): "Obj::SPEED_TABLE",
    (0x32, 16): "Enemy::ANIM_END", (0x32, 8): "Enemy::ANIM_END_BYTE",
    (0x33, 8): "Enemy::KEY_FRAME",
    (0x3e, 8): "Enemy::STRIKE_KEY",
    (0x42, 16): "Enemy::CREATOR", (0x42, 'link'): "Enemy::CREATOR",
    (0x44, 8): "Enemy::STRIKE_KIND",
    (0x45, 8): "Enemy::STRIKE_POWER",
    (0x46, 16): "Enemy::HOST", (0x46, 'link'): "Enemy::HOST",
    (0x49, 8): "Enemy::REACTION",
    (0x4a, 8): "Enemy::ATTACKER_FLIP",
    (0x50, 16): "Enemy::TARGET", (0x50, 32): "Enemy::TARGET_LONG", (0x50, 'link'): "Enemy::TARGET",
    (0x53, 8): "Enemy::FREEZE",
    (0x54, 16): "Enemy::HOLDER", (0x54, 'link'): "Enemy::HOLDER",
    (0x56, 8): "Enemy::STUN",
    (0x58, 32): "Enemy::HIT_LOCK",
    (0x5c, 16): "Enemy::SPAWNER", (0x5c, 'link'): "Enemy::SPAWNER",
    (0x60, 16): "Enemy::HP_MAX",
    (0x62, 16): "Enemy::HP",
    (0x64, 16): "Enemy::HP_LAST",
    (0x67, 8): "Enemy::VULNERABLE",
    (0x68, 8): "Enemy::AI_68",
    (0x69, 8): "Enemy::AI_69",
    (0x6a, 8): "Enemy::AI_WANDER_ROW",
    (0x6b, 8): "Enemy::AI_WANDER",
    (0x6c, 8): "Enemy::AI_WAIT_MASKS",
    (0x6d, 8): "Enemy::AI_WAIT_ROW",
    (0x6e, 8): "Enemy::AI_WAIT",
    (0x70, 8): "Enemy::AI_STANCE_ROW",
    (0x71, 8): "Enemy::AI_STANCE",
    (0x72, 8): "Enemy::AI_DECISION_A",
    (0x73, 8): "Enemy::AI_DECISION_B",
    (0x74, 8): "Enemy::AI_BOX_CHANCE",
    (0x75, 8): "Enemy::AI_BOX_ANIM",
    (0x76, 8): "Enemy::AI_BYTE_ROW",
    (0x77, 8): "Enemy::AI_BYTE",
    (0x78, 8): "Enemy::AI_SHOT",
    (0x79, 8): "Enemy::AI_POINT_SET",
    (0x7e, 8): "Enemy::EVENT_KIND",
    (0x7f, 8): "Enemy::EVENT_LOW",
    (0x80, 8): "Enemy::LOOT_ROW",
    (0x81, 8): "Enemy::LOOT",
    (0x84, 16): "Enemy::RECOVER",
    (0x86, 8): "Enemy::DOWNED",
    (0x87, 8): "Enemy::GRABBED",
    (0x8a, 32): "Enemy::RESUME_STATE", (0x8a, 16): "Enemy::RESUME_WORD",
    (0x8e, 8): "Enemy::AI_ROW_8E",
    (0x90, 8): "Enemy::COUNTER",
    (0x91, 8): "Enemy::PENDING_PALETTE",
    (0x94, 32): "Enemy::IMMUNITY",
    (0xbf, 8): "Enemy::ANIM_VARIANT",
}
BOSS = {
    (0x6b, 8): "Boss::WALK_STEPS",
    (0x6e, 8): "Boss::IDLE_COUNT",
    (0x71, 8): "Boss::WALK_DELAY",
    (0x79, 8): "Boss::SHOT",
    (0x84, 16): "Boss::DOWN_TIME",
    (0x90, 8): "Boss::SHAKE",
    (0x93, 8): "Boss::DEATH_CREDIT",
    (0x9a, 8): "Boss::DEATH_VALUE",
}
import os
if os.environ.get("BOSS_FIELDS"):
    E.update(BOSS)
WIDTH = {}
for m in ["u8", "i8", "set8", "set_u8", "add8"]:
    WIDTH[m] = 8
for m in ["u16", "i16", "set16", "set_u16", "add16"]:
    WIDTH[m] = 16
for m in ["u32", "i32", "set32", "set_u32", "add32"]:
    WIDTH[m] = 32
WIDTH["set_bit"] = WIDTH["bit"] = 'bit'
WIDTH["linked"] = 'link'
METHODS = "|".join(sorted(WIDTH, key=len, reverse=True))
CALL = re.compile(rf"\.({METHODS})\(((?:[a-z_][\w.]*, *)?)(0x[0-9a-fA-F_]+|\d+)\b(?!\s*[+\-*])")
# tuples `(0x68, 2)` of the init lists
TUPLE = re.compile(r"^(\s*)\((0x[0-9a-fA-F]+), (0x[0-9a-fA-F]+|\d+)\),\s*$")


def fix(path, apply):
    src = open(path).read().split("\n")
    out = []
    left = []
    in_init_list = False
    for n, line in enumerate(src, 1):
        code, sep, comment = line.partition("//")

        def sub(m):
            meth, pre, lit = m.groups()
            key = (int(lit, 0), WIDTH[meth])
            name = E.get(key)
            if name is None:
                left.append(f"{path}:{n}: {line.strip()}")
                return m.group(0)
            return f".{meth}({pre}{name}"

        code = CALL.sub(sub, code)
        if "for (off, v) in [" in code:
            in_init_list = True
        if in_init_list:
            def tup(m):
                key = (int(m.group(1), 0), 8)
                if key in E:
                    return f"({E[key]}, "
                left.append(f"{path}:{n}: {line.strip()} (init)")
                return m.group(0)
            code = re.sub(r"\((0x[0-9a-fA-F]+), ", tup, code)
            if "] {" in code:
                in_init_list = False
        out.append(code + sep + comment)
    if apply:
        open(path, "w").write("\n".join(out))
    return left


if __name__ == "__main__":
    apply = sys.argv[1] == "apply"
    for p in sys.argv[2:]:
        for l in fix(p, apply):
            print(l)
