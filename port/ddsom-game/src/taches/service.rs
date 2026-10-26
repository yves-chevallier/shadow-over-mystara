//! Tâche du menu TEST (`0x29162`), créée par la tâche des crédits quand l'interrupteur TEST est actionné.
//!
//! À la création : enregistrement du menu remis à zéro, tâche de l'emplacement 15 (`0xf0`) tuée. Puis, à
//! chaque image, le gestionnaire de l'état du menu (`service::handler`), suivi d'une image d'attente. La
//! sortie (9. EXIT) remplace la tâche par la tâche de démarrage.
//!
//! | Adresse | Étape |
//! |---|---|
//! | `0x29162` | entrée |
//! | `0x29190` | après l'image d'attente : un tour |
//! | `0x2918a` | retour du gestionnaire quand il a cédé la main en route : l'image d'attente |

use crate::modes::{Flow, Sequence};
use crate::resume;
use crate::scene::Ctx;
use crate::service::{self, at, ext, MENU, MENU_LONGS};
use crate::Native;

const ENTRY: u32 = 0x29162;
const TURN: u32 = 0x29190;
/// Emplacement tué à l'entrée.
const KILLED_SLOT: u16 = 0xf0;

fn entry(x: &mut Ctx) -> Flow {
    for k in 0..MENU_LONGS {
        crate::mem::wr32(x.b, MENU + 4 * k, 0);
    }
    service::call_d0(x, ext::KILL_TASK, KILLED_SLOT);
    turn(x)
}

/// `0x2917a` : le gestionnaire, appelé comme par `jsr` (son retour est empilé) ; s'il rend la main sans
/// céder, l'image d'attente.
fn turn(x: &mut Ctx) -> Flow {
    x.push_return(at::HANDLER_RETURN);
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
    x.tail(ext::WAIT_ONE_FRAME, TURN)
}

resume!(R_ENTRY, ENTRY, entry);
resume!(R_TURN, TURN, turn);
resume!(R_HANDLER_RETURN, at::HANDLER_RETURN, wait);

pub const NATIVES: &[Native] = &[R_ENTRY, R_TURN, R_HANDLER_RETURN];
