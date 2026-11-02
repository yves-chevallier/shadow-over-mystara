//! Tools: the natives of `crate::families::effects_creators` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::families::effects_creators::*;
use crate::families::effects_creators::leftovers::{leftover, Regs};
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::mem::{prog16, prog32, rd16, rd32, rd8, set_low_word, wr16, wr32, A5};
use crate::objects::pools::{self, Pool};
use crate::objects::Obj;
use crate::families::effects_creators::leftovers::Pure;
use crate::regs::d as D;
use crate::mem::set_low_byte;

/// Declares the natives: `address "name" |c, b, me| body, exits;` (`me` = A0, the creator); each native ends with an
/// RTS.
macro_rules! creators {
    ($( $entry:literal $name:literal |$c:ident, $b:ident, $me:ident| $body:expr, $out:expr; )*) => {
        pub const NATIVES: &[Native] = &[ $(
            creator($entry, $name, |$c, $b| {
                let $me = Obj($c.a[0]);
                let _ = $me;
                let _ = $body;
                rts($c, $b)
            }, $out),
        )* ];
    };
}

/// `blow` with the original's registers (callers by address: `player/`): A0 = the creator, D0.b = variant, D1.b =
/// setting; A1, carry and flags like the original.
#[track_caller]
pub fn blow_68k(c: &mut Cpu, b: &mut Cps2Bus, creator: Obj, variant: u8, setting: u8) -> Option<Obj> {
    c.a[0] = creator.0;
    let r = attached_via(c, b, kind::BLOW, creator, variant, setting);
    crate::tools::diag::dead_regs(c, "effects", CREATED);
    r
}

/// `halo` with the original's registers (callers by address: `player/`): A1, carry and flags.
#[track_caller]
pub fn halo_68k(c: &mut Cpu, b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    c.a[0] = creator.0;
    let r = halo_via(c, b, creator);
    crate::tools::diag::dead_regs(c, "effects", CREATED);
    r
}

/// 0xa8a42: scene camera in mode 0x60 (`A5+0x51e`), `A5+0x4ff` cleared.
fn camera_mode_60(c: &mut Cpu, b: &mut Cps2Bus) {
    camera_in_mode_60(b);
    c.flags_logic(0x60, Size::Word);
}

const fn creator(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus), outputs: u32) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs }
}

/// Parameters read from the registers.
fn d0b(c: &Cpu) -> u8 {
    c.d[0] as u8
}

fn d1b(c: &Cpu) -> u8 {
    c.d[1] as u8
}

/// 0xaaa40: A1 to A4 are returned.
fn spark_at_native(c: &mut Cpu, b: &mut Cps2Bus, me: Obj) {
    let saved = [c.a[1], c.a[2], c.a[3], c.a[4]];
    let (other, variant, pos) = (c.a[1], d1b(c), (c.d[2] as u16, c.d[3] as u16, c.d[4] as u16));
    spark_at_via(c, b, me, other, variant, pos);
    [c.a[1], c.a[2], c.a[3], c.a[4]] = saved;
}

fn tile_block_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let (at, tile) = tile_block(b, c.a[1], c.d[0] as u16, c.d[1] as u16);
    c.a[1] = at;
    set_low_word(&mut c.d[0], tile);
}

creators! {
    0xae776 "create_interface" |c, b, me| interface(c, b), CREATED;
    0xaf346 "create_banner" |c, b, me| { let v = d0b(c); strip(c, b, v) }, CREATED;
    0xb1a7a "create_end_screen" |c, b, me| { let (v, s) = (d0b(c), d1b(c)); with_setting_via(c, b, kind::COUNTDOWN, v, s) }, CREATED;
    0xafe02 "create_vote" |c, b, me| { let (v, s) = (d0b(c), d1b(c)); with_setting_via(c, b, kind::VOTE, v, s) }, CREATED;
    0xab7d8 "create_sheaf_debris" |c, b, me| { let v = d0b(c); debris_via(c, b, me, v) }, CREATED;
    0xabec8 "create_effect_attack" |c, b, me| { let (v, s) = (d0b(c), d1b(c)); attached_via(c, b, kind::ATTACK, me, v, s) }, CREATED;
    0xb378a "create_hit" |c, b, me| { let (v, s) = (d0b(c), d1b(c)); attached_via(c, b, kind::BLOW, me, v, s) }, CREATED;
    0xab708 "create_halo" |c, b, me| halo_via(c, b, me), CREATED;
    0xb1352 "create_spark_placed" |c, b, me| { let v = d0b(c); posed_spark_via(c, b, me, v) }, CREATED;
    0xb141a "create_impact" |c, b, me| { let (vx, ax) = (c.d[2] as u16, c.d[3] as u16); impact(c, b, me, vx, ax) }, CREATED;
    0xb12c0 "create_pickup" |c, b, me| { let v = d0b(c); pickup_via(c, b, me, v) }, CREATED;
    0xb9c6a "create_ember" |c, b, me| linked_via(c, b, me, kind::WISP, Some(0)), CREATED;
    0xb9c88 "create_ember_b" |c, b, me| linked_via(c, b, me, kind::WISP, Some(1)), CREATED;
    0xb9aa0 "create_final" |c, b, me| linked_via(c, b, me, kind::FINALE, None), CREATED;
    0xb84ae "create_victory_screen" |c, b, me| linked_via(c, b, me, kind::VICTORY, None), CREATED;
    0xb85e2 "create_bubble_creator" |c, b, me| linked_via(c, b, me, kind::BUBBLE, None), CREATED;
    0xb9942 "create_rain" |c, b, me| spawn(c, b, kind::RAIN), CREATED;
    0xb7f14 "create_type_68" |c, b, me| spawn(c, b, kind::TYPE_68), CREATED;
    0xb8956 "create_timer" |c, b, me| { let v = d1b(c); timer(c, b, v) }, CREATED;
    0xba18a "create_drift" |c, b, me| drift_via(c, b, me), CREATED;
    0xb66e4 "create_type_53" |c, b, me| type_53(c, b), CREATED;
    0xb6a5c "create_launcher" |c, b, me| launcher_via(c, b, me), CREATED;
    0xb6c42 "create_shard_camera" |c, b, me| camera_shard_via(c, b), CREATED;
    0xb2ef6 "create_silhouette_reflection" |c, b, me| mirrored_pair_via(c, b, me), CREATED;
    0xb6ba8 "create_type_57" |c, b, me| { let (v, s) = (d0b(c), d1b(c)); type_57(c, b, me, v, s) }, CREATED;
    0xb8a0a "create_sound_fade" |c, b, me| spawn(c, b, kind::SOUND_FADE), CREATED;
    0xb68b0 "create_types_54" |c, b, me| type_54_trio(c, b), CREATED;
    0xae3ec "create_shimmer" |c, b, me| twinkle_via(c, b, me), CREATED;
    0xb6596 "create_shimmer_character" |c, b, me| shimmer_via(c, b, me), CREATED | D(0);
    0xb5730 "create_marker" |c, b, me| beacon_via(c, b, me), CREATED;
    0xb3c14 "create_thrown" |c, b, me| toss_via(c, b, me), CREATED;
    0xaba16 "create_shard_single" |c, b, me| flash_via(c, b, me), CREATED;
    0xaaa40 "create_spark_at" |c, b, me| spark_at_native(c, b, me), regs::CCR;
    0xb3df6 "create_scenery_0" |c, b, me| scenery_0_pair_via(c, b, me), CREATED;
    0xb3f40 "create_scenery_3" |c, b, me| scenery_burst_via(c, b, me, 3, 6, true), CREATED | D(1);
    0xb3f6a "create_scenery_4" |c, b, me| scenery_4_via(c, b, me), CREATED | D(0) | D(1);
    0xb3fb0 "create_scenery_5" |c, b, me| { let s = d1b(c); scenery_via(c, b, me, 5, Some(s)) }, CREATED;
    0xb3fd4 "create_scenery_6" |c, b, me| scenery_by_chance_via(c, b, me, 0xf, 6, None), CREATED | D(0);
    0xb4002 "create_scenery_7" |c, b, me| scenery_via(c, b, me, 7, Some(0)), CREATED;
    0xb4026 "create_scenery_8" |c, b, me| scenery_burst_via(c, b, me, 8, 9, false), CREATED | D(1);
    0xb4088 "create_scenery_10" |c, b, me| scenery_by_chance_via(c, b, me, 3, 10, Some(0)), CREATED | D(0);
    0xb4f44 "create_spell" |c, b, me| { let s = d1b(c); spell_via(c, b, me, s) }, CREATED;
    0xb4ef4 "create_spells" |c, b, me| spell_ring_via(c, b, me, 0), CREATED | D(1);
    0xb4f60 "create_spells_b" |c, b, me| spell_ring_via(c, b, me, 2), CREATED | D(1);
    0xb1764 "create_bench" |c, b, me| fish_school(c, b), CREATED | D(0) | D(4) | D(5);
    0xb17ba "create_swimmers" |c, b, me| fish_swimmers(c, b), CREATED | D(0) | D(1) | D(2) | D(4) | D(5);
    0xb7dec "create_columns" |c, b, me| pillars_via(c, b), CREATED | D(5) | D(6) | regs::a(2);
    0xabb70 "create_trails" |c, b, me| trails(c, b), CREATED | D(0) | D(4) | D(5) | D(6) | regs::a(2) | regs::a(4);
    0xa8a42 "camera_mode_60" |c, b, me| camera_mode_60(c, b), regs::CCR;
    0xa8af8 "create_scenery_list" |c, b, me| { let v = d1b(c); set_list_via(c, b, v) }, CREATED | D(0) | D(1) | regs::a(4);
    0xb3e4e "create_cries" |c, b, me| shouts_via(c, b, me, 1, 0xb3e86), CREATED | D(0) | D(1) | regs::a(2);
    0xb3ec6 "create_cries_b" |c, b, me| shouts_via(c, b, me, 2, 0xb3f00), CREATED | D(0) | D(1) | regs::a(2);
    0xb4bcc "create_rain_debris" |c, b, me| { let (v, s) = (d1b(c), c.d[2] as u8); debris_rain_via(c, b, me, v, s) }, CREATED;
    0xb4f1c "create_spells_c" |c, b, me| spell_ring_via(c, b, me, 4), CREATED | D(1);
    0xb53f4 "create_type_46" |c, b, me| type_46_trio(c, b, me), CREATED;
    0xb59a6 "create_type_48" |c, b, me| type_48(c, b, me, 1), CREATED;
    0xb59c8 "create_type_48_b" |c, b, me| type_48(c, b, me, 2), CREATED;
    0xb64b0 "create_projectile" |c, b, me| linked_via(c, b, me, kind::PROJECTILE, None), CREATED;
    0xb96c0 "create_shard_thrown" |c, b, me| { let v = d1b(c); thrown_shard(c, b, me, v) }, CREATED;
    0xb97a2 "create_shard_placed" |c, b, me| linked_via(c, b, me, kind::POSED_FLASH, None), CREATED;
    0xb97ba "create_shard_placed_b" |c, b, me| linked_via(c, b, me, kind::POSED_FLASH, Some(1)), CREATED;
    0xb230e "block_tiles" |c, b, me| tile_block_native(c, b), regs::d(0) | regs::a(1);
    0xa836c "create_shot_hit" |c, b, me| { let s = c.d[4] as u8; eshl_slash_via(c, b, me, s) }, CREATED;
    0xa817c "create_shot_column" |c, b, me| eshl_rise_via(c, b, me), CREATED | D(0);
}

/// Registers that the original leaves at this place and that nothing seems to reread: written like it, then
/// poisoned with `DDSOM_POISON=leftovers` to prove it before removing the write.
#[track_caller]
#[allow(dead_code)] // proof tool: used during a conversion, removed along with the writes proven dead
pub(crate) fn leftovers(c: &mut Cpu, mask: u32) {
    crate::tools::diag::dead_regs(c, "leftovers", mask);
}

/// The original's registers.
impl Regs for Cpu {
    #[track_caller]
    fn allocated(&mut self, b: &mut Cps2Bus, pool: &Pool) -> Option<Obj> {
        pool.alloc_68k(self, b)
    }
    #[track_caller]
    fn alloc_without_test(&mut self, b: &mut Cps2Bus, pool: &Pool) -> Obj {
        pool.alloc_unchecked_68k(self, b)
    }
    #[track_caller]
    fn alloc_or_rest(&mut self, b: &mut Cps2Bus, pool: &Pool) -> Result<Obj, Obj> {
        let a1 = pool.alloc_unchecked_68k(self, b);
        if self.sr & m68k::SR_C != 0 { Err(a1) } else { Ok(a1) }
    }
    fn flags(&mut self, v: u32, size: crate::ops::Size) {
        self.flags_logic(v, size.into());
    }
    fn comparison(&mut self, src: u32, dst: u32, size: crate::ops::Size) {
        crate::tools::ops::cmp(self, src, dst, size.into());
    }
    fn x(&mut self, bit: bool) {
        self.sr = if bit { self.sr | m68k::SR_X } else { self.sr & !m68k::SR_X };
    }
    fn addition(&mut self, src: u32, dst: u32, size: crate::ops::Size) -> u32 {
        crate::tools::ops::add(self, src, dst, size.into())
    }
    fn subtract(&mut self, src: u32, dst: u32, size: crate::ops::Size) -> u32 {
        crate::tools::ops::sub(self, src, dst, size.into())
    }
    fn d(&mut self, n: usize, v: u32) {
        self.d[n] = v;
    }
    fn d_word(&mut self, n: usize, v: u16) {
        set_low_word(&mut self.d[n], v);
    }
    fn d_byte(&mut self, n: usize, v: u8) {
        set_low_byte(&mut self.d[n], v);
    }
    fn a(&mut self, n: usize, v: u32) {
        self.a[n] = v;
    }
    fn carry_only(&mut self) {
        self.sr = self.sr & 0xff00 | m68k::SR_C;
    }
}

pub(crate) const CREATED: u32 = regs::a(1) | regs::CCR;
