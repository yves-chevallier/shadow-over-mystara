//! Tâche des messages de pièces (`0xe03fe`, créée par `0xe03e4` à l'emplacement 0xd0) : sur l'écran des
//! crédits, le message du prix (« INSERT n COINS », « FREE PLAY », « CONTINUE REQUIRES ONLY 1 »...) clignote :
//! affiché 40 images, effacé 20, sans fin (la tâche est tuée par son créateur).
//!
//! Le message est choisi une fois pour toutes : jeu gratuit (`A5+0x25`), partie à deux crédits (`A5+0x457`,
//! message en texte simple, 0xd5e / 0xd32), sinon selon le nombre de pièces par crédit (`A5+0x1e`, table de
//! pointeurs en clair 0xe0488) ; il est dessiné en blocs de tuiles (0xf38 / 0xf4a). Son adresse reste en
//! `A5-0x5ac2`.

use crate::mem::{rd32, rd8, wr32, A5};
use crate::modes::{Flow, Sequence};
use crate::resume;
use crate::scene::Ctx;
use crate::Native;

const MESSAGE: u32 = A5 - 0x5ac2;
const FREE_PLAY: u32 = 0xe04b0;
const TWO_CREDITS: u32 = 0xe04e4;
const BY_PRICE: u32 = 0xe0488;
const SHOWN: u16 = 40;
const HIDDEN: u16 = 20;

/// Blocs de tuiles (message des crédits) ou texte simple (partie à deux crédits).
mod draw {
    pub const BLOCKS: u32 = 0xf38;
    pub const CLEAR_BLOCKS: u32 = 0xf4a;
    pub const TEXT: u32 = 0xd5e;
    pub const ERASE: u32 = 0xd32;
}

/// Adresses de reprise de l'original (après chaque `jsr 0xa40`).
mod at {
    pub const BLOCKS_SHOWN: u32 = 0xe0440;
    pub const BLOCKS_HIDDEN: u32 = 0xe0452;
    pub const TEXT_SHOWN: u32 = 0xe0478;
    pub const TEXT_HIDDEN: u32 = 0xe048a;
}

/// Dessine (ou efface) le message par la routine `routine`, puis attend `frames` images.
fn step(x: &mut Ctx, routine: u32, frames: u16, resume: u32) -> Flow {
    x.c.a[4] = rd32(x.b, MESSAGE);
    x.call(routine);
    x.wait(frames as u32, resume)
}

resume!(ENTRY, 0xe03fe, |x| {
    x.c.a[5] = A5;
    x.c.a[0] = MESSAGE;
    if rd8(x.b, A5 + 0x25) != 0 {
        wr32(x.b, MESSAGE, FREE_PLAY);
    } else if rd8(x.b, A5 + 0x457) != 0 {
        wr32(x.b, MESSAGE, TWO_CREDITS);
        return step(x, draw::TEXT, SHOWN, at::TEXT_SHOWN);
    } else {
        let price = rd8(x.b, A5 + 0x1e) as u32;
        x.c.d[0] = 4 * price;
        let message = rd32(x.b, BY_PRICE + 4 * price);
        wr32(x.b, MESSAGE, message);
    }
    step(x, draw::BLOCKS, SHOWN, at::BLOCKS_SHOWN)
});
resume!(BLOCKS_SHOWN, at::BLOCKS_SHOWN, |x| step(x, draw::CLEAR_BLOCKS, HIDDEN, at::BLOCKS_HIDDEN));
resume!(BLOCKS_HIDDEN, at::BLOCKS_HIDDEN, |x| step(x, draw::BLOCKS, SHOWN, at::BLOCKS_SHOWN));
resume!(TEXT_SHOWN, at::TEXT_SHOWN, |x| step(x, draw::ERASE, HIDDEN, at::TEXT_HIDDEN));
resume!(TEXT_HIDDEN, at::TEXT_HIDDEN, |x| step(x, draw::TEXT, SHOWN, at::TEXT_SHOWN));

/// 0xe03e4 : crée la tâche (emplacement 0xd0) ; D0 et A0-A4 sont rendus intacts.
fn create(c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus) {
    let (d0, a) = (c.d[0], c.a);
    c.a[0] = 0xe03fe;
    c.d[0] = 0xd0;
    crate::call(c, b, 0x9b6);
    c.d[0] = d0;
    c.a[..5].copy_from_slice(&a[..5]);
    crate::mem::rts(c, b);
}

pub const CREATE: Native = Native {
    entry: 0xe03e4,
    name: "tache_messages_creer",
    cost: |_, _| 200,
    run: create,
    touches_sound: false,
    outputs: 0,
    exact_cost: false,
};

pub const NATIVES: &[Native] = &[CREATE, ENTRY, BLOCKS_SHOWN, BLOCKS_HIDDEN, TEXT_SHOWN, TEXT_HIDDEN];
