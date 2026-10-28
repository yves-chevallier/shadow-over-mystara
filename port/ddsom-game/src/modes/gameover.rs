//! Phase 20 (`0x1c80c`) : fin de partie.
//!
//! La partie est finie (plus de joueur ni de crédit) : la tâche de l'interface des joueurs est tuée, le
//! décor s'efface en fondu, tout est vidé, puis la tâche principale est *remplacée* par la machine à modes
//! (`jmp 0xae4` avec A0 = `0x133fe`). Le drapeau de démarrage est levé : la machine commence par
//! l'écran de fin de partie (sous-mode 20, `ranking.rs`) au lieu du titre.
//!
//! Le remplacement de la tâche ne touche pas à sa pile : la machine à modes repart avec le pointeur de
//! pile que la tâche avait à sa dernière cession (celle de l'effacement vidéo, `0xc32`).

use super::{mode, Flow, Sequence};
use crate::mem::{rd16, rts, set_low_word, wr16, wr32, wr8, A5};
use crate::scene::Ctx;
use crate::Native;

mod ext {
    pub const PHASE_VARS_CLEAR: u32 = 0x1c44a;
    pub const MODE_LOOP: u32 = 0x133fe;
}

/// Tâche d'interface des joueurs (emplacement 0x30).
const PLAYER_UI_SLOT: u16 = 0x30;
const VIDEO_ON: u16 = 0x12ce;
const CPS_REG_VIDEO_B: u32 = 0x804166;
/// Reprises d'origine après chaque cession.
const FADED: u32 = 0x1c828;
const CLEARED: u32 = 0x1c846;

pub const PHASE_20: Native = Native {
    entry: 0x1c80c,
    name: "gameover_phase_20",
    run: |c, b| {
        let mut x = Ctx::new(c, b);
        if phase_20(&mut x) == Flow::Done {
            rts(x.c, x.b);
        }
    },
    touches_sound: false,
    outputs: 0,
};

/// Phase 20 de la partie (`0x1c80c`) : fin de partie.
pub fn phase_20(x: &mut Ctx) -> Flow {
    x.c.a[5] = A5;
    set_low_word(&mut x.c.d[0], PLAYER_UI_SLOT);
    crate::kernel::tuer_tache(x.b, PLAYER_UI_SLOT as u32);
    wr16(x.b, A5 + 0x490, 1);
    wr8(x.b, A5 + 0x48e, 0xf);
    x.sous_programme(crate::taches::fondu::descente, Etape::Faded)
}

crate::etapes! {
    /// Étapes (points de reprise) de la machine à états.
    pub enum Etape {
        Faded = FADED => |x| {
            set_low_word(&mut x.c.d[0], VIDEO_ON);
            wr16(x.b, CPS_REG_VIDEO_B, VIDEO_ON);
            wr16(x.b, A5 + 0x14, VIDEO_ON);
            wr8(x.b, A5 + 0x53c, 0);
            let flags = rd16(x.b, A5 + 0x12) & !1;
            wr16(x.b, A5 + 0x12, flags);
            x.sous_programme(crate::video::ecran_noir::debut, Etape::Cleared)
        },
        Cleared = CLEARED => |x| {
            x.call(ext::PHASE_VARS_CLEAR);
            wr8(x.b, A5 + 0x30, 1);
            reset_modes(x);
            wr8(x.b, mode::BOOTED, 1);
            crate::sound::service(x.b, crate::sound::Service::ArretGeneral);
            x.c.a[0] = ext::MODE_LOOP;
            crate::taches::recommencer(x)
        },
    }
}

/// `0x133dc` : remet à zéro le bloc d'état de la machine à modes (64 octets) et celui de l'écran de
/// classement (quatre longs à `FF2B10`, `0xfc562`).
pub fn reset_modes(x: &mut Ctx) {
    for i in 0..16 {
        wr32(x.b, mode::BASE + 4 * i, 0);
    }
    for i in 0..4 {
        wr32(x.b, 0xff_2b10 + 4 * i, 0);
    }
    x.c.a[6] = mode::BASE + 64;
    x.c.d[0] = 0;
    x.c.d[6] = 0xffff;
}

pub const NATIVES: &[Native] = &[PHASE_20];
