//! Doubles graphiques d'ennemis (types ENEMY 22, 23 et 24). Voir `docs/familles/enemy.md`.
//!
//! Un double est un ennemi sans logique propre : il recopie son maître (mot `+0xb2`, un ennemi de
//! type 25 ou 26 qui le crée par paires) et joue l'animation que l'état du maître commande. Deux
//! jeux de tables de décision, de même charpente : l'état du maître (`+4`) mène à son sous-état
//! (`+5`), puis à `+6` et au geste en cours `+7`, qui désignent un jeu d'animations de la ROM.

use super::cx::{handlers, Cx};
use super::enemy_common::*;

/// Animation à lancer : rien, par `0x10428` (jeu choisi d'après `+0x33` du maître, puis
/// `+0x44 = +0x3e`) ou par `0x10450` (d'après `+0x3e` du maître : dernier coup).
#[derive(Debug, PartialEq)]
enum Play {
    Nothing,
    Anim(u32),
    Hit(u32),
    /// Si `+0xb6` est levé : le baisse et lance l'entrée `n` du jeu 0x16abd6 (`0x103d6`) ; sinon un pas
    /// d'animation (`0x1040c`).
    Restart(u32),
    /// Comme `Restart`, et à la fin d'un cycle d'animation (`+0x33`) crée un tir ESHL 0x31 du maître.
    RestartSpawn(u32),
    /// Animation par `0x10428`, puis `+0x38 = 0`.
    AnimClear(u32),
    /// Un pas d'animation (`0x1040c`).
    Step,
}
use Play::*;
use crate::objects::pools;
use crate::objects::Courant;

/// États du maître (`+4` à `+7`).
struct Master {
    state: u8,
    sub: u8,
    s6: u8,
    s7: u8,
}

impl Master {
    fn read<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) -> Master {
        let m = x.linked(MASTER);
        Master { state: m.u8(x.b, 4), sub: m.u8(x.b, 5), s6: m.u8(x.b, 6), s7: m.u8(x.b, 7) }
    }
}

/// Champ du maître (adresse RAM sur 16 bits).
const MASTER: u32 = 0xb2;

fn play<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, play: Play) {
    match play {
        Anim(set) => lib::animation_of_master(x, set),
        Hit(set) => lib::animation_of_mark(x, set),
        Restart(n) => restart_or_step(x, n),
        RestartSpawn(n) => {
            restart_or_step_spawn(x, n);
        }
        AnimClear(set) => {
            lib::animation_of_master(x, set);
            x.set8(0x38, 0);
        }
        Step => {
            lib::step(x);
        }
        Nothing => {}
    }
}

/// Jeu d'animations des gestes de la Chimère.
const GESTURES: u32 = 0x16abd6;

fn restart_or_step<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, n: u32) {
    if x.u8(0xb6) != 0 {
        x.set8(0xb6, 0);
        lib::start(x, GESTURES, n);
    } else {
        lib::step(x);
    }
}

fn restart_or_step_spawn<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, n: u32) {
    if x.u8(0xb6) != 0 {
        x.set8(0xb6, 0);
        return lib::start(x, GESTURES, n);
    }
    lib::step(x);
    if x.u8(0x33) == 0 {
        return;
    }
    x.set8(0x33, 0);
    // l'original réessaie tant que le pool de tirs est plein
    let shot = loop {
        if let Some(o) = x.alloc(&pools::ESHL) {
            break o;
        }
    };
    shot.set_u8(x.b, 0, 1);
    shot.set_u8(x.b, 2, 0x31);
    let master = x.u16(MASTER);
    shot.set_u16(x.b, f::CREATOR, master);
}

/// Perte de PV d'un double qui suit sa propre vie (types 22 et 23) : le gel (`+0x58`) tombe après 20
/// images ; son ; à 0 PV le double passe en « blessé » (`+0xaf`), le jeu note `dying_code` en `+0x7e`
/// et crée un effet EFFCT 0x3f de variante `effect_variant`.
fn hurt<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, dying_code: u8, effect_variant: u8) {
    let wait = x.u16(0xb4) as i16;
    if wait > 0 {
        x.set16(0xb4, (wait - 1) as u16);
        if wait - 1 <= 0 {
            x.set32(0x58, 0);
        }
    }
    let hp = x.u16(f::HP) as i16;
    if hp >= x.u16(0x64) as i16 {
        return;
    }
    x.set16(0x64, hp as u16);
    x.set16(0xb4, 0x14);
    if hp > 0 {
        return lib::sound(x, 0xe40b4);
    }
    x.set8(0xaf, 1);
    lib::sound(x, 0xe4188);
    x.set8(0x7e, dying_code);
    lib::object_event(x);
    let Some(e) = x.alloc(&pools::EFFCT) else { return };
    e.set_u8(x.b, 0, 1);
    e.set_u8(x.b, 2, 0x3f);
    e.set_u8(x.b, 3, effect_variant);
    e.set_u16(x.b, f::CREATOR, x.o.0 as u16);
}

/// Fin commune : recopie l'état du maître (activité, drapeaux, position décalée de `dy` en
/// profondeur, orientation, propriétaire) et remet l'objet en liste, sauf s'il vient d'être libéré.
fn mirror<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, dy: u32) {
    let master = x.linked(MASTER);
    if master.u8(x.b, 0xad) != 0 {
        x.set8(0x38, 0);
    }
    if x.u8(0) == 0 {
        return;
    }
    let v = master.u8(x.b, 0);
    x.set8(0, v);
    let v = master.u8(x.b, 1);
    x.set8(1, v);
    let v = master.u16(x.b, 0x8);
    x.set16(0x8, v);
    let v = master.u16(x.b, 0xc);
    x.set16(0xc, v);
    let v = master.u32(x.b, 0x10).wrapping_add(dy);
    x.set32(0x10, v);
    let v = master.u16(x.b, 0x2c);
    x.set16(0x2c, v);
    let v = master.u16(x.b, f::OWNER);
    x.set16(f::OWNER, v);
    x.reinsert();
}

// ---------------------------------------------------------------------------------------------
// Type 24 (0x76afc) : « Manticore » (h)
// ---------------------------------------------------------------------------------------------

/// Double graphique d'un ennemi de type 25. Il se place un pixel plus bas que son maître (profondeur
/// + 1) et choisit son animation avec `choose_active` (maître dans l'état 0 ou 2, hors pause) ou
/// `choose_hit` (maître touché).
mod manticore {
    use super::*;

    /// Jeu neutre (repos) joué quand le maître est dans un état sans geste propre.
    const REST: u32 = 0x16c2da;

    pub fn run<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        let m = Master::read(x);
        let choice = match m.state {
            0 | 2 if !paused(x) => choose_active(m.sub, m.s6, m.s7 as i8),
            4 => choose_hit(m.sub, m.s6 as i8),
            _ => Nothing,
        };
        play(x, choice);
        if m.state == st::DEAD {
            free(x);
        }
        mirror(x, 1);
    }

    /// Maître actif : animation d'après son sous-état `sub`, `s6` (sous-sous-état) et `s7` (geste en
    /// cours, signé : <= 0 = rien). Les `step` sont les indices des petites tables de l'original
    /// (`s7 - 2`).
    fn choose_active(sub: u8, s6: u8, s7: i8) -> Play {
        let step = (s7 as u8).wrapping_sub(2);
        let gesture = s7 > 0;
        match sub {
            0 => match s6 {
                2 => Anim(0x16c59a),
                4 => Anim(REST),
                _ => Nothing,
            },
            2 => match s6 {
                0 if s7 != 0 => Anim(REST),
                2 if s7 != 0 => Anim(0x16c33a),
                4 | 6 if s7 != 0 => Anim(0x16c39a),
                8 if gesture => match step {
                    0 => Anim(0x16c43a),
                    2 => Anim(0x16c48a),
                    _ => Nothing,
                },
                10 if gesture => match step {
                    0 | 2 | 4 => Anim(0x16c49a),
                    6 => Anim(0x16c4ba),
                    _ => Nothing,
                },
                _ => Nothing,
            },
            4 if gesture => match (s6, step) {
                (0, 0) => Anim(0x16c52a),
                (2, 0) => Anim(0x16c59a),
                (2, 4) => Anim(0x16c5fa),
                (2, 6) => Anim(0x16c60a),
                (4, 0) => Anim(0x16c59a),
                (4, 4) => Anim(0x16c66a),
                (4, 6) => Anim(0x16c67a),
                (6, 0) => Anim(0x16c69a),
                (6, 2) => Anim(0x16c6ba),
                (6, 4) => Anim(0x16c6ca),
                (6, 6) => Anim(0x16c6da),
                (8, 0 | 2 | 4 | 6 | 8) => Anim(0x16c6fa),
                (8, 10) => Anim(REST),
                (10, 0 | 2 | 4 | 6) | (12, 0 | 2 | 4 | 6 | 8) => Anim(REST),
                _ => Nothing,
            },
            6 | 8 | 10 | 12 if s6 != 0 => Anim(REST),
            _ => Nothing,
        }
    }

    /// Maître touché (état 4) : animation du coup (`s6` = geste, signé).
    fn choose_hit(sub: u8, s6: i8) -> Play {
        match sub {
            0 | 4 | 6 | 8 if s6 > 0 => Hit(0x16c75a),
            2 if s6 > 0 && matches!((s6 as u8).wrapping_sub(2), 0 | 2) => Anim(0x16c51a),
            _ => Nothing,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 23 (0x76592) : « Spectre » (h)
// ---------------------------------------------------------------------------------------------

/// Double graphique d'un ennemi, avec deux jeux d'animations (normal et « blessé » quand `+0xaf` est
/// non nul) et son propre suivi de vie : quand ses PV (`+0x62`) passent sous le minimum connu
/// (`+0x64`), il joue un son et, à 0 PV, lève `+0xaf`, meurt en son (`0xe4188`) et crée un effet
/// (EFFCT 0x3f). Position : un pixel plus haut que le maître (profondeur - 1).
mod spectre {
    use super::*;
    /// Jeu de repos (normal, blessé).
    const REST: (u32, u32) = (0x16b7c2, 0x16b822);

    pub fn run<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        if x.u8(0xaf) == 0 {
            hurt(x, 0x25, 1);
        }
        let m = Master::read(x);
        let injured = x.u8(0xaf) != 0;
        let choice = match m.state {
            0 | 2 if !paused(x) => choose_active(m.sub, m.s6, m.s7 as i8, injured),
            4 => choose_hit(m.sub, m.s6 as i8, injured),
            _ => Nothing,
        };
        play(x, choice);
        if m.state == st::DEAD {
            free(x);
        }
        mirror(x, 0xffff_ffff);
    }

    /// Jeu (normal, blessé) selon l'état du double.
    fn pick(injured: bool, normal: u32, hurt: u32) -> Play {
        Anim(if injured { hurt } else { normal })
    }

    fn choose_active(sub: u8, s6: u8, s7: i8, injured: bool) -> Play {
        let step = (s7 as u8).wrapping_sub(2);
        let gesture = s7 > 0;
        let rest = || pick(injured, REST.0, REST.1);
        match sub {
            0 => match s6 {
                2 => pick(injured, 0x16bdea, 0x16be4a),
                4 => rest(),
                _ => Nothing,
            },
            2 => match s6 {
                0 if s7 != 0 => rest(),
                2 if s7 != 0 => pick(injured, 0x16b882, 0x16b8e2),
                4 | 6 if s7 != 0 => pick(injured, 0x16b942, 0x16b9e2),
                8 if gesture => match step {
                    0 => pick(injured, 0x16ba82, 0x16bad2),
                    2 => pick(injured, 0x16bb22, 0x16bb32),
                    _ => Nothing,
                },
                10 if gesture => match step {
                    0 | 2 | 4 => pick(injured, 0x16bb42, 0x16bb62),
                    6 => pick(injured, 0x16bb82, 0x16bb92),
                    _ => Nothing,
                },
                _ => Nothing,
            },
            4 if gesture => match (s6, step) {
                (0, 0) => pick(injured, 0x16bd0a, 0x16bd7a),
                (2, 0) => pick(injured, 0x16bdea, 0x16be4a),
                (2, 4) => pick(injured, 0x16beaa, 0x16beba),
                (2, 6) => pick(injured, 0x16beca, 0x16bf2a),
                (4, 0) => pick(injured, 0x16bdea, 0x16be4a),
                (4, 4) => pick(injured, 0x16bf8a, 0x16bf9a),
                (4, 6) => pick(injured, 0x16bfaa, 0x16bfca),
                (6, 0) => pick(injured, 0x16bfea, 0x16c00a),
                (6, 2) => pick(injured, 0x16c02a, 0x16c03a),
                (6, 4) => pick(injured, 0x16c04a, 0x16c05a),
                (6, 6) => pick(injured, 0x16c06a, 0x16c08a),
                (8, 0 | 2 | 4 | 6 | 8) => pick(injured, 0x16c0aa, 0x16c10a),
                (8, 10) => rest(),
                (10, 0 | 2 | 4 | 6) | (12, 0 | 2 | 4 | 6 | 8) => rest(),
                _ => Nothing,
            },
            6 | 8 | 10 | 12 if s6 != 0 => rest(),
            _ => Nothing,
        }
    }

    fn choose_hit(sub: u8, s6: i8, injured: bool) -> Play {
        match sub {
            0 | 4 | 6 | 8 if s6 > 0 => Hit(if injured { 0x16c20a } else { 0x16c16a }),
            2 if s6 > 0 && matches!((s6 as u8).wrapping_sub(2), 0 | 2) => pick(injured, 0x16bc42, 0x16bc52),
            _ => Nothing,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 22 (0x75f42) : « Chimera » (h)
// ---------------------------------------------------------------------------------------------

/// Double graphique de même charpente que le Spectre (deux jeux d'animations, vie propre, un pixel plus
/// haut que le maître), avec en plus des gestes dont l'animation est relancée à la demande
/// (`+0xb6`, jeu 0x16abd6) et un tir ESHL 0x31 (souffle) émis à la fin d'un geste du maître.
mod chimera {
    use super::*;

    /// Jeu de repos (normal, blessé).
    const REST: (u32, u32) = (0x16ac0c, 0x16ac6c);

    pub fn run<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        if x.u8(0xaf) == 0 {
            hurt(x, 0x21, 0);
        }
        let m = Master::read(x);
        let injured = x.u8(0xaf) != 0;
        let choice = match m.state {
            0 | 2 if !paused(x) => choose_active(m.sub, m.s6, m.s7 as i8, injured),
            4 => choose_hit(m.sub, m.s6 as i8, injured),
            _ => Nothing,
        };
        play(x, choice);
        if m.state == st::DEAD {
            free(x);
        }
        mirror(x, 1);
    }

    fn pick(injured: bool, normal: u32, hurt: u32) -> Play {
        Anim(if injured { hurt } else { normal })
    }

    fn choose_active(sub: u8, s6: u8, s7: i8, injured: bool) -> Play {
        let step = (s7 as u8).wrapping_sub(2);
        let gesture = s7 > 0;
        let rest = || pick(injured, REST.0, REST.1);
        match sub {
            0 => match s6 {
                2 => pick(injured, 0x16b214, 0x16b274),
                4 => rest(),
                _ => Nothing,
            },
            2 => match s6 {
                0 if s7 != 0 => rest(),
                2 if s7 != 0 => pick(injured, 0x16accc, 0x16ad2c),
                4 | 6 if s7 != 0 => pick(injured, 0x16ad8c, 0x16ae2c),
                8 if gesture => match step {
                    0 => pick(injured, 0x16aecc, 0x16af1c),
                    2 => pick(injured, 0x16af6c, 0x16af7c),
                    _ => Nothing,
                },
                10 if gesture => match step {
                    0 | 2 | 4 => pick(injured, 0x16af8c, 0x16afac),
                    6 => pick(injured, 0x16afcc, 0x16afdc),
                    _ => Nothing,
                },
                _ => Nothing,
            },
            4 if gesture => match (s6, step) {
                (0, 0) => pick(injured, 0x16b134, 0x16b1a4),
                (2 | 4, 0) => pick(injured, 0x16b214, 0x16b274),
                (2, 4) => pick(injured, 0x16b2d4, 0x16b2e4),
                (2, 6) => pick(injured, 0x16b2f4, 0x16b354),
                (4, 4) => pick(injured, 0x16b3b4, 0x16b3c4),
                (4, 6) => pick(injured, 0x16b3d4, 0x16b3f4),
                (6, 0) => pick(injured, 0x16b414, 0x16b434),
                (6, 2) => pick(injured, 0x16b454, 0x16b464),
                (6, 4) => pick(injured, 0x16b474, 0x16b484),
                (6, 6) => pick(injured, 0x16b494, 0x16b4b4),
                (8, 0 | 2 | 6 | 8) => pick(injured, 0x16b214, 0x16b274),
                (8, 4) => Restart(0xb),
                (8, 10) => rest(),
                (10, 0) => AnimClear(0x16ac0c),
                (10, 2) => RestartSpawn(0xc),
                (10, 4) => Restart(0xd),
                (10, 6) => rest(),
                (12, 0) => AnimClear(0x16ac0c),
                (12, 2) => Restart(0xc),
                (12, 4) => Step,
                (12, 6) => Restart(0xd),
                (12, 8) => rest(),
                _ => Nothing,
            },
            6 | 8 | 10 | 12 if s6 != 0 => rest(),
            _ => Nothing,
        }
    }

    fn choose_hit(sub: u8, s6: i8, injured: bool) -> Play {
        match sub {
            0 | 4 | 6 | 8 if s6 > 0 => Hit(if injured { 0x16b6c0 } else { 0x16b620 }),
            2 if s6 > 0 && matches!((s6 as u8).wrapping_sub(2), 0 | 2) => pick(injured, 0x16b08c, 0x16b09c),
            _ => Nothing,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 26 (0x77736) : « DarkWarrior 2 » (h)
// ---------------------------------------------------------------------------------------------

/// Double d'un ennemi de type 25 (mot `+0xb0`) : au lieu de dessiner une animation propre, il rejoue
/// le mouvement du maître. Les quatre octets `+0xb8` à `+0xbb` du maître (groupe, mode, phase, étape)
/// disent comment recopier ses vitesses : à l'identique, inversées, ou en rejouant le déplacement
/// (`0x1d90` : X, Y, Z ; `0x1da6` : X, Z ; `0x1d78` : X) pour que le double reste synchrone avec les
/// obstacles. Quand le maître est touché (`+0xb8 = 4`), il rejoue aussi la réaction aux coups avec
/// les routines communes. À la mort du maître ou du double, il crée un effet (EFFCT 0x32).
mod dark_warrior_double {
    use super::*;

    const MASTER: u32 = 0xb0;
    /// Groupe, mode, phase et étape du maître.
    const GROUP: u32 = 0xb8;
    const MODE: u32 = 0xb9;
    const PHASE: u32 = 0xba;
    const STEP: u32 = 0xbb;
    /// Table d'adresses des réactions aux coups (code `+0x49` du maître).
    const REACTIONS: u32 = 0x77aec;
    /// Terrain sous l'objet qui renverse la vitesse en profondeur (octets par terrain 0..8).
    const REVERSING_TERRAIN: u32 = 0x77d08;
    const ATTACKED: u16 = 0x400;
    const MASTER_ATTACKED: u16 = 0x406;

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        let m = x.linked(MASTER);
        if x.u16(f::STATE) == ATTACKED || m.u16(x.b, f::STATE) == MASTER_ATTACKED {
            return vanish(x);
        }
        match m.u8(x.b, GROUP) {
            0 | 2 => follow(x),
            4 => struck(x),
            6 => free(x),
            _ => {}
        }
        if x.u8(0) == 0 {
            return;
        }
        let m = x.linked(MASTER);
        let v = m.u8(x.b, 0);
        x.set8(0, v);
        let v = m.u8(x.b, f::FLIP);
        x.set8(f::FLIP, v);
        let v = m.u16(x.b, 0x54);
        x.set16(0x54, v);
        x.set8(0x3f, 4);
        x.reinsert();
    }

    /// Le maître oublie son double, qui crée un effet puis disparaît.
    fn vanish<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        m.set_u16(x.b, MASTER, 0);
        spawn_effect(x);
        free(x);
    }

    /// 0x77c8a : effet EFFCT 0x32 à la place du double (un pixel plus haut).
    fn spawn_effect<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
        let Some(e) = x.alloc(&pools::EFFCT) else { return };
        e.set_u8(x.b, 0, 1);
        e.set_u8(x.b, 2, 0x32);
        e.set_u8(x.b, 3, 7);
        e.set_u16(x.b, f::CREATOR, x.o.0 as u16);
        for off in [0x8, 0xc, 0x10] {
            let v = x.u16(off);
            e.set_u16(x.b, off, v);
        }
        x.add16(0x10, 1);
        let v = x.u8(f::FLIP);
        e.set_u8(x.b, f::FLIP, v);
        let v = x.u32(0x34);
        e.set_u32(x.b, 0x34, v);
        let v = x.u8(0x3c);
        e.set_u8(x.b, 0x3c, v);
        let v = x.u8(0x3f);
        e.set_u8(x.b, 0x3f, v);
        e.set_u16(x.b, 0x30, 0x28);
    }

    // --- recopies du maître ----------------------------------------------------------------------

    /// 0x77cdc : recopie du cadre d'animation (`+0x34` long, `+0x38`) du maître.
    fn copy_frame<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        let v = m.u32(x.b, 0x34);
        x.set32(0x34, v);
        let v = m.u8(x.b, 0x38);
        x.set8(0x38, v);
    }

    /// Hauteur du maître (`+0xc`, mot), puis son cadre.
    fn copy_height<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        let v = m.u16(x.b, 0xc);
        x.set16(0xc, v);
    }

    fn copy_height_frame<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
        copy_height(x);
        copy_frame(x);
    }

    /// Les trois vitesses (X, Z, Y : longs en `+0x14`, `+0x18`, `+0x1c`) du maître.
    fn copy_speeds<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        for off in [0x14, 0x18, 0x1c] {
            let v = m.u32(x.b, off);
            x.set32(off, v);
        }
    }

    /// Vitesses du maître inversées en X et Y (Z conservée), hauteur et cadre (0x777fa, 0x77a34).
    fn mirror_speeds<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        let z = m.u32(x.b, 0x18);
        x.set32(0x18, z);
        let vx = m.u32(x.b, 0x14).wrapping_neg();
        let vy = m.u32(x.b, 0x1c).wrapping_neg();
        x.set32(0x14, vx);
        x.set32(0x1c, vy);
        copy_height_frame(x);
    }

    /// Le maître n'a pas fini son pas d'animation (`+0x33` nul) : le double rejoue le déplacement.
    fn replay(x: &mut Cx, mover: fn(&mut Cx)) {
        let m = x.linked(MASTER);
        if m.u8(x.b, 0x33) == 0 {
            mover(x);
        }
    }

    // --- maître actif (+0xb8 = 0 ou 2) -----------------------------------------------------------

    fn follow(x: &mut Cx) {
        if !paused(x) {
            let m = x.linked(MASTER);
            match m.u8(x.b, MODE) {
                0 => mode0(x),
                2 => mode2(x),
                4 => mode4(x),
                _ => copy_frame(x),
            }
        }
        lib::keep_in_zone(x);
        lib::collide_terrain(x);
    }

    /// Le maître marche : vitesses inversées, ou déplacement rejoué.
    fn mode0(x: &mut Cx) {
        let m = x.linked(MASTER);
        match m.u8(x.b, PHASE) {
            0 | 2 | 4 | 6 => mirror_speeds(x),
            8 => {
                replay(x, |x| { x.move_xyz(); });
                copy_height_frame(x);
            }
            10 | 12 => {
                x.move_xyz();
                copy_height_frame(x);
            }
            _ => copy_height_frame(x),
        }
    }

    /// 0x77842 : le maître est en vol ou saute ; on recopie ou rejoue selon la phase.
    fn mode2(x: &mut Cx) {
        let m = x.linked(MASTER);
        match m.u8(x.b, PHASE) {
            4 | 6 => {
                let step = m.u8(x.b, STEP);
                match step {
                    0 => {
                        copy_speeds(x);
                        copy_frame(x);
                    }
                    2 => {
                        if m.u8(x.b, 0x33) == 0 {
                            x.move_xyz();
                            copy_height(x);
                            if terrain_reverses(x) {
                                let v = (x.u32(0x1c) as i32).wrapping_neg();
                                x.set32(0x1c, v as u32);
                            }
                        }
                        copy_frame(x);
                    }
                    4 | 6 => {
                        x.move_xyz();
                        copy_height(x);
                        if terrain_reverses(x) {
                            let v = (x.u32(0x1c) as i32).wrapping_neg();
                            x.set32(0x1c, v as u32);
                        }
                        copy_frame(x);
                    }
                    _ => copy_frame(x),
                }
            }
            8 => {
                let step = m.u8(x.b, STEP);
                match step {
                    0 => copy_speeds(x),
                    2 => {
                        if m.u8(x.b, 0x33) == 0 {
                            x.move_xyz();
                            copy_height(x);
                        }
                    }
                    4 | 6 => {
                        x.move_xyz();
                        copy_height(x);
                    }
                    _ => {}
                }
                copy_frame(x);
            }
            10 => {
                let step = m.u8(x.b, STEP);
                match step {
                    2 => {
                        if m.u8(x.b, 0x33) != 0 {
                            copy_speeds(x);
                            x.move_xz();
                            copy_height(x);
                        }
                    }
                    4 => {
                        x.move_xz();
                        copy_height(x);
                        let v = m.u16(x.b, 0x16);
                        x.set16(0x16, v);
                    }
                    6 => {
                        x.move_xz();
                        let h = m.u16(x.b, 0xc);
                        x.set16(0xc, h);
                        if (h as i16) <= 0 {
                            let v = m.u32(x.b, 0x14);
                            x.set32(0x14, v);
                        }
                    }
                    8 => {
                        x.move_x();
                        mode4(x);
                        return;
                    }
                    _ => {}
                }
                copy_frame(x);
            }
            _ => copy_frame(x),
        }
    }

    /// 0x779da : le maître est dans un état spécial ; seules quelques phases déplacent le double.
    fn mode4(x: &mut Cx) {
        let m = x.linked(MASTER);
        match m.u8(x.b, PHASE) {
            10 => match m.u8(x.b, STEP) {
                0 => mirror_speeds(x),
                2 => {
                    replay(x, |x| { x.move_xyz(); });
                    copy_height_frame(x);
                }
                4 | 6 => {
                    x.move_xyz();
                    copy_height_frame(x);
                }
                _ => copy_height_frame(x),
            },
            _ => copy_frame(x),
        }
    }

    /// 0x77cee : le terrain sous le double renverse-t-il sa vitesse en profondeur ?
    fn terrain_reverses(x: &mut Cx) -> bool {
        let t = crate::video::terrain_of(x.b, x.o);
        crate::video::terrain_68k(x.c, &t);
        let terrain = t.numero & 0x3f;
        x.c.d[2] = x.c.d[2] & !0xffff | terrain as u32;
        if terrain as u8 >= 9 {
            x.c.d[0] = 0;
            return false;
        }
        let b = x.rom8(REVERSING_TERRAIN + terrain as u32);
        b != 0
    }

    // --- maître touché (+0xb8 = 4) ---------------------------------------------------------------

    fn struck(x: &mut Cx) {
        x.set16(f::HP, 0x7fff);
        let m = x.linked(MASTER);
        match m.u8(x.b, MODE) {
            0 => replay_reaction(x),
            2 => match m.u8(x.b, PHASE) {
                0 => {
                    let v = m.u32(x.b, 0x14);
                    x.set32(0x14, v);
                    x.set16(0xc, 0);
                    copy_frame(x);
                }
                2 | 4 => {
                    let v = m.u16(x.b, 0x16);
                    x.set16(0x16, v);
                    x.move_x();
                    copy_frame(x);
                }
                _ => {}
            },
            4 => match m.u8(x.b, PHASE) {
                0 | 2 | 8 => copy_frame(x),
                4 => {
                    for off in [0x14, 0x18, 0x1c] {
                        let v = m.u16(x.b, off);
                        x.set16(off, v);
                    }
                    copy_frame(x);
                }
                6 => {
                    x.move_xyz();
                    copy_height_frame(x);
                }
                _ => {}
            },
            6 | 8 => match m.u8(x.b, PHASE) {
                0 => {
                    copy_speeds(x);
                    copy_frame(x);
                }
                2 => {
                    x.move_xz();
                    copy_height_frame(x);
                }
                4 | 6 => copy_frame(x),
                _ => {}
            },
            10 => copy_frame(x),
            _ => {}
        }
        lib::keep_in_zone(x);
        lib::collide_terrain(x);
    }

    /// 0x77ab6 : le double reprend l'état de réaction du maître et rejoue sa routine de réaction.
    fn replay_reaction(x: &mut Cx) {
        let m = x.linked(MASTER);
        let sub = m.u16(x.b, PHASE);
        x.set16(f::SUB2, sub);
        copy_speeds(x);
        copy_height(x);
        let code = m.u8(x.b, 0x49) as u32;
        x.c.d[0] = code;
        let routine = rom32(x, REACTIONS + code);
        x.c.a[1] = routine;
        x.call(routine);
        x.set8(0x3b, 0);
    }
}

handlers! {
    0x77736 "enemy_26" dark_warrior_double::run,
    0x75f42 "enemy_22" chimera::run,
    0x76592 "enemy_23" spectre::run,
    0x76afc "enemy_24" manticore::run,
}
