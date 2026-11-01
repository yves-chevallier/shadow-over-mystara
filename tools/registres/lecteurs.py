#!/usr/bin/env python3
"""Lecteurs de registres d'entrée : fonctions Rust (familles) qui lisent un registre du processeur (x.c.d[k],
x.c.a[k], c.d[k]...) avant de l'écrire. Approximation textuelle (ordre du texte, pas du flot).

lecteurs.py <racine src> [--famille motif] [--resume]
"""
import re, sys, pathlib, collections

racine = pathlib.Path(sys.argv[1])
motif = None
if '--famille' in sys.argv:
    motif = sys.argv[sys.argv.index('--famille') + 1]
resume = '--resume' in sys.argv

FN = re.compile(r'^\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+(\w+)\s*(<[^>]*>)?\s*\(([^)]*)\)', re.M)
REG = re.compile(r'\b(?:x\.c|c|self\.c)\.(d|a)\[(\d)\]')

def corps(src, debut):
    i = src.index('{', debut)
    n = 0
    for j in range(i, len(src)):
        if src[j] == '{':
            n += 1
        elif src[j] == '}':
            n -= 1
            if n == 0:
                return src[i:j + 1]
    return src[i:]

def nature(texte, fin):
    """écriture pure, lecture-écriture, ou lecture, selon ce qui suit l'accès."""
    suite = texte[fin:fin + 6]
    if re.match(r'\s*=[^=]', suite):
        return 'w'
    if re.match(r'\s*(\|=|&=|\+=|-=|\^=|<<=|>>=)', suite):
        return 'rw'
    return 'r'

resultats = []
for p in sorted(racine.rglob('*.rs')):
    rel = str(p.relative_to(racine))
    if motif and motif not in rel:
        continue
    src = p.read_text()
    # retirer les commentaires de ligne
    src_nc = re.sub(r'//[^\n]*', lambda m: ' ' * len(m.group(0)), src)
    for m in FN.finditer(src_nc):
        nom, gen, params = m.group(1), m.group(2) or '', m.group(3)
        if 'Cpu' not in params and 'Cx' not in params:
            continue
        if 'Cx' in params and '<' in params.split('Cx', 1)[1][:3]:
            continue  # Cx<'_, R> générique : pas de registres
        try:
            b = corps(src_nc, m.end())
        except ValueError:
            continue
        ecrits, lus = set(), []
        for r in REG.finditer(b):
            reg = f'{r.group(1).upper()}{r.group(2)}'
            # préfixe : `let` ou membre gauche partiel `x.c.d[0] = x.c.d[0] & ...` : compté à droite
            k = nature(b, r.end())
            if reg in ecrits:
                continue
            if k == 'w':
                # x.c.d[0] = x.c.d[0] & ... : la droite lit d'abord
                ligne_fin = b.find(';', r.end())
                droite = b[r.end():ligne_fin]
                if re.search(r'\b(?:x\.c|c|self\.c)\.' + r.group(1) + r'\[' + r.group(2) + r'\]', droite):
                    lus.append(reg)
                ecrits.add(reg)
            else:
                lus.append(reg)
                ecrits.add(reg)
        ligne = src.count('\n', 0, m.start()) + 1
        lus = [r for r in dict.fromkeys(lus) if r not in ('A0', 'A5', 'A6', 'A7')]
        if lus:
            resultats.append((rel, ligne, nom, lus))

if resume:
    par = collections.Counter(r[0].split('/')[1] if r[0].startswith('families/') and '/' in r[0][9:] else r[0] for r in resultats)
    for k, v in sorted(par.items(), key=lambda kv: -kv[1]):
        print(f'{v:4} {k}')
    print(f'{len(resultats)} fonctions')
else:
    for rel, ligne, nom, lus in resultats:
        print(f'{rel}:{ligne} {nom} {",".join(lus)}')
