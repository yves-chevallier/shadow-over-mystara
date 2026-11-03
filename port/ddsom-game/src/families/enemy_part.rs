//! Composite enemy parts (ENEMY type 7). See `docs/families/enemy.md`.
//!
//! A part is an enemy with no combat logic of its own: it sticks to its host (word `+0x46`), copies its position with
//! an offset specific to its variant, and plays the animations that the host's state (`+0xa2`) commands. It notifies
//! the host of the end of its gestures (`+0xa3` of the host).

use super::cx::{handlers, Cx};
use crate::families::cx::Bench;
use super::enemy_common::*;
use crate::record::record;
use crate::objects::Obj;
use crate::objects::Current;

// ---------------------------------------------------------------------------------------------
// Type 7 (0x63b2a): "Wyvern" (h), tentacle of a Displacer Beast
// ---------------------------------------------------------------------------------------------

/// Tentacle of the host (an enemy of type 19, or 14 for another animation set); variant 0 = attacking tentacle, variant
/// 1 = accompanying tentacle. Substates (`+5`) chosen from byte `+0xa2` of the host by a ROM table (0x63fa8): 0 wait, 2
/// attack start, 4 hit, 6 return, 8 special hit, 10 recoil. As long as the host is no longer a valid RAM address or is
/// freed, the part goes to state 6 (release).
/// Fields of a part beyond the common ones of `Enemy` (`record!`); the host (a troll or a displacer beast) has the
/// same two state bytes, which the part copies.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tentacle(pub Obj);

impl Tentacle {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Tentacle {
        /// State of the host shown by its parts (h).
        host_state, set_host_state, HOST_STATE: u8 @ 0xa2;
        /// Non-zero on the host: its parts end (h).
        host_done, set_host_done, HOST_DONE: u8 @ 0xa3;
        /// Countdown of the rest animation, and of the life of a part left alone (word).
        idle_timer, set_idle_timer, IDLE_TIMER: u16 @ 0xa4;
    }
}

mod tentacle {
    use super::*;

    /// Fields of the host (word `+0x46`) read by the part, and kept by the part itself.
    const HOST_STATE: u32 = Tentacle::HOST_STATE;
    const HOST_DONE: u32 = Tentacle::HOST_DONE;
    const HOST_LAST_HIT: u32 = Enemy::STRIKE_KEY;
    /// Offset (X, Y, fraction of Y, Z) per variant, 8 bytes per row.
    const OFFSETS: &str = "part/offsets";
    /// Substate from the host's state (bytes in data space).
    const SUB_FOR_HOST: &str = "part/sub_for_host";
    /// Wait durations of the rest animation (words, indexed by an even draw).
    const IDLE_DELAYS: &str = "part/idle_delays";
    /// Animation sets by host type: (other host, Displacer Beast).
    const BODY: (u32, u32) = (0x17dbb0, 0x1824d4);
    const ALT: (u32, u32) = (0x17dbd2, 0x1824f6);
    const MID: (u32, u32) = (0x17dbba, 0x1824de);
    const DISPLACER_BEAST: u8 = 0x13;
    const TROLL: u8 = 0xe;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        match x.u8(f::STATE) {
            st::INIT => init(x),
            st::ACTIVE => active(x),
            st::HIT => hit(x),
            st::DEAD => free(x),
            _ => {}
        }
    }

    /// Live host, or `None`: then the part goes to state 6 (`0x63e96`).
    fn host<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
        let h = host_ptr(x)?;
        if h.u8(x.b, Enemy::ACTIVE) == 0 {
            x.set8(f::STATE, st::DEAD);
            return None;
        }
        Some(h)
    }

    /// Host designated by `+0x46`, without checking that it is active.
    fn host_ptr<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
        let w = x.u16(f::HOST);
        if (w as i16) >= 0 {
            x.set8(f::STATE, st::DEAD);
            return None;
        }
        Some(x.linked(f::HOST))
    }

    /// The host that the original keeps in A1 (reread after the animations): that of `+0x46`, which `follow` and `host`
    /// read before (the link does not change in the meantime).
    fn a1<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
        x.linked(f::HOST)
    }

    // --- state 0: creation ---------------------------------------------------------------------

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(f::STATE, 2);
        let Some(h) = host(x) else { return };
        if h.u8(x.b, Enemy::KIND) == TROLL {
            x.set8(Enemy::OWN_PALETTE, 1);
            x.set8(Enemy::PALETTE, 0x1c);
        }
        x.set8(f::BOX_SET, 0x2e);
        x.set8(f::SUB, 0);
        x.set16(f::SUB2, 0);
        follow(x);
        lib::set_immunities(x);
        let n = if x.u8(Enemy::VARIANT) != 0 { 3 } else { 2 };
        start(x, BODY, n);
    }

    /// Sticks to the host: position, orientation and flags copied, plus the variant's offset (X inverted when the host
    /// faces left).
    fn follow<R: Bench>(x: &mut Cx<'_, R>) {
        let Some(h) = host_ptr(x) else { return };
        for off in [0x8, 0x10, 0xc] {
            let v = h.u32(x.b, off);
            x.set32(off, v);
        }
        let flip = h.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        let flags = h.u8(x.b, Enemy::DISPLAY);
        x.set8(Enemy::DISPLAY, flags);
        let row = (x.u8(Enemy::VARIANT) as u32) << 3;
        let t = tables::table(OFFSETS);
        let mut dx = t.at16(x.b, row) as i16;
        let dy = t.at16(x.b, row + 2);
        let dy_frac = t.at16(x.b, row + 4) as i16;
        let dz = t.at16(x.b, row + 6);
        if flip != 0 {
            dx = dx.wrapping_neg();
        }
        x.add16(Obj::X, dx as u16);
        x.add16(Obj::Y, dy);
        x.add32(Obj::Y, dy_frac as i32 as u32);
        x.add16(Obj::Z, dz);
    }

    /// Starts animation `n` of the set `sets` (that of the Displacer Beast if the host is one).
    fn start<R: Bench>(x: &mut Cx<'_, R>, sets: (u32, u32), n: u32) {
        let mut set = sets.0;
        let w = x.u16(f::HOST);
        if (w as i16) < 0 && x.linked(f::HOST).u8(x.b, Enemy::KIND) == DISPLACER_BEAST {
            set = sets.1;
        }
        x.anim_start(set, n as u16);
    }

    // --- state 2: active -----------------------------------------------------------------------

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        follow(x);
        if paused(x) {
            if x.u8(Enemy::VARIANT) != 0 {
                return finish(x);
            }
            let l = x.u32(f::STATE) & !0xff;
            if l == 0x0204_0400 || l == 0x0208_0400 {
                x.set16(Tentacle::IDLE_TIMER, 1);
            }
        }
        let Some(h) = host(x) else { return };
        let host_state = h.u8(x.b, HOST_STATE);
        if host_state != x.u8(HOST_STATE) {
            x.set8(HOST_STATE, host_state);
            if x.u8(Enemy::VARIANT) == 0 {
                h.set_u8(x.b, HOST_DONE, 0);
            }
            let sub = tables::table(SUB_FOR_HOST).at8(x.b, host_state as u32);
            x.set8(f::SUB, sub);
            x.set16(f::SUB2, 0);
        }
        match x.u8(f::SUB) {
            0 => idle(x),
            2 => step2(x),
            4 => step4(x),
            6 => step6(x),
            8 => step8(x),
            10 => step10(x),
            _ => {}
        }
        finish(x);
    }

    /// Common end: hidden when the host's last hit is negative, then put back in the list.
    fn finish<R: Bench>(x: &mut Cx<'_, R>) {
        let Some(h) = host(x) else { return };
        if (h.u8(x.b, HOST_LAST_HIT) as i8) < 0 {
            x.set_bit(Enemy::DISPLAY, 1, true);
        }
        x.reinsert();
    }

    /// Starts the rest animation and draws the wait duration (`+0x30`).
    fn idle_anim<R: Bench>(x: &mut Cx<'_, R>) {
        start(x, BODY, 2);
        let r = rand_and(x, 0xe);
        let delay = tables::table(IDLE_DELAYS).at16(x.b, r);
        x.set16(Obj::ANIM_TIMER, delay);
    }

    /// Simple substate of variant 1: starts animation 3 once.
    fn variant1_once<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::SUB2) == 0 {
            x.add8(f::SUB2, 2);
            start(x, BODY, 3);
        }
    }

    fn idle<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(Enemy::VARIANT) != 0 {
            return variant1_once(x);
        }
        if x.u8(f::SUB2) == 0 {
            x.add8(f::SUB2, 2);
            idle_anim(x);
        }
        if x.anim_step() {
            return idle_anim(x);
        }
        if x.u8(Enemy::KEY_FRAME) == 0 {
            return;
        }
        x.set8(Enemy::KEY_FRAME, 0);
        if x.u8(Enemy::KIND) == TROLL {
            lib::sound(x, crate::sound::CMD_311_411_B);
        } else {
            lib::sound(x, crate::sound::CMD_311_411);
        }
    }

    fn step2<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(Enemy::VARIANT) == 0 {
            if x.u8(f::SUB2) == 0 {
                x.add8(f::SUB2, 2);
                idle_anim(x);
            }
            return;
        }
        match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                x.set8(Enemy::STRIKE_KIND, 0x48);
                start(x, MID, 0);
            }
            2 => {
                if !x.anim_step() {
                    return;
                }
                x.add8(f::SUB2, 2);
                a1(x).set_u8(x.b, HOST_DONE, 1);
            }
            _ => {}
        }
    }

    fn step4<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(Enemy::VARIANT) != 0 {
            return variant1_once(x);
        }
        match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                start(x, MID, 1);
            }
            2 => {
                if !x.anim_step() {
                    return;
                }
                x.add8(f::SUB2, 2);
                x.set16(Tentacle::IDLE_TIMER, 0x168);
                start(x, MID, 2);
                if var::GAME_ON.get(x.b) != 0 {
                    lib::sound(x, crate::sound::CMD_377_377);
                }
            }
            4 => {
                let n = (x.u16(Tentacle::IDLE_TIMER) as i16).wrapping_sub(1);
                x.set16(Tentacle::IDLE_TIMER, n as u16);
                if n <= 0 {
                    x.add8(f::SUB2, 2);
                    start(x, MID, 3);
                } else {
                    crate::families::creators::enemy_shot_9_via(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
                }
            }
            6 => {
                if !x.anim_step() {
                    return;
                }
                x.add8(f::SUB2, 2);
                a1(x).set_u8(x.b, HOST_DONE, 1);
            }
            8 => a1(x).set_u8(x.b, HOST_DONE, 1),
            _ => {}
        }
    }

    fn step6<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(Enemy::VARIANT) != 0 {
            return variant1_once(x);
        }
        if x.u8(f::SUB2) == 0 {
            x.add8(f::SUB2, 2);
            start(x, MID, 8);
        }
    }

    fn step8<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(Enemy::VARIANT) != 0 {
            return variant1_once(x);
        }
        match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                start(x, MID, 9);
            }
            2 => {
                if !x.anim_step() {
                    return;
                }
                x.add8(f::SUB2, 2);
                x.set16(Tentacle::IDLE_TIMER, 0x168);
                start(x, MID, 10);
                crate::families::eshl_creators::pair_9(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
                lib::sound(x, crate::sound::CMD_379);
                lib::sound(x, crate::sound::CMD_311_411);
            }
            4 => {
                let n = (x.u16(Tentacle::IDLE_TIMER) as i16).wrapping_sub(1);
                x.set16(Tentacle::IDLE_TIMER, n as u16);
                if n <= 0 {
                    x.add8(f::SUB2, 2);
                    start(x, MID, 11);
                }
            }
            6 => {
                if x.anim_step() {
                    x.add8(f::SUB2, 2);
                }
            }
            8 => a1(x).set_u8(x.b, HOST_DONE, 1),
            _ => {}
        }
    }

    fn step10<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(Enemy::VARIANT) != 0 {
            return variant1_once(x);
        }
        if x.u8(f::SUB2) == 0 {
            x.add8(f::SUB2, 2);
            start(x, ALT, 0);
        } else {
            x.anim_step();
        }
    }

    // --- state 4: hit --------------------------------------------------------------------------

    /// The host was hit: the part syncs to its HP (`+0x60/+0x62/+0x64` at 0x7fff).
    fn hit<R: Bench>(x: &mut Cx<'_, R>) {
        follow(x);
        if x.u16(Enemy::HP_MAX) != x.u16(f::HP) {
            let e = Enemy(x.o);
            e.set_hp_max(x.b, 0x7fff);
            e.set_hp(x.b, 0x7fff);
            e.set_hp_last(x.b, 0x7fff);
        }
        x.set32(f::STATE, 0x0200_0000);
        finish(x);
    }
}

handlers! {
    ENEMY_7 0x63b2a "enemy_7" tentacle::run,
}
