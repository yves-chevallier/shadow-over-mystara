//! Boss 9, état 4 : coup reçu (`+5` = 0) et engin détruit (`+5` = 2).

use crate::families::cx::Banc;
use super::moves::{restart, roll};
use super::*;
use crate::objects::Courant;

pub(super) fn run(x: &mut Cx) {
    if x.u8(f::SUB) == 0 {
        shaken(x)
    } else {
        wreck(x)
    }
}

// --- groupe 0 : coup reçu --------------------------------------------------------------------------

/// 0x8b49c : gel, puis secousse (`+6` = 0 à 4) ; remise en liste.
fn shaken(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE);
    if freeze != 0 && (freeze as i8) > 0 {
        x.set8(f::FREEZE, freeze - 1);
    } else {
        x.c.d[0] = x.u8(f::SUB2) as u32;
        match x.u8(f::SUB2) {
            0 => struck(x),
            2 => {
                x.c.d[0] = 4;
                if x.u8(SHAKE) != 0 {
                    shake(x);
                } else {
                    next6(x);
                }
            }
            _ => recoil(x),
        }
    }
    x.reinsert();
}

/// 0x8b4c2 : le coup : avaries, gel ; plus de PV : détruit ; sinon secousse et recul à l'opposé de l'attaquant.
fn struck(x: &mut Cx) {
    lib::sound(x, 0xea984);
    damage_bits(x);
    let f = x.u8(0x56);
    x.set8(f::FREEZE, f);
    if x.u16(f::HP) == 0 {
        return destroyed(x);
    }
    next6(x);
    x.set32(0xc, 0);
    x.set8(GAIT, 2);
    if sub8(x, LIVES, 1).mi() {
        x.set8(0, 2);
        x.set8(BLINK_TIME, 0x3c);
        x.set8(BLINKING, 1);
    }
    x.set8(0, 2);
    x.set8(SHAKE, 0xf);
    lib::debris_spray(x, 4);
    let away_left = x.u16(0x54) != 0 && {
        let a = x.linked(0x54);
        x.c.a[1] = a.0;
        let ax = a.u16(x.b, 0x8);
        set_dw(x, 0, ax);
        let d = x.u16(0x8).wrapping_sub(ax);
        set_dw(x, 1, d);
        (d as i16) < 0
    };
    let (vx, vz) = if away_left { ((-0x280i16) as u16, 0x28u16) } else { (0x280, (-0x28i16) as u16) };
    x.set16(0x14, vx);
    x.set16(0x16, vz);
}

/// 0x8b9ca : secousse de ±D0 (alternée, dans le sens de l'engin).
fn shake(x: &mut Cx) {
    let n = x.u8(SHAKE).wrapping_sub(1);
    x.set8(SHAKE, n);
    let mut d = x.c.d[0] as u16;
    if n & 1 != 0 {
        d = d.wrapping_neg();
    }
    if x.u8(f::FLIP) != 0 {
        d = d.wrapping_neg();
    }
    set_dw(x, 0, d);
    x.add16(0x8, d);
}

/// 0x8b55e : recul jusqu'à l'arrêt (octet fort de la vitesse nul), puis reprise de l'état mémorisé.
fn recoil(x: &mut Cx) {
    if every(x, 3) {
        smoke_pair(x, 0, 2);
    }
    if x.u8(0x14) != 0 {
        x.move_x();
        return;
    }
    if var::PAUSE.get(x.b) == 0 {
        x.set8(0, 1);
        let s = x.u32(RESUME);
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

/// Le coup est consommé (gel, dérive, coup, état mémorisé).
fn clear_hit<R: Banc>(x: &mut Cx<'_, R>) {
    x.set8(f::FREEZE, 0);
    x.set16(0x16, 0);
    x.set32(0x58, 0);
    x.set8(0x49, 0);
    x.set32(RESUME, 0);
}

/// 0x8b628 : détruit : la scène est prévenue (0x900 : boss 9 vaincu), l'équipage saute.
fn destroyed(x: &mut Cx) {
    damage_bits(x);
    count_crew(x);
    x.add8(f::SUB, 2);
    steps(x, 0, 0);
    x.set32(0x14, 0);
    x.set8(0, 2);
    crate::mem::wr16(x.b, A5_DEATH, 0x900);
    let score = x.u8(0x7e);
    set_abs8(x, A5_DEATH + 2, score);
    x.set_a5_u8(0x42, 1);
    x.set_a5_u8(0x50, 0);
    lib::flash_and_shake(x);
    for k in 0..4u32 {
        let m = x.linked(CREW + 2 * k);
        x.c.a[3] = m.0;
        if m.u8(x.b, 0) != 0 {
            m.set_u8(x.b, 0, 2);
        }
    }
    x.c.d[5] = 0xffff;
    x.c.a[2] = x.o.0 + CREW + 8;
}

/// Boss vaincu : type et variante (mot), score (`A5-0x557c..`), lus par la fin de scène.
const A5_DEATH: u32 = 0xff8000 - 0x557c;

// --- groupe 2 : détruit ------------------------------------------------------------------------------

/// 0x8b67c : l'épave sort de l'écran, revient, l'équipage est éjecté, dialogue, puis chute hors de l'écran.
fn wreck(x: &mut Cx) {
    x.set_a5_u16(0x51e, 0);
    if var::PAUSE.get(x.b) != 0 {
        return;
    }
    match x.u8(f::SUB2) {
        0 => next6(x),
        2 => leaving(x),
        4 => {
            jolt(x);
            smoke_every(x, 3);
            let edge = x.a5_u16(CAM_X).wrapping_add(0x180);
            if right_of(x, 0x80, edge) {
                next6(x);
                x.set16(0x14, (-0x500i16) as u16);
                x.set8(f::FLIP, 0);
                x.set8(0x2d, 0x18);
                lib::sound(x, 0xe4626);
            }
            roll(x);
        }
        6 => {
            jolt(x);
            let a = x.u16(0x8).wrapping_add(0x80);
            let cam = x.a5_u16(CAM_X);
            set_dw(x, 0, a.wrapping_sub(cam));
            if !Fl::sub16(a, cam).gt() {
                count_crew(x);
                if (x.a5_u16(ENEMY_FREE) as i16) >= 4 {
                    next6(x);
                    refill_crew(x, true);
                    lib::sound(x, 0xea984);
                    lib::sound(x, 0xe1330);
                }
            }
            roll(x);
        }
        8 => anim(x),
        10 => {
            next6(x);
            x.set8(f::FLIP, 0x20);
            x.set8(0x2d, 8);
            x.set16(0x10, 0x48);
            x.set16(0x14, 0x800);
            x.set16(0x16, 0);
            x.set8(GAIT, 1);
            lib::sound(x, 0xe4682);
        }
        12 => {
            jolt(x);
            smoke_every(x, 3);
            let edge = x.a5_u16(CAM_X).wrapping_add(0x180);
            let a = x.u16(0x8).wrapping_add(0x90);
            set_dw(x, 1, edge);
            set_dw(x, 0, a.wrapping_sub(edge));
            if Fl::sub16(a, edge).gt() {
                next6(x);
                let n = x.a5_u8(0x4ff).wrapping_add(4);
                x.set_a5_u8(0x4ff, n);
                crate::mem::wr16(x.b, 0x400004, 0x1355);
            }
            roll(x);
        }
        14 => {
            jolt(x);
            smoke_every(x, 3);
            if x.u16(0x8) as i16 >= 0x660 {
                // le conducteur saute
                if x.u16(DRIVER) != 0 {
                    next6(x);
                    let d = x.linked(DRIVER);
                    x.c.a[1] = d.0;
                    d.set_u32(x.b, f::STATE, 0x020c_0000);
                }
            } else {
                roll(x);
                x.set8(0x3b, 9);
            }
        }
        16 => {
            let cam = x.a5_u16(CAM_X);
            set_dw(x, 1, cam);
            if cam != 0x500 {
                anim(x);
                x.set8(0x3b, 9);
            } else {
                crate::families::effects_creators::spawn_kind(x.b, crate::families::effects_creators::kind::SOUND_FADE);
                x.set16(TIMER, 0xc0);
                if let Some(bulle) = crate::families::creators::scene_set(x.b, 8) {
                    next6(x);
                    x.set16(MESSAGE, bulle.word());
                }
            }
        }
        18 => {
            if sub16(x, TIMER, 1).mi() {
                x.set8(GAIT, 0);
            }
            if x.u16(MESSAGE) != 0 {
                let m = x.linked(MESSAGE);
                x.c.a[1] = m.0;
                if m.u8(x.b, 0x22) != 0 {
                    next6(x);
                    x.set16(0x18, (-0x600i16) as u16);
                    x.set16(0x1a, (-0x10i16) as u16);
                    x.set8(GAIT, 2);
                    lib::sound(x, 0xea9ee);
                    for off in [DRIVER, CREW, CREW + 2, CREW + 4, CREW + 6] {
                        if x.u16(off) != 0 {
                            let m = x.linked(off);
                            x.c.a[1] = m.0;
                            m.set_u32(x.b, f::STATE, 0x020c_0400);
                        }
                    }
                }
            }
        }
        _ => {
            let d = x.u16(0xc).wrapping_add(0x50).wrapping_sub(x.a5_u16(CAM_Y));
            set_dw(x, 0, d);
            if d >= 0x100 {
                x.set32(f::STATE, 0x0600_0000);
            }
            x.move_z();
            anim(x);
            x.set8(0x3b, 9);
        }
    }
    if x.u8(f::SUB2) < 8 {
        lib::collide_terrain(x);
    }
    x.reinsert();
}

/// 0x8b6c8 : l'épave part vers le bord (en remplaçant l'équipage tant qu'il en manque).
fn leaving(x: &mut Cx) {
    if x.u8(CREW_LOST) != 0 {
        x.set8(GAIT, 2);
        count_crew(x);
        if x.u8(CREW_COUNT) == 0 {
            refill_crew(x, false);
        }
        return anim(x);
    }
    smoke_every(x, 3);
    next6(x);
    x.set16(0x14, 0x500);
    x.set16(0x16, 0);
    x.set8(GAIT, 0);
    if x.u8(f::FLIP) == 0 {
        next6(x);
        negate(x, 0x14);
        lib::sound(x, 0xe4626);
    } else {
        lib::sound(x, 0xe45ca);
        lib::sound(x, 0xea8be);
    }
    roll(x)
}
