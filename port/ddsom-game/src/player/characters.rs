//! Points d'accroche des personnages (P5).
//!
//! Le noyau commun (`actions`) ne contient pas le code des attaques : six personnages × deux variantes
//! ont chacun leur saut, leur attaque, leur attaque spéciale et deux autres actions. Dans le programme
//! d'origine, chaque `Slot` est une table de 12 adresses (une par valeur de `Player::CHARACTER`) ;
//! `actions::act` appelle l'adresse de la table.
//!
//! Pour réécrire le code d'un personnage, deux possibilités équivalentes :
//!
//! 1. déclarer une `Native` à l'adresse d'entrée de la routine d'origine (voir `rom_address`) : toute
//!    exécution de la routine d'origine, y compris par ce noyau, passe par elle ;
//! 2. ajouter une ligne à `OVERRIDES` avec une fonction Rust qui reçoit le joueur : le noyau l'appelle
//!    à la place de la routine d'origine, sans passer par les registres du 68000.
//!
//! Les fonctions communes que les personnages réutilisent sont publiques : `checks::turn_check`,
//! `attack_button`, `jump_button`, `actions::to_stand`, `walk::walk_speed`, `walk::move_xy`,
//! `animation::start`, `animation::step`, `hurt::hurt_animation`...

/// Déclare une `Native` à l'adresse d'origine d'une action de personnage : la fonction Rust reçoit le
/// joueur (A0) ; aucun registre n'est lu par les appelants. Passer par une native (plutôt que par
/// `OVERRIDES`) permet la validation différentielle de chaque action (`DDSOM_CHECK=1`, `fuzz_player`).
macro_rules! action_native {
    ($name:ident, $entry:expr, $label:expr, $f:path) => {
        pub const $name: crate::Native = crate::Native {
            entry: $entry,
            name: $label,
            cost: |_, _| 100,
            run: |c, b| {
                let p = $crate::player::Player(c.a[0]);
                $f(&mut $crate::player::Ctx::new(c, b), p);
                $crate::mem::rts(c, b)
            },
            touches_sound: false,
            outputs: 0,
            exact_cost: false,
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

impl Slot {
    /// Table d'origine des adresses (un long par valeur de `Player::CHARACTER`), dans le code.
    pub const fn table(self) -> u32 {
        match self {
            Slot::Jump => 0x35918,
            Slot::Attack => 0x35972,
            Slot::Special => 0x359cc,
            Slot::Action10 => 0x35a0a,
            Slot::Action12 => 0x35a48,
        }
    }
}

/// Remplacements Rust du code des personnages : (valeur de `Player::CHARACTER`, action, routine).
/// À compléter par les agents qui réécrivent les personnages ; la routine d'origine n'est plus
/// appelée pour ces couples.
pub const OVERRIDES: &[(u8, Slot, CharacterFn)] = &[];

/// Adresse de la routine d'origine du personnage `character` pour l'action `slot`.
pub fn rom_address(g: &mut Ctx, slot: Slot, character: u8) -> u32 {
    g.prog32(slot.table() + 4 * character as u32)
}

/// Exécute l'action `slot` du personnage du joueur `p`.
pub fn run(g: &mut Ctx, p: Player, slot: Slot) {
    let character = p.u8(g, Player::CHARACTER);
    if let Some(&(_, _, f)) = OVERRIDES.iter().find(|o| o.0 == character && o.1 == slot) {
        return f(g, p);
    }
    let target = rom_address(g, slot, character);
    g.c.d[0] = p.u8(g, Player::ACTION) as u32; // D0 = numéro d'action, octet haut nul (comme l'aiguillage d'origine)
    g.rom(target, p);
}
