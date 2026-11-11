//! Identifiers (original addresses) of routines of other subsystems that several files of the players share: a single
//! declaration per address (sounds: `sound::play` through `Ctx::sound`); the names marked (h) are hypotheses based on
//! usage.

/// Sounds ("wrappers" of `sound`, A0 = the player for the panning).
pub mod sound {
    /// Wind-up of a charged hit, in two stages.
    pub const CHARGE_A: crate::hw::sound::Sound = crate::hw::sound::CMD_129_229;
    pub const CHARGE_B: crate::hw::sound::Sound = crate::hw::sound::CMD_A1_BY_CHARACTER_C;
    /// Landed hit: heavy weapons (fighter, dwarf; second strike of the elf and the magic-user) and light ones (cleric,
    /// elf, magic-user) (h).
    pub const STRIKE_HEAVY: crate::hw::sound::Sound = crate::hw::sound::CMD_A0_BY_CHARACTER;
    pub const STRIKE_LIGHT: crate::hw::sound::Sound = crate::hw::sound::CMD_A1_BY_CHARACTER;
    /// Attack jump (start, end) (h).
    pub const LEAP: crate::hw::sound::Sound = crate::hw::sound::CMD_A1_BY_CHARACTER_B;
    pub const LEAP_END: crate::hw::sound::Sound = crate::hw::sound::CMD_113_213;
    /// Spinning attack.
    pub const SPIN: crate::hw::sound::Sound = crate::hw::sound::CMD_A2_BY_CHARACTER;
    /// Resurrection, victory, cursed weapon falling (h).
    pub const FANFARE: crate::hw::sound::Sound = crate::hw::sound::CMD_A9_BY_CHARACTER;
    pub const LANDING: crate::hw::sound::Sound = crate::hw::sound::LANDING_VOICE;
    pub const SLIDE: crate::hw::sound::Sound = crate::hw::sound::SLIDE_VOICE;
    /// Cleric's healing prayer, elf's earthquake (h).
    pub const PRAYER_QUAKE: crate::hw::sound::Sound = crate::hw::sound::CMD_16A;
}

/// Effects created by other subsystems (`families`, `effects_aids`).
pub mod effect {
    /// Life bar of a struck monster (D0 = player and type, D1 = max HP, D2 = HP, D3 = class).
    pub const HEALTH_BAR: crate::world::data::RomAddr = crate::world::data::RomAddr(0x19dce);
    /// SET object of the wand.
    pub const WAND: crate::world::data::RomAddr = crate::world::data::RomAddr(0xcc352);
    /// Player effects of the elf's and magic-user's spells: sparks, bolt, earthquake.
    pub const SPARKS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x4dc9a);
    pub const BOLT: crate::world::data::RomAddr = crate::world::data::RomAddr(0x4b7a0);
    pub const QUAKE: crate::world::data::RomAddr = crate::world::data::RomAddr(0x4b340);
    pub const QUAKE_B: crate::world::data::RomAddr = crate::world::data::RomAddr(0x4f132);
}
