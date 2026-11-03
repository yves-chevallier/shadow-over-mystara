//! Fields of the ENEMY objects (and of the bosses, which share the framework and the common reactions), declared once
//! ([`record!`]): the offsets (`Enemy::REACTION`) and the typed accessors (`enemy.reaction(b)`). The fields common to
//! all objects (position, speeds, animation...) are those of `Obj`. A meaning guessed from the code alone is marked
//! (h). Names from docs/familles/enemy.md, docs/combat.md and the families.
//!
//! The families (`enemy_*/`) declare their own fields (`+0xa0` and beyond, the AI parameters they use) in their own
//! records; the fields below are those of the framework, of the common reactions and of the AI helpers (`ai`,
//! `ai_aids`), which read them from this one declaration.

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
        /// State and substate as a word (`0x205`: active state, group 5).
        state_word, set_state_word, STATE_WORD: u16 @ 0x04;
        /// Substate (group of the active state; in the hit state, 6 = death).
        substate, set_substate, SUB: u8 @ 0x05;
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
        /// Fraction of the depth (low word of `Obj::Y`).
        y_fraction, set_y_fraction, Y_FRACTION: u16 @ 0x12;
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
        /// Heading of the moves by speed table (0 to 31, `Obj::SPEED_INDEX`: 0 right, 8 down, 0x10 left, 0x18 up).
        heading, set_heading, HEADING: u8 @ 0x2d;
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
        /// Mark of the shadow on the ground (`sprites`: `GROUND_MARK`), cleared at the end of some attacks.
        ground_mark, set_ground_mark, GROUND_MARK: u8 @ 0x3b;
        /// Bounds of the current frame (`sprites`: `BOUNDS`).
        bounds, set_bounds, BOUNDS: u8 @ 0x3c;
        /// Strike key of the animation entry (copied to `STRIKE_KIND` by `enemy_lib`).
        strike_key, set_strike_key, STRIKE_KEY: u8 @ 0x3e;
        /// Display priority (`sprites`: `PRIORITY`).
        priority, set_priority, PRIORITY: u8 @ 0x3f;
        /// Creator of a sub-object (word, RAM address).
        creator, set_creator, CREATOR: u16 @ 0x42;
        /// Blinking arrival: mask of the frames shown and number of blinks left (the word `CREATOR`, set to 0x703).
        blink_mask, set_blink_mask, BLINK_MASK: u8 @ 0x42;
        blink_count, set_blink_count, BLINK_COUNT: u8 @ 0x43;
        /// Strike type and strength of the current attack (docs/combat.md).
        strike_kind, set_strike_kind, STRIKE_KIND: u8 @ 0x44;
        strike_power, set_strike_power, STRIKE_POWER: u8 @ 0x45;
        /// Host of a part of a composite enemy (word, RAM address).
        host, set_host, HOST: u16 @ 0x46;
        /// Last hit received (word): the player who hit (bits 8-9) and the kind of the hit (h); the score events of
        /// the object name this player (`events::event_of`).
        hit_by, set_hit_by, HIT_BY: u16 @ 0x48;
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
        /// Strength of the hit of an attacker (read on the object `HOLDER`; halved into the freeze of the golem) (h).
        hit_strength, set_hit_strength, HIT_STRENGTH: u8 @ 0x57;
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
        /// AI: row of the first parry table (0xb048), set by `enemy_lib::set_68` from the level and the players.
        ai_68, set_ai_68, AI_68: u8 @ 0x68;
        /// AI: row of the second parry table (0xb068).
        ai_69, set_ai_69, AI_69: u8 @ 0x69;
        /// AI: row of the wander draw (`enemy_lib::draw_6b`) and the drawn duration (counted down by the families).
        ai_wander_row, set_ai_wander_row, AI_WANDER_ROW: u8 @ 0x6a;
        ai_wander, set_ai_wander, AI_WANDER: u8 @ 0x6b;
        /// AI: row of the masks of `enemy_lib::maybe_draw_6e`.
        ai_wait_masks, set_ai_wait_masks, AI_WAIT_MASKS: u8 @ 0x6c;
        /// AI: row of the wait draw (`enemy_lib::draw_6e`) and the drawn duration.
        ai_wait_row, set_ai_wait_row, AI_WAIT_ROW: u8 @ 0x6d;
        ai_wait, set_ai_wait, AI_WAIT: u8 @ 0x6e;
        /// AI row parameter set at the creation by some families, read by no code of the port (h).
        ai_spare_row, set_ai_spare_row, AI_SPARE_ROW: u8 @ 0x6f;
        /// AI: row of the stance draw (`enemy_lib::draw_stance`) and the drawn stance (or its duration).
        ai_stance_row, set_ai_stance_row, AI_STANCE_ROW: u8 @ 0x70;
        ai_stance, set_ai_stance, AI_STANCE: u8 @ 0x71;
        /// AI: rows of the decision masks (`enemy_lib::decision_a`, `decision_b`).
        ai_decision_a, set_ai_decision_a, AI_DECISION_A: u8 @ 0x72;
        ai_decision_b, set_ai_decision_b, AI_DECISION_B: u8 @ 0x73;
        /// AI: chance row and animation of the attack box (`ai_aids`).
        ai_box_chance, set_ai_box_chance, AI_BOX_CHANCE: u8 @ 0x74;
        ai_box_anim, set_ai_box_anim, AI_BOX_ANIM: u8 @ 0x75;
        /// AI: row of the byte draw (`enemy_lib::pick_byte`) and the drawn byte (a duration or a move).
        ai_byte_row, set_ai_byte_row, AI_BYTE_ROW: u8 @ 0x76;
        ai_byte, set_ai_byte, AI_BYTE: u8 @ 0x77;
        /// AI: row of the chance of a shot or a special gesture (`enemy_lib::roll_78`; 0xff: none).
        ai_shot, set_ai_shot, AI_SHOT: u8 @ 0x78;
        /// AI: set of approach points (`ai_aids`); some families keep the kind of their shot there.
        ai_point_set, set_ai_point_set, AI_POINT_SET: u8 @ 0x79;
        /// AI: point aimed at around the target (x, depth), chosen in the approach points (`ai_aids::walk`).
        ai_goal_x, set_ai_goal_x, AI_GOAL_X: u16 @ 0x7a;
        ai_goal_y, set_ai_goal_y, AI_GOAL_Y: u16 @ 0x7c;
        /// Type of the event of the object (`enemy_lib::object_event`).
        event_kind, set_event_kind, EVENT_KIND: u8 @ 0x7e;
        /// Second byte of the event word, cleared or set with `EVENT_KIND` at the creation; not read (h).
        event_low, set_event_low, EVENT_LOW: u8 @ 0x7f;
        /// Row of the loot table (`ai_aids`); cleared by the disintegration.
        loot_row, set_loot_row, LOOT_ROW: u8 @ 0x80;
        /// Loot that a thief's hit can make the enemy drop (`combat::damage::steal_loot`; 0: none).
        loot, set_loot, LOOT: u8 @ 0x81;
        /// Non-zero: a hit of the weapon 0xb of a player deals one and a half times the damage (`combat`).
        weak_to_b, set_weak_to_b, WEAK_TO_B: u8 @ 0x82;
        /// Non-zero: a hit of the weapon 0xc of a player kills at once (0x200 damage, `combat`).
        weak_to_c, set_weak_to_c, WEAK_TO_C: u8 @ 0x83;
        /// Hits before the end of a grab (word).
        recover, set_recover, RECOVER: u16 @ 0x84;
        /// AI (same place as `RECOVER` for the families that draw it): cleared inside the play area by
        /// `ai_aids::draw_84`, which writes the word 1 here outside it (h).
        ai_offscreen, set_ai_offscreen, AI_OFFSCREEN: u8 @ 0x84;
        /// AI: byte drawn in the row `AI_ROW_8E` (`ai_aids::draw_84`).
        ai_draw_85, set_ai_draw_85, AI_DRAW_85: u8 @ 0x85;
        /// Non-zero: the object was down (lying on the ground); 0 makes it not grabbable for some types.
        downed, set_downed, DOWNED: u8 @ 0x86;
        /// Non-zero: grabbed by a player.
        grabbed, set_grabbed, GRABBED: u8 @ 0x87;
        /// State to resume after a slide (long; 0: none).
        resume_state, set_resume_state, RESUME_STATE: u32 @ 0x8a;
        /// Word of the state to resume (`0x205`: active, group 5).
        resume_word, set_resume_word, RESUME_WORD: u16 @ 0x8a;
        /// AI: row of the draw of `enemy_lib::draw_84`.
        ai_row_8e, set_ai_row_8e, AI_ROW_8E: u8 @ 0x8e;
        /// Counter of the reaction steps.
        counter, set_counter, COUNTER: u8 @ 0x90;
        /// Palette to take at the next reaction (bit 7: pending; becomes `PALETTE`, `OWN_PALETTE` set).
        pending_palette, set_pending_palette, PENDING_PALETTE: u8 @ 0x91;
        /// Immunities (one bit per shot behaviour).
        immunity, set_immunity, IMMUNITY: u32 @ 0x94;
        /// AI: behaviour parameter by type, also the countdown of `ai_aids::countdown_then_state`.
        ai_behaviour, set_ai_behaviour, AI_BEHAVIOUR: u8 @ 0x98;
        /// AI: number of the last approach point retained (0xff: none).
        ai_last_point, set_ai_last_point, AI_LAST_POINT: u8 @ 0x99;
        /// Loot: row of the luck masks of the object dropped once per type, and whether it was dropped.
        once_mask, set_once_mask, ONCE_MASK: u8 @ 0x9c;
        once_done, set_once_done, ONCE_DONE: u8 @ 0x9d;
        /// HP tiers (`ai_aids::next_threshold`): current tier, number of tiers, column of the thresholds (HP rank).
        hp_tier, set_hp_tier, HP_TIER: u8 @ 0xaa;
        hp_tiers, set_hp_tiers, HP_TIERS: u8 @ 0xab;
        hp_rank, set_hp_rank, HP_RANK: u8 @ 0xac;
        /// On the object that spawned skeletons or ghouls: its children still alive (h).
        children, set_children, CHILDREN: u8 @ 0xaf;
        /// Object whose animation key or mark chooses the animation of `ai_aids::animation_of_master` and
        /// `animation_of_mark` (word, RAM address; the families that call them name it in their own records) (h).
        anim_master, set_anim_master, ANIM_MASTER: u16 @ 0xb2;
        /// Offset of the animation series (added by the routines of `anim_set_routines`).
        anim_variant, set_anim_variant, ANIM_VARIANT: u8 @ 0xbf;
    }
}
