use super::*;

const ANIM_BY_ADVICE: u32 = 0xdf9ae;
const EVENT_BY_ADVICE: u32 = 0xdfa0e;
const LAST_ADVICE: u16 = 0x136;
const BLINK_FRAMES: u16 = 0x14;
const BLINKS: u8 = 0xa;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(0x42) != 0 {
        x.set32(0x4, 0x0600_0000);
    }
    if suspended(x) {
        // a hidden or suspended arrow disappears
        return x.set32(0x4, 0x0600_0000);
    }
    match x.u8(0x4) {
        0 => start(x),
        2 => step(x),
        _ => x.free(&pools::SOBJ),
    }
}

fn advice_of_owner<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    let owner = x.linked(0x46);
    owner.u16(x.b, 0x14)
}

fn start<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(0x46) == 0 {
        return;
    }
    let advice = advice_of_owner(x);
    if advice == LAST_ADVICE {
        return x.set8(0x4, 4);
    }
    x.add8(0x4, 2);
    x.set16(0x16, BLINK_FRAMES);
    x.set8(0x18, BLINKS);
    place(x, 0xa7f);
    announce(x);
    pick_anim(x);
    x.reinsert();
}

fn place<R: Bench>(x: &mut Cx<'_, R>, dz: u16) {
    let v = x.a5_u16(ram::CAMERA_X).wrapping_add(0x58);
    x.set16(0x8, v);
    let v = x.a5_u16(ram::CAMERA_Y).wrapping_add(dz);
    x.set16(0xc, v);
    x.set32(0x10, 0);
}

/// Four consecutive events (codes +0x100 per player).
fn announce<R: Bench>(x: &mut Cx<'_, R>) {
    let advice = advice_of_owner(x);
    let i = advice.wrapping_sub(0x118).wrapping_mul(2);
    let code = x.rom16(EVENT_BY_ADVICE.wrapping_add(i as i16 as i32 as u32));
    // one event per player (high byte: 0 to 3)
    for player in 0..4u16 {
        crate::events::enqueue_points(x.b, code.wrapping_add(player << 8));
    }
}

fn pick_anim<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(0x46) == 0 {
        return;
    }
    let advice = advice_of_owner(x);
    let i = advice.wrapping_sub(0x118).wrapping_mul(2);
    let anim = x.rom16(ANIM_BY_ADVICE.wrapping_add(i as i16 as i32 as u32)) as u32;
    x.anim_start(ANIMS, anim as u16);
}

fn step<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u16(0x16).wrapping_sub(1);
    x.set16(0x16, n);
    if (n as i16) < 0 {
        let v = x.u8(0x1) ^ 2;
        x.set8(0x1, v);
        x.set16(0x16, BLINK_FRAMES);
        let n = x.u8(0x18).wrapping_sub(1);
        x.set8(0x18, n);
        if (n as i8) < 0 {
            return x.set8(0x4, 4);
        }
    }
    place(x, 0xaf);
}
