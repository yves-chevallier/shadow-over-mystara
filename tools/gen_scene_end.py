"""Generates the scene exit scenarios (scenarios/scene_end/end_NN.txt), without a bot.

    python3 tools/gen_scene_end.py [scene number ...]        (all exit sequences if none)
    python3 tools/gen_scene_end.py --transitions              (transition scripts: trans_NN.txt)

No key is pressed after the entry (a key during a transition makes the two machines diverge, see docs/couverture.md). Each scenario enters scene N (FF8034 written during character select), lets the game
run, then forces phase 16 (scene exit: FF802C = 0x10, step and sub-step at zero): the scene's exit
sequence plays, then the next scene starts. The player is invincible. They exercise
`port/ddsom-game/src/scene/exit.rs`. The scenarios only contain inputs and RAM writes.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import bot_lib as make_bot  # noqa: E402

ROOT = os.path.join(os.path.dirname(__file__), "..")
FRAMES = 9000
FORCE_AT = 3600


# one scene per exit sequence (indices 0 to 35 of table 0xf31ea, see docs/scenes.md)
REPRESENTATIVES = [3, 1, 2, 4, 5, 6, 7, 9, 10, 11, 13, 14, 20, 24, 26, 27, 31, 32, 33, 34, 35, 38, 40, 41, 42, 44, 46, 54, 58,
                   59, 64, 65, 66, 67, 68, 105]


def generate(scene):
    lines = make_bot.build(frames=FRAMES, players=1, chars=(0,), scene=scene, coin_period=0, base_only=True, god=0x70)
    lines = [l for l in lines if int(l.split()[0]) < FORCE_AT]
    lines += [f"{FORCE_AT} poke8 ff802c 16", f"{FORCE_AT} poke8 ff802d 0", f"{FORCE_AT} poke16 ff802e 0"]
    # the invincibility pokes go on
    for g in range(FORCE_AT, FRAMES, 50):
        lines.append(f"{g} poke16 ff8640 112")
    lines.sort(key=lambda l: int(l.split()[0]))
    out = os.path.join(ROOT, "scenarios", "scene_end", f"end_{scene:02d}.txt")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, "w") as f:
        f.write("\n".join(lines) + "\n")


# transition scripts (table 0xf81fa, docs/scenes.md §5) that forced exits do not reach: the sequences of these
# scenes wait for a boss death or a timer never armed. The exit is forced directly into the fade-to-black state
# (`0xf7ecc`) of sequence 18 (CLEAR, WALK, TALLY, BONUS, CHARACTERS, BLACKOUT, departure): block A5-0x55ca =
# state 2, sequence 0x24, step 20; script variables (A5-0x5520) at zero. The scene's script plays, then the departure.
# (scene, forced `A5+0x38` clock or None, suffix): scripts 11 and 13 have two branches depending on the time left.
TRANSITIONS = [(2, None, ""), (4, None, ""), (5, None, ""), (10, None, ""), (11, 0x0300, ""), (11, 0, "_tard"),
               (13, None, ""), (13, 0, "_tard"), (14, None, ""), (20, None, ""), (32, None, ""), (33, None, ""),
               (34, None, ""), (35, None, ""), (38, None, ""), (40, None, ""), (42, None, ""), (44, None, ""),
               (46, None, ""), (67, None, "")]


def generate_transition(scene, clock=None, suffix=""):
    lines = make_bot.build(frames=FRAMES, players=1, chars=(0,), scene=scene, coin_period=0, base_only=True, god=0x70)
    lines = [l for l in lines if int(l.split()[0]) < FORCE_AT]
    lines += [f"{FORCE_AT} poke8 ff802c 16", f"{FORCE_AT} poke8 ff802d 0", f"{FORCE_AT} poke16 ff802e 0",
              f"{FORCE_AT} poke32 ff2a36 0x02241400"]
    lines += [f"{FORCE_AT} poke32 ff{0x2ae0 + 4 * k:04x} 0" for k in range(4)]
    if clock is not None:
        lines.append(f"{FORCE_AT} poke16 ff8038 {clock:#x}")
    for g in range(FORCE_AT, FRAMES, 50):
        lines.append(f"{g} poke16 ff8640 112")
    lines.sort(key=lambda l: int(l.split()[0]))
    name = f"trans_{scene:02d}{suffix}.txt"
    with open(os.path.join(ROOT, "scenarios", "scene_end", name), "w") as f:
        f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    if sys.argv[1:] == ["--transitions"]:
        for t in TRANSITIONS:
            generate_transition(*t)
        print("ok")
    else:
        for s in [int(a) for a in sys.argv[1:]] or REPRESENTATIVES:
            generate(s)
            print(s, "ok")
