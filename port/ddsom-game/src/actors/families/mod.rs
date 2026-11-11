//! Families of ITEM (pickups), BOX (chests) and EFFCT (effects) objects: one handler per type.
//!
//! The dispatch loop of a class (`0x39f34` for ITEM, `0x39e50` BOX, `0x39cba` EFFCT) calls, for each active object, the
//! handler of the object's type (`jsr (table + 4 × type)`) with `A0` = object and `D7` = loop counter. Each handler is
//! a small state machine (bytes `+4`, `+5`, `+6` of the object); it yields with `rts`, leaving `A0` and `D7` intact.
//!
//! A native routine replaces a whole handler: the logic is written on a generic [`cx::Cx`], called without registers
//! (see `handler!` and `cx::handlers!`).
//!
//! See `docs/familles.md` for the description of each type.

pub mod boss_common;
pub mod boss_0;
pub mod boss_16;
pub mod boss_22;
pub mod boss_17;
pub mod boss_6;
pub mod boss_10;
pub mod boss_5;
pub mod boss_21;
pub mod boss_19;
pub mod boss_15;
pub mod boss_18;
pub mod boss_4;
pub mod boss_7;
pub mod boss_8;
pub mod boss_11;
pub mod boss_dragon;
pub mod boss_1;
pub mod boss_3;
pub mod boss_9;
pub mod boxes;
pub mod creators;
pub mod small;
pub mod cx;
pub mod effects;
pub mod effects_creators;
pub mod items_b;
pub mod eshl_creators;
pub mod enemy;
pub mod enemy_beetle;
pub mod enemy_common;
pub mod enemy_dbeast;
pub mod enemy_double;
pub mod enemy_gargoyle;
pub mod enemy_kobold;
pub mod enemy_goblin;
pub mod enemy_troll;
pub mod enemy_gnoll;
pub mod enemy_bow;
pub mod enemy_golem;
pub mod enemy_warmachine;
pub mod enemy_lib;
pub mod enemy_ogre;
pub mod enemy_elf;
pub mod enemy_ghoul;
pub mod enemy_harpy;
pub mod enemy_hellhound;
pub mod enemy_owlbear;
pub mod enemy_part;
pub mod reactions;
pub mod enemy_warrior;
pub mod enemy_riyakaa;
pub mod enemy_scorpion;
pub mod enemy_trog;
pub mod enemy_skeleton;
pub mod eshl;
pub mod eshl_b;
pub mod eshl_c;
pub mod eshl_d;
pub mod items;
pub mod player;
pub mod set;
pub mod set_b;
pub mod set_residue;
pub mod scenery;
pub mod set_c;
pub mod shell;
pub mod sobj;

use crate::world::objects::Obj;

/// `addq.b #n,(off,A0)`: adds `n` to a byte of the object (modulo 256).
pub(crate) fn add_u8<R>(x: &mut cx::Cx<'_, R>, o: Obj, off: u32, n: u8) {
    let v = o.u8(x.b, off).wrapping_add(n);
    o.set_u8(x.b, off, v);
}

/// Object designated by a sign-extended 16-bit RAM pointer (`movea.w (off,A0),An`).
pub(crate) fn pointer<R>(x: &mut cx::Cx<'_, R>, o: Obj, off: u32) -> Obj {
    Obj(o.u16(x.b, off) as i16 as i32 as u32 & 0xff_ffff)
}

/// Declares the native routine of a handler: entry address, name, logic (on a generic `cx::Cx`, called without
/// registers: what the dispatch loop passes).
macro_rules! handler {
    ($const:ident, $entry:expr, $name:expr, $logic:path) => {
        pub const $const: crate::Native = crate::actors::families::cx::handler_without($entry, $name, |b, a| {
            $logic(&mut crate::actors::families::cx::Gx::dispatch(b, a));
        });
    };
}
pub(crate) use handler;

