use super::*;

const ANIMS: u32 = 0x17b9c2;
/// Offset tables (words, program space) to frame arrays (longs).
const FRAMES_EVEN: u32 = 0x9e638;
const FRAMES_ODD: u32 = 0x9e6a4;
/// Choice of the odd frame: bytes (frame, flip) indexed by `+0x42`, `+0x43`.
const MIRRORED: u32 = 0x9e6bc;
/// Boxes per value of `+0x2f`: 4 bytes (index of the body, of the push, of the attack, then an index stored in
/// `+0x3b`).
const BOXES: u32 = 0x9e55c;
const HOST: u32 = 0x46;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    if (x.u16(HOST) as i16) >= 0 {
        return vanish(x);
    }
    let host = x.linked(HOST);
    if host.u8(x.b, 0) == 0 {
        return vanish(x);
    }
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(0x5, 0);
            x.set8(0x42, 0);
            x.set8(0x43, 0);
            x.set8(0x2d, 0);
            x.anim_start(ANIMS, 0);
            let kind = host.u8(x.b, 0x2);
            x.set8(f::ATTACK_ID, if kind == 0xc { 0x29 } else { 0x2a });
            x.set8(f::BOX_SET, 7);
            boxes(x, host);
        }
        2 => {
            if x.a5_u8(GAME_PAUSED) == 0 {
                x.set8(0x56, 0);
                x.set8(0x53, 0);
                select_frame(x);
                boxes(x, host);
            }
            let flags = host.u8(x.b, 0x1);
            x.set8(0x1, flags);
            x.reinsert();
        }
        4 => x.add8(0x4, 2),
        _ => x.free_eshl(),
    }
}

fn vanish<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x4, st::FREE);
    x.set8(0x5, 0);
}

/// Collision boxes according to the enemy's attack state: disarmed (0) for some states.
fn boxes<R: Bench>(x: &mut Cx<'_, R>, host: Obj) {
    let row = 4 * x.u8(0x2f) as u32;
    let index = x.rom8(BOXES + 3 + row);
    x.set8(0x3b, index);
    let (kind, detail) = (host.u16(x.b, 0xa0), host.u8(x.b, 0xa1));
    let special = matches!(kind, 0x200 | 0x224 | 0x225 | 0x226) || kind >> 8 == 4;
    let armed = if kind >> 8 == 0 {
        match detail {
            5 | 6 => true,
            7 => !special,
            _ => false,
        }
    } else {
        !special
    };
    for i in 0..3 {
        let v = if armed { x.rom8(BOXES + i + row) } else { 0 };
        x.set8(0x38 + i, v);
    }
}

/// Frame array number `set` of an offset table (words, relative to the table).
fn table<R: Bench>(x: &mut Cx<'_, R>, base: u32, set: u32) -> u32 {
    base.wrapping_add(x.rom16(base + 2 * set) as i16 as i32 as u32)
}

/// Chooses the sprite frame (`+0x34`); `+0x2d` is a counter 1..=0x11.
fn select_frame<R: Bench>(x: &mut Cx<'_, R>) {
    let mut n = x.u8(0x2d) as u16;
    if n >= 0x11 {
        n = 0;
    }
    n += 1;
    x.set8(0x2d, n as u8);
    let host = x.linked(HOST);
    let set = x.u8(0x2f) as u32;
    if host.u8(x.b, 0xb2) != 0 || n & 1 == 0 {
        let frames = table(x, FRAMES_EVEN, set);
        let frame = x.data32(frames + 2 * (n & !1) as u32);
        x.set32(0x34, frame);
    } else if (x.u16(HOST) as i16) < 0 {
        let row = (x.u8(0x42) as u32 >> 1 << 5) + 2 * (x.u8(0x43) as u32 >> 1);
        let flip = x.data8(MIRRORED + row + 1);
        x.set8(f::FLIP, flip);
        // the original clears D0 before this read: the frame always comes from the first row
        let slot = 4 * x.data8(MIRRORED) as u32;
        let frames = table(x, FRAMES_ODD, set);
        let frame = x.data32(frames + slot);
        x.set32(0x34, frame);
    }
}
