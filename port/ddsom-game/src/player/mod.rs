//! Player characters: record, common state machine, creation, hit reactions.
//!
//! Four records of 0x200 bytes at `A5+0x5de` (`PLAYER_BASE`, stride `PLAYER_STRIDE`); a player
//! is also an object (`objects::Obj`): the common fields (position, speeds, animation, list
//! links) are those of `docs/objets.md`, the player-specific fields are named in `Player`.
//! See `docs/joueur.md` for the details of the structures and states.
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
//! Interface for the characters (P5): see `characters`. Every routine reached through a code table of the original
//! is called directly; the natives at the original addresses only serve the tools (`fuzz_player`, `DDSOM_CHECK`):
//! they read their arguments in the 68000 registers and call the Rust function.

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
pub mod tables;
pub mod update;
pub mod walk;

use crate::world::World;

use crate::dispatch::Leftovers;
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::objects::Obj;
use crate::Native;

/// Native routines of the players.
pub const NATIVES: &[Native] = &[
    init::CLEAR_PLAYERS,
    init::CLEAR_OVERLAYS,
    init::INIT_P1,
    init::INIT_P2,
    init::INIT_P3,
    init::INIT_P4,
    
    
    
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
    
    
    
    
    moves::CURSE_DROP,
    moves::DUST_PUFFS,
    moves::BOX_RECOIL,
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
];

/// First player record (offset from A5) and stride between two players.
pub const PLAYER_BASE: u32 = crate::ram::PLAYERS.base - A5;
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

/// Execution context of the players' code: the memory, without registers.
pub struct Ctx<'a> {
    pub b: &'a mut World,
    /// What the last step dispatch left (see [`Jumped`]).
    pub jumped: Jumped,
    /// The leftovers D2-D5 that the players' code leaves to what follows (the object handlers read them:
    /// `dispatch::Leftovers`): original glitches, written by name where the original writes these registers.
    pub leftovers: Leftovers,
}

/// The original reaches the steps of the reactions and cinematics through jump tables of offsets (`move.w
/// (table,PC,D0.w),D1 ; jmp (table,PC,D1.w)`): the step reached finds the index in D0.b and the offset in D1.w. Some
/// steps hand D1.b on, as the setting of the effect they create (original glitch, written in the clear).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Jumped {
    pub index: u8,
    pub offset: u16,
}

impl<'a> Ctx<'a> {
    /// Context of a caller whose leftovers are not read afterwards.
    pub fn new(b: &'a mut World) -> Self {
        Ctx { b, jumped: Jumped::default(), leftovers: Leftovers::default() }
    }

    /// Runs `f` on the player that a native at an original address receives (`Body::Handler`: the player is the
    /// object, A0 of the original), the leftovers D2-D5 passing in and out like those of the dispatch loop.
    pub fn on_player(b: &mut World, a: &mut crate::dispatch::Dispatch, f: impl FnOnce(&mut Ctx, Player)) {
        let mut g = Ctx { b, jumped: Jumped::default(), leftovers: a.leftovers };
        f(&mut g, Player(a.object.0));
        a.leftovers = g.leftovers;
    }

    /// 0xb10c8: shock effect (dust, spark) of `variant` and `setting`, speeds `vx`, `vy`, created for player `p`;
    /// `None` when the pool is full or in the scene without shocks (41).
    pub fn shock(&mut self, p: Player, variant: u8, setting: u16, vx: u16, vy: u16) -> Option<Obj> {
        crate::effects_aids::shock(self.b, p.obj(), variant, setting, vx, vy)
    }

    /// Plays sound `sound` for player `p` (panning, character: `sound::play`).
    pub fn sound(&mut self, sound: crate::sound::Sound, p: Player) {
        crate::sound::play(self.b, sound, Obj(p.0));
    }
}

impl crate::machine::Machine for Ctx<'_> {
    fn bus(&mut self) -> &mut World {
        &mut *self.b
    }
}

/// A player, designated by the address of its record (register A0 of the original).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Player(pub u32);

// Fields of the player record (0x200 bytes), declared once (`record!`): the offset (`Player::HP`) and the typed
// accessors (`p.hp(b)`). This is the record that every family reads (effects, objects, SET, enemies). The common
// object fields (position, speeds, animation, list links) are those of `Obj`. Names from docs/joueur.md; a meaning
// guessed from the code alone is marked (h).
crate::record::record! {
    impl Player {
        /// Byte: slot occupied (0 = free, 1 = active); 2 = falling to the ground (state 4).
        active, set_active, ACTIVE: u8 @ Obj::ACTIVE;
        /// Bit 1: player hidden (invulnerability blinking).
        flags, set_flags, FLAGS: u8 @ Obj::FLAGS;
        /// Player number 0..3.
        index, set_index, INDEX: u8 @ 0x02;
        /// Character 0..11: `CHARACTER >> 1` = class (fighter, cleric, elf, dwarf, magic-user, thief).
        character, set_character, CHARACTER: u8 @ Obj::VARIANT;
        /// Main state (`states::Main`), sub-state (`states::Sub`), action (`states::Action`), step.
        main, set_main, MAIN: u8 @ 0x04;
        sub_state, set_sub_state, SUB: u8 @ 0x05;
        action, set_action, ACTION: u8 @ 0x06;
        step, set_step, STEP: u8 @ 0x07;
        /// Low word of `Obj::X`: its fraction.
        x_fraction, set_x_fraction, X_FRACTION: u16 @ 0x0a;
        /// Low word of `Obj::Z`: its fraction.
        z_fraction, set_z_fraction, Z_FRACTION: u16 @ 0x0e;
        /// Low word of `Obj::Y`: its fraction.
        y_fraction, set_y_fraction, Y_FRACTION: u16 @ 0x12;
        /// Contact with an obstacle: set by the collision with the scenery (0x33fe4).
        blocked, set_blocked, BLOCKED: u8 @ 0x29;
        /// Palette group and number.
        palette_group, set_palette_group, PALETTE_GROUP: u8 @ 0x2a;
        palette, set_palette, PALETTE: u8 @ 0x2b;
        /// Orientation: 0 = faces right, 0x20 = left.
        facing, set_facing, FACING: u8 @ 0x2c;
        /// Collision box set (character class).
        box_set, set_box_set, BOX_SET: u8 @ 0x2e;
        /// Objects in reserve (h), read by the pickup objects.
        reserve, set_reserve, RESERVE: u8 @ 0x31;
        /// Event set by the animation script: 1 landed hit (or throw), 2 second gesture.
        anim_event, set_anim_event, ANIM_EVENT: u8 @ 0x33;
        /// Index of the body box (first byte of `Obj::FRAME_DATA_A`).
        body_box, set_body_box, BODY_BOX: u8 @ 0x38;
        /// Index of the attack box.
        attack_box, set_attack_box, ATTACK_BOX: u8 @ 0x3a;
        /// Index of the ground mark (`sprites.rs`).
        ground_mark, set_ground_mark, GROUND_MARK: u8 @ 0x3b;
        /// Kind of the current hit (< 2: weapon hit) (h).
        attack_kind, set_attack_kind, ATTACK_KIND: u8 @ 0x3c;
        /// Identifier of the attack in progress, compared by the collisions (h).
        attack_id, set_attack_id, ATTACK_ID: u8 @ 0x3e;
        /// Current action as the effects read it (bubbles) (h).
        bubble_action, set_bubble_action, BUBBLE_ACTION: u8 @ 0x3d;
        /// Frame of the held weapon, of the low shield, of the raised shield (indices of the `overlays` tables).
        weapon_frame, set_weapon_frame, WEAPON_FRAME: u16 @ 0x40;
        shield_low_frame, set_shield_low_frame, SHIELD_LOW_FRAME: u16 @ 0x42;
        shield_high_frame, set_shield_high_frame, SHIELD_HIGH_FRAME: u16 @ 0x44;
        /// Player effect to create at the end of the frame (variant number; 0: nothing).
        pending_effect, set_pending_effect, PENDING_EFFECT: u8 @ 0x46;
        /// Identifier of the current hit (read by `hit_reactions`).
        blow_id, set_blow_id, BLOW_ID: u8 @ 0x47;
        /// Attached weapon and shield sub-objects (words: object addresses; 0: not drawn).
        overlay_body, set_overlay_body, OVERLAY_BODY: u16 @ 0x48;
        overlay_attack, set_overlay_attack, OVERLAY_ATTACK: u16 @ 0x4a;
        /// Animation counter of the screen countdown palette (h).
        pal_step, set_pal_step, PAL_STEP: u8 @ 0x52;
        /// Hit freeze (frames remaining; negative: to clear).
        hitstop, set_hitstop, HITSTOP: u8 @ Obj::COUNTER_53;
        /// Orientation of the aggressor, duration of the freeze received.
        attacker_facing, set_attacker_facing, ATTACKER_FACING: u8 @ 0x56;
        hit_freeze, set_hit_freeze, HIT_FREEZE: u8 @ 0x57;
        /// Result of the attack: bit 0, a strike landed; positive while it strikes (h).
        strike, set_strike, STRIKE: u8 @ 0x58;
        /// The same place read as a word by the dragon that accompanies the camera leader: its horizontal step this
        /// frame (h).
        step_x, set_step_x, STEP_X: u16 @ 0x58;
        /// Style of the strike in progress (h).
        strike_style, set_strike_style, STRIKE_STYLE: u8 @ 0x59;
        /// Last edge of the screen touched (1 left, 2 right).
        edge, set_edge, EDGE: u8 @ 0x5a;
        /// Strikes landed, set by the collision engine (h).
        strike_hits, set_strike_hits, STRIKE_HITS: u8 @ 0x5b;
        /// Step of the prayer effect read by its sparks (h).
        prayer_step, set_prayer_step, PRAYER_STEP: u8 @ 0x5f;
        /// Maximum, current, previous hit points.
        hp_max, set_hp_max, HP_MAX: u16 @ 0x60;
        hp, set_hp, HP: u16 @ 0x62;
        hp_prev, set_hp_prev, HP_PREV: u16 @ 0x64;
        /// Forced slow walk (animation 1, one pixel per frame); cleared at the return from a sub-scene.
        slow, set_slow, SLOW: u8 @ 0x66;
        /// Jump type: 3 vertical, 0 on the spot, 1 forward, 2 after a run.
        jump_kind, set_jump_kind, JUMP_KIND: u8 @ 0x67;
        /// Non-zero: weapon in hand (otherwise the animation numbers are shifted by 0x40).
        armed, set_armed, ARMED: u8 @ 0x68;
        /// Parameter of the current action (fallback animation number, duration...).
        param, set_param, PARAM: u8 @ 0x69;
        /// Kind of strike of the attack (0, 6, 8, 0xa, 0xc, 0xe, 0x16, 0x18, 0x1a, 0x1c, 0x1e).
        subaction, set_subaction, SUBACTION: u8 @ 0x6a;
        /// Second parameter of the action: an attack was already launched during this jump.
        param_6b, set_param_6b, PARAM_6B: u8 @ 0x6b;
        /// Held inputs, from the previous frame, new.
        keys_held, set_keys_held, KEYS_HELD: u8 @ 0x6c;
        keys_prev, set_keys_prev, KEYS_PREV: u8 @ 0x6d;
        keys_new, set_keys_new, KEYS_NEW: u8 @ 0x6e;
        /// Reaction counter (slide, getting up, blinking).
        timer_6f, set_timer_6f, TIMER_6F: u8 @ 0x6f;
        /// Recoil received; cleared when `RECOIL_TIMER` falls back to 0.
        recoil, set_recoil, RECOIL: u8 @ 0x70;
        recoil_timer, set_recoil_timer, RECOIL_TIMER: u8 @ 0x71;
        /// Set by an effect or a hit when it has played for the player (gauge filled, spark) (h).
        mark, set_mark, MARK: u8 @ 0x77;
        /// Set on an ally in range of a healing spell (h).
        ally_mark, set_ally_mark, ALLY_MARK: u8 @ 0x78;
        /// Temporary power bonus and its timer.
        powerup, set_powerup, POWERUP: u8 @ 0x79;
        powerup_timer, set_powerup_timer, POWERUP_TIMER: u8 @ 0x7a;
        /// Rank of the player in the scene (index of the spot tables of the cinematics, order at the end of the bonus
        /// level).
        rank, set_rank, RANK: u8 @ 0x7d;
        /// Invulnerability: frames remaining (0x50 at the return from a sub-scene).
        invuln, set_invuln, INVULN: u8 @ 0x7e;
        /// Step of the aura palette cycle (h).
        aura_step, set_aura_step, AURA_STEP: u8 @ 0x7f;
        /// Held object / carrier (long: object address).
        link, set_link, LINK: u32 @ 0x86;
        /// The player holds something (slow walk of carrying) (h).
        slow_8a, set_slow_8a, SLOW_8A: u8 @ 0x8a;
        /// State of the carried object: 2 lifted (h).
        carry, set_carry, CARRY: u8 @ 0x8b;
        /// Object that grabs the player (word: address; action 0x22).
        holder, set_holder, HOLDER: u16 @ 0x8c;
        /// Characteristics drawn at random at the creation (`init`), from `+0x100`: the first is added to the damage.
        stats, set_stats, STATS: u8 @ 0x100;
        /// Element of the attacks (h).
        element, set_element, ELEMENT: u8 @ 0x107;
        /// Protection: 1/32 slices removed from the damage received.
        protection, set_protection, PROTECTION: u8 @ 0x108;
        /// Base power of weapon A and B.
        power_a, set_power_a, POWER_A: u8 @ 0x109;
        power_b, set_power_b, POWER_B: u8 @ 0x10a;
        /// Defense (h).
        defense, set_defense, DEFENSE: u8 @ 0x10b;
        /// Initial level (table per scene).
        level, set_level, LEVEL: u8 @ 0x10c;
        /// Held weapon and shield / second weapon (numbers), see the P1 tables.
        weapon_a, set_weapon_a, WEAPON_A: u8 @ 0x10d;
        weapon_b, set_weapon_b, WEAPON_B: u8 @ 0x10e;
        /// Non-zero: blocks buttons and bonuses, protected from the attacks (h).
        power_10f, set_power_10f, POWER_10F: u8 @ 0x10f;
        /// Score (BCD, long) (h).
        score_bcd, set_score_bcd, SCORE_BCD: u32 @ 0x110;
        /// Gold (BCD; six digits in bytes 0x112-0x117).
        gold, set_gold, GOLD: u32 @ 0x114;
        /// Two longs after the gold, 0xffffffff without a name (h).
        name_head, set_name_head, NAME_HEAD: u32 @ 0x118;
        name_tail, set_name_tail, NAME_TAIL: u32 @ 0x11c;
        /// Equipment: one item number per slot, from `+0x120`; their quantities from `+0x128`.
        slot_table, set_slot_table, SLOT_TABLE: u8 @ 0x120;
        slot_counts, set_slot_counts, SLOT_COUNTS: u8 @ 0x128;
        /// Inventory: six item slots (0x56: empty); the first is the item in use.
        inventory, set_inventory, INVENTORY: u8 @ 0x140;
        /// Quantities of the six inventory slots (at most 9); the first: charges of the item in use.
        quantities, set_quantities, QUANTITIES: u8 @ 0x148;
        /// Spell reserve (three longs then a word, `init`).
        spell_reserve, set_spell_reserve, SPELL_RESERVE: u32 @ 0x150;
        spell_reserve_b, set_spell_reserve_b, SPELL_RESERVE_B: u32 @ 0x154;
        spell_reserve_c, set_spell_reserve_c, SPELL_RESERVE_C: u16 @ 0x158;
        /// Player effect created by button 3 (word: address).
        special_effect, set_special_effect, SPECIAL_EFFECT: u16 @ 0x15a;
        /// A special move is in progress (h): with `SLOW` and `SLOW_8A`, the player is busy.
        special_active, set_special_active, SPECIAL_ACTIVE: u8 @ 0x15c;
        /// Row of the spells / special items table (16 bytes per row).
        special_count, set_special_count, SPECIAL_COUNT: u8 @ 0x15d;
        /// Row chosen in the special attacks / items table (`checks::special_attack`).
        special_kind, set_special_kind, SPECIAL_KIND: u8 @ 0x15e;
        /// New equipment to put on (swap item).
        special_id, set_special_id, SPECIAL_ID: u8 @ 0x15f;
        /// Worn pieces (h): eight bytes from `+0x160`; the second is the class.
        worn, set_worn, WORN: u32 @ 0x160;
        worn_class, set_worn_class, WORN_CLASS: u8 @ 0x161;
        /// Non-zero: normal walk 2 pixels per frame, otherwise fast 3.
        speed_class, set_speed_class, SPEED_CLASS: u8 @ 0x162;
        /// Piece worn in slot 4.
        worn_4, set_worn_4, WORN_4: u8 @ 0x164;
        /// Talisman worn (3) (h).
        talisman, set_talisman, TALISMAN: u8 @ 0x165;
        /// Armor worn (4) (h).
        armor, set_armor, ARMOR: u8 @ 0x166;
        /// 9: the next item bought is free; 2: a gift (h).
        free_item, set_free_item, FREE_ITEM: u8 @ 0x167;
        /// Counter specific to the action (frames, gesture countdowns).
        action_timer, set_action_timer, ACTION_TIMER: u16 @ 0x168;
        /// The other players are frozen by the spell (1).
        spell_frozen, set_spell_frozen, SPELL_FROZEN: u8 @ 0x16a;
        /// Shop visit counter (h).
        shop_visit, set_shop_visit, SHOP_VISIT: u8 @ 0x16b;
        /// Damage cap of the weapons.
        damage_cap, set_damage_cap, DAMAGE_CAP: u8 @ 0x16e;
        /// Kind of the last contact with an object (2, 4, 6, 8) (h).
        contact, set_contact, CONTACT: u8 @ 0x16f;
        /// Auras: blinking timer (word, also the potion timer), three palette changes.
        aura_timer, set_aura_timer, AURA_TIMER: u16 @ 0x180;
        aura, set_aura, AURA: u8 @ 0x182;
        aura_b, set_aura_b, AURA_B: u8 @ 0x183;
        aura_c, set_aura_c, AURA_C: u8 @ 0x184;
        /// Hit during the reaction: the reaction is replayed.
        rehit, set_rehit, REHIT: u8 @ 0x185;
        /// Enemy holding the player (16-bit pointer) and "held" flag (reaction 0x32; read by the enemies that grab).
        holder_enemy, set_holder_enemy, HOLDER_ENEMY: u16 @ 0x186;
        held, set_held, HELD: u8 @ 0x188;
        /// The spell is cancelled (by a protection): only the failure spark is played.
        spell_cancelled, set_spell_cancelled, SPELL_CANCELLED: u8 @ 0x189;
        /// Holy pose: the victory animation is 36 instead of 35 (h).
        holy, set_holy, HOLY: u8 @ 0x18a;
        /// Glow timers of the special weapon, the shield and the ring.
        glow_weapon, set_glow_weapon, GLOW_WEAPON: u8 @ 0x18b;
        glow_shield, set_glow_shield, GLOW_SHIELD: u8 @ 0x18c;
        glow_ring, set_glow_ring, GLOW_RING: u8 @ 0x18d;
        /// Kind of the current movement (h): 1 jump or charge, 2 spinning attack; added to the damage.
        move_kind, set_move_kind, MOVE_KIND: u8 @ 0x18e;
        /// Movement carried by the current attack (h): 1 the attack box stays active.
        move_state, set_move_state, MOVE_STATE: u8 @ 0x18f;
        /// Kills counted by the spells (word, +2 per foe) (h).
        spell_kills, set_spell_kills, SPELL_KILLS: u16 @ 0x190;
        /// Glow timer of the other pieces, charge of the wand.
        glow_other, set_glow_other, GLOW_OTHER: u8 @ 0x191;
        /// Cleared when the player gets up (h).
        get_up_mark, set_get_up_mark, GET_UP_MARK: u8 @ 0x192;
        /// Magic used during the game (score table) (h).
        magic_used, set_magic_used, MAGIC_USED: u8 @ 0x193;
        /// Counter decreasing with the mask 0x3f (stun) (h).
        freeze_194, set_freeze_194, FREEZE_194: u8 @ 0x194;
        /// Countdown to death.
        dying_timer, set_dying_timer, DYING_TIMER: u8 @ 0x196;
        /// Nine recognizers of special moves of 6 bytes (`combos`).
        recognizers, set_recognizers, RECOGNIZERS: u8 @ 0x1a0;
        /// Timer of the scripts (`scripted`, word) (h).
        script_timer, set_script_timer, SCRIPT_TIMER: u16 @ 0x1e0;
        /// Parity of the double update under the speed aura.
        double_parity, set_double_parity, DOUBLE_PARITY: u8 @ 0x1e3;
        // Scratch of the current action (from +0x1e8): each action uses these bytes its own way; the names give the
        // most common use (h).
        /// Counter of the current action (frames, strokes, side).
        action_count, set_action_count, ACTION_COUNT: u8 @ 0x1e8;
        /// Level of the spell or item in use (chosen with the up and down keys; read by its effects).
        spell_level, set_spell_level, SPELL_LEVEL: u8 @ 0x1e9;
        /// A strike already landed during the action (fighter, thief; read by the shadow shot) (h).
        strike_landed, set_strike_landed, STRIKE_LANDED: u8 @ 0x1ec;
        /// The spell hits an ally in range (1) or only the caster (0).
        spell_area, set_spell_area, SPELL_AREA: u8 @ 0x1ee;
        /// Blinks of the spell in progress (byte).
        spell_blinks, set_spell_blinks, SPELL_BLINKS: u8 @ 0x1ea;
        /// Phase or flashes of the spell in progress.
        spell_phase, set_spell_phase, SPELL_PHASE: u8 @ 0x1eb;
        /// Repetitions of the item or spell effect.
        spell_repeat, set_spell_repeat, SPELL_REPEAT: u8 @ 0x1ed;
        /// A second strike already landed during the action (elf, magic-user) (h).
        second_strike_landed, set_second_strike_landed, SECOND_STRIKE_LANDED: u8 @ 0x1ef;
        /// Undead the turning can still affect: type 1, type 5 (cleric).
        undead_left, set_undead_left, UNDEAD_LEFT: u8 @ 0x1f0;
        undead_left_b, set_undead_left_b, UNDEAD_LEFT_B: u8 @ 0x1f1;
    }
}


impl Player {
    /// End (excluded) of the BCD digits of the gold (bytes `GOLD + 2`..`GOLD_END`).
    pub const GOLD_END: u32 = 0x118;

    /// The player record that the object `o` is (an object designated as a player by another one).
    pub fn of(o: Obj) -> Player {
        Player(o.0)
    }

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
