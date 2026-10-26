#!/usr/bin/env python3
"""Zones de la ROM programme lues par les routines natives et que les assets ne couvrent pas : écrit
port/extract/zones_natives.txt (docs/sans-rom.md).

  tools/zones_natives.py <couvert.bin> <lectures.txt>... > port/extract/zones_natives.txt

<couvert.bin> : `couverture_rom <assets> --octets <couvert.bin> --sans data/natives.json` (couverture mot par mot
des autres fichiers d'assets) ; <lectures.txt> : sorties de `lectures_rom` en mode sans 68000 (DDSOM_SANS_68K=1,
DDSOM_COUVERT=<couvert.bin>). Les zones lues non couvertes séparées de moins de ECART octets sont réunies ; chacune
est attribuée au fichier Rust qui en lit le plus de mots (premier appelant hors des accès génériques mem.rs, vue.rs).
"""
import collections
import sys

ECART = 0x10
GENERIQUES = ('mem.rs', 'vue.rs')
# lues avant toute mesure (Cpu::reset au chargement, espace programme) : pile et PC initiaux
FIXES = [('p', 0x0, 0x8, 'vecteurs')]


def lecteur(sites):
    for s in sites.split(';'):
        if s and not s.startswith(GENERIQUES):
            return s.rsplit(':', 1)[0]
    return 'inconnu'


def main():
    couvert = open(sys.argv[1], 'rb').read()
    lus = {'d': set(), 'p': set()}
    qui = {}
    for f in sys.argv[2:]:
        for l in open(f):
            t = l.split()
            if len(t) == 5 and t[0] in ('d', 'p'):
                lus[t[0]].update(range(int(t[1], 16), int(t[2], 16), 2))
            elif t and t[0] == 's':
                qui.setdefault((t[1], int(t[2], 16)), lecteur(t[3] if len(t) > 3 else ''))
    print('# espace début fin(exclue) lecteur — généré par tools/zones_natives.py, ne pas éditer')
    zones = []
    for esp, a, b, nom in FIXES:
        lus[esp].update(range(a, b, 2))
        for m in range(a, b, 2):
            qui.setdefault((esp, m), nom)
    for esp, base in (('d', 0), ('p', 0x200000)):
        mots = sorted(a for a in lus[esp] if not couvert[base + a // 2])
        for a in mots:
            if zones and zones[-1][0] == esp and a - zones[-1][2] <= ECART:
                zones[-1][2] = a + 2
                zones[-1][3][qui.get((esp, a), 'inconnu')] += 1
            else:
                zones.append([esp, a, a + 2, collections.Counter([qui.get((esp, a), 'inconnu')])])
    for esp, a, b, c in zones:
        print(f'{esp} {a:06x} {b:06x} {c.most_common(1)[0][0]}')


main()
