//! Entrées des joueurs.
//!
//! Chaque image, `read_inputs` (appelée par le VBLANK) relit le panneau. La RAM conserve l'état
//! courant (actif à 1), l'état précédent et les nouveaux appuis. Puis `copy_inputs_to_players`
//! recopie ces octets, masqués, dans l'enregistrement de chaque joueur.
//!
//! Bits d'un octet joueur : 0 droite, 1 gauche, 2 bas, 3 haut, 4..7 boutons 1..4.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::mem::{rd16, rd8, rts, set_low_byte, wr16, wr8, A5};
use crate::Native;

/// Décalages par rapport à A5 (RAM FF8000).
pub mod ram {
    /// État courant : P1, P2, P3, P4, système bas, système haut.
    pub const CUR: u32 = 0x00;
    /// État de l'image précédente (même ordre).
    pub const PREV: u32 = 0x06;
    /// Nouveaux appuis de P1..P4.
    pub const NEW: u32 = 0x0c;
    /// Mode 4 joueurs (0 : les boutons de P3 complètent ceux de P1/P2).
    pub const FOUR_PLAYERS: u32 = 0x20;
    /// Masque d'entrées appliqué aux joueurs (0 pendant les cinématiques).
    pub const INPUT_MASK: u32 = 0x43;
    /// Entrées gelées : les joueurs reçoivent l'état brut, limité à droite/gauche.
    pub const INPUT_RAW: u32 = 0x44;
    /// Copie du registre CPS-B 0x26 (contrôle des couches).
    pub const LAYER_CTRL: u32 = 0x14;
    /// Copie du port 804040 (EEPROM, pièces).
    pub const EEPROM_PORT: u32 = 0x16;
    /// Le port EEPROM est piloté ailleurs (accès EEPROM en cours).
    pub const EEPROM_BUSY: u32 = 0x74;
    /// Couches de décor coupées (A5 - 0x7155).
    pub const LAYERS_OFF: u32 = 0xffff_8eab;
    /// Premier enregistrement joueur ; les suivants sont espacés de 0x200.
    pub const PLAYER1: u32 = 0x5de;
    pub const PLAYER_STRIDE: u32 = 0x200;
    /// Dans un enregistrement joueur : entrées maintenues, précédentes, nouvelles.
    pub const P_HELD: u32 = 0x6c;
    pub const P_PREV: u32 = 0x6d;
    pub const P_NEW: u32 = 0x6e;
}

/// Ports du panneau, dans l'ordre où l'original les lit (octet poids faible d'abord).
const PORTS: [u32; 6] = [0x804001, 0x804000, 0x804011, 0x804010, 0x804020, 0x804021];

fn set_nz_byte(c: &mut Cpu, v: u8) {
    c.flags_logic(v as u32, Size::Byte);
}

/// 0x080c (VBLANK) : lecture du panneau. Rend ce que l'original laisse dans ses registres (`EntreesLues`).
pub fn lire_panneau(b: &mut Cps2Bus) -> EntreesLues {
    let a5 = A5;
    let four_players = rd8(b, a5 + ram::FOUR_PLAYERS) != 0;
    let layers_on = rd8(b, a5.wrapping_add(ram::LAYERS_OFF)) == 0;
    let eeprom_busy = rd8(b, a5 + ram::EEPROM_BUSY) != 0;

    let mut cur = [0u8; 6];
    for i in 0..6 {
        let old = rd8(b, a5 + ram::CUR + i as u32);
        wr8(b, a5 + ram::PREV + i as u32, old);
        cur[i] = !rd8(b, PORTS[i]);
        wr8(b, a5 + ram::CUR + i as u32, cur[i]);
    }
    let prev: Vec<u8> = (0..4).map(|i| rd8(b, a5 + ram::PREV + i)).collect();
    let pressed: Vec<u8> = (0..4).map(|i| cur[i] & !prev[i]).collect();
    for (i, &p) in pressed.iter().enumerate() {
        wr8(b, a5 + ram::NEW + i as u32, p);
    }

    let d0 = if four_players {
        cur[2]
    } else {
        // cabinet 2 joueurs : le bouton 1 de P3 devient le bouton 4 de P2, la droite de P3 le
        // bouton 4 de P1 (câblage des boutons supplémentaires)
        let p2_b4 = (cur[2] & 0x10) << 3;
        let p1_b4 = (cur[2] & 0x01) << 7;
        wr8(b, a5 + ram::CUR + 1, cur[1] | p2_b4);
        wr8(b, a5 + ram::CUR, cur[0] | p1_b4);
        p1_b4
    };

    if !layers_on {
        let v = rd16(b, a5 + ram::LAYER_CTRL) & 0xfff1;
        wr16(b, a5 + ram::LAYER_CTRL, v);
    }
    let layer_ctrl = rd16(b, a5 + ram::LAYER_CTRL);
    wr16(b, 0x804166, layer_ctrl);
    let dernier = if eeprom_busy {
        Dernier::Octet(rd8(b, a5 + ram::EEPROM_BUSY))
    } else {
        let v = rd16(b, a5 + ram::EEPROM_PORT);
        wr16(b, 0x804040, v);
        Dernier::Mot(v)
    };
    EntreesLues { d: [d0, cur[3], pressed[2], pressed[3]], quatre_joueurs: four_players, dernier }
}

/// Dernière valeur testée par 0x80c (drapeaux de l'original).
#[derive(Clone, Copy)]
pub enum Dernier {
    Octet(u8),
    Mot(u16),
}

/// Restes de 0x80c dans les registres de l'original : octets bas de D0-D3, X effacé (cabinet à deux joueurs),
/// drapeaux de la dernière valeur testée.
#[derive(Clone, Copy)]
pub struct EntreesLues {
    pub d: [u8; 4],
    pub quatre_joueurs: bool,
    pub dernier: Dernier,
}

fn read_inputs(c: &mut Cpu, b: &mut Cps2Bus) {
    let r = lire_panneau(b);
    for (n, v) in r.d.into_iter().enumerate() {
        set_low_byte(&mut c.d[n], v);
    }
    if !r.quatre_joueurs {
        // lsl.b #3 sur (cur2 & 0x10) : le dernier bit sorti vaut 0, X et C passent à 0
        c.sr &= !SR_X;
    }
    match r.dernier {
        Dernier::Octet(v) => set_nz_byte(c, v),
        Dernier::Mot(v) => c.flags_logic(v as u32, Size::Word),
    }
    rts(c, b);
}

pub const READ_INPUTS: Native = Native {
    entry: 0x080c,
    name: "read_inputs",
    run: crate::Corps::Processeur(read_inputs),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

/// 0x2aec : recopie les entrées dans les enregistrements des joueurs. Rend les restes de l'original (`Recopie`).
pub fn recopier(b: &mut Cps2Bus) -> Recopie {
    let a5 = A5;
    let player = |k: u32| a5 + ram::PLAYER1 + ram::PLAYER_STRIDE * k;
    if rd8(b, a5 + ram::INPUT_RAW) != 0 {
        // entrées gelées : état brut limité à droite/gauche, aucun nouvel appui
        for k in 0..4 {
            let held = rd8(b, a5 + ram::CUR + k) & 3;
            let prev = rd8(b, a5 + ram::PREV + k) & 3;
            wr8(b, player(k) + ram::P_HELD, held);
            wr8(b, player(k) + ram::P_PREV, prev);
        }
        for k in 0..4 {
            wr8(b, player(k) + ram::P_NEW, 0);
        }
        Recopie::Gelees
    } else {
        let mask = rd8(b, a5 + ram::INPUT_MASK);
        let mut regs = [0u8; 4];
        for pair in [0u32, 2] {
            for (j, k) in [pair, pair + 1].into_iter().enumerate() {
                let held = rd8(b, a5 + ram::CUR + k) & mask;
                let prev = rd8(b, a5 + ram::PREV + k) & mask;
                let new = held & !prev;
                wr8(b, player(k) + ram::P_HELD, held);
                wr8(b, player(k) + ram::P_PREV, prev);
                wr8(b, player(k) + ram::P_NEW, new);
                regs[j] = held;
                regs[2 + j] = new;
            }
        }
        Recopie::Masquees { masque: mask, d: regs }
    }
}

/// Restes de 0x2aec dans les registres de l'original.
#[derive(Clone, Copy)]
pub enum Recopie {
    /// Entrées gelées : drapeaux du `clr.b` final.
    Gelees,
    /// D4.b = masque, D0-D3.b = entrées et nouveaux appuis de la dernière paire de joueurs ; drapeaux de D3.b.
    Masquees { masque: u8, d: [u8; 4] },
}

/// `recopier` avec les registres de l'original.
pub fn recopier_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    match recopier(b) {
        // clr.b final
        Recopie::Gelees => c.sr = (c.sr & !(SR_N | SR_V | SR_C)) | SR_Z,
        Recopie::Masquees { masque, d } => {
            set_low_byte(&mut c.d[4], masque);
            for (n, v) in d.into_iter().enumerate() {
                set_low_byte(&mut c.d[n], v);
            }
            // move.b d3,(...) final
            set_nz_byte(c, d[3]);
        }
    }
}

fn copy_inputs_to_players(c: &mut Cpu, b: &mut Cps2Bus) {
    recopier_68k(c, b);
    rts(c, b);
}

pub const COPY_INPUTS_TO_PLAYERS: Native = Native {
    entry: 0x2aec,
    name: "copy_inputs_to_players",
    run: crate::Corps::Processeur(copy_inputs_to_players),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

/// Entrées d'un joueur : touches maintenues et nouveaux appuis.
#[derive(Clone, Copy)]
pub struct Entrees {
    pub maintenues: u8,
    pub nouvelles: u8,
}

/// 0x2bfe : entrées du joueur de l'objet `objet` (son numéro en `+2`).
pub fn entrees_du_joueur(b: &mut Cps2Bus, objet: u32) -> Entrees {
    let player = rd8(b, objet + 2) as u32;
    Entrees { maintenues: rd8(b, A5 + player), nouvelles: rd8(b, A5 + ram::NEW + player) }
}

/// 0x2bfe : entrées du joueur de l'objet A0. Sortie : D0 = maintenues, D1 = nouveaux appuis.
fn get_player_input(c: &mut Cpu, b: &mut Cps2Bus) {
    let player = rd8(b, c.a[0] + 2) as u32;
    c.d[2] = player;
    c.a[3] = c.a[5];
    c.a[4] = c.a[5].wrapping_add(ram::NEW);
    let held = rd8(b, c.a[3] + player);
    let new = rd8(b, c.a[4] + player);
    set_low_byte(&mut c.d[0], held);
    set_low_byte(&mut c.d[1], new);
    set_nz_byte(c, new);
    rts(c, b);
}

pub const GET_PLAYER_INPUT: Native = Native {
    entry: 0x2bfe,
    name: "get_player_input",
    run: crate::Corps::Processeur(get_player_input),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

/// 0x2bdc : entrées des quatre joueurs effacées (mot des touches maintenues `+0x6c`, octet `+0x6e`).
fn clear_player_inputs(c: &mut Cpu, b: &mut Cps2Bus) {
    for k in 0..4 {
        let player = 0xff85de + 0x200 * k;
        crate::mem::wr16(b, player + 0x6c, 0);
        crate::mem::wr8(b, player + 0x6e, 0);
    }
    c.sr = c.sr & !0xf | m68k::SR_Z;
    rts(c, b);
}

pub const CLEAR_PLAYER_INPUTS: Native = Native {
    entry: 0x2bdc,
    name: "clear_player_inputs",
    run: crate::Corps::Processeur(clear_player_inputs),
    touches_sound: false,
    outputs: crate::regs::CCR,
};
