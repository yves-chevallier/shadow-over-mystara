use super::*;
use crate::data::RomAddr;
use crate::ram;
use crate::families::shell::records::view;

const ANIMS: RomAddr = RomAddr(0x16ea18);
const IMPACT_EFFECT: crate::sound::Sound = crate::sound::CMD_34D_44D;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => wait_and_drop(x),
        2 => fall(x),
        4 => x.add8(Obj::STATE, 2),
        _ => x.free_eshl(),
    }
}

fn wait_and_drop<R: Bench>(x: &mut Cx<'_, R>) {
    x.add16(Shot25::TIMER, 0xffff);
    if x.u16(Shot25::TIMER) != 0 {
        return;
    }
    x.add8(Obj::STATE, 2);
    x.set8(Base::BOX_SET, 7);
    x.set8(Shot::ATTACK_ID, 0x17);
    let source = x.linked(Shot25::SOURCE);
    let (sx, sy) = (source.u16(x.b, Obj::X), source.u16(x.b, Obj::Y));
    x.set16(Obj::X, sx);
    x.set16(Obj::Y, sy);
    let camera = if ram::CAMERA_ALT_ACTIVE.get(x.b) != 0 { ram::CAMERA_ALT_Y.get(x.b) } else { ram::CAMERA_Y.get(x.b) };
    x.set16(Obj::Z, camera.wrapping_add(0x100).wrapping_sub(sy));
    x.set16(Obj::VZ, -0x100i16 as u16);
    x.set16(Obj::AZ, -0x40i16 as u16);
    let variant = x.u8(Obj::VARIANT);
    if variant != 0 {
        let rng = x.random();
        let mut dx = rng & 0x7f;
        if variant & 1 != 0 {
            dx = dx.wrapping_neg();
        }
        x.add16(Obj::X, dx);
        let rng = x.random();
        let r = rng & 0x3f;
        let dy = if r & 1 != 0 { 0xffff } else { 0 };
        x.add16(Obj::Y, dy);
    }
    x.anim_start(ANIMS.0, 0);
    if let Some(shadow) = crate::families::small::shot_18(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o, 0) {
        x.set16(Shot25::LINK, low16(shadow));
    }
    x.reinsert();
}

fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    x.move_z();
    if (x.u16(Obj::Z) as i16) >= 0 {
        return;
    }
    x.add8(Obj::STATE, 2);
    x.set32(Obj::Z, 0);
    crate::families::effects_creators::debris(x.b, x.o, 3);
    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 8);
    crate::sound::play(x.b, IMPACT_EFFECT, x.o);
    if x.u16(Shot25::LINK) != 0 {
        let shadow = x.linked(Shot25::LINK);
        shadow.set_u8(x.b, Obj::STATE, 6);
    }
}

view! {
    /// Fields of shot 25 (a spawner, see `Spawner`).
    Shot25 {
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
