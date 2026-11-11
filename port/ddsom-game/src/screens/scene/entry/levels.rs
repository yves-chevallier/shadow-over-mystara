//! Opening sequences of the first scenes (levels 1 to 4, caves, the time challenge of scene 11, the
//! title cards): one step per frame.

use super::*;
use crate::world::ram::Long;

/// Setting of the enemies brought in by the opening sequences (word `+0x42`).
const ENEMY_SETTING: u32 = 0x42;
use crate::world::ram::{Word};

/// Scene 1 (level 1): fade, title card, first dialogue, then three enemies enter from the right.
pub(super) fn stage_1(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a32);
    const MESSAGE: u32 = scratch(0x9a34);
    match x.sub() {
        0 => {
            begin(x);
            Word(TIMER).set(x.b, 0x140);
            x.fade_in();
            creators::trails_pure(x.b);
            x.title_card();
            crate::world::objects::rank::reset(x.b);
            crate::hw::sound::play_global(x.b, crate::hw::sound::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                // Original glitch: the failure is not tested, the A1 left by the allocation is stored
                let message = crate::actors::families::creators::scene_set_or_rest(x.b, 0);
                Long(MESSAGE).set(x.b, message.0);
            }
        }
        4 => {
            let message = Obj(Long(MESSAGE).get(x.b));
            if message.u8(x.b, crate::screens::obj::MESSAGE_DONE) != 0 {
                x.next_sub();
                Word(TIMER).set(x.b, 0xb0);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.next_sub();
                Word(TIMER).set(x.b, 0x60);
            }
        }
        8 => {
            if x.tick(TIMER) {
                x.leave_phase();
                var::HOLD.set(x.b, 0);
                var::PLAYERS_FROZEN.set(x.b, 0);
            } else if Word(TIMER).get(x.b) == 0x50 {
                for (px, py) in [(0x180, 0x210), (0x230, 0x220), (0x2d0, 0x200)] {
                    let o = x.spawn(&pools::ENEMY, 0x14);
                    o.set_u8(x.b, Obj::ACTIVE, 2);
                    o.set_u8(x.b, Obj::VARIANT, 1);
                    o.set_u16(x.b, Obj::X, px);
                    o.set_u16(x.b, Obj::Y, py);
                    o.set_u16(x.b, Obj::VX, 0xfb80);
                }
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 2: fade, title card; waits a moment.
pub(super) fn stage_2(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a38);
    match x.sub() {
        0 => {
            begin(x);
            Word(TIMER).set(x.b, 1);
            x.fade_in();
            x.title_card();
            crate::world::objects::rank::reset(x.b);
            crate::hw::sound::play_global(x.b, crate::hw::sound::CMD_42);
            crate::hw::sound::play_global(x.b, crate::hw::sound::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 4: title, then two scene effects at intervals.
pub(super) fn scene_4(x: &mut Ctx) {
    const SHAKE: u32 = scratch(0x9a3c);
    const TIMER: u32 = scratch(0x9a3a);
    match x.sub() {
        0 => {
            begin(x);
            Word(SHAKE).set(x.b, 0);
            Word(TIMER).set(x.b, 0x158);
            x.fade_in();
            x.title_card();
            crate::world::objects::rank::reset(x.b);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                Word(TIMER).set(x.b, 0x40);
                x.set_v8(0x54f, 8);
            }
        }
        4 => {
            if x.tick(TIMER) {
                x.next_sub();
                Word(SHAKE).set(x.b, 1);
                Word(TIMER).set(x.b, 0x130);
                x.set_v8(0x500, 8);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scenes 7 and 9 (boss cave): an effect, a boss and a piece of scenery enter; 7 resets the rank to zero.
pub(super) fn boss_cave(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a3a);
    match x.sub() {
        0 => {
            begin(x);
            Word(TIMER).set(x.b, 0x90);
            x.fade_in();
            let scene = var::SCENE.get(x.b);
            if scene != 9 {
                x.title_card();
            }
            let effect = x.spawn(&pools::EFFCT, 13);
            let camera_x = x.v16(0x506);
            let camera_y = x.v16(0x50a);
            effect.set_u16(x.b, Obj::X, camera_x.wrapping_sub(0x100));
            effect.set_u16(x.b, Obj::Y, camera_y.wrapping_add(0x50));
            x.set_v16(0x46, effect.0 as u16);
            x.spawn(&pools::BOSS, 13);
            let set = x.spawn(&pools::SET, 14);
            if scene == 9 {
                set.set_u8(x.b, Obj::VARIANT, 1);
            }
            if scene == 7 {
                crate::world::objects::rank::reset(x.b);
            }
            crate::hw::sound::play_global(x.b, crate::hw::sound::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 8: an effect 0x22 near the camera, a sound command, then effect 0xb8956.
pub(super) fn scene_8(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a3a);
    match x.sub() {
        0 => {
            begin(x);
            Word(TIMER).set(x.b, 0x100);
            x.fade_in();
            let effect = x.spawn(&pools::EFFCT, 0x22);
            let camera_x = x.v16(0x506);
            let camera_y = x.v16(0x50a);
            effect.set_u16(x.b, Obj::X, camera_x.wrapping_sub(0x60));
            effect.set_u16(x.b, Obj::Y, camera_y.wrapping_add(0x50));
            x.set_v16(0x46, effect.0 as u16);
            crate::hw::sound::service(x.b, crate::hw::sound::Service::GeneralStop);
            creators::countdown_timer(x.b, 1);
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 10: forced transition (`A5+0x51e` = 4), an effect 0x13 and a piece of scenery.
pub(super) fn scene_10(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a3a);
    match x.sub() {
        0 => {
            x.next_sub();
            x.set_v16(0x51e, 4);
            x.lock();
            Word(TIMER).set(x.b, 0xc0);
            x.fade_in();
            let effect = x.spawn(&pools::EFFCT, 0x13);
            x.set_v16(0x46, effect.0 as u16);
            let set = x.spawn(&pools::SET, 14);
            set.set_u8(x.b, Obj::VARIANT, 1);
            crate::hw::sound::play_global(x.b, crate::hw::sound::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.leave_phase();
                x.set_v16(0x51e, 0);
                var::HOLD.set(x.b, 0);
                var::PLAYERS_FROZEN.set(x.b, 0);
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 11: title card, two dialogues (95), an enemy (21), a forced scroll.
pub(super) fn scene_11(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a3e);
    const MESSAGE: u32 = scratch(0x9a40);
    const ENEMY: u32 = scratch(0x9a42);
    match x.sub() {
        0 => {
            begin(x);
            Word(TIMER).set(x.b, 0x78);
            x.fade_in();
            x.title_card();
            creators::swimmers(x.b);
            crate::world::objects::rank::reset(x.b);
            crate::hw::sound::play_global(x.b, crate::hw::sound::CMD_7D);
            crate::hw::sound::play_global(x.b, crate::hw::sound::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                Word(TIMER).set(x.b, 0xa0);
                creators::bench(x.b);
            }
        }
        4 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(95, MESSAGE);
            } else if Word(TIMER).get(x.b) == 0x40 {
                creators::bench(x.b);
            }
        }
        6 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                Word(TIMER).set(x.b, 0x10);
            }
        }
        8 => {
            if x.tick(TIMER) {
                x.next_sub();
                Word(TIMER).set(x.b, 0xe0);
                let o = x.spawn(&pools::ENEMY, 0x15);
                o.set_u8(x.b, Obj::VARIANT, 8);
                o.set_u16(x.b, ENEMY_SETTING, 0x401);
                x.store_obj(ENEMY, o);
            }
        }
        10 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.set_v16(0x4ba, 0x100);
                Word(TIMER).set(x.b, 12);
                crate::services::palette::set_flash_color(x.b, crate::services::palette::FLASH_COLOR_WHITE_VALUE);
                crate::services::camera_modes::start_shake(x.b, crate::services::camera_modes::shake::B, 60);
            }
        }
        12 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.set_v16(0x4ba, 0);
                Word(TIMER).set(x.b, 0x100);
                let o = x.load_obj(ENEMY);
                o.set_u32(x.b, Obj::STATE, 0x0206_0000);
            }
        }
        14 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scenes 5 and 20: waits for all the players in the game to be in state 10.
pub(super) fn wait_players_10(x: &mut Ctx) {
    match x.sub() {
        0 => {
            x.next_sub();
            x.set_v8(0x67, 0);
            x.lock();
            x.fade_in();
            x.title_card();
            crate::world::objects::rank::reset(x.b);
            crate::hw::sound::play_global(x.b, crate::hw::sound::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(10) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 14 and variants: fade, title card, one waiting frame.
pub(super) fn title_card(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a44);
    match x.sub() {
        0 => {
            begin(x);
            Word(TIMER).set(x.b, 1);
            x.fade_in();
            x.title_card();
            crate::world::objects::rank::reset(x.b);
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 24 (Manticore): two dialogues (31, then 32 if a player is in state 4), a boss.
pub(super) fn scene_24(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a46);
    const MESSAGE: u32 = scratch(0x9a48);
    const BOSS: u32 = scratch(0x9a4a);
    match x.sub() {
        0 => {
            x.next_sub();
            x.set_v8(0x67, 0);
            x.lock();
            Word(TIMER).set(x.b, 0xb4);
            x.fade_in();
            crate::world::objects::rank::reset(x.b);
            x.title_card();
            crate::hw::sound::play_global(x.b, crate::hw::sound::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(6) {
                x.next_sub();
                x.message(31, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                let mut found = false;
                for k in 0..4 {
                    let p = PLAYERS + k * PLAYER_STRIDE;
                    if Obj(p).u8(x.b, Obj::ACTIVE) != 0 {
                        let character = Obj(p).u8(x.b, Obj::VARIANT) as u16 >> 1;
                        found |= character as u8 == 2;
                    }
                }
                if found {
                    x.message(32, MESSAGE);
                } else {
                    x.next_sub();
                    Word(TIMER).set(x.b, 60);
                }
            }
        }
        6 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                Word(TIMER).set(x.b, 90);
            }
        }
        8 => {
            if x.tick(TIMER) {
                x.next_sub();
                Word(TIMER).set(x.b, 0xa0);
                let boss = x.spawn(&pools::BOSS, 6);
                x.store_obj(BOSS, boss);
            }
        }
        10 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 26 (Lich): fade, dialogue 34, then a sound command.
pub(super) fn scene_26(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a46);
    const MESSAGE: u32 = scratch(0x9a48);
    match x.sub() {
        0 => {
            x.next_sub();
            x.set_v8(0x67, 0);
            x.lock();
            Word(TIMER).set(x.b, 0x60);
            x.fade_in();
            crate::world::objects::rank::reset(x.b);
            crate::hw::sound::play_global(x.b, crate::hw::sound::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(34, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                Word(TIMER).set(x.b, 30);
                creators::camera_in_mode_60(x.b);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 27: title card, dialogue 37.
pub(super) fn scene_27(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a4c);
    const MESSAGE: u32 = scratch(0x9a4e);
    match x.sub() {
        0 => {
            begin(x);
            Word(TIMER).set(x.b, 0x100);
            x.fade_in();
            x.title_card();
            crate::hw::sound::play_global(x.b, crate::hw::sound::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(37, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 28: title card, dialogue 140; an external trigger (`A5+0x4e`) jumps to sub-step 6.
pub(super) fn scene_28(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a50);
    const MESSAGE: u32 = scratch(0x9a52);
    const MUSIC_STATE: u32 = scratch(0x9a54);
    match x.sub() {
        0 => {
            begin(x);
            Word(TIMER).set(x.b, 0x100);
            x.fade_in();
            x.title_card();
            if Word(MUSIC_STATE).get(x.b) == 0 {
                Word(MUSIC_STATE).set(x.b, 1);
                crate::hw::sound::play_global(x.b, crate::hw::sound::SCENE_MUSIC);
            }
            if x.v8(0x4e) != 0 {
                x.set_sub(6);
                Word(TIMER).set(x.b, 1);
                if Word(MUSIC_STATE).get(x.b) != 2 {
                    Word(MUSIC_STATE).set(x.b, 2);
                    crate::hw::sound::play_with(x.b, crate::hw::sound::FROM_D4, crate::hw::sound::Source { d4: Some(24), ..Default::default() });
                    x.world();
                    return;
                }
            }
            x.set_v32(0x38, 0x0459_5900);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(140, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                Word(TIMER).set(x.b, 80);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 32 (black dragon): title card; an opening boss (type 0x13) drives the rest; ends when the
/// players are done (state 6).
pub(super) fn scene_32(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a50);
    match x.sub() {
        0 => {
            begin(x);
            Word(TIMER).set(x.b, 0xe0);
            x.fade_in();
            x.title_card();
            crate::hw::sound::play_global(x.b, crate::hw::sound::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.spawn(&pools::BOSS, 0x13);
            }
        }
        4 => {} // the opening boss itself moves to sub-step 6
        6 => {
            if x.players_in_state(6) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 34: partial input lock (0x0f), opening sound command.
pub(super) fn scene_34(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a50);
    match x.sub() {
        0 => {
            begin(x);
            var::INPUT_LOCK.set(x.b, 0x0f);
            Word(TIMER).set(x.b, 1);
            x.fade_in();
            if x.v8(0x59) != 0 {
                creators::type_68_pure(x.b);
            }
            crate::hw::sound::service(x.b, crate::hw::sound::Service::GeneralStop);
            crate::hw::sound::play_global(x.b, crate::hw::sound::SCENE_MUSIC);
            crate::hw::sound::service(x.b, crate::hw::sound::Service::Ff06 { lock: 144, line: 0 });
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}
