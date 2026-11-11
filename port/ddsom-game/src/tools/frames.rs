//! Tools: returns of the natives run by the processor (the original code calls them; their `rts`).

use crate::world::World;
use m68k::Cpu;

use crate::core::dispatch::{Dispatch, Leftovers};
use crate::world::mem::rd32;
use crate::world::objects::Obj;
use crate::core::tasks::table::{table_if, Next, OutsideFrame, Table};
use crate::core::tasks::return_to_caller;

/// Where the original code resumes after a native that returned to it.
#[derive(Clone, Copy)]
enum ToolsReturn {
    Address(u32),
    /// The return address popped from the 68000 stack.
    Stack,
}

thread_local! {
    static TOOLS_RETURN: std::cell::Cell<Option<ToolsReturn>> = const { std::cell::Cell::new(None) };
}

/// A native called by the original code returns to it (`tasks::return_to_caller`): at `address`, or at the address on
/// the 68000 stack (`None`). The processor jumps there at the end of the native ([`apply_tools_return`]).
pub(crate) fn original_code_resumes(address: Option<u32>) {
    let r = match address {
        Some(a) => ToolsReturn::Address(a),
        None => ToolsReturn::Stack,
    };
    TOOLS_RETURN.with(|t| t.set(Some(r)));
}

/// `rts` of a native on the processor: [`return_to_caller`], then, in the tools, the PC of the original code that
/// resumes.
pub(crate) fn return_to_caller_68k(c: &mut Cpu, b: &mut World) {
    return_to_caller(b);
    apply_tools_return(c, b);
}

/// Tools: a native returned to the original code ([`return_to_caller`]): its PC.
pub fn apply_tools_return(c: &mut Cpu, b: &mut World) {
    match TOOLS_RETURN.with(|r| r.take()) {
        None => {}
        Some(ToolsReturn::Address(a)) => c.pc = a,
        Some(ToolsReturn::Stack) => c.pc = pop_68000(c, b),
    }
}

/// Tools: pops the return address from the 68000 stack (`rts` of a native called by the original code).
pub(crate) fn pop_68000(c: &mut Cpu, b: &mut World) -> u32 {
    let address = rd32(b, c.a[7]);
    c.a[7] = c.a[7].wrapping_add(4);
    address
}

/// [`pop_return`] for a native called by the original code (tools): its return address may be on the 68000 stack.
pub(crate) fn pop_return_68k(c: &mut Cpu, b: &mut World) -> Next {
    match table_if(b).and_then(Table::pop_most_recent) {
        Some(Err(frame)) => frame.next,
        Some(Ok(OutsideFrame::ByAddress)) => panic!("cession or abandon of the call in progress inside a call by address"),
        Some(Ok(OutsideFrame::By68000)) | None => {
            assert!(!crate::core::frame::active(), "cession or abandon without a call in progress");
            Next::Address(pop_68000(c, b))
        }
    }
}

/// Returns to the caller (RTS): to the continuation of the task's current call if it was made by a Rust step (a Rust
/// step runs immediately), otherwise to the popped return address (see `tasks::table::Frame`).
pub fn rts(c: &mut m68k::Cpu, b: &mut World) {
    crate::tools::frames::return_to_caller_68k(c, b);
}

impl Leftovers {
    /// The code on the processor has finished: its leftovers (D2-D5 of the bench) become those of the game (task
    /// table).
    pub(crate) fn from_bench(c: &Cpu, b: &mut World) {
        crate::core::tasks::table::table(b).leftovers = Leftovers { d2: c.d[2], d3: c.d[3], d4: c.d[4], d5: c.d[5] };
    }

    /// The code on the processor resumes: the game's leftovers return to the bench (D2-D5).
    pub(crate) fn to_bench(c: &mut Cpu, b: &mut World) {
        let r = crate::core::tasks::table::table(b).leftovers;
        (c.d[2], c.d[3], c.d[4], c.d[5]) = (r.d2, r.d3, r.d4, r.d5);
    }
}

impl Dispatch {
    /// The values that the original finds in the registers on entry to the handler.
    pub(crate) fn from_cpu(c: &Cpu) -> Self {
        Dispatch { object: Obj(c.a[0]), counter: c.d[7] as u16, leftovers: Leftovers { d2: c.d[2], d3: c.d[3], d4: c.d[4], d5: c.d[5] } }
    }

    /// Returns the leftovers to the processor (the handler leaves the other registers as they are).
    pub(crate) fn give_back(&self, c: &mut Cpu) {
        c.d[2] = self.leftovers.d2;
        c.d[3] = self.leftovers.d3;
        c.d[4] = self.leftovers.d4;
        c.d[5] = self.leftovers.d5;
    }
}
