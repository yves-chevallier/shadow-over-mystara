//! Tâche de démarrage (`0x11b16`) : initialisation du jeu, deux écrans d'avertissement selon la région,
//! création des tâches des crédits et du gel, puis la machine à modes.
//!
//! Chaque avertissement reste à l'écran deux ou trois secondes ; un bouton (bit 4 du port `IN1`) accélère
//! le décompte. Le texte dépend de la région (octet 0xc4 du programme et réglage `A5+0x70`).

use super::{Flow, Sequence};
use crate::video::Couche;
use crate::mem::{rd16, rd32, rd8, set_low_word, wr16, wr32, wr8, A5};
use crate::scene::Ctx;
use crate::Native;

/// Tâches créées à la fin du démarrage : crédits et gel.
mod tache {
    pub const CREDITS: u32 = 0x13c0a;
    pub const GEL: u32 = 0xee080;
}

/// Palette de départ : huit longs de la ROM copiés dans la table des palettes.
const START_PALETTE_SRC: u32 = 0x133ba;
const START_PALETTE_DST: u32 = 0x91c580;
/// Région du jeu (octet du programme) et tables de textes (une par région, indexées par `A5+0x70`).
const REGION_BYTE: u32 = 0xc4;
const WARNING_TABLES: [u32; 3] = [0x11cba, 0x11cd2, 0x11cea];
const NOTICE_TABLE: u32 = 0x11d02;
/// Port des boutons (bit 4 : un bouton pressé accélère le décompte).
const PORT_IN1: u32 = 0x804001;
/// Décompte des écrans d'avertissement : mot à FF0E6E.
const COUNTDOWN: u32 = A5 - 0x7192;
const WARNING_FRAMES: u16 = 0x78;
const NOTICE_FRAMES: u16 = 0xb4;
const CPS_REG_VIDEO: u32 = 0x804122;
const CPS_REG_VIDEO_B: u32 = 0x804166;
const SLOT_CREDITS: u32 = 0x20;
const SLOT_FREEZE: u32 = 0xe0;

const INIT: u32 = 0x11b22;
crate::etapes! {
    /// Étapes (points de reprise) de la machine à états.
    pub enum Etape {
        /// Entrée de la tâche (`0x11b16`) : écran noir, puis l'initialisation.
        Entree = 0x11b16 => |x| x.sous_programme(crate::video::ecran_noir::debut, Etape::Init),
        Init = INIT => |x| {
            crate::palette::charger_tout(x.b);
            for i in 0..8 {
                let v = rd32(x.b, START_PALETTE_SRC + 4 * i);
                wr32(x.b, START_PALETTE_DST + 4 * i, v);
            }
            wr8(x.b, A5 + 0x48e, 0xf);
            crate::palette::full_brightness(x.b);
            if !crate::config::initialiser(x.c, x.b) {
                return Flow::Yield;
            }
            crate::credits::initialiser(x.b);
            crate::evenements::vider_points(x.b);
            crate::evenements::vider_compteur(x.b);
            crate::evenements::vider_popups(x.b);
            crate::sound::vider_pile_privee(x.b);
            crate::player::init::effacer_joueurs(x.c, x.b);
            crate::player::init::effacer_sous_objets(x.c, x.b);
            clear_mode_state(x);
            crate::rng::seed(x.b);
            default_high_scores(x);
            crate::video::copier_ecrans_fixes(x.b);
            crate::families::items::effacer_ramasses(x.b);
            crate::taches::partie::effacer_variables(x.c, x.b);
            wr8(x.b, A5 - 0x713c, 0);
            set_low_word(&mut x.c.d[0], 0xe);
            wr16(x.b, CPS_REG_VIDEO, 0xe);
            wr16(x.b, A5 + 0x12, 0xe);
            set_low_word(&mut x.c.d[0], 0x12ce);
            wr16(x.b, CPS_REG_VIDEO_B, 0x12ce);
            wr16(x.b, A5 + 0x14, 0x12ce);
            let region = rd8(x.b, REGION_BYTE);
            let table = match region {
                0 => WARNING_TABLES[0],
                1 => WARNING_TABLES[1],
                _ => WARNING_TABLES[2],
            };
            print_region_text(x, table);
            wr16(x.b, COUNTDOWN, WARNING_FRAMES);
            x.wait(1, Etape::WarningTick)
        },
        WarningTick = WARNING_TICK => |x| {
            if countdown_tick(x) {
                return x.wait(1, Etape::WarningTick);
            }
            crate::video::effacer_couche(x.b, Couche::Un);
            print_region_text(x, NOTICE_TABLE);
            wr16(x.b, COUNTDOWN, NOTICE_FRAMES);
            x.wait(1, Etape::NoticeTick)
        },
        NoticeTick = NOTICE_TICK => |x| {
            if countdown_tick(x) {
                return x.wait(1, Etape::NoticeTick);
            }
            crate::video::effacer_couche(x.b, Couche::Un);
            wr16(x.b, A5 + 0x7fc6, 0);
            if rd8(x.b, A5 + 4) & 1 != 0 {
                wr16(x.b, A5 + 0x7fc6, 1);
            }
            crate::eeprom::initialiser_compta(x.b);
            if !crate::config::initialiser(x.c, x.b) {
                return Flow::Yield;
            }
            crate::kernel::creer_tache(x.b, SLOT_CREDITS, tache::CREDITS);
            crate::kernel::creer_tache(x.b, SLOT_FREEZE, tache::GEL);
            // la tâche continue dans la boucle de la machine à modes (`jmp 0x133fe`)
            crate::modes::start(x)
        },
    }
}

/// État de la machine à modes (16 longs en `A5-0x7190`) et `A5-0x54f0..` (4 longs) à zéro (`0x133dc`, puis
/// `0xfc562`).
fn clear_mode_state(x: &mut Ctx) {
    const MODES: u32 = A5 - 0x7190;
    const SCENE_SCRIPT: u32 = A5 - 0x54f0;
    for i in 0..16 {
        wr32(x.b, MODES + 4 * i, 0);
    }
    for i in 0..4 {
        wr32(x.b, SCENE_SCRIPT + 4 * i, 0);
    }
    x.c.d[0] = 0;
    x.c.d[6] |= 0xffff;
    x.c.a[6] = MODES + 0x40;
}

/// Tableau des meilleurs scores par défaut (`0x18082`) : dix entrées de 14 octets copiées de la ROM vers
/// FF0FD0 ; une autre table pour la version à palette alternative (`A5+0x70`).
fn default_high_scores(x: &mut Ctx) {
    const TABLES: [u32; 2] = [0x18f1e, 0x18faa];
    const SCORES: u32 = 0xff0fd0;
    const SIZE: u32 = 10 * 14;
    let src = TABLES[(rd16(x.b, A5 + 0x70) != 0) as usize];
    for i in (0..SIZE).step_by(2) {
        let w = rd16(x.b, src + i);
        wr16(x.b, SCORES + i, w);
    }
    x.c.a[1] = src + SIZE;
    x.c.a[2] = SCORES + SIZE;
    x.c.d[6] |= 0xffff;
}

/// Affiche le texte de la région courante : mot `A5+0x70` (modulo 16) indexe la table de pointeurs.
fn print_region_text(x: &mut Ctx, table: u32) {
    let region = (rd16(x.b, A5 + 0x70) & 0xf) as u32;
    let texte = rd32(x.b, table + 2 * region);
    crate::text::print(x.b, texte);
}

/// Un tour du décompte : un bouton pressé le raccourcit de neuf images.
fn countdown_tick(x: &mut Ctx) -> bool {
    let mut left = rd16(x.b, COUNTDOWN) as i16;
    if rd8(x.b, PORT_IN1) & 0x10 == 0 {
        left = left.wrapping_sub(8);
    }
    left = left.wrapping_sub(1);
    wr16(x.b, COUNTDOWN, left as u16);
    left > 0
}

const WARNING_TICK: u32 = 0x11c06;

const NOTICE_TICK: u32 = 0x11c4a;

pub const NATIVES: &[Native] = &[];
