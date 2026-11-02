use super::*;

const GATE_LOCK: u32 = 0x52;
const PARAMS: u32 = 0xd84e8;
const ENTER_KINDS: u32 = 0xd8824;
const PLACEMENT_SIDE: u32 = 0xd896e;
const PLACEMENT: u32 = 0xd89be;
const SCRIPT: u32 = 0xd86e8;
const CHECK_FRAMES: i8 = 0x18;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let me = x.o;
    crate::effects_aids::living_players(x.b, me);
    if x.u8(0x40) != 0 || x.a5_u8(0x3d) != 0 {
        return;
    }
    let lock = x.a5_u16(GATE_LOCK);
    if lock != 0 && lock != x.o.0 as u16 {
        return;
    }
    match x.u8(0x4) {
        0 => init(x),
        2 => watch(x),
        4 => arrive(x),
        _ => x.free(&pools::SET),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    for off in [0x42, 0x43, 0x44, 0x45, 0x16, 0x17] {
        x.set8(off, 0);
    }
    x.set_a5_u8(0x4e, 0);
    let row = PARAMS + ((x.u8(0x3) as u32) << 4);
    let v = x.rom16(row);
    x.set16(0x8, v);
    let v = x.rom16(row + 2);
    x.set16(0xc, v);
    let v = x.rom16(row + 4);
    x.set16(0x10, v);
    let v = x.rom8(row + 6);
    x.set8(0x48, v);
    let v = x.rom8(row + 7);
    x.set8(0x46, v);
    let v = x.rom8(row + 8);
    x.set8(0x4f, v);
    let v = x.rom8(row + 9);
    x.set8(0x1b, v);
}

fn watch<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(0x59) == 0 {
        if x.u8(0x48) == 0 {
            return;
        }
        let v = x.a5_u8(0x35);
        x.set8(0x16, v);
        x.set_a5_u16(GATE_LOCK, x.o.0 as u16);
    }
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
                let input = p.u8(x.b, 0x6c) as u32;
                at_gate(x, p, input, counter);
                if (x.u8(counter) as i8) > CHECK_FRAMES {
                    return enter(x, p, bit);
                }
                continue;
            }
        }
        if counted_check && x.u8(0x1a) & bit == 0 && p.u32(x.b, pl::STATE) == 0x0200_2010 {
            x.add8(0x16, 1);
            x.set_bit(0x1a, i as u8, true);
        }
    }
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
    leave(x);
}

/// A player goes through the door: either an appearance (variant 0 without `A5+0x4e`), or it is put in the entry
/// state.
fn enter<R: Bench>(x: &mut Cx<'_, R>, p: Obj, bit: u8) {
    if x.u8(0x3) == 0 && x.a5_u8(0x4e) == 0 {
        let spawned = scene_set(x, 0x8b);
        spawned.set_u8(x.b, 0x42, 0xf);
        let v = p.u8(x.b, 3);
        spawned.set_u8(x.b, 0x43, v);
        let i = bit.trailing_zeros();
        x.set8(0x42 + i, 0);
        return;
    }
    let v = x.u8(0x17) | bit;
    x.set8(0x17, v);
    x.set_a5_u16(GATE_LOCK, x.o.0 as u16);
    let kind = x.u8(0x46) as u32;
    let state = x.rom16(ENTER_KINDS + 4 * kind) as u32;
    let state = state << 16 | x.rom16(ENTER_KINDS + 4 * kind + 2) as u32;
    p.set_u32(x.b, pl::STATE, state);
    p.set_u8(x.b, 0, 2);
    p.set_u8(x.b, 0x10f, 1);
}

/// All the players concerned are at the door: the passage begins.
fn leave<R: Bench>(x: &mut Cx<'_, R>) {
    crate::sound::service(x.b, crate::sound::Service::Ff06 { lock: 0xff, line: 4 });
    x.set_a5_u8(0x51, 1);
    let level = x.u8(0x4f);
    x.set_a5_u8(0x34, level);
    if x.u8(0x3) == 0 {
        x.set_a5_u8(0x42, 1);
        return;
    }
    crate::text::clear_box(x.b, SCRIPT);
    x.set_a5_u32(0x5c6, 0);
    // 0xdd12c: the pickup objects' appearance lock lifted
    crate::mem::wr8(x.b, A5 - 0x5ac4, 0);
    x.set_a5_u16(GATE_LOCK, 0);
    match x.u8(0x48) as i8 {
        1 => exit_zone(x),
        mode if mode > 1 => {}
        _ => {
            save_camera(x);
            suspend(x, &SET_POOL);
            suspend(x, &EFFCT_POOL);
            suspend(x, &SOBJ_POOL);
            x.set16(0x4, 0x400);
            enter_zone(x);
        }
    }
}

/// Loading of the hidden zone (state 0x400 of the door).
fn enter_zone<R: Bench>(x: &mut Cx<'_, R>) {
    crate::families::scenery::resume_scenery(x, crate::families::scenery::Cameras::Start);
    x.set_a5_u8(0x48e, 0xf);
    crate::palette::full_brightness(x.b);
    x.set_a5_u16(GATE_LOCK, 0);
    reset_players_after_exit(x);
    crate::families::scenery::place_players(x);
}

/// Return to the original zone: the players are released and all the objects of the hidden zone deleted; each door
/// of type 0x1b of the zone found again is replayed.
fn exit_zone<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_a5_u8(0x4e, 1);
    release_players(x);
    x.set8(0x4, 6);
    resume_with(x, &SET_POOL, |x, o| {
        if o.u8(x.b, 2) == 0x1b {
            run_on(x, o, run);
        }
    });
    resume(x, &EFFCT_POOL);
    resume(x, &BOX_POOL);
    resume(x, &ITEM_POOL);
    resume(x, &SOBJ_POOL);
}

/// State 4: arrival in the hidden zone (loading of the new zone).
fn arrive<R: Bench>(x: &mut Cx<'_, R>) {
    crate::families::scenery::resume_scenery(x, crate::families::scenery::Cameras::Return);
    x.set_a5_u8(0x48e, 0xf);
    crate::palette::full_brightness(x.b);
    x.set_a5_u16(GATE_LOCK, 0);
    x.set_a5_u8(0x51, 0);
    restore_camera(x);
    // Original glitch: D4.w = `+0x1c` (the zone's music, read by the sound of 0xe17a4 in door 29) stays for the
    // following handlers (`dispatch::Leftovers`)
    let d4 = x.u16(0x1c);
    *x.c.d4() = (*x.c.d4() & 0xffff_0000) | d4 as u32;
    crate::sound::service(x.b, crate::sound::Service::Ff06 { lock: 0x90, line: 0 });
    for p in players() {
        if p.u8(x.b, pl::ACTIVE) != 0 {
            place(x, p);
        }
    }
    x.set8(0x4, 6);
}

/// Places a player on arrival (position relative to the door, table 0xd89be according to the side).
fn place<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
    if p.u8(x.b, 4) != 2 {
        let v = p.u8(x.b, 1) | 2;
        p.set_u8(x.b, 1, v);
        crate::placement::place_on_free_ground(x.b, p);
        return;
    }
    p.set_u32(x.b, pl::STATE, 0x0200_0000);
    let v = p.u8(x.b, 1) & !2;
    p.set_u8(x.b, 1, v);
    p.set_u8(x.b, pl::ACTIVE, 1);
    let variant = x.u8(0x3) as u32;
    let side = x.rom8(PLACEMENT_SIDE + variant) as u32;
    let at = (side << 4) + 4 * p.u8(x.b, 2) as u32;
    let dx = x.rom16(PLACEMENT + at);
    let dy = x.rom16(PLACEMENT + 2 + at);
    let v = x.u16(0x8);
    p.set_u16(x.b, pl::X, v);
    let v = x.u16(0x10);
    p.set_u16(x.b, pl::Y, v);
    p.set_u16(x.b, pl::Z, 0);
    let v = p.u16(x.b, pl::X).wrapping_add(dx);
    p.set_u16(x.b, pl::X, v);
    let v = p.u16(x.b, pl::Y).wrapping_add(dy);
    p.set_u16(x.b, pl::Y, v);
}
