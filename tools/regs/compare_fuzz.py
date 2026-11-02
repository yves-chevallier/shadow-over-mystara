# compare_fuzz.py <dir>: compares the fuzz_couv outputs of fuzz.sh, branch against main.
import re, glob, collections, sys
d = sys.argv[1]
# Matches the summary lines printed by the Rust fuzz_couv example (French output today).
L = re.compile(r'^([0-9a-f]{6}) (\S+)\s+(\d+) appels\s+(\d+) essais \(\s*(\d+) neufs\)\s+(\d+) écarts \(\s*(\d+) neufs,\s*(\d+) plausibles\)\s+(\d+) paniques')
tot = {v: collections.defaultdict(lambda: [0]*5) for v in ('branch', 'main')}
sig = {v: collections.defaultdict(set) for v in ('branch', 'main')}
for f in glob.glob(d + '/*.txt'):
    v, z, *s = f.split('/')[-1][:-4].split('_')
    cur = None
    for l in open(f):
        m = L.match(l)
        if m:
            cur = (z, m.group(1), m.group(2))
            t = tot[v][cur]
            for i, g in enumerate((4, 6, 8, 9)):
                t[i] += int(m.group(g))
            t[4] += int(m.group(3))
        elif cur and l.startswith('    ['):
            sig[v][cur].add(l.strip())
keys = sorted(set(tot['branch']) | set(tot['main']))
print('native zone: trials deviations plausible panics (branch | main)')
for k in keys:
    b, m = tot['branch'].get(k, [0]*5), tot['main'].get(k, [0]*5)
    if b[1:4] != m[1:4]:
        print(f'{k[0]} {k[1]} {k[2]:28} {b[0]:6} {b[1]:5} {b[2]:4} {b[3]:4} | {m[0]:6} {m[1]:5} {m[2]:4} {m[3]:4}')
        for s in sorted(sig['branch'].get(k, set()) - sig['main'].get(k, set()))[:6]:
            print('     + ' + s[:220])
        for s in sorted(sig['main'].get(k, set()) - sig['branch'].get(k, set()))[:3]:
            print('     - ' + s[:220])
