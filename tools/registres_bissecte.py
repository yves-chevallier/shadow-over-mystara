#!/usr/bin/env python3
"""bissecte.py <scénario> <images> <poison de base> [étiquette=sortie]
Trouve, pas à pas, les (gestionnaire, registre) dont la sortie est relue, et les exclut.
Imprime la liste finale des exclusions."""
import os, re, subprocess, sys
from concurrent.futures import ThreadPoolExecutor
BIN=os.environ.get('BIN', os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'port/target/release/examples/validate'))
ROM=os.environ.get('DDSOM_ROMS', 'rom'); EEP=os.environ.get('DDSOM_EEPROM', 'testdata/eeprom_ddsom.bin')
sc, n, base = sys.argv[1], int(sys.argv[2]), sys.argv[3]
lab = sys.argv[4] if len(sys.argv) > 4 else 'sortie'
JOBS = int(os.environ.get('JOBS', '8'))
def run(poison, frames, trace=False):
    env = dict(os.environ, DDSOM_POISON=poison)
    if trace: env['DDSOM_POISON_TRACE'] = '1'
    p = subprocess.run([BIN, ROM, EEP, sc, str(frames)], env=env, capture_output=True, text=True)
    last = p.stdout.strip().splitlines()[-1] if p.stdout.strip() else p.stderr.strip().splitlines()[-1]
    m = re.search(r"image (\d+)", last)
    ok = last.startswith('VALIDE')
    return ok, (int(m.group(1)) if m and not ok else None), p.stderr, last
excl = []
while True:
    poison = ','.join([base] + excl)
    ok, img, _, last = run(poison, n)
    print('#', poison, '=>', last, flush=True)
    if ok: break
    if img is None:
        print('panique ?', last); break
    _, _, err, _ = run(poison, img + 1, trace=True)
    sites = {}
    for l in err.splitlines():
        m = re.match(r'POISON (\S+) (.+) masque=([0-9a-f]+)', l)
        if m and m.group(1).startswith(lab) and int(m.group(3), 16):
            sites[m.group(2)] = sites.get(m.group(2), 0) | int(m.group(3), 16)
    # le gestionnaire (dernier mot du site), sans exclure ce qui l'est déjà
    hs = sorted({s.split()[-1] for s in sites})
    print('  candidats', len(hs), flush=True)
    def t(h):
        return h, run(f'{lab}@{h}', img + 1)[0]
    with ThreadPoolExecutor(JOBS) as ex:
        bad = [h for h, ok in ex.map(t, hs) if not ok]
    if not bad:
        print('  aucun candidat seul ne fait échouer : combinaison ?'); break
    h = bad[0]
    mask = 0
    for s, m in sites.items():
        if s.split()[-1] == h: mask |= m
    regs = [f'd{i}' for i in range(8) if mask >> i & 1] + [f'a{i}' for i in range(7) if mask >> (8 + i) & 1] + (['ccr'] if mask >> 16 & 1 else [])
    def tr(r):
        return r, run(f'{lab}.{r}@{h}', img + 1)[0]
    with ThreadPoolExecutor(JOBS) as ex:
        badr = [r for r, ok in ex.map(tr, regs) if not ok]
    print('  lecteur :', h, badr or regs, flush=True)
    for r in (badr or regs):
        excl.append(f'!{lab}.{r}@{h}')
print('EXCLUSIONS', ','.join(excl))
