//! Typed views of the SET objects (docs/fin.md, "Vues typées"): the fields of each type, declared once with
//! `record!` (offset constant, getter, setter). The handlers mostly pass the offset constants to the context
//! (`x.u8(BumperObj::KIND)`); the fields common to every object are those of `Obj`.
//!
//! The views of the types of `set_b` and `set_c` are in their own `fields` modules. A guessed meaning is marked (h).

use crate::record::record;

/// Declares view types over an object: `Name;` gives `pub struct Name(pub Obj)` and its `at`.
macro_rules! view {
    ($($(#[$doc:meta])* $name:ident;)*) => {$(
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub struct $name(pub crate::objects::Obj);

        impl $name {
            pub fn at(self, off: u32) -> u32 {
                self.0.at(off)
            }
        }
    )*};
}

pub(crate) use view;

view! {
    /// Any object of the SET class: the fields that every SET type uses the same way.
    SetObj;
}

record! {
    impl SetObj {
        /// Sub-state of the handler's automaton (`+4` is the main state).
        substate, set_substate, SUBSTATE: u8 @ 0x05;
        /// Display attribute byte: bit 0x20 flips the object horizontally (h).
        attr, set_attr, ATTR: u8 @ 0x2c;
        /// Creation parameter (`+0x2f` of the object scripts; variant of the created controller...).
        param, set_param, PARAM: u8 @ 0x2f;
        /// Word parameter of the object scripts (`+0x42`).
        arg, set_arg, ARG: u16 @ 0x42;
        /// Owner: low word of the address of the object that created this one (scene controller, wave...).
        owner, set_owner, OWNER: u16 @ 0x46;
    }
}

view! {
    /// The fields of a player record that `crate::player::Player` does not name yet, as the SET types use them.
    PlayerFields;
}

record! {
    impl PlayerFields {
        /// Order of the player at the end of the bonus level (h).
        order, set_order, ORDER: u8 @ 0x7d;
        /// A special move is in progress (h): with `Player::SLOW` and `Player::SLOW_8A`, the player is busy.
        /// Objects carried (h), one byte each from `+0x140`.
        items, set_items, ITEMS: u8 @ 0x140;
        special_active, set_special_active, SPECIAL_ACTIVE: u8 @ 0x15c;
    }
}

view! {
    /// The weapon object that a player holds (`Player::LINK`), as the end of the bonus level releases it.
    WeaponObj;
}

record! {
    impl WeaponObj {
        /// Sub-state (8: dropped).
        substate, set_substate, SUBSTATE: u8 @ 0x05;
        /// Action word, reset to zero.
        action, set_action, ACTION: u16 @ 0x06;
    }
}

view! {
    /// SET 1 (`wave_leader`): fields beyond its module constants.
    WaveLeaderObj;
}

record! {
    impl WaveLeaderObj {
        /// Action word of the leader, reset at the start.
        wave_state, set_wave_state, WAVE_STATE: u16 @ 0x06;
        /// Low byte of the chief's depth, moved by the waves (h).
        depth_low, set_depth_low, DEPTH_LOW: u8 @ 0x11;
    }
}

view! {
    /// SET 5 variant 0 (`name_entry::board`): the overlap records of the previous frame (`+0x40..+0x4f`).
    NameBoardObj;
}

record! {
    impl NameBoardObj {
        /// Second long of the previous records.
        prev_record_1, set_prev_record_1, PREV_RECORD_1: u32 @ 0x44;
        /// Third long of the previous records.
        prev_record_2, set_prev_record_2, PREV_RECORD_2: u32 @ 0x48;
        /// Fourth long of the previous records.
        prev_record_3, set_prev_record_3, PREV_RECORD_3: u32 @ 0x4c;
    }
}

view! {
    /// SET 5 non-zero variant (`name_entry::cursor`): the BCD check digit of the name.
    NameCursorObj;
}

record! {
    impl NameCursorObj {
        /// Carry of the BCD addition.
        carry, set_carry, CARRY: u8 @ 0x2a;
        /// Check digit (BCD sum).
        check, set_check, CHECK: u8 @ 0x2b;
        /// High byte of the addend.
        addend_high, set_addend_high, ADDEND_HIGH: u8 @ 0x2c;
        /// Digit added.
        addend, set_addend, ADDEND: u8 @ 0x2d;
    }
}

view! {
    /// SET 12 (`fade_white`).
    FadeWhiteObj;
}

record! {
    impl FadeWhiteObj {
        /// Number of steps (0xf, not read back).
        steps, set_steps, STEPS: u16 @ 0x3c;
    }
}
