//! Game logic of ddsom rewritten in idiomatic Rust.
//!
//! The game is the game without a 68000 (`install_no_68k`, module `no68k`): Rust functions run by the kernel's
//! frame loop. It has no processor: no `m68k::Cpu`, no register.
//!
//! A *native routine* is the Rust version of a 68000 routine, designated by the routine's entry address. The game
//! still finds by address the object handlers of the ROM tables (`dispatch`): their natives are register free
//! ([`Body::Memory`], [`Body::Handler`]).
//!
//! The tools (feature `tools`, module [`tools`]: `validate`, `fuzz_*`, coverage, `DDSOM_CHECK`) add the reference
//! machine (original code on the 68000) and the natives at the original addresses whose arguments and results are in
//! the registers of the original, to compare them with the original code.

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

use std::collections::HashMap;
use std::sync::OnceLock;

use cps2::Cps2Bus;

#[cfg(feature = "tools")]
pub mod tools;
#[cfg(feature = "tools")]
pub use tools::{
    call, call_site, idle, install_reference, report, run, run_native_only, run_traced, set_check,
    set_check_all, set_count, set_native_hook, set_trace, Stats,
};

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
    /// Tools: on the processor, for the original code that calls it (arguments and results in the registers of the
    /// original), ending with its `rts`.
    #[cfg(feature = "tools")]
    WithCpu(fn(&mut m68k::Cpu, &mut Cps2Bus)),
    /// Tools: like `WithCpu`, for a body shared by several entries: it receives the address of its own.
    #[cfg(feature = "tools")]
    CpuIn(fn(&mut m68k::Cpu, &mut Cps2Bus, u32)),
    /// Without registers (the target form: memory only); the return (`rts`) is done for it.
    Memory(fn(&mut Cps2Bus)),
    /// Object handler without registers: it receives what the dispatch loop passes it (the object, the counter,
    /// the leftovers that the handlers pass to each other: `dispatch::Dispatch`); the return is done for it.
    Handler(fn(&mut Cps2Bus, &mut dispatch::Dispatch)),
}

impl Native {
    /// The body does not use the processor.
    pub fn without_cpu(&self) -> bool {
        matches!(self.run, Body::Memory(_) | Body::Handler(_))
    }
}

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
    
    
    
    
    
    
    sprites::RESET_LISTS,
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    sound::SEND_TO_Z80,
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    credits::READ_COIN_SLOTS,
    
    
    
    
    
    
    
    
    
    
    
    
    
    config::DEFAULTS,
    config::VALIDATE,
    config::VARIABLES,
    config::INIT,
    
    
    
];

/// Index of the native routines by entry address: `NATIVES`, the object model and the sound wrappers
/// (described by a table). A binary sieve avoids the search for the vast majority of blocks.
struct Index {
    sieve: Vec<u64>,
    by_address: HashMap<u32, &'static Native>,
}

pub(crate) fn lookup(pc: u32) -> Option<&'static Native> {
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

/// All the native routines: those of the game and, with the tools, those for the original code (coverage tools).
pub fn all_natives() -> impl Iterator<Item = &'static Native> {
    let natives = game_natives();
    #[cfg(feature = "tools")]
    let natives = natives.chain(tools::natives());
    natives
}

/// The natives of the game: register free (`Body::Memory`, `Body::Handler`), designated by the original address of the
/// routine (object handlers of the ROM tables, `dispatch`).
pub fn game_natives() -> impl Iterator<Item = &'static Native> {
    // one source per line (parallel branches each add their own)
    std::iter::empty()
        .chain(NATIVES)
        .chain(objects::NATIVES)
        .chain(player::NATIVES)
        .chain(player::init::JOIN)
        .chain(video::NATIVES)
        .chain(palette::NATIVES)
        .chain(scroll::NATIVES)
        .chain(camera_modes::NATIVES)
        .chain(terrain::NATIVES)
        .chain(families::NATIVES)
        .chain(families::natives())
        .chain(game::NATIVES)
        .chain(scene::natives())
        .chain(modes::natives())
        .chain(tasks::natives())
        .chain(aids::NATIVES)
        .chain(hud::NATIVES)
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
pub(crate) fn cap_steps(m: &mut cps2::Machine) {
    let max = std::env::var("DDSOM_STEP_CAP").ok().and_then(|v| v.parse().ok()).unwrap_or(MAX_STEPS_PER_FRAME);
    m.step_cap = (max > 0).then(|| cps2::StepCap::new(GAME_STEP, max));
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

/// Game without a 68000: Rust functions run by the kernel frame loop (`no68k`). The machine's `Cpu` is not used; an
/// address without a native stops the game with a diagnostic.
pub fn install_no_68k(m: &mut cps2::Machine) {
    cap_steps(m);
    no68k::install(m);
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
