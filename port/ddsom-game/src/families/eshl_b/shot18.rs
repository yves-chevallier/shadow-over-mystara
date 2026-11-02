use super::*;

const ANIMS: u32 = 0x16e534;
/// Speed table number (byte) according to A5+0x7f92.
const SPEED_TABLES: u32 = 0xa0a60;
const LEVEL_VARIABLE: u32 = 0x7f92;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_POWER, 6);
            x.set8(f::ATTACK_ID, 0x1a);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(0x2d, angle);
            let level = x.a5_u16(LEVEL_VARIABLE);
            let table = x.rom8(SPEED_TABLES.wrapping_add(level as i16 as i32 as u32));
            speed_table(x, level as u32 & 0xff00 | table as u32);
            x.anim_start(ANIMS, 7);
        }
        2 => fly(x),
        _ => x.free_eshl(),
    }
}

fn fly<R: Bench>(x: &mut Cx<'_, R>) {
    let hit = x.u8(f::HIT_FLAGS);
    if hit & 0x08 != 0 {
        crate::sound::play(x.b, crate::sound::CMD_122_222, x.o);
        return x.free_eshl();
    }
    if hit & 0x20 != 0 {
        x.add8(0x4, 2);
        return x.free_eshl();
    }
    if blocked_by_terrain(x) {
        return x.free_eshl();
    }
    x.move_by_table_xy();
    x.anim_step();
    x.cull_offscreen();
    x.reinsert();
}
