//! Tâche du gel (`0xee080`, emplacement 14 de la table) : attente active et « avance rapide ».
//!
//! À chaque passage de l'ordonnanceur elle fait ce que fait le VBLANK pour les tâches qui dorment, mais
//! seulement pour la tâche principale (emplacement 0), et seulement quand un joueur présent presse un bouton
//! pendant une partie : le jeu enchaîne alors ses pas sans attendre l'image (voir `docs/boucle.md` §1). Dans tous
//! les autres cas elle cède la main sans rien faire (`Attente::Prete`) : c'est l'attente active qui occupe le
//! processeur surcadencé ([`attente_vide`] la reconnaît).
//!
//! Le pas est le même à l'entrée (`0xee080`, tâche neuve) et après chacune des deux cessions (`0xee0fa`,
//! `0xee16a` : l'original reboucle par un `bra 0xee080`).

use super::{ceder, Attente};
use crate::mem::{rd8, set_low_byte, set_low_word, wr8, A5};
use crate::modes::Flow;
use crate::resume;
use crate::scene::Ctx;
use crate::Native;

/// Variables de la RAM de travail (décalages depuis A5).
mod var {
    /// Mot de réglages (DSW) : le bit 7 vaut 1 en marche normale.
    pub const DSW: u32 = 0x7fd0;
    /// Partie en cours, et écran de jeu actif.
    pub const GAME: u32 = 0x30;
    pub const SCREEN: u32 = 0x33;
    /// Boutons des quatre joueurs (`A5+0..3`) et leurs drapeaux de présence.
    pub const BUTTONS: u32 = 0;
    pub const PRESENT: [u32; 4] = [0x5de, 0x7de, 0x9de, 0xbde];
}

/// Table des tâches : premier enregistrement, taille d'un enregistrement et nombre d'emplacements.
const TASKS: u32 = 0xff_0064;
const RECORD: u32 = 16;
const SLOTS: u32 = 16;
/// Emplacement de la tâche du gel.
pub const SLOT: u32 = 14;
/// Bits de bouton qui comptent.
const BUTTON_MASK: u8 = 0x16;
/// États d'un enregistrement : en attente (compte à rebours en +1), prête.
const WAITING: u8 = 1;
const READY: u8 = 4;
/// Bit du mot de réglages qui désigne la marche normale.
const NORMAL: u16 = 0x80;

/// Adresses de reprise : entrée de la tâche, et retours des deux appels de cession.
pub const ENTRY: u32 = 0xee080;
pub const AFTER_YIELD: [u32; 2] = [0xee0fa, 0xee16a];

/// Un pas : l'effet du passage, puis la cession.
fn step(x: &mut Ctx) -> Flow {
    let normal = x.v16(var::DSW) & NORMAL;
    set_low_word(&mut x.c.d[0], normal);
    let resume = if normal == 0 {
        wake_all(x);
        AFTER_YIELD[0]
    } else if x.v8(var::SCREEN) != 0 && x.v8(var::GAME) != 0 {
        fast_forward(x)
    } else {
        AFTER_YIELD[0]
    };
    ceder(x, Attente::Prete, resume)
}

/// Décompte d'un enregistrement en attente : à zéro, la tâche est prête.
fn count_down(x: &mut Ctx, record: u32) {
    if rd8(x.b, record) == WAITING {
        let left = rd8(x.b, record + 1).wrapping_sub(1);
        wr8(x.b, record + 1, left);
        if left == 0 {
            wr8(x.b, record, READY);
        }
    }
}

/// `0xee0ce` (réglages sans le bit 7, jamais vus dans ce jeu) : décompte comme le VBLANK, pour toutes les tâches.
fn wake_all(x: &mut Ctx) {
    for slot in 0..SLOTS {
        count_down(x, TASKS + slot * RECORD);
    }
    x.c.a[0] = TASKS + SLOTS * RECORD;
    set_low_word(&mut x.c.d[0], 0xffff);
}

/// `0xee0fc` : si un joueur présent presse un bouton, la tâche principale perd une image d'attente. Rend
/// l'adresse de reprise de la cession qui suit.
fn fast_forward(x: &mut Ctx) -> u32 {
    x.c.d[0] = 0;
    for (player, present) in var::PRESENT.into_iter().enumerate() {
        if x.v8(present) != 0 {
            let pressed = x.v8(var::BUTTONS + player as u32) & BUTTON_MASK;
            set_low_byte(&mut x.c.d[1], pressed);
            let seen = x.c.d[0] as u8 | pressed;
            set_low_byte(&mut x.c.d[0], seen);
        }
    }
    if x.c.d[0] as u8 == 0 {
        return AFTER_YIELD[0];
    }
    count_down(x, TASKS);
    AFTER_YIELD[1]
}

resume!(R_ENTRY, ENTRY, step);
resume!(R_AFTER_YIELD_A, AFTER_YIELD[0], step);
resume!(R_AFTER_YIELD_B, AFTER_YIELD[1], step);

pub const NATIVES: &[Native] = &[R_ENTRY, R_AFTER_YIELD_A, R_AFTER_YIELD_B];

/// L'ordonnanceur tourne à vide : la tâche du gel est la seule prête, elle vient de céder la main, et son prochain
/// pas ne ferait rien. L'état du jeu ne peut plus changer avant la prochaine interruption : `idle` (`lib.rs`) peut
/// avancer jusqu'à elle. `ram` est la RAM de travail (FF0000-FFFFFF).
pub fn attente_vide(ram: &[u16]) -> bool {
    let byte = |a: u32| (ram[(a as usize & 0xffff) / 2] >> if a & 1 == 0 { 8 } else { 0 }) as u8;
    let long = |a: u32| (ram[(a as usize & 0xffff) / 2] as u32) << 16 | ram[(a as usize & 0xffff) / 2 + 1] as u32;
    let var = |offset: u32| A5.wrapping_add(offset);
    let ready = |slot: u32| byte(TASKS + slot * RECORD) >= READY;
    let gel = TASKS + SLOT * RECORD;
    if !ready(SLOT) || (0..SLOTS).any(|slot| slot != SLOT && ready(slot)) {
        return false;
    }
    // elle a cédé par `jsr 0xa86` depuis l'une de ses deux cessions, et c'est la dernière tâche lancée
    // (FF0056 : l'état de l'ordonnanceur est alors celui de chaque passage suivant)
    if long(gel + 4) != 0xa8c || !AFTER_YIELD.contains(&long(long(gel + 8) + 60)) || long(0xff_0056) != gel {
        return false;
    }
    if ram[(var(var::DSW) as usize & 0xffff) / 2] & NORMAL == 0 {
        return false;
    }
    if byte(var(var::SCREEN)) != 0 && byte(var(var::GAME)) != 0 && byte(TASKS) == WAITING {
        let pressed = (0..4).any(|player| byte(var(var::PRESENT[player])) != 0 && byte(var(var::BUTTONS + player as u32)) & BUTTON_MASK != 0);
        if pressed {
            return false;
        }
    }
    true
}
