//! Custom states of the exit sequences (the states that are not shared by several sequences).

use super::*;

/// Wait of sequence 30 by the number of players (PC-relative words).
const PLAYER_WAITS: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0xf5542);
use crate::objects::Obj;

// ---------------------------------------------------------------------------------------------
// Custom states

/// Sequence 0 (the most common): players leave, fade.
pub(super) fn seq0_start(ex: &mut Ex) {
    ex.next();
    ex.fade(2, fading::DESCENT.0);
    var::INPUT_LOCK.set(ex.x.b, 0);
    crate::inputs::clear_inputs(ex.x.b);
    ex.x.world();
}

/// Sequence 6: like `seq0_start`, with sound command 0xe0a82(4).
pub(super) fn seq6_start(ex: &mut Ex) {
    ex.next();
    ex.fade(2, fading::DESCENT.0);
    var::INPUT_LOCK.set(ex.x.b, 0);
    crate::inputs::clear_inputs(ex.x.b);
    crate::sound::service(ex.x.b, crate::sound::Service::Ff07 { lock: 0, line: 4 });
    ex.x.world();
}

/// Sequence 20: players leave at the end of the timer (48 more frames); full update.
pub(super) fn seq20_walk(ex: &mut Ex) {
    walk_wait(ex, Some(48));
    run_update(ex, Update::Full);
}

/// Sequences 20 and 31: exit fade at the end of the timer; the world goes on.
pub(super) fn fade_out_world(ex: &mut Ex) {
    if ex.countdown() {
        ex.next();
        ex.fade(2, fading::DESCENT.0);
    }
    ex.x.world();
}

/// Sequence 27: camera, timer + 90, transition variables and shop block cleared.
pub(super) fn seq27_clear(ex: &mut Ex) {
    ex.next();
    ex.x.set_v16(0x51e, 4);
    let t = ex.b16(blk::TIMER).wrapping_add(90);
    ex.set_b16(blk::TIMER, t);
    ex.reset_transition();
    ex.reset_shop();
}

/// Sequence 29: timer + 80, transition variables cleared, then the wait in the same frame.
pub(super) fn seq29_start(ex: &mut Ex) {
    ex.next();
    let t = ex.b16(blk::TIMER).wrapping_add(80);
    ex.set_b16(blk::TIMER, t);
    ex.reset_transition();
    seq29_walk(ex);
}

/// Sequence 29: at the end of the timer, players leave and the fade starts; full update.
pub(super) fn seq29_walk(ex: &mut Ex) {
    if ex.countdown() {
        ex.next();
        ex.start_walk_out(false);
        ex.fade(2, fading::DESCENT.0);
    }
    run_update(ex, Update::Full);
}

/// Sequence 31: timer 192, inputs released.
pub(super) fn seq31_start(ex: &mut Ex) {
    ex.next();
    ex.set_b16(blk::TIMER, 192);
    var::INPUT_LOCK.set(ex.x.b, 0);
    crate::inputs::clear_inputs(ex.x.b);
    ex.x.world();
}

/// Sequence 33: transition variables and shop block cleared, then the wait in the same frame.
pub(super) fn seq33_start(ex: &mut Ex) {
    ex.next();
    ex.reset_transition();
    ex.reset_shop();
    seq33_walk(ex);
}

/// Sequence 33: at the end of the timer, exit fade then players leave.
pub(super) fn seq33_walk(ex: &mut Ex) {
    if ex.countdown() {
        ex.next();
        PAUSE.set(ex.x.b, 0);
        var::INPUT_LOCK.set(ex.x.b, 0);
        crate::inputs::clear_inputs(ex.x.b);
        ex.fade(2, fading::DESCENT.0);
        ex.release_players(false);
    }
    ex.x.world();
}

/// Sequence 10: transition variables cleared, timer 128.
pub(super) fn seq10_clear(ex: &mut Ex) {
    ex.next();
    ex.set_b16(blk::TIMER, 128);
    ex.reset_transition();
}

/// Sequence 32: depending on the route (`A5+0x5c`), the exit or the bonus screen 0xf6492.
pub(super) fn seq32_route(ex: &mut Ex) {
    if ex.x.v8(0x5c) != 0 {
        ex.set_step(24);
        if let Some(St::Leave(leave)) = SEQ_32.states.get(6) {
            do_leave(ex, leave);
        }
    } else {
        ex.set_step(20);
        redispatch(ex, &SEQ_32);
    }
}

/// Sequence 1: players leave immediately.
pub(super) fn walk_out_now(ex: &mut Ex) {
    ex.next();
    ex.start_walk_out(false);
}

/// Sequence 1: transition variables cleared, then event 0xdc5fe(1).
pub(super) fn seq1_clear(ex: &mut Ex) {
    ex.next();
    ex.reset_transition();
    crate::families::creators::scene_object(ex.x.b, 1);
}

/// Sequence 2: the timer (word) is decremented once, then the players leave if there are any.
pub(super) fn seq2_start(ex: &mut Ex) {
    // the timer (word) starts at 0: it is decremented as long as it stays positive
    let left = ex.b16(blk::TIMER).wrapping_sub(1);
    ex.set_b16(blk::TIMER, left);
    if left as i16 >= 0 || var::PLAYERS.get(ex.x.b) == 0 {
        return;
    }
    ex.next();
    ex.start_walk_out(false);
}

/// Sequence 2: waits for all players to be in exit state 0xc0c00, then dialogue 7.
pub(super) fn seq2_wait_exit(ex: &mut Ex) {
    ex.reset_transition();
    for k in 0..4 {
        let p = PLAYERS + k * PLAYER_STRIDE;
        if rd8(ex.x.b, p) != 0 && Obj(p).u16(ex.x.b, player::HEALTH) != 0 && crate::mem::rd32(ex.x.b, p + P_STATE_LONG) != 0x000c_0c00 {
            return;
        }
    }
    ex.next();
    ex.message(7);
}

/// Sequence 2: message done, the boss of object `+0x10` goes to state 0x04020a00.
pub(super) fn seq2_message_done(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
        let o = ex.object(blk::OBJECT);
        o.set_u32(ex.x.b, Obj::STATE, 0x0402_0a00);
        ex.set_b16(blk::TIMER, 240);
    }
}

pub(super) fn seq2_boss_state(ex: &mut Ex) {
    let o = ex.object(blk::OBJECT);
    if o.u8(ex.x.b, 4) == 6 {
        ex.next();
        ex.set_b16(blk::TIMER, 160);
    }
}

pub(super) fn seq2_countdown(ex: &mut Ex) {
    if ex.countdown() {
        ex.next();
        ex.set_b16(blk::TIMER, 128);
        crate::camera_modes::start_shake(ex.x.b, crate::camera_modes::shake::B, 30);
        crate::sound::play_global(ex.x.b, crate::sound::CMD_98);
    }
}

/// Sequence 4: after the exit, an effect appears; if a player is in state 4 or 5, dialogue 33.
pub(super) fn seq4_probe(ex: &mut Ex) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    for k in 0..4 {
        let p = PLAYERS + k * PLAYER_STRIDE;
        if rd8(ex.x.b, p) != 0 && Obj(p).u16(ex.x.b, player::HEALTH) != 0 {
            let character = rd8(ex.x.b, p + 3);
            if character == 4 || character == 5 {
                ex.next();
                ex.message(33);
                return;
            }
        }
    }
    let effect = ex.x.spawn(&pools::EFFCT, 0x59);
    ex.set_b16(blk::EFFECT, effect.0 as u16);
}

pub(super) fn seq4_effect(ex: &mut Ex) {
    let e = ex.object(blk::EFFECT);
    if e.u8(ex.x.b, 5) == 2 {
        ex.next();
        ex.message(33);
    }
}

pub(super) fn message_then_next(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
    }
}

/// Sequences 5 and 7: players leave, effect `A5+0x46` receives 0x20000 at `+0x58`, exit fade.
pub(super) fn walk_with_effect(ex: &mut Ex) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    PAUSE.set(ex.x.b, 0);
    var::INPUT_LOCK.set(ex.x.b, 0);
    crate::inputs::clear_inputs(ex.x.b);
    ex.set_b16(blk::TIMER, 144);
    ex.release_players(false);
    let effect = Obj(ex.x.v16(0x46) as i16 as i32 as u32 & 0xff_ffff);
    wr32(ex.x.b, effect.0 + 0x58, 0x2_0000);
    ex.fade(1, fading::DESCENT.0);
}

/// Sequence 10: players receive their rank then 0xd5f12.
pub(super) fn seq10_walk(ex: &mut Ex) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    ex.start_walk_out(true);
    crate::families::set_residue::clear_columns(ex.x.b);
}

/// Sequence 14: waits for state 12 of the players, then dialogue 35.
pub(super) fn seq14_wait_players(ex: &mut Ex) {
    if ex.all_in_state(12) {
        ex.next();
        ex.message(35);
    }
}

pub(super) fn seq14_message_done(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
        ex.set_b16(blk::TIMER, 256);
        crate::video::clear_layer1_start(ex.x.b);
    }
}

pub(super) fn seq14_countdown(ex: &mut Ex) {
    if ex.countdown() {
        ex.next();
        ex.message(36);
        crate::sound::play_global(ex.x.b, crate::sound::CMD_89_89);
    }
}

pub(super) fn seq14_message_done_wait(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
        ex.set_b16(blk::TIMER, 288);
    }
}

/// Sequence 15: clears the temporary variable A5-0x65ac, resets transition and shop.
pub(super) fn seq15_clear(ex: &mut Ex) {
    ex.next();
    ex.x.wr16(scratch(0x9a54), 0);
    ex.reset_transition();
    ex.reset_shop();
}

/// Sequence 15: fade, fade script, event 0xdc5fe(3).
pub(super) fn seq15_fade(ex: &mut Ex) {
    ex.next();
    ex.fade(2, fading::RISE.0);
    ex.clear_text();
    crate::families::creators::scene_object(ex.x.b, 3);
}

/// Sequence 16: transition and shop cleared, players leave.
pub(super) fn seq16_start(ex: &mut Ex) {
    ex.next();
    ex.reset_transition();
    ex.reset_shop();
    ex.start_walk_out(false);
}

/// Sequence 21: players leave, dialogue 70; the next scene depends on the result.
pub(super) fn seq21_start(ex: &mut Ex) {
    ex.next();
    PAUSE.set(ex.x.b, 0);
    var::INPUT_LOCK.set(ex.x.b, 0);
    crate::inputs::clear_inputs(ex.x.b);
    ex.reset_transition();
    ex.reset_shop();
    ex.message(70);
    ex.release_players(false);
}

/// Sequence 21: the finished message gives an index (`+0x23`) into a table of next scenes.
pub(super) fn seq21_route(ex: &mut Ex) {
    const NEXT_SCENES: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0xf4a0c);
    let m = ex.object(blk::MESSAGE);
    if m.u8(ex.x.b, 0x22) != 0 {
        ex.next();
        let index = m.u8(ex.x.b, 0x23) as u32;
        let scene = NEXT_SCENES.pc_byte(ex.x.b, index);
        ex.set_b8(blk::NEXT_SCENE, scene);
        ex.fade(1, fading::DESCENT.0);
    }
}

/// Sequence 23: players leave then exit fade.
pub(super) fn seq23_walk(ex: &mut Ex) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    ex.start_walk_out(false);
    ex.fade(2, fading::DESCENT.0);
}

/// Sequence 25: depending on the route (`A5+0x5c`), jumps to state 20 (bonus screen) or fades.
pub(super) fn seq25_route(ex: &mut Ex) {
    if ex.x.v8(0x5c) == 0 {
        ex.set_step(20);
        return redispatch(ex, &SEQ_25);
    }
    ex.next();
    ex.set_b16(blk::TIMER, 128);
    ex.fade(2, fading::RISE.0);
    ex.clear_text();
}

/// Sequence 28: like `Clear`, the timer increases by 90.
pub(super) fn seq28_clear(ex: &mut Ex) {
    ex.next();
    let t = ex.b16(blk::TIMER).wrapping_add(90);
    ex.set_b16(blk::TIMER, t);
    ex.reset_transition();
    ex.reset_shop();
}

/// Sequence 30: series of dialogues (79, 80) around an effect 75 and the exit.
pub(super) fn seq30_message_79(ex: &mut Ex) {
    if ex.countdown() {
        ex.next();
        ex.message(79);
    }
}

pub(super) fn seq30_message_done(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
        ex.set_b16(blk::TIMER, 80);
    }
}

pub(super) fn seq30_effect(ex: &mut Ex) {
    if ex.b16(blk::TIMER) == 32 {
        let e = ex.x.spawn(&pools::EFFCT, 75);
        e.set_u8(ex.x.b, Obj::VARIANT, 1);
        crate::sound::play_global(ex.x.b, crate::sound::CMD_38E);
    }
    if ex.countdown() {
        ex.next();
        ex.message(80);
    }
}

pub(super) fn seq30_players_wait(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
        let players = var::PLAYERS.get(ex.x.b) as u32;
        let t = PLAYER_WAITS.pc_word(ex.x.b, 2 * players);
        ex.set_b16(blk::TIMER, t);
    }
}

/// Sequence 32: clears, draws; if the route is taken (`A5+0x5c`) jumps to state 12.
pub(super) fn seq32_clear(ex: &mut Ex) {
    ex.next();
    ex.reset_transition();
    ex.reset_shop();
    if ex.x.v8(0x5c) != 0 {
        ex.set_step(12);
        return redispatch(ex, &SEQ_32);
    }
    walk_wait(ex, None);
}

/// Sequence 35: transition variables cleared, then players leave after the wait.
pub(super) fn seq35_clear(ex: &mut Ex) {
    ex.next();
    ex.reset_transition();
    seq35_walk(ex);
}

pub(super) fn seq35_walk(ex: &mut Ex) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    PAUSE.set(ex.x.b, 0);
    ex.x.wr16(scratch(0xaa84), 0x1400);
    var::INPUT_LOCK.set(ex.x.b, 0);
    crate::inputs::clear_inputs(ex.x.b);
    ex.release_players(false);
}

pub(super) fn seq35_fade(ex: &mut Ex) {
    ex.next();
    ex.set_b16(blk::TIMER, 64);
    ex.fade(2, fading::RISE.0);
    ex.clear_text();
    crate::sound::service(ex.x.b, crate::sound::Service::Ff07 { lock: 0, line: 4 });
}

pub(super) fn seq35_message(ex: &mut Ex) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    ex.message(88);
    // Original glitch: the failure is not tested, the A1 left by the allocation is stored
    let set = crate::families::creators::set_15_pure(ex.x.b, 24).unwrap_or_else(|a1| a1);
    ex.set_b16(blk::OBJECT, set.0 as u16);
    crate::camera_modes::start_shake(ex.x.b, crate::camera_modes::shake::B, 0x7fff);
    crate::sound::play_global(ex.x.b, crate::sound::CMD_9A);
}

pub(super) fn seq35_boss(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
        let o = ex.object(blk::OBJECT);
        o.set_u8(ex.x.b, 4, 4);
        ex.x.set_v16(0x544, 1);
    }
}
