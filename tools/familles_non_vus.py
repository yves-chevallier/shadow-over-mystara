#!/usr/bin/env python3
"""familles_non_vus.py COUVERTURE... : par type, instructions du listing que le fuzz n'a pas exécutées
(code d'origine). Les fichiers viennent de `DDSOM_COUVERTURE=fichier fuzz familles ...` ; FAMILLE=ESHL|SET|SOBJ
restreint l'affichage. Les plages qui sont des tables ou des fonctions de création appelées d'ailleurs
(non des états du gestionnaire) sont à écarter à la main."""
import re, sys
import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from familles_tables import tab, T
vus = set()
for f in sys.argv[1:]:
    vus |= {int(l, 16) for l in open(f).read().split()}
lst = {}
for ln in open(os.environ.get('DDSOM_SRC', '/home/ycr/som') + '/listing.txt'):
    m = re.match(r'^([0-9a-f]{6})  (.*)$', ln)
    if m:
        lst[int(m.group(1), 16)] = m.group(2).rstrip()
want = None
if len(sys.argv) > 0:
    import os
    want = os.environ.get('FAMILLE')
for fam, (base, n) in T.items():
    if want and fam != want:
        continue
    t = tab(base, n)
    ends = sorted(set(t))
    for k, a in enumerate(t):
        if a not in ends:
            continue
        nxt = [e for e in ends if e > a]
        end = nxt[0] if nxt else a + 0x200
        code = [x for x in sorted(lst) if a <= x < end and not lst[x].startswith('ori.b #0x0,D0b')]
        # ignore les tables de sauts: lignes dans les plages (move.w ... PC) suivies d'instructions bizarres
        miss = [x for x in code if x not in vus]
        if not any(x in vus for x in code):
            print(f'{fam} {k} {a:06x}: AUCUNE couverture ({len(code)} instr)')
            continue
        if miss:
            # regroupe en plages
            rngs = []
            for x in miss:
                if rngs and x - rngs[-1][1] <= 8:
                    rngs[-1][1] = x
                else:
                    rngs.append([x, x])
            print(f'{fam} {k} {a:06x}: {len(miss)}/{len(code)} instr non vues : ' + ' '.join(f'{r[0]:x}-{r[1]:x}' for r in rngs[:14]))
