//! Réactions communes des ennemis et des boss aux coups (O3, `0x3bb00-0x3e480`).
//!
//! Quand un coup touche, la collision range un code de réaction en `+0x49` ; l'état « touché » de chaque type
//! appelle la routine désignée par ce code dans sa table de réactions (longs, par exemple `0x56816` pour le
//! Kobold, voir `enemy_common::react`). Ces routines sont communes à tous les types : recul, projection en l'air,
//! chute, rebond au sol, relevé, mort clignotante, saisie, glissade... Chacune est une machine à états sur `+6`
//! (pas de 2, table de sauts relative au PC) dont les pas sont souvent partagés (`chute`, `rebond`, `relevé`,
//! `mort`). Les animations et les sons dépendent du type (`+2`) et de la classe (`+0x28` : 2 ENEMY, 4 BOSS),
//! par des tables de la ROM.
//!
//! Champs (en plus de ceux de `enemy_common::f`) : vitesses `+0x14` (X), `+0x16` (profondeur), `+0x18`
//! (hauteur), `+0x1a` (gravité), `+0x1c`/`+0x1e` (secondaires) ; hauteur `+0xc` (long) ; compteur `+0x90` ;
//! liaison en attente `+0x91` (bit 7) vers `+0x2a`/`+0x2b` ; orientation de l'agresseur `+0x4a`.

use super::cx::Cx;
use crate::mem::{rd16, rd32, rd8, rts};
use crate::objects::anim::load_entry;
use crate::objects::Obj;
use crate::Native;
use crate::objects::pools;
use crate::objects::Courant;
use crate::machine::Machine;

/// Champs des objets touchés.
mod f {
    pub const ACTIVE: u32 = 0;
    pub const DISPLAY: u32 = 1;
    pub const KIND: u32 = 2;
    pub const STATE: u32 = 4;
    pub const SUB: u32 = 5;
    pub const STEP: u32 = 6;
    pub const SUB_STEP: u32 = 7;
    pub const X: u32 = 8;
    pub const X_SAVED: u32 = 0xa;
    pub const HEIGHT: u32 = 0xc;
    pub const Y: u32 = 0x10;
    pub const VX: u32 = 0x14;
    pub const VY: u32 = 0x16;
    pub const VZ: u32 = 0x18;
    pub const GRAVITY: u32 = 0x1a;
    pub const VX2: u32 = 0x1c;
    pub const VY2: u32 = 0x1e;
    pub const CLASS: u32 = 0x28;
    /// Contact avec un obstacle pendant le déplacement.
    pub const BLOCKED: u32 = 0x29;
    pub const LINKED: u32 = 0x2a;
    pub const LINK: u32 = 0x2b;
    pub const FLIP: u32 = 0x2c;
    pub const ANGLE: u32 = 0x2d;
    /// Fin de l'animation (mot ; octet `+0x33` : événement de l'animation).
    pub const ANIM_END: u32 = 0x32;
    pub const ANIM_EVENT: u32 = 0x33;
    pub const REACTION: u32 = 0x49;
    pub const ATTACKER_FLIP: u32 = 0x4a;
    pub const OWNER: u32 = 0x50;
    pub const FREEZE: u32 = 0x53;
    pub const STUN: u32 = 0x56;
    pub const HP: u32 = 0x62;
    pub const RECOVER: u32 = 0x84;
    pub const DOWNED: u32 = 0x86;
    pub const GRABBED: u32 = 0x87;
    pub const RESUME_STATE: u32 = 0x8a;
    pub const COUNTER: u32 = 0x90;
    pub const PENDING: u32 = 0x91;
}

/// Classes (octet `+0x28`).
const ENEMY: u8 = 2;
const BOSS: u8 = 4;

/// Tables de la ROM.
mod rom {
    /// Jeux d'animations par type (longs lus comme données ; mot haut nul : routine propre, décalage).
    pub const ANIM_SETS: u32 = 0x3de58;
    /// Variante d'animation selon `+0xbf` (octets, relatifs au PC).
    pub const ANIM_VARIANTS: u32 = 0x3e28a;
    /// Sons de coup et de mort par type (longs relatifs au PC), routine du relevé par type.
    pub const HIT_SOUNDS: u32 = 0x3e2ac;
    pub const DEATH_SOUNDS: u32 = 0x3e3a4;
    pub const GET_UP: u32 = 0x3e000;
    /// Types qui ne restent pas au sol quand ils sont saisis (octets relatifs au PC).
    pub const NO_GRAB_DOWN: u32 = 0x3e1ee;
    /// Projections prédéfinies : compteur (octets), vitesses (4 mots par entrée).
    pub const LAUNCH_COUNTERS: u32 = 0x3bdec;
    pub const LAUNCH_SPEEDS: u32 = 0x3bdcc;
    /// Routine vide (`rts`) des tables par type.
    pub const NOTHING: [u32; 2] = [0x3e2aa, 0x3bb9a];
    /// Animations de réaction du joueur (longs par personnage, données).
    pub const PLAYER_HURT: u32 = 0x3bb52;
}

/// Sous-routines du moteur.
mod call {
    pub const LANDED: u32 = 0x3e480;
    pub const SPAWN_EFFECT: u32 = 0xabec8;
    pub const DEATH: u32 = 0x19ad6;
    pub const BOSS_RECOVER: u32 = 0xf118;
}

// ---------------------------------------------------------------------------------------------
// Aides

fn next(x: &mut Cx) {
    x.add8(f::STEP, 2);
}
fn next_sub(x: &mut Cx) {
    x.add8(f::SUB_STEP, 2);
}
/// Pas suivant, sous-pas à zéro.
fn next_clear(x: &mut Cx) {
    next(x);
    x.set8(f::SUB_STEP, 0);
}
fn neg16(x: &mut Cx, off: u32) {
    let v = x.u16(off).wrapping_neg();
    x.set16(off, v);
}
fn neg32(x: &mut Cx, off: u32) {
    let v = x.u32(off).wrapping_neg();
    x.set32(off, v);
}
fn flipped(x: &mut Cx) -> bool {
    x.u8(f::FLIP) != 0
}
fn dead(x: &mut Cx) -> bool {
    (x.u16(f::HP) as i16) <= 0
}
fn paused(x: &mut Cx) -> bool {
    x.a5_u8(0x3d) != 0
}
/// Mort : sous-état 6 (+ `+0` = 2).
fn mark_dead(x: &mut Cx) {
    x.set8(f::SUB, 6);
    x.set8(f::ACTIVE, 2);
}
fn speeds(x: &mut Cx, vx: u16, vy: u16, vz: u16, gravity: u16) {
    x.set16(f::VX, vx);
    x.set16(f::VY, vy);
    x.set16(f::VZ, vz);
    x.set16(f::GRAVITY, gravity);
}
fn anim_step(x: &mut Cx) {
    x.anim_step();
}
fn move_x(x: &mut Cx) {
    x.move_x();
}
fn move_xz(x: &mut Cx) {
    x.move_xz();
}
fn move_xyz(x: &mut Cx) {
    x.move_xyz();
}
fn landed(x: &mut Cx) {
    x.call(call::LANDED);
}
fn random(x: &mut Cx) -> u32 {
    let alea = x.random_68k();
    x.c.d[0] & 0xffff_0000 | alea as u32
}
/// Effet `0xabec8` (D0 = variante, D1 = paramètre).
fn effect(x: &mut Cx, d0: u32, d1: u32) {
    x.c.d[0] = d0;
    x.c.d[1] = d1;
    x.call(call::SPAWN_EFFECT);
}
/// Indice des tables par type : type, plus 32 hors de la classe ENEMY.
fn type_index(x: &mut Cx) -> u32 {
    let base = if x.u8(f::CLASS) == ENEMY { 0 } else { 32 };
    (base + x.u8(f::KIND) as u32) & 0xff
}

/// 0x3de26 : le gel après le coup prend la durée d'étourdissement.
fn stun(x: &mut Cx) {
    let s = x.u8(f::STUN);
    x.set8(f::FREEZE, s);
}

/// 0x3de2e : animation `n` du jeu du type (les types 13, 19, 34 et 54 ajoutent une variante `+0xbf`).
fn anim(x: &mut Cx, n: u32) {
    x.c.d[0] = n;
    let i = 4 * type_index(x);
    x.c.d[1] = i;
    let entry = rd32(x.b, rom::ANIM_SETS + i);
    if entry >> 16 != 0 {
        x.c.a[4] = entry;
    } else {
        // routine propre : `bsr 0x3e27e ; movea.l #jeu,A4 ; jmp 0x1a32`
        let routine = rom::ANIM_SETS + (entry & 0xffff);
        let v = x.u8(0xbf) as u32;
        x.c.d[1] = v;
        let add = x.rom8(rom::ANIM_VARIANTS + v);
        x.c.d[0] = x.c.d[0] & !0xff | (x.c.d[0] as u8).wrapping_add(add) as u32;
        x.c.a[4] = (x.rom16(routine + 6) as u32) << 16 | x.rom16(routine + 8) as u32;
    }
    let (set, n) = (x.c.a[4], x.c.d[0] as u16);
    x.anim_start(set, n);
}

/// Routine d'une table par type (longs relatifs au PC) ; les entrées vides (`rts`) ne font rien.
fn per_type(x: &mut Cx, table: u32, index: u32) {
    let i = 4 * index;
    x.c.d[0] = i;
    let routine = (x.rom16(table + i) as u32) << 16 | x.rom16(table + i + 2) as u32;
    x.c.a[1] = routine;
    if !rom::NOTHING.contains(&routine) {
        x.call(routine);
    }
}
/// 0x3e28e et 0x3e388 : son du coup, son de la mort.
fn hit_sound(x: &mut Cx) {
    let i = type_index(x);
    per_type(x, rom::HIT_SOUNDS, i);
}
fn death_sound(x: &mut Cx) {
    let i = type_index(x);
    per_type(x, rom::DEATH_SOUNDS, i);
}
/// 0x3e180 : son du coup, ou mort (sous-état 6) et son de la mort.
fn hurt(x: &mut Cx) {
    if dead(x) {
        mark_dead(x);
        death_sound(x);
    } else {
        hit_sound(x);
    }
}

/// 0x3e130 : l'objet se tourne vers l'agresseur ; les vitesses X et profondeur suivent l'orientation.
fn face_attacker(x: &mut Cx) {
    let flip = x.u8(f::ATTACKER_FLIP) ^ 0x20;
    x.c.d[2] = x.c.d[2] & !0xff | flip as u32;
    x.set8(f::FLIP, flip);
    if flip != 0 {
        neg16(x, f::VX);
        neg16(x, f::VY);
    }
}
/// 0x3e148 : les vitesses suivent l'orientation actuelle.
fn orient_speeds(x: &mut Cx) {
    if flipped(x) {
        neg16(x, f::VX);
        neg16(x, f::VY);
    }
}

/// 0x3e25e : une liaison en attente (`+0x91`, bit 7) devient effective (`+0x2b`, `+0x2a` levé).
fn apply_pending(x: &mut Cx) {
    let p = x.u8(f::PENDING);
    if (p as i8) >= 0 {
        return;
    }
    x.set8(f::LINK, p & 0x7f);
    x.set8(f::LINKED, 1);
    x.set8(f::PENDING, 0);
}
/// Variante qui efface toujours `+0x91`.
fn take_pending(x: &mut Cx) {
    let p = x.u8(f::PENDING);
    if (p as i8) < 0 {
        x.set8(f::LINK, p & 0x7f);
        x.set8(f::LINKED, 1);
    }
    x.set8(f::PENDING, 0);
}
/// Inverse : la liaison effective repasse en attente (`+0x2b` | 0x80 dans `+0x91`).
fn defer_link(x: &mut Cx) {
    if (x.u8(f::PENDING) as i8) < 0 || x.u8(f::LINKED) == 0 {
        return;
    }
    let l = x.u8(f::LINK) | 0x80;
    x.set8(f::PENDING, l);
    x.set8(f::LINKED, 0);
}

/// 0x3bda2 : projection prédéfinie `n` (compteur et quatre vitesses de la ROM).
fn launch(x: &mut Cx, n: u32) {
    x.set8(f::DOWNED, 0);
    next(x);
    let c = x.rom8(rom::LAUNCH_COUNTERS + n);
    x.set8(f::COUNTER, c);
    let at = rom::LAUNCH_SPEEDS + 8 * n;
    x.c.d[0] = 8 * n;
    for (k, off) in [f::VX, f::VY, f::VZ, f::GRAVITY].into_iter().enumerate() {
        let v = x.rom16(at + 2 * k as u32);
        x.set16(off, v);
    }
}

/// Tremblement sur place : X oscille de ±2 selon la parité du compteur et l'orientation.
fn shake(x: &mut Cx, amount: u16) {
    let mut d = amount;
    if x.u8(f::COUNTER) & 1 != 0 {
        d = d.wrapping_neg();
    }
    if flipped(x) {
        d = d.wrapping_neg();
    }
    x.c.d[0] = x.c.d[0] & !0xffff | d as u32;
    x.add16(f::X, d);
}

// ---------------------------------------------------------------------------------------------
// Pas communs

/// 0x3bdf0 : attend la fin de l'animation.
fn wait_anim(x: &mut Cx) {
    if x.u16(f::ANIM_END) != 0 {
        next(x);
    } else {
        anim_step(x);
    }
}

/// 0x3be02 : chute (déplacement X et hauteur) ; au sol, pas suivant et atterrissage.
fn fall(x: &mut Cx) {
    move_xz(x);
    if x.u16(f::HEIGHT) as i16 > 0 {
        if x.u8(f::BLOCKED) != 0 {
            x.set32(f::VX, 0);
            x.set8(f::COUNTER, 0);
        }
        return anim_step(x);
    }
    next_clear(x);
    x.set32(f::HEIGHT, 0);
    x.set32(f::VZ, 0);
    landed(x);
}

/// 0x3be38 : chute, l'octet `+0x38` à zéro.
fn fall_38(x: &mut Cx) {
    fall(x);
    x.set8(0x38, 0);
}

/// 0x3e22e : pas suivant ; le boss de type 3 non saisi enchaîne sa réaction propre (0x81ae0).
fn boss_3_hook(x: &mut Cx) {
    next(x);
    if x.u8(f::KIND) == 3 && x.u8(f::CLASS) == BOSS && x.u8(f::SUB) != 6 && x.u8(f::GRABBED) == 0 {
        x.call(0x81ae0);
    }
}

/// 0x3be52 : rebond au sol (animation 5).
fn bounce_start(x: &mut Cx) {
    x.set16(f::VY, 0);
    x.set16(f::VZ, 0x300);
    x.set16(f::GRAVITY, -72i16 as u16);
    next(x);
    x.set8(f::SUB_STEP, 0);
    anim(x, 5);
}
/// 0x3be40 : rebond en reculant.
fn bounce_back(x: &mut Cx) {
    x.set16(f::VX, 0x200);
    if flipped(x) {
        neg16(x, f::VX);
    }
    bounce_start(x);
}
/// 0x3be70 : rebond (animation 2) après une liaison.
fn bounce_linked(x: &mut Cx) {
    take_pending(x);
    x.set16(f::VZ, 0x300);
    x.set16(f::GRAVITY, -72i16 as u16);
    next(x);
    x.set8(f::SUB_STEP, 0);
    anim(x, 2);
}
/// 0x3cf74 : rebond (animation 2).
fn bounce_2(x: &mut Cx) {
    x.set16(f::VZ, 0x300);
    x.set16(f::GRAVITY, -72i16 as u16);
    next(x);
    x.set8(f::SUB_STEP, 0);
    anim(x, 2);
}

/// 0x3bea8 : le rebond, quatre sous-pas (`+7`).
fn bounce(x: &mut Cx) {
    let s = x.u8(f::SUB_STEP);
    x.c.d[0] = x.c.d[0] & !0xff | s as u32;
    match s {
        0 => {
            anim_step(x);
            if x.u8(f::ANIM_EVENT) != 0 {
                next_sub(x);
            }
        }
        2 => {
            move_xz(x);
            if x.u8(f::BLOCKED) != 0 {
                x.set32(f::VX, 0);
            }
            if x.u16(f::HEIGHT) as i16 > 0 {
                return;
            }
            next_sub(x);
            landed(x);
        }
        4 => {
            if (x.u8(f::ANIM_END) as i8) >= 0 {
                return anim_step(x);
            }
            if x.u8(f::COUNTER) == 0 {
                return next_clear(x);
            }
            x.set16(f::VX, 0x280);
            x.set16(f::VY, -64i16 as u16);
            if flipped(x) {
                neg16(x, f::VX);
                neg16(x, f::VY);
            }
            next_sub(x);
        }
        6 => {
            move_x(x);
            if x.u8(f::VX) == 0 {
                next_clear(x);
            }
        }
        _ => {}
    }
}

/// 0x3e0dc : relevé ; mort (sous-état 6) : pas suivant.
fn get_up(x: &mut Cx) {
    x.set32(0x16, 0);
    x.set8(0x67, 0);
    if x.u8(f::SUB) == 6 {
        return next_clear(x);
    }
    x.set8(f::DOWNED, 1);
    x.c.a[6] = 0x3e0fc;
    grab_check(x, get_up_done);
}
/// 0x3df7c : fin de la glissade : l'objet se relève.
fn slide_end(x: &mut Cx) {
    x.set32(0x16, 0);
    if x.a5_u8(0x30) != 0 {
        x.set8(0x67, 1);
    }
    x.set8(f::ACTIVE, 1);
    x.set8(f::REACTION, 0);
    x.c.a[6] = 0x3df9e;
    grab_check(x, resume_after_slide);
}

/// 0x3e19a : un objet saisi (`+0x87`) au sol peut rester à terre (code de réaction 104, ou 108 s'il était à
/// terre) ; sinon la suite `then` (A6 la désigne dans l'original : `jmp (A6)`).
fn grab_check(x: &mut Cx, then: fn(&mut Cx)) {
    if x.u8(f::GRABBED) != 0 {
        // `bhi` sur la dernière comparaison de 0x1e92 (Z compris) : drapeaux de l'original (adaptateur)
        crate::objects::cull::in_arena_68k(x.c, x.b, x.o, 0);
        if !x.c.cond(2) {
            x.set16(f::RECOVER, 1);
            let i = type_index(x);
            x.c.d[0] = i;
            let keep = x.rom8(rom::NO_GRAB_DOWN + i);
            x.c.d[0] = keep as u32;
            if keep == 0 {
                let code = if x.u8(f::DOWNED) != 0 { 108 } else { 104 };
                x.set8(f::REACTION, code);
                x.set16(f::STEP, 0);
                return;
            }
        }
    }
    x.set8(f::GRABBED, 0);
    then(x);
}

/// 0x3e0fc : debout, l'état principal reprend (0x04040000) ; le relevé du boss (0xf118) sauf code 108.
fn get_up_done(x: &mut Cx) {
    x.set8(f::ACTIVE, 1);
    x.set32(f::STATE, 0x0404_0000);
    x.set32(0x58, 0);
    x.set8(0x67, 0);
    x.set16(f::RECOVER, 1);
    if x.u8(f::REACTION) != 108 {
        x.call(call::BOSS_RECOVER);
    }
}

/// 0x3df9e : après la glissade, l'état mémorisé (`+0x8a`) reprend s'il le peut, sinon l'état actif et la
/// routine de relevé du type.
fn resume_after_slide(x: &mut Cx) {
    let saved = x.u32(f::RESUME_STATE);
    if saved != 0 {
        x.set32(0x58, 0);
        x.set8(f::ACTIVE, 1);
        if x.u16(f::RESUME_STATE) == 0x205 {
            x.call(0xad7c);
            if !x.c.cond(7) {
                x.set32(f::STATE, saved);
                x.set8(f::SUB_STEP, 0);
                x.set32(f::RESUME_STATE, 0);
                x.set8(0x78, 0xff);
                return;
            }
        }
    }
    x.set32(f::STATE, 0x0202_0000);
    x.set8(0x78, 0xff);
    let base = if x.u8(f::CLASS) == BOSS { 32 } else { 0 };
    let i = (base + x.u8(f::KIND) as u32) & 0xff;
    per_type(x, rom::GET_UP, i);
}

/// 0x3dd9c : mort ; 40 images de clignotement, puis l'objet disparaît (0x19ad6).
fn death(x: &mut Cx) {
    if x.u8(f::SUB_STEP) == 0 {
        x.set8(f::COUNTER, 40);
        next_sub(x);
        for r in [0x27ee, 0xb780, 0xbad6, 0xe2f6] {
            x.call(r);
        }
    }
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c == 0 {
        x.set32(f::STATE, 0x0600_0000);
        return x.call(call::DEATH);
    }
    blink(x, c);
}
/// Clignotement : bit 1 de `+1` (caché) une image sur quatre... selon les deux bits bas du compteur.
fn blink(x: &mut Cx, c: u8) {
    x.set_bit(f::DISPLAY, 1, false);
    x.c.d[0] = x.c.d[0] & !0xff | (c & 3) as u32;
    if c & 3 != 0 {
        x.set_bit(f::DISPLAY, 1, true);
    }
}

/// 0x3e158 : pas suivant, puis 0x51c06.
fn step_51c06(x: &mut Cx) {
    next(x);
    x.call(0x51c06);
}
/// 0x3e162 : pas suivant.
fn skip(x: &mut Cx) {
    next(x);
}
/// `rts`.
fn idle(_: &mut Cx) {}

// ---------------------------------------------------------------------------------------------
// Projections (premier pas des réactions)

/// 0x3bb9c (code 0) : recul sur place ; mort si les PV sont épuisés.
fn recoil(x: &mut Cx) {
    if x.u8(f::STEP) != 0 {
        return recoil_shake(x);
    }
    x.set8(0x67, 0);
    apply_pending(x);
    if dead(x) {
        x.set8(f::SUB, 6);
        x.set8(f::ACTIVE, 2);
        return run_entry(x, 0x3dd72);
    }
    recoil_start(x, 0);
}
/// Début du recul (16 images de tremblement, petite poussée), animation `n`.
fn recoil_start(x: &mut Cx, n: u32) {
    next(x);
    stun(x);
    x.set8(f::COUNTER, 16);
    x.set16(f::VX, 0x180);
    x.set16(f::VY, -56i16 as u16);
    face_attacker(x);
    anim(x, n);
    hit_sound(x);
}
/// 0x3bbec : tremblement, puis glissade (0x3bc10).
fn recoil_shake(x: &mut Cx) {
    if x.u8(f::COUNTER) != 0 {
        let c = x.u8(f::COUNTER).wrapping_sub(1);
        x.set8(f::COUNTER, c);
        return shake(x, 2);
    }
    slide(x);
}
/// 0x3bc10 / 0x3d7f8 : glissade jusqu'à l'arrêt, puis relevé.
fn slide(x: &mut Cx) {
    if x.u8(f::VX) == 0 {
        return slide_end(x);
    }
    move_x(x);
    anim_step(x);
}

/// 0x3bf76 (code 0x38 et voisins) : comme le recul, animation 1, mort par 0x3ddf6.
fn recoil_1(x: &mut Cx) {
    if dead(x) {
        mark_dead(x);
        return run_entry(x, 0x3ddf6);
    }
    if x.u8(f::STEP) != 0 {
        return recoil_shake(x);
    }
    apply_pending(x);
    next(x);
    x.set8(f::COUNTER, 16);
    stun(x);
    x.set16(f::VX, 0x180);
    x.set16(f::VY, -56i16 as u16);
    face_attacker(x);
    anim(x, 1);
    hit_sound(x);
}

/// Projection en l'air : vitesse X `vx` (long), hauteur 0x400, gravité -64, compteur `counter`, animation
/// `anim_n` ; `steps` pas d'avance (deux pour certaines variantes) ; `clear_67` : l'octet `+0x67` est effacé.
fn throw(x: &mut Cx, vx: u32, counter: u8, anim_n: u32, steps: u8, clear_67: bool) {
    if clear_67 {
        x.set8(0x67, 0);
    }
    apply_pending(x);
    x.set8(f::DOWNED, 0);
    anim(x, anim_n);
    x.add8(f::STEP, 2 * steps);
    x.set8(f::COUNTER, counter);
    x.set32(f::VX, vx);
    x.set16(f::VZ, 0x400);
    x.set16(f::GRAVITY, -64i16 as u16);
    x.set32(f::VX2, 0);
    face_attacker(x);
    stun(x);
    hurt(x);
}

/// 0x3bc42, 0x3bca0, 0x3bd00, 0x3bd60 : projections (animation 2), de plus en plus fortes.
fn throw_1(x: &mut Cx) {
    throw(x, 0x0180_0000, 0, 2, 1, true);
}
fn throw_2(x: &mut Cx) {
    throw(x, 0x0200_0000, 1, 2, 1, true);
}
fn throw_3(x: &mut Cx) {
    throw(x, 0x0280_0000, 1, 2, 1, true);
}
fn throw_4(x: &mut Cx) {
    throw(x, 0x0300_0000, 1, 2, 1, true);
}
/// 0x3bfde, 0x3c03a, 0x3c098, 0x3c0f2 : mêmes projections, animation 3 (les deux premières sautent un pas).
fn throw_1b(x: &mut Cx) {
    throw(x, 0x0180_0000, 0, 3, 2, false);
}
fn throw_2b(x: &mut Cx) {
    throw(x, 0x0200_0000, 1, 3, 2, false);
}
fn throw_3b(x: &mut Cx) {
    throw(x, 0x0280_0000, 1, 3, 1, false);
}
fn throw_4b(x: &mut Cx) {
    throw(x, 0x0300_0000, 1, 2, 1, false);
}

// ---------------------------------------------------------------------------------------------
// Projections hautes (0x3c14e et voisines) et glissades au sol

/// 0x3c15e : coup qui fait décoller (vitesses X 0x600, hauteur 0x280) après l'animation `n`.
fn uppercut(x: &mut Cx, n: u32) {
    anim(x, n);
    x.set8(0x67, 0);
    apply_pending(x);
    x.set8(f::DOWNED, 0);
    hurt(x);
    next(x);
    stun(x);
    speeds(x, 0x600, 0, 0x280, -64i16 as u16);
    face_attacker(x);
}
fn uppercut_2(x: &mut Cx) {
    uppercut(x, 2);
}
fn uppercut_3(x: &mut Cx) {
    uppercut(x, 3);
}

/// 0x3c192 : rebond haut (animation 5).
fn bounce_high(x: &mut Cx) {
    next(x);
    x.set16(f::VX, 0x300);
    x.set16(f::VZ, 0x300);
    x.set16(f::GRAVITY, -72i16 as u16);
    orient_speeds(x);
    x.set8(f::SUB_STEP, 0);
    anim(x, 5);
}

/// Début d'une glissade au sol (animation 6) à la vitesse `vx`.
fn slide_start(x: &mut Cx, vx: u16) {
    next(x);
    x.set8(f::SUB_STEP, 0);
    x.set16(f::VX, vx);
    x.set16(f::VY, -64i16 as u16);
    orient_speeds(x);
    anim(x, 6);
}
fn slide_start_fast(x: &mut Cx) {
    slide_start(x, 0x600);
}
fn slide_start_slow(x: &mut Cx) {
    slide_start(x, 0x400);
}

/// Poussière de la glissade, une image sur deux (0xb10c8).
fn dust(x: &mut Cx) {
    let was = x.bit(f::COUNTER, 0);
    x.set_bit(f::COUNTER, 0, !was);
    if !was {
        return;
    }
    x.c.d[0] = 0;
    x.c.d[1] = x.c.d[1] & !0xffff | 1;
    x.c.d[2] = 0;
    x.c.d[3] = u32::MAX;
    x.call(0xb10c8);
}

/// 0x3c270 : glissade ; arrêtée, l'ENEMY de type 0 attend l'événement de son animation pour l'animation 7.
fn slide_fast(x: &mut Cx) {
    if x.u8(f::SUB_STEP) != 0 {
        anim_step(x);
        if x.u8(f::ANIM_EVENT) == 0 {
            return dust(x);
        }
        return next(x);
    }
    anim_step(x);
    move_x(x);
    if x.u8(f::BLOCKED) != 0 {
        x.set32(f::VX, 0);
    }
    if x.u8(f::VX) == 0 {
        x.set32(f::VX, 0);
        if x.u8(f::KIND) == 0 && x.u8(f::CLASS) == ENEMY {
            if x.u8(f::ANIM_EVENT) != 0 {
                next_sub(x);
                return anim(x, 7);
            }
        } else {
            next(x);
        }
    }
    dust(x)
}

/// 0x3c500 : glissade lente ; arrêtée, l'animation 7 quand l'animation en cours le signale.
fn slide_slow(x: &mut Cx) {
    anim_step(x);
    if x.u8(f::SUB_STEP) != 0 {
        if x.u8(f::ANIM_EVENT) == 0 {
            return dust(x);
        }
        return next_clear(x);
    }
    move_x(x);
    if x.u8(f::BLOCKED) != 0 {
        x.set16(f::VX, 0);
        x.set16(f::VY, 0);
    }
    if x.u8(f::VX) == 0 {
        x.set16(f::VX, 0);
        x.set16(f::VY, 0);
        if x.u8(f::ANIM_EVENT) != 0 {
            next_sub(x);
            return anim(x, 7);
        }
    }
    dust(x)
}

/// 0x3c30a : coup bas (animation 4).
fn low_hit(x: &mut Cx) {
    x.set8(0x67, 0);
    apply_pending(x);
    hurt(x);
    next(x);
    stun(x);
    speeds(x, 0x200, 0, 0x180, -72i16 as u16);
    face_attacker(x);
    anim(x, 4);
}

/// 0x3c374 : projection rasante (18 images, animation 1).
fn skim(x: &mut Cx) {
    x.set8(0x67, 0);
    apply_pending(x);
    hurt(x);
    next(x);
    x.set8(f::SUB_STEP, 0);
    stun(x);
    x.set8(f::COUNTER, 18);
    speeds(x, 0x800, 0, 0x100, -4i16 as u16);
    face_attacker(x);
    anim(x, 1);
}

/// 0x3c3ce : vol rasant (`+7`) : 0 plané, 4 chute, 2 chute qui repart en arrière.
fn skim_flight(x: &mut Cx) {
    let s = x.u8(f::SUB_STEP);
    x.c.d[0] = x.c.d[0] & !0xff | s as u32;
    match s {
        0 => {
            move_xz(x);
            let c = x.u8(f::COUNTER).wrapping_sub(1);
            x.set8(f::COUNTER, c);
            if c == 0 {
                x.add8(f::SUB_STEP, 4);
                x.set8(f::COUNTER, 1);
                x.set16(f::GRAVITY, -64i16 as u16);
                x.set16(f::VY, -64i16 as u16);
                if flipped(x) {
                    neg16(x, f::VY);
                }
                return;
            }
            if x.u32(f::HEIGHT) as i32 <= 0 {
                return next(x);
            }
            let camera = if x.a5_u8(0x36) != 0 { x.a5_u16(0x556) } else { x.a5_u16(0x506) };
            let dx = x.u16(f::X).wrapping_sub(camera);
            x.c.d[0] = x.c.d[0] & !0xffff | dx as u32;
            if dx <= 0x180 && x.u8(f::BLOCKED) == 0 {
                return;
            }
            next_sub(x);
            x.c.d[0] = 0;
            x.set8(f::COUNTER, 0);
            x.set32(f::VX, 0);
            x.set16(f::VZ, 0);
            x.set16(f::GRAVITY, -64i16 as u16);
        }
        2 => {
            move_xz(x);
            if x.u16(f::HEIGHT) as i16 > 0 {
                return;
            }
            next(x);
            x.set32(f::HEIGHT, 0);
            x.set32(f::VX, 0);
            landed(x);
        }
        4 => {
            move_xz(x);
            if x.u16(f::HEIGHT) as i16 > 0 {
                return;
            }
            next(x);
            x.set32(f::HEIGHT, 0);
            x.set16(f::VY, 0);
            landed(x);
            x.set16(f::VX, 0x200);
            if flipped(x) {
                neg16(x, f::VX);
            }
        }
        _ => {}
    }
}

/// Projection en hauteur (animation `n`, vitesse X `vx`, hauteur `vz`, gravité `g`), compteur 1.
fn pop(x: &mut Cx, n: u32, vx: u16, vz: u16, g: u16, clear_67: bool) {
    if clear_67 {
        x.set8(0x67, 0);
    }
    apply_pending(x);
    next(x);
    x.set8(f::SUB_STEP, 0);
    stun(x);
    anim(x, n);
    hurt(x);
    x.set8(f::COUNTER, 1);
    speeds(x, vx, 0, vz, g);
    face_attacker(x);
}

/// 0x3c58c : projection dont la hauteur dépend du code de réaction (table 0x3c5f2), animation 9.
fn pop_by_code(x: &mut Cx) {
    x.set8(0x67, 0);
    apply_pending(x);
    next(x);
    x.set8(f::SUB_STEP, 0);
    stun(x);
    anim(x, 9);
    hurt(x);
    x.set8(f::COUNTER, 1);
    let i = (x.u8(f::REACTION) as u32).wrapping_sub(68) & 0xffff;
    x.c.d[0] = i;
    let at = 0x3c5f2u32.wrapping_add(i as u16 as i16 as i32 as u32);
    let (vz, g) = (x.rom16(at), x.rom16(at + 2));
    speeds(x, 0x100, 0, vz, g);
    face_attacker(x);
}
fn pop_10(x: &mut Cx) {
    pop(x, 10, 64, 0xc00, -128i16 as u16, true);
}
fn pop_11(x: &mut Cx) {
    pop(x, 11, 0x600, 0xf800, -64i16 as u16, false);
}

/// 0x3c5fa : chute qui anime en descendant.
fn fall_animated(x: &mut Cx) {
    move_xz(x);
    if x.u16(f::HEIGHT) as i16 > 0 {
        if x.u8(f::BLOCKED) != 0 {
            x.set16(f::VX, 0);
            x.set16(f::VY, 0);
            x.set8(f::COUNTER, 0);
        }
        if (x.u8(f::VZ) as i8) < 0 {
            anim_step(x);
        }
        return;
    }
    next(x);
    x.set32(f::HEIGHT, 0);
    x.set32(f::VZ, 0);
    landed(x);
}

fn held_start(x: &mut Cx) {
    x.set8(0x67, 0);
    let px = x.u16(f::X);
    x.set16(f::X_SAVED, px);
    let n = x.u16(f::RECOVER).wrapping_sub(10);
    x.set16(f::RECOVER, n);
    x.set8(f::ACTIVE, 2);
    hurt(x);
    next(x);
    stun(x);
    x.set8(f::COUNTER, 16);
    anim(x, 8);
}
fn held_shake(x: &mut Cx) {
    if x.u8(f::COUNTER) == 0 {
        next(x);
        let px = x.u16(f::X_SAVED);
        return x.set16(f::X, px);
    }
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    shake(x, 2);
}
fn held_end(x: &mut Cx) {
    if x.u8(f::SUB) == 6 {
        next_clear(x);
        return anim_step(x);
    }
    x.set32(0x58, 0);
    if x.u16(f::RECOVER) as i16 <= 0 {
        x.set8(f::ACTIVE, 2);
    }
    if x.u8(f::CLASS) == BOSS {
        x.call(0xff54);
        return anim_step(x);
    }
    x.set32(0x16, 0);
    x.set8(f::ACTIVE, 1);
    x.set32(f::STATE, 0x0404_0200);
    anim_step(x);
}

/// 0x3c7fa : deux éclats (EFFCT type 8, variantes 1 et 0) si le décor le permet.
fn sparks(x: &mut Cx) {
    next(x);
    if x.a5_u16(0x7e68) < 2 {
        return;
    }
    for v in [1u8, 0] {
        x.c.d[5] = v as u32;
        // Glitch d'origine : l'échec de l'allocation n'est pas testé (voir `Pool::alloc_unchecked`).
        let o = x.alloc_unchecked(&pools::EFFCT);
        o.set_u8(x.b, 0, 1);
        o.set_u8(x.b, 2, 8);
        o.set_u8(x.b, 3, v);
        o.set_u16(x.b, 0x5c, x.o.0 as u16);
    }
    x.c.d[5] = 0xffff;
}

/// 0x3c828 : désintégration (ENEMY de type 1 : 0xeb448, sinon 0xe35a6), puis clignotement et disparition.
fn disintegrate(x: &mut Cx) -> bool {
    if x.u8(f::STEP) == 0 {
        next(x);
        x.set8(f::SUB, 6);
        x.set8(f::ACTIVE, 2);
        x.set8(0x81, 0);
        x.set8(0x80, 0);
        anim(x, 12);
        let r = if x.u8(f::KIND) == 1 { 0xeb448 } else { 0xe35a6 };
        x.call(r);
        return true;
    }
    if (x.u16(f::ANIM_END) as i16) >= 0 {
        anim_step(x);
        return true;
    }
    if x.u8(f::SUB_STEP) == 0 {
        x.set8(f::COUNTER, 40);
        next_sub(x);
        x.call(0x27ee);
    }
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c == 0 {
        x.set32(f::STATE, 0x0600_0000);
        return true;
    }
    blink(x, c);
    true
}

/// 0x3c8d8 : étourdi : délai tiré au hasard (table 0x3c904), des étoiles (0xabec8) à la fin de chaque délai.
fn dazed(x: &mut Cx) {
    x.set8(0x67, 0);
    apply_pending(x);
    next(x);
    anim(x, 3);
    face_attacker(x);
    dazed_delay(x);
}
fn dazed_delay(x: &mut Cx) {
    let r = random(x) & 3;
    x.c.d[0] = x.c.d[0] & !0xffff | r;
    let d = x.rom8(0x3c904 + r);
    x.set8(f::COUNTER, d);
    hurt(x);
}
fn dazed_wait(x: &mut Cx) {
    if !paused(x) {
        next(x);
    }
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c != 0 {
        return;
    }
    effect(x, 0, 0);
    if x.c.sr & m68k::SR_C == 0 {
        return dazed_delay(x);
    }
    x.set8(f::COUNTER, 1);
}
/// 0x3c936 : projection prédéfinie 1, gel d'une image.
fn knock(x: &mut Cx) {
    hurt(x);
    launch(x, 1);
    x.set8(f::FREEZE, 1);
    if flipped(x) {
        neg32(x, f::VX);
    }
}

/// 0x3c970 : grande projection rasante (étoiles 0xabec8(2)).
fn blast(x: &mut Cx) {
    x.set8(0x67, 0);
    next(x);
    apply_pending(x);
    x.set8(f::SUB_STEP, 0);
    x.set8(f::COUNTER, 0xff);
    hurt(x);
    stun(x);
    x.set16(f::VX, 0x800);
    x.set16(f::VY, 0);
    x.set32(f::VZ, 0x0100_0000);
    x.set16(f::GRAVITY, 0);
    anim(x, 1);
    if flipped(x) {
        neg16(x, f::VX);
    }
    x.set8(f::COUNTER, 18);
    effect(x, 2, 0);
}

/// 0x3c9ec : animation 19 (avant une projection).
fn wind_up(x: &mut Cx) {
    x.set8(0x67, 0);
    apply_pending(x);
    next(x);
    x.set8(f::SUB_STEP, 0);
    anim(x, 19);
}
/// 0x3ca04 : l'animation joue tant que le jeu est en pause, puis pas suivant.
fn wind_up_wait(x: &mut Cx) {
    anim_step(x);
    if !paused(x) {
        next(x);
    }
}
/// 0x3ca18 / 0x3ca3a : projection prédéfinie 3 (animation 2), les PV à zéro pour la première.
fn launch_3(x: &mut Cx, kill: bool) {
    anim(x, 2);
    if kill {
        x.set16(f::HP, 0);
    }
    hurt(x);
    launch(x, 3);
    if flipped(x) {
        neg32(x, f::VX);
    }
}
fn launch_3_kill(x: &mut Cx) {
    launch_3(x, true);
}
fn launch_3_hurt(x: &mut Cx) {
    launch_3(x, false);
}

/// 0x3ca96 : saut vertical (animation 0).
fn hop(x: &mut Cx) {
    x.set8(0x67, 0);
    apply_pending(x);
    next(x);
    if dead(x) {
        mark_dead(x);
    }
    x.c.d[0] = 0;
    x.set16(f::VZ, 0x400);
    x.set16(f::GRAVITY, 64);
    x.set32(f::VX, 0);
    anim(x, 0);
}
/// 0x3cacc : monte jusqu'au haut de l'écran (0x110 au-dessus de la caméra), puis 0x3cafc.
fn hop_rise(x: &mut Cx) {
    move_xz(x);
    let camera = if x.a5_u8(0x36) != 0 { x.a5_u16(0x55a) } else { x.a5_u16(0x50a) };
    x.c.d[0] = x.c.d[0] & !0xffff | camera as u32;
    let d1 = x.u16(f::HEIGHT).wrapping_add(x.u16(f::Y)).wrapping_sub(camera);
    x.c.d[1] = x.c.d[1] & !0xffff | d1 as u32;
    if d1 >= 0x110 {
        next(x);
    }
    hop_top(x);
}
/// 0x3cafc : quand l'objet lié (`+0x54`) est au sous-pas 4, retombe d'une hauteur aléatoire (animation 11).
fn hop_top(x: &mut Cx) {
    let a1 = x.u16(0x54) as i16 as i32 as u32;
    x.c.a[1] = a1;
    if rd8(x.b, a1 + 7) != 4 {
        return;
    }
    let r = (random(x) & 7) << 4;
    x.c.d[0] = x.c.d[0] & !0xffff | r;
    x.add16(f::HEIGHT, r as u16);
    x.set8(f::STEP, 6);
    x.set16(f::VZ, 0xfc00);
    x.set16(f::GRAVITY, -128i16 as u16);
    anim(x, 11);
}
/// 0x3cb32 : chute ; au sol, son et 0x2824 (D0 = 8).
fn hop_land(x: &mut Cx) {
    fall(x);
    if x.u16(f::HEIGHT) != 0 {
        return;
    }
    hurt(x);
    x.c.d[0] = x.c.d[0] & !0xffff | 8;
    x.call(0x2824);
}

/// 0x3cbaa : projection vers l'arrière (compteur 1, X 0x200, hauteur 0x600).
fn knockback_speeds(x: &mut Cx) {
    next(x);
    x.set8(f::COUNTER, 1);
    x.set32(f::VX, 0x0200_0000);
    x.set16(f::VZ, 0x600);
    x.set16(f::GRAVITY, -64i16 as u16);
}
/// Son du coup, ou mort et son de la mort, l'objet restant actif (`+0` = 2).
fn hurt_active(x: &mut Cx) {
    x.set8(f::ACTIVE, 2);
    if dead(x) {
        x.set8(f::SUB, 6);
        death_sound(x);
    } else {
        hit_sound(x);
    }
}
/// 0x3cc3e : à terre, la liaison repasse en attente, animation 13.
fn tumble_anim(x: &mut Cx) {
    x.set8(0x67, 0);
    x.set8(f::DOWNED, 0);
    defer_link(x);
    anim(x, 13);
}
/// 0x3cb6c : projection vers l'arrière, culbute.
fn tumble(x: &mut Cx) {
    x.set8(0x67, 0);
    x.set16(f::VY2, 0);
    x.set16(f::VX2, 64);
    x.set8(f::GRABBED, 0);
    knockback_speeds(x);
    face_attacker(x);
    hurt_active(x);
    tumble_anim(x);
}
/// 0x3cbca : comme la culbute, animation 18.
fn tumble_18(x: &mut Cx) {
    x.set8(0x67, 0);
    x.set8(f::DOWNED, 0);
    x.set8(f::GRABBED, 0);
    knockback_speeds(x);
    face_attacker(x);
    hurt_active(x);
    defer_link(x);
    anim(x, 18);
}
/// 0x3cc00 : culbute, animation 18 et étoiles 0xabec8(5).
fn tumble_18_stars(x: &mut Cx) {
    x.set8(0x67, 0);
    x.set8(f::DOWNED, 0);
    x.set8(f::GRABBED, 0);
    knockback_speeds(x);
    face_attacker(x);
    anim(x, 18);
    effect(x, 5, 0);
    hurt_active(x);
}
/// 0x3cd0c : culbute, animation 13 et étoiles 0xabec8(7).
fn tumble_13_stars(x: &mut Cx) {
    x.set8(0x67, 0);
    x.set16(f::VY2, 0);
    x.set16(f::VX2, 64);
    x.set8(f::GRABBED, 0);
    x.set8(f::DOWNED, 0);
    knockback_speeds(x);
    face_attacker(x);
    anim(x, 13);
    effect(x, 7, 0);
    hurt_active(x);
}
/// 0x3cc6e : montée en trois dimensions ; au sommet, pas suivant (actif s'il n'est pas mort).
fn tumble_rise(x: &mut Cx) {
    move_xyz(x);
    if x.u16(f::VZ) as i16 > 0 {
        return tumble_blocked(x);
    }
    next(x);
    if x.u8(f::SUB) != 6 {
        x.set8(f::ACTIVE, 1);
    }
}
/// 0x3cc90 : descente en trois dimensions jusqu'au sol.
fn tumble_fall(x: &mut Cx) {
    move_xyz(x);
    if x.u16(f::HEIGHT) as i16 > 0 {
        return tumble_blocked(x);
    }
    next_clear(x);
    x.set32(f::HEIGHT, 0);
    x.set32(f::VZ, 0);
    landed(x);
}
fn tumble_blocked(x: &mut Cx) {
    if x.u8(f::BLOCKED) != 0 {
        x.set16(f::VX, 0);
        x.set16(f::VY, 0);
        x.set8(f::COUNTER, 0);
    }
    anim_step(x);
}
/// 0x3ccca : pas suivant, la liaison en attente devient effective.
fn tumble_link(x: &mut Cx) {
    next(x);
    take_pending(x);
}

/// 0x3cdb0 / 0x3cf1e : renversé (animation 17, son 0xebbda) ; la seconde ajoute les étoiles 0xabec8(8).
fn floored(x: &mut Cx, stars: bool) {
    x.set8(0x67, 0);
    x.set8(f::DOWNED, 0);
    next(x);
    x.set8(f::GRABBED, 0);
    if dead(x) {
        mark_dead(x);
    }
    x.c.d[0] = 1;
    speeds(x, 0x200, 0, 0x400, -128i16 as u16);
    face_attacker(x);
    if !stars {
        defer_link(x);
    }
    anim(x, 17);
    x.call(0xebbda);
    if stars {
        effect(x, 8, 0);
    }
}
fn floored_plain(x: &mut Cx) {
    floored(x, false);
}
fn floored_stars(x: &mut Cx) {
    floored(x, true);
}
/// 0x3ce4c / 0x3ce84 : 30 images au sol ; la première variante lance 0xab7d8(1).
fn floored_wait(x: &mut Cx, cry: bool) {
    if x.u8(f::SUB_STEP) == 0 {
        next_sub(x);
        x.set8(f::COUNTER, 30);
    }
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c != 0 {
        return;
    }
    next_clear(x);
    x.set8(f::COUNTER, 0);
    if cry {
        x.c.d[0] = 1;
        x.call(0xab7d8);
    }
}
fn floored_wait_cry(x: &mut Cx) {
    floored_wait(x, true);
}
fn floored_wait_quiet(x: &mut Cx) {
    floored_wait(x, false);
}
/// 0x3ceb4 / 0x3cecc : hors pause, pas suivant (la première lance 0xab7d8(1)).
fn unpaused_cry(x: &mut Cx) {
    if paused(x) {
        return;
    }
    next_clear(x);
    x.c.d[0] = 1;
    x.call(0xab7d8);
}
fn unpaused_next(x: &mut Cx) {
    if paused(x) {
        return;
    }
    next_clear(x);
}

/// 0x3d6f8 : renversé sans liaison (animation 17, étoiles 0xabec8(9)), vitesses orientées.
fn floored_9(x: &mut Cx) {
    x.set8(0x67, 0);
    x.set8(f::DOWNED, 0);
    next(x);
    x.set8(f::GRABBED, 0);
    x.set8(f::ACTIVE, 2);
    if dead(x) {
        x.set8(f::SUB, 6);
    }
    x.c.d[0] = 1;
    speeds(x, 0x200, 0, 0x400, -128i16 as u16);
    orient_speeds(x);
    anim(x, 17);
    x.call(0xebbda);
    effect(x, 9, 0);
}

// ---------------------------------------------------------------------------------------------
// Saisies et réactions particulières

/// 0x3d00a : saisi par un joueur (animation 14), immunités totales.
fn seized(x: &mut Cx) {
    x.set32(0x94, u32::MAX);
    apply_pending(x);
    next(x);
    x.set8(0x67, 0);
    x.set8(f::SUB_STEP, 0);
    anim(x, 14);
}
/// 0x3d02a : retour à l'attente de la saisie.
fn seized_again(x: &mut Cx) {
    x.set8(f::SUB_STEP, 2);
    anim(x, 14);
}
/// 0x3d036 : la saisie (`+7`) : se colle au joueur, puis subit ses coups.
fn seized_by(x: &mut Cx) {
    let s = x.u8(f::SUB_STEP);
    x.c.d[0] = x.c.d[0] & !0xff | s as u32;
    match s {
        0 => seized_stick(x),
        2 => seized_hits(x),
        4 => {
            let c = x.u8(f::COUNTER).wrapping_sub(1);
            x.set8(f::COUNTER, c);
            if c == 0 {
                return seized_again(x);
            }
            shake(x, 2);
        }
        _ => {}
    }
}
/// 0x3d048 : quand le joueur (`+0x50`) signale la prise, l'objet se place devant lui.
fn seized_stick(x: &mut Cx) {
    let p = x.u16(f::OWNER) as i16 as i32 as u32;
    x.c.a[6] = p;
    if rd8(x.b, p + 0x33) != 1 {
        return;
    }
    next_sub(x);
    let px = rd16(x.b, p + 8);
    x.set16(f::X, px);
    let i = 4 * x.u8(f::KIND) as u32;
    let mut d = x.rom16(0x3d07e + i);
    if !flipped(x) {
        d = d.wrapping_neg();
    }
    x.c.d[0] = d as u32;
    x.add16(f::X, d);
}
/// 0x3d0e8 : le joueur (personnage 5, état 0x02000cxx, action 26) frappe : dégâts selon son coup (`+0x10c`,
/// table 0x3d0da), points (0x19dce), effet (0xb141a) ; sinon fin de la saisie.
fn seized_hits(x: &mut Cx) {
    let p = x.u16(f::OWNER) as i16 as i32 as u32;
    x.c.a[6] = p;
    let character = rd8(x.b, p + 3) >> 1;
    if character != 5 || rd32(x.b, p + 4) & 0xffff_ff00 != 0x0200_0c00 || rd8(x.b, p + 0x6a) != 26 {
        return next_clear(x);
    }
    if (rd8(x.b, p + 0x33) as i8) >= 0 {
        return;
    }
    let hit = (rd8(x.b, p + 0x10c).wrapping_sub(11)) as u32;
    let damage = x.rom8(0x3d0da + hit) as u16;
    x.c.d[0] = damage as u32;
    next_sub(x);
    crate::mem::wr8(x.b, p + 0x33, 0);
    let hp = x.u16(f::HP).wrapping_sub(damage);
    x.set16(f::HP, if (hp as i16) > 0 { hp } else { 0 });
    x.set8(f::COUNTER, 7);
    anim_step(x);
    // 0x19dce avec A0 = le joueur, A1 = l'objet saisi
    let me = x.o.0;
    let owner = x.u16(f::OWNER) as i16 as i32 as u32;
    let d0 = ((rd8(x.b, owner + 2) as u32) << 8) as u16;
    let d0 = d0 & 0xff00 | (d0 as u8).wrapping_add(x.u8(f::KIND)) as u16;
    x.c.d[0] = d0 as u32;
    x.c.d[1] = x.c.d[1] & !0xffff | x.u16(0x60) as u32;
    x.c.d[2] = x.c.d[2] & !0xffff | x.u16(f::HP) as u32;
    x.c.d[3] = x.c.d[3] & !0xff | x.u8(f::CLASS) as u32;
    x.c.a[1] = me;
    x.c.a[0] = owner;
    x.c.a[5] = crate::mem::A5;
    crate::call(x.c, x.b, 0x19dce);
    x.c.a[0] = me;
    let i = 4 * x.u8(f::KIND) as u32;
    x.c.d[0] = i;
    let mut d2 = x.rom16(0x3d198 + i);
    let d3 = x.rom16(0x3d19a + i);
    if flipped(x) {
        d2 = d2.wrapping_neg();
    }
    x.c.d[2] = x.c.d[2] & !0xffff | d2 as u32;
    x.c.d[3] = x.c.d[3] & !0xffff | d3 as u32;
    x.call(0xb141a);
}
/// 0x3d224 : lâché : projection prédéfinie 1, immunités du type (0xfe5a).
fn released(x: &mut Cx) {
    x.set8(f::ACTIVE, 1);
    anim(x, 3);
    hurt(x);
    launch(x, 1);
    x.set8(f::FREEZE, 1);
    if flipped(x) {
        neg32(x, f::VX);
    }
    x.call(0xfe5a);
}

/// 0x3d26a : coup reçu par un objet saisi (`+0x87`) ou non ; selon la hauteur, recul (code 0) ou coup bas (56).
fn grab_hit(x: &mut Cx) {
    if x.u8(f::GRABBED) == 0 {
        if x.u16(f::HEIGHT) as i16 > 0 {
            x.set8(f::REACTION, 56);
            return run_entry(x, 0x3c2ee);
        }
        x.set8(f::REACTION, 0);
        return run_entry(x, 0x3bb9c);
    }
    if dead(x) {
        mark_dead(x);
        if x.u16(f::HEIGHT) as i16 > 0 {
            x.set8(f::REACTION, 56);
            return run_entry(x, 0x3c2ee);
        }
        x.set8(f::REACTION, 0);
        return run_entry(x, 0x3dd72);
    }
    x.set8(0x67, 0);
    x.set8(f::COUNTER, 16);
    apply_pending(x);
    if x.u8(f::DOWNED) != 0 {
        x.set8(f::REACTION, 108);
        x.set8(0x67, 0);
        x.set8(f::COUNTER, 16);
        return run_entry(x, 0x3d350);
    }
    if x.u16(f::HEIGHT) as i16 <= 0 {
        x.set8(0x67, 0);
        x.set32(f::HEIGHT, 0);
        x.set8(f::STEP, 10);
        anim(x, 15);
        return effect(x, 3, 0);
    }
    anim(x, 2);
    let flip = x.u8(f::FLIP) ^ 0x20;
    x.c.d[0] = x.c.d[0] & !0xff | flip as u32;
    x.set8(f::ATTACKER_FLIP, flip);
    x.c.d[0] = 0;
    launch(x, 0);
    face_attacker(x);
}
/// 0x3d318 : à terre saisi : `+0x87` s'use, puis relevé par la glissade.
fn grab_down(x: &mut Cx) {
    let c = x.u8(f::COUNTER);
    if c != 0 {
        x.set8(f::COUNTER, c - 1);
        if c == 1 {
            x.set32(0x58, 0);
            x.set8(f::ACTIVE, 1);
        }
    }
    let g = x.u8(f::GRABBED).wrapping_sub(1);
    x.set8(f::GRABBED, g);
    if g == 0 {
        return slide_end(x);
    }
    anim_step(x);
}
/// 0x3d350 (code 108) : à terre et saisi ; mort par 0x3c63a.
fn grounded_grab(x: &mut Cx) -> bool {
    if dead(x) {
        x.set8(f::SUB, 6);
        x.set8(f::REACTION, 76);
        x.set8(f::ACTIVE, 2);
        return run_entry_flag(x, 0x3c63a);
    }
    let g = x.u8(f::GRABBED).wrapping_sub(1);
    x.set8(f::GRABBED, g);
    if g == 0 {
        get_up(x);
        return true;
    }
    let c = x.u8(f::COUNTER);
    if c != 0 {
        x.set8(f::COUNTER, c - 1);
        if c == 1 {
            x.set32(0x58, 0);
            x.set8(f::ACTIVE, 1);
        }
    }
    if x.u8(f::STEP) != 0 {
        return true;
    }
    next(x);
    anim(x, 16);
    effect(x, 3, 1);
    true
}

/// 0x3d3c8 : tremble sur place (étoiles 0xabec8(1)), délai tiré au hasard (table 0x3d42a).
fn quiver(x: &mut Cx) {
    x.set8(0x67, 0);
    x.set8(f::DOWNED, 0);
    apply_pending(x);
    x.set8(f::ACTIVE, 2);
    effect(x, 1, 0);
    anim(x, 3);
    x.set8(f::DOWNED, 0);
    next(x);
    x.set8(f::COUNTER, 0);
    x.set32(f::VX, 0x0180_0000);
    x.set16(f::VZ, 0x400);
    x.set16(f::GRAVITY, -64i16 as u16);
    x.set32(f::VX2, 0);
    quiver_delay(x);
    orient_speeds(x);
    stun(x);
    hurt(x);
}
fn quiver_delay(x: &mut Cx) {
    let r = random(x) & 3;
    x.c.d[0] = x.c.d[0] & !0xffff | r;
    let d = x.rom8(0x3d42a + r);
    x.set8(f::COUNTER, d);
}
/// 0x3d42e : le tremblement ; à chaque délai, des étoiles une fois sur quatre... trois sur quatre.
fn quiver_shake(x: &mut Cx) {
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if (c as i8) <= 0 {
        let r = random(x) & 3;
        x.c.d[0] = x.c.d[0] & !0xffff | r;
        if r != 0 {
            effect(x, 1, 1);
        }
        quiver_delay(x);
    }
    shake(x, 1);
    if !paused(x) {
        next(x);
    }
}

/// 0x3d4a6 / 0x3d522 : culbute basse (X 0x100, hauteur 0x800) ; la seconde a l'animation 13 et ses étoiles.
fn roll(x: &mut Cx, stars: bool) {
    x.set8(0x67, 0);
    next(x);
    x.set8(f::GRABBED, 0);
    if stars {
        x.set8(f::DOWNED, 0);
    }
    x.set8(f::FREEZE, 1);
    x.set8(f::COUNTER, 1);
    x.c.d[0] = 0;
    speeds(x, 0x100, 0, 0x800, -64i16 as u16);
    x.set16(f::VY2, 0);
    x.set16(f::VX2, 64);
    orient_speeds(x);
    if stars {
        anim(x, 13);
        effect(x, 7, 0);
        return hurt_active(x);
    }
    // 0x3d4e6 : mort, les deux sons ; puis l'animation de la culbute
    x.set8(f::ACTIVE, 2);
    if dead(x) {
        x.set8(f::SUB, 6);
        death_sound(x);
    }
    hit_sound(x);
    tumble_anim(x);
}
fn roll_plain(x: &mut Cx) {
    roll(x, false);
}
fn roll_stars(x: &mut Cx) {
    roll(x, true);
}

/// 0x3d5a2 : électrocuté (80 images, animation 0), il meurt.
fn zapped(x: &mut Cx) {
    x.set8(0x67, 0);
    apply_pending(x);
    next(x);
    x.set8(f::ACTIVE, 2);
    x.set8(f::SUB, 6);
    x.set8(f::COUNTER, 80);
    anim(x, 0);
}
/// 0x3d5c6 : étincelles toutes les 8 images (0xb10c8), puis disparition.
fn zapped_burn(x: &mut Cx) {
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c == 0 {
        x.call(0xb3c14);
        x.set32(f::STATE, 0x0600_0000);
        x.call(0x27ee);
        return x.call(call::DEATH);
    }
    x.c.d[0] = x.c.d[0] & !0xff | (c & 7) as u32;
    if c & 7 != 0 {
        return;
    }
    x.c.d[0] = 7;
    x.c.d[1] = x.c.d[1] & !0xffff | 0x200;
    x.c.d[2] = 0;
    x.c.d[3] = x.c.d[3] & !0xffff | 0xffff;
    x.call(0xb10c8);
    if x.c.sr & m68k::SR_C != 0 {
        return;
    }
    let o = Obj(x.c.a[1]);
    o.set_u8(x.b, 0x43, 1);
    let h = o.u16(x.b, 0xc).wrapping_add(16);
    o.set_u16(x.b, 0xc, h);
}

/// 0x3d62c : pétrifié (animation 20) ; en l'air, il tombe d'abord.
fn petrified(x: &mut Cx) {
    x.set8(0x67, 0);
    apply_pending(x);
    next(x);
    x.set8(f::SUB, 6);
    x.set8(f::ACTIVE, 2);
    defer_link(x);
    anim(x, 20);
    if x.u16(f::HEIGHT) as i16 <= 0 {
        next(x);
        x.set8(f::COUNTER, 40);
        return x.call(0xebc4c);
    }
    x.set16(f::VZ, 0);
    x.set16(f::GRAVITY, -64i16 as u16);
    x.set32(f::VX, 0);
}
fn petrified_fall(x: &mut Cx) {
    move_xz(x);
    if x.u16(f::HEIGHT) as i16 > 0 {
        return;
    }
    x.set32(f::HEIGHT, 0);
    next(x);
    x.set8(f::COUNTER, 1);
    landed(x);
}
fn petrified_end(x: &mut Cx) {
    if paused(x) {
        return;
    }
    x.c.d[0] = 5;
    x.call(0xab7d8);
    x.set32(f::STATE, 0x0600_0000);
    x.call(0x27ee);
    x.call(call::DEATH);
}

/// 0x3d762 (code 0x5c...) : en l'air, projection (code 4) ; mort au sol ; sinon tremblement de 60 images puis
/// glissade.
fn stagger(x: &mut Cx) {
    x.set8(0x67, 0);
    if x.u16(f::HEIGHT) as i16 > 0 {
        x.set8(f::REACTION, 4);
        return run_entry(x, 0x3bc24);
    }
    if dead(x) {
        mark_dead(x);
        return run_entry(x, 0x3dd72);
    }
    next(x);
    x.c.d[0] = 5;
    x.c.d[1] = 0;
    x.set8(f::COUNTER, 60);
    stun(x);
    x.set16(f::VX, 0x180);
    x.set16(f::VY, -56i16 as u16);
    face_attacker(x);
    anim(x, 0);
}
fn stagger_shake(x: &mut Cx) {
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c == 40 {
        x.set32(0x58, 0);
    }
    shake(x, 2);
    if x.u8(f::COUNTER) == 0 {
        next(x);
    }
}

/// 0x3d828 : petite projection (animation 3), vitesses orientées.
fn hop_back(x: &mut Cx) {
    apply_pending(x);
    next(x);
    if dead(x) {
        mark_dead(x);
    }
    anim(x, 3);
    x.set8(f::COUNTER, 1);
    x.set32(f::VX, 0x0200_0000);
    x.set16(f::VZ, 0x400);
    x.set16(f::GRAVITY, -64i16 as u16);
    x.set32(f::VX2, 0);
    orient_speeds(x);
    stun(x);
}

/// 0x3d882 : brûlé (0xe909a), animation D0 laissé tel quel, six étoiles (0xabec8(4)).
fn burnt(x: &mut Cx) {
    x.set8(0x67, 0);
    x.call(0xe909a);
    apply_pending(x);
    next(x);
    x.set8(f::ACTIVE, 1);
    if x.u16(f::HEIGHT) as i16 > 0 {
        x.set16(f::VZ, 0);
        x.set16(f::GRAVITY, -64i16 as u16);
        x.set32(f::VX, 0);
        x.add8(f::STEP, (-2i8) as u8);
    }
    next(x);
    x.set8(f::DOWNED, 0);
    x.set8(f::GRABBED, 0);
    x.set8(f::COUNTER, 0xf0);
    x.c.d[6] = 15;
    let n = x.c.d[0];
    anim(x, n);
    x.c.d[0] = 4;
    x.c.d[1] = 5;
    loop {
        x.call(call::SPAWN_EFFECT);
        if x.c.sr & m68k::SR_C != 0 {
            return x.set8(f::STEP, 6);
        }
        let n = (x.c.d[1] as u16).wrapping_sub(1);
        x.c.d[1] = x.c.d[1] & !0xffff | n as u32;
        if n == 0xffff {
            return;
        }
    }
}
fn burnt_fall(x: &mut Cx) {
    move_xz(x);
    if x.u16(f::HEIGHT) as i16 > 0 {
        return;
    }
    x.set32(f::HEIGHT, 0);
    next(x);
}
fn burnt_wait(x: &mut Cx) {
    if x.u16(f::HEIGHT) != 0 {
        move_xz(x);
        if x.u16(f::HEIGHT) as i16 > 0 {
            return;
        }
        x.set32(f::HEIGHT, 0);
    }
    anim_step(x);
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c == 0 {
        slide_end(x);
    }
}

/// Angle de départ d'un tourbillon : au hasard, centré à gauche (8) ou à droite (24) selon l'orientation.
fn spin_angle(x: &mut Cx) {
    let d2 = if flipped(x) { 24 } else { 8 };
    x.c.d[2] = d2;
    let r = random(x);
    let a = (r as u16).wrapping_add(d2 as u16 & 15) & 31;
    x.c.d[0] = x.c.d[0] & !0xffff | a as u32;
    x.set8(f::ANGLE, a as u8);
    anim(x, 3);
}
/// 0x3d950 : emporté par un tourbillon.
fn whirl(x: &mut Cx) {
    x.set8(0x67, 0);
    x.set8(f::DOWNED, 0);
    apply_pending(x);
    next(x);
    x.c.d[0] = 0;
    x.set32(f::VX, 0);
    x.set32(f::VZ, 0);
    x.set32(f::VX2, 0);
    x.set8(f::GRABBED, 0);
    x.set8(f::DOWNED, 0);
    let c = if x.c.d[7] & 1 != 0 { 1 } else { 2 };
    x.set8(f::COUNTER, c);
    spin_angle(x);
}
/// Tour suivant de l'angle (pas de 2 sur 32) ; l'orientation suit le demi-cercle.
fn turn_angle(x: &mut Cx) {
    let a = x.u8(f::ANGLE).wrapping_add(2) & 31;
    x.set8(f::ANGLE, a);
    x.set8(f::FLIP, 0);
    let d = a.wrapping_sub(8);
    x.c.d[0] = x.c.d[0] & !0xff | d as u32;
    if d >= 16 {
        x.set8(f::FLIP, 32);
    }
}
/// 0x3da0e : position sur le cercle (table 0x1c94b6 : X, profondeur), vitesses ajoutées.
fn orbit(x: &mut Cx) {
    const CIRCLE: u32 = 0x1c94b6;
    x.c.a[4] = CIRCLE;
    let i = 4 * x.u8(f::ANGLE) as u32;
    x.c.d[0] = i;
    let vx = rd16(x.b, CIRCLE + i).wrapping_add(x.u16(f::VY));
    x.set16(f::VX, vx);
    x.add32(f::X, ((vx as i16 as i32) << 8) as u32);
    let vy = rd16(x.b, CIRCLE + i + 2).wrapping_add(x.u16(f::VY2));
    x.set16(f::VX2, vy);
    let d = ((vy as i16 as i32) << 8) as u32;
    x.c.d[1] = d;
    x.add32(f::Y, d);
}
/// 0x3d9b2 : monte en tournant jusqu'à 96, puis retombe.
fn whirl_rise(x: &mut Cx) {
    x.c.a[4] = 0x1c94b6;
    x.add32(f::HEIGHT, 0x2_0000);
    if x.u16(f::HEIGHT) as i16 >= 96 {
        next(x);
        x.set16(f::VZ, 0x200);
        x.set16(f::GRAVITY, -128i16 as u16);
        return;
    }
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c == 0 {
        x.set8(f::COUNTER, 1);
        turn_angle(x);
    }
    orbit(x);
}
/// 0x3da44 : retombe ; au sol, atterrissage et son.
fn whirl_fall(x: &mut Cx) {
    anim_step(x);
    move_xyz(x);
    if x.u16(f::HEIGHT) as i16 > 0 {
        if x.u8(f::BLOCKED) != 0 {
            x.set16(f::VX, 0);
            x.set16(f::VY, 0);
            x.set8(f::COUNTER, 0);
        }
        return;
    }
    next(x);
    x.set32(f::HEIGHT, 0);
    x.set32(f::VZ, 0);
    landed(x);
    hurt(x);
}

/// 0x3dc30 : tourbillon des boss (60 images).
fn spin(x: &mut Cx) {
    next(x);
    x.set8(f::COUNTER, 60);
    apply_pending(x);
    x.c.d[0] = 0;
    x.set32(f::VX, 0);
    x.set32(f::VZ, 0);
    x.set32(f::VX2, 0);
    let g = if x.c.d[7] & 1 != 0 { 1 } else { 2 };
    x.set8(f::GRABBED, g);
    spin_angle(x);
}
/// 0x3dc86 : tourne en montant jusqu'à 64.
fn spin_rise(x: &mut Cx) {
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c == 0 {
        return next(x);
    }
    x.c.a[4] = 0x1c94b6;
    x.add32(f::HEIGHT, 0x2_0000);
    if x.u16(f::HEIGHT) as i16 >= 64 {
        x.set32(f::HEIGHT, 0x0040_0000);
    }
    let g = x.u8(f::GRABBED).wrapping_sub(1);
    x.set8(f::GRABBED, g);
    if g == 0 {
        x.set8(f::GRABBED, 1);
        turn_angle(x);
    }
    orbit(x);
}
/// 0x3db4e : le boss redevient actif (état 4) et retombe renversé (animation 17).
fn spin_drop(x: &mut Cx) {
    x.set8(f::STATE, 4);
    x.set8(f::ACTIVE, 2);
    next(x);
    x.set8(f::COUNTER, 30);
    x.set8(f::SUB_STEP, 0);
    x.set8(0x67, 0);
    x.set8(f::DOWNED, 0);
    x.set8(f::GRABBED, 0);
    x.set32(f::VX, 0);
    x.set16(f::VZ, 0x600);
    x.set16(f::GRAVITY, -128i16 as u16);
    orient_speeds(x);
    defer_link(x);
    anim(x, 17);
    x.call(0xebbda);
}
/// 0x3dada : variante : animation 17 et étoiles 0xabec8(8).
fn spin_drop_stars(x: &mut Cx) {
    x.set8(0x67, 0);
    next(x);
    x.set8(f::COUNTER, 30);
    x.set8(f::SUB_STEP, 0);
    x.set32(f::VX, 0);
    x.set16(f::VZ, 0x600);
    x.set16(f::GRAVITY, -128i16 as u16);
    orient_speeds(x);
    anim(x, 17);
    x.call(0xebbda);
    effect(x, 8, 0);
}
/// 0x3dbb4 : animation ; chute jusqu'au sol, puis attente de la fin du compteur.
fn spin_fall(x: &mut Cx) {
    anim_step(x);
    if x.u8(f::SUB_STEP) == 0 {
        move_xz(x);
    }
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c == 0 {
        return next_clear(x);
    }
    if x.u16(f::HEIGHT) as i16 > 0 {
        return;
    }
    x.set32(f::HEIGHT, 0);
    next_sub(x);
}
/// 0x3dbec : rebond haut (60 images, animation 13).
fn spin_bounce(x: &mut Cx) {
    next(x);
    x.set32(f::VX, 0);
    x.set8(f::COUNTER, 60);
    x.set16(f::VZ, 0x800);
    x.set16(f::GRAVITY, -64i16 as u16);
    hit_sound(x);
    defer_link(x);
    anim(x, 13);
}
/// 0x3db18 : variante : étoiles 0xabec8(7), puis le son.
fn spin_bounce_stars(x: &mut Cx) {
    next(x);
    x.set8(f::COUNTER, 60);
    x.set32(f::VX, 0);
    x.set32(f::VX2, 0);
    x.set16(f::VZ, 0x800);
    x.set16(f::GRAVITY, -64i16 as u16);
    orient_speeds(x);
    anim(x, 13);
    effect(x, 7, 0);
    hit_sound(x);
}
/// 0x3dcea : projection rasante finale (étoiles 0xabec8(2)).
fn spin_blast(x: &mut Cx) {
    next(x);
    apply_pending(x);
    x.set8(f::COUNTER, 18);
    x.set16(f::VX, 0x1000);
    x.set16(f::VY, 0);
    x.set32(f::VZ, 0x0100_0000);
    x.set16(f::GRAVITY, 0);
    anim(x, 1);
    if flipped(x) {
        neg16(x, f::VX);
    }
    effect(x, 2, 0);
}
/// 0x3dd2e : mort du boss ; hors ENEMY, `A5+0x4f` signale la fin ; certains types gardent leur pas.
fn spin_death(x: &mut Cx) {
    next(x);
    x.set8(f::ACTIVE, 2);
    x.set16(f::HP, 0);
    x.set8(f::SUB, 6);
    if x.u8(f::CLASS) != ENEMY {
        x.set_a5_u8(0x4f, 0xff);
        return x.set16(f::STEP, 0);
    }
    if [8, 18, 27].contains(&x.u8(f::KIND)) {
        x.set16(f::STEP, 0);
    }
}

/// 0x3dd8c / 0x3dd94 : mort en l'air : son, puis projection basse (animation 2 ou 3).
fn death_throw(x: &mut Cx) {
    death_sound(x);
    throw_1(x);
}
fn death_throw_b(x: &mut Cx) {
    death_sound(x);
    throw_1b(x);
}

// ---------------------------------------------------------------------------------------------
// Les routines de réaction (entrées des tables par type)

type Step = fn(&mut Cx);

/// Aiguillage d'origine sur `+6` (`moveq #0,D0 ; move.b 6(A0),D0 ; move.w table(PC,D0),D1 ; jmp`) ; faux pour
/// un pas inconnu (le code d'origine s'en charge).
fn dispatch(x: &mut Cx, table: u32, steps: &[Step]) -> bool {
    let s = x.u8(f::STEP);
    x.c.d[0] = s as u32;
    let off = x.rom16(table + s as u32);
    x.c.d[1] = x.c.d[1] & !0xffff | off as u32;
    match steps.get(s as usize / 2) {
        Some(step) if s & 1 == 0 => {
            step(x);
            true
        }
        _ => false,
    }
}

/// Une routine de réaction : sa table de sauts et ses pas.
struct Reaction {
    entry: u32,
    table: u32,
    steps: &'static [Step],
}

/// Pas communs de fin : relevé, mort.
const AIR: [Step; 6] = [wait_anim, fall, boss_3_hook, bounce_start, bounce, get_up];
const fn standard(first: Step) -> [Step; 8] {
    [first, AIR[0], AIR[1], AIR[2], AIR[3], AIR[4], AIR[5], death]
}
const HIGH: [Step; 8] = [wait_anim, step_51c06, fall_38, skip, bounce_high, bounce, get_up, death];
const fn high(first: Step) -> [Step; 9] {
    [first, HIGH[0], HIGH[1], HIGH[2], HIGH[3], HIGH[4], HIGH[5], HIGH[6], HIGH[7]]
}
const fn slides(first: Step) -> [Step; 9] {
    [first, wait_anim, step_51c06, fall_38, skip, slide_start_fast, slide_fast, get_up, death]
}

const THROW_1: [Step; 8] = standard(throw_1);
const THROW_2: [Step; 8] = standard(throw_2);
const THROW_3: [Step; 8] = standard(throw_3);
const THROW_4: [Step; 8] = standard(throw_4);
const THROW_1B: [Step; 8] = standard(throw_1b);
const THROW_2B: [Step; 8] = standard(throw_2b);
const THROW_3B: [Step; 8] = standard(throw_3b);
const THROW_4B: [Step; 8] = standard(throw_4b);
const UPPERCUT_2: [Step; 9] = high(uppercut_2);
const UPPERCUT_3: [Step; 9] = high(uppercut_3);
const SLIDE_2: [Step; 9] = slides(uppercut_2);
const SLIDE_3: [Step; 9] = slides(uppercut_3);

static REACTIONS: &[Reaction] = &[
    Reaction { entry: 0x3bc24, table: 0x3bc32, steps: &THROW_1 },
    Reaction { entry: 0x3bc82, table: 0x3bc90, steps: &THROW_2 },
    Reaction { entry: 0x3bce2, table: 0x3bcf0, steps: &THROW_3 },
    Reaction { entry: 0x3bd42, table: 0x3bd50, steps: &THROW_4 },
    Reaction { entry: 0x3bf56, table: 0x3bf64, steps: &UPPERCUT_2 },
    Reaction { entry: 0x3bfc0, table: 0x3bfce, steps: &THROW_1B },
    Reaction { entry: 0x3c01c, table: 0x3c02a, steps: &THROW_2B },
    Reaction { entry: 0x3c07a, table: 0x3c088, steps: &THROW_3B },
    Reaction { entry: 0x3c0d4, table: 0x3c0e2, steps: &THROW_4B },
    Reaction { entry: 0x3c12e, table: 0x3c13c, steps: &UPPERCUT_3 },
    Reaction { entry: 0x3c1c2, table: 0x3c1d0, steps: &SLIDE_2 },
    Reaction { entry: 0x3c1e2, table: 0x3c1f0, steps: &SLIDE_3 },
    Reaction {
        entry: 0x3c2ee,
        table: 0x3c2fc,
        steps: &[low_hit, fall, boss_3_hook, bounce_start, bounce, get_up, death],
    },
    Reaction { entry: 0x3c35a, table: 0x3c368, steps: &[skim, skim_flight, bounce_start, bounce, get_up, death] },
    Reaction {
        entry: 0x3c4b4,
        table: 0x3c4c2,
        steps: &[throw_1, wait_anim, step_51c06, fall, slide_start_slow, slide_slow, get_up, death],
    },
    Reaction {
        entry: 0x3c570,
        table: 0x3c57e,
        steps: &[pop_by_code, wait_anim, fall_animated, bounce_start, bounce, get_up, death],
    },
    Reaction {
        entry: 0x3c70c,
        table: 0x3c71a,
        steps: &[pop_10, wait_anim, fall_animated, bounce_start, bounce, get_up, death],
    },
    Reaction {
        entry: 0x3c784,
        table: 0x3c792,
        steps: &[pop_11, step_51c06, fall_38, sparks, skip, bounce_back, bounce, get_up, death],
    },
    Reaction {
        entry: 0x3c8b8,
        table: 0x3c8c6,
        steps: &[dazed, dazed_wait, knock, wait_anim, fall, bounce_start, bounce, get_up, death],
    },
    Reaction { entry: 0x3c954, table: 0x3c962, steps: &[blast, idle, skim_flight, bounce_start, bounce, get_up, death] },
    Reaction {
        entry: 0x3c9cc,
        table: 0x3c9da,
        steps: &[wind_up, wind_up_wait, launch_3_kill, wait_anim, fall, bounce_start, bounce, get_up, death],
    },
    Reaction {
        entry: 0x3ca56,
        table: 0x3ca64,
        steps: &[wind_up, wind_up_wait, launch_3_hurt, wait_anim, fall, bounce_start, bounce, get_up, death],
    },
    Reaction {
        entry: 0x3ca76,
        table: 0x3ca84,
        steps: &[hop, hop_rise, hop_top, hop_land, sparks, bounce_back, bounce, get_up, death],
    },
    Reaction {
        entry: 0x3cb4c,
        table: 0x3cb5a,
        steps: &[tumble, tumble_rise, tumble_fall, tumble_link, boss_3_hook, bounce_start, bounce, get_up, death],
    },
    Reaction {
        entry: 0x3ccee,
        table: 0x3ccfc,
        steps: &[tumble_13_stars, tumble_rise, tumble_fall, boss_3_hook, bounce_start, bounce, get_up, death],
    },
    Reaction {
        entry: 0x3cd70,
        table: 0x3cd7e,
        steps: &[floored_plain, fall, floored_wait_cry, bounce_linked, fall, bounce_start, bounce, get_up, death],
    },
    Reaction {
        entry: 0x3cd90,
        table: 0x3cd9e,
        steps: &[floored_stars, fall, floored_wait_quiet, bounce_2, fall, bounce_start, bounce, get_up, death],
    },
    Reaction {
        entry: 0x3cede,
        table: 0x3ceec,
        steps: &[floored_plain, fall, unpaused_cry, bounce_linked, fall, bounce_start, bounce, get_up, death],
    },
    Reaction {
        entry: 0x3cefe,
        table: 0x3cf0c,
        steps: &[floored_stars, fall, unpaused_next, bounce_2, fall, bounce_start, bounce, get_up, death],
    },
    Reaction {
        entry: 0x3cf8e,
        table: 0x3cf9c,
        steps: &[tumble_18, fall, tumble_link, bounce_start, bounce, get_up, death],
    },
    Reaction { entry: 0x3cfd0, table: 0x3cfde, steps: &[tumble_18_stars, fall, bounce_start, bounce, get_up, death] },
    Reaction {
        entry: 0x3cfea,
        table: 0x3cff8,
        steps: &[seized, seized_by, released, wait_anim, fall, bounce_start, bounce, get_up, death],
    },
    Reaction { entry: 0x3d250, table: 0x3d25e, steps: &[grab_hit, fall, bounce_start, bounce, get_up, grab_down] },
    Reaction {
        entry: 0x3d3a8,
        table: 0x3d3b6,
        steps: &[quiver, quiver_shake, knock, wait_anim, fall, bounce_start, bounce, get_up, death],
    },
    Reaction {
        entry: 0x3d488,
        table: 0x3d496,
        steps: &[roll_plain, tumble_rise, tumble_fall, tumble_link, bounce_start, bounce, get_up, death],
    },
    Reaction {
        entry: 0x3d504,
        table: 0x3d512,
        steps: &[roll_stars, tumble_rise, tumble_fall, tumble_link, bounce_start, bounce, get_up, death],
    },
    Reaction { entry: 0x3d590, table: 0x3d59e, steps: &[zapped, zapped_burn] },
    Reaction { entry: 0x3d618, table: 0x3d626, steps: &[petrified, petrified_fall, petrified_end] },
    Reaction {
        entry: 0x3d6d8,
        table: 0x3d6e6,
        steps: &[floored_9, fall, unpaused_next, bounce_2, fall, bounce_start, bounce, get_up, death],
    },
    Reaction { entry: 0x3d74e, table: 0x3d75c, steps: &[stagger, stagger_shake, slide] },
    Reaction { entry: 0x3d80c, table: 0x3d81a, steps: &[hop_back, wait_anim, fall, bounce_start, bounce, get_up, death] },
    Reaction {
        entry: 0x3d932,
        table: 0x3d940,
        steps: &[whirl, whirl_rise, whirl_fall, sparks, bounce_start, bounce, get_up, death],
    },
    Reaction {
        entry: 0x3da86,
        table: 0x3da94,
        steps: &[
            spin, spin_rise, spin_drop, spin_fall, spin_bounce, spin_fall, spin_blast, idle, spin_death, skim_flight,
            bounce_start, bounce, get_up, death,
        ],
    },
    Reaction {
        entry: 0x3dab0,
        table: 0x3dabe,
        steps: &[
            spin, spin_rise, spin_drop_stars, spin_fall, spin_bounce_stars, spin_fall, spin_blast, idle, spin_death,
            skim_flight, bounce_start, bounce, get_up, death,
        ],
    },
    Reaction { entry: 0x3dd72, table: 0x3dd80, steps: &[death_throw, wait_anim, fall, bounce_start, bounce, death] },
    Reaction { entry: 0x3ddf6, table: 0x3de04, steps: &[death_throw_b, wait_anim, fall, bounce_start, bounce, death] },
];

/// Routines sans table de sauts en tête.
const RECOIL: u32 = 0x3bb9c;
const RECOIL_1: u32 = 0x3bf76;
const HELD: u32 = 0x3c63a;
const DISINTEGRATE: u32 = 0x3c828;
const GROUNDED_GRAB: u32 = 0x3d350;
const BURNT: u32 = 0x3d86e;

/// Exécute la routine de réaction d'origine `entry` ; faux si le pas est hors de sa table (voir `native_run`).
fn run_entry_flag(x: &mut Cx, entry: u32) -> bool {
    match entry {
        RECOIL => {
            recoil(x);
            true
        }
        RECOIL_1 => {
            recoil_1(x);
            true
        }
        HELD => {
            let n = x.u16(f::RECOVER).wrapping_sub(1);
            x.set16(f::RECOVER, n);
            dispatch(x, 0x3c64c, &[held_start, held_shake, held_end, death])
        }
        DISINTEGRATE => disintegrate(x),
        GROUNDED_GRAB => grounded_grab(x),
        BURNT => {
            // `move.b 6(A0),D0` sans `moveq` : l'indice est le mot bas de D0
            if x.c.d[0] & 0xff00 != 0 {
                return false;
            }
            dispatch(x, 0x3d87a, &[burnt, burnt_fall, anim_step, burnt_wait])
        }
        _ => match REACTIONS.iter().find(|r| r.entry == entry) {
            Some(r) => dispatch(x, r.table, r.steps),
            None => false,
        },
    }
}

/// Saut vers une autre routine de réaction (`bra`) depuis un pas.
fn run_entry(x: &mut Cx, entry: u32) {
    let step = x.u8(f::STEP);
    assert!(run_entry_flag(x, entry), "réaction {entry:#x} : pas {step:#x} hors de la table d'origine");
}

/// 0x3bb16 : animation de réaction `D6` du joueur (jeu par personnage `+3`, table 0x3bb52).
fn player_hurt_animation(c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus) {
    let o = Obj(c.a[0]);
    let character = rd8(b, o.0 + 3) as u32;
    c.d[5] = 4 * character;
    let set = rd32(b, rom::PLAYER_HURT + 4 * character);
    let index = (c.d[6] as u16).wrapping_mul(2) as i16 as i32 as u32;
    let off = rd16(b, set.wrapping_add(index));
    c.d[6] = c.d[6] & !0xffff | off as u32;
    let at = set.wrapping_add(off as i16 as i32 as u32);
    c.a[4] = load_entry(b, o, at, true);
    rts(c, b);
}

/// Exécute la routine de réaction `entry` (RTS compris). Tous les pas des tables d'origine sont réécrits : le pas
/// (`+6`) part de 0 et n'avance que de 2 en 2 jusqu'au dernier pas de la table, qui ne l'avance plus (mort, fin de
/// saisie...), et l'indice de `BURNT` (D0.w) est le code de réaction posé par `enemy_common::react` (octet haut
/// nul). Un pas hors table ferait sauter l'original dans des données : il ne peut pas se produire.
fn native_run(c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, entry: u32) {
    let mut x = Cx::new(c, b);
    x.c.a[5] = crate::mem::A5;
    let step = x.u8(f::STEP);
    assert!(run_entry_flag(&mut x, entry), "réaction {entry:#x} : pas {step:#x} hors de la table d'origine");
    x.ret();
}

macro_rules! reaction_natives {
    ($($entry:expr),* $(,)?) => {
        &[$(Native {
            entry: $entry,
            name: "enemy_reaction",
            run: |c, b| native_run(c, b, $entry),
            touches_sound: false,
            outputs: 0,
        }),*]
    };
}

/// Les routines de réaction, à leur adresse d'origine (appelées par `jsr (A1)` depuis les tables par type).
pub const NATIVES: &[Native] = reaction_natives![
    0x3bb9c, 0x3bc24, 0x3bc82, 0x3bce2, 0x3bd42, 0x3bf56, 0x3bf76, 0x3bfc0, 0x3c01c, 0x3c07a, 0x3c0d4, 0x3c12e,
    0x3c1c2, 0x3c1e2, 0x3c2ee, 0x3c35a, 0x3c4b4, 0x3c570, 0x3c63a, 0x3c70c, 0x3c784, 0x3c828, 0x3c8b8, 0x3c954,
    0x3c9cc, 0x3ca56, 0x3ca76, 0x3cb4c, 0x3ccee, 0x3cd70, 0x3cd90, 0x3cede, 0x3cefe, 0x3cf8e, 0x3cfd0, 0x3cfea,
    0x3d250, 0x3d350, 0x3d3a8, 0x3d488, 0x3d504, 0x3d590, 0x3d618, 0x3d6d8, 0x3d74e, 0x3d80c, 0x3d86e, 0x3d932,
    0x3da86, 0x3dab0, 0x3dd72, 0x3ddf6,
];

/// Animation de réaction du joueur (appelée par les états du joueur restés en 68000).
pub const PLAYER_HURT_ANIMATION: Native = Native {
    entry: 0x3bb16,
    name: "player_hurt_animation",
    run: player_hurt_animation,
    touches_sound: false,
    outputs: 0,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Chaque routine de réaction a une native, et inversement.
    #[test]
    fn routines_et_natives() {
        for r in REACTIONS {
            assert!(NATIVES.iter().any(|n| n.entry == r.entry), "{:#x}", r.entry);
        }
        assert_eq!(NATIVES.len(), REACTIONS.len() + 6);
    }
}
