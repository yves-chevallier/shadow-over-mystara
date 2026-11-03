//! Fields of the objects of the ESHL, SOBJ, ITEM and BOX families, declared once ([`record!`]): the offsets
//! (`Base::SUB_STATE`) and the typed accessors (`Base(o).sub_state(b)`). The fields that every object has are those
//! of [`Obj`]; [`Base`] adds the layout common to these four pools, [`Shot`] the fields of the ESHL objects. A meaning
//! guessed from the code alone is marked (h). Names from docs/objets.md, docs/combat.md and
//! docs/familles-eshl-set-sobj.md.

use crate::objects::Obj;
use crate::record::record;

/// Declares a view of an object (`pub struct Name(pub Obj)` and its `at`) with its fields ([`record!`]):
/// `view! { /// doc \n Name { getter, setter, OFFSET: type @ offset; ... } }`.
macro_rules! view {
    ($(#[$doc:meta])* $name:ident { $($fields:tt)* }) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub struct $name(pub $crate::objects::Obj);

        impl $name {
            pub fn at(self, off: u32) -> u32 {
                self.0.at(off)
            }
        }

        $crate::record::record! {
            impl $name { $($fields)* }
        }
    };
}
pub(crate) use view;

/// The fields common to the objects of the ESHL, SOBJ, ITEM and BOX pools.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Base(pub Obj);

impl Base {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Base {
        /// Sub-state of the automaton (`+5`, even bytes: index of a jump table of the original).
        sub_state, set_sub_state, SUB_STATE: u8 @ 0x5;
        /// Third state byte of the automaton.
        step, set_step, STEP: u8 @ 0x6;
        /// Fourth state byte of the automaton.
        sub_step, set_sub_step, SUB_STEP: u8 @ 0x7;
        /// Low word of `X` (16.16): its fraction.
        x_fraction, set_x_fraction, X_FRACTION: u16 @ 0xa;
        /// Low word of `Z` (16.16): its fraction.
        z_fraction, set_z_fraction, Z_FRACTION: u16 @ 0xe;
        /// Low word of `Y` (16.16): its fraction.
        y_fraction, set_y_fraction, Y_FRACTION: u16 @ 0x12;
        /// Non-zero: the object is drawn with its own palette `PALETTE`.
        own_palette, set_own_palette, OWN_PALETTE: u8 @ 0x2a;
        /// Palette of the object when `OWN_PALETTE` is set.
        palette, set_palette, PALETTE: u8 @ 0x2b;
        /// Orientation (non-zero: flipped; 0x20 the sprite attribute bit), often copied from the owner.
        flip, set_flip, FLIP: u8 @ 0x2c;
        /// Collision box set (docs/combat.md).
        box_set, set_box_set, BOX_SET: u8 @ 0x2e;
        /// Setting given by the creator (rank, mode, sub-type... specific to each type).
        setting, set_setting, SETTING: u8 @ 0x2f;
        /// Low byte of the flags of the current animation entry: an event marked by the script.
        anim_event, set_anim_event, ANIM_EVENT: u8 @ 0x33;
        /// Index of the body box (first byte of `FRAME_DATA_A`).
        body_box, set_body_box, BODY_BOX: u8 @ 0x38;
        /// Index of the push box.
        push_box, set_push_box, PUSH_BOX: u8 @ 0x39;
        /// Index of the attack box.
        attack_box, set_attack_box, ATTACK_BOX: u8 @ 0x3a;
        /// Index of the ground mark (`sprites.rs`).
        ground_mark, set_ground_mark, GROUND_MARK: u8 @ 0x3b;
    }
}

/// The fields of an ESHL object (enemy shots and attacks).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Shot(pub Obj);

impl Shot {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Shot {
        /// Attack identifier (read by `combat`).
        attack_id, set_attack_id, ATTACK_ID: u8 @ 0x44;
        /// Attack power.
        attack_power, set_attack_power, ATTACK_POWER: u8 @ 0x45;
        /// Owner (16-bit RAM address).
        owner, set_owner, OWNER: u16 @ 0x50;
        /// Flags set by the collision engine (bit 3: hit, bit 5: parried).
        hit_flags, set_hit_flags, HIT_FLAGS: u8 @ 0x57;
        /// Per-type countdown, also the blinking phase (`shell::blink`).
        timer, set_timer, TIMER: u16 @ 0x5c;
    }
}

view! {
    /// Enemy or boss that carries or creates a shot: the fields the shots read or write (h).
    Host {
        /// Set to 1 when its marker shot is in place (h).
        shot_mark, set_shot_mark, SHOT_MARK: u8 @ 0x1c;
        /// Shots launched, counted by the shots (h).
        shots, set_shots, SHOTS: u8 @ 0x1d;
        /// Phase of the boss (h).
        phase, set_phase, PHASE: u8 @ 0x3e;
        /// Target of the enemy (16-bit pointer) (h).
        target, set_target, TARGET: u16 @ 0x46;
        /// Aim angle given to the shot it carries (h).
        aim, set_aim, AIM: u16 @ 0x50;
        /// Value of the halo it carries, given back when the halo ends (h).
        halo, set_halo, HALO: u16 @ 0x54;
        /// Initial hit points.
        life_max, set_life_max, LIFE_MAX: u16 @ 0x60;
        /// Hit points left.
        life, set_life, LIFE: u16 @ 0x62;
        /// Attack state word: attack and detail (h).
        attack, set_attack, ATTACK: u16 @ 0xa0;
        /// Detail of the attack state (h).
        attack_detail, set_attack_detail, ATTACK_DETAIL: u8 @ 0xa1;
        /// Step of the attack that throws debris (h).
        phase_b, set_phase_b, PHASE_B: u8 @ 0xa4;
        /// Set to 1 when its shot lands (h).
        shot_landed, set_shot_landed, SHOT_LANDED: u8 @ 0xa5;
        /// Attack flags: bit 0, the attack is launched (h).
        launched, set_launched, LAUNCHED: u8 @ 0xa8;
        /// Set to 1 when its shot ends (h).
        shot_done, set_shot_done, SHOT_DONE: u8 @ 0xae;
        /// Animation flags read by the halo it carries (bits 0 and 1) (h).
        anim_sync, set_anim_sync, ANIM_SYNC: u8 @ 0xb0;
        /// Non-zero: the host is frozen, its shots stop (h).
        frozen, set_frozen, FROZEN: u8 @ 0xb2;
        /// Cleared when its shot ends (h).
        shot_alive, set_shot_alive, SHOT_ALIVE: u8 @ 0xb5;
        /// Target point of the attack, X (h).
        target_x, set_target_x, TARGET_X: u16 @ 0xbc;
        /// Non-zero while it charges its breath (h).
        charging, set_charging, CHARGING: u8 @ 0xbd;
        /// Target point of the attack, Y (h).
        target_y, set_target_y, TARGET_Y: u16 @ 0xbe;
    }
}

view! {
    /// A shot that creates rows of shots (types 24 to 29), read by the shots it creates (h).
    Spawner {
        /// Label given to the created enemy (h).
        label, set_label, LABEL: u16 @ 0x42;
        /// Number of created shots, row of the offsets (h).
        count, set_count, COUNT: u16 @ 0x56;
        /// Countdown of the current step (h).
        timer, set_timer, TIMER: u16 @ 0x58;
        /// Linked object: shadow or second timer (h).
        link, set_link, LINK: u16 @ 0x5a;
        /// Source object (16-bit pointer).
        source, set_source, SOURCE: u16 @ 0x5c;
        /// Row of the height tables (h).
        row, set_row, ROW: u8 @ 0x60;
    }
}

view! {
    /// Fields of the SET objects created by the objects (h).
    SetView {
        /// Counter of the master scene element (h).
        param_d, set_param_d, PARAM_D: u8 @ 0xd;
        /// Non-zero: its sequence is finished.
        finished, set_finished, FINISHED: u8 @ 0x22;
        /// Result of its sequence (h).
        result, set_result, RESULT: u8 @ 0x23;
        /// Character it shows (h).
        character, set_character, CHARACTER: u8 @ 0x37;
        /// First setting given by the creator (h).
        param_a, set_param_a, PARAM_A: u8 @ 0x42;
        /// Second setting given by the creator (h).
        param_b, set_param_b, PARAM_B: u8 @ 0x43;
    }
}

view! {
    /// Fields of the effects linked to a shot (h).
    EffectView {
        /// Set to 1 by the shot to end the effect (h).
        signal, set_signal, SIGNAL: u8 @ 0x43;
    }
}
