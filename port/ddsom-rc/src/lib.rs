//! Code 68000 de ddsom recompilé en Rust. Brancher sur la machine avec `install`.

#[allow(clippy::all, unused_braces, unused_imports, unused_variables)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/mod.rs"));
}

pub use generated::{run_block, BLOCKS};

/// Active l'exécution du code recompilé sur la machine (l'interpréteur reste le filet de sécurité).
pub fn install(m: &mut cps2::Machine) {
    m.executor = Some(run_block);
}
