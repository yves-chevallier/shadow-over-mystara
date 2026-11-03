//! Fields of the BOSS objects shared by several bosses (`record!`), beyond those of `Enemy` (the frame, the common
//! reactions, the AI library). Each boss declares its own fields in its own record (`Boss0`, `Boss7`...). A meaning
//! guessed from the code alone is marked (h).
//!
//! Two groups:
//! * the fields that all the bosses use with the same meaning (`+0x6b`..`+0x9a`): the parameters of the AI library as
//!   the bosses use them, and the credit of a death by a spell;
//! * the fields of the template shared by the bosses 0, 1, 4 and 6 (and, for L9b, 10, 17, 18 and 21): animation kind,
//!   saved state, timers, phase, invulnerability, mark (`+0xa0`..`+0xbd`). The other bosses lay out `+0xa0` and beyond
//!   in their own way and must not use them.

use crate::objects::Obj;
use crate::record::record;

/// A scene message (SET object created by `creators::scene_set`) that a boss waits for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Message(pub Obj);

impl Message {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Message {
        /// Non-zero: the message is over (its window closes) (h).
        done, set_done, DONE: u8 @ 0x22;
    }
}

/// A BOSS object (fields shared by the bosses; the common fields of the enemies are those of `Enemy`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Boss(pub Obj);

impl Boss {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Boss {
        /// Low byte of the whole abscissa (`Obj::X`): toggling its bit 1 shakes the boss by two pixels.
        x_low, set_x_low, X_LOW: u8 @ 0x09;
        /// Low byte of the whole height (`Obj::Z`): toggling its bit 1 jolts the boss.
        height_low, set_height_low, HEIGHT_LOW: u8 @ 0x0d;
        /// Shadow byte of the sprite (`sprites::SHADOW_B`): 1 while boss 9 rolls (h).
        shadow_b, set_shadow_b, SHADOW_B: u8 @ 0x3a;
        /// Bits of the hits that the current attack gave (bit 2: it connected; `Enemy::HIT_STRENGTH` of the hits
        /// received), cleared at the start of an attack (h).
        hit_flags, set_hit_flags, HIT_FLAGS: u8 @ 0x57;
        /// Steps (or leaps) of the current walk (`Enemy::AI_WANDER` of the AI library).
        walk_steps, set_walk_steps, WALK_STEPS: u8 @ 0x6b;
        /// Frames of the current standing wait (`Enemy::AI_WAIT`).
        idle_count, set_idle_count, IDLE_COUNT: u8 @ 0x6e;
        /// Frames between two steps of the walk (`Enemy::AI_STANCE`).
        walk_delay, set_walk_delay, WALK_DELAY: u8 @ 0x71;
        /// Kind of the current shot or approach (`Enemy::AI_POINT_SET`).
        shot, set_shot, SHOT: u8 @ 0x79;
        /// Frames on the ground before getting up (word; `Enemy::RECOVER` of the reactions).
        down_time, set_down_time, DOWN_TIME: u16 @ 0x84;
        /// Countdown of the shaking in place (reaction 0x4c; `Enemy::COUNTER` of the reactions).
        shake, set_shake, SHAKE: u8 @ 0x90;
        /// Non-zero: a player who kills the boss with a spell is credited with `DEATH_VALUE` (`player::peff_*`).
        death_credit, set_death_credit, DEATH_CREDIT: u8 @ 0x93;
        /// Value credited by a death by a spell (0xff for the bosses).
        death_value, set_death_value, DEATH_VALUE: u8 @ 0x9a;
        /// Element the boss is weak to (damage + 4) and element it resists (damage halved) (`combat::damage`).
        weak_element, set_weak_element, WEAK_ELEMENT: u8 @ 0x9e;
        resisted_element, set_resisted_element, RESISTED_ELEMENT: u8 @ 0x9f;
        /// Template: kind of the animation requested and of the last one started (0xff: none).
        anim_kind, set_anim_kind, ANIM_KIND: u8 @ 0xa0;
        anim_last, set_anim_last, ANIM_LAST: u8 @ 0xa1;
        /// Template: state (long `+4..+7`) to resume after an about-face.
        saved_state, set_saved_state, SAVED_STATE: u32 @ 0xa2;
        /// Template: countdowns of the steps (words).
        timer, set_timer, TIMER: u16 @ 0xa6;
        timer2, set_timer2, TIMER2: u16 @ 0xa8;
        /// Template: phase (crossed HP thresholds, `enemy_lib::next_threshold`) and number of phases.
        phase, set_phase, PHASE: u8 @ 0xaa;
        phases, set_phases, PHASES: u8 @ 0xab;
        /// Template: non-zero, the boss is invulnerable (`+0x38` cleared every frame).
        invulnerable, set_invulnerable, INVULNERABLE: u8 @ 0xad;
        /// Countdown of the vertical oscillation (`boss_common::bob`, word; bosses 4, 9 and 13).
        bob_timer, set_bob_timer, BOB_TIMER: u16 @ 0xa8;
        /// Template: X position aimed at by a charge (word).
        mark_x, set_mark_x, MARK_X: u16 @ 0xbc;
    }
}
