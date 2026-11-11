//! Typed records in RAM: [`record!`] declares the fields of a record once (name, type, offset) and produces their
//! accessors (docs/fin.md, "Vues typées").
//!
//! ```ignore
//! record! {
//!     impl Obj {
//!         /// Abscissa (16.16, pixels).
//!         x, set_x, X: u32 @ 0x08;
//!     }
//! }
//! ```
//!
//! declares the offset `Obj::X` (for the code that still passes offsets around), `obj.x(b)` and `obj.set_x(b, v)`.
//! The record type must have a method `at(self, off: u32) -> u32` giving the address of a field.

use crate::world::World;

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8};

/// A value stored in RAM, big-endian like the 68000.
pub trait Value: Sized {
    fn read(b: &mut World, a: u32) -> Self;
    fn write(self, b: &mut World, a: u32);
}

macro_rules! value {
    ($($t:ty: $u:ty, $rd:ident, $wr:ident;)*) => {$(
        impl Value for $t {
            fn read(b: &mut World, a: u32) -> Self {
                $rd(b, a) as $t
            }
            fn write(self, b: &mut World, a: u32) {
                $wr(b, a, self as $u)
            }
        }
    )*};
}

value! {
    u8: u8, rd8, wr8;
    i8: u8, rd8, wr8;
    u16: u16, rd16, wr16;
    i16: u16, rd16, wr16;
    u32: u32, rd32, wr32;
    i32: u32, rd32, wr32;
}

/// Declares the fields of a record: `getter, setter, OFFSET: type @ offset;` per field (see the module).
macro_rules! record {
    (impl $ty:ident { $( $(#[$doc:meta])* $get:ident, $set:ident, $off:ident: $t:ident @ $at:expr; )* }) => {
        impl $ty {
            $(
                $(#[$doc])*
                pub const $off: u32 = $at;

                $(#[$doc])*
                pub fn $get(self, b: &mut crate::world::World) -> $t {
                    <$t as $crate::record::Value>::read(b, self.at(Self::$off))
                }

                pub fn $set(self, b: &mut crate::world::World, v: $t) {
                    <$t as $crate::record::Value>::write(v, b, self.at(Self::$off))
                }
            )*
        }
    };
}

pub(crate) use record;
