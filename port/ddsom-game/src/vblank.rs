//! Interruption de VBLANK (K1, 0x574) : une fois par image, avant tout le reste.
//!
//! Dans l'ordre : registres d'affichage CPS-A (défilement des trois plans, 0x722), lecture des
//! entrées (0x80c), registre de contrôle vidéo (0x902), envoi des commandes au Z80 (0x68c), lecture
//! des monnayeurs (0x143da), comptabilité (0x2dcf6), puis réveil des tâches qui attendent (table des
//! tâches, `tasks/table.rs`). Tous les registres sont sauvés et rendus.
//!
//! Le code d'origine teste trois octets de la ROM (0xc4, 0xc5, 0xc7), drapeaux de mise au point : tous
//! nuls dans ce jeu (pas d'attente de l'entrée de test, pas de combinaison de dump, réglages DSW
//! forcés à 0xffff). Ils n'apparaissent donc pas ici.

use cps2::Cps2Bus;

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};

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
    pub const LINES: u32 = BASE + 0x20;
    pub const CONTROL: u32 = BASE + 0x22;
    /// Registre de page du plan 1 (bit 3 basculé à chaque image en mode 1).
    pub const PAGE_PLAN_1: u32 = BASE + 0x08;
}

/// Ports d'E/S écrits par le VBLANK.
mod port {
    /// Tampon de sortie (0x8040e1) : copie de `A5+0x5bc`.
    pub const EXIT: u32 = 0x8040e1;
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
    pub const EXIT: u32 = 0x5bc;
    /// Mot remis à zéro à chaque image.
    pub const RAZ: u32 = 0x7fce;
    /// Mot de réglages (DSW).
    pub const DSW: u32 = 0x7fd0;
    /// Décompte de ralentissement des tâches (octet) : une image sur quatre pour la première examinée.
    pub const SLOWDOWN: u32 = 0x4f;
    /// Contrôle vidéo et ombres.
    pub const CONTROL: u32 = 0x12;
    pub const SHADOWS: u32 = 0x26;
        /// Mode de défilement : 1 et 2 alternent les pages / les lignes d'un plan à l'autre.
    pub const SCROLL_MODE: u32 = 0x53c;
    /// Page du plan 1 (bit 3 basculé à chaque image en mode 1).
    pub const PAGE: u32 = 0x534;
    /// Tremblement : nombre d'images restantes (mot ; le bit 0 de l'octet bas donne la phase).
    pub const QUAKE: u32 = 0x4be;
    /// Alternance du mode 2.
    pub const ALTERNATION: u32 = 0x54c;
    /// Défilement par ligne affiché (mot).
    pub const LINES: u32 = 0x7fb2;
}

/// Défilement d'un plan : le long (X, Y) voulu par le jeu, sa copie d'une image à l'autre (la valeur
/// écrite dans le registre est celle de l'image précédente) et le registre.
struct Plan {
    wanted: u32,
    copy: u32,
    register: u32,
}

const PLAN_1: Plan = Plan { wanted: 0x4b6, copy: 0x59e, register: cps_a::PLAN1_X };
const PLAN_2: Plan = Plan { wanted: 0x506, copy: 0x5a6, register: cps_a::PLAN2_X };
const PLAN_3: Plan = Plan { wanted: 0x556, copy: 0x5ae, register: cps_a::PLAN3_X };
/// Les longs voulus à +4 de chaque plan portent le défilement vertical affiché dans les registres Y.
const Y_1: Plan = Plan { wanted: 0x4ba, copy: 0x5a2, register: cps_a::PLAN1_Y };
const Y_2: Plan = Plan { wanted: 0x50a, copy: 0x5aa, register: cps_a::PLAN2_Y };
const Y_3: Plan = Plan { wanted: 0x55a, copy: 0x5b2, register: cps_a::PLAN3_Y };

const NEW_FRAME: u32 = 0xff0164;
/// Octet remis à zéro par le VBLANK, relu par la boucle de partie (avance rapide, `docs/looped.md` §1).
const FAST_FORWARD: u32 = A5 - 0x660e;

fn a5(off: u32) -> u32 {
    A5 + off
}

/// Écrit dans le registre X du plan la copie précédente moins la marge de 0x40 pixels, puis copie
/// la valeur voulue.
fn advance_x(b: &mut Cps2Bus, p: &Plan) {
    let previous = rd16(b, a5(p.copy));
    wr16(b, p.register, previous.wrapping_sub(0x40));
    let wanted = rd32(b, a5(p.wanted));
    wr32(b, a5(p.copy), wanted);
}

/// Idem pour le défilement vertical : le registre reçoit `original` moins la copie précédente.
fn advance_y(b: &mut Cps2Bus, p: &Plan, original: u16) {
    let previous = rd16(b, a5(p.copy));
    wr16(b, p.register, original.wrapping_sub(previous));
    let wanted = rd32(b, a5(p.wanted));
    wr32(b, a5(p.copy), wanted);
}

/// 0x722 : registres de défilement CPS-A, avec une image de retard, tremblement d'écran et modes
/// d'alternance (`SCROLL_MODE` = 1 : pages ; 2 : lignes).
fn display_regs(b: &mut Cps2Bus) {
    let remainder = rd16(b, a5(var::QUAKE));
    if remainder != 0 {
        wr16(b, a5(Y_1.wanted), 0x100);
        wr16(b, a5(var::QUAKE), remainder - 1);
        if rd8(b, a5(var::QUAKE + 1)) & 1 == 0 {
            wr16(b, a5(Y_1.wanted), 0);
        }
    }
    advance_x(b, &PLAN_1);
    let mode = rd8(b, a5(var::SCROLL_MODE));
    if mode == 1 {
        let x = rd16(b, a5(PLAN_2.copy)).wrapping_sub(0x90);
        wr16(b, a5(PLAN_2.copy), x);
        let page = rd16(b, a5(var::PAGE)) ^ 8;
        wr16(b, a5(var::PAGE), page);
        wr16(b, cps_a::PAGE_PLAN_1, page);
    }
    advance_x(b, &PLAN_2);
    advance_x(b, &PLAN_3);
    advance_y(b, &Y_1, 0x100);
    advance_y(b, &Y_2, 0x300);
    advance_y(b, &Y_3, 0x700);
    let lines = rd16(b, a5(var::LINES));
    wr16(b, cps_a::LINES, lines);
    if mode == 2 {
        wr16(b, a5(var::LINES), 0);
        wr16(b, a5(Y_2.wanted), 0x28);
        let old = rd8(b, a5(var::ALTERNATION));
        wr8(b, a5(var::ALTERNATION), old ^ 1);
        if old & 1 == 0 {
            wr16(b, a5(Y_2.wanted), 0x128);
            wr16(b, a5(var::LINES), 0x100);
        }
    }
}

/// 0x902 : registre de contrôle vidéo (bit 15 : ombres) et sortie de service.
fn video_control(b: &mut Cps2Bus) {
    let mut control = rd16(b, a5(var::CONTROL));
    let mut exit = 0x807d;
    if rd8(b, a5(var::SHADOWS)) != 0 {
        control |= 0x8000;
        exit = 0x808e;
    }
    wr16(b, cps_a::CONTROL, control);
    wr16(b, port::OUTPUT, exit);
}

/// Réveille les tâches dont le compte d'attente tombe à zéro (table Rust, `tasks::table`).
fn wake_tasks(b: &mut Cps2Bus) {
    let mut slowdown = rd8(b, a5(var::SLOWDOWN));
    crate::tasks::table::table(b).count_down_frame(&mut slowdown);
    wr8(b, a5(var::SLOWDOWN), slowdown);
}

/// L'interruption de VBLANK (niveau `level`, autovecteur `0x574`), prise par le noyau entre deux points
/// (`kernel::interrupt`) : acquittement, puis le traitement. Ni registre ni pile : l'original passe en superviseur,
/// empile PC et SR et relève son masque, mais son traitement sauve et rend tous les registres et ne peut être
/// interrompu ; le `rte` rend le SR. Le point où revenir est gardé par le noyau (`Table::interrupted`).
///
/// Écart assumé : PC et SR ne sont plus écrits sur la pile courante (pile de la tâche interrompue, FF01A6-FF0DA5, ou
/// du superviseur, sous FF0E66). `validate` ignore ces mots (`tasks::table::kernel_ram`), et le jeu ne les relit pas
/// (preuve : docs/refactorisation.md, lot « sortie du Cpu », partie 4).
pub fn interruption(b: &mut Cps2Bus, level: u8) {
    m68k::Bus::irq_ack(b, level);
    vblank(b);
}

/// 0x574 : le traitement de l'interruption. L'original sauve et rend tous les registres : ce qu'y laissent ses
/// services ne sort pas.
fn vblank(b: &mut Cps2Bus) {
    let exit = rd8(b, a5(var::EXIT));
    wr8(b, port::EXIT, exit);
    wr16(b, a5(var::RAZ), 0);
    wr16(b, port::CPS_B_REG, 0x3f);
    wr16(b, cps_a::PALETTE, 0x91c0);
    wr16(b, a5(var::DSW), 0xffff);
    let images = rd16(b, a5(var::IMAGES)).wrapping_add(1);
    wr16(b, a5(var::IMAGES), images);
    wr8(b, NEW_FRAME, 0xff);
    wr8(b, FAST_FORWARD, 0);
    display_regs(b);
    // les services rendent des valeurs que l'original laisse dans ses registres ; le VBLANK les rend tous
    crate::inputs::read_panel(b); // 0x80c
    video_control(b);
    crate::sound::send(b); // 0x68c : commandes du Z80
    crate::credits::read_coin_slots(b); // 0x143da
    crate::eeprom::vblank(b); // 0x2dcf6 : comptabilité
    wake_tasks(b);
    // le code d'origine finit par la jauge de charge (0xb3e), inactive tant que DSW a son bit 0x400
}
