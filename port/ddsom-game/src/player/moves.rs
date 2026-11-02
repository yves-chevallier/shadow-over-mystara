//! Secondary actions of the normal game (`+4` = 2, `+5` = 0): run, carry, crouch, push, attack
//! jump specific to each family of characters, and the actions launched by the special moves
//! (`combos`).
//!
//! Like the common actions (`actions`), each is a small automaton whose step is `+7`. The
//! original routines still called (sounds, effects) go through `Ctx::rom`.

use super::actions::to_stand;
use super::checks::{attack_button, item_curse, jump_button};
use super::spawn::Thrown;
use super::states::State;
use super::walk::{clamp_x, crawl_speed, move_x, move_xy, move_xyz, move_xz, move_z, run_speed, walk_speed};
use super::{animation, ram, Ctx, Player};
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8};
use crate::objects::{pools, Obj};
use crate::machine::Machine;
use crate::player::external as ext;

/// Original routines still called as they are.
mod rom {
    /// Push sound.
    pub const PUSH_SOUND: u32 = 0xeae14;
    /// Rare pose 0x26: sound, flash, creation of a SET object (D0.b = kind; A1 and carry on return).
    pub const CREATE_SET: u32 = 0xc8312;
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

/// One frame out of four (depending on the character), dust under the feet: variant D0, settings D1, speeds D2/D3.
fn dust(g: &mut Ctx, p: Player, d0: u16, d1: u16, d2: u16, d3: u16) {
    let set = |r: &mut u32, v: u16| *r = *r & 0xffff_0000 | v as u32;
    set(&mut g.c.d[0], d0);
    set(&mut g.c.d[1], d1);
    set(&mut g.c.d[2], d2);
    set(&mut g.c.d[3], d3);
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
    match p.u8(g, 0x8b) {
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
    let at = 0x35ce2 + 4 * p.class(g) as u32;
    let (mut dx, dz) = (g.prog16(at), g.prog16(at + 2));
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
            p.set_u8(g, 0x7e, 0x10);
            p.set_u8(g, Player::SLOW, 0);
            let flags = p.u8(g, 1) & !2;
            p.set_u8(g, 1, flags);
            to_stand(g, p);
        }
        _ => {}
    }
}

// ---- climb (action 0x1a, 0x35e56) (h) ---------------------------------------------------------

/// Climbing fields.
mod climb_field {
    /// Step phase (0: to prepare).
    pub const PHASE: u32 = 0x1e0;
    /// Frames before the next oscillation step (modulo 4).
    pub const SWAY_TIMER: u32 = 0x1e1;
    /// Orientation at the start of the lateral movement.
    pub const SIDE: u32 = 0x1e2;
    /// Rank in the oscillation table (`+0x4c`), modulo 32.
    pub const SWAY: u32 = 0x2d;
    pub const SPEED_TABLE: u32 = 0x4c;
    /// Object in hand (`+0x140`) and number of uses (`+0x15d`) of the elf's special move.
    pub const HELD_ITEM: u32 = 0x140;
}

/// Attack animation from the climb, per character (second table).
const CLIMB_ATTACKS: u32 = 0x35eee;

/// Action 0x1a (0x35e56): climb (h). The player rises in Z through an oscillation table, moves
/// according to the directions (up: climbs, down: descends to the ground, sides: walk or run), and can
/// attack (button 1) or, as an elf with a wand, cast a spell (button 4).
pub fn climb(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => climb_start(g, p),
        2 => climb_hold(g, p),
        4 => climb_up(g, p),
        6 => climb_down(g, p),
        n @ (8 | 0xa | 0xc | 0xe) => climb_side(g, p, n),
        0x10 => climb_attack(g, p),
        0x12 => climb_spell(g, p),
        v => unreachable!("0x35e56 : value {v:#x} outside the original jump table"),
    }
    if matches!(p.u8(g, Player::STEP), 0 | 0x10 | 0x12) {
        return;
    }
    let pressed = p.u8(g, Player::KEYS_NEW);
    if pressed & 0x10 != 0 {
        p.set_u8(g, Player::STEP, 0x10);
        let character = p.u8(g, Player::CHARACTER) as u32;
        let anim = g.prog8(CLIMB_ATTACKS + character);
        animation::start_plain(g, p, anim as u16);
    } else if pressed & 0x80 != 0
        && p.class(g) == 2
        && p.u8(g, Player::SPECIAL_COUNT) == 2
        && (0x24..=0x26).contains(&p.u8(g, climb_field::HELD_ITEM))
    {
        p.set_u8(g, Player::STEP, 0x12);
        p.set_u8(g, Player::TIMER_6F, 0x1e);
        animation::start_plain(g, p, 0x22);
    }
}

/// Return to hold (step 2), phase to prepare.
fn climb_to_hold(g: &mut Ctx, p: Player) {
    p.set_u8(g, Player::STEP, 2);
    p.set_u8(g, climb_field::PHASE, 0);
}

/// Step 0: start (climb speed, oscillation table 1), then climb up to a height of 0x18.
fn climb_start(g: &mut Ctx, p: Player) {
    if p.u8(g, climb_field::PHASE) == 0 {
        p.set_u8(g, climb_field::PHASE, 2);
        p.set_u32(g, Obj::VZ, 0x0100_0000);
        p.set_u8(g, climb_field::SWAY_TIMER, 4);
        p.set_u8(g, climb_field::SWAY, 0);
        crate::objects::motion::select_speed(g.b, p.obj(), &crate::objects::motion::SPEEDS_B, 1);
        return animation::start(g, p, 0x3c);
    }
    move_z(g, p);
    if p.u16(g, Obj::Z) > 0x18 {
        next_step(g, p);
        p.set_u8(g, climb_field::PHASE, 0);
    }
    animation::step(g, p);
}

/// Step 2: hold: the player oscillates in height (table `+0x4c`); on the ground, they resume the pose.
fn climb_hold(g: &mut Ctx, p: Player) {
    if p.u8(g, climb_field::PHASE) == 0 {
        p.set_u8(g, climb_field::PHASE, 2);
        return animation::start(g, p, 0x3b);
    }
    let sway = p.u8(g, climb_field::SWAY) as u32;
    let table = p.u32(g, climb_field::SPEED_TABLE);
    let vz = g.data16(table + 4 * sway + 2);
    p.set_u16(g, Obj::VZ, vz);
    let z = (p.u32(g, Obj::Z) as i32).wrapping_add(((vz as u32) << 16) as i32 >> 8);
    p.set_u32(g, Obj::Z, z as u32);
    if z < 0 {
        p.set_u32(g, Obj::Z, 0);
        to_stand(g, p);
        return;
    }
    let timer = p.u8(g, climb_field::SWAY_TIMER).wrapping_sub(1) & 3;
    p.set_u8(g, climb_field::SWAY_TIMER, timer);
    if timer == 0 {
        p.set_u8(g, climb_field::SWAY_TIMER, 4);
        let sway = (p.u8(g, climb_field::SWAY) + 1) & 0x1f;
        p.set_u8(g, climb_field::SWAY, sway);
    }
    climb_directions(g, p);
}

/// `0x35f92`: choice of the step according to the held directions (frozen game: descend).
fn climb_directions(g: &mut Ctx, p: Player) {
    if ram::FROZEN.get(g.b) != 0 {
        p.set_u8(g, Player::STEP, 6);
        return animation::start(g, p, 0x3d);
    }
    let dir = p.u8(g, Player::KEYS_HELD) & 0xf;
    if dir == 0 {
        animation::step(g, p);
        return;
    }
    p.set_u8(g, climb_field::PHASE, 0);
    if dir & 3 == 0 {
        // vertical: bit 3 climbs, otherwise descends
        let (step, anim) = if dir & 8 != 0 { (4, 0x3c) } else { (6, 0x3d) };
        p.set_u8(g, Player::STEP, step);
        return animation::start(g, p, anim);
    }
    let facing = if dir & 1 != 0 { 0x20 } else { 0 };
    let (step, anim) = if facing == p.u8(g, Player::FACING) { (8, 0x3e) } else { (0xa, 0x3f) };
    p.set_u8(g, Player::STEP, step);
    let f = p.u8(g, Player::FACING);
    p.set_u8(g, climb_field::SIDE, f);
    animation::start(g, p, anim);
}

/// Step 4: climb as long as up is held (up to 0x80 above the top of the camera).
fn climb_up(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::KEYS_HELD) & 0xf {
        0 => {
            climb_to_hold(g, p);
            animation::step(g, p);
        }
        8 => {
            let cam = if ram::USE_ALT_CAMERA.get(g.b) != 0 { ram::CAMERA_ALT_Y.get(g.b) } else { ram::CAMERA_Y.get(g.b) };
            let top = cam.wrapping_add(0x80);
            let height = p.u16(g, Obj::Y).wrapping_add(p.u16(g, Obj::Z));
            if height as i16 <= top as i16 {
                let z = p.u16(g, Obj::Z).wrapping_add(2);
                p.set_u16(g, Obj::Z, z);
            }
            animation::step(g, p);
        }
        _ => climb_directions(g, p),
    }
}

/// Step 6: descend as long as down is held (or game frozen); on the ground, the pose.
fn climb_down(g: &mut Ctx, p: Player) {
    if ram::FROZEN.get(g.b) == 0 {
        match p.u8(g, Player::KEYS_HELD) & 0xf {
            0 => {
                climb_to_hold(g, p);
                animation::step(g, p);
                return;
            }
            4 => {}
            _ => return climb_directions(g, p),
        }
    }
    let z = p.u16(g, Obj::Z).wrapping_sub(2);
    p.set_u16(g, Obj::Z, z);
    if (z as i16) < 0 {
        p.set_u32(g, Obj::Z, 0);
        to_stand(g, p);
        return;
    }
    animation::step(g, p);
}

/// Steps 8 and 0xa (walk), 0xc and 0xe (run, limited by `+0x69`): lateral movement as long as the
/// starting direction (8, 0xc) or the opposite one (0xa, 0xe) is held; otherwise change of direction.
fn climb_side(g: &mut Ctx, p: Player, step: u8) {
    let run = step >= 0xc;
    if run {
        let left = p.u8(g, Player::PARAM).wrapping_sub(1);
        p.set_u8(g, Player::PARAM, left);
        if left == 0 {
            climb_to_hold(g, p);
            animation::step(g, p);
            return;
        }
    }
    let dir = p.u8(g, Player::KEYS_HELD) & 0xf;
    if dir == 0 {
        climb_to_hold(g, p);
        animation::step(g, p);
        return;
    }
    if dir & 3 == 0 {
        return climb_directions(g, p);
    }
    let first = step & 2 == 0;
    let side = p.u8(g, climb_field::SIDE) != 0;
    let bit = if side == first { 0 } else { 1 };
    if dir & 3 & (1 << bit) == 0 {
        let (next, anim) = if first { (0xa, 0x3f) } else { (8, 0x3e) };
        p.set_u8(g, Player::STEP, next);
        return animation::start(g, p, anim);
    }
    if run {
        run_speed(g, p);
    } else {
        walk_speed(g, p);
    }
    move_xy(g, p);
    animation::step(g, p);
}

/// Step 0x10: attack from the climb; the animation event launches its effect.
fn climb_attack(g: &mut Ctx, p: Player) {
    if animation::step(g, p) {
        climb_to_hold(g, p);
        animation::step(g, p);
        return;
    }
    let event = p.u8(g, field::ANIM_EVENT);
    if event != 0 {
        p.set_u8(g, field::ANIM_EVENT, 0);
        super::spawn::weapon_trail(g, p, event);
    }
}

/// Step 0x12: elf's wand spell (`+0x140` = 0x24 to 0x26), cast at the gesture.
fn climb_spell(g: &mut Ctx, p: Player) {
    animation::step(g, p);
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if (left as i8) < 0 {
        climb_to_hold(g, p);
    }
    if p.u8(g, field::GESTURE) == 0 {
        return;
    }
    p.set_u8(g, field::GESTURE, 0);
    let shot = match p.u8(g, climb_field::HELD_ITEM) {
        0x24 => Thrown::C,
        0x25 => Thrown::D,
        0x26 => Thrown::E,
        _ => return,
    };
    super::spawn::thrown(g, p, shot);
}

// ---- exit (action 0x20, 0x3632c) (h) ----------------------------------------------------------

/// Exit fields.
mod exit_field {
    /// Frames remaining before the player is hidden.
    pub const TIMER: u32 = 0x1e8;
    /// Fade step of their palette (0 to 3) and frames before the next step.
    pub const FADE: u32 = 0x1e9;
    pub const FADE_TIMER: u32 = 0x1ea;
    /// Object toward which the player jumps (word, `+0x86`).
    pub const TARGET: u32 = 0x86;
}

/// Fade palettes: per step (0x180 bytes), per character (32 bytes); destination per character.
pub(super) const EXIT_PALETTES: u32 = 0x3663e;
pub(super) const EXIT_PALETTE_SLOTS: u32 = 0x36626;

/// Action 0x20 (0x3632c): the player leaves the scene (h). The starting step selects the exit: walk
/// down, up, left, right or in place (0 to 8), sinking (0xa) or jump toward the object
/// `+0x86` (0xc); the player is hidden at the end of the delay, while their palette fades out in three steps.
pub fn exit_scene(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => exit_walk(g, p, 0x1e, 0, 0x0001_0000, None, 0x25),
        2 => exit_walk(g, p, 0x1e, 0, 0xffff_0000, None, 0x26),
        4 => exit_walk(g, p, 0x28, 0xfe00_0000, 0, Some(0), 2),
        6 => exit_walk(g, p, 0x28, 0x0200_0000, 0, Some(0x20), 2),
        8 => {
            exit_start(g, p, 0xe, 0x1e);
            p.set_u8(g, Player::FACING, 0x20);
            p.set_u32(g, Obj::VX, 0);
            p.set_u32(g, Obj::VY, 0);
            animation::start(g, p, 2);
        }
        0xa => {
            p.set_u8(g, Player::STEP, 0x12);
            p.set_u8(g, Obj::ACTIVE, 2);
            p.set_u8(g, exit_field::TIMER, 0x30);
            animation::start(g, p, 0x38);
            p.set_u8(g, 0x3b, 0);
        }
        0xc => {
            exit_start(g, p, 0x14, 0x1e);
            p.set_u16(g, Obj::VX, 0xff00);
            p.set_u16(g, Obj::AX, 0);
            p.set_u16(g, Obj::VZ, 0x500);
            p.set_u16(g, Obj::AZ, 0xffc8);
            let target = p.u16(g, exit_field::TARGET) as i16 as i32 as u32;
            p.set_u8(g, Player::FACING, 0);
            if (p.u16(g, Obj::X).wrapping_sub(rd16(g.b, target + Obj::X)) as i16) < 0 {
                p.set_u8(g, Player::FACING, 0x20);
                let v = p.u16(g, Obj::VX).wrapping_neg();
                p.set_u16(g, Obj::VX, v);
            }
            crate::objects::anim::start_by_character(g.b, p.obj(), &crate::objects::anim::SETS_BY_VARIANT, 2);
        }
        0xe => {
            move_xy(g, p);
            animation::step(g, p);
            if exit_countdown(g, p) {
                next_step(g, p);
                p.set_u8(g, 0x3b, 0);
            }
            exit_fade(g, p);
        }
        0x10 => {}
        0x12 => {
            let z = p.u16(g, Obj::Z).wrapping_sub(4);
            p.set_u16(g, Obj::Z, z);
            animation::step(g, p);
            if exit_countdown(g, p) {
                p.set_u8(g, Player::STEP, 0x10);
            }
            p.set_u8(g, 0x3b, 0);
        }
        0x14 => {
            move_z(g, p);
            let target = p.u16(g, exit_field::TARGET) as i16 as i32 as u32;
            if p.u16(g, Obj::X) != rd16(g.b, target + Obj::X) {
                move_x(g, p);
            }
            animation::step(g, p);
            if exit_countdown(g, p) {
                p.set_u8(g, Player::STEP, 0x10);
            }
            p.set_u8(g, 0x3b, 0);
            exit_fade(g, p);
        }
        v => unreachable!("0x3632c : value {v:#x} outside the original jump table"),
    }
}

/// Start of an exit: next step `step`, player displayed without shadow, delay, fade ready.
fn exit_start(g: &mut Ctx, p: Player, step: u8, timer: u8) {
    p.set_u8(g, Player::STEP, step);
    p.set_u8(g, Obj::ACTIVE, 2);
    p.set_u8(g, exit_field::TIMER, timer);
    p.set_u8(g, exit_field::FADE, 0);
    p.set_u8(g, exit_field::FADE_TIMER, 1);
}

/// Exit by walking (X and Y speeds as longs, optional orientation); in slow-motion mode, animation 0x35.
fn exit_walk(g: &mut Ctx, p: Player, timer: u8, vx: u32, vy: u32, facing: Option<u8>, anim: u16) {
    exit_start(g, p, 0xe, timer);
    if let Some(f) = facing {
        p.set_u8(g, Player::FACING, f);
    }
    p.set_u32(g, Obj::VX, vx);
    p.set_u32(g, Obj::VY, vy);
    let anim = if ram::SLOW_MODE.get(g.b) != 0 { 0x35 } else { anim };
    animation::start(g, p, anim);
}

/// The delay runs out; at zero, the player is hidden (true).
fn exit_countdown(g: &mut Ctx, p: Player) -> bool {
    let left = p.u8(g, exit_field::TIMER).wrapping_sub(1);
    p.set_u8(g, exit_field::TIMER, left);
    if left != 0 {
        return false;
    }
    let flags = p.u8(g, Player::FLAGS) | 2;
    p.set_u8(g, Player::FLAGS, flags);
    true
}

/// One fade step every 12 frames (three steps): the character's palette copied from the table.
fn exit_fade(g: &mut Ctx, p: Player) {
    let stage = p.u8(g, exit_field::FADE);
    if stage > 2 {
        return;
    }
    let left = p.u8(g, exit_field::FADE_TIMER).wrapping_sub(1);
    p.set_u8(g, exit_field::FADE_TIMER, left);
    if left != 0 {
        return;
    }
    let character = p.u8(g, Player::CHARACTER) as u32;
    let from = EXIT_PALETTES + (0x180 * stage as u32 + (character << 5)) as u16 as i16 as i32 as u32;
    let slot = g.data16(EXIT_PALETTE_SLOTS + 2 * character) as i16 as i32 as u32;
    let to = 0x91c000u32.wrapping_add(slot);
    for i in 0..8 {
        let v = rd32(g.b, from + 4 * i);
        wr32(g.b, to + 4 * i, v);
    }
    p.set_u8(g, exit_field::FADE_TIMER, 0xc);
    p.set_u8(g, exit_field::FADE, stage + 1);
}

// ---- guard (actions 0x16 and 0x18, 0x35d52 and 0x35dca) ---------------------------------------------

/// Shield guard as long as the button `held` (bit 7: special, bit 4: attack) is held: the
/// player slowly backs away (directions opposite to their original direction, or vertical) without
/// turning; when released, they resume the pose (or attack, or jump).
pub fn guard(g: &mut Ctx, p: Player, held: u8) {
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        p.set_u8(g, Player::SUBACTION, 0);
        let facing = p.u8(g, Player::FACING) ^ 0x20;
        p.set_u8(g, field::HELD_FACING, facing);
        animation::start(g, p, 25);
        return;
    }
    let keys = p.u8(g, Player::KEYS_HELD);
    if keys & held == 0 {
        if !attack_button(g, p) && !jump_button(g, p) {
            to_stand(g, p);
        }
        return;
    }
    let dir = keys & 0xf;
    let moving = dir != 0 && (dir & 3 == 0 || facing_for(dir) == p.u8(g, field::HELD_FACING));
    if !moving {
        animation::start(g, p, 25);
        return;
    }
    crawl_speed(g, p);
    move_xy(g, p);
    animation::step(g, p);
}

// ---- action 0x28 (0x36be0): end of slowdown (h) --------------------------------------------

/// Countdown of action 0x28 (`+0x1e8`).
const SLOW_END_TIMER: u32 = 0x1e8;

/// 0x36be0: the slowed player stays 64 frames in pose 0x39, then the slowdown (`+0x66`) ends and they
/// stand back up.
pub fn slow_end(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, SLOW_END_TIMER, 0x40);
            animation::start(g, p, 0x39);
        }
        2 => {
            let left = p.u8(g, SLOW_END_TIMER).wrapping_sub(1);
            p.set_u8(g, SLOW_END_TIMER, left);
            if left == 0 {
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
            }
        }
        v => unreachable!("0x36be0 : value {v:#x} outside the original jump table"),
    }
}

// ---- actions 0x26, 0x2c and 0x30: rare poses ----------------------------------------------------

/// Countdown of the rare poses (`+0x1e8`, byte) and number of the next puff.
const RARE_TIMER: u32 = 0x1e8;

/// 0x36b88: action 0x26, the cursed weapon falls (`checks::item_curse`, `common::curse_check`, `combat`): pose 0x23
/// and sound 0xe24d8, then 192 frames of animation; at each gesture (`+0x33`), flash (0xb97ba) and SET object 0x29
/// (0xc8312) marked at `+0x42`. At the end, the slowdown (`+0x66`) ends and the player stands back up.
pub fn curse_drop(g: &mut Ctx, p: Player) {
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        p.set_u8(g, RARE_TIMER, 0xc0);
        animation::start(g, p, 0x23);
        g.sound(ext::sound::FANFARE, p);
        return;
    }
    animation::step(g, p);
    let left = p.u8(g, RARE_TIMER).wrapping_sub(1);
    p.set_u8(g, RARE_TIMER, left);
    if left == 0 {
        p.set_u8(g, Player::SLOW, 0);
        to_stand(g, p);
        return;
    }
    if p.u8(g, field::GESTURE) == 0 {
        return;
    }
    p.set_u8(g, field::GESTURE, 0);
    crate::families::effects_creators::for_players::posed_flash_b_68k(g.c, g.b, p.obj());
    crate::mem::set_low_byte(&mut g.c.d[0], 0x29);
    g.rom(rom::CREATE_SET, p);
    if g.c.sr & m68k::SR_C == 0 {
        wr8(g.b, g.c.a[1] + 0x42, 1);
    }
}

/// Dust puffs of action 0x2c (0x36d2c, rows of 4 words: D0, D1, D2 of 0xb10c8, height).
const PUFFS: u32 = 0x36d2c;

/// 0x36cb8: action 0x2c (an effect gives it to the player, `families/effects_f.rs`): pose 0x35, six puffs of
/// dust (0xb10c8) spaced 6 frames apart, then 6 frames of waiting before standing back up.
pub fn dust_puffs(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, RARE_TIMER, 0);
            p.set_u8(g, Player::TIMER_6F, 1);
            animation::start(g, p, 0x35);
        }
        2 => {
            let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
            p.set_u8(g, Player::TIMER_6F, left);
            if left != 0 {
                return;
            }
            let row = PUFFS + 8 * p.u8(g, RARE_TIMER) as u32;
            let height = g.prog16(row + 6);
            for (r, off) in [(0, 0), (1, 2), (2, 4)] {
                let v = g.prog16(row + off);
                crate::mem::set_low_word(&mut g.c.d[r], v);
            }
            g.c.d[4] = height as u32;
            crate::mem::set_low_word(&mut g.c.d[3], 0xffff);
            let (variant, setting, vx) = (g.c.d[0] as u8, g.c.d[1] as u16, g.c.d[2] as u16);
            g.shock(p, variant, setting, vx, 0xffff);
            if g.c.sr & m68k::SR_C == 0 {
                wr16(g.b, g.c.a[1] + Obj::Z, height);
            }
            p.set_u8(g, Player::TIMER_6F, 6);
            let n = p.u8(g, RARE_TIMER).wrapping_add(1);
            p.set_u8(g, RARE_TIMER, n);
            if n == 6 {
                next_step(g, p);
                p.set_u8(g, Player::TIMER_6F, 6);
            }
        }
        4 => {
            let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
            p.set_u8(g, Player::TIMER_6F, left);
            if left == 0 {
                to_stand(g, p);
            }
        }
        v => unreachable!("0x36cb8 : value {v:#x} outside the original jump table"),
    }
}

/// 0x36f1a: action 0x30 (the player hit a crate, `families/boxes.rs`): pose 0x36 until its end, then the
/// slowdown (`+0x66`) ends and the player stands back up.
pub fn box_recoil(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            animation::start(g, p, 0x36);
        }
        2 => {
            if animation::step(g, p) {
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
            }
        }
        v => unreachable!("0x36f1a : value {v:#x} outside the original jump table"),
    }
}

/// Natives of the rare poses, for the differential validation (`fuzz_player player_action`): `actions::act` calls them
/// directly.
macro_rules! rare_native {
    ($name:ident, $entry:expr, $label:expr, $f:path) => {
        pub const $name: crate::Native = crate::Native {
            entry: $entry,
            name: $label,
            run: crate::Body::WithCpu(|c, b| {
                let p = Player(c.a[0]);
                $f(&mut Ctx::new(c, b), p);
                crate::mem::rts(c, b)
            }),
            touches_sound: false,
            outputs: 0,
        };
    };
}
rare_native!(CURSE_DROP, 0x36b88, "player_action_26", curse_drop);
rare_native!(DUST_PUFFS, 0x36cb8, "player_action_2c", dust_puffs);
rare_native!(BOX_RECOIL, 0x36f1a, "player_action_30", box_recoil);

// ---- action 0x2a (0x36c12): jump in place with bounce (h) ---------------------------------------

/// 0x36c12: takeoff, landing then walk on the height; the height oscillates by 2 pixels.
pub fn hop(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, Player::TIMER_6F, 0);
            p.set_u32(g, Obj::VX, 0);
            p.set_u16(g, Obj::VZ, (-0x900i16) as u16);
            p.set_u16(g, Obj::AZ, 40);
            animation::start(g, p, 56);
        }
        2 => {
            move_z(g, p);
            if p.u16(g, Obj::Z) as i16 <= 0 {
                next_step(g, p);
                p.set_u32(g, Obj::Z, 0);
                p.set_u32(g, Obj::VZ, 0);
            }
            animation::step(g, p);
        }
        4 => {
            walk_speed(g, p);
            move_x(g, p);
            let y = p.u16(g, Obj::Y).wrapping_sub(4);
            p.set_u16(g, Obj::Y, y);
            animation::step(g, p);
            p.set_u8(g, 0x3b, 0);
            clamp_x(g.b, p);
        }
        v => unreachable!("0x36c12 : value {v:#x} outside the original jump table"),
    }
    p.set_u8(g, 0x3b, 0);
    let z = p.u16(g, Obj::Z).wrapping_add(2);
    p.set_u16(g, Obj::Z, z);
    let phase = p.u8(g, Player::TIMER_6F).wrapping_add(1) & 1;
    p.set_u8(g, Player::TIMER_6F, phase);
    if phase == 0 {
        let z = p.u16(g, Obj::Z).wrapping_sub(4);
        p.set_u16(g, Obj::Z, z);
    }
}

// ---- action 0x24 (0x36b26): fall to the ground (h) -----------------------------------------------

/// 0x36b26: the player falls (animation 13), the fall follows the speed in X, Y and Z until the height
/// is no longer positive; they then touch the ground (animation 14, landing sound) then, the animation over,
/// stand back up. Step 2 (animation 18) is never reached: step 0 advances by two notches.
pub fn fall_in(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            next_step(g, p);
            animation::start(g, p, 13);
        }
        2 => {
            next_step(g, p);
            animation::start(g, p, 18);
        }
        4 => {
            move_xyz(g, p);
            if p.u16(g, Obj::Z) as i16 > 0 {
                return;
            }
            next_step(g, p);
            p.set_u32(g, Obj::Z, 0);
            animation::start(g, p, 14);
            g.sound(ext::sound::LANDING, p);
        }
        6 => {
            if animation::step(g, p) {
                to_stand(g, p);
            }
        }
        v => unreachable!("0x36b26 : value {v:#x} outside the original jump table"),
    }
}

// ---- push (action 0x2e, 0x36d68) ------------------------------------------------------------

/// 0x36d68: pushes the linked object (`+0x86`) in one of the four directions (steps 2 to 8: up, down,
/// left, right) as long as the direction is held.
pub fn push(g: &mut Ctx, p: Player) {
    let step = p.u8(g, Player::STEP);
    let key = match step {
        0 => {
            let first = p.u8(g, 0x16f);
            p.set_u8(g, Player::STEP, first);
            p.set_u8(g, Player::TIMER_6F, 32);
            animation::start(g, p, 58);
            p.set_u8(g, field::GESTURE, 0);
            return;
        }
        2 => 8,
        4 => 4,
        6 => 2,
        8 => 1,
        v => unreachable!("0x36d68 : value {v:#x} outside the original jump table"),
    };
    if p.u8(g, Player::KEYS_HELD) & key == 0 {
        let held = p.u32(g, HELD);
        wr8(g.b, held + 0x66, 0);
        wr16(g.b, held + 0x6c, 0);
        to_stand(g, p);
        return;
    }
    let wait = p.u8(g, Player::TIMER_6F);
    if wait != 0 {
        p.set_u8(g, Player::TIMER_6F, wait - 1);
        if wait - 1 != 0 {
            return;
        }
        p.set_u8(g, field::GESTURE, 1);
    }
    animation::step(g, p);
    if step >= 6 {
        push_dust(g, p);
    }
    if p.u8(g, field::ANIM_EVENT) != 0 {
        p.set_u8(g, field::ANIM_EVENT, 0);
        g.sound(rom::PUSH_SOUND, p);
    }
}

/// 0x36eb0: dust of the pushing gesture (settings per class, table 0x36eea).
fn push_dust(g: &mut Ctx, p: Player) {
    if !dust_turn(g, p) || p.u8(g, field::GESTURE) == 0 {
        return;
    }
    let at = 0x36eea + 8 * p.class(g) as u32;
    let (d0, d1, d2, d3) = (g.prog16(at), g.prog16(at + 2), g.prog16(at + 4), g.prog16(at + 6));
    g.c.d[4] = g.c.d[4] & 0xffff_0000 | 8 * p.class(g) as u32;
    dust(g, p, d0, d1, d2, d3);
}

// ---- action 0x32 (0x36f48) ---------------------------------------------------------------------

/// 0x36f48: animation 55 for 24 frames, then end of the slow walk.
pub fn spell_fizzle(g: &mut Ctx, p: Player) {
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        p.set_u8(g, Player::TIMER_6F, 24);
        animation::start(g, p, 55);
        return;
    }
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left == 0 {
        p.set_u8(g, Player::SLOW, 0);
        to_stand(g, p);
    } else {
        animation::step(g, p);
    }
}

// ---- action 0x12 of the characters: leaps ---------------------------------------------------------

/// 0x35a78 (fighter, cleric, elf): forward leap, with a possible aerial attack.
pub fn leap_attack(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::AIR_ATTACK, 0);
            p.set_u8(g, field::AIRBORNE, 0);
            p.set_u16(g, Obj::VX, 0x200);
            p.set_u16(g, Obj::AX, 0);
            p.set_u16(g, Obj::VZ, 0x380);
            p.set_u16(g, Obj::AZ, (-56i16) as u16);
            if p.faces_left(g) {
                p.set_u16(g, Obj::VX, (-0x200i16) as u16);
            }
            animation::start(g, p, 31);
            g.sound(ext::sound::JUMP_VOICE, p);
        }
        2 => {
            animation::step(g, p);
            move_xz(g, p);
            air_attack(g, p);
            if (p.u16(g, Obj::Z) as i16) < 0 {
                next_step(g, p);
                p.set_u32(g, Obj::Z, 0);
                animation::start(g, p, 14);
                g.sound(ext::sound::LANDING, p);
            }
        }
        4 => {
            if animation::step(g, p) {
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
            }
        }
        v => unreachable!("0x35a78 : value {v:#x} outside the original jump table"),
    }
    let event = p.u8(g, field::ANIM_EVENT);
    if event != 0 {
        p.set_u8(g, field::ANIM_EVENT, 0);
        super::spawn::weapon_trail(g, p, event);
    }
}

/// 0x3769c: during the leap, button 1 gives an aerial attack (animation per character, table
/// 0x376d6, or 0x376e2 with down held).
fn air_attack(g: &mut Ctx, p: Player) {
    if p.u8(g, field::AIR_ATTACK) != 0 || p.u8(g, Player::KEYS_NEW) & 0x10 == 0 {
        return;
    }
    let character = p.u8(g, Player::CHARACTER) as u32;
    let table = if p.u8(g, Player::KEYS_HELD) & 4 != 0 { 0x376e2 } else { 0x376d6 };
    let anim = g.prog8(table + character);
    g.c.d[6] = anim as u32;
    animation::start_plain(g, p, anim as u16);
    item_curse(g, p);
}

/// 0x35b96 (magic-user): small backward jump (the recoil stops in front of a terrain obstacle).
pub fn hop_back(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            animation::start(g, p, 31);
        }
        2 => {
            if !animation::step(g, p) {
                return;
            }
            next_step(g, p);
            p.set_u16(g, Obj::VX, 0x300);
            p.set_u16(g, Obj::AX, (-32i16) as u16);
            let mut shift: u16 = 64;
            if p.faces_left(g) {
                shift = shift.wrapping_neg();
                let pair = p.u32(g, Obj::VX).wrapping_neg();
                p.set_u32(g, Obj::VX, pair);
            }
            let (x, y) = (p.u16(g, Obj::X).wrapping_add(shift), p.u16(g, Obj::Y));
            if crate::video::terrain(g.b, x, y).number & 63 == 0 {
                p.set_u16(g, Obj::X, x);
            }
            super::walk::clamp_to_screen(g.b, p);
            animation::start(g, p, 32);
        }
        4 => {
            move_x(g, p);
            let mut vx = p.u16(g, Obj::VX);
            if p.faces_left(g) {
                vx = vx.wrapping_neg();
            }
            if (vx as i16) < 0 {
                to_stand(g, p);
            } else {
                animation::step(g, p);
            }
        }
        v => unreachable!("0x35b96 : value {v:#x} outside the original jump table"),
    }
}

/// Natives of the characters' actions 0x12 (table `characters::Slot::Action12`): the Rust function receives
/// the player (A0); the callers read no register.
pub const LEAP: crate::Native = crate::Native {
    entry: 0x35a78,
    name: "leap_attack",
    run: crate::Body::WithCpu(|c, b| {
        let p = Player(c.a[0]);
        leap_attack(&mut Ctx::new(c, b), p);
        crate::mem::rts(c, b)
    }),
    touches_sound: false,
    outputs: 0,
};

pub const HOP_BACK: crate::Native = crate::Native {
    entry: 0x35b96,
    name: "magic_user_hop_back",
    run: crate::Body::WithCpu(|c, b| {
        let p = Player(c.a[0]);
        hop_back(&mut Ctx::new(c, b), p);
        crate::mem::rts(c, b)
    }),
    touches_sound: false,
    outputs: 0,
};

// ---- parry (`+5` = 8, 0x39ff0) ----------------------------------------------------------------------

/// Counterattack after a parry: attack sub-action per class (code bytes).
const RIPOSTE: u32 = 0x3a0a2;

/// 0x39ff0: after a parry, the player slides backward (speed 0x400 braked by 0x40 per frame); at
/// rest, they resume the game at the action `+0x6f`. During the slide, the opponent's direction
/// held chains a counterattack (except the magic-user). Then terrain and screen limits.
pub fn parry(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::ACTION) {
        0 => {
            p.set_u8(g, Player::ACTION, 2);
            p.set_u8(g, 0x3e, 0);
            set_vx_pair(g, p, 0x400, (-64i16) as u16);
            parry_slide(g, p);
        }
        2 => parry_slide(g, p),
        v => unreachable!("0x39ff0 : value {v:#x} outside the original jump table"),
    }
    crate::terrain::push(g.b, p.obj());
    super::walk::clamp_to_screen(g.b, p);
}

fn parry_slide(g: &mut Ctx, p: Player) {
    move_x(g, p);
    let mut vx = p.u16(g, Obj::VX);
    if p.faces_left(g) {
        vx = vx.wrapping_neg();
    }
    if (vx as i16) < 0 {
        p.set_u32(g, Obj::VX, 0);
        p.set_u8(g, Player::ACTIVE, 1);
        p.set_state(g, 0x0200_0000);
        let resume = p.u8(g, Player::TIMER_6F);
        p.set_u8(g, Player::ACTION, resume);
        return;
    }
    let class = p.class(g);
    if class == 4 {
        return;
    }
    let toward = if p.faces_left(g) { 0 } else { 1 };
    if p.u8(g, Player::KEYS_HELD) >> toward & 1 != 0 {
        p.set_u8(g, Player::ACTIVE, 1);
        p.set_state(g, 0x0200_0c00);
        p.set_u32(g, Obj::VX, 0);
        let kind = g.prog8(RIPOSTE + class as u32);
        p.set_u8(g, Player::SUBACTION, kind);
    }
}

// ---- special-move actions (called by `combos`) --------------------------------------------

/// Attack (action 0xc) of sub-action `kind` in the stored direction `facing_from` (or the current direction).
fn combo_attack(g: &mut Ctx, p: Player, kind: u8, facing_from: Option<u32>) -> bool {
    p.set_u8(g, Player::HITSTOP, 0);
    p.set_state(g, State::pack(2, 0, 0xc, 0).0);
    p.set_u8(g, Player::SUBACTION, kind);
    if let Some(at) = facing_from {
        let facing = p.u8(g, at);
        p.set_u8(g, Player::FACING, facing);
    }
    item_curse(g, p);
    true
}

/// A special-move action; returns true if it changed the state (Z at 0 in the original).
#[derive(Clone, Copy, Debug)]
pub enum Combo {
    /// 0x37386: run in the direction of the sequence (true only to the left: accidental Z).
    Dash,
    /// 0x373a8: action 0x10, on the ground and outside an attack.
    Action10,
    /// 0x373cc: action 0x12 (or 0x1a with speed class 2).
    Action12,
    /// 0x3733c: attack of sub-action 10.
    Attack10,
    /// 0x37366: attack of sub-action 12.
    Attack12,
    /// 0x373f4: attack of sub-action 22.
    Attack22,
    /// 0x3740e: attack of sub-action 24 (except with 8 HP or less, cleric excepted).
    Attack24,
    /// 0x3743e: attack of sub-action 26 of the dwarf, in the middle of an attack.
    DwarfAttack26,
    /// 0x3746a: attack of sub-action 30 (spell 36 or 44 in row 2).
    Attack30,
    /// 0x374aa: the thief grabs an enemy from behind.
    Backstab,
}

pub fn combo(g: &mut Ctx, p: Player, which: Combo) -> bool {
    let slow = p.u8(g, Player::SLOW) != 0;
    match which {
        Combo::Attack10 => !slow && combo_attack(g, p, 10, Some(0x1b7)),
        Combo::Attack12 => combo_attack(g, p, 12, Some(0x1bd)),
        Combo::Attack22 => combo_attack(g, p, 22, None),
        Combo::Dash => {
            if slow || p.u8(g, Player::ACTION) == 0xc {
                return false;
            }
            p.set_state(g, State::pack(2, 0, 6, 0).0);
            let facing = p.u8(g, 0x1a5);
            p.set_u8(g, Player::FACING, facing);
            facing != 0
        }
        Combo::Action10 => {
            if p.u16(g, Obj::Z) != 0 || p.u8(g, Player::ACTION) == 0xc {
                return false;
            }
            p.set_state(g, State::pack(2, 0, 0x10, 0).0);
            let facing = p.u8(g, 0x1ab);
            p.set_u8(g, Player::FACING, facing);
            true
        }
        Combo::Action12 => {
            if p.u8(g, Player::SPEED_CLASS) == 2 {
                p.set_state(g, State::pack(2, 0, 0x1a, 0).0);
                p.set_u8(g, 0x1e0, 0);
            } else {
                p.set_state(g, State::pack(2, 0, 0x12, 0).0);
            }
            true
        }
        Combo::Attack24 => {
            if p.class(g) != 1 && p.u16(g, Player::HP) <= 8 {
                return false;
            }
            p.set_state(g, State::pack(2, 0, 0xc, 0).0);
            p.set_u8(g, Player::SUBACTION, 24);
            item_curse(g, p);
            true
        }
        Combo::DwarfAttack26 => {
            if p.u8(g, Player::ACTION) != 0xc || p.u8(g, Player::SUBACTION) != 0 {
                return false;
            }
            combo_attack(g, p, 26, None)
        }
        Combo::Attack30 => {
            let spell = if p.class(g) == 2 { 36 } else { 44 };
            if p.u8(g, Player::SPECIAL_COUNT) != 2 || p.u8(g, 0x140) != spell {
                return false;
            }
            p.set_state(g, State::pack(2, 0, 0xc, 0).0);
            p.set_u8(g, Player::SUBACTION, 30);
            true
        }
        Combo::Backstab => backstab(g, p),
    }
}

/// Orientation of an object (0 or 0x20).
const FACING: u32 = 0x2c;

/// Table (code, 0x37588): non-zero for the enemy types that cannot be grabbed.
const UNGRABBABLE: u32 = 0x37588;
/// Body boxes (data): one pointer per box set, 8-byte entries.
const BODY_BOXES: u32 = 0x11bffa;

/// 0x374aa: an enemy on the ground, facing the same direction as the player (hence from behind), grabbable and
/// close enough is grabbed; the player goes to attack of sub-action 26.
fn backstab(g: &mut Ctx, p: Player) -> bool {
    if ram::FROZEN.get(g.b) != 0 {
        return false;
    }
    for enemy in pools::ENEMY.iter() {
        let e = enemy.0;
        let b = &mut *g.b;
        let ok = rd8(b, e) != 0
            && rd8(b, e + 1) & 1 != 0
            && rd8(b, e + 4) == 2
            && rd16(b, e + Obj::Z) == 0
            && rd16(b, e + 0x62) != 0
            && rd8(b, e + 0x86) == 0
            && rd8(b, e + 0x40) == 0
            && rd32(b, e + 0x94) & 1 << 30 == 0
            && rd8(b, p.at(Player::FACING)) == rd8(b, e + FACING);
        if !ok {
            continue;
        }
        let kind = rd8(b, e + 2) as u32;
        if crate::mem::prog8(b, UNGRABBABLE + kind) != 0 {
            continue;
        }
        let box_set = rd8(b, e + 0x2e) as u32;
        let set = rd32(b, BODY_BOXES + 4 * box_set);
        let body = set + 8 * rd8(b, e + 0x38) as u32;
        let facing = rd8(b, p.at(0x1cf));
        wr8(b, p.at(Player::FACING), facing);
        if !within_reach(b, p.0, e, body) {
            continue;
        }
        wr8(b, e, 2);
        wr32(b, e + 4, 0x0400_0000);
        wr8(b, e + 0x49, 100);
        let index = rd8(b, p.at(Player::INDEX));
        wr8(b, e + 0x48, index);
        wr16(b, e + 0x50, p.0 as u16);
        wr32(b, p.at(Player::MAIN), 0x0200_0c00);
        wr8(b, p.at(0x47), 88);
        wr8(b, p.at(Player::SUBACTION), 26);
        wr16(b, p.at(0x50), e as u16);
        return true;
    }
    false
}

/// 0x375a8: is enemy `e` (body box `body`) within grabbing range of player `me`? (in
/// depth ±(16 + half-depth), in x in front of the player ±(48 + the enemy's byte `+1`, like the original)).
fn within_reach(b: &mut cps2::Cps2Bus, me: u32, e: u32, body: u32) -> bool {
    let sbyte = |b: &mut cps2::Cps2Bus, a: u32| rd8(b, a) as i8 as i16 as u16;
    let near = |gap: u16, half: u16| gap.wrapping_add(half) <= half.wrapping_add(half);
    let gap = 4u16.wrapping_add(rd16(b, me + Obj::Y)).wrapping_sub(sbyte(b, body + 4).wrapping_add(rd16(b, e + Obj::Y)));
    if !near(gap, 16u8.wrapping_add(rd8(b, body + 5)) as u16) {
        return false;
    }
    let mut reach = (-48i16) as u16;
    if rd8(b, me + FACING) != 0 {
        reach = reach.wrapping_neg();
    }
    let mut offset = sbyte(b, body);
    if rd8(b, e + FACING) != 0 {
        offset = offset.wrapping_neg();
    }
    let gap = reach.wrapping_add(rd16(b, me + Obj::X)).wrapping_sub(offset.wrapping_add(rd16(b, e + Obj::X)));
    near(gap, 48u8.wrapping_add(rd8(b, e + 1)) as u16)
}

