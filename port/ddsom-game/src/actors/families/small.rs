//! Small routines that remained in 68000 and that the Rust still called: object creators (shots,
//! effects, enemies, system objects), palette copies, a few game variables.
//!
//! Creators' contract: A0 = the creator; on return A1 = the created object, carry raised if the pool is
//! full. The residual registers that the original leaves (D0, D1, D2, D5, flags) are reproduced.

use crate::world::World;

use super::cx::{handler_without, Bench, Cx};
use super::effects_creators::leftovers::{Pure, Regs};
use crate::hw::video::{palette, regs, Vram};
use crate::world::objects::pools;
use crate::world::objects::{Current, Enemy, Obj};
use crate::world::ops::Size;
use crate::world::objects::player::Player;
use crate::world::ram::{Byte, Long};
use crate::world::record::record;
use crate::services::scroll::CAMERA_1;
use crate::world::tables::engine;
use crate::Native;

/// The fields that these creators set in the objects they create, and read in their creator.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Spawn(pub Obj);

impl Spawn {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Spawn {
        /// Blinking counter copied from the creator.
        blink, set_blink, BLINK: u8 @ 0x43;
        /// Resistances of a player shot (two bytes).
        resist_a, set_resist_a, RESIST_A: u8 @ 0x45;
        resist_b, set_resist_b, RESIST_B: u8 @ 0x46;
        /// `Obj::TIED` as a long, for the shots that keep a 32-bit address.
        tied_long, set_tied_long, TIED_LONG: u32 @ Obj::TIED;
        /// Second link (word): the creator of a chained shot, or a delay.
        partner, set_partner, PARTNER: u16 @ 0x5c;
        /// The creator, as a long (harpy shot).
        creator_long, set_creator_long, CREATOR_LONG: u32 @ 0x56;
        /// A byte of the creator passed on (harpy companion: its `+0xa2`).
        passed, set_passed, PASSED: u8 @ 0x56;
        /// Harpy: quarter of the creator's HP, and the creator's shot flags (`+0x42`, `+0xa2`, `+0xa5`) (h).
        quarter_hp, set_quarter_hp, QUARTER_HP: u16 @ 0x58;
        shot_flag, set_shot_flag, SHOT_FLAG: u8 @ 0x42;
        /// Weapon shot: the player holding the weapon (word), its power, given to the shot.
        holder, set_holder, HOLDER: u16 @ 0x54;
        power, set_power, POWER: u8 @ 0x49;
        weapon_power, set_weapon_power, WEAPON_POWER: u8 @ 0x42;
        /// Player effect: step `+7` of the player when it was created, duration (h).
        player_step, set_player_step, PLAYER_STEP: u8 @ 0x5d;
        duration, set_duration, DURATION: u8 @ 0x5a;
        /// SET objects created by the scenes: cleared state bytes (`+0x22`, `+0x37` = 0xff, `+0x38`), a cleared flag of
        /// SET 5 (`+0x24`), the object of a health bar (word `+0x20`) (h).
        state_22, set_state_22, STATE_22: u8 @ 0x22;
        flag_24, set_flag_24, FLAG_24: u8 @ 0x24;
        mark_37, set_mark_37, MARK_37: u8 @ 0x37;
        bar_owner, set_bar_owner, BAR_OWNER: u16 @ 0x20;
        /// SOBJ 8: the object it is linked to (word).
        link_14, set_link_14, LINK_14: u16 @ 0x14;
        companion_byte, set_companion_byte, COMPANION_BYTE: u8 @ 0xa2;
        shot_ready, set_shot_ready, SHOT_READY: u8 @ 0xa5;
    }
}

/// Starts object `o` (active) as type `kind`.
fn start(b: &mut World, o: Obj, kind: u8) {
    o.set_active(b, 1);
    o.set_kind(b, kind);
}


/// Creates an object of `pool` of type `kind`, linked to the creator by word `link` (flags of that `move.w`).
pub(crate) fn linked_via(r: &mut impl Regs, b: &mut World, creator: Obj, pool: &pools::Pool, kind: u8, link: u32) -> Option<Obj> {
    let o = r.allocated(b, pool)?;
    o.set_u8(b, Obj::ACTIVE, 1);
    o.set_u8(b, Obj::KIND, kind);
    o.set_u16(b, link, creator.word());
    r.flags(creator.word() as u32, Size::Word);
    Some(o)
}

/// 0x4c930 for the player module: player effect 0x10 (revival) linked to `player`.
pub(crate) fn revive(r: &mut impl Regs, b: &mut World, player: Obj) -> Option<Obj> {
    linked_via(r, b, player, &pools::PEFF, 0x10, Obj::TIED)
}

/// 0x4f132: player effect 0x21 (second phase of the spell quake), linked to the player by `+0x50`.
pub fn quake_b(b: &mut World, player: Obj) -> Option<Obj> {
    linked_via(&mut Pure, b, player, &pools::PEFF, 0x21, Obj::TIED)
}

/// 0x54700: shot of type 0x17 that takes over the creator's word `+0x50`.
pub(crate) fn shot_17(r: &mut impl Regs, b: &mut World, me: Obj) -> Option<Obj> {
    let o = r.allocated(b, &pools::PSHL)?;
    start(b, o, 0x17);
    let v = me.tied(b);
    o.set_tied(b, v);
    r.flags(v as u32, Size::Word);
    Some(o)
}

/// 0x54818 (variant left as is: `variant` zero), 0x54842 (variant 1) and 0x54872 (variant 2): shot of type
/// 0x18 linked to the creator by `+0x5c`, resistances 0x10; the created shot, `None` if the PSHL pool is full.
pub(crate) fn shot_18(r: &mut impl Regs, b: &mut World, me: Obj, variant: u8) -> Option<Obj> {
    let shot = r.allocated(b, &pools::PSHL)?;
    start(b, shot, 0x18);
    if variant != 0 {
        shot.set_variant(b, variant);
    }
    let s = Spawn(shot);
    s.set_resist_a(b, 0x10);
    s.set_resist_b(b, 0x10);
    s.set_partner(b, me.word());
    Some(shot)
}

/// 0x76ec4: if more than two ENEMY slots remain, creates two enemies of type 0x19 (variants 0 and
/// 1) linked to the creator; true (D0 = 1) if created. The original does not test the allocations' carry.
pub(crate) fn enemy_pair_via(r: &mut impl Regs, b: &mut World, creator: Obj) -> bool {
    enemy_pair_kind_via(r, b, creator, 0x19)
}
/// 0x76ec4 (type 0x19) and 0x63ae0 (type 7, `creators`): two enemies of type `kind`, variants 0 and 1.
pub(crate) fn enemy_pair_kind_via(r: &mut impl Regs, b: &mut World, creator: Obj, kind: u8) -> bool {
    let created = (pools::ENEMY.free_count(b) as i16) > 2;
    if created {
        for variant in 0..2 {
            // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
            let o = r.alloc_without_test(b, &pools::ENEMY);
            start(b, o, kind);
            o.set_variant(b, variant);
            o.set_setting(b, 0);
            Enemy(o).set_host(b, creator.word());
        }
    }
    r.d(0, created as u32);
    r.flags(created as u32, Size::Long);
    created
}
/// 0x76ec4 with no registers.
pub fn enemy_pair_created(b: &mut World, creator: Obj) -> bool {
    enemy_pair_via(&mut Pure, b, creator)
}

/// 0xa7234 and 0xa727c: swelling of Boss 15's roar: a number of ESHL shots of type 0x41 (variant
/// `variant`) given by the table (`small/roar_big`, `small/roar_small`: by rank / 2), numbered from n-1 to 0 by
/// `+0x2f`, linked by `+0x5c`.
/// D0.w and D1.w of the original: the table index, the counter (exhausted: 0xffff, or the number of the refused shot).
pub(crate) fn roar_via(r: &mut impl Regs, b: &mut World, creator: Obj, big: bool) {
    let (counts, variant) = if big { (crate::world::tables::engine_table!("small/roar_big"), 0) } else { (crate::world::tables::engine_table!("small/roar_small"), 1) };
    let i = crate::world::objects::rank::RANK.get(b) >> 1;
    r.d_word(0, i);
    let total = counts.at8(b, i as u32);
    r.d(1, total as u32);
    let mut n = (total as u16).wrapping_sub(1);
    r.d_word(1, n);
    while let Some(o) = r.allocated(b, &pools::ESHL) {
        start(b, o, 0x41);
        o.set_variant(b, variant);
        o.set_setting(b, n as u8);
        let link = Spawn(creator).partner(b);
        Spawn(o).set_partner(b, link);
        r.flags(link as u32, Size::Word);
        n = n.wrapping_sub(1);
        r.d_word(1, n);
        if n == 0xffff {
            break;
        }
    }
}
/// 0xa7234 (`big`) and 0xa727c with no registers.
pub fn roar_created(b: &mut World, creator: Obj, big: bool) {
    roar_via(&mut Pure, b, creator, big)
}

/// Core of 0xa036e: returns the companion and the HP/4 word that the original leaves in D2 (leftover read afterwards),
/// or, on failure, the A1 that the allocation leaves.
pub(crate) fn harpy_companion_via(r: &mut impl Regs, b: &mut World, me: Obj) -> Result<(Obj, u16), Obj> {
    let o = r.alloc_or_rest(b, &pools::ESHL)?;
    let flip = me.flip(b) != 0;
    let v = me.variant(b);
    o.set_variant(b, v);
    start(b, o, 0x10);
    o.set_box_set(b, 0x3e);
    let (sm, so) = (Spawn(me), Spawn(o));
    o.set_tied(b, me.word());
    let v = sm.blink(b);
    so.set_blink(b, v);
    let hp = Enemy(me).hp_max(b) >> 2;
    r.d_word(2, hp);
    sm.set_quarter_hp(b, hp);
    let v = sm.companion_byte(b);
    so.set_passed(b, v);
    for field in [Obj::X, Obj::Y, Obj::Z] {
        let v = me.u32(b, field);
        o.set_u32(b, field, v);
    }
    let dx: u16 = if flip { 0x2fu16.wrapping_neg() } else { 0x2f };
    r.d_word(0, dx);
    r.d_word(1, 0x78u16.wrapping_neg());
    let x = o.u16(b, Obj::X).wrapping_add(dx);
    o.set_u16(b, Obj::X, x);
    let z = o.u16(b, Obj::Z).wrapping_add(0x78u16.wrapping_neg());
    o.set_u16(b, Obj::Z, z);
    let y = o.y(b).wrapping_add(1);
    o.set_y(b, y);
    let z = o.z(b).wrapping_sub(1);
    o.set_z(b, z);
    let f = me.flip(b);
    o.set_flip(b, f);
    r.flags(f as u32, Size::Byte);
    Ok((o, hp))
}

/// Core of 0xa0566: `d5` = the byte placed in the creator's `+0x42`, `target` = the long A2 linked at `+0x50`.
pub(crate) fn harpy_shot_via(r: &mut impl Regs, b: &mut World, me: Obj, d5: u8, target: u32) -> Option<Obj> {
    let o = r.allocated(b, &pools::ESHL)?;
    let (sm, so) = (Spawn(me), Spawn(o));
    sm.set_shot_flag(b, d5);
    let v = sm.blink(b);
    so.set_blink(b, v);
    so.set_tied_long(b, target);
    so.set_creator_long(b, me.0);
    sm.set_shot_ready(b, 0);
    start(b, o, 0x11);
    for field in [Obj::X, Obj::Y, Obj::Z] {
        let v = me.u32(b, field);
        o.set_u32(b, field, v);
    }
    let v = me.flip(b);
    o.set_flip(b, v);
    let v = me.variant(b);
    o.set_variant(b, v);
    r.flags(v as u32, Size::Byte);
    Some(o)
}

// ---------------------------------------------------------------------------------------------
// Game variables, palette copies, chains of initializations
// ---------------------------------------------------------------------------------------------

/// 0x10158 (`brighten`) and 0x101da for a generic handler: the four blocks of eight colours copied into the palette,
/// like `palette_blocks_lit` and `palette_blocks`. Original glitch: the last block's `movem.l` leaves its colours 2 to
/// 5 in D2-D5, read back by the next handlers (`dispatch::Leftovers`).
pub(crate) fn four_palette_blocks<R: Bench>(x: &mut Cx<'_, R>, brighten: bool) {
    const DESTINATIONS: [Vram; 4] = [palette(0, 0x170), palette(0, 0x180), palette(0, 0x1b0), palette(0, 0x1f0)];
    // palettes 0x46 to 0x49 of the library, or the four palettes of `small/palettes_69`
    let mut last = 0;
    for (k, dst) in DESTINATIONS.into_iter().enumerate() {
        let src = if brighten {
            ddsom_data::Palettes::ROM + 32 * (0x46 + k as u32)
        } else {
            crate::world::tables::engine_table!("small/palettes_69").rom + 0x20 * k as u32
        };
        for i in 0..8 {
            let v = engine().long(x.b, src + 4 * i);
            dst.offset(4 * i).set_long(x.b, if brighten { v | 0xf000_f000 } else { v });
        }
        last = src;
    }
    *x.c.d2() = engine().long(x.b, last + 8);
    *x.c.d3() = engine().long(x.b, last + 12);
    *x.c.d4() = engine().long(x.b, last + 16);
    *x.c.d5() = engine().long(x.b, last + 20);
}

/// 0x3525c for a generic handler: clears the start of scroll layer 1 (`video::clear_layer1_start`).
/// Original glitch: the empty cell stays in D2-D5 (`dispatch::Leftovers`).
pub(crate) fn clear_scroll1_start<R: Bench>(x: &mut Cx<'_, R>) {
    crate::services::video::clear_layer1_start(x.b);
    (*x.c.d2(), *x.c.d3(), *x.c.d4(), *x.c.d5()) = (0x1f_0000, 0x1f_0000, 0x1f_0000, 0x1f_0000);
}

/// 0x4aed0: two effects of type 5, variant 1, indices `+0x2f` 0 and 1, linked to the creator (`+0x50`).
pub(crate) fn peff_5_pair(r: &mut impl Regs, b: &mut World, me: Obj) {
    for index in [0, 1] {
        let Some(o) = r.allocated(b, &pools::PEFF) else { break };
        start(b, o, 5);
        o.set_variant(b, 1);
        o.set_setting(b, index);
        o.set_tied(b, me.word());
        r.flags(me.word() as u32, Size::Word);
    }
}

/// 0x53746: twelve stones (shots of type 0x11, variants 0 to 11) spread around the camera in use,
/// X, Y offsets in a table (`small/stone_offsets`), height 0x100 above their impact point, delay `+0x5c` from 0x40
/// to 0x148 in steps of 0x18; orientation and owner of the creator.
pub(crate) fn pshl_stones(r: &mut impl Regs, b: &mut World, me: Obj) {
    const COUNT: u16 = 12;
    // D3: variant, D4: delay, D5: `dbf` counter; left to the following handlers (`dispatch::Leftovers`)
    let (mut variant, mut delay, mut n) = (0u8, 0x40u16, COUNT - 1);
    r.d(3, 0);
    r.d_word(4, delay);
    r.d_word(5, n);
    while let Some(o) = r.allocated(b, &pools::PSHL) {
        start(b, o, 0x11);
        let v = me.flip(b);
        o.set_flip(b, v);
        let v = me.tied(b);
        o.set_tied(b, v);
        o.set_variant(b, variant);
        Spawn(o).set_partner(b, delay);
        let (x, y) = crate::world::ram::camera(b);
        let index = (variant as u16).wrapping_shl(2) as u32;
        let offsets = crate::world::tables::engine_table!("small/stone_offsets");
        let (dx, dy) = (offsets.at16(b, index), offsets.at16(b, 2 + index));
        let ground = y.wrapping_add(dy);
        o.set_u16(b, Obj::X, x.wrapping_add(dx));
        o.set_u16(b, Obj::Y, ground);
        o.set_u16(b, Obj::Z, y.wrapping_add(0x100).wrapping_sub(ground));
        r.d_word(0, ground);
        r.d_word(1, dx);
        r.d_word(2, dy);
        variant = variant.wrapping_add(1);
        r.d_byte(3, variant);
        delay = r.addition(0x18, delay as u32, Size::Word) as u16;
        r.d_word(4, delay);
        n = n.wrapping_sub(1);
        r.d_word(5, n);
        if n == 0xffff {
            break;
        }
    }
}

/// 0x1a5e8: twelve palettes of the library (from palette 0x4a6) copied into page 1 from its colour 0x40. Core.
pub fn palette_twelve_blocks(b: &mut World) {
    let (dst, src) = (palette(1, 0x40), ddsom_data::Palettes::ROM + 32 * 0x4a6);
    for i in 0..12 * 8 {
        let v = engine().long(b, src + 4 * i);
        dst.offset(4 * i).set_long(b, v);
    }
}

/// Flash colors: yellow (0x1ac9a), green (0x1ac72), blue (0x1ac7c), end of the flash (0x1ac90).
pub(crate) const LIGHTNING_YELLOW: u16 = 0xfff0;
pub(crate) const LIGHTNING_GREEN: u16 = 0xf0f0;
pub(crate) const LIGHTNING_BLUE: u16 = 0xf00f;
pub(crate) const LIGHTNING_END: u16 = 0xff50;

/// The eight tile blocks of background 2 (0x26d4: D0 = block, A1 = destination).
const BACKGROUND_BLOCKS: [(u16, u32); 8] = [
    (0xce, 0x917000),
    (0xad, 0x917400),
    (0xaf, 0x917800),
    (0xaf, 0x917c00),
    (0xac, 0x916000),
    (0xcd, 0x916400),
    (0xae, 0x916800),
    (0xae, 0x916c00),
];

/// 0x2a52: background 2 on (tile blocks, bit 0 of the video control, camera 1 frozen in mode 0x80, camera 2 in
/// service).
pub fn background_2_on(b: &mut World) -> crate::services::video::GridEnd {
    let mut end = None;
    for (block, dst) in BACKGROUND_BLOCKS {
        end = Some(crate::services::video::load_grid(b, crate::services::video::Grid::Layer2, block, dst));
    }
    let video = crate::services::video::VIDEO_CONTROL.get(b) | 1;
    crate::services::video::VIDEO_CONTROL.set(b, video);
    regs::VIDEO_CONTROL.set(b, video);
    regs::ROW_SCROLL_BASE.set(b, 0x9200);
    CAMERA_1.set_frozen(b, 2);
    CAMERA_1.set_mode(b, 0x80);
    crate::world::ram::CAMERA_ALT_ACTIVE.set(b, 1);
    end.expect("eight blocks")
}

/// 0x2a36: background 1 on (camera 1 in mode 0x18); 0x2a42: its timer = D0, background 1 set to 0x34.
pub fn background_1_on(b: &mut World) {
    background_1_in_mode(b, 0x18);
}
pub(crate) fn background_1_in_mode(b: &mut World, mode: u16) {
    CAMERA_1.set_step(b, 0);
    CAMERA_1.set_mode(b, mode);
}
/// 0x2a42: the timer of camera 1 = `value` (D0.w of the original), background 1 set to 0x34.
pub fn background_1_lit_a(b: &mut World, value: u16) {
    CAMERA_1.set_timer(b, value);
    background_1_in_mode(b, 0x34);
}

/// The "startup done" indicator (`A5-0x715e`).
const STARTUP_DONE: Byte = Byte(0xff0ea2);
/// The pickup objects' spawn lock (`A5-0x5ac4`).
const ITEM_LOCK: Byte = Byte(0xff253c);
/// Long cleared at the reset of a fixed screen (`A5-0x70b0`) (h).
const FIXED_SCREEN_STATE: Long = Long(0xff0f50);

/// 0x133f6 / 0x133f0: the "startup done" indicator raised / lowered.
pub fn startup_done(b: &mut World, done: bool) {
    STARTUP_DONE.set(b, done as u8);
}

/// 0xdd12c: the pickup objects' spawn lock raised.
pub(crate) fn item_lock_clear(b: &mut World) {
    ITEM_LOCK.set(b, 0);
}

/// 0x3baea: player's `+0x1f0`, from table `small/undead_counts` indexed by the quarter of the rank. Returns the
/// index (the D0.w that the original leaves, which a caller reads as the variant of the effect it creates).
pub(crate) fn player_table_byte(b: &mut World, player: Obj) -> u16 {
    let i = crate::world::objects::rank::RANK.get(b) >> 2;
    let v = crate::world::tables::engine_table!("small/undead_counts").at8(b, i as u32);
    Player::of(player).set_undead_left(b, v);
    i
}

/// 0x1c44a: the game variables from `ram::PHASE` (64 bytes, to `A5+0x6b`) set to zero.
pub fn zero_game_variables(b: &mut World) {
    for k in 0..16 {
        Long(crate::world::ram::PHASE.0 + 4 * k).set(b, 0);
    }
}

/// 0x180ac: reset of the fixed screen: SET and EFFCT pools, sprites, sorted list.
pub fn reset_fixed_screen(b: &mut World) {
    FIXED_SCREEN_STATE.set(b, 0);
    crate::world::objects::lists::unlink_all(b);
    crate::services::sprites::clear_lists(b);
    pools::SET.reset(b);
    pools::EFFCT.reset(b);
}

// ---------------------------------------------------------------------------------------------
// Hit reaction of two enemies (common reaction table, `react`)
// ---------------------------------------------------------------------------------------------

/// 0x78ad2, 0x71680 and 0x65236: the hit enemy shakes in place. Step 0: saves X (`+0xa`), starts
/// animation 8 of set `anim` (with the hit type, 0x103d6, if `strike_kind`) and sets the counter (`+0x90` = 16).
/// Step 2: shifts X by ±2 according to the counter parity and the orientation, then, once the counter is
/// exhausted, goes back to the original X
/// and to state 0x04040200. `_table`: original jump table on `+6`.
fn shake<R: Bench>(x: &mut Cx<'_, R>, _table: u32, anim: u32, strike_kind: bool) {
    let step = x.u8(Enemy::STEP);
    match step {
        0 => {
            x.add8(Enemy::STEP, 2);
            x.set8(Enemy::ACTIVE, 2);
            let at = x.u16(Enemy::X);
            x.set16(Enemy::X_SAVED, at);
            x.set16(Enemy::RECOVER, 0);
            let v = x.u8(Enemy::STUN);
            x.set8(Enemy::FREEZE, v);
            x.set8(Enemy::COUNTER, 0x10);
            if strike_kind {
                crate::actors::families::enemy_lib::start(x, anim, 8);
            } else {
                x.anim_start(anim, 8);
            }
        }
        2 => {
            let timer = x.u8(Enemy::COUNTER);
            if (timer as i8) <= 0 {
                let at = x.u16(Enemy::X_SAVED);
                x.set16(Enemy::X, at);
                x.set8(Enemy::ACTIVE, 1);
                x.set32(Enemy::STATE_LONG, 0x0404_0200);
                x.set32(Enemy::HIT_LOCK, 0);
                return;
            }
            let timer = timer - 1;
            x.set8(Enemy::COUNTER, timer);
            let mut d: i16 = 2;
            if timer & 1 != 0 {
                d = -d;
            }
            if x.u8(Enemy::FLIP) != 0 {
                d = -d;
            }
            x.add16(Enemy::X, d as u16);
        }
        _ => {}
    }
}
/// Reaction specific to a type, from the enemy's reaction table (code 0x4c: shaking), designated by its
/// original address; `None` if the address is not one.
pub(crate) fn own_reaction<R: Bench>(x: &mut Cx<'_, R>, routine: u32) -> Option<()> {
    match routine {
        // engin de guerre (type 27)
        0x78ad2 => shake(x, 0x78ae0, 0x171a94, true),
        // ogre (type 18)
        0x71680 => shake(x, 0x7168e, 0x1778a4, true),
        // gargoyle (type 8); never reached by the scenarios (see docs/code-mort.md)
        0x65236 => shake(x, 0x65244, 0x162f12, false),
        _ => return None,
    }
    Some(())
}

/// Enemy reactions specific to a type (`own_reaction`), at their original address (the game calls them directly
/// depuis `enemy_common::react`).
pub const SHAKES: &[Native] = &[
    handler_without(0x78ad2, "enemy_shake_a", |b, a| run_shake(b, a, 0x78ad2)),
    handler_without(0x71680, "enemy_shake_b", |b, a| run_shake(b, a, 0x71680)),
    handler_without(0x65236, "enemy_shake_c", |b, a| run_shake(b, a, 0x65236)),
];

fn run_shake(b: &mut World, a: &mut crate::core::dispatch::Dispatch, routine: u32) {
    own_reaction(&mut crate::actors::families::cx::Gx::dispatch(b, a), routine);
}
