"""Génère les scénarios d'entrée de scène (scenarios/scene/scene_NN.txt) avec le bot réactif autobot.

    python3 tools/gen_scene.py [numéro de scène ...]        (une scène représentative par séquence si aucun)

Chaque scénario entre directement dans la scène (FF8034 écrit pendant la sélection du personnage) : le bot
joue ensuite avec un joueur invincible. Ils exercent la phase 8 (séquence d'ouverture de chaque scène,
`port/ddsom-game/src/scene/entry.rs`) et le lecteur de script d'apparition. Prérequis : `cargo build --release
-p ddsom-game --examples`, DDSOM_ROMS / DDSOM_EEPROM. Les scénarios produits ne contiennent que des entrées
et des écritures RAM (`poke`). Voir docs/scenes.md.
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
FRAMES = 6500

# une scène par fonction de séquence d'ouverture (la table complète est dans docs/scenes.md)
SCENES = [1, 2, 3, 4, 5, 7, 8, 9, 10, 11, 14, 24, 25, 26, 27, 28, 32, 33, 34, 35, 38, 39, 40, 41, 42, 44, 47, 54, 58,
          59, 64, 65, 66, 67, 90, 105, 109, 110, 111]


def generate(scene):
    name = f"scene_{scene:02d}"
    base = os.path.join(ROOT, "work", name + ".base")
    os.makedirs(os.path.dirname(base), exist_ok=True)
    lines = make_bot.build(frames=FRAMES, players=1, chars=(0,), coin_period=1500, base_only=True)
    with open(base, "w") as f:
        f.write("\n".join(lines) + "\n")
    out = os.path.join(ROOT, "scenarios", "scene", name + ".txt")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    cmd = [AUTOBOT, ROMS, EEPROM, base, str(FRAMES), out, "--players", "1", "--seed", "1", "--god", "0x70",
           "--plan", f"{scene}:{FRAMES}"]
    log = subprocess.run(cmd, capture_output=True, text=True)
    with open(os.path.join(ROOT, "work", name + ".log"), "w") as f:
        f.write(log.stderr)
    return log.returncode


if __name__ == "__main__":
    for s in [int(a) for a in sys.argv[1:]] or SCENES:
        print(s, "ok" if generate(s) == 0 else "ECHEC", flush=True)
