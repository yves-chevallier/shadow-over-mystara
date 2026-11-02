//! Pièces et crédits (C1) : lecture des monnayeurs dans le VBLANK (0x143da), boutons de départ
//! (0x141e0) et verrouillage des compteurs de pièces (0x14290).
//!
//! Le port `IN2` (0x804020) porte les lignes des monnayeurs (bits 4 à 7 : pièces 1 à 4) ; `IN2+1`
//! (0x804021) les boutons de service. Jusqu'à quatre monnayeurs, chacun avec un enregistrement de
//! 16 octets en `A5+0x44e + 0x10 k` :
//!
//! | Octet | Rôle |
//! |---|---|
//! | +0 | antirebond : images consécutives où la ligne est restée basse (plafonné à 50) |
//! | +1 | pièces insérées pas encore converties en crédit |
//! | +2 | crédits (plafonnés à 9) |
//! | +3 | total des pièces insérées |
//! | +7 | « départ pris en compte » |
//!
//! Compteurs communs (`SEPARATE_MODE` nul) : tout se passe dans l'enregistrement du premier monnayeur,
//! les autres ne servent que d'antirebond. Compteurs séparés : chaque monnayeur a les siens.
//!
//! Les routines lisent, décident et écrivent comme l'original ; le bruit de pièce, la comptabilité EEPROM et les
//! verrous (0xb96/0xbac) sont appelés par leurs fonctions Rust.

use cps2::Cps2Bus;

use crate::mem::{rd16, rd8, wr8, A5};
use crate::Native;

/// Port des monnayeurs et port des boutons de service.
const PORT_PIECES: u32 = 0x804020;
const PORT_SERVICE: u32 = 0x804021;

/// Réglages et états, relatifs à A5.
mod ram {
    /// Pièces nécessaires pour un crédit.
    pub const COINS_PER_CREDIT: u32 = 0x1e;
    /// Crédits accordés quand le seuil est atteint.
    pub const CREDITS_PER_THRESHOLD: u32 = 0x1f;
    /// Mode de partage des pièces (≠ 0 : le 2e monnayeur lit la ligne 3 ; lockout du 2e).
    pub const SHARED_MODE: u32 = 0x20;
    /// Nombre de monnayeurs moins un.
    pub const COIN_SLOTS: u32 = 0x21;
    /// Compteurs séparés par monnayeur (0 : compteurs communs).
    pub const SEPARATE_MODE: u32 = 0x22;
    /// Jeu gratuit : pas de verrouillage.
    pub const FREE: u32 = 0x25;
    /// Partie en cours.
    pub const GAME: u32 = 0x30;
    /// Mot non nul pendant le menu de test : rien n'est lu.
    pub const MENU_TEST: u32 = 0x7fcc;
    /// Drapeau qui, avec `GAME`, autorise le bruit de pièce (A5 - 0x713c).
    pub const COIN_SOUND: u32 = 0xffff_8ec4;
    /// Premier enregistrement monnayeur, et pas.
    pub const COIN_SLOT_1: u32 = 0x44e;
    pub const STEP: u32 = 0x10;
}

/// 0x14cb8 : les quatre enregistrements de monnayeur à zéro, puis leurs réglages : `+0xb` = 0xff, `+8` =
/// « plus d'une pièce par crédit », `+9` = 4 et `+0xf` = « départ gratuit » (`A5+0x24`) en mode de
/// monnayage 17 ; jeu gratuit (`A5+0x25`) en mode 18.
pub fn initialize(b: &mut Cps2Bus) {
    use crate::mem::{wr32, wr8, A5};
    const COIN_MODE: u32 = 0x1c;
    const FREE_START: u32 = 0x24;
    let coin_slot = |k: u32| A5 + ram::COIN_SLOT_1 + ram::STEP * k;
    for i in 0..16 {
        wr32(b, A5 + ram::COIN_SLOT_1 + 4 * i, 0);
    }
    for k in 0..4 {
        wr8(b, coin_slot(k) + 0xb, 0xff);
    }
    let mode = rd8(b, A5 + COIN_MODE);
    wr8(b, A5 + ram::FREE, (mode == 18) as u8);
    let several = (rd8(b, A5 + ram::COINS_PER_CREDIT) as i8 > 1) as u8;
    let setting_9 = if mode == 17 { 4 } else { 0 };
    let free_start = rd8(b, A5 + FREE_START);
    for k in 0..4 {
        wr8(b, coin_slot(k) + 8, several);
        wr8(b, coin_slot(k) + 9, setting_9);
        wr8(b, coin_slot(k) + 0xf, free_start);
    }
}

/// Antirebond : saturation, et durée minimale d'une pièce valide (en images).
const DEBOUNCE_MAX: u8 = 50;
const DEBOUNCE_MIN: u8 = 2;
/// Crédits maximum.
const CREDITS_MAX: u8 = 9;

/// Bruit d'insertion de pièce (wrapper son).
const COIN_NOISE: u32 = 0xe18f8;

/// Une ligne de monnayeur : indice de l'enregistrement et masque dans `IN2`.
#[derive(Clone, Copy)]
struct Line {
    k: u32,
    mask: u8,
}

/// Parcourt les monnayeurs configurés dans l'ordre de l'original : les lignes à examiner et, entre
/// elles, les décisions des tests sur le nombre de monnayeurs (`apres_ligne(k)` est appelé après chaque
/// ligne). Le deuxième monnayeur lit la ligne 3 si les pièces sont partagées.
fn walk(b: &mut Cps2Bus, mut line: impl FnMut(&mut Cps2Bus, Line)) {
    let n = rd8(b, A5 + ram::COIN_SLOTS) as i8;
    line(b, Line { k: 0, mask: 0x10 });
    if n == 0 {
        return;
    }
    if n == 1 {
        let shared = rd8(b, A5 + ram::SHARED_MODE) != 0;
        line(b, Line { k: 1, mask: if shared { 0x40 } else { 0x20 } });
        return;
    }
    line(b, Line { k: 1, mask: 0x20 });
    if n < 2 {
        return;
    }
    line(b, Line { k: 2, mask: 0x40 });
    if n < 3 {
        return;
    }
    line(b, Line { k: 3, mask: 0x80 });
}

// ---------------------------------------------------------------------------------------------
// Lecture des monnayeurs (0x143da)

/// Traite une ligne (0x14546 compteurs communs, 0x145cc compteurs séparés). `level` : `IN2 & mask`.
fn handle_line(b: &mut Cps2Bus, separate: bool, l: Line, level: u8) {
    let a0 = A5 + ram::COIN_SLOT_1 + ram::STEP * l.k;
    let counters = if separate { a0 } else { A5 + ram::COIN_SLOT_1 };
    let debounce = rd8(b, a0);
    if !(level != 0) {
        // ligne basse : l'antirebond compte les images, plafonné
        wr8(b, a0, debounce.wrapping_add(1).min(DEBOUNCE_MAX));
        return;
    }
    // ligne haute : la pièce est comptée si la ligne était restée basse assez longtemps, sans saturer
    if (debounce < DEBOUNCE_MIN) || (debounce == DEBOUNCE_MAX) {
        wr8(b, a0, 0);
        return;
    }
    let total = rd8(b, counters + 3).wrapping_add(1);
    wr8(b, counters + 3, total);
    let credits = rd8(b, counters + 2);
    if !(credits as i8 >= CREDITS_MAX as i8) {
        let pieces = rd8(b, counters + 1).wrapping_add(1);
        wr8(b, counters + 1, pieces);
    }
    let pieces = rd8(b, counters + 1);
    let threshold = rd8(b, A5 + ram::COINS_PER_CREDIT);
    if !(pieces < threshold) {
        wr8(b, counters + 1, pieces - threshold);
        let gain = rd8(b, A5 + ram::CREDITS_PER_THRESHOLD);
        let credits = rd8(b, counters + 2).wrapping_add(gain);
        let cap_reached = !(credits < 10);
        wr8(b, counters + 2, if cap_reached { CREDITS_MAX } else { credits });
    }
    // bruit de pièce si une partie est en cours ou si le drapeau son est levé
    let sum = rd8(b, A5 + ram::GAME).wrapping_add(rd8(b, A5.wrapping_add(ram::COIN_SOUND)));
    if !(sum == 0) {
        crate::sound::play_global(b, COIN_NOISE);
    }
    // 0x2e448 : comptabilité
    crate::eeprom::count_coin(b);
    wr8(b, a0, 0);
}

/// 0x143da (VBLANK) : lit les lignes des monnayeurs. L'original rend D0 et A0 intacts et le VBLANK rend tous les
/// registres : rien ne sort dans les registres.
pub fn read_coin_slots(b: &mut Cps2Bus) {
    let menu = rd16(b, A5 + ram::MENU_TEST);
    if menu != 0 {
        return;
    }
    let separate = rd8(b, A5 + ram::SEPARATE_MODE) != 0;
    walk(b, |b, l| {
        let level = rd8(b, PORT_PIECES) & l.mask;
        handle_line(b, separate, l, level);
    });
}


// ---------------------------------------------------------------------------------------------
// Boutons de départ (0x141e0)

/// Absolu de l'enregistrement du premier monnayeur (comparé à A0 par l'original).
const FIRST_ABSOLUTE: u32 = 0xff844e;

/// 0x1422e : bouton de départ du monnayeur `coin_slot` (`service` = octet de IN2+1). Un départ ajoute un crédit
/// gratuit au monnayeur (0x1427a), avec comptabilité pour le premier.
fn line_start(b: &mut Cps2Bus, coin_slot: u32, service: u8) {
    if service & 4 != 0 {
        wr8(b, coin_slot + 7, 0);
        return;
    }
    if rd8(b, coin_slot + 7) != 0 {
        return;
    }
    let several = rd8(b, A5 + ram::COIN_SLOTS) != 0;
    if !several && coin_slot != FIRST_ABSOLUTE {
        return;
    }
    wr8(b, coin_slot + 7, 1);
    if coin_slot == FIRST_ABSOLUTE {
        crate::eeprom::count_start(b);
    }
    // le crédit ajouté est l'octet `+7` (relu après la comptabilité : 1)
    let credits = rd8(b, coin_slot + 2).wrapping_add(rd8(b, coin_slot + 7));
    wr8(b, coin_slot + 2, credits);
    if credits >= 10 {
        wr8(b, coin_slot + 2, CREDITS_MAX);
    }
}

/// 0x141e0 : les boutons de départ des monnayeurs configurés.
pub fn read_starts(b: &mut Cps2Bus) {
    let service = rd8(b, PORT_SERVICE);
    let absolute = |k: u32| FIRST_ABSOLUTE + ram::STEP * k;
    line_start(b, absolute(0), service);
    for k in 1..4 {
        // tst.b COIN_SLOTS ; beq (k = 1) puis SEPARATE_MODE ; beq, puis cmpi ; beq après chaque ligne
        let coin_slots = rd8(b, A5 + ram::COIN_SLOTS);
        let continue_game = match k {
            1 => coin_slots != 0 && rd8(b, A5 + ram::SEPARATE_MODE) != 0,
            _ => coin_slots != k as u8 - 1,
        };
        if !continue_game {
            return;
        }
        line_start(b, absolute(k), service);
    }
}

// ---------------------------------------------------------------------------------------------
// Verrouillage des compteurs de pièces (0x14290)

/// Décide du verrou pour le monnayeur `coin_slot` (0x14322) : verrouillé (vrai) si le jeu n'est pas gratuit et que
/// les crédits plus le gain d'un seuil restent sous 10.
fn simple_lock(b: &mut Cps2Bus, coin_slot: u32) -> bool {
    if rd8(b, A5 + ram::FREE) != 0 {
        return false;
    }
    let sum = rd8(b, coin_slot + 2).wrapping_add(rd8(b, A5 + ram::CREDITS_PER_THRESHOLD));
    (sum as i8) < 10
}

/// Idem pour les monnayeurs 2 à 4 (0x14300) : `rank` est leur numéro.
fn rank_lock(b: &mut Cps2Bus, coin_slot: u32, rank: u8) -> bool {
    if rd8(b, A5 + ram::FREE) != 0 {
        return false;
    }
    let coin_slots = rd8(b, A5 + ram::COIN_SLOTS);
    let shared = rd8(b, A5 + ram::SHARED_MODE) != 0;
    if rank as i8 > coin_slots as i8 {
        // monnayeur absent : verrouillé seulement s'il partage les pièces du premier (hors rang 3)
        if coin_slots != 1 || !shared || rank == 3 {
            return false;
        }
        return simple_lock(b, coin_slot);
    }
    // un seul monnayeur supplémentaire en mode partagé : pas de verrou propre
    if coin_slots == 1 && shared {
        return false;
    }
    simple_lock(b, coin_slot)
}

fn apply_lock(b: &mut Cps2Bus, locked: bool, mask: u8) {
    if locked {
        crate::video::lock_counters(b, mask);
    } else {
        crate::video::unlock_counters(b, mask);
    }
}

/// 0x14290 : verrouille ou libère le compteur de pièces de chaque monnayeur.
pub fn lock_counters(b: &mut Cps2Bus) {
    let separate = rd8(b, A5 + ram::SEPARATE_MODE) != 0;
    let coin_slot = |k: u32| A5 + ram::COIN_SLOT_1 + ram::STEP * k;
    let lock = simple_lock(b, coin_slot(0));
    apply_lock(b, lock, 0x10);
    for (k, mask) in [(1u32, 0x20u8), (2, 0x40), (3, 0x80)] {
        let m = coin_slot(if separate { k } else { 0 });
        let lock = rank_lock(b, m, k as u8);
        apply_lock(b, lock, mask);
    }
}

pub const READ_COIN_SLOTS: Native = Native {
    entry: 0x143da,
    name: "credits_lire_monnayeurs",
    run: crate::Body::Memory(read_coin_slots),
    touches_sound: false,
    outputs: 0,
};

// ---------------------------------------------------------------------------------------------
// Chiffre de l'affichage des crédits (0x14e46)

/// 0x14e46 : écrit le chiffre `value & 15` (tuile) puis l'attribut `attr` dans la couche de caractères en
/// `case`. Rend la case qui suit la tuile (A4 de l'original n'avance que d'un mot).
pub fn write_digit(b: &mut Cps2Bus, case: u32, value: u8, attr: u16) -> u32 {
    // addi.w #0,D0 : décalage de la tuile de base des chiffres, nul dans cette ROM
    let tile = (value & 15) as u16;
    crate::mem::wr16(b, case, tile);
    crate::mem::wr16(b, case.wrapping_add(2), attr);
    case.wrapping_add(2)
}
/// Sous-programme d'origine réécrit dans le corps de la tâche des crédits, présenté comme une native pour le fuzz
/// différentiel (`examples/fuzz_couv`) ; le jeu ne l'appelle pas par son adresse.
pub const TEST_NATIVES: &[Native] = &[Native {
    // 0x1422e : bouton de départ d'un monnayeur (A0 monnayeur, D1.b octet de IN2+1)
    entry: 0x1422e,
    name: "credits_depart_ligne",
    run: crate::Body::Processeur(|c, b| {
        let (a0, service) = (c.a[0], c.d[1] as u8);
        line_start(b, a0, service);
        crate::mem::rts(c, b);
    }),
    touches_sound: false,
    outputs: 0,
}];
