#!/usr/bin/env python3
"""Génère port/ddsom-game/src/sound_table.rs : description compacte des « wrappers » son du 68000.

Chaque wrapper (0xe0be2-0xede08) enfile une commande dans la file son en suivant un des schémas
reconnus ci-dessous ; les wrappers qui n'en suivent aucun (aiguillages par table de sauts) sont
décrits à la main dans `sound.rs` (`AIGUILLAGES`). Entrées : le listing (`listing.txt`, non versionné) et,
pour les wrappers tombés dans ses trous, le désassembleur de l'interpréteur (`examples/dis`), dont la sortie
est traduite dans la syntaxe du listing pour les seules instructions des wrappers :
    tools/gen_sound_wrappers.py /home/ycr/som/listing.txt [prog_dec.bin dis roms] > port/ddsom-game/src/sound_table.rs
"""
import re
import subprocess
import sys

LISTING = sys.argv[1] if len(sys.argv) > 1 else "listing.txt"
PROG, DIS, ROMS = (sys.argv[2:5] + [None] * 3)[:3]
ins, order = {}, []
for line in open(LISTING):
    m = re.match(r"^([0-9a-f]{6})  (.*)$", line)
    if m:
        a = int(m.group(1), 16)
        if 0xE0836 <= a <= 0xEE080:
            ins[a] = m.group(2)
            order.append(a)


def norm(t):
    return re.sub(r"-?0x[0-9a-f]+", "N", t)


def num(s):
    return int(s, 16) if not s.startswith("-") else -int(s[1:], 16)


# calcul du panoramique depuis la position X de l'objet A0 (17 instructions identiques)
PAN = ["move.w (0x8,A0),D0w", "move.w (0x5a6,A5),D4w", "tst.b (0x36,A5)", "beq.b N", "move.w (0x5ae,A5),D4w",
       "sub.w D4w,D0w", "bpl.b N", "moveq 0x0,D0", "cmpi.w #0x180,D0w", "bcs.b N", "move.w #0x178,D0w",
       "movea.l #0xee04e,A1", "andi.w #0x1ff,D0w", "lsr.w #0x3,D0w", "move.b (0x0,A1,D0w*0x1),D0b",
       "lsl.w #0x8,D0w", "or.w D0w,D2w"]
TBL = r"move\.w \((0x[0-9a-f]+),PC,D1w\*0x1\),D1w$"

PROLOGUE = bytes.fromhex("48e700022c6daa1a")  # movem.l {A6},-(SP) ; movea.l (-0x55e6,A5),A6


def hx(v):
    return ("-0x%x" % -v) if v < 0 else "0x%x" % v


def listing_text(op, size):
    """Instruction de `dis` (Debug) dans la syntaxe du listing ; None si hors des schémas des wrappers."""
    sfx = ".b" if size == 2 else ".w"
    cc = {5: "bcs", 6: "bne", 7: "beq", 10: "bpl"}
    m = re.fullmatch(r"(Bsr|Bra)\((\d+)\)", op)
    if m:
        return "%s.w 0x%08x" % (m.group(1).lower(), int(m.group(2)))
    m = re.fullmatch(r"Bcc\((\d+), (\d+)\)", op)
    if m and int(m.group(1)) in cc:
        return "%s%s 0x%08x" % (cc[int(m.group(1))], sfx, int(m.group(2)))
    m = re.fullmatch(r"Jsr\((?:AbsL|PcDisp)\((\d+)\)\)", op)
    if m:
        return "jsr 0x%08x.l" % int(m.group(1))
    simple = {
        "Nop": "nop", "Rts": "rts", "Swap(0)": "swap D0",
        "Shift { kind: Ls, left: true, size: Word, count: Imm(8), dn: 0 }": "lsl.w #0x8,D0w",
        "Shift { kind: Ls, left: false, size: Word, count: Imm(3), dn: 0 }": "lsr.w #0x3,D0w",
        "Or(Long, Dn(0), Dn(2))": "or.l D0,D2", "Or(Word, Dn(0), Dn(2))": "or.w D0w,D2w",
        "Or(Long, Dn(0), Dn(3))": "or.l D0,D3", "Tst(Byte, Disp(5, -21986))": "tst.b (-0x55e2,A5)",
        "Move(Word, Dn(1), Disp(5, 32708))": "move.w D1w,(0x7fc4,A5)", "Moveq(0, 1)": "moveq 0x0,D1",
        "Move(Byte, Disp(0, 2), Dn(1))": "move.b (0x2,A0),D1b",
        "Move(Word, Disp(5, 32708), Dn(1))": "move.w (0x7fc4,A5),D1w", "Move(Word, Dn(4), Dn(1))": "move.w D4w,D1w", "Add(Word, Dn(1), Dn(1))": "add.w D1w,D1w",
        "Move(Word, Disp(0, 8), Dn(0))": "move.w (0x8,A0),D0w", "Move(Word, Disp(5, 1446), Dn(4))": "move.w (0x5a6,A5),D4w",
        "Tst(Byte, Disp(5, 54))": "tst.b (0x36,A5)", "Move(Word, Disp(5, 1454), Dn(4))": "move.w (0x5ae,A5),D4w",
        "Sub(Word, Dn(4), Dn(0))": "sub.w D4w,D0w", "Cmp(Word, Imm(384), Dn(0))": "cmpi.w #0x180,D0w",
        "Move(Word, Imm(376), Dn(0))": "move.w #0x178,D0w", "Movea(Long, Imm(974926), 1)": "movea.l #0xee04e,A1",
        "And(Word, Imm(511), Dn(0))": "andi.w #0x1ff,D0w", "Move(Byte, Idx(1, 0), Dn(0))": "move.b (0x0,A1,D0w*0x1),D0b",
    }
    if op in simple:
        return simple[op]
    m = re.fullmatch(r"Moveq\((-?\d+), 0\)", op)
    if m:
        return "moveq %s,D0" % hx(int(m.group(1)))
    m = re.fullmatch(r"Move\(Long, Imm\((\d+)\), Dn\(0\)\)", op)
    if m:
        return "move.l #0x%x,D0" % int(m.group(1))
    m = re.fullmatch(r"And\(Long, Imm\((\d+)\), Dn\(2\)\)", op)
    if m:
        return "andi.l #0x%x,D2" % int(m.group(1))
    m = re.fullmatch(r"Move\(Word, Imm\((\d+)\), Dn\(1\)\)", op)
    if m:
        return "move.w #0x%x,D1w" % int(m.group(1))
    m = re.fullmatch(r"Move\(Word, PcIdx\((\d+), 4176\), Dn\(1\)\)", op)
    if m:
        return "move.w (0x%x,PC,D1w*0x1),D1w" % int(m.group(1))
    return None


def disassemble(start):
    """Wrapper absent du listing : désassemblé par `examples/dis` jusqu'à son `rts`."""
    out = subprocess.run([DIS, ROMS, "%x" % start, "%x" % (start + 0x100)], capture_output=True, text=True).stdout
    lines = [l.split(None, 2) for l in out.splitlines()]
    seq = []
    for k, (a, _, op) in enumerate(lines):
        a = int(a, 16)
        size = int(lines[k + 1][0], 16) - a if k + 1 < len(lines) else 2
        seq.append((a, listing_text(op, size) or "?? " + op))
        if op == "Rts":
            return seq
    return seq


PAN_F, GUARD_F, MEMO_F, DIRECT_F = 1, 2, 4, 8
rows, excluded = [], []
starts = [a for a in order if ins[a] == "movem.l {  A6},-(SP)"]
if PROG:
    prog = open(PROG, "rb").read()
    found, i = [], 0xE0836
    while (j := prog.find(PROLOGUE, i, 0xEE080)) >= 0:
        found.append(j)
        i = j + 2
    starts = sorted(set(starts) | set(found))
for s in starts:
    if ins.get(s) == "movem.l {  A6},-(SP)":
        i = order.index(s)
        seq = []
        while True:
            seq.append((order[i], ins[order[i]]))
            if ins[order[i]] == "rts":
                break
            i += 1
    else:
        seq = disassemble(s)
    body = seq[8:-6]  # sans prologue (sauvegarde des registres, D0-D3 = 0) ni épilogue
    t_ = [t for _, t in body]
    d0 = d2 = d3 = 0
    flags, sel, tbl, cmd = 0, None, None, None
    ok, pan_pos, pos = True, None, 0
    while pos < len(body) and ok:
        t = t_[pos]
        if [norm(x) for x in t_[pos:pos + 17]] == [norm(x) for x in PAN]:
            flags |= PAN_F
            pan_pos = pos
            pos += 17
        elif re.match(r"(bsr\.w|jsr) 0x000edee4(\.l)?$", t):
            sel, pos = "alt", pos + 1
        elif re.match(r"jsr 0x000edef2(\.l)?$", t):
            sel, pos = "obj", pos + 1
        elif t == "nop":
            pos += 1
        elif m := re.match(r"moveq (-?0x[0-9a-f]+),D0$", t):
            d0, pos = num(m.group(1)) & 0xFFFFFFFF, pos + 1
        elif m := re.match(r"move\.l #(-?0x[0-9a-f]+),D0$", t):
            d0, pos = num(m.group(1)) & 0xFFFFFFFF, pos + 1
        elif t == "swap D0":
            d0, pos = ((d0 & 0xFFFF) << 16) | (d0 >> 16), pos + 1
        elif t == "lsl.w #0x8,D0w":
            d0, pos = (d0 & 0xFFFF0000) | ((d0 << 8) & 0xFFFF), pos + 1
        elif m := re.match(r"andi\.l #(-?0x[0-9a-f]+),D2$", t):
            d2, pos = d2 & (num(m.group(1)) & 0xFFFFFFFF), pos + 1
        elif t == "or.l D0,D2":
            d2, pos = d2 | d0, pos + 1
        elif t == "or.w D0w,D2w":
            d2, pos = d2 | (d0 & 0xFFFF), pos + 1
        elif t == "or.l D0,D3":
            d3, pos = d3 | d0, pos + 1
        elif m := re.match(r"move\.w #(-?0x[0-9a-f]+),D1w$", t):
            cmd, pos = num(m.group(1)) & 0xFFFF, pos + 1
        elif m := re.match(TBL, t):
            tbl, pos = num(m.group(1)), pos + 1
        elif t_[pos:pos + 3] == ["moveq 0x0,D1", "move.b (0x2,A0),D1b", "add.w D1w,D1w"] and re.match(TBL, t_[pos + 3]):
            sel, tbl, pos = "oct2", num(re.match(TBL, t_[pos + 3]).group(1)), pos + 4
        elif t == "tst.b (-0x55e2,A5)" and t_[pos + 1].startswith("bne.w"):
            flags, pos = flags | GUARD_F, pos + 2
        elif t == "move.w (0x7fc4,A5),D1w":
            cmd, pos = "Memo", pos + 1
        elif t == "move.w D4w,D1w":
            cmd, pos = "DuD4", pos + 1
        elif t == "move.w D1w,(0x7fc4,A5)":
            flags, pos = flags | MEMO_F, pos + 1
        else:
            ok = False
    m = re.match(r"jsr 0x000e0(836|848)\.l$", seq[-6][1])
    if not ok or not m or (cmd is None and tbl is None):
        excluded.append(s)
        continue
    if m.group(1) == "848":
        flags |= DIRECT_F
    if pan_pos is not None and any(",D2" in t for t in t_[pan_pos + 17:]):
        excluded.append(s)
        continue
    if tbl is not None:
        c = {"alt": "Alternee", "obj": "ParObjet", "oct2": "ParOctet2"}[sel] + f"(0x{tbl:x})"
    elif isinstance(cmd, str):
        c = cmd
    else:
        c = f"Fixe(0x{cmd:x})"
    rows.append((s, c, d2, d3, flags))

out = ["// Fichier généré par tools/gen_sound_wrappers.py : ne pas modifier à la main.",
       "// Un wrapper son par ligne : (entrée, commande, D2, D3, options).",
       "use super::{Cmd::*, Wrapper, DIRECT, GARDE, MEMO, PAN};", "",
       "const fn w(entry: u32, cmd: super::Cmd, d2: u32, d3: u32, opts: u8) -> Wrapper {",
       "    Wrapper { entry, cmd, d2, d3, opts }", "}", "",
       f"/// {len(rows)} wrappers décrits ; les autres ({len(excluded)}) sont des aiguillages (`sound::AIGUILLAGES`).",
       "pub const WRAPPERS: &[Wrapper] = &["]
names = [(PAN_F, "PAN"), (GUARD_F, "GARDE"), (MEMO_F, "MEMO"), (DIRECT_F, "DIRECT")]
for s, c, d2, d3, f in rows:
    o = " | ".join(n for b, n in names if f & b) or "0"
    out.append(f"    w(0x{s:x}, {c}, 0x{d2:08x}, 0x{d3:08x}, {o}),")
out.append("];")
print("\n".join(out))
print("exclus : " + " ".join(f"{a:x}" for a in excluded), file=sys.stderr)
