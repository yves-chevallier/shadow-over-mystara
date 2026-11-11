use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::vars;
use crate::actors::families::shell::records::{Host, view};
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x17a276);
const ANIMS_CHARGE: RomAddr = RomAddr(0x17a29e);
const ANIMS_END: RomAddr = RomAddr(0x17a2b4);
/// Attack flags of the owner.
const LAUNCHED: u32 = 0xa8;
const PREPARING: u32 = 0xa9;

/// Phases (`+4`).
const START: u8 = 0;
const ACTIVE: u8 = 2;
const ENDING: u8 = 4;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        START => {
            if vars::GAME_PAUSED.get(x.b) != 0 {
                return attach(x);
            }
            if (vars::LIMIT_TWELVE.get(x.b) as i16) < 1 {
                return;
            }
            x.add8(Obj::STATE, 2);
            attach(x);
            x.set32(Obj::Z, 0);
            x.set8(Base::BOX_SET, 7);
            let variant = x.u8(Obj::VARIANT) as u32;
            let anim = tables().shot14_anims_of_variant.at8(x.b, variant);
            x.anim_start(ANIMS.0, (anim as u32) as u16);
        }
        ACTIVE => {
            if vars::GAME_PAUSED.get(x.b) != 0 {
                return attach(x);
            }
            let owner = x.linked(Shot::OWNER);
            let stalled = owner.u16(x.b, Obj::STATE) != 0x402 && (owner.u8(x.b, Obj::COUNTER_53) as i8) > 0;
            if !stalled {
                match x.u8(Base::SUB_STATE) {
                    0 => enter(x, owner),
                    2 => attack(x),
                    _ => {}
                }
            }
            x.reinsert();
        }
        ENDING => {
            match x.u8(Base::SUB_STATE) {
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
    if x.u16(Shot::OWNER) == 0 {
        // the original calls itself indefinitely; this case does not exist in play
        return;
    }
    let owner = x.linked(Shot::OWNER);
    for off in [0x8, 0x10, 0xc] {
        let v = owner.u32(x.b, off);
        x.set32(off, v);
    }
    let flip = owner.u8(x.b, Base::FLIP);
    x.set8(Base::FLIP, flip);
    let row = (8 * x.u8(Obj::VARIANT) as u32) & 0xff;
    let (mut dx, mut dy, mut dfrac, dz) = (tables().shot14_offsets.at16(x.b, row), tables().shot14_offsets.at16(x.b, row + 2), tables().shot14_offsets.at16(x.b, row + 4), tables().shot14_offsets.at16(x.b, row + 6));
    if flip != 0 {
        dx = dx.wrapping_neg();
        if x.u8(Obj::VARIANT) == 2 {
            dx = dx.wrapping_sub(0x24);
            dy = dy.wrapping_sub(0x20);
            dfrac = 0x9000;
        }
    }
    x.add16(Obj::X, dx);
    x.add16(Obj::Y, dy);
    x.add32(Obj::Y, dfrac as i16 as i32 as u32);
    x.add16(Obj::Z, dz);
}

/// First substate of the active phase: according to the variant and the owner's attack flags, sets the part's
/// states (`+4..+7`), then animates.
fn enter<R: Bench>(x: &mut Cx<'_, R>, owner: Obj) {
    attach(x);
    let v = x.u8(Obj::VARIANT);
    let flags = |x: &mut Cx<'_, R>, off: u32| owner.u8(x.b, off) >> (v & 7) & 1 != 0;
    let (launched, preparing) = (flags(x, LAUNCHED), flags(x, PREPARING));
    let owner_state = owner.u32(x.b, Obj::STATE);
    let remap = |x: &mut Cx<'_, R>, rules: &[(u32, u32)]| {
        for &(from, to) in rules {
            if owner_state == from {
                x.set32(Obj::STATE, to);
            }
        }
    };
    match v {
        0 => {
            if launched {
                x.set32(Obj::STATE, 0x0400_0a00);
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
                x.set32(Obj::STATE, 0x0400_0400);
            }
        }
        3 | 4 => {
            if launched {
                x.set32(Obj::STATE, if v == 3 { 0x0400_0800 } else { 0x0400_0600 });
            } else if preparing {
                remap(
                    x,
                    &[(0x0204_0804, 0x0202_0400), (0x0204_0a02, 0x0202_0400), (0x0204_1404, 0x0202_0400), (0x0204_1602, 0x0202_0400)],
                );
            }
        }
        6 => {
            if launched {
                x.set32(Obj::STATE, 0x0400_0000);
            }
        }
        7 => {
            if launched {
                x.set32(Obj::STATE, 0x0400_0200);
            }
        }
        _ => {}
    }
    if v == 2 {
        let anim = if owner.u8(x.b, Host::ATTACK_DETAIL) == 2 { 4 } else { 3 };
        if anim != x.u8(Shot14::ANIM) {
            x.set8(Shot14::ANIM, anim);
            x.anim_start(ANIMS.0, (anim as u32) as u16);
        }
    }
    x.anim_step();
    x.reinsert();
}

/// Creates a type 15 shot linked to this part; the original does not examine the allocation failure.
fn spawn_child<R: Bench>(x: &mut Cx<'_, R>, variant: u8) -> Obj {
    // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
    let child = x.alloc_unchecked(&pools::ESHL);
    let active = x.u8(Obj::ACTIVE);
    child.set_u8(x.b, Obj::ACTIVE, active);
    child.set_u8(x.b, Obj::KIND, 0xf);
    child.set_u8(x.b, Obj::VARIANT, variant);
    child.set_u16(x.b, Shot::OWNER, x.o.0 as u16);
    child
}

/// Substate 2 of the active phase: five animation sequences according to `+6` / `+7`.
fn attack<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Base::STEP) {
        0 => match x.u8(Base::SUB_STEP) {
            0 => begin_charge(x, 3),
            2 => {
                attach(x);
                if x.anim_step() {
                    x.add8(Base::SUB_STEP, 2);
                    let child = spawn_child(x, 1);
                    x.set16(Shot::ATTACK_ID, child.0 as u16);
                }
            }
            4 => wait_child(x),
            _ => close(x, 7),
        },
        2 => match x.u8(Base::SUB_STEP) {
            0 => begin_charge(x, 3),
            2 => {
                attach(x);
                if x.anim_step() {
                    x.add8(Base::SUB_STEP, 2);
                    let owner = x.linked(Shot::OWNER);
                    owner.set_u8(x.b, Host::CHARGING, 0);
                    let child = spawn_child(x, 1);
                    x.set16(Shot::ATTACK_ID, child.0 as u16);
                }
            }
            4 => {
                let child = x.u16(Shot::ATTACK_ID);
                if child == 0 || Obj(child as i16 as i32 as u32 & 0xff_ffff).u8(x.b, Shot14::ENDED) == 1 {
                    x.add8(Base::SUB_STEP, 2);
                    let owner = x.linked(Shot::OWNER);
                    owner.set_u8(x.b, Host::CHARGING, 0);
                    x.anim_start(ANIMS_CHARGE.0, 4);
                } else {
                    attach(x);
                    x.anim_step();
                }
            }
            _ => close(x, 7),
        },
        4 => match x.u8(Base::SUB_STEP) {
            0 => {
                x.add8(Base::SUB_STEP, 2);
                x.set8(Shot::ATTACK_POWER, 8);
                x.set8(Shot::ATTACK_ID, 0xf);
                crate::services::sound::play(x.b, crate::services::sound::CMD_32A_42A, x.o);
                breath_anim(x);
            }
            _ => {
                if x.u8(Base::ANIM_EVENT) != 0 {
                    x.set8(Base::ANIM_EVENT, 0);
                    let owner = x.linked(Shot::OWNER);
                    owner.set_u8(x.b, Host::CHARGING, 0);
                }
                attach(x);
                if x.anim_step() {
                    x.set32(Obj::STATE, 0x0200_0000);
                    let owner = x.linked(Shot::OWNER);
                    let v = owner.u8(x.b, PREPARING);
                    owner.set_u8(x.b, PREPARING, v & !0x18);
                    let anim = if x.u8(Obj::VARIANT) == 3 { 8 } else { 9 };
                    x.anim_start(ANIMS.0, anim);
                }
            }
        },
        _ => match x.u8(Base::SUB_STEP) {
            0 => {
                x.add8(Base::SUB_STEP, 2);
                x.anim_start(ANIMS_CHARGE.0, 2);
            }
            2 => {
                if x.u8(Base::ANIM_EVENT) != 0 {
                    crate::services::sound::play(x.b, crate::services::sound::CMD_32C_42C, x.o);
                    x.add8(Base::SUB_STEP, 2);
                    let child = spawn_child(x, 0);
                    x.set16(Shot14::CHILD, child.0 as u16);
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
    x.add8(Base::SUB_STEP, 2);
    x.anim_start(ANIMS_CHARGE.0, anim as u16);
}

fn wait_child<R: Bench>(x: &mut Cx<'_, R>) {
    let child = x.u16(Shot::ATTACK_ID);
    if child == 0 || Obj(child as i16 as i32 as u32 & 0xff_ffff).u8(x.b, Shot14::ENDED) == 1 {
        x.add8(Base::SUB_STEP, 2);
        x.anim_start(ANIMS_CHARGE.0, 4);
    } else {
        attach(x);
        x.anim_step();
    }
}

/// End of a charge: cedes control to the owner (state 2) and starts the closing animation.
fn close<R: Bench>(x: &mut Cx<'_, R>, anim: u32) {
    attach(x);
    if x.anim_step() {
        x.set32(Obj::STATE, 0x0200_0000);
        let owner = x.linked(Shot::OWNER);
        let v = owner.u8(x.b, PREPARING);
        owner.set_u8(x.b, PREPARING, v & !1);
        owner.set_u8(x.b, Host::CHARGING, 0);
        x.anim_start(ANIMS.0, anim as u16);
    }
}

/// Breath animation according to the distance between the owner and its target.
fn breath_anim<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(Shot::OWNER) == 0 {
        return;
    }
    let owner = x.linked(Shot::OWNER);
    let target = owner.u16(x.b, Shot::OWNER);
    let mut index = 3u32;
    if target != 0 {
        let target = Obj(target as i16 as i32 as u32 & 0xff_ffff);
        let (a, b) = if owner.u8(x.b, Base::FLIP) == 0 {
            (target.u16(x.b, Obj::X), owner.u16(x.b, Obj::X))
        } else {
            (owner.u16(x.b, Obj::X), target.u16(x.b, Obj::X))
        };
        index = if (b.wrapping_sub(a) as i16) > 0xfc { 3 } else { 2 };
    }
    index *= 2;
    let first = tables().shot14_breath_anims.at8(x.b, index);
    let second = tables().shot14_breath_anims.at8(x.b, index + 1);
    let anim = if x.u8(Obj::VARIANT) == 3 { first } else { second };
    x.anim_start(ANIMS_CHARGE.0, (anim as u32) as u16);
}

/// Phase 4, substate 0: releases the owner's preparation flag and starts the end animation chosen by `+6`.
fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Base::SUB_STATE, 2);
    let v = x.u8(Obj::VARIANT);
    let owner = x.linked(Shot::OWNER);
    let flags = owner.u8(x.b, PREPARING);
    owner.set_u8(x.b, PREPARING, flags & !(1 << (v & 7)));
    let anim = match x.u8(Base::STEP) {
        0 => 5,
        2 => 6,
        4 => 7,
        6 => 4,
        8 => 3,
        _ => 2,
    };
    x.anim_start(ANIMS_END.0, anim);
    attach(x);
}

view! {
    /// Fields of shot 14.
    Shot14 {
        /// Child object (16-bit pointer).
        child, set_child, CHILD: u16 @ 0x42;
        /// Current animation (h).
        anim, set_anim, ANIM: u8 @ 0x5c;
        /// Set to 1 when the child ends (h).
        ended, set_ended, ENDED: u8 @ 0x5d;
    }
}
