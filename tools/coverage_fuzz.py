#!/usr/bin/env python3
"""What the differential fuzzers (`examples/fuzz_couv`) executed of the original code that the scenarios do not execute
(docs/couverture-fonctions.md, fuzz validation section).

  tools/coverage_fuzz.py <listing.txt> <index.tsv> <atlas.md> <scenario union.bin> <fuzz bitmaps>... [--detail ZONE]

Bitmaps: 1 bit per program word (outputs of `cov` and `fuzz_couv`). For each atlas zone (K4, M1, M2, M3
left out): instructions of the executed functions that the scenarios do not execute, and among them those the fuzz
executes; conditional branches taken one way only by the scenarios, and among them those whose other side the fuzz
takes; functions never executed by the scenarios, and among them those the fuzz executes. `--detail ZONE` lists
per function the instructions and branch sides that remain.
"""
import re
import sys

args = sys.argv[1:]
detail = None
if '--detail' in args:
    i = args.index('--detail')
    detail = args[i + 1]
    del args[i:i + 2]
listing, index, atlas, union_f, *fuzz_f = args
OFF_GAME = {'K4', 'M1', 'M2', 'M3'}
CONDITIONAL = re.compile(r'^(b(eq|ne|pl|mi|cs|cc|gt|ge|lt|le|hi|ls|vc|vs)|db[a-z]{1,2})(\.[bwls])?$')


def load(f):
    return bytearray(open(f, 'rb').read())


union = load(union_f)
fuzz = bytearray(len(union))
for f in fuzz_f:
    for i, x in enumerate(load(f)):
        fuzz[i] |= x


def seen(bm, a):
    w = a >> 1
    return a < 0x100000 and bm[w >> 3] >> (w & 7) & 1


instr = []
for l in open(listing, errors='replace'):
    m = re.match(r'^([0-9a-f]{6})  (\S+)(?: (.*))?$', l.rstrip('\n'))
    if m and int(m[1], 16) < 0x100000:
        target = re.search(r'0x([0-9a-f]{8})\s*$', m[3] or '')
        instr.append((int(m[1], 16), m[2], int(target[1], 16) if target else None))
instr.sort()
zones = [(int(m[1], 16), int(m[2], 16), m[3]) for m in
         (re.match(r'^\| 0x([0-9a-f]{6})-0x([0-9a-f]{6}) \| (\w+) \|', l) for l in open(atlas)) if m]


def zone(a):
    return next((code for d, f, code in zones if d <= a < f), '?')


functions = sorted((int(v[0], 16), v[1], int(v[2])) for v in (l.split('\t') for l in open(index).read().splitlines()[1:]))
totals = {}
remaining = {}
i = 0
for a, name, size in functions:
    z = zone(a)
    if z in OFF_GAME:
        continue
    while i < len(instr) and instr[i][0] < a:
        i += 1
    j = i
    body = []
    while j < len(instr) and instr[j][0] < a + size:
        body.append(j)
        j += 1
    t = totals.setdefault(z, [0] * 8)
    done = [k for k in body if seen(union, instr[k][0])]
    if not done:
        # function never executed by the scenarios
        t[6] += 1
        if any(seen(fuzz, instr[k][0]) for k in body):
            t[7] += 1
        else:
            remaining.setdefault(z, []).append(f'  0x{a:05x} {name}: never executed')
        continue
    never = [k for k in body if not seen(union, instr[k][0])]
    t[0] += len(never)
    t[1] += sum(1 for k in never if seen(fuzz, instr[k][0]))
    missing = []
    for k in body:
        ad, mn, target = instr[k]
        if not CONDITIONAL.match(mn) or not seen(union, ad) or target is None or k + 1 >= len(instr):
            continue
        nxt = instr[k + 1][0]
        if seen(union, target) == seen(union, nxt):
            continue
        t[2] += 1
        side = nxt if seen(union, target) else target
        if seen(fuzz, side):
            t[3] += 1
        else:
            missing.append(f'{ad:x}:{"next" if side == nxt else "target"}')
    left = [instr[k][0] for k in never if not seen(fuzz, instr[k][0])]
    if left or missing:
        remaining.setdefault(z, []).append(f'  0x{a:05x} {name}: {len(left)} instructions ({" ".join(f"{x:x}" for x in left[:12])}{" ..." if len(left) > 12 else ""}); branches {" ".join(missing)}')

print('| Zone | Instructions never executed by the scenarios | of which executed by the fuzz | Branches taken one way only | of which other side taken by the fuzz | Functions never executed | of which executed by the fuzz |')
print('|---|---:|---:|---:|---:|---:|---:|')
tot = [0] * 8
for z, t in sorted(totals.items(), key=lambda kv: -kv[1][0]):
    tot = [x + y for x, y in zip(tot, t)]
    print(f'| {z} | {t[0]} | {t[1]} | {t[2]} | {t[3]} | {t[6]} | {t[7]} |')
print(f'| total | {tot[0]} | {tot[1]} | {tot[2]} | {tot[3]} | {tot[6]} | {tot[7]} |')
if detail:
    print(f'\nRemaining in {detail}:')
    print('\n'.join(remaining.get(detail, [])))
