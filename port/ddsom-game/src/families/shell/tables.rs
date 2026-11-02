//! The tables of the ESHL, SOBJ, ITEM and BOX families (`data().objects`, declared in `ddsom_data::objects`), read like
//! the original: at a byte offset from the start of a table ([`Read`], shared with the effects). A read past the
//! extracted bytes goes to the residue (counted, see `crate::data`).

use cps2::Cps2Bus;

pub(crate) use crate::families::effects::tables::Read;
pub(crate) use ddsom_data::objects::ObjectTables;

use crate::data::{data, Missing};

/// The tables of the families.
pub(crate) fn tables() -> &'static ObjectTables {
    &data().objects
}

/// A table entry designated by its address, which the original keeps in the state of an object or reads from another
/// table (a pointer into a table): the table that holds it, and the offset of the address in it.
fn held(a: u32, program: bool) -> Option<(&'static ddsom_data::effects::Table, u32)> {
    tables().find(a, program).map(|t| (t, a - t.rom))
}

fn residue(a: u32) -> Missing {
    Missing { table: "objects (by address)", key: a }
}

/// Byte at the address `a` of a table (data space).
pub(crate) fn held8(b: &mut Cps2Bus, a: u32) -> u8 {
    match held(a, false) {
        Some((t, off)) => t.at8(b, off),
        None => residue(a).read8(b, a),
    }
}

/// Word at the address `a` of a table (data space).
pub(crate) fn held16(b: &mut Cps2Bus, a: u32) -> u16 {
    match held(a, false) {
        Some((t, off)) => t.at16(b, off),
        None => residue(a).read16(b, a),
    }
}

/// Long at the address `a` of a table (data space).
pub(crate) fn held32(b: &mut Cps2Bus, a: u32) -> u32 {
    match held(a, false) {
        Some((t, off)) => t.at32(b, off),
        None => residue(a).read32(b, a),
    }
}

/// Byte at the address `a` of the collision boxes (`sprites/tables.json`, `box_blocks`, data space), designated by
/// an address the code computes (a box set plus 8 bytes per box).
pub(crate) fn box_byte(b: &mut Cps2Bus, a: u32) -> u8 {
    let blocks = &data().sprite_tables.box_blocks;
    let byte = blocks.partition_point(|k| k.rom <= a).checked_sub(1).and_then(|i| {
        let k = &blocks[i];
        let off = (a - k.rom) as usize;
        k.boxes.get(off / 8).map(|h| h.bytes()[off % 8]).or_else(|| k.tail.get(off - 8 * k.boxes.len()).copied())
    });
    byte.unwrap_or_else(|| Missing { table: "collision boxes", key: a }.read8(b, a))
}
