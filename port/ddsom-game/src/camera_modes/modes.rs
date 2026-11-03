//! Mode scripts of camera 1 (`run_mode`): per-line scroll effects, gates, pushes, shakes, storms.

use super::*;

/// Drawn position = target position.
pub(super) fn sync_drawn(x: &mut Cam) {
    let (px, py) = (x.u16(cam::X), x.u16(cam::Y));
    x.set16(cam::DRAWN_X, px);
    x.set16(cam::DRAWN_Y, py);
}

/// 0x31888: direction of the vertical push according to the drawn position, then movement.
pub(super) fn push_vertical(x: &mut Cam) {
    x.set16(f::SPEED, 0xffff);
    if x.u16(cam::DRAWN_Y) as i16 > x.u16(cam::Y) as i16 {
        let v = 0u16.wrapping_sub(x.u16(f::SPEED));
        x.set16(f::SPEED, v);
    }
    x.move_z();
}

/// 0x3016e: sky gradient in the per-line scroll table.
pub(super) fn sky_gradient(x: &mut Cam) {
    follow_objects(x);
    if x.u8(f::STEP) == 0 {
        x.add8(f::STEP, 1);
        rowscroll_on(x);
        x.set16(f::GRADIENT_POS, 0x9200);
        for k in 0..0x101 {
            wr32(x.b, hw::ROWSCROLL + 4 * k, 0x0090_0090);
        }
        return;
    }
    let px = x.u16(cam::X) as i16;
    x.set16(cam::X, px.clamp(0, 0xd0) as u16);
    let step = ((0x90u16.wrapping_sub(rd16(x.b, A5 + 0x5a6)) as u32) << 16) as i32 >> 7;
    let table = ((x.u16(f::GRADIENT_POS) as u32 ^ 8) << 8) + 0x1c0;
    let mut acc: u32 = 0x0090_0000;
    x.set32(f::GRADIENT_ACC, acc);
    let first = (acc >> 16) as u16;
    let mut at = table;
    for _ in 0..32 {
        at -= 2;
        wr16(x.b, at, first);
    }
    for _ in 0..80 {
        acc = acc.wrapping_add(step as u32);
        at -= 2;
        wr16(x.b, at, (acc >> 16) as u16);
    }
    x.set32(f::GRADIENT_ACC, acc);
    wr16(x.b, A5 + 0x57e, (acc >> 16) as u16);
    for _ in 0..56 {
        at -= 4;
        wr32(x.b, at, (first as u32) << 16 | first as u32);
    }
}

/// 0x3022a: undulation (starting per-line scroll table in ROM, then background offset).
pub(super) fn ripple(x: &mut Cam) {
    if x.u8(f::STEP) == 0 {
        x.add8(f::STEP, 1);
        let y = x.u16(cam::Y);
        x.set16(f::SCRATCH, y);
        rowscroll_on(x);
        let mut at = hw::ROWSCROLL;
        for _ in 0..9 {
            for k in 0..16 {
                let v = x.data32(0x3028e + 4 * k);
                wr32(x.b, at, v);
                at += 4;
            }
        }
        return;
    }
    let reference = rd16(x.b, A5 + 0x5aa);
    let delta = x.u16(f::SCRATCH).wrapping_sub(reference);
    x.set16(f::SCRATCH, reference);
    let v = rd16(x.b, A5 + BACKDROP).wrapping_add(delta);
    wr16(x.b, A5 + BACKDROP, v);
    let long = crate::mem::rd32(x.b, A5 + BACKDROP).wrapping_add(0x4000);
    wr32(x.b, A5 + BACKDROP, long);
    let v = rd16(x.b, A5 + BACKDROP) & 0x1f;
    wr16(x.b, A5 + BACKDROP, v);
}

/// Oscillating per-line scroll table: two counters (0x4a, 0x4c) that advance by 1 and 2,
/// written 129 times; the order of the two words depends on the active table.
pub(super) fn waves(x: &mut Cam) {
    let a = x.u16(f::TIMER).wrapping_add(1);
    let a = if (a as i16) < 0x100 { a } else { 0 };
    x.set16(f::TIMER, a);
    let b = x.u16(f::TIMER_2).wrapping_add(2);
    let b = if (b as i16) < 0x100 { b } else { 0 };
    x.set16(f::TIMER_2, b);
    let (mut at, first, second) = if rd16(x.b, A5 + BACKDROP) != 0 { (hw::ROWSCROLL, b, a) } else { (hw::ROWSCROLL_B, a, b) };
    for _ in 0..129 {
        wr16(x.b, at, first);
        wr16(x.b, at + 2, second);
        at += 4;
    }
}

/// Camera 2: copy of the previous position before moving it.
pub(super) fn remember_camera_2(x: &mut Cam) {
    let (px, py) = (rd16(x.b, A5 + CAM2_X), rd16(x.b, A5 + CAM2_Y));
    wr16(x.b, A5 + CAM2_PREVIOUS_X, px);
    wr16(x.b, A5 + CAM2_PREVIOUS_Y, py);
}

/// 0x303f2: camera 2 stays between camera 1's stop and its own left stop.
pub(super) fn clamp_camera_2(x: &mut Cam) {
    let max = x.u16(cam::MAX_X) as i16;
    if max < rd16(x.b, A5 + CAM2_X) as i16 {
        wr16(x.b, A5 + CAM2_X, max as u16);
    }
    let min = rd16(x.b, A5 + CAM2_MIN_X);
    if min as i16 > rd16(x.b, A5 + CAM2_X) as i16 {
        wr16(x.b, A5 + CAM2_X, min);
    }
    stage_script(x);
}

/// Runs `f` with camera 2 as the current camera.
pub(super) fn on_camera_2(x: &mut Cam, f: fn(&mut Cam)) {
    let cam1 = x.o;
    x.o = Obj(A5 + CAMERA_2);
    f(x);
    x.o = cam1;
}

/// 0x30308: undulation, then camera 2 follows the players.
pub(super) fn waves_and_follow(x: &mut Cam) {
    waves(x);
    let px = rd16(x.b, A5 + CAM2_X);
    wr16(x.b, A5 + CAM2_PREVIOUS_X, px);
    on_camera_2(x, follow_x);
    clamp_camera_2(x);
}

/// 0x3041c: camera 2 follows the depth of the living players.
pub(super) fn camera_2_depth(x: &mut Cam) {
    remember_camera_2(x);
    if any(x, alive) {
        on_camera_2(x, follow_depth);
    }
}

/// 0x303c8: camera 2 follows the players (narrow), then stays within the stops.
pub(super) fn camera_2_follow(x: &mut Cam) {
    remember_camera_2(x);
    if any(x, present) {
        on_camera_2(x, follow_xy);
        clamp_camera_2(x);
    }
}

/// Creation of a scenery object (SET) at the end of a scroll: `kind` written to its type.
pub(super) fn spawn_gate(x: &mut Cam, pool: &pools::Pool, kind: u8, bump: u8) -> bool {
    let Some(Obj(obj)) = x.alloc(pool) else { return false };
    let variant = x.u8(f::KIND);
    wr8(x.b, obj, 1);
    wr8(x.b, obj + 2, kind);
    wr8(x.b, obj + 3, variant.wrapping_add(bump));
    true
}

/// 0x3047e / 0x3053c: the camera goes to `target` (2 pixels per frame); on arrival: an object is created and
/// the mode switches to `next`.
pub(super) fn advance_to(x: &mut Cam, target: i16, next: u16) {
    let mut px = x.u16(cam::X) as i16;
    if target >= px {
        px = px.wrapping_add(2);
        x.set16(cam::X, px as u16);
        if target > px {
            return;
        }
    } else {
        px = px.wrapping_sub(2);
        x.set16(cam::X, px as u16);
        if target < px {
            return;
        }
    }
    x.set16(cam::X, target as u16);
    if spawn_gate(x, &pools::EFFCT, 0, 0) {
        x.set16(cam::MODE, next);
        x.set8(f::STEP, 0);
        x.add16(f::TIMER, 1);
    }
}

/// 0x304dc: with the portal created, the camera locks and shifts the players present.
pub(super) fn gate(x: &mut Cam) {
    if x.u8(f::STEP) == 0 {
        return;
    }
    if !spawn_gate(x, &pools::SET, 2, 1) {
        return;
    }
    x.set16(cam::X, 0x468);
    x.set16(cam::MIN_X, 0x468);
    let shift = if x.u16(f::TIMER) == 1 { 0x400u16 } else { 0xfec0 };
    shift_players(x, shift);
    stage_script(x);
}

pub(super) fn shift_players(x: &mut Cam, shift: u16) {
    for k in 0..PLAYER_COUNT {
        if present(x, k) {
            let v = rd16(x.b, record(k) + Obj::X).wrapping_add(shift);
            wr16(x.b, record(k) + Obj::X, v);
        }
    }
}

/// 0x3058e: exit from the second portal.
pub(super) fn gate_far(x: &mut Cam) {
    match x.u8(f::STEP) {
        0x04 => x.add16(cam::X, 0xfffe),
        0x08 => {
            if !spawn_gate(x, &pools::SET, 2, 1) {
                return;
            }
            x.set16(cam::X, 0x820);
            x.set16(cam::MIN_X, 0x820);
            shift_players(x, 0x2c0);
            stage_script(x);
            start_shake(x.b, shake::C, 0x20);
        }
        _ => {}
    }
}

/// 0x305fc: the camera follows the players forwards; the depth follows a quarter of the movement.
pub(super) fn push(x: &mut Cam) {
    let before = x.u32(cam::X);
    follow_x(x);
    if (x.u16(cam::X) as i16) < x.u16(cam::MIN_X) as i16 {
        let m = x.u16(cam::MIN_X);
        x.set16(cam::X, m);
    }
    let moved = (before.wrapping_sub(x.u32(cam::X)) as i32) >> 2;
    x.add32(cam::Y, moved as u32);
    stage_script(x);
}

/// 0x3063a: shake of 4 positions (movements `dx`, `dy` per position), then end.
pub(super) fn shake(x: &mut Cam, amplitude: i16) {
    let t = x.u16(f::SHAKE_COUNT).wrapping_sub(1);
    x.set16(f::SHAKE_COUNT, t);
    if (t as i16) < 0 {
        let (sx, sy, sm) = (x.u16(f::SHAKE_X), x.u16(f::SHAKE_Y), x.u16(f::SHAKE_MODE));
        x.set16(cam::X, sx);
        x.set16(cam::Y, sy);
        x.set16(cam::MODE, sm);
        x.set8(f::SHAKING, 0);
        return;
    }
    x.set8(f::SHAKING, 1);
    const OFFSETS: [(i16, i16); 4] = [(1, -1), (1, 1), (-1, -1), (-1, 1)];
    let (dx, dy) = OFFSETS[(t & 3) as usize];
    let (sx, sy) = (x.u16(f::SHAKE_X), x.u16(f::SHAKE_Y));
    x.set16(cam::X, sx.wrapping_add((dx * amplitude) as u16));
    x.set16(cam::Y, sy.wrapping_add((dy * amplitude) as u16));
}

/// Fills the per-line scroll table with random values (mask `mask`): 513 words.
pub(super) fn rowscroll_noise(x: &mut Cam, mask: u16) {
    for k in 0..0x201 {
        let v = x.random() & mask;
        wr16(x.b, hw::ROWSCROLL + 2 * k, v);
    }
}

/// Shifts the per-line scroll table by one word towards the start and adds a draw at the end.
pub(super) fn rowscroll_shift(x: &mut Cam, mask: u16, tail_offset: u32) {
    for k in 0..0x200 {
        let v = rd16(x.b, hw::ROWSCROLL + 2 * k + 2);
        wr16(x.b, hw::ROWSCROLL + 2 * k, v);
    }
    let v = x.random() & mask;
    wr16(x.b, hw::ROWSCROLL + 0x400 + tail_offset, v);
}

/// 0x308da: storm (noise in the per-line scroll table), return to free mode.
pub(super) fn storm(x: &mut Cam) {
    match x.u8(f::STEP) {
        0 => {
            x.add8(f::STEP, 4);
            rowscroll_on(x);
            wr16(x.b, hw::ROWSCROLL_OFFSET, 0);
            rowscroll_noise(x, 0xf);
        }
        4 => {
            let t = x.u16(f::TIMER).wrapping_sub(1);
            x.set16(f::TIMER, t);
            if (t as i16) < 0 {
                x.set16(cam::MODE, mode::FREE);
                x.set8(f::STEP, 0);
                rowscroll_control(x, false);
            } else {
                rowscroll_shift(x, 7, 0);
            }
        }
        _ => {}
    }
}

/// Animated palettes (in ROM: source and destination pointer per 8-byte entry).
pub(super) const PALETTE_STEPS: u32 = 0x30816;

/// 0x3071a: thunderstorm (like `storm`, then fade of the lightning palettes).
pub(super) fn thunder(x: &mut Cam) {
    match x.u8(f::STEP) {
        0 => {
            x.add8(f::STEP, 4);
            x.set16(f::TIMER, 24);
            rowscroll_on(x);
            wr16(x.b, hw::ROWSCROLL_OFFSET, 0);
            rowscroll_noise(x, 0xf);
        }
        4 => {
            let t = x.u16(f::TIMER).wrapping_sub(1);
            x.set16(f::TIMER, t);
            if (t as i16) < 0 {
                x.set32(f::TIMER, 0);
                x.add8(f::STEP, 4);
            } else {
                rowscroll_shift(x, 7, 2);
            }
        }
        8 => {
            rowscroll_shift(x, 0xf, 2);
            let t = x.u16(f::TIMER).wrapping_sub(1);
            x.set16(f::TIMER, t);
            if (t as i16) >= 0 {
                return;
            }
            x.set16(f::TIMER, 1);
            // the original calls the same subroutine twice (`bsr` to the next instruction)
            for _ in 0..2 {
                palette_step(x);
            }
        }
        _ => {}
    }
}

/// 0x307d8: one step of the palette fade; end of the table: the per-line scroll stops.
pub(super) fn palette_step(x: &mut Cam) {
    let at = x.u16(f::TIMER_2) as i16 as i32 as u32;
    let rom32 = |x: &mut Cam, a: u32| (x.rom16(a) as u32) << 16 | x.rom16(a + 2) as u32;
    let source = rom32(x, PALETTE_STEPS.wrapping_add(at));
    if source == 0 {
        rowscroll_control(x, false);
        stage_script(x);
        return;
    }
    let mut dest = rom32(x, PALETTE_STEPS.wrapping_add(at) + 4);
    let mut from = source;
    for _ in 0..12 {
        let kept = rd16(x.b, dest + 2) as u32 & 0xff80;
        let v = kept.wrapping_add(x.data32(from));
        from += 4;
        wr32(x.b, dest, v);
        dest += 4;
    }
    x.add16(f::TIMER_2, 8);
}

/// 0x30988: waits, then the camera descends to 256 (scene 67).
pub(super) fn waves_left(x: &mut Cam) {
    match x.u8(f::STEP) {
        0x00 => {
            x.add8(f::STEP, 4);
            x.set16(f::TIMER, 0x40);
        }
        0x04 => {
            let t = x.u16(f::TIMER).wrapping_sub(1);
            x.set16(f::TIMER, t);
            if (t as i16) >= 0 {
                return;
            }
            x.add8(f::STEP, 4);
        }
        0x08 => {
            if x.u16(cam::Y) as i16 <= 368 {
                wr8(x.b, A5 + 0x42, 1);
            }
            let y = (x.u16(cam::Y) as i16).wrapping_sub(4);
            x.set16(cam::Y, if y > 256 { y as u16 } else { 256 });
            stage_script(x);
        }
        _ => {}
    }
}

/// 0x31822: the camera follows an object (pointer at word `A5+0x46`).
pub(super) fn screen_object(x: &mut Cam) {
    let obj = rd16(x.b, A5 + 0x46) as i16 as i32 as u32;
    let v = rd16(x.b, obj + Obj::X);
    track(x, cam::X, (v, v), X_OBJECT);
    let v = rd16(x.b, obj + Obj::Y);
    track(x, cam::Y, (v, v), Y_OBJECT);
}

/// 0x3047e..0x3071a, 0x3141a...: runs the script of mode `mode` of camera 1.
pub(super) fn run_mode(x: &mut Cam, mode: u16) {
    match mode {
        mode::STAGE => stage_script(x),
        mode::FREE => {}
        mode::FOLLOW => {
            follow_xy(x);
            stage_script(x);
        }
        mode::SKY_GRADIENT => sky_gradient(x),
        0x10 => {
            follow_x(x);
            push_vertical(x);
            stage_script(x);
        }
        mode::FOLLOW_X => {
            follow_x(x);
            stage_script(x);
        }
        mode::RIPPLE => ripple(x),
        mode::FOLLOW_SYNC => {
            follow_xy(x);
            sync_drawn(x);
            stage_script(x);
        }
        0x20 => {
            if any(x, alive) {
                follow_depth(x);
            }
        }
        0x24 => {
            follow_xy(x);
            sync_drawn(x);
        }
        0x28 => {
            let sum = rd16(x.b, A5 + 0x5e6).wrapping_add(rd16(x.b, A5 + 0x7e6)) >> 1;
            x.set16(cam::X, sum.wrapping_sub(0xc0));
            follow_depth(x);
        }
        mode::WAVES => waves_and_follow(x),
        0x30 => camera_2_depth(x),
        0x34 => storm(x),
        mode::DUAL_FOLLOW => camera_2_follow(x),
        mode::WALL_RIGHT => {
            let target = if x.u16(f::TIMER) != 0 { 0x5a8 } else { 0x68 };
            advance_to(x, target, mode::GATE);
        }
        mode::GATE => gate(x),
        mode::PUSH => push(x),
        mode::PARKED => {
            if rd8(x.b, A5 + 0x2c) == 12 {
                x.add16(cam::X, 1);
            }
            let px = x.u16(cam::X);
            x.set16(cam::DRAWN_X, px);
        }
        mode::WALL_FAR => advance_to(x, 0x640, mode::GATE_FAR),
        mode::GATE_FAR => gate_far(x),
        0x54 | 0x68 | 0x6c | 0x70 => shake(x, 1),
        0x58 => shake(x, 2),
        0x5c => shake(x, 3),
        0x60 => thunder(x),
        mode::SNAP => follow_snap(x),
        mode::FOLLOW_X_SYNC => {
            follow_x(x);
            sync_drawn(x);
            stage_script(x);
        }
        0x78 => {
            follow_xy(x);
            ripple(x);
        }
        mode::SCREEN_OBJECT => screen_object(x),
        0x80 => waves(x),
        mode::WAVES_LEFT => waves_left(x),
        mode::FOLLOW_MID => follow_mid(x),
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
