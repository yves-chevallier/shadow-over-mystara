use super::*;
use crate::families::set::tables as set_tables;
use crate::families::set::vars;
use crate::objects::Obj;
use crate::families::set::fields::SetObj;
use crate::families::set_c::fields::ScreenEffect;
use crate::families::set_c::fields::ScreenObj;
use crate::families::set_c::fields::IntroObj;
use crate::families::set_c::fields::FireworksObj;
use crate::families::set_c::fields::RankingObj;
use crate::families::set_c::fields::Conductor;

// --- variant 10: opening shake ------------------------------------------------------------------------

/// Variant 10 (0xcf5e6): shake (table 0xd0b30) until the end of the fade (`A5+0x48f`), then the background
/// accelerates in horizontal scrolling (`+0x14`, `+0x16`) until the speed falls below 0x100,
/// and finally sets the conductor to 2. `+5`: 0 shake, 2 wait for the conductor, 4 scrolling.
pub fn shaking_intro<R: Bench>(x: &mut Cx<'_, R>) {
    const SHAKE: u32 = 0xd0b30;
    const SCROLL_SHAKE: u32 = 0xd0b50;
    if x.u8(Obj::STATE) == 0 {
        next_state(x);
        x.set8(IntroObj::STEP, 0);
    } else if x.u8(Obj::STATE) != 2 {
        return;
    }
    match x.u8(SetObj::SUBSTATE) {
        0 => {
            shake4(x, SHAKE);
            if x.a5_u8(vars::FADE_STEPS) != 0 {
                return;
            }
            next_substate(x);
            x.set16(IntroObj::SPEED, 0x800);
            x.set16(IntroObj::ACCEL, 0xfff8);
            x.set_a5_u16(a5::CAM_X, 0);
            x.set_a5_u16(a5::CAM_Y, 0);
            x.set_a5_u16(a5::BG_X, 0x80);
            x.set_a5_u16(a5::BG_Y, 0);
            copy_cameras(x);
        }
        2 => {
            if conductor(x).u8(x.b, Conductor::SIGNAL) == 1 {
                next_substate(x);
            }
            let v = x.a5_u16(a5::CAM_X).wrapping_add(8);
            x.set_a5_u16(a5::CAM_X, v);
            shake4(x, SCROLL_SHAKE);
        }
        4 => {
            if x.u8(IntroObj::SPEED) != 0 {
                let speed = x.u16(IntroObj::SPEED).wrapping_add(x.u16(IntroObj::ACCEL));
                x.set16(IntroObj::SPEED, speed);
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
                conductor(x).set_u8(x.b, Conductor::SIGNAL, 2);
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
    if !matches!(x.u8(Obj::STATE), 0 | 2) {
        return;
    }
    let boss = x.u32(ScreenObj::CONDUCTOR);
    let mut taken = characters_present(x);
    let mut a6 = PLAYERS;
    for d1 in (0..=5u32).rev() {
        let here = d1 as i8 > 1 && present(x, a6);
        let Some(m) = spawn_sobj(x, 0xe) else {
            break;
        };
        if here {
            m.set_u8(x.b, SetObj::PARAM, d1 as u8);
            m.set_u16(x.b, SetObj::OWNER, a6 as u16);
            m.set_u32(x.b, ScreenObj::CONDUCTOR, boss);
        } else if let Some(n) = free_character(&mut taken) {
            m.set_u8(x.b, SetObj::PARAM, d1 as u8);
            m.set_u16(x.b, SetObj::OWNER, n as u16);
            m.set_u32(x.b, ScreenObj::CONDUCTOR, boss);
        }
        a6 += PLAYER_STRIDE;
    }
    retire(x);
    let Some(m) = spawn_sobj(x, 0) else { return };
    m.set_u8(x.b, SetObj::PARAM, 0xb);
    m.set_u16(x.b, Obj::X, 0x108);
    m.set_u16(x.b, Obj::Y, 0x100);
    m.set_u16(x.b, Obj::Z, 0xff80);
    m.set_u32(x.b, ScreenObj::CONDUCTOR, boss);
}

// --- variant 13: fireworks ----------------------------------------------------------------------------------

/// Variant 13 (0xcf8a0): prepares the palette (0x91c280, with 0xf000f000), then, as long as the
/// conductor does not switch to a negative `+8`, launches from time to time a SOBJ object of variant 0x10
/// (position drawn from table 0xcf974) and, once, a bonus screen (type 15 variant 0xf).
pub fn fireworks<R: Bench>(x: &mut Cx<'_, R>) {
    const PALETTE_ROM: u32 = 0x253280;
    const PALETTE: u32 = 0x91c280;
    const POSITIONS: u32 = 0xcf974;
    match x.u8(Obj::STATE) {
        0 => {
            next_state(x);
            for i in 0..8 {
                let v = set_tables::long(x.b, PALETTE_ROM + 4 * i);
                wr32(x.b, PALETTE + 4 * i, v);
            }
            for i in 0..8 {
                let v = rd32(x.b, PALETTE + 4 * i) | 0xf000_f000;
                wr32(x.b, PALETTE + 4 * i, v);
            }
            x.set8(FireworksObj::STARTED, 0);
        }
        2 => {}
        _ => return,
    }
    if (conductor(x).u8(x.b, Conductor::SIGNAL) as i8) < 0 {
        return retire(x);
    }
    if rng(x) & 7 != 0 {
        return;
    }
    let Some(o) = spawn_sobj(x, 0x10) else { return };
    let owner = x.u32(ScreenObj::CONDUCTOR);
    o.set_u32(x.b, ScreenObj::CONDUCTOR, owner);
    let pick = ((rng(x) & 7) as u32) << 2;
    let (px, pz) = (set_tables::pc_word(x.b, POSITIONS + pick), set_tables::pc_word(x.b, POSITIONS + pick + 2));
    o.set_u16(x.b, Obj::X, px);
    o.set_u16(x.b, Obj::Z, pz);
    o.set_u32(x.b, Obj::Y, 0);
    if x.u8(FireworksObj::STARTED) != 0 {
        return;
    }
    let Some(f) = spawn_finale(x, 0xf) else { return };
    x.set8(FireworksObj::STARTED, 1);
    let owner = x.u32(ScreenObj::CONDUCTOR);
    f.set_u32(x.b, ScreenObj::CONDUCTOR, owner);
}

// --- variant 14: lightning --------------------------------------------------------------------------------

/// Variant 14 (0xcf994): creates lightning bolts (SOBJ variant 0x11, number 0 to 4 drawn without repetition
/// among the free bits of `+0x30`) as long as the second plane (`A5+0x55a`) has not returned to zero;
/// the sound call 0xea83e rings out from time to time.
pub fn lightning<R: Bench>(x: &mut Cx<'_, R>) {
    const ORDER: u32 = 0xcfa3c;
    const SOUND: crate::sound::Sound = crate::sound::CMD_3F;
    match x.u8(Obj::STATE) {
        0 => {
            next_state(x);
            x.set8(FireworksObj::USED, 1);
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
    if x.u8(SetObj::SUBSTATE) == 0 && x.a5_u16(a5::CAM_X) == 0 {
        next_substate(x);
        x.set_bit(FireworksObj::USED, 0, false);
    }
    if x.u8(FireworksObj::USED) == 0x1f {
        return;
    }
    if x.a5_u16(vars::FRAMES_WORD) & 0xf != 0 {
        return;
    }
    let Some(o) = spawn_sobj(x, 0x11) else { return };
    let pick = (rng(x) & 0xf) as u32;
    let mut bit = set_tables::pc_byte(x.b, ORDER + pick);
    while x.u8(FireworksObj::USED) >> (bit & 7) & 1 != 0 {
        bit = bit.wrapping_sub(1);
        if bit & 0x80 != 0 {
            bit = 4;
        }
    }
    x.set_bit(FireworksObj::USED, bit & 7, true);
    o.set_u8(x.b, SetObj::PARAM, bit);
    o.set_u16(x.b, SetObj::OWNER, x.o.0 as u16);
}

// --- variant 15: light shake -----------------------------------------------------------------------------------

/// Variant 15 (0xcfa4c): shakes the background (table 0xd0b70) as long as the conductor has `+8 >= 0`.
pub fn quake_light<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            next_state(x);
            x.set8(IntroObj::STEP, 0);
        }
        2 => {}
        _ => return,
    }
    if (conductor(x).u8(x.b, Conductor::SIGNAL) as i8) < 0 {
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
    match x.u8(Obj::STATE) {
        0 => {
            next_state(x);
            let mut taken: u32 = 0xf000;
            x.set8(RankingObj::ORDER, 0);
            x.set8(RankingObj::SHOWN, 0);
            for p in players() {
                if present(x, p) {
                    taken |= 1 << (rd8(x.b, p + 3) & 31);
                    x.add8(RankingObj::ORDER, 1);
                }
            }
            x.set16(RankingObj::TAKEN, taken as u16);
            x.set16(RankingObj::DELAY, 0xb4);
            let mut left = x.u8(RankingObj::ORDER);
            x.add8(RankingObj::ORDER, 0xff);
            loop {
                left = left.wrapping_sub(1);
                if left == 0 {
                    break;
                }
                x.add16(RankingObj::DELAY, 0xb4);
            }
        }
        2 => {
            if x.u8(SetObj::SUBSTATE) != 0 {
                if x.u8(RankingObj::SHOWN) != 0 {
                    return;
                }
                next_state(x);
                let v = x.a5_u8(vars::STEP).wrapping_add(2);
                x.set_a5_u8(vars::STEP, v);
                return;
            }
            x.add16(RankingObj::DELAY, 0xffff);
            if x.u16(RankingObj::DELAY) != 0 {
                return;
            }
            x.set16(RankingObj::DELAY, 0xb4);
            let mut taken = x.u16(RankingObj::TAKEN) as u32;
            let mut n = (rng(x) & 0xf) as u32;
            while taken >> n & 1 != 0 {
                n = (n + 1) & 0xf;
            }
            taken |= 1 << n;
            x.set16(RankingObj::TAKEN, taken as u16);
            let Some(fx) = spawn_effct(x, 7, n) else { return };
            fx.set_u8(x.b, SetObj::PARAM, n as u8);
            let order = x.u8(RankingObj::ORDER);
            fx.set_u8(x.b, ScreenEffect::ORDER, order);
            fx.set_u16(x.b, SetObj::OWNER, x.o.0 as u16);
            x.add8(RankingObj::ORDER, 1);
            x.add8(RankingObj::SHOWN, 1);
            if x.u16(RankingObj::TAKEN) == 0xffff {
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
    if x.u8(Obj::STATE) == 0 {
        next_state(x);
        x.set8(RankingObj::TAKEN, 4);
        x.set8(RankingObj::STEPS, 0xf);
    } else if x.u8(Obj::STATE) != 2 {
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
    match x.u8(SetObj::SUBSTATE) {
        0 => {
            x.add8(RankingObj::TAKEN, 0xff);
            if x.u8(RankingObj::TAKEN) != 0 {
                return;
            }
            x.set8(RankingObj::TAKEN, 4);
            fade(x, 0x1000_1000);
            x.add8(RankingObj::STEPS, 0xff);
            if x.u8(RankingObj::STEPS) != 0 {
                return;
            }
            next_substate(x);
            x.set8(RankingObj::STEPS, 0xf);
        }
        2 => {
            x.add16(RankingObj::WAIT, 0xffff);
            if x.u16(RankingObj::WAIT) == 0 {
                next_substate(x);
            }
        }
        4 => {
            x.add8(RankingObj::TAKEN, 0xff);
            if x.u8(RankingObj::TAKEN) != 0 {
                return;
            }
            x.set8(RankingObj::TAKEN, 4);
            fade(x, 0xefff_f000);
            x.add8(RankingObj::STEPS, 0xff);
            if x.u8(RankingObj::STEPS) != 0 {
                return;
            }
            next_state(x);
            let target = adr16(x.u16(RankingObj::TARGET));
            wr16(x.b, target + 0x32, 1);
        }
        _ => {}
    }
}
