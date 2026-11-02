//! Facade of the helpers common to enemies and bosses: the families of batch A call these functions (`lib::…`), which
//! call the Rust cores of the original routines (`ai`, `ai_aids`, `camera_modes`, `creators`, `events`, `popups`...) on
//! the processed object. Inputs as arguments, results as Rust values; no register.
//!
//! The few registers that the continuation still rereads (original glitches) are written in the clear by the caller, at
//! the place that rereads them (see `docs/refactorisation.md`, AI batch).

use crate::families::cx::Bench;
use super::cx::Cx;
use super::enemy_common::*;
use crate::ai;
use crate::ai_aids;
use crate::machine::Machine;
use crate::objects::{Current, Obj};

/// 0x1046c: the hit type `+0x44` follows the animation key `+0x3e`.
fn copy_strike_kind<R: Bench>(x: &mut Cx<'_, R>) {
    let k = x.u8(0x3e);
    x.set8(0x44, k);
}

/// 0x103d6: starts script `n` of set `set`.
pub fn start<R: Bench>(x: &mut Cx<'_, R>, set: u32, n: u32) {
    x.anim_start(set, n as u16);
    copy_strike_kind(x);
}

/// 0x103e0: resumes the animation with the same key (`+0x33`) in set `set`, keeping the duration (`+0x30`).
pub fn switch_keeping_time<R: Bench>(x: &mut Cx<'_, R>, set: u32) {
    ai_aids::animation_by_variant(x.b, x.o, set);
}

/// 0x1040c: one animation step; true at the end of the script.
pub fn step<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let end = x.anim_step();
    copy_strike_kind(x);
    end
}

/// 0x10474: at least one player alive (`A5+0x35`).
pub fn players_alive<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    ai_aids::players_alive(x.b)
}

/// 0x104a2: true if the object is outside the band `[screen + left, screen + right]`.
pub fn outside_band<R: Bench>(x: &mut Cx<'_, R>, left: i16, right: i16) -> bool {
    ai_aids::outside_band(x.b, x.o, left as u16, right as u16)
}

/// 0x1050c: true if the object faces its target (or has none).
pub fn facing_target<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    ai::facing_target(x.b, x.o)
}

/// 0x10560: true if the X speed follows the orientation.
pub fn moving_forward<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    ai::moving_forward(x.b, x.o)
}

/// 0x10574: the heading `Obj::SPEED_INDEX` changes by at most one notch at random.
pub fn drift_heading<R: Bench>(x: &mut Cx<'_, R>) {
    ai::drift_heading(x.b, x.o);
}

/// 0x1059e: HP rank `+0xac` according to the number of living players and the rank.
pub fn spread_index<R: Bench>(x: &mut Cx<'_, R>) {
    ai::hp_rank(x.b, x.o);
}

/// 0x105b6: crosses the next HP threshold of the table `thresholds`; true if a threshold was passed.
pub fn next_threshold<R: Bench>(x: &mut Cx<'_, R>, thresholds: u32) -> bool {
    ai_aids::next_threshold(x.b, x.o, thresholds)
}

/// Gesture chosen by `turn_decision` (0x1047a).
pub use crate::ai::Turn;

/// 0x1047a: AI decision according to the side of the target.
pub fn turn_decision<R: Bench>(x: &mut Cx<'_, R>) -> Turn {
    ai::turn_decision(x.b, x.o)
}

/// 0xa470: draws the stance `+0x71` in row `+0x70` of the stance table (indexed by rank).
pub fn draw_stance<R: Bench>(x: &mut Cx<'_, R>) {
    ai_aids::stance(x.b, x.o);
}

/// 0x788c: `+0x6b` drawn at random in row `+0x6a` of its table.
pub fn draw_6b<R: Bench>(x: &mut Cx<'_, R>) {
    ai_aids::draw_6b(x.b, x.o);
}

/// 0x79f6: on the screen, `+0x6e` is drawn in row `+0x6d` of its table; otherwise it is 1.
pub fn draw_6e<R: Bench>(x: &mut Cx<'_, R>) {
    ai_aids::draw_6e(x.b, x.o);
}

/// 0x79ce: if the bit drawn in the mask of row `+0x6c` is set, like `draw_6e` and true; otherwise false.
pub fn maybe_draw_6e<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    ai_aids::maybe_draw_6e(x.b, x.o)
}

/// 0xf118: off the screen, `+0x84` = 1; otherwise `+0x84` = 0 and `+0x85` drawn in row `+0x8e`.
pub fn draw_84<R: Bench>(x: &mut Cx<'_, R>) {
    ai_aids::draw_84(x.b, x.o);
}

/// 0x10112: `+0x68` according to the level `A5+0x5a` and the number of living players (8 without level).
pub fn set_68<R: Bench>(x: &mut Cx<'_, R>) {
    ai_aids::set_68(x.b, x.o);
}

/// 0xac3a: `+0x77` drawn from the table chosen by `+0x76` (indexed by rank).
pub fn pick_byte<R: Bench>(x: &mut Cx<'_, R>) {
    ai::pick_byte(x.b, x.o);
}

/// 0xe76c: aims at the approach point of the target `+0x50` (the closest; far from the screen, the farthest) and turns
/// the object one notch toward it.
pub fn approach_target<R: Bench>(x: &mut Cx<'_, R>) {
    ai_aids::approach_target(x.b, x.o);
}

/// 0xf0e8: turns the heading `Obj::SPEED_INDEX` one notch toward the point (`px`, `py`).
pub fn turn_toward<R: Bench>(x: &mut Cx<'_, R>, px: u16, py: u16) {
    ai::turn_toward(x.b, x.o, (px, py));
}

/// 0xfbae: keeps the object on the screen (margin of 0x30 pixels); this is the end-of-frame zone test. True if it was
/// moved.
pub fn keep_in_zone<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.keep_in_zone()
}

/// 0x33fe4: collision with the scenery (`terrain.rs`): the object is pushed out of an obstacle.
/// Original glitch: D2, D3 and D5 (1 on impact) of the terrain search leak through the handlers; the spark of
/// `flash_and_shake` takes its speeds from them (D2, D3: `long_scene_20_2p`), the skeleton rereads D5: they are
/// returned in the leftovers (`Cx::push_terrain`).
pub fn collide_terrain<R: Bench>(x: &mut Cx<'_, R>) {
    x.push_terrain();
}

/// 0xfbb0: keeps the object on the screen, within `margin` pixels; true if it was moved.
pub fn keep_on_screen<R: Bench>(x: &mut Cx<'_, R>, margin: u16) -> bool {
    x.keep_on_screen(margin)
}

/// 0x424e: chooses a player as target (`+0x50`, criterion drawn at random); returns that player.
pub fn pick_target<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
    let t = ai::pick_target(x.b, x.o, ai::Rule::Strict);
    found_d5(x, t)
}

/// D5 = 2 if a player is found, 0 otherwise (`moveq`): the original's D5 crosses the handlers
/// (`dispatch::Leftovers::d5`).
fn found_d5<R: Bench>(x: &mut Cx<'_, R>, t: Option<Obj>) -> Option<Obj> {
    *x.c.d5() = if t.is_some() { 2 } else { 0 };
    t
}

/// 0x43ae: like `pick_target`, with the broad eligibility rule.
pub fn pick_target_any<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let t = ai::pick_target(x.b, x.o, ai::Rule::Lenient);
    found_d5(x, t).is_some()
}

/// 0x44ee: a targetable player drawn at random (three tries) becomes the target; false if none.
pub fn random_player<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    ai::random_player(x.b, x.o).is_some()
}

/// Criteria of `target_by_criterion` (D0 of 0x43bc): the closest, the fewest HP, the farthest.
pub const NEAREST: u8 = 0;
pub const LOWEST_HP: u8 = 2;
pub const FARTHEST: u8 = 4;

/// 0x43bc: the target becomes the player that meets the criterion (`NEAREST`...); returns that player.
pub fn target_by_criterion<R: Bench>(x: &mut Cx<'_, R>, criterion: u8) -> Option<Obj> {
    let t = ai::target_by(x.b, x.o, ai::Criterion::from_byte(criterion), ai::Rule::Lenient);
    found_d5(x, t)
}

/// 0x453c: initial HP of a boss (table 0x458c), copied to `+0x60`, `+0x62` and `+0x64`.
pub fn initial_hp_boss<R: Bench>(x: &mut Cx<'_, R>) {
    ai::initial_hp(x.b, x.o, ai::HP_TABLE_BOSS);
}

/// 0x4544: initial HP of an enemy (table 0x5c8c).
pub fn initial_hp<R: Bench>(x: &mut Cx<'_, R>) {
    ai::initial_hp(x.b, x.o, ai::HP_TABLE_ENEMY);
}

/// 0xfe5a: immunities (`+0x94`) according to the class and the type. Returns the index read in the table (D0 of the
/// original, which the harpy rereads).
pub fn set_immunities<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    ai::set_immunities(x.b, x.o)
}

/// Side of the target found in a box (D0 of 0xb5e0 and 0x7f19c).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InBox {
    No = 0,
    /// On the side the object faces.
    Ahead = 1,
    Behind = 2,
}

impl InBox {
    pub fn found(self) -> bool {
        self != InBox::No
    }
}

impl From<ai_aids::InHitbox> for InBox {
    fn from(d: ai_aids::InHitbox) -> InBox {
        match d {
            ai_aids::InHitbox::No => InBox::No,
            ai_aids::InHitbox::Face => InBox::Ahead,
            ai_aids::InHitbox::Back => InBox::Behind,
        }
    }
}

/// The box of offset `dx` (flipped according to the orientation), of half-width `half_w` and half-depth `half_d`.
fn hitbox(dx: i16, half_w: u16, half_d: u16) -> ai_aids::Hitbox {
    ai_aids::Hitbox { offset: dx as u16, half_width: half_w, half_depth: half_d }
}

/// 0xb5e0: is the target `+0x50` (on the ground) inside the box in front of the object?
///
/// Original glitch: the box tests leave D2 and D3 (and the low byte of D4), which leak through the handlers and give
/// their speeds to the spark of `flash_and_shake` (seen on `long_scene_20_2p`); boss 11 rereads D2 (`roll`). They are
/// returned in the leftovers.
pub fn target_in_front<R: Bench>(x: &mut Cx<'_, R>, dx: i16, half_w: u16, half_d: u16) -> InBox {
    let (found, leftovers) = ai_aids::target_in_front_leftovers(x.b, x.o, hitbox(dx, half_w, half_d));
    set_leftovers(x, leftovers);
    found.into()
}

/// Leftovers of a box test (D2, D3, D4) into those of the dispatch loop.
fn set_leftovers<R: Bench>(x: &mut Cx<'_, R>, leftovers: ai_aids::HitboxLeftovers) {
    let (mut d2, mut d3, mut d4) = (*x.c.d2(), *x.c.d3(), *x.c.d4());
    leftovers.set(&mut d2, &mut d3, &mut d4);
    (*x.c.d2(), *x.c.d3(), *x.c.d4()) = (d2, d3, d4);
}

/// `target_in_front`: is the target in the box, on one side or the other?
pub fn target_in_box<R: Bench>(x: &mut Cx<'_, R>, dx: i16, half_w: u16, half_d: u16) -> bool {
    target_in_front(x, dx, half_w, half_d).found()
}

/// 0x7f19c: is the object `other` in the box in front of the object (same parameters as `target_in_front`)?
pub fn in_box<R: Bench>(x: &mut Cx<'_, R>, other: Obj, dx: i16, half_w: u16, half_d: u16) -> InBox {
    let (found, leftovers) = ai_aids::box_around_leftovers(x.b, x.o, other, hitbox(dx, half_w, half_d));
    set_leftovers(x, leftovers);
    found.into()
}

/// 0x7ef4: is the target, on guard (`+6` = 12), in the box in front of the object and facing it? If so and after a
/// draw, the object goes to state 6 with a randomly drawn parry, and the function returns true.
pub fn guard_in_front<R: Bench>(x: &mut Cx<'_, R>, dx: i16, half_w: u16, half_d: u16) -> bool {
    let (yes, leftovers) = ai_aids::in_front_with_leftovers(x.b, x.o, false, hitbox(dx, half_w, half_d));
    set_leftovers(x, leftovers);
    yes
}

/// 0x7f0c: like `guard_in_front`, the guard also being allowed to be 14, and the target on the ground.
pub fn guard_in_front_strict<R: Bench>(x: &mut Cx<'_, R>, dx: i16, half_w: u16, half_d: u16) -> bool {
    let (yes, leftovers) = ai_aids::in_front_with_leftovers(x.b, x.o, true, hitbox(dx, half_w, half_d));
    set_leftovers(x, leftovers);
    yes
}

/// 0xa4b4 (`+0x72`): does the AI act on this frame? (Bit drawn in a mask chosen by the rank.)
pub fn decision_a<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    ai::decision_a(x.b, x.o)
}

/// 0xa4c2: like `decision_a`, with the table of `+0x73`.
pub fn decision_b<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    ai::decision_b(x.b, x.o)
}

/// AI decision according to the side of the target: `decision_b` if it faces the same side as the object, otherwise
/// `decision_a`.
pub fn decision<R: Bench>(x: &mut Cx<'_, R>, same_side: bool) -> bool {
    if same_side { decision_b(x) } else { decision_a(x) }
}

/// 0x57c66 (`creators.rs`): is the object in the right half of the screen (x >= camera + 0xc0)?
pub fn in_right_half<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    super::creators::in_right_half(x.b, x.o)
}

/// 0x5cdc6 (`creators.rs`): random draw, true 10 times out of 16.
pub fn chance_10_16<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    super::creators::chance_10_16(x.b)
}

/// 0x1386 (`aids::direction`): direction (0 to 31) from `de` to `to`.
pub fn aim<R: Bench>(x: &mut Cx<'_, R>, de: (u16, u16), to: (u16, u16)) -> u8 {
    crate::aids::direction(x.b, de, to)
}

/// 0x137e: direction from the object to the point `to`.
pub fn aim_from<R: Bench>(x: &mut Cx<'_, R>, to: (u16, u16)) -> u8 {
    crate::aids::direction_from(x.b, x.o, to)
}

/// 0x1376: direction from the object to `target`.
pub fn aim_to<R: Bench>(x: &mut Cx<'_, R>, target: Obj) -> u8 {
    crate::aids::direction_to(x.b, x.o, target)
}

/// 0xfc44: is a player hit by the object's attack box `attack`? Returns that player.
pub fn attack_hits_a_player<R: Bench>(x: &mut Cx<'_, R>, attack: u8) -> Option<Obj> {
    ai_aids::attack_hits_a_player(x.b, x.o, attack)
}

/// 0xfd64: the player `player` is grabbed by the object (grab animation `anim`).
pub fn seize_player<R: Bench>(x: &mut Cx<'_, R>, player: Obj, anim: u8) {
    ai_aids::seize_player(x.b, x.o, player, anim);
}

/// 0xfd8c: the player `player` is released.
pub fn release_player<R: Bench>(x: &mut Cx<'_, R>, player: Obj) {
    ai_aids::release_player(x.b, player);
}

/// 0xfda2: the grabbed player (`+0x50`) follows the object according to the step of the grab animation.
pub fn hold_seized<R: Bench>(x: &mut Cx<'_, R>) {
    ai_aids::hold_seized(x.b, x.o);
}

/// 0xff54: countdown `+0x98`, then new state according to the number of players and a draw.
pub fn countdown_then_state<R: Bench>(x: &mut Cx<'_, R>) {
    ai_aids::countdown_then_state(x.b, x.o);
}

/// 0xfffc: behaviour parameter `+0x98` read according to the type.
pub fn behaviour_by_kind<R: Bench>(x: &mut Cx<'_, R>) {
    ai_aids::behaviour_by_kind(x.b, x.o);
}

/// 0x1172e: debris set no. `set` around the object.
pub fn debris<R: Bench>(x: &mut Cx<'_, R>, set: u32) {
    ai_aids::debris_set(x.b, x.o, set as u16);
    // Original glitch: D5 = the exhausted counter of the `dbf`, which crosses the handlers (`dispatch::Leftovers::d5`)
    *x.c.d5() = 0xffff;
}

/// 0x11a36: five debris on the ground (second table if `second` is non-zero), then the sound 0xede08.
pub fn ground_debris<R: Bench>(x: &mut Cx<'_, R>, second: u8) {
    ai_aids::ground_debris(x.b, x.o, second != 0);
    // Original glitch: D5.w = the exhausted counter of the `dbf` (`dispatch::Leftovers::d5`)
    *x.c.d5() |= 0xffff;
}

/// 0xb648: only once per object, according to a draw, drops the object specific to its type.
pub fn drop_once<R: Bench>(x: &mut Cx<'_, R>) {
    ai_aids::drop_once(x.b, x.o);
}

/// End loot of an enemy or a boss: 0xb780, 0xbad6 then 0xe2f6. Original glitch: D2 and D3 left by these routines (the
/// treasure row, the difficulty row of the special loot) give their speeds to the spark of `flash_and_shake`, which
/// follows for most bosses, and D5 (counter of the last loop) crosses the handlers up to 0xa17d0
/// (`ai::keep_on_screen_d5`): they are returned in the clear.
pub fn drop_rewards<R: Bench>(x: &mut Cx<'_, R>) {
    ai_aids::drop_loot(x.b, x.o);
    let d3 = ai_aids::drop_special(x.b, x.o);
    let (d2, d5) = ai_aids::drop_treasure(x.b, x.o);
    // reread: speeds of the spark of 0x103ba (`flash_and_shake`)
    *x.c.d2() = *x.c.d2() & !0xffff | d2 as u32;
    if let Some(d3) = d3 {
        *x.c.d3() = *x.c.d3() & !0xffff | d3 as u32;
    }
    if let Some(d5) = d5 {
        *x.c.d5() = d5;
    }
}

/// 0x115e2: the boss pushes the present players in front of it.
pub fn push_players<R: Bench>(x: &mut Cx<'_, R>) {
    ai_aids::push_players(x.b, x.o);
}

/// 0x103ba: screen flash, shake and impact spark. Original glitch: the spark speeds are the D2 and D3 left by what
/// precedes (`drop_rewards`, or the handler): they are read here in the clear.
pub fn flash_and_shake<R: Bench>(x: &mut Cx<'_, R>) {
    let (vx, ax) = (*x.c.d2() as u16, *x.c.d3() as u16);
    ai_aids::flash_and_shake(x.b, x.o, vx, ax);
}

/// 0x3e480: landing sound specific to the object's type (ROM table per type: each entry is a sound, or an `rts`).
pub fn landing_by_kind<R: Bench>(x: &mut Cx<'_, R>) {
    if let Some((sound_id, _)) = ai_aids::landing_routine(x.b, x.o) {
        sound(x, sound_id);
    }
}

/// 0x10428: starts the animation of set `set` whose mark is the variant (`+0x33`) of the master (`+0xb2`).
pub fn animation_of_master<R: Bench>(x: &mut Cx<'_, R>, set: u32) {
    ai_aids::animation_of_master(x.b, x.o, set);
}

/// 0x10450: starts the animation of set `set` whose mark is that of `+0x3e` of the master.
pub fn animation_of_mark<R: Bench>(x: &mut Cx<'_, R>, set: u32) {
    ai_aids::animation_of_mark(x.b, x.o, set);
}

/// 0x2824, 0x2860, 0x289c: shake of the camera in service for `frames` frames, in mode 0x54, 0x58 or 0x5c
/// (`camera_modes.rs`).
pub fn shake_54<R: Bench>(x: &mut Cx<'_, R>, frames: u16) {
    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::A, frames);
}
pub fn shake_58<R: Bench>(x: &mut Cx<'_, R>, frames: u16) {
    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, frames);
}
pub fn shake_5c<R: Bench>(x: &mut Cx<'_, R>, frames: u16) {
    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::C, frames);
}

/// Walk points around the target: 0xe886 (the farthest, except the last), 0xe914 (reached: another one), 0xeff2 (the
/// closest, flipped according to the target), 0xf084 (the farthest), 0xe7fa (the closest, numbered).
pub fn choose_walk_point<R: Bench>(x: &mut Cx<'_, R>) {
    ai_aids::choose_walk_point(x.b, x.o);
}
pub fn walk_point_reached<R: Bench>(x: &mut Cx<'_, R>) {
    ai_aids::walk_point_reached(x.b, x.o);
}
pub fn walk_near_point<R: Bench>(x: &mut Cx<'_, R>) {
    ai_aids::walk_near_point(x.b, x.o);
}
pub fn walk_far_point<R: Bench>(x: &mut Cx<'_, R>) {
    ai_aids::walk_far_point(x.b, x.o);
}
pub fn choose_near_walk_point<R: Bench>(x: &mut Cx<'_, R>) {
    ai_aids::choose_near_walk_point(x.b, x.o);
}

// --- sounds -------------------------------------------------------------------------------------------

/// Plays the sound `wrapper` (small original routine of the table `sound_table.rs`) for the object.
pub fn sound<R: Bench>(x: &mut Cx<'_, R>, wrapper: u32) {
    crate::sound::play(x.b, wrapper, x.o);
}

// --- object creators and services of other modules -----------------------------------------------------

/// 0xd5f30: in game, SET object 0x14 linked to the boss (health bar, held in `A5-0x5b8a`).
pub fn create_set_14<R: Bench>(x: &mut Cx<'_, R>) {
    super::creators::set_14(x.b, x.o);
}

/// 0xd6952: SET object 0x15 (variant 2) whose `+0x42` depends on the creator's type.
pub fn create_set_15<R: Bench>(x: &mut Cx<'_, R>) {
    super::creators::set_15_b(x.b, x.o);
}

/// 0xd5ef8: the displayed health bar is attached to the boss.
pub fn create_life_bar<R: Bench>(x: &mut Cx<'_, R>) {
    super::set_residue::attach_life_bar(x.b, x.o);
}

/// 0x19ad6: event of the object (type `+0x7e`, player `+0x48`) in the points ring.
pub fn object_event<R: Bench>(x: &mut Cx<'_, R>) {
    crate::events::object_event(x.b, x.o);
}

/// 0x19e10: clears the current pop-up.
pub fn clear_popup<R: Bench>(x: &mut Cx<'_, R>) {
    crate::events::clear_current_popup(x.b);
}

/// 0x19db4: empties the pop-up queue.
pub fn empty_popups<R: Bench>(x: &mut Cx<'_, R>) {
    crate::events::clear_popups(x.b);
}

/// 0x19eb0: clears all the displayed pop-ups.
pub fn clear_all_popups<R: Bench>(x: &mut Cx<'_, R>) {
    crate::popups::clear_all(x.b);
}

/// What the 0xb10c8 shock returns: the created effect, or what the original leaves in A1 and the carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Impact {
    Created(Obj),
    /// Pool full (carry set): A1 = the slot left by the allocation (`Pool::alloc_or_leftover`).
    Refuse(Obj),
    /// Scene 41: no effect, neither A1 nor carry (the final `cmp.b` leaves the carry at zero).
    Nothing,
}

/// 0xb10c8: shock effect at the object's position: variant, setting (`+0x2d`, `+0x2f`), speeds `+0x14`, `+0x1c`.
pub fn shock<R: Bench>(x: &mut Cx<'_, R>, variant: u32, setting: u16, vx: u16, vy: u16) -> Impact {
    if let Some(o) = crate::effects_aids::shock(x.b, x.o, variant as u8, setting, vx, vy) {
        return Impact::Created(o);
    }
    if crate::ram::SCENE.get(x.b) == 41 {
        return Impact::Nothing;
    }
    // the allocation failed: redoing it changes nothing, and returns the A1 left
    Impact::Refuse(crate::objects::pools::EFFCT.alloc_or_leftover(x.b).unwrap_err())
}

/// 0xabec8: attack effect (EFFCT 11) linked by `+0x5c`, of given variant and setting; false if the pool refused it (the
/// original's carry).
pub fn attack_effect<R: Bench>(x: &mut Cx<'_, R>, variant: u32, setting: u32) -> bool {
    super::effects_creators::attack(x.b, x.o, variant as u8, setting as u8).is_some()
}

/// Series of attack effects (0xabec8) of variant `variant`, settings `first` down to 0 (`dbf D1`). False if the pool
/// refused an effect (the series stops there).
pub fn attack_effects<R: Bench>(x: &mut Cx<'_, R>, variant: u32, first: u32) -> bool {
    // `dbf D1`: the setting counts down to 0, only the low byte is passed to the creator
    let mut setting = first;
    loop {
        if super::effects_creators::attack(x.b, x.o, variant as u8, setting as u8).is_none() {
            return false;
        }
        let n = (setting as u16).wrapping_sub(1);
        setting = setting & !0xffff | n as u32;
        if n == 0xffff {
            return true;
        }
    }
}

/// 0xab7d8: burst of debris (EFFCT 6) of given variant, linked by `+0x50`.
pub fn debris_spray<R: Bench>(x: &mut Cx<'_, R>, variant: u32) {
    super::effects_creators::debris(x.b, x.o, variant as u8);
}

// --- original routines without a native, never reached by the scenarios ---------------------------------
//
// Called by address from the families of batch A, they had no native: the game without the 68000 would have stopped on
// reaching them. Transcribed from `listing.txt` (original's output registers set), not validated by a scenario (see
// docs/code-mort.md, batch A).

/// Copy of the object's positions and orientation to a created shot.
fn copy_place<R: Bench>(x: &mut Cx<'_, R>, o: Obj, with_z: bool) {
    let (px, py) = (x.u32(Obj::X), x.u32(Obj::Y));
    o.set_u32(x.b, Obj::X, px);
    o.set_u32(x.b, Obj::Y, py);
    if with_z {
        let pz = x.u32(Obj::Z);
        o.set_u32(x.b, Obj::Z, pz);
    }
}

/// ESHL allocation of these creators (`jsr 0x22ea ; bcs end`): A1 = the object like the original.
fn alloc_eshl<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
    let o = x.alloc(&crate::objects::pools::ESHL)?;
    Some(o)
}

/// 0xa09ca: ESHL shot 0x12 placed 0x30 pixels above the object and 0x60 pixels in front of it. Returns the shot and the
/// original's final carry. Original glitch: the carry that the caller then tests (`bcc`) is that of the addition of the
/// X position, not that of the allocation, when the shot could be created.
pub fn eshl_12<R: Bench>(x: &mut Cx<'_, R>) -> (Option<Obj>, bool) {
    let Some(o) = alloc_eshl(x) else { return (None, true) };
    o.set_u8(x.b, 0, 1);
    o.set_u8(x.b, Obj::KIND, 0x12);
    let flip = x.u8(f::FLIP);
    o.set_u8(x.b, f::FLIP, flip);
    copy_place(x, o, false);
    o.set_u16(x.b, Obj::Z, 0x30);
    let d2: u16 = if flip != 0 { 0x60 } else { (-0x60i16) as u16 };
    let old = o.u16(x.b, Obj::X);
    o.set_u16(x.b, Obj::X, old.wrapping_add(d2));
    (Some(o), old.checked_add(d2).is_none())
}

/// 0xa0ae0: ESHL shot 0x13 aiming at `target` (long `+0x50`), launched by the object (`+0x54`), at its position.
pub fn eshl_13<R: Bench>(x: &mut Cx<'_, R>, target: u32) -> Option<Obj> {
    let o = alloc_eshl(x)?;
    let link = x.u16(0x42);
    o.set_u16(x.b, 0x42, link);
    o.set_u32(x.b, 0x50, target);
    o.set_u32(x.b, 0x54, x.o.0);
    o.set_u8(x.b, f::BOX_SET, 0);
    o.set_u8(x.b, 0, 1);
    o.set_u8(x.b, Obj::KIND, 0x13);
    copy_place(x, o, true);
    let flip = x.u8(f::FLIP);
    o.set_u8(x.b, f::FLIP, flip);
    Some(o)
}

/// 0xa114a: ESHL shot 0x16 of variant `variant` at the object's position (`+0x2f` = its frame mark `+0x33`).
pub fn eshl_16<R: Bench>(x: &mut Cx<'_, R>, variant: u8) {
    let Some(o) = alloc_eshl(x) else { return };
    o.set_u8(x.b, 0, 1);
    o.set_u8(x.b, Obj::KIND, 0x16);
    copy_place(x, o, true);
    let flip = x.u8(f::FLIP);
    o.set_u8(x.b, f::FLIP, flip);
    o.set_u8(x.b, Obj::VARIANT, variant);
    let mark = x.u8(f::KEY_FRAME);
    o.set_u8(x.b, 0x2f, mark);
}

/// 0x9f346: ESHL shot 0xa launched by the object (`+0x50`), at its position, oriented the opposite way.
pub fn eshl_0a<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
    let o = alloc_eshl(x)?;
    o.set_u16(x.b, 0x50, x.o.0 as u16);
    o.set_u8(x.b, 0, 1);
    o.set_u8(x.b, Obj::KIND, 0xa);
    copy_place(x, o, false);
    let flip = x.u8(f::FLIP) ^ 0x20;
    o.set_u8(x.b, f::FLIP, flip);
    Some(o)
}

/// 0xa2d22: six ESHL 0x26 shots (`+0x2f` = 5 down to 0) linked by `+0x46`; the first pool refusal stops the series.
pub fn eshl_burst_26<R: Bench>(x: &mut Cx<'_, R>) {
    for n in (0..=5u8).rev() {
        let Some(o) = alloc_eshl(x) else { return };
        o.set_u8(x.b, 0, 1);
        o.set_u8(x.b, Obj::KIND, 0x26);
        o.set_u8(x.b, Obj::VARIANT, 0);
        o.set_u8(x.b, 0x2f, n);
        o.set_u16(x.b, 0x46, x.o.0 as u16);
    }
}

/// Draw of a bit (`random & bits`) in a mask.
fn roll_bit<R: Bench>(x: &mut Cx<'_, R>, mask: u32, bits: u32) -> bool {
    let r = (x.random() as u32) & bits;
    mask >> r & 1 != 0
}

/// 0xffda: draw of a bit in the mask (word) of table 0xfff4, chosen by rank / 8. (Batch A read the table at 0xffd8:
/// fixed by `fuzz_ia`.)
pub fn roll_ffda<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let i = (var::RANK.get(x.b) >> 3) << 1;
    let mask = x.rom16(0xfff4u32.wrapping_add(i as i16 as u32));
    // leftover: the mask in D2.w (`dispatch::Leftovers::d2`)
    *x.c.d2() = *x.c.d2() & !0xffff | mask as u32;
    roll_bit(x, mask as u32, 0xf)
}

/// 0xad7c: if `+0x78` is positive, draw of a bit in the mask (long) of its row of table 0xadb0, chosen by the frame
/// counter; false otherwise.
pub fn roll_78<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let line = x.u8(0x78);
    if (line as i8) < 0 {
        return false;
    }
    let row = x.data32(0xadb0 + 4 * line as u32);
    let tick = var::RANK.get(x.b);
    let mask = x.data32(row.wrapping_add(((tick << 2) as i16) as u32));
    let hit = roll_bit(x, mask, 0x1f);
    hit
}

/// 0x10298: palette script `index`: eight longs of table 0x102ba copied into the palette (0x91c380), maximum brightness
/// (`ori.l #0xf000f000`).
pub fn palette_script<R: Bench>(x: &mut Cx<'_, R>, index: u16) {
    const PALETTE: u32 = 0x91c380;
    let from = 0x102bau32.wrapping_add((index << 5) as i16 as u32);
    // movem.l (A2),{D0-D6,A3} then movem.l ...,(A1)
    let mut longs = [0u32; 8];
    for (k, v) in longs.iter_mut().enumerate() {
        let a = from + 4 * k as u32;
        // `movem.l (A2)`: data read (batch A read the program space: fixed by `fuzz_ia`)
        *v = x.data32(a);
    }
    for (k, &v) in longs.iter().enumerate() {
        crate::mem::wr32(x.b, PALETTE + 4 * k as u32, v);
    }
    // leftovers: the `movem` leaves longs 2 to 5 in D2-D5 (`dispatch::Leftovers`)
    (*x.c.d2(), *x.c.d3(), *x.c.d4(), *x.c.d5()) = (longs[2], longs[3], longs[4], longs[5]);
    for k in 0..8 {
        let v = crate::mem::rd32(x.b, PALETTE + 4 * k);
        crate::mem::wr32(x.b, PALETTE + 4 * k, v | 0xf000_f000);
    }
}
