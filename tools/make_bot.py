"""Génère un scénario « bot » pour mame/scenario.lua et la validation Rust.

    python3 tools/make_bot.py <sortie> [--frames N] [--seed S] [--char K] [--chars K1,K2,..]
                              [--players P] [--scene N] [--poke IMAGE:ADRESSE:VALEUR[:TAILLE]]...
                              [--god PV] [--item-rate R] [--coin-period N]

Le bot met une pièce, démarre, déplace le curseur de sélection de K crans vers la droite,
valide, puis avance en combattant (attaques, sauts, magie, objets, déplacements) et remet des
pièces (+ Start : « continue ») quand il meurt.

Accès direct aux scènes : `--scene N` écrit N à l'adresse FF8034 (n° de scène/stage) pendant la
sélection du personnage (image 2400, avant l'entrée en phase 8) ; `--poke` ajoute une écriture RAM
quelconque (poke8/16/32, voir port/ddsom-game/src/scenario.rs). Le script tools/gen_scenarios.py
s'appuie sur ce module pour produire le jeu complet de scénarios (couverture de tout le jeu).
"""
import argparse
import random


def build(frames=30000, seed=1, chars=(0,), players=1, scene=None, pokes=(), item_rate=1,
          coin_period=3000, poke_frame=2400, start_frame=4300, god=0, base_only=False):
    """Retourne la liste des lignes du scénario."""
    rnd = random.Random(seed)
    ev = []  # (image, nom, valeur) ; l'ordre relatif des lignes de même image est conservé
    pk = []

    def press(f, name, dur=4):
        ev.append((f, name, 1))
        ev.append((f + dur, name, 0))

    pls = range(1, players + 1)

    def coin(pl):
        return f"Coin {pl}"
    if players > 2:
        # réglage monnayeur chargé de l'EEPROM (FF8021 = 1 : deux monnayeurs) : il en faut un par joueur
        pk.append((800, "poke8 ff8021", 3))
        pk.append((800, "poke8 ff8020", 2))  # FF8020 = joueurs - 2 (1 : trois joueurs)
    for pl in pls:
        press(900 + 20 * pl, coin(pl), 6)
    press(1000 if players == 1 else 1100, "1 Player Start", 6)
    for pl in range(2, players + 1):
        # les autres joueurs rejoignent pendant l'écran de sélection (« JOIN IN »)
        press(1250 + 20 * (pl - 2), f"{pl} Players Start", 6)
    for pl in pls:
        k_char = chars[(pl - 1) % len(chars)]
        for k in range(k_char + pl - 1):
            press(1400 + 40 * k, f"P{pl} Right", 6)
        press(1400 + 40 * (k_char + pl), f"P{pl} Button 1", 6)

    if scene is not None:
        pk.append((poke_frame, "poke8 ff8034", scene))
    for spec in pokes:
        f, addr, val, *size = spec.split(":")
        pk.append((int(f), f"poke{8 * int(size[0]) if size else 8} {addr}", int(val, 0)))

    if god:
        # « god » : PV (+0x62 de l'enregistrement joueur FF85DE + 0x200 k) remis à `god` toutes les 50 images
        for g in range(start_frame - 100, frames, 50):
            for pl in pls:
                pk.append((g, f"poke16 {0xff85de + 0x200 * (pl - 1) + 0x62:x}", god))

    f = start_frame
    while f < frames:
        if base_only:
            # seulement les pièces / Start périodiques (le bot réactif d'autobot.rs fait le reste)
            if coin_period and f % coin_period < 60:
                for pl in pls:
                    press(f + 10 * pl, coin(pl), 6)
                    press(f + 30 + 10 * pl, "1 Player Start" if pl == 1 else f"{pl} Players Start", 6)
            f += 100
            continue
        if coin_period and f % coin_period < 60:
            for pl in pls:
                press(f + 10 * pl, coin(pl), 6)
                press(f + 30 + 10 * pl, "1 Player Start" if pl == 1 else f"{pl} Players Start", 6)
        for pl in pls:
            segs = ["walk", "fight", "fight", "jump", "magic", "up", "down", "back"]
            if item_rate:
                segs += ["item"] * item_rate
            seg = rnd.choice(segs)
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
            elif seg == "item":
                # Button 4 : choix de l'objet/magie, puis Button 3 : utilisation
                press(f, f"{q} Button 4", 4); press(f + 10, f"{q} Button 3", 4)
            elif seg == "up":
                ev.append((f, f"{q} Up", 1)); ev.append((f + 40, f"{q} Up", 0))
            else:
                ev.append((f, f"{q} Down", 1)); ev.append((f + 40, f"{q} Down", 0))
        f += 100

    lines = [(fr, f"{n} {v}") for fr, n, v in ev]
    lines += [(fr, f"{n} {v}") for fr, n, v in pk]
    lines.sort(key=lambda e: e[0])  # tri stable
    out = [f"{fr} {s}" for fr, s in lines]
    return out if base_only else out + [f"{frames + 1} quit"]


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("out")
    p.add_argument("--frames", type=int, default=30000)
    p.add_argument("--seed", type=int, default=1)
    p.add_argument("--char", type=int, default=0)
    p.add_argument("--chars", help="personnage de chaque joueur, ex. 0,1,2,3")
    p.add_argument("--players", type=int, default=1)
    p.add_argument("--scene", type=lambda v: int(v, 0), help="n° de scène (FF8034)")
    p.add_argument("--poke", action="append", default=[], help="IMAGE:ADRESSE_HEX:VALEUR[:OCTETS]")
    p.add_argument("--god", type=lambda v: int(v, 0), default=0, help="PV remis à cette valeur toutes les 50 images (ex. 0x70)")
    p.add_argument("--item-rate", type=int, default=1)
    p.add_argument("--coin-period", type=int, default=3000)
    p.add_argument("--base-only", action="store_true", help="entrées fixes seulement (pour autobot)")
    p.add_argument("--dir", default="Right", help="(obsolète)")
    a = p.parse_args()
    chars = tuple(int(c) for c in a.chars.split(",")) if a.chars else (a.char,)
    lines = build(a.frames, a.seed, chars, a.players, a.scene, a.poke, a.item_rate, a.coin_period, god=a.god, base_only=a.base_only)
    with open(a.out, "w") as o:
        o.write("\n".join(lines) + "\n")
