#!/usr/bin/env python3
"""Ce que les fuzz différentiels (`examples/fuzz_couv`) ont exécuté du code d'origine que les scénarios n'exécutent pas
(docs/couverture-fonctions.md, « Validation par fuzz »).

  tools/couverture_fuzz.py <listing.txt> <index.tsv> <atlas.md> <union des scénarios.bin> <bitmaps de fuzz>... [--detail ZONE]

Bitmaps : 1 bit par mot de programme (sorties de `cov` et de `fuzz_couv`). Pour chaque zone de l'atlas (K4, M1, M2, M3
écartées) : instructions des fonctions exécutées que les scénarios n'exécutent pas, et parmi elles celles que le fuzz
exécute ; sauts conditionnels pris d'un seul côté par les scénarios, et parmi eux ceux dont le fuzz prend l'autre
côté ; fonctions jamais exécutées par les scénarios, et parmi elles celles que le fuzz exécute. `--detail ZONE` liste
par fonction les instructions et côtés de saut qui restent.
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
HORS_JEU = {'K4', 'M1', 'M2', 'M3'}
CONDITIONNEL = re.compile(r'^(b(eq|ne|pl|mi|cs|cc|gt|ge|lt|le|hi|ls|vc|vs)|db[a-z]{1,2})(\.[bwls])?$')


def charger(f):
    return bytearray(open(f, 'rb').read())


union = charger(union_f)
fuzz = bytearray(len(union))
for f in fuzz_f:
    for i, x in enumerate(charger(f)):
        fuzz[i] |= x


def vu(bm, a):
    w = a >> 1
    return a < 0x100000 and bm[w >> 3] >> (w & 7) & 1


instr = []
for l in open(listing, errors='replace'):
    m = re.match(r'^([0-9a-f]{6})  (\S+)(?: (.*))?$', l.rstrip('\n'))
    if m and int(m[1], 16) < 0x100000:
        cible = re.search(r'0x([0-9a-f]{8})\s*$', m[3] or '')
        instr.append((int(m[1], 16), m[2], int(cible[1], 16) if cible else None))
instr.sort()
zones = [(int(m[1], 16), int(m[2], 16), m[3]) for m in
         (re.match(r'^\| 0x([0-9a-f]{6})-0x([0-9a-f]{6}) \| (\w+) \|', l) for l in open(atlas)) if m]


def zone(a):
    return next((code for d, f, code in zones if d <= a < f), '?')


fonctions = sorted((int(v[0], 16), v[1], int(v[2])) for v in (l.split('\t') for l in open(index).read().splitlines()[1:]))
bilan = {}
restes = {}
i = 0
for a, nom, taille in fonctions:
    z = zone(a)
    if z in HORS_JEU:
        continue
    while i < len(instr) and instr[i][0] < a:
        i += 1
    j = i
    corps = []
    while j < len(instr) and instr[j][0] < a + taille:
        corps.append(j)
        j += 1
    t = bilan.setdefault(z, [0] * 8)
    faites = [k for k in corps if vu(union, instr[k][0])]
    if not faites:
        # fonction jamais exécutée par les scénarios
        t[6] += 1
        if any(vu(fuzz, instr[k][0]) for k in corps):
            t[7] += 1
        else:
            restes.setdefault(z, []).append(f'  0x{a:05x} {nom} : jamais exécutée')
        continue
    jamais = [k for k in corps if not vu(union, instr[k][0])]
    t[0] += len(jamais)
    t[1] += sum(1 for k in jamais if vu(fuzz, instr[k][0]))
    manque = []
    for k in corps:
        ad, mn, cible = instr[k]
        if not CONDITIONNEL.match(mn) or not vu(union, ad) or cible is None or k + 1 >= len(instr):
            continue
        suite = instr[k + 1][0]
        if vu(union, cible) == vu(union, suite):
            continue
        t[2] += 1
        cote = suite if vu(union, cible) else cible
        if vu(fuzz, cote):
            t[3] += 1
        else:
            manque.append(f'{ad:x}:{"suite" if cote == suite else "cible"}')
    reste = [instr[k][0] for k in jamais if not vu(fuzz, instr[k][0])]
    if reste or manque:
        restes.setdefault(z, []).append(f'  0x{a:05x} {nom} : {len(reste)} instructions ({" ".join(f"{x:x}" for x in reste[:12])}{" ..." if len(reste) > 12 else ""}) ; sauts {" ".join(manque)}')

print('| Zone | Instructions jamais exécutées par les scénarios | dont exécutées par le fuzz | Sauts pris d\'un seul côté | dont autre côté pris par le fuzz | Fonctions jamais exécutées | dont exécutées par le fuzz |')
print('|---|---:|---:|---:|---:|---:|---:|')
tot = [0] * 8
for z, t in sorted(bilan.items(), key=lambda kv: -kv[1][0]):
    tot = [x + y for x, y in zip(tot, t)]
    print(f'| {z} | {t[0]} | {t[1]} | {t[2]} | {t[3]} | {t[6]} | {t[7]} |')
print(f'| total | {tot[0]} | {tot[1]} | {tot[2]} | {tot[3]} | {tot[6]} | {tot[7]} |')
if detail:
    print(f'\nReste en {detail} :')
    print('\n'.join(restes.get(detail, [])))
