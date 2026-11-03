//! Player shots (PSHL) against enemies: contact behaviours (ROM table 0x267c4).

use super::*;
use crate::machine::Machine;
use crate::player::tables::{player_data, Read};

// ---------------------------------------------------------------------------------------------
// Player shots against enemies

/// 0x265c4: each visible player shot is confronted with each visible enemy.
pub(super) fn player_shots_hit_enemies(w: &mut World) {
    cov::hit("player_shots_hit_enemies");
    if List::PlayerShots.len(w) == 0 || List::Enemies.len(w) == 0 {
        return;
    }
    scan(w, List::PlayerShots, scratch::SHOTS_OUTER, |w, shot| {
        if !w.is_active(shot) || shot.u8(w.b, f::ATTACK_BOX) == 0 || shot.u8(w.b, f::BOXES_OFF) != 0 {
            return Flow::Next;
        }
        let atk = HitBox(rom::SHOT_ATTACK + 8 * shot.u8(w.b, f::ATTACK_BOX) as u32);
        scan(w, List::Enemies, scratch::SHOTS_INNER, |w, e| {
            // the shot may have been consumed by the previous enemy
            if !w.is_active(shot) {
                return Flow::Stop;
            }
            if w.is_active(e) && e.u8(w.b, f::BODY_BOX) != 0 && e.u8(w.b, f::BOXES_OFF) == 0 && e.u16(w.b, f::HP) != 0 {
                let body = object_box(w, rom::BODY, e, f::BODY_BOX);
                if overlap3(w, shot, atk, e, body) {
                    shot_hits_enemy(w, shot, e);
                }
            }
            Flow::Next
        });
        Flow::Next
    });
}

/// 0x266fe: a player shot hits an enemy. The shot's behaviour (according to its `KIND`) decides whether
/// the hit lands; if so, the victim records the aggressor, loses HP and the displayed
/// HP counter is updated.
pub(super) fn shot_hits_enemy(w: &mut World, shot: Obj, e: Obj) {
    cov::hit("shot_hits_enemy");
    if e.u8(w.b, f::HITSTOP) != 0 {
        return;
    }
    let kind = shot.u8(w.b, f::KIND);
    if kind == 0x18 {
        shot_kind_24(w, shot, e);
        return;
    }
    let owner = Obj::from_word(shot.u16(w.b, f::OWNER));
    let hit = match kind {
        0 | 13 => shot_plain(w, shot, e),
        1 => shot_launch(w, shot, e, 23, Launch::Fixed),
        2 => shot_launch(w, shot, e, 25, Launch::ShotFlip),
        3 => shot_knockdown(w, shot, e, owner),
        4 => shot_stagger(w, shot, e),
        5 | 22 => shot_combo(w, shot, e, owner, 2, Combo::Random(0x60, true)),
        6 => shot_spark(w, shot, e, owner),
        7 => shot_combo(w, shot, e, owner, 3, Combo::OwnerFlip),
        8 => shot_slam(w, shot, e, owner, true),
        9 => shot_boost_launch(w, shot, e, 24, Launch::Fixed),
        10 => shot_boost_launch(w, shot, e, 26, Launch::ShotFlip),
        11 => shot_mage(w, shot, e, owner),
        12 => shot_slam(w, shot, e, owner, false),
        14 => shot_simple(w, shot, e, owner, 4, 0x70, None, 5),
        15 => shot_simple(w, shot, e, owner, 2, 0x60, Some(()), 4),
        16 => shot_simple(w, shot, e, owner, 3, 0x74, None, 6),
        17 => shot_combo(w, shot, e, owner, 13, Combo::Random(0x88, false)),
        18 => shot_combo(w, shot, e, owner, 10, Combo::VictimFlip),
        19 => shot_lightning(w, shot, e, owner),
        20 => shot_sweep(w, shot, e, owner),
        21 => shot_push(w, shot, e),
        23 => shot_execute(w, shot, e, owner),
        // the player shots have the kinds 0 to 0x18 (their handlers, `families/player/pshl*`): the table 0x267c4
        // has no other entry (`docs/code-mort.md`, "Sortie du Cpu, partie 6")
        _ => unreachable!("player shot of kind {kind:#x}: no PSHL kind beyond 0x18"),
    };
    trace!("shot {:06x} type {kind} on {:06x}: {hit}", shot.0, e.0);
    if !hit {
        return;
    }
    let owner = Obj::from_word(shot.u16(w.b, f::OWNER));
    let hp = e.u16(w.b, f::HP);
    e.set_u16(w.b, f::HP_PREV, hp);
    let var = owner.u8(w.b, f::VARIANT);
    e.set_u8(w.b, f::HIT_BY_VARIANT, var);
    let k = owner.u8(w.b, f::KIND);
    e.set_u8(w.b, f::HIT_BY_KIND, k);
    e.set_u8(w.b, f::HITSTOP_VICTIM, 8);
    e.set_u16(w.b, f::LINK, shot.word());
    shot.set_u16(w.b, f::LINK, e.word());
    w.set_bit(e, f::HIT_FLAGS, 1);
    w.set_bit(shot, f::HIT_FLAGS, 1);
    let st = e.u32(w.b, f::STATE);
    e.set_u32(w.b, f::PREV_STATE, st);
    if kind == 0x17 {
        return;
    }
    let mut dmg = shot_damage(w, shot, e);
    // 0x26774: the original tests `+0x166` of the shot itself (A0), not that of the launcher
    if shot.u8(w.b, f::GEAR_166) == 4 {
        dmg = dmg.wrapping_add(5);
    }
    enemy_loses_hp(w, e, dmg);
    if e.u16(w.b, f::HP) == 0 && e.u8(w.b, f::DEATH_CREDIT) != 0 {
        owner.set_u8(w.b, f::KILLS_FLAG, 1);
        let v = e.u8(w.b, f::DEATH_VALUE);
        ram::KILL_FLAG.set(w.b, v);
    }
    kill_small_enemy(w, shot, e);
    show_enemy_hp(w, owner, e);
}

/// 0x26828: certain shots finish off small enemies in one hit (class 2, types 0 or 0x13).
pub(super) fn kill_small_enemy(w: &mut World, shot: Obj, e: Obj) {
    cov::hit("kill_small_enemy");
    let t = shot.u8(w.b, f::KIND) as u32;
    if player_data().small_kill_shots.at8(w.b, t) == 0 || e.u8(w.b, f::CLASS) != 2 {
        return;
    }
    let k = e.u8(w.b, f::KIND);
    if k == 0 || k == 0x13 {
        e.set_u16(w.b, f::HP, 0);
    }
}

// --- shot behaviours (ROM table 0x267c4) -----------------------------------------------------

/// Orientation given to the victim of a shot that knocks it back.
#[derive(Clone, Copy)]
pub(super) enum Launch {
    /// 0, or 0x20 if the shot is not of type 6 (field `FACING_AUX`).
    Fixed,
    /// Orientation of the shot.
    ShotFlip,
}

/// Is the victim immune to this behaviour (long `+0x94`, one bit per behaviour)?
pub(super) fn immune(w: &mut World, e: Obj, bit: u32) -> bool {
    e.u32(w.b, f::IMMUNITY) >> bit & 1 != 0
}

/// Victim already on the ground (state 4 with reaction 0x60 or 0x88): certain shots do not relaunch it.
pub(super) fn already_down(w: &mut World, e: Obj) -> bool {
    e.u8(w.b, f::STATE) == 4 && matches!(e.u8(w.b, f::REACTION), 0x60 | 0x88)
}

/// Increases the shot's power (bytes A and B) by a fraction of itself.
pub(super) fn boost_power(w: &mut World, shot: Obj, shift: u32) {
    let (a, b) = (shot.u8(w.b, f::POWER_A), shot.u8(w.b, f::POWER_B));
    w.add8(shot, f::POWER_A, a >> shift);
    w.add8(shot, f::POWER_B, b >> shift);
}

/// Shot power bonus according to the launcher's equipment, starting from the initial powers.
#[derive(Clone, Copy)]
pub(super) struct OwnerBonus {
    /// Equipment item (`+0x10e`) 0x20: +25%, 0x21: +50%.
    gear: bool,
    /// Ring (`+0x166` = 4): +25%.
    ring: bool,
    /// Fixed bonus when `+0x165` equals the value: (value, addition).
    flat: Option<(u8, u8)>,
}

pub(super) fn owner_bonus(w: &mut World, shot: Obj, owner: Obj, bonus: OwnerBonus) {
    let (a, b) = (shot.u8(w.b, f::POWER_A), shot.u8(w.b, f::POWER_B));
    if bonus.gear {
        let g = owner.u8(w.b, f::WEAPON_10E);
        for (val, sh) in [(0x20u8, 2u32), (0x21, 1)] {
            if g == val {
                w.add8(shot, f::POWER_A, a >> sh);
                w.add8(shot, f::POWER_B, b >> sh);
            }
        }
    }
    if bonus.ring && owner.u8(w.b, f::GEAR_166) == 4 {
        w.add8(shot, f::POWER_A, a >> 2);
        w.add8(shot, f::POWER_B, b >> 2);
    }
    owner_flat_bonus(w, shot, owner, bonus.flat);
}

pub(super) fn owner_flat_bonus(w: &mut World, shot: Obj, owner: Obj, flat: Option<(u8, u8)>) {
    if let Some((val, add)) = flat {
        if owner.u8(w.b, f::GEAR_165) == val {
            w.add8(shot, f::POWER_A, add);
            w.add8(shot, f::POWER_B, add);
        }
    }
}

/// Launcher's counter of hits landed (`+0x190`, word).
pub(super) fn owner_hits(w: &mut World, owner: Obj, n: u16) {
    w.add16(owner, f::HITS_LANDED, n);
}

/// Common reaction of the shots that knock back (0x268ec / 0x269a0): the victim goes into reaction
/// 0x4c (or 0x38 on the ground, 0 if it is down), spark of type 1.
pub(super) fn launch(w: &mut World, shot: Obj, e: Obj, face: Launch) -> bool {
    cov::hit("launch");
    e.set_u32(w.b, f::STATE, state::HIT);
    e.set_u8(w.b, f::FLAGS, 1);
    e.set_u8(w.b, f::HITSTOP_VICTIM, 8);
    if matches!(face, Launch::ShotFlip) {
        let fl = shot.u8(w.b, f::FLIP);
        e.set_u8(w.b, f::REACTION_FLIP, fl);
    }
    e.set_u8(w.b, f::REACTION, 0x4c);
    if e.u8(w.b, f::FLIES) == 0 {
        e.set_u8(w.b, f::REACTION, 0x38);
        if (e.u16(w.b, f::Z) as i16) <= 0 {
            e.set_u8(w.b, f::REACTION, 0);
        }
    }
    if matches!(face, Launch::Fixed) {
        e.set_u8(w.b, f::REACTION_FLIP, 0);
        if shot.u8(w.b, f::FACING_AUX) != 6 {
            e.set_u8(w.b, f::REACTION_FLIP, 0x20);
        }
    }
    w.spark(Spark::Linked, shot, e, 1);
    true
}

/// Types 1 and 2.
pub(super) fn shot_launch(w: &mut World, shot: Obj, e: Obj, bit: u32, face: Launch) -> bool {
    cov::hit("shot_launch");
    !immune(w, e, bit) && launch(w, shot, e, face)
}

/// Types 9 and 10: like 1 and 2, but against a big enemy (boss of type 5, 8 or 0x12, enemy of
/// type 8) the shot's power increases by half.
pub(super) fn shot_boost_launch(w: &mut World, shot: Obj, e: Obj, bit: u32, face: Launch) -> bool {
    cov::hit("shot_boost_launch");
    if immune(w, e, bit) {
        return false;
    }
    let k = e.u8(w.b, f::KIND);
    let big = if e.u8(w.b, f::CLASS) == 4 { matches!(k, 5 | 8 | 0x12) } else { k == 8 };
    if big {
        boost_power(w, shot, 1);
    }
    launch(w, shot, e, face)
}

/// Types 0 and 13: no test; the shot freezes as much as the victim.
pub(super) fn shot_plain(w: &mut World, shot: Obj, e: Obj) -> bool {
    cov::hit("shot_plain");
    let v = shot.u8(w.b, f::HITSTOP_VICTIM);
    shot.set_u8(w.b, f::HITSTOP, v);
    e.set_u32(w.b, f::STATE, state::HIT);
    e.set_u8(w.b, f::FLAGS, 1);
    e.set_u8(w.b, f::REACTION, 0x20);
    true
}

/// Type 3: knocks over (reaction 0x60), victim facing away from itself.
pub(super) fn shot_knockdown(w: &mut World, shot: Obj, e: Obj, owner: Obj) -> bool {
    cov::hit("shot_knockdown");
    if immune(w, e, 28) || already_down(w, e) {
        return false;
    }
    e.set_u32(w.b, f::STATE, state::HIT);
    e.set_u8(w.b, f::FLAGS, 1);
    e.set_u8(w.b, f::REACTION, 0x60);
    let fl = e.u8(w.b, f::FLIP) ^ 0x20;
    e.set_u8(w.b, f::REACTION_FLIP, fl);
    owner_flat_bonus(w, shot, owner, Some((5, 4)));
    true
}

/// Type 4: unbalances; alternates reaction 0x68 and nothing thanks to a delay (`+0x87`), adds the sound.
pub(super) fn shot_stagger(w: &mut World, shot: Obj, e: Obj) -> bool {
    cov::hit("shot_stagger");
    if immune(w, e, 29) {
        return false;
    }
    e.set_u32(w.b, f::STATE, state::HIT);
    e.set_u8(w.b, f::FLAGS, 1);
    e.set_u8(w.b, f::HITSTOP_VICTIM, 8);
    let fl = shot.u8(w.b, f::FLIP);
    e.set_u8(w.b, f::REACTION_FLIP, fl);
    e.set_u8(w.b, f::REACTION, 0x4c);
    if e.u8(w.b, f::FLIES) == 0 {
        e.set_u8(w.b, f::REACTION, 0x38);
        if (e.u16(w.b, f::Z) as i16) <= 0 {
            e.set_u8(w.b, f::REACTION, 0);
        }
    }
    if e.u8(w.b, f::STAGGER_DELAY) != 0 {
        e.set_u8(w.b, f::STAGGER_DELAY, 0);
    } else {
        e.set_u8(w.b, f::REACTION, 0x68);
        e.set_u8(w.b, f::STAGGER_DELAY, 0xb4);
    }
    w.spark(Spark::Linked, shot, e, 1);
    crate::sound::play(w.b, crate::sound::CMD_124_224, shot);
    true
}

/// Type 11: magic; +25% of power B against the type 0xb boss.
pub(super) fn shot_mage(w: &mut World, shot: Obj, e: Obj, owner: Obj) -> bool {
    cov::hit("shot_mage");
    if immune(w, e, 27) {
        return false;
    }
    e.set_u32(w.b, f::STATE, state::HIT);
    e.set_u8(w.b, f::FLAGS, 1);
    e.set_u8(w.b, f::REACTION, 0x60);
    let fl = shot.u8(w.b, f::FLIP);
    e.set_u8(w.b, f::REACTION_FLIP, fl);
    let b = shot.u8(w.b, f::POWER_B);
    if e.u8(w.b, f::CLASS) == 4 && e.u8(w.b, f::KIND) == 0xb {
        w.add8(shot, f::POWER_B, b >> 2);
    }
    owner_flat_bonus(w, shot, owner, Some((5, 4)));
    true
}

/// Orientation and reaction of the chained hits.
#[derive(Clone, Copy)]
pub(super) enum Combo {
    /// Given reaction, randomly drawn orientation; the boolean marks the dying victim.
    Random(u8, bool),
    /// Reaction 0x78, the victim faces the launcher.
    OwnerFlip,
    /// Reaction 0x88, the victim turns around.
    VictimFlip,
}

/// Types 5, 22, 7, 17, 18: chained hit (+2 hits landed, +1 hit taken, launcher bonus).
pub(super) fn shot_combo(w: &mut World, shot: Obj, e: Obj, owner: Obj, bit: u32, kind: Combo) -> bool {
    cov::hit("shot_combo");
    // type 7 (OwnerFlip) does not check that the victim is already on the ground
    let down = !matches!(kind, Combo::OwnerFlip) && already_down(w, e);
    if immune(w, e, bit) || down {
        return false;
    }
    owner_hits(w, owner, 2);
    w.add8(e, f::HITS_TAKEN, 1);
    if let Combo::Random(_, true) = kind {
        e.set_u8(w.b, f::ACTIVE, 2);
    }
    e.set_u32(w.b, f::STATE, state::HIT);
    e.set_u8(w.b, f::FLAGS, 1);
    let (react, flat) = match kind {
        Combo::Random(r, _) => (r, (4, 5)),
        Combo::OwnerFlip => (0x78, (6, 5)),
        Combo::VictimFlip => (0x88, (4, 5)),
    };
    e.set_u8(w.b, f::REACTION, react);
    match kind {
        Combo::Random(..) => {
            e.set_u8(w.b, f::REACTION_FLIP, 0);
            if w.random() & 1 != 0 {
                e.set_u8(w.b, f::REACTION_FLIP, 0x20);
            }
        }
        Combo::OwnerFlip => {
            let fl = owner.u8(w.b, f::FLIP) ^ 0x20;
            e.set_u8(w.b, f::REACTION_FLIP, fl);
        }
        Combo::VictimFlip => {
            let fl = e.u8(w.b, f::FLIP) ^ 0x20;
            e.set_u8(w.b, f::REACTION_FLIP, fl);
        }
    }
    owner_bonus(w, shot, owner, OwnerBonus { gear: true, ring: true, flat: Some(flat) });
    true
}

/// Type 6: light hit with spark and sound (0xe7376).
pub(super) fn shot_spark(w: &mut World, shot: Obj, e: Obj, owner: Obj) -> bool {
    cov::hit("shot_spark");
    if immune(w, e, 18) {
        return false;
    }
    owner_hits(w, owner, 2);
    e.set_u32(w.b, f::STATE, state::HIT);
    e.set_u8(w.b, f::FLAGS, 1);
    e.set_u8(w.b, f::REACTION, 0xc);
    let fl = shot.u8(w.b, f::FLIP) ^ 0x20;
    e.set_u8(w.b, f::REACTION_FLIP, fl);
    w.spark(Spark::Linked, shot, e, 1);
    crate::sound::play(w.b, crate::sound::CMD_122_222, shot);
    true
}

/// Types 8 and 12: hit in place (reaction 0x4c, or 0x88 if the victim is on the ground).
pub(super) fn shot_slam(w: &mut World, shot: Obj, e: Obj, owner: Obj, counts: bool) -> bool {
    cov::hit("shot_slam");
    if immune(w, e, 28) {
        return false;
    }
    if counts {
        if already_down(w, e) {
            return false;
        }
        owner_hits(w, owner, 1);
    }
    e.set_u32(w.b, f::STATE, state::HIT);
    e.set_u8(w.b, f::FLAGS, 1);
    e.set_u8(w.b, f::REACTION, 0x4c);
    if e.u8(w.b, f::FLIES) == 0 {
        e.set_u8(w.b, f::REACTION, 0x88);
        let fl = e.u8(w.b, f::FLIP) ^ 0x20;
        e.set_u8(w.b, f::REACTION_FLIP, fl);
    }
    owner_flat_bonus(w, shot, owner, Some((4, 5)));
    true
}

/// Types 14, 15, 16: simple hit that kills the victim (`ACTIVE` = 2). `down`: also checks
/// that the victim is not already on the ground; `flat`: value of `+0x165` giving +5.
pub(super) fn shot_simple(w: &mut World, shot: Obj, e: Obj, owner: Obj, bit: u32, react: u8, down: Option<()>, flat: u8) -> bool {
    cov::hit("shot_simple");
    if immune(w, e, bit) || (down.is_some() && already_down(w, e)) {
        return false;
    }
    owner_hits(w, owner, 1);
    e.set_u8(w.b, f::ACTIVE, 2);
    e.set_u32(w.b, f::STATE, state::HIT);
    e.set_u8(w.b, f::FLAGS, 1);
    e.set_u8(w.b, f::REACTION, react);
    let fl = e.u8(w.b, f::FLIP) ^ 0x20;
    e.set_u8(w.b, f::REACTION_FLIP, fl);
    owner_flat_bonus(w, shot, owner, Some((flat, 5)));
    true
}

/// Type 19: lightning; the shot itself goes into reaction.
pub(super) fn shot_lightning(w: &mut World, shot: Obj, e: Obj, owner: Obj) -> bool {
    cov::hit("shot_lightning");
    if immune(w, e, 1) {
        return false;
    }
    owner_hits(w, owner, 2);
    w.add8(e, f::HITS_TAKEN, 1);
    shot.set_u32(w.b, f::STATE, state::HIT);
    e.set_u32(w.b, f::STATE, state::HIT);
    e.set_u8(w.b, f::FLAGS, 1);
    e.set_u8(w.b, f::HITSTOP_VICTIM, 8);
    e.set_u8(w.b, f::REACTION, 0x24);
    let fl = e.u8(w.b, f::FLIP) ^ 0x20;
    e.set_u8(w.b, f::REACTION_FLIP, fl);
    owner_bonus(w, shot, owner, OwnerBonus { gear: true, ring: true, flat: None });
    true
}

/// Type 20: sweep; ordinary enemies drop to 0 HP.
pub(super) fn shot_sweep(w: &mut World, shot: Obj, e: Obj, owner: Obj) -> bool {
    cov::hit("shot_sweep");
    if immune(w, e, 9) {
        return false;
    }
    owner_hits(w, owner, 2);
    e.set_u8(w.b, f::ACTIVE, 2);
    w.add8(e, f::HITS_TAKEN, 1);
    e.set_u32(w.b, f::STATE, state::HIT);
    e.set_u8(w.b, f::FLAGS, 1);
    e.set_u8(w.b, f::REACTION, 0x94);
    if e.u8(w.b, f::CLASS) != 4 && e.u8(w.b, f::KIND) != 0x12 {
        e.set_u16(w.b, f::HP, 0);
    }
    owner_bonus(w, shot, owner, OwnerBonus { gear: true, ring: true, flat: None });
    true
}

/// Type 21: pushes back (the shot goes out); no effect on boss 0x12 / enemy 8.
pub(super) fn shot_push(w: &mut World, shot: Obj, e: Obj) -> bool {
    cov::hit("shot_push");
    let protected = if e.u8(w.b, f::CLASS) == 4 { 0x12 } else { 8 };
    if protected == e.u8(w.b, f::KIND) {
        return false;
    }
    shot.set_u8(w.b, f::ACTIVE, 2);
    e.set_u32(w.b, f::STATE, state::HIT);
    e.set_u8(w.b, f::FLAGS, 1);
    e.set_u8(w.b, f::HITSTOP_VICTIM, 8);
    e.set_u8(w.b, f::REACTION, 0x4c);
    if e.u8(w.b, f::FLIES) == 0 {
        e.set_u8(w.b, f::REACTION, 0x38);
        if (e.u16(w.b, f::Z) as i16) <= 0 {
            e.set_u8(w.b, f::REACTION, 0);
        }
    }
    e.set_u8(w.b, f::REACTION_FLIP, 0);
    if (shot.u16(w.b, f::VX) as i16) >= 0 {
        e.set_u8(w.b, f::REACTION_FLIP, 0x20);
    }
    w.spark(Spark::Linked, shot, e, 1);
    true
}

/// Type 23: fatal hit (HP to 0); the shot goes out.
pub(super) fn shot_execute(w: &mut World, shot: Obj, e: Obj, owner: Obj) -> bool {
    cov::hit("shot_execute");
    if immune(w, e, 0) {
        return false;
    }
    owner_hits(w, owner, 2);
    shot.set_u8(w.b, f::ACTIVE, 2);
    e.set_u32(w.b, f::STATE, state::HIT);
    e.set_u8(w.b, f::FLAGS, 1);
    e.set_u8(w.b, f::REACTION, 0x8c);
    e.set_u16(w.b, f::HP, 0);
    let fl = e.u8(w.b, f::FLIP) ^ 0x20;
    e.set_u8(w.b, f::REACTION_FLIP, fl);
    true
}

/// Type 24 (0x27242): special shot; direct damage, without updating the gauge.
pub(super) fn shot_kind_24(w: &mut World, shot: Obj, e: Obj) {
    cov::hit("shot_kind_24");
    if shot.u8(w.b, f::VARIANT) == 1 {
        if immune(w, e, 28) || e.u8(w.b, f::REACTION) == 0x60 {
            return;
        }
        e.set_u8(w.b, f::REACTION, 0x60);
    } else {
        e.set_u8(w.b, f::REACTION, 0xc);
        if e.u16(w.b, f::Z) != 0 {
            e.set_u8(w.b, f::REACTION, 0x38);
        }
    }
    e.set_u32(w.b, f::STATE, state::HIT);
    e.set_u8(w.b, f::FLAGS, 1);
    let fl = e.u8(w.b, f::FLIP) ^ 0x20;
    e.set_u8(w.b, f::REACTION_FLIP, fl);
    let hp = e.u16(w.b, f::HP);
    e.set_u16(w.b, f::HP_PREV, hp);
    e.set_u8(w.b, f::HITSTOP_VICTIM, 8);
    w.set_bit(e, f::HIT_FLAGS, 1);
    let st = e.u32(w.b, f::STATE);
    e.set_u32(w.b, f::PREV_STATE, st);
    let dmg = shot_damage(w, shot, e);
    enemy_loses_hp(w, e, dmg);
}
