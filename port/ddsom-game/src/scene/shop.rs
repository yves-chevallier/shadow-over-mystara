//! État partagé `0xf792e` des sorties de scène : la boutique entre deux niveaux.
//!
//! Bloc `A5-0x5560` (A6 dans l'original) : `+0` état (pas de 2), `+1` sous-état de la sortie, `+2` sous-sous-état,
//! `+4` les objets des joueurs (quatre mots), `+0xc` numéro de boutique (table par scène `0xf7ad0`), `+0xd` nombre
//! de joueurs dans la boutique, `+0xe` minuterie, `+0x10` scène de retour, `+0x11` boutique déjà ouverte, `+0x12`
//! message en attente, `+0x14` message à ouvrir, `+0x16` message ouvert (mot), `+0x18` message en cours,
//! `+0x1a` liste de messages de sortie (long).
//!
//! La boutique est une scène (69 + numéro) : son SOBJ (type 13) et un EFFCT par joueur (type 20) font le jeu ;
//! cet état ouvre la scène, enchaîne les messages, puis ramène à la scène de retour (ou à la scène fixée par
//! `0xf7d5e`, ou recommence une boutique si `0xf7d72` le demande).

use super::Ctx;
use crate::mem::{prog16, prog32, prog8, rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::modes::{Flow, Sequence};
use crate::Native;
use crate::objects::{pools, Obj};

/// Bloc de la boutique.
pub const BLOCK: u32 = A5 - 0x5560;
mod f {
    pub const STATE: u32 = 0;
    pub const EXIT_STEP: u32 = 1;
    pub const EXIT_SUB: u32 = 2;
    pub const PLAYER_OBJECTS: u32 = 4;
    pub const SHOP: u32 = 0xc;
    pub const SHOPPERS: u32 = 0xd;
    pub const TIMER: u32 = 0xe;
    pub const RETURN_SCENE: u32 = 0x10;
    pub const REOPENED: u32 = 0x11;
    pub const PENDING: u32 = 0x12;
    pub const NEXT_MESSAGE: u32 = 0x14;
    pub const MESSAGE: u32 = 0x16;
    pub const TALKING: u32 = 0x18;
    pub const EXIT_MESSAGES: u32 = 0x1a;
}

/// Tables (`data` : lues comme données ; `pc` : relatives au PC).
mod rom {
    pub const DISPATCH: u32 = 0xf793e;
    /// Numéro de boutique par scène (data).
    pub const SHOP_OF_SCENE: u32 = 0xf7ad0;
    /// Musique de la boutique : décalage (data) dans une table de routines (data).
    pub const MUSIC_INDEX: u32 = 0xf7ab0;
    pub const MUSIC: u32 = 0xf7ac4;
    /// Son d'accueil : indice (data, 20 boutiques) dans une table de routines (data).
    pub const WELCOME_INDEX: u32 = 0xf7b88;
    pub const WELCOME: u32 = 0xf7b9c;
    pub const WELCOME_SHOPS: u8 = 20;
    /// Listes de messages de sortie : indice (pc) dans une table de pointeurs (pc).
    pub const EXIT_LIST_INDEX: u32 = 0xf7c14;
    pub const EXIT_LISTS: u32 = 0xf7c28;
    /// Boutique suivie d'une autre (data) ; scène imposée après la boutique (data, 0 : la scène de retour).
    pub const CHAINED: u32 = 0xf7d72;
    pub const NEXT_SCENE: u32 = 0xf7d5e;
    /// Portraits des joueurs : 8 octets par personnage (adresse, tuile, attributs), selon la disposition.
    pub const PORTRAITS_WIDE: u32 = 0xf7eb2;
    pub const PORTRAITS: u32 = 0xf7e92;
}

mod sub {
    pub const MESSAGE: u32 = 0xc8312;
    /// Chemin d'erreur de l'original (`tst.w 1(A5)` : erreur d'adresse) quand un pool est plein.
    pub const FULL: u32 = 0xf7dfa;
}

/// Tâche des portraits (créée à l'ouverture de la boutique).
const PORTRAITS_TASK: u32 = 0xf7dfe;

mod at {
    pub const OPEN_FADED: u32 = 0xf7960;
    pub const LEAVE_FADED: u32 = 0xf7c76;
    pub const LEAVE_WAITED: u32 = 0xf7c96;
    pub const CHAIN_FADED: u32 = 0xf7cfe;
    pub const REOPEN_WAITED: u32 = 0xf7d34;
}

fn b8(x: &mut Ctx, off: u32) -> u8 {
    rd8(x.b, BLOCK + off)
}
fn set_b8(x: &mut Ctx, off: u32, v: u8) {
    wr8(x.b, BLOCK + off, v)
}
fn b16(x: &mut Ctx, off: u32) -> u16 {
    rd16(x.b, BLOCK + off)
}
fn set_b16(x: &mut Ctx, off: u32, v: u16) {
    wr16(x.b, BLOCK + off, v)
}
fn add_b8(x: &mut Ctx, off: u32, v: u8) {
    let n = b8(x, off).wrapping_add(v);
    set_b8(x, off, n);
}

/// Un état de 0xf792e. A6 désigne le bloc pendant tout l'état, comme dans l'original.
pub fn run(x: &mut Ctx) -> Flow {
    x.c.a[6] = BLOCK;
    let s = b8(x, f::STATE);
    x.c.d[0] = s as u32;
    let off = prog16(x.b, rom::DISPATCH + s as u32);
    x.c.d[1] = x.c.d[1] & !0xffff | off as u32;
    match s {
        0 => open(x),
        2 => {
            let t = b16(x, f::TIMER).wrapping_sub(1);
            set_b16(x, f::TIMER, t);
            if (t as i16) < 0 {
                add_b8(x, f::STATE, 2);
            }
            Flow::Done
        }
        4 => welcome(x),
        6 => {
            if (b8(x, f::SHOPPERS) as i8) > 0 {
                messages(x);
                return Flow::Done;
            }
            set_b16(x, f::STATE, 0x800);
            x.call(0xe0a82);
            Flow::Done
        }
        8 => match b8(x, f::EXIT_STEP) {
            0 => {
                exit_messages(x);
                Flow::Done
            }
            4 => leave(x),
            _ => Flow::Done,
        },
        _ => Flow::Done,
    }
}

/// État 0 : fondu au noir (délai 2) avant d'ouvrir la boutique.
fn open(x: &mut Ctx) -> Flow {
    x.set_v8(0x42, 2);
    x.set_v16(0x490, 2);
    x.set_v8(0x48e, 15);
    x.sous_programme(crate::taches::fondu::descente, Etape::OpenFaded)
}

crate::etapes! {
    /// Étapes (points de reprise) de la boutique, et la tâche des portraits.
    pub enum Etape {
        /// Entrée de la tâche des portraits.
        Portraits = PORTRAITS_TASK => portraits,
        /// Ouverture : numéro de boutique, tâche des portraits, musique, objets de la boutique et des joueurs, puis la
        /// scène de la boutique (69 + numéro) est chargée et son script d'apparition relancé.
        OpenFaded = at::OPEN_FADED => |x| {
            x.call(0x1f184);
            x.call(0x2062);
            let scene = x.v8(0x34) as u32;
            let shop = rd8(x.b, rom::SHOP_OF_SCENE + scene);
            set_b8(x, f::SHOP, shop);
            let a0 = x.c.a[0];
            x.c.a[0] = PORTRAITS_TASK;
            x.c.d[0] = x.c.d[0] & !0xffff | 192;
            crate::kernel::creer_tache(x.b, x.c.d[0], x.c.a[0]);
            x.c.a[0] = a0;
            x.set_v8(0x4e, (scene == 5) as u8);
            if b8(x, f::REOPENED) == 0 {
                set_b8(x, f::RETURN_SCENE, scene as u8);
            }
            let index = rd8(x.b, rom::MUSIC_INDEX + shop as u32) as u32;
            x.c.d[0] = index;
            let music = rd32(x.b, rom::MUSIC + index);
            x.c.a[1] = music;
            x.call(music);
            set_b16(x, f::STATE, 0x200);
            set_b8(x, f::SHOPPERS, 0);
            set_b8(x, f::TALKING, 0);
            set_b16(x, f::PENDING, 0);
            set_b16(x, f::NEXT_MESSAGE, 0);
            set_b16(x, f::TIMER, 1);
            x.set_v8(0x3c, 4);
            set_b16(x, 0x1e, 0);
            set_b16(x, 0x20, 0);
            x.set_v8(0x3d, 0);
            x.call(0xca8);
            crate::hud::forget_displayed(x.b);
            let Some(Obj(o)) = pools::SOBJ.alloc_68k(x.c, x.b) else { return x.jump_to(sub::FULL) };
            wr8(x.b, o, 1);
            wr8(x.b, o + 2, 13);
            for k in 0..4u32 {
                let p = super::PLAYERS + k * super::PLAYER_STRIDE;
                let slot = BLOCK + f::PLAYER_OBJECTS + 2 * k;
                wr16(x.b, slot, 0);
                if rd8(x.b, p) == 0 || rd16(x.b, p + 0x62) == 0 {
                    continue;
                }
                x.c.a[2] = p;
                let Some(Obj(e)) = pools::EFFCT.alloc_68k(x.c, x.b) else { return x.jump_to(sub::FULL) };
                wr8(x.b, e, 1);
                wr8(x.b, e + 2, 20);
                let (variant, character) = (rd8(x.b, p + 3), rd8(x.b, p + 2));
                wr8(x.b, e + 3, variant);
                wr8(x.b, e + 0x2f, character);
                wr16(x.b, e + 0x50, p as u16);
                wr16(x.b, slot, e as u16);
                add_b8(x, f::SHOPPERS, 1);
            }
            x.c.d[7] = x.c.d[7] & !0xffff | 0xffff;
            let shop_scene = b8(x, f::SHOP).wrapping_add(69);
            x.set_v8(0x34, shop_scene);
            x.call(0x1a296);
            x.set_v8(0x48e, 15);
            x.call(0x168c);
            let saved = (x.c.d[7], x.c.a[0]);
            x.call(0x33aa6);
            (x.c.d[7], x.c.a[0]) = saved;
            x.c.d[0] = x.c.d[0] & !0xffff | 0x12c0;
            x.set_v16(0x14, 0);
            x.call(0xbc6);
            x.call(0xf2e56);
            x.call(0xf2e5c);
            Flow::Done
        },
        LeaveFaded = at::LEAVE_FADED => |x| {
            x.call(0xc58);
            let a0 = x.c.a[0];
            x.call(0x1f184);
            x.c.a[0] = a0;
            x.set_v8(0x3d, 1);
            crate::taches::une_image(x, Etape::LeaveWaited)
        },
        /// Retour de la boutique : couches effacées, état suivant de la séquence, scène de retour.
        LeaveWaited = at::LEAVE_WAITED => |x| {
            for r in [0xca8, 0xcc8, 0xce8] {
                x.call(r);
            }
            let a0 = x.c.a[0];
            let s = rd8(x.b, a0 + 2).wrapping_add(4);
            wr8(x.b, a0 + 2, s);
            x.set_v8(0x3c, 0xff);
            x.set_v8(0x59, 0);
            x.and16(0x12, 0xfffe);
            let layers = x.v16(0x12);
            wr16(x.b, 0x804122, layers);
            let shop = rd8(x.b, BLOCK + f::SHOP) as u32;
            x.c.d[0] = shop;
            x.c.a[1] = rom::NEXT_SCENE;
            let next = rd8(x.b, rom::NEXT_SCENE + shop);
            x.set_v8(0x34, next);
            if next == 0 {
                let back = rd8(x.b, BLOCK + f::RETURN_SCENE);
                x.set_v8(0x34, back);
            }
            Flow::Done
        },
        ChainFaded = at::CHAIN_FADED => |x| {
            x.c.d[0] = x.c.d[0] & !0xffff | 0x12c0;
            x.set_v16(0x14, 0);
            x.call(0xbc6);
            Flow::Done
        },
        ReopenWaited = at::REOPEN_WAITED => |x| {
            for r in [0x2134, 0x21bc, 0x20ae, 0x2150, 0xca8, 0xcc8, 0xce8] {
                x.call(r);
            }
            Flow::Done
        },
    }
}

/// État 4 : fondu d'ouverture (tâche 0x14ba, qui laisse A0 sur elle), premier message (101), son d'accueil.
fn welcome(x: &mut Ctx) -> Flow {
    add_b8(x, f::STATE, 2);
    x.c.d[0] = x.c.d[0] & !0xffff | 0x12ce;
    x.set_v16(0x14, 0);
    x.call(0xbc6);
    x.set_v16(0x490, 2);
    x.set_v8(0x48e, 15);
    x.c.d[0] = x.c.d[0] & !0xffff | 16;
    x.c.a[0] = 0x14ba;
    crate::kernel::creer_tache(x.b, x.c.d[0], x.c.a[0]);
    set_b16(x, f::PENDING, 101);
    let shop = b8(x, f::SHOP);
    if shop >= rom::WELCOME_SHOPS {
        // scènes de démonstration hors table : correctif commun aux deux machines (`exit::patch_shared_next`)
        x.c.d[0] &= 0xffff_0000;
        x.c.a[1] = rom::WELCOME_INDEX;
        return Flow::Done;
    }
    let index = rd8(x.b, rom::WELCOME_INDEX + shop as u32);
    x.c.d[0] = index as u32;
    if index == 0 {
        x.c.a[1] = rom::WELCOME_INDEX;
        return Flow::Done;
    }
    x.c.d[0] = 4 * index as u32;
    let sound = rd32(x.b, rom::WELCOME + 4 * index as u32);
    x.c.a[1] = sound;
    x.call(sound);
    Flow::Done
}

/// Messages (0xf7d86) : le message en attente passe en tête ; s'il n'y en a pas d'ouvert, il s'ouvre ; un message
/// ouvert qui dure encore est abrégé quand un autre attend.
fn messages(x: &mut Ctx) {
    if b16(x, f::NEXT_MESSAGE) == 0 {
        let p = b16(x, f::PENDING);
        set_b16(x, f::NEXT_MESSAGE, p);
        set_b16(x, f::PENDING, 0);
    }
    if b8(x, f::TALKING) == 0 {
        let n = b16(x, f::NEXT_MESSAGE);
        x.c.d[0] = x.c.d[0] & !0xffff | n as u32;
        if n == 0 {
            return;
        }
        x.call(sub::MESSAGE);
        if x.c.sr & m68k::SR_C != 0 {
            return;
        }
        set_b8(x, f::TALKING, 1);
        set_b16(x, f::MESSAGE, x.c.a[1] as u16);
        let p = b16(x, f::PENDING);
        set_b16(x, f::NEXT_MESSAGE, p);
        set_b16(x, f::PENDING, 0);
        return;
    }
    let m = rd16(x.b, BLOCK + f::MESSAGE) as i16 as i32 as u32;
    x.c.a[1] = m;
    let m = m & 0xff_ffff;
    if rd8(x.b, m) == 0 || rd8(x.b, m + 0x22) != 0 {
        set_b8(x, f::TALKING, 0);
        return;
    }
    if b16(x, f::NEXT_MESSAGE) == 0 || (rd16(x.b, m + 0x32) as i16) <= 1 {
        return;
    }
    wr16(x.b, m + 0x32, 1);
}

/// Sortie, étape 0 : messages de sortie de la liste de la boutique, l'un après l'autre.
fn exit_messages(x: &mut Ctx) {
    messages(x);
    x.c.d[0] = 0;
    match b8(x, f::EXIT_SUB) {
        0 => {
            let shop = b8(x, f::SHOP) as u32;
            let index = prog8(x.b, rom::EXIT_LIST_INDEX + shop) as u32;
            let list = prog32(x.b, rom::EXIT_LISTS + 4 * index);
            x.c.d[0] = 4 * index;
            wr32(x.b, BLOCK + f::EXIT_MESSAGES, list);
            if list == 0 {
                return exit_messages_done(x);
            }
            add_b8(x, f::EXIT_SUB, 4);
            next_exit_message(x);
        }
        4 => {
            if b8(x, f::TALKING) == 0 {
                next_exit_message(x);
            }
        }
        _ => {}
    }
}

fn next_exit_message(x: &mut Ctx) {
    let at = rd32(x.b, BLOCK + f::EXIT_MESSAGES);
    let n = rd16(x.b, at);
    x.c.d[0] = x.c.d[0] & !0xffff | n as u32;
    x.c.a[1] = at + 2;
    if n == 0 {
        return exit_messages_done(x);
    }
    set_b16(x, f::PENDING, n);
    wr32(x.b, BLOCK + f::EXIT_MESSAGES, at + 2);
}

fn exit_messages_done(x: &mut Ctx) {
    add_b8(x, f::EXIT_STEP, 4);
    set_b8(x, f::EXIT_SUB, 0);
}

/// Sortie, étape 4 : fondu au noir, puis retour à la scène de retour ; ou une boutique enchaînée.
fn leave(x: &mut Ctx) -> Flow {
    let shop = b8(x, f::SHOP) as u32;
    x.c.d[0] = shop;
    let chained = rd8(x.b, rom::CHAINED + shop);
    x.c.d[0] = chained as u32;
    x.c.a[1] = rom::CHAINED;
    if chained == 0 {
        x.set_v16(0x490, 2);
        x.set_v8(0x48e, 15);
        return x.sous_programme(crate::taches::fondu::descente, Etape::LeaveFaded);
    }
    if b8(x, f::EXIT_SUB) == 0 {
        add_b8(x, f::EXIT_SUB, 2);
        x.set_v16(0x490, 2);
        x.set_v8(0x48e, 15);
        return x.sous_programme(crate::taches::fondu::descente, Etape::ChainFaded);
    }
    // boutique suivante : le bloc repart de l'état 0
    x.and16(0x12, 0xfffe);
    set_b8(x, f::REOPENED, 1);
    x.call(0xc58);
    wr32(x.b, BLOCK, 0);
    let a0 = x.c.a[0];
    x.call(0x1f184);
    x.c.a[0] = a0;
    crate::taches::une_image(x, Etape::ReopenWaited)
}

/// Tâche des portraits (0xf7dfe) : pour chaque joueur en jeu, son portrait (deux colonnes) et le cadre qui le suit ;
/// puis la tâche se termine (`jmp 0x9f8`).
fn portraits(x: &mut Ctx) -> Flow {
    let b = &mut *x.b;
    let table = if rd8(b, A5 + 0x20) == 1 { rom::PORTRAITS_WIDE } else { rom::PORTRAITS };
    for k in 0..4 {
        let p = super::PLAYERS + k * super::PLAYER_STRIDE;
        if rd8(b, p) == 0 || (rd8(b, p + 4) as i8) >= 4 {
            continue;
        }
        let e = table + 8 * rd8(b, p + 2) as u32;
        let (at, tile, attr) = (rd32(b, e), rd16(b, e + 4), rd16(b, e + 6));
        super::tiles::glyph(b, at, tile, attr);
        super::tiles::glyph(b, at + 0x100, 0x1b2, attr);
    }
    crate::taches::terminer(x)
}

pub const SHOP: Native = Native {
    entry: 0xf792e,
    name: "scene_shop",
    run: |c, b| {
        let mut x = Ctx::new(c, b);
        if run(&mut x) == Flow::Done {
            crate::mem::rts(x.c, x.b);
        }
    },
    touches_sound: false,
    outputs: 0,
};

pub const NATIVES: &[Native] = &[
    SHOP,
];
