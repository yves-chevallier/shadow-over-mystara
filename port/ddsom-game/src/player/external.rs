//! Routines of subsystems other than the players that are still called by their original address (`Ctx::rom`) and that
//! several files share: a single declaration per address. They will become Rust calls when their
//! module has an API (sound: `sound`, palettes: `palette`, effects: `families`); the names marked (h) are
//! hypotheses based on usage.

/// Sounds ("wrappers" of `sound`, A0 = the player for the panning).
pub mod sound {
    /// Wind-up of a charged hit, in two stages.
    pub const CHARGE_A: u32 = 0xe792e;
    pub const CHARGE_B: u32 = 0xe1fc0;
    /// Landed hit: heavy weapons (fighter, dwarf; second strike of the elf and the magic-user) and light ones (cleric,
    /// elf, magic-user) (h).
    pub const STRIKE_HEAVY: u32 = 0xe1de0;
    pub const STRIKE_LIGHT: u32 = 0xe1e80;
    /// Attack jump (start, end) (h).
    pub const LEAP: u32 = 0xe1f20;
    pub const LEAP_END: u32 = 0xe7668;
    /// Spinning attack.
    pub const SPIN: u32 = 0xe2060;
    /// Resurrection, victory, cursed weapon falling (h).
    pub const FANFARE: u32 = 0xe24d8;
    /// Jump voice, landing sound, slide sound (per-character dispatches).
    pub const JUMP_VOICE: u32 = 0xe7622;
    pub const LANDING: u32 = 0xe774c;
    pub const SLIDE: u32 = 0xe78e8;
    /// Cleric's healing prayer, elf's earthquake (h).
    pub const PRAYER_QUAKE: u32 = 0xe8db4;
}

/// Effects created by other subsystems (`families`, `effects_aids`).
pub mod effect {
    /// Life bar of a struck monster (D0 = player and type, D1 = max HP, D2 = HP, D3 = class).
    pub const HEALTH_BAR: u32 = 0x19dce;
    /// SET object of the wand.
    pub const WAND: u32 = 0xcc352;
    /// Player effects of the elf's and magic-user's spells: sparks, bolt, earthquake.
    pub const SPARKS: u32 = 0x4dc9a;
    pub const BOLT: u32 = 0x4b7a0;
    pub const QUAKE: u32 = 0x4b340;
    pub const QUAKE_B: u32 = 0x4f132;
}
