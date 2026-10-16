//! Recompilateur statique 68000 -> Rust.
//!
//! 1. Découverte du code par descente récursive depuis les vecteurs et des points d'entrée connus,
//!    avec les heuristiques propres aux jeux Capcom (tables de saut relatives au PC, appels sans
//!    pile via A6, créations de tâches par TRAP #0).
//! 2. Découpage en blocs de base.
//! 3. Génération : chaque instruction devient un appel à `Cpu::op_*` avec une instruction
//!    constante (spécialisée à l'inlining), suivi du décompte de cycles et d'une sortie dès que
//!    l'échéance de la ligne est atteinte. La sémantique est donc celle de l'interpréteur validé.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write;

use m68k::decode::{BitOp, Ea, Fetch, Instr, Op, ShiftCount, ShiftKind, Size};

/// Image du programme vue par le CPU.
pub struct Image<'a> {
    /// Espace programme (opcodes et lectures relatives au PC), mots big-endian.
    pub prog: &'a [u16],
    /// Espace données (vecteurs d'exception).
    pub data: &'a [u16],
}

impl Fetch for Image<'_> {
    fn fetch16(&mut self, a: u32) -> u16 {
        *self.prog.get((a as usize & 0xff_ffff) >> 1).unwrap_or(&0)
    }
}

impl Image<'_> {
    fn w(&self, a: u32) -> u16 {
        *self.prog.get((a as usize) >> 1).unwrap_or(&0)
    }
    fn l(&self, a: u32) -> u32 {
        (self.w(a) as u32) << 16 | self.w(a + 2) as u32
    }
    fn in_rom(&self, a: u32) -> bool {
        (a as usize) < self.prog.len() * 2 && a & 1 == 0
    }
}

/// L'instruction termine-t-elle un bloc ?
fn ends_block(op: &Op) -> bool {
    use Op::*;
    matches!(
        op,
        Bra(_) | Bsr(_) | Bcc(..) | Dbcc(..) | Jmp(_) | Jsr(_) | Rts | Rte | Rtr | Trap(_) | Trapv
            | Chk(..) | Illegal | LineA | LineF | Stop(_) | Reset
            // changement possible du masque d'interruption : on rend la main au répartiteur
            | MoveToSr(_) | AndiSr(_) | OriSr(_) | EoriSr(_)
    )
}

/// Le flot continue-t-il après l'instruction ?
fn falls_through(op: &Op) -> bool {
    use Op::*;
    !matches!(op, Bra(_) | Jmp(_) | Rts | Rte | Rtr | Illegal | LineA | LineF)
}

fn static_target(op: &Op) -> Option<u32> {
    use Op::*;
    match *op {
        Bra(t) | Bsr(t) | Bcc(_, t) | Dbcc(_, _, t) => Some(t),
        Jmp(Ea::AbsW(t) | Ea::AbsL(t) | Ea::PcDisp(t)) | Jsr(Ea::AbsW(t) | Ea::AbsL(t) | Ea::PcDisp(t)) => Some(t),
        _ => None,
    }
}

/// Résultat de la découverte.
pub struct Program {
    pub instrs: BTreeMap<u32, Instr>,
    pub leaders: BTreeSet<u32>,
}

/// Découverte du code depuis `entries` (plus les vecteurs d'exception lus en espace données).
pub fn discover(img: &mut Image, entries: &[u32]) -> Program {
    let mut instrs: BTreeMap<u32, Instr> = BTreeMap::new();
    let mut leaders: BTreeSet<u32> = BTreeSet::new();
    let mut work: VecDeque<u32> = VecDeque::new();

    // vecteurs : reset en espace programme, les autres en espace données
    let reset = img.l(4);
    let mut roots = vec![reset];
    for v in 2..64u32 {
        let i = (v * 2) as usize;
        let t = (img.data[i] as u32) << 16 | img.data[i + 1] as u32;
        roots.push(t);
    }
    roots.extend_from_slice(entries);
    for r in roots {
        if img.in_rom(r) && leaders.insert(r) {
            work.push_back(r);
        }
    }

    while let Some(start) = work.pop_front() {
        let mut pc = start;
        loop {
            if instrs.contains_key(&pc) {
                break;
            }
            let ins = m68k::decode(img, pc);
            instrs.insert(pc, ins);
            let mut targets: Vec<u32> = static_target(&ins.op).into_iter().collect();
            targets.extend(table_targets(img, &instrs, &ins));
            for t in targets {
                if img.in_rom(t) && leaders.insert(t) {
                    work.push_back(t);
                }
            }
            let next = pc.wrapping_add(ins.len);
            if !falls_through(&ins.op) {
                break;
            }
            if ends_block(&ins.op) {
                // l'instruction suivante commence un nouveau bloc
                if img.in_rom(next) && leaders.insert(next) {
                    work.push_back(next);
                }
                break;
            }
            pc = next;
            if !img.in_rom(pc) {
                break;
            }
        }
    }
    Program { instrs, leaders }
}

/// Cibles implicites : idiomes de tables relatives au PC et d'appels indirects (cf. Cps2Setup.java).
fn table_targets(img: &mut Image, instrs: &BTreeMap<u32, Instr>, ins: &Instr) -> Vec<u32> {
    let a = ins.pc;
    let op = ins.opcode;
    let mut out = Vec::new();
    // lea (d16,pc),a6 : adresse de retour d'un appel sans pile
    if op == 0x4dfa {
        out.push(a.wrapping_add(2).wrapping_add(img.w(a + 2) as i16 as i32 as u32));
    }
    // lea abs.l,a6 | lea abs.l,An ; jmp (An) | lea abs.l,a0 ; ... trap #0 (création de tâche)
    if op & 0xf1ff == 0x41f9 {
        let an = (op >> 9) & 7;
        let t = img.l(a + 2);
        let next = img.w(a + 6);
        let spawn = an == 0 && spawns_task_soon(img, a + 6);
        if an == 6 || next == (0x4ed0 | an) || spawn {
            out.push(t);
        }
    }
    // movea.l (d8,pc,Dn),An ; jmp/jsr (An) : table de pointeurs longs
    if op & 0xf1ff == 0x207b {
        let an = (op >> 9) & 7;
        let nxt = img.w(a + 4);
        if nxt == (0x4ed0 | an) || nxt == (0x4e90 | an) {
            let base = a.wrapping_add(2).wrapping_add(img.w(a + 2) as u8 as i8 as i32 as u32);
            for k in 0..64u32 {
                let t = img.l(base + 4 * k);
                if !img.in_rom(t) {
                    if k == 0 {
                        continue;
                    }
                    break;
                }
                out.push(t);
            }
        }
    }
    // jsr/jmp (d8,pc,Dn)
    if op == 0x4ebb || op == 0x4efb {
        let base = a.wrapping_add(2).wrapping_add(img.w(a + 2) as u8 as i8 as i32 as u32);
        let prev = a.wrapping_sub(4);
        let offset_table = instrs.contains_key(&prev)
            && (img.w(prev) & 0xf1ff) == 0x303b
            && prev.wrapping_add(2).wrapping_add(img.w(prev + 2) as u8 as i8 as i32 as u32) == base;
        if offset_table {
            let mut end = u32::MAX;
            for k in 0..512u32 {
                let slot = base + 2 * k;
                if slot >= end {
                    break;
                }
                let v = img.w(slot) as i16 as i32;
                let t = base.wrapping_add(v as u32);
                if v & 1 != 0 || !img.in_rom(t) || (t >= base && t <= slot) {
                    break;
                }
                out.push(t);
                if t > base {
                    end = end.min(t);
                }
            }
        } else {
            for k in 0..512u32 {
                let slot = base + 4 * k;
                let w = img.w(slot);
                if w != 0x6000 && w != 0x4e75 && w != 0x4e71 {
                    break;
                }
                out.push(slot);
            }
        }
    }
    out
}

/// trap #0 dans les 4 instructions suivantes, ou appel d'une fonction qui commence par un trap #0.
fn spawns_task_soon(img: &mut Image, mut pc: u32) -> bool {
    for _ in 0..4 {
        if !img.in_rom(pc) {
            return false;
        }
        let ins = m68k::decode(img, pc);
        if img.w(pc) == 0x4e40 {
            return true;
        }
        if let Op::Bsr(t) | Op::Jsr(Ea::AbsW(t) | Ea::AbsL(t)) = ins.op {
            if (0..8).step_by(2).any(|k| img.w(t + k) == 0x4e40) {
                return true;
            }
        }
        pc = pc.wrapping_add(ins.len);
    }
    false
}

// ---------- génération ----------

fn size(s: Size) -> &'static str {
    match s {
        Size::Byte => "Size::Byte",
        Size::Word => "Size::Word",
        Size::Long => "Size::Long",
    }
}

fn ea(e: Ea) -> String {
    match e {
        Ea::Dn(n) => format!("Ea::Dn({n})"),
        Ea::An(n) => format!("Ea::An({n})"),
        Ea::Ind(n) => format!("Ea::Ind({n})"),
        Ea::PostInc(n) => format!("Ea::PostInc({n})"),
        Ea::PreDec(n) => format!("Ea::PreDec({n})"),
        Ea::Disp(n, d) => format!("Ea::Disp({n}, {d})"),
        Ea::Idx(n, x) => format!("Ea::Idx({n}, {x:#06x})"),
        Ea::AbsW(a) => format!("Ea::AbsW({a:#x})"),
        Ea::AbsL(a) => format!("Ea::AbsL({a:#x})"),
        Ea::PcDisp(a) => format!("Ea::PcDisp({a:#x})"),
        Ea::PcIdx(a, x) => format!("Ea::PcIdx({a:#x}, {x:#06x})"),
        Ea::Imm(v) => format!("Ea::Imm({v:#x})"),
    }
}

fn count(c: ShiftCount) -> String {
    match c {
        ShiftCount::Imm(v) => format!("ShiftCount::Imm({v})"),
        ShiftCount::Reg(r) => format!("ShiftCount::Reg({r})"),
    }
}

fn kind(k: ShiftKind) -> &'static str {
    match k {
        ShiftKind::As => "ShiftKind::As",
        ShiftKind::Ls => "ShiftKind::Ls",
        ShiftKind::Rox => "ShiftKind::Rox",
        ShiftKind::Ro => "ShiftKind::Ro",
    }
}

fn bitop(b: BitOp) -> &'static str {
    match b {
        BitOp::Btst => "BitOp::Btst",
        BitOp::Bchg => "BitOp::Bchg",
        BitOp::Bclr => "BitOp::Bclr",
        BitOp::Bset => "BitOp::Bset",
    }
}

/// Expression Rust construisant `op`.
pub fn op_expr(op: &Op) -> String {
    use Op::*;
    let s3 = |n: &str, sz: Size, a: Ea, b: Ea| format!("Op::{n}({}, {}, {})", size(sz), ea(a), ea(b));
    let sa = |n: &str, sz: Size, a: Ea, r: u8| format!("Op::{n}({}, {}, {r})", size(sz), ea(a));
    match *op {
        Add(z, a, b) => s3("Add", z, a, b),
        Sub(z, a, b) => s3("Sub", z, a, b),
        Addx(z, a, b) => s3("Addx", z, a, b),
        Subx(z, a, b) => s3("Subx", z, a, b),
        Cmp(z, a, b) => s3("Cmp", z, a, b),
        And(z, a, b) => s3("And", z, a, b),
        Or(z, a, b) => s3("Or", z, a, b),
        Eor(z, a, b) => s3("Eor", z, a, b),
        Move(z, a, b) => s3("Move", z, a, b),
        Adda(z, a, r) => sa("Adda", z, a, r),
        Suba(z, a, r) => sa("Suba", z, a, r),
        Cmpa(z, a, r) => sa("Cmpa", z, a, r),
        Movea(z, a, r) => sa("Movea", z, a, r),
        AndiCcr(v) => format!("Op::AndiCcr({v})"),
        OriCcr(v) => format!("Op::OriCcr({v})"),
        EoriCcr(v) => format!("Op::EoriCcr({v})"),
        AndiSr(v) => format!("Op::AndiSr({v})"),
        OriSr(v) => format!("Op::OriSr({v})"),
        EoriSr(v) => format!("Op::EoriSr({v})"),
        Neg(z, a) => format!("Op::Neg({}, {})", size(z), ea(a)),
        Negx(z, a) => format!("Op::Negx({}, {})", size(z), ea(a)),
        Not(z, a) => format!("Op::Not({}, {})", size(z), ea(a)),
        Clr(z, a) => format!("Op::Clr({}, {})", size(z), ea(a)),
        Tst(z, a) => format!("Op::Tst({}, {})", size(z), ea(a)),
        Tas(a) => format!("Op::Tas({})", ea(a)),
        Ext(z, r) => format!("Op::Ext({}, {r})", size(z)),
        Swap(r) => format!("Op::Swap({r})"),
        Exg(m, x, y) => format!("Op::Exg({m}, {x}, {y})"),
        Mulu(a, r) => format!("Op::Mulu({}, {r})", ea(a)),
        Muls(a, r) => format!("Op::Muls({}, {r})", ea(a)),
        Divu(a, r) => format!("Op::Divu({}, {r})", ea(a)),
        Divs(a, r) => format!("Op::Divs({}, {r})", ea(a)),
        Abcd(a, b) => format!("Op::Abcd({}, {})", ea(a), ea(b)),
        Sbcd(a, b) => format!("Op::Sbcd({}, {})", ea(a), ea(b)),
        Nbcd(a) => format!("Op::Nbcd({})", ea(a)),
        Chk(a, r) => format!("Op::Chk({}, {r})", ea(a)),
        Moveq(v, r) => format!("Op::Moveq({v}, {r})"),
        Movem { size: z, to_mem, mask, ea: e } => {
            format!("Op::Movem {{ size: {}, to_mem: {to_mem}, mask: {mask:#06x}, ea: {} }}", size(z), ea(e))
        }
        Movep { size: z, to_mem, dn, an, disp } => {
            format!("Op::Movep {{ size: {}, to_mem: {to_mem}, dn: {dn}, an: {an}, disp: {disp} }}", size(z))
        }
        MoveFromSr(a) => format!("Op::MoveFromSr({})", ea(a)),
        MoveToSr(a) => format!("Op::MoveToSr({})", ea(a)),
        MoveToCcr(a) => format!("Op::MoveToCcr({})", ea(a)),
        MoveUsp { to_usp, an } => format!("Op::MoveUsp {{ to_usp: {to_usp}, an: {an} }}"),
        Lea(a, r) => format!("Op::Lea({}, {r})", ea(a)),
        Pea(a) => format!("Op::Pea({})", ea(a)),
        Link(r, d) => format!("Op::Link({r}, {d})"),
        Unlk(r) => format!("Op::Unlk({r})"),
        Bit(k, c, a) => format!("Op::Bit({}, {}, {})", bitop(k), count(c), ea(a)),
        Shift { kind: k, left, size: z, count: c, dn } => format!(
            "Op::Shift {{ kind: {}, left: {left}, size: {}, count: {}, dn: {dn} }}",
            kind(k), size(z), count(c)
        ),
        ShiftMem { kind: k, left, ea: e } => format!("Op::ShiftMem {{ kind: {}, left: {left}, ea: {} }}", kind(k), ea(e)),
        Bra(t) => format!("Op::Bra({t:#x})"),
        Bsr(t) => format!("Op::Bsr({t:#x})"),
        Bcc(c, t) => format!("Op::Bcc({c}, {t:#x})"),
        Dbcc(c, r, t) => format!("Op::Dbcc({c}, {r}, {t:#x})"),
        Scc(c, a) => format!("Op::Scc({c}, {})", ea(a)),
        Jmp(a) => format!("Op::Jmp({})", ea(a)),
        Jsr(a) => format!("Op::Jsr({})", ea(a)),
        Rts => "Op::Rts".into(),
        Rte => "Op::Rte".into(),
        Rtr => "Op::Rtr".into(),
        Trap(n) => format!("Op::Trap({n})"),
        Trapv => "Op::Trapv".into(),
        Nop => "Op::Nop".into(),
        Reset => "Op::Reset".into(),
        Stop(v) => format!("Op::Stop({v:#x})"),
        Illegal => "Op::Illegal".into(),
        LineA => "Op::LineA".into(),
        LineF => "Op::LineF".into(),
    }
}

/// Découverte statique, puis ajout des points d'entrée appris à l'exécution qui ne sont pas déjà
/// des instructions connues (les autres viennent des sorties de bloc à l'échéance d'une ligne).
pub fn discover_with_extra(img: &mut Image, extra: &[u32]) -> Program {
    let base = discover(img, &[]);
    let new: Vec<u32> = extra.iter().copied().filter(|a| !base.instrs.contains_key(a)).collect();
    if new.is_empty() {
        return base;
    }
    discover(img, &new)
}

/// Paramètres de génération.
pub struct Options<'a> {
    /// Type Rust du bus (implémente m68k::Bus), ex. "cps2::Cps2Bus".
    pub bus_type: &'a str,
    /// Nombre de modules générés (compilation parallèle).
    pub modules: usize,
    /// Longueur maximale d'un bloc.
    pub max_block: usize,
}

/// Blocs de base : (adresse de tête, instructions).
pub fn blocks(p: &Program, max_block: usize) -> Vec<(u32, Vec<Instr>)> {
    let mut out = Vec::new();
    for &lead in &p.leaders {
        let mut pc = lead;
        let mut v = Vec::new();
        while let Some(ins) = p.instrs.get(&pc) {
            v.push(*ins);
            let next = pc.wrapping_add(ins.len);
            if ends_block(&ins.op) || !falls_through(&ins.op) || v.len() >= max_block || p.leaders.contains(&next) {
                break;
            }
            pc = next;
        }
        if !v.is_empty() {
            out.push((lead, v));
        }
    }
    out
}

/// Génère les fichiers Rust : `mod.rs` (répartiteur) et `b{k}.rs` (blocs). Renvoie (nom, contenu).
pub fn generate(p: &Program, opt: &Options) -> Vec<(String, String)> {
    let blocks = blocks(p, opt.max_block);
    let n = opt.modules.max(1);
    let per = blocks.len().div_ceil(n).max(1);
    let mut files = Vec::new();
    let mut dispatch = String::new();
    for (k, chunk) in blocks.chunks(per).enumerate() {
        let mut s = String::new();
        writeln!(s, "// Code généré par recomp : ne pas modifier.").unwrap();
        writeln!(s, "use m68k::decode::{{BitOp, Ea, Instr, Op, ShiftCount, ShiftKind, Size}};").unwrap();
        writeln!(s, "use m68k::Cpu;").unwrap();
        writeln!(s, "type B = {};", opt.bus_type).unwrap();
        for (lead, ins) in chunk {
            writeln!(s, "\n#[inline(never)]\npub fn b_{lead:06x}(c: &mut Cpu, b: &mut B, dl: u64) -> u32 {{").unwrap();
            for (i, x) in ins.iter().enumerate() {
                let base = m68k::timing::base_cycles(x);
                let f = m68k::exec::op_fn_name(&x.op);
                let last = i + 1 == ins.len();
                writeln!(
                    s,
                    "    {{ const I: Instr = Instr {{ op: {}, pc: {:#x}, len: {}, opcode: {:#06x} }};",
                    op_expr(&x.op), x.pc, x.len, x.opcode
                )
                .unwrap();
                writeln!(s, "      c.pc = {:#x}; c.op_start_cycles = c.cycles; c.cycles += {base};", x.pc.wrapping_add(x.len)).unwrap();
                if last {
                    writeln!(s, "      c.{f}(b, &I); }}").unwrap();
                } else {
                    writeln!(s, "      if c.{f}(b, &I) || c.cycles >= dl {{ return c.pc; }} }}").unwrap();
                }
            }
            writeln!(s, "    c.pc\n}}").unwrap();
            writeln!(dispatch, "        {lead:#x} => Some(b{k}::b_{lead:06x}(c, b, dl)),").unwrap();
        }
        files.push((format!("b{k}.rs"), s));
    }
    let mut m = String::new();
    writeln!(m, "// Code généré par recomp : ne pas modifier.").unwrap();
    for k in 0..files.len() {
        writeln!(m, "mod b{k} {{ include!(concat!(env!(\"OUT_DIR\"), \"/b{k}.rs\")); }}").unwrap();
    }
    writeln!(m, "\n/// Nombre de blocs recompilés.\npub const BLOCKS: usize = {};", blocks.len()).unwrap();
    writeln!(
        m,
        "\n/// Exécute le bloc commençant à `pc` (jusqu'à sa fin, une exception ou l'échéance `dl`).\n\
         /// Renvoie None si aucun bloc ne commence à cette adresse.\n\
         #[inline(never)]\n\
         pub fn run_block(pc: u32, c: &mut m68k::Cpu, b: &mut {}, dl: u64) -> Option<u32> {{\n    match pc {{",
        opt.bus_type
    )
    .unwrap();
    m.push_str(&dispatch);
    writeln!(m, "        _ => None,\n    }}\n}}").unwrap();
    files.push(("mod.rs".into(), m));
    files
}
