use super::*;
use crate::families::shell::tables::{tables, Read};

const FADE: RomAddr = RomAddr(0x291f00);
const FADE_OUT_FINAL: RomAddr = RomAddr(0x292120);
const PALETTE_DST: u32 = 0x91c1e0;
const DURATION: u16 = 0xb4;
const STEP_FRAMES: u16 = 4;
const STEPS: u16 = 0x10;
const STAGE_7: u8 = 7;
const STAGE_2B: u8 = 0x2b;
/// Direct animation scripts of level 0x2b (normal, with `A5+0x70`).
const SCRIPT_2B: u32 = 0xe0152;
const SCRIPT_2B_ALT: u32 = 0xe01cc;
/// `A5+0x70`: two-player game (h), shifts the title.
const ALT: u32 = 0x70;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(0x48f) != 0 {
        return;
    }
    match x.u8(0x4) {
        0 => start(x),
        2 => step(x),
        4 => {
            if x.a5_u8(0x34) == STAGE_2B {
                x.set_a5_u32(0x5c6, 0);
            }
            x.free(&pools::SOBJ)
        }
        _ => x.free(&pools::SOBJ),
    }
}

fn fade_to<R: Bench>(x: &mut Cx<'_, R>, step: u16) {
    let from = FADE.offset(((step as u32) << 5 & 0xffff) as i32);
    copy_longs(x, from, PALETTE_DST, 8);
}

fn start<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(0x30) == 0 {
        return x.free(&pools::SOBJ);
    }
    let stage = x.a5_u8(0x34);
    if stage == STAGE_2B {
        return start_2b(x);
    }
    if x.a5_u8(0x4e) != 0 && stage == 0x1c {
        return x.free(&pools::SOBJ);
    }
    x.add8(0x4, 2);
    x.set16(0x14, DURATION);
    x.set16(0x18, STEP_FRAMES);
    x.set16(0x1c, STEPS);
    fade_to(x, STEPS);
    let mut anim = tables().sobj_stage_title_anims_by_stage.at16(x.b, 2 * stage as u32);
    if x.a5_u16(ALT) != 0 {
        anim = anim.wrapping_add(0x10d);
    }
    x.anim_start(ANIMS, (anim as u32) as u16);
    place(x);
}

fn start_2b<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set16(0x14, DURATION);
    x.set16(0x18, STEP_FRAMES);
    x.set16(0x1c, STEPS);
    fade_to(x, STEPS);
    let at = if x.a5_u16(ALT) != 0 { SCRIPT_2B_ALT } else { SCRIPT_2B };
    x.anim_start_entry(at);
    place(x);
}

/// Title at (camera X + 0x44, camera Z + 0xa8); a bit further left in two-player mode.
fn place<R: Bench>(x: &mut Cx<'_, R>) {
    let camera = Obj(0xff_0000 | camera_record(x) as u32);
    let v = camera.u16(x.b, 0x8);
    x.set16(0x8, v);
    let v = camera.u16(x.b, 0xc);
    x.set16(0xc, v);
    x.set16(0x10, 0);
    x.add16(0x8, 0x44);
    if x.a5_u16(ALT) != 0 {
        x.add16(0x8, 0u16.wrapping_sub(0xc));
    }
    x.add16(0xc, 0xa8);
}

fn step<R: Bench>(x: &mut Cx<'_, R>) {
    let stage = x.a5_u8(0x34);
    if stage == STAGE_7 && x.u8(0x5) == 0 {
        return step_stage_7(x);
    }
    if stage == STAGE_2B {
        return step_stage_2b(x);
    }
    place(x);
    x.anim_step();
    if tick_fade(x) {
        return finish_tick(x);
    }
    finish_tick(x)
}

/// One fade step: every 4 frames, next palette; at stage 0 the counter stays
/// stuck at 0xffff.
fn tick_fade<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let n = x.u16(0x18).wrapping_sub(1);
    x.set16(0x18, n);
    if n != 0 {
        return false;
    }
    let s = x.u16(0x1c).wrapping_sub(1);
    x.set16(0x1c, s);
    if s != 0 {
        x.set16(0x18, STEP_FRAMES);
    } else {
        x.set16(0x18, 0xffff);
    }
    let s = x.u16(0x1c);
    fade_to(x, s);
    true
}

/// Countdown of the title's duration; at zero it goes to state 4. Put back in the list.
fn finish_tick<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u16(0x14).wrapping_sub(1);
    x.set16(0x14, n);
    if n == 0 {
        x.add8(0x4, 2);
    }
    x.reinsert();
}

/// Level 0x2b: like the normal fade but without advancing the animation.
fn step_stage_2b<R: Bench>(x: &mut Cx<'_, R>) {
    tick_fade(x);
    place(x);
    finish_tick(x);
}

/// Level 7, substate 0: the fade lasts until the negative stage, where the final palette
/// (0x292120) is set and the substate goes to 2.
fn step_stage_7<R: Bench>(x: &mut Cx<'_, R>) {
    place(x);
    let n = x.u16(0x18).wrapping_sub(1);
    x.set16(0x18, n);
    if n == 0 {
        let s = x.u16(0x1c).wrapping_sub(1);
        x.set16(0x1c, s);
        if (s as i16) < 0 {
            x.set16(0x18, 0xffff);
            copy_longs(x, FADE_OUT_FINAL, PALETTE_DST, 8);
            x.add8(0x5, 2);
            return x.reinsert();
        }
        x.set16(0x18, STEP_FRAMES);
        fade_to(x, s);
    }
    finish_tick(x);
}
