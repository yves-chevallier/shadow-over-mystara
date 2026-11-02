//! EFFCT family: scene objects that command the players (type table `0x39ce0`).
//!
//! - 11: effects of an enemy's attacks (sparks, marks, reflections, columns, trails, pieces, sparkles,
//!   sprays); variants 0, 2, 5, 8 and 9, which no scenario exercises, are validated by
//!   `examples/fuzz_effct.rs` ;
//! - 13: raft that scrolls and carries the players away (the players' directions steer it);
//! - 14: passage to a sub-scene, which oscillates, waits for a player, then launches the transition
//!   (tile curtain in background 2): entry into the sub-scene (`0xada96`: scene context
//!   saved in the object, objects frozen), then return (`0xae11a`: context restored, players repositioned).

use crate::families::cx::Bench;
use super::cx::{handlers, Cx, Gx};
use crate::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;
use crate::machine::Machine;

mod f {
    pub const STATE: u32 = 0x04;
    pub const SUB_STATE: u32 = 0x05;
    pub const VARIANT: u32 = 0x03;
}

const ANIM_SET: u32 = 0x1c03a6;
/// Players: four records of 0x200 bytes.
const PLAYERS: u32 = A5 + 0x5de;

fn players() -> impl DoubleEndedIterator<Item = Obj> {
    (0..4).map(|k| Obj(PLAYERS + 0x200 * k))
}

fn free(x: &mut impl Current) {
    x.free(&pools::EFFCT);
}

/// Common preamble: the pause hook (0x29ea, Z false: object suspended), the game pause, and at
/// least one player present in a state ≤ 2. True if the handler continues.
fn scene_running<X: Current>(x: &mut X, suspended: impl FnOnce(&mut X)) -> bool {
    if x.suspended() {
        suspended(x);
        return false;
    }
    let b = x.bus();
    if rd8(b, A5 + 0x3d) != 0 {
        return false;
    }
    players().any(|p| p.is_active(b) && (p.u8(b, f::STATE) as i8) <= 2)
}

// ---------------------------------------------------------------------------------------------
// Type 13 : radeau
// ---------------------------------------------------------------------------------------------

mod raft {
    /// Scroll speed (long, 16.16).
    pub const SPEED: u32 = 0x58;
    /// Camera mode (`A5+0x51e`): 4 = camera fixed, the raft can no longer be steered.
    pub const CAMERA_MODE: u32 = 0x51e;
}

fn raft(x: &mut Gx) {
    if !scene_running(x, |x| x.set8(f::STATE, 4)) {
        return;
    }
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set32(raft::SPEED, 0x4_0000);
            x.set16(Obj::Z, 0);
            x.anim_start(ANIM_SET, 0);
            x.reinsert();
        }
        2 => {
            if x.u8(f::SUB_STATE) == 0 {
                let v = x.u32(raft::SPEED);
                x.add32(Obj::X, v);
                if crate::ram::PHASE.get(x.b) == 0xc {
                    x.add8(f::SUB_STATE, 2);
                }
                x.anim_step();
                return;
            }
            raft_drift(x);
            x.anim_step();
            x.reinsert();
        }
        4 => x.set8(f::STATE, 2),
        _ => free(x),
    }
}

/// The directions of the players present accelerate the raft (up/down: depth ±1 or ±0.19 beyond
/// ±1; right/left: x ±1.19 or ±0.25), then the players are placed on it, within 0x48 in x.
fn raft_drift(x: &mut Gx) {
    // Original glitch: without steering (camera fixed), the movement is the D4 left by what precedes (last
    // long of the SET 0 palette animation, raft movement of a previous frame, sprite list...).
    let mut dx = x.leftovers().d4 as i32;
    if x.a5_u16(raft::CAMERA_MODE) != 4 {
        let (mut dy, mut sx) = (0i32, 0i32);
        for (k, p) in players().enumerate() {
            if !p.is_active(x.b) {
                continue;
            }
            let input = rd8(x.b, A5 + k as u32);
            if input & 8 != 0 {
                dy = dy.wrapping_add(if dy >= 0x1_0000 { 0x3000 } else { 0x1_0000 });
            }
            if input & 4 != 0 {
                dy = dy.wrapping_sub(if dy <= -0x1_0000 { 0x3000 } else { 0x1_0000 });
            }
            if input & 1 != 0 {
                sx = sx.wrapping_add(if sx >= 0x1_3000 { 0x4000 } else { 0x1_3000 });
            }
            if input & 2 != 0 {
                sx = sx.wrapping_sub(if sx <= -0x1_3000 { 0x4000 } else { 0x1_3000 });
            }
        }
        dx = sx.wrapping_add(x.u32(raft::SPEED) as i32);
        x.add32(Obj::X, dx as u32);
        x.add32(Obj::Y, dy as u32);
        x.leftovers().d4 = dx as u32;
        crate::terrain::push(x.b, x.o);
    }
    let (rx, ry) = (x.u16(Obj::X), x.u16(Obj::Y));
    for p in players().rev() {
        if !p.is_active(x.b) {
            continue;
        }
        p.set_u16(x.b, Obj::Y, ry.wrapping_sub(0x12));
        let px = p.u32(x.b, Obj::X).wrapping_add(dx as u32);
        p.set_u32(x.b, Obj::X, px);
        let px = p.u16(x.b, Obj::X) as i16;
        let right = rx.wrapping_add(0x48);
        let left = rx.wrapping_sub(0x48);
        if (right as i16) < px {
            p.set_u16(x.b, Obj::X, right);
        } else if (left as i16) > px {
            p.set_u16(x.b, Obj::X, left);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 14: passage to a sub-scene
// ---------------------------------------------------------------------------------------------

mod gate {
    /// Animation sets per mode `+0x2f` (longs, program space).
    pub const ANIM_SETS: u32 = 0xad954;
    /// Oscillation: half-period (`+0x42`), counter (`+0x5e`), active (`+0x5f`).
    pub const PERIOD: u32 = 0x42;
    pub const SWING: u32 = 0x43;
    pub const COUNTER: u32 = 0x5e;
    pub const MOVING: u32 = 0x5f;
    /// Curtain step (word).
    pub const CURTAIN: u32 = 0x5c;
    /// Curtain: (offset in the tiles, address) per step, 6 bytes (data); tiles (data).
    pub const CURTAIN_STEPS: u32 = 0xadad0;
    pub const CURTAIN_TILES: u32 = 0xadbf2;
    /// Scene context saved in the object: cameras (longs `+0x44`, `+0x48`, words `+0x4c`,
    /// `+0x4e`), scroll state (`+0x54`, `+0x5a`) and music (`+0x16`).
    pub const CAMERA_X: u32 = 0x44;
    pub const CAMERA_Y: u32 = 0x48;
    pub const ALT_X: u32 = 0x4c;
    pub const ALT_Y: u32 = 0x4e;
    pub const SCROLL: u32 = 0x54;
    pub const SCROLL_STEP: u32 = 0x5a;
    pub const MUSIC: u32 = 0x16;
    /// Scene scroll state (`A5-0x55d0` long, `A5-0x55cc` byte) and music (`A5+0x7fc4`).
    pub const SCENE_SCROLL: u32 = 0xff2a30;
    pub const SCENE_SCROLL_STEP: u32 = 0xff2a34;
    pub const SCENE_MUSIC: u32 = 0xffffc4;
    /// Pools whose objects are frozen during the sub-scene (`+0x40` incremented, `+0x41` cleared):
    /// (offset from `A5`, slots, size).
    pub const FROZEN: [(u32, u32, u32); 6] =
        [(0x4a9e, 32, 0x50), (0x549e, 38, 0x60), (0x795e, 16, 0x50), (0x3a1e, 3, 0xc0), (0x3c5e, 38, 0x60), (0x179e, 48, 0x60)];
    /// X offset of the players, per character, on return (words, program space).
    pub const RETURN_OFFSETS: u32 = 0xae3b4;
    pub const MUSIC_OF_SCENE: crate::sound::Sound = crate::sound::SCENE_MUSIC_OR_STOP;
    pub const PLAY_MUSIC: crate::sound::Sound = crate::sound::FROM_D4;
}

fn gate<R: Bench>(x: &mut Cx<'_, R>) {
    if !scene_running(x, |_| {}) {
        return;
    }
    match x.u8(f::STATE) {
        0 => gate_init(x),
        2 => {
            if x.u8(f::SUB_STATE) != 0 {
                return gate_curtain(x);
            }
            gate_watch(x);
        }
        4 => {
            if x.u8(f::SUB_STATE) == 0 {
                return gate_leave(x);
            }
            gate_open(x);
        }
        _ => free(x),
    }
}

fn gate_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let period = x.u8(gate::PERIOD);
    x.set8(gate::MOVING, period);
    x.set8(gate::COUNTER, period >> 1);
    x.set16(Obj::VX, 0);
    x.set16(Obj::VZ, 0);
    // bit 4 de `+0x43` : oscillation en x, sinon en profondeur
    let swing = x.u8(gate::SWING);
    if swing & 0x10 != 0 {
        x.set8(Obj::VX + 1, swing & !0x10);
    } else {
        x.set8(Obj::VZ + 1, swing);
    }
    if x.u8(gate::MOVING) != 0 {
        let o = x.o;
        super::creators::sobj_8(x.b, o);
    }
    let mode = x.u8(0x2f) as u32;
    if mode == 0 {
        let v = x.u8(0x2a) | 1;
        x.set8(0x2a, v);
        x.set8(0x2b, 0x19);
    }
    let set = x.rom16(gate::ANIM_SETS + 4 * mode) as u32;
    let set = set << 16 | x.rom16(gate::ANIM_SETS + 4 * mode + 2) as u32;
    x.anim_start(set, 0);
    x.reinsert();
}

/// 0xae3bc: oscillation (speeds `+0x14` in x, `+0x18` in depth, inverted every half-period).
fn gate_swing<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(gate::MOVING) == 0 {
        return;
    }
    let (vx, vy) = (x.u16(Obj::VX), x.u16(Obj::VZ));
    x.add16(Obj::X, vx);
    x.add16(Obj::Y, vy);
    let n = x.u8(gate::COUNTER).wrapping_sub(1);
    x.set8(gate::COUNTER, n);
    if (n as i8) >= 0 {
        return;
    }
    let period = x.u8(gate::PERIOD);
    x.set8(gate::COUNTER, period);
    x.set16(Obj::VX, vx.wrapping_neg());
    x.set16(Obj::VZ, vy.wrapping_neg());
}

fn gate_watch<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    gate_swing(x);
    if (crate::ram::CAMERA_X.get(x.b).wrapping_sub(0x40) as i16) > (x.u16(Obj::X) as i16) {
        // left the screen on the left
        return x.set8(f::STATE, 6);
    }
    let leader = Obj(x.a5_u16(0x46) as i16 as i32 as u32 & 0xff_ffff);
    let dx = leader.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)).wrapping_add(0x44);
    let dy = leader.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y)).wrapping_sub(9);
    if dx > 0x74 || dy > 0x1c {
        return;
    }
    if !players().any(|p| p.is_active(x.b) && p.u16(x.b, 0x62) != 0) {
        return;
    }
    // a living player enters: start of the transition
    x.set_a5_u8(0x5f, 0);
    wr8(x.b, 0xff2472, 1);
    x.add8(f::SUB_STATE, 2);
    x.set_a5_u16(0x51e, 4);
    x.set_a5_u8(0x4e, 1);
}

/// Substate 2: curtain palette, then two bands of 22 tiles per frame.
fn gate_curtain<R: Bench>(x: &mut Cx<'_, R>) {
    for k in 0..8 {
        let v = x.data32(0x25c4a0 + 4 * k);
        let v = if k < 6 { v | 0xf000_f000 } else { v.wrapping_add(0xf000_f000) };
        wr32(x.b, 0x91c7e0 + 4 * k, v);
    }
    for _ in 0..2 {
        let step = x.u16(gate::CURTAIN);
        let entry = gate::CURTAIN_STEPS.wrapping_add(step as i16 as i32 as u32);
        let offset = x.data16(entry);
        if (offset as i16) < 0 {
            // end of the curtain: entry into the sub-scene
            return gate_enter(x);
        }
        let mut at = x.data32(entry + 2);
        x.add16(gate::CURTAIN, 6);
        let mut src = gate::CURTAIN_TILES.wrapping_add(offset as i16 as i32 as u32);
        for _ in 0..22 {
            let tile = rd16(x.b, src);
            wr16(x.b, at, tile);
            wr16(x.b, at + 2, 0x1f);
            src += 2;
            at += 4;
        }
    }
}

/// 0xada96: entry into the sub-scene (scene 7 → 6, otherwise 12). The scene context is saved in
/// the object, the objects of the pools are frozen, the variant's scenery is created, then the scene restarts.
fn gate_enter<R: Bench>(x: &mut Cx<'_, R>) {
    let scene = if crate::ram::SCENE.get(x.b) == 7 { 6 } else { 0xc };
    x.set_a5_u8(0x34, scene);
    x.set16(f::STATE, 0x400);
    x.set16(gate::CURTAIN, 0);
    x.set_a5_u8(0x44, 0);
    x.unlink();
    // 0xae298: context save
    let (cam_x, cam_y) = (x.a5_u32(0x506), x.a5_u32(0x50a));
    x.set32(gate::CAMERA_X, cam_x);
    x.set32(gate::CAMERA_Y, cam_y);
    let (alt_x, alt_y) = (crate::ram::CAMERA_ALT_X.get(x.b), crate::ram::CAMERA_ALT_Y.get(x.b));
    x.set16(gate::ALT_X, alt_x);
    x.set16(gate::ALT_Y, alt_y);
    let scroll = crate::mem::rd32(x.b, gate::SCENE_SCROLL);
    x.set32(gate::SCROLL, scroll);
    let step = rd8(x.b, gate::SCENE_SCROLL_STEP);
    x.set8(gate::SCROLL_STEP, step);
    let music = rd16(x.b, gate::SCENE_MUSIC);
    x.set16(gate::MUSIC, music);
    for (base, slots, size) in gate::FROZEN {
        for k in 0..slots {
            let o = A5 + base + size * k;
            if rd8(x.b, o) != 0 {
                let frozen = rd8(x.b, o + 0x40).wrapping_add(1);
                wr8(x.b, o + 0x40, frozen);
                wr8(x.b, o + 0x41, 0);
            }
        }
    }
    // 0xae37c: variant's scenery (D0 = 0, D1 = variant)
    let v = x.u8(f::VARIANT);
    scene_prop(x, 0, v);
    crate::sound::play(x.b, gate::MUSIC_OF_SCENE, x.o);
    // 0xcc0ca
    super::scenery::restart_scene(x);
}

/// 0xcc200: scenery (SET 8) of variant `variant` (D0) and setting `setting` (D1); failing that a SOBJ 4, failing that
/// an
/// EFFCT 18.
fn scene_prop<R: Bench>(x: &mut Cx<'_, R>, variant: u8, setting: u8) {
    for (alloc, kind) in [(&pools::SET, 8), (&pools::SOBJ, 4), (&pools::EFFCT, 0x12)] {
        if let Some(o) = x.alloc(alloc) {
            o.set_u8(x.b, 0, 1);
            o.set_u8(x.b, 2, kind);
            o.set_u8(x.b, f::VARIANT, variant);
            o.set_u8(x.b, 0x2f, setting);
            return;
        }
    }
}

/// 0xae11a, substate 0: return from the sub-scene (scene 6 → 7, otherwise 9). The scene restarts with the
/// context saved by `gate_enter`, the living players are repositioned in front of the followed object.
fn gate_leave<R: Bench>(x: &mut Cx<'_, R>) {
    let scene = if crate::ram::SCENE.get(x.b) == 6 { 7 } else { 9 };
    x.set_a5_u8(0x34, scene);
    x.set_a5_u8(0x44, 1);
    super::scenery::resume_scenery_lit(x, super::scenery::Cameras::Start);
    for p in players() {
        if p.u8(x.b, 0) == 0 || p.u16(x.b, 0x62) == 0 {
            continue;
        }
        p.set_u8(x.b, 0, 1);
        p.set_u32(x.b, f::STATE, 0x0200_0000);
        p.set_u8(x.b, 0x66, 0);
        p.set_u8(x.b, 0x15c, 0);
        p.set_u8(x.b, 0x7e, 0x50);
        // 0xae388: in front of the followed object, offset according to the character
        let leader = Obj(x.a5_u16(0x46) as i16 as i32 as u32 & 0xff_ffff);
        let (lx, ly) = (leader.u16(x.b, Obj::X), leader.u16(x.b, Obj::Y));
        p.set_u16(x.b, Obj::X, lx);
        p.set_u16(x.b, Obj::Y, ly);
        p.set_u16(x.b, Obj::Z, 0);
        let k = 2 * p.u8(x.b, 2) as u32;
        let dx = x.rom16(gate::RETURN_OFFSETS + k);
        p.set_u16(x.b, Obj::X, lx.wrapping_add(dx));
        p.set_u16(x.b, Obj::Y, ly.wrapping_sub(0x12));
    }
    x.set_a5_u16(0x51e, 4);
    let (cam_x, cam_y) = (x.u32(gate::CAMERA_X), x.u32(gate::CAMERA_Y));
    x.set_a5_u32(0x506, cam_x);
    x.set_a5_u32(0x50a, cam_y);
    let (alt_x, alt_y) = (x.u16(gate::ALT_X), x.u16(gate::ALT_Y));
    x.set_a5_u16(0x556, alt_x);
    x.set_a5_u16(0x55a, alt_y);
    // camera targets: aligned to 32 pixels in x, 0x160 higher
    x.set_a5_u32(0x50e, cam_x & !0x1f_0000);
    x.set_a5_u32(0x512, cam_y.wrapping_sub(0x160_0000));
    x.set_a5_u16(0x55e, alt_x & 0xffe0);
    x.set_a5_u16(0x562, alt_y.wrapping_sub(0x160));
    let scroll = x.u32(gate::SCROLL);
    crate::mem::wr32(x.b, gate::SCENE_SCROLL, scroll);
    let step = x.u8(gate::SCROLL_STEP);
    wr8(x.b, gate::SCENE_SCROLL_STEP, step);
    let music = x.u16(gate::MUSIC);
    // Original glitch: the music stays in D4.w (`dispatch::Leftovers`)
    *x.c.d4() = *x.c.d4() & 0xffff_0000 | music as u32;
    x.add8(f::SUB_STATE, 2);
    crate::sound::play_with(x.b, gate::PLAY_MUSIC, crate::sound::Source { object: Some(x.o), d4: Some(music as u32), ..Default::default() });
}

/// State 4, substate 2: the curtain opens (two columns cleared per frame from the center).
fn gate_open<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_a5_u16(0x51e, 4);
    let step = x.u16(gate::CURTAIN);
    if step as i16 > 0xa {
        x.set_a5_u8(0x5f, 1);
        wr8(x.b, 0xff2472, 0);
        x.set_a5_u8(0x4e, 0);
        x.set_a5_u16(0x51e, 0);
        return x.set8(f::STATE, 6);
    }
    let shift = (step.wrapping_mul(4)) as i16 as i32 as u32;
    for base in [0x91203cu32.wrapping_sub(shift), 0x912040u32.wrapping_add(shift)] {
        for row in 0..48 {
            wr32(x.b, base + 0x80 * row, 0x10_0000);
        }
    }
    x.add16(gate::CURTAIN, 1);
}

// ---------------------------------------------------------------------------------------------
// Type 11: effects attached to an enemy's attacks (ten variants)
// ---------------------------------------------------------------------------------------------

mod attack_fx {
    /// Variant 0: spots around the attacker (offset word per attacker type, then eight (dx, dz)).
    pub const FLASH_PLACES: u32 = 0xabfbe;
    pub const SOUND_FLASH: crate::sound::Sound = crate::sound::CMD_18F;
    /// Variant 2: offsets (dx, dz) and animation per attacker type (program space).
    pub const MARK_PLACES: u32 = 0xac6ea;
    pub const MARK_ANIMS: u32 = 0xac6ca;
    /// Attacker's action (`+0x49`), noted by variant 5 in `+0x42`.
    pub const ACTION: u32 = 0x49;
    pub const NOTED_ACTION: u32 = 0x42;
    /// Variants 8 and 9: spray sheet per attacker type (byte, program space: 0 = none),
    /// sheets (data: offsets, then count minus one and (vx, vz, setting, padding) per spark),
    /// spark animation per setting.
    pub const SHOWER_SHEETS: u32 = 0xad1b4;
    pub const SHOWER_DATA: u32 = 0xad232;
    pub const SHOWER_ANIMS: u32 = 0xad354;
    pub const ANIM_A: u32 = 0x18f412;
    pub const ANIM_B: u32 = 0x15cdb6;
    pub const OWNER: u32 = 0x5c;
    pub const SOURCE: u32 = 0x54;
}

/// 0xabec8: creates an EFFCT 11 of variant `variant` and mode `mode`, attached to the current object.
fn attack_fx_spawn<R: Bench>(x: &mut Cx<'_, R>, variant: u8, mode: u8) -> Option<Obj> {
    let o = x.alloc(&pools::EFFCT)?;
    wr8(x.b, o.0, 1);
    wr8(x.b, o.0 + 2, 11);
    wr16(x.b, o.0 + attack_fx::OWNER, x.o.0 as u16);
    wr8(x.b, o.0 + 3, variant);
    wr8(x.b, o.0 + 0x2f, mode);
    Some(o)
}

/// Index of the attacker in the tables: its type, + 0x20 if its class (`+0x28`) equals `class`
/// (`when_equal`) or does not.
fn attacker_index<R: Bench>(x: &mut Cx<'_, R>, a: Obj, class: u8, when_equal: bool) -> u32 {
    let shifted = (a.u8(x.b, 0x28) == class) == when_equal;
    let base: u8 = if shifted { 0x20 } else { 0 };
    base.wrapping_add(a.u8(x.b, 2)) as u32
}

fn attack_fx<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::VARIANT) {
        0 => attack_fx_flash(x),
        1 => attack_fx_burst(x),
        2 => attack_fx_mark(x),
        3 => attack_fx_glint(x),
        4 => attack_fx_rise(x),
        5 => attack_fx_trail(x),
        6 => attack_fx_parts(x),
        7 => attack_fx_sparks(x),
        8 | 9 => attack_fx_shower(x),
        _ => {}
    }
}

/// End of animation: removal (state 6), otherwise re-listing.
fn attack_fx_anim_or_leave<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        return x.set32(f::STATE, 0x0600_0000);
    }
    x.reinsert();
}

/// Variant 0: spark placed at random around the attacker (eight spots per attacker type, flipped every other
/// frame), random animation, sound; disappears at the end of the animation.
fn attack_fx_flash<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let owner = x.linked(attack_fx::OWNER);
            for off in [Obj::X, Obj::Z, Obj::Y] {
                let v = owner.u32(x.b, off);
                x.set32(off, v);
            }
            x.add32(Obj::Y, 0xffff_ffff);
            let k = attacker_index(x, owner, 2, false);
            let offset = x.data16(attack_fx::FLASH_PLACES + 2 * k) as i16 as i32 as u32;
            let places = attack_fx::FLASH_PLACES.wrapping_add(offset);
            let rng = x.random();
            let pick = 4 * (rng as u32 & 7);
            let (mut dx, dz) = (x.data16(places + pick), x.data16(places + pick + 2));
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            if facing != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(Obj::X, dx);
            x.add16(Obj::Z, dz);
            if x.a5_u8(0x6f) & 1 != 0 {
                x.set8(0x2c, facing ^ 0x20);
            }
            let rng = x.random();
            let anim = rng as u32 & 3;
            x.anim_start(attack_fx::ANIM_A, anim as u16);
            crate::sound::play(x.b, attack_fx::SOUND_FLASH, x.o);
        }
        2 => attack_fx_anim_or_leave(x),
        _ => free(x),
    }
}

/// Variant 2: mark placed on the attacker (offset and animation per type); at the end of its animation,
/// it advances the attacker (`+6` += 2) and disappears.
fn attack_fx_mark<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let owner = x.linked(attack_fx::OWNER);
            for off in [Obj::X, Obj::Z, Obj::Y] {
                let v = owner.u32(x.b, off);
                x.set32(off, v);
            }
            x.add32(Obj::Y, 0xffff_ffff);
            let kind = owner.u8(x.b, Obj::KIND) as u32;
            let mut dx = x.rom16(attack_fx::MARK_PLACES + 4 * kind);
            let dz = x.rom16(attack_fx::MARK_PLACES + 4 * kind + 2);
            x.add16(Obj::Z, dz);
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            if facing != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(Obj::X, dx);
            let anim = x.rom8(attack_fx::MARK_ANIMS + kind);
            x.anim_start(attack_fx::ANIM_A, (anim as u32) as u16);
            x.reinsert();
        }
        2 => {
            if !x.anim_step() {
                return;
            }
            let owner = x.linked(attack_fx::OWNER);
            let step = owner.u8(x.b, 6).wrapping_add(2);
            owner.set_u8(x.b, 6, step);
            x.set32(f::STATE, 0x0600_0000);
        }
        _ => free(x),
    }
}

/// Variant 5: like the spark (1), but it follows the attacker only as long as the attacker keeps the action noted at
/// birth (`+0x49` → `+0x42`) and has not passed step 4.
fn attack_fx_trail<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            let owner = x.linked(attack_fx::OWNER);
            let action = owner.u8(x.b, attack_fx::ACTION);
            x.set8(attack_fx::NOTED_ACTION, action);
            burst_init(x);
        }
        2 => {
            let owner = x.linked(attack_fx::OWNER);
            if owner.u8(x.b, attack_fx::ACTION) != x.u8(attack_fx::NOTED_ACTION) || owner.u8(x.b, 6) as i8 >= 4 {
                return free(x);
            }
            for off in [Obj::X, Obj::Z] {
                let v = owner.u32(x.b, off);
                x.set32(off, v);
            }
            let (vx, vz) = (x.u16(Obj::VX), x.u16(Obj::VZ));
            x.add16(Obj::X, vx);
            x.add16(Obj::Z, vz);
            x.anim_step();
            x.reinsert();
        }
        _ => free(x),
    }
}

/// Variants 8 and 9: spray of sparks from the attacker. The first (setting 0) creates the sparks of a sheet
/// specific to the attacker's type (speeds, setting) then disappears; each spark follows the attacker as long as
/// it is in the expected action, then detaches, falls back blinking and disappears on the ground.
fn attack_fx_shower<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x2f) == 0 {
        return attack_fx_shower_spawn(x);
    }
    match x.u8(f::STATE) {
        0 => {
            let mode = x.u8(0x2f).wrapping_sub(1) as u32;
            let anim = x.rom8(attack_fx::SHOWER_ANIMS + mode);
            if x.u8(f::VARIANT) != 8 {
                x.set16(0x2a, 0x110);
            }
            x.add8(f::STATE, 2);
            x.anim_start(attack_fx::ANIM_A, (anim as u32) as u16);
            attack_fx_shard(x);
        }
        2 => attack_fx_shard(x),
        _ => free(x),
    }
}

fn attack_fx_shower_spawn<R: Bench>(x: &mut Cx<'_, R>) {
    let owner = x.linked(attack_fx::OWNER);
    let k = attacker_index(x, owner, 4, true);
    let sheet = x.rom8(attack_fx::SHOWER_SHEETS + k) as u16;
    if sheet != 0 {
        let offset = x.data16(attack_fx::SHOWER_DATA + sheet.wrapping_sub(2) as i16 as i32 as u32) as i16 as i32 as u32;
        let mut at = attack_fx::SHOWER_DATA.wrapping_add(offset);
        let mut n = x.data16(at);
        at += 2;
        loop {
            let variant = x.u8(f::VARIANT);
            let me = x.o;
            let created = super::effects_creators::attack(x.b, me, variant, 0);
            let Some(o) = created else { break };
            let (vx, vz) = (x.data16(at), x.data16(at + 2));
            o.set_u16(x.b, Obj::VX, vx);
            o.set_u16(x.b, Obj::VZ, vz);
            let mode = x.data8(at + 4);
            o.set_u8(x.b, 0x2f, mode);
            at += 6;
            let link = x.u16(attack_fx::OWNER);
            o.set_u16(x.b, attack_fx::OWNER, link);
            n = n.wrapping_sub(1);
            if n == 0xffff {
                break;
            }
        }
    }
    free(x);
}

/// A spark of the spray (`0xad384`).
fn attack_fx_shard<R: Bench>(x: &mut Cx<'_, R>) {
    let owner = x.linked(attack_fx::OWNER);
    if x.u8(f::SUB_STATE) == 0 {
        let first = x.u8(f::VARIANT) == 8;
        let (a, b) = if first { (0x70, 0x90) } else { (0x94, 0x94) };
        let action = owner.u8(x.b, attack_fx::ACTION);
        let follows = if action == a || action == b || action != 0xa8 {
            owner.u8(x.b, f::STATE) == 4 && (owner.u8(x.b, 6) as i8) < 6
        } else {
            // action 0xa8: the spark follows the attacker during its steps 4 to 6, otherwise disappears
            if owner.u8(x.b, 6).wrapping_sub(4) > 2 {
                return free(x);
            }
            true
        };
        if follows {
            let (px, pz, py) = (owner.u16(x.b, Obj::X), owner.u16(x.b, Obj::Z), owner.u32(x.b, Obj::Y));
            x.set16(Obj::X, px);
            x.set16(Obj::Z, pz);
            x.set32(Obj::Y, py.wrapping_sub(1));
            let vz = x.u16(Obj::VZ);
            x.add16(Obj::Z, vz);
            let mut vx = x.u16(Obj::VX);
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            if facing != 0 {
                vx = vx.wrapping_neg();
            }
            x.add16(Obj::X, vx);
            return x.reinsert();
        }
        // detached: it falls back, pushed outward
        x.add8(f::SUB_STATE, 2);
        x.set16(Obj::VZ, 0x200);
        x.set16(Obj::AZ, 0xffc0);
        x.set16(Obj::AX, 0);
        let mut vx: u16 = 0x180;
        if (x.u16(Obj::VX) as i16) < 0 {
            vx = vx.wrapping_neg();
        }
        if x.u8(0x2c) != 0 {
            vx = vx.wrapping_neg();
        }
        x.set16(Obj::VX, vx);
        if x.counter() & 1 != 0 {
            let v = x.u8(0x01) ^ 2;
            x.set8(0x01, v);
        }
    }
    let v = x.u8(0x01) ^ 2;
    x.set8(0x01, v);
    x.move_xz();
    if x.u16(Obj::Z) as i16 <= 0 {
        free(x);
    }
}

/// Variant 1: spark thrown from the attacker, which follows it then disappears.
fn attack_fx_burst<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => burst_init(x),
        2 => {
            let owner = x.linked(attack_fx::OWNER);
            for off in [Obj::X, Obj::Z] {
                let v = owner.u32(x.b, off);
                x.set32(off, v);
            }
            let (vx, vz) = (x.u16(Obj::VX), x.u16(Obj::VZ));
            x.add16(Obj::X, vx);
            x.add16(Obj::Z, vz);
            // the pause (`A5+0x3d`) keeps it alive until the end of the animation, otherwise it disappears
            if x.a5_u8(0x3d) == 0 || x.anim_step() {
                x.set32(f::STATE, 0x0600_0000);
            }
        }
        _ => free(x),
    }
}

/// Birth of the spark (variants 1 and 5): placed on the attacker, speed and animation according to its type.
fn burst_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let owner = x.linked(attack_fx::OWNER);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = owner.u32(x.b, off);
        x.set32(off, v);
    }
    let facing = owner.u8(x.b, 0x2c);
    x.set8(0x2c, facing);
    let (anim, table, k) = match x.u8(0x2f) {
        0 => {
            x.add32(Obj::Y, 0xffff_ffff);
            let k = attacker_index(x, owner, 4, true);
            (x.rom8(0xac38c + k) as u32, 0xac3cc, k)
        }
        1 => {
            x.add32(Obj::Y, 1);
            let base: u8 = if x.u8(0x28) == 4 { 0x20 } else { 0 };
            (4, 0xac526, base.wrapping_add(owner.u8(x.b, 2)) as u32)
        }
        _ => return,
    };
    let (mut vx, vz) = (x.data16(table + 4 * k), x.data16(table + 4 * k + 2));
    if x.u8(0x2c) != 0 {
        vx = vx.wrapping_neg();
    }
    x.set16(Obj::VX, vx);
    x.set16(Obj::VZ, vz);
    x.add16(Obj::X, vx);
    x.add16(Obj::Z, vz);
    x.anim_start(attack_fx::ANIM_A, anim as u16);
    x.reinsert();
}

/// Variant 3: reflection placed on the attacker as long as it is in action 0x68 or 0x6c.
fn attack_fx_glint<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let owner = x.linked(attack_fx::OWNER);
            for off in [Obj::X, Obj::Z] {
                let v = owner.u16(x.b, off);
                x.set16(off, v);
            }
            let depth = owner.u32(x.b, Obj::Y).wrapping_sub(1);
            x.set32(Obj::Y, depth);
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            let k = 4 * attacker_index(x, owner, 2, false) + 2 * x.u8(0x2f) as u32;
            let mut dx = x.rom8(0xac80a + k) as i8 as u16;
            let dz = x.rom8(0xac80b + k) as i8 as u16;
            x.add16(Obj::Z, dz);
            if x.u8(0x2c) != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(Obj::X, dx);
            x.anim_start(attack_fx::ANIM_A, 0xb);
            x.reinsert();
        }
        2 => {
            x.anim_step();
            let owner = x.linked(attack_fx::OWNER);
            if owner.u16(x.b, f::STATE) == 0x400 && matches!(owner.u8(x.b, 0x49), 0x68 | 0x6c) {
                return x.reinsert();
            }
            x.add8(f::STATE, 2);
        }
        _ => free(x),
    }
}

/// Variant 4: column that rises from the bottom of the screen to a height depending on the attacker.
fn attack_fx_rise<R: Bench>(x: &mut Cx<'_, R>) {
    let owner = x.linked(attack_fx::OWNER);
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            for off in [Obj::X, Obj::Y] {
                let v = owner.u32(x.b, off);
                x.set32(off, v);
            }
            let d: u32 = if x.u8(0x2f) & 1 != 0 { 1 } else { 0xffff_ffff };
            x.add32(Obj::Y, d);
            let top = if crate::ram::CAMERA_ALT_ACTIVE.get(x.b) != 0 { crate::ram::CAMERA_ALT_Y.get(x.b) } else { crate::ram::CAMERA_Y.get(x.b) };
            let z = top.wrapping_add(0x100).wrapping_sub(x.u16(Obj::Y));
            x.set16(Obj::Z, z);
            let mode = x.u8(0x2f) as u32;
            let anim = x.rom8(0xac9f0 + mode) as u32;
            x.anim_start(attack_fx::ANIM_A, anim as u16);
            x.reinsert();
        }
        2 => {
            if x.u8(f::SUB_STATE) == 0 {
                x.add16(Obj::Z, 4u16.wrapping_neg());
                let k = (x.u8(0x2f) & !1) as u32 + ((((owner.u8(x.b, 2) & 3) >> 1) as u32) << 3);
                let limit = x.rom16(0xaca5e + k);
                if (limit as i16) >= (x.u16(Obj::Z) as i16) {
                    x.add8(f::SUB_STATE, 2);
                    x.set16(Obj::Z, limit);
                    if x.u8(0x2f) == 0 {
                        owner.set_u8(x.b, 6, 6);
                    }
                }
            }
            if owner.u16(x.b, f::STATE) == 0x400 && owner.u8(x.b, 0x49) == 0x9c {
                x.anim_step();
            } else {
                x.add8(f::STATE, 2);
            }
        }
        _ => free(x),
    }
}

/// Variant 6: pieces of a broken object; mode 0 creates modes 2 and 3 (and becomes mode 1).
fn attack_fx_parts<R: Bench>(x: &mut Cx<'_, R>) {
    let mode = x.u8(0x2f);
    if mode == 0 {
        let source = x.linked(attack_fx::SOURCE);
        let k = attacker_index(x, source, 4, true);
        let src = x.u16(attack_fx::SOURCE);
        if x.rom8(0xacafe + k) != 0 {
            if let Some(o) = attack_fx_spawn(x, 6, 2) {
                wr16(x.b, o.0 + attack_fx::SOURCE, src);
            }
        }
        if let Some(o) = attack_fx_spawn(x, 6, 3) {
            wr16(x.b, o.0 + attack_fx::SOURCE, src);
        }
        return x.set8(0x2f, 1);
    }
    if mode > 3 {
        return;
    }
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let source = x.linked(attack_fx::SOURCE);
            let k = attacker_index(x, source, 4, true);
            match mode {
                1 | 2 => {
                    let table = if mode == 1 { 0xacc06 } else { 0xacd8a };
                    let mut dx = x.rom16(table + 4 * k);
                    if dx == 0 && mode == 2 {
                        return x.set32(f::STATE, 0x0600_0000);
                    }
                    if dx != 0 {
                        let dz = x.rom16(table + 4 * k + 2);
                        let (sx, sz) = (source.u16(x.b, Obj::X), source.u16(x.b, Obj::Z));
                        x.set16(Obj::X, sx);
                        x.set16(Obj::Z, sz);
                        if source.u8(x.b, 0x2c) != 0 {
                            dx = dx.wrapping_neg();
                        }
                        x.add16(Obj::X, dx);
                        x.add16(Obj::Z, dz);
                    }
                    let d: u32 = if mode == 1 { 0xffff_ffff } else { 1 };
                    let depth = source.u32(x.b, Obj::Y).wrapping_add(d);
                    x.set32(Obj::Y, depth);
                    x.anim_start(attack_fx::ANIM_B, (4 + mode as u32) as u16);
                }
                _ => {
                    let depth = source.u32(x.b, Obj::Y).wrapping_add(2);
                    x.set32(Obj::Y, depth);
                    let mut dx = x.data16(0xacf02 + 4 * k);
                    let dz = x.data16(0xacf02 + 4 * k + 2);
                    let anchor = if dz != 0 { source } else { x.linked(attack_fx::OWNER) };
                    for off in [Obj::X, Obj::Z] {
                        let v = anchor.u32(x.b, off);
                        x.set32(off, v);
                    }
                    let facing = anchor.u8(x.b, 0x2c);
                    x.set8(0x2c, facing);
                    if facing != 0 {
                        dx = dx.wrapping_neg();
                    }
                    x.add16(Obj::X, dx);
                    x.add16(Obj::Z, dz);
                    x.anim_start(attack_fx::ANIM_B, 7);
                }
            }
            x.reinsert();
        }
        2 => {
            if x.anim_step() {
                x.add8(f::STATE, 2);
            }
        }
        _ => free(x),
    }
}

/// Variant 7: mode 0 creates a spray of sparkles (modes 1 and 2, list depending on the attacker) then
/// disappears; a sparkle follows the attacker as long as it strikes.
fn attack_fx_sparks<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x2f) {
        0 => {
            if x.u8(f::STATE) == 0 {
                return x.add8(f::STATE, 2);
            }
            let owner = x.linked(attack_fx::OWNER);
            let k = attacker_index(x, owner, 4, true);
            let list = x.rom8(0xad034 + k);
            if list != 0 {
                let entry = 0xad0a8u32.wrapping_add((list as u16).wrapping_sub(2) as i16 as i32 as u32);
                let mut p = 0xad0a8u32.wrapping_add(x.data16(entry) as i16 as i32 as u32);
                let count = x.data16(p);
                p += 2;
                for _ in 0..=count {
                    // temporary mode: D1.b of the original (word of the mode table), replaced immediately
                    let Some(o) = attack_fx_spawn(x, 7, 6) else { break };
                    let (vx, vz, m) = (x.data16(p), x.data16(p + 2), x.data8(p + 4));
                    wr16(x.b, o.0 + Obj::VX, vx);
                    wr16(x.b, o.0 + Obj::VZ, vz);
                    wr8(x.b, o.0 + 0x2f, m);
                    p += 6;
                    let owner = x.u16(attack_fx::OWNER);
                    wr16(x.b, o.0 + attack_fx::OWNER, owner);
                }
            }
            free(x);
        }
        m @ (1 | 2) => {
            if x.u8(f::STATE) == 0 {
                x.add8(f::STATE, 2);
                x.anim_start(attack_fx::ANIM_A, (0x15 + m as u32) as u16);
                return;
            }
            let owner = x.linked(attack_fx::OWNER);
            let action = owner.u8(x.b, 0x49);
            let striking = matches!(action, 0x60 | 0x88)
                || (action == 0xa8 && owner.u8(x.b, 6).wrapping_sub(8) <= 2);
            if !striking || owner.u8(x.b, f::STATE) != 4 || (owner.u16(x.b, Obj::Z) as i16) <= 0 {
                return free(x);
            }
            for off in [Obj::X, Obj::Z] {
                let v = owner.u16(x.b, off);
                x.set16(off, v);
            }
            let depth = owner.u32(x.b, Obj::Y).wrapping_sub(1);
            x.set32(Obj::Y, depth);
            let vz = x.u16(Obj::VZ);
            x.add16(Obj::Z, vz);
            let mut vx = x.u16(Obj::VX);
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            if facing != 0 {
                vx = vx.wrapping_neg();
            }
            x.add16(Obj::X, vx);
            x.anim_step();
            x.reinsert();
        }
        _ => {}
    }
}

handlers! {
    0xabeea "effct_attack" attack_fx,
    0xad63e "effct_radeau" raft,
    0xad866 "effct_passage" gate,
}
