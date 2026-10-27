#!/usr/bin/env python3
"""Bots longs (scenarios/long/, validés avec DDSOM_LONG=1 tools/validate_all.sh) : parties de 40 000 à 90 000 images
jouées par `examples/autobot.rs` (plusieurs joueurs, magies, routes, boutiques, difficultés, joueurs mortels).

    tools/gen_long.py [nom...]      (tous par défaut ; il faut port/target/release/examples/autobot)

Écrit scenarios/long/<nom>.txt (le scénario d'entrées produit par le bot) et <nom>.images (son nombre d'images).
Variables : DDSOM_ROMS (rom), DDSOM_EEPROM (testdata/eeprom_ddsom.bin). Voir docs/couverture.md (« Bots longs »).
"""
import os
import subprocess
import sys

ICI = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, ICI)
import bot_lib  # noqa: E402

RACINE = os.path.dirname(ICI)
AUTOBOT = os.path.join(RACINE, 'port/target/release/examples/autobot')
SORTIE = os.path.join(RACINE, 'scenarios/long')

# personnages : 0 combattant, 1 clerc, 2 elfe, 3 nain, 4 magicien, 5 voleur ; pokes « image:adresse:valeur:taille »
# (ff802a : difficulté, ff8072 : langue) ; plan « scène:durée,... » (scènes en décimal)
SPECS = {
    # deux joueurs (clerc, magicien), magies et objets fréquents
    'long_2p_magies': dict(players=2, chars=(1, 4), frames=90000, seed=501, god='0x70', cast=30, charges=2),
    # nain seul, 28 scènes (1, 5, 9... 109), difficulté 7, langue 4
    'long_28_scenes': dict(players=1, chars=(3,), frames=90000, seed=502, god='0x70',
                           plan=','.join(f'{s}:2800' for s in range(1, 112, 4)),
                           pokes=['800:ff802a:7:1', '800:ff8072:4:2']),
    # trois joueurs mortels (combattant, elfe, nain)
    'long_3p_mortels': dict(players=3, chars=(0, 2, 3), frames=60000, seed=503),
    # quatre joueurs (combattant, elfe, magicien, voleur)
    'long_4p': dict(players=4, chars=(0, 2, 4, 5), frames=60000, seed=504, god='0x70', cast=20, charges=2),
    # elfe seul depuis le choix de route 0x14
    'long_route_14': dict(players=1, chars=(2,), frames=60000, seed=505, god='0x70', plan='20:40000', cast=10, charges=1),
    # voleur et nain depuis le choix de route 0x38
    'long_route_38': dict(players=2, chars=(5, 3), frames=60000, seed=506, god='0x70', plan='56:40000', cast=10,
                          charges=1),
    # clerc, les boutiques 69 à 82 (0x45-0x52), difficulté 0
    'long_boutiques': dict(players=1, chars=(1,), frames=40000, seed=507, god='0x70',
                           plan=','.join(f'{s}:1500' for s in range(69, 83)), pokes=['800:ff802a:0:1']),
    # combattant et magicien mortels, difficulté 3, scènes 29 à 56
    'long_2p_d3_mortels': dict(players=2, chars=(0, 4), frames=60000, seed=508,
                               plan=','.join(f'{s}:2500' for s in range(29, 57, 3)), cast=15, charges=1,
                               pokes=['800:ff802a:3:1']),
    # nain seul mortel, jeu naturel
    'long_nain_mortel': dict(players=1, chars=(3,), frames=60000, seed=509, cast=10, charges=1),
    # voleur seul, objets et magies très fréquents, scènes 1 à 56
    'long_voleur_objets': dict(players=1, chars=(5,), frames=60000, seed=510, god='0x70', cast=40, charges=3,
                               plan=','.join(f'{s}:3000' for s in range(1, 57, 5))),
    # magicien seul, magies très fréquentes, scènes 57 à 86
    'long_magicien_57_86': dict(players=1, chars=(4,), frames=60000, seed=511, god='0x70', cast=40, charges=3,
                                plan=','.join(f'{s}:2500' for s in range(57, 87, 3))),
    # clerc et elfe mortels, difficulté 7
    'long_2p_d7_mortels': dict(players=2, chars=(1, 2), frames=60000, seed=512, cast=10, charges=1,
                               pokes=['800:ff802a:7:1']),
    # quatre joueurs depuis le choix de route 0x38
    'long_4p_route_38': dict(players=4, chars=(1, 3, 4, 5), frames=60000, seed=513, god='0x70', plan='56:40000',
                             cast=15, charges=1),
    # combattant et clerc depuis le choix de route 0x14
    'long_route_14_2p': dict(players=2, chars=(0, 1), frames=60000, seed=514, god='0x70', plan='20:40000'),
}


def gen(nom):
    sp = SPECS[nom]
    roms = os.environ.get('DDSOM_ROMS', os.path.join(RACINE, 'rom'))
    eeprom = os.environ.get('DDSOM_EEPROM', os.path.join(RACINE, 'testdata/eeprom_ddsom.bin'))
    lignes = bot_lib.build(frames=sp['frames'], players=sp['players'], chars=sp['chars'], coin_period=1500,
                           base_only=True, pokes=sp.get('pokes', []))
    os.makedirs(SORTIE, exist_ok=True)
    base = os.path.join(SORTIE, nom + '.base')
    with open(base, 'w') as f:
        f.write('\n'.join(lignes) + '\n')
    out = os.path.join(SORTIE, nom + '.txt')
    cmd = [AUTOBOT, roms, eeprom, base, str(sp['frames']), out, '--players', str(sp['players']),
           '--seed', str(sp['seed']), '--weaken', '450', '--patience', '400']
    if sp.get('god'):
        cmd += ['--god', sp['god']]
    if sp.get('plan'):
        cmd += ['--plan', sp['plan']]
    if sp.get('cast'):
        cmd += ['--cast', str(sp['cast']), '--charges', str(sp['charges'])]
    r = subprocess.run(cmd, capture_output=True, text=True)
    os.remove(base)
    with open(os.path.join(SORTIE, nom + '.images'), 'w') as f:
        f.write(f"{sp['frames']}\n")
    visites = [l for l in (r.stdout + r.stderr).splitlines() if l.startswith('scènes visitées')]
    print(nom, r.returncode, *visites)


for nom in sys.argv[1:] or SPECS:
    gen(nom)
