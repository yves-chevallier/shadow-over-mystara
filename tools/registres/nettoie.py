#!/usr/bin/env python3
"""Après generique.py : traite les accès aux registres (« no field d/a on type R ») dans les fichiers dont le chemin
contient <motif> : restes D2-D5 -> banc (*x.c.dN()), D7 lu -> x.compteur(), écriture seule de D0/D1/D6/An -> retirée
(registre mort : la fonction devient générique, et entre gestionnaires seuls D2-D5 sont relus, DDSOM_POISON=sortie).
Affiche le reste.   nettoie.py <port/> <motif>
"""
import re, subprocess, sys, json, pathlib
port = pathlib.Path(sys.argv[1]); motif = sys.argv[2]
r = subprocess.run(['cargo', 'check', '--release', '-p', 'ddsom-game', '--lib', '--message-format=json'], cwd=port, capture_output=True, text=True)
sites = {}
autres = []
for l in r.stdout.splitlines():
    try: m = json.loads(l)
    except Exception: continue
    if m.get('reason') != 'compiler-message' or m['message']['level'] != 'error': continue
    e = m['message']; sp = next(s for s in e['spans'] if s['is_primary'])
    f = sp['file_name']
    if motif in f and ('no field' in e['message'] or ('mismatched types' in e['message'] and 'set_dw(' in (sp['text'][0]['text'] if sp['text'] else ''))):
        sites.setdefault(f, set()).add(sp['line_start'])
    else:
        autres.append(e['rendered'])
REG = r'x\.c\.(d|a)\[(\d)\]'
for f, lignes in sites.items():
    p = port / f
    L = p.read_text().split('\n')
    for n in sorted(lignes, reverse=True):
        l = L[n - 1]
        if l is None:
            continue
        ms = re.match(r'^(\s*)set_dw\(x, (\d), (.*)\);(\s*//.*)?$', l)
        if ms:
            k = int(ms.group(2))
            if k in (2, 3, 4, 5):
                L[n - 1] = f'{ms.group(1)}*x.c.d{k}() = *x.c.d{k}() & 0xffff_0000 | ({ms.group(3)}) as u32;'
                print(f'{f}:{n} reste : {L[n-1].strip()}')
            else:
                print(f'{f}:{n} retiré : {l.strip()}')
                L[n - 1] = None
            continue
        if re.search(r'x\.c\.d\[7\] as u(8|16)', l) and not re.search(r'x\.c\.d\[7\]\s*=', l):
            L[n - 1] = re.sub(r'x\.c\.d\[7\] as u(8|16)', r'x.compteur() as u\1', l)
            print(f'{f}:{n} compteur : {L[n-1].strip()}')
            continue
        # restes D2-D5 : par le banc
        l2 = re.sub(r'x\.c\.d\[([2-5])\]', r'(*x.c.d\1())', l)
        l2 = re.sub(r'\(\*x\.c\.d([2-5])\(\)\)(\s*=[^=])', r'*x.c.d\1()\2', l2)
        l2 = re.sub(r'\(\*x\.c\.d([2-5])\(\)\)(\s*(\|=|&=|\+=|-=))', r'*x.c.d\1()\2', l2)
        if l2 != l:
            L[n - 1] = l2
            print(f'{f}:{n} reste : {l2.strip()}')
            continue
        mm = re.match(r'^\s*x\.c\.(d\[[016]\]|a\[[1-4]\])\s*(=[^=]|\|=|&=|\+=|-=|\^=).*;\s*(//.*)?$', l)
        if mm:
            print(f'{f}:{n} retiré : {l.strip()}')
            L[n - 1] = None
            continue
        if 'x.c.d[7]' in l:
            print(f'{f}:{n} À VOIR (D7) : {l.strip()}')
            continue
        print(f'{f}:{n} À VOIR : {l.strip()}')
    p.write_text('\n'.join(x for x in L if x is not None))
print(f'{len(autres)} autres erreurs')
for a in autres[:10]: print(a)
