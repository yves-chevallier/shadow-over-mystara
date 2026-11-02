use super::*;
use crate::families::set::tables as set_tables;
use crate::families::set::vars;
use crate::objects::Obj;
use crate::player::Player;
use crate::families::set_b::fields::DoorObj;

const GATE_LOCK: u32 = 0x52;
const PARAMS: u32 = 0xd8e5e;
const PLACEMENT: u32 = 0xd9052;
const SOUND_FLAG: u32 = 0xff_24e0;
const NEAR_X: u16 = 0x30;
const NEAR_Y: u16 = 0x20;

/// Stays on the processor: the exit fade (`watch`) cedes control (`set::exit_fading`).
pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let me = x.o;
    crate::effects_aids::living_players(x.b, me);
    if x.u8(Obj::HIDDEN) != 0 || x.a5_u8(vars::PAUSED) != 0 {
        return;
    }
    let lock = x.a5_u16(GATE_LOCK);
    if lock != 0 && lock != x.o.0 as u16 {
        return;
    }
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => watch(x),
        4 => arrive(x),
        _ => x.free(&pools::SET),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    for off in [0x42, 0x43, 0x44, 0x45, 0x16, 0x17] {
        x.set8(off, 0);
    }
    x.set_a5_u8(vars::EXIT_LOCK, 0);
    let row = PARAMS + ((x.u8(Obj::VARIANT) as u32) << 4);
    let v = set_tables::pc_word(x.b, row);
    x.set16(Obj::X, v);
    let v = set_tables::pc_word(x.b, row + 2);
    x.set16(Obj::Z, v);
    let v = set_tables::pc_word(x.b, row + 4);
    x.set16(Obj::Y, v);
    for (off, at) in [(0x48, 6), (0x46, 7), (0x4f, 8), (0x1b, 9)] {
        let v = set_tables::pc_byte(x.b, row + at);
        x.set8(off, v);
    }
}

/// The player is within range of the door.
fn near<R: Bench>(x: &mut Cx<'_, R>, p: Obj) -> bool {
    let dx = p.u16(x.b, pl::X).wrapping_sub(x.u16(Obj::X)).wrapping_add(NEAR_X);
    if dx > 2 * NEAR_X {
        return false;
    }
    if x.u8(DoorObj::IGNORE_DEPTH) != 0 {
        return true;
    }
    let dy = p.u16(x.b, pl::Y).wrapping_sub(x.u16(Obj::Y)).wrapping_add(NEAR_Y);
    dy <= 2 * NEAR_Y
}

fn watch<R: Bench>(x: &mut Cx<'_, R>) {
    if !players().any(|p| p.u8(x.b, pl::ACTIVE) == 1 && near(x, p)) {
        return;
    }
    let v = rd8(x.b, SOUND_FLAG).wrapping_add(1);
    wr8(x.b, SOUND_FLAG, v);
    x.set_a5_u16(GATE_LOCK, 0);
    x.set_a5_u8(vars::INPUT_MASK, 0xff);
    // exit fade: cedes control, the task resumes at `after_fade`
    use crate::families::set::exit_fading::{fade_to_black, Step};
    fade_to_black(x, Step::Passage)
}

/// After the exit fade (0xd8f06).
pub fn after_fade<R: Bench>(x: &mut Cx<'_, R>) {
    let level = x.u8(DoorObj::NEXT_SCENE);
    x.set_a5_u8(vars::SCENE, level);
    match x.u8(DoorObj::MODE) as i8 {
        1 => exit_zone(x),
        mode if mode > 1 => x.set_a5_u8(vars::FROZEN, 1),
        _ => {
            x.set_a5_u8(vars::PAUSE, 0);
            save_camera(x);
            for pool in [&pools::SET, &pools::SOBJ, &pools::EFFCT, &pools::BOX, &pools::ITEM, &pools::BOSS, &pools::ESHL] {
                suspend(x, pool);
            }
            x.set16(Obj::STATE, 0x400);
            enter_zone(x);
        }
    }
}

fn enter_zone<R: Bench>(x: &mut Cx<'_, R>) {
    crate::families::scenery::resume_scenery(x, crate::families::scenery::Cameras::Start);
    x.set_a5_u8(vars::FADE_PAGES, 0xf);
    crate::palette::full_brightness(x.b);
    crate::sound::play(x.b, crate::sound::SCENE_MUSIC, x.o);
    x.set_a5_u16(GATE_LOCK, 0);
    reset_players_after_exit(x);
    crate::families::scenery::place_players(x);
}

fn exit_zone<R: Bench>(x: &mut Cx<'_, R>) {
    release_players(x);
    x.set8(Obj::STATE, 6);
    resume_with(x, &pools::SET, |x, o| {
        if o.u8(x.b, Obj::KIND) == 0x1d {
            run_on(x, o, run);
        }
    });
    for pool in [&pools::ENEMY, &pools::EFFCT, &pools::BOX, &pools::ITEM, &pools::BOSS, &pools::ESHL, &pools::SOBJ] {
        resume(x, pool);
    }
}

fn arrive<R: Bench>(x: &mut Cx<'_, R>) {
    crate::families::scenery::resume_scenery(x, crate::families::scenery::Cameras::Return);
    x.set_a5_u8(vars::FADE_PAGES, 0xf);
    crate::palette::full_brightness(x.b);
    x.set_a5_u16(GATE_LOCK, 0);
    x.set_a5_u8(vars::PAUSE, 1);
    x.set_a5_u8(vars::INPUT_MASK, 0xf);
    restore_camera(x);
    let music = x.u16(DoorObj::MUSIC);
    crate::sound::play_with(x.b, crate::sound::FROM_D4, crate::sound::Source { d4: Some(music as u32), ..Default::default() });
    for p in players() {
        if p.u8(x.b, pl::ACTIVE) != 0 {
            place(x, p);
        }
    }
    x.set8(Obj::STATE, 6);
}

fn place<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
    if p.u8(x.b, Player::MAIN) != 2 {
        let v = p.u8(x.b, Player::FLAGS) | 2;
        p.set_u8(x.b, Player::FLAGS, v);
        crate::placement::place_on_free_ground(x.b, p);
        return;
    }
    p.set_u32(x.b, pl::STATE, 0x0200_2a00);
    let v = p.u8(x.b, Player::FLAGS) & !2;
    p.set_u8(x.b, Player::FLAGS, v);
    p.set_u8(x.b, pl::ACTIVE, 1);
    let slot = 2 * p.u8(x.b, Player::INDEX) as u32;
    let px = set_tables::pc_word(x.b, PLACEMENT + slot);
    p.set_u16(x.b, pl::X, px);
    p.set_u16(x.b, pl::Z, 0xa0);
    let y = x.a5_u16(ram::CAMERA_Y).wrapping_add(0xf0);
    p.set_u16(x.b, pl::Y, y);
}
