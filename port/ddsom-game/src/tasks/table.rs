//! Task table in Rust: replaces the FF0064 records of the original kernel (see `docs/noyau.md`).
//!
//! 16 slots, numbered by their offset in the original table divided by 16 (`kill(0x30)`: slot 3).
//! The scheduler examines them from 0 to 15 (in the original, D0 then counts down from 15 to 0). Each slot carries
//! the original's state and wait count, the 68000 stack given to the task and what is needed to resume it:
//! its entry as long as it has not run (the [`Task`], which starts at its first stage), then the Rust stage where it
//! will resume (what it re-reads on resuming is in the fields of the stage, no longer in saved registers), and its
//! calls in progress ([`Frame`]). The table also keeps where the processor is (`kernel::Point`).
//!
//! The table lives in the bus (`Cps2Bus::ext`): the game machine no longer writes FF0052-FF01A5 (scheduler
//! stack, current task, stack heap, records), and the cession wrappers no longer save the
//! registers on the task's stack. [`gaps`] compares the table with that of the RAM of a reference machine.
//!
//! A task always resumes at a stage ([`Step`], `Resume::Step`) and its calls in progress continue at stages
//! ([`Next::Step`]): the scheduler calls its step without setting a PC. The original address of the stage
//! (`Step::address`) is now only used for diagnostics, and for the registers the original leaves there (foreign
//! epilogue of the
//! bubbles). A continuation designated by an address ([`Next::Address`]) is now just a return of the 68000 stack: that
//! of a call by address (`crate::call`, sentinel) or of a diagnostic tool that calls a native.

use cps2::Cps2Bus;

use super::{Step, Task};

/// Number of slots.
pub const SLOTS: usize = 16;
/// Address of the original record of slot 0; slot k is at `+ 0x10 k`.
pub const RECORDS: u32 = 0xff_0064;

/// Address (fictitious in the game machine) of the record of slot `k`, which the original leaves in A0.
pub fn record(k: usize) -> u32 {
    RECORDS + 0x10 * k as u32
}

/// Slot designated by an offset in the table (low byte of D0 of the traps).
pub fn slot(offset: u32) -> usize {
    (offset as usize & 0xff) / 0x10
}

/// State of a slot (byte +0 of the original record).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Free,
    /// Waits until VBLANK has counted its count down to zero.
    Waiting,
    /// Waits for another task to wake it (trap 6).
    Suspended,
    /// Will be launched at the scheduler's next pass.
    Ready,
    /// Launched, has not ceded control yet.
    InProgress,
    /// Created (trap 0) or relaunched (trap 7): will start at its entry.
    Fresh,
}

impl State {
    /// State byte of the original.
    pub fn byte(self) -> u8 {
        match self {
            State::Free => 0,
            State::Waiting => 1,
            State::Suspended => 2,
            State::Ready => 4,
            State::InProgress => 8,
            State::Fresh => 0xc,
        }
    }

    pub fn from_byte(byte: u8) -> Option<State> {
        [State::Free, State::Waiting, State::Suspended, State::Ready, State::InProgress, State::Fresh]
            .into_iter()
            .find(|e| e.byte() == byte)
    }

    /// The scheduler launches the task (state ≥ 4 in the original).
    pub fn castable(self) -> bool {
        matches!(self, State::Ready | State::InProgress | State::Fresh)
    }
}

/// Where to resume a task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resume {
    /// Not launched yet: the task, which starts at its first stage (`Task::first_step`).
    Entry(Task),
    /// Has ceded control; will resume at this Rust stage. The original saves its 15 registers and its SR at the
    /// cession and
    /// restores them; the probe `DDSOM_POISON=resume` (`diag::probe_launch`, all registers and flags inverted on
    /// resuming, 296 scenarios) shows that only a few are re-read, and they are named fields of the stages that
    /// re-read them (see `tasks::resume_step`). The restored SR is always that of user mode, interrupts
    /// allowed: no task cedes while masking them (`kernel::cede`).
    Step(Step),
    /// Tools: a slot of a table rebuilt from the reference's FF0064 table (`fuzz_couv`) whose entry or PC is not
    /// the entry of a task of the game.
    #[cfg(feature = "tools")]
    Address(u32),
}

/// What follows a call in progress, or a cession: a native at its address, or a Rust stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    Address(u32),
    Step(Step),
}

impl Next {
    /// Original address (the one the original pushes).
    pub fn address(self) -> u32 {
        match self {
            Next::Address(a) => a,
            Next::Step(e) => e.address(),
        }
    }
}

impl From<Step> for Next {
    fn from(e: Step) -> Self {
        Next::Step(e)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    /// The task occupying the slot (from its last entry; none before the first).
    pub task: Option<Task>,
    pub state: State,
    /// Remaining wait frames (kept as is by the states that do not use it, like the original).
    pub count: u8,
    /// 68000 stack given to the task at its creation, returned at its death.
    pub stack: u32,
    pub resume: Resume,
    /// Calls in progress of the task, held in Rust instead of its 68000 stack (see [`Frame`]).
    pub frames: Frames,
}

impl Default for Slot {
    fn default() -> Self {
        Slot {
            task: None,
            state: State::Free,
            count: 0,
            stack: 0,
            // a free slot: never resumed (`kernel::launch` and `gaps` skip it)
            resume: Resume::Entry(Task::Startup),
            frames: Frames::default(),
        }
    }
}

/// A call in progress of a task, in place of its 68000 stack. When a Rust step calls a subroutine that may
/// cede control (fade, black screen, object handler, world update...), the caller's continuation is stored
/// here instead of a pushed return address (what the subroutine keeps while it cedes is in its stage):
/// at each cession, the task's 68000 stack thus stays empty, and its resume depends only on the table (stage,
/// context, frames).
///
/// The return of a subroutine (`rts`) goes back to the most recent of the calls in progress: the task's last frame, or
/// a call by address opened after it ([`Table::calls`]). The depth of each says which; the 68000
/// stack pointer (A7) is no longer used for this.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    /// Number of calls outside frames in progress ([`Table::calls`]) when the frame was opened: a call by address
    /// opened after it (`crate::call`, nested) returns before it.
    pub calls: usize,
    /// When the callee returns (`rts` of a native, end of a stage, or terminal cession `jmp 0xa40`), the task
    /// continues at this continuation.
    pub next: Next,
    /// Call expected by a Rust function in progress (see [`call_expected`]): its return hands control back to that
    /// function instead of executing the continuation. The continuation is only used if the task cedes before.
    pub expected: Option<u32>,
}

/// Maximum depth of the calls in progress of a task (6 in the game: turn, frame end, phase, world, dispatch,
/// subroutine of a handler).
pub const DEPTH: usize = 8;

/// Stack of the calls in progress of a task (the last is the most recent).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frames {
    list: [Frame; DEPTH],
    n: usize,
}

impl Default for Frames {
    fn default() -> Self {
        Frames { list: [Frame { calls: 0, next: Next::Address(0), expected: None }; DEPTH], n: 0 }
    }
}

impl Frames {
    pub fn push(&mut self, frame: Frame) {
        assert!(self.n < DEPTH, "too many calls in progress for a task: {:x?}", self.iter().collect::<Vec<_>>());
        self.list[self.n] = frame;
        self.n += 1;
    }

    pub fn pop(&mut self) -> Option<Frame> {
        self.n = self.n.checked_sub(1)?;
        Some(self.list[self.n])
    }

    pub fn top(&self) -> Option<Frame> {
        self.n.checked_sub(1).map(|i| self.list[i])
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// From oldest to most recent.
    pub fn iter(&self) -> impl Iterator<Item = Frame> + '_ {
        self.list[..self.n].iter().copied()
    }

    /// The expected call `id` is no longer expected (the Rust function that was waiting for it abandons it): if it is
    /// still there, its
    /// return will execute its continuation.
    fn leave(&mut self, id: u32) {
        for frame in &mut self.list[..self.n] {
            if frame.expected == Some(id) {
                frame.expected = None;
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Table {
    pub slots: [Slot; SLOTS],
    /// Last task launched (the one that cedes when a cession wrapper is called).
    pub current: usize,
    /// Free stacks: the last is the next one given (the FF005A heap of the original, filled by `boot_init`).
    pub stacks: Vec<u32>,
    /// Where the processor is (boot or scheduler point, or task in progress).
    pub point: crate::kernel::Point,
    /// Slot that the scheduler examines (`kernel::Point::Exam`; the original's A0 = its record, D0.w = slots left).
    pub exam: usize,
    /// Interrupt mask (level of the original's SR): see `kernel::interrupt_mask`.
    pub interrupt_mask: u8,
    /// During the VBLANK interrupt (`Point::Interruption`): the point where the processor will return.
    pub interrupted: Option<crate::kernel::Point>,
    /// Number of the next expected call, and last expected call returned (see [`call_expected`]).
    next_call: u32,
    returned: Option<u32>,
    /// Calls in progress outside the task frames, from oldest to most recent: calls by address
    /// (`crate::call`), and, in the tools, natives called by the original code. They never cede control:
    /// none is in progress at a cession (they are therefore in no task).
    pub calls: Vec<OutsideFrame>,
    /// Leftover registers that the object handlers pass to each other (`dispatch::Leftovers`): a game state.
    /// The natives called by the original code (tools) find them in their register bench (D2-D5,
    /// `dispatch::Leftovers::to_bench`).
    pub leftovers: crate::dispatch::Leftovers,
    /// The A0 that a shared state of the scene exit leaves to the dispatch of its sequence (original glitch:
    /// `scene::exit::set_dispatch_a0`).
    pub scene_exit_a0: u32,
}

/// A call in progress outside the frames of a task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutsideFrame {
    /// Call by address of a Rust function (`crate::call`): the return of the native hands control back to it.
    ByAddress,
    /// Tools (machine that also runs the original code): native called by a 68000 `jsr`, whose return
    /// address is on the 68000 stack.
    By68000,
}

/// What the task becomes on the return of a subroutine (see [`ret`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Return {
    /// The continuation of the call in progress, stored by a Rust step.
    Next(Next),
    /// The Rust function that was waiting for this return resumes (expected call, call by address).
    Render,
    /// No call in progress held in Rust: the return address is on the 68000 stack (tools only; in the game,
    /// it is an error).
    Stack68000,
}

impl Table {
    /// Trap 0: task `k` (re)starts from `entry` with a new stack; that of a task already there is returned.
    pub fn create(&mut self, k: usize, task: Task) {
        if self.slots[k].state != State::Free {
            self.give_back_stack(k);
        }
        let stack = self.stacks.pop().expect("no free stack left for a task");
        self.slots[k] = Slot { task: Some(task), state: State::Fresh, stack, resume: Resume::Entry(task), ..Slot::default() };
    }

    /// Trap 1 (the task ends) and trap 2 on a live task.
    pub fn free(&mut self, k: usize) {
        self.slots[k].state = State::Free;
        self.slots[k].frames = Frames::default();
        self.give_back_stack(k);
    }

    /// Trap 2: kills task `k`, if it exists.
    pub fn kill(&mut self, k: usize) {
        if self.slots[k].state != State::Free {
            self.free(k);
        }
    }

    fn give_back_stack(&mut self, k: usize) {
        self.stacks.push(self.slots[k].stack);
    }

    /// Trap 5: suspends task `k`.
    pub fn suspend(&mut self, k: usize) {
        let e = &mut self.slots[k];
        e.state = State::Suspended;
        e.count = 0;
    }

    /// Trap 6: task `k` becomes ready again (with this count, which nothing reads).
    pub fn wake(&mut self, k: usize, count: u8) {
        let e = &mut self.slots[k];
        e.state = State::Ready;
        e.count = count;
    }

    /// Trap 7: the current task restarts from the entry of `task` (same stack).
    pub fn restart(&mut self, task: Task) {
        let e = &mut self.slots[self.current];
        e.task = Some(task);
        e.state = State::Fresh;
        e.count = 0;
        e.resume = Resume::Entry(task);
        e.frames = Frames::default();
    }

    /// Cession of the current task: new state, and where to resume it.
    pub fn cede(&mut self, state: State, count: Option<u8>, resume: Resume) {
        let e = &mut self.slots[self.current];
        e.state = state;
        if let Some(count) = count {
            e.count = count;
        }
        e.resume = resume;
    }

    /// One frame less for task `k` if it is waiting; at zero, it is ready.
    pub fn count_down(&mut self, k: usize) {
        let e = &mut self.slots[k];
        if e.state == State::Waiting {
            e.count = e.count.wrapping_sub(1);
            if e.count == 0 {
                e.state = State::Ready;
            }
        }
    }

    /// VBLANK wake-ups: each waiting task loses a frame; slot 0 (the first examined) only
    /// counts down one frame out of four as long as the slow-motion (`A5+0x4f`, counted down here) is not exhausted.
    pub fn count_down_frame(&mut self, slowdown: &mut u8) {
        for k in 0..SLOTS {
            if self.slots[k].state != State::Waiting {
                continue;
            }
            if *slowdown != 0 && k == 0 {
                *slowdown -= 1;
                if *slowdown & 3 != 0 {
                    continue;
                }
            }
            self.count_down(k);
        }
    }

    /// Calls in progress of the current task.
    pub fn frames(&mut self) -> &mut Frames {
        &mut self.slots[self.current].frames
    }
}

/// The machine's task table (created at first access).
#[inline]
pub fn table(b: &mut Cps2Bus) -> &mut Table {
    let ext = b.ext.get_or_insert_with(|| Box::new(Table::default()));
    ext.as_mut().as_any_mut().downcast_mut().expect("bus extension: task table")
}

/// The task table, if the machine has one (game machine; the original machine keeps its own in RAM).
#[inline]
pub fn read(b: &Cps2Bus) -> Option<&Table> {
    b.ext.as_deref()?.as_any().downcast_ref()
}

/// The task table, if the machine has one, without creating it.
#[inline]
pub(crate) fn table_if(b: &mut Cps2Bus) -> Option<&mut Table> {
    b.ext.as_deref_mut()?.as_any_mut().downcast_mut()
}

// ---------------------------------------------------------------------------------------------
// Calls in progress (see `Frame`)

/// A Rust step of the current task calls a subroutine that may cede control: when it returns, the
/// task will continue at `next`. Nothing is pushed on the 68000 stack.
pub fn call(b: &mut Cps2Bus, next: impl Into<Next>) {
    table(b).open_frame(next.into(), None);
}

/// Expected call (see [`call_expected`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub struct Call(u32);

/// A Rust function calls a native that returns via `rts` (an object's handler) and continues after it: the
/// return of the native hands control back to it ([`returned`]). If the native cedes control before, the task will
/// resume later at
/// `next`, the Rust function having abandoned.
pub fn call_expected(b: &mut Cps2Bus, next: impl Into<Next>) -> Call {
    let t = table(b);
    t.next_call = t.next_call.wrapping_add(1);
    let id = t.next_call;
    t.open_frame(next.into(), Some(id));
    Call(id)
}

/// Has the expected call returned? If not (the task ceded, or a callee popped the call to return higher), the
/// Rust function that was waiting for it abandons: its frame, if still there, will execute its continuation when the
/// callee returns.
pub fn returned(b: &mut Cps2Bus, call: Call) -> bool {
    let t = table(b);
    if t.returned == Some(call.0) {
        t.returned = None;
        return true;
    }
    t.frames().leave(call.0);
    false
}

impl Table {
    /// The most recent of the calls in progress of the current task: a call outside frames opened after the last
    /// frame (it
    /// is popped and returned), otherwise the last frame (popped and returned).
    pub(crate) fn pop_most_recent(&mut self) -> Option<Result<OutsideFrame, Frame>> {
        let top = self.frames().top();
        let height = top.map_or(0, |k| k.calls);
        assert!(
            self.calls.len() >= height,
            "calls outside frames {:?} returned before the frame {top:x?} opened during them",
            self.calls
        );
        if self.calls.len() > height {
            return self.calls.pop().map(Ok);
        }
        self.frames().pop().map(Err)
    }

    /// Return of a subroutine (`rts`): it goes back to the most recent of the calls in progress ([`Frame`]).
    pub fn ret(&mut self) -> Return {
        match self.pop_most_recent() {
            Some(Ok(OutsideFrame::ByAddress)) => Return::Render,
            Some(Ok(OutsideFrame::By68000)) | None => Return::Stack68000,
            Some(Err(Frame { expected: Some(id), .. })) => {
                self.returned = Some(id);
                Return::Render
            }
            Some(Err(frame)) => Return::Next(frame.next),
        }
    }

    /// A Rust step of the current task calls a subroutine that may cede; `expected`: expected call.
    fn open_frame(&mut self, next: Next, expected: Option<u32>) {
        let calls = self.calls.len();
        self.frames().push(Frame { calls, next, expected });
    }
}

/// Return of a subroutine (`rts`): it goes back to the most recent of the calls in progress ([`Frame`]).
pub fn ret(b: &mut Cps2Bus) -> Return {
    table_if(b).map_or(Return::Stack68000, Table::ret)
}

/// A call by address (`crate::call`) begins; its `rts` will close it ([`ret`]). Returns the depth to
/// return to.
pub fn open_call(b: &mut Cps2Bus) -> usize {
    let t = table(b);
    t.calls.push(OutsideFrame::ByAddress);
    t.calls.len() - 1
}

/// Has the call by address opened at depth `depth` returned?
pub fn call_returned(b: &Cps2Bus, depth: usize) -> bool {
    read(b).is_some_and(|t| t.calls.len() == depth)
}

/// Tools: the original code calls a native (its return address is on the 68000 stack). Returns the depth to
/// restore after it ([`close_calls`]) if the machine has a task table (without a table, all returns are on
/// the 68000 stack).
pub fn open_call_68000(b: &mut Cps2Bus) -> Option<usize> {
    let t = table_if(b)?;
    t.calls.push(OutsideFrame::By68000);
    Some(t.calls.len() - 1)
}

/// Tools: the native called by the original code is finished; if it did not return via `rts` (jump, cession),
/// its call is closed.
pub fn close_calls(b: &mut Cps2Bus, depth: Option<usize>) {
    if let (Some(p), Some(t)) = (depth, table_if(b)) {
        t.calls.truncate(p);
    }
}

/// Like [`ret`], for a terminal cession (`jmp 0xa40`) or an abandoned call: the continuation of the last frame,
/// whether it
/// is expected or not (the Rust function that was waiting for it sees that it did not return). A cession cannot
/// take place in a call by address.
pub fn pop_return(b: &mut Cps2Bus) -> Next {
    match table_if(b).and_then(Table::pop_most_recent) {
        Some(Err(frame)) => frame.next,
        Some(Ok(OutsideFrame::ByAddress)) => panic!("cession or abandon of the call in progress inside a call by address"),
        Some(Ok(OutsideFrame::By68000)) | None => panic!("cession or abandon without a call in progress"),
    }
}

/// The `n` most recent calls in progress of the current task are popped without returning (a routine that pops the
/// returns of its callers to return higher: foreign epilogue of the bubbles); returns their continuations, from the
/// most
/// recent to the oldest.
pub fn pop_calls(b: &mut Cps2Bus, n: usize) -> Vec<Next> {
    let frames = table(b).frames();
    (0..n)
        .map(|_| frames.pop().unwrap_or_else(|| panic!("{n} calls in progress to pop: there are not enough")).next)
        .collect()
}

/// Bytes of the original kernel RAM (offset from FF0000) that the game no longer holds in RAM: scheduler
/// stack, current task and top of the heap (FF0052-FF005D), records (FF0064-FF0163), stack heap
/// (FF0166-FF01A5), task stacks (FF01A6-FF0DA5) and supervisor stack (up to FF0E65). The tools that compare the
/// game RAM with that of the original ignore them and compare the table with [`gaps`]. FF0E66 and beyond are
/// game variables (relative to A5: `placement::LAST_X`...), compared like the rest of the RAM.
pub fn kernel_ram(offset: u32) -> bool {
    (0x52..0x5e).contains(&offset) || (0x64..0x164).contains(&offset) || (0x166..0xe66).contains(&offset)
}

// ---------------------------------------------------------------------------------------------
// Differential test: Rust table against the FF0064 table of an original machine

/// Fields of a record of the original.
mod field {
    pub const STATE: u32 = 0;
    pub const COUNT: u32 = 1;
    pub const PC: u32 = 4;
    pub const USP: u32 = 8;
    pub const STACK: u32 = 0xc;
}

/// Resumes of the original's cession wrappers (`0xa40`/`0xb64`, `0xa86`): they restore the 15 registers
/// saved on the task's stack then return (`rts`) to the resume address.
const ORIGINAL_RESUMES: [u32; 2] = [0xa4c, 0xa8c];
/// Stack heap of the original: top (FF005A) and bottom.
const HEAP: u32 = 0xff_005a;
const HEAP_BOTTOM: u32 = 0xff_01a6;

/// Slot of the main task (game, mode machine), which the freeze task counts down itself.
const MAIN: usize = 0;

/// The two states of the main task differ only by one frame counted down by the freeze task (fast forward).
/// When the game works to the end of the frame (fast forward of a loaded scene), the frame end falls at a
/// moment that depends on the duration of the routines, native or original: shortly before or after a pass of the
/// freeze, or even
/// in the middle (count decremented, state not yet "ready").
fn one_gap_countdown(rust: (State, u8), original: (u8, u8)) -> bool {
    let waits = State::Waiting.byte();
    let next = |(state, count): (u8, u8)| match (state == waits, count.wrapping_sub(1)) {
        (true, 0) => Some((State::Ready.byte(), 0)),
        (true, remainder) => Some((waits, remainder)),
        _ => None,
    };
    let rust = (rust.0.byte(), rust.1);
    // "running" equals "ready"; "waiting" at zero is a freeze countdown interrupted between the count and the state
    let normal = |(state, count): (u8, u8)| {
        let ready = state == State::InProgress.byte() || state == waits && count == 0;
        (if ready { State::Ready.byte() } else { state }, count)
    };
    let (rust, original) = (normal(rust), normal(original));
    rust == original || next(rust) == Some(original) || next(original) == Some(rust)
}

/// Differences between the Rust table and the FF0064 table of the RAM `ram` (FF0000-FFFFFF) of an original machine,
/// at the
/// same frame: state ("ready" and "running" merged: the frame end may fall while a task runs
/// in the original; for the main task, a one-frame gap counted down by the freeze is allowed, see
/// [`one_gap_countdown`]), count, stack, entry of a new task, and stack heap. With `all`, the resume address
/// of the tasks that ceded too (diagnostics): it may differ with no effect (a native sometimes resumes at an identical
/// copy of the original resume code, `0xf32cc` for `0xf5902`).
pub fn gaps(t: &Table, ram: &[u16], all: bool) -> Vec<String> {
    let word = |a: u32| ram[(a as usize & 0xffff) / 2];
    let byte = |a: u32| (word(a & !1) >> if a & 1 == 0 { 8 } else { 0 }) as u8;
    let long = |a: u32| (word(a) as u32) << 16 | word(a + 2) as u32;
    let mut gaps = Vec::new();
    for (k, e) in t.slots.iter().enumerate() {
        let r = record(k);
        let state = byte(r + field::STATE);
        let normal = |o: u8| if o == State::InProgress.byte() { State::Ready.byte() } else { o };
        let mut gap = |what: &str, rust: u32, original: u32| {
            if rust != original {
                gaps.push(format!("task {k}: {what} {rust:x} instead of {original:x}"));
            }
        };
        let count = byte(r + field::COUNT);
        if !(k == MAIN && one_gap_countdown((e.state, e.count), (state, count))) {
            gap("state", normal(e.state.byte()) as u32, normal(state) as u32);
            gap("count", e.count as u32, count as u32);
        }
        if e.state == State::Free || state == State::Free.byte() {
            continue;
        }
        gap("stack", e.stack, long(r + field::STACK));
        let in_progress = e.state == State::InProgress || state == State::InProgress.byte();
        match e.resume {
            Resume::Entry(task) => gap("entry", task.address(), long(r + field::PC)),
            #[cfg(feature = "tools")]
            Resume::Address(address) => gap("entry", address, long(r + field::PC)),
            // the stack of a running task has changed since its cession
            Resume::Step(step) if all && !in_progress => {
                let usp = long(r + field::USP);
                if !ORIGINAL_RESUMES.contains(&long(r + field::PC)) {
                    gap("resume (PC of the record)", step.address(), long(r + field::PC));
                    continue;
                }
                gap("resume", step.address(), long(usp + 60));
            }
            Resume::Step(_) => {}
        }
    }
    let top = if long(HEAP) == 0 { HEAP_BOTTOM } else { long(HEAP) };
    let heap: Vec<u32> = (top..HEAP_BOTTOM).step_by(4).rev().map(long).collect();
    if heap != t.stacks {
        gaps.push(format!("stack heap {:x?} instead of {:x?}", t.stacks, heap));
    }
    gaps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation_and_death_return_stacks_in_order() {
        let mut t = Table { stacks: vec![3, 2, 1], ..Table::default() };
        t.create(0, Task::Game);
        t.create(3, Task::Interface);
        assert_eq!((t.slots[0].stack, t.slots[3].stack), (1, 2));
        t.create(0, Task::Service);
        assert_eq!(t.slots[0].stack, 1, "the returned stack is reused immediately");
        t.kill(3);
        t.kill(3);
        assert_eq!(t.stacks, vec![3, 2]);
    }

    #[test]
    fn a_return_goes_back_to_the_most_recent_pending_call() {
        let mut t = Table::default();
        t.open_frame(Next::Address(0x1341a), None);
        t.open_frame(Next::Address(0x1376e), Some(1));
        // a nested call by address returns first
        t.calls.push(OutsideFrame::ByAddress);
        assert_eq!(t.ret(), Return::Render);
        assert!(t.calls.is_empty());
        assert_eq!(t.ret(), Return::Render);
        assert_eq!(t.returned, Some(1));
        // a call by address opened before a frame returns after it
        t.calls.push(OutsideFrame::ByAddress);
        t.open_frame(Next::Address(0x1376e), None);
        assert_eq!(t.ret(), Return::Next(Next::Address(0x1376e)));
        assert_eq!(t.ret(), Return::Render);
        assert_eq!(t.ret(), Return::Next(Next::Address(0x1341a)));
        assert_eq!(t.ret(), Return::Stack68000);
    }

    #[test]
    fn a_frame_counted_down_by_freeze_is_allowed() {
        assert!(one_gap_countdown((State::Waiting, 1), (4, 0)));
        assert!(!one_gap_countdown((State::Waiting, 2), (8, 0)));
        assert!(one_gap_countdown((State::Waiting, 2), (1, 3)));
        assert!(one_gap_countdown((State::Ready, 0), (1, 1)));
        assert!(!one_gap_countdown((State::Waiting, 3), (1, 1)));
        assert!(one_gap_countdown((State::Waiting, 1), (1, 0)));
        assert!(one_gap_countdown((State::Ready, 0), (1, 0)));
    }

    #[test]
    fn slowdown_counts_slot_0_down_one_frame_in_four() {
        let mut t = Table::default();
        for k in [0, 1] {
            t.slots[k] = Slot { state: State::Waiting, count: 2, ..Slot::default() };
        }
        let mut slowdown = 8;
        for _ in 0..4 {
            t.count_down_frame(&mut slowdown);
        }
        assert_eq!(slowdown, 4);
        assert_eq!((t.slots[0].state, t.slots[0].count), (State::Waiting, 1));
        assert_eq!(t.slots[1].state, State::Ready);
    }
}
