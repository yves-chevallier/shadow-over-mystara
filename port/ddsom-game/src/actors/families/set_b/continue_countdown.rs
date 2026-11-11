use super::*;
use crate::hw::mem::{rd8, wr16, wr32};
use crate::actors::families::set::tables as set_tables;
use crate::actors::families::set::vars;
use crate::world::objects::Obj;
use crate::world::objects::player::Player;
use crate::actors::families::set::fields::{SetObj, WeaponObj};
use crate::actors::families::set_b::fields::ContinueObj;
use crate::actors::families::set_b::fields::CameraFields;

const PALETTE_SRC: u32 = 0x25c0e0;
const PALETTE_DST: u32 = 0x91c420;
const BRIGHT: u32 = 0xf000_f000;
/// Byte (A5-0x5b8e = 0xff2472): sound muted.
const MUTED: u32 = 0xff_2472;
const SCRIPT_ON: u32 = 0xd6922;
const SCRIPT_OFF: u32 = 0xd694e;
/// Variant -> invitation parameter (`+0x2c`), according to the level `A5+0x34` (bytes).
const PROMPT_BY_STAGE: u32 = 0xd6464;
const DIGITS: u32 = 0x912b38;
const GET_READY: u32 = 0x912830;
const GAME_OVER: u32 = 0x9127b0;
const ROW: u32 = 0x80;
const ATTR: u32 = 3;
const COLON: u16 = 0x3a;
/// Rows (left tile, right tile) of the blinking inscription (attribute 3).
const GET_READY_ROWS: [(u16, u16); 16] = [
    (0x192, 0x1a2), (0x193, 0x1a3), (0x1bc, 0x1cc), (0x1bd, 0x1cd),
    (0x1b6, 0x1c6), (0x1b7, 0x1c7), (0x1b6, 0x1c6), (0x1b7, 0x1c7),
    (0x1d4, 0x1e4), (0x1d5, 0x1e5), (0x2fc, 0x2fc), (0x2fc, 0x2fc),
    (0x1bc, 0x1cc), (0x1bd, 0x1cd), (0x1b2, 0x1c2), (0x1b3, 0x1c3),
];
/// End-of-game inscription.
const GAME_OVER_ROWS: [(u16, u16); 18] = [
    (0x1ba, 0x1ca), (0x1bb, 0x1cb), (0x194, 0x1a4), (0x195, 0x1a5),
    (0x19c, 0x1ac), (0x19d, 0x1ad), (0x17c, 0x18c), (0x17d, 0x18d),
    (0x2fc, 0x2fc), (0x2fc, 0x2fc), (0x1b0, 0x1c0), (0x1b1, 0x1c1),
    (0x1be, 0x1ce), (0x1bf, 0x1cf), (0x17c, 0x18c), (0x17d, 0x18d),
    (0x1b6, 0x1c6), (0x1b7, 0x1c7),
];
/// States of the condemned player.
const DOOMED: u32 = 0x0204_0600;
const SAFE_STATE: u32 = 0x0200_2000;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let now = x.u8(Obj::HIDDEN) as i8;
    if now > x.u8(ContinueObj::LAST_SUSPEND) as i8 {
        x.add8(ContinueObj::MINUTES, 1);
        hide(x);
    }
    x.set8(ContinueObj::LAST_SUSPEND, now as u8);
    if x.u8(Obj::VARIANT) != 2 && x.a5_u8(vars::EXIT_LOCK) != 0 {
        return hide(x);
    }
    if x.a5_u8(vars::FROZEN) != 0 || x.a5_u8(GAME_PAUSED) != 0 {
        return hide(x);
    }
    let camera = if x.a5_u8(vars::CAMERA_ALT_ACTIVE) != 0 { 0x54e } else { 0x4fe };
    let camera = Obj(A5 + camera);
    match x.u8(Obj::STATE) {
        0 => init(x, camera),
        2 => counting(x, camera),
        4 => finish(x),
        _ => x.free(&pools::SET),
    }
}

const GAME_PAUSED: u32 = 0x3d;

/// Removes the announcement (once) and cuts the display.
fn hide<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(ContinueObj::SHOWN) == 0 {
        return;
    }
    x.set8(ContinueObj::SHOWN, 0);
    if rd8(x.b, MUTED) != 0 {
        return;
    }
    crate::services::text::clear_box(x.b, SCRIPT_OFF);
}

fn rows<R: Bench>(x: &mut Cx<'_, R>, base: u32, rows: &[(u16, u16)]) {
    for (i, (left, right)) in rows.iter().enumerate() {
        let at = base + ROW * i as u32;
        wr32(x.b, at, (*left as u32) << 16 | ATTR);
        wr32(x.b, at + 4, (*right as u32) << 16 | ATTR);
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>, camera: Obj) {
    x.add8(Obj::STATE, 2);
    x.set16(ContinueObj::DELAY, 0x400);
    let v = camera.u16(x.b, Obj::X);
    x.set16(ContinueObj::CAMERA_X, v);
    x.set8(ContinueObj::TICK, 0);
    let v = x.a5_u8(vars::PLAYERS_ALIVE);
    x.set8(ContinueObj::PLAYERS, v);
    if x.u8(SetObj::SUBSTATE) == 0 {
        let stage = x.a5_u8(vars::SCENE) as u32;
        let v = set_tables::pc_byte(x.b, PROMPT_BY_STAGE + stage);
        x.set8(ContinueObj::PROMPT, v);
    }
    x.set8(SetObj::SUBSTATE, 0);
    // palette: six longs forced to maximum brightness, the last two added
    for i in 0..8 {
        let v = set_tables::long(x.b, PALETTE_SRC + 4 * i);
        let v = if i < 6 { v | BRIGHT } else { v.wrapping_add(BRIGHT) };
        wr32(x.b, PALETTE_DST + 4 * i, v);
    }
    let v = x.u8(ContinueObj::START_MINUTES);
    x.set8(ContinueObj::MINUTES, v);
    let packed = x.u8(ContinueObj::START_SECONDS);
    x.set8(ContinueObj::UNITS, packed);
    x.set8(ContinueObj::TENS, packed >> 4);
    x.set8(ContinueObj::UNITS, packed & 0xf);
}

fn counting<R: Bench>(x: &mut Cx<'_, R>, camera: Obj) {
    let players = x.a5_u8(vars::PLAYERS_ALIVE);
    let known = x.u8(ContinueObj::PLAYERS);
    if players != known {
        if (players as i8) < (known as i8) {
            x.set8(ContinueObj::PLAYERS, players);
        } else {
            x.set16(Obj::STATE, 0x401);
            x.set16(ContinueObj::DELAY, 0);
            return;
        }
    }
    if players == 0 {
        return hide(x);
    }
    if x.a5_u8(vars::SCENES) != 0 {
        x.set16(ContinueObj::DELAY, 0);
        return hide(x);
    }
    if x.u8(Obj::VARIANT) != 1 {
        if !tick(x) {
            return;
        }
        if camera.u16(x.b, CameraFields::MODE) == 4 {
            x.set16(ContinueObj::DELAY, 0x200);
            return;
        }
    }
    prompt(x, camera)
}

/// One-frame countdown; false if the time has elapsed (the players are then condemned).
fn tick<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let n = x.u8(ContinueObj::TICK).wrapping_sub(1);
    x.set8(ContinueObj::TICK, n);
    if (n as i8) < 0 {
        x.set8(ContinueObj::TICK, 0x3c);
        if x.u8(ContinueObj::MINUTES) == 0 {
            x.set8(ContinueObj::SHOWN, 1);
        }
        let n = x.u8(ContinueObj::UNITS).wrapping_sub(1);
        x.set8(ContinueObj::UNITS, n);
        if (n as i8) < 0 {
            x.set8(ContinueObj::UNITS, 9);
            let n = x.u8(ContinueObj::TENS).wrapping_sub(1);
            x.set8(ContinueObj::TENS, n);
            if (n as i8) < 0 {
                x.set8(ContinueObj::TENS, 5);
                let n = x.u8(ContinueObj::MINUTES).wrapping_sub(1);
                x.set8(ContinueObj::MINUTES, n);
                if (n as i8) < 0 {
                    time_out(x);
                    return false;
                }
            }
        }
    }
    if x.u8(ContinueObj::SHOWN) != 0 {
        draw(x);
    }
    true
}

/// The time has elapsed: the living players are condemned.
fn time_out<R: Bench>(x: &mut Cx<'_, R>) {
    for p in players() {
        if p.u8(x.b, pl::ACTIVE) == 0 || p.u8(x.b, pl::STATE) as i8 >= 4 {
            continue;
        }
        if p.u32(x.b, pl::STATE) & 0xffff_ff00 == SAFE_STATE {
            continue;
        }
        p.set_u16(x.b, pl::HEALTH, 0);
        p.set_u32(x.b, pl::STATE, DOOMED);
        let n = x.a5_u8(vars::PLAYERS_ALIVE).wrapping_sub(1);
        x.set_a5_u8(vars::PLAYERS_ALIVE, n);
        if p.u8(x.b, Player::SLOW_8A) != 0 {
            p.set_u8(x.b, Player::SLOW_8A, 0);
            let a2 = Obj(p.u32(x.b, Player::LINK) & 0xff_ffff);
            a2.set_u8(x.b, WeaponObj::SUBSTATE, 8);
            a2.set_u16(x.b, WeaponObj::ACTION, 0);
        }
    }
    x.set16(Obj::STATE, 0x400);
    x.set_a5_u8(vars::INPUT_MASK, 0xff);
    x.set_a5_u16(vars::SCROLL_Y_PLANE1, 0);
}

/// Displays the digits and makes the inscription blink.
fn draw<R: Bench>(x: &mut Cx<'_, R>) {
    let (d0, d1, d2) = (x.u8(ContinueObj::MINUTES) as u16, x.u8(ContinueObj::TENS) as u16, x.u8(ContinueObj::UNITS) as u16);
    for (i, tile) in [0, d0, COLON, d1, d2].into_iter().enumerate() {
        wr16(x.b, DIGITS + ROW * i as u32, tile);
        wr16(x.b, DIGITS + ROW * i as u32 + 2, ATTR as u16);
    }
    let n = x.u8(ContinueObj::BLINK).wrapping_sub(1);
    x.set8(ContinueObj::BLINK, n);
    if n != 0 {
        return;
    }
    x.set8(ContinueObj::BLINK, 0x80);
    if x.u8(ContinueObj::TENS) == 0 {
        x.set8(ContinueObj::BLINK, 0x10);
    }
    let was = x.bit(ContinueObj::BLINK_PHASE, 0);
    x.set_bit(ContinueObj::BLINK_PHASE, 0, !was);
    if !was {
        rows(x, GET_READY, &GET_READY_ROWS);
    } else if rd8(x.b, MUTED) == 0 {
        crate::services::text::clear_box(x.b, SCRIPT_OFF);
    }
}

/// Periodic invitation (h): as long as the camera does not move, creates an invitation SOBJ object every 256
/// frames.
fn prompt<R: Bench>(x: &mut Cx<'_, R>, camera: Obj) {
    if x.u8(Obj::HIDDEN) != 0 {
        return;
    }
    if x.a5_u8(vars::TRANSITION) != 0 {
        return x.set16(ContinueObj::DELAY, 0);
    }
    let here = camera.u16(x.b, Obj::X);
    if here != x.u16(ContinueObj::CAMERA_X) {
        x.set16(ContinueObj::CAMERA_X, here);
        return x.set16(ContinueObj::DELAY, 0x200);
    }
    let n = x.u16(ContinueObj::DELAY).wrapping_sub(1);
    x.set16(ContinueObj::DELAY, n);
    if (n as i16) >= 0 || x.u8(Obj::VARIANT) == 2 {
        return;
    }
    x.set16(ContinueObj::DELAY, 0x100);
    let orientation = x.u8(ContinueObj::PROMPT);
    let _ = crate::actors::effects_aids::scenery_11(x.b, 0, orientation);
}

/// State 4: final inscription then return to state 0 (substate 1) after 0x80 frames.
fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(SetObj::SUBSTATE) == 0 {
        x.add8(SetObj::SUBSTATE, 1);
        x.set16(ContinueObj::DELAY, 0x80);
        if rd8(x.b, MUTED) != 0 {
            return;
        }
        crate::services::text::clear_box(x.b, SCRIPT_ON);
        rows(x, GAME_OVER, &GAME_OVER_ROWS);
        return;
    }
    let n = x.u16(ContinueObj::DELAY).wrapping_sub(1);
    x.set16(ContinueObj::DELAY, n);
    if (n as i16) >= 0 {
        return;
    }
    x.set8(Obj::STATE, 0);
    x.set8(SetObj::SUBSTATE, 1);
    x.set8(ContinueObj::SHOWN, 0);
    if rd8(x.b, MUTED) != 0 {
        return;
    }
    crate::services::text::clear_box(x.b, SCRIPT_ON);
}
