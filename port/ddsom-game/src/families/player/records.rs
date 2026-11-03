//! Fields of the players' shots (PSHL) and effects (PEFF), declared once ([`record!`]): the offsets (`Shot::OWNER`)
//! and the typed accessors (`Shot(o).owner(b)`). The fields that every object has are those of [`Obj`]; the owning
//! player is a [`crate::player::Player`]. Each type declares its own fields (`+0x42` and beyond, most of them) next to
//! its handler. A meaning guessed from the code alone is marked (h). Names from docs/familles.md and
//! docs/familles/joueur.md.

use crate::objects::Obj;
use crate::record::record;

/// A PSHL or PEFF object.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Shot(pub Obj);

impl Shot {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Shot {
        /// Sub-state of the automaton (`+5`).
        sub_state, set_sub_state, SUB_STATE: u8 @ 0x05;
        /// Third and fourth state bytes of the automaton.
        step, set_step, STEP: u8 @ 0x06;
        sub_step, set_sub_step, SUB_STEP: u8 @ 0x07;
        /// Low words of `Obj::X`, `Obj::Z`, `Obj::Y` (16.16): their fractions.
        x_fraction, set_x_fraction, X_FRACTION: u16 @ 0x0a;
        z_fraction, set_z_fraction, Z_FRACTION: u16 @ 0x0e;
        y_fraction, set_y_fraction, Y_FRACTION: u16 @ 0x12;
        /// Non-zero: the object is drawn with its own palette `PALETTE`.
        own_palette, set_own_palette, OWN_PALETTE: u8 @ 0x2a;
        palette, set_palette, PALETTE: u8 @ 0x2b;
        /// Orientation inherited from the player (non-zero: flipped).
        flip, set_flip, FLIP: u8 @ 0x2c;
        /// Collision box set.
        box_set, set_box_set, BOX_SET: u8 @ 0x2e;
        /// Setting given by the creator (rank, mode, sub-type... specific to each type).
        setting, set_setting, SETTING: u8 @ 0x2f;
        /// Low byte of the flags of the current animation entry: an event marked by the script.
        anim_event, set_anim_event, ANIM_EVENT: u8 @ 0x33;
        /// Index of the attack box.
        attack_box, set_attack_box, ATTACK_BOX: u8 @ 0x3a;
        /// Identifier and power of the attack (read by `combat`).
        attack_id, set_attack_id, ATTACK_ID: u8 @ 0x45;
        attack_power, set_attack_power, ATTACK_POWER: u8 @ 0x46;
        /// Identifier of the hit, read by the reaction of the victim (`player::hit_reactions`).
        blow_id, set_blow_id, BLOW_ID: u8 @ 0x47;
        /// Owning player (16-bit RAM address).
        owner, set_owner, OWNER: u16 @ 0x50;
        /// Flags set by the collision engine.
        hit_flags, set_hit_flags, HIT_FLAGS: u8 @ 0x57;
    }
}

