use crate::world::data::RomAddr;
use crate::actors::families::shell::vars;
use crate::actors::families::shell::records::Base;
use crate::actors::families::cx::Bench;
use crate::world::objects::Current;
use crate::actors::families::cx::{handlers, Cx};
use crate::world::objects::{pools, Obj};
use crate::actors::families::shell::tables::{tables, Read};

const ANIMATIONS: RomAddr = RomAddr(0x1bebc2);
/// Realignment of the object after the collision with the terrain (`terrain::push_68k`).
/// Rows of `pushes_against` (offset of the player against the block, per character) by push direction.
const AGAINST_HIGH: u32 = 0;
const AGAINST_LOW: u32 = 0x10;
const AGAINST_LEFT_1: u32 = 0x20;
const AGAINST_RIGHT_1: u32 = 0x30;
const AGAINST_LEFT_0: u32 = 0x40;
const AGAINST_RIGHT_0: u32 = 0x50;

mod f {
    /// Non nul : objet suspendu.
    pub const HIDDEN: u32 = 0x40;
    pub const SET: u32 = 0x5c;
    /// Bounds of variant 0 (minimum X excluded, maximum X).
    pub const X_MIN: u32 = 0x60;
    pub const X_MAX: u32 = 0x62;
    pub const STOPS: u32 = 0x64;
    pub const MOBILE: u32 = 0x65;
    pub const PUSHES: u32 = 0x66;
    /// Character of the pushing player × 2.
    pub const CHARACTER: u32 = 0x68;
    /// Pushing player (word, 0 = none).
    pub const PLAYER: u32 = 0x6c;
}
/// Fields of the pushing player.
mod j {
    /// States `+4..+6` (the `+4` long masked) of the player who is pushing.
    pub const STATES: u32 = 0x4;
    pub const PUSHING: u32 = 0x0200_2e00;
    pub const CHARACTER: u32 = 0x3;
    pub const DIRECTION: u32 = 0x7;
    /// Bit 4 of `+0x6e`: step of two pixels; `+0x33` non-zero: one more pixel.
    pub const MOMENTUM: u32 = 0x6e;
    pub const FORCE: u32 = 0x33;
}
/// Push directions (`+7` of the player).
mod dir {
    pub const HIGH: u8 = 2;
    pub const LOW: u8 = 4;
    pub const LEFT: u8 = 6;
    pub const RIGHT: u8 = 8;
}

fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::HIDDEN) != 0 {
        return;
    }
    let state = x.u8(Obj::STATE);
    match state {
        0 => {
            let v = 2 * x.u8(Obj::VARIANT) as u32;
            if v == 0 { init_0(x) } else { init_1(x) }
            x.reinsert();
        }
        2 => {
            if vars::GAME_PAUSED.get(x.b) == 0 {
                let v = 2 * x.u8(Obj::VARIANT) as u32;
                if x.u8(Base::SUB_STATE) == 0 {
                    if v == 0 { active_0(x) } else { active_1(x) }
                }
            }
            x.reinsert();
        }
        _ => x.free(&pools::BOX),
    }
}

fn init_common<R: Bench>(x: &mut Cx<'_, R>, mobile: u8) {
    x.add8(Obj::STATE, 2);
    x.set8(Obj::ACTIVE, 1);
    x.set8(Base::BOX_SET, 0x35);
    x.set8(f::MOBILE, mobile);
}

fn init_0<R: Bench>(x: &mut Cx<'_, R>) {
    init_common(x, 1);
    x.set16(Obj::X, 0x234);
    x.set16(Obj::Y, 0x290);
    x.set16(f::X_MAX, 0x290);
    x.set16(f::X_MIN, 0x1d8);
    x.anim_start(ANIMATIONS.0, 7);
    let Some(set) = x.alloc(&pools::SET) else { return };
    set.set_u8(x.b, Obj::ACTIVE, 1);
    set.set_u8(x.b, Obj::KIND, 7);
    set.set_u8(x.b, Obj::VARIANT, 0xc);
    set.set_u8(x.b, Base::SETTING, 1);
    x.set16(f::SET, set.0 as u16);
}

fn init_1<R: Bench>(x: &mut Cx<'_, R>) {
    init_common(x, 0);
    x.anim_start(ANIMATIONS.0, 6);
}

/// The pushing player (A1), if still pushing; otherwise the block forgets them.
fn pusher<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
    if x.u16(f::PLAYER) == 0 {
        return None;
    }
    let player = x.linked(f::PLAYER);
    let states = player.u32(x.b, j::STATES) & 0xffff_ff00;
    if states != j::PUSHING {
        x.set8(f::PUSHES, 0);
        x.set16(f::PLAYER, 0);
        return None;
    }
    let character = (player.u8(x.b, j::CHARACTER) & 0xfe) as u32;
    x.set16(f::CHARACTER, character as u16);
    Some(player)
}

/// Places the player back against the block on axis `axe` (+8 or +0x10).
fn realign_player<R: Bench>(x: &mut Cx<'_, R>, axe: u32, table: u32) {
    let player = x.linked(f::PLAYER);
    let v = x.u16(axe);
    player.set_u16(x.b, axe, v);
    let i = x.u16(f::CHARACTER) as i16 as i32 as u32;
    let d = tables().pushes_against.at16(x.b, table.wrapping_add(i));
    player.set_u16(x.b, axe, v.wrapping_add(d));
}

/// Variant 0: sliding in X between the bounds.
fn active_0<R: Bench>(x: &mut Cx<'_, R>) {
    let px = x.u16(Obj::X);
    if px > x.u16(f::X_MAX) || px <= x.u16(f::X_MIN) {
        x.add8(Base::SUB_STATE, 2);
        x.set8(f::STOPS, 1);
        let set = x.linked(f::SET);
        set.set_u8(x.b, Base::SETTING, 0);
        return;
    }
    let Some(player) = pusher(x) else { return };
    let (sign, table) = match player.u8(x.b, j::DIRECTION) {
        dir::LEFT => (-1i16, AGAINST_LEFT_0),
        dir::RIGHT => (1, AGAINST_RIGHT_0),
        _ => return,
    };
    if player.u8(x.b, j::MOMENTUM) & 0x10 != 0 {
        x.add16(Obj::X, (2 * sign) as u16);
    }
    if player.u8(x.b, j::FORCE) != 0 {
        x.add16(Obj::X, sign as u16);
    }
    realign_player(x, 0x8, table);
}

/// One step of variant 1: the block advances then realigns itself with the terrain.
fn step<R: Bench>(x: &mut Cx<'_, R>, axe: u32, d: i16) {
    x.add16(axe, d as u16);
    realign_scenery(x);
}

fn realign_scenery<R: Bench>(x: &mut Cx<'_, R>) {
    x.push_terrain();
    // 0x35c0: keeps the crate on screen like a player
    crate::actors::player::walk::clamp_to_screen(x.b, crate::world::objects::player::Player(x.o.0));
}

/// Variant 1: sliding in height (directions 2 and 4) or in X (6 and 8).
fn active_1<R: Bench>(x: &mut Cx<'_, R>) {
    let Some(player) = pusher(x) else { return };
    let direction = player.u8(x.b, j::DIRECTION);
    let (axe, sign, table) = match direction {
        dir::HIGH => (0x10, 1i16, AGAINST_HIGH),
        dir::LOW => (0x10, -1, AGAINST_LOW),
        dir::LEFT => (0x8, -1, AGAINST_LEFT_1),
        dir::RIGHT => (0x8, 1, AGAINST_RIGHT_1),
        _ => return,
    };
    let horizontal = axe == 0x8;
    let step_long = if horizontal { 2 * sign } else { sign };
    if player.u8(x.b, j::MOMENTUM) & 0x10 != 0 {
        step(x, axe, step_long);
    }
    // the player is read again after each call
    let player = x.linked(f::PLAYER);
    if player.u8(x.b, j::FORCE) != 0 {
        if horizontal {
            x.add16(axe, sign as u16);
        } else {
            step(x, axe, sign);
        }
    }
    if horizontal {
        realign_scenery(x);
    }
    realign_player(x, axe, table);
}

handlers! {
    BOX_PUSHES 0xda7f2 "box_pushes" run,
}
