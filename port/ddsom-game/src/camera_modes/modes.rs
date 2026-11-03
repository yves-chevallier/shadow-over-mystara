//! Mode scripts of camera 1 (`run_mode`): per-line scroll effects, gates, pushes, shakes, storms.

use super::*;

/// Drawn position = target position.
pub(super) fn sync_drawn(x: &mut Cam) {
    let (px, py) = (x.u16(Camera::X), x.u16(Camera::Y));
    x.set16(Camera::DRAWN_X, px);
    x.set16(Camera::DRAWN_Y, py);
}

/// 0x31888: direction of the vertical push according to the drawn position, then movement.
pub(super) fn push_vertical(x: &mut Cam) {
    x.set16(Camera::SPEED, 0xffff);
    if x.u16(Camera::DRAWN_Y) as i16 > x.u16(Camera::Y) as i16 {
        let v = 0u16.wrapping_sub(x.u16(Camera::SPEED));
        x.set16(Camera::SPEED, v);
    }
    x.move_z();
}

/// 0x3016e: sky gradient in the per-line scroll table.
pub(super) fn sky_gradient(x: &mut Cam) {
    follow_objects(x);
    if x.u8(Camera::STEP) == 0 {
        x.add8(Camera::STEP, 1);
        rowscroll_on(x);
        x.set16(Camera::GRADIENT_TABLE, 0x9200);
        for k in 0..0x101 {
            ROW_SCROLL.offset(4 * k).set_long(x.b, 0x0090_0090);
        }
        return;
    }
    let px = x.u16(Camera::X) as i16;
    x.set16(Camera::X, px.clamp(0, 0xd0) as u16);
    let step = ((0x90u16.wrapping_sub(ram::CAMERA1_X.get(x.b)) as u32) << 16) as i32 >> 7;
    let table = ((x.u16(Camera::GRADIENT_TABLE) as u32 ^ 8) << 8) + 0x1c0;
    let mut acc: u32 = 0x0090_0000;
    x.set32(Camera::GRADIENT, acc);
    let first = (acc >> 16) as u16;
    let mut at = table;
    for _ in 0..32 {
        at -= 2;
        Vram(at).set(x.b, first);
    }
    for _ in 0..80 {
        acc = acc.wrapping_add(step as u32);
        at -= 2;
        Vram(at).set(x.b, (acc >> 16) as u16);
    }
    x.set32(Camera::GRADIENT, acc);
    CAMERA_2.set_screen_offset(x.b, (acc >> 16) as u16);
    for _ in 0..56 {
        at -= 4;
        Vram(at).set_long(x.b, (first as u32) << 16 | first as u32);
    }
}

/// 0x3022a: undulation (starting per-line scroll table in ROM, then background offset).
pub(super) fn ripple(x: &mut Cam) {
    if x.u8(Camera::STEP) == 0 {
        x.add8(Camera::STEP, 1);
        let y = x.u16(Camera::Y);
        x.set16(Camera::SCRATCH, y);
        rowscroll_on(x);
        let rows = engine().get("camera/ripple_rows");
        let mut at = ROW_SCROLL;
        for _ in 0..9 {
            for k in 0..16 {
                let v = rows.nth32(x.b, k);
                at.set_long(x.b, v);
                at = at.offset(4);
            }
        }
        return;
    }
    let reference = ram::CAMERA1_Y.get(x.b);
    let delta = x.u16(Camera::SCRATCH).wrapping_sub(reference);
    x.set16(Camera::SCRATCH, reference);
    let v = BACKDROP_WORD.get(x.b).wrapping_add(delta);
    BACKDROP_WORD.set(x.b, v);
    let long = BACKDROP.get(x.b).wrapping_add(0x4000);
    BACKDROP.set(x.b, long);
    let v = BACKDROP_WORD.get(x.b) & 0x1f;
    BACKDROP_WORD.set(x.b, v);
}

/// Oscillating per-line scroll table: two counters (0x4a, 0x4c) that advance by 1 and 2,
/// written 129 times; the order of the two words depends on the active table.
pub(super) fn waves(x: &mut Cam) {
    let a = x.u16(Camera::TIMER).wrapping_add(1);
    let a = if (a as i16) < 0x100 { a } else { 0 };
    x.set16(Camera::TIMER, a);
    let b = x.u16(Camera::TIMER_2).wrapping_add(2);
    let b = if (b as i16) < 0x100 { b } else { 0 };
    x.set16(Camera::TIMER_2, b);
    let (mut at, first, second) = if BACKDROP_WORD.get(x.b) != 0 { (ROW_SCROLL, b, a) } else { (ROW_SCROLL_B, a, b) };
    for _ in 0..129 {
        at.set(x.b, first);
        at.offset(2).set(x.b, second);
        at = at.offset(4);
    }
}

/// Camera 2: copy of the previous position before moving it.
pub(super) fn remember_camera_2(x: &mut Cam) {
    let (px, py) = (CAMERA_2.x(x.b), CAMERA_2.y(x.b));
    CAMERA_2.set_previous_x(x.b, px);
    CAMERA_2.set_previous_y(x.b, py);
}

/// 0x303f2: camera 2 stays between camera 1's stop and its own left stop.
pub(super) fn clamp_camera_2(x: &mut Cam) {
    let max = x.u16(Camera::MAX_X) as i16;
    if max < CAMERA_2.x(x.b) as i16 {
        CAMERA_2.set_x(x.b, max as u16);
    }
    let min = CAMERA_2.min_x(x.b);
    if min as i16 > CAMERA_2.x(x.b) as i16 {
        CAMERA_2.set_x(x.b, min);
    }
    stage_script(x);
}

/// Runs `f` with camera 2 as the current camera.
pub(super) fn on_camera_2(x: &mut Cam, f: fn(&mut Cam)) {
    let cam1 = x.o;
    x.o = CAMERA_2.0;
    f(x);
    x.o = cam1;
}

/// 0x30308: undulation, then camera 2 follows the players.
pub(super) fn waves_and_follow(x: &mut Cam) {
    waves(x);
    let px = CAMERA_2.x(x.b);
    CAMERA_2.set_previous_x(x.b, px);
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
    let Some(obj) = x.alloc(pool) else { return false };
    let variant = x.u8(Camera::KIND);
    obj.set_active(x.b, 1);
    obj.set_kind(x.b, kind);
    obj.set_variant(x.b, variant.wrapping_add(bump));
    true
}

/// 0x3047e / 0x3053c: the camera goes to `target` (2 pixels per frame); on arrival: an object is created and
/// the mode switches to `next`.
pub(super) fn advance_to(x: &mut Cam, target: i16, next: u16) {
    let mut px = x.u16(Camera::X) as i16;
    if target >= px {
        px = px.wrapping_add(2);
        x.set16(Camera::X, px as u16);
        if target > px {
            return;
        }
    } else {
        px = px.wrapping_sub(2);
        x.set16(Camera::X, px as u16);
        if target < px {
            return;
        }
    }
    x.set16(Camera::X, target as u16);
    if spawn_gate(x, &pools::EFFCT, 0, 0) {
        x.set16(Camera::MODE, next);
        x.set8(Camera::STEP, 0);
        x.add16(Camera::TIMER, 1);
    }
}

/// 0x304dc: with the portal created, the camera locks and shifts the players present.
pub(super) fn gate(x: &mut Cam) {
    if x.u8(Camera::STEP) == 0 {
        return;
    }
    if !spawn_gate(x, &pools::SET, 2, 1) {
        return;
    }
    x.set16(Camera::X, 0x468);
    x.set16(Camera::MIN_X, 0x468);
    let shift = if x.u16(Camera::TIMER) == 1 { 0x400u16 } else { 0xfec0 };
    shift_players(x, shift);
    stage_script(x);
}

pub(super) fn shift_players(x: &mut Cam, shift: u16) {
    for k in 0..ram::PLAYERS.count {
        if present(x, k) {
            let p = player(k).obj();
            let v = p.u16(x.b, Obj::X).wrapping_add(shift);
            p.set_u16(x.b, Obj::X, v);
        }
    }
}

/// 0x3058e: exit from the second portal.
pub(super) fn gate_far(x: &mut Cam) {
    match x.u8(Camera::STEP) {
        0x04 => x.add16(Camera::X, 0xfffe),
        0x08 => {
            if !spawn_gate(x, &pools::SET, 2, 1) {
                return;
            }
            x.set16(Camera::X, 0x820);
            x.set16(Camera::MIN_X, 0x820);
            shift_players(x, 0x2c0);
            stage_script(x);
            start_shake(x.b, shake::C, 0x20);
        }
        _ => {}
    }
}

/// 0x305fc: the camera follows the players forwards; the depth follows a quarter of the movement.
pub(super) fn push(x: &mut Cam) {
    let before = x.u32(Camera::X_LONG);
    follow_x(x);
    if (x.u16(Camera::X) as i16) < x.u16(Camera::MIN_X) as i16 {
        let m = x.u16(Camera::MIN_X);
        x.set16(Camera::X, m);
    }
    let moved = (before.wrapping_sub(x.u32(Camera::X_LONG)) as i32) >> 2;
    x.add32(Camera::Y_LONG, moved as u32);
    stage_script(x);
}

/// 0x3063a: shake of 4 positions (movements `dx`, `dy` per position), then end.
pub(super) fn shake(x: &mut Cam, amplitude: i16) {
    let t = x.u16(Camera::SHAKE_COUNT).wrapping_sub(1);
    x.set16(Camera::SHAKE_COUNT, t);
    if (t as i16) < 0 {
        let (sx, sy, sm) = (x.u16(Camera::SHAKE_X), x.u16(Camera::SHAKE_Y), x.u16(Camera::SHAKE_MODE));
        x.set16(Camera::X, sx);
        x.set16(Camera::Y, sy);
        x.set16(Camera::MODE, sm);
        x.set8(Camera::SHAKING, 0);
        return;
    }
    x.set8(Camera::SHAKING, 1);
    const OFFSETS: [(i16, i16); 4] = [(1, -1), (1, 1), (-1, -1), (-1, 1)];
    let (dx, dy) = OFFSETS[(t & 3) as usize];
    let (sx, sy) = (x.u16(Camera::SHAKE_X), x.u16(Camera::SHAKE_Y));
    x.set16(Camera::X, sx.wrapping_add((dx * amplitude) as u16));
    x.set16(Camera::Y, sy.wrapping_add((dy * amplitude) as u16));
}

/// Fills the per-line scroll table with random values (mask `mask`): 513 words.
pub(super) fn rowscroll_noise(x: &mut Cam, mask: u16) {
    for k in 0..0x201 {
        let v = x.random() & mask;
        ROW_SCROLL.offset(2 * k).set(x.b, v);
    }
}

/// Shifts the per-line scroll table by one word towards the start and adds a draw at the end.
pub(super) fn rowscroll_shift(x: &mut Cam, mask: u16, tail_offset: u32) {
    for k in 0..0x200 {
        let v = ROW_SCROLL.offset(2 * k + 2).get(x.b);
        ROW_SCROLL.offset(2 * k).set(x.b, v);
    }
    let v = x.random() & mask;
    ROW_SCROLL.offset(0x400 + tail_offset).set(x.b, v);
}

/// 0x308da: storm (noise in the per-line scroll table), return to free mode.
pub(super) fn storm(x: &mut Cam) {
    match x.u8(Camera::STEP) {
        0 => {
            x.add8(Camera::STEP, 4);
            rowscroll_on(x);
            regs::ROW_SCROLL_OFFSET.set(x.b, 0);
            rowscroll_noise(x, 0xf);
        }
        4 => {
            let t = x.u16(Camera::TIMER).wrapping_sub(1);
            x.set16(Camera::TIMER, t);
            if (t as i16) < 0 {
                x.set16(Camera::MODE, mode::FREE);
                x.set8(Camera::STEP, 0);
                rowscroll_control(x, false);
            } else {
                rowscroll_shift(x, 7, 0);
            }
        }
        _ => {}
    }
}

/// 0x3071a: thunderstorm (like `storm`, then fade of the lightning palettes).
pub(super) fn thunder(x: &mut Cam) {
    match x.u8(Camera::STEP) {
        0 => {
            x.add8(Camera::STEP, 4);
            x.set16(Camera::TIMER, 24);
            rowscroll_on(x);
            regs::ROW_SCROLL_OFFSET.set(x.b, 0);
            rowscroll_noise(x, 0xf);
        }
        4 => {
            let t = x.u16(Camera::TIMER).wrapping_sub(1);
            x.set16(Camera::TIMER, t);
            if (t as i16) < 0 {
                x.set32(Camera::TIMERS, 0);
                x.add8(Camera::STEP, 4);
            } else {
                rowscroll_shift(x, 7, 2);
            }
        }
        8 => {
            rowscroll_shift(x, 0xf, 2);
            let t = x.u16(Camera::TIMER).wrapping_sub(1);
            x.set16(Camera::TIMER, t);
            if (t as i16) >= 0 {
                return;
            }
            x.set16(Camera::TIMER, 1);
            // the original calls the same subroutine twice (`bsr` to the next instruction)
            for _ in 0..2 {
                palette_step(x);
            }
        }
        _ => {}
    }
}

/// 0x307d8: one step of the palette fade (`camera/palette_steps`: source in the palette library, destination; 8 bytes per
/// step); end of the table: the per-line scroll stops.
pub(super) fn palette_step(x: &mut Cam) {
    let at = x.u16(Camera::TIMER_2) as i16 as i32 as u32;
    let steps = engine().get("camera/palette_steps");
    let source = steps.at32(x.b, at);
    if source == 0 {
        rowscroll_control(x, false);
        stage_script(x);
        return;
    }
    let mut dest = steps.at32(x.b, at.wrapping_add(4));
    let mut from = source;
    for _ in 0..12 {
        let kept = Vram(dest + 2).get(x.b) as u32 & 0xff80;
        let v = kept.wrapping_add(engine().long(x.b, from));
        from += 4;
        Vram(dest).set_long(x.b, v);
        dest += 4;
    }
    x.add16(Camera::TIMER_2, 8);
}

/// 0x30988: waits, then the camera descends to 256 (scene 67).
pub(super) fn waves_left(x: &mut Cam) {
    match x.u8(Camera::STEP) {
        0x00 => {
            x.add8(Camera::STEP, 4);
            x.set16(Camera::TIMER, 0x40);
        }
        0x04 => {
            let t = x.u16(Camera::TIMER).wrapping_sub(1);
            x.set16(Camera::TIMER, t);
            if (t as i16) >= 0 {
                return;
            }
            x.add8(Camera::STEP, 4);
        }
        0x08 => {
            if x.u16(Camera::Y) as i16 <= 368 {
                ram::PLAYERS_FROZEN.set(x.b, 1);
            }
            let y = (x.u16(Camera::Y) as i16).wrapping_sub(4);
            x.set16(Camera::Y, if y > 256 { y as u16 } else { 256 });
            stage_script(x);
        }
        _ => {}
    }
}

/// 0x31822: the camera follows an object (`ram::LEADER`).
pub(super) fn screen_object(x: &mut Cam) {
    let obj = Obj::from_word(ram::LEADER.get(x.b));
    let v = obj.u16(x.b, Obj::X);
    track(x, Camera::X, (v, v), X_OBJECT);
    let v = obj.u16(x.b, Obj::Y);
    track(x, Camera::Y, (v, v), Y_OBJECT);
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
            let sum = player(0).obj().u16(x.b, Obj::X).wrapping_add(player(1).obj().u16(x.b, Obj::X)) >> 1;
            x.set16(Camera::X, sum.wrapping_sub(0xc0));
            follow_depth(x);
        }
        mode::WAVES => waves_and_follow(x),
        0x30 => camera_2_depth(x),
        0x34 => storm(x),
        mode::DUAL_FOLLOW => camera_2_follow(x),
        mode::WALL_RIGHT => {
            let target = if x.u16(Camera::TIMER) != 0 { 0x5a8 } else { 0x68 };
            advance_to(x, target, mode::GATE);
        }
        mode::GATE => gate(x),
        mode::PUSH => push(x),
        mode::PARKED => {
            if ram::PHASE.get(x.b) == 12 {
                x.add16(Camera::X, 1);
            }
            let px = x.u16(Camera::X);
            x.set16(Camera::DRAWN_X, px);
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
