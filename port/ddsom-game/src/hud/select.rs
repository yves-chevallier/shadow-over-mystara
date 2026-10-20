//! Choix du personnage : personnages déjà pris, recherche du suivant libre, lecture de la croix.
//!
//! Un personnage est un nombre de 0 à 11 : le numéro (pas de 2) et, dans le bit 0, la variante de
//! couleur. Un joueur ne peut pas prendre le personnage d'un autre.

use cps2::Cps2Bus;

use super::{panel, player, v8, var, Slot};
use crate::mem::{rd8, A5};

/// Personnages pris (un bit par personnage et variante) : ceux des joueurs présents, ou celui qu'ils ont
/// choisi s'ils sont en train de choisir (états 8 et 0xc). `except` : un joueur à ignorer (le sien).
fn taken(b: &mut Cps2Bus, except: Option<u32>) -> u32 {
    let count = 3u8.wrapping_sub(v8(b, var::LAYOUT)) as u32 + 1;
    let mut mask = 0u32;
    for k in 0..count {
        let who = A5 + var::PLAYERS + player::SIZE * k;
        let panel_k = A5 + var::PANELS + panel::SIZE * k;
        if rd8(b, who) == 0 || Some(who) == except {
            continue;
        }
        let state = rd8(b, panel_k + panel::STATE);
        let character = if state == 8 || state == 0xc { rd8(b, panel_k + panel::CHOSEN) } else { rd8(b, who + player::CHARACTER) };
        mask |= 1 << (character & 31);
    }
    mask
}

fn is_free(mask: u32, choice: u8) -> bool {
    mask >> (choice & 31) & 1 == 0
}

/// Cherche à partir de `choice` un personnage libre, en avant ou en arrière. Si la variante voisine est
/// libre, on la prend ; sinon on passe au personnage suivant (ou précédent). Rend aussi si la recherche s'est
/// arrêtée sur la variante voisine (`true`) ou sur le choix lui-même.
fn search(mask: u32, mut choice: u8, backward: bool) -> (u8, bool) {
    loop {
        if is_free(mask, choice) {
            return (choice, false);
        }
        choice ^= 1;
        if is_free(mask, choice) {
            return (choice, true);
        }
        choice &= !1;
        if backward {
            choice = choice.wrapping_sub(2);
            if (choice as i8) < 0 {
                choice = 0xa;
            }
        } else {
            choice += 2;
            if choice > 0xb {
                choice = 0;
            }
        }
    }
}

/// Le joueur garde sa variante si le personnage trouvé est le sien (`panel::CHOSEN`).
fn keep_own_variant(b: &mut Cps2Bus, s: Slot, choice: u8) -> u8 {
    let own = s.p8(b, panel::CHOSEN);
    if own >> 1 == choice >> 1 {
        own
    } else {
        choice
    }
}

/// 0x153ba : premier personnage libre à partir de `choice` (choix à l'entrée en jeu).
pub fn free_choice(b: &mut Cps2Bus, choice: u8, backward: bool) -> u8 {
    let mask = taken(b, None);
    search(mask, choice, backward).0
}

/// 0x15444 : comme `free_choice`, sans compter le personnage du joueur lui-même, qui garde sa variante.
pub fn other_choice(b: &mut Cps2Bus, s: Slot, choice: u8, backward: bool) -> u8 {
    let mask = taken(b, Some(s.player));
    let (found, _) = search(mask, choice, backward);
    keep_own_variant(b, s, found)
}

/// 0x154e8 : comme `other_choice`, mais le personnage trouvé est rendu tel quel, sauf en arrière quand c'est
/// la variante voisine (l'original sort alors par la fin de 0x15444).
pub fn resume_choice(b: &mut Cps2Bus, s: Slot, choice: u8, backward: bool) -> u8 {
    let mask = taken(b, Some(s.player));
    let (found, neighbour) = search(mask, choice, backward);
    if backward && neighbour {
        keep_own_variant(b, s, found)
    } else {
        found
    }
}

/// 0x173ee : entrées du joueur `index` : état courant et fronts montants.
pub fn inputs(b: &mut Cps2Bus, index: u8) -> (u8, u8) {
    (rd8(b, A5 + var::INPUT + index as u32), rd8(b, A5 + var::PRESSED + index as u32))
}

/// Répétition de la croix : nombre d'images avant de répéter un mouvement maintenu.
const REPEAT_FRAMES: i8 = 0x3c;

/// 0x158c6 / 0x15958 : lit la croix et déplace le choix `choice` d'un cran (à droite : 1 ou 2 selon la variante ;
/// à gauche : 2 ou 3) à chaque front montant, puis toutes les 60 images si la direction est maintenue. Le
/// nouveau choix passe par la recherche d'un personnage libre (`resolve`).
pub fn navigate(
    b: &mut Cps2Bus,
    s: Slot,
    choice: u8,
    resolve: &dyn Fn(&mut Cps2Bus, u8, bool) -> u8,
) -> u8 {
    let index = s.index(b);
    let (held, pressed) = inputs(b, index);
    let direction = held & 3;
    if direction == 0 {
        s.set_p8(b, panel::FLAG_27, 0);
        return choice;
    }
    let right = direction == 1;
    let edge = if right { pressed & 1 != 0 } else { pressed & 2 != 0 };
    if !edge {
        let held_frames = s.p8(b, panel::FLAG_27).wrapping_add(1);
        s.set_p8(b, panel::FLAG_27, held_frames);
        if (held_frames as i8) < REPEAT_FRAMES {
            return choice;
        }
    }
    s.set_p8(b, panel::FLAG_27, 0);
    if right {
        let mut next = choice.wrapping_add(if choice & 1 != 0 { 1 } else { 2 });
        if next > 0xb {
            next = 0;
        }
        resolve(b, next, false)
    } else {
        let mut next = choice.wrapping_sub(if choice & 1 != 0 { 3 } else { 2 });
        if (next as i8) < 0 {
            next = 0xa;
        }
        resolve(b, next, true)
    }
}
