#!/usr/bin/env python3
"""Bots longs (scenarios/long/, validés avec DDSOM_LONG=1 tools/validate_all.sh) : parties de 40 000 à 90 000 images
jouées par `examples/autobot.rs` (plusieurs joueurs, magies, routes, boutiques, difficultés, joueurs mortels).

    tools/gen_long.py [nom...]      (tous par défaut ; il faut port/target/release/examples/autobot)

Écrit scenarios/long/<nom>.txt (le scénario d'entrées produit par le bot) et <nom>.images (son nombre d'images).
Variables : DDSOM_ROMS (rom), DDSOM_EEPROM (testdata/eeprom_ddsom.bin), GEN_LONG_LOG (fichier : journal du bot). Voir docs/couverture.md (« Bots longs »).
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

# bots de sorts : les magies et objets valides de chaque personnage, lancés tour à tour (--spells : identifiant posé dans la
# 1re ligne d'équipement, l'attaque spéciale du jeu le convertit en numéro de sort par la table 0x372e4), baguettes du
# magicien (--rods) ; voir docs/couverture.md (« Sorts et objets »)


def ident(sort):
    """Identifiant de ligne d'équipement donnant le numéro de sort `sort` (inverse de la table 0x372e4)."""
    if sort == 0xc:
        return 0x2e
    return sort - 0xd if sort >= 0xd else 0x21 + sort


COMMUN = [0, 1, 2, 4, 5, 6, 7, 8, 9, 10, *range(13, 21)]  # sans 12 (échange d'arme : exige un objet ramassé)
SORTS = {
    'combattant': COMMUN,
    'nain': COMMUN,
    'voleur': COMMUN + [11],
    'clerc': [6, 7, 8, 9, 10, *range(13, 21), *range(0x24, 0x2e)],
    'elfe': [x for x in range(0x1d) if x != 12],
    'magicien': [0, 1, 6, 7, 9, 10, *range(13, 21), *range(0x18, 0x25)],
}
# numéros de sélection réels : 0 combattant, 1 magicien, 2 clerc, 3 voleur, 4 nain, 5 elfe (le +3 de l'enregistrement du
# joueur vaut 0, 8, 2, 0xa, 6, 4 ; classe = +3 >> 1 : combattant 0, clerc 1, voleur 2, nain 3, magicien 4, elfe 5).
# Les commentaires des bots plus haut (« clerc » pour le 1, « magicien » pour le 4...) suivent un ordre erroné.
PERSOS = {'combattant': 0, 'magicien': 1, 'clerc': 2, 'voleur': 3, 'nain': 4, 'elfe': 5}
for i, (nom, perso) in enumerate(PERSOS.items()):
    SPECS['sorts_' + nom] = dict(players=1, chars=(perso,), frames=30000, seed=601 + perso, god='0x70', plan='1:30000',
                                 spells=','.join(f'{ident(x):x}' for x in SORTS[nom]), spell_period=150,
                                 rods=350 if nom == 'magicien' else 0)


# noms des tableaux de scores (F3, 0x180c8) : un premier octet de -2 à -7 désigne un nom prédéfini, et des scores très
# différents font entrer le joueur à tous les rangs ; quatre joueurs mortels qui quittent la partie un à un
def noms(premiers, scores):
    return [f'5000:{0xff86f6 + 0x200 * k:x}:{p << 24}:4' for k, p in enumerate(premiers)] + \
           [f'5000:{0xff86ee + 0x200 * k:x}:{sc}:4' for k, sc in enumerate(scores)]


SPECS['sorts_noms_a'] = dict(players=4, chars=(0, 1, 2, 3), frames=40000, seed=611, pokes=noms(
    (0xfe, 0xfd, 0xfc, 0xfb), (0x00800000, 0x00030000, 0x00001000, 0x00000500)))
SPECS['sorts_noms_b'] = dict(players=4, chars=(4, 5, 0, 1), frames=40000, seed=612, pokes=noms(
    (0xfa, 0xf9, 0xff, 0xfe), (0x00002000, 0x00600000, 0x00010000, 0x00004000)))


# sélection du personnage d'un joueur qui rejoint en cours de partie (curseur de l'interface, 0x158c6 et 0x15958) : touches
# tenues plus de 60 images (répétition du curseur) et choix qui passent par-dessus les personnages déjà pris
SPECS['sorts_select'] = dict(players=4, chars=(5, 0, 3, 1), frames=12000, seed=621, god='0x70', late=[
    (2, 3500, [('Right', 80, 100), ('Left', 230, 90), ('Right', 380, 6), ('Right', 420, 6), ('Button 1', 520, 6)]),
    (3, 3900, [('Left', 80, 140), ('Right', 300, 6), ('Right', 340, 6), ('Left', 380, 90), ('Button 1', 520, 6)]),
    (4, 4300, [('Right', 60, 75), ('Right', 200, 130), ('Left', 400, 6), ('Button 1', 520, 6)])])


# portes de passage vers une zone cachée (SET 27 et 29, scènes 34, 67, 68 et 110) : les joueurs sont posés devant la porte
# (position des lignes de 16 octets 0xd84e8 et 0xd8e5e) et tiennent la touche 25 images ou plus ; une seconde porte ramène
SPECS['sorts_porte_34'] = dict(players=1, chars=(0,), frames=7500, seed=631, god='0x70', plan='1:600,34:6500',
                               doors=['4400:0x70:0x137:3', '4800:0x124:0x14f:3'],
                               pokes=[f'{f}:ffff92:0:2' for f in range(4100, 4400, 2)])


def gen(nom):
    sp = SPECS[nom]
    roms = os.environ.get('DDSOM_ROMS', os.path.join(RACINE, 'rom'))
    eeprom = os.environ.get('DDSOM_EEPROM', os.path.join(RACINE, 'testdata/eeprom_ddsom.bin'))
    lignes = bot_lib.build(frames=sp['frames'], players=sp['players'], chars=sp['chars'], coin_period=1500,
                           base_only=True, pokes=sp.get('pokes', []), late=sp.get('late', ()))
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
    if sp.get('spells'):
        cmd += ['--spells', sp['spells'], '--spell-period', str(sp['spell_period'])]
    for d in sp.get('doors', ()):
        cmd += ['--door', d]
    if sp.get('rods'):
        cmd += ['--rods', str(sp['rods'])]
    r = subprocess.run(cmd, capture_output=True, text=True)
    os.remove(base)
    with open(os.path.join(SORTIE, nom + '.images'), 'w') as f:
        f.write(f"{sp['frames']}\n")
    if os.environ.get('GEN_LONG_LOG'):
        with open(os.environ['GEN_LONG_LOG'], 'w') as f:
            f.write(r.stderr)
    visites = [l for l in (r.stdout + r.stderr).splitlines() if l.startswith('scènes visitées')]
    print(nom, r.returncode, *visites)


for nom in sys.argv[1:] or SPECS:
    gen(nom)
