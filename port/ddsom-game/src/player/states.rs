//! Values of the players' state machine.
//!
//! The bytes `+4..+7` of the record form a long: main state, sub-state, action and step
//! of the action. The original code often writes the long at once (`move.l #0x02000c00,(4,A0)`);
//! `State::pack` reproduces these constants.

/// Main state (`+4`).
pub mod main {
    /// Scripted: cinematics per scene; `+5` selects the sub-state (P3).
    pub const SCRIPTED: u8 = 0;
    /// Normal game.
    pub const PLAYING: u8 = 2;
    /// On the ground / ejected.
    pub const DOWN: u8 = 4;
    /// Removed from the game.
    pub const REMOVED: u8 = 6;
}

/// Sub-state (`+5`) of the normal game.
pub mod sub {
    /// The player acts: the action (`+6`) is executed.
    pub const ACTING: u8 = 0;
    /// Hit: the action (`+6`) is a reaction (`hurt`).
    pub const HURT: u8 = 4;
    /// Parry.
    pub const PARRY: u8 = 8;
}

/// Actions of the normal game (`+6`, always even: indices of the handler table 0x35586).
/// The names are deduced from the places where the action is chosen; (h) = hypothesis.
pub mod action {
    /// Standing.
    pub const STAND: u8 = 0x00;
    /// Entering slow walk (`Player::SLOW`) or returning to normal walk (down + jump) (h).
    pub const SLOW_SWITCH: u8 = 0x02;
    /// Walk.
    pub const WALK: u8 = 0x04;
    /// Run.
    pub const RUN: u8 = 0x06;
    /// Turn.
    pub const TURN: u8 = 0x08;
    /// Jump (character-specific code, `characters`).
    pub const JUMP: u8 = 0x0a;
    /// Button 1 attack (character-specific).
    pub const ATTACK: u8 = 0x0c;
    /// Button 4 special attack (character-specific).
    pub const SPECIAL: u8 = 0x0e;
    /// Picks up (`+0x8b` = 0) or drops (2) the carried object (`moves::carry`).
    pub const RELEASE: u8 = 0x14;
    /// Shield guard (weapons 0x28..0x30) while button 4 is held (`moves::guard`).
    pub const SPECIAL_ALT: u8 = 0x16;
    /// Shield guard while button 1 is held, grab during a turn (`moves::guard`).
    pub const TURN_ATTACK: u8 = 0x18;
    /// Climb (h) (`moves::climb`).
    pub const CLIMB: u8 = 0x1a;
    /// Pushes the linked object in a direction (`moves::push`).
    pub const PUSH: u8 = 0x2e;
    /// Grabbed by an opponent (the player is held).
    pub const GRAB: u8 = 0x22;
    /// Spell with no charge left: failure animation (h).
    pub const SPELL_FIZZLE: u8 = 0x32;
    /// These actions undergo neither collision with the terrain nor screen re-framing.
    pub const NO_TERRAIN_A: u8 = 0x20;
    pub const NO_TERRAIN_B: u8 = 0x2a;
    pub const NO_TERRAIN_C: u8 = 0x24;
}

/// A complete state (`+4..+7`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct State(pub u32);

impl State {
    /// Assembles the four bytes.
    pub const fn pack(main: u8, sub: u8, action: u8, step: u8) -> State {
        State((main as u32) << 24 | (sub as u32) << 16 | (action as u32) << 8 | step as u32)
    }

    /// The long without the step byte (comparisons `andi.l #-0x100`).
    pub const fn without_step(self) -> u32 {
        self.0 & !0xff
    }

    pub const fn main(self) -> u8 {
        (self.0 >> 24) as u8
    }
    pub const fn sub(self) -> u8 {
        (self.0 >> 16) as u8
    }
    pub const fn action(self) -> u8 {
        (self.0 >> 8) as u8
    }
    pub const fn step(self) -> u8 {
        self.0 as u8
    }
}

/// States written by the common code.
pub mod state {
    use super::{action, main, sub, State};

    /// Standing, animation in progress (step 2).
    pub const STAND: State = State::pack(main::PLAYING, sub::ACTING, action::STAND, 2);
    /// Walk, start.
    pub const WALK: State = State::pack(main::PLAYING, sub::ACTING, action::WALK, 0);
    /// Turn (step 2: slow walk).
    pub const TURN: State = State::pack(main::PLAYING, sub::ACTING, action::TURN, 0);
    pub const TURN_SLOW: State = State::pack(main::PLAYING, sub::ACTING, action::TURN, 2);
    /// Button 1 attack.
    pub const ATTACK: State = State::pack(main::PLAYING, sub::ACTING, action::ATTACK, 0);
    /// Jump.
    pub const JUMP: State = State::pack(main::PLAYING, sub::ACTING, action::JUMP, 0);
    /// Special attack.
    pub const SPECIAL: State = State::pack(main::PLAYING, sub::ACTING, action::SPECIAL, 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_states_match_the_original_constants() {
        assert_eq!(state::STAND.0, 0x0200_0002);
        assert_eq!(state::ATTACK.0, 0x0200_0c00);
        assert_eq!(State::pack(2, 4, 0, 0).sub(), sub::HURT);
        assert_eq!(State(0x0200_0e01).without_step(), 0x0200_0e00);
        assert_eq!(State(0x0200_0e01).action(), action::SPECIAL);
    }
}
