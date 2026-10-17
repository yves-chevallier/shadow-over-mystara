//! Valeurs de la machine à états des joueurs.
//!
//! Les octets `+4..+7` de l'enregistrement forment un long : état principal, sous-état, action et pas
//! de l'action. Le code d'origine écrit souvent le long d'un coup (`move.l #0x02000c00,(4,A0)`) ;
//! `State::pack` reproduit ces constantes.

/// État principal (`+4`).
pub mod main {
    /// Scénarisé : cinématiques par scène ; `+5` choisit le sous-état (P3).
    pub const SCRIPTED: u8 = 0;
    /// Jeu normal.
    pub const PLAYING: u8 = 2;
    /// À terre / expulsé.
    pub const DOWN: u8 = 4;
    /// Retiré de la partie.
    pub const REMOVED: u8 = 6;
}

/// Sous-état (`+5`) du jeu normal.
pub mod sub {
    /// Le joueur agit : l'action (`+6`) est exécutée.
    pub const ACTING: u8 = 0;
    /// Touché : l'action (`+6`) est une réaction (`hurt`).
    pub const HURT: u8 = 4;
    /// Parade.
    pub const PARRY: u8 = 8;
}

/// Actions du jeu normal (`+6`, toujours paires : indices de la table de gestionnaires 0x35586).
/// Les noms sont déduits des endroits où l'action est choisie ; (h) = hypothèse.
pub mod action {
    /// Immobile.
    pub const STAND: u8 = 0x00;
    /// Entrée dans la marche lente (`Player::SLOW`) ou retour à la marche normale (bas + saut) (h).
    pub const SLOW_SWITCH: u8 = 0x02;
    /// Marche.
    pub const WALK: u8 = 0x04;
    /// Course.
    pub const RUN: u8 = 0x06;
    /// Demi-tour.
    pub const TURN: u8 = 0x08;
    /// Saut (code propre au personnage, `characters`).
    pub const JUMP: u8 = 0x0a;
    /// Attaque du bouton 1 (propre au personnage).
    pub const ATTACK: u8 = 0x0c;
    /// Attaque spéciale du bouton 4 (propre au personnage).
    pub const SPECIAL: u8 = 0x0e;
    /// Relâche l'objet tenu (le joueur porte une caisse ou un adversaire) (h).
    pub const RELEASE: u8 = 0x14;
    /// Attaque spéciale n° 0x2d (arme 0x28..0x30) (h).
    pub const SPECIAL_ALT: u8 = 0x16;
    /// Attaque pendant un demi-tour, avec les armes 0x28..0x30 (h).
    pub const TURN_ATTACK: u8 = 0x18;
    /// Saisi par un adversaire (le joueur est tenu).
    pub const GRAB: u8 = 0x22;
    /// Sort sans charge restante : animation d'échec (h).
    pub const SPELL_FIZZLE: u8 = 0x32;
    /// Ces actions ne subissent ni la collision avec le décor ni le recadrage de l'écran.
    pub const NO_TERRAIN_A: u8 = 0x20;
    pub const NO_TERRAIN_B: u8 = 0x2a;
    pub const NO_TERRAIN_C: u8 = 0x24;
}

/// Un état complet (`+4..+7`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct State(pub u32);

impl State {
    /// Assemble les quatre octets.
    pub const fn pack(main: u8, sub: u8, action: u8, step: u8) -> State {
        State((main as u32) << 24 | (sub as u32) << 16 | (action as u32) << 8 | step as u32)
    }

    /// Le long sans l'octet de pas (comparaisons `andi.l #-0x100`).
    pub const fn without_step(self) -> u32 {
        self.0 & !0xff
    }

    pub const fn main(self) -> u8 {
        (self.0 >> 24) as u8
    }
    pub const fn sub(self) -> u8 {
        (self.0 >> 16) as u8
    }
    pub const fn action(self) -> u8 {
        (self.0 >> 8) as u8
    }
    pub const fn step(self) -> u8 {
        self.0 as u8
    }
}

/// États écrits par le code commun.
pub mod state {
    use super::{action, main, sub, State};

    /// Immobile, animation en cours (pas 2).
    pub const STAND: State = State::pack(main::PLAYING, sub::ACTING, action::STAND, 2);
    /// Marche, départ.
    pub const WALK: State = State::pack(main::PLAYING, sub::ACTING, action::WALK, 0);
    /// Demi-tour (pas 2 : marche lente).
    pub const TURN: State = State::pack(main::PLAYING, sub::ACTING, action::TURN, 0);
    pub const TURN_SLOW: State = State::pack(main::PLAYING, sub::ACTING, action::TURN, 2);
    /// Attaque du bouton 1.
    pub const ATTACK: State = State::pack(main::PLAYING, sub::ACTING, action::ATTACK, 0);
    /// Saut.
    pub const JUMP: State = State::pack(main::PLAYING, sub::ACTING, action::JUMP, 0);
    /// Attaque spéciale.
    pub const SPECIAL: State = State::pack(main::PLAYING, sub::ACTING, action::SPECIAL, 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_states_match_the_original_constants() {
        assert_eq!(state::STAND.0, 0x0200_0002);
        assert_eq!(state::ATTACK.0, 0x0200_0c00);
        assert_eq!(State::pack(2, 4, 0, 0).sub(), sub::HURT);
        assert_eq!(State(0x0200_0e01).without_step(), 0x0200_0e00);
        assert_eq!(State(0x0200_0e01).action(), action::SPECIAL);
    }
}
