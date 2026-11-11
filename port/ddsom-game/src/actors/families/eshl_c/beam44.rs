use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::vars;

const ANIMS: RomAddr = RomAddr(0x17f002);
const CREATOR: u32 = 0x46;
/// Step count of the beam.
const STEP: u32 = 0x14;
/// Beam counter of the creator.
const BEAM_COUNT: u32 = 0x42;
/// Height above A2 of the aiming position in Z.
const RAISE_Z: u16 = 0x50;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(Obj::VARIANT) {
        0 => variant0(x),
        1 => variant1(x),
        _ => {}
    }
}

/// Living object designated by a word (negative address and non-zero activity byte).
fn live_link<R: Bench>(x: &mut Cx<'_, R>, word: u16) -> Option<Obj> {
    if s16(word) >= 0 {
        return None;
    }
    let o = obj_of(word);
    (o.u8(x.b, Obj::ACTIVE) != 0).then_some(o)
}

/// The beam is finished: next state.
fn end<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set8(Base::SUB_STATE, 0);
}

/// The two ends (creator and creator of the creator).
fn ends<R: Bench>(x: &mut Cx<'_, R>) -> Option<(Obj, Obj)> {
    let w = x.u16(CREATOR);
    let a1 = live_link(x, w)?;
    let w = a1.u16(x.b, CREATOR);
    let a2 = live_link(x, w)?;
    Some((a1, a2))
}

/// The creator counts one more beam.
fn count_beam<R: Bench>(x: &mut Cx<'_, R>, a1: Obj) {
    let n = a1.u8(x.b, BEAM_COUNT).wrapping_add(1);
    a1.set_u8(x.b, BEAM_COUNT, n);
}

fn variant0<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            end(x);
            x.set8(STEP, 0);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_POWER, 0x20);
            let w = x.u16(CREATOR);
            let Some(a1) = live_link(x, w) else { return end(x) };
            count_beam(x, a1);
            ai_anim(x, ANIMS.0, 0xd);
        }
        2 => beam(x, 0xf, false),
        4 => release(x),
        6 => x.free_eshl(),
        _ => {}
    }
}

fn variant1<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            end(x);
            x.set8(STEP, 0);
            let w = x.u16(CREATOR);
            let Some(a1) = live_link(x, w) else { return end(x) };
            count_beam(x, a1);
            let w = a1.u16(x.b, CREATOR);
            let Some(a2) = live_link(x, w) else { return end(x) };
            let (px, py, pz) = (a2.u16(x.b, Obj::X), a2.u16(x.b, Obj::Y), a2.u16(x.b, Obj::Z));
            x.set16(Obj::X, px);
            x.set16(Obj::Y, py);
            x.set16(Obj::Z, pz.wrapping_add(RAISE_Z));
            copy_flip(x, a2);
            x.set8(Base::SETTING, 0);
            let anim = if a1.u8(x.b, Base::SETTING) != 0 { 0x1a } else { 0x18 };
            ai_anim(x, ANIMS.0, anim);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_POWER, 0x20);
        }
        2 => beam(x, 0x1b, true),
        4 => release(x),
        6 => x.free_eshl(),
        _ => {}
    }
}

/// State 2. `hit_anim`: end animation launched on contact; `lingers`: variant 1, where the end of the end animation
/// keeps the beam (and where the running animation ends it).
fn beam<R: Bench>(x: &mut Cx<'_, R>, hit_anim: u32, lingers: bool) {
    if vars::GAME_PAUSED.get(x.b) != 0 {
        return x.reinsert();
    }
    if x.u8(Base::SUB_STATE) != 0 {
        if ai_step(x) == lingers {
            return x.reinsert();
        }
        return end(x);
    }
    if x.u8(Shot::HIT_FLAGS) & 0x8 != 0 {
        x.add8(Base::SUB_STATE, 2);
        return ai_anim(x, ANIMS.0, hit_anim);
    }
    let Some((a1, a2)) = ends(x) else { return end(x) };
    interpolate(x, a1, a2);
    x.add8(STEP, 1);
    if s8(x.u8(STEP)) > 0xf {
        return end(x);
    }
    ai_step(x);
    x.reinsert();
}

/// Position = A2 + t * (A1 - A2) / 16 (word arithmetic), t = `+0x14`; nothing if t = 0. In Z, the difference is
/// measured 0x50 higher.
fn interpolate<R: Bench>(x: &mut Cx<'_, R>, a1: Obj, a2: Obj) {
    let t = x.u8(STEP) as u16;
    if t == 0 {
        return;
    }
    for (off, raise) in [(0x8, 0), (0xc, RAISE_Z), (0x10, 0)] {
        let base = a2.u16(x.b, off);
        let d = s16(a1.u16(x.b, off).wrapping_sub(base).wrapping_sub(raise)) >> 4;
        let pos = (d as u16).wrapping_mul(t).wrapping_add(base).wrapping_add(raise);
        x.set16(off, pos);
    }
}

/// State 4: gives a beam back to its creator.
fn release<R: Bench>(x: &mut Cx<'_, R>) {
    let w = x.u16(CREATOR);
    if let Some(a1) = live_link(x, w) {
        let n = a1.u8(x.b, BEAM_COUNT).wrapping_sub(1);
        a1.set_u8(x.b, BEAM_COUNT, n);
    }
    x.add8(Obj::STATE, 2);
}
