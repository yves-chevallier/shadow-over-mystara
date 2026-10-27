//! Anneaux de choix d'objets et de magies (PEFF types 0 et 1, classe C12).
//!
//! Le parent (variante `+3` négative) est lié au joueur (`+0x50`) ; il crée six enfants (type 0) ou un
//! par emplacement de la rangée d'équipement (type 1), garde leurs adresses en `+0x30..` et les pilote
//! par leurs sous-états (`+5`). Les enfants tournent sur l'anneau : l'angle est l'indice de vitesse
//! `+0x2d` (32 positions) et les vitesses viennent de tables de vitesses (`+0x4c`) de la ROM.
//! Les noms de rôle sont des hypothèses (h).

use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Courant;

/// Jeux d'animations des icônes : normal, palette alternative.
const ICONS: u32 = 0x1aef78;
const ICONS_ALT: u32 = 0x1b21c2;

/// Champs des parents et des enfants.
mod g {
    /// Génération de la partie (`A5+0x34`) au moment de la création : l'anneau meurt si elle change.
    pub const GENERATION: u32 = 0x5a;
    /// Lien de l'enfant vers son parent.
    pub const PARENT: u32 = 0x52;
    /// Angle visé, étape de la rotation.
    pub const TARGET: u32 = 0x54;
    pub const STEP: u32 = 0x55;
    /// Enfants du parent (mots).
    pub const CHILDREN: u32 = 0x30;
    /// Délai de l'anneau (octet) et durée de vie (mot).
    pub const DELAY: u32 = 0x3c;
    /// Équipement de l'icône : classe d'objet et quantité (`+0x57`, `+0x58`).
    pub const ITEM: u32 = 0x57;
    pub const COUNT: u32 = 0x58;
}

/// Mot signé de la table de vitesses `table`, converti en 16.16 entier (partie entière du mot 8.8).
fn whole_part(word: u16) -> u32 {
    ((((word as u32) << 16) as i32) >> 8) as u32 & 0xffff_0000
}

/// Règle les vitesses de l'enfant pour l'étape `step` (`0x4a1e0`, `0x4a822`) : table de vitesses
/// choisie par `step`, ligne choisie par l'angle `+0x2d`.
fn set_speeds(x: &mut Cx, tables: u32, step: u8) {
    let table = x.rom32(tables + 4 * (step as i8 as i16 as u16 as u32 & 0xffff) );
    x.set32(0x4c, table);
    let row = (x.u8(0x2d) as u32) << 2 & 0xffff;
    let vx = x.data16(table + row);
    let vz = x.data16(table + row + 2);
    x.set16(0x14, vx);
    x.set16(0x18, vz);
    x.set16(0x16, 0);
    x.set16(0x1a, 0);
}

/// Position de l'enfant : celle du parent, plus deux fois la partie entière des vitesses de la rangée courante.
fn follow_parent(x: &mut Cx) {
    let parent = x.linked(g::PARENT);
    let d0 = if x.u8(0x3) != 0 { 0x8000 } else { 0 };
    x.copy_pos(parent);
    x.set16(0x12, d0);
    let row = (x.u8(0x2d) as u32) << 2 & 0xffff;
    let table = x.u32(0x4c);
    let vx = whole_part(x.data16(table + row));
    x.add32(0x8, vx);
    x.add32(0x8, vx);
    let vz = whole_part(x.data16(table + row + 2));
    x.add32(0xc, vz);
    x.add32(0xc, vz);
}

/// L'anneau meurt si la génération a changé, si le jeu est en pause ou si le joueur est mort.
fn ring_alive(x: &mut Cx, generation: u32) -> bool {
    if x.a5_u8(a5::PAUSED) != 0 {
        return false;
    }
    let current = x.a5_u8(0x34);
    if current != x.u8(generation) {
        return false;
    }
    let player = x.linked(f::OWNER);
    player.u16(x.b, 0x62) != 0
}

/// Le parent continue tant que le jeu n'est pas en pause, que la génération n'a pas changé et que le joueur vit.
fn parent_alive(x: &mut Cx, generation: u32, player: Obj) -> bool {
    if x.a5_u8(a5::PAUSED) != 0 {
        return false;
    }
    let current = x.a5_u8(0x34);
    // registre résiduel (`move.b (0x34,A5),D0b`), voir `parent_follow`
    x.c.d[0] = x.c.d[0] & !0xff | current as u32;
    current == x.u8(generation) && player.u16(x.b, 0x62) != 0
}

/// Parent : suit le joueur à un décalage (mots lus en ROM, selon la variante du joueur).
/// Laisse D0 = rang, D1.w = décalage X, D2.w = décalage Z comme l'original : la boucle de dispatch ne les
/// rétablit pas et le gestionnaire PEFF suivant peut les relire (le rebond du gel, 0x4bda8, indexe sa table
/// par D2 : bot `longue_a`, image 59 919).
fn parent_follow(x: &mut Cx, offsets: u32) {
    let player = x.linked(f::OWNER);
    x.copy_pos(player);
    x.add16(0x10, 0xffff);
    let row = (player.u8(x.b, 0x3) as u32) << 2 & 0xffff;
    let mut dx = x.rom16(offsets + row);
    let dz = x.rom16(offsets + row + 2);
    if player.u8(x.b, f::FLIP) == 0 {
        dx = dx.wrapping_neg();
    }
    x.c.d[0] = row;
    x.c.d[1] = x.c.d[1] & !0xffff | dx as u32;
    x.c.d[2] = x.c.d[2] & !0xffff | dz as u32;
    x.add16(0x8, dx);
    x.add16(0xc, dz);
}

/// Pour chacun des `n` enfants du parent, appelle `f`.
fn for_children(x: &mut Cx, n: u32, mut f: impl FnMut(&mut Cx, Obj)) {
    let mut a2 = x.o.0 + g::CHILDREN;
    for _ in 0..n {
        let child = Obj(x.u16(a2 - x.o.0) as i16 as i32 as u32 & 0xff_ffff);
        f(x, child);
        a2 += 2;
    }
}

/// Anneau de magies/objets du clerc, de l'elfe... : type 0 (0x49e36).
mod ring0 {
    use super::*;

    const OFFSETS: u32 = 0x49f88;
    const INITIAL_ANGLE: u32 = 0x49f0e;
    const SPEEDS: u32 = 0x4a20a;
    const NEXT_ANGLE: u32 = 0x4a172;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => init(x),
            2 => {
                if (x.u8(0x3) as i8) < 0 {
                    parent(x)
                } else {
                    child(x)
                }
            }
            4 => {
                if (x.u8(0x3) as i8) < 0 {
                    let player = x.linked(f::OWNER);
                    player.set_u16(x.b, 0x15a, 0);
                    player.set_u8(x.b, 0x15c, 0);
                }
                x.free_peff();
            }
            _ => x.free_peff(),
        }
    }

    fn init(x: &mut Cx) {
        if (x.u8(0x3) as i8) < 0 {
            return parent_init(x);
        }
        x.add8(0x4, 2);
        let gen = x.a5_u8(0x34);
        x.set8(g::GENERATION, gen);
        x.set8(g::STEP, 0);
        let variant = x.u8(0x3) as u32;
        let angle = x.rom8(INITIAL_ANGLE + variant);
        x.set8(0x2d, angle);
        set_speeds(x, SPEEDS, 0);
        icon(x);
    }

    /// Animation de l'icône d'après l'équipement (`0x49edc`).
    fn icon(x: &mut Cx) {
        let item = x.u8(g::ITEM) as u32;
        let count = x.u8(g::COUNT) as u32;
        let count = if x.u8(0x2d) != 0 { 0xa } else { count };
        let set = if x.a5_u16(a5::ALT_PALETTE) != 0 { ICONS_ALT } else { ICONS };
        x.anim_start(set, ((item << 4) + count & 0xffff) as u16);
    }

    fn parent_init(x: &mut Cx) {
        if x.a5_u16(a5::PEFF_FREE) <= 6 {
            return;
        }
        x.add8(0x4, 2);
        let gen = x.a5_u8(0x34);
        x.set8(g::GENERATION, gen);
        x.set16(0x3e, 0x258);
        x.set8(g::DELAY, 0x10);
        let player = x.linked(f::OWNER);
        player.set_u8(x.b, 0x15d, 2);
        let mut a2 = x.o.0 + g::CHILDREN;
        for k in 0..6u32 {
            // Glitch d'origine : l'échec de l'allocation n'est pas testé (voir `Pool::alloc_unchecked`).
            let child = x.alloc_unchecked(&pools::PEFF);
            child.set_u8(x.b, 0, 1);
            child.set_u8(x.b, 2, 0);
            child.set_u8(x.b, 3, k as u8);
            child.set_u16(x.b, g::PARENT, x.o.0 as u16);
            x.set16(a2 - x.o.0, child.0 as u16);
            a2 += 2;
            let owner = x.u16(f::OWNER);
            child.set_u16(x.b, f::OWNER, owner);
            let item = player.u8(x.b, 0x140 + k);
            let count = player.u8(x.b, 0x148 + k);
            child.set_u8(x.b, g::ITEM, item);
            child.set_u8(x.b, g::COUNT, count);
        }
    }

    fn parent(x: &mut Cx) {
        match x.u8(0x5) {
            0 => {
                // attente de l'ouverture de l'anneau
                x.add8(g::DELAY, 0xff);
                if x.u8(g::DELAY) == 0 {
                    x.add8(0x5, 2);
                }
            }
            2 => open(x),
            _ => {
                x.add8(g::DELAY, 0xff);
                if x.u8(g::DELAY) == 0 {
                    x.add8(0x4, 2);
                    for_children(x, 6, |x, child| {
                        let v = child.u8(x.b, 4).wrapping_add(2);
                        child.set_u8(x.b, 4, v)
                    });
                }
            }
        }
        parent_follow(x, OFFSETS);
        let owner = x.linked(f::OWNER);
        if parent_alive(x, g::GENERATION, owner) {
            return;
        }
        x.set8(0x4, 4);
    }

    /// L'anneau est ouvert : le bouton fait tourner la rangée d'équipement, la fin referme l'anneau.
    fn open(x: &mut Cx) {
        x.add16(0x3e, 0xffff);
        if x.u16(0x3e) == 0 {
            return close(x);
        }
        if x.a5_u8(a5::PAUSED) != 0 {
            return close(x);
        }
        let player = x.linked(f::OWNER);
        let keys = player.u8(x.b, 0x6e);
        if keys & 0x10 != 0 || keys & 0x80 != 0 {
            return close(x);
        }
        let held = player.u8(x.b, 0x6c);
        if held & 0x40 == 0 {
            return;
        }
        x.add8(0x5, 0xfe);
        x.set8(g::DELAY, 0x10);
        for_children(x, 6, |x, child| child.set_u8(x.b, 5, 2));
        // rotation de la rangée d'équipement (6 emplacements, quantités à +8)
        let last = player.u8(x.b, 0x145);
        let last_count = player.u8(x.b, 0x14d);
        for k in (0..5u32).rev() {
            let (a, b) = (player.u8(x.b, 0x140 + k), player.u8(x.b, 0x148 + k));
            player.set_u8(x.b, 0x141 + k, a);
            player.set_u8(x.b, 0x149 + k, b);
        }
        player.set_u8(x.b, 0x140, last);
        player.set_u8(x.b, 0x148, last_count);
    }

    fn close(x: &mut Cx) {
        x.set8(0x5, 4);
        x.set8(g::DELAY, 8);
        for_children(x, 6, |x, child| child.set_u8(x.b, 5, 6));
    }

    fn child(x: &mut Cx) {
        match x.u8(0x5) {
            0 => {
                x.add8(g::STEP, 1);
                if x.u8(g::STEP) > 4 {
                    x.set8(0x5, 8);
                } else {
                    let step = x.u8(g::STEP);
                    set_speeds(x, SPEEDS, step);
                }
            }
            2 => {
                x.add8(0x5, 2);
                x.linked(g::PARENT);
                let n = x.u8(0x3).wrapping_add(1);
                let n = if n > 5 { 0 } else { n };
                x.set8(0x3, n);
                let target = x.rom8(NEXT_ANGLE + n as u32);
                x.set8(g::TARGET, target);
            }
            4 => {
                let angle = x.u8(0x2d).wrapping_add(1) & 0x1f;
                x.set8(0x2d, angle);
                if angle == x.u8(g::TARGET) {
                    x.add8(0x5, 0xfe);
                    icon(x);
                    let target = x.u8(g::TARGET);
                    x.set8(0x2d, target);
                    let n = x.u8(0x3) as i8 as i16 as u16 as u32;
                    let next = x.rom8(NEXT_ANGLE + n);
                    x.set8(g::TARGET, next);
                    x.set8(0x5, 8);
                }
            }
            6 => {
                if x.u8(g::STEP) == 0 {
                    x.set32(0x14, 0);
                    x.set32(0x18, 0);
                } else {
                    x.add8(g::STEP, 0xff);
                    let step = x.u8(g::STEP);
                    set_speeds(x, SPEEDS, step);
                }
            }
            _ => x.set8(0x5, 8),
        }
        follow_parent(x);
        if !ring_alive(x, g::GENERATION) {
            x.set8(0x4, 4);
        }
        x.reinsert();
    }
}

/// Anneau d'équipement du type 1 (0x4a21e) : variante de 0 qui lit les quantités du joueur.
mod ring1 {
    use super::*;

    const OFFSETS: u32 = 0x4a3ac;
    const SPEEDS: u32 = 0x4a84c;
    const ANGLE_A: u32 = 0x4a296;
    const ANGLE_B: u32 = 0x4a290;
    const ANGLE_C: u32 = 0x4a28c;
    const NEXT_A: u32 = 0x4a6f8;
    const NEXT_B: u32 = 0x4a6f2;
    const NEXT_C: u32 = 0x4a6ee;
    const NEXT2_A: u32 = 0x4a7b4;
    const NEXT2_B: u32 = 0x4a7ae;
    /// Mode de l'anneau : `+0x59` vaut 2 pour la rangée des magies.
    const MODE: u32 = 0x59;
    const GENERATION: u32 = 0x5b;
    const CHILD_COUNT: u32 = 0x3d;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => init(x),
            2 => {
                if (x.u8(0x3) as i8) < 0 {
                    parent(x)
                } else {
                    child(x)
                }
            }
            4 => {
                if (x.u8(0x3) as i8) < 0 {
                    let player = x.linked(f::OWNER);
                    player.set_u16(x.b, 0x15a, 0);
                    player.set_u8(x.b, 0x15c, 0);
                }
                x.free_peff();
            }
            _ => x.free_peff(),
        }
    }

    fn player_is_8_or_9(x: &mut Cx) -> bool {
        let player = x.linked(f::OWNER);
        let v = player.u8(x.b, 0x3);
        v == 8 || v == 9
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        if (x.u8(0x3) as i8) < 0 {
            return parent(x);
        }
        let gen = x.a5_u8(0x34);
        x.set8(GENERATION, gen);
        x.set8(g::STEP, 0);
        let v = x.u8(0x3) as u32;
        let angle = x.rom8(ANGLE_A + v);
        x.set8(0x2d, angle);
        if x.u8(MODE) != 2 {
            let angle = x.rom8(ANGLE_B + v);
            x.set8(0x2d, angle);
            if !player_is_8_or_9(x) {
                let angle = x.rom8(ANGLE_C + v);
                x.set8(0x2d, angle);
            }
        }
        set_speeds(x, SPEEDS, 0);
        icon(x);
    }

    /// Animation de l'icône (`0x4a29c`).
    fn icon(x: &mut Cx) {
        let item = x.u8(g::ITEM) as u32;
        let count = x.u8(g::COUNT) as u32;
        let d0;
        let d1;
        if x.u8(MODE) == 2 {
            d0 = item << 4 & 0xffff;
            d1 = if x.u8(0x2d) != 0 { 0xa } else { count };
        } else {
            let player = x.linked(f::OWNER);
            let kind = player.u8(x.b, 0x3) >> 1;
            let item = if kind == 2 && item == 0xf { 0x57 } else { item };
            let d0w = ((item as u32) << 4) & 0xffff;
            let stock = player.u8(x.b, 0x150 + count);
            d0 = d0w;
            d1 = if x.u8(0x2d) != 0 {
                if stock == 0 { 0xb } else { 0xa }
            } else {
                stock as u32
            };
        }
        let set = if x.a5_u16(a5::ALT_PALETTE) != 0 { ICONS_ALT } else { ICONS };
        x.anim_start(set, (d0 + d1 & 0xffff) as u16);
    }

    fn parent(x: &mut Cx) {
        match x.u8(0x5) {
            0 => create(x),
            2 => {
                x.add8(g::DELAY, 0xff);
                if x.u8(g::DELAY) == 0 {
                    x.add8(0x5, 2);
                }
            }
            4 => rotate_or_close(x),
            6 => {
                x.add8(g::DELAY, 0xff);
                if x.u8(g::DELAY) == 0 {
                    x.set8(0x5, 0);
                    let player = x.linked(f::OWNER);
                    let n = player.u8(x.b, 0x15d).wrapping_add(1);
                    player.set_u8(x.b, 0x15d, n);
                    if n == 3 {
                        player.set_u8(x.b, 0x15d, 0);
                    }
                }
            }
            _ => {
                x.add8(g::DELAY, 0xff);
                if x.u8(g::DELAY) == 0 {
                    x.add8(0x4, 2);
                    let n = x.u8(CHILD_COUNT) as u32 + 1;
                    for_children(x, n, |x, child| {
                        let v = child.u8(x.b, 4).wrapping_add(2);
                        child.set_u8(x.b, 4, v)
                    });
                }
            }
        }
        // suit le joueur
        parent_follow(x, OFFSETS);
        let player = x.linked(f::OWNER);
        if parent_alive(x, GENERATION, player) {
            return;
        }
        x.set8(0x4, 4);
    }

    /// Sous-état 0 du parent : prépare l'anneau (nombre d'enfants et leurs équipements).
    fn create(x: &mut Cx) {
        if x.a5_u16(a5::PEFF_FREE) <= 6 {
            return;
        }
        x.add8(0x5, 2);
        let gen = x.a5_u8(0x34);
        x.set8(GENERATION, gen);
        x.set16(0x40, 0x258);
        x.set8(g::DELAY, 0x10);
        let player = x.linked(f::OWNER);
        let page = player.u8(x.b, 0x15d);
        x.set8(MODE, page);
        let mut count = x.rom8(0x4a474 + page as u32);
        if !matches!(player.u8(x.b, 0x3), 8 | 9) {
            count = x.rom8(0x4a470 + page as u32);
        }
        x.set8(CHILD_COUNT, count);
        let mut a2 = x.o.0 + g::CHILDREN;
        for k in 0..=count as u32 {
            // Glitch d'origine : l'échec de l'allocation n'est pas testé (voir `Pool::alloc_unchecked`).
            let child = x.alloc_unchecked(&pools::PEFF);
            child.set_u8(x.b, 0, 1);
            child.set_u8(x.b, 2, 1);
            child.set_u8(x.b, 3, k as u8);
            child.set_u8(x.b, MODE, page);
            child.set_u16(x.b, g::PARENT, x.o.0 as u16);
            x.set16(a2 - x.o.0, child.0 as u16);
            a2 += 2;
            let owner = x.u16(f::OWNER);
            child.set_u16(x.b, f::OWNER, owner);
            let at = ((page as u32) << 4) + k;
            let item = player.u8(x.b, 0x120 + at);
            let amount = player.u8(x.b, 0x128 + at);
            child.set_u8(x.b, g::ITEM, item);
            child.set_u8(x.b, g::COUNT, amount);
        }
    }

    fn rotate_or_close(x: &mut Cx) {
        x.add16(0x40, 0xffff);
        if x.u16(0x40) == 0 {
            return shut(x, 8);
        }
        if x.a5_u8(a5::PAUSED) != 0 {
            return shut(x, 8);
        }
        let player = x.linked(f::OWNER);
        let keys = player.u8(x.b, 0x6e);
        if keys & 0x40 != 0 {
            return turn(x, player);
        }
        if keys & 0x20 != 0 {
            return shut(x, 6);
        }
        if keys & 0x10 != 0 || keys & 0x80 != 0 {
            return shut(x, 8);
        }
        let held = player.u8(x.b, 0x6c);
        if held & 0x40 != 0 {
            turn(x, player);
        }
    }

    /// Referme l'anneau : tous les enfants en sous-état 6, le parent en `sub`.
    fn shut(x: &mut Cx, sub: u8) {
        x.set8(0x5, sub);
        x.set8(g::DELAY, 8);
        let n = x.u8(CHILD_COUNT) as u32 + 1;
        for_children(x, n, |x, child| child.set_u8(x.b, 5, 6));
    }

    /// Fait tourner la rangée des équipements de la page courante.
    fn turn(x: &mut Cx, player: Obj) {
        x.add8(0x5, 0xfe);
        x.set8(g::DELAY, 0x10);
        let n = x.u8(CHILD_COUNT) as u32 + 1;
        for_children(x, n, |x, child| child.set_u8(x.b, 5, 2));
        let n = x.u8(CHILD_COUNT);
        let base = player.0 + 0x120 + ((x.u8(MODE) as u32) << 4);
        let last = base + n as u32;
        let (keep, keep_count) = (x.data8(last), x.data8(last + 8));
        let mut dst = last;
        let mut src = last - 1;
        for _ in 0..n {
            let (a, b) = (x.data8(src), x.data8(src + 8));
            crate::mem::wr8(x.b, dst, a);
            crate::mem::wr8(x.b, dst + 8, b);
            src = src.wrapping_sub(1);
            dst = dst.wrapping_sub(1);
        }
        crate::mem::wr8(x.b, base, keep);
        crate::mem::wr8(x.b, base + 8, keep_count);
    }

    fn child(x: &mut Cx) {
        match x.u8(0x5) {
            0 => {
                x.add8(g::STEP, 1);
                if x.u8(g::STEP) > 4 {
                    x.set8(0x5, 8);
                } else {
                    let step = x.u8(g::STEP);
                    set_speeds(x, SPEEDS, step);
                }
            }
            2 => {
                x.add8(0x5, 2);
                let parent = x.linked(g::PARENT);
                x.set8(0x56, 1);
                let n = x.u8(0x3).wrapping_add(1);
                let n = if n > parent.u8(x.b, CHILD_COUNT) { 0 } else { n };
                x.set8(0x3, n);
                let mut target = x.rom8(NEXT_A + n as u32);
                x.set8(g::TARGET, target);
                if x.u8(MODE) != 2 {
                    target = x.rom8(NEXT_B + n as u32);
                    x.set8(g::TARGET, target);
                    if !player_is_8_or_9(x) {
                        target = x.rom8(NEXT_C + n as u32);
                        x.set8(g::TARGET, target);
                    }
                }
            }
            4 => {
                let angle = x.u8(0x2d).wrapping_add(1) & 0x1f;
                x.set8(0x2d, angle);
                if angle == x.u8(g::TARGET) {
                    x.add8(0x5, 0xfe);
                    icon(x);
                    let target = x.u8(g::TARGET);
                    x.set8(0x2d, target);
                    let n = x.u8(0x3) as i8 as i16 as u16 as u32;
                    let mut next = x.rom8(NEXT2_A + n);
                    x.set8(g::TARGET, next);
                    x.set8(0x5, 8);
                    if x.u8(MODE) != 2 {
                        next = x.rom8(NEXT2_B + n);
                        x.set8(g::TARGET, next);
                        if !player_is_8_or_9(x) {
                            next = x.rom8(NEXT2_B + n);
                            x.set8(g::TARGET, next);
                        }
                    }
                }
            }
            6 => {
                if x.u8(g::STEP) == 0 {
                    x.set8(0x5, 0xa);
                    x.set8(0x5a, 4);
                    set_speeds(x, SPEEDS, 0);
                } else {
                    x.add8(g::STEP, 0xff);
                    let step = x.u8(g::STEP);
                    set_speeds(x, SPEEDS, step);
                }
            }
            0xa => {
                if x.u8(0x5a) == 0 {
                    x.add8(0x4, 2);
                }
                x.add8(0x5a, 0xff);
            }
            0xc => {
                if x.u8(g::STEP) == 0 {
                    x.set32(0x14, 0);
                    x.set32(0x18, 0);
                } else {
                    x.add8(g::STEP, 0xff);
                    let step = x.u8(g::STEP);
                    set_speeds(x, SPEEDS, step);
                }
            }
            _ => x.set8(0x5, 8),
        }
        follow_parent(x);
        if !ring_alive(x, GENERATION) {
            x.set8(0x4, 4);
        }
        x.reinsert();
    }
}

handlers! {
    0x49e36 "peff_0" ring0::run,
    0x4a21e "peff_1" ring1::run,
}
