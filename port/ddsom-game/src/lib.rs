//! Game logic of ddsom rewritten in idiomatic Rust.
//!
//! A *native routine* replaces a 68000 routine at its entry address. It produces exactly
//! the same state (memory, registers, flags).
//!
//! The game is the game without a 68000 (`install_no_68k`, module `no68k`): natives only, always run in
//! full, and the kernel's frame loop; an address without a native stops the game.
//!
//! Hybrid mode (`install`, diagnostics): natives and recompiled original code for the rest, on the overclocked 68000.
//! Each native lasts [`NATIVE_DURATION`] cycles; if it does not fit before the line's deadline, the original
//! version runs in its place. This mode no longer has to reproduce the original frame by frame.
//!
//! Check mode (`set_check(true)`): each call also runs the original routine on a copy
//! of the state and compares the two; the differences are counted in `report()`.

pub mod camera_modes;
pub mod config;
pub mod credits;
pub mod startup;
pub mod diag;
pub mod dispatch;
pub mod credits_screen;
pub mod effects_aids;
pub mod eeprom;
pub mod events;
pub mod game;
pub mod hud;
pub mod aids;
pub mod ai;
pub mod ai_aids;
pub mod combat;
pub mod families;
pub mod inputs;
pub mod kernel;
pub mod machine;
pub mod mem;
pub mod modes;
pub mod objects;
pub mod ops;
pub mod palette;
pub mod placement;
pub mod player;
pub mod popups;
pub mod ram;
pub mod rng;
pub mod no68k;
pub mod scene;
pub mod scroll;
pub mod scenario;
pub mod sound;
pub mod terrain;
pub mod service;
pub mod sprites;
pub mod credits_task;
pub mod tasks;
pub mod text;
pub mod video;
pub mod vblank;

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use cps2::Cps2Bus;
use m68k::Cpu;

/// Native routine.
pub struct Native {
    pub entry: u32,
    pub name: &'static str,
    /// The body of the routine (duration not counted: added by the caller, [`NATIVE_DURATION`]).
    pub run: Body,
    /// The routine accesses the RAM shared with the Z80: it must fit within the current line,
    /// otherwise the interleaving with the Z80 would change.
    pub touches_sound: bool,
    /// Contract: registers and flags that the caller may read on return (see `regs`). The other
    /// registers are free; the memory must always be identical.
    pub outputs: u32,
}

/// Body of a native.
#[derive(Clone, Copy)]
pub enum Body {
    /// On the processor: reads registers of the original or leaves some to its callers (contract of a call by
    /// address), and ends with its `rts`.
    WithCpu(fn(&mut Cpu, &mut Cps2Bus)),
    /// Like `WithCpu`, for a body shared by several entries: it receives the address of its own.
    CpuIn(fn(&mut Cpu, &mut Cps2Bus, u32)),
    /// Without registers (the target form: memory only); the return (`rts`) is done for it.
    Memory(fn(&mut Cps2Bus)),
    /// Object handler without registers: it receives what the dispatch loop passes it (the object, the counter,
    /// the leftovers that the handlers pass to each other: `dispatch::Dispatch`); the return is done for it.
    Handler(fn(&mut Cps2Bus, &mut dispatch::Dispatch)),
}

impl Native {
    /// Runs the routine, return (`rts`) included.
    pub fn execute(&self, c: &mut Cpu, b: &mut Cps2Bus) {
        match self.run {
            Body::WithCpu(f) => f(c, b),
            Body::CpuIn(f) => f(c, b, self.entry),
            Body::Memory(f) => {
                f(b);
                mem::rts(c, b);
            }
            Body::Handler(f) => {
                // called from the processor (dispatch loop): the leftovers go from the bench to the table, the
                // handler reads and writes them by name, then they return to the bench
                dispatch::Leftovers::from_bench(c, b);
                let leftovers = tasks::table::table(b).leftovers;
                let mut a = dispatch::Dispatch { object: objects::Obj(c.a[0]), counter: c.d[7] as u16, leftovers };
                f(b, &mut a);
                tasks::table::table(b).leftovers = a.leftovers;
                dispatch::Leftovers::to_bench(c, b);
                mem::rts(c, b);
            }
        }
    }

    /// The body does not use the processor.
    pub fn without_cpu(&self) -> bool {
        matches!(self.run, Body::Memory(_) | Body::Handler(_))
    }
}

/// Duration of a native in hybrid mode (68000 cycles), the same for all: this mode is now only for diagnostics.
pub const NATIVE_DURATION: u64 = 100;

/// Register masks for `Native::outputs`.
pub mod regs {
    pub const fn d(n: u32) -> u32 {
        1 << n
    }
    pub const fn a(n: u32) -> u32 {
        1 << (8 + n)
    }
    pub const CCR: u32 = 1 << 16;
    /// All the registers and the flags (exact replacement of a leaf routine).
    pub const ALL: u32 = 0x1_ffff;
}

/// All the native routines.
pub const NATIVES: &[Native] = &[
    combat::COMBAT_UPDATE,
    inputs::READ_INPUTS,
    inputs::COPY_INPUTS_TO_PLAYERS,
    inputs::GET_PLAYER_INPUT,
    inputs::CLEAR_PLAYER_INPUTS,
    sprites::BUILD_SPRITE_LIST,
    sprites::RESET_LISTS,
    text::PRINT,
    text::ERASE,
    text::PRINT_OVER,
    text::PRINT_TALL,
    text::PRINT_TALL_AT,
    text::CLEAR_BLOCKS,
    text::DRAW_BLOCKS,
    text::CLEAR_BOX,
    palette::FULL_BRIGHTNESS,
    palette::NO_BRIGHTNESS,
    palette::OR_BRIGHTNESS,
    palette::SET_BRIGHTNESS,
    sound::ENQUEUE_IF_SOUND,
    sound::ENQUEUE_DIRECT,
    sound::SEND_TO_Z80,
    sound::SERVICE_FF00,
    sound::SERVICE_FF05,
    sound::SERVICE_FF06_D1,
    sound::SERVICE_FF06,
    sound::SERVICE_FF07_D1,
    sound::SERVICE_FF07,
    sound::SERVICE_FF08,
    sound::SERVICE_FF09,
    sound::INIT_PRIVATE_STACK,
    sound::NONE,
    rng::SEED,
    rng::NEXT,
    events::CLEAR_POINTS,
    events::CLEAR_COUNTER,
    events::CLEAR_POPUPS,
    events::CLEAR_POPUP,
    events::ENQUEUE_POINTS,
    events::ENQUEUE_COUNTER,
    events::OBJECT_EVENT,
    events::CREATE_POPUP,
    events::CREDIT_POINTS_NATIVE,
    events::CREDIT_COUNTER_NATIVE,
    credits::READ_COIN_SLOTS,
    eeprom::READ,
    eeprom::WRITE,
    eeprom::WRITE_VERIFY,
    eeprom::READ_VERIFY,
    eeprom::COUNT_COINS,
    eeprom::COUNT_STARTS,
    eeprom::COUNT_CONTINUES,
    eeprom::COUNT_GAMES,
    eeprom::VBLANK_ACCOUNTING,
    eeprom::COUNT_GAMES_ACC,
    eeprom::ACCOUNTING_INIT,
    eeprom::STAT_CHARACTER,
    eeprom::STAT_DURATION,
    config::VARIANT,
    config::DEFAULTS,
    config::VALIDATE,
    config::VARIABLES,
    config::INIT,
    popups::ANIMATE,
    eeprom::FULL_COINS_NATIVE,
    eeprom::FULL_STARTS_NATIVE,
];

/// Index of the native routines by entry address: `NATIVES`, the object model and the sound wrappers
/// (described by a table). A binary sieve avoids the search for the vast majority of blocks.
struct Index {
    sieve: Vec<u64>,
    by_address: HashMap<u32, &'static Native>,
}

fn lookup(pc: u32) -> Option<&'static Native> {
    static INDEX: OnceLock<Index> = OnceLock::new();
    let index = INDEX.get_or_init(|| {
        // DDSOM_SANS=prefix,...: native routines disabled (search for a faulty routine)
        let without: Vec<String> = std::env::var("DDSOM_SANS")
            .map(|v| v.split(',').map(str::to_owned).collect())
            .unwrap_or_default();
        let by_address: HashMap<_, _> = all_natives()
            .filter(|n| !without.iter().any(|p| n.name.starts_with(p.as_str())))
            .map(|n| (n.entry, n))
            .collect();
        let mut sieve = vec![0u64; 0x100000 / 2 / 64];
        for &a in by_address.keys() {
            sieve[(a >> 7) as usize] |= 1 << (a >> 1 & 63);
        }
        Index { sieve, by_address }
    });
    let case = index.sieve.get((pc >> 7) as usize)?;
    if case >> (pc >> 1 & 63) & 1 == 0 {
        return None;
    }
    index.by_address.get(&pc).copied()
}

/// A native routine replaces the original code at address `pc`.
pub fn is_native(pc: u32) -> bool {
    lookup(pc).is_some()
}

/// All the native routines, sound wrappers included (coverage tools).
pub fn all_natives() -> impl Iterator<Item = &'static Native> {
    // one source per line (parallel branches each add their own)
    std::iter::empty()
        .chain(NATIVES)
        .chain(objects::NATIVES)
        .chain(player::NATIVES)
        .chain(player::init::JOIN)
        .chain(player::init::NATIVES)
        .chain(video::NATIVES)
        .chain(palette::NATIVES)
        .chain(scroll::NATIVES)
        .chain(camera_modes::NATIVES)
        .chain(terrain::NATIVES)
        .chain(placement::NATIVES)
        .chain(effects_aids::NATIVES)
        .chain(families::NATIVES)
        .chain(families::natives())
        .chain(dispatch::NATIVES)
        .chain(game::NATIVES)
        .chain(scene::natives())
        .chain(modes::natives())
        .chain(sound::native_wrappers())
        .chain(kernel::NATIVES)
        .chain(tasks::natives())
        .chain(aids::NATIVES)
        .chain(hud::NATIVES)
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
        let n = no68k::native(c, b, addr);
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
            panic!("call of {addr:06x} ({site}): the routine ceded control or returned higher than its caller (PC {:06x})", c.pc);
        }
        one_step(c, b);
    }
    diag::probe_call(c, addr, &before, site);
}

/// Terminal call (`jmp`) of native `n` from a Rust function: it runs immediately (with the diagnostics of
/// `DDSOM_CHECK`...), and its `rts` returns to the continuation of the task's current call. Replaces setting the PC
/// (jump from one native to another).
pub fn jump(c: &mut Cpu, b: &mut Cps2Bus, n: &'static Native) {
    execute(n, c, b, 0);
}

/// Like [`jump`], to the native set at `address` (handler designated by a ROM table).
#[track_caller]
pub fn jump_to(c: &mut Cpu, b: &mut Cps2Bus, address: u32) {
    let n = lookup(address).unwrap_or_else(|| panic!("jump to {address:06x} ({}): no native", std::panic::Location::caller()));
    execute(n, c, b, 0);
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

/// Calls the native `address` (handler designated by a ROM table) for a Rust function that continues
/// after it: the expected call `frame` was opened by `tasks::table::call_expected`, with the task's continuation if
/// the native cedes. The natives chain (jumps) until the return. Returns true if the native returned; false if the
/// task ceded control, or if the callee popped the call to return higher (foreign epilogue of the bubbles): the
/// Rust function then gives up, and the task will continue through the continuations of the pending calls.
pub fn call_expected(c: &mut Cpu, b: &mut Cps2Bus, address: u32, frame: tasks::table::Call) -> bool {
    if no68k::active() {
        let n = no68k::native(c, b, address);
        execute(n, c, b, 0);
    } else if let Some(n) = lookup(address) {
        execute(n, c, b, 0);
    } else {
        // tools: original code, until the return to a pushed sentinel address
        const RETURN: u32 = 0x00_fffe;
        let sp = c.a[7];
        c.a[7] = sp.wrapping_sub(4);
        mem::wr32(b, c.a[7], RETURN);
        c.pc = address;
        while !(c.pc == RETURN && c.a[7] == sp) && tasks::table::pending(b, frame) && !tasks::has_ceded() {
            one_step(c, b);
        }
        if c.pc == RETURN && c.a[7] == sp {
            tasks::table::give_back_expected(b, frame);
        }
    }
    tasks::table::returned(b, frame)
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

/// Start of a game step in the main loop (0x1c4b0: `st (-0x660e,A5)` just before the call of the phase).
pub const GAME_STEP: u32 = 0x1c4b0;

/// Maximum game steps per frame.
///
/// The main loop (0x1c48c) usually cedes one frame per step (0xa40, D0 = 1). But when a button is
/// pressed at a scene entry or in the attract mode, an auxiliary task (entry 13 of the table, 0xee080) makes the main
/// task ready again at each pass of the scheduler: the game chains its steps without waiting for the frame
/// ("fast forward"). The number of steps per frame is then (frame duration) / (step duration): 5 to 10 on
/// the arcade (measured in the attract mode: 7-8, `examples/pc_hits`; in game: 5-7; scene entry: 9-10) but 128 at
/// ×16, and it depends on the duration of the routines, hence differs between the original code and the natives.
///
/// Deliberate difference from the overclocked original code (not from the arcade): at most `MAX_STEPS_PER_FRAME` steps
/// per frame, whatever the 68000 frequency (`cps2::Machine::step_cap`). Applied identically by the
/// game (`install`) and by the reference machine (`install_reference`), it makes these phases deterministic.
pub const MAX_STEPS_PER_FRAME: u32 = 8;

/// Caps the game steps per frame (`DDSOM_STEP_CAP=n`: other value, 0 = no cap, diagnostics).
fn cap_steps(m: &mut cps2::Machine) {
    let max = std::env::var("DDSOM_STEP_CAP").ok().and_then(|v| v.parse().ok()).unwrap_or(MAX_STEPS_PER_FRAME);
    m.step_cap = (max > 0).then(|| cps2::StepCap::new(GAME_STEP, max));
    m.patches = patches();
}

/// Patches common to the game and the reference machine (`cps2::Machine::patches`).
fn patches() -> Vec<cps2::Patch> {
    let mut v = vec![
        cps2::Patch { entry: player::scripted::END_BY_SCENE_ENTRY, apply: player::scripted::patch_end_by_scene },
        cps2::Patch { entry: scene::exit::SHARED_NEXT_ENTRY, apply: scene::exit::patch_shared_next },
        cps2::Patch { entry: service::memory::CHECK, apply: service::memory::patch_instant_check },
    ];
    v.extend(sprites::EMPTY_FRAME_PATCHES);
    v
}

/// Enables the native routines (and the recompiled code) on the machine.
///
/// The game is the game without a 68000 (`install_no_68k`). The hybrid mode (natives and recompiled original code for
/// the rest, `DDSOM_HYBRIDE=1`) is removed: the tasks are Rust state machines launched by a Rust scheduler, and the
/// pending calls are no longer on the 68000 stack; the original code could no longer take over from a native in
/// the middle of a task. Only the reference machine (`install_reference`) still runs the 68000.
pub fn install(m: &mut cps2::Machine) {
    install_no_68k(m);
}

/// `DDSOM_HYBRIDE=1` asked for the hybrid mode, removed (see [`install`]): stop with a message.
pub fn refuse_hybrid() {
    if std::env::var("DDSOM_HYBRIDE").is_ok_and(|v| v == "1") {
        eprintln!("DDSOM_HYBRIDE=1: the hybrid mode is removed (docs/refactorisation.md, batch 'final kernel'); the game is the game without a 68000.");
        std::process::exit(2);
    }
}

/// Game without a 68000: natives only, kernel frame loop (`no68k`). The machine's `Cpu` is now only a register
/// bench; an address without a native stops the game with a diagnostic.
pub fn install_no_68k(m: &mut cps2::Machine) {
    cap_steps(m);
    no68k::install(m);
}

/// Validation reference machine: original code only, with the same step cap as `install`.
pub fn install_reference(m: &mut cps2::Machine) {
    ddsom_rc::install(m);
    m.idle = Some(idle);
    cap_steps(m);
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
        // the original state is kept so as not to propagate the difference
        *c = c2;
        *b = b2;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn unique_inputs() {
        let mut views = std::collections::HashSet::new();
        for n in super::all_natives() {
            assert!(views.insert(n.entry), "{:06x} ({}) replaced twice", n.entry, n.name);
        }
    }
}
