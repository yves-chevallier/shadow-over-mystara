//! Boss of type 13 (0x8f59c): the black dragon flying in scenes 7 and 9 (h). See `docs/families/boss.md`.
//!
//! It does not fight: it follows the camera at the left of the screen (`scroll - 0x80`), floats up and down (`bob`),
//! then, at fixed scroll abscissae (`STOPS.0`), stops for a cutscene (a "message" object obtained through `0xc8312`),
//! moves toward the camera leader, follows it, moves away from it and leaves. At birth it creates two ESHL shots of
//! type 0x14 that keep a link to it (`+0x50`).
//!
//! Substates `+6` of the active state (`+5` = 0; `+5` = 2 releases the object):
//!
//! | `+6` | Role |
//! |---|---|
//! | 0 | prepares the stage |
//! | 2 | waits for the next abscissa of `STOPS.0`, then starts the cutscene |
//! | 4 | waits for the end of the cutscene |
//! | 6 | moves toward the leader |
//! | 8, 12 | follows the leader for a while (`+0xa2`) |
//! | 10 | same, then holds the position while `+0xa5` bit 0 is set |
//! | 14 | moves away and leaves the screen |

use crate::families::cx::Bench;
use crate::data::RomAddr;
use crate::objects::Obj;
use crate::record::record;
use super::boss_common::*;
use super::cx::handlers;
use crate::player::Player;

/// Dragon animation set.
const ANIMS: RomAddr = RomAddr(0x17d7d2);
/// Abscissa thresholds (camera + 0x100) that trigger each stage (words, `data/enemies.json`).
const STOPS: &str = "boss_13/stops";
/// Floating speeds: 4 words (VZ, AZ, VY, AY) starting at `4 × draw` (overlapping windows, like the original).
const BOB_TABLE: &str = "boss_13/bob_speeds";

/// Fields of this boss beyond those of `Enemy` and `Boss` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Dragon(pub Obj);

impl Dragon {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Dragon {
        /// Bit 0: cutscene requested; bit 1: stage passed.
        cutscene, set_cutscene, CUTSCENE: u8 @ 0x42;
        /// Non-zero while the cutscene talks (h).
        talking, set_talking, TALKING: u8 @ 0x43;
        /// Floating phase (0: speeds to draw, 1: speeds to invert) and timer.
        bob_phase, set_bob_phase, BOB_PHASE: u8 @ 0xa0;
        bob_timer, set_bob_timer, BOB_TIMER: u8 @ 0xa1;
        /// Substate timer.
        timer, set_timer, TIMER: u16 @ 0xa2;
        /// Bit 0: holds its position while following the leader (h).
        hold, set_hold, HOLD: u8 @ 0xa5;
        /// Index of the next stage of `STOPS`.
        stop_index, set_stop_index, STOP_INDEX: u8 @ 0xa7;
        /// Drawn floating speeds (VZ, AZ, VY, AY).
        bob_speeds, set_bob_speeds, BOB_SPEEDS: u16 @ 0xa8;
        /// "Message" object of the cutscene (word).
        message, set_message, MESSAGE: u16 @ 0xb0;
        /// Non-zero: it left while holding its position (h).
        leaving, set_leaving, LEAVING: u8 @ 0xb2;
        /// Non-zero: slow departure (8 pixels a frame).
        slow, set_slow, SLOW: u8 @ 0xbc;
    }
}

/// The camera leader that the dragon accompanies (a player, `+0x58` read as a word).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Leader(pub Obj);

impl Leader {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Leader {
        /// Horizontal step of the leader this frame (word) (h).
        step_x, set_step_x, STEP_X: u16 @ 0x58;
    }
}
use crate::objects::pools;
use crate::objects::Current;


/// Long states (`+4..+7`): active, `+6` = 0; active, `+6` = 0xe.
const ACTIVE_FIRST: u32 = 0x0200_0000;
const ACTIVE_LEAVE: u32 = 0x0200_0e00;

/// The object followed by the camera, if there is one.
fn leader<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
    let lead = var::LEADER.get(x.b);
    (lead != 0).then_some(Obj(lead as i16 as i32 as u32 & 0xff_ffff))
}

/// Camera abscissa.
fn scroll_x<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    var::SCROLL_X.get(x.b)
}

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
    if (pools::ESHL.free_count(x.b) as i16) < 2 {
        return;
    }
    let Some(lead) = leader(x) else { return };
    x.add8(f::STATE, 2);
    var::FROZEN.set(x.b, 0);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(f::FLIP, 0x20);
    x.set8(Enemy::AI_SPARE_ROW, 2);
    x.set8(f::BOX_SET, 0x29);
    let y = lead.u16(x.b, Obj::Z).wrapping_sub(0x10);
    x.set16(Obj::Z, y);
    let left = scroll_x(x).wrapping_sub(0x80);
    x.set16(Obj::X, left);
    x.set8(Dragon::BOB_PHASE, 0);
    x.set8(Dragon::BOB_TIMER, 0x10);
    x.set16(Dragon::TIMER, 0x258);
    x.set16(Enemy::VZ, 0x300);
    x.set16(Enemy::GRAVITY, (-0x28i16) as u16);
    for variant in [1, 0] {
        let shot = alloc(x, &pools::ESHL);
        let active = x.u8(Enemy::ACTIVE);
        shot.set_u8(x.b, Obj::ACTIVE, active);
        shot.set_u8(x.b, Obj::VARIANT, variant);
        shot.set_u8(x.b, Obj::KIND, 0x14);
        shot.set_u16(x.b, f::OWNER, x.o.0 as u16);
    }
    x.anim_start(ANIMS.0, 0);
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
    x.set16(Obj::X, left);
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

/// Has the camera passed the next threshold of `STOPS.0`? If so, moves on to the next one.
fn next_stop<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let reach = scroll_x(x).wrapping_add(0x100) as i16;
    let i = x.u8(Dragon::STOP_INDEX) as u32;
    let stop = word(x, STOPS, 2 * i) as i16;
    if stop >= reach {
        return false;
    }
    x.add8(Dragon::STOP_INDEX, 1);
    true
}

/// End requested (`RETREAT_FLAG`) or threshold passed: the dragon switches to "departure" mode (`+6` = 0xe).
fn must_leave<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    var::END_COUNTDOWN.get(x.b) != 0 || next_stop(x)
}

fn leave_now<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, ACTIVE_LEAVE);
    x.set8(Dragon::SLOW, 1);
}

fn begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
    x.set_bit(Dragon::CUTSCENE, 0, false);
    x.set_bit(Dragon::CUTSCENE, 1, false);
    x.set8(Dragon::LEAVING, 0);
    // 0xdf4bc: SOBJ object 7 linked through `+0x46`
    super::small::linked_via(&mut super::effects_creators::leftovers::Pure, x.b, x.o, &crate::objects::pools::SOBJ, 0x7, 0x46);
    drift(x);
}

/// Freezes or releases the game and the players during the cutscene.
fn freeze_players<R: Bench>(x: &mut Cx<'_, R>, on: bool) {
    for player in crate::ram::PLAYERS.iter() {
        let player = Player(player.0);
        if player.active(x.b) != 0 {
            player.set_spell_frozen(x.b, on as u8);
        }
    }
}

fn wait_stop<R: Bench>(x: &mut Cx<'_, R>) {
    if next_stop(x) {
        x.set32(f::STATE, ACTIVE_FIRST);
        x.set_bit(Dragon::CUTSCENE, 1, true);
        return;
    }
    if x.bit(Dragon::CUTSCENE, 0) && var::CAMERA_MODE.get(x.b) != 4 && var::FROZEN.get(x.b) == 0 {
        var::PAUSE.set(x.b, 1);
        var::DISPLAY_PANELS.set(x.b, 1);
        var::BLINK_SUSPENDED.set(x.b, 1);
        freeze_players(x, true);
        if let Some(bubble) = crate::families::creators::scene_set(x.b, 0xe) {
            x.set16(Dragon::MESSAGE, bubble.word());
            x.add8(f::SUB2, 2);
            lib::sound(x, crate::sound::SCENE_MUSIC_OR_STOP);
            x.set8(Dragon::TALKING, 1);
            x.set_bit(Dragon::CUTSCENE, 0, false);
        }
    }
    drift(x);
}

fn wait_cutscene<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(Dragon::MESSAGE) != 0 && Message(x.linked(Dragon::MESSAGE)).done(x.b) != 0 {
        var::PAUSE.set(x.b, 0);
        var::BLINK_SUSPENDED.set(x.b, 0);
        freeze_players(x, false);
        x.add8(f::SUB2, 2);
        x.set16(Dragon::TIMER, 0x12c);
        x.set16(Enemy::VX, 0x200);
        if let Some(lead) = leader(x) {
            let y = lead.u16(x.b, Obj::Y).wrapping_add(0x10);
            x.set16(Obj::Y, y);
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
        let dx = Leader(lead).step_x(x.b);
        x.add16(Obj::X, dx);
        let reach = scroll_x(x).wrapping_add(0x50);
        if x.in_arena(reach) {
            x.add8(f::SUB2, 2);
            x.set16(Dragon::TIMER, 0xb4);
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
    let left = x.u16(Dragon::TIMER).wrapping_sub(1);
    x.set16(Dragon::TIMER, left);
    if (left as i16) >= 0 {
        glide(x);
    } else {
        x.add8(f::SUB2, 2);
    }
}

fn follow_and_hold<R: Bench>(x: &mut Cx<'_, R>) {
    if must_leave(x) {
        x.set8(Dragon::LEAVING, 1);
        return leave_now(x);
    }
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set_bit(Dragon::HOLD, 0, true);
            glide(x);
        }
        _ if x.bit(Dragon::HOLD, 0) => glide(x),
        _ => {
            x.add8(f::SUB2, 2);
            x.set8(Enemy::SUB_STEP, 0);
            x.set16(Dragon::TIMER, 0x12c);
        }
    }
}

fn leave<R: Bench>(x: &mut Cx<'_, R>) {
    next_stop(x);
    let Some(lead) = leader(x) else { return };
    let left = scroll_x(x).wrapping_sub(0x80);
    if x.u16(Obj::X).wrapping_sub(left) >= 0x200 {
        lib::sound(x, crate::sound::SCENE_MUSIC);
        x.set8(Dragon::TALKING, 0);
        x.set_bit(Dragon::HOLD, 0, false);
        x.set8(Dragon::SLOW, 0);
        x.set32(f::STATE, ACTIVE_FIRST);
        return;
    }
    let vx = if var::END_COUNTDOWN.get(x.b) != 0 {
        -0x200
    } else if x.u8(Dragon::SLOW) != 0 {
        -8
    } else {
        (Leader(lead).step_x(x.b) as i16).wrapping_sub(var::PLAYERS_ALIVE.get(x.b) as i16)
    };
    x.set16(Enemy::VX, vx as u16);
    let y = lead.u16(x.b, Obj::Y).wrapping_sub(0x10);
    x.set16(Obj::Y, y);
    bob(x);
    x.move_x();
    x.move_yz();
    x.anim_step();
}

/// Moves next to the leader: same depth, abscissa kept within `[leader - 0xe0, leader - 0x60]`.
fn chase<R: Bench>(x: &mut Cx<'_, R>) {
    let Some(lead) = leader(x) else { return };
    let y = lead.u16(x.b, Obj::Y).wrapping_sub(0x10);
    x.set16(Obj::Y, y);
    let mut dx = Leader(lead).step_x(x.b);
    let lx = lead.u16(x.b, Obj::X);
    let mine = x.u16(Obj::X);
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
    x.set16(Obj::X, pos);
    bob(x);
}

/// Floating: every 16 frames, new speeds drawn from `BOB_TABLE.0`, then their opposites.
fn bob<R: Bench>(x: &mut Cx<'_, R>) {
    let inverse = x.u8(Dragon::BOB_PHASE) != 0;
    let t = x.u8(Dragon::BOB_TIMER).wrapping_sub(1);
    x.set8(Dragon::BOB_TIMER, t);
    if (t as i8) >= 0 {
        return;
    }
    if inverse {
        for k in 0..4 {
            let v = x.u16(Dragon::BOB_SPEEDS + 2 * k);
            x.set16(Enemy::VZ + 2 * k, v.wrapping_neg());
        }
    } else {
        let pick = ((x.random() as u32) & 7) * 4;
        for k in 0..4 {
            let v = word(x, BOB_TABLE, pick + 2 * k);
            x.set16(Dragon::BOB_SPEEDS + 2 * k, v);
            x.set16(Enemy::VZ + 2 * k, v);
        }
    }
    x.set8(Dragon::BOB_PHASE, !inverse as u8);
    x.set8(Dragon::BOB_TIMER, 0x10);
}

handlers! {
    0x8f59c "boss_13" run,
}
