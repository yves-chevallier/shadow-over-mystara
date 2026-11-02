//! Écran de fin de partie (sous-mode 20, `0xfc576`) : le classement défile, puis les scores des
//! joueurs s'affichent. La machine à modes y arrive après la phase 20 (`0x1c80c`), avec le drapeau de
//! démarrage levé (`0x133f6`) ; elle le baisse en partant (`0x133f0`).
//!
//! L'écran a son propre bloc d'état, à `FF2B10` :
//!
//! | Champ | Rôle |
//! |---|---|
//! | `+0` | étape (pas de 2) : 0 préparation, 2 défilement du classement, 4 scores, 6 et 8 décor, 10 sortie |
//! | `+4` | minuterie (mot) |
//! | `+6` | pointeur sur le texte du classement (long, dans la ROM) |
//! | `+10` | ordonnée de la caméra à la dernière ligne ajoutée (mot) |
//!
//! Le défilement vertical est le long `FF2B1C` (16.16) ; un bouton pressé le ralentit.

use super::{Flow, Sequence};
use crate::families::effects_creators::{end_screen, or_rest};
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::objects::pools;
use crate::scene::Ctx;
use crate::video::{clear_layer, Layer};
use crate::Native;

const STATE: u32 = 0xff_2b10;
mod f {
    pub const STEP: u32 = 0;
    pub const TIMER: u32 = 4;
    pub const TEXT: u32 = 6;
    pub const MARK: u32 = 0xa;
}
/// Défilement du fond (long, 16.16) et du second plan.
const SCROLL: u32 = 0xff_2b1c;
const SCROLL_2: u32 = 0xff_2b20;
/// Table des quatre joueurs (16 octets chacun : score et état).
const SCORES: u32 = A5 - 0x5b18;
const FRAME_COUNT: u32 = A5 + 0x7e68;
const VIDEO: u32 = A5 + 0x14;
const CPS_REG_4: u32 = 0x400004;

/// Musique de l'écran.
const SOUND_E172C: u32 = 0xe172c;

/// Retour de l'étape à `0xfc584` : A0 revient sur le bloc de la machine à modes.
const RETURN: u32 = 0xfc584;

/// Écran du sous-mode `sub`, s'il est réécrit ici.
pub fn screen(x: &mut Ctx, sub: u8) -> Option<Flow> {
    (sub == 0x14).then(|| post_game(x))
}

fn post_game(x: &mut Ctx) -> Flow {
    x.c.a[0] = STATE;
    let step = rd8(x.b, STATE + f::STEP);
    x.push_return(Step::Return);
    match step {
        0 => s0_prepare(x),
        2 => s2_scroll(x),
        4 => s4_scores(x),
        6 => s6_scenery(x),
        8 => s8_scenery(x),
        10 => sa_leave(x),
        _ => Flow::Done,
    }
}

crate::steps! {
    /// Étapes (points de reprise) de la machine à états.
    pub enum Step {
        Return = RETURN => |x| {
            x.c.a[0] = super::mode::BASE;
            Flow::Done
        },
        S0Video = S0_VIDEO => |x| {
            wr16(x.b, CPS_REG_4, 0x1355);
            wr16(x.b, VIDEO, 0x24c0);
            x.wait(1, Step::S0Palettes)
        },
        S0Palettes = S0_PALETTES => |x| {
            clear_layers(x);
            crate::palette::load_scene_page(x.b, 1);
            crate::palette::load_palette_page(x.b, 2, 83);
            crate::palette::load_palette_page(x.b, 2, 83);
            wr8(x.b, A5 + 0x34, 0);
            crate::palette::load_scene_palettes_of(x.b, 89);
            // quatre blocs de palette, de la ROM vers la table des palettes
            for (dst, src) in [(0x91c3e0u32, 0x291f00u32), (0x91c2e0, 0xfc6d8), (0x91c000, 0x292260), (0x91c3c0, 0x2537c0)] {
                for i in 0..8 {
                    let w = rd32(x.b, src + 4 * i);
                    wr32(x.b, dst + 4 * i, w);
                }
            }
            wr8(x.b, A5 + 0x48e, 0xf);
            crate::palette::full_brightness(x.b);
            for off in [0x506u32, 0x50a, 0x556, 0x4b6, 0x4ba] {
                wr16(x.b, A5 + off, 0);
            }
            wr16(x.b, A5 + 0x55a, 0x701);
            wr32(x.b, SCROLL, 0x0701_0000);
            wr8(x.b, A5 + 0x36, 1);
            x.wait(4, Step::S0Tiles)
        },
        S0Tiles = S0_TILES => |x| {
            let end = crate::video::load_grid(x.b, crate::video::Grid::Layer2, 197, 0x917000);
            crate::video::load_grid(x.b, crate::video::Grid::Layer2, 198, end.destination);
            wr16(x.b, VIDEO, 0x24ce);
            wr16(x.b, STATE + f::MARK, 0x728);
            wr32(x.b, STATE + f::TEXT, 0xfc938);
            Flow::Done
        },
        SaFaded = SA_FADED => |x| {
            crate::sound::service(x.b, crate::sound::Service::GeneralStop);
            reset_objects(x);
            x.wait(1, Step::SaVideo)
        },
        SaVideo = SA_VIDEO => |x| {
            wr16(x.b, VIDEO, 0x12c0);
            x.wait(4, Step::SaCleared)
        },
        SaCleared = SA_CLEARED => |x| {
            clear_layers(x);
            wr8(x.b, super::mode::SUB, 0);
            wr16(x.b, super::mode::STEP, 0);
            wr8(x.b, A5 + 0x36, 0);
            for off in [0x4bau32, 0x50a, 0x55a] {
                wr16(x.b, A5 + off, 0);
            }
            crate::families::small::startup_done(x.b, false);
            crate::families::items::clear_picked_up(x.b);
            wr8(x.b, A5 + 0x30, 0);
            Flow::Done
        },
    }
}

fn next_step(x: &mut Ctx) {
    let s = rd8(x.b, STATE).wrapping_add(2);
    wr8(x.b, STATE, s);
}

/// Les trois couches effacées.
fn clear_layers(x: &mut Ctx) {
    for layer in [Layer::One, Layer::Two, Layer::Three] {
        clear_layer(x.b, layer);
    }
}

/// Listes défaites et vidées, objets effacés, pools remis à zéro (0x1f184, 0x1de7c, 0xc58, 0x2062).
fn reset_objects(x: &mut Ctx) {
    crate::objects::lists::unlink_all(x.b);
    crate::sprites::clear_lists(x.b);
    crate::video::clear_objects(x.b);
    for pool in pools::POOLS {
        pool.reset(x.b);
    }
}

/// Un rendu : PEFF, EFFCT, sprites (`0xfc91c`).
fn render(x: &mut Ctx) {
    for class in [&crate::dispatch::PEFF, &crate::dispatch::EFFCT] {
        class.update_no_next(x.c, x.b);
    }
    crate::sprites::build_68k(x.c, x.b);
    x.c.a[0] = STATE;
}

/// Vitesse du défilement et boutons pressés (D0 et D1.b de l'original).
struct Speed {
    speed: u32,
    top_buttons: u8,
}

/// `0xfc6f8` : vitesse du défilement (1 pixel par image, 1/4 si un bouton est pressé).
fn speed(x: &mut Ctx) -> Speed {
    let mut buttons = 0;
    for k in 0..4 {
        buttons |= rd8(x.b, A5 + k);
    }
    let top_buttons = buttons & 0xf0;
    Speed { speed: if top_buttons == 0 { 0x10000 } else { 0x4000 }, top_buttons }
}

/// Fait avancer le défilement du fond d'une image ; la caméra 2 le suit.
fn scroll(x: &mut Ctx) -> Speed {
    let v = speed(x);
    let s = rd32(x.b, SCROLL).wrapping_sub(v.speed);
    wr32(x.b, SCROLL, s);
    wr16(x.b, A5 + 0x55a, (s >> 16) as u16);
    v
}

fn s0_prepare(x: &mut Ctx) -> Flow {
    next_step(x);
    wr16(x.b, STATE + f::TIMER, 300);
    crate::sound::play_global(x.b, SOUND_E172C);
    reset_objects(x);
    x.c.a[0] = STATE;
    x.wait(1, Step::S0Video)
}
const S0_VIDEO: u32 = 0xfc5cc;
const S0_PALETTES: u32 = 0xfc5e2;
const S0_TILES: u32 = 0xfc6a8;

/// Étape 2 : le classement défile ; toutes les 40 lignes de pixels, les lignes suivantes du texte
/// sont ajoutées. Quand le texte est fini (octet négatif), étape suivante.
fn s2_scroll(x: &mut Ctx) -> Flow {
    render(x);
    scroll(x);
    let d0 = rd16(x.b, STATE + f::MARK).wrapping_sub(rd16(x.b, A5 + 0x55a));
    if d0 as u8 != 40 {
        return Flow::Done;
    }
    let camera = rd16(x.b, A5 + 0x55a);
    wr16(x.b, STATE + f::MARK, camera);
    let mut a4 = rd32(x.b, STATE + f::TEXT);
    for counter in (0..6u8).rev() {
        if rd8(x.b, a4) & 0x80 != 0 {
            next_step(x);
            return Flow::Done;
        }
        // 0x517ba : une ligne du classement (variante = compteur), son réglage est l'octet suivant du texte, qui
        // n'avance que si l'effet est créé
        let setting = rd8(x.b, a4);
        if crate::effects_aids::effect_44(x.b, counter, setting).is_some() {
            a4 += 1;
        }
    }
    if rd8(x.b, a4) & 0x80 != 0 {
        next_step(x);
        return Flow::Done;
    }
    wr32(x.b, STATE + f::TEXT, a4);
    Flow::Done
}

/// Étape 4 : les cartouches des scores (un objet par joueur ayant joué), après dix images de jeu.
fn s4_scores(x: &mut Ctx) -> Flow {
    if (rd16(x.b, FRAME_COUNT) as i16) < 10 {
        return Flow::Done;
    }
    render(x);
    let v = scroll(x);
    next_step(x);
    // les deux premiers cartouches reçoivent pour réglage le D1.b de la vitesse (les boutons pressés)
    for kind in [4u8, 6] {
        end_screen(x.b, kind, v.top_buttons);
    }
    // Glitch d'origine : la retenue n'est pas testée, un échec écrit dans l'A1 laissé par l'allocation
    let mut rank = 3u8;
    let mut a4 = SCORES;
    for counter in (0..4u8).rev() {
        if rd32(x.b, a4) != 0 || rd32(x.b, a4 + 4) != 0 {
            let o = end_screen(x.b, 5, counter);
            let o = or_rest(x.b, o);
            wr8(x.b, o.0 + 0x42, rank);
            rank = rank.wrapping_sub(1);
            wr32(x.b, o.0 + 0x4c, a4);
        }
        a4 += 0x10;
    }
    let mut rank = 3u8;
    a4 = SCORES;
    for counter in (0..4u8).rev() {
        if rd8(x.b, a4) != 0 {
            if rd8(x.b, a4 + 14) != 0 {
                let o = end_screen(x.b, 9, counter);
                let o = or_rest(x.b, o);
                wr8(x.b, o.0 + 0x42, rank);
                wr32(x.b, o.0 + 0x4c, a4);
            }
            rank = rank.wrapping_sub(1);
        }
        a4 += 0x10;
    }
    Flow::Done
}

/// Étape 6 : le fond descend jusqu'à l'ordonnée 1064, puis un second plan se met à défiler.
fn s6_scenery(x: &mut Ctx) -> Flow {
    render(x);
    scroll(x);
    if rd16(x.b, A5 + 0x55a) != 1064 {
        return Flow::Done;
    }
    next_step(x);
    wr32(x.b, SCROLL_2, 0x0108_0000);
    wr16(x.b, A5 + 0x4ba, 264);
    Flow::Done
}

/// Étape 8 : les deux plans défilent ensemble jusqu'à l'ordonnée 832.
fn s8_scenery(x: &mut Ctx) -> Flow {
    render(x);
    let speed = scroll(x).speed;
    let v = rd32(x.b, SCROLL_2).wrapping_sub(speed);
    wr32(x.b, SCROLL_2, v);
    wr16(x.b, A5 + 0x4ba, (v >> 16) as u16);
    if rd16(x.b, A5 + 0x55a) != 832 {
        return Flow::Done;
    }
    next_step(x);
    wr16(x.b, STATE + f::TIMER, 300);
    Flow::Done
}

/// Étape 10 : après 300 images (ou un bouton), fondu au noir et retour à l'attract.
fn sa_leave(x: &mut Ctx) -> Flow {
    let mut buttons = 0;
    for k in 0..4 {
        buttons |= rd8(x.b, A5 + 0xc + k);
    }
    if buttons & 0xf0 != 0 {
        wr16(x.b, STATE + f::TIMER, 1);
    }
    let left = rd16(x.b, STATE + f::TIMER).wrapping_sub(1);
    wr16(x.b, STATE + f::TIMER, left);
    if left != 0 {
        render(x);
        return Flow::Done;
    }
    crate::sound::service(x.b, crate::sound::Service::Ff07 { lock: 0, line: 4 });
    wr8(x.b, A5 + 0x48e, 0xf);
    wr16(x.b, A5 + 0x490, 3);
    x.subroutine(crate::tasks::fading::descent, Step::SaFaded)
}
const SA_FADED: u32 = 0xfc8a8;
const SA_VIDEO: u32 = 0xfc8ce;
const SA_CLEARED: u32 = 0xfc8dc;

pub const NATIVES: &[Native] = &[];
