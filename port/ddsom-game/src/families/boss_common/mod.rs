//! Elements shared by the handlers of the BOSS class (pool FFBA1E, 3 × 0xC0 bytes, table 0x39b12).
//! See `docs/familles/boss.md`.
//!
//! A boss has the same frame as an enemy (`enemy_common`): `+4` main state (0 creation,
//! 2 active, 4 hit, 6 release), `+5`, `+6`, `+7` substates (even bytes that index jump
//! tables). So the fields, states and loops of `enemy_common` are reused; this module adds the
//! fields shared by the bosses ([`records`]: `Boss`), the game variables of the boss scenes ([`vars`]), the reads of
//! the boss tables of `data/enemies.json` (`decide`, `byte`, `word`, `long`, `per_rank`), the 68000 flags for
//! comparisons that depend on overflow (`Fl`) and helpers of the AI library.

use crate::families::cx::Bench;

pub mod records;
pub mod vars;
pub mod warrior;

pub use super::cx::Cx;
pub use super::enemy_common::*;
pub(crate) use super::effects::bad_index;
pub use records::{Boss, Message};
pub use warrior::hp_threshold;
/// Work RAM variables read by the bosses (those of the enemies included).
pub use vars as var;
use crate::mem::A5;
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

/// `moveq #k,D0 ; move.b D0,(0xa0,A0) ; move.b D0,(0xa1,A0)`: animation kind `k` (`Boss::ANIM_KIND` and
/// `Boss::ANIM_LAST` of the template).
pub fn anim_kind<R: Bench>(x: &mut Cx<'_, R>, k: u8) {
    x.set8(Boss::ANIM_KIND, k);
    x.set8(Boss::ANIM_LAST, k);
}

/// `subq.w #1,(off,A0)`: true when the countdown has elapsed (`ble`).
pub fn expired<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> bool {
    sub16(x, off, 1).le()
}

/// Entry at byte offset `i` of the relative jump table `name` of `data/enemies.json` (`move.w (base,PC,Di),Di ; jmp
/// (base,PC,Di)`): the address of the chosen routine, the key of the state stub that the boss runs.
pub fn jump_target<R: Bench>(x: &mut Cx<'_, R>, name: &str, i: u32) -> u32 {
    let t = tables::table(name);
    t.rom.wrapping_add(t.at16(x.b, i) as i16 as u32)
}

/// A decision of the bosses: byte `r` of the pick table `picks`, then the entry it designates in the jump table
/// `jumps` (see [`jump_target`]).
pub fn decide<R: Bench>(x: &mut Cx<'_, R>, picks: &str, r: u32, jumps: &str) -> u32 {
    let i = tables::table(picks).at8(x.b, r) as u32;
    jump_target(x, jumps, i)
}

/// Byte at offset `i` of the table `name`.
pub fn byte<R: Bench>(x: &mut Cx<'_, R>, name: &str, i: u32) -> u8 {
    tables::table(name).at8(x.b, i)
}

/// Word at byte offset `i` of the table `name`.
pub fn word<R: Bench>(x: &mut Cx<'_, R>, name: &str, i: u32) -> u16 {
    tables::table(name).at16(x.b, i)
}

/// Long at byte offset `i` of the table `name`.
pub fn long<R: Bench>(x: &mut Cx<'_, R>, name: &str, i: u32) -> u32 {
    tables::table(name).at32(x.b, i)
}

/// Byte of the table `name` for the difficulty level (`RANK`, one byte per level).
pub fn per_rank<R: Bench>(x: &mut Cx<'_, R>, name: &str) -> u8 {
    let level = var::RANK.get(x.b) as u32;
    byte(x, name, level)
}

/// Ordinate of the camera in use.
pub fn camera_y<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    let v = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA2_Y } else { var::CAMERA1_Y };
    v.get(x.b)
}

/// Address of the "boss defeated" record (`var::DEFEAT_NAME`...), for the bosses of L9b that still write it by
/// address.
pub const A5_DEATH: u32 = A5 - 0x557c;

/// The "boss defeated" record of the stage tally (`var::DEFEAT_*`): the banner name `name` (the type of the boss),
/// 0, then the index of the bonus (`Enemy::EVENT_KIND` of the boss).
pub fn record_defeat<R: Bench>(x: &mut Cx<'_, R>, name: u8) {
    var::DEFEAT_NAME_WORD.set(x.b, (name as u16) << 8);
    let bonus = x.u8(Enemy::EVENT_KIND);
    var::DEFEAT_BONUS.set(x.b, bonus);
}

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


/// 0x81aa4: pushes the object out of the registered boss's body, if that boss is active, of type 3 and has a
/// non-zero shape (the shape, `boss_3::body`, moves the object in X). True if the object was moved.
pub fn push_out_of_boss<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let body = var::BODY_OWNER.get(x.b);
    if (body as i16) >= 0 {
        return false;
    }
    let boss = Obj(body as i16 as i32 as u32 & 0xff_ffff);
    if Enemy(boss).active(x.b) == 0 || Enemy(boss).kind(x.b) != 3 {
        return false;
    }
    let shape = super::boss_3::Boss3(boss).body_shape(x.b);
    if shape == 0 {
        return false;
    }
    super::boss_3::body::push(x.b, boss, x.o, shape)
}

/// Vertical oscillation (by `+0x33`): long offsets added to the height `+0xc`, in one direction
/// then the other depending on the countdown `+0xa8` (period 72 frames, reversal at 36).
const BOB_OFFSETS: &str = "boss/bob_offsets";

/// 0x833b6: one step of a boss's vertical oscillation.
pub fn bob<R: Bench>(x: &mut Cx<'_, R>) {
    let i = (x.u8(f::KEY_FRAME) & 0x7f) as u32 * 4;
    let mut d = long(x, BOB_OFFSETS, i);
    if x.u16(Boss::BOB_TIMER) as i16 <= 36 {
        d = d.wrapping_neg();
    }
    x.add32(Obj::Z, d);
    // `subq.w #1` then `bgt`: exact signed comparison of the old value (0x8000 also restarts at 72)
    let before = x.u16(Boss::BOB_TIMER);
    x.set16(Boss::BOB_TIMER, before.wrapping_sub(1));
    if before as i16 <= 1 {
        x.set16(Boss::BOB_TIMER, 72);
    }
}
