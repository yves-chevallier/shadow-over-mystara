//! Variants 8 to 24 of SET type 15 (see `set_c_finale.rs`). The role names are guesses.

use crate::families::cx::Bench;
use super::*;
use crate::mem::{rd16, rd32, rd8, wr32, A5};
use crate::objects::pools;

/// Players: four records of 0x200 bytes.
const PLAYERS: u32 = A5 + 0x5de;
const PLAYER_STRIDE: u32 = 0x200;
/// Pause handler of the linked objects (word A5+0x14: activity bits of the background tasks).
const TASK_FLAGS: u32 = 0x14;

fn players() -> impl Iterator<Item = u32> {
    (0..4).map(|i| PLAYERS + i * PLAYER_STRIDE)
}

/// Player in game: active and main state (byte `+4`) below 4.
fn present<R: Bench>(x: &mut Cx<'_, R>, p: u32) -> bool {
    rd8(x.b, p) != 0 && (rd8(x.b, p + 4) as i8) < 4
}

/// Releases the object on the next pass: `+4 = 6` and the next three bytes at zero.
fn retire<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(0x4, 0x0600_0000);
}

/// RAM address stored on 16 bits (`movea.w`: sign extension).
fn adr16(v: u16) -> u32 {
    v as i16 as i32 as u32
}

/// Creates a SET of type 15 (variant `variant`): routine 0xce00a. Returns the object or `None`.
fn spawn_finale<R: Bench>(x: &mut Cx<'_, R>, variant: u32) -> Option<Obj> {
    crate::families::creators::set_15_pure(x.b, variant as u8).ok()
}

/// Creates a SOBJ of type 6 (routine 0xddeaa), variant `variant`.
fn spawn_sobj<R: Bench>(x: &mut Cx<'_, R>, variant: u32) -> Option<Obj> {
    crate::effects_aids::scenery_6(x.b, variant as u8).ok()
}

/// Creates an effect (type 0x1f of 0xb1a7a): variant, value of `+0x2f`.
fn spawn_effct<R: Bench>(x: &mut Cx<'_, R>, variant: u32, extra: u32) -> Option<Obj> {
    crate::families::effects_creators::countdown(x.b, variant as u8, extra as u8)
}

/// Four-component shake (0xce24c): background and second plane camera, with copy of the
/// "previous" positions. `table`: 8-byte records, one per step of `+0x18` & 3.
fn shake4<R: Bench>(x: &mut Cx<'_, R>, table: u32) {
    let at = table + 8 * (x.u8(0x18) & 3) as u32;
    x.add8(0x18, 1);
    for (i, var) in [a5::CAM_X, a5::CAM_Y, a5::BG_X, a5::BG_Y].into_iter().enumerate() {
        let d = x.data16(at + 2 * i as u32);
        let v = x.a5_u16(var).wrapping_add(d);
        x.set_a5_u16(var, v);
    }
    copy_cameras(x);
}

fn copy_cameras<R: Bench>(x: &mut Cx<'_, R>) {
    for (from, to) in [(a5::CAM_X, a5::CAM_X_PREV), (a5::CAM_Y, a5::CAM_Y_PREV), (a5::BG_X, a5::BG_X_PREV), (a5::BG_Y, a5::BG_Y_PREV)] {
        let v = x.a5_u16(from);
        x.set_a5_u16(to, v);
    }
}

fn mask_tasks<R: Bench>(x: &mut Cx<'_, R>, keep: u16) {
    let v = x.a5_u16(TASK_FLAGS) & keep;
    x.set_a5_u16(TASK_FLAGS, v);
}

// --- variants 8 and 23: HP refill ----------------------------------------------------------

/// Prepares the list (player, reserve): `+0x30 + 4 i` = player's address, `+0x32 + 4 i` = HP given back
/// automatically; the base step depends on the score `A5+0x38`, increased (variant 23) by the player's `+0x64`.
fn refill_setup<R: Bench>(x: &mut Cx<'_, R>, with_player_bonus: bool) {
    let score = x.a5_u16(0x38) as i16;
    let mut step: u16 = 0x14;
    for (limit, add) in [(0x100, 8), (0x130, 4), (0x200, 8), (0x230, 8)] {
        if score <= limit {
            break;
        }
        step = step.wrapping_add(add);
    }
    let mut slot = 0x30;
    for p in players() {
        if present(x, p) {
            x.set16(slot, p as u16);
            if with_player_bonus {
                step = step.wrapping_add(rd16(x.b, p + 0x64));
            }
            x.set16(slot + 2, step);
            slot += 4;
        }
    }
}

/// One refill round: each player whose HP (`+0x62`) are below the maximum (`+0x60`)
/// gains one as long as their reserve is non-empty, then up to 1.25 times half the maximum. True
/// if a player received an HP.
fn refill_step<R: Bench>(x: &mut Cx<'_, R>, variant_b: bool) -> bool {
    let mut slot = 0x30;
    let mut gave = 0u8;
    let count = (x.a5_u8(0x35) as u16).wrapping_sub(1) as u32 + 1;
    for _ in 0..count {
        let p = adr16(x.u16(slot));
        let max = rd16(x.b, p + 0x60) as i16;
        let cur = rd16(x.b, p + 0x62) as i16;
        if max > cur {
            if x.u16(slot + 2) != 0 {
                gave = gave.wrapping_add(1);
                wr16(x.b, p + 0x62, cur.wrapping_add(1) as u16);
                let left = x.u16(slot + 2).wrapping_sub(1);
                x.set16(slot + 2, left);
            } else {
                let half = (max as u16) >> 1;
                let bonus = if variant_b && x.u8(0x42) == 0 { 0 } else { half >> 2 };
                if (half.wrapping_add(bonus) as i16) > cur {
                    gave = gave.wrapping_add(1);
                    wr16(x.b, p + 0x62, cur.wrapping_add(1) as u16);
                }
            }
        }
        slot += 4;
    }
    gave != 0
}

/// Variant 8 (0xcf3a8): end of level, the HP climb back up.
pub fn hp_refill_a<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            refill_setup(x, false);
        }
        2 => {
            if !refill_step(x, false) {
                next_state(x);
            }
        }
        _ => {}
    }
}

/// Variant 23 (0xd0932): same with the bonus specific to each player.
pub fn hp_refill_b<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            refill_setup(x, true);
        }
        2 => {
            if !refill_step(x, true) {
                next_state(x);
            }
        }
        _ => {}
    }
}

// --- variants 9 and 11: effects attached to the players ------------------------------------------------------

/// Picks, for an effect with no player, a free character number in the `taken` mask:
/// examines the pairs (0,1), (2,3)...; the first entirely free pair gives its first, otherwise
/// the first free number. The result is marked taken.
fn free_character(taken: &mut u32) -> Option<u32> {
    let bit = |t: u32, n: u32| t >> (n & 31) & 1 != 0;
    let mut n = 0;
    for _ in 0..6 {
        if bit(*taken, n) || bit(*taken, n + 1) {
            n += 2;
        } else {
            *taken |= 1 << n;
            return Some(n);
        }
    }
    let mut n = 0;
    for _ in 0..12 {
        for _ in 0..2 {
            if !bit(*taken, n) {
                *taken |= 1 << n;
                return Some(n);
            }
            n += 1;
        }
    }
    None
}

/// Mask of the characters present (variant `+3` of each player in game).
fn characters_present<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    let mut mask = 0;
    for p in players() {
        if present(x, p) {
            mask |= 1 << (rd8(x.b, p + 3) & 31);
        }
    }
    mask
}

/// Variant 9 (0xcf476): launches six effects (one per player, then per absent character), notifies the
/// conductor (`+8 = 1`), waits for the end, then hands control back to it (`+8 = 0xff`).
pub fn spawn_player_effects<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            x.set8(0x30, 0);
            player_effects(x);
        }
        2 => match x.u8(0x5) {
            0 => {
                if x.a5_u8(0x48f) != 0 {
                    return;
                }
                handover_step(x, 0)
            }
            2 => handover_step2(x),
            4 => handover_step3(x),
            _ => {}
        },
        _ => {}
    }
}

fn player_effects<R: Bench>(x: &mut Cx<'_, R>) {
    let mut taken = characters_present(x);
    let mut a6 = PLAYERS;
    for d1 in (0..=5u32).rev() {
        let here = d1 as i8 > 1 && present(x, a6);
        let Some(fx) = spawn_effct(x, 1, d1) else { break };
        if here {
            fx.set_u16(x.b, 0x50, a6 as u16);
            fx.set_u16(x.b, 0x5c, x.o.0 as u16);
        } else if let Some(n) = free_character(&mut taken) {
            fx.set_u16(x.b, 0x50, n as u16);
            fx.set_u16(x.b, 0x5c, x.o.0 as u16);
        }
        a6 += PLAYER_STRIDE;
    }
    x.set16(0x42, 0x3c);
    let boss = conductor(x);
    boss.set_u8(x.b, 8, 1);
}

/// First phase: after the delay `+0x42`, creates the bonus screen (type 15 variant 0x17).
fn handover_step<R: Bench>(x: &mut Cx<'_, R>, mark: u8) {
    x.add16(0x42, 0xffff);
    if x.u16(0x42) != 0 {
        return;
    }
    if let Some(o) = spawn_finale(x, 0x17) {
        o.set_u8(x.b, 0x42, mark);
    }
    next_substate(x);
    x.set8(0x30, 1);
    x.set8(0x31, 6);
    x.set_a5_u8(0x33, 1);
}

fn handover_step2<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(0x31) as i8) >= 0 {
        return;
    }
    x.set_a5_u8(0x33, 0);
    next_substate(x);
    x.set16(0x42, 1);
}

fn handover_step3<R: Bench>(x: &mut Cx<'_, R>) {
    x.add16(0x42, 0xffff);
    if x.u16(0x42) != 0 {
        return;
    }
    let boss = conductor(x);
    boss.set_u8(x.b, 8, 0xff);
    x.set8(0x30, 2);
    next_state(x);
}

/// Variant 11 (0xcf6e8): like 9 for state 0; then it waits for the conductor
/// to go to 2 before launching the bonus screen (`+0x42 = 1`).
pub fn handover_effects<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => spawn_player_effects(x),
        2 => match x.u8(0x5) {
            0 => {
                if conductor(x).u8(x.b, 8) != 2 {
                    return;
                }
                handover_step(x, 1)
            }
            2 => handover_step2(x),
            4 => handover_step3(x),
            _ => {}
        },
        _ => {}
    }
}

// --- variant 10: opening shake ------------------------------------------------------------------------

/// Variant 10 (0xcf5e6): shake (table 0xd0b30) until the end of the fade (`A5+0x48f`), then the background
/// accelerates in horizontal scrolling (`+0x14`, `+0x16`) until the speed falls below 0x100,
/// and finally sets the conductor to 2. `+5`: 0 shake, 2 wait for the conductor, 4 scrolling.
pub fn shaking_intro<R: Bench>(x: &mut Cx<'_, R>) {
    const SHAKE: u32 = 0xd0b30;
    const SCROLL_SHAKE: u32 = 0xd0b50;
    if x.u8(0x4) == 0 {
        next_state(x);
        x.set8(0x18, 0);
    } else if x.u8(0x4) != 2 {
        return;
    }
    match x.u8(0x5) {
        0 => {
            shake4(x, SHAKE);
            if x.a5_u8(0x48f) != 0 {
                return;
            }
            next_substate(x);
            x.set16(0x14, 0x800);
            x.set16(0x16, 0xfff8);
            x.set_a5_u16(a5::CAM_X, 0);
            x.set_a5_u16(a5::CAM_Y, 0);
            x.set_a5_u16(a5::BG_X, 0x80);
            x.set_a5_u16(a5::BG_Y, 0);
            copy_cameras(x);
        }
        2 => {
            if conductor(x).u8(x.b, 8) == 1 {
                next_substate(x);
            }
            let v = x.a5_u16(a5::CAM_X).wrapping_add(8);
            x.set_a5_u16(a5::CAM_X, v);
            shake4(x, SCROLL_SHAKE);
        }
        4 => {
            if x.u8(0x14) != 0 {
                let speed = x.u16(0x14).wrapping_add(x.u16(0x16));
                x.set16(0x14, speed);
                let v = x.a5_u16(a5::CAM_X).wrapping_add(((speed as i16) >> 8) as u16);
                x.set_a5_u16(a5::CAM_X, v);
                if (v as i16) >= 0x300 {
                    x.set_a5_u16(a5::CAM_X, v & 0xff);
                }
                shake4(x, SCROLL_SHAKE);
            } else {
                next_substate(x);
                x.set_a5_u16(a5::BG_X, 0x80);
                x.set_a5_u16(a5::BG_Y, 0);
                copy_cameras(x);
                conductor(x).set_u8(x.b, 8, 2);
                next_state(x);
            }
        }
        _ => {}
    }
}

// --- variant 12: player markers ---------------------------------------------------------------------------

/// Variant 12 (0xcf782): creates six SOBJ objects (type 6, variant 0xe), one per player then per
/// absent character (like variant 9), then a last one (variant 0, `+0x2f = 0xb`) at (0x108, 0x100,
/// -0x80). The object then goes to state 6.
pub fn spawn_player_markers<R: Bench>(x: &mut Cx<'_, R>) {
    if !matches!(x.u8(0x4), 0 | 2) {
        return;
    }
    let boss = x.u32(0x4c);
    let mut taken = characters_present(x);
    let mut a6 = PLAYERS;
    for d1 in (0..=5u32).rev() {
        let here = d1 as i8 > 1 && present(x, a6);
        let Some(m) = spawn_sobj(x, 0xe) else {
            break;
        };
        if here {
            m.set_u8(x.b, 0x2f, d1 as u8);
            m.set_u16(x.b, 0x46, a6 as u16);
            m.set_u32(x.b, 0x4c, boss);
        } else if let Some(n) = free_character(&mut taken) {
            m.set_u8(x.b, 0x2f, d1 as u8);
            m.set_u16(x.b, 0x46, n as u16);
            m.set_u32(x.b, 0x4c, boss);
        }
        a6 += PLAYER_STRIDE;
    }
    retire(x);
    let Some(m) = spawn_sobj(x, 0) else { return };
    m.set_u8(x.b, 0x2f, 0xb);
    m.set_u16(x.b, 8, 0x108);
    m.set_u16(x.b, 0x10, 0x100);
    m.set_u16(x.b, 0xc, 0xff80);
    m.set_u32(x.b, 0x4c, boss);
}

// --- variant 13: fireworks ----------------------------------------------------------------------------------

/// Variant 13 (0xcf8a0): prepares the palette (0x91c280, with 0xf000f000), then, as long as the
/// conductor does not switch to a negative `+8`, launches from time to time a SOBJ object of variant 0x10
/// (position drawn from table 0xcf974) and, once, a bonus screen (type 15 variant 0xf).
pub fn fireworks<R: Bench>(x: &mut Cx<'_, R>) {
    const PALETTE_ROM: u32 = 0x253280;
    const PALETTE: u32 = 0x91c280;
    const POSITIONS: u32 = 0xcf974;
    match x.u8(0x4) {
        0 => {
            next_state(x);
            for i in 0..8 {
                let v = x.data32(PALETTE_ROM + 4 * i);
                wr32(x.b, PALETTE + 4 * i, v);
            }
            for i in 0..8 {
                let v = rd32(x.b, PALETTE + 4 * i) | 0xf000_f000;
                wr32(x.b, PALETTE + 4 * i, v);
            }
            x.set8(0x42, 0);
        }
        2 => {}
        _ => return,
    }
    if (conductor(x).u8(x.b, 8) as i8) < 0 {
        return retire(x);
    }
    if rng(x) & 7 != 0 {
        return;
    }
    let Some(o) = spawn_sobj(x, 0x10) else { return };
    let owner = x.u32(0x4c);
    o.set_u32(x.b, 0x4c, owner);
    let pick = ((rng(x) & 7) as u32) << 2;
    let (px, pz) = (x.rom16(POSITIONS + pick), x.rom16(POSITIONS + pick + 2));
    o.set_u16(x.b, 8, px);
    o.set_u16(x.b, 0xc, pz);
    o.set_u32(x.b, 0x10, 0);
    if x.u8(0x42) != 0 {
        return;
    }
    let Some(f) = spawn_finale(x, 0xf) else { return };
    x.set8(0x42, 1);
    let owner = x.u32(0x4c);
    f.set_u32(x.b, 0x4c, owner);
}

// --- variant 14: lightning --------------------------------------------------------------------------------

/// Variant 14 (0xcf994): creates lightning bolts (SOBJ variant 0x11, number 0 to 4 drawn without repetition
/// among the free bits of `+0x30`) as long as the second plane (`A5+0x55a`) has not returned to zero;
/// the sound call 0xea83e rings out from time to time.
pub fn lightning<R: Bench>(x: &mut Cx<'_, R>) {
    const ORDER: u32 = 0xcfa3c;
    const SOUND: crate::sound::Sound = crate::sound::CMD_3F;
    match x.u8(0x4) {
        0 => {
            next_state(x);
            x.set8(0x30, 1);
        }
        2 => {}
        _ => return,
    }
    if rng(x) & 7 == 0 {
        crate::sound::play(x.b, SOUND, x.o);
    }
    if x.a5_u16(a5::BG_Y) == 0 {
        return next_state(x);
    }
    if x.u8(0x5) == 0 && x.a5_u16(a5::CAM_X) == 0 {
        next_substate(x);
        x.set_bit(0x30, 0, false);
    }
    if x.u8(0x30) == 0x1f {
        return;
    }
    if x.a5_u16(0x6e) & 0xf != 0 {
        return;
    }
    let Some(o) = spawn_sobj(x, 0x11) else { return };
    let pick = (rng(x) & 0xf) as u32;
    let mut bit = x.rom8(ORDER + pick);
    while x.u8(0x30) >> (bit & 7) & 1 != 0 {
        bit = bit.wrapping_sub(1);
        if bit & 0x80 != 0 {
            bit = 4;
        }
    }
    x.set_bit(0x30, bit & 7, true);
    o.set_u8(x.b, 0x2f, bit);
    o.set_u16(x.b, 0x46, x.o.0 as u16);
}

// --- variant 15: light shake -----------------------------------------------------------------------------------

/// Variant 15 (0xcfa4c): shakes the background (table 0xd0b70) as long as the conductor has `+8 >= 0`.
pub fn quake_light<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            x.set8(0x18, 0);
        }
        2 => {}
        _ => return,
    }
    if (conductor(x).u8(x.b, 8) as i8) < 0 {
        return retire(x);
    }
    shake4(x, 0xd0b70);
}

// --- variant 16: arrival list (h) ------------------------------------------------------------------------------------

/// Variant 16 (0xcfa86): one effect (type 0x1f variant 7) per player present, spaced 0xb4
/// frames apart and taken in random order among 16 numbers (mask `+0x20`); `+0x22` counts the
/// effects launched, `+0x23` those still alive. When everything has left, goes to state 4 and advances
/// `A5+0x2d`.
pub fn ranking_start<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            let mut taken: u32 = 0xf000;
            x.set8(0x22, 0);
            x.set8(0x23, 0);
            for p in players() {
                if present(x, p) {
                    taken |= 1 << (rd8(x.b, p + 3) & 31);
                    x.add8(0x22, 1);
                }
            }
            x.set16(0x20, taken as u16);
            x.set16(0x24, 0xb4);
            let mut left = x.u8(0x22);
            x.add8(0x22, 0xff);
            loop {
                left = left.wrapping_sub(1);
                if left == 0 {
                    break;
                }
                x.add16(0x24, 0xb4);
            }
        }
        2 => {
            if x.u8(0x5) != 0 {
                if x.u8(0x23) != 0 {
                    return;
                }
                next_state(x);
                let v = x.a5_u8(0x2d).wrapping_add(2);
                x.set_a5_u8(0x2d, v);
                return;
            }
            x.add16(0x24, 0xffff);
            if x.u16(0x24) != 0 {
                return;
            }
            x.set16(0x24, 0xb4);
            let mut taken = x.u16(0x20) as u32;
            let mut n = (rng(x) & 0xf) as u32;
            while taken >> n & 1 != 0 {
                n = (n + 1) & 0xf;
            }
            taken |= 1 << n;
            x.set16(0x20, taken as u16);
            let Some(fx) = spawn_effct(x, 7, n) else { return };
            fx.set_u8(x.b, 0x2f, n as u8);
            let order = x.u8(0x22);
            fx.set_u8(x.b, 0x42, order);
            fx.set_u16(x.b, 0x46, x.o.0 as u16);
            x.add8(0x22, 1);
            x.add8(0x23, 1);
            if x.u16(0x20) == 0xffff {
                next_substate(x);
            }
        }
        _ => {}
    }
}

// --- variant 17: palette fade
// ---------------------------------------------------------------------------------------

/// Variant 17 (0xcfb76): fade of the three palette lines 0x91c1e0, 0x91c300, 0x91c400: `+5` = 0
/// brightens 15 times (+0x1000 on two words of each line every 4 frames), 2 waits for `+0x42`,
/// 4 darkens the same way then sets `+0x32 = 1` on object `+0x46`.
pub fn palette_fade<R: Bench>(x: &mut Cx<'_, R>) {
    const LINES: [u32; 3] = [0x91c1e0, 0x91c300, 0x91c400];
    if x.u8(0x4) == 0 {
        next_state(x);
        x.set8(0x20, 4);
        x.set8(0x21, 0xf);
    } else if x.u8(0x4) != 2 {
        return;
    }
    let fade = |x: &mut Cx<'_, R>, delta: u32| {
        for line in LINES {
            for i in 0..8 {
                let v = rd32(x.b, line + 4 * i).wrapping_add(delta);
                wr32(x.b, line + 4 * i, v);
            }
        }
    };
    match x.u8(0x5) {
        0 => {
            x.add8(0x20, 0xff);
            if x.u8(0x20) != 0 {
                return;
            }
            x.set8(0x20, 4);
            fade(x, 0x1000_1000);
            x.add8(0x21, 0xff);
            if x.u8(0x21) != 0 {
                return;
            }
            next_substate(x);
            x.set8(0x21, 0xf);
        }
        2 => {
            x.add16(0x42, 0xffff);
            if x.u16(0x42) == 0 {
                next_substate(x);
            }
        }
        4 => {
            x.add8(0x20, 0xff);
            if x.u8(0x20) != 0 {
                return;
            }
            x.set8(0x20, 4);
            fade(x, 0xefff_f000);
            x.add8(0x21, 0xff);
            if x.u8(0x21) != 0 {
                return;
            }
            next_state(x);
            let target = adr16(x.u16(0x46));
            wr16(x.b, target + 0x32, 1);
        }
        _ => {}
    }
}

// --- variant 18: choice of end-of-level bonuses ----------------------------------------------------------------

/// Variant 18 (0xcfc56): chooses, player after player, a bonus among candidates (tests of
/// table 0xcff10, depending on the character and the turn), shows the reward (creation of a SET by
/// 0xc8312), animates the player's palette and starts over until all players are served (`+0x25`).
pub fn bonus_selection<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            x.set32(0x20, 0xff00_0000);
            x.set8(0x24, 0);
            let mut n = 0;
            for p in players() {
                if present(x, p) {
                    n += 1;
                }
            }
            x.set8(0x25, n);
        }
        2 => bonus_step(x),
        _ => {}
    }
}

const REWARD_PALETTE_ROM: u32 = 0xcfdd2;

fn bonus_step<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x5) {
        0 => {
            let Some(o) = crate::families::creators::scene_set(x.b, 0x94) else { return };
            next_substate(x);
            x.set16(0x46, o.0 as u16);
            o.set_u16(x.b, 0x46, 0);
            for p in players() {
                if present(x, p) && rd8(x.b, p + 0x18a) != 0 {
                    o.set_u16(x.b, 0x46, p as u16);
                    return;
                }
            }
        }
        2 | 0xa => {
            let reward = adr16(x.u16(0x46));
            if rd8(x.b, reward + 0x22) == 0 {
                return;
            }
            next_substate(x);
            for i in 0..8 {
                wr32(x.b, 0x912224 + 0x80 * i, 0x20_0000);
            }
        }
        4 => {
            let index = x.u8(0x24) as u16;
            x.add8(0x24, 1);
            let mut a1 = PLAYERS.wrapping_add(adr16(index.rotate_right(7)));
            while !present(x, a1) {
                x.add8(0x24, 1);
                a1 = a1.wrapping_add(PLAYER_STRIDE);
            }
            next_substate(x);
            x.set16(0x26, a1 as u16);
            x.set8(0x2b, 8);
            x.set8(0x2a, 4);
        }
        6 => palette_pulse(x, 0x1000_1000),
        0xc => palette_pulse(x, 0xefff_f000),
        8 => choose_reward(x),
        0xe => {
            x.add8(0x25, 0xff);
            if x.u8(0x25) == 0 {
                next_substate(x);
            } else {
                x.set8(0x5, 4);
            }
        }
        0x10 => {
            next_state(x);
            let v = x.a5_u8(0x2d).wrapping_add(2);
            x.set_a5_u8(0x2d, v);
            x.set_a5_u16(0x2e, 0);
        }
        _ => {}
    }
}

/// Varies eight longs of the current player's palette (`+0x26`) every 4 frames, 8 times.
fn palette_pulse<R: Bench>(x: &mut Cx<'_, R>, delta: u32) {
    x.add8(0x2a, 0xff);
    if x.u8(0x2a) != 0 {
        return;
    }
    x.set8(0x2a, 4);
    let player = adr16(x.u16(0x26));
    let character = rd8(x.b, player + 3) as u32;
    let offset = adr16(x.rom16(REWARD_PALETTE_ROM + 2 * character));
    let base = 0x91c000u32.wrapping_add(offset);
    for i in 0..8 {
        let v = rd32(x.b, base + 4 * i).wrapping_add(delta);
        wr32(x.b, base + 4 * i, v);
    }
    x.add8(0x2b, 0xff);
    if x.u8(0x2b) == 0 {
        x.set8(0x2b, 8);
        next_substate(x);
    }
}

/// A candidate is discarded (true) according to the test that table 0xcff10 associates with the pair (character,
/// rank): the lists of objects already owned, the room in the inventory (+0x18a, +0x114...).
fn reward_blocked<R: Bench>(x: &mut Cx<'_, R>, player: u32, rank: u32) -> bool {
    const TESTS: u32 = 0xcff10;
    let character = (rd8(x.b, player + 3) & !1) as u32;
    let index = (character << 2) + 2 * rank;
    let target = TESTS + x.rom16(TESTS + index) as u32;
    // lists of objects (bytes, end = negative) compared with a byte of the player
    let listed = |x: &mut Cx<'_, R>, list: u32, value: u8| {
        let mut p = list;
        loop {
            let b = x.data8(p);
            p += 1;
            if b as i8 <= -1 && b & 0x80 != 0 {
                return false;
            }
            if b == value {
                return true;
            }
        }
    };
    match target {
        0xcff40 | 0xcff7c => {
            let (a, b) = if target == 0xcff40 { (0xcffb8, 0xcffbe) } else { (0xcffc4, 0xcffca) };
            let (v1, v2) = (rd8(x.b, player + 0x10d), rd8(x.b, player + 0x10e));
            if listed(x, a, v1) || listed(x, a, v2) {
                return false;
            }
            for i in 0..6 {
                let v = rd8(x.b, player + 0x140 + i);
                if listed(x, b, v) {
                    return false;
                }
            }
            true
        }
        0xcfff6 => rd8(x.b, player + 0x18a) == 0,
        0xd0006 => (rd32(x.b, player + 0x114) as i32) < 0x3000,
        0xd0016 => x.a5_u8(0x5d) == 0,
        0xcffdc => !(0..0x66).any(|i| rd8(x.b, 0xff2478 + i) == 0),
        _ => false,
    }
}

fn choose_reward<R: Bench>(x: &mut Cx<'_, R>) {
    const REWARD_IDS: u32 = 0xcfe98;
    const REWARD_ICONS: u32 = 0xcfec8;
    let player = adr16(x.u16(0x26));
    let character = rd8(x.b, player + 3) as u32;
    let mut taken = x.u32(0x20);
    let mut candidate = character & !1;
    candidate = (candidate << 1) & 0xffff;
    let mut rank = 0u32;
    loop {
        let blocked = taken >> (candidate & 31) & 1 != 0 || reward_blocked(x, player, rank);
        if !blocked {
            break;
        }
        candidate += 1;
        rank += 1;
    }
    taken |= 1 << (candidate & 31);
    taken &= 0xff77_7777;
    x.set32(0x20, taken);
    let id = x.rom16(REWARD_IDS + 2 * candidate);
    let Some(o) = crate::families::creators::scene_set(x.b, id as u8) else { return };
    next_substate(x);
    x.set16(0x46, o.0 as u16);
    let chosen = x.u16(0x26);
    o.set_u16(x.b, 0x46, chosen);
    let low = candidate & 3;
    let icons = REWARD_ICONS.wrapping_add((candidate & 0x1c) << 1);
    let mut at = 0x912224;
    for i in 0..7 {
        let tile = x.data8(icons + i) as u16;
        wr16(x.b, at, tile);
        wr16(x.b, at + 2, 0);
        at += 0x80;
    }
    wr16(x.b, at, (low + 1) as u16);
    wr16(x.b, at + 2, 0);
    for i in 0..8 {
        let v = rd32(x.b, 0x91c400 + 4 * i) & 0x0fff_0fff;
        wr32(x.b, 0x91c400 + 4 * i, v);
    }
}

// --- variant 19: countdown in digits ---------------------------------------------------------------------------------

/// Variant 19 (0xd004a): the object carries a BCD number in `+0x4c` (4 digits of 4 bits in
/// bytes 0x100, 0x1000, 0x10000, 0x100000); each frame removes one unit from the lowest
/// non-zero digit and launches event 0x19ae6 (sound/score) for that digit; at zero, it releases itself.
pub fn countdown_digits<R: Bench>(x: &mut Cx<'_, R>) {
    let mut value = x.u32(0x4c);
    if value == 0 {
        return x.free(&pools::SET);
    }
    let (sub, event) = if value & 0xf00 != 0 {
        (0x100, 4)
    } else if value & 0xf000 != 0 {
        (0x1000, 0xe)
    } else if value & 0xf_0000 != 0 {
        (0x1_0000, 0x21)
    } else {
        (0x10_0000, 0x28)
    };
    value = value.wrapping_sub(sub);
    x.set32(0x4c, value);
    let event_word = event | (x.u8(0x2f) as u32) << 8;
    crate::events::enqueue_points(x.b, event_word as u16);
}

// --- variant 20: credits scroll ---------------------------------------------------------------------------------

/// Variant 20 (0xd00c2): scrolling text (h: credits) written line by line into the tilemap
/// 0x910000 from the ROM text 0xd01de: one byte per cell (0xfe: end, 0xff n: attribute n,
/// 0x2f: end of page). The background (`A5+0x4ba`) scrolls by 0x4000/65536 pixel per frame.
pub fn credits_scroll<R: Bench>(x: &mut Cx<'_, R>) {
    const TEXT: u32 = 0xd01de;
    if x.u8(0x4) == 6 {
        return x.free(&pools::SET);
    }
    if x.u8(0x4) == 0 {
        next_state(x);
        crate::video::clear_layer(x.b, crate::video::Layer::One);
        x.set_a5_u8(0x48e, 2);
        crate::palette::full_brightness(x.b);
        x.set32(0x20, 0);
        x.set32(0x24, TEXT);
        x.set8(0x29, 0);
        x.set8(0x2a, 0);
        x.set16(0x2c, 0xf00);
        x.set8(0x2e, 0);
    }
    let v = x.u32(0x20).wrapping_sub(0x4000);
    x.set32(0x20, v);
    let scroll = x.u16(0x20) & 0x1ff;
    x.set16(0x20, scroll);
    x.set_a5_u16(0x4ba, scroll);
    if x.u8(0x5) != 0 {
        return;
    }
    let phase = (x.a5_u16(0x4ba) & 8) as u8;
    if x.u8(0x2a) == phase {
        return;
    }
    let toggled = x.u8(0x2a) ^ 8;
    x.set8(0x2a, toggled);
    let mut text = x.u32(0x24);
    let mut dest = 0x910000u32.wrapping_add(adr16(x.u16(0x2c)));
    x.add16(0x2c, 4);
    x.add8(0x29, 1);
    if x.u8(0x29) == 0x20 {
        let v = x.u16(0x2c).wrapping_sub(0x80);
        x.set16(0x2c, v);
        x.set8(0x29, 0);
        let v = x.u16(0x2c).wrapping_add(0x2000) & 0x3fff;
        x.set16(0x2c, v);
    }
    let mut attr = x.u8(0x2e) as u16;
    // cell cleared at the same time: the one on the opposite page (0x2000 further), set once
    let erase = 0x910000u32 + ((dest as u16).wrapping_add(0x2000) & 0x3fff) as u32;
    loop {
        let c = x.data8(text);
        text += 1;
        match c {
            0xfe => return next_substate(x),
            0xff => {
                let a = x.data8(text);
                text += 1;
                x.set8(0x2e, a);
                attr = a as u16;
            }
            0x2f => {
                x.set32(0x24, text);
                for i in 0..17 {
                    wr32(x.b, erase + 0x80 * i, 0x20_0000);
                }
                return;
            }
            _ => {
                wr16(x.b, dest, c as u16);
                wr16(x.b, dest + 2, attr);
                dest += 0x80;
            }
        }
    }
}

// --- variant 21: shimmer
// ---------------------------------------------------------------------------------------------

/// Variant 21 (0xd06e6): makes a palette color twinkle (0x91cfb2 takes 0xf00 from time
/// to time) and launches a SOBJ object of variant 0x14; withdraws when the conductor has `+8 < 0`.
pub fn flicker<R: Bench>(x: &mut Cx<'_, R>) {
    const COLOR: u32 = 0x91cfb2;
    const REFERENCE: u32 = 0x91cc00;
    match x.u8(0x4) {
        0 => {
            next_state(x);
            x.set8(0x20, 0);
            let v = rd16(x.b, COLOR);
            x.set16(0x22, v);
        }
        2 => {}
        _ => return,
    }
    if (conductor(x).u8(x.b, 8) as i8) < 0 {
        return x.set8(0x4, 6);
    }
    if x.u8(0x20) != 0 {
        x.set8(0x20, 0);
        let v = x.u16(0x22) & 0xfff | rd16(x.b, REFERENCE) & 0xf000;
        return wr16(x.b, COLOR, v);
    }
    if rng(x) & 7 != 0 {
        return;
    }
    x.set8(0x20, 1);
    let v = 0xf00 | rd16(x.b, REFERENCE) & 0xf000;
    wr16(x.b, COLOR, v);
    let Some(o) = spawn_sobj(x, 0x14) else { return };
    let owner = x.u32(0x4c);
    o.set_u32(x.b, 0x4c, owner);
}

// --- variant 22: entering initials (h) -----------------------------------------------------------

/// Variant 22 (0xd077c): goes through the players present, one after the other (`+0x20` = player,
/// `+0x22` = remaining players); for each one creates a SOBJ of variant 0x15 that displays their name
/// (list of tiles at `+0x118` of the player, written by 0xb230e on a background of 0x912000), waits 0x78
/// frames, removes the object and moves on to the next. Ends with 0xca8 and 0xcc8.
pub fn name_entry<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x4) == 0 {
        next_state(x);
        x.set16(0x20, PLAYERS as u16);
        x.set8(0x22, 4);
    } else if x.u8(0x4) != 2 {
        return;
    }
    match x.u8(0x5) {
        0 => {
            let mut a1 = adr16(x.u16(0x20));
            loop {
                if x.u8(0x22) == 0 {
                    next_state(x);
                    let v = x.a5_u8(0x2d).wrapping_add(2);
                    x.set_a5_u8(0x2d, v);
                    crate::video::clear_layer(x.b, crate::video::Layer::One);
                    return crate::video::clear_layer(x.b, crate::video::Layer::Two);
                }
                x.add8(0x22, 0xff);
                if rd8(x.b, a1) != 0 && (rd8(x.b, a1 + 4) as i8) < 4 {
                    next_substate(x);
                    x.set16(0x20, a1 as u16);
                    mask_tasks(x, 0xfff1);
                    return;
                }
                a1 += PLAYER_STRIDE;
            }
        }
        2 => {
            let player = adr16(x.u16(0x20));
            let Some(o) = spawn_sobj(x, 0x15) else { return };
            crate::video::clear_layer(x.b, crate::video::Layer::One);
            next_substate(x);
            x.set16(0x24, o.0 as u16);
            let character = rd8(x.b, player + 3);
            o.set_u8(x.b, 0x2f, character);
            let dest = 0x912000u32.wrapping_add(x.rom16(0xd0858 + 2 * character as u32) as i16 as i32 as u32);
            let kind = rd8(x.b, player + 2) as u32;
            let attr = x.rom8(0xd0854 + kind) as u16;
            write_name(x, player + 0x118, dest, attr);
            block_loader_for(x, character as u32);
        }
        4 => {
            next_substate(x);
            let v = x.a5_u16(TASK_FLAGS) | 0xe;
            x.set_a5_u16(TASK_FLAGS, v);
            x.set16(0x26, 0x78);
        }
        6 | 0xa => {}
        8 => {
            x.add16(0x26, 0xffff);
            if x.u16(0x26) == 0 {
                next_substate(x);
            }
        }
        0xc => {
            next_substate(x);
            let o = Obj(adr16(x.u16(0x24)));
            o.set_u32(x.b, 4, 0x0600_0000);
        }
        0xe => {
            next_substate(x);
            let next = adr16(x.u16(0x20)) + PLAYER_STRIDE;
            x.set16(0x20, next as u16);
            mask_tasks(x, 0xfff1);
            x.set16(0x26, 4);
        }
        0x10 => {
            x.add16(0x26, 0xffff);
            if x.u16(0x26) == 0 {
                x.set8(0x5, 0);
            }
        }
        _ => {}
    }
}

/// Writes the name (subroutine 0xd0870): list of tile indices at `list` (end 0xff; beyond
/// 0xf8, a pattern of six consecutive tiles from a table), each tile drawn by 0xb230e at `at` (which moves on) with
/// attribute `attr`.
fn write_name<R: Bench>(x: &mut Cx<'_, R>, list: u32, mut at: u32, attr: u16) {
    const TILES: u32 = 0xb2920;
    const PATTERNS: u32 = 0xb2a24;
    let mut p = list;
    if x.data8(p) >= 0xf9 {
        let code = (x.data8(p) as i8 as i16).wrapping_neg().wrapping_sub(2) as u16;
        let mut tile = x.data16(PATTERNS.wrapping_add(code.wrapping_mul(2) as i16 as i32 as u32));
        for _ in 0..6 {
            // 0xb230e leaves D0 = tile + 0x11; the pattern advances by two (D0 - 0xf)
            at = crate::families::effects_creators::tile_block(x.b, at, tile, attr).0;
            tile = tile.wrapping_add(2);
        }
        return;
    }
    loop {
        let c = x.data8(p);
        p += 1;
        if c == 0xff {
            return;
        }
        let tile = x.data16(TILES + 2 * c as u32);
        at = crate::families::effects_creators::tile_block(x.b, at, tile, attr).0;
    }
}

// --- variant 24: results table -----------------------------------------------------------

/// Variant 24 (0xd0a0c): prepares the results background (4x4 blocks at 0x91ba00, copy of palette
/// 0x91cc00 into the object, background at 0x200), then blinks the palette and from time to time launches a
/// SOBJ (variant 0x16) tied to the object. State 4 turns the blinking off.
pub fn results_panel<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            x.set_a5_u16(0x56e, 4);
            let end = load_blocks(x, rom::LOAD_SMALL, 0x91ba00, 8);
            load_blocks(x, rom::LOAD_SMALL, end, 8);
            for i in 0..8 {
                let v = rd32(x.b, 0x91cc00 + 4 * i);
                x.set32(0x30 + 4 * i, v);
            }
            x.set8(0x20, 0);
            x.set_a5_u16(a5::BG_X, 0x200);
            x.set_a5_u16(a5::BG_Y, 0);
            let (px, py) = (x.a5_u16(a5::BG_X), x.a5_u16(a5::BG_Y));
            x.set_a5_u16(a5::BG_X_PREV, px);
            x.set_a5_u16(a5::BG_Y_PREV, py);
            x.set8(0x20, 1);
            let low = rd32(x.b, 0x91cc00) & 0xf000_f000;
            for i in 0..8 {
                wr32(x.b, 0x91cc00 + 4 * i, 0x0f00_0f00 | low);
            }
            results_tick(x)
        }
        2 => results_tick(x),
        4 => {
            next_state(x);
            fade_flag(x)
        }
        _ => {}
    }
}

fn fade_flag<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x20, 0xff);
    mask_tasks(x, 0xfff7);
}

fn results_tick<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x20) != 0 {
        return fade_flag(x);
    }
    x.set8(0x20, 2);
    if rng(x) & 7 != 0 {
        return;
    }
    let v = x.a5_u16(TASK_FLAGS) | 8;
    x.set_a5_u16(TASK_FLAGS, v);
    let Some(o) = spawn_sobj(x, 0x16) else { return };
    o.set_u16(x.b, 0x46, x.o.0 as u16);
}
