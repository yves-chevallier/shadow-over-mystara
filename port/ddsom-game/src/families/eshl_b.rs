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
                let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
                x.set8(0x2d, angle);
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
        let sprite = x.data8(entry);
        x.set8(0x2d, sprite);
        let anim = x.data8(entry + 1) as i8 as i16 as u16 as u32;
        let off = OFFSETS + 4 * step;
        let dx = x.data16(off);
        x.add16(0x8, dx);
        let dz = x.data16(off + 2);
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

// ---------------------------------------------------------------------------------------------
// Aide commune des types 20 et 21 : collage au propriétaire par une table de décalages
// ---------------------------------------------------------------------------------------------

/// Long lu dans l'espace programme (table adressée relativement au PC).
fn rom32(x: &mut Cx, addr: u32) -> u32 {
    (x.rom16(addr) as u32) << 16 | x.rom16(addr + 2) as u32
}

/// Se colle au propriétaire avec le décalage de la ligne `variante` d'une table (8 octets par ligne :
/// X, Y entier, Y fractionnaire signé, Z). Sans propriétaire, l'objet est libéré à l'image suivante.
fn attach(x: &mut Cx, table: u32) {
    if x.u16(f::OWNER) == 0 {
        return kill(x);
    }
    let owner = x.linked(f::OWNER);
    copy_position32(x, owner);
    let flip = owner.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    let row = table + (x.u8(0x3) << 3) as u32;
    let mut dx = x.rom16(row);
    if flip != 0 {
        dx = dx.wrapping_neg();
    }
    x.add16(0x8, dx);
    let dy = x.rom16(row + 2);
    x.add16(0x10, dy);
    let dy_fraction = x.rom16(row + 4) as i16 as i32 as u32;
    x.add32(0x10, dy_fraction);
    let dz = x.rom16(row + 6);
    x.add16(0xc, dz);
}

// ---------------------------------------------------------------------------------------------
// Type 20 (0xa0c5a) : coup d'une partie d'ennemi (h), deux variantes
// ---------------------------------------------------------------------------------------------

/// Arme collée à son propriétaire (`attach`). Variante 0 : animation d'attente tirée au hasard puis,
/// quand le propriétaire arme son attaque (`+0xa5` bit 0), état 2 sous-état 2 : animation de coup
/// qui crée un tir de type 21 ; variante 1 : arme à retardement (`+0x5c` images), qui frappe
/// aussi. Les états gelés par la pause (`A5+0x3d`, `A5+0x4e`) ou l'objet suspendu la figent.
mod shot20 {
    use super::*;

    const ANIMS: u32 = 0x17d7d2;
    const ANIMS_STRIKE: u32 = 0x17d7e0;
    const ATTACH: u32 = 0xa0f86;
    /// Animation d'attente selon un tirage 0..7.
    const IDLE_ANIMS: u32 = 0xa0d60;
    /// Attente (images) de la variante 1 selon un tirage 0..7.
    const DELAYS: u32 = 0xa0e2c;
    const FROZEN: u32 = 0x4e;
    const ARMED: u32 = 0xa5;
    /// Sous-états (`+5`).
    const IDLE: u8 = 0;
    const STRIKE: u8 = 2;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            _ => x.free_eshl(),
        }
    }

    fn init(x: &mut Cx) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return;
        }
        x.add8(0x4, 2);
        x.set8(0x1d, 0);
        attach(x, ATTACH);
        let mut anim = 3;
        if x.u8(0x3) != 0 {
            x.set16(f::TIMER, 0x1e);
            anim = 2;
        }
        x.anim_start(ANIMS, anim);
    }

    fn active(x: &mut Cx) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return;
        }
        if x.a5_u8(FROZEN) != 0 || suspended(x) {
            return freeze(x);
        }
        match x.u8(0x5) {
            IDLE => idle(x),
            STRIKE => strike(x),
            _ => {}
        }
        x.reinsert();
    }

    /// Objet figé : la variante 0 repasse une fois à l'état 2 et rend l'arme au propriétaire.
    fn freeze(x: &mut Cx) {
        if x.u8(0x3) != 0 || x.u8(0x1d) != 0 {
            return;
        }
        x.set8(0x1d, 1);
        x.set32(0x4, 0x0200_0000);
        let owner = x.linked(f::OWNER);
        let armed = owner.u8(x.b, ARMED) & !1;
        owner.set_u8(x.b, ARMED, armed);
        x.call(sfx::E5020);
        x.anim_start(ANIMS, 3);
    }

    fn idle(x: &mut Cx) {
        match (x.u8(0x6), x.u8(0x3)) {
            (0, 0) => {
                x.set8(0x1d, 0);
                x.add8(0x6, 2);
                x.c.d[0] = 0;
                x.call(RNG);
                let anim = x.rom8(IDLE_ANIMS + (x.c.d[0] & 7)) as u32;
                x.anim_start(ANIMS, anim);
                attach(x, ATTACH);
                x.anim_step();
            }
            (0, _) => {
                x.add16(f::TIMER, 0xffff);
                if (x.u16(f::TIMER) as i16) < 0 {
                    x.add8(0x6, 2);
                    x.set8(f::BOX_SET, 7);
                    x.set8(f::ATTACK_ID, 0x28);
                    x.anim_start(ANIMS_STRIKE, 0);
                }
                attach(x, ATTACH);
                x.anim_step();
            }
            (2, 0) => {
                let owner = x.linked(f::OWNER);
                if (owner.u32(x.b, 0x4) as i32) >= 0x0200_0600 && x.u8(0x33) != 0 {
                    x.set8(0x33, 0);
                    x.call(sfx::E4E5A);
                }
                if x.anim_step() {
                    if x.u16(f::OWNER) == 0 {
                        return kill(x);
                    }
                    let owner = x.linked(f::OWNER);
                    if owner.u8(x.b, ARMED) & 1 != 0 {
                        return x.set32(0x4, 0x0202_0000);
                    }
                    x.add8(0x6, 0xfe);
                }
                attach(x, ATTACH);
                x.anim_step();
            }
            (2, _) => {
                if x.anim_step() {
                    x.add8(0x6, 0xfe);
                    x.c.d[0] = 0;
                    x.call(RNG);
                    let delay = x.rom16(DELAYS + 2 * (x.c.d[0] & 7));
                    x.set16(f::TIMER, delay);
                    x.anim_start(ANIMS, 2);
                } else {
                    attach(x, ATTACH);
                }
            }
            _ => {}
        }
    }

    fn strike(x: &mut Cx) {
        match x.u8(0x6) {
            0 => {
                x.add8(0x6, 2);
                x.anim_start(ANIMS_STRIKE, 1);
                attach(x, ATTACH);
                x.anim_step();
            }
            2 => {
                if !x.anim_step() {
                    return attach(x, ATTACH);
                }
                x.call(sfx::E4E5A);
                x.call(sfx::E4FB6);
                x.add8(0x6, 2);
                x.call(sub::ALLOC_ESHL);
                let blow = Obj(x.c.a[1]);
                let active = x.u8(0x0);
                blow.set_u8(x.b, 0, active);
                blow.set_u8(x.b, 3, 0);
                blow.set_u8(x.b, 2, 0x15);
                blow.set_u16(x.b, f::OWNER, low16(x.o));
                x.set16(0x42, low16(blow));
                x.anim_start(ANIMS_STRIKE, 2);
            }
            4 => {
                let blow_gone = x.u16(0x42) == 0 || {
                    let blow = x.linked(0x42);
                    blow.u8(x.b, 0x1e) == 1
                };
                if blow_gone {
                    x.add8(0x6, 2);
                    x.anim_start(ANIMS_STRIKE, 3);
                } else {
                    attach(x, ATTACH);
                    x.anim_step();
                }
            }
            6 => {
                if x.anim_step() {
                    x.set32(0x4, 0x0200_0000);
                    let owner = x.linked(f::OWNER);
                    let armed = owner.u8(x.b, ARMED) & !1;
                    owner.set_u8(x.b, ARMED, armed);
                    x.call(sfx::E5020);
                    x.anim_start(ANIMS, 3);
                } else {
                    attach(x, ATTACH);
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 21 (0xa0f96) : onde / souffle attaché à une arme (h)
// ---------------------------------------------------------------------------------------------

/// Tir créé par le type 20 : collé au propriétaire (`attach`), animation de 0x4f répétitions
/// (`+0x42`) pendant lesquelles il crée un effet (EFFCT 0x18, mémorisé en `+0x46`), puis
/// animation de fin. Il disparaît si le jeu est figé (`A5+0x4e`) ou si le possesseur de son
/// propriétaire est dans l'état `+0xb2`.
mod shot21 {
    use super::*;

    const ANIMS: u32 = 0x16e752;
    const ATTACH: u32 = 0xa113a;
    const LOOP: u8 = 0;
    const END: u8 = 2;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            _ => {
                x.call(sfx::E5020);
                x.free_eshl()
            }
        }
    }

    fn init(x: &mut Cx) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return;
        }
        x.add8(0x4, 2);
        attach(x, ATTACH);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 0xb);
        x.set8(0x42, 0x4f);
        x.set8(0x1e, 0);
        x.anim_start(ANIMS, 2);
        x.call(sfx::E4FB6);
    }

    fn burn(x: &mut Cx) {
        x.set8(0x1e, 1);
        kill(x);
    }

    fn active(x: &mut Cx) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return;
        }
        if x.a5_u8(0x4e) != 0 {
            return burn(x);
        }
        let weapon = x.linked(f::OWNER);
        let host = link_of(x, weapon, f::OWNER);
        if host.u8(x.b, 0xb2) != 0 || suspended(x) {
            return burn(x);
        }
        match x.u8(0x5) {
            LOOP => {
                attach(x, ATTACH);
                if !x.anim_step() {
                    return x.reinsert();
                }
                x.add8(0x42, 0xff);
                if (x.u8(0x42) as i8) < 0 {
                    let effect = x.linked(0x46);
                    effect.set_u8(x.b, 0x43, 1);
                    x.add8(0x5, 2);
                    x.anim_start(ANIMS, 1);
                } else {
                    if x.u8(0x46) == 0 {
                        if let Some(e) = alloc(x, ALLOC_EFFCT) {
                            e.set_u8(x.b, 0, 1);
                            e.set_u8(x.b, 3, 0);
                            e.set_u8(x.b, 2, 0x18);
                            e.set_u16(x.b, f::OWNER, low16(x.o));
                            x.set16(0x46, low16(e));
                        }
                    }
                    x.anim_start(ANIMS, 0);
                }
            }
            END => {
                attach(x, ATTACH);
                if x.anim_step() {
                    burn(x);
                }
            }
            _ => {}
        }
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 22 (0xa1180) : éclat lancé par un ennemi (h)
// ---------------------------------------------------------------------------------------------

/// Éclat qui part de la position du créateur avec une vitesse tirée d'une table par variante
/// (vitesses en X et Z), retombe jusqu'à Z <= 0 puis clignote 30 images.
mod shot22 {
    use super::*;

    const ANIMS: u32 = 0x1649d0;
    /// Décalages (X, Z) selon `+0x2f` (4 octets par ligne).
    const OFFSETS: u32 = 0xa1194;
    /// Vitesses (VX+AX, VZ+AZ) selon la variante (8 octets par ligne).
    const SPEEDS: u32 = 0xa123a;
    const FLY: u8 = 0;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => init(x),
            2 => {
                if x.u8(0x5) == FLY {
                    x.call(sub::MOVE_XZ);
                    if (x.u16(0xc) as i16) > 0 {
                        return;
                    }
                    x.set32(0xc, 0);
                    x.add8(0x5, 2);
                } else {
                    let hidden = !x.bit(0x1, 1);
                    x.set_bit(0x1, 1, hidden);
                    x.add8(0x54, 0xff);
                    if x.u8(0x54) == 0 {
                        x.free_eshl();
                    }
                }
            }
            _ => x.free_eshl(),
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(0x54, 0x1e);
        x.set8(f::ATTACK_ID, 0x1a);
        x.set8(f::ATTACK_POWER, 5);
        let row = OFFSETS + ((x.u8(0x2f) << 2) as u32);
        let mut dx = x.rom16(row);
        let dz = x.rom16(row + 2);
        x.add16(0xc, dz);
        x.add16(0x10, 0xffff);
        x.add16(0xc, 1);
        let variant = x.u8(0x3);
        let row = SPEEDS + ((variant as u32) << 3);
        let vx = rom32(x, row);
        x.set32(0x14, vx);
        let vz = rom32(x, row + 4);
        x.set32(0x18, vz);
        if x.u8(f::FLIP) != 0 {
            dx = dx.wrapping_neg();
            x.set32(0x14, vx.wrapping_neg());
        }
        x.add16(0x8, dx);
        x.anim_start(ANIMS, variant.wrapping_add(10) as u32);
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 23 (0xa1292) : objet de décor qui glisse devant la caméra (h)
// ---------------------------------------------------------------------------------------------

/// Objet placé à la bordure de la caméra (`A5+0x556`), qui se déplace d'après la table de vitesses 0xd
/// pendant 48 images, attend le signal `A5+0x42` puis repart avec la table 2.
mod shot23 {
    use super::*;

    const ANIMS: u32 = 0x1c03a6;
    const WAIT: u8 = 0;
    const GO: u8 = 2;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => init(x),
            2 => match x.u8(0x5) {
                WAIT => {
                    x.add8(0x54, 0xff);
                    if (x.u8(0x54) as i8) < 0 {
                        x.add8(0x5, 2);
                    }
                    slide(x);
                }
                _ => {
                    if x.a5_u8(0x42) == 0 {
                        x.anim_step();
                        return;
                    }
                    x.set8(0x54, 0x30);
                    x.add8(0x4, 2);
                    x.set8(0x2d, 0x10);
                    speed_table(x, 2);
                    x.anim_step();
                }
            },
            4 => {
                x.add8(0x54, 0xff);
                if (x.u8(0x54) as i8) < 0 {
                    x.add8(0x4, 2);
                }
                slide(x);
            }
            _ => x.free_eshl(),
        }
    }

    fn slide(x: &mut Cx) {
        x.call(sub::MOVE_TABLE_XY);
        x.anim_step();
        reinsert_foreground(x);
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(0x54, 0x30);
        x.set8(0x2d, 0);
        speed_table(x, 0xd);
        let camera = x.a5_u16(0x556);
        x.set16(0x8, camera);
        x.set32(0xc, 0);
        x.set32(0x10, 0);
        x.set16(0xc, 0x70);
        x.add16(0x10, -0x20i16 as u16);
        let mut anim = 0x4a;
        if x.u8(0x2f) != 0 {
            anim += 0x9f;
            x.add16(0x10, 0x80);
            x.add16(0xc, -0x10i16 as u16);
        }
        x.anim_start(ANIMS, anim);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 24 (0xa137e) : vague d'ennemis / de tirs lancée par un créateur (h)
// ---------------------------------------------------------------------------------------------

/// Variante 0 : générateur. Il se place sur son créateur (`+0x5c`), puis pendant `+0x58` images il
/// lance un tir de type 24 (variante 1) toutes les 2 images et, à `+0x58 == 0x20`, fait apparaître
/// un groupe d'ennemis (ENEMY type 1, nombre et positions selon `A5+0x7f92` et `A5+0x35`).
/// Variante 1 : effet qu'il lance, à l'une des 16 positions de la table 0xa1438 (`+0x56`).
mod shot24 {
    use super::*;

    const ANIMS: u32 = 0x16e920;
    /// Décalages (X, Z, Y) des 16 effets (8 octets par ligne).
    const OFFSETS: u32 = 0xa1438;
    /// Nombre d'ennemis selon (A5+0x7f92 / 2) * 4 + A5+0x35.
    const GROUP_SIZES: u32 = 0xa15ae;
    /// Positions relatives (X, Y) des ennemis : 2 paires par ligne, tirées au hasard.
    const GROUP_OFFSETS: u32 = 0xa15ee;
    const GROUP_SIZE_SELECTOR: u32 = 0x7f92;
    const SPAWN_ENEMIES_AT: u16 = 0x20;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn copy_position16(x: &mut Cx, from: Obj) {
        for off in [0x8, 0xc, 0x10] {
            let v = from.u16(x.b, off);
            x.set16(off, v);
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        let source = x.linked(0x5c);
        if x.u8(0x3) == 0 {
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_ID, 0x2f);
            x.set16(0x58, 0x50);
            x.set16(0x5a, 1);
            x.set16(0x56, 0);
            copy_position16(x, source);
            x.anim_start(ANIMS, 2);
            x.reinsert();
            x.set32(0x34, 0);
            active(x);
        } else {
            copy_position16(x, source);
            let row = OFFSETS + ((x.u16(0x56) << 3) as u32);
            for (off, column) in [(0x8, 0), (0xc, 2), (0x10, 4)] {
                let d = x.rom16(row + column);
                x.add16(off, d);
            }
            x.anim_start(ANIMS, 1);
            x.reinsert();
        }
    }

    fn active(x: &mut Cx) {
        if x.u8(0x3) != 0 {
            if x.anim_step() {
                x.add8(0x4, 2);
            }
            return;
        }
        x.add16(0x58, 0xffff);
        if x.u16(0x58) == 0 {
            x.add8(0x4, 2);
        }
        if x.u16(0x58) == SPAWN_ENEMIES_AT {
            spawn_enemies(x);
        }
        x.add16(0x5a, 0xffff);
        if x.u16(0x5a) == 0 {
            x.set16(0x5a, 2);
            spawn_effect(x);
        }
    }

    /// Lance l'effet suivant (variante 1) et avance `+0x56` modulo 16.
    fn spawn_effect(x: &mut Cx) {
        if let Some(e) = alloc(x, sub::ALLOC_ESHL) {
            e.set_u8(x.b, 0, 1);
            e.set_u8(x.b, 2, 0x18);
            e.set_u8(x.b, 3, 1);
            let (n, source) = (x.u16(0x56), x.u16(0x5c));
            e.set_u16(x.b, 0x56, n);
            e.set_u16(x.b, 0x5c, source);
            x.set16(0x56, n.wrapping_add(1) & 0xf);
        }
    }

    /// Fait apparaître le groupe d'ennemis autour du créateur.
    fn spawn_enemies(x: &mut Cx) {
        let mut d0 = x.a5_u8(0x35) as u16;
        if d0 != 0 {
            d0 = d0.wrapping_sub(1) & 3;
        }
        d0 = d0.wrapping_add((x.a5_u16(GROUP_SIZE_SELECTOR) >> 1) << 2);
        let mut count = x.rom8(GROUP_SIZES.wrapping_add(d0 as i16 as i32 as u32)) as u16;
        count = count.wrapping_sub(1);
        let mut label: u16 = 1;
        x.c.d[0] = 0;
        x.call(RNG);
        let mut table = GROUP_OFFSETS + ((x.c.d[0] & 7) << 3);
        loop {
            let Some(e) = alloc(x, ALLOC_ENEMY) else { return };
            e.set_u8(x.b, 0, 1);
            e.set_u8(x.b, 2, 1);
            e.set_u8(x.b, 3, 4);
            let source = x.linked(0x5c);
            let (sx, sy) = (source.u16(x.b, 0x8), source.u16(x.b, 0x10));
            let (dx, dy) = (x.data16(table), x.data16(table + 2));
            table += 4;
            e.set_u16(x.b, 0x8, sx.wrapping_add(dx));
            e.set_u16(x.b, 0x10, sy.wrapping_add(dy));
            e.set_u16(x.b, 0x42, label);
            label = label.wrapping_add(0x40);
            count = count.wrapping_sub(1);
            if count == 0xffff {
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 25 (0xa16bc) : projectile qui tombe du ciel sur une cible (h)
// ---------------------------------------------------------------------------------------------

/// Après un délai (`+0x58` images), tombe depuis la hauteur de la caméra + 0x100 au-dessus de la
/// position du créateur (`+0x5c`), avec un décalage aléatoire pour les variantes non nulles.
/// À l'impact (Z < 0) : sons et effets, puis il fait passer son ombre (`+0x5a`) à l'état 6.
mod shot25 {
    use super::*;

    const ANIMS: u32 = 0x16ea18;
    const SHADOW: u32 = 0x54818;
    const IMPACT_SOUND: u32 = 0xab7d8;
    const IMPACT_SOUND_2: u32 = 0x2860;
    const IMPACT_EFFECT: u32 = 0xeafdc;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => wait_and_drop(x),
            2 => fall(x),
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn wait_and_drop(x: &mut Cx) {
        x.add16(0x58, 0xffff);
        if x.u16(0x58) != 0 {
            return;
        }
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 0x17);
        let source = x.linked(0x5c);
        let (sx, sy) = (source.u16(x.b, 0x8), source.u16(x.b, 0x10));
        x.set16(0x8, sx);
        x.set16(0x10, sy);
        let camera = if x.a5_u8(0x36) != 0 { x.a5_u16(0x55a) } else { x.a5_u16(0x50a) };
        x.set16(0xc, camera.wrapping_add(0x100).wrapping_sub(sy));
        x.set16(0x18, -0x100i16 as u16);
        x.set16(0x1a, -0x40i16 as u16);
        let variant = x.u8(0x3);
        if variant != 0 {
            x.c.d[0] = 0;
            x.call(RNG);
            let mut dx = x.c.d[0] as u16 & 0x7f;
            if variant & 1 != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(0x8, dx);
            x.call(RNG);
            let r = x.c.d[0] as u16 & 0x3f;
            let dy = if r & 1 != 0 { 0xffff } else { 0 };
            x.add16(0x10, dy);
        }
        x.anim_start(ANIMS, 0);
        x.call(SHADOW);
        if x.c.cond(6) {
            x.set16(0x5a, low16(Obj(x.c.a[1])));
        }
        x.reinsert();
    }

    fn fall(x: &mut Cx) {
        x.anim_step();
        x.call(sub::MOVE_Z);
        if (x.u16(0xc) as i16) >= 0 {
            return;
        }
        x.add8(0x4, 2);
        x.set32(0xc, 0);
        x.c.d[0] = 3;
        x.call(IMPACT_SOUND);
        x.c.d[0] = x.c.d[0] & 0xffff_0000 | 8;
        x.call(IMPACT_SOUND_2);
        x.call(IMPACT_EFFECT);
        if x.u16(0x5a) != 0 {
            let shadow = x.linked(0x5a);
            shadow.set_u8(x.b, 0x4, 6);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 26 (0xa18dc) : projectile lancé en cloche d'un créateur, avec ombre (h)
// ---------------------------------------------------------------------------------------------

/// Après un délai, part de la position du créateur (`+0x5c`) à une hauteur de la table 0xa1968 (selon
/// `+0x60` du créateur), avec la table de vitesses 3 (ou 2). Retombe (Z < 0), reste 128 images
/// au sol puis 96 en clignotant, avant de disparaître.
mod shot26 {
    use super::*;

    const ANIMS: u32 = 0x16ea90;
    const HEIGHTS: u32 = 0xa1968;
    const SPEED_TABLES: u32 = 0xa1972;
    const SHADOW: u32 = 0x54842;
    const LAND_SOUND: u32 = 0xeb1a4;
    const FLY: u8 = 0;
    const GROUND: u8 = 2;
    const BLINK: u8 = 4;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => init(x),
            2 => {
                match x.u8(0x5) {
                    FLY => fly(x),
                    GROUND => ground(x),
                    BLINK => blink_out(x),
                    _ => {}
                }
                x.reinsert();
            }
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn init(x: &mut Cx) {
        x.add16(0x58, 0xffff);
        if x.u16(0x58) != 0 {
            return;
        }
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        let source = x.linked(0x5c);
        let (sx, sy) = (source.u16(x.b, 0x8), source.u16(x.b, 0x10));
        x.set16(0x8, sx);
        x.set16(0x10, sy);
        let row = x.u8(0x60) as u32;
        let height = x.rom16(HEIGHTS + 2 * row);
        x.set16(0xc, height);
        x.c.d[0] = 0;
        x.call(RNG);
        let mut table = x.rom16(SPEED_TABLES + 2 * (x.c.d[0] & 3)) as u32;
        if x.u8(0x56) != 0 && x.u8(0x3) == 0 {
            table = 2;
        }
        speed_table(x, table);
        x.set16(0x18, 0x300);
        x.set16(0x1a, -0x30i16 as u16);
        x.anim_start(ANIMS, 0);
        x.reinsert();
    }

    fn fly(x: &mut Cx) {
        x.anim_step();
        x.call(sub::MOVE_XYZ);
        if (x.u16(0xc) as i16) >= 0 {
            return;
        }
        x.add8(0x5, 2);
        x.set32(0xc, 0);
        x.set16(0x58, 0x80);
        x.anim_start(ANIMS, 1);
        x.call(LAND_SOUND);
        x.call(SHADOW);
        if x.c.cond(6) {
            x.set16(0x5a, low16(Obj(x.c.a[1])));
        }
    }

    fn ground(x: &mut Cx) {
        x.anim_step();
        x.add16(0x58, 0xffff);
        if x.u16(0x58) != 0 {
            return;
        }
        x.add8(0x5, 2);
        x.set16(0x58, 0x60);
        x.anim_start(ANIMS, 2);
        if x.u16(0x5a) != 0 {
            let shadow = x.linked(0x5a);
            shadow.set_u8(x.b, 0x4, 6);
        }
    }

    fn blink_out(x: &mut Cx) {
        x.anim_step();
        x.add16(0x58, 0xffff);
        if x.u16(0x58) == 0 {
            x.add8(0x4, 2);
        }
        let hidden = x.u16(0x58) & 1 == 0;
        x.set_bit(0x1, 1, hidden);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 27 (0xa1a64) : rideau d'éclats d'un créateur, deux variantes (h)
// ---------------------------------------------------------------------------------------------

/// Variante 0 : épicentre (animation 0, 96 images) posé sur le créateur (`+0x5c`) ; à 80 images
/// il crée une rangée de tirs de type 27 variante 1 de part et d'autre (`spawn_row`) et le
/// son. Variante 1 : un élément de la rangée, clignotant, qui disparaît avec l'épicentre.
mod shot27 {
    use super::*;

    const ANIMS: u32 = 0x16ec24;
    const SOUND_END: u32 = 0xed90a;
    const SOUND_ROW: u32 = 0xed40a;
    const ROW_AT: u16 = 0x50;
    const RISE: u8 = 0;
    const FADE: u8 = 2;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn init(x: &mut Cx) {
        if x.u8(0x3) == 0 {
            x.add8(0x4, 2);
            let source = x.linked(0x5c);
            let (sx, sy) = (source.u16(x.b, 0x8), source.u16(x.b, 0x10));
            x.set16(0x8, sx);
            x.set16(0x10, sy);
            x.set16(0xc, 0x20);
            x.add16(0x10, 0xffff);
            x.set16(0x58, 0x60);
        } else {
            x.add16(0x58, 0xffff);
            if x.u16(0x58) != 0 {
                return;
            }
            x.add8(0x4, 2);
        }
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 0x16);
        let anim = (x.u8(0x3) != 0) as u32;
        x.anim_start(ANIMS, anim);
        x.reinsert();
    }

    fn active(x: &mut Cx) {
        if x.u8(0x3) == 0 {
            x.add16(0x58, 0xffff);
            if x.u16(0x58) == 0 {
                x.add8(0x4, 2);
                x.set8(0x0, 2);
                x.call(SOUND_END);
            }
            x.anim_step();
            if x.u16(0x58) == ROW_AT {
                spawn_row(x);
                x.call(SOUND_ROW);
            }
            return;
        }
        match x.u8(0x5) {
            RISE => {
                x.anim_step();
                let source = x.linked(0x5c);
                if source.u16(x.b, 0x58) <= 0x30 {
                    x.add8(0x5, 2);
                    x.anim_start(ANIMS, 2);
                }
                flicker(x);
            }
            FADE => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
                flicker(x);
            }
            _ => {}
        }
    }

    /// Caché une image sur deux d'après le compteur d'images `A5+0x6f`.
    fn flicker(x: &mut Cx) {
        let hidden = x.a5_u8(0x6f) & 1 == 0;
        x.set_bit(0x1, 1, hidden);
    }

    /// Crée la rangée d'éléments (variante 1) de part et d'autre, depuis le créateur de l'épicentre
    /// ou le bord de la caméra, jusqu'aux limites de l'écran.
    fn spawn_row(x: &mut Cx) {
        let mut label: u16 = 1;
        let camera_x = |x: &mut Cx| if x.a5_u8(0x36) != 0 { x.a5_u16(0x556) } else { x.a5_u16(0x506) };
        let mut px = x.u16(0x8);
        let rightwards;
        if x.u8(0x56) == 0 {
            let target = x.linked(f::OWNER);
            if px <= target.u16(x.b, 0x8) {
                rightwards = true;
                px = px.wrapping_add(0x20);
            } else {
                rightwards = false;
                px = px.wrapping_sub(0x20);
            }
        } else {
            let limit = camera_x(x).wrapping_add(0xc0);
            if px <= limit {
                rightwards = true;
                px = px.wrapping_add(0x20);
            } else {
                rightwards = false;
                px = px.wrapping_sub(0x20);
            }
        }
        loop {
            let Some(e) = alloc(x, sub::ALLOC_ESHL) else { return };
            e.set_u8(x.b, 0, 1);
            e.set_u8(x.b, 2, 0x1b);
            e.set_u8(x.b, 3, 1);
            e.set_u16(x.b, 0x8, px);
            let (z, y) = (x.u16(0xc), x.u16(0x10));
            e.set_u16(x.b, 0xc, z.wrapping_sub(1));
            e.set_u16(x.b, 0x10, y.wrapping_add(1));
            e.set_u16(x.b, 0x58, label);
            label = label.wrapping_add(1);
            e.set_u16(x.b, 0x5c, low16(x.o));
            let camera = camera_x(x);
            if rightwards {
                px = px.wrapping_add(0x38);
                if px > camera.wrapping_add(0x1c0) {
                    return;
                }
            } else {
                px = px.wrapping_sub(0x38);
                if camera > px {
                    return;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 28 (0xa1c96) : fragment tournant autour d'un créateur (h)
// ---------------------------------------------------------------------------------------------

/// Fragment (4 variantes, `+3`) lancé depuis le créateur (`+0x5c`), à une hauteur de la table
/// 0xa1d18 selon `+0x60` du créateur, avec la table de vitesses 0x3f ; il avance et disparaît
/// hors écran (test une image sur 4).
mod shot28 {
    use super::*;

    const ANIMS: u32 = 0x16ee30;
    const HEIGHTS: u32 = 0xa1d18;
    const ANGLES: u32 = 0xa1d22;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x2e);
                let source = x.linked(0x5c);
                for off in [0x8, 0xc, 0x10] {
                    let v = source.u16(x.b, off);
                    x.set16(off, v);
                }
                let row = (source.u8(x.b, 0x60) & 0xf) as u32;
                let dz = x.rom16(HEIGHTS + 2 * row);
                x.add16(0xc, dz);
                x.add16(0x10, 0xffff);
                let variant = x.u8(0x3);
                let angle = x.rom8(ANGLES + (variant & 3) as u32);
                x.set8(0x2d, angle);
                speed_table(x, 0x3f);
                x.anim_start(ANIMS, variant as u32);
                x.reinsert();
            }
            2 => {
                x.call(sub::MOVE_TABLE_XY);
                if x.a5_u8(0x6f).wrapping_add(x.u8(0x3)) & 3 == 0 {
                    x.call(sub::CULL_OFFSCREEN);
                }
                x.reinsert();
            }
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }
}

handlers! {
    0x9fe0a "eshl_15" 600 shot15::run,
    0xa03e0 "eshl_16" 600 shot16::run,
    0xa05ce "eshl_17" 600 shot17::run,
    0xa0a06 "eshl_18" 600 shot18::run,
    0xa0b20 "eshl_19" 600 shot19::run,
    0xa0c5a "eshl_20" 600 shot20::run,
    0xa0f96 "eshl_21" 600 shot21::run,
    0xa1180 "eshl_22" 600 shot22::run,
    0xa1292 "eshl_23" 600 shot23::run,
    0xa137e "eshl_24" 600 shot24::run,
    0xa16bc "eshl_25" 600 shot25::run,
    0xa18dc "eshl_26" 600 shot26::run,
    0xa1a64 "eshl_27" 600 shot27::run,
    0xa1c96 "eshl_28" 600 shot28::run,
}
