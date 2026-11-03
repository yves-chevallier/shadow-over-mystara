//! Player effects (PEFF), fifth series: types 12 (freeze), 13 (blessing) and 14 (reflect).
//! See `docs/families/joueur.md`.

use crate::families::player::records::Victim;
use super::vars;
use crate::families::player::records::Shot;
use crate::player::Player;
use crate::families::enemy_common::records::Enemy;
use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;
use crate::machine::Machine;
use crate::player::tables::{player_data, byte, Aligned, Read};

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Reference camera: position (X, Y) of the main camera, or of the secondary one if `A5+0x36`.
fn camera<R: Bench>(x: &mut Cx<'_, R>) -> (u16, u16) {
    if vars::USE_CAM2.get(x.b) != 0 {
        (vars::CAM2_X.get(x.b), vars::CAM2_Y.get(x.b))
    } else {
        (vars::CAM_X.get(x.b), vars::CAM_Y.get(x.b))
    }
}

/// Type 12 (0x4bc66): the freeze (magic 15 and freeze spells), six variants that each have their own states.
/// 0: ice shard that bounces off the edges of the screen; 1: its halo; 2: mist; 3: snowflake;
/// 4: falling shard (`+2f` even); 5: crown around the caster. As long as the game is frozen (`A5+0x3d` non-zero) the
/// effects last; it goes to state 4 as soon as the freeze ends.
mod ice {
    use super::*;

    /// Countdown of the current phase.
    const TIMER: u32 = 0x54;
    /// Low byte of `TIMER`.
    const TIMER_LOW: u32 = 0x55;
    /// Gesture of the player when the effect started.
    const GESTURE: u32 = 0x56;
    /// Parent effect (16-bit pointer).
    const PARENT: u32 = 0x5c;

    const ANIMS: u32 = 0x1b6350;
    const OFFSETS: u32 = 0x4bd0e;
    const SFX: crate::sound::Sound = crate::sound::CMD_165_265;
    const CROWN_A: u32 = 0x4c140;
    const CROWN_B: u32 = 0x4c158;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let variant = x.u8(Obj::VARIANT);
        match variant {
            0 => shard(x),
            1 => halo(x),
            2 => mist(x),
            3 => flake(x),
            4 => falling(x),
            _ => crown(x),
        }
    }

    // --- variant 0: bouncing shard ---------------------------------------------------------

    fn shard<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4bc90) {
            0 => {
                x.add16(TIMER, 0xffff);
                if x.u16(TIMER) != 0 {
                    return;
                }
                x.add8(Obj::STATE, 2);
                let (cx, cy) = camera(x);
                x.set16(Obj::X, cx);
                x.set16(Obj::Y, cy);
                x.set16(Obj::Z, 0);
                let k = (x.u8(Shot::SETTING) as u32) << 2 & 0xffff;
                let (dx, dy) = offset(x, OFFSETS, k);
                x.add16(Obj::X, dx);
                x.add16(Obj::Y, dy);
                let r = x.rand_and_b(0x1f);
                x.set8(Obj::SPEED_INDEX, r);
                x.set8(TIMER, r.wrapping_add(1));
                let n = 0x2b;
                x.select_speed(&crate::objects::motion::SPEEDS_B, n);
                x.anim(ANIMS, 0);
                // a halo (variant 1) follows the shard
                if let Some(o) = x.alloc(&pools::PEFF) {
                    o.set_u8(x.b, Obj::ACTIVE, 1);
                    o.set_u8(x.b, Obj::KIND, 0xc);
                    o.set_u8(x.b, Obj::VARIANT, 1);
                    o.set_u16(x.b, PARENT, x.o.0 as u16);
                    let owner = x.u16(f::OWNER);
                    o.set_u16(x.b, f::OWNER, owner);
                }
            }
            2 => {
                x.add8(TIMER, 0xff);
                if x.u8(TIMER) == 0 {
                    let w = x.rand_and_b_word(0x1f);
                    x.set8(Obj::SPEED_INDEX, w as u8);
                    // Original glitch: `andi.b #0xf,D0` then index D0.w: the high byte of the draw enters the delay
                    // index
                    let index = (w & 0xff00 | w & 0xf) as i16 as i32 as u32;
                    let delay = player_data().ice_delays.at8(x.b, index);
                    x.set8(TIMER, delay);
                }
                bounce(x);
                x.anim_step();
                x.move_by_table_xy();
                if !x.push_terrain() {
                    let a = x.u8(Obj::SPEED_INDEX).wrapping_add(0x10) & 0x1f;
                    x.set8(Obj::SPEED_INDEX, a);
                    x.move_by_table_xy();
                }
                if vars::PAUSE.get(x.b) == 0 {
                    x.add8(Obj::STATE, 2);
                }
                x.set_bit(Obj::FLAGS, 1, true);
                let t = x.u8(TIMER);
                if t & 2 != 0 {
                    x.set_bit(Obj::FLAGS, 1, false);
                }
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }

    /// Bounce off the edges of the screen (`0x4bda8`): the draw `andi.w #7,D2w` operates on D2, left by the caller.
    fn bounce<R: Bench>(x: &mut Cx<'_, R>) {
        // Original glitch: the draw is not used, the bounce is chosen by the inherited D2.
        x.random();
        let d2 = *x.c.d2() & 7;
        *x.c.d2() = *x.c.d2() & 0xffff_0000 | d2;
        let d0 = x.u16(Obj::X);
        let (cx, cy) = camera(x);
        let d1 = cx.wrapping_add(8);
        if d1 > d0 {
            let d1 = d1.wrapping_add(1);
            x.set16(Obj::X, d1);
            let a = player_data().ice_bounce_left.at8(x.b, d2);
            x.set8(Obj::SPEED_INDEX, a);
            x.set8(TIMER, 0x18);
            return;
        }
        let d1 = cx.wrapping_add(0x178);
        if d1 <= d0 {
            let d1 = d1.wrapping_sub(1);
            x.set16(Obj::X, d1);
            let a = player_data().ice_bounce_right.at8(x.b, d2);
            x.set8(Obj::SPEED_INDEX, a);
            x.set8(TIMER, 0x18);
            return;
        }
        let d0 = x.u16(Obj::Y);
        let d1 = cy.wrapping_add(0x28);
        if d1 > d0 {
            let d1 = d1.wrapping_add(1);
            x.set16(Obj::Y, d1);
            let a = player_data().ice_bounce_top.at8(x.b, d2);
            x.set8(Obj::SPEED_INDEX, a);
            x.set8(TIMER, 0x18);
        }
        let d1 = cy.wrapping_add(0xc0);
        if d1 <= d0 {
            let d1 = d1.wrapping_sub(1);
            x.set16(Obj::Y, d1);
            let a = player_data().ice_bounce_bottom.at8(x.b, d2);
            x.set8(Obj::SPEED_INDEX, a);
            x.set8(TIMER, 0x18);
        }
    }

    // --- variant 1: halo of the shard --------------------------------------------------------------

    fn halo<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4be9e) {
            0 => {
                x.add8(Obj::STATE, 2);
                let parent = x.linked(PARENT);
                x.copy_pos(parent);
                x.add16(Obj::Y, 0xffff);
                x.anim(ANIMS, 1);
            }
            2 => {
                x.anim_step();
                let parent = x.linked(PARENT);
                let flags = parent.u8(x.b, Obj::FLAGS);
                x.set8(Obj::FLAGS, flags);
                x.copy_pos(parent);
                x.add16(Obj::Y, 0xffff);
                if vars::PAUSE.get(x.b) == 0 {
                    x.add8(Obj::STATE, 2);
                }
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }

    // --- variante 2 : brume ------------------------------------------------------------------------

    fn mist<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4bf16) {
            0 => {
                x.add8(Obj::STATE, 2);
                crate::sound::play(x.b, SFX, x.o);
                x.set16(TIMER, 0x168);
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, Player::SPECIAL_KIND) != 0x1a {
                    x.set16(TIMER, 0x78);
                }
                mist_tick(x);
            }
            2 => mist_tick(x),
            _ => x.free_peff(),
        }
    }

    fn mist_tick<R: Bench>(x: &mut Cx<'_, R>) {
        x.add16(TIMER, 0xffff);
        if x.u16(TIMER) == 0 {
            x.add8(Obj::STATE, 2);
        }
        let t = x.u16(TIMER);
        let d0 = x.u8(TIMER_LOW);
        let mut mask = 1u8;
        if t <= 0x78 {
            mask = 3;
            if t <= 0x3c {
                mask = 7;
            }
        }
        if d0 & mask == 0 {
            // snowflake (variant 3), unrelated
            if let Some(o) = x.alloc(&pools::PEFF) {
                o.set_u8(x.b, Obj::ACTIVE, 1);
                o.set_u8(x.b, Obj::KIND, 0xc);
                o.set_u8(x.b, Obj::VARIANT, 3);
            }
        }
        if t & 0x3f == 0 {
            crate::sound::play(x.b, SFX, x.o);
        }
    }

    // --- variante 3 : flocon -----------------------------------------------------------------------

    fn flake<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4bf92) {
            0 => {
                x.add8(Obj::STATE, 2);
                let (cx, cy) = camera(x);
                x.set16(Obj::X, cx);
                x.set16(Obj::Y, cy);
                let a = x.rand_and(0xf);
                let b = x.rand_and(0xf);
                let (a2, b2) = (b * 2, a * 2);
                let dx = player_data().ice_scatter_x.word_at(x.b, a2);
                let dy = player_data().ice_scatter_y.word_at(x.b, b2);
                x.add16(Obj::X, dx);
                x.add16(Obj::Y, dy);
                // Original glitch: the leftovers of 0x24b0 (D2 = the terrain) are re-read by the bounce of a following
                // handler
                let (px, py) = (x.u16(Obj::X), x.u16(Obj::Y));
                let t = crate::video::terrain(x.b, px, py);
                leftovers_terrain(x, &t);
                if t.number != 0 {
                    x.set8(Obj::STATE, 4);
                }
                let n = x.rand_and(1) + 2;
                x.anim(ANIMS, n);
            }
            2 => {
                if x.anim_step() {
                    x.add8(Obj::STATE, 2);
                }
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }

    // --- variant 4: falling shard ----------------------------------------------------------------

    fn falling<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4c076) {
            0 => {
                x.add16(TIMER, 0xffff);
                if x.u16(TIMER) != 0 {
                    return;
                }
                x.add8(Obj::STATE, 2);
                let k = x.u8(Shot::SETTING) as u32;
                let dx = player_data().ice_spread.word_at(x.b, k);
                let dx = signed(dx, x.u8(f::FLIP) == 0);
                x.add16(Obj::X, dx);
                x.anim(ANIMS, 4);
            }
            2 => {
                if x.anim_step() {
                    x.add8(Obj::STATE, 2);
                }
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }

    // --- variant 5: crown around the caster ----------------------------------------------------

    fn crown<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4c0da) {
            0 => {
                x.add8(Obj::STATE, 2);
                let owner = x.linked(f::OWNER);
                x.copy_pos(owner);
                x.add16(Obj::Y, 0xffff);
                let k = ((owner.u8(x.b, Player::CHARACTER) >> 1) as u32) << 2;
                let table = if owner.u8(x.b, Player::SPECIAL_KIND) == 0x1a { CROWN_A } else { CROWN_B };
                let (dx, dz) = offset(x, table, k);
                let dx = signed(dx, owner.u8(x.b, f::FLIP) != 0);
                x.add16(Obj::X, dx);
                x.add16(Obj::Z, dz);
                x.anim(ANIMS, 5);
            }
            2 => {
                x.anim_step();
                let frozen = vars::PAUSE.get(x.b) != 0;
                if frozen {
                    let owner = x.linked(f::OWNER);
                    let gesture = owner.u8(x.b, Player::STEP);
                    if gesture == x.u8(GESTURE) {
                        return x.reinsert();
                    }
                }
                x.add8(Obj::STATE, 2);
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 13 (0x4c1b6): blessing (variants 0 to 2): placed in front of the caster, it blinks as long as its effect
/// (`+0x168` of the caster) lasts.
mod blessing {
    use super::*;

    const OFFSETS: u32 = 0x4c226;
    const ANIMS: u32 = 0x1b670a;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4c1c4) {
            0 => {
                x.add8(Obj::STATE, 2);
                follow_owner(x);
                x.add16(Obj::Y, 0xffff);
                let v = x.u8(Obj::VARIANT);
                let k = (v as u32) << 2;
                let (dx, dz) = offset(x, OFFSETS, k);
                let dx = signed(dx, x.u8(f::FLIP) != 0);
                x.add16(Obj::X, dx);
                x.add16(Obj::Z, dz);
                x.anim(ANIMS, (v & 1) as u32);
            }
            2 => {
                let owner = x.linked(f::OWNER);
                if owner.u16(x.b, Player::ACTION_TIMER) == 0 {
                    x.add8(Obj::STATE, 2);
                }
                x.set_bit(Obj::FLAGS, 1, false);
                let blink = vars::FRAME_COUNTER.get(x.b) & 1;
                if blink != 0 {
                    x.set_bit(Obj::FLAGS, 1, true);
                }
                x.anim_step();
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 14 (0x4c2b4): reflect. Variant 0: ray placed on the caster; variant 1: four beams that come down on the
/// targeted monster (`+0x5c`) and hurt it when touching the ground.
mod banish {
    use super::*;

    /// Hold delay of the frame.
    const HOLD: u32 = 0x58;
    /// Target of the banishment (16-bit pointer).
    const TARGET: u32 = 0x5c;

    const ANIMS: u32 = 0x1b67f2;
    const SFX: crate::sound::Sound = crate::sound::CMD_18E;
    const POWER_TABLE: u32 = 0x1042e2;
    const POWER_TABLE_BOSS: u32 = 0x104300;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(Obj::VARIANT) != 0;
        match x.state(0x4c2c2) {
            0 if !child => {
                x.add8(Obj::STATE, 2);
                follow_owner(x);
                let dx = signed(-7i16 as u16, x.u8(f::FLIP) != 0);
                x.add16(Obj::X, dx);
                x.add16(Obj::Z, 0x5e);
                x.add16(Obj::Y, 0xffff);
                x.anim(ANIMS, 0);
            }
            0 => init_beam(x),
            2 if !child => {
                x.anim_step();
                x.set_bit(Obj::FLAGS, 1, true);
                let blink = vars::FRAME_COUNTER.get(x.b) & 1;
                if blink != 0 {
                    x.set_bit(Obj::FLAGS, 1, false);
                }
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, Player::STEP) == 4 {
                    x.add8(Obj::STATE, 2);
                }
                x.reinsert();
            }
            2 => fall(x),
            _ => x.free_peff(),
        }
    }

    fn init_beam<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let target = Obj(x.u16(TARGET) as i16 as i32 as u32 & 0xff_ffff);
        let (tx, ty) = (target.u16(x.b, Enemy::X), target.u16(x.b, Enemy::Y));
        x.set16(Obj::X, tx);
        x.set16(Obj::Y, ty);
        target.set_u16(x.b, Shot::Y_FRACTION, 0x4000);
        let k = x.u8(Shot::SETTING);
        let priority = if k & 2 != 0 { 0x8000 } else { 0 };
        x.set16(Shot::Y_FRACTION, priority);
        let (_, cy) = camera(x);
        let z = cy.wrapping_add(0x100).wrapping_sub(x.u16(Obj::Y));
        x.set16(Obj::Z, z);
        let hold = player_data().banish_alt_bank.at8(x.b, k as u32);
        x.set8(HOLD, hold);
        let kind = target.u8(x.b, Enemy::KIND) as u32;
        let row = kind * 2;
        let mut frame = player_data().banish_frames_a.word_at(x.b, row);
        if target.u8(x.b, Enemy::CLASS) != 4 {
            frame = player_data().banish_frames_b.word_at(x.b, row);
        }
        let anim = frame & 0xff00 | (frame as u8).wrapping_add(k) as u16;
        let n = anim;
        x.anim_start(ANIMS, n);
        if k == 0 {
            crate::sound::play(x.b, SFX, x.o);
        }
    }

    fn fall<R: Bench>(x: &mut Cx<'_, R>) {
        let k = x.u8(Shot::SETTING);
        if x.u8(HOLD) != 0 {
            x.add8(HOLD, 0xff);
        } else if k & 1 != 0 {
            if x.u16(Obj::Z) > 0x60 {
                x.add16(Obj::Z, 0xfffd);
            }
        } else if (x.u16(Obj::Z) as i16) >= 0 {
            x.add16(Obj::Z, 0xfffd);
        }
        if vars::PAUSE.get(x.b) == 0 {
            x.add8(Obj::STATE, 2);
        }
        strike(x);
        x.anim_step();
        x.reinsert();
    }

    /// Beam 0 touches the ground: hurts the targeted monster (`0x4c4a0`).
    fn strike<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(Shot::SETTING) != 0 || (x.u16(Obj::Z) as i16) >= 0 {
            return;
        }
        let player = x.linked(f::OWNER);
        let v = player.u16(x.b, Player::SPELL_KILLS).wrapping_add(2);
        player.set_u16(x.b, Player::SPELL_KILLS, v);
        let monster = Obj(x.u16(TARGET) as i16 as i32 as u32 & 0xff_ffff);
        let hits = monster.u8(x.b, Victim::HITS_TAKEN).wrapping_add(1);
        monster.set_u8(x.b, Victim::HITS_TAKEN, hits);
        if monster.u8(x.b, Enemy::REACTION) == 0x5c {
            return;
        }
        monster.set_u32(x.b, Enemy::STATE_LONG, 0x0400_0000);
        monster.set_u8(x.b, Enemy::DISPLAY, 1);
        monster.set_u8(x.b, Enemy::REACTION, 0x5c);
        let (class_id, variant_id) = (player.u8(x.b, Player::INDEX), player.u8(x.b, Player::CHARACTER));
        monster.set_u8(x.b, Victim::HIT_BY_KIND, class_id);
        monster.set_u8(x.b, Victim::HIT_BY_VARIANT, variant_id);
        monster.set_u16(x.b, Enemy::HOLDER, player.0 as u16);
        monster.set_u8(x.b, Enemy::ATTACKER_FLIP, 0);
        if x.rand_and_b(1) != 0 {
            monster.set_u8(x.b, Enemy::ATTACKER_FLIP, 0x20);
        }
        let class = player.u8(x.b, Player::LEVEL) as u32;
        let table = if monster.u8(x.b, Enemy::CLASS) == 4 { POWER_TABLE_BOSS } else { POWER_TABLE };
        let mut damage = byte(x.b, table + class) as u16;
        if player.u8(x.b, Player::ARMOR) == 4 {
            damage += damage >> 2;
        }
        let hp = monster.u16(x.b, Enemy::HP).wrapping_sub(damage);
        monster.set_u16(x.b, Enemy::HP, if (hp as i16) < 0 { 0 } else { hp });
        if monster.u8(x.b, Enemy::CLASS) != 4 && matches!(monster.u8(x.b, Enemy::KIND), 0 | 0x13) {
            monster.set_u16(x.b, Enemy::HP, 0);
        }
        if monster.u16(x.b, Enemy::HP) == 0 && monster.u8(x.b, Victim::DEATH_CREDIT) != 0 {
            player.set_u8(x.b, Player::HOLY, 1);
            let v = monster.u8(x.b, Victim::DEATH_VALUE);
            vars::KILL_FLAG.set(x.b, v);
        }
        let mut d0 = (player.u8(x.b, Player::INDEX) as u16) << 8;
        d0 = d0 & 0xff00 | (d0 as u8).wrapping_add(monster.u8(x.b, Enemy::KIND)) as u16;
        let (max, value, monster_class) = (monster.u16(x.b, Enemy::HP_MAX), monster.u16(x.b, Enemy::HP), monster.u8(x.b, Enemy::CLASS));
        crate::events::create_popup(x.b, crate::events::Popup { header: d0, max, value, raw: monster_class == 2 });
    }
}

handlers! {
    0x4bc66 "peff_12" ice::run,
    0x4c1b6 "peff_13" blessing::run,
    0x4c2b4 "peff_14" banish::run,
}
