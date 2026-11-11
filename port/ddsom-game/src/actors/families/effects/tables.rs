//! The tables of the EFFCT family (`data().effects`, declared in `ddsom_data::effects`), read like the original:
//! at a byte offset from the start of a table. A read past the extracted bytes goes to the residue (counted, see
//! `crate::world::data`).

use crate::world::World;

pub(crate) use ddsom_data::effects::{EffectTables, Table};

use crate::world::data::{data, Missing};

/// The tables of the family.
pub(crate) fn tables() -> &'static EffectTables {
    &data().effects
}

/// Reads of a table, by byte offset (`at*`) or by element index (`nth*`).
pub(crate) trait Read {
    fn at8(&self, b: &mut World, off: u32) -> u8;
    fn at16(&self, b: &mut World, off: u32) -> u16;
    fn at32(&self, b: &mut World, off: u32) -> u32;

    /// Word `i` (offset `2 i`).
    fn nth16(&self, b: &mut World, i: u32) -> u16 {
        self.at16(b, i.wrapping_mul(2))
    }
    /// Long `i` (offset `4 i`).
    fn nth32(&self, b: &mut World, i: u32) -> u32 {
        self.at32(b, i.wrapping_mul(4))
    }
}

impl Read for Table {
    fn at8(&self, b: &mut World, off: u32) -> u8 {
        self.byte(off).unwrap_or_else(|| {
            let m = Missing { table: self.name, key: self.rom };
            let a = self.rom.wrapping_add(off);
            m.byte_in(b, a, self.program)
        })
    }

    fn at16(&self, b: &mut World, off: u32) -> u16 {
        self.word(off).unwrap_or_else(|| {
            let m = Missing { table: self.name, key: self.rom };
            let a = self.rom.wrapping_add(off);
            m.word_in(b, a, self.program)
        })
    }

    fn at32(&self, b: &mut World, off: u32) -> u32 {
        self.long(off).unwrap_or_else(|| {
            let m = Missing { table: self.name, key: self.rom };
            let a = self.rom.wrapping_add(off);
            m.long_in(b, a, self.program)
        })
    }
}

/// A table entry designated by its address, which the original keeps in the state of an object (a pointer into a
/// table, read in data space): the table that holds it, and the offset of the address in it.
pub(crate) fn held(a: u32) -> Option<(&'static Table, u32)> {
    tables().find(a, false).map(|t| (t, a - t.rom))
}

fn residue(a: u32) -> Missing {
    Missing { table: "effects (by address)", key: a }
}

/// Byte at the address `a` held in the state (data space).
pub(crate) fn held8(b: &mut World, a: u32) -> u8 {
    match held(a) {
        Some((t, off)) => t.at8(b, off),
        None => residue(a).read8(b, a),
    }
}

/// Word at the address `a` held in the state (data space).
pub(crate) fn held16(b: &mut World, a: u32) -> u16 {
    match held(a) {
        Some((t, off)) => t.at16(b, off),
        None => residue(a).read16(b, a),
    }
}

/// Word at byte `offset` of the speed table `table` that an object keeps (`Obj::SPEED_TABLE`), like
/// `objects::motion` reads it.
pub(crate) fn speed_word(b: &mut World, table: u32, offset: u32) -> u16 {
    let a = table.wrapping_add(offset);
    match data().speeds.at(crate::world::data::RomAddr(a)) {
        Some(v) => v,
        None => Missing { table: "speed table", key: table }.read16(b, a),
    }
}
