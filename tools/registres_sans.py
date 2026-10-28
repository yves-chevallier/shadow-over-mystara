#!/usr/bin/env python3
"""Passe en `#[sans]` (Corps::Gestionnaire, sans registres) chaque entrée `handlers!` / `entry!` des familles dont le
gestionnaire est générique sur le banc de registres.

Vérification par le compilateur : à la fin de chaque fichier, une fonction de contrôle par entrée appelle le gestionnaire
avec un `Gx` ; celles qui ne compilent pas désignent les gestionnaires encore sur le processeur. Les contrôles sont
ensuite retirés, et les autres entrées marquées `#[sans]`.

    sans.py <adresses exclues, séparées par des virgules>   (gestionnaires dont la sortie est relue : `Restes`)
"""
import os, re, subprocess, sys, json

SRC = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'port/ddsom-game/src/families')
PORT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'port')
ENV = dict(os.environ)  # CARGO_TARGET_DIR, DDSOM_ROMS : ceux de l'appelant
exclues = {int(a, 16) for a in sys.argv[1].split(',') if a} if len(sys.argv) > 1 else set()

ENTREE = re.compile(r'^(\s*)(0x[0-9a-f]+) ("[^"]*") ([\w:]+),')                 # handlers! { 0x.. "nom" chemin, }
ENTREE_SET = re.compile(r'^(\s*entry!\()(0x[0-9a-f]+) ("[^"]*"), ([\w:]+)\),')  # set.rs
DEBUT = '// --- contrôle sans registres (sans.py) ---'

def fichiers():
    out = []
    for root, _, fs in os.walk(SRC):
        for f in fs:
            if f.endswith('.rs') and f != 'cx.rs':
                out.append(os.path.join(root, f))
    return sorted(out)

def dans_handlers(lignes, i):
    for k in range(i, -1, -1):
        if 'handlers! {' in lignes[k]:
            return True
        if lignes[k].strip() == '}' and k != i:
            return False
    return False

def entrees(lignes):
    for i, l in enumerate(lignes):
        m = ENTREE.match(l)
        if m and dans_handlers(lignes, i):
            yield i, int(m[2], 16), m[4], 'h'
        m = ENTREE_SET.match(l)
        if m:
            yield i, int(m[2], 16), m[4], 's'

# 1. contrôles
controles = {}  # (fichier, ligne du contrôle) -> (fichier, ligne de l'entrée)
for f in fichiers():
    lignes = open(f).read().split('\n')
    es = [e for e in entrees(lignes) if e[1] not in exclues]
    if not es:
        continue
    bloc = ['', DEBUT]
    base = len(lignes) + 2
    for k, (i, adr, chemin, _) in enumerate(es):
        bloc.append(f'#[allow(dead_code)] fn __controle_sans_{k}(x: &mut crate::families::cx::Gx) {{ {chemin}(x); }}')
        controles[(f, base + 1 + k)] = (f, i)
    open(f, 'w').write('\n'.join(lignes + bloc))

p = subprocess.run(['cargo', 'build', '--release', '-p', 'ddsom-game', '--lib', '--message-format=json'], cwd=PORT, env=ENV,
                   capture_output=True, text=True)
echecs = set()
for l in p.stdout.splitlines():
    try:
        j = json.loads(l)
    except ValueError:
        continue
    if j.get('reason') != 'compiler-message' or j['message']['level'] != 'error':
        continue
    for sp in j['message']['spans']:
        cle = (os.path.join(PORT, sp['file_name']), sp['line_start'])
        if cle in controles:
            echecs.add(controles[cle])

# 2. retrait des contrôles, marquage
n = 0
for f in fichiers():
    s = open(f).read()
    if DEBUT not in s:
        continue
    s = s[:s.index('\n' + DEBUT) - 1] if s[s.index('\n' + DEBUT) - 1] == '\n' else s[:s.index('\n' + DEBUT)]
    lignes = s.split('\n')
    for i, adr, chemin, sorte in list(entrees(lignes)):
        if adr in exclues or (f, i) in echecs or '#[sans]' in lignes[i]:
            continue
        if sorte == 'h':
            m = ENTREE.match(lignes[i])
            lignes[i] = f'{m[1]}{m[2]} {m[3]} #[sans] {m[4]},' + lignes[i][m.end():]
        else:
            m = ENTREE_SET.match(lignes[i])
            lignes[i] = f'{m[1]}{m[2]} {m[3]}, #[sans] {m[4]}),' + lignes[i][m.end():]
        n += 1
    open(f, 'w').write('\n'.join(lignes))
print('contrôles :', len(controles), '; sur le processeur :', len(echecs), '; passées sans registres :', n)
