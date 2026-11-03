//! Fields of the effect objects, declared once ([`record!`]): the offsets (`Ballot::VOTERS`) and the typed
//! accessors (`ballot.voters(b)`). A meaning guessed from the code alone is marked (h). Names from docs/familles.md.

use crate::objects::Obj;
use crate::record::record;

/// Blinking marker of an object (type of `effects_b`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Blinker(pub Obj);

impl Blinker {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Blinker {
        /// Blinks left (h).
        blinks, set_blinks, BLINKS: u8 @ 0x57;
        /// Blink state.
        on, set_on, ON: u8 @ 0x5a;
        /// Set once a player of class 5 comes near (h).
        armed, set_armed, ARMED: u8 @ 0x5b;
    }
}

/// Fields of the objects that carry or create effects (enemies, players, scenery), read by the effects.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Carrier(pub Obj);

impl Carrier {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Carrier {
        /// State and sub-state as a word.
        state_word, set_state_word, STATE_WORD: u16 @ 0x4;
        /// Third state byte of the automaton (attack step of an enemy) (h).
        step, set_step, STEP: u8 @ 0x6;
        /// Fourth state byte of the automaton (h).
        sub_step, set_sub_step, SUB_STEP: u8 @ 0x7;
        /// Words per player of the interface owner (0xffff: absent) (h).
        slots, set_slots, SLOTS: u16 @ 0x8;
        /// Low word of its depth (fraction).
        y_low, set_y_low, Y_LOW: u16 @ 0x12;
        /// Gauge bits of the interface owner, two per player (h).
        gauge_bits, set_gauge_bits, GAUGE_BITS: u16 @ 0x14;
        /// Non-zero: the interface owner is present (h).
        present, set_present, PRESENT: u8 @ 0x1c;
        /// Non-zero: the scenery that carries the platform has stopped (h).
        halted, set_halted, HALTED: u8 @ 0x22;
        /// Characters still on their way, counted down by the walkers (h).
        waiting, set_waiting, WAITING: u8 @ 0x23;
        /// Low byte of the flags of its current animation entry.
        anim_event, set_anim_event, ANIM_EVENT: u8 @ 0x33;
        /// Current sprite frame.
        frame, set_frame, FRAME: u32 @ 0x34;
        /// Attribute byte of its current sprite frame (low byte of `FRAME_DATA_A`) (h).
        frame_attribute, set_frame_attribute, FRAME_ATTRIBUTE: u8 @ 0x3b;
        /// Current action of the carried object (players and objects) (h).
        action_byte, set_action_byte, ACTION_BYTE: u8 @ 0x3d;
        /// Non-zero: hidden or suspended.
        hidden, set_hidden, HIDDEN: u8 @ 0x40;
        /// Non-zero: the followed object stops and the alternate camera rests (h).
        stopped, set_stopped, STOPPED: u8 @ 0x42;
        /// Setting given to its beacon (h).
        beacon_setting, set_beacon_setting, BEACON_SETTING: u8 @ 0x46;
        /// Current action of the enemy (attack, hit...).
        action, set_action, ACTION: u8 @ 0x49;
        /// Non-zero: the owner shows the spark of its attack (h).
        spark_flag, set_spark_flag, SPARK_FLAG: u8 @ 0x58;
        /// Index of the rise of its sparkle (h).
        lift, set_lift, LIFT: u8 @ 0x60;
        /// Set to 1 when the effect it created ends (h).
        mark, set_mark, MARK: u8 @ 0x77;
        /// Pose of the carrier: places the bubble of variant 3 (h).
        pose, set_pose, POSE: u8 @ 0x86;
        /// Step of its script (0x225: glowing) (h).
        script_step, set_script_step, SCRIPT_STEP: u16 @ 0xa0;
    }
}

/// Coin thrown by a reward (type of `effects_d`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Coin(pub Obj);

impl Coin {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Coin {
        /// Object that throws it (16-bit pointer).
        source, set_source, SOURCE: u16 @ 0x42;
    }
}

/// Fields shared by the effect objects (EFFCT) and read by them on other objects: palette, orientation, setting,
/// links.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Effect(pub Obj);

impl Effect {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Effect {
        /// Low word of the depth `Y` (16.16): its fraction.
        y_low, set_y_low, Y_LOW: u16 @ 0x12;
        /// Non-zero: the object is drawn with its own palette `PALETTE` (h).
        own_palette, set_own_palette, OWN_PALETTE: u8 @ 0x2a;
        /// Palette of the object when `OWN_PALETTE` is set.
        palette, set_palette, PALETTE: u8 @ 0x2b;
        /// Orientation: 0 facing right, 0x20 flipped (the sprite attribute bit).
        facing, set_facing, FACING: u8 @ 0x2c;
        /// Setting given by the creator (mode, player, kind... specific to each type).
        setting, set_setting, SETTING: u8 @ 0x2f;
        /// Low byte of the flags of the current animation entry: an event marked by the script (h).
        anim_event, set_anim_event, ANIM_EVENT: u8 @ 0x33;
        /// Attribute byte of the current sprite frame (h).
        frame_attribute, set_frame_attribute, FRAME_ATTRIBUTE: u8 @ 0x3b;
        /// Countdown of the current step (frames).
        timer, set_timer, TIMER: u8 @ 0x42;
        /// Owner or creator (16-bit pointer).
        owner, set_owner, OWNER: u16 @ 0x50;
        /// Rank among the trails (h).
        rank, set_rank, RANK: u8 @ 0x52;
        /// Blinking phase (0 or 1) compared with the frame counter.
        blink_phase, set_blink_phase, BLINK_PHASE: u8 @ 0x58;
        /// Linked object (16-bit pointer), for the attack and scenery effects.
        link, set_link, LINK: u16 @ 0x5c;
    }
}

/// Player interface elements (type 17).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Interface(pub Obj);

impl Interface {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Interface {
        /// Shape of the element (word) (h).
        shape, set_shape, SHAPE: u16 @ 0x12;
        /// Value shown: presence of the player (variant 2), counter (variant 3) (h).
        shown, set_shown, SHOWN: u8 @ 0x14;
        /// Blink countdown (variant 2), value displayed (variant 3) (h).
        blink, set_blink, BLINK: u8 @ 0x15;
        /// Non-zero: raised by 0x10 pixels (h).
        raised, set_raised, RAISED: u8 @ 0x16;
        /// Ready flag (variant 0), gauge bits noted (variant 2) (h).
        value, set_value, VALUE: u8 @ 0x1c;
    }
}

