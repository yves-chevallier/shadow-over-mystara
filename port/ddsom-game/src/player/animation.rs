//! Animations des joueurs : jeux d'animations propres à chaque personnage (entrées longues).
//!
//! Le numéro d'une animation (`D6` dans l'original) est un index dans le jeu du personnage
//! (`Player::CHARACTER`). Les jeux sont des tableaux de décalages en ROM, atteints par une table de
//! pointeurs du code (voir `objects::anim` pour le format des scripts).


use super::{Ctx, Player};
use crate::objects::anim;

/// 0x2c16 : démarre l'animation `number` (décalée de 0x40 si le joueur n'a pas d'arme en main).
pub fn start(g: &mut Ctx, p: Player, number: u16) {
    anim::start_by_character(g.b, p.obj(), &anim::SETS_ARMED, number);
}

/// 0x2c88 : démarre l'animation `number` de la seconde table (sans décalage).
pub fn start_plain(g: &mut Ctx, p: Player, number: u16) {
    anim::start_by_character(g.b, p.obj(), &anim::SETS_PLAIN, number);
}

/// 0x2d58 : un pas d'animation. Renvoie vrai quand la retenue de l'original est levée : script terminé, ou
/// durée nulle qui repart de 0xFFFF (glitch d'origine, `anim::Step::Wrapped`).
pub fn step(g: &mut Ctx, p: Player) -> bool {
    anim::step(g.b, p.obj(), true).finished()
}
