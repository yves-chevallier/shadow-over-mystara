"""Génère les scénarios de validation des boss (scenarios/boss/*.txt) avec le bot réactif autobot.

    python3 tools/gen_boss.py [nom ...]        (tous si aucun nom)

Chaque scénario entre directement dans la scène du boss (FF8034 écrit pendant la sélection du personnage)
et le bot le combat jusqu'à ses dernières phases ; sans `weaken`, les PV du boss ne sont pas touchés.
Prérequis : `cargo build --release -p ddsom-game --examples`, DDSOM_ROMS / DDSOM_EEPROM. Les scénarios produits
ne contiennent que des entrées et des écritures RAM (`poke`). Voir docs/familles/boss.md.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
import bot_lib as make_bot  # noqa: E402

ROOT = os.path.join(os.path.dirname(__file__), "..")
AUTOBOT = os.path.join(ROOT, "port/target/release/examples/autobot")
ROMS = os.environ.get("DDSOM_ROMS", os.path.join(ROOT, "rom"))
EEPROM = os.environ.get("DDSOM_EEPROM", os.path.join(ROOT, "testdata/eeprom_ddsom.bin"))
FRAMES = 12000
# scénarios tronqués : le bot tue le boss naturellement, et la fin de scène diverge (routines d'autres modules, voir
# scenarios/ko/README.md) ; on garde les entrées jusqu'à l'image indiquée
TRONQUE = {"boss18_scene26": 10300}

# nom -> (type de boss, scènes "scène:images,...", crans du curseur, PV du joueur remis à, affaiblissement, dossier)
# dossier "ko" : le scénario diverge à cause d'une autre routine (voir scenarios/ko/LISEZMOI.txt)
SPECS = {
    "boss17_scene13": (17, "13:11000", 0, "0x70", 0, "boss"),
    "boss6_scene24": (6, "24:11000", 0, "0x70", 0, "ko"),
    "boss10_scene65": (10, "65:11000", 0, "0x70", 0, "ko"),
    "boss10_scene66": (10, "66:11000", 2, "0x70", 0, "boss"),
    "boss18_scene26": (18, "26:11000", 0, "0x70", 0, "boss"),
    "boss18_scene26_mort": (18, "26:11000", 0, "0x70", 1500, "boss"),
    "boss19_scene32": (19, "32:11000", 0, "0x70", 0, "boss"),
    "boss19_scene32_mort": (19, "32:11000", 0, "0x70", 1500, "boss"),
    "boss12_scene40": (12, "40:11000", 0, "0x70", 0, "boss"),
    "boss12_scene40_mort": (12, "40:11000", 0, "0x70", 1500, "boss"),
    "boss11_scene42": (11, "42:11000", 0, "0x70", 0, "boss"),
    "boss11_scene42_mort": (11, "42:11000", 0, "0x70", 1500, "boss"),
    "boss21_scene44": (21, "44:11000", 0, "0x70", 0, "boss"),
    "boss21_scene44_mort": (21, "44:11000", 0, "0x70", 1500, "ko"),
    "boss15_scene46": (15, "46:11000", 0, "0x70", 0, "boss"),
    "boss15_scene46_mort": (15, "46:11000", 0, "0x70", 1500, "boss"),
    "boss5_scene58": (5, "58:11000", 0, "0x70", 0, "boss"),
    "boss5_scene58_mort": (5, "58:11000", 0, "0x70", 1500, "ko"),
    "boss8_scene64": (8, "64:11000", 0, "0x70", 0, "boss"),
    "boss8_scene64_mort": (8, "64:11000", 0, "0x70", 1500, "boss"),
    "boss13_dragon": (13, "7:3000,9:3000", 0, "0x70", 0, "ko"),
}


def generate(name):
    _, plan, k, god, weaken, folder = SPECS[name]
    base = os.path.join(ROOT, "work", name + ".base")
    os.makedirs(os.path.dirname(base), exist_ok=True)
    lines = make_bot.build(frames=FRAMES, players=1, chars=(k,), coin_period=1500, base_only=True)
    with open(base, "w") as f:
        f.write("\n".join(lines) + "\n")
    out = os.path.join(ROOT, "scenarios", folder, name + ".txt")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    cmd = [AUTOBOT, ROMS, EEPROM, base, str(FRAMES), out, "--players", "1", "--seed", "1", "--god", god, "--plan", plan]
    if weaken:
        cmd += ["--weaken", str(weaken)]
    log = subprocess.run(cmd, capture_output=True, text=True)
    with open(os.path.join(ROOT, "work", name + ".log"), "w") as f:
        f.write(log.stderr)
    if name in TRONQUE:
        gardees = [l for l in open(out).read().split("\n") if not (l.split()[:1] and l.split()[0].isnumeric() and int(l.split()[0]) > TRONQUE[name])]
        with open(out, "w") as f:
            f.write("\n".join(gardees))
    return log.returncode


if __name__ == "__main__":
    for n in sys.argv[1:] or SPECS:
        print(n, "ok" if generate(n) == 0 else "ECHEC")
