use super::*;


// --- variant 18: choice of end-of-level bonuses ----------------------------------------------------------------

/// Variant 18 (0xcfc56): chooses, player after player, a bonus among candidates (tests of
/// table 0xcff10, depending on the character and the turn), shows the reward (creation of a SET by
/// 0xc8312), animates the player's palette and starts over until all players are served (`+0x25`).
pub fn bonus_selection<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            x.set32(0x20, 0xff00_0000);
            x.set8(0x24, 0);
            let mut n = 0;
            for p in players() {
                if present(x, p) {
                    n += 1;
                }
            }
            x.set8(0x25, n);
        }
        2 => bonus_step(x),
        _ => {}
    }
}

const REWARD_PALETTE_ROM: u32 = 0xcfdd2;

fn bonus_step<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x5) {
        0 => {
            let Some(o) = crate::families::creators::scene_set(x.b, 0x94) else { return };
            next_substate(x);
            x.set16(0x46, o.0 as u16);
            o.set_u16(x.b, 0x46, 0);
            for p in players() {
                if present(x, p) && rd8(x.b, p + 0x18a) != 0 {
                    o.set_u16(x.b, 0x46, p as u16);
                    return;
                }
            }
        }
        2 | 0xa => {
            let reward = adr16(x.u16(0x46));
            if rd8(x.b, reward + 0x22) == 0 {
                return;
            }
            next_substate(x);
            for i in 0..8 {
                wr32(x.b, 0x912224 + 0x80 * i, 0x20_0000);
            }
        }
        4 => {
            let index = x.u8(0x24) as u16;
            x.add8(0x24, 1);
            let mut a1 = PLAYERS.wrapping_add(adr16(index.rotate_right(7)));
            while !present(x, a1) {
                x.add8(0x24, 1);
                a1 = a1.wrapping_add(PLAYER_STRIDE);
            }
            next_substate(x);
            x.set16(0x26, a1 as u16);
            x.set8(0x2b, 8);
            x.set8(0x2a, 4);
        }
        6 => palette_pulse(x, 0x1000_1000),
        0xc => palette_pulse(x, 0xefff_f000),
        8 => choose_reward(x),
        0xe => {
            x.add8(0x25, 0xff);
            if x.u8(0x25) == 0 {
                next_substate(x);
            } else {
                x.set8(0x5, 4);
            }
        }
        0x10 => {
            next_state(x);
            let v = x.a5_u8(0x2d).wrapping_add(2);
            x.set_a5_u8(0x2d, v);
            x.set_a5_u16(0x2e, 0);
        }
        _ => {}
    }
}

/// Varies eight longs of the current player's palette (`+0x26`) every 4 frames, 8 times.
fn palette_pulse<R: Bench>(x: &mut Cx<'_, R>, delta: u32) {
    x.add8(0x2a, 0xff);
    if x.u8(0x2a) != 0 {
        return;
    }
    x.set8(0x2a, 4);
    let player = adr16(x.u16(0x26));
    let character = rd8(x.b, player + 3) as u32;
    let offset = adr16(x.rom16(REWARD_PALETTE_ROM + 2 * character));
    let base = 0x91c000u32.wrapping_add(offset);
    for i in 0..8 {
        let v = rd32(x.b, base + 4 * i).wrapping_add(delta);
        wr32(x.b, base + 4 * i, v);
    }
    x.add8(0x2b, 0xff);
    if x.u8(0x2b) == 0 {
        x.set8(0x2b, 8);
        next_substate(x);
    }
}

/// A candidate is discarded (true) according to the test that table 0xcff10 associates with the pair (character,
/// rank): the lists of objects already owned, the room in the inventory (+0x18a, +0x114...).
fn reward_blocked<R: Bench>(x: &mut Cx<'_, R>, player: u32, rank: u32) -> bool {
    const TESTS: u32 = 0xcff10;
    let character = (rd8(x.b, player + 3) & !1) as u32;
    let index = (character << 2) + 2 * rank;
    let target = TESTS + x.rom16(TESTS + index) as u32;
    // lists of objects (bytes, end = negative) compared with a byte of the player
    let listed = |x: &mut Cx<'_, R>, list: u32, value: u8| {
        let mut p = list;
        loop {
            let b = x.data8(p);
            p += 1;
            if b as i8 <= -1 && b & 0x80 != 0 {
                return false;
            }
            if b == value {
                return true;
            }
        }
    };
    match target {
        0xcff40 | 0xcff7c => {
            let (a, b) = if target == 0xcff40 { (0xcffb8, 0xcffbe) } else { (0xcffc4, 0xcffca) };
            let (v1, v2) = (rd8(x.b, player + 0x10d), rd8(x.b, player + 0x10e));
            if listed(x, a, v1) || listed(x, a, v2) {
                return false;
            }
            for i in 0..6 {
                let v = rd8(x.b, player + 0x140 + i);
                if listed(x, b, v) {
                    return false;
                }
            }
            true
        }
        0xcfff6 => rd8(x.b, player + 0x18a) == 0,
        0xd0006 => (rd32(x.b, player + 0x114) as i32) < 0x3000,
        0xd0016 => x.a5_u8(0x5d) == 0,
        0xcffdc => !(0..0x66).any(|i| rd8(x.b, 0xff2478 + i) == 0),
        _ => false,
    }
}

fn choose_reward<R: Bench>(x: &mut Cx<'_, R>) {
    const REWARD_IDS: u32 = 0xcfe98;
    const REWARD_ICONS: u32 = 0xcfec8;
    let player = adr16(x.u16(0x26));
    let character = rd8(x.b, player + 3) as u32;
    let mut taken = x.u32(0x20);
    let mut candidate = character & !1;
    candidate = (candidate << 1) & 0xffff;
    let mut rank = 0u32;
    loop {
        let blocked = taken >> (candidate & 31) & 1 != 0 || reward_blocked(x, player, rank);
        if !blocked {
            break;
        }
        candidate += 1;
        rank += 1;
    }
    taken |= 1 << (candidate & 31);
    taken &= 0xff77_7777;
    x.set32(0x20, taken);
    let id = x.rom16(REWARD_IDS + 2 * candidate);
    let Some(o) = crate::families::creators::scene_set(x.b, id as u8) else { return };
    next_substate(x);
    x.set16(0x46, o.0 as u16);
    let chosen = x.u16(0x26);
    o.set_u16(x.b, 0x46, chosen);
    let low = candidate & 3;
    let icons = REWARD_ICONS.wrapping_add((candidate & 0x1c) << 1);
    let mut at = 0x912224;
    for i in 0..7 {
        let tile = x.data8(icons + i) as u16;
        wr16(x.b, at, tile);
        wr16(x.b, at + 2, 0);
        at += 0x80;
    }
    wr16(x.b, at, (low + 1) as u16);
    wr16(x.b, at + 2, 0);
    for i in 0..8 {
        let v = rd32(x.b, 0x91c400 + 4 * i) & 0x0fff_0fff;
        wr32(x.b, 0x91c400 + 4 * i, v);
    }
}

// --- variant 19: countdown in digits ---------------------------------------------------------------------------------

/// Variant 19 (0xd004a): the object carries a BCD number in `+0x4c` (4 digits of 4 bits in
/// bytes 0x100, 0x1000, 0x10000, 0x100000); each frame removes one unit from the lowest
/// non-zero digit and launches event 0x19ae6 (sound/score) for that digit; at zero, it releases itself.
pub fn countdown_digits<R: Bench>(x: &mut Cx<'_, R>) {
    let mut value = x.u32(0x4c);
    if value == 0 {
        return x.free(&pools::SET);
    }
    let (sub, event) = if value & 0xf00 != 0 {
        (0x100, 4)
    } else if value & 0xf000 != 0 {
        (0x1000, 0xe)
    } else if value & 0xf_0000 != 0 {
        (0x1_0000, 0x21)
    } else {
        (0x10_0000, 0x28)
    };
    value = value.wrapping_sub(sub);
    x.set32(0x4c, value);
    let event_word = event | (x.u8(0x2f) as u32) << 8;
    crate::events::enqueue_points(x.b, event_word as u16);
}

