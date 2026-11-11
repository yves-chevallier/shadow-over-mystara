//! Interface with the modelled hardware (docs/fin.md, section 2, layer `hw`): graphics memory and video registers
//! ([`video`]), input and output ports ([`io`]). No game logic: typed places where the game reads and writes the
//! hardware, so that the rest of the game no longer writes hardware addresses.
//!
//! The buffers keep the CPS2 format, since that is what `validate` compares with the original frame by frame.

pub mod io;
pub mod video;
pub mod eeprom;
pub mod inputs;
pub mod sound;
