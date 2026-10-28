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
//! Compteurs communs (`MODE_SEPARE` nul) : tout se passe dans l'enregistrement du premier monnayeur,
//! les autres ne servent que d'antirebond. Compteurs séparés : chaque monnayeur a les siens.
//!
//! Les routines natives lisent, décident et écrivent comme l'original ; elles appellent par `call` les
//! sous-routines non réécrites (bruit de pièce, comptabilité EEPROM, verrous 0xb96/0xbac).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::mem::{rd16, rd8, rts, set_low_byte, wr8, A5};
use crate::ops;
use crate::{regs, Native};

/// Port des monnayeurs et port des boutons de service.
const PORT_PIECES: u32 = 0x804020;
const PORT_SERVICE: u32 = 0x804021;

/// Réglages et états, relatifs à A5.
mod ram {
    /// Pièces nécessaires pour un crédit.
    pub const PIECES_PAR_CREDIT: u32 = 0x1e;
    /// Crédits accordés quand le seuil est atteint.
    pub const CREDITS_PAR_SEUIL: u32 = 0x1f;
    /// Mode de partage des pièces (≠ 0 : le 2e monnayeur lit la ligne 3 ; lockout du 2e).
    pub const MODE_PARTAGE: u32 = 0x20;
    /// Nombre de monnayeurs moins un.
    pub const MONNAYEURS: u32 = 0x21;
    /// Compteurs séparés par monnayeur (0 : compteurs communs).
    pub const MODE_SEPARE: u32 = 0x22;
    /// Jeu gratuit : pas de verrouillage.
    pub const GRATUIT: u32 = 0x25;
    /// Partie en cours.
    pub const PARTIE: u32 = 0x30;
    /// Mot non nul pendant le menu de test : rien n'est lu.
    pub const MENU_TEST: u32 = 0x7fcc;
    /// Drapeau qui, avec `PARTIE`, autorise le bruit de pièce (A5 - 0x713c).
    pub const SON_PIECE: u32 = 0xffff_8ec4;
    /// Premier enregistrement monnayeur, et pas.
    pub const MONNAYEUR_1: u32 = 0x44e;
    pub const PAS: u32 = 0x10;
}

/// 0x14cb8 : les quatre enregistrements de monnayeur à zéro, puis leurs réglages : `+0xb` = 0xff, `+8` =
/// « plus d'une pièce par crédit », `+9` = 4 et `+0xf` = « départ gratuit » (`A5+0x24`) en mode de
/// monnayage 17 ; jeu gratuit (`A5+0x25`) en mode 18. D0 garde la dernière valeur écrite.
pub fn initialiser(c: &mut Cpu, b: &mut Cps2Bus) {
    use crate::mem::{wr32, wr8, A5};
    const MODE_MONNAYAGE: u32 = 0x1c;
    const DEPART_GRATUIT: u32 = 0x24;
    let monnayeur = |k: u32| A5 + ram::MONNAYEUR_1 + ram::PAS * k;
    for i in 0..16 {
        wr32(b, A5 + ram::MONNAYEUR_1 + 4 * i, 0);
    }
    for k in 0..4 {
        wr8(b, monnayeur(k) + 0xb, 0xff);
    }
    let mode = rd8(b, A5 + MODE_MONNAYAGE);
    wr8(b, A5 + ram::GRATUIT, (mode == 18) as u8);
    let plusieurs = (rd8(b, A5 + ram::PIECES_PAR_CREDIT) as i8 > 1) as u8;
    let reglage_9 = if mode == 17 { 4 } else { 0 };
    let depart_gratuit = rd8(b, A5 + DEPART_GRATUIT);
    for k in 0..4 {
        wr8(b, monnayeur(k) + 8, plusieurs);
        wr8(b, monnayeur(k) + 9, reglage_9);
        wr8(b, monnayeur(k) + 0xf, depart_gratuit);
    }
    c.d[0] = depart_gratuit as u32;
}

/// Antirebond : saturation, et durée minimale d'une pièce valide (en images).
const ANTIREBOND_MAX: u8 = 50;
const ANTIREBOND_MIN: u8 = 2;
/// Crédits maximum.
const CREDITS_MAX: u8 = 9;

/// Sous-routines non réécrites.
mod ext {
    /// Bruit d'insertion de pièce (wrapper son).
    pub const BRUIT_PIECE: u32 = 0xe18f8;
    /// Comptabilité des départs (EEPROM, compteurs de service).
    pub const COMPTABILITE_DEPART: u32 = 0x2e49a;
    /// Verrouillage / déverrouillage des compteurs de pièces (masque dans D0).
    pub const VERROUILLER: u32 = 0xb96;
    pub const DEVERROUILLER: u32 = 0xbac;
}

fn a5(c: &Cpu, off: u32) -> u32 {
    c.a[5].wrapping_add(off)
}

fn enregistrement(c: &Cpu, k: u32) -> u32 {
    a5(c, ram::MONNAYEUR_1 + ram::PAS * k)
}

/// Une ligne de monnayeur : indice de l'enregistrement et masque dans `IN2`.
#[derive(Clone, Copy)]
struct Ligne {
    k: u32,
    masque: u8,
}

/// Parcourt les monnayeurs configurés dans l'ordre de l'original : les lignes à examiner et, entre
/// elles, les décisions des tests sur le nombre de monnayeurs (`apres_ligne(k)` est appelé après chaque
/// ligne). Le deuxième monnayeur lit la ligne 3 si les pièces sont partagées.
fn parcourir(b: &mut Cps2Bus, mut ligne: impl FnMut(&mut Cps2Bus, Ligne)) {
    let n = rd8(b, A5 + ram::MONNAYEURS) as i8;
    ligne(b, Ligne { k: 0, masque: 0x10 });
    if n == 0 {
        return;
    }
    if n == 1 {
        let partage = rd8(b, A5 + ram::MODE_PARTAGE) != 0;
        ligne(b, Ligne { k: 1, masque: if partage { 0x40 } else { 0x20 } });
        return;
    }
    ligne(b, Ligne { k: 1, masque: 0x20 });
    if n < 2 {
        return;
    }
    ligne(b, Ligne { k: 2, masque: 0x40 });
    if n < 3 {
        return;
    }
    ligne(b, Ligne { k: 3, masque: 0x80 });
}

// ---------------------------------------------------------------------------------------------
// Lecture des monnayeurs (0x143da)

/// Traite une ligne (0x14546 compteurs communs, 0x145cc compteurs séparés). `niveau` : `IN2 & masque`.
fn traiter_ligne(b: &mut Cps2Bus, separes: bool, l: Ligne, niveau: u8) {
    let a0 = A5 + ram::MONNAYEUR_1 + ram::PAS * l.k;
    let compteurs = if separes { a0 } else { A5 + ram::MONNAYEUR_1 };
    let antirebond = rd8(b, a0);
    if !(niveau != 0) {
        // ligne basse : l'antirebond compte les images, plafonné
        wr8(b, a0, antirebond.wrapping_add(1).min(ANTIREBOND_MAX));
        return;
    }
    // ligne haute : la pièce est comptée si la ligne était restée basse assez longtemps, sans saturer
    if (antirebond < ANTIREBOND_MIN) || (antirebond == ANTIREBOND_MAX) {
        wr8(b, a0, 0);
        return;
    }
    let total = rd8(b, compteurs + 3).wrapping_add(1);
    wr8(b, compteurs + 3, total);
    let credits = rd8(b, compteurs + 2);
    if !(credits as i8 >= CREDITS_MAX as i8) {
        let pieces = rd8(b, compteurs + 1).wrapping_add(1);
        wr8(b, compteurs + 1, pieces);
    }
    let pieces = rd8(b, compteurs + 1);
    let seuil = rd8(b, A5 + ram::PIECES_PAR_CREDIT);
    if !(pieces < seuil) {
        wr8(b, compteurs + 1, pieces - seuil);
        let gain = rd8(b, A5 + ram::CREDITS_PAR_SEUIL);
        let credits = rd8(b, compteurs + 2).wrapping_add(gain);
        let plafond_atteint = !(credits < 10);
        wr8(b, compteurs + 2, if plafond_atteint { CREDITS_MAX } else { credits });
    }
    // bruit de pièce si une partie est en cours ou si le drapeau son est levé
    let somme = rd8(b, A5 + ram::PARTIE).wrapping_add(rd8(b, A5.wrapping_add(ram::SON_PIECE)));
    if !(somme == 0) {
        crate::sound::jouer_global(b, ext::BRUIT_PIECE);
    }
    // 0x2e448 : comptabilité
    crate::eeprom::compter_piece(b);
    wr8(b, a0, 0);
}

/// 0x143da (VBLANK) : lit les lignes des monnayeurs. L'original rend D0 et A0 intacts et le VBLANK rend tous les
/// registres : rien ne sort dans les registres.
pub fn lire_monnayeurs(b: &mut Cps2Bus) {
    let menu = rd16(b, A5 + ram::MENU_TEST);
    if menu != 0 {
        return;
    }
    let separes = rd8(b, A5 + ram::MODE_SEPARE) != 0;
    parcourir(b, |b, l| {
        let niveau = rd8(b, PORT_PIECES) & l.masque;
        traiter_ligne(b, separes, l, niveau);
    });
}


// ---------------------------------------------------------------------------------------------
// Boutons de départ (0x141e0)

/// Absolu de l'enregistrement du premier monnayeur (comparé à A0 par l'original).
const PREMIER_ABSOLU: u32 = 0xff844e;

/// 0x1422e : bouton de départ d'un monnayeur (`service` = octet de IN2+1). Un départ ajoute un crédit
/// gratuit au monnayeur (0x1427a), avec comptabilité pour le premier.
fn depart_ligne(b: &mut Cps2Bus, c: &mut Cpu, a0: u32, service: u8) {
    set_low_byte(&mut c.d[0], service & 4);
    if !(service & 4 == 0) {
        wr8(b, a0 + 7, 0);
        return;
    }
    let pris_en_compte = rd8(b, a0 + 7) != 0;
    if pris_en_compte {
        return;
    }
    let plusieurs = rd8(b, a5(c, ram::MONNAYEURS)) != 0;
    if !plusieurs && (a0 != PREMIER_ABSOLU) {
        return;
    }
    c.d[0] = 1;
    wr8(b, a0 + 7, 1);
    c.a[0] = a0;
    if !(a0 != PREMIER_ABSOLU) {
        crate::call(c, b, ext::COMPTABILITE_DEPART);
        set_low_byte(&mut c.d[0], rd8(b, a0 + 7));
    }
    let credits = rd8(b, a0 + 2).wrapping_add(c.d[0] as u8);
    wr8(b, a0 + 2, credits);
    if !(credits < 10) {
        wr8(b, a0 + 2, CREDITS_MAX);
    }
}

fn lire_departs(b: &mut Cps2Bus, c: &mut Cpu) {
    let service = rd8(b, PORT_SERVICE);
    set_low_byte(&mut c.d[1], service);
    let absolu = |k: u32| PREMIER_ABSOLU + ram::PAS * k;
    c.a[0] = absolu(0);
    depart_ligne(b, c, absolu(0), service);
    for k in 1..4 {
        // tst.b MONNAYEURS ; beq (k = 1) puis MODE_SEPARE ; beq, puis cmpi ; beq après chaque ligne
        let monnayeurs = rd8(b, a5(c, ram::MONNAYEURS));
        let continuer = match k {
            1 => {
                !(monnayeurs == 0) && {
                    let separes = rd8(b, a5(c, ram::MODE_SEPARE)) == 0;
                    !separes
                }
            }
            _ => !(monnayeurs == k as u8 - 1),
        };
        if !continuer {
            return;
        }
        c.a[0] = absolu(k);
        depart_ligne(b, c, absolu(k), service);
    }
}

fn natif_lire_departs(c: &mut Cpu, b: &mut Cps2Bus) {
    lire_departs(b, c);
    rts(c, b);
}

// ---------------------------------------------------------------------------------------------
// Verrouillage des compteurs de pièces (0x14290)

/// Décide du verrou pour un monnayeur d'enregistrement `a0` (0x14322) : verrouillé (vrai) si le jeu
/// n'est pas gratuit et que les crédits plus le gain d'un seuil restent sous 10. Laisse la somme dans D1.
fn verrou_simple(b: &mut Cps2Bus, c: &mut Cpu, a0: u32) -> bool {
    let gratuit = rd8(b, a5(c, ram::GRATUIT)) != 0;
    if gratuit {
        return false;
    }
    let somme = rd8(b, a0 + 2).wrapping_add(rd8(b, a5(c, ram::CREDITS_PAR_SEUIL)));
    set_low_byte(&mut c.d[1], somme);
    !(somme as i8 >= 10)
}

/// Idem pour les monnayeurs 2 à 4 (0x14300) : `rang` est leur numéro (D2).
fn verrou_rang(b: &mut Cps2Bus, c: &mut Cpu, a0: u32, rang: u8) -> bool {
    let gratuit = rd8(b, a5(c, ram::GRATUIT)) != 0;
    if gratuit {
        return false;
    }
    let monnayeurs = rd8(b, a5(c, ram::MONNAYEURS));
    if rang as i8 > monnayeurs as i8 {
        // monnayeur absent : verrouillé seulement s'il partage les pièces du premier (hors rang 3)
        if monnayeurs != 1 {
            return false;
        }
        let partage = rd8(b, a5(c, ram::MODE_PARTAGE)) != 0;
        if (!partage) || (rang == 3) {
            return false;
        }
        let somme = rd8(b, a0 + 2).wrapping_add(rd8(b, a5(c, ram::CREDITS_PAR_SEUIL)));
        set_low_byte(&mut c.d[1], somme);
        return !(somme as i8 >= 10);
    }
    if !(monnayeurs != 1) {
        // un seul monnayeur supplémentaire en mode partagé : pas de verrou propre
        let partage = rd8(b, a5(c, ram::MODE_PARTAGE)) != 0;
        if partage {
            return false;
        }
    }
    verrou_simple(b, c, a0)
}

fn appliquer_verrou(b: &mut Cps2Bus, c: &mut Cpu, verrouille: bool, masque: u16) {
    c.d[0] = c.d[0] & 0xffff_0000 | masque as u32;
    crate::call(c, b, if verrouille { ext::VERROUILLER } else { ext::DEVERROUILLER });
}

fn verrouiller_compteurs(b: &mut Cps2Bus, c: &mut Cpu) {
    let separes = rd8(b, a5(c, ram::MODE_SEPARE)) != 0;
    c.a[0] = enregistrement(c, 0);
    let verrou = verrou_simple(b, c, c.a[0]);
    appliquer_verrou(b, c, verrou, 0x10);
    for (k, masque) in [(1u32, 0x20u16), (2, 0x40), (3, 0x80)] {
        c.a[0] = enregistrement(c, if separes { k } else { 0 });
        c.d[2] = k;
        let verrou = verrou_rang(b, c, c.a[0], k as u8);
        appliquer_verrou(b, c, verrou, masque);
    }
}

fn natif_verrouiller_compteurs(c: &mut Cpu, b: &mut Cps2Bus) {
    verrouiller_compteurs(b, c);
    rts(c, b);
}

pub const LIRE_MONNAYEURS: Native = Native {
    entry: 0x143da,
    name: "credits_lire_monnayeurs",
    run: crate::Corps::Memoire(lire_monnayeurs),
    touches_sound: false,
    outputs: 0,
};

pub const LIRE_DEPARTS: Native = Native {
    entry: 0x141e0,
    name: "credits_lire_departs",
    run: crate::Corps::Processeur(natif_lire_departs),
    touches_sound: false,
    outputs: 0,
};

pub const VERROUILLER_COMPTEURS: Native = Native {
    entry: 0x14290,
    name: "credits_verrouiller",
    run: crate::Corps::Processeur(natif_verrouiller_compteurs),
    touches_sound: false,
    outputs: 0,
};

// ---------------------------------------------------------------------------------------------
// Chiffre de l'affichage des crédits (0x14e46)

/// 0x14e46 : écrit le chiffre `D0 & 15` (tuile) puis l'attribut D1 dans la couche de caractères en (A4).
/// Rend D0 (mot bas), A4 avancé d'un mot et les drapeaux de l'addition d'origine.
fn ecrire_chiffre(c: &mut Cpu, b: &mut Cps2Bus) {
    let chiffre = (c.d[0] & 15) as u16;
    ops::logique(c, chiffre as u32, Size::Word);
    // addi.w #0,D0 : décalage de la tuile de base des chiffres, nul dans cette ROM
    let tuile = ops::add(c, 0, chiffre as u32, Size::Word) as u16;
    c.d[0] = c.d[0] & 0xffff_0000 | tuile as u32;
    crate::mem::wr16(b, c.a[4], tuile);
    crate::mem::wr16(b, c.a[4].wrapping_add(2), c.d[1] as u16);
    c.a[4] = c.a[4].wrapping_add(2);
    ops::logique(c, c.d[1], Size::Word); // le dernier move.w donne N et Z
    rts(c, b);
}

pub const ECRIRE_CHIFFRE: Native = Native {
    entry: 0x14e46,
    name: "credits_chiffre",
    run: crate::Corps::Processeur(ecrire_chiffre),
    touches_sound: false,
    outputs: regs::d(0) | regs::a(4) | regs::CCR,
};

/// Sous-programme d'origine réécrit dans le corps de la tâche des crédits, présenté comme une native pour le fuzz
/// différentiel (`examples/fuzz_couv`) ; le jeu ne l'appelle pas par son adresse.
pub const NATIVES_DE_TEST: &[Native] = &[Native {
    // 0x1422e : bouton de départ d'un monnayeur (A0 monnayeur, D1.b octet de IN2+1)
    entry: 0x1422e,
    name: "credits_depart_ligne",
    run: crate::Corps::Processeur(|c, b| {
        let (a0, service) = (c.a[0], c.d[1] as u8);
        depart_ligne(b, c, a0, service);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: 0,
}];
