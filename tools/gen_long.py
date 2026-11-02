#!/usr/bin/env python3
"""Long bots (scenarios/long/, validated with DDSOM_LONG=1 tools/validate_all.sh): games of 7,500 to 90,000 frames
played by `examples/autobot.rs` (several players, spells, route votes, shops, difficulties, mortal players).

    tools/gen_long.py [name...]      (all by default; needs port/target/release/examples/autobot)

Writes scenarios/long/<name>.txt (the input scenario produced by the bot) and <name>.images (its frame count).
Variables: DDSOM_ROMS (rom), DDSOM_EEPROM (testdata/eeprom_ddsom.bin), GEN_LONG_LOG (file: bot log). See
docs/couverture.md ("Bots longs").
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import bot_lib  # noqa: E402

ROOT = os.path.dirname(HERE)
AUTOBOT = os.path.join(ROOT, 'port/target/release/examples/autobot')
OUT_DIR = os.path.join(ROOT, 'scenarios/long')

# characters (`chars`): selection number 0 fighter, 1 magic-user, 2 cleric, 3 thief, 4 dwarf, 5 elf for player 1;
# for players 2 to 4, `bot_lib.build` shifts the cursor (k + player - 1 steps) and the cursor skips characters already
# taken: the character actually obtained, read from RAM (player +3, frame 4700), is given in the comment ("alt": same
# character as another player, second colour set, odd +3); pokes "frame:address:value:size"
# (ff802a: difficulty, ff8072: language); plan "scene:duration,..." (decimal scene numbers)
SPECS = {
    # two magic-users (the second one "alt"), frequent spells and items
    'long_2p_magies': dict(players=2, chars=(1, 4), frames=90000, seed=501, god='0x70', cast=30, charges=2),
    # thief alone, 28 scenes (1, 5, 9... 109), difficulty 7, language 4
    'long_28_scenes': dict(players=1, chars=(3,), frames=90000, seed=502, god='0x70',
                           plan=','.join(f'{s}:2800' for s in range(1, 112, 4)),
                           pokes=['800:ff802a:7:1', '800:ff8072:4:2']),
    # three mortal players (fighter, elf, dwarf)
    'long_3p_mortels': dict(players=3, chars=(0, 2, 3), frames=60000, seed=503),
    # four players (fighter, elf, elf alt, thief)
    'long_4p': dict(players=4, chars=(0, 2, 4, 5), frames=60000, seed=504, god='0x70', cast=20, charges=2),
    # cleric alone from scene 20 (0x14; not a route choice: the votes are in the `long_votes_*`)
    'long_scene_20': dict(players=1, chars=(2,), frames=60000, seed=505, god='0x70', plan='20:40000', cast=10, charges=1),
    # elf and fighter from scene 56 (0x38)
    'long_scene_56': dict(players=2, chars=(5, 3), frames=60000, seed=506, god='0x70', plan='56:40000', cast=10,
                          charges=1),
    # magic-user, shops 69 to 82 (0x45-0x52), difficulty 0
    'long_boutiques': dict(players=1, chars=(1,), frames=40000, seed=507, god='0x70',
                           plan=','.join(f'{s}:1500' for s in range(69, 83)), pokes=['800:ff802a:0:1']),
    # mortal fighter and magic-user, difficulty 3, scenes 29 to 56
    'long_2p_d3_mortels': dict(players=2, chars=(0, 4), frames=60000, seed=508,
                               plan=','.join(f'{s}:2500' for s in range(29, 57, 3)), cast=15, charges=1,
                               pokes=['800:ff802a:3:1']),
    # mortal thief alone, natural play
    'long_voleur_mortel': dict(players=1, chars=(3,), frames=60000, seed=509, cast=10, charges=1),
    # elf alone, very frequent items and spells, scenes 1 to 56
    'long_elfe_objets': dict(players=1, chars=(5,), frames=60000, seed=510, god='0x70', cast=40, charges=3,
                             plan=','.join(f'{s}:3000' for s in range(1, 57, 5))),
    # dwarf alone, very frequent spells, scenes 57 to 86
    'long_nain_57_86': dict(players=1, chars=(4,), frames=60000, seed=511, god='0x70', cast=40, charges=3,
                            plan=','.join(f'{s}:2500' for s in range(57, 87, 3))),
    # mortal magic-user and elf, difficulty 7
    'long_2p_d7_mortels': dict(players=2, chars=(1, 2), frames=60000, seed=512, cast=10, charges=1,
                               pokes=['800:ff802a:7:1']),
    # four players (magic-user, cleric, elf, thief) from scene 56 (0x38)
    'long_4p_scene_56': dict(players=4, chars=(1, 3, 4, 5), frames=60000, seed=513, god='0x70', plan='56:40000',
                             cast=15, charges=1),
    # fighter and dwarf from scene 20 (0x14)
    'long_scene_20_2p': dict(players=2, chars=(0, 1), frames=60000, seed=514, god='0x70', plan='20:40000'),
}

# spell bots: each character's valid spells and items, cast in turn (--spells: identifier put in the first equipment
# row, the game's special attack converts it to a spell number through table 0x372e4), magic-user rods (--rods); see
# docs/couverture.md ("Sorts et objets")


def row_ident(spell):
    """Equipment row identifier giving spell number `spell` (inverse of table 0x372e4)."""
    if spell == 0xc:
        return 0x2e
    return spell - 0xd if spell >= 0xd else 0x21 + spell


SHARED = [0, 1, 2, 4, 5, 6, 7, 8, 9, 10, *range(13, 21)]  # without 12 (weapon swap: needs a picked-up item)
SPELLS = {
    'combattant': SHARED,
    'nain': SHARED,
    'voleur': SHARED + [11],
    'clerc': [6, 7, 8, 9, 10, *range(13, 21), *range(0x24, 0x2e)],
    'elfe': [x for x in range(0x1d) if x != 12],
    'magicien': [0, 1, 6, 7, 9, 10, *range(13, 21), *range(0x18, 0x25)],
}
# actual selection numbers: 0 fighter, 1 magic-user, 2 cleric, 3 thief, 4 dwarf, 5 elf (the player record's +3 is
# 0, 8, 2, 0xa, 6, 4; class = +3 >> 1: fighter 0, cleric 1, thief 2, dwarf 3, magic-user 4, elf 5). The keys are the
# (French) scenario name suffixes.
CHARACTERS = {'combattant': 0, 'magicien': 1, 'clerc': 2, 'voleur': 3, 'nain': 4, 'elfe': 5}
for name, character in CHARACTERS.items():
    SPECS['sorts_' + name] = dict(players=1, chars=(character,), frames=30000, seed=601 + character, god='0x70',
                                  plan='1:30000', spells=','.join(f'{row_ident(x):x}' for x in SPELLS[name]),
                                  spell_period=150, rods=350 if name == 'magicien' else 0)


# high score names (F3, 0x180c8): a first byte from -2 to -7 selects a preset name, and very different scores put the
# players at every rank; four mortal players who leave the game one by one
def names(first_bytes, scores):
    return [f'5000:{0xff86f6 + 0x200 * k:x}:{p << 24}:4' for k, p in enumerate(first_bytes)] + \
           [f'5000:{0xff86ee + 0x200 * k:x}:{sc}:4' for k, sc in enumerate(scores)]


SPECS['sorts_noms_a'] = dict(players=4, chars=(0, 1, 2, 3), frames=40000, seed=611, pokes=names(
    (0xfe, 0xfd, 0xfc, 0xfb), (0x00800000, 0x00030000, 0x00001000, 0x00000500)))
SPECS['sorts_noms_b'] = dict(players=4, chars=(4, 5, 0, 1), frames=40000, seed=612, pokes=names(
    (0xfa, 0xf9, 0xff, 0xfe), (0x00002000, 0x00600000, 0x00010000, 0x00004000)))


# character selection of a player joining mid-game (interface cursor, 0x158c6 and 0x15958): keys held for more than
# 60 frames (cursor repeat) and choices that jump over characters already taken
SPECS['sorts_select'] = dict(players=4, chars=(5, 0, 3, 1), frames=12000, seed=621, god='0x70', late=[
    (2, 3500, [('Right', 80, 100), ('Left', 230, 90), ('Right', 380, 6), ('Right', 420, 6), ('Button 1', 520, 6)]),
    (3, 3900, [('Left', 80, 140), ('Right', 300, 6), ('Right', 340, 6), ('Left', 380, 90), ('Button 1', 520, 6)]),
    (4, 4300, [('Right', 60, 75), ('Right', 200, 130), ('Left', 400, 6), ('Button 1', 520, 6)])])


# passage doors to a hidden zone (SET 27 and 29, scenes 34, 67, 68 and 110): the players are put in front of the door
# (position of the 16-byte rows 0xd84e8 and 0xd8e5e) and hold the key for 25 frames or more; a second door brings them back
SPECS['sorts_porte_34'] = dict(players=1, chars=(0,), frames=7500, seed=631, god='0x70', plan='1:600,34:6500',
                               doors=['4400:0x70:0x137:3', '4800:0x124:0x14f:3'],
                               pokes=[f'{f}:ffff92:0:2' for f in range(4100, 4400, 2)])


# route votes (EFFCT 23) and shops (0x45 to 0x52) played at random (autobot `--wander`: cursors moved every way,
# buttons tapped, abstentions that let the countdown run out), with 2 to 4 players, the three-player interface
# (ff8020 = 1: narrow F3 columns), languages 4 and 6 (ff8072: choice texts in bytes; language 0, which the settings
# menu skips, makes the original machine reboot in scene 33); the players' gold (+0x114, BCD) is set in turn to 99,999
# and to 0 (purchases and refusals)
def purse(players, start, end, period=2500):
    return [f'{f}:{0xff85de + 0x114 + 0x200 * k:x}:{0x99999 if (f // period) % 3 else 0}:4'
            for f in range(start, end, period) for k in range(players)]


def three_player_cabinet(pokes):
    # three-player cabinet: FF8020 = 1 (bot_lib.build writes 2 for more than two players, at the same frame)
    return pokes + ['800:ff8020:1:1']


# scene-exit votes (scenes 2, 13, 33 and 38 in decimal: answer to dialogue 13, question 136 to 138, dialogue 40,
# dialogue 70 towards 39, 41 or 43): after 1,200 game frames the scene exit is forced (`:t`, transition script like
# the `trans_*` of gen_scene_end.py; `:f`, exit sequence like `end_38`), the bot votes, then plays 400 frames in the
# chosen scene; the former `long_route_14` and `long_route_38`, renamed `long_scene_20` and `long_scene_56`, never voted
ROUTES = ','.join(['2:1200:t', '13:1200:t', '33:1200:t', '38:1200:f'] * 3)
SPECS['long_votes_4p'] = dict(players=4, chars=(0, 1, 2, 3), frames=50000, seed=701, god='0x70', plan=ROUTES,
                              wander=True)
SPECS['long_votes_3p_l4'] = dict(players=3, chars=(4, 5, 0), frames=50000, seed=702, god='0x70', plan=ROUTES,
                                 wander=True, pokes=three_player_cabinet(['800:ff8072:4:2']))
SPECS['long_votes_2p_l6'] = dict(players=2, chars=(2, 3), frames=50000, seed=703, god='0x70', plan=ROUTES,
                                 wander=True, pokes=['800:ff8072:6:2'])
SHOPS = ','.join(f'{s}:2000' for s in range(69, 83))
SPECS['long_boutiques_4p'] = dict(players=4, chars=(5, 4, 3, 2), frames=34000, seed=711, god='0x70', plan=SHOPS,
                                  wander=True, pokes=purse(4, 4500, 34000))
SPECS['long_boutiques_3p_l6'] = dict(players=3, chars=(1, 0, 5), frames=34000, seed=712, god='0x70', plan=SHOPS,
                                     wander=True, pokes=three_player_cabinet(['800:ff8072:6:2'] + purse(3, 4500, 34000)))
SPECS['long_boutiques_2p_l4'] = dict(players=2, chars=(3, 0), frames=34000, seed=713, god='0x70', plan=SHOPS,
                                     wander=True, pokes=['800:ff8072:4:2'] + purse(2, 4500, 34000))


# player interface (F3) beyond the cursor and the names:
# * frame cleared when player 3 or 4 falls during a boss fight (`A5+0x50` set by the bosses, 0x16f54): four mortal
#   players, players 3 and 4 set to 1 HP every 600 frames (`--frail`), boss scenes;
# * "INSERT COIN" banners with missing coins (0x155ba) on a free slot: two players in a three- or four-player cabinet
#   (separate coin slots, ff8021 = 3), price of 2 coins per credit (`A5+0x1e`) or two-credit games (`A5+0x457` and
#   following), set after the start, and coins dropped one by one in slots 3 and 4 without pressing Start (`events`:
#   raw inputs added to the base)
BOSS = ','.join(f'{s}:3000' for s in (24, 26, 32, 40, 44, 46, 54, 58, 64))
SPECS['long_boss_4p_frail'] = dict(players=4, chars=(0, 1, 2, 3), frames=40000, seed=721, plan=BOSS, frail=600,
                                   cast=20, charges=1)
SPECS['long_credits_prix2'] = dict(players=2, chars=(3, 4), frames=24000, seed=722, cast=10, charges=1,
                                   plan='1:3000,5:3000,9:3000,12:3000,16:3000,19:3000,23:3000',
                                   pokes=['800:ff8021:3:1', '800:ff8020:1:1', '3000:ff801e:2:1'],
                                   events=[(5000, 'Coin 3'), (11000, 'Coin 3'), (15000, 'Coin 3')])
SPECS['long_credits_deux'] = dict(players=2, chars=(5, 2), frames=24000, seed=723, cast=10, charges=1,
                                  plan='2:3000,6:3000,10:3000,13:3000,17:3000,21:3000,25:3000',
                                  pokes=['800:ff8021:3:1', '800:ff8020:2:1'] +
                                  [f'3000:{0xff8457 + 0x10 * k:x}:1:1' for k in range(4)],
                                  events=[(5000, 'Coin 3'), (7000, 'Coin 4'), (12000, 'Coin 4')])

# * high score emblem (F4, 0x18a0e): a player who went through scene 46 (`+0x193` = 1, set by 0x24394) and enters the
#   table leaves byte `+0xd` there; the attract's score screen then creates the row's EFFCT 0x5b object. `+0x193` is
#   set by poke; without periodic coins, the players set to 1 HP (frame 9000) fall one after the other, the game ends
#   and the attract shows the table
# First try of this bot, kept: with periodic coins, a player joins while the inputs are frozen (`A5+0x44`: state
# 0x00040800, step 8 of the revival, 0x23c7a, frame 19,259), a routine that had no native
SPECS['long_rejoint_fige'] = dict(players=4, chars=(2, 3, 4, 5), frames=40000, seed=613, pokes=names(
    (0x41, 0xfe, 0x42, 0xfb), (0x00900000, 0x00050000, 0x00002000, 0x00700000)) +
    [f'{f}:{0xff85de + 0x193 + 0x200 * k:x}:1:1' for f in range(5000, 30000, 1000) for k in range(4)])
SPECS['long_noms_embleme'] = dict(players=4, chars=(2, 3, 4, 5), frames=30000, seed=613, coin_period=0, pokes=names(
    (0x41, 0xfe, 0x42, 0xfb), (0x00900000, 0x00050000, 0x00002000, 0x00700000)) +
    [f'{f}:{0xff85de + 0x193 + 0x200 * k:x}:1:1' for f in (5000, 7000, 8900) for k in range(4)] +
    [f'9000:{0xff85de + 0x62 + 0x200 * k:x}:1:2' for k in range(4)])


def gen(name):
    sp = SPECS[name]
    roms = os.environ.get('DDSOM_ROMS', os.path.join(ROOT, 'rom'))
    eeprom = os.environ.get('DDSOM_EEPROM', os.path.join(ROOT, 'testdata/eeprom_ddsom.bin'))
    lines = bot_lib.build(frames=sp['frames'], players=sp['players'], chars=sp['chars'],
                          coin_period=sp.get('coin_period', 1500), base_only=True, pokes=sp.get('pokes', []),
                          late=sp.get('late', ()))
    for f, input_name in sp.get('events', ()):
        lines += [f'{f} {input_name} 1', f'{f + 6} {input_name} 0']
    lines.sort(key=lambda l: int(l.split()[0]))
    os.makedirs(OUT_DIR, exist_ok=True)
    base = os.path.join(OUT_DIR, name + '.base')
    with open(base, 'w') as f:
        f.write('\n'.join(lines) + '\n')
    out = os.path.join(OUT_DIR, name + '.txt')
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
    if sp.get('frail'):
        cmd += ['--frail', str(sp['frail'])]
    if sp.get('wander'):
        cmd += ['--wander']
    if sp.get('rods'):
        cmd += ['--rods', str(sp['rods'])]
    r = subprocess.run(cmd, capture_output=True, text=True)
    os.remove(base)
    with open(os.path.join(OUT_DIR, name + '.images'), 'w') as f:
        f.write(f"{sp['frames']}\n")
    if os.environ.get('GEN_LONG_LOG'):
        with open(os.environ['GEN_LONG_LOG'], 'w') as f:
            f.write(r.stderr)
    visited = [l for l in (r.stdout + r.stderr).splitlines() if l.startswith('visited scenes')]
    print(name, r.returncode, *visited)


for scenario in sys.argv[1:] or SPECS:
    gen(scenario)
