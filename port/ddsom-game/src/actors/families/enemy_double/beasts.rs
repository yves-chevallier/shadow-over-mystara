//! Doubles that draw their own animation: types 24 (Manticore), 23 (Spectre) and 22 (Chimera).

use super::*;
use crate::world::data::RomAddr;

// ---------------------------------------------------------------------------------------------
// Type 24 (0x76afc): "Manticore" (h)
// ---------------------------------------------------------------------------------------------

/// Graphical double of an enemy of type 25. It sits one pixel lower than its master
/// (depth + 1) and picks its animation with `choose_active` (master in state 0 or 2, outside pause) or
/// `choose_hit` (master hit).
pub(super) mod manticore {
    use super::*;

    /// Neutral set (rest) played when the master is in a state with no gesture of its own.
    const REST: RomAddr = RomAddr(0x16c2da);

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        let m = Master::read(x);
        let choice = match m.state {
            0 | 2 if !paused(x) => choose_active(m.sub, m.s6, m.s7 as i8),
            4 => choose_hit(m.sub, m.s6 as i8),
            _ => Nothing,
        };
        play(x, choice);
        if m.state == st::DEAD {
            free(x);
        }
        mirror(x, 1);
    }

    /// Active master: animation from its substate `sub`, `s6` (sub-substate) and `s7` (current
    /// gesture, signed: <= 0 = nothing). The `step`s are the indices of the original's small tables
    /// (`s7 - 2`).
    fn choose_active(sub: u8, s6: u8, s7: i8) -> Play {
        let step = (s7 as u8).wrapping_sub(2);
        let gesture = s7 > 0;
        match sub {
            0 => match s6 {
                2 => Anim(0x16c59a),
                4 => Anim(REST.0),
                _ => Nothing,
            },
            2 => match s6 {
                0 if s7 != 0 => Anim(REST.0),
                2 if s7 != 0 => Anim(0x16c33a),
                4 | 6 if s7 != 0 => Anim(0x16c39a),
                8 if gesture => match step {
                    0 => Anim(0x16c43a),
                    2 => Anim(0x16c48a),
                    _ => Nothing,
                },
                10 if gesture => match step {
                    0 | 2 | 4 => Anim(0x16c49a),
                    6 => Anim(0x16c4ba),
                    _ => Nothing,
                },
                _ => Nothing,
            },
            4 if gesture => match (s6, step) {
                (0, 0) => Anim(0x16c52a),
                (2, 0) => Anim(0x16c59a),
                (2, 4) => Anim(0x16c5fa),
                (2, 6) => Anim(0x16c60a),
                (4, 0) => Anim(0x16c59a),
                (4, 4) => Anim(0x16c66a),
                (4, 6) => Anim(0x16c67a),
                (6, 0) => Anim(0x16c69a),
                (6, 2) => Anim(0x16c6ba),
                (6, 4) => Anim(0x16c6ca),
                (6, 6) => Anim(0x16c6da),
                (8, 0 | 2 | 4 | 6 | 8) => Anim(0x16c6fa),
                (8, 10) => Anim(REST.0),
                (10, 0 | 2 | 4 | 6) | (12, 0 | 2 | 4 | 6 | 8) => Anim(REST.0),
                _ => Nothing,
            },
            6 | 8 | 10 | 12 if s6 != 0 => Anim(REST.0),
            _ => Nothing,
        }
    }

    /// Hit master (state 4): hit animation (`s6` = gesture, signed).
    fn choose_hit(sub: u8, s6: i8) -> Play {
        match sub {
            0 | 4 | 6 | 8 if s6 > 0 => Hit(0x16c75a),
            2 if s6 > 0 && matches!((s6 as u8).wrapping_sub(2), 0 | 2) => Anim(0x16c51a),
            _ => Nothing,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 23 (0x76592): "Spectre" (h)
// ---------------------------------------------------------------------------------------------

/// Graphical double of an enemy, with two animation sets (normal and "wounded" when `+0xaf` is
/// non-zero) and its own life tracking: when its HP (`+0x62`) drop below the known minimum
/// (`+0x64`), it plays a sound and, at 0 HP, raises `+0xaf`, dies with a sound (`0xe4188`) and creates an effect
/// (EFFCT 0x3f). Position: one pixel higher than the master (depth - 1).
pub(super) mod spectre {
    use super::*;
    /// Rest set (normal, wounded).
    const REST: (u32, u32) = (0x16b7c2, 0x16b822);

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        if x.u8(Double::WOUNDED) == 0 {
            hurt(x, 0x25, 1);
        }
        let m = Master::read(x);
        let injured = x.u8(Double::WOUNDED) != 0;
        let choice = match m.state {
            0 | 2 if !paused(x) => choose_active(m.sub, m.s6, m.s7 as i8, injured),
            4 => choose_hit(m.sub, m.s6 as i8, injured),
            _ => Nothing,
        };
        play(x, choice);
        if m.state == st::DEAD {
            free(x);
        }
        mirror(x, 0xffff_ffff);
    }

    /// Set (normal, wounded) according to the double's state.
    fn pick(injured: bool, normal: u32, hurt: u32) -> Play {
        Anim(if injured { hurt } else { normal })
    }

    fn choose_active(sub: u8, s6: u8, s7: i8, injured: bool) -> Play {
        let step = (s7 as u8).wrapping_sub(2);
        let gesture = s7 > 0;
        let rest = || pick(injured, REST.0, REST.1);
        match sub {
            0 => match s6 {
                2 => pick(injured, 0x16bdea, 0x16be4a),
                4 => rest(),
                _ => Nothing,
            },
            2 => match s6 {
                0 if s7 != 0 => rest(),
                2 if s7 != 0 => pick(injured, 0x16b882, 0x16b8e2),
                4 | 6 if s7 != 0 => pick(injured, 0x16b942, 0x16b9e2),
                8 if gesture => match step {
                    0 => pick(injured, 0x16ba82, 0x16bad2),
                    2 => pick(injured, 0x16bb22, 0x16bb32),
                    _ => Nothing,
                },
                10 if gesture => match step {
                    0 | 2 | 4 => pick(injured, 0x16bb42, 0x16bb62),
                    6 => pick(injured, 0x16bb82, 0x16bb92),
                    _ => Nothing,
                },
                _ => Nothing,
            },
            4 if gesture => match (s6, step) {
                (0, 0) => pick(injured, 0x16bd0a, 0x16bd7a),
                (2, 0) => pick(injured, 0x16bdea, 0x16be4a),
                (2, 4) => pick(injured, 0x16beaa, 0x16beba),
                (2, 6) => pick(injured, 0x16beca, 0x16bf2a),
                (4, 0) => pick(injured, 0x16bdea, 0x16be4a),
                (4, 4) => pick(injured, 0x16bf8a, 0x16bf9a),
                (4, 6) => pick(injured, 0x16bfaa, 0x16bfca),
                (6, 0) => pick(injured, 0x16bfea, 0x16c00a),
                (6, 2) => pick(injured, 0x16c02a, 0x16c03a),
                (6, 4) => pick(injured, 0x16c04a, 0x16c05a),
                (6, 6) => pick(injured, 0x16c06a, 0x16c08a),
                (8, 0 | 2 | 4 | 6 | 8) => pick(injured, 0x16c0aa, 0x16c10a),
                (8, 10) => rest(),
                (10, 0 | 2 | 4 | 6) | (12, 0 | 2 | 4 | 6 | 8) => rest(),
                _ => Nothing,
            },
            6 | 8 | 10 | 12 if s6 != 0 => rest(),
            _ => Nothing,
        }
    }

    fn choose_hit(sub: u8, s6: i8, injured: bool) -> Play {
        match sub {
            0 | 4 | 6 | 8 if s6 > 0 => Hit(if injured { 0x16c20a } else { 0x16c16a }),
            2 if s6 > 0 && matches!((s6 as u8).wrapping_sub(2), 0 | 2) => pick(injured, 0x16bc42, 0x16bc52),
            _ => Nothing,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 22 (0x75f42): "Chimera" (h)
// ---------------------------------------------------------------------------------------------

/// Graphical double with the same frame as the Spectre (two animation sets, own life, one pixel
/// higher than the master), with in addition gestures whose animation is restarted on demand
/// (`+0xb6`, set 0x16abd6) and an ESHL 0x31 shot (breath) emitted at the end of a master gesture.
pub(super) mod chimera {
    use super::*;

    /// Rest set (normal, wounded).
    const REST: (u32, u32) = (0x16ac0c, 0x16ac6c);

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        if x.u8(Double::WOUNDED) == 0 {
            hurt(x, 0x21, 0);
        }
        let m = Master::read(x);
        let injured = x.u8(Double::WOUNDED) != 0;
        let choice = match m.state {
            0 | 2 if !paused(x) => choose_active(m.sub, m.s6, m.s7 as i8, injured),
            4 => choose_hit(m.sub, m.s6 as i8, injured),
            _ => Nothing,
        };
        play(x, choice);
        if m.state == st::DEAD {
            free(x);
        }
        mirror(x, 1);
    }

    fn pick(injured: bool, normal: u32, hurt: u32) -> Play {
        Anim(if injured { hurt } else { normal })
    }

    fn choose_active(sub: u8, s6: u8, s7: i8, injured: bool) -> Play {
        let step = (s7 as u8).wrapping_sub(2);
        let gesture = s7 > 0;
        let rest = || pick(injured, REST.0, REST.1);
        match sub {
            0 => match s6 {
                2 => pick(injured, 0x16b214, 0x16b274),
                4 => rest(),
                _ => Nothing,
            },
            2 => match s6 {
                0 if s7 != 0 => rest(),
                2 if s7 != 0 => pick(injured, 0x16accc, 0x16ad2c),
                4 | 6 if s7 != 0 => pick(injured, 0x16ad8c, 0x16ae2c),
                8 if gesture => match step {
                    0 => pick(injured, 0x16aecc, 0x16af1c),
                    2 => pick(injured, 0x16af6c, 0x16af7c),
                    _ => Nothing,
                },
                10 if gesture => match step {
                    0 | 2 | 4 => pick(injured, 0x16af8c, 0x16afac),
                    6 => pick(injured, 0x16afcc, 0x16afdc),
                    _ => Nothing,
                },
                _ => Nothing,
            },
            4 if gesture => match (s6, step) {
                (0, 0) => pick(injured, 0x16b134, 0x16b1a4),
                (2 | 4, 0) => pick(injured, 0x16b214, 0x16b274),
                (2, 4) => pick(injured, 0x16b2d4, 0x16b2e4),
                (2, 6) => pick(injured, 0x16b2f4, 0x16b354),
                (4, 4) => pick(injured, 0x16b3b4, 0x16b3c4),
                (4, 6) => pick(injured, 0x16b3d4, 0x16b3f4),
                (6, 0) => pick(injured, 0x16b414, 0x16b434),
                (6, 2) => pick(injured, 0x16b454, 0x16b464),
                (6, 4) => pick(injured, 0x16b474, 0x16b484),
                (6, 6) => pick(injured, 0x16b494, 0x16b4b4),
                (8, 0 | 2 | 6 | 8) => pick(injured, 0x16b214, 0x16b274),
                (8, 4) => Restart(0xb),
                (8, 10) => rest(),
                (10, 0) => AnimClear(0x16ac0c),
                (10, 2) => RestartSpawn(0xc),
                (10, 4) => Restart(0xd),
                (10, 6) => rest(),
                (12, 0) => AnimClear(0x16ac0c),
                (12, 2) => Restart(0xc),
                (12, 4) => Step,
                (12, 6) => Restart(0xd),
                (12, 8) => rest(),
                _ => Nothing,
            },
            6 | 8 | 10 | 12 if s6 != 0 => rest(),
            _ => Nothing,
        }
    }

    fn choose_hit(sub: u8, s6: i8, injured: bool) -> Play {
        match sub {
            0 | 4 | 6 | 8 if s6 > 0 => Hit(if injured { 0x16b6c0 } else { 0x16b620 }),
            2 if s6 > 0 && matches!((s6 as u8).wrapping_sub(2), 0 | 2) => pick(injured, 0x16b08c, 0x16b09c),
            _ => Nothing,
        }
    }
}
