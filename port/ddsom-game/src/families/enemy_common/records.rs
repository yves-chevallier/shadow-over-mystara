//! Fields of the ENEMY objects (and of the bosses, which share the framework and the common reactions), declared once
//! ([`record!`]): the offsets (`Enemy::REACTION`) and the typed accessors (`enemy.reaction(b)`). The fields common to
//! all objects (position, speeds, animation...) are those of `Obj`. A meaning guessed from the code alone is marked
//! (h). Names from docs/familles/enemy.md, docs/combat.md and the families.
//!
//! The families (`enemy_*/`) declare their own fields (`+0xa0` and beyond, the AI parameters they use) in their own
//! records; the fields below are those of the framework, of the common reactions and of the AI library.

use crate::objects::Obj;
use crate::record::record;

/// An ENEMY or BOSS object.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Enemy(pub Obj);

impl Enemy {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Enemy {
        /// Pool slot: 0 free, 1 active, 2 active without being hit again (during a reaction, a death).
        active, set_active, ACTIVE: u8 @ 0x00;
        /// Display flags: bit 1 hidden (blinking).
        display, set_display, DISPLAY: u8 @ 0x01;
        /// Type of the object in its class.
        kind, set_kind, KIND: u8 @ 0x02;
        /// Variant (arrival, colour, role).
        variant, set_variant, VARIANT: u8 @ 0x03;
        /// Main state: 0 creation, 2 active, 4 hit, 6 dead (`st`).
        state, set_state, STATE: u8 @ 0x04;
        /// The four state bytes as a long (`st::DEAD_LONG`...).
        state_long, set_state_long, STATE_LONG: u32 @ 0x04;
        /// Substate (group of the active state; in the hit state, 6 = death).
        substate, set_substate, SUB: u8 @ 0x05;
        /// State and substate as a word (`0x205`: active state, group 5).
        state_word, set_state_word, STATE_WORD: u16 @ 0x04;
        /// Step of the substate (the steps of a reaction routine, by 2).
        step, set_step, STEP: u8 @ 0x06;
        /// Step and sub-step as a word.
        step_word, set_step_word, STEP_WORD: u16 @ 0x06;
        /// Sub-step.
        sub_step, set_sub_step, SUB_STEP: u8 @ 0x07;
        /// Abscissa, whole pixels (high word of `Obj::X`).
        x, set_x, X: u16 @ 0x08;
        /// Fraction of the abscissa; the held reaction keeps the abscissa there while it shakes (h).
        x_saved, set_x_saved, X_SAVED: u16 @ 0x0a;
        /// Height above the ground, whole pixels (high word of `Obj::Z`).
        height, set_height, HEIGHT: u16 @ 0x0c;
        /// Height as a long (16.16).
        height_long, set_height_long, HEIGHT_LONG: u32 @ 0x0c;
        /// Depth, whole pixels (high word of `Obj::Y`).
        y, set_y, Y: u16 @ 0x10;
        /// X speed (8.8); as a long with the X acceleration.
        vx, set_vx, VX: u16 @ 0x14;
        vx_long, set_vx_long, VX_LONG: u32 @ 0x14;
        /// X acceleration (friction of the slides).
        ax, set_ax, AX: u16 @ 0x16;
        /// The X acceleration and the height speed as a long (cleared together at the get-up).
        ax_long, set_ax_long, AX_LONG: u32 @ 0x16;
        /// Height speed; as a long with the gravity.
        vz, set_vz, VZ: u16 @ 0x18;
        vz_long, set_vz_long, VZ_LONG: u32 @ 0x18;
        /// Gravity (height acceleration).
        gravity, set_gravity, GRAVITY: u16 @ 0x1a;
        /// Depth speed; as a long with the depth acceleration.
        vy, set_vy, VY: u16 @ 0x1c;
        vy_long, set_vy_long, VY_LONG: u32 @ 0x1c;
        /// Depth acceleration.
        ay, set_ay, AY: u16 @ 0x1e;
        /// Counter kept by the composite enemies on their parent (h).
        parent_count_20, set_parent_count_20, PARENT_COUNT_20: u8 @ 0x20;
        /// Parts that died, counted on the parent of a composite enemy (h).
        parent_deaths, set_parent_deaths, PARENT_DEATHS: u8 @ 0x21;
        /// Non-zero: the parent of a composite enemy waits for its parts (h).
        parent_wait, set_parent_wait, PARENT_WAIT: u8 @ 0x24;
        /// 0xff: a part of a composite enemy ended (on its parent) (h).
        parent_done, set_parent_done, PARENT_DONE: u8 @ 0x26;
        /// Class (2 ENEMY, 4 BOSS).
        class, set_class, CLASS: u8 @ 0x28;
        /// Non-zero: the last move hit an obstacle of the scenery.
        blocked, set_blocked, BLOCKED: u8 @ 0x29;
        /// Non-zero: the object is drawn with its own palette `PALETTE`.
        own_palette, set_own_palette, OWN_PALETTE: u8 @ 0x2a;
        /// Parts still attached to a host (word, decreased by `detach_from_host`).
        parts, set_parts, PARTS: u16 @ 0x2a;
        /// Palette of the object when `OWN_PALETTE` is set.
        palette, set_palette, PALETTE: u8 @ 0x2b;
        /// Orientation: 0 faces right, 0x20 faces left.
        flip, set_flip, FLIP: u8 @ 0x2c;
        /// Angle of a whirl (0 to 31), also the heading `Obj::SPEED_INDEX`.
        angle, set_angle, ANGLE: u8 @ 0x2d;
        /// Collision box set (docs/combat.md §3).
        box_set, set_box_set, BOX_SET: u8 @ 0x2e;
        /// Group or series of the object (AI profile, list, rank of a piece).
        group, set_group, GROUP: u8 @ 0x2f;
        /// Kept hits of the parent of a composite enemy (h).
        parent_hits, set_parent_hits, PARENT_HITS: u8 @ 0x30;
        /// Hits in reserve of the parent of a composite enemy (h).
        parent_reserve, set_parent_reserve, PARENT_RESERVE: u8 @ 0x32;
        /// End of the animation: negative at the last entry (word; flags of `Obj::ANIM_FLAGS`).
        anim_end, set_anim_end, ANIM_END: u16 @ 0x32;
        /// High byte of the animation flags (negative at the last entry).
        anim_end_byte, set_anim_end_byte, ANIM_END_BYTE: u8 @ 0x32;
        /// Key-frame mark of the current animation entry (bit 7: event).
        key_frame, set_key_frame, KEY_FRAME: u8 @ 0x33;
        /// First byte of the frame data (h).
        frame_tag, set_frame_tag, FRAME_TAG: u8 @ 0x38;
        /// Strike key of the animation entry (copied to `STRIKE_KIND` by `enemy_lib`).
        strike_key, set_strike_key, STRIKE_KEY: u8 @ 0x3e;
        /// Creator of a sub-object (word, RAM address).
        creator, set_creator, CREATOR: u16 @ 0x42;
        /// Strike type and strength of the current attack (docs/combat.md).
        strike_kind, set_strike_kind, STRIKE_KIND: u8 @ 0x44;
        strike_power, set_strike_power, STRIKE_POWER: u8 @ 0x45;
        /// Host of a part of a composite enemy (word, RAM address).
        host, set_host, HOST: u16 @ 0x46;
        /// Reaction code stored by the collision (index ×4 of the reaction tables).
        reaction, set_reaction, REACTION: u8 @ 0x49;
        /// Orientation of the attacker at the hit.
        attacker_flip, set_attacker_flip, ATTACKER_FLIP: u8 @ 0x4a;
        /// Target (a player) or owner (word, RAM address).
        target, set_target, TARGET: u16 @ 0x50;
        /// Owner or target as a long (the long `+0x50` of the shots).
        target_long, set_target_long, TARGET_LONG: u32 @ 0x50;
        /// Freeze counter after a hit (decreased by the end-of-handler counters).
        freeze, set_freeze, FREEZE: u8 @ 0x53;
        /// Object that holds or carries this one (word; its sub-step 4 releases a hop) (h).
        holder, set_holder, HOLDER: u16 @ 0x54;
        /// Stun duration given by the hit: copied to `FREEZE`.
        stun, set_stun, STUN: u8 @ 0x56;
        /// Hit lock (long) cleared at the end of a reaction (h).
        hit_lock, set_hit_lock, HIT_LOCK: u32 @ 0x58;
        /// Creator that a created object reports to (word, RAM address).
        spawner, set_spawner, SPAWNER: u16 @ 0x5c;
        /// Hit points at the creation, current hit points (signed word), hit points at the last hit.
        hp_max, set_hp_max, HP_MAX: u16 @ 0x60;
        hp, set_hp, HP: u16 @ 0x62;
        hp_last, set_hp_last, HP_LAST: u16 @ 0x64;
        /// Non-zero: can be hit again (cleared at the start of a reaction).
        vulnerable, set_vulnerable, VULNERABLE: u8 @ 0x67;
        /// AI: get-up parameter set by `enemy_lib::set_68`.
        ai_68, set_ai_68, AI_68: u8 @ 0x68;
        /// AI: row of the chance of a shot or a special gesture (`enemy_lib::roll_78`; 0xff: none).
        ai_shot, set_ai_shot, AI_SHOT: u8 @ 0x78;
        /// Type of the event of the object (`enemy_lib::object_event`).
        event_kind, set_event_kind, EVENT_KIND: u8 @ 0x7e;
        /// Cleared by the disintegration (h).
        flag_80, set_flag_80, FLAG_80: u8 @ 0x80;
        flag_81, set_flag_81, FLAG_81: u8 @ 0x81;
        /// Hits before the end of a grab (word).
        recover, set_recover, RECOVER: u16 @ 0x84;
        /// Non-zero: the object was down (lying on the ground); 0 makes it not grabbable for some types.
        downed, set_downed, DOWNED: u8 @ 0x86;
        /// Non-zero: grabbed by a player.
        grabbed, set_grabbed, GRABBED: u8 @ 0x87;
        /// State to resume after a slide (long; 0: none).
        resume_state, set_resume_state, RESUME_STATE: u32 @ 0x8a;
        /// Word of the state to resume (`0x205`: active, group 5).
        resume_word, set_resume_word, RESUME_WORD: u16 @ 0x8a;
        /// Counter of the reaction steps.
        counter, set_counter, COUNTER: u8 @ 0x90;
        /// Palette to take at the next reaction (bit 7: pending; becomes `PALETTE`, `OWN_PALETTE` set).
        pending_palette, set_pending_palette, PENDING_PALETTE: u8 @ 0x91;
        /// Immunities (one bit per shot behaviour).
        immunity, set_immunity, IMMUNITY: u32 @ 0x94;
        /// Offset of the animation series (added by the routines of `anim_set_routines`).
        anim_variant, set_anim_variant, ANIM_VARIANT: u8 @ 0xbf;
    }
}
