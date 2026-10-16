"""Génère un scénario « bot » pour mame/scenario.lua et la validation Rust.

    python3 tools/make_bot.py <sortie> [--frames N] [--seed S] [--char K] [--players P]

Le bot met une pièce, démarre, déplace le curseur de sélection de K crans vers la droite,
valide, puis avance en combattant (attaques, sauts, magie, déplacements) et remet des pièces.
"""
import argparse
import random

p = argparse.ArgumentParser()
p.add_argument("out")
p.add_argument("--frames", type=int, default=30000)
p.add_argument("--seed", type=int, default=1)
p.add_argument("--char", type=int, default=0)
p.add_argument("--players", type=int, default=1)
p.add_argument("--dir", default="Right", choices=["Right", "Left", "Up", "Down"], help="sens de déplacement du curseur")
a = p.parse_args()
random.seed(a.seed)

ev = []


def press(f, name, dur=4):
    ev.append((f, name, 1))
    ev.append((f + dur, name, 0))


players = range(1, a.players + 1)
for pl in players:
    press(900 + 10 * pl, f"Coin {pl}", 6)
press(1000, "1 Player Start", 6)
if a.players > 1:
    press(1010, "2 Players Start", 6)
for pl in players:
    for k in range(a.char + pl - 1):
        press(1200 + 40 * k, f"P{pl} {a.dir}", 6)
    press(1200 + 40 * (a.char + pl), f"P{pl} Button 1", 6)

f = 4300
while f < a.frames:
    if f % 3000 < 60:
        for pl in players:
            press(f + 10 * pl, f"Coin {pl}", 6)
        press(f + 30, "1 Player Start", 6)
    for pl in players:
        seg = random.choice(["walk", "fight", "fight", "jump", "magic", "up", "down", "back"])
        q = f"P{pl}"
        if seg == "walk":
            ev.append((f, f"{q} Right", 1)); ev.append((f + 90, f"{q} Right", 0))
        elif seg == "back":
            ev.append((f, f"{q} Left", 1)); ev.append((f + 40, f"{q} Left", 0))
        elif seg == "fight":
            for k in range(8):
                press(f + k * 12, f"{q} Button 1", 4)
        elif seg == "jump":
            ev.append((f, f"{q} Right", 1)); press(f + 5, f"{q} Button 2", 4); ev.append((f + 40, f"{q} Right", 0))
        elif seg == "magic":
            press(f, f"{q} Button 3", 4)
        elif seg == "up":
            ev.append((f, f"{q} Up", 1)); ev.append((f + 40, f"{q} Up", 0))
        else:
            ev.append((f, f"{q} Down", 1)); ev.append((f + 40, f"{q} Down", 0))
    f += 100

ev.sort(key=lambda e: e[0])
with open(a.out, "w") as o:
    for fr, n, v in ev:
        o.write(f"{fr} {n} {v}\n")
    o.write(f"{a.frames + 1} quit\n")
