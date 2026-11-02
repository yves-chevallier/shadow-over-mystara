use super::*;

const ANIMS: u32 = 0x16e146;
const HELD: u8 = 0;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(0x58, 4);
            let anim = (x.u8(0x43) != 0) as u32;
            x.anim_start(ANIMS, anim as u16);
            reinsert_foreground(x);
        }
        2 => active(x),
        4 => dropped(x),
        6 => x.free_eshl(),
        _ => {}
    }
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x5) {
        HELD => {
            let owner = x.linked(f::OWNER);
            if (owner.u8(x.b, 0x4) as i8) >= 4 {
                x.set8(0x0, 2);
            }
            if x.u8(0x5a) == 0 {
                x.anim_step();
                copy_position32(x, owner);
                let dx = if x.u8(f::FLIP) != 0 { -0x2ai16 } else { 0x2a };
                x.add16(0x8, dx as u16);
                x.add16(0xc, -0x78i16 as u16);
                x.add32(0x10, 1);
                x.add32(0xc, 0xffff_ffff);
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
    x.add8(0x5, 2);
    speed_table(x, 0x18);
    let v = x.u32(0x1c);
    x.set32(0x18, v);
    let angle = if x.u8(f::FLIP) != 0 { 0xa } else { 0x16 };
    x.set8(0x2d, angle);
    x.set8(0x55, 2);
    reinsert_foreground(x);
}

fn flying<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_by_table_xz();
    x.anim_step();
    if x.u8(0x2d) != 0 {
        x.add8(0x55, 0xff);
        if (x.u8(0x55) as i8) < 0 {
            x.set8(0x55, 0x18);
            let step: u8 = if x.u8(f::FLIP) != 0 { 0xff } else { 1 };
            let v = x.u8(0x2d).wrapping_add(step) & 0x1f;
            x.set8(0x2d, v);
        }
    }
    if (x.u16(0xc) as i16) > 0x100 {
        x.add8(0x4, 4);
    }
    reinsert_foreground(x);
}

/// State 4: as long as the owner has HP, nothing; otherwise the object is thrown (or released).
fn dropped<R: Bench>(x: &mut Cx<'_, R>) {
    let owner = x.linked(f::OWNER);
    if (owner.u16(x.b, 0x62) as i16) <= 0 {
        x.set8(0x0, 2);
        x.set32(0x4, 0x0200_0000);
        return launch(x);
    }
    x.set32(0x4, 0x0200_0000);
    if x.u8(0x5a) != 0 {
        x.set8(0x0, 2);
        x.set32(0x58, 0);
        return;
    }
    x.set32(0x58, 0);
    owner.set_u32(x.b, 0x4, 0x0402_0000);
    if (x.u16(0x62) as i16) > 0 {
        x.add8(0x5, 2);
    }
}
