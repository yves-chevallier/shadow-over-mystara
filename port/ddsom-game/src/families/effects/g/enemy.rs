use super::*;

// ---------------------------------------------------------------------------------------------
// Type 46: armor pieces of an enemy (created three at a time by `0xb53f4`)
// ---------------------------------------------------------------------------------------------

pub(super) mod armour {
    /// Carrier (ENEMY): the piece follows its position and replays its received hit (`+0x33`).
    pub const WEARER: u32 = 0x46;
    pub const PIECE: u32 = 0x2f;
    pub const LAST_HIT: u32 = 0x42;
    /// Carrier's life (`+0x62`) and starting life (`+0x60`): the piece falls when the life drops below
    /// a fraction of the starting life (quarter × (threshold + 1), threshold per piece, table `THRESHOLDS`).
    pub const LIFE: u32 = 0x62;
    pub const FULL_LIFE: u32 = 0x60;
    pub const HIT: u32 = 0x33;
    pub const WORN_ANIMS: crate::data::RomAddr = crate::data::RomAddr(0x195384);
    pub const SHARD_ANIMS: crate::data::RomAddr = crate::data::RomAddr(0x17efe8);
    pub const BOUNCE_TIMER: u32 = 0x5c;
    pub const FALLING: u32 = 0x05;
    pub const ROLLING: u32 = 0x06;
}

/// Common ending: clamp on screen, shadow, re-listing.
pub(super) fn on_screen<R: Bench>(x: &mut Cx<'_, R>) {
    x.keep_in_zone();
    x.push_terrain();
    x.reinsert();
}

pub(super) fn armour<R: Bench>(x: &mut Cx<'_, R>) {
    if x.suspended() {
        return;
    }
    if x.u8(f::VARIANT) != 0 {
        return armour_shard(x);
    }
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set8(armour::LAST_HIT, 0xff);
            x.anim_start(armour::WORN_ANIMS.0, 4);
        }
        2 => armour_worn(x),
        4 => armour_break(x),
        _ => free(x),
    }
}

/// Has the carrier's life dropped below the piece's threshold?
pub(super) fn armour_falls<R: Bench>(x: &mut Cx<'_, R>, wearer: Obj) -> bool {
    let quarter = wearer.u16(x.b, armour::FULL_LIFE) >> 2;
    let piece = x.u8(armour::PIECE) as u32;
    let steps = tables().armour_thresholds.at8(x.b, piece) as u16 + 1;
    let threshold = quarter.wrapping_mul(steps);
    threshold as i16 > wearer.u16(x.b, armour::LIFE) as i16
}

pub(super) fn armour_worn<R: Bench>(x: &mut Cx<'_, R>) {
    let link = x.u16(armour::WEARER);
    let wearer = Obj(link as i16 as i32 as u32 & 0xff_ffff);
    if (link as i16) >= 0 || wearer.u8(x.b, Obj::ACTIVE) == 0 || armour_falls(x, wearer) {
        return x.add8(f::STATE, 2);
    }
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = wearer.u16(x.b, off);
        x.set16(off, v);
    }
    let facing = wearer.u8(x.b, f::FACING);
    x.set8(f::FACING, facing);
    let hit = wearer.u8(x.b, armour::HIT) & 0x7f;
    if hit == 0 {
        set_hidden(x, true);
        return on_screen(x);
    }
    if hit == x.u8(armour::LAST_HIT) {
        return on_screen(x);
    }
    // new hit: the piece plays the hit animation
    x.set8(armour::LAST_HIT, hit);
    set_hidden(x, false);
    let piece = x.u8(armour::PIECE) as u32;
    let set = tables().armour_anim_sets.at32(x.b, 4 * piece);
    x.anim_start(set, (hit as u32) as u16);
}

/// The piece falls: it breaks into shards (variant 1) placed on it.
pub(super) fn armour_break<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let shards = &tables().armour_shards;
    let mut at = 4 * x.u8(armour::PIECE) as u32;
    let count = shards.at8(x.b, at);
    at += 1;
    let mut n = count as u16;
    loop {
        let Some(o) = x.alloc(&pools::EFFCT) else { return };
        o.set_u8(x.b, Obj::ACTIVE, 1);
        o.set_u8(x.b, Obj::KIND, 46);
        o.set_u8(x.b, Obj::VARIANT, 1);
        let kind = shards.at8(x.b, at);
        at += 1;
        o.set_u8(x.b, armour::PIECE, kind);
        for off in [Obj::X, Obj::Y, Obj::Z] {
            let v = x.u16(off);
            o.set_u16(x.b, off, v);
        }
        let facing = x.u8(f::FACING);
        o.set_u8(x.b, f::FACING, facing);
        o.set_u16(x.b, armour::WEARER, x.o.0 as u16);
        n = n.wrapping_sub(1);
        if n == 0xffff {
            return;
        }
    }
}

/// Allocation of an effect: the slot, or `None` if the pool is full.
pub(super) fn armour_shard<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => armour_shard_init(x),
        2 => armour_shard_active(x),
        4 => x.add8(f::STATE, 2),
        _ => free(x),
    }
}

pub(super) fn armour_shard_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set8(f::SUB_STATE, 0);
    let launch = &tables().armour_shard_launch;
    let sheet = ((x.u8(armour::PIECE) as u16) << 4) as u32;
    let mut dx = launch.at16(x.b, sheet);
    let dy = launch.at16(x.b, sheet + 2);
    x.add16(Obj::Y, dy);
    let dz = launch.at16(x.b, sheet + 4);
    x.add16(Obj::Z, dz);
    let mut vx = launch.at16(x.b, sheet + 6);
    x.set16(Obj::AX, 0);
    let vy = launch.at16(x.b, sheet + 8);
    x.set16(Obj::VY, vy);
    x.set16(Obj::AY, 0);
    let vz = launch.at32(x.b, sheet + 10);
    x.set32(Obj::VZ, vz);
    if x.u8(f::FACING) == 0 {
        dx = dx.wrapping_neg();
        vx = vx.wrapping_neg();
    }
    x.add16(Obj::X, dx);
    x.set16(Obj::VX, vx);
    x.anim_start(armour::SHARD_ANIMS.0, 9);
}

/// New bounce: vertical speed and gravity, randomly drawn delay before stopping.
pub(super) fn armour_shard_bounce<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(armour::ROLLING, 0);
    x.set16(Obj::VZ, 0x380);
    x.set16(Obj::AZ, 0xff80);
    let r = rng(x) & 0x1f;
    x.set8(armour::BOUNCE_TIMER, r as u8);
}

pub(super) fn armour_shard_active<R: Bench>(x: &mut Cx<'_, R>) {
    if vars::PAUSE.get(x.b) != 0 {
        return on_screen(x);
    }
    if x.u8(armour::FALLING) == 0 {
        x.move_xyz();
        if (x.u16(Obj::Z) as i16) < 0 {
            // first contact with the ground
            x.set16(Obj::Z, 0);
            x.add8(armour::FALLING, 2);
            x.set8(armour::ROLLING, 0);
            armour_shard_bounce(x);
            return on_screen(x);
        }
    }
    if x.u8(armour::ROLLING) == 0 {
        x.anim_step();
        x.move_z();
        if (x.u16(Obj::Z) as i16) >= 0 {
            x.anim_step();
            return on_screen(x);
        }
        x.set16(Obj::Z, 0);
        x.add8(armour::ROLLING, 2);
    }
    let t = x.u8(armour::BOUNCE_TIMER).wrapping_sub(1);
    x.set8(armour::BOUNCE_TIMER, t);
    if (t as i8) < 0 {
        // next bounce (no clamp or re-listing on this frame)
        return armour_shard_bounce(x);
    }
    x.anim_step();
    on_screen(x);
}

// ---------------------------------------------------------------------------------------------
// Type 48: flying machine of an enemy and its cannons (created by `0xb59a6`, `0xb59c8`)
// ---------------------------------------------------------------------------------------------

pub(super) mod flyer {
    /// Followed object (`+0x46`); variant 0 follows the object followed by its followed object.
    pub const LINK: u32 = 0x46;
    /// Phase of the following (`+0xa4` of the carrier): 1 for variant 1, 3 for variant 2.
    pub const PHASE: u32 = 0xa4;
    pub const ARMED: u32 = 0x42;
    pub const COUNTDOWN: u32 = 0x43;
    /// Variante 1 : trajet circulaire, table de (dx, dy) en `+0x4c`, angle en `+0x5c`, position relative
    /// saved in `+0x18` / `+0x1a`.
    pub const PATH: u32 = 0x4c;
    pub const ANGLE: u32 = 0x5c;
    pub const REL_X: u32 = 0x18;
    pub const REL_Y: u32 = 0x1a;
    /// Variant 2: a player on the same depth (`+0x2f` = 1).
    pub const PLAYER_IN_LINE: u32 = 0x2f;
    pub const ANIMS: crate::data::RomAddr = crate::data::RomAddr(0x17f002);
    /// Enemy shot (ESHL 0x2c) released every fourth frame.
    pub const SHOT: u8 = 0x2c;
}

/// The followed object, if it exists and is active (`0xb5bf0`).
pub(super) fn flyer_link<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
    let p = x.u16(flyer::LINK);
    let o = Obj(p as i16 as i32 as u32 & 0xff_ffff);
    ((p as i16) < 0 && o.u8(x.b, Obj::ACTIVE) != 0).then_some(o)
}

pub(super) fn flyer<R: Bench>(x: &mut Cx<'_, R>) {
    if x.suspended() {
        return;
    }
    let state = x.u8(f::STATE);
    match state {
        4 => return x.add8(f::STATE, 2),
        6 => return free(x),
        _ => {}
    }
    match (x.u8(f::VARIANT), state) {
        (0, 0) => {
            x.add8(f::STATE, 2);
            x.anim_start(flyer::ANIMS.0, 0xe);
        }
        (0, _) => flyer_body(x),
        (1, 0) => flyer_gun_init(x),
        (1, _) => flyer_gun(x),
        (_, 0) => {
            x.add8(f::STATE, 2);
            let Some(link) = flyer_link(x) else { return x.add8(f::STATE, 2) };
            let facing = link.u8(x.b, f::FACING);
            x.set8(f::FACING, facing);
            x.set8(flyer::ARMED, 0);
            x.anim_start(flyer::ANIMS.0, 8);
        }
        _ => flyer_sweeper(x),
    }
}

/// Variant 0: body placed on the object followed by the followed object, blinking every other frame.
pub(super) fn flyer_body<R: Bench>(x: &mut Cx<'_, R>) {
    if vars::PAUSE.get(x.b) != 0 {
        return x.reinsert();
    }
    let Some(link) = flyer_link(x) else { return x.add8(f::STATE, 2) };
    let p = link.u16(x.b, flyer::LINK);
    let host = Obj(p as i16 as i32 as u32 & 0xff_ffff);
    if (p as i16) >= 0 || host.u8(x.b, Obj::ACTIVE) == 0 {
        return x.add8(f::STATE, 2);
    }
    for off in [Obj::X, Obj::Y, Obj::Z] {
        let v = host.u16(x.b, off);
        x.set16(off, v);
    }
    let facing = host.u8(x.b, f::FACING);
    x.set8(f::FACING, facing);
    x.add16(Obj::X, if facing != 0 { 0xfffa } else { 6 });
    x.add16(Obj::Y, 0xffff);
    x.add16(Obj::Z, 0x54);
    if vars::FRAME_LOW.get(x.b) & 1 != 0 {
        x.set8(f::SUB_STATE, 0);
        set_hidden(x, true);
    } else {
        x.anim_step();
        set_hidden(x, false);
    }
    x.reinsert();
}

/// Enemy shot (ESHL 0x2c) of variant `variant`, every fourth frame (`0xb5d56`, `0xb5d82`).
pub(super) fn flyer_shoot<R: Bench>(x: &mut Cx<'_, R>, variant: u8) {
    if vars::FRAME_LOW.get(x.b) & 3 != 0 {
        return;
    }
    let Some(o) = x.alloc(&pools::ESHL) else { return };
    o.set_u8(x.b, Obj::ACTIVE, 1);
    o.set_u8(x.b, Obj::KIND, flyer::SHOT);
    o.set_u8(x.b, Obj::VARIANT, variant);
    o.set_u8(x.b, flyer::PLAYER_IN_LINE, 0);
    o.set_u16(x.b, flyer::LINK, x.o.0 as u16);
}

pub(super) fn flyer_gun_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let Some(link) = flyer_link(x) else { return x.add8(f::STATE, 2) };
    let facing = link.u8(x.b, f::FACING);
    x.set8(f::FACING, facing);
    // its body (variant 0)
    if let Some(o) = x.alloc(&pools::EFFCT) {
        o.set_u8(x.b, Obj::ACTIVE, 1);
        o.set_u8(x.b, Obj::KIND, 48);
        o.set_u8(x.b, Obj::VARIANT, 0);
        o.set_u8(x.b, flyer::PLAYER_IN_LINE, 0);
        o.set_u16(x.b, flyer::LINK, x.o.0 as u16);
    }
    x.set8(flyer::ARMED, 0);
    x.set32(flyer::PATH, tables().flyer_path.rom);
    let rel = if x.u8(f::FACING) != 0 { 0xff40 } else { 0xc0 };
    x.set16(flyer::REL_X, rel);
    x.set16(flyer::ANGLE, 0x1000);
    x.set16(flyer::REL_Y, 0);
}

/// End of the phase: disarmed, the effect leaves; armed, it waits `COUNTDOWN` frames.
pub(super) fn flyer_wind_down<R: Bench>(x: &mut Cx<'_, R>, keep_while_positive: bool) -> bool {
    if x.u8(flyer::ARMED) as i8 <= 0 {
        x.add8(f::STATE, 2);
        return false;
    }
    let t = x.u8(flyer::COUNTDOWN).wrapping_sub(1);
    x.set8(flyer::COUNTDOWN, t);
    if t as i8 > 0 {
        return keep_while_positive;
    }
    x.add8(f::STATE, 2);
    false
}

/// Variant 1: cannon that turns around the followed object as long as it is in phase 1, and fires.
pub(super) fn flyer_gun<R: Bench>(x: &mut Cx<'_, R>) {
    if vars::PAUSE.get(x.b) != 0 {
        return;
    }
    let Some(link) = flyer_link(x) else { return x.add8(f::STATE, 2) };
    if x.u8(f::SUB_STATE) != 0 {
        flyer_wind_down(x, true);
        return;
    }
    if link.u8(x.b, flyer::PHASE) != 1 {
        x.set8(flyer::COUNTDOWN, 0x20);
        return x.add8(f::SUB_STATE, 2);
    }
    // 0xb5c48: one step on the circle (table of 32 positions, angle in 1/256 in the high byte)
    let angle = ((x.u8(flyer::ANGLE) as u16 & 0x1f) << 2) as u32;
    let path = x.u32(flyer::PATH);
    for (pos, rel, step) in [(Obj::X, flyer::REL_X, 0u32), (Obj::Y, flyer::REL_Y, 2)] {
        let r = x.u16(rel);
        x.set16(pos, r);
        let d = held16(x.b, path.wrapping_add(angle + step)) as i16 as i32 as u32;
        x.add32(pos, d << 8);
        let v = x.u16(pos);
        x.set16(rel, v);
        let base = link.u16(x.b, pos);
        x.add16(pos, base);
    }
    let turn = if x.u8(f::FACING) != 0 { 0xff80 } else { 0x80 };
    x.add16(flyer::ANGLE, turn);
    flyer_shoot(x, 0);
    x.keep_on_screen(0x10);
}

/// Variant 2: sweeper placed at the screen edge, at the depth of the followed object (phase 3), which fires.
pub(super) fn flyer_sweeper<R: Bench>(x: &mut Cx<'_, R>) {
    if vars::PAUSE.get(x.b) != 0 {
        return x.add8(f::STATE, 2);
    }
    let Some(link) = flyer_link(x) else { return x.add8(f::STATE, 2) };
    if x.u8(f::SUB_STATE) != 0 {
        if flyer_wind_down(x, true) {
            flyer_sweep(x, link);
        }
        return;
    }
    if link.u8(x.b, flyer::PHASE) != 3 {
        x.set8(flyer::COUNTDOWN, 0x20);
        return x.add8(f::SUB_STATE, 2);
    }
    flyer_sweep(x, link);
    flyer_shoot(x, 1);
}

/// `0xb5cce`: at the screen edge (side depending on the orientation), at the depth of the followed
/// object; notes whether
/// a player present is on the same line (within 8 pixels).
pub(super) fn flyer_sweep<R: Bench>(x: &mut Cx<'_, R>, link: Obj) {
    x.set8(flyer::PLAYER_IN_LINE, 0);
    let y = link.u16(x.b, Obj::Y);
    x.set16(Obj::Y, y);
    let camera = if crate::ram::CAMERA_ALT_ACTIVE.get(x.b) != 0 { 0x556 } else { 0x506 };
    let mut px = x.a5_u16(camera);
    if x.u8(f::FACING) != 0 {
        px = px.wrapping_add(0x1c0);
    }
    x.set16(Obj::X, px);
    x.set16(Obj::Z, 0x20);
    for k in 0..4 {
        let p = crate::ram::PLAYERS.nth(k);
        if p.u8(x.b, Obj::ACTIVE) == 0 {
            continue;
        }
        let d = p.u16(x.b, Obj::Y).wrapping_sub(y).wrapping_add(8);
        if d < 0x10 {
            x.set8(flyer::PLAYER_IN_LINE, 1);
        }
    }
}

