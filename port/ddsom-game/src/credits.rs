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
//! sous-routines non réécrites (bruit de pièce, comptabilité EEPROM, verrous 0xb96/0xbac). La durée
//! exacte vient du parcours du code d'origine selon les décisions prises (`chemin`).

use std::collections::HashMap;

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::chemin::Chemin;
use crate::mem::{rd16, rd8, rts, set_low_byte, wr8};
use crate::{call, regs, Native};

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

/// Antirebond : saturation, et durée minimale d'une pièce valide (en images).
const ANTIREBOND_MAX: u8 = 50;
const ANTIREBOND_MIN: u8 = 2;
/// Crédits maximum.
const CREDITS_MAX: u8 = 9;

/// Sous-routines non réécrites.
mod ext {
    /// Bruit d'insertion de pièce (wrapper son).
    pub const BRUIT_PIECE: u32 = 0xe18f8;
    /// Comptabilité des pièces / des départs (EEPROM, compteurs de service).
    pub const COMPTABILITE_PIECE: u32 = 0x2e448;
    pub const COMPTABILITE_DEPART: u32 = 0x2e49a;
    /// Verrouillage / déverrouillage des compteurs de pièces (masque dans D0).
    pub const VERROUILLER: u32 = 0xb96;
    pub const DEVERROUILLER: u32 = 0xbac;
}

/// Appels dont la durée est ajoutée par `call` et non par le parcours du code d'origine.
const EXTERNES: [u32; 5] =
    [ext::BRUIT_PIECE, ext::COMPTABILITE_PIECE, ext::COMPTABILITE_DEPART, ext::VERROUILLER, ext::DEVERROUILLER];

/// Accès à la mémoire de la routine : réel, ou simulé (calcul de la durée sans effet de bord). Les
/// décisions de branchement rencontrées sont enregistrées dans les deux cas.
struct Vue<'a> {
    b: &'a mut Cps2Bus,
    /// Écritures simulées (mode durée) ; `None` : écritures réelles.
    simule: Option<HashMap<u32, u8>>,
    chemin: Chemin,
}

impl<'a> Vue<'a> {
    fn reelle(b: &'a mut Cps2Bus) -> Self {
        Vue { b, simule: None, chemin: Chemin::default() }
    }

    fn simulee(b: &'a mut Cps2Bus) -> Self {
        Vue { b, simule: Some(HashMap::new()), chemin: Chemin::default() }
    }

    fn rd8(&mut self, a: u32) -> u8 {
        match self.simule.as_ref().and_then(|m| m.get(&(a & 0xff_ffff))) {
            Some(&v) => v,
            None => rd8(self.b, a),
        }
    }

    fn wr8(&mut self, a: u32, v: u8) {
        match &mut self.simule {
            Some(m) => {
                m.insert(a & 0xff_ffff, v);
            }
            None => wr8(self.b, a, v),
        }
    }

    /// Enregistre la décision d'un branchement conditionnel de l'original.
    fn pris(&mut self, pris: bool) -> bool {
        self.chemin.pris(pris);
        pris
    }

    /// Appelle une sous-routine non réécrite (sauf en mode simulé).
    fn appeler(&mut self, c: &mut Cpu, adresse: u32) {
        if self.simule.is_none() {
            call(c, self.b, adresse);
        }
    }
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
fn parcourir(v: &mut Vue, c: &mut Cpu, mut ligne: impl FnMut(&mut Vue, &mut Cpu, Ligne)) {
    let n = v.rd8(a5(c, ram::MONNAYEURS)) as i8;
    ligne(v, c, Ligne { k: 0, masque: 0x10 });
    if v.pris(n == 0) {
        return;
    }
    if v.pris(n == 1) {
        let partage = v.rd8(a5(c, ram::MODE_PARTAGE)) != 0;
        v.pris(partage);
        ligne(v, c, Ligne { k: 1, masque: if partage { 0x40 } else { 0x20 } });
        return;
    }
    ligne(v, c, Ligne { k: 1, masque: 0x20 });
    if v.pris(n < 2) {
        return;
    }
    ligne(v, c, Ligne { k: 2, masque: 0x40 });
    if v.pris(n < 3) {
        return;
    }
    ligne(v, c, Ligne { k: 3, masque: 0x80 });
}

// ---------------------------------------------------------------------------------------------
// Lecture des monnayeurs (0x143da)

/// Traite une ligne (0x14546 compteurs communs, 0x145cc compteurs séparés). `niveau` : `IN2 & masque`.
fn traiter_ligne(v: &mut Vue, c: &mut Cpu, separes: bool, l: Ligne, niveau: u8) {
    let a0 = enregistrement(c, l.k);
    let compteurs = if separes { a0 } else { enregistrement(c, 0) };
    let antirebond = v.rd8(a0);
    if !v.pris(niveau != 0) {
        // ligne basse : l'antirebond compte les images, plafonné
        let n = antirebond.wrapping_add(1);
        let sous_le_plafond = v.pris(n < ANTIREBOND_MAX);
        v.wr8(a0, if sous_le_plafond { n } else { ANTIREBOND_MAX });
        return;
    }
    // ligne haute : la pièce est comptée si la ligne était restée basse assez longtemps, sans saturer
    if v.pris(antirebond < ANTIREBOND_MIN) || v.pris(antirebond == ANTIREBOND_MAX) {
        v.wr8(a0, 0);
        return;
    }
    let total = v.rd8(compteurs + 3).wrapping_add(1);
    v.wr8(compteurs + 3, total);
    if !v.pris(v.rd8(compteurs + 2) as i8 >= CREDITS_MAX as i8) {
        let pieces = v.rd8(compteurs + 1).wrapping_add(1);
        v.wr8(compteurs + 1, pieces);
    }
    let pieces = v.rd8(compteurs + 1);
    let seuil = v.rd8(a5(c, ram::PIECES_PAR_CREDIT));
    if !v.pris(pieces < seuil) {
        v.wr8(compteurs + 1, pieces - seuil);
        let gain = v.rd8(a5(c, ram::CREDITS_PAR_SEUIL));
        let credits = v.rd8(compteurs + 2).wrapping_add(gain);
        let plafond_atteint = !v.pris(credits < 10);
        v.wr8(compteurs + 2, if plafond_atteint { CREDITS_MAX } else { credits });
    }
    // bruit de pièce si une partie est en cours ou si le drapeau son est levé
    let somme = v.rd8(a5(c, ram::PARTIE)).wrapping_add(v.rd8(a5(c, ram::SON_PIECE)));
    c.a[0] = a0;
    set_low_byte(&mut c.d[0], somme);
    if !v.pris(somme == 0) {
        v.appeler(c, ext::BRUIT_PIECE);
    }
    v.appeler(c, ext::COMPTABILITE_PIECE);
    v.wr8(a0, 0);
}

/// 0x143da (VBLANK) : D0 et A0 sont rendus intacts ; les autres registres ne servent pas au retour.
fn lire_monnayeurs(v: &mut Vue, c: &mut Cpu) {
    let menu = rd16(v.b, a5(c, ram::MENU_TEST));
    if v.pris(menu != 0) {
        return;
    }
    let (d0, a0) = (c.d[0], c.a[0]);
    let separes = v.rd8(a5(c, ram::MODE_SEPARE)) != 0;
    v.pris(separes);
    parcourir(v, c, |v, c, l| {
        let niveau = rd8(v.b, PORT_PIECES) & l.masque;
        traiter_ligne(v, c, separes, l, niveau);
    });
    c.d[0] = d0;
    c.a[0] = a0;
}

fn natif_lire_monnayeurs(c: &mut Cpu, b: &mut Cps2Bus) {
    lire_monnayeurs(&mut Vue::reelle(b), c);
    rts(c, b);
}

fn cout_lire_monnayeurs(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let mut c2 = c.clone();
    let mut v = Vue::simulee(b);
    lire_monnayeurs(&mut v, &mut c2);
    let chemin = v.chemin;
    chemin.duree_hors(b, 0x143da, None, &EXTERNES)
}

// ---------------------------------------------------------------------------------------------
// Boutons de départ (0x141e0)

/// Absolu de l'enregistrement du premier monnayeur (comparé à A0 par l'original).
const PREMIER_ABSOLU: u32 = 0xff844e;

/// 0x1422e : bouton de départ d'un monnayeur (`service` = octet de IN2+1). Un départ ajoute un crédit
/// gratuit au monnayeur (0x1427a), avec comptabilité pour le premier.
fn depart_ligne(v: &mut Vue, c: &mut Cpu, a0: u32, service: u8) {
    set_low_byte(&mut c.d[0], service & 4);
    if !v.pris(service & 4 == 0) {
        v.wr8(a0 + 7, 0);
        return;
    }
    if v.pris(v.rd8(a0 + 7) != 0) {
        return;
    }
    if !v.pris(v.rd8(a5(c, ram::MONNAYEURS)) != 0) && v.pris(a0 != PREMIER_ABSOLU) {
        return;
    }
    c.d[0] = 1;
    v.wr8(a0 + 7, 1);
    c.a[0] = a0;
    if !v.pris(a0 != PREMIER_ABSOLU) {
        v.appeler(c, ext::COMPTABILITE_DEPART);
        set_low_byte(&mut c.d[0], v.rd8(a0 + 7));
    }
    let credits = v.rd8(a0 + 2).wrapping_add(c.d[0] as u8);
    v.wr8(a0 + 2, credits);
    if !v.pris(credits < 10) {
        v.wr8(a0 + 2, CREDITS_MAX);
    }
}

fn lire_departs(v: &mut Vue, c: &mut Cpu) {
    let service = rd8(v.b, PORT_SERVICE);
    set_low_byte(&mut c.d[1], service);
    let absolu = |k: u32| PREMIER_ABSOLU + ram::PAS * k;
    c.a[0] = absolu(0);
    depart_ligne(v, c, absolu(0), service);
    for k in 1..4 {
        // tst.b MONNAYEURS ; beq (k = 1) puis MODE_SEPARE ; beq, puis cmpi ; beq après chaque ligne
        let monnayeurs = v.rd8(a5(c, ram::MONNAYEURS));
        let continuer = match k {
            1 => {
                !v.pris(monnayeurs == 0) && {
                    let separes = v.rd8(a5(c, ram::MODE_SEPARE)) == 0;
                    !v.pris(separes)
                }
            }
            _ => !v.pris(monnayeurs == k as u8 - 1),
        };
        if !continuer {
            return;
        }
        c.a[0] = absolu(k);
        depart_ligne(v, c, absolu(k), service);
    }
}

fn natif_lire_departs(c: &mut Cpu, b: &mut Cps2Bus) {
    lire_departs(&mut Vue::reelle(b), c);
    rts(c, b);
}

fn cout_lire_departs(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let mut c2 = c.clone();
    let mut v = Vue::simulee(b);
    lire_departs(&mut v, &mut c2);
    let chemin = v.chemin;
    chemin.duree_hors(b, 0x141e0, None, &EXTERNES)
}

// ---------------------------------------------------------------------------------------------
// Verrouillage des compteurs de pièces (0x14290)

/// Décide du verrou pour un monnayeur d'enregistrement `a0` (0x14322) : verrouillé (vrai) si le jeu
/// n'est pas gratuit et que les crédits plus le gain d'un seuil restent sous 10. Laisse la somme dans D1.
fn verrou_simple(v: &mut Vue, c: &mut Cpu, a0: u32) -> bool {
    if v.pris(v.rd8(a5(c, ram::GRATUIT)) != 0) {
        return false;
    }
    let somme = v.rd8(a0 + 2).wrapping_add(v.rd8(a5(c, ram::CREDITS_PAR_SEUIL)));
    set_low_byte(&mut c.d[1], somme);
    !v.pris(somme as i8 >= 10)
}

/// Idem pour les monnayeurs 2 à 4 (0x14300) : `rang` est leur numéro (D2).
fn verrou_rang(v: &mut Vue, c: &mut Cpu, a0: u32, rang: u8) -> bool {
    if v.pris(v.rd8(a5(c, ram::GRATUIT)) != 0) {
        return false;
    }
    let monnayeurs = v.rd8(a5(c, ram::MONNAYEURS));
    if v.pris(rang as i8 > monnayeurs as i8) {
        // monnayeur absent : verrouillé seulement s'il partage les pièces du premier (hors rang 3)
        if v.pris(monnayeurs != 1) || v.pris(v.rd8(a5(c, ram::MODE_PARTAGE)) == 0) || v.pris(rang == 3) {
            return false;
        }
        let somme = v.rd8(a0 + 2).wrapping_add(v.rd8(a5(c, ram::CREDITS_PAR_SEUIL)));
        set_low_byte(&mut c.d[1], somme);
        return !v.pris(somme as i8 >= 10);
    }
    if !v.pris(monnayeurs != 1) {
        // un seul monnayeur supplémentaire en mode partagé : pas de verrou propre
        if v.pris(v.rd8(a5(c, ram::MODE_PARTAGE)) != 0) {
            return false;
        }
    }
    verrou_simple(v, c, a0)
}

fn appliquer_verrou(v: &mut Vue, c: &mut Cpu, verrouille: bool, masque: u16) {
    c.d[0] = c.d[0] & 0xffff_0000 | masque as u32;
    v.appeler(c, if verrouille { ext::VERROUILLER } else { ext::DEVERROUILLER });
}

fn verrouiller_compteurs(v: &mut Vue, c: &mut Cpu) {
    let separes = v.rd8(a5(c, ram::MODE_SEPARE)) != 0;
    v.pris(separes);
    c.a[0] = enregistrement(c, 0);
    let verrou = verrou_simple(v, c, c.a[0]);
    appliquer_verrou(v, c, verrou, 0x10);
    for (k, masque) in [(1u32, 0x20u16), (2, 0x40), (3, 0x80)] {
        c.a[0] = enregistrement(c, if separes { k } else { 0 });
        c.d[2] = k;
        let verrou = verrou_rang(v, c, c.a[0], k as u8);
        appliquer_verrou(v, c, verrou, masque);
    }
}

fn natif_verrouiller_compteurs(c: &mut Cpu, b: &mut Cps2Bus) {
    verrouiller_compteurs(&mut Vue::reelle(b), c);
    rts(c, b);
}

fn cout_verrouiller_compteurs(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let mut c2 = c.clone();
    let mut v = Vue::simulee(b);
    verrouiller_compteurs(&mut v, &mut c2);
    let chemin = v.chemin;
    chemin.duree_hors(b, 0x14290, None, &EXTERNES)
}

pub const LIRE_MONNAYEURS: Native = Native {
    entry: 0x143da,
    name: "credits_lire_monnayeurs",
    cost: cout_lire_monnayeurs,
    run: natif_lire_monnayeurs,
    touches_sound: false,
    outputs: 0,
    exact_cost: false,
};

pub const LIRE_DEPARTS: Native = Native {
    entry: 0x141e0,
    name: "credits_lire_departs",
    cost: cout_lire_departs,
    run: natif_lire_departs,
    touches_sound: false,
    outputs: 0,
    exact_cost: false,
};

pub const VERROUILLER_COMPTEURS: Native = Native {
    entry: 0x14290,
    name: "credits_verrouiller",
    cost: cout_verrouiller_compteurs,
    run: natif_verrouiller_compteurs,
    touches_sound: false,
    outputs: 0,
    exact_cost: false,
};

/// Quels registres sont rendus : voir `regs`.
#[allow(dead_code)]
const _CONTRAT: u32 = regs::ALL;
