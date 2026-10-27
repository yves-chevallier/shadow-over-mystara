//! Transitions du jeu normal selon les entrées (demi-tour, boutons) et événements périodiques.
//!
//! Chaque contrôle est appelé par les actions communes (`actions`) ou par le code des personnages
//! avant leur propre traitement, et renvoie vrai quand il a **changé l'état** du joueur (le
//! gestionnaire appelant s'arrête alors). Ce booléen est l'inverse du drapeau Z que l'original
//! laisse au retour (`bne` après `bsr` : Z levé = rien fait). Certains chemins de l'original laissent
//! un Z accidentel (dernier octet écrit nul) : ils sont reproduits à l'identique (`z_of`).
//!
//! Entrées (`KEYS_HELD`, `KEYS_NEW`) : bits 0..3 directions, 4 bouton 1 (attaque), 5 bouton 2 (saut),
//! 6 bouton 3 (lancer), 7 bouton 4 (spécial).

use crate::machine::Machine;
use super::actions::to_stand;
use super::{animation, combos};
use super::states::{action, main, state, sub, State};
use super::{ram, Ctx, Player};
use crate::mem::{rd16, rd8, wr8, A5};
use crate::objects::pools;

/// Boutons (bits de `KEYS_*`).
pub mod key {
    pub const DIRECTIONS: u8 = 0x0f;
    pub const HORIZONTAL: u8 = 0x03;
    pub const DOWN: u8 = 0x04;
    pub const UP: u8 = 0x08;
    pub const ATTACK: u8 = 0x10;
    pub const JUMP: u8 = 0x20;
    pub const THROW: u8 = 0x40;
    pub const SPECIAL: u8 = 0x80;
}


/// Orientation demandée par les directions (bit 0 : 0x20, sinon 0).
fn facing_for(keys: u8) -> u8 {
    if keys & 1 != 0 {
        0x20
    } else {
        0
    }
}

/// 0x36f7e : le joueur demande l'autre sens. En marche lente globale il se retourne sur place ;
/// sinon le demi-tour est une action (`TURN`) avec sa propre animation.
pub fn turn_check(g: &mut Ctx, p: Player) -> bool {
    cov!();
    let keys = p.u8(g, Player::KEYS_HELD) & key::HORIZONTAL;
    if keys == 0 || facing_for(keys) == p.u8(g, Player::FACING) {
        return false;
    }
    if ram::SLOW_MODE.get(g.b) != 0 {
        p.flip(g);
        return to_stand(g, p);
    }
    p.set_state(g, state::TURN_SLOW.0);
    let mut anim = 6;
    if p.u8(g, Player::SLOW) == 0 {
        p.set_state(g, state::TURN.0);
        anim = 0x30;
        if p.u8(g, Player::SLOW_8A) == 0 {
            anim = 5;
            let weapon_ok = p.u8(g, Player::WEAPON_B).wrapping_sub(0x28) <= 8
                && !matches!(p.u8(g, Player::WEAPON_A), 8 | 0x18);
            if weapon_ok && p.u8(g, Player::KEYS_HELD) & key::ATTACK != 0 {
                p.set_state(g, State::pack(main::PLAYING, sub::ACTING, action::TURN_ATTACK, 0).0);
                return true;
            }
        }
    }
    p.flip(g);
    p.set_u32(g, 0x1b2, 0);
    p.set_u16(g, 0x1b6, 0);
    animation::start(g, p, anim);
    true
}

/// 0x37018 : bouton 1 (attaque). Selon le contexte : coup au sol, coup en marche lente, ou
/// libération d'une prise.
pub fn attack_button(g: &mut Ctx, p: Player) -> bool {
    cov!();
    if ram::SLOW_MODE.get(g.b) != 0 || p.u8(g, Player::POWER_10F) != 0 {
        return false;
    }
    if p.u8(g, Player::KEYS_NEW) & key::ATTACK == 0 {
        return false;
    }
    if p.u8(g, Player::SLOW_8A) != 0 {
        // lâche ce qu'il tient : l'objet lié reçoit l'ordre de partir
        p.set_state(g, State::pack(main::PLAYING, sub::ACTING, action::RELEASE, 0).0);
        p.set_u8(g, 0x8b, 2);
        p.set_u8(g, Player::SLOW_8A, 0);
        let held = p.u32(g, Player::LINK);
        wr8(g.b, held + 5, 6);
        animation::start(g, p, 0x31);
        return true;
    }
    if p.u8(g, Player::SLOW) != 0 {
        p.set_u8(g, Player::SUBACTION, 8);
        p.set_state(g, state::ATTACK.0);
        return item_curse(g, p);
    }
    if !target_in_range(g, p) {
        // personne à portée : le type de coup dépend de l'action en cours
        let current = p.u8(g, Player::ACTION);
        let kind = if current == action::RUN {
            0xe
        } else if current == action::TURN {
            6
        } else {
            0
        };
        p.set_u8(g, Player::SUBACTION, kind);
    }
    p.set_state(g, state::ATTACK.0);
    item_curse(g, p)
}

/// 0x370b2 : bouton 2 (saut). La direction choisit l'animation et `+0x67` le type de saut :
/// 3 vertical, 0 sur place, 1 en avant, 2 après une course.
pub fn jump_button(g: &mut Ctx, p: Player) -> bool {
    cov!();
    if ram::FROZEN.get(g.b) != 0 || p.u8(g, 0x15c) != 0 || p.u8(g, Player::SLOW_8A) != 0 {
        return false;
    }
    let dir = p.u8(g, Player::KEYS_HELD) & key::DIRECTIONS;
    if p.u8(g, Player::KEYS_NEW) & key::JUMP == 0 {
        return false;
    }
    if p.u8(g, Player::SLOW) != 0 {
        p.set_state(g, State::pack(main::PLAYING, sub::ACTING, action::SLOW_SWITCH, 2).0);
        animation::start(g, p, 9);
        return true;
    }
    if dir == key::DOWN {
        p.set_state(g, State::pack(main::PLAYING, sub::ACTING, action::SLOW_SWITCH, 0).0);
        animation::start(g, p, 8);
        return true;
    }
    p.set_u8(g, 0x67, 3);
    let mut anim = 0xa;
    if dir != key::UP {
        p.set_u8(g, 0x67, 0);
        if dir & key::HORIZONTAL != 0 {
            p.set_u8(g, 0x67, 1);
            anim = 0xf;
        }
        if p.u8(g, Player::ACTION) == action::RUN {
            combos::clear_recognizers(g, p);
            p.set_u8(g, 0x67, 2);
            anim = 0x14;
        }
    }
    p.set_state(g, state::JUMP.0);
    animation::start(g, p, anim);
    true
}

/// Type de l'objet de lancer par personnage (octets du code, 0x371b2).
const THROW_KIND: u32 = 0x371b2;

/// 0x3714c : bouton 3 (lancer). Crée un effet joueur lié au joueur (`0x15a`). Renvoie vrai quand
/// l'original laisse Z à 0 : échec d'allocation, ou type d'effet non nul. Rien n'est fait si le bouton n'est
/// pas un nouvel appui ou si le joueur est occupé : Z levé.
pub fn throw_button(g: &mut Ctx, p: Player) -> bool {
    cov!();
    if p.u8(g, Player::KEYS_NEW) & key::THROW == 0
        || p.u8(g, 0x15c) != 0
        || p.u8(g, Player::SLOW_8A) != 0
        || p.u8(g, Player::POWER_10F) != 0
        || matches!(p.u8(g, Player::ACTION), 0xc | 0xe)
    {
        return false;
    }
    let Some(effect) = pools::PEFF.alloc(g.b) else {
        // l'allocation laisse C levé (`move #1,CCR`), donc Z à 0
        return true;
    };
    wr8(g.b, effect.0, 1);
    wr8(g.b, effect.0 + 3, 0xff);
    crate::mem::wr16(g.b, effect.0 + 0x50, p.0 as u16);
    p.set_u16(g, 0x15a, effect.0 as u16);
    p.set_u8(g, 0x15c, 1);
    let character = p.u8(g, Player::CHARACTER) as u32;
    let kind = g.prog8(THROW_KIND + character);
    wr8(g.b, effect.0 + 2, kind);
    kind != 0
}

/// Table (octets du code, 0x372e4) : animation de l'attaque spéciale selon l'identifiant mémorisé.
const SPECIAL_ANIM: u32 = 0x372e4;

/// 0x371be : bouton 4 (attaque spéciale), seulement en phase de jeu 0xc.
pub fn special_attack(g: &mut Ctx, p: Player) {
    cov!();
    if ram::PHASE.get(g.b) != 0xc || ram::PAUSE.get(g.b) != 0 {
        return;
    }
    if p.u8(g, Player::KEYS_NEW) & key::SPECIAL == 0 || p.u8(g, 0x15c) != 0 {
        return;
    }
    if p.u16(g, Player::MAIN) != 0x200 || p.u8(g, Player::POWER_10F) != 0 || p.u16(g, 0xc) != 0 {
        return;
    }
    if matches!(p.u8(g, Player::ACTION), 0xc | 0xe | 0x14 | 0x22) || p.u8(g, Player::SLOW_8A) != 0 {
        return;
    }
    // l'emplacement de sort choisi (`0x15d`) donne l'identifiant du sort en tête de sa ligne de 16 octets
    let slot = p.at(Player::SLOT_TABLE) + ((p.u8(g, Player::SPECIAL_COUNT) as u32) << 4);
    let id = rd8(g.b, slot);
    p.set_u8(g, Player::SPECIAL_ID, id);
    if id == 0x2d {
        let weapon_ok = p.u8(g, Player::WEAPON_B).wrapping_sub(0x28) <= 8
            && !matches!(p.u8(g, Player::WEAPON_A), 8 | 0x18);
        if weapon_ok {
            p.set_state(g, State::pack(main::PLAYING, sub::ACTING, action::SPECIAL_ALT, 0).0);
        }
        return;
    }
    if id == 0x56 {
        if p.u8(g, Player::SUBACTION) != 0x1c {
            p.set_u8(g, Player::SLOW, 0);
            p.set_u8(g, Player::SUBACTION, 0x1c);
            p.set_state(g, state::ATTACK.0);
        }
        return;
    }
    let ready = p.u8(g, Player::SPECIAL_COUNT) == 2 || {
        let charge = rd8(g.b, slot + 8) as u32 + 0x150;
        p.u8(g, charge) != 0
    };
    if ready {
        p.set_state(g, state::SPECIAL.0);
        let anim = g.prog8(SPECIAL_ANIM + id as u32);
        p.set_u8(g, 0x15e, anim);
    } else if p.state(g) & !0xff != 0x0200_3200 {
        p.set_state(g, State::pack(main::PLAYING, sub::ACTING, action::SPELL_FIZZLE, 0).0);
    }
}

/// 0x3760e : le magicien (personnage 8/9) avec l'arme 0x21 lance son sort ultime quand au moins
/// trois joueurs sont là, que leurs scores dépassent 2 000 000 et que tous sont prêts.
pub fn near_death(g: &mut Ctx, p: Player) {
    cov!();
    if ram::INPUT_MASK.get(g.b) & 0x80 == 0 || ram::PLAYERS_ALIVE.get(g.b) < 3 {
        return;
    }
    if !team_score_reached(g) || !combos::all_ready(g) {
        return;
    }
    if p.class(g) == 4 && p.u8(g, Player::WEAPON_B) == 0x21 {
        p.set_state(g, state::SPECIAL.0);
        p.set_u8(g, 0x15e, 0x24);
    }
}

/// Total des scores (BCD) des joueurs vivants, conservé en `A5-0x5fc6`.
pub const TEAM_SCORE: u32 = 0xff_203a;

/// 0x37658 : somme BCD des scores des joueurs en vie ; vrai si elle atteint 2 000 000.
pub fn team_score_reached(g: &mut Ctx) -> bool {
    cov!();
    crate::mem::wr32(g.b, TEAM_SCORE, 0);
    for q in Player::all() {
        if q.u8(g, Player::ACTIVE) == 0 || q.u16(g, Player::HP) == 0 {
            continue;
        }
        let mut carry = false;
        for i in (0..4).rev() {
            let sum = rd8(g.b, TEAM_SCORE + i);
            let add = q.u8(g, Player::SCORE_BCD + i);
            let (r, c) = bcd_add(sum, add, carry);
            wr8(g.b, TEAM_SCORE + i, r);
            carry = c;
        }
    }
    (crate::mem::rd32(g.b, TEAM_SCORE) as i32) >= 0x0200_0000
}

/// Addition décimale codée d'un octet (ABCD) avec retenue entrante ; renvoie le résultat et la retenue.
fn bcd_add(a: u8, b: u8, carry: bool) -> (u8, bool) {
    let mut r = a as u16 + b as u16 + carry as u16;
    if (a & 0xf) + (b & 0xf) + carry as u8 > 9 {
        r += 6;
    }
    let out = r > 0x99;
    if out {
        r += 0x60;
    }
    (r as u8, out)
}

/// Mots (code, 0x37784) : pour chaque tirage de 4 bits, le masque des tirages qui déclenchent l'effet.
const CURSE_MASKS: u32 = 0x37784;

/// 0x376ee : objets maudits (armes 0xe et 0xf) : à chaque coup, un tirage décide d'un effet (le
/// joueur lâche son arme, ou un monstre apparaît). Renvoie l'inverse de Z comme l'original.
pub fn item_curse(g: &mut Ctx, p: Player) -> bool {
    cov!();
    let weapon = p.u8(g, Player::WEAPON_A);
    if weapon != 0xe && weapon != 0xf {
        return true;
    }
    let pick = (g.random() & 0xf) as u32;
    let mask = g.prog16(CURSE_MASKS + 2 * pick);
    let bit = g.random() & 0xf;
    if mask >> bit & 1 != 0 {
        if weapon != 0xe {
            p.set_state(g, 0x0204_2600);
            super::spawn::cursed_weapon(g, p);
            // Z du dernier `move.w` du créateur (le lien), ou de l'allocation qui échoue (nul) : toujours vrai
            return g.c.sr & m68k::SR_Z == 0;
        }
        let count = ram::CURSE_COUNT.get(g.b).wrapping_add(1);
        ram::CURSE_COUNT.set(g.b, count);
        if count == 0x50 {
            return curse_fall(g, p);
        }
        let facing = p.u8(g, Player::FACING);
        p.set_u8(g, 0x4a, facing);
        p.set_state(g, 0x0204_3600);
        super::spawn::cursed_weapon(g, p);
    }
    if p.u8(g, Player::WEAPON_A) != 0xe {
        return true;
    }
    let count = ram::CURSE_COUNT.get(g.b).wrapping_add(1);
    ram::CURSE_COUNT.set(g.b, count);
    if count <= 0x4f {
        // `bls` : le drapeau Z reste celui de la comparaison
        return count != 0x4f;
    }
    curse_fall(g, p)
}

/// 0x37768 : le joueur maudit perd son arme (devient une arme 0xd) s'il est au sol.
fn curse_fall(g: &mut Ctx, p: Player) -> bool {
    cov!();
    if p.u16(g, 0xc) == 0 {
        p.set_u8(g, Player::WEAPON_A, 0xd);
        p.set_state(g, 0x0200_2600);
        wr8(g.b, CURSE_DROPPED, 1);
    }
    true
}

/// Drapeau en `A5-0x5b82` : une arme maudite a été lâchée (h).
const CURSE_DROPPED: u32 = A5 - 0x5b82;

/// Tables de boîtes de la ROM (données) : pointeur par jeu de boîtes, puis 8 octets par boîte.
mod boxes {
    pub const BODY: u32 = 0x11bffa;
    pub const ATTACK: u32 = 0x11c23a;
}

/// 0x371a : y a-t-il un boss ou un ennemi dans la portée du coup ? Teste la boîte d'attaque 1 (sous-
/// action 2) puis la boîte 2 (sous-action 4) du jeu de boîtes du joueur ; si l'une touche, `SUBACTION`
/// reçoit le numéro correspondant.
pub fn target_in_range(g: &mut Ctx, p: Player) -> bool {
    cov!();
    let box_set = p.u8(g, Player::BOX_SET) as u32;
    let set = g.data32(boxes::ATTACK + 4 * box_set);
    for (slot, sub) in [(1u32, 2u8), (2, 4)] {
        let attack_box = set + 8 * slot;
        if probe(g, p, attack_box, slot == 2) {
            p.set_u8(g, Player::SUBACTION, sub);
            return true;
        }
    }
    false
}

/// 0x3776 : les boss puis les ennemis.
fn probe(g: &mut Ctx, p: Player, attack_box: u32, second: bool) -> bool {
    cov!();
    pools::BOSS.iter().chain(pools::ENEMY.iter()).any(|o| touches(g, p, attack_box, o.0, second))
}

/// 0x3796 : la boîte d'attaque du joueur recouvre-t-elle la boîte de corps de l'objet `o` ?
/// Pour la première boîte, l'objet ne doit pas être en l'air ; pour la seconde, il doit être
/// porté (`+0x86`) ou être un ennemi de type 0xa.
fn touches(g: &mut Ctx, p: Player, attack_box: u32, o: u32, second: bool) -> bool {
    cov!();
    let b = &mut *g.b;
    if rd8(b, o) & 1 == 0 || rd8(b, o + 0x38) == 0 {
        return false;
    }
    if !second {
        if (rd16(b, o + 0xc) as i16) > 0 {
            return false;
        }
    } else if rd8(b, o + 0x86) == 0 && (rd8(b, o + 0x28) != 2 || rd8(b, o + 2) != 0xa) {
        return false;
    }
    let body_index = rd8(b, o + 0x2e) as u32;
    let body_set = crate::mem::rd32(b, boxes::BODY + 4 * body_index);
    let body_box = body_set + 8 * rd8(b, o + 0x38) as u32;
    // profondeur (Y)
    let (ya, yb) = (sbyte(b, attack_box + 4), sbyte(b, body_box + 4));
    let (pya, pyb) = (rd16(b, p.0 + 0x10), rd16(b, o + 0x10));
    if !overlap(ya, yb, pya, pyb, rd8(b, attack_box + 5), rd8(b, body_box + 5)) {
        return false;
    }
    // abscisse (X), décalages inversés selon l'orientation de chacun
    let (mut xa, mut xb) = (sbyte(b, attack_box), sbyte(b, body_box));
    if rd8(b, p.0 + 0x2c) != 0 {
        xa = xa.wrapping_neg();
    }
    if rd8(b, o + 0x2c) != 0 {
        xb = xb.wrapping_neg();
    }
    let (pxa, pxb) = (rd16(b, p.0 + 8), rd16(b, o + 8));
    if !overlap(xa, xb, pxa, pxb, rd8(b, attack_box + 1), rd8(b, body_box + 1)) {
        return false;
    }
    // altitude (Z)
    let (za, zb) = (sbyte(b, attack_box + 2), sbyte(b, body_box + 2));
    let (pza, pzb) = (rd16(b, p.0 + 0xc), rd16(b, o + 0xc));
    overlap(za, zb, pza, pzb, rd8(b, attack_box + 3), rd8(b, body_box + 3))
}

/// Octet signé étendu sur 16 bits.
fn sbyte(b: &mut cps2::Cps2Bus, a: u32) -> i16 {
    rd8(b, a) as i8 as i16
}

/// Recouvrement sur un axe, en arithmétique 16 bits comme l'original : `h` est la somme (sur 8 bits)
/// des demi-tailles, et `(décalage + position)` des deux boîtes doivent différer d'au plus `h`.
fn overlap(ofs_a: i16, ofs_b: i16, pos_a: u16, pos_b: u16, half_a: u8, half_b: u8) -> bool {
    let d = ofs_a.wrapping_add(pos_a as i16).wrapping_sub(ofs_b.wrapping_add(pos_b as i16));
    let h = half_a.wrapping_add(half_b) as u16;
    (d as u16).wrapping_add(h) <= h + h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bcd_addition() {
        assert_eq!(bcd_add(0x19, 0x01, false), (0x20, false));
        assert_eq!(bcd_add(0x99, 0x01, false), (0x00, true));
        assert_eq!(bcd_add(0x45, 0x55, true), (0x01, true));
    }

    #[test]
    fn box_overlap_on_an_axis() {
        // positions voisines, demi-tailles 2 + 2 : recouvrement
        assert!(overlap(0, 0, 10, 12, 2, 2));
        // trop éloignées
        assert!(!overlap(0, 0, 10, 30, 2, 2));
        // les décalages comptent
        assert!(overlap(-8, 0, 20, 12, 2, 2));
    }
}
