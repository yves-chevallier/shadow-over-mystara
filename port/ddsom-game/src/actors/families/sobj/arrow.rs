use super::*;
use crate::actors::families::shell::vars;
use crate::world::ram;
use crate::actors::families::shell::records::view;
use crate::actors::families::shell::tables::{tables, Read};

const LAST_ADVICE: u16 = 0x136;
const BLINK_FRAMES: u16 = 0x14;
const BLINKS: u8 = 0xa;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if vars::FROZEN.get(x.b) != 0 {
        x.set32(Obj::STATE, 0x0600_0000);
    }
    if suspended(x) {
        // a hidden or suspended arrow disappears
        return x.set32(Obj::STATE, 0x0600_0000);
    }
    match x.u8(Obj::STATE) {
        0 => start(x),
        2 => step(x),
        _ => x.free(&pools::SOBJ),
    }
}

fn advice_of_owner<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    let owner = x.linked(Arrow::OWNER);
    owner.u16(x.b, super::hint::Hint::ADVICE)
}

fn start<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(Arrow::OWNER) == 0 {
        return;
    }
    let advice = advice_of_owner(x);
    if advice == LAST_ADVICE {
        return x.set8(Obj::STATE, 4);
    }
    x.add8(Obj::STATE, 2);
    x.set16(Arrow::BLINK_TIMER, BLINK_FRAMES);
    x.set8(Arrow::BLINKS, BLINKS);
    place(x, 0xa7f);
    announce(x);
    pick_anim(x);
    x.reinsert();
}

fn place<R: Bench>(x: &mut Cx<'_, R>, dz: u16) {
    let v = ram::CAMERA_X.get(x.b).wrapping_add(0x58);
    x.set16(Obj::X, v);
    let v = ram::CAMERA_Y.get(x.b).wrapping_add(dz);
    x.set16(Obj::Z, v);
    x.set32(Obj::Y, 0);
}

/// Four consecutive events (codes +0x100 per player).
fn announce<R: Bench>(x: &mut Cx<'_, R>) {
    let advice = advice_of_owner(x);
    let i = advice.wrapping_sub(0x118).wrapping_mul(2);
    let code = tables().sobj_arrow_events_by_advice.at16(x.b, i as i16 as i32 as u32);
    // one event per player (high byte: 0 to 3)
    for player in 0..4u16 {
        crate::world::events::enqueue_points(x.b, code.wrapping_add(player << 8));
    }
}

fn pick_anim<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(Arrow::OWNER) == 0 {
        return;
    }
    let advice = advice_of_owner(x);
    let i = advice.wrapping_sub(0x118).wrapping_mul(2);
    let anim = tables().sobj_arrow_anims_by_advice.at16(x.b, i as i16 as i32 as u32) as u32;
    x.anim_start(ANIMS.0, anim as u16);
}

fn step<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u16(Arrow::BLINK_TIMER).wrapping_sub(1);
    x.set16(Arrow::BLINK_TIMER, n);
    if (n as i16) < 0 {
        let v = x.u8(Obj::FLAGS) ^ 2;
        x.set8(Obj::FLAGS, v);
        x.set16(Arrow::BLINK_TIMER, BLINK_FRAMES);
        let n = x.u8(Arrow::BLINKS).wrapping_sub(1);
        x.set8(Arrow::BLINKS, n);
        if (n as i8) < 0 {
            return x.set8(Obj::STATE, 4);
        }
    }
    place(x, 0xaf);
}

view! {
    /// Fields of the arrow (types 7, 9, 10).
    Arrow {
        /// Frames before the next blink.
        blink_timer, set_blink_timer, BLINK_TIMER: u16 @ 0x16;
        /// Blinks left.
        blinks, set_blinks, BLINKS: u8 @ 0x18;
        /// Owner (16-bit pointer).
        owner, set_owner, OWNER: u16 @ 0x46;
    }
}
