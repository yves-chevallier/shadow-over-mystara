use super::*;

// ---------------------------------------------------------------------------------------------
// Type 6: scene objects (0xdc624)

/// Type 6, scene objects attached to a player (`+0x50`). Four variants, three automata:
///
/// - variants 0 and 2, "player icon" (h): follows the player (player position offset by
///   0x20 pixels, or 0x18 if flipped, one pixel in front of it, 0x18 above), plays a randomly
///   drawn animation (4) and a sound every 0x3c frames (outside pause `A5+0x42`);
/// - variant 1, "guide" (h): chooses the closest living player, walks toward a point 0x20
///   pixels from it through a speed table, stops, creates SET objects (`0xc8312`) and drives the
///   counter `A5-0x55c8`;
/// - variant 3: see the original code (not exercised, delegated).
///
/// Common states: 0 initialization, 2 active, 4 end (variant 1), 6 release. Substates 2 and 4 of the icon (flight,
/// pickup) and state 4 of the icon are not exercised: they stay in original code.
pub const ENTRY: u32 = 0xdc624;
pub const TABLE_VARIANTS: u32 = 0xdc63c;
pub const ANIMATIONS: u32 = 0x1bf8fe;
// icon
pub const ICON_STATES: u32 = 0xdc654;
pub const ICON_SUB_STATES: u32 = 0xdc6ac;
pub const ICON_ANIMATIONS: u32 = 0xdc69c;
pub const ICON_SOUNDS: u32 = 0xdc6b2;
pub const ICON_TAKEOFF: u32 = 0xdc7a0;
pub const OWNER_2: u32 = 0x46;
pub const OFFSETS_X: u32 = 0xdcdd8;
pub const DELAY_SOUND: u8 = 0x3c;
// guide
pub const GUIDE_STATES: u32 = 0xdcb30;
pub const GUIDE_SUB_STATES: u32 = 0xdcb92;
pub const GUIDE_ANIMATIONS: u32 = 0xdcb6e;
pub const GUIDE_ANIMATIONS_ARRIVAL: u32 = 0xdcbbc;
pub const TABLE_SPEEDS: u32 = 0x1c65b6;
/// Champs.
pub const DELAY: u32 = 0x4c;
pub const TARGET_X: u32 = 0x52;
pub const TARGET_Y: u32 = 0x54;
pub const COUNTDOWN_TIMER: u32 = 0x56;
pub const CREATED: u32 = 0x5c;
/// Scene variables.
pub const PAUSE: i32 = 0x42;
pub const SCENE_FROZEN: i32 = 0x3d;
pub const LEVEL: i32 = -0x5ac6;
pub const COUNTER: i32 = -0x55c8;
pub const PLAYERS: u32 = 0xff85de;
pub const STEP_PLAYER: u32 = 0x200;
// subroutines not rewritten

/// `0xdcdae`: places itself next to the owner (one pixel in front, offset according to the orientation). Returns the
/// owner.
pub(super) fn follow_owner<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> Obj {
    let player = pointer(ctx, o, field::OWNER);
    let x = player.u32(ctx.b, Obj::X);
    o.set_u32(ctx.b, Obj::X, x);
    let y = player.u32(ctx.b, Obj::Y);
    o.set_u32(ctx.b, Obj::Y, y);
    let y = o.u16(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u16(ctx.b, Obj::Y, y);
    let turned = o.u8(ctx.b, 0x2c) != 0;
    let offset = if !turned { 0 } else { 2 };
    let dx = ctx.prog16(scene::OFFSETS_X + offset);
    let x = o.u16(ctx.b, Obj::X).wrapping_add(dx);
    o.set_u16(ctx.b, Obj::X, x);
    player
}

pub(super) fn icon_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, field::SUB_STATE, 0);
    let player = follow_owner(ctx, o);
    let z = player.u32(ctx.b, Obj::Z);
    o.set_u32(ctx.b, Obj::Z, z);
    let z = o.u16(ctx.b, Obj::Z).wrapping_add(0x18);
    o.set_u16(ctx.b, Obj::Z, z);
    o.set_u8(ctx.b, scene::DELAY, scene::DELAY_SOUND);
    o.set_u8(ctx.b, Obj::ACTIVE, 2);
    let rng = ctx.random();
    let draw = rng & 3;
    o.set_u8(ctx.b, field::MODE, draw as u8);
    let animation = ctx.prog8(scene::ICON_ANIMATIONS + draw as u32);
    ctx.anim_start(scene::ANIMATIONS, animation as u16);
}

/// `0xdc6e8`: every 0x3c frames (outside pause), plays the icon's sound drawn from the table (`base`: 0 or 0x10
/// depending on the caller, first or second series of sounds).
pub(super) fn icon_sound<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, base: u32) {
    let paused = ctx.ram8(scene::PAUSE) != 0;
    if paused {
        return;
    }
    let remainder = o.u8(ctx.b, scene::DELAY).wrapping_sub(1);
    o.set_u8(ctx.b, scene::DELAY, remainder);
    if remainder != 0 {
        return;
    }
    o.set_u8(ctx.b, scene::DELAY, scene::DELAY_SOUND);
    let draw = o.u8(ctx.b, field::MODE) as u32;
    let sound = ctx.prog32(scene::ICON_SOUNDS + base + draw * 4);
    crate::sound::play(ctx.b, crate::sound::Sound::at(sound), ctx.obj());
}

/// Substate 0 of the icon: follows the player, animates, sorts, and plays the sound drawn every 0x3c frames.
pub(super) fn icon_followed<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    follow_owner(ctx, o);
    ctx.anim_step();
    o.set_u8(ctx.b, 0x3b, 0);
    ctx.reinsert();
    icon_sound(ctx, o, 0);
}

/// Flight speeds (`0xdc7c2`): flipped with the orientation, then animation step.
pub(super) fn icon_takeoff_speed<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let flip = o.u8(ctx.b, field::FLIP) != 0;
    if flip {
        let v = o.u16(ctx.b, Obj::VX).wrapping_neg();
        o.set_u16(ctx.b, Obj::VX, v);
    }
    ctx.anim_step();
}

/// Substate 2 of the icon: the icon flies off in an arc (four steps, according to `+6`).
pub(super) fn icon_takeoff<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let time = o.u8(ctx.b, field::SUB_SUB_STATE);
    let target = crate::families::effects::target(ctx, scene::ICON_TAKEOFF, time as u32);
    match time {
        0 | 4 => {
            add_u8(ctx, o, field::SUB_SUB_STATE, 2);
            o.set_u16(ctx.b, Obj::VX, 0x200);
            o.set_u16(ctx.b, Obj::VZ, if time == 0 { 0x400 } else { 0x200 });
            o.set_u16(ctx.b, Obj::AZ, 0xff80);
            o.set_u16(ctx.b, Obj::AX, 0);
            icon_takeoff_speed(ctx, o);
        }
        2 => {
            ctx.move_xz();
            let z = o.u16(ctx.b, Obj::Z);
            if !(z as i16 > 0) {
                add_u8(ctx, o, field::SUB_SUB_STATE, 2);
                o.set_u32(ctx.b, Obj::Z, 0);
            }
            ctx.anim_step();
        }
        6 => {
            ctx.move_xz();
            let z = o.u16(ctx.b, Obj::Z);
            if !(z as i16 > 0) {
                add_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
                o.set_u8(ctx.b, field::SUB_SUB_STATE, 0);
                o.set_u32(ctx.b, Obj::Z, 0);
                o.set_u8(ctx.b, Obj::ACTIVE, 1);
                o.set_u16(ctx.b, scene::COUNTDOWN_TIMER, 0x12c);
            }
            ctx.anim_step();
        }
        _ => crate::families::effects::outside_table(target),
    }
}

/// Substate 4 of the icon: waits for `+0x58`, replays its sound, then disappears when the delay `+0x56` has elapsed (or
/// returns to following if the owner resumes).
pub(super) fn icon_wait<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let pickup = o.u8(ctx.b, field::PICKUP);
    if pickup == 1 {
        let pause = ctx.ram8(scene::PAUSE) != 0;
        if pause {
            o.set_u8(ctx.b, field::PICKUP, 0);
        } else {
            add_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, field::SUB_STATE, 0);
            let owner = pointer(ctx, o, scene::OWNER_2);
            let variant = o.u8(ctx.b, Obj::VARIANT);
            if !(variant != 2) {
                let reserve = owner.u8(ctx.b, 0x31);
                if !(reserve == 0) {
                    o.set_u8(ctx.b, Obj::VARIANT, 0);
                }
            }
            add_u8(ctx, owner, 0x31, 1);
            return;
        }
    }
    icon_sound(ctx, o, 0x10);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if !(variant != 2) {
        let owner = pointer(ctx, o, scene::OWNER_2);
        let reserve = owner.u8(ctx.b, 0x31);
        if !(reserve == 0) {
            o.set_u16(ctx.b, scene::COUNTDOWN_TIMER, 1);
        }
    }
    let remainder = o.u16(ctx.b, scene::COUNTDOWN_TIMER).wrapping_sub(1);
    o.set_u16(ctx.b, scene::COUNTDOWN_TIMER, remainder);
    if !(remainder != 0) {
        o.set_u8(ctx.b, Obj::VARIANT, 0);
        o.set_u32(ctx.b, Obj::STATE, 0x0400_0000);
    }
    ctx.anim_step();
    ctx.reinsert();
}

/// State 4 of the icon (`0xdc9b8`): the icon leaves the scene. It departs (spark, sound, routine specific to its mode),
/// walks in X until it leaves the screen, or stops near the player, creates a SET and waits like the guide (`0xdcc5a`,
/// `0xdcc82`).
mod start {
    pub const SUB_STATES: u32 = 0xdc9c4;
    /// Per mode: routine called at departure (longs, 0 = none), departure animation, stop sound and animation.
    pub const ROUTINES: u32 = 0xdca50;
    pub const ANIMATIONS: u32 = 0xdca60;
    pub const SOUNDS_STOP: u32 = 0xdcaa0;
    pub const ANIMATIONS_STOP: u32 = 0xdca9c;
    pub const SOUND_STOP: crate::sound::Sound = crate::sound::CMD_4B;
    /// Icon counter of the object `+0x46`.
    pub const ICONS: u32 = 0x30;
    /// Stop requested (`+0x33`).
    pub const STOP: u32 = 0x33;
}

pub(super) fn start_launches<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    shard(ctx, o);
    add_u8(ctx, o, field::SUB_STATE, 2);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    add_u8(ctx, o, field::SUB_STATE, variant);
    o.set_u16(ctx.b, Obj::VX, 0x380);
    if o.u8(ctx.b, field::PICKUP) != 0 {
        o.set_u16(ctx.b, Obj::VX, 0x200);
        let player = pointer(ctx, o, field::OWNER);
        // the sound is played on the icon if the player is not in front of it
        if player.u16(ctx.b, Obj::Y) as i16 >= o.u16(ctx.b, Obj::Y) as i16 {
            o.set_u16(ctx.b, field::OWNER, o.0 as u16);
        }
        effect_pickup(ctx, o, 0xa);
        o.set_u16(ctx.b, field::OWNER, player.0 as u16);
        let mode = o.u8(ctx.b, field::MODE) as u32;
        // departure sound of the mode (0xe1da0, 0xe1d60), played on the icon
        let sound = ctx.prog32(start::ROUTINES + 4 * mode);
        if sound != 0 {
            crate::sound::play(ctx.b, crate::sound::Sound::at(sound), o);
        }
    }
    if o.u8(ctx.b, 0x2c) == 0 {
        let v = o.u16(ctx.b, Obj::VX).wrapping_neg();
        o.set_u16(ctx.b, Obj::VX, v);
    }
    let mode = o.u8(ctx.b, field::MODE) as u32;
    let animation = ctx.prog8(start::ANIMATIONS + mode);
    ctx.anim_start(scene::ANIMATIONS, animation as u16);
}

/// One scene object fewer for the object `+0x46`.
pub(super) fn start_countdown<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let carrier = pointer(ctx, o, scene::OWNER_2);
    let n = carrier.u8(ctx.b, start::ICONS).wrapping_sub(1);
    carrier.set_u8(ctx.b, start::ICONS, n);
}

/// Substate 2: walks in X; on leaving the screen, the icon is freed.
pub(super) fn start_running<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    if o.u8(ctx.b, start::STOP) == 0 {
        ctx.move_x();
        let gap = o.u16(ctx.b, Obj::X).wrapping_sub(crate::ram::CAMERA_X.get(ctx.b)).wrapping_add(0x20);
        if gap > 0x1c0 {
            o.set_u32(ctx.b, Obj::STATE, 0x0600_0000);
            start_countdown(ctx, o);
        }
    }
    ctx.anim_step();
}

/// Substate 4: stop requested: stop sound and animation, SET created, the icon turns toward the player.
pub(super) fn start_stop<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    if o.u8(ctx.b, start::STOP) != 0 {
        ctx.anim_step();
        return;
    }
    let mode = o.u8(ctx.b, field::MODE) as u32;
    let sound = ctx.prog8(start::SOUNDS_STOP + mode);
    if sound != 0 {
        crate::sound::play(ctx.b, start::SOUND_STOP, ctx.obj());
    }
    let animation = ctx.prog8(start::ANIMATIONS_STOP + mode);
    ctx.anim_start(scene::ANIMATIONS, animation as u16);
    add_u8(ctx, o, field::SUB_STATE, 2);
    start_countdown(ctx, o);
    o.set_u16(ctx.b, scene::CREATED, 0);
    if let Some(n) = created_set(ctx, 1) {
        o.set_u16(ctx.b, scene::CREATED, n.0 as u16);
    }
    let player = pointer(ctx, o, field::OWNER);
    if o.u16(ctx.b, Obj::X) <= player.u16(ctx.b, Obj::X) {
        o.set_u8(ctx.b, 0x2c, 0x20);
    }
    if o.u8(ctx.b, 0x2c) == player.u8(ctx.b, 0x2c) {
        let v = player.u8(ctx.b, 0x2c) ^ 0x20;
        player.set_u8(ctx.b, 0x2c, v);
    }
    ctx.anim_step();
}

pub(super) fn icon_start<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = crate::families::effects::target(ctx, start::SUB_STATES, sub_state as u32);
    match sub_state {
        0 => start_launches(ctx, o),
        2 => start_running(ctx, o),
        4 => start_stop(ctx, o),
        6 => guide_wait(ctx, o),
        8 => guide_second_object(ctx, o),
        0xa => {
            let set = pointer(ctx, o, scene::CREATED);
            if set.u8(ctx.b, 0x22) != 0 {
                add_u8(ctx, o, field::SUB_STATE, 2);
            }
        }
        0xc => {
            let n = ctx.ram8(scene::COUNTER).wrapping_add(4);
            ctx.set_ram8(scene::COUNTER, n);
        }
        _ => crate::families::effects::outside_table(target),
    }
}

pub(super) fn icon<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = crate::families::effects::target(ctx, scene::ICON_STATES, state as u32);
    match state {
        state::INIT => icon_init(ctx, o),
        state::ACTIVE => {
            let sub_state = o.u8(ctx.b, field::SUB_STATE);
            let target = crate::families::effects::target(ctx, scene::ICON_SUB_STATES, sub_state as u32);
            match sub_state {
                sub::FALL => icon_followed(ctx, o),
                2 => icon_takeoff(ctx, o),
                4 => icon_wait(ctx, o),
                _ => crate::families::effects::outside_table(target),
            }
        }
        4 => icon_start(ctx, o),
        6 => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

/// `0xdcddc`: places the guide at the right edge of the screen and chooses the closest living player, whose owner
/// (`+0x50`), height (`+0x54`) and an arrival point (`+0x52`) it retains. Without a living player, the original keeps
/// its caller's A2: `a2` (guide 3 leaves there the end of its palette copy; guide 1 inherits it from the previous
/// handlers, a value the register-free game does not hold: `None`).
pub(super) fn guide_target<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, a2: Option<u32>) {
    let cam_x = crate::ram::CAMERA_X.get(ctx.b);
    let cam_y = crate::ram::CAMERA_Y.get(ctx.b);
    o.set_u16(ctx.b, Obj::X, cam_x.wrapping_add(0x1a0));
    o.set_u16(ctx.b, Obj::Y, cam_y.wrapping_add(0x40));
    o.set_u32(ctx.b, Obj::Z, 0);
    o.set_u8(ctx.b, 0x2c, 0);
    let mut best: u16 = 0xffff;
    let mut chosen = None;
    for i in 0..4u32 {
        let player = Obj(scene::PLAYERS + i * scene::STEP_PLAYER);
        let active = player.u8(ctx.b, Obj::ACTIVE) != 0;
        let alive = active && {
            let state = player.u8(ctx.b, Obj::STATE) as i8;
            !(state >= 4)
        };
        if alive {
            let mut dx = o.u16(ctx.b, Obj::X).wrapping_sub(player.u16(ctx.b, Obj::X));
            if !(dx as i16 >= 0) {
                dx = dx.wrapping_neg();
            }
            let mut dy = o.u16(ctx.b, Obj::Y).wrapping_sub(player.u16(ctx.b, Obj::Y));
            if !(dy as i16 >= 0) {
                dy = dy.wrapping_neg();
            }
            let distance = dx.wrapping_add(dy);
            if !(best <= distance) {
                best = distance;
                chosen = Some(player.0);
            }
        }
    }
    // Original dead code (docs/code-mort.md, familles3): guide 1 is created by the scene script, with players present
    let chosen = chosen.or(a2).unwrap_or_else(|| unreachable!("guide 1 without a living player: A2 inherited from the previous handlers"));
    o.set_u16(ctx.b, field::OWNER, chosen as u16);
    let target = Obj(chosen);
    let y = target.u16(ctx.b, Obj::Y);
    o.set_u16(ctx.b, scene::TARGET_Y, y);
    let gap = target.u16(ctx.b, Obj::X).wrapping_sub(cam_x);
    let offset: u16 = if gap <= 0x140 { 0x20 } else { 0x20u16.wrapping_neg() };
    let x = target.u16(ctx.b, Obj::X).wrapping_add(offset);
    o.set_u16(ctx.b, scene::TARGET_X, x);
}

pub(super) fn guide_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, field::SUB_STATE, 0);
    guide_target(ctx, o, None);
    o.set_u32(ctx.b, Obj::SPEED_TABLE, scene::TABLE_SPEEDS);
    let level = ctx.ram8(scene::LEVEL) as u32;
    let animation = ctx.prog8(scene::GUIDE_ANIMATIONS + 2 * level);
    let mode = ctx.prog8(scene::GUIDE_ANIMATIONS + 2 * level + 1);
    o.set_u8(ctx.b, field::MODE, mode);
    ctx.anim_start(scene::ANIMATIONS, animation as u16);
}

/// `0xdcb9a`: animation of the guide according to its mode (0x12 for variant 3).
pub(super) fn guide_animation<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let mut animation = 0x12;
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if !(variant == 3) {
        let mode = o.u8(ctx.b, field::MODE) as u32;
        animation = ctx.prog8(scene::GUIDE_ANIMATIONS_ARRIVAL + mode);
    }
    ctx.anim_start(scene::ANIMATIONS, animation as u16);
}

/// Creates a SET object of type `type_set`; returns its address, or `None` if the pool is full.
pub(super) fn created_set<R: Bench>(ctx: &mut Cx<'_, R>, type_set: u32) -> Option<Obj> {
    scene_set_of(ctx, type_set as u8)
}

/// Substate 0 of the guide: walks toward the arrival point; once arrived, changes animation and creates a SET.
pub(super) fn guide_running<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let target_x = o.u16(ctx.b, scene::TARGET_X);
    let target_y = o.u16(ctx.b, scene::TARGET_Y);
    let direction = crate::aids::direction_from(ctx.b, o, (target_x, target_y));
    o.set_u8(ctx.b, Obj::SPEED_INDEX, direction);
    ctx.move_by_table_xy();
    let dx = target_x.wrapping_sub(o.u16(ctx.b, Obj::X)).wrapping_add(8);
    let dy = target_y.wrapping_sub(o.u16(ctx.b, Obj::Y)).wrapping_add(8);
    let arrive = !(dx > 0x10) && !(dy > 0x10);
    if arrive {
        add_u8(ctx, o, field::SUB_STATE, 2);
        guide_animation(ctx, o);
        o.set_u16(ctx.b, scene::COUNTDOWN_TIMER, 0x3c);
        o.set_u16(ctx.b, scene::CREATED, 0);
        let variant = o.u8(ctx.b, Obj::VARIANT);
        let type_set = if variant != 3 { 1 } else { 0x26 };
        if let Some(n) = created_set(ctx, type_set) {
            o.set_u16(ctx.b, scene::CREATED, n.0 as u16);
        }
        let owner = pointer(ctx, o, field::OWNER);
        let x = o.u16(ctx.b, Obj::X);
        let x_owner = owner.u16(ctx.b, Obj::X);
        if !(x > x_owner) {
            o.set_u8(ctx.b, 0x2c, 0x20);
        }
        let (a, b) = (o.u8(ctx.b, 0x2c), owner.u8(ctx.b, 0x2c));
        if !(a != b) {
            let v = owner.u8(ctx.b, 0x2c) ^ 0x20;
            owner.set_u8(ctx.b, 0x2c, v);
        }
    }
    ctx.anim_step();
}

/// Substate 2 of the guide: waits for the end of the timer or the disappearance of the SET object created.
pub(super) fn guide_wait<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let created = o.u16(ctx.b, scene::CREATED);
    if !(created == 0) {
        let set = pointer(ctx, o, scene::CREATED);
        let finished = set.u8(ctx.b, 0x22) != 0;
        if !finished {
            return;
        }
    } else {
        let remainder = o.u16(ctx.b, scene::COUNTDOWN_TIMER).wrapping_sub(1);
        o.set_u16(ctx.b, scene::COUNTDOWN_TIMER, remainder);
        if remainder != 0 {
            return;
        }
    }
    add_u8(ctx, o, field::SUB_STATE, 2);
    let n = ctx.ram8(scene::COUNTER).wrapping_add(4);
    ctx.set_ram8(scene::COUNTER, n);
}

/// Substate 4 of the guide: when the scene counter reaches 0x10, creates a second SET.
pub(super) fn guide_second_object<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let counter = ctx.ram8(scene::COUNTER);
    if counter != 0x10 {
        return;
    }
    o.set_u16(ctx.b, scene::CREATED, 0);
    if let Some(n) = created_set(ctx, 2) {
        add_u8(ctx, o, field::SUB_STATE, 2);
        o.set_u16(ctx.b, scene::CREATED, n.0 as u16);
        let mode = o.u8(ctx.b, field::MODE);
        n.set_u8(ctx.b, 0x42, mode);
    }
}

/// Substate 6 of the guide: waits for the end of the SET object, then goes to state 4.
pub(super) fn guide_end<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let set = pointer(ctx, o, scene::CREATED);
    let in_progress = set.u8(ctx.b, 0x22) == 0;
    if !in_progress {
        add_u8(ctx, o, Obj::STATE, 2);
    }
}

pub(super) fn guide_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let frozen = ctx.ram8(scene::SCENE_FROZEN) != 0;
    if !frozen {
        let sub_state = o.u8(ctx.b, field::SUB_STATE);
        let target = crate::families::effects::target(ctx, scene::GUIDE_SUB_STATES, sub_state as u32);
        match sub_state {
            0 => guide_running(ctx, o),
            2 => guide_wait(ctx, o),
            4 => guide_second_object(ctx, o),
            6 => guide_end(ctx, o),
            _ => crate::families::effects::outside_table(target),
        }
    }
    ctx.reinsert();
}

pub(super) fn guide<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = crate::families::effects::target(ctx, scene::GUIDE_STATES, state as u32);
    match state {
        state::INIT => guide_init(ctx, o),
        state::ACTIVE => guide_active(ctx, o),
        state::FREED => {
            let n = ctx.ram8(scene::COUNTER).wrapping_add(4);
            ctx.set_ram8(scene::COUNTER, n);
        }
        state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

/// Variant 3 (`0xdccd6`), the "guide" of an attract scene (h): loads a palette, then walks like the guide of variant 1
/// and waits for the end of its SET object (type 0x26), whose result (`+0x23`) it copies into `A5-0x551c`.
mod guide_3 {
    pub const STATES: u32 = 0xdcce4;
    pub const SUB_STATES: u32 = 0xdcd76;
    /// 16-color palette (data in the clear) and its destination.
    pub const PALETTE: u32 = 0xdccec;
    pub const PALETTE_DEST: u32 = 0x91c300;
    /// First color of palette 0: its brightness bits (0xf000f000) are copied.
    pub const REFERENCE: u32 = 0x91c000;
    pub const ANIMATION: u16 = 0x13;
    pub const RESULT: i32 = -0x551c;
}

pub(super) fn guide_3_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let light = crate::mem::rd32(ctx.b, guide_3::REFERENCE) & 0xf000_f000;
    for i in 0..8 {
        let colour = crate::mem::rd32(ctx.b, guide_3::PALETTE + 4 * i);
        // Original glitch: the original's `movem` passes colours 1 to 6 through D1-D6; D2-D5 stay for the next
        // handlers (`dispatch::Leftovers`)
        match i {
            2 => *ctx.c.d2() = colour,
            3 => *ctx.c.d3() = colour,
            4 => *ctx.c.d4() = colour,
            5 => *ctx.c.d5() = colour,
            _ => {}
        }
        crate::mem::wr32(ctx.b, guide_3::PALETTE_DEST + 4 * i, colour | light);
    }
    add_u8(ctx, o, Obj::STATE, 2);
    // A2: the end of the palette copy
    guide_target(ctx, o, Some(guide_3::PALETTE_DEST + 0x20));
    o.set_u32(ctx.b, Obj::SPEED_TABLE, scene::TABLE_SPEEDS);
    o.set_u8(ctx.b, 0x2a, 1);
    o.set_u8(ctx.b, 0x2b, 0x18);
    ctx.anim_start(scene::ANIMATIONS, guide_3::ANIMATION);
    ctx.reinsert();
}

/// Substate 2: waits for the end of the SET object (whose result it keeps) or of the timer.
pub(super) fn guide_3_wait<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    if o.u16(ctx.b, scene::CREATED) != 0 {
        let set = pointer(ctx, o, scene::CREATED);
        if set.u8(ctx.b, 0x22) == 0 {
            return;
        }
        let result = set.u8(ctx.b, 0x23);
        ctx.set_ram8(guide_3::RESULT, result);
    } else {
        let remainder = o.u16(ctx.b, scene::COUNTDOWN_TIMER).wrapping_sub(1);
        o.set_u16(ctx.b, scene::COUNTDOWN_TIMER, remainder);
        if remainder != 0 {
            return;
        }
    }
    add_u8(ctx, o, field::SUB_STATE, 2);
    let n = ctx.ram8(scene::COUNTER).wrapping_add(4);
    ctx.set_ram8(scene::COUNTER, n);
}

pub(super) fn guide_3<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = crate::families::effects::target(ctx, guide_3::STATES, state as u32);
    match state {
        state::INIT => guide_3_init(ctx, o),
        state::ACTIVE => {
            let sub_state = o.u8(ctx.b, field::SUB_STATE);
            let target = crate::families::effects::target(ctx, guide_3::SUB_STATES, sub_state as u32);
            match sub_state {
                0 => guide_running(ctx, o),
                2 => guide_3_wait(ctx, o),
                4 => {}
                _ => crate::families::effects::outside_table(target),
            }
        }
        state::FREED => {}
        state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

pub(super) fn scene_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let target = crate::families::effects::target(ctx, scene::TABLE_VARIANTS, 2 * variant as u32);
    match variant {
        0 | 2 => icon(ctx, o),
        1 => guide(ctx, o),
        3 => guide_3(ctx, o),
        _ => crate::families::effects::outside_table(target),
    }
}
