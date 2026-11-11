use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::vars;
use crate::actors::families::shell::records::{Host, view};
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x17a276);
/// Substates of state 2 (`+5`).
const FLY: u8 = 0;
const HOLD: u8 = 2;
const SWING: u8 = 4;
/// Flight phases (`+6`).
const PHASE_LAST: u8 = 8;
const PHASE_BLINK: u8 = 10;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => spawn(x),
        2 => active(x),
        _ => x.free_eshl(),
    }
}

/// Creation: placed in front of the owner, depending on the variant.
fn spawn<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Base::BOX_SET, 7);
    if x.u16(Shot::OWNER) == 0 {
        return kill(x);
    }
    let owner = x.linked(Shot::OWNER);
    let variant = x.u8(Obj::VARIANT);
    let (dx, dz, dy) = if variant == 0 { (0x67, 0x4a, 0x10) } else { (0x42, 0x2e, 0) };
    place_in_front(x, owner, dx, dz);
    let flip = owner.u8(x.b, Base::FLIP);
    x.set8(Base::FLIP, flip);
    x.add16(Obj::Y, dy);
    x.set8(Shot15::BLINK, 4);
    match variant {
        0 => {
            x.set32(Obj::STATE, 0x0200_0000);
            x.set8(Shot::ATTACK_POWER, 8);
            x.set8(Shot::ATTACK_ID, 0x10);
            aim(x);
            x.anim_start(ANIMS.0, 5);
        }
        1 => {
            x.set8(Shot15::RELEASED, 0);
            x.set8(Shot::ATTACK_POWER, 0xc);
            x.set8(Shot::ATTACK_ID, 0x11);
            crate::hw::sound::play(x.b, crate::hw::sound::CMD_32B, x.o);
            let owner_state = owner.u32(x.b, Obj::STATE);
            if owner_state == 0x0202_0004 {
                x.set32(Obj::STATE, 0x0202_0000);
                x.anim_start(ANIMS.0, 0x13);
            } else if owner_state == 0x0202_0204 {
                x.set32(Obj::STATE, 0x0204_0000);
                x.anim_start(ANIMS.0, 0x10);
            }
        }
        _ => {}
    }
}

/// Initial speeds of variant 0: range between the enemy (owner of the owner) and
/// its target, row 8, 0x10 or 0x18 of `SPEEDS`.
fn aim<R: Bench>(x: &mut Cx<'_, R>) {
    let part = x.linked(Shot::OWNER);
    let enemy = link_of(x, part, Shot::OWNER);
    let target = link_of(x, enemy, Shot::OWNER);
    let row = if enemy.u16(x.b, Shot::OWNER) == 0 {
        0x18
    } else {
        let flipped = enemy.u8(x.b, Base::FLIP) != 0;
        let (tx, ex) = (target.u16(x.b, Obj::X), enemy.u16(x.b, Obj::X));
        let range = if flipped { tx.wrapping_sub(ex) } else { ex.wrapping_sub(tx) } as i16;
        if range <= 0xa8 {
            8
        } else if range <= 0xfc {
            0x10
        } else {
            0x18
        }
    };
    let mut vx = tables().shot15_speeds.at16(x.b, row);
    if enemy.u8(x.b, Base::FLIP) != 0 {
        vx = vx.wrapping_neg();
    }
    x.set16(Obj::VX, vx);
    let vz = tables().shot15_speeds.at16(x.b, row + 2);
    x.set16(Obj::VZ, vz);
    let az = tables().shot15_speeds.at16(x.b, row + 4);
    x.set16(Obj::AZ, az);
    let ax = tables().shot15_speeds.at16(x.b, row + 6);
    x.set16(Obj::AX, ax);
}

/// Copies the owner's position (offset in front of it).
fn follow<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(Shot::OWNER) == 0 {
        return;
    }
    let owner = x.linked(Shot::OWNER);
    let flip = owner.u8(x.b, Base::FLIP);
    x.set8(Base::FLIP, flip);
    place_in_front(x, owner, 0x42, 0x2e);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(Shot::OWNER) == 0 {
        return kill(x);
    }
    if x.u8(Obj::VARIANT) != 0 {
        let part = x.linked(Shot::OWNER);
        let enemy = link_of(x, part, Shot::OWNER);
        if enemy.u16(x.b, Obj::STATE) != 0x402 && (enemy.u8(x.b, Obj::COUNTER_53) as i8) > 0 {
            return x.reinsert();
        }
    }
    match x.u8(Base::SUB_STATE) {
        FLY => fly(x),
        HOLD => hold(x),
        SWING => swing(x),
        _ => {}
    }
    x.reinsert();
}

/// The owner has finished its attack (or is in state 0x402): the weapon disappears.
fn owner_done<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let part = x.linked(Shot::OWNER);
    let enemy = link_of(x, part, Shot::OWNER);
    if enemy.u16(x.b, Obj::STATE) == 0x402 || enemy.u8(x.b, Host::LAUNCHED) & 1 != 0 {
        x.set8(Shot15::RELEASED, 1);
        x.set8(Obj::STATE, st::FREE);
        return true;
    }
    false
}

/// Flight of variant 0: the direction reverses every 5 frames, six phases of bounces
/// (decreasing speeds), then blinking.
fn fly<R: Bench>(x: &mut Cx<'_, R>) {
    let phase = x.u8(Base::STEP);
    if phase > PHASE_BLINK {
        return;
    }
    x.add8(Shot15::BLINK, 0xff);
    if (x.u8(Shot15::BLINK) as i8) < 0 {
        let flip = x.u8(Base::FLIP) ^ 0x20;
        x.set8(Base::FLIP, flip);
        x.set8(Shot15::BLINK, 4);
    }
    if phase == PHASE_BLINK {
        x.add8(Shot::TIMER, 0xff);
        if (x.u8(Shot::TIMER) as i8) < 0 {
            return x.set8(Obj::STATE, st::FREE);
        }
        let visible = x.u8(Shot::TIMER) & 3 == 0;
        x.set_bit(Obj::FLAGS, 1, !visible);
        return;
    }
    if !x.move_xz().gt() {
        x.add8(Base::STEP, 2);
        if phase == PHASE_LAST {
            x.set8(Shot::TIMER, 0x32);
        } else {
            // bounce: (VX, VZ) per phase
            let (vx, vz) = [(-0x180i16, 0x3c0), (-0x100, 0x300), (-0xc0, 0x280), (-0x80, 0x200)][phase as usize / 2];
            x.set32(Obj::Z, 0);
            x.set16(Obj::VX, vx as u16);
            x.set16(Obj::VZ, vz);
            x.set16(Obj::AZ, -0x38i16 as u16);
            let part = x.linked(Shot::OWNER);
            let enemy = link_of(x, part, Shot::OWNER);
            if enemy.u8(x.b, Base::FLIP) != 0 {
                let v = x.u16(Obj::VX).wrapping_neg();
                x.set16(Obj::VX, v);
            }
        }
    }
    x.cull_offscreen();
}

/// Variant 1 (held weapon): follows the owner while the enemy attacks.
fn hold<R: Bench>(x: &mut Cx<'_, R>) {
    if vars::GAME_PAUSED.get(x.b) != 0 {
        return follow(x);
    }
    if owner_done(x) {
        return;
    }
    if x.u8(Base::STEP) == 0 {
        follow(x);
        if x.anim_step() {
            x.set8(Shot15::RELEASED, 1);
            x.set8(Obj::STATE, st::FREE);
        }
    }
}

/// Variant 1 (hit): three-stage animation (hit, 8-frame wait, return).
fn swing<R: Bench>(x: &mut Cx<'_, R>) {
    if vars::GAME_PAUSED.get(x.b) != 0 {
        return follow(x);
    }
    if owner_done(x) {
        return;
    }
    match x.u8(Base::STEP) {
        0 => {
            follow(x);
            if x.anim_step() {
                x.add8(Base::STEP, 2);
                x.set8(Shot15::TIMER, 8);
                x.anim_start(ANIMS.0, 0x11);
            }
        }
        2 => {
            follow(x);
            if x.anim_step() {
                x.add8(Shot15::TIMER, 0xff);
                if (x.u8(Shot15::TIMER) as i8) >= 0 {
                    x.anim_start(ANIMS.0, 0x11);
                } else {
                    x.add8(Base::STEP, 2);
                    x.anim_start(ANIMS.0, 0x12);
                }
            }
        }
        4 => {
            follow(x);
            if x.anim_step() {
                x.set8(Shot15::RELEASED, 1);
                x.set8(Obj::STATE, st::FREE);
            }
        }
        _ => {}
    }
}

view! {
    /// Fields of shot 15.
    Shot15 {
        /// Countdown (h).
        timer, set_timer, TIMER: u8 @ 0x42;
        /// Blink countdown (h).
        blink, set_blink, BLINK: u8 @ 0x43;
        /// Set to 1 when released by its enemy (h).
        released, set_released, RELEASED: u8 @ 0x5d;
    }
}
