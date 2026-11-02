//! Opening sequences of the later scenes (from scene 33: plain fades, boss gates, the final scenes): one step per
//! frame.

use super::*;

/// Scenes 33 and 36: simple fade, one waiting frame.
pub(super) fn plain_fade(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a50);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in();
            crate::sound::play_global(x.b, crate::sound::SCENE_MUSIC);
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

/// Scene 35: title card, music, one waiting frame.
pub(super) fn title_card_music(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a50);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in();
            x.title_card();
            crate::sound::play_global(x.b, crate::sound::SCENE_MUSIC);
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

/// Scene 59: title card, music, one waiting frame.
pub(super) fn title_card_timed(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a5a);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in();
            x.title_card();
            crate::sound::play_global(x.b, crate::sound::SCENE_MUSIC);
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

/// Scene 38: the number of players sets the duration; dialogue 68; the end leaves `A5+0x4a` in place.
pub(super) fn scene_38(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a50);
    const MESSAGE: u32 = scratch(0x9a52);
    const DURATIONS: u32 = 0x22c28;
    match x.sub() {
        0 => {
            x.next_sub();
            x.set_v8(var::PLAYERS_FROZEN, 1);
            x.set_v8(var::INPUT_LOCK, 0);
            let players = x.v8(var::PLAYERS) as u32;
            let duration = x.prog16(DURATIONS + 2 * players);
            x.wr16(TIMER, duration);
            fading::DELAY.set(x.b, 2);
            fading::PAGES.set(x.b, 15);
            crate::palette::no_brightness(x.b);
            crate::kernel::create_task(x.b, 0x10, fading::RISE);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(68, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 60);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.leave_phase();
                x.set_v8(0x42, 1);
                x.set_v8(var::HOLD, 0);
            }
        }
        _ => {}
    }
    x.inputs();
    x.world();
}

/// Scenes 39, 41, 43, 45: waits for the players in state 6; 41 also launches 0xdfa4a.
pub(super) fn wait_players_6(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a56);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in();
            x.title_card();
            crate::sound::play_global(x.b, crate::sound::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(6) {
                if x.v8(var::SCENE) == 41 {
                    let _ = crate::effects_aids::scenery_11(x.b, 0, 0);
                }
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 40: players in state 4, grey lightning (0x1ab56) and four effects 0x13 (0x4cfbc), then a boss (type 12)
/// appears during the countdown.
pub(super) fn scene_40(x: &mut Ctx) {
    boss_gate(x, Opening::Lightning, 0x60, 12, 0x90)
}

/// Scene 42: like 40 with a lightning bolt (0x1aaba) and rain (0xb9942), and the type 11 boss.
pub(super) fn scene_42(x: &mut Ctx) {
    boss_gate(x, Opening::Rain, 0x40, 11, 0x80)
}

/// What scenes 40 and 42 launch when the players are in place.
#[derive(Clone, Copy)]
pub(super) enum Opening {
    Lightning,
    Rain,
}

/// Subroutine common to scenes 40 and 42.
pub(super) fn boss_gate(x: &mut Ctx, opening: Opening, spawn_at: u16, boss: u8, wait: u16) {
    const TIMER: u32 = scratch(0x9a56);
    match x.sub() {
        0 => {
            begin(x);
            x.fade_in();
            crate::sound::play_global(x.b, crate::sound::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(4) {
                x.next_sub();
                x.wr16(TIMER, wait);
                match opening {
                    Opening::Lightning => {
                        crate::palette::flash(x.b, crate::palette::Flash::Gray);
                        // Original glitch: the effects are linked to the "object" A0 left by the game loop (the
                        // address of the phase handler)
                        crate::families::creators::effects_13(x.b, Obj(x.phase_handler));
                    }
                    Opening::Rain => {
                        crate::palette::flash(x.b, crate::palette::Flash::B);
                        creators::rain(x.b);
                    }
                }
            }
        }
        4 => {
            if x.tick(TIMER) {
                x.end_scene();
                crate::palette::restore_flash(x.b);
            } else if x.rd16(TIMER) == spawn_at {
                x.spawn(&pools::BOSS, boss);
            }
        }
        _ => {}
    }
    x.world();
}

/// Scenes 44 and 111: type 0x11 boss, waits for state 10, dialogue 43.
pub(super) fn scene_44(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a56);
    const MESSAGE: u32 = scratch(0x9a58);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in();
            if x.v8(0x30) == 0 {
                x.sobj_6(23);
            }
            crate::sound::play_global(x.b, crate::sound::SCENE_MUSIC);
        }
        2 => {
            x.next_sub();
            let boss = x.spawn(&pools::BOSS, 0x11);
            boss.set_u8(x.b, Obj::VARIANT, 1);
        }
        4 => {}
        6 => {
            if x.players_in_state(10) {
                x.next_sub();
                x.wr16(TIMER, 48);
            }
        }
        8 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(43, MESSAGE);
            }
        }
        10 => {
            if x.message_done(MESSAGE) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 47: fade to a level in steps; the end waits for the end of the fade.
pub(super) fn scene_47(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a5a);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in_stepped();
            x.title_card();
            crate::sound::play_global(x.b, crate::sound::SCENE_MUSIC);
        }
        2 => {
            if fading::STEP.get(x.b) == 0 {
                x.end_scene();
                crate::video::clear_layer1_start(x.b);
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 54 (Tel'Arin): players in states 4 then 8, dialogue 43; a type 0x11 boss if `A5+0x66`.
pub(super) fn scene_54(x: &mut Ctx) {
    const MESSAGE: u32 = scratch(0x9a5c);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(scratch(0x9a5a), 1);
            x.fade_in();
            x.title_card();
            if x.v8(0x66) == 0 {
                x.set_sub(4);
                let boss = x.spawn(&pools::BOSS, 0x11);
                boss.set_u8(x.b, Obj::VARIANT, 2);
            }
            crate::sound::play_global(x.b, crate::sound::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(4) {
                x.end_scene();
            }
        }
        4 => {
            if x.players_in_state(8) {
                x.next_sub();
                x.message(43, MESSAGE);
            }
        }
        6 => {
            if x.message_done(MESSAGE) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 58: a type 5 boss, title card; players in state 4, dialogue 75.
pub(super) fn scene_58(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a5a);
    const MESSAGE: u32 = scratch(0x9a5c);
    match x.sub() {
        0 => {
            begin(x);
            x.fade_in();
            x.spawn(&pools::BOSS, 5);
            x.title_card();
            crate::sound::play_global(x.b, crate::sound::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(4) {
                x.next_sub();
                x.wr16(TIMER, 48);
            }
        }
        4 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(75, MESSAGE);
            }
        }
        6 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 8);
            }
        }
        8 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 64: effect 75, title card; players in state 4, dialogue 77.
pub(super) fn scene_64(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a5a);
    const MESSAGE: u32 = scratch(0x9a5c);
    match x.sub() {
        0 => {
            begin(x);
            x.fade_in();
            x.spawn(&pools::EFFCT, 75);
            x.title_card();
            crate::sound::play_global(x.b, crate::sound::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(4) {
                x.next_sub();
                x.message(77, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 0x100);
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

/// Scene 65: players in state 6, dialogue 81; the end opens transition 0xeadd4.
pub(super) fn scene_65(x: &mut Ctx) {
    const MESSAGE: u32 = scratch(0x9a5c);
    match x.sub() {
        0 => {
            begin(x);
            x.fade_in();
            x.title_card();
            crate::sound::play_global(x.b, crate::sound::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(6) {
                x.next_sub();
                x.message(81, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.leave_phase();
                x.set_v8(0x42, 1);
                x.set_v8(var::HOLD, 0);
                crate::sound::play_global(x.b, crate::sound::CMD_4D);
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 66 (last level): chains three dialogues (82, 83, 84) around a forced scroll and
/// four effects 0x50; `A5+0x5c` jumps directly to the effects step.
pub(super) fn scene_66(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a5a);
    const MESSAGE: u32 = scratch(0x9a5c);
    match x.sub() {
        0 => {
            if x.v8(0x5c) != 0 {
                x.set_sub(8);
                final_effects(x);
            } else {
                x.next_sub();
                x.wr16(scratch(0x9a60), 0);
                x.lock();
                x.fade_in_stepped();
                x.spawn(&pools::BOSS, 10);
            }
        }
        2 => {
            if x.players_in_state(4) {
                x.next_sub();
                x.message(82, MESSAGE);
                crate::video::clear_layer1_start(x.b);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 0x100);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
            if x.rd16(TIMER) == 0x80 {
                crate::sound::play_global(x.b, crate::sound::CMD_21);
            }
        }
        8 => final_effects(x),
        10 => {
            if x.v16(0x51e) != 0 {
                x.next_sub();
                x.wr16(TIMER, 48);
                crate::sound::play_global(x.b, crate::sound::CMD_2D);
            }
        }
        12 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(83, MESSAGE);
                crate::video::clear_layer1_start(x.b);
            }
        }
        14 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.message(84, MESSAGE);
            }
        }
        16 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 66, sub-step 8: fade to a level in steps and four effects 0x50 (variants 0 to 3).
pub(super) fn final_effects(x: &mut Ctx) {
    x.next_sub();
    x.lock();
    x.fade_in_stepped();
    for variant in 0..4 {
        let effect = x.spawn(&pools::EFFCT, 0x50);
        effect.set_u8(x.b, Obj::VARIANT, variant);
    }
    crate::sound::play_global(x.b, crate::sound::SCENE_MUSIC);
}

/// Scene 105: an effect 0x24, dialogues 86 and 87, two possible endings (depending on `A5+0x5c`), dialogue 42.
pub(super) fn scene_105(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a5a);
    const MESSAGE: u32 = scratch(0x9a5c);
    const BOSS: u32 = scratch(0x9a5e);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 0x80);
            x.fade_in();
            let effect = x.spawn(&pools::EFFCT, 0x24);
            effect.set_u8(x.b, Obj::VARIANT, 2);
            crate::sound::play_global(x.b, crate::sound::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(86, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                if x.v8(0x5c) == 0 {
                    x.set_sub(12);
                    x.wr16(TIMER, 0x40);
                    let finale = finale(x);
                    x.wr16(BOSS, finale.0 as u16);
                    crate::sound::play_global(x.b, crate::sound::CMD_8B);
                } else {
                    x.next_sub();
                    x.message(87, MESSAGE);
                    let effect = x.spawn(&pools::EFFCT, 0x50);
                    effect.set_u8(x.b, Obj::VARIANT, 6);
                }
            }
        }
        6 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                crate::sound::play_global(x.b, crate::sound::CMD_8B);
            }
        }
        8 => {
            if x.players_in_state(8) {
                x.next_sub();
                x.message(42, MESSAGE);
            }
        }
        10 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                let finale = finale(x);
                x.wr16(BOSS, finale.0 as u16);
            }
        }
        12 | 14 => {
            let boss = x.load_obj(BOSS);
            if boss.u8(x.b, 0x2f) != 0 {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}
