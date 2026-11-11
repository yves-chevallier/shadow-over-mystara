//! Boss 15 / 20, state 4 "hit" (0x928ee). `+5` groups: 0 reaction (a single routine for all hit codes `+0x49`),
//! 2 knockback, 4 getting up, 6 death.
//!
//! Armor: during the first `+0xb5` hits of each 64-frame period (`+0xb4`), a hit received in the active state
//! does not interrupt it: the dragon resumes the state it had on the previous frame (`+0xb0`).

use super::*;
use crate::actors::families::cx::Bench;
use crate::world::objects::Current;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB) == 0 {
        if x.u8(Enemy::KIND) == TYPE_BIG {
            lib::drop_once(x);
        }
        if (x.u16(f::HP) as i16) <= 0 {
            x.set8(f::SUB, 6);
            return death(x);
        }
        if (x.u8(Boss15::ARMOR) as i8) > 0 {
            if sub8(x, Boss15::ARMOR, 1).gt() && x.u8(Boss15::LAST_STATE) == 2 {
                // the hit glances off the armor: resume the previous state
                let s = x.u32(Boss15::LAST_STATE);
                x.set32(f::STATE, s);
                let b = x.u8(Boss15::LAST_BOX);
                x.set8(Boss15::HURT_BOX, b);
                x.set8(Enemy::ACTIVE, 1);
                let stun = x.u8(Enemy::STUN);
                x.set8(f::FREEZE, stun);
                x.set8(Enemy::REACTION, 0);
                x.set8(Boss15::PUSHED, 0x10);
                return active(x);
            }
            x.set8(Boss15::ARMOR_TICK, 0);
        }
    }
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
        return x.reinsert();
    }
    if tst8(x, Boss15::PUSHED).gt() && sub8(x, Boss15::PUSHED, 1).le() {
        x.set32(Enemy::HIT_LOCK, 0);
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
    if tst8(x, Boss15::SHIELD).gt() {
        sub8(x, Boss15::SHIELD, 1);
        x.set8(Enemy::FRAME_TAG, 0);
    }
    x.reinsert();
}

/// 0x929c4: reaction to the hit (table 0x929da has only one routine, 0x92a82).
fn reaction<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        lib::set_68(x);
    }
    if x.u8(f::SUB2) == 0 {
        var::CAMERA_MODE.set(x.b, 0);
        if (x.u16(f::HP) as i16) <= 0 {
            x.set8(f::SUB, 6);
            return death(x);
        }
        x.add8(f::SUB2, 2);
        let stun = x.u8(Enemy::STUN);
        x.set8(f::FREEZE, stun);
        x.set8(Boss::SHAKE, 0x10);
        return set_anim(x, 4, 0);
    }
    if var::PAUSE.get(x.b) != 0 {
        x.set8(Enemy::ACTIVE, 2);
        return;
    }
    x.set8(Enemy::ACTIVE, 1);
    step(x);
    if tst8(x, Boss::SHAKE).gt() {
        sub8(x, Boss::SHAKE, 1);
        let v = x.u16(Enemy::VX).wrapping_neg();
        x.set16(Enemy::VX, v);
        let mut d: i16 = 2;
        if x.u8(Boss::SHAKE) & 1 != 0 {
            d = -d;
        }
        x.add16(Obj::X, d as u16);
        return;
    }
    x.set8(Enemy::ACTIVE, 1);
    x.set8(Enemy::REACTION, 0);
    x.set32(Enemy::HIT_LOCK, 0);
    stand(x);
}

/// 0x92b2c: knockback: the dragon slides backward for 15 frames.
fn pushed_back<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PAUSE.get(x.b) != 0 {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set16(Boss15::TIMER, 0xf);
        x.set16(Enemy::VX, 0x480);
        x.set16(Enemy::AX, (-0x80i16) as u16);
        let side = x.u8(Enemy::ATTACKER_FLIP) ^ 0x20;
        x.set8(f::FLIP, side);
        if side != 0 {
            let (a, b) = (x.u16(Enemy::VX).wrapping_neg(), x.u16(Enemy::AX).wrapping_neg());
            x.set16(Enemy::VX, a);
            x.set16(Enemy::AX, b);
        }
        return set_anim(x, 0, 1);
    }
    if x.u8(Enemy::VX) != 0 {
        x.move_x();
        return;
    }
    x.set32(Enemy::HIT_LOCK, 0);
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
            if sub16(x, Boss::DOWN_TIME, 1).gt() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(Enemy::ACTIVE, 2);
            x.set16(Boss::DOWN_TIME, 0);
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
        }
        4 => {
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            if !step(x) {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(Boss15::TIMER, 8);
            x.set8(Enemy::ACTIVE, 2);
            x.set_bit(Enemy::DISPLAY, 1, true);
        }
        6 => {
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            if sub16(x, Boss15::TIMER, 1).pl() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set_bit(Enemy::DISPLAY, 1, false);
        }
        8 => {
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            if !step(x) {
                return;
            }
            x.set8(Enemy::VULNERABLE, 1);
            x.set32(f::STATE, STAND);
            x.set8(Enemy::ACTIVE, 1);
        }
        _ => {}
    }
}

/// 0x92c2a: death: fall off the screen, then release when the scenery is done (`A5+0x42` ≥ 2).
fn death<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            lib::sound(x, crate::services::sound::CMD_390);
            var::PLAYERS_FROZEN.set(x.b, 1);
            var::BOSS_FIGHT.set(x.b, 0);
            var::DEFEAT_NAME_WORD.set(x.b, 0x0f00);
            let bar = x.u8(Enemy::EVENT_KIND);
            var::DEFEAT_BONUS.set(x.b, bar);
            lib::drop_rewards(x);
            crate::services::sound::service(x.b, crate::services::sound::Service::Ff05);
            lib::sound(x, crate::services::sound::CMD_FF);
            lib::flash_and_shake(x);
            x.add8(f::SUB2, 2);
            x.set8(Enemy::ACTIVE, 2);
            let stun = x.u8(Enemy::STUN);
            x.set8(f::FREEZE, stun);
            x.set32(Enemy::VX_LONG, 0);
            x.set16(Enemy::VZ, 0x200);
            x.set16(Enemy::GRAVITY, 0);
            set_anim(x, 4, 0);
        }
        2 => {
            step(x);
            x.move_xz();
            let cam_y = var::SCROLL_Y.get(x.b);
            let d = x.u16(Obj::Y).wrapping_sub(cam_y).wrapping_add(x.u16(Obj::Z));
            if (d as i16) <= 0x200 {
                return;
            }
            x.add8(f::SUB2, 2);
        }
        4 => {
            x.set8(Boss15::AIRBORNE, 1);
            if (var::PLAYERS_FROZEN.get(x.b) as i8) < 2 {
                return;
            }
            x.add8(f::STATE, 2);
        }
        _ => {}
    }
}
