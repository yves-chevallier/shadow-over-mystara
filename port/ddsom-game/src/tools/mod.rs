//! Tools only (feature `tools`): the 68000 and the original code, never the game.
//!
//! * The reference machine of `validate` (original code, overclocked 68000: [`install_reference`], [`idle`], the
//!   patches of the original's undefined behaviours).
//! * The natives at the original addresses for the original code that calls them, with their arguments and results in
//!   the registers of the original (`Body::WithCpu`, `Body::CpuIn`; submodules mirroring the game modules): the
//!   differential fuzz (`fuzz_*`) and the call by call check (`DDSOM_CHECK`) run them against the original routine
//!   on the same state.
//! * Running the natives from the processor ([`Native::execute`], [`run`], [`run_traced`], [`call`]), the check mode
//!   (`set_check(true)`: each call also runs the original routine on a copy of the state and compares the two; the
//!   differences are counted in `report()`).
//!
//! Each native lasts [`NATIVE_DURATION`] cycles on the processor.

// The adapters keep the imports of the game modules they come from (cleaned up as the tools shrink).
#![allow(unused_imports)]

pub mod ai;
pub mod ai_aids;
pub mod aids;
pub mod camera_modes;
pub mod combat;
pub mod credits;
pub mod credits_task;
pub mod diag;
pub mod dispatch;
pub mod eeprom;
pub mod effects_aids;
pub mod events;
pub mod families;
pub mod frames;
pub mod hud;
pub mod inputs;
pub mod kernel;
pub mod modes;
pub mod objects;
pub mod ops;
pub mod palette;
pub mod placement;
pub mod player;
pub mod popups;
pub mod rng;
pub mod scene;
pub mod scroll;
pub mod service;
pub mod sound;
pub mod sprites;
pub mod terrain;
pub mod text;
pub mod video;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::machine::Machine;
use crate::{lookup, mem, no68k, regs, tasks, Body, Native};

/// Duration of a native run by the processor (68000 cycles), the same for all.
pub const NATIVE_DURATION: u64 = 100;

impl Native {
    /// Runs the routine from the processor, return (`rts`) included.
    pub fn execute(&self, c: &mut Cpu, b: &mut Cps2Bus) {
        match self.run {
            Body::WithCpu(f) => {
                f(c, b);
                frames::apply_tools_return(c, b);
            }
            Body::CpuIn(f) => {
                f(c, b, self.entry);
                frames::apply_tools_return(c, b);
            }
            Body::Memory(f) => {
                f(b);
                frames::rts(c, b);
            }
            Body::Handler(f) => {
                // called from the processor: the object (A0), the counter (D7) and the leftovers (D2-D5) come from the
                // registers and the leftovers return there; on the game machine they are also those of the task
                // table. No table is created on a machine without one (the original's returns stay on its stack).
                let mut a = crate::dispatch::Dispatch::from_cpu(c);
                if let Some(t) = tasks::table::table_if(b) {
                    t.leftovers = a.leftovers;
                }
                f(b, &mut a);
                if let Some(t) = tasks::table::table_if(b) {
                    t.leftovers = a.leftovers;
                }
                a.give_back(c);
                frames::rts(c, b);
            }
        }
    }
}

/// The natives for the original code (arguments and results in its registers), at the original addresses: the tools
/// only (`crate::all_natives` adds them to those of the game).
pub fn natives() -> impl Iterator<Item = &'static Native> {
    use crate::tools;
    static LIST: OnceLock<Vec<&'static Native>> = OnceLock::new();
    LIST.get_or_init(|| {
        let lists: &[&[Native]] = &[
            &[tools::aids::DIRECTION],
            &[tools::aids::DIRECTION_FROM_OBJECT],
            &[tools::aids::DIRECTION_TO_OBJECT],
            &[tools::aids::DIVIDE],
            &[tools::aids::GAME_OVER],
            &[tools::aids::GRABBED],
            &[tools::camera_modes::CAMERA1_UPDATE],
            &[tools::camera_modes::CAMERA2_UPDATE],
            &[tools::combat::COMBAT_UPDATE],
            &[tools::eeprom::ACCOUNTING_INIT],
            &[tools::eeprom::COUNT_COINS],
            &[tools::eeprom::COUNT_CONTINUES],
            &[tools::eeprom::COUNT_GAMES],
            &[tools::eeprom::COUNT_GAMES_ACC],
            &[tools::eeprom::COUNT_STARTS],
            &[tools::eeprom::FULL_COINS_NATIVE],
            &[tools::eeprom::FULL_STARTS_NATIVE],
            &[tools::eeprom::READ],
            &[tools::eeprom::READ_VERIFY],
            &[tools::eeprom::STAT_CHARACTER],
            &[tools::eeprom::STAT_DURATION],
            &[tools::eeprom::VBLANK_ACCOUNTING],
            &[tools::eeprom::WRITE],
            &[tools::eeprom::WRITE_VERIFY],
            &[tools::events::CLEAR_COUNTER],
            &[tools::events::CLEAR_POINTS],
            &[tools::events::CLEAR_POPUPS],
            &[tools::events::CLEAR_POPUP],
            &[tools::events::CREATE_POPUP],
            &[tools::events::CREDIT_COUNTER_NATIVE],
            &[tools::events::CREDIT_POINTS_NATIVE],
            &[tools::events::ENQUEUE_COUNTER],
            &[tools::events::ENQUEUE_POINTS],
            &[tools::events::OBJECT_EVENT],
            &[tools::families::reactions::PLAYER_HURT_ANIMATION],
            &[tools::inputs::CLEAR_PLAYER_INPUTS],
            &[tools::inputs::COPY_INPUTS_TO_PLAYERS],
            &[tools::inputs::GET_PLAYER_INPUT],
            &[tools::inputs::READ_INPUTS],
            &[tools::modes::demo::DEMO_PHASE_12],
            &[tools::modes::demo::RECORDER_RESET],
            &[tools::modes::ending::PHASE_28],
            &[tools::modes::gameover::PHASE_20],
            &[tools::modes::select::PHASE_4],
            &[tools::objects::anim::ANIM_START],
            &[tools::objects::anim::ANIM_START_INDEXED],
            &[tools::objects::anim::ANIM_START_LONG_BY_CHAR],
            &[tools::objects::anim::ANIM_START_LONG_BY_VARIANT],
            &[tools::objects::anim::ANIM_START_LONG_DOUBLE],
            &[tools::objects::anim::ANIM_STEP],
            &[tools::objects::anim::ANIM_STEP_LONG],
            &[tools::objects::cull::CULL_OFFSCREEN],
            &[tools::objects::cull::IN_ARENA],
            &[tools::objects::cull::IN_ARENA_DEFAULT],
            &[tools::objects::cull::ON_SCREEN],
            &[tools::objects::lists::REINSERT_ACTORS],
            &[tools::objects::lists::REINSERT_FOREGROUND],
            &[tools::objects::lists::UNLINK],
            &[tools::objects::lists::UNLINK_ALL],
            &[tools::objects::motion::COUNTDOWNS],
            &[tools::objects::motion::MOVE_BY_TABLE_XY],
            &[tools::objects::motion::MOVE_BY_TABLE_XZ],
            &[tools::objects::motion::MOVE_XYZ],
            &[tools::objects::motion::MOVE_XY],
            &[tools::objects::motion::MOVE_XZ],
            &[tools::objects::motion::MOVE_X],
            &[tools::objects::motion::MOVE_YZ],
            &[tools::objects::motion::MOVE_Y],
            &[tools::objects::motion::MOVE_Z],
            &[tools::objects::motion::PAUSE_HOOK],
            &[tools::objects::motion::SPEED_TABLE_A],
            &[tools::objects::motion::SPEED_TABLE_B],
            &[tools::objects::pools::ALLOC_BOSS],
            &[tools::objects::pools::ALLOC_BOX],
            &[tools::objects::pools::ALLOC_EFFCT],
            &[tools::objects::pools::ALLOC_ENEMY],
            &[tools::objects::pools::ALLOC_ESHL],
            &[tools::objects::pools::ALLOC_ITEM],
            &[tools::objects::pools::ALLOC_PEFF],
            &[tools::objects::pools::ALLOC_PSHL],
            &[tools::objects::pools::ALLOC_SET],
            &[tools::objects::pools::ALLOC_SOBJ],
            &[tools::objects::pools::FREE_BOSS],
            &[tools::objects::pools::FREE_BOX],
            &[tools::objects::pools::FREE_EFFCT],
            &[tools::objects::pools::FREE_ENEMY],
            &[tools::objects::pools::FREE_ESHL],
            &[tools::objects::pools::FREE_ITEM],
            &[tools::objects::pools::FREE_PEFF],
            &[tools::objects::pools::FREE_PSHL],
            &[tools::objects::pools::FREE_SET],
            &[tools::objects::pools::FREE_SOBJ],
            &[tools::objects::pools::INIT_ALL],
            &[tools::objects::pools::INIT_EFFCT],
            &[tools::objects::pools::INIT_PEFF],
            &[tools::objects::pools::INIT_SET],
            &[tools::objects::pools::INIT_SOBJ],
            &[tools::objects::rank::RANK_LOAD_PRESET],
            &[tools::objects::rank::RANK_LOWER],
            &[tools::objects::rank::RANK_RAISE],
            &[tools::objects::rank::RANK_RESET],
            &[tools::objects::rank::RANK_TICK],
            &[tools::palette::APPROACH_CONSTANT],
            &[tools::palette::APPROACH_PAGE],
            &[tools::palette::BRIGHTEN_PAGE],
            &[tools::palette::CHARACTER_PALETTE],
            &[tools::palette::FADE_TO_COLORS],
            &[tools::palette::FADE_TO_PALETTES],
            &[tools::palette::FADE_TO_WHITE],
            &[tools::palette::FILL_PAGE],
            &[tools::palette::FLASH_A],
            &[tools::palette::FLASH_B],
            &[tools::palette::FLASH_COLOR_ON],
            &[tools::palette::FLASH_COLOR_WHITE],
            &[tools::palette::FLASH_COLOR_WHITE_B],
            &[tools::palette::FLASH_C],
            &[tools::palette::FLASH_D],
            &[tools::palette::FLASH_E],
            &[tools::palette::FLASH_F],
            &[tools::palette::FLASH_GRAY],
            &[tools::palette::FLASH_G],
            &[tools::palette::FLASH_RESTORE],
            &[tools::palette::FLASH_SAVE],
            &[tools::palette::FULL_BRIGHTNESS],
            &[tools::palette::LOAD_ALL],
            &[tools::palette::LOAD_SET],
            &[tools::palette::NO_BRIGHTNESS],
            &[tools::palette::OR_BRIGHTNESS],
            &[tools::palette::PAGE1_D0],
            &[tools::palette::PAGE1_STAGE],
            &[tools::palette::PAGE2_D0],
            &[tools::palette::PAGE2_STAGE],
            &[tools::palette::PAGE3_D0],
            &[tools::palette::PAGE3_STAGE],
            &[tools::palette::SAVE_PAGE1],
            &[tools::palette::SET_BRIGHTNESS],
            &[tools::palette::STAGE_FREE],
            &[tools::palette::STAGE_PAGE0],
            &[tools::palette::STAGE_SCROLL1],
            &[tools::player::actions::START_STAND],
            &[tools::player::actions::TO_STAND],
            &[tools::player::moves::HOP_BACK],
            &[tools::player::moves::LEAP],
            &[tools::player::overlays::UPDATE],
            &[tools::player::spawn::AURA],
            &[tools::player::spawn::BANISH],
            &[tools::player::spawn::BEAM],
            &[tools::player::spawn::BEAM_CAST],
            &[tools::player::spawn::BLESSING],
            &[tools::player::spawn::BLOOD],
            &[tools::player::spawn::CURE],
            &[tools::player::spawn::CURSED_WEAPON],
            &[tools::player::spawn::DASH],
            &[tools::player::spawn::DYING_SHARDS],
            &[tools::player::spawn::FIREBALL],
            &[tools::player::spawn::FLARE],
            &[tools::player::spawn::HOLY_LIGHT],
            &[tools::player::spawn::ICE_COLUMNS],
            &[tools::player::spawn::ICE_CROWN],
            &[tools::player::spawn::ICE_GROUND],
            &[tools::player::spawn::ICE_MIST],
            &[tools::player::spawn::ICE_SHARDS],
            &[tools::player::spawn::KNIFE],
            &[tools::player::spawn::LEAP],
            &[tools::player::spawn::LIGHTNING],
            &[tools::player::spawn::LIGHTNING_TARGET],
            &[tools::player::spawn::METEORS],
            &[tools::player::spawn::MOON],
            &[tools::player::spawn::MOON_B],
            &[tools::player::spawn::PORTAL],
            &[tools::player::spawn::PORTAL_BURST],
            &[tools::player::spawn::PORTAL_GLOW],
            &[tools::player::spawn::PORTAL_SPARKS],
            &[tools::player::spawn::POTION],
            &[tools::player::spawn::RAIN],
            &[tools::player::spawn::RAIN_GLOW],
            &[tools::player::spawn::RING],
            &[tools::player::spawn::ROD_BALL_SHOT],
            &[tools::player::spawn::ROD_FLAME_SHOT],
            &[tools::player::spawn::ROD_STAR_SHOT],
            &[tools::player::spawn::SHADOWS],
            &[tools::player::spawn::SHOWER],
            &[tools::player::spawn::SLASH],
            &[tools::player::spawn::STORM],
            &[tools::player::spawn::STORM_CLOUD],
            &[tools::player::spawn::STORM_SPELL],
            &[tools::player::spawn::SUMMON],
            &[tools::player::spawn::SWAP],
            &[tools::player::spawn::SWARM],
            &[tools::player::spawn::THROWN_A],
            &[tools::player::spawn::THROWN_B],
            &[tools::player::spawn::THROWN_C],
            &[tools::player::spawn::THROWN_D],
            &[tools::player::spawn::THROWN_E],
            &[tools::player::spawn::THROWN_F],
            &[tools::player::spawn::THROWN_G],
            &[tools::player::spawn::THROWN_H],
            &[tools::player::spawn::WALL_SPARK],
            &[tools::player::spawn::WALL_TARGET],
            &[tools::player::spawn::WAVE_BOLT],
            &[tools::player::spawn::WEAPON_TRAIL],
            &[tools::player::spawn::WITCH],
            &[tools::player::spawn::WITCH_A],
            &[tools::player::spawn::WITCH_B],
            &[tools::player::update::UPDATE_PLAYERS],
            &[tools::player::walk::CLAMP],
            &[tools::popups::ANIMATE],
            &[tools::rng::NEXT],
            &[tools::rng::SEED],
            &[tools::scene::blackout::BLACKOUT],
            &[tools::scene::bonus::BONUS],
            &[tools::scene::characters::CHARACTERS],
            &[tools::scene::entry::PHASE_8],
            &[tools::scene::entry::PLACE_PLAYERS],
            &[tools::scene::exit::PHASE_16],
            &[tools::scene::shop::SHOP],
            &[tools::scene::tally::TALLY],
            &[tools::scroll::CREEP_3],
            &[tools::scroll::CREEP_6],
            &[tools::scroll::FOLLOW_PLAYERS_Y],
            &[tools::scroll::PARALLAX_QUARTER],
            &[tools::scroll::PARALLAX_THREE_QUARTERS],
            &[tools::scroll::RESET_CAMERAS],
            &[tools::scroll::RETURN_CAMERAS],
            &[tools::scroll::UPDATE_CAMERAS],
            &[tools::sound::ENQUEUE_DIRECT],
            &[tools::sound::ENQUEUE_IF_SOUND],
            &[tools::sound::INIT_PRIVATE_STACK],
            &[tools::sound::NONE],
            &[tools::sound::SERVICE_FF00],
            &[tools::sound::SERVICE_FF05],
            &[tools::sound::SERVICE_FF06],
            &[tools::sound::SERVICE_FF06_D1],
            &[tools::sound::SERVICE_FF07],
            &[tools::sound::SERVICE_FF07_D1],
            &[tools::sound::SERVICE_FF08],
            &[tools::sound::SERVICE_FF09],
            &[tools::sprites::BUILD_SPRITE_LIST],
            &[tools::terrain::SHADOW],
            &[tools::terrain::SHADOW_SECOND],
            &[tools::text::CLEAR_BLOCKS],
            &[tools::text::CLEAR_BOX],
            &[tools::text::DRAW_BLOCKS],
            &[tools::text::ERASE],
            &[tools::text::PRINT],
            &[tools::text::PRINT_OVER],
            &[tools::text::PRINT_TALL],
            &[tools::text::PRINT_TALL_AT],
            &[tools::video::CLEAR_SCROLL1_START],
            &[tools::video::DECIMAL_DIGITS],
            &[tools::video::DISABLE_LAYERS],
            &[tools::video::ENABLE_LAYERS],
            &[tools::video::GAUGE_NATIVE],
            &[tools::video::GAUGE_THUNK_NATIVE],
            &[tools::video::GAUGE_VALUE_NATIVE],
            &[tools::video::HEX_DIGITS],
            &[tools::video::LOAD_BLOCKS_4X4],
            &[tools::video::LOAD_BLOCKS_SCROLL2],
            &[tools::video::LOAD_BLOCKS_SMALL],
            &[tools::video::LOCK],
            &[tools::video::TERRAIN],
            &[tools::video::TERRAIN_OF_OBJECT],
            &[tools::video::UNLOCK],
            tools::dispatch::NATIVES,
            tools::effects_aids::NATIVES,
            tools::families::boss_3::NATIVES,
            tools::families::boss_common::NATIVES,
            tools::families::creators::NATIVES,
            tools::families::effects_creators::NATIVES,
            tools::families::enemy_common::NATIVES,
            tools::families::eshl_creators::NATIVES,
            tools::families::set_residue::NATIVES,
            tools::families::small::NATIVES,
            tools::kernel::NATIVES,
            tools::placement::NATIVES,
            tools::player::init::NATIVES,
        ];
        lists.iter().flat_map(|l| l.iter()).chain(tools::sound::native_wrappers()).collect()
    })
    .iter()
    .copied()
}

/// Calls a routine by its address from a native routine, like a `jsr`. In the game, the native runs
/// immediately and its `rts` closes the call (`tasks::table::open_call`): no PC, no 68000 stack. In the tools, the
/// natives and the original code run until the return to a pushed sentinel address. The routine must neither
/// cede control nor return higher than its caller: the calls that may do so have a continuation (Rust step, see
/// `tasks::table::call` and [`call_expected`]).
#[track_caller]
pub fn call(c: &mut Cpu, b: &mut Cps2Bus, addr: u32) {
    // never run by the game
    const RETURN: u32 = 0x00_fffe;
    let site = std::panic::Location::caller();
    let _site = (TRACE.get().is_some() || no68k::active()).then(|| CallSite::enter(site, addr));
    let before = (c.d, c.a, c.sr);
    if no68k::active() {
        // the game: the native runs immediately; its `rts` returns to an expected call of the task (no PC, no 68000
        // stack)
        let n = no68k::native(b, addr);
        let frame = tasks::table::open_call(b);
        execute(n, c, b, 0);
        if tasks::has_ceded() || !tasks::table::call_returned(b, frame) {
            panic!("call of {addr:06x} ({site}): the routine ceded control or returned higher than its caller");
        }
        diag::probe_call(c, addr, &before, site);
        return;
    }
    // tools (machine without a Rust task table): natives and original code, until the return to a sentinel
    let sp = c.a[7];
    c.a[7] = sp.wrapping_sub(4);
    mem::wr32(b, c.a[7], RETURN);
    c.pc = addr;
    while !(c.pc == RETURN && c.a[7] == sp) {
        if tasks::has_ceded() || c.pc == TASK_SWITCH || c.sr & 0x2000 == 0 && c.a[7] > sp && c.a[7] - sp < 0x1000 {
            panic!(
                "call of {addr:06x} ({site}): the routine ceded control or returned higher than its caller (PC {:06x}, A7 \
                 {:06x} for {sp:06x}, SR {:04x}, ceded {})",
                c.pc,
                c.a[7],
                c.sr,
                tasks::has_ceded()
            );
        }
        one_step(c, b);
    }
    diag::probe_call(c, addr, &before, site);
}

/// Switch point of the scheduler: a task that cedes control (0xa40, 0xa86) or ends goes through it.
const TASK_SWITCH: u32 = 0x984;

/// Tools: a native, or an original instruction or block, at the PC address.
fn one_step(c: &mut Cpu, b: &mut Cps2Bus) {
    if TRACE.get().is_some() {
        run_traced(c.pc, c, b, u64::MAX);
    } else if run(c.pc, c, b, u64::MAX).is_none() {
        c.step(b);
    }
}

/// 68000 trace (`examples/who_enters`, `recensement`, `fuzz_porte`): function called with the address of each 68000
/// instruction executed outside a native routine.
static TRACE: OnceLock<fn(u32)> = OnceLock::new();

pub fn set_trace(f: fn(u32)) {
    let _ = TRACE.set(f);
}

/// Diagnostics (`examples/who_enters`): called on entry (true) and on exit (false) of each native run
/// by `run_traced`.
static NATIVE_HOOK: OnceLock<fn(u32, bool)> = OnceLock::new();

pub fn set_native_hook(f: fn(u32, bool)) {
    let _ = NATIVE_HOOK.set(f);
}

thread_local! {
    /// Calls of original code in progress during the inventory: call site in Rust and routine called.
    static CALLS: std::cell::RefCell<Vec<(&'static std::panic::Location<'static>, u32)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Call in progress (restored at the end of the call).
struct CallSite;

impl CallSite {
    fn enter(site: &'static std::panic::Location<'static>, routine: u32) -> Self {
        CALLS.with(|a| a.borrow_mut().push((site, routine)));
        CallSite
    }
}

impl Drop for CallSite {
    fn drop(&mut self) {
        CALLS.with(|a| a.borrow_mut().pop());
    }
}

/// Inventory: the innermost Rust call in progress (site and routine called), if there is one. The site is
/// the call of `call` or one of its wrappers (`Ctx::call`, `Cx::call`...).
pub fn call_site() -> Option<(&'static std::panic::Location<'static>, u32)> {
    CALLS.with(|a| a.borrow().last().copied())
}

/// Inventory executor: native routines, otherwise one 68000 instruction at a time, reported to
/// `set_trace`. The durations are not checked (the 68000 is overclocked).
pub fn run_traced(pc: u32, c: &mut Cpu, b: &mut Cps2Bus, _dl: u64) -> Option<u32> {
    if let Some(n) = lookup(pc) {
        let hook = NATIVE_HOOK.get();
        if let Some(h) = hook {
            h(pc, true);
        }
        let frame = tasks::table::open_call_68000(b);
        exec(n, c, b);
        tasks::table::close_calls(b, frame);
        if let Some(h) = hook {
            h(pc, false);
        }
        c.cycles += NATIVE_DURATION;
    } else {
        if let Some(t) = TRACE.get() {
            t(pc);
        }
        c.step(b);
    }
    Some(c.pc)
}

thread_local! {
    /// Natives running inside one another (calls by address, steps).
    static DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Runs a native. A top-level native (scheduler, VBLANK, native chained by a jump) starts with no
/// cession in progress (`tasks::has_ceded`).
fn exec(n: &Native, c: &mut Cpu, b: &mut Cps2Bus) {
    /// Restores the depth, even if the native panics (tools that catch panics).
    struct Level(u32);
    impl Drop for Level {
        fn drop(&mut self) {
            DEPTH.with(|p| p.set(self.0));
        }
    }
    let level = Level(DEPTH.with(|p| p.replace(p.get() + 1)));
    if level.0 == 0 {
        tasks::forget_cession();
    }
    n.execute(c, b);
}

/// Executor: native routine if available, otherwise recompiled block.
pub fn run(pc: u32, c: &mut Cpu, b: &mut Cps2Bus, dl: u64) -> Option<u32> {
    if let Some(n) = lookup(pc) {
        let limit = if n.touches_sound { dl } else { b.irq_horizon };
        if c.cycles + NATIVE_DURATION <= limit || holds_table(n) {
            let frame = tasks::table::open_call_68000(b);
            execute(n, c, b, NATIVE_DURATION);
            tasks::table::close_calls(b, frame);
            return Some(c.pc);
        }
    }
    ddsom_rc::run_block(pc, c, b, dl)
}

/// Runs native `n`, which lasts `duration` cycles, with the requested diagnostics: check against the original
/// (`DDSOM_CHECK`), registers taken from the original (`DDSOM_ADOPT_REGS`), call counting.
fn execute(n: &Native, c: &mut Cpu, b: &mut Cps2Bus, duration: u64) {
    if CHECK.load(Ordering::Relaxed) && checked(n) {
        check(n, c, b);
    } else if adopt_regs(n) {
        // diagnostics: output registers taken from the original (inherited registers)
        let c2 = original(n, c, b);
        exec(n, c, b);
        c.cycles += duration;
        if c.pc == c2.pc {
            // DDSOM_ADOPT_MASK=hex: only the registers of the mask (bits of `regs`) are taken
            let mask = adopt_mask();
            if std::env::var_os("DDSOM_ADOPT_TRACE").is_some() {
                let d: Vec<String> = (0..8).filter(|&i| c.d[i] != c2.d[i]).map(|i| format!("d{i}={:08x}/{:08x}", c.d[i], c2.d[i])).chain((0..7).filter(|&i| c.a[i] != c2.a[i]).map(|i| format!("a{i}={:08x}/{:08x}", c.a[i], c2.a[i]))).collect();
                eprintln!("ADOPT {} {}", n.name, d.join(" "));
            }
            for i in 0..8 {
                if mask & regs::d(i) != 0 {
                    c.d[i as usize] = c2.d[i as usize];
                }
                if i < 7 && mask & regs::a(i) != 0 {
                    c.a[i as usize] = c2.a[i as usize];
                }
            }
            if mask & regs::CCR != 0 {
                c.sr = c2.sr;
            }
        }
    } else {
        exec(n, c, b);
        c.cycles += duration;
        if COUNT.load(Ordering::Relaxed) {
            STATS.lock().unwrap().entry(n.name).or_default().calls += 1;
        }
    }
}

/// Natives that keep their state in the Rust task table (kernel, VBLANK, startup): the original
/// code, which would look for it in RAM, must never run in their place, even when they overflow onto
/// the next interrupt (which only happens in an original-code call that lasts).
fn holds_table(n: &Native) -> bool {
    kernel::NATIVES.iter().any(|k| k.entry == n.entry)
}

/// Executor: native routines only (the rest is left to the interpreter). Diagnostic tools.
pub fn run_native_only(pc: u32, c: &mut Cpu, b: &mut Cps2Bus, dl: u64) -> Option<u32> {
    let n = lookup(pc)?;
    let limit = if n.touches_sound { dl } else { b.irq_horizon };
    if c.cycles + NATIVE_DURATION > limit {
        return None;
    }
    let frame = tasks::table::open_call_68000(b);
    n.execute(c, b);
    tasks::table::close_calls(b, frame);
    c.cycles += NATIVE_DURATION;
    Some(c.pc)
}

/// Patches of the reference machine (`cps2::Machine::patches`): undefined behaviours of the original that the game
/// reproduces in a fixed way.
fn patches() -> Vec<cps2::Patch> {
    let mut v = vec![
        cps2::Patch { entry: crate::player::scripted::END_BY_SCENE_ENTRY, apply: player::scripted::patch_end_by_scene },
        cps2::Patch { entry: crate::scene::exit::SHARED_NEXT_ENTRY, apply: scene::exit::patch_shared_next },
        cps2::Patch { entry: crate::service::memory::CHECK, apply: service::patch_instant_check },
    ];
    v.extend(sprites::EMPTY_FRAME_PATCHES);
    v
}

/// Validation reference machine: original code only, with the same step cap as `install`.
pub fn install_reference(m: &mut cps2::Machine) {
    ddsom_rc::install(m);
    m.idle = Some(idle);
    crate::cap_steps(m);
    m.patches = patches();
}

/// Reference machine: the original scheduler (0x932-0x95e) idles: no task ready (table FF0064)
/// and no new frame, or only the freeze task is ready and its next step would do nothing
/// (`tasks::freeze::idle_wait_original`). The game has its own criterion (`no68k`).
pub fn idle(c: &Cpu, b: &Cps2Bus) -> bool {
    const NEW_FRAME: usize = 0x164 / 2; // FF0164 (high byte): set by VBLANK, cleared at the start of a walk
    const TASKS: usize = 0x64 / 2; // FF0064: 16 tasks of 16 bytes, state in the high byte
    (0x932..0x960).contains(&c.pc)
        && b.ram[NEW_FRAME] >> 8 == 0
        && ((0..16).all(|t| b.ram[TASKS + 8 * t] >> 8 < 4) || tasks::freeze::idle_wait_original(&b.ram))
}

static CHECK: AtomicBool = AtomicBool::new(false);
/// Counts the calls of the native routines outside check mode (coverage).
static COUNT: AtomicBool = AtomicBool::new(false);

/// Check mode: compare all the registers, not just the contract (diagnosing an incomplete
/// contract; many differences are then harmless).
static CHECK_ALL: AtomicBool = AtomicBool::new(false);

pub fn set_check_all(on: bool) {
    CHECK_ALL.store(on, Ordering::Relaxed);
}

pub fn set_count(on: bool) {
    COUNT.store(on, Ordering::Relaxed);
}

#[derive(Default, Clone)]
pub struct Stats {
    pub calls: u64,
    /// Calls not compared (`check_handler`: the handler cedes control).
    pub skipped: u64,
    pub mismatches: u64,
    pub first: Option<String>,
    /// Last difference (diagnostic tools).
    pub last: Option<String>,
    /// `CHECK_ALL` mode: number of calls where each register (D0-D7, A0-A7) differs from the original.
    pub regs: [u64; 16],
}

static STATS: Mutex<BTreeMap<&'static str, Stats>> = Mutex::new(BTreeMap::new());

pub fn set_check(on: bool) {
    CHECK.store(on, Ordering::Relaxed);
}

/// Check mode summary per routine.
pub fn report() -> BTreeMap<&'static str, Stats> {
    STATS.lock().unwrap().clone()
}

/// Runs the original routine on a copy and the native on the real state, then compares.
/// DDSOM_ADOPT_REGS=prefix,...: the native routines named this way return the registers of
/// the original (run on a copy). If a difference disappears, one of them leaves a register
/// that a 68000 caller (or the next handler) rereads.
fn adopt_regs(n: &Native) -> bool {
    static PREFIXES: OnceLock<Vec<String>> = OnceLock::new();
    let p = PREFIXES.get_or_init(|| {
        std::env::var("DDSOM_ADOPT_REGS").map(|v| v.split(',').map(str::to_owned).collect()).unwrap_or_default()
    });
    !p.is_empty() && p.iter().any(|x| n.name.starts_with(x.as_str()))
}

fn adopt_mask() -> u32 {
    static MASK: OnceLock<u32> = OnceLock::new();
    *MASK.get_or_init(|| {
        std::env::var("DDSOM_ADOPT_MASK").ok().and_then(|v| u32::from_str_radix(&v, 16).ok()).unwrap_or(regs::ALL)
    })
}

/// Runs the original routine on a copy of the machine until its return; returns the resulting CPU.
fn original(n: &Native, c: &Cpu, b: &Cps2Bus) -> Cpu {
    let (mut c2, mut b2) = (c.clone(), b.clone());
    stack_for_original(n, &mut c2, &mut b2);
    let ret = mem::rd32(&mut b2, c2.a[7]);
    let sp_after = c2.a[7].wrapping_add(4);
    let mut guard = 0;
    while !(c2.pc == ret && c2.a[7] == sp_after) {
        if ddsom_rc::run_block(c2.pc, &mut c2, &mut b2, u64::MAX).is_none() {
            c2.step(&mut b2);
        }
        guard += 1;
        assert!(guard < 1_000_000, "{}: the original routine does not return", n.name);
    }
    c2
}

/// Check of a native in the game (called from Rust: its return is a call in progress held in Rust, not a pushed
/// address): the copy that runs the original receives a sentinel return address, and starts from the entry of the
/// native (the bench PC is not maintained without a 68000: `call` does not set it).
fn stack_for_original(n: &Native, c2: &mut Cpu, b2: &mut Cps2Bus) {
    if no68k::active() {
        c2.a[7] = c2.a[7].wrapping_sub(4);
        mem::wr32(b2, c2.a[7], 0x00_fffe);
        c2.pc = n.entry;
    }
}

/// `DDSOM_CHECK_ONLY=prefix,...`: only the named natives are checked (the others, including those
/// of the kernel that never return to their caller, run without a check).
fn checked(n: &Native) -> bool {
    static ONLY: OnceLock<Option<Vec<String>>> = OnceLock::new();
    let only = ONLY.get_or_init(|| std::env::var("DDSOM_CHECK_ONLY").ok().map(|v| v.split(',').map(str::to_owned).collect()));
    only.as_ref().map_or(true, |p| p.iter().any(|p| n.name.starts_with(p.as_str())))
}

fn check(n: &Native, c: &mut Cpu, b: &mut Cps2Bus) {
    // in the game, a native does not set a PC on return
    let by_frame = no68k::active();
    let pre = if std::env::var("DDSOM_TRACE_ORIG").map_or(false, |v| n.name == v) { Some((c.clone(), b.clone())) } else { None };
    let mut c2 = c.clone();
    let mut b2 = b.clone();
    stack_for_original(n, &mut c2, &mut b2);
    let ret = mem::rd32(&mut b2, c2.a[7]);
    let sp_after = c2.a[7].wrapping_add(4);
    let mut guard = 0;
    while !(c2.pc == ret && c2.a[7] == sp_after) {
        if ddsom_rc::run_block(c2.pc, &mut c2, &mut b2, u64::MAX).is_none() {
            c2.step(&mut b2);
        }
        guard += 1;
        assert!(guard < 1_000_000, "{}: the original routine does not return", n.name);
    }
    n.execute(c, b);
    let mut diffs = Vec::new();
    let mut reg_diffs = [false; 16];
    let outputs = if CHECK_ALL.load(Ordering::Relaxed) { regs::ALL } else { n.outputs };
    for i in 0..8 {
        if outputs & regs::d(i as u32) != 0 && c.d[i] != c2.d[i] {
            diffs.push(format!("d{i}={:08x} instead of {:08x}", c.d[i], c2.d[i]));
            reg_diffs[i] = true;
        }
        // A7 (stack) must always be restored
        if (outputs & regs::a(i as u32) != 0 || i == 7) && c.a[i] != c2.a[i] {
            diffs.push(format!("a{i}={:08x} instead of {:08x}", c.a[i], c2.a[i]));
            reg_diffs[8 + i] = true;
        }
    }
    let sr_mask = if outputs & regs::CCR != 0 { 0xffff } else { 0xff00 };
    if c.sr & sr_mask != c2.sr & sr_mask {
        diffs.push(format!("sr={:04x} instead of {:04x}", c.sr, c2.sr));
    }
    if !by_frame && c.pc != c2.pc {
        diffs.push(format!("pc={:06x} instead of {:06x}", c.pc, c2.pc));
    }
    let sp = c2.a[7] & 0xff_ffff;
    if by_frame {
        // the game keeps its tasks in its Rust table and never writes the kernel RAM of the original (stale on its
        // machine): the kernel RAM is not compared (the tasks that a routine creates are not compared either)
        for i in 0..0xe66 / 2 {
            if tasks::table::kernel_ram(2 * i as u32) {
                b2.ram[i] = b.ram[i];
            }
        }
    }
    mem::diff_bus(b, &b2, sp.saturating_sub(0x400)..sp, &mut diffs);
    // The real duration of the original is taken. The registers outside the contract keep the values of the native
    // routine: if a caller depended on them (incomplete contract), the continuation will diverge and validation will
    // see it.
    c.cycles = c2.cycles;
    if let (Some((mut c3, mut b3)), false) = (pre, diffs.is_empty()) {
        eprintln!("TRACE {} A0={:06x}: {}", n.name, c3.a[0], diffs.join(", "));
        let ret = mem::rd32(&mut b3, c3.a[7]);
        let sp_after = c3.a[7].wrapping_add(4);
        let mut k = 0;
        while !(c3.pc == ret && c3.a[7] == sp_after) && k < 3000 {
            eprintln!("  {:06x} d0={:08x} d1={:08x} d2={:08x} d5={:08x} d6={:08x} a1={:08x}", c3.pc, c3.d[0], c3.d[1], c3.d[2], c3.d[5], c3.d[6], c3.a[1]);
            c3.step(&mut b3);
            k += 1;
        }
    }
    let mut st = STATS.lock().unwrap();
    let s = st.entry(n.name).or_default();
    s.calls += 1;
    for (n, &d) in reg_diffs.iter().enumerate() {
        s.regs[n] += d as u64;
    }
    if !diffs.is_empty() {
        if std::env::var("DDSOM_VERBOSE").is_ok() {
            eprintln!("[{}] pc entry {:06x}: {}", n.name, n.entry, diffs.join(", "));
        }
        s.mismatches += 1;
        if s.first.is_none() {
            s.first = Some(diffs.join(", "));
        }
        s.last = Some(diffs.join(", "));
        // the original state is kept so as not to propagate the difference (with the tools only: the game keeps its own
        // state, its task table and the call in progress are not in the copy's RAM)
        if !by_frame {
            *c = c2;
            *b = b2;
        }
    }
}

/// Context that also holds the processor: the `_68k` adapters (registers of the original) go through it.
pub trait WithCpu: Machine {
    /// The processor and the bus, borrowed together (the `_68k` adapters need both).
    fn machine(&mut self) -> (&mut Cpu, &mut Cps2Bus);
}

/// Stack of the copy that runs the original handler (`check_handler`): in the supervisor stack, which the game no longer
/// uses (`tasks::table::kernel_ram`); the words below it are not compared.
const CHECK_STACK: u32 = 0xff_0e60;
/// Return address of the copy (never run by the game).
const CHECK_RETURN: u32 = 0x00_fffe;
/// Instructions at most for an original handler (it does not return: it cedes control, see `check_handler`).
const CHECK_STEPS: u32 = 1_000_000;

/// `DDSOM_CHECK` in the game: the object handler `n` (an entry of a ROM table, called by the dispatch loop) also runs in
/// its original version, on a copy of the machine whose registers are those that the original loop passes (A0 = the
/// object, D7.w = the counter, D2-D5 = the leftovers, A5); the memory that the two leave is compared (counted in
/// `report()`). The game keeps the state of the Rust handler. A handler whose original cedes control (it reaches
/// the kernel, 0x900-0xb70) or whose Rust version ceded is not compared (`skipped`).
pub fn check_handler(
    n: &'static Native,
    handler: fn(&mut Cps2Bus, &mut crate::dispatch::Dispatch),
    b: &mut Cps2Bus,
    a: &mut crate::dispatch::Dispatch,
) {
    if !CHECK.load(Ordering::Relaxed) || !checked(n) {
        handler(b, a);
        return;
    }
    let mut b2 = b.clone();
    let mut c2 = Cpu::new();
    c2.sr = 0x2700;
    c2.a[0] = a.object.0;
    c2.a[5] = mem::A5;
    c2.d[7] = a.counter as u32;
    (c2.d[2], c2.d[3], c2.d[4], c2.d[5]) = (a.leftovers.d2, a.leftovers.d3, a.leftovers.d4, a.leftovers.d5);
    c2.a[7] = CHECK_STACK - 4;
    mem::wr32(&mut b2, c2.a[7], CHECK_RETURN);
    c2.pc = n.entry;
    let mut steps = 0;
    let mut ceded = false;
    while !(c2.pc == CHECK_RETURN && c2.a[7] == CHECK_STACK) {
        if (0x900..0xb70).contains(&c2.pc) || steps >= CHECK_STEPS {
            ceded = true;
            break;
        }
        if ddsom_rc::run_block(c2.pc, &mut c2, &mut b2, u64::MAX).is_none() {
            c2.step(&mut b2);
        }
        steps += 1;
    }
    handler(b, a);
    let mut st = STATS.lock().unwrap();
    let s = st.entry(n.name).or_default();
    if ceded || tasks::has_ceded() {
        s.skipped += 1;
        return;
    }
    s.calls += 1;
    let mut diffs = Vec::new();
    mem::diff_bus(b, &b2, CHECK_STACK - 0x400..CHECK_STACK, &mut diffs);
    if !diffs.is_empty() {
        let d = format!("A0={:06x}: {}", a.object.0, diffs.join(", "));
        if std::env::var("DDSOM_VERBOSE").is_ok() {
            eprintln!("[{}] handler {:06x} {d}", n.name, n.entry);
        }
        s.mismatches += 1;
        if s.first.is_none() {
            s.first = Some(d.clone());
        }
        s.last = Some(d);
    }
}
