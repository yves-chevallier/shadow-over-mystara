use crate::player::Player;
use super::*;

// ---------------------------------------------------------------------------------------------
// Type 13 : radeau
// ---------------------------------------------------------------------------------------------

pub(super) mod raft {
    /// Scroll speed (long, 16.16).
    pub const SPEED: u32 = 0x58;
}

pub(super) fn raft(x: &mut Gx) {
    if !scene_running(x, |x| x.set8(f::STATE, 4)) {
        return;
    }
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set32(raft::SPEED, 0x4_0000);
            x.set16(Obj::Z, 0);
            x.anim_start(ANIM_SET.0, 0);
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
pub(super) fn raft_drift(x: &mut Gx) {
    // Original glitch: without steering (camera fixed), the movement is the D4 left by what precedes (last
    // long of the SET 0 palette animation, raft movement of a previous frame, sprite list...).
    let mut dx = x.leftovers().d4 as i32;
    if vars::CAMERA_MODE.get(x.b) != 4 {
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

pub(super) mod gate {
    /// Oscillation: half-period (`+0x42`), counter (`+0x5e`), active (`+0x5f`).
    pub const PERIOD: u32 = 0x42;
    pub const SWING: u32 = 0x43;
    pub const COUNTER: u32 = 0x5e;
    pub const MOVING: u32 = 0x5f;
    /// Curtain step (word).
    pub const CURTAIN: u32 = 0x5c;
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
    pub const MUSIC_OF_SCENE: crate::sound::Sound = crate::sound::SCENE_MUSIC_OR_STOP;
    pub const PLAY_MUSIC: crate::sound::Sound = crate::sound::FROM_D4;
}

pub(super) fn gate<R: Bench>(x: &mut Cx<'_, R>) {
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

pub(super) fn gate_init<R: Bench>(x: &mut Cx<'_, R>) {
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
        crate::families::creators::sobj_8(x.b, o);
    }
    let mode = x.u8(Effect::SETTING) as u32;
    if mode == 0 {
        let v = x.u8(Effect::OWN_PALETTE) | 1;
        x.set8(Effect::OWN_PALETTE, v);
        x.set8(Effect::PALETTE, 0x19);
    }
    let set = tables().gate_anim_sets.at16(x.b, 4 * mode) as u32;
    let set = set << 16 | tables().gate_anim_sets.at16(x.b, 4 * mode + 2) as u32;
    x.anim_start(set, 0);
    x.reinsert();
}

/// 0xae3bc: oscillation (speeds `+0x14` in x, `+0x18` in depth, inverted every half-period).
pub(super) fn gate_swing<R: Bench>(x: &mut Cx<'_, R>) {
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

pub(super) fn gate_watch<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    gate_swing(x);
    if (crate::ram::CAMERA_X.get(x.b).wrapping_sub(0x40) as i16) > (x.u16(Obj::X) as i16) {
        // left the screen on the left
        return x.set8(f::STATE, 6);
    }
    let leader = Obj(vars::LEADER.get(x.b) as i16 as i32 as u32 & 0xff_ffff);
    let dx = leader.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)).wrapping_add(0x44);
    let dy = leader.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y)).wrapping_sub(9);
    if dx > 0x74 || dy > 0x1c {
        return;
    }
    if !players().any(|p| p.is_active(x.b) && p.u16(x.b, Player::HP) != 0) {
        return;
    }
    // a living player enters: start of the transition
    vars::GATE_ACTIVE.set(x.b, 0);
    wr8(x.b, 0xff2472, 1);
    x.add8(f::SUB_STATE, 2);
    vars::CAMERA_MODE.set(x.b, 4);
    vars::SCRIPTED.set(x.b, 1);
}

/// Substate 2: curtain palette, then two bands of 22 tiles per frame.
pub(super) fn gate_curtain<R: Bench>(x: &mut Cx<'_, R>) {
    for k in 0..8 {
        let v = crate::palette::library_excerpt!("gate_curtain_palette", 0x25c4a0, 0x25c4c0, 4).nth32(x.b, k);
        let v = if k < 6 { v | 0xf000_f000 } else { v.wrapping_add(0xf000_f000) };
        wr32(x.b, 0x91c7e0 + 4 * k, v);
    }
    for _ in 0..2 {
        let step = x.u16(gate::CURTAIN);
        let steps = &tables().gate_curtain_steps;
        let entry = step as i16 as i32 as u32;
        let offset = steps.at16(x.b, entry);
        if (offset as i16) < 0 {
            // end of the curtain: entry into the sub-scene
            return gate_enter(x);
        }
        let mut at = steps.at32(x.b, entry.wrapping_add(2));
        x.add16(gate::CURTAIN, 6);
        let mut src = offset as i16 as i32 as u32;
        for _ in 0..22 {
            let tile = tables().gate_curtain_tiles.at16(x.b, src);
            wr16(x.b, at, tile);
            wr16(x.b, at + 2, 0x1f);
            src += 2;
            at += 4;
        }
    }
}

/// 0xada96: entry into the sub-scene (scene 7 → 6, otherwise 12). The scene context is saved in
/// the object, the objects of the pools are frozen, the variant's scenery is created, then the scene restarts.
pub(super) fn gate_enter<R: Bench>(x: &mut Cx<'_, R>) {
    let scene = if crate::ram::SCENE.get(x.b) == 7 { 6 } else { 0xc };
    crate::ram::SCENE.set(x.b, scene);
    x.set16(f::STATE, 0x400);
    x.set16(gate::CURTAIN, 0);
    vars::INPUTS_LOCKED.set(x.b, 0);
    x.unlink();
    // 0xae298: context save
    let (cam_x, cam_y) = (vars::CAMERA_X_LONG.get(x.b), vars::CAMERA_Y_LONG.get(x.b));
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
    crate::families::scenery::restart_scene(x);
}

/// 0xcc200: scenery (SET 8) of variant `variant` (D0) and setting `setting` (D1); failing that a SOBJ 4, failing that
/// an
/// EFFCT 18.
pub(super) fn scene_prop<R: Bench>(x: &mut Cx<'_, R>, variant: u8, setting: u8) {
    for (alloc, kind) in [(&pools::SET, 8), (&pools::SOBJ, 4), (&pools::EFFCT, 0x12)] {
        if let Some(o) = x.alloc(alloc) {
            o.set_u8(x.b, Obj::ACTIVE, 1);
            o.set_u8(x.b, Obj::KIND, kind);
            o.set_u8(x.b, f::VARIANT, variant);
            o.set_u8(x.b, Effect::SETTING, setting);
            return;
        }
    }
}

/// 0xae11a, substate 0: return from the sub-scene (scene 6 → 7, otherwise 9). The scene restarts with the
/// context saved by `gate_enter`, the living players are repositioned in front of the followed object.
pub(super) fn gate_leave<R: Bench>(x: &mut Cx<'_, R>) {
    let scene = if crate::ram::SCENE.get(x.b) == 6 { 7 } else { 9 };
    crate::ram::SCENE.set(x.b, scene);
    vars::INPUTS_LOCKED.set(x.b, 1);
    crate::families::scenery::resume_scenery_lit(x, crate::families::scenery::Cameras::Start);
    for p in players() {
        if p.u8(x.b, Player::ACTIVE) == 0 || p.u16(x.b, Player::HP) == 0 {
            continue;
        }
        p.set_u8(x.b, Player::ACTIVE, 1);
        p.set_u32(x.b, f::STATE, 0x0200_0000);
        p.set_u8(x.b, Player::SLOW, 0);
        p.set_u8(x.b, Player::SPECIAL_ACTIVE, 0);
        p.set_u8(x.b, Player::INVULN, 0x50);
        // 0xae388: in front of the followed object, offset according to the character
        let leader = Obj(vars::LEADER.get(x.b) as i16 as i32 as u32 & 0xff_ffff);
        let (lx, ly) = (leader.u16(x.b, Obj::X), leader.u16(x.b, Obj::Y));
        p.set_u16(x.b, Obj::X, lx);
        p.set_u16(x.b, Obj::Y, ly);
        p.set_u16(x.b, Obj::Z, 0);
        let k = 2 * p.u8(x.b, Player::INDEX) as u32;
        let dx = tables().gate_return_offsets.at16(x.b, k);
        p.set_u16(x.b, Obj::X, lx.wrapping_add(dx));
        p.set_u16(x.b, Obj::Y, ly.wrapping_sub(0x12));
    }
    vars::CAMERA_MODE.set(x.b, 4);
    let (cam_x, cam_y) = (x.u32(gate::CAMERA_X), x.u32(gate::CAMERA_Y));
    vars::CAMERA_X_LONG.set(x.b, cam_x);
    vars::CAMERA_Y_LONG.set(x.b, cam_y);
    let (alt_x, alt_y) = (x.u16(gate::ALT_X), x.u16(gate::ALT_Y));
    crate::ram::CAMERA_ALT_X.set(x.b, alt_x);
    crate::ram::CAMERA_ALT_Y.set(x.b, alt_y);
    // camera targets: aligned to 32 pixels in x, 0x160 higher
    vars::CAMERA_TARGET_X.set(x.b, cam_x & !0x1f_0000);
    vars::CAMERA_TARGET_Y.set(x.b, cam_y.wrapping_sub(0x160_0000));
    vars::CAMERA_ALT_TARGET_X.set(x.b, alt_x & 0xffe0);
    vars::CAMERA_ALT_TARGET_Y.set(x.b, alt_y.wrapping_sub(0x160));
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
pub(super) fn gate_open<R: Bench>(x: &mut Cx<'_, R>) {
    vars::CAMERA_MODE.set(x.b, 4);
    let step = x.u16(gate::CURTAIN);
    if step as i16 > 0xa {
        vars::GATE_ACTIVE.set(x.b, 1);
        wr8(x.b, 0xff2472, 0);
        vars::SCRIPTED.set(x.b, 0);
        vars::CAMERA_MODE.set(x.b, 0);
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

