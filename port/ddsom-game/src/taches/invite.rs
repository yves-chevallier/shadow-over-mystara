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

/// Impressions et effacements du texte gardé : blocs (0xf38, 0xf4a) ou texte simple (0xd5e, 0xd32). Les blocs ne
/// lisent l'objet A0 que pour des attributs négatifs : jamais pour ces textes (attribut 3).
#[derive(Clone, Copy)]
enum Impression {
    Pieces,
    EffacerPieces,
    Simple,
    EffacerSimple,
}


const SHOW_FRAMES: u32 = 40;
const HIDE_FRAMES: u32 = 20;

crate::etapes! {
    /// Étapes : l'entrée, puis après l'affichage et après l'effacement, pour chacune des deux familles de textes.
    pub enum Etape {
        Entree = 0xe03fe => entry,
        CoinsShown = 0xe0440 => |x| {
            print_shown(x, Impression::EffacerPieces);
            ceder(x, Attente::Images(HIDE_FRAMES), Etape::CoinsHidden)
        },
        CoinsHidden = 0xe0452 => |x| show(x, Impression::Pieces, Etape::CoinsShown),
        PlainShown = 0xe0478 => |x| {
            print_shown(x, Impression::EffacerSimple);
            ceder(x, Attente::Images(HIDE_FRAMES), Etape::PlainHidden)
        },
        PlainHidden = 0xe048a => |x| show(x, Impression::Simple, Etape::PlainShown),
    }
}

fn entry(x: &mut Ctx) -> Flow {
    x.c.a[0] = SHOWN;
    if x.v8(var::FREE_PLAY) != 0 {
        wr32(x.b, SHOWN, text::FREE_PLAY);
        return show(x, Impression::Pieces, Etape::CoinsShown);
    }
    if x.v8(var::SETTING) != 0 {
        wr32(x.b, SHOWN, text::SETTING);
        return show(x, Impression::Simple, Etape::PlainShown);
    }
    let k = 4 * x.v8(var::COINAGE) as u32;
    x.c.d[0] = k;
    let t = rd32(x.b, text::BY_COINAGE + k - 4);
    wr32(x.b, SHOWN, t);
    show(x, Impression::Pieces, Etape::CoinsShown)
}

/// Imprime le texte gardé, puis l'attente d'affichage.
fn show(x: &mut Ctx, impression: Impression, resume: Etape) -> Flow {
    print_shown(x, impression);
    ceder(x, Attente::Images(SHOW_FRAMES), resume)
}
fn print_shown(x: &mut Ctx, impression: Impression) {
    let texte = rd32(x.b, SHOWN);
    match impression {
        Impression::Pieces => crate::text::draw_blocks(x.b, texte, 0),
        Impression::EffacerPieces => crate::text::clear_blocks(x.b, texte, 0),
        Impression::Simple => crate::text::print(x.b, texte),
        Impression::EffacerSimple => crate::text::erase(x.b, texte),
    };
}

/// `0xe03c6` : efface le texte (comme le texte des pièces, 0xf4a) et arrête la tâche (0xa14) ; aucun registre ne
/// change. L'effacement ne lit l'objet A0 de l'appelant que si les attributs du texte sont négatifs : jamais pour
/// les textes que la tâche garde (attribut 3, tables 0xe048c, 0xe04b0, 0xe04e4).
pub fn arreter(b: &mut cps2::Cps2Bus) {
    let texte = rd32(b, SHOWN);
    crate::text::clear_blocks(b, texte, 0);
    crate::kernel::tuer_tache(b, SLOT);
}

/// `0xe03e4` : lance la tâche ; registres gardés.
pub fn lancer(b: &mut cps2::Cps2Bus) {
    crate::kernel::creer_tache(b, SLOT, ENTRY);
}

pub const STOP: Native = Native {
    entry: 0xe03c6,
    name: "invite_stop",
    run: crate::Corps::Memoire(arreter),
    touches_sound: false,
    outputs: 0,
};
pub const START: Native = Native {
    entry: 0xe03e4,
    name: "invite_start",
    run: crate::Corps::Memoire(lancer),
    touches_sound: false,
    outputs: 0,
};

pub const NATIVES: &[Native] = &[STOP, START];
