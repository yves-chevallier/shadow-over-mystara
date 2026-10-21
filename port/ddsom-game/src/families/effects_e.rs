//! Famille EFFCT : objets de scène qui commandent les joueurs (table des types `0x39ce0`).
//!
//! - 11 : effets des attaques d'un ennemi (éclats, reflets, colonnes, pièces, étincelles) ; variantes 0,
//!   2, 5, 8 et 9 jamais exercées : code d'origine ;
//! - 13 : radeau qui défile et emporte les joueurs (les directions des joueurs le pilotent) ;
//! - 14 : passage vers une sous-scène, qui oscille, attend un joueur, puis lance la transition
//!   (rideau de tuiles dans le fond 2) ; la transition elle-même (`0xada96`, `0xae11a` sous-état 0, une
//!   dizaine d'appels par scénario) reste en code d'origine.

use super::cx::{handlers, Cx};
use crate::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use crate::objects::Obj;

mod f {
    pub const STATE: u32 = 0x04;
    pub const SUB_STATE: u32 = 0x05;
    pub const VARIANT: u32 = 0x03;
}

const FREE_EFFECT: u32 = 0x2218;
const ANIM_SET: u32 = 0x1c03a6;
/// Joueurs : quatre enregistrements de 0x200 octets.
const PLAYERS: u32 = A5 + 0x5de;

fn players() -> impl DoubleEndedIterator<Item = Obj> {
    (0..4).map(|k| Obj(PLAYERS + 0x200 * k))
}

fn free(x: &mut Cx) {
    x.call(FREE_EFFECT);
}

/// Préambule commun : le crochet de pause (0x29ea, Z faux : objet suspendu), la pause du jeu, et au
/// moins un joueur présent dans un état ≤ 2. Vrai si le gestionnaire continue.
fn scene_running(x: &mut Cx, suspended: impl FnOnce(&mut Cx)) -> bool {
    x.call(super::cx::sub::PAUSE_HOOK);
    if x.c.sr & m68k::SR_Z == 0 {
        suspended(x);
        return false;
    }
    if x.a5_u8(0x3d) != 0 {
        return false;
    }
    players().any(|p| p.is_active(x.b) && (p.u8(x.b, f::STATE) as i8) <= 2)
}

// ---------------------------------------------------------------------------------------------
// Type 13 : radeau
// ---------------------------------------------------------------------------------------------

mod raft {
    /// Vitesse de défilement (long, 16.16).
    pub const SPEED: u32 = 0x58;
    /// Mode de la caméra (`A5+0x51e`) : 4 = caméra fixée, le radeau ne se pilote plus.
    pub const CAMERA_MODE: u32 = 0x51e;
}

fn raft(x: &mut Cx) {
    if !scene_running(x, |x| x.set8(f::STATE, 4)) {
        return;
    }
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set32(raft::SPEED, 0x4_0000);
            x.set16(Obj::Z, 0);
            x.anim_start(ANIM_SET, 0);
            x.reinsert();
        }
        2 => {
            if x.u8(f::SUB_STATE) == 0 {
                let v = x.u32(raft::SPEED);
                x.add32(Obj::X, v);
                if x.a5_u8(0x2c) == 0xc {
                    x.add8(f::SUB_STATE, 2);
                }
                x.anim_step();
                return;
            }
            raft_drift(x);
            x.anim_step();
            x.reinsert();
        }
        4 => x.set8(f::STATE, 2),
        _ => free(x),
    }
}

/// Les directions des joueurs présents accélèrent le radeau (haut/bas : profondeur ±1 ou ±0.19 au-delà
/// de ±1 ; droite/gauche : x ±1.19 ou ±0.25), puis les joueurs sont posés dessus, à 0x48 près en x.
fn raft_drift(x: &mut Cx) {
    // D4 de l'original : sans pilotage (caméra fixée), la valeur laissée dans le registre
    let mut dx = x.c.d[4] as i32;
    if x.a5_u16(raft::CAMERA_MODE) != 4 {
        let (mut dy, mut sx) = (0i32, 0i32);
        for (k, p) in players().enumerate() {
            if !p.is_active(x.b) {
                continue;
            }
            let input = rd8(x.b, A5 + k as u32);
            if input & 8 != 0 {
                dy = dy.wrapping_add(if dy >= 0x1_0000 { 0x3000 } else { 0x1_0000 });
            }
            if input & 4 != 0 {
                dy = dy.wrapping_sub(if dy <= -0x1_0000 { 0x3000 } else { 0x1_0000 });
            }
            if input & 1 != 0 {
                sx = sx.wrapping_add(if sx >= 0x1_3000 { 0x4000 } else { 0x1_3000 });
            }
            if input & 2 != 0 {
                sx = sx.wrapping_sub(if sx <= -0x1_3000 { 0x4000 } else { 0x1_3000 });
            }
        }
        dx = sx.wrapping_add(x.u32(raft::SPEED) as i32);
        x.add32(Obj::X, dx as u32);
        x.add32(Obj::Y, dy as u32);
        x.c.d[4] = dx as u32;
        x.call(super::rom::OMBRE);
    }
    let (rx, ry) = (x.u16(Obj::X), x.u16(Obj::Y));
    for p in players().rev() {
        if !p.is_active(x.b) {
            continue;
        }
        p.set_u16(x.b, Obj::Y, ry.wrapping_sub(0x12));
        let px = p.u32(x.b, Obj::X).wrapping_add(dx as u32);
        p.set_u32(x.b, Obj::X, px);
        let px = p.u16(x.b, Obj::X) as i16;
        let right = rx.wrapping_add(0x48);
        let left = rx.wrapping_sub(0x48);
        if (right as i16) < px {
            p.set_u16(x.b, Obj::X, right);
        } else if (left as i16) > px {
            p.set_u16(x.b, Obj::X, left);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 14 : passage vers une sous-scène
// ---------------------------------------------------------------------------------------------

mod gate {
    /// Jeux d'animations par mode `+0x2f` (longs, espace programme).
    pub const ANIM_SETS: u32 = 0xad954;
    /// Oscillation : demi-période (`+0x42`), compteur (`+0x5e`), active (`+0x5f`).
    pub const PERIOD: u32 = 0x42;
    pub const SWING: u32 = 0x43;
    pub const COUNTER: u32 = 0x5e;
    pub const MOVING: u32 = 0x5f;
    /// Étape du rideau (mot).
    pub const CURTAIN: u32 = 0x5c;
    /// Rideau : (décalage dans les tuiles, adresse) par étape, 6 octets (données) ; tuiles (données).
    pub const CURTAIN_STEPS: u32 = 0xadad0;
    pub const CURTAIN_TILES: u32 = 0xadbf2;
    /// Suite de la transition (code d'origine).
    pub const ENTER: u32 = 0xada96;
    pub const LEAVE: u32 = 0xae11a;
}

fn gate(x: &mut Cx) {
    if !scene_running(x, |_| {}) {
        return;
    }
    match x.u8(f::STATE) {
        0 => gate_init(x),
        2 => {
            if x.u8(f::SUB_STATE) != 0 {
                return gate_curtain(x);
            }
            gate_watch(x);
        }
        4 => {
            if x.u8(f::SUB_STATE) == 0 {
                return x.call(gate::LEAVE);
            }
            gate_open(x);
        }
        _ => free(x),
    }
}

fn gate_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    let period = x.u8(gate::PERIOD);
    x.set8(gate::MOVING, period);
    x.set8(gate::COUNTER, period >> 1);
    x.set16(Obj::VX, 0);
    x.set16(Obj::VZ, 0);
    // bit 4 de `+0x43` : oscillation en x, sinon en profondeur
    let swing = x.u8(gate::SWING);
    if swing & 0x10 != 0 {
        x.set8(Obj::VX + 1, swing & !0x10);
    } else {
        x.set8(Obj::VZ + 1, swing);
    }
    if x.u8(gate::MOVING) != 0 {
        x.call(0xdf688);
    }
    let mode = x.u8(0x2f) as u32;
    if mode == 0 {
        let v = x.u8(0x2a) | 1;
        x.set8(0x2a, v);
        x.set8(0x2b, 0x19);
    }
    let set = x.rom16(gate::ANIM_SETS + 4 * mode) as u32;
    let set = set << 16 | x.rom16(gate::ANIM_SETS + 4 * mode + 2) as u32;
    x.anim_start(set, 0);
    x.reinsert();
}

/// 0xae3bc : oscillation (vitesses `+0x14` en x, `+0x18` en profondeur, inversées chaque demi-période).
fn gate_swing(x: &mut Cx) {
    if x.u8(gate::MOVING) == 0 {
        return;
    }
    let (vx, vy) = (x.u16(Obj::VX), x.u16(Obj::VZ));
    x.add16(Obj::X, vx);
    x.add16(Obj::Y, vy);
    let n = x.u8(gate::COUNTER).wrapping_sub(1);
    x.set8(gate::COUNTER, n);
    if (n as i8) >= 0 {
        return;
    }
    let period = x.u8(gate::PERIOD);
    x.set8(gate::COUNTER, period);
    x.set16(Obj::VX, vx.wrapping_neg());
    x.set16(Obj::VZ, vy.wrapping_neg());
}

fn gate_watch(x: &mut Cx) {
    x.anim_step();
    gate_swing(x);
    if (x.a5_u16(0x506).wrapping_sub(0x40) as i16) > (x.u16(Obj::X) as i16) {
        // sorti de l'écran par la gauche
        return x.set8(f::STATE, 6);
    }
    let leader = Obj(x.a5_u16(0x46) as i16 as i32 as u32 & 0xff_ffff);
    let dx = leader.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)).wrapping_add(0x44);
    let dy = leader.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y)).wrapping_sub(9);
    if dx > 0x74 || dy > 0x1c {
        return;
    }
    if !players().any(|p| p.is_active(x.b) && p.u16(x.b, 0x62) != 0) {
        return;
    }
    // un joueur vivant entre : début de la transition
    x.set_a5_u8(0x5f, 0);
    wr8(x.b, 0xff2472, 1);
    x.add8(f::SUB_STATE, 2);
    x.set_a5_u16(0x51e, 4);
    x.set_a5_u8(0x4e, 1);
}

/// Sous-état 2 : palette du rideau, puis deux bandes de 22 tuiles par image.
fn gate_curtain(x: &mut Cx) {
    for k in 0..8 {
        let v = x.data32(0x25c4a0 + 4 * k);
        let v = if k < 6 { v | 0xf000_f000 } else { v.wrapping_add(0xf000_f000) };
        wr32(x.b, 0x91c7e0 + 4 * k, v);
    }
    for _ in 0..2 {
        let step = x.u16(gate::CURTAIN);
        let entry = gate::CURTAIN_STEPS.wrapping_add(step as i16 as i32 as u32);
        let offset = x.data16(entry);
        if (offset as i16) < 0 {
            // fin du rideau : la transition continue en code d'origine (registres de l'original)
            x.c.d[0] = x.c.d[0] & 0xffff_0000 | step as u32;
            x.c.d[1] = x.c.d[1] & 0xffff_0000 | offset as u32;
            x.c.a[4] = gate::CURTAIN_STEPS;
            return x.call(gate::ENTER);
        }
        let mut at = x.data32(entry + 2);
        x.add16(gate::CURTAIN, 6);
        let mut src = gate::CURTAIN_TILES.wrapping_add(offset as i16 as i32 as u32);
        for _ in 0..22 {
            let tile = rd16(x.b, src);
            wr16(x.b, at, tile);
            wr16(x.b, at + 2, 0x1f);
            src += 2;
            at += 4;
        }
    }
}

/// État 4, sous-état 2 : le rideau s'ouvre (deux colonnes effacées par image depuis le centre).
fn gate_open(x: &mut Cx) {
    x.set_a5_u16(0x51e, 4);
    let step = x.u16(gate::CURTAIN);
    if step as i16 > 0xa {
        x.set_a5_u8(0x5f, 1);
        wr8(x.b, 0xff2472, 0);
        x.set_a5_u8(0x4e, 0);
        x.set_a5_u16(0x51e, 0);
        return x.set8(f::STATE, 6);
    }
    let shift = (step.wrapping_mul(4)) as i16 as i32 as u32;
    for base in [0x91203cu32.wrapping_sub(shift), 0x912040u32.wrapping_add(shift)] {
        for row in 0..48 {
            wr32(x.b, base + 0x80 * row, 0x10_0000);
        }
    }
    x.add16(gate::CURTAIN, 1);
}

// ---------------------------------------------------------------------------------------------
// Type 11 : effets attachés aux attaques d'un ennemi (dix variantes)
// ---------------------------------------------------------------------------------------------

mod attack_fx {
    /// Variantes qu'aucun scénario n'exerce : code d'origine (entrée de la variante).
    pub const VARIANTS_ORIGINAL: [(u8, u32); 5] = [(0, 0xabf0e), (2, 0xac656), (5, 0xaca8e), (8, 0xad174), (9, 0xad174)];
    pub const TABLE: u32 = 0xabefa;
    pub const ANIM_A: u32 = 0x18f412;
    pub const ANIM_B: u32 = 0x15cdb6;
    pub const OWNER: u32 = 0x5c;
    pub const SOURCE: u32 = 0x54;
}

fn alloc_effect(x: &mut Cx) -> Option<Obj> {
    x.call(0x2318);
    (x.c.sr & m68k::SR_C == 0).then(|| Obj(x.c.a[1]))
}

/// 0xabec8 : crée un EFFCT 11 de variante `variant` et de mode `mode`, rattaché à l'objet courant.
fn attack_fx_spawn(x: &mut Cx, variant: u8, mode: u8) -> Option<Obj> {
    let o = alloc_effect(x)?;
    wr8(x.b, o.0, 1);
    wr8(x.b, o.0 + 2, 11);
    wr16(x.b, o.0 + attack_fx::OWNER, x.o.0 as u16);
    wr8(x.b, o.0 + 3, variant);
    wr8(x.b, o.0 + 0x2f, mode);
    Some(o)
}

/// Indice de l'attaquant dans les tables : son type, + 0x20 si sa classe (`+0x28`) vaut `class`
/// (`when_equal`) ou ne la vaut pas.
fn attacker_index(x: &mut Cx, a: Obj, class: u8, when_equal: bool) -> u32 {
    let shifted = (a.u8(x.b, 0x28) == class) == when_equal;
    let base: u8 = if shifted { 0x20 } else { 0 };
    base.wrapping_add(a.u8(x.b, 2)) as u32
}

fn attack_fx(x: &mut Cx) {
    let v = x.u8(f::VARIANT);
    if let Some(&(_, original)) = attack_fx::VARIANTS_ORIGINAL.iter().find(|(w, _)| *w == v) {
        x.c.d[0] = 2 * v as u32;
        let word = x.rom16(attack_fx::TABLE + 2 * v as u32);
        x.c.d[1] = x.c.d[1] & 0xffff_0000 | word as u32;
        return x.call(original);
    }
    match v {
        1 => attack_fx_burst(x),
        3 => attack_fx_glint(x),
        4 => attack_fx_rise(x),
        6 => attack_fx_parts(x),
        7 => attack_fx_sparks(x),
        _ => {}
    }
}

/// Variante 1 : éclat lancé depuis l'attaquant, qui le suit puis disparaît.
fn attack_fx_burst(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let owner = x.linked(attack_fx::OWNER);
            for off in [Obj::X, Obj::Z, Obj::Y] {
                let v = owner.u32(x.b, off);
                x.set32(off, v);
            }
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            let (anim, table, k) = match x.u8(0x2f) {
                0 => {
                    x.add32(Obj::Y, 0xffff_ffff);
                    let k = attacker_index(x, owner, 4, true);
                    (x.rom8(0xac38c + k) as u32, 0xac3cc, k)
                }
                1 => {
                    x.add32(Obj::Y, 1);
                    let base: u8 = if x.u8(0x28) == 4 { 0x20 } else { 0 };
                    (4, 0xac526, base.wrapping_add(owner.u8(x.b, 2)) as u32)
                }
                _ => return,
            };
            let (mut vx, vz) = (x.data16(table + 4 * k), x.data16(table + 4 * k + 2));
            if x.u8(0x2c) != 0 {
                vx = vx.wrapping_neg();
            }
            x.set16(Obj::VX, vx);
            x.set16(Obj::VZ, vz);
            x.add16(Obj::X, vx);
            x.add16(Obj::Z, vz);
            x.anim_start(attack_fx::ANIM_A, anim);
            x.reinsert();
        }
        2 => {
            let owner = x.linked(attack_fx::OWNER);
            for off in [Obj::X, Obj::Z] {
                let v = owner.u32(x.b, off);
                x.set32(off, v);
            }
            let (vx, vz) = (x.u16(Obj::VX), x.u16(Obj::VZ));
            x.add16(Obj::X, vx);
            x.add16(Obj::Z, vz);
            // la pause (`A5+0x3d`) le fait vivre jusqu'à la fin de l'animation, sinon il disparaît
            if x.a5_u8(0x3d) == 0 || x.anim_step() {
                x.set32(f::STATE, 0x0600_0000);
            }
        }
        _ => free(x),
    }
}

/// Variante 3 : reflet posé sur l'attaquant tant qu'il est dans l'action 0x68 ou 0x6c.
fn attack_fx_glint(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let owner = x.linked(attack_fx::OWNER);
            for off in [Obj::X, Obj::Z] {
                let v = owner.u16(x.b, off);
                x.set16(off, v);
            }
            let depth = owner.u32(x.b, Obj::Y).wrapping_sub(1);
            x.set32(Obj::Y, depth);
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            let k = 4 * attacker_index(x, owner, 2, false) + 2 * x.u8(0x2f) as u32;
            let mut dx = x.rom8(0xac80a + k) as i8 as u16;
            let dz = x.rom8(0xac80b + k) as i8 as u16;
            x.add16(Obj::Z, dz);
            if x.u8(0x2c) != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(Obj::X, dx);
            x.anim_start(attack_fx::ANIM_A, 0xb);
            x.reinsert();
        }
        2 => {
            x.anim_step();
            let owner = x.linked(attack_fx::OWNER);
            if owner.u16(x.b, f::STATE) == 0x400 && matches!(owner.u8(x.b, 0x49), 0x68 | 0x6c) {
                return x.reinsert();
            }
            x.add8(f::STATE, 2);
        }
        _ => free(x),
    }
}

/// Variante 4 : colonne qui monte depuis le bas de l'écran jusqu'à une hauteur selon l'attaquant.
fn attack_fx_rise(x: &mut Cx) {
    let owner = x.linked(attack_fx::OWNER);
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            for off in [Obj::X, Obj::Y] {
                let v = owner.u32(x.b, off);
                x.set32(off, v);
            }
            let d: u32 = if x.u8(0x2f) & 1 != 0 { 1 } else { 0xffff_ffff };
            x.add32(Obj::Y, d);
            let top = if x.a5_u8(0x36) != 0 { x.a5_u16(0x55a) } else { x.a5_u16(0x50a) };
            let z = top.wrapping_add(0x100).wrapping_sub(x.u16(Obj::Y));
            x.set16(Obj::Z, z);
            let mode = x.u8(0x2f) as u32;
            let anim = x.rom8(0xac9f0 + mode) as u32;
            x.anim_start(attack_fx::ANIM_A, anim);
            x.reinsert();
        }
        2 => {
            if x.u8(f::SUB_STATE) == 0 {
                x.add16(Obj::Z, 4u16.wrapping_neg());
                let k = (x.u8(0x2f) & !1) as u32 + ((((owner.u8(x.b, 2) & 3) >> 1) as u32) << 3);
                let limit = x.rom16(0xaca5e + k);
                if (limit as i16) >= (x.u16(Obj::Z) as i16) {
                    x.add8(f::SUB_STATE, 2);
                    x.set16(Obj::Z, limit);
                    if x.u8(0x2f) == 0 {
                        owner.set_u8(x.b, 6, 6);
                    }
                }
            }
            if owner.u16(x.b, f::STATE) == 0x400 && owner.u8(x.b, 0x49) == 0x9c {
                x.anim_step();
            } else {
                x.add8(f::STATE, 2);
            }
        }
        _ => free(x),
    }
}

/// Variante 6 : pièces d'un objet brisé ; le mode 0 crée les modes 2 et 3 (et devient le mode 1).
fn attack_fx_parts(x: &mut Cx) {
    let mode = x.u8(0x2f);
    if mode == 0 {
        let source = x.linked(attack_fx::SOURCE);
        let k = attacker_index(x, source, 4, true);
        let src = x.u16(attack_fx::SOURCE);
        if x.rom8(0xacafe + k) != 0 {
            if let Some(o) = attack_fx_spawn(x, 6, 2) {
                wr16(x.b, o.0 + attack_fx::SOURCE, src);
            }
        }
        if let Some(o) = attack_fx_spawn(x, 6, 3) {
            wr16(x.b, o.0 + attack_fx::SOURCE, src);
        }
        return x.set8(0x2f, 1);
    }
    if mode > 3 {
        return;
    }
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let source = x.linked(attack_fx::SOURCE);
            let k = attacker_index(x, source, 4, true);
            match mode {
                1 | 2 => {
                    let table = if mode == 1 { 0xacc06 } else { 0xacd8a };
                    let mut dx = x.rom16(table + 4 * k);
                    if dx == 0 && mode == 2 {
                        return x.set32(f::STATE, 0x0600_0000);
                    }
                    if dx != 0 {
                        let dz = x.rom16(table + 4 * k + 2);
                        let (sx, sz) = (source.u16(x.b, Obj::X), source.u16(x.b, Obj::Z));
                        x.set16(Obj::X, sx);
                        x.set16(Obj::Z, sz);
                        if source.u8(x.b, 0x2c) != 0 {
                            dx = dx.wrapping_neg();
                        }
                        x.add16(Obj::X, dx);
                        x.add16(Obj::Z, dz);
                    }
                    let d: u32 = if mode == 1 { 0xffff_ffff } else { 1 };
                    let depth = source.u32(x.b, Obj::Y).wrapping_add(d);
                    x.set32(Obj::Y, depth);
                    x.anim_start(attack_fx::ANIM_B, 4 + mode as u32);
                }
                _ => {
                    let depth = source.u32(x.b, Obj::Y).wrapping_add(2);
                    x.set32(Obj::Y, depth);
                    let mut dx = x.data16(0xacf02 + 4 * k);
                    let dz = x.data16(0xacf02 + 4 * k + 2);
                    let anchor = if dz != 0 { source } else { x.linked(attack_fx::OWNER) };
                    for off in [Obj::X, Obj::Z] {
                        let v = anchor.u32(x.b, off);
                        x.set32(off, v);
                    }
                    let facing = anchor.u8(x.b, 0x2c);
                    x.set8(0x2c, facing);
                    if facing != 0 {
                        dx = dx.wrapping_neg();
                    }
                    x.add16(Obj::X, dx);
                    x.add16(Obj::Z, dz);
                    x.anim_start(attack_fx::ANIM_B, 7);
                }
            }
            x.reinsert();
        }
        2 => {
            if x.anim_step() {
                x.add8(f::STATE, 2);
            }
        }
        _ => free(x),
    }
}

/// Variante 7 : le mode 0 crée une gerbe d'étincelles (modes 1 et 2, liste selon l'attaquant) puis
/// disparaît ; une étincelle suit l'attaquant tant qu'il frappe.
fn attack_fx_sparks(x: &mut Cx) {
    match x.u8(0x2f) {
        0 => {
            if x.u8(f::STATE) == 0 {
                return x.add8(f::STATE, 2);
            }
            let owner = x.linked(attack_fx::OWNER);
            let k = attacker_index(x, owner, 4, true);
            let list = x.rom8(0xad034 + k);
            if list != 0 {
                let entry = 0xad0a8u32.wrapping_add((list as u16).wrapping_sub(2) as i16 as i32 as u32);
                let mut p = 0xad0a8u32.wrapping_add(x.data16(entry) as i16 as i32 as u32);
                let count = x.data16(p);
                p += 2;
                for _ in 0..=count {
                    // mode provisoire : D1.b de l'original (mot de la table des modes), remplacé aussitôt
                    let Some(o) = attack_fx_spawn(x, 7, 6) else { break };
                    let (vx, vz, m) = (x.data16(p), x.data16(p + 2), x.data8(p + 4));
                    wr16(x.b, o.0 + Obj::VX, vx);
                    wr16(x.b, o.0 + Obj::VZ, vz);
                    wr8(x.b, o.0 + 0x2f, m);
                    p += 6;
                    let owner = x.u16(attack_fx::OWNER);
                    wr16(x.b, o.0 + attack_fx::OWNER, owner);
                }
            }
            free(x);
        }
        m @ (1 | 2) => {
            if x.u8(f::STATE) == 0 {
                x.add8(f::STATE, 2);
                return x.anim_start(attack_fx::ANIM_A, 0x15 + m as u32);
            }
            let owner = x.linked(attack_fx::OWNER);
            let action = owner.u8(x.b, 0x49);
            let striking = matches!(action, 0x60 | 0x88)
                || (action == 0xa8 && owner.u8(x.b, 6).wrapping_sub(8) <= 2);
            if !striking || owner.u8(x.b, f::STATE) != 4 || (owner.u16(x.b, Obj::Z) as i16) <= 0 {
                return free(x);
            }
            for off in [Obj::X, Obj::Z] {
                let v = owner.u16(x.b, off);
                x.set16(off, v);
            }
            let depth = owner.u32(x.b, Obj::Y).wrapping_sub(1);
            x.set32(Obj::Y, depth);
            let vz = x.u16(Obj::VZ);
            x.add16(Obj::Z, vz);
            let mut vx = x.u16(Obj::VX);
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            if facing != 0 {
                vx = vx.wrapping_neg();
            }
            x.add16(Obj::X, vx);
            x.anim_step();
            x.reinsert();
        }
        _ => {}
    }
}

handlers! {
    0xabeea "effct_attaque" 300 attack_fx,
    0xad63e "effct_radeau" 300 raft,
    0xad866 "effct_passage" 300 gate,
}
