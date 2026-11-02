use super::*;

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
    if x.u8(0x40) != 0 || x.a5_u8(0x3d) != 0 {
        return;
    }
    let lock = x.a5_u16(GATE_LOCK);
    if lock != 0 && lock != x.o.0 as u16 {
        return;
    }
    match x.u8(0x4) {
        0 => init(x),
        2 => watch(x),
        4 => arrive(x),
        _ => x.free(&pools::SET),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    for off in [0x42, 0x43, 0x44, 0x45, 0x16, 0x17] {
        x.set8(off, 0);
    }
    x.set_a5_u8(0x50, 0);
    let row = PARAMS + ((x.u8(0x3) as u32) << 4);
    let v = x.rom16(row);
    x.set16(0x8, v);
    let v = x.rom16(row + 2);
    x.set16(0xc, v);
    let v = x.rom16(row + 4);
    x.set16(0x10, v);
    for (off, at) in [(0x48, 6), (0x46, 7), (0x4f, 8), (0x1b, 9)] {
        let v = x.rom8(row + at);
        x.set8(off, v);
    }
}

/// The player is within range of the door.
fn near<R: Bench>(x: &mut Cx<'_, R>, p: Obj) -> bool {
    let dx = p.u16(x.b, pl::X).wrapping_sub(x.u16(0x8)).wrapping_add(NEAR_X);
    if dx > 2 * NEAR_X {
        return false;
    }
    if x.u8(0x1b) != 0 {
        return true;
    }
    let dy = p.u16(x.b, pl::Y).wrapping_sub(x.u16(0x10)).wrapping_add(NEAR_Y);
    dy <= 2 * NEAR_Y
}

fn watch<R: Bench>(x: &mut Cx<'_, R>) {
    if !players().any(|p| p.u8(x.b, pl::ACTIVE) == 1 && near(x, p)) {
        return;
    }
    let v = rd8(x.b, SOUND_FLAG).wrapping_add(1);
    wr8(x.b, SOUND_FLAG, v);
    x.set_a5_u16(GATE_LOCK, 0);
    x.set_a5_u8(0x43, 0xff);
    // exit fade: cedes control, the task resumes at `after_fade`
    use crate::families::set::exit_fading::{fade_to_black, Step};
    fade_to_black(x, Step::Passage)
}

/// After the exit fade (0xd8f06).
pub fn after_fade<R: Bench>(x: &mut Cx<'_, R>) {
    let level = x.u8(0x4f);
    x.set_a5_u8(0x34, level);
    match x.u8(0x48) as i8 {
        1 => exit_zone(x),
        mode if mode > 1 => x.set_a5_u8(0x42, 1),
        _ => {
            x.set_a5_u8(0x4a, 0);
            save_camera(x);
            for pool in [&SET_POOL, &SOBJ_POOL, &EFFCT_POOL, &BOX_POOL, &ITEM_POOL, &BOSS_POOL, &ESHL_POOL] {
                suspend(x, pool);
            }
            x.set16(0x4, 0x400);
            enter_zone(x);
        }
    }
}

fn enter_zone<R: Bench>(x: &mut Cx<'_, R>) {
    crate::families::scenery::resume_scenery(x, crate::families::scenery::Cameras::Start);
    x.set_a5_u8(0x48e, 0xf);
    crate::palette::full_brightness(x.b);
    crate::sound::play(x.b, crate::sound::SCENE_MUSIC, x.o);
    x.set_a5_u16(GATE_LOCK, 0);
    reset_players_after_exit(x);
    crate::families::scenery::place_players(x);
}

fn exit_zone<R: Bench>(x: &mut Cx<'_, R>) {
    release_players(x);
    x.set8(0x4, 6);
    resume_with(x, &SET_POOL, |x, o| {
        if o.u8(x.b, 2) == 0x1d {
            run_on(x, o, run);
        }
    });
    for pool in [&ENEMY_POOL, &EFFCT_POOL, &BOX_POOL, &ITEM_POOL, &BOSS_POOL, &ESHL_POOL, &SOBJ_POOL] {
        resume(x, pool);
    }
}

fn arrive<R: Bench>(x: &mut Cx<'_, R>) {
    crate::families::scenery::resume_scenery(x, crate::families::scenery::Cameras::Return);
    x.set_a5_u8(0x48e, 0xf);
    crate::palette::full_brightness(x.b);
    x.set_a5_u16(GATE_LOCK, 0);
    x.set_a5_u8(0x4a, 1);
    x.set_a5_u8(0x43, 0xf);
    restore_camera(x);
    let music = x.u16(0x1c);
    crate::sound::play_with(x.b, crate::sound::FROM_D4, crate::sound::Source { d4: Some(music as u32), ..Default::default() });
    for p in players() {
        if p.u8(x.b, pl::ACTIVE) != 0 {
            place(x, p);
        }
    }
    x.set8(0x4, 6);
}

fn place<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
    if p.u8(x.b, 4) != 2 {
        let v = p.u8(x.b, 1) | 2;
        p.set_u8(x.b, 1, v);
        crate::placement::place_on_free_ground(x.b, p);
        return;
    }
    p.set_u32(x.b, pl::STATE, 0x0200_2a00);
    let v = p.u8(x.b, 1) & !2;
    p.set_u8(x.b, 1, v);
    p.set_u8(x.b, pl::ACTIVE, 1);
    let slot = 2 * p.u8(x.b, 2) as u32;
    let px = x.rom16(PLACEMENT + slot);
    p.set_u16(x.b, pl::X, px);
    p.set_u16(x.b, pl::Z, 0xa0);
    let y = x.a5_u16(ram::CAMERA_Y).wrapping_add(0xf0);
    p.set_u16(x.b, pl::Y, y);
}
