"""Generates the coverage scenario set (scenarios/cov/*.txt) with the reactive bot autobot.

    python3 tools/gen_scenarios.py [name ...]        (all if no name)

Prerequisites: `cargo build --release -p ddsom-game --examples` and DDSOM_ROMS / DDSOM_EEPROM
(the bot reads the machine's RAM to play; the produced scenarios only contain
inputs and `poke` RAM writes, no ROM data). See docs/couverture.md.

Scene access: `autobot --plan scene:duration,...` writes the scene number (FF8034) during
character select, then jumps from scene to scene (FF8034 ← n-1, phase FF802C ← 0x10).
"""
import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

sys.path.insert(0, os.path.dirname(__file__))
import bot_lib as make_bot  # noqa: E402

ROOT = os.path.join(os.path.dirname(__file__), "..")
AUTOBOT = os.path.join(ROOT, "port/target/release/examples/autobot")
ROMS = os.environ.get("DDSOM_ROMS", os.path.join(ROOT, "rom"))
EEPROM = os.environ.get("DDSOM_EEPROM", os.path.join(ROOT, "testdata/eeprom_ddsom.bin"))
FRAMES = 30000

# Selection cursor: number of steps to get each character alone
# (0 fighter, 1 cleric, 2 elf, 3 dwarf, 4 magic-user, 5 thief: order of the game's tables).
SOLO_K = {0: 0, 1: 2, 2: 5, 3: 4, 4: 1, 5: 3}
# Multiplayer combinations giving distinct characters (calibrated on the game).
MULTI = {
    "2a": ((0, 1), [0, 3]), "2b": ((1, 2), [4, 2]), "2c": ((2, 3), [1, 0]),
    "2d": ((3, 4), [5, 4]), "2e": ((4, 5), [3, 1]), "2f": ((5, 0), [2, 5]),
    "4a": ((0, 1, 2, 3), [0, 3, 5, 4]), "4b": ((2, 3, 4, 5), [1, 0, 2, 5]), "4c": ((4, 5, 0, 1), [3, 1, 4, 2]),
}

# Scenes (FF8034): 1-114 valid; 69-82 shops; 83-89 screens with uninitialized graphics (direct entry).
NORMAL, SHORT = 350, 200


def plan(first, last, dur=NORMAL):
    out = []
    for s in range(first, last + 1):
        out.append(f"{s}:{SHORT if 83 <= s <= 89 else dur}")
    return ",".join(out)


# name -> (players/characters, plan, god, frames, options)
SPECS = {}


def add(name, players, plan_s="", god="0x70", frames=FRAMES, weaken=450, coin_period=1500, seed=1):
    SPECS[name] = dict(players=players, plan=plan_s, god=god, frames=frames, weaken=weaken,
                       coin_period=coin_period, seed=seed)


# 1. tour of the 114 scenes in 4 chunks, the 6 characters alone
CHUNKS = [(1, 28), (29, 56), (57, 86), (87, 114)]
TOURS = [(0, 0), (1, 1), (2, 2), (3, 3), (4, 0), (5, 1), (0, 2), (1, 3)]  # (character, chunk)
for i, (c, t) in enumerate(TOURS):
    a, b = CHUNKS[t]
    add(f"cov_tour{i + 1}_c{c}", ("solo", c), plan(a, b), seed=i + 1)
# 2. several players on other chunks
add("cov_2p_a", ("multi", "2a"), plan(*CHUNKS[1]), seed=11)
add("cov_2p_b", ("multi", "2e"), plan(*CHUNKS[0]), seed=12)
add("cov_2p_c", ("multi", "2d"), plan(*CHUNKS[3]), seed=13)
add("cov_3p", ("multi", "4a3"), plan(*CHUNKS[2]), seed=14)
add("cov_4p_a", ("multi", "4a"), plan(*CHUNKS[0]), seed=15)
add("cov_4p_b", ("multi", "4b"), plan(*CHUNKS[2]), seed=16)
add("cov_4p_c", ("multi", "4c"), plan(*CHUNKS[1]), seed=17)
# 3. natural games, without scene jumps: full game start, mortal (continue / game over)
add("cov_natural_c0", ("solo", 0), "", seed=21)
add("cov_mortal_c1", ("solo", 1), "1:99999", god="0", seed=22)
add("cov_mortal_late", ("solo", 3), "40:99999", god="0", seed=23)
add("cov_gameover_c4", ("solo", 4), "24:99999", god="0", coin_period=0, seed=24)
add("cov_mortal_2p", ("multi", "2c"), "60:99999", god="0", seed=25)


def gen_script(name):
    """Scenarios without bot: attract mode alone, TEST menu (Service Mode) browsed at random."""
    import random
    rnd = random.Random(7)
    lines = []

    def press(f, n, d=4):
        lines.append((f, f"{n} 1"))
        lines.append((f + d, f"{n} 0"))

    if name == "cov_service":
        lines.append((600, "Service Mode 1"))  # stays held: the game enters the TEST menu
        f = 900
        while f < FRAMES:
            n = rnd.choice(["P1 Down", "P1 Down", "P1 Up", "P1 Button 1", "P1 Button 1", "P1 Button 2", "P1 Right", "P1 Left", "P2 Button 1", "Service 1"])
            press(f, n, rnd.choice([4, 4, 20]))
            f += rnd.choice([40, 60, 120])
    elif name == "cov_attract_coin":
        # attract mode up to 20000 frames, then a short game without bot (coin, Start, default selection)
        press(20000, "Coin 1", 6)
        press(20100, "1 Player Start", 6)
        for k in range(40):
            press(21000 + 100 * k, "P1 Button 1", 4)
    lines.sort(key=lambda e: e[0])
    out = os.path.join(ROOT, "scenarios", "cov", name + ".txt")
    with open(out, "w") as fh:
        fh.write("".join(f"{f} {t}\n" for f, t in lines) + f"{FRAMES + 1} quit\n")
    return name, 0


SCRIPTS = ["cov_attract_coin", "cov_service"]


def players_args(spec):
    kind, v = spec
    if kind == "solo":
        return 1, [SOLO_K[v]]
    if v == "4a3":
        return 3, [0, 1, 2]
    ks, _ = MULTI[v]
    return len(ks), list(ks)


def generate(name):
    sp = SPECS[name]
    n, ks = players_args(sp["players"])
    base = os.path.join(ROOT, "work", f"{name}.base")
    os.makedirs(os.path.dirname(base), exist_ok=True)
    lines = make_bot.build(frames=sp["frames"], players=n, chars=tuple(ks), coin_period=sp["coin_period"], base_only=True)
    with open(base, "w") as f:
        f.write("\n".join(lines) + "\n")
    out = os.path.join(ROOT, "scenarios", "cov", name + ".txt")
    cmd = [AUTOBOT, ROMS, EEPROM, base, str(sp["frames"]), out, "--players", str(n), "--seed", str(sp["seed"]),
           "--weaken", str(sp["weaken"])]
    if sp["god"] != "0":
        cmd += ["--god", sp["god"]]
    if sp["plan"]:
        cmd += ["--plan", sp["plan"]]
    log = subprocess.run(cmd, capture_output=True, text=True)
    with open(os.path.join(ROOT, "work", name + ".log"), "w") as f:
        f.write(log.stderr)
    return name, log.returncode


if __name__ == "__main__":
    names = sys.argv[1:] or list(SPECS) + SCRIPTS
    with ThreadPoolExecutor(int(os.environ.get("JOBS", "8"))) as ex:
        for name, rc in ex.map(lambda n: gen_script(n) if n in SCRIPTS else generate(n), names):
            print(name, "ok" if rc == 0 else f"FAILED {rc}")
