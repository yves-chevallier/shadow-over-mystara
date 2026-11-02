use super::*;

const ANIMS: u32 = 0x1713f8;
const CREATOR: u32 = 0x5c;
const GENERATION: u32 = 0x2f;
const TIMER: u32 = 0x42;
const SOUND_COUNT: u32 = 0x46;
const LAST_GENERATION: u8 = 4;
const SFX_FLAME: crate::sound::Sound = crate::sound::CMD_330;
const SFX_LAND: crate::sound::Sound = crate::sound::CMD_32F;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            if x.u16(CREATOR) != 0 {
                init(x);
            }
        }
        2 => {
            match x.u8(0x3) {
                0 => active_0(x),
                1 => column(x, 1),
                _ => column(x, 2),
            }
            x.reinsert();
        }
        4 => x.add8(0x4, 2),
        _ => x.free_eshl(),
    }
}

fn shape<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(f::BOX_SET, 7);
    x.set8(f::ATTACK_ID, 0x26);
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x3) == 0 {
        let g = x.u8(GENERATION).wrapping_sub(1);
        x.set8(GENERATION, g);
        if g == 0 {
            x.set8(0x4, 4);
            return;
        }
        x.add8(0x4, 2);
        if g == LAST_GENERATION {
            x.set8(SOUND_COUNT, 3);
            shape(x);
            follow(x);
            x.set16(0x18, 0xfb40);
            x.anim_start(ANIMS, 0);
        } else {
            follow(x);
            let dx = if x.u8(f::FLIP) != 0 { 0x20 } else { 0xffe0 };
            x.add16(0x8, dx);
            x.set16(TIMER, 12);
            shape(x);
            x.anim_start(ANIMS, 2);
        }
        return;
    }
    x.add8(0x4, 2);
    let n = x.u8(SOUND_COUNT).wrapping_sub(1);
    x.set8(SOUND_COUNT, n);
    if n == 0 {
        crate::sound::play(x.b, SFX_FLAME, x.o);
        x.set8(SOUND_COUNT, 3);
    }
    follow(x);
    let dz = if x.u8(0x3) == 1 { 8 } else { 0xfff8 };
    x.add16(0x10, dz);
    let z = x.u16(0x10) as i16;
    if z <= 0x170 || z > 0x1af {
        x.set8(0x4, 4);
    }
    x.set16(TIMER, 12);
    shape(x);
    x.anim_start(ANIMS, 2);
}

/// Position (longs) and orientation of the creator, one pixel in front of it.
fn follow<R: Bench>(x: &mut Cx<'_, R>) {
    let creator = x.linked(CREATOR);
    for off in [0x8, 0x10, 0xc] {
        let v = creator.u32(x.b, off);
        x.set32(off, v);
    }
    let flip = creator.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    x.add16(0x10, 0xffff);
}

fn active_0<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(GENERATION) != LAST_GENERATION {
        if count_down(x, TIMER) {
            spawn_all(x);
        }
        finish(x);
        return;
    }
    match x.u8(0x5) {
        0 => {
            if x.anim_step() {
                x.add8(0x5, 2);
                shape(x);
                crate::sound::play(x.b, SFX_LAND, x.o);
                x.anim_start(ANIMS, 1);
            }
        }
        2 => {
            if x.move_xyz().gt() {
                x.anim_step();
                return;
            }
            x.set16(0xc, 0);
            crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 8);
            x.add8(0x5, 2);
            shape(x);
            x.set16(TIMER, 12);
            x.anim_start(ANIMS, 2);
            crate::sound::play(x.b, SFX_FLAME, x.o);
        }
        _ => {
            if count_down(x, TIMER) {
                spawn_all(x);
            }
            finish(x);
        }
    }
}

/// Column: at the end of its time, creates the next flame of its variant.
fn column<R: Bench>(x: &mut Cx<'_, R>, variant: u8) {
    if count_down(x, TIMER) {
        spawn(x, variant);
    }
    finish(x);
}

fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        x.add8(0x4, 2);
    }
}

fn spawn_all<R: Bench>(x: &mut Cx<'_, R>) {
    spawn(x, 0);
    spawn(x, 1);
    spawn(x, 2);
}

/// Child flame of variant `variant` (0xa6e8e, 0xa6eb2, 0xa6ed6): variant 0 inherits the generation, the columns the
/// sound counter.
fn spawn<R: Bench>(x: &mut Cx<'_, R>, variant: u8) {
    let Some(child) = x.alloc(&pools::ESHL) else { return };
    child.set_u8(x.b, 0x0, 1);
    child.set_u8(x.b, 0x2, 64);
    child.set_u8(x.b, 0x3, variant);
    if variant == 0 {
        let g = x.u8(GENERATION);
        child.set_u8(x.b, GENERATION, g);
        child.set_u16(x.b, CREATOR, x.o.0 as u16);
    } else {
        child.set_u16(x.b, CREATOR, x.o.0 as u16);
        let n = x.u8(SOUND_COUNT);
        child.set_u8(x.b, SOUND_COUNT, n);
    }
}
