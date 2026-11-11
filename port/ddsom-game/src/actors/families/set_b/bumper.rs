use super::*;
use crate::actors::families::set::tables as set_tables;
use crate::world::objects::Obj;
use crate::actors::player::Player;
use crate::actors::families::set_b::fields::BumperObj;
use crate::actors::families::set_b::fields::LaunchedPlayer;

const ANIMS: u32 = 0x1c4738;
/// RAM word (A5+0x46): occupied trampoline (address of the object).
const LOCK: u32 = 0x46;
/// RAM byte (A5+0x50): game frozen.
const FROZEN: u32 = 0x50;
const LOCK_FRAMES: u16 = 0x20;
/// Variant -> trampoline kind (`+0x42`); variant -> position X, Y (two words).
const KIND: u32 = 0xd4198;
const POSITION: u32 = 0xd41a6;
/// Tables of box pointers (by `+0x2e`), 8 bytes per box (dx, half-width, dz, half-height ...): contact box,
/// presence box 1 and 2.
const BOXES_HIT: u32 = 0x11bffa;
const BOXES_NEAR: u32 = 0x11c11a;
const BOXES_NEAR2: u32 = 0x11c23a;
const BOX_SET: u8 = 0x42;
/// Subfields of the contact box (`+0x38`) and of the presence boxes (`+0x39`, `+0x3a`).
const HIT_SELECTOR: u32 = 0x38;
const NEAR_SELECTOR: u32 = 0x39;
const NEAR2_SELECTOR: u32 = 0x3a;
/// States of the launched player.
const AIRBORNE: u32 = 0x0200_2400;
const AIRBORNE_BACK: u32 = 0x0200_2402;

/// A launch: player key (bit of `+0x6c`), speeds, initial height, damping (`+0x1e`), arrival state, and whether the
/// orientation is reset to zero.
struct Launch {
    button: u8,
    vx: i16,
    vy: i16,
    z: u32,
    drag: i16,
    state: u32,
    reset_flip: bool,
}

const LAUNCHES: [Launch; 8] = [
    Launch { button: 0, vx: 0x300, vy: 0x400, z: 0x2_0000, drag: -0x20, state: AIRBORNE, reset_flip: false },
    Launch { button: 1, vx: -0x380, vy: 0x380, z: 0x2_0000, drag: -0x20, state: AIRBORNE, reset_flip: false },
    Launch { button: 2, vx: 0, vy: 0, z: 0x2_0000, drag: -0x30, state: AIRBORNE, reset_flip: false },
    // entry 3 is special (see `kick_up`)
    Launch { button: 3, vx: 0, vy: 0, z: 0, drag: 0, state: 0, reset_flip: false },
    Launch { button: 0, vx: 0x480, vy: 0, z: 0x2_0000, drag: -0x30, state: AIRBORNE_BACK, reset_flip: false },
    Launch { button: 2, vx: -0x200, vy: 0, z: 0x3_0000, drag: -0x30, state: AIRBORNE_BACK, reset_flip: true },
    Launch { button: 0, vx: 0x300, vy: 0x500, z: 0x2_0000, drag: -0x20, state: AIRBORNE, reset_flip: false },
    Launch { button: 2, vx: -0x300, vy: 0, z: 0x2_4000, drag: -0x30, state: AIRBORNE_BACK, reset_flip: true },
];

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let lock = x.a5_u16(LOCK);
    if lock != 0 {
        if lock != x.o.0 as u16 {
            return;
        }
        let n = x.u16(BumperObj::LOCK_DELAY).wrapping_sub(1);
        x.set16(BumperObj::LOCK_DELAY, n);
        if (n as i16) < 0 {
            x.set_a5_u16(LOCK, 0);
        }
    }
    if x.u8(Obj::HIDDEN) != 0 || x.a5_u8(FROZEN) != 0 {
        return;
    }
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => watch(x),
        _ => x.free(&pools::SET),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set8(BumperObj::BOX_SET, BOX_SET);
    let variant = x.u8(Obj::VARIANT) as u32;
    let kind = set_tables::pc_byte(x.b, KIND + variant);
    x.set8(BumperObj::KIND, kind);
    let px = set_tables::pc_word(x.b, POSITION + 4 * variant);
    x.set16(Obj::X, px);
    let py = set_tables::pc_word(x.b, POSITION + 4 * variant + 2);
    x.set16(Obj::Y, py);
    x.set16(Obj::Z, 0);
    x.anim_start(ANIMS, variant as u16);
    x.set32(BumperObj::PLAYER, 0);
    x.reinsert();
}

fn watch<R: Bench>(x: &mut Cx<'_, R>) {
    for p in players() {
        if p.u8(x.b, pl::ACTIVE) == 0 || p.u16(x.b, pl::HEALTH) == 0 {
            continue;
        }
        if !within(x, p, BOXES_NEAR, NEAR_SELECTOR) && !within(x, p, BOXES_NEAR2, NEAR2_SELECTOR) {
            return;
        }
    }
    for p in players() {
        if p.u8(x.b, pl::ACTIVE) != 0 && keys(x, p) {
            launch(x, p);
        }
    }
}

/// Address of the box of row `selector` of the set `+0x2e` of the table `table`.
fn box_of<R: Bench>(x: &mut Cx<'_, R>, table: u32, selector: u32) -> u32 {
    let set = x.u8(BumperObj::BOX_SET) as u32;
    let boxes = set_tables::long(x.b, table + 4 * set);
    boxes.wrapping_add(8 * x.u8(selector) as u32)
}

/// Test of an axis: the corrected distance is in the interval (unsigned comparison).
fn in_range(delta: u16, half: u8, shift: Option<i8>) -> bool {
    let mut d = delta.wrapping_add(half as u16);
    if let Some(s) = shift {
        d = d.wrapping_sub(s as i16 as u16);
    }
    d <= (half as u16).wrapping_mul(2)
}

/// The player is in a presence box (box offset by two offset bytes).
fn within<R: Bench>(x: &mut Cx<'_, R>, p: Obj, table: u32, selector: u32) -> bool {
    let b = box_of(x, table, selector);
    let dx = p.u16(x.b, pl::X).wrapping_sub(x.u16(Obj::X));
    let shift_x = set_tables::byte(x.b, b) as i8;
    let half_x = set_tables::byte(x.b, b + 1);
    if !in_range(dx, half_x, Some(shift_x)) {
        return false;
    }
    let dy = p.u16(x.b, pl::Y).wrapping_sub(x.u16(Obj::Y));
    let shift_y = set_tables::byte(x.b, b + 2) as i8;
    let half_y = set_tables::byte(x.b, b + 3);
    in_range(dy, half_y, Some(shift_y))
}

/// The player is in the contact box and in a state that allows it.
fn keys<R: Bench>(x: &mut Cx<'_, R>, p: Obj) -> bool {
    let b = box_of(x, BOXES_HIT, HIT_SELECTOR);
    if p.u8(x.b, Player::SUB) != 0 || p.u8(x.b, Player::SLOW_8A) != 0 {
        return false;
    }
    if p.u8(x.b, Player::ACTION) == 0x22 || p.u16(x.b, pl::STATE) == 0x204 || p.u8(x.b, Player::ACTION) == 0xe {
        return false;
    }
    if p.u16(x.b, pl::HEALTH) == 0 {
        return false;
    }
    if p.u32(x.b, pl::STATE) & 0xffff_ff00 == 0x0200_2000 {
        return false;
    }
    let dx = p.u16(x.b, pl::X).wrapping_sub(x.u16(Obj::X));
    if !in_range(dx, set_tables::byte(x.b, b + 1), None) {
        return false;
    }
    let dy = p.u16(x.b, pl::Y).wrapping_sub(x.u16(Obj::Y));
    in_range(dy, set_tables::byte(x.b, b + 3), None)
}

/// Launches the player according to the trampoline kind (`+0x42`).
fn launch<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
    let kind = x.u8(BumperObj::KIND) as usize;
    if kind >= LAUNCHES.len() {
        return;
    }
    if kind == 3 {
        return kick_up(x, p);
    }
    let l = &LAUNCHES[kind];
    if p.u8(x.b, Player::KEYS_NEW) & 0x20 == 0 || (p.u16(x.b, pl::Z) as i16) > 0 || p.u8(x.b, Player::KEYS_HELD) >> l.button & 1 == 0 {
        return;
    }
    grab(x);
    p.set_u8(x.b, Player::SLOW, 0);
    p.set_u32(x.b, pl::Z, l.z);
    p.set_u16(x.b, LaunchedPlayer::AX, 0);
    p.set_u16(x.b, LaunchedPlayer::VZ, 0);
    p.set_u16(x.b, LaunchedPlayer::VX, l.vx as u16);
    p.set_u16(x.b, LaunchedPlayer::VY, l.vy as u16);
    p.set_u16(x.b, LaunchedPlayer::AZ, 0xffff);
    p.set_u16(x.b, LaunchedPlayer::DRAG, l.drag as u16);
    if l.reset_flip {
        p.set_u8(x.b, Player::FACING, 0);
    }
    p.set_u32(x.b, pl::STATE, l.state);
}

/// Takes the lock and arms the release delay.
fn grab<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_a5_u16(LOCK, x.o.0 as u16);
    x.set16(BumperObj::LOCK_DELAY, LOCK_FRAMES);
}

/// Kind 3: pushes a player who presses key 3 and whose height is at least 0x28: the player is moved by 0x28 in
/// depth and lowered by as much.
fn kick_up<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
    if p.u8(x.b, Player::KEYS_HELD) & 8 == 0 || (p.u16(x.b, pl::Z) as i16) < 0x28 {
        return;
    }
    grab(x);
    let y = p.u16(x.b, pl::Y).wrapping_add(0x28);
    p.set_u16(x.b, pl::Y, y);
    let z = p.u16(x.b, pl::Z).wrapping_sub(0x28);
    p.set_u16(x.b, pl::Z, z);
}
