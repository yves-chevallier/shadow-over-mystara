//! Parades, attaques ennemies, caisses, poussées et objets ramassables.

use super::*;

// ---------------------------------------------------------------------------------------------
// Parades : l'attaque d'un joueur croise celle d'un ennemi ou d'un tir ennemi

/// 0x27d74 : le coup d'un joueur croise le coup d'un ennemi.
pub(super) fn player_attacks_clash_enemies(w: &mut World) {
    cov::hit("player_attacks_clash_enemies");
    if List::Enemies.len(w) == 0 {
        return;
    }
    for p in players() {
        if !w.is_active(p) || w.u8(p, f::ATTACK_BOX) == 0 || w.u16(p, f::STATE) == 0x0208 {
            continue;
        }
        let mine = object_box(w, rom::ATTACK, p, f::ATTACK_BOX);
        scan(w, List::Enemies, scratch::CLASH, |w, e| {
            if w.is_active(e) && w.u8(e, f::ATTACK_BOX) != 0 && w.u8(e, f::BOXES_OFF) == 0 {
                let theirs = object_box(w, rom::ATTACK, e, f::ATTACK_BOX);
                if overlap3(w, p, mine, e, theirs) {
                    clash_with_enemy(w, p, e);
                }
            }
            Flow::Next
        });
    }
}

/// 0x27e0a : le coup d'un joueur croise un tir ennemi.
pub(super) fn player_attacks_clash_enemy_shots(w: &mut World) {
    cov::hit("player_attacks_clash_enemy_shots");
    if List::EnemyShots.len(w) == 0 {
        return;
    }
    for p in players() {
        if !w.is_active(p) || w.u8(p, f::ATTACK_BOX) == 0 || w.u16(p, f::STATE) == 0x0208 {
            continue;
        }
        let mine = object_box(w, rom::ATTACK, p, f::ATTACK_BOX);
        scan(w, List::EnemyShots, scratch::CLASH, |w, s| {
            if w.is_active(s) && w.u8(s, f::ATTACK_BOX) != 0 && w.u8(s, f::BOXES_OFF) == 0 {
                let theirs = object_box(w, rom::ATTACK, s, f::ATTACK_BOX);
                if overlap3(w, p, mine, s, theirs) {
                    clash_with_enemy_shot(w, p, s);
                }
            }
            Flow::Next
        });
    }
}

/// 0x27f32 : parade possible si le joueur attaque avec la boîte 4 face à l'ennemi.
pub(super) fn clash_with_enemy(w: &mut World, p: Obj, e: Obj) {
    cov::hit("clash_with_enemy");
    if w.u8(e, f::HITSTOP) == 0
        && w.u8(p, f::ACTION) != 6
        && w.u8(p, f::ATTACK_BOX) == 4
        && w.u8(p, f::FLIP) != w.u8(e, f::FLIP)
    {
        parry(w, p, e);
    }
}

/// 0x27f5e : parade d'un tir ennemi, selon une table (par type de tir) : 0 jamais, 1 toujours,
/// 2..5 seulement avec l'arme 0x2c..0x2f.
pub(super) fn clash_with_enemy_shot(w: &mut World, p: Obj, s: Obj) {
    cov::hit("clash_with_enemy_shot");
    if w.u8(s, f::HITSTOP) != 0 || w.u8(p, f::ACTION) == 6 || w.u8(p, f::ATTACK_BOX) != 4 {
        return;
    }
    let kind = w.u8(s, f::KIND) as u32;
    let need = w.pc8(0x27fd6 + kind);
    let weapon = w.u8(p, f::WEAPON_10E);
    let ok = match need {
        1 => true,
        n @ 2..=5 => weapon == 0x2a + n,
        _ => false,
    };
    if ok && w.u8(p, f::FLIP) != w.u8(s, f::FLIP) {
        parry(w, p, s);
    }
}

/// 0x28026 : le joueur pare : état de parade, gel des deux camps, étincelle, son (0xe75b0).
pub(super) fn parry(w: &mut World, p: Obj, other: Obj) {
    cov::hit("parry");
    let a = w.u8(p, f::ACTION);
    w.set8(p, f::PREV_ACTION, a);
    w.set32(p, f::STATE, state::PARRY);
    w.set8(p, f::ACTIVE, 2);
    w.set8(p, f::INVULN, 0x3c);
    w.set8(p, f::HITSTOP, 8);
    w.set8(p, f::HIT_FLAGS, 8);
    w.set8(other, f::HITSTOP_VICTIM, 8);
    w.set_bit(p, f::HIT_MARK, 5);
    w.set_bit(other, f::HIT_FLAGS, 5);
    w.call_d1(0xaa96c, p, other, 0);
    w.call_ao(0xe75b0, p, other);
}

// ---------------------------------------------------------------------------------------------
// Les ennemis frappent les joueurs

/// Un joueur peut-il encaisser un coup (en vie, boîte de corps, ni invulnérable ni en agonie) ?
pub(super) fn player_vulnerable(w: &mut World, p: Obj) -> bool {
    w.is_active(p) && w.u8(p, f::BODY_BOX) != 0 && w.u8(p, f::INVULN) == 0 && (w.u16(p, f::DYING) as i16) <= 0
}

/// 0x272c2 : coups des ennemis contre les joueurs.
pub(super) fn enemies_hit_players(w: &mut World) {
    cov::hit("enemies_hit_players");
    if w.a5_8(ram::FROZEN) != 0 || w.a5_8(ram::PAUSE) != 0 || List::Enemies.len(w) == 0 {
        return;
    }
    for p in players() {
        if !player_vulnerable(w, p) {
            continue;
        }
        let body = object_box(w, rom::BODY, p, f::BODY_BOX);
        scan(w, List::Enemies, scratch::ENEMY_ATTACKS, |w, e| {
            if w.is_active(e) && w.u8(e, f::ATTACK_BOX) != 0 && w.u8(e, f::BOXES_OFF) == 0 {
                let atk = object_box(w, rom::ATTACK, e, f::ATTACK_BOX);
                if overlap3_attack(w, p, body, e, atk) {
                    enemy_hits_player(w, p, e);
                }
            }
            Flow::Next
        });
    }
}

/// 0x2736c : tirs des ennemis contre les joueurs. La boucle s'arrête quand le joueur n'est plus
/// actif (il vient d'être touché).
pub(super) fn enemy_shots_hit_players(w: &mut World) {
    cov::hit("enemy_shots_hit_players");
    if w.a5_8(ram::FROZEN) != 0 || w.a5_8(ram::PAUSE) != 0 || List::EnemyShots.len(w) == 0 {
        return;
    }
    for p in players() {
        if !player_vulnerable(w, p) {
            continue;
        }
        let body = object_box(w, rom::BODY, p, f::BODY_BOX);
        let n = List::EnemyShots.len(w);
        wr16(w.b, scratch::ENEMY_ATTACKS, n);
        let mut ptr = A5 + ram::LISTS[List::EnemyShots as usize];
        while w.is_active(p) {
            let s = Obj::from_word(rd16(w.b, ptr));
            ptr += 2;
            if w.is_active(s) && w.u8(s, f::ATTACK_BOX) != 0 && w.u8(s, f::BOXES_OFF) == 0 {
                let atk = HitBox(rom::ENEMY_SHOT_ATTACK + 8 * w.u8(s, f::ATTACK_BOX) as u32);
                if overlap3_attack(w, p, body, s, atk) {
                    enemy_shot_hits_player(w, p, s);
                }
            }
            let k = rd16(w.b, scratch::ENEMY_ATTACKS).wrapping_sub(1);
            wr16(w.b, scratch::ENEMY_ATTACKS, k);
            if k == 0 {
                break;
            }
        }
    }
}

/// Réaction d'un joueur à une attaque (4 octets par identifiant d'attaque, tables ROM).
pub(super) struct Reaction {
    ground: u8,
    air: u8,
    power: u8,
    power_hell: u8,
}

pub(super) fn reaction(w: &mut World, table: u32, id: u8) -> Reaction {
    let at = table + 4 * id as u32;
    Reaction { ground: rd8(w.b, at), air: rd8(w.b, at + 1), power: rd8(w.b, at + 2), power_hell: rd8(w.b, at + 3) }
}

/// Puissance de l'attaque : valeur "enfer" au-delà de la difficulté 5.
pub(super) fn attack_power(w: &mut World, r: &Reaction) -> u8 {
    if w.a5_8(ram::DIFFICULTY) > 5 {
        r.power_hell
    } else {
        r.power
    }
}

/// Le joueur est touché : agonie un instant (`ACTIVE` = 2), invulnérable 0x28 images ; l'attaquant
/// gèle, se lie au joueur et note quel type de coup a porté (`flag` : bit de `HIT_FLAGS`).
pub(super) fn hurt_setup(w: &mut World, p: Obj, attacker: Obj, flag: u32) {
    w.set8(p, f::ACTIVE, 2);
    w.set8(p, f::HIT_MARK, 0);
    let fl = w.u8(attacker, f::FLIP);
    w.set8(p, f::HITSTOP_VICTIM, fl);
    w.set8(p, f::INVULN, 0x28);
    w.set8(p, f::HIT_FLAGS, 8);
    w.set8(attacker, f::HITSTOP, 8);
    w.set_bit(attacker, f::HIT_FLAGS, flag);
    w.set16(attacker, f::LINK, p.word());
    w.set32(p, f::STATE, state::PLAYER_HURT);
}

/// 0x2749e : un ennemi touche un joueur.
pub(super) fn enemy_hits_player(w: &mut World, p: Obj, e: Obj) {
    cov::hit("enemy_hits_player");
    trace!("ennemi {:06x} frappe {:06x}", e.0, p.0);
    hurt_setup(w, p, e, 2);
    let id = w.u8(e, f::ATTACK_ID_ENEMY);
    let r = reaction(w, rom::ENEMY_ATTACK_REACTIONS, id);
    let airborne = w.u16(p, f::Z) != 0;
    w.set8(p, f::ACTION, if airborne { r.air } else { r.ground });
    let power = attack_power(w, &r);
    w.set8(e, f::POWER_A, power);
    player_loses_hp(w, p, e);
    // les attaques 0x11 et 0x12 projettent tous les joueurs sauf la classe 2 (magicien (h))
    if w.u8(p, f::VARIANT) >> 1 != 2 && matches!(id, 0x11 | 0x12) {
        w.set8(p, f::ACTION, 0x26);
        if airborne {
            w.set8(p, f::ACTION, 0x2e);
        }
    }
    if w.u8(p, f::GEAR_166) == 1 {
        match w.u8(p, f::ACTION) {
            0x26 => w.set8(p, f::ACTION, 0),
            0x2e => w.set8(p, f::ACTION, 0x1c),
            _ => {}
        }
    }
    w.call_d1(0xaaa04, p, e, 2);
    w.call_ao(0xec046, p, e);
}

/// 0x2756c : un tir ennemi touche un joueur ; certains équipements ou états l'annulent.
pub(super) fn enemy_shot_hits_player(w: &mut World, p: Obj, s: Obj) {
    cov::hit("enemy_shot_hits_player");
    trace!("tir ennemi {:06x} sur {:06x}", s.0, p.0);
    let id = w.u8(s, f::ATTACK_ID_ENEMY);
    let r = reaction(w, rom::ENEMY_SHOT_REACTIONS, id);
    let action = if w.u16(p, f::Z) != 0 { r.air } else { r.ground };
    let power = attack_power(w, &r);
    w.set8(s, f::POWER_A, power);
    // déjà dans cette réaction (état 0x0204, action 0x38 ou 0x28) : le tir est sans effet
    if w.u16(p, f::STATE) == 0x0204 && matches!(w.u8(p, f::ACTION), 0x38 | 0x28) && action == w.u8(p, f::ACTION) {
        return;
    }
    if w.u8(p, f::GEAR_165) == 1 && action == 0x20 {
        return;
    }
    let kind = w.u8(s, f::KIND);
    if w.u8(p, f::GEAR_166) == 1 && matches!(kind, 0x1b | 0x1d) {
        return;
    }
    // protections selon l'équipement : table par type de tir (octets en ROM), 1 = parable, 2 = magie
    let guard = rd8(w.b, rom::SHOT_GUARD + kind as u32);
    if w.u8(p, f::GEAR_165) == 2 && guard == 1 {
        return;
    }
    if w.u8(p, f::GEAR_166) == 1 && guard == 1 && w.random() & 1 != 0 {
        return;
    }
    if w.u8(p, f::GEAR_167) == 0xa && guard == 2 {
        return;
    }
    hurt_setup(w, p, s, 3);
    w.set8(p, f::ACTION, action);
    player_loses_hp(w, p, s);
    w.call_d1(0xaaa04, p, s, 5);
    w.call_ao(0xec200, p, s);
}

/// 0x276e0 : retire des PV au joueur (dégâts de 0x4002 moins l'armure) ; à 0 PV, un joueur de
/// moins en vie.
pub(super) fn player_loses_hp(w: &mut World, p: Obj, attacker: Obj) {
    cov::hit("player_loses_hp");
    let hp = w.u16(p, f::HP);
    w.set16(p, f::HP_PREV, hp);
    let mut dmg = damage_vs_player(w, p, attacker);
    if w.u8(p, f::GEAR_165) == 0 {
        dmg = dmg.wrapping_sub(2);
    }
    let armor = w.u8(p, f::ARMOR);
    if (armor as i8) >= 0 {
        // réduction par (armure, classe) : mots en ROM, 8 par classe
        let class = (w.u8(p, f::VARIANT) >> 1) as u32;
        let at = 0x27732 + 2 * armor as u32 + 8 * class;
        let reduction = w.pc16(at);
        dmg = dmg.wrapping_sub(reduction);
    }
    if (dmg as i16) < 0 {
        dmg = 0;
    }
    let left = w.u16(p, f::HP).wrapping_sub(dmg);
    w.set16(p, f::HP, if (left as i16) < 0 { 0 } else { left });
    if w.u16(p, f::HP) == 0 {
        let n = w.a5_8(ram::PLAYERS_ALIVE).wrapping_sub(1);
        wr8(w.b, A5 + ram::PLAYERS_ALIVE, if (n as i8) < 0 { 0 } else { n });
    }
}

// ---------------------------------------------------------------------------------------------
// Caisses et objets qui frappent

/// Table des dégâts des caisses : par type de caisse (`+0x60`) et phase du compteur FFFF92.
pub(super) fn box_damage(w: &mut World, bx: Obj) -> u16 {
    let row = (w.u8(bx, f::HP_MAX) as u32) << 4;
    let tick = (w.a5_16(ram::TICK) >> 1) & 0xf;
    w.pc16(0x28334 + row + 2 * tick as u32)
}

/// 0x2806a : les caisses (lancées) blessent les joueurs.
pub(super) fn boxes_hit_players(w: &mut World) {
    cov::hit("boxes_hit_players");
    if List::Boxes.len(w) == 0 {
        return;
    }
    for p in players() {
        if !w.is_active(p) || w.u8(p, f::BODY_BOX) == 0 || (w.u8(p, f::DYING) as i8) > 0 {
            continue;
        }
        if w.u8(p, f::ACTION) == 0x14 && w.u8(p, 0x8b) == 2 {
            continue;
        }
        let body = object_box(w, rom::BODY, p, f::BODY_BOX);
        scan(w, List::Boxes, scratch::BOX_ATTACKS, |w, bx| {
            if w.is_active(bx) && w.u8(bx, f::ATTACK_BOX) != 0 && w.u8(bx, f::BOXES_OFF) == 0 {
                let atk = object_box(w, rom::ATTACK, bx, f::ATTACK_BOX);
                if (Pair { a: p, abox: body, b: bx, bbox: atk, signed: true }).overlaps(w) {
                    box_hits_player(w, p, bx);
                }
            }
            Flow::Next
        });
    }
}

/// 0x28246 : un joueur reçoit une caisse.
pub(super) fn box_hits_player(w: &mut World, p: Obj, bx: Obj) {
    cov::hit("box_hits_player");
    trace!("caisse {:06x} sur joueur {:06x}", bx.0, p.0);
    w.set8(p, f::ACTIVE, 2);
    let fl = w.u8(bx, f::FLIP);
    w.set8(p, f::HITSTOP_VICTIM, fl);
    w.set8(p, f::HIT_FLAGS, 8);
    w.set8(bx, f::HITSTOP, 6);
    w.set32(p, f::STATE, state::PLAYER_HURT);
    w.set8(p, f::ACTION, 4);
    w.set8(bx, 0x71, 1);
    let hp = w.u16(p, f::HP);
    w.set16(p, f::HP_PREV, hp);
    let dmg = box_damage(w, bx);
    let left = hp.wrapping_sub(dmg);
    if (left as i16) < 0 {
        w.set16(p, f::HP, 0);
        let n = w.a5_8(ram::PLAYERS_ALIVE).wrapping_sub(1);
        wr8(w.b, A5 + ram::PLAYERS_ALIVE, if (n as i8) < 0 { 0 } else { n });
    } else {
        w.set16(p, f::HP, left);
    }
    w.call_d1(0xaa96c, p, bx, 2);
}

/// 0x28116 : les caisses (lancées) blessent les ennemis.
pub(super) fn boxes_hit_enemies(w: &mut World) {
    cov::hit("boxes_hit_enemies");
    if List::Boxes.len(w) == 0 || List::Enemies.len(w) == 0 {
        return;
    }
    let n = List::Enemies.len(w);
    let mut ptr = A5 + ram::LISTS[List::Enemies as usize];
    for _ in 0..n {
        let e = Obj::from_word(rd16(w.b, ptr));
        ptr += 2;
        if !(w.is_active(e) && w.u8(e, f::BODY_BOX) != 0 && w.u8(e, f::BOXES_OFF) == 0) {
            continue;
        }
        let body = object_box(w, rom::BODY, e, f::BODY_BOX);
        scan(w, List::Boxes, scratch::BOX_ATTACKS, |w, bx| {
            if w.is_active(bx) && w.u8(bx, f::ATTACK_BOX) != 0 && w.u8(bx, f::BOXES_OFF) == 0 {
                let atk = object_box(w, rom::ATTACK, bx, f::ATTACK_BOX);
                if (Pair { a: e, abox: body, b: bx, bbox: atk, signed: true }).overlaps(w) {
                    box_hits_enemy(w, e, bx);
                }
            }
            Flow::Next
        });
    }
}

/// 0x28296 : un ennemi reçoit une caisse (une seule fois : marque `HIT_MARK` = 0xff).
pub(super) fn box_hits_enemy(w: &mut World, e: Obj, bx: Obj) {
    cov::hit("box_hits_enemy");
    trace!("caisse {:06x} sur ennemi {:06x}", bx.0, e.0);
    if w.u8(e, f::HITSTOP) != 0 || w.u8(e, f::HIT_MARK) == 0xff {
        return;
    }
    w.set8(e, f::HIT_MARK, 0xff);
    let thrower = Obj::from_word(w.u16(bx, 0x6c));
    let fl = w.u8(thrower, f::FLIP);
    w.set8(e, f::REACTION_FLIP, fl);
    w.set8(e, f::HITSTOP_VICTIM, 8);
    w.set8(bx, f::HITSTOP, 4);
    w.set32(e, f::STATE, state::HIT);
    w.set8(e, f::REACTION, 0x4c);
    if w.u8(e, f::FLIES) == 0 {
        w.set8(e, f::REACTION, 0x38);
        if (w.u16(e, f::Z) as i16) <= 0 {
            w.set8(e, f::REACTION, 8);
        }
    }
    w.set8(bx, 0x71, 1);
    let st = w.u32(e, f::STATE);
    w.set32(e, f::PREV_STATE, st);
    let hp = w.u16(e, f::HP);
    w.set16(e, f::HP_PREV, hp);
    let dmg = box_damage(w, bx);
    enemy_loses_hp(w, e, dmg);
    w.call_d1(0xaaa84, e, bx, 7);
}

// ---------------------------------------------------------------------------------------------
// Poussées : les joueurs bousculent ennemis et caisses

/// 0x27792 : poussées des joueurs contre les ennemis puis contre les caisses.
pub(super) fn players_push_others(w: &mut World) {
    cov::hit("players_push_others");
    players_push_enemies(w);
    players_push_boxes(w);
}

/// 0x2779c : les joueurs poussent les ennemis (boîtes de poussée).
pub(super) fn players_push_enemies(w: &mut World) {
    cov::hit("players_push_enemies");
    if List::Enemies.len(w) == 0 || w.a5_8(ram::PAUSE) != 0 {
        return;
    }
    for p in players() {
        if w.u8(p, f::ACTIVE) == 0 || w.u8(p, f::PUSH_BOX) == 0 {
            continue;
        }
        let mine = object_box(w, rom::PUSH, p, f::PUSH_BOX);
        scan(w, List::Enemies, scratch::PUSH, |w, e| {
            if w.is_active(e) && w.u8(e, f::PUSH_BOX) != 0 && w.u8(e, f::FLIES) != 0 && w.u8(e, f::BOXES_OFF) == 0 {
                let theirs = object_box(w, rom::PUSH, e, f::PUSH_BOX);
                if push_enemy(w, p, mine, e, theirs) {
                    return Flow::Stop;
                }
            }
            Flow::Next
        });
    }
}

/// 0x27834 : les joueurs poussent les caisses.
pub(super) fn players_push_boxes(w: &mut World) {
    cov::hit("players_push_boxes");
    if List::Boxes.len(w) == 0 || w.a5_8(ram::PAUSE) != 0 {
        return;
    }
    for p in players() {
        if w.u8(p, f::ACTIVE) == 0 || w.u8(p, f::PUSH_BOX) == 0 {
            continue;
        }
        let mine = object_box(w, rom::PUSH, p, f::PUSH_BOX);
        scan(w, List::Boxes, scratch::PUSH, |w, bx| {
            if w.u8(bx, f::ACTIVE) != 0 && w.u8(bx, f::PUSH_BOX) != 0 && w.u8(bx, f::BOXES_OFF) == 0 {
                let theirs = object_box(w, rom::PUSH, bx, f::PUSH_BOX);
                if push_box(w, p, mine, bx, theirs) {
                    return Flow::Stop;
                }
            }
            Flow::Next
        });
    }
}

/// Recouvrement de deux boîtes de poussée avec pénétration sur Y puis X.
/// Renvoie (pénétration Y, signe Y, pénétration X, signe X) s'il y a contact.
pub(super) struct Push {
    depth_y: u16,
    neg_y: bool,
    depth_x: u16,
    neg_x: bool,
}

pub(super) fn push_probe(w: &mut World, pair: &Pair) -> Option<Push> {
    let y = pair.probe_y(w);
    if !y.overlaps() {
        return None;
    }
    let (depth_y, neg_y) = y.penetration();
    let x = pair.probe_x(w);
    if !x.overlaps() {
        return None;
    }
    let (depth_x, neg_x) = x.penetration();
    Some(Push { depth_y, neg_y, depth_x, neg_x })
}

/// 0x27b84 : le joueur `p` pousse l'ennemi `e`. Renvoie vrai (drapeau Z clair de l'original) si le
/// joueur vient d'enchaîner sur une poussée marquante, ce qui interrompt la boucle.
pub(super) fn push_enemy(w: &mut World, p: Obj, mine: HitBox, e: Obj, theirs: HitBox) -> bool {
    cov::hit("push_enemy");
    let pair = Pair { a: p, abox: mine, b: e, bbox: theirs, signed: false };
    let Some(push) = push_probe(w, &pair) else { return false };
    // l'axe de moindre pénétration cède ; le joueur est repoussé de la profondeur de l'autre axe
    if push.depth_x <= push.depth_y {
        let d = if push.neg_x { push.depth_x.wrapping_neg() } else { push.depth_x };
        w.add16(p, f::X, d);
        if w.u16(p, f::Z) != 0
            || w.u8(p, f::SUBSTATE) != 0
            || w.u8(p, 0x8a) != 0
            || w.u8(p, 0x10f) != 0
            || w.u8(p, 0x6a) == 4
            || w.u8(p, f::KEYS_NEW) & 0x10 == 0
        {
            return false;
        }
        w.set32(p, f::STATE, state::ATTACKING);
        w.set8(p, 0x6a, 4);
        true
    } else {
        let d = if push.neg_y { push.depth_y.wrapping_neg() } else { push.depth_y };
        w.add16(p, f::Y, d);
        if w.u16(p, f::Z) != 0
            || w.u8(p, f::SUBSTATE) != 0
            || w.u8(p, 0x8a) != 0
            || w.u8(p, 0x10f) != 0
            || w.u8(p, 0x6a) == 0x10
            || w.u8(p, f::KEYS_NEW) & 0x10 == 0
            || !push.neg_y
        {
            return false;
        }
        w.set32(p, f::STATE, state::ATTACKING);
        w.set8(p, 0x6a, 0x10);
        true
    }
}

/// 0x27a4c : le joueur `p` pousse la caisse `bx`. Même convention de retour que `push_enemy`.
pub(super) fn push_box(w: &mut World, p: Obj, mine: HitBox, bx: Obj, theirs: HitBox) -> bool {
    cov::hit("push_box");
    if w.u8(bx, f::KIND) == 3 {
        return push_barrel(w, p, mine, bx, theirs);
    }
    if w.u8(bx, f::KIND) != 1 && w.u16(bx, f::Z) != 0 {
        return false;
    }
    let pair = Pair { a: p, abox: mine, b: bx, bbox: theirs, signed: false };
    let Some(push) = push_probe(w, &pair) else { return false };
    if push.depth_x <= push.depth_y {
        let d = if push.neg_x { push.depth_x.wrapping_neg() } else { push.depth_x };
        w.add16(p, f::X, d);
        return false;
    }
    let d = if push.neg_y { push.depth_y.wrapping_neg() } else { push.depth_y };
    w.add16(p, f::Y, d);
    if w.u8(bx, f::ACTIVE) == 2
        || w.u16(p, f::Z) != 0
        || w.u8(p, f::SUBSTATE) != 0
        || w.u8(p, 0x8a) != 0
        || w.u8(p, 0x10f) != 0
        || (w.u8(bx, f::KIND) != 1 && w.u16(bx, f::Z) != 0)
        || w.u8(p, f::KEYS_NEW) & 0x10 == 0
    {
        return false;
    }
    if w.u8(p, f::VARIANT) >> 1 == 1
        && w.u32(p, f::STATE) & !0xff == state::ATTACKING
        && w.u8(p, 0x6a) == 0x18
    {
        return false;
    }
    if !push.neg_y {
        // 0x27cb2 : le joueur saisit la caisse (qui devient projectile)
        grab_crate(w, p, bx)
    } else {
        // 0x27cfc
        shove_crate(w, p, bx)
    }
}

/// 0x27cb2 : le joueur soulève la caisse. Renvoie vrai si le drapeau Z est clair au retour.
pub(super) fn grab_crate(w: &mut World, p: Obj, bx: Obj) -> bool {
    cov::hit("grab_crate");
    if w.u8(bx, f::KIND) != 0 {
        return true;
    }
    if w.u16(bx, f::STATE) == 0x0204 {
        return false;
    }
    if w.u8(p, f::VARIANT) >> 1 == 4 {
        return false;
    }
    if w.u8(p, 0x66) != 0 {
        return true;
    }
    w.set32(p, f::STATE, state::GRABBED);
    w.set8(p, 0x8a, 1);
    w.set8(p, 0x8b, 0);
    w.set32(p, 0x86, bx.0);
    w.set8(bx, f::ACTIVE, 2);
    w.set16(bx, 0x6c, p.word());
    w.set32(bx, f::STATE, state::PLAYER_HURT);
    true
}

/// 0x27cfc : pousse / lance la caisse depuis l'autre côté.
pub(super) fn shove_crate(w: &mut World, p: Obj, bx: Obj) -> bool {
    cov::hit("shove_crate");
    if w.u8(bx, f::KIND) == 1 {
        return shove_special_crate(w, p, bx);
    }
    if w.u8(bx, f::SUBSTATE) != 0 {
        return true;
    }
    if w.u32(p, f::STATE) & !0xff == state::GRABBED {
        return false;
    }
    w.set32(p, f::STATE, state::GRABBED);
    w.set8(p, 0x8b, 4);
    w.set32(p, 0x86, bx.0);
    w.set8(bx, f::ACTIVE, 2);
    w.set16(bx, 0x6c, p.word());
    true
}

/// 0x27d3c : caisse de type 1 (coffre) : le joueur la fait réagir (octet `+0x6e` = 1).
pub(super) fn shove_special_crate(w: &mut World, p: Obj, bx: Obj) -> bool {
    cov::hit("shove_special_crate");
    if w.u8(bx, f::VARIANT) == 0 {
        if w.u8(bx, f::SUBSTATE) != 0 {
            return true;
        }
        let slot = Obj(bx.0 + 0x78 + w.u8(p, f::KIND) as u32);
        if rd8(w.b, slot.0) != 0 {
            return true;
        }
        wr8(w.b, slot.0, 1);
        w.set32(p, f::STATE, state::CRATE_PUSHED);
    }
    w.set8(bx, 0x6e, 1);
    w.set16(bx, 0x6c, p.word());
    true
}

/// 0x278c4 : le joueur pousse un tonneau (caisse de type 3).
pub(super) fn push_barrel(w: &mut World, p: Obj, mine: HitBox, bx: Obj, theirs: HitBox) -> bool {
    cov::hit("push_barrel");
    if w.u8(bx, 0x66) != 0 || w.u16(bx, f::Z) != 0 {
        return false;
    }
    let pair = Pair { a: p, abox: mine, b: bx, bbox: theirs, signed: false };
    let Some(push) = push_probe(w, &pair) else { return false };
    let cmp_dir = |w: &mut World, field: u32, flip_ok: bool, bx: Obj, p: Obj| -> bool {
        let _ = (w, field, flip_ok, bx, p);
        false
    };
    let _ = cmp_dir;
    if push.depth_x <= push.depth_y {
        let d = if push.neg_x { push.depth_x.wrapping_neg() } else { push.depth_x };
        w.add16(p, f::X, d);
        let keys = w.u8(p, f::KEYS_HELD);
        if keys & 1 != 0 {
            w.set8(p, 0x16f, 8);
            if w.u8(p, f::FLIP) == 0 {
                return false;
            }
            let px = w.u16(p, f::X);
            if px <= w.u16(bx, f::X) {
                return barrel_kick(w, p, bx);
            }
            return false;
        }
        w.set8(p, 0x16f, 6);
        if keys & 2 == 0 || w.u8(p, f::FLIP) != 0 {
            return false;
        }
        let px = w.u16(p, f::X);
        if (px as i16) > (w.u16(bx, f::X) as i16) {
            return barrel_kick(w, p, bx);
        }
        return false;
    }
    let d = if push.neg_y { push.depth_y.wrapping_neg() } else { push.depth_y };
    w.add16(p, f::Y, d);
    if w.u8(bx, 0x65) != 0 {
        return false;
    }
    let keys = w.u8(p, f::KEYS_HELD);
    if keys & 8 != 0 {
        w.set8(p, 0x16f, 2);
        let py = w.u16(p, f::Y);
        if py <= w.u16(bx, f::Y) {
            return barrel_kick(w, p, bx);
        }
        return false;
    }
    w.set8(p, 0x16f, 4);
    if keys & 4 == 0 {
        return false;
    }
    let py = w.u16(p, f::Y) as i16;
    if py > w.u16(bx, f::Y) as i16 {
        return barrel_kick(w, p, bx);
    }
    false
}

/// 0x27a04 : le joueur lance le tonneau s'il le peut. Renvoie vrai (Z clair) quand c'est fait.
pub(super) fn barrel_kick(w: &mut World, p: Obj, bx: Obj) -> bool {
    cov::hit("barrel_kick");
    if w.u8(p, f::VARIANT) >> 1 == 4 {
        return false;
    }
    let st = w.u32(p, f::STATE);
    if st != 0x0200_0000 && st != 0x0200_0400 {
        return false;
    }
    w.set32(p, f::STATE, state::GRABBING);
    w.set32(p, 0x86, bx.0);
    w.set8(bx, 0x66, 1);
    w.set16(bx, 0x6c, p.word());
    true
}

// ---------------------------------------------------------------------------------------------
// Objets ramassables

/// 0x283d4 : les joueurs ramassent les objets au sol.
pub(super) fn players_pick_up_items(w: &mut World) {
    cov::hit("players_pick_up_items");
    if List::Items.len(w) == 0 || w.a5_8(ram::PAUSE) != 0 {
        return;
    }
    for p in players() {
        if !w.is_active(p)
            || w.u8(p, f::BODY_BOX) == 0
            || w.u16(p, f::Z) != 0
            || w.u8(p, f::SUBSTATE) != 0
            || w.u8(p, 0x8a) != 0
            || matches!(w.u8(p, f::ACTION), 0xe | 0x22 | 0x2e | 0x28)
        {
            continue;
        }
        let body = object_box(w, rom::BODY, p, f::BODY_BOX);
        scan(w, List::Items, scratch::ITEMS, |w, item| {
            if w.is_active(item)
                && w.u8(item, f::STATE) == 2
                && w.u8(item, f::BODY_BOX) != 0
                && w.u8(item, f::HIT_MARK) == 0
                && w.u8(item, f::BOXES_OFF) == 0
                && w.u16(item, f::Z) == 0
            {
                let pickup = HitBox(rom::ITEM_PICKUP + 8 * w.u8(item, f::BODY_BOX) as u32);
                if item_reached(w, p, body, item, pickup) && !item_taken_by_button(w, p, item) {
                    item_taken_by_walking(w, p, item);
                }
            }
            Flow::Next
        });
    }
}

/// 0x284d2 : recouvrement du joueur et de l'objet (axes Y et X seulement).
pub(super) fn item_reached(w: &mut World, p: Obj, body: HitBox, item: Obj, pickup: HitBox) -> bool {
    let pair = Pair { a: p, abox: body, b: item, bbox: pickup, signed: false };
    pair.probe_y(w).overlaps() && pair.probe_x(w).overlaps()
}

/// 0x2853a : ramassage au bouton (objets de type 8 : appui sur le bouton 1 ; autres : bouton
/// maintenu, joueur à l'arrêt). Renvoie vrai si l'objet est pris.
pub(super) fn item_taken_by_button(w: &mut World, p: Obj, item: Obj) -> bool {
    cov::hit("item_taken_by_button");
    let mut keys = w.u8(p, f::KEYS_NEW);
    if w.u8(item, f::KIND) == 8 {
        let idx = w.u8(p, f::KIND) as u32;
        let now = rd8(w.b, A5 + idx);
        let before = rd8(w.b, A5 + 6 + idx);
        keys = now & !before;
    }
    if keys & 0x10 == 0 {
        return false;
    }
    if w.u8(item, f::KIND) != 8 {
        if w.u16(p, f::ACTION) != 0x0c00 || w.u8(p, 0x6a) > 8 {
            return false;
        }
        w.set32(p, f::STATE, state::PICKING_UP);
        w.set16(p, 0x8c, item.word());
    }
    w.set8(item, f::HIT_MARK, 1);
    let k = w.u8(p, f::KIND) & 3;
    w.set8(item, f::HIT_BY_KIND, k);
    w.set16(item, f::OWNER, p.word());
    true
}

/// 0x285b0 : ramassage automatique en marchant (joueur en action 0x10, objet de la table).
pub(super) fn item_taken_by_walking(w: &mut World, p: Obj, item: Obj) {
    cov::hit("item_taken_by_walking");
    if w.u8(p, f::ACTION) != 0x10 {
        return;
    }
    let kind = w.u8(item, f::KIND) as u32;
    if w.pc8(0x285e0 + kind) == 0 {
        return;
    }
    w.set8(item, f::HIT_MARK, 2);
    let k = w.u8(p, f::KIND) & 3;
    w.set8(item, f::HIT_BY_KIND, k);
    w.set16(item, f::OWNER, p.word());
}
