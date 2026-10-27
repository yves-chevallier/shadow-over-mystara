//! Interface des joueurs (F3) : panneaux d'état, bannières « INSERT COIN / PUSH START », choix du
//! personnage et compte à rebours de reprise. Voir `docs/hud.md`.
//!
//! C'est une tâche permanente (`0x150ac`, créée en fin de phase 0 de la partie), qui boucle sur
//! `0x15178` : une fois par image (cession `0xb64`), elle met à jour la couche de caractères
//! (`0x912000`, voir `layer.rs`) pour chacun des 1 à 4 emplacements de joueur.
//!
//! ## Un tour (`tour`)
//!
//! * `A5+0x3c` est le *mode d'affichage* : négatif, la zone des panneaux est effacée (une fois, quand le mode
//!   change) ; 0, rien n'est dessiné ; 1 à 4, les panneaux sont mis à jour (`slot.rs`). Quand le mode change,
//!   chaque emplacement redessine ses zones (`mode_change`).
//! * un emplacement (`Slot`) a trois enregistrements de la RAM de travail : le panneau (`A5+0x30e + 0x50 k`,
//!   champs dans `panel`), le joueur (`A5+0x5de + 0x200 k`, champs dans `player`) et le compteur de pièces
//!   (`A5+0x44e + 0x10 k`, voir `credits.rs`). Selon `A5+0x344` (disposition : 4, 3, 2 ou 1 joueurs ; le bit 0
//!   choisit les colonnes étroites), seuls les premiers emplacements existent.
//! * `status.rs` : la ligne d'état commune (`0x17814`) et le compte à rebours de fin de partie (`0x175a8`).
//!
//! ## Machine à états d'un emplacement (`panel::STATE`, pas de 2)
//!
//! | État | Rôle |
//! |---|---|
//! | 0 | initialisation du panneau à partir du joueur (`enter`) |
//! | 2 | attente : bannière « INSERT COIN » ou « PUSH START » ; départ si un crédit est disponible (`idle`) |
//! | 4 | joueur dans la partie : mise à jour des jauges et chiffres du panneau (`play`) |
//! | 6 | choix du personnage, avec compte à rebours (`choose`) |
//! | 8 | reprise (« continue ») : compte à rebours de 9 à 0 (`resume`) |
//! | 0xa | fin de la reprise (`resume_end`) |
//! | 0xc | choix du personnage de la reprise (`resume_choose`) |
//!
//! Les routines d'origine non réécrites (comptabilité `0x2e4ec`, sons `0x2e630`/`0x2e696`, musique
//! `0xe12b8`...) sont appelées par `call`.

mod choice;
mod draw;
mod layer;
mod select;
mod slot;
mod status;

pub use slot::forget_displayed;

use cps2::Cps2Bus;

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::modes::Flow;
use crate::scene::Ctx;
use crate::Native;

/// Variables de la RAM de travail (adresses absolues ou décalages depuis A5).
pub mod var {
    /// Boutons de départ : état courant et état précédent (un bit par joueur).
    pub const START_NOW: u32 = 0x04;
    pub const START_BEFORE: u32 = 0x0a;
    /// Entrées par joueur (4 octets) et fronts montants (`+0x0c`, 4 octets).
    pub const INPUT: u32 = 0x00;
    pub const PRESSED: u32 = 0x0c;
    /// Mode des monnayeurs (0 à 11) : choisit le compteur de pièces d'un emplacement.
    pub const COIN_MODE: u32 = 0x1d;
    /// Prix d'une partie en pièces.
    pub const PRICE: u32 = 0x1e;
    /// Partie à deux crédits.
    pub const PRICE_TWO: u32 = 0x457;
    /// Mode « continue » : les crédits servent à reprendre une partie.
    pub const CONTINUE_MODE: u32 = 0x25;
    /// Reprise (« continue ») autorisée.
    pub const CONTINUE_ALLOWED: u32 = 0x23;
    pub const GAME_ON: u32 = 0x30;
    pub const PHASE: u32 = 0x2c;
    pub const STEP: u32 = 0x2d;
    /// Mode d'affichage des panneaux (voir le module).
    pub const DISPLAY: u32 = 0x3c;
    /// Pause : un bit par joueur.
    pub const PAUSE: u32 = 0x4a;
    /// Octet de réglages (bit 5 : seconde série de textes).
    pub const SETTINGS: u32 = 0x6d;
    /// Disposition : nombre d'emplacements = 4 - valeur ; bit 0 : colonnes étroites.
    pub const LAYOUT: u32 = 0x344;
    /// Premier enregistrement de panneau, premier joueur, premier compteur de pièces.
    pub const PANELS: u32 = 0x30e;
    pub const PLAYERS: u32 = 0x5de;
    pub const COINS: u32 = 0x44e;
    /// Dernier mode d'affichage traité, et dernière valeur de la pause (variables de la tâche).
    pub const SEEN_DISPLAY: u32 = 0xff_0f06;
    pub const SEEN_PAUSE: u32 = 0xff_0f07;
    /// Écran figé (A5-0x5b8e) : les bannières ne sont pas effacées.
    pub const FROZEN: u32 = 0xff_2472;
}

/// Champs d'un panneau (enregistrement de 0x50 octets).
pub mod panel {
    pub const SIZE: u32 = 0x50;
    /// Numéro du joueur (0 à 3).
    pub const INDEX: u32 = 0x00;
    pub const STATE: u32 = 0x04;
    /// Dernier PV et PV maximum affichés (mots), dernier score et dernier argent affichés (longs).
    pub const SHOWN_HP: u32 = 0x0e;
    pub const SHOWN_HP_MAX: u32 = 0x10;
    pub const SHOWN_SCORE: u32 = 0x12;
    pub const SHOWN_MONEY: u32 = 0x16;
    /// Jauge à redessiner (mot de PV non nul : jauge affichée).
    pub const HP_BAR: u32 = 0x08;
    pub const HP_DIRTY: u32 = 0x09;
    pub const SCORE_DIRTY: u32 = 0x0a;
    pub const MONEY_DIRTY: u32 = 0x0b;
    pub const CLASS: u32 = 0x0c;
    /// Objets affichés (deux longs) et drapeau « à redessiner ».
    pub const SHOWN_ITEMS: u32 = 0x1c;
    pub const PLAYER_DIRTY: u32 = 0x1b;
    pub const DISPLAYED_A: u32 = 0x24;
    pub const DISPLAYED_B: u32 = 0x25;
    /// Personnage sélectionné (pas de 2, bit 0 : variante).
    pub const CHOICE: u32 = 0x26;
    pub const FLAG_27: u32 = 0x27;
    /// Compte à rebours en BCD (`+0x28`) et sous-compteur d'images (`+0x29`).
    pub const COUNTDOWN: u32 = 0x28;
    pub const COUNTDOWN_FRAMES: u32 = 0x29;
    pub const FLAG_2A: u32 = 0x2a;
    pub const STEP: u32 = 0x2c;
    pub const SUBSTEP: u32 = 0x2d;
    pub const DELAY_2E: u32 = 0x2e;
    pub const LAST_MODE: u32 = 0x31;
    /// Images écoulées dans l'état (long).
    pub const FRAMES: u32 = 0x32;
    pub const CHOSEN: u32 = 0x37;
    pub const HINT: u32 = 0x38;
    pub const CLAIM: u32 = 0x39;
    pub const SHOWN_NAME: u32 = 0x40;
    /// Pièces comptées à l'ouverture de la reprise.
    pub const SHOWN_COINS: u32 = 0x2b;
}

/// Champs du joueur (enregistrement de 0x200 octets).
pub mod player {
    pub const SIZE: u32 = 0x200;
    pub const ACTIVE: u32 = 0x00;
    pub const FLAGS: u32 = 0x01;
    pub const NUMBER: u32 = 0x02;
    /// Personnage (pas de 2, bit 7 : choix verrouillé).
    pub const CHARACTER: u32 = 0x03;
    pub const STATE: u32 = 0x04;
    pub const HP_MAX: u32 = 0x60;
    pub const HP: u32 = 0x62;
    pub const CLASS: u32 = 0x10c;
    pub const SCORE: u32 = 0x110;
    pub const MONEY: u32 = 0x114;
    pub const ITEMS: u32 = 0x118;
}

/// Les trois enregistrements d'un emplacement.
#[derive(Clone, Copy)]
pub struct Slot {
    pub panel: u32,
    pub player: u32,
    pub coins: u32,
}

impl Slot {
    pub fn new(k: u32) -> Slot {
        Slot {
            panel: A5 + var::PANELS + panel::SIZE * k,
            player: A5 + var::PLAYERS + player::SIZE * k,
            coins: A5 + var::COINS + 0x10 * k,
        }
    }

    /// Octet, mot, long du panneau.
    pub fn p8(&self, b: &mut Cps2Bus, off: u32) -> u8 {
        rd8(b, self.panel + off)
    }
    pub fn p16(&self, b: &mut Cps2Bus, off: u32) -> u16 {
        rd16(b, self.panel + off)
    }
    pub fn p32(&self, b: &mut Cps2Bus, off: u32) -> u32 {
        rd32(b, self.panel + off)
    }
    pub fn set_p8(&self, b: &mut Cps2Bus, off: u32, v: u8) {
        wr8(b, self.panel + off, v)
    }
    pub fn set_p16(&self, b: &mut Cps2Bus, off: u32, v: u16) {
        wr16(b, self.panel + off, v)
    }
    pub fn set_p32(&self, b: &mut Cps2Bus, off: u32, v: u32) {
        wr32(b, self.panel + off, v)
    }

    /// Octet, mot, long du joueur.
    pub fn g8(&self, b: &mut Cps2Bus, off: u32) -> u8 {
        rd8(b, self.player + off)
    }
    pub fn g16(&self, b: &mut Cps2Bus, off: u32) -> u16 {
        rd16(b, self.player + off)
    }
    pub fn g32(&self, b: &mut Cps2Bus, off: u32) -> u32 {
        rd32(b, self.player + off)
    }
    pub fn set_g8(&self, b: &mut Cps2Bus, off: u32, v: u8) {
        wr8(b, self.player + off, v)
    }

    /// Numéro du joueur.
    pub fn index(&self, b: &mut Cps2Bus) -> u8 {
        self.p8(b, panel::INDEX)
    }
}

/// Octet de la RAM de travail à un décalage depuis A5.
pub fn v8(b: &mut Cps2Bus, off: u32) -> u8 {
    rd8(b, A5 + off)
}

/// Les emplacements mis à jour selon la disposition : 4, 3, puis toujours au moins 2.
fn slot_count(b: &mut Cps2Bus) -> u32 {
    match v8(b, var::LAYOUT) as i8 {
        0 => 4,
        1 => 3,
        _ => 2,
    }
}

/// Adresses de l'original (reprise de la tâche).
mod at {
    /// Entrée de la tâche, et reprise de l'attente du début de la partie.
    /// Disposition par type de borne (`A5+0x20`), table d'octets relative au PC.
    pub const LAYOUTS: u32 = 0x15164;
    /// Services de l'interface appelés à chaque tour (entrées des pièces, sons...).
    pub const SERVICES: [u32; 2] = [0x19b2a, 0x19b9c];
}

/// Type de borne (nombre de joueurs, h) : choisit la disposition.
const CABINET: u32 = 0x20;

crate::etapes! {
    /// Étapes de la tâche (le haut de la boucle, `0x15178`, n'est jamais une reprise : l'entrée y enchaîne).
    pub enum Etape {
        Entree = 0x150ac => entry,
        /// Attente du début de la partie.
        Starting = 0x15146 => starting,
        /// Après la cession d'un tour.
        AfterWait = 0x15212 => tour,
    }
}

/// 0x150ac : entrée de la tâche. Panneaux remis à zéro (numéro de joueur en `+0`), variables de la tâche, puis
/// attente du début de la partie.
fn entry(x: &mut Ctx) -> Flow {
    x.c.d[0] = 0;
    for k in 0..4 {
        let at = A5 + var::PANELS + k * panel::SIZE;
        wr32(x.b, at, 0);
        wr32(x.b, at + 4, 0);
        for off in [panel::STEP, panel::SUBSTEP, 0x30] {
            wr8(x.b, at + off, 0);
        }
        wr32(x.b, at + panel::FRAMES, 0);
    }
    wr32(x.b, 0xff_0eca, 0);
    wr8(x.b, 0xff_0ec6, 0);
    wr8(x.b, var::SEEN_DISPLAY, 0x80);
    wr8(x.b, 0xff_0f08, 0);
    x.set_v8(0x5f, 1);
    let pause = x.v8(var::PAUSE);
    wr8(x.b, var::SEEN_PAUSE, pause);
    for k in 1..4 {
        x.set_v8(var::PANELS + k * panel::SIZE + panel::INDEX, k as u8);
    }
    crate::taches::une_image(x, Etape::Starting)
}

/// 0x15146 : disposition selon la borne ; hors partie, panneaux affichés en disposition 0 ; en partie, attente de
/// la phase 4 (choix des personnages) avant la première mise à jour.
fn starting(x: &mut Ctx) -> Flow {
    let cabinet = x.v8(CABINET);
    x.c.d[0] = x.c.d[0] & !0xff | cabinet as u32;
    let layout = x.prog8(at::LAYOUTS + cabinet as u32);
    x.set_v8(var::LAYOUT, layout);
    if x.v8(var::GAME_ON) == 0 {
        x.set_v8(var::DISPLAY, 1);
        x.set_v8(var::LAYOUT, 0);
        slot::forget_displayed(x.b);
    } else if (x.v8(var::PHASE) as i8) < 4 {
        return crate::taches::une_image(x, Etape::Starting);
    }
    tour(x)
}

/// 0x1507a, appelée par d'autres modules.
pub const FORGET_DISPLAYED: Native = Native {
    entry: 0x1507a,
    name: "hud_forget_displayed",
    run: |c, b| {
        slot::forget_displayed(b);
        crate::mem::rts(c, b);
    },
    touches_sound: false,
    outputs: 0,
};


/// Un tour de la tâche : mise à jour des panneaux selon le mode d'affichage, services, puis cession.
fn tour(x: &mut Ctx) -> Flow {
    let mode = x.v8(var::DISPLAY);
    let seen = rd8(x.b, var::SEEN_DISPLAY);
    if (mode as i8) < 0 {
        if mode != seen {
            wr8(x.b, var::SEEN_DISPLAY, mode);
            draw::clear_panels(x.b);
        }
    } else {
        if mode != seen {
            slot::forget_displayed(x.b);
        }
        wr8(x.b, var::SEEN_DISPLAY, mode);
        for k in 0..slot_count(x.b) {
            slot::step(x, Slot::new(k));
        }
        status::status_line(x.b);
        let pause = x.v8(var::PAUSE);
        wr8(x.b, var::SEEN_PAUSE, pause);
        status::countdown(x);
    }
    for service in at::SERVICES {
        x.call(service);
    }
    crate::taches::une_image(x, Etape::AfterWait)
}

/// Routines natives de l'interface des joueurs.
pub const NATIVES: &[Native] = &[FORGET_DISPLAYED];
