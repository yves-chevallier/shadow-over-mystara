//! Player effects (PEFF), eighth series: types 25 to 29 (downpour, wizard's lightning, ring, moon, prayer aura). See
//! `docs/families/joueur.md`.

use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::Obj;
use crate::objects::Current;

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Has the player's move (`+7`) changed since creation (byte `field`)?
fn gesture_changed<R: Bench>(x: &mut Cx<'_, R>, field: u32) -> bool {
    let owner = x.linked(f::OWNER);
    let gesture = owner.u8(x.b, 0x7);
    gesture != x.u8(field)
}

/// Type 25 (0x4dd02): downpour (magic 13, kind 0x19). Variant 0: cloud placed in front of the caster; variant 1: drops
/// (rank `+2f`) that launch the lightning (PSHL 19) on the frame of the move.
mod shower {
    use super::*;

    const ANIMS: u32 = 0x1bb4c0;
    const OFFSETS_A: u32 = 0x4dd84;
    const OFFSETS_B: u32 = 0x4de22;
    const OFFSETS_C: u32 = 0x4deac;
    const SPEEDS_A: u32 = 0x4df52;
    const SPEEDS_B: u32 = 0x4dfc4;
    const SFX: u32 = 0xe8530;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4dd10) {
            0 if !child => {
                x.add8(0x4, 2);
                place(x, OFFSETS_A, true);
                x.anim(ANIMS, 0);
                x.reinsert();
            }
            0 => {
                x.add8(0x4, 2);
                if x.u8(0x15e) == 0x18 {
                    place(x, OFFSETS_B, true);
                } else {
                    place(x, OFFSETS_C, false);
                }
                x.anim(ANIMS, 1);
                x.reinsert();
            }
            2 if !child => {
                if gesture_changed(x, 0x5d) {
                    x.add8(0x4, 2);
                }
                x.set_bit(0x1, 1, false);
                let v = x.u8(0x5c) ^ 1;
                x.set8(0x5c, v);
                if v != 0 {
                    x.set_bit(0x1, 1, true);
                }
                x.anim_step();
            }
            2 => bolt(x),
            _ => x.free_peff(),
        }
    }

    /// Places the cloud on the caster: offset read from a table, row of 8 bytes (caster's variant, plus 4 for the
    /// second copy) or of 4 bytes.
    fn place<R: Bench>(x: &mut Cx<'_, R>, table: u32, wide: bool) {
        let owner = follow_owner(x);
        let mut row = (owner.u8(x.b, 0x3) >> 1) as u32;
        if wide {
            row <<= 3;
            if x.u8(0x2f) != 0 {
                row += 4;
            }
        } else {
            row <<= 2;
        }
        let dx = x.rom16(table + row);
        let dz = x.rom16(table + row + 2);
        *x.c.d2() = dz as u32;
        let dx = signed(dx, x.u8(f::FLIP) != 0);
        x.add16(0x8, dx);
        x.add16(0xc, dz);
        x.add16(0x10, 0xffff);
    }

    /// Variant 1, state 2: a drop brings down a lightning bolt at the end of its animation.
    fn bolt<R: Bench>(x: &mut Cx<'_, R>) {
        let wide = x.u8(0x15e) == 0x18;
        if x.anim_step() {
            x.add8(0x4, 2);
        }
        if x.u8(0x33) == 0 {
            return;
        }
        let owner = x.linked(f::OWNER);
        let rank = x.u8(0x2f);
        let mut row = (owner.u8(x.b, 0x3) >> 1) as u32;
        if wide {
            row <<= 3;
            if rank != 0 {
                row += 4;
            }
        } else {
            row <<= 2;
        }
        let speeds = if wide { SPEEDS_A } else { SPEEDS_B };
        let vx = x.rom16(speeds + row);
        let vz = x.rom16(speeds + row + 2);
        // Original glitch: the rank and the row stay in D3 and D2 (`dispatch::Leftovers`)
        *x.c.d3() = rank as u32;
        *x.c.d2() = row;
        crate::families::creators::bolt_shot(x.b, owner, vx, vz);
        crate::sound::play(x.b, SFX, Obj(ext(owner)));
        x.set8(0x33, 0);
    }
}

/// Type 26 (0x4e038): lightning on the scene (wizard's magic, kind 0x1a): the column (variant 0) then the shards
/// (variant 1, rank `+2f`) that blink with the move; the rank 0 shard sweeps all the monsters (bosses, then enemies) in
/// the zone when it hits.
mod strike {
    use super::*;

    const ANIMS: u32 = 0x1bb5ea;
    const FRAMES: u32 = 0x11bffa;
    const POWER_BOSS: u32 = 0x1043b4;
    const POWER_ENEMY: u32 = 0x104396;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4e046) {
            0 if !child => {
                x.add8(0x4, 2);
                let owner = follow_owner(x);
                let _ = owner;
                let dx = signed(4, x.u8(f::FLIP) != 0);
                x.add16(0x8, dx);
                x.add16(0xc, 0x7c);
                x.set16(0x12, 0x4000);
                x.anim(ANIMS, 0);
                x.reinsert();
            }
            0 => {
                x.add8(0x4, 2);
                let owner = x.linked(f::OWNER);
                for off in [0x8, 0x10] {
                    let v = owner.u16(x.b, off);
                    x.set16(off, v);
                }
                if x.u8(0x2f) == 0 {
                    x.set16(0x12, 0xc000);
                }
                let n = x.u8(0x2f) as u32 + 1;
                x.anim(ANIMS, n);
                x.reinsert();
            }
            2 if !child => {
                x.anim_step();
                if gesture_changed(x, 0x59) {
                    x.add8(0x4, 2);
                }
            }
            2 => {
                if x.u8(0x5) == 0 {
                    x.anim_step();
                    let owner = x.linked(f::OWNER);
                    let same = owner.u8(x.b, 0x7) == x.u8(0x59);
                    if !same {
                        x.add8(0x5, 2);
                        x.set_bit(0x1, 1, false);
                        let n = x.u8(0x2f) as u32 + 3;
                        return x.anim(ANIMS, n);
                    }
                    x.set_bit(0x1, 1, true);
                    let timer = owner.u16(x.b, 0x168) & 1;
                    if timer != 0 {
                        x.set_bit(0x1, 1, false);
                    }
                    if x.u8(0x2f) == 0 && x.u8(0x33) != 0 {
                        x.set8(0x33, 0);
                        sweep(x);
                    }
                } else {
                    if x.anim_step() {
                        x.add8(0x4, 2);
                    }
                    x.set_bit(0x1, 1, true);
                    // Original glitch: the original re-reads `(0x168,A1)` with the A1 left by the dispatch loop, the
                    // address of the handler (0x4e038): a word of its code, and not the player
                    let timer = x.data16(0x4e038 + 0x168) & 1;
                    if timer != 0 {
                        x.set_bit(0x1, 1, false);
                    }
                }
            }
            _ => x.free_peff(),
        }
    }

    /// `0x4e190`: hurts the monsters in the lightning zone: the bosses (3 slots), then the enemies (22).
    fn sweep<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = x.linked(f::OWNER);
        for (first, count, power) in [(0x3a1eu32, 3u32, POWER_BOSS), (0x299e, 22, POWER_ENEMY)] {
            let mut a1 = 0xff8000 + first;
            for _ in 0..count {
                hit(x, owner, Obj(a1), power);
                a1 += 0xc0;
            }
        }
    }

    fn hit<R: Bench>(x: &mut Cx<'_, R>, player: Obj, m: Obj, power: u32) {
        if m.u8(x.b, 0) == 0 || m.u16(x.b, 0x62) == 0 || m.u8(x.b, 0x38) == 0 || m.u8(x.b, 0x40) != 0 {
            return;
        }
        if m.u32(x.b, 0x94) >> 11 & 1 != 0 {
            return;
        }
        let set = m.u8(x.b, 0x2e) as u32;
        let table = crate::mem::rd32(x.b, FRAMES + (set << 2));
        let frame = table.wrapping_add((m.u8(x.b, 0x38) as u32) << 3);
        let b4 = x.data8(frame + 4) as i8 as i16 as u16;
        let b5 = x.data8(frame + 5);
        let theirs = m.u16(x.b, 0x10).wrapping_add(b4);
        let d0 = (-8i16 as u16).wrapping_add(x.u16(0x10)).wrapping_sub(theirs);
        let reach = 0x2cu8.wrapping_add(b5) as u16;
        if d0.wrapping_add(reach) > reach.wrapping_mul(2) {
            return;
        }
        let dx = signed(x.data8(frame) as i8 as i16 as u16, m.u8(x.b, f::FLIP) != 0);
        let d0 = x.u16(0x8).wrapping_sub(m.u16(x.b, 0x8).wrapping_add(dx));
        let reach = 0x90u8.wrapping_add(x.data8(frame + 1)) as u16;
        let d0 = d0.wrapping_add(reach);
        if d0 > reach.wrapping_mul(2) {
            return;
        }
        // hit
        let v = player.u16(x.b, 0x190).wrapping_add(2);
        player.set_u16(x.b, 0x190, v);
        let hits = m.u8(x.b, 0x9c).wrapping_add(1);
        m.set_u8(x.b, 0x9c, hits);
        m.set_u32(x.b, 0x4, 0x0400_0000);
        m.set_u8(x.b, 0x1, 1);
        m.set_u8(x.b, 0x49, 0x84);
        let (class_id, variant_id) = (player.u8(x.b, 0x2), player.u8(x.b, 0x3));
        m.set_u8(x.b, 0x48, class_id);
        m.set_u8(x.b, 0x4b, variant_id);
        m.set_u16(x.b, 0x54, player.0 as u16);
        let class = player.u8(x.b, 0x10c) as u32;
        let mut d0 = x.data8(power + class) as u16;
        if player.u8(x.b, 0x10e) == 0x20 {
            d0 += d0 >> 2;
        }
        if player.u8(x.b, 0x10e) == 0x21 {
            d0 += d0 >> 1;
        }
        if player.u8(x.b, 0x166) == 4 {
            d0 += d0 >> 1;
        }
        let hp = m.u16(x.b, 0x62).wrapping_sub(d0);
        m.set_u16(x.b, 0x62, if (hp as i16) < 0 { 0 } else { hp });
        if m.u8(x.b, 0x28) != 4 && matches!(m.u8(x.b, 0x2), 0 | 0x13) {
            m.set_u16(x.b, 0x62, 0);
        }
        if m.u16(x.b, 0x62) == 0 && m.u8(x.b, 0x93) != 0 {
            player.set_u8(x.b, 0x18a, 1);
            let v = m.u8(x.b, 0x9a);
            x.set_a5_u8(0x4f, v);
        }
    }
}

/// Type 27 (0x4e316): ring of fire or ice (magic 16, kind 0x1b): variant 0 placed in front of the caster; variant 1
/// that blazes and shakes the screen at the end of the move.
mod ring {
    use super::*;

    const ANIMS: u32 = 0x1bb80a;
    const OFFSETS: u32 = 0x4e412;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4e324) {
            0 => {
                x.add8(0x4, 2);
                let owner = x.linked(f::OWNER);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                x.copy_pos(owner);
                let weapon = owner.u8(x.b, 0x15e);
                if weapon == 0x15 || weapon == 0x16 {
                    let (mut dx, mut dz) = (0x17u16, 0x26u16);
                    if child {
                        (dx, dz) = (4, 0x50);
                        if weapon == 0x15 && owner.u8(x.b, 0x1ee) != 0 {
                            (dx, dz) = (-0x37i16 as u16, 0x36);
                        }
                    }
                    let dx = signed(dx, x.u8(f::FLIP) != 0);
                    x.add16(0x8, dx);
                    x.add16(0xc, dz);
                } else {
                    let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
                    let dx = x.rom16(OFFSETS + row);
                    let dz = x.rom16(OFFSETS + row + 2);
                    let dx = signed(dx, x.u8(f::FLIP) != 0);
                    x.add16(0x8, dx);
                    x.add16(0xc, dz);
                }
                x.add16(0x10, 0xffff);
                let n = x.u8(0x3) as u32;
                x.anim(ANIMS, n);
                x.reinsert();
            }
            2 if !child => {
                x.set_bit(0x1, 1, true);
                let v = x.u8(0x5a) ^ 1;
                x.set8(0x5a, v);
                if v != 0 {
                    x.set_bit(0x1, 1, false);
                }
                if gesture_changed(x, 0x5b) {
                    x.add8(0x4, 2);
                }
                x.anim_step();
            }
            2 => {
                if !x.anim_step() {
                    return;
                }
                x.add8(0x4, 2);
                x.set_a5_u16(0x4ba, 0x100);
                x.set8(0x5c, 8);
                x.set8(0x5d, 3);
                x.set8(0x5e, 1);
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, 0x15e) == 0x16 {
                    crate::palette::set_flash_color(x.b, crate::families::small::LIGHTNING_YELLOW);
                } else {
                    crate::palette::set_flash_color(x.b, crate::families::small::LIGHTNING_GREEN);
                }
            }
            4 if child => {
                x.add8(0x5c, 0xff);
                if x.u8(0x5c) != 0 {
                    return;
                }
                x.set8(0x5c, 4);
                x.set_a5_u16(0x4ba, 0);
                let v = x.u8(0x5e) ^ 1;
                x.set8(0x5e, v);
                if v != 0 {
                    x.set_a5_u16(0x4ba, 0x100);
                    x.set8(0x5c, 8);
                }
                x.add8(0x5d, 0xff);
                if x.u8(0x5d) == 0 {
                    x.add8(0x4, 2);
                    x.set_a5_u16(0x4ba, 0);
                }
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 28 (0x4e4fa): moon (magic 12, kind 0x1c), its glow (variant 1) placed on the caster.
mod moon {
    use super::*;

    const ANIMS: u32 = 0x1bb8f4;
    const OFFSETS: u32 = 0x4e578;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4e508) {
            0 => {
                x.add8(0x4, 2);
                follow_owner(x);
                let row = (x.u8(0x3) as u32) << 3;
                let dx = x.rom16(OFFSETS + row);
                let dz = x.rom16(OFFSETS + row + 2);
                let dy = x.rom16(OFFSETS + row + 4);
                let dp = x.rom16(OFFSETS + row + 6);
                let dx = signed(dx, x.u8(f::FLIP) != 0);
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                x.add16(0x10, dy);
                x.add16(0x12, dp);
                let n = x.u8(0x3) as u32;
                x.anim(ANIMS, n);
                x.reinsert();
            }
            2 => {
                x.anim_step();
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, 0x7) == 6 {
                    x.add8(0x4, 2);
                    return x.anim(ANIMS, if child { 5 } else { 4 });
                }
                if !child {
                    x.add16(0x58, 1);
                    if x.u8(0x33) != 0 {
                        x.set8(0x33, 0);
                        x.set_bit(0x1, 1, false);
                        x.set8(0x5c, 1);
                    }
                    if x.u8(0x5c) != 0 {
                        x.add16(0x8, 2);
                        x.add16(0xc, 1);
                        if x.u16(0x58) & 1 == 0 {
                            x.add16(0x8, 0xfffc);
                            x.add16(0xc, 0xfffe);
                        }
                    } else {
                        blink(x);
                    }
                } else {
                    x.set_bit(0x1, 1, true);
                    x.add16(0x58, 1);
                    let t = x.u16(0x58) & 1;
                    if t == 0 {
                        x.set_bit(0x1, 1, false);
                    }
                }
            }
            4 => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
                x.set_bit(0x1, 1, true);
                x.add16(0x58, 1);
                let t = x.u16(0x58) & 1;
                if t == 0 {
                    x.set_bit(0x1, 1, false);
                }
            }
            _ => x.free_peff(),
        }
    }

    fn blink<R: Bench>(x: &mut Cx<'_, R>) {
        x.set_bit(0x1, 1, true);
        let t = x.u16(0x58) & 1;
        if t == 0 {
            x.set_bit(0x1, 1, false);
        }
    }
}

/// Type 29 (0x4e6ca): prayer aura (kind 0x1d): two nimbi placed on the caster.
mod aura {
    use super::*;

    const ANIMS: u32 = 0x1bbc9a;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4e6d8) {
            0 => {
                x.add8(0x4, 2);
                let owner = x.linked(f::OWNER);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                if child {
                    x.set8(f::FLIP, flip ^ 0x20);
                }
                x.copy_pos(owner);
                let (mut dx, mut dz) = (-0x1ai16 as u16, 0x45u16);
                if child {
                    (dx, dz) = (-0x17i16 as u16, 0x68);
                }
                let dx = signed(dx, owner.u8(x.b, f::FLIP) != 0);
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                x.add16(0x10, 0xffff);
                let n = x.u8(0x3) as u32;
                x.anim(ANIMS, n);
                x.reinsert();
            }
            2 => {
                let end = x.anim_step();
                if !child {
                    if gesture_changed(x, 0x59) {
                        x.add8(0x4, 2);
                    }
                } else if end {
                    x.add8(0x4, 2);
                }
            }
            _ => x.free_peff(),
        }
    }
}

handlers! {
    0x4dd02 "peff_25" #[without] shower::run,
    0x4e038 "peff_26" #[without] strike::run,
    0x4e316 "peff_27" #[without] ring::run,
    0x4e4fa "peff_28" #[without] moon::run,
    0x4e6ca "peff_29" #[without] aura::run,
}
