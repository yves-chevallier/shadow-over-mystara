//! Diagnostics: refactoring checks (`docs/guide-refactorisation.md`, "dead registers") and the last natives run
//! (stops of the game, `core::frame`).
//!
//! A routine converted to an ordinary Rust function stops writing the registers that the original left (A1 of an
//! allocation, D0/D1/A4 of a release, flags...). Before dropping them, one must know that nobody
//! rereads them: the Rust caller, but also what follows (next handler of the dispatch loop, task that inherits the
//! registers, code that tests a flag). The `_68k` adapter that still writes them calls
//! [`dead_regs`] right after: with `DDSOM_POISON=label,...`, these registers are then **inverted** (value
//! complemented, flags inverted), which guarantees a wrong value. If validation stays VALID, no reader
//! exists in the scenarios: the adapter can be replaced by the pure function. Otherwise the difference points to the
//! reader (frame, address); the write is kept at that exact place, commented out.
//!
//! Without the variable, `dead_regs` costs only one atomic read.

use std::cell::Cell;
use std::sync::OnceLock;

use crate::world::World;
use crate::Native;

/// Poisoned labels (`DDSOM_POISON`, comma-separated; `all` takes them all).
pub(crate) fn labels() -> &'static [String] {
    static E: OnceLock<Vec<String>> = OnceLock::new();
    E.get_or_init(|| {
        std::env::var("DDSOM_POISON").map(|v| v.split(',').map(str::to_owned).collect()).unwrap_or_default()
    })
}

/// The label is poisoned.
pub fn poison_active(label: &str) -> bool {
    let e = labels();
    !e.is_empty() && e.iter().any(|x| x == label || x == "all")
}

/// The last natives run, oldest first from `next` (a ring).
#[derive(Clone, Copy)]
struct Last {
    natives: [Option<&'static Native>; LAST_NATIVES],
    next: usize,
}

/// Natives kept for the diagnostic of a stop.
const LAST_NATIVES: usize = 8;

thread_local! {
    /// Last natives run (diagnosing a stop).
    static LAST: Cell<Last> = const { Cell::new(Last { natives: [None; LAST_NATIVES], next: 0 }) };
}

/// Notes the native about to run (handler of a dispatch table), for diagnosing a stop (`DDSOM_SANS_TRACE=frame`:
/// each native of the frame).
#[inline]
pub(crate) fn note(b: &World, n: &'static Native) {
    LAST.with(|last| {
        let mut l = last.get();
        l.natives[l.next] = Some(n);
        l.next = (l.next + 1) % LAST_NATIVES;
        last.set(l);
    });
    if trace(b) {
        eprintln!("{}:{} {:06x} {} mask={}", b.hw.frame, b.hw.scanline, n.entry, n.name, crate::world::tasks::traps::interrupt_mask(b));
    }
}

/// Diagnostics: `DDSOM_SANS_TRACE=n` prints each native run during frame n.
fn trace(b: &World) -> bool {
    static IMAGE: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    *IMAGE.get_or_init(|| std::env::var("DDSOM_SANS_TRACE").ok().and_then(|v| v.parse().ok())) == Some(b.hw.frame)
}

/// The last natives run, oldest first (diagnostic of a stop).
pub(crate) fn last_natives() -> Vec<&'static Native> {
    let l = LAST.with(Cell::get);
    (0..LAST_NATIVES).filter_map(|k| l.natives[(l.next + k) % LAST_NATIVES]).collect()
}
