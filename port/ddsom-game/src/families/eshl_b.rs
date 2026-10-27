//! Tirs et attaques des ennemis, types ESHL 15 à 39 (suite de `eshl.rs`). Voir `docs/familles/eshl_b.md`.
//!
//! Mêmes conventions que `eshl.rs` : chaque type est un `mod` avec une fonction `run` ; `+4` est l'état
//! principal, `+5` et `+6` des sous-états (octets pairs, indices de tables de sauts de l'original) ;
//! `+0x50` désigne le propriétaire (mot sur 16 bits, ou long pour quelques types).



use super::cx::{handlers, Cx};
use super::shell::*;
use crate::mem::{rd32, wr32};
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Courant;
use crate::machine::Machine;

/// Sons (wrappers de la zone 0xe0836-0xee080) et autres routines d'effets.
mod sfx {
    pub const E4752: u32 = 0xe4752;
    pub const E4E5A: u32 = 0xe4e5a;
    pub const E4FB6: u32 = 0xe4fb6;
    pub const E5020: u32 = 0xe5020;
    pub const E7376: u32 = 0xe7376;
    pub const ED686: u32 = 0xed686;
}


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
/// L'objet est marqué à libérer à la prochaine image.
fn kill(x: &mut Cx) {
    x.set32(0x4, DEAD);
}

/// Valeur du mot de poids faible d'un registre d'adresse, à ranger dans un champ (`move.w A0w,(off,A1)`).
fn low16(o: Obj) -> u16 {
    o.0 as u16
}

fn reinsert_foreground(x: &mut Cx) {
    x.reinsert_foreground();
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
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
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
                crate::sound::jouer(x.b, sfx::E4752, x.o);
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
        if !x.move_xz().gt() {
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
        x.cull_offscreen();
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

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x58, 4);
                let anim = (x.u8(0x43) != 0) as u32;
                x.anim_start(ANIMS, anim as u16);
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
        x.move_by_table_xz();
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
                    land(x, |x| { x.move_by_table_xz(); }, true)
                } else if (mode as i8) < 0 {
                    land(x, |x| { x.move_xz(); }, true)
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
            x.anim_start(ANIMS, 1);
            return;
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
        let quotient = crate::aides::divide(x.u32(0xc), far).0 << 7;
        let slope = (quotient as u16).wrapping_add(((far >> 16) as u16) << 2);
        x.set16(0x18, slope.wrapping_neg());
        let angle = crate::aides::direction_to(x.b, x.o, target) & 0x1f;
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
            let vers = (target.u16(x.b, 0x8), 0);
            let de = (x.u16(0x8), x.u16(0xc));
            let angle = crate::aides::direction(x.b, de, vers) & 0x1f;
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
    fn land(x: &mut Cx, mover: fn(&mut Cx), clear_state: bool) {
        mover(x);
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
        crate::sound::jouer(x.b, sfx::ED686, x.o);
    }

    /// Chute du mode > 0 : déplacement en X, Y et Z.
    fn land_xyz(x: &mut Cx) {
        x.move_xyz();
        x.anim_step();
        if (x.u16(0xc) as i16) > 0 {
            return;
        }
        x.add8(0x4, 2);
        x.set8(0x2a, 0);
        x.set32(0xc, 0);
        x.anim_start(ANIMS, 2);
        crate::sound::jouer(x.b, sfx::ED686, x.o);
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
            crate::sound::jouer(x.b, sfx::E7376, x.o);
            return x.free_eshl();
        }
        if hit & 0x20 != 0 {
            x.add8(0x4, 2);
            return x.free_eshl();
        }
        if blocked_by_terrain(x) {
            return x.free_eshl();
        }
        x.move_by_table_xy();
        x.anim_step();
        x.cull_offscreen();
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
                    x.move_xz();
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
        let de = (x.u16(0x8), x.u16(0xc));
        let vers = (target.u16(x.b, 0x8), 0);
        let angle = crate::aides::direction(x.b, de, vers) & 0x1f;
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
        x.anim_start(ANIMS, anim as u16);
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
        crate::sound::jouer(x.b, sfx::E5020, x.o);
        x.anim_start(ANIMS, 3);
    }

    fn idle(x: &mut Cx) {
        match (x.u8(0x6), x.u8(0x3)) {
            (0, 0) => {
                x.set8(0x1d, 0);
                x.add8(0x6, 2);
                x.c.d[0] = 0;
                let alea = x.random();
                let anim = x.rom8(IDLE_ANIMS + (alea as u32 & 7)) as u32;
                x.anim_start(ANIMS, anim as u16);
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
                    crate::sound::jouer(x.b, sfx::E4E5A, x.o);
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
                    let alea = x.random();
                    let delay = x.rom16(DELAYS + 2 * (alea as u32 & 7));
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
                crate::sound::jouer(x.b, sfx::E4E5A, x.o);
                crate::sound::jouer(x.b, sfx::E4FB6, x.o);
                x.add8(0x6, 2);
                // Glitch d'origine : l'échec de l'allocation n'est pas testé (voir `Pool::alloc_unchecked`).
                let blow = x.alloc_unchecked(&pools::ESHL);
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
                    crate::sound::jouer(x.b, sfx::E5020, x.o);
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
                crate::sound::jouer(x.b, sfx::E5020, x.o);
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
        crate::sound::jouer(x.b, sfx::E4FB6, x.o);
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
                        if let Some(e) = x.alloc(&pools::EFFCT) {
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
                    x.move_xz();
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
        x.anim_start(ANIMS, (variant.wrapping_add(10) as u32) as u16);
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
        x.move_by_table_xy();
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
        if let Some(e) = x.alloc(&pools::ESHL) {
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
        let alea = x.random();
        let mut table = GROUP_OFFSETS + ((alea as u32 & 7) << 3);
        loop {
            let Some(e) = x.alloc(&pools::ENEMY) else { return };
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
            let alea = x.random();
            let mut dx = alea & 0x7f;
            if variant & 1 != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(0x8, dx);
            let alea = x.random();
            let r = alea & 0x3f;
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
        x.move_z();
        if (x.u16(0xc) as i16) >= 0 {
            return;
        }
        x.add8(0x4, 2);
        x.set32(0xc, 0);
        x.c.d[0] = 3;
        x.call(IMPACT_SOUND);
        crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 8);
        crate::sound::jouer(x.b, IMPACT_EFFECT, x.o);
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
        let alea = x.random();
        let mut table = x.rom16(SPEED_TABLES + 2 * (alea as u32 & 3)) as u32;
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
        x.move_xyz();
        if (x.u16(0xc) as i16) >= 0 {
            return;
        }
        x.add8(0x5, 2);
        x.set32(0xc, 0);
        x.set16(0x58, 0x80);
        x.anim_start(ANIMS, 1);
        crate::sound::jouer(x.b, LAND_SOUND, x.o);
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
        x.anim_start(ANIMS, anim as u16);
        x.reinsert();
    }

    fn active(x: &mut Cx) {
        if x.u8(0x3) == 0 {
            x.add16(0x58, 0xffff);
            if x.u16(0x58) == 0 {
                x.add8(0x4, 2);
                x.set8(0x0, 2);
                crate::sound::jouer(x.b, SOUND_END, x.o);
            }
            x.anim_step();
            if x.u16(0x58) == ROW_AT {
                start_row(x);
                crate::sound::jouer(x.b, SOUND_ROW, x.o);
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
    fn start_row(x: &mut Cx) {
        let px = x.u16(0x8);
        let limit = if x.u8(0x56) == 0 {
            let target = x.linked(f::OWNER);
            target.u16(x.b, 0x8)
        } else {
            camera_x(x).wrapping_add(0xc0)
        };
        let rightwards = px <= limit;
        let start = if rightwards { px.wrapping_add(0x20) } else { px.wrapping_sub(0x20) };
        spawn_row(x, 0x1b, start, rightwards);
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
                x.anim_start(ANIMS, (variant as u32) as u16);
                x.reinsert();
            }
            2 => {
                x.move_by_table_xy();
                if x.a5_u8(0x6f).wrapping_add(x.u8(0x3)) & 3 == 0 {
                    x.cull_offscreen();
                }
                x.reinsert();
            }
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Aides des types 29 à 39
// ---------------------------------------------------------------------------------------------

/// Démarrage d'animation de l'IA (`0x103d6` : `anim_start_indexed` puis `+0x44 = +0x3e`).
fn ai_start(x: &mut Cx, set: u32, n: u32) {
    crate::families::enemy_lib::start(x, set, n);
}

/// Pas d'animation de l'IA (`0x1040c`) ; vrai en fin de script.
fn ai_step(x: &mut Cx) -> bool {
    crate::families::enemy_lib::step(x)
}

/// Abscisse gauche de la caméra courante (`A5+0x506`, ou `0x556` si la caméra alternative est active).
fn camera_x(x: &mut Cx) -> u16 {
    if x.a5_u8(0x36) != 0 { x.a5_u16(0x556) } else { x.a5_u16(0x506) }
}

/// Recopie la position (X, Z, Y : parties entières) d'un objet.
fn copy_position16(x: &mut Cx, from: Obj) {
    for off in [0x8, 0xc, 0x10] {
        let v = from.u16(x.b, off);
        x.set16(off, v);
    }
}

/// Crée une rangée de tirs de type `kind` (variante 1) en partant de `px`, de 0x38 en 0x38 vers la
/// droite ou vers la gauche jusqu'aux limites de l'écran (`+0x58` numérote les éléments).
fn spawn_row(x: &mut Cx, kind: u8, mut px: u16, rightwards: bool) {
    let mut label: u16 = 1;
    loop {
        let Some(e) = x.alloc(&pools::ESHL) else { return };
        e.set_u8(x.b, 0, 1);
        e.set_u8(x.b, 2, kind);
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

/// Clignotement selon la parité de `A5+0x6e` et de la variante : caché quand elles diffèrent.
fn blink_by_parity(x: &mut Cx) {
    let hidden = (x.u8(0x3) ^ x.a5_u16(0x6e) as u8) & 1 != 0;
    x.set_bit(0x1, 1, hidden);
}

/// Gabarit des types 32, 33, 35, 36 et 38 : pause (objet suspendu), état, puis, si l'objet existe
/// encore, remise en place dans la liste.
fn guarded(x: &mut Cx, state: fn(&mut Cx)) {
    if suspended(x) {
        return;
    }
    state(x);
    if x.u8(0x0) != 0 {
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 29 (0xa1d70) : flamme / onde d'une rangée créée par un créateur (h)
// ---------------------------------------------------------------------------------------------

/// Trois variantes : 0 = élément central posé sur le créateur (`+0x5c`), qui attend que le créateur
/// arme son attaque (`+0xaf`) puis crée une rangée d'éléments de variante 1 (de part et d'autre,
/// `spawn_row`) ; 1 = élément de la rangée, clignotant, qui s'éteint avec l'épicentre ; 2 = la
/// version à retardement (30 images). Tous disparaissent avec le créateur (état >= 4 ou 0x20a).
mod shot29 {
    use super::*;

    const ANIMS: u32 = 0x16eec2;
    const ATTACH_SHIFT: u32 = 0xa1fac;
    const SOUND_START: u32 = 0xed40a;
    const SOUND_END: u32 = 0xed90a;
    const SOURCE_ARMED: u32 = 0xaf;
    const WAIT: u8 = 0;
    const HOLD: u8 = 2;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            4 => stop(x),
            _ => x.free_eshl(),
        }
    }

    fn init(x: &mut Cx) {
        x.set8(f::ATTACK_ID, 6);
        x.set8(f::ATTACK_POWER, 5);
        x.set8(f::BOX_SET, 7);
        match x.u8(0x3) {
            2 => {
                x.add8(0x4, 2);
                follow(x);
                x.set16(0x58, 0x1e);
                x.anim_start(ANIMS, 3);
            }
            0 => {
                x.add8(0x4, 2);
                follow(x);
                x.anim_start(ANIMS, 0);
            }
            _ => {
                x.add16(0x58, 0xffff);
                if x.u16(0x58) != 0 {
                    return;
                }
                x.add8(0x4, 2);
                x.anim_start(ANIMS, 1);
            }
        }
        x.reinsert();
    }

    /// Fin : le créateur est mort ou dans l'état 0x20a.
    fn stop(x: &mut Cx) {
        crate::sound::jouer(x.b, SOUND_END, x.o);
        x.add8(0x4, 2);
    }

    fn active(x: &mut Cx) {
        let source = x.linked(0x5c);
        if (source.u8(x.b, 0x4) as i8) >= 4 || source.u16(x.b, 0x4) == 0x20a {
            return stop(x);
        }
        match x.u8(0x3) {
            2 => {
                follow(x);
                x.add16(0x58, 0xffff);
                if (x.u16(0x58) as i16) < 0 {
                    x.add8(0x4, 2);
                }
                x.anim_step();
            }
            0 => centre(x),
            _ => satellite(x, source),
        }
    }

    /// Élément central : attend le signal du créateur puis déclenche la rangée.
    fn centre(x: &mut Cx) {
        match x.u8(0x5) {
            WAIT => {
                let source = x.linked(0x5c);
                if source.u8(x.b, SOURCE_ARMED) != 0 {
                    crate::sound::jouer(x.b, SOUND_START, x.o);
                    x.add8(0x5, 2);
                    x.set16(0x58, 0x50);
                    let px = source.u16(x.b, 0x8);
                    let rightwards = source.u8(x.b, f::FLIP) != 0;
                    let start = if rightwards { px.wrapping_add(0x4d) } else { px.wrapping_sub(0x4d) };
                    spawn_row(x, 0x1d, start, rightwards);
                } else {
                    follow(x);
                    x.anim_step();
                    x.reinsert();
                }
            }
            HOLD => {
                x.add16(0x58, 0xffff);
                if x.u16(0x58) == 0 {
                    x.add8(0x4, 2);
                    x.set8(0x5, 0);
                    x.set8(0x0, 2);
                    let source = x.linked(0x5c);
                    source.set_u8(x.b, SOURCE_ARMED, 0);
                }
                follow(x);
                x.anim_step();
                x.reinsert();
            }
            _ => {}
        }
    }

    /// Élément de la rangée : clignote selon `A5+0x6f`, change d'animation quand l'épicentre s'éteint.
    fn satellite(x: &mut Cx, source: Obj) {
        let flicker = |x: &mut Cx| {
            let hidden = x.a5_u8(0x6f) & 1 == 0;
            x.set_bit(0x1, 1, hidden);
        };
        match x.u8(0x5) {
            WAIT => {
                x.anim_step();
                if source.u16(x.b, 0x58) <= 0x30 {
                    x.add8(0x5, 2);
                    x.anim_start(ANIMS, 2);
                }
                flicker(x);
            }
            HOLD => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
                flicker(x);
            }
            _ => {}
        }
    }

    /// Se colle au créateur ; le décalage dépend de son état d'attaque (`+7`).
    fn follow(x: &mut Cx) {
        let source = x.linked(0x5c);
        copy_position16(x, source);
        let flip = source.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        let attack = source.u8(x.b, 0x7);
        if attack == 0x16 || attack == 0x18 {
            let index = source.u8(x.b, 0x33) as u32;
            let mut dx = x.rom16(ATTACH_SHIFT + index);
            if flip != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(0x8, dx);
            x.set16(0xc, 0x1e);
        } else {
            let mut dx = -0x2di16 as u16;
            if flip != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(0x8, dx);
            x.set16(0xc, 0x27);
        }
        x.add16(0x10, 0xffff);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 30 (0xa2060) : coup porté par un ennemi, avec son (h)
// ---------------------------------------------------------------------------------------------

/// Colle un coup au propriétaire (orientation inversée) : attend que le propriétaire arme son attaque
/// (`+0xaf`), joue le coup (animation 1, 48 images), le retour (animation 2), puis rend la main.
mod shot30 {
    use super::*;

    const ANIMS: u32 = 0x16f174;
    const SOUND_START: u32 = 0xed474;
    const SOUND_END: u32 = 0xed90a;
    const OWNER_ARMED: u32 = 0xaf;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            4 => {
                crate::sound::jouer(x.b, SOUND_END, x.o);
                x.free_eshl()
            }
            _ => x.free_eshl(),
        }
    }

    fn init(x: &mut Cx) {
        if x.u16(f::OWNER) == 0 {
            return;
        }
        x.add8(0x4, 2);
        let owner = x.linked(f::OWNER);
        copy_position16(x, owner);
        let flip = owner.u8(x.b, f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set8(f::ATTACK_ID, 5);
        x.set8(f::ATTACK_POWER, 0xc);
        x.set8(f::BOX_SET, 7);
        x.add16(0xc, 0x27);
        let dx = if flip != 0 { -0x2di16 } else { 0x2d };
        x.add16(0x8, dx as u16);
        x.anim_start(ANIMS, 0);
    }

    fn active(x: &mut Cx) {
        if x.u16(f::OWNER) == 0 {
            return;
        }
        let owner = x.linked(f::OWNER);
        if (owner.u8(x.b, 0x4) as i8) >= 4 || owner.u16(x.b, 0x4) == 0x20a {
            crate::sound::jouer(x.b, SOUND_END, x.o);
            return x.free_eshl();
        }
        match x.u8(0x5) {
            0 => {
                if owner.u8(x.b, OWNER_ARMED) != 0 {
                    x.add8(0x5, 2);
                }
                x.anim_step();
                x.reinsert();
            }
            2 => {
                x.add8(0x5, 2);
                crate::sound::jouer(x.b, SOUND_START, x.o);
                x.set16(0x58, 0x30);
                x.anim_start(ANIMS, 1);
            }
            4 => {
                x.add16(0x58, 0xffff);
                if (x.u16(0x58) as i16) < 0 {
                    x.add8(0x5, 2);
                    x.anim_start(ANIMS, 2);
                } else {
                    x.anim_step();
                    x.reinsert();
                }
            }
            6 => {
                if x.anim_step() {
                    crate::sound::jouer(x.b, SOUND_END, x.o);
                    x.add8(0x4, 4);
                    x.set8(0x5, 0);
                    let owner = x.linked(f::OWNER);
                    owner.set_u8(x.b, OWNER_ARMED, 0);
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 31 (0xa21b6) : boule à tête chercheuse d'un créateur (h)
// ---------------------------------------------------------------------------------------------

/// Boule qui part du créateur (`+0x5c`, côté regardé) avec la table de vitesses 0x30. Elle attend
/// 48 images (`+0x42`), puis toutes les 10 images oriente sa direction (`+0x2d`) vers lui (angle +
/// `+0x58`, signe selon la hauteur de sa cible) et explose au contact (boîte de 0x20 autour du
/// créateur). Quand le créateur passe à l'état >= 0x406, elle file droit et sort de l'écran.
mod shot31 {
    use super::*;

    const ANIMS: u32 = 0x16f504;
    const SOUND_SPAWN: u32 = 0xedaa6;
    const SOUND_HIT: u32 = 0xedb18;
    const SEARCH: u8 = 0;
    const HUNT: u8 = 2;

    pub fn run(x: &mut Cx) {
        if x.u16(0x5c) == 0 {
            return kill(x);
        }
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            4 => fly_away(x),
            _ => x.free_eshl(),
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 7);
        x.set8(f::ATTACK_POWER, 5);
        let source = x.linked(0x5c);
        copy_position16(x, source);
        let flip = source.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.add16(0x10, 0xffff);
        x.set8(0x42, 0x30);
        let (dx, angle) = if flip != 0 { (0x4a, 8) } else { (-0x4ai16 as u16, 0x18) };
        x.set8(0x2d, angle);
        x.add16(0x8, dx);
        x.add16(0xc, 0x32);
        x.anim_start(ANIMS, 0);
        speed_table(x, 0x30);
    }

    fn fly_away(x: &mut Cx) {
        x.move_by_table_xy();
        x.anim_step();
        x.cull_offscreen();
    }

    /// Son de rebond quand l'animation lève son drapeau (`+0x33`).
    fn animation_cue(x: &mut Cx) {
        if x.u8(0x33) != 0 {
            x.set8(0x33, 0);
            crate::sound::jouer(x.b, SOUND_SPAWN, x.o);
        }
    }

    fn active(x: &mut Cx) {
        let source = x.linked(0x5c);
        if (source.u16(x.b, 0x4) as i16) >= 0x406 {
            return fly_away(x);
        }
        match x.u8(0x5) {
            SEARCH => {
                animation_cue(x);
                x.add8(0x42, 0xff);
                if (x.u8(0x42) as i8) < 0 {
                    x.add8(0x5, 2);
                    x.set8(0x58, 0xfc);
                    if source.u16(x.b, f::OWNER) != 0 {
                        let target = link_of(x, source, f::OWNER);
                        let d = x.u16(0x10).wrapping_sub(target.u16(x.b, 0x10));
                        if (d as i16) < 0 {
                            x.set8(0x58, 4);
                        }
                    }
                }
                x.anim_step();
                x.move_by_table_xy();
            }
            HUNT => {
                animation_cue(x);
                x.add8(0x42, 0xff);
                if (x.u8(0x42) as i8) < 0 {
                    x.set8(0x42, 0xa);
                    let vers = (source.u16(x.b, 0x8), source.u16(x.b, 0x10));
                    let angle = crate::aides::direction_from(x.b, x.o, vers).wrapping_add(x.u8(0x58)) & 0x1f;
                    x.set8(0x2d, angle);
                }
                let touching = |x: &mut Cx, a: u16, b: u16| a.wrapping_sub(b).wrapping_add(8) <= 0x20 && x.a5_u8(GAME_PAUSED) == 0;
                let (sx, sy) = (source.u16(x.b, 0x8), source.u16(x.b, 0x10));
                let (mx, my) = (x.u16(0x8), x.u16(0x10));
                let hit = source.u8(x.b, 0x4) != 4 && touching(x, sx, mx) && touching(x, sy, my);
                if hit {
                    crate::sound::jouer(x.b, SOUND_HIT, x.o);
                    x.set8(0x4, st::FREE);
                    source.set_u8(x.b, 0xb5, 0);
                } else {
                    x.move_by_table_xy();
                    x.anim_step();
                }
            }
            _ => {}
        }
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Types 32 et 33 : tirs d'un hôte (+0x42), animations de l'IA (0x103d6 / 0x1040c)
// ---------------------------------------------------------------------------------------------

/// Vrai quand l'hôte (`+0x42`) de l'objet est dans l'état 4 : le tir doit disparaître (0xa2510).
fn host_dead(x: &mut Cx) -> bool {
    let host = x.linked(0x42);
    host.u8(x.b, 0x4) == 4
}

/// Type 32 (0xa239a) : projectile à tête chercheuse tiré sur la position de l'hôte (h)
///
/// Au départ (état 0) il vise le propriétaire (`+0x50`, variante 0) ou la position (`+0xbc/+0xbe`)
/// de l'hôte (`+0x42`, autres variantes) : direction `+0x2d` d'après l'angle, vitesse verticale
/// calculée pour atteindre la cible en un nombre de pas proportionnel à la distance. Puis il vole
/// (état 2 sous-état 0 : attend le drapeau `+0x58`), retombe (sous-état 2) en jouant un son si
/// sa chute croise la zone visible, et se termine par son animation (sous-état 4).
mod shot32 {
    use super::*;

    const ANIMS: u32 = 0x16f58c;
    const SOUND_FLYING: u32 = 0xed4de;
    const SOUND_LANDING: u32 = 0xed548;
    const SCREEN_LEFT: u32 = 0x5a6;
    const FLY: u8 = 0;
    const FALL: u8 = 2;
    const END: u8 = 4;

    pub fn run(x: &mut Cx) {
        guarded(x, state)
    }

    fn state(x: &mut Cx) {
        match x.u8(0x4) {
            0 => init(x),
            2 => match x.u8(0x5) {
                FLY => {
                    if host_dead(x) {
                        return x.free_eshl();
                    }
                    if x.u8(0x58) != 0 {
                        x.add8(0x5, 2);
                        crate::sound::jouer(x.b, SOUND_FLYING, x.o);
                    }
                    ai_step(x);
                }
                FALL => {
                    if host_dead(x) {
                        return x.free_eshl();
                    }
                    x.move_xyz();
                    if (x.u16(0xc) as i16) > 0 {
                        ai_step(x);
                        return;
                    }
                    x.add8(0x5, 2);
                    x.set16(0xc, 0);
                    x.set8(0x5a, 1);
                    let left = x.a5_u16(SCREEN_LEFT);
                    let x_pos = x.u16(0x8);
                    let inside = (left.wrapping_sub(0x30) as i16) < x_pos as i16 && (left.wrapping_add(0x1b0) as i16) > x_pos as i16;
                    if inside {
                        crate::sound::jouer(x.b, SOUND_LANDING, x.o);
                    }
                    ai_start(x, ANIMS, 2);
                }
                END => {
                    if ai_step(x) {
                        x.add8(0x4, 2);
                    }
                }
                _ => {}
            },
            _ => x.free_eshl(),
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 0x14);
        crate::placement::load_effect_palette(x.b, crate::placement::effect_palette::LIGHT);
        let variant = x.u8(0x3);
        let (target, angle);
        if variant == 0 {
            target = x.linked(f::OWNER);
            angle = crate::aides::direction_to(x.b, x.o, target);
        } else {
            target = x.linked(0x42);
            let vers = (target.u16(x.b, 0xbc), target.u16(x.b, 0xbe));
            angle = crate::aides::direction_from(x.b, x.o, vers);
        }
        x.set8(0x2d, angle);
        speed_table(x, 0x41);
        // distance horizontale à la cible
        let reference = if variant == 0 { target.u32(x.b, 0x8) } else { target.u32(x.b, 0xbc) };
        let mut distance = x.u32(0x8).wrapping_sub(reference);
        if (distance as i32) < 0 {
            distance = distance.wrapping_neg();
        }
        distance &= 0x7fff_0000;
        // nombre de pas de 9 pixels pour la couvrir
        let step = 0x0009_0000u32;
        let mut steps: u32 = 0;
        let mut rest = distance as i32;
        loop {
            rest = rest.wrapping_sub(step as i32);
            if rest <= 0 {
                break;
            }
            steps = (steps + 1) & 0xffff;
        }
        let quotient = crate::aides::divide(x.u16(0xc) as u32, steps).0;
        let slope = (quotient as i32).wrapping_neg() as u32;
        let slope = (slope.rotate_left(16) as i32 >> 8) as u16;
        x.set16(0x18, slope);
        ai_start(x, ANIMS, 0);
    }
}

/// Type 33 (0xa2524) : tir en trois temps lié à son hôte (h)
///
/// Même gabarit que le type 32 (suspension, hôte mort = l'état 4 de l'hôte). Animation 0 puis, quand
/// l'animation lève `+0x58`, son et animation 1 ; quand elle lève `+0x59`, animation 2 jusqu'à
/// ce que `+0x32` se vide (`+0x5a` = 1), puis fin à la fin du script.
mod shot33 {
    use super::*;

    const ANIMS: u32 = 0x16f8ea;
    const SOUND_FIRE: u32 = 0xed474;
    const SOUND_END: u32 = 0xed90a;

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        if host_dead(x) {
            crate::sound::jouer(x.b, SOUND_END, x.o);
            return x.free_eshl();
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 0xf);
                ai_start(x, ANIMS, 0);
            }
            2 => match x.u8(0x5) {
                0 => {
                    if x.u8(0x58) == 0 {
                        ai_step(x);
                    } else {
                        x.add8(0x5, 2);
                        crate::sound::jouer(x.b, SOUND_FIRE, x.o);
                        ai_start(x, ANIMS, 1);
                    }
                }
                2 => {
                    if x.u8(0x59) == 0 {
                        ai_step(x);
                    } else {
                        x.add8(0x5, 2);
                        x.set8(0x6, 0);
                        ai_start(x, ANIMS, 2);
                    }
                }
                4 => match x.u8(0x6) {
                    0 => {
                        ai_step(x);
                        if x.u16(0x32) != 0 {
                            x.add8(0x6, 2);
                            x.set8(0x5a, 1);
                        }
                    }
                    2 => {
                        if ai_step(x) {
                            x.add8(0x4, 2);
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
            _ => {
                crate::sound::jouer(x.b, SOUND_END, x.o);
                return x.free_eshl();
            }
        }
        if x.u8(0x0) != 0 {
            x.reinsert();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 34 (0xa2630) : mise en scène d'une attaque de boss, trois variantes (h)
// ---------------------------------------------------------------------------------------------

/// Objet de mise en scène lié à son créateur (`+0x5c`). Variante 0 : voile plein écran (position de
/// la caméra, 600 images) avec un décor SET (type 0 variante 0x47) rattaché dans les deux sens ;
/// variante 1 : même idée à côté du créateur (400 images, un seul décor, animation de sortie) ;
/// variante 2 : posée sur le créateur. Au premier passage la variante 0 charge quatre palettes
/// (0x91c2e0...) de la ROM, teintées par 0xf000f000.
mod shot34 {
    use super::*;

    const ANIMS_SCREEN: u32 = 0x1c03a6;
    const ANIMS: u32 = 0x17e03a;
    /// (destination dans la palette, source dans la ROM), 8 longs chacun.
    const PALETTES: [(u32, u32); 4] = [(0x91c2e0, 0x2538c0), (0x91c300, 0x2538e0), (0x91c360, 0x253900), (0x91c3e0, 0x253920)];
    const TINT: u32 = 0xf000_f000;
    const SOURCE_BUSY: u32 = 0xa0;

    pub fn run(x: &mut Cx) {
        if x.u16(0x5c) == 0 {
            return kill(x);
        }
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            _ => x.free_eshl(),
        }
    }

    fn load_palettes(x: &mut Cx) {
        for (dst, src) in PALETTES {
            for i in 0..8 {
                let v = x.data32(src + 4 * i);
                wr32(x.b, dst + 4 * i, v);
            }
            for i in 0..8 {
                let v = rd32(x.b, dst + 4 * i) | TINT;
                wr32(x.b, dst + 4 * i, v);
            }
        }
    }

    /// Décor SET (type 0, variante 0x47) ; `None` si le pool est plein.
    fn spawn_scenery(x: &mut Cx) -> Option<Obj> {
        let s = x.alloc(&pools::SET)?;
        s.set_u8(x.b, 0, 1);
        s.set_u8(x.b, 2, 0);
        s.set_u8(x.b, 3, 0x47);
        Some(s)
    }

    /// Se place sur le créateur (position, orientation, un pixel plus près).
    fn place_on_source(x: &mut Cx) -> Obj {
        let source = x.linked(0x5c);
        copy_position16(x, source);
        let flip = source.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.add16(0x10, 0xffff);
        source
    }

    fn init(x: &mut Cx) {
        let variant = x.u8(0x3);
        if variant == 0 && x.u8(0x5) == 0 {
            x.add8(0x5, 2);
            return load_palettes(x);
        }
        if x.a5_u8(GAME_PAUSED) != 0 {
            return;
        }
        x.add8(0x4, 2);
        x.set8(0x5, 0);
        if variant == 2 {
            place_on_source(x);
            x.add16(0x8, 0x9c);
            x.add16(0x10, -0x62i16 as u16);
            x.anim_start(ANIMS, 8);
            return;
        }
        if let Some(s) = spawn_scenery(x) {
            s.set_u8(x.b, 0x2f, 0);
            x.set16(f::OWNER, low16(s));
        }
        let source = place_on_source(x);
        if variant == 0 {
            if let Some(s) = spawn_scenery(x) {
                x.set8(0x2f, 0);
                s.set_u16(x.b, f::OWNER, low16(x.o));
            }
            let camera_x = x.a5_u16(0x506);
            x.set16(0x8, camera_x);
            let camera_y = x.a5_u16(0x50a);
            x.set16(0xc, camera_y.wrapping_add(0xda));
            x.set16(0x10, 0);
            x.set16(0x42, 0x258);
            x.c.d[0] = x.c.d[0] & 0xffff_0000 | 0xf0;
            let anim = x.c.d[0];
            x.anim_start(ANIMS_SCREEN, anim as u16);
        } else {
            let dx = if source.u8(x.b, f::FLIP) != 0 { 8 } else { -8i16 as u16 };
            x.add16(0x8, dx);
            x.add16(0xc, 0x20);
            x.set16(0x42, 0x190);
            x.anim_start(ANIMS, 2);
        }
    }

    fn active(x: &mut Cx) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return;
        }
        let source = x.linked(0x5c);
        if (source.u8(x.b, 0x4) as i8) >= 4 {
            return x.free_eshl();
        }
        match x.u8(0x3) {
            0 => {
                x.add16(0x42, 0xffff);
                if (x.u16(0x42) as i16) < 0 {
                    let scenery = x.linked(f::OWNER);
                    scenery.set_u8(x.b, 0x4, 4);
                    kill(x);
                    source.set_u8(x.b, SOURCE_BUSY, 0);
                }
                x.anim_step();
            }
            1 => match x.u8(0x5) {
                0 => {
                    x.add16(0x42, 0xffff);
                    if (x.u16(0x42) as i16) < 0 {
                        x.add8(0x5, 2);
                        source.set_u8(x.b, SOURCE_BUSY, 0);
                        x.anim_start(ANIMS, 7);
                    } else {
                        x.anim_step();
                    }
                }
                2 => {
                    if x.anim_step() {
                        let scenery = x.linked(f::OWNER);
                        scenery.set_u8(x.b, 0x4, 4);
                        kill(x);
                    }
                }
                _ => {}
            },
            _ => {
                x.anim_step();
            }
        }
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 35 (0xa28d2) : éclat tombant avec ombre, 16 variantes (h)
// ---------------------------------------------------------------------------------------------

/// Éclat décoratif : table de vitesses 0xb avec direction `+0x2d` selon la variante (0, 2, ... 30),
/// vole 90 images (`+0x58`), puis anime sa fin. Son ombre est tracée par 0x33fe4 ; il clignote
/// selon la parité de `A5+0x6e` et de la variante.
mod shot35 {
    use super::*;

    const ANIMS: u32 = 0x16fafa;
    const DIRECTIONS: u32 = 0xa2934;
    const FLY: u8 = 0;
    const FADE: u8 = 2;

    pub fn run(x: &mut Cx) {
        guarded(x, state)
    }

    fn state(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 0xc);
                x.set16(0x58, 0x5a);
                let variant = x.u8(0x3) as u32;
                let direction = x.rom8(DIRECTIONS + variant);
                x.set8(0x2d, direction);
                speed_table(x, 0xb);
                ai_start(x, ANIMS, 0);
            }
            2 => {
                match x.u8(0x5) {
                    FLY => {
                        x.add16(0x58, 0xffff);
                        if (x.u16(0x58) as i16) <= 0 {
                            x.add8(0x5, 2);
                            ai_start(x, ANIMS, 1);
                        } else {
                            ai_step(x);
                            if (x.u16(0x58) as i16) > 0x38 {
                                x.move_by_table_xy();
                            }
                        }
                    }
                    FADE => {
                        if ai_step(x) {
                            x.add8(0x4, 2);
                        } else {
                            x.move_by_table_xy();
                        }
                    }
                    _ => {}
                }
                blink_by_parity(x);
                x.call(crate::terrain::SHADOW.entry);
            }
            _ => x.free_eshl(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 36 (0xa29cc) : débris lancé en l'air, retombe sur la scène (h)
// ---------------------------------------------------------------------------------------------

/// Apparaît à une position aléatoire de l'écran (tables 0xa2b0a / 0xa2b2a) en hauteur 0x100 - Y,
/// avec une vitesse verticale tirée au hasard ; retombe (Z <= 0), joue un son d'impact puis anime
/// sa fin.
mod shot36 {
    use super::*;

    const ANIMS: u32 = 0x16fbf4;
    const SPEEDS: u32 = 0xa2aa4;
    const X_OFFSETS: u32 = 0xa2b0a;
    const Y_OFFSETS: u32 = 0xa2b2a;
    const SOUND_LAND: u32 = 0xed548;

    pub fn run(x: &mut Cx) {
        guarded(x, state)
    }

    fn state(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 0x1e);
                launch(x);
                place(x);
                ai_start(x, ANIMS, 0);
            }
            2 => match x.u8(0x5) {
                0 => {
                    x.move_xyz();
                    if (x.u16(0xc) as i16) > 0 {
                        ai_step(x);
                        return;
                    }
                    x.add8(0x5, 2);
                    x.set16(0xc, 0);
                    crate::sound::jouer(x.b, SOUND_LAND, x.o);
                    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 0x20);
                    ai_start(x, ANIMS, 1);
                }
                2 => {
                    if ai_step(x) {
                        x.add8(0x4, 2);
                    }
                }
                _ => {}
            },
            _ => x.free_eshl(),
        }
    }

    /// Direction selon l'orientation, table de vitesses 0x17 et vitesse verticale tirée au hasard.
    fn launch(x: &mut Cx) {
        let direction = if x.u8(f::FLIP) != 0 { 0x18 } else { 8 };
        x.set8(0x2d, direction);
        speed_table(x, 0x17);
        let alea = x.random();
        let r = alea as u32 & 0xf;
        let vz = x.rom16(SPEEDS + 2 * r);
        x.set16(0x18, vz);
    }

    /// Position aléatoire autour de la caméra (écran de gauche : décalage de -0xc0).
    fn place(x: &mut Cx) {
        let alea = x.random();
        let r = alea as u32 & 0xf;
        let mut dx = x.rom16(X_OFFSETS + 2 * r);
        let alea = x.random();
        let r = alea as u32 & 0xf;
        let dy = x.rom16(Y_OFFSETS + 2 * r);
        if x.u8(f::FLIP) == 0 {
            dx = dx.wrapping_sub(0xc0);
        }
        let z = 0x100u16.wrapping_sub(dy);
        let px = dx.wrapping_add(x.a5_u16(0x5a6));
        let py = dy.wrapping_add(x.a5_u16(0x5aa));
        x.set16(0x8, px);
        x.set16(0x10, py);
        x.set16(0xc, z);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 37 (0xa2b62) : météorite lancée par un créateur, retombe et explose (h)
// ---------------------------------------------------------------------------------------------

/// Tombe du ciel à une position aléatoire de l'écran (tables 0xa2ce0 / 0xa2d00), vitesse verticale
/// aléatoire (0xa2c7e) et animation de départ aléatoire (0xa2c9e). Au sol (Z <= 0) elle déclenche
/// l'effet 0xb10c8 / 0xb4bcc, clignote 25 images et disparaît ; elle disparaît avec le créateur.
mod shot37 {
    use super::*;

    const ANIMS: u32 = 0x16fe3e;
    const SPEEDS: u32 = 0xa2c7e;
    const START_ANIMS: u32 = 0xa2c9e;
    const X_OFFSETS: u32 = 0xa2ce0;
    const Y_OFFSETS: u32 = 0xa2d00;
    const IMPACT: u32 = 0xb10c8;
    const IMPACT_PIECE: u32 = 0xb4bcc;
    const FALL: u8 = 0;
    const BLINK: u8 = 2;

    pub fn run(x: &mut Cx) {
        if x.u16(0x5c) == 0 {
            return kill(x);
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 5);
                x.set8(f::ATTACK_ID, 0x17);
                x.set8(0x50, 0x14);
                launch(x);
                place(x);
            }
            2 => active(x),
            _ => x.free_eshl(),
        }
    }

    fn launch(x: &mut Cx) {
        x.c.d[0] = 0;
        let alea = x.random();
        let r = alea as u32 & 0xf;
        let vz = x.rom16(SPEEDS + 2 * r);
        x.set16(0x18, vz);
        let alea = x.random();
        let r = alea as u32 & 7;
        let anim = x.rom8(START_ANIMS + r) as u32;
        x.anim_start(ANIMS, anim as u16);
    }

    fn place(x: &mut Cx) {
        let alea = x.random();
        let r = alea as u32 & 0xf;
        let dx = x.rom16(X_OFFSETS + 2 * r);
        let alea = x.random();
        let r = alea as u32 & 0xf;
        let dy = x.rom16(Y_OFFSETS + 2 * r);
        let px = dx.wrapping_add(x.a5_u16(0x506));
        let py = dy.wrapping_add(x.a5_u16(0x50a));
        x.set16(0x8, px);
        x.set16(0x10, py);
        x.set16(0xc, 0x100);
    }

    fn active(x: &mut Cx) {
        let source = x.linked(0x5c);
        if (source.u8(x.b, 0x4) as i8) >= 4 {
            return x.free_eshl();
        }
        match x.u8(0x5) {
            FALL => {
                x.move_z();
                if (x.u16(0xc) as i16) > 0 {
                    x.anim_step();
                } else {
                    x.add8(0x5, 2);
                    x.set16(0xc, 0);
                    x.set8(0x42, 0x19);
                    x.c.d[0] = 0;
                    x.c.d[1] = x.c.d[1] & 0xffff_0000 | 0x201;
                    x.c.d[2] &= 0xffff_0000;
                    x.c.d[3] &= 0xffff_0000;
                    x.call(IMPACT);
                    for _ in 0..2 {
                        x.c.d[2] = 1;
                        x.call(IMPACT_PIECE);
                    }
                }
            }
            BLINK => {
                x.add8(0x42, 0xff);
                if (x.u8(0x42) as i8) < 0 {
                    x.add8(0x4, 2);
                    x.set_bit(0x1, 1, true);
                    x.set8(0x0, 2);
                } else {
                    let hidden = x.u8(0x42) & 3 != 0;
                    x.set_bit(0x1, 1, hidden);
                }
            }
            _ => {}
        }
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 38 (0xa2d48) : projectile orienté vers sa cible (16 sprites), lancé par un effet (h)
// ---------------------------------------------------------------------------------------------

/// Créé avec le pointeur de son créateur dans `+0x46` (mot négatif = adresse RAM). Prend sa position
/// (+0x30 en altitude), des vitesses de la ligne `+0x2f` de la table 0xa2ed4 (ou 0xa2f32 pour les
/// variantes non nulles), et son sprite d'après l'angle vers la cible du créateur, borné à 0x14..0x16
/// (miroir si retourné). Puis il vole (table de vitesses + gravité) jusqu'au sol.
mod shot38 {
    use super::*;

    const ANIMS: u32 = 0x174776;
    const SPEED_ROWS: u32 = 0xa2ed4;
    const SPEED_ROWS_VARIANT: u32 = 0xa2f34;
    const SPRITES: u32 = 0xa2e10;

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        state(x)
    }

    /// Miroir d'un angle (0x20 - angle) pour les objets retournés.
    fn mirror(a: u16) -> u16 {
        a.wrapping_sub(0x10).wrapping_neg().wrapping_add(0x10)
    }

    fn state(x: &mut Cx) {
        match x.u8(0x4) {
            0 => init(x),
            2 => {
                fall(x);
                crate::ai::keep_on_screen_d5(x.c, x.b, x.o, 0x10);
                x.call(crate::terrain::SHADOW.entry);
                x.reinsert();
            }
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    /// Déplacement tant que Z > 0 ; au sol, passe à l'état suivant.
    fn fall(x: &mut Cx) {
        if x.u8(0x29) == 0 {
            x.move_by_table_xz();
            x.move_y();
            if (x.u16(0xc) as i16) >= 0 {
                return;
            }
            x.set16(0xc, 0);
        }
        x.add8(0x4, 2);
        x.set8(0x5, 0);
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        let creator = x.u16(0x46);
        if (creator as i16) >= 0 {
            return x.set8(0x4, st::FREE);
        }
        let host = Obj(creator as i16 as i32 as u32 & 0xff_ffff);
        copy_position16(x, host);
        let flip = host.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.add16(0xc, 0x30);
        speeds(x);
        let mut angle = host.u16(x.b, 0x50);
        if (angle as i16) < 0 {
            let target = Obj(angle as i16 as i32 as u32 & 0xff_ffff);
            let vers = (target.u16(x.b, 0x8), target.u16(x.b, 0xc).wrapping_add(0x30));
            let de = (x.u16(0x8), x.u16(0xc));
            angle = crate::aides::direction(x.b, de, vers) as u16;
        }
        let flipped = flip != 0;
        let mapped = |a: u16| if flipped { mirror(a) } else { a };
        let seen = mapped(angle) as i16;
        let chosen = if seen < 0x14 {
            mapped(0x14)
        } else if seen > 0x16 {
            mapped(0x16)
        } else {
            angle
        };
        x.set8(0x2d, chosen as u8);
        let sprite = x.rom8(SPRITES + ((mapped(chosen) >> 1) & 0xf) as u32) as u32;
        ai_start(x, ANIMS, sprite);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 8);
    }

    /// Table de vitesses et vitesses initiales de la ligne `+0x2f`.
    fn speeds(x: &mut Cx) {
        let base = if x.u8(0x3) != 0 { SPEED_ROWS_VARIANT } else { SPEED_ROWS };
        let row = base + ((x.u8(0x2f) as u32) << 4);
        let table = x.data32(row);
        x.set32(0x4c, table);
        let mut vx = x.data16(row + 4);
        let mut ax = x.data16(row + 6);
        if x.u8(f::FLIP) == 0 {
            vx = vx.wrapping_neg();
            ax = ax.wrapping_neg();
        }
        x.set16(0x14, vx);
        x.set16(0x16, ax);
        for i in 0..4 {
            let v = x.data16(row + 8 + 2 * i);
            x.set16(0x18 + 2 * i, v);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 39 (0xa2fb8) : boule de feu lancée en l'air par un créateur, 8 variantes (h)
// ---------------------------------------------------------------------------------------------

/// Part du créateur (`+0x5c`) vers le haut (VZ = -5, table de vitesses 0x13), retombe en tournant
/// (`+0x2d` varie), s'enflamme au sol (animation de fin) puis signale au créateur (`+0xae`).
/// Disparaît avec le créateur (état 0x406).
mod shot39 {
    use super::*;

    const ANIMS: u32 = 0x16ff22;
    const DIRECTIONS: u32 = 0xa3148;
    const START_ANIMS: u32 = 0xa3158;
    const FRAMES: u32 = 0xa31ba;
    const SOUND_IMPACT: u32 = 0xecb98;
    const FLY: u8 = 0;
    const SETTLE: u8 = 2;
    const BURN: u8 = 4;

    pub fn run(x: &mut Cx) {
        if x.u16(0x5c) == 0 {
            return kill(x);
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 5);
                x.set8(f::ATTACK_ID, 0x1d);
                x.set8(0x46, 4);
                launch(x);
                speed_table(x, 0x13);
            }
            2 => active(x),
            _ => x.free_eshl(),
        }
    }

    fn launch(x: &mut Cx) {
        if x.u16(0x5c) == 0 {
            return;
        }
        let source = x.linked(0x5c);
        copy_position32(x, source);
        let flip = source.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.add16(0x10, 0xffff);
        x.add16(0xc, 0x100);
        x.set16(0x18, -0x500i16 as u16);
        x.set16(0x1a, -0x38i16 as u16);
        let variant = x.u8(0x3) as u32;
        let direction = x.rom8(DIRECTIONS + variant);
        x.set8(0x2d, direction);
        let anim = x.rom8(START_ANIMS + variant) as u32;
        x.anim_start(ANIMS, anim as u16);
    }

    /// Fait tourner les 8 images de flamme toutes les 4 images.
    fn spin(x: &mut Cx) {
        x.add8(0x46, 0xff);
        if (x.u8(0x46) as i8) >= 0 {
            return;
        }
        x.set8(0x46, 4);
        x.set8(0x2a, 1);
        let n = x.u8(0x47).wrapping_add(1) & 7;
        x.set8(0x47, n);
        let frame = x.rom8(FRAMES + n as u32);
        x.set8(0x2b, frame);
    }

    fn active(x: &mut Cx) {
        spin(x);
        let source = x.linked(0x5c);
        if source.u16(x.b, 0x4) == 0x406 {
            return x.free_eshl();
        }
        match x.u8(0x5) {
            FLY => {
                x.move_by_table_xy();
                x.move_z();
                if (x.u16(0xc) as i16) <= 0 {
                    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 0x30);
                    crate::sound::jouer(x.b, SOUND_IMPACT, x.o);
                    x.add8(0x5, 2);
                    x.set16(0xc, 0);
                    x.set8(0x42, 0x30);
                }
                x.anim_step();
            }
            SETTLE => {
                x.add8(0x42, 0xff);
                if (x.u8(0x42) as i8) < 0 {
                    x.add8(0x5, 2);
                    x.set8(0x42, 0x1e);
                    let d = x.u8(0x2d).wrapping_sub(6) & 0x1f;
                    x.set8(0x2d, d);
                    x.set8(0x43, 3);
                }
                x.anim_step();
            }
            BURN => {
                x.anim_step();
                if (x.u8(0x42) as i8) < 0x14 {
                    x.set8(0x3a, 0);
                }
                x.add8(0x42, 0xff);
                if (x.u8(0x42) as i8) < 0 {
                    x.add8(0x4, 2);
                    source.set_u8(x.b, 0xae, 1);
                } else {
                    x.add8(0x43, 0xff);
                    if (x.u8(0x43) as i8) < 0 {
                        x.set8(0x43, 3);
                        let d = x.u8(0x2d).wrapping_add(1) & 0x1f;
                        x.set8(0x2d, d);
                    }
                    x.move_by_table_xy();
                }
            }
            _ => {}
        }
        blink_by_parity(x);
        x.reinsert();
    }
}
handlers! {
    0x9fe0a "eshl_15" shot15::run,
    0xa03e0 "eshl_16" shot16::run,
    0xa05ce "eshl_17" shot17::run,
    0xa0a06 "eshl_18" shot18::run,
    0xa0b20 "eshl_19" shot19::run,
    0xa0c5a "eshl_20" shot20::run,
    0xa0f96 "eshl_21" shot21::run,
    0xa1180 "eshl_22" shot22::run,
    0xa1292 "eshl_23" shot23::run,
    0xa137e "eshl_24" shot24::run,
    0xa16bc "eshl_25" shot25::run,
    0xa18dc "eshl_26" shot26::run,
    0xa1a64 "eshl_27" shot27::run,
    0xa1c96 "eshl_28" shot28::run,
    0xa1d70 "eshl_29" shot29::run,
    0xa2060 "eshl_30" shot30::run,
    0xa21b6 "eshl_31" shot31::run,
    0xa239a "eshl_32" shot32::run,
    0xa2524 "eshl_33" shot33::run,
    0xa2630 "eshl_34" shot34::run,
    0xa28d2 "eshl_35" shot35::run,
    0xa29cc "eshl_36" shot36::run,
    0xa2b62 "eshl_37" shot37::run,
    0xa2d48 "eshl_38" shot38::run,
    0xa2fb8 "eshl_39" shot39::run,
}
