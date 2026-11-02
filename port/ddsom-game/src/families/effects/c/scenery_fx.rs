use super::*;

// ---------------------------------------------------------------------------------------------
// Type 42: scene effects (eleven subtypes according to the variant)
// ---------------------------------------------------------------------------------------------

pub(super) mod scenery {
    /// Variant 1: per setting, 16-byte sheet (dx, dy, vx, ax, vy, ay; data).
    pub const THROWS_1: u32 = 0xb4202;
    /// Variant 2: per setting, offset (dx, dy) (data); pushed with the players.
    pub const PLACES_2: u32 = 0xb4394;
    /// Variant 9: per setting (`& 7`), 8-byte sheet (dx, dz as signed bytes, then vz, az; program).
    pub const SPRAY: u32 = 0xb4a32;
    pub const ANIM_SPRAY: u32 = 0x17b9d6;
    /// Variant 0: brief spark (animation 0xf, or 0x10 flipped), animation set 0x174776.
    pub const ANIM_FLASH: u32 = 0x174776;
    /// Followed object (16-bit pointer, negative when it exists).
    pub const TARGET: u32 = 0x46;
    pub const ANIM_DEBRIS: u32 = 0x1948ea;
    pub const ANIM_DUST: u32 = 0x1901ba;
    pub const ANIM_GLOW: u32 = 0x17e048;
    /// Speeds (vz, az) per mode `+0x2f`: variant 3 (data), variant 4 (data).
    pub const FALL_3: u32 = 0xb4480;
    pub const FALL_4: u32 = 0xb45ba;
    /// X speeds of variant 4 (program space, random even index).
    pub const DRIFT_4: u32 = 0xb45aa;
    /// Spawn zones of variant 8: (x, mask, y, mask) per mode (data).
    pub const AREAS_8: u32 = 0xb4986;
    /// Variant 10: animation (bytes) and speeds (vx, ax) per mode (program space).
    pub const ANIMS_10: u32 = 0xb4af2;
    pub const SPEEDS_10: u32 = 0xb4b56;
    /// Sprite frames of the glow (variant 7), longs.
    pub const GLOW_FRAMES: u32 = 0x17efca;
    /// History of the followed object's positions (variant 5): 8 longs (x, y + z) starting at `+8`.
    pub const TRAIL: u32 = 0x08;
    pub const TRAIL_WRITE: u32 = 0x06;
    pub const TRAIL_READ: u32 = 0x07;
    /// Alternate camera (`A5+0x556`, `A5+0x55a`).
    pub const ALT_CAMERA_X: u32 = 0x556;
    pub const ALT_CAMERA_Y: u32 = 0x55a;
}

/// The game is paused (`A5+0x3d`).
pub(super) fn paused<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.a5_u8(0x3d) != 0
}

/// The followed object, if it exists (negative pointer), with no activity test.
pub(super) fn scenery_target<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
    let p = x.u16(scenery::TARGET);
    ((p as i16) < 0).then_some(Obj(p as i16 as i32 as u32 & 0xff_ffff))
}

/// The followed object, if it exists and is active.
pub(super) fn scenery_live_target<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
    scenery_target(x).filter(|t| t.is_active(x.b))
}

pub(super) fn reinsert_foreground<R: Bench>(x: &mut Cx<'_, R>) {
    x.reinsert_foreground();
}

pub(super) fn scenery<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(f::VARIANT);
    match x.u8(f::STATE) {
        0 => match v {
            0 => flash_init(x),
            1 | 2 => thrown_init(x, v),
            9 => spray_init(x),
            3 => debris_init(x),
            4 => shard_init(x),
            5 => follow_init(x),
            6 => spark_init(x),
            7 => glow_init(x),
            8 => puff_init(x),
            10 => dust_init(x),
            _ => {}
        },
        2 => match v {
            0 => flash_active(x),
            1 | 2 => thrown_active(x, v),
            9 => spray_active(x),
            3 => debris_active(x),
            4 => shard_active(x),
            5 => follow_active(x),
            6 => spark_active(x),
            7 => glow_active(x),
            8 => puff_active(x),
            10 => dust_active(x),
            _ => {}
        },
        4 => x.add8(f::STATE, 2),
        6 => free(x),
        _ => {}
    }
}

/// Variants 1 and 2 (0xb4194, 0xb4342): placed on the followed object, offset according to setting `+0x2f`;
/// variant 1 is thrown (speeds from the sheet, data in `THROWS_1`), variant 2 placed on the ground and
/// pushed back with the players (`0x81aa4`). Disappears at the end of its animation.
pub(super) fn thrown_init<R: Bench>(x: &mut Cx<'_, R>, v: u8) {
    x.add8(f::STATE, 2);
    let Some(target) = scenery_target(x) else { return x.set8(f::STATE, 6) };
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let w = target.u16(x.b, off);
        x.set16(off, w);
    }
    let facing = target.u8(x.b, 0x2c);
    x.set8(0x2c, facing);
    let mode = x.u8(0x2f) as u32;
    if v == 1 {
        let at = scenery::THROWS_1.wrapping_add(((mode as u16) << 4) as i16 as i32 as u32);
        let (mut dx, dy, mut vx, mut ax) = (x.data16(at), x.data16(at + 2), x.data16(at + 4), x.data16(at + 6));
        if facing == 0 {
            dx = dx.wrapping_neg();
            vx = vx.wrapping_neg();
            ax = ax.wrapping_neg();
        }
        x.add16(Obj::X, dx);
        x.add16(Obj::Y, dy);
        x.set16(Obj::Z, 0);
        x.set16(Obj::VX, vx);
        x.set16(Obj::AX, ax);
        let (vy, ay) = (x.data16(at + 8), x.data16(at + 10));
        x.set16(Obj::VY, vy);
        x.set16(Obj::AY, ay);
        x.set32(Obj::VZ, 0);
    } else {
        let at = scenery::PLACES_2.wrapping_add(((mode as u16) << 2) as i16 as i32 as u32);
        let (mut dx, dy) = (x.data16(at), x.data16(at + 2));
        if facing == 0 {
            dx = dx.wrapping_neg();
        }
        x.add16(Obj::X, dx);
        x.add16(Obj::Y, dy);
        x.set16(Obj::Z, 0);
    }
    x.anim_start(scenery::ANIM_DUST, 0);
}

pub(super) fn thrown_active<R: Bench>(x: &mut Cx<'_, R>, v: u8) {
    if !paused(x) {
        if v == 1 {
            x.move_xy();
        } else {
            crate::families::boss_common::push_out_of_boss(x);
        }
        if x.anim_step() {
            return x.add8(f::STATE, 4);
        }
    }
    x.push_terrain();
    x.reinsert();
}

/// Variant 9 (0xb49c4): spark thrown at random (x speed drawn, sign according to the setting), which falls back and
/// disappears on the ground; it blinks every other frame.
pub(super) fn spray_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set16(Obj::AX, 0);
    x.set16(Obj::AY, 0);
    let k = 8 * (x.u8(0x2f) & 7) as u32;
    let dx = x.rom8(scenery::SPRAY + k) as i8 as i16 as u16;
    x.add16(Obj::X, dx);
    let dz = x.rom8(scenery::SPRAY + k + 1) as i8 as i16 as u16;
    x.add16(Obj::Z, dz);
    x.set32(Obj::VY, 0);
    let (vz, az) = (x.rom16(scenery::SPRAY + k + 4), x.rom16(scenery::SPRAY + k + 6));
    x.set16(Obj::VZ, vz);
    x.set16(Obj::AZ, az);
    let rng = x.random();
    let mut vx = rng & 0x3ff;
    if x.u8(0x2f) & 1 != 0 {
        vx = vx.wrapping_neg();
    }
    x.set16(Obj::VX, vx);
    x.anim_start(scenery::ANIM_SPRAY, 0x19);
}

pub(super) fn spray_active<R: Bench>(x: &mut Cx<'_, R>) {
    if !paused(x) {
        x.anim_step();
        x.move_xyz();
        if (x.u16(Obj::Z) as i16) < 0 {
            x.set16(Obj::Z, 0);
            return x.add8(f::STATE, 4);
        }
        let blink = x.a5_u16(0x6e) & 1 != 0;
        x.set_bit(0x01, 1, blink);
    }
    x.reinsert();
}

/// Variant 0 (0xb411e): one pixel in front, just above the ground (or below if `+0x2f`).
pub(super) fn flash_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.add16(Obj::Y, 0xffff);
    x.set16(Obj::Z, 1);
    let anim = if x.u8(0x2f) != 0 {
        x.add16(Obj::Z, 0xfffe);
        x.add16(Obj::Y, 2);
        0x10
    } else {
        0xf
    };
    x.anim_start(scenery::ANIM_FLASH, anim);
}

pub(super) fn flash_active<R: Bench>(x: &mut Cx<'_, R>) {
    if !paused(x) && x.anim_step() {
        return scenery_done(x);
    }
    x.reinsert();
}

/// 0xb4bb2: position and orientation of the followed object.
pub(super) fn copy_target<R: Bench>(x: &mut Cx<'_, R>, target: Obj) {
    copy_position(x, target);
    let facing = target.u8(x.b, 0x2c);
    x.set8(0x2c, facing);
}

/// End of animation: goes straight to release (`addq.b #2` twice).
pub(super) fn scenery_done<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 4);
}

/// `subq.b #1` on counter `+0x42`; false (and goes to release) when it runs out.
pub(super) fn scenery_countdown<R: Bench>(x: &mut Cx<'_, R>) -> Option<u8> {
    let old = x.u8(0x42);
    if (old as i8) <= 1 {
        scenery_done(x);
        return None;
    }
    let n = old - 1;
    x.set8(0x42, n);
    Some(n)
}

// variant 3: debris that falls and bounces

pub(super) fn debris_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let Some(target) = scenery_target(x) else { return x.set8(f::STATE, 6) };
    copy_target(x, target);
    let mode = x.u8(0x2f) as u32;
    let (vz, az) = (x.data16(scenery::FALL_3 + 4 * mode), x.data16(scenery::FALL_3 + 4 * mode + 2));
    x.set16(Obj::VZ, vz);
    x.set16(Obj::AZ, az);
    let px = x.u16(Obj::X).wrapping_sub(0x80).wrapping_add((mode << 4) as u16);
    x.set16(Obj::X, px);
    let y = crate::ram::CAMERA_Y.get(x.b).wrapping_add(0x20);
    x.set16(Obj::Y, y);
    x.set16(Obj::Z, 0);
    x.anim_start(scenery::ANIM_DEBRIS, 0);
}

pub(super) fn debris_active<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return reinsert_foreground(x);
    }
    x.move_z();
    if x.u8(f::SUB_STATE) == 0 {
        let vz = x.u16(Obj::VZ) as i16;
        if vz < 0 {
            // bounce: speed halved, braking by a quarter
            x.add8(f::SUB_STATE, 2);
            x.set8(0x42, 0x20);
            let vz = vz >> 1;
            x.set16(Obj::VZ, vz as u16);
            x.set16(Obj::AZ, (vz.wrapping_neg() >> 2) as u16);
        }
        x.anim_step();
        return;
    }
    let Some(n) = scenery_countdown(x) else { return };
    set_hidden(x, n & 1 == 0);
    reinsert_foreground(x);
}

// variant 4: spark that glides then blinks

pub(super) fn shard_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let Some(target) = scenery_target(x) else { return x.set8(f::STATE, 6) };
    copy_target(x, target);
    x.set16(Obj::Z, 0xffb0);
    let rng = x.random();
    let k = rng as u32 & 0xe;
    let vx = x.rom16(scenery::DRIFT_4 + k);
    x.set16(Obj::VX, vx);
    x.set16(Obj::AX, 0);
    let mode = x.u8(0x2f) as u32;
    let (vz, az) = (x.data16(scenery::FALL_4 + 4 * mode), x.data16(scenery::FALL_4 + 4 * mode + 2));
    x.set16(Obj::VZ, vz);
    x.set16(Obj::AZ, az);
    let px = x.u16(Obj::X).wrapping_sub(0x40).wrapping_add((mode << 5) as u16);
    x.set16(Obj::X, px);
    let y = crate::ram::CAMERA_Y.get(x.b).wrapping_add(0x20);
    x.set16(Obj::Y, y);
    x.set16(Obj::Z, 0);
    x.anim_start(scenery::ANIM_DEBRIS, 3);
}

pub(super) fn shard_active<R: Bench>(x: &mut Cx<'_, R>) {
    if !paused(x) {
        x.move_xz();
        if x.u8(f::SUB_STATE) == 0 {
            if x.u8(0x33) != 0 {
                x.add8(f::SUB_STATE, 2);
                x.set8(0x42, 0x20);
                let vz = (x.u16(Obj::VZ) as i16) >> 2;
                x.set16(Obj::VZ, vz as u16);
                x.set16(Obj::AZ, 0);
            }
            x.anim_step();
        } else if scenery_countdown(x).is_none() {
            return;
        }
    }
    // every other frame according to the parity of `+0x58`
    let hidden = (x.a5_u16(0x6e) as u8 & 1) ^ x.u8(0x58) == 0;
    set_hidden(x, hidden);
    reinsert_foreground(x);
}

// variant 5: alternate camera that follows an object with eight frames of delay

/// 0xb4686: fills the history with the followed object's current position.
pub(super) fn follow_fill<R: Bench>(x: &mut Cx<'_, R>, target: Obj) {
    let (px, depth) = (target.u16(x.b, Obj::X), target.u16(x.b, Obj::Z).wrapping_add(target.u16(x.b, Obj::Y)));
    for k in 0..8 {
        x.set16(scenery::TRAIL + 4 * k, px);
        x.set16(scenery::TRAIL + 4 * k + 2, depth);
    }
}

/// 0xb4784: alternate camera at rest, then release.
pub(super) fn follow_end<R: Bench>(x: &mut Cx<'_, R>) {
    follow_rest(x);
    x.add8(f::STATE, 2);
}

pub(super) fn follow_rest<R: Bench>(x: &mut Cx<'_, R>) {
    let y = if crate::ram::SCENE.get(x.b) == 0x2e { 0 } else { 0x500 };
    x.set_a5_u16(scenery::ALT_CAMERA_X, 0);
    x.set_a5_u16(scenery::ALT_CAMERA_Y, y);
}

pub(super) fn follow_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set8(scenery::TRAIL_WRITE, 7);
    x.set8(scenery::TRAIL_READ, 0);
    match scenery_target(x) {
        Some(target) if x.u8(0) != 0 => follow_fill(x, target),
        _ => follow_end(x),
    }
}

pub(super) fn follow_active<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    let Some(target) = scenery_live_target(x) else { return follow_end(x) };
    // b46a2: notes the current position; b46cc: reads back the one from eight frames ago
    let w = (x.u8(scenery::TRAIL_WRITE) as u32 + 1) & 7;
    x.set8(scenery::TRAIL_WRITE, w as u8);
    let (px, depth) = (target.u16(x.b, Obj::X), target.u16(x.b, Obj::Z).wrapping_add(target.u16(x.b, Obj::Y)));
    x.set16(scenery::TRAIL + 4 * w, px);
    x.set16(scenery::TRAIL + 4 * w + 2, depth);
    let r = (x.u8(scenery::TRAIL_READ) as u32 + 1) & 7;
    x.set8(scenery::TRAIL_READ, r as u8);
    let (old_x, old_depth) = (x.u16(scenery::TRAIL + 4 * r), x.u16(scenery::TRAIL + 4 * r + 2));
    let (big_x, big_y) = if target.u8(x.b, 2) == 0x14 { (0x440, 0x400) } else { (0x230, 0x320) };
    let cam_x = crate::ram::CAMERA_X.get(x.b).wrapping_add(0xc0).wrapping_sub(old_x).wrapping_add(big_x);
    let mut cam_y = crate::ram::CAMERA_Y.get(x.b).wrapping_add(0x40).wrapping_sub(old_depth).wrapping_add(big_y);
    let limit: u16 = if crate::ram::SCENE.get(x.b) == 0x2e { 0x2c0 } else { 0x3c0 };
    if cam_y as i16 > limit as i16 {
        cam_y = limit;
    }
    x.set_a5_u16(scenery::ALT_CAMERA_X, cam_x);
    x.set_a5_u16(scenery::ALT_CAMERA_Y, cam_y);
    if target.u8(x.b, 0x42) != 0 {
        follow_fill(x, target);
        follow_rest(x);
    }
}

// variant 6: spark around an object

pub(super) fn spark_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let Some(target) = scenery_live_target(x) else { return x.set8(f::STATE, 6) };
    let rng = x.random();
    let px = (rng & 0x1ff).wrapping_sub(0xff).wrapping_add(target.u16(x.b, Obj::X));
    x.set16(Obj::X, px);
    let rng = x.random();
    let y = 0x1fu16.wrapping_sub(rng & 0x7f).wrapping_add(target.u16(x.b, Obj::Y));
    x.set16(Obj::Y, y);
    x.anim_start(scenery::ANIM_DUST, 0);
}

pub(super) fn spark_active<R: Bench>(x: &mut Cx<'_, R>) {
    if !paused(x) {
        if let Some(target) = scenery_live_target(x) {
            // 0x930a4: boss 15's approach, toward the target (A1)
            crate::families::boss_15::attacks::approach_toward(x, target);
        }
        if x.anim_step() {
            return scenery_done(x);
        }
    }
    x.reinsert();
}

// variant 7: glow on an object of type 0x19 (according to its frame `+0x33`)

pub(super) fn glow_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let Some(target) = scenery_live_target(x) else { return x.add8(f::STATE, 4) };
    let facing = target.u8(x.b, 0x2c);
    x.set8(0x2c, facing);
    x.anim_start(scenery::ANIM_GLOW, 0x20);
}

pub(super) fn glow_active<R: Bench>(x: &mut Cx<'_, R>) {
    if !paused(x) {
        let target = scenery_live_target(x)
            .filter(|t| t.u8(x.b, 2) == 0x19 && t.u16(x.b, 0xa0) == 0x225);
        let Some(target) = target else { return scenery_done(x) };
        set_hidden(x, true);
        let frame = target.u8(x.b, 0x33) as u32;
        if frame != 0 {
            let sprite = x.data32(scenery::GLOW_FRAMES + 4 * frame);
            x.set32(0x34, sprite);
            let (px, z, y) = (target.u16(x.b, Obj::X), target.u16(x.b, Obj::Z), target.u16(x.b, Obj::Y));
            x.set16(Obj::X, px);
            x.set16(Obj::Z, z.wrapping_add(9));
            x.set16(Obj::Y, y.wrapping_sub(1));
            set_hidden(x, false);
        }
    }
    x.reinsert();
}

// variant 8: puff drawn at random in a zone of the screen

pub(super) fn puff_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let area = scenery::AREAS_8 + 8 * x.u8(0x2f) as u32;
    for (field, origin, k) in [(Obj::X, 0x506, 0), (Obj::Y, 0x50a, 4)] {
        let mut v = x.a5_u16(origin).wrapping_add(x.data16(area + k));
        let rng = x.random();
        let mask = x.data16(area + k + 2);
        v = v.wrapping_add(rng & mask);
        let rng = x.random();
        v = v.wrapping_add(rng & 0xf);
        x.set16(field, v);
    }
    let rng = x.random();
    x.anim_start(scenery::ANIM_DEBRIS, (rng & 1) + 1);
}

pub(super) fn puff_active<R: Bench>(x: &mut Cx<'_, R>) {
    if !paused(x) && x.anim_step() {
        return scenery_done(x);
    }
    x.reinsert();
}

// variant 10: dust that crosses the screen; mode 0 creates the others

pub(super) fn dust_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let mode = x.u8(0x2f) as u32;
    if mode == 0 {
        let rng = x.random();
        let mut dx = rng & 0x1ff;
        if dx as i16 > 0x190 {
            dx -= 0x70;
        }
        let px = dx.wrapping_add(crate::ram::CAMERA_X.get(x.b));
        x.set16(Obj::X, px);
        let rng = x.random();
        let y = (rng & 0x7f).wrapping_add(crate::ram::CAMERA_Y.get(x.b)).wrapping_add(0x20);
        x.set16(Obj::Y, y);
        x.set8(Obj::Z, 0);
        dust_spawn(x);
    }
    let (vx, ax) = (x.rom16(scenery::SPEEDS_10 + 4 * mode), x.rom16(scenery::SPEEDS_10 + 4 * mode + 2));
    x.set16(Obj::VX, vx);
    x.set16(Obj::AX, ax);
    x.set32(0x18, 0);
    x.set32(0x1c, 0);
    let anim = x.rom8(scenery::ANIMS_10 + mode) as u32;
    x.anim_start(scenery::ANIM_DUST, anim as u16);
}

/// 0xb40ba: fills the effect pool with dust of mode 1 at the same position.
pub(super) fn dust_spawn<R: Bench>(x: &mut Cx<'_, R>) {
    while let Some(o) = x.alloc(&pools::EFFCT) {
        wr8(x.b, o.0, 1);
        wr8(x.b, o.0 + 2, 42);
        wr8(x.b, o.0 + 3, 10);
        wr8(x.b, o.0 + 0x2f, 1);
        for field in [Obj::X, Obj::Z, Obj::Y] {
            let v = x.u16(field);
            wr16(x.b, o.0 + field, v);
        }
    }
}

pub(super) fn dust_active<R: Bench>(x: &mut Cx<'_, R>) {
    if !paused(x) {
        if x.anim_step() {
            return scenery_done(x);
        }
        x.move_xyz();
        let dx: u16 = if x.a5_u16(0x6e) & 1 != 0 { 0xc0u16.wrapping_neg() } else { 0xc0 };
        x.add16(Obj::X, dx);
    }
    x.keep_in_zone();
    x.push_terrain();
    x.reinsert();
}

