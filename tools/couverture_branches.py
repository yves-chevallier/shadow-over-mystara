#!/usr/bin/env python3
"""Instructions et sauts conditionnels jamais exécutés dans les fonctions d'origine que les scénarios touchent
(docs/couverture-fonctions.md, « Branches »).

  tools/couverture_branches.py <listing.txt> <index.tsv> <atlas.md> <natives.txt> <tsv de sortie> <bitmaps>...

Bitmaps : sorties de l'exemple `cov` (1 bit par mot de programme, instruction exécutée). Un saut conditionnel
(`bcc`, `dbcc`) a deux côtés : la cible et l'instruction suivante ; un côté est pris si son instruction a été
exécutée (approximation : elle peut l'avoir été par un autre chemin). Les zones de débogage et de test (K4, M1, M2,
M3) sont écartées. Écrit le tableau par zone et les 50 fonctions les plus trouées (Markdown, sur la sortie), et une
ligne par fonction dans le TSV.
"""
import re
import sys

listing, index, atlas, natives, sortie, *bins = sys.argv[1:]
HORS_JEU = {'K4', 'M1', 'M2', 'M3'}
CONDITIONNEL = re.compile(r'^(b(eq|ne|pl|mi|cs|cc|gt|ge|lt|le|hi|ls|vc|vs)|db[a-z]{1,2})(\.[bwls])?$')

union = None
for b in bins:
    bm = bytearray(open(b, 'rb').read())
    union = bm if union is None else bytearray(x | y for x, y in zip(union, bm))


def execute(a):
    w = a >> 1
    return a < 0x100000 and union[w >> 3] >> (w & 7) & 1


instr = []  # (adresse, mnémonique, cible)
for l in open(listing, errors='replace'):
    m = re.match(r'^([0-9a-f]{6})  (\S+)(?: (.*))?$', l.rstrip('\n'))
    if m and int(m[1], 16) < 0x100000:
        cible = re.search(r'0x([0-9a-f]{8})\s*$', m[3] or '')
        instr.append((int(m[1], 16), m[2], int(cible[1], 16) if cible else None))
instr.sort()

zones = [(int(m[1], 16), int(m[2], 16), m[3], m[4].strip()) for m in
         (re.match(r'^\| 0x([0-9a-f]{6})-0x([0-9a-f]{6}) \| (\w+) \| ([^|]+)\|', l) for l in open(atlas)) if m]
desc_de = {}
for _, _, code, desc in zones:
    desc_de.setdefault(code, desc)
noms = {int(a, 16): n.strip() for a, n in (l.split(maxsplit=1) for l in open(natives))}


def zone(a):
    return next((code for d, f, code, _ in zones if d <= a < f), '?')


fonctions = []
for l in open(index).read().splitlines()[1:]:
    v = l.split('\t')
    fonctions.append((int(v[0], 16), v[1], int(v[2])))
fonctions.sort()

lignes = []
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
    faites = sum(1 for k in corps if execute(instr[k][0]))
    if not faites:
        continue
    un_cote = []
    for k in corps:
        ad, mn, cible = instr[k]
        if not CONDITIONNEL.match(mn) or not execute(ad) or cible is None or k + 1 >= len(instr):
            continue
        pris, suite = execute(cible), execute(instr[k + 1][0])
        if pris != suite:
            un_cote.append((ad, 'cible' if not pris else 'suite'))
    lignes.append((z, a, nom, len(corps), faites, un_cote))

with open(sortie, 'w') as f:
    f.write('zone\tadresse\tnom\tnative\tinstructions\texécutées\tjamais\tsauts_un_seul_côté\n')
    for z, a, nom, n, faites, un in lignes:
        cotes = ' '.join(f'{ad:x}:{c}' for ad, c in un)
        f.write(f'{z}\t{a:05x}\t{nom}\t{noms.get(a, "")}\t{n}\t{faites}\t{n - faites}\t{cotes}\n')

par_zone = {}
for z, a, nom, n, faites, un in lignes:
    t = par_zone.setdefault(z, [0, 0, 0, 0])
    t[0] += 1
    t[1] += n
    t[2] += n - faites
    t[3] += len(un)
print('| Zone | Sous-système | Fonctions exécutées | Instructions | Jamais exécutées | Sauts pris d\'un seul côté |')
print('|---|---|---:|---:|---:|---:|')
for z, (nf, n, jamais, un) in sorted(par_zone.items(), key=lambda kv: -kv[1][2]):
    print(f'| {z} | {desc_de.get(z, "hors atlas")} | {nf} | {n} | {jamais} ({100 * jamais / n:.0f} %) | {un} |')
print('\n| Fonction | Zone | Native | Instructions | Jamais exécutées | Sauts pris d\'un seul côté |')
print('|---|---|---|---:|---:|---:|')
for z, a, nom, n, faites, un in sorted(lignes, key=lambda l: -(l[3] - l[4]))[:50]:
    print(f'| `0x{a:05x}` | {z} | {noms.get(a, "")} | {n} | {n - faites} | {len(un)} |')
