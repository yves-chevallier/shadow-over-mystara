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

use super::table::{enregistrement, table, Etat, Reprise, Table, EMPLACEMENTS};
use super::{ceder, Attente};
use crate::mem::{set_low_byte, set_low_word, A5};
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

/// Emplacement de la tâche du gel, et celui de la tâche principale.
pub const SLOT: usize = 14;
const MAIN: usize = 0;
/// Bits de bouton qui comptent.
const BUTTON_MASK: u8 = 0x16;
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

/// `0xee0ce` (réglages sans le bit 7, jamais vus dans ce jeu) : décompte comme le VBLANK, pour toutes les tâches.
fn wake_all(x: &mut Ctx) {
    let tasks = table(x.b);
    for slot in 0..EMPLACEMENTS {
        tasks.decompter(slot);
    }
    x.c.a[0] = enregistrement(EMPLACEMENTS);
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
    table(x.b).decompter(MAIN);
    AFTER_YIELD[1]
}

resume!(R_ENTRY, ENTRY, step);
resume!(R_AFTER_YIELD_A, AFTER_YIELD[0], step);
resume!(R_AFTER_YIELD_B, AFTER_YIELD[1], step);

pub const NATIVES: &[Native] = &[R_ENTRY, R_AFTER_YIELD_A, R_AFTER_YIELD_B];

/// L'ordonnanceur tourne à vide : la tâche du gel est la seule prête, elle vient de céder la main, et son prochain
/// pas ne ferait rien. L'état du jeu ne peut plus changer avant la prochaine interruption : `idle` (`lib.rs`) peut
/// avancer jusqu'à elle. `ram` est la RAM de travail (FF0000-FFFFFF).
pub fn attente_vide(tasks: &Table, ram: &[u16]) -> bool {
    let ready = |slot: usize| tasks.emplacements[slot].etat.lancable();
    if !ready(SLOT) || (0..EMPLACEMENTS).any(|slot| slot != SLOT && ready(slot)) {
        return false;
    }
    // elle a cédé par `jsr 0xa86` depuis l'une de ses deux cessions, et c'est la dernière tâche lancée
    // (l'état de l'ordonnanceur est alors celui de chaque passage suivant)
    let ceded = matches!(tasks.emplacements[SLOT].reprise, Reprise::Cession(c) if AFTER_YIELD.contains(&c.pc));
    let main_waiting = tasks.emplacements[MAIN].etat == Etat::Attend;
    ceded && tasks.courante == SLOT && !step_would_act(ram, main_waiting)
}

/// Même test sur la table de la RAM d'une machine d'origine (enregistrements FF0064 : état, PC de reprise `0xa8c`
/// de l'enveloppe, pile où l'adresse de retour suit les 15 registres sauvés ; tâche courante en FF0056).
pub fn attente_vide_origine(ram: &[u16]) -> bool {
    let byte = |a: u32| (ram[(a as usize & 0xffff) / 2] >> if a & 1 == 0 { 8 } else { 0 }) as u8;
    let long = |a: u32| (ram[(a as usize & 0xffff) / 2] as u32) << 16 | ram[(a as usize & 0xffff) / 2 + 1] as u32;
    let ready = |slot: usize| byte(enregistrement(slot)) >= Etat::Prete.octet();
    let gel = enregistrement(SLOT);
    if !ready(SLOT) || (0..EMPLACEMENTS).any(|slot| slot != SLOT && ready(slot)) {
        return false;
    }
    if long(gel + 4) != 0xa8c || !AFTER_YIELD.contains(&long(long(gel + 8) + 60)) || long(0xff_0056) != gel {
        return false;
    }
    let main_waiting = byte(enregistrement(MAIN)) == Etat::Attend.octet();
    !step_would_act(ram, main_waiting)
}

/// Le prochain pas ferait quelque chose : réglages sans le bit 7 (tout décompter), ou un joueur présent presse un
/// bouton en partie alors que la tâche principale attend.
fn step_would_act(ram: &[u16], main_waiting: bool) -> bool {
    let byte = |a: u32| (ram[(a as usize & 0xffff) / 2] >> if a & 1 == 0 { 8 } else { 0 }) as u8;
    let var = |offset: u32| A5.wrapping_add(offset);
    if ram[(var(var::DSW) as usize & 0xffff) / 2] & NORMAL == 0 {
        return true;
    }
    byte(var(var::SCREEN)) != 0
        && byte(var(var::GAME)) != 0
        && main_waiting
        && (0..4).any(|player| byte(var(var::PRESENT[player])) != 0 && byte(var(var::BUTTONS + player as u32)) & BUTTON_MASK != 0)
}
