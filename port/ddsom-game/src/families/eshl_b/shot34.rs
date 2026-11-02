use super::*;
use crate::data::RomAddr;
use crate::families::shell::vars;
use crate::ram;
use crate::families::shell::records::view;
use crate::families::shell::tables::{tables, Read};

const ANIMS_SCREEN: RomAddr = RomAddr(0x1c03a6);
const ANIMS: RomAddr = RomAddr(0x17e03a);
/// (destination in the palette, offset of the source in `shot34_palettes`), 8 longs each.
const PALETTES: [(u32, u32); 4] = [(0x91c2e0, 0), (0x91c300, 0x20), (0x91c360, 0x40), (0x91c3e0, 0x60)];
const TINT: u32 = 0xf000_f000;
const SOURCE_BUSY: u32 = 0xa0;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(Shot34::SOURCE) == 0 {
        return kill(x);
    }
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => active(x),
        _ => x.free_eshl(),
    }
}

fn load_palettes<R: Bench>(x: &mut Cx<'_, R>) {
    for (dst, src) in PALETTES {
        for i in 0..8 {
            let v = tables().shot34_palettes.at32(x.b, src + 4 * i);
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
    s.set_u8(x.b, Obj::ACTIVE, 1);
    s.set_u8(x.b, Obj::KIND, 0);
    s.set_u8(x.b, Obj::VARIANT, 0x47);
    Some(s)
}

/// Places itself on the creator (position, orientation, one pixel closer).
fn place_on_source<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
    let source = x.linked(Shot34::SOURCE);
    copy_position16(x, source);
    let flip = source.u8(x.b, Base::FLIP);
    x.set8(Base::FLIP, flip);
    x.add16(Obj::Y, 0xffff);
    source
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    let variant = x.u8(Obj::VARIANT);
    if variant == 0 && x.u8(Base::SUB_STATE) == 0 {
        x.add8(Base::SUB_STATE, 2);
        return load_palettes(x);
    }
    if vars::GAME_PAUSED.get(x.b) != 0 {
        return;
    }
    x.add8(Obj::STATE, 2);
    x.set8(Base::SUB_STATE, 0);
    if variant == 2 {
        place_on_source(x);
        x.add16(Obj::X, 0x9c);
        x.add16(Obj::Y, -0x62i16 as u16);
        x.anim_start(ANIMS.0, 8);
        return;
    }
    if let Some(s) = spawn_scenery(x) {
        s.set_u8(x.b, Base::SETTING, 0);
        x.set16(Shot::OWNER, low16(s));
    }
    let source = place_on_source(x);
    if variant == 0 {
        if let Some(s) = spawn_scenery(x) {
            x.set8(Base::SETTING, 0);
            s.set_u16(x.b, Shot::OWNER, low16(x.o));
        }
        let camera_x = ram::CAMERA_X.get(x.b);
        x.set16(Obj::X, camera_x);
        let camera_y = ram::CAMERA_Y.get(x.b);
        x.set16(Obj::Z, camera_y.wrapping_add(0xda));
        x.set16(Obj::Y, 0);
        x.set16(Shot34::TIMER, 0x258);
        x.anim_start(ANIMS_SCREEN.0, 0xf0);
    } else {
        let dx = if source.u8(x.b, Base::FLIP) != 0 { 8 } else { -8i16 as u16 };
        x.add16(Obj::X, dx);
        x.add16(Obj::Z, 0x20);
        x.set16(Shot34::TIMER, 0x190);
        x.anim_start(ANIMS.0, 2);
    }
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if vars::GAME_PAUSED.get(x.b) != 0 {
        return;
    }
    let source = x.linked(Shot34::SOURCE);
    if (source.u8(x.b, Obj::STATE) as i8) >= 4 {
        return x.free_eshl();
    }
    match x.u8(Obj::VARIANT) {
        0 => {
            x.add16(Shot34::TIMER, 0xffff);
            if (x.u16(Shot34::TIMER) as i16) < 0 {
                let scenery = x.linked(Shot::OWNER);
                scenery.set_u8(x.b, Obj::STATE, 4);
                kill(x);
                source.set_u8(x.b, SOURCE_BUSY, 0);
            }
            x.anim_step();
        }
        1 => match x.u8(Base::SUB_STATE) {
            0 => {
                x.add16(Shot34::TIMER, 0xffff);
                if (x.u16(Shot34::TIMER) as i16) < 0 {
                    x.add8(Base::SUB_STATE, 2);
                    source.set_u8(x.b, SOURCE_BUSY, 0);
                    x.anim_start(ANIMS.0, 7);
                } else {
                    x.anim_step();
                }
            }
            2 => {
                if x.anim_step() {
                    let scenery = x.linked(Shot::OWNER);
                    scenery.set_u8(x.b, Obj::STATE, 4);
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

view! {
    /// Fields of shot 34.
    Shot34 {
        /// Countdown (h).
        timer, set_timer, TIMER: u16 @ 0x42;
        /// Source object (16-bit pointer).
        source, set_source, SOURCE: u16 @ 0x5c;
    }
}
