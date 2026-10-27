//! Noyau (K1) : ordonnanceur coopératif de tâches (0x928) et traps 0 à 7 et 12.
//!
//! Le jeu fait tourner 16 tâches en alternance. Une tâche rend la main par une *enveloppe de cession* (`jsr 0xa40`,
//! `0xa86`...) ; l'ordonnanceur choisit alors la première tâche prête de la table. L'état des tâches est la table Rust
//! de [`crate::taches::table`] ; ces natives n'en sont que la frontière avec le 68000 : elles gardent les points
//! d'entrée, les registres que l'original laisse et la fenêtre d'interruption entre deux emplacements, mais plus
//! aucun enregistrement en RAM (FF0064), ni pile de l'ordonnanceur (FF0E66 reste son pointeur fixe), ni tas des
//! piles (FF005A), ni registres sauvés sur la pile des tâches.
//!
//! ## Ordonnanceur
//!
//! Il parcourt les emplacements dans l'ordre (A0 = adresse de l'enregistrement d'origine, D0.w = emplacements
//! restants, de 15 à 0). Chaque emplacement est examiné par une exécution native séparée, interruptions masquées
//! jusqu'au niveau 6 ; entre deux, la fenêtre de 0x952 les autorise. Un VBLANK pose l'indicateur
//! `NOUVELLE_IMAGE` (FF0164) : le parcours reprend alors au début. Une tâche prête reprend avec les registres, le SR
//! et la pile sauvés à sa cession, à l'adresse qui suit son appel de l'enveloppe ; une tâche neuve démarre à son
//! entrée, A0 = enregistrement, A1 = entrée, A4 = pile, SR = 0 (mode utilisateur). Quand elle cède, l'ordonnanceur
//! reprend à l'emplacement suivant (0x984 puis 0x952).
//!
//! ## Enveloppes (et traps de l'original)
//!
//! | Enveloppe | Trap | Effet |
//! |---|---|---|
//! | 0x9b6 | 0 | crée la tâche D0 (décalage dans la table) avec l'entrée A0 |
//! | 0x9f8 | 1 | la tâche courante se termine |
//! | 0xa14 | 2 | tue la tâche D0 |
//! | 0xa40, 0xb64 | 3 | cède D0 images (0 : suspendue jusqu'au trap 6) ; 0xb64 : une image |
//! | 0xa86 | 4 | cède, reste prête |
//! | 0xaac | 5 | suspend la tâche D0 |
//! | 0xac8 | 6 | réveille la tâche D0 avec le compte D1 |
//! | 0xae4 | 7 | la tâche courante recommence en A0 |
//! | 0xaf6 | 12 | masque (D0 ≠ 0) ou démasque les interruptions |
//!
//! Dans l'original, une enveloppe sauve les 15 registres sur la pile de la tâche, prend le trap, et sa reprise
//! (0x9bc, 0xa4c...) les restaure : les enveloppes qui ne cèdent pas laissent donc registres et drapeaux intacts, et
//! celles qui cèdent sauvent ici le contexte dans la table. Seul `trap #7` est aussi exécuté directement par du code
//! 68000 (0x2c3e0, menu de mise au point) ; les autres handlers ne sont atteints que par leurs enveloppes.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::mem::{rd16, rd32, rd8, rts, set_low_byte, set_low_word, wr16, wr32, wr8, A5};
use crate::taches::table::{emplacement, enregistrement, table, Contexte, Etat, Reprise, Suite, ENREGISTREMENTS};
use crate::Native;

/// Adresses de la RAM.
mod ram {
    /// Indicateur posé par le VBLANK : le parcours de l'ordonnanceur recommence.
    pub const NOUVELLE_IMAGE: u32 = 0xff0164;
    /// Mot de réglages lu en VBLANK (DSW) : bits 0 à 2, vitesse du jeu.
    pub const DSW: u32 = 0xffffd0;
    /// Pile du superviseur de l'ordonnanceur (A5 - 0x719a).
    pub const PILE_SUPERVISEUR: u32 = super::A5 - 0x719a;
}

/// Points du code d'origine.
mod at {
    /// Début de l'ordonnanceur : pile, A5.
    pub const DEBUT: u32 = 0x928;
    /// Nouveau parcours de la table.
    pub const PARCOURS: u32 = 0x932;
    /// Examen d'un emplacement (D0 = reste à voir, A0 = enregistrement).
    pub const EXAMEN: u32 = 0x93e;
    /// Passage à l'emplacement suivant, après une cession.
    pub const SUIVANT: u32 = 0x952;
    /// Retour à l'ordonnanceur d'une tâche qui cède.
    pub const RETOUR: u32 = 0x984;
    pub const TRAP_RECOMMENCER: u32 = 0xae6;
    pub const TRAP_MASQUE: u32 = 0xafa;
}

const SR_SUPERVISEUR: u16 = 0x2000;
const SR_PARCOURS: u16 = 0x2600;
/// Emplacements examinés par un parcours, moins un (D0.w au début).
const DERNIER: u16 = 15;

fn depiler32(c: &mut Cpu, b: &mut Cps2Bus) -> u32 {
    let v = rd32(b, c.a[7]);
    c.a[7] = c.a[7].wrapping_add(4);
    v
}

/// RTE : dépile SR puis PC.
fn rte(c: &mut Cpu, b: &mut Cps2Bus) {
    let sr = rd16(b, c.a[7]);
    c.a[7] = c.a[7].wrapping_add(2);
    c.pc = depiler32(c, b);
    c.set_sr(sr);
}

/// Passage en superviseur, comme à la prise d'un trap.
fn superviseur(c: &mut Cpu) {
    c.set_sr((c.sr | SR_SUPERVISEUR) & !m68k::SR_T);
}

// ---------------------------------------------------------------------------------------------
// Ordonnanceur

/// 0x928 : l'ordonnanceur prend la main (appelé une fois, au démarrage).
fn debut(c: &mut Cpu, _b: &mut Cps2Bus) {
    c.a[7] = ram::PILE_SUPERVISEUR;
    c.a[5] = A5;
    c.pc = at::PARCOURS;
}

/// 0x932 : nouveau parcours de la table.
fn parcours(c: &mut Cpu, b: &mut Cps2Bus) {
    wr8(b, ram::NOUVELLE_IMAGE, 0);
    c.a[0] = ENREGISTREMENTS;
    set_low_word(&mut c.d[0], DERNIER);
    c.pc = at::EXAMEN;
}

/// Passe à l'emplacement suivant (`dbf`), ou recommence quand la table est épuisée.
fn suivant(c: &mut Cpu) {
    c.a[0] += 0x10;
    let reste = (c.d[0] as u16).wrapping_sub(1);
    set_low_word(&mut c.d[0], reste);
    c.pc = if reste == 0xffff { at::PARCOURS } else { at::EXAMEN };
}

/// 0x93e : examen d'un emplacement, avec interruptions masquées (jusqu'au niveau 6).
fn examen(c: &mut Cpu, b: &mut Cps2Bus) {
    c.set_sr(SR_PARCOURS);
    if rd8(b, ram::NOUVELLE_IMAGE) != 0 {
        c.pc = at::PARCOURS;
        return;
    }
    let k = emplacement(c.a[0] - ENREGISTREMENTS);
    let etat = table(b).emplacements[k].etat;
    set_low_byte(&mut c.d[1], etat.octet());
    if etat.lancable() {
        lancer(c, b, k);
    } else {
        c.set_sr(SR_SUPERVISEUR);
        suivant(c);
    }
}

/// 0x952 : la tâche a cédé, la fenêtre d'interruption s'ouvre et le parcours continue.
fn apres_cession(c: &mut Cpu, _b: &mut Cps2Bus) {
    c.set_sr(SR_SUPERVISEUR);
    suivant(c);
}

/// 0x960 : lance la tâche de l'emplacement `k` (A0 = son enregistrement).
fn lancer(c: &mut Cpu, b: &mut Cps2Bus, k: usize) {
    let t = table(b);
    t.courante = k;
    let e = &mut t.emplacements[k];
    set_low_word(&mut c.d[1], e.etat.octet() as u16);
    match (e.etat, e.reprise) {
        (Etat::Neuve, Reprise::Entree(entree)) => {
            e.etat = Etat::EnCours;
            c.a[1] = entree;
            c.a[4] = e.pile;
            c.other_sp = e.pile;
            c.set_sr(0);
            match crate::taches::entree(entree) {
                Some(etape) => crate::taches::executer(etape, c, b),
                None => c.pc = entree,
            }
        }
        (Etat::Prete | Etat::EnCours, Reprise::Cession(ctx)) => {
            e.etat = Etat::EnCours;
            remettre(c, ctx);
            c.pc = ctx.pc;
        }
        // une tâche Rust : son pas est appelé directement
        (Etat::Prete | Etat::EnCours, Reprise::Etape(etape, ctx)) => {
            e.etat = Etat::EnCours;
            remettre(c, ctx);
            crate::taches::executer(etape, c, b);
        }
        (etat, reprise) => unreachable!("tâche {k} lancée dans l'état {etat:?} avec la reprise {reprise:?}"),
    }
}

/// Registres, SR et pile d'une tâche qui a cédé.
fn remettre(c: &mut Cpu, ctx: Contexte) {
    c.d = ctx.d;
    c.a[..7].copy_from_slice(&ctx.a);
    c.other_sp = ctx.usp;
    c.set_sr(ctx.sr);
}

/// 0x984 : une tâche a cédé (ou s'est terminée) ; l'ordonnanceur reprend à l'emplacement qui suit le sien.
fn retour_ordonnanceur(c: &mut Cpu, b: &mut Cps2Bus) {
    let k = table(b).courante;
    c.a[7] = ram::PILE_SUPERVISEUR;
    c.a[0] = enregistrement(k);
    set_low_word(&mut c.d[0], DERNIER - k as u16);
    c.pc = at::SUIVANT;
}

/// Rend la main à l'ordonnanceur après une opération sur la tâche courante (registres laissés par le trap).
fn vers_ordonnanceur(c: &mut Cpu, b: &mut Cps2Bus) {
    c.a[0] = enregistrement(table(b).courante);
    superviseur(c);
    c.pc = at::RETOUR;
}

// ---------------------------------------------------------------------------------------------
// Cessions : la tâche courante rend la main ; elle reprendra en `reprise` avec ses registres, son SR et sa pile
// (A7) actuels. Les tâches Rust cèdent directement (`taches::ceder`) ; les enveloppes de l'original dépilent d'abord
// leur adresse de retour.

/// Range le contexte de la tâche courante dans la table, puis rend la main à l'ordonnanceur. A1 reçoit la pile de la
/// tâche telle que l'original l'avait laissée (adresse de retour et 15 registres empilés).
fn ceder(c: &mut Cpu, b: &mut Cps2Bus, etat: Etat, compte: Option<u8>, reprise: Suite) {
    crate::taches::table::relever(c, b, reprise);
    let mut a = [0; 7];
    a.copy_from_slice(&c.a[..7]);
    let ctx = Contexte { pc: reprise.adresse(), sr: c.sr, usp: c.a[7], d: c.d, a };
    let reprise = match reprise {
        Suite::Adresse(_) => Reprise::Cession(ctx),
        Suite::Etape(etape) => Reprise::Etape(etape, ctx),
    };
    table(b).ceder(etat, compte, reprise);
    c.a[1] = ctx.usp.wrapping_sub(64);
    vers_ordonnanceur(c, b);
}

/// Trap 3 : cède D0.b images (plus le retard des réglages DSW), 0 : suspendue. L'enveloppe 0xa40 appelle d'abord
/// 0xb40, qui selon DSW remplit un bloc de la GFX RAM de la jauge de charge (jamais, dans ce jeu : bit 0x400 de DSW) ;
/// les drapeaux sauvés sont ceux de son test de DSW. Le trap laisse D0.w = état et compte, D1.w = 0x100 ou 0x200.
pub fn attendre(c: &mut Cpu, b: &mut Cps2Bus, reprise: impl Into<Suite>) {
    let reprise = reprise.into();
    c.sr &= !0x0f;
    let reglage = !rd16(b, ram::DSW) & 7;
    let compte = (c.d[0] as u8).wrapping_add(reglage as u8);
    let (mut d0, mut d1) = (c.d[0], c.d[1]);
    set_low_word(&mut d1, if compte == 0 { 0x200 } else { 0x100 });
    let mot = (d0 as u16 & 0xff00) | compte as u16 | d1 as u16;
    set_low_word(&mut d0, mot);
    let etat = Etat::depuis_octet((mot >> 8) as u8).unwrap_or_else(|| panic!("cession vers {reprise:x?} : état {mot:#06x}"));
    ceder(c, b, etat, Some(compte), reprise);
    (c.d[0], c.d[1]) = (d0, d1);
}

/// Trap 4 : cède en restant prête.
pub fn ceder_prete(c: &mut Cpu, b: &mut Cps2Bus, reprise: impl Into<Suite>) {
    ceder(c, b, Etat::Prete, None, reprise.into());
}

/// Trap 1 : la tâche courante se termine et rend sa pile.
pub fn terminer(c: &mut Cpu, b: &mut Cps2Bus) {
    let t = table(b);
    let k = t.courante;
    t.liberer(k);
    vers_ordonnanceur(c, b);
}

/// Enveloppe de cession appelée par `jsr` : son adresse de retour est la reprise (ou la suite de l'appel en cours,
/// quand un pas Rust l'a appelée).
fn enveloppe(c: &mut Cpu, b: &mut Cps2Bus, cession: fn(&mut Cpu, &mut Cps2Bus, Suite)) {
    let reprise = crate::taches::table::depiler_retour(c, b);
    cession(c, b, reprise);
}

/// 0xae4 (et `trap #7`, 0xae6) : la tâche courante repart de zéro en A0, même pile.
pub fn recommencer(c: &mut Cpu, b: &mut Cps2Bus) {
    let t = table(b);
    t.recommencer(c.a[0]);
    c.a[4] = enregistrement(t.courante);
    superviseur(c);
    c.pc = at::RETOUR;
}

// ---------------------------------------------------------------------------------------------
// Opérations sur une autre tâche : registres et drapeaux intacts

/// 0x9b6 : crée la tâche D0 (décalage), entrée A0. Une tâche déjà là rend sa pile.
fn creer(c: &mut Cpu, b: &mut Cps2Bus) {
    creer_tache(b, c.d[0], c.a[0]);
    rts(c, b);
}

/// Crée la tâche de décalage `decalage` dans la table (`0x10 k`), qui démarrera à `entree` (enveloppe `0x9b6` : ni
/// registre ni drapeau ne change).
pub fn creer_tache(b: &mut Cps2Bus, decalage: u32, entree: u32) {
    table(b).creer(emplacement(decalage), entree);
}

/// 0xa14 : tue la tâche D0.
fn tuer(c: &mut Cpu, b: &mut Cps2Bus) {
    tuer_tache(b, c.d[0]);
    rts(c, b);
}

/// Tue la tâche de décalage `decalage` (enveloppe `0xa14` : ni registre ni drapeau ne change).
pub fn tuer_tache(b: &mut Cps2Bus, decalage: u32) {
    table(b).tuer(emplacement(decalage));
}

/// 0xaac : suspend la tâche D0.
fn suspendre(c: &mut Cpu, b: &mut Cps2Bus) {
    table(b).suspendre(emplacement(c.d[0]));
    rts(c, b);
}

/// 0xac8 : réveille la tâche D0 avec le compte D1.
fn reveiller(c: &mut Cpu, b: &mut Cps2Bus) {
    table(b).reveiller(emplacement(c.d[0]), c.d[1] as u8);
    rts(c, b);
}

/// `trap #12` (enveloppe 0xaf6) : D0 ≠ 0 masque toutes les interruptions de la tâche, D0 = 0 les autorise (SR du
/// cadre).
fn trap_masque(c: &mut Cpu, b: &mut Cps2Bus) {
    let sr = rd16(b, c.a[7]);
    wr16(b, c.a[7], if c.d[0] as u8 == 0 { sr & 0xf8ff } else { sr | 0x0700 });
    rte(c, b);
}

/// `trap #n` : passe en superviseur, empile PC (`retour`) et SR, saute au handler.
fn prendre_trap(c: &mut Cpu, b: &mut Cps2Bus, handler: u32, retour: u32) {
    let sr = c.sr;
    superviseur(c);
    c.a[7] = c.a[7].wrapping_sub(4);
    wr32(b, c.a[7], retour);
    c.a[7] = c.a[7].wrapping_sub(2);
    wr16(b, c.a[7], sr);
    c.pc = handler;
}

/// Handlers des traps 0 à 6 : seules leurs enveloppes, natives, les prenaient.
fn trap_direct(c: &mut Cpu, _b: &mut Cps2Bus) {
    panic!("trap pris directement par du code 68000 (handler {:x}) : pas de table en RAM", c.pc);
}

macro_rules! natif {
    ($nom:ident, $entree:expr, $corps:expr) => {
        pub const $nom: Native = Native {
            entry: $entree,
            name: stringify!($nom),
            run: $corps,
            touches_sound: false,
            outputs: 0,
        };
    };
}

// ordonnanceur
natif!(ORDONNANCEUR, at::DEBUT, debut);
natif!(PARCOURS, at::PARCOURS, parcours);
natif!(EXAMEN, at::EXAMEN, examen);
natif!(APRES_CESSION, at::SUIVANT, apres_cession);
natif!(RETOUR, at::RETOUR, retour_ordonnanceur);
// enveloppes
natif!(CREER_TACHE, 0x9b6, creer);
natif!(TERMINER_TACHE, 0x9f8, terminer);
natif!(TUER_TACHE, 0xa14, tuer);
natif!(ATTENDRE, 0xa40, |c, b| enveloppe(c, b, attendre));
natif!(CEDER, 0xa86, |c, b| enveloppe(c, b, ceder_prete));
natif!(SUSPENDRE_TACHE, 0xaac, suspendre);
natif!(REVEILLER_TACHE, 0xac8, reveiller);
natif!(RECOMMENCER_TACHE, 0xae4, recommencer);
natif!(MASQUER, 0xaf6, |c, b| prendre_trap(c, b, at::TRAP_MASQUE, 0xaf8));
natif!(ATTENDRE_UNE_IMAGE, 0xb64, |c, b| {
    set_low_word(&mut c.d[0], 1);
    enveloppe(c, b, attendre)
});
// handlers des traps
natif!(TRAP_CREER, 0x9c2, trap_direct);
natif!(TRAP_FINIR, 0x9fa, trap_direct);
natif!(TRAP_TUER, 0xa20, trap_direct);
natif!(TRAP_ATTENDRE, 0xa52, trap_direct);
natif!(TRAP_CEDER, 0xa92, trap_direct);
natif!(TRAP_SUSPENDRE, 0xab8, trap_direct);
natif!(TRAP_REVEILLER, 0xad4, trap_direct);
natif!(TRAP_RECOMMENCER, at::TRAP_RECOMMENCER, recommencer);
natif!(TRAP_MASQUE, at::TRAP_MASQUE, trap_masque);
natif!(REPRISE_MASQUER, 0xaf8, rts);

pub const NATIVES: &[Native] = &[
    ORDONNANCEUR,
    PARCOURS,
    EXAMEN,
    APRES_CESSION,
    RETOUR,
    CREER_TACHE,
    TERMINER_TACHE,
    TUER_TACHE,
    ATTENDRE,
    CEDER,
    SUSPENDRE_TACHE,
    REVEILLER_TACHE,
    RECOMMENCER_TACHE,
    MASQUER,
    ATTENDRE_UNE_IMAGE,
    TRAP_CREER,
    TRAP_FINIR,
    TRAP_TUER,
    TRAP_ATTENDRE,
    TRAP_CEDER,
    TRAP_SUSPENDRE,
    TRAP_REVEILLER,
    TRAP_RECOMMENCER,
    TRAP_MASQUE,
    REPRISE_MASQUER,
];
