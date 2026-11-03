//! Fields of the objects that the screens create and drive (scenery, dialogue and screen objects), besides those of
//! `Obj`: the screens give them a number, a pointer to their state block, and read their progress.

/// Number of the object among those of its kind (`+0x2f`: which picture, which player, which line...).
pub const INDEX: u32 = 0x2f;
/// Pointer (long) to the state block of the screen, whose signal byte the object reads and writes.
pub const SIGNAL: u32 = 0x4c;
/// Non-zero when a dialogue object has finished its message.
pub const MESSAGE_DONE: u32 = 0x22;
/// Sub-state of the object's automaton.
pub const SUB_STATE: u32 = 0x05;
/// Orientation: 0 faces right, 0x20 left.
pub const FACING: u32 = 0x2c;
