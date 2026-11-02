use super::*;

const ANIMS_SCREEN: u32 = 0x1c03a6;
const ANIMS: u32 = 0x17e03a;
/// (destination in the palette, source in the ROM), 8 longs each.
const PALETTES: [(u32, u32); 4] = [(0x91c2e0, 0x2538c0), (0x91c300, 0x2538e0), (0x91c360, 0x253900), (0x91c3e0, 0x253920)];
const TINT: u32 = 0xf000_f000;
const SOURCE_BUSY: u32 = 0xa0;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(0x5c) == 0 {
        return kill(x);
    }
    match x.u8(0x4) {
        0 => init(x),
        2 => active(x),
        _ => x.free_eshl(),
    }
}

fn load_palettes<R: Bench>(x: &mut Cx<'_, R>) {
    for (dst, src) in PALETTES {
        for i in 0..8 {
            let v = x.data32(src + 4 * i);
            wr32(x.b, dst + 4 * i, v);
        }
        for i in 0..8 {
            let v = rd32(x.b, dst + 4 * i) | TINT;
            wr32(x.b, dst + 4 * i, v);
        }
    }
}

/// SET scenery (type 0, variant 0x47); `None` if the pool is full.
fn spawn_scenery<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
    let s = x.alloc(&pools::SET)?;
    s.set_u8(x.b, 0, 1);
    s.set_u8(x.b, 2, 0);
    s.set_u8(x.b, 3, 0x47);
    Some(s)
}

/// Places itself on the creator (position, orientation, one pixel closer).
fn place_on_source<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
    let source = x.linked(0x5c);
    copy_position16(x, source);
    let flip = source.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    x.add16(0x10, 0xffff);
    source
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    let variant = x.u8(0x3);
    if variant == 0 && x.u8(0x5) == 0 {
        x.add8(0x5, 2);
        return load_palettes(x);
    }
    if x.a5_u8(GAME_PAUSED) != 0 {
        return;
    }
    x.add8(0x4, 2);
    x.set8(0x5, 0);
    if variant == 2 {
        place_on_source(x);
        x.add16(0x8, 0x9c);
        x.add16(0x10, -0x62i16 as u16);
        x.anim_start(ANIMS, 8);
        return;
    }
    if let Some(s) = spawn_scenery(x) {
        s.set_u8(x.b, 0x2f, 0);
        x.set16(f::OWNER, low16(s));
    }
    let source = place_on_source(x);
    if variant == 0 {
        if let Some(s) = spawn_scenery(x) {
            x.set8(0x2f, 0);
            s.set_u16(x.b, f::OWNER, low16(x.o));
        }
        let camera_x = x.a5_u16(0x506);
        x.set16(0x8, camera_x);
        let camera_y = x.a5_u16(0x50a);
        x.set16(0xc, camera_y.wrapping_add(0xda));
        x.set16(0x10, 0);
        x.set16(0x42, 0x258);
        x.anim_start(ANIMS_SCREEN, 0xf0);
    } else {
        let dx = if source.u8(x.b, f::FLIP) != 0 { 8 } else { -8i16 as u16 };
        x.add16(0x8, dx);
        x.add16(0xc, 0x20);
        x.set16(0x42, 0x190);
        x.anim_start(ANIMS, 2);
    }
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(GAME_PAUSED) != 0 {
        return;
    }
    let source = x.linked(0x5c);
    if (source.u8(x.b, 0x4) as i8) >= 4 {
        return x.free_eshl();
    }
    match x.u8(0x3) {
        0 => {
            x.add16(0x42, 0xffff);
            if (x.u16(0x42) as i16) < 0 {
                let scenery = x.linked(f::OWNER);
                scenery.set_u8(x.b, 0x4, 4);
                kill(x);
                source.set_u8(x.b, SOURCE_BUSY, 0);
            }
            x.anim_step();
        }
        1 => match x.u8(0x5) {
            0 => {
                x.add16(0x42, 0xffff);
                if (x.u16(0x42) as i16) < 0 {
                    x.add8(0x5, 2);
                    source.set_u8(x.b, SOURCE_BUSY, 0);
                    x.anim_start(ANIMS, 7);
                } else {
                    x.anim_step();
                }
            }
            2 => {
                if x.anim_step() {
                    let scenery = x.linked(f::OWNER);
                    scenery.set_u8(x.b, 0x4, 4);
                    kill(x);
                }
            }
            _ => {}
        },
        _ => {
            x.anim_step();
        }
    }
    x.reinsert();
}
