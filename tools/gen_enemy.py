"""Génère les scénarios de validation des ennemis (scenarios/enemy/*.txt) avec le bot réactif autobot.

    python3 tools/gen_enemy.py [nom ...]        (tous si aucun nom)

Chaque scénario entre directement dans une scène où le type d'ennemi apparaît (FF8034 écrit pendant la
sélection du personnage), puis enchaîne d'autres scènes ; le joueur est invincible et le bot combat les
ennemis qu'il rencontre. Prérequis : `cargo build --release -p ddsom-game --examples`, DDSOM_ROMS / DDSOM_EEPROM.
Les scénarios produits ne contiennent que des entrées et des écritures RAM (`poke`). Voir docs/familles/enemy.md.
Les scènes se trouvent avec `TYPES_SCENES=1 examples/types` sur les scénarios de `scenarios/cov/`.
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

# nom -> (scènes "scène:images,..." en décimal, crans du curseur de personnage, PV du joueur remis à, dossier)
# dossier "ko" : le scénario diverge à cause d'une autre routine (voir scenarios/ko/README.md)
SPECS = {
    "skeleton_scene13": ("13:20000", 0, "0x70", "enemy"),
    "skeleton_scene58": ("58:20000", 1, "0x70", "enemy"),
    "skeleton_scene64": ("64:20000", 2, "0x70", "enemy"),
    "elf_scene96": ("96:20000", 0, "0x70", "enemy"),
    "elf_scene92": ("92:20000", 3, "0x70", "enemy"),
    "elf_scene94": ("94:20000", 4, "0x70", "enemy"),
    "trog_scene104": ("104:20000", 0, "0x70", "enemy"),
    "trog_scene101": ("101:20000", 2, "0x70", "enemy"),
    "trog_scene101b": ("101:20000", 4, "0x70", "enemy"),
    "ghoul_scene95": ("95:20000", 0, "0x70", "enemy"),
    "ghoul_scene26": ("26:20000", 2, "0x70", "enemy"),
    "ghoul_scene97": ("97:20000", 3, "0x70", "ko"),
    "ghoul_scene19": ("19:20000", 1, "0x70", "ko"),
    "beetle_scene16": ("16:20000", 0, "0x70", "enemy"),
    "beetle_scene18": ("18:20000", 2, "0x70", "enemy"),
    "beetle_scene43": ("43:20000", 1, "0x70", "enemy"),
    "beetle_scene61": ("61:20000", 3, "0x70", "enemy"),
    "beetle_scene112": ("112:20000", 4, "0x70", "enemy"),
    "scorpion_scene36": ("36:20000", 0, "0x70", "enemy"),
    "scorpion_scene37": ("37:20000", 2, "0x70", "enemy"),
    "scorpion_scene38": ("38:20000", 1, "0x70", "enemy"),
    "scorpion_scene39": ("39:20000", 3, "0x70", "enemy"),
    "scorpion_scene53": ("53:20000", 4, "0x70", "enemy"),
    "gargoyle_scene42": ("42:20000", 0, "0x70", "enemy"),
    "gargoyle_scene55": ("55:20000", 2, "0x70", "enemy"),
    "gargoyle_scene63": ("63:20000", 1, "0x70", "enemy"),
    "hellhound_scene21": ("21:20000", 0, "0x70", "enemy"),
    "hellhound_scene40": ("40:20000", 1, "0x70", "enemy"),
    "hellhound_scene49": ("49:20000", 2, "0x70", "enemy"),
    "hellhound_scene50": ("50:20000", 3, "0x70", "enemy"),
    "hellhound_scene55": ("55:20000", 4, "0x70", "enemy"),
    "hellhound_scene62": ("62:20000", 5, "0x70", "enemy"),
    "ogre_scene65": ("65:20000", 0, "0x70", "enemy"),
    "ogre_scene66": ("66:20000", 3, "0x70", "enemy"),
    "kobold_scene7": ("7:20000", 0, "0x70", "enemy"),
    "kobold_scene17": ("17:20000", 1, "0x70", "enemy"),
    "kobold_scene25": ("25:20000", 2, "0x70", "enemy"),
    "kobold_scene29": ("29:20000", 3, "0x70", "enemy"),
    "kobold_scene50": ("50:20000", 4, "0x70", "enemy"),
    "kobold_scene52": ("52:20000", 5, "0x70", "enemy"),
    "kobold_scene99": ("99:20000", 0, "0x70", "enemy"),
}


def generate(name):
    plan, k, god, folder = SPECS[name]
    base = os.path.join(ROOT, "work", name + ".base")
    os.makedirs(os.path.dirname(base), exist_ok=True)
    lines = make_bot.build(frames=FRAMES, players=1, chars=(k,), coin_period=1500, base_only=True)
    with open(base, "w") as f:
        f.write("\n".join(lines) + "\n")
    out = os.path.join(ROOT, "scenarios", folder, name + ".txt")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    cmd = [AUTOBOT, ROMS, EEPROM, base, str(FRAMES), out, "--players", "1", "--seed", "1", "--god", god, "--plan", plan]
    log = subprocess.run(cmd, capture_output=True, text=True)
    with open(os.path.join(ROOT, "work", name + ".log"), "w") as f:
        f.write(log.stderr)
    return log.returncode


if __name__ == "__main__":
    for n in sys.argv[1:] or SPECS:
        print(n, "ok" if generate(n) == 0 else "ECHEC")
