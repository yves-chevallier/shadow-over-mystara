use super::*;
use crate::families::shell::tables::{box_byte, tables, Read};

const ANIMS: u32 = 0x17e048;
const PARENT: u32 = 0x46;
const PARENT_STATE: u32 = 0xa0;
const SUBTYPE: u32 = 0x2f;
const TIMER: u32 = 0x42;
/// Screen: X (`A5+0x506`), Y (long `A5+0x50a`).
const SCREEN_X: u32 = 0x506;
const SCREEN_Y: u32 = 0x50a;
const STAGE: u32 = 0x34;
const SCREEN_LOCK: u32 = 0x43;
const LOCK_TAKEN: u8 = 0x3f;
const LOCK_FREE: u8 = 0xff;
const FLAG_4BA: u32 = 0x4ba;
/// Safe rectangles of stage 0x69, one per bit (h).
const RECT_FLAGS: u32 = 0xff2064;
/// First player (`A5+0x5de`) and step.
const PLAYERS: u32 = 0x5de;
const PLAYER_SIZE: u32 = 0x200;
const SCREEN_END_FX: crate::sound::Sound = crate::sound::CMD_1F9;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(0x3) {
        0 => variant0(x),
        1 => variant1(x),
        2 => variant2(x),
        3 => variant3(x),
        _ => {}
    }
}

fn vanish<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x4, 6);
    x.set8(0x5, 0);
}

/// Living boss (negative address, non-zero activity byte).
fn parent<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
    let w = x.u16(PARENT);
    if s16(w) >= 0 {
        return None;
    }
    let o = obj_of(w);
    (o.u8(x.b, 0) != 0).then_some(o)
}

fn init_boxes<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(f::BOX_SET, 7);
    x.set8(f::ATTACK_POWER, 0);
}

/// Aligns on the boss; the sub-type offsets in height (0x1c minus the sub-type).
fn attach<R: Bench>(x: &mut Cx<'_, R>, boss: Obj) {
    copy_xyz16(x, boss);
    let sub = x.u8(SUBTYPE) as u16;
    if sub == 0 {
        x.add32(0x10, 1);
    }
    x.add16(0xc, 0x1cu16.wrapping_sub(sub));
    x.add16(0x10, sub);
}

/// End of the frame: kept on the screen then put back in the sorted list.
fn settle<R: Bench>(x: &mut Cx<'_, R>) {
    x.keep_on_screen(0x10);
    x.reinsert();
}

fn unlock_screen<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_a5_u8(SCREEN_LOCK, LOCK_FREE);
    x.set_a5_u16(FLAG_4BA, 0);
}

// ----- variant 0 -----

fn variant0<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(0x5, 0);
            let Some(boss) = parent(x) else { return vanish(x) };
            attach(x, boss);
            let sub = x.u8(SUBTYPE) as u32;
            let anim = tables().finale52_anims_of_subtype.at8(x.b, sub);
            x.anim_start(ANIMS, (anim as u32) as u16);
            init_boxes(x);
        }
        2 => {
            let Some(boss) = parent(x) else { return vanish(x) };
            if boss.u16(x.b, PARENT_STATE) != 0x201 {
                return vanish(x);
            }
            x.set_bit(0x1, 1, false);
            match x.u8(SUBTYPE) {
                0 => orb(x, boss),
                1..=3 => follower(x, boss),
                _ => {}
            }
            x.set_a5_u16(FLAG_4BA, 0);
            hit_players(x);
            x.reinsert();
        }
        4 => x.add8(0x4, 2),
        6 => x.free_eshl(),
        _ => {}
    }
}

/// Sub-type 0: follows the launch animation, then launches three elements and sticks beside the boss.
fn orb<R: Bench>(x: &mut Cx<'_, R>, boss: Obj) {
    let mut source = boss;
    if x.u8(0x5) == 0 {
        if !x.anim_step() {
            return attach(x, boss);
        }
        x.add8(0x5, 2);
        let last = spawn_followers(x);
        x.anim_start(ANIMS, 0x1b);
        // Original glitch: the position is then copied from A1, which then designates the last element created (and
        // not the boss), or, if the pool was full, the slot that the allocation leaves in A1.
        source = last;
    }
    x.anim_step();
    copy_xyz16(x, source);
    copy_flip(x, source);
    let dx = if x.u8(f::FLIP) != 0 { 8 } else { neg(8) };
    x.add16(0x8, dx);
    x.add16(0xc, 0xe);
    x.add16(0x10, 0xffff);
}

/// Creates three elements of sub-types 3, 2, 1 (table), linked to the same boss. Returns the original's A1: the
/// last element created, or what the failing allocation leaves (`Pool::alloc_or_leftover`).
fn spawn_followers<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
    let mut a1 = None;
    for i in (0..3u32).rev() {
        let c = match x.alloc_or_leftover(&pools::ESHL) {
            Ok(c) => c,
            Err(a1) => return a1,
        };
        a1 = Some(c);
        c.set_u8(x.b, 0, 1);
        c.set_u8(x.b, 2, 0x34);
        c.set_u8(x.b, 3, 0);
        let sub = tables().finale52_orb_subtypes.at8(x.b, i);
        c.set_u8(x.b, SUBTYPE, sub);
        let boss = x.u16(PARENT);
        c.set_u16(x.b, PARENT, boss);
    }
    a1.expect("three followers created")
}

/// Sub-types 1 to 3: invisible as long as the boss has not armed `+0x3e` (except sub-type 2).
fn follower<R: Bench>(x: &mut Cx<'_, R>, boss: Obj) {
    attach(x, boss);
    x.set_bit(0x1, 1, true);
    let always = x.u8(SUBTYPE) & 7 == 2;
    if !always && boss.u8(x.b, 0x3e) == 0 {
        return;
    }
    x.set_bit(0x1, 1, false);
    x.anim_step();
}

// ----- variant 1 -----

fn variant1<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(0x5, 0);
            let Some(boss) = parent(x) else { return vanish(x) };
            attach(x, boss);
            x.anim_start(ANIMS, 0x1c);
            init_boxes(x);
        }
        2 => {
            x.set_a5_u16(FLAG_4BA, 0);
            let Some(boss) = parent(x) else { return vanish(x) };
            x.set_bit(0x1, 1, false);
            if x.u8(0x5) == 0 {
                if boss.u16(x.b, PARENT_STATE) == 0x214 {
                    x.anim_step();
                    beside(x, boss);
                    hit_players(x);
                    return settle(x);
                }
                x.add8(0x5, 2);
                x.anim_start(ANIMS, 0x1d);
            }
            if x.anim_step() {
                return vanish(x);
            }
            beside(x, boss);
            hit_players(x);
            settle(x);
        }
        4 => x.add8(0x4, 2),
        6 => x.free_eshl(),
        _ => {}
    }
}

/// Stuck to the boss, 8 pixels in front of it and 0x28 higher; the boss advances one notch in Y.
fn beside<R: Bench>(x: &mut Cx<'_, R>, boss: Obj) {
    let (px, pz) = (boss.u16(x.b, 0x8), boss.u16(x.b, 0xc));
    x.set16(0x8, px);
    x.set16(0xc, pz);
    let py = boss.u32(x.b, 0x10);
    x.set32(0x10, py);
    copy_flip(x, boss);
    let dx = if x.u8(f::FLIP) != 0 { neg(8) } else { 8 };
    x.add16(0x8, dx);
    x.add16(0xc, 0x28);
    let n = boss.u16(x.b, 0x12).wrapping_add(1);
    boss.set_u16(x.b, 0x12, n);
}

// ----- variants 2 and 3: full-screen veil -----

/// Aligns the veil on the screen (X, Y) at height 0x108; the boss advances one notch in Y if there is one.
fn cover_screen<R: Bench>(x: &mut Cx<'_, R>, boss: Option<Obj>) {
    let sx = x.a5_u16(SCREEN_X);
    x.set16(0x8, sx);
    x.set16(0xc, 0);
    let sy = x.a5_u32(SCREEN_Y);
    x.set32(0x10, sy);
    x.add16(0xc, 0x108);
    if let Some(boss) = boss {
        let n = boss.u16(x.b, 0x12).wrapping_add(1);
        boss.set_u16(x.b, 0x12, n);
    }
}

fn lock_screen<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_a5_u8(SCREEN_LOCK, LOCK_TAKEN);
    init_boxes(x);
}

fn variant2<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(0x5, 0);
            let Some(boss) = parent(x) else { return vanish(x) };
            attach(x, boss);
            x.anim_start(ANIMS, 0x1e);
            lock_screen(x);
            crate::families::small::four_palette_blocks(x, true);
        }
        2 => {
            let boss = parent(x).filter(|b| b.u16(x.b, PARENT_STATE) == 0x215);
            let Some(boss) = boss else {
                x.add8(0x4, 2);
                x.add8(0x4, 2);
                return unlock_screen(x);
            };
            x.set_bit(0x1, 1, false);
            if x.u8(0x5) == 0 {
                if x.anim_step() {
                    x.add8(0x5, 2);
                    x.anim_start(ANIMS, 0x1f);
                    return;
                }
                cover_screen(x, Some(boss));
                let d = boss.u16(x.b, 0x10).wrapping_sub(x.u16(0x10));
                x.add16(0x10, d);
                x.add16(0x10, 0xffff);
                x.add16(0xc, neg(d));
                x.add16(0xc, 1);
            } else {
                x.set_a5_u16(FLAG_4BA, 0);
                x.anim_step();
                fall_check(x);
                // original trap: the player loop left A1 after the fourth player (0xff8dde) and 0xa49fc increments
                // the word +0x12 of this "object", not that of the boss
                cover_screen(x, Some(Obj(0xff_8000 + PLAYERS + 4 * PLAYER_SIZE)));
            }
            settle(x);
        }
        4 => {
            x.add8(0x4, 2);
            unlock_screen(x);
        }
        6 => x.free_eshl(),
        _ => {}
    }
}

fn variant3<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(0x5, 0);
            x.anim_start(ANIMS, 0x23);
            lock_screen(x);
            // 0x101da in scene 0x69, 0x10158 otherwise
            let brighten = x.a5_u8(STAGE) != 0x69;
            crate::families::small::four_palette_blocks(x, brighten);
        }
        2 => {
            if x.a5_u8(STAGE) == 0x2d && s16(x.a5_u16(SCREEN_X)) >= 0x340 {
                crate::sound::play(x.b, SCREEN_END_FX, x.o);
                return end3(x);
            }
            if x.u8(0x5) == 0 {
                if x.anim_step() {
                    x.add8(0x5, 2);
                    x.set16(TIMER, 0x100);
                    x.anim_start(ANIMS, 0x1f);
                    return;
                }
            } else {
                x.set_a5_u16(FLAG_4BA, 0);
                let t = x.u16(TIMER).wrapping_sub(1);
                x.set16(TIMER, t);
                if s16(t) <= 0 {
                    return end3(x);
                }
                x.anim_step();
                fall_check(x);
            }
            cover_screen(x, None);
            settle(x);
        }
        4 => {
            x.add8(0x4, 2);
            unlock_screen(x);
        }
        6 => x.free_eshl(),
        _ => {}
    }
}

fn end3<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.add8(0x4, 2);
    unlock_screen(x);
}

// ----- collisions with the players -----

fn players() -> impl Iterator<Item = Obj> {
    (0..4).map(|i| Obj(0xff_8000 + PLAYERS + PLAYER_SIZE * i))
}

/// Player in normal play: active, alive (non-zero HP), state 2, not invulnerable.
fn alive<R: Bench>(x: &mut Cx<'_, R>, p: Obj) -> bool {
    p.u8(x.b, 0) != 0 && p.u8(x.b, 0x7e) == 0 && p.u8(x.b, 4) == 2 && p.u8(x.b, 0x10f) == 0 && p.u16(x.b, 0x62) != 0
}

/// The veil hurts the players touched by the attack (boxes `+0x3e`).
fn hit_players<R: Bench>(x: &mut Cx<'_, R>) {
    for p in players() {
        if alive(x, p) && overlap(x, p) {
            hurt(x, p);
        }
    }
}

/// Hurts the players who are not in a safe zone (stages 0x2e and 0x69).
fn fall_check<R: Bench>(x: &mut Cx<'_, R>) {
    for p in players() {
        if !alive(x, p) {
            continue;
        }
        let stage = x.a5_u8(STAGE);
        let hurts = if stage == 0x2e {
            !on_safe_range(x, p)
        } else if stage == 0x69 {
            let flags = crate::mem::rd8(x.b, RECT_FLAGS);
            !(flags & 2 != 0 && in_rect(x, p, 0) || flags & 1 != 0 && in_rect(x, p, 8))
        } else {
            true
        };
        if hurts {
            hurt(x, p);
        }
    }
}

/// Stage 0x2e: the player is on the ground (Z < 0x100) and in one of the safe zones.
fn on_safe_range<R: Bench>(x: &mut Cx<'_, R>, p: Obj) -> bool {
    if p.u8(x.b, 0xc) != 0 {
        return false;
    }
    let t = &tables().finale52_safe_ranges;
    let mut a2 = 0;
    loop {
        let start = t.at16(x.b, a2);
        a2 += 2;
        if start == 0 {
            return false;
        }
        let d = p.u16(x.b, 0x8).wrapping_sub(start);
        if s16(d) < 0 {
            return false;
        }
        let len = t.at16(x.b, a2);
        a2 += 2;
        if s16(d) <= s16(len) {
            return true;
        }
    }
}

/// Stage 0x69: the player is on the ground and in the rectangle (X, width, Y, height) at offset `rect` of
/// `finale52_safe_rects`.
fn in_rect<R: Bench>(x: &mut Cx<'_, R>, p: Obj, rect: u32) -> bool {
    let t = &tables().finale52_safe_rects;
    if p.u8(x.b, 0xc) != 0 {
        return false;
    }
    let px = s16(p.u16(x.b, 0x8));
    let x0 = t.at16(x.b, rect);
    if s16(x0) >= px {
        return false;
    }
    if s16(x0.wrapping_add(t.at16(x.b, rect + 2))) <= px {
        return false;
    }
    let y1 = t.at16(x.b, rect + 4).wrapping_add(t.at16(x.b, rect + 6));
    s16(y1) > s16(p.u16(x.b, 0x10))
}

/// Overlap of the attack boxes (row `+0x3e` - 1) and the player's (pointer according to its character number, row
/// according to `+0x38`), axis by axis: X (mirrored according to the orientation), Z then Y. Absolute distance of
/// the centers against the sum of the half-sizes.
fn overlap<R: Bench>(x: &mut Cx<'_, R>, p: Obj) -> bool {
    if p.u8(x.b, 0x38) == 0 {
        return false;
    }
    let n = x.u8(0x3e);
    if n == 0 {
        return false;
    }
    let rows = 16 * (n - 1) as u32;
    let attack = &tables().finale52_attack_boxes;
    let character = p.u8(x.b, 3) as u32 & 0xe;
    let table = tables().finale52_player_boxes.at32(x.b, 2 * character);
    let boxes = table + 8 * p.u8(x.b, 0x38) as u32;
    for (k, pos) in [0x8u32, 0xc, 0x10].into_iter().enumerate() {
        let k = k as u32;
        let mut mine = attack.at16(x.b, rows + 4 * k);
        let mut theirs = box_byte(x.b, boxes + 2 * k) as i8 as i16 as u16;
        if k == 0 {
            if x.u8(f::FLIP) != 0 {
                mine = neg(mine);
            }
            if p.u8(x.b, f::FLIP) != 0 {
                theirs = neg(theirs);
            }
        }
        let a = mine.wrapping_add(x.u16(pos));
        let b = theirs.wrapping_add(p.u16(x.b, pos));
        let mut d = a.wrapping_sub(b);
        if s16(d) < 0 {
            d = neg(d);
        }
        let reach = attack.at16(x.b, rows + 4 * k + 2).wrapping_add(box_byte(x.b, boxes + 2 * k + 1) as u16);
        if s16(reach) < s16(d) {
            return false;
        }
    }
    true
}

/// Wound: the player goes to state 0x02042000; HP -= max HP / 2 (except attacks 0x2c), a player at 0 HP is removed
/// from the count `A5+0x35`.
fn hurt<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
    p.set_u32(x.b, 0x4, 0x0204_2000);
    let mut hp = 0u16;
    if p.u8(x.b, 0x10d) == 0x2c || p.u8(x.b, 0x10e) == 0x2c {
        hp = p.u16(x.b, 0x62);
        let half = p.u16(x.b, 0x60) >> 1;
        let below = s16(hp) < s16(half);
        hp = hp.wrapping_sub(half);
        if below {
            hp = 0;
        }
    }
    p.set_u16(x.b, 0x62, hp);
    if hp == 0 {
        let n = x.a5_u8(0x35).wrapping_sub(1);
        x.set_a5_u8(0x35, n);
    }
}
