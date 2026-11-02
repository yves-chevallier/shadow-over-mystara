//! Boss 15 / 20, state 4 "hit" (0x928ee). `+5` groups: 0 reaction (a single routine for all hit codes `+0x49`),
//! 2 knockback, 4 getting up, 6 death.
//!
//! Armor: during the first `+0xb5` hits of each 64-frame period (`+0xb4`), a hit received in the active state
//! does not interrupt it: the dragon resumes the state it had on the previous frame (`+0xb0`).

use super::*;
use crate::families::cx::Bench;
use crate::objects::Current;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB) == 0 {
        if x.u8(TYPE) == TYPE_BIG {
            lib::drop_once(x);
        }
        if (x.u16(f::HP) as i16) <= 0 {
            x.set8(f::SUB, 6);
            return death(x);
        }
        if (x.u8(ARMOR) as i8) > 0 {
            if sub8(x, ARMOR, 1).gt() && x.u8(LAST_STATE) == 2 {
                // the hit glances off the armor: resume the previous state
                let s = x.u32(LAST_STATE);
                x.set32(f::STATE, s);
                let b = x.u8(LAST_BOX);
                x.set8(HURT_BOX, b);
                x.set8(0, 1);
                let stun = x.u8(STUN);
                x.set8(f::FREEZE, stun);
                x.set8(0x49, 0);
                x.set8(PUSHED, 0x10);
                return active(x);
            }
            x.set8(ARMOR_TICK, 0);
        }
    }
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
        return x.reinsert();
    }
    if tst8(x, PUSHED).gt() && sub8(x, PUSHED, 1).le() {
        x.set32(0x58, 0);
    }
    restore_position(x);
    match x.u8(f::SUB) {
        0 => reaction(x),
        2 => pushed_back(x),
        4 => get_up(x),
        6 => death(x),
        // table 0x929b8 has six entries: 8 (`rts`) and 0xa (0x92ce0, back to the fight) are never chosen (no writer
        // sets `+5` = 8 or 0xa in state 4: docs/code-mort.md), kept like the original
        8 => {}
        0xa => chase(x),
        s => unreachable!("boss 15 hit: sub-state {s:#x} outside table 0x929b8"),
    }
    keep_position(x);
    if tst8(x, SHIELD).gt() {
        sub8(x, SHIELD, 1);
        x.set8(0x38, 0);
    }
    x.reinsert();
}

/// 0x929c4: reaction to the hit (table 0x929da has only one routine, 0x92a82).
fn reaction<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        lib::set_68(x);
    }
    if x.u8(f::SUB2) == 0 {
        x.set_a5_u16(0x51e, 0);
        if (x.u16(f::HP) as i16) <= 0 {
            x.set8(f::SUB, 6);
            return death(x);
        }
        x.add8(f::SUB2, 2);
        let stun = x.u8(STUN);
        x.set8(f::FREEZE, stun);
        x.set8(SHAKE, 0x10);
        return set_anim(x, 4, 0);
    }
    if var::PAUSE.get(x.b) != 0 {
        x.set8(0, 2);
        return;
    }
    x.set8(0, 1);
    step(x);
    if tst8(x, SHAKE).gt() {
        sub8(x, SHAKE, 1);
        let v = x.u16(0x14).wrapping_neg();
        x.set16(0x14, v);
        let mut d: i16 = 2;
        if x.u8(SHAKE) & 1 != 0 {
            d = -d;
        }
        x.add16(0x8, d as u16);
        return;
    }
    x.set8(0, 1);
    x.set8(0x49, 0);
    x.set32(0x58, 0);
    stand(x);
}

/// 0x92b2c: knockback: the dragon slides backward for 15 frames.
fn pushed_back<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PAUSE.get(x.b) != 0 {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set16(TIMER, 0xf);
        x.set16(0x14, 0x480);
        x.set16(0x16, (-0x80i16) as u16);
        let side = x.u8(0x4a) ^ 0x20;
        x.set8(f::FLIP, side);
        if side != 0 {
            let (a, b) = (x.u16(0x14).wrapping_neg(), x.u16(0x16).wrapping_neg());
            x.set16(0x14, a);
            x.set16(0x16, b);
        }
        return set_anim(x, 0, 1);
    }
    if x.u8(0x14) != 0 {
        x.move_x();
        return;
    }
    x.set32(0x58, 0);
    stand(x);
}

/// 0x92b8c: getting up (`+6` = 0 to 8): wait, then straightening up with blinking.
fn get_up<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 | 2 => {
            if x.u8(f::SUB2) == 0 {
                x.add8(f::SUB2, 2);
            }
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            if sub16(x, 0x84, 1).gt() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(0, 2);
            x.set16(0x84, 0);
            x.set8(0x86, 0);
            x.set8(0x49, 0);
        }
        4 => {
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            if !step(x) {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(TIMER, 8);
            x.set8(0, 2);
            x.set_bit(0x1, 1, true);
        }
        6 => {
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            if sub16(x, TIMER, 1).pl() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set_bit(0x1, 1, false);
        }
        8 => {
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            if !step(x) {
                return;
            }
            x.set8(VULNERABLE, 1);
            x.set32(f::STATE, STAND);
            x.set8(0, 1);
        }
        _ => {}
    }
}

/// 0x92c2a: death: fall off the screen, then release when the scenery is done (`A5+0x42` ≥ 2).
fn death<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            lib::sound(x, crate::sound::CMD_390);
            x.set_a5_u8(0x42, 1);
            x.set_a5_u8(0x50, 0);
            crate::mem::wr16(x.b, 0xff2a84, 0x0f00);
            let bar = x.u8(0x7e);
            set_abs8(x, 0xff2a86, bar);
            lib::drop_rewards(x);
            crate::sound::service(x.b, crate::sound::Service::Ff05);
            lib::sound(x, crate::sound::CMD_FF);
            lib::flash_and_shake(x);
            x.add8(f::SUB2, 2);
            x.set8(0, 2);
            let stun = x.u8(STUN);
            x.set8(f::FREEZE, stun);
            x.set32(0x14, 0);
            x.set16(0x18, 0x200);
            x.set16(0x1a, 0);
            set_anim(x, 4, 0);
        }
        2 => {
            step(x);
            x.move_xz();
            let cam_y = var::SCROLL_Y.get(x.b);
            let d = x.u16(0x10).wrapping_sub(cam_y).wrapping_add(x.u16(0xc));
            if (d as i16) <= 0x200 {
                return;
            }
            x.add8(f::SUB2, 2);
        }
        4 => {
            x.set8(0x42, 1);
            if (x.a5_u8(0x42) as i8) < 2 {
                return;
            }
            x.add8(f::STATE, 2);
        }
        _ => {}
    }
}
