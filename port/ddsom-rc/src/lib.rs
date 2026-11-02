//! ddsom 68000 code recompiled to Rust. Plug it into the machine with `install`.

#[allow(clippy::all, unused_braces, unused_imports, unused_variables)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/mod.rs"));
}

pub use generated::{run_block, BLOCKS};

/// Enables execution of the recompiled code on the machine (the interpreter remains the safety net).
pub fn install(m: &mut cps2::Machine) {
    m.executor = Some(run_block);
}
