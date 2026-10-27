//! Aides d'IA communes aux ennemis et aux boss (sous-système E1) : choix d'un joueur comme cible,
//! tests de portée et de direction, décisions pseudo-aléatoires par tables, valeurs initiales.
//!
//! Ces routines sont appelées depuis des centaines de handlers d'ennemis ; elles gardent donc le
//! contrat exact de l'original (registres, drapeaux). Leur logique est écrite sur `Mach`
//! (`fn xxx(m: &mut Mach)`).
//!
//! Joueurs : 4 enregistrements de 0x200 octets à A5+0x5de ; un joueur est « ciblable » s'il est
//! actif (`+0`), non masqué (bit 1 de `+1`), d'état principal < 4 (`+4`) et `+0x182` nul.

use cps2::Cps2Bus;
use m68k::decode::{Fetch, Size};
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8, A5};
use crate::ops::cc;
use crate::{call, regs, Native};

// ---------------------------------------------------------------------------------------------
// Machine

/// Exécution d'une routine.
struct Mach<'a> {
    c: &'a mut Cpu,
    b: &'a mut Cps2Bus,
}

impl Mach<'_> {
    fn r8(&mut self, a: u32) -> u8 {
        rd8(self.b, a)
    }
    fn r16(&mut self, a: u32) -> u16 {
        rd16(self.b, a)
    }
    fn w8(&mut self, a: u32, v: u8) {
        wr8(self.b, a, v)
    }
    fn w16(&mut self, a: u32, v: u16) {
        wr16(self.b, a, v)
    }
    fn w32(&mut self, a: u32, v: u32) {
        wr32(self.b, a, v)
    }
    /// Lecture en espace programme (adressage relatif au PC).
    fn pc8(&mut self, a: u32) -> u8 {
        let w = self.b.fetch16(a & !1);
        if a & 1 == 0 {
            (w >> 8) as u8
        } else {
            w as u8
        }
    }
    fn pc16(&mut self, a: u32) -> u16 {
        self.b.fetch16(a)
    }
    fn pc32(&mut self, a: u32) -> u32 {
        (self.pc16(a) as u32) << 16 | self.pc16(a + 2) as u32
    }
    /// 0x19a88 : tirage pseudo-aléatoire (D0 reçoit le nouvel état).
    fn random(&mut self) {
        crate::rng::random_68k(self.c, self.b);
    }
}

fn set_w(r: &mut u32, v: u16) {
    *r = (*r & 0xffff_0000) | v as u32;
}

fn set_b(r: &mut u32, v: u8) {
    *r = (*r & 0xffff_ff00) | v as u32;
}

/// Pointeur écrit par `movea.w` : étendu en signe.
fn sext(w: u16) -> u32 {
    w as i16 as i32 as u32
}

// Opérations arithmétiques à drapeaux exacts (X compris), sur le mot ou l'octet bas.

fn add_w(c: &mut Cpu, s: u16, d: u16) -> u16 {
    c.addsub(false, s as u32, d as u32, false, Size::Word, false) as u16
}
fn sub_w(c: &mut Cpu, s: u16, d: u16) -> u16 {
    c.addsub(true, s as u32, d as u32, false, Size::Word, false) as u16
}
fn neg_w(c: &mut Cpu, d: u16) -> u16 {
    c.addsub(true, d as u32, 0, false, Size::Word, false) as u16
}
fn add_b(c: &mut Cpu, s: u8, d: u8) -> u8 {
    c.addsub(false, s as u32, d as u32, false, Size::Byte, false) as u8
}
fn sub_b(c: &mut Cpu, s: u8, d: u8) -> u8 {
    c.addsub(true, s as u32, d as u32, false, Size::Byte, false) as u8
}
/// CMP : drapeaux N Z V C de `d - s`, X inchangé.
fn cmp(c: &mut Cpu, s: u32, d: u32, sz: Size) {
    let x = c.sr & SR_X;
    c.addsub(true, s, d, false, sz, false);
    c.sr = (c.sr & !SR_X) | x;
}
/// Décalage logique à gauche d'un mot : N Z selon le résultat, V = 0, C = X = dernier bit sorti.
fn lsl_w(c: &mut Cpu, v: u16, n: u32) -> u16 {
    let r = v << n;
    let out = (v >> (16 - n)) & 1 != 0;
    set_shift_flags(c, r as u32, 0x8000, out);
    r
}
fn lsr_w(c: &mut Cpu, v: u16, n: u32) -> u16 {
    let r = v >> n;
    let out = (v >> (n - 1)) & 1 != 0;
    set_shift_flags(c, r as u32, 0x8000, out);
    r
}
fn lsl_b(c: &mut Cpu, v: u8, n: u32) -> u8 {
    let r = v << n;
    let out = (v >> (8 - n)) & 1 != 0;
    set_shift_flags(c, r as u32, 0x80, out);
    r
}
fn set_shift_flags(c: &mut Cpu, r: u32, msb: u32, out: bool) {
    let mut f = 0;
    if r == 0 {
        f |= SR_Z;
    }
    if r & msb != 0 {
        f |= SR_N;
    }
    if out {
        f |= SR_C | SR_X;
    }
    c.sr = (c.sr & !(SR_N | SR_Z | SR_V | SR_C | SR_X)) | f;
}
/// Rotation à droite d'un mot : N Z selon le résultat, V = 0, C = bit sorti, X inchangé.
fn ror_w(c: &mut Cpu, v: u16, n: u32) -> u16 {
    let r = v.rotate_right(n);
    let x = c.sr & SR_X;
    set_shift_flags(c, r as u32, 0x8000, r & 0x8000 != 0);
    c.sr = (c.sr & !SR_X) | x;
    r
}
/// Drapeaux d'un transfert ou d'une opération logique (N Z selon la valeur, V = C = 0).
fn logic(c: &mut Cpu, v: u32, sz: Size) {
    c.flags_logic(v, sz);
}

// ---------------------------------------------------------------------------------------------
// Choix d'un joueur comme cible (0x424e, 0x43ae, 0x44ee)

const PLAYERS: u32 = A5 + 0x5de;
const PLAYER_SIZE: u32 = 0x200;

/// Critère de choix parmi les joueurs ciblables.
#[derive(Clone, Copy, PartialEq)]
enum Pick {
    /// Le plus proche (distance de Manhattan en X et Y).
    Nearest,
    /// Celui qui a le moins de PV.
    LowestHp,
    /// Le plus éloigné.
    Farthest,
}

/// Variante de la règle d'éligibilité.
#[derive(Clone, Copy, PartialEq)]
enum Eligibility {
    /// 0x424e : `+0x182` non nul exclut le joueur.
    Normal,
    /// 0x43ae : `+0x182` non nul dispense du test « masqué ».
    Special,
}

/// Entrées des 6 boucles de recherche de l'original.
const LAYOUTS: [(Pick, Eligibility, u32); 6] = [
    (Pick::Nearest, Eligibility::Normal, 0x427a),
    (Pick::LowestHp, Eligibility::Normal, 0x42e0),
    (Pick::Farthest, Eligibility::Normal, 0x4338),
    (Pick::Nearest, Eligibility::Special, 0x43ca),
    (Pick::LowestHp, Eligibility::Special, 0x4430),
    (Pick::Farthest, Eligibility::Special, 0x4488),
];

/// Recherche parmi les 4 joueurs (corps de 0x427a, 0x42e0, 0x4338 et de leurs variantes 0x43ca,
/// 0x4430, 0x4488). Pose la cible dans A1 (0 si aucune), son adresse dans `+0x50` de A0, D5 = 2 si
/// trouvée, et les drapeaux de `andw d5,d5`.
fn search_players(m: &mut Mach, pick: Pick, elig: Eligibility) {
    let a0 = m.c.a[0];
    m.c.a[2] = PLAYERS;
    m.c.a[1] = 0;
    let (x, y) = (m.r16(a0 + 8), m.r16(a0 + 0x10));
    set_w(&mut m.c.d[1], x);
    set_w(&mut m.c.d[2], y);
    match pick {
        Pick::Nearest => m.c.d[0] = 0xffff_ffff,
        Pick::LowestHp => set_w(&mut m.c.d[0], 0x7fff),
        Pick::Farthest => m.c.d[0] = 0,
    }
    m.c.d[5] = 0;
    m.c.d[6] = 3;
    for _ in 0..4 {
        let p = m.c.a[2];
        let alive = m.r8(p) != 0;
        let flag182 = m.r8(p + 0x182) != 0;
        let hidden = m.r8(p + 1) & 2 != 0;
        let state_ok = (m.r8(p + 4) as i8) < 4;
        let eligible = alive
            && match elig {
                Eligibility::Normal => !flag182 && !hidden,
                Eligibility::Special => flag182 || !hidden,
            }
            && state_ok;
        if eligible {
            let better;
            match pick {
                Pick::LowestHp => {
                    let hp = m.r16(p + 0x62);
                    cmp(m.c, hp as u32, m.c.d[0] & 0xffff, Size::Word);
                    better = !m.c.cond(13); // blt : d0 < hp => ignoré
                    if better {
                        set_w(&mut m.c.d[0], hp);
                    }
                }
                _ => {
                    let px = m.r16(p + 8);
                    let mut d3 = sub_w(m.c, m.c.d[1] as u16, px);
                    let neg_x = m.c.sr & SR_N != 0;
                    if neg_x {
                        d3 = neg_w(m.c, d3);
                    }
                    let py = m.r16(p + 0x10);
                    let mut d4 = sub_w(m.c, m.c.d[2] as u16, py);
                    let neg_y = m.c.sr & SR_N != 0;
                    if neg_y {
                        d4 = neg_w(m.c, d4);
                    }
                    d3 = add_w(m.c, d4, d3);
                    set_w(&mut m.c.d[3], d3);
                    set_w(&mut m.c.d[4], d4);
                    cmp(m.c, m.c.d[0] & 0xffff, d3 as u32, Size::Word);
                    // plus proche : d3 < d0 (C) ; plus loin : d3 >= d0 (pas de C)
                    better = if pick == Pick::Nearest { m.c.cond(5) } else { m.c.cond(4) };
                    if better {
                        set_w(&mut m.c.d[0], d3);
                    }
                }
            }
            if better {
                m.c.d[5] = 2;
                m.c.a[1] = p;
            }
        }
        m.c.a[2] = p + PLAYER_SIZE;
        let d6 = (m.c.d[6] as u16).wrapping_sub(1);
        set_w(&mut m.c.d[6], d6);
    }
    let a1 = m.c.a[1];
    m.w16(a0 + 0x50, a1 as u16);
    logic(m.c, m.c.d[5] & 0xffff, Size::Word);
}

/// 0x424e / 0x43ae : choisit au hasard le critère (nearest 10/16, lowest 4/16, farthest 2/16...)
/// puis cherche la cible. Résultat : A1 = joueur (ou 0), `+0x50` de A0, Z = aucun trouvé.
fn pick_target(m: &mut Mach, entry: u32, elig: Eligibility) {
    // les tables (octets de choix puis décalages) et le saut sont ceux de l'original
    let (modes, offsets) = if entry == 0x424e { (0x426a, 0x4264) } else { (0x439e, 0x43c4) };
    m.random();
    let r = m.c.d[0] as u16 & 15;
    set_w(&mut m.c.d[0], r);
    let mode = m.pc8(modes + r as u32);
    set_b(&mut m.c.d[0], mode);
    let off = m.pc16(offsets + mode as u32);
    set_w(&mut m.c.d[1], off);
    let target = (offsets as i32 + off as i16 as i32) as u32;
    let (pick, _, _) = LAYOUTS.iter().find(|l| l.2 == target).unwrap_or_else(|| panic!("pick_target : cible {target:06x}"));
    search_players(m, *pick, elig);
}

/// 0x44ee : un joueur ciblable tiré au hasard : trois essais consécutifs (tirage + 1, + 2...)
/// parmi les 4 enregistrements. A1 = joueur trouvé (sinon 0), copié dans `+0x50` de A0.
fn random_player(m: &mut Mach) {
    let a0 = m.c.a[0];
    let alive_count = m.r8(A5 + 0x35);
    logic(m.c, alive_count as u32, Size::Byte);
    if alive_count == 0 {
        return;
    }
    m.c.a[1] = 0;
    m.random();
    m.c.d[2] = 4;
    for _ in 0..4 {
        // 0x4502 : subqw #1,d2 ; beq fin
        let d2 = sub_w(m.c, 1, m.c.d[2] as u16);
        set_w(&mut m.c.d[2], d2);
        if d2 == 0 {
            return;
        }
        let d0 = add_w(m.c, 1, m.c.d[0] as u16);
        m.c.a[2] = PLAYERS;
        let d0 = d0 & 3;
        set_w(&mut m.c.d[0], d0);
        logic(m.c, d0 as u32, Size::Word);
        set_w(&mut m.c.d[3], d0);
        let d3 = lsl_w(m.c, d0, 4);
        let d3 = lsl_w(m.c, d3, 5);
        set_w(&mut m.c.d[3], d3);
        m.c.a[2] = m.c.a[2].wrapping_add(sext(d3));
        let p = m.c.a[2];
        let alive = m.r8(p) != 0;
        let flag182 = m.r8(p + 0x182) != 0;
        let hidden = m.r8(p + 1) & 2 != 0;
        let state_ok = (m.r8(p + 4) as i8) < 4;
        let found = alive && !flag182 && !hidden && state_ok;
        if found {
            m.c.a[1] = p;
            m.w16(a0 + 0x50, p as u16);
            logic(m.c, p as u16 as u32, Size::Word);
            return;
        }
    }
}

/// 0x453c et 0x4544 : points de vie initiaux d'un ennemi, tirés d'une table (octet 2 = type,
/// +0x80 au-delà de la difficulté 5, nombre de joueurs, compteur FFFF92) et copiés en `+0x60`,
/// `+0x62` et `+0x64`.
fn initial_hp(m: &mut Mach, table: u32) {
    let a0 = m.c.a[0];
    m.c.a[4] = table;
    let difficulty = m.r8(A5 + 0x48);
    let low = difficulty <= 5;
    let kind = m.r8(a0 + 2);
    m.c.d[0] = kind as u32;
    let mut d0 = lsl_w(m.c, kind as u16, 8);
    set_w(&mut m.c.d[0], d0);
    cmp(m.c, 5, difficulty as u32, Size::Byte);
    if !low {
        d0 = add_w(m.c, 0x80, d0);
        set_w(&mut m.c.d[0], d0);
    }
    let players = m.r8(A5 + 0x35);
    set_b(&mut m.c.d[1], players);
    let d1b = sub_b(m.c, 1, players);
    set_b(&mut m.c.d[1], d1b);
    let d1 = (m.c.d[1] as u16) & 3;
    set_w(&mut m.c.d[1], d1);
    logic(m.c, d1 as u32, Size::Word);
    let d1 = lsl_w(m.c, d1, 5);
    set_w(&mut m.c.d[1], d1);
    d0 = add_w(m.c, d1, d0);
    set_w(&mut m.c.d[0], d0);
    let tick = m.r16(A5 + 0x7f92);
    set_w(&mut m.c.d[1], tick);
    let d1 = lsr_w(m.c, tick, 1);
    let d1 = d1 & 15;
    set_w(&mut m.c.d[1], d1);
    logic(m.c, d1 as u32, Size::Word);
    let d1 = add_w(m.c, d1, d1);
    set_w(&mut m.c.d[1], d1);
    d0 = add_w(m.c, d1, d0);
    let hp = m.r16(table.wrapping_add(sext(d0)));
    set_w(&mut m.c.d[0], hp);
    m.w16(a0 + 0x60, hp);
    m.w16(a0 + 0x62, hp);
    m.w16(a0 + 0x64, hp);
    logic(m.c, hp as u32, Size::Word);
}

/// 0xa4b4 et 0xa4c2 : « l'IA agit-elle cette image ? ». Un octet de l'ennemi (`+0x72` ou `+0x73`)
/// choisit une table de masques de 32 bits ; le compteur FFFF92 choisit le masque ; un tirage
/// choisit le bit. D0 = 1 si le bit est à 1, sinon 0 (drapeau Z).
fn decision(m: &mut Mach, field: u32) {
    let a0 = m.c.a[0];
    let tables = if field == 0x72 { 0xa4f2 } else { 0xa516 };
    let sel = m.r8(a0 + field);
    m.c.d[0] = sel as u32;
    let d0 = lsl_w(m.c, sel as u16, 2);
    set_w(&mut m.c.d[0], d0);
    m.c.a[4] = tables;
    let masks = rd32(m.b, tables.wrapping_add(sext(d0)));
    m.c.a[4] = masks;
    let tick = m.r16(A5 + 0x7f92);
    set_w(&mut m.c.d[0], tick);
    let d0 = lsl_w(m.c, tick, 2);
    set_w(&mut m.c.d[0], d0);
    m.c.d[2] = rd32(m.b, masks.wrapping_add(sext(d0)));
    m.random();
    let bit = m.c.d[0] as u16 & 31;
    set_w(&mut m.c.d[0], bit);
    logic(m.c, bit as u32, Size::Word);
    let clear = m.c.d[2] >> bit & 1 == 0;
    m.c.sr = (m.c.sr & !SR_Z) | if clear { SR_Z } else { 0 };
    m.c.d[0] = !clear as u32;
    logic(m.c, m.c.d[0], Size::Long);
}

/// 0xac3a : tire dans une table (choisie par `+0x76`, indexée par FFFF92) l'octet `+0x77`.
fn pick_byte_by_tick(m: &mut Mach) {
    let a0 = m.c.a[0];
    let sel = m.r8(a0 + 0x76);
    m.c.d[0] = sel as u32;
    let d0 = lsl_w(m.c, sel as u16, 2);
    set_w(&mut m.c.d[0], d0);
    m.c.a[4] = 0xac58;
    let table = rd32(m.b, 0xac58u32.wrapping_add(sext(d0)));
    m.c.a[4] = table;
    let tick = m.r16(A5 + 0x7f92);
    m.c.d[0] = tick as u32;
    let v = m.r8(table.wrapping_add(sext(tick)));
    m.w8(a0 + 0x77, v);
    logic(m.c, v as u32, Size::Byte);
}

/// 0xfe5a : immunités d'un ennemi (`+0x94`, long) tirées d'une table selon sa classe (+0x20 pour
/// un boss) et son type.
fn set_immunities(m: &mut Mach) {
    let a0 = m.c.a[0];
    let boss = m.r8(a0 + 0x28) == 4;
    m.c.d[0] = 0;
    if boss {
        m.c.d[0] = 32;
    }
    let kind = m.r8(a0 + 2);
    let d0 = add_b(m.c, kind, m.c.d[0] as u8);
    set_b(&mut m.c.d[0], d0);
    let d0 = add_w(m.c, m.c.d[0] as u16, m.c.d[0] as u16);
    set_w(&mut m.c.d[0], d0);
    let d0 = add_w(m.c, m.c.d[0] as u16, m.c.d[0] as u16);
    set_w(&mut m.c.d[0], d0);
    let v = m.pc32(0xfe78u32.wrapping_add(sext(d0)));
    m.w32(a0 + 0x94, v);
    logic(m.c, v, Size::Long);
}

/// 0x1059e : `+0xac` = (joueurs en vie - 1) * 32 + (compteur FFFF92 & 30).
fn spread_index(m: &mut Mach) {
    let a0 = m.c.a[0];
    let alive = m.r8(A5 + 0x35);
    set_b(&mut m.c.d[0], alive);
    let d0 = sub_b(m.c, 1, alive);
    let d0 = lsl_b(m.c, d0, 5);
    set_b(&mut m.c.d[0], d0);
    let tick = m.r16(A5 + 0x7f92);
    set_w(&mut m.c.d[1], tick);
    let d1 = tick as u8 & 30;
    set_b(&mut m.c.d[1], d1);
    logic(m.c, d1 as u32, Size::Byte);
    let d0 = add_b(m.c, d1, d0);
    set_b(&mut m.c.d[0], d0);
    m.w8(a0 + 0xac, d0);
    logic(m.c, d0 as u32, Size::Byte);
}

/// 0xb5e0 : la cible (`+0x50`) est-elle dans la boîte devant l'ennemi ? D0 = décalage X de la
/// boîte, D1 = demi-largeur, D2 = demi-profondeur. Résultat D0 : 0 hors boîte (ou cible en l'air),
/// 1 cible du côté vers lequel l'ennemi regarde, 2 de l'autre côté.
fn target_in_front(m: &mut Mach) {
    let a0 = m.c.a[0];
    let mut result = 0u32;
    let tw = m.r16(a0 + 0x50);
    logic(m.c, tw as u32, Size::Word);
    'body: {
        if tw == 0 {
            break 'body;
        }
        let t = sext(tw);
        m.c.a[1] = t;
        let z = m.r16(t + 0xc);
        logic(m.c, z as u32, Size::Word);
        if z != 0 {
            break 'body;
        }
        let flip = m.r8(a0 + 0x2c);
        logic(m.c, flip as u32, Size::Byte);
        let flipped = flip != 0;
        let mut d0 = m.c.d[0] as u16;
        if flipped {
            d0 = neg_w(m.c, d0);
        }
        let ax = m.r16(a0 + 8);
        d0 = add_w(m.c, ax, d0);
        let tx = m.r16(t + 8);
        set_w(&mut m.c.d[3], tx);
        logic(m.c, tx as u32, Size::Word);
        d0 = sub_w(m.c, tx, d0);
        let mut d1 = m.c.d[1] as u16;
        d0 = add_w(m.c, d1, d0);
        d1 = add_w(m.c, d1, d1);
        set_w(&mut m.c.d[0], d0);
        set_w(&mut m.c.d[1], d1);
        cmp(m.c, d1 as u32, d0 as u32, Size::Word);
        if m.c.cond(2) {
            break 'body;
        }
        let ay = m.r16(a0 + 0x10);
        let ty = m.r16(t + 0x10);
        set_w(&mut m.c.d[0], ay);
        logic(m.c, ay as u32, Size::Word);
        set_w(&mut m.c.d[1], ty);
        logic(m.c, ty as u32, Size::Word);
        let mut d0 = sub_w(m.c, ty, ay);
        let mut d2 = m.c.d[2] as u16;
        d0 = add_w(m.c, d2, d0);
        d2 = add_w(m.c, d2, d2);
        set_w(&mut m.c.d[0], d0);
        set_w(&mut m.c.d[2], d2);
        cmp(m.c, d2 as u32, d0 as u32, Size::Word);
        if m.c.cond(2) {
            break 'body;
        }
        set_b(&mut m.c.d[2], 32);
        logic(m.c, 32, Size::Byte);
        set_w(&mut m.c.d[1], ax);
        logic(m.c, ax as u32, Size::Word);
        let d1 = sub_w(m.c, tx, ax);
        set_w(&mut m.c.d[1], d1);
        let left = m.c.sr & SR_N != 0;
        if !left {
            set_b(&mut m.c.d[2], 0);
            logic(m.c, 0, Size::Byte);
        }
        cmp(m.c, flip as u32, m.c.d[2] & 0xff, Size::Byte);
        let behind = m.c.d[2] as u8 != flip;
        result = if behind { 2 } else { 1 };
    }
    m.c.d[0] = result;
    logic(m.c, result, Size::Long);
}

/// 0x1046c : recopie `+0x3e` en `+0x44` (identifiant de l'attaque en cours) ; N Z selon l'octet.
fn copy_attack_id(m: &mut Mach) {
    let a0 = m.c.a[0];
    let v = m.r8(a0 + 0x3e);
    m.w8(a0 + 0x44, v);
    logic(m.c, v as u32, Size::Byte);
}

/// 0x103d6 : charge une entrée d'animation (0x1a32, O1) puis mémorise l'identifiant d'attaque.
fn start_animation(m: &mut Mach) {
    m.ext(0x1a32);
    copy_attack_id(m);
}

/// 0x1040c : un pas d'animation (0x1a56, O1) puis mémorise l'identifiant d'attaque ; C = fin de
/// l'animation (retenue du pas), les autres drapeaux sont effacés.
fn step_animation(m: &mut Mach) {
    m.ext(0x1a56);
    let carry = m.c.sr & SR_C != 0;
    copy_attack_id(m);
    m.c.set_ccr(carry as u16);
}

/// 0x1050c : la cible est-elle du côté vers lequel l'ennemi regarde ? Z = oui (D0 = 0 ou 0x20).
fn facing_target(m: &mut Mach) {
    let a0 = m.c.a[0];
    let tw = m.r16(a0 + 0x50);
    let flip = m.r8(a0 + 0x2c);
    logic(m.c, tw as u32, Size::Word);
    if (tw as i16) >= 0 {
        m.c.d[0] = 0;
        logic(m.c, 0, Size::Long);
        return;
    }
    let target = sext(tw);
    m.c.d[0] = 0;
    m.c.a[1] = target;
    let ax = m.r16(a0 + 8);
    let tx = m.r16(target + 8);
    set_w(&mut m.c.d[1], ax);
    let d1 = sub_w(m.c, tx, ax);
    set_w(&mut m.c.d[1], d1);
    let neg = m.c.sr & SR_N != 0;
    if neg {
        m.c.d[0] = 32;
    }
    cmp(m.c, flip as u32, m.c.d[0] & 0xff, Size::Byte);
}

/// 0x10560 : compare le signe de la vitesse X (`+0x14`) à l'orientation de l'ennemi.
fn moving_forward(m: &mut Mach) {
    let a0 = m.c.a[0];
    let flip = m.r8(a0 + 0x2c);
    m.c.d[1] = flip as u32;
    let d1 = ror_w(m.c, flip as u16, 6);
    set_w(&mut m.c.d[1], d1);
    let vx = m.r16(a0 + 0x14);
    set_w(&mut m.c.d[2], vx);
    let d2 = vx & 0x8000;
    set_w(&mut m.c.d[2], d2);
    cmp(m.c, d1 as u32, d2 as u32, Size::Word);
}

// ---------------------------------------------------------------------------------------------
// Routines natives

impl Mach<'_> {
    /// Appelle une routine d'origine (non réécrite).
    fn ext(&mut self, addr: u32) {
        self.c.a[5] = A5;
        call(self.c, self.b, addr);
    }
}

macro_rules! native {
    ($name:ident, $entry:expr, $label:expr, $logic:expr) => {
        pub const $name: Native = Native {
            entry: $entry,
            name: $label,
            run: |c, b| {
                let mut m = Mach { c, b };
                $logic(&mut m);
                rts(c, b);
            },
            touches_sound: false,
            outputs: regs::ALL,
        };
    };
}

native!(PICK_TARGET, 0x424e, "ai_pick_target", |m: &mut Mach| pick_target(m, 0x424e, Eligibility::Normal));
native!(PICK_TARGET_ANY, 0x43ae, "ai_pick_target_any", |m: &mut Mach| pick_target(m, 0x43ae, Eligibility::Special));
native!(RANDOM_PLAYER, 0x44ee, "ai_random_player", random_player);
native!(INITIAL_HP_A, 0x453c, "ai_initial_hp_a", |m: &mut Mach| initial_hp(m, 0x458c));
native!(INITIAL_HP_B, 0x4544, "ai_initial_hp_b", |m: &mut Mach| initial_hp(m, 0x5c8c));
native!(DECISION_A, 0xa4b4, "ai_decision_a", |m: &mut Mach| decision(m, 0x72));
native!(DECISION_B, 0xa4c2, "ai_decision_b", |m: &mut Mach| decision(m, 0x73));
native!(PICK_BYTE, 0xac3a, "ai_pick_byte", pick_byte_by_tick);
native!(IMMUNITIES, 0xfe5a, "ai_set_immunities", set_immunities);
native!(SPREAD_INDEX, 0x1059e, "ai_spread_index", spread_index);
native!(TARGET_IN_FRONT, 0xb5e0, "ai_target_in_front", target_in_front);
native!(START_ANIMATION, 0x103d6, "ai_start_animation", start_animation);
native!(STEP_ANIMATION, 0x1040c, "ai_step_animation", step_animation);
native!(FACING_TARGET, 0x1050c, "ai_facing_target", facing_target);
native!(MOVING_FORWARD, 0x10560, "ai_moving_forward", moving_forward);

// ---------------------------------------------------------------------------------------------
// Rester dans l'écran (0xfbae, 0xfbb0) et s'approcher d'un point d'une cible (0xe76c, 0xf0e8)
//
// Transcrites instruction par instruction (opérations à drapeaux exacts) : les handlers qui suivent
// relisent D0-D6, A1, A2 et les drapeaux.
// ---------------------------------------------------------------------------------------------

/// Scroll courant (x, y) de la caméra 1 et de la caméra 2, octet « caméra 2 courante ».
const SCROLL_1: (u32, u32) = (A5 + 0x506, A5 + 0x50a);
const SCROLL_2: (u32, u32) = (A5 + 0x556, A5 + 0x55a);
const USE_CAMERA_2: u32 = A5 + 0x36;
/// Champs de l'objet : abscisse, profondeur.
const OBJ_X: u32 = 8;
const OBJ_Y: u32 = 0x10;
/// Marge par défaut de `in_zone` (0xfbae), en pixels.
const ZONE_MARGIN: u32 = 0x30;

/// `move.w src,Dn` (drapeaux N Z).
fn move_w(c: &mut Cpu, n: usize, v: u16) {
    set_w(&mut c.d[n], v);
    logic(c, v as u32, Size::Word);
}

/// Ramène un axe de l'objet A0 dans la fenêtre de l'écran : `off` est le champ de l'objet,
/// `origin` le registre qui porte le scroll de l'axe, `span` la taille de la fenêtre. La position
/// négative est remise à 0 ; hors de `[scroll - D0, scroll + span + D0]` (D0 = marge), elle est
/// ramenée au bord. D5 = 1 si l'objet a été déplacé.
fn pull_inside(c: &mut Cpu, b: &mut Cps2Bus, off: u32, origin: usize, span: u16) {
    let a0 = c.a[0];
    let pos = rd16(b, a0 + off);
    move_w(c, 2, pos);
    if (pos as i16) < 0 {
        wr16(b, a0 + off, 0);
        c.d[5] = 1;
        logic(c, 1, Size::Long);
        return;
    }
    let margin = c.d[0] as u16;
    let d2 = sub_w(c, c.d[origin] as u16, pos);
    let d2 = add_w(c, margin, d2);
    set_w(&mut c.d[2], d2);
    move_w(c, 4, span);
    let d4 = add_w(c, margin, span);
    let d4 = add_w(c, margin, d4);
    set_w(&mut c.d[4], d4);
    cmp(c, d4 as u32, d2 as u32, Size::Word);
    if c.cond(cc::LS) {
        return;
    }
    move_w(c, 3, margin);
    let d3 = neg_w(c, margin);
    set_w(&mut c.d[3], d3);
    logic(c, d2 as u32, Size::Word);
    let d3 = if d2 as i16 >= 0 {
        move_w(c, 3, span);
        add_w(c, margin, span)
    } else {
        d3
    };
    set_w(&mut c.d[3], d3);
    let o = add_w(c, d3, c.d[origin] as u16);
    set_w(&mut c.d[origin], o);
    wr16(b, a0 + off, o);
    logic(c, o as u32, Size::Word);
    c.d[5] = 1;
    logic(c, 1, Size::Long);
}

/// 0xfbb0 : garde l'objet A0 dans l'écran (D0 = marge en pixels). D5 = 1 s'il a bougé.
fn keep_on_screen(c: &mut Cpu, b: &mut Cps2Bus) {
    let (sx, sy) = if rd8(b, USE_CAMERA_2) != 0 { SCROLL_2 } else { SCROLL_1 };
    let (x, y) = (rd16(b, sx), rd16(b, sy));
    move_w(c, 1, x);
    move_w(c, 6, y);
    c.d[5] = 0;
    pull_inside(c, b, OBJ_X, 1, 0x180);
    c.d[0] = (-0x10i32) as u32; // moveq #-16,D0 : marge verticale
    pull_inside(c, b, OBJ_Y, 6, 0x100);
    rts(c, b);
}

/// 0xfbae : comme 0xfbb0 avec la marge de 0x30 pixels (c'est le test de zone, qui remet D5 à 0).
fn in_zone(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[0] = ZONE_MARGIN;
    keep_on_screen(c, b);
}

pub const IN_ZONE: Native = Native {
    entry: 0xfbae,
    name: "ai_zone",
    run: in_zone,
    touches_sound: false,
    outputs: regs::ALL,
};

pub const KEEP_ON_SCREEN: Native = Native {
    entry: 0xfbb0,
    name: "ai_garde_a_l_ecran",
    run: keep_on_screen,
    touches_sound: false,
    outputs: regs::ALL,
};

/// Offsets de l'ennemi A0 : cible (mot), orientation (octet, 0-31), type de trajectoire, point visé.
const OBJ_TARGET: u32 = 0x50;
const OBJ_FACING: u32 = 0x2d;
const OBJ_PATH: u32 = 0x79;
const OBJ_AIM_X: u32 = 0x7a;
const OBJ_AIM_Y: u32 = 0x7c;
/// Table (en ROM) des tables de points d'approche par type : mot n, puis n + 1 couples (dx, dy).
const APPROACH_TABLES: u32 = 0xe93e;

/// 0xf0e8 : tourne d'un cran l'orientation de A0 vers le point (D1, D2) (angle de 0x137e).
fn turn_toward(c: &mut Cpu, b: &mut Cps2Bus) {
    call(c, b, 0x137e);
    let a0 = c.a[0];
    let facing = rd8(b, a0 + OBJ_FACING);
    set_b(&mut c.d[1], facing);
    let diff = sub_b(c, facing, c.d[0] as u8);
    set_b(&mut c.d[0], diff);
    let d0 = (c.d[0] as u16) & 0x1f;
    set_w(&mut c.d[0], d0);
    logic(c, d0 as u32, Size::Word);
    if d0 == 0 {
        return rts(c, b);
    }
    // btst #4,D0 : le plus court est dans le sens positif ou négatif
    let turned = if d0 & 0x10 == 0 { add_b(c, 1, facing) } else { sub_b(c, 1, facing) };
    set_b(&mut c.d[1], turned);
    let d1 = (c.d[1] as u16) & 0x1f;
    set_w(&mut c.d[1], d1);
    logic(c, d1 as u32, Size::Word);
    wr8(b, a0 + OBJ_FACING, d1 as u8);
    logic(c, d1 as u32 & 0xff, Size::Byte);
    rts(c, b);
}

/// 0xe76c : choisit parmi les points d'approche de la cible (`OBJ_TARGET`) celui qui est le plus
/// proche de l'ennemi A0 (de loin : le plus éloigné), le mémorise en `OBJ_AIM_X/Y` et tourne vers lui.
fn approach_target(c: &mut Cpu, b: &mut Cps2Bus) {
    let a0 = c.a[0];
    let target = rd16(b, a0 + OBJ_TARGET);
    logic(c, target as u32, Size::Word);
    if target == 0 {
        return rts(c, b);
    }
    let x = rd16(b, a0 + OBJ_X);
    move_w(c, 0, x);
    let scroll = if rd8(b, USE_CAMERA_2) != 0 { SCROLL_2.0 } else { SCROLL_1.0 };
    let scroll_x = rd16(b, scroll);
    move_w(c, 1, scroll_x);
    let d0 = sub_w(c, scroll_x, x);
    set_w(&mut c.d[0], d0);
    cmp(c, 0x180, d0 as u32, Size::Word);
    // loin de l'écran (D0 > 0x180) : on vise le point le plus éloigné, sinon le plus proche
    let far = c.cond(cc::HI);
    c.a[1] = sext(target);
    let a1 = c.a[1];
    c.d[0] = 0;
    set_b(&mut c.d[0], rd8(b, a0 + OBJ_PATH));
    let index = lsl_w(c, c.d[0] as u16, 2);
    set_w(&mut c.d[0], index);
    c.a[2] = rd32(b, APPROACH_TABLES + sext(index));
    let count = rd16(b, c.a[2]);
    c.a[2] += 2;
    move_w(c, 6, count);
    let (tx, ty) = (rd16(b, a1 + OBJ_X), rd16(b, a1 + OBJ_Y));
    wr16(b, a0 + OBJ_AIM_X, tx);
    wr16(b, a0 + OBJ_AIM_Y, ty);
    if far {
        c.d[4] = 0;
    } else {
        move_w(c, 4, 0x7fff);
    }
    loop {
        let (dx, dy) = (rd16(b, c.a[2]), rd16(b, c.a[2] + 2));
        c.a[2] += 4;
        move_w(c, 0, dx);
        move_w(c, 1, dy);
        let px = add_w(c, tx, dx);
        set_w(&mut c.d[0], px);
        let py = add_w(c, ty, dy);
        set_w(&mut c.d[1], py);
        move_w(c, 2, px);
        move_w(c, 3, py);
        let mut d2 = sub_w(c, rd16(b, a0 + OBJ_X), px);
        if c.cond(cc::MI) {
            d2 = neg_w(c, d2);
        }
        set_w(&mut c.d[2], d2);
        let mut d3 = sub_w(c, rd16(b, a0 + OBJ_Y), py);
        if c.cond(cc::MI) {
            d3 = neg_w(c, d3);
        }
        set_w(&mut c.d[3], d3);
        let d2 = add_w(c, d3, d2);
        set_w(&mut c.d[2], d2);
        cmp(c, c.d[4] as u16 as u32, d2 as u32, Size::Word);
        if c.cond(if far { cc::HI } else { cc::LS }) {
            move_w(c, 4, d2);
            wr16(b, a0 + OBJ_AIM_X, px);
            logic(c, px as u32, Size::Word);
            wr16(b, a0 + OBJ_AIM_Y, py);
            logic(c, py as u32, Size::Word);
        }
        let left = (c.d[6] as u16).wrapping_sub(1);
        set_w(&mut c.d[6], left);
        if left == 0xffff {
            break;
        }
    }
    let (ax, ay) = (rd16(b, a0 + OBJ_AIM_X), rd16(b, a0 + OBJ_AIM_Y));
    move_w(c, 1, ax);
    move_w(c, 2, ay);
    turn_toward(c, b);
}

pub const APPROACH_TARGET: Native = Native {
    entry: 0xe76c,
    name: "ai_approche_cible",
    run: approach_target,
    touches_sound: false,
    outputs: regs::ALL,
};

pub const TURN_TOWARD: Native = Native {
    entry: 0xf0e8,
    name: "ai_tourne_vers",
    run: turn_toward,
    touches_sound: false,
    outputs: regs::ALL,
};
