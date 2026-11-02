#!/usr/bin/env python3
"""families_unseen.py COVERAGE...: per type, listing instructions the fuzz did not execute
(original code). The files come from `DDSOM_COUVERTURE=file fuzz familles ...`; FAMILLE=ESHL|SET|SOBJ
restricts the display. Ranges that are tables or creation functions called from elsewhere
(not handler states) must be discarded by hand."""
import re, sys
import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from families_tables import table, T
seen = set()
for f in sys.argv[1:]:
    seen |= {int(l, 16) for l in open(f).read().split()}
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
    t = table(base, n)
    ends = sorted(set(t))
    for k, a in enumerate(t):
        if a not in ends:
            continue
        nxt = [e for e in ends if e > a]
        end = nxt[0] if nxt else a + 0x200
        code = [x for x in sorted(lst) if a <= x < end and not lst[x].startswith('ori.b #0x0,D0b')]
        # ignores jump tables: lines in the ranges (move.w ... PC) followed by odd instructions
        miss = [x for x in code if x not in seen]
        if not any(x in seen for x in code):
            print(f'{fam} {k} {a:06x}: NO coverage ({len(code)} instr)')
            continue
        if miss:
            # group into ranges
            rngs = []
            for x in miss:
                if rngs and x - rngs[-1][1] <= 8:
                    rngs[-1][1] = x
                else:
                    rngs.append([x, x])
            print(f'{fam} {k} {a:06x}: {len(miss)}/{len(code)} instr not seen: ' + ' '.join(f'{r[0]:x}-{r[1]:x}' for r in rngs[:14]))
