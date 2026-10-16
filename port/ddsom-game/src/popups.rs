//! Pop-ups (U1) : machine à états qui sort les textes flottants de leur file, un à la fois.
//!
//! `A5-0x6e80` : pop-up courant (4 mots : identifiant, deux paramètres, durée restante en images).
//! La file (`A5+0x20e`, entrées de 8 octets, lecture en `A5+0x20d`) est remplie par `creer_popup`
//! (0x19dce, voir `evenements.rs`). À chaque image (0x19e20) :
//!
//! * pas de pop-up courant (durée nulle) : le suivant, s'il y en a un, devient courant (sa case est
//!   libérée), son dessin est lancé (0x19f24) ; sans second paramètre sa durée est de 0x60 images ;
//! * pop-up courant : sa durée diminue ; à zéro il est effacé (0x19ee0) puis remplacé par le suivant ;
//!   si un autre attend et que la durée restante est d'au moins 4 images, elle est ramenée à 4.
//!
//! Le dessin (0x19ee0 effacement, 0x19f24 affichage, 0x19fd2 et 0x1a20e cadres) reste en code
//! d'origine : il est appelé par `call`.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::mem::rts;
use crate::vue::Vue;
use crate::Native;

const VIDE: u16 = 0xffff;
/// Pop-up courant (4 mots) ; la durée restante est le dernier.
const COURANT: u32 = 0xffff_9180;
const DUREE: u32 = 6;
const ENTREES: u32 = 0x20e;
const LECTURE: u32 = 0x20d;
const ENTREE_TAILLE: u32 = 8;
/// Durée par défaut d'un pop-up sans second paramètre (images), et durée raccourcie.
const DUREE_DEFAUT: u16 = 0x60;
const DUREE_RACCOURCIE: u16 = 4;

mod ext {
    pub const EFFACER: u32 = 0x19ee0;
    pub const DESSINER: u32 = 0x19f24;
}

fn a5(c: &Cpu, off: u32) -> u32 {
    c.a[5].wrapping_add(off)
}

/// Fait du prochain pop-up de la file le pop-up courant (s'il y en a un) et lance son dessin.
fn suivant(v: &mut Vue, c: &mut Cpu, courant: u32) {
    let index = v.rd8(a5(c, LECTURE));
    c.d[0] = index as u32;
    let case = a5(c, ENTREES) + index as u32;
    let vide = v.rd16(case) == VIDE;
    if v.pris(vide) {
        return;
    }
    for decalage in [0, 2, 4, 6] {
        let mot = v.rd16(case + decalage);
        v.wr16(courant + decalage, mot);
        v.wr16(case + decalage, VIDE);
    }
    let second_parametre = v.rd16(courant + 4);
    if !v.pris(second_parametre != 0) {
        v.wr16(courant + DUREE, DUREE_DEFAUT);
    }
    v.appeler(c, ext::DESSINER);
    v.wr8(a5(c, LECTURE), index.wrapping_add(ENTREE_TAILLE as u8));
}

fn animer(v: &mut Vue, c: &mut Cpu) {
    let courant = a5(c, COURANT);
    c.a[0] = courant;
    c.a[2] = a5(c, ENTREES);
    let duree = v.rd16(courant + DUREE);
    if !v.pris(duree == 0) {
        let duree = duree.wrapping_sub(1);
        v.wr16(courant + DUREE, duree);
        if !v.pris(duree == 0) {
            // le pop-up courant continue ; s'il y en a un autre en attente, on abrège
            if v.pris(duree < DUREE_RACCOURCIE) {
                return;
            }
            let index = v.rd8(a5(c, LECTURE));
            c.d[0] = index as u32;
            let vide = v.rd16(a5(c, ENTREES) + index as u32) == VIDE;
            if !v.pris(vide) {
                v.wr16(courant + DUREE, DUREE_RACCOURCIE);
            }
            return;
        }
        v.appeler(c, ext::EFFACER);
    }
    suivant(v, c, courant);
}

fn natif_animer(c: &mut Cpu, b: &mut Cps2Bus) {
    animer(&mut Vue::reelle(b), c);
    rts(c, b);
}

fn cout_animer(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let mut c2 = c.clone();
    let mut v = Vue::simulee(b);
    animer(&mut v, &mut c2);
    let chemin = v.chemin;
    chemin.duree_hors(b, 0x19e20, None, &[ext::EFFACER, ext::DESSINER])
}

pub const ANIMER: Native = Native {
    entry: 0x19e20,
    name: "popups_animer",
    cost: cout_animer,
    run: natif_animer,
    touches_sound: false,
    outputs: 0,
    exact_cost: false,
};
