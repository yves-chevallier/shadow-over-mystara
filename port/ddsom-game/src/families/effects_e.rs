//! Famille EFFCT : objets de scène qui commandent les joueurs (table des types `0x39ce0`).
//!
//! - 11 : effets des attaques d'un ennemi (éclats, marques, reflets, colonnes, traînées, pièces, étincelles,
//!   gerbes) ; les variantes 0, 2, 5, 8 et 9, qu'aucun scénario n'exerce, sont validées par
//!   `examples/fuzz_effct.rs` ;
//! - 13 : radeau qui défile et emporte les joueurs (les directions des joueurs le pilotent) ;
//! - 14 : passage vers une sous-scène, qui oscille, attend un joueur, puis lance la transition
//!   (rideau de tuiles dans le fond 2) : entrée dans la sous-scène (`0xada96` : contexte de la scène
//!   sauvé dans l'objet, objets gelés), puis retour (`0xae11a` : contexte rendu, joueurs replacés).

use super::cx::{handlers, Cx};
use crate::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use crate::objects::Obj;
use crate::objects::pools;

mod f {
    pub const STATE: u32 = 0x04;
    pub const SUB_STATE: u32 = 0x05;
    pub const VARIANT: u32 = 0x03;
}

const ANIM_SET: u32 = 0x1c03a6;
/// Joueurs : quatre enregistrements de 0x200 octets.
const PLAYERS: u32 = A5 + 0x5de;

fn players() -> impl DoubleEndedIterator<Item = Obj> {
    (0..4).map(|k| Obj(PLAYERS + 0x200 * k))
}

fn free(x: &mut Cx) {
    x.free(&pools::EFFCT);
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
    /// Contexte de la scène sauvé dans l'objet : caméras (longs `+0x44`, `+0x48`, mots `+0x4c`,
    /// `+0x4e`), état du défilement (`+0x54`, `+0x5a`) et musique (`+0x16`).
    pub const CAMERA_X: u32 = 0x44;
    pub const CAMERA_Y: u32 = 0x48;
    pub const ALT_X: u32 = 0x4c;
    pub const ALT_Y: u32 = 0x4e;
    pub const SCROLL: u32 = 0x54;
    pub const SCROLL_STEP: u32 = 0x5a;
    pub const MUSIC: u32 = 0x16;
    /// État du défilement de la scène (`A5-0x55d0` long, `A5-0x55cc` octet) et musique (`A5+0x7fc4`).
    pub const SCENE_SCROLL: u32 = 0xff2a30;
    pub const SCENE_SCROLL_STEP: u32 = 0xff2a34;
    pub const SCENE_MUSIC: u32 = 0xffffc4;
    /// Pools dont les objets sont gelés pendant la sous-scène (`+0x40` incrémenté, `+0x41` effacé) :
    /// (décalage de `A5`, emplacements, taille).
    pub const FROZEN: [(u32, u32, u32); 6] =
        [(0x4a9e, 32, 0x50), (0x549e, 38, 0x60), (0x795e, 16, 0x50), (0x3a1e, 3, 0xc0), (0x3c5e, 38, 0x60), (0x179e, 48, 0x60)];
    /// Décalage en x des joueurs, par personnage, au retour (mots, espace programme).
    pub const RETURN_OFFSETS: u32 = 0xae3b4;
    pub const MUSIC_OF_SCENE: u32 = 0xe0d8c;
    pub const RESTART_SCENE: u32 = 0xcc0ca;
    pub const PLAY_MUSIC: u32 = 0xe17a4;
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
                return gate_leave(x);
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
            // fin du rideau : entrée dans la sous-scène
            return gate_enter(x);
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

/// 0xada96 : entrée dans la sous-scène (scène 7 → 6, sinon 12). Le contexte de la scène est sauvé dans
/// l'objet, les objets des pools sont gelés, le décor de la variante est créé, puis la scène repart.
fn gate_enter(x: &mut Cx) {
    let scene = if x.a5_u8(0x34) == 7 { 6 } else { 0xc };
    x.set_a5_u8(0x34, scene);
    x.set16(f::STATE, 0x400);
    x.set16(gate::CURTAIN, 0);
    x.set_a5_u8(0x44, 0);
    x.call(0x1f0be);
    // 0xae298 : sauvegarde du contexte
    let (cam_x, cam_y) = (x.a5_u32(0x506), x.a5_u32(0x50a));
    x.set32(gate::CAMERA_X, cam_x);
    x.set32(gate::CAMERA_Y, cam_y);
    let (alt_x, alt_y) = (x.a5_u16(0x556), x.a5_u16(0x55a));
    x.set16(gate::ALT_X, alt_x);
    x.set16(gate::ALT_Y, alt_y);
    let scroll = crate::mem::rd32(x.b, gate::SCENE_SCROLL);
    x.set32(gate::SCROLL, scroll);
    let step = rd8(x.b, gate::SCENE_SCROLL_STEP);
    x.set8(gate::SCROLL_STEP, step);
    let music = rd16(x.b, gate::SCENE_MUSIC);
    x.set16(gate::MUSIC, music);
    for (base, slots, size) in gate::FROZEN {
        for k in 0..slots {
            let o = A5 + base + size * k;
            if rd8(x.b, o) != 0 {
                let frozen = rd8(x.b, o + 0x40).wrapping_add(1);
                wr8(x.b, o + 0x40, frozen);
                wr8(x.b, o + 0x41, 0);
            }
        }
    }
    // 0xae37c : décor de la variante (D0 = 0, D1 = variante)
    x.c.d[0] = 0;
    let v = x.u8(f::VARIANT);
    x.c.d[1] = x.c.d[1] & !0xff | v as u32;
    scene_prop(x);
    x.call(gate::MUSIC_OF_SCENE);
    x.call(gate::RESTART_SCENE);
}

/// 0xcc200 : décor (SET 8) de variante D0 et de réglage D1 ; à défaut un SOBJ 4, à défaut un EFFCT 18.
fn scene_prop(x: &mut Cx) {
    for (alloc, kind) in [(&pools::SET, 8), (&pools::SOBJ, 4), (&pools::EFFCT, 0x12)] {
        if let Some(o) = x.alloc(alloc) {
            o.set_u8(x.b, 0, 1);
            o.set_u8(x.b, 2, kind);
            o.set_u8(x.b, f::VARIANT, x.c.d[0] as u8);
            o.set_u8(x.b, 0x2f, x.c.d[1] as u8);
            return;
        }
    }
}

/// 0xae11a, sous-état 0 : retour de la sous-scène (scène 6 → 7, sinon 9). La scène repart avec le
/// contexte sauvé par `gate_enter`, les joueurs vivants sont replacés devant l'objet suivi.
fn gate_leave(x: &mut Cx) {
    let scene = if x.a5_u8(0x34) == 6 { 7 } else { 9 };
    x.set_a5_u8(0x34, scene);
    x.set_a5_u8(0x44, 1);
    let (a0, d7) = (x.c.a[0], x.c.d[7]);
    crate::call(x.c, x.b, 0x33aa6);
    (x.c.a[0], x.c.d[7]) = (a0, d7);
    crate::call(x.c, x.b, 0xf2e56);
    let (a0, d7) = (x.c.a[0], x.c.d[7]);
    for routine in [0x1a360, 0x1a678, 0x1a8ac] {
        crate::call(x.c, x.b, routine);
    }
    (x.c.a[0], x.c.d[7]) = (a0, d7);
    x.set_a5_u8(0x48e, 0xf);
    crate::call(x.c, x.b, 0x1686);
    for p in players() {
        if p.u8(x.b, 0) == 0 || p.u16(x.b, 0x62) == 0 {
            continue;
        }
        p.set_u8(x.b, 0, 1);
        p.set_u32(x.b, f::STATE, 0x0200_0000);
        p.set_u8(x.b, 0x66, 0);
        p.set_u8(x.b, 0x15c, 0);
        p.set_u8(x.b, 0x7e, 0x50);
        // 0xae388 : devant l'objet suivi, décalé selon le personnage
        let leader = Obj(x.a5_u16(0x46) as i16 as i32 as u32 & 0xff_ffff);
        let (lx, ly) = (leader.u16(x.b, Obj::X), leader.u16(x.b, Obj::Y));
        p.set_u16(x.b, Obj::X, lx);
        p.set_u16(x.b, Obj::Y, ly);
        p.set_u16(x.b, Obj::Z, 0);
        let k = 2 * p.u8(x.b, 2) as u32;
        let dx = x.rom16(gate::RETURN_OFFSETS + k);
        p.set_u16(x.b, Obj::X, lx.wrapping_add(dx));
        p.set_u16(x.b, Obj::Y, ly.wrapping_sub(0x12));
    }
    x.set_a5_u16(0x51e, 4);
    let (cam_x, cam_y) = (x.u32(gate::CAMERA_X), x.u32(gate::CAMERA_Y));
    x.set_a5_u32(0x506, cam_x);
    x.set_a5_u32(0x50a, cam_y);
    let (alt_x, alt_y) = (x.u16(gate::ALT_X), x.u16(gate::ALT_Y));
    x.set_a5_u16(0x556, alt_x);
    x.set_a5_u16(0x55a, alt_y);
    // cibles des caméras : alignées sur 32 pixels en x, 0x160 plus haut
    x.set_a5_u32(0x50e, cam_x & !0x1f_0000);
    x.set_a5_u32(0x512, cam_y.wrapping_sub(0x160_0000));
    x.set_a5_u16(0x55e, alt_x & 0xffe0);
    x.set_a5_u16(0x562, alt_y.wrapping_sub(0x160));
    let scroll = x.u32(gate::SCROLL);
    crate::mem::wr32(x.b, gate::SCENE_SCROLL, scroll);
    let step = x.u8(gate::SCROLL_STEP);
    wr8(x.b, gate::SCENE_SCROLL_STEP, step);
    let music = x.u16(gate::MUSIC);
    x.c.d[4] = x.c.d[4] & 0xffff_0000 | music as u32;
    x.add8(f::SUB_STATE, 2);
    x.call(gate::PLAY_MUSIC);
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
    /// Variante 0 : places autour de l'attaquant (mot de décalage par type d'attaquant, puis huit (dx, dz)).
    pub const FLASH_PLACES: u32 = 0xabfbe;
    pub const SOUND_FLASH: u32 = 0xe9ef4;
    /// Variante 2 : décalages (dx, dz) et animation par type d'attaquant (espace programme).
    pub const MARK_PLACES: u32 = 0xac6ea;
    pub const MARK_ANIMS: u32 = 0xac6ca;
    /// Action de l'attaquant (`+0x49`), notée par la variante 5 en `+0x42`.
    pub const ACTION: u32 = 0x49;
    pub const NOTED_ACTION: u32 = 0x42;
    /// Variantes 8 et 9 : fiche de la gerbe par type d'attaquant (octet, espace programme : 0 = aucune),
    /// fiches (données : décalages, puis nombre moins un et (vx, vz, réglage, bourrage) par éclat),
    /// animation des éclats par réglage.
    pub const SHOWER_SHEETS: u32 = 0xad1b4;
    pub const SHOWER_DATA: u32 = 0xad232;
    pub const SHOWER_ANIMS: u32 = 0xad354;
    /// Créateur d'un effet 11 (`0xabec8`, natif).
    pub const SPAWN: u32 = 0xabec8;
    pub const ANIM_A: u32 = 0x18f412;
    pub const ANIM_B: u32 = 0x15cdb6;
    pub const OWNER: u32 = 0x5c;
    pub const SOURCE: u32 = 0x54;
}

/// 0xabec8 : crée un EFFCT 11 de variante `variant` et de mode `mode`, rattaché à l'objet courant.
fn attack_fx_spawn(x: &mut Cx, variant: u8, mode: u8) -> Option<Obj> {
    let o = x.alloc(&pools::EFFCT)?;
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
    match x.u8(f::VARIANT) {
        0 => attack_fx_flash(x),
        1 => attack_fx_burst(x),
        2 => attack_fx_mark(x),
        3 => attack_fx_glint(x),
        4 => attack_fx_rise(x),
        5 => attack_fx_trail(x),
        6 => attack_fx_parts(x),
        7 => attack_fx_sparks(x),
        8 | 9 => attack_fx_shower(x),
        _ => {}
    }
}

/// Fin d'animation : retrait (état 6), sinon remise en liste.
fn attack_fx_anim_or_leave(x: &mut Cx) {
    if x.anim_step() {
        return x.set32(f::STATE, 0x0600_0000);
    }
    x.reinsert();
}

/// Variante 0 : éclat posé au hasard autour de l'attaquant (huit places par type d'attaquant, retourné une
/// image sur deux), animation tirée au hasard, son ; disparaît à la fin de l'animation.
fn attack_fx_flash(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let owner = x.linked(attack_fx::OWNER);
            for off in [Obj::X, Obj::Z, Obj::Y] {
                let v = owner.u32(x.b, off);
                x.set32(off, v);
            }
            x.add32(Obj::Y, 0xffff_ffff);
            let k = attacker_index(x, owner, 2, false);
            let offset = x.data16(attack_fx::FLASH_PLACES + 2 * k) as i16 as i32 as u32;
            let places = attack_fx::FLASH_PLACES.wrapping_add(offset);
            x.call(super::rom::RNG);
            let pick = 4 * (x.c.d[0] & 7);
            let (mut dx, dz) = (x.data16(places + pick), x.data16(places + pick + 2));
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            if facing != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(Obj::X, dx);
            x.add16(Obj::Z, dz);
            if x.a5_u8(0x6f) & 1 != 0 {
                x.set8(0x2c, facing ^ 0x20);
            }
            x.call(super::rom::RNG);
            let anim = x.c.d[0] & 3;
            x.anim_start(attack_fx::ANIM_A, anim);
            x.call(attack_fx::SOUND_FLASH);
        }
        2 => attack_fx_anim_or_leave(x),
        _ => free(x),
    }
}

/// Variante 2 : marque posée sur l'attaquant (décalage et animation par type) ; à la fin de son animation,
/// elle fait avancer l'attaquant (`+6` += 2) et disparaît.
fn attack_fx_mark(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let owner = x.linked(attack_fx::OWNER);
            for off in [Obj::X, Obj::Z, Obj::Y] {
                let v = owner.u32(x.b, off);
                x.set32(off, v);
            }
            x.add32(Obj::Y, 0xffff_ffff);
            let kind = owner.u8(x.b, Obj::KIND) as u32;
            let mut dx = x.rom16(attack_fx::MARK_PLACES + 4 * kind);
            let dz = x.rom16(attack_fx::MARK_PLACES + 4 * kind + 2);
            x.add16(Obj::Z, dz);
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            if facing != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(Obj::X, dx);
            let anim = x.rom8(attack_fx::MARK_ANIMS + kind);
            x.anim_start(attack_fx::ANIM_A, anim as u32);
            x.reinsert();
        }
        2 => {
            if !x.anim_step() {
                return;
            }
            let owner = x.linked(attack_fx::OWNER);
            let step = owner.u8(x.b, 6).wrapping_add(2);
            owner.set_u8(x.b, 6, step);
            x.set32(f::STATE, 0x0600_0000);
        }
        _ => free(x),
    }
}

/// Variante 5 : comme l'éclat (1), mais il ne suit l'attaquant que tant que celui-ci garde l'action notée à la
/// naissance (`+0x49` → `+0x42`) et n'a pas passé l'étape 4.
fn attack_fx_trail(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            let owner = x.linked(attack_fx::OWNER);
            let action = owner.u8(x.b, attack_fx::ACTION);
            x.set8(attack_fx::NOTED_ACTION, action);
            burst_init(x);
        }
        2 => {
            let owner = x.linked(attack_fx::OWNER);
            if owner.u8(x.b, attack_fx::ACTION) != x.u8(attack_fx::NOTED_ACTION) || owner.u8(x.b, 6) as i8 >= 4 {
                return free(x);
            }
            for off in [Obj::X, Obj::Z] {
                let v = owner.u32(x.b, off);
                x.set32(off, v);
            }
            let (vx, vz) = (x.u16(Obj::VX), x.u16(Obj::VZ));
            x.add16(Obj::X, vx);
            x.add16(Obj::Z, vz);
            x.anim_step();
            x.reinsert();
        }
        _ => free(x),
    }
}

/// Variantes 8 et 9 : gerbe d'éclats de l'attaquant. Le premier (réglage 0) crée les éclats d'une fiche
/// propre au type de l'attaquant (vitesses, réglage) puis disparaît ; chaque éclat suit l'attaquant tant
/// qu'il est dans l'action prévue, puis se détache, retombe en clignotant et disparaît au sol.
fn attack_fx_shower(x: &mut Cx) {
    if x.u8(0x2f) == 0 {
        return attack_fx_shower_spawn(x);
    }
    match x.u8(f::STATE) {
        0 => {
            let mode = x.u8(0x2f).wrapping_sub(1) as u32;
            let anim = x.rom8(attack_fx::SHOWER_ANIMS + mode);
            if x.u8(f::VARIANT) != 8 {
                x.set16(0x2a, 0x110);
            }
            x.add8(f::STATE, 2);
            x.anim_start(attack_fx::ANIM_A, anim as u32);
            attack_fx_shard(x);
        }
        2 => attack_fx_shard(x),
        _ => free(x),
    }
}

fn attack_fx_shower_spawn(x: &mut Cx) {
    let owner = x.linked(attack_fx::OWNER);
    let k = attacker_index(x, owner, 4, true);
    let sheet = x.rom8(attack_fx::SHOWER_SHEETS + k) as u16;
    if sheet != 0 {
        let offset = x.data16(attack_fx::SHOWER_DATA + sheet.wrapping_sub(2) as i16 as i32 as u32) as i16 as i32 as u32;
        let mut at = attack_fx::SHOWER_DATA.wrapping_add(offset);
        let mut n = x.data16(at);
        at += 2;
        loop {
            let variant = x.u8(f::VARIANT);
            x.c.d[0] = variant as u32;
            x.c.d[1] = 0;
            x.c.a[4] = at;
            x.call(attack_fx::SPAWN);
            if x.c.sr & m68k::SR_C != 0 {
                break;
            }
            let o = Obj(x.c.a[1]);
            let (vx, vz) = (x.data16(at), x.data16(at + 2));
            o.set_u16(x.b, Obj::VX, vx);
            o.set_u16(x.b, Obj::VZ, vz);
            let mode = x.data8(at + 4);
            o.set_u8(x.b, 0x2f, mode);
            at += 6;
            let link = x.u16(attack_fx::OWNER);
            o.set_u16(x.b, attack_fx::OWNER, link);
            n = n.wrapping_sub(1);
            if n == 0xffff {
                break;
            }
        }
        x.c.a[4] = at;
        x.c.d[5] = x.c.d[5] & 0xffff_0000 | n as u32;
    }
    free(x);
}

/// Un éclat de la gerbe (`0xad384`).
fn attack_fx_shard(x: &mut Cx) {
    let owner = x.linked(attack_fx::OWNER);
    if x.u8(f::SUB_STATE) == 0 {
        let first = x.u8(f::VARIANT) == 8;
        let (a, b) = if first { (0x70, 0x90) } else { (0x94, 0x94) };
        let action = owner.u8(x.b, attack_fx::ACTION);
        let follows = if action == a || action == b || action != 0xa8 {
            owner.u8(x.b, f::STATE) == 4 && (owner.u8(x.b, 6) as i8) < 6
        } else {
            // action 0xa8 : l'éclat suit l'attaquant pendant ses étapes 4 à 6, sinon disparaît
            if owner.u8(x.b, 6).wrapping_sub(4) > 2 {
                return free(x);
            }
            true
        };
        if follows {
            let (px, pz, py) = (owner.u16(x.b, Obj::X), owner.u16(x.b, Obj::Z), owner.u32(x.b, Obj::Y));
            x.set16(Obj::X, px);
            x.set16(Obj::Z, pz);
            x.set32(Obj::Y, py.wrapping_sub(1));
            let vz = x.u16(Obj::VZ);
            x.add16(Obj::Z, vz);
            let mut vx = x.u16(Obj::VX);
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            if facing != 0 {
                vx = vx.wrapping_neg();
            }
            x.add16(Obj::X, vx);
            return x.reinsert();
        }
        // détaché : il retombe, poussé vers l'extérieur
        x.add8(f::SUB_STATE, 2);
        x.set16(Obj::VZ, 0x200);
        x.set16(Obj::AZ, 0xffc0);
        x.set16(Obj::AX, 0);
        let mut vx: u16 = 0x180;
        if (x.u16(Obj::VX) as i16) < 0 {
            vx = vx.wrapping_neg();
        }
        if x.u8(0x2c) != 0 {
            vx = vx.wrapping_neg();
        }
        x.set16(Obj::VX, vx);
        if x.c.d[7] & 1 != 0 {
            let v = x.u8(0x01) ^ 2;
            x.set8(0x01, v);
        }
    }
    let v = x.u8(0x01) ^ 2;
    x.set8(0x01, v);
    x.call(super::cx::sub::MOVE_XZ);
    if x.u16(Obj::Z) as i16 <= 0 {
        free(x);
    }
}

/// Variante 1 : éclat lancé depuis l'attaquant, qui le suit puis disparaît.
fn attack_fx_burst(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => burst_init(x),
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

/// Naissance de l'éclat (variantes 1 et 5) : posé sur l'attaquant, vitesse et animation selon son type.
fn burst_init(x: &mut Cx) {
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
