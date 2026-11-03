//! Attacks on the players, grabs and pushes (0xfc44-0xfda2, 0xfe34, 0x115e2).

use super::*;

/// Push profiles: byte table (indexed by the x gap / 16) in program space, then
/// 8-byte entries (maximum height, maximum depth gap, push) in data space.
pub(super) const PUSH_PROFILE_INDEX: u32 = 0x1168e;
pub(super) const PUSH_PROFILES: u32 = 0x116ae;

/// Byte of the program space (table addressed PC-relative).
pub(super) fn prog8(b: &mut Cps2Bus, a: u32) -> u8 {
    let word = b.fetch16(a & !1);
    if a & 1 == 0 {
        (word >> 8) as u8
    } else {
        word as u8
    }
}

/// Push (16.16) that boss `me` applies to player `player` for an x gap `gap`: 0 if the
/// player is too high or too far in depth.
pub(super) fn push_amount_for(b: &mut Cps2Bus, me: u32, player: u32, gap: u16) -> u32 {
    if gap as i16 >= 512 {
        return 0;
    }
    let profile = prog8(b, PUSH_PROFILE_INDEX.wrapping_add(offset(gap >> 4))) as u16;
    let entry = PUSH_PROFILES.wrapping_add(offset(profile << 3));
    let max_height = rd16(b, entry) as i16;
    let max_depth = rd16(b, entry + 2) as i16;
    let depth = (rd16(b, player + f::DEPTH).wrapping_sub(rd16(b, me + f::DEPTH)) as i16).wrapping_abs();
    if max_height >= rd16(b, player + 0x0c) as i16 && depth < max_depth {
        rd32(b, entry + 4)
    } else {
        0
    }
}

/// Boss `me` pushes player `player` in x, towards the side it faces; true if pushed.
pub(super) fn push_player_once(b: &mut Cps2Bus, me: u32, player: u32) -> bool {
    let gap = (rd16(b, player + f::X).wrapping_sub(rd16(b, me + f::X)) as i16).wrapping_abs() as u16;
    let push = push_amount_for(b, me, player, gap);
    let facing = rd8(b, me + f::FACING);
    let side = if (rd16(b, me + f::X).wrapping_sub(rd16(b, player + f::X)) as i16) < 0 { 32 } else { 0 };
    if push == 0 || side != facing {
        return false;
    }
    let push = if facing == 0 { push.wrapping_neg() } else { push };
    let x = rd32(b, player + f::X).wrapping_add(push);
    wr32(b, player + f::X, x);
    true
}

pub(super) const ANIM_MARK: u32 = 0x3e;

/// Enemy attack boxes (8-byte entries: offset and half-extent in x, z, y) and player
/// body boxes (one table per character, 8-byte entries), in data space.
pub(super) const ATTACK_BOXES: u32 = 0x123602;
pub(super) const BODY_BOX_TABLES: u32 = 0xfd4c;

/// Signed byte of the data space.
pub(super) fn rd8s(b: &mut Cps2Bus, a: u32) -> i16 {
    rd8(b, a) as i8 as i16
}

/// Is the player at `player` hit by the attack box `attack` (entry address) of object `me`?
/// The player must be present, visible, unprotected, neither on the ground nor dead, and have a body box.
pub(super) fn player_hit(b: &mut Cps2Bus, me: u32, attack: u32, player: u32) -> bool {
    let flags = rd8(b, player);
    if flags & 1 == 0 || flags & 2 != 0 || rd8(b, player + 0x188) != 0 {
        return false;
    }
    if rd16(b, player + 4) == 0x204 || rd8(b, player + 4) == 4 {
        return false;
    }
    let body = rd8(b, player + 0x38);
    if body == 0 || rd8(b, player + 0x7e) != 0 || rd16(b, player + 0x196) != 0 {
        return false;
    }
    let character = rd8(b, player + 3) & 14;
    let table = rd32(b, BODY_BOX_TABLES + 2 * character as u32);
    let mut mine = attack;
    let mut theirs = table.wrapping_add(8 * body as u32);
    // x, then z, then y: (offset, extent) of each box; only x takes the facing direction into account
    for (axis, mirror) in [(0x08, true), (0x0c, false), (0x10, false)] {
        let (mut d0, mut d1) = (rd8s(b, mine), rd8s(b, theirs));
        if mirror {
            if rd8(b, me + f::FACING) != 0 {
                d0 = -d0;
            }
            if rd8(b, player + f::FACING) != 0 {
                d1 = -d1;
            }
        }
        let gap = d0.wrapping_add(rd16(b, me + axis) as i16).wrapping_sub(d1.wrapping_add(rd16(b, player + axis) as i16));
        let reach = rd8(b, mine + 1) as i16 + rd8(b, theirs + 1) as i16;
        mine += 2;
        theirs += 2;
        if reach < gap.wrapping_abs() {
            return false;
        }
    }
    true
}

/// Grab movement table (signed bytes: x offset, height offset, orientation, direction).
pub(super) const SEIZE_OFFSETS: u32 = 0x123652;

/// 0xfe34: object A0 is brought back into the camera's screen. Returns true if it was.
pub(super) fn keep_inside_camera(b: &mut Cps2Bus, me: u32) -> bool {
    let left = if rd8(b, A5 + 0x36) == 0 { rd16(b, A5 + 0x506) } else { rd16(b, A5 + 0x556) };
    let x = rd16(b, me + f::X) as i16;
    if left as i16 > x {
        wr16(b, me + f::X, left);
        return true;
    }
    let right = left.wrapping_add(384);
    if right as i16 >= x {
        return false;
    }
    wr16(b, me + f::X, right);
    true
}

/// 0xfc44: is a player hit by the attack box `attack` of `o`? Returns the first one hit.
pub fn attack_hits_a_player(b: &mut Cps2Bus, o: Obj, attack: u8) -> Option<Obj> {
    let boxes = ATTACK_BOXES.wrapping_add(offset((attack as u16) << 3));
    crate::ram::PLAYERS.iter().find(|p| player_hit(b, o.0, boxes, p.0))
}

/// 0xfd64: player `player` is grabbed by `o` (grab animation `anim` of `o`).
pub fn seize_player(b: &mut Cps2Bus, o: Obj, player: Obj, anim: u8) {
    player.set_u32(b, Obj::STATE, 0x0204_3200);
    player.set_u8(b, Obj::ACTIVE, 2);
    player.set_u16(b, 0x186, o.word());
    player.set_u8(b, 0x188, 1);
    o.set_u16(b, crate::ai::f::TARGET, player.word());
    o.set_u32(b, Obj::STATE, 0x020c_0000);
    o.set_u8(b, f::SUB_STATE, anim);
}

/// 0xfd8c: player `player` is released (action 0x34).
pub fn release_player(b: &mut Cps2Bus, player: Obj) {
    player.set_u32(b, Obj::STATE, 0x0204_0000);
    player.set_u8(b, f::SUB_STATE, 0x34);
    player.set_u8(b, 0x188, 0);
}

/// 0xfda2: `o` is brought back into the screen, then the object it holds (`+0x50`) follows it according to the step
/// (`+0x3e`) of the grab animation (movement table: x, height, orientation, direction of `+0x12`).
pub fn hold_seized(b: &mut Cps2Bus, o: Obj) {
    let me = o.0;
    keep_inside_camera(b, me);
    let pointer = rd16(b, me + f::TARGET);
    if (pointer as i16) >= 0 {
        return;
    }
    let held = object(pointer);
    let step = rd8(b, me + ANIM_MARK) as u32;
    if step == 0 {
        return;
    }
    let mut at = SEIZE_OFFSETS + 4 * step;
    let mirrored = rd8(b, me + f::FACING) != 0;
    let mut dx = rd8s(b, at);
    if mirrored {
        dx = -dx;
    }
    let x = (dx as u16).wrapping_add(rd16(b, me + f::X));
    wr16(b, held + f::X, x);
    let rise = (rd8s(b, at + 1) as u16).wrapping_add(rd16(b, me + 0x0c));
    let height = if (rise as i16) < 0 {
        let z = rd16(b, me + 0x0c).wrapping_sub(rise);
        wr16(b, me + 0x0c, z);
        0
    } else {
        rise
    };
    wr16(b, held + 0x0c, height);
    let depth = rd16(b, me + f::DEPTH);
    wr16(b, held + f::DEPTH, depth);
    wr16(b, me + 0x12, 1);
    wr16(b, held + 0x12, 1);
    at += 2;
    let faces = rd8(b, at) != 0;
    let mut facing: u8 = if faces { 32 } else { 0 };
    if mirrored {
        facing = if facing != 0 { 0 } else { 32 };
    }
    wr8(b, held + f::FACING, facing);
    let direction: i16 = if (rd8(b, at + 1) as i8) < 0 { 1 } else { -1 };
    let v = rd16(b, me + 0x12).wrapping_add(direction as u16);
    wr16(b, me + 0x12, v);
}

/// 0x115e2: boss `o` pushes each player present in front of it, except the thief or the dwarf in their state 26
/// (`+0x6a`), a player exiting (`+0x66`) or protected (`+0x188`).
pub fn push_players(b: &mut Cps2Bus, o: Obj) {
    for p in crate::ram::PLAYERS.iter() {
        let player = p.0;
        if rd8(b, player) == 0 {
            continue;
        }
        if rd8(b, player + 3) >> 1 == 5 && rd8(b, player + 0x6a) == 26 {
            continue;
        }
        if rd8(b, player + 0x66) == 0 && rd8(b, player + 0x188) == 0 {
            push_player_once(b, o.0, player);
        }
    }
}
