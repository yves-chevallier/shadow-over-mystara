//! Elements shared by the handlers of the BOSS class (pool FFBA1E, 3 × 0xC0 bytes, table 0x39b12).
//! Voir `docs/families/boss.md`.
//!
//! A boss has the same frame as an enemy (`enemy_common`): `+4` main state (0 creation,
//! 2 active, 4 hit, 6 release), `+5`, `+6`, `+7` substates (even bytes that index jump
//! tables). So the fields, states and loops of `enemy_common` are reused; this module adds the
//! game variables specific to boss scenes, the 68000 flags for comparisons that depend on
//! overflow (`Fl`) and the helpers of the AI library (0x103d6..0x105b6, still in 68000).

use crate::families::cx::Bench;

pub use super::cx::Cx;
pub use super::enemy_common::*;
use crate::mem::{prog16, prog32, A5};
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;

/// The object followed by the camera, if there is one.
pub fn leader<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
    (var::LEADER.get(x.b) != 0).then(|| Obj(var::LEADER.get(x.b) as i16 as i32 as u32 & 0xff_ffff))
}

/// Camera abscissa.
pub fn scroll_x<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    var::SCROLL_X.get(x.b)
}

/// Abscissa of the camera in use (1, or 2 if `A5+0x36`).
pub fn camera_x<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    let v = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA2_X } else { var::CAMERA1_X };
    v.get(x.b)
}

// --- 68000 flags -------------------------------------------------------------------------

/// N Z V C flags of a subtraction or comparison `dst - src`, for branches whose
/// outcome depends on overflow (`ble`, `bgt`, `blt`, `bge`): the old extreme value (0x80, 0x8000)
/// does not behave like a "negative result".
#[derive(Clone, Copy, Debug)]
pub struct Fl {
    pub z: bool,
    pub n: bool,
    pub v: bool,
    pub c: bool,
}

impl Fl {
    pub fn sub8(dst: u8, src: u8) -> Fl {
        let r = dst.wrapping_sub(src);
        Fl { z: r == 0, n: r & 0x80 != 0, v: (dst ^ src) & (dst ^ r) & 0x80 != 0, c: dst < src }
    }
    pub fn sub16(dst: u16, src: u16) -> Fl {
        let r = dst.wrapping_sub(src);
        Fl { z: r == 0, n: r & 0x8000 != 0, v: (dst ^ src) & (dst ^ r) & 0x8000 != 0, c: dst < src }
    }
    pub fn sub32(dst: u32, src: u32) -> Fl {
        let r = dst.wrapping_sub(src);
        Fl { z: r == 0, n: r & 0x8000_0000 != 0, v: (dst ^ src) & (dst ^ r) & 0x8000_0000 != 0, c: dst < src }
    }
    /// `tst`: N and Z only.
    pub fn tst8(v: u8) -> Fl {
        Fl { z: v == 0, n: v & 0x80 != 0, v: false, c: false }
    }
    pub fn tst16(v: u16) -> Fl {
        Fl { z: v == 0, n: v & 0x8000 != 0, v: false, c: false }
    }
    pub fn eq(self) -> bool {
        self.z
    }
    pub fn ne(self) -> bool {
        !self.z
    }
    pub fn mi(self) -> bool {
        self.n
    }
    pub fn pl(self) -> bool {
        !self.n
    }
    pub fn lt(self) -> bool {
        self.n != self.v
    }
    pub fn ge(self) -> bool {
        self.n == self.v
    }
    pub fn gt(self) -> bool {
        !self.z && self.n == self.v
    }
    pub fn le(self) -> bool {
        self.z || self.n != self.v
    }
}

/// `addq.b #k,(off,A0)`: adds `k` to the byte. The original's flags are read back by no one (callers
/// test the returned values; only `carry`/`zero` read back those of a routine called by address).
pub fn addq8<R: Bench>(x: &mut Cx<'_, R>, off: u32, k: u8) {
    x.add8(off, k);
}

/// `subq.b #k,(off,A0)`: subtracts `k` from the byte and returns the flags.
pub fn sub8<R: Bench>(x: &mut Cx<'_, R>, off: u32, k: u8) -> Fl {
    let old = x.u8(off);
    x.set8(off, old.wrapping_sub(k));
    Fl::sub8(old, k)
}

/// `subq.w #k,(off,A0)`.
pub fn sub16<R: Bench>(x: &mut Cx<'_, R>, off: u32, k: u16) -> Fl {
    let old = x.u16(off);
    x.set16(off, old.wrapping_sub(k));
    Fl::sub16(old, k)
}

/// `cmpi.b #imm,(off,A0)` : drapeaux de `byte - imm`.
pub fn cmp8<R: Bench>(x: &mut Cx<'_, R>, off: u32, imm: u8) -> Fl {
    Fl::sub8(x.u8(off), imm)
}

/// `cmpi.w #imm,(off,A0)`.
pub fn cmp16<R: Bench>(x: &mut Cx<'_, R>, off: u32, imm: u16) -> Fl {
    Fl::sub16(x.u16(off), imm)
}

/// `tst.b (off,A0)`.
pub fn tst8<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> Fl {
    Fl::tst8(x.u8(off))
}

/// `tst.w (off,A0)`.
pub fn tst16<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> Fl {
    Fl::tst16(x.u16(off))
}

// --- 68000 AI library (0x103d6..) -----------------------------------------------------

/// Picks speed table `n` (`0x1b64`).
pub fn speed<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

// --- helpers shared by the bosses with decision tables ------------------------------------------

/// Target in the box (0xb5e0) then gesture chosen by the AI (0x1047a): `Some(D0)` if the AI chose one.
pub fn ai_box<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16) -> Option<u32> {
    if !lib::target_in_box(x, d0, d1, d2) {
        return None;
    }
    let turn = lib::turn_decision(x);
    (turn != lib::Turn::None).then_some(turn as u32)
}

/// `moveq #k,D0 ; move.b D0,(0xa0,A0) ; move.b D0,(0xa1,A0)`: animation kind `k` (fields `+0xa0` and `+0xa1`).
pub fn anim_kind<R: Bench>(x: &mut Cx<'_, R>, k: u8) {
    x.set8(0xa0, k);
    x.set8(0xa1, k);
}

/// `subq.w #1,(off,A0)`: true when the countdown has elapsed (`ble`).
pub fn expired<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> bool {
    sub16(x, off, 1).le()
}

/// Word of the PC-relative jump table `base` for byte index `i`: address of the chosen routine
/// (`move.w (base,PC,Di),Di ; jmp (base,PC,Di)`).
pub fn jump_target<R: Bench>(x: &mut Cx<'_, R>, base: u32, i: u32) -> u32 {
    base.wrapping_add(x.rom16(base + i) as i16 as u32)
}

/// Ordinate of the camera in use.
pub fn camera_y<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    let v = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA2_Y } else { var::CAMERA1_Y };
    v.get(x.b)
}

/// Address of the "boss defeated" long (type, variant, score) read by the scene end.
pub const A5_DEATH: u32 = A5 - 0x557c;

/// Allocates an object of class `routine` (carry raised = pool full). The handlers that insist
/// (`jsr alloc ; bcs retry`) loop here; the pool only empties with time, the original would stay
/// stuck: we give up after 64 attempts.
pub fn alloc_retry<R: Bench>(x: &mut Cx<'_, R>, pool: &pools::Pool) -> Option<Obj> {
    (0..64).find_map(|_| x.alloc(pool))
}

/// Writes a byte at an absolute address of the work RAM.
pub fn set_abs8<R: Bench>(x: &mut Cx<'_, R>, addr: u32, v: u8) {
    crate::mem::wr8(x.b, addr, v);
}

// --- common subroutines called by the original boss code -------------------------

/// Boss registered as an obstacle for the other objects (word, 0 / positive = none).
const BODY: u32 = A5 - 0x5fb2;
/// Body shapes (`+0xa4` of the boss): relative jump table (1 = 0x818e6, 2 = 0x819e2).
const BODY_SHAPES: u32 = 0x81ad4;

/// 0x81aa4: pushes the object out of the registered boss's body, if that boss is active, of class 3 and has a
/// non-zero shape (the shape, `boss_3::body`, moves the object in X). True if the object was moved.
pub fn push_out_of_boss<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let body = x.data16(BODY);
    if (body as i16) >= 0 {
        return false;
    }
    let boss = Obj(body as i16 as i32 as u32 & 0xff_ffff);
    if boss.u8(x.b, 0) == 0 || boss.u8(x.b, 2) != 3 {
        return false;
    }
    let shape = boss.u8(x.b, 0xa4) as u16;
    if shape == 0 {
        return false;
    }
    let off = prog16(x.b, BODY_SHAPES + 2 * shape as u32);
    let routine = BODY_SHAPES.wrapping_add(off as i16 as u32);
    super::boss_3::body::push(x.b, boss, x.o, routine)
}

/// Vertical oscillation (by `+0x33`): long offsets added to the height `+0xc`, in one direction
/// then the other depending on the countdown `+0xa8` (period 72 frames, reversal at 36).
const BOB_OFFSETS: u32 = 0x833e4;

/// 0x833b6: one step of a boss's vertical oscillation.
pub fn bob<R: Bench>(x: &mut Cx<'_, R>) {
    let i = (x.u8(f::KEY_FRAME) & 0x7f) as u32 * 4;
    let mut d = prog32(x.b, BOB_OFFSETS + i);
    if x.u16(0xa8) as i16 <= 36 {
        d = d.wrapping_neg();
    }
    x.add32(Obj::Z, d);
    // `subq.w #1` then `bgt`: exact signed comparison of the old value (0x8000 also restarts at 72)
    let before = x.u16(0xa8);
    x.set16(0xa8, before.wrapping_sub(1));
    if before as i16 <= 1 {
        x.set16(0xa8, 72);
    }
}

/// 0x87048: depending on the scene difficulty (`A5+0x5a`: 0 nothing, 3 always) and the shape `+0xad`
/// (2 or 3: depending on HP), sets `+0x43` (no threshold) or, if half the HP `+0x62` exceeds 72,
/// keeps that threshold in `+0xb6` and sets `+0x42`.
pub fn hp_threshold<R: Bench>(x: &mut Cx<'_, R>) {
    let level = var::SCENE_LEVEL.get(x.b);
    if level == 0 {
        return;
    }
    let by_hp = level != 3 && matches!(x.u8(0xad), 2 | 3);
    if by_hp {
        if x.u8(0x42) != 0 {
            return;
        }
        let half = x.u16(f::HP) >> 1;
        // the original computes the half in D1.w (output contract)
        if half as i16 > 72 {
            x.set8(0xb9, 0);
            x.set16(0xb6, half);
            x.set8(0x42, 1);
            return;
        }
    }
    x.set8(0x43, 1);
    x.set8(0xb9, 0);
}
