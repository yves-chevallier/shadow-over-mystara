#!/usr/bin/env python3
"""Bissection d'une validation empoisonnée (`DDSOM_POISON`, docs/guide-refactorisation.md, « Registres morts »).

    tools/bissecte_poison.py <validate> <scénario> <images> <poison> [gardes...]

Lance le scénario avec la liste `poison` et la trace des empoisonnements (DDSOM_POISON_TRACE) ; s'il échoue, cherche
par dichotomie l'entrée « étiquette@site » responsable parmi les sites empoisonnés avant l'écart, puis le ou les
registres relus (N/Z/V/C et X séparés), l'épargne (« étiquette@site#registres ») et recommence jusqu'à VALIDE.
Imprime chaque lecteur trouvé, puis la ligne « GARDES » des entrées à ajouter. Les entrées `=…#…` de la liste de départ
(registres épargnés sans empoisonner) valent pour tous les essais.

Variables : DDSOM_ROMS (rom), DDSOM_EEPROM (testdata/eeprom_ddsom.bin) ; le scénario est relatif à la racine du dépôt.
"""
import os
import re
import subprocess
import sys

RACINE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ROM = os.environ.get("DDSOM_ROMS", os.path.join(RACINE, "rom"))
EEPROM = os.environ.get("DDSOM_EEPROM", os.path.join(RACINE, "testdata/eeprom_ddsom.bin"))
REGS = [f"d{i}" for i in range(8)] + [f"a{i}" for i in (0, 1, 2, 3, 4, 6)] + ["nzvc", "x"]


def lancer(val, scen, images, poison, trace=False):
    env = dict(os.environ)
    env["DDSOM_POISON"] = poison
    if trace:
        env["DDSOM_POISON_TRACE"] = "1"
    else:
        env.pop("DDSOM_POISON_TRACE", None)
    return subprocess.run([val, ROM, EEPROM, scen, str(images)], cwd=RACINE, env=env, capture_output=True, text=True)


def verdict(r):
    return r.stdout.strip().split("\n")[-1]


def main():
    val, scen, images, base = sys.argv[1:5]
    gardes = list(sys.argv[5:])
    gardes_base = [e for e in base.split(",") if "#" in e]
    limite = [images]

    def echoue(entrees):
        if not entrees:
            return False
        return not verdict(lancer(val, scen, limite[0], ",".join(entrees + gardes_base))).startswith("VALIDE")

    while True:
        limite[0] = images
        r = lancer(val, scen, images, ",".join([base] + gardes), trace=True)
        v = verdict(r)
        print("départ :", v, flush=True)
        if v.startswith("VALIDE"):
            break
        m = re.search(r"image (\d+)", v)
        if m:
            limite[0] = int(m.group(1)) + 1
        candidats = []
        for ligne in r.stderr.split("\n"):
            m = re.match(r"POISON (\S+) (\S+)", ligne)
            if m and f"{m.group(1)}@{m.group(2)}" not in candidats:
                candidats.append(f"{m.group(1)}@{m.group(2)}")
        print(len(candidats), "candidats", flush=True)
        if not echoue(candidats + gardes):
            print("écart non reproduit par les seuls candidats", flush=True)
            break
        bas, haut = 0, len(candidats)
        while haut - bas > 1:
            milieu = (bas + haut) // 2
            if echoue(candidats[:milieu] + gardes):
                haut = milieu
            else:
                bas = milieu
        coupable = candidats[haut - 1]
        lus = [reg for reg in REGS
               if echoue(candidats[:haut - 1] + [f"{coupable}#{'+'.join(x for x in REGS if x != reg)}"] + gardes)]
        if lus:
            print("lecteur :", coupable, "registres lus :", "+".join(lus), flush=True)
            gardes.append(f"{coupable}#{'+'.join(lus)}")
        else:
            print("lecteur :", coupable, "(combinaison de registres)", flush=True)
            gardes.append("!" + coupable)
    print("GARDES", " ".join(gardes))


if __name__ == "__main__":
    main()
