//! ITEM family (class 0xC): pickup objects. Dispatch `0x39f34`, type table `0x39f5a` (10 types, 32 slots of 0x60
//! bytes).
//!
//! An ITEM object is born (created by an enemy, a chest or scenery) at the position of its creator; it falls if it is
//! in the air, stays on the ground for a limited time blinking before disappearing, and produces its effect when a
//! player picks it up (the player's code sets `PICKUP` and `OWNER`). See `docs/familles.md` for the description of
//! each type.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::mem::{rd16, wr16, wr32, A5};
use crate::objects::pools;
use crate::objects::Obj;

use super::{add_u8, effects_creators, handler, pointer};
use super::effects_creators::leftovers::Regs;
use super::cx::{Bench, Cx};
use crate::objects::Current;
use crate::machine::Machine;

/// Values of the state byte `+4`.
mod state {
    pub const INIT: u8 = 0;
    pub const ACTIVE: u8 = 2;
    pub const FREED: u8 = 4;
    /// Set by `obj_cull_offscreen`: treated as `FREED`.
    pub const OUTSIDE_SCREEN: u8 = 6;
}

/// Values of the substate byte `+5` of the active state (falling types).
mod sub {
    pub const FALL: u8 = 0;
    pub const WAIT: u8 = 2;
    pub const PICKUP: u8 = 4;
}

/// Fields specific to ITEMs.
mod field {
    pub const SUB_STATE: u32 = 0x05;
    pub const SUB_SUB_STATE: u32 = 0x06;
    pub const FLIP: u32 = 0x2c;
    pub const GAME_SHADOWS: u32 = 0x2e;
    pub const MODE: u32 = 0x2f;
    pub const DURATION: u32 = 0x42;
    pub const OWNER: u32 = 0x50;
    pub const SHIMMER: u32 = 0x54;
    pub const PICKUP: u32 = 0x58;
    pub const INDEX: u32 = 0x5d;
}

/// Shadow set common to ITEMs.
const SHADOWS: u8 = 0x3c;
/// "Collectible picked up" flags (one byte per object), `A5 - 0x5b88`.
const PICKED: i32 = -0x5b88;

/// Subroutines not rewritten.
mod call {
    /// Common end of the potion effects.
    pub const END_EFFECT: u32 = 0xebaf6;
}

/// `0xae3ec`: sparkle (EFFCT 15) above the object, according to `SHIMMER`.
fn shimmer<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    effects_creators::twinkle(ctx.b, o);
}

/// `0xb12c0` (D0 = `variant`): pickup effect (EFFCT 27) attached to the player owning the object.
fn effect_pickup<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, variant: u8) {
    effects_creators::pickup(ctx.b, o, variant);
}

/// `0xc8312`: SET object of the scene; `None` if the pool is full.
fn scene_set_of<R: Bench>(ctx: &mut Cx<'_, R>, variant: u8) -> Option<Obj> {
    super::creators::scene_set(ctx.b, variant)
}

/// `0xaba16`: single-animation burst (EFFCT 7) on the object.
fn shard<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    effects_creators::flash(ctx.b, o);
}

/// Goes to the next state with a given substate (`addq.b #2,(4,A0)`, sub-substate zero).
fn state_next<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, sub_state: u8) {
    add_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, field::SUB_STATE, sub_state);
    o.set_u16(ctx.b, field::SUB_SUB_STATE, 0);
}

/// An object created above the ground (non-zero altitude) starts by falling.
fn airborne<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let z = o.u16(ctx.b, Obj::Z);
    !(z == 0) // tst.w (0xc,A0) ; beq: on the ground
}

/// Marks the collectible object `+0x5d` as picked up.
fn mark_pickup<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let index = o.u8(ctx.b, field::INDEX) as i32;
    ctx.set_ram8(PICKED + index, 1);
}

/// Brings the object back into the visible area: at most as high as `camera.y + 0x20`, between `camera.x + 0x18` and
/// `camera.x + 0x168` (block copied into each type).
fn in_view<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let (cam_x, cam_y) = ctx.camera();
    let y_min = cam_y.wrapping_add(0x20);
    let y = o.u16(ctx.b, Obj::Y);
    if !(y_min < y) {
        o.set_u16(ctx.b, Obj::Y, y_min);
    }
    let x_min = cam_x.wrapping_add(0x18);
    let x = o.u16(ctx.b, Obj::X);
    if !(x_min as i16 <= x as i16) {
        o.set_u16(ctx.b, Obj::X, x_min);
    } else {
        let x_max = cam_x.wrapping_add(0x168);
        if !(x_max > x) {
            o.set_u16(ctx.b, Obj::X, x_max);
        }
    }
}

/// Lifetime countdown; below 0x3c frames the object blinks (bit 1 of `FLAGS` hides the object every other frame, the
/// phase is given by `D7`); at zero it is freed.
fn expire_and_blinks<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let remainder = o.u16(ctx.b, field::DURATION).wrapping_sub(1);
    o.set_u16(ctx.b, field::DURATION, remainder);
    if !(remainder as i16 >= 0) {
        o.set_u8(ctx.b, Obj::STATE, state::FREED);
        o.set_u8(ctx.b, Obj::ACTIVE, 2);
    }
    if remainder > 0x3c {
        return;
    }
    let phase = remainder.wrapping_add(ctx.counter());
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    if phase & 1 == 0 {
        return;
    }
    o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
}

/// Common end of the fall state: bring back into the area, shadow, sort by depth.
fn fall_end<R: Bench>(ctx: &mut Cx<'_, R>) {
    ctx.keep_in_zone();
    ctx.push_terrain();
    ctx.reinsert();
}

/// Starts script `number` of the animation set `game`, then sorts the object (end of the initializations).
fn animate_and_sort<R: Bench>(ctx: &mut Cx<'_, R>, game: u32, number: u8) {
    ctx.anim_start(game, number as u16);
    ctx.reinsert();
}

/// "Picked up" substate: waits for the player to finish its gesture (owner ready) or a forced pickup (`PICKUP` = 2).
/// Returns false as long as it must wait.
fn pickup_ready<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let pickup = o.u8(ctx.b, field::PICKUP);
    if pickup == 2 {
        return true;
    }
    let player = pointer(ctx, o, field::OWNER);
    let ready = player.u8(ctx.b, 0x33) != 0;
    ready
}

// ---------------------------------------------------------------------------------------------
// Type 0: treasures (0xdabca)

/// Type 0, "treasure": coins and gems picked up for points. Variants 0 to 5; an eight-byte table per variant gives
/// sparkle (+0x54), a specific byte (+0x55), score event (+0x56), sound (+0x57) and pickup index (+0x5d).
///
/// States: 0 init (in the air: fall; on the ground: wait 0x4b0 frames + sparkle), 2 active (fall, wait with blinking,
/// picked up: sound + score), 4/6 release.
mod treasure {
    pub const ENTRY: u32 = 0xdabca;
    pub const TABLE_STATES: u32 = 0xdabe0;
    pub const TABLE_SUB_STATES: u32 = 0xdac68;
    /// Parameters per variant (8 bytes): read in program space.
    pub const PARAMS: u32 = 0xdadb6;
    pub const ANIMATIONS: u32 = 0x1bed1a;
    pub const DURATION_TO_GROUND: u16 = 0x4b0;
    pub const SCORE: u32 = 0x56;
    pub const SOUND: u32 = 0x57;
}

/// `0xdad8e`: copies the variant's parameters.
fn treasure_params<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let base = treasure::PARAMS + ((o.u8(ctx.b, Obj::VARIANT) as u32) << 3);
    for (i, off) in [field::SHIMMER, 0x55, treasure::SCORE, treasure::SOUND, field::INDEX].into_iter().enumerate() {
        let v = ctx.prog8(base + i as u32);
        o.set_u8(ctx.b, off, v);
    }
}

fn treasure_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if airborne(ctx, o) {
        state_next(ctx, o, sub::FALL);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        treasure_params(ctx, o);
    } else {
        state_next(ctx, o, sub::WAIT);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        o.set_u16(ctx.b, field::DURATION, treasure::DURATION_TO_GROUND);
        treasure_params(ctx, o);
        shimmer(ctx, o);
    }
    animate_and_sort(ctx, treasure::ANIMATIONS, variant);
}

fn treasure_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = super::effects::target(ctx, treasure::TABLE_SUB_STATES, sub_state as u32);
    match sub_state {
        sub::FALL => {
            let in_flight = ctx.move_xyz().gt();
            if !in_flight {
                // placed: wait on the ground
                add_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u16(ctx.b, field::DURATION, treasure::DURATION_TO_GROUND);
                o.set_u32(ctx.b, Obj::Z, 0);
                in_view(ctx, o);
                shimmer(ctx, o);
            }
            fall_end(ctx);
        }
        sub::WAIT => {
            // during a cutscene, an object left at the left of the screen disappears
            let cutscene = ctx.ram8(0x44) != 0;
            if cutscene {
                let limit = crate::ram::CAMERA_X.get(ctx.b).wrapping_sub(0x40);
                let x = o.u16(ctx.b, Obj::X);
                if !((limit as i16) < x as i16) {
                    o.set_u8(ctx.b, Obj::STATE, state::OUTSIDE_SCREEN);
                    return;
                }
            }
            let pickup = o.u8(ctx.b, field::PICKUP);
            if !(pickup == 0) {
                add_u8(ctx, o, field::SUB_STATE, 2);
            }
            expire_and_blinks(ctx, o);
        }
        sub::PICKUP => {
            if !pickup_ready(ctx, o) {
                return;
            }
            add_u8(ctx, o, Obj::STATE, 2);
            let sound = o.u8(ctx.b, treasure::SOUND);
            effect_pickup(ctx, o, sound);
            let player_bits = o.u16(ctx.b, 0x48) & 0x300;
            let event = (o.u8(ctx.b, treasure::SCORE) as u16).wrapping_add(player_bits);
            crate::events::enqueue_points(ctx.b, event);
            mark_pickup(ctx, o);
        }
        _ => crate::families::effects::outside_table(target),
    }
}

fn treasure<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, treasure::TABLE_STATES, state as u32);
    match state {
        state::INIT => treasure_init(ctx, o),
        state::ACTIVE => treasure_active(ctx, o),
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 1: food and potions (0xdae26)

/// Type 1, consumables: variants 0 and 1 food (restores 20 or 40 hit points; at full health, a score and a sound), 2
/// and 3 timed potions (0x2d0 frames, player flags +0x180..0x184), 4 no effect, 5 counter `A5+0x45` (capped at 9). The
/// sparkle (+0x54) and the pickup index (+0x5d) come from a two-byte table per variant. Only variants 2 to 5 disappear
/// at the end of the lifetime (0x960 frames, 0x708 after a fall).
mod consumable {
    pub const ENTRY: u32 = 0xdae26;
    pub const TABLE_STATES: u32 = 0xdae3c;
    pub const TABLE_SUB_STATES: u32 = 0xdaef6;
    pub const PARAMS: u32 = 0xdaedc;
    pub const EFFECTS: u32 = 0xdafb8;
    pub const ANIMATIONS: u32 = 0x1bee16;
    pub const DURATION_INITIAL: u16 = 0x960;
    pub const DURATION_AFTER_FALL: u16 = 0x708;
    /// Pickup effects (original addresses, read from the `EFFECTS` table).
    pub const FOOD: u32 = 0xdafd0;
    pub const POTION_B: u32 = 0xdb044;
    pub const POTION_A: u32 = 0xdb026;
    pub const WITHOUT_EFFECT: u32 = 0xdb068;
    pub const COUNTER: u32 = 0xdb06a;
}

fn consumable_params<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let base = consumable::PARAMS + ((o.u8(ctx.b, Obj::VARIANT) as u32) * 2 & 0xff);
    let shimmering = ctx.prog8(base);
    o.set_u8(ctx.b, field::SHIMMER, shimmering);
    let index = ctx.prog8(base + 1);
    o.set_u8(ctx.b, field::INDEX, index);
}

fn consumable_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if airborne(ctx, o) {
        state_next(ctx, o, sub::FALL);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        o.set_u16(ctx.b, field::DURATION, consumable::DURATION_INITIAL);
        consumable_params(ctx, o);
    } else {
        state_next(ctx, o, sub::WAIT);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        o.set_u16(ctx.b, field::DURATION, consumable::DURATION_INITIAL);
        consumable_params(ctx, o);
        shimmer(ctx, o);
    }
    animate_and_sort(ctx, consumable::ANIMATIONS, variant);
}

/// Effect of a food: heals the player; at full health, score event and sound.
fn effect_food<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let player = pointer(ctx, o, field::OWNER);
    let (life_max, life) = (player.u16(ctx.b, 0x60), player.u16(ctx.b, 0x62));
    if !(life_max != life) {
        let event = 0x11 + ((player.u8(ctx.b, 2) as u16) << 8);
        crate::events::enqueue_points(ctx.b, event);
        effect_pickup(ctx, o, 5);
        return;
    }
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let heal: u16 = if variant == 0 { 0x14 } else { 0x28 };
    let life = life.wrapping_add(heal);
    player.set_u16(ctx.b, 0x62, life);
    if !(life <= life_max) {
        player.set_u16(ctx.b, 0x62, life_max);
    }
    crate::sound::play(ctx.b, call::END_EFFECT, ctx.obj());
}

/// Potions: the player receives a countdown of 0x2d0 frames and flags +0x180..0x184.
fn effect_potion<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, bytes: &[(u32, u8)]) {
    let player = pointer(ctx, o, field::OWNER);
    player.set_u16(ctx.b, 0x180, 0x2d0);
    for &(off, v) in bytes {
        player.set_u8(ctx.b, off, v);
    }
    crate::sound::play(ctx.b, call::END_EFFECT, ctx.obj());
}

fn consumable_effect<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT) as u32;
    let target = ctx.prog32(consumable::EFFECTS + 4 * variant);
    // the original's registers at the time of the `jsr (A1)` (useful if the variant is outside the table)
    match target {
        consumable::FOOD => effect_food(ctx, o),
        consumable::POTION_B => effect_potion(ctx, o, &[(1, 1), (0x183, 1), (0x184, 0), (0x182, 0)]),
        consumable::POTION_A => effect_potion(ctx, o, &[(0x184, 1), (0x183, 0), (0x182, 0)]),
        consumable::WITHOUT_EFFECT => {}
        consumable::COUNTER => {
            let n = ctx.ram8(0x45).wrapping_add(1);
            ctx.set_ram8(0x45, n);
            if !(n <= 9) {
                ctx.set_ram8(0x45, 9);
            }
        }
        _ => crate::families::effects::outside_table(target),
    }
    mark_pickup(ctx, o);
}

fn consumable_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = super::effects::target(ctx, consumable::TABLE_SUB_STATES, sub_state as u32);
    match sub_state {
        sub::FALL => {
            let in_flight = ctx.move_xyz().gt();
            if !in_flight {
                add_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u16(ctx.b, field::DURATION, consumable::DURATION_AFTER_FALL);
                o.set_u32(ctx.b, Obj::Z, 0);
                in_view(ctx, o);
                shimmer(ctx, o);
            }
            fall_end(ctx);
        }
        sub::WAIT => {
            let pickup = o.u8(ctx.b, field::PICKUP);
            if !(pickup == 0) {
                add_u8(ctx, o, field::SUB_STATE, 2);
            }
            // only variants 2 and following disappear
            let variant = o.u8(ctx.b, Obj::VARIANT);
            if (variant == 0) || (variant == 1) {
                return;
            }
            expire_and_blinks(ctx, o);
        }
        sub::PICKUP => {
            if !pickup_ready(ctx, o) {
                return;
            }
            add_u8(ctx, o, Obj::STATE, 2);
            consumable_effect(ctx, o);
        }
        _ => crate::families::effects::outside_table(target),
    }
}

fn consumable_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, consumable::TABLE_STATES, state as u32);
    match state {
        state::INIT => consumable_init(ctx, o),
        state::ACTIVE => consumable_active(ctx, o),
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 4: inventory objects (0xdbdf6)

/// Type 4, inventory objects (vials, secondary weapons, keys...). The player carries six slots (identifiers at
/// `+0x140`, quantities at `+0x148`; 0x56: empty slot). On pickup: refusal if the player's class cannot carry the
/// object (eight-byte table per variant); otherwise quantity + `+0x5a` (capped at 9, or 1 for variants 7, 0x10 and
/// 0x11), empty slot, or swap with the first slot, whose replaced object is recreated in the air (mode 1). A refused
/// object is thrown off in the air then becomes pickable again. The creation mode `+0x2f` is 1 for an object thrown by
/// a player: it waits `+0x59` frames before appearing.
mod inventory {
    pub const ENTRY: u32 = 0xdbdf6;
    pub const TABLE_STATES: u32 = 0xdbe0c;
    pub const TABLE_SUB_STATES: u32 = 0xdbf36;
    pub const ANIMATIONS: u32 = 0x1bf3c6;
    /// Sparkle per variant; pickup index per variant.
    pub const SHIMMERS: u32 = 0xdbf04;
    pub const INDICES: u32 = 0xdbf16;
    /// Per variant, 8 bytes: object identifier then one prohibition byte per class.
    pub const SHEETS: u32 = 0xdc1fa;
    /// Per player class, 8 identifiers of objects that cannot be replaced.
    pub const LOCKED: u32 = 0xdc1ca;
    /// Per object identifier: (type, variant) of the ITEM recreated when it is replaced.
    pub const RECREATED: u32 = 0xdc2d6;
    pub const DURATION_TO_GROUND: u16 = 0x4b0;
    pub const EMPTY: u8 = 0x56;
    /// Player fields.
    pub const IDS: u32 = 0x140;
    pub const QUANTITIES: u32 = 0x148;
    /// Quantity of the object (`+0x5a`) and delay before appearing (`+0x59`).
    pub const QUANTITY: u32 = 0x5a;
    pub const DELAY: u32 = 0x59;
}

/// `0xdc28a`: recreates in the air the object `old` (replaced in the inventory) with its quantity. `index`: D0.w of the
/// caller, the identifier in the low byte (the high byte is non-zero only with a real player, see `equipment_add`).
fn object_ejected<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, player: Obj, index: u16, quantity: u8) {
    let Some(n) = ctx.alloc(&pools::ITEM) else {
        return;
    };
    n.set_u8(ctx.b, Obj::ACTIVE, 1);
    n.set_u8(ctx.b, field::MODE, 1);
    n.set_u8(ctx.b, inventory::QUANTITY, quantity);
    // `add.w D0,D0` then `(d8,PC,D0.w)`: sign-extended word
    let rank = index.wrapping_mul(2) as i16 as i32 as u32;
    let kind = ctx.prog8(inventory::RECREATED.wrapping_add(rank));
    n.set_u8(ctx.b, Obj::KIND, kind);
    let variant = ctx.prog8(inventory::RECREATED.wrapping_add(rank).wrapping_add(1));
    n.set_u8(ctx.b, Obj::VARIANT, variant);
    n.set_u16(ctx.b, field::OWNER, player.0 as u16);
    let x = o.u16(ctx.b, Obj::X);
    n.set_u16(ctx.b, Obj::X, x);
    let y = player.u16(ctx.b, Obj::Y).wrapping_sub(1);
    n.set_u16(ctx.b, Obj::Y, y);
    n.set_u16(ctx.b, Obj::Z, 0x10);
    n.set_u16(ctx.b, Obj::AZ, 0xffc0);
    n.set_u8(ctx.b, inventory::DELAY, 0xc);
}

/// Pickup refused (`0xdc19a`): the object will be thrown off (substate 6).
fn pickup_refused<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    o.set_u8(ctx.b, field::SUB_STATE, 6);
    o.set_u8(ctx.b, field::SUB_SUB_STATE, 0);
    o.set_u8(ctx.b, inventory::DELAY, 0xc);
    true
}

/// Score event and sound of an object that changes nothing else (`0xdc124`).
fn event_inventory<R: Bench>(ctx: &mut Cx<'_, R>, player: Obj) {
    let character = player.u8(ctx.b, 2) as u32;
    crate::events::enqueue_points(ctx.b, ((character << 8) + 0x11) as u16);
    effect_pickup(ctx, ctx.obj(), 5);
}

/// `0xdc0aa`: stores the object in the owner's inventory. Returns true if the pickup is refused.
fn inventory_add<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let player = pointer(ctx, o, field::OWNER);
    let line = (o.u8(ctx.b, Obj::VARIANT) as u32) << 3 & 0xffff;
    let id = ctx.data8(inventory::SHEETS + line);
    let class = player.u8(ctx.b, 3) >> 1;
    let forbidden = ctx.data8(inventory::SHEETS + 1 + (class.wrapping_add(line as u8)) as u32);
    if forbidden != 0 {
        return pickup_refused(ctx, o);
    }
    if !(id != 0x23) && (class == 2) {
        event_inventory(ctx, player);
        return false;
    }
    let quantity = o.u8(ctx.b, inventory::QUANTITY);
    // 1. the object is already carried: the quantity increases
    for i in 0..6u32 {
        let door = player.u8(ctx.b, inventory::IDS + i);
        if door != id {
            continue;
        }
        let n = player.u8(ctx.b, inventory::QUANTITIES + i).wrapping_add(quantity);
        player.set_u8(ctx.b, inventory::QUANTITIES + i, n);
        let variant = o.u8(ctx.b, Obj::VARIANT);
        // single-copy variants: 7, 0x10, 0x11
        let unique = (variant == 7) || (variant == 0x10) || (variant == 0x11);
        if unique {
            player.set_u8(ctx.b, inventory::QUANTITIES + i, 1);
        } else if !(n <= 9) {
            player.set_u8(ctx.b, inventory::QUANTITIES + i, 9);
        } else {
            return false;
        }
        event_inventory(ctx, player);
        return false;
    }
    // 2. an empty slot
    for i in 0..6u32 {
        let door = player.u8(ctx.b, inventory::IDS + i);
        if door != inventory::EMPTY {
            continue;
        }
        player.set_u8(ctx.b, inventory::IDS + i, id);
        player.set_u8(ctx.b, inventory::QUANTITIES + i, quantity);
        return false;
    }
    // 3. swap with the first slot, unless it is locked for this class
    let first = player.u8(ctx.b, inventory::IDS);
    for k in 0..8u32 {
        let lock = ctx.data8(inventory::LOCKED + ((class as u32) << 3) + k);
        if !(first != lock) {
            return pickup_refused(ctx, o);
        }
    }
    let old_quantity = player.u8(ctx.b, inventory::QUANTITIES);
    player.set_u8(ctx.b, inventory::IDS, id);
    player.set_u8(ctx.b, inventory::QUANTITIES, quantity);
    object_ejected(ctx, o, player, first as u16, old_quantity);
    false
}

fn inventory_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let mode = o.u8(ctx.b, field::MODE);
    if mode != 0 {
        // thrown object: waits for the end of the delay
        let delay = o.u8(ctx.b, inventory::DELAY);
        if !(delay == 0) {
            o.set_u8(ctx.b, inventory::DELAY, delay - 1);
            return;
        }
        add_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        let index = ctx.data8(inventory::INDICES + variant as u32);
        o.set_u8(ctx.b, field::INDEX, index);
        animate_and_sort(ctx, inventory::ANIMATIONS, variant);
        return;
    }
    if airborne(ctx, o) {
        state_next(ctx, o, sub::FALL);
        o.set_u8(ctx.b, inventory::QUANTITY, 1);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        let shimmering = ctx.data8(inventory::SHIMMERS + variant as u32);
        o.set_u8(ctx.b, field::SHIMMER, shimmering);
        let index = ctx.data8(inventory::INDICES + variant as u32);
        o.set_u8(ctx.b, field::INDEX, index);
    } else {
        state_next(ctx, o, sub::WAIT);
        o.set_u8(ctx.b, inventory::QUANTITY, 1);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        o.set_u16(ctx.b, field::DURATION, inventory::DURATION_TO_GROUND);
        let shimmering = ctx.data8(inventory::SHIMMERS + variant as u32);
        o.set_u8(ctx.b, field::SHIMMER, shimmering);
        let index = ctx.data8(inventory::INDICES + variant as u32);
        o.set_u8(ctx.b, field::INDEX, index);
        shimmer(ctx, o);
    }
    animate_and_sort(ctx, inventory::ANIMATIONS, variant);
}

/// Substate 6: the refused object is thrown off in an arc toward the outside of the screen then falls back.
fn inventory_reject<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let launches = o.u8(ctx.b, field::SUB_SUB_STATE) != 0;
    if !launches {
        // wait for the delay, then throw
        let delay = o.u8(ctx.b, inventory::DELAY).wrapping_sub(1);
        o.set_u8(ctx.b, inventory::DELAY, delay);
        if delay != 0 {
            return;
        }
        add_u8(ctx, o, field::SUB_SUB_STATE, 2);
        let player = pointer(ctx, o, field::OWNER);
        let y = player.u16(ctx.b, Obj::Y).wrapping_sub(1);
        o.set_u16(ctx.b, Obj::Y, y);
        o.set_u16(ctx.b, Obj::Z, 0x10);
        o.set_u16(ctx.b, Obj::VZ, 0);
        o.set_u16(ctx.b, Obj::AZ, 0xffc0);
        o.set_u32(ctx.b, Obj::VX, 0x0200_0000);
        let (cam_x, _) = ctx.camera();
        let limit = cam_x.wrapping_add(0xc0);
        let x = o.u16(ctx.b, Obj::X);
        if !(limit as i16 > x as i16) {
            let v = o.u32(ctx.b, Obj::VX).wrapping_neg();
            o.set_u32(ctx.b, Obj::VX, v);
        }
        ctx.reinsert();
        return;
    }
    ctx.move_xz();
    let z = o.u16(ctx.b, Obj::Z);
    if z as i16 >= 0 {
        return;
    }
    o.set_u8(ctx.b, field::SUB_STATE, sub::WAIT);
    o.set_u8(ctx.b, field::SUB_SUB_STATE, 0);
    o.set_u32(ctx.b, Obj::Z, 0);
    o.set_u8(ctx.b, field::PICKUP, 0);
}

fn inventory_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = super::effects::target(ctx, inventory::TABLE_SUB_STATES, sub_state as u32);
    match sub_state {
        sub::FALL => {
            let in_flight = ctx.move_xyz().gt();
            if !in_flight {
                add_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u16(ctx.b, field::DURATION, inventory::DURATION_TO_GROUND);
                o.set_u32(ctx.b, Obj::Z, 0);
                let mode = o.u8(ctx.b, field::MODE);
                if !(mode != 0) {
                    in_view(ctx, o);
                    shimmer(ctx, o);
                }
            }
            ctx.anim_step();
            fall_end(ctx);
        }
        sub::WAIT => {
            let pickup = o.u8(ctx.b, field::PICKUP);
            if !(pickup == 0) {
                add_u8(ctx, o, field::SUB_STATE, 2);
            }
            ctx.anim_step();
            expire_and_blinks(ctx, o);
        }
        sub::PICKUP => {
            let player = pointer(ctx, o, field::OWNER);
            let ready = player.u8(ctx.b, 0x33) != 0;
            if !ready {
                return;
            }
            let refuse = inventory_add(ctx, o);
            if !refuse {
                add_u8(ctx, o, Obj::STATE, 2);
            }
            mark_pickup(ctx, o);
        }
        6 => inventory_reject(ctx, o),
        _ => crate::families::effects::outside_table(target),
    }
}

fn inventory_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, inventory::TABLE_STATES, state as u32);
    match state {
        state::INIT => inventory_init(ctx, o),
        state::ACTIVE => inventory_active(ctx, o),
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 5: animated bonuses (0xdc382)

/// Type 5, animated bonuses: thrown in the air according to a creation mode (`+0x2f`: initial speeds read from an
/// eight-byte table per mode), they fall back, wait 0x4b0 frames blinking and give on pickup a score event (`0x19b08`,
/// variant + player).
mod bonus {
    pub const ENTRY: u32 = 0xdc382;
    pub const TABLE_STATES: u32 = 0xdc398;
    pub const TABLE_SUB_STATES: u32 = 0xdc4f2;
    pub const ANIMATIONS: u32 = 0x1bf5c2;
    /// Per mode: vx, vz, az, vy (words), read in program space.
    pub const SPEEDS: u32 = 0xdc42c;
    pub const DURATION_TO_GROUND: u16 = 0x4b0;
    /// Gap between the falling animation and the ground animation, and ground animation of a placed object.
    pub const ANIMATION_GROUND: u8 = 6;
    pub const ANIMATION_POSE: u8 = 0xc;
}

fn bonus_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if airborne(ctx, o) {
        state_next(ctx, o, sub::FALL);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        let mode = o.u8(ctx.b, field::MODE);
        if !(mode == 0) {
            let base = bonus::SPEEDS + ((mode as u32) << 3);
            for (i, off) in [Obj::VX, Obj::VZ, Obj::AZ, Obj::VY].into_iter().enumerate() {
                let v = ctx.prog16(base + 2 * i as u32);
                o.set_u16(ctx.b, off, v);
            }
        }
        animate_and_sort(ctx, bonus::ANIMATIONS, variant);
    } else {
        state_next(ctx, o, sub::WAIT);
        o.set_u16(ctx.b, field::DURATION, bonus::DURATION_TO_GROUND);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        animate_and_sort(ctx, bonus::ANIMATIONS, variant.wrapping_add(bonus::ANIMATION_POSE));
    }
}

fn bonus_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = super::effects::target(ctx, bonus::TABLE_SUB_STATES, sub_state as u32);
    match sub_state {
        sub::FALL => {
            ctx.anim_step();
            let in_flight = ctx.move_xyz().gt();
            if !in_flight {
                add_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u16(ctx.b, field::DURATION, bonus::DURATION_TO_GROUND);
                o.set_u32(ctx.b, Obj::Z, 0);
                let variant = o.u8(ctx.b, Obj::VARIANT);
                ctx.anim_start(bonus::ANIMATIONS, variant.wrapping_add(bonus::ANIMATION_GROUND) as u16);
                in_view(ctx, o);
            }
            fall_end(ctx);
        }
        sub::WAIT => {
            let pickup = o.u8(ctx.b, field::PICKUP);
            if !(pickup == 0) {
                add_u8(ctx, o, field::SUB_STATE, 2);
            }
            ctx.anim_step();
            expire_and_blinks(ctx, o);
        }
        sub::PICKUP => {
            if !pickup_ready(ctx, o) {
                return;
            }
            add_u8(ctx, o, Obj::STATE, 2);
            let player_bits = o.u16(ctx.b, 0x48) & 0x300;
            let event = (o.u8(ctx.b, Obj::VARIANT) as u16).wrapping_add(player_bits);
            crate::events::enqueue_counter(ctx.b, event);
        }
        _ => crate::families::effects::outside_table(target),
    }
}

fn bonus_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, bonus::TABLE_STATES, state as u32);
    match state {
        state::INIT => bonus_init(ctx, o),
        state::ACTIVE => bonus_active(ctx, o),
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

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
mod scene {
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
}

/// `0xdcdae`: places itself next to the owner (one pixel in front, offset according to the orientation). Returns the
/// owner.
fn follow_owner<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> Obj {
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

fn icon_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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
fn icon_sound<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, base: u32) {
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
    crate::sound::play(ctx.b, sound, ctx.obj());
}

/// Substate 0 of the icon: follows the player, animates, sorts, and plays the sound drawn every 0x3c frames.
fn icon_followed<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    follow_owner(ctx, o);
    ctx.anim_step();
    o.set_u8(ctx.b, 0x3b, 0);
    ctx.reinsert();
    icon_sound(ctx, o, 0);
}

/// Flight speeds (`0xdc7c2`): flipped with the orientation, then animation step.
fn icon_takeoff_speed<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let flip = o.u8(ctx.b, field::FLIP) != 0;
    if flip {
        let v = o.u16(ctx.b, Obj::VX).wrapping_neg();
        o.set_u16(ctx.b, Obj::VX, v);
    }
    ctx.anim_step();
}

/// Substate 2 of the icon: the icon flies off in an arc (four steps, according to `+6`).
fn icon_takeoff<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let time = o.u8(ctx.b, field::SUB_SUB_STATE);
    let target = super::effects::target(ctx, scene::ICON_TAKEOFF, time as u32);
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
fn icon_wait<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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
    pub const SOUND_STOP: u32 = 0xe1d60;
    /// Icon counter of the object `+0x46`.
    pub const ICONS: u32 = 0x30;
    /// Stop requested (`+0x33`).
    pub const STOP: u32 = 0x33;
}

fn start_launches<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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
            crate::sound::play(ctx.b, sound, o);
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
fn start_countdown<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let carrier = pointer(ctx, o, scene::OWNER_2);
    let n = carrier.u8(ctx.b, start::ICONS).wrapping_sub(1);
    carrier.set_u8(ctx.b, start::ICONS, n);
}

/// Substate 2: walks in X; on leaving the screen, the icon is freed.
fn start_running<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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
fn start_stop<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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

fn icon_start<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = super::effects::target(ctx, start::SUB_STATES, sub_state as u32);
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

fn icon<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, scene::ICON_STATES, state as u32);
    match state {
        state::INIT => icon_init(ctx, o),
        state::ACTIVE => {
            let sub_state = o.u8(ctx.b, field::SUB_STATE);
            let target = super::effects::target(ctx, scene::ICON_SUB_STATES, sub_state as u32);
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
fn guide_target<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, a2: Option<u32>) {
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

fn guide_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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
fn guide_animation<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let mut animation = 0x12;
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if !(variant == 3) {
        let mode = o.u8(ctx.b, field::MODE) as u32;
        animation = ctx.prog8(scene::GUIDE_ANIMATIONS_ARRIVAL + mode);
    }
    ctx.anim_start(scene::ANIMATIONS, animation as u16);
}

/// Creates a SET object of type `type_set`; returns its address, or `None` if the pool is full.
fn created_set<R: Bench>(ctx: &mut Cx<'_, R>, type_set: u32) -> Option<Obj> {
    scene_set_of(ctx, type_set as u8)
}

/// Substate 0 of the guide: walks toward the arrival point; once arrived, changes animation and creates a SET.
fn guide_running<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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
fn guide_wait<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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
fn guide_second_object<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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
fn guide_end<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let set = pointer(ctx, o, scene::CREATED);
    let in_progress = set.u8(ctx.b, 0x22) == 0;
    if !in_progress {
        add_u8(ctx, o, Obj::STATE, 2);
    }
}

fn guide_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let frozen = ctx.ram8(scene::SCENE_FROZEN) != 0;
    if !frozen {
        let sub_state = o.u8(ctx.b, field::SUB_STATE);
        let target = super::effects::target(ctx, scene::GUIDE_SUB_STATES, sub_state as u32);
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

fn guide<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, scene::GUIDE_STATES, state as u32);
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

fn guide_3_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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
fn guide_3_wait<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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

fn guide_3<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, guide_3::STATES, state as u32);
    match state {
        state::INIT => guide_3_init(ctx, o),
        state::ACTIVE => {
            let sub_state = o.u8(ctx.b, field::SUB_STATE);
            let target = super::effects::target(ctx, guide_3::SUB_STATES, sub_state as u32);
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

fn scene_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let target = super::effects::target(ctx, scene::TABLE_VARIANTS, 2 * variant as u32);
    match variant {
        0 | 2 => icon(ctx, o),
        1 => guide(ctx, o),
        3 => guide_3(ctx, o),
        _ => crate::families::effects::outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 7: player reserve (0xdce6e)

/// Type 7, objects that feed a player reserve (counters `+0x150 + variant`, capped at 9) for the authorized characters
/// (table per character, then per class). Same cycle as type 4: fall, wait (0x4b0 frames, blinking), pickup (refused:
/// thrown off in an arc), mode 1: thrown object.
mod reserve {
    pub const ENTRY: u32 = 0xdce6e;
    pub const TABLE_STATES: u32 = 0xdce84;
    pub const TABLE_SUB_STATES: u32 = 0xdcf38;
    pub const ANIMATIONS: u32 = 0x1c001e;
    pub const DURATION_TO_GROUND: u16 = 0x4b0;
    /// Per character (program space): is the object usable?
    pub const CHARACTERS: u32 = 0xdd01e;
    /// Per class (data): reserve accepted?
    pub const CLASSES: u32 = 0xdd122;
    pub const COUNTERS: u32 = 0x150;
}

fn reserve_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let mode = o.u8(ctx.b, field::MODE);
    if mode != 0 {
        let delay = o.u8(ctx.b, inventory::DELAY);
        if !(delay == 0) {
            o.set_u8(ctx.b, inventory::DELAY, delay - 1);
            return;
        }
        add_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        return animate_and_sort(ctx, reserve::ANIMATIONS, variant.wrapping_add(0xa));
    }
    if airborne(ctx, o) {
        state_next(ctx, o, sub::FALL);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
    } else {
        state_next(ctx, o, sub::WAIT);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        o.set_u16(ctx.b, field::DURATION, reserve::DURATION_TO_GROUND);
    }
    animate_and_sort(ctx, reserve::ANIMATIONS, variant);
}

/// Wait substate: the player who touches the object (`PICKUP`) makes it go to the pickup.
fn reserve_wait<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let pickup = o.u8(ctx.b, field::PICKUP);
    if !(pickup == 0) {
        add_u8(ctx, o, field::SUB_STATE, 2);
    }
    expire_and_blinks(ctx, o);
}

/// `0xdd0da`: adds the object to the owner's reserve. True if the pickup is refused.
fn reserve_add<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let player = pointer(ctx, o, field::OWNER);
    let class = (player.u8(ctx.b, 3) >> 1) as u32;
    let accepted = ctx.data8(reserve::CLASSES + class);
    if !(accepted != 0) {
        o.set_u8(ctx.b, field::SUB_STATE, 6);
        o.set_u8(ctx.b, field::SUB_SUB_STATE, 0);
        o.set_u8(ctx.b, inventory::DELAY, 0xc);
        return true;
    }
    let variant = o.u8(ctx.b, Obj::VARIANT) as u32;
    let counter = player.0 + reserve::COUNTERS + variant;
    let n = crate::mem::rd8(ctx.b, counter).wrapping_add(1);
    crate::mem::wr8(ctx.b, counter, n);
    if !(n <= 9) {
        crate::mem::wr8(ctx.b, counter, 9);
    }
    false
}

fn reserve_pickup<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let player = pointer(ctx, o, field::OWNER);
    let pickup = o.u8(ctx.b, field::PICKUP);
    if !(pickup == 2) {
        let ready = player.u8(ctx.b, 0x33) != 0;
        if !ready {
            return;
        }
    }
    let character = player.u8(ctx.b, 3) as u32;
    let allowed = ctx.prog8(reserve::CHARACTERS + character);
    if !(allowed != 0) {
        add_u8(ctx, o, field::SUB_STATE, 0xfe);
        o.set_u8(ctx.b, field::PICKUP, 0);
        return;
    }
    let refuse = reserve_add(ctx, o);
    if !refuse {
        add_u8(ctx, o, Obj::STATE, 2);
    }
}

fn reserve_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = super::effects::target(ctx, reserve::TABLE_SUB_STATES, sub_state as u32);
    match sub_state {
        sub::FALL => {
            let pose = o.u8(ctx.b, field::SUB_SUB_STATE);
            if !(pose != 0) {
                let in_flight = ctx.move_xyz().gt();
                if !in_flight {
                    add_u8(ctx, o, field::SUB_SUB_STATE, 2);
                    o.set_u16(ctx.b, field::DURATION, reserve::DURATION_TO_GROUND);
                    o.set_u32(ctx.b, Obj::Z, 0);
                    let mode = o.u8(ctx.b, field::MODE);
                    if !(mode != 0) {
                        in_view(ctx, o);
                    }
                }
                return fall_end(ctx);
            }
            let mode = o.u8(ctx.b, field::MODE);
            if !(mode == 0) {
                add_u8(ctx, o, field::SUB_STATE, 2);
                return reserve_wait(ctx, o);
            }
            let end = ctx.anim_step();
            if end {
                add_u8(ctx, o, field::SUB_STATE, 2);
            }
        }
        sub::WAIT => reserve_wait(ctx, o),
        sub::PICKUP => reserve_pickup(ctx, o),
        6 => inventory_reject(ctx, o),
        _ => crate::families::effects::outside_table(target),
    }
}

fn reserve_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, reserve::TABLE_STATES, state as u32);
    match state {
        state::INIT => reserve_init(ctx, o),
        state::ACTIVE => reserve_active(ctx, o),
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 9: icon that accompanies an object (0xdd5a2)

/// Type 9: interface spark that follows an object (`+0x5c`), one pixel in front of it, for `+0x14` frames (the count is
/// prepared by the creator), then disappears. Palette according to the variant.
mod escort {
    pub const ENTRY: u32 = 0xdd5a2;
    pub const PALETTES: u32 = 0xdd5de;
    pub const ANIMATIONS: u32 = 0x1c03a6;
    pub const TARGET: u32 = 0x5c;
    pub const DURATION: u32 = 0x14;
}

fn escort_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    if !(state != 0) {
        add_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, 0x2a, 1);
        let variant = o.u8(ctx.b, Obj::VARIANT);
        let palette = ctx.prog8(escort::PALETTES + variant as u32);
        o.set_u8(ctx.b, 0x2b, palette);
        o.set_u16(ctx.b, escort::DURATION, 0x100);
        ctx.anim_start(escort::ANIMATIONS, 0x8e + variant as u16);
        return ctx.reinsert();
    }
    let target = pointer(ctx, o, escort::TARGET);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = target.u16(ctx.b, off);
        o.set_u16(ctx.b, off, v);
    }
    let y = o.u16(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u16(ctx.b, Obj::Y, y);
    ctx.anim_step();
    let remainder = o.u16(ctx.b, escort::DURATION).wrapping_sub(1);
    o.set_u16(ctx.b, escort::DURATION, remainder);
    if remainder as i16 >= 0 {
        return;
    }
    ctx.free(&pools::ITEM);
}

// ---------------------------------------------------------------------------------------------
// Type 8: contact objects (0xdd132)

/// Type 8, objects picked up by simple contact (box test with the four players, `0xdd470`) that trigger a scene effect
/// through a SET object (`0xc8312`): three variants.
///
/// - 0: placed bonus that waits for `+0x58` (touched by a player), creates a SET (`+0x42`) and follows it;
/// - 1: bonus that oscillates horizontally (speed `+0x14`, about-turn every 0x3c frames) until a
///   player touches it;
/// - 2: same principle as 0, without oscillation.
///
/// `A5-0x5ac4` is non-zero as long as a player is picking one up: only one at a time.
mod contact {
    pub const ENTRY: u32 = 0xdd132;
    pub const TABLE_VARIANTS: u32 = 0xdd142;
    pub const ANIMATIONS: u32 = 0x1bf8fe;
    // variant 0
    pub const V0_STATES: u32 = 0xdd156;
    pub const V0_SUB_STATES: u32 = 0xdd190;
    pub const V0_ANIMATIONS: u32 = 0xdd180;
    pub const V0_ANIMATIONS_KEY: u32 = 0xdd1d2;
    pub const V0_ANIMATIONS_SET: u32 = 0xdd22a;
    pub const V0_ANIMATIONS_END: u32 = 0xdd246;
    // variant 1
    pub const V1_STATES: u32 = 0xdd280;
    pub const V1_SUB_STATES: u32 = 0xdd2d4;
    // variant 2
    pub const V2_STATES: u32 = 0xdd3ca;
    pub const V2_SUB_STATES: u32 = 0xdd400;
    /// Lock: a player is picking up (A5-0x5ac4) and the event set in `A5+0x5e`.
    pub const BUSY: i32 = -0x5ac4;
    pub const SET_CREATED: u32 = 0x42;
    pub const DELAY: u32 = 0x4c;
    /// Contact boxes: one per type (`+0x39`) for the object, one per character and per `+0x38` for the player.
    pub const BOXES_OBJECT: u32 = 0xdd590;
    pub const BOXES_PLAYER: u32 = 0xdd578;
    pub const PLAYERS: u32 = 0xff85de;
}

/// `0xdd470`: does a player touch the object? If so, the substate advances, the lock is lifted and the player becomes
/// the owner.
fn contact_player<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let busy = ctx.ram8(contact::BUSY) != 0;
    if busy {
        return false;
    }
    let hitbox = o.u8(ctx.b, 0x39) as u32;
    if hitbox == 0 {
        return false;
    }
    let my_box = contact::BOXES_OBJECT + (hitbox << 3);
    for i in 0..4u32 {
        let player = Obj(contact::PLAYERS + i * 0x200);
        if player_key(ctx, o, player, my_box) {
            add_u8(ctx, o, field::SUB_STATE, 2);
            ctx.set_ram8(contact::BUSY, 1);
            o.set_u16(ctx.b, field::OWNER, player.0 as u16);
            return true;
        }
    }
    false
}

/// Is the player present, alive, free, and does its box overlap the object's?
fn player_key<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, player: Obj, my_box: u32) -> bool {
    let absent = player.u8(ctx.b, Obj::ACTIVE) != 1;
    if absent {
        return false;
    }
    let dead = player.u8(ctx.b, Obj::STATE) as i8 >= 4;
    if dead {
        return false;
    }
    let busy = player.u8(ctx.b, 0x182) != 0;
    if busy {
        return false;
    }
    let number = player.u8(ctx.b, 0x38) as u32;
    if number == 0 {
        return false;
    }
    let class = (player.u8(ctx.b, 3) >> 1) as u32;
    let table = ctx.data32(contact::BOXES_PLAYER + (class << 2));
    let its_box = table + (number << 3);
    overlaps(ctx, o, player, my_box, its_box)
}

/// The three box overlaps (X, Y then Z) between the object `o` (box `a`) and the player (box `b`).
fn overlaps<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, player: Obj, a: u32, b: u32) -> bool {
    let byte = |ctx: &mut Cx<'_, R>, address: u32| ctx.data8(address) as i8 as i16 as u16;
    // X: the offsets invert with the orientation of each
    let mut da = byte(ctx, a);
    let turned = o.u8(ctx.b, field::FLIP) != 0;
    if turned {
        da = da.wrapping_neg();
    }
    let mut db = byte(ctx, b);
    let player_turned = player.u8(ctx.b, field::FLIP) != 0;
    if player_turned {
        db = db.wrapping_neg();
    }
    let gap = da.wrapping_add(o.u16(ctx.b, Obj::X)).wrapping_sub(db).wrapping_sub(player.u16(ctx.b, Obj::X));
    let half = ctx.data8(a + 1).wrapping_add(ctx.data8(b + 1)) as u16;
    if gap.wrapping_add(half) > half.wrapping_mul(2) {
        return false;
    }
    // Y then Z: fixed offsets
    for (offset, size, position) in [(4u32, 5u32, Obj::Y), (2, 3, Obj::Z)] {
        let da = byte(ctx, a + offset);
        let db = byte(ctx, b + offset);
        let gap = da.wrapping_add(o.u16(ctx.b, position)).wrapping_sub(db.wrapping_add(player.u16(ctx.b, position)));
        let half = ctx.data8(a + size).wrapping_add(ctx.data8(b + size)) as u16;
        if gap.wrapping_add(half) > half.wrapping_mul(2) {
            return false;
        }
    }
    true
}

/// `0xdd16e`: starts animation `animation`, then sorts the object.
fn contact_animate<R: Bench>(ctx: &mut Cx<'_, R>, animation: u8) {
    ctx.anim_start(contact::ANIMATIONS, animation as u16);
    ctx.reinsert();
}

/// Animation drawn from a table indexed by the mode (`+0x2f`).
fn contact_animate_mode<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, table: u32) {
    let mode = o.u8(ctx.b, field::MODE) as u32;
    let animation = ctx.prog8(table + mode);
    contact_animate(ctx, animation);
}

/// Creates the SET object of the pickup scene (type `+0x42`, word) and attaches it to the object. False if the type is
/// negative (no scene) or if the pool is full.
fn contact_created_set<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let word = o.u16(ctx.b, contact::SET_CREATED);
    if word as i16 <= -1 {
        return false;
    }
    let Some(new) = scene_set_of(ctx, word as u8) else {
        return false;
    };
    o.set_u16(ctx.b, contact::SET_CREATED, new.0 as u16);
    let owner = pointer(ctx, o, field::OWNER);
    let character = owner.u8(ctx.b, 2);
    new.set_u8(ctx.b, 0x37, character);
    true
}

/// Orients the object toward the side where the owner is (carry of `x_owner - x`).
fn contact_facing<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let owner = pointer(ctx, o, field::OWNER);
    let (x_owner, x) = (owner.u16(ctx.b, Obj::X), o.u16(ctx.b, Obj::X));
    let flip = if x_owner < x { 0 } else { 0x20 };
    o.set_u8(ctx.b, field::FLIP, flip);
}

/// Has the SET object finished? (`+0x22` of the SET object non-zero.)
fn contact_set_finished<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let set = pointer(ctx, o, contact::SET_CREATED);
    let finished = set.u8(ctx.b, 0x22) != 0;
    finished
}

/// Variant 0, end of the scene (`0xdd22e`): lifts the lock and goes to the flee substate.
fn contact_0_end<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ctx.set_ram8(contact::BUSY, 0);
    o.set_u8(ctx.b, field::SUB_STATE, 6);
    contact_animate_mode(ctx, o, contact::V0_ANIMATIONS_END);
}

fn shifted_x<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, step: u16) {
    let x = o.u16(ctx.b, Obj::X).wrapping_add(step);
    o.set_u16(ctx.b, Obj::X, x);
}

fn contact_0<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, contact::V0_STATES, state as u32);
    match state {
        state::INIT => {
            add_u8(ctx, o, Obj::STATE, 2);
            let mode = o.u8(ctx.b, field::MODE) as u32;
            let animation = ctx.prog8(contact::V0_ANIMATIONS + mode);
            o.set_u8(ctx.b, field::SUB_STATE, 0);
            contact_animate(ctx, animation);
        }
        state::ACTIVE => {
            let sub_state = o.u8(ctx.b, field::SUB_STATE);
            let target = super::effects::target(ctx, contact::V0_SUB_STATES, sub_state as u32);
            match sub_state {
                0 => {
                    ctx.anim_step();
                    let pickup = o.u8(ctx.b, field::PICKUP);
                    if pickup == 0 {
                        return;
                    }
                    let busy = ctx.ram8(contact::BUSY) != 0;
                    if busy {
                        o.set_u8(ctx.b, field::PICKUP, 0);
                        return;
                    }
                    ctx.set_ram8(contact::BUSY, 1);
                    add_u8(ctx, o, field::SUB_STATE, 2);
                    shard(ctx, o);
                    contact_animate_mode(ctx, o, contact::V0_ANIMATIONS_KEY);
                }
                2 => {
                    ctx.anim_step();
                    let key = o.u8(ctx.b, 0x33);
                    if key == 1 {
                        return;
                    }
                    add_u8(ctx, o, field::SUB_STATE, 2);
                    if !contact_created_set(ctx, o) {
                        return contact_0_end(ctx, o);
                    }
                    contact_facing(ctx, o);
                    contact_animate_mode(ctx, o, contact::V0_ANIMATIONS_SET);
                }
                4 => {
                    if contact_set_finished(ctx, o) {
                        contact_0_end(ctx, o);
                    }
                }
                6 => {
                    let flip = o.u8(ctx.b, field::FLIP) != 0;
                    let step: u16 = if flip { 3 } else { 3u16.wrapping_neg() };
                    shifted_x(ctx, o, step);
                    ctx.anim_step();
                    ctx.cull_offscreen();
                }
                _ => crate::families::effects::outside_table(target),
            }
        }
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

/// Variant 1: swinging animation (`0xdd2a8`), sort and edge adjustment.
fn contact_1_animate<R: Bench>(ctx: &mut Cx<'_, R>) {
    contact_animate(ctx, 0x18);
    ctx.keep_in_zone();
    ctx.push_terrain();
}

/// Variant 1: horizontal movement `X += VX << 8`.
fn contact_1_advance<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let vx = o.u16(ctx.b, Obj::VX) as i16 as i32;
    let x = o.u32(ctx.b, Obj::X).wrapping_add((vx << 8) as u32);
    o.set_u32(ctx.b, Obj::X, x);
}

/// Variant 1: speed 0x200 in the direction of the orientation (`0xdd296`).
fn contact_1_speed<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    o.set_u16(ctx.b, Obj::VX, 0x200);
    let flip = o.u8(ctx.b, field::FLIP) != 0;
    if !flip {
        let v = o.u16(ctx.b, Obj::VX).wrapping_neg();
        o.set_u16(ctx.b, Obj::VX, v);
    }
}

/// Variant 1, scene exit (`0xdd37c`): the lock is lifted, the bonus moves away.
fn contact_1_exit<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    contact_1_speed(ctx, o);
    ctx.set_ram8(contact::BUSY, 0);
    o.set_u8(ctx.b, field::SUB_STATE, 6);
    contact_1_animate(ctx);
}

fn contact_1<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, contact::V1_STATES, state as u32);
    match state {
        state::INIT => {
            add_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, field::SUB_STATE, 0);
            o.set_u8(ctx.b, contact::DELAY, 0x3c);
            contact_1_speed(ctx, o);
            contact_1_animate(ctx);
        }
        state::ACTIVE => {
            let sub_state = o.u8(ctx.b, field::SUB_STATE);
            let target = super::effects::target(ctx, contact::V1_SUB_STATES, sub_state as u32);
            match sub_state {
                0 => {
                    let key = contact_player(ctx, o);
                    if key {
                        // no next contact: the substates advance by themselves
                        ctx.set_ram8(0x5e, 1);
                        ctx.anim_start(contact::ANIMATIONS, 0x19);
                        return;
                    }
                    contact_1_advance(ctx, o);
                    let remainder = o.u8(ctx.b, contact::DELAY).wrapping_sub(1);
                    o.set_u8(ctx.b, contact::DELAY, remainder);
                    if !(remainder != 0) {
                        o.set_u8(ctx.b, contact::DELAY, 0x3c);
                        let flip = o.u8(ctx.b, field::FLIP) ^ 0x20;
                        o.set_u8(ctx.b, field::FLIP, flip);
                        let v = o.u16(ctx.b, Obj::VX).wrapping_neg();
                        o.set_u16(ctx.b, Obj::VX, v);
                    }
                    ctx.anim_step();
                    ctx.reinsert();
                    ctx.keep_in_zone();
                    ctx.push_terrain();
                }
                2 => {
                    add_u8(ctx, o, field::SUB_STATE, 2);
                    if !contact_created_set(ctx, o) {
                        return contact_1_exit(ctx, o);
                    }
                    contact_facing(ctx, o);
                }
                4 => {
                    let set = pointer(ctx, o, contact::SET_CREATED);
                    let in_progress = set.u8(ctx.b, 0x22) == 0;
                    if !in_progress {
                        contact_1_exit(ctx, o);
                    }
                }
                6 => {
                    contact_1_advance(ctx, o);
                    ctx.anim_step();
                    ctx.reinsert();
                    ctx.cull_offscreen();
                }
                _ => crate::families::effects::outside_table(target),
            }
        }
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

/// Variant 2, end of the scene (`0xdd44e`).
fn contact_2_end<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ctx.set_ram8(contact::BUSY, 0);
    add_u8(ctx, o, field::SUB_STATE, 2);
}

fn contact_2<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, contact::V2_STATES, state as u32);
    match state {
        state::INIT => {
            add_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, field::SUB_STATE, 0);
            let mode = o.u8(ctx.b, field::MODE);
            contact_animate(ctx, mode.wrapping_add(0x14));
        }
        state::ACTIVE => {
            let sub_state = o.u8(ctx.b, field::SUB_STATE);
            let target = super::effects::target(ctx, contact::V2_SUB_STATES, sub_state as u32);
            match sub_state {
                0 => {
                    ctx.anim_step();
                    let pickup = o.u8(ctx.b, field::PICKUP);
                    if pickup == 0 {
                        return;
                    }
                    let busy = ctx.ram8(contact::BUSY) != 0;
                    if busy {
                        o.set_u8(ctx.b, field::PICKUP, 0);
                        return;
                    }
                    ctx.set_ram8(contact::BUSY, 1);
                    add_u8(ctx, o, field::SUB_STATE, 2);
                    if !contact_created_set(ctx, o) {
                        contact_2_end(ctx, o);
                    }
                }
                2 => {
                    if contact_set_finished(ctx, o) {
                        contact_2_end(ctx, o);
                    }
                }
                4 => {
                    ctx.anim_step();
                }
                _ => crate::families::effects::outside_table(target),
            }
        }
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

fn contact_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let target = super::effects::target(ctx, contact::TABLE_VARIANTS, 2 * variant as u32);
    match variant {
        0 => contact_0(ctx, o),
        1 => contact_1(ctx, o),
        2 => contact_2(ctx, o),
        _ => crate::families::effects::outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 3: equipment (0xdb7de)

/// Type 3, equipment (weapons, protections and accessories, identifiers `0x2e` and above; below, inventory objects). An
/// eight-byte sheet per variant (`0xdbc8c`) gives six "usable by the class" flags then the equipment identifier and its
/// group. Pickup: the equipment is added to the player's six slots (`+0x140`), replaces an equipment of the same group
/// if it is better, or takes a free slot; refused if the class cannot carry it or if the first slot is locked (thrown
/// off in an arc). Variants 0x24 to 0x27 also give the player a temporary bonus (`+0x18b..0x191`).
mod equipment {
    pub const ENTRY: u32 = 0xdb7de;
    pub const STATES: u32 = 0xdb7f4;
    pub const SUB_STATES: u32 = 0xdb88c;
    pub const ANIMATIONS: u32 = 0x1bf156;
    /// Pickup index per variant.
    pub const INDICES: u32 = 0xdbdcc;
    /// Sheets (data); bytes 0 to 5: authorized classes, 6: identifier, 7: group.
    pub const SHEETS: u32 = 0xdbc8c;
    /// Per class, 8 identifiers that the first slot cannot replace.
    pub const LOCKED: u32 = 0xdbc5c;
    pub const FIRST_EQUIPPED: u32 = 0x10d;
    pub const SECOND_EQUIPPED: u32 = 0x10e;
}

/// `0xdba96`: temporary bonus given by variants 0x24 to 0x27.
fn equipment_bonus<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let player = pointer(ctx, o, field::OWNER);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if !(variant != 0x24) {
        return player.set_u8(ctx.b, 0x18c, 0x14);
    }
    if !(variant != 0x25) {
        return player.set_u8(ctx.b, 0x18b, 0x14);
    }
    if !(variant != 0x26) {
        player.set_u8(ctx.b, 0x18d, 0x14);
    }
    if !(variant != 0x27) {
        player.set_u8(ctx.b, 0x191, 0x14);
    }
}

/// `0xdbb0e`: successful pickup: bonus, score event and sound. Returns false (accepted).
fn equipment_accepted<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, player: Obj) -> bool {
    equipment_bonus(ctx, o);
    let event = 0x11 + ((player.u8(ctx.b, 2) as u16) << 8);
    crate::events::enqueue_points(ctx.b, event);
    effect_pickup(ctx, ctx.obj(), 5);
    false
}

/// `0xdbafa`: pickup refused: the object will be thrown off.
fn equipment_refused<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    o.set_u8(ctx.b, field::SUB_STATE, 6);
    o.set_u8(ctx.b, field::SUB_SUB_STATE, 0);
    o.set_u8(ctx.b, inventory::DELAY, 0xc);
    true
}

/// Group of an equipment (byte 7 of its sheet).
fn equipment_group<R: Bench>(ctx: &mut Cx<'_, R>, id: u8) -> u8 {
    let rank = ((id.wrapping_sub(0x2e) as u32) << 3) & 0xffff;
    ctx.data8(equipment::SHEETS + 7 + rank)
}

/// `0xdbad8`: stores the equipment in the owner's inventory. Returns true if refused.
fn equipment_add<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let player = pointer(ctx, o, field::OWNER);
    let line = ((o.u8(ctx.b, Obj::VARIANT) as u32) << 3) & 0xffff;
    let class = (player.u8(ctx.b, 3) >> 1) as u32;
    let allowed = ctx.data8(equipment::SHEETS + line + class);
    if !(allowed != 0) {
        return equipment_refused(ctx, o);
    }
    let mut id = ctx.data8(equipment::SHEETS + 6 + line);
    // already equipped?
    for off in [equipment::FIRST_EQUIPPED, equipment::SECOND_EQUIPPED] {
        let equipped = player.u8(ctx.b, off).wrapping_add(0x2e);
        if equipped == id {
            return equipment_accepted(ctx, o, player);
        }
    }
    // is the identifier already in the inventory?
    for i in 0..6u32 {
        let slot = player.u8(ctx.b, inventory::IDS + i);
        let empty = slot == inventory::EMPTY;
        if !empty {
            let object = slot <= 0x2d;
            if object {
                // inventory object: only if identical
                if slot == id {
                    return equipment_accepted(ctx, o, player);
                }
            } else {
                let (g_slot, g_new) = (equipment_group(ctx, slot), equipment_group(ctx, id));
                if !(g_new != g_slot) {
                    // same group: keep the better one
                    if !(id as i8 > slot as i8) {
                        id = slot;
                    }
                    player.set_u8(ctx.b, inventory::IDS + i, id);
                    return equipment_accepted(ctx, o, player);
                }
            }
        }
    }
    // equipment held in hand (byte 0x10d, or 0x10e for class 4)
    let mut door = player.u8(ctx.b, equipment::FIRST_EQUIPPED);
    let class_main = player.u8(ctx.b, 3) >> 1;
    if !(class_main != 4) {
        door = player.u8(ctx.b, equipment::SECOND_EQUIPPED);
    }
    let group_door = ctx.data8(equipment::SHEETS + 7 + ((door as u32) << 3));
    let group_new = equipment_group(ctx, id);
    if !(group_new != group_door) {
        let new = id.wrapping_sub(0x2e);
        if !(door as i8 > new as i8) {
            player.set_u8(ctx.b, equipment::FIRST_EQUIPPED, new);
        }
        return equipment_accepted(ctx, o, player);
    }
    // a free slot
    for i in 0..6u32 {
        let slot = player.u8(ctx.b, inventory::IDS + i);
        if !(slot != inventory::EMPTY) {
            player.set_u8(ctx.b, inventory::IDS + i, id);
            equipment_bonus(ctx, o);
            return false;
        }
    }
    // swap with the first slot, unless it is locked for this class
    let first = player.u8(ctx.b, inventory::IDS);
    for k in 0..8u32 {
        let lock = ctx.data8(equipment::LOCKED + (class << 3) + k);
        if first == lock {
            return equipment_refused(ctx, o);
        }
    }
    let old_quantity = player.u8(ctx.b, inventory::QUANTITIES);
    // D0.w: the class × 8 (`lsr.w #1` then `lsl.w #3` of byte +3) keeps its high byte under the identifier; it is
    // non-zero only if the owner is not a player (null pointer: ROM bytes)
    let index = ((class << 3) as u16 & 0xff00) | first as u16;
    object_ejected(ctx, o, player, index, old_quantity);
    player.set_u8(ctx.b, inventory::IDS, id);
    player.set_u8(ctx.b, inventory::QUANTITIES, 0);
    equipment_bonus(ctx, o);
    false
}

fn equipment_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let mode = o.u8(ctx.b, field::MODE);
    if mode != 0 {
        let delay = o.u8(ctx.b, inventory::DELAY);
        if !(delay == 0) {
            o.set_u8(ctx.b, inventory::DELAY, delay - 1);
            return;
        }
        add_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
    } else if airborne(ctx, o) {
        state_next(ctx, o, sub::FALL);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
    } else {
        state_next(ctx, o, sub::WAIT);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        o.set_u16(ctx.b, field::DURATION, 0x4b0);
    }
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let index = ctx.data8(equipment::INDICES + variant as u32);
    o.set_u8(ctx.b, field::INDEX, index);
    animate_and_sort(ctx, equipment::ANIMATIONS, variant);
}

/// Substate 6: the refused equipment is thrown off in an arc; on landing, a variant 0x0f picked up by class 1 more than
/// seven times triggers a scene.
fn equipment_reject<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let launches = o.u8(ctx.b, field::SUB_SUB_STATE) != 0;
    if !launches {
        let delay = o.u8(ctx.b, inventory::DELAY).wrapping_sub(1);
        o.set_u8(ctx.b, inventory::DELAY, delay);
        if delay != 0 {
            return;
        }
        add_u8(ctx, o, field::SUB_SUB_STATE, 2);
        let player = pointer(ctx, o, field::OWNER);
        let y = player.u16(ctx.b, Obj::Y).wrapping_sub(1);
        o.set_u16(ctx.b, Obj::Y, y);
        o.set_u16(ctx.b, Obj::Z, 0x10);
        o.set_u16(ctx.b, Obj::VZ, 0x200);
        o.set_u16(ctx.b, Obj::AZ, 0xffc0);
        o.set_u32(ctx.b, Obj::VX, 0x0280_0000);
        let (cam_x, _) = ctx.camera();
        let limit = cam_x.wrapping_add(0xc0);
        let x = o.u16(ctx.b, Obj::X);
        if !(limit as i16 > x as i16) {
            let v = o.u32(ctx.b, Obj::VX).wrapping_neg();
            o.set_u32(ctx.b, Obj::VX, v);
        }
        return ctx.reinsert();
    }
    ctx.move_xz();
    let z = o.u16(ctx.b, Obj::Z);
    if z as i16 >= 0 {
        return;
    }
    o.set_u8(ctx.b, field::SUB_STATE, sub::WAIT);
    o.set_u8(ctx.b, field::SUB_SUB_STATE, 0);
    o.set_u32(ctx.b, Obj::Z, 0);
    o.set_u8(ctx.b, field::PICKUP, 0);
    let player = pointer(ctx, o, field::OWNER);
    let class = player.u8(ctx.b, 3) >> 1;
    if class != 1 {
        return;
    }
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if variant != 0xf {
        return;
    }
    let n = ctx.ram8(0x57).wrapping_add(1);
    ctx.set_ram8(0x57, n);
    if n <= 7 {
        return;
    }
    o.set_u16(ctx.b, field::DURATION, 0x4b0);
    o.set_u8(ctx.b, Obj::FLAGS, 1);
    o.set_u8(ctx.b, Obj::VARIANT, 0xc);
    ctx.anim_start(equipment::ANIMATIONS, 0xc);
    effects_creators::posed_flash(ctx.b, o);
    ctx.set_ram8(-0x5b83, 1);
    scene_set_of(ctx, 0x29);
}

fn equipment_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = super::effects::target(ctx, equipment::SUB_STATES, sub_state as u32);
    match sub_state {
        sub::FALL => {
            let in_flight = ctx.move_xyz().gt();
            if !in_flight {
                add_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u16(ctx.b, field::DURATION, 0x4b0);
                o.set_u32(ctx.b, Obj::Z, 0);
                let mode = o.u8(ctx.b, field::MODE);
                if !(mode != 0) {
                    in_view(ctx, o);
                }
            }
            fall_end(ctx);
        }
        sub::WAIT => reserve_wait(ctx, o),
        sub::PICKUP => {
            let player = pointer(ctx, o, field::OWNER);
            let ready = player.u8(ctx.b, 0x33) != 0;
            if !ready {
                return;
            }
            let refuse = equipment_add(ctx, o);
            if !refuse {
                add_u8(ctx, o, Obj::STATE, 2);
            }
            mark_pickup(ctx, o);
        }
        6 => equipment_reject(ctx, o),
        _ => crate::families::effects::outside_table(target),
    }
}

fn equipment_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, equipment::STATES, state as u32);
    match state {
        state::INIT => equipment_init(ctx, o),
        state::ACTIVE => equipment_active(ctx, o),
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

handler!(ITEM_3, equipment::ENTRY, "item_equipment", #[without] equipment_handler);
handler!(ITEM_0, treasure::ENTRY, "item_treasure", #[without] treasure);
handler!(ITEM_1, consumable::ENTRY, "item_consumable", #[without] consumable_handler);
handler!(ITEM_4, inventory::ENTRY, "item_inventory", #[without] inventory_handler);
handler!(ITEM_5, bonus::ENTRY, "item_bonus", #[without] bonus_handler);
handler!(ITEM_6, scene::ENTRY, "item_scene", #[without] scene_handler);
handler!(ITEM_7, reserve::ENTRY, "item_reserve", #[without] reserve_handler);
handler!(ITEM_8, contact::ENTRY, "item_contact", #[without] contact_handler);
handler!(ITEM_9, escort::ENTRY, "item_accompagnement", #[without] escort_handler);

// ---------------------------------------------------------------------------------------------
// Subroutines called outside the handlers

/// `0xdabba`: clears the array of collectible objects picked up (48 longs at `A5 - 0x5b88`).
pub fn clear_picked_up(b: &mut Cps2Bus) {
    let start = A5.wrapping_add(PICKED as u32);
    for k in 0..48 {
        wr32(b, start + 4 * k, 0);
    }
}

/// `clear_picked_up` for a caller still on the processor (`game::leave`), registers left like the original: D0 = 0,
/// D6.w = 0xffff, A0 = end of the array.
pub fn clear_picked_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    clear_picked_up(b);
    c.d[0] = 0;
    c.d[6] = c.d[6] & 0xffff_0000 | 0xffff;
    c.a[0] = A5.wrapping_add(PICKED as u32) + 4 * 48;
}

/// A player's equipment that can drop when it is hit: (byte of the carried object, hit counter, chance table per hit).
struct Door {
    object: u32,
    hits: u32,
    chances: u32,
}

/// The hit player loses at most one object per hit: shield (`+0x165`, 0 to 3), special weapon (`+0x163`) then ring
/// (`+0x162`, except 1). Each hit decrements a counter; as long as it stays positive, the drop depends on a draw (bit
/// `rng & 0xf` of the table word for this counter, in the clear); the object falls through `0xcc352`
/// (`creators::set_a`, D0.b = its object number).
const SHIELD: Door = Door { object: 0x165, hits: 0x16d, chances: 0xdb7c4 };
const WEAPON: Door = Door { object: 0x163, hits: 0x16c, chances: 0xdb7b8 };
const RING: Door = Door { object: 0x162, hits: 0x16b, chances: 0xdb7d0 };
/// Number of objects dropped at this hit (word).
const DROPPED: u32 = A5 - 0x5ac8;

/// The hit counter of `p` is exhausted, or the draw makes the object fall. The registers the original leaves (A4 = the
/// chance table, D0.w = the draw, D2.w = the mask) go to `r`.
fn falls(r: &mut impl Regs, b: &mut Cps2Bus, o: Obj, p: &Door) -> bool {
    let n = o.u8(b, p.hits).wrapping_sub(1);
    o.set_u8(b, p.hits, n);
    if (n as i8) < 0 {
        return true;
    }
    r.a(4, p.chances);
    let i = 2 * n as u32;
    r.d(0, i);
    let mask = rd16(b, p.chances + i);
    r.d_word(2, mask);
    let bit = crate::rng::random(b) & 0xf;
    r.d_word(0, bit);
    mask >> bit & 1 != 0
}

fn dropped(r: &mut impl Regs, b: &mut Cps2Bus, o: Obj, p: &Door, number: u8) {
    r.d_byte(0, number);
    super::creators::set_a_via(r, b, number, |b| o.u8(b, Obj::VARIANT));
    o.set_u8(b, p.object, 0xff);
    o.set_u8(b, p.hits, 0);
}

fn count_dropped(b: &mut Cps2Bus) {
    let n = rd16(b, DROPPED).wrapping_add(1);
    wr16(b, DROPPED, n);
}

/// `0xdb6c0`: equipment dropped by the hit player `o` (`player::hurt`); the registers the original leaves (D0, D2, A4)
/// go to `r`.
pub(crate) fn fall_equipment(r: &mut impl Regs, b: &mut Cps2Bus, o: Obj) {
    wr16(b, DROPPED, 0);
    let shield = o.u8(b, SHIELD.object);
    if shield <= 3 && falls(r, b, o, &SHIELD) {
        dropped(r, b, o, &SHIELD, shield + 4);
        count_dropped(b);
    }
    if (o.u8(b, WEAPON.object) as i8) >= 0 && falls(r, b, o, &WEAPON) && rd16(b, DROPPED) == 0 {
        r.d(0, 0);
        dropped(r, b, o, &WEAPON, 3);
        count_dropped(b);
    }
    let ring = o.u8(b, RING.object);
    if (ring as i8) >= 0 && ring != 1 && falls(r, b, o, &RING) && rd16(b, DROPPED) == 0 {
        r.d_byte(0, ring);
        super::creators::set_a_via(r, b, ring, |b| o.u8(b, Obj::VARIANT));
        o.set_u8(b, RING.object, 0xff);
        // character 5 keeps its original ring (1)
        let character = o.u8(b, 0x3) >> 1;
        r.d(0, character as u32);
        if character == 5 {
            o.set_u8(b, RING.object, 1);
        }
        o.set_u8(b, RING.hits, 0);
    }
}
