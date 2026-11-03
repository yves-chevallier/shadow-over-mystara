//! Choosing a point to walk to around the target (0xe76c, 0xe7fa, 0xe886, 0xe914, 0xeff2, 0xf084).

use super::*;

/// Tables of candidate points by number (`+0x79`): pointer, then number of points - 1 and pairs
/// (x offset, depth offset) around the target.
pub(crate) const WALK_POINTS: u32 = 0xe93e;
pub(super) const MIRRORED_WALK_POINTS: u32 = 0xf06a;

/// "City block" gap between two points: |dx| + |dy| (words).
pub(super) fn manhattan(b: &mut Cps2Bus, me: u32, x: u16, depth: u16) -> u16 {
    let dx = (x.wrapping_sub(rd16(b, me + f::X)) as i16).wrapping_abs() as u16;
    let dy = (depth.wrapping_sub(rd16(b, me + f::DEPTH)) as i16).wrapping_abs() as u16;
    dx.wrapping_add(dy)
}

/// The walking point retained: the nearest or the farthest from `o`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Pick {
    Nearest,
    Farthest,
}

/// Retains, among the points of the table of `o` (`+0x79`) around its target, the one that is nearest (on a
/// tie, the last) or farthest (on a tie, the first) from `o`, and stores it at `+0x7a`/`+0x7c` (the
/// target's position if none). `skip`: point number skipped without reading its pair (original glitch: the following
/// points take the shifted pair); the point retained is then noted at `+0x99`. `mirror`: x offsets
/// flipped if the target faces left. Returns the point retained.
pub(super) fn pick_point(b: &mut Cps2Bus, o: Obj, table: u32, pick: Pick, skip: Option<u8>, mirror: bool) -> (u16, u16) {
    let target = linked(o.u16(b, crate::ai::f::TARGET));
    let set = o.u8(b, g::POINT_SET) as u32;
    let mut at = rd32(b, table + 4 * set);
    let last = rd16(b, at);
    at += 2;
    let (target_x, target_depth) = (target.u16(b, Obj::X), target.u16(b, Obj::Y));
    o.set_u16(b, g::GOAL_X, target_x);
    o.set_u16(b, g::GOAL_DEPTH, target_depth);
    let mut best: u16 = if pick == Pick::Farthest { 0 } else { 0x7fff };
    for point in (0..=last).rev() {
        if skip == Some(point as u8) {
            continue;
        }
        let mut dx = rd16(b, at);
        let dy = rd16(b, at + 2);
        at += 4;
        if mirror && target.u8(b, f::FACING) != 0 {
            dx = dx.wrapping_neg();
        }
        let (x, depth) = (dx.wrapping_add(target_x), dy.wrapping_add(target_depth));
        let distance = manhattan(b, o.0, x, depth);
        let better = match pick {
            Pick::Farthest => distance > best,
            Pick::Nearest => distance <= best,
        };
        if better {
            best = distance;
            o.set_u16(b, g::GOAL_X, x);
            o.set_u16(b, g::GOAL_DEPTH, depth);
            if skip.is_some() {
                o.set_u8(b, g::LAST_POINT, point as u8);
            }
        }
    }
    (o.u16(b, g::GOAL_X), o.u16(b, g::GOAL_DEPTH))
}

/// Is `o` more than 384 pixels to the right of the left edge of the camera in service?
pub(super) fn far_from_camera(b: &mut Cps2Bus, o: Obj) -> bool {
    let (camera, _) = crate::ram::camera(b);
    o.u16(b, Obj::X).wrapping_sub(camera) > 0x180
}

/// 0xf084: aims at the farthest point around the target and turns one notch towards it.
pub fn walk_far_point(b: &mut Cps2Bus, o: Obj) {
    let goal = pick_point(b, o, WALK_POINTS, Pick::Farthest, None, false);
    crate::ai::turn_toward(b, o, goal);
}

/// 0xe76c: aims at the nearest point around the target (far from the camera: the farthest, 0xf084) and
/// turns one notch towards it. Nothing without a target.
pub fn approach_target(b: &mut Cps2Bus, o: Obj) {
    if o.u16(b, crate::ai::f::TARGET) == 0 {
        return;
    }
    if far_from_camera(b, o) {
        return walk_far_point(b, o);
    }
    let goal = pick_point(b, o, WALK_POINTS, Pick::Nearest, None, false);
    crate::ai::turn_toward(b, o, goal);
}

/// 0xeff2: like `approach_target` (without the far case), the offsets flipped according to the target's direction.
pub fn walk_near_point(b: &mut Cps2Bus, o: Obj) {
    if o.u16(b, crate::ai::f::TARGET) == 0 {
        return;
    }
    let goal = pick_point(b, o, MIRRORED_WALK_POINTS, Pick::Nearest, None, true);
    crate::ai::turn_toward(b, o, goal);
}

/// 0xe886: retains the farthest point around the target, except the last one retained (`+0x99`); far from the
/// camera, aims straight away at a far point (0xf084). Nothing without a target.
pub fn choose_walk_point(b: &mut Cps2Bus, o: Obj) {
    if o.u16(b, crate::ai::f::TARGET) == 0 {
        return;
    }
    if far_from_camera(b, o) {
        return walk_far_point(b, o);
    }
    let last = o.u8(b, g::LAST_POINT);
    pick_point(b, o, WALK_POINTS, Pick::Farthest, Some(last), false);
}

/// 0xe914: is the targeted point reached (to within 8 pixels on each axis)? Then another is chosen (0xe886).
pub fn walk_point_reached(b: &mut Cps2Bus, o: Obj) {
    let near = |goal: u16, here: u16| goal.wrapping_sub(here).wrapping_add(8) <= 16;
    let (gx, x) = (o.u16(b, g::GOAL_X), o.u16(b, Obj::X));
    let (gy, y) = (o.u16(b, g::GOAL_DEPTH), o.u16(b, Obj::Y));
    if near(gx, x) && near(gy, y) {
        choose_walk_point(b, o);
    }
}

/// 0xe7fa: retains the nearest point around the target and notes its number (`+0x99`, 0xff if none); far
/// from the camera, aims at a far point (0xf084). Nothing without a target.
pub fn choose_near_walk_point(b: &mut Cps2Bus, o: Obj) {
    if o.u16(b, crate::ai::f::TARGET) == 0 {
        return;
    }
    if far_from_camera(b, o) {
        return walk_far_point(b, o);
    }
    o.set_u8(b, g::LAST_POINT, 0xff);
    let target = linked(o.u16(b, crate::ai::f::TARGET));
    let set = o.u8(b, g::POINT_SET) as u32;
    let mut at = rd32(b, WALK_POINTS + 4 * set);
    let last = rd16(b, at);
    at += 2;
    let (target_x, target_depth) = (target.u16(b, Obj::X), target.u16(b, Obj::Y));
    o.set_u16(b, g::GOAL_X, target_x);
    o.set_u16(b, g::GOAL_DEPTH, target_depth);
    let mut best: u16 = 0x7fff;
    for point in (0..=last).rev() {
        let (x, depth) = (rd16(b, at).wrapping_add(target_x), rd16(b, at + 2).wrapping_add(target_depth));
        at += 4;
        let distance = manhattan(b, o.0, x, depth);
        if distance <= best {
            best = distance;
            o.set_u16(b, g::GOAL_X, x);
            o.set_u16(b, g::GOAL_DEPTH, depth);
            o.set_u8(b, g::LAST_POINT, point as u8);
        }
    }
}
