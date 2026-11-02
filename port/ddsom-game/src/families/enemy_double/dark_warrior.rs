//! Double of type 26 (DarkWarrior 2): replays the movement of its master.

use super::*;

// ---------------------------------------------------------------------------------------------
// Type 26 (0x77736): "DarkWarrior 2" (h)
// ---------------------------------------------------------------------------------------------

/// Double of an enemy of type 25 (word `+0xb0`): instead of drawing an animation of its own, it replays
/// the master's movement. The four bytes `+0xb8` to `+0xbb` of the master (group, mode, phase, step)
/// say how to copy its speeds: identically, inverted, or by replaying the movement
/// (`0x1d90`: X, Y, Z; `0x1da6`: X, Z; `0x1d78`: X) so that the double stays in sync with the
/// obstacles. When the master is hit (`+0xb8 = 4`), it also replays the hit reaction with
/// the common routines. When the master or the double dies, it creates an effect (EFFCT 0x32).
pub(super) mod dark_warrior_double {
    use super::*;

    const MASTER: u32 = 0xb0;
    /// Group, mode, phase and step of the master.
    const GROUP: u32 = 0xb8;
    const MODE: u32 = 0xb9;
    const PHASE: u32 = 0xba;
    const STEP: u32 = 0xbb;
    /// Table of hit reaction addresses (master's code `+0x49`).
    const REACTIONS: u32 = 0x77aec;
    /// Terrain under the object that reverses the depth speed (bytes per terrain 0..8).
    const REVERSING_TERRAIN: u32 = 0x77d08;
    const ATTACKED: u16 = 0x400;
    const MASTER_ATTACKED: u16 = 0x406;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        let m = x.linked(MASTER);
        if x.u16(f::STATE) == ATTACKED || m.u16(x.b, f::STATE) == MASTER_ATTACKED {
            return vanish(x);
        }
        match m.u8(x.b, GROUP) {
            0 | 2 => follow(x),
            4 => struck(x),
            6 => free(x),
            _ => {}
        }
        if x.u8(0) == 0 {
            return;
        }
        let m = x.linked(MASTER);
        let v = m.u8(x.b, 0);
        x.set8(0, v);
        let v = m.u8(x.b, f::FLIP);
        x.set8(f::FLIP, v);
        let v = m.u16(x.b, 0x54);
        x.set16(0x54, v);
        x.set8(0x3f, 4);
        x.reinsert();
    }

    /// The master forgets its double, which creates an effect then disappears.
    fn vanish<R: Bench>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        m.set_u16(x.b, MASTER, 0);
        spawn_effect(x);
        free(x);
    }

    /// 0x77c8a: EFFCT 0x32 effect in place of the double (one pixel higher).
    fn spawn_effect<R: Bench>(x: &mut Cx<'_, R>) {
        let Some(e) = x.alloc(&pools::EFFCT) else { return };
        e.set_u8(x.b, 0, 1);
        e.set_u8(x.b, 2, 0x32);
        e.set_u8(x.b, 3, 7);
        e.set_u16(x.b, f::CREATOR, x.o.0 as u16);
        for off in [0x8, 0xc, 0x10] {
            let v = x.u16(off);
            e.set_u16(x.b, off, v);
        }
        x.add16(0x10, 1);
        let v = x.u8(f::FLIP);
        e.set_u8(x.b, f::FLIP, v);
        let v = x.u32(0x34);
        e.set_u32(x.b, 0x34, v);
        let v = x.u8(0x3c);
        e.set_u8(x.b, 0x3c, v);
        let v = x.u8(0x3f);
        e.set_u8(x.b, 0x3f, v);
        e.set_u16(x.b, 0x30, 0x28);
    }

    // --- copies from the master ----------------------------------------------------------------------

    /// 0x77cdc: copy of the animation frame (`+0x34` long, `+0x38`) from the master.
    fn copy_frame<R: Bench>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        let v = m.u32(x.b, 0x34);
        x.set32(0x34, v);
        let v = m.u8(x.b, 0x38);
        x.set8(0x38, v);
    }

    /// Master's height (`+0xc`, word), then its frame.
    fn copy_height<R: Bench>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        let v = m.u16(x.b, 0xc);
        x.set16(0xc, v);
    }

    fn copy_height_frame<R: Bench>(x: &mut Cx<'_, R>) {
        copy_height(x);
        copy_frame(x);
    }

    /// The master's three speeds (X, Z, Y: longs at `+0x14`, `+0x18`, `+0x1c`).
    fn copy_speeds<R: Bench>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        for off in [0x14, 0x18, 0x1c] {
            let v = m.u32(x.b, off);
            x.set32(off, v);
        }
    }

    /// Master's speeds inverted in X and Y (Z kept), height and frame (0x777fa, 0x77a34).
    fn mirror_speeds<R: Bench>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        let z = m.u32(x.b, 0x18);
        x.set32(0x18, z);
        let vx = m.u32(x.b, 0x14).wrapping_neg();
        let vy = m.u32(x.b, 0x1c).wrapping_neg();
        x.set32(0x14, vx);
        x.set32(0x1c, vy);
        copy_height_frame(x);
    }

    /// The master has not finished its animation step (`+0x33` zero): the double replays the movement.
    fn replay<'a, R: Bench>(x: &mut Cx<'a, R>, mover: fn(&mut Cx<'a, R>)) {
        let m = x.linked(MASTER);
        if m.u8(x.b, 0x33) == 0 {
            mover(x);
        }
    }

    // --- active master (+0xb8 = 0 or 2) -----------------------------------------------------------

    fn follow<R: Bench>(x: &mut Cx<'_, R>) {
        if !paused(x) {
            let m = x.linked(MASTER);
            match m.u8(x.b, MODE) {
                0 => mode0(x),
                2 => mode2(x),
                4 => mode4(x),
                _ => copy_frame(x),
            }
        }
        lib::keep_in_zone(x);
        lib::collide_terrain(x);
    }

    /// The master walks: speeds inverted, or movement replayed.
    fn mode0<R: Bench>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        match m.u8(x.b, PHASE) {
            0 | 2 | 4 | 6 => mirror_speeds(x),
            8 => {
                replay(x, |x| { x.move_xyz(); });
                copy_height_frame(x);
            }
            10 | 12 => {
                x.move_xyz();
                copy_height_frame(x);
            }
            _ => copy_height_frame(x),
        }
    }

    /// 0x77842: the master is flying or jumping; copy or replay depending on the phase.
    fn mode2<R: Bench>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        match m.u8(x.b, PHASE) {
            4 | 6 => {
                let step = m.u8(x.b, STEP);
                match step {
                    0 => {
                        copy_speeds(x);
                        copy_frame(x);
                    }
                    2 => {
                        if m.u8(x.b, 0x33) == 0 {
                            x.move_xyz();
                            copy_height(x);
                            if terrain_reverses(x) {
                                let v = (x.u32(0x1c) as i32).wrapping_neg();
                                x.set32(0x1c, v as u32);
                            }
                        }
                        copy_frame(x);
                    }
                    4 | 6 => {
                        x.move_xyz();
                        copy_height(x);
                        if terrain_reverses(x) {
                            let v = (x.u32(0x1c) as i32).wrapping_neg();
                            x.set32(0x1c, v as u32);
                        }
                        copy_frame(x);
                    }
                    _ => copy_frame(x),
                }
            }
            8 => {
                let step = m.u8(x.b, STEP);
                match step {
                    0 => copy_speeds(x),
                    2 => {
                        if m.u8(x.b, 0x33) == 0 {
                            x.move_xyz();
                            copy_height(x);
                        }
                    }
                    4 | 6 => {
                        x.move_xyz();
                        copy_height(x);
                    }
                    _ => {}
                }
                copy_frame(x);
            }
            10 => {
                let step = m.u8(x.b, STEP);
                match step {
                    2 => {
                        if m.u8(x.b, 0x33) != 0 {
                            copy_speeds(x);
                            x.move_xz();
                            copy_height(x);
                        }
                    }
                    4 => {
                        x.move_xz();
                        copy_height(x);
                        let v = m.u16(x.b, 0x16);
                        x.set16(0x16, v);
                    }
                    6 => {
                        x.move_xz();
                        let h = m.u16(x.b, 0xc);
                        x.set16(0xc, h);
                        if (h as i16) <= 0 {
                            let v = m.u32(x.b, 0x14);
                            x.set32(0x14, v);
                        }
                    }
                    8 => {
                        x.move_x();
                        mode4(x);
                        return;
                    }
                    _ => {}
                }
                copy_frame(x);
            }
            _ => copy_frame(x),
        }
    }

    /// 0x779da: the master is in a special state; only a few phases move the double.
    fn mode4<R: Bench>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        match m.u8(x.b, PHASE) {
            10 => match m.u8(x.b, STEP) {
                0 => mirror_speeds(x),
                2 => {
                    replay(x, |x| { x.move_xyz(); });
                    copy_height_frame(x);
                }
                4 | 6 => {
                    x.move_xyz();
                    copy_height_frame(x);
                }
                _ => copy_height_frame(x),
            },
            _ => copy_frame(x),
        }
    }

    /// 0x77cee: does the terrain under the double reverse its depth speed?
    fn terrain_reverses<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        let t = crate::video::terrain_of(x.b, x.o);
        let terrain = t.number & 0x3f;
        *x.c.d2() = *x.c.d2() & !0xffff | terrain as u32;
        if terrain as u8 >= 9 {
            return false;
        }
        let b = x.rom8(REVERSING_TERRAIN + terrain as u32);
        b != 0
    }

    // --- hit master (+0xb8 = 4) ---------------------------------------------------------------

    fn struck<R: Bench>(x: &mut Cx<'_, R>) {
        x.set16(f::HP, 0x7fff);
        let m = x.linked(MASTER);
        match m.u8(x.b, MODE) {
            0 => replay_reaction(x),
            2 => match m.u8(x.b, PHASE) {
                0 => {
                    let v = m.u32(x.b, 0x14);
                    x.set32(0x14, v);
                    x.set16(0xc, 0);
                    copy_frame(x);
                }
                2 | 4 => {
                    let v = m.u16(x.b, 0x16);
                    x.set16(0x16, v);
                    x.move_x();
                    copy_frame(x);
                }
                _ => {}
            },
            4 => match m.u8(x.b, PHASE) {
                0 | 2 | 8 => copy_frame(x),
                4 => {
                    for off in [0x14, 0x18, 0x1c] {
                        let v = m.u16(x.b, off);
                        x.set16(off, v);
                    }
                    copy_frame(x);
                }
                6 => {
                    x.move_xyz();
                    copy_height_frame(x);
                }
                _ => {}
            },
            6 | 8 => match m.u8(x.b, PHASE) {
                0 => {
                    copy_speeds(x);
                    copy_frame(x);
                }
                2 => {
                    x.move_xz();
                    copy_height_frame(x);
                }
                4 | 6 => copy_frame(x),
                _ => {}
            },
            10 => copy_frame(x),
            _ => {}
        }
        lib::keep_in_zone(x);
        lib::collide_terrain(x);
    }

    /// 0x77ab6: the double takes the master's reaction state and replays its reaction routine.
    fn replay_reaction<R: Bench>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        let sub = m.u16(x.b, PHASE);
        x.set16(f::SUB2, sub);
        copy_speeds(x);
        copy_height(x);
        let code = m.u8(x.b, 0x49) as u32;
        let routine = x.prog32(REACTIONS + code);
        run_reaction(x, routine);
        x.set8(0x3b, 0);
    }
}
