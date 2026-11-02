use super::*;

const ANIMS: u32 = 0x170c3e;
/// Creation animation per object type (variant / 2).
const INIT_ANIMS: [u32; 6] = [2, 4, 5, 5, 4, 7];
/// Delays (16 words) drawn at random for variants 6 and 7.
const DELAYS: u32 = 0xa55b2;
const PICKUP_SOUND: crate::sound::Sound = crate::sound::CMD_36E_46E;
const COUNTDOWN: u32 = 0x56;
/// Variable `A5+0x6f`: blinking phase; `A5+0x33`: blinking suspended.
const BLINK_PHASE: u32 = 0x6f;
const BLINK_OFF: u32 = 0x33;
const STAGE: u32 = 0x34;
const STAGE_NO_CULL: u8 = 0x19;
const LINK: u32 = 0x5c;
const HIDDEN: u32 = 0x40;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            let v = x.u8(0x3);
            if v < 12 {
                init(x, v);
            }
            x.reinsert();
        }
        2 => live(x),
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>, v: u8) {
    let k = v as usize / 2;
    x.add8(0x4, 2);
    x.set8(f::BOX_SET, 7);
    x.set8(0x2a, 1);
    x.set8(0x2b, 0x14);
    x.set8(f::ATTACK_ID, 0x21);
    if v != 2 * k as u8 {
        x.set8(0x2b, 0x13);
        x.set8(f::ATTACK_ID, 0x22);
    }
    x.anim_start(ANIMS, (INIT_ANIMS[k]) as u16);
    if k == 3 {
        random_delay(x);
    }
}

/// Delay drawn from table 0xa55b2.
fn random_delay<R: Bench>(x: &mut Cx<'_, R>) {
    let r = random(x) & 0xf;
    let d = x.rom16(DELAYS + 2 * r);
    x.set16(COUNTDOWN, d);
}

fn live<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(HIDDEN) != 0 {
        return x.reinsert();
    }
    let v = x.u8(0x3);
    match v / 2 {
        0 => touch_then_pick(x, true),
        1 => flash_animate(x),
        2 => touch_then_pick(x, false),
        3 => wait_and_replay(x),
        4 => hp_dependent(x),
        5 => flash_animate(x),
        _ => {}
    }
    if v < 12 && x.a5_u8(STAGE) != STAGE_NO_CULL {
        let off = if x.a5_u8(0x36) != 0 { 0x556 } else { 0x506 };
        let edge = x.a5_u16(off).wrapping_sub(0x200);
        if !(s16(edge) < s16(x.u16(0x8))) {
            x.set8(0x4, 6);
        }
    }
    x.reinsert();
}

/// Makes the object visible then hides it every other frame (except if `A5+0x33`).
fn flash<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_bit(0x1, 1, false);
    if x.a5_u8(BLINK_OFF) == 0 && x.a5_u8(BLINK_PHASE).wrapping_add(x.counter() as u8) & 1 != 0 {
        x.set_bit(0x1, 1, true);
    }
}

fn flash_animate<R: Bench>(x: &mut Cx<'_, R>) {
    flash(x);
    x.anim_step();
}

/// A player on the ground, close enough, picks up the object: next `+5`, 30-frame countdown.
fn pickup_touch<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(HIDDEN) != 0 || x.a5_u8(GAME_PAUSED) != 0 {
        return;
    }
    for p in players() {
        if p.u8(x.b, 0) == 0 || p.u16(x.b, 0xc) != 0 {
            continue;
        }
        let dy = x.u16(0x10).wrapping_sub(p.u16(x.b, 0x10)).wrapping_add(6);
        if dy > 0xc {
            continue;
        }
        let dx = x.u16(0x8).wrapping_sub(p.u16(x.b, 0x8)).wrapping_add(8);
        if dx > 0x10 {
            continue;
        }
        x.add8(0x5, 2);
        x.set16(COUNTDOWN, 0x1e);
        return;
    }
}

/// Pickup countdown: blinks every 8 steps; `true` when it has elapsed.
fn pick_countdown<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let t = x.u16(COUNTDOWN).wrapping_sub(1);
    x.set16(COUNTDOWN, t);
    if t == 0 {
        x.add8(0x5, 2);
        x.set8(0x1, 1);
        return true;
    }
    if t & 7 == 0 {
        let flags = x.u8(0x1);
        x.set8(0x1, flags ^ 2);
    }
    false
}

/// Variants 0, 1 (`with_effects`) and 4, 5: wait for the pickup, blink, play the end animation.
fn touch_then_pick<R: Bench>(x: &mut Cx<'_, R>, with_effects: bool) {
    match x.u8(0x5) {
        0 => pickup_touch(x),
        2 => {
            if pick_countdown(x) && with_effects {
                x.anim_start(ANIMS, 3);
                crate::sound::play(x.b, PICKUP_SOUND, x.o);
            }
        }
        4 => {
            if x.anim_step() {
                x.add8(0x4, 2);
            }
        }
        _ => {}
    }
}

/// Variants 6, 7: wait for a delay (outside pause), then replay animation 5 with a new delay.
fn wait_and_replay<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x5) == 0 {
        if x.u8(HIDDEN) != 0 || x.a5_u8(GAME_PAUSED) != 0 {
            return;
        }
        let t = x.u16(COUNTDOWN).wrapping_sub(1);
        x.set16(COUNTDOWN, t);
        if t == 0 {
            x.add8(0x5, 2);
        }
    } else if x.anim_step() {
        x.set8(0x5, 0);
        random_delay(x);
        x.anim_start(ANIMS, 5);
    }
}

/// Variants 8, 9: alive as long as the creator's HP are non-zero; at its death, animation 6, then blinks for 32
/// frames before disappearing.
fn hp_dependent<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x5) {
        0 => {
            let maker = x.linked(LINK);
            if maker.u16(x.b, 0x62) != 0 {
                flash_animate(x);
            } else {
                x.add8(0x5, 2);
                x.anim_start(ANIMS, 6);
            }
        }
        2 => {
            if x.anim_step() {
                x.add8(0x5, 2);
                x.set16(COUNTDOWN, 0x20);
            }
            flash(x);
        }
        4 => {
            x.set_bit(0x1, 1, false);
            let t = x.u16(COUNTDOWN).wrapping_sub(1);
            x.set16(COUNTDOWN, t);
            if t == 0 {
                x.add8(0x4, 2);
            }
            if x.u16(COUNTDOWN) & 1 != 0 {
                x.set_bit(0x1, 1, true);
            }
        }
        _ => {}
    }
}
