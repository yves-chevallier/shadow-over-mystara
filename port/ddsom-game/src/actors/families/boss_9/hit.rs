//! Boss 9, state 4: hit received (`+5` = 0) and vehicle destroyed (`+5` = 2).

use crate::actors::families::cx::Bench;
use super::moves::{restart, roll};
use super::*;
use crate::world::objects::Current;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB) == 0 {
        shaken(x)
    } else {
        wreck(x)
    }
}

// --- group 0: hit received --------------------------------------------------------------------------

/// 0x8b49c: freeze, then shake (`+6` = 0 to 4); put back in the list.
fn shaken<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE);
    if freeze != 0 && (freeze as i8) > 0 {
        x.set8(f::FREEZE, freeze - 1);
    } else {
        match x.u8(f::SUB2) {
            0 => struck(x),
            2 => {
                if x.u8(Boss9::SHAKE) != 0 {
                    shake(x, 4);
                } else {
                    next6(x);
                }
            }
            _ => recoil(x),
        }
    }
    x.reinsert();
}

/// 0x8b4c2: the hit: damage, freeze; out of HP: destroyed; otherwise shake and knockback away from the attacker.
fn struck<R: Bench>(x: &mut Cx<'_, R>) {
    lib::sound(x, crate::hw::sound::CMD_94);
    damage_bits(x);
    let f = x.u8(Enemy::STUN);
    x.set8(f::FREEZE, f);
    if x.u16(f::HP) == 0 {
        return destroyed(x);
    }
    next6(x);
    x.set32(Obj::Z, 0);
    x.set8(Boss9::GAIT, 2);
    if sub8(x, Boss9::LIVES, 1).mi() {
        x.set8(Enemy::ACTIVE, 2);
        x.set8(Boss9::BLINK_TIME, 0x3c);
        x.set8(Boss9::BLINKING, 1);
    }
    x.set8(Enemy::ACTIVE, 2);
    x.set8(Boss9::SHAKE, 0xf);
    lib::debris_spray(x, 4);
    let away_left = x.u16(Enemy::HOLDER) != 0 && {
        let a = x.linked(Enemy::HOLDER);
        let ax = a.u16(x.b, Obj::X);
        let d = x.u16(Obj::X).wrapping_sub(ax);
        (d as i16) < 0
    };
    let (vx, vz) = if away_left { ((-0x280i16) as u16, 0x28u16) } else { (0x280, (-0x28i16) as u16) };
    x.set16(Enemy::VX, vx);
    x.set16(Enemy::AX, vz);
}

/// 0x8b9ca: shake of ±`amplitude` (D0 of the original; alternating, in the vehicle's direction).
fn shake<R: Bench>(x: &mut Cx<'_, R>, amplitude: u16) {
    let n = x.u8(Boss9::SHAKE).wrapping_sub(1);
    x.set8(Boss9::SHAKE, n);
    let mut d = amplitude;
    if n & 1 != 0 {
        d = d.wrapping_neg();
    }
    if x.u8(f::FLIP) != 0 {
        d = d.wrapping_neg();
    }
    x.add16(Obj::X, d);
}

/// 0x8b55e: knockback until it stops (high byte of the speed zero), then resumption of the saved state.
fn recoil<R: Bench>(x: &mut Cx<'_, R>) {
    if every(x, 3) {
        smoke_pair(x, 0, 2);
    }
    if x.u8(Enemy::VX) != 0 {
        x.move_x();
        return;
    }
    if var::PAUSE.get(x.b) == 0 {
        x.set8(Enemy::ACTIVE, 1);
        let s = x.u32(Boss9::RESUME);
        x.set32(f::STATE, s);
        if s == 0 {
            x.set32(f::STATE, 0x0202_0000);
            clear_hit(x);
            return restart(x);
        }
        if x.u8(f::STATE) == 4 {
            damage_bits(x);
            x.set32(f::STATE, 0x0204_0002);
        } else if (x.u8(f::SUB2) as i8) <= 2 {
            steps(x, 0, 2);
        }
        clear_hit(x);
    }
    count_crew(x);
}

/// The hit is consumed (freeze, drift, hit, saved state).
fn clear_hit<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(f::FREEZE, 0);
    x.set16(Enemy::AX, 0);
    x.set32(Enemy::HIT_LOCK, 0);
    x.set8(Enemy::REACTION, 0);
    x.set32(Boss9::RESUME, 0);
}

/// 0x8b628: destroyed: the stage is notified (0x900: boss 9 defeated), the crew jumps.
fn destroyed<R: Bench>(x: &mut Cx<'_, R>) {
    damage_bits(x);
    count_crew(x);
    x.add8(f::SUB, 2);
    steps(x, 0, 0);
    x.set32(Enemy::VX_LONG, 0);
    x.set8(Enemy::ACTIVE, 2);
    record_defeat(x, 9);
    var::PLAYERS_FROZEN.set(x.b, 1);
    var::BOSS_FIGHT.set(x.b, 0);
    lib::flash_and_shake(x);
    for k in 0..4u32 {
        let m = x.linked(Boss9::CREW + 2 * k);
        if m.u8(x.b, Enemy::ACTIVE) != 0 {
            m.set_u8(x.b, Enemy::ACTIVE, 2);
        }
    }
    *x.c.d5() = 0xffff;
}


// --- group 2: destroyed ------------------------------------------------------------------------------

/// 0x8b67c: the wreck leaves the screen, comes back, the crew is ejected, dialogue, then falls off the screen.
fn wreck<R: Bench>(x: &mut Cx<'_, R>) {
    var::CAMERA_MODE.set(x.b, 0);
    if var::PAUSE.get(x.b) != 0 {
        return;
    }
    match x.u8(f::SUB2) {
        0 => next6(x),
        2 => leaving(x),
        4 => {
            jolt(x);
            smoke_every(x, 3);
            let edge = var::CAMERA_ALT_X.get(x.b).wrapping_add(0x180);
            if right_of(x, 0x80, edge) {
                next6(x);
                x.set16(Enemy::VX, (-0x500i16) as u16);
                x.set8(f::FLIP, 0);
                x.set8(Enemy::HEADING, 0x18);
                lib::sound(x, crate::hw::sound::CMD_92_B);
            }
            roll(x);
        }
        6 => {
            jolt(x);
            let a = x.u16(Obj::X).wrapping_add(0x80);
            let cam = var::CAMERA_ALT_X.get(x.b);
            if !Fl::sub16(a, cam).gt() {
                count_crew(x);
                if (pools::ENEMY.free_count(x.b) as i16) >= 4 {
                    next6(x);
                    refill_crew(x, true);
                    lib::sound(x, crate::hw::sound::CMD_94);
                    lib::sound(x, crate::hw::sound::CMD_20);
                }
            }
            roll(x);
        }
        8 => anim(x),
        10 => {
            next6(x);
            x.set8(f::FLIP, 0x20);
            x.set8(Enemy::HEADING, 8);
            x.set16(Obj::Y, 0x48);
            x.set16(Enemy::VX, 0x800);
            x.set16(Enemy::AX, 0);
            x.set8(Boss9::GAIT, 1);
            lib::sound(x, crate::hw::sound::CMD_92_C);
        }
        12 => {
            jolt(x);
            smoke_every(x, 3);
            let edge = var::CAMERA_ALT_X.get(x.b).wrapping_add(0x180);
            let a = x.u16(Obj::X).wrapping_add(0x90);
            if Fl::sub16(a, edge).gt() {
                next6(x);
                let n = var::SCENE_STEP.get(x.b).wrapping_add(4);
                var::SCENE_STEP.set(x.b, n);
                // video control word of the output port (layer priorities), as at the end of a scene
                crate::hw::video::regs::OBJECT_OUTPUT.set(x.b, 0x1355);
            }
            roll(x);
        }
        14 => {
            jolt(x);
            smoke_every(x, 3);
            if x.u16(Obj::X) as i16 >= 0x660 {
                // the driver jumps
                if x.u16(Boss9::DRIVER) != 0 {
                    next6(x);
                    let d = x.linked(Boss9::DRIVER);
                    d.set_u32(x.b, f::STATE, 0x020c_0000);
                }
            } else {
                roll(x);
                x.set8(Enemy::GROUND_MARK, 9);
            }
        }
        16 => {
            let cam = var::CAMERA_ALT_X.get(x.b);
            if cam != 0x500 {
                anim(x);
                x.set8(Enemy::GROUND_MARK, 9);
            } else {
                crate::actors::families::effects_creators::spawn_kind(x.b, crate::actors::families::effects_creators::kind::SOUND_FADE);
                x.set16(Boss9::TIMER, 0xc0);
                if let Some(bubble) = crate::actors::families::creators::scene_set(x.b, 8) {
                    next6(x);
                    x.set16(Boss9::MESSAGE, bubble.word());
                }
            }
        }
        18 => {
            if sub16(x, Boss9::TIMER, 1).mi() {
                x.set8(Boss9::GAIT, 0);
            }
            if x.u16(Boss9::MESSAGE) != 0 {
                let m = x.linked(Boss9::MESSAGE);
                if Message(m).done(x.b) != 0 {
                    next6(x);
                    x.set16(Enemy::VZ, (-0x600i16) as u16);
                    x.set16(Enemy::GRAVITY, (-0x10i16) as u16);
                    x.set8(Boss9::GAIT, 2);
                    lib::sound(x, crate::hw::sound::CMD_95);
                    for off in [Boss9::DRIVER, Boss9::CREW, Boss9::CREW + 2, Boss9::CREW + 4, Boss9::CREW + 6] {
                        if x.u16(off) != 0 {
                            let m = x.linked(off);
                            m.set_u32(x.b, f::STATE, 0x020c_0400);
                        }
                    }
                }
            }
        }
        _ => {
            let d = x.u16(Obj::Z).wrapping_add(0x50).wrapping_sub(var::CAMERA_ALT_Y.get(x.b));
            if d >= 0x100 {
                x.set32(f::STATE, 0x0600_0000);
            }
            x.move_z();
            anim(x);
            x.set8(Enemy::GROUND_MARK, 9);
        }
    }
    if x.u8(f::SUB2) < 8 {
        lib::collide_terrain(x);
    }
    x.reinsert();
}

/// 0x8b6c8: the wreck heads for the edge (replacing the crew as long as some are missing).
fn leaving<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Boss9::CREW_LOST) != 0 {
        x.set8(Boss9::GAIT, 2);
        count_crew(x);
        if x.u8(Boss9::CREW_COUNT) == 0 {
            refill_crew(x, false);
        }
        return anim(x);
    }
    smoke_every(x, 3);
    next6(x);
    x.set16(Enemy::VX, 0x500);
    x.set16(Enemy::AX, 0);
    x.set8(Boss9::GAIT, 0);
    if x.u8(f::FLIP) == 0 {
        next6(x);
        negate(x, 0x14);
        lib::sound(x, crate::hw::sound::CMD_92_B);
    } else {
        lib::sound(x, crate::hw::sound::CMD_92);
        lib::sound(x, crate::hw::sound::CMD_92_D);
    }
    roll(x)
}
