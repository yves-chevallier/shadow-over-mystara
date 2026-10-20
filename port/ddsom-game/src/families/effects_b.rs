//! Famille EFFCT (suite) : effets de la fin de niveau et du ramassage, types 72 à 75 et 79 (table des
//! types `0x39ce0`, qui compte 92 entrées de 4 octets).
//!
//! - 72 : détecteur de ramassage, qui crée l'éclat de type 55 et, selon sa variante, le clignotement 79 ;
//! - 73 : écran de victoire (sprite qui tombe, clignote et change d'animation selon l'objet qui l'a créé) ;
//! - 74 : bulle à côté d'un joueur qui disparaît quand son créateur s'en va ;
//! - 75 : bouton de menu, une machine à sept sous-états ;
//! - 79 : clignotement d'un objet ramassé, détecté par les joueurs voisins.

use super::cx::{handlers, Cx};
use crate::mem::{rd16, rd8, wr16, wr8, A5};
use crate::objects::Obj;
use crate::rng::Rng;

/// Valeurs de l'octet d'état `+4` (le suivant se calcule par `+2`).
mod state {
    pub const INIT: u8 = 0;
    pub const ACTIVE: u8 = 2;
    pub const FREE: u8 = 4;
    /// Premier état de l'objet lié à un créateur disparu : libération au tour suivant.
    pub const EXIT: u8 = 6;
}

/// Champs partagés.
mod f {
    pub const STATE: u32 = 0x04;
    pub const SUB_STATE: u32 = 0x05;
    /// Variante.
    pub const VARIANT: u32 = 0x03;
    pub const FLAGS: u32 = 0x01;
    pub const FACING: u32 = 0x2c;
    /// Créateur (pointeur de 16 bits).
    pub const OWNER: u32 = 0x5c;
    pub const TIMER: u32 = 0x58;
    pub const TIMER_B: u32 = 0x42;
}

/// Routines d'origine utilisées.
mod rom {
    pub const FREE_EFFECT: u32 = 0x2218;
    pub const ALLOC_EFFECT: u32 = 0x2318;
    pub const ALLOC_ESHL: u32 = 0x22ea;
    pub const ALLOC_BOX: u32 = 0x23a8;
    pub const ANIM_STEP: u32 = 0x1a56;
    pub const MOVE_Z: u32 = 0x1da6;
    pub const SOUND_0ED11C: u32 = 0xed11c;
    pub const SOUND_0ED056: u32 = 0xed056;
    pub const SOUND_0ED90A: u32 = 0xed90a;
    pub const SOUND_0ECF10: u32 = 0xecf10;
}

/// Jeux d'animations.
mod anims {
    pub const INTERFACE: u32 = 0x19431a;
    pub const MENU: u32 = 0x196baa;
    pub const BUBBLE: u32 = 0x1790f6;
    pub const BLINK: u32 = 0x1bc77c;
}

/// Joueurs : quatre enregistrements de 0x200 octets.
const PLAYERS: u32 = A5 + 0x5de;
const PLAYER_STRIDE: u32 = 0x200;
/// Jeu en pause.
const PAUSED: u32 = 0x3d;

fn players() -> impl Iterator<Item = u32> {
    (0..4).map(|k| PLAYERS + k * PLAYER_STRIDE)
}

/// Créateur de l'objet.
fn owner(x: &mut Cx) -> Obj {
    x.linked(f::OWNER)
}

fn free(x: &mut Cx) {
    x.call(rom::FREE_EFFECT);
}

// ---------------------------------------------------------------------------------------------
// Type 72 : détecteur de ramassage
// ---------------------------------------------------------------------------------------------

/// 0xb8dc2 : crée le clignotement (type 79) sur la position de l'objet, un cran plus haut.
fn spawn_blink(x: &mut Cx) {
    x.call(rom::ALLOC_EFFECT);
    if x.c.sr & m68k::SR_C != 0 {
        return;
    }
    let blink = Obj(x.c.a[1]);
    wr8(x.b, blink.0, 1);
    wr8(x.b, blink.0 + 2, 79);
    for field in [Obj::X, Obj::Z, Obj::Y] {
        let v = x.u16(field);
        wr16(x.b, blink.0 + field, v);
    }
    let z = x.u16(Obj::Z).wrapping_add(16);
    wr16(x.b, blink.0 + Obj::Z, z);
    wr16(x.b, blink.0 + f::OWNER, x.o.0 as u16);
}

/// 0xa532e : crée le tir d'éclat de ramassage (ESHL de type 55) à l'endroit où le détecteur est touché.
fn spawn_pickup_flash(x: &mut Cx) {
    x.call(rom::ALLOC_ESHL);
    if x.c.sr & m68k::SR_C != 0 {
        return;
    }
    let flash = x.c.a[1];
    wr8(x.b, flash, 1);
    wr8(x.b, flash + 2, 55);
    let variant = x.u8(f::VARIANT);
    wr8(x.b, flash + 3, variant);
    let (px, py) = (x.u16(Obj::X), x.u16(Obj::Y));
    wr16(x.b, flash + 0x58, px);
    wr16(x.b, flash + 0x5a, py);
    wr16(x.b, flash + f::OWNER, x.o.0 as u16);
}

/// Un joueur marche sur le détecteur : à 6 pixels en profondeur et 8 en x près.
fn player_on_detector(x: &mut Cx) -> bool {
    let (depth, across) = (x.u16(Obj::Y), x.u16(Obj::X));
    players().any(|player| {
        rd8(x.b, player) != 0
            && depth.wrapping_sub(rd16(x.b, player + Obj::Y)).wrapping_add(6) <= 12
            && across.wrapping_sub(rd16(x.b, player + Obj::X)).wrapping_add(8) <= 16
    })
}

fn pickup_detector(x: &mut Cx) {
    match x.u8(f::STATE) {
        state::INIT => {
            x.add8(f::STATE, 2);
            x.set8(0, 1);
            if x.u8(0x2f) != 0 {
                return spawn_blink(x);
            }
            pickup_waiting(x);
        }
        state::ACTIVE => pickup_waiting(x),
        state::FREE => {
            let t = x.u8(f::TIMER).wrapping_sub(1);
            x.set8(f::TIMER, t);
            if (t as i8) < 0 {
                x.add8(f::STATE, 2);
            }
        }
        _ => free(x),
    }
}

fn pickup_waiting(x: &mut Cx) {
    if x.u8(0x40) != 0 || x.a5_u8(PAUSED) != 0 {
        return;
    }
    if player_on_detector(x) {
        x.add8(f::STATE, 2);
        x.set8(f::TIMER, 2);
        spawn_pickup_flash(x);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 73 : écran de victoire
// ---------------------------------------------------------------------------------------------

/// Clignotement : caché une image sur deux selon la parité du compteur d'images de partie.
fn blink_by_frame(x: &mut Cx) {
    let hidden = x.a5_u16(0x6e) & 1 == 0;
    let flags = x.u8(f::FLAGS);
    x.set8(f::FLAGS, if hidden { flags | 2 } else { flags & !2 });
}

fn victory_screen(x: &mut Cx) {
    match x.u8(f::STATE) {
        state::INIT => {
            x.add8(f::STATE, 2);
            let (cx, cy) = (x.a5_u16(0x5a6).wrapping_add(0xc0), x.a5_u16(0x5aa).wrapping_add(0x68));
            x.set16(Obj::X, cx);
            x.set16(Obj::Y, cy);
            x.set16(Obj::Z, 0x80);
            x.set16(0x18, 0xf000);
            x.call(rom::SOUND_0ED11C);
            x.anim_start(anims::INTERFACE, 0);
        }
        state::ACTIVE => victory_active(x),
        _ => free(x),
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

fn victory_active(x: &mut Cx) {
    let creator = owner(x);
    match x.u8(f::SUB_STATE) {
        0 => {
            x.call(rom::MOVE_Z);
            if (x.u16(Obj::Z) as i16) <= 0 {
                x.add8(f::SUB_STATE, 2);
                x.set16(Obj::Z, 0);
            }
            x.call(rom::ANIM_STEP);
        }
        2 => {
            if creator.u32(x.b, f::STATE) != 0x0200_0200 {
                return x.call(rom::ANIM_STEP);
            }
            x.add8(f::SUB_STATE, 2);
            blink_by_frame(x);
            x.call(rom::SOUND_0ED056);
            x.anim_start(anims::INTERFACE, 1);
        }
        4 => {
            blink_by_frame(x);
            if creator.u32(x.b, f::STATE) != 0x0200_0800 {
                return x.call(rom::ANIM_STEP);
            }
            x.add8(f::SUB_STATE, 2);
            x.call(rom::SOUND_0ED90A);
            x.anim_start(anims::INTERFACE, 2);
        }
        6 => {
            blink_by_frame(x);
            x.call(rom::ANIM_STEP);
            if x.c.sr & m68k::SR_C != 0 {
                free(x);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Type 74 : bulle à côté du créateur
// ---------------------------------------------------------------------------------------------

fn bubble(x: &mut Cx) {
    if x.u16(f::OWNER) == 0 {
        return x.set32(f::STATE, 0x0600_0000);
    }
    match x.u8(f::STATE) {
        state::INIT => {
            x.add8(f::STATE, 2);
            let creator = owner(x);
            for field in [Obj::X, Obj::Y, Obj::Z] {
                let v = creator.u32(x.b, field);
                x.set32(field, v);
            }
            let facing = creator.u8(x.b, f::FACING);
            x.set8(f::FACING, facing);
            x.add16(Obj::Y, 1);
            let (mut dx, dz) = (36u16, 106u16);
            x.set16(0x14, 128);
            x.set16(0x18, 1472);
            x.set16(0x1a, 0xffc8);
            if facing != 0 {
                dx = dx.wrapping_neg();
                x.set16(0x14, 128u16.wrapping_neg());
            }
            x.add16(Obj::X, dx);
            x.add16(Obj::Z, dz);
            x.anim_start(anims::BUBBLE, 14);
            x.reinsert();
        }
        state::ACTIVE => match x.u8(f::SUB_STATE) {
            0 => {
                x.call(rom::MOVE_Z);
                if (x.u16(Obj::Z) as i16) > 64 {
                    return x.call(rom::ANIM_STEP);
                }
                x.add8(f::SUB_STATE, 2);
                x.set16(Obj::Z, 0);
                x.set16(f::TIMER_B, 150);
                x.anim_start(anims::BUBBLE, 15);
            }
            2 => {
                let t = x.u16(f::TIMER_B).wrapping_sub(1);
                x.set16(f::TIMER_B, t);
                if (t as i16) < 0 {
                    x.add8(f::SUB_STATE, 2);
                    x.set16(f::TIMER_B, 60);
                }
            }
            4 => {
                let t = x.u16(f::TIMER_B).wrapping_sub(1);
                x.set16(f::TIMER_B, t);
                if (t as i16) < 0 {
                    x.set8(f::STATE, state::EXIT);
                } else {
                    let flags = x.u8(f::FLAGS) & !2;
                    x.set8(f::FLAGS, if t & 1 != 0 { flags | 2 } else { flags });
                }
            }
            _ => {}
        },
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 75 : bouton de menu
// ---------------------------------------------------------------------------------------------

/// Variable `A5-0x55c8` (0xff2a38) : étape de la séquence de menu qui commande les sous-états du bouton.
const MENU_STEP: u32 = 0xff2a38;
/// Variable `A5+0x2e` : étape de la sélection.
const SELECT_STEP: u32 = 0x2e;

fn menu_button(x: &mut Cx) {
    match x.u8(f::STATE) {
        state::INIT => {
            x.add8(f::STATE, 2);
            if x.u8(f::VARIANT) == 0 {
                x.set16(Obj::X, 328);
                x.set16(Obj::Y, 368);
                x.anim_start(anims::MENU, 0);
            } else {
                let (cx, cy) = (x.a5_u16(0x506), x.a5_u16(0x50a));
                x.set16(Obj::X, cx.wrapping_add(0xc0));
                x.set16(Obj::Y, cy.wrapping_add(0x40));
                x.set16(Obj::Z, 40);
                x.anim_start(anims::INTERFACE, 3);
            }
            x.reinsert();
        }
        state::ACTIVE => menu_button_active(x),
        _ => free(x),
    }
}

fn menu_step(x: &mut Cx) -> u8 {
    crate::mem::rd8(x.b, MENU_STEP)
}

fn menu_button_active(x: &mut Cx) {
    if x.u8(f::VARIANT) != 0 {
        return x.call(rom::ANIM_STEP);
    }
    let advance = |x: &mut Cx, n: u32| {
        x.add8(f::SUB_STATE, 2);
        x.anim_start(anims::MENU, n);
    };
    match x.u8(f::SUB_STATE) {
        0 => {
            if x.a5_u8(SELECT_STEP) == 6 {
                advance(x, 4);
            } else {
                x.call(rom::ANIM_STEP);
            }
        }
        2 => {
            x.call(rom::ANIM_STEP);
            if x.c.sr & m68k::SR_C != 0 {
                advance(x, 5);
                x.call(rom::SOUND_0ECF10);
            } else if x.u8(0x33) != 0 {
                x.set8(0x33, 0);
                x.call(rom::ALLOC_BOX);
                let spawned = x.c.a[1];
                wr8(x.b, spawned, 1);
                wr8(x.b, spawned + 2, 8);
            }
        }
        4 => {
            x.call(rom::ANIM_STEP);
            if x.c.sr & m68k::SR_C != 0 {
                x.add8(f::SUB_STATE, 2);
                let flags = x.u8(f::FLAGS) | 2;
                x.set8(f::FLAGS, flags);
            }
        }
        6 => {
            if menu_step(x) == 20 {
                x.add8(f::SUB_STATE, 2);
                let flags = x.u8(f::FLAGS) & !2;
                x.set8(f::FLAGS, flags);
                x.anim_start(anims::MENU, 6);
                x.call(rom::SOUND_0ECF10);
            }
        }
        8 => {
            x.call(rom::ANIM_STEP);
            if x.c.sr & m68k::SR_C != 0 {
                advance(x, 7);
            }
        }
        10 => {
            if menu_step(x) == 28 {
                advance(x, 5);
                x.call(rom::SOUND_0ECF10);
            } else {
                x.call(rom::ANIM_STEP);
            }
        }
        12 => {
            x.call(rom::ANIM_STEP);
            if x.c.sr & m68k::SR_C != 0 {
                x.add8(f::STATE, 2);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Type 79 : clignotement d'un objet ramassé
// ---------------------------------------------------------------------------------------------

/// Un joueur vivant, de classe 5, est à portée (32 en profondeur, 96 en x).
fn class_5_player_near(x: &mut Cx) -> bool {
    let (depth, across) = (x.u16(Obj::Y), x.u16(Obj::X));
    players().any(|player| {
        rd8(x.b, player) != 0
            && rd16(x.b, player + 0x62) != 0
            && rd8(x.b, player + 3) >> 1 == 5
            && depth.wrapping_sub(rd16(x.b, player + Obj::Y)).wrapping_add(32) <= 64
            && across.wrapping_sub(rd16(x.b, player + Obj::X)).wrapping_add(96) <= 192
    })
}

fn blinking(x: &mut Cx) {
    match x.u8(f::STATE) {
        state::INIT => {
            x.add8(f::STATE, 2);
            let flags = x.u8(f::FLAGS) | 2;
            x.set8(f::FLAGS, flags);
            x.set8(0x57, 8);
            let mut rng = Rng(rd16(x.b, A5 + 0x1a));
            let (v, _) = rng.suivant();
            wr16(x.b, A5 + 0x1a, v);
            x.set16(f::TIMER, v & 31);
            x.anim_start(anims::BLINK, 0);
            x.reinsert();
        }
        state::ACTIVE => {
            let creator = owner(x);
            if creator.u8(x.b, f::STATE) != 2 {
                x.add8(f::STATE, 2);
            }
            if x.u8(0x5b) == 0 {
                if class_5_player_near(x) {
                    x.set8(0x5b, 1);
                }
                return;
            }
            if creator.u8(x.b, 0x40) != 0 {
                let flags = x.u8(f::FLAGS) | 2;
                return x.set8(f::FLAGS, flags);
            }
            let t = x.u16(f::TIMER).wrapping_sub(1);
            x.set16(f::TIMER, t);
            if t != 0 {
                return;
            }
            x.set16(f::TIMER, 48);
            let on = x.u8(0x5a) ^ 1;
            x.set8(0x5a, on);
            let flags = x.u8(f::FLAGS) & !2;
            x.set8(f::FLAGS, flags);
            if x.u8(0x5a) != 0 {
                return;
            }
            x.set8(f::FLAGS, flags | 2);
            let left = x.u8(0x57).wrapping_sub(1);
            x.set8(0x57, left);
            if left == 0 {
                x.set32(f::STATE, 0x0400_0000);
            }
        }
        _ => free(x),
    }
}

handlers! {
    0xb841c "effct_detecteur" 200 pickup_detector,
    0xb84c6 "effct_victoire" 300 victory_screen,
    0xb85fa "effct_bulle_createur" 300 bubble,
    0xb871a "effct_bouton" 300 menu_button,
    0xb8df2 "effct_clignotement" 300 blinking,
}
