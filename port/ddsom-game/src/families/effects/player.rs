use super::*;

// ---------------------------------------------------------------------------------------------
// Type 5: halo attached to a player (0xab726)

/// Type 5: appears at the position of the owner player (`+0x50`), offset according to the variant (even, 0 to 10) and
/// the player's orientation, waits for a delay (byte `+0x18`, a function of byte `+0x7d` of the player) then lives
/// `0x28` frames before disappearing. Animation 0 of set `0x18e650`.
pub(super) mod halo {
    pub const ENTRY: u32 = 0xab726;
    pub const TABLE: u32 = 0xab734;
    /// Offsets (dx, dz) per half-variant, words.
    pub const OFFSETS: u32 = 0xab79a;
    /// Appearance delays according to `+0x7d` of the player.
    pub const DELAYS: u32 = 0xab7b2;
    pub const ANIMATIONS: u32 = 0x18e650;
    pub const DURATION: u32 = 0x14;
    pub const DELAY: u32 = 0x18;
}

pub(super) fn halo_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, halo::DURATION, 0x28);
    let player = pointer(ctx, o, field::OWNER);
    copy_position(ctx, o, player);
    let rank = ((o.u8(ctx.b, Obj::VARIANT) >> 1) as u32) << 2;
    let mut dx = ctx.prog16(halo::OFFSETS + rank);
    let dz = ctx.prog16(halo::OFFSETS + 2 + rank);
    let turned = player.u8(ctx.b, field::FLIP) != 0;
    if !turned {
        dx = dx.wrapping_neg();
    }
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Z, dz);
    let phase = player.u8(ctx.b, 0x7d) as u32;
    let delay = ctx.prog8(halo::DELAYS + phase);
    o.set_u8(ctx.b, halo::DELAY, delay);
    ctx.anim_start(halo::ANIMATIONS, 0);
}

pub(super) fn halo_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let delay = o.u8(ctx.b, halo::DELAY);
    if !(delay == 0) {
        o.set_u8(ctx.b, halo::DELAY, delay - 1);
        return;
    }
    let remainder = o.u8(ctx.b, halo::DURATION).wrapping_sub(1);
    o.set_u8(ctx.b, halo::DURATION, remainder);
    if !(remainder != 0) {
        add_u8(ctx, o, Obj::STATE, 2);
    }
    ctx.reinsert();
}

pub(super) fn halo<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, halo::TABLE, halo_init, halo_active)
}

// ---------------------------------------------------------------------------------------------
// Type 57: mark attached to a player (0xb6bca)

/// Type 57: mark placed 0x18 to the right and 0x2d above the owner (`+0x50`) as long as it stays in state `0x0204` /
/// `+6 = 0x2c`; otherwise it disappears. Not exercised.
pub(super) mod mark {
    pub const ENTRY: u32 = 0xb6bca;
    pub const TABLE: u32 = 0xb6bd8;
    pub const ANIMATIONS: u32 = 0x196b62;
}

pub(super) fn mark_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let owner = pointer(ctx, o, field::OWNER);
    for off in [Obj::X, Obj::Z] {
        let v = owner.u16(ctx.b, off);
        o.set_u16(ctx.b, off, v);
    }
    let y = owner.u32(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u32(ctx.b, Obj::Y, y);
    shifted_u16(ctx, o, Obj::X, 0x18);
    shifted_u16(ctx, o, Obj::Z, 0x2d);
    ctx.anim_start(mark::ANIMATIONS, 0);
    ctx.reinsert();
}

pub(super) fn mark_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let owner = pointer(ctx, o, field::OWNER);
    let state = owner.u16(ctx.b, 4);
    if state != 0x204 {
        return ctx.free(&pools::EFFCT);
    }
    let sub = owner.u8(ctx.b, 6);
    if sub != 0x2c {
        return ctx.free(&pools::EFFCT);
    }
    ctx.anim_step();
}

pub(super) fn mark_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, mark::TABLE, state as u32);
    match state {
        state::INIT => mark_init(ctx, o),
        state::ACTIVE => mark_active(ctx, o),
        state::FREED => ctx.free(&pools::EFFCT),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 52: sparkle of a character (0xb65e0)

/// Type 52: variant of the sparkle (type 15) that only watches the players of class 5 still alive (non-zero hit points
/// `+0x62`); once triggered it blinks eight times, hidden as long as its object (`+0x5c`) is. Not exercised.
pub(super) mod shimmer_class {
    pub const ENTRY: u32 = 0xb65e0;
    pub const TABLE: u32 = 0xb65ee;
    pub const ANIMATIONS: u32 = 0x1bc77c;
    pub const REPETITIONS: u32 = 0x57;
    pub const DELAY: u32 = 0x58;
    pub const TOGGLE: u32 = 0x5a;
    pub const TRIGGERS: u32 = 0x5b;
    pub const CARRIER: u32 = 0x5c;
}

pub(super) fn shimmer_class_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    o.set_u8(ctx.b, shimmer_class::REPETITIONS, 8);
    let t = draw(ctx, 0x1f);
    o.set_u16(ctx.b, shimmer_class::DELAY, t);
    ctx.anim_start(shimmer_class::ANIMATIONS, 0);
    ctx.reinsert();
}

/// `0xb668a`: has a living class 5 player passed near the object?
pub(super) fn shimmer_class_player<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    for i in 0..4u32 {
        let player = Obj(PLAYERS + i * STEP_PLAYER);
        let absent = player.u8(ctx.b, Obj::ACTIVE) == 0;
        if !absent {
            let dead = player.u16(ctx.b, 0x62) == 0;
            if !dead {
                let class = player.u8(ctx.b, 3) >> 1;
                if !(class != 5) {
                    let dy = o.u16(ctx.b, Obj::Y).wrapping_sub(player.u16(ctx.b, Obj::Y)).wrapping_add(0x20);
                    if !(dy > 0x40) {
                        let dx = o.u16(ctx.b, Obj::X).wrapping_sub(player.u16(ctx.b, Obj::X)).wrapping_add(0x60);
                        if !(dx > 0xc0) {
                            o.set_u8(ctx.b, shimmer_class::TRIGGERS, 1);
                            return;
                        }
                    }
                }
            }
        }
    }
}

pub(super) fn shimmer_class_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let carrier = pointer(ctx, o, shimmer_class::CARRIER);
    let state_carrier = carrier.u16(ctx.b, 4);
    if !(state_carrier == 0x200) {
        add_u8(ctx, o, Obj::STATE, 2);
    }
    let triggers = o.u8(ctx.b, shimmer_class::TRIGGERS);
    if !(triggers != 0) {
        return shimmer_class_player(ctx, o);
    }
    let cache = carrier.u8(ctx.b, Obj::HIDDEN);
    if !(cache == 0) {
        let flags = o.u8(ctx.b, Obj::FLAGS);
        o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
        return;
    }
    let remainder = o.u16(ctx.b, shimmer_class::DELAY).wrapping_sub(1);
    o.set_u16(ctx.b, shimmer_class::DELAY, remainder);
    if remainder != 0 {
        return;
    }
    o.set_u16(ctx.b, shimmer_class::DELAY, 0x30);
    let toggle = o.u8(ctx.b, shimmer_class::TOGGLE) ^ 1;
    o.set_u8(ctx.b, shimmer_class::TOGGLE, toggle);
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
    if toggle != 0 {
        return;
    }
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    let remainder = o.u8(ctx.b, shimmer_class::REPETITIONS).wrapping_sub(1);
    o.set_u8(ctx.b, shimmer_class::REPETITIONS, remainder);
    if remainder != 0 {
        return;
    }
    o.set_u32(ctx.b, Obj::STATE, 0x0400_0000);
}

pub(super) fn shimmer_class_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, shimmer_class::TABLE, shimmer_class_init, shimmer_class_active)
}

// ---------------------------------------------------------------------------------------------
// Type 24: character accessory (0xb0ff2)

/// Type 24: accessory stuck to its owner (`+0x50`) at an offset (dx, dy, dz) read according to the variant; active only
/// when the scene is not frozen (`A5+0x3d`). If the owner has disappeared it goes to state 6. Not exercised.
pub(super) mod accessory {
    pub const ENTRY: u32 = 0xb0ff2;
    pub const TABLE: u32 = 0xb1000;
    pub const OFFSETS: u32 = 0xb10c0;
    pub const ANIMATIONS: u32 = 0x16e752;
    pub const END_ANIMATION: u32 = 0x43;
}

/// `0xb1052`: sticks the object back to the owner.
pub(super) fn accessory_follow<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let pointer_owner = o.u16(ctx.b, field::OWNER);
    if !(pointer_owner != 0) {
        o.set_u32(ctx.b, Obj::STATE, 0x0600_0000);
        return;
    }
    let owner = pointer(ctx, o, field::OWNER);
    for off in [Obj::X, Obj::Y, Obj::Z] {
        let v = owner.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    let flip = owner.u8(ctx.b, field::FLIP);
    o.set_u8(ctx.b, field::FLIP, flip);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let rank = (variant << 3) as u32; // one-byte offset, like the original
    let mut dx = ctx.prog16(accessory::OFFSETS + rank);
    let dy = ctx.prog16(accessory::OFFSETS + rank + 2);
    let ddepth = ctx.prog16(accessory::OFFSETS + rank + 4) as i16 as i32;
    let dz = ctx.prog16(accessory::OFFSETS + rank + 6);
    if !(flip == 0) {
        dx = dx.wrapping_neg();
    }
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Y, dy);
    let y = o.u32(ctx.b, Obj::Y).wrapping_add(ddepth as u32);
    o.set_u32(ctx.b, Obj::Y, y);
    shifted_u16(ctx, o, Obj::Z, dz);
}

pub(super) fn accessory_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, accessory::TABLE, state as u32);
    match state {
        state::INIT => {
            let frozen = ctx.ram8(0x3d) != 0;
            if frozen {
                return;
            }
            add_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, accessory::END_ANIMATION, 0);
            accessory_follow(ctx, o);
            ctx.anim_start(accessory::ANIMATIONS, 3);
        }
        state::ACTIVE => {
            let frozen = ctx.ram8(0x3d) != 0;
            if frozen {
                return;
            }
            let end = o.u8(ctx.b, accessory::END_ANIMATION);
            if !(end == 0) {
                add_u8(ctx, o, Obj::STATE, 2);
            }
            accessory_follow(ctx, o);
            ctx.anim_step();
            ctx.reinsert();
        }
        state::FREED => ctx.free(&pools::EFFCT),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 4: action bubble of a character (0xaaac0)

/// Type 4: bubble (animated icon) linked to a player (`+0x50`) or to the object it handles (`+0x54`). The animation is
/// chosen from the carrier's current action (`+0x3d` of the player, `+0x10d/0x10e`, character `+3`) in small ROM
/// tables; eight variants:
///
/// - 0: player action (`+0x3d` from 0 to 32; beyond that, the original's table jumps into data);
/// - 1, 2, 5, 6: action of the carried object (`+0x54`) or fixed (`0x10`, `0x25` to `0x27`);
/// - 3: secondary bubble that settles on the carried object (`0xaadba`);
/// - 4, 7: fixed animations (`0x23`, `4`).
///
/// State 2 animates the bubble (variant 3: makes it blink and sticks it back to its carrier); state 4 frees; a table
/// code equal to `0x29` spawns a bubble of variant 3.
pub(super) mod bubble {
    pub const ENTRY: u32 = 0xaaac0;
    pub const STATES: u32 = 0xaaace;
    pub const VARIANTS: u32 = 0xaaaee;
    pub const ANIMATIONS: u32 = 0x18d53e;
    // jump tables according to the action (`+0x3d`)
    pub const ACTIONS_0: u32 = 0xaab0e;
    pub const ACTIONS_1: u32 = 0xaab40;
    pub const ACTIONS_2: u32 = 0xaab5e;
    pub const ACTIONS_5: u32 = 0xaab84;
    pub const ACTIONS_6: u32 = 0xaab9c;
    // animation tables (data)
    pub const TABLE_77: u32 = 0xab4de;
    pub const TABLE_HIT: u32 = 0xab4f6;
    pub const TABLE_CHARACTER: u32 = 0xab4f0;
    pub const TABLE_OBJECT_A: u32 = 0xab4e4;
    pub const TABLE_OBJECT_B: u32 = 0xab4ea;
    pub const TABLE_10D_A: u32 = 0xab676;
    pub const TABLE_10D_B: u32 = 0xab6a6;
    pub const TABLE_10D_C: u32 = 0xab6d6;
    /// Offsets of variant 3 (according to the carrier's class, `+0x43` and `+0x86` of the player).
    pub const OFFSETS_3: u32 = 0xaae2a;
    pub const OWNER: u32 = 0x50;
    pub const DOOR: u32 = 0x54;
    pub const END_ANIMATION: u32 = 0x43;
    pub const HIT: u32 = 0x33;
    pub const COUNTDOWN_TIMER: u32 = 0x42;
    pub const PALETTE_GAUGE: u32 = 0x91c41c;
}

/// `0xaac00`: displays animation `number` of the bubble set, then sorts the object.
pub(super) fn bubble_shown<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, number: u8) {
    if !(number != 0x2b) {
        o.set_u16(ctx.b, 0x2a, 0x115);
    }
    ctx.anim_start(bubble::ANIMATIONS, number as u16);
    ctx.reinsert();
}

/// `0xab26a`: a `0x29` code spawns a bubble of variant 3 (or, if the carrier is a BOX, plays animation 4).
pub(super) fn bubble_created_next<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let door = pointer(ctx, o, bubble::DOOR);
    let class = door.u8(ctx.b, Obj::CLASS);
    if !(class != 0xa) {
        return bubble_shown(ctx, o, 4);
    }
    o.set_u32(ctx.b, Obj::STATE, 0);
    o.set_u8(ctx.b, Obj::VARIANT, 3);
    o.set_u8(ctx.b, bubble::END_ANIMATION, 0);
    let new = ctx.alloc(&pools::EFFCT);
    if new.is_none() {
        return;
    }
    let n = new.unwrap();
    n.set_u8(ctx.b, Obj::ACTIVE, 1);
    n.set_u8(ctx.b, Obj::KIND, 4);
    let p = o.u16(ctx.b, bubble::DOOR);
    n.set_u16(ctx.b, bubble::DOOR, p);
    n.set_u8(ctx.b, Obj::VARIANT, 3);
    n.set_u8(ctx.b, bubble::END_ANIMATION, 1);
}

/// `0xaabf4`: reads the animation code in the table `table` (data) at index `index`; code `0x29` creates the
/// continuation, the others are displayed.
pub(super) fn bubble_reads<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, table: u32, index: u16) {
    let code = ctx.data8(table + index as u32);
    if code == 0x29 {
        return bubble_created_next(ctx, o);
    }
    bubble_shown(ctx, o, code);
}

/// The original jumps into another routine's epilogue (`movem.l (SP)+,{A1-A4} ; rts`, 0xaa9fe): the next four return
/// addresses are popped into A1-A4, and the return goes to the fifth. These are the callers' returns: the bubble goes
/// straight back to the top of the game loop (`0x1c468`), over the EFFCT dispatch, the world update, the end of the
/// phase and the end of the frame. Here they are the task's four most recent pending calls; the handler's return goes
/// to the fifth, and the overtaken Rust functions (dispatch loop...) give up (`tasks::table::returned`). Nobody reads
/// A1-A4 (those addresses): the task resumes in Rust, without registers.
/// Original glitch: the rest of the frame (next objects, sprites, collisions, end of frame) is skipped.
pub(super) fn foreign_epilogue<R>(ctx: &mut Cx<'_, R>) {
    crate::tasks::table::pop_calls(ctx.b, 4);
}

/// `0xaace8`: common continuation. For character 1 the original jumps into the epilogue of another routine (`movem.l
/// (SP)+,{A1-A4} ; rts`): the effect is reproduced on the stack.
pub(super) fn bubble_next<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let owner = pointer(ctx, o, bubble::OWNER);
    let character = (owner.u8(ctx.b, 3) >> 1).wrapping_sub(1);
    if character == 0 {
        return foreign_epilogue(ctx);
    }
    create_impact(ctx, o);
}

/// `0xb13fa`: creates the impact spark (type 29, variant 0) on the carried object (`+0x54`). For character 1, the
/// caller jumps into the epilogue `0xaa9fe` of another routine (see `foreign_epilogue`).
pub(super) fn create_impact<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    if let Some(n) = ctx.alloc(&pools::EFFCT) {
        n.set_u8(ctx.b, Obj::ACTIVE, 1);
        n.set_u8(ctx.b, Obj::KIND, 0x1d);
        n.set_u8(ctx.b, Obj::VARIANT, 0);
        let door = o.u16(ctx.b, bubble::DOOR);
        n.set_u16(ctx.b, 0x5c, door);
    }
}

/// `0xaacfe`: about one time out of sixteen, draws at random whether to continue (`0x5555`: even bits).
pub(super) fn bubble_random<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let rng = ctx.random();
    let draw = rng & 0xf;
    if 0x5555u16 >> draw & 1 != 0 {
        bubble_next(ctx, o);
    }
}

/// Common cases of variant 0: `0xaabc4` (byte `+0x10d` or `+0x10e` of the player in D0).
pub(super) fn bubble_0_common<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, player: Obj, d0: u8) {
    let class = player.u8(ctx.b, 3) >> 1;
    let mark = player.u8(ctx.b, 0x77);
    if !(mark == 0) {
        player.set_u8(ctx.b, 0x77, 0);
        bubble_reads(ctx, o, bubble::TABLE_77, class as u16);
        return bubble_random(ctx, o);
    }
    let index = ((d0 as u16) << 3).wrapping_add(class as u16);
    bubble_reads(ctx, o, bubble::TABLE_HIT, index);
    bubble_random(ctx, o);
}

/// `0xaabac`: animation according to `+0x10d` (or `+0x10e` for character 4).
pub(super) fn bubble_0_action_10d<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, player: Obj) {
    let mut d0 = player.u8(ctx.b, 0x10d);
    let class = player.u8(ctx.b, 3) >> 1;
    if !(class != 4) {
        d0 = player.u8(ctx.b, 0x10e);
    }
    bubble_0_common(ctx, o, player, d0);
}

/// Tables of variant 0: the player's action `0x3d` leads to one of the ten routines.
pub(super) fn bubble_0<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let player = pointer(ctx, o, bubble::OWNER);
    let action = player.u8(ctx.b, 0x3d);
    let target = target(ctx, bubble::ACTIONS_0, action as u32);
    let byte = |ctx: &mut Cx<'_, R>, off: u32| player.u8(ctx.b, off) as u16;
    let class = player.u8(ctx.b, 3) >> 1;
    match action {
        0 => bubble_0_action_10d(ctx, o, player),
        2 => {
            let d0 = player.u8(ctx.b, 0x10e);
            bubble_0_common(ctx, o, player, d0);
        }
        4 => {
            let index = byte(ctx, 0x10d);
            bubble_reads(ctx, o, bubble::TABLE_10D_A, index);
        }
        6 => {
            let index = byte(ctx, 0x10e);
            bubble_reads(ctx, o, bubble::TABLE_10D_A, index);
        }
        8 => {
            let index = byte(ctx, 0x10d);
            bubble_reads(ctx, o, bubble::TABLE_10D_C, index);
        }
        10 => {
            let index = byte(ctx, 0x10e);
            bubble_reads(ctx, o, bubble::TABLE_10D_C, index);
        }
        12 => {
            let mut index = byte(ctx, 0x10d);
            if !(class != 4) {
                index = byte(ctx, 0x10e);
            }
            bubble_reads(ctx, o, bubble::TABLE_10D_B, index);
        }
        14 => bubble_reads(ctx, o, bubble::TABLE_CHARACTER, class as u16),
        16 => {
            let door = pointer(ctx, o, bubble::DOOR);
            let is_box = door.u8(ctx.b, Obj::CLASS) == 0xa;
            if is_box {
                return bubble_0_action_10d(ctx, o, player);
            }
            let gauge_inactive = ctx.ram16(0x4be) != 0;
            if !gauge_inactive {
                crate::mem::wr16(ctx.b, bubble::PALETTE_GAUGE, 0xf0ff);
            }
            bubble_reads(ctx, o, bubble::TABLE_OBJECT_A, class as u16);
            bubble_next(ctx, o);
        }
        // 0xaacba (actions 18 and 20 share the same code)
        18 | 20 => {
            let door = pointer(ctx, o, bubble::DOOR);
            let is_box = door.u8(ctx.b, Obj::CLASS) == 0xa;
            if is_box {
                return bubble_0_action_10d(ctx, o, player);
            }
            let gauge_inactive = ctx.ram16(0x4be) != 0;
            if !gauge_inactive {
                crate::mem::wr16(ctx.b, bubble::PALETTE_GAUGE, 0xf0ff);
            }
            bubble_reads(ctx, o, bubble::TABLE_OBJECT_B, class as u16);
            bubble_next(ctx, o);
        }
        // 0xaac82
        22 => bubble_shown(ctx, o, 0x23),
        // 0xaad12: the player's gauge (`+0x77`) has just been filled
        24 | 26 | 28 | 30 | 32 => {
            let gauge = player.u8(ctx.b, 0x77) != 0;
            if gauge {
                player.set_u8(ctx.b, 0x77, 0);
                o.set_u8(ctx.b, field::MODE, 1);
                crate::mem::wr16(ctx.b, bubble::PALETTE_GAUGE, 0xff00);
                return bubble_shown(ctx, o, 0x24);
            }
            bubble_shown(ctx, o, 0x10);
        }
        _ => outside_table(target),
    }
}

/// Variants 1, 2, 5 and 6: the action `0x3d` of the carried object (`+0x54`; player for variant 1) chooses a fixed
/// animation.
pub(super) fn bubble_by_action<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, offset_pointer: u32, doubler: bool, table: u32, count: u32) {
    let source = pointer(ctx, o, offset_pointer);
    let action = source.u8(ctx.b, 0x3d) as u32;
    let index = if doubler { action * 2 } else { action };
    let target = target(ctx, table, index);
    if index >= count || (!doubler && index & 1 != 0) {
        // an action outside the table would lead the original into the code that follows it
        outside_table(target);
    }
    let code = match (table, index / if doubler { 2 } else { 1 }) {
        (bubble::ACTIONS_1, _) => 0x10,
        (_, 0) => 0x25,
        (_, 1) => 0x26,
        _ => 0x27,
    };
    bubble_shown(ctx, o, code);
}

/// Variant 3 (`0xaadba`): sticks the bubble back to its carrier.
pub(super) fn bubble_3_reattach<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let door = pointer(ctx, o, bubble::DOOR);
    let mut d0: u8 = 0;
    let class = door.u8(ctx.b, Obj::CLASS);
    if !(class == 2) {
        d0 = 0x20;
    }
    d0 = d0.wrapping_add(door.u8(ctx.b, Obj::KIND));
    let flip = door.u8(ctx.b, field::FLIP) ^ 0x20;
    o.set_u8(ctx.b, field::FLIP, flip);
    for off in [Obj::X, Obj::Z] {
        let v = door.u16(ctx.b, off);
        o.set_u16(ctx.b, off, v);
    }
    let y = door.u32(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u32(ctx.b, Obj::Y, y);
    let next = (o.u8(ctx.b, bubble::END_ANIMATION) & 1) as u16;
    let base = bubble::OFFSETS_3 + (((d0 as u16) << 4).wrapping_add(next << 3)) as u32;
    let index = ((door.u8(ctx.b, 0x86) as u32) << 2) & 0xffff;
    let mut dx = ctx.data16(base + index);
    let dz = ctx.data16(base + index + 2);
    shifted_u16(ctx, o, Obj::Z, dz);
    if !(flip != 0) {
        dx = dx.wrapping_neg();
    }
    shifted_u16(ctx, o, Obj::X, dx);
}

pub(super) fn bubble_3_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let end = o.u8(ctx.b, bubble::END_ANIMATION);
    let number = if end == 0 { 0x29 } else { 0x2a };
    ctx.anim_start(bubble::ANIMATIONS, number);
    o.set_u8(ctx.b, bubble::COUNTDOWN_TIMER, 0x1e);
    bubble_3_reattach(ctx, o);
}

pub(super) fn bubble_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, field::MODE, 0);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let target = target(ctx, bubble::VARIANTS, 2 * variant as u32);
    match variant {
        0 => bubble_0(ctx, o),
        1 => bubble_by_action(ctx, o, bubble::OWNER, false, bubble::ACTIONS_1, 6),
        2 => bubble_by_action(ctx, o, bubble::DOOR, true, bubble::ACTIONS_2, 20),
        3 => bubble_3_init(ctx, o),
        4 => bubble_shown(ctx, o, 0x23),
        5 => bubble_by_action(ctx, o, bubble::DOOR, true, bubble::ACTIONS_5, 6),
        6 => bubble_by_action(ctx, o, bubble::DOOR, true, bubble::ACTIONS_6, 6),
        7 => bubble_shown(ctx, o, 4),
        _ => outside_table(target),
    }
}

pub(super) fn bubble_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if variant == 3 {
        // variant 3: blinks, sticks back, and goes to the next state after `COUNTDOWN_TIMER` frames
        let flags = o.u8(ctx.b, Obj::FLAGS);
        o.set_u8(ctx.b, Obj::FLAGS, flags ^ 2);
        ctx.anim_step();
        bubble_3_reattach(ctx, o);
        let remainder = o.u8(ctx.b, bubble::COUNTDOWN_TIMER).wrapping_sub(1);
        o.set_u8(ctx.b, bubble::COUNTDOWN_TIMER, remainder);
        if !(remainder != 0) {
            add_u8(ctx, o, Obj::STATE, 2);
        }
        return ctx.reinsert();
    }
    let end = ctx.anim_step();
    if end {
        add_u8(ctx, o, Obj::STATE, 2);
        return;
    }
    // the background scroll gauge follows the press of the button linked to the hit (`+0x33`)
    let mut value = 0u16;
    let hit = o.u8(ctx.b, bubble::HIT);
    if !(hit == 0) {
        let gauge = ctx.ram16(0x4be) != 0;
        if !gauge {
            let blocked = ctx.ram8(0x4f) != 0;
            if !blocked {
                value = 0x100;
            }
        }
    }
    let address = crate::mem::A5.wrapping_add(0x4ba);
    crate::mem::wr16(ctx.b, address, value);
    ctx.reinsert();
}

/// Release (`0xab24c`): gives back the gauge palette if the bubble modified it.
pub(super) fn bubble_end<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let modified = o.u8(ctx.b, field::MODE);
    if !(modified == 0) {
        let gauge = ctx.ram16(0x4be) != 0;
        if !gauge {
            crate::mem::wr16(ctx.b, bubble::PALETTE_GAUGE, 0xffff);
        }
    }
    ctx.free(&pools::EFFCT);
}

pub(super) fn bubble_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, bubble::STATES, state as u32);
    match state {
        state::INIT => bubble_init(ctx, o),
        state::ACTIVE => bubble_active(ctx, o),
        state::FREED => bubble_end(ctx, o),
        6 => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

