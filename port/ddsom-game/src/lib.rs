//! Game logic of ddsom rewritten in idiomatic Rust.
//!
//! The game is the game without a 68000 (`install_no_68k`, module `core::frame`): Rust functions run by the kernel's
//! frame loop. It has no processor: no `m68k::Cpu`, no register.
//!
//! The game state is its [`world::World`]: the work RAM (indexed without address decoding), the task table and the
//! hardware. The modules are in layers (docs/fin.md, section 2), from the bottom: [`hw`] (memory accesses, video,
//! input and output ports), [`world`] (state, objects, data), [`services`] (palette, text, sprites, scrolling,
//! sound, inputs, EEPROM...), [`actors`] (object families, players, their aids), [`screens`] (modes, scenes,
//! interface), [`core`] (kernel, tasks, dispatch, frame loop).
//!
//! A *native routine* is the Rust version of a 68000 routine; its entry address and name are metadata for the tools.
//! The game calls its routines directly and finds the object handlers in the dispatch tables (`dispatch`, Rust arrays
//! indexed by the type); it has no registry by address: [`Native`] values are only listed and found by address by the
//! tools (`tools::registry`).
//!
//! The tools (feature `tools`, module [`tools`]: `validate`, `fuzz_*`, coverage, `DDSOM_CHECK`) add the reference
//! machine (original code on the 68000) and the natives at the original addresses whose arguments and results are in
//! the registers of the original, to compare them with the original code.

// The layers, from the bottom (docs/fin.md, section 2): each one calls only those below it.
pub mod world;
pub mod hw;
pub mod services;
pub mod actors;
pub mod screens;
pub mod core;

use crate::world::World;

/// The game machine: the CPS2 machine whose board is the game's [`World`] (work RAM and task table out of the bus).
pub type GameMachine = cps2::Machine<World>;

#[cfg(feature = "tools")]
pub mod tools;
#[cfg(feature = "tools")]
pub use tools::{
    all_natives, call, call_site, game_natives, idle, install_reference, is_native, report, run, run_native_only,
    run_traced, set_check, set_check_all, set_count, set_native_hook, set_trace, Stats,
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
    WithCpu(fn(&mut m68k::Cpu, &mut World)),
    /// Tools: like `WithCpu`, for a body shared by several entries: it receives the address of its own.
    #[cfg(feature = "tools")]
    CpuIn(fn(&mut m68k::Cpu, &mut World, u32)),
    /// Without registers (the target form: memory only); the return (`rts`) is done for it.
    Memory(fn(&mut World)),
    /// Object handler without registers: it receives what the dispatch loop passes it (the object, the counter,
    /// the leftovers that the handlers pass to each other: `dispatch::Dispatch`); the return is done for it.
    Handler(fn(&mut World, &mut crate::core::dispatch::Dispatch)),
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
pub(crate) fn cap_steps<B: cps2::Board>(m: &mut cps2::Machine<B>) {
    let max = std::env::var("DDSOM_STEP_CAP").ok().and_then(|v| v.parse().ok()).unwrap_or(MAX_STEPS_PER_FRAME);
    m.step_cap = (max > 0).then(|| cps2::StepCap::new(GAME_STEP, max));
}

/// Enables the native routines (and the recompiled code) on the machine.
///
/// The game is the game without a 68000 (`install_no_68k`). The hybrid mode (natives and recompiled original code for
/// the rest, `DDSOM_HYBRIDE=1`) is removed: the tasks are Rust state machines launched by a Rust scheduler, and the
/// pending calls are no longer on the 68000 stack; the original code could no longer take over from a native in
/// the middle of a task. Only the reference machine (`install_reference`) still runs the 68000.
pub fn install(m: cps2::Machine) -> GameMachine {
    install_no_68k(m)
}

/// `DDSOM_HYBRIDE=1` asked for the hybrid mode, removed (see [`install`]): stop with a message.
pub fn refuse_hybrid() {
    if std::env::var("DDSOM_HYBRIDE").is_ok_and(|v| v == "1") {
        eprintln!("DDSOM_HYBRIDE=1: the hybrid mode is removed (docs/refactorisation.md, batch 'final kernel'); the game is the game without a 68000.");
        std::process::exit(2);
    }
}

/// Game without a 68000: Rust functions run by the kernel frame loop (`frame`). The machine's `Cpu` is not used. The
/// machine's RAM moves into the game's [`World`].
pub fn install_no_68k(m: cps2::Machine) -> GameMachine {
    install_data(&m);
    let mut m = m.with_board(World::from_bus);
    cap_steps(&mut m);
    crate::core::frame::install(&mut m);
    m
}

/// The game data and the residue must be installed before the game runs (`extract::install_game`,
/// `extract::install_game_assets`): the application loads them from the assets or extracts them from the ROMs. The
/// tools extract them from the machine's program when none are installed (machines loaded from the ROMs).
pub fn install_data(m: &cps2::Machine) {
    #[cfg(feature = "tools")]
    if let Some(rom) = m.bus.rom.as_ref().filter(|_| !ddsom_data::installed() || !ddsom_data::residue::installed()) {
        let program = extract::Program::from_words(rom.data.clone(), rom.decrypted.clone());
        ddsom_data::install_with(|| extract::to_data_from(&program).expect("game data: extraction from the machine's program"));
        ddsom_data::residue::install(ddsom_data::residue::Source::Ready(program.residue()));
    }
    #[cfg(not(feature = "tools"))]
    let _ = m;
    let _ = world::data::data();
    assert!(ddsom_data::residue::installed(), "residue not installed: install the game with `extract::install_game`");
}
