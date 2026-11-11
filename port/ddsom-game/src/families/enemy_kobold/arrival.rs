//! Type 0, substate 0: arrival. `+6`: 0 and 2 howl, 4 entrance leap, 6 and 8 somersault (variants 12 and 13), 10 fall
//! from above, 12 appearance, 14 arrival run, 16 and 20 wake-up, 18 run toward the player.

use crate::families::cx::Bench;
use crate::families::eshl_creators;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => howl(x),
        2 => growl(x),
        4 => leap_in(x),
        6 => somersault(x, 12, 0x140, 0xd),
        8 => somersault(x, 13, -0x140, 0xe),
        10 => drop_in(x),
        12 => appear(x),
        14 => run_in(x),
        16 => wake(x),
        18 => dash(x),
        20 => get_up(x),
        _ => {}
    }
}

/// The player in range: the move is drawn at random between two animations (`a` if the draw is even, `b` otherwise).
fn pick_anim<R: Bench>(x: &mut Cx<'_, R>, a: u8, b: u8) {
    let r = rand_and(x, 1);
    x.add8(f::SUB3, 2);
    x.set8(Enemy::VULNERABLE, 1);
    let n = if r == 0 { a } else { b };
    x.set8(Kobold::LOOK, n);
    mv(x, n as u32);
}

/// Waits for a player in the box in front of the enemy, animates the wait; true when one is there.
fn player_ahead<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if !pick_target(x) {
        return false;
    }
    x.anim_step();
    lib::target_in_box(x, 0, 0x80, 0x20)
}

/// +6 = 0: howl, then advance toward the target (animations 4 or 3).
fn howl<R: Bench>(x: &mut Cx<'_, R>) {
    if grabbed(x) {
        return set_state_long(x, HELD);
    }
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            look(x, 0x17);
            mv(x, 0x17);
        }
        2 => {
            if player_ahead(x) {
                pick_anim(x, 4, 3);
            }
        }
        4 => stand_when_done(x),
        _ => {}
    }
}

/// +6 = 2: like 0 without the howl; animations 2 or 1.
fn growl<R: Bench>(x: &mut Cx<'_, R>) {
    if grabbed(x) {
        return set_state_long(x, HELD);
    }
    match x.u8(f::SUB3) {
        0 => {
            if player_ahead(x) {
                pick_anim(x, 2, 1);
            }
        }
        2 => stand_when_done(x),
        _ => {}
    }
}

/// +6 = 4: entrance leap.
fn leap_in<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(Enemy::VULNERABLE, 1);
            look(x, 6);
            x.set16(Obj::VX, -0x3c0i16 as u16);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x4c0);
            x.set16(Obj::AZ, -0x38i16 as u16);
            mv(x, 6);
        }
        2 => wait_key(x),
        4 => {
            if x.move_xz().gt() {
                x.anim_step();
                return;
            }
            x.set32(Obj::Z, 0);
            x.add8(f::SUB3, 2);
            x.set8(Enemy::VULNERABLE, 1);
            look(x, 0x18);
            mv(x, 0x18);
        }
        6 => stand_when_done(x),
        _ => {}
    }
}

/// +6 = 6 and 8: somersault, the animation key triggers the effect (0xeb92e or 0xeb9a0); `d3`: variant that goes with
/// it, `vx2` the side speed.
fn somersault<R: Bench>(x: &mut Cx<'_, R>, variant: u8, vx2: i16, anim: u32) {
    match x.u8(f::SUB3) {
        0 => {
            x.set16(Obj::VY, vx2 as u16);
            x.add8(f::SUB3, 2);
            x.set8(Enemy::VULNERABLE, 1);
            look(x, anim as u8);
            x.set16(Obj::AY, 0);
            x.set16(Obj::VZ, 0x500);
            x.set16(Obj::AZ, -0x38i16 as u16);
            mv(x, anim);
        }
        2 => {
            if x.u8(f::KEY_FRAME) != 0 {
                let special = matches!(var::SCENE.get(x.b), 6 | 0xc);
                if x.u8(Obj::VARIANT) == variant && special {
                    lib::sound(x, crate::sound::CMD_305_405);
                } else if !special {
                    lib::sound(x, crate::sound::CMD_36F_46F);
                }
                x.add8(f::SUB3, 2);
                x.set8(Enemy::VULNERABLE, 0);
            }
            x.anim_step();
        }
        4 => {
            if x.move_yz().gt() {
                x.anim_step();
                return;
            }
            x.set8(Enemy::ACTIVE, 1);
            x.set32(Obj::Z, 0);
            x.add8(f::SUB3, 2);
            x.set8(Enemy::VULNERABLE, 1);
            look(x, 0x18);
            mv(x, 0x18);
        }
        6 => stand_when_done(x),
        _ => {}
    }
}

/// +6 = 10: fall from above.
fn drop_in<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Kobold::LOOK, 0x19);
            x.set16(Obj::VZ, -0x300i16 as u16);
            x.set16(Obj::AZ, -0x10i16 as u16);
            let h = var::SCROLL_Y.get(x.b).wrapping_add(0x100).wrapping_sub(x.u16(Obj::Y));
            x.set16(Obj::Z, h);
            mv(x, 0x19);
        }
        2 => {
            x.move_z();
            if (x.u16(Obj::Z) as i16) >= 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set32(Obj::Z, 0);
            mv(x, 0x18);
        }
        4 => stand_when_done(x),
        _ => {}
    }
}

/// +6 = 12 : apparition.
fn appear<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Kobold::LOOK, 0x1a);
            x.set16(Kobold::TIMER_W, 0x400);
            mv(x, 0x1a);
        }
        2 => {
            let n = x.u8(Kobold::TIMER_W).wrapping_sub(1);
            x.set8(Kobold::TIMER_W, n);
            if n == 0 {
                x.add8(f::SUB3, 2);
                x.set8(Kobold::LOOK, 0x1b);
                mv(x, 0x1b);
            } else {
                x.anim_step();
            }
        }
        4 => stand_when_done(x),
        _ => {}
    }
}

/// +6 = 14: arrival run (0x1e frames).
fn run_in<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set16(Kobold::TIMER, 0x1e);
            x.set8(Enemy::VULNERABLE, 1);
            set_angle(x);
            mv(x, 0x16);
            set_speed(x, 0x17);
        }
        2 => {
            let n = x.u16(Kobold::TIMER).wrapping_sub(1);
            x.set16(Kobold::TIMER, n);
            if (n as i16) < 0 {
                set_state_long(x, STANCE);
            }
            x.anim_step();
            x.move_by_table_xy();
        }
        _ => {}
    }
}

/// +6 = 16: wait of a number of frames (`+0x42`) then advance.
fn wake<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            look(x, 0x17);
            let t = x.u16(Enemy::CREATOR);
            x.set16(Kobold::TIMER, t);
            mv(x, 0x17);
        }
        2 => {
            let n = x.u16(Kobold::TIMER).wrapping_sub(1);
            x.set16(Kobold::TIMER, n);
            if (n as i16) >= 0 {
                return;
            }
            x.anim_step();
            pick_anim(x, 4, 3);
        }
        4 => stand_when_done(x),
        _ => {}
    }
}

/// +6 = 18: run toward the player for a randomly drawn duration, then big attack.
fn dash<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            let r = rand_and(x, 0xf);
            let t = tables::table("kobold/dash_times").at16(x.b, 2 * r);
            x.set16(Kobold::TIMER, t);
            set_angle(x);
            mv(x, 0xc);
            set_speed(x, 0x17);
        }
        2 => {
            let n = x.u16(Kobold::TIMER).wrapping_sub(1);
            x.set16(Kobold::TIMER, n);
            if (n as i16) >= 0 {
                x.anim_step();
                x.move_by_table_xy();
                return;
            }
            x.add8(f::SUB3, 2);
            x.anim_start(SET_STRIKE.0, 6);
        }
        4 => landing_hit(x, eshl_creators::throw_1),
        6 => {
            if x.anim_step() {
                x.set32(f::STATE, 0x0408_0000);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                mv(x, 0);
            }
        }
        _ => {}
    }
}

/// +6 = 20: like 18, with another effect and a different resume.
fn get_up<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            let r = rand_and(x, 0xf);
            let t = tables::table("kobold/dash_times").at16(x.b, 2 * r);
            x.set16(Kobold::TIMER, t);
            set_angle(x);
            mv(x, 0xc);
            set_speed(x, 0x17);
        }
        2 => {
            let n = x.u16(Kobold::TIMER).wrapping_sub(1);
            x.set16(Kobold::TIMER, n);
            if (n as i16) >= 0 {
                x.anim_step();
                x.move_by_table_xy();
                return;
            }
            x.add8(f::SUB3, 2);
            x.anim_start(SET_STRIKE.0, 6);
        }
        4 => landing_hit(x, eshl_creators::throw_2),
        6 => {
            if x.anim_step() {
                x.set32(f::STATE, 0x0408_0000);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                mv(x, 0);
            }
        }
        _ => {}
    }
}

/// Animation key: effect `effect`; at the end of the script, the Kobold resumes its walk one step forward (`−0x20` to
/// the right, `+0x20` to the left).
fn landing_hit<R: Bench>(x: &mut Cx<'_, R>, throw: fn(&mut crate::world::World, crate::objects::Obj)) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        throw(x.b, x.o);
    }
    if !x.anim_step() {
        return;
    }
    mv(x, 0);
    let dx: u16 = if x.u8(f::FLIP) == 0 { 0xffe0 } else { 0x20 };
    x.add16(Obj::X, dx);
    x.add8(f::SUB3, 2);
    mv(x, 5);
}
