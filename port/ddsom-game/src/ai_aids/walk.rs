//! Choosing a point to walk to around the target (0xe76c, 0xe7fa, 0xe886, 0xe914, 0xeff2, 0xf084).

use super::*;

/// Tables of candidate points by number (`+0x79`): address of a list (number of points - 1, then pairs (x offset,
/// depth offset) around the target). The mirrored table has its x offsets flipped by the direction of the target.
fn walk_points() -> &'static Table {
    crate::tables::engine_table!("approach_tables")
}
fn mirrored_walk_points() -> &'static Table {
    crate::tables::engine_table!("ai/mirrored_walk_points")
}

/// "City block" gap between two points: |dx| + |dy| (words).
pub(super) fn manhattan(b: &mut World, me: Obj, x: u16, depth: u16) -> u16 {
    let dx = (x.wrapping_sub(me.u16(b, Obj::X)) as i16).wrapping_abs() as u16;
    let dy = (depth.wrapping_sub(me.u16(b, Obj::Y)) as i16).wrapping_abs() as u16;
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
pub(super) fn pick_point(b: &mut World, o: Obj, table: &Table, pick: Pick, skip: Option<u8>, mirror: bool) -> (u16, u16) {
    let target = Obj::from_word(Enemy(o).target(b));
    let set = Enemy(o).ai_point_set(b) as u32;
    let mut at = table.nth32(b, set);
    let last = engine().word(b, at);
    at += 2;
    let (target_x, target_depth) = (target.u16(b, Obj::X), target.u16(b, Obj::Y));
    Enemy(o).set_ai_goal_x(b, target_x);
    Enemy(o).set_ai_goal_y(b, target_depth);
    let mut best: u16 = if pick == Pick::Farthest { 0 } else { 0x7fff };
    for point in (0..=last).rev() {
        if skip == Some(point as u8) {
            continue;
        }
        let mut dx = engine().word(b, at);
        let dy = engine().word(b, at + 2);
        at += 4;
        if mirror && target.flip(b) != 0 {
            dx = dx.wrapping_neg();
        }
        let (x, depth) = (dx.wrapping_add(target_x), dy.wrapping_add(target_depth));
        let distance = manhattan(b, o, x, depth);
        let better = match pick {
            Pick::Farthest => distance > best,
            Pick::Nearest => distance <= best,
        };
        if better {
            best = distance;
            Enemy(o).set_ai_goal_x(b, x);
            Enemy(o).set_ai_goal_y(b, depth);
            if skip.is_some() {
                Enemy(o).set_ai_last_point(b, point as u8);
            }
        }
    }
    (Enemy(o).ai_goal_x(b), Enemy(o).ai_goal_y(b))
}

/// Is `o` more than 384 pixels to the right of the left edge of the camera in service?
pub(super) fn far_from_camera(b: &mut World, o: Obj) -> bool {
    let (camera, _) = crate::ram::camera(b);
    o.u16(b, Obj::X).wrapping_sub(camera) > 0x180
}

/// 0xf084: aims at the farthest point around the target and turns one notch towards it.
pub fn walk_far_point(b: &mut World, o: Obj) {
    let goal = pick_point(b, o, walk_points(), Pick::Farthest, None, false);
    crate::ai::turn_toward(b, o, goal);
}

/// 0xe76c: aims at the nearest point around the target (far from the camera: the farthest, 0xf084) and
/// turns one notch towards it. Nothing without a target.
pub fn approach_target(b: &mut World, o: Obj) {
    if Enemy(o).target(b) == 0 {
        return;
    }
    if far_from_camera(b, o) {
        return walk_far_point(b, o);
    }
    let goal = pick_point(b, o, walk_points(), Pick::Nearest, None, false);
    crate::ai::turn_toward(b, o, goal);
}

/// 0xeff2: like `approach_target` (without the far case), the offsets flipped according to the target's direction.
pub fn walk_near_point(b: &mut World, o: Obj) {
    if Enemy(o).target(b) == 0 {
        return;
    }
    let goal = pick_point(b, o, mirrored_walk_points(), Pick::Nearest, None, true);
    crate::ai::turn_toward(b, o, goal);
}

/// 0xe886: retains the farthest point around the target, except the last one retained (`+0x99`); far from the
/// camera, aims straight away at a far point (0xf084). Nothing without a target.
pub fn choose_walk_point(b: &mut World, o: Obj) {
    if Enemy(o).target(b) == 0 {
        return;
    }
    if far_from_camera(b, o) {
        return walk_far_point(b, o);
    }
    let last = Enemy(o).ai_last_point(b);
    pick_point(b, o, walk_points(), Pick::Farthest, Some(last), false);
}

/// 0xe914: is the targeted point reached (to within 8 pixels on each axis)? Then another is chosen (0xe886).
pub fn walk_point_reached(b: &mut World, o: Obj) {
    let near = |goal: u16, here: u16| goal.wrapping_sub(here).wrapping_add(8) <= 16;
    let (gx, x) = (Enemy(o).ai_goal_x(b), o.u16(b, Obj::X));
    let (gy, y) = (Enemy(o).ai_goal_y(b), o.u16(b, Obj::Y));
    if near(gx, x) && near(gy, y) {
        choose_walk_point(b, o);
    }
}

/// 0xe7fa: retains the nearest point around the target and notes its number (`+0x99`, 0xff if none); far
/// from the camera, aims at a far point (0xf084). Nothing without a target.
pub fn choose_near_walk_point(b: &mut World, o: Obj) {
    if Enemy(o).target(b) == 0 {
        return;
    }
    if far_from_camera(b, o) {
        return walk_far_point(b, o);
    }
    Enemy(o).set_ai_last_point(b, 0xff);
    let target = Obj::from_word(Enemy(o).target(b));
    let set = Enemy(o).ai_point_set(b) as u32;
    let mut at = walk_points().nth32(b, set);
    let last = engine().word(b, at);
    at += 2;
    let (target_x, target_depth) = (target.u16(b, Obj::X), target.u16(b, Obj::Y));
    Enemy(o).set_ai_goal_x(b, target_x);
    Enemy(o).set_ai_goal_y(b, target_depth);
    let mut best: u16 = 0x7fff;
    for point in (0..=last).rev() {
        let (x, depth) = (engine().word(b, at).wrapping_add(target_x), engine().word(b, at + 2).wrapping_add(target_depth));
        at += 4;
        let distance = manhattan(b, o, x, depth);
        if distance <= best {
            best = distance;
            Enemy(o).set_ai_goal_x(b, x);
            Enemy(o).set_ai_goal_y(b, depth);
            Enemy(o).set_ai_last_point(b, point as u8);
        }
    }
}
