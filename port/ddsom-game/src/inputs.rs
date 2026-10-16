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

use crate::mem::{rd16, rd8, rts, set_low_byte, wr16, wr8};
use crate::timing::path_cycles;
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

/// Branchements de read_inputs : (FOUR_PLAYERS != 0, LAYERS_OFF == 0, EEPROM_BUSY != 0).
fn read_inputs_path(c: &Cpu, b: &mut Cps2Bus) -> (bool, bool, bool) {
    let a5 = c.a[5];
    (
        rd8(b, a5 + ram::FOUR_PLAYERS) != 0,
        rd8(b, a5.wrapping_add(ram::LAYERS_OFF)) == 0,
        rd8(b, a5 + ram::EEPROM_BUSY) != 0,
    )
}

/// 0x080c : lecture du panneau (appelée par le VBLANK).
fn read_inputs(c: &mut Cpu, b: &mut Cps2Bus) {
    let a5 = c.a[5];
    let (four_players, layers_on, eeprom_busy) = read_inputs_path(c, b);

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

    // registres laissés par l'original
    set_low_byte(&mut c.d[1], cur[3]);
    set_low_byte(&mut c.d[2], pressed[2]);
    set_low_byte(&mut c.d[3], pressed[3]);
    if four_players {
        set_low_byte(&mut c.d[0], cur[2]);
    } else {
        // cabinet 2 joueurs : le bouton 1 de P3 devient le bouton 4 de P2, la droite de P3 le
        // bouton 4 de P1 (câblage des boutons supplémentaires)
        let p2_b4 = (cur[2] & 0x10) << 3;
        let p1_b4 = (cur[2] & 0x01) << 7;
        wr8(b, a5 + ram::CUR + 1, cur[1] | p2_b4);
        wr8(b, a5 + ram::CUR, cur[0] | p1_b4);
        set_low_byte(&mut c.d[0], p1_b4);
        // lsl.b #3 sur (cur2 & 0x10) : le dernier bit sorti vaut 0, X et C passent à 0
        c.sr &= !SR_X;
    }

    if !layers_on {
        let v = rd16(b, a5 + ram::LAYER_CTRL) & 0xfff1;
        wr16(b, a5 + ram::LAYER_CTRL, v);
    }
    let layer_ctrl = rd16(b, a5 + ram::LAYER_CTRL);
    wr16(b, 0x804166, layer_ctrl);
    if eeprom_busy {
        let v = rd8(b, a5 + ram::EEPROM_BUSY);
        set_nz_byte(c, v);
    } else {
        let v = rd16(b, a5 + ram::EEPROM_PORT);
        wr16(b, 0x804040, v);
        c.flags_logic(v as u32, Size::Word);
    }
    rts(c, b);
}

fn read_inputs_cost(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let (four, on, busy) = read_inputs_path(c, b);
    let path = (four as u64) | (on as u64) << 1 | (busy as u64) << 2;
    path_cycles(b, 0x080c, path, |pc| match pc {
        0x08bc => four,
        0x08de => on,
        0x08f4 => busy,
        _ => panic!("read_inputs : branchement inattendu en {pc:06x}"),
    })
}

pub const READ_INPUTS: Native = Native {
    entry: 0x080c,
    name: "read_inputs",
    cost: read_inputs_cost,
    run: read_inputs,
    touches_sound: false,
};

/// 0x2aec : recopie les entrées dans les enregistrements des joueurs.
fn copy_inputs_to_players(c: &mut Cpu, b: &mut Cps2Bus) {
    let a5 = c.a[5];
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
        // clr.b final
        c.sr = (c.sr & !(SR_N | SR_V | SR_C)) | SR_Z;
    } else {
        let mask = rd8(b, a5 + ram::INPUT_MASK);
        set_low_byte(&mut c.d[4], mask);
        for pair in [0u32, 2] {
            let mut regs = [0u8; 4];
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
            set_low_byte(&mut c.d[0], regs[0]);
            set_low_byte(&mut c.d[1], regs[1]);
            set_low_byte(&mut c.d[2], regs[2]);
            set_low_byte(&mut c.d[3], regs[3]);
        }
        // move.b d3,(...) final
        let d3 = c.d[3] as u8;
        set_nz_byte(c, d3);
    }
    rts(c, b);
}

fn copy_inputs_cost(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let raw = rd8(b, c.a[5] + ram::INPUT_RAW) != 0;
    path_cycles(b, 0x2aec, raw as u64, |_| raw)
}

pub const COPY_INPUTS_TO_PLAYERS: Native = Native {
    entry: 0x2aec,
    name: "copy_inputs_to_players",
    cost: copy_inputs_cost,
    run: copy_inputs_to_players,
    touches_sound: false,
};

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
    cost: |_, b| path_cycles(b, 0x2bfe, 0, |_| false),
    run: get_player_input,
    touches_sound: false,
};
