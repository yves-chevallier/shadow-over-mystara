//! Type 11, sous-états 6 (gestes de repli : bonds, accroupi, sortie d'écran) et 8 (attente, bond latéral).
//! Chacun commence par le drapeau de coup `+0x8f` (`hit_gate`).

use super::attacks::squat_begin;
use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

pub(super) fn run_c(x: &mut Cx) {
    if !hit_gate(x) {
        return;
    }
    let step = x.u8(f::SUB3);
    match x.u8(f::SUB2) {
        0 => {
            if step == 0 {
                x.add8(f::SUB3, 2);
                x.set16(TIMER, 0x10);
                return mv(x, 0);
            }
            countdown_stance(x);
        }
        2 => back_hop(x, step),
        4 => forward_hop(x, step),
        6 => punch_hop(x, step),
        8 => squat_again(x, step),
        10 => leave(x),
        _ => {}
    }
}

/// 0x68080 : minuterie `+0xa2` puis debout.
fn countdown_stance(x: &mut Cx) {
    let n = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, n);
    if n == 0 {
        set_state_long(x, STANCE);
    }
}

/// 0x680ee : attend la clé d'animation.
fn wait_key(x: &mut Cx) {
    x.anim_step();
    if x.u8(f::KEY_FRAME) == 0 {
        return;
    }
    x.add8(f::SUB3, 2);
}

/// 0x68146 : attend la fin de l'animation, puis debout.
fn end_stance(x: &mut Cx) {
    x.anim_step();
    if (x.u16(ANIM_END) as i16) < 0 {
        set_state_long(x, STANCE);
    }
}

/// +6 = 2 : bond en arrière (vitesses latérales ±0x200).
fn back_hop(x: &mut Cx, step: u8) {
    match step {
        0 => next_anim(x),
        2 => wait(x),
        4 => {
            if x.u8(GROUP) != 0 {
                x.set16(0xa8, 0);
                x.set8(f::SUB2, 8);
                return squat_begin(x);
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::VY, 0x200);
            if rand_and(x, 1) != 0 {
                negate(x, Obj::VY);
            }
            x.set16(Obj::AY, 0);
            x.set16(Obj::VZ, 0x400);
            x.set16(Obj::AZ, -0x80i16 as u16);
            mv(x, 3);
        }
        6 => wait_key(x),
        8 => {
            x.anim_step();
            x.move_yz();
            if (x.u16(Obj::VZ) as i16) < 0 {
                x.add8(f::SUB3, 2);
                x.set16(ANIM_TIME, 1);
            }
        }
        10 => {
            x.anim_step();
            x.move_yz();
            if x.u32(Obj::Z) as i32 > 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set32(Obj::Z, 0);
            x.set16(ANIM_TIME, 1);
        }
        12 => end_stance(x),
        _ => {}
    }
}

/// +6 = 4 : bond en avant (vitesse ±0x200).
fn forward_hop(x: &mut Cx, step: u8) {
    match step {
        0 => next_anim(x),
        2 => wait(x),
        4 => {
            if x.u8(GROUP) != 0 {
                x.set8(f::SUB2, 8);
                let r = rand_and(x, 1);
                x.set8(0xa9, (r * 2) as u8);
                x.set8(0xa8, 0xff);
                return squat_begin(x);
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::VX, 0x200);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x800);
            x.set16(Obj::AZ, -0x80i16 as u16);
            mv(x, 3);
        }
        6 => wait_key(x),
        8 => {
            x.anim_step();
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) < 0 {
                x.add8(f::SUB3, 2);
                x.set16(ANIM_TIME, 1);
            }
        }
        10 => {
            x.anim_step();
            x.move_xz();
            if x.u32(Obj::Z) as i32 > 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set32(Obj::Z, 0);
            x.set16(ANIM_TIME, 1);
        }
        12 => end_stance(x),
        _ => {}
    }
}

/// +6 = 6 : bond avec coup de poing en l'air.
fn punch_hop(x: &mut Cx, step: u8) {
    match step {
        0 => next_anim(x),
        2 => wait(x),
        4 => {
            x.add8(f::SUB3, 2);
            x.set16(Obj::VX, 0x200);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x800);
            x.set16(Obj::AZ, -0x80i16 as u16);
            strike(x, 2);
        }
        6 => {
            x.anim_step();
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.add8(f::SUB3, 2);
        }
        8 => {
            x.move_xz();
            x.anim_step();
            if (x.u16(Obj::VZ) as i16) < 0 {
                x.add8(f::SUB3, 2);
                x.set16(ANIM_TIME, 1);
                x.set8(f::KEY_FRAME, 0);
            }
        }
        10 => {
            x.move_xz();
            x.anim_step();
            if x.u32(Obj::Z) as i32 > 0 {
                return;
            }
            x.set32(Obj::Z, 0);
            x.set16(ANIM_TIME, 1);
            x.add8(f::SUB3, 2);
        }
        12 => {
            x.anim_step();
            if (x.u8(ANIM_END) as i8) < 0 {
                set_state_long(x, 0x1ed2a0);
            }
        }
        _ => {}
    }
}

/// +6 = 8 : accroupi (tirage ou profil), puis retour debout.
fn squat_again(x: &mut Cx, step: u8) {
    match step {
        0 => next_anim(x),
        2 => wait(x),
        4 => {
            let r = rand_and(x, 1);
            x.set8(0xa9, (r * 2) as u8);
            x.set8(0xa8, 0xff);
            squat_begin(x);
        }
        6 => {
            if x.u8(f::KEY_FRAME) != 0 {
                let n = x.u16(TIMER).wrapping_sub(1);
                x.set16(TIMER, n);
                if (n as i16) >= 0 {
                    return;
                }
                x.add8(f::SUB3, 2);
                x.set16(ANIM_TIME, 1);
                let group = x.u8(GROUP) as i8 as i32 as u32;
                let v = x.rom8(0x682e8u32.wrapping_add(group));
                x.set8(0x68, v);
            }
            x.anim_step();
        }
        8 => end_stance(x),
        _ => {}
    }
}

/// +6 = 10 : sortie d'écran à pied (vitesse −0x230 vers le bord le plus proche), puis disparition.
fn leave(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.set16(Obj::VX, -0x230i16 as u16);
        x.set8(f::FLIP, 0);
        let d = var::SCROLL_X.get(x.b).wrapping_sub(x.u16(Obj::X));
        if (d as i16) >= 0xc0 {
            // `blt` non pris : à droite du bord, il part dans l'autre sens
            negate(x, Obj::VX);
            x.set8(f::FLIP, 0x20);
        }
        x.add8(f::SUB3, 2);
        return mv(x, 0);
    }
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        x.move_x();
    }
    x.anim_step();
    let d = x.u16(Obj::X).wrapping_sub(var::SCROLL_X.get(x.b)).wrapping_add(0x10);
    if d > 0x1a0 {
        detach_from_host(x);
        set_state_long(x, st::DEAD_LONG);
    }
}

// --- sous-état 8 --------------------------------------------------------------------------------------

pub(super) fn run_d(x: &mut Cx) {
    if !hit_gate(x) {
        return;
    }
    let step = x.u8(f::SUB3);
    match x.u8(f::SUB2) {
        0 | 6 => {
            if step == 0 {
                x.add8(f::SUB3, 2);
                x.set16(TIMER, 0x1e);
                return mv(x, 0);
            }
            countdown_stance(x);
        }
        2 => match step {
            0 => {
                x.add8(f::SUB3, 2);
                restart_walk(x);
                x.set16(TIMER, 0x20);
            }
            2 => {
                x.anim_step();
                if (x.u8(f::KEY_FRAME) as i8) < 0 {
                    return;
                }
                let n = x.u16(TIMER).wrapping_sub(1);
                x.set16(TIMER, n);
                if n != 0 {
                    x.move_by_table_xy();
                    let d = x.u16(TIMER2).wrapping_sub(x.u16(Obj::X)).wrapping_add(0x30);
                    if d < 0x60 {
                        return;
                    }
                }
                set_state_long(x, STANCE);
            }
            _ => {}
        },
        4 => sidestep(x, step),
        _ => {}
    }
}

/// +6 = 4 : pas latéral en profondeur ou bond latéral (au hasard).
fn sidestep(x: &mut Cx, step: u8) {
    match step {
        0 => {
            x.add8(f::SUB3, 2);
            let mut angle = 0u8;
            if x.u16(f::TARGET) != 0 {
                let target = load_target(x);
                if x.u16(Obj::Y) < target.u16(x.b, Obj::Y) {
                    angle = 0x10;
                }
            }
            x.set8(Obj::SPEED_INDEX, angle);
            if rand_and(x, 1) != 0 {
                // bond latéral
                x.set8(f::SUB3, 4);
                x.set16(Obj::VY, 0x300);
                if x.u8(Obj::SPEED_INDEX) != 0 {
                    negate(x, Obj::VY);
                }
                x.set16(Obj::AY, 0);
                x.set16(Obj::VZ, 0x400);
                x.set16(Obj::AZ, -0x80i16 as u16);
                return mv(x, 3);
            }
            x.set16(TIMER, 0x16);
            let depth = x.u16(Obj::Y);
            x.set16(TIMER2, depth);
            x.set32(0x4c, 0x1c74b6);
            mv(x, 0);
        }
        2 => {
            x.anim_step();
            if (x.u8(f::KEY_FRAME) as i8) < 0 {
                return;
            }
            let n = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, n);
            if n != 0 {
                x.move_by_table_xy();
                let d = x.u16(TIMER2).wrapping_sub(x.u16(Obj::Y)).wrapping_add(0x20);
                if d < 0x40 {
                    return;
                }
            }
            set_state_long(x, STANCE);
        }
        4 => wait_key(x),
        6 => {
            x.anim_step();
            x.move_yz();
            if (x.u16(Obj::VZ) as i16) < 0 {
                x.add8(f::SUB3, 2);
                x.set16(ANIM_TIME, 1);
            }
        }
        8 => {
            x.anim_step();
            x.move_yz();
            if x.u32(Obj::Z) as i32 > 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set32(Obj::Z, 0);
            x.set16(ANIM_TIME, 1);
        }
        10 => end_stance(x),
        _ => {}
    }
}
