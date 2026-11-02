//! Boss of type 13 (0x8f59c): the black dragon flying in scenes 7 and 9 (h). See `docs/families/boss.md`.
//!
//! It does not fight: it follows the camera at the left of the screen (`scroll - 0x80`), floats up and down (`bob`),
//! then, at fixed scroll abscissae (`STOPS`), stops for a cutscene (a "message" object obtained through `0xc8312`),
//! moves toward the camera leader, follows it, moves away from it and leaves. At birth it creates two ESHL shots of
//! type 0x14 that keep a link to it (`+0x50`).
//!
//! Substates `+6` of the active state (`+5` = 0; `+5` = 2 releases the object):
//!
//! | `+6` | Role |
//! |---|---|
//! | 0 | prepares the stage |
//! | 2 | waits for the next abscissa of `STOPS`, then starts the cutscene |
//! | 4 | waits for the end of the cutscene |
//! | 6 | moves toward the leader |
//! | 8, 12 | follows the leader for a while (`+0xa2`) |
//! | 10 | same, then holds the position while `+0xa5` bit 0 is set |
//! | 14 | moves away and leaves the screen |

use crate::families::cx::Bench;
use super::boss_common::*;
use super::cx::handlers;
use crate::mem::wr8;

/// Dragon animation set.
const ANIMS: u32 = 0x17d7d2;
/// Abscissa thresholds (camera + 0x100) that trigger each stage (words, opcode space).
const STOPS: u32 = 0x8f97a;
/// Floating speeds: 4 words (VZ, AZ, VY, AY) starting at `4 × draw` (overlapping windows, like the original), data
/// space.
const BOB_TABLE: u32 = 0x8faa2;

/// Fields specific to this type.
mod g {
    /// Bit 0: cutscene requested; bit 1: stage passed.
    pub const CUTSCENE: u32 = 0x42;
    pub const TALKING: u32 = 0x43;
    /// Floating phase (0: speeds to draw, 1: speeds to invert) and timer.
    pub const BOB_PHASE: u32 = 0xa0;
    pub const BOB_TIMER: u32 = 0xa1;
    /// Substate timer.
    pub const TIMER: u32 = 0xa2;
    pub const HOLD: u32 = 0xa5;
    /// Index of the next stage of `STOPS`.
    pub const STOP_INDEX: u32 = 0xa7;
    /// Drawn floating speeds (VZ, AZ, VY, AY).
    pub const BOB_SPEEDS: u32 = 0xa8;
    /// "Message" object of the cutscene (word).
    pub const MESSAGE: u32 = 0xb0;
    pub const LEAVING: u32 = 0xb2;
    pub const SLOW: u32 = 0xbc;
}
use g::*;
use crate::objects::pools;
use crate::objects::Current;

/// Byte `0xff0ec6` (`A5-0x713a`): non-zero, the dragon withdraws.
const RETREAT_FLAG: u32 = 0xff0ec6;
const SCENE_PHASE: u32 = 0x51e;
const SHOT_SLOTS: u32 = 0x7e64;
const PLAYERS: u32 = 0xff85de;

/// Long states (`+4..+7`): active, `+6` = 0; active, `+6` = 0xe.
const ACTIVE_FIRST: u32 = 0x0200_0000;
const ACTIVE_LEAVE: u32 = 0x0200_0e00;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if var::FROZEN.get(x.b) != 0 {
        return;
    }
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        _ => x.free(&pools::ENEMY),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.a5_u16(SHOT_SLOTS) as i16) < 2 {
        return;
    }
    let Some(lead) = leader(x) else { return };
    x.add8(f::STATE, 2);
    var::FROZEN.set(x.b, 0);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(f::FLIP, 0x20);
    x.set8(0x6f, 2);
    x.set8(f::BOX_SET, 0x29);
    let y = lead.u16(x.b, 0xc).wrapping_sub(0x10);
    x.set16(0xc, y);
    let left = scroll_x(x).wrapping_sub(0x80);
    x.set16(0x8, left);
    x.set8(BOB_PHASE, 0);
    x.set8(BOB_TIMER, 0x10);
    x.set16(TIMER, 0x258);
    x.set16(0x18, 0x300);
    x.set16(0x1a, (-0x28i16) as u16);
    for variant in [1, 0] {
        let shot = alloc(x, &pools::ESHL);
        let active = x.u8(0);
        shot.set_u8(x.b, 0, active);
        shot.set_u8(x.b, 3, variant);
        shot.set_u8(x.b, 2, 0x14);
        shot.set_u16(x.b, f::OWNER, x.o.0 as u16);
    }
    x.anim_start(ANIMS, 0);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    if x.u8(f::SUB) == 0 {
        stage(x);
    } else {
        x.free(&pools::ENEMY);
    }
    x.reinsert();
}

fn stage<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => begin(x),
        2 => wait_stop(x),
        4 => wait_cutscene(x),
        6 => approach(x),
        8 | 12 => follow(x),
        10 => follow_and_hold(x),
        14 => leave(x),
        _ => {}
    }
}

/// Free flight: attached to the left of the screen, with the floating.
fn drift<R: Bench>(x: &mut Cx<'_, R>) {
    let left = scroll_x(x).wrapping_sub(0x80);
    x.set16(0x8, left);
    bob(x);
    x.move_yz();
    x.anim_step();
}

/// Movement of the dragon that follows the leader.
fn glide<R: Bench>(x: &mut Cx<'_, R>) {
    chase(x);
    x.move_yz();
    x.anim_step();
}

/// Has the camera passed the next threshold of `STOPS`? If so, moves on to the next one.
fn next_stop<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let reach = scroll_x(x).wrapping_add(0x100) as i16;
    let i = x.u8(STOP_INDEX) as u32;
    let stop = x.rom16(STOPS + 2 * i) as i16;
    if stop >= reach {
        return false;
    }
    x.add8(STOP_INDEX, 1);
    true
}

/// End requested (`RETREAT_FLAG`) or threshold passed: the dragon switches to "departure" mode (`+6` = 0xe).
fn must_leave<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.data8(RETREAT_FLAG) != 0 || next_stop(x)
}

fn leave_now<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, ACTIVE_LEAVE);
    x.set8(SLOW, 1);
}

fn begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
    x.set_bit(CUTSCENE, 0, false);
    x.set_bit(CUTSCENE, 1, false);
    x.set8(LEAVING, 0);
    // 0xdf4bc: SOBJ object 7 linked through `+0x46`
    super::small::linked_via(&mut super::effects_creators::leftovers::Pure, x.b, x.o, &crate::objects::pools::SOBJ, 0x7, 0x46);
    drift(x);
}

/// Freezes or releases the game and the players during the cutscene.
fn freeze_players<R: Bench>(x: &mut Cx<'_, R>, on: bool) {
    for k in 0..4 {
        let player = PLAYERS + 0x200 * k;
        if x.data8(player) != 0 {
            wr8(x.b, player + 0x16a, on as u8);
        }
    }
}

fn wait_stop<R: Bench>(x: &mut Cx<'_, R>) {
    if next_stop(x) {
        x.set32(f::STATE, ACTIVE_FIRST);
        x.set_bit(CUTSCENE, 1, true);
        return;
    }
    if x.bit(CUTSCENE, 0) && x.a5_u16(SCENE_PHASE) != 4 && var::FROZEN.get(x.b) == 0 {
        var::PAUSE.set(x.b, 1);
        x.set_a5_u8(0x3c, 1);
        x.set_a5_u8(0x33, 1);
        freeze_players(x, true);
        if let Some(bubble) = crate::families::creators::scene_set(x.b, 0xe) {
            x.set16(MESSAGE, bubble.word());
            x.add8(f::SUB2, 2);
            lib::sound(x, 0xe0d8c);
            x.set8(TALKING, 1);
            x.set_bit(CUTSCENE, 0, false);
        }
    }
    drift(x);
}

fn wait_cutscene<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(MESSAGE) != 0 && x.linked(MESSAGE).u8(x.b, 0x22) != 0 {
        var::PAUSE.set(x.b, 0);
        x.set_a5_u8(0x33, 0);
        freeze_players(x, false);
        x.add8(f::SUB2, 2);
        x.set16(TIMER, 0x12c);
        x.set16(0x14, 0x200);
        if let Some(lead) = leader(x) {
            let y = lead.u16(x.b, 0x10).wrapping_add(0x10);
            x.set16(0x10, y);
            return;
        }
    }
    drift(x);
}

fn approach<R: Bench>(x: &mut Cx<'_, R>) {
    if must_leave(x) {
        return leave_now(x);
    }
    if let Some(lead) = leader(x) {
        let dx = lead.u16(x.b, 0x58);
        x.add16(0x8, dx);
        let reach = scroll_x(x).wrapping_add(0x50);
        if x.in_arena(reach) {
            x.add8(f::SUB2, 2);
            x.set16(TIMER, 0xb4);
        }
    }
    bob(x);
    x.move_xyz();
    x.anim_step();
}

fn follow<R: Bench>(x: &mut Cx<'_, R>) {
    if must_leave(x) {
        return leave_now(x);
    }
    let left = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, left);
    if (left as i16) >= 0 {
        glide(x);
    } else {
        x.add8(f::SUB2, 2);
    }
}

fn follow_and_hold<R: Bench>(x: &mut Cx<'_, R>) {
    if must_leave(x) {
        x.set8(LEAVING, 1);
        return leave_now(x);
    }
    match x.u8(f::SUB2 + 1) {
        0 => {
            x.add8(f::SUB2 + 1, 2);
            x.set_bit(HOLD, 0, true);
            glide(x);
        }
        _ if x.bit(HOLD, 0) => glide(x),
        _ => {
            x.add8(f::SUB2, 2);
            x.set8(f::SUB2 + 1, 0);
            x.set16(TIMER, 0x12c);
        }
    }
}

fn leave<R: Bench>(x: &mut Cx<'_, R>) {
    next_stop(x);
    let Some(lead) = leader(x) else { return };
    let left = scroll_x(x).wrapping_sub(0x80);
    if x.u16(0x8).wrapping_sub(left) >= 0x200 {
        lib::sound(x, 0xe0c5c);
        x.set8(TALKING, 0);
        x.set_bit(HOLD, 0, false);
        x.set8(SLOW, 0);
        x.set32(f::STATE, ACTIVE_FIRST);
        return;
    }
    let vx = if x.data8(RETREAT_FLAG) != 0 {
        -0x200
    } else if x.u8(SLOW) != 0 {
        -8
    } else {
        (lead.u16(x.b, 0x58) as i16).wrapping_sub(var::PLAYERS_ALIVE.get(x.b) as i16)
    };
    x.set16(0x14, vx as u16);
    let y = lead.u16(x.b, 0x10).wrapping_sub(0x10);
    x.set16(0x10, y);
    bob(x);
    x.move_x();
    x.move_yz();
    x.anim_step();
}

/// Moves next to the leader: same depth, abscissa kept within `[leader - 0xe0, leader - 0x60]`.
fn chase<R: Bench>(x: &mut Cx<'_, R>) {
    let Some(lead) = leader(x) else { return };
    let y = lead.u16(x.b, 0x10).wrapping_sub(0x10);
    x.set16(0x10, y);
    let mut dx = lead.u16(x.b, 0x58);
    let lx = lead.u16(x.b, 0x8);
    let mine = x.u16(0x8);
    if (lx.wrapping_sub(0x80).wrapping_sub(mine) as i16) >= 0 {
        dx = dx.wrapping_add(1);
    }
    let next = mine.wrapping_add(dx) as i16;
    let (hi, lo) = (lx.wrapping_sub(0x60), lx.wrapping_sub(0xe0));
    let pos = if next >= hi as i16 {
        hi
    } else if next > lo as i16 {
        next as u16
    } else {
        lo
    };
    x.set16(0x8, pos);
    bob(x);
}

/// Floating: every 16 frames, new speeds drawn from `BOB_TABLE`, then their opposites.
fn bob<R: Bench>(x: &mut Cx<'_, R>) {
    let inverse = x.u8(BOB_PHASE) != 0;
    let t = x.u8(BOB_TIMER).wrapping_sub(1);
    x.set8(BOB_TIMER, t);
    if (t as i8) >= 0 {
        return;
    }
    if inverse {
        for k in 0..4 {
            let v = x.u16(BOB_SPEEDS + 2 * k);
            x.set16(0x18 + 2 * k, v.wrapping_neg());
        }
    } else {
        let pick = (random(x) & 7) * 4;
        for k in 0..4 {
            let v = x.data16(BOB_TABLE + pick + 2 * k);
            x.set16(BOB_SPEEDS + 2 * k, v);
            x.set16(0x18 + 2 * k, v);
        }
    }
    x.set8(BOB_PHASE, !inverse as u8);
    x.set8(BOB_TIMER, 0x10);
}

handlers! {
    0x8f59c "boss_13" #[without] run,
}
