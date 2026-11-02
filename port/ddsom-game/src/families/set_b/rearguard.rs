use super::*;

const ESHL_TYPE: u8 = 0x12;
const COOLDOWN: u8 = 0x60;
const FIRST_X: u16 = 0x1a0;
const SPACING_X: u16 = 0xc;
const HEIGHT: u16 = 0x20;
/// Depth offsets of the three shots (words).
const OFFSETS: u32 = 0xd80fe;
/// A5+0x53a: cutscene in progress (h).
const CUTSCENE: u32 = 0x53a;
const PLAYING: u8 = 0xc;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            let v = x.a5_u16(ram::CAMERA_X);
            x.set16(0xa, v);
            watch(x);
        }
        2 => watch(x),
        _ => x.free(&pools::SET),
    }
}

fn remember_camera<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.a5_u16(ram::CAMERA_X);
    x.set16(0xa, v);
}

fn watch<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x8) != 0 {
        x.add8(0x8, 0xff);
        return remember_camera(x);
    }
    if x.a5_u8(0x50) != 0 {
        return x.add8(0x4, 2);
    }
    if x.a5_u8(0x2c) != PLAYING || x.a5_u8(0x35) == 0 || x.a5_u8(0x3d) != 0 {
        return remember_camera(x);
    }
    if x.a5_u8(CUTSCENE) != 0 {
        return;
    }
    let camera = x.a5_u16(ram::CAMERA_X) as i16;
    if camera > 0x180 || camera >= x.u16(0xa) as i16 {
        return remember_camera(x);
    }
    // a present player, drawn at random
    let target = loop {
        let rng = x.random();
        let p = Obj(PLAYERS + (rng as u32 & 3) * PLAYER_SIZE);
        if p.u8(x.b, pl::ACTIVE) != 0 {
            break p;
        }
    };
    x.set8(0x8, COOLDOWN);
    let mut shot_x = FIRST_X;
    for i in (0..3u32).rev() {
        let Some(shot) = x.alloc(&pools::ESHL) else { return remember_camera(x) };
        shot.set_u8(x.b, 0, 1);
        shot.set_u8(x.b, 2, ESHL_TYPE);
        shot.set_u8(x.b, 0x2c, 0);
        let v = x.a5_u16(ram::CAMERA_X).wrapping_add(shot_x);
        shot.set_u16(x.b, 0x8, v);
        shot_x += SPACING_X;
        shot.set_u16(x.b, 0xc, HEIGHT);
        let y = target.u16(x.b, pl::Y).wrapping_add(x.rom16(OFFSETS + 2 * i));
        shot.set_u16(x.b, 0x10, y);
    }
    remember_camera(x);
}
