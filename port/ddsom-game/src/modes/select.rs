//! Phase 4 (`0x1ca20`) : sélection des personnages.
//!
//! Après la mise de départ, la partie commence par cette phase : chaque joueur présent déplace un
//! curseur sur les douze cases (six personnages, deux variantes chacun), un compte à rebours de
//! trente secondes force le choix, deux joueurs ne peuvent pas prendre la même case. Quand tous ont
//! validé, la phase passe à 8 (entrée de la première scène).
//!
//! La phase est une machine à états (`A5+0x2d`, pas de 2) qui fait un pas par image ; seules les
//! étapes d'ouverture et de fermeture cèdent la main (fondus), à des adresses d'origine reprises par
//! des étapes Rust (`etapes!`).
//!
//! | Étape | Rôle |
//! |---|---|
//! | 0 | écran de sélection : objets curseurs (`ae776`), fond, fondu d'ouverture |
//! | 2 | curseurs des joueurs, file d'attente, décompte (30 s) |
//! | 4 | attente de 20 images avant la suite |
//! | 6 | joueurs figés dans leur personnage, fondu, écran d'attente |
//! | 8 | fond du second écran (choix de route), objets |
//! | 10 / 12 | attente d'un bouton, annonce clignotante |
//! | 14 | fondu final, phase 8 |

use super::{Flow, Sequence};
use crate::scene::fondu;
use crate::dispatch;
use crate::families::effects_creators::{bandeau, element_d_interface};
use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8, A5};
use crate::objects::{pools, Obj};
use crate::scene::{Ctx, PLAYERS, PLAYER_STRIDE};
use crate::video::{charger_grille, effacer_couche, Couche, Grille};
use crate::Native;

/// Variables de la RAM de travail (adresses absolues).
mod var {
    use crate::mem::A5;
    /// Phase de partie, étape (octets), sous-étape (mot).
    pub const PHASE: u32 = A5 + 0x2c;
    pub const STEP: u32 = A5 + 0x2d;
    pub const SUB: u32 = A5 + 0x2e;
    /// Joueurs figés (≠ 0) ; partie active ; boutons ; masque des joueurs en jeu.
    pub const PLAYERS_FROZEN: u32 = A5 + 0x4a;
    pub const GAME_ACTIVE: u32 = A5 + 0x30;
    pub const BUTTONS_A: u32 = A5 + 4;
    pub const BUTTONS_B: u32 = A5 + 0xa;
    pub const FRAME_COUNTER: u32 = A5 + 0x6f;
    pub const MODE_FLAG: u32 = A5 + 0x73;
    pub const SCREEN_ID: u32 = A5 + 0x3c;
    pub const VIDEO: u32 = A5 + 0x14;
    /// Fondu.
    pub const FADE_SOURCES: u32 = A5 + 0x492;
    pub const SCROLL_1: u32 = A5 + 0x4b6;
    pub const SCROLL_2: u32 = A5 + 0x4ba;
    pub const CAMERA: u32 = A5 + 0x506;
    pub const CAMERA_Y: u32 = A5 + 0x50a;
    pub const CAMERA_2: u32 = A5 + 0x556;
    pub const CAMERA_2_Y: u32 = A5 + 0x55a;
    /// Annonce clignotante : adresse de texte (long), abscisse et ordonnée.
    pub const BANNER: u32 = A5 + 0x5c6;
    pub const BANNER_X: u32 = A5 + 0x5ca;
    pub const BANNER_Y: u32 = A5 + 0x5cc;
    /// Indicateur d'écran en attente (octet à FF0EAB).
    pub const SCREEN_WAIT: u32 = A5 - 0x7155;

    /// Mot de décompte (octet haut : secondes, octet bas : images), cases prises (bits), liste des choix.
    pub const COUNTDOWN: u32 = A5 - 0x6602;
    pub const COUNTDOWN_TICK: u32 = A5 - 0x6601;
    pub const TAKEN: u32 = A5 - 0x6600;
    pub const CHOICES: u32 = A5 - 0x65fe;
    pub const WORD_9A0E: u32 = A5 - 0x65f2;
    pub const LINK_9A10: u32 = A5 - 0x65f0;
    pub const PLAYERS_PRESENT: u32 = A5 - 0x65ee;
    pub const LOCKED_FLAG: u32 = A5 - 0x65ed;
    pub const SLOT_LINKS: u32 = A5 - 0x660c;
    pub const MAIN_LINK: u32 = A5 - 0x6604;
}

/// Sons et tâches de fondu de l'écran.
mod ext {
    pub const SOUND_E10D8: u32 = 0xe10d8;
    pub const SOUND_E109C: u32 = 0xe109c;
}

/// Tables en ROM : octets de variante par personnage (code), annonces, déplacements du curseur (données). Les
/// routines d'initialisation des quatre joueurs (0x1d0f6) sont celles de `player::init::initialiser`.
mod table {
    pub const VARIANT: u32 = 0x1d118;
    pub const BANNERS: u32 = 0x1d3aa;
    pub const CURSOR_DOWN_DELTAS: u32 = 0x1d6d2;
    pub const CURSOR_UP_DELTAS: u32 = 0x1d6cc;
}

const SLOTS: usize = 12;
const COUNTDOWN_START: u16 = 0x1432;

fn prog32(x: &mut Ctx, addr: u32) -> u32 {
    (x.prog16(addr) as u32) << 16 | x.prog16(addr + 2) as u32
}

/// Mot de l'espace programme étendu du signe (adresse `movea.w`).
fn sign_ext(v: u16) -> u32 {
    v as i16 as i32 as u32 & 0xff_ffff
}

fn player(k: u32) -> u32 {
    PLAYERS + k * PLAYER_STRIDE
}

/// Aiguillage des objets des classes données, puis la liste de sprites (`jsr 0x39cba`... `jsr 0x1de96`).
fn render(x: &mut Ctx, classes: &[&dispatch::Class]) {
    for classe in classes {
        classe.update_sans_suite(x.c, x.b);
    }
    crate::sprites::construire_68k(x.c, x.b);
}

/// Deux blocs de tuiles (`n`, `n + 1`) à la suite l'un de l'autre à partir de `destination`.
fn two_blocks(x: &mut Ctx, grille: Grille, n: u16, destination: u32) {
    let fin = charger_grille(x.b, grille, n, destination);
    charger_grille(x.b, grille, n + 1, fin.destination);
}

pub const PHASE_4: Native = Native {
    entry: 0x1ca20,
    name: "select_phase_4",
    run: crate::Corps::Processeur(|c, b| {
        let mut x = Ctx::new(c, b);
        if phase_4(&mut x) == Flow::Done {
            rts(x.c, x.b);
        }
    }),
    touches_sound: false,
    outputs: 0,
};

/// Phase 4 de la partie (`0x1ca20`) : l'écran de sélection, un pas par image.
pub fn phase_4(x: &mut Ctx) -> Flow {
    let step = rd8(x.b, var::STEP);
    match step {
        0 => step_0(x),
        2 => step_2(x),
        4 => step_4(x),
        6 => step_6(x),
        8 => step_8(x),
        10 => step_a(x),
        12 => step_c(x),
        14 => step_e(x),
        _ => Flow::Done,
    }
}

// ---------------------------------------------------------------------------------------------
// Étape 0 : écran de sélection

fn step_0(x: &mut Ctx) -> Flow {
    wr8(x.b, var::SCREEN_WAIT, 0);
    let s = rd8(x.b, var::STEP).wrapping_add(2);
    wr8(x.b, var::STEP, s);
    wr8(x.b, var::SUB, 0);
    crate::video::vider_objets(x.b);
    for pool in [&crate::objects::pools::EFFCT, &crate::objects::pools::SET, &crate::objects::pools::SOBJ] {
        pool.reset(x.b);
    }
    crate::sprites::vider_listes(x.b);
    wr32(x.b, var::SCROLL_1, 0);
    wr32(x.b, var::SCROLL_2, 0);
    // curseur de départ de chaque joueur : un bit de case par joueur
    for (k, cell) in [0u8, 2, 4, 8].into_iter().enumerate() {
        wr8(x.b, player(k as u32) + 3, cell);
    }
    for off in [0x770u32, 0x970, 0xb70, 0xd70] {
        wr8(x.b, A5 + off, 0);
    }
    for off in [0x6f6u32, 0x6fa, 0x8f6, 0x8fa, 0xaf6, 0xafa, 0xcf6, 0xcfa] {
        wr32(x.b, A5 + off, 0xffff_ffff);
    }
    wr16(x.b, var::TAKEN, 0);
    wr16(x.b, var::WORD_9A0E, 0);
    wr8(x.b, var::LOCKED_FLAG, 0);
    wr16(x.b, var::CAMERA, 0x20);
    wr16(x.b, var::CAMERA_Y, 0x20);
    wr16(x.b, var::CAMERA_2_Y, 0x30);
    wr16(x.b, var::CAMERA_2, 0x10);
    wr16(x.b, var::CAMERA_2_Y, 0xfff8);
    for off in [0u32, 4, 8] {
        wr32(x.b, var::CHOICES + off, 0xffff_ffff);
    }
    crate::hud::forget_displayed(x.b);
    crate::sound::jouer_global(x.b, ext::SOUND_E10D8);
    wr16(x.b, var::COUNTDOWN, 0x203c);
    wr16(x.b, var::COUNTDOWN, COUNTDOWN_START);
    objects_0(x)
}

/// Crée les objets de l'écran : quatre curseurs, un panneau, six marqueurs, un compteur, huit
/// cases ; si un pool est plein l'étape s'arrête (l'original sort sans rien défaire).
fn objects_0(x: &mut Ctx) -> Flow {
    let mut links = var::SLOT_LINKS;
    for slot in (0..4u32).rev() {
        let Some(Obj(o)) = element_d_interface(x.b) else {
            return Flow::Done;
        };
        wr8(x.b, o + 3, 1);
        wr8(x.b, o + 0x2f, slot as u8);
        wr16(x.b, links, o as u16);
        links += 2;
    }
    let Some(Obj(main)) = element_d_interface(x.b) else {
        return Flow::Done;
    };
    wr8(x.b, main + 3, 0);
    wr8(x.b, main + 0x1c, 0);
    wr16(x.b, var::MAIN_LINK, main as u16);
    publish_choices(x, main);
    for k in 0..4u32 {
        let o = sign_ext(rd16(x.b, var::SLOT_LINKS + 2 * k));
        wr16(x.b, o + 0x1e, main as u16);
    }
    for slot in (0..6u32).rev() {
        let Some(Obj(o)) = element_d_interface(x.b) else {
            return Flow::Done;
        };
        wr8(x.b, o + 3, 2);
        wr8(x.b, o + 0x2f, slot as u8);
        wr16(x.b, o + 0x1e, main as u16);
    }
    let Some(Obj(o)) = element_d_interface(x.b) else {
        return Flow::Done;
    };
    wr8(x.b, o + 3, 3);
    wr8(x.b, o + 0x2f, 0);
    wr16(x.b, var::LINK_9A10, o as u16);
    let seconds = rd8(x.b, var::COUNTDOWN);
    wr8(x.b, o + 0x14, seconds);
    wr8(x.b, o + 0x15, 0);
    for slot in (0..8u32).rev() {
        let Some(Obj(o)) = element_d_interface(x.b) else {
            return Flow::Done;
        };
        wr8(x.b, o + 3, 4);
        wr8(x.b, o + 0x2f, slot as u8);
    }
    render(x, &[&dispatch::EFFCT]);
    x.wait(2, Etape::FadeStart)
}
const FADE_START: u32 = 0x1cc0c;
crate::etapes! {
    /// Étapes (points de reprise) de la machine à états.
    pub enum Etape {
        FadeStart = FADE_START => |x| {
            fondu::DELAI.set(x.b, 1);
            fondu::PAGES.set(x.b, 0xf);
            wr32(x.b, var::FADE_SOURCES, 0x28ff00);
            wr32(x.b, var::FADE_SOURCES + 4, 0x25c0c0);
            wr32(x.b, var::FADE_SOURCES + 8, 0x279460);
            wr32(x.b, var::FADE_SOURCES + 12, 0x28c6e0);
            create_task(x, crate::scene::fondu::DEPUIS_LE_BLANC, 0x10);
            fondu::PAS.set(x.b, 1);
            two_blocks(x, Grille::Couche2, 0x203, 0x917000);
            two_blocks(x, Grille::Couche3, 0xfc, 0x91b800);
            wr16(x.b, var::VIDEO, 0x12ce);
            fade_wait(x)
        },
        FadeLoop = FADE_LOOP => fade_wait,
        Step6Faded = STEP_6_FADED => |x| {
            dispatch::EFFCT.update_sans_suite(x.c, x.b);
            crate::objects::pools::EFFCT.reset(x.b);
            crate::sprites::vider_listes(x.b);
            crate::sprites::construire_68k(x.c, x.b);
            let v = rd16(x.b, var::VIDEO) & !0xe;
            wr16(x.b, var::VIDEO, v);
            x.wait(1, Etape::Step6Clear)
        },
        Step6Clear = STEP_6_CLEAR => |x| {
            wr8(x.b, var::SCREEN_ID, 0xff);
            crate::video::vider_objets(x.b);
            clear_layers(x);
            x.wait(4, Etape::Step6Palette)
        },
        Step6Palette = STEP_6_PALETTE => |x| {
            // 0x1a608 (la page 1 prend l'entrée 0), 0x1a67e, 0x1a8b2, 0x1a3d4
            crate::palette::charger_page_de_la_scene(x.b, 1);
            crate::palette::charger_page(x.b, 2, 0x56);
            crate::palette::charger_page(x.b, 3, 0x56);
            crate::palette::charger_ensemble(x.b, 3);
            for i in 0..8 {
                for j in 0..8 {
                    wr32(x.b, 0x91c000 + 0x20 * i + 4 * j, 0);
                }
            }
            fondu::PAGES.set(x.b, 0xf);
            crate::palette::full_brightness(x.b);
            for v in [var::SCROLL_1, var::SCROLL_2, var::CAMERA, var::CAMERA_Y, var::CAMERA_2, var::CAMERA_2_Y] {
                wr32(x.b, v, 0);
            }
            Flow::Done
        },
        Step8Show = STEP_8_SHOW => |x| {
            let v = rd16(x.b, var::VIDEO) | 0xe;
            wr16(x.b, var::VIDEO, v);
            x.c.d[0] = 1;
            crate::taches::attendre_puis_revenir(x)
        },
        StepEFaded = STEP_E_FADED => |x| {
            crate::objects::pools::SET.reset(x.b);
            crate::objects::pools::SOBJ.reset(x.b);
            crate::sprites::vider_listes(x.b);
            crate::video::vider_objets(x.b);
            x.wait(1, Etape::StepEHide)
        },
        StepEHide = STEP_E_HIDE => |x| {
            let v = rd16(x.b, var::VIDEO) & !0xe;
            wr16(x.b, var::VIDEO, v);
            x.wait(1, Etape::StepEClear)
        },
        StepEClear = STEP_E_CLEAR => |x| {
            clear_layers(x);
            x.wait(4, Etape::StepEFade)
        },
        StepEFade = STEP_E_FADE => |x| {
            fondu::DELAI.set(x.b, 3);
            fondu::PAGES.set(x.b, 0xf);
            create_task(x, crate::scene::fondu::MONTEE, 0x10);
            Flow::Done
        },
    }
}
/// Attend la fin du fondu d'ouverture (une image par tour), puis allume les curseurs.
fn fade_wait(x: &mut Ctx) -> Flow {
    if fondu::PAS.get(x.b) != 0 {
        render(x, &[&dispatch::EFFCT]);
        return x.wait(1, Etape::FadeLoop);
    }
    wr8(x.b, var::SCREEN_ID, 3);
    let main = sign_ext(rd16(x.b, var::MAIN_LINK));
    wr8(x.b, main + 0x1c, 1);
    // `jmp 0x1de96` à la fin
    render(x, &[&dispatch::EFFCT]);
    Flow::Done
}

/// Les trois couches effacées.
fn clear_layers(x: &mut Ctx) {
    for couche in [Couche::Un, Couche::Deux, Couche::Trois] {
        effacer_couche(x.b, couche);
    }
}
const FADE_LOOP: u32 = 0x1cca4;

/// Crée la tâche d'entrée `entry` dans l'emplacement `slot` (`move.w #slot,D0w ; jsr 0x9b6`).
fn create_task(x: &mut Ctx, entry: u32, slot: u16) {
    crate::kernel::creer_tache(x.b, slot as u32, entry);
}

// ---------------------------------------------------------------------------------------------
// Étape 2 : curseurs, décompte

fn step_2(x: &mut Ctx) -> Flow {
    count_players(x);
    if countdown(x) {
        // décompte fini : la suite
        wr8(x.b, var::STEP, 4);
        wr8(x.b, var::PLAYERS_FROZEN, 1);
    }
    for k in 0..4 {
        let p = player(k);
        if rd8(x.b, p) != 0 {
            cursor(x, p);
        }
    }
    queue(x);
    mark_choices(x);
    check_all_chosen(x);
    let main = sign_ext(rd16(x.b, var::MAIN_LINK));
    publish_choices(x, main);
    wr8(x.b, var::LOCKED_FLAG, 0);
    // `jmp 0x1de96` à la fin
    render(x, &[&dispatch::EFFCT]);
    Flow::Done
}

/// Recopie les choix des joueurs dans l'objet principal (lu par les objets curseurs).
fn publish_choices(x: &mut Ctx, main: u32) {
    for k in 0..3u32 {
        let v = rd32(x.b, var::CHOICES + 4 * k);
        wr32(x.b, main + 8 + 4 * k, v);
    }
    let v = rd16(x.b, var::WORD_9A0E);
    wr16(x.b, main + 0x14, v);
    let v = rd8(x.b, var::LOCKED_FLAG);
    wr8(x.b, main + 0x18, v);
}

/// `0x1d6d8` : compte les joueurs présents et réarme le décompte quand leur nombre change.
fn count_players(x: &mut Ctx) {
    let mut n = 0u8;
    for k in 0..4 {
        let p = player(k);
        if rd8(x.b, p) != 0 {
            n += 1;
            wr8(x.b, p + 0x192, 1);
        }
    }
    if n != rd8(x.b, var::PLAYERS_PRESENT) {
        wr16(x.b, var::COUNTDOWN, COUNTDOWN_START);
        let panel = sign_ext(rd16(x.b, var::LINK_9A10));
        wr8(x.b, panel + 0x14, 20);
    }
    wr8(x.b, var::PLAYERS_PRESENT, n);
}

/// `0x1d802` : une image du décompte ; vrai quand les secondes sont écoulées (drapeau Z de l'original).
fn countdown(x: &mut Ctx) -> bool {
    let tick = rd8(x.b, var::COUNTDOWN_TICK).wrapping_sub(1);
    wr8(x.b, var::COUNTDOWN_TICK, tick);
    if tick != 0 {
        return false;
    }
    let seconds = rd8(x.b, var::COUNTDOWN);
    if seconds == 0 {
        return true;
    }
    let seconds = seconds - 1;
    wr8(x.b, var::COUNTDOWN, seconds);
    let panel = sign_ext(rd16(x.b, var::LINK_9A10));
    wr8(x.b, panel + 0x14, seconds);
    wr8(x.b, var::COUNTDOWN_TICK, 50);
    false
}

/// `0x1d584` : curseur du joueur `a0` (cases 0 à 11, deux par personnage).
fn cursor(x: &mut Ctx, a0: u32) {
    if rd8(x.b, a0 + 3) & 0x80 != 0 {
        return;
    }
    dedupe(x, a0);
    let d1 = crate::inputs::entrees_du_joueur(x.b, a0).nouvelles;
    let mut d3 = rd8(x.b, a0 + 3) as u32;
    let slot = rd8(x.b, a0 + 2);
    let mut d0: u8 = 0;
    loop {
        let d5 = ((d3 as u8) >> 1) as u32;
        if d1 & 2 != 0 {
            d0 = rd8(x.b, table::CURSOR_DOWN_DELTAS + d5);
        }
        if d1 & 1 != 0 {
            d0 = rd8(x.b, table::CURSOR_UP_DELTAS + d5);
        }
        if d0 == 0 {
            break;
        }
        // libère la case quittée
        let mut cell = var::CHOICES + (d3 & 0xffff);
        if rd8(x.b, cell) == slot || {
            cell += 1;
            rd8(x.b, cell) == slot
        } {
            wr8(x.b, cell, 0xff);
        }
        // nouvelle case, avec retour à l'autre bout
        let mut n = (d3 as u8).wrapping_add(d0);
        if n & 0x80 != 0 {
            n = 10;
        }
        if n > 10 {
            n = 0;
        }
        d3 = n as u32;
        let first = var::CHOICES + d3;
        if rd8(x.b, first) & 0x80 != 0 {
            wr8(x.b, first, slot);
            break;
        }
        if rd8(x.b, first + 1) & 0x80 != 0 {
            wr8(x.b, first + 1, slot);
            break;
        }
    }
    // validation
    d0 = 0;
    let mut lock = true;
    if rd16(x.b, var::COUNTDOWN) != 0 {
        if d1 as u8 & 0xf0 == 0 {
            let s = slot as u32;
            d0 = 1;
            let held = !rd8(x.b, var::BUTTONS_B) & rd8(x.b, var::BUTTONS_A);
            lock = (held >> s) & 1 != 0;
        }
    }
    if lock {
        let mut taken = rd16(x.b, var::TAKEN) as u32;
        taken |= 1 << (d3 & 31);
        wr16(x.b, var::TAKEN, taken as u16);
        let mut other = rd16(x.b, var::WORD_9A0E) as u32;
        d3 = (d3 as u8).wrapping_add(d0) as u32;
        if (other >> (d3 & 31)) & 1 != 0 {
            d3 ^= 1;
        }
        other |= 1 << (d3 & 31);
        wr16(x.b, var::WORD_9A0E, other as u16);
        // 0x2e630 : statistique du personnage (carte de développement seulement). Glitch d'origine : elle reçoit
        // D0.w, dont seul l'octet bas est le choix ; l'octet haut est un reste des appels précédents
        let d0 = (x.c.d[0] as u16 & 0xff00) | (d3 & 0xff) as u16;
        crate::eeprom::stat_personnage_choisi(x.b, d0);
        d3 |= 0x80;
        wr8(x.b, var::LOCKED_FLAG, 1);
    }
    wr8(x.b, a0 + 3, d3 as u8);
}

/// `0x1d714` : le joueur ne doit pas occuper la case d'un autre joueur ; sinon il avance de deux cases.
fn dedupe(x: &mut Ctx, a0: u32) {
    loop {
        let cell = rd8(x.b, a0 + 3) as u32;
        let a1 = var::CHOICES + cell;
        let slot = rd8(x.b, a0 + 2);
        let first = rd8(x.b, a1);
        if first & 0x80 != 0 || first == slot {
            // la première place est libre ou à moi : la seconde ne doit pas être à moi
            if rd8(x.b, a1 + 1) != slot {
                wr8(x.b, a1, slot);
            }
            return;
        }
        let second = rd8(x.b, a1 + 1);
        if second & 0x80 != 0 {
            wr8(x.b, a1 + 1, slot);
            return;
        }
        if second == slot {
            return;
        }
        let mut n = rd8(x.b, a0 + 3).wrapping_add(2);
        if n > 12 {
            n = 0;
        }
        wr8(x.b, a0 + 3, n);
    }
}

/// `0x1d760` : un joueur qui n'a pas validé et dont la case voisine a été libérée prend la première place.
fn queue(x: &mut Ctx) {
    let taken = rd16(x.b, var::TAKEN) as u32;
    for k in 0..6u32 {
        let a1 = var::CHOICES + 2 * k;
        if rd8(x.b, a1) & 0x80 == 0 {
            continue;
        }
        let other = rd8(x.b, a1 + 1);
        if other & 0x80 != 0 || (taken >> (2 * k)) & 1 != 0 {
            continue;
        }
        wr8(x.b, a1, other);
        wr8(x.b, a1 + 1, 0xff);
    }
}

/// `0x1d794` : pour chaque curseur de case, `+0x16` = parité de la case occupée par son joueur.
fn mark_choices(x: &mut Ctx) {
    for k in 0..4u32 {
        let o = sign_ext(rd16(x.b, var::SLOT_LINKS + 2 * k));
        let slot = rd8(x.b, o + 0x2f);
        let mut parity = 0u8;
        for i in 0..SLOTS as u32 {
            if rd8(x.b, var::CHOICES + i) == slot {
                break;
            }
            parity ^= 1;
        }
        wr8(x.b, o + 0x16, parity);
    }
}

/// `0x1d7c0` : si tous les joueurs présents ont validé, passe à l'étape 4.
fn check_all_chosen(x: &mut Ctx) {
    if rd8(x.b, var::STEP) != 4 {
        let mut left = rd8(x.b, var::PLAYERS_PRESENT);
        for k in 0..4 {
            if rd8(x.b, player(k) + 3) & 0x80 != 0 {
                left = left.wrapping_sub(1);
                if left == 0 {
                    wr8(x.b, var::STEP, 4);
                    freeze(x);
                    return;
                }
            }
        }
        return;
    }
    freeze(x);
}
fn freeze(x: &mut Ctx) {
    wr8(x.b, var::COUNTDOWN, 30);
    wr8(x.b, var::PLAYERS_FROZEN, 1);
}

// ---------------------------------------------------------------------------------------------
// Étape 4 : courte attente

fn step_4(x: &mut Ctx) -> Flow {
    render(x, &[&dispatch::EFFCT]);
    let left = rd8(x.b, var::COUNTDOWN).wrapping_sub(1);
    wr8(x.b, var::COUNTDOWN, left);
    if left == 0 {
        let s = rd8(x.b, var::STEP).wrapping_add(2);
        wr8(x.b, var::STEP, s);
    }
    Flow::Done
}

// ---------------------------------------------------------------------------------------------
// Étape 6 : personnages figés, fondu

fn step_6(x: &mut Ctx) -> Flow {
    let s = rd8(x.b, var::STEP).wrapping_add(2);
    wr8(x.b, var::STEP, s);
    for k in 0..4 {
        let p = player(k);
        if rd8(x.b, p) == 0 {
            continue;
        }
        let cell = rd8(x.b, p + 3) & 0x7f;
        wr8(x.b, p + 3, cell);
        let variant = x.prog8(table::VARIANT + (cell as u32 >> 1));
        wr8(x.b, p + 0x15d, variant);
        // la routine de la table 0x1d0f6 (`movea.w`, appelée avec A0 = le joueur) est l'initialisation du joueur de
        // ce numéro (0x2e58, 0x2e6e, 0x2e86, 0x2e9e)
        let kind = rd8(x.b, p + 2) as u32;
        assert!(kind < 4, "sélection : joueur {kind} hors de la table 0x1d0f6");
        crate::player::init::initialiser(x.c, x.b, kind);
        if (rd8(x.b, p + 3) & 0x7f) >> 1 == 5 {
            wr8(x.b, p + 0x162, 1);
            wr8(x.b, p + 0x16b, 5);
            wr8(x.b, 0xff2478 + 0x3f, 1);
        }
    }
    crate::sound::service(x.b, crate::sound::Service::Ff07 { verrou: 0, ligne: 0x1c });
    fondu::DELAI.set(x.b, 1);
    fondu::PAGES.set(x.b, 0xf);
    x.sous_programme(crate::taches::fondu::descente, Etape::Step6Faded)
}
const STEP_6_FADED: u32 = 0x1d19c;
const STEP_6_CLEAR: u32 = 0x1d1c2;
const STEP_6_PALETTE: u32 = 0x1d1e8;

// ---------------------------------------------------------------------------------------------
// Étape 8 : second écran

fn step_8(x: &mut Ctx) -> Flow {
    let s = rd8(x.b, var::STEP).wrapping_add(2);
    wr8(x.b, var::STEP, s);
    crate::sound::service(x.b, crate::sound::Service::Ff06 { verrou: 0xff, ligne: 4 });
    crate::sound::jouer_global(x.b, ext::SOUND_E109C);
    two_blocks(x, Grille::Couche2, 0x208, 0x917000);
    two_blocks(x, Grille::Couche3, 0x14a, 0x91b800);
    let mut count = 0u8;
    for counter in (0..2u8).rev() {
        // la variante du panneau est le compteur de la boucle `dbf` (D0)
        let Some(Obj(o)) = bandeau(x.b, counter) else { break };
        wr32(x.b, o + 0x58, var::PLAYERS_PRESENT);
        count = count.wrapping_add(1);
    }
    wr8(x.b, var::PLAYERS_PRESENT, count);
    let mut d4 = 0xeau8;
    for _ in 0..2 {
        let Some(Obj(o)) = pools::SOBJ.alloc(x.b) else { break };
        wr8(x.b, o, 1);
        wr8(x.b, o + 2, 0);
        wr8(x.b, o + 3, d4);
        d4 = d4.wrapping_add(1);
        wr16(x.b, o + 8, 0x80);
        wr16(x.b, o + 0xc, 0xc0);
        wr32(x.b, o + 0x10, 1);
    }
    render(x, &[&dispatch::EFFCT, &dispatch::SOBJ]);
    x.wait(2, Etape::Step8Show)
}
const STEP_8_SHOW: u32 = 0x1d31c;

/// `0x1d686` : un joueur en jeu (état < 4) a-t-il pressé un bouton ? (drapeau Z de l'original : faux)
fn any_button(x: &mut Ctx) -> bool {
    if rd8(x.b, var::GAME_ACTIVE) == 0 {
        return false;
    }
    let mut buttons = 0u8;
    for k in 0..4u32 {
        let p = player(3 - k);
        if rd8(x.b, p) == 0 || (rd8(x.b, p + 4) as i8) >= 4 {
            continue;
        }
        buttons |= rd8(x.b, A5 + 0xc + (3 - k));
    }
    buttons & 0xf0 != 0
}

// ---------------------------------------------------------------------------------------------
// Étapes 10, 12, 14

fn step_a(x: &mut Ctx) -> Flow {
    render(x, &[&dispatch::EFFCT, &dispatch::SOBJ]);
    if any_button(x) {
        wr16(x.b, var::COUNTDOWN, 1);
        crate::objects::pools::EFFCT.reset(x.b);
    } else if rd8(x.b, var::PLAYERS_PRESENT) != 0 {
        return Flow::Done;
    }
    let s = rd8(x.b, var::STEP).wrapping_add(2);
    wr8(x.b, var::STEP, s);
    let which = rd8(x.b, var::MODE_FLAG).wrapping_sub(2) as u16;
    let banner = prog32(x, table::BANNERS + 2 * which as u32);
    wr32(x.b, var::BANNER, banner);
    wr16(x.b, var::BANNER_X, 0x44);
    wr16(x.b, var::BANNER_Y, 0xc0);
    wr8(x.b, var::PLAYERS_PRESENT, 0);
    wr8(x.b, var::LOCKED_FLAG, 0);
    fondu::DELAI.set(x.b, 3);
    fondu::PAGES.set(x.b, 0xf);
    fondu::PAS.set(x.b, 7);
    create_task(x, crate::scene::fondu::DESCENTE_PAS, 0x10);
    Flow::Done
}

fn step_c(x: &mut Ctx) -> Flow {
    render(x, &[&dispatch::SET, &dispatch::SOBJ]);
    if rd8(x.b, var::FRAME_COUNTER) & 3 == 0 {
        let y = rd16(x.b, var::BANNER_Y).wrapping_sub(1);
        wr16(x.b, var::BANNER_Y, y);
        if y <= 0x40 {
            let s = rd8(x.b, var::STEP).wrapping_add(2);
            wr8(x.b, var::STEP, s);
            wr16(x.b, var::BANNER_Y, 0x40);
            wr16(x.b, var::COUNTDOWN, 0x258);
            return finish_c(x);
        }
        let phase = (y.wrapping_sub(1) & 0x10) as u8;
        if phase != rd8(x.b, var::PLAYERS_PRESENT) && rd8(x.b, var::LOCKED_FLAG) != 7 {
            let v = rd8(x.b, var::PLAYERS_PRESENT) ^ 0x10;
            wr8(x.b, var::PLAYERS_PRESENT, v);
            // 0xcdedc : objet SET de type 13 (le clignotement), de variante le compteur
            let n = rd8(x.b, var::LOCKED_FLAG);
            if crate::families::creators::set_13_pur(x.b, n).is_ok() {
                wr8(x.b, var::LOCKED_FLAG, n + 1);
            }
        }
    }
    finish_c(x)
}
fn finish_c(x: &mut Ctx) -> Flow {
    if any_button(x) {
        let s = rd8(x.b, var::STEP).wrapping_add(2);
        wr8(x.b, var::STEP, s);
        wr16(x.b, var::COUNTDOWN, 1);
    }
    Flow::Done
}

fn step_e(x: &mut Ctx) -> Flow {
    render(x, &[&dispatch::SET, &dispatch::SOBJ]);
    if fondu::PAS.get(x.b) != 0 {
        return Flow::Done;
    }
    if any_button(x) {
        wr16(x.b, var::COUNTDOWN, 1);
    }
    let left = rd16(x.b, var::COUNTDOWN).wrapping_sub(1);
    wr16(x.b, var::COUNTDOWN, left);
    if left != 0 {
        return Flow::Done;
    }
    crate::sound::service(x.b, crate::sound::Service::Ff07 { verrou: 0, ligne: 0x1c });
    let phase = rd8(x.b, var::PHASE).wrapping_add(4);
    wr8(x.b, var::PHASE, phase);
    wr8(x.b, var::STEP, 0);
    wr16(x.b, var::SUB, 0);
    fondu::DELAI.set(x.b, 1);
    x.c.d[0] = 8;
    fondu::PAGES.set(x.b, 0xf);
    x.sous_programme(crate::taches::fondu::descente_d0, Etape::StepEFaded)
}
const STEP_E_FADED: u32 = 0x1d498;
const STEP_E_HIDE: u32 = 0x1d4b8;
const STEP_E_CLEAR: u32 = 0x1d4c6;
const STEP_E_FADE: u32 = 0x1d4e0;

pub const NATIVES: &[Native] = &[
    PHASE_4,
];
