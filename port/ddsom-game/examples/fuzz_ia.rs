//! Differential fuzz of the cores of the AI batch (`ai.rs`, `ai_aids.rs`, routines of `families::enemy_lib`): each Rust
//! core is compared to the original routine, run by the 68000 interpreter alone (no native), on states
//! drawn at random from a real game (an active enemy or boss serves as object A0; players, target, camera,
//! rank and pseudo-random draw perturbed). The memory (RAM, video, palettes...) and the core's result are compared
//! to what the original leaves in its registers (Z, D0, A1...). The work registers are not compared:
//! the cores no longer write them.
//!
//!   fuzz_ia <roms> <eeprom> <scenario> <warm-up frames> <rounds> [hexadecimal entry...]
//!
//! Variables: `DDSOM_GRAINE` (draw seed), `DDSOM_VERBOSE` (each mismatch).

use std::path::Path;

use ddsom_game::families::cx::Cx;
use ddsom_game::families::enemy_lib as lib;
use ddsom_game::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use ddsom_game::objects::{pools, Obj};
use ddsom_game::{ai, ai_aids};

type Cpu = m68k::Cpu;
type Bus = cps2::Cps2Bus;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn among<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u32) as usize]
    }
}

/// Fictitious return address of the original's call.
const RETURN: u32 = 0x00_fffe;

/// Runs the original routine `entry` (interpreter alone) until it returns.
fn original(c: &mut Cpu, b: &mut Bus, entry: u32) {
    let sp = c.a[7];
    c.a[7] = sp.wrapping_sub(4);
    wr32(b, c.a[7], RETURN);
    c.pc = entry;
    let mut n = 0u32;
    while !(c.pc == RETURN && c.a[7] == sp) {
        c.step(b);
        n += 1;
        assert!(n < 5_000_000, "{entry:06x}: the original does not return");
    }
}

/// An active object of an enemy or boss pool, if there is one.
fn actors(b: &mut Bus) -> Vec<u32> {
    let mut v = Vec::new();
    for pool in [&pools::ENEMY, &pools::BOSS] {
        for k in 0..pool.slots {
            let o = A5 + pool.first + k * pool.stride;
            if rd8(b, o) != 0 {
                v.push(o);
            }
        }
    }
    v
}

/// Perturbs the state around object `o`: players (presence, HP, state, aura, position), target, camera, rank, chance.
fn perturbs(c: &mut Cpu, b: &mut Bus, rng: &mut Rng, o: u32) {
    for r in 0..8 {
        c.d[r] = rng.next();
    }
    for r in 1..5 {
        c.a[r] = 0xff0000 | (rng.next() & 0xfffe);
    }
    c.a[0] = o;
    c.a[5] = A5;
    let (x, y) = (rd16(b, o + 8), rd16(b, o + 0x10));
    let mut alive = 0;
    for k in 0..4u32 {
        let p = A5 + 0x5de + 0x200 * k;
        let active = rng.below(4) != 0;
        alive += active as u8;
        wr8(b, p, if active { rng.among(&[1, 1, 1, 2]) } else { 0 });
        wr8(b, p + 1, rng.among(&[0, 0, 1, 2, 3]));
        wr8(b, p + 3, rng.below(12) as u8);
        wr8(b, p + 4, rng.among(&[2, 2, 2, 4, 6]));
        wr8(b, p + 6, rng.among(&[0, 12, 14, 0x34]));
        wr16(b, p + 0x62, if rng.below(5) == 0 { 0 } else { rng.below(0x200) as u16 });
        wr8(b, p + 0x182, (rng.below(5) == 0) as u8);
        wr8(b, p + 0x188, (rng.below(6) == 0) as u8);
        wr8(b, p + 0x66, (rng.below(6) == 0) as u8);
        wr8(b, p + 0x67, rng.below(3) as u8);
        wr8(b, p + 0x68, rng.below(4) as u8);
        wr8(b, p + 0x69, rng.below(4) as u8);
        wr16(b, p + 8, x.wrapping_add(rng.below(0x200) as u16).wrapping_sub(0x100));
        wr16(b, p + 0x10, y.wrapping_add(rng.below(0x80) as u16).wrapping_sub(0x40));
        wr16(b, p + 0xc, if rng.below(3) == 0 { rng.below(0x40) as u16 } else { 0 });
        wr8(b, p + 0x2c, rng.among(&[0, 0x20]));
    }
    wr8(b, A5 + 0x35, alive.min(rng.below(5) as u8));
    wr8(b, A5 + 0x36, (rng.below(4) == 0) as u8);
    wr8(b, A5 + 0x48, rng.below(8) as u8);
    wr8(b, A5 + 0x49, rng.below(8) as u8);
    wr8(b, A5 + 0x5a, rng.below(6) as u8);
    wr16(b, A5 + 0x7f92, rng.below(32) as u16);
    wr16(b, A5 + 0x1a, rng.next() as u16);
    for off in [0x506u32, 0x556, 0x5a6, 0x5ae] {
        wr16(b, A5 + off, x.wrapping_add(rng.below(0x300) as u16).wrapping_sub(0x200));
    }
    for off in [0x50au32, 0x55a, 0x5aa] {
        wr16(b, A5 + off, y.wrapping_add(rng.below(0x200) as u16).wrapping_sub(0x100));
    }
    // target: a player, nothing, or another actor
    let target = match rng.below(6) {
        0 => 0,
        1 => actors(b).first().copied().unwrap_or(0) as u16,
        _ => (A5 + 0x5de + 0x200 * rng.below(4)) as u16,
    };
    wr16(b, o + 0x50, target);
    wr8(b, o + 0x2c, rng.among(&[0, 0x20]));
    wr8(b, o + 0x2d, rng.below(32) as u8);
    wr16(b, o + 0x14, rng.next() as u16);
    wr16(b, o + 0xc, if rng.below(3) == 0 { rng.below(0x40) as u16 } else { 0 });
    if rng.below(3) == 0 {
        wr16(b, o + 8, rng.next() as u16 & 0xfff);
        wr16(b, o + 0x10, rng.next() as u16 & 0x7ff);
    }
    wr16(b, o + 0x7a, x.wrapping_add(rng.below(0x20) as u16).wrapping_sub(0x10));
    wr16(b, o + 0x7c, y.wrapping_add(rng.below(0x20) as u16).wrapping_sub(0x10));
    wr8(b, o + 0x99, rng.below(6) as u8);
    wr8(b, o + 0x9d, (rng.below(3) == 0) as u8);
    wr8(b, o + 0x98, rng.below(4) as u8);
    wr8(b, o + 0x3e, rng.below(8) as u8);
    wr8(b, o + 0xaa, rng.below(4) as u8);
    wr8(b, o + 0xab, rng.below(5) as u8);
    wr8(b, o + 0xac, (rng.below(4) * 32 + rng.below(16) * 2) as u8);
    wr16(b, o + 0x62, rng.below(0x400) as u16);
}

/// Compared result: the core's, and what the original leaves (read from its registers).
type Core = fn(&mut Cpu, &mut Bus, Obj) -> u32;
type Read = fn(&Cpu) -> u32;

fn z(c: &Cpu) -> u32 {
    (c.sr & m68k::SR_Z != 0) as u32
}
fn nz(c: &Cpu) -> u32 {
    (c.sr & m68k::SR_Z == 0) as u32
}
fn d0(c: &Cpu) -> u32 {
    c.d[0]
}
fn d5(c: &Cpu) -> u32 {
    c.d[5] as u16 as u32
}
fn nothing(_: &Cpu) -> u32 {
    0
}
/// The target found (A1, 0 if none) when Z is clear.
fn a1_when(c: &Cpu) -> u32 {
    if c.sr & m68k::SR_Z == 0 {
        c.a[1] & 0xffffff
    } else {
        0
    }
}
fn obj(o: Option<Obj>) -> u32 {
    o.map_or(0, |o| o.0 & 0xffffff)
}

struct Case {
    entry: u32,
    name: &'static str,
    /// Input registers set before the call (depending on the drawn state).
    inputs: fn(&mut Cpu, &mut Bus, &mut Rng),
    core: Core,
    read: Read,
}

fn none(_: &mut Cpu, _: &mut Bus, _: &mut Rng) {}

fn hitbox(c: &mut Cpu, _: &mut Bus, a: &mut Rng) {
    c.d[0] = c.d[0] & !0xffff | (a.below(0x80) as u16).wrapping_sub(0x40) as u32;
    c.d[1] = c.d[1] & !0xffff | a.below(0xa0) + 4;
    c.d[2] = c.d[2] & !0xffff | a.below(0x80) + 2;
}

fn criterion(c: &mut Cpu, _: &mut Bus, a: &mut Rng) {
    c.d[0] = a.among(&[0, 2, 4]);
}

fn point(c: &mut Cpu, b: &mut Bus, a: &mut Rng) {
    let o = c.a[0];
    c.d[1] = c.d[1] & !0xffff | rd16(b, o + 8).wrapping_add(a.below(0x200) as u16).wrapping_sub(0x100) as u32;
    c.d[2] = c.d[2] & !0xffff | rd16(b, o + 0x10).wrapping_add(a.below(0x200) as u16).wrapping_sub(0x100) as u32;
}

fn margin(c: &mut Cpu, _: &mut Bus, a: &mut Rng) {
    c.d[0] = c.d[0] & !0xffff | a.among(&[0x10, 0x20, 0x30, 0x40]);
}

fn band(c: &mut Cpu, _: &mut Bus, a: &mut Rng) {
    c.d[0] = c.d[0] & !0xffff | (a.below(0x100) as u16).wrapping_sub(0x80) as u32;
    c.d[1] = c.d[1] & !0xffff | 0x100 + a.below(0x100);
}

/// Tier table (A1): a known boss table (0x83... ), or a RAM zone.
fn steps(c: &mut Cpu, b: &mut Bus, a: &mut Rng) {
    let t = 0xffe000;
    for k in 0..0x400u32 {
        wr16(b, t + 2 * k, a.below(0x400) as u16);
    }
    c.a[1] = t;
}

fn attack(c: &mut Cpu, _: &mut Bus, a: &mut Rng) {
    c.d[0] = a.below(40);
}

fn random_inputs(c: &mut Cpu, _: &mut Bus, a: &mut Rng) {
    c.a[1] = A5 + 0x5de + 0x200 * a.below(4);
    c.d[0] = a.next() & 0xff;
}

fn player_a1(c: &mut Cpu, _: &mut Bus, a: &mut Rng) {
    c.a[1] = (A5 + 0x5de + 0x200 * a.below(4)) | 0xff00_0000;
}

fn parade(c: &mut Cpu, b: &mut Bus, a: &mut Rng) {
    // agresseur A0 = un joueur, victime A1 = l'objet
    let o = c.a[0];
    c.a[1] = o;
    c.a[0] = A5 + 0x5de + 0x200 * a.below(4);
    wr8(b, o + 0x67, a.below(3) as u8);
    wr8(b, o + 0x68, a.below(8) as u8);
    wr8(b, o + 0x69, a.below(8) as u8);
}

fn game_debris(c: &mut Cpu, _: &mut Bus, a: &mut Rng) {
    c.d[5] = a.below(16);
}

fn ground_debris(c: &mut Cpu, _: &mut Bus, a: &mut Rng) {
    c.d[0] = c.d[0] & !0xff | a.below(2);
}

fn game_animation(c: &mut Cpu, b: &mut Bus, a: &mut Rng) {
    // animation set: the one the object plays (address of the current entry, aligned on the start of the unknown set)
    // ; the skeleton's animation table is taken (0x1c74b6 plays the walk) and a master = the object itself
    let o = c.a[0];
    c.a[4] = 0x1c74b6;
    wr16(b, o + 0xb2, o as u16);
    wr8(b, o + 0x33, a.below(4) as u8);
    let mark = rd8(b, 0x1c74b6 + 3 + 16 * a.below(4));
    wr8(b, o + 0x3e, mark);
}

fn speeds(c: &mut Cpu, _: &mut Bus, a: &mut Rng) {
    c.d[2] = a.next();
    c.d[3] = a.next();
}

fn case() -> Vec<Case> {
    vec![
        Case { entry: 0x424e, name: "pick_target", inputs: none, core: |_, b, o| obj(ai::pick_target(b, o, ai::Rule::Strict)), read: a1_when },
        Case { entry: 0x43ae, name: "pick_target_any", inputs: none, core: |_, b, o| obj(ai::pick_target(b, o, ai::Rule::Lenient)), read: a1_when },
        Case {
            entry: 0x43bc,
            name: "target_by",
            inputs: criterion,
            core: |c, b, o| obj(ai::target_by(b, o, ai::Criterion::from_byte(c.d[0] as u8), ai::Rule::Lenient)),
            read: a1_when,
        },
        Case { entry: 0x44ee, name: "random_player", inputs: none, core: |_, b, o| obj(ai::random_player(b, o)), read: a1_when },
        Case { entry: 0x453c, name: "initial_hp_boss", inputs: none, core: |_, b, o| { ai::initial_hp(b, o, ai::HP_TABLE_BOSS); 0 }, read: nothing },
        Case { entry: 0x4544, name: "initial_hp", inputs: none, core: |_, b, o| { ai::initial_hp(b, o, ai::HP_TABLE_ENEMY); 0 }, read: nothing },
        Case { entry: 0xa4b4, name: "decision_a", inputs: none, core: |_, b, o| ai::decision_a(b, o) as u32, read: nz },
        Case { entry: 0xa4c2, name: "decision_b", inputs: none, core: |_, b, o| ai::decision_b(b, o) as u32, read: nz },
        Case { entry: 0xac3a, name: "pick_byte", inputs: none, core: |_, b, o| { ai::pick_byte(b, o); 0 }, read: nothing },
        Case { entry: 0xfe5a, name: "set_immunities", inputs: none, core: |_, b, o| { ai::set_immunities(b, o); 0 }, read: nothing },
        Case { entry: 0x1059e, name: "hp_rank", inputs: none, core: |_, b, o| { ai::hp_rank(b, o); 0 }, read: nothing },
        Case {
            entry: 0xb5e0,
            name: "target_in_front",
            inputs: hitbox,
            core: |c, b, o| {
                let hitbox = ai_aids::Hitbox { offset: c.d[0] as u16, half_width: c.d[1] as u16, half_depth: c.d[2] as u16 };
                ai::target_in_front(b, o, hitbox) as u32
            },
            read: d0,
        },
        Case { entry: 0x1050c, name: "facing_target", inputs: none, core: |_, b, o| ai::facing_target(b, o) as u32, read: z },
        Case { entry: 0x1052e, name: "target_faces_me", inputs: none, core: |_, b, o| ai::target_faces_me(b, o) as u32, read: d0 },
        Case { entry: 0x104e0, name: "back_to_target", inputs: none, core: |_, b, o| ai::back_to_target(b, o) as u32, read: nz },
        Case { entry: 0x1047a, name: "turn_decision", inputs: none, core: |_, b, o| ai::turn_decision(b, o) as u32, read: d0 },
        Case { entry: 0x10560, name: "moving_forward", inputs: none, core: |_, b, o| ai::moving_forward(b, o) as u32, read: z },
        Case { entry: 0xfbae, name: "keep_in_zone", inputs: none, core: |_, b, o| ai::keep_in_zone(b, o) as u32, read: d5 },
        Case { entry: 0xfbb0, name: "keep_on_screen", inputs: margin, core: |c, b, o| ai::keep_on_screen(b, o, c.d[0] as u16) as u32, read: d5 },
        Case { entry: 0xf0e8, name: "turn_toward", inputs: point, core: |c, b, o| { ai::turn_toward(b, o, (c.d[1] as u16, c.d[2] as u16)); 0 }, read: nothing },
        Case { entry: 0x10574, name: "drift_heading", inputs: none, core: |_, b, o| { ai::drift_heading(b, o); 0 }, read: nothing },
        Case { entry: 0xe76c, name: "approach_target", inputs: none, core: |_, b, o| { ai_aids::approach_target(b, o); 0 }, read: nothing },
        Case { entry: 0xe7fa, name: "choose_near_walk_point", inputs: none, core: |_, b, o| { ai_aids::choose_near_walk_point(b, o); 0 }, read: nothing },
        Case { entry: 0xe886, name: "choose_walk_point", inputs: none, core: |_, b, o| { ai_aids::choose_walk_point(b, o); 0 }, read: nothing },
        Case { entry: 0xe914, name: "walk_point_reached", inputs: none, core: |_, b, o| { ai_aids::walk_point_reached(b, o); 0 }, read: nothing },
        Case { entry: 0xeff2, name: "walk_near_point", inputs: none, core: |_, b, o| { ai_aids::walk_near_point(b, o); 0 }, read: nothing },
        Case { entry: 0xf084, name: "walk_far_point", inputs: none, core: |_, b, o| { ai_aids::walk_far_point(b, o); 0 }, read: nothing },
        Case { entry: 0x103e0, name: "animation_by_variant", inputs: game_animation, core: |c, b, o| { ai_aids::animation_by_variant(b, o, c.a[4]); 0 }, read: nothing },
        Case { entry: 0x10428, name: "animation_of_master", inputs: game_animation, core: |c, b, o| { ai_aids::animation_of_master(b, o, c.a[4]); 0 }, read: nothing },
        Case { entry: 0x10450, name: "animation_of_mark", inputs: game_animation, core: |c, b, o| { ai_aids::animation_of_mark(b, o, c.a[4]); 0 }, read: nothing },
        Case { entry: 0x10474, name: "players_alive", inputs: none, core: |_, b, _| ai_aids::players_alive(b) as u32, read: nz },
        Case {
            entry: 0x104a2,
            name: "outside_band",
            inputs: band,
            core: |c, b, o| ai_aids::outside_band(b, o, c.d[0] as u16, c.d[1] as u16) as u32,
            read: d0,
        },
        Case { entry: 0x105b6, name: "next_threshold", inputs: steps, core: |c, b, o| ai_aids::next_threshold(b, o, c.a[1]) as u32, read: d0 },
        Case {
            entry: 0xfc44,
            name: "attack_hits_a_player",
            inputs: attack,
            core: |c, b, o| obj(ai_aids::attack_hits_a_player(b, o, c.d[0] as u8)),
            read: |c| if c.d[0] != 0 { c.a[1] & 0xffffff } else { 0 },
        },
        Case {
            entry: 0xfd64,
            name: "seize_player",
            inputs: random_inputs,
            core: |c, b, o| { ai_aids::seize_player(b, o, Obj(c.a[1]), c.d[0] as u8); 0 },
            read: nothing,
        },
        Case { entry: 0xfd8c, name: "release_player", inputs: player_a1, core: |c, b, _| { ai_aids::release_player(b, Obj(c.a[1] & 0xffffff)); 0 }, read: nothing },
        Case { entry: 0xfda2, name: "hold_seized", inputs: none, core: |_, b, o| { ai_aids::hold_seized(b, o); 0 }, read: nothing },
        Case { entry: 0xff54, name: "countdown_then_state", inputs: none, core: |_, b, o| { ai_aids::countdown_then_state(b, o); 0 }, read: nothing },
        Case { entry: 0xfffc, name: "behaviour_by_kind", inputs: none, core: |_, b, o| { ai_aids::behaviour_by_kind(b, o); 0 }, read: nothing },
        Case { entry: 0x788c, name: "draw_6b", inputs: none, core: |_, b, o| { ai_aids::draw_6b(b, o); 0 }, read: nothing },
        Case { entry: 0x79f6, name: "draw_6e", inputs: none, core: |_, b, o| { ai_aids::draw_6e(b, o); 0 }, read: nothing },
        Case { entry: 0x79ce, name: "maybe_draw_6e", inputs: none, core: |_, b, o| ai_aids::maybe_draw_6e(b, o) as u32, read: d0 },
        Case {
            entry: 0xb048,
            name: "parries_a",
            inputs: parade,
            core: |c, b, _| ai_aids::parries(b, Obj(c.a[0]), Obj(c.a[1]), ai_aids::Parry::A) as u32,
            read: d0,
        },
        Case {
            entry: 0xb068,
            name: "parries_b",
            inputs: parade,
            core: |c, b, _| ai_aids::parries(b, Obj(c.a[0]), Obj(c.a[1]), ai_aids::Parry::B) as u32,
            read: d0,
        },
        Case { entry: 0xf118, name: "draw_84", inputs: none, core: |_, b, o| { ai_aids::draw_84(b, o); 0 }, read: nothing },
        Case { entry: 0x10112, name: "set_68", inputs: none, core: |_, b, o| { ai_aids::set_68(b, o); 0 }, read: nothing },
        Case { entry: 0x115e2, name: "push_players", inputs: none, core: |_, b, o| { ai_aids::push_players(b, o); 0 }, read: nothing },
        Case { entry: 0x1172e, name: "debris_set", inputs: game_debris, core: |c, b, o| { ai_aids::debris_set(b, o, c.d[5] as u16); 0 }, read: nothing },
        Case { entry: 0x11a36, name: "ground_debris", inputs: ground_debris, core: |c, b, o| { ai_aids::ground_debris(b, o, c.d[0] as u8 != 0); 0 }, read: nothing },
        Case { entry: 0xb648, name: "drop_once", inputs: none, core: |_, b, o| { ai_aids::drop_once(b, o); 0 }, read: nothing },
        Case { entry: 0xb780, name: "drop_loot", inputs: none, core: |_, b, o| { ai_aids::drop_loot(b, o); 0 }, read: nothing },
        Case {
            entry: 0xbad6,
            name: "drop_special",
            inputs: none,
            core: |c, b, o| ai_aids::drop_special(b, o).unwrap_or(c.d[3] as u16) as u32,
            read: |c| c.d[3] as u16 as u32,
        },
        Case { entry: 0xe2f6, name: "drop_treasure", inputs: none, core: |c, b, o| { let (d2, d5) = ai_aids::drop_treasure(b, o); d2 as u32 | (d5.unwrap_or(c.d[5]) & 0xffff) << 16 }, read: |c| c.d[2] as u16 as u32 | (c.d[5] & 0xffff) << 16 },
        Case { entry: 0x57c66, name: "in_right_half", inputs: none, core: |_, b, o| ddsom_game::families::creators::in_right_half(b, o) as u32, read: d0 },
        Case { entry: 0x5cdc6, name: "chance_10_16", inputs: none, core: |_, b, _| ddsom_game::families::creators::chance_10_16(b) as u32, read: d0 },
        Case { entry: 0x19eb0, name: "popups_effacer_tout", inputs: none, core: |_, b, _| { ddsom_game::popups::clear_all(b); 0 }, read: nothing },
        Case { entry: 0x19e20, name: "popups_animer", inputs: none, core: |_, b, _| { ddsom_game::popups::animer(b); 0 }, read: nothing },
        Case { entry: 0xd5f30, name: "set_14", inputs: none, core: |_, b, o| { ddsom_game::families::creators::set_14(b, o); 0 }, read: nothing },
        Case { entry: 0xd6952, name: "set_15_b", inputs: none, core: |_, b, o| { ddsom_game::families::creators::set_15_b(b, o); 0 }, read: nothing },
        Case { entry: 0xd5ef8, name: "attach_life_bar", inputs: none, core: |_, b, o| { ddsom_game::families::set_residue::attach_life_bar(b, o); 0 }, read: nothing },
        Case {
            entry: 0x103ba,
            name: "flash_and_shake",
            inputs: speeds,
            core: |c, b, o| { ai_aids::flash_and_shake(b, o, c.d[2] as u16, c.d[3] as u16); 0 },
            read: nothing,
        },
        Case {
            entry: 0xa09ca,
            name: "lib_eshl_12",
            inputs: none,
            core: |c, b, _| {
                let mut x = Cx::new(c, b);
                let (o, carry) = lib::eshl_12(&mut x);
                (o.is_some() as u32) << 1 | carry as u32
            },
            read: |c| ((c.a[1] != 0) as u32) << 1 | (c.sr & m68k::SR_C != 0) as u32,
        },
        Case { entry: 0xa0ae0, name: "lib_eshl_13", inputs: none, core: |c, b, _| { let t = c.a[2]; lib::eshl_13(&mut Cx::new(c, b), t); 0 }, read: nothing },
        Case { entry: 0xa114a, name: "lib_eshl_16", inputs: ground_debris, core: |c, b, _| { let v = c.d[0] as u8; lib::eshl_16(&mut Cx::new(c, b), v); 0 }, read: nothing },
        Case { entry: 0x9f346, name: "lib_eshl_0a", inputs: none, core: |c, b, _| { lib::eshl_0a(&mut Cx::new(c, b)); 0 }, read: nothing },
        Case { entry: 0xa2d22, name: "lib_eshl_burst_26", inputs: none, core: |c, b, _| { lib::eshl_burst_26(&mut Cx::new(c, b)); 0 }, read: nothing },
        Case { entry: 0xffda, name: "lib_roll_ffda", inputs: none, core: |c, b, _| lib::roll_ffda(&mut Cx::new(c, b)) as u32, read: nz },
        Case { entry: 0xad7c, name: "lib_roll_78", inputs: none, core: |c, b, _| lib::roll_78(&mut Cx::new(c, b)) as u32, read: d0 },
        Case { entry: 0x10298, name: "lib_palette_script", inputs: ground_debris, core: |c, b, _| { let i = c.d[0] as u16; lib::palette_script(&mut Cx::new(c, b), i); 0 }, read: nothing },
    ]
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 6 {
        eprintln!("usage: fuzz_ia <roms> <eeprom> <scenario> <warm-up frames> <rounds> [entries...]");
        std::process::exit(2);
    }
    let (roms, eeprom, scenario) = (&a[1], &a[2], &a[3]);
    let warmup: u32 = a[4].parse().unwrap();
    let tours: u32 = a[5].parse().unwrap();
    let filter: Vec<u32> = a[6..].iter().map(|e| u32::from_str_radix(e.trim_start_matches("0x"), 16).unwrap()).collect();
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    for f in 0..warmup as usize {
        script.apply(f, &mut m);
        m.run_frame();
    }
    let seed: u64 = std::env::var("DDSOM_GRAINE").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut rng = Rng(0x2545_f491_4f6c_dd1d ^ seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let (c0, mut b0) = (m.cpu.clone(), m.bus.clone());
    b0.irq_horizon = u64::MAX;
    let objects = actors(&mut b0);
    if objects.is_empty() {
        eprintln!("no active enemy or boss at frame {warmup}");
        std::process::exit(2);
    }
    let verbose = std::env::var("DDSOM_VERBOSE").is_ok();
    let mut bad = 0;
    for case in case() {
        if !filter.is_empty() && !filter.contains(&case.entry) {
            continue;
        }
        let (mut gaps, mut first, mut skipped) = (0, None, 0);
        for _ in 0..tours {
            let (mut c, mut b) = (c0.clone(), b0.clone());
            let o = rng.among(&objects);
            perturbs(&mut c, &mut b, &mut rng, o);
            (case.inputs)(&mut c, &mut b, &mut rng);
            let (mut c2, mut b2) = (c.clone(), b.clone());
            let expected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                original(&mut c2, &mut b2, case.entry);
                (case.read)(&c2)
            }));
            let Ok(expected) = expected else {
                skipped += 1;
                continue;
            };
            let a0 = Obj(c.a[0]);
            let got = (case.core)(&mut c, &mut b, a0);
            let mut diffs = Vec::new();
            if got != expected {
                diffs.push(format!("result {got:x} instead of {expected:x}"));
            }
            let sp = c2.a[7] & 0xff_ffff;
            ddsom_game::mem::diff_bus(&b, &b2, sp.saturating_sub(0x400)..sp + 4, &mut diffs);
            if !diffs.is_empty() {
                gaps += 1;
                let msg = format!("A0={:06x} : {}", a0.0, diffs.join(", "));
                if verbose {
                    eprintln!("[{}] {msg}", case.name);
                }
                first.get_or_insert(msg);
            }
        }
        println!("{:06x} {:24} {:6} rounds {:6} mismatches {:4} skipped {}", case.entry, case.name, tours, gaps, skipped, first.as_deref().unwrap_or(""));
        bad += gaps;
    }
    println!("{}", if bad == 0 { "FUZZ OK" } else { "FUZZ MISMATCHES" });
}
