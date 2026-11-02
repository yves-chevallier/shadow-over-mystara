use super::*;
use crate::data::RomAddr;
use crate::families::shell::records::{Host, view};

const ANIMS: RomAddr = RomAddr(0x16e146);
const HELD: u8 = 0;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Shot16::TIMER, 4);
            let anim = (x.u8(Shot16::MODE) != 0) as u32;
            x.anim_start(ANIMS.0, anim as u16);
            reinsert_foreground(x);
        }
        2 => active(x),
        4 => dropped(x),
        6 => x.free_eshl(),
        _ => {}
    }
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Base::SUB_STATE) {
        HELD => {
            let owner = x.linked(Shot::OWNER);
            if (owner.u8(x.b, Obj::STATE) as i8) >= 4 {
                x.set8(Obj::ACTIVE, 2);
            }
            if x.u8(Shot16::STOPPED) == 0 {
                x.anim_step();
                copy_position32(x, owner);
                let dx = if x.u8(Base::FLIP) != 0 { -0x2ai16 } else { 0x2a };
                x.add16(Obj::X, dx as u16);
                x.add16(Obj::Z, -0x78i16 as u16);
                x.add32(Obj::Y, 1);
                x.add32(Obj::Z, 0xffff_ffff);
                reinsert_foreground(x);
            } else {
                launch(x);
            }
        }
        _ => flying(x),
    }
}

/// Goes to the throw: speed table 0x18, the vertical speed becomes that of Y.
fn launch<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Base::SUB_STATE, 2);
    speed_table(x, 0x18);
    let v = x.u32(Obj::VY);
    x.set32(Obj::VZ, v);
    let angle = if x.u8(Base::FLIP) != 0 { 0xa } else { 0x16 };
    x.set8(Obj::SPEED_INDEX, angle);
    x.set8(Shot16::BLINK, 2);
    reinsert_foreground(x);
}

fn flying<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_by_table_xz();
    x.anim_step();
    if x.u8(Obj::SPEED_INDEX) != 0 {
        x.add8(Shot16::BLINK, 0xff);
        if (x.u8(Shot16::BLINK) as i8) < 0 {
            x.set8(Shot16::BLINK, 0x18);
            let step: u8 = if x.u8(Base::FLIP) != 0 { 0xff } else { 1 };
            let v = x.u8(Obj::SPEED_INDEX).wrapping_add(step) & 0x1f;
            x.set8(Obj::SPEED_INDEX, v);
        }
    }
    if (x.u16(Obj::Z) as i16) > 0x100 {
        x.add8(Obj::STATE, 4);
    }
    reinsert_foreground(x);
}

/// State 4: as long as the owner has HP, nothing; otherwise the object is thrown (or released).
fn dropped<R: Bench>(x: &mut Cx<'_, R>) {
    let owner = x.linked(Shot::OWNER);
    if (owner.u16(x.b, Host::LIFE) as i16) <= 0 {
        x.set8(Obj::ACTIVE, 2);
        x.set32(Obj::STATE, 0x0200_0000);
        return launch(x);
    }
    x.set32(Obj::STATE, 0x0200_0000);
    if x.u8(Shot16::STOPPED) != 0 {
        x.set8(Obj::ACTIVE, 2);
        x.set32(Shot16::TIMER_LONG, 0);
        return;
    }
    x.set32(Shot16::TIMER_LONG, 0);
    owner.set_u32(x.b, Obj::STATE, 0x0402_0000);
    if (x.u16(Shot16::LIFE) as i16) > 0 {
        x.add8(Base::SUB_STATE, 2);
    }
}

view! {
    /// Fields of shot 16.
    Shot16 {
        /// Animation mode (h).
        mode, set_mode, MODE: u8 @ 0x43;
        /// Blink countdown (h).
        blink, set_blink, BLINK: u8 @ 0x55;
        /// Countdown (h).
        timer, set_timer, TIMER: u8 @ 0x58;
        /// The countdown and its flags as a long (h).
        timer_long, set_timer_long, TIMER_LONG: u32 @ 0x58;
        /// Non-zero: stopped (h).
        stopped, set_stopped, STOPPED: u8 @ 0x5a;
        /// Hit points (h).
        life, set_life, LIFE: u16 @ 0x62;
    }
}
