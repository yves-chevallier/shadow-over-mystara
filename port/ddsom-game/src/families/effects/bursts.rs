use super::*;

// ---------------------------------------------------------------------------------------------
// Type 33: rubble that falls back (0xb2cca)

/// Type 33: burst thrown from a screen position (variant 0 to 3, speeds drawn from a table), drawn in the foreground.
/// It falls back; on touching the ground, the non-zero variants disappear, variant 0 (outside pause) creates two enemy
/// shots (ESHL 0x17, modes 0 and 1) before disappearing.
pub(super) mod shard {
    pub const ANIMATIONS: crate::data::RomAddr = crate::data::RomAddr(0x193ffe);
    pub const OFFSET: u32 = 0x5c;
    pub const CREATE_SHOT_A: crate::sound::Sound = crate::sound::CMD_8C;
}

pub(super) fn shard_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let rng = ctx.random();
    let draw = rng & 0x3f;
    o.set_u16(ctx.b, shard::OFFSET, draw);
    let flip = o.u8(ctx.b, field::FLIP) ^ 0x20;
    o.set_u8(ctx.b, field::FLIP, flip);
    let rank = (o.u8(ctx.b, Obj::VARIANT) as u32) << 2;
    let dx = tables().shard_positions.at16(ctx.b, rank);
    let dz = tables().shard_positions.at16(ctx.b, rank + 2);
    o.set_u16(ctx.b, Obj::Y, 0x40);
    let (x, z) = (crate::ram::CAMERA_ALT_X.get(ctx.b), crate::ram::CAMERA_ALT_Y.get(ctx.b));
    o.set_u16(ctx.b, Obj::X, x);
    o.set_u16(ctx.b, Obj::Z, z);
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Z, dz);
    let rank = rank << 1;
    for (i, off) in [Obj::VX, Obj::VZ, Obj::AZ].into_iter().enumerate() {
        let v = tables().shard_speeds.at16(ctx.b, rank + 2 * i as u32);
        o.set_u16(ctx.b, off, v);
    }
    let az = o.u16(ctx.b, Obj::AZ).wrapping_neg();
    o.set_u16(ctx.b, Obj::AZ, az);
    ctx.anim_start(shard::ANIMATIONS.0, 0);
    ctx.reinsert_foreground();
}

/// Landing (`0xb2d92`): disappearance, or two shots for variant 0.
pub(super) fn shard_lands<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if !(variant != 0) {
        let pause = vars::FROZEN.get(ctx.b) != 0;
        if !pause {
            crate::sound::play(ctx.b, shard::CREATE_SHOT_A, ctx.obj());
            let shot = ctx.alloc(&pools::ESHL);
            if shot.is_none() {
                return;
            }
            let shot = shot.unwrap();
            shot.set_u8(ctx.b, Obj::ACTIVE, 1);
            shot.set_u8(ctx.b, Obj::KIND, 0x17);
            shot.set_u8(ctx.b, field::MODE, 0);
            crate::families::effects_creators::set_list(ctx.b, 3);
            let shot = ctx.alloc(&pools::ESHL);
            if shot.is_none() {
                return;
            }
            let shot = shot.unwrap();
            shot.set_u8(ctx.b, Obj::ACTIVE, 1);
            shot.set_u8(ctx.b, Obj::KIND, 0x17);
            shot.set_u8(ctx.b, field::MODE, 1);
        }
    }
    ctx.free(&pools::EFFCT);
}

pub(super) fn shard_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ctx.anim_step();
    let in_flight = ctx.move_xz().pl(); // bpl
    if in_flight {
        return;
    }
    shard_lands(ctx, o)
}

pub(super) fn shard<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => shard_init(ctx, o),
        state::ACTIVE => shard_active(ctx, o),
        state::FREED => shard_lands(ctx, o),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => bad_index(state as u32),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 29: impact spark on a target (0xb1440)

/// Type 29: spark stuck to a target (`+0x5c`, enemy or boss) at the impact point drawn from a table according to the
/// target's type; it plays an animation (drawn at random according to a flag `+0x42` read from a table per target type)
/// and disappears at the end of the animation. If the target no longer exists or does not fit, the spark goes directly
/// to state 6 (release).
pub(super) mod impact {
    pub const ANIMATIONS: crate::data::RomAddr = crate::data::RomAddr(0x1908d6);
    pub const TARGET: u32 = 0x5c;
    pub const VARIANT_RANDOM: u32 = 0x42;
}

/// Sets state 6 (release on the next frame).
pub(super) fn impact_abandon<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    o.set_u32(ctx.b, Obj::STATE, 0x0600_0000);
}

/// `0xb14dc`: reads the target's flag from the tables, or gives up.
pub(super) fn impact_flag<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let pointer_target = o.u16(ctx.b, impact::TARGET);
    if pointer_target == 0 {
        return impact_abandon(ctx, o);
    }
    let target = pointer(ctx, o, impact::TARGET);
    let word = target.u16(ctx.b, Carrier::STATE_WORD);
    if word == 0x402 {
        return impact_abandon(ctx, o);
    }
    let class = target.u8(ctx.b, Obj::CLASS);
    let rank = (target.u8(ctx.b, Obj::KIND) as u32) * 2;
    if !(class != 2) {
        let forbidden = tables().impact_enemies.at8(ctx.b, rank);
        if forbidden != 0 {
            return impact_abandon(ctx, o);
        }
        let flag = tables().impact_enemies.at8(ctx.b, rank + 1);
        return o.set_u8(ctx.b, impact::VARIANT_RANDOM, flag);
    }
    if class != 4 {
        return impact_abandon(ctx, o);
    }
    let forbidden = tables().impact_boss.at8(ctx.b, rank);
    if forbidden != 0 {
        return impact_abandon(ctx, o);
    }
    let flag = tables().impact_boss.at8(ctx.b, rank + 1);
    o.set_u8(ctx.b, impact::VARIANT_RANDOM, flag);
}

/// `0xb15c8` (non-zero variant): position of the target offset by `(+0x14, +0x16)`.
pub(super) fn impact_position_shifted<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let pointer_target = o.u16(ctx.b, impact::TARGET);
    if pointer_target == 0 {
        return;
    }
    let target = pointer(ctx, o, impact::TARGET);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = target.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    let flip = target.u8(ctx.b, field::FLIP);
    o.set_u8(ctx.b, field::FLIP, flip);
    let (dx, dz) = (o.u16(ctx.b, Obj::VX), o.u16(ctx.b, Obj::AX));
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Z, dz);
}

/// `0xb15fe` (variant 0): position of the target offset by the impact drawn from the table.
pub(super) fn impact_position_table<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let pointer_target = o.u16(ctx.b, impact::TARGET);
    if pointer_target == 0 {
        return;
    }
    let target = pointer(ctx, o, impact::TARGET);
    for off in [Obj::X, Obj::Y, Obj::Z] {
        let v = target.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    let flip = target.u8(ctx.b, field::FLIP);
    o.set_u8(ctx.b, field::FLIP, flip);
    let class = target.u8(ctx.b, Obj::CLASS);
    let table = if class == 2 { &tables().impact_offsets_enemy } else { &tables().impact_offsets_other };
    let rank = ((target.u8(ctx.b, Obj::KIND) << 2) as u32) & 0xff;
    let mut dx = table.at16(ctx.b, rank);
    let dz = table.at16(ctx.b, rank + 2);
    let turned = target.u8(ctx.b, field::FLIP) != 0;
    if turned {
        dx = dx.wrapping_neg();
    }
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Z, dz);
}

pub(super) fn impact_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if !(variant == 0) {
        impact_position_shifted(ctx, o);
        o.set_u8(ctx.b, impact::VARIANT_RANDOM, 0);
    } else {
        impact_position_table(ctx, o);
    }
    impact_flag(ctx, o);
    let rng = ctx.random();
    let draw = rng & 0xf;
    let flag = o.u8(ctx.b, impact::VARIANT_RANDOM);
    let table = if flag == 0 { &tables().impact_random_a } else { &tables().impact_random_b };
    let animation = table.at8(ctx.b, draw as u32);
    ctx.anim_start(impact::ANIMATIONS.0, animation as u16);
}

pub(super) fn impact_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let end = ctx.anim_step();
    if end {
        add_u8(ctx, o, Obj::STATE, 2);
    }
    ctx.reinsert();
}

pub(super) fn impact_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => impact_init(ctx, o),
        state::ACTIVE => impact_active(ctx, o),
        state::FREED | 6 => ctx.free(&pools::EFFCT),
        _ => bad_index(state as u32),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 6: burst of debris (0xab7fa)

/// Type 6: debris thrown from an owner (`+0x50`). The first object (`+0x42` zero) creates as many siblings as indicated
/// by a table (according to the variant) then becomes a debris itself; each debris takes the owner's position, reads
/// its speed, orientation, offset and animation from a ROM sheet (variant, mode `+0x2f`), rises and falls back
/// blinking, then disappears.
pub(super) mod debris {
    pub const ANIMATIONS: crate::data::RomAddr = crate::data::RomAddr(0x18e664);
    pub const CHILD: u32 = 0x42;
    pub const BLINK: u32 = 0x53;
}

/// `0xab7d8`: creates a debris (type 6, variant `variant`) attached to the current object.
pub(super) fn created_debris<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, variant: u8) -> Option<Obj> {
    let new = ctx.alloc(&pools::EFFCT);
    // two successive `bcs`: in `0xab7d8`, then in the caller (the carry stayed set)
    let n = new?;
    n.set_u8(ctx.b, Obj::ACTIVE, 1);
    n.set_u8(ctx.b, Obj::KIND, 6);
    n.set_u8(ctx.b, Obj::VARIANT, variant);
    n.set_u8(ctx.b, debris::CHILD, 0);
    n.set_u16(ctx.b, field::OWNER, o.0 as u16);
    Some(n)
}

/// First debris: creates its siblings, then goes to the common initialization.
pub(super) fn debris_parent<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    o.set_u8(ctx.b, debris::CHILD, 1);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let count = tables().debris_numbers.at8(ctx.b, variant as u32);
    o.set_u8(ctx.b, field::MODE, count.wrapping_add(1));
    let owner = o.u16(ctx.b, field::OWNER);
    for remaining in (0..=count as u16).rev() {
        let Some(sibling) = created_debris(ctx, o, variant) else {
            break;
        };
        sibling.set_u16(ctx.b, field::OWNER, owner);
        sibling.set_u8(ctx.b, debris::CHILD, 1);
        sibling.set_u8(ctx.b, field::MODE, remaining as u8);
    }
    debris_init(ctx, o);
}

pub(super) fn debris_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let child = o.u8(ctx.b, debris::CHILD);
    if child == 0 {
        return debris_parent(ctx, o);
    }
    add_u8(ctx, o, Obj::STATE, 2);
    let owner = pointer(ctx, o, field::OWNER);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = owner.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    o.set_u8(ctx.b, debris::BLINK, 0);
    o.set_u16(ctx.b, Obj::AX, 0);
    let variant = o.u8(ctx.b, Obj::VARIANT) as u32;
    let offset = tables().debris_sheets.at16(ctx.b, 2 * variant) as i16 as i32 as u32;
    let mode = (o.u8(ctx.b, field::MODE) as u32) << 4;
    // the sheets follow the offsets in the same table
    let sheets = &tables().debris_sheets;
    let sheet = offset.wrapping_add(mode);
    let vx = sheets.at16(ctx.b, sheet);
    o.set_u16(ctx.b, Obj::VX, vx);
    let vz_az = sheets.at32(ctx.b, sheet + 2);
    o.set_u32(ctx.b, Obj::VZ, vz_az);
    let flip = sheets.at8(ctx.b, sheet + 6);
    o.set_u8(ctx.b, field::FLIP, flip);
    let animation = sheets.at8(ctx.b, sheet + 7);
    let (mut dx, dz, dy) = (sheets.at16(ctx.b, sheet + 8), sheets.at16(ctx.b, sheet + 10), sheets.at16(ctx.b, sheet + 12));
    let turned = owner.u8(ctx.b, field::FLIP) != 0;
    if turned {
        let flip = o.u8(ctx.b, field::FLIP) ^ 0x20;
        o.set_u8(ctx.b, field::FLIP, flip);
        let vx = o.u16(ctx.b, Obj::VX).wrapping_neg();
        o.set_u16(ctx.b, Obj::VX, vx);
        dx = dx.wrapping_neg();
        o.set_u8(ctx.b, debris::BLINK, 1);
    }
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Z, dz);
    shifted_u16(ctx, o, 0x12, dy);
    ctx.anim_start(debris::ANIMATIONS.0, animation as u16);
}

pub(super) fn debris_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ctx.move_xz();
    ctx.anim_step();
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
    let blinks = o.u8(ctx.b, debris::BLINK) ^ 1;
    o.set_u8(ctx.b, debris::BLINK, blinks);
    if !(blinks == 0) {
        let flags = o.u8(ctx.b, Obj::FLAGS);
        o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    }
    let z = o.u16(ctx.b, Obj::Z);
    if !(z as i16 > 0) {
        o.set_u32(ctx.b, Obj::Z, 0);
        o.set_u8(ctx.b, debris::BLINK, 6);
        add_u8(ctx, o, Obj::STATE, 2);
        let variant = o.u8(ctx.b, Obj::VARIANT);
        if !(variant == 0) {
            add_u8(ctx, o, debris::BLINK, 0xff);
        }
    }
    ctx.reinsert();
}

pub(super) fn debris_end<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
    let remainder = o.u8(ctx.b, debris::BLINK).wrapping_sub(1);
    o.set_u8(ctx.b, debris::BLINK, remainder);
    if remainder == 0 {
        return ctx.free(&pools::EFFCT);
    }
    if !(remainder & 1 == 0) {
        let flags = o.u8(ctx.b, Obj::FLAGS);
        o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    }
    ctx.reinsert();
}

pub(super) fn debris_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => debris_init(ctx, o),
        state::ACTIVE => debris_active(ctx, o),
        state::FREED => debris_end(ctx, o),
        6 => ctx.free(&pools::EFFCT),
        _ => bad_index(state as u32),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 1: removed effect (0xa8758)

/// Type 1: unused table slot in this version of the game: the handler frees the object immediately (`bra` to `jmp
/// 0x2218`).
pub(super) fn removed<R: Bench>(ctx: &mut Cx<'_, R>) {
    ctx.free(&pools::EFFCT);
}

// ---------------------------------------------------------------------------------------------
// Type 7: single-animation burst (0xaba4e)

/// Type 7: plays the animation (drawn according to the mode `+0x2f`); at the end of the animation it rises by two units
/// in depth then blinks for 0x1e frames before disappearing. Not exercised.
pub(super) mod shard_single {
    pub const COUNT: u32 = 0x53;
}

pub(super) fn shard_single_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let mode = o.u8(ctx.b, field::MODE) as u32;
    let animation = tables().shard_single_animations_mode.at8(ctx.b, mode);
    ctx.anim_start(0x1bf8fe, animation as u16);
    ctx.reinsert();
}

pub(super) fn shard_single_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let end = o.u8(ctx.b, Obj::ANIM_FLAGS);
    if !(end == 0) {
        add_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, shard_single::COUNT, 0x1e);
        let y = o.u32(ctx.b, Obj::Y).wrapping_add(2);
        o.set_u32(ctx.b, Obj::Y, y);
    }
    ctx.anim_step();
}

pub(super) fn shard_single_blinks<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let remainder = o.u8(ctx.b, shard_single::COUNT).wrapping_sub(1);
    o.set_u8(ctx.b, shard_single::COUNT, remainder);
    if remainder == 0 {
        return ctx.free(&pools::EFFCT);
    }
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags ^ 2);
}

pub(super) fn shard_single_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => shard_single_init(ctx, o),
        state::ACTIVE => shard_single_active(ctx, o),
        state::FREED => shard_single_blinks(ctx, o),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => bad_index(state as u32),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 62: burst of fragments (0xb77ac)

/// Type 62: takes the variant and mode of the object it follows (`+0x5c`), places itself 0x48 to the left of it and
/// waits for it to be hit (`+0x33` non-zero); it then explodes into nine EFFCT 0x40 objects (and a tenth described by
/// the mode) then disappears. Not exercised.
pub(super) mod sheaf {
    pub const ANIMATIONS: crate::data::RomAddr = crate::data::RomAddr(0x197a5c);
    pub const CARRIER: u32 = 0x5c;
}

pub(super) fn sheaf_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let carrier = pointer(ctx, o, sheaf::CARRIER);
    let variant = carrier.u8(ctx.b, Obj::VARIANT);
    o.set_u8(ctx.b, Obj::VARIANT, variant);
    let mode = carrier.u8(ctx.b, field::MODE);
    o.set_u8(ctx.b, field::MODE, mode);
    for off in [Obj::X, Obj::Y] {
        let v = carrier.u16(ctx.b, off);
        o.set_u16(ctx.b, off, v);
    }
    let x = o.u16(ctx.b, Obj::X).wrapping_sub(0x48);
    o.set_u16(ctx.b, Obj::X, x);
    o.set_u16(ctx.b, Obj::Z, 0x40);
    ctx.anim_start(sheaf::ANIMATIONS.0, 0);
    o.set_u8(ctx.b, Effect::FRAME_ATTRIBUTE, 0);
    ctx.reinsert();
}

/// `0xb7810`: the explosion.
pub(super) fn sheaf_explodes<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let mut last = None;
    for _ in 0..9 {
        let new = ctx.alloc(&pools::EFFCT);
        if new.is_none() {
            return ctx.free(&pools::EFFCT);
        }
        let n = new.unwrap();
        n.set_u8(ctx.b, Obj::ACTIVE, 1);
        n.set_u8(ctx.b, Obj::KIND, 0x40);
        for off in [Obj::X, Obj::Y] {
            let v = o.u16(ctx.b, off);
            n.set_u16(ctx.b, off, v);
        }
        last = Some(n);
    }
    let mode = o.u8(ctx.b, field::MODE);
    if !(mode == 0) {
        // the last object created is reused to carry the mode
        let n = last.unwrap();
        n.set_u8(ctx.b, Obj::ACTIVE, 1);
        n.set_u8(ctx.b, Obj::KIND, 0x40);
        n.set_u8(ctx.b, Obj::VARIANT, 1);
        let variant = o.u8(ctx.b, Obj::VARIANT);
        n.set_u8(ctx.b, field::MODE, variant);
        for off in [Obj::X, Obj::Y] {
            let v = o.u16(ctx.b, off);
            n.set_u16(ctx.b, off, v);
        }
    }
    ctx.free(&pools::EFFCT);
}

pub(super) fn sheaf_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => sheaf_init(ctx, o),
        state::ACTIVE => {
            let carrier = pointer(ctx, o, sheaf::CARRIER);
            let key = carrier.u8(ctx.b, Effect::ANIM_EVENT);
            if key == 0 {
                return;
            }
            sheaf_explodes(ctx, o);
        }
        state::FREED => sheaf_explodes(ctx, o),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => bad_index(state as u32),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 56: debris fragments (0xb6c6e)

/// Type 56: burst thrown into the air from its position (random scatter of ±128 in X and ±32 in depth, speeds drawn
/// from two tables, animation 0x164 to 0x167); disappears when it falls back below the ground. Not exercised.
pub(super) mod shards_debris {
}

/// Masked random draw.
pub(super) fn draw<R: Bench>(ctx: &mut Cx<'_, R>, mask: u16) -> u16 {
    let rng = ctx.random();
    rng & mask
}

pub(super) fn shards_debris_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    o.set_u16(ctx.b, Effect::TIMER, 0x40);
    o.set_u8(ctx.b, Effect::OWN_PALETTE, 1);
    o.set_u8(ctx.b, Effect::PALETTE, 0x1d);
    let flip = draw(ctx, 0x20);
    o.set_u8(ctx.b, field::FLIP, flip as u8);
    let dx = draw(ctx, 0xff).wrapping_sub(0x80);
    shifted_u16(ctx, o, Obj::X, dx);
    let dy = draw(ctx, 0x3f).wrapping_sub(0x20);
    shifted_u16(ctx, o, Obj::Y, dy);
    o.set_u16(ctx.b, Obj::VZ, 0x400);
    o.set_u16(ctx.b, Obj::AZ, 0xffc0);
    let r = draw(ctx, 0x1c) as u32;
    let vx = tables().shards_debris_speeds_x.at16(ctx.b, r);
    o.set_u16(ctx.b, Obj::VX, vx);
    let r = draw(ctx, 0x1c) as u32;
    let vy = tables().shards_debris_speeds_y.at16(ctx.b, r);
    o.set_u16(ctx.b, Obj::VY, vy);
    let animation = draw(ctx, 3).wrapping_add(0x164);
    ctx.anim_start(ANIMATIONS_INTERFACE.0, animation);
    ctx.reinsert();
}

pub(super) fn shards_debris_active<R: Bench>(ctx: &mut Cx<'_, R>, _o: Obj) {
    let positive = ctx.move_xyz().pl(); // bpl
    if positive {
        return;
    }
    ctx.free(&pools::EFFCT);
}

pub(super) fn shards_debris_end<R: Bench>(ctx: &mut Cx<'_, R>, _o: Obj) {
    ctx.free(&pools::EFFCT);
}

pub(super) fn shards_debris_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => shards_debris_init(ctx, o),
        state::ACTIVE => shards_debris_active(ctx, o),
        state::FREED => shards_debris_end(ctx, o),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => bad_index(state as u32),
    }
}

