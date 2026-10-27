//! Tirs des joueurs (PSHL) contre les ennemis : comportements au contact (table ROM 0x267c4).

use super::*;
use crate::machine::Machine;

// ---------------------------------------------------------------------------------------------
// Tirs des joueurs contre les ennemis

/// 0x265c4 : chaque tir joueur visible est confronté à chaque ennemi visible.
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
            // le tir a pu être consommé par l'ennemi précédent
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

/// 0x266fe : un tir joueur touche un ennemi. Le comportement du tir (selon son `KIND`) décide si
/// le coup porte ; si oui, la victime enregistre l'agresseur, perd des PV et le compteur de PV
/// affiché est mis à jour.
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
        _ => {
            // type inconnu : on laisse l'original décider (Z clair = le tir touche)
            let h = rd32(w.b, rom::SHOT_HANDLERS + 4 * kind as u32);
            w.c.a[3] = owner.0;
            !w.call_ao(h, shot, e)
        }
    };
    trace!("tir {:06x} type {kind} sur {:06x} : {hit}", shot.0, e.0);
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
    // 0x26774 : l'original teste `+0x166` du tir lui-même (A0), pas celui du lanceur
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

/// 0x26828 : certains tirs achèvent d'un coup les petits ennemis (classe 2, types 0 ou 0x13).
pub(super) fn kill_small_enemy(w: &mut World, shot: Obj, e: Obj) {
    cov::hit("kill_small_enemy");
    let t = shot.u8(w.b, f::KIND) as u32;
    if w.prog8(0x26856 + t) == 0 || e.u8(w.b, f::CLASS) != 2 {
        return;
    }
    let k = e.u8(w.b, f::KIND);
    if k == 0 || k == 0x13 {
        e.set_u16(w.b, f::HP, 0);
    }
}

// --- comportements des tirs (table ROM 0x267c4) ----------------------------------------------

/// Orientation donnée à la victime d'un tir qui la projette.
#[derive(Clone, Copy)]
pub(super) enum Launch {
    /// 0, ou 0x20 si le tir n'est pas du type 6 (champ `FACING_AUX`).
    Fixed,
    /// Orientation du tir.
    ShotFlip,
}

/// La victime est-elle immunisée contre ce comportement (long `+0x94`, un bit par comportement) ?
pub(super) fn immune(w: &mut World, e: Obj, bit: u32) -> bool {
    e.u32(w.b, f::IMMUNITY) >> bit & 1 != 0
}

/// Victime déjà à terre (état 4 avec réaction 0x60 ou 0x88) : certains tirs ne la relancent pas.
pub(super) fn already_down(w: &mut World, e: Obj) -> bool {
    e.u8(w.b, f::STATE) == 4 && matches!(e.u8(w.b, f::REACTION), 0x60 | 0x88)
}

/// Majore la puissance du tir (octets A et B) d'une fraction d'elle-même.
pub(super) fn boost_power(w: &mut World, shot: Obj, shift: u32) {
    let (a, b) = (shot.u8(w.b, f::POWER_A), shot.u8(w.b, f::POWER_B));
    w.add8(shot, f::POWER_A, a >> shift);
    w.add8(shot, f::POWER_B, b >> shift);
}

/// Bonus de puissance du tir selon l'équipement du lanceur, à partir des puissances initiales.
#[derive(Clone, Copy)]
pub(super) struct OwnerBonus {
    /// Objet d'équipement (`+0x10e`) 0x20 : +25 %, 0x21 : +50 %.
    gear: bool,
    /// Bague (`+0x166` = 4) : +25 %.
    ring: bool,
    /// Bonus fixe quand `+0x165` vaut la valeur : (valeur, ajout).
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

/// Compteur de coups portés du lanceur (`+0x190`, mot).
pub(super) fn owner_hits(w: &mut World, owner: Obj, n: u16) {
    w.add16(owner, f::HITS_LANDED, n);
}

/// Réaction commune des tirs qui projettent (0x268ec / 0x269a0) : la victime part en réaction
/// 0x4c (ou 0x38 au sol, 0 si elle est à terre), étincelle de type 1.
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
    w.call_d1(0xaa96c, shot, e, 1);
    true
}

/// Types 1 et 2.
pub(super) fn shot_launch(w: &mut World, shot: Obj, e: Obj, bit: u32, face: Launch) -> bool {
    cov::hit("shot_launch");
    !immune(w, e, bit) && launch(w, shot, e, face)
}

/// Types 9 et 10 : comme 1 et 2, mais contre un gros ennemi (boss de type 5, 8 ou 0x12, ennemi de
/// type 8) la puissance du tir augmente de moitié.
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

/// Types 0 et 13 : aucun test ; le tir gèle autant que la victime.
pub(super) fn shot_plain(w: &mut World, shot: Obj, e: Obj) -> bool {
    cov::hit("shot_plain");
    let v = shot.u8(w.b, f::HITSTOP_VICTIM);
    shot.set_u8(w.b, f::HITSTOP, v);
    e.set_u32(w.b, f::STATE, state::HIT);
    e.set_u8(w.b, f::FLAGS, 1);
    e.set_u8(w.b, f::REACTION, 0x20);
    true
}

/// Type 3 : renverse (réaction 0x60), victime face à l'opposé d'elle-même.
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

/// Type 4 : déséquilibre ; alterne réaction 0x68 et rien grâce à un délai (`+0x87`), ajoute le son.
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
    w.call_d1(0xaa96c, shot, e, 1);
    crate::sound::jouer(w.b, 0xe745a, shot);
    true
}

/// Type 11 : magie ; +25 % de la puissance B contre le boss de type 0xb.
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

/// Orientation et réaction des coups enchaînés.
#[derive(Clone, Copy)]
pub(super) enum Combo {
    /// Réaction donnée, orientation tirée au hasard ; le booléen marque la victime mourante.
    Random(u8, bool),
    /// Réaction 0x78, la victime regarde le lanceur.
    OwnerFlip,
    /// Réaction 0x88, la victime fait demi-tour.
    VictimFlip,
}

/// Types 5, 22, 7, 17, 18 : coup enchaîné (+2 coups portés, +1 coup encaissé, bonus du lanceur).
pub(super) fn shot_combo(w: &mut World, shot: Obj, e: Obj, owner: Obj, bit: u32, kind: Combo) -> bool {
    cov::hit("shot_combo");
    // le type 7 (OwnerFlip) ne vérifie pas que la victime est déjà à terre
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

/// Type 6 : coup léger avec étincelle et son (0xe7376).
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
    w.call_d1(0xaa96c, shot, e, 1);
    crate::sound::jouer(w.b, 0xe7376, shot);
    true
}

/// Types 8 et 12 : coup sur place (réaction 0x4c, ou 0x88 si la victime est au sol).
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

/// Types 14, 15, 16 : coup simple qui fait mourir la victime (`ACTIVE` = 2). `down` : vérifie
/// aussi que la victime n'est pas déjà à terre ; `flat` : valeur de `+0x165` donnant +5.
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

/// Type 19 : éclair ; le tir lui-même passe en réaction.
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

/// Type 20 : balayage ; les ennemis ordinaires tombent à 0 PV.
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

/// Type 21 : repousse (le tir s'éteint) ; sans effet sur le boss 0x12 / l'ennemi 8.
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
    w.call_d1(0xaa96c, shot, e, 1);
    true
}

/// Type 23 : coup fatal (PV à 0) ; le tir s'éteint.
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

/// Type 24 (0x27242) : tir spécial ; dégâts directs, sans mise à jour de la jauge.
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
