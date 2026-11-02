//! Handlers of the enemy types of the ENEMY class (C02). See `docs/families/enemy.md`.
//!
//! Same framework as the ESHL / SET / SOBJ families (`cx.rs`): a type is a `mod` with a `run` function, named constants
//! for its tables and animation sets, and a role comment. The role names are hypotheses (h) drawn from the behaviour
//! and the debug names of the game.

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::mem::wr16;
use crate::ram::PLAYERS;
use crate::data::RomAddr;
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;

// ---------------------------------------------------------------------------------------------
// Type 20 (0x74fc0): "ManScorpion" (h), composite enemy grafted on an enemy of type 19
// ---------------------------------------------------------------------------------------------

/// Created by an enemy of type 19 (parent in `+0x50`), it spawns its halos (ESHL type 11 in `+0xa4/+0xa6`), a shadow
/// (EFFCT 0x20 in `+0xb4`), two flames (ITEM type 6 in `+0xa8/+0xaa`) and, for variant 1, two arms (ENEMY type 19,
/// `+0xac/+0xae`, `+0xb0` = arms present). Variants 0 and 1: flight toward the left edge throwing sparks; variant 2:
/// flight toward the right edge; when the arms have fallen the parent is notified. States: 0 creation, 2 flight, 4 hit
/// (15-frame recoil then death in several steps), 6 release of the parts.
mod man_scorpion {
    use super::*;

    const ANIMS: RomAddr = RomAddr(0x169fb0);
    const PART_TYPE: u8 = 0xb;
    const ARM_TYPE: u8 = 0x13;
    const SHADOW_TYPE: u8 = 0x20;
    const FLAME_TYPE: u8 = 6;
    /// Long state of a dropped arm (state 2, substate 6, sub-substate 0xc).
    const ARM_RELEASED: u32 = 0x0206_0c00;
    /// Hit, death in progress (state 4, substate 2).
    const DYING: u32 = 0x0402_0600;

    /// Fields specific to this type.
    mod p {
        pub const LIMBS: u32 = 0xa4;
        pub const FLAMES: u32 = 0xa8;
        pub const ARMS: u32 = 0xac;
        pub const SHADOW: u32 = 0xb4;
        pub const ARMS_MASK: u32 = 0xb0;
        pub const COUNTER: u32 = 0xb2;
        pub const DELAY: u32 = 0xa2;
        pub const HIT_FREEZE: u32 = 0x56;
        /// Non-zero once the parent has been told of the end of the parts (h).
        pub const TOLD: u32 = 0xb1;
        /// Rank of an arm among the pieces (`+0xbc` of the type 19).
        pub const ARM_RANK: u32 = 0xbc;
    }

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(f::STATE) {
            st::INIT => init(x),
            st::ACTIVE => active(x),
            st::HIT => hit(x),
            st::DEAD => die(x),
            _ => {}
        }
    }

    // --- creation ------------------------------------------------------------------------------

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.set32(f::IMMUNITY, 0xffff_ffff);
        x.set8(Enemy::EVENT_KIND, 0xb);
        let r = x.random() as u32;
        x.set8(p::COUNTER, r as u8);
        // at least two free ESHL and ITEM slots, and fewer than two free ENEMY slots or a free EFFCT slot
        let free = |x: &mut Cx<'_, R>, pool: &pools::Pool| pool.free_count(x.b) as i16;
        if free(x, &pools::ESHL) < 2 || free(x, &pools::ITEM) < 2 {
            return;
        }
        if !(free(x, &pools::ENEMY) < 2 || free(x, &pools::EFFCT) >= 1) {
            return;
        }
        match x.u8(Enemy::VARIANT) {
            0 => spawn(x, 0, false),
            1 => spawn(x, 0, true),
            2 => spawn(x, 2, false),
            _ => {}
        }
        x.set16(Enemy::AX, 0);
        x.set8(f::FLIP, 0);
        x.set8(f::BOX_SET, 0x3b);
        x.set16(Enemy::HEIGHT, 4);
        lib::set_immunities(x);
        x.anim_start(ANIMS.0, 0);
        lib::initial_hp(x);
        lib::sound(x, crate::sound::CMD_92_D);
    }

    /// Creates the parts; `flame_kind` is the variant of the flames; variant 1 also creates the arms.
    fn spawn<R: Bench>(x: &mut Cx<'_, R>, flame_kind: u8, with_arms: bool) {
        spawn_limbs(x);
        spawn_shadow(x);
        spawn_flames(x, flame_kind);
        if with_arms {
            let mut at = x.o.0 + p::ARMS;
            for d5 in [1u8, 0] {
                let o = alloc(x, &pools::ENEMY);
                let arm = Enemy(o);
                let active = x.u8(Enemy::ACTIVE);
                arm.set_active(x.b, active);
                o.set_u8(x.b, p::ARM_RANK, d5);
                arm.set_variant(x.b, 0);
                arm.set_group(x.b, 1);
                arm.set_kind(x.b, ARM_TYPE);
                o.set_u16(x.b, f::CREATOR, x.o.0 as u16);
                wr16(x.b, at, o.0 as u16);
                at += 2;
            }
            x.set8(p::ARMS_MASK, 3);
            x.add8(f::STATE, 2);
            return;
        }
        let parent = Enemy(x.linked(f::OWNER));
        let v = parent.parent_hits(x.b).wrapping_add(2);
        parent.set_parent_hits(x.b, v);
        x.add8(f::STATE, 2);
        x.set16(Enemy::VX, 0xfb00);
    }

    /// Two halos (ESHL type 11, variants 1 then 0) stored in `+0xa4`, `+0xa6`.
    fn spawn_limbs<R: Bench>(x: &mut Cx<'_, R>) {
        let mut at = x.o.0 + p::LIMBS;
        for d5 in [1u8, 0] {
            let o = Enemy(alloc(x, &pools::ESHL));
            let active = x.u8(Enemy::ACTIVE);
            o.set_active(x.b, active);
            o.set_variant(x.b, d5);
            o.set_kind(x.b, PART_TYPE);
            o.set_target(x.b, x.o.0 as u16);
            let o = o.0;
            wr16(x.b, at, o.0 as u16);
            at += 2;
        }
    }

    fn spawn_shadow<R: Bench>(x: &mut Cx<'_, R>) {
        let o = Enemy(alloc(x, &pools::EFFCT));
        let active = x.u8(Enemy::ACTIVE);
        o.set_active(x.b, active);
        o.set_kind(x.b, SHADOW_TYPE);
        o.set_target(x.b, x.o.0 as u16);
        let o = o.0;
        x.set16(p::SHADOW, o.0 as u16);
    }

    /// Two flames (ITEM type 6, orientation 0x20 then 0) stored in `+0xa8`, `+0xaa`.
    fn spawn_flames<R: Bench>(x: &mut Cx<'_, R>, kind: u8) {
        let mut at = x.o.0 + p::FLAMES;
        for d5 in [1u8, 0] {
            let o = alloc(x, &pools::ITEM);
            let flame = Enemy(o);
            let active = x.u8(Enemy::ACTIVE);
            flame.set_active(x.b, active);
            flame.set_flip(x.b, d5 << 5);
            flame.set_kind(x.b, FLAME_TYPE);
            flame.set_target(x.b, x.o.0 as u16);
            flame.set_variant(x.b, kind);
            let parent = x.u16(f::OWNER);
            o.set_u16(x.b, f::HOST, parent);
            wr16(x.b, at, o.0 as u16);
            at += 2;
        }
    }

    // --- state 2: flight -----------------------------------------------------------------------

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        check_arms(x);
        if !paused(x) {
            match (x.u8(Enemy::VARIANT), x.u8(f::SUB)) {
                (0, 0) | (1, _) => {
                    if x.u8(Enemy::VARIANT) == 0 && x.u8(p::ARMS_MASK) == 0 {
                        stop_flight(x);
                    } else {
                        fly_left(x);
                    }
                }
                (2, 0) => {
                    if x.u8(p::ARMS_MASK) == 0 {
                        stop_flight(x);
                    } else {
                        fly_right(x);
                    }
                }
                (0, 2) | (2, 2) => take_from_parent(x),
                _ => {}
            }
        }
        x.reinsert();
    }

    /// No more arms: substate 2, then attempt to switch to the "death in progress" state.
    fn stop_flight<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(f::SUB, 2);
        detach_from_host(x);
        take_from_parent(x);
    }

    /// The finishing blow is possible only while the parent has hits in reserve (`+0x32`).
    fn take_from_parent<R: Bench>(x: &mut Cx<'_, R>) {
        let parent = Enemy(x.linked(f::OWNER));
        let left = parent.parent_reserve(x.b);
        if left == 0 {
            x.anim_step();
            return;
        }
        parent.set_parent_reserve(x.b, left - 1);
        x.set32(f::STATE, DYING);
    }

    /// Sparks: every 8 steps, every 16 if there is only one arm.
    fn sparkles<R: Bench>(x: &mut Cx<'_, R>) {
        let mask = x.u8(p::ARMS_MASK);
        let one_arm = (mask & 1 != 0) != (mask & 2 != 0);
        let step = if one_arm { 0xf } else { 7 };
        let n = x.u8(p::COUNTER).wrapping_add(1) & step;
        x.set8(p::COUNTER, n);
        if n == 0 {
            lib::shock(x, 0, 0x201, 0x20, 0x10);
        }
    }

    /// Flight toward the left edge: death (state 6) once off the screen.
    fn fly_left<R: Bench>(x: &mut Cx<'_, R>) {
        sparkles(x);
        x.move_x();
        let edge = var::SCROLL_X.get(x.b).wrapping_sub(0x50) as i16;
        if edge > x.u16(Enemy::X) as i16 {
            x.set32(f::STATE, st::DEAD_LONG);
        }
        x.anim_step();
    }

    /// Flight toward the right edge; on arrival, drops the arms and cedes control to the parent.
    fn fly_right<R: Bench>(x: &mut Cx<'_, R>) {
        sparkles(x);
        x.move_x();
        let dx = x.u16(Enemy::X).wrapping_sub(var::SCROLL_X.get(x.b));
        if dx < 0x100 {
            let mask = x.u8(p::ARMS_MASK);
            for (bit, off) in [(1u8, p::ARMS), (2, p::ARMS + 2)] {
                if mask & bit != 0 {
                    let arm = Enemy(x.linked(off));
                    arm.set_state_long(x.b, ARM_RELEASED);
                    arm.set_active(x.b, 2);
                }
            }
            x.set8(p::ARMS_MASK, 0);
            Enemy(x.linked(f::OWNER)).set_parent_wait(x.b, 0);
            x.set32(f::STATE, 0x0202_0000);
        }
        x.anim_step();
    }

    /// An arm whose state is >= 4 is detached; with no arms left, the parent is released from its wait (`+0x24`).
    /// Leaves D0 = 0 like the original.
    fn check_arms<R: Bench>(x: &mut Cx<'_, R>) {
        let mut left = x.u8(p::ARMS_MASK);
        if left != 0 {
            if left & 1 != 0 {
                let arm = x.linked(p::ARMS);
                if (arm.u8(x.b, f::STATE) as i8) >= 4 {
                    x.set16(p::ARMS, 0);
                    x.set_bit(p::ARMS_MASK, 0, false);
                    left &= !1;
                    x.set16(Enemy::VX, 0xfe80);
                }
            }
            if x.u8(p::ARMS_MASK) & 2 != 0 {
                let arm = x.linked(p::ARMS + 2);
                if (arm.u8(x.b, f::STATE) as i8) < 4 {
                    return;
                }
                x.set16(p::ARMS + 2, 0);
                x.set_bit(p::ARMS_MASK, 1, false);
                left &= !2;
                x.set16(Enemy::VX, 0xfe80);
            }
            if left == 0 {
                Enemy(x.linked(f::OWNER)).set_parent_wait(x.b, 0);
            }
        }
    }

    // --- state 4: hit --------------------------------------------------------------------------

    fn hit<R: Bench>(x: &mut Cx<'_, R>) {
        check_arms(x);
        let freeze = x.u8(f::FREEZE) as i8;
        if freeze > 0 {
            return x.set8(f::FREEZE, (freeze - 1) as u8);
        }
        match x.u8(f::SUB) {
            0 => {
                match x.u8(f::SUB2) {
                    0 => {
                        x.add8(f::SUB2, 2);
                        if (x.u16(f::HP) as i16) > 0 {
                            react(x);
                        } else {
                            x.set8(f::SUB, 2);
                            x.set8(f::SUB2, 0);
                            x.set8(Enemy::ACTIVE, 2);
                            dying(x);
                        }
                    }
                    2 => recoil(x),
                    _ => {}
                }
                x.reinsert();
            }
            2 => dying(x),
            _ => {}
        }
    }

    /// Copies the freeze (`+0x58` long, `+0x53`) onto a part.
    fn share_freeze<R: Bench>(x: &mut Cx<'_, R>, o: Obj) {
        let (l, g) = (x.u32(Enemy::HIT_LOCK), x.u8(f::FREEZE));
        Enemy(o).set_hit_lock(x.b, l);
        Enemy(o).set_freeze(x.b, g);
    }

    /// Start of a hit: freezes the parts, starts the recoil (15 frames) and the `0xab7d8` effect.
    fn react<R: Bench>(x: &mut Cx<'_, R>) {
        let freeze = x.u8(p::HIT_FREEZE);
        x.set8(f::FREEZE, freeze);
        x.set8(p::DELAY, 0xf);
        let (a1, a2) = (x.linked(p::LIMBS), x.linked(p::LIMBS + 2));
        share_freeze(x, a2);
        share_freeze(x, a1);
        a1.set_u32(x.b, f::STATE, 0x0400_0000);
        a2.set_u32(x.b, f::STATE, 0x0400_0000);
        let mask = x.u8(p::ARMS_MASK);
        for (bit, off) in [(1u8, p::ARMS), (2, p::ARMS + 2)] {
            if mask & bit != 0 {
                let arm = x.linked(off);
                arm.set_u8(x.b, f::FREEZE, freeze);
            }
        }
        lib::debris_spray(x, 0);
    }

    /// 15-frame recoil (2 pixels, alternating direction), then back to state 2.
    fn recoil<R: Bench>(x: &mut Cx<'_, R>) {
        let left = x.u8(p::DELAY);
        if left != 0 {
            let left = left - 1;
            x.set8(p::DELAY, left);
            let mut d = 2i16;
            if left & 1 != 0 {
                d = -d;
            }
            if x.u8(f::FLIP) != 0 {
                d = -d;
            }
            x.add16(Enemy::X, d as u16);
            return;
        }
        x.set8(Enemy::ACTIVE, 1);
        x.set32(Enemy::HIT_LOCK, 0);
        x.set32(f::STATE, 0x0200_0000);
        for off in [p::LIMBS, p::LIMBS + 2] {
            let o = Enemy(x.linked(off));
            o.set_hit_lock(x.b, 0);
            o.set_state_long(x.b, 0x0200_0000);
        }
    }

    /// Substate 2: death in several steps (`+6`: 0 starts the death of a variant 0, 6 that of another variant; 2 and 8
    /// wait; 4 and 10 finish).
    fn dying<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(f::SUB2) {
            0 => death_start(x),
            2 | 8 => wait(x),
            4 => death_end(x),
            6 => death_start_b(x),
            10 => death_end_b(x),
            _ => {}
        }
    }

    fn death_start<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(f::SUB2, 2);
        react(x);
        if x.u8(Enemy::VARIANT) == 0 {
            let parent = Enemy(x.linked(f::OWNER));
            let v = parent.parent_deaths(x.b).wrapping_add(1);
            parent.set_parent_deaths(x.b, v);
            parent.set_parent_wait(x.b, 0);
        }
        lib::sound(x, crate::sound::CMD_131_231);
        for off in [p::LIMBS, p::LIMBS + 2, p::SHADOW] {
            let o = x.linked(off);
            o.set_u32(x.b, f::STATE, st::DEAD_LONG);
        }
        for off in [p::FLAMES, p::FLAMES + 2] {
            let o = x.linked(off);
            o.set_u32(x.b, f::STATE, 0x0202_0000);
        }
        for off in [p::ARMS, p::ARMS + 2] {
            if x.u16(off) != 0 {
                Enemy(x.linked(off)).set_counter(x.b, 1);
            }
        }
        detach_from_host(x);
        x.set8(p::DELAY, 0x5a);
        x.anim_start(ANIMS.0, 2);
        x.reinsert();
    }

    fn wait<R: Bench>(x: &mut Cx<'_, R>) {
        let n = x.u8(p::DELAY).wrapping_sub(1);
        x.set8(p::DELAY, n);
        if n == 0 {
            x.add8(f::SUB2, 2);
            x.set8(p::DELAY, 0x28);
        }
        x.reinsert();
    }

    fn death_end<R: Bench>(x: &mut Cx<'_, R>) {
        let n = x.u8(p::DELAY).wrapping_sub(1);
        x.set8(p::DELAY, n);
        if n != 0 {
            return blink(x);
        }
        if x.u8(Enemy::VARIANT) == 0 {
            Enemy(x.linked(f::OWNER)).set_parent_done(x.b, 0xff);
        }
        x.set32(f::STATE, st::DEAD_LONG);
        lib::object_event(x);
        free(x);
    }

    fn death_start_b<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(f::SUB2, 2);
        lib::sound(x, crate::sound::CMD_131_231);
        let parent = Enemy(x.linked(f::OWNER));
        let v = parent.parent_deaths(x.b).wrapping_add(1);
        parent.set_parent_deaths(x.b, v);
        parent.set_parent_wait(x.b, 0);
        for off in [p::LIMBS, p::LIMBS + 2, p::SHADOW] {
            let o = x.linked(off);
            o.set_u32(x.b, f::STATE, st::DEAD_LONG);
        }
        for off in [p::FLAMES, p::FLAMES + 2] {
            let o = x.linked(off);
            let s = o.u8(x.b, f::SUB).wrapping_add(2);
            o.set_u8(x.b, f::SUB, s);
        }
        x.set8(p::DELAY, 0x5a);
        x.anim_start(ANIMS.0, 2);
        x.reinsert();
    }

    fn death_end_b<R: Bench>(x: &mut Cx<'_, R>) {
        let n = x.u8(p::DELAY).wrapping_sub(1);
        x.set8(p::DELAY, n);
        if n != 0 {
            return blink(x);
        }
        Enemy(x.linked(f::OWNER)).set_parent_done(x.b, 0xff);
        x.set32(f::STATE, st::DEAD_LONG);
        free(x);
    }

    /// Blinks while waiting (hidden when `(+0xa2) & 3 != 0`), then puts the object back in the list.
    fn blink<R: Bench>(x: &mut Cx<'_, R>) {
        let hidden = x.u8(p::DELAY) & 3 != 0;
        x.set_bit(Enemy::DISPLAY, 1, hidden);
        x.reinsert();
    }

    // --- state 6: release ----------------------------------------------------------------------

    /// Releases the parts; variant 0 also gives its counters back to the parent and to the host.
    fn die<R: Bench>(x: &mut Cx<'_, R>) {
        let first = x.u8(Enemy::VARIANT) == 0;
        let mut host = None;
        if first {
            let h = x.linked(f::HOST);
            add16(x, h, Enemy::PARTS, 0xffff);
            host = Some(h);
            let parent = x.linked(f::OWNER);
            add8(x, parent, Enemy::PARENT_COUNT_20, 0xff);
            Enemy(parent).set_parent_wait(x.b, 0);
            add8(x, parent, Enemy::PARENT_HITS, 0xfe);
            let flag = Enemy(parent).parent_done(x.b);
            let v = x.u8(p::TOLD) ^ flag;
            x.set8(p::TOLD, v);
            if v != 0 {
                Enemy(parent).set_parent_done(x.b, 0xff);
            }
        }
        for off in [p::LIMBS, p::LIMBS + 2, p::SHADOW, p::FLAMES, p::FLAMES + 2] {
            let o = x.linked(off);
            o.set_u32(x.b, f::STATE, st::DEAD_LONG);
        }
        let mask = x.u8(p::ARMS_MASK);
        for (bit, off) in [(1u8, p::ARMS), (2, p::ARMS + 2)] {
            if mask & bit != 0 {
                let o = x.linked(off);
                o.set_u32(x.b, f::STATE, st::DEAD_LONG);
                if let Some(h) = host {
                    add16(x, h, Enemy::PARTS, 0xffff);
                }
            }
        }
        free(x);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 28 (0x79300): "Nagpa" (h), scene character that appears when a player approaches
// ---------------------------------------------------------------------------------------------

/// Hidden at the start (bit 1 of `+1`), it shows itself when a living player comes within 0x30 pixels (0x50 for variant
/// 1), plays its appearance animation, then becomes hidden again when no one is in range any more. The variant's bit in
/// byte `0xff2064` (`A5-0x5f9c`, bit 1 variant 0, bit 0 variant 1) indicates that it is visible. Substates (`+5`): 0
/// wait, 2 appearance, 4 visible, 6 disappearance.
mod nagpa {
    use super::*;

    const ANIMS: RomAddr = RomAddr(0x16c7fc);

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(f::STATE) {
            st::INIT => init(x),
            st::ACTIVE => active(x),
            st::HIT | st::DEAD => free(x),
            _ => {}
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        let variant = x.u8(Enemy::VARIANT);
        if variant > 1 {
            return;
        }
        x.add8(f::STATE, 2);
        x.set_bit(Enemy::DISPLAY, 1, true);
        x.set8(f::BOX_SET, 0x47);
        lib::set_immunities(x);
        x.anim_start(ANIMS.0, (variant as u32) as u16);
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        if paused(x) {
            return;
        }
        match x.u8(f::SUB) {
            0 => wait_for_player(x),
            2 => appear(x),
            4 => stay(x),
            6 => disappear(x),
            _ => {}
        }
        x.reinsert();
    }

    /// Is a living player within range? (D0 = 1 if so in the original)
    fn player_near<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        let range: u16 = if x.u8(Enemy::VARIANT) == 0 { 0x30 } else { 0x50 };
        for player in PLAYERS.iter() {
            if player.active(x.b) == 1 && lib::in_box(x, player, range as i16, 0x14, 0xffe8).found() {
                return true;
            }
        }
        false
    }

    fn mark_visible<R: Bench>(x: &mut Cx<'_, R>, on: bool) {
        let bit = x.u8(Enemy::VARIANT) != 0;
        let v = var::NAGPA_VISIBLE.get(x.b);
        let mask = if bit { 1 } else { 2 };
        var::NAGPA_VISIBLE.set(x.b, if on { v | mask } else { v & !mask });
    }

    fn wait_for_player<R: Bench>(x: &mut Cx<'_, R>) {
        if !player_near(x) {
            return;
        }
        lib::sound(x, crate::sound::CMD_34C_44C);
        mark_visible(x, true);
        x.add8(f::SUB, 2);
        x.set_bit(Enemy::DISPLAY, 1, false);
        // the byte that the reactions use as `DOWNED`
        x.set8(Enemy::DOWNED, 1);
    }

    fn appear<R: Bench>(x: &mut Cx<'_, R>) {
        if x.anim_step() {
            x.add8(f::SUB, 2);
        }
    }

    fn stay<R: Bench>(x: &mut Cx<'_, R>) {
        if player_near(x) {
            return;
        }
        x.add8(f::SUB, 2);
        lib::sound(x, crate::sound::CMD_34C_44C);
        mark_visible(x, false);
        let n = 2 + (x.u8(Enemy::VARIANT) != 0) as u32;
        x.anim_start(ANIMS.0, n as u16);
    }

    fn disappear<R: Bench>(x: &mut Cx<'_, R>) {
        if !x.anim_step() {
            return;
        }
        x.set8(Enemy::DOWNED, 0);
        x.add8(f::SUB, 0xfa);
        x.set_bit(Enemy::DISPLAY, 1, true);
        let n = (x.u8(Enemy::VARIANT) != 0) as u32;
        x.anim_start(ANIMS.0, n as u16);
    }
}

handlers! {
    0x74fc0 "enemy_20" man_scorpion::run,
    0x79300 "enemy_28" nagpa::run,
}
