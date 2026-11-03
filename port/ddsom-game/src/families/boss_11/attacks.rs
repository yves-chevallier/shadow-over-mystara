//! Boss 11/12, group 4: the eight attacks (`+6` = 0 to 14), steps `+7` = 0, 2, 4...

use crate::families::cx::Bench;
use super::behave::life_rank;
use super::*;
use crate::objects::Current;
use crate::machine::Machine;


pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => strike(x),
        2 => spit(x),
        4 => leap(x),
        6 => lunge(x),
        8 => charge(x),
        0xa => approach(x),
        0xc => spit_c(x),
        _ => leap_c(x),
    }
}

/// 0x8dc30: tail hit (the effect depends on the type).
fn sting<R: Bench>(x: &mut Cx<'_, R>) {
    if is_12(x) {
        lib::sound(x, crate::sound::CMD_387_487)
    } else {
        lib::sound(x, crate::sound::CMD_386_486)
    }
}

/// 0x8dbee (`+6` = 0): a hit.
fn strike<R: Bench>(x: &mut Cx<'_, R>) {
    set44(x, 1);
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        act(x, 1);
        return;
    }
    if x.anim_step() {
        return to_chase(x);
    }
    sync(x);
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        sting(x);
    }
}

/// 0x8dc44 (`+6` = 2): spit (types 11 and 12: different projectiles).
fn spit<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set16(Boss11::TIMER, 0x10);
            act(x, 0)
        }
        2 => {
            x.anim_step();
            sync(x);
            if sub16(x, Boss11::TIMER, 1).gt() {
                return;
            }
            x.add8(f::SUB3, 2);
            act(x, 0x24)
        }
        4 => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                act(x, 0x25);
                return lib::sound(x, crate::sound::CMD_411);
            }
            sync(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.set8(f::KEY_FRAME, 0);
            let rng = x.random();
            if is_12(x) {
                let r = rng & 3;
                setw(x, 0, r);
                if r == 0 {
                    return crate::families::eshl_creators::fragments_45(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
                }
                return crate::families::eshl_creators::boss11_shards(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
            }
            let r = rng & 7;
            setw(x, 0, r);
            crate::families::eshl_creators::fragments_42(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o, r)
        }
        6 => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                return act(x, 0x26);
            }
            sync(x);
        }
        _ => {
            if x.anim_step() {
                return to_chase(x);
            }
            sync(x);
        }
    }
}

/// Orientation toward the camera: D1b = 0 or 0x20, D2w = ±speed (the head jumps toward the center of the screen).
fn jump_side<R: Bench>(x: &mut Cx<'_, R>, speed: i16, z: u16, rise: u16) {
    stop(x);
    let mut d0 = cam_x(x);
    let mut side = 0;
    let mut d2 = speed as u16;
    d0 = d0.wrapping_add(0xe0);
    if !Fl::sub16(d0, x.u16(Obj::X)).le() {
        side = 0x20;
        d2 = d2.wrapping_neg();
    }
    x.set8(f::FLIP, side);
    x.set16(Enemy::VX, d2);
    x.set16(Enemy::VZ, z);
    x.set16(Enemy::GRAVITY, rise);
    body(x, 5);
}

/// Landing from a leap: the height becomes zero again, the snake stops moving (noise and dust).
fn landing<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.anim_step();
    sync(x);
    x.move_xz();
    if !tst16(x, Obj::Z).mi() {
        return false;
    }
    x.add8(f::SUB3, 2);
    x.set16(Obj::Z, 0);
    body(x, 7);
    lib::shake_58(x, 8);
    lib::sound(x, crate::sound::CMD_328_428_B);
    true
}

/// 0x8dd1c (`+6` = 4): leap onto the opponent.
fn leap<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss11::ATTACKING, 1);
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            jump_side(x, -0x400, 0x600, (-0x40i16) as u16);
            lib::sound(x, crate::sound::CMD_385_485)
        }
        2 => {
            landing(x);
        }
        _ => {
            if !x.anim_step() {
                return to_chase(x);
            }
            sync(x);
        }
    }
}

/// 0x8e2ea (`+6` = 14): longer leap (speed -0x100).
fn leap_c<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss11::ATTACKING, 1);
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            jump_side(x, -0x100, 0x800, (-0x80i16) as u16);
            lib::sound(x, crate::sound::CMD_385_485)
        }
        2 => {
            landing(x);
        }
        _ => {
            if !x.anim_step() {
                return to_chase(x);
            }
            sync(x);
        }
    }
}

/// 0x8ddcc (`+6` = 6): run-up then fall.
fn lunge<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            stop(x);
            x.set16(Enemy::VZ, 0x200);
            x.set16(Enemy::GRAVITY, (-0x80i16) as u16);
            x.set16(Boss11::TIMER, 0x20);
            act(x, 0x20);
            sync(x);
            lib::sound(x, crate::sound::CMD_385_485)
        }
        2 => {
            x.anim_step();
            sync(x);
            if x.u16(Enemy::GRAVITY) == 0 {
                if sub16(x, Boss11::TIMER, 1).gt() {
                    return;
                }
                x.add8(f::SUB3, 2);
                x.set16(Obj::Z, 0);
                return act(x, 0x22);
            }
            x.move_xz();
            if !tst16(x, Obj::Z).mi() {
                return;
            }
            x.set16(Obj::Z, 0);
            stop(x)
        }
        _ => {
            x.set8(Boss11::ATTACKING, 1);
            if !x.anim_step() {
                return to_chase(x);
            }
            sync(x);
        }
    }
}

/// 0x8de64 (`+6` = 8): charge at a player (heading taken from the table at 0x1376).
fn charge<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss11::ATTACKING, 1);
    let mut t = x.u8(f::SUB3);
    if t == 0 {
        x.add8(f::SUB3, 2);
        act(x, 0x27);
        sync(x);
        x.set16(Boss11::TIMER, 0x20);
        t = 2;
    }
    if t == 2 {
        x.anim_step();
        sync(x);
        if sub16(x, Boss11::TIMER, 1).gt() {
            return;
        }
        x.add8(f::SUB3, 2);
        t = 4;
    }
    match t {
        4 => {
            x.add8(f::SUB3, 2);
            seg_choice(x);
            if !aim_target(x) {
                return to_chase(x);
            }
            let d0 = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(Enemy::HEADING, d0 as u8);
            stop(x);
            speed(x, 0x34);
            lib::sound(x, crate::sound::CMD_385_485)
        }
        6 => {
            if !sub8(x, Boss11::AIM_PAUSE, 1).gt() {
                if sub8(x, Boss11::CHARGE_LEFT, 1).le() {
                    return charge_end(x);
                }
                if !aim_target(x) {
                    return charge_end(x);
                }
                pause_tick(x);
                lib::sound(x, crate::sound::CMD_385_485);
            }
            // D0: `moveq #0`, then the low byte = the gap
            let diff = x.u8(Boss11::AIM).wrapping_sub(x.u8(Enemy::HEADING));
            if diff != 0 {
                let mut w = diff as u16 & 0x1f;
                if !Fl::sub16(w, 0x10).lt() {
                    w = w.wrapping_sub(0x20);
                }
                let cur = x.u8(Enemy::HEADING).wrapping_sub(w as u8) & 0x1f;
                x.set8(Enemy::HEADING, cur);
            }
            x.move_by_table_xy();
            x.anim_step();
            sync(x);
        }
        _ => {
            x.move_xy();
            if x.anim_step() {
                to_chase(x);
            }
        }
    }
}

/// 0x8df38: end of the charge, braking.
fn charge_end<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    let a = asr_w(x.u16(Enemy::VX), 2);
    x.set16(Enemy::VX, a);
    let b = asr_w(x.u16(Enemy::VY), 2);
    x.set16(Enemy::VY, b);
    act(x, 0x29);
    sync(x);
}

/// 0x8df70: picks a target (0x1376 gives the heading); false with no player.
fn aim_target<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let Some(target) = lib::target_by_criterion(x, lib::LOWEST_HP) else {
        return false;
    };
    let a = lib::aim_to(x, target);
    x.set8(Boss11::AIM, a);
    true
}

/// 0x8df8e: duration of the charge (`+0xb0`, table 0x8dfcc according to life rank) then `pause_tick`.
fn seg_choice<R: Bench>(x: &mut Cx<'_, R>) {
    let i = life_rank(x) & 0x30;
    let rng = x.random();
    let r = rng & 0xf;
    setw(x, 0, r);
    let v = byte(x, "boss_11/charge_lengths", i as u32 + r as u32);
    x.set8(Boss11::CHARGE_LEFT, v);
    pause_tick(x)
}

/// 0x8dfaa: step between two aimings (`+0xb1`, table 0x8dfbc).
fn pause_tick<R: Bench>(x: &mut Cx<'_, R>) {
    let rng = x.random();
    let r = rng & 0xf;
    let v = byte(x, "boss_11/aim_pauses", r as u32);
    x.set8(Boss11::AIM_PAUSE, v);
}

/// 0x8e00c (`+6` = 10): approach following a path (table 0x8e0f8).
fn approach<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss11::ATTACKING, 1);
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            stop(x);
            speed(x, 0x2b);
            if !route_start(x) {
                return to_chase(x);
            }
            act(x, 0x27);
            sync(x);
        }
        2 => {
            if sub16(x, Boss11::TIMER, 1).mi() && !route_step(x) {
                x.add8(f::SUB3, 2);
                let a = asr_w(x.u16(Enemy::VX), 2);
                x.set16(Enemy::VX, a);
                let b = asr_w(x.u16(Enemy::VY), 2);
                x.set16(Enemy::VY, b);
                act(x, 0x29);
                return sync(x);
            }
            x.move_by_table_xy();
            x.anim_step();
            sync(x);
        }
        _ => {
            if x.anim_step() {
                to_chase(x);
            }
        }
    }
}

/// 0x8e0a2: path drawn according to the distance to the camera (three identical tables in the ROM).
fn route_start<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let d1 = cam_x(x);
    let d0 = x.u16(Obj::X).wrapping_sub(d1);
    setw(x, 0, d0);
    let rng = x.random();
    let r = rng & 0xf;
    setw(x, 0, r);
    let v = byte(x, "boss_11/routes", r as u32);
    setb(x, 0, v);
    x.set8(Boss11::ROUTE, v);
    x.set8(Boss11::ROUTE_POINT, 0);
    x.set16(Boss11::TIMER, 0);
    route_step(x)
}

/// 0x8e0e8: next point of the path; true if there is path left (the step's duration goes to `+0xb0`).
fn route_step<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let route = x.u8(Boss11::ROUTE) as u32;
    let a1 = jump_target(x, "boss_11/route_paths", route);
    let point = x.u8(Boss11::ROUTE_POINT) as u16;
    let count = tables::held16(x.b, a1.wrapping_sub(2));
    if Fl::sub16(point, count).gt() {
        return false;
    }
    addq8(x, Boss11::ROUTE_POINT, 1);
    let i = point.wrapping_mul(4);
    let a1 = a1.wrapping_add(i as i16 as i32 as u32);
    let d1 = cam_x(x).wrapping_add(tables::held16(x.b, a1));
    let d2 = cam_y(x).wrapping_add(tables::held16(x.b, a1 + 2));
    // D2.w, D3.w and D4.w: leftovers for the following handlers (`dispatch::Leftovers`)
    setw(x, 2, d2);
    let (px, py) = (x.u16(Obj::X), x.u16(Obj::Y));
    setw(x, 3, px);
    setw(x, 4, py);
    let angle = lib::aim(x, (px, py), (d1, d2));
    x.set8(Enemy::HEADING, angle);
    let i = (angle as u16).wrapping_mul(4);
    let a2 = x.u32(Obj::SPEED_TABLE);
    let mut v = tables::speed16(x.b, a2.wrapping_add(i as i16 as i32 as u32));
    setw(x, 0, v);
    let (mut d0, mut d1l): (u32, u32);
    if v != 0 {
        if (v as i16) < 0 {
            v = v.wrapping_neg();
        }
        d0 = (v as u32) << 16;
        let mut w = cam_x(x).wrapping_add(tables::held16(x.b, a1)).wrapping_sub(x.u16(Obj::X));
        if (w as i16) < 0 {
            w = w.wrapping_neg();
        }
        d1l = (w as u32) << 16;
    } else {
        v = tables::speed16(x.b, a2.wrapping_add(2));
        setw(x, 0, v);
        if v == 0 {
            return false;
        }
        if (v as i16) < 0 {
            v = v.wrapping_neg();
        }
        d0 = (v as u32) << 16;
        let mut w = cam_y(x).wrapping_add(tables::held16(x.b, a1 + 2)).wrapping_sub(x.u16(Obj::Y));
        if (w as i16) < 0 {
            w = w.wrapping_neg();
        }
        d1l = (w as u32) << 16;
    }
    // the duration: how many times the speed fits in the gap (hundreds, sixteenths, units)
    let mut d2: u16 = 0;
    let mut d3;
    loop {
        d3 = d1l;
        d1l = d1l.wrapping_sub(d0);
        if (d1l as i32) < 0 {
            break;
        }
        d2 = d2.wrapping_add(0x100);
    }
    d1l = d3;
    d0 = ((d0 as i32) >> 4) as u32;
    if d0 != 0 {
        loop {
            d3 = d1l;
            d1l = d1l.wrapping_sub(d0);
            if (d1l as i32) < 0 {
                break;
            }
            d2 = d2.wrapping_add(0x10);
        }
        // the units: high words (`swap`) of the remainder and of the speed
        let w = asr_w((d0 >> 16) as u16, 4);
        if w != 0 {
            let mut d1w = (d3 >> 16) as u16;
            loop {
                let r = Fl::sub16(d1w, w);
                d1w = d1w.wrapping_sub(w);
                if r.mi() {
                    break;
                }
                d2 = d2.wrapping_add(1);
            }
        }
    }
    x.set16(Boss11::TIMER, d2);
    true
}

/// 0x8e20a (`+6` = 12, type 12 only): spit of a homing projectile.
fn spit_c<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            if !is_12(x) {
                return to_chase(x);
            }
            x.add8(f::SUB3, 2);
            x.set16(Boss11::TIMER, 0x10);
            act(x, 0)
        }
        2 => {
            x.anim_step();
            sync(x);
            if sub16(x, Boss11::TIMER, 1).gt() {
                return;
            }
            x.add8(f::SUB3, 2);
            act(x, 0x24)
        }
        4 => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                act(x, 0x25);
                return lib::sound(x, crate::sound::CMD_411);
            }
            sync(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.set8(f::KEY_FRAME, 0);
            x.set16(Enemy::SPAWNER, 0);
            let Some(column) = crate::families::effects_creators::eshl_rise(x.b, x.o) else { return };
            x.set16(Enemy::SPAWNER, column.word());
        }
        6 => {
            if !x.anim_step() {
                return sync(x);
            }
            x.add8(f::SUB3, 2);
            let w = x.u16(Enemy::SPAWNER);
            if (w as i16) < 0 {
                let a1 = w as i16 as i32 as u32;
                if rd8(x.b, a1) != 0 && rd8(x.b, a1 + 2) == 0x47 {
                    wr8(x.b, a1 + 0x58, 1);
                }
            }
            act(x, 0x26)
        }
        _ => {
            if x.anim_step() {
                return to_chase(x);
            }
            sync(x);
        }
    }
}
