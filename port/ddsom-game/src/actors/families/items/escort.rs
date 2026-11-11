use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::records::Base;
use crate::actors::families::shell::tables::{tables, Read};

// ---------------------------------------------------------------------------------------------
// Type 9: icon that accompanies an object (0xdd5a2)

pub const ANIMATIONS: RomAddr = RomAddr(0x1c03a6);
pub const TARGET: u32 = 0x5c;
pub const DURATION: u32 = 0x14;

pub(super) fn escort_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    if !(state != 0) {
        add_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, Base::OWN_PALETTE, 1);
        let variant = o.u8(ctx.b, Obj::VARIANT);
        let palette = tables().item_escort_palettes.at8(ctx.b, variant as u32);
        o.set_u8(ctx.b, Base::PALETTE, palette);
        o.set_u16(ctx.b, DURATION, 0x100);
        ctx.anim_start(ANIMATIONS.0, 0x8e + variant as u16);
        return ctx.reinsert();
    }
    let target = pointer(ctx, o, TARGET);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = target.u16(ctx.b, off);
        o.set_u16(ctx.b, off, v);
    }
    let y = o.u16(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u16(ctx.b, Obj::Y, y);
    ctx.anim_step();
    let remainder = o.u16(ctx.b, DURATION).wrapping_sub(1);
    o.set_u16(ctx.b, DURATION, remainder);
    if remainder as i16 >= 0 {
        return;
    }
    ctx.free(&pools::ITEM);
}
