use super::*;

const GATE_LOCK: u32 = 0x52;
/// Sound script played when leaving the zone.
const LEAVE_SCRIPT: u32 = 0xd6f0e;
const EFFECT_TYPE: u8 = 0x2f;
const ENTERED: u32 = 0x0200_2006;
const WAITING_AT_DOOR: u32 = 0x0200_2010;
const WAIT_FRAMES: i8 = 8;
const DOOR_X: i16 = 0x1e0;
const LAST_STAGE: u8 = 0x1f;
const BONUS_STAGE: u8 = 0x1c;
const END_STAGE: u8 = 0x20;
const BONUS_STAGES: u8 = 4;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let me = x.o;
    crate::effects_aids::living_players(x.b, me);
    let lock = x.a5_u16(GATE_LOCK);
    if lock != 0 && lock != x.o.0 as u16 {
        return;
    }
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            for off in [0x42, 0x43, 0x44, 0x45, 0x16, 0x17] {
                x.set8(off, 0);
            }
            let door_y = if x.u8(0x3) == 0 { 0x160 } else { 0x114 };
            x.set16(0x10, door_y);
        }
        2 => watch(x),
        4 => leave(x),
        _ => x.free(&pools::SET),
    }
}

/// Is a player in a position to enter? Counts the consecutive frames (byte `counter`).
fn at_door<R: Bench>(x: &mut Cx<'_, R>, p: Obj, input: u8, counter: u32) {
    let busy = p.u8(x.b, 0x66).wrapping_add(p.u8(x.b, 0x15c)).wrapping_add(p.u8(x.b, 0x8a));
    let state = p.u32(x.b, pl::STATE) & 0xffff_ff00;
    let standing = matches!(state, 0x0200_0000 | 0x0200_0600 | 0x0200_0400);
    let y = p.u16(x.b, pl::Y).wrapping_sub(x.u16(0x10)).wrapping_add(0x20);
    if busy == 0
        && input & 1 != 0
        && p.u8(x.b, pl::STATE) == 2
        && standing
        && p.u16(x.b, pl::Z) == 0
        && (p.u16(x.b, pl::X) as i16) >= DOOR_X
        && y <= 0x40
    {
        x.add8(counter, 1);
    } else {
        x.set8(counter, 0);
    }
}

fn watch<R: Bench>(x: &mut Cx<'_, R>) {
    for (i, p) in players().enumerate() {
        let bit = 1u8 << i;
        let counter = 0x42 + i as u32;
        let entered = x.u8(0x17) & bit != 0;
        let mut counted_check = entered;
        if !entered {
            if p.u8(x.b, pl::ACTIVE) != 1 {
                continue;
            }
            if (x.u8(counter) as i8) < 0 {
                counted_check = true;
            } else {
                let input = x.a5_u8(i as u32);
                at_door(x, p, input, counter);
                if (x.u8(counter) as i8) > WAIT_FRAMES {
                    return enter(x, p, bit);
                }
                continue;
            }
        }
        if counted_check && x.u8(0x1a) & bit == 0 && p.u32(x.b, pl::STATE) == WAITING_AT_DOOR {
            x.add8(0x16, 1);
            x.set_bit(0x1a, i as u8, true);
        }
    }
    // have all the players concerned entered?
    let mut needed = x.a5_u8(0x35);
    if needed == 0 {
        return;
    }
    for p in players() {
        if p.u8(x.b, pl::ACTIVE) != 0 && p.u16(x.b, pl::HEALTH) == 0 && p.u16(x.b, pl::STATE) != 0x402 {
            needed = needed.wrapping_add(1);
        }
    }
    if needed as i8 > x.u8(0x16) as i8 {
        return;
    }
    scene_end(x);
    x.set_a5_u16(GATE_LOCK, 0);
    x.set16(0x4, 0x400);
}

/// End of the zone (0xcc1c2): the count of players entered is added to the byte `A5-0x5b20`, the sound script
/// 0xcbc02 is played and the dead players who remain on the screen go to state 0x04020000.
fn scene_end<R: Bench>(x: &mut Cx<'_, R>) {
    const COUNT: u32 = 0xff_24e0;
    const SCRIPT: u32 = 0xcbc02;
    const OUT: u32 = 0x0402_0000;
    let entered = x.u8(0x24);
    let n = rd8(x.b, COUNT).wrapping_add(entered);
    wr8(x.b, COUNT, n);
    crate::text::clear_box(x.b, SCRIPT);
    for p in players() {
        if p.u8(x.b, pl::ACTIVE) != 0 && p.u16(x.b, pl::HEALTH) == 0 {
            let flags = p.u8(x.b, 1) | 2;
            p.set_u8(x.b, 1, flags);
            p.set_u32(x.b, pl::STATE, OUT);
        }
    }
}

/// Player `p` (number `bit`) enters: it is put in the "entered" state.
fn enter<R: Bench>(x: &mut Cx<'_, R>, p: Obj, bit: u8) {
    if p.u8(x.b, pl::ACTIVE) == 0 || p.u16(x.b, pl::HEALTH) == 0 {
        return;
    }
    let v = x.u8(0x17) | bit;
    x.set8(0x17, v);
    if x.a5_u16(GATE_LOCK) == 0 {
        if let Some(effect) = x.alloc(&pools::EFFCT) {
            effect.set_u8(x.b, 0, 1);
            effect.set_u8(x.b, 2, EFFECT_TYPE);
            effect.set_u8(x.b, 0x2f, 0);
            effect.set_u8(x.b, 3, 0xff);
            let v = x.u16(0x10);
            effect.set_u16(x.b, 0x10, v);
        }
    }
    x.set_a5_u16(GATE_LOCK, x.o.0 as u16);
    p.set_u8(x.b, 0, 2);
    p.set_u32(x.b, pl::STATE, ENTERED);
    p.set_u8(x.b, 0x10f, 1);
}

/// State 4: cedes control to the players, announces the exit and moves on to the next level.
fn leave<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_a5_u8(0x50, 0);
    for p in players() {
        if p.u8(x.b, pl::ACTIVE) != 0 && p.u16(x.b, pl::HEALTH) != 0 {
            let v = p.u8(x.b, 1) & !2;
            p.set_u8(x.b, 1, v);
        }
    }
    x.set_a5_u32(0x2c, 0x0800_0000);
    crate::text::clear_box(x.b, LEAVE_SCRIPT);
    let stage = x.a5_u8(0x34);
    if stage != LAST_STAGE {
        x.set_a5_u8(0x34, stage + 1);
        if x.u8(0x2f) != 0 {
            bonus_stage(x);
        }
    } else if x.u8(0x2f) == 0 {
        x.set_a5_u8(0x42, 1);
    } else {
        bonus_stage(x);
    }
    x.free(&pools::SET);
}

/// Bonus zone: return to level 0x1c, four times at most, then end of game.
fn bonus_stage<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_a5_u8(0x34, BONUS_STAGE);
    let n = x.a5_u8(0x4e).wrapping_add(1);
    x.set_a5_u8(0x4e, n);
    if n as i8 >= BONUS_STAGES as i8 {
        x.set_a5_u8(0x34, END_STAGE);
        x.set_a5_u8(0x4e, 0);
    }
}
