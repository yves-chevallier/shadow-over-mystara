#!/usr/bin/env python3
"""Génère port/ddsom-game/src/sound_table.rs : description compacte des « wrappers » son du 68000.

Chaque wrapper (0xe0be2-0xede08) enfile une commande dans la file son en suivant un des schémas
reconnus ci-dessous ; les wrappers qui n'en suivent aucun (aiguillages par table de sauts) sont
décrits à la main dans `sound.rs` (`AIGUILLAGES`). Le listing (`listing.txt`, non versionné) est la seule entrée :
    tools/gen_sound_wrappers.py /home/ycr/som/listing.txt > port/ddsom-game/src/sound_table.rs
"""
import re
import sys

LISTING = sys.argv[1] if len(sys.argv) > 1 else "listing.txt"
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

PAN_F, GUARD_F, MEMO_F, DIRECT_F = 1, 2, 4, 8
rows, excluded = [], []
for s in [a for a in order if ins[a] == "movem.l {  A6},-(SP)"]:
    i = order.index(s)
    seq = []
    while True:
        seq.append((order[i], ins[order[i]]))
        if ins[order[i]] == "rts":
            break
        i += 1
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
