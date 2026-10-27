//! Files d'événements d'affichage (U1) : points de score, second compteur, pop-ups.
//!
//! Trois anneaux en RAM de travail, vidés d'un seul coup à l'initialisation. Une case vide vaut 0xFFFF.
//!
//! | Anneau | Index écriture / lecture | Cases | Rôle |
//! |---|---|---|---|
//! | points | `A5+0x108` / `A5+0x109` | 64 mots en `A5+0x10a` | événements de score |
//! | second compteur | `A5+0x18a` / `A5+0x18b` | 64 mots en `A5+0x18c` | idem, compteur à 5 chiffres |
//! | pop-ups | `A5+0x20c` / `A5+0x20d` | 32 entrées de 8 octets en `A5+0x20e` | textes flottants |
//!
//! Un événement de score est un mot `(joueur << 8) | type` : le joueur est lu dans l'objet émetteur
//! (mot `+0x48`, bits 8-9), le type est son octet `+0x7e`. Chaque image, la tâche d'interface des
//! joueurs (0x15200) vide les anneaux : la valeur BCD associée au type (table de ROM, 4 octets par
//! type) est ajoutée au compteur BCD du joueur (`+0x110`, 8 chiffres ; `+0x114`, 5 chiffres), plafonné.
//!
//! Cœurs sans registres (`vider_points`, `enfiler_points`, `evenement_objet`, `crediter_points`, `creer_popup`...)
//! et natives `_68k` pour les appels par adresse : celles-ci reproduisent les registres et drapeaux de l'original,
//! que des centaines d'appelants (objets ennemis...) peuvent relire.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_X};

use crate::mem::{rd16, rd32, rd8, rts, set_low_byte, wr16, wr32, wr8};
use crate::ops;
use crate::{regs, Native};

const VIDE: u16 = 0xffff;
/// Les index des anneaux avancent de 2 (octets) modulo 0x80.
const MASQUE: u8 = 0x7f;

/// Un anneau d'événements de score.
struct Anneau {
    /// Index d'écriture (octet) ; l'index de lecture est l'octet suivant.
    ecriture: u32,
    /// Première case (mots).
    cases: u32,
}

const POINTS: Anneau = Anneau { ecriture: 0x108, cases: 0x10a };
const COMPTEUR: Anneau = Anneau { ecriture: 0x18a, cases: 0x18c };

/// File des pop-ups : index d'écriture et de lecture (octets), première entrée.
const POPUP_ECRITURE: u32 = 0x20c;
const POPUP_LECTURE: u32 = 0x20d;
const POPUP_ENTREES: u32 = 0x20e;
/// Pop-up courant (4 longs en `A5-0x6e80`).
const POPUP_COURANT: u32 = 0xffff_9180;
/// Octet non nul : plus de nouveaux pop-ups.
const POPUP_INHIBE: u32 = 0x50;

/// Enregistrements joueur : le premier et la taille (0x200).
const JOUEUR_1: u32 = 0x5de;

/// Adresse `A5 + off` (A5 vaut toujours `mem::A5`).
const fn a5(off: u32) -> u32 {
    crate::mem::A5.wrapping_add(off)
}

fn set_mot(r: &mut u32, v: u16) {
    *r = *r & 0xffff_0000 | v as u32;
}

// ---------------------------------------------------------------------------------------------
// Remises à vide

/// `n` longs à 0xFFFFFFFF depuis `debut`.
fn remplir(b: &mut Cps2Bus, debut: u32, n: u32) {
    for k in 0..n {
        wr32(b, debut + 4 * k, 0xffff_ffff);
    }
}

/// Registres de `move.l D1,(A)+ ; dbf D0` : D1 = -1, mot bas de D0 = 0xFFFF, le registre d'adresse `ptr` rend la fin
/// de la zone.
fn remplir_68k(c: &mut Cpu, fin: u32, ptr: usize) {
    c.a[ptr] = fin;
    set_mot(&mut c.d[0], 0xffff);
    c.d[1] = 0xffff_ffff;
    ops::logique(c, 0xffff_ffff, Size::Long);
}

fn vider_anneau(b: &mut Cps2Bus, anneau: &Anneau) {
    wr8(b, a5(anneau.ecriture), 0);
    wr8(b, a5(anneau.ecriture + 1), 0);
    remplir(b, a5(anneau.cases), 32);
}

/// 0x19aa2 : vide l'anneau des points.
pub fn vider_points(b: &mut Cps2Bus) {
    vider_anneau(b, &POINTS);
}

/// 0x19abc : vide l'anneau du second compteur.
pub fn vider_compteur(b: &mut Cps2Bus) {
    vider_anneau(b, &COMPTEUR);
}

/// 0x19db4 : vide la file des pop-ups.
pub fn vider_popups(b: &mut Cps2Bus) {
    wr8(b, a5(POPUP_ECRITURE), 0);
    wr8(b, a5(POPUP_LECTURE), 0);
    remplir(b, a5(POPUP_ENTREES), 64);
}

/// 0x19e10 : efface le pop-up courant (4 longs).
pub fn effacer_popup_courant(b: &mut Cps2Bus) {
    for k in 0..4 {
        wr32(b, a5(POPUP_COURANT) + 4 * k, 0);
    }
}

fn vider_anneau_68k(c: &mut Cpu, b: &mut Cps2Bus, anneau: &Anneau) {
    vider_anneau(b, anneau);
    remplir_68k(c, a5(anneau.cases) + 4 * 32, 0);
    rts(c, b);
}

fn vider_popups_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    vider_popups(b);
    remplir_68k(c, a5(POPUP_ENTREES) + 4 * 64, 1);
    rts(c, b);
}

/// Laisse A1 après la zone, D0 = 0, D6 = 0xFFFF.
fn effacer_popup_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    effacer_popup_courant(b);
    c.a[1] = a5(POPUP_COURANT) + 16;
    c.d[0] = 0;
    c.d[6] = 0xffff;
    ops::logique(c, 0, Size::Long);
    rts(c, b);
}

// ---------------------------------------------------------------------------------------------
// Enfilage d'un événement de score

/// Issue d'un enfilage : l'index d'écriture lu, et le contenu de la case s'il n'était pas libre.
struct Enfile {
    index: u8,
    occupee: Option<u16>,
}

fn enfiler(b: &mut Cps2Bus, anneau: &Anneau, evenement: u16) -> Enfile {
    let index = rd8(b, a5(anneau.ecriture));
    let case = a5(anneau.cases).wrapping_add(index as u32);
    let contenu = rd16(b, case);
    if contenu != VIDE {
        return Enfile { index, occupee: Some(contenu) };
    }
    wr16(b, case, evenement);
    wr8(b, a5(anneau.ecriture), index.wrapping_add(2) & MASQUE);
    Enfile { index, occupee: None }
}

/// 0x19ae6 : enfile l'événement de score `evenement` (`(joueur << 8) | type`) si la case d'écriture est libre ;
/// rend vrai s'il est enfilé.
pub fn enfiler_points(b: &mut Cps2Bus, evenement: u16) -> bool {
    enfiler(b, &POINTS, evenement).occupee.is_none()
}

/// 0x19b08 : idem dans l'anneau du second compteur.
pub fn enfiler_compteur(b: &mut Cps2Bus, evenement: u16) -> bool {
    enfiler(b, &COMPTEUR, evenement).occupee.is_none()
}

/// Événement de score de l'objet `o` : type `+0x7e`, joueur `+0x48` bits 8-9.
pub fn evenement_de(b: &mut Cps2Bus, o: crate::objects::Obj) -> u16 {
    let type_ = rd8(b, o.0 + 0x7e) as u16;
    (rd16(b, o.0 + 0x48) & 0x300).wrapping_add(type_)
}

/// 0x19ad6 : enfile l'événement de score de l'objet `o` dans l'anneau des points.
pub fn evenement_objet(b: &mut Cps2Bus, o: crate::objects::Obj) -> bool {
    let evenement = evenement_de(b, o);
    enfiler_points(b, evenement)
}

/// Registres et drapeaux de 0x19ae6 / 0x19b08 : D6 (index), A4 (début des cases).
fn enfiler_68k(c: &mut Cpu, b: &mut Cps2Bus, anneau: &Anneau, evenement: u16) {
    let e = enfiler(b, anneau, evenement);
    c.d[6] = e.index as u32;
    c.a[4] = a5(anneau.cases);
    match e.occupee {
        Some(contenu) => ops::cmp(c, VIDE as u32, contenu as u32, Size::Word),
        None => {
            ops::add(c, 2, e.index as u32, Size::Byte);
            let reduit = e.index.wrapping_add(2) & MASQUE;
            set_mot(&mut c.d[6], reduit as u16);
            ops::logique(c, reduit as u32, Size::Byte);
        }
    }
}

fn enfiler_points_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    enfiler_68k(c, b, &POINTS, c.d[0] as u16);
    rts(c, b);
}

fn enfiler_compteur_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    enfiler_68k(c, b, &COMPTEUR, c.d[0] as u16);
    rts(c, b);
}

/// 0x19ad6 : D0 = l'événement, D1.w = le joueur (bits 8-9).
fn evenement_objet_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    let o = crate::objects::Obj(c.a[0]);
    let type_ = rd8(b, o.0 + 0x7e) as u32;
    c.d[0] = type_;
    let joueur = rd16(b, o.0 + 0x48) & 0x300;
    set_mot(&mut c.d[1], joueur);
    let evenement = ops::add(c, joueur as u32, type_, Size::Word) as u16;
    set_mot(&mut c.d[0], evenement);
    enfiler_68k(c, b, &POINTS, evenement);
    rts(c, b);
}

// ---------------------------------------------------------------------------------------------
// Crédit des points

/// Un anneau à vider, avec la table de valeurs et le compteur du joueur à incrémenter.
struct Credit {
    anneau: Anneau,
    /// Index de lecture (octet).
    lecture: u32,
    /// Table de ROM des valeurs BCD (4 octets par type).
    table: u32,
    /// Décalage du compteur BCD dans l'enregistrement joueur.
    compteur: u32,
    /// Plafond BCD (comparaison signée de 32 bits).
    plafond: u32,
}

const CREDIT_POINTS: Credit = Credit {
    anneau: POINTS,
    lecture: 0x109,
    table: 0x19c0e,
    compteur: 0x110,
    plafond: 0x0999_9999,
};
const CREDIT_COMPTEUR: Credit = Credit {
    anneau: COMPTEUR,
    lecture: 0x18b,
    table: 0x19d52,
    compteur: 0x114,
    plafond: 0x0009_9999,
};

/// Ce que le traitement d'un événement laisse dans les registres.
struct Traite {
    joueur: u32,
    ligne: u32,
}

/// Traite l'événement de la case `index` : vide la case, ajoute la valeur BCD du type au compteur
/// du joueur, plafonne, avance l'index de lecture.
fn traiter(b: &mut Cps2Bus, k: &Credit, index: u8, evenement: u16) -> Traite {
    wr16(b, a5(k.anneau.cases).wrapping_add(index as u32), VIDE);
    let joueur = a5(JOUEUR_1) + (((evenement & 0x300) as u32) << 1);
    let ligne = (evenement as u32 & 0xff) << 2;
    let (table, compteur) = (k.table + ligne, joueur + k.compteur);
    // abcd -(A3),-(A4) ×4 : octet de poids faible d'abord, retenue chaînée, X initial nul
    let mut retenue = false;
    for i in (0..4).rev() {
        let (somme, x) = ops::abcd(rd8(b, table + i), rd8(b, compteur + i), retenue);
        wr8(b, compteur + i, somme);
        retenue = x;
    }
    // cmpi.l #plafond,(A4) ; blt : on remplace par le plafond si la valeur n'est pas inférieure
    let plafonne = rd32(b, compteur) as i32 >= k.plafond as i32;
    if plafonne {
        wr32(b, compteur, k.plafond);
    }
    wr8(b, a5(k.lecture), index.wrapping_add(2) & MASQUE);
    Traite { joueur, ligne }
}

/// Évènement en attente à l'index de lecture, s'il y en a un.
fn en_attente(b: &mut Cps2Bus, k: &Credit) -> Option<(u8, u16)> {
    let index = rd8(b, a5(k.lecture));
    let evenement = rd16(b, a5(k.anneau.cases).wrapping_add(index as u32));
    (evenement != VIDE).then_some((index, evenement))
}

/// Dernier événement traité par un crédit : de quoi reconstituer les registres de l'original.
struct Dernier {
    index: u8,
    traite: Traite,
}

fn crediter(b: &mut Cps2Bus, k: &Credit) -> Option<Dernier> {
    let mut dernier = None;
    while let Some((index, evenement)) = en_attente(b, k) {
        let traite = traiter(b, k, index, evenement);
        dernier = Some(Dernier { index, traite });
    }
    dernier
}

/// 0x19b2a : crédite au compteur de points de chaque joueur les événements en attente.
pub fn crediter_points(b: &mut Cps2Bus) {
    crediter(b, &CREDIT_POINTS);
}

/// 0x19b9c : idem pour le second compteur.
pub fn crediter_compteur(b: &mut Cps2Bus) {
    crediter(b, &CREDIT_COMPTEUR);
}

/// Registres de 0x19b2a / 0x19b9c : restes de la dernière itération, D0 = index de lecture final.
fn crediter_68k(c: &mut Cpu, b: &mut Cps2Bus, k: &Credit) {
    c.d[1] = 0;
    c.d[2] = 0;
    c.a[2] = a5(k.anneau.cases);
    let dernier = crediter(b, k);
    if let Some(d) = &dernier {
        c.d[0] = d.index as u32;
        c.d[1] = d.traite.ligne;
        c.a[0] = d.traite.joueur;
        c.a[3] = k.table + d.traite.ligne;
        c.a[4] = d.traite.joueur + k.compteur;
        c.d[6] = 0xffff;
    }
    c.d[0] = rd8(b, a5(k.lecture)) as u32;
    // dernier cmpi.w #-1 : égalité ; X : retenue de l'`addq.b #2` sur l'index de lecture de la dernière itération
    ops::cmp(c, VIDE as u32, VIDE as u32, Size::Word);
    if let Some(d) = dernier {
        ops::drapeaux(c, SR_X, if d.index as u16 + 2 > 0xff { SR_X } else { 0 });
    }
    rts(c, b);
}

// ---------------------------------------------------------------------------------------------
// Pop-ups

/// Un pop-up (barre de vie d'un monstre frappé, `player::externes::effet::HEALTH_BAR`) : D0, D1, D2 de l'original.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Popup {
    /// Joueur et type (D0.w) : l'octet bas gagne 0x20 sauf si `brut`.
    pub entete: u16,
    /// D1.w, D2.w : PV maximum et PV (h).
    pub max: u16,
    pub valeur: u16,
    /// D3.b = 2 : l'en-tête est pris tel quel.
    pub brut: bool,
}

/// Issue de 0x19dce : rien (pop-ups inhibés), case occupée, ou enfilé.
enum Creation {
    Inhibe(u8),
    Occupee { index: u8, contenu: u16 },
    Faite { index: u8, entete: u16 },
}

fn creer(b: &mut Cps2Bus, p: Popup) -> Creation {
    let inhibe = rd8(b, a5(POPUP_INHIBE));
    if inhibe != 0 {
        return Creation::Inhibe(inhibe);
    }
    let index = rd8(b, a5(POPUP_ECRITURE));
    let case = a5(POPUP_ENTREES).wrapping_add(index as u32);
    let contenu = rd16(b, case);
    if contenu != VIDE {
        return Creation::Occupee { index, contenu };
    }
    let entete = if p.brut { p.entete } else { p.entete & 0xff00 | (p.entete as u8).wrapping_add(0x20) as u16 };
    for (decalage, valeur) in [(0, entete), (2, p.max), (4, p.valeur), (6, 0x80)] {
        wr16(b, case + decalage, valeur);
    }
    // l'index avance de 8 sans masque : il fait le tour des 32 entrées en débordant de son octet
    wr8(b, a5(POPUP_ECRITURE), index.wrapping_add(8));
    Creation::Faite { index, entete }
}

/// Les pop-ups sont-ils inhibés (`A5+0x50`) ? 0x19dce n'écrit alors rien et laisse A2 intact ; sinon il laisse
/// A2 = [`ENTREES_POPUPS`] (relu par le combat : `combat::show_enemy_hp`).
pub fn popups_inhibes(b: &mut Cps2Bus) -> bool {
    rd8(b, a5(POPUP_INHIBE)) != 0
}

/// Première entrée de la file des pop-ups (A2 que laisse 0x19dce).
pub const ENTREES_POPUPS: u32 = a5(POPUP_ENTREES);

/// 0x19dce : enfile un pop-up dans la case d'écriture si elle est libre et si les pop-ups ne sont pas inhibés ;
/// rend vrai s'il est enfilé.
pub fn creer_popup(b: &mut Cps2Bus, p: Popup) -> bool {
    matches!(creer(b, p), Creation::Faite { .. })
}

/// Registres et drapeaux de 0x19dce : D6 (index), A2 (entrées), D0.b (en-tête), drapeaux du dernier test.
fn creer_popup_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    let p = Popup { entete: c.d[0] as u16, max: c.d[1] as u16, valeur: c.d[2] as u16, brut: c.d[3] as u8 == 2 };
    match creer(b, p) {
        Creation::Inhibe(inhibe) => ops::logique(c, inhibe as u32, Size::Byte),
        Creation::Occupee { index, contenu } => {
            c.d[6] = index as u32;
            c.a[2] = a5(POPUP_ENTREES);
            ops::cmp(c, VIDE as u32, contenu as u32, Size::Word);
        }
        Creation::Faite { index, entete } => {
            c.a[2] = a5(POPUP_ENTREES);
            ops::cmp(c, 2, c.d[3] & 0xff, Size::Byte);
            if !p.brut {
                ops::add(c, 0x20, c.d[0] & 0xff, Size::Byte);
                set_low_byte(&mut c.d[0], entete as u8);
            }
            let suivant = ops::add(c, 8, index as u32, Size::Byte) as u8;
            c.d[6] = suivant as u32;
            ops::logique(c, suivant as u32, Size::Byte);
        }
    }
    rts(c, b);
}

macro_rules! natif {
    ($nom:ident, $entree:expr, $name:expr, $run:expr) => {
        pub const $nom: Native = Native {
            entry: $entree,
            name: $name,
            run: $run,
            touches_sound: false,
            outputs: regs::ALL,
        };
    };
}

natif!(VIDER_POINTS, 0x19aa2, "evt_vider_points", |c, b| vider_anneau_68k(c, b, &POINTS));
natif!(VIDER_COMPTEUR, 0x19abc, "evt_vider_compteur", |c, b| vider_anneau_68k(c, b, &COMPTEUR));
natif!(VIDER_POPUPS, 0x19db4, "evt_vider_popups", vider_popups_68k);
natif!(EFFACER_POPUP, 0x19e10, "evt_effacer_popup", effacer_popup_68k);
natif!(ENFILER_POINTS, 0x19ae6, "evt_enfiler_points", enfiler_points_68k);
natif!(ENFILER_COMPTEUR, 0x19b08, "evt_enfiler_compteur", enfiler_compteur_68k);
natif!(EVENEMENT_OBJET, 0x19ad6, "evt_evenement_objet", evenement_objet_68k);
natif!(CREER_POPUP, 0x19dce, "evt_creer_popup", creer_popup_68k);
natif!(CREDITER_POINTS, 0x19b2a, "evt_crediter_points", |c, b| crediter_68k(c, b, &CREDIT_POINTS));
natif!(CREDITER_COMPTEUR, 0x19b9c, "evt_crediter_compteur", |c, b| crediter_68k(c, b, &CREDIT_COMPTEUR));
