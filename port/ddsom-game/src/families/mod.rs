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

use crate::objects::Obj;
use crate::Native;

/// All the native routines of the families.
pub const NATIVES: &[Native] = &[
    items::ITEM_0,
    items::ITEM_1,
    items::ITEM_3,
    items::ITEM_4,
    items::ITEM_5,
    items::ITEM_6,
    items::ITEM_7,
    items::ITEM_8,
    items::ITEM_9,
    boxes::BOX_1,
    boxes::BOX_2,
    effects::EFFCT_1,
    effects::EFFCT_4,
    effects::EFFCT_5,
    effects::EFFCT_6,
    effects::EFFCT_7,
    effects::EFFCT_8,
    effects::EFFCT_9,
    effects::EFFCT_15,
    effects::EFFCT_17,
    effects::EFFCT_19,
    effects::EFFCT_21,
    effects::EFFCT_24,
    effects::EFFCT_25,
    effects::EFFCT_26,
    effects::EFFCT_27,
    effects::EFFCT_28,
    effects::EFFCT_29,
    effects::EFFCT_32,
    effects::EFFCT_33,
    effects::EFFCT_45,
    effects::EFFCT_51,
    effects::EFFCT_52,
    effects::EFFCT_56,
    effects::EFFCT_57,
    effects::EFFCT_62,
];

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
        pub const $const: crate::Native = crate::families::cx::handler_without($entry, $name, |b, a| {
            $logic(&mut crate::families::cx::Gx::dispatch(b, a));
        });
    };
}
pub(crate) use handler;

/// Native routines of the ESHL, SET and SOBJ families (see `docs/families-eshl-set-sobj.md`): one handler per type,
/// written on [`cx::Cx`].
pub fn natives() -> impl Iterator<Item = &'static Native> {
    [eshl::NATIVES, eshl_b::NATIVES, eshl_c::NATIVES, eshl_d::NATIVES, set::NATIVES, set_b::NATIVES, set_c::NATIVES, sobj::NATIVES, effects::b::NATIVES, effects::c::NATIVES, effects::d::NATIVES, effects::e::NATIVES, effects::f::NATIVES, enemy::NATIVES, enemy_double::NATIVES, enemy_part::NATIVES, enemy_riyakaa::NATIVES, enemy_dbeast::NATIVES, enemy_owlbear::NATIVES, enemy_harpy::NATIVES, enemy_warrior::NATIVES, enemy_skeleton::NATIVES, enemy_elf::NATIVES, enemy_trog::NATIVES, enemy_ghoul::NATIVES, enemy_beetle::NATIVES, enemy_scorpion::NATIVES, enemy_gargoyle::NATIVES, enemy_hellhound::NATIVES, enemy_ogre::NATIVES, enemy_kobold::NATIVES, enemy_goblin::NATIVES, enemy_troll::NATIVES, enemy_gnoll::NATIVES, enemy_bow::NATIVES, enemy_golem::NATIVES, enemy_warmachine::NATIVES, boss_dragon::NATIVES, boss_17::NATIVES, boss_6::NATIVES, boss_10::NATIVES, boss_5::NATIVES, boss_21::NATIVES, boss_19::NATIVES, boss_15::NATIVES, boss_18::NATIVES, boss_4::NATIVES, boss_7::NATIVES, boss_8::NATIVES, boss_11::NATIVES, boss_1::NATIVES, boss_9::NATIVES]
        .into_iter()
        .flatten()
        .chain(items_b::NATIVES)
        .chain(boxes::chest::NATIVES)
        .chain(boxes::pushes::NATIVES)
        .chain(boss_0::NATIVES)
        .chain(boss_16::NATIVES)
        .chain(boss_22::NATIVES)
        .chain(small::SHAKES)
        .chain(effects::g::NATIVES)
        .chain(player::natives())
        .chain(reactions::NATIVES)
        .chain(boss_3::NATIVES)
}
