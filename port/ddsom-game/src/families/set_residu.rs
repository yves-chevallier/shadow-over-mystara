//! Petites routines du décor (SET) appelées par des tâches, des scènes ou des gestionnaires d'autres
//! familles : le reste de 68000 qui ne tenait à aucun gestionnaire. Chacune est une native à son adresse
//! d'origine ; les registres qu'elle laisse sont ceux de l'original.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::mem::{prog32, prog8, rd16, rd8, rts, set_low_word, wr16, wr32, wr8, A5};
use crate::{call, regs, Native};

/// Tampon de tuiles de la couche 3 : les colonnes de 44 mots longs effacées par 0xd5f12.
const COLUMNS: [u32; 3] = [0x91_215c, 0x91_2160, 0x91_2164];
const COLUMN_ROWS: u32 = 44;
/// Pas d'une ligne du tampon.
const ROW: u32 = 0x80;
/// Tuile vide (code 0x20, attribut 0).
const BLANK: u32 = 0x20_0000;

/// 0xd5f12 : efface trois colonnes du tampon de tuiles (44 lignes chacune) pour la sortie de scène.
fn erase_columns(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut a1 = 0;
    for column in COLUMNS {
        a1 = column;
        for _ in 0..COLUMN_ROWS {
            wr32(b, a1, BLANK);
            a1 += ROW;
        }
    }
    c.a[1] = a1;
    c.d[0] = 0xffff; // compteur du `dbf` épuisé
    c.d[4] = BLANK;
    c.flags_logic(BLANK, Size::Long);
    rts(c, b);
}

/// Barre de vie du boss (h) : drapeau d'affichage (FF8030) et objet SET de la barre (pointeur court en FF2476).
const LIFE_BAR_SHOWN: u32 = 0x30;
const LIFE_BAR_OBJECT: u32 = 0xff_2476;

/// 0xd5ef8 : quand la barre de vie est affichée, la rattache au boss A0 (`+0x34`, variante 1, état remis à zéro)
/// et efface les colonnes de tuiles de l'ancienne barre (0xd5f12).
fn attach_life_bar(c: &mut Cpu, b: &mut Cps2Bus) {
    let shown = rd8(b, A5 + LIFE_BAR_SHOWN);
    c.flags_logic(shown as u32, Size::Byte);
    if shown == 0 {
        return rts(c, b);
    }
    let bar = rd16(b, LIFE_BAR_OBJECT) as i16 as i32 as u32 & 0xff_ffff;
    c.a[1] = bar;
    wr16(b, bar + 0x34, c.a[0] as u16);
    wr8(b, bar + 3, 1);
    wr32(b, bar + 4, 0);
    erase_columns(c, b);
}

/// 0xce668 : charge deux grilles de blocs (0x26d4, destination 0x917000) dont les numéros sont lus, par
/// paire de mots, dans la table 0xce686 à l'indice D0 (numéro de personnage).
fn load_two_grids(c: &mut Cpu, b: &mut Cps2Bus) {
    const GRIDS: u32 = 0xce686;
    const LOAD_BLOCKS: u32 = 0x26d4;
    const DESTINATION: u32 = 0x91_7000;
    let mut a4 = GRIDS.wrapping_add((c.d[0] as u16).wrapping_mul(4) as u32);
    c.a[1] = DESTINATION;
    for _ in 0..2 {
        set_low_word(&mut c.d[0], rd16(b, a4)); // lecture par (A4)+ : espace données
        a4 += 2;
        c.a[4] = a4;
        call(c, b, LOAD_BLOCKS);
    }
    rts(c, b);
}

/// Routine propre à la variante (`+3`) du générateur : un octet de la table `kinds` donne l'indice dans la
/// table de routines `routines` ; l'indice 0 est un simple RTS.
fn variant_routine(c: &mut Cpu, b: &mut Cps2Bus, kinds: u32, routines: u32) {
    const NOTHING: u32 = 0xd99f8;
    let variant = rd8(b, c.a[0] + 3) as u32;
    c.d[0] = prog8(b, kinds + variant) as u32;
    let index = (c.d[0] as u16).wrapping_mul(4);
    set_low_word(&mut c.d[0], index);
    c.flags_logic(index as u32, Size::Word);
    c.a[1] = prog32(b, routines + index as u32);
    if c.a[1] != NOTHING {
        call(c, b, c.a[1]);
    }
}

/// 0xd98b8 : routine propre à la variante, au démarrage d'une vague (appelée directement par le générateur SET 19,
/// A0 = le générateur ; la native ajoute le `rts`).
pub fn wave_start(c: &mut Cpu, b: &mut Cps2Bus) {
    variant_routine(c, b, 0xd98d8, 0xd98cc);
}

/// 0xd995a : routine propre à la variante, à la fin d'une vague.
pub fn wave_end(c: &mut Cpu, b: &mut Cps2Bus) {
    variant_routine(c, b, 0xd9976, 0xd996e);
}

/// Réglages de la caméra posés par les entrées « appel » des scripts de scène (D0 = valeur) : limite de
/// défilement en X (`A5+0x52c`), position verticale (`A5+0x57c`) et mode de la caméra (`A5+0x4ff`) ; les
/// autres réglages (0x2a36, 0x2a42) sont dans `petites`.
mod camera {
    use super::*;

    const LIMIT_X: u32 = 0x52c;
    const POSITION_Y: u32 = 0x57c;
    const MODE: u32 = 0x4ff;

    /// 0x2a24
    pub fn set_limit_x(c: &mut Cpu, b: &mut Cps2Bus) {
        wr16(b, A5 + LIMIT_X, c.d[0] as u16);
        c.flags_logic(c.d[0] as u16 as u32, Size::Word);
        rts(c, b);
    }

    /// 0x2a2a
    pub fn set_position_y(c: &mut Cpu, b: &mut Cps2Bus) {
        wr16(b, A5 + POSITION_Y, c.d[0] as u16);
        c.flags_logic(c.d[0] as u16 as u32, Size::Word);
        rts(c, b);
    }

    /// 0x2a30
    pub fn set_mode(c: &mut Cpu, b: &mut Cps2Bus) {
        wr8(b, A5 + MODE, c.d[0] as u8);
        c.flags_logic(c.d[0] as u8 as u32, Size::Byte);
        rts(c, b);
    }
}

const fn native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus), outputs: u32) -> Native {
    Native { entry, name, run, touches_sound: false, outputs }
}

pub const NATIVES: &[Native] = &[
    native(0xd5ef8, "set_barre_de_vie", attach_life_bar, regs::ALL),
    native(0xd5f12, "set_efface_colonnes", erase_columns, regs::ALL),
    native(0xce668, "set_deux_grilles", load_two_grids, regs::ALL),
    native(0xd98b8, "set_vague_debut", |c, b| { wave_start(c, b); rts(c, b) }, regs::ALL),
    native(0xd995a, "set_vague_fin", |c, b| { wave_end(c, b); rts(c, b) }, regs::ALL),
    native(0x2a24, "camera_limite_x", camera::set_limit_x, regs::CCR),
    native(0x2a2a, "camera_position_y", camera::set_position_y, regs::CCR),
    native(0x2a30, "camera_mode", camera::set_mode, regs::CCR),
];
