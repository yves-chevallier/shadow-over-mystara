use super::*;

const STEP: u32 = 0x42;
const BLAST: u32 = 0x46;
/// Sound routines (D0 = number).
/// Offset tables (16 words each) of the explosion in X and in Y.
const BLAST_DX: u32 = 0xd4910;
const BLAST_DY: u32 = 0xd4930;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(BLAST, 0x14);
        }
        2 => step(x),
        4 => x.add8(0x4, 2),
        _ => x.free(&pools::SET),
    }
}

fn camera_x<R: Bench>(x: &mut Cx<'_, R>) -> i16 {
    x.a5_u16(ram::CAMERA_X) as i16
}

/// Goes to the next step and arms the step's countdown.
fn next<R: Bench>(x: &mut Cx<'_, R>, frames: u8) {
    x.set8(STEP, frames);
    x.add8(0x5, 2);
}

fn countdown<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let n = x.u8(STEP).wrapping_sub(1);
    x.set8(STEP, n);
    n == 0
}

fn step<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x5) {
        0 => {
            x.add8(0x5, 2);
            freeze_and_show(x);
        }
        2 => {
            if camera_x(x) >= 0x100 {
                crate::sound::play(x.b, crate::sound::CMD_9A_B, x.o);
                x.add8(0x5, 2);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::A, 0x1e);
            }
        }
        4 => {
            if camera_x(x) >= 0x180 {
                next(x, 0x28);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::A, 0x28);
            }
        }
        6 => {
            if countdown(x) {
                next(x, 0x28);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 0x28);
            }
        }
        8 => {
            if countdown(x) {
                next(x, 0x28);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::A, 0x28);
            }
        }
        10 => {
            if countdown(x) {
                x.add8(0x5, 2);
                crate::sound::play(x.b, crate::sound::CMD_9B_B, x.o);
            }
        }
        12 => {
            if camera_x(x) >= 0x200 {
                next(x, 0x32);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 0x32);
                crate::sound::play(x.b, crate::sound::CMD_9A_B, x.o);
            }
        }
        14 => {
            if countdown(x) {
                eshl_creators::single_25(&mut Pure, x.b, x.o);
                next(x, 0x32);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::C, 0x32);
            }
        }
        16 => explosions(x),
        18 => {
            if countdown(x) {
                x.add8(0x5, 2);
                crate::sound::play(x.b, crate::sound::CMD_9B_B, x.o);
            }
        }
        20 => {
            if camera_x(x) >= 0x2b0 {
                next(x, 0x28);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::C, 0x50);
                crate::sound::play(x.b, crate::sound::CMD_9A_B, x.o);
            }
        }
        22 => spawn_boss(x),
        24 => {
            let n = x.u8(BLAST).wrapping_sub(1);
            x.set8(BLAST, n);
            if n == 0 {
                crate::sound::play(x.b, crate::sound::CMD_9B_B, x.o);
            }
            let n = x.u8(STEP).wrapping_sub(1);
            x.set8(STEP, n);
            if (n as i8) < 0 {
                x.add8(0x4, 2);
            }
        }
        _ => {}
    }
}

/// Step 0: freezes the game (`A5+0x48e` = 0xc, `A5+0x48f` = 7), starts the fade 0x1686 and the fade task 0x14de
/// (0x9b6).
fn freeze_and_show<R: Bench>(x: &mut Cx<'_, R>) {
    show_message(x, 7);
}

/// Step 16: every 20 frames a hit of crate::sound::CMD_351_451; when `+0x42` is a multiple of 4 a random explosion; at 0, end.
fn explosions<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u8(BLAST).wrapping_sub(1);
    x.set8(BLAST, n);
    if (n as i8) < 0 {
        crate::sound::play(x.b, crate::sound::CMD_351_451, x.o);
        x.set8(BLAST, 0x14);
    }
    if x.u8(STEP) & 3 == 0 {
        eshl_creators::single_25(&mut Pure, x.b, x.o);
        random_blast(x);
    }
    if countdown(x) {
        x.add8(0x5, 2);
        crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 0x32);
    }
}

fn random_blast<R: Bench>(x: &mut Cx<'_, R>) {
    let rng = x.random();
    let i = (rng as u32 & 0xf) << 1;
    let dx = x.rom16(BLAST_DX + i);
    let rng = x.random();
    let i = (rng as u32 & 0xf) << 1;
    let dy = x.rom16(BLAST_DY + i);
    let px = dx.wrapping_add(x.a5_u16(ram::CAMERA_X));
    let py = dy.wrapping_add(x.a5_u16(ram::CAMERA_Y));
    crate::families::enemy_common::enemy_blast_at(x, px, py);
}

/// Step 22: 30 smokes, then the boss (BOSS type 2) appears.
fn spawn_boss<R: Bench>(x: &mut Cx<'_, R>) {
    if !countdown(x) {
        return;
    }
    // 0xb4bcc thirty times: the variant is the loop counter (D1, 0x1d down to 0), the setting zero
    for n in (0..=0x1d).rev() {
        crate::families::effects_creators::debris_rain(x.b, x.o, n, 0);
    }
    crate::sound::play(x.b, crate::sound::CMD_9C, x.o);
    x.set_a5_u8(0x48e, 0xf);
    crate::palette::full_brightness(x.b);
    let Some(boss) = x.alloc(&pools::BOSS) else { return };
    boss.set_u8(x.b, 0, 1);
    boss.set_u8(x.b, 2, 2);
    x.add8(0x5, 2);
    x.set8(STEP, 0x3c);
    x.set8(BLAST, 0x28);
}
