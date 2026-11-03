#!/usr/bin/env python3
"""Rewrites the state dispatches through PC-relative jump tables (`dispatch(x, 0xT, FIELD)`, `step(x, 0xT)`) of a
boss file into matches on the index byte: each arm `0xADDR =>` becomes the indices whose entry jumps to ADDR.

Usage: jt.py STEP_FIELD FILE...  (STEP_FIELD: the field that the file's local `step(x, table)` reads, e.g. f::SUB3)
The number of entries of a table: up to its first target (the code follows the table), at most 32.
"""
import re
import struct
import sys

import os
PROG = open(os.environ.get('PROG_DEC', '/home/ycr/som/prog_dec.bin'), 'rb').read()


def entries(t):
    out = []
    first = None
    for i in range(64):
        a = t + 2 * i
        if first is not None and a >= first:
            break
        tgt = t + struct.unpack('>h', PROG[a:a + 2])[0]
        out.append(tgt)
        if tgt > t:
            first = tgt if first is None else min(first, tgt)
    return out


CALL = re.compile(r"(?:dispatch\(x, (0x[0-9a-f]+), ([\w:]+)\)|\bstep\(x, (0x[0-9a-f]+)\))")


def index_pattern(idxs):
    return " | ".join(str(2 * i) if 2 * i < 10 else hex(2 * i) for i in idxs)


def rewrite(src, step_field):
    out = []
    pos = 0
    report = []
    for m in CALL.finditer(src):
        if m.start() < pos:
            continue
        t = int(m.group(1) or m.group(3), 16)
        field = m.group(2) or step_field
        tab = entries(t)
        by_target = {}
        for i, a in enumerate(tab):
            by_target.setdefault(a, []).append(i)
        # what follows the call
        rest = src[m.end():]
        out.append(src[pos:m.start()])
        call_repl = f"x.u8({field})"
        mm = re.match(r"\s*\{", rest)
        before = src[m.start() - 6:m.start()]
        if mm and before.endswith("match "):
            # parse the match block, replace top-level arm patterns
            depth = 0
            j = m.end()
            start = None
            arm_start = True
            k = j
            body = []
            while True:
                c = src[k]
                if c == '{':
                    depth += 1
                    if depth == 1:
                        start = k
                elif c == '}':
                    depth -= 1
                    if depth == 0:
                        break
                k += 1
            block = src[m.end():k + 1]
            # replace arm patterns at depth 1: lines starting with `0x....(| 0x...)* =>`
            def arm(mo):
                addrs = [int(a, 16) for a in re.findall(r"0x[0-9a-f]+", mo.group(2))]
                idxs = []
                for a in addrs:
                    if a not in by_target:
                        raise SystemExit(f"table {t:#x}: target {a:#x} not in {[hex(v) for v in tab]}")
                    idxs += by_target[a]
                return mo.group(1) + index_pattern(sorted(idxs)) + " =>"
            # only the arms at depth 1 (nested matches are handled by a later pass)
            pieces, d, last = [], 0, 0
            for ci, c in enumerate(block):
                if c == '{':
                    d += 1
                elif c == '}':
                    d -= 1
                elif c == '\n' and d == 1:
                    mo = re.match(r"(\n\s*)((?:0x[0-9a-f]+)(?:\s*\|\s*0x[0-9a-f]+)*)\s*=>", block[ci:])
                    if mo:
                        pieces.append(block[last:ci])
                        pieces.append(arm(mo))
                        last = ci + mo.end()
            pieces.append(block[last:])
            new_block = "".join(pieces)
            out.append(call_repl + new_block)
            pos = k + 1
            report.append(f"{t:#x} {field}: {[hex(v) for v in tab]}")
        else:
            cmp = re.match(r"\s*(==|!=)\s*(0x[0-9a-f]+)", rest)
            if cmp:
                a = int(cmp.group(2), 16)
                idxs = by_target[a]
                if len(idxs) == 1:
                    out.append(f"{call_repl} {cmp.group(1)} {index_pattern(idxs)}")
                else:
                    neg = "!" if cmp.group(1) == "!=" else ""
                    out.append(f"{neg}matches!({call_repl}, {index_pattern(idxs)})")
                pos = m.end() + cmp.end()
                report.append(f"{t:#x} {field} cmp: {[hex(v) for v in tab]}")
            else:
                out.append(src[m.start():m.end()])
                pos = m.end()
                report.append(f"UNHANDLED {t:#x} at {src[m.start():m.start()+80]!r}")
    out.append(src[pos:])
    return "".join(out), report


if __name__ == "__main__":
    step_field = sys.argv[1]
    for p in sys.argv[2:]:
        src = open(p).read()
        while True:
            new, rep = rewrite(src, step_field)
            for r in rep:
                print(p, r)
            if new == src:
                break
            src = new
        open(p, "w").write(new)
