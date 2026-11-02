//! Player characters: record, common state machine, creation, hit reactions.
//!
//! Four records of 0x200 bytes at `A5+0x5de` (`PLAYER_BASE`, stride `PLAYER_STRIDE`); a player
//! is also an object (`objects::Obj`): the common fields (position, speeds, animation, list
//! links) are those of `docs/objects.md`, the player-specific fields are named in `Player`.
//! See `docs/player.md` for the details of the structures and states.
//!
//! Organization (the letters P1.. are those of `docs/atlas.md`):
//! - `states`: values of the state machine (`+4..+7`);
//! - `init` (P1): reset and creation of the records;
//! - `update` (P4): loop over the four players and dispatch of the main states;
//! - `actions` (P4): common actions (standing, walking, turning, grabbing);
//! - `checks` (P4): transitions according to buttons (turn, attack, jump, throw, special);
//! - `walk` (P4): walking speeds, movement, screen limits;
//! - `animation`: player-specific animations (long entries);
//! - `effects`, `overlays` (P4): auras, palettes, weapon and shield drawn with the player;
//! - `hurt` (P4): player hit, on the ground or dead;
//! - `combos` (P6): recognition of special moves by input sequences;
//! - `hit_reactions` (P6): reaction of an enemy hit by a player;
//! - `scripted` (P3): scripted state (scene entry, resurrection, cinematics);
//! - `characters`: hook points for the code of the six characters (P5, rewritten elsewhere);
//! - `cov`: coverage of the Rust functions by `examples/player_coverage`.
//!
//! Interface for the characters (P5): see `characters`. The original routines not yet
//! rewritten are called by `Ctx::rom`, which goes through the executor and hence through the natives: declaring
//! a `Native` at the address of an original routine is enough to replace it everywhere.

/// Counts the call of the enclosing function when coverage is active (`cov`).
macro_rules! cov {
    () => {{
        fn here() {}
        $crate::player::cov::hit(std::any::type_name_of_val(&here))
    }};
}

pub mod actions;
pub mod animation;
pub mod cov;
pub mod characters;
pub mod cinematics;
pub mod combos;
pub mod checks;
pub mod effects;
pub mod external;
pub mod hit_reactions;
pub mod hurt;
pub mod init;
pub mod moves;
pub mod overlays;
pub mod scripted;
pub mod spawn;
pub mod states;
pub mod update;
pub mod walk;

use cps2::Cps2Bus;
use m68k::{Cpu, SR_Z};

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::objects::Obj;
use crate::{call, Native};

/// Native routines of the players.
pub const NATIVES: &[Native] = &[
    init::CLEAR_PLAYERS,
    init::CLEAR_OVERLAYS,
    init::INIT_P1,
    init::INIT_P2,
    init::INIT_P3,
    init::INIT_P4,
    update::UPDATE_PLAYERS,
    overlays::UPDATE,
    walk::CLAMP,
    characters::fighter::JUMP,
    characters::fighter::ATTACK,
    characters::fighter::SPECIAL,
    characters::fighter::DASH,
    characters::cleric::JUMP,
    characters::cleric::ATTACK,
    characters::cleric::SPECIAL,
    characters::cleric::DASH,
    characters::elf::JUMP,
    characters::elf::ATTACK,
    characters::elf::SPECIAL,
    characters::elf::DASH,
    characters::magic_user::JUMP,
    characters::magic_user::ATTACK,
    characters::magic_user::SPECIAL,
    characters::magic_user::DASH,
    characters::dwarf::JUMP,
    characters::dwarf::ATTACK,
    characters::dwarf::SPECIAL,
    characters::dwarf::DASH,
    characters::dwarf::ROLL,
    characters::thief::JUMP,
    characters::thief::ATTACK,
    characters::thief::SPECIAL,
    characters::thief::DASH,
    characters::thief::ROLL,
    actions::TO_STAND,
    actions::START_STAND,
    moves::LEAP,
    moves::HOP_BACK,
    moves::CURSE_DROP,
    moves::DUST_PUFFS,
    moves::BOX_RECOIL,
    spawn::SLASH,
    spawn::BLOOD,
    spawn::CURE,
    spawn::POTION,
    spawn::HOLY_LIGHT,
    spawn::BEAM,
    spawn::BLESSING,
    spawn::BANISH,
    spawn::WEAPON_TRAIL,
    spawn::SHOWER,
    spawn::AURA,
    spawn::LIGHTNING_TARGET,
    spawn::SWARM,
    spawn::STORM,
    spawn::RING,
    spawn::BEAM_CAST,
    spawn::METEORS,
    spawn::ICE_COLUMNS,
    spawn::ICE_MIST,
    spawn::ICE_SHARDS,
    spawn::ICE_CROWN,
    spawn::FIREBALL,
    spawn::ICE_GROUND,
    spawn::THROWN_A,
    spawn::THROWN_B,
    spawn::THROWN_C,
    spawn::THROWN_D,
    spawn::THROWN_E,
    spawn::THROWN_F,
    spawn::THROWN_G,
    spawn::THROWN_H,
    spawn::LEAP,
    spawn::SWAP,
    spawn::DASH,
    spawn::WALL_TARGET,
    spawn::LIGHTNING,
    spawn::KNIFE,
    spawn::SHADOWS,
    spawn::FLARE,
    spawn::SUMMON,
    spawn::PORTAL,
    spawn::WITCH,
    spawn::WITCH_A,
    spawn::WITCH_B,
    spawn::ROD_FLAME_SHOT,
    spawn::ROD_BALL_SHOT,
    spawn::ROD_STAR_SHOT,
    spawn::CURSED_WEAPON,
    spawn::DYING_SHARDS,
    spawn::STORM_CLOUD,
    spawn::PORTAL_GLOW,
    spawn::PORTAL_SPARKS,
    spawn::PORTAL_BURST,
    spawn::RAIN,
    spawn::STORM_SPELL,
    spawn::RAIN_GLOW,
    spawn::WALL_SPARK,
    spawn::WAVE_BOLT,
    spawn::MOON,
    spawn::MOON_B,
];

/// First player record (offset from A5) and stride between two players.
pub const PLAYER_BASE: u32 = 0x5de;
pub const PLAYER_STRIDE: u32 = 0x200;
pub const PLAYER_COUNT: u32 = 4;

/// Work RAM variables read or written by the players' code (typed views, `crate::ram`). The
/// shared variables come from `crate::ram`; the names marked (h) are hypotheses.
pub mod ram {
    use crate::ram::{Word, Byte};

    pub use crate::ram::{CAMERA_ALT_ACTIVE as USE_ALT_CAMERA, CAMERA_ALT_X, CAMERA_ALT_Y, CAMERA_X, CAMERA_Y, PHASE, SCENE};
    /// Spell-in-progress indicator (player 1's pause bit, `crate::ram::PAUSE`).
    pub use crate::ram::PAUSE as SPELL_FLAG;

    /// Byte 2 of the current inputs (`FF8000-FF8005`), which the effect of footsteps in water adds to the frame
    /// counter.
    pub const INPUTS_2: Byte = Byte::a5(0x2);
    /// Game sub-step (`FF802E`): progress of the scene entry.
    pub const SUB_STEP: Byte = Byte::a5(0x2e);
    /// Bit k: player k takes part in the game.
    pub const PLAYERS_MASK: Byte = Byte::a5(0x31);
    /// Number of players alive.
    pub use crate::ram::PLAYERS_ALIVE as PLAYERS_ALIVE;
    /// Game pause during a spell: the players no longer age, collisions are suspended.
    pub const PAUSE: Byte = Byte::a5(0x3d);
    /// Frozen game (cinematic).
    pub const FROZEN: Byte = Byte::a5(0x42);
    /// Player input mask.
    pub const INPUT_MASK: Byte = Byte::a5(0x43);
    /// Frozen inputs (scenario in progress).
    pub const INPUT_RAW: Byte = Byte::a5(0x44);
    /// Object followed by the camera (word: short address).
    pub const CAMERA_FOCUS: Word = Word::a5(0x46);
    /// Scenario in progress (second flag, with `INPUT_RAW`).
    pub const SCRIPTED_B: Byte = Byte::a5(0x4e);
    /// Value copied from the killed enemy when it provides one (loot).
    pub const KILL_FLAG: Byte = Byte::a5(0x4f);
    /// Counter of the curses of weapon 0xe (h).
    pub const CURSE_COUNT: Byte = Byte::a5(0x56);
    /// "Slow-motion" mode: all the players walk slowly (h).
    pub const SLOW_MODE: Byte = Byte::a5(0x59);
    /// Row chosen by the spell in progress.
    pub const SPELL_ROW: Byte = Byte::a5(0x5b);
    /// End of scene without a victory pose (h).
    pub const NO_VICTORY: Byte = Byte::a5(0x5c);
    /// Scene entry without walking (h).
    pub const NO_WALK_IN: Byte = Byte::a5(0x66);
    /// Low byte of the VBLANK frame counter (`crate::ram::IMAGES`).
    pub const VBLANK_FRAMES_LOW: Byte = Byte::a5(0x6d);
    /// Low byte of the game frame counter (`FF806E`): parity, blinking.
    pub const FRAME_COUNTER: Byte = Byte::a5(0x6f);
    /// Game version (h): selects the series of predefined names of the score table.
    pub const VERSION: Byte = Byte::a5(0x71);
    /// Pending palette fade (state of `tasks::fading`): pages, number of steps, delay.
    pub const FADE_PAGES: Byte = Byte::a5(0x48e);
    pub const FADE_STEPS: Byte = Byte::a5(0x48f);
    pub const FADE_DELAY: Word = Word::a5(0x490);
    /// Vertical scroll of plane 1 (`vblank`): spells shake the screen by toggling it.
    pub use crate::ram::SCROLL_Y_PLANE1 as SCROLL1_Y;
    /// Boss signal to the scripted players (throw, landing).
    pub const BOSS_SIGNAL: Word = Word::a5(0x51e);
    /// Cinematic in progress (h).
    pub const CUTSCENE: Byte = Byte::a5(0x53a);
    /// Switch word read at VBLANK (`FF7FD0`).
    pub use crate::ram::DSW;
    /// Word: table random counter (see combat).
    pub const TICK: Word = Word::a5(0x7f92);
}

/// Execution context of the native routines: registers and memory of the 68000.
pub struct Ctx<'a> {
    pub c: &'a mut Cpu,
    pub b: &'a mut Cps2Bus,
}

impl<'a> Ctx<'a> {
    pub fn new(c: &'a mut Cpu, b: &'a mut Cps2Bus) -> Self {
        Ctx { c, b }
    }

    /// Calls an original (or native) routine for player `p`: A0 = player, A5 = base of the
    /// RAM. Returns true if the Z flag is set on return.
    #[track_caller]
    pub fn rom(&mut self, addr: u32, p: Player) -> bool {
        cov::rom_call(addr);
        self.c.a[0] = p.0;
        self.c.a[5] = A5;
        call(self.c, self.b, addr);
        self.c.sr & SR_Z != 0
    }

    /// 0xb10c8: shock effect (dust, spark) of `variant` and `setting`, speeds `vx`, `vy`, created for player `p`.
    /// The registers are left like the original routine leaves them (A0 = the player, A1 = the effect, carry when the
    /// pool is full, D1, flags).
    pub fn shock(&mut self, p: Player, variant: u8, setting: u16, vx: u16, vy: u16) -> Option<Obj> {
        self.c.a[5] = A5;
        crate::effects_aids::shock_68k(self.c, self.b, p.obj(), variant, setting, vx, vy)
    }

    /// Plays sound `sound` for player `p` (panning, character: `sound::play`).
    pub fn sound(&mut self, sound: u32, p: Player) {
        crate::sound::play(self.b, sound, Obj(p.0));
    }

    /// Same with parameter D6 (animation number, effect number...).
    #[track_caller]
    pub fn rom_d6(&mut self, addr: u32, p: Player, d6: u32) -> bool {
        self.c.d[6] = d6;
        self.rom(addr, p)
    }
}

/// A player, designated by the address of its record (register A0 of the original).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Player(pub u32);

/// Fields of the player record (offsets). The names marked (h) are hypotheses.
impl Player {
    /// Byte: slot occupied (0 = free); 2 = leaving.
    pub const ACTIVE: u32 = Obj::ACTIVE;
    /// Bit 1: player hidden (invulnerability blinking).
    pub const FLAGS: u32 = Obj::FLAGS;
    /// Player number 0..3.
    pub const INDEX: u32 = 0x02;
    /// Character 0..11: `CHARACTER >> 1` = class (fighter, cleric, elf, dwarf, magic-user, thief).
    pub const CHARACTER: u32 = Obj::VARIANT;
    /// Main state (`states::Main`), sub-state (`states::Sub`), action (`states::Action`), step.
    pub const MAIN: u32 = 0x04;
    pub const SUB: u32 = 0x05;
    pub const ACTION: u32 = 0x06;
    pub const STEP: u32 = 0x07;
    /// Palette group and number.
    pub const PALETTE_GROUP: u32 = 0x2a;
    pub const PALETTE: u32 = 0x2b;
    /// Orientation: 0 = faces right, 0x20 = left.
    pub const FACING: u32 = 0x2c;
    /// Collision box set (character class).
    pub const BOX_SET: u32 = 0x2e;
    /// Contact with the ground / a wall (h): tested after a movement.
    pub const BLOCKED: u32 = 0x29;
    /// Hit freeze (frames remaining; negative: to clear).
    pub const HITSTOP: u32 = Obj::COUNTER_53;
    /// Attached weapon / sub-objects (words: object addresses).
    pub const OVERLAY_BODY: u32 = 0x48;
    pub const OVERLAY_ATTACK: u32 = 0x4a;
    /// Animation counter of the screen countdown palette (h).
    pub const PAL_STEP: u32 = 0x52;
    /// Maximum, current, previous hit points.
    pub const HP_MAX: u32 = 0x60;
    pub const HP: u32 = 0x62;
    pub const HP_PREV: u32 = 0x64;
    /// Parameter of the current action (fallback animation number, duration...).
    pub const PARAM: u32 = 0x69;
    pub const SUBACTION: u32 = 0x6a;
    pub const PARAM_6B: u32 = 0x6b;
    /// Held inputs, from the previous frame, new.
    pub const KEYS_HELD: u32 = 0x6c;
    pub const KEYS_PREV: u32 = 0x6d;
    pub const KEYS_NEW: u32 = 0x6e;
    pub const TIMER_6F: u32 = 0x6f;
    /// Countdown (frames) after which `RECOIL` clears.
    pub const RECOIL: u32 = 0x70;
    pub const RECOIL_TIMER: u32 = 0x71;
    /// Invulnerability: frames remaining.
    pub const INVULN: u32 = 0x7e;
    pub const POWERUP: u32 = 0x79;
    pub const POWERUP_TIMER: u32 = 0x7a;
    /// Forced slow walk.
    pub const SLOW: u32 = 0x66;
    /// Rider / linked object (16-bit pointer at +0x86: the attached object).
    pub const LINK: u32 = 0x86;
    /// Slow walk (mount, mud...) (h).
    pub const SLOW_8A: u32 = 0x8a;
    pub const LEVEL: u32 = 0x10c;
    /// Current weapon and shield (numbers), see P1 tables.
    pub const WEAPON_A: u32 = 0x10d;
    pub const WEAPON_B: u32 = 0x10e;
    pub const POWER_10F: u32 = 0x10f;
    /// Purse (BCD, long) (h): sum of points.
    pub const SCORE_BCD: u32 = 0x110;
    pub const SLOT_TABLE: u32 = 0x120;
    pub const SPECIAL_COUNT: u32 = 0x15d;
    pub const SPECIAL_ID: u32 = 0x15f;
    pub const SPEED_CLASS: u32 = 0x162;
    pub const AURA: u32 = 0x182;
    pub const AURA_B: u32 = 0x183;
    pub const AURA_C: u32 = 0x184;
    pub const AURA_TIMER: u32 = 0x180;
    pub const FREEZE_194: u32 = 0x194;
    pub const DYING_TIMER: u32 = 0x196;
}

impl Player {
    pub fn obj(self) -> Obj {
        Obj(self.0)
    }

    pub fn at(self, off: u32) -> u32 {
        self.0.wrapping_add(off)
    }

    /// Player number `k` (0..3).
    pub fn nth(k: u32) -> Player {
        Player(A5 + PLAYER_BASE + PLAYER_STRIDE * k)
    }

    pub fn all() -> impl Iterator<Item = Player> {
        (0..PLAYER_COUNT).map(Player::nth)
    }

    pub fn u8(self, g: &mut Ctx, off: u32) -> u8 {
        rd8(g.b, self.at(off))
    }
    pub fn u16(self, g: &mut Ctx, off: u32) -> u16 {
        rd16(g.b, self.at(off))
    }
    pub fn u32(self, g: &mut Ctx, off: u32) -> u32 {
        rd32(g.b, self.at(off))
    }
    pub fn set_u8(self, g: &mut Ctx, off: u32, v: u8) {
        wr8(g.b, self.at(off), v)
    }
    pub fn set_u16(self, g: &mut Ctx, off: u32, v: u16) {
        wr16(g.b, self.at(off), v)
    }
    pub fn set_u32(self, g: &mut Ctx, off: u32, v: u32) {
        wr32(g.b, self.at(off), v)
    }

    /// `+4..+7` read together (main state, sub-state, action, step).
    pub fn state(self, g: &mut Ctx) -> u32 {
        self.u32(g, Self::MAIN)
    }
    pub fn set_state(self, g: &mut Ctx, s: u32) {
        self.set_u32(g, Self::MAIN, s)
    }

    pub fn is_active(self, g: &mut Ctx) -> bool {
        self.u8(g, Self::ACTIVE) != 0
    }

    /// Facing left?
    pub fn faces_left(self, g: &mut Ctx) -> bool {
        self.u8(g, Self::FACING) != 0
    }

    /// True if the direction (bits 0..1 of the inputs) asks to look the other way than
    /// the current orientation. Bit 0 = right (orientation 0), otherwise left (0x20).
    pub fn wants_flip(self, g: &mut Ctx, keys: u8) -> bool {
        let want = if keys & 1 != 0 { 0x20 } else { 0 };
        want != self.u8(g, Self::FACING)
    }

    /// Toggles the orientation bit half (turns the character around).
    pub fn flip(self, g: &mut Ctx) {
        let v = self.u8(g, Self::FACING) ^ 0x20;
        self.set_u8(g, Self::FACING, v);
    }

    /// Character class (0 fighter ... 5 thief).
    pub fn class(self, g: &mut Ctx) -> u8 {
        self.u8(g, Self::CHARACTER) >> 1
    }
}
