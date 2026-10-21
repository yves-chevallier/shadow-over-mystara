//! État partagé `0xf7ecc` des sorties de scène : fondu au noir et préparation de la scène suivante.
//!
//! Trois sous-états (octet `+3` du bloc, pas de 2) : (0) fondu au noir, joueurs et objets remis à zéro, couches
//! effacées, caméras à l'origine (la scène 105, fin du jeu, garde une caméra placée) ; (1) palettes de la scène
//! suivante (table `0xf80c4`, quatre octets par scène) ; (2) script de transition propre à la scène (table de
//! sauts `0xf81fa`, variables `A5-0x5520`, module [`super::transition`]).

use super::bonus::{call_keep_a0, dispatch};
use super::screen;
use super::Ctx;
use crate::mem::{prog16, rd8, wr16, wr8};
use crate::modes::{Flow, Sequence};
use crate::{resume, Native};

const DISPATCH: u32 = 0xf7eda;
/// Palettes de la scène suivante : ensemble de la page 0 (négatif : aucun), puis D0 de 0x1a60a, 0x1a67e, 0x1a8b2.
const PALETTES: u32 = 0xf80c4;
/// Scripts de transition, par scène (mots relatifs à la table) : D1 de l'aiguillage d'origine.
const SCRIPTS: u32 = 0xf81fa;
/// La scène 105 (fin du jeu) prend les palettes et le script de la scène 69.
const FINALE: u8 = 105;
const FINALE_AS: u32 = 69;

mod at {
    pub const FADED: u32 = 0xf7f12;
    pub const CLEARED: u32 = 0xf7f60;
    pub const FINALE_1: u32 = 0xf802e;
    pub const FINALE_2: u32 = 0xf8042;
}

fn next_sub_step(x: &mut Ctx) {
    let a0 = x.c.a[0];
    let s = rd8(x.b, a0 + 3).wrapping_add(2);
    wr8(x.b, a0 + 3, s);
}

/// Scène dont on lit les tables.
fn table_scene(x: &mut Ctx) -> u32 {
    let scene = x.v8(0x34);
    if scene == FINALE { FINALE_AS } else { scene as u32 }
}

pub fn run(x: &mut Ctx) -> Flow {
    match dispatch(x, DISPATCH) {
        0 => {
            next_sub_step(x);
            x.set_v8(0x42, 2);
            x.set_v8(0x5f, 0);
            x.set_v16(0x490, 3);
            x.set_v8(0x48e, 15);
            x.tail(0x1460, at::FADED)
        }
        2 => {
            palettes(x);
            Flow::Done
        }
        4 => script(x),
        _ => Flow::Done,
    }
}

resume!(BLACKOUT_FADED, at::FADED, |x| {
    x.set_v8(0x48e, 15);
    x.call(0x168c);
    if x.v8(0x34) == FINALE {
        // fin du jeu : caméras placées sur le décor final
        for (off, v) in [(0x506, 0), (0x50a, 0x200), (0x51e, 4), (0x556, 0), (0x55a, 0x500), (0x56e, 4)] {
            x.set_v16(off, v);
        }
        let (a0, a6) = (x.c.a[0], x.c.a[6]);
        x.call(0x1de96);
        (x.c.a[0], x.c.a[6]) = (a0, a6);
        return x.wait(1, at::FINALE_1);
    }
    for off in [0x602, 0x802, 0xa02, 0xc02] {
        x.set_v32(off, 0);
    }
    for r in [0x1f184, 0x1de7c, 0xc58] {
        x.call(r);
    }
    x.set_v16(0x14, 0x12c0 & 0xfff1);
    x.wait(1, at::CLEARED)
});

// Couches effacées, défilements et caméras remis à zéro.
resume!(BLACKOUT_CLEARED, at::CLEARED, |x| {
    wr16(x.b, 0x400004, 0x1355);
    x.call(0xca8);
    x.set_v8(0x3c, 1);
    x.call(0xcc8);
    x.call(0xce8);
    let f = x.v8(0x13) & !1;
    x.set_v8(0x13, f);
    screen::clear_920000(x.b);
    for off in [0x4b6, 0x506, 0x556, 0x4ba, 0x50a, 0x55a] {
        x.set_v32(off, 0);
    }
    x.set_v16(0x52c, 0);
    x.set_v16(0x57c, 0);
    x.set_v8(0x36, 0);
    x.set_v8(0x53c, 0);
    x.set_v32(0x534, 0);
    super::bonus::reset_camera_copies(x);
    x.c.d[0] = 0;
    call_keep_a0(x, &[0x2062]);
    Flow::Done
});

resume!(BLACKOUT_FINALE_1, at::FINALE_1, |x| {
    x.set_v16(0x14, 0x12c0 & 0xfff1);
    x.wait(1, at::FINALE_2)
});
resume!(BLACKOUT_FINALE_2, at::FINALE_2, |x| {
    x.set_v8(0x3c, 1);
    Flow::Done
});

/// Sous-état 1 : les palettes de la scène suivante, puis l'écran de joueurs et le fondu.
fn palettes(x: &mut Ctx) {
    next_sub_step(x);
    let at = PALETTES + 4 * table_scene(x);
    let set = rd8(x.b, at);
    if (set as i8) >= 0 {
        x.c.d[0] = set as u32;
        x.call(0x1a3d4);
    }
    for (k, routine) in [0x1a60a, 0x1a67e, 0x1a8b2].into_iter().enumerate() {
        x.c.d[0] = rd8(x.b, at + 1 + k as u32) as u32;
        x.call(routine);
    }
    x.c.a[6] = at + 4;
    x.call(0x1507a);
    x.set_v8(0x48e, 12);
    x.call(0x168c);
}

/// Sous-état 2 : le script de transition de la scène (`transition`), avec les registres de l'aiguillage d'origine.
fn script(x: &mut Ctx) -> Flow {
    let scene = table_scene(x);
    let index = 2 * scene;
    x.c.d[0] = index;
    x.c.a[6] = super::transition::VARS;
    let off = prog16(x.b, SCRIPTS + index);
    x.c.d[1] = x.c.d[1] & !0xffff | off as u32;
    super::transition::run(x, scene)
}

pub const BLACKOUT: Native = Native {
    entry: 0xf7ecc,
    name: "scene_blackout",
    cost: |_, _| 100,
    run: |c, b| {
        let mut x = Ctx::new(c, b);
        if run(&mut x) == Flow::Done {
            crate::mem::rts(x.c, x.b);
        }
    },
    touches_sound: false,
    outputs: 0,
    exact_cost: false,
};

pub const NATIVES: &[Native] = &[BLACKOUT, BLACKOUT_FADED, BLACKOUT_CLEARED, BLACKOUT_FINALE_1, BLACKOUT_FINALE_2];
