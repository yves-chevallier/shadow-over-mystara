//! Scene flow: scene entry, spawn scripts, transitions. See `docs/scenes.md`.
//!
//! A game is a sequence of *scenes* (`A5+0x34`, 1 to 114: levels, bosses, shops, attract
//! demos). The game loop (`game.rs`) calls the current *phase* handler (`A5+0x2c`, step of 4)
//! every frame; this module rewrites those that orchestrate the scenes:
//!
//! | Phase | Role | Module |
//! |---|---|---|
//! | 8 | scene entry: initialisation, then opening sequence specific to the scene | [`entry`] |
//! | 12 | the game; the spawn script reader runs every frame | [`script`] (+ `game.rs`) |
//! | 24 | scripted sequences (boss arrival, forced scrolling) | [`intermission`] |
//!
//! Each scene sequence is a two-level state machine: the step (`A5+0x2d`) and the
//! sub-step (`A5+0x2e`, step of 2); it lives in work RAM, never on the stack, so
//! that each phase takes one step per frame and returns (no yield to the scheduler).

pub mod blackout;
pub mod bonus;
pub mod characters;
pub mod entry;
pub mod exit;
pub mod intermission;
pub mod screen;
pub mod script;
pub mod shop;
pub mod tally;
pub mod tiles;
pub mod transition;

use cps2::Cps2Bus;
use m68k::decode::Fetch;
use m68k::Cpu;

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::objects::{pools, Obj};
use crate::Native;

/// Work RAM variables (offsets from A5) shared by the scene sequences.
pub mod var {
    /// Game phase (byte, step of 4), step (byte), sub-step (word, step of 2).
    pub const PHASE: u32 = 0x2c;
    pub const STEP: u32 = 0x2d;
    pub const SUB: u32 = 0x2e;
    /// Number of players in the game (alive at scene entry).
    pub const PLAYERS: u32 = 0x35;
    /// Scene number (byte).
    pub const SCENE: u32 = 0x34;
    /// Non-zero byte: the scene is in transition, the world waits.
    pub const HOLD: u32 = 0x33;
    /// Non-zero byte: the players no longer respond to commands.
    pub const PLAYERS_FROZEN: u32 = 0x4a;
    /// Mask of locked buttons (read by the input copy).
    pub const INPUT_LOCK: u32 = 0x43;
    /// Palette fade: page mask, number of steps left, delay between steps.
    /// Main camera and alternate camera (chosen if `USE_ALT_CAMERA` is non-zero).
    pub const CAMERA: u32 = 0x4fe;
    pub const ALT_CAMERA: u32 = 0x54e;
    pub const USE_ALT_CAMERA: u32 = 0x36;
}

/// Player records: four of 0x200 bytes.
pub const PLAYERS: u32 = A5 + 0x5de;
pub const PLAYER_STRIDE: u32 = 0x200;
/// Player fields read by the sequences.
pub mod player {
    pub const ACTIVE: u32 = 0x00;
    /// Player automaton sub-state (byte): the sequences wait for it to reach a value.
    pub const STATE: u32 = 0x06;
    pub const HEALTH: u32 = 0x62;
}

/// Address of a temporary sequence variable (word of the page A5-0x8000..A5-0x6000, designated
/// in the original by its 16-bit displacement relative to A5, `0x9a2a` = A5 - 0x65d6).
pub const fn scratch(offset16: u16) -> u32 {
    A5 - 0x1_0000 + offset16 as u32
}

/// The brightness fade driven by the fade tasks (`tasks/fading.rs`): pages concerned, steps left,
/// delay between two steps; and the entries of these tasks, which the screens create in slot 0x10.
pub mod fading {
    use crate::ram::{Word, Byte};

    /// Pages concerned (bit k: page k).
    pub const PAGES: Byte = Byte::a5(0x48e);
    /// Steps left (zero: the fade is over).
    pub const STEP: Byte = Byte::a5(0x48f);
    /// Delay between two steps (frames).
    pub const DELAY: Word = Word::a5(0x490);

    /// Tasks: full rise and descent, descent and rise by the state's number of steps, fade to white,
    /// per-component fade from white to the palettes.
    pub const RISE: u32 = 0x14ba;
    pub const DESCENT: u32 = 0x14d0;
    pub const DESCENT_STEP: u32 = 0x14de;
    pub const RISE_STEP: u32 = 0x14c8;
    pub const TO_WHITE: u32 = 0x1ccf8;
    pub const FROM_WHITE: u32 = 0x1ced4;

    /// Creates the fade task `entry` in slot 0x10 (`jsr 0x9b6`).
    pub fn launch(b: &mut cps2::Cps2Bus, entry: u32) {
        crate::kernel::create_task(b, 0x10, entry);
    }
}


/// Context of a scene sequence: the machine, and named accesses to work RAM.
pub struct Ctx<'a> {
    pub c: &'a mut Cpu,
    pub b: &'a mut Cps2Bus,
}

impl<'a> Ctx<'a> {
    pub fn new(c: &'a mut Cpu, b: &'a mut Cps2Bus) -> Self {
        Ctx { c, b }
    }

    // --- work RAM variables ---------------------------------------------------------

    pub fn v8(&mut self, off: u32) -> u8 {
        rd8(self.b, A5 + off)
    }
    pub fn v16(&mut self, off: u32) -> u16 {
        rd16(self.b, A5 + off)
    }
    pub fn set_v8(&mut self, off: u32, v: u8) {
        wr8(self.b, A5 + off, v)
    }
    pub fn set_v16(&mut self, off: u32, v: u16) {
        wr16(self.b, A5 + off, v)
    }
    pub fn set_v32(&mut self, off: u32, v: u32) {
        wr32(self.b, A5 + off, v)
    }
    /// Word or long at an absolute address (temporary variables, ROM tables as data).
    pub fn rd16(&mut self, addr: u32) -> u16 {
        rd16(self.b, addr)
    }
    pub fn rd32(&mut self, addr: u32) -> u32 {
        rd32(self.b, addr)
    }
    pub fn rd8(&mut self, addr: u32) -> u8 {
        rd8(self.b, addr)
    }
    pub fn wr16(&mut self, addr: u32, v: u16) {
        wr16(self.b, addr, v)
    }
    /// Program-space word (table addressed relative to the PC in the original).
    pub fn prog16(&mut self, addr: u32) -> u16 {
        self.b.fetch16(addr)
    }
    /// Program-space byte.
    pub fn prog8(&mut self, addr: u32) -> u8 {
        let word = self.b.fetch16(addr & !1);
        if addr & 1 == 0 { (word >> 8) as u8 } else { word as u8 }
    }

    // --- steps ----------------------------------------------------------------------------------

    /// Current sub-step (low byte of word `A5+0x2e`, always even).
    pub fn sub(&mut self) -> u8 {
        self.v8(var::SUB)
    }
    /// Moves to the next sub-step.
    pub fn next_sub(&mut self) {
        let sub = self.v8(var::SUB).wrapping_add(2);
        self.set_v8(var::SUB, sub);
    }
    pub fn set_sub(&mut self, sub: u8) {
        self.set_v8(var::SUB, sub);
    }
    /// End of the sequence: next phase, steps reset to zero, no more lock.
    pub fn end_scene(&mut self) {
        self.leave_phase();
        self.set_v8(var::HOLD, 0);
        self.set_v8(var::PLAYERS_FROZEN, 0);
    }
    /// Next phase (`A5+0x2c` += 4), step and sub-step to zero.
    pub fn leave_phase(&mut self) {
        let phase = self.v8(var::PHASE).wrapping_add(4);
        self.set_v8(var::PHASE, phase);
        self.set_v8(var::STEP, 0);
        self.set_v16(var::SUB, 0);
    }
    /// Start of a sequence: the scene and the players are locked.
    pub fn lock(&mut self) {
        self.set_v8(var::HOLD, 1);
        self.set_v8(var::PLAYERS_FROZEN, 1);
    }

    /// End of a sequence frame: world update (0x1c86e), then sprite list (0x1de96). No
    /// handler yields here (the original's call by address did not allow it).
    #[track_caller]
    pub fn world(&mut self) {
        // probe `rendering` (see `dispatch::Class::update_no_next`)
        crate::diag::dead_regs(self.c, "handback", crate::diag::TASK_REGS);
        let flow = crate::game::update_world(self.c, self.b);
        assert!(flow == crate::modes::Flow::Done, "scene sequence: a handler ceded control during the world update");
        crate::sprites::build_68k(self.c, self.b);
    }

    /// System object "card" at scene start (SOBJ of type 14, 0xdfe26).
    pub fn title_card(&mut self) {
        let _ = crate::families::creators::sobj_14_pure(self.b);
    }

    /// System object of type 6, of variant `variant` (0xddeaa).
    pub fn sobj_6(&mut self, variant: u8) {
        let _ = crate::effects_aids::scenery_6(self.b, variant);
    }

    /// Copy of the inputs to the players (0x2aec). Original glitch: the D0-D4 it leaves are re-read by
    /// handlers of the world update (`inputs::copy_68k`).
    pub fn inputs(&mut self) {
        crate::inputs::copy_68k(self.c, self.b);
    }

    // --- timers ------------------------------------------------------------------------------

    /// Takes one frame off the timer (word); true when it reaches zero.
    pub fn tick(&mut self, timer: u32) -> bool {
        let left = self.rd16(timer).wrapping_sub(1);
        self.wr16(timer, left);
        left == 0
    }

    // --- fade, objects, messages ----------------------------------------------------------------

    /// Black screen then opening fade: delay 2, all pages, fade task.
    pub fn fade_in(&mut self) {
        fading::DELAY.set(self.b, 2);
        fading::PAGES.set(self.b, 15);
        crate::palette::no_brightness(self.b);
        self.start_fade_task(crate::scene::fading::RISE);
    }
    /// Variant: fade one level per step (`A5+0x48f` = 1), without going through black.
    pub fn fade_in_stepped(&mut self) {
        fading::DELAY.set(self.b, 2);
        fading::PAGES.set(self.b, 15);
        fading::STEP.set(self.b, 1);
        self.start_fade_task(crate::scene::fading::FROM_WHITE);
    }
    fn start_fade_task(&mut self, entry: u32) {
        crate::kernel::create_task(self.b, 0x10, entry);
    }

    /// Creates an active object (allocator `alloc`), of type `kind`; the original writes without testing the carry.
    pub fn spawn(&mut self, pool: &pools::Pool, kind: u8) -> Obj {
        let o = pool.alloc_unchecked(self.b);
        o.set_u8(self.b, Obj::ACTIVE, 1);
        o.set_u8(self.b, Obj::KIND, kind);
        o
    }

    /// Opens message `n` (0xc8312) and stores (as 16 bits) the object's address in `slot`. Original glitch:
    /// the failure is not tested, the A1 left by the allocation is stored.
    pub fn message(&mut self, n: u8, slot: u32) {
        let o = crate::families::creators::scene_set_or_rest(self.b, n);
        self.store_obj(slot, o);
    }
    /// Message finished (byte `+0x22` non-zero)?
    pub fn message_done(&mut self, slot: u32) -> bool {
        let o = self.load_obj(slot);
        o.u8(self.b, 0x22) != 0
    }

    /// Stores an object as 16 bits (`movea.w`/`move.w An` of the original).
    pub fn store_obj(&mut self, slot: u32, o: Obj) {
        self.wr16(slot, o.0 as u16);
    }
    pub fn load_obj(&mut self, slot: u32) -> Obj {
        Obj(self.rd16(slot) as i16 as i32 as u32 & 0xff_ffff)
    }

    // --- players ---------------------------------------------------------------------------------

    /// Are all the players in the game and alive in state `state` (byte `+6`)? Leaves A1 and D6
    /// like the original loop (A1 on the blocking player, or past the last; D6 = remainder).
    pub fn players_in_state(&mut self, state: u8) -> bool {
        for k in 0..4u32 {
            let p = PLAYERS + k * PLAYER_STRIDE;
            let alive = rd8(self.b, p + player::ACTIVE) != 0 && rd16(self.b, p + player::HEALTH) != 0;
            if alive && rd8(self.b, p + player::STATE) != state {
                self.c.a[1] = p;
                self.c.d[6] = 3 - k;
                return false;
            }
        }
        self.c.a[1] = PLAYERS + 4 * PLAYER_STRIDE;
        self.c.d[6] = 0xffff;
        true
    }
}

/// Native routines of scene flow.
pub const NATIVES: &[Native] = &[entry::PHASE_8, entry::PLACE_PLAYERS, intermission::PHASE_24, script::SPAWN_SCRIPT, script::SCRIPT_RESET];

/// Native routines of all the scene modules.
pub fn natives() -> impl Iterator<Item = &'static Native> {
    NATIVES
        .iter()
        .chain(exit::NATIVES)
        .chain(tally::NATIVES)
        .chain(bonus::NATIVES)
        .chain(characters::NATIVES)
        .chain(shop::NATIVES)
        .chain(blackout::NATIVES)
        .chain(screen::NATIVES)
        .chain(transition::natives())
}
