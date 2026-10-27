#!/usr/bin/env python3
"""Fonctions d'origine jamais exécutées par les scénarios, rangées par sous-système de l'atlas.

  tools/couverture_fonctions.py <index.tsv> <atlas.md> <natives.txt> <code-mort.md> <bitmap.bin>...
"""
import re
import sys

index, atlas, natives, mort, *bins = sys.argv[1:]

union = None
for b in bins:
    bm = bytearray(open(b, 'rb').read())
    union = bm if union is None else bytearray(x | y for x, y in zip(union, bm))


def touche(a, taille):
    return any(union[(x >> 1) >> 3] >> ((x >> 1) & 7) & 1 for x in range(a, a + taille, 2))


fonctions = []
for l in open(index).read().splitlines()[1:]:
    v = l.split('\t')
    fonctions.append((int(v[0], 16), v[1], int(v[2])))
zones = []
for l in open(atlas):
    m = re.match(r'^\| 0x([0-9a-f]{6})-0x([0-9a-f]{6}) \| (\w+) \| ([^|]+)\|', l)
    if m:
        zones.append((int(m[1], 16), int(m[2], 16), m[3], m[4].strip()))
premier, desc_de = {}, {}
for d, f, code, desc in zones:
    premier.setdefault(code, d)
    desc_de.setdefault(code, desc)
noms = {}
for l in open(natives):
    a, n = l.split(maxsplit=1)
    noms[int(a, 16)] = n.strip()
morts = {int(x, 16) for x in re.findall(r'0x([0-9a-f]{4,6})\b', open(mort).read())}


def zone(a):
    for d, f, code, desc in zones:
        if d <= a < f:
            return (premier[code], code, desc_de[code])
    return (0x1000000, '?', 'hors atlas')


par_zone = {}
for a, nom, taille in sorted(fonctions):
    z = zone(a)
    t = par_zone.setdefault(z, [0, []])
    t[0] += 1
    if not touche(a, taille):
        t[1].append((a, nom, taille))

total = len(fonctions)
manquees = sum(len(v[1]) for v in par_zone.values())
print(f'# {total - manquees} fonctions exécutées sur {total} ; {manquees} jamais exécutées\n')
print('| Zone | Sous-système | Fonctions | Jamais exécutées | dont entrées de natives |')
print('|---|---|---:|---:|---:|')
for (d, code, desc), (n, m) in sorted(par_zone.items()):
    print(f'| {code} | {desc} | {n} | {len(m)} | {sum(1 for x in m if x[0] in noms)} |')
print()
for (d, code, desc), (n, m) in sorted(par_zone.items()):
    if not m:
        continue
    print(f'## {code} : {desc} ({len(m)} sur {n})\n')
    for a, nom, taille in m:
        notes = []
        if a in noms:
            notes.append(f'native `{noms[a]}`')
        if a in morts:
            notes.append('code-mort.md')
        print(f'* `0x{a:05x}` {nom} ({taille} o)' + (' : ' + ', '.join(notes) if notes else ''))
    print()
