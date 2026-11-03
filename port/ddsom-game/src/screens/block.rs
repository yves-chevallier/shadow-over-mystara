//! A state block of a screen: a record of the work RAM that a screen (a transition script, the service menu...)
//! keeps its variables in, read and written by field offset. It replaces the local `get8`/`set8`/`get16`/`set16`
//! helpers that each screen used to declare.

use cps2::Cps2Bus;

use crate::objects::Obj;

/// A state block, designated by its address.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Block(pub u32);

impl Block {
    pub fn get8(self, b: &mut Cps2Bus, field: u32) -> u8 {
        Obj(self.0).u8(b, field)
    }
    pub fn set8(self, b: &mut Cps2Bus, field: u32, v: u8) {
        Obj(self.0).set_u8(b, field, v)
    }
    pub fn get16(self, b: &mut Cps2Bus, field: u32) -> u16 {
        Obj(self.0).u16(b, field)
    }
    pub fn set16(self, b: &mut Cps2Bus, field: u32, v: u16) {
        Obj(self.0).set_u16(b, field, v)
    }
    pub fn get32(self, b: &mut Cps2Bus, field: u32) -> u32 {
        Obj(self.0).u32(b, field)
    }
    pub fn set32(self, b: &mut Cps2Bus, field: u32, v: u32) {
        Obj(self.0).set_u32(b, field, v)
    }
}
