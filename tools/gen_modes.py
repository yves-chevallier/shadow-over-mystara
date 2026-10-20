"""Génère le scénario du générique de fin (scenarios/modes/ending.txt) avec le bot réactif autobot.

    python3 tools/gen_modes.py [images]

Le scénario saute dans la dernière scène (105, saut de `FF8034` pendant la sélection du personnage), le bot y
combat le boss final, dont les PV sont affaiblis après 1 500 images ; la scène finie, la phase 28 (générique)
commence vers l'image 7 900. Les scénarios `attract.txt` (aucune entrée) et `gameover.txt` (début de
`bot_char4.txt` jusqu'à l'écran de classement) se font à la main. Prérequis : `cargo build --release -p
ddsom-game --examples`, DDSOM_ROMS / DDSOM_EEPROM. Voir docs/modes.md.
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

frames = int(sys.argv[1]) if len(sys.argv) > 1 else 12000
out = os.path.join(ROOT, "scenarios", "modes", "ending.txt")
base = os.path.join(ROOT, "work", "ending.base")
os.makedirs(os.path.dirname(base), exist_ok=True)
lines = make_bot.build(frames=frames, players=1, chars=(0,), coin_period=1500, base_only=True)
with open(base, "w") as f:
    f.write("\n".join(lines) + "\n")
cmd = [AUTOBOT, ROMS, EEPROM, base, str(frames), out, "--players", "1", "--seed", "1", "--god", "0x70",
       "--plan", f"105:{frames}", "--weaken", "1500"]
sys.exit(subprocess.run(cmd).returncode)
