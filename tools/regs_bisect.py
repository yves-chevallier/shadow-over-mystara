#!/usr/bin/env python3
"""regs_bisect.py <scenario> <frames> <base poison> [label=exit]
Finds, step by step, the (handler, register) pairs whose output is read back, and excludes them.
Prints the final list of exclusions."""
import os, re, subprocess, sys
from concurrent.futures import ThreadPoolExecutor
BIN=os.environ.get('BIN', os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'port/target/release/examples/validate'))
ROM=os.environ.get('DDSOM_ROMS', 'rom'); EEP=os.environ.get('DDSOM_EEPROM', 'testdata/eeprom_ddsom.bin')
sc, n, base = sys.argv[1], int(sys.argv[2]), sys.argv[3]
lab = sys.argv[4] if len(sys.argv) > 4 else 'exit'
JOBS = int(os.environ.get('JOBS', '8'))
def run(poison, frames, trace=False):
    env = dict(os.environ, DDSOM_POISON=poison)
    if trace: env['DDSOM_POISON_TRACE'] = '1'
    p = subprocess.run([BIN, ROM, EEP, sc, str(frames)], env=env, capture_output=True, text=True)
    last = p.stdout.strip().splitlines()[-1] if p.stdout.strip() else p.stderr.strip().splitlines()[-1]
    m = re.search(r"(?:image|frame) (\d+)", last)
    ok = last.startswith('VALID')
    return ok, (int(m.group(1)) if m and not ok else None), p.stderr, last
excl = []
while True:
    poison = ','.join([base] + excl)
    ok, img, _, last = run(poison, n)
    print('#', poison, '=>', last, flush=True)
    if ok: break
    if img is None:
        print('panic?', last); break
    _, _, err, _ = run(poison, img + 1, trace=True)
    sites = {}
    for l in err.splitlines():
        m = re.match(r'POISON (\S+) (.+) (?:masque|mask)=([0-9a-f]+)', l)
        if m and m.group(1).startswith(lab) and int(m.group(3), 16):
            sites[m.group(2)] = sites.get(m.group(2), 0) | int(m.group(3), 16)
    # the handler (last word of the site), without excluding what already is
    hs = sorted({s.split()[-1] for s in sites})
    print('  candidates', len(hs), flush=True)
    def t(h):
        return h, run(f'{lab}@{h}', img + 1)[0]
    with ThreadPoolExecutor(JOBS) as ex:
        bad = [h for h, ok in ex.map(t, hs) if not ok]
    if not bad:
        print('  no single candidate makes it fail: combination?'); break
    h = bad[0]
    mask = 0
    for s, m in sites.items():
        if s.split()[-1] == h: mask |= m
    regs = [f'd{i}' for i in range(8) if mask >> i & 1] + [f'a{i}' for i in range(7) if mask >> (8 + i) & 1] + (['ccr'] if mask >> 16 & 1 else [])
    def tr(r):
        return r, run(f'{lab}.{r}@{h}', img + 1)[0]
    with ThreadPoolExecutor(JOBS) as ex:
        badr = [r for r, ok in ex.map(tr, regs) if not ok]
    print('  reader:', h, badr or regs, flush=True)
    for r in (badr or regs):
        excl.append(f'!{lab}.{r}@{h}')
print('EXCLUSIONS', ','.join(excl))
