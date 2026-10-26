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
# scénarios tronqués : après la mort du boss, un bouton pressé ou maintenu pendant la fin de scène déclenche l'avance
# rapide (« tempête de pas », voir docs/couverture.md), dont le nombre de pas dépend de la durée des routines natives ;
# on garde les entrées jusqu'à l'image indiquée, avant que le bot ne touche aux boutons à ce moment
TRONQUE = {
    "boss18_scene26": 10900,
    "boss18_scene26_mort": 7738,
    "boss19_scene32_mort": 6208,
    "boss15_scene46_mort": 7108,
    "boss12_scene40_mort": 6208,
    "boss11_scene42_mort": 6208,
    "boss21_scene44_mort": 7738,
    "boss5_scene58_mort": 6208,
    "boss8_scene64_mort": 6649,
}

# nom -> (type de boss, scènes "scène:images,...", crans du curseur, PV du joueur remis à, affaiblissement, dossier)
# 7e élément facultatif "type:variante:image[:joueurs]" : boss créé à la main (autobot --spawn) à cette image, pour un type que
# les scènes du jeu ne font pas apparaître dans les durées jouées
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
    "boss21_scene44_mort": (21, "44:11000", 0, "0x70", 1500, "boss"),
    "boss15_scene46": (15, "46:11000", 0, "0x70", 0, "boss"),
    "boss15_scene46_mort": (15, "46:11000", 0, "0x70", 1500, "boss"),
    "boss5_scene58": (5, "58:11000", 0, "0x70", 0, "boss"),
    "boss5_scene58_mort": (5, "58:11000", 0, "0x70", 1500, "boss"),
    "boss8_scene64": (8, "64:11000", 0, "0x70", 0, "boss"),
    "boss8_scene64_mort": (8, "64:11000", 0, "0x70", 1500, "boss"),
    "boss0_poke": (0, "20:11000", 0, "0x70", 0, "boss", "0:0:3000"),
    "boss0_poke_mort": (0, "20:11000", 0, "0x70", 1500, "boss", "0:0:3000"),
    "boss16_poke": (16, "14:11000", 0, "0x70", 0, "boss", "16:0:3000"),
    "boss16_poke_mort": (16, "14:11000", 0, "0x70", 1500, "boss", "16:0:3000"),
    "boss2_poke": (2, "10:11000", 0, "0x70", 0, "boss", "2:0:3000"),
    "boss2_poke3p": (2, "10:11000", 0, "0x70", 0, "boss", "2:0:3000:3"),
    "boss2_poke3p_mort": (2, "10:11000", 0, "0x70", 1500, "boss", "2:0:3000:3"),
    "boss13_dragon": (13, "7:3000,9:3000", 0, "0x70", 0, "ko"),
}


def tronquer(lines, fin):
    """Garde les entrées jusqu'à l'image `fin`, puis relâche (image fin + 1) les boutons restés enfoncés : un bouton
    maintenu à l'entrée de la scène suivante suffit à déclencher l'avance rapide."""
    gardees, enfonces = [], {}
    for l in lines:
        mots = l.split()
        if mots and mots[0].isnumeric():
            if int(mots[0]) > fin:
                continue
            if mots[1] in ("P1", "P2", "P3", "P4", "Coin", "Service") and mots[-1] in ("0", "1"):
                enfonces[" ".join(mots[1:-1])] = mots[-1]
        gardees.append(l)
    while gardees and not gardees[-1]:
        gardees.pop()
    gardees += [f"{fin + 1} {entree} 0" for entree, etat in enfonces.items() if etat == "1"]
    return "\n".join(gardees) + "\n"


def generate(name):
    _, plan, k, god, weaken, folder, *apparition = SPECS[name]
    base = os.path.join(ROOT, "work", name + ".base")
    os.makedirs(os.path.dirname(base), exist_ok=True)
    joueurs = 1
    if apparition:
        champs = apparition[0].split(":")
        joueurs = int(champs[3]) if len(champs) > 3 else 1
    lines = make_bot.build(frames=FRAMES, players=joueurs, chars=(k,) * joueurs, coin_period=1500, base_only=True)
    with open(base, "w") as f:
        f.write("\n".join(lines) + "\n")
    out = os.path.join(ROOT, "scenarios", folder, name + ".txt")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    cmd = [AUTOBOT, ROMS, EEPROM, base, str(FRAMES), out, "--players", str(joueurs), "--seed", "1", "--god", god, "--plan", plan]
    if weaken:
        cmd += ["--weaken", str(weaken)]
    if apparition:
        # objet créé à la main (type:variante:image) pour un boss que les scènes du jeu ne montrent pas
        kind, variant, image = apparition[0].split(":")[:3]
        cmd += ["--spawn", f"boss:{kind}:{variant}:{image}"]
    log = subprocess.run(cmd, capture_output=True, text=True)
    with open(os.path.join(ROOT, "work", name + ".log"), "w") as f:
        f.write(log.stderr)
    if name in TRONQUE:
        texte = tronquer(open(out).read().split("\n"), TRONQUE[name])
        with open(out, "w") as f:
            f.write(texte)
    return log.returncode


if __name__ == "__main__":
    for n in sys.argv[1:] or SPECS:
        print(n, "ok" if generate(n) == 0 else "ECHEC")
