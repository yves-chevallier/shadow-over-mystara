//! Tools: the natives of `crate::player::spawn` for the original code that calls them (arguments and results in the registers
//! of the original).


use crate::player::spawn::*;
use crate::tools::frames::*;
use crate::Native;
use crate::player::{Ctx, Player};

/// Declares the native of a creator at its original address; `$body` reads its parameters in `$d` and `$a`.
macro_rules! creator {
    ($name:ident, $entry:expr, $label:expr, |$d:ident, $a:ident, $g:ident, $p:ident| $body:expr) => {
        pub(crate) const $name: Native = Native {
            entry: $entry,
            name: $label,
            run: crate::Body::WithCpu(|c, b| run(c, b, |$d, $a, $g, $p| { let _ = ($d, $a); $body })),
            touches_sound: false,
            outputs: 0,
        };
    };
}

/// Runs a creator for the tools (`fuzz_player`, `DDSOM_CHECK`; the game calls the Rust functions): A0 = the player,
/// `f` reads its parameters in the data and address registers `d`, `a`; the leftovers D2-D5 pass through the bench,
/// the other registers the original leaves (A1, flags) are not reproduced.
fn run(c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, f: impl FnOnce(&[u32; 8], &[u32; 8], &mut Ctx, Player)) {
    let (d, a, p) = (c.d, c.a, Player(c.a[0]));
    super::on_bench(c, b, |g| f(&d, &a, g, p));
    rts(c, b);
}

// ---- natives --------------------------------------------------------------------------------------

creator!(SLASH, 0x4a99a, "spawn_slash", |d, a, g, p| {
    let (d0, d1) = (d[0] as u8, d[1] as u8);
    slash(g, p, if d0 == 0 { Some(d1) } else { None })
});
creator!(BLOOD, 0x4a860, "spawn_blood", |d, a, g, p| blood(g, p));
creator!(CURE, 0x4ab48, "spawn_cure", |d, a, g, p| {
    let n = d[0] as u8;
    cure(g, p, n)
});
creator!(POTION, 0x4ab64, "spawn_potion", |d, a, g, p| potion(g, p));
creator!(HOLY_LIGHT, 0x4aeb2, "spawn_holy_light", |d, a, g, p| holy_light(g, p));
creator!(BEAM, 0x4b5ac, "spawn_beam", |d, a, g, p| {
    let aimed = d[0] as u8 != 0;
    beam(g, p, aimed)
});
creator!(BLESSING, 0x4c19a, "spawn_blessing", |d, a, g, p| {
    let v = d[0] as u8;
    blessing(g, p, v)
});
creator!(BANISH, 0x4c26c, "spawn_banish", |d, a, g, p| {
    let foe = (d[0] as u8 != 0).then_some(a[2]);
    banish(g, p, foe)
});
creator!(WEAPON_TRAIL, 0x4c58e, "spawn_weapon_trail", |d, a, g, p| {
    let v = d[0] as u8;
    weapon_trail(g, p, v)
});
creator!(SHOWER, 0x4dcda, "spawn_shower", |d, a, g, p| {
    let side = d[0] as u8;
    shower(g, p, side)
});
creator!(AURA, 0x4e6a8, "spawn_aura", |d, a, g, p| {
    let v = d[0] as u8;
    aura(g, p, v)
});
creator!(LIGHTNING_TARGET, 0x4f004, "spawn_lightning_target", |d, a, g, p| {
    let v = d[0] as u8;
    lightning_target(g, p, v);
});
creator!(SWARM, 0x4f272, "spawn_swarm", |d, a, g, p| {
    let count = (d[0] as u16 as u32) + 1;
    swarm(g, p, count)
});
creator!(LEAP, 0x4fbde, "spawn_leap", |d, a, g, p| {
    let v = d[0] as u8;
    leap(g, p, v)
});
creator!(SWAP, 0x4fd2c, "spawn_swap", |d, a, g, p| {
    let v = d[0] as u8;
    swap(g, p, v)
});
creator!(DASH, 0x4fe18, "spawn_dash", |d, a, g, p| dash(g, p));
creator!(WALL_TARGET, 0x50324, "spawn_wall_target", |d, a, g, p| {
    wall_target(g, p);
});
creator!(LIGHTNING, 0x4cdd0, "spawn_lightning", |d, a, g, p| {
    let target = a[2];
    lightning(g, p, target)
});
creator!(RING, 0x4e2f4, "spawn_ring", |d, a, g, p| {
    let v = d[0] as u8;
    ring(g, p, v)
});
creator!(BEAM_CAST, 0x4b196, "spawn_beam_cast", |d, a, g, p| {
    let v = d[0] as u8;
    beam_cast(g, p, v)
});
creator!(STORM, 0x4ec66, "spawn_storm", |d, a, g, p| {
    let v = d[0] as u8;
    storm(g, p, v)
});
creator!(METEORS, 0x5012e, "spawn_meteors", |d, a, g, p| meteors(g, p));
creator!(ICE_COLUMNS, 0x4bb68, "spawn_ice_columns", |d, a, g, p| ice_columns(g, p));
creator!(ICE_MIST, 0x4bbbe, "spawn_ice_mist", |d, a, g, p| ice_mist(g, p));
creator!(ICE_SHARDS, 0x4bbf6, "spawn_ice_shards", |d, a, g, p| ice_shards(g, p));
creator!(ICE_CROWN, 0x4bc42, "spawn_ice_crown", |d, a, g, p| ice_crown(g, p));
creator!(FIREBALL, 0x54522, "spawn_fireball", |d, a, g, p| fireball(g, p));
creator!(ICE_GROUND, 0x52b54, "spawn_ice_ground", |d, a, g, p| {
    ice_ground(g, p);
});
creator!(THROWN_A, 0x51d02, "spawn_thrown_a", |d, a, g, p| {
    thrown(g, p, Thrown::A);
});
creator!(THROWN_B, 0x51d54, "spawn_thrown_b", |d, a, g, p| {
    thrown(g, p, Thrown::B);
});
creator!(THROWN_C, 0x51f6a, "spawn_thrown_c", |d, a, g, p| {
    thrown(g, p, Thrown::C);
});
creator!(THROWN_D, 0x51fb6, "spawn_thrown_d", |d, a, g, p| {
    thrown(g, p, Thrown::D);
});
creator!(THROWN_E, 0x52002, "spawn_thrown_e", |d, a, g, p| {
    thrown(g, p, Thrown::E);
});
creator!(THROWN_F, 0x52204, "spawn_thrown_f", |d, a, g, p| {
    thrown(g, p, Thrown::F);
});
creator!(THROWN_G, 0x5308c, "spawn_thrown_g", |d, a, g, p| {
    thrown(g, p, Thrown::G);
});
creator!(THROWN_H, 0x52394, "spawn_thrown_h", |d, a, g, p| {
    thrown(g, p, Thrown::H);
});
creator!(KNIFE, 0x54426, "spawn_knife", |d, a, g, p| {
    knife(g, p);
});
creator!(SHADOWS, 0x52e20, "spawn_shadows", |d, a, g, p| shadows(g, p));
creator!(FLARE, 0x4cd8e, "spawn_flare", |d, a, g, p| flare(g, p));
creator!(SUMMON, 0x4ccac, "spawn_summon", |d, a, g, p| summon(g, p));
creator!(PORTAL, 0x4d3c6, "spawn_portal", |d, a, g, p| {
    let v = d[0] as u8;
    portal(g, p, v)
});
creator!(WITCH, 0x53918, "spawn_witch", |d, a, g, p| witch(g, p));
creator!(CURSED_WEAPON, 0x4fefe, "spawn_cursed_weapon", |d, a, g, p| cursed_weapon(g, p));
creator!(DYING_SHARDS, 0x5009e, "spawn_dying_shards", |d, a, g, p| dying_shards(g, p));
creator!(STORM_CLOUD, 0x51aa4, "spawn_storm_cloud", |d, a, g, p| storm_cloud(g, p));
creator!(PORTAL_GLOW, 0x51872, "spawn_portal_glow", |d, a, g, p| portal_glow(g, p));
creator!(PORTAL_SPARKS, 0x4cfee, "spawn_portal_sparks", |d, a, g, p| portal_sparks(g, p));
creator!(PORTAL_BURST, 0x4d064, "spawn_portal_burst", |d, a, g, p| {
    let n = d[0] as u8;
    portal_burst(g, p, n);
});
creator!(RAIN, 0x54274, "spawn_rain", |d, a, g, p| rain(g, p));
creator!(MOON, 0x4e4de, "spawn_moon", |d, a, g, p| {
    let v = d[0] as u8;
    moon(g, p, v)
});
creator!(MOON_B, 0x4e782, "spawn_moon_b", |d, a, g, p| moon_b(g, p));
creator!(STORM_SPELL, 0x4d020, "spawn_storm_spell", |d, a, g, p| storm_spell(g, p));
creator!(RAIN_GLOW, 0x4d660, "spawn_rain_glow", |d, a, g, p| {
    let v = d[0] as u8;
    rain_glow(g, p, v)
});
creator!(WALL_SPARK, 0x4d2c0, "spawn_wall_spark", |d, a, g, p| {
    let v = (d[0] as u8 != 0) as u8;
    wall_spark(g, p, v)
});
creator!(WAVE_BOLT, 0x4dfe2, "spawn_wave_bolt", |d, a, g, p| {
    let columns = d[0] as u8 != 0;
    wave_bolt(g, p, columns)
});
creator!(WITCH_A, 0x53954, "spawn_witch_a", |d, a, g, p| witch_strike(g, p, 1));
creator!(WITCH_B, 0x539e4, "spawn_witch_b", |d, a, g, p| witch_strike(g, p, 4));
creator!(ROD_FLAME_SHOT, 0x5336a, "spawn_rod_flame", |d, a, g, p| rod_shot(g, p, RodShot::Flame));
creator!(ROD_BALL_SHOT, 0x53478, "spawn_rod_ball", |d, a, g, p| rod_shot(g, p, RodShot::Ball));
creator!(ROD_STAR_SHOT, 0x535f2, "spawn_rod_star", |d, a, g, p| rod_shot(g, p, RodShot::Star));
