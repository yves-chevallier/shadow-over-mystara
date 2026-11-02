//! The game without a 68000: natives only and the kernel's frame loop (see `docs/noyau.md`, "towards a loop
//! without 68000").
//!
//! The game does not use the machine's `Cpu` (only its cycle counter, for the machine's schedule): the code still on
//! the processor runs on a fresh register bench (`bench`). The processor is at a
//! *point* (`kernel::Point`, stored in the task table): a startup step, a scheduler point
//! (Rust state machine, `kernel`), a task, or the held game step. A task is launched by calling the step of its
//! stage (`tasks::execute`). A native always runs in full, even when
//! it would overflow onto the next interrupt: none is replaced by its original. An address without a native
//! stops the game dead with a diagnostic ([`STOP`]) instead of running original code.
//!
//! ## The frame
//!
//! The machine keeps its schedule (`cps2::Machine::line_work`): at each line, [`line`] does the work of the
//! processor, then the Z80 plays its line. The processor's work has no duration:
//!
//! 1. the VBLANK interrupt, raised at the end of the previous frame, is taken as soon as the SR allows it, between two
//!    points (at the start of the frame: the scheduler waits for it, the held game step too; during startup, when
//!    `boot_init` unmasks): it is a kernel point (`Point::Interruption`, the interrupted point stored in the table,
//!    without pushed PC or SR), which runs in full (`vblank::interruption`) then returns to the interrupted point;
//! 2. then the points chain (scheduler, tasks, cessions) until the scheduler idles
//!    ([`at_rest`]: no task ready any more, or only the freeze task with nothing to do), or the game step
//!    reaches the frame cap (`MAX_STEPS_PER_FRAME` steps, [`game_step`]: the held step runs at the start of
//!    the next frame, right after VBLANK);
//! 3. at startup, a point that waits for a beam position (Z80 release, `boot_init`:
//!    `startup.rs`) hands control back without changing anything: the line stops there and it retries at the next.
//!
//! All the work of the frame is thus done at the start of its first line (240), before the Z80, as at ×16 where it ends
//! in a few lines: only VBLANK and the sending of commands to the Z80 (`0x68c`, which it calls) touch the shared
//! RAM, and they come first. The overclocking, `idle`, the durations of the natives and the machine
//! patches (`patches`, which apply to the original code) are no longer used.
//!
//! A ready task waiting for an event brought by VBLANK (end of a fade...) makes the scheduler spin:
//! the original spins like this until the interrupt. Here, when the state comes back identical from one walk of the
//! scheduler to the next ([`Snapshot`]), the frame is finished. Safety cap: at most [`NATIVES_CAP`]
//! points per frame (counted in [`summary`]).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use cps2::{Cps2Bus, StepCap};
use m68k::Cpu;

use crate::kernel::Point;
use crate::tasks::table::{read, table};
use crate::Native;

/// The game runs without a 68000 (a single machine in the process, the game's: the `validate` reference machine
/// does not run natives).
static ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// Hooks the loop without a 68000 onto the machine (the machine's step cap is only used for its value).
pub(crate) fn install(m: &mut cps2::Machine) {
    ACTIVE.store(true, Ordering::Relaxed);
    m.executor = None;
    m.idle = None;
    m.patches.clear();
    m.line_work = Some(line);
}

/// Points at most per frame: an ordinary frame runs a few thousand.
pub const NATIVES_CAP: u32 = 1_000_000;

/// Scheduler walk in a frame from which an active wait is looked for (see [`Snapshot`]).
const ORDINARY_WALKS: u32 = 3;

/// Frames finished on a recognised active wait, and frames stopped by [`NATIVES_CAP`].
static IDLE_FRAMES: AtomicU64 = AtomicU64::new(0);
static CAPPED_FRAMES: AtomicU64 = AtomicU64::new(0);

/// Summary for `validate`: frames finished on an active wait, frames stopped by the native cap.
pub fn summary() -> (u64, u64) {
    (IDLE_FRAMES.load(Ordering::Relaxed), CAPPED_FRAMES.load(Ordering::Relaxed))
}

/// State of the machine at the start of a scheduler walk (`0x932`): memory that the natives write, task
/// table. If it comes back identical from one walk to the next, the tasks are spinning (a ready task
/// waits for an event that only VBLANK will bring, like the end of a fade): nothing will change before
/// the interrupt, the frame is finished. The original, for its part, spins until the interrupt.
#[derive(PartialEq)]
struct Snapshot {
    ram: Vec<u16>,
    extra_ram: Vec<u16>,
    gfxram: Vec<u16>,
    objram: [Vec<u16>; 2],
    palette: Vec<u16>,
    qram: Vec<u8>,
    regs: ([u16; 6], [u16; 32], [u16; 32], u16, u16),
    table: Option<crate::tasks::table::Table>,
}

impl Snapshot {
    fn take(b: &Cps2Bus) -> Self {
        let mut ram = b.ram.clone();
        // the RAM of the original stacks does not count (changed at each point by the `stack` probe)
        ram[STACKS.0 / 2..STACKS.1 / 2].fill(0);
        Snapshot {
            ram,
            extra_ram: b.extra_ram.clone(),
            gfxram: b.gfxram.clone(),
            objram: b.objram.clone(),
            palette: b.palette.clone(),
            qram: b.qram.clone(),
            regs: (b.output, b.cps_a, b.cps_b, b.objram_bank, b.eeprom_out),
            table: read(b).cloned(),
        }
    }
}

/// Where the current frame is.
#[derive(Default)]
struct Image {
    number: u64,
    natives: u32,
    walk: u32,
    /// Game steps run in the frame, and their cap (`cps2::StepCap::max`; none: no cap).
    step: u32,
    cap: Option<u32>,
    /// Nothing left to do before the interrupt (active wait recognised, or cap).
    finished: bool,
    snapshot: Option<Snapshot>,
}

thread_local! {
    static IMAGE: RefCell<Image> = RefCell::new(Image { number: u64::MAX, ..Image::default() });
    /// Last natives run (diagnosing a stop).
    static LAST: RefCell<VecDeque<&'static Native>> = const { RefCell::new(VecDeque::new()) };
}

/// Processor work during a line (`cps2::Machine::line_work`). The cap on game steps is only read for
/// its value: the count of the frame's steps is kept here ([`game_step`]).
pub fn line(c: &mut Cpu, b: &mut Cps2Bus, end: u64, cap: Option<&mut StepCap>) {
    if b.irq_raster {
        stop(b, "raster interrupt (level 4): never armed by the game".into());
    }
    IMAGE.with(|i| {
        let mut i = i.borrow_mut();
        if i.number != b.frame {
            *i = Image { number: b.frame, ..Image::default() };
        }
        i.cap = cap.map(|p| p.max);
    });
    let poison_stack = crate::diag::poison_active("stack");
    loop {
        // a top-level point: no cession in progress
        crate::tasks::forget_cession();
        let point = crate::kernel::point(b);
        if poison_stack && !point.startup() {
            poison_stacks(b);
        }
        let level = b.irq_level();
        if point != Point::Interruption && level > crate::kernel::interrupt_mask(b) {
            crate::kernel::interrupt(b, point);
            continue;
        }
        // (a masked interrupt waits for the scheduler to reopen its window)
        if level == 0 && (at_rest(b, point) || frame_done(b, point)) {
            break;
        }
        let next = match point {
            Point::Reset | Point::ReleaseZ80 | Point::Handshake | Point::BootInit | Point::Launch => {
                // `None`: it is waiting for its beam position
                let Some(next) = crate::startup::execute(b, point) else { break };
                next
            }
            Point::Start | Point::Walk | Point::Exam | Point::NextTask | Point::Return => {
                crate::kernel::execute(b, point)
            }
            Point::Interruption => {
                crate::vblank::interruption(b, level);
                crate::kernel::interrupt_end(b)
            }
            // a task no longer continues by a jump to a native: each step cedes, ends or blocks
            Point::Task => stop(b, "the task handed control back without ceding".into()),
            // the original's endless loop: the frame is finished
            Point::Blocked(_) => break,
            Point::StepKept { d4 } => {
                if cap_reached() {
                    break;
                }
                table(b).point = Point::Task;
                crate::tasks::execute(crate::tasks::game::Step::Frame { d4 }.into(), b);
                crate::kernel::point(b)
            }
        };
        if next == Point::Task {
            stop(b, "the task handed control back without ceding".into());
        }
        table(b).point = next;
    }
    c.cycles = c.cycles.max(end);
}

/// RAM of the original stacks (offsets from FF0000): stack heap, stacks of the tasks and of the supervisor
/// (FF0166-FF0E65, which `validate` ignores: `tasks::table::kernel_ram`). Beyond, FF0E66 and the rest are
/// game variables (relative to A5: `placement::LAST_X` at A5 - 0x719a...).
const STACKS: (usize, usize) = (0x166, 0xe66);

/// Probe `DDSOM_POISON=stack`: before each scheduler or task point, the RAM of the original stacks
/// ([`STACKS`]) is filled with a value that changes each time. If validation stays VALID, the game reads nothing there:
/// it no longer has a 68000 stack.
fn poison_stacks(b: &mut Cps2Bus) {
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let pattern = (n as u16).wrapping_mul(0x9e37) ^ 0xa5c3;
    for (i, word) in b.ram[STACKS.0 / 2..STACKS.1 / 2].iter_mut().enumerate() {
        *word = pattern.wrapping_add(i as u16);
    }
}

/// The scheduler idles, between two tasks: no task ready and no new frame, or only the freeze task
/// is ready and its next step would do nothing (`tasks::freeze::idle_wait`).
fn at_rest(b: &Cps2Bus, point: Point) -> bool {
    const NEW_FRAME: usize = 0x164 / 2; // FF0164 (high byte): set by VBLANK, cleared at the start of a walk
    if !point.between_two_tasks() || b.ram[NEW_FRAME] >> 8 != 0 {
        return false;
    }
    let Some(t) = read(b) else { return false };
    t.slots.iter().all(|e| !e.state.castable()) || crate::tasks::freeze::idle_wait(t, &b.ram)
}

/// A game step begins (`tasks::game`): true if it runs in this frame (it is counted), false if it is
/// held until the next (cap on steps per frame, `crate::MAX_STEPS_PER_FRAME`: deliberate difference from the
/// overclocked original game, applied identically by the reference machine).
pub fn game_step() -> bool {
    IMAGE.with(|i| {
        let mut i = i.borrow_mut();
        if i.cap.is_some_and(|max| i.step >= max) {
            return false;
        }
        i.step += 1;
        true
    })
}

fn cap_reached() -> bool {
    IMAGE.with(|i| {
        let i = i.borrow();
        i.cap.is_some_and(|max| i.step >= max)
    })
}

/// The frame has nothing left to do before the interrupt: active wait recognised at the start of a walk of the
/// scheduler (same [`Snapshot`] as at the previous walk), or point cap reached.
fn frame_done(b: &Cps2Bus, point: Point) -> bool {
    IMAGE.with(|i| {
        let mut i = i.borrow_mut();
        if i.finished {
            return true;
        }
        i.natives += 1;
        if i.natives >= NATIVES_CAP {
            CAPPED_FRAMES.fetch_add(1, Ordering::Relaxed);
            i.finished = true;
        } else if point == Point::Walk {
            i.walk += 1;
            if i.walk >= ORDINARY_WALKS {
                let now = Snapshot::take(b);
                if i.snapshot.as_ref() == Some(&now) {
                    IDLE_FRAMES.fetch_add(1, Ordering::Relaxed);
                    i.finished = true;
                } else {
                    i.snapshot = Some(now);
                }
            }
        }
        i.finished
    })
}

/// The native at address `address` (call by address, handler of a ROM table), or a game stop if there is
/// none. It is noted for diagnosing a stop (`DDSOM_SANS_TRACE=frame`: each native of the frame).
pub(crate) fn native(b: &mut Cps2Bus, address: u32) -> &'static Native {
    let Some(n) = crate::lookup(address) else {
        stop(b, format!("no native at {address:06x}"));
    };
    LAST.with(|d| {
        let mut d = d.borrow_mut();
        if d.len() == 8 {
            d.pop_front();
        }
        d.push_back(n);
    });
    if trace(b) {
        eprintln!("{}:{} {:06x} {} mask={}", b.frame, b.scanline, address, n.name, crate::kernel::interrupt_mask(b));
    }
    n
}

/// Diagnostics: `DDSOM_SANS_TRACE=n` prints each native run during frame n.
fn trace(b: &Cps2Bus) -> bool {
    static IMAGE: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    *IMAGE.get_or_init(|| std::env::var("DDSOM_SANS_TRACE").ok().and_then(|v| v.parse().ok())) == Some(b.frame)
}

/// Start of the message of a stop (panic) of the game without a 68000.
pub const STOP: &str = "without 68000: ";

/// Stops dead (panic, message starting with [`STOP`]): cause, frame and line, current task and its pending calls
/// (frames), interrupt mask, last natives run, Rust call of original code in progress.
fn stop(b: &mut Cps2Bus, cause: String) -> ! {
    let mut m = format!("{cause} (frame {}, line {})", b.frame, b.scanline);
    if let Some(t) = read(b) {
        m += &format!(" ; point {:?}", t.point);
        let k = t.current;
        let e = &t.slots[k];
        let frames: Vec<String> = e.frames.iter().map(|k| format!("{:x?}@{}", k.next, k.calls)).collect();
        m += &format!(" ; task {k} ({:?}, {:?}) ; frames [{}]", e.task, e.state, frames.join(" "));
    }
    m += &format!(" ; interrupt mask {}", crate::kernel::interrupt_mask(b));
    let last = LAST.with(|d| d.borrow().iter().map(|n| format!("{}@{:x}", n.name, n.entry)).collect::<Vec<_>>());
    m += &format!(" ; last natives: {}", last.join(" "));
    if let Some((site, routine)) = crate::call_site() {
        m += &format!(" ; Rust call in progress: {site} -> {routine:x}");
    }
    panic!("{STOP}{m}");
}
