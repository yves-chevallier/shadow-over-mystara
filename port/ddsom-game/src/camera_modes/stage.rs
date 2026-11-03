//! Per-scene dispatch of camera 1 (0x309e0) and the scene sequences it plays.

use super::*;

/// Writes the video control register with bit 0 set (per-line scroll) or cleared.
pub(super) fn rowscroll_control(x: &mut Cam, on: bool) {
    let v = rd16(x.b, A5 + VIDEO_CONTROL_COPY);
    let v = if on { v | 1 } else { v & !1 };
    wr16(x.b, A5 + VIDEO_CONTROL_COPY, v);
    wr16(x.b, hw::VIDEO_CONTROL, v);
}

/// Enables the per-line scroll table in GFX RAM.
pub(super) fn rowscroll_on(x: &mut Cam) {
    rowscroll_control(x, true);
    wr16(x.b, hw::ROWSCROLL_BASE, 0x9200);
}

/// 0x309e0: depending on the scene, sets the camera mode or plays a scene sequence.
pub(super) fn stage_script(x: &mut Cam) {
    let stage = rd8(x.b, A5 + STAGE);
    match stage {
        2 | 83..=90 | 102 => scripted_scroll(x),
        6 | 12 | 17 | 19 | 97 | 101 => x.set16(cam::MODE, mode::FOLLOW_X),
        1 => arrival(x),
        3 | 13 | 18 | 21..=23 | 26 | 58 | 64 | 68 | 70 | 73..=75 | 77 | 79 | 80 | 93 | 100 => x.set16(cam::MODE, mode::FREE),
        8 => x.set16(cam::MODE, mode::PARKED),
        11 | 94 => {
            if x.u8(f::STEP) == 0 {
                load_backdrops(x, &BACKDROP_BLOCKS_A);
                rowscroll_on(x);
                x.add8(f::STEP, 1);
            }
            x.set16(cam::MODE, mode::WAVES);
        }
        20 => x.set16(cam::MODE, mode::DUAL_FOLLOW),
        38 | 40 => x.set16(cam::MODE, mode::RIPPLE),
        54 | 59 => x.set16(cam::MODE, mode::FOLLOW_X_SYNC),
        69 | 71 | 72 | 76 | 78 | 81 | 82 => {
            rowscroll_on(x);
            wr16(x.b, hw::ROWSCROLL_OFFSET, 0);
            x.set8(cam::FROZEN, 1);
            x.set16(cam::MODE, mode::SKY_GRADIENT);
        }
        4 | 91 => rise_then_follow(x),
        33 | 104 => pause_then_follow(x),
        7 | 9 => x.set16(cam::MODE, mode::SCREEN_OBJECT),
        14 | 92 | 95 | 109 => x.set16(cam::MODE, mode::SNAP),
        67 => x.set16(cam::MODE, mode::WAVES_LEFT),
        41 => x.set16(cam::MODE, mode::FOLLOW_SYNC),
        66 => climb(x, 1, 296),
        105 => x.set16(cam::MODE, mode::FOLLOW_MID),
        44 | 111 => climb(x, 2, 280),
        _ => x.set16(cam::MODE, mode::FOLLOW),
    }
}

/// Tile blocks loaded into the scroll layers (block index, GFX RAM address).
pub(super) const BACKDROP_BLOCKS_A: [(u16, u32); 8] = [
    (206, 0x917000),
    (173, 0x917400),
    (175, 0x917800),
    (175, 0x917c00),
    (172, 0x916000),
    (205, 0x916400),
    (174, 0x916800),
    (174, 0x916c00),
];

pub(super) fn load_backdrops(x: &mut Cam, blocks: &[(u16, u32)]) {
    for &(index, dest) in blocks {
        crate::video::load_grid(x.b, Grid::Layer2, index, dest);
    }
}

/// Scenes 2, 83-90, 102: camera 2 scrolls according to a script (step of camera 1).
pub(super) fn scripted_scroll(x: &mut Cam) {
    let sync_previous = |x: &mut Cam| {
        let v = rd16(x.b, A5 + CAM2_X);
        wr16(x.b, A5 + CAM2_PREVIOUS_X, v);
    };
    match x.u8(f::STEP) {
        0x00 => x.set16(cam::MODE, mode::DUAL_FOLLOW),
        0x08 => {
            wr16(x.b, A5 + CAM2_MIN_X, 0);
            sync_previous(x);
            let v = rd16(x.b, A5 + CAM2_X).wrapping_sub(4);
            wr16(x.b, A5 + CAM2_X, v);
            if (v as i16) < 0 {
                wr16(x.b, A5 + CAM2_X, 0);
                x.add8(f::STEP, 4);
                sync_previous(x);
            }
        }
        0x0c => sync_previous(x),
        0x10 => {
            sync_previous(x);
            let v = rd16(x.b, A5 + CAM2_X).wrapping_add(4);
            wr16(x.b, A5 + CAM2_X, v);
            if (v as i16) >= 384 {
                x.add8(f::STEP, 4);
            }
        }
        0x14 => {
            sync_previous(x);
            let v = rd16(x.b, A5 + CAM2_X).wrapping_add(16);
            wr16(x.b, A5 + CAM2_X, v);
            if (v as i16) >= 800 {
                x.add8(f::STEP, 4);
                wr16(x.b, A5 + CAM2_X, 800);
                wr16(x.b, A5 + CAM2_PREVIOUS_X, 800);
                x.set16(f::TIMER, 120);
                // 0xa8ae8: scenery list 2
                crate::families::effects_creators::set_list(x.b, 2);
            }
        }
        0x18 => {
            let t = x.u16(f::TIMER).wrapping_sub(1);
            x.set16(f::TIMER, t);
            if (t as i16) < 0 {
                x.add8(f::STEP, 4);
                x.set16(cam::MODE, mode::STAGE);
                sync_previous(x);
                start_shake(x.b, shake::A, 30);
            }
        }
        0x20 => {
            sync_previous(x);
            let v = rd16(x.b, A5 + CAM2_X).wrapping_add(8);
            wr16(x.b, A5 + CAM2_X, v);
            if (v as i16) >= 1280 {
                wr16(x.b, A5 + CAM2_X, 1280);
                x.add8(f::STEP, 4);
            }
        }
        0x04 | 0x1c | 0x24 => {
            sync_previous(x);
            let v = rd16(x.b, A5 + CAM2_Y);
            wr16(x.b, A5 + CAM2_PREVIOUS_Y, v);
        }
        _ => {}
    }
}

/// Scene 1: the camera locks according to the arrival abscissa.
pub(super) fn arrival(x: &mut Cam) {
    let step = x.u8(f::STEP);
    let px = x.u16(cam::X) as i16;
    let first = if step != 0 { px <= 299 } else { px <= 0 };
    if first {
        x.set16(cam::Y, if step != 0 { 470 } else { 544 });
        x.set16(cam::MODE, mode::FOLLOW_X);
    } else if px <= 320 {
        x.set16(cam::MODE, mode::PUSH);
    } else {
        x.set16(cam::Y, 464);
        x.set16(cam::PREVIOUS_Y, 464);
        x.set16(cam::MODE, mode::FOLLOW_X);
    }
}

/// Scenes 4 and 91: wait, then the camera rises to `48` and switches to following.
pub(super) fn rise_then_follow(x: &mut Cam) {
    match x.u8(f::SUB_STEP) {
        0x00 => x.add8(f::SUB_STEP, 4),
        0x08 => {
            let v = x.u16(cam::X).wrapping_sub(2);
            x.set16(cam::X, v);
            if (v as i16) <= 0x30 {
                x.add8(f::SUB_STEP, 4);
                let a = rd8(x.b, A5 + 0x54f).wrapping_add(4);
                wr8(x.b, A5 + 0x54f, a);
                x.set16(f::TIMER, 0);
            }
        }
        0x0c => x.set16(cam::MODE, mode::FOLLOW_X),
        _ => {}
    }
}

/// Scenes 33 and 104: pause of 64 frames then normal following.
pub(super) fn pause_then_follow(x: &mut Cam) {
    match x.u8(f::STEP) {
        0x00 => {
            x.add8(f::STEP, 4);
            x.set16(f::TIMER, 0x40);
        }
        0x04 => {
            let t = x.u16(f::TIMER).wrapping_sub(1);
            x.set16(f::TIMER, t);
            if (t as i16) < 0 {
                x.add8(f::STEP, 4);
            }
        }
        0x08 => x.set16(cam::MODE, mode::FOLLOW),
        _ => {}
    }
}

/// Scenes 66 and 44/111: the camera rises by `speed` pixels per frame to `top`, then following.
pub(super) fn climb(x: &mut Cam, speed: u16, top: i16) {
    match x.u8(f::SUB_STEP) {
        0x00 => {
            let y = x.u16(cam::Y).wrapping_sub(speed);
            x.set16(cam::Y, y);
            if (y as i16) <= top {
                x.set16(cam::Y, top as u16);
                x.add8(f::SUB_STEP, 4);
            }
        }
        0x04 => x.set16(cam::MODE, mode::FOLLOW),
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
