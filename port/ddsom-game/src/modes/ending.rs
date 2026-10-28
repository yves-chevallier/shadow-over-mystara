//! Phase 28 (`0xfb292`) : la fin du jeu (générique).
//!
//! Après la dernière scène (séquence de sortie 35, `scene::exit`), la partie passe en phase 28 : le boss est
//! vaincu, les joueurs reçoivent leurs récompenses, des dialogues et des écrans fixes se succèdent, puis la
//! phase 20 (fin de partie) rend la main à la machine à modes. Les états (`A5+0x2d`, pas de 2) sont rangés
//! par ordre de passage ; l'état 0 *tombe* dans l'état 2 (le compteur avance de 4).
//!
//! Le bloc d'état est à `FF2B00` : `+0` minuterie (mot), `+2` lien vers l'objet de dialogue en cours
//! (mot), `+8` octet de signal lu et écrit par les objets (par le pointeur `+0x4c` de leur enregistrement).
//! Chaque état revient en `0xfb2a4` (la native de reprise rétablit A0).

use super::{Flow, Sequence};
use crate::scene::fondu;
use crate::effects_aids::decor_6;
use crate::families::creators::set_15_pur;
use crate::families::effects_creators::ecran_de_fin;
use crate::palette::{charger_ensemble, charger_page};
use crate::video::{charger_grille, effacer_couche, Couche, Grille};
use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8, A5};
use crate::scene::Ctx;
use crate::Native;
use crate::objects::{pools, Obj};

const BLOCK: u32 = 0xff_2b00;
mod f {
    pub const TIMER: u32 = 0;
    pub const LINK: u32 = 2;
    pub const SIGNAL: u32 = 8;
}

mod var {
    use crate::mem::A5;
    pub const STEP: u32 = A5 + 0x2d;
    pub const PHASE: u32 = A5 + 0x2c;
    pub const VIDEO: u32 = A5 + 0x14;
    pub const SCROLL_1: u32 = A5 + 0x4b6;
    pub const SCROLL_2: u32 = A5 + 0x4ba;
    pub const CAMERA: u32 = A5 + 0x506;
    pub const CAMERA_Y: u32 = A5 + 0x50a;
    pub const CAMERA_MODE: u32 = A5 + 0x51e;
    pub const SCREEN_ID: u32 = A5 + 0x36;
    pub const CAMERA_2: u32 = A5 + 0x556;
    pub const CAMERA_2_Y: u32 = A5 + 0x55a;
    pub const CAMERA_2_COPY: u32 = A5 + 0x55e;
    pub const CAMERA_2_COPY_Y: u32 = A5 + 0x562;
    pub const CAMERA_2_MODE: u32 = A5 + 0x56e;
    pub const PLAYERS: u32 = A5 + 0x5de;
    pub const CPS_REG_4: u32 = 0x400004;
    pub const COUNTER: u32 = A5 - 0x5b8e;
}

mod ext {
    pub const SOUND_E1528: u32 = 0xe1528;
    pub const SOUND_E16F0: u32 = 0xe16f0;
}

mod at {
    pub const RETURN: u32 = 0xfb2a4;
}

const FADE_ALL: u8 = 0xf;

pub const PHASE_28: Native = Native {
    entry: 0xfb292,
    name: "ending_phase_28",
    run: crate::Corps::Processeur(|c, b| {
        let mut x = Ctx::new(c, b);
        if phase_28(&mut x) == Flow::Done {
            rts(x.c, x.b);
        }
    }),
    touches_sound: false,
    outputs: 0,
};
/// Phase 28 de la partie (`0xfb292`) : le générique de fin. L'état revient à [`Etape::Return`].
pub fn phase_28(x: &mut Ctx) -> Flow {
    x.c.a[0] = BLOCK;
    let step = rd8(x.b, var::STEP);
    x.push_return(Etape::Return);
    state(x, step)
}

crate::etapes! {
    /// Étapes (points de reprise) de la machine à états.
    pub enum Etape {
        Return = at::RETURN => |x| {
            x.c.a[0] = BLOCK;
            Flow::Done
        },
        S00Video = S00_VIDEO => |x| {
            wr16(x.b, var::CPS_REG_4, 0x1355);
            for v in [var::SCROLL_1, var::SCROLL_2, var::CAMERA, var::CAMERA_Y] {
                wr32(x.b, v, 0);
            }
            wr16(x.b, var::CAMERA_MODE, 4);
            wr8(x.b, A5 + 0x53c, 0);
            wr32(x.b, var::CAMERA_2, 0);
            wr32(x.b, var::CAMERA_2_Y, 0);
            wr16(x.b, var::CAMERA_2_MODE, 4);
            wr16(x.b, var::VIDEO, 0x12c0);
            let v = rd8(x.b, A5 + 0x13) & !1;
            wr8(x.b, A5 + 0x13, v);
            x.wait(4, Etape::S00Palette)
        },
        S00Palette = S00_PALETTE => |x| {
            crate::palette::charger_page_de_la_scene(x.b, 1);
            charger_page(x.b, 2, 89);
            charger_page(x.b, 3, 86);
            fondu::PAGES.set(x.b, 12);
            crate::palette::full_brightness(x.b);
            s02_clear(x)
        },
        S02Cleared = S02_CLEARED => |x| {
            clear_layers(x, &[Couche::Deux, Couche::Trois]);
            Flow::Done
        },
        S04Fade = S04_FADE => |x| {
            crate::sound::jouer_global(x.b, ext::SOUND_E1528);
            fade_settings(x, 3, 4);
            x.sous_programme(crate::taches::fondu::montee, Etape::S04Timer)
        },
        S04Timer = S04_TIMER => |x| {
            wr16(x.b, BLOCK + f::TIMER, 60);
            Flow::Done
        },
        S06Retry = S06_RETRY => s06_object,
        S08Fade = S08_FADE => |x| {
            next_step(x);
            fade_settings(x, 3, 4);
            x.sous_programme(crate::taches::fondu::descente, Etape::S08Faded)
        },
        S08Faded = S08_FADED => |x| {
            let v = rd16(x.b, var::VIDEO) & !0xe;
            wr16(x.b, var::VIDEO, v);
            x.wait(4, Etape::S08Picture)
        },
        S08Picture = S08_PICTURE => |x| {
            charger_page(x.b, 3, 87);
            charger_ensemble(x.b, 6);
            tile_blocks(x, 0x91b800, 350, 351, Grille::Couche3);
            tile_blocks(x, 0x917000, 548, 549, Grille::Couche2);
            wr8(x.b, BLOCK + f::SIGNAL, 0);
            s08_object(x)
        },
        S08Retry = S08_RETRY => s08_object,
        S0aRetry = S0A_RETRY => |x| open_message(x, 89, Etape::S0aRetry),
        S0cObject = S0C_OBJECT => s0c_object,
        S0cRetry = S0C_RETRY => s0c_object,
        S0eLoop = S0E_LOOP => |x| {
            render(x);
            s0e_wait_fade(x)
        },
        S0eMusic = S0E_MUSIC => |x| {
            render(x);
            s0e_wait_music(x)
        },
        S0eObjects = S0E_OBJECTS => |x| {
            wr8(x.b, var::SCREEN_ID, 1);
            // Glitch d'origine : la retenue n'est pas testée ; un échec écrit dans l'A1 laissé par l'allocation
            let _ = decor_6(x.b, 11);
            let o = decor_6(x.b, 12).unwrap_or_else(|a1| a1);
            wr8(x.b, o.0 + 0x2f, 0);
            let o = decor_6(x.b, 12).unwrap_or_else(|a1| a1);
            wr8(x.b, o.0 + 0x2f, 1);
            let _ = set_15_pur(x.b, 14);
            wr8(x.b, BLOCK + f::SIGNAL, 0xff);
            render(x);
            wr8(x.b, BLOCK + f::SIGNAL, 1);
            wr16(x.b, BLOCK + f::TIMER, 180);
            fade_settings(x, 3, FADE_ALL);
            // `jmp 0x9b6` : la tâche de fondu, puis le retour
            crate::kernel::creer_tache(x.b, 0x10, crate::scene::fondu::MONTEE);
            Flow::Done
        },
        S10Retry = S10_RETRY => |x| {
            render(x);
            open_message(x, 90, Etape::S10Retry)
        },
        S14Retry = S14_RETRY => |x| {
            render(x);
            open_message(x, 91, Etape::S14Retry)
        },
        S18Retry = S18_RETRY => s18_object,
        S1aLoop = S1A_LOOP => |x| {
            render(x);
            s1a_wait_fade(x)
        },
        S1cScroll = S1C_SCROLL => |x| {
            if !timer_tick(x) {
                return s1c_scroll(x);
            }
            wr16(x.b, var::SCROLL_2, 0);
            x.wait(4, Etape::S1cPalette)
        },
        S1cPalette = S1C_PALETTE => |x| {
            crate::palette::charger_palettes_de_la_scene(x.b);
            // sauvegarde les 1024 octets de palette en 0x910000, puis les remplit de blanc
            for i in 0..256u32 {
                let v = rd32(x.b, 0x91c000 + 4 * i);
                wr32(x.b, 0x910000 + 4 * i, v);
            }
            for i in 0..256u32 {
                wr32(x.b, 0x91c000 + 4 * i, 0xffff_ffff);
            }
            fondu::DELAI.set(x.b, 1);
            fondu::PAGES.set(x.b, FADE_ALL);
            wr32(x.b, A5 + 0x492, 0x910000);
            wr32(x.b, A5 + 0x496, 0x25c0c0);
            wr32(x.b, A5 + 0x49a, 0x269120);
            wr32(x.b, A5 + 0x49e, 0x27e600);
            crate::kernel::creer_tache(x.b, 0x10, crate::scene::fondu::DEPUIS_LE_BLANC);
            crate::sound::jouer_global(x.b, ext::SOUND_E16F0);
            fondu::PAS.set(x.b, 1);
            x.c.a[0] = BLOCK;
            s1c_wait_fade(x)
        },
        S1cFadeLoop = S1C_FADE_LOOP => |x| {
            render_scroll(x);
            if fondu::PAS.get(x.b) != 0 {
                return s1c_wait_fade(x);
            }
            wr16(x.b, BLOCK + f::TIMER, 30);
            Flow::Done
        },
        S1eLoop = S1E_LOOP => |x| {
            render_scroll(x);
            if fondu::PAS.get(x.b) != 0 {
                return s1e_loop(x);
            }
            next_step(x);
            s1e_object(x)
        },
        S1eRetry = S1E_RETRY => s1e_object,
        S22Loop = S22_LOOP => |x| {
            if fondu::PAS.get(x.b) != 0 {
                return s22_wait(x);
            }
            next_step(x);
            wr16(x.b, BLOCK + f::TIMER, 60);
            wr8(x.b, BLOCK + f::SIGNAL, 0);
            render_scroll(x);
            let Ok(set) = set_15_pur(x.b, 20) else {
                return Flow::Done;
            };
            wr16(x.b, BLOCK + f::LINK, set.0 as u16);
            let n = rd8(x.b, var::COUNTER).wrapping_add(1);
            wr8(x.b, var::COUNTER, n);
            Flow::Done
        },
        S28Loop = S28_LOOP => |x| {
            x.c.a[0] = BLOCK;
            render_scroll(x);
            if fondu::PAS.get(x.b) != 0 {
                return s28_loop(x);
            }
            Flow::Done
        },
        S2aPlanes = S2A_PLANES => |x| {
            render_scroll(x);
            let v = rd16(x.b, var::VIDEO) & !0xc;
            wr16(x.b, var::VIDEO, v);
            x.wait(1, Etape::S2aScrolls)
        },
        S2aScrolls = S2A_SCROLLS => |x| {
            clear_layers(x, &[Couche::Deux, Couche::Trois]);
            wr8(x.b, var::CAMERA_MODE, 4);
            for v in [var::CAMERA, var::CAMERA_Y, var::CAMERA_2, var::CAMERA_2_Y] {
                wr16(x.b, v, 0);
            }
            next_step(x);
            wr16(x.b, BLOCK + f::TIMER, 4);
            charger_page(x.b, 3, 87);
            charger_ensemble(x.b, 6);
            s2a_wait(x)
        },
        S2aWait = S2A_WAIT => |x| {
            if !timer_tick(x) {
                return s2a_wait(x);
            }
            wr16(x.b, BLOCK + f::TIMER, 600);
            tile_blocks(x, 0x91b800, 354, 355, Grille::Couche3);
            wr8(x.b, BLOCK + f::SIGNAL, 0);
            s2a_object(x)
        },
        S2aRetry = S2A_RETRY => s2a_object,
        S2aShow = S2A_SHOW => |x| {
            wr16(x.b, var::VIDEO, 0x12ce);
            render(x);
            fade_settings(x, 3, 13);
            crate::kernel::creer_tache(x.b, 0x10, crate::scene::fondu::MONTEE);
            Flow::Done
        },
        S2cLoop = S2C_LOOP => |x| {
            if fondu::PAS.get(x.b) != 0 {
                return s2c_loop(x);
            }
            wr8(x.b, BLOCK + f::SIGNAL, 0xff);
            unlink_and_clear_lists(x);
            crate::objects::pools::EFFCT.reset(x.b);
            crate::objects::pools::SOBJ.reset(x.b);
            crate::video::vider_objets(x.b);
            x.wait(1, Etape::S2cPlanes)
        },
        S2cPlanes = S2C_PLANES => |x| {
            let v = rd16(x.b, var::VIDEO) & !0xc;
            wr16(x.b, var::VIDEO, v);
            x.wait(1, Etape::S2cCleared)
        },
        S2cCleared = S2C_CLEARED => |x| {
            clear_layers(x, &[Couche::Trois, Couche::Un]);
            wr16(x.b, BLOCK + f::TIMER, 4);
            let o = link(x);
            wr32(x.b, o + 4, 0x0600_0000);
            let n = rd8(x.b, var::COUNTER).wrapping_sub(1);
            wr8(x.b, var::COUNTER, n);
            wr8(x.b, var::SCREEN_ID, 0);
            wr8(x.b, A5 + 0x53c, 0);
            let v = rd8(x.b, A5 + 0x13) & !1;
            wr8(x.b, A5 + 0x13, v);
            for v in [var::CAMERA, var::CAMERA_Y, var::SCROLL_1, var::SCROLL_2] {
                wr16(x.b, v, 0);
            }
            s2c_wait_timer(x)
        },
        S2cTimer = S2C_TIMER => |x| {
            if !timer_tick(x) {
                return s2c_wait_timer(x);
            }
            charger_page(x.b, 2, 84);
            charger_page(x.b, 3, 84);
            fondu::PAGES.set(x.b, 12);
            crate::palette::no_brightness(x.b);
            crate::sound::service(x.b, crate::sound::Service::Ff07 { verrou: 0, ligne: 4 });
            // joueurs présents et joueurs « prêts à partir » (octet +0x192)
            let mut present = 0u8;
            let mut ready = 0u8;
            let count = rd8(x.b, A5 + 0x20) as u32 + 1;
            let mut fp = var::PLAYERS;
            for _ in 0..=count {
                if rd8(x.b, fp) != 0 && (rd8(x.b, fp + 4) as i8) < 4 {
                    present += 1;
                    if rd8(x.b, fp + 0x192) != 0 {
                        ready += 1;
                    }
                }
                fp += 0x200;
            }
            if ready != 0 || present.wrapping_sub(2) == rd8(x.b, A5 + 0x20) {
                if let Ok(set) = set_15_pur(x.b, 22) {
                    wr16(x.b, BLOCK + f::LINK, set.0 as u16);
                    return render_flow(x);
                }
            }
            next_step(x);
            render_flow(x)
        },
        S2eOpened = S2E_OPENED => |x| {
            let o = link(x);
            let v = rd8(x.b, o + 5).wrapping_add(2);
            wr8(x.b, o + 5, v);
            render_flow(x)
        },
        S2eClosed = S2E_CLOSED => |x| {
            let o = link(x);
            let v = rd8(x.b, o + 5).wrapping_add(2);
            wr8(x.b, o + 5, v);
            render_flow(x)
        },
        S30Wait = S30_WAIT => |x| {
            if (rd16(x.b, A5 + 0x7e68) as i16) < 3 {
                return Flow::Done;
            }
            next_step(x);
            let mut d4 = 13u8;
            for counter in (0..3u32).rev() {
                wr8(x.b, BLOCK + f::SIGNAL, 0);
                // Glitch d'origine : la retenue n'est pas testée ; un échec écrit dans l'A1 laissé par l'allocation
                let Obj(o) = decor_6(x.b, 0).unwrap_or_else(|a1| a1);
                wr32(x.b, o + 0x4c, BLOCK);
                let v = rd16(x.b, var::CAMERA);
                wr16(x.b, o + 8, v);
                let v = rd16(x.b, var::CAMERA_Y);
                wr16(x.b, o + 0xc, v);
                wr32(x.b, o + 0x10, 0);
                wr8(x.b, o + 0x2f, d4);
                d4 = d4.wrapping_add(1);
                let idx = counter * 4;
                let dx = x.prog16(0xfc3e4 + idx);
                let dy = x.prog16(0xfc3e6 + idx);
                let ox = rd16(x.b, o + 8).wrapping_add(dx);
                wr16(x.b, o + 8, ox);
                let oy = rd16(x.b, o + 0xc).wrapping_add(dy);
                wr16(x.b, o + 0xc, oy);
            }
            render(x);
            wr16(x.b, var::VIDEO, 0x12ce);
            wr16(x.b, BLOCK + f::TIMER, 240);
            fade_settings(x, 3, FADE_ALL);
            crate::kernel::creer_tache(x.b, 0x10, crate::scene::fondu::MONTEE);
            Flow::Done
        },
        S32Faded = S32_FADED => |x| {
            unlink_and_clear_lists(x);
            reset_pools(x);
            crate::video::vider_objets(x.b);
            x.wait(1, Etape::S32Cleared)
        },
        S32Cleared = S32_CLEARED => |x| {
            clear_layers(x, &[Couche::Un, Couche::Deux, Couche::Trois]);
            // le score (long BCD à +0x110) reçoit la récompense (+0x114, sans son quartet faible), plafonné à 9 999 999
            for k in 0..4u32 {
                let p = var::PLAYERS + 0x200 * k;
                if rd8(x.b, p) == 0 {
                    continue;
                }
                wr32(x.b, p + 4, 0x0600_0000);
                let reward = rd32(x.b, p + 0x114) & 0xffff_fff0;
                wr32(x.b, p + 0x114, reward);
                let score = bcd_add(rd32(x.b, p + 0x110), reward);
                wr32(x.b, p + 0x110, if score as i32 > 0x0999_9999 { 0x0999_9999 } else { score });
            }
            crate::player::update::mettre_a_jour(x.c, x.b);
            wr8(x.b, var::PHASE, 20);
            crate::sound::service(x.b, crate::sound::Service::ArretGeneral);
            // `jmp 0x133f6` : le démarrage noté fait, puis le retour à l'appelant
            crate::families::petites::demarrage_fait(x.b, true);
            Flow::Done
        },
    }
}

fn state(x: &mut Ctx, step: u8) -> Flow {
    match step {
        0 => s00_reset(x),
        2 => s02_clear(x),
        4 => s04_picture(x),
        6 => s06_wait(x),
        8 => s08_wait_dialogue(x),
        10 => s0a_wait_signal(x),
        12 => s0c_wait_dialogue(x),
        14 => s0e_wait_timer(x),
        16 => s10_wait_camera(x),
        18 | 22 => s12_wait_dialogue(x),
        20 => s14_wait_timer(x),
        24 => s18_object(x),
        26 => s1a_wait_signal(x),
        28 => s1c_wait(x),
        30 => s1e_fade(x),
        32 => render_scroll(x),
        34 => s22_object(x),
        36 => render_scroll(x),
        38 => s26_wait(x),
        40 => s28_fade(x),
        42 => s2a_clear(x),
        44 => s2c_wait(x),
        46 => s2e_dialogue(x),
        48 => s30_rewards(x),
        50 => s32_end(x),
        _ => Flow::Done,
    }
}

// ---------------------------------------------------------------------------------------------
// Aides

fn next_step(x: &mut Ctx) {
    let s = rd8(x.b, var::STEP).wrapping_add(2);
    wr8(x.b, var::STEP, s);
}

/// `0xfc4ea` : décor (SET, SOBJ) et sprites ; A0 est conservé.
fn render(x: &mut Ctx) {
    let a0 = x.c.a[0];
    crate::dispatch::SET.update_sans_suite(x.c, x.b);
    crate::dispatch::SOBJ.update_sans_suite(x.c, x.b);
    crate::sprites::construire_68k(x.c, x.b);
    x.c.a[0] = a0;
}

/// `0xfc506` : comme `render`, avec les effets et les caméras (dont le D4 est relu par des gestionnaires : voir
/// `scroll::update_cameras_68k`).
fn render_scroll(x: &mut Ctx) -> Flow {
    let a0 = x.c.a[0];
    crate::dispatch::SET.update_sans_suite(x.c, x.b);
    crate::dispatch::EFFCT.update_sans_suite(x.c, x.b);
    crate::scroll::update_cameras_68k(x.c, x.b);
    crate::dispatch::SOBJ.update_sans_suite(x.c, x.b);
    crate::sprites::construire_68k(x.c, x.b);
    x.c.a[0] = a0;
    Flow::Done
}

/// Couches effacées (0xca8, 0xcc8, 0xce8).
fn clear_layers(x: &mut Ctx, couches: &[Couche]) {
    for &couche in couches {
        effacer_couche(x.b, couche);
    }
}

/// Listes triées défaites (0x1f184) et listes de sprites vidées (0x1de7c).
fn unlink_and_clear_lists(x: &mut Ctx) {
    crate::objects::lists::unlink_all(x.b);
    crate::sprites::vider_listes(x.b);
}

/// Les dix pools remis à zéro (0x2062).
fn reset_pools(x: &mut Ctx) {
    for pool in pools::POOLS {
        pool.reset(x.b);
    }
}

fn timer_tick(x: &mut Ctx) -> bool {
    let left = rd16(x.b, BLOCK + f::TIMER).wrapping_sub(1);
    wr16(x.b, BLOCK + f::TIMER, left);
    left == 0
}

fn link(x: &mut Ctx) -> u32 {
    rd16(x.b, BLOCK + f::LINK) as i16 as i32 as u32 & 0xff_ffff
}

fn fade_settings(x: &mut Ctx, delay: u16, pages: u8) {
    fondu::DELAI.set(x.b, delay);
    fondu::PAGES.set(x.b, pages);
}

/// Crée la tâche de fondu d'entrée `entry` (emplacement 0x10).
fn fade_task(x: &mut Ctx, entry: u32) {
    crate::kernel::creer_tache(x.b, 0x10, entry);
}

/// Deux blocs de tuiles de la grille `grille` à la suite l'un de l'autre à partir de `destination`.
fn tile_blocks(x: &mut Ctx, destination: u32, first: u16, second: u16, grille: Grille) {
    let fin = charger_grille(x.b, grille, first, destination);
    charger_grille(x.b, grille, second, fin.destination);
}

// ---------------------------------------------------------------------------------------------
// États

/// État 0 : vide tout, copie les récompenses des joueurs, puis tombe dans l'état 2.
fn s00_reset(x: &mut Ctx) -> Flow {
    next_step(x);
    crate::kernel::tuer_tache(x.b, 0x30);
    reset_pools(x);
    crate::video::ecran_noir(x.b);
    unlink_and_clear_lists(x);
    crate::video::vider_objets(x.b);
    for k in 0..4u32 {
        let p = var::PLAYERS + 0x200 * k;
        if rd8(x.b, p) != 0
            && (rd8(x.b, p + 4) as i8) < 4
            && rd32(x.b, p + 0x118) == 0xffff_ffff
            && rd32(x.b, p + 0x11c) == 0xffff_ffff
        {
            reward_table(x, p);
        }
    }
    x.wait(1, Etape::S00Video)
}

/// `0xfb798` : deux longs de la table des fins (par personnage et région) dans le joueur.
fn reward_table(x: &mut Ctx, p: u32) {
    let mut offset = (rd8(x.b, p + 3) as u32) << 3;
    if rd16(x.b, A5 + 0x70) != 0 {
        offset += 96;
    }
    for i in 0..2 {
        let v = rd32(x.b, 0xfb7bc + offset + 4 * i);
        wr32(x.b, p + 0x118 + 4 * i, v);
    }
}
const S00_VIDEO: u32 = 0xfb712;
const S00_PALETTE: u32 = 0xfb758;

/// État 2 (où tombe l'état 0) : l'écran est vidé.
fn s02_clear(x: &mut Ctx) -> Flow {
    next_step(x);
    wr16(x.b, var::VIDEO, 0x12c0);
    x.wait(1, Etape::S02Cleared)
}
const S02_CLEARED: u32 = 0xfb78c;

/// État 4 : image de fond, fondu d'ouverture.
fn s04_picture(x: &mut Ctx) -> Flow {
    next_step(x);
    tile_blocks(x, 0x917000, 0x222, 0x223, Grille::Couche2);
    wr16(x.b, var::VIDEO, 0x12ce);
    x.wait(1, Etape::S04Fade)
}
const S04_FADE: u32 = 0xfb8a8;
const S04_TIMER: u32 = 0xfb8c0;

/// État 6 : après la minuterie, crée l'objet de dialogue (SET 70).
fn s06_wait(x: &mut Ctx) -> Flow {
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    s06_object(x)
}
fn s06_object(x: &mut Ctx) -> Flow {
    let Some(Obj(o)) = pools::SET.alloc(x.b) else { return x.wait(1, Etape::S06Retry) };
    wr8(x.b, o, 1);
    wr8(x.b, o + 2, 0);
    wr8(x.b, o + 3, 70);
    wr8(x.b, o + 0x2f, 0);
    wr16(x.b, BLOCK + f::LINK, o as u16);
    Flow::Done
}
const S06_RETRY: u32 = 0xfb8e0;

/// État 8 : l'objet de dialogue arrive à l'état 6 : fondu au noir puis nouveau décor.
fn s08_wait_dialogue(x: &mut Ctx) -> Flow {
    render(x);
    let o = link(x);
    if rd8(x.b, o + 4) != 6 {
        return Flow::Done;
    }
    x.wait(60, Etape::S08Fade)
}
const S08_FADE: u32 = 0xfb916;
const S08_FADED: u32 = 0xfb92c;
const S08_PICTURE: u32 = 0xfb93a;
fn s08_object(x: &mut Ctx) -> Flow {
    let Ok(Obj(o)) = decor_6(x.b, 10) else {
        return x.wait(1, Etape::S08Retry);
    };
    wr32(x.b, o + 0x4c, BLOCK);
    render(x);
    let v = rd16(x.b, var::VIDEO) | 0xe;
    wr16(x.b, var::VIDEO, v);
    fade_settings(x, 3, FADE_ALL);
    crate::taches::fondu::montee(x)
}
/// Réessai de la création de l'objet : `bra 0xfb982` (la remise à zéro du signal n'est pas refaite).
const S08_RETRY: u32 = 0xfb996;

/// État 10 : attend le signal de l'objet, puis ouvre le message 89.
fn s0a_wait_signal(x: &mut Ctx) -> Flow {
    render(x);
    if rd8(x.b, BLOCK + f::SIGNAL) == 0 {
        return Flow::Done;
    }
    next_step(x);
    open_message(x, 89, Etape::S0aRetry)
}
/// Message `n` (0xc8312) : l'objet de dialogue est rangé dans le bloc ; `retry` : cession en cas d'échec.
fn open_message(x: &mut Ctx, n: u8, retry: Etape) -> Flow {
    let Some(message) = crate::families::creators::scene_set(x.b, n) else {
        return x.wait(1, retry);
    };
    wr16(x.b, BLOCK + f::LINK, message.0 as u16);
    Flow::Done
}
const S0A_RETRY: u32 = 0xfb9dc;

/// État 12 : le message est fini : 30 images, puis l'objet SET 13.
fn s0c_wait_dialogue(x: &mut Ctx) -> Flow {
    render(x);
    let o = link(x);
    if rd8(x.b, o + 0x22) == 0 {
        return Flow::Done;
    }
    next_step(x);
    x.wait(30, Etape::S0cObject)
}
const S0C_OBJECT: u32 = 0xfb9fa;
fn s0c_object(x: &mut Ctx) -> Flow {
    let Ok(Obj(set)) = set_15_pur(x.b, 13) else {
        return x.wait(1, Etape::S0cRetry);
    };
    wr32(x.b, set + 0x4c, BLOCK);
    wr16(x.b, BLOCK + f::TIMER, 180);
    Flow::Done
}
const S0C_RETRY: u32 = 0xfba14;

/// État 14 : après la minuterie, fondu au noir (tâche), image suivante, palette, objets des joueurs.
fn s0e_wait_timer(x: &mut Ctx) -> Flow {
    render(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    fade_settings(x, 3, FADE_ALL);
    fondu::PAS.set(x.b, 1);
    fade_task(x, crate::scene::fondu::DESCENTE);
    s0e_wait_fade(x)
}
fn s0e_wait_fade(x: &mut Ctx) -> Flow {
    if fondu::PAS.get(x.b) != 0 {
        return x.wait(1, Etape::S0eLoop);
    }
    tile_blocks(x, 0x91b000, 352, 353, Grille::Couche3);
    tile_blocks(x, 0x916000, 543, 0, Grille::Couche2);
    wr16(x.b, var::CAMERA_2_Y, 256);
    wr16(x.b, var::CAMERA_2, 0);
    wr16(x.b, var::CAMERA_Y, 256);
    wr16(x.b, var::CAMERA, 128);
    // palette : huit longs copiés puis assombris (ET 0x0fff0fff sur les huit suivants)
    let words: Vec<u32> = (0..8).map(|i| rd32(x.b, 0x291980 + 4 * i)).collect();
    for (i, w) in words.iter().enumerate() {
        wr32(x.b, 0x91c280 + 4 * i as u32, w & 0x0fff_0fff);
    }
    s0e_wait_music(x)
}
const S0E_LOOP: u32 = 0xfba68;
/// Attend que la musique ait avancé (`7e70` ≥ 3 ou `7e66` ≥ 1), une image à la fois.
fn s0e_wait_music(x: &mut Ctx) -> Flow {
    let ready = (rd16(x.b, A5 + 0x7e70) as i16) >= 3 && (rd16(x.b, A5 + 0x7e66) as i16) >= 1;
    x.wait(1, if ready { Etape::S0eObjects } else { Etape::S0eMusic })
}
const S0E_MUSIC: u32 = 0xfbb18;
const S0E_OBJECTS: u32 = 0xfbb26;

/// État 16 : attend que la caméra se soit arrêtée et la minuterie, puis ouvre le message 90.
fn s10_wait_camera(x: &mut Ctx) -> Flow {
    render(x);
    if rd16(x.b, var::CAMERA) != 0 || !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    open_message(x, 90, Etape::S10Retry)
}
const S10_RETRY: u32 = 0xfbbae;

/// États 18 et 22 : le message est fini : 30 images.
fn s12_wait_dialogue(x: &mut Ctx) -> Flow {
    render(x);
    let o = link(x);
    if rd8(x.b, o + 0x22) == 0 {
        return Flow::Done;
    }
    wr16(x.b, BLOCK + f::TIMER, 30);
    next_step(x);
    Flow::Done
}

/// État 20 : après la minuterie, ouvre le message 91.
fn s14_wait_timer(x: &mut Ctx) -> Flow {
    render(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    open_message(x, 91, Etape::S14Retry)
}
const S14_RETRY: u32 = 0xfbbf6;

/// État 24 : objet SET 13, caméras remises à zéro.
fn s18_object(x: &mut Ctx) -> Flow {
    let Ok(Obj(set)) = set_15_pur(x.b, 13) else {
        return x.wait(1, Etape::S18Retry);
    };
    wr32(x.b, set + 0x4c, BLOCK);
    for v in [var::CAMERA, var::CAMERA_Y, var::CAMERA_2, var::CAMERA_2_Y] {
        wr16(x.b, v, 0);
    }
    next_step(x);
    wr8(x.b, BLOCK + f::SIGNAL, 2);
    render(x);
    Flow::Done
}
const S18_RETRY: u32 = 0xfbc16;

/// État 26 : le signal passe à 1 : fondu (tâche), vide tout, prépare les joueurs.
fn s1a_wait_signal(x: &mut Ctx) -> Flow {
    render(x);
    if rd8(x.b, BLOCK + f::SIGNAL) != 1 {
        return Flow::Done;
    }
    next_step(x);
    crate::sound::service(x.b, crate::sound::Service::Ff07 { verrou: 0, ligne: 4 });
    fondu::PAS.set(x.b, 1);
    fondu::DELAI.set(x.b, 4);
    fondu::PAGES.set(x.b, FADE_ALL);
    fade_task(x, crate::scene::fondu::VERS_LE_BLANC);
    s1a_wait_fade(x)
}
fn s1a_wait_fade(x: &mut Ctx) -> Flow {
    if fondu::PAS.get(x.b) != 0 {
        return x.wait(1, Etape::S1aLoop);
    }
    wr16(x.b, BLOCK + f::TIMER, 120);
    wr8(x.b, BLOCK + f::SIGNAL, 0xff);
    render(x);
    unlink_and_clear_lists(x);
    reset_pools(x);
    crate::video::effacer_debut_couche1_68k(x.c, x.b);
    wr16(x.b, var::SCROLL_2, 256);
    // `jmp 0xc58`
    crate::video::vider_objets(x.b);
    Flow::Done
}
const S1A_LOOP: u32 = 0xfbc92;

/// État 28 : le générique : décor, objets, caméra, attente de l'ouverture.
fn s1c_wait(x: &mut Ctx) -> Flow {
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    wr16(x.b, var::VIDEO, 0x138e);
    tile_blocks(x, 0x91b900, 115, 116, Grille::Couche3);
    wr8(x.b, var::SCREEN_ID, 1);
    wr16(x.b, var::CAMERA_2, 384);
    wr16(x.b, var::CAMERA_2_Y, 0);
    let v = rd16(x.b, var::CAMERA_2);
    wr16(x.b, var::CAMERA_2_COPY, v);
    let v = rd16(x.b, var::CAMERA_2_Y);
    wr16(x.b, var::CAMERA_2_COPY_Y, v);
    wr16(x.b, var::CAMERA_2_MODE, 4);
    crate::families::petites::fond_2_allume(x.b);
    credits_objects_a(x);
    credits_objects_b(x);
    credits_objects_c(x);
    wr8(x.b, A5 + 0x34, 11);
    wr8(x.b, A5 + 0x54e, 2);
    wr16(x.b, BLOCK + f::TIMER, 3);
    s1c_scroll(x)
}
/// `0xfbdd2` : objets du décor d'après une table de la ROM (dix octets par objet, 0xffff termine).
fn credits_objects_a(x: &mut Ctx) {
    let mut a2 = 0xfbe92;
    while rd16(x.b, a2) != 0xffff {
        let Some(Obj(o)) = pools::SOBJ.alloc(x.b) else { return };
        wr8(x.b, o, 1);
        for (dst, size) in [(8u32, 2u32), (0xc, 2), (0x10, 2), (0x2c, 1), (2, 1), (3, 1), (0x2f, 1)] {
            if size == 2 {
                let v = rd16(x.b, a2);
                wr16(x.b, o + dst, v);
            } else {
                let v = rd8(x.b, a2);
                wr8(x.b, o + dst, v);
            }
            a2 += size;
        }
    }
}
/// `0xfbe0a` : un objet de score par joueur présent.
fn credits_objects_b(x: &mut Ctx) {
    let mut fp = var::PLAYERS;
    let mut a2 = 0xfbebc;
    for (counter, d3) in (0..4u32).rev().zip(0u8..) {
        if rd8(x.b, fp) != 0 && (rd8(x.b, fp + 4) as i8) < 4 {
            if let Some(Obj(o)) = ecran_de_fin(x.b, 2, counter as u8) {
                let variant = rd8(x.b, fp + 3);
                wr8(x.b, o + 0x2f, variant);
                let (px, py) = (rd16(x.b, a2), rd16(x.b, a2 + 2));
                a2 += 4;
                let cx = rd16(x.b, var::CAMERA_2);
                let cy = rd16(x.b, var::CAMERA_2_Y);
                wr16(x.b, o + 8, px.wrapping_add(cx));
                wr16(x.b, o + 0x10, py.wrapping_add(cy));
                wr32(x.b, o + 0xc, 0);
                wr8(x.b, o + 0x42, d3);
                wr32(x.b, o + 0x4c, BLOCK);
            }
        }
        fp += 0x200;
    }
}
/// `0xfbe6e` : deux objets (variante 18).
fn credits_objects_c(x: &mut Ctx) {
    for k in 0..2 {
        let Ok(Obj(o)) = decor_6(x.b, 18) else { return };
        wr8(x.b, o + 0x2f, k);
    }
}
/// `0xfbd38` : rendu avec défilement pendant trois images.
fn s1c_scroll(x: &mut Ctx) -> Flow {
    render_scroll(x);
    x.wait(1, Etape::S1cScroll)
}
const S1C_SCROLL: u32 = 0xfbd44;
const S1C_PALETTE: u32 = 0xfbd56;
fn s1c_wait_fade(x: &mut Ctx) -> Flow {
    x.wait(1, Etape::S1cFadeLoop)
}
const S1C_FADE_LOOP: u32 = 0xfbdc0;

/// État 30 : le générique défile ; à la fin de la minuterie, fondu (tâche), puis objet SET 18.
fn s1e_fade(x: &mut Ctx) -> Flow {
    render_scroll(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    fondu::PAS.set(x.b, 8);
    fade_settings(x, 3, FADE_ALL);
    fade_task(x, crate::scene::fondu::DESCENTE_PAS);
    s1e_loop(x)
}
fn s1e_loop(x: &mut Ctx) -> Flow {
    x.wait(1, Etape::S1eLoop)
}
const S1E_LOOP: u32 = 0xfbf00;
fn s1e_object(x: &mut Ctx) -> Flow {
    if set_15_pur(x.b, 18).is_ok() {
        return Flow::Done;
    }
    // `moveq 1,D0 ; bsr 0xfc506 ; jsr 0xa40`. Glitch d'origine : la cession dure D0 images, tel que le rendu le laisse
    // (le dernier gestionnaire d'objet appelé)
    x.c.d[0] = 1;
    render_scroll(x);
    crate::taches::attendre(x, Etape::S1eRetry)
}
const S1E_RETRY: u32 = 0xfbf26;

/// État 34 : objet SET 16, fondu (tâche), puis objet SET 20.
fn s22_object(x: &mut Ctx) -> Flow {
    if set_15_pur(x.b, 16).is_err() {
        return Flow::Done;
    }
    fondu::PAS.set(x.b, 8);
    fade_settings(x, 3, FADE_ALL);
    fade_task(x, crate::scene::fondu::MONTEE_PAS);
    s22_wait(x)
}
fn s22_wait(x: &mut Ctx) -> Flow {
    render_scroll(x);
    x.wait(1, Etape::S22Loop)
}
const S22_LOOP: u32 = 0xfbf64;

/// État 38 : après la minuterie, signal 2 et nouvelle minuterie.
fn s26_wait(x: &mut Ctx) -> Flow {
    render_scroll(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    wr8(x.b, BLOCK + f::SIGNAL, 2);
    next_step(x);
    wr16(x.b, BLOCK + f::TIMER, 120);
    Flow::Done
}

/// État 40 : après la minuterie, fondu au noir (tâche, attente de la fin).
fn s28_fade(x: &mut Ctx) -> Flow {
    render_scroll(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    fondu::PAS.set(x.b, 1);
    fade_settings(x, 3, 13);
    fade_task(x, crate::scene::fondu::DESCENTE);
    s28_loop(x)
}
fn s28_loop(x: &mut Ctx) -> Flow {
    x.wait(1, Etape::S28Loop)
}
const S28_LOOP: u32 = 0xfbfe8;

/// État 42 : vide tout, retire les plans, nouvelle image, objet SOBJ 0.
fn s2a_clear(x: &mut Ctx) -> Flow {
    render_scroll(x);
    unlink_and_clear_lists(x);
    crate::objects::pools::EFFCT.reset(x.b);
    crate::objects::pools::SOBJ.reset(x.b);
    crate::video::vider_objets(x.b);
    x.wait(1, Etape::S2aPlanes)
}
const S2A_PLANES: u32 = 0xfc022;
const S2A_SCROLLS: u32 = 0xfc034;
fn s2a_wait(x: &mut Ctx) -> Flow {
    render(x);
    x.wait(1, Etape::S2aWait)
}
const S2A_WAIT: u32 = 0xfc07e;
fn s2a_object(x: &mut Ctx) -> Flow {
    let Ok(Obj(o)) = decor_6(x.b, 0) else {
        render(x);
        return x.wait(1, Etape::S2aRetry);
    };
    wr32(x.b, o + 0x4c, BLOCK);
    wr8(x.b, o + 0x2f, 12);
    wr16(x.b, o + 8, 32);
    wr16(x.b, o + 0xc, 208);
    wr32(x.b, o + 0x10, 0);
    render(x);
    x.wait(1, Etape::S2aShow)
}
const S2A_RETRY: u32 = 0xfc0c2;
const S2A_SHOW: u32 = 0xfc0ee;

/// État 44 : après la minuterie, fondu au noir, vide tout et prépare l'écran des récompenses.
fn s2c_wait(x: &mut Ctx) -> Flow {
    if !timer_tick(x) {
        render(x);
        return Flow::Done;
    }
    next_step(x);
    fade_settings(x, 3, 13);
    fondu::PAS.set(x.b, 1);
    fade_task(x, crate::scene::fondu::DESCENTE);
    s2c_loop(x)
}
fn s2c_loop(x: &mut Ctx) -> Flow {
    render(x);
    x.wait(1, Etape::S2cLoop)
}
const S2C_LOOP: u32 = 0xfc14c;
const S2C_PLANES: u32 = 0xfc17e;
const S2C_CLEARED: u32 = 0xfc18c;
fn s2c_wait_timer(x: &mut Ctx) -> Flow {
    render(x);
    x.wait(1, Etape::S2cTimer)
}
const S2C_TIMER: u32 = 0xfc1d8;
/// `bra 0xfc4ea` : un rendu puis retour.
fn render_flow(x: &mut Ctx) -> Flow {
    render(x);
    Flow::Done
}

/// État 46 : le dialogue de fin ouvre puis ferme l'écran (fondus au noir et depuis le noir).
fn s2e_dialogue(x: &mut Ctx) -> Flow {
    let o = link(x);
    if rd8(x.b, o + 5) == 6 {
        fade_settings(x, 3, FADE_ALL);
        return x.sous_programme(crate::taches::fondu::montee, Etape::S2eOpened);
    }
    if rd8(x.b, o + 5) != 10 {
        return render_flow(x);
    }
    fade_settings(x, 3, FADE_ALL);
    x.sous_programme(crate::taches::fondu::descente, Etape::S2eClosed)
}
const S2E_OPENED: u32 = 0xfc286;
const S2E_CLOSED: u32 = 0xfc2ae;

/// État 48 : récompenses : palettes, puis un objet par joueur ayant gagné une récompense.
fn s30_rewards(x: &mut Ctx) -> Flow {
    for (src, dst) in [(0x28fa80u32, 0x91c120u32), (0x28faa0, 0x91c1a0), (0x28f4c0, 0x91c180), (0x292260, 0x91c1e0), (0x291f00, 0x91c200)] {
        for i in 0..8 {
            let v = rd32(x.b, src + 4 * i);
            wr32(x.b, dst + 4 * i, v);
        }
    }
    fondu::PAGES.set(x.b, 1);
    crate::palette::no_brightness(x.b);
    render(x);
    let mut fp = var::PLAYERS;
    for _ in 0..4 {
        if rd8(x.b, fp) != 0 && (rd8(x.b, fp + 4) as i8) < 4 && rd8(x.b, fp + 0x18a) != 0 {
            let character = rd8(x.b, fp + 3);
            crate::families::set_residu::deux_grilles(x.b, character as u16);
            break;
        }
        fp += 0x200;
    }
    x.wait(1, Etape::S30Wait)
}
const S30_WAIT: u32 = 0xfc366;

/// État 50 : dernier écran (quatre cadres de tuiles), puis fin du jeu : phase 20.
fn s32_end(x: &mut Ctx) -> Flow {
    render(x);
    if rd16(x.b, BLOCK + f::TIMER) == 120 {
        let mut a1 = 0x91336c;
        let mut a2 = 0xfc4e2;
        for _ in 0..4 {
            let first = rd16(x.b, a2);
            a2 += 2;
            // deux lignes de quatre tuiles : n, n + 16 puis n + 1, n + 17 (attribut 2)
            for tile in [first, first.wrapping_add(1)] {
                for t in [tile, tile.wrapping_add(16)] {
                    wr16(x.b, a1, t);
                    wr16(x.b, a1 + 2, 2);
                    a1 += 4;
                }
                a1 += 120;
            }
        }
    }
    if !timer_tick(x) {
        return Flow::Done;
    }
    fade_settings(x, 3, FADE_ALL);
    x.sous_programme(crate::taches::fondu::descente, Etape::S32Faded)
}
const S32_FADED: u32 = 0xfc450;
const S32_CLEARED: u32 = 0xfc470;

/// Addition décimale codée (huit chiffres, retenue finale perdue) : quatre `abcd`.
fn bcd_add(dest: u32, src: u32) -> u32 {
    let mut carry = 0u32;
    let mut out = 0u32;
    for i in 0..4 {
        let (d, s) = ((dest >> (8 * i)) & 0xff, (src >> (8 * i)) & 0xff);
        let mut lo = (d & 0xf) + (s & 0xf) + carry;
        let mut hi = (d >> 4) + (s >> 4);
        if lo > 9 {
            lo -= 10;
            hi += 1;
        }
        carry = 0;
        if hi > 9 {
            hi -= 10;
            carry = 1;
        }
        out |= (hi << 4 | lo) << (8 * i);
    }
    out
}

pub const NATIVES: &[Native] = &[
    PHASE_28,
];

#[cfg(test)]
mod tests {
    use super::bcd_add;

    #[test]
    fn addition_decimale() {
        assert_eq!(bcd_add(0x0000_0199, 0x0000_0001), 0x0000_0200);
        assert_eq!(bcd_add(0x0123_4567, 0x0876_5430), 0x0999_9997);
        // la retenue finale est perdue
        assert_eq!(bcd_add(0x9999_9999, 0x0000_0001), 0x0000_0000);
    }
}
