//! Tâche des invites (`0xe03fe`, emplacement 0xd0) : le texte « INSERT COIN(S) », « CONTINUE », etc. clignote,
//! affiché 40 images puis effacé 20 images, tant que la machine à modes ne l'arrête pas.
//!
//! Le texte dépend de la situation : partie gratuite (`A5+0x25`) ou réglage `A5+0x457` (texte imprimé par `0xd5e`,
//! effacé par `0xd32`), sinon un texte par réglage de pièces (`A5+0x1e`, table de longs `0xe048c` ; imprimé par
//! `0xf38`, effacé par `0xf4a`). Le texte affiché est gardé en `A5-0x5ac2` : `0xe03c6` l'efface en arrêtant la tâche,
//! `0xe03e4` la lance.

use super::{ceder, Attente};
use crate::mem::{rd32, wr32, A5};
use crate::modes::Flow;
use crate::scene::Ctx;
use crate::Native;

/// Texte affiché (long).
const SHOWN: u32 = A5 - 0x5ac2;
/// Emplacement de la tâche, et son entrée.
const SLOT: u32 = 0xd0;
const ENTRY: u32 = Etape::Entree.adresse();

mod var {
    pub const FREE_PLAY: u32 = 0x25;
    pub const SETTING: u32 = 0x457;
    pub const COINAGE: u32 = 0x1e;
}

/// Textes : par réglage de pièces (longs, le premier pour le réglage 1), partie gratuite, réglage `A5+0x457`.
mod text {
    pub const BY_COINAGE: u32 = 0xe048c;
    pub const FREE_PLAY: u32 = 0xe04b0;
    pub const SETTING: u32 = 0xe04e4;
}

/// Impressions et effacements de texte (A4 = texte).
mod print {
    pub const COINS: u32 = 0xf38;
    pub const COINS_CLEAR: u32 = 0xf4a;
    pub const PLAIN: u32 = 0xd5e;
    pub const PLAIN_CLEAR: u32 = 0xd32;
}

const SHOW_FRAMES: u32 = 40;
const HIDE_FRAMES: u32 = 20;

crate::etapes! {
    /// Étapes : l'entrée, puis après l'affichage et après l'effacement, pour chacune des deux familles de textes.
    pub enum Etape {
        Entree = 0xe03fe => entry,
        CoinsShown = 0xe0440 => |x| {
            print_shown(x, print::COINS_CLEAR);
            ceder(x, Attente::Images(HIDE_FRAMES), Etape::CoinsHidden)
        },
        CoinsHidden = 0xe0452 => |x| show(x, print::COINS, Etape::CoinsShown),
        PlainShown = 0xe0478 => |x| {
            print_shown(x, print::PLAIN_CLEAR);
            ceder(x, Attente::Images(HIDE_FRAMES), Etape::PlainHidden)
        },
        PlainHidden = 0xe048a => |x| show(x, print::PLAIN, Etape::PlainShown),
    }
}

fn entry(x: &mut Ctx) -> Flow {
    x.c.a[0] = SHOWN;
    if x.v8(var::FREE_PLAY) != 0 {
        wr32(x.b, SHOWN, text::FREE_PLAY);
        return show(x, print::COINS, Etape::CoinsShown);
    }
    if x.v8(var::SETTING) != 0 {
        wr32(x.b, SHOWN, text::SETTING);
        return show(x, print::PLAIN, Etape::PlainShown);
    }
    let k = 4 * x.v8(var::COINAGE) as u32;
    x.c.d[0] = k;
    let t = rd32(x.b, text::BY_COINAGE + k - 4);
    wr32(x.b, SHOWN, t);
    show(x, print::COINS, Etape::CoinsShown)
}

/// Imprime le texte gardé, puis l'attente d'affichage.
fn show(x: &mut Ctx, routine: u32, resume: Etape) -> Flow {
    print_shown(x, routine);
    ceder(x, Attente::Images(SHOW_FRAMES), resume)
}
fn print_shown(x: &mut Ctx, routine: u32) {
    x.c.a[4] = rd32(x.b, SHOWN);
    x.call(routine);
}

/// `0xe03c6` : efface le texte (comme le texte des pièces) et arrête la tâche (0xa14) ; D0 et A4 gardés.
fn stop(c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus) {
    let (d0, a4) = (c.d[0], c.a[4]);
    let mut x = Ctx::new(c, b);
    print_shown(&mut x, print::COINS_CLEAR);
    crate::kernel::tuer_tache(x.b, SLOT);
    (x.c.d[0], x.c.a[4]) = (d0, a4);
    crate::mem::rts(x.c, x.b);
}

/// `0xe03e4` : lance la tâche ; registres gardés.
fn start(b: &mut cps2::Cps2Bus) {
    crate::kernel::creer_tache(b, SLOT, ENTRY);
}

pub const STOP: Native = Native {
    entry: 0xe03c6,
    name: "invite_stop",
    run: crate::Corps::Processeur(stop),
    touches_sound: false,
    outputs: 0,
};
pub const START: Native = Native {
    entry: 0xe03e4,
    name: "invite_start",
    run: crate::Corps::Memoire(start),
    touches_sound: false,
    outputs: 0,
};

pub const NATIVES: &[Native] = &[STOP, START];
