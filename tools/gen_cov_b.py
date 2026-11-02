"""Targeted coverage scenarios (scenarios/cov/covb_*.txt, replayed like the other cov/ scenarios over 30,000 frames): paths that the general
coverage scenarios (gen_scenarios.py) do not reach, according to the ranking in docs/couverture.md.

    python3 tools/gen_cov_b.py [name ...]        (all if no name)

Same mechanics as gen_scenarios.py: fixed inputs (tools/bot_lib.py) + reactive bot
(port/ddsom-game/examples/autobot.rs), which writes a pure input and `poke` scenario (no ROM data).
Settings forced by `poke` after the EEPROM is loaded (frame 800):
  * region `A5+0x70` (word FF8070: 0 JPN, 2 USA, 4 HIS, 6 ASI, 8 EUR (the ROMs' one), 10 BRA);
  * language `A5+0x72` (word FF8072: 0 Japanese, 2, 4, 6);
  * difficulty `A5+0x2a` (byte FF802A, 0-7);
  * `offset`: the attract mode runs `offset` more frames before the coin (region-specific screens).
Prerequisites: `cargo build --release -p ddsom-game --examples`, DDSOM_ROMS / DDSOM_EEPROM.
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
# mismatching scenarios (wrong Rust routine on their path, docs/couverture.md): stored in scenarios/ko/
KO = {"long3p_02", "long3p_27", "long3p_59"}
WORK = os.environ.get("DDSOM_WORK", os.path.join(ROOT, "work"))
FRAMES = 20000  # frames recorded by the bot (the scenario is replayed over 30,000 frames)
# selection cursor steps for each character alone (see gen_scenarios.SOLO_K)
SOLO_K = {0: 0, 1: 2, 2: 5, 3: 4, 4: 1, 5: 3}

SPECS = {}


def add(name, chars=(0,), players=1, plan="", god="0x70", frames=FRAMES, weaken=450, coin_period=1500, seed=1,
        offset=0, pokes=(), cut=None, cast=0, charges=0, patience=0):
    """`pokes`: (frame, address, value, bytes) added to the base; `cut`: inputs kept up to this frame."""
    SPECS[name] = dict(chars=chars, players=players, plan=plan, god=god, frames=frames, weaken=weaken,
                       coin_period=coin_period, seed=seed, offset=offset, pokes=pokes, cut=cut, cast=cast,
                       charges=charges, patience=patience)


# 1. regions: region-specific attract mode, then a game (natural stage 1, then a few scenes)
for r, c in ((0, 1), (2, 2), (4, 3), (6, 4), (10, 5)):
    add(f"region_{r}", chars=(c,), offset=6000, frames=24000, seed=30 + r,
        pokes=((800, "ff8070", r, 2),), plan="1:3500,24:2500,40:2500,58:2500")
# 2. extreme difficulty (A5+0x2a): AI decisions and damage depending on the rank
add("diff0", chars=(2,), seed=40, pokes=((800, "ff802a", 0, 1),), plan="1:1500,3:1500,7:1500,13:2500,17:1500,21:1500,26:2500")
add("diff7", chars=(3,), seed=41, pokes=((800, "ff802a", 7, 1),), plan="2:1500,5:1500,10:1500,16:1500,24:2500,29:1500,40:2500")
# 3. languages (A5+0x72): dialogues, shops, scene messages
add("langue4", chars=(4,), seed=42, pokes=((800, "ff8072", 4, 2),), plan="1:1500,8:1500,14:1500,38:1500,69:1200,72:1200,75:1200,78:1200,44:2000")
add("langue6", chars=(5,), seed=43, pokes=((800, "ff8072", 6, 2),), plan="2:1500,11:1500,20:1500,33:1500,70:1200,73:1200,76:1200,79:1200,46:2000")
# 4. three and four mortal players: continues, simultaneous deaths, multiplayer game over and ranking
add("mortal_3p", chars=(0, 1, 2), players=3, god="0", seed=44, plan="24:99999", frames=24000)
add("mortal_4p", chars=(0, 3, 5, 4), players=4, god="0", seed=45, plan="40:99999", frames=24000, coin_period=0)
# 5. long stays in the scenes whose script (or that of a SET 2 controller) creates a never-seen type late:
#    ENEMY 13/15 (5, 59, 31, 43), BOSS 3 (11), BOSS 1 (27), BOSS 16 (14), BOSS 0 (20), SET 11 (2), SET 25/26 (10)
for sc, c in ((5, 0), (59, 1), (31, 2), (43, 3), (11, 4), (27, 5), (14, 0), (20, 1), (2, 2), (10, 3)):
    add(f"long_{sc:02d}", chars=(c,), seed=50 + sc, plan=f"{sc}:99999", frames=16000)
# 6. same scenes with three players, for longer: the waves fall faster and the entries reserved for three
#    players or more ("players >= 3" in the scripts) appear; `patience`: the bot gives up an object out of
#    reach (otherwise it stays stuck and the scene no longer moves on, as in the stays of section 5)
for sc, cs in ((5, (0, 1, 2)), (59, (3, 4, 5)), (14, (1, 2, 3)), (20, (4, 5, 0)), (11, (2, 3, 4)), (27, (5, 0, 1)),
               (2, (0, 2, 4)), (10, (3, 5, 1))):
    add(f"long3p_{sc:02d}", chars=cs, players=3, seed=70 + sc, plan=f"{sc}:99999", frames=29000, patience=400)
# 7. magic and items: each character starts in a late scene (high level, all their spells), keeps selecting
#    and casting their spells and items (autobot --cast), charges kept up (--charges)
for c in (0, 4):  # the other four brought almost nothing (fewer than 20 instructions each)
    add(f"magie_c{c}", chars=(c,), seed=90 + c, cast=90, charges=3,
        plan="61:3500,63:3500,65:4500,52:3000,57:3000", patience=400)
# 8. high rank (byte FFFF93, which rises with play time): script entries reserved for a rank >= 21..26
#    (SET 25 and 26 of scene 10, controller of scene 14, ENEMY 6 and controller of 39, waves of 43 and 112)
# the rank is recomputed at each scene entry: one scenario per scene, rank set during character select
for sc, c in ((10, 1), (14, 2), (39, 3), (43, 4)):
    add(f"rang_{sc}", chars=(c,), seed=60 + sc, pokes=((2400, "ffff93", 0x1c, 1),), patience=400, plan=f"{sc}:99999",
        frames=12000)
# 9. magic-user wands (weapon B 0x24..0x27, game treasures: a shot on each jumping hit, 0x46044): player 1's
#    weapon (FF85DE + 0x10e) and reserve flag (+0x18b..+0x191) set by poke, one wand per slice
#    of 3,000 frames; the last one runs out (flag reset to zero) and becomes a staff again
WANDS = []
for k, (w, flag) in enumerate(((0x24, 0x18c), (0x25, 0x18b), (0x26, 0x18d), (0x27, 0x191))):
    f = 4500 + 3000 * k
    WANDS += [(f, "ff86ec", w, 1), (f, f"{0xff85de + flag:x}", 1, 1)]
add("baguettes_c4", chars=(4,), seed=99, pokes=tuple(WANDS), patience=400, plan="21:5000,29:5000,36:5000,49:5000")


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
            print(name, "ok" if rc == 0 else f"FAILED {rc}", flush=True)
