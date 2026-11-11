//! Tools: the natives of `crate::families::small` for the original code that calls them (arguments and results in the registers
//! of the original).

use crate::world::World;
use m68k::decode::Size;
use m68k::Cpu;

use crate::families::small::*;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::mem::{prog8, rd32, set_low_word, wr32, A5};
use crate::objects::Obj;
use crate::objects::pools;

/// Allocates in a pool; `None` (carry raised) if the pool is full.
fn alloc(c: &mut Cpu, b: &mut World, pool: &pools::Pool) -> Option<Obj> {
    pool.alloc_68k(c, b)
}

/// Flags of a `move`/`clr` of `v`: N and Z according to the value, V and C at zero.
fn moved(c: &mut Cpu, v: u32, size: Size) {
    c.flags_logic(v, size);
}

fn linked(c: &mut Cpu, b: &mut World, pool: &pools::Pool, kind: u8, link: u32) {
    let me = Obj(c.a[0]);
    linked_via(c, b, me, pool, kind, link);
    rts(c, b);
}

fn peff_16(c: &mut Cpu, b: &mut World) {
    linked(c, b, &pools::PEFF, 0x10, 0x50);
}

fn peff_33(c: &mut Cpu, b: &mut World) {
    linked(c, b, &pools::PEFF, 0x21, 0x50);
}

fn sobj_7(c: &mut Cpu, b: &mut World) {
    linked(c, b, &pools::SOBJ, 0x7, 0x46);
}

fn sobj_10(c: &mut Cpu, b: &mut World) {
    linked(c, b, &pools::SOBJ, 0xa, 0x46);
}

/// Native of `shot_18`: D0 = 1 if the shot is created.
/// Original glitch: 0x54872 (`always_zero`) returns D0 = 0 even when the shot is created.
fn shot_18_native(c: &mut Cpu, b: &mut World, variant: u8, always_zero: bool) {
    let me = Obj(c.a[0]);
    let created = shot_18(c, b, me, variant).is_some();
    c.d[0] = (created && !always_zero) as u32;
    moved(c, c.d[0], Size::Long);
    rts(c, b);
}

fn enemy_pair(c: &mut Cpu, b: &mut World) {
    let me = Obj(c.a[0]);
    enemy_pair_via(c, b, me);
    rts(c, b);
}

/// 0x792f8 and 0x71e3c: `+0x53` takes over `+0x56`.
fn copy_56_to_53(c: &mut Cpu, b: &mut World) {
    let me = Obj(c.a[0]);
    let v = me.u8(b, 0x56);
    me.set_u8(b, 0x53, v);
    moved(c, v as u32, Size::Byte);
    rts(c, b);
}

/// 0xa66c0: creates D5 + 1 ESHL shots of type 0x3e whose variant decreases from D5 (byte of table 0xa66e2
/// indexed by D2) to 0.
fn eshl_volley(c: &mut Cpu, b: &mut World) {
    let row = 0xa66e2u32.wrapping_add(c.d[2] as i16 as i32 as u32);
    c.d[5] = prog8(b, row) as u32;
    while let Some(o) = alloc(c, b, &pools::ESHL) {
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

fn roar_big(c: &mut Cpu, b: &mut World) {
    let me = Obj(c.a[0]);
    roar_via(c, b, me, true);
    rts(c, b);
}

fn roar_small(c: &mut Cpu, b: &mut World) {
    let me = Obj(c.a[0]);
    roar_via(c, b, me, false);
    rts(c, b);
}

/// 0xa036e: harpy, companion creature (ESHL 0x10): copies the variant, the position offset by ±0x2f in X
/// and by -0x78 in Z; half the creator's HP (`+0x60 / 4`) goes into the creator's `+0x58`.
fn harpy_companion(c: &mut Cpu, b: &mut World) {
    let me = Obj(c.a[0]);
    let _ = harpy_companion_via(c, b, me);
    rts(c, b);
}

/// 0xa0566: harpy, ESHL 0x11 projectile: D5 goes into the creator's `+0x42`, A2 and the creator are the
/// links `+0x50` and `+0x56` (longs); the creator resets `+0xa5` to zero.
fn harpy_shot(c: &mut Cpu, b: &mut World) {
    let (me, d5, a2) = (Obj(c.a[0]), c.d[5] as u8, c.a[2]);
    let _ = harpy_shot_via(c, b, me, d5, a2);
    rts(c, b);
}

/// `movem.l (addr)+,list`: loads the registers of the mask (D0-D7 then A0-A7); returns the next address.
fn movem_load(c: &mut Cpu, b: &mut World, mask: u16, mut addr: u32) -> u32 {
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
fn movem_store(c: &Cpu, b: &mut World, mask: u16, mut addr: u32) -> u32 {
    for bit in (0..16).filter(|bit| mask >> bit & 1 != 0) {
        wr32(b, addr, if bit < 8 { c.d[bit] } else { c.a[bit - 8] });
        addr += 4;
    }
    addr
}

/// 0x10158: four blocks of eight colors copied into the palette (GFX RAM) with the high bit
/// of each nibble raised (`ori.l #0xF000F000`).
fn palette_blocks_lit(c: &mut Cpu, b: &mut World) {
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
fn palette_blocks(c: &mut Cpu, b: &mut World) {
    let mut src = 0x10218;
    for dst in [0x91c2e0, 0x91c300, 0x91c360, 0x91c3e0] {
        src = movem_load(c, b, MASK_D0_D6_A3, src);
        c.a[1] = dst;
        movem_store(c, b, MASK_D0_D6_A3, dst);
    }
    c.a[2] = src;
    rts(c, b);
}

/// `palette_twelve_blocks` with the original's registers (the last block in D1-D6, A1, A2).
fn palette_blocks_copy(c: &mut Cpu, b: &mut World) {
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
pub(crate) fn palette_page_restore(c: &mut Cpu, b: &mut World) {
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
fn flash(c: &mut Cpu, b: &mut World, v: u16) {
    crate::palette::set_flash_color(b, v);
    moved(c, v as u32, Size::Word);
    rts(c, b);
}

fn flash_off(c: &mut Cpu, b: &mut World) {
    flash(c, b, LIGHTNING_END);
}

fn flash_a(c: &mut Cpu, b: &mut World) {
    flash(c, b, LIGHTNING_YELLOW);
}

fn flash_b(c: &mut Cpu, b: &mut World) {
    flash(c, b, LIGHTNING_GREEN);
}

fn flash_c(c: &mut Cpu, b: &mut World) {
    flash(c, b, LIGHTNING_BLUE);
}

/// `background_2_on`, with the original's registers: D0 and A1 of the last block (0x26d4), flags of the last `move.b`.
fn scroll_2_on(c: &mut Cpu, b: &mut World) {
    let end = background_2_on(b);
    c.d[0] = 0xffff;
    c.a[1] = end.destination;
    c.a[2] = end.indices;
    moved(c, 1, Size::Byte);
    rts(c, b);
}

fn scroll_1_on_with(c: &mut Cpu, b: &mut World, mode: u16) {
    background_1_in_mode(b, mode);
    moved(c, mode as u32, Size::Word);
    rts(c, b);
}

fn scroll_1_on(c: &mut Cpu, b: &mut World) {
    scroll_1_on_with(c, b, 0x18);
}

fn scroll_1_on_at_d0(c: &mut Cpu, b: &mut World) {
    background_1_lit_a(b, c.d[0] as u16);
    moved(c, 0x34, Size::Word);
    rts(c, b);
}

fn booted_set(c: &mut Cpu, b: &mut World) {
    startup_done(b, true);
    moved(c, 1, Size::Byte);
    rts(c, b);
}

fn booted_clear(c: &mut Cpu, b: &mut World) {
    startup_done(b, false);
    moved(c, 0, Size::Byte);
    rts(c, b);
}

fn game_vars_clear(c: &mut Cpu, b: &mut World) {
    c.d[0] = 0;
    zero_game_variables(b);
    c.a[0] = A5 + 0x6c;
    c.d[6] = 0xffff;
    moved(c, 0, Size::Long);
    rts(c, b);
}

/// `reset_fixed_screen`; A0 = the last unlinked object (0x1f184).
fn fixed_screen_reset(c: &mut Cpu, b: &mut World) {
    wr32(b, A5 - 0x70b0, 0);
    crate::tools::objects::lists::unlink_all_68k(c, b);
    crate::sprites::clear_lists(b);
    pools::SET.reset(b);
    pools::EFFCT.reset(b);
    rts(c, b);
}

const fn native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut World), outputs: u32) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs }
}

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

/// D0-D6 and A3.
const MASK_D0_D6_A3: u16 = 0x087f;

const CREATED: u32 = regs::a(1) | regs::CCR;
