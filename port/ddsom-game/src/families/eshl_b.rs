//! Tirs et attaques des ennemis, types ESHL 15 à 39 (suite de `eshl.rs`). Voir `docs/familles/eshl_b.md`.
//!
//! Mêmes conventions que `eshl.rs` : chaque type est un `mod` avec une fonction `run` ; `+4` est l'état
//! principal, `+5` et `+6` des sous-états (octets pairs, indices de tables de sauts de l'original) ;
//! `+0x50` désigne le propriétaire (mot sur 16 bits, ou long pour quelques types).

use m68k::SR_C;

use super::cx::{handlers, sub, Cx};
use super::shell::*;
use crate::mem::{rd32, wr32};
use crate::objects::Obj;

/// Sons (wrappers de la zone 0xe0836-0xee080) et autres routines d'effets.
mod sfx {
    pub const E4752: u32 = 0xe4752;
    pub const E4E5A: u32 = 0xe4e5a;
    pub const E4FB6: u32 = 0xe4fb6;
    pub const E5020: u32 = 0xe5020;
    pub const E7376: u32 = 0xe7376;
    pub const ED686: u32 = 0xed686;
}

/// Générateur pseudo-aléatoire du jeu (`rng.rs`) : D0.w = tirage.
const RNG: u32 = 0x19a88;
/// Angle (0..31) entre deux points : (D1, D2) = (x, y) de la cible, (D3, D4) = (x, z) de l'objet.
const ANGLE: u32 = 0x1386;
/// Division non signée D0 / D1 (quotient dans D0).
const DIVIDE: u32 = 0x17ea;
/// Réservation d'un objet ENEMY et EFFCT (ESHL : `sub::ALLOC_ESHL`).
const ALLOC_ENEMY: u32 = 0x22bc;
const ALLOC_EFFCT: u32 = 0x2318;

/// État long « à libérer » (`+4 = 6`, `+5..+7` = 0).
const DEAD: u32 = 0x0600_0000;

// ---------------------------------------------------------------------------------------------
// Aides communes
// ---------------------------------------------------------------------------------------------

/// Propriétaire (mot de 16 bits étendu en signe) d'un autre objet.
fn link_of(x: &mut Cx, o: Obj, off: u32) -> Obj {
    Obj(o.u16(x.b, off) as i16 as i32 as u32 & 0xff_ffff)
}

/// Objet désigné par un champ long (`movea.l (off,A0),A1`).
fn ptr(x: &mut Cx, off: u32) -> Obj {
    Obj(x.u32(off) & 0xff_ffff)
}

/// Réserve un objet dans le pool dont `alloc` est la routine d'allocation ; `None` si le pool est plein.
fn alloc(x: &mut Cx, alloc: u32) -> Option<Obj> {
    x.call(alloc);
    (x.c.sr & SR_C == 0).then(|| Obj(x.c.a[1]))
}

/// L'objet est marqué à libérer à la prochaine image.
fn kill(x: &mut Cx) {
    x.set32(0x4, DEAD);
}

/// Valeur du mot de poids faible d'un registre d'adresse, à ranger dans un champ (`move.w A0w,(off,A1)`).
fn low16(o: Obj) -> u16 {
    o.0 as u16
}

fn reinsert_foreground(x: &mut Cx) {
    x.call(sub::REINSERT_FOREGROUND);
}

/// Copie la position (X, Z, Y en 16.16) d'un objet dans le tir.
fn copy_position32(x: &mut Cx, from: Obj) {
    for off in [0x8, 0xc, 0x10] {
        let v = from.u32(x.b, off);
        x.set32(off, v);
    }
}

/// Appelle `1b64` (table de vitesses `n`, indexée par `+0x2d`).
fn speed_table(x: &mut Cx, n: u32) {
    x.c.d[0] = n;
    x.call(sub::SPEED_TABLE_B);
}

// ---------------------------------------------------------------------------------------------
// Type 15 (0x9fe0a) : arme d'un ennemi composite, sous-objet d'un sous-objet (h)
// ---------------------------------------------------------------------------------------------

/// Pièce lancée par une partie d'ennemi composite (type ESHL 14 puis son tir) : le propriétaire
/// (`+0x50`) est un tir de type 14 dont le propriétaire est lui-même l'ennemi. Variante 0 : projectile
/// qui part en cloche vers le joueur (vitesses choisies selon la distance entre l'ennemi et sa
/// cible) ; variante 1 : arme collée au propriétaire, qui suit l'animation d'attaque.
mod shot15 {
    use super::*;

    const ANIMS: u32 = 0x17a276;
    /// Vitesses (VX VZ AZ AX), une ligne de 8 octets par portée (8, 0x10, 0x18).
    const SPEEDS: u32 = 0xa034e;
    /// Sous-états de l'état 2 (`+5`).
    const FLY: u8 = 0;
    const HOLD: u8 = 2;
    const SWING: u8 = 4;
    /// Phases du vol (`+6`).
    const PHASE_LAST: u8 = 8;
    const PHASE_BLINK: u8 = 10;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => spawn(x),
            2 => active(x),
            _ => x.free_eshl(),
        }
    }

    /// Création : placée devant le propriétaire, selon la variante.
    fn spawn(x: &mut Cx) {
        x.set8(f::BOX_SET, 7);
        if x.u16(f::OWNER) == 0 {
            return kill(x);
        }
        let owner = x.linked(f::OWNER);
        let variant = x.u8(0x3);
        let (dx, dz, dy) = if variant == 0 { (0x67, 0x4a, 0x10) } else { (0x42, 0x2e, 0) };
        place_in_front(x, owner, dx, dz);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.add16(0x10, dy);
        x.set8(0x43, 4);
        match variant {
            0 => {
                x.set32(0x4, 0x0200_0000);
                x.set8(f::ATTACK_POWER, 8);
                x.set8(f::ATTACK_ID, 0x10);
                aim(x);
                x.anim_start(ANIMS, 5);
            }
            1 => {
                x.set8(0x5d, 0);
                x.set8(f::ATTACK_POWER, 0xc);
                x.set8(f::ATTACK_ID, 0x11);
                x.call(sfx::E4752);
                let owner_state = owner.u32(x.b, 0x4);
                if owner_state == 0x0202_0004 {
                    x.set32(0x4, 0x0202_0000);
                    x.anim_start(ANIMS, 0x13);
                } else if owner_state == 0x0202_0204 {
                    x.set32(0x4, 0x0204_0000);
                    x.anim_start(ANIMS, 0x10);
                }
            }
            _ => {}
        }
    }

    /// Vitesses initiales de la variante 0 : portée entre l'ennemi (propriétaire du propriétaire) et
    /// sa cible, ligne 8, 0x10 ou 0x18 de `SPEEDS`.
    fn aim(x: &mut Cx) {
        let part = x.linked(f::OWNER);
        let enemy = link_of(x, part, f::OWNER);
        let target = link_of(x, enemy, f::OWNER);
        let row = if enemy.u16(x.b, f::OWNER) == 0 {
            0x18
        } else {
            let flipped = enemy.u8(x.b, f::FLIP) != 0;
            let (tx, ex) = (target.u16(x.b, 0x8), enemy.u16(x.b, 0x8));
            let range = if flipped { tx.wrapping_sub(ex) } else { ex.wrapping_sub(tx) } as i16;
            if range <= 0xa8 {
                8
            } else if range <= 0xfc {
                0x10
            } else {
                0x18
            }
        };
        let mut vx = x.rom16(SPEEDS + row);
        if enemy.u8(x.b, f::FLIP) != 0 {
            vx = vx.wrapping_neg();
        }
        x.set16(0x14, vx);
        let vz = x.rom16(SPEEDS + row + 2);
        x.set16(0x18, vz);
        let az = x.rom16(SPEEDS + row + 4);
        x.set16(0x1a, az);
        let ax = x.rom16(SPEEDS + row + 6);
        x.set16(0x16, ax);
    }

    /// Recopie la position du propriétaire (décalée devant lui).
    fn follow(x: &mut Cx) {
        if x.u16(f::OWNER) == 0 {
            return;
        }
        let owner = x.linked(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        place_in_front(x, owner, 0x42, 0x2e);
    }

    fn active(x: &mut Cx) {
        if x.u16(f::OWNER) == 0 {
            return kill(x);
        }
        if x.u8(0x3) != 0 {
            let part = x.linked(f::OWNER);
            let enemy = link_of(x, part, f::OWNER);
            if enemy.u16(x.b, 0x4) != 0x402 && (enemy.u8(x.b, 0x53) as i8) > 0 {
                return x.reinsert();
            }
        }
        match x.u8(0x5) {
            FLY => fly(x),
            HOLD => hold(x),
            SWING => swing(x),
            _ => {}
        }
        x.reinsert();
    }

    /// Le propriétaire a fini son attaque (ou est dans l'état 0x402) : l'arme disparaît.
    fn owner_done(x: &mut Cx) -> bool {
        let part = x.linked(f::OWNER);
        let enemy = link_of(x, part, f::OWNER);
        if enemy.u16(x.b, 0x4) == 0x402 || enemy.u8(x.b, 0xa8) & 1 != 0 {
            x.set8(0x5d, 1);
            x.set8(0x4, st::FREE);
            return true;
        }
        false
    }

    /// Vol de la variante 0 : le sens se retourne toutes les 5 images, six phases de rebonds
    /// (vitesses décroissantes), puis clignotement.
    fn fly(x: &mut Cx) {
        let phase = x.u8(0x6);
        if phase > PHASE_BLINK {
            return;
        }
        x.add8(0x43, 0xff);
        if (x.u8(0x43) as i8) < 0 {
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            x.set8(0x43, 4);
        }
        if phase == PHASE_BLINK {
            x.add8(f::TIMER, 0xff);
            if (x.u8(f::TIMER) as i8) < 0 {
                return x.set8(0x4, st::FREE);
            }
            let visible = x.u8(f::TIMER) & 3 == 0;
            x.set_bit(0x1, 1, !visible);
            return;
        }
        x.call(sub::MOVE_XZ);
        if !x.c.cond(14) {
            x.add8(0x6, 2);
            if phase == PHASE_LAST {
                x.set8(f::TIMER, 0x32);
            } else {
                // rebond : (VX, VZ) par phase
                let (vx, vz) = [(-0x180i16, 0x3c0), (-0x100, 0x300), (-0xc0, 0x280), (-0x80, 0x200)][phase as usize / 2];
                x.set32(0xc, 0);
                x.set16(0x14, vx as u16);
                x.set16(0x18, vz);
                x.set16(0x1a, -0x38i16 as u16);
                let part = x.linked(f::OWNER);
                let enemy = link_of(x, part, f::OWNER);
                if enemy.u8(x.b, f::FLIP) != 0 {
                    let v = x.u16(0x14).wrapping_neg();
                    x.set16(0x14, v);
                }
            }
        }
        x.call(sub::CULL_OFFSCREEN);
    }

    /// Variante 1 (arme tenue) : suit le propriétaire pendant que l'ennemi attaque.
    fn hold(x: &mut Cx) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return follow(x);
        }
        if owner_done(x) {
            return;
        }
        if x.u8(0x6) == 0 {
            follow(x);
            if x.anim_step() {
                x.set8(0x5d, 1);
                x.set8(0x4, st::FREE);
            }
        }
    }

    /// Variante 1 (coup) : animation en trois temps (coup, attente de 8 images, retour).
    fn swing(x: &mut Cx) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return follow(x);
        }
        if owner_done(x) {
            return;
        }
        match x.u8(0x6) {
            0 => {
                follow(x);
                if x.anim_step() {
                    x.add8(0x6, 2);
                    x.set8(0x42, 8);
                    x.anim_start(ANIMS, 0x11);
                }
            }
            2 => {
                follow(x);
                if x.anim_step() {
                    x.add8(0x42, 0xff);
                    if (x.u8(0x42) as i8) >= 0 {
                        x.anim_start(ANIMS, 0x11);
                    } else {
                        x.add8(0x6, 2);
                        x.anim_start(ANIMS, 0x12);
                    }
                }
            }
            4 => {
                follow(x);
                if x.anim_step() {
                    x.set8(0x5d, 1);
                    x.set8(0x4, st::FREE);
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 16 (0xa03e0) : objet ramassé / porté (h), placé dans la liste du premier plan
// ---------------------------------------------------------------------------------------------

/// Objet attaché à son propriétaire (un ennemi : PV en `+0x62`). Tant qu'il est « porté »
/// (`+0x5a` nul) il suit le propriétaire ; sinon il est lancé en cloche (table de vitesses 0x18),
/// tourne (`+0x2d` varie de 1 toutes les 25 images) et passe à l'état 4 en retombant sous Z = 0x100.
mod shot16 {
    use super::*;

    const ANIMS: u32 = 0x16e146;
    const HELD: u8 = 0;
    const THROWN: u8 = 2;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x58, 4);
                let anim = (x.u8(0x43) != 0) as u32;
                x.anim_start(ANIMS, anim);
                reinsert_foreground(x);
            }
            2 => active(x),
            4 => dropped(x),
            6 => x.free_eshl(),
            _ => {}
        }
    }

    fn active(x: &mut Cx) {
        match x.u8(0x5) {
            HELD => {
                let owner = x.linked(f::OWNER);
                if (owner.u8(x.b, 0x4) as i8) >= 4 {
                    x.set8(0x0, 2);
                }
                if x.u8(0x5a) == 0 {
                    x.anim_step();
                    copy_position32(x, owner);
                    let dx = if x.u8(f::FLIP) != 0 { -0x2ai16 } else { 0x2a };
                    x.add16(0x8, dx as u16);
                    x.add16(0xc, -0x78i16 as u16);
                    x.add32(0x10, 1);
                    x.add32(0xc, 0xffff_ffff);
                    reinsert_foreground(x);
                } else {
                    launch(x);
                }
            }
            _ => flying(x),
        }
    }

    /// Passe au lancer : table de vitesses 0x18, la vitesse verticale devient celle de Y.
    fn launch(x: &mut Cx) {
        x.add8(0x5, 2);
        speed_table(x, 0x18);
        let v = x.u32(0x1c);
        x.set32(0x18, v);
        let angle = if x.u8(f::FLIP) != 0 { 0xa } else { 0x16 };
        x.set8(0x2d, angle);
        x.set8(0x55, 2);
        reinsert_foreground(x);
    }

    fn flying(x: &mut Cx) {
        x.call(sub::MOVE_TABLE_XZ);
        x.anim_step();
        if x.u8(0x2d) != 0 {
            x.add8(0x55, 0xff);
            if (x.u8(0x55) as i8) < 0 {
                x.set8(0x55, 0x18);
                let step: u8 = if x.u8(f::FLIP) != 0 { 0xff } else { 1 };
                let v = x.u8(0x2d).wrapping_add(step) & 0x1f;
                x.set8(0x2d, v);
            }
        }
        if (x.u16(0xc) as i16) > 0x100 {
            x.add8(0x4, 4);
        }
        reinsert_foreground(x);
    }

    /// État 4 : tant que le propriétaire a des PV, rien ; sinon l'objet est lancé (ou relâché).
    fn dropped(x: &mut Cx) {
        let owner = x.linked(f::OWNER);
        if (owner.u16(x.b, 0x62) as i16) <= 0 {
            x.set8(0x0, 2);
            x.set32(0x4, 0x0200_0000);
            return launch(x);
        }
        x.set32(0x4, 0x0200_0000);
        if x.u8(0x5a) != 0 {
            x.set8(0x0, 2);
            x.set32(0x58, 0);
            return;
        }
        x.set32(0x58, 0);
        owner.set_u32(x.b, 0x4, 0x0402_0000);
        if (x.u16(0x62) as i16) > 0 {
            x.add8(0x5, 2);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 17 (0xa05ce) : projectile à trajectoire visée, trois modes selon +0x43 (h)
// ---------------------------------------------------------------------------------------------

/// Projectile qui apparaît en chargeant (animation 0, 60 images) puis part vers sa cible (`+0x50`,
/// pointeur long) avec une vitesse verticale/angle calculés ; `+0x43` choisit le mode : 0 viser
/// l'angle par table (`1386`), < 0 tirer à plat vers la cible, > 0 viser en profondeur. Le parent
/// (`+0x56`, pointeur long) l'entretient : s'il passe à l'état >= 4 le projectile meurt. Au
/// démarrage il éclaire la palette 0x91c300 (copie de 0x25a660 teintée par 0x91c000).
mod shot17 {
    use super::*;

    const ANIMS: u32 = 0x16e1f4;
    const PALETTE_SRC: u32 = 0x25a660;
    const PALETTE_DST: u32 = 0x91c300;
    const PALETTE_TINT: u32 = 0x91c000;
    /// Décalage d'apparition (Z, X) par `+0x42` (4 octets par ligne).
    const SPAWN_OFFSETS: u32 = 0xa0670;
    /// `+0x2d` selon l'angle (Y) pour le mode 0 et le mode > 0.
    const AIM_TABLE_B: u32 = 0xa07ac;
    const AIM_TABLE_A: u32 = 0xa0892;
    /// Vitesse verticale et horizontale du mode < 0 (4 octets par ligne).
    const DROP_SPEEDS: u32 = 0xa0962;
    const CHARGE: u8 = 0;
    const AIM: u8 = 2;
    const FALL: u8 = 4;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            4 => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
            }
            _ => x.free_eshl(),
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        for i in 0..8 {
            let v = x.data32(PALETTE_SRC + 4 * i);
            wr32(x.b, PALETTE_DST + 4 * i, v);
        }
        let tint = rd32(x.b, PALETTE_TINT) & 0xf000_f000;
        for i in 0..8 {
            let v = rd32(x.b, PALETTE_DST + 4 * i) | tint;
            wr32(x.b, PALETTE_DST + 4 * i, v);
        }
        x.set8(0x2a, 1);
        x.set8(0x2b, 0x18);
        x.set8(0x54, 0x3c);
        let row = x.u8(0x42) as u32;
        let dz = x.rom16(SPAWN_OFFSETS + row);
        x.add16(0xc, dz);
        let mut dx = x.rom16(SPAWN_OFFSETS + 2 + row);
        if x.u8(f::FLIP) != 0 {
            dx = dx.wrapping_neg();
        }
        x.add16(0x8, dx);
        x.add32(0x10, 0xffff_ffff);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 8);
        x.set8(f::ATTACK_ID, 2);
        x.anim_start(ANIMS, 0);
        x.reinsert();
    }

    /// Le parent a quitté ses états normaux : le projectile disparaît.
    fn check_parent(x: &mut Cx) {
        let parent = ptr(x, 0x56);
        if (parent.u8(x.b, 0x4) as i8) >= 4 {
            kill(x);
        }
    }

    fn active(x: &mut Cx) {
        let mode = x.u8(0x43);
        let state = x.u8(0x5);
        match state {
            CHARGE => charge(x),
            AIM => {
                if mode == 0 {
                    aim_flat(x)
                } else if (mode as i8) < 0 {
                    aim_drop(x)
                } else {
                    aim_angle(x)
                }
            }
            FALL => {
                if mode == 0 {
                    land(x, sub::MOVE_TABLE_XZ, true)
                } else if (mode as i8) < 0 {
                    land(x, sub::MOVE_XZ, true)
                } else {
                    land_xyz(x)
                }
            }
            _ => {}
        }
        x.reinsert();
    }

    /// Charge : animation 0 pendant 60 images.
    fn charge(x: &mut Cx) {
        let parent = ptr(x, 0x56);
        if (parent.u8(x.b, 0x4) as i8) >= 4 {
            return kill(x);
        }
        x.anim_step();
        x.add8(0x54, 0xff);
        if (x.u8(0x54) as i8) < 0 {
            x.add8(0x5, 2);
        }
        check_parent(x);
    }

    /// Début de la phase de tir : annonce au parent (`+0xa5`), attente de 20 images.
    fn begin_aim(x: &mut Cx) -> bool {
        if x.u8(0x6) == 0 {
            x.add8(0x6, 2);
            x.set8(0x54, 0x14);
            x.anim_start(ANIMS, 1);
            return false;
        }
        true
    }

    /// Après l'attente : prévient le parent, vrai quand le projectile doit partir.
    fn ready_to_go(x: &mut Cx) -> bool {
        x.anim_step();
        x.add8(0x54, 0xff);
        if (x.u8(0x54) as i8) >= 0 {
            return false;
        }
        let parent = ptr(x, 0x56);
        parent.set_u8(x.b, 0xa5, 1);
        x.u8(0x55) != 0
    }

    /// Mode > 0 : vise la cible (rapport distance/altitude, angle).
    fn aim_angle(x: &mut Cx) {
        if x.u8(0x6) == 0 {
            x.add8(0x6, 2);
            x.set8(0x54, 0x14);
            return x.anim_start(ANIMS, 1);
        }
        let parent = ptr(x, 0x56);
        if (parent.u8(x.b, 0x4) as i8) >= 4 {
            return kill(x);
        }
        if ready_to_go(x) {
            x.add8(0x5, 2);
            x.set8(0x2d, 0x10);
            if x.u8(0x3) == 8 {
                x.set32(0x14, 0);
                x.set32(0x18, 0xf800_0000);
                x.set32(0x1c, 0xfb80_0000);
            } else {
                let target = ptr(x, f::OWNER);
                if target.u8(x.b, 0) == 0 {
                    return x.set8(0x4, st::FREE);
                }
                aim_at(x, target);
            }
        }
        check_parent(x);
    }

    /// Calcule la vitesse verticale (pente vers la cible) et l'angle `+0x2d`.
    fn aim_at(x: &mut Cx, target: Obj) {
        let dy = x.u32(0x10).wrapping_sub(target.u32(x.b, 0x10));
        let mut dx = x.u32(0x8).wrapping_sub(target.u32(x.b, 0x8));
        if (dx as i32) < 0 {
            dx = dx.wrapping_neg();
        }
        let far = if (dx as i32) < (dy as i32) { dy } else { dx } >> 3;
        x.c.d[0] = x.u32(0xc);
        x.c.d[1] = far;
        x.call(DIVIDE);
        let quotient = x.c.d[0] << 7;
        let slope = (quotient as u16).wrapping_add(((far >> 16) as u16) << 2);
        x.set16(0x18, slope.wrapping_neg());
        x.c.a[1] = target.0;
        x.call(0x1376);
        let angle = (x.c.d[0] as u16 & 0x1f) as u8;
        let idx = angle.wrapping_sub(8) as u32;
        let v = x.rom8(AIM_TABLE_B + idx);
        x.set8(0x2d, v);
        speed_table(x, 0x40);
    }

    /// Mode 0 : tir par table d'angles.
    fn aim_flat(x: &mut Cx) {
        if !begin_aim(x) {
            return;
        }
        if ready_to_go(x) {
            x.add8(0x5, 2);
            let target = ptr(x, f::OWNER);
            if target.u8(x.b, 0) == 0 {
                return x.set8(0x4, st::FREE);
            }
            x.c.d[1] = target.u16(x.b, 0x8) as u32;
            x.c.d[4] = x.u16(0xc) as u32;
            x.c.d[3] = x.u16(0x8) as u32;
            x.c.d[2] = 0;
            x.call(ANGLE);
            let angle = (x.c.d[0] as u16 & 0x1f) as u8;
            let v = x.rom8(AIM_TABLE_A + angle.wrapping_sub(8) as u32);
            x.set8(0x2d, v);
            speed_table(x, 0x44);
            let v = x.u16(0x1c);
            x.set16(0x18, v);
        }
        check_parent(x);
    }

    /// Mode < 0 : tombe devant la cible (vitesse selon la distance en X).
    fn aim_drop(x: &mut Cx) {
        if !begin_aim(x) {
            return;
        }
        if ready_to_go(x) {
            x.add8(0x5, 2);
            let target = ptr(x, f::OWNER);
            if target.u8(x.b, 0) == 0 {
                return x.set8(0x4, st::FREE);
            }
            let mut d = target.u16(x.b, 0x8).wrapping_sub(x.u16(0x8));
            if (d as i16) < 0 {
                d = d.wrapping_neg();
            }
            let row = if d > 0x50 { 0 } else { 4 };
            let vz = x.rom16(DROP_SPEEDS + row);
            x.set16(0x18, vz);
            x.set16(0x1a, 0);
            x.set16(0x14, 0);
        }
        check_parent(x);
    }

    /// Chute après le tir (modes 0 et < 0) : touche le sol (Z < 0) → animation d'impact.
    fn land(x: &mut Cx, mover: u32, clear_state: bool) {
        x.call(mover);
        x.anim_step();
        if (x.u16(0xc) as i16) >= 0 {
            return;
        }
        x.add8(0x4, 2);
        if clear_state {
            x.set8(0x5, 0);
        }
        x.set32(0xc, 0);
        x.anim_start(ANIMS, 2);
        x.call(sfx::ED686);
    }

    /// Chute du mode > 0 : déplacement en X, Y et Z.
    fn land_xyz(x: &mut Cx) {
        x.call(sub::MOVE_XYZ);
        x.anim_step();
        if (x.u16(0xc) as i16) > 0 {
            return;
        }
        x.add8(0x4, 2);
        x.set8(0x2a, 0);
        x.set32(0xc, 0);
        x.anim_start(ANIMS, 2);
        x.call(sfx::ED686);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 18 (0xa0a06) : projectile oblique à vitesse tirée d'une table (h)
// ---------------------------------------------------------------------------------------------

/// Projectile à sens selon l'orientation, table de vitesses 1b64 choisie par une variable globale
/// (A5+0x7f92) ; détruit au contact, sur certains terrains, ou hors écran.
mod shot18 {
    use super::*;

    const ANIMS: u32 = 0x16e534;
    /// Numéro de table de vitesses (octet) selon A5+0x7f92.
    const SPEED_TABLES: u32 = 0xa0a60;
    const LEVEL_VARIABLE: u32 = 0x7f92;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 6);
                x.set8(f::ATTACK_ID, 0x1a);
                x.set8(0x2d, if x.u8(f::FLIP) != 0 { 8 } else { 0x18 });
                let level = x.a5_u16(LEVEL_VARIABLE);
                let table = x.rom8(SPEED_TABLES.wrapping_add(level as i16 as i32 as u32));
                speed_table(x, level as u32 & 0xff00 | table as u32);
                x.anim_start(ANIMS, 7);
            }
            2 => fly(x),
            _ => x.free_eshl(),
        }
    }

    fn fly(x: &mut Cx) {
        let hit = x.u8(f::HIT_FLAGS);
        if hit & 0x08 != 0 {
            x.call(sfx::E7376);
            return x.free_eshl();
        }
        if hit & 0x20 != 0 {
            x.add8(0x4, 2);
            return x.free_eshl();
        }
        if blocked_by_terrain(x) {
            return x.free_eshl();
        }
        x.call(sub::MOVE_TABLE_XY);
        x.anim_step();
        x.call(sub::CULL_OFFSCREEN);
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 19 (0xa0b20) : projectile lancé vers le joueur, sprite selon l'angle (h)
// ---------------------------------------------------------------------------------------------

/// Projectile qui choisit sa direction (8 sprites) d'après l'angle vers la cible (`+0x50`, pointeur
/// long), est lancé avec la table de vitesses 0x40 (VZ = VY), retombe jusqu'à Z < 0 puis reste 48
/// images en animation d'impact.
mod shot19 {
    use super::*;

    const ANIMS: u32 = 0x16e5fa;
    /// Lignes de 16 octets (une par sens) : (valeur de `+0x2d`, animation) par pas d'angle.
    const STEPS_FACING_RIGHT: u32 = 0xa0bea;
    const STEPS_FACING_LEFT: u32 = 0xa0bda;
    /// Décalages (X, Z) du sprite par pas d'angle.
    const OFFSETS: u32 = 0xa0bfa;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => init(x),
            2 => {
                if (x.u16(0xc) as i16) >= 0 {
                    x.call(sub::MOVE_XZ);
                } else {
                    x.set32(0xc, 0);
                    x.add8(0x4, 2);
                    x.set8(0x58, 0x30);
                    x.anim_start(ANIMS, 0x10);
                }
            }
            4 => {
                x.anim_step();
                x.add8(0x58, 0xff);
                if (x.u8(0x58) as i8) < 0 {
                    x.add8(0x4, 2);
                }
            }
            _ => x.free_eshl(),
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 5);
        x.set8(f::ATTACK_ID, 0x19);
        let target = ptr(x, f::OWNER);
        if target.u8(x.b, 0) == 0 {
            return x.add8(0x6, 4);
        }
        x.c.d[3] = x.u16(0x8) as u32;
        x.c.d[4] = x.u16(0xc) as u32;
        x.c.d[1] = target.u16(x.b, 0x8) as u32;
        x.c.d[2] = 0;
        x.call(ANGLE);
        let angle = (x.c.d[0] as u16 & 0x1f) as u8;
        let (base, steps) = if x.u8(f::FLIP) != 0 { (9, STEPS_FACING_LEFT) } else { (0x10, STEPS_FACING_RIGHT) };
        let r = angle.wrapping_sub(base);
        let step = if r == 0 {
            0
        } else if (r as i8) < 0 || r > 8 {
            7
        } else {
            r as u32
        };
        let entry = steps + 2 * step;
        let sprite = x.rom8(entry);
        x.set8(0x2d, sprite);
        let anim = x.rom8(entry + 1) as i8 as i16 as u16 as u32;
        let off = OFFSETS + 4 * step;
        let dx = x.rom16(off);
        x.add16(0x8, dx);
        let dz = x.rom16(off + 2);
        x.add16(0xc, dz);
        x.set8(f::FLIP, 0);
        x.anim_start(ANIMS, anim);
        speed_table(x, 0x40);
        let v = x.u16(0x1c);
        x.set16(0x18, v);
        x.set16(0x1c, 0);
        x.reinsert();
    }
}

handlers! {
    0x9fe0a "eshl_15" 600 shot15::run,
    0xa03e0 "eshl_16" 600 shot16::run,
    0xa05ce "eshl_17" 600 shot17::run,
    0xa0a06 "eshl_18" 600 shot18::run,
    0xa0b20 "eshl_19" 600 shot19::run,
}
