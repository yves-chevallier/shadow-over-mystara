//! Tools: the natives of `crate::player::init` for the original code that calls them (arguments and results in the registers
//! of the original).

use crate::world::World;
use m68k::Cpu;

use crate::player::init::*;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::player::{Ctx, Player};

/// 0x3552 for the tools (the game calls `init_weapon_stats`): registers left like the original (A4 = character
/// table, D0 = last weapon number read, doubled if it is in the table).
fn weapon_stats_native(c: &mut Cpu, b: &mut World) {
    let p = Player(c.a[0]);
    let mut g = Ctx::new(b);
    init_weapon_stats(&mut g, p);
    let table = table_of(&mut g, p, tables::WEAPON_STATS);
    let w = p.u8(&mut g, Player::WEAPON_B);
    c.a[4] = table;
    c.d[0] = if w <= 0x27 { (2 * w) as u32 } else { w as u32 };
    rts(c, b);
}

pub(crate) const NATIVES: &[Native] = &[Native {
    entry: 0x3552,
    name: "player_weapon_stats",
    run: crate::Body::WithCpu(weapon_stats_native),
    touches_sound: false,
    outputs: regs::d(0) | regs::a(4),
}];
