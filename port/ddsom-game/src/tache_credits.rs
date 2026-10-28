//! Tâche des crédits (C1, `0x13c0a`) : monnayeurs, écran « crédits », départs de partie et reprise
//! (continue). C'est une des tâches permanentes de l'ordonnanceur (`kernel.rs`).
//!
//! Elle tourne en boucle : chaque tour lit les boutons de départ (`0x141e0`), gère l'état des
//! crédits (`0x13c2c`), verrouille les compteurs de pièces (`0x14290`) et lit les boutons de
//! continue (`0x1436e`), puis cède une image (`0xb64`). Tant que l'interrupteur TEST n'est pas
//! actionné (bit 1 de `IN2+1`, actif bas, test de `0x14d74`), elle continue ; sinon elle se démonte (`0x14d8c`) :
//! écran remis à zéro, tâche d'interface (`0x29162`) créée, tâche terminée.
//!
//! ## État des crédits (`0x13c2c`)
//!
//! Selon les crédits et les départs demandés (enregistrements de monnayeurs en `A5+0x44e`, voir
//! `credits.rs`), elle prépare l'écran des crédits au premier crédit (`init`, qui efface l'écran et
//! cède une image), l'affiche, puis appelle `depart` (`0x140b4`) pour chaque joueur : si le bouton de
//! départ du joueur est pressé et qu'il y a assez de crédits, le joueur entre dans la partie (tâche de
//! partie `0x1c45a` créée au premier départ). Quand un joueur a démarré (`A5+0x31` ≠ 0), la tâche
//! de partie est lancée à la place de l'écran des crédits (`fin`).
//!
//! Les cessions d'origine (`0xc32` efface l'écran en cédant, `0x1460` fond au noir, `0x1436e`, `0xb64`)
//! coupent le code en morceaux : chacun est une étape de la machine à états de la tâche ([`Etape`]), à l'adresse
//! de retour de l'original.

use crate::modes::{Flow, Sequence};
use crate::mem::{rd16, rd8, set_low_byte, set_low_word, wr16, wr32, wr8, A5};
use crate::scene::Ctx;
use crate::taches;
use crate::objects::{pools, Obj};

/// Sous-routines d'origine.
mod ext {
    pub const VIDEO_ON: u32 = 0xbc6;
    pub const PAGE_PALETTE: u32 = 0x1a3d4;
    pub const PALETTE_SCENE: u32 = 0x1a60a;
    pub const PALETTE_BACK: u32 = 0x1a67e;
    pub const PALETTE_FRONT: u32 = 0x1a8b2;
    pub const PALETTE_RESET: u32 = 0x1a296;
    pub const SPRITES: u32 = 0x1de96;
    pub const START_BUTTONS: u32 = 0x141e0;
    pub const LOCK_COUNTERS: u32 = 0x14290;
    pub const LOCK: u32 = 0xb96;
    pub const UNLOCK: u32 = 0xbac;
    pub const DISPLAY_CREDITS: u32 = 0x148be;
    pub const DISPLAY_COUNTS: u32 = 0x14652;
    pub const PLAYERS_INIT: u32 = 0x2d9e;
    pub const PLAYERS_RESET: u32 = 0x1de7c;
    pub const PLAYERS_PANELS: u32 = 0x21bc;
    pub const DISPATCH_SOBJ: u32 = 0x39f82;
    pub const TILES_16: u32 = 0x260c;
    pub const TILES_BLOCK: u32 = 0x26d4;
    pub const TILES_SCROLL3: u32 = 0x2766;
    pub const SOUND_E1060: u32 = 0xe1060;
    pub const SOUND_E18B8: u32 = 0xe18b8;
    pub const SOUND_START: u32 = 0xe181c;
    pub const ACCOUNTING: u32 = 0x2e4ec;
}

/// Entrée de la tâche de partie, et de la tâche d'interface des joueurs.
const GAME_TASK: u32 = 0x1c45a;
const UI_TASK: u32 = 0x29162;
/// Textes affichés par l'écran des crédits (adresses de pointeurs ou de chaînes en ROM).
const TEXT_CREDITS: u32 = 0x14ea2;
const TEXT_TABLE: u32 = 0x15060;

/// Registres matériel.
const IN2: u32 = 0x804020;
const IN2_SERVICE: u32 = 0x804021;
const CPS_VIDEO_A: u32 = 0x804122;
const CPS_VIDEO_B: u32 = 0x804166;
const CPS_A_OBJ: u32 = 0x804100;
const CPS_A_SCROLL: u32 = 0x80410c;
const OUTPUT_4: u32 = 0x400004;
const VIDEO_ON: u16 = 0x12ce;
const OUTPUT_ON: u16 = 0x1355;

/// Variables de la RAM de travail (décalages depuis A5).
mod var {
    /// Partie en cours.
    pub const GAME: u32 = 0x30;
    /// Joueurs qui ont démarré (un bit par joueur).
    pub const STARTED: u32 = 0x31;
    /// Mode « continue » : les crédits servent à reprendre une partie.
    pub const CONTINUE: u32 = 0x25;
    pub const SEPARATE: u32 = 0x22;
    pub const COIN_COUNT: u32 = 0x21;
    pub const SHARED: u32 = 0x20;
    /// Premier monnayeur, et pas.
    pub const COIN_1: u32 = 0x44e;
    /// Écran des crédits déjà préparé (octet +6 du premier monnayeur).
    pub const SCREEN_READY: u32 = COIN_1 + 6;
    /// Drapeau de bruit de pièce (A5 - 0x713c).
    pub const COIN_SOUND: u32 = 0xffff_8ec4;
    /// Défilements demandés (voir `vblank.rs`).
    pub const SCROLLS: [u32; 6] = [0x4b6, 0x4ba, 0x506, 0x50a, 0x556, 0x55a];
    pub const FLAGS: u32 = 0x12;
    pub const VIDEO_B: u32 = 0x14;
    pub const SCROLL_MODE: u32 = 0x53c;
    pub const FADE_PAGES: u32 = 0x48e;
    pub const FADE_STEP: u32 = 0x490;
    pub const GAME_LANGUAGE: u32 = 0x70;
    /// Drapeau (A5 - 0x7155) et octet (A5 - 0x5b8e) remis à zéro / posé par l'écran.
    pub const CLEAR_FLAG: u32 = 0xffff_8eab;
    pub const SCREEN_FLAG: u32 = 0xffff_a472;
    pub const FRAME_SKIP: u32 = 0x5bc;
    pub const FRAME_SKIP_2: u32 = 0x5ba;
    pub const TILES_OFF: u32 = 0x33;
}

/// Adresses de retour de l'original après chaque appel qui peut céder la main.
mod at {
    /// Retours des `bsr 0x143a6` de `0x1436e`, et reprise de l'impulsion après sa première cession.
    pub const LOCKOUT: [u32; 3] = [0x14376, 0x14386, 0x14398];
    pub const PULSE_AFTER_WAIT: u32 = 0x143c8;
    /// Après `bsr 0x13c2c`, `0x14290`, `0x1436e` de la boucle, et après la cession.
    pub const AFTER_STATE: u32 = 0x13c18;
    pub const AFTER_CONTINUE: u32 = 0x13c20;
    pub const AFTER_WAIT: u32 = 0x13c2a;
    /// Après l'effacement de l'écran d'initialisation.
    pub const AFTER_INIT_CLEAR: u32 = 0x13cc6;
    /// Retours des quatre appels de départ de chaque séquence (le dernier de B est la fin, `0x13fa8`).
    pub const SEQ_A: [u32; 4] = [0x13eb6, 0x13ec8, 0x13ee2, 0x13efe];
    pub const SEQ_B: [u32; 4] = [0x13f72, 0x13f84, 0x13f96, 0x13fa8];
    pub const SEQ_C: [u32; 4] = [0x13f26, 0x13f38, 0x13f4a, 0x13f5c];
    /// Après le fondu et l'effacement de la reprise de partie.
    pub const AFTER_FADE: u32 = 0x14066;
    pub const AFTER_FADE_CLEAR: u32 = 0x1406c;
    /// Reprise de `0x140b4` après l'effacement de l'écran.
    pub const START_AFTER_CLEAR: u32 = 0x14174;
    /// Démontage : après l'effacement.
    pub const SHUTDOWN_AFTER_CLEAR: u32 = 0x14e24;
}

fn a5(off: u32) -> u32 {
    A5.wrapping_add(off)
}

/// Octet de la RAM de travail à un décalage signé (`A5 - n` est donné en 32 bits).
fn v8(x: &mut Ctx, off: u32) -> u8 {
    rd8(x.b, a5(off))
}
fn set8(x: &mut Ctx, off: u32, v: u8) {
    wr8(x.b, a5(off), v)
}
fn set16(x: &mut Ctx, off: u32, v: u16) {
    wr16(x.b, a5(off), v)
}

/// Tue les tâches de ces décalages dans la table (`0xa14`, D0 = décalage).
fn kill(x: &mut Ctx, slots: &[u16]) {
    for &s in slots {
        set_low_word(&mut x.c.d[0], s);
        crate::kernel::tuer_tache(x.b, s as u32);
    }
}

fn clear_scrolls(x: &mut Ctx) {
    for off in var::SCROLLS {
        set16(x, off, 0);
    }
}

/// `D0 = VIDEO_ON ; A5+0x14 = 0 ; 0xbc6` : remet les plans en route.
fn video_on(x: &mut Ctx) {
    set_low_word(&mut x.c.d[0], VIDEO_ON);
    set16(x, var::VIDEO_B, 0);
    x.call(ext::VIDEO_ON);
}

// ---------------------------------------------------------------------------------------------
// La boucle de la tâche

/// Étapes de la tâche : son entrée, et chaque retour d'un appel qui peut céder la main.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Etape {
    /// `0x13c0a`.
    Entree,
    /// Après `bsr 0x13c2c` (état des crédits), `0x1436e` (verrous), et après la cession du tour.
    AfterState,
    AfterContinue,
    AfterWait,
    /// Démontage, après l'effacement de l'écran.
    ShutdownAfterClear,
    /// Première préparation de l'écran, après l'effacement.
    AfterInitClear,
    /// Retour du `k`-ième appel de départ d'une séquence (le dernier de la séquence B tombe sur `0x13fa8`, la fin).
    Sequence(Seq, usize),
    /// Reprise de partie : après le fondu au noir, puis après l'effacement.
    AfterFade,
    AfterFadeClear,
    /// Départ d'un joueur en mode « continue », après l'effacement de l'écran.
    StartAfterClear,
    /// Impulsion de verrouillage après sa première cession ; `d0` est le D0 que l'original garde sur la pile
    /// (`move.l D0,-(SP)`).
    PulseAfterWait { d0: u32 },
    /// Retour de l'impulsion du monnayeur `k - 1` : celle du monnayeur `k` (1 à 3).
    Lock(usize),
}

impl Etape {
    /// Adresse que l'original reprend à cette étape.
    pub fn adresse(self) -> u32 {
        match self {
            Etape::Entree => 0x13c0a,
            Etape::AfterState => at::AFTER_STATE,
            Etape::AfterContinue => at::AFTER_CONTINUE,
            Etape::AfterWait => at::AFTER_WAIT,
            Etape::ShutdownAfterClear => at::SHUTDOWN_AFTER_CLEAR,
            Etape::AfterInitClear => at::AFTER_INIT_CLEAR,
            Etape::Sequence(seq, k) => seq.returns()[k],
            Etape::AfterFade => at::AFTER_FADE,
            Etape::AfterFadeClear => at::AFTER_FADE_CLEAR,
            Etape::StartAfterClear => at::START_AFTER_CLEAR,
            Etape::PulseAfterWait { .. } => at::PULSE_AFTER_WAIT,
            Etape::Lock(k) => at::LOCKOUT[k - 1],
        }
    }

    pub fn pas(self, x: &mut Ctx) -> Flow {
        match self {
            Etape::Entree | Etape::AfterWait => tour(x),
            Etape::AfterState => after_state(x),
            Etape::AfterContinue => after_continue(x),
            Etape::ShutdownAfterClear => shutdown_after_clear(x),
            Etape::AfterInitClear => after_init_clear(x),
            Etape::Sequence(seq, k) => sequence(x, seq, k + 1),
            Etape::AfterFade => x.sous_programme(crate::video::ecran_noir::debut, Etape::AfterFadeClear),
            Etape::AfterFadeClear => after_fade_clear(x),
            Etape::StartAfterClear => start_after_clear(x),
            Etape::PulseAfterWait { d0 } => pulse_after_wait(x, d0),
            Etape::Lock(k) => lockouts(x, k),
        }
    }
}

/// `0x13c10` : un tour.
fn tour(x: &mut Ctx) -> Flow {
    x.call(ext::START_BUTTONS);
    x.push_return(Etape::AfterState);
    credits(x)
}

fn after_state(x: &mut Ctx) -> Flow {
    x.call(ext::LOCK_COUNTERS);
    x.push_return(Etape::AfterContinue);
    lockouts(x, 0)
}

fn after_continue(x: &mut Ctx) -> Flow {
    // 0x14d74 : la tâche continue tant que l'interrupteur TEST n'est pas actionné (le drapeau 0xc8 de la ROM est nul)
    if x.rd8(IN2_SERVICE) & 2 != 0 {
        taches::une_image(x, Etape::AfterWait)
    } else {
        shutdown(x)
    }
}

/// `0x14d8c` : démontage de la tâche.
fn shutdown(x: &mut Ctx) -> Flow {
    kill(x, &[0, 0x10, 0x30, 0xb0, 0xd0]);
    wr16(x.b, CPS_VIDEO_A, 0xe);
    wr16(x.b, CPS_VIDEO_B, 0x12ce);
    set8(x, var::FRAME_SKIP, 1);
    wr16(x.b, CPS_A_OBJ, 0x7080);
    set16(x, var::FRAME_SKIP_2, 0x7080);
    for k in 0..3 {
        wr32(x.b, CPS_A_SCROLL + 4 * k, 0);
    }
    for off in var::SCROLLS {
        wr32(x.b, a5(off), 0);
    }
    set8(x, var::TILES_OFF, 0);
    let flags = rd16(x.b, a5(var::FLAGS)) & !1;
    set16(x, var::FLAGS, flags);
    set8(x, var::SCROLL_MODE, 0);
    set8(x, var::CLEAR_FLAG, 0);
    x.sous_programme(crate::video::ecran_noir::debut, Etape::ShutdownAfterClear)
}

/// Fin du démontage : la tâche du menu TEST prend l'emplacement 0, la tâche se termine.
fn shutdown_after_clear(x: &mut Ctx) -> Flow {
    x.call(ext::PALETTE_RESET);
    crate::sound::service(x.b, crate::sound::Service::ArretGeneral);
    x.c.a[0] = UI_TASK;
    set_low_word(&mut x.c.d[0], 0);
    crate::kernel::creer_tache(x.b, 0, UI_TASK);
    taches::terminer(x)
}

// ---------------------------------------------------------------------------------------------
// État des crédits (0x13c2c)

/// `0x13c2c`.
fn credits(x: &mut Ctx) -> Flow {
    if v8(x, var::GAME) != 0 {
        return Flow::Done;
    }
    x.c.a[1] = a5(var::COIN_1);
    if v8(x, var::CONTINUE) == 0 {
        // des crédits ou des pièces quelque part ?
        let somme = [0x44f, 0x45f, 0x46f, 0x47f, 0x450, 0x460, 0x470, 0x480].iter().fold(0u8, |s, &o| s.wrapping_add(v8(x, o)));
        if somme == 0 {
            return Flow::Done;
        }
        if v8(x, var::SCREEN_READY) == 0 {
            return init(x);
        }
        return display(x);
    }
    starts(x)
}

/// `0x13c6c` : première fois : prépare l'écran des crédits, après l'effacement de l'écran.
fn init(x: &mut Ctx) -> Flow {
    set8(x, var::SCREEN_READY, 1);
    kill(x, &[0, 0x10, 0x30, 0xd0, 0xf0]);
    set8(x, var::SCROLL_MODE, 0);
    clear_scrolls(x);
    x.sous_programme(crate::video::ecran_noir::debut, Etape::AfterInitClear)
}

fn after_init_clear(x: &mut Ctx) -> Flow {
    let flags = rd16(x.b, a5(var::FLAGS)) & !1;
    set16(x, var::FLAGS, flags);
    set8(x, var::CLEAR_FLAG, 0);
    set8(x, var::SCREEN_FLAG, 1);
    crate::taches::partie::effacer_variables(x.c, x.b);
    x.call(ext::PLAYERS_INIT);
    x.call(ext::PLAYERS_RESET);
    wr16(x.b, OUTPUT_4, OUTPUT_ON);
    video_on(x);
    x.c.d[0] = 1;
    x.call(ext::PAGE_PALETTE);
    x.call(ext::PLAYERS_PANELS);
    // cinq objets système : (variante, X, Z)
    for (variant, px, pz) in [(0xe7, 0x70, 0xd0), (0xe8, 0x70, 0xd0), (0xe4, 0x20, 0xb0), (0xe5, 0x00, 0x70), (0xe6, 0x13c, 0x60)] {
        // Glitch d'origine : l'échec de l'allocation n'est pas testé (voir `Pool::alloc_unchecked`).
        let Obj(o) = pools::SOBJ.alloc_unchecked_68k(x.c, x.b);
        wr8(x.b, o, 1);
        wr8(x.b, o + 2, 0);
        wr8(x.b, o + 3, variant);
        wr16(x.b, o + 8, px);
        wr16(x.b, o + 0xc, pz);
    }
    x.call(ext::DISPATCH_SOBJ);
    x.c.d[0] = 5;
    x.c.a[1] = 0x912000;
    x.call(ext::TILES_16);
    x.c.d[0] = 6;
    x.call(ext::TILES_16);
    set_low_word(&mut x.c.d[0], 0x201);
    x.c.a[1] = 0x917000;
    x.call(ext::TILES_BLOCK);
    set_low_word(&mut x.c.d[0], 0x202);
    x.call(ext::TILES_BLOCK);
    set_low_word(&mut x.c.d[0], 8);
    x.c.a[1] = 0x91b800;
    x.call(ext::TILES_SCROLL3);
    set_low_word(&mut x.c.d[0], 8);
    x.call(ext::TILES_SCROLL3);
    x.c.d[0] = 2;
    x.call(ext::PALETTE_SCENE);
    x.c.d[0] = 0x55;
    x.call(ext::PALETTE_BACK);
    x.c.d[0] = 0x55;
    x.call(ext::PALETTE_FRONT);
    set8(x, var::FADE_PAGES, 0xf);
    crate::palette::full_brightness(x.b);
    video_on(x);
    crate::sound::service(x.b, crate::sound::Service::ArretGeneral);
    crate::sound::jouer_global(x.b, ext::SOUND_E1060);
    crate::sound::jouer_global(x.b, ext::SOUND_E18B8);
    set8(x, var::COIN_SOUND, 1);
    display(x)
}

/// `0x13e46` : affiche l'écran des crédits (compteurs, textes, sprites), puis traite les départs.
fn display(x: &mut Ctx) -> Flow {
    x.c.a[1] = a5(var::COIN_1);
    x.call(ext::DISPLAY_CREDITS);
    x.call(ext::DISPLAY_COUNTS);
    crate::text::print(x.b, TEXT_CREDITS);
    let d0 = (rd16(x.b, a5(var::GAME_LANGUAGE)) as u32 * 2) & 0xffff;
    set_low_word(&mut x.c.d[0], d0 as u16);
    let texte = x.rd32(TEXT_TABLE + d0);
    crate::text::print(x.b, texte);
    x.call(ext::SPRITES);
    starts(x)
}

// ---------------------------------------------------------------------------------------------
// Impulsions de verrouillage des compteurs (0x1436e)

/// Décalages des quatre monnayeurs.
const COINS: [u32; 4] = [0x44e, 0x45e, 0x46e, 0x47e];

/// `0x1436e` : pour chaque monnayeur dont le compteur `+3` n'est pas nul, une impulsion de
/// verrouillage (voir `pulse`), de plus en plus loin selon le nombre de monnayeurs (`A5+0x21`). Chaque
/// impulsion cède la main : sa suite est l'étape `Lock(k + 1)`.
fn lockouts(x: &mut Ctx, k: usize) -> Flow {
    // le monnayeur k n'existe que si `A5+0x21` ≥ k
    if k > 0 && v8(x, var::COIN_COUNT) as usize == k - 1 {
        return Flow::Done;
    }
    x.c.a[0] = a5(COINS[k]);
    if k < 3 {
        x.push_return(Etape::Lock(k + 1));
    }
    pulse(x)
}

/// `0x143a6` : si le monnayeur A0 a une pièce comptée (`+3`), la décompte, verrouille le compteur de
/// pièces 16 images puis le déverrouille et attend encore 16 images.
fn pulse(x: &mut Ctx) -> Flow {
    set_low_word(&mut x.c.d[0], 1);
    let counter = rd8(x.b, x.c.a[0] + 3);
    if counter == 0 {
        return Flow::Done;
    }
    wr8(x.b, x.c.a[0] + 3, counter - 1);
    // D0 est gardé pendant la cession (`move.l D0,-(SP)` de l'original) : dans l'étape de reprise
    let d0 = x.c.d[0];
    x.call(ext::LOCK);
    set_low_word(&mut x.c.d[0], 0x10);
    crate::kernel::attendre(x.c, x.b, Etape::PulseAfterWait { d0 });
    Flow::Yield
}

/// `0x143c8` : déverrouille, puis la seconde attente (`jmp 0xa40`) : la tâche reprendra à la suite de l'appelant.
fn pulse_after_wait(x: &mut Ctx, d0: u32) -> Flow {
    x.c.d[0] = d0;
    x.call(ext::UNLOCK);
    set_low_word(&mut x.c.d[0], 0x10);
    let suite = crate::taches::table::depiler_retour(x.c, x.b);
    crate::kernel::attendre(x.c, x.b, suite);
    Flow::Yield
}

// ---------------------------------------------------------------------------------------------
// Départs de partie (0x13e7a)

/// Un appel de `depart` : monnayeur (décalage de A1), masque du bouton (D1), joueur (D2), enregistrement
/// du joueur (décalage de A3).
type Start = (u32, u8, u32, u32);

const SEQ_A: [Start; 4] = [(0x44e, 1, 0, 0x5de), (0x44e, 2, 1, 0x7de), (0x44e, 4, 2, 0x9de), (0x44e, 8, 3, 0xbde)];
const SEQ_B: [Start; 4] = [(0x44e, 1, 0, 0x5de), (0x44e, 2, 1, 0x7de), (0x45e, 4, 2, 0x9de), (0x45e, 8, 3, 0xbde)];
const SEQ_C: [Start; 4] = [(0x44e, 1, 0, 0x5de), (0x45e, 2, 1, 0x7de), (0x46e, 4, 2, 0x9de), (0x47e, 8, 3, 0xbde)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seq {
    A,
    B,
    C,
}

impl Seq {
    fn calls(self) -> &'static [Start; 4] {
        match self {
            Seq::A => &SEQ_A,
            Seq::B => &SEQ_B,
            Seq::C => &SEQ_C,
        }
    }
    fn returns(self) -> &'static [u32; 4] {
        match self {
            Seq::A => &at::SEQ_A,
            Seq::B => &at::SEQ_B,
            Seq::C => &at::SEQ_C,
        }
    }
}

/// `0x13e7a` : choisit la séquence d'appels de départ.
fn starts(x: &mut Ctx) -> Flow {
    x.c.a[1] = a5(var::COIN_1);
    if v8(x, var::SEPARATE) != 0 {
        let seq = if v8(x, var::COIN_COUNT) == 1 && v8(x, var::SHARED) != 0 { Seq::B } else { Seq::C };
        return sequence(x, seq, 0);
    }
    if v8(x, var::CONTINUE) == 0 {
        let needed = if v8(x, var::COIN_1 + 9) != 0 { 2 } else { 1 };
        if v8(x, var::COIN_1 + 2) < needed {
            return end(x);
        }
    }
    sequence(x, Seq::A, 0)
}

/// Exécute les appels de départ de la séquence à partir du `k`-ième. Chaque appel revient à l'étape
/// `Sequence(seq, k)`, qui reprend à l'appel suivant.
fn sequence(x: &mut Ctx, seq: Seq, k: usize) -> Flow {
    if k == 4 {
        return end(x);
    }
    if let Seq::A = seq {
        // le troisième joueur n'existe que si A5+0x20 ≠ 0, le quatrième s'il est ≥ 2
        let shared = v8(x, var::SHARED) as i8;
        if (k == 2 && shared == 0) || (k == 3 && shared <= 1) {
            return end(x);
        }
    }
    let (a1, mask, player, a3) = seq.calls()[k];
    x.push_return(Etape::Sequence(seq, k));
    start_player(x, a5(a1), mask, player, a5(a3))
}

/// `0x13fa8` : si un joueur a démarré, lance la partie.
fn end(x: &mut Ctx) -> Flow {
    if v8(x, var::GAME) != 0 || v8(x, var::STARTED) == 0 {
        return Flow::Done;
    }
    if v8(x, var::CONTINUE) == 0 {
        kill(x, &[0, 0x10]);
        set8(x, var::COIN_SOUND, 0);
        set8(x, var::GAME, 1);
        set8(x, var::COIN_1 + 6, 0);
        crate::text::erase(x.b, TEXT_CREDITS);
        let d0 = (rd16(x.b, a5(var::GAME_LANGUAGE)) as u32 * 2) & 0xffff;
        set_low_word(&mut x.c.d[0], d0 as u16);
        let texte = x.rd32(TEXT_TABLE + d0);
        crate::text::erase(x.b, texte);
        return create_game_task(x);
    }
    // reprise de partie : l'écran de continue est démonté, fondu au noir et effacé
    x.call(ext::PLAYERS_RESET);
    x.call(ext::PLAYERS_PANELS);
    let flags = rd16(x.b, a5(var::FLAGS)) & !1;
    set16(x, var::FLAGS, flags);
    set8(x, var::SCROLL_MODE, 0);
    set8(x, var::CLEAR_FLAG, 0);
    kill(x, &[0, 0x10, 0xd0]);
    set8(x, var::FADE_PAGES, 0xf);
    crate::palette::full_brightness(x.b);
    set16(x, var::FADE_STEP, 2);
    set8(x, var::FADE_PAGES, 0xf);
    x.sous_programme(crate::taches::fondu::descente, Etape::AfterFade)
}

fn after_fade_clear(x: &mut Ctx) -> Flow {
    wr16(x.b, OUTPUT_4, OUTPUT_ON);
    video_on(x);
    set8(x, var::GAME, 1);
    set8(x, var::COIN_1 + 6, 0);
    clear_scrolls(x);
    create_game_task(x)
}

/// `jmp 0x9b6` avec A0 = tâche de partie, D0 = emplacement 0 : la tâche de partie est créée, et l'on revient à
/// l'appelant de `0x13c2c`.
fn create_game_task(x: &mut Ctx) -> Flow {
    set_low_word(&mut x.c.d[0], 0);
    x.c.a[0] = GAME_TASK;
    crate::kernel::creer_tache(x.b, 0, GAME_TASK);
    Flow::Done
}

// ---------------------------------------------------------------------------------------------
// Départ d'un joueur (0x140b4)

/// `0x140b4` : le joueur dont le bouton de départ est `mask` (IN2) entre dans la partie. `a1` est
/// son monnayeur, `a3` son enregistrement de joueur, `player` son numéro.
fn start_player(x: &mut Ctx, a1: u32, mask: u8, player: u32, a3: u32) -> Flow {
    x.c.a[1] = a1;
    set_low_byte(&mut x.c.d[1], mask);
    x.c.d[2] = player;
    x.c.a[3] = a3;
    let button = x.rd8(IN2) & mask;
    if v8(x, var::CONTINUE) == 0 {
        let cost = if rd8(x.b, a1 + 9) != 0 { 2 } else { 1 };
        x.c.d[3] = cost;
        if rd8(x.b, a1 + 2) < cost as u8 || button != 0 {
            return Flow::Done;
        }
        return join(x);
    }
    x.c.d[3] = 0;
    if button != 0 {
        return Flow::Done;
    }
    if rd8(x.b, a1 + 6) == 0 {
        wr8(x.b, a1 + 6, 1);
        kill(x, &[0, 0x10, 0x30, 0xd0, 0xf0]);
        clear_scrolls(x);
        return x.sous_programme(crate::video::ecran_noir::debut, Etape::StartAfterClear);
    }
    join(x)
}

fn start_after_clear(x: &mut Ctx) -> Flow {
    wr16(x.b, OUTPUT_4, OUTPUT_ON);
    video_on(x);
    let flags = rd16(x.b, a5(var::FLAGS)) & !1;
    set16(x, var::FLAGS, flags);
    crate::taches::partie::effacer_variables(x.c, x.b);
    x.call(ext::PLAYERS_INIT);
    set8(x, var::SCROLL_MODE, 0);
    x.call(ext::PLAYERS_RESET);
    join(x)
}

/// `0x141a6` : le joueur démarre : bruit, crédit débité (`D3`), enregistrement du joueur créé,
/// comptabilité (`jmp`, qui revient à l'appelant).
fn join(x: &mut Ctx) -> Flow {
    crate::sound::jouer_global(x.b, ext::SOUND_START);
    crate::sound::service(x.b, crate::sound::Service::Ff07 { verrou: 0, ligne: 4 });
    let (a1, a3, player, cost) = (x.c.a[1], x.c.a[3], x.c.d[2], x.c.d[3] as u8);
    let credits = rd8(x.b, a1 + 2).wrapping_sub(cost);
    wr8(x.b, a1 + 2, credits);
    crate::taches::partie::effacer_variables(x.c, x.b);
    wr8(x.b, a3, 1);
    wr8(x.b, a3 + 2, player as u8);
    let started = rd8(x.b, a5(var::STARTED)) | 1 << (player & 7);
    wr8(x.b, a5(var::STARTED), started);
    x.call(ext::ACCOUNTING);
    Flow::Done
}


/// Sous-programme d'origine réécrit dans le corps de la tâche, présenté comme une native pour le fuzz différentiel
/// (`examples/fuzz_couv`) ; le jeu ne l'appelle pas par son adresse. S'il cède la main, l'essai est écarté (panique).
pub const NATIVES_DE_TEST: &[crate::Native] = &[crate::Native {
    // 0x13c2c : état des crédits (démarrage, reprise, ralliement des quatre emplacements)
    entry: 0x13c2c,
    name: "credits_etat",
    run: |c, b| {
        if let Flow::Yield = credits(&mut Ctx::new(c, b)) {
            panic!("credits_etat : cède la main (hors du cadre du fuzz)");
        }
        crate::mem::rts(c, b);
    },
    touches_sound: false,
    outputs: 0,
}];
