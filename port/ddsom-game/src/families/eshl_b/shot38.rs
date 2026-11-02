use super::*;
use crate::families::shell::tables::{tables, Read};

const ANIMS: u32 = 0x174776;
/// Rows of `shot38_speed_rows`: variant 0, other variants.
const SPEED_ROWS: u32 = 0;
const SPEED_ROWS_VARIANT: u32 = 0x60;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    state(x)
}

/// Mirror of an angle (0x20 - angle) for flipped objects.
fn mirror(a: u16) -> u16 {
    a.wrapping_sub(0x10).wrapping_neg().wrapping_add(0x10)
}

fn state<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => init(x),
        2 => {
            fall(x);
            x.keep_on_screen(0x10);
            x.push_terrain();
            x.reinsert();
        }
        4 => x.add8(0x4, 2),
        _ => x.free_eshl(),
    }
}

/// Movement while Z > 0; on the ground, goes to the next state.
fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x29) == 0 {
        x.move_by_table_xz();
        x.move_y();
        if (x.u16(0xc) as i16) >= 0 {
            return;
        }
        x.set16(0xc, 0);
    }
    x.add8(0x4, 2);
    x.set8(0x5, 0);
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    let creator = x.u16(0x46);
    if (creator as i16) >= 0 {
        return x.set8(0x4, st::FREE);
    }
    let host = Obj(creator as i16 as i32 as u32 & 0xff_ffff);
    copy_position16(x, host);
    let flip = host.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    x.add16(0xc, 0x30);
    speeds(x);
    let mut angle = host.u16(x.b, 0x50);
    if (angle as i16) < 0 {
        let target = Obj(angle as i16 as i32 as u32 & 0xff_ffff);
        let to = (target.u16(x.b, 0x8), target.u16(x.b, 0xc).wrapping_add(0x30));
        let de = (x.u16(0x8), x.u16(0xc));
        angle = crate::aids::direction(x.b, de, to) as u16;
    }
    let flipped = flip != 0;
    let mapped = |a: u16| if flipped { mirror(a) } else { a };
    let seen = mapped(angle) as i16;
    let chosen = if seen < 0x14 {
        mapped(0x14)
    } else if seen > 0x16 {
        mapped(0x16)
    } else {
        angle
    };
    x.set8(0x2d, chosen as u8);
    let sprite = tables().shot38_sprites.at8(x.b, ((mapped(chosen) >> 1) & 0xf) as u32) as u32;
    ai_start(x, ANIMS, sprite);
    x.set8(f::BOX_SET, 7);
    x.set8(f::ATTACK_POWER, 8);
}

/// Speed table and initial speeds of row `+0x2f`.
fn speeds<R: Bench>(x: &mut Cx<'_, R>) {
    let base = if x.u8(0x3) != 0 { SPEED_ROWS_VARIANT } else { SPEED_ROWS };
    let row = base + ((x.u8(0x2f) as u32) << 4);
    let t = &tables().shot38_speed_rows;
    let table = t.at32(x.b, row);
    x.set32(0x4c, table);
    let mut vx = t.at16(x.b, row + 4);
    let mut ax = t.at16(x.b, row + 6);
    if x.u8(f::FLIP) == 0 {
        vx = vx.wrapping_neg();
        ax = ax.wrapping_neg();
    }
    x.set16(0x14, vx);
    x.set16(0x16, ax);
    for i in 0..4 {
        let v = t.at16(x.b, row + 8 + 2 * i);
        x.set16(0x18 + 2 * i, v);
    }
}
