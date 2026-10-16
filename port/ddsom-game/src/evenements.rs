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
//! Les registres et drapeaux laissés par l'original sont reproduits : ces petites routines ont des
//! centaines d'appelants (objets ennemis...) dont on ne vérifie pas un à un l'usage des restes.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_X};

use crate::chemin::Chemin;
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

fn a5(c: &Cpu, off: u32) -> u32 {
    c.a[5].wrapping_add(off)
}

fn set_mot(r: &mut u32, v: u16) {
    *r = *r & 0xffff_0000 | v as u32;
}

// ---------------------------------------------------------------------------------------------
// Remises à vide

/// Décisions d'une boucle `dbf` de `n` tours.
fn boucle(n: u32) -> Chemin {
    let mut p = Chemin::default();
    p.repete(n - 1, true);
    p.pris(false);
    p
}

/// `n` longs à 0xFFFFFFFF depuis `debut` (`move.l D1,(A)+ ; dbf D0`). D1 = -1, mot bas de D0 = 0xFFFF,
/// le registre d'adresse `ptr` rend la fin de la zone.
fn remplir(c: &mut Cpu, b: &mut Cps2Bus, debut: u32, n: u32, ptr: usize) {
    for k in 0..n {
        wr32(b, debut + 4 * k, 0xffff_ffff);
    }
    c.a[ptr] = debut + 4 * n;
    set_mot(&mut c.d[0], 0xffff);
    c.d[1] = 0xffff_ffff;
    ops::logique(c, 0xffff_ffff, Size::Long);
}

/// 0x19aa2 / 0x19abc : vide l'anneau.
fn vider_anneau(c: &mut Cpu, b: &mut Cps2Bus, anneau: &Anneau) {
    wr8(b, a5(c, anneau.ecriture), 0);
    wr8(b, a5(c, anneau.ecriture + 1), 0);
    remplir(c, b, a5(c, anneau.cases), 32, 0);
    rts(c, b);
}

fn vider_points(c: &mut Cpu, b: &mut Cps2Bus) {
    vider_anneau(c, b, &POINTS);
}

fn vider_compteur(c: &mut Cpu, b: &mut Cps2Bus) {
    vider_anneau(c, b, &COMPTEUR);
}

/// 0x19db4 : vide la file des pop-ups (64 longs, A1 rend la fin).
fn vider_popups(c: &mut Cpu, b: &mut Cps2Bus) {
    wr8(b, a5(c, POPUP_ECRITURE), 0);
    wr8(b, a5(c, POPUP_LECTURE), 0);
    remplir(c, b, a5(c, POPUP_ENTREES), 64, 1);
    rts(c, b);
}

/// 0x19e10 : efface le pop-up courant (4 longs). Laisse A1 après la zone, D0 = 0, D6 = 0xFFFF.
fn effacer_popup_courant(c: &mut Cpu, b: &mut Cps2Bus) {
    let debut = a5(c, POPUP_COURANT);
    for k in 0..4 {
        wr32(b, debut + 4 * k, 0);
    }
    c.a[1] = debut + 16;
    c.d[0] = 0;
    c.d[6] = 0xffff;
    ops::logique(c, 0, Size::Long);
    rts(c, b);
}

// ---------------------------------------------------------------------------------------------
// Enfilage d'un événement de score

/// Enfile `evenement` dans l'anneau si la case d'écriture est libre (0x19ae6 / 0x19b08).
/// Laisse D6 (index), A4 (début des cases) et les drapeaux comme l'original.
fn enfiler(c: &mut Cpu, b: &mut Cps2Bus, anneau: &Anneau, evenement: u16) {
    c.d[6] = 0;
    ops::logique(c, 0, Size::Long);
    c.a[4] = a5(c, anneau.cases);
    let index = rd8(b, a5(c, anneau.ecriture));
    set_low_byte(&mut c.d[6], index);
    ops::logique(c, index as u32, Size::Byte);
    let case = c.a[4].wrapping_add(index as u32);
    let contenu = rd16(b, case);
    ops::cmp(c, VIDE as u32, contenu as u32, Size::Word);
    if contenu != VIDE {
        return;
    }
    wr16(b, case, evenement);
    ops::logique(c, evenement as u32, Size::Word);
    let suivant = ops::add(c, 2, index as u32, Size::Byte) as u8;
    set_low_byte(&mut c.d[6], suivant);
    let reduit = suivant & MASQUE;
    set_mot(&mut c.d[6], reduit as u16);
    ops::logique(c, reduit as u32, Size::Word);
    wr8(b, a5(c, anneau.ecriture), reduit);
    ops::logique(c, reduit as u32, Size::Byte);
}

/// Décision unique de ces routines : la case d'écriture est-elle occupée (retour sans rien faire) ?
fn chemin_enfiler(c: &Cpu, b: &mut Cps2Bus, anneau: &Anneau) -> Chemin {
    let mut p = Chemin::default();
    let index = rd8(b, a5(c, anneau.ecriture));
    p.pris(rd16(b, a5(c, anneau.cases).wrapping_add(index as u32)) != VIDE);
    p
}

fn enfiler_points(c: &mut Cpu, b: &mut Cps2Bus) {
    enfiler(c, b, &POINTS, c.d[0] as u16);
    rts(c, b);
}

fn enfiler_compteur(c: &mut Cpu, b: &mut Cps2Bus) {
    enfiler(c, b, &COMPTEUR, c.d[0] as u16);
    rts(c, b);
}

/// 0x19ad6 : événement de l'objet A0 (type `+0x7e`, joueur `+0x48` bits 8-9) dans l'anneau des points.
fn evenement_objet(c: &mut Cpu, b: &mut Cps2Bus) {
    let type_ = rd8(b, c.a[0] + 0x7e) as u32;
    c.d[0] = type_;
    ops::logique(c, type_, Size::Byte);
    let mot = rd16(b, c.a[0] + 0x48);
    set_mot(&mut c.d[1], mot);
    ops::logique(c, mot as u32, Size::Word);
    let joueur = mot & 0x300;
    set_mot(&mut c.d[1], joueur);
    ops::logique(c, joueur as u32, Size::Word);
    let evenement = ops::add(c, joueur as u32, type_, Size::Word) as u16;
    set_mot(&mut c.d[0], evenement);
    enfiler(c, b, &POINTS, evenement);
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
    /// Entrée de la routine, et début de sa boucle (pour mesurer la durée).
    entree: u32,
    boucle: u32,
}

const CREDIT_POINTS: Credit = Credit {
    anneau: POINTS,
    lecture: 0x109,
    table: 0x19c0e,
    compteur: 0x110,
    plafond: 0x0999_9999,
    entree: 0x19b2a,
    boucle: 0x19b32,
};
const CREDIT_COMPTEUR: Credit = Credit {
    anneau: COMPTEUR,
    lecture: 0x18b,
    table: 0x19d52,
    compteur: 0x114,
    plafond: 0x0009_9999,
    entree: 0x19b9c,
    boucle: 0x19ba4,
};

/// Ce que le traitement d'un événement laisse dans les registres.
struct Traite {
    joueur: u32,
    ligne: u32,
}

/// Traite l'événement de la case `index` : vide la case, ajoute la valeur BCD du type au compteur
/// du joueur, plafonne, avance l'index de lecture.
fn traiter(c: &Cpu, b: &mut Cps2Bus, k: &Credit, index: u8, evenement: u16) -> Traite {
    wr16(b, a5(c, k.anneau.cases).wrapping_add(index as u32), VIDE);
    let joueur = a5(c, JOUEUR_1) + (((evenement & 0x300) as u32) << 1);
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
    wr8(b, a5(c, k.lecture), index.wrapping_add(2) & MASQUE);
    Traite { joueur, ligne }
}

/// Évènement en attente à l'index de lecture, s'il y en a un.
fn en_attente(c: &Cpu, b: &mut Cps2Bus, k: &Credit) -> Option<(u8, u16)> {
    let index = rd8(b, a5(c, k.lecture));
    let evenement = rd16(b, a5(c, k.anneau.cases).wrapping_add(index as u32));
    (evenement != VIDE).then_some((index, evenement))
}

/// 0x19b2a / 0x19b9c : crédite tous les événements en attente.
fn crediter(c: &mut Cpu, b: &mut Cps2Bus, k: &Credit) {
    c.d[1] = 0;
    c.d[2] = 0;
    ops::logique(c, 0, Size::Long);
    c.a[2] = a5(c, k.anneau.cases);
    let mut retenue = None;
    while let Some((index, evenement)) = en_attente(c, b, k) {
        let t = traiter(c, b, k, index, evenement);
        // X final : retenue de l'`addq.b #2` sur l'index de lecture
        retenue = Some(index as u16 + 2 > 0xff);
        // restes de l'itération, tels que l'original les laisse
        c.d[0] = index as u32;
        c.d[1] = t.ligne;
        c.a[0] = t.joueur;
        c.a[3] = k.table + t.ligne;
        c.a[4] = t.joueur + k.compteur;
        c.d[6] = 0xffff;
    }
    c.d[0] = rd8(b, a5(c, k.lecture)) as u32;
    // dernier cmpi.w #-1 : égalité ; X reste celui de la dernière addition décimale
    ops::cmp(c, VIDE as u32, VIDE as u32, Size::Word);
    if let Some(r) = retenue {
        ops::drapeaux(c, SR_X, if r { SR_X } else { 0 });
    }
}

/// Pour chaque événement en attente, le compteur serait-il plafonné ? (simulation sans effet sur la
/// mémoire : les compteurs déjà modifiés par un événement précédent sont suivis à part)
fn plafonnements(c: &Cpu, b: &mut Cps2Bus, k: &Credit) -> Vec<bool> {
    let mut sortie = Vec::new();
    let mut index = rd8(b, a5(c, k.lecture));
    let mut vides: Vec<u8> = Vec::new();
    let mut compteurs: Vec<(u32, u32)> = Vec::new();
    loop {
        let evenement = if vides.contains(&index) {
            VIDE
        } else {
            rd16(b, a5(c, k.anneau.cases).wrapping_add(index as u32))
        };
        if evenement == VIDE {
            return sortie;
        }
        let joueur = a5(c, JOUEUR_1) + (((evenement & 0x300) as u32) << 1);
        let table = k.table + ((evenement as u32 & 0xff) << 2);
        let adresse = joueur + k.compteur;
        let courant = match compteurs.iter().position(|&(a, _)| a == adresse) {
            Some(i) => compteurs[i].1,
            None => rd32(b, adresse),
        };
        let mut valeur = courant.to_be_bytes();
        let mut retenue = false;
        for i in (0..4).rev() {
            (valeur[i], retenue) = ops::abcd(rd8(b, table + i as u32), valeur[i], retenue);
        }
        let mut valeur = u32::from_be_bytes(valeur);
        let plafonne = valeur as i32 >= k.plafond as i32;
        if plafonne {
            valeur = k.plafond;
        }
        match compteurs.iter_mut().find(|(a, _)| *a == adresse) {
            Some(e) => e.1 = valeur,
            None => compteurs.push((adresse, valeur)),
        }
        sortie.push(plafonne);
        vides.push(index);
        index = index.wrapping_add(2) & MASQUE;
    }
}

/// Durée : tronçon d'entrée, un tour de boucle par événement (selon le plafonnement), tour final.
fn cout_crediter(c: &Cpu, b: &mut Cps2Bus, k: &Credit) -> u64 {
    let mut total = Chemin::default().duree_entre(b, k.entree, Some(k.boucle));
    for plafonne in plafonnements(c, b, k) {
        let mut p = Chemin::default();
        p.pris(false); // beq : case non vide
        p.repete(3, true);
        p.pris(false); // fin de la boucle abcd
        p.pris(!plafonne); // blt
        total += p.duree_entre(b, k.boucle, Some(k.boucle));
    }
    let mut fin = Chemin::default();
    fin.pris(true); // beq : case vide
    total + fin.duree(b, k.boucle)
}

fn crediter_points(c: &mut Cpu, b: &mut Cps2Bus) {
    crediter(c, b, &CREDIT_POINTS);
    rts(c, b);
}

fn crediter_compteur(c: &mut Cpu, b: &mut Cps2Bus) {
    crediter(c, b, &CREDIT_COMPTEUR);
    rts(c, b);
}

// ---------------------------------------------------------------------------------------------
// Pop-ups

/// 0x19dce : enfile un pop-up (D0, D1, D2 = paramètres ; D3 = 2 : D0 tel quel, sinon + 0x20 sur son
/// octet bas) dans la case d'écriture si elle est libre et si les pop-ups ne sont pas inhibés.
fn creer_popup(c: &mut Cpu, b: &mut Cps2Bus) {
    let inhibe = rd8(b, a5(c, POPUP_INHIBE));
    ops::logique(c, inhibe as u32, Size::Byte);
    if inhibe != 0 {
        rts(c, b);
        return;
    }
    c.d[6] = 0;
    ops::logique(c, 0, Size::Long);
    c.a[2] = a5(c, POPUP_ENTREES);
    let index = rd8(b, a5(c, POPUP_ECRITURE));
    set_low_byte(&mut c.d[6], index);
    ops::logique(c, index as u32, Size::Byte);
    let case = c.a[2].wrapping_add(index as u32);
    let contenu = rd16(b, case);
    ops::cmp(c, VIDE as u32, contenu as u32, Size::Word);
    if contenu != VIDE {
        rts(c, b);
        return;
    }
    ops::cmp(c, 2, c.d[3] & 0xff, Size::Byte);
    if c.d[3] as u8 != 2 {
        let v = ops::add(c, 0x20, c.d[0] & 0xff, Size::Byte);
        set_low_byte(&mut c.d[0], v as u8);
    }
    for (decalage, valeur) in [(0, c.d[0] as u16), (2, c.d[1] as u16), (4, c.d[2] as u16), (6, 0x80)] {
        wr16(b, case + decalage, valeur);
        ops::logique(c, valeur as u32, Size::Word);
    }
    let suivant = ops::add(c, 8, index as u32, Size::Byte) as u8;
    set_low_byte(&mut c.d[6], suivant);
    set_mot(&mut c.d[6], suivant as u16);
    ops::logique(c, suivant as u32, Size::Word);
    wr8(b, a5(c, POPUP_ECRITURE), suivant);
    ops::logique(c, suivant as u32, Size::Byte);
    rts(c, b);
}

/// Décisions de 0x19dce : inhibé, case occupée, D3 == 2.
fn chemin_creer_popup(c: &Cpu, b: &mut Cps2Bus) -> Chemin {
    let mut p = Chemin::default();
    let inhibe = rd8(b, a5(c, POPUP_INHIBE)) != 0;
    p.pris(inhibe);
    if inhibe {
        return p;
    }
    let index = rd8(b, a5(c, POPUP_ECRITURE));
    let occupee = rd16(b, a5(c, POPUP_ENTREES).wrapping_add(index as u32)) != VIDE;
    p.pris(occupee);
    if !occupee {
        p.pris(c.d[3] as u8 == 2);
    }
    p
}

macro_rules! natif {
    ($nom:ident, $entree:expr, $name:expr, $run:expr, $cost:expr) => {
        pub const $nom: Native = Native {
            entry: $entree,
            name: $name,
            cost: $cost,
            run: $run,
            touches_sound: false,
            outputs: regs::ALL,
            exact_cost: true,
        };
    };
}

natif!(VIDER_POINTS, 0x19aa2, "evt_vider_points", vider_points, |_, b| boucle(32).duree(b, 0x19aa2));
natif!(VIDER_COMPTEUR, 0x19abc, "evt_vider_compteur", vider_compteur, |_, b| boucle(32).duree(b, 0x19abc));
natif!(VIDER_POPUPS, 0x19db4, "evt_vider_popups", vider_popups, |_, b| boucle(64).duree(b, 0x19db4));
natif!(EFFACER_POPUP, 0x19e10, "evt_effacer_popup", effacer_popup_courant, |_, b| boucle(4).duree(b, 0x19e10));
natif!(ENFILER_POINTS, 0x19ae6, "evt_enfiler_points", enfiler_points, |c, b| chemin_enfiler(c, b, &POINTS).duree(b, 0x19ae6));
natif!(ENFILER_COMPTEUR, 0x19b08, "evt_enfiler_compteur", enfiler_compteur, |c, b| chemin_enfiler(c, b, &COMPTEUR).duree(b, 0x19b08));
natif!(EVENEMENT_OBJET, 0x19ad6, "evt_evenement_objet", evenement_objet, |c, b| chemin_enfiler(c, b, &POINTS).duree(b, 0x19ad6));
natif!(CREER_POPUP, 0x19dce, "evt_creer_popup", creer_popup, |c, b| chemin_creer_popup(c, b).duree(b, 0x19dce));
natif!(CREDITER_POINTS, 0x19b2a, "evt_crediter_points", crediter_points, |c, b| cout_crediter(c, b, &CREDIT_POINTS));
natif!(CREDITER_COMPTEUR, 0x19b9c, "evt_crediter_compteur", crediter_compteur, |c, b| cout_crediter(c, b, &CREDIT_COMPTEUR));
