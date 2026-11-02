//! Scripts de transition entre deux scènes (table `0xf81fa`, sous-état 2 de l'état partagé `0xf7ecc`).
//!
//! Après le fondu au noir de la sortie de scène, le script propre à la scène qui se termine (la scène 105
//! prend celui de la 69) fait, une image après l'autre, l'intermède entre deux niveaux : décors fixes,
//! dialogues (objets SET `0xc8312`, dont l'octet `+0x23` donne la réponse choisie), objets système
//! animés, défilements, fondus ; puis il fixe la scène suivante et rend la main à la sortie de scène.
//!
//! Variables (`A5-0x5520`, A6 dans l'original) : `+0` état (pas de 2), `+1` sous-état, `+2` minuterie (mot),
//! `+4` message ouvert (mot) ou scène choisie (octet), `+6` drapeau ou choix, `+8` signal échangé avec les
//! objets système du script (ils reçoivent l'adresse des variables en `+0x4c`).
//!
//! Le code d'origine cède la main à l'ordonnanceur à presque chaque pas (`jsr 0xa40`, fondus `0x144c`/`0x1460`,
//! `0x1ccc2`...) ; comme pour la sortie de scène, chaque reprise est une native posée à l'adresse que
//! l'original reprend (adresse qui suit le `jsr`/`bsr`), et les sous-programmes qui cèdent sont appelés comme
//! dans l'original : adresse de retour empilée (`bsr`, [`Ret::At`]) ou appel terminal (`bra`, [`Ret::Caller`],
//! retour à l'aiguillage de la sortie de scène).

pub(crate) mod early;
pub(crate) mod late;

use super::Ctx;
use crate::scene::fondu;
use crate::video::{charger_grille, effacer_couche, Couche, Grille};
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::modes::{Flow, Sequence};
use crate::objects::Obj;
use crate::taches::table::Suite;
use crate::Native;

/// Variables du script (A6).
pub const VARS: u32 = A5 - 0x5520;
mod var {
    pub const STATE: u32 = 0;
    pub const SUB: u32 = 1;
    pub const TIMER: u32 = 2;
    pub const MESSAGE: u32 = 4;
    pub const FLAG: u32 = 6;
    pub const SIGNAL: u32 = 8;
}

/// Variables de la RAM de travail (décalages de A5).
mod a5 {
    /// Couches affichées (mot : bits 2-3 plans 2 et 3, bit 1 plan 1).
    pub const LAYERS: u32 = 0x14;
    pub const SCENE: u32 = 0x34;
    /// Horloge de la scène (mot BCD, minutes et secondes) ; non nulle : temps restant.
    pub const CLOCK: u32 = 0x38;
    /// Fondu : pages, pas restants, délai, palettes cibles (longs).
    /// Caméras : position (X, Y) et leurs copies (`+8`).
    pub const CAM1_X: u32 = 0x506;
    pub const CAM1_Y: u32 = 0x50a;
    pub const CAM2_X: u32 = 0x556;
    pub const CAM2_Y: u32 = 0x55a;
    pub const COPY: u32 = 8;
    /// Texte défilant (h) : pointeur sur le texte, puis deux mots d'en-tête.
    pub const SCROLL_TEXT: u32 = 0x5c6;
    /// Région (mot, 2 et plus), qui choisit la langue des textes.
    pub const REGION: u32 = 0x72;
    pub const PLAYERS: u32 = 0x5de;
}

/// Pages de palettes : toutes, ou celles de la scène (0, 2 et 3).
const ALL_PAGES: u8 = 15;
const SCENE_PAGES: u8 = 13;

/// Grilles de blocs de tuiles : couche 2 (0x26d4) et couche 3 (0x2766).
const TB: Grille = Grille::Couche2;
const TB2: Grille = Grille::Couche3;

/// Plans des décors : plan 1 (caractères), plan 2 et plan 3, en GFX RAM.
const SCROLL_2: u32 = 0x917000;
const SCROLL_3: u32 = 0x91b800;

/// Suite d'un sous-programme qui cède : retour à une adresse du script (`bsr`), ou à l'appelant du script
/// (`bra`, l'aiguillage de la sortie de scène).
#[derive(Clone, Copy)]
enum Ret {
    At(Suite),
    Caller,
}
use Ret::{At, Caller};

/// Rend la main (la tâche a cédé) si l'attente n'est pas finie.
macro_rules! or_yield {
    ($e:expr) => {
        if !$e {
            return Flow::Yield;
        }
    };
}
use or_yield;

// ---------------------------------------------------------------------------------------------
// Variables

fn get8(x: &mut Ctx, off: u32) -> u8 {
    rd8(x.b, VARS + off)
}
fn set8(x: &mut Ctx, off: u32, v: u8) {
    wr8(x.b, VARS + off, v)
}
fn get16(x: &mut Ctx, off: u32) -> u16 {
    rd16(x.b, VARS + off)
}
fn set16(x: &mut Ctx, off: u32, v: u16) {
    wr16(x.b, VARS + off, v)
}
/// État suivant.
fn next(x: &mut Ctx) {
    let s = get8(x, var::STATE).wrapping_add(2);
    set8(x, var::STATE, s);
}
fn next_sub(x: &mut Ctx) {
    let s = get8(x, var::SUB).wrapping_add(2);
    set8(x, var::SUB, s);
}
/// Retire une image à la minuterie ; vrai quand elle atteint zéro.
fn tick(x: &mut Ctx) -> bool {
    let t = get16(x, var::TIMER).wrapping_sub(1);
    set16(x, var::TIMER, t);
    t == 0
}

/// Aiguillage `moveq #0,D0 ; move.b (off,A6),D0 ; move.w (table,PC,D0.w),D1` : rend l'état (la table n'est que
/// celle de l'original).
fn dispatch(x: &mut Ctx, off: u32, _table: u32) -> u8 {
    get8(x, off)
}

fn layers_or(x: &mut Ctx, bits: u16) {
    let v = x.v16(a5::LAYERS) | bits;
    x.set_v16(a5::LAYERS, v);
}
fn layers_and(x: &mut Ctx, mask: u16) {
    let v = x.v16(a5::LAYERS) & mask;
    x.set_v16(a5::LAYERS, v);
}

/// Bloc de tuiles `n` de la grille à `destination` ; rend la case qui suit (A1 de l'original : le bloc suivant
/// d'une suite s'y écrit).
fn tiles(x: &mut Ctx, grille: Grille, n: u16, destination: u32) -> u32 {
    charger_grille(x.b, grille, n, destination).destination
}

/// Commande son 0xff07 de la ligne `ligne` (0xe0a82, sans verrou).
fn sound(x: &mut Ctx, ligne: u16) {
    crate::sound::service(x.b, crate::sound::Service::Ff07 { verrou: 0, ligne });
}

/// Caméra : position et copie.
fn set_camera(x: &mut Ctx, off: u32, v: u16) {
    x.set_v16(off, v);
    x.set_v16(off + a5::COPY, v);
}

// ---------------------------------------------------------------------------------------------
// Sous-programmes du script (0xfa0e6-0xfa294)

/// 0xfa0e6 : fin du script, scène suivante `next` ; la sortie de scène passe à son état suivant, une image cédée.
fn finish(x: &mut Ctx, next_scene: u8) -> Flow {
    x.set_v8(a5::SCENE, next_scene);
    end(x)
}

/// La sortie de scène passe à son état suivant (`+2` du bloc désigné par A0), script remis à zéro, une image cédée.
fn end(x: &mut Ctx) -> Flow {
    let a0 = x.c.a[0];
    let step = rd8(x.b, a0 + 2).wrapping_add(4);
    wr8(x.b, a0 + 2, step);
    set8(x, var::STATE, 0);
    wr8(x.b, a0 + 3, 0);
    layers_or(x, 0xe);
    yield_frames(x, 1, Caller)
}

/// Cède `n` images (`moveq #n,D0 ; jsr/jmp 0xa40`).
fn yield_frames(x: &mut Ctx, n: u32, ret: Ret) -> Flow {
    x.c.d[0] = n;
    goto(x, crate::taches::attendre_puis_revenir, ret)
}

/// Saute à un sous-programme qui cède (`bsr`, ou `bra` qui revient à l'appelant du script), en rangeant la suite
/// s'il y a lieu.
fn goto(x: &mut Ctx, routine: fn(&mut Ctx) -> Flow, ret: Ret) -> Flow {
    if let At(r) = ret {
        x.push_return(r);
    }
    routine(x)
}

/// 0xfa102 : couches affichées, une image, puis fondu d'ouverture de toutes les pages (reprise 0xfa110).
fn show_and_fade_in(x: &mut Ctx, ret: Ret) -> Flow {
    if let At(r) = ret {
        x.push_return(r);
    }
    layers_or(x, 0xe);
    x.wait(1, Etape::FadeInAfterFrame)
}
crate::etapes! {
    /// Étapes (points de reprise) de la machine à états.
    pub enum Etape {
        FadeInAfterFrame = 0xfa110 => |x| fade(x, ALL_PAGES, crate::taches::fondu::montee, Caller),
        SpritesAfterFrame = 0xfa1ca => |x| {
            crate::sprites::construire_68k(x.c, x.b);
            Flow::Done
        },
    }
}

/// Fondu (délai 3) des `pages` vers le noir (0x1460) ou depuis le noir (0x144c) : 0xfa122, 0xfa134, 0xfa146.
fn fade(x: &mut Ctx, pages: u8, routine: fn(&mut Ctx) -> Flow, ret: Ret) -> Flow {
    fondu::DELAI.set(x.b, 3);
    fondu::PAGES.set(x.b, pages);
    goto(x, routine, ret)
}

/// Fondu avec délai et pages donnés, sans attendre sa fin : tâche de fondu `task` (0xfa1fa, 0xfa214).
fn fade_task(x: &mut Ctx, delay: u16, pages: u8, task: u32) {
    fondu::DELAI.set(x.b, delay);
    fondu::PAGES.set(x.b, pages);
    start_task(x, task);
}
fn start_task(x: &mut Ctx, task: u32) {
    crate::kernel::creer_tache(x.b, 0x10, task);
}

/// 0xfa158 : plans 2 et 3 cachés, une image.
fn hide_layers(x: &mut Ctx, ret: Ret) -> Flow {
    layers_and(x, !0xc);
    yield_frames(x, 1, ret)
}
/// 0xfa166 : plans 2 et 3 affichés, une image.
fn show_layers(x: &mut Ctx, ret: Ret) -> Flow {
    layers_or(x, 0xc);
    yield_frames(x, 1, ret)
}
/// 0xfa174 : caméra 1 à l'origine, une image.
fn home_camera_1(x: &mut Ctx, ret: Ret) -> Flow {
    set_camera(x, a5::CAM1_X, 0);
    set_camera(x, a5::CAM1_Y, 0);
    yield_frames(x, 1, ret)
}
/// 0xfa1ac : les deux caméras à l'origine, une image.
fn home_cameras(x: &mut Ctx, ret: Ret) -> Flow {
    for off in [a5::CAM1_X, a5::CAM1_Y, a5::CAM2_X, a5::CAM2_Y] {
        set_camera(x, off, 0);
    }
    yield_frames(x, 1, ret)
}

/// 0xfa1c2 : une image, puis la liste de sprites (reprise 0xfa1ca), retour en `ret`.
fn sprites_next_frame(x: &mut Ctx, ret: impl Into<Suite>) -> Flow {
    x.push_return(ret);
    x.wait(1, Etape::SpritesAfterFrame)
}

/// 0xfa1e2 : mise à jour du monde (A0 et A6 gardés).
fn world(x: &mut Ctx) {
    let (a0, a6) = (x.c.a[0], x.c.a[6]);
    // sonde `rendu` (voir `dispatch::Class::update_sans_suite`)
    crate::diag::registres_morts(x.c, "rendu", crate::diag::REGISTRES_DE_TACHE);
    let flow = crate::game::update_world(x.c, x.b);
    assert!(flow == Flow::Done, "script de transition : un gestionnaire a cédé la main pendant la mise à jour du monde");
    (x.c.a[0], x.c.a[6]) = (a0, a6);
}

/// 0xf7fdc : pools d'objets remis à zéro (A0 gardé).
fn reset_pools(x: &mut Ctx) {
    for pool in crate::objects::pools::POOLS {
        pool.reset(x.b);
    }
}

/// 0xfa22e : palette `n` des pages de fond (0x1a67e), puis noir sur la page 2.
fn dark_palette(x: &mut Ctx, n: u16) {
    crate::palette::charger_page(x.b, 2, n);
    fondu::PAGES.set(x.b, 4);
    crate::palette::no_brightness(x.b);
}

/// 0xfa240 : 0x800 octets à zéro en 0x920000 (registres gardés).
fn clear_920000(x: &mut Ctx) {
    super::screen::clear_920000(x.b);
}

/// 0xfa270 : un joueur en jeu presse-t-il un bouton (`A5+0xc + k`, quatre bits hauts) ?
fn any_button(x: &mut Ctx) -> bool {
    let mut pressed = 0u8;
    for k in (0..4u32).rev() {
        let player = A5 + a5::PLAYERS + k * 0x200;
        if rd8(x.b, player) != 0 {
            pressed |= rd8(x.b, A5 + 0xc + k);
        }
    }
    pressed & 0xf0 != 0
}

// ---------------------------------------------------------------------------------------------
// Objets : messages, objets système, objets SET

/// Le message ouvert (`movea.w 4(A6),A1`).
fn message_obj(x: &mut Ctx) -> Obj {
    Obj(get16(x, var::MESSAGE) as i16 as i32 as u32)
}
/// Le message ouvert est-il terminé (octet `+0x22`) ?
fn message_done(x: &mut Ctx) -> bool {
    let o = message_obj(x);
    rd8(x.b, o.0 + 0x22) != 0
}
/// Réponse choisie dans le message ouvert (octet `+0x23`), qui indexe une table d'octets de la ROM.
fn answer(x: &mut Ctx, table: u32) -> u8 {
    let o = message_obj(x);
    let k = rd8(x.b, o.0 + 0x23);
    x.prog8(table + k as u32)
}

/// Ouvre le message `n` (0xc8312, mémorisé en `+4`) ; vrai si l'objet a été créé.
fn try_message(x: &mut Ctx, n: u8) -> bool {
    let Some(o) = crate::families::creators::scene_set(x.b, n) else { return false };
    store_message(x, o);
    true
}
fn store_message(x: &mut Ctx, o: Obj) {
    set16(x, var::MESSAGE, o.0 as u16);
}
/// Message `n` dont l'échec n'est pas testé. Glitch d'origine : l'A1 laissé par l'allocation est mémorisé.
fn message_untested(x: &mut Ctx, n: u8) -> Obj {
    crate::families::creators::scene_set_ou_reste(x.b, n)
}
/// Boucle `moveq #n,D0 ; jsr 0xc8312 ; bcc ; moveq #1,D0 ; jsr 0xa40 ; bra` : vrai si le message est ouvert ;
/// sinon la tâche cède une image et reprendra en `retry`.
fn message(x: &mut Ctx, n: u8, retry: impl Into<Suite>) -> bool {
    try_message(x, n) || retry_next_frame(x, retry)
}
fn retry_next_frame(x: &mut Ctx, retry: impl Into<Suite>) -> bool {
    yield_frames(x, 1, At(retry.into()));
    false
}

/// Objet système de variante `n` (0xddeaa) qui reçoit l'adresse des variables ; sinon cède et reprendra en `retry`.
fn system_object(x: &mut Ctx, n: u8, retry: impl Into<Suite>) -> Option<Obj> {
    let Ok(o) = crate::effects_aids::decor_6(x.b, n) else {
        retry_next_frame(x, retry);
        return None;
    };
    Some(attach(x, o))
}
/// Objet système de variante `n` dont l'échec n'est pas testé. Glitch d'origine : l'A1 laissé par l'allocation reçoit
/// l'adresse des variables.
fn system_object_untested(x: &mut Ctx, n: u8) -> Obj {
    let o = crate::effects_aids::decor_6(x.b, n).unwrap_or_else(|a1| a1);
    attach(x, o)
}
/// Objet SET de type `n` (0xce00a) attaché aux variables ; `None` si le pool est plein.
fn try_set_object(x: &mut Ctx, n: u8) -> Option<Obj> {
    let o = crate::families::creators::set_15_pur(x.b, n).ok()?;
    Some(attach(x, o))
}
fn set_object(x: &mut Ctx, n: u8, retry: impl Into<Suite>) -> bool {
    try_set_object(x, n).is_some() || retry_next_frame(x, retry)
}
/// `move.l A6,0x4c(A1)` : l'objet créé connaît les variables du script (A6 de l'original).
fn attach(x: &mut Ctx, o: Obj) -> Obj {
    wr32(x.b, o.0 + 0x4c, VARS);
    o
}

/// Texte défilant de la région (table de pointeurs `table` relative au PC, indexée par `A5+0x72`) : en-tête de deux
/// mots, puis le texte.
fn scroll_text(x: &mut Ctx, table: u32) {
    let k = x.v16(a5::REGION).wrapping_sub(2).wrapping_mul(2);
    let mut a4 = crate::mem::prog32(x.b, table + (k as i16 as i32 as u32));
    for off in [a5::SCROLL_TEXT + 4, a5::SCROLL_TEXT + 6] {
        let w = rd16(x.b, a4);
        x.set_v16(off, w);
        a4 += 2;
    }
    x.set_v32(a5::SCROLL_TEXT, a4);
}

/// Copie de 0x400 octets de palettes (0xf9e38, `movem.l` par huit longs).
fn copy_palettes(x: &mut Ctx, from: u32, to: u32) {
    for i in (0..0x400).step_by(4) {
        let v = rd32(x.b, from + i);
        wr32(x.b, to + i, v);
    }
}

// ---------------------------------------------------------------------------------------------
// Aiguillage par scène

/// Script de la scène ; rend la main à l'appelant (`Flow::Done`) ou cède.
pub fn run(x: &mut Ctx, scene: u32) -> Flow {
    match scene {
        1 => early::scene_1(x),
        2 => early::scene_2(x),
        4 | 10 => early::scene_4(x),
        5 | 14 | 20 => early::scene_5(x),
        11 => early::scene_11(x),
        13 => early::scene_13(x),
        27 => late::scene_27(x),
        31 => late::scene_31(x),
        33 => late::scene_33(x),
        32 | 34 | 35 | 67 => late::scene_32(x),
        38 | 40 | 42 | 44 => late::scene_38(x),
        46 => late::scene_46(x),
        69 => late::finale(x),
        // pas de script : la scène suivante est l'octet bas de D0 de l'aiguillage (deux fois la scène)
        _ => finish(x, (2 * scene) as u8),
    }
}

const COMMON: &[Native] = &[];

pub fn natives() -> impl Iterator<Item = &'static Native> {
    COMMON.iter().chain(early::NATIVES).chain(late::NATIVES)
}
