#!/usr/bin/env python3
"""Instructions and conditional branches never executed in the original functions that the scenarios touch
(docs/couverture-fonctions.md, branches section).

  tools/coverage_branches.py <listing.txt> <index.tsv> <atlas.md> <natives.txt> <output tsv> <bitmaps>...

Bitmaps: outputs of the `cov` example (1 bit per program word, instruction executed). A conditional branch
(`bcc`, `dbcc`) has two sides: the target and the next instruction; a side is taken if its instruction was
executed (approximation: it may have been by another path). The debug and test zones (K4, M1, M2,
M3) are left out. Writes the table per zone and the 50 most holed functions (Markdown, on stdout), and one
line per function in the TSV.
"""
import re
import sys

listing, index, atlas, natives, output, *bins = sys.argv[1:]
OFF_GAME = {'K4', 'M1', 'M2', 'M3'}
CONDITIONAL = re.compile(r'^(b(eq|ne|pl|mi|cs|cc|gt|ge|lt|le|hi|ls|vc|vs)|db[a-z]{1,2})(\.[bwls])?$')

union = None
for b in bins:
    bm = bytearray(open(b, 'rb').read())
    union = bm if union is None else bytearray(x | y for x, y in zip(union, bm))


def executed(a):
    w = a >> 1
    return a < 0x100000 and union[w >> 3] >> (w & 7) & 1


instr = []  # (address, mnemonic, target)
for l in open(listing, errors='replace'):
    m = re.match(r'^([0-9a-f]{6})  (\S+)(?: (.*))?$', l.rstrip('\n'))
    if m and int(m[1], 16) < 0x100000:
        target = re.search(r'0x([0-9a-f]{8})\s*$', m[3] or '')
        instr.append((int(m[1], 16), m[2], int(target[1], 16) if target else None))
instr.sort()

zones = [(int(m[1], 16), int(m[2], 16), m[3], m[4].strip()) for m in
         (re.match(r'^\| 0x([0-9a-f]{6})-0x([0-9a-f]{6}) \| (\w+) \| ([^|]+)\|', l) for l in open(atlas)) if m]
desc_of = {}
for _, _, code, desc in zones:
    desc_of.setdefault(code, desc)
names = {int(a, 16): n.strip() for a, n in (l.split(maxsplit=1) for l in open(natives))}


def zone(a):
    return next((code for d, f, code, _ in zones if d <= a < f), '?')


functions = []
for l in open(index).read().splitlines()[1:]:
    v = l.split('\t')
    functions.append((int(v[0], 16), v[1], int(v[2])))
functions.sort()

rows = []
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
    done = sum(1 for k in body if executed(instr[k][0]))
    if not done:
        continue
    one_side = []
    for k in body:
        ad, mn, target = instr[k]
        if not CONDITIONAL.match(mn) or not executed(ad) or target is None or k + 1 >= len(instr):
            continue
        taken, fallthrough = executed(target), executed(instr[k + 1][0])
        if taken != fallthrough:
            one_side.append((ad, 'target' if not taken else 'next'))
    rows.append((z, a, name, len(body), done, one_side))

with open(output, 'w') as f:
    f.write('zone\taddress\tname\tnative\tinstructions\texecuted\tnever\tone_sided_branches\n')
    for z, a, name, n, done, one in rows:
        sides = ' '.join(f'{ad:x}:{c}' for ad, c in one)
        f.write(f'{z}\t{a:05x}\t{name}\t{names.get(a, "")}\t{n}\t{done}\t{n - done}\t{sides}\n')

per_zone = {}
for z, a, name, n, done, one in rows:
    t = per_zone.setdefault(z, [0, 0, 0, 0])
    t[0] += 1
    t[1] += n
    t[2] += n - done
    t[3] += len(one)
print('| Zone | Subsystem | Functions executed | Instructions | Never executed | Branches taken one way only |')
print('|---|---|---:|---:|---:|---:|')
for z, (nf, n, never, one) in sorted(per_zone.items(), key=lambda kv: -kv[1][2]):
    print(f'| {z} | {desc_of.get(z, "outside atlas")} | {nf} | {n} | {never} ({100 * never / n:.0f} %) | {one} |')
print('\n| Function | Zone | Native | Instructions | Never executed | Branches taken one way only |')
print('|---|---|---|---:|---:|---:|')
for z, a, name, n, done, one in sorted(rows, key=lambda l: -(l[3] - l[4]))[:50]:
    print(f'| `0x{a:05x}` | {z} | {names.get(a, "")} | {n} | {n - done} | {len(one)} |')
