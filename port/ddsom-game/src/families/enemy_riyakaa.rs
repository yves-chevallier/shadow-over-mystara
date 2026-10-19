//! Ennemi « Riyakaa » (ENEMY type 16, 0x6f1c0). Voir `docs/familles/enemy.md`.
//!
//! Machine à états à quatre étages : `+4` état principal (0 création, 2 actif, 4 touché, 6 libération),
//! `+5` sous-état (état actif : 0 arrivée selon la variante `+3`, 2 choix d'une attaque, 4 coups de
//! patte, 6 coups spéciaux, 8 poursuite, 10 repos), `+6` et `+7` sous-sous-états. Chaque fonction
//! ci-dessous est un sous-état de l'original, nommé d'après son rôle ; l'adresse d'origine est notée
//! quand elle aide à retrouver le code.

use super::cx::{handlers, Cx};
use super::enemy_common::*;

mod riyakaa {
    use super::*;

    /// Jeux d'animations du corps et des coups.
    const BODY: u32 = 0x1665d6;
    const STRIKE: u32 = 0x1665f8;
    /// Tables d'adresses des réactions aux coups (état 4 : touché debout / touché en pleine action).
    const REACT_STANDING: u32 = 0x6fe22;
    const REACT_ACTING: u32 = 0x70018;
    /// Octets de choix du prochain geste, 32 par niveau de difficulté (`A5+0x7f92` / 2), tirage de 5 bits.
    const NEXT_MOVES: u32 = 0x5ce72;
    const NEXT_ATTACKS: u32 = 0x701dc;
    /// Masques de décision de la contre-attaque (un long par niveau de difficulté).
    const MASKS: u32 = 0x6ff70;
    /// Routines du moteur et de l'IA.
    const SPEED_TABLE: u32 = 0x1b64;
    const MOVE_TABLE: u32 = 0x1cb4;
    const STEP: u32 = 0x1a56;
    /// Test « un joueur est dans cette boîte ? » : D0, D1, D2 = décalage, portée, hauteur ; Z = non.
    const TARGET_IN_BOX: u32 = 0xb5e0;
    /// Choix d'une cible (Z = aucune), regard vers elle, demi-tour vers elle.
    const PICK_TARGET: u32 = 0x424e;
    const FACE_TARGET: u32 = 0xa470;
    const TURN_TO_TARGET: u32 = 0x788c;
    /// État long « actif, sous-état 0 » : retour à la marche.
    const WALK: u32 = 0x0202_0000;

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        x.call(rom::TICK_COUNTERS);
        match x.u8(f::STATE) {
            st::INIT => {
                if paused(x) {
                    return;
                }
                init(x);
                active(x);
            }
            st::ACTIVE => active(x),
            st::HIT => hit(x),
            st::DEAD => free(x),
            _ => {}
        }
    }

    fn init(x: &mut Cx) {
        x.add8(f::STATE, 2);
        x.set8(f::BOX_SET, 0xf);
        x.call(rom::INITIAL_HP);
        x.call(rom::IMMUNITIES);
        x.c.d[0] = 0;
        // paramètres d'IA lus par les tables de décision, et valeurs de départ
        for (off, v) in [
            (0x68, 2),
            (0x69, 2),
            (0x70, 0),
            (0x6a, 1),
            (0x6c, 2),
            (0x6d, 4),
            (0x72, 2),
            (0x73, 2),
            (0x74, 2),
            (0x75, 0),
            (0x76, 3),
            (0xa2, 0x20),
            (0xa1, 2),
            (0x67, 1),
            (0x8e, 0),
            (0x7f, 0),
            (0x7e, 0x44),
        ] {
            x.set8(off, v);
        }
        x.set32(0x8a, 0);
        x.set8(0xad, 0);
        x.set8(0x79, 6);
        x.set8(0x81, 6);
        x.set8(0x80, 7);
    }

    /// Joue l'animation `n` du jeu des mouvements.
    fn body(x: &mut Cx, n: u32) {
        x.anim_start(BODY, n);
    }

    /// Joue l'animation `n` du jeu des coups.
    fn strike(x: &mut Cx, n: u32) {
        x.anim_start(STRIKE, n);
    }

    /// Choisit la table de vitesses `n`.
    fn speed(x: &mut Cx, n: u32) {
        x.c.d[0] = n;
        x.call(SPEED_TABLE);
    }

    /// Retour à l'état actif, sous-état 0.
    fn walk(x: &mut Cx) {
        x.set32(f::STATE, WALK);
    }

    fn toggle_flip(x: &mut Cx) {
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
    }

    /// Décompte un octet et rend sa nouvelle valeur.
    fn countdown(x: &mut Cx, off: u32) -> u8 {
        let n = x.u8(off).wrapping_sub(1);
        x.set8(off, n);
        n
    }

    // --- état 2 : actif ------------------------------------------------------------------------

    fn active(x: &mut Cx) {
        active_loop(x, |x| {
            match x.u8(f::SUB) {
                0 => return arrive(x),
                2 => decide(x),
                4 => swipe(x),
                6 => special(x),
                8 => chase(x),
                10 => rest(x),
                _ => {}
            }
            false
        });
    }

    /// Sous-état 0 : arrivée selon la variante `+3`. Rend vrai quand l'original abandonne l'image sans
    /// l'achever (variante 8, `+6 = 2` : il dépile l'adresse de retour de la fonction de sous-état).
    fn arrive(x: &mut Cx) -> bool {
        match x.u8(0x3) {
            0 | 4 => charge_in(x),
            2 => rise(x),
            6 => leap_in(x),
            8 => return approach(x),
            10 => hover(x),
            _ => {}
        }
        false
    }

    /// Déplacement commun des arrivées : la durée `+0xa2` décomptée, puis retour à la marche.
    fn fly_step(x: &mut Cx) {
        if countdown(x, 0xa2) == 0 {
            walk(x);
            x.set8(0, 1);
        }
        x.call(MOVE_TABLE);
        x.call(STEP);
    }

    /// 0x6f2e0 : entre en courant (variantes 0 et 4).
    fn charge_in(x: &mut Cx) {
        if x.u8(f::SUB2) != 0 {
            return fly_step(x);
        }
        x.add8(f::SUB2, 2);
        x.set8(0x67, 1);
        x.set16(0xa0, 0);
        x.set8(0xa2, 0x18);
        x.set8(0x2d, 0x18);
        if x.u8(f::FLIP) != 0 {
            x.set8(0x2d, 8);
        }
        x.set16(0xa0, 0x202);
        speed(x, 0x17);
        let mut n = 3;
        if x.u8(0x3) == 0 {
            x.set16(0xa0, 0);
            speed(x, 0xd);
            n = 2;
        }
        body(x, n);
    }

    /// 0x6f35e : entre en bondissant (variante 6).
    fn leap_in(x: &mut Cx) {
        if x.u8(f::SUB2) != 0 {
            return fly_step(x);
        }
        x.add8(f::SUB2, 2);
        x.set8(0x67, 1);
        x.set8(0xa2, 0x20);
        x.set8(0xa1, 0xd);
        x.set8(0x2d, 0x10);
        speed(x, 0xd);
        body(x, 6);
    }

    /// 0x6f3aa : début de la chute (jaillit vers le haut puis retombe).
    fn rise_start(x: &mut Cx) {
        x.add8(f::SUB2, 2);
        x.set8(0x67, 0);
        x.set8(0xa1, 0xe);
        x.set16(0x18, 0xfd00);
        x.set16(0x1a, 0xff80);
        body(x, 0xb);
    }

    /// 0x6f3d2 : retombe jusqu'au sol.
    fn rise_fall(x: &mut Cx) {
        x.call(0x1dbc);
        if (x.u16(0xc) as i16) >= 0 {
            return;
        }
        x.add8(f::SUB2, 2);
        x.set32(0xc, 0);
        body(x, 0x10);
    }

    /// 0x6f3f6 : fin de l'animation d'atterrissage.
    fn rise_land(x: &mut Cx) {
        if x.anim_step() {
            walk(x);
            x.set8(0, 1);
        }
    }

    /// Variante 2 : tombe du haut de l'écran.
    fn rise(x: &mut Cx) {
        match x.u8(f::SUB2) {
            0 => rise_start(x),
            2 => rise_fall(x),
            4 => rise_land(x),
            _ => {}
        }
    }

    /// Variante 10 : apparaît après 30 images, puis tombe comme la variante 2.
    fn hover(x: &mut Cx) {
        match x.u8(f::SUB2) {
            0 => {
                x.set8(0x90, 0x1e);
                rise_start(x);
            }
            2 => {
                if countdown(x, 0x90) == 0 {
                    x.add8(f::SUB2, 2);
                }
            }
            4 => rise_fall(x),
            6 => rise_land(x),
            _ => {}
        }
    }

    /// Variante 8 : s'approche jusqu'à voir un joueur ; à `+6 = 2` l'original abandonne l'image.
    fn approach(x: &mut Cx) -> bool {
        match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                x.set8(0x67, 0);
                x.set8(0xa1, 0xb);
                body(x, 0xd);
            }
            2 => {
                x.call(STEP);
                x.call(PICK_TARGET);
                if !x.c.cond(7) {
                    x.c.d[0] = 0;
                    set_dw(x, 1, 0x80);
                    x.c.d[2] = 0x20;
                    x.call(TARGET_IN_BOX);
                    if !x.c.cond(7) {
                        x.add8(f::SUB2, 2);
                    }
                }
                return true;
            }
            4 => {
                x.add8(f::SUB2, 2);
                x.set8(0x67, 1);
                x.set8(0xa1, 0xc);
                body(x, 0xa);
            }
            6 => {
                if x.anim_step() {
                    x.set8(f::SUB, 0xa);
                    x.set16(f::SUB2, 0);
                }
            }
            _ => {}
        }
        false
    }

    /// Sous-état 10 : attend, debout, qu'un joueur soit en vie.
    fn rest(x: &mut Cx) {
        if x.u8(f::SUB2) == 0 {
            x.add8(f::SUB2, 2);
            x.set8(0x67, 1);
            x.set8(0xa1, 1);
            return body(x, 0);
        }
        if x.a5_u8(0x35) == 0 {
            x.call(STEP);
        } else {
            walk(x);
            x.call(PICK_TARGET);
        }
    }

    // --- sous-état 2 : choix d'une attaque -------------------------------------------------------

    fn decide(x: &mut Cx) {
        x.call(0x2810);
        if !x.c.cond(7) {
            x.set16(f::STATE, 0x0408);
            x.set16(f::SUB2, 0);
            return grab(x);
        }
        match x.u8(f::SUB2) {
            0 => {
                if x.u8(0x7) != 0 {
                    decide_follow(x);
                } else {
                    look_begin(x);
                }
            }
            2 => decide_turn(x),
            4 => decide_wait(x),
            _ => {}
        }
        if x.a5_u8(0x35) == 0 {
            x.set32(f::STATE, 0x020a_0000);
        }
    }

    /// 0x6f546 : début de la recherche d'une cible ; l'animation dépend de la présence d'un joueur
    /// tout près.
    fn look_begin(x: &mut Cx) {
        x.add8(0x7, 2);
        x.set8(0x79, 6);
        x.set8(0x67, 1);
        let (mut d4, mut d5, mut d6) = (0xd, 0, 2);
        x.c.d[0] = 0;
        set_dw(x, 1, 0xa0);
        set_dw(x, 2, 0x20);
        x.call(TARGET_IN_BOX);
        if x.c.cond(7) {
            x.set8(0xad, 1);
            d4 = 0x17;
            d6 = 3;
            d5 = 2;
        }
        x.set8(0xa1, d5);
        set_dw(x, 0, d6);
        x.c.a[4] = BODY;
        x.call(0x1a32);
        set_dw(x, 0, d4);
        x.call(SPEED_TABLE);
        x.call(PICK_TARGET);
        x.call(FACE_TARGET);
        x.call(TURN_TO_TARGET);
    }

    /// 0x6f5a6 : suit la cible tant que le geste dure.
    fn decide_follow(x: &mut Cx) {
        x.call(0xe76c);
        let n = countdown(x, 0x71) as i8;
        if n < 0 {
            x.call(FACE_TARGET);
            x.call(PICK_TARGET);
            if x.c.cond(7) {
                return;
            }
            if countdown(x, 0x6b) == 0 {
                x.call(TURN_TO_TARGET);
                x.call(0x79ce);
                if !x.c.cond(7) {
                    x.set16(f::SUB2, 0x400);
                    return;
                }
            }
        }
        decide_near(x);
    }

    /// 0x6f5e0 : adversaire tout près (coup), à portée de saut (bond) ou à rejoindre (marche).
    fn decide_near(x: &mut Cx) {
        set_dw(x, 0, 0xffe0);
        set_dw(x, 1, 0x20);
        set_dw(x, 2, 8);
        x.call(TARGET_IN_BOX);
        if !x.c.cond(7) {
            x.set32(f::STATE, 0x0204_0c00);
            x.set8(0xa3, 6);
            return;
        }
        if want_attack(x) {
            return choose_attack(x);
        }
        set_dw(x, 0, 0xffc8);
        set_dw(x, 1, 0x38);
        set_dw(x, 2, 8);
        x.call(0x7f0c);
        if !x.c.cond(7) {
            let v = x.u8(f::SUB2);
            x.set8(0xa3, v);
            x.set16(f::SUB2, 0xc00);
            return x.call(0xac3a);
        }
        x.call(STEP);
        x.call(MOVE_TABLE);
        if needs_turn(x) {
            x.add8(f::SUB2, 2);
            x.set8(0x7, 0);
        }
    }

    /// 0x6f650 : se retourne vers la cible.
    fn decide_turn(x: &mut Cx) {
        if x.u8(0x7) == 0 {
            x.add8(0x7, 2);
            x.set8(0x67, 1);
            return body(x, 4);
        }
        if !x.anim_step() {
            return;
        }
        toggle_flip(x);
        x.set8(f::SUB2, 0);
        body(x, 2);
    }

    /// 0x6f694 : attente debout avant de repartir.
    fn decide_wait(x: &mut Cx) {
        if x.u8(0x7) == 0 {
            x.add8(0x7, 2);
            x.set8(0x67, 1);
            x.c.d[0] = 1;
            x.set8(0xa0, 1);
            if x.u8(0xa1) != 1 {
                x.set8(0xa1, 1);
                return body(x, 0);
            }
        }
        if (countdown(x, 0x6e) as i8) < 0 {
            x.set16(f::SUB2, 0);
        }
        x.call(STEP);
    }

    // --- sous-état 4 : coups de patte ------------------------------------------------------------

    fn swipe(x: &mut Cx) {
        match x.u8(f::SUB2) {
            0 | 2 | 4 => swing(x),
            6 => lunge(x),
            8 | 10 => dash(x),
            12 => wait_target(x),
            14 => turn_around(x),
            _ => {}
        }
    }

    /// Pose d'un coup de patte : identifiant d'attaque `id`, animation `anim` des coups, décalage
    /// `shift` du regard (`+0xa0`).
    fn swing_pose(x: &mut Cx, id: u8, anim: u32, shift: u8) {
        x.set8(0x44, id);
        x.c.d[2] = anim;
        x.set8(0xa0, shift);
        x.add8(0x7, 2);
        set_dw(x, 0, anim as u16);
        x.c.a[4] = STRIKE;
        x.call(0x1a32);
    }

    /// 0x6f6f0 : coup de patte (`+6` = 0 haut, 2 milieu, 4 bas).
    fn swing(x: &mut Cx) {
        if x.u8(0x7) != 0 {
            if !x.anim_step() {
                return;
            }
            if (countdown(x, 0xa2) as i8) < 0 {
                return retreat(x);
            }
            x.add8(f::SUB2, 0xfe);
            return swing_pose(x, 0xb, 1, 0xf0);
        }
        x.set8(0x67, 0);
        x.set8(0xa2, 0);
        x.set8(0x45, 5);
        let v = x.u8(0xa0);
        x.set8(0xa1, v);
        match x.u8(f::SUB2) {
            2 => swing_pose(x, 0xc, 0, 0xf2),
            0 => swing_pose(x, 0xb, 1, 0xf0),
            _ => {
                x.add8(0xa2, 1);
                swing_pose(x, 0xc, 0, 0xf4);
            }
        }
    }

    /// 0x6f770 : bond en avant puis réception (`+7` : 0 élan, 2 vol, 4 retombée, 6 rebond).
    fn lunge(x: &mut Cx) {
        match x.u8(0x7) {
            0 => lunge_start(x),
            2 => {
                x.call(0x1d62);
                if x.anim_step() {
                    x.add8(0x7, 2);
                    x.set16(0xc, 0xc);
                    x.set16(0x14, 0);
                    x.set16(0x16, 0);
                    strike(x, 4);
                }
            }
            4 => {
                x.call(0x1da6);
                x.call(STEP);
                if x.u16(0xc) < 0xc {
                    x.add8(0x7, 2);
                    body(x, 0x10);
                }
            }
            6 => {
                x.call(0x1da6);
                x.call(STEP);
                if (x.u16(0xc) as i16) < 0 {
                    x.set16(0xc, 0);
                    retreat(x);
                }
            }
            _ => {}
        }
    }

    /// 0x6f784 : élan du bond (aussi utilisé comme contre-attaque, état touché).
    fn lunge_start(x: &mut Cx) {
        x.add8(0x7, 2);
        x.set8(0xac, 0x1a);
        x.set8(0x45, 8);
        x.set8(0x78, 2);
        x.set8(0x67, 0);
        let v = x.u8(0xa0);
        x.set8(0xa1, v);
        x.set8(0xa0, 0xf6);
        x.c.d[0] = 0;
        x.set16(0x14, 0);
        x.set16(0x1c, 0);
        x.set16(0x1e, 0);
        x.set16(0x18, 0x600);
        x.set16(0x1a, 0xffa0);
        let mut d = 0xffd0u16;
        if x.u8(f::FLIP) != 0 {
            d = d.wrapping_neg();
        }
        set_dw(x, 0, d);
        x.set32(0x8a, 0x0204_0600);
        x.set16(0x16, d);
        strike(x, 2);
    }

    /// 0x6f85e : course en ligne droite (`+6` = 8 : puis coup, 10 : puis marche).
    fn dash(x: &mut Cx) {
        if x.u8(0x7) == 0 {
            x.add8(0x7, 2);
            x.set8(0x67, 0);
            x.set8(0xa1, 0);
            x.set32(0xa8, 0x0204_0200);
            if x.u8(f::SUB2) != 8 {
                x.set32(0xa8, WALK);
            }
            x.set8(0xa0, 0);
            x.set8(0xa2, 0x20);
            x.set8(0x2d, 8);
            if x.u8(f::FLIP) != 0 {
                x.set8(0x2d, 0x18);
            }
            speed(x, 0xd);
            return body(x, 2);
        }
        x.call(MOVE_TABLE);
        x.call(STEP);
        if (countdown(x, 0xa2) as i8) < 0 {
            let s = x.u32(0xa8);
            x.set32(f::STATE, s);
        }
    }

    /// 0x6f8d6 : attend de savoir où aller (`+0xa3` = sous-état à reprendre).
    fn wait_target(x: &mut Cx) {
        if x.u8(0x7) == 0 {
            x.add8(0x7, 2);
            x.set8(0x67, 1);
            x.call(0xac3a);
            if x.u8(0x77) == 0 {
                x.set8(0x7, 0);
                let v = x.u8(0xa3);
                x.set8(f::SUB2, v);
            } else if x.u8(0xa1) != 1 {
                x.set8(0xa1, 1);
            } else {
                return wait_target_follow(x);
            }
            return body(x, 0);
        }
        wait_target_follow(x);
    }

    fn wait_target_follow(x: &mut Cx) {
        x.call(STEP);
        if (countdown(x, 0x77) as i8) >= 0 {
            return;
        }
        x.set8(0x7, 0);
        if needs_turn(x) {
            x.add8(f::SUB2, 2);
        } else {
            let v = x.u8(0xa3);
            x.set8(f::SUB2, v);
        }
    }

    /// 0x6f93e : demi-tour.
    fn turn_around(x: &mut Cx) {
        if x.u8(0x7) == 0 {
            x.add8(0x7, 2);
            x.set8(0x67, 1);
            return body(x, 4);
        }
        if !x.anim_step() {
            return;
        }
        toggle_flip(x);
        x.set16(f::SUB2, 0xc00);
        body(x, 0);
    }

    // --- sous-état 6 : coups spéciaux ------------------------------------------------------------

    fn special(x: &mut Cx) {
        match x.u8(f::SUB2) {
            0 | 10 => roar(x),
            2 | 8 => circle(x),
            4 | 6 => jump_strike(x),
            12 => wait_target(x),
            14 => turn_around(x),
            _ => {}
        }
    }

    /// 0x6f990 : cri (si le jeu l'interdit, simple regard).
    fn roar(x: &mut Cx) {
        match x.u8(0x7) {
            0 => {
                x.add8(0x7, 2);
                x.set8(0xa1, 0x1e);
                x.set8(0x67, 1);
                x.call(0x5cdc6);
                if !x.c.cond(7) {
                    return body(x, 1);
                }
                x.add8(0x7, 2);
                x.set8(0xa2, 0x21);
                body(x, 0xe);
            }
            2 => {
                if x.anim_step() {
                    stand_up(x);
                }
            }
            4 => {
                if (countdown(x, 0xa2) as i8) < 0 {
                    return stand_up(x);
                }
                x.call(STEP);
                if x.u8(0x33) != 0 {
                    toggle_flip(x);
                }
            }
            _ => {}
        }
    }

    /// 0x6fa0c : tourne autour de sa cible (`+6` = 2 : course puis fin ; 8 : course puis coup).
    fn circle(x: &mut Cx) {
        match x.u8(0x7) {
            0 => {
                x.add8(0x7, 2);
                x.set8(0x67, 1);
                x.set8(0x6b, 0x18);
                x.set8(0x2d, 0);
                if random(x) & 1 != 0 {
                    x.add8(0x2d, 0x10);
                }
                speed(x, 0xd);
                x.set8(0xa1, 0);
                body(x, 2);
            }
            2 => {
                x.call(MOVE_TABLE);
                x.call(STEP);
                if (countdown(x, 0x6b) as i8) >= 0 {
                    return;
                }
                if x.u8(f::SUB2) == 8 {
                    x.set8(0xa1, 0x1d);
                    x.add8(0x7, 2);
                } else {
                    stand_up(x);
                }
            }
            4 => {
                x.add8(0x7, 2);
                x.set8(0x44, 0xc);
                x.set8(0x45, 5);
                strike(x, 0);
            }
            6 => {
                if x.anim_step() {
                    stand_up(x);
                }
            }
            _ => {}
        }
    }

    /// 0x6faba : saut avec coup (`+6` = 4 : atterrit et finit ; 6 : atterrit et enchaîne un coup).
    fn jump_strike(x: &mut Cx) {
        match x.u8(0x7) {
            0 => {
                x.add8(0x7, 2);
                x.set8(0x67, 1);
                x.set8(0xa1, 0x15);
                x.set16(0x1a, 0xffc8);
                x.set16(0x18, 0x200);
                x.set16(0x14, 0x400);
                x.set16(0x16, 0);
                if x.u8(f::FLIP) != 0 {
                    let v = (x.u16(0x14) as i16).wrapping_neg();
                    x.set16(0x14, v as u16);
                }
                body(x, 9);
            }
            2 => {
                x.call(0x1da6);
                x.call(STEP);
                if (x.u32(0xc) as i32) >= 0 {
                    return;
                }
                x.set32(0xc, 0);
                if x.u8(f::SUB2) == 6 {
                    x.add8(0x7, 2);
                } else {
                    stand_up(x);
                }
            }
            4 => {
                x.add8(0x7, 2);
                x.set8(0x44, 0xb);
                x.set8(0x45, 5);
                strike(x, 1);
            }
            6 => {
                if x.anim_step() {
                    stand_up(x);
                }
            }
            _ => {}
        }
    }

    // --- sous-état 8 : se jette sur un joueur ---------------------------------------------------

    fn chase(x: &mut Cx) {
        match x.u8(f::SUB2) {
            0 => chase_wait(x),
            2 | 4 => chase_run(x),
            6 => chase_pounce(x),
            _ => {}
        }
    }

    /// 0x6fbfc : attente de 16 images.
    fn chase_wait(x: &mut Cx) {
        if x.u8(0x7) == 0 {
            x.add8(0x7, 2);
            x.set8(0x67, 1);
            x.set8(0xa2, 0x10);
            x.set8(0xa1, 1);
            x.set8(0xa0, 1);
            x.add8(0x7, 2);
            return body(x, 0);
        }
        if countdown(x, 0xa2) == 0 {
            walk(x);
        }
        x.call(STEP);
    }

    /// 0x6fc44 : court vers sa cible (`+6` = 4 en X, 2 en profondeur).
    fn chase_run(x: &mut Cx) {
        if x.u8(0x7) == 0 {
            x.add8(0x7, 2);
            x.set8(0x67, 1);
            let target = x.linked(f::OWNER);
            x.set8(0xa1, 0);
            if x.u8(f::SUB2) == 4 {
                x.set8(0xa2, 0x18);
                let d = x.u16(0x8).wrapping_sub(target.u16(x.b, 0x8));
                x.set8(0x2d, 8);
                if (d as i16) < 0 {
                    x.set8(0x2d, 0x18);
                }
            } else {
                x.set8(0xa2, 0x10);
                let d = x.u16(0x10).wrapping_sub(target.u16(x.b, 0x10));
                x.set8(0x2d, 0x10);
                if (d as i16) < 0 {
                    x.set8(0x2d, 0);
                }
            }
            x.c.d[2] = 0;
            x.set8(0xa0, 0);
            x.set8(0x67, 1);
            x.set8(0xa1, 0);
            speed(x, 0xd);
            body(x, 2);
        }
        if countdown(x, 0xa2) == 0 {
            walk(x);
        }
        x.call(MOVE_TABLE);
        x.call(STEP);
    }

    /// 0x6fb78 : saute sur le joueur qu'il vise (trois essais de regard).
    fn chase_pounce(x: &mut Cx) {
        x.set8(0x67, 1);
        let v = x.u8(0xa0);
        x.set8(0xa1, v);
        set_dw(x, 0, 0xffd8);
        set_dw(x, 1, 0x28);
        set_dw(x, 2, 0xc);
        x.call(TARGET_IN_BOX);
        let shift;
        if !x.c.cond(7) {
            x.set32(f::STATE, 0x0204_0c00);
            x.set8(0xa3, 6);
            shift = 0xf6u8;
        } else {
            if !want_attack(x) {
                walk(x);
                x.set8(0xac, 0);
                return look_begin(x);
            }
            choose_attack(x);
            x.c.d[0] = 0;
            shift = x.u8(0xa3).wrapping_add(0xf0);
        }
        x.set8(0xa0, shift);
        if shift != x.u8(0xa1) {
            x.set8(0xac, 0xff);
        }
        x.add8(0xac, 1);
        if x.u8(0xac) == 3 {
            walk(x);
            look_begin(x);
        }
    }

    // --- aides communes --------------------------------------------------------------------------

    /// 0x700fe : retour à la marche ; un joueur proche déclenche le sous-état 10.
    fn stand_up(x: &mut Cx) {
        walk(x);
        x.call(0x79ce);
        if !x.c.cond(7) {
            x.set8(f::SUB, 0xa);
        }
    }

    /// 0x700ca : prend du recul, puis choisit le prochain geste dans une table (sous-état 8).
    fn retreat(x: &mut Cx) {
        x.set8(0x78, 0xff);
        let row = ((x.a5_u16(0x7f92) >> 1) << 5) as u32;
        let r = random(x) & 0x1f;
        x.set8(f::SUB, 8);
        let v = x.data8(NEXT_MOVES + row + r);
        x.set8(f::SUB2, v);
        x.set8(0x7, 0);
    }

    /// 0x70116 : un joueur dans la boîte devant lui et la décision d'attaquer est favorable.
    fn want_attack(x: &mut Cx) -> bool {
        x.c.d[0] = 0;
        set_dw(x, 1, 0x70);
        set_dw(x, 2, 8);
        x.call(TARGET_IN_BOX);
        x.c.d[0] = 0;
        if x.c.cond(7) {
            return false;
        }
        x.c.a[1] = x.u16(f::OWNER) as i16 as i32 as u32;
        let target = x.linked(f::OWNER);
        let facing = target.u8(x.b, f::FLIP);
        x.c.d[1] = x.c.d[1] & !0xff | facing as u32;
        x.call(0xa4c2);
        if x.c.cond(7) {
            return false;
        }
        x.call(0xa4b4);
        if x.c.cond(7) {
            return false;
        }
        x.c.d[0] = 1;
        true
    }

    /// 0x70150 : la cible est-elle derrière lui (il doit se retourner) ?
    fn needs_turn(x: &mut Cx) -> bool {
        if x.u16(f::OWNER) == 0 {
            return false;
        }
        let target = x.linked(f::OWNER);
        let d = x.u16(0x8).wrapping_sub(target.u16(x.b, 0x8));
        let facing = if (d as i16) < 0 { 0x20 } else { 0 };
        facing != x.u8(f::FLIP)
    }

    /// 0x70178 : tire le prochain geste d'attaque dans une table ; si le jeu le permet, repasse en
    /// sous-état 4 avec ce geste.
    fn choose_attack(x: &mut Cx) {
        x.c.d[2] = 0;
        let row = ((x.a5_u16(0x7f92) >> 1) << 5) as u32;
        let r = random(x) & 0x1f;
        let pick = x.data8(NEXT_ATTACKS + row + r);
        x.c.d[2] = pick as u32;
        if pick > 4 {
            return;
        }
        x.call(0x5cdc6);
        if x.c.cond(7) {
            x.set8(f::SUB, 4);
            x.set8(0xa3, pick);
            x.set8(0x7, 0);
            x.set8(f::SUB2, 0xc);
        } else if x.u8(0xad) != 0 {
            speed(x, 0xd);
            x.set8(0xad, 0);
            body(x, 2);
        }
    }

    // --- état 4 : touché -------------------------------------------------------------------------

    fn hit(x: &mut Cx) {
        hit_loop(x, |x| match x.u8(f::SUB) {
            0 => react(x, REACT_STANDING),
            2 => knockback(x),
            4 => recover(x),
            6 => react(x, REACT_ACTING),
            8 => grab(x),
            _ => {}
        });
    }

    /// 0x6fece : projeté en arrière (vitesse +/- 0x500), puis contre-attaque ou reprend sa marche.
    fn knockback(x: &mut Cx) {
        if paused(x) {
            return;
        }
        if x.u8(f::SUB2) == 0 {
            x.add8(f::SUB2, 2);
            x.set8(f::FREEZE, 4);
            x.set16(0x14, 0x500);
            x.set16(0x16, 0xffc8);
            x.call(0xec6ca);
            let side = x.u8(0x4a) ^ 0x20;
            x.set8(0x4a, side);
            if side != 0 {
                let a = (x.u16(0x14) as i16).wrapping_neg();
                x.set16(0x14, a as u16);
                let b = (x.u16(0x16) as i16).wrapping_neg();
                x.set16(0x16, b as u16);
            }
            return body(x, 5);
        }
        if x.u8(0x14) != 0 {
            return x.call(0x1d78);
        }
        x.set8(0, 1);
        x.set32(0x58, 0);
        let level = ((x.a5_u16(0x7f92) >> 2) << 2) as u32;
        let mask = x.data32(MASKS + level);
        let bit = random(x) & 0x1f;
        x.c.d[2] = mask;
        if mask >> bit & 1 != 0 {
            x.set32(f::STATE, 0x0204_0600);
            lunge_start(x);
        } else {
            walk(x);
            body(x, 0);
        }
    }

    /// 0x6ff90 : relève après le coup (`+6` : 0 animation, 2 attente, 4 fin).
    fn recover(x: &mut Cx) {
        match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                body(x, 8);
            }
            2 => {
                if paused(x) {
                    return;
                }
                let n = (x.u16(0x84) as i16).wrapping_sub(1);
                x.set16(0x84, n as u16);
                if n > 0 {
                    return;
                }
                x.set8(0x86, 0);
                x.set8(0x49, 0);
                x.set8(0, 2);
                x.add8(f::SUB2, 2);
                body(x, 8);
            }
            4 => {
                if paused(x) {
                    return;
                }
                if x.anim_step() {
                    x.set8(0x86, 0);
                    x.set8(0, 1);
                    x.set8(0x67, 1);
                    x.set32(f::STATE, WALK);
                    x.set8(0xa0, 0xc0);
                }
            }
            _ => {}
        }
    }

    /// 0x6fd22 : saisi par un joueur ; le mouvement de fuite dépend de la scène et de l'orientation.
    fn grab(x: &mut Cx) {
        if paused(x) {
            return;
        }
        match x.u8(f::SUB2) {
            0 => grab_start(x),
            2 => {
                if x.anim_step() {
                    x.add8(f::SUB2, 2);
                    toggle_flip(x);
                    grab_run(x);
                }
            }
            4 => grab_flee(x),
            _ => {}
        }
    }

    fn grab_start(x: &mut Cx) {
        x.set8(0xae, 1);
        let flipped = x.u8(f::FLIP) != 0;
        let turn = if x.a5_u8(0x34) == 4 {
            flipped
        } else {
            x.call(0x57c66);
            if x.c.cond(7) { flipped } else { !flipped }
        };
        if turn {
            x.add8(f::SUB2, 2);
            x.set8(0x67, 1);
            x.set8(0xa0, 0x7f);
            x.set8(0xa1, 0x7f);
            body(x, 4);
        } else {
            x.add8(f::SUB2, 4);
            grab_run(x);
        }
    }

    /// 0x6fdc0 : s'enfuit en courant, face au sens de la fuite.
    fn grab_run(x: &mut Cx) {
        x.set8(0x67, 1);
        x.set8(0x2d, 8);
        if x.u8(f::FLIP) == 0 {
            x.set8(0x2d, 0x18);
        }
        body(x, 2);
        speed(x, 0x21);
    }

    /// 0x6fdf2 : fuit jusqu'à sortir de l'écran, puis disparaît.
    fn grab_flee(x: &mut Cx) {
        x.call(STEP);
        x.call(MOVE_TABLE);
        x.call(0x1e90);
        if x.c.cond(5) {
            return;
        }
        x.set32(f::STATE, st::DEAD_LONG);
        x.call(0x27ee);
    }
}

handlers! {
    0x6f1c0 "enemy_16" 600 riyakaa::run,
}
