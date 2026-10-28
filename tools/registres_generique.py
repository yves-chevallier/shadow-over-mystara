#!/usr/bin/env python3
"""Rend génériques sur le banc de registres (`Cx<'_, R>`) les fonctions des familles qui n'utilisent pas le processeur.

Toutes les fonctions libres `fn f(x: &mut Cx, ...)` deviennent `fn f<R: Banc>(x: &mut Cx<'_, R>, ...)` ; on compile,
et chaque fonction où le compilateur trouve une erreur reprend sa signature d'origine ; on recommence jusqu'à ce que
tout compile. Ce qui reste générique est prouvé sans registres par le compilateur.

    generique.py [fichiers...]   (défaut : tous les fichiers de families/)
"""
import os, re, subprocess, sys, json

SRC = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'port/ddsom-game/src')
ENV = dict(os.environ)  # CARGO_TARGET_DIR, DDSOM_ROMS : ceux de l'appelant
SIG = re.compile(r'^(\s*(?:pub(?:\([a-z]+\))?\s+)?fn\s+)(\w+)(<[^>()]*>)?\((\s*)(mut\s+)?x: &mut Cx(\s*[,)])')
GEN = re.compile(r'^(\s*(?:pub(?:\([a-z]+\))?\s+)?fn\s+)(\w+)<R: crate::families::cx::Banc(?:, ([^>]*))?>\((\s*)(mut\s+)?x: &mut Cx<\'_, R>(\s*[,)])')

def fichiers():
    if len(sys.argv) > 1:
        return [os.path.abspath(f) for f in sys.argv[1:]]
    out = []
    for root, _, fs in os.walk(SRC + '/families'):
        for f in fs:
            if f.endswith('.rs') and f != 'cx.rs':
                out.append(os.path.join(root, f))
    return sorted(out)

def generaliser(path):
    lignes = open(path).read().split('\n')
    n = 0
    for i, l in enumerate(lignes):
        m = SIG.match(l)
        if m:
            autres = m[3][1:-1] if m[3] else None
            gen = '<R: crate::families::cx::Banc' + (', ' + autres if autres else '') + '>'
            lignes[i] = l[:m.start(2)] + m[2] + gen + '(' + m[4] + (m[5] or '') + "x: &mut Cx<'_, R>" + m[6] + l[m.end():]
            n += 1
    if n:
        open(path, 'w').write('\n'.join(lignes))
    return n

def ajouter_import(s):
    # après la première ligne `use` du fichier (niveau module)
    m = re.search(r'^use [^\n]*\n', s, re.M)
    imp = 'use crate::families::cx::Banc;\n'
    if m:
        return s[:m.start()] + imp + s[m.start():]
    return imp + s

def revenir(path, ligne):
    """Remet la signature d'origine de la fonction qui contient la ligne (1-indexée). Vrai si changé."""
    lignes = open(path).read().split('\n')
    for i in range(ligne - 1, -1, -1):
        m = GEN.match(lignes[i])
        if m:
            autres = m[3]
            gen = '<' + autres + '>' if autres else ''
            lignes[i] = lignes[i][:m.start(2)] + m[2] + gen + '(' + m[4] + (m[5] or '') + 'x: &mut Cx' + m[6] + lignes[i][m.end():]
            open(path, 'w').write('\n'.join(lignes))
            return True
        if re.match(r'^\s*(pub(\([a-z]+\))?\s+)?fn\s', lignes[i]) or re.match(r'^(impl|mod|pub mod)\b', lignes[i]):
            return False
    return False

def compiler():
    p = subprocess.run(['cargo', 'build', '--release', '-p', 'ddsom-game', '--lib', '--message-format=json'],
                       cwd=os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'port'), env=ENV, capture_output=True, text=True)
    erreurs = []
    for l in p.stdout.splitlines():
        try:
            j = json.loads(l)
        except ValueError:
            continue
        if j.get('reason') != 'compiler-message' or j['message']['level'] != 'error':
            continue
        for sp in j['message']['spans']:
            if not sp.get('is_primary'):
                continue
            erreurs.append((os.path.join(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'port'), sp['file_name']), sp['line_start'], j['message']['message']))
    return p.returncode == 0, erreurs

import os as _os
PREMIERE = _os.environ.get('PREMIERE', '1') == '1'
if PREMIERE:
    total = sum(generaliser(f) for f in fichiers())
    print('généralisées :', total, flush=True)
for tour in range(60 if PREMIERE else 0):
    ok, erreurs = compiler()
    if ok:
        print('compile au tour', tour, flush=True)
        break
    changes = 0
    sans_fonction = set()
    for f, l, msg in erreurs:
        if revenir(f, l):
            changes += 1
        else:
            sans_fonction.add((f, l, msg[:100]))
    print(f'tour {tour} : {len(erreurs)} erreurs, {changes} fonctions rendues au processeur', flush=True)
    if changes == 0:
        for e in sorted(sans_fonction)[:30]:
            print('  ', e)
        break

# Deuxième passe : les fonctions génériques passées en pointeur `fn(&mut Cx)` (erreurs « mismatched types » au site
# d'appel) reprennent leur signature : on cherche, sur la ligne de l'erreur, les noms des fonctions génériques.
def generiques():
    noms = {}
    for f in fichiers():
        for i, l in enumerate(open(f).read().split('\n'), 1):
            m = GEN.match(l)
            if m:
                noms.setdefault(m[2], []).append((f, i))
    return noms

for tour in range(20):
    ok, erreurs = compiler()
    if ok:
        print('compile (2e passe) au tour', tour, flush=True)
        break
    noms = generiques()
    changes = 0
    for f, l, msg in erreurs:
        if revenir(f, l):
            changes += 1
            continue
        texte = open(f).read().split('\n')[l - 1]
        for ident in re.findall(r'(?:\w+::)*(\w+)', texte):
            for (g, i) in noms.get(ident, []):
                if revenir(g, i):
                    changes += 1
    print(f'2e passe, tour {tour} : {len(erreurs)} erreurs, {changes} fonctions rendues', flush=True)
    if changes == 0:
        for e in erreurs[:20]:
            print('  ', e)
        break
