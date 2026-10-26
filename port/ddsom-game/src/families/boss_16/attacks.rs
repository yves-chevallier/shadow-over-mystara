//! Boss 16, groupe 4 : les huit attaques (`+6` = 0 à 14). Chacune finit par `next_attack` (0x94e24).

use super::*;

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => crawl_leap(x),
        2 => charge(x),
        4 => tail_flame(x),
        6 => sweep_a(x),
        8 | 10 => sweep_b(x),
        12 => roar(x),
        14 => double_jump(x),
        _ => {}
    }
    if x.a5_u8(var::PLAYERS_ALIVE) == 0 {
        x.set32(f::STATE, state::IDLE);
    }
}

/// 0xb59a6 / 0xb59c8 : un EFFCT 0x30 (variante 1 ou 2) lié au boss (`+0x46`).
fn spawn_trail(x: &mut Cx, variant: u8) {
    if let Some(e) = try_alloc(x, call::ALLOC_EFFCT) {
        e.set_u8(x.b, 0, 1);
        e.set_u8(x.b, 2, 0x30);
        e.set_u8(x.b, 3, variant);
        e.set_u8(x.b, 0x2f, 0);
        e.set_u16(x.b, f::HOST, x.o.0 as u16);
    }
}

/// 0xa3b4a / 0xa3b66 : un ESHL 0x2d (variante 0 ou, avec `index` en `+0x2f`, 1) lié au boss (`+0x46`).
fn spawn_flame(x: &mut Cx, variant: u8, index: u8) {
    if let Some(s) = try_alloc(x, rom::ALLOC_ESHL) {
        s.set_u8(x.b, 0, 1);
        s.set_u8(x.b, 2, 0x2d);
        s.set_u8(x.b, 3, variant);
        if variant != 0 {
            s.set_u8(x.b, 0x2f, index);
        }
        s.set_u16(x.b, f::HOST, x.o.0 as u16);
    }
}

/// 0x93950 : cherche un joueur à saisir (0xfc44) et le saisit (0xfd64) ; vrai s'il est saisi.
fn try_grab(x: &mut Cx) -> bool {
    x.c.d[0] = 2;
    x.call(call::GRAB_FIND);
    if zero(x) {
        x.c.d[0] = 0;
        return false;
    }
    x.c.d[0] = 0;
    x.call(call::GRAB);
    x.c.d[0] = 1;
    true
}

/// 0x93884 : le boss se baisse en tremblant, bondit sur le joueur et tente de le saisir pendant le saut.
fn crawl_leap(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set8(TIMER, 1);
            x.add16(0xc, 1);
            anim_b(x, 4);
        }
        2 => {
            let d = x.u8(TIMER) as i8 as i16;
            x.add16(0xc, d as u16);
            x.set8(TIMER, (d as u8).wrapping_neg());
            if !step(x) {
                return;
            }
            next7(x);
            launch(x);
        }
        4 => launch(x),
        6 => in_flight(x),
        8 => {
            x.call(call::MOVE_XY);
            if sub8(x, TIMER, 1).gt() {
                return;
            }
            next_attack(x);
        }
        _ => {}
    }
}

/// 0x938ce : décollage vers l'avant, puis la suite du vol dans la même image.
fn launch(x: &mut Cx) {
    next7(x);
    x.set8(TIMER, 3);
    stop(x);
    x.set16(0x18, 0x200);
    x.set16(0x1a, (-0x40i16) as u16);
    let v = if x.u8(f::FLIP) != 0 { 0x800u16 } else { (-0x800i16) as u16 };
    x.c.d[0] = v as u32;
    x.set16(0x14, v);
    anim_b(x, 1);
    in_flight(x);
}

/// 0x938fe : en vol ; après trois images le boss essaie de saisir un joueur, et à l'atterrissage il freine.
fn in_flight(x: &mut Cx) {
    if tst8(x, TIMER).gt() {
        sub8(x, TIMER, 1);
    } else if try_grab(x) {
        return;
    }
    x.call(ia::MOVE_XZ);
    if (x.u16(0xc) as i16) > 0 {
        return;
    }
    x.set16(0xc, 0);
    next7(x);
    x.set32(0x18, 0);
    let v = (x.u16(0x14) as i16 >> 2) as u16;
    set_dw(x, 0, v);
    x.set16(0x14, v);
    x.set8(TIMER, 0x10);
}

/// 0x9396a : charge à toute vitesse, tremble puis repart en arrière.
fn charge(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set8(0x45, 0x10);
            stop(x);
            let (vx, vz) = if x.u8(f::FLIP) != 0 { (-0x400i16, 0xe0i16) } else { (0x400, -0xe0) };
            x.set16(0x14, vx as u16);
            x.set16(0x16, vz as u16);
            x.c.d[0] = 0;
            anim_b(x, 0);
        }
        2 => {
            x.call(call::MOVE_XY);
            let mut v = x.u16(0x14);
            if x.u8(f::FLIP) == 0 {
                v = v.wrapping_neg();
            }
            if (v as i16) < 0 {
                return;
            }
            next7(x);
            stop(x);
            x.set8(TIMER, 8);
        }
        4 => {
            if sub8(x, TIMER, 1).le() {
                next7(x);
                let v = if x.u8(f::FLIP) != 0 { 0x700u16 } else { (-0x700i16) as u16 };
                x.set16(0x14, v);
                x.set8(TIMER, 0x20);
                return;
            }
            let d: u16 = if x.u8(TIMER) & 1 != 0 { 1 } else { 0xffff };
            x.add16(0x8, d);
        }
        6 => {
            step(x);
            x.call(call::MOVE_XY);
            if sub8(x, TIMER, 1).gt() {
                return;
            }
            next_attack(x);
        }
        _ => {}
    }
}

/// 0x93a26 : crache une flamme (ESHL 0x2d variante 0 ; les EFFCT 0x30 l'accompagnent).
fn tail_flame(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set8(TIMER, 0x40);
            anim_b(x, 0xa);
        }
        2 => {
            if !step(x) {
                return;
            }
            next7(x);
            spawn_trail(x, 1);
            x.set8(0xa4, 1);
            anim_b(x, 0xb);
            x.call(0xed836);
        }
        4 => {
            x.set8(0xa4, 1);
            if !step(x) {
                return;
            }
            next7(x);
            anim_b(x, 0xc);
        }
        6 => {
            if step(x) {
                next_attack(x);
            }
        }
        _ => {}
    }
}

/// 0x93a9a : souffle long (variante 1 de la flamme tous les seize images).
fn sweep_a(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set8(TIMER, 0x40);
            anim_b(x, 0x14);
        }
        2 => {
            if !step(x) {
                return;
            }
            next7(x);
            anim_b(x, 0x15);
        }
        4 => {
            if x.a5_u16(0x6e) & 0xf == 0 {
                spawn_flame(x, 0, 0);
                x.call(0xed7cc);
            }
            if !step(x) {
                return;
            }
            next7(x);
            anim_b(x, 0x16);
        }
        6 => {
            if step(x) {
                next_attack(x);
            }
        }
        _ => {}
    }
}

/// 0x93b0c et 0x93b80 : souffle de feu qui laisse une traînée (EFFCT 0x30 variante 2).
fn sweep_b(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set8(TIMER, 0x40);
            anim_b(x, 0x14);
        }
        2 => {
            if !step(x) {
                return;
            }
            next7(x);
            spawn_trail(x, 2);
            x.set8(0xa4, 3);
            anim_b(x, 0x15);
            x.call(0xed8a0);
        }
        4 => {
            x.set8(0xa4, 3);
            if !step(x) {
                return;
            }
            next7(x);
            anim_b(x, 0x16);
        }
        6 => {
            if step(x) {
                next_attack(x);
            }
        }
        _ => {}
    }
}

/// 0x93bf4 : rugissement (ESHL 0x2d variante 1 à l'image-clé).
fn roar(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set8(0, 1);
            x.set8(TIMER, 0x40);
            anim_b(x, 0x1c);
        }
        2 => {
            x.set8(0xa4, 4);
            if x.anim_step() {
                next7(x);
                return anim_b(x, 0x1d);
            }
            let key = x.u8(0x3e);
            if (key as i8) >= 0 {
                return;
            }
            x.set8(0x3e, 0);
            let index = key & 0x7f;
            x.c.d[0] = index as u32;
            spawn_flame(x, 1, index);
            x.call(0xed762);
        }
        4 => {
            if x.anim_step() {
                next_attack(x);
            }
        }
        _ => {}
    }
}

/// 0x93c60 : deux sauts successifs, le second plus long.
fn double_jump(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set8(0x45, 0x10);
            stop(x);
            let v = if x.u8(f::FLIP) != 0 { -0x400i16 } else { 0x400 };
            x.set16(0x14, v as u16);
            x.set16(0x18, 0x800);
            x.set16(0x1a, (-0x60i16) as u16);
            x.c.d[0] = 0;
            anim_b(x, 0);
        }
        2 => {
            step(x);
            x.call(ia::MOVE_XZ);
            if (x.u16(0xc) as i16) < 0 {
                x.set16(0xc, 0);
                next7(x);
                let v = if x.u8(f::FLIP) != 0 { 0x600i16 } else { -0x600 };
                x.set16(0x14, v as u16);
                x.set16(0x18, 0x900);
                x.set16(0x1a, (-0x80i16) as u16);
            }
        }
        4 => {
            step(x);
            x.call(ia::MOVE_XZ);
            if (x.u16(0xc) as i16) < 0 {
                x.set16(0xc, 0);
                next_attack(x);
            }
        }
        _ => {}
    }
}
