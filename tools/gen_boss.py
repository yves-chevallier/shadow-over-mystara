"""Generates the boss validation scenarios (scenarios/boss/*.txt) with the reactive bot autobot.

    python3 tools/gen_boss.py [name ...]        (all if no name)

Each scenario enters the boss scene directly (FF8034 written during character select)
and the bot fights it up to its last phases; without `weaken`, the boss HP are not touched.
Prerequisites: `cargo build --release -p ddsom-game --examples`, DDSOM_ROMS / DDSOM_EEPROM. The produced scenarios
only contain inputs and RAM writes (`poke`). See docs/familles/boss.md.
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
# truncated scenarios: after the boss dies, a button pressed or held during the end of the scene triggers the fast
# forward ("step storm", see docs/couverture.md), whose number of steps depends on the duration of the native routines;
# the inputs are kept up to the given frame, before the bot touches the buttons at that point
TRUNCATED = {
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

# name -> (boss type, scenes "scene:frames,...", cursor steps, player HP reset to, weakening, folder)
# optional 7th element "type:variant:frame[:players]": boss spawned by hand (autobot --spawn) at that frame, for a type that
# the game's scenes do not bring in within the played durations
# folder "ko": the scenario diverges because of another routine (see scenarios/ko/README.md)
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


def truncate(lines, end):
    """Keeps the inputs up to frame `end`, then releases (frame end + 1) the buttons still held: a button
    held when the next scene starts is enough to trigger the fast forward."""
    kept, held = [], {}
    for l in lines:
        words = l.split()
        if words and words[0].isnumeric():
            if int(words[0]) > end:
                continue
            if words[1] in ("P1", "P2", "P3", "P4", "Coin", "Service") and words[-1] in ("0", "1"):
                held[" ".join(words[1:-1])] = words[-1]
        kept.append(l)
    while kept and not kept[-1]:
        kept.pop()
    kept += [f"{end + 1} {inp} 0" for inp, state in held.items() if state == "1"]
    return "\n".join(kept) + "\n"


def generate(name):
    _, plan, k, god, weaken, folder, *spawn = SPECS[name]
    base = os.path.join(ROOT, "work", name + ".base")
    os.makedirs(os.path.dirname(base), exist_ok=True)
    players = 1
    if spawn:
        fields = spawn[0].split(":")
        players = int(fields[3]) if len(fields) > 3 else 1
    lines = make_bot.build(frames=FRAMES, players=players, chars=(k,) * players, coin_period=1500, base_only=True)
    with open(base, "w") as f:
        f.write("\n".join(lines) + "\n")
    out = os.path.join(ROOT, "scenarios", folder, name + ".txt")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    cmd = [AUTOBOT, ROMS, EEPROM, base, str(FRAMES), out, "--players", str(players), "--seed", "1", "--god", god, "--plan", plan]
    if weaken:
        cmd += ["--weaken", str(weaken)]
    if spawn:
        # object spawned by hand (type:variant:frame) for a boss that the game's scenes do not show
        kind, variant, frame = spawn[0].split(":")[:3]
        cmd += ["--spawn", f"boss:{kind}:{variant}:{frame}"]
    log = subprocess.run(cmd, capture_output=True, text=True)
    with open(os.path.join(ROOT, "work", name + ".log"), "w") as f:
        f.write(log.stderr)
    if name in TRUNCATED:
        text = truncate(open(out).read().split("\n"), TRUNCATED[name])
        with open(out, "w") as f:
            f.write(text)
    return log.returncode


if __name__ == "__main__":
    for n in sys.argv[1:] or SPECS:
        print(n, "ok" if generate(n) == 0 else "FAILED")
