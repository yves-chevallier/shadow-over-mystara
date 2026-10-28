//! Tirs et attaques des ennemis (classe ESHL), types 64 à 72 (fin de la table 0x39b96). Voir
//! `docs/familles/eshl_c.md`.
//!
//! Même cadre que `eshl.rs` : un `mod` par type, `run` est appelé par le dispatch avec l'objet
//! courant ; `+4` est l'état principal, `+5` / `+6` les sous-états (octets pairs).

use super::cx::{handlers, Cx};
use super::shell::*;
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Courant;
use crate::machine::Machine;

/// Alloue un objet ESHL ; `None` si le pool est plein.
/// `subq.w #1,(off,A0)` : vrai quand le compteur atteint zéro.
fn count_down(x: &mut Cx, off: u32) -> bool {
    x.add16(off, 0xffff);
    x.u16(off) == 0
}

// ---------------------------------------------------------------------------------------------
// Type 64 : colonnes de flammes du guerrier (0xa6efa)
// ---------------------------------------------------------------------------------------------

/// Tir 64 (0xa6efa) : flammes posées par un créateur (`+0x5c`, un ennemi ou une autre flamme). La
/// variante 0 compte ses générations dans `+0x2f` : la quatrième retombe en arc et s'éteint au sol
/// (dégâts de zone 0x2860), les autres s'allument sur place ; à la fin de leur temps (`+0x42`), elles
/// créent trois flammes : une de variante 0 (génération suivante) et deux colonnes (variantes 1 et 2)
/// qui montent ou descendent de 8 pixels par génération (`+0x46` : son toutes les trois).
mod flame64 {
    use super::*;

    const ANIMS: u32 = 0x1713f8;
    const CREATOR: u32 = 0x5c;
    const GENERATION: u32 = 0x2f;
    const TIMER: u32 = 0x42;
    const SOUND_COUNT: u32 = 0x46;
    const LAST_GENERATION: u8 = 4;
    const SFX_FLAME: u32 = 0xed548;
    const SFX_LAND: u32 = 0xed4de;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                if x.u16(CREATOR) != 0 {
                    init(x);
                }
            }
            2 => {
                match x.u8(0x3) {
                    0 => active_0(x),
                    1 => column(x, 1),
                    _ => column(x, 2),
                }
                x.reinsert();
            }
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn shape(x: &mut Cx) {
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 0x26);
    }

    fn init(x: &mut Cx) {
        if x.u8(0x3) == 0 {
            let g = x.u8(GENERATION).wrapping_sub(1);
            x.set8(GENERATION, g);
            if g == 0 {
                x.set8(0x4, 4);
                return;
            }
            x.add8(0x4, 2);
            if g == LAST_GENERATION {
                x.set8(SOUND_COUNT, 3);
                shape(x);
                follow(x);
                x.set16(0x18, 0xfb40);
                x.anim_start(ANIMS, 0);
            } else {
                follow(x);
                let dx = if x.u8(f::FLIP) != 0 { 0x20 } else { 0xffe0 };
                x.add16(0x8, dx);
                x.set16(TIMER, 12);
                shape(x);
                x.anim_start(ANIMS, 2);
            }
            return;
        }
        x.add8(0x4, 2);
        let n = x.u8(SOUND_COUNT).wrapping_sub(1);
        x.set8(SOUND_COUNT, n);
        if n == 0 {
            crate::sound::jouer(x.b, SFX_FLAME, x.o);
            x.set8(SOUND_COUNT, 3);
        }
        follow(x);
        let dz = if x.u8(0x3) == 1 { 8 } else { 0xfff8 };
        x.add16(0x10, dz);
        let z = x.u16(0x10) as i16;
        if z <= 0x170 || z > 0x1af {
            x.set8(0x4, 4);
        }
        x.set16(TIMER, 12);
        shape(x);
        x.anim_start(ANIMS, 2);
    }

    /// Position (longs) et orientation du créateur, un pixel devant lui.
    fn follow(x: &mut Cx) {
        let creator = x.linked(CREATOR);
        x.c.a[1] = x.u16(CREATOR) as i16 as i32 as u32;
        for off in [0x8, 0x10, 0xc] {
            let v = creator.u32(x.b, off);
            x.set32(off, v);
        }
        let flip = creator.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.add16(0x10, 0xffff);
    }

    fn active_0(x: &mut Cx) {
        if x.u8(GENERATION) != LAST_GENERATION {
            if count_down(x, TIMER) {
                spawn_all(x);
            }
            finish(x);
            return;
        }
        match x.u8(0x5) {
            0 => {
                if x.anim_step() {
                    x.add8(0x5, 2);
                    shape(x);
                    crate::sound::jouer(x.b, SFX_LAND, x.o);
                    x.anim_start(ANIMS, 1);
                }
            }
            2 => {
                if x.move_xyz().gt() {
                    x.anim_step();
                    return;
                }
                x.set16(0xc, 0);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 8);
                x.add8(0x5, 2);
                shape(x);
                x.set16(TIMER, 12);
                x.anim_start(ANIMS, 2);
                crate::sound::jouer(x.b, SFX_FLAME, x.o);
            }
            _ => {
                if count_down(x, TIMER) {
                    spawn_all(x);
                }
                finish(x);
            }
        }
    }

    /// Colonne : à la fin de son temps, crée la flamme suivante de sa variante.
    fn column(x: &mut Cx, variant: u8) {
        if count_down(x, TIMER) {
            spawn(x, variant);
        }
        finish(x);
    }

    fn finish(x: &mut Cx) {
        if x.anim_step() {
            x.add8(0x4, 2);
        }
    }

    fn spawn_all(x: &mut Cx) {
        spawn(x, 0);
        spawn(x, 1);
        spawn(x, 2);
    }

    /// Flamme fille de variante `variant` (0xa6e8e, 0xa6eb2, 0xa6ed6) : la variante 0 hérite de la
    /// génération, les colonnes du compteur de sons.
    fn spawn(x: &mut Cx, variant: u8) {
        let Some(child) = x.alloc(&pools::ESHL) else { return };
        child.set_u8(x.b, 0x0, 1);
        child.set_u8(x.b, 0x2, 64);
        child.set_u8(x.b, 0x3, variant);
        if variant == 0 {
            let g = x.u8(GENERATION);
            child.set_u8(x.b, GENERATION, g);
            child.set_u16(x.b, CREATOR, x.o.0 as u16);
        } else {
            child.set_u16(x.b, CREATOR, x.o.0 as u16);
            let n = x.u8(SOUND_COUNT);
            child.set_u8(x.b, SOUND_COUNT, n);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 65 : pluie de pierres (0xa72c4)
// ---------------------------------------------------------------------------------------------

/// Tir 65 (0xa72c4) : pierre qui tombe du haut de l'écran à une position tirée au hasard (deux
/// tirages : X dans l'écran, profondeur dans une bande de 0x50), après une attente lue dans une table
/// par rang `+0x2f` ; au sol elle déclenche des dégâts de zone (0x2860). Variante 0 : éclat d'impact
/// (0xab7d8) et disparition ; variante 1 : animation d'impact jusqu'à son terme.
mod rocks65 {
    use super::*;

    /// Par variante : jeu d'animations, identifiant d'attaque, attentes par rang (octets, ROM programme).
    const ANIMS: [u32; 2] = [0x16ea18, 0x171574];
    const ATTACK: [u8; 2] = [0x25, 0x38];
    const DELAYS: [u32; 2] = [0xa739a, 0xa74b6];
    const TIMER: u32 = 0x42;
    const RANK: u32 = 0x2f;
    /// Coin de l'écran (joueur 1, ou joueur 2 si `A5+0x36`).
    const SCREEN: u32 = 0x4fe;
    const SCREEN_P2: u32 = 0x54e;
    const SFX: [u32; 2] = [0xeafdc, 0xed548];

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        let v = (x.u8(0x3) != 0) as usize;
        // la variante 1 a un état de plus (fin d'animation) avant la libération
        let state = x.u8(0x4);
        match (state, v) {
            (0, _) => init(x, v),
            (2, _) => fall(x, v),
            (4, 1) => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
            }
            (4, 0) | (6, 1) => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn init(x: &mut Cx, v: usize) {
        x.add8(0x4, 2);
        x.set8(0x5, 0);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, ATTACK[v]);
        let screen = if x.a5_u8(0x36) != 0 { SCREEN_P2 } else { SCREEN };
        x.c.a[2] = crate::mem::A5 + screen;
        x.set16(0xc, 0x100);
        let r = random(x) & 0x1ff;
        let dx = if r > 0x180 { (r >> 1) + 0x40 } else { r };
        let left = x.a5_u16(screen + 8);
        x.set16(0x8, dx.wrapping_add(left));
        let r = random(x) & 0x7f;
        let dz = if r > 0x50 { (r >> 1) + 0x10 } else { r } + 0x10;
        let top = x.a5_u16(screen + 0xc);
        x.set16(0x10, dz.wrapping_add(top));
        x.set16(0x18, 0xfe00);
        x.set16(0x1a, 0xffe0);
        x.set32(0x14, 0);
        x.set32(0x1c, 0);
        let rank = (x.u8(RANK) & 0x1f) as u32;
        let delay = x.rom8(DELAYS[v] + rank);
        x.c.d[0] = delay as u32;
        x.set16(TIMER, delay as u16);
        x.anim_start(ANIMS[v], 0);
    }

    /// Tirage pseudo-aléatoire (mot bas de D0).
    fn random(x: &mut Cx) -> u16 {
        x.random()
    }

    fn fall(x: &mut Cx, v: usize) {
        if x.u16(TIMER) as i16 > 0 {
            x.add16(TIMER, 0xffff);
            return;
        }
        x.anim_step();
        x.move_z();
        if x.u16(0xc) as i16 > 0 {
            crate::ai::keep_on_screen_d5(x.c, x.b, x.o, 16);
            x.reinsert();
            return;
        }
        x.set16(0xc, 0);
        x.add8(0x4, 2);
        if v == 0 {
            x.c.d[0] = 3;
            crate::families::effects_creators::debris(x.b, x.o, 3);
        } else {
            x.anim_start(ANIMS[1], 1);
        }
        crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 8);
        crate::sound::jouer(x.b, SFX[v], x.o);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 66 : éclats tenus puis lâchés (0xa75c6)
// ---------------------------------------------------------------------------------------------

/// Tir 66 (0xa75c6) : éclat collé à son porteur (`+0x46`, mot) à un décalage lu dans une table par
/// variante et rang `+0x2f`, qui s'en rapproche en clignotant pendant 8 images, puis est projeté vers
/// le haut, retombe (son, dégâts de zone 0x2824), attend 16 images et clignote 16 images avant de
/// disparaître. Sans porteur, il repart de l'initialisation.
mod shard66 {
    use super::*;

    const ANIMS: u32 = 0x17b9d6;
    /// Animation par variante, à la création puis à l'impact (octets, ROM programme).
    const START_ANIMS: u32 = 0xa760e;
    const LAND_ANIMS: u32 = 0xa777a;
    /// Décalages (X, profondeur) par variante, quatre octets par rang (mots, en clair).
    const OFFSETS: [u32; 6] = [0xa7834, 0xa7834, 0xa7854, 0xa7870, 0xa7890, 0xa78a8];
    const CARRIER: u32 = 0x46;
    const TIMER: u32 = 0x42;
    const RANK: u32 = 0x2f;
    const SFX_LAND: u32 = 0xe4de6;
    /// États `+4.w` de l'éclat dans lequel il reste tenu.
    const HELD_STATE: u16 = 0x204;

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
        x.set8(0x5, 0);
        x.set8(f::BOX_SET, 0x2c);
        x.set8(f::ATTACK_ID, 0x31);
        let v = x.u8(0x3) as usize;
        let table = OFFSETS.get(v).copied().unwrap_or(OFFSETS[0]);
        let row = table + 4 * x.u8(RANK) as u32;
        let dx = x.data16(row);
        let dx = if x.u8(f::FLIP) != 0 { dx.wrapping_neg() } else { dx };
        x.set16(0x14, dx);
        let dz = x.data16(row + 2);
        x.set16(0x1c, dz);
        x.set16(0x18, 0x60);
        x.set16(TIMER, 8);
        let n = x.rom8(START_ANIMS + v as u32) as u32;
        x.anim_start(ANIMS, n as u16);
    }

    fn active(x: &mut Cx) {
        let carrier = x.u16(CARRIER);
        if carrier as i16 >= 0 {
            // pas de porteur : l'original repart de l'initialisation (sous-état 4)
            x.set16(0x4, 4);
            return;
        }
        x.c.a[1] = carrier as i16 as i32 as u32;
        let carrier = Obj(x.c.a[1] & 0xff_ffff);
        match x.u8(0x5) {
            0 => {
                for off in [0x8, 0xc, 0x10] {
                    let v = carrier.u16(x.b, off);
                    x.set16(off, v);
                }
                let flip = carrier.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                for (pos, speed) in [(0x8, 0x14), (0x10, 0x1c), (0xc, 0x18)] {
                    let d = x.u16(speed);
                    x.add16(pos, d);
                }
                let t = x.u16(TIMER);
                if t as i16 > 0 {
                    let t = t - 1;
                    x.set16(TIMER, t);
                    x.add16(0x10, t);
                    return blink(x, t & 1 != 0);
                }
                // (test de l'original sur l'éclat lui-même : toujours faux dans le sous-état 0)
                if x.u16(0x4) == HELD_STATE && (x.u8(0x6) as i8) < 6 {
                    return blink(x, false);
                }
                x.add8(0x5, 2);
                x.set32(0x14, 0);
                x.set32(0x18, 0);
                x.set32(0x1c, 0);
                x.set16(0x18, 0xf800);
                x.set16(0x1a, 0xff00);
            }
            2 => {
                x.move_z();
                if x.u16(0xc) as i16 <= 0 {
                    x.set16(0xc, 0);
                    x.add8(0x5, 2);
                    x.set8(f::ATTACK_ID, 0x32);
                    x.set16(TIMER, 16);
                    let v = x.u8(0x3) as u32;
                    let n = x.rom8(LAND_ANIMS + v) as u32;
                    x.anim_start(ANIMS, n as u16);
                    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::A, 4);
                    crate::sound::jouer(x.b, SFX_LAND, x.o);
                }
                blink(x, false);
            }
            4 => {
                if count_down_signed(x) {
                    x.set16(TIMER, 16);
                    x.add8(0x5, 2);
                } else {
                    blink(x, false);
                }
            }
            _ => {
                let t = x.u16(TIMER);
                if t as i16 <= 0 {
                    x.add8(0x4, 2);
                    return;
                }
                let t = t - 1;
                x.set16(TIMER, t);
                blink(x, t & 1 != 0);
            }
        }
    }

    /// `subq.w #1,+0x42 ; bgt` : vrai quand le compteur n'est plus positif.
    fn count_down_signed(x: &mut Cx) -> bool {
        x.add16(TIMER, 0xffff);
        x.u16(TIMER) as i16 <= 0
    }

    /// Fin commune : visibilité (bit 1 de `+1` : caché), terrain, retenue dans l'écran, tri.
    fn blink(x: &mut Cx, hidden: bool) {
        x.set_bit(0x1, 1, hidden);
        x.call(crate::terrain::SHADOW.entry);
        crate::ai::keep_on_screen_d5(x.c, x.b, x.o, 16);
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 67 : tonneau lancé (0xa78c2)
// ---------------------------------------------------------------------------------------------

/// Tir 67 (0xa78c2) : objet lancé en arc vers le propriétaire (`+0x50`) : la vitesse verticale est
/// choisie pour l'atteindre (12 de moins par pas de la distance horizontale, bornée à 16..256). Il
/// tourne sur lui-même (orientation basculée toutes les 4 images) ; au sol il glisse 30 images en
/// ralentissant (effet 0x1172e toutes les 6 images) ; touché ou paré, il rebondit trois fois (effets
/// 12, 13, 8). Il clignote pendant ses 30 dernières images et disparaît hors de l'écran.
mod barrel67 {
    use super::*;

    const ANIMS: u32 = 0x171a52;
    const SPEED_TABLE: u32 = 53;
    const TIMER: u32 = 0x58;
    const DUST_TIMER: u32 = 0x5a;
    const SPIN_TIMER: u32 = 0x5e;

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            _ => x.free_eshl(),
        }
        if x.u8(0x0) != 0 {
            x.reinsert();
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        let dir = if x.u8(f::FLIP) != 0 { 8 } else { 24 };
        x.set8(0x2d, dir);
        x.select_speed(&crate::objects::motion::SPEEDS_B, SPEED_TABLE as u16);
        let step = ((x.u16(0x14) as i16).unsigned_abs() as i32) << 16 >> 8;
        let owner = x.linked(f::OWNER);
        x.c.a[1] = x.u16(f::OWNER) as i16 as i32 as u32;
        let mut distance = (owner.u32(x.b, 0x8).wrapping_sub(x.u32(0x8)) as i32).wrapping_abs();
        let mut vz = (x.u32(0xc) as i32 >> 8) as i16 >> 6;
        loop {
            distance = distance.wrapping_sub(step);
            if distance <= 0 {
                break;
            }
            vz = vz.wrapping_sub(12);
        }
        let vz = vz.clamp(16, 256);
        x.set16(0x1a, (vz as u16).wrapping_neg());
        crate::families::enemy_lib::start(x, ANIMS, 21);
    }

    /// 0xa7af4 : bascule l'orientation toutes les 4 images.
    fn spin(x: &mut Cx) {
        x.add16(SPIN_TIMER, 0xffff);
        if x.u16(SPIN_TIMER) as i16 > 0 {
            return;
        }
        x.set16(SPIN_TIMER, 4);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
    }

    fn impact(x: &mut Cx, kind: u32) {
        crate::ai_aids::debris_set(x.b, x.o, kind as u16); x.c.d[5] = 0xffff; // D5 : compteur épuisé du `dbf`, relu plus loin (voir `ai::keep_on_screen_d5`)
    }

    /// Mouvement en X et en hauteur ; vrai quand l'objet touche le sol (hauteur remise à 0).
    fn lands(x: &mut Cx) -> bool {
        x.move_xz();
        if x.u16(0xc) as i16 > 0 {
            return false;
        }
        x.set16(0xc, 0);
        true
    }

    fn active(x: &mut Cx) {
        match x.u8(0x5) {
            0 => {
                if crate::ai_aids::outside_band(x.b, x.o, 0xffc0, 448) {
                    return x.free_eshl();
                }
                if x.u8(f::HIT_FLAGS) & 0x28 != 0 {
                    x.set8(0x5, 4);
                    let vx = (x.u16(0x14) as i16 >> 3) as u16;
                    x.set16(0x14, vx.wrapping_neg());
                    x.set16(0x18, 0x200);
                    x.set16(0x1a, 0xffe0);
                    return;
                }
                spin(x);
                if lands(x) {
                    x.set8(0x5, 2);
                    x.set16(TIMER, 30);
                    x.set8(0x0, 2);
                    let friction = (x.u16(0x14) as i16 >> 5) as u16;
                    x.set16(0x16, friction.wrapping_neg());
                }
            }
            2 => {
                spin(x);
                x.move_x();
                if count_down_le(x, TIMER) {
                    x.set8(0x5, 6);
                    x.set16(TIMER, 60);
                } else if count_down_le(x, DUST_TIMER) {
                    x.set16(DUST_TIMER, 6);
                    impact(x, 9);
                }
            }
            4 => match x.u8(0x6) {
                0 => {
                    spin(x);
                    x.move_xz();
                    if x.u16(0x18) as i16 <= 0 {
                        x.add8(0x6, 2);
                        x.set8(0x0, 2);
                        x.set16(0x1a, 0xffa0);
                    }
                }
                2 => {
                    spin(x);
                    if lands(x) {
                        x.add8(0x6, 2);
                        x.set16(0x18, 0x300);
                        x.set16(0x1a, 0xffc0);
                        impact(x, 12);
                    }
                }
                4 => {
                    if lands(x) {
                        x.add8(0x6, 2);
                        x.set16(0x18, 0x200);
                        x.set16(0x1a, 0xffc0);
                        impact(x, 13);
                    }
                }
                _ => {
                    if lands(x) {
                        x.set8(0x5, 6);
                        x.set16(TIMER, 60);
                        impact(x, 8);
                    }
                }
            },
            _ => {
                if count_down_le(x, TIMER) {
                    return x.free_eshl();
                }
                if x.u16(TIMER) as i16 <= 30 {
                    let v = x.u8(0x1) ^ 2;
                    x.set8(0x1, v);
                }
            }
        }
    }

    /// `subq.w #1 ; ble` : vrai quand le compteur n'est plus positif.
    fn count_down_le(x: &mut Cx, off: u32) -> bool {
        x.add16(off, 0xffff);
        x.u16(off) as i16 <= 0
    }
}

// ---------------------------------------------------------------------------------------------
// Type 68 : pierres soulevées (0xa7b76)
// ---------------------------------------------------------------------------------------------

/// Tir 68 (0xa7b76) : pierre qui monte du sol (premier plan) au-dessus d'une abscisse lue par rang
/// `+0x2f`, puis reste en l'air tant que son porteur (`+0x46`) n'est pas dans l'état 0x204 au-delà du
/// sous-état 6 ; lâchée, elle file (table de vitesses 31, direction par rang) avec une animation qui
/// dépend de sa hauteur dans l'écran, et disparaît en passant sous le haut de l'écran.
mod stone68 {
    use super::*;

    const ANIMS: u32 = 0x17b9d6;
    /// Abscisse par rang (mots, ROM programme), direction par rang (octets, ROM programme).
    const COLUMNS: u32 = 0xa7be4;
    const DIRECTIONS: u32 = 0xa7c7c;
    /// Animations selon la hauteur dans l'écran : compte puis paires (animation, seuil) (mots, en clair).
    const HEIGHT_ANIMS: u32 = 0xa7d50;
    const CARRIER: u32 = 0x46;
    const RANK: u32 = 0x2f;
    const TIMER: u32 = 0x42;
    const SPEED_TABLE: u32 = 31;
    const SFX_RELEASE: u32 = 0xe4de6;
    const SCREEN_TOP: u32 = 0x50a;
    const SCREEN_TOP_P2: u32 = 0x55a;
    const RELEASE_STATE: u32 = 0x0204_0200;
    const RISE_HEIGHT: u16 = 0x20;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x5, 0);
                x.set8(f::BOX_SET, 0x2c);
                x.set8(f::ATTACK_ID, 0x33);
                x.set16(0x10, 0x1c0);
                x.set16(0xc, 0xff90);
                let rank = x.u8(RANK) as u32;
                let column = x.rom16(COLUMNS + 2 * rank);
                x.set16(0x8, column);
                x.set16(TIMER, 8);
                x.anim_start(ANIMS, 0x2a);
            }
            2 => active(x),
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn active(x: &mut Cx) {
        let carrier = x.u16(CARRIER);
        if carrier as i16 >= 0 {
            x.set16(0x4, 4);
            return;
        }
        x.c.a[1] = carrier as i16 as i32 as u32;
        let carrier = Obj(x.c.a[1] & 0xff_ffff);
        match x.u8(0x5) {
            0 => {
                let z = x.u16(0xc);
                let z = z.wrapping_sub((z.wrapping_sub(RISE_HEIGHT) as i16 >> 3) as u16);
                x.set16(0xc, z);
                if (z as i16) < RISE_HEIGHT as i16 {
                    x.set_bit(0x1, 1, false);
                    x.reinsert_foreground();
                    return;
                }
                x.set16(0xc, RISE_HEIGHT);
                x.add8(0x5, 2);
                x.set32(0x14, 0);
                x.set32(0x1c, 0);
                x.set32(0x18, 0);
                let rank = (x.u8(RANK) & 7) as u32;
                let dir = x.rom8(DIRECTIONS + rank);
                x.set8(0x2d, dir);
                x.select_speed(&crate::objects::motion::SPEEDS_A, SPEED_TABLE as u16);
                x.unlink();
            }
            2 => {
                let held = carrier.u8(x.b, 0x0) != 0 && carrier.u32(x.b, 0x4) & !0xff == RELEASE_STATE;
                if !(held && (carrier.u8(x.b, 0x7) as i8) < 6) {
                    x.add8(0x5, 2);
                    let rank = x.u8(RANK);
                    if rank == 0 || rank == 5 {
                        crate::sound::jouer(x.b, SFX_RELEASE, x.o);
                    }
                    return;
                }
            }
            _ => {
                height_anim(x);
                x.move_xy();
                let top = x.a5_u16(SCREEN_TOP);
                if (x.u16(0x10).wrapping_sub(top) as i16) <= 0 {
                    x.add8(0x4, 2);
                    return;
                }
            }
        }
        x.set_bit(0x1, 1, false);
        x.reinsert();
    }

    /// 0xa7d20 : animation selon la hauteur dans l'écran (seuils croissants, la dernière par défaut).
    fn height_anim(x: &mut Cx) {
        let top = if x.a5_u8(0x36) != 0 { SCREEN_TOP_P2 } else { SCREEN_TOP };
        let h = x.u16(0x10).wrapping_sub(x.a5_u16(top)) as i16;
        let count = x.data16(HEIGHT_ANIMS);
        let mut anim = 0;
        for k in 0..=count as u32 {
            anim = x.data16(HEIGHT_ANIMS + 2 + 4 * k);
            if h < x.data16(HEIGHT_ANIMS + 4 + 4 * k) as i16 {
                break;
            }
        }
        x.anim_start(ANIMS, (anim as u32) as u16);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 69 : pierre qui rebondit (0xa7e30)
// ---------------------------------------------------------------------------------------------

/// Tir 69 (0xa7e30) : pierre lancée devant le créateur (48 pixels, hauteur 32) après une attente lue
/// par rang `+0x2f` ; elle rebondit à chaque contact avec le sol (son) et disparaît quand elle sort de
/// l'écran (16 pixels à gauche, 400 à droite). `+0x42` (octet, posé par le créateur) = 11 : coup plus
/// fort (0x36).
mod bouncer69 {
    use super::*;

    const ANIMS: u32 = 0x17b9d6;
    /// Par rang : attente puis animation (mots, ROM programme).
    const RANKS: u32 = 0xa7eb4;
    const RANK: u32 = 0x2f;
    const TIMER: u32 = 0x42;
    const SFX_BOUNCE: u32 = 0xe4cfe;
    const CAMERA_X: u32 = 0x506;
    const CAMERA2_X: u32 = 0x556;

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
        x.set8(0x5, 0);
        x.set8(f::BOX_SET, 0x2c);
        let strong = x.u8(TIMER) == 11;
        x.set8(f::ATTACK_ID, if strong { 0x36 } else { 0x33 });
        let dx: u16 = if x.u8(f::FLIP) != 0 { 48 } else { 48u16.wrapping_neg() };
        x.add16(0x8, dx);
        x.set16(0xc, 32);
        bounce(x);
        let rank = x.u8(RANK) as u32;
        let row = RANKS + 4 * rank;
        let delay = x.rom16(row);
        x.set16(TIMER, delay);
        let anim = x.rom16(row + 2);
        x.anim_start(ANIMS, (anim as u32) as u16);
    }

    /// 0xa7ed4 : vitesses d'un bond (vers l'avant), son.
    fn bounce(x: &mut Cx) {
        let vx: u16 = if x.u8(f::FLIP) != 0 { 0x400 } else { 0x400u16.wrapping_neg() };
        x.set16(0x14, vx);
        x.set16(0x16, 0);
        x.set16(0x18, 0x800);
        x.set16(0x1a, 0xff80);
        x.set32(0x1c, 0);
        crate::sound::jouer(x.b, SFX_BOUNCE, x.o);
    }

    fn active(x: &mut Cx) {
        if x.u8(0x5) == 0 {
            x.add16(TIMER, 0xffff);
            if x.u16(TIMER) as i16 > 0 {
                return;
            }
            x.add8(0x5, 2);
        }
        x.anim_step();
        x.move_xz();
        if x.u16(0xc) as i16 <= 0 {
            x.set16(0xc, 0);
            bounce(x);
        }
        let camera = if x.a5_u8(0x36) != 0 { CAMERA2_X } else { CAMERA_X };
        let dx = x.u16(0x8).wrapping_sub(x.a5_u16(camera)) as i16;
        if dx <= -16 || dx > 400 {
            x.add8(0x4, 2);
            return;
        }
        x.set_bit(0x1, 1, false);
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 70 : grilles qui se lèvent selon le défilement (0xa801a)
// ---------------------------------------------------------------------------------------------

/// Tir 70 (0xa801a) : élément de décor dangereux lié à un SET (type 7, `+0x5c`) qui le signale tant que
/// l'écran n'a pas atteint une abscisse donnée (0x500 pour la variante 0, 0x650 pour la variante 1) ;
/// passé ce point, le SET est prévenu (`+0x2f` = 0) et l'objet disparaît. La variante 1 alterne
/// attente (`+0x42`, 4 images) et animation.
mod gate70 {
    use super::*;

    const ANIMS: u32 = 0x1717be;
    const SET: u32 = 0x5c;
    const TIMER: u32 = 0x42;
    const CAMERA_X: u32 = 0x506;
    const LIMITS: [u16; 2] = [0x500, 0x650];

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        let variant = x.u8(0x3);
        match x.u8(0x4) {
            0 => init(x, variant),
            2 => {
                let limit = LIMITS[(variant != 0) as usize];
                if (x.a5_u16(CAMERA_X) as i16) < limit as i16 {
                    if variant == 0 {
                        x.anim_step();
                    } else {
                        wait_and_animate(x);
                    }
                    return;
                }
                let set = x.linked(SET);
                x.c.a[1] = x.u16(SET) as i16 as i32 as u32;
                set.set_u8(x.b, 0x2f, 0);
                x.add8(0x4, 2);
                if variant == 0 {
                    x.anim_step();
                }
            }
            _ => x.free_eshl(),
        }
    }

    fn init(x: &mut Cx, variant: u8) {
        x.add8(0x4, 2);
        if let Some(set) = x.alloc(&pools::SET) {
            set.set_u8(x.b, 0x0, 1);
            set.set_u8(x.b, 0x2, 7);
            set.set_u8(x.b, 0x3, if variant == 0 { 0x2c } else { 0x2e });
            set.set_u8(x.b, 0x2f, 1);
            x.set16(SET, set.0 as u16);
        }
        if variant != 0 {
            x.set16(TIMER, 4);
        }
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, if variant == 0 { 0x34 } else { 0x35 });
        x.anim_start(ANIMS, ((variant != 0) as u32) as u16);
        x.reinsert();
    }

    fn wait_and_animate(x: &mut Cx) {
        if x.u8(0x5) == 0 {
            x.add16(TIMER, 0xffff);
            if x.u16(TIMER) as i16 >= 0 {
                return;
            }
            x.add8(0x5, 2);
            x.anim_start(ANIMS, 1);
        } else if x.anim_step() {
            x.add8(0x5, 0xfe);
            x.set16(TIMER, 4);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 71 : boules qui roulent (0xa81ca)
// ---------------------------------------------------------------------------------------------

/// Tir 71 (0xa81ca) : boule lancée par un ennemi (`+0x46`). Variante 0 : tenue jusqu'à ce que `+0x58`
/// la libère (disparaît si le lanceur passe dans l'état 4 hors 0x204), part en arc (table de vitesses
/// 33, direction selon l'orientation), roule au sol ; touchée (`+0x33`), elle crée devant elle une boule
/// de variante 1 si elle est dans l'écran. Variante 1 : roule jusqu'à la fin de son animation.
mod ball71 {
    use super::*;

    const ANIMS: u32 = 0x16f58c;
    const OWNER: u32 = 0x46;
    const RELEASE: u32 = 0x58;
    const HIT: u32 = 0x33;
    const SPEED_TABLE: u32 = 33;
    const SFX_THROW: u32 = 0xed4de;

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        let variant = x.u8(0x3);
        match x.u8(0x4) {
            0 => init(x, variant),
            2 => match (variant, x.u8(0x5)) {
                (0, 0) => held(x),
                (0, 2) => {
                    x.move_xyz();
                    if x.u16(0xc) as i16 > 0 {
                        crate::families::enemy_lib::step(x);
                    } else {
                        x.add8(0x5, 2);
                        x.set16(0xc, 0);
                        ai_anim(x, 3);
                    }
                }
                _ => rolling(x),
            },
            _ => x.free_eshl(),
        }
        if x.u8(0x0) != 0 {
            x.reinsert();
        }
    }

    fn ai_anim(x: &mut Cx, n: u32) {
        crate::families::enemy_lib::start(x, ANIMS, n);
    }

    fn init(x: &mut Cx, variant: u8) {
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 0x14);
        if variant != 0 {
            return ai_anim(x, 3);
        }
        crate::placement::load_effect_palette(x.b, crate::placement::effect_palette::LIGHT);
        let dir = if x.u8(f::FLIP) != 0 { 8 } else { 24 };
        x.set8(0x2d, dir);
        x.select_speed(&crate::objects::motion::SPEEDS_B, SPEED_TABLE as u16);
        x.set16(0x18, 0xfc00);
        ai_anim(x, 0);
    }

    /// Tenue par le lanceur : libérée par `+0x58`, détruite si le lanceur meurt (état 4 hors 0x204).
    fn held(x: &mut Cx) {
        let owner = x.linked(OWNER);
        x.c.a[1] = x.u16(OWNER) as i16 as i32 as u32;
        let dying = owner.u8(x.b, 0x4) == 4 && owner.u16(x.b, 0x4) != 0x204;
        x.c.d[0] = dying as u32;
        if dying {
            return x.free_eshl();
        }
        if x.u8(RELEASE) != 0 {
            x.add8(0x5, 2);
            crate::sound::jouer(x.b, SFX_THROW, x.o);
        }
        crate::families::enemy_lib::step(x);
    }

    fn rolling(x: &mut Cx) {
        if crate::families::enemy_lib::step(x) {
            return x.free_eshl();
        }
        if x.u8(HIT) == 0 {
            return;
        }
        x.set8(HIT, 0);
        if crate::ai_aids::outside_band(x.b, x.o, 0xfff0, 400) {
            return;
        }
        let Some(ball) = x.alloc(&pools::ESHL) else { return };
        ball.set_u8(x.b, 0x0, 1);
        ball.set_u8(x.b, 0x2, 71);
        ball.set_u8(x.b, 0x3, 1);
        for off in [0x8, 0x10] {
            let v = x.u16(off);
            ball.set_u16(x.b, off, v);
        }
        let flip = x.u8(f::FLIP);
        ball.set_u8(x.b, f::FLIP, flip);
        let dx: u16 = if flip != 0 { 32 } else { 32u16.wrapping_neg() };
        let bx = ball.u16(x.b, 0x8).wrapping_add(dx);
        ball.set_u16(x.b, 0x8, bx);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 72 : éclats projetés par un créateur (0xa838e)
// ---------------------------------------------------------------------------------------------

/// Tir 72 (0xa838e) : éclat posé sur son créateur (`+0x5c`) avec un décalage et des vitesses lus dans
/// une fiche par rang `+0x2f` et variante ; il finit son animation, part en arc, retombe (dégâts de
/// zone 0x2860 de force 40) et disparaît à la fin de l'animation d'impact. Sans créateur, il se libère.
mod splinter72 {
    use super::*;

    const ANIMS: u32 = 0x1718d8;
    const IMPACT_ANIMS: u32 = 0x16d1d6;
    /// Par rang : adresse d'une fiche de 18 octets par variante (longs puis mots, en clair).
    const ROWS: u32 = 0xa84f4;
    const CREATOR: u32 = 0x5c;
    const RANK: u32 = 0x2f;
    const SFX_THROW: u32 = 0xed4de;
    const SFX_LAND: u32 = 0xed548;

    pub fn run(x: &mut Cx) {
        if x.u16(CREATOR) == 0 {
            x.set32(0x4, 0x0600_0000);
            return;
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                crate::placement::load_effect_palette(x.b, crate::placement::effect_palette::LIGHT);
                place(x);
            }
            2 => {
                match x.u8(0x5) {
                    0 => {
                        if x.anim_step() {
                            x.add8(0x5, 2);
                            x.set8(f::BOX_SET, 7);
                            x.set8(f::ATTACK_ID, 0x37);
                            crate::sound::jouer(x.b, SFX_THROW, x.o);
                            x.anim_start(ANIMS, 1);
                        }
                    }
                    2 => {
                        if x.move_xyz().gt() {
                            x.anim_step();
                        } else {
                            x.set16(0xc, 0);
                            crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 40);
                            x.add8(0x5, 2);
                            x.set8(f::BOX_SET, 7);
                            x.set8(f::ATTACK_ID, 0x1c);
                            x.anim_start(IMPACT_ANIMS, 0);
                            crate::sound::jouer(x.b, SFX_LAND, x.o);
                        }
                    }
                    _ => {
                        if x.anim_step() {
                            x.add8(0x4, 2);
                        }
                    }
                }
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    /// 0xa8478 : position (longs) et orientation du créateur, un pixel devant lui, plus le décalage et
    /// les vitesses de la fiche.
    fn place(x: &mut Cx) {
        let creator = x.linked(CREATOR);
        x.c.a[1] = x.u16(CREATOR) as i16 as i32 as u32;
        for off in [0x8, 0x10, 0xc] {
            let v = creator.u32(x.b, off);
            x.set32(off, v);
        }
        let flip = creator.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.add16(0x10, 0xffff);
        let rank = x.u8(RANK) as u32;
        let rows = x.data32(ROWS + 4 * rank);
        let variant = x.u8(0x3) as u32;
        let row = rows + 18 * variant;
        let [dx, dy, dz] = [0, 2, 4].map(|k| x.data16(row + k));
        for (field, k) in [(0x14, 6), (0x18, 8), (0x18, 10), (0x1a, 12), (0x1c, 14), (0x1e, 16)] {
            let v = x.data16(row + k);
            x.set16(field, v);
        }
        x.add16(0x8, dx);
        x.add16(0xc, dy);
        x.add16(0x10, dz);
    }
}

handlers! {
    0xa6efa "eshl_64" flame64::run,
    0xa72c4 "eshl_65" rocks65::run,
    0xa75c6 "eshl_66" shard66::run,
    0xa78c2 "eshl_67" barrel67::run,
    0xa7b76 "eshl_68" stone68::run,
    0xa7e30 "eshl_69" bouncer69::run,
    0xa801a "eshl_70" gate70::run,
    0xa81ca "eshl_71" ball71::run,
    0xa838e "eshl_72" splinter72::run,
}
