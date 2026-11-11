//! Secondary actions of the normal game (`+4` = 2, `+5` = 0): run, carry, crouch, push, attack
//! jump specific to each family of characters, and the actions launched by the special moves
//! (`combos`).
//!
//! Like the common actions (`actions`), each is a small automaton whose step is `+7`. The
//! sounds and effects are Rust calls.

use super::actions::to_stand;
use super::checks::{attack_button, item_curse, jump_button};
use super::spawn::Thrown;
use super::states::State;
use super::walk::{clamp_x, crawl_speed, move_x, move_xy, move_xyz, move_xz, move_z, run_speed, walk_speed};
use super::{animation, ram, Ctx, Player};
use crate::world::mem::{rd16, rd32, rd8, wr16, wr32, wr8};
use crate::world::objects::{pools, Obj};
use crate::actors::player::external as ext;
use crate::actors::player::tables::{player_data, Aligned};

mod climb;
pub use climb::*;
mod poses;
pub use poses::*;
mod leaps;
pub use leaps::*;

/// Identifiers (original addresses) of the routines reached (sounds).
mod rom {
    /// Push sound.
    pub const PUSH_SOUND: crate::hw::sound::Sound = crate::hw::sound::CMD_14B_24B;
    /// Rare pose 0x26: sound, flash, creation of a SET object of the scene (0xc8312) of this variant.
    pub const SCENE_SET_VARIANT: u8 = 0x29;
}

/// Fields specific to these actions.
mod field {
    /// Animation event (byte set by the script, consumed by the action).
    pub const ANIM_EVENT: u32 = 0x46;
    /// The player is airborne for a jumping attack (h).
    pub const AIRBORNE: u32 = 0x67;
    /// Aerial attack already given.
    pub const AIR_ATTACK: u32 = 0x6b;
    /// Gesture event of the animation script.
    pub const GESTURE: u32 = 0x33;
    /// Direction stored at the start of an action (orientation opposite to the original one).
    pub const HELD_FACING: u32 = 0x1e8;
}

fn next_step(g: &mut Ctx, p: Player) {
    let step = p.u8(g, Player::STEP).wrapping_add(2);
    p.set_u8(g, Player::STEP, step);
}

/// Direction requested by the directions: bit 0 (right) -> 0x20.
fn facing_for(keys: u8) -> u8 {
    if keys & 1 != 0 {
        0x20
    } else {
        0
    }
}

/// Horizontal speed and acceleration (words), reversed (`neg.l` of the pair) if the player faces left.
fn set_vx_pair(g: &mut Ctx, p: Player, vx: u16, ax: u16) {
    p.set_u16(g, Obj::VX, vx);
    p.set_u16(g, Obj::AX, ax);
    if p.faces_left(g) {
        let pair = p.u32(g, Obj::VX).wrapping_neg();
        p.set_u32(g, Obj::VX, pair);
    }
}

/// One frame out of four (depending on the character), dust under the feet: variant D0, settings D1, speeds D2/D3
/// (which stay in D2.w and D3.w: leftovers of the original).
fn dust(g: &mut Ctx, p: Player, d0: u16, d1: u16, d2: u16, d3: u16) {
    crate::world::mem::set_low_word(&mut g.leftovers.d2, d2);
    crate::world::mem::set_low_word(&mut g.leftovers.d3, d3);
    g.shock(p, d0 as u8, d1, d2, d3);
}

fn dust_turn(g: &mut Ctx, p: Player) -> bool {
    p.u8(g, Player::CHARACTER).wrapping_add(ram::FRAME_COUNTER.get(g.b)) & 3 == 0
}

// ---- run (action 6, 0x3573e) ----------------------------------------------------------------

/// Run duration (frames) and the field that counts it down.
const RUN_TIME: u8 = 200;
const RUN_LEFT: u32 = Player::PARAM;

/// 0x3573e: run. Step 0: start; 2: run (the counter drops by 2 per frame); 4: stopping
/// skid; 6: turning skid.
pub fn run(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, RUN_LEFT, RUN_TIME);
            animation::start(g, p, 7);
            running(g, p);
        }
        2 => running(g, p),
        4 => skid_stop(g, p),
        6 => skid_turn(g, p),
        v => unreachable!("0x3573e : value {v:#x} outside the original jump table"),
    }
}

/// 0x35764.
fn running(g: &mut Ctx, p: Player) {
    if attack_button(g, p) || jump_button(g, p) || run_steer(g, p) {
        return;
    }
    let left = p.u8(g, RUN_LEFT).wrapping_sub(1);
    p.set_u8(g, RUN_LEFT, left);
    if left == 0 {
        to_stand(g, p);
        return;
    }
    run_speed(g, p);
    move_xy(g, p);
    animation::step(g, p);
}

/// Skid: step `step`, speed -0x480 (acceleration `ax`), animation `anim`.
fn start_skid(g: &mut Ctx, p: Player, step: u8, ax: u16, anim: u16) {
    p.set_u8(g, Player::STEP, step);
    set_vx_pair(g, p, (-0x480i16) as u16, ax);
    animation::start(g, p, anim);
}

/// 0x35814: end of run (counter exhausted or no more direction) or turn requested: skid. True
/// if the run is interrupted.
fn run_steer(g: &mut Ctx, p: Player) -> bool {
    let left = p.u8(g, RUN_LEFT).wrapping_sub(1);
    p.set_u8(g, RUN_LEFT, left);
    let keys = p.u8(g, Player::KEYS_HELD) & 0xf;
    if left == 0 || keys == 0 {
        p.set_u8(g, RUN_LEFT, 4);
        start_skid(g, p, 4, 40, 26);
    } else if keys & 3 == 0 || facing_for(keys) == p.u8(g, Player::FACING) {
        return false;
    } else {
        start_skid(g, p, 6, 56, 27);
    }
    g.sound(ext::sound::SLIDE, p);
    true
}

/// 0x358a4: skid dust as long as the speed exceeds 1.5 pixels per frame.
fn skid_dust(g: &mut Ctx, p: Player) {
    let speed = (p.u16(g, Obj::VX) as i16).wrapping_abs() as u16;
    if dust_turn(g, p) && speed > 0x180 {
        dust(g, p, 0, 0x101, (-16i16) as u16, 6);
    }
}

/// 0x35792: stopping skid; a turn remains possible as long as `+0x69` lasts.
fn skid_stop(g: &mut Ctx, p: Player) {
    move_x(g, p);
    skid_dust(g, p);
    if animation::step(g, p) {
        p.set_u32(g, Obj::VX, 0);
        to_stand(g, p);
        return;
    }
    let left = p.u8(g, RUN_LEFT);
    if left == 0 {
        return;
    }
    p.set_u8(g, RUN_LEFT, left - 1);
    let keys = p.u8(g, Player::KEYS_HELD);
    if keys != 0 && facing_for(keys) != p.u8(g, Player::FACING) {
        start_skid(g, p, 6, 56, 27);
    }
}

/// 0x357f6: turning skid; at the end, the player has turned around.
fn skid_turn(g: &mut Ctx, p: Player) {
    move_x(g, p);
    skid_dust(g, p);
    if animation::step(g, p) {
        p.flip(g);
        to_stand(g, p);
    }
}

// ---- carry (action 0x14, 0x35c26) -------------------------------------------------------------

/// Carried object (`+0x86`): its state (`+5`, `+6`).
const HELD: u32 = Player::LINK;

/// 0x35c26: pick up (`+0x8b` = 0) or drop (2) the carried object.
pub fn carry(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::CARRY) {
        0 => {}
        2 => {
            if animation::step(g, p) {
                to_stand(g, p);
            }
            return;
        }
        4 => return throw(g, p),
        v => unreachable!("0x35c26 : value {v:#x} outside the original jump table"),
    }
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, Player::SLOW_8A, 1);
            let held = p.u32(g, HELD);
            let anim = if rd8(g.b, held + 0x6b) != 0 {
                next_step(g, p);
                45
            } else {
                43
            };
            animation::start(g, p, anim);
        }
        2 => {
            if animation::step(g, p) {
                to_stand(g, p);
            }
        }
        4 => {
            if p.u8(g, field::GESTURE) != 0 {
                p.set_u8(g, field::GESTURE, 0);
                p.set_u8(g, Player::SLOW_8A, 0);
                let held = p.u32(g, HELD);
                wr8(g.b, held + 5, 8);
                wr16(g.b, held + 6, 0);
                put_down(g, p, held);
            }
            if animation::step(g, p) {
                to_stand(g, p);
            }
        }
        v => unreachable!("0x35c38 : value {v:#x} outside the original jump table"),
    }
}

/// 0x35d08: (`+0x8b` = 4) throws the carried object. Step 0: animation 41 (42 in slow walk). Step 2: at the gesture
/// (`field::GESTURE`) the object returns to state 2, then the animation ends on standing.
fn throw(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            let anim = if p.u8(g, Player::SLOW) != 0 { 42 } else { 41 };
            animation::start(g, p, anim);
        }
        2 => {
            if p.u8(g, field::GESTURE) != 0 {
                p.set_u8(g, field::GESTURE, 0);
                let held = p.u32(g, HELD);
                wr8(g.b, held + 5, 2);
                wr16(g.b, held + 6, 0);
            }
            if animation::step(g, p) {
                to_stand(g, p);
            }
        }
        _ => {}
    }
}

/// 0x35ca8: puts the object in front of the player (offsets per class, table 0x35ce2).
fn put_down(g: &mut Ctx, p: Player, held: u32) {
    let at = 4 * p.class(g) as u32;
    let t = &player_data().put_down_offsets;
    let (mut dx, dz) = (t.word_at(g.b, at), t.word_at(g.b, at + 2));
    if p.faces_left(g) {
        dx = dx.wrapping_neg();
    }
    let (x, z, y) = (p.u16(g, Obj::X), p.u16(g, Obj::Z), p.u16(g, Obj::Y));
    wr16(g.b, held + Obj::X, x.wrapping_add(dx));
    wr16(g.b, held + Obj::Z, z.wrapping_add(dz));
    wr16(g.b, held + Obj::Y, y.wrapping_sub(8));
}

// ---- pauses of 0x1a frames (actions 0x1c and 0x1e, 0x3626c and 0x362d0) -------------------------------

/// Frame counter of the pause.
const PAUSE_TIMER: u32 = 0x1e8;

/// Actions 0x1c (0x3626c, `last` = 34) and 0x1e (0x362d0, `last` = 33): animation 0, pause of 0x1a frames
/// then animation `last` while the game is stopped (`A5+0x3d`); the player then becomes active again,
/// invulnerable for 16 frames, and standing. During the stop, only action 0x1c advances its animation: 0x362d0
/// hands back control doing nothing (`rts` at 0x3632a; bot `longue_h`, frame 19,500).
pub fn pause(g: &mut Ctx, p: Player, last: u16) {
    match p.u8(g, Player::STEP) {
        // the table of 0x3626c repeats steps 0 and 4 at steps 6 and 8 (that of 0x362d0 stops at step 4)
        0 | 6 => {
            next_step(g, p);
            p.set_u8(g, PAUSE_TIMER, 0x1a);
            animation::start(g, p, 0);
        }
        2 => {
            let left = p.u8(g, PAUSE_TIMER).wrapping_sub(1);
            p.set_u8(g, PAUSE_TIMER, left);
            if left != 0 {
                animation::step(g, p);
            } else {
                next_step(g, p);
                animation::start(g, p, last);
            }
        }
        4 | 8 => {
            if ram::PAUSE.get(g.b) != 0 {
                if last == 34 {
                    animation::step(g, p);
                }
                return;
            }
            p.set_u8(g, Player::ACTIVE, 1);
            p.set_u8(g, Player::INVULN, 0x10);
            p.set_u8(g, Player::SLOW, 0);
            let flags = p.u8(g, Player::FLAGS) & !2;
            p.set_u8(g, Player::FLAGS, flags);
            to_stand(g, p);
        }
        _ => {}
    }
}

