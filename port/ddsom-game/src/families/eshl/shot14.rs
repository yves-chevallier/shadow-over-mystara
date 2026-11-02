use super::*;

const ANIMS: u32 = 0x17a276;
const ANIMS_CHARGE: u32 = 0x17a29e;
const ANIMS_END: u32 = 0x17a2b4;
const ANIM_OF_VARIANT: u32 = 0x9f7dc;
const OFFSETS: u32 = 0x9fd50;
/// Byte table (data space) of the edge shot animations.
const BREATH_ANIMS: u32 = 0x9fe00;
/// Attack flags of the owner.
const LAUNCHED: u32 = 0xa8;
const PREPARING: u32 = 0xa9;

/// Phases (`+4`).
const START: u8 = 0;
const ACTIVE: u8 = 2;
const ENDING: u8 = 4;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        START => {
            if x.a5_u8(GAME_PAUSED) != 0 {
                return attach(x);
            }
            if (x.a5_u16(0x7e64) as i16) < 1 {
                return;
            }
            x.add8(0x4, 2);
            attach(x);
            x.set32(0xc, 0);
            x.set8(f::BOX_SET, 7);
            let variant = x.u8(0x3) as u32;
            let anim = x.rom8(ANIM_OF_VARIANT + variant);
            x.anim_start(ANIMS, (anim as u32) as u16);
        }
        ACTIVE => {
            if x.a5_u8(GAME_PAUSED) != 0 {
                return attach(x);
            }
            let owner = x.linked(f::OWNER);
            let stalled = owner.u16(x.b, 0x4) != 0x402 && (owner.u8(x.b, 0x53) as i8) > 0;
            if !stalled {
                match x.u8(0x5) {
                    0 => enter(x, owner),
                    2 => attack(x),
                    _ => {}
                }
            }
            x.reinsert();
        }
        ENDING => {
            match x.u8(0x5) {
                0 => finish(x),
                2 => attach(x),
                _ => {}
            }
            x.reinsert();
        }
        _ => x.free_eshl(),
    }
}

/// Sticks the part to the owner: position, orientation and offset according to the variant.
fn attach<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::OWNER) == 0 {
        // the original calls itself indefinitely; this case does not exist in play
        return;
    }
    let owner = x.linked(f::OWNER);
    for off in [0x8, 0x10, 0xc] {
        let v = owner.u32(x.b, off);
        x.set32(off, v);
    }
    let flip = owner.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    let row = OFFSETS + ((8 * x.u8(0x3) as u32) & 0xff);
    let (mut dx, mut dy, mut dfrac, dz) = (x.rom16(row), x.rom16(row + 2), x.rom16(row + 4), x.rom16(row + 6));
    if flip != 0 {
        dx = dx.wrapping_neg();
        if x.u8(0x3) == 2 {
            dx = dx.wrapping_sub(0x24);
            dy = dy.wrapping_sub(0x20);
            dfrac = 0x9000;
        }
    }
    x.add16(0x8, dx);
    x.add16(0x10, dy);
    x.add32(0x10, dfrac as i16 as i32 as u32);
    x.add16(0xc, dz);
}

/// First substate of the active phase: according to the variant and the owner's attack flags, sets the part's
/// states (`+4..+7`), then animates.
fn enter<R: Bench>(x: &mut Cx<'_, R>, owner: Obj) {
    attach(x);
    let v = x.u8(0x3);
    let flags = |x: &mut Cx<'_, R>, off: u32| owner.u8(x.b, off) >> (v & 7) & 1 != 0;
    let (launched, preparing) = (flags(x, LAUNCHED), flags(x, PREPARING));
    let owner_state = owner.u32(x.b, 0x4);
    let remap = |x: &mut Cx<'_, R>, rules: &[(u32, u32)]| {
        for &(from, to) in rules {
            if owner_state == from {
                x.set32(0x4, to);
            }
        }
    };
    match v {
        0 => {
            if launched {
                x.set32(0x4, 0x0400_0a00);
            } else if preparing {
                remap(
                    x,
                    &[
                        (0x0204_0402, 0x0202_0000),
                        (0x0204_0602, 0x0202_0200),
                        (0x0204_0a06, 0x0202_0000),
                        (0x0204_1002, 0x0202_0600),
                        (0x0204_1204, 0x0202_0600),
                        (0x0204_1606, 0x0202_0600),
                    ],
                );
            }
        }
        2 => {
            if launched {
                x.set32(0x4, 0x0400_0400);
            }
        }
        3 | 4 => {
            if launched {
                x.set32(0x4, if v == 3 { 0x0400_0800 } else { 0x0400_0600 });
            } else if preparing {
                remap(
                    x,
                    &[(0x0204_0804, 0x0202_0400), (0x0204_0a02, 0x0202_0400), (0x0204_1404, 0x0202_0400), (0x0204_1602, 0x0202_0400)],
                );
            }
        }
        6 => {
            if launched {
                x.set32(0x4, 0x0400_0000);
            }
        }
        7 => {
            if launched {
                x.set32(0x4, 0x0400_0200);
            }
        }
        _ => {}
    }
    if v == 2 {
        let anim = if owner.u8(x.b, 0xa1) == 2 { 4 } else { 3 };
        if anim != x.u8(0x5c) {
            x.set8(0x5c, anim);
            x.anim_start(ANIMS, (anim as u32) as u16);
        }
    }
    x.anim_step();
    x.reinsert();
}

/// Creates a type 15 shot linked to this part; the original does not examine the allocation failure.
fn spawn_child<R: Bench>(x: &mut Cx<'_, R>, variant: u8) -> Obj {
    // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
    let child = x.alloc_unchecked(&pools::ESHL);
    let active = x.u8(0);
    child.set_u8(x.b, 0, active);
    child.set_u8(x.b, 2, 0xf);
    child.set_u8(x.b, 3, variant);
    child.set_u16(x.b, f::OWNER, x.o.0 as u16);
    child
}

/// Substate 2 of the active phase: five animation sequences according to `+6` / `+7`.
fn attack<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x6) {
        0 => match x.u8(0x7) {
            0 => begin_charge(x, 3),
            2 => {
                attach(x);
                if x.anim_step() {
                    x.add8(0x7, 2);
                    let child = spawn_child(x, 1);
                    x.set16(0x44, child.0 as u16);
                }
            }
            4 => wait_child(x),
            _ => close(x, 7),
        },
        2 => match x.u8(0x7) {
            0 => begin_charge(x, 3),
            2 => {
                attach(x);
                if x.anim_step() {
                    x.add8(0x7, 2);
                    let owner = x.linked(f::OWNER);
                    owner.set_u8(x.b, 0xbd, 0);
                    let child = spawn_child(x, 1);
                    x.set16(0x44, child.0 as u16);
                }
            }
            4 => {
                let child = x.u16(0x44);
                if child == 0 || Obj(child as i16 as i32 as u32 & 0xff_ffff).u8(x.b, 0x5d) == 1 {
                    x.add8(0x7, 2);
                    let owner = x.linked(f::OWNER);
                    owner.set_u8(x.b, 0xbd, 0);
                    x.anim_start(ANIMS_CHARGE, 4);
                } else {
                    attach(x);
                    x.anim_step();
                }
            }
            _ => close(x, 7),
        },
        4 => match x.u8(0x7) {
            0 => {
                x.add8(0x7, 2);
                x.set8(f::ATTACK_POWER, 8);
                x.set8(f::ATTACK_ID, 0xf);
                crate::sound::play(x.b, crate::sound::CMD_32A_42A, x.o);
                breath_anim(x);
            }
            _ => {
                if x.u8(0x33) != 0 {
                    x.set8(0x33, 0);
                    let owner = x.linked(f::OWNER);
                    owner.set_u8(x.b, 0xbd, 0);
                }
                attach(x);
                if x.anim_step() {
                    x.set32(0x4, 0x0200_0000);
                    let owner = x.linked(f::OWNER);
                    let v = owner.u8(x.b, PREPARING);
                    owner.set_u8(x.b, PREPARING, v & !0x18);
                    let anim = if x.u8(0x3) == 3 { 8 } else { 9 };
                    x.anim_start(ANIMS, anim);
                }
            }
        },
        _ => match x.u8(0x7) {
            0 => {
                x.add8(0x7, 2);
                x.anim_start(ANIMS_CHARGE, 2);
            }
            2 => {
                if x.u8(0x33) != 0 {
                    crate::sound::play(x.b, crate::sound::CMD_32C_42C, x.o);
                    x.add8(0x7, 2);
                    let child = spawn_child(x, 0);
                    x.set16(0x42, child.0 as u16);
                }
                attach(x);
                x.anim_step();
            }
            // 0x9fbd4: the third word of the jump table (0x9fbe2) leads to 0x9fb1e, the end of the other series
            _ => close(x, 7),
        },
    }
}

fn begin_charge<R: Bench>(x: &mut Cx<'_, R>, anim: u32) {
    x.add8(0x7, 2);
    x.anim_start(ANIMS_CHARGE, anim as u16);
}

fn wait_child<R: Bench>(x: &mut Cx<'_, R>) {
    let child = x.u16(0x44);
    if child == 0 || Obj(child as i16 as i32 as u32 & 0xff_ffff).u8(x.b, 0x5d) == 1 {
        x.add8(0x7, 2);
        x.anim_start(ANIMS_CHARGE, 4);
    } else {
        attach(x);
        x.anim_step();
    }
}

/// End of a charge: cedes control to the owner (state 2) and starts the closing animation.
fn close<R: Bench>(x: &mut Cx<'_, R>, anim: u32) {
    attach(x);
    if x.anim_step() {
        x.set32(0x4, 0x0200_0000);
        let owner = x.linked(f::OWNER);
        let v = owner.u8(x.b, PREPARING);
        owner.set_u8(x.b, PREPARING, v & !1);
        owner.set_u8(x.b, 0xbd, 0);
        x.anim_start(ANIMS, anim as u16);
    }
}

/// Breath animation according to the distance between the owner and its target.
fn breath_anim<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::OWNER) == 0 {
        return;
    }
    let owner = x.linked(f::OWNER);
    let target = owner.u16(x.b, f::OWNER);
    let mut index = 3u32;
    if target != 0 {
        let target = Obj(target as i16 as i32 as u32 & 0xff_ffff);
        let (a, b) = if owner.u8(x.b, f::FLIP) == 0 {
            (target.u16(x.b, 0x8), owner.u16(x.b, 0x8))
        } else {
            (owner.u16(x.b, 0x8), target.u16(x.b, 0x8))
        };
        index = if (b.wrapping_sub(a) as i16) > 0xfc { 3 } else { 2 };
    }
    index *= 2;
    let first = x.data8(BREATH_ANIMS + index);
    let second = x.data8(BREATH_ANIMS + index + 1);
    let anim = if x.u8(0x3) == 3 { first } else { second };
    x.anim_start(ANIMS_CHARGE, (anim as u32) as u16);
}

/// Phase 4, substate 0: releases the owner's preparation flag and starts the end animation chosen by `+6`.
fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x5, 2);
    let v = x.u8(0x3);
    let owner = x.linked(f::OWNER);
    let flags = owner.u8(x.b, PREPARING);
    owner.set_u8(x.b, PREPARING, flags & !(1 << (v & 7)));
    let anim = match x.u8(0x6) {
        0 => 5,
        2 => 6,
        4 => 7,
        6 => 4,
        8 => 3,
        _ => 2,
    };
    x.anim_start(ANIMS_END, anim);
    attach(x);
}
