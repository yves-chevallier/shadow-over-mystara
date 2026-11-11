//! Camera 2: scenery of scroll layer 3, which follows the scroll of camera 1 (per-scene dispatch 0x332c0, modes).

use super::*;

/// Modes of camera 2 (`Camera::MODE`).
pub(super) mod mode2 {
    pub const STAGE: u16 = 0x00;
    pub const FREE: u16 = 0x04;
    pub const HALF: u16 = 0x14;
    pub const FULL: u16 = 0x18;
    pub const DRIFT: u16 = 0x1c;
    pub const QUARTER_X: u16 = 0x2c;
    pub const RUBBLE: u16 = 0x34;
    pub const QUARTER_THREE: u16 = 0x38;
    pub const FOLLOWED: u16 = 0x40;
    pub const QUARTER_BOTH: u16 = 0x44;
    pub const CREEP_6: u16 = 0x48;
    pub const HALF_SYNC: u16 = 0x4c;
    pub const HALF_X: u16 = 0x50;
    pub const SCREEN_X: u16 = 0x60;
    pub const WAIT_THEN_CREEP: u16 = 0x64;
    pub const CREEP_3: u16 = 0x68;
    pub const FROM_CAMERA_1: u16 = 0x74;
    pub const DRIVES_CAMERA_1: u16 = 0x78;
}

/// Scroll of camera 1 since the previous frame on axis 0 (x) or 1 (y).
pub(super) fn delta(x: &mut Cam, axis: usize) -> i32 {
    crate::scroll::scroll_delta(x.b, axis)
}

/// Camera 2: advances by `dx`/`dy` (positions in 16.16).
pub(super) fn drift(x: &mut Cam, dx: i32, dy: i32) {
    x.add32(Camera::X_LONG, dx as u32);
    x.add32(Camera::Y_LONG, dy as u32);
}

/// Per-scene dispatch of camera 2 (0x332c0).
pub(super) fn stage_script_2(x: &mut Cam) {
    let stage = ram::SCENE.get(x.b);
    match stage {
        2 | 20 | 90 | 102 => x.set16(Camera::MODE, mode2::DRIVES_CAMERA_1),
        3 | 7 | 9 | 21..=23 | 26 | 36 | 37 | 47 | 50 | 61 | 62 | 64 | 65 | 93 | 100 | 114 => x.set16(Camera::MODE, mode2::FULL),
        1 | 39 | 43 | 56 | 57 | 63 | 66 | 112 => x.set16(Camera::MODE, mode2::QUARTER_THREE),
        67 => x.set16(Camera::MODE, mode2::QUARTER_BOTH),
        4 => x.set16(Camera::MODE, mode2::WAIT_THEN_CREEP),
        35 => x.set16(Camera::MODE, mode2::HALF_X),
        8 | 41 => x.set16(Camera::MODE, mode2::HALF_SYNC),
        105 => {}
        13 => falling_leaves(x),
        54 | 59 => x.set16(Camera::MODE, mode2::FROM_CAMERA_1),
        46 | 113 => rising(x),
        69 | 71 | 72 | 76 | 78 | 81..=89 => x.set16(Camera::MODE, mode2::SCREEN_X),
        11 | 94 => x.set16(Camera::MODE, mode2::FOLLOWED),
        6 | 12 | 101 => x.set16(Camera::MODE, mode2::CREEP_3),
        18 => x.set16(Camera::MODE, mode2::DRIFT),
        25 | 106..=108 => x.set16(Camera::MODE, mode2::QUARTER_X),
        91 => x.set16(Camera::MODE, mode2::CREEP_6),
        _ => x.set16(Camera::MODE, mode2::HALF),
    }
}

/// Scene 13: the scenery rises, then leaves fall (scenery objects created at random).
pub(super) fn falling_leaves(x: &mut Cam) {
    const VARIANTS: [u8; 8] = [0x4b, 0x4c, 0x4d, 0x58, 0x4c, 0x4d, 0x58, 0x4b];
    match x.u8(Camera::STEP) {
        0x00 => {
            x.add8(Camera::STEP, 4);
            x.set16(Camera::TIMER, 128);
            x.set16(Camera::TIMER_2, 0);
        }
        0x04 => {
            let t = x.u16(Camera::TIMER).wrapping_sub(1);
            x.set16(Camera::TIMER, t);
            if (t as i16) < 0 {
                x.add8(Camera::STEP, 4);
            }
        }
        0x08 => {
            let y = x.u16(Camera::Y).wrapping_sub(1);
            x.set16(Camera::Y, y);
            if (y as i16) <= 656 {
                x.set16(Camera::Y, 656);
                x.add8(Camera::STEP, 4);
                x.set16(Camera::TIMER, 512);
            }
        }
        0x0c => {
            if (Word(ram::STAGE_CLOCK.0).get(x.b) as i16) <= 0 {
                x.add8(Camera::STEP, 4);
                return;
            }
            let t = x.u16(Camera::TIMER_2).wrapping_sub(1);
            x.set16(Camera::TIMER_2, t);
            if (t as i16) >= 0 {
                return;
            }
            let wait = x.random() & 63;
            x.set16(Camera::TIMER_2, wait);
            let Some(leaf) = x.alloc(&pools::SOBJ) else { return };
            leaf.set_active(x.b, 1);
            leaf.set_kind(x.b, 0);
            let variant = VARIANTS[(x.random() & 7) as usize];
            leaf.set_variant(x.b, variant);
            leaf.set_setting(x.b, 0);
        }
        0x10 => {
            let y = x.u16(Camera::Y).wrapping_sub(1);
            x.set16(Camera::Y, y);
            if (y as i16) < 0 {
                x.set16(Camera::Y, y.wrapping_add(64));
            }
        }
        _ => {}
    }
}

/// Scenes 46 and 113: the scenery descends from 0 to 704, creating an object on the way.
pub(super) fn rising(x: &mut Cam) {
    let finish = |x: &mut Cam| {
        let y = x.u16(Camera::Y).wrapping_add(4);
        x.set16(Camera::Y, y);
        if (y as i16) >= 704 {
            x.set16(Camera::Y, 704);
            x.set16(Camera::MODE, mode2::FREE);
            x.add8(Camera::STEP, 4);
        }
    };
    match x.u8(Camera::STEP) {
        0x00 => {
            let y = x.u16(Camera::Y).wrapping_sub(4);
            x.set16(Camera::Y, y);
            if (y as i16) >= 0 {
                return;
            }
            x.set16(Camera::Y, 0);
            x.set16(Camera::X, 560);
            let Some(obj) = x.alloc(&pools::BOSS) else { return };
            obj.set_active(x.b, 1);
            obj.set_kind(x.b, 0xf);
            x.add8(Camera::STEP, 4);
            finish(x);
        }
        0x04 => finish(x),
        _ => {}
    }
}

/// 0x32f7c / 0x32fd2: the scenery advances 6 pixels per frame over a 0x600-pixel buffer, which it
/// brings back by 0x200; the first time, it locks onto the next page.
pub(super) fn strolling(x: &mut Cam) {
    x.add16(Camera::X, 6);
    let px = x.u16(Camera::X);
    x.set16(Camera::DRAWN_X, px);
    if x.u8(Camera::STEP) == 0 {
        let px = px & 0xff;
        x.set16(Camera::X, px);
        x.set16(Camera::DRAWN_X, px);
        x.add16(Camera::Y, 0x100);
        x.add16(Camera::DRAWN_Y, 0x100);
        x.add8(Camera::STEP, 2);
    } else if (px as i16) >= 0x600 {
        x.add16(Camera::X, 0xfe00);
        x.add16(Camera::DRAWN_X, 0xfe00);
    }
}

/// 0x33152: a scenery object is created when the scenery has arrived at the bottom.
pub(super) fn rubble(x: &mut Cam) {
    let Some(obj) = x.alloc(&pools::ENEMY) else { return };
    let v = obj.active(x.b).wrapping_add(1);
    obj.set_active(x.b, v);
    obj.set_kind(x.b, 0x15);
    x.set16(Camera::MODE, mode2::RUBBLE);
}

/// Runs the script of camera 2's mode (A0).
pub(super) fn run_mode_2(x: &mut Cam, mode: u16) {
    match mode {
        mode2::STAGE => stage_script_2(x),
        mode2::FREE => {}
        0x08 => {
            let dx = delta(x, 0);
            drift(x, dx >> 1, 0);
            stage_script_2(x);
        }
        0x0c => {
            let (dx, dy) = (delta(x, 0), delta(x, 1));
            drift(x, dx << 1, dy << 1);
            stage_script_2(x);
        }
        0x10 | 0x20 => strolling(x),
        mode2::HALF => {
            let (dx, dy) = (delta(x, 0), delta(x, 1));
            drift(x, dx >> 1, dy >> 1);
            stage_script_2(x);
        }
        mode2::FULL => {
            let (dx, dy) = (delta(x, 0), delta(x, 1));
            drift(x, dx, dy);
            stage_script_2(x);
        }
        mode2::DRIFT => {
            x.add32(Camera::X_LONG, 0x4000);
            let px = x.u16(Camera::X);
            x.set16(Camera::DRAWN_X, px);
        }
        0x24 => {
            let (dx, dy) = (delta(x, 0), delta(x, 1));
            drift(x, (dx >> 1).wrapping_add(dx), dy >> 1);
            stage_script_2(x);
        }
        0x28 => {
            let dx = delta(x, 0);
            drift(x, dx, 0);
            let y = x.u16(Camera::Y).wrapping_sub(8);
            x.set16(Camera::Y, y);
            if (y as i16) <= 0 {
                x.add16(Camera::X, 0x400);
                x.add16(Camera::DRAWN_X, 0x400);
                x.set16(Camera::MODE, mode2::RUBBLE);
                rubble(x);
            }
        }
        mode2::QUARTER_X => {
            let dx = delta(x, 0);
            drift(x, dx >> 2, 0);
            stage_script_2(x);
        }
        0x30 => {
            let dx = delta(x, 0);
            drift(x, dx, 0);
            if (x.u16(Camera::Y) as i16) >= 944 {
                rubble(x);
            } else {
                x.add16(Camera::Y, 3);
            }
        }
        mode2::RUBBLE => {
            let dx = delta(x, 0);
            drift(x, dx, 0);
        }
        mode2::QUARTER_THREE => crate::scroll::parallax_quarter(x.b, x.o),
        0x3c => crate::scroll::parallax_three_quarters(x.b, x.o),
        mode2::FOLLOWED => {
            let d = CAMERA_2.x_long(x.b).wrapping_sub(CAMERA_2.previous_x_long(x.b));
            let v = CAMERA_1.x_long(x.b).wrapping_add(d);
            CAMERA_1.set_x_long(x.b, v);
            stage_script_2(x);
        }
        mode2::QUARTER_BOTH => {
            let (dx, dy) = (delta(x, 0), delta(x, 1));
            drift(x, dx >> 2, dy >> 2);
        }
        mode2::CREEP_6 => {
            crate::scroll::creep(x.b, x.o, 6);
        }
        mode2::HALF_SYNC => {
            let d = (delta(x, 0) >> 1) as u32;
            x.add32(Camera::X_LONG, d);
            x.add32(Camera::DRAWN_X_LONG, d);
            stage_script_2(x);
        }
        mode2::HALF_X => {
            let dx = delta(x, 0);
            drift(x, dx >> 1, 0);
            stage_script_2(x);
        }
        0x54 | 0x6c | 0x70 => shake(x, 1),
        0x58 => shake(x, 2),
        0x5c => shake(x, 3),
        mode2::SCREEN_X => {
            let v = ram::CAMERA1_X.get(x.b).wrapping_add(x.u16(Camera::SCREEN_OFFSET));
            x.set16(Camera::X, v.wrapping_sub(0x90));
        }
        mode2::WAIT_THEN_CREEP => match x.u8(Camera::STEP) {
            0 => {
                x.add8(Camera::STEP, 4);
                let px = x.u16(Camera::X);
                x.set16(Camera::DRAWN_X, px);
            }
            8 => {
                x.add16(Camera::X, 4);
                let px = x.u16(Camera::X);
                x.set16(Camera::DRAWN_X, px);
            }
            0xc => x.set16(Camera::MODE, mode2::CREEP_6),
            _ => {}
        },
        mode2::CREEP_3 => {
            crate::scroll::creep(x.b, x.o, 3);
        }
        mode2::FROM_CAMERA_1 => {
            let y = ((CAMERA_1.x(x.b) & 0x30) << 4).wrapping_add(8);
            x.set16(Camera::Y, y);
            x.set16(Camera::DRAWN_Y, y);
        }
        mode2::DRIVES_CAMERA_1 => {
            let dx = (CAMERA_2.x_long(x.b).wrapping_sub(CAMERA_2.previous_x_long(x.b)) as i32) >> 1;
            let dy = (CAMERA_2.y_long(x.b).wrapping_sub(CAMERA_2.previous_y_long(x.b)) as i32) >> 1;
            let v = CAMERA_1.x_long(x.b).wrapping_add(dx as u32);
            CAMERA_1.set_x_long(x.b, v);
            let v = CAMERA_1.y_long(x.b).wrapping_add(dy as u32);
            CAMERA_1.set_y_long(x.b, v);
            if (CAMERA_1.x(x.b) as i16) > 576 {
                CAMERA_1.set_x(x.b, 576);
            }
            stage_script_2(x);
        }
        _ => {}
    }
}

/// 0x32898: one step of the initialised camera 2: mode script, then scenery copy.
pub(super) fn step_2(b: &mut World) -> crate::scroll::D4Leftovers {
    let mut x = on(b, CAMERA_2);
    x.set8(Camera::SHAKING, 0);
    let m = x.u16(Camera::MODE);
    run_mode_2(&mut x, m);
    crate::scroll::draw2(x.b)
}

/// Clears the per-line scroll bit of the video register copy (without writing the register).
pub(super) fn clear_rowscroll_flag(x: &mut Cam) {
    let v = VIDEO_CONTROL_COPY.get(x.b) & !1;
    VIDEO_CONTROL_COPY.set(x.b, v);
}

// ---------------------------------------------------------------------------------------------
