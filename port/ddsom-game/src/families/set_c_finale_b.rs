//! Variantes 8 à 24 du type SET 15 (voir `set_c_finale.rs`). Les noms de rôle sont des hypothèses.

use super::*;
use crate::mem::{rd16, rd32, rd8, wr32, A5};

/// Joueurs : quatre enregistrements de 0x200 octets.
const PLAYERS: u32 = A5 + 0x5de;
const PLAYER_STRIDE: u32 = 0x200;
/// Gestionnaire de pause des objets liés (mot A5+0x14 : bits d'activité des tâches de fond).
const TASK_FLAGS: u32 = 0x14;

fn players() -> impl Iterator<Item = u32> {
    (0..4).map(|i| PLAYERS + i * PLAYER_STRIDE)
}

/// Joueur en jeu : actif et état principal (octet `+4`) inférieur à 4.
fn present(x: &mut Cx, p: u32) -> bool {
    rd8(x.b, p) != 0 && (rd8(x.b, p + 4) as i8) < 4
}

/// Libère l'objet au passage suivant : `+4 = 6` et les trois octets suivants à zéro.
fn retire(x: &mut Cx) {
    x.set32(0x4, 0x0600_0000);
}

/// Adresse RAM stockée sur 16 bits (`movea.w` : extension de signe).
fn adr16(v: u16) -> u32 {
    v as i16 as i32 as u32
}

/// Crée un SET de type 15 (variante `variant`) : routine 0xce00a. Rend l'objet ou `None`.
fn spawn_finale(x: &mut Cx, variant: u32) -> Option<Obj> {
    x.c.d[0] = variant;
    x.call(0xce00a);
    (x.c.sr & SR_C == 0).then(|| Obj(x.c.a[1]))
}

/// Crée un SOBJ de type 6 (routine 0xddeaa), variante `variant`.
fn spawn_sobj(x: &mut Cx, variant: u32) -> Option<Obj> {
    x.c.d[0] = variant;
    x.call(rom::SPAWN_SOBJ);
    (x.c.sr & SR_C == 0).then(|| Obj(x.c.a[1]))
}

/// Crée un effet (type 0x1f de 0xb1a7a) : D0 = variante, D1 = valeur de `+0x2f`.
fn spawn_effct(x: &mut Cx, variant: u32, extra: u32) -> Option<Obj> {
    x.c.d[0] = variant;
    x.c.d[1] = extra;
    x.call(rom::SPAWN_EFFCT);
    (x.c.sr & SR_C == 0).then(|| Obj(x.c.a[1]))
}

/// Secousse à quatre composantes (0xce24c) : caméra du fond et du second plan, avec recopie des
/// positions « précédentes ». `table` : enregistrements de 8 octets, un par pas de `+0x18` & 3.
fn shake4(x: &mut Cx, table: u32) {
    let at = table + 8 * (x.u8(0x18) & 3) as u32;
    x.add8(0x18, 1);
    for (i, var) in [a5::CAM_X, a5::CAM_Y, a5::BG_X, a5::BG_Y].into_iter().enumerate() {
        let d = x.data16(at + 2 * i as u32);
        let v = x.a5_u16(var).wrapping_add(d);
        x.set_a5_u16(var, v);
    }
    copy_cameras(x);
}

fn copy_cameras(x: &mut Cx) {
    for (from, to) in [(a5::CAM_X, a5::CAM_X_PREV), (a5::CAM_Y, a5::CAM_Y_PREV), (a5::BG_X, a5::BG_X_PREV), (a5::BG_Y, a5::BG_Y_PREV)] {
        let v = x.a5_u16(from);
        x.set_a5_u16(to, v);
    }
}

fn mask_tasks(x: &mut Cx, keep: u16) {
    let v = x.a5_u16(TASK_FLAGS) & keep;
    x.set_a5_u16(TASK_FLAGS, v);
}

// --- variantes 8 et 23 : remplissage des PV ----------------------------------------------------------

/// Prépare la liste (joueur, réserve) : `+0x30 + 4 i` = adresse du joueur, `+0x32 + 4 i` = PV rendus
/// d'office ; le pas de base dépend du score `A5+0x38`, majoré (variante 23) du `+0x64` du joueur.
fn refill_setup(x: &mut Cx, with_player_bonus: bool) {
    let score = x.a5_u16(0x38) as i16;
    let mut step: u16 = 0x14;
    for (limit, add) in [(0x100, 8), (0x130, 4), (0x200, 8), (0x230, 8)] {
        if score <= limit {
            break;
        }
        step = step.wrapping_add(add);
    }
    let mut slot = 0x30;
    for p in players() {
        if present(x, p) {
            x.set16(slot, p as u16);
            if with_player_bonus {
                step = step.wrapping_add(rd16(x.b, p + 0x64));
            }
            x.set16(slot + 2, step);
            slot += 4;
        }
    }
}

/// Un tour de remplissage : chaque joueur dont les PV (`+0x62`) sont sous le maximum (`+0x60`) en
/// gagne un tant que sa réserve est non vide, puis jusqu'à 1,25 fois la moitié du maximum. Vrai
/// si un joueur a reçu un PV.
fn refill_step(x: &mut Cx, variant_b: bool) -> bool {
    let mut slot = 0x30;
    let mut gave = 0u8;
    let count = (x.a5_u8(0x35) as u16).wrapping_sub(1) as u32 + 1;
    for _ in 0..count {
        let p = adr16(x.u16(slot));
        let max = rd16(x.b, p + 0x60) as i16;
        let cur = rd16(x.b, p + 0x62) as i16;
        if max > cur {
            if x.u16(slot + 2) != 0 {
                gave = gave.wrapping_add(1);
                wr16(x.b, p + 0x62, cur.wrapping_add(1) as u16);
                let left = x.u16(slot + 2).wrapping_sub(1);
                x.set16(slot + 2, left);
            } else {
                let half = (max as u16) >> 1;
                let bonus = if variant_b && x.u8(0x42) == 0 { 0 } else { half >> 2 };
                if (half.wrapping_add(bonus) as i16) > cur {
                    gave = gave.wrapping_add(1);
                    wr16(x.b, p + 0x62, cur.wrapping_add(1) as u16);
                }
            }
        }
        slot += 4;
    }
    gave != 0
}

/// Variante 8 (0xcf3a8) : fin de niveau, les PV remontent.
pub fn hp_refill_a(x: &mut Cx) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            refill_setup(x, false);
        }
        2 => {
            if !refill_step(x, false) {
                next_state(x);
            }
        }
        _ => {}
    }
}

/// Variante 23 (0xd0932) : idem avec le bonus propre à chaque joueur.
pub fn hp_refill_b(x: &mut Cx) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            refill_setup(x, true);
        }
        2 => {
            if !refill_step(x, true) {
                next_state(x);
            }
        }
        _ => {}
    }
}

// --- variantes 9 et 11 : effets attachés aux joueurs ------------------------------------------------------

/// Choisit, pour un effet sans joueur, un numéro de personnage libre dans le masque `taken` :
/// examine les couples (0,1), (2,3)... ; la première paire entièrement libre donne son premier, sinon
/// le premier numéro libre. Le résultat est marqué pris.
fn free_character(taken: &mut u32) -> Option<u32> {
    let bit = |t: u32, n: u32| t >> (n & 31) & 1 != 0;
    let mut n = 0;
    for _ in 0..6 {
        if bit(*taken, n) || bit(*taken, n + 1) {
            n += 2;
        } else {
            *taken |= 1 << n;
            return Some(n);
        }
    }
    let mut n = 0;
    for _ in 0..12 {
        for _ in 0..2 {
            if !bit(*taken, n) {
                *taken |= 1 << n;
                return Some(n);
            }
            n += 1;
        }
    }
    None
}

/// Masque des personnages présents (variante `+3` de chaque joueur en jeu).
fn characters_present(x: &mut Cx) -> u32 {
    let mut mask = 0;
    for p in players() {
        if present(x, p) {
            mask |= 1 << (rd8(x.b, p + 3) & 31);
        }
    }
    mask
}

/// Variante 9 (0xcf476) : lance six effets (un par joueur, puis par personnage absent), prévient le
/// chef d'orchestre (`+8 = 1`), attend la fin, puis lui rend la main (`+8 = 0xff`).
pub fn spawn_player_effects(x: &mut Cx) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            x.set8(0x30, 0);
            player_effects(x);
        }
        2 => match x.u8(0x5) {
            0 => {
                if x.a5_u8(0x48f) != 0 {
                    return;
                }
                handover_step(x, 0)
            }
            2 => handover_step2(x),
            4 => handover_step3(x),
            _ => {}
        },
        _ => {}
    }
}

fn player_effects(x: &mut Cx) {
    let mut taken = characters_present(x);
    let mut a6 = PLAYERS;
    for d1 in (0..=5u32).rev() {
        let here = d1 as i8 > 1 && present(x, a6);
        let Some(fx) = spawn_effct(x, 1, d1) else { break };
        if here {
            fx.set_u16(x.b, 0x50, a6 as u16);
            fx.set_u16(x.b, 0x5c, x.o.0 as u16);
        } else if let Some(n) = free_character(&mut taken) {
            fx.set_u16(x.b, 0x50, n as u16);
            fx.set_u16(x.b, 0x5c, x.o.0 as u16);
        }
        a6 += PLAYER_STRIDE;
    }
    x.set16(0x42, 0x3c);
    let boss = conductor(x);
    boss.set_u8(x.b, 8, 1);
}

/// Premier temps : après le délai `+0x42`, crée l'écran de bonus (type 15 variante 0x17).
fn handover_step(x: &mut Cx, mark: u8) {
    x.add16(0x42, 0xffff);
    if x.u16(0x42) != 0 {
        return;
    }
    if let Some(o) = spawn_finale(x, 0x17) {
        o.set_u8(x.b, 0x42, mark);
    }
    next_substate(x);
    x.set8(0x30, 1);
    x.set8(0x31, 6);
    x.set_a5_u8(0x33, 1);
}

fn handover_step2(x: &mut Cx) {
    if (x.u8(0x31) as i8) >= 0 {
        return;
    }
    x.set_a5_u8(0x33, 0);
    next_substate(x);
    x.set16(0x42, 1);
}

fn handover_step3(x: &mut Cx) {
    x.add16(0x42, 0xffff);
    if x.u16(0x42) != 0 {
        return;
    }
    let boss = conductor(x);
    boss.set_u8(x.b, 8, 0xff);
    x.set8(0x30, 2);
    next_state(x);
}

/// Variante 11 (0xcf6e8) : comme 9 pour l'état 0 ; ensuite elle attend que le chef d'orchestre
/// passe à 2 avant de lancer l'écran de bonus (`+0x42 = 1`).
pub fn handover_effects(x: &mut Cx) {
    match x.u8(0x4) {
        0 => spawn_player_effects(x),
        2 => match x.u8(0x5) {
            0 => {
                if conductor(x).u8(x.b, 8) != 2 {
                    return;
                }
                handover_step(x, 1)
            }
            2 => handover_step2(x),
            4 => handover_step3(x),
            _ => {}
        },
        _ => {}
    }
}

// --- variante 10 : secousse d'ouverture ------------------------------------------------------------------

/// Variante 10 (0xcf5e6) : secousse (table 0xd0b30) jusqu'à la fin du fondu (`A5+0x48f`), puis le fond
/// accélère en défilement horizontal (`+0x14`, `+0x16`) jusqu'à ce que la vitesse tombe sous 0x100,
/// et enfin met le chef d'orchestre à 2. `+5` : 0 secousse, 2 attente du chef, 4 défilement.
pub fn shaking_intro(x: &mut Cx) {
    const SHAKE: u32 = 0xd0b30;
    const SCROLL_SHAKE: u32 = 0xd0b50;
    if x.u8(0x4) == 0 {
        next_state(x);
        x.set8(0x18, 0);
    } else if x.u8(0x4) != 2 {
        return;
    }
    match x.u8(0x5) {
        0 => {
            shake4(x, SHAKE);
            if x.a5_u8(0x48f) != 0 {
                return;
            }
            next_substate(x);
            x.set16(0x14, 0x800);
            x.set16(0x16, 0xfff8);
            x.set_a5_u16(a5::CAM_X, 0);
            x.set_a5_u16(a5::CAM_Y, 0);
            x.set_a5_u16(a5::BG_X, 0x80);
            x.set_a5_u16(a5::BG_Y, 0);
            copy_cameras(x);
        }
        2 => {
            if conductor(x).u8(x.b, 8) == 1 {
                next_substate(x);
            }
            let v = x.a5_u16(a5::CAM_X).wrapping_add(8);
            x.set_a5_u16(a5::CAM_X, v);
            shake4(x, SCROLL_SHAKE);
        }
        4 => {
            if x.u8(0x14) != 0 {
                let speed = x.u16(0x14).wrapping_add(x.u16(0x16));
                x.set16(0x14, speed);
                let v = x.a5_u16(a5::CAM_X).wrapping_add(((speed as i16) >> 8) as u16);
                x.set_a5_u16(a5::CAM_X, v);
                if (v as i16) >= 0x300 {
                    x.set_a5_u16(a5::CAM_X, v & 0xff);
                }
                shake4(x, SCROLL_SHAKE);
            } else {
                next_substate(x);
                x.set_a5_u16(a5::BG_X, 0x80);
                x.set_a5_u16(a5::BG_Y, 0);
                copy_cameras(x);
                conductor(x).set_u8(x.b, 8, 2);
                next_state(x);
            }
        }
        _ => {}
    }
}

// --- variante 12 : marqueurs de joueurs --------------------------------------------------------------------

/// Variante 12 (0xcf782) : crée six objets SOBJ (type 6, variante 0xe), un par joueur puis par
/// personnage absent (comme la variante 9), puis un dernier (variante 0, `+0x2f = 0xb`) à (0x108, 0x100,
/// -0x80). L'objet passe ensuite à l'état 6.
pub fn spawn_player_markers(x: &mut Cx) {
    if !matches!(x.u8(0x4), 0 | 2) {
        return;
    }
    let boss = x.u32(0x4c);
    let mut taken = characters_present(x);
    let mut a6 = PLAYERS;
    for d1 in (0..=5u32).rev() {
        let here = d1 as i8 > 1 && present(x, a6);
        let Some(m) = spawn_sobj(x, 0xe) else {
            break;
        };
        if here {
            m.set_u8(x.b, 0x2f, d1 as u8);
            m.set_u16(x.b, 0x46, a6 as u16);
            m.set_u32(x.b, 0x4c, boss);
        } else if let Some(n) = free_character(&mut taken) {
            m.set_u8(x.b, 0x2f, d1 as u8);
            m.set_u16(x.b, 0x46, n as u16);
            m.set_u32(x.b, 0x4c, boss);
        }
        a6 += PLAYER_STRIDE;
    }
    retire(x);
    let Some(m) = spawn_sobj(x, 0) else { return };
    m.set_u8(x.b, 0x2f, 0xb);
    m.set_u16(x.b, 8, 0x108);
    m.set_u16(x.b, 0x10, 0x100);
    m.set_u16(x.b, 0xc, 0xff80);
    m.set_u32(x.b, 0x4c, boss);
}

// --- variante 13 : feu d'artifice ----------------------------------------------------------------------------

/// Variante 13 (0xcf8a0) : prépare la palette (0x91c280, avec 0xf000f000), puis, tant que le chef
/// d'orchestre ne passe pas à un `+8` négatif, lance de temps à autre un objet SOBJ de variante 0x10
/// (position tirée dans la table 0xcf974) et, une fois, un écran de bonus (type 15 variante 0xf).
pub fn fireworks(x: &mut Cx) {
    const PALETTE_ROM: u32 = 0x253280;
    const PALETTE: u32 = 0x91c280;
    const POSITIONS: u32 = 0xcf974;
    match x.u8(0x4) {
        0 => {
            next_state(x);
            for i in 0..8 {
                let v = x.data32(PALETTE_ROM + 4 * i);
                wr32(x.b, PALETTE + 4 * i, v);
            }
            for i in 0..8 {
                let v = rd32(x.b, PALETTE + 4 * i) | 0xf000_f000;
                wr32(x.b, PALETTE + 4 * i, v);
            }
            x.set8(0x42, 0);
        }
        2 => {}
        _ => return,
    }
    if (conductor(x).u8(x.b, 8) as i8) < 0 {
        return retire(x);
    }
    if rng(x) & 7 != 0 {
        return;
    }
    let Some(o) = spawn_sobj(x, 0x10) else { return };
    let owner = x.u32(0x4c);
    o.set_u32(x.b, 0x4c, owner);
    let pick = ((rng(x) & 7) as u32) << 2;
    let (px, pz) = (x.rom16(POSITIONS + pick), x.rom16(POSITIONS + pick + 2));
    o.set_u16(x.b, 8, px);
    o.set_u16(x.b, 0xc, pz);
    o.set_u32(x.b, 0x10, 0);
    if x.u8(0x42) != 0 {
        return;
    }
    let Some(f) = spawn_finale(x, 0xf) else { return };
    x.set8(0x42, 1);
    let owner = x.u32(0x4c);
    f.set_u32(x.b, 0x4c, owner);
}

// --- variante 14 : éclairs --------------------------------------------------------------------------------------

/// Variante 14 (0xcf994) : crée des éclairs (SOBJ variante 0x11, numéro 0 à 4 tiré sans répétition
/// parmi les bits libres de `+0x30`) tant que le second plan (`A5+0x55a`) n'est pas revenu à zéro ;
/// l'appel sonore 0xea83e retentit de temps à autre.
pub fn lightning(x: &mut Cx) {
    const ORDER: u32 = 0xcfa3c;
    const SOUND: u32 = 0xea83e;
    match x.u8(0x4) {
        0 => {
            next_state(x);
            x.set8(0x30, 1);
        }
        2 => {}
        _ => return,
    }
    if rng(x) & 7 == 0 {
        x.call(SOUND);
    }
    if x.a5_u16(a5::BG_Y) == 0 {
        return next_state(x);
    }
    if x.u8(0x5) == 0 && x.a5_u16(a5::CAM_X) == 0 {
        next_substate(x);
        x.set_bit(0x30, 0, false);
    }
    if x.u8(0x30) == 0x1f {
        return;
    }
    if x.a5_u16(0x6e) & 0xf != 0 {
        return;
    }
    let Some(o) = spawn_sobj(x, 0x11) else { return };
    let pick = (rng(x) & 0xf) as u32;
    let mut bit = x.rom8(ORDER + pick);
    while x.u8(0x30) >> (bit & 7) & 1 != 0 {
        bit = bit.wrapping_sub(1);
        if bit & 0x80 != 0 {
            bit = 4;
        }
    }
    x.set_bit(0x30, bit & 7, true);
    o.set_u8(x.b, 0x2f, bit);
    o.set_u16(x.b, 0x46, x.o.0 as u16);
}

// --- variante 15 : secousse légère -----------------------------------------------------------------------------------

/// Variante 15 (0xcfa4c) : secoue le fond (table 0xd0b70) tant que le chef d'orchestre a `+8 >= 0`.
pub fn quake_light(x: &mut Cx) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            x.set8(0x18, 0);
        }
        2 => {}
        _ => return,
    }
    if (conductor(x).u8(x.b, 8) as i8) < 0 {
        return retire(x);
    }
    shake4(x, 0xd0b70);
}

// --- variante 16 : liste d'arrivée (h) ------------------------------------------------------------------------------------

/// Variante 16 (0xcfa86) : un effet (type 0x1f variante 7) par joueur présent, espacés de 0xb4
/// images et pris dans un ordre aléatoire parmi 16 numéros (masque `+0x20`) ; `+0x22` compte les
/// effets lancés, `+0x23` ceux encore vivants. Quand tout est parti, passe à l'état 4 et avance
/// `A5+0x2d`.
pub fn ranking_start(x: &mut Cx) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            let mut taken: u32 = 0xf000;
            x.set8(0x22, 0);
            x.set8(0x23, 0);
            for p in players() {
                if present(x, p) {
                    taken |= 1 << (rd8(x.b, p + 3) & 31);
                    x.add8(0x22, 1);
                }
            }
            x.set16(0x20, taken as u16);
            x.set16(0x24, 0xb4);
            let mut left = x.u8(0x22);
            x.add8(0x22, 0xff);
            loop {
                left = left.wrapping_sub(1);
                if left == 0 {
                    break;
                }
                x.add16(0x24, 0xb4);
            }
        }
        2 => {
            if x.u8(0x5) != 0 {
                if x.u8(0x23) != 0 {
                    return;
                }
                next_state(x);
                let v = x.a5_u8(0x2d).wrapping_add(2);
                x.set_a5_u8(0x2d, v);
                return;
            }
            x.add16(0x24, 0xffff);
            if x.u16(0x24) != 0 {
                return;
            }
            x.set16(0x24, 0xb4);
            let mut taken = x.u16(0x20) as u32;
            let mut n = (rng(x) & 0xf) as u32;
            while taken >> n & 1 != 0 {
                n = (n + 1) & 0xf;
            }
            taken |= 1 << n;
            x.set16(0x20, taken as u16);
            let Some(fx) = spawn_effct(x, 7, n) else { return };
            fx.set_u8(x.b, 0x2f, n as u8);
            let order = x.u8(0x22);
            fx.set_u8(x.b, 0x42, order);
            fx.set_u16(x.b, 0x46, x.o.0 as u16);
            x.add8(0x22, 1);
            x.add8(0x23, 1);
            if x.u16(0x20) == 0xffff {
                next_substate(x);
            }
        }
        _ => {}
    }
}

// --- variante 17 : fondu de palette ---------------------------------------------------------------------------------------

/// Variante 17 (0xcfb76) : fondu des trois lignes de palette 0x91c1e0, 0x91c300, 0x91c400 : `+5` = 0
/// éclaircit 15 fois (+0x1000 sur deux mots de chaque ligne toutes les 4 images), 2 attend `+0x42`,
/// 4 assombrit de la même façon puis met `+0x32 = 1` à l'objet `+0x46`.
pub fn palette_fade(x: &mut Cx) {
    const LINES: [u32; 3] = [0x91c1e0, 0x91c300, 0x91c400];
    if x.u8(0x4) == 0 {
        next_state(x);
        x.set8(0x20, 4);
        x.set8(0x21, 0xf);
    } else if x.u8(0x4) != 2 {
        return;
    }
    let fade = |x: &mut Cx, delta: u32| {
        for line in LINES {
            for i in 0..8 {
                let v = rd32(x.b, line + 4 * i).wrapping_add(delta);
                wr32(x.b, line + 4 * i, v);
            }
        }
    };
    match x.u8(0x5) {
        0 => {
            x.add8(0x20, 0xff);
            if x.u8(0x20) != 0 {
                return;
            }
            x.set8(0x20, 4);
            fade(x, 0x1000_1000);
            x.add8(0x21, 0xff);
            if x.u8(0x21) != 0 {
                return;
            }
            next_substate(x);
            x.set8(0x21, 0xf);
        }
        2 => {
            x.add16(0x42, 0xffff);
            if x.u16(0x42) == 0 {
                next_substate(x);
            }
        }
        4 => {
            x.add8(0x20, 0xff);
            if x.u8(0x20) != 0 {
                return;
            }
            x.set8(0x20, 4);
            fade(x, 0xefff_f000);
            x.add8(0x21, 0xff);
            if x.u8(0x21) != 0 {
                return;
            }
            next_state(x);
            let target = adr16(x.u16(0x46));
            wr16(x.b, target + 0x32, 1);
        }
        _ => {}
    }
}

// --- variante 18 : choix des bonus de fin de niveau ----------------------------------------------------------------

/// Variante 18 (0xcfc56) : choisit, joueur après joueur, un bonus parmi des candidats (tests de la
/// table 0xcff10, selon le personnage et le tour), affiche la récompense (création d'un SET par
/// 0xc8312), anime la palette du joueur et recommence jusqu'à avoir servi tous les joueurs (`+0x25`).
pub fn bonus_selection(x: &mut Cx) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            x.set32(0x20, 0xff00_0000);
            x.set8(0x24, 0);
            let mut n = 0;
            for p in players() {
                if present(x, p) {
                    n += 1;
                }
            }
            x.set8(0x25, n);
        }
        2 => bonus_step(x),
        _ => {}
    }
}

const REWARD_PALETTE_ROM: u32 = 0xcfdd2;

fn bonus_step(x: &mut Cx) {
    match x.u8(0x5) {
        0 => {
            x.c.d[0] = 0x94;
            x.call(0xc8312);
            if x.c.sr & SR_C != 0 {
                return;
            }
            next_substate(x);
            let o = Obj(x.c.a[1]);
            x.set16(0x46, o.0 as u16);
            o.set_u16(x.b, 0x46, 0);
            for p in players() {
                if present(x, p) && rd8(x.b, p + 0x18a) != 0 {
                    o.set_u16(x.b, 0x46, p as u16);
                    return;
                }
            }
        }
        2 | 0xa => {
            let reward = adr16(x.u16(0x46));
            if rd8(x.b, reward + 0x22) == 0 {
                return;
            }
            next_substate(x);
            for i in 0..8 {
                wr32(x.b, 0x912224 + 0x80 * i, 0x20_0000);
            }
        }
        4 => {
            let index = x.u8(0x24) as u16;
            x.add8(0x24, 1);
            let mut a1 = PLAYERS.wrapping_add(adr16(index.rotate_right(7)));
            while !present(x, a1) {
                x.add8(0x24, 1);
                a1 = a1.wrapping_add(PLAYER_STRIDE);
            }
            next_substate(x);
            x.set16(0x26, a1 as u16);
            x.set8(0x2b, 8);
            x.set8(0x2a, 4);
        }
        6 => palette_pulse(x, 0x1000_1000),
        0xc => palette_pulse(x, 0xefff_f000),
        8 => choose_reward(x),
        0xe => {
            x.add8(0x25, 0xff);
            if x.u8(0x25) == 0 {
                next_substate(x);
            } else {
                x.set8(0x5, 4);
            }
        }
        0x10 => {
            next_state(x);
            let v = x.a5_u8(0x2d).wrapping_add(2);
            x.set_a5_u8(0x2d, v);
            x.set_a5_u16(0x2e, 0);
        }
        _ => {}
    }
}

/// Fait varier huit longs de la palette du joueur courant (`+0x26`) toutes les 4 images, 8 fois.
fn palette_pulse(x: &mut Cx, delta: u32) {
    x.add8(0x2a, 0xff);
    if x.u8(0x2a) != 0 {
        return;
    }
    x.set8(0x2a, 4);
    let player = adr16(x.u16(0x26));
    let character = rd8(x.b, player + 3) as u32;
    let offset = adr16(x.rom16(REWARD_PALETTE_ROM + 2 * character));
    let base = 0x91c000u32.wrapping_add(offset);
    for i in 0..8 {
        let v = rd32(x.b, base + 4 * i).wrapping_add(delta);
        wr32(x.b, base + 4 * i, v);
    }
    x.add8(0x2b, 0xff);
    if x.u8(0x2b) == 0 {
        x.set8(0x2b, 8);
        next_substate(x);
    }
}

/// Un candidat est écarté (vrai) selon le test que la table 0xcff10 associe au couple (personnage,
/// rang) : les listes d'objets déjà possédés, la place dans l'inventaire (+0x18a, +0x114...).
fn reward_blocked(x: &mut Cx, player: u32, rank: u32) -> bool {
    const TESTS: u32 = 0xcff10;
    let character = (rd8(x.b, player + 3) & !1) as u32;
    let index = (character << 2) + 2 * rank;
    let target = TESTS + x.rom16(TESTS + index) as u32;
    // listes d'objets (octets, fin = négatif) comparées à un octet du joueur
    let listed = |x: &mut Cx, list: u32, value: u8| {
        let mut p = list;
        loop {
            let b = x.data8(p);
            p += 1;
            if b as i8 <= -1 && b & 0x80 != 0 {
                return false;
            }
            if b == value {
                return true;
            }
        }
    };
    match target {
        0xcff40 | 0xcff7c => {
            let (a, b) = if target == 0xcff40 { (0xcffb8, 0xcffbe) } else { (0xcffc4, 0xcffca) };
            let (v1, v2) = (rd8(x.b, player + 0x10d), rd8(x.b, player + 0x10e));
            if listed(x, a, v1) || listed(x, a, v2) {
                return false;
            }
            for i in 0..6 {
                let v = rd8(x.b, player + 0x140 + i);
                if listed(x, b, v) {
                    return false;
                }
            }
            true
        }
        0xcfff6 => rd8(x.b, player + 0x18a) == 0,
        0xd0006 => (rd32(x.b, player + 0x114) as i32) < 0x3000,
        0xd0016 => x.a5_u8(0x5d) == 0,
        0xcffdc => !(0..0x66).any(|i| rd8(x.b, 0xff2478 + i) == 0),
        _ => false,
    }
}

fn choose_reward(x: &mut Cx) {
    const REWARD_IDS: u32 = 0xcfe98;
    const REWARD_ICONS: u32 = 0xcfec8;
    let player = adr16(x.u16(0x26));
    let character = rd8(x.b, player + 3) as u32;
    let mut taken = x.u32(0x20);
    let mut candidate = character & !1;
    candidate = (candidate << 1) & 0xffff;
    let mut rank = 0u32;
    loop {
        let blocked = taken >> (candidate & 31) & 1 != 0 || reward_blocked(x, player, rank);
        if !blocked {
            break;
        }
        candidate += 1;
        rank += 1;
    }
    taken |= 1 << (candidate & 31);
    taken &= 0xff77_7777;
    x.set32(0x20, taken);
    x.c.d[1] = candidate;
    let id = x.rom16(REWARD_IDS + 2 * candidate);
    x.c.d[0] = id as u32;
    x.call(0xc8312);
    if x.c.sr & SR_C != 0 {
        return;
    }
    next_substate(x);
    let o = Obj(x.c.a[1]);
    x.set16(0x46, o.0 as u16);
    let chosen = x.u16(0x26);
    o.set_u16(x.b, 0x46, chosen);
    let low = candidate & 3;
    let icons = REWARD_ICONS.wrapping_add((candidate & 0x1c) << 1);
    let mut at = 0x912224;
    for i in 0..7 {
        let tile = x.data8(icons + i) as u16;
        wr16(x.b, at, tile);
        wr16(x.b, at + 2, 0);
        at += 0x80;
    }
    wr16(x.b, at, (low + 1) as u16);
    wr16(x.b, at + 2, 0);
    for i in 0..8 {
        let v = rd32(x.b, 0x91c400 + 4 * i) & 0x0fff_0fff;
        wr32(x.b, 0x91c400 + 4 * i, v);
    }
}

// --- variante 19 : décompte en chiffres ---------------------------------------------------------------------------------

/// Variante 19 (0xd004a) : l'objet porte un nombre en BCD dans `+0x4c` (4 chiffres de 4 bits en
/// octets 0x100, 0x1000, 0x10000, 0x100000) ; chaque image retire une unité du chiffre de poids
/// faible non nul et lance l'événement 0x19ae6 (son/score) pour ce chiffre ; à zéro, se libère.
pub fn countdown_digits(x: &mut Cx) {
    let mut value = x.u32(0x4c);
    if value == 0 {
        return x.call(sub::FREE_SET);
    }
    let (sub, event) = if value & 0xf00 != 0 {
        (0x100, 4)
    } else if value & 0xf000 != 0 {
        (0x1000, 0xe)
    } else if value & 0xf_0000 != 0 {
        (0x1_0000, 0x21)
    } else {
        (0x10_0000, 0x28)
    };
    value = value.wrapping_sub(sub);
    x.set32(0x4c, value);
    x.c.d[1] = (x.u8(0x2f) as u32) << 8;
    x.c.d[0] = event | (x.u8(0x2f) as u32) << 8;
    x.call(0x19ae6);
}

// --- variante 20 : défilement des crédits ---------------------------------------------------------------------------------

/// Variante 20 (0xd00c2) : texte défilant (h : crédits) écrit ligne par ligne dans la tilemap
/// 0x910000 à partir du texte en ROM 0xd01de : un octet par case (0xfe : fin, 0xff n : attribut n,
/// 0x2f : fin de page). Le fond (`A5+0x4ba`) défile de 0x4000/65536 pixel par image.
pub fn credits_scroll(x: &mut Cx) {
    const TEXT: u32 = 0xd01de;
    if x.u8(0x4) == 6 {
        return x.call(sub::FREE_SET);
    }
    if x.u8(0x4) == 0 {
        next_state(x);
        x.call(0xca8);
        x.set_a5_u8(0x48e, 2);
        x.call(0x1686);
        x.set32(0x20, 0);
        x.set32(0x24, TEXT);
        x.set8(0x29, 0);
        x.set8(0x2a, 0);
        x.set16(0x2c, 0xf00);
        x.set8(0x2e, 0);
    }
    let v = x.u32(0x20).wrapping_sub(0x4000);
    x.set32(0x20, v);
    let scroll = x.u16(0x20) & 0x1ff;
    x.set16(0x20, scroll);
    x.set_a5_u16(0x4ba, scroll);
    if x.u8(0x5) != 0 {
        return;
    }
    let phase = (x.a5_u16(0x4ba) & 8) as u8;
    if x.u8(0x2a) == phase {
        return;
    }
    let toggled = x.u8(0x2a) ^ 8;
    x.set8(0x2a, toggled);
    let mut text = x.u32(0x24);
    let mut dest = 0x910000u32.wrapping_add(adr16(x.u16(0x2c)));
    x.add16(0x2c, 4);
    x.add8(0x29, 1);
    if x.u8(0x29) == 0x20 {
        let v = x.u16(0x2c).wrapping_sub(0x80);
        x.set16(0x2c, v);
        x.set8(0x29, 0);
        let v = x.u16(0x2c).wrapping_add(0x2000) & 0x3fff;
        x.set16(0x2c, v);
    }
    let mut attr = x.u8(0x2e) as u16;
    // case effacée en même temps : celle de la page opposée (0x2000 plus loin), fixée une fois
    let erase = 0x910000u32 + ((dest as u16).wrapping_add(0x2000) & 0x3fff) as u32;
    loop {
        let c = x.data8(text);
        text += 1;
        match c {
            0xfe => return next_substate(x),
            0xff => {
                let a = x.data8(text);
                text += 1;
                x.set8(0x2e, a);
                attr = a as u16;
            }
            0x2f => {
                x.set32(0x24, text);
                for i in 0..17 {
                    wr32(x.b, erase + 0x80 * i, 0x20_0000);
                }
                return;
            }
            _ => {
                wr16(x.b, dest, c as u16);
                wr16(x.b, dest + 2, attr);
                dest += 0x80;
            }
        }
    }
}

// --- variante 21 : scintillement ---------------------------------------------------------------------------------------------

/// Variante 21 (0xd06e6) : fait scintiller une couleur de la palette (0x91cfb2 prend 0xf00 de temps
/// à autre) et lance un objet SOBJ de variante 0x14 ; se retire quand le chef d'orchestre a `+8 < 0`.
pub fn flicker(x: &mut Cx) {
    const COLOR: u32 = 0x91cfb2;
    const REFERENCE: u32 = 0x91cc00;
    match x.u8(0x4) {
        0 => {
            next_state(x);
            x.set8(0x20, 0);
            let v = rd16(x.b, COLOR);
            x.set16(0x22, v);
        }
        2 => {}
        _ => return,
    }
    if (conductor(x).u8(x.b, 8) as i8) < 0 {
        return x.set8(0x4, 6);
    }
    if x.u8(0x20) != 0 {
        x.set8(0x20, 0);
        let v = x.u16(0x22) & 0xfff | rd16(x.b, REFERENCE) & 0xf000;
        return wr16(x.b, COLOR, v);
    }
    if rng(x) & 7 != 0 {
        return;
    }
    x.set8(0x20, 1);
    let v = 0xf00 | rd16(x.b, REFERENCE) & 0xf000;
    wr16(x.b, COLOR, v);
    let Some(o) = spawn_sobj(x, 0x14) else { return };
    let owner = x.u32(0x4c);
    o.set_u32(x.b, 0x4c, owner);
}

// --- variante 22 : saisie des initiales (h) ---------------------------------------------------------------------------------------------

/// Variante 22 (0xd077c) : passe en revue les joueurs présents, un après l'autre (`+0x20` = joueur,
/// `+0x22` = joueurs restants) ; pour chacun crée un SOBJ de variante 0x15 qui affiche son nom
/// (liste de tuiles à `+0x118` du joueur, écrite par 0xb230e sur un fond de 0x912000), attend 0x78
/// images, retire l'objet et passe au suivant. Termine par 0xca8 et 0xcc8.
pub fn name_entry(x: &mut Cx) {
    if x.u8(0x4) == 0 {
        next_state(x);
        x.set16(0x20, PLAYERS as u16);
        x.set8(0x22, 4);
    } else if x.u8(0x4) != 2 {
        return;
    }
    match x.u8(0x5) {
        0 => {
            let mut a1 = adr16(x.u16(0x20));
            loop {
                if x.u8(0x22) == 0 {
                    next_state(x);
                    let v = x.a5_u8(0x2d).wrapping_add(2);
                    x.set_a5_u8(0x2d, v);
                    x.call(0xca8);
                    return x.call(0xcc8);
                }
                x.add8(0x22, 0xff);
                if rd8(x.b, a1) != 0 && (rd8(x.b, a1 + 4) as i8) < 4 {
                    next_substate(x);
                    x.set16(0x20, a1 as u16);
                    mask_tasks(x, 0xfff1);
                    return;
                }
                a1 += PLAYER_STRIDE;
            }
        }
        2 => {
            let player = adr16(x.u16(0x20));
            let Some(o) = spawn_sobj(x, 0x15) else { return };
            x.call(0xca8);
            next_substate(x);
            x.set16(0x24, o.0 as u16);
            let character = rd8(x.b, player + 3);
            o.set_u8(x.b, 0x2f, character);
            let dest = 0x912000u32.wrapping_add(x.rom16(0xd0858 + 2 * character as u32) as i16 as i32 as u32);
            let kind = rd8(x.b, player + 2) as u32;
            x.c.a[1] = dest;
            x.c.d[1] = x.rom8(0xd0854 + kind) as u32;
            write_name(x, player + 0x118);
            block_loader_for(x, character as u32);
        }
        4 => {
            next_substate(x);
            let v = x.a5_u16(TASK_FLAGS) | 0xe;
            x.set_a5_u16(TASK_FLAGS, v);
            x.set16(0x26, 0x78);
        }
        6 | 0xa => {}
        8 => {
            x.add16(0x26, 0xffff);
            if x.u16(0x26) == 0 {
                next_substate(x);
            }
        }
        0xc => {
            next_substate(x);
            let o = Obj(adr16(x.u16(0x24)));
            o.set_u32(x.b, 4, 0x0600_0000);
        }
        0xe => {
            next_substate(x);
            let next = adr16(x.u16(0x20)) + PLAYER_STRIDE;
            x.set16(0x20, next as u16);
            mask_tasks(x, 0xfff1);
            x.set16(0x26, 4);
        }
        0x10 => {
            x.add16(0x26, 0xffff);
            if x.u16(0x26) == 0 {
                x.set8(0x5, 0);
            }
        }
        _ => {}
    }
}

/// Écrit le nom (sous-routine 0xd0870) : liste d'indices de tuiles à `list` (fin 0xff ; au-delà de
/// 0xf8, un motif de six tuiles consécutives d'une table), chaque tuile rendue par 0xb230e à A1 avec
/// l'attribut D1.
fn write_name(x: &mut Cx, list: u32) {
    const TILES: u32 = 0xb2920;
    const PATTERNS: u32 = 0xb2a24;
    let mut p = list;
    if x.data8(p) >= 0xf9 {
        let code = (x.data8(p) as i8 as i16).wrapping_neg().wrapping_sub(2) as u16;
        let mut tile = x.data16(PATTERNS.wrapping_add(code.wrapping_mul(2) as i16 as i32 as u32));
        for _ in 0..6 {
            x.c.d[0] = tile as u32;
            x.call(0xb230e);
            // 0xb230e laisse D0 = tuile + 0x11 ; le motif avance de deux (D0 - 0xf)
            tile = tile.wrapping_add(2);
        }
        return;
    }
    loop {
        let c = x.data8(p);
        p += 1;
        if c == 0xff {
            return;
        }
        let tile = x.data16(TILES + 2 * c as u32);
        x.c.d[0] = tile as u32;
        x.call(0xb230e);
    }
}

// --- variante 24 : tableau des résultats ----------------------------------------------------------------------------------------

/// Variante 24 (0xd0a0c) : prépare le fond des résultats (blocs 4x4 à 0x91ba00, copie de la palette
/// 0x91cc00 dans l'objet, fond à 0x200), puis fait clignoter la palette et lance de temps à autre un
/// SOBJ (variante 0x16) lié à l'objet. L'état 4 éteint le clignotement.
pub fn results_panel(x: &mut Cx) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            x.set_a5_u16(0x56e, 4);
            x.c.d[0] = 8;
            load_blocks(x, rom::LOAD_SMALL, 0x91ba00, 8);
            x.c.d[0] = 8;
            load_next(x, rom::LOAD_SMALL, 8);
            for i in 0..8 {
                let v = rd32(x.b, 0x91cc00 + 4 * i);
                x.set32(0x30 + 4 * i as u32, v);
            }
            x.set8(0x20, 0);
            x.set_a5_u16(a5::BG_X, 0x200);
            x.set_a5_u16(a5::BG_Y, 0);
            let (px, py) = (x.a5_u16(a5::BG_X), x.a5_u16(a5::BG_Y));
            x.set_a5_u16(a5::BG_X_PREV, px);
            x.set_a5_u16(a5::BG_Y_PREV, py);
            x.set8(0x20, 1);
            let low = rd32(x.b, 0x91cc00) & 0xf000_f000;
            for i in 0..8 {
                wr32(x.b, 0x91cc00 + 4 * i, 0x0f00_0f00 | low);
            }
            results_tick(x)
        }
        2 => results_tick(x),
        4 => {
            next_state(x);
            fade_flag(x)
        }
        _ => {}
    }
}

fn fade_flag(x: &mut Cx) {
    x.add8(0x20, 0xff);
    mask_tasks(x, 0xfff7);
}

fn results_tick(x: &mut Cx) {
    if x.u8(0x20) != 0 {
        return fade_flag(x);
    }
    x.set8(0x20, 2);
    if rng(x) & 7 != 0 {
        return;
    }
    let v = x.a5_u16(TASK_FLAGS) | 8;
    x.set_a5_u16(TASK_FLAGS, v);
    let Some(o) = spawn_sobj(x, 0x16) else { return };
    o.set_u16(x.b, 0x46, x.o.0 as u16);
}
