use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::vars;
use crate::actors::families::shell::records::{Spawner, view};

const ANIMS: RomAddr = RomAddr(0x16ec24);
const SOUND_END: crate::hw::sound::Sound = crate::hw::sound::CMD_337;
const SOUND_ROW: crate::hw::sound::Sound = crate::hw::sound::CMD_32D;
const ROW_AT: u16 = 0x50;
const RISE: u8 = 0;
const FADE: u8 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => active(x),
        4 => x.add8(Obj::STATE, 2),
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Obj::VARIANT) == 0 {
        x.add8(Obj::STATE, 2);
        let source = x.linked(Shot27::SOURCE);
        let (sx, sy) = (source.u16(x.b, Obj::X), source.u16(x.b, Obj::Y));
        x.set16(Obj::X, sx);
        x.set16(Obj::Y, sy);
        x.set16(Obj::Z, 0x20);
        x.add16(Obj::Y, 0xffff);
        x.set16(Shot27::TIMER, 0x60);
    } else {
        x.add16(Shot27::TIMER, 0xffff);
        if x.u16(Shot27::TIMER) != 0 {
            return;
        }
        x.add8(Obj::STATE, 2);
    }
    x.set8(Base::BOX_SET, 7);
    x.set8(Shot::ATTACK_ID, 0x16);
    let anim = (x.u8(Obj::VARIANT) != 0) as u32;
    x.anim_start(ANIMS.0, anim as u16);
    x.reinsert();
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Obj::VARIANT) == 0 {
        x.add16(Shot27::TIMER, 0xffff);
        if x.u16(Shot27::TIMER) == 0 {
            x.add8(Obj::STATE, 2);
            x.set8(Obj::ACTIVE, 2);
            crate::hw::sound::play(x.b, SOUND_END, x.o);
        }
        x.anim_step();
        if x.u16(Shot27::TIMER) == ROW_AT {
            start_row(x);
            crate::hw::sound::play(x.b, SOUND_ROW, x.o);
        }
        return;
    }
    match x.u8(Base::SUB_STATE) {
        RISE => {
            x.anim_step();
            let source = x.linked(Shot27::SOURCE);
            if source.u16(x.b, Spawner::TIMER) <= 0x30 {
                x.add8(Base::SUB_STATE, 2);
                x.anim_start(ANIMS.0, 2);
            }
            flicker(x);
        }
        FADE => {
            if x.anim_step() {
                x.add8(Obj::STATE, 2);
            }
            flicker(x);
        }
        _ => {}
    }
}

/// Hidden every other frame according to the frame counter `A5+0x6f`.
fn flicker<R: Bench>(x: &mut Cx<'_, R>) {
    let hidden = vars::FRAME_BYTE.get(x.b) & 1 == 0;
    x.set_bit(Obj::FLAGS, 1, hidden);
}

/// Creates the row of elements (variant 1) on either side, from the epicenter's creator
/// or the camera edge, up to the screen limits.
fn start_row<R: Bench>(x: &mut Cx<'_, R>) {
    let px = x.u16(Obj::X);
    let limit = if x.u8(Shot27::COUNT) == 0 {
        let target = x.linked(Shot::OWNER);
        target.u16(x.b, Obj::X)
    } else {
        camera_x(x).wrapping_add(0xc0)
    };
    let rightwards = px <= limit;
    let start = if rightwards { px.wrapping_add(0x20) } else { px.wrapping_sub(0x20) };
    spawn_row(x, 0x1b, start, rightwards);
}

view! {
    /// Fields of shot 27 (a spawner, see `Spawner`).
    Shot27 {
        /// Number of created shots, row of the offsets (h).
        count, set_count, COUNT: u16 @ 0x56;
        /// Countdown of the current step (h).
        timer, set_timer, TIMER: u16 @ 0x58;
        /// Linked object: shadow or second timer (h).
        link, set_link, LINK: u16 @ 0x5a;
        /// Source object (16-bit pointer).
        source, set_source, SOURCE: u16 @ 0x5c;
        /// Row of the height tables (h).
        row, set_row, ROW: u8 @ 0x60;
    }
}
