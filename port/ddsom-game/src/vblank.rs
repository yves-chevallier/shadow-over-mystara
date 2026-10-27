//! Interruption de VBLANK (K1, 0x574) : une fois par image, avant tout le reste.
//!
//! Dans l'ordre : registres d'affichage CPS-A (défilement des trois plans, 0x722), lecture des
//! entrées (0x80c), registre de contrôle vidéo (0x902), envoi des commandes au Z80 (0x68c), lecture
//! des monnayeurs (0x143da), comptabilité (0x2dcf6), puis réveil des tâches qui attendent (table des
//! tâches, `taches/table.rs`). Tous les registres sont sauvés et rendus.
//!
//! Le code d'origine teste trois octets de la ROM (0xc4, 0xc5, 0xc7), drapeaux de mise au point : tous
//! nuls dans ce jeu (pas d'attente de l'entrée de test, pas de combinaison de dump, réglages DSW
//! forcés à 0xffff). Ils n'apparaissent donc pas ici.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::call;

/// Registres CPS-A (0x804100 + décalage).
mod cps_a {
    pub const BASE: u32 = 0x804100;
    pub const PALETTE: u32 = BASE + 0x0a;
    /// Défilement X des plans 1 à 3 (le Y suit à +2).
    pub const PLAN1_X: u32 = BASE + 0x0c;
    pub const PLAN1_Y: u32 = BASE + 0x0e;
    pub const PLAN2_X: u32 = BASE + 0x10;
    pub const PLAN2_Y: u32 = BASE + 0x12;
    pub const PLAN3_X: u32 = BASE + 0x14;
    pub const PLAN3_Y: u32 = BASE + 0x16;
    /// Défilement par ligne du plan 2 (décalage).
    pub const LIGNES: u32 = BASE + 0x20;
    pub const CONTROLE: u32 = BASE + 0x22;
    /// Registre de page du plan 1 (bit 3 basculé à chaque image en mode 1).
    pub const PAGE_PLAN_1: u32 = BASE + 0x08;
}

/// Ports d'E/S écrits par le VBLANK.
mod port {
    /// Tampon de sortie (0x8040e1) : copie de `A5+0x5bc`.
    pub const SORTIE: u32 = 0x8040e1;
    /// Registre de verrouillage de la table de sprites (0x804170) et valeur écrite.
    pub const CPS_B_REG: u32 = 0x804170;
    /// Entrées de service et affichage (0x804166 : bit de jeu, 0x804040 : voir `inputs.rs`).
    pub const OUTPUT: u32 = 0x400002;
}

/// Variables de la RAM de travail (décalages depuis A5).
mod var {
    /// Compteur d'images du VBLANK.
    pub const IMAGES: u32 = 0x6c;
    /// Valeur de la sortie 0x8040e1.
    pub const SORTIE: u32 = 0x5bc;
    /// Mot remis à zéro à chaque image.
    pub const RAZ: u32 = 0x7fce;
    /// Mot de réglages (DSW).
    pub const DSW: u32 = 0x7fd0;
    /// Décompte de ralentissement des tâches (octet) : une image sur quatre pour la première examinée.
    pub const RALENTI: u32 = 0x4f;
    /// Contrôle vidéo et ombres.
    pub const CONTROLE: u32 = 0x12;
    pub const OMBRES: u32 = 0x26;
        /// Mode de défilement : 1 et 2 alternent les pages / les lignes d'un plan à l'autre.
    pub const MODE_DEFILEMENT: u32 = 0x53c;
    /// Page du plan 1 (bit 3 basculé à chaque image en mode 1).
    pub const PAGE: u32 = 0x534;
    /// Tremblement : nombre d'images restantes (mot ; le bit 0 de l'octet bas donne la phase).
    pub const TREMBLEMENT: u32 = 0x4be;
    /// Alternance du mode 2.
    pub const ALTERNANCE: u32 = 0x54c;
    /// Défilement par ligne affiché (mot).
    pub const LIGNES: u32 = 0x7fb2;
}

/// Défilement d'un plan : le long (X, Y) voulu par le jeu, sa copie d'une image à l'autre (la valeur
/// écrite dans le registre est celle de l'image précédente) et le registre.
struct Plan {
    voulu: u32,
    copie: u32,
    registre: u32,
}

const PLAN_1: Plan = Plan { voulu: 0x4b6, copie: 0x59e, registre: cps_a::PLAN1_X };
const PLAN_2: Plan = Plan { voulu: 0x506, copie: 0x5a6, registre: cps_a::PLAN2_X };
const PLAN_3: Plan = Plan { voulu: 0x556, copie: 0x5ae, registre: cps_a::PLAN3_X };
/// Les longs voulus à +4 de chaque plan portent le défilement vertical affiché dans les registres Y.
const Y_1: Plan = Plan { voulu: 0x4ba, copie: 0x5a2, registre: cps_a::PLAN1_Y };
const Y_2: Plan = Plan { voulu: 0x50a, copie: 0x5aa, registre: cps_a::PLAN2_Y };
const Y_3: Plan = Plan { voulu: 0x55a, copie: 0x5b2, registre: cps_a::PLAN3_Y };

const NOUVELLE_IMAGE: u32 = 0xff0164;
/// Octet remis à zéro par le VBLANK, relu par la boucle de partie (avance rapide, `docs/boucle.md` §1).
const AVANCE_RAPIDE: u32 = A5 - 0x660e;

fn a5(off: u32) -> u32 {
    A5 + off
}

/// Écrit dans le registre X du plan la copie précédente moins la marge de 0x40 pixels, puis copie
/// la valeur voulue.
fn avancer_x(b: &mut Cps2Bus, p: &Plan) {
    let precedent = rd16(b, a5(p.copie));
    wr16(b, p.registre, precedent.wrapping_sub(0x40));
    let voulu = rd32(b, a5(p.voulu));
    wr32(b, a5(p.copie), voulu);
}

/// Idem pour le défilement vertical : le registre reçoit `origine` moins la copie précédente.
fn avancer_y(b: &mut Cps2Bus, p: &Plan, origine: u16) {
    let precedent = rd16(b, a5(p.copie));
    wr16(b, p.registre, origine.wrapping_sub(precedent));
    let voulu = rd32(b, a5(p.voulu));
    wr32(b, a5(p.copie), voulu);
}

/// 0x722 : registres de défilement CPS-A, avec une image de retard, tremblement d'écran et modes
/// d'alternance (`MODE_DEFILEMENT` = 1 : pages ; 2 : lignes).
fn registres_affichage(b: &mut Cps2Bus) {
    let reste = rd16(b, a5(var::TREMBLEMENT));
    if reste != 0 {
        wr16(b, a5(Y_1.voulu), 0x100);
        wr16(b, a5(var::TREMBLEMENT), reste - 1);
        if rd8(b, a5(var::TREMBLEMENT + 1)) & 1 == 0 {
            wr16(b, a5(Y_1.voulu), 0);
        }
    }
    avancer_x(b, &PLAN_1);
    let mode = rd8(b, a5(var::MODE_DEFILEMENT));
    if mode == 1 {
        let x = rd16(b, a5(PLAN_2.copie)).wrapping_sub(0x90);
        wr16(b, a5(PLAN_2.copie), x);
        let page = rd16(b, a5(var::PAGE)) ^ 8;
        wr16(b, a5(var::PAGE), page);
        wr16(b, cps_a::PAGE_PLAN_1, page);
    }
    avancer_x(b, &PLAN_2);
    avancer_x(b, &PLAN_3);
    avancer_y(b, &Y_1, 0x100);
    avancer_y(b, &Y_2, 0x300);
    avancer_y(b, &Y_3, 0x700);
    let lignes = rd16(b, a5(var::LIGNES));
    wr16(b, cps_a::LIGNES, lignes);
    if mode == 2 {
        wr16(b, a5(var::LIGNES), 0);
        wr16(b, a5(Y_2.voulu), 0x28);
        let ancien = rd8(b, a5(var::ALTERNANCE));
        wr8(b, a5(var::ALTERNANCE), ancien ^ 1);
        if ancien & 1 == 0 {
            wr16(b, a5(Y_2.voulu), 0x128);
            wr16(b, a5(var::LIGNES), 0x100);
        }
    }
}

/// 0x902 : registre de contrôle vidéo (bit 15 : ombres) et sortie de service.
fn controle_video(b: &mut Cps2Bus) {
    let mut controle = rd16(b, a5(var::CONTROLE));
    let mut sortie = 0x807d;
    if rd8(b, a5(var::OMBRES)) != 0 {
        controle |= 0x8000;
        sortie = 0x808e;
    }
    wr16(b, cps_a::CONTROLE, controle);
    wr16(b, port::OUTPUT, sortie);
}

/// Réveille les tâches dont le compte d'attente tombe à zéro (table Rust, `taches::table`).
fn reveiller_taches(b: &mut Cps2Bus) {
    let mut ralenti = rd8(b, a5(var::RALENTI));
    crate::taches::table::table(b).decompter_image(&mut ralenti);
    wr8(b, a5(var::RALENTI), ralenti);
}

/// Prise de l'interruption de VBLANK (niveau `niveau`, autovecteur `0x574`) là où le processeur en est (`pc` : adresse
/// d'origine du point interrompu), comme le 68000 : superviseur, PC et SR empilés, niveau dans le SR ; puis le
/// traitement et le `rte`.
pub fn interruption(c: &mut Cpu, b: &mut Cps2Bus, niveau: u8, pc: u32) {
    m68k::Bus::irq_ack(b, niveau);
    let sr = c.sr;
    c.set_sr((sr | SR_S) & !m68k::SR_T);
    c.a[7] = c.a[7].wrapping_sub(4);
    wr32(b, c.a[7], pc);
    c.a[7] = c.a[7].wrapping_sub(2);
    wr16(b, c.a[7], sr);
    c.sr = c.sr & !0x0700 | (niveau as u16) << 8;
    vblank(c, b);
    // `rte`
    c.a[7] = c.a[7].wrapping_add(6);
    c.set_sr(sr);
}

const SR_S: u16 = 0x2000;

/// 0x574 : le traitement de l'interruption ; tous les registres sont rendus.
fn vblank(c: &mut Cpu, b: &mut Cps2Bus) {
    let (d, a) = (c.d, c.a);
    c.a[5] = A5;
    let sortie = rd8(b, a5(var::SORTIE));
    wr8(b, port::SORTIE, sortie);
    wr16(b, a5(var::RAZ), 0);
    wr16(b, port::CPS_B_REG, 0x3f);
    wr16(b, cps_a::PALETTE, 0x91c0);
    wr16(b, a5(var::DSW), 0xffff);
    let images = rd16(b, a5(var::IMAGES)).wrapping_add(1);
    wr16(b, a5(var::IMAGES), images);
    wr8(b, NOUVELLE_IMAGE, 0xff);
    wr8(b, AVANCE_RAPIDE, 0);
    registres_affichage(b);
    call(c, b, 0x80c); // entrées
    controle_video(b);
    call(c, b, 0x68c); // commandes du Z80
    call(c, b, 0x143da); // monnayeurs
    call(c, b, 0x2dcf6); // comptabilité
    reveiller_taches(b);
    // le code d'origine finit par la jauge de charge (0xb3e), inactive tant que DSW a son bit 0x400
    c.d = d;
    c.a = a;
}
