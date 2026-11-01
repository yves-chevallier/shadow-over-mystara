#!/usr/bin/env python3
"""Rend génériques (Cx<'_, R: Banc>) des fonctions de familles, puis propage aux fonctions appelées tant que
`cargo check` le demande (erreur « expected &mut Cx<'_>, found &mut Cx<'_, R> » sur l'argument d'un appel).
S'arrête sur les autres erreurs (accès aux registres...), qu'il affiche.

generique.py <port/> fichier.rs:fonction [...]   (chemins relatifs à ddsom-game/src)
"""
import re, subprocess, sys, pathlib, json

port = pathlib.Path(sys.argv[1])
src = port / 'ddsom-game/src'

def generiser(fichier, nom):
    p = src / fichier
    s = p.read_text()
    pat = re.compile(r'\bfn ' + re.escape(nom) + r'\(x: &mut Cx([,)])')
    s2, n = pat.subn(lambda m: f"fn {nom}<R: Banc>(x: &mut Cx<'_, R>{m.group(1)}", s)
    if n == 0:
        # autre nom de paramètre que x
        pat = re.compile(r'\bfn ' + re.escape(nom) + r'\((\w+): &mut Cx([,)])')
        s2, n = pat.subn(lambda m: f"fn {nom}<R: Banc>({m.group(1)}: &mut Cx<'_, R>{m.group(2)}", s)
    if n and 'Banc' not in re.sub(r'fn \w+<R: Banc>', '', s2).split('fn ', 1)[0] and not re.search(r'use [^;]*\bBanc\b', s2):
        # importer Banc
        s2 = re.sub(r'(\nuse [^\n]*\n)', r'\1use crate::families::cx::Banc;\n', s2, count=1)
    p.write_text(s2)
    return n

EXCLUS = ('fn set_dw(',)

def generiser_ligne(fichier, ligne):
    p = src / fichier
    lignes = p.read_text().split('\n')
    l = lignes[ligne - 1]
    if any(e in l for e in EXCLUS):
        return 0
    l2, n = re.subn(r"\bfn (\w+)\((\w+): &mut Cx([,)])", lambda m: f"fn {m.group(1)}<R: Banc>({m.group(2)}: &mut Cx<'_, R>{m.group(3)}", l)
    if n == 0:
        return 0
    if re.search(r"(FnOnce|FnMut|Fn|fn)\(&mut Cx[,)]", l2):
        # paramètres fonctions : même durée de vie que le contexte
        l2 = re.sub(r"(FnOnce|FnMut|Fn|fn)\(&mut Cx([,)])", lambda m: f"{m.group(1)}(&mut Cx<'a, R>{m.group(2)}", l2)
        l2 = l2.replace("<R: Banc>(", "<'a, R: Banc>(", 1).replace("&mut Cx<'_, R>", "&mut Cx<'a, R>", 1)
    lignes[ligne - 1] = l2
    t = '\n'.join(lignes)
    if not re.search(r'use [^;]*\bBanc\b', t):
        t = re.sub(r'(\nuse [^\n]*\n)', r'\1use crate::families::cx::Banc;\n', t, count=1)
    p.write_text(t)
    return n

def check():
    r = subprocess.run(['cargo', 'check', '--release', '-p', 'ddsom-game', '--lib', '--message-format=json'],
                       cwd=port, capture_output=True, text=True)
    errs = []
    for l in r.stdout.splitlines():
        try:
            m = json.loads(l)
        except Exception:
            continue
        if m.get('reason') == 'compiler-message' and m['message']['level'] == 'error':
            errs.append(m['message'])
    return errs

# définition d'une fonction par son chemin d'appel : on cherche `fn nom(` dans les fichiers
def trouver(nom, chemin_appel, fichier_appelant):
    cands = []
    for p in src.rglob('*.rs'):
        t = p.read_text()
        if re.search(r'\bfn ' + re.escape(nom) + r'\(\w+: &mut Cx[,)]', t):
            cands.append(str(p.relative_to(src)))
    if len(cands) == 1:
        return cands[0]
    # préférer celui que suggère le chemin (super::enemy_ogre::hit::regroup)
    parts = [q for q in chemin_appel.split('::')[:-1] if q not in ('super', 'crate', 'self', 'families')]
    meilleurs = [c for c in cands if all(q in c for q in parts)] if parts else []
    if len(meilleurs) == 1:
        return meilleurs[0]
    if not parts:
        # même module que l'appelant
        d = str(pathlib.Path(fichier_appelant).parent)
        meme = [c for c in cands if c == fichier_appelant] or [c for c in cands if str(pathlib.Path(c).parent) == d]
        if len(meme) == 1:
            return meme[0]
    return None

for arg in sys.argv[2:]:
    f, n = arg.split(':')
    print(f'{f}:{n} -> {generiser(f, n)}')

for tour in range(200):
    errs = check()
    if not errs:
        print('compile')
        break
    progres = False
    autres = []
    for e in errs:
        msg = e['rendered']
        sp = next((s for s in e['spans'] if s['is_primary']), None)
        if "expected `&mut Cx<'_>`, found `&mut Cx<'_, R>`" in msg or "expected `&mut cx::Cx<'_>`" in msg or ("expected mutable reference `&mut cx::Cx<'_, &mut m68k::Cpu>`" in msg and "found mutable reference `&mut cx::Cx<'_, R>`" in msg):
            # l'appel fautif : le texte de la span secondaire « arguments to this function »
            defs = [(sp2['file_name'], sp2['line_start']) for ch in e.get('children', []) if 'defined here' in ch['message'] for sp2 in ch['spans']]
            ok = False
            for (fn_, ln) in defs:
                if generiser_ligne(fn_.replace('ddsom-game/src/', ''), ln):
                    print(f"  {fn_}:{ln}")
                    ok = True
            if ok:
                progres = True
            else:
                autres.append(msg)
        else:
            autres.append(msg)
    if not progres:
        print(f'{len(autres)} erreurs restantes :')
        for a in autres[:40]:
            print(a)
        break
