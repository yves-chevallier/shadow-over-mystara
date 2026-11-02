//! Small routines that remained in 68000 and that the Rust still called: object creators (shots,
//! effects, enemies, system objects), palette copies, a few game variables.
//!
//! Creators' contract: A0 = the creator; on return A1 = the created object, carry raised if the pool is
//! full. The residual registers that the original leaves (D0, D1, D2, D5, flags) are reproduced.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::mem::{prog16, prog8, rd16, rd32, rts, set_low_word, wr16, wr32, wr8, A5};
use crate::objects::Obj;
use super::cx::{handler_without, Bench, Cx};
use crate::objects::Current;
use crate::{regs, Native};
use crate::objects::pools;
use super::effects_creators::leftovers::{Pure, Regs};


/// Allocates in a pool; `None` (carry raised) if the pool is full.
fn alloc(c: &mut Cpu, b: &mut Cps2Bus, pool: &pools::Pool) -> Option<Obj> {
    pool.alloc_68k(c, b)
}

/// Flags of a `move`/`clr` of `v`: N and Z according to the value, V and C at zero.
fn moved(c: &mut Cpu, v: u32, size: Size) {
    c.flags_logic(v, size);
}

/// Creates an object of `pool` of type `kind`, linked to the creator by word `link` (flags of that `move.w`).
pub(crate) fn linked_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, pool: &pools::Pool, kind: u8, link: u32) -> Option<Obj> {
    let o = r.allocated(b, pool)?;
    o.set_u8(b, Obj::ACTIVE, 1);
    o.set_u8(b, Obj::KIND, kind);
    o.set_u16(b, link, creator.word());
    r.flags(creator.word() as u32, Size::Word);
    Some(o)
}

fn linked(c: &mut Cpu, b: &mut Cps2Bus, pool: &pools::Pool, kind: u8, link: u32) {
    let me = Obj(c.a[0]);
    linked_via(c, b, me, pool, kind, link);
    rts(c, b);
}

/// 0x4c930 for the player module, still on the processor: player effect 0x10 (revival) linked to `player`, with the
/// registers the original leaves (A0 = the player, A1, carry, flags).
pub fn revive_68k(c: &mut Cpu, b: &mut Cps2Bus, player: Obj) -> Option<Obj> {
    c.a[0] = player.0;
    linked_via(c, b, player, &pools::PEFF, 0x10, 0x50)
}

/// 0x4f132: player effect 0x21 (second phase of the spell quake), linked to the player by `+0x50`.
pub fn quake_b(b: &mut Cps2Bus, player: Obj) -> Option<Obj> {
    linked_via(&mut Pure, b, player, &pools::PEFF, 0x21, 0x50)
}

/// `quake_b` with the original's registers (callers by address: `player/`): A1, carry and flags.
#[track_caller]
pub fn quake_b_68k(c: &mut Cpu, b: &mut Cps2Bus, player: Obj) -> Option<Obj> {
    c.a[0] = player.0;
    let r = linked_via(c, b, player, &pools::PEFF, 0x21, 0x50);
    crate::diag::dead_regs(c, "effects", CREATED);
    r
}

fn peff_16(c: &mut Cpu, b: &mut Cps2Bus) {
    linked(c, b, &pools::PEFF, 0x10, 0x50);
}
fn peff_33(c: &mut Cpu, b: &mut Cps2Bus) {
    linked(c, b, &pools::PEFF, 0x21, 0x50);
}
fn sobj_7(c: &mut Cpu, b: &mut Cps2Bus) {
    linked(c, b, &pools::SOBJ, 0x7, 0x46);
}
fn sobj_10(c: &mut Cpu, b: &mut Cps2Bus) {
    linked(c, b, &pools::SOBJ, 0xa, 0x46);
}

/// 0x54700: shot of type 0x17 that takes over the creator's word `+0x50`.
pub(crate) fn shot_17(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    let o = r.allocated(b, &pools::PSHL)?;
    o.set_u8(b, 0x0, 1);
    o.set_u8(b, 0x2, 0x17);
    let v = me.u16(b, 0x50);
    o.set_u16(b, 0x50, v);
    r.flags(v as u32, Size::Word);
    Some(o)
}

/// 0x54818 (variant left as is: `variant` zero), 0x54842 (variant 1) and 0x54872 (variant 2): shot of type
/// 0x18 linked to the creator by `+0x5c`, resistances 0x10; the created shot, `None` if the PSHL pool is full.
pub(crate) fn shot_18(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, variant: u8) -> Option<Obj> {
    let shot = r.allocated(b, &pools::PSHL)?;
    shot.set_u8(b, 0x0, 1);
    shot.set_u8(b, 0x2, 0x18);
    if variant != 0 {
        shot.set_u8(b, 0x3, variant);
    }
    shot.set_u8(b, 0x45, 0x10);
    shot.set_u8(b, 0x46, 0x10);
    shot.set_u16(b, 0x5c, me.word());
    Some(shot)
}

/// Native of `shot_18`: D0 = 1 if the shot is created.
/// Original glitch: 0x54872 (`always_zero`) returns D0 = 0 even when the shot is created.
fn shot_18_native(c: &mut Cpu, b: &mut Cps2Bus, variant: u8, always_zero: bool) {
    let me = Obj(c.a[0]);
    let created = shot_18(c, b, me, variant).is_some();
    c.d[0] = (created && !always_zero) as u32;
    moved(c, c.d[0], Size::Long);
    rts(c, b);
}

/// 0x76ec4: if more than two ENEMY slots remain, creates two enemies of type 0x19 (variants 0 and
/// 1) linked to the creator; true (D0 = 1) if created. The original does not test the allocations' carry.
pub(crate) fn enemy_pair_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj) -> bool {
    enemy_pair_kind_via(r, b, creator, 0x19)
}
/// 0x76ec4 (type 0x19) and 0x63ae0 (type 7, `creators`): two enemies of type `kind`, variants 0 and 1.
pub(crate) fn enemy_pair_kind_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, kind: u8) -> bool {
    let created = (rd16(b, A5 + 0x7e62) as i16) > 2;
    if created {
        for variant in 0..2 {
            // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
            let o = r.alloc_without_test(b, &pools::ENEMY);
            o.set_u8(b, 0x0, 1);
            o.set_u8(b, 0x2, kind);
            o.set_u8(b, 0x3, variant);
            o.set_u8(b, 0x2f, 0);
            o.set_u16(b, 0x46, creator.word());
        }
    }
    r.d(0, created as u32);
    r.flags(created as u32, Size::Long);
    created
}
/// 0x76ec4 with no registers.
pub fn enemy_pair_created(b: &mut Cps2Bus, creator: Obj) -> bool {
    enemy_pair_via(&mut Pure, b, creator)
}
fn enemy_pair(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    enemy_pair_via(c, b, me);
    rts(c, b);
}

/// 0x792f8 and 0x71e3c: `+0x53` takes over `+0x56`.
fn copy_56_to_53(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    let v = me.u8(b, 0x56);
    me.set_u8(b, 0x53, v);
    moved(c, v as u32, Size::Byte);
    rts(c, b);
}

/// 0xa66c0: creates D5 + 1 ESHL shots of type 0x3e whose variant decreases from D5 (byte of table 0xa66e2
/// indexed by D2) to 0.
fn eshl_volley(c: &mut Cpu, b: &mut Cps2Bus) {
    let row = 0xa66e2u32.wrapping_add(c.d[2] as i16 as i32 as u32);
    c.d[5] = prog8(b, row) as u32;
    loop {
        let Some(o) = alloc(c, b, &pools::ESHL) else { break };
        o.set_u8(b, 0x0, 1);
        o.set_u8(b, 0x2, 0x3e);
        o.set_u8(b, 0x3, c.d[5] as u8);
        moved(c, c.d[5], Size::Byte);
        let n = (c.d[5] as u16).wrapping_sub(1);
        set_low_word(&mut c.d[5], n);
        if n == 0xffff {
            break;
        }
    }
    rts(c, b);
}

/// 0xa7234 and 0xa727c: swelling of Boss 15's roar: a number of ESHL shots of type 0x41 (variant
/// `variant`) given by the table (according to `A5+0x7f92`), numbered from n-1 to 0 by `+0x2f`, linked by `+0x5c`.
/// D0.w and D1.w of the original: the table index, the counter (exhausted: 0xffff, or the number of the refused shot).
pub(crate) fn roar_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, big: bool) {
    let (table, variant) = if big { (0xa726c, 0) } else { (0xa72b4, 1) };
    let i = rd16(b, A5 + 0x7f92) >> 1;
    r.d_word(0, i);
    let total = prog8(b, table + i as u32);
    r.d(1, total as u32);
    let mut n = (total as u16).wrapping_sub(1);
    r.d_word(1, n);
    loop {
        let Some(o) = r.allocated(b, &pools::ESHL) else { break };
        o.set_u8(b, 0x0, 1);
        o.set_u8(b, 0x2, 0x41);
        o.set_u8(b, 0x3, variant);
        o.set_u8(b, 0x2f, n as u8);
        let link = creator.u16(b, 0x5c);
        o.set_u16(b, 0x5c, link);
        r.flags(link as u32, Size::Word);
        n = n.wrapping_sub(1);
        r.d_word(1, n);
        if n == 0xffff {
            break;
        }
    }
}
/// 0xa7234 (`big`) and 0xa727c with no registers.
pub fn roar_created(b: &mut Cps2Bus, creator: Obj, big: bool) {
    roar_via(&mut Pure, b, creator, big)
}
fn roar_big(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    roar_via(c, b, me, true);
    rts(c, b);
}
fn roar_small(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    roar_via(c, b, me, false);
    rts(c, b);
}

/// 0xa036e: harpy, companion creature (ESHL 0x10): copies the variant, the position offset by ±0x2f in X
/// and by -0x78 in Z; half the creator's HP (`+0x60 / 4`) goes into the creator's `+0x58`.
fn harpy_companion(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    let _ = harpy_companion_via(c, b, me);
    rts(c, b);
}

/// Core of 0xa036e: returns the companion and the HP/4 word that the original leaves in D2 (leftover read afterwards),
/// or, on failure, the A1 that the allocation leaves.
pub(crate) fn harpy_companion_via(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Result<(Obj, u16), Obj> {
    let o = r.alloc_or_rest(b, &pools::ESHL)?;
    let flip = me.u8(b, 0x2c) != 0;
    let v = me.u8(b, 0x3);
    o.set_u8(b, 0x3, v);
    o.set_u8(b, 0x0, 1);
    o.set_u8(b, 0x2, 0x10);
    o.set_u8(b, 0x2e, 0x3e);
    o.set_u16(b, 0x50, me.word());
    let v = me.u8(b, 0x43);
    o.set_u8(b, 0x43, v);
    let hp = me.u16(b, 0x60) >> 2;
    r.d_word(2, hp);
    me.set_u16(b, 0x58, hp);
    let v = me.u8(b, 0xa2);
    o.set_u8(b, 0x56, v);
    for off in [0x8, 0x10, 0xc] {
        let v = me.u32(b, off);
        o.set_u32(b, off, v);
    }
    let dx: u16 = if flip { 0x2fu16.wrapping_neg() } else { 0x2f };
    r.d_word(0, dx);
    r.d_word(1, 0x78u16.wrapping_neg());
    let x = o.u16(b, 0x8).wrapping_add(dx);
    o.set_u16(b, 0x8, x);
    let z = o.u16(b, 0xc).wrapping_add(0x78u16.wrapping_neg());
    o.set_u16(b, 0xc, z);
    let y = o.u32(b, 0x10).wrapping_add(1);
    o.set_u32(b, 0x10, y);
    let z = o.u32(b, 0xc).wrapping_sub(1);
    o.set_u32(b, 0xc, z);
    let f = me.u8(b, 0x2c);
    o.set_u8(b, 0x2c, f);
    r.flags(f as u32, Size::Byte);
    Ok((o, hp))
}

/// 0xa0566: harpy, ESHL 0x11 projectile: D5 goes into the creator's `+0x42`, A2 and the creator are the
/// links `+0x50` and `+0x56` (longs); the creator resets `+0xa5` to zero.
fn harpy_shot(c: &mut Cpu, b: &mut Cps2Bus) {
    let (me, d5, a2) = (Obj(c.a[0]), c.d[5] as u8, c.a[2]);
    let _ = harpy_shot_via(c, b, me, d5, a2);
    rts(c, b);
}

/// Core of 0xa0566: `d5` = the byte placed in the creator's `+0x42`, `target` = the long A2 linked at `+0x50`.
pub(crate) fn harpy_shot_via(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, d5: u8, target: u32) -> Option<Obj> {
    let o = r.allocated(b, &pools::ESHL)?;
    me.set_u8(b, 0x42, d5);
    let v = me.u8(b, 0x43);
    o.set_u8(b, 0x43, v);
    o.set_u32(b, 0x50, target);
    o.set_u32(b, 0x56, me.0);
    me.set_u8(b, 0xa5, 0);
    o.set_u8(b, 0x0, 1);
    o.set_u8(b, 0x2, 0x11);
    for off in [0x8, 0x10, 0xc] {
        let v = me.u32(b, off);
        o.set_u32(b, off, v);
    }
    let v = me.u8(b, 0x2c);
    o.set_u8(b, 0x2c, v);
    let v = me.u8(b, 0x3);
    o.set_u8(b, 0x3, v);
    r.flags(v as u32, Size::Byte);
    Some(o)
}

// ---------------------------------------------------------------------------------------------
// Game variables, palette copies, chains of initializations
// ---------------------------------------------------------------------------------------------

/// `movem.l (addr)+,list`: loads the registers of the mask (D0-D7 then A0-A7); returns the next address.
fn movem_load(c: &mut Cpu, b: &mut Cps2Bus, mask: u16, mut addr: u32) -> u32 {
    for bit in (0..16).filter(|bit| mask >> bit & 1 != 0) {
        let v = rd32(b, addr);
        if bit < 8 {
            c.d[bit] = v;
        } else {
            c.a[bit - 8] = v;
        }
        addr += 4;
    }
    addr
}

/// `movem.l list,(addr)`: stores the registers of the mask; returns the next address.
fn movem_store(c: &Cpu, b: &mut Cps2Bus, mask: u16, mut addr: u32) -> u32 {
    for bit in (0..16).filter(|bit| mask >> bit & 1 != 0) {
        wr32(b, addr, if bit < 8 { c.d[bit] } else { c.a[bit - 8] });
        addr += 4;
    }
    addr
}

/// D0-D6 and A3.
const MASK_D0_D6_A3: u16 = 0x087f;

/// 0x10158: four blocks of eight colors copied into the palette (GFX RAM) with the high bit
/// of each nibble raised (`ori.l #0xF000F000`).
fn palette_blocks_lit(c: &mut Cpu, b: &mut Cps2Bus) {
    for (dst, src) in [(0x91c2e0, 0x2538c0), (0x91c300, 0x2538e0), (0x91c360, 0x253900), (0x91c3e0, 0x253920)] {
        movem_load(c, b, MASK_D0_D6_A3, src);
        c.a[2] = src;
        movem_store(c, b, MASK_D0_D6_A3, dst);
        for k in 0..8 {
            let at = dst + 4 * k;
            let v = rd32(b, at) | 0xf000_f000;
            wr32(b, at, v);
            moved(c, v, Size::Long);
        }
        c.a[1] = dst + 0x20;
        c.d[6] = 0xffff;
    }
    rts(c, b);
}

/// 0x101da: like 0x10158 for scene 0x69, without brightening: four blocks of eight colors read in sequence
/// at 0x10218 (data space).
fn palette_blocks(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut src = 0x10218;
    for dst in [0x91c2e0, 0x91c300, 0x91c360, 0x91c3e0] {
        src = movem_load(c, b, MASK_D0_D6_A3, src);
        c.a[1] = dst;
        movem_store(c, b, MASK_D0_D6_A3, dst);
    }
    c.a[2] = src;
    rts(c, b);
}

/// 0x10158 (`brighten`) and 0x101da for a generic handler: the four blocks of eight colours copied into the palette,
/// like `palette_blocks_lit` and `palette_blocks`. Original glitch: the last block's `movem.l` leaves its colours 2 to
/// 5 in D2-D5, read back by the next handlers (`dispatch::Leftovers`).
pub(crate) fn four_palette_blocks<R: Bench>(x: &mut Cx<'_, R>, brighten: bool) {
    const DESTINATIONS: [u32; 4] = [0x91c2e0, 0x91c300, 0x91c360, 0x91c3e0];
    const BRIGHT_SOURCES: [u32; 4] = [0x2538c0, 0x2538e0, 0x253900, 0x253920];
    const SOURCES_69: u32 = 0x10218;
    let mut last = 0;
    for (k, dst) in DESTINATIONS.into_iter().enumerate() {
        let src = if brighten { BRIGHT_SOURCES[k] } else { SOURCES_69 + 0x20 * k as u32 };
        for i in 0..8 {
            let v = rd32(x.b, src + 4 * i);
            wr32(x.b, dst + 4 * i, if brighten { v | 0xf000_f000 } else { v });
        }
        last = src;
    }
    *x.c.d2() = rd32(x.b, last + 8);
    *x.c.d3() = rd32(x.b, last + 12);
    *x.c.d4() = rd32(x.b, last + 16);
    *x.c.d5() = rd32(x.b, last + 20);
}

/// 0x3525c for a generic handler: clears the start of scroll layer 1 (`video::clear_layer1_start`).
/// Original glitch: the empty cell stays in D2-D5 (`dispatch::Leftovers`).
pub(crate) fn clear_scroll1_start<R: Bench>(x: &mut Cx<'_, R>) {
    crate::video::clear_layer1_start(x.b);
    (*x.c.d2(), *x.c.d3(), *x.c.d4(), *x.c.d5()) = (0x1f_0000, 0x1f_0000, 0x1f_0000, 0x1f_0000);
}

/// 0x4aed0: two effects of type 5, variant 1, indices `+0x2f` 0 and 1, linked to the creator (`+0x50`).
pub(crate) fn peff_5_pair(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) {
    for index in [0, 1] {
        let Some(o) = r.allocated(b, &pools::PEFF) else { break };
        o.set_u8(b, 0x0, 1);
        o.set_u8(b, 0x2, 5);
        o.set_u8(b, 0x3, 1);
        o.set_u8(b, 0x2f, index);
        o.set_u16(b, 0x50, me.word());
        r.flags(me.word() as u32, Size::Word);
    }
}

/// 0x53746: twelve stones (shots of type 0x11, variants 0 to 11) spread around the camera in use,
/// X, Y offsets in a table (0x537ca, program space), height 0x100 above their impact point, delay
/// `+0x5c` from 0x40 to 0x148 in steps of 0x18; orientation and owner of the creator.
pub(crate) fn pshl_stones(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) {
    const OFFSETS: u32 = 0x537ca;
    const COUNT: u16 = 12;
    // D3: variant, D4: delay, D5: `dbf` counter; left to the following handlers (`dispatch::Leftovers`)
    let (mut variant, mut delay, mut n) = (0u8, 0x40u16, COUNT - 1);
    r.d(3, 0);
    r.d_word(4, delay);
    r.d_word(5, n);
    loop {
        let Some(o) = r.allocated(b, &pools::PSHL) else { break };
        o.set_u8(b, 0x0, 1);
        o.set_u8(b, 0x2, 0x11);
        let v = me.u8(b, 0x2c);
        o.set_u8(b, 0x2c, v);
        let v = me.u16(b, 0x50);
        o.set_u16(b, 0x50, v);
        o.set_u8(b, 0x3, variant);
        o.set_u16(b, 0x5c, delay);
        let (cx, cy) = if crate::ram::CAMERA_ALT_ACTIVE.get(b) != 0 { (0x556, 0x55a) } else { (0x506, 0x50a) };
        let (x, y) = (rd16(b, A5 + cx), rd16(b, A5 + cy));
        let index = (variant as u16).wrapping_shl(2) as u32;
        let (dx, dy) = (prog16(b, OFFSETS + index), prog16(b, OFFSETS + 2 + index));
        let ground = y.wrapping_add(dy);
        o.set_u16(b, 0x8, x.wrapping_add(dx));
        o.set_u16(b, 0x10, ground);
        o.set_u16(b, 0xc, y.wrapping_add(0x100).wrapping_sub(ground));
        r.d_word(0, ground);
        r.d_word(1, dx);
        r.d_word(2, dy);
        variant = variant.wrapping_add(1);
        r.d_byte(3, variant);
        delay = r.addition(0x18, delay as u32, Size::Word) as u16;
        r.d_word(4, delay);
        n = n.wrapping_sub(1);
        r.d_word(5, n);
        if n == 0xffff {
            break;
        }
    }
}

/// 0x1a5e8: twelve blocks of eight colors (0x25c4c0) copied into palette 0x91c480. Core.
pub fn palette_twelve_blocks(b: &mut Cps2Bus) {
    let (dst, src) = (0x91c480, 0x25c4c0);
    for i in 0..12 * 8 {
        let v = rd32(b, src + 4 * i);
        wr32(b, dst + 4 * i, v);
    }
}

/// `palette_twelve_blocks` with the original's registers (the last block in D1-D6, A1, A2).
fn palette_blocks_copy(c: &mut Cpu, b: &mut Cps2Bus) {
    const MASK: u16 = 0x067e; // D1-D6, A1, A2
    let (mut dst, mut src) = (0x91c480, 0x25c4c0);
    // `moveq #0xb,D7`: only flags set (neither `movem` nor `dbf` change any)
    moved(c, 0xb, Size::Long);
    for _ in 0..12 {
        src = movem_load(c, b, MASK, src);
        dst = movem_store(c, b, MASK, dst);
    }
    c.a[3] = src;
    c.a[4] = dst;
    c.d[7] = 0xffff;
    rts(c, b);
}

/// 0x1ad6a: 32 blocks of 32 bytes read from 0x285280 are stored in turn at the same place,
/// FF1190 (the original does not advance its destination: only the last block stays there), then eight colors of
/// 0x1ad4a vont en FF1390.
pub(crate) fn palette_page_restore(c: &mut Cpu, b: &mut Cps2Bus) {
    const MASK: u16 = 0x183f; // D0-D5, A3, A4
    let mut src = 0x285280;
    for _ in 0..32 {
        src = movem_load(c, b, MASK, src);
        movem_store(c, b, MASK, 0xff1190);
    }
    c.a[2] = src;
    c.d[6] = 0xffff;
    c.a[2] = movem_load(c, b, MASK_D0_D6_A3, 0x1ad4a);
    c.a[1] = 0xff1390;
    movem_store(c, b, MASK_D0_D6_A3, 0xff1390);
}

/// Natives of the screen flashes (color 14 of page 1, `palette::set_flash_color`): the flags of the `move.w`.
fn flash(c: &mut Cpu, b: &mut Cps2Bus, v: u16) {
    crate::palette::set_flash_color(b, v);
    moved(c, v as u32, Size::Word);
    rts(c, b);
}
/// Flash colors: yellow (0x1ac9a), green (0x1ac72), blue (0x1ac7c), end of the flash (0x1ac90).
pub(crate) const LIGHTNING_YELLOW: u16 = 0xfff0;
pub(crate) const LIGHTNING_GREEN: u16 = 0xf0f0;
pub(crate) const LIGHTNING_BLUE: u16 = 0xf00f;
pub(crate) const LIGHTNING_END: u16 = 0xff50;
fn flash_off(c: &mut Cpu, b: &mut Cps2Bus) {
    flash(c, b, LIGHTNING_END);
}
fn flash_a(c: &mut Cpu, b: &mut Cps2Bus) {
    flash(c, b, LIGHTNING_YELLOW);
}
fn flash_b(c: &mut Cpu, b: &mut Cps2Bus) {
    flash(c, b, LIGHTNING_GREEN);
}
fn flash_c(c: &mut Cpu, b: &mut Cps2Bus) {
    flash(c, b, LIGHTNING_BLUE);
}

/// The eight tile blocks of background 2 (0x26d4: D0 = block, A1 = destination).
const BACKGROUND_BLOCKS: [(u16, u32); 8] = [
    (0xce, 0x917000),
    (0xad, 0x917400),
    (0xaf, 0x917800),
    (0xaf, 0x917c00),
    (0xac, 0x916000),
    (0xcd, 0x916400),
    (0xae, 0x916800),
    (0xae, 0x916c00),
];

/// 0x2a52: background 2 on (tile blocks, bit 0 of video register `A5+0x12`, camera 2 in mode 0x80).
pub fn background_2_on(b: &mut Cps2Bus) -> crate::video::GridEnd {
    let mut end = None;
    for (block, dst) in BACKGROUND_BLOCKS {
        end = Some(crate::video::load_grid(b, crate::video::Grid::Layer2, block, dst));
    }
    let video = rd16(b, A5 + 0x12) | 1;
    wr16(b, A5 + 0x12, video);
    wr16(b, 0x804122, video);
    wr16(b, 0x804108, 0x9200);
    wr8(b, A5 + 0x53c, 2);
    wr16(b, A5 + 0x51e, 0x80);
    wr8(b, A5 + 0x36, 1);
    end.expect("huit blocs")
}

/// `background_2_on`, with the original's registers: D0 and A1 of the last block (0x26d4), flags of the last `move.b`.
fn scroll_2_on(c: &mut Cpu, b: &mut Cps2Bus) {
    let end = background_2_on(b);
    c.d[0] = 0xffff;
    c.a[1] = end.destination;
    c.a[2] = end.indices;
    moved(c, 1, Size::Byte);
    rts(c, b);
}

/// 0x2a36: background 1 on (camera in mode 0x18); 0x2a42: `A5+0x548` = D0, background 1 set to 0x34.
pub fn background_1_on(b: &mut Cps2Bus) {
    background_1_in_mode(b, 0x18);
}
fn background_1_in_mode(b: &mut Cps2Bus, mode: u16) {
    wr8(b, A5 + 0x4ff, 0);
    wr16(b, A5 + 0x51e, mode);
}
fn scroll_1_on_with(c: &mut Cpu, b: &mut Cps2Bus, mode: u16) {
    background_1_in_mode(b, mode);
    moved(c, mode as u32, Size::Word);
    rts(c, b);
}
fn scroll_1_on(c: &mut Cpu, b: &mut Cps2Bus) {
    scroll_1_on_with(c, b, 0x18);
}
/// 0x2a42: `A5+0x548` = `value` (D0.w of the original), background 1 set to 0x34.
pub fn background_1_lit_a(b: &mut Cps2Bus, value: u16) {
    wr16(b, A5 + 0x548, value);
    background_1_in_mode(b, 0x34);
}
fn scroll_1_on_at_d0(c: &mut Cpu, b: &mut Cps2Bus) {
    background_1_lit_a(b, c.d[0] as u16);
    moved(c, 0x34, Size::Word);
    rts(c, b);
}

/// 0x133f6 / 0x133f0: the "startup done" indicator (`A5-0x715e`) raised / lowered.
pub fn startup_done(b: &mut Cps2Bus, done: bool) {
    wr8(b, A5 - 0x715e, done as u8);
}
fn booted_set(c: &mut Cpu, b: &mut Cps2Bus) {
    startup_done(b, true);
    moved(c, 1, Size::Byte);
    rts(c, b);
}
fn booted_clear(c: &mut Cpu, b: &mut Cps2Bus) {
    startup_done(b, false);
    moved(c, 0, Size::Byte);
    rts(c, b);
}

/// 0xdd12c: the pickup objects' spawn lock (`A5-0x5ac4`) raised.
pub(crate) fn item_lock_clear(c: &mut Cpu, b: &mut Cps2Bus) {
    wr8(b, A5 - 0x5ac4, 0);
    moved(c, 0, Size::Byte);
}

/// 0x3baea: player's `+0x1f0`, from table 0x3baf8 indexed by the quarter of the counter `A5+0x7f92`.
pub(crate) fn player_table_byte(c: &mut Cpu, b: &mut Cps2Bus, player: Obj) {
    c.a[0] = player.0;
    let i = rd16(b, A5 + 0x7f92) >> 2;
    set_low_word(&mut c.d[0], i);
    let v = prog8(b, 0x3baf8 + i as u32);
    wr8(b, player.0 + 0x1f0, v);
    moved(c, v as u32, Size::Byte);
}

/// 0x1c44a: the game variables `A5+0x2c..0x6b` set to zero.
pub fn zero_game_variables(b: &mut Cps2Bus) {
    for k in 0..16 {
        wr32(b, A5 + 0x2c + 4 * k, 0);
    }
}
fn game_vars_clear(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[0] = 0;
    zero_game_variables(b);
    c.a[0] = A5 + 0x6c;
    c.d[6] = 0xffff;
    moved(c, 0, Size::Long);
    rts(c, b);
}

/// 0x180ac: reset of the fixed screen: SET and EFFCT pools, sprites, sorted list.
pub fn reset_fixed_screen(b: &mut Cps2Bus) {
    wr32(b, A5 - 0x70b0, 0);
    crate::objects::lists::unlink_all(b);
    crate::sprites::clear_lists(b);
    pools::SET.reset(b);
    pools::EFFCT.reset(b);
}

/// `reset_fixed_screen`; A0 = the last unlinked object (0x1f184).
fn fixed_screen_reset(c: &mut Cpu, b: &mut Cps2Bus) {
    wr32(b, A5 - 0x70b0, 0);
    crate::objects::lists::unlink_all_68k(c, b);
    crate::sprites::clear_lists(b);
    pools::SET.reset(b);
    pools::EFFCT.reset(b);
    rts(c, b);
}

// ---------------------------------------------------------------------------------------------
// Hit reaction of two enemies (common reaction table, `react`)
// ---------------------------------------------------------------------------------------------


/// 0x78ad2, 0x71680 and 0x65236: the hit enemy shakes in place. Step 0: saves X (`+0xa`), starts
/// animation 8 of set `anim` (with the hit type, 0x103d6, if `strike_kind`) and sets the counter (`+0x90` = 16).
/// Step 2: shifts X by ±2 according to the counter parity and the orientation, then, once the counter is
/// exhausted, goes back to the original X
/// and to state 0x04040200. `_table`: original jump table on `+6`.
fn shake<R: Bench>(x: &mut Cx<'_, R>, _table: u32, anim: u32, strike_kind: bool) {
    let step = x.u8(0x6);
    match step {
        0 => {
            x.add8(0x6, 2);
            x.set8(0x0, 2);
            let at = x.u16(0x8);
            x.set16(0xa, at);
            x.set16(0x84, 0);
            let v = x.u8(0x56);
            x.set8(0x53, v);
            x.set8(0x90, 0x10);
            if strike_kind {
                crate::families::enemy_lib::start(x, anim, 8);
            } else {
                x.anim_start(anim, 8);
            }
        }
        2 => {
            let timer = x.u8(0x90);
            if (timer as i8) <= 0 {
                let at = x.u16(0xa);
                x.set16(0x8, at);
                x.set8(0x0, 1);
                x.set32(0x4, 0x0404_0200);
                x.set32(0x58, 0);
                return;
            }
            let timer = timer - 1;
            x.set8(0x90, timer);
            let mut d: i16 = 2;
            if timer & 1 != 0 {
                d = -d;
            }
            if x.u8(0x2c) != 0 {
                d = -d;
            }
            x.add16(0x8, d as u16);
        }
        _ => {}
    }
}
/// Reaction specific to a type, from the enemy's reaction table (code 0x4c: shaking), designated by its
/// original address; `None` if the address is not one.
pub(crate) fn own_reaction<R: Bench>(x: &mut Cx<'_, R>, routine: u32) -> Option<()> {
    match routine {
        // engin de guerre (type 27)
        0x78ad2 => shake(x, 0x78ae0, 0x171a94, true),
        // ogre (type 18)
        0x71680 => shake(x, 0x7168e, 0x1778a4, true),
        // gargoyle (type 8); never reached by the scenarios (see docs/code-mort.md)
        0x65236 => shake(x, 0x65244, 0x162f12, false),
        _ => return None,
    }
    Some(())
}

const fn native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus), outputs: u32) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs }
}

const CREATED: u32 = regs::a(1) | regs::CCR;

pub const NATIVES: &[Native] = &[
    native(0x4c930, "create_effect_10", peff_16, CREATED),
    native(0x4f132, "create_effect_21", peff_33, CREATED),
    native(0xdf4bc, "create_sobj_7", sobj_7, CREATED),
    native(0xdf896, "create_sobj_10", sobj_10, CREATED),
    native(0x54700, "create_shot_17", |c, b| { let me = Obj(c.a[0]); shot_17(c, b, me); rts(c, b) }, CREATED),
    native(0x54818, "create_shot_18_a", |c, b| shot_18_native(c, b, 0, false), CREATED | regs::d(0)),
    native(0x54842, "create_shot_18", |c, b| shot_18_native(c, b, 1, false), CREATED | regs::d(0)),
    native(0x54872, "create_shot_18_c", |c, b| shot_18_native(c, b, 2, true), CREATED | regs::d(0)),
    native(0x76ec4, "create_enemies_19", enemy_pair, CREATED | regs::d(0)),
    native(0x792f8, "copy_56_53_a", copy_56_to_53, regs::CCR),
    native(0x71e3c, "copy_56_53_b", copy_56_to_53, regs::CCR),
    native(0xa66c0, "create_volley_3e", eshl_volley, CREATED | regs::d(5)),
    native(0xa7234, "create_roar_big", roar_big, CREATED | regs::d(0) | regs::d(1)),
    native(0xa727c, "create_roar_small", roar_small, CREATED | regs::d(0) | regs::d(1)),
    native(0xa036e, "create_companion_harpy", harpy_companion, CREATED | regs::d(0) | regs::d(1) | regs::d(2)),
    native(0xa0566, "create_shot_harpy", harpy_shot, CREATED),
    native(0x10158, "palette_four_blocks", palette_blocks_lit, regs::ALL),
    native(0x101da, "palette_four_blocks_69", palette_blocks, regs::ALL),
    native(0x1ac90, "lightning_palette_end", flash_off, regs::CCR),
    native(0x4aed0, "create_effects_5", |c, b| { let me = Obj(c.a[0]); peff_5_pair(c, b, me); rts(c, b) }, CREATED),
    native(0x1a5e8, "palette_twelve_blocks", palette_blocks_copy, regs::ALL),
    native(0x1ad6a, "palette_page_restitue", |c, b| { palette_page_restore(c, b); rts(c, b) }, regs::ALL),
    native(0x1ac9a, "lightning_palette_yellow", flash_a, regs::CCR),
    native(0x1ac72, "lightning_palette_green", flash_b, regs::CCR),
    native(0x1ac7c, "lightning_palette_blue", flash_c, regs::CCR),
    native(0x2a52, "background_2_lit", scroll_2_on, regs::d(0) | regs::a(1) | regs::CCR),
    native(0x2a36, "background_1_lit", scroll_1_on, regs::CCR),
    native(0x2a42, "background_1_lit_d0", scroll_1_on_at_d0, regs::CCR),
    native(0x133f6, "boot_raised", booted_set, regs::CCR),
    native(0x133f0, "boot_lowered", booted_clear, regs::CCR),
    native(0x1c44a, "zero_game_variables", game_vars_clear, regs::ALL),
    native(0x180ac, "fixed_screen_reset", fixed_screen_reset, regs::CCR),
];

/// Enemy reactions specific to a type (`own_reaction`), at their original address (the game calls them directly
/// depuis `enemy_common::react`).
pub const SHAKES: &[Native] = &[
    handler_without(0x78ad2, "enemy_shake_a", |b, a| run_shake(b, a, 0x78ad2)),
    handler_without(0x71680, "enemy_shake_b", |b, a| run_shake(b, a, 0x71680)),
    handler_without(0x65236, "enemy_shake_c", |b, a| run_shake(b, a, 0x65236)),
];

fn run_shake(b: &mut Cps2Bus, a: &mut crate::dispatch::Dispatch, routine: u32) {
    own_reaction(&mut crate::families::cx::Gx::dispatch(b, a), routine);
}
