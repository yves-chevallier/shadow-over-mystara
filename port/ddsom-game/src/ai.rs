//! Aides d'IA communes aux ennemis et aux boss (sous-système E1) : choix d'un joueur comme cible,
//! tests de portée et de direction, décisions pseudo-aléatoires par tables, valeurs initiales.
//!
//! Ces routines sont appelées depuis des centaines de handlers d'ennemis ; elles gardent donc le
//! contrat exact de l'original (registres, drapeaux, durée). Leur logique est écrite une seule fois
//! (`fn xxx(m: &mut Mach)`) et exécutée de deux façons : pour de vrai (`Mach::dry == false`) ou à
//! blanc (`dry == true`), sans écriture ni appel, pour calculer la durée exacte avant exécution.
//! Le générateur pseudo-aléatoire est prédit à blanc (`Mach::random`), donc la durée l'est aussi.
//!
//! Joueurs : 4 enregistrements de 0x200 octets à A5+0x5de ; un joueur est « ciblable » s'il est
//! actif (`+0`), non masqué (bit 1 de `+1`), d'état principal < 4 (`+4`) et `+0x182` nul.

use std::cell::RefCell;
use std::collections::HashMap;

use cps2::Cps2Bus;
use m68k::decode::{BitOp, Fetch, Op, ShiftCount, Size};
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8, A5};
use crate::{call, regs, Native};

// ---------------------------------------------------------------------------------------------
// Durée des segments de code d'origine

thread_local! {
    static SPANS: RefCell<HashMap<(u32, u32, u64), u64>> = RefCell::new(HashMap::new());
}

/// Fin de routine : le segment s'arrête après le RTS.
const END: u32 = u32::MAX;

/// Durée en cycles des instructions de `from` jusqu'à `stop` (exclu), ou jusqu'au RTS (inclus).
/// `taken(pc)` décide des branchements conditionnels ; `key` identifie le jeu de décisions pour le
/// cache. Les `jsr` et `bsr` sont comptés sans leur sous-routine : celle-ci est soit exécutée par
/// `crate::call` (qui compte ses cycles), soit réécrite en Rust (qui ajoute sa propre durée).
fn span(b: &mut Cps2Bus, from: u32, stop: u32, key: u64, taken: impl Fn(u32) -> bool) -> u64 {
    if let Some(v) = SPANS.with(|c| c.borrow().get(&(from, stop, key)).copied()) {
        return v;
    }
    let mut pc = from;
    let mut cyc = 0u64;
    let mut first = true;
    for _ in 0..10_000 {
        if pc == stop && !first {
            break;
        }
        first = false;
        let ins = m68k::decode(b, pc);
        cyc += m68k::timing::base_cycles(&ins) as u64;
        pc = pc.wrapping_add(ins.len);
        match ins.op {
            Op::Rts => break,
            Op::Bra(t) => pc = t,
            Op::Jsr(_) | Op::Bsr(_) => {}
            Op::Bcc(_, t) => {
                if taken(ins.pc) {
                    cyc += 2;
                    pc = t;
                } else if ins.opcode & 0xff == 0 {
                    cyc += 4;
                }
            }
            Op::Shift { count: ShiftCount::Imm(n), .. } => cyc += 2 * n as u64,
            Op::Bit(kind, count, d)
                if kind == BitOp::Btst || matches!(count, ShiftCount::Imm(_)) || !matches!(d, m68k::decode::Ea::Dn(_)) => {}
            Op::Shift { .. } | Op::Dbcc(..) | Op::Movem { .. } | Op::Mulu(..) | Op::Muls(..) | Op::Divu(..)
            | Op::Divs(..) | Op::Jmp(_) | Op::Bit(..) | Op::Scc(..) => {
                panic!("span : durée dépendant des données non gérée en {:06x} ({:?})", ins.pc, ins.op)
            }
            _ => {}
        }
    }
    SPANS.with(|c| c.borrow_mut().insert((from, stop, key), cyc));
    cyc
}

/// Durée de l'instruction isolée à `pc` (hors dépendances aux données).
fn insn(b: &mut Cps2Bus, pc: u32) -> u64 {
    let ins = m68k::decode(b, pc);
    m68k::timing::base_cycles(&ins) as u64
}

// ---------------------------------------------------------------------------------------------
// Machine : exécution réelle ou à blanc

/// Exécution d'une routine : `dry` = à blanc (registres copiés, mémoire intacte, tirage prédit).
struct Mach<'a> {
    c: &'a mut Cpu,
    b: &'a mut Cps2Bus,
    dry: bool,
    /// Durée cumulée des segments exécutés (hors sous-routines appelées).
    cyc: u64,
    /// État prédit du générateur pseudo-aléatoire (à blanc).
    rng: Option<u16>,
}

impl Mach<'_> {
    fn r8(&mut self, a: u32) -> u8 {
        rd8(self.b, a)
    }
    fn r16(&mut self, a: u32) -> u16 {
        rd16(self.b, a)
    }
    fn w8(&mut self, a: u32, v: u8) {
        if !self.dry {
            wr8(self.b, a, v)
        }
    }
    fn w16(&mut self, a: u32, v: u16) {
        if !self.dry {
            wr16(self.b, a, v)
        }
    }
    fn w32(&mut self, a: u32, v: u32) {
        if !self.dry {
            wr32(self.b, a, v)
        }
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
    /// Durée d'un segment de l'original.
    fn seg(&mut self, from: u32, stop: u32, key: u64, taken: impl Fn(u32) -> bool) {
        self.cyc += span(self.b, from, stop, key, taken);
    }
    fn seg1(&mut self, from: u32, stop: u32) {
        self.seg(from, stop, 0, |_| false);
    }
    /// 0x19a88 : tirage pseudo-aléatoire (D0 reçoit le nouvel état). À blanc : prédit.
    fn random(&mut self) {
        if self.dry {
            let s = match self.rng {
                Some(s) => s,
                None => rd16(self.b, A5 + 0x1a),
            };
            let d0 = (s.wrapping_add(s).wrapping_add(s)) >> 8;
            let n = (d0 << 8) | (s as u8).wrapping_add(d0 as u8) as u16;
            self.rng = Some(n);
            set_w(&mut self.c.d[0], n);
        } else {
            self.c.a[5] = A5;
            call(self.c, self.b, 0x19a88);
        }
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

/// Adresses des instructions de décision des 6 boucles de recherche de l'original.
struct Layout {
    entry: u32,
    loop_start: u32,
    dbf: u32,
    tail: u32,
    alive: u32,
    flag182: u32,
    hidden: u32,
    state: u32,
    better: u32,
}

const LAYOUTS: [(Pick, Eligibility, Layout); 6] = [
    (Pick::Nearest, Eligibility::Normal, Layout { entry: 0x427a, loop_start: 0x4290, dbf: 0x42d4, tail: 0x42d8, alive: 0x4292, flag182: 0x4298, hidden: 0x42a2, state: 0x42ac, better: 0x42c8 }),
    (Pick::LowestHp, Eligibility::Normal, Layout { entry: 0x42e0, loop_start: 0x42f8, dbf: 0x432c, tail: 0x4330, alive: 0x42fa, flag182: 0x4300, hidden: 0x430a, state: 0x4314, better: 0x431c }),
    (Pick::Farthest, Eligibility::Normal, Layout { entry: 0x4338, loop_start: 0x434e, dbf: 0x4392, tail: 0x4396, alive: 0x4350, flag182: 0x4356, hidden: 0x4360, state: 0x436a, better: 0x4386 }),
    (Pick::Nearest, Eligibility::Special, Layout { entry: 0x43ca, loop_start: 0x43e0, dbf: 0x4424, tail: 0x4428, alive: 0x43e2, flag182: 0x43e8, hidden: 0x43f2, state: 0x43fc, better: 0x4418 }),
    (Pick::LowestHp, Eligibility::Special, Layout { entry: 0x4430, loop_start: 0x4448, dbf: 0x447c, tail: 0x4480, alive: 0x444a, flag182: 0x4450, hidden: 0x445a, state: 0x4464, better: 0x446c }),
    (Pick::Farthest, Eligibility::Special, Layout { entry: 0x4488, loop_start: 0x449e, dbf: 0x44e2, tail: 0x44e6, alive: 0x44a0, flag182: 0x44a6, hidden: 0x44b0, state: 0x44ba, better: 0x44d4 }),
];

/// Recherche parmi les 4 joueurs (corps de 0x427a, 0x42e0, 0x4338 et de leurs variantes 0x43ca,
/// 0x4430, 0x4488). Pose la cible dans A1 (0 si aucune), son adresse dans `+0x50` de A0, D5 = 2 si
/// trouvée, et les drapeaux de `andw d5,d5`.
fn search_players(m: &mut Mach, pick: Pick, elig: Eligibility) {
    let lay = &LAYOUTS.iter().find(|l| l.0 == pick && l.1 == elig).unwrap().2;
    let a0 = m.c.a[0];
    m.seg1(lay.entry, lay.loop_start);
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
    for i in 0..4 {
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
        let mut better = false;
        let (mut neg_x, mut neg_y) = (false, false);
        if eligible {
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
                    neg_x = m.c.sr & SR_N != 0;
                    if neg_x {
                        d3 = neg_w(m.c, d3);
                    }
                    let py = m.r16(p + 0x10);
                    let mut d4 = sub_w(m.c, m.c.d[2] as u16, py);
                    neg_y = m.c.sr & SR_N != 0;
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
        let key = (alive as u64) | (flag182 as u64) << 1 | (hidden as u64) << 2 | ((!state_ok) as u64) << 3 | (better as u64) << 4 | (neg_x as u64) << 5 | (neg_y as u64) << 6;
        let abs_branches = std::cell::Cell::new(0);
        m.seg(lay.loop_start, lay.dbf, key, |pc| {
            if pc == lay.alive {
                !alive
            } else if pc == lay.flag182 {
                flag182
            } else if pc == lay.hidden {
                hidden
            } else if pc == lay.state {
                !state_ok
            } else if pc == lay.better {
                !better
            } else {
                // les deux valeurs absolues : bpl saute le neg quand le signe est positif
                let n = abs_branches.get();
                abs_branches.set(n + 1);
                if n == 0 { !neg_x } else { !neg_y }
            }
        });
        m.cyc += if i < 3 { 10 } else { 14 };
        m.c.a[2] = p + PLAYER_SIZE;
        let d6 = (m.c.d[6] as u16).wrapping_sub(1);
        set_w(&mut m.c.d[6], d6);
    }
    m.seg1(lay.tail, END);
    let a1 = m.c.a[1];
    m.w16(a0 + 0x50, a1 as u16);
    logic(m.c, m.c.d[5] & 0xffff, Size::Word);
}

/// 0x424e / 0x43ae : choisit au hasard le critère (nearest 10/16, lowest 4/16, farthest 2/16...)
/// puis cherche la cible. Résultat : A1 = joueur (ou 0), `+0x50` de A0, Z = aucun trouvé.
fn pick_target(m: &mut Mach, entry: u32, elig: Eligibility) {
    // les tables (octets de choix puis décalages) et le saut sont ceux de l'original
    let (modes, offsets) = if entry == 0x424e { (0x426a, 0x4264) } else { (0x439e, 0x43c4) };
    m.seg(entry, if entry == 0x424e { 0x4260 } else { 0x43bc + 4 }, 0, |_| false);
    m.random();
    let r = m.c.d[0] as u16 & 15;
    set_w(&mut m.c.d[0], r);
    let mode = m.pc8(modes + r as u32);
    set_b(&mut m.c.d[0], mode);
    let off = m.pc16(offsets + mode as u32);
    set_w(&mut m.c.d[1], off);
    let jmp = if entry == 0x424e { 0x4260 } else { 0x43c0 };
    m.cyc += insn(m.b, jmp);
    let target = (offsets as i32 + off as i16 as i32) as u32;
    let (pick, _, _) = LAYOUTS.iter().find(|l| l.2.entry == target).unwrap_or_else(|| panic!("pick_target : cible {target:06x}"));
    search_players(m, *pick, elig);
}

/// 0x44ee : un joueur ciblable tiré au hasard : trois essais consécutifs (tirage + 1, + 2...)
/// parmi les 4 enregistrements. A1 = joueur trouvé (sinon 0), copié dans `+0x50` de A0.
fn random_player(m: &mut Mach) {
    let a0 = m.c.a[0];
    let alive_count = m.r8(A5 + 0x35);
    logic(m.c, alive_count as u32, Size::Byte);
    if alive_count == 0 {
        m.seg(0x44ee, END, 1, |pc| pc == 0x44f2);
        return;
    }
    m.seg(0x44ee, 0x4502, 0, |_| false);
    m.c.a[1] = 0;
    m.random();
    m.c.d[2] = 4;
    for _ in 0..4 {
        // 0x4502 : subqw #1,d2 ; beq fin
        let d2 = sub_w(m.c, 1, m.c.d[2] as u16);
        set_w(&mut m.c.d[2], d2);
        if d2 == 0 {
            m.seg(0x4502, END, 0x10, |pc| pc == 0x4504);
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
        let key = 0x20 | (alive as u64) | (flag182 as u64) << 1 | (hidden as u64) << 2 | ((!state_ok) as u64) << 3;
        m.seg(0x4502, 0x4502, key, |pc| match pc {
            0x4504 => false,
            0x451c => !alive,
            0x4522 => flag182,
            0x452a => hidden,
            0x4532 => !state_ok,
            _ => panic!("random_player : branchement inattendu en {pc:06x}"),
        });
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
    m.seg(if table == 0x458c { 0x453c } else { 0x4544 }, END, low as u64, |pc| pc == 0x4556 && low);
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
    let (entry, tables) = if field == 0x72 { (0xa4b4, 0xa4f2) } else { (0xa4c2, 0xa516) };
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
    m.seg(entry, END, clear as u64, |pc| pc == 0xa4e8 && clear);
    m.c.d[0] = !clear as u32;
    logic(m.c, m.c.d[0], Size::Long);
}

/// 0xac3a : tire dans une table (choisie par `+0x76`, indexée par FFFF92) l'octet `+0x77`.
fn pick_byte_by_tick(m: &mut Mach) {
    let a0 = m.c.a[0];
    m.seg1(0xac3a, END);
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
    m.seg(0xfe5a, END, boss as u64, |pc| pc == 0xfe62 && !boss);
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
    m.seg1(0x1059e, END);
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

/// Décisions prises par `target_in_front` (pour en déduire la durée).
#[derive(Default)]
struct FrontPath {
    no_target: bool,
    airborne: bool,
    flipped: bool,
    miss_x: bool,
    miss_y: bool,
    left: bool,
    behind: bool,
}

/// 0xb5e0 : la cible (`+0x50`) est-elle dans la boîte devant l'ennemi ? D0 = décalage X de la
/// boîte, D1 = demi-largeur, D2 = demi-profondeur. Résultat D0 : 0 hors boîte (ou cible en l'air),
/// 1 cible du côté vers lequel l'ennemi regarde, 2 de l'autre côté.
fn target_in_front(m: &mut Mach) {
    let a0 = m.c.a[0];
    let mut path = FrontPath::default();
    let mut result = 0u32;
    let tw = m.r16(a0 + 0x50);
    logic(m.c, tw as u32, Size::Word);
    'body: {
        if tw == 0 {
            path.no_target = true;
            break 'body;
        }
        let t = sext(tw);
        m.c.a[1] = t;
        let z = m.r16(t + 0xc);
        logic(m.c, z as u32, Size::Word);
        if z != 0 {
            path.airborne = true;
            break 'body;
        }
        let flip = m.r8(a0 + 0x2c);
        logic(m.c, flip as u32, Size::Byte);
        path.flipped = flip != 0;
        let mut d0 = m.c.d[0] as u16;
        if path.flipped {
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
            path.miss_x = true;
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
            path.miss_y = true;
            break 'body;
        }
        set_b(&mut m.c.d[2], 32);
        logic(m.c, 32, Size::Byte);
        set_w(&mut m.c.d[1], ax);
        logic(m.c, ax as u32, Size::Word);
        let d1 = sub_w(m.c, tx, ax);
        set_w(&mut m.c.d[1], d1);
        path.left = m.c.sr & SR_N != 0;
        if !path.left {
            set_b(&mut m.c.d[2], 0);
            logic(m.c, 0, Size::Byte);
        }
        cmp(m.c, flip as u32, m.c.d[2] & 0xff, Size::Byte);
        path.behind = m.c.d[2] as u8 != flip;
        result = if path.behind { 2 } else { 1 };
    }
    let key = (path.no_target as u64)
        | (path.airborne as u64) << 1
        | (path.flipped as u64) << 2
        | (path.miss_x as u64) << 3
        | (path.miss_y as u64) << 4
        | (path.left as u64) << 5
        | (path.behind as u64) << 6;
    m.seg(0xb5e0, END, key, |pc| match pc {
        0xb5e4 => path.no_target,
        0xb5f0 => path.airborne,
        0xb5f8 => !path.flipped,
        0xb60c => path.miss_x,
        0xb620 => path.miss_y,
        0xb630 => path.left,
        0xb63a => path.behind,
        _ => panic!("target_in_front : branchement inattendu en {pc:06x}"),
    });
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
    m.seg1(0x103d6, END);
    m.ext(0x1a32);
    copy_attack_id(m);
}

/// 0x1040c : un pas d'animation (0x1a56, O1) puis mémorise l'identifiant d'attaque ; C = fin de
/// l'animation (retenue du pas), les autres drapeaux sont effacés.
fn step_animation(m: &mut Mach) {
    // à blanc : retenue supposée levée (branchement pris, 2 cycles de plus) ; l'exécution réelle
    // rend l'écart quand la retenue est claire
    m.seg(0x1040c, END, 1, |pc| pc == 0x10412);
    m.seg1(0x1046c, END);
    m.ext(0x1a56);
    let carry = m.c.sr & SR_C != 0;
    if !m.dry && !carry {
        m.refund(2);
    }
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
        m.seg(0x1050c, END, 1, |pc| pc == 0x10510);
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
    m.seg(0x1050c, END, 2 | (neg as u64), |pc| pc == 0x10520 && !neg);
    cmp(m.c, flip as u32, m.c.d[0] & 0xff, Size::Byte);
}

/// 0x10560 : compare le signe de la vitesse X (`+0x14`) à l'orientation de l'ennemi.
fn moving_forward(m: &mut Mach) {
    let a0 = m.c.a[0];
    m.seg1(0x10560, END);
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
    /// Appelle une routine d'origine (non réécrite). À blanc : rien (durée hors de `cost`).
    fn ext(&mut self, addr: u32) {
        if !self.dry {
            self.c.a[5] = A5;
            call(self.c, self.b, addr);
        }
    }
    /// Rend des cycles que `cost` a comptés en trop (hypothèse pessimiste d'un chemin).
    fn refund(&mut self, n: u64) {
        self.c.cycles -= n;
    }
}

macro_rules! native {
    ($name:ident, $entry:expr, $label:expr, $logic:expr) => {
        pub const $name: Native = Native {
            entry: $entry,
            name: $label,
            cost: |c, b| {
                let mut c2 = c.clone();
                let mut m = Mach { c: &mut c2, b, dry: true, cyc: 0, rng: None };
                $logic(&mut m);
                m.cyc
            },
            run: |c, b| {
                let mut m = Mach { c, b, dry: false, cyc: 0, rng: None };
                $logic(&mut m);
                rts(c, b);
            },
            touches_sound: false,
            outputs: regs::ALL,
            exact_cost: true,
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
