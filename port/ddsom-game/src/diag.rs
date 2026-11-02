//! Refactoring checks (`docs/guide-refactorisation.md`, "dead registers").
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

use std::sync::OnceLock;

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
