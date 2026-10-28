//! Petites routines communes (V2) : direction entre deux points (0x1386), objet saisi (0x2810),
//! fin de partie quand tous les joueurs sont à terre (0x28d8, dernière étape de la phase 12) et division
//! entière (0x17ea).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::mem::{prog8, rd16, rd32, rd8, rts, wr16, wr32, wr8, A5};
use crate::{ops, regs, Native};

fn set_low_word(r: &mut u32, v: u16) {
    *r = *r & 0xffff_0000 | v as u32;
}

fn set_low_byte(r: &mut u32, v: u8) {
    *r = *r & 0xffff_ff00 | v as u32;
}

// ---------------------------------------------------------------------------------------------
// 0x1386 : direction (0..31) de (D3, D4) vers (D1, D2)
// ---------------------------------------------------------------------------------------------

/// Table des directions, en ROM programme : indexée par (octant << 3) | pas.
const DIRECTIONS: u32 = 0x140c;

/// Calcul de 0x1386 sur les registres de l'original (D1-D4), avec les registres de travail qu'il laisse.
struct Calcul {
    direction: u8,
    d1: u32,
    d2: u32,
    d3: u32,
    /// D4.w : unité de la comparaison.
    unite: u16,
    /// D6 : compteur de la boucle d'estimation, s'il a été posé.
    d6: Option<u32>,
}

/// Direction du point (D3, D4) vers le point (D1, D2). Les écarts sont pris en valeur absolue (16 bits,
/// positions biaisées de 0x8000 pour comparer en non signé) ; l'octant dépend des signes et du plus grand
/// écart, le pas (0 à 3) du rapport entre le petit écart et le grand, estimé par soustractions.
fn calcul(b: &mut Cps2Bus, d1: u32, d2: u32, d3: u32, d4: u32) -> Calcul {
    let mut octant = 0u8;
    let mut dx = d1.wrapping_add(0x8000);
    let (diff, borrow) = (dx as u16).overflowing_sub(d3.wrapping_add(0x8000) as u16);
    let abs = if borrow { diff.wrapping_neg() } else { diff };
    set_low_word(&mut dx, abs);
    if borrow {
        octant = 4;
    }
    let mut dy = d2.wrapping_add(0x8000);
    let (diff, borrow) = (dy as u16).overflowing_sub(d4.wrapping_add(0x8000) as u16);
    let abs = if borrow { diff.wrapping_neg() } else { diff };
    set_low_word(&mut dy, abs);
    if borrow {
        octant += 2;
    }
    // le grand écart passe dans `dx`
    if (dy as u16) < dx as u16 {
        std::mem::swap(&mut dx, &mut dy);
        if octant == 0 || octant == 4 {
            octant += 1;
        }
    } else if octant == 2 || octant == 6 {
        octant += 1;
    }
    let mut step = 0u8;
    let high = (dy as u16) >> 3;
    set_low_word(&mut dy, high);
    let unit = (dy as u16).wrapping_mul(2);
    let mut d6 = None;
    if (dy as u16) < dx as u16 {
        let mut compteur = 2u32;
        loop {
            step += 1;
            let sum = (dy as u8).wrapping_add(unit as u8);
            set_low_byte(&mut dy, sum);
            if dy as u16 >= dx as u16 {
                break;
            }
            // dbf D6
            let left = compteur as u16;
            set_low_word(&mut compteur, left.wrapping_sub(1));
            if left == 0 {
                step += 1;
                break;
            }
        }
        d6 = Some(compteur);
    }
    let index = (octant << 3 | step) as u32;
    let direction = prog8(b, DIRECTIONS + index);
    Calcul { direction, d1: dx, d2: dy, d3: step as u32, unite: unit, d6 }
}

/// 0x1386 : direction (0 à 31) du point `de` vers le point `vers` (x, z).
pub fn direction(b: &mut Cps2Bus, de: (u16, u16), vers: (u16, u16)) -> u8 {
    calcul(b, vers.0 as u32, vers.1 as u32, de.0 as u32, de.1 as u32).direction
}

/// 0x137e : direction de l'objet `o` (x, z) vers le point `vers`.
pub fn direction_from(b: &mut Cps2Bus, o: crate::objects::Obj, vers: (u16, u16)) -> u8 {
    let de = (rd16(b, o.0 + 8), rd16(b, o.0 + 0x10));
    direction(b, de, vers)
}

/// 0x1376 : direction de l'objet `o` vers l'objet `cible`.
pub fn direction_to(b: &mut Cps2Bus, o: crate::objects::Obj, cible: crate::objects::Obj) -> u8 {
    let vers = (rd16(b, cible.0 + 8), rd16(b, cible.0 + 0x10));
    direction_from(b, o, vers)
}

/// Native de 0x1386 : registres de travail comme l'original les laisse (D1 à D4 et D6), D0 = la direction.
fn direction_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let r = calcul(b, c.d[1], c.d[2], c.d[3], c.d[4]);
    if let Some(d6) = r.d6 {
        c.d[6] = d6;
    }
    c.d[1] = r.d1;
    c.d[2] = r.d2;
    c.d[3] = r.d3;
    set_low_word(&mut c.d[4], r.unite);
    c.d[0] = r.direction as u32;
    ops::logique(c, r.direction as u32, Size::Byte);
    rts(c, b);
}

/// 0x137e : direction de l'objet A0 (D3, D4 = son x et son z) vers (D1, D2).
fn direction_from_object(c: &mut Cpu, b: &mut Cps2Bus) {
    let (x, z) = (rd16(b, c.a[0] + 8), rd16(b, c.a[0] + 0x10));
    set_low_word(&mut c.d[3], x);
    set_low_word(&mut c.d[4], z);
    direction_native(c, b);
}

/// 0x1376 : direction de l'objet A0 vers l'objet A1.
fn direction_to_object(c: &mut Cpu, b: &mut Cps2Bus) {
    let (x, z) = (rd16(b, c.a[1] + 8), rd16(b, c.a[1] + 0x10));
    set_low_word(&mut c.d[1], x);
    set_low_word(&mut c.d[2], z);
    direction_from_object(c, b);
}

pub const DIRECTION_FROM_OBJECT: Native = Native {
    entry: 0x137e,
    name: "direction_objet",
    run: crate::Corps::Processeur(direction_from_object),
    touches_sound: false,
    outputs: regs::d(0) | regs::CCR,
};

pub const DIRECTION_TO_OBJECT: Native = Native {
    entry: 0x1376,
    name: "direction_vers_objet",
    run: crate::Corps::Processeur(direction_to_object),
    touches_sound: false,
    outputs: regs::d(0) | regs::CCR,
};

pub const DIRECTION: Native = Native {
    entry: 0x1386,
    name: "direction",
    run: crate::Corps::Processeur(direction_native),
    touches_sound: false,
    outputs: regs::d(0) | regs::CCR,
};

// ---------------------------------------------------------------------------------------------
// 0x2810 : l'objet A0 est-il saisi par un joueur ?
// ---------------------------------------------------------------------------------------------

/// Résultat de 0x2810.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Saisie {
    /// Jeu gelé (`A5+0x42` non nul) : rien n'est lu (Z = 0).
    Gele,
    /// Personne ne tient l'objet (mot `+0x46` nul ; Z = 1).
    Libre,
    /// Le joueur `joueur` le tient ; `normal` : son octet `+0x2c` est nul (Z).
    Par { joueur: crate::objects::Obj, normal: bool },
}

impl Saisie {
    /// Le drapeau Z que l'original laisse (le test des appelants).
    pub fn z(self) -> bool {
        match self {
            Saisie::Gele => false,
            Saisie::Libre => true,
            Saisie::Par { normal, .. } => normal,
        }
    }
}

/// 0x2810 : l'objet `o` est-il saisi par un joueur (mot `+0x46`) ?
pub fn grabbed(b: &mut Cps2Bus, o: crate::objects::Obj) -> Saisie {
    if rd8(b, A5 + 0x42) != 0 {
        return Saisie::Gele;
    }
    let holder = rd16(b, o.0 + 0x46);
    if holder == 0 {
        return Saisie::Libre;
    }
    let joueur = crate::objects::Obj::from_word(holder);
    Saisie::Par { joueur, normal: rd8(b, joueur.0 + 0x2c) == 0 }
}

/// Native : D0.w = le mot `+0x46`, A1 = le joueur (mot étendu en signe), drapeaux du dernier test.
fn grabbed_native(c: &mut Cpu, b: &mut Cps2Bus) {
    if grabbed(b, crate::objects::Obj(c.a[0])) == Saisie::Gele {
        ops::logique(c, 1, Size::Byte);
    } else {
        let holder = rd16(b, c.a[0] + 0x46);
        set_low_word(&mut c.d[0], holder);
        ops::logique(c, holder as u32, Size::Word);
        if holder != 0 {
            c.a[1] = holder as i16 as i32 as u32;
            let state = rd8(b, c.a[1] + 0x2c);
            ops::logique(c, state as u32, Size::Byte);
        }
    }
    rts(c, b);
}

pub const GRABBED: Native = Native {
    entry: 0x2810,
    name: "objet_saisi",
    run: crate::Corps::Processeur(grabbed_native),
    touches_sound: false,
    outputs: regs::d(0) | regs::a(1) | regs::CCR,
};

// ---------------------------------------------------------------------------------------------
// 0x28d8 : tous les joueurs à terre
// ---------------------------------------------------------------------------------------------

/// Variables de A5.
mod var {
    pub const STAGE: u32 = 0x34;
    pub const GAME_ON: u32 = 0x42;
    pub const LOCK: u32 = 0x3d;
    pub const STAGE_FLAG: u32 = 0x43;
    pub const PHASE: u32 = 0x2c;
    pub const HOLD: u32 = 0xffff_aa3a; // A5 - 0x55c6
}

mod player {
    pub const ACTIVE: u32 = 0x00;
    pub const FLAGS: u32 = 0x01;
    pub const STATE: u32 = 0x04;
    pub const ALTITUDE: u32 = 0x0c;
    pub const HP: u32 = 0x62;
    /// Octets remis à zéro quand le joueur revient (effets, saisie, magie...).
    pub const CLEARED: [u32; 8] = [0x2a, 0x66, 0x15c, 0x79, 0x7a, 0x182, 0x183, 0x184];
    pub const RANK: u32 = 0x7d;
    pub const GRAB: u32 = 0x8a;
    pub const GRAB_OBJECT: u32 = 0x86;
}

/// Scènes sans fin de partie (boutique, cinématique) et scènes de démonstration particulières.
const STAGE_NO_END: [u8; 2] = [6, 0xc];
const STAGE_SKIP_CHECK: u8 = 0x43;
const STAGE_SKIP_WAIT: u8 = 0x22;
/// État d'un joueur qui attend (`0x2000c00` sans l'octet bas) : la fin n'est pas déclenchée.
const STATE_WAITING: u32 = 0x0200_0c00;
/// Phase de fin de partie (`A5+0x2c` = 0x10, étape 0...) et temporisation selon la scène.
const PHASE_END: u32 = 0x1000_0000;

/// 0x28d8, fin de la phase 12 : quand `A5+0x42` est non nul (partie en cours) et que des joueurs sont à terre
/// (PV nuls), ils sont remis à l'état d'attente et la phase de fin de partie démarre. Rien ne se passe si un joueur
/// est encore en l'air ou dans l'état d'attente. (Le processeur ne sert qu'au contexte des joueurs, pour
/// `player::actions::to_stand`.)
pub fn game_over(c: &mut Cpu, b: &mut Cps2Bus) {
    let stage = rd8(b, A5 + var::STAGE);
    if STAGE_NO_END.contains(&stage) || rd8(b, A5 + var::GAME_ON) == 0 {
        return;
    }
    wr8(b, A5 + var::STAGE_FLAG, 0x3f);
    if rd8(b, A5 + var::LOCK) != 0 || crate::ram::JOUEURS_EN_VIE.get(b) == 0 {
        return;
    }
    let (mut rang, mut a_terre) = (0u8, 0u8);
    for p in crate::ram::JOUEURS.iter() {
        let p = p.0;
        if rd8(b, p + player::ACTIVE) != 0 && rd16(b, p + player::HP) != 0 {
            if stage != STAGE_SKIP_CHECK {
                if rd16(b, p + player::ALTITUDE) != 0 || rd32(b, p + player::STATE) & 0xffff_ff00 == STATE_WAITING {
                    return;
                }
            }
            a_terre = a_terre.wrapping_add(1);
            for off in player::CLEARED {
                wr8(b, p + off, 0);
            }
            let flags = rd8(b, p + player::FLAGS) & !2;
            wr8(b, p + player::FLAGS, flags);
            crate::palette::character_palette(b, crate::objects::Obj(p));
            wr8(b, p + player::ACTIVE, 1);
            if rd8(b, p + player::GRAB) != 0 {
                wr8(b, p + player::GRAB, 0);
                let object = rd32(b, p + player::GRAB_OBJECT);
                wr8(b, object + 5, 8);
                wr16(b, object + 6, 0);
            }
            wr8(b, p + player::RANK, rang);
            rang = rang.wrapping_add(1);
            if stage != STAGE_SKIP_WAIT && stage != STAGE_SKIP_CHECK {
                crate::player::actions::to_stand(&mut crate::player::Ctx::new(c, b), crate::player::Player(p));
            }
        }
    }
    if a_terre != 0 {
        wr32(b, A5 + var::PHASE, PHASE_END);
        crate::ram::DEFILEMENT_Y_PLAN1.set(b, 0);
        wr16(b, A5.wrapping_add(var::HOLD), if stage == 1 { 0x60 } else { 0x120 });
    }
}

fn game_over_native(c: &mut Cpu, b: &mut Cps2Bus) {
    game_over(c, b);
    rts(c, b);
}

pub const GAME_OVER: Native = Native {
    entry: 0x28d8,
    name: "fin_de_partie",
    run: crate::Corps::Processeur(game_over_native),
    touches_sound: false,
    outputs: 0,
};

/// `0x17ea` : division non signée de 32 bits par décalages et soustractions (32 tours). Rend le quotient et
/// le reste ; avec un diviseur de plus de 2^31 le reste déborde comme dans l'original (diviseur nul :
/// quotient 0xffffffff, reste le dividende).
pub fn divide(mut n: u32, d: u32) -> (u32, u32) {
    let mut r = 0u32;
    for _ in 0..32 {
        let carry = n >> 31;
        n <<= 1;
        r = r.wrapping_shl(1) | carry;
        if r >= d {
            r = r.wrapping_sub(d);
            n += 1;
        }
    }
    (n, r)
}

/// D0 / D1 : quotient dans D0, reste dans D2 ; D3 = 0xffff (compteur de la boucle).
fn divide_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let (q, r) = divide(c.d[0], c.d[1]);
    c.d[0] = q;
    c.d[2] = r;
    c.d[3] = 0xffff;
    rts(c, b);
}

pub const DIVIDE: Native = Native {
    entry: 0x17ea,
    name: "division",
    run: crate::Corps::Processeur(divide_native),
    touches_sound: false,
    outputs: regs::d(0) | regs::d(2) | regs::d(3),
};

pub const NATIVES: &[Native] = &[DIRECTION, DIRECTION_FROM_OBJECT, DIRECTION_TO_OBJECT, GRABBED, GAME_OVER, DIVIDE];

#[cfg(test)]
mod tests {
    // Les fonctions travaillent sur les registres du processeur : validées par la comparaison
    // image par image (`tools/validate_all.sh`) et le mode contrôle.
}
