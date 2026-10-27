//! Tâche du menu TEST (`0x29162`), créée par la tâche des crédits quand l'interrupteur TEST est actionné.
//!
//! À la création : enregistrement du menu remis à zéro, tâche de l'emplacement 15 (`0xf0`) tuée. Puis, à
//! chaque image, le gestionnaire de l'état du menu (`service::handler`), suivi d'une image d'attente. La
//! sortie (9. EXIT) remplace la tâche par la tâche de démarrage.

use crate::mem::set_low_word;
use crate::modes::{Flow, Sequence};
use crate::scene::Ctx;
use crate::service::{self, MENU, MENU_LONGS};

crate::etapes! {
    /// Étapes de la tâche.
    pub enum Etape {
        Entree = 0x29162 => entry,
        /// Après l'image d'attente : un tour.
        Turn = 0x29190 => turn,
        /// Retour du gestionnaire quand il a cédé la main en route : l'image d'attente.
        HandlerReturn = 0x2918a => wait,
        /// Reprise du menu après l'image d'attente qui suit l'effacement de l'écran (`service::show_menu`).
        MenuAfterClear = 0x291ec => service::menu_after_clear,
    }
}

/// Emplacement tué à l'entrée.
const KILLED_SLOT: u16 = 0xf0;

fn entry(x: &mut Ctx) -> Flow {
    for k in 0..MENU_LONGS {
        crate::mem::wr32(x.b, MENU + 4 * k, 0);
    }
    set_low_word(&mut x.c.d[0], KILLED_SLOT);
    crate::kernel::tuer_tache(x.b, KILLED_SLOT as u32);
    turn(x)
}

/// `0x2917a` : le gestionnaire, appelé comme par `jsr` (sa suite est un appel en cours) ; s'il rend la main sans
/// céder, l'image d'attente.
fn turn(x: &mut Ctx) -> Flow {
    x.push_return(Etape::HandlerReturn);
    match service::handler(x) {
        Flow::Done => {
            x.drop_return();
            wait(x)
        }
        Flow::Yield => Flow::Yield,
    }
}

/// `0x2918a` : `jsr 0xb64`, puis un nouveau tour.
fn wait(x: &mut Ctx) -> Flow {
    crate::taches::une_image(x, Etape::Turn)
}
