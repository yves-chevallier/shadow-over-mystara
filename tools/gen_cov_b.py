"""Scénarios de couverture ciblés (scenarios/cov/covb_*.txt, rejoués comme les autres scénarios de cov/ sur 30 000 images) : chemins que les scénarios de couverture
généraux (gen_scenarios.py) n'atteignent pas, d'après le classement de docs/couverture.md.

    python3 tools/gen_cov_b.py [nom ...]        (tous si aucun nom)

Même mécanique que gen_scenarios.py : entrées fixes (tools/bot_lib.py) + bot réactif
(port/ddsom-game/examples/autobot.rs), qui écrit un scénario d'entrées et de `poke` pur (aucune donnée de ROM).
Réglages forcés par `poke` après le chargement de l'EEPROM (image 800) :
  * région `A5+0x70` (mot FF8070 : 0 JPN, 2 USA, 4 HIS, 6 ASI, 8 EUR (celle des ROM), 10 BRA) ;
  * langue `A5+0x72` (mot FF8072 : 0 japonais, 2, 4, 6) ;
  * difficulté `A5+0x2a` (octet FF802A, 0-7) ;
  * `offset` : l'attract tourne `offset` images de plus avant la pièce (écrans propres à la région).
Prérequis : `cargo build --release -p ddsom-game --examples`, DDSOM_ROMS / DDSOM_EEPROM.
"""
import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

sys.path.insert(0, os.path.dirname(__file__))
import bot_lib  # noqa: E402

ROOT = os.path.join(os.path.dirname(__file__), "..")
AUTOBOT = os.path.join(ROOT, "port/target/release/examples/autobot")
ROMS = os.environ.get("DDSOM_ROMS", os.path.join(ROOT, "rom"))
EEPROM = os.environ.get("DDSOM_EEPROM", os.path.join(ROOT, "testdata/eeprom_ddsom.bin"))
# scénarios en écart (routine Rust fausse sur leur chemin, docs/couverture.md) : rangés dans scenarios/ko/
KO = {"long3p_02", "long3p_27", "long3p_59"}
WORK = os.environ.get("DDSOM_WORK", os.path.join(ROOT, "work"))
FRAMES = 20000  # images enregistrées par le bot (le scénario est rejoué sur 30 000 images)
# crans du curseur de sélection pour chaque personnage seul (voir gen_scenarios.SOLO_K)
SOLO_K = {0: 0, 1: 2, 2: 5, 3: 4, 4: 1, 5: 3}

SPECS = {}


def add(name, chars=(0,), players=1, plan="", god="0x70", frames=FRAMES, weaken=450, coin_period=1500, seed=1,
        offset=0, pokes=(), cut=None, cast=0, charges=0, patience=0):
    """`pokes` : (image, adresse, valeur, octets) ajoutés à la base ; `cut` : entrées gardées jusqu'à cette image."""
    SPECS[name] = dict(chars=chars, players=players, plan=plan, god=god, frames=frames, weaken=weaken,
                       coin_period=coin_period, seed=seed, offset=offset, pokes=pokes, cut=cut, cast=cast,
                       charges=charges, patience=patience)


# 1. régions : attract propre à la région, puis une partie (stage 1 naturel, puis quelques scènes)
for r, c in ((0, 1), (2, 2), (4, 3), (6, 4), (10, 5)):
    add(f"region_{r}", chars=(c,), offset=6000, frames=24000, seed=30 + r,
        pokes=((800, "ff8070", r, 2),), plan="1:3500,24:2500,40:2500,58:2500")
# 2. difficulté (A5+0x2a) extrême : décisions d'IA et dégâts selon le rang
add("diff0", chars=(2,), seed=40, pokes=((800, "ff802a", 0, 1),), plan="1:1500,3:1500,7:1500,13:2500,17:1500,21:1500,26:2500")
add("diff7", chars=(3,), seed=41, pokes=((800, "ff802a", 7, 1),), plan="2:1500,5:1500,10:1500,16:1500,24:2500,29:1500,40:2500")
# 3. langues (A5+0x72) : dialogues, boutiques, messages de scène
add("langue4", chars=(4,), seed=42, pokes=((800, "ff8072", 4, 2),), plan="1:1500,8:1500,14:1500,38:1500,69:1200,72:1200,75:1200,78:1200,44:2000")
add("langue6", chars=(5,), seed=43, pokes=((800, "ff8072", 6, 2),), plan="2:1500,11:1500,20:1500,33:1500,70:1200,73:1200,76:1200,79:1200,46:2000")
# 4. trois et quatre joueurs mortels : continues, morts simultanées, fin de partie et classement à plusieurs
add("mortal_3p", chars=(0, 1, 2), players=3, god="0", seed=44, plan="24:99999", frames=24000)
add("mortal_4p", chars=(0, 3, 5, 4), players=4, god="0", seed=45, plan="40:99999", frames=24000, coin_period=0)
# 5. séjours longs dans les scènes dont le script (ou celui d'un contrôleur SET 2) crée tard un type jamais vu :
#    ENEMY 13/15 (5, 59, 31, 43), BOSS 3 (11), BOSS 1 (27), BOSS 16 (14), BOSS 0 (20), SET 11 (2), SET 25/26 (10)
for sc, c in ((5, 0), (59, 1), (31, 2), (43, 3), (11, 4), (27, 5), (14, 0), (20, 1), (2, 2), (10, 3)):
    add(f"long_{sc:02d}", chars=(c,), seed=50 + sc, plan=f"{sc}:99999", frames=16000)
# 6. mêmes scènes à trois joueurs, plus longtemps : les vagues tombent plus vite et les entrées réservées à trois
#    joueurs ou plus (« joueurs >= 3 » des scripts) apparaissent ; `patience` : le bot abandonne un objet hors
#    d'atteinte (sans cela il reste bloqué et la scène n'avance plus, comme dans les séjours de la section 5)
for sc, cs in ((5, (0, 1, 2)), (59, (3, 4, 5)), (14, (1, 2, 3)), (20, (4, 5, 0)), (11, (2, 3, 4)), (27, (5, 0, 1)),
               (2, (0, 2, 4)), (10, (3, 5, 1))):
    add(f"long3p_{sc:02d}", chars=cs, players=3, seed=70 + sc, plan=f"{sc}:99999", frames=29000, patience=400)
# 7. magies et objets : chaque personnage commence dans une scène tardive (niveau élevé, tous ses sorts), choisit
#    et lance sans cesse ses sorts et objets (autobot --cast), charges entretenues (--charges)
for c in (0, 4):  # les quatre autres n'apportaient presque rien (moins de 20 instructions chacun)
    add(f"magie_c{c}", chars=(c,), seed=90 + c, cast=90, charges=3,
        plan="61:3500,63:3500,65:4500,52:3000,57:3000", patience=400)
# 8. rang élevé (octet FFFF93, qui monte avec le temps de jeu) : entrées de script réservées à un rang >= 21..26
#    (SET 25 et 26 de la scène 10, contrôleur de la scène 14, ENEMY 6 et contrôleur de la 39, vagues des 43 et 112)
# le rang est recalculé à chaque entrée de scène : un scénario par scène, rang posé pendant la sélection
for sc, c in ((10, 1), (14, 2), (39, 3), (43, 4)):
    add(f"rang_{sc}", chars=(c,), seed=60 + sc, pokes=((2400, "ffff93", 0x1c, 1),), patience=400, plan=f"{sc}:99999",
        frames=12000)


def generate(name):
    sp = SPECS[name]
    n = sp["players"]
    ks = [SOLO_K[c] for c in sp["chars"]] if n == 1 else list(sp["chars"])
    pokes = [f"{f}:{a}:{v}:{s}" for f, a, v, s in sp["pokes"]]
    lines = bot_lib.build(frames=sp["frames"], players=n, chars=tuple(ks), coin_period=sp["coin_period"],
                          base_only=True, offset=sp["offset"], pokes=pokes)
    os.makedirs(WORK, exist_ok=True)
    base = os.path.join(WORK, f"{name}.base")
    with open(base, "w") as f:
        f.write("\n".join(lines) + "\n")
    out = os.path.join(ROOT, "scenarios", "ko" if name in KO else "cov", "covb_" + name + ".txt")
    cmd = [AUTOBOT, ROMS, EEPROM, base, str(sp["frames"]), out, "--players", str(n), "--seed", str(sp["seed"]),
           "--weaken", str(sp["weaken"]), "--offset", str(sp["offset"])]
    if sp["god"] != "0":
        cmd += ["--god", sp["god"]]
    if sp["plan"]:
        cmd += ["--plan", sp["plan"]]
    if sp["patience"]:
        cmd += ["--patience", str(sp["patience"])]
    if sp["cast"]:
        cmd += ["--cast", str(sp["cast"]), "--charges", str(sp["charges"])]
    log = subprocess.run(cmd, capture_output=True, text=True)
    with open(os.path.join(WORK, name + ".log"), "w") as f:
        f.write(log.stderr)
    if sp["cut"] is not None and log.returncode == 0:
        keep = [l for l in open(out).read().splitlines() if int(l.split()[0]) <= sp["cut"] or l.endswith("quit")]
        with open(out, "w") as f:
            f.write("\n".join(keep) + "\n")
    return name, log.returncode


if __name__ == "__main__":
    names = sys.argv[1:] or list(SPECS)
    with ThreadPoolExecutor(int(os.environ.get("JOBS", "2"))) as ex:
        for name, rc in ex.map(generate, names):
            print(name, "ok" if rc == 0 else f"ECHEC {rc}", flush=True)
