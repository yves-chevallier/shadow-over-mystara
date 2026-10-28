//! Points d'accroche des personnages (P5).
//!
//! Le noyau commun (`actions`) ne contient pas le code des attaques : six personnages × deux variantes
//! ont chacun leur saut, leur attaque, leur attaque spéciale et deux autres actions. Dans le programme
//! d'origine, chaque `Slot` est une table de 12 adresses (une par valeur de `Player::CHARACTER`) ;
//! `actions::act` appelle l'adresse de la table.
//!
//! `run` aiguille vers la fonction Rust du personnage (`routine`) ; les natives (`action_native!`) ne servent plus
//! qu'aux appels par adresse restants (fuzz, `DDSOM_CHECK`).
//!
//! Les fonctions communes que les personnages réutilisent sont publiques : `checks::turn_check`,
//! `attack_button`, `jump_button`, `actions::to_stand`, `walk::walk_speed`, `walk::move_xy`,
//! `animation::start`, `animation::step`, `hurt::hurt_animation`...

/// Déclare une `Native` à l'adresse d'origine d'une action de personnage : la fonction Rust reçoit le
/// joueur (A0) ; aucun registre n'est lu par les appelants. Le jeu appelle la fonction directement (`routine`) ; la
/// native sert à la validation différentielle de chaque action (`DDSOM_CHECK=1`, `fuzz_player`).
macro_rules! action_native {
    ($name:ident, $entry:expr, $label:expr, $f:path) => {
        pub const $name: crate::Native = crate::Native {
            entry: $entry,
            name: $label,
            run: crate::Corps::Processeur(|c, b| {
                let p = $crate::player::Player(c.a[0]);
                $f(&mut $crate::player::Ctx::new(c, b), p);
                $crate::mem::rts(c, b)
            }),
            touches_sound: false,
            outputs: 0,
        };
    };
}

pub mod cleric;
pub mod common;
pub mod dwarf;
pub mod elf;
pub mod fighter;
pub mod foes;
pub mod items;
pub mod magic_user;
pub mod thief;

use super::{Ctx, Player};

/// Une routine Rust d'un personnage : s'exécute pour une image du joueur.
pub type CharacterFn = fn(&mut Ctx, Player);

/// Les cinq actions propres aux personnages (octet `+6`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    /// Saut (`action::JUMP`, 0x0a).
    Jump,
    /// Attaque du bouton 1 (`action::ATTACK`, 0x0c).
    Attack,
    /// Attaque spéciale (`action::SPECIAL`, 0x0e).
    Special,
    /// Action 0x10 (h : attaque en l'air ou lancer d'arme).
    Action10,
    /// Action 0x12 (h).
    Action12,
}

/// Routine du personnage `character` (`Player::CHARACTER`) pour l'action `slot` : tables d'origine 0x35918 (saut),
/// 0x35972 (attaque), 0x359cc (spéciale), 0x35a0a (action 0x10) et 0x35a48 (action 0x12), une adresse par valeur
/// de `CHARACTER` ; les deux variantes d'un personnage ont le même code.
pub fn routine(slot: Slot, character: u8) -> CharacterFn {
    use super::moves;
    use Slot::*;
    match (character >> 1, slot) {
        (0, Jump) => fighter::jump,
        (0, Attack) => fighter::attack,
        (0, Special) => fighter::special,
        (1, Jump) => cleric::jump,
        (1, Attack) => cleric::attack,
        (1, Special) => cleric::special,
        (2, Jump) => elf::jump,
        (2, Attack) => elf::attack,
        (2, Special) => elf::special,
        (3, Jump) => dwarf::jump,
        (3, Attack) => dwarf::attack,
        (3, Special) => dwarf::special,
        (3, Action12) => dwarf::roll,
        (4, Jump) => magic_user::jump,
        (4, Attack) => magic_user::attack,
        (4, Special) => magic_user::special,
        (4, Action10) => magic_user::dash,
        (4, Action12) => moves::hop_back,
        (5, Jump) => thief::jump,
        (5, Attack) => thief::attack,
        (5, Special) => thief::special,
        (5, Action10) => thief::dash,
        (5, Action12) => thief::roll,
        (0..=3, Action10) => common::dash,
        (0..=2, Action12) => moves::leap_attack,
        (class, _) => unreachable!("personnage {character} (classe {class}) hors des tables 0x35918-0x35a48"),
    }
}

/// Exécute l'action `slot` du personnage du joueur `p`.
pub fn run(g: &mut Ctx, p: Player, slot: Slot) {
    let character = p.u8(g, Player::CHARACTER);
    // D0 = numéro d'action, octet haut nul, comme l'aiguillage d'origine : le pas d'animation fidèle aux registres
    // (`common::step_keeping_regs`) et les créateurs qui suivent peuvent encore le lire
    g.c.d[0] = p.u8(g, Player::ACTION) as u32;
    routine(slot, character)(g, p)
}
