"""Generates a "bot" scenario for mame/scenario.lua and the Rust validation.

    python3 tools/bot_lib.py <output> [--frames N] [--seed S] [--char K] [--chars K1,K2,..]
                              [--players P] [--scene N] [--poke FRAME:ADDRESS:VALUE[:SIZE]]...
                              [--god HP] [--item-rate R] [--coin-period N]

The bot inserts a coin, starts, moves the selection cursor K steps to the right,
confirms, then moves on fighting (attacks, jumps, magic, items, moves) and inserts more
coins (+ Start: "continue") when it dies.

Direct scene access: `--scene N` writes N at address FF8034 (scene/stage number) during
character select (frame 2400, before entering phase 8); `--poke` adds any RAM write
(poke8/16/32, see port/ddsom-game/src/scenario.rs). The script tools/gen_scenarios.py
relies on this module to produce the full set of scenarios (coverage of the whole game).
"""
import argparse
import random


def build(frames=30000, seed=1, chars=(0,), players=1, scene=None, pokes=(), item_rate=1,
          coin_period=3000, poke_frame=2400, start_frame=4300, god=0, base_only=False, offset=0, late=()):
    """Returns the list of scenario lines. `offset`: coins, start and selection shifted by that many
    frames (the attract mode runs first; autobot --offset), the `pokes` stay at their frame."""
    rnd = random.Random(seed)
    ev = []  # (frame, name, value); the relative order of lines with the same frame is kept
    pk = []
    poke_frame += offset
    start_frame += offset

    def press(f, name, dur=4):
        if f < 2000 + offset:
            f += offset
        ev.append((f, name, 1))
        ev.append((f + dur, name, 0))

    pls = range(1, players + 1)
    # `late`: (player, frame, keys): the player joins during the game (coin then Start) and moves their selection
    # cursor; keys = (name, offset, duration) from the coin frame
    latecomers = {pl for pl, _, _ in late}
    early = [pl for pl in pls if pl not in latecomers]

    def coin(pl):
        return f"Coin {pl}"
    if players > 2:
        # coin slot setting loaded from the EEPROM (FF8021 = 1: two coin slots): one per player is needed
        pk.append((800 + offset, "poke8 ff8021", 3))
        pk.append((800 + offset, "poke8 ff8020", 2))  # FF8020 = players - 2 (1: three players)
    for pl in early:
        press(900 + 20 * pl, coin(pl), 6)
    press(1000 if players == 1 else 1100, "1 Player Start", 6)
    for pl in range(2, players + 1):
        if pl in latecomers:
            continue
        # the other players join during the selection screen ("JOIN IN")
        press(1250 + 20 * (pl - 2), f"{pl} Players Start", 6)
    for pl, f0, keys in late:
        press(f0, coin(pl), 6)
        press(f0 + 30, f"{pl} Players Start", 6)
        for name, dt, dur in keys:
            ev.append((f0 + dt, f"P{pl} {name}", 1))
            ev.append((f0 + dt + dur, f"P{pl} {name}", 0))
    for pl in early:
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
        # "god": HP (+0x62 of the player record FF85DE + 0x200 k) reset to `god` every 50 frames
        for g in range(start_frame - 100, frames, 50):
            for pl in pls:
                pk.append((g, f"poke16 {0xff85de + 0x200 * (pl - 1) + 0x62:x}", god))

    f = start_frame
    while f < frames:
        if base_only:
            # only the periodic coins / Start (the reactive bot of autobot.rs does the rest)
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
                # Button 4: item/magic choice, then Button 3: use
                press(f, f"{q} Button 4", 4); press(f + 10, f"{q} Button 3", 4)
            elif seg == "up":
                ev.append((f, f"{q} Up", 1)); ev.append((f + 40, f"{q} Up", 0))
            else:
                ev.append((f, f"{q} Down", 1)); ev.append((f + 40, f"{q} Down", 0))
        f += 100

    lines = [(fr, f"{n} {v}") for fr, n, v in ev]
    lines += [(fr, f"{n} {v}") for fr, n, v in pk]
    lines.sort(key=lambda e: e[0])  # stable sort
    out = [f"{fr} {s}" for fr, s in lines]
    return out if base_only else out + [f"{frames + 1} quit"]


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("out")
    p.add_argument("--frames", type=int, default=30000)
    p.add_argument("--seed", type=int, default=1)
    p.add_argument("--char", type=int, default=0)
    p.add_argument("--chars", help="character of each player, e.g. 0,1,2,3")
    p.add_argument("--players", type=int, default=1)
    p.add_argument("--scene", type=lambda v: int(v, 0), help="scene number (FF8034)")
    p.add_argument("--poke", action="append", default=[], help="FRAME:HEX_ADDRESS:VALUE[:BYTES]")
    p.add_argument("--god", type=lambda v: int(v, 0), default=0, help="HP reset to this value every 50 frames (e.g. 0x70)")
    p.add_argument("--item-rate", type=int, default=1)
    p.add_argument("--coin-period", type=int, default=3000)
    p.add_argument("--base-only", action="store_true", help="fixed inputs only (for autobot)")
    p.add_argument("--dir", default="Right", help="(obsolete)")
    a = p.parse_args()
    chars = tuple(int(c) for c in a.chars.split(",")) if a.chars else (a.char,)
    lines = build(a.frames, a.seed, chars, a.players, a.scene, a.poke, a.item_rate, a.coin_period, god=a.god, base_only=a.base_only)
    with open(a.out, "w") as o:
        o.write("\n".join(lines) + "\n")
