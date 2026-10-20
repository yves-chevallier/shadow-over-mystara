//! Noyau (K1) : ordonnanceur coopératif de tâches (0x928) et traps 0 à 7 et 12.
//!
//! Le jeu fait tourner 16 tâches en alternance. Une tâche est un morceau de code 68000 en mode
//! utilisateur avec sa propre pile ; elle rend la main par un trap (cession), l'ordonnanceur choisit
//! alors la première tâche prête de la table. Cette version native garde exactement la structure de
//! l'original : mêmes tables en RAM, mêmes piles, mêmes registres aux points de reprise. C'est la
//! base du remplacement des tâches par des coroutines Rust (voir `docs/architecture.md`).
//!
//! ## Table des tâches (FF0064, 16 enregistrements de 16 octets)
//!
//! | Octet | Champ |
//! |---|---|
//! | +0 | état : 0 libre, 1 attend (compte à rebours en +1, décompté par le VBLANK), 2 suspendue, 4 prête, 8 en cours, 0xc neuve |
//! | +1 | images restantes avant le réveil |
//! | +2 | SR de reprise (mot) |
//! | +4 | PC de reprise |
//! | +8 | pointeur de pile utilisateur de reprise |
//! | +0xc | pile allouée à la tâche (tirée du *tas de piles* FF005A, rendue à la mort) |
//!
//! L'ordonnanceur (pile du superviseur en FF0E66) parcourt la table dans l'ordre. Entre deux
//! enregistrements, les interruptions sont autorisées (fenêtre de 0x952) ; un VBLANK pose
//! l'indicateur `NOUVELLE_IMAGE` (FF0164) et le parcours reprend alors au début. Quand il trouve une
//! tâche d'état ≥ 4, il empile `0x952`, D0 (indice décroissant), A0 (enregistrement), range ce
//! pointeur de pile en FF0052 et l'enregistrement courant en FF0056, puis lance la tâche. Une tâche
//! qui cède sauvegarde SR/PC/USP dans son enregistrement et saute en `0x984`, qui restaure la pile de
//! l'ordonnanceur et reprend le parcours en `0x952`.
//!
//! ## Traps (vecteurs 32 à 39 et 44)
//!
//! | Trap | Handler | Enveloppe | Effet |
//! |---|---|---|---|
//! | 0 | 0x9c2 | 0x9b6 | crée la tâche D0 (décalage dans la table) avec l'entrée A0 |
//! | 1 | 0x9fa | 0x9f8 | la tâche courante se termine |
//! | 2 | 0xa20 | 0xa14 | tue la tâche D0 |
//! | 3 | 0xa52 | 0xa40 | cède D0 images (0 : suspendue jusqu'au trap 6) |
//! | 4 | 0xa92 | 0xa86 | cède, reste prête |
//! | 5 | 0xab8 | 0xaac | suspend la tâche D0 |
//! | 6 | 0xad4 | 0xac8 | réveille la tâche D0 avec le compte D1 |
//! | 7 | 0xae6 | 0xae4 | la tâche courante recommence en A0 |
//! | 12 | 0xafa | 0xaf6 | masque (D0 ≠ 0) ou démasque les interruptions |
//!
//! Une enveloppe sauve les 15 registres sur la pile utilisateur, déclenche le trap, et sa reprise
//! (`reprise_*`) les restaure avant le `rts`. Les natives des enveloppes empilent les registres puis
//! prennent le trap comme le ferait le processeur ([`prendre_trap`]) : le handler natif qui suit est
//! le même que pour un trap exécuté par du code 68000.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::mem::{rd16, rd32, rd8, rts, set_low_byte, set_low_word, wr16, wr32, wr8, A5};
use crate::Native;

/// Adresses de la RAM.
mod ram {
    /// Table des tâches (16 enregistrements).
    pub const TACHES: u32 = 0xff0064;
    /// Indicateur posé par le VBLANK : le parcours de l'ordonnanceur recommence.
    pub const NOUVELLE_IMAGE: u32 = 0xff0164;
    /// Pointeur de pile de l'ordonnanceur pendant qu'une tâche tourne.
    pub const PILE_ORDONNANCEUR: u32 = 0xff0052;
    /// Enregistrement de la tâche courante.
    pub const COURANTE: u32 = 0xff0056;
    /// Sommet du tas de piles libres.
    pub const TAS_PILES: u32 = 0xff005a;
    /// Mot de réglages lu en VBLANK (DSW) : bits 0 à 2, vitesse du jeu.
    pub const DSW: u32 = 0xffffd0;
    /// Pile du superviseur au démarrage de l'ordonnanceur (A5 - 0x719a).
    pub const PILE_SUPERVISEUR: u32 = super::A5 - 0x719a;
}

/// Champs d'un enregistrement de tâche.
mod champ {
    pub const ETAT: u32 = 0;
    pub const COMPTE: u32 = 1;
    pub const SR: u32 = 2;
    pub const PC: u32 = 4;
    pub const USP: u32 = 8;
    pub const PILE: u32 = 0xc;
}

/// États d'une tâche (octet +0).
mod etat {
    pub const LIBRE: u8 = 0;
    pub const SUSPENDUE: u8 = 2;
    pub const PRETE: u8 = 4;
    pub const EN_COURS: u8 = 8;
    pub const NEUVE: u8 = 0xc;
}

/// Points du code d'origine.
mod at {
    /// Début de l'ordonnanceur : pile, A5.
    pub const DEBUT: u32 = 0x928;
    /// Nouveau parcours de la table.
    pub const PARCOURS: u32 = 0x932;
    /// Examen d'un enregistrement (D0 = reste à voir, A0 = enregistrement).
    pub const EXAMEN: u32 = 0x93e;
    /// Adresse de retour empilée par l'ordonnanceur : passage à l'enregistrement suivant.
    pub const SUIVANT: u32 = 0x952;
    /// Retour à l'ordonnanceur d'une tâche qui cède.
    pub const RETOUR: u32 = 0x984;
    /// Handlers des traps 0 à 7 et 12.
    pub const TRAPS: [u32; 8] = [0x9c2, 0x9fa, 0xa20, 0xa52, 0xa92, 0xab8, 0xad4, 0xae6];
    pub const TRAP_MASQUE: u32 = 0xafa;
}

const SR_SUPERVISEUR: u16 = 0x2000;
const SR_PARCOURS: u16 = 0x2600;

// ---------------------------------------------------------------------------------------------
// Pile et exceptions

fn empiler32(c: &mut Cpu, b: &mut Cps2Bus, v: u32) {
    c.a[7] = c.a[7].wrapping_sub(4);
    wr32(b, c.a[7], v);
}

fn empiler16(c: &mut Cpu, b: &mut Cps2Bus, v: u16) {
    c.a[7] = c.a[7].wrapping_sub(2);
    wr16(b, c.a[7], v);
}

fn depiler32(c: &mut Cpu, b: &mut Cps2Bus) -> u32 {
    let v = rd32(b, c.a[7]);
    c.a[7] = c.a[7].wrapping_add(4);
    v
}

fn depiler16(c: &mut Cpu, b: &mut Cps2Bus) -> u16 {
    let v = rd16(b, c.a[7]);
    c.a[7] = c.a[7].wrapping_add(2);
    v
}

/// RTE : dépile SR puis PC.
fn rte(c: &mut Cpu, b: &mut Cps2Bus) {
    let sr = depiler16(c, b);
    c.pc = depiler32(c, b);
    c.set_sr(sr);
}

/// `trap #n` : passe en superviseur, empile PC (`retour`) et SR, saute au handler.
fn prendre_trap(c: &mut Cpu, b: &mut Cps2Bus, handler: u32, retour: u32) {
    let sr = c.sr;
    c.set_sr((sr | SR_SUPERVISEUR) & !m68k::SR_T);
    empiler32(c, b, retour);
    empiler16(c, b, sr);
    c.pc = handler;
}

/// Sauvegarde des 15 registres (`movem.l {D0-D7/A0-A6},-(SP)` : D0 à l'adresse la plus basse).
fn sauver_registres(c: &mut Cpu, b: &mut Cps2Bus) {
    for k in (0..7).rev() {
        let v = c.a[k];
        empiler32(c, b, v);
    }
    for k in (0..8).rev() {
        let v = c.d[k];
        empiler32(c, b, v);
    }
}

/// Restauration des 15 registres, puis `rts`.
fn restaurer_registres_rts(c: &mut Cpu, b: &mut Cps2Bus) {
    for k in 0..8 {
        c.d[k] = depiler32(c, b);
    }
    for k in 0..7 {
        c.a[k] = depiler32(c, b);
    }
    rts(c, b);
}

fn tache(d: u32) -> u32 {
    ram::TACHES + (d & 0xffff)
}

/// Rend la pile d'une tâche morte au tas.
fn rendre_pile(b: &mut Cps2Bus, enregistrement: u32) {
    let pile = rd32(b, enregistrement + champ::PILE);
    let tas = rd32(b, ram::TAS_PILES).wrapping_sub(4);
    wr32(b, tas, pile);
    wr32(b, ram::TAS_PILES, tas);
}

// ---------------------------------------------------------------------------------------------
// Ordonnanceur

/// 0x928 : l'ordonnanceur prend la main (appelé une fois, au démarrage et par `jmp`).
fn debut(c: &mut Cpu, _b: &mut Cps2Bus) {
    c.a[7] = ram::PILE_SUPERVISEUR;
    c.a[5] = A5;
    c.pc = at::PARCOURS;
}

/// 0x932 : nouveau parcours de la table.
fn parcours(c: &mut Cpu, b: &mut Cps2Bus) {
    wr8(b, ram::NOUVELLE_IMAGE, 0);
    c.a[0] = ram::TACHES;
    set_low_word(&mut c.d[0], 15);
    c.pc = at::EXAMEN;
}

/// Passe à l'enregistrement suivant (`dbf`), ou recommence quand la table est épuisée.
fn suivant(c: &mut Cpu) {
    c.a[0] += 0x10;
    let reste = (c.d[0] as u16).wrapping_sub(1);
    set_low_word(&mut c.d[0], reste);
    c.pc = if reste == 0xffff { at::PARCOURS } else { at::EXAMEN };
}

/// 0x93e : examen d'un enregistrement, avec interruptions masquées (jusqu'au niveau 6). Chaque
/// enregistrement est une exécution séparée : l'interruption de la fenêtre de 0x952 peut tomber entre deux.
fn examen(c: &mut Cpu, b: &mut Cps2Bus) {
    c.set_sr(SR_PARCOURS);
    if rd8(b, ram::NOUVELLE_IMAGE) != 0 {
        c.pc = at::PARCOURS;
        return;
    }
    let etat = rd8(b, c.a[0]);
    set_low_byte(&mut c.d[1], etat);
    if etat >= etat::PRETE {
        lancer(c, b, etat);
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

/// 0x960 : lance la tâche de l'enregistrement A0.
fn lancer(c: &mut Cpu, b: &mut Cps2Bus, etat: u8) {
    let (d0, a0) = (c.d[0] as u16, c.a[0]);
    empiler32(c, b, at::SUIVANT);
    empiler16(c, b, d0);
    empiler32(c, b, a0);
    wr32(b, ram::PILE_ORDONNANCEUR, c.a[7]);
    wr32(b, ram::COURANTE, c.a[0]);
    set_low_word(&mut c.d[1], etat as i8 as i16 as u16);
    let t = c.a[0];
    match etat {
        etat::PRETE | etat::EN_COURS => {
            if etat == etat::PRETE {
                wr8(b, t + champ::ETAT, etat::EN_COURS);
            }
            // reprise : SR et PC de la tâche, pile utilisateur en USP, puis RTE
            let (pc, sr, usp) = (rd32(b, t + champ::PC), rd16(b, t + champ::SR), rd32(b, t + champ::USP));
            empiler32(c, b, pc);
            empiler16(c, b, sr);
            c.a[1] = usp;
            c.other_sp = usp;
            rte(c, b);
        }
        etat::NEUVE => {
            // départ : A0 reste l'enregistrement, A1 l'entrée, A4 la pile, SR = 0
            wr8(b, t + champ::ETAT, etat::EN_COURS);
            c.a[1] = rd32(b, t + champ::PC);
            c.a[4] = rd32(b, t + champ::PILE);
            c.other_sp = c.a[4];
            c.set_sr(0);
            c.pc = c.a[1];
        }
        _ => unreachable!("état de tâche {etat:#x} : l'original saute dans ses données"),
    }
}

/// 0x984 : une tâche a cédé ; l'ordonnanceur retrouve sa pile et reprend en 0x952.
fn retour_ordonnanceur(c: &mut Cpu, b: &mut Cps2Bus) {
    c.a[7] = rd32(b, ram::PILE_ORDONNANCEUR);
    c.a[0] = depiler32(c, b);
    let d0 = depiler16(c, b);
    set_low_word(&mut c.d[0], d0);
    rts(c, b);
}

/// Cession : range l'état de la tâche courante (`mot` dans l'état et le compte, SR/PC dépilés du
/// cadre du trap, pile utilisateur) puis rend la main à l'ordonnanceur.
fn ceder(c: &mut Cpu, b: &mut Cps2Bus, mot: u16) {
    let t = rd32(b, ram::COURANTE);
    c.a[0] = t;
    wr16(b, t, mot);
    c.a[1] = c.other_sp;
    wr32(b, t + champ::USP, c.a[1]);
    let sr = depiler16(c, b);
    wr16(b, t + champ::SR, sr);
    let pc = depiler32(c, b);
    wr32(b, t + champ::PC, pc);
    c.pc = at::RETOUR;
}

// ---------------------------------------------------------------------------------------------
// Handlers des traps

/// Trap 0 : crée la tâche D0 (décalage dans la table), entrée A0. Une tâche déjà là rend sa pile.
fn trap_creer(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[0] &= 0xffff_00ff;
    c.a[4] = ram::TACHES;
    let t = tache(c.d[0]);
    if rd8(b, t) != etat::LIBRE {
        rendre_pile(b, t);
    }
    let tas = rd32(b, ram::TAS_PILES);
    c.a[1] = rd32(b, tas);
    wr32(b, ram::TAS_PILES, tas + 4);
    c.a[6] = tas + 4;
    wr32(b, t + champ::PILE, c.a[1]);
    wr16(b, t, (etat::NEUVE as u16) << 8);
    wr32(b, t + champ::PC, c.a[0]);
    rte(c, b);
}

/// Trap 1 : la tâche courante se termine et rend sa pile.
fn trap_finir(c: &mut Cpu, b: &mut Cps2Bus) {
    c.a[7] = c.a[7].wrapping_add(6);
    let t = rd32(b, ram::COURANTE);
    c.a[0] = t;
    wr8(b, t, etat::LIBRE);
    rendre_pile(b, t);
    c.pc = at::RETOUR;
}

/// Trap 2 : tue la tâche D0 (décalage dans la table).
fn trap_tuer(c: &mut Cpu, b: &mut Cps2Bus) {
    c.a[4] = ram::TACHES;
    let t = tache(c.d[0]);
    if rd8(b, t) != etat::LIBRE {
        wr8(b, t, etat::LIBRE);
        rendre_pile(b, t);
    }
    rte(c, b);
}

/// Trap 3 : attend D0 (octet) images, plus le retard demandé par les réglages ; 0 : suspendue.
fn trap_attendre(c: &mut Cpu, b: &mut Cps2Bus) {
    let reglage = !rd16(b, ram::DSW) & 7;
    let compte = (c.d[0] as u8).wrapping_add(reglage as u8);
    set_low_byte(&mut c.d[0], compte);
    set_low_word(&mut c.d[1], if compte == 0 { 0x200 } else { 0x100 });
    let mot = c.d[0] as u16 | c.d[1] as u16;
    set_low_word(&mut c.d[0], mot);
    ceder(c, b, mot);
}

/// Trap 4 : cède la main en restant prête.
fn trap_ceder(c: &mut Cpu, b: &mut Cps2Bus) {
    let t = rd32(b, ram::COURANTE);
    c.a[0] = t;
    wr8(b, t, etat::PRETE);
    c.a[1] = c.other_sp;
    wr32(b, t + champ::USP, c.a[1]);
    let sr = depiler16(c, b);
    wr16(b, t + champ::SR, sr);
    let pc = depiler32(c, b);
    wr32(b, t + champ::PC, pc);
    c.pc = at::RETOUR;
}

/// Trap 5 : suspend la tâche D0.
fn trap_suspendre(c: &mut Cpu, b: &mut Cps2Bus) {
    c.a[4] = ram::TACHES;
    let t = tache(c.d[0]);
    wr8(b, t + champ::ETAT, etat::SUSPENDUE);
    wr8(b, t + champ::COMPTE, 0);
    rte(c, b);
}

/// Trap 6 : réveille la tâche D0 avec le compte D1.
fn trap_reveiller(c: &mut Cpu, b: &mut Cps2Bus) {
    c.a[4] = ram::TACHES;
    let t = tache(c.d[0]);
    wr8(b, t + champ::ETAT, etat::PRETE);
    wr8(b, t + champ::COMPTE, c.d[1] as u8);
    rte(c, b);
}

/// Trap 7 : la tâche courante repart de zéro en A0 (même pile).
fn trap_recommencer(c: &mut Cpu, b: &mut Cps2Bus) {
    let t = rd32(b, ram::COURANTE);
    c.a[4] = t;
    wr16(b, t, (etat::NEUVE as u16) << 8);
    wr32(b, t + champ::PC, c.a[0]);
    c.pc = at::RETOUR;
}

/// Trap 12 : D0 ≠ 0 masque toutes les interruptions de la tâche, D0 = 0 les autorise (SR du cadre).
fn trap_masque(c: &mut Cpu, b: &mut Cps2Bus) {
    let sr = rd16(b, c.a[7]);
    wr16(b, c.a[7], if c.d[0] as u8 == 0 { sr & 0xf8ff } else { sr | 0x0700 });
    rte(c, b);
}

// ---------------------------------------------------------------------------------------------
// Enveloppes : sauvent les registres, prennent le trap, la reprise les restaure

fn enveloppe(c: &mut Cpu, b: &mut Cps2Bus, trap: usize, reprise: u32) {
    sauver_registres(c, b);
    prendre_trap(c, b, at::TRAPS[trap], reprise);
}

/// Cession d'au moins une image (0xa40, 0xb64) : l'enveloppe d'origine appelle d'abord 0xb40, qui
/// charge D1 et, selon DSW, remplit un bloc de la GFX RAM de la jauge de charge (jamais, dans ce jeu : bit 0x400 de DSW).
/// L'adresse de retour de cet appel reste sous la pile ; les drapeaux sont ceux du test de DSW.
fn attendre(c: &mut Cpu, b: &mut Cps2Bus) {
    sauver_registres(c, b);
    wr32(b, c.a[7].wrapping_sub(4), 0xa4a);
    c.sr &= !0x0f;
    prendre_trap(c, b, at::TRAPS[3], 0xa4c);
}

fn attendre_une_image(c: &mut Cpu, b: &mut Cps2Bus) {
    set_low_word(&mut c.d[0], 1);
    attendre(c, b);
}

macro_rules! natif {
    ($nom:ident, $entree:expr, $corps:expr) => {
        pub const $nom: Native = Native {
            entry: $entree,
            name: stringify!($nom),
            cost: |_, _| 60,
            run: $corps,
            touches_sound: false,
            outputs: 0,
            exact_cost: false,
        };
    };
}

// ordonnanceur
natif!(ORDONNANCEUR, at::DEBUT, debut);
natif!(PARCOURS, at::PARCOURS, parcours);
natif!(EXAMEN, at::EXAMEN, examen);
natif!(APRES_CESSION, at::SUIVANT, apres_cession);
natif!(RETOUR, at::RETOUR, retour_ordonnanceur);
// handlers de traps
natif!(TRAP_CREER, 0x9c2, trap_creer);
natif!(TRAP_FINIR, 0x9fa, trap_finir);
natif!(TRAP_TUER, 0xa20, trap_tuer);
natif!(TRAP_ATTENDRE, 0xa52, trap_attendre);
natif!(TRAP_CEDER, 0xa92, trap_ceder);
natif!(TRAP_SUSPENDRE, 0xab8, trap_suspendre);
natif!(TRAP_REVEILLER, 0xad4, trap_reveiller);
natif!(TRAP_RECOMMENCER, 0xae6, trap_recommencer);
natif!(TRAP_MASQUE, at::TRAP_MASQUE, trap_masque);
// enveloppes
natif!(CREER_TACHE, 0x9b6, |c, b| enveloppe(c, b, 0, 0x9bc));
natif!(TERMINER_TACHE, 0x9f8, |c, b| prendre_trap(c, b, at::TRAPS[1], 0x9fa));
natif!(TUER_TACHE, 0xa14, |c, b| enveloppe(c, b, 2, 0xa1a));
natif!(ATTENDRE, 0xa40, attendre);
natif!(CEDER, 0xa86, |c, b| enveloppe(c, b, 4, 0xa8c));
natif!(SUSPENDRE_TACHE, 0xaac, |c, b| enveloppe(c, b, 5, 0xab2));
natif!(REVEILLER_TACHE, 0xac8, |c, b| enveloppe(c, b, 6, 0xace));
natif!(RECOMMENCER_TACHE, 0xae4, |c, b| prendre_trap(c, b, at::TRAPS[7], 0xae6));
natif!(MASQUER, 0xaf6, |c, b| prendre_trap(c, b, at::TRAP_MASQUE, 0xaf8));
natif!(ATTENDRE_UNE_IMAGE, 0xb64, attendre_une_image);
// reprises : restauration des registres et retour à l'appelant de l'enveloppe
natif!(REPRISE_CREER, 0x9bc, restaurer_registres_rts);
natif!(REPRISE_TUER, 0xa1a, restaurer_registres_rts);
natif!(REPRISE_ATTENDRE, 0xa4c, restaurer_registres_rts);
natif!(REPRISE_CEDER, 0xa8c, restaurer_registres_rts);
natif!(REPRISE_SUSPENDRE, 0xab2, restaurer_registres_rts);
natif!(REPRISE_REVEILLER, 0xace, restaurer_registres_rts);
natif!(REPRISE_MASQUER, 0xaf8, rts);

pub const NATIVES: &[Native] = &[
    ORDONNANCEUR,
    PARCOURS,
    EXAMEN,
    APRES_CESSION,
    RETOUR,
    TRAP_CREER,
    TRAP_FINIR,
    TRAP_TUER,
    TRAP_ATTENDRE,
    TRAP_CEDER,
    TRAP_SUSPENDRE,
    TRAP_REVEILLER,
    TRAP_RECOMMENCER,
    TRAP_MASQUE,
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
    REPRISE_CREER,
    REPRISE_TUER,
    REPRISE_ATTENDRE,
    REPRISE_CEDER,
    REPRISE_SUSPENDRE,
    REPRISE_REVEILLER,
    REPRISE_MASQUER,
];
