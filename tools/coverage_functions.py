#!/usr/bin/env python3
"""Original functions never executed by the scenarios, grouped by atlas subsystem.

  tools/coverage_functions.py <index.tsv> <atlas.md> <natives.txt> <code-mort.md> <bitmap.bin>...
"""
import re
import sys

index, atlas, natives, dead, *bins = sys.argv[1:]

union = None
for b in bins:
    bm = bytearray(open(b, 'rb').read())
    union = bm if union is None else bytearray(x | y for x, y in zip(union, bm))


def touched(a, size):
    return any(union[(x >> 1) >> 3] >> ((x >> 1) & 7) & 1 for x in range(a, a + size, 2))


functions = []
for l in open(index).read().splitlines()[1:]:
    v = l.split('\t')
    functions.append((int(v[0], 16), v[1], int(v[2])))
zones = []
for l in open(atlas):
    m = re.match(r'^\| 0x([0-9a-f]{6})-0x([0-9a-f]{6}) \| (\w+) \| ([^|]+)\|', l)
    if m:
        zones.append((int(m[1], 16), int(m[2], 16), m[3], m[4].strip()))
first, desc_of = {}, {}
for d, f, code, desc in zones:
    first.setdefault(code, d)
    desc_of.setdefault(code, desc)
names = {}
for l in open(natives):
    a, n = l.split(maxsplit=1)
    names[int(a, 16)] = n.strip()
dead_addrs = {int(x, 16) for x in re.findall(r'0x([0-9a-f]{4,6})\b', open(dead).read())}


def zone(a):
    for d, f, code, desc in zones:
        if d <= a < f:
            return (first[code], code, desc_of[code])
    return (0x1000000, '?', 'outside atlas')


per_zone = {}
for a, name, size in sorted(functions):
    z = zone(a)
    t = per_zone.setdefault(z, [0, []])
    t[0] += 1
    if not touched(a, size):
        t[1].append((a, name, size))

total = len(functions)
missed = sum(len(v[1]) for v in per_zone.values())
print(f'# {total - missed} functions executed out of {total}; {missed} never executed\n')
print('| Zone | Subsystem | Functions | Never executed | of which native entries |')
print('|---|---|---:|---:|---:|')
for (d, code, desc), (n, m) in sorted(per_zone.items()):
    print(f'| {code} | {desc} | {n} | {len(m)} | {sum(1 for x in m if x[0] in names)} |')
print()
for (d, code, desc), (n, m) in sorted(per_zone.items()):
    if not m:
        continue
    print(f'## {code}: {desc} ({len(m)} of {n})\n')
    for a, name, size in m:
        notes = []
        if a in names:
            notes.append(f'native `{names[a]}`')
        if a in dead_addrs:
            notes.append('code-mort.md')
        print(f'* `0x{a:05x}` {name} ({size} B)' + (': ' + ', '.join(notes) if notes else ''))
    print()
