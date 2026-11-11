use super::*;
use crate::actors::families::set::tables as set_tables;
use crate::actors::families::set::vars;
use crate::world::objects::Obj;
use crate::actors::families::set_b::fields::RearguardObj;
use crate::actors::families::set::fields::SetObj;

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
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            let v = x.a5_u16(ram::CAMERA_X);
            x.set16(RearguardObj::START_X, v);
            watch(x);
        }
        2 => watch(x),
        _ => x.free(&pools::SET),
    }
}

fn remember_camera<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.a5_u16(ram::CAMERA_X);
    x.set16(RearguardObj::START_X, v);
}

fn watch<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Obj::X) != 0 {
        x.add8(Obj::X, 0xff);
        return remember_camera(x);
    }
    if x.a5_u8(vars::EXIT_LOCK) != 0 {
        return x.add8(Obj::STATE, 2);
    }
    if x.a5_u8(vars::PHASE) != PLAYING || x.a5_u8(vars::PLAYERS_ALIVE) == 0 || x.a5_u8(vars::PAUSED) != 0 {
        return remember_camera(x);
    }
    if x.a5_u8(CUTSCENE) != 0 {
        return;
    }
    let camera = x.a5_u16(ram::CAMERA_X) as i16;
    if camera > 0x180 || camera >= x.u16(RearguardObj::START_X) as i16 {
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
    x.set8(Obj::X, COOLDOWN);
    let mut shot_x = FIRST_X;
    for i in (0..3u32).rev() {
        let Some(shot) = x.alloc(&pools::ESHL) else { return remember_camera(x) };
        shot.set_u8(x.b, Obj::ACTIVE, 1);
        shot.set_u8(x.b, Obj::KIND, ESHL_TYPE);
        shot.set_u8(x.b, SetObj::ATTR, 0);
        let v = x.a5_u16(ram::CAMERA_X).wrapping_add(shot_x);
        shot.set_u16(x.b, Obj::X, v);
        shot_x += SPACING_X;
        shot.set_u16(x.b, Obj::Z, HEIGHT);
        let y = target.u16(x.b, pl::Y).wrapping_add(set_tables::pc_word(x.b, OFFSETS + 2 * i));
        shot.set_u16(x.b, Obj::Y, y);
    }
    remember_camera(x);
}
