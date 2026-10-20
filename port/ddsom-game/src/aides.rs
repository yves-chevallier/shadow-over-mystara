//! Petites routines communes (V2) : direction entre deux points (0x1386), objet saisi (0x2810) et
//! fin de partie quand tous les joueurs sont à terre (0x28d8, dernière étape de la phase 12).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::mem::{prog8, rd16, rd32, rd8, rts, wr16, wr32, wr8, A5};
use crate::{call, ops, regs, Native};

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

/// Direction du point (D3, D4) vers le point (D1, D2). Les écarts sont pris en valeur absolue (16 bits,
/// positions biaisées de 0x8000 pour comparer en non signé) ; l'octant dépend des signes et du plus grand
/// écart, le pas (0 à 3) du rapport entre le petit écart et le grand, estimé par soustractions.
/// Registres de travail comme l'original les laisse (D1 à D4 et D6).
fn direction(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut octant = 0u8;
    let mut dx = c.d[1].wrapping_add(0x8000);
    let (diff, borrow) = (dx as u16).overflowing_sub(c.d[3].wrapping_add(0x8000) as u16);
    let abs = if borrow { diff.wrapping_neg() } else { diff };
    set_low_word(&mut dx, abs);
    if borrow {
        octant = 4;
    }
    let mut dy = c.d[2].wrapping_add(0x8000);
    let (diff, borrow) = (dy as u16).overflowing_sub(c.d[4].wrapping_add(0x8000) as u16);
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
    if (dy as u16) < dx as u16 {
        c.d[6] = 2;
        loop {
            step += 1;
            let sum = (dy as u8).wrapping_add(unit as u8);
            set_low_byte(&mut dy, sum);
            if dy as u16 >= dx as u16 {
                break;
            }
            // dbf D6
            let left = c.d[6] as u16;
            set_low_word(&mut c.d[6], left.wrapping_sub(1));
            if left == 0 {
                step += 1;
                break;
            }
        }
    }
    let index = (octant << 3 | step) as u32;
    let value = prog8(b, DIRECTIONS + index);
    c.d[1] = dx;
    c.d[2] = dy;
    c.d[3] = step as u32;
    set_low_word(&mut c.d[4], unit);
    c.d[0] = value as u32;
    ops::logique(c, value as u32, Size::Byte);
    rts(c, b);
}

pub const DIRECTION: Native = Native {
    entry: 0x1386,
    name: "direction",
    cost: |_, _| 200,
    run: direction,
    touches_sound: false,
    outputs: regs::d(0) | regs::CCR,
    exact_cost: false,
};

// ---------------------------------------------------------------------------------------------
// 0x2810 : l'objet A0 est-il saisi par un joueur ?
// ---------------------------------------------------------------------------------------------

/// Objet saisi : si le jeu n'est pas en démonstration (`A5+0x42` nul) et que le mot `+0x46` de A0 désigne
/// un joueur, A1 = ce joueur et Z dit s'il est à l'état « normal » (`+0x2c` nul).
fn grabbed(c: &mut Cpu, b: &mut Cps2Bus) {
    if rd8(b, A5 + 0x42) != 0 {
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
    cost: |_, _| 60,
    run: grabbed,
    touches_sound: false,
    outputs: regs::d(0) | regs::a(1) | regs::CCR,
    exact_cost: false,
};

// ---------------------------------------------------------------------------------------------
// 0x28d8 : tous les joueurs à terre
// ---------------------------------------------------------------------------------------------

/// Variables de A5.
mod var {
    pub const STAGE: u32 = 0x34;
    pub const PLAYERS_PRESENT: u32 = 0x35;
    pub const GAME_ON: u32 = 0x42;
    pub const LOCK: u32 = 0x3d;
    pub const STAGE_FLAG: u32 = 0x43;
    pub const PHASE: u32 = 0x2c;
    pub const SCROLL_Y: u32 = 0x4ba;
    pub const HOLD: u32 = 0xffff_aa3a; // A5 - 0x55c6
    pub const PLAYERS: u32 = 0x5de;
}

mod player {
    pub const ACTIVE: u32 = 0x00;
    pub const FLAGS: u32 = 0x01;
    pub const STATE: u32 = 0x04;
    pub const ALTITUDE: u32 = 0x0c;
    pub const HP: u32 = 0x62;
    pub const SIZE: u32 = 0x200;
    /// Octets remis à zéro quand le joueur revient (effets, saisie, magie...).
    pub const CLEARED: [u32; 8] = [0x2a, 0x66, 0x15c, 0x79, 0x7a, 0x182, 0x183, 0x184];
    pub const RANK: u32 = 0x7d;
    pub const GRAB: u32 = 0x8a;
    pub const GRAB_OBJECT: u32 = 0x86;
}

mod rout {
    /// Couleurs du joueur (palette de personnage), et passage à l'état « attente de départ ».
    pub const PLAYER_PALETTE: u32 = 0x37882;
    pub const PLAYER_WAIT: u32 = 0x35614;
}

/// Scènes sans fin de partie (boutique, cinématique) et scènes de démonstration particulières.
const STAGE_NO_END: [u8; 2] = [6, 0xc];
const STAGE_SKIP_CHECK: u8 = 0x43;
const STAGE_SKIP_WAIT: u8 = 0x22;
/// État d'un joueur qui attend (`0x2000c00` sans l'octet bas) : la fin n'est pas déclenchée.
const STATE_WAITING: u32 = 0x0200_0c00;
/// Phase de fin de partie (`A5+0x2c` = 0x10, étape 0...) et temporisation selon la scène.
const PHASE_END: u32 = 0x1000_0000;

/// Fin de la phase 12 : quand `A5+0x42` est non nul (partie en cours) et que des joueurs sont à terre
/// (PV nuls), ils sont remis à l'état d'attente et la phase de fin de partie démarre. Rien ne se
/// passe si un joueur est encore en l'air ou dans l'état d'attente.
fn game_over(c: &mut Cpu, b: &mut Cps2Bus) {
    let stage = rd8(b, A5 + var::STAGE);
    if STAGE_NO_END.contains(&stage) || rd8(b, A5 + var::GAME_ON) == 0 {
        return rts(c, b);
    }
    wr8(b, A5 + var::STAGE_FLAG, 0x3f);
    if rd8(b, A5 + var::LOCK) != 0 || rd8(b, A5 + var::PLAYERS_PRESENT) == 0 {
        return rts(c, b);
    }
    c.d[0] = 0;
    c.d[1] = 0;
    c.a[0] = A5 + var::PLAYERS;
    for _ in 0..4 {
        let p = c.a[0];
        if rd8(b, p + player::ACTIVE) != 0 && rd16(b, p + player::HP) != 0 {
            if stage != STAGE_SKIP_CHECK {
                if rd16(b, p + player::ALTITUDE) != 0 || rd32(b, p + player::STATE) & 0xffff_ff00 == STATE_WAITING {
                    return rts(c, b);
                }
            }
            let count = (c.d[1] as u8).wrapping_add(1);
            set_low_byte(&mut c.d[1], count);
            for off in player::CLEARED {
                wr8(b, p + off, 0);
            }
            let flags = rd8(b, p + player::FLAGS) & !2;
            wr8(b, p + player::FLAGS, flags);
            let saved = [c.d[0], c.d[1], c.d[2]];
            call(c, b, rout::PLAYER_PALETTE);
            [c.d[0], c.d[1], c.d[2]] = saved;
            wr8(b, p + player::ACTIVE, 1);
            if rd8(b, p + player::GRAB) != 0 {
                wr8(b, p + player::GRAB, 0);
                let object = rd32(b, p + player::GRAB_OBJECT);
                wr8(b, object + 5, 8);
                wr16(b, object + 6, 0);
            }
            wr8(b, p + player::RANK, c.d[0] as u8);
            let next = (c.d[0] as u8).wrapping_add(1);
            set_low_byte(&mut c.d[0], next);
            if stage != STAGE_SKIP_WAIT && stage != STAGE_SKIP_CHECK {
                call(c, b, rout::PLAYER_WAIT);
            }
        }
        c.a[0] = p + player::SIZE;
    }
    if c.d[1] as u8 != 0 {
        wr32(b, A5 + var::PHASE, PHASE_END);
        wr16(b, A5 + var::SCROLL_Y, 0);
        wr16(b, A5.wrapping_add(var::HOLD), if stage == 1 { 0x60 } else { 0x120 });
    }
    rts(c, b);
}

pub const GAME_OVER: Native = Native {
    entry: 0x28d8,
    name: "fin_de_partie",
    cost: |_, _| 200,
    run: game_over,
    touches_sound: false,
    outputs: 0,
    exact_cost: false,
};

pub const NATIVES: &[Native] = &[DIRECTION, GRABBED, GAME_OVER];

#[cfg(test)]
mod tests {
    // Les fonctions travaillent sur les registres du processeur : validées par la comparaison
    // image par image (`tools/validate_all.sh`) et le mode contrôle.
}
