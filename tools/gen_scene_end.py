"""Génère les scénarios de sortie de scène (scenarios/scene_end/end_NN.txt), sans bot.

    python3 tools/gen_scene_end.py [numéro de scène ...]        (toutes les séquences de sortie si aucun)

Aucune touche n'est pressée après l'entrée (une touche pendant une transition fait diverger les deux machines, voir docs/couverture.md). Chaque scénario entre dans la scène N (FF8034 écrit pendant la sélection du personnage), laisse le jeu
tourner puis force la phase 16 (sortie de scène : FF802C = 0x10, étape et sous-étape à zéro) : la séquence
de sortie de la scène se déroule, puis la scène suivante commence. Le joueur est invincible. Ils exercent
`port/ddsom-game/src/scene/exit.rs`. Les scénarios ne contiennent que des entrées et des écritures RAM.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import bot_lib as make_bot  # noqa: E402

ROOT = os.path.join(os.path.dirname(__file__), "..")
FRAMES = 9000
FORCE_AT = 3600


# une scène par séquence de sortie (indices 0 à 35 de la table 0xf31ea, voir docs/scenes.md)
REPRESENTATIVES = [3, 1, 2, 4, 5, 6, 7, 9, 10, 11, 13, 14, 20, 24, 26, 27, 31, 32, 33, 34, 35, 38, 40, 41, 42, 44, 46, 54, 58,
                   59, 64, 65, 66, 67, 68, 105]


def generate(scene):
    lines = make_bot.build(frames=FRAMES, players=1, chars=(0,), scene=scene, coin_period=0, base_only=True, god=0x70)
    lines = [l for l in lines if int(l.split()[0]) < FORCE_AT]
    lines += [f"{FORCE_AT} poke8 ff802c 16", f"{FORCE_AT} poke8 ff802d 0", f"{FORCE_AT} poke16 ff802e 0"]
    # les pokes d'invincibilité continuent
    for g in range(FORCE_AT, FRAMES, 50):
        lines.append(f"{g} poke16 ff8640 112")
    lines.sort(key=lambda l: int(l.split()[0]))
    out = os.path.join(ROOT, "scenarios", "scene_end", f"end_{scene:02d}.txt")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, "w") as f:
        f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    for s in [int(a) for a in sys.argv[1:]] or REPRESENTATIVES:
        generate(s)
        print(s, "ok")
