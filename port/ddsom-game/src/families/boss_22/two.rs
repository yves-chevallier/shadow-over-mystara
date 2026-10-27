//! Ce que le boss de type 2 (0x7c5a8) a en propre : création (il fait naître le type 22 et, plus tard, la scène de
//! fin), arrivée par un grand bond, et deux attaques (bond-écrasement `+6` = 10, rotation et rugissement `+6` = 12).

use super::moves::step_anim;
use super::*;
use crate::objects::pools;

/// 0x7c5b8 : quand le compte à rebours `+0xbc` est écoulé, crée le contrôleur de scène SET 2 (variante 0x3e), une fois.
pub(super) fn spawn_scene_controller(x: &mut Cx) {
    if x.u8(0xa4) != 0 || !sub16(x, SPOT_TIMER, 1).mi() {
        return;
    }
    let Some(set) = x.alloc(&pools::SET) else { return };
    set.set_u8(x.b, 0, 1);
    set.set_u8(x.b, 3, 0x3e);
    set.set_u8(x.b, 2, 2);
    x.set8(0xa4, 1);
}

/// 0x7c600 : création. Selon le nombre de joueurs et la difficulté, un boss de type 22 lui est lié (`+0xac`).
pub(super) fn init(x: &mut Cx) {
    x.set_a5_u8(0x50, 1);
    x.set_a5_u32(0x38, 0x0459_5900);
    for r in [0x19e10, 0x19db4, 0x19eb0, 0xe1330, 0xd6952] {
        x.call(r);
    }
    let r = random(x);
    x.set8(EFFECT_COUNT, r as u8);
    x.set8(f::BOX_SET, 0x22);
    x.call(0x453c);
    x.call(0xd5f30);
    for (off, v) in [(0x70, 0), (0x7e, 0x23), (0x7f, 0), (0x8e, 3), (0x78, 0xff), (0x68, 8), (0x69, 8), (0x6a, 3), (0x6c, 1), (0x6d, 8)] {
        x.set8(off, v);
    }
    let speed = if x.u16(f::TARGET) != 0 && x.a5_u8(var::PLAYERS_ALIVE) != 1 { 7 } else { 6 };
    x.set8(0x72, speed);
    x.set8(0x73, speed);
    for (off, v) in [(0x74, 5), (0x75, 0x3c), (0x76, 6), (0x81, 0x12), (0x80, 0x17)] {
        x.set8(off, v);
    }
    x.set16(GUARD, 0xb4);
    x.set16(GUARD_TIMER, 0xf0);
    x.set_a5_u8(0x5a, 0);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    pick_target(x);
    x.call(call::FACE_TARGET);
    x.call(call::AIM);
    x.set8(RAGE, 0xff);
    x.set8(SECOND_FX, 0);
    x.set16(SPOT_TIMER, 0x12c);
    x.set8(0x93, 0);
    x.set8(0xac, 0);
    let players = x.a5_u8(var::PLAYERS_ALIVE);
    let alone = players == 1 || (players == 2 && (x.a5_u16(var::DIFFICULTY) as i16) < 0x17);
    if !alone {
        let Some(part) = x.alloc(&pools::BOSS) else { return };
        part.set_u8(x.b, 0, 1);
        part.set_u8(x.b, 2, 0x16);
        x.set16(HOST, part.0 as u16);
        part.set_u16(x.b, HOST, x.o.0 as u16);
        x.set8(0xac, 1);
    }
    if x.u8(0xac) == 0 {
        x.set8(0x93, 1);
        x.set8(0x9a, 0xff);
    }
    x.add8(f::STATE, 2);
    x.c.d[0] = 0;
    x.c.a[4] = TWO_KIT.set_a;
    x.tail(0x1a32);
}

// --- groupe 0 : arrivée -----------------------------------------------------------------------------

/// 0x7c814 : le boss arrive par un grand bond depuis la droite, retombe en poussière et passe en combat.
pub(super) fn arrive(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set8(VULNERABLE, 0);
            let px = x.u16(0x8).wrapping_add(0x1b0).wrapping_add(x.a5_u16(var::SCROLL_X));
            let pz = x.u16(0x10).wrapping_add(0x80).wrapping_add(x.a5_u16(var::SCROLL_Y));
            x.set16(0x8, px);
            x.set16(0x10, pz);
            x.set8(f::FLIP, 0);
            x.set16(0x14, (-0x700i16) as u16);
            x.set16(0x18, 0x600);
            x.set16(0x1a, (-0x50i16) as u16);
            x.set16(0x16, 0);
            x.set8(0x44, 0x2a);
            x.set8(ANIM_ID, 7);
            x.set8(ANIM_LAST, 7);
            start(x, Set::A, 0x1c);
        }
        2 => rise_wait(x),
        4 => climb(x, true),
        6 => climb(x, false),
        8 => {
            x.call(call::MOVE_XZ);
            if (x.u32(0xc) as i32) > 0 {
                return step_anim(x);
            }
            x.c.d[0] = 0x1e;
            x.call(call::MESSAGE);
            x.call(rom::IMMUNITIES);
            x.set32(0xc, 0);
            dust_line(x);
            next7(x);
            x.set16(0x30, 1);
            x.set8(VULNERABLE, 0);
            start(x, Set::A, 6);
        }
        10 => {
            if x.anim_step() {
                x.set8(f::SUB, 2);
                x.set8(0x7, 0);
                x.set8(VULNERABLE, 1);
            }
        }
        _ => {}
    }
}

/// 0x7d0bc : attend l'image-clé de l'animation de saut.
pub(super) fn rise_wait(x: &mut Cx) {
    if x.u8(KEY) != 0 {
        next7(x);
        x.set8(VULNERABLE, 0);
    }
    step_anim(x);
}

/// 0x7d0d2 (montée) et 0x7d0f2 (descente) : la vitesse verticale franchit ±0x100.
pub(super) fn climb(x: &mut Cx, up: bool) {
    x.call(call::MOVE_XZ);
    let vz = x.u16(0x18);
    let reached = if up { Fl::sub16(vz, 0x100).gt() } else { Fl::sub16(vz, 0xff00).gt() };
    if !reached {
        next7(x);
        x.set16(0x30, 1);
    }
    step_anim(x);
}

/// 0x7d112 : retombée du saut (son, poussière en ligne, vulnérable).
pub(super) fn land(x: &mut Cx) {
    x.call(call::MOVE_XZ);
    if (x.u32(0xc) as i32) > 0 {
        return step_anim(x);
    }
    x.c.d[0] = 0x1e;
    x.call(ia::SOUND_EVENT);
    x.call(0xecd50);
    x.set32(0xc, 0);
    dust_line(x);
    next7(x);
    x.set16(0x30, 1);
    x.set8(VULNERABLE, 1);
    start(x, Set::A, 6);
}

// --- attaque 10 : bond-écrasement ---------------------------------------------------------------------

/// 0x7f10a : vitesses du bond selon la distance à la caméra (quatre lignes de 8 octets en 0x7f14c : X, hauteur,
/// gravité, profondeur).
fn jump_speeds(x: &mut Cx) {
    let d = x.u16(0x8).wrapping_sub(x.a5_u16(var::SCROLL_X));
    let row = if Fl::sub16(d, 0x60).le() {
        0
    } else if Fl::sub16(d, 0xc0).le() {
        8
    } else if Fl::sub16(d, 0x120).le() {
        0x10
    } else {
        0x18
    };
    for (off, at) in [(0x14, 0x7f14c), (0x18, 0x7f14e), (0x1a, 0x7f150), (0x16, 0x7f152)] {
        let v = x.rom16(at + row);
        x.set16(off, v);
    }
}

/// 0x7d05c : bond-écrasement. Étapes `+7` : 0 saut, 2 à 8 bond (comme à l'arrivée), 10 réception et coup, 12 demi-tour,
/// 14 coup double, 16 course, 18 course vers un point, 20 virage, 22 à 30 fin.
pub(super) fn slam(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            jump_speeds(x);
            next7(x);
            x.set8(VULNERABLE, 1);
            x.set8(0x44, 0x2a);
            x.set8(0x45, 5);
            x.set8(ANIM_ID, 0xb);
            x.set8(ANIM_LAST, 0xb);
            start(x, Set::A, 5);
        }
        2 => rise_wait(x),
        4 => climb(x, true),
        6 => climb(x, false),
        8 => land(x),
        10 => {
            if !x.anim_step() {
                return;
            }
            x.set8(0, 1);
            if needs_turn(x) {
                next7(x);
                x.set8(VULNERABLE, 1);
                sounds(x, 0x27, 5);
                start(x, Set::A, 1);
            } else {
                x.add8(0x7, 4);
                start(x, Set::B, 1);
            }
        }
        12 => {
            if x.anim_step() {
                next7(x);
                let side = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, side);
                start(x, Set::B, 1);
            }
        }
        14 => {
            if x.u8(KEY) == 1 {
                x.set_a5_u16(0x51e, 4);
                x.set8(KEY, 0);
                x.c.d[1] = 2;
                eshl_sweep_1d(x, 2);
            }
            if x.u8(KEY) == 2 {
                x.set8(KEY, 0);
                x.c.d[1] = 0;
                eshl_sweep_1d(x, 0);
            }
            if !x.anim_step() {
                return;
            }
            next7(x);
            x.set16(TIMER2, 0x12c);
            x.set8(0x2d, 0);
            let r = random(x);
            if r & 1 == 0 {
                x.set8(0x2d, 0x10);
            }
            start(x, Set::A, 8);
            speed(x, 0x1f);
        }
        16 => {
            rubble(x, 0, 0x201, 0x20, 0x20);
            if sub16(x, TIMER2, 1).mi() {
                return next7(x);
            }
            if player_within_reach(x) {
                return x.set8(0x7, 0x1a);
            }
            if target_in_box(x, 0, 0x180, 0x10) {
                return next7(x);
            }
            if x.u8(0x29) != 0 {
                let a = x.u8(0x2d);
                x.set8(0x2d, if a != 0 { 0 } else { 0x10 });
            }
            x.call(call::MOVE_TABLE_XY);
            step_anim(x);
        }
        18 => {
            if player_within_reach(x) {
                return x.set8(0x7, 0x1a);
            }
            next7(x);
            x.set16(TIMER2, 0x12c);
            x.set16(TIMER, 6);
            choose_spot(x);
            start(x, Set::A, 0xa);
            speed(x, 0x1f);
            x.call(call::MOVE_TABLE_XY);
            step_anim(x);
        }
        20 => {
            rubble(x, 0, 0x201, 0x20, 0x20);
            if sub16(x, TIMER2, 1).mi() || player_within_reach(x) {
                return x.set8(0x7, 0x1a);
            }
            if target_in_box(x, 0xc0, 0xc0, 0x10) || target_in_box(x, -0xc0, 0xc0, 0x10) {
                return x.set8(0x7, 0x1a);
            }
            let dx = x.u16(SPOT_X).wrapping_sub(x.u16(0x8)).wrapping_add(8);
            let dz = x.u16(SPOT_Z).wrapping_sub(x.u16(0x10)).wrapping_add(8);
            if dx <= 0x10 && dz <= 0x10 {
                return x.set8(0x7, 0x1a);
            }
            if sub16(x, TIMER, 1).mi() {
                let a = x.u8(0x2d);
                let a = if x.u8(SPIN) != 0 { a.wrapping_sub(1) } else { a.wrapping_add(1) };
                x.set8(0x2d, a & 0x1f);
                x.set16(TIMER, 6);
            }
            if needs_turn(x) {
                return next7(x);
            }
            x.call(call::MOVE_TABLE_XY);
            step_anim(x);
        }
        22 => {
            next7(x);
            start(x, Set::A, 0xb);
        }
        24 => {
            rubble(x, 0, 0x201, 0x20, 0x20);
            if sub16(x, TIMER, 1).mi() {
                let a = x.u8(0x2d);
                let a = if x.u8(SPIN) != 0 { a.wrapping_sub(1) } else { a.wrapping_add(1) };
                x.set8(0x2d, a & 0x1f);
                x.set16(TIMER, 6);
            }
            if x.anim_step() {
                x.add8(0x7, 0xfc);
                let side = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, side);
                return start(x, Set::A, 0x15);
            }
            x.call(call::MOVE_TABLE_XY);
        }
        26 => {
            next7(x);
            x.set8(ARM_TIMER, 0x1e);
            start(x, Set::B, 2);
        }
        28 => {
            if x.u8(KEY) != 0 {
                next7(x);
                x.set8(0xaf, 1);
            }
            step_anim(x);
        }
        30 => {
            if x.u8(0xaf) == 0 {
                x.set_a5_u16(0x51e, 0);
                return next_attack(x);
            }
            step_anim(x);
        }
        _ => {}
    }
}

// --- attaque 12 : rotation et rugissement ------------------------------------------------------------------

/// Point visé sur le bord de l'écran, du côté où le boss regarde (`+0xb0`, `+0xb2`).
fn edge_spot(x: &mut Cx) {
    let off = if x.u8(f::FLIP) != 0 { 0x160 } else { 0x20 };
    let sx = x.a5_u16(var::SCROLL_X).wrapping_add(off);
    x.set16(SPOT_X, sx);
    let sz = x.a5_u16(var::SCROLL_Y).wrapping_add(0x60);
    x.set16(SPOT_Z, sz);
}

/// 0x7d4a0 : le boss tourne autour d'un point du bord de l'écran, se retourne, rugit (effets, cinématique) puis reprend.
pub(super) fn spin_roar(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set8(VULNERABLE, 1);
            x.set8(ANIM_ID, 3);
            sounds(x, 0x27, 5);
            x.set16(TIMER, 0);
            edge_spot(x);
            x.set8(0xb6, 4);
            if x.u8(ANIM_ID) != x.u8(ANIM_LAST) {
                x.set8(ANIM_LAST, 3);
                start(x, Set::A, 4);
            }
            speed(x, 0x26);
        }
        2 => {
            rubble(x, 0, 0x201, 0x20, 0x20);
            edge_spot(x);
            if sub16(x, TIMER, 1).mi() {
                let r = rand_and(x, 7);
                let t = x.rom16(0x7d5f0 + 2 * r);
                x.set16(TIMER, t);
                let b6 = x.u8(0xb6).wrapping_neg();
                x.set8(0xb6, b6);
                let (sx, sz) = (x.u16(SPOT_X), x.u16(SPOT_Z));
                set_dw(x, 1, sx);
                set_dw(x, 2, sz);
                x.call(0x137e);
                let angle = (x.c.d[0] as u8).wrapping_add(b6);
                x.set8(0x2d, angle);
            }
            let cam = x.a5_u16(var::SCROLL_X);
            if x.u8(f::FLIP) != 0 {
                if (cam.wrapping_add(0x130).wrapping_sub(x.u16(0x8)) as i16) < 0 {
                    return next7(x);
                }
            } else if (cam.wrapping_add(0x50).wrapping_sub(x.u16(0x8)) as i16) >= 0 {
                return next7(x);
            }
            x.call(call::MOVE_TABLE_XY);
            step_anim(x);
        }
        4 => {
            next7(x);
            start(x, Set::A, 1);
        }
        6 => {
            if x.anim_step() {
                next7(x);
                let side = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, side);
                start(x, Set::B, 5);
            }
        }
        8 => {
            next7(x);
            x.set_a5_u16(0x51e, 4);
            x.set8(ARM_TIMER, 0x32);
            x.set_a5_u8(0x43, 0x7f);
            x.set8(VULNERABLE, 1);
            x.set8(ANIM_ID, 0x10);
            x.set8(ANIM_LAST, 0x10);
            start(x, Set::B, 5);
        }
        10 => {
            if x.u8(KEY) == 1 {
                x.set8(KEY, 0);
                x.c.d[1] = 2;
                eshl_sweep_1d(x, 2);
            }
            if x.u8(KEY) == 2 {
                x.set8(KEY, 0);
                eshl_wave_1e(x);
                x.call(0x1ac9a);
                x.set8(0xb9, 4);
            }
            x.set_a5_u16(0x4ba, 0);
            if x.u8(0xb9) != 0 {
                x.set_a5_u16(0x4ba, 0x100);
                sub8(x, 0xb9, 1);
            }
            if x.anim_step() {
                next7(x);
                x.set8(0xb8, 0);
                x.set8(0xaf, 1);
                start(x, Set::B, 6);
            }
        }
        12 => {
            if x.u8(0xaf) == 0 {
                x.set8(VULNERABLE, 0);
                x.set_a5_u8(0x43, 0xff);
                x.set_a5_u16(0x51e, 0);
                return next_attack(x);
            }
            step_anim(x);
        }
        _ => {}
    }
}
