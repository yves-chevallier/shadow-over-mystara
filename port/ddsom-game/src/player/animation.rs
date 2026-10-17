//! Animations des joueurs : jeux d'animations propres à chaque personnage (entrées longues).
//!
//! Le numéro d'une animation (`D6` dans l'original) est un index dans le jeu du personnage
//! (`Player::CHARACTER`). Les jeux sont des tableaux de décalages en ROM, atteints par une table de
//! pointeurs du code (voir `objects::anim` pour le format des scripts).

use cps2::Cps2Bus;
use m68k::Bus;

use super::{Ctx, Player};
use crate::mem::{rd16, rd8};
use crate::objects::anim::{self, Step};

/// Table de pointeurs de jeux (un par personnage) lue dans le code : sans arme (`0x68` nul), les
/// numéros sont décalés de 0x40.
const SETS_WEAPON: u32 = 0x2c58;
/// Seconde table de jeux, numéro direct.
const SETS_PLAIN: u32 = 0x2cc0;

/// Octet `0x68` : le personnage tient son arme.
const ARMED: u32 = 0x68;

fn prog32(b: &mut Cps2Bus, a: u32) -> u32 {
    (b.read_prog16(a & 0xff_ffff) as u32) << 16 | b.read_prog16(a.wrapping_add(2) & 0xff_ffff) as u32
}

/// Charge l'entrée de départ de l'animation `number` du jeu pointé par `table`.
fn start_in(b: &mut Cps2Bus, p: Player, table: u32, number: u16) {
    let variant = rd8(b, p.at(Player::CHARACTER)) as i8 as i16 as u16;
    let set = prog32(b, table.wrapping_add((variant << 2) as i16 as i32 as u32));
    let off = rd16(b, set.wrapping_add((number.wrapping_add(number)) as i16 as i32 as u32));
    anim::load_entry(b, p.obj(), set.wrapping_add(off as i16 as i32 as u32), true);
}

/// 0x2c16 : démarre l'animation `number` (décalée de 0x40 si le joueur n'a pas d'arme en main).
pub fn start(g: &mut Ctx, p: Player, number: u16) {
    let armed = rd8(g.b, p.at(ARMED)) != 0;
    let number = if armed { number } else { number.wrapping_add(0x40) };
    start_in(g.b, p, SETS_WEAPON, number);
}

/// 0x2c88 : démarre l'animation `number` de la seconde table (sans décalage).
pub fn start_plain(g: &mut Ctx, p: Player, number: u16) {
    start_in(g.b, p, SETS_PLAIN, number);
}

/// 0x2d58 : un pas d'animation. Renvoie vrai quand le script est terminé (retenue de l'original).
pub fn step(g: &mut Ctx, p: Player) -> bool {
    anim::step(g.b, p.obj(), true) == Step::Ended
}
