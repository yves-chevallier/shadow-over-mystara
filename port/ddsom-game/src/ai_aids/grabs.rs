//! Attacks on the players, grabs and pushes (0xfc44-0xfda2, 0xfe34, 0x115e2).

use super::*;

/// Push (16.16) that boss `me` applies to player `player` for an x gap `gap`: 0 if the player is too high or too far
/// in depth. Profile by x gap / 16 (`ai/push_profile_index`), then the profile (`ai/push_profiles`: maximum height,
/// maximum depth gap, push).
fn push_amount_for(b: &mut Cps2Bus, me: Obj, player: Obj, gap: u16) -> u32 {
    if gap as i16 >= 512 {
        return 0;
    }
    let profile = table("ai/push_profile_index").at8(b, offset(gap >> 4)) as u16;
    let profiles = table("ai/push_profiles");
    let entry = offset(profile << 3);
    let max_height = profiles.at16(b, entry) as i16;
    let max_depth = profiles.at16(b, entry.wrapping_add(2)) as i16;
    let depth = (player.u16(b, Obj::Y).wrapping_sub(me.u16(b, Obj::Y)) as i16).wrapping_abs();
    if max_height >= player.u16(b, Obj::Z) as i16 && depth < max_depth {
        profiles.at32(b, entry.wrapping_add(4))
    } else {
        0
    }
}

/// Boss `me` pushes player `player` in x, towards the side it faces; true if pushed.
fn push_player_once(b: &mut Cps2Bus, me: Obj, player: Obj) -> bool {
    let gap = (player.u16(b, Obj::X).wrapping_sub(me.u16(b, Obj::X)) as i16).wrapping_abs() as u16;
    let push = push_amount_for(b, me, player, gap);
    let facing = me.flip(b);
    let side = if (me.u16(b, Obj::X).wrapping_sub(player.u16(b, Obj::X)) as i16) < 0 { 32 } else { 0 };
    if push == 0 || side != facing {
        return false;
    }
    let push = if facing == 0 { push.wrapping_neg() } else { push };
    let x = player.x(b).wrapping_add(push);
    player.set_x(b, x);
    true
}

/// Signed byte of the engine data at `a`.
fn byte_signed(b: &mut Cps2Bus, a: u32) -> i16 {
    engine().byte(b, a) as i8 as i16
}

/// Is player `p` hit by the attack box at `attack` (entry address in `ai/attack_boxes`: offset and half-extent in
/// x, z, y) of object `me`? The player must be present, visible, unprotected, neither on the ground nor dead, and
/// have a body box (one set per character, `ai/body_box_sets`, in the collision boxes).
fn player_hit(b: &mut Cps2Bus, me: Obj, attack: u32, p: Player) -> bool {
    let flags = p.active(b);
    if flags & 1 == 0 || flags & 2 != 0 || p.held(b) != 0 {
        return false;
    }
    if p.obj().u16(b, Player::MAIN) == 0x204 || p.main(b) == 4 {
        return false;
    }
    let body = p.body_box(b);
    if body == 0 || p.invuln(b) != 0 || p.obj().u16(b, Player::DYING_TIMER) != 0 {
        return false;
    }
    let character = p.character(b) & 14;
    let set = table("ai/body_box_sets").at32(b, 2 * character as u32);
    let mut mine = attack;
    let mut theirs = set.wrapping_add(8 * body as u32);
    // x, then z, then y: (offset, extent) of each box; only x takes the facing direction into account
    for (axis, mirror) in [(Obj::X, true), (Obj::Z, false), (Obj::Y, false)] {
        let (mut d0, mut d1) = (byte_signed(b, mine), byte_signed(b, theirs));
        if mirror {
            if me.flip(b) != 0 {
                d0 = -d0;
            }
            if p.facing(b) != 0 {
                d1 = -d1;
            }
        }
        let gap = d0.wrapping_add(me.u16(b, axis) as i16).wrapping_sub(d1.wrapping_add(p.obj().u16(b, axis) as i16));
        let reach = engine().byte(b, mine + 1) as i16 + engine().byte(b, theirs + 1) as i16;
        mine += 2;
        theirs += 2;
        if reach < gap.wrapping_abs() {
            return false;
        }
    }
    true
}

/// 0xfe34: object `me` is brought back into the camera's screen. Returns true if it was.
fn keep_inside_camera(b: &mut Cps2Bus, me: Obj) -> bool {
    let (left, _) = crate::ram::camera(b);
    let x = me.u16(b, Obj::X) as i16;
    if left as i16 > x {
        me.set_u16(b, Obj::X, left);
        return true;
    }
    let right = left.wrapping_add(384);
    if right as i16 >= x {
        return false;
    }
    me.set_u16(b, Obj::X, right);
    true
}

/// 0xfc44: is a player hit by the attack box `attack` of `o`? Returns the first one hit.
pub fn attack_hits_a_player(b: &mut Cps2Bus, o: Obj, attack: u8) -> Option<Obj> {
    let boxes = table("ai/attack_boxes").rom.wrapping_add(offset((attack as u16) << 3));
    crate::ram::PLAYERS.iter().find(|&p| player_hit(b, o, boxes, Player::of(p)))
}

/// 0xfd64: player `player` is grabbed by `o` (grab animation `anim` of `o`).
pub fn seize_player(b: &mut Cps2Bus, o: Obj, player: Obj, anim: u8) {
    let p = Player::of(player);
    player.set_u32(b, Obj::STATE, 0x0204_3200);
    p.set_active(b, 2);
    p.set_holder_enemy(b, o.word());
    p.set_held(b, 1);
    let e = Enemy(o);
    e.set_target(b, player.word());
    e.set_state_long(b, 0x020c_0000);
    e.set_step(b, anim);
}

/// 0xfd8c: player `player` is released (action 0x34).
pub fn release_player(b: &mut Cps2Bus, player: Obj) {
    let p = Player::of(player);
    player.set_u32(b, Obj::STATE, 0x0204_0000);
    p.set_action(b, 0x34);
    p.set_held(b, 0);
}

/// 0xfda2: `o` is brought back into the screen, then the object it holds (`+0x50`) follows it according to the step
/// (`+0x3e`) of the grab animation (`ai/seize_moves`: x, height, orientation, direction of `+0x12`).
pub fn hold_seized(b: &mut Cps2Bus, o: Obj) {
    let e = Enemy(o);
    keep_inside_camera(b, o);
    let pointer = e.target(b);
    if (pointer as i16) >= 0 {
        return;
    }
    let held = Enemy(Obj::from_word(pointer));
    let step = e.strike_key(b) as u32;
    if step == 0 {
        return;
    }
    let mut at = table("ai/seize_moves").rom + 4 * step;
    let mirrored = e.flip(b) != 0;
    let mut dx = byte_signed(b, at);
    if mirrored {
        dx = -dx;
    }
    let x = (dx as u16).wrapping_add(e.x(b));
    held.set_x(b, x);
    let rise = (byte_signed(b, at + 1) as u16).wrapping_add(e.height(b));
    let height = if (rise as i16) < 0 {
        let z = e.height(b).wrapping_sub(rise);
        e.set_height(b, z);
        0
    } else {
        rise
    };
    held.set_height(b, height);
    let depth = e.y(b);
    held.set_y(b, depth);
    e.set_y_fraction(b, 1);
    held.set_y_fraction(b, 1);
    at += 2;
    let faces = engine().byte(b, at) != 0;
    let mut facing: u8 = if faces { 32 } else { 0 };
    if mirrored {
        facing = if facing != 0 { 0 } else { 32 };
    }
    held.set_flip(b, facing);
    let direction: i16 = if (engine().byte(b, at + 1) as i8) < 0 { 1 } else { -1 };
    let v = e.y_fraction(b).wrapping_add(direction as u16);
    e.set_y_fraction(b, v);
}

/// 0x115e2: boss `o` pushes each player present in front of it, except the thief or the dwarf in their state 26
/// (`+0x6a`), a player exiting (`+0x66`) or protected (`+0x188`).
pub fn push_players(b: &mut Cps2Bus, o: Obj) {
    for player in crate::ram::PLAYERS.iter() {
        let p = Player::of(player);
        if p.active(b) == 0 {
            continue;
        }
        if p.character(b) >> 1 == 5 && p.subaction(b) == 26 {
            continue;
        }
        if p.slow(b) == 0 && p.held(b) == 0 {
            push_player_once(b, o, player);
        }
    }
}
