//! Fuzz différentiel guidé par la couverture : valide contre l'original les chemins des routines natives que les
//! scénarios n'exercent pas (docs/couverture-fonctions.md, « Validation par fuzz »).
//!
//! Le scénario déroule la partie avec le code d'origine seul (machine de référence de `validate`). Aux appels d'une
//! routine native ciblée, l'état réel est gardé comme base ; des copies en sont perturbées puis exécutées par le code
//! d'origine (interprété, couverture relevée) et par la native, et comparées (mémoire, registres du contrat, PC).
//!
//! Les perturbations visent ce que l'original lit : chaque base est d'abord exécutée par l'original avec un bus qui
//! journalise les lectures de RAM ; les octets et mots lus sont ensuite réécrits (valeurs petites, extrêmes, voisines,
//! bits basculés). Les lectures faites juste avant un saut conditionnel dont un côté n'a jamais été pris par les
//! scénarios (bitmap `FUZZ_UNION`, sortie fusionnée de `cov`) sont « chaudes » : elles sont tirées plus souvent, avec
//! la valeur immédiate comparée et ses voisines. Une copie qui atteint une instruction nouvelle entre au corpus et
//! sera de nouveau perturbée (fuzz évolutif).
//!
//!   fuzz_couv <roms> <eeprom> <scénario> <images> <cibles> <couverture.bin>
//!
//! Cibles (séparées par des virgules) : code de zone de docs/atlas.md (`C04`), adresse d'entrée hexa (`0x8ec02`) ou
//! préfixe de nom de native (`nom:boss_`). `-nom:préfixe` exclut. La sortie est un bitmap au format de `cov`
//! (instructions de l'original exécutées pendant les essais).
//!
//! Environnement : FUZZ_UNION (bitmap des scénarios), FUZZ_ATLAS (docs/atlas.md), FUZZ_PERIODE (16 : un appel sur
//! n), FUZZ_POINTS (400 appels retenus au plus par native), FUZZ_ESSAIS (24 par appel retenu), FUZZ_GRAINE, FUZZ_PAS (300 000 instructions au plus pour l'original),
//! FUZZ_EXEMPLES (3 par native), FUZZ_TRACE=1 (trace de l'original pour le premier écart).

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use cps2::Cps2Bus;
use m68k::decode::Fetch;
use m68k::{Cpu, Ea, Op};

use ddsom_game::mem::rd32;

/// Bus de l'original pendant un essai : journalise les lectures de RAM de travail (adresse, taille, PC).
struct Journal<'a> {
    b: &'a mut Cps2Bus,
    pc: u32,
    lectures: Option<&'a mut Vec<(u32, u8, u32)>>,
}

impl Journal<'_> {
    fn note(&mut self, a: u32, taille: u8) {
        if let Some(l) = self.lectures.as_deref_mut() {
            let a = a & 0xff_ffff;
            // ni la RAM du noyau (tâches, piles), ni la pile privée des wrappers de son et son pointeur, ni les
            // listes triées (têtes, liens des objets) : des états qu'aucun code ne produit
            if a >= 0xff_1000 && !(0xff_2900..0xff_2a20).contains(&a) && !lien_de_liste(a) && l.len() < 200_000 {
                // lecture longue : deux mots consécutifs lus par la même instruction
                if let Some(d) = l.last_mut().filter(|d| taille == 2 && d.1 == 2 && d.2 == self.pc && d.0 + 2 == a) {
                    d.1 = 4;
                    return;
                }
                l.push((a, taille, self.pc));
            }
        }
    }
}

impl Fetch for Journal<'_> {
    fn fetch16(&mut self, a: u32) -> u16 {
        self.b.fetch16(a)
    }
}

impl m68k::Bus for Journal<'_> {
    fn read8(&mut self, a: u32) -> u8 {
        self.note(a, 1);
        self.b.read8(a)
    }
    fn read16(&mut self, a: u32) -> u16 {
        self.note(a, 2);
        self.b.read16(a)
    }
    fn write8(&mut self, a: u32, v: u8) {
        ecriture_originale(self.pc, a, 1, v as u32);
        self.b.write8(a, v)
    }
    fn write16(&mut self, a: u32, v: u16) {
        ecriture_originale(self.pc, a, 2, v as u32);
        self.b.write16(a, v)
    }
    fn read_prog16(&mut self, a: u32) -> u16 {
        self.b.read_prog16(a)
    }
    fn reset_devices(&mut self) {
        self.b.reset_devices()
    }
    fn irq_ack(&mut self, l: u8) {
        self.b.irq_ack(l)
    }
}

thread_local! {
    /// Adresse dont les écritures de l'original sont signalées (diagnostic `FUZZ_SURVEILLE`), 0 : aucune.
    static SURVEILLEE: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static SURVEILLEE_FIN: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    /// PC de l'original dont les registres sont montrés (`FUZZ_PC=adresse,...`) pendant la surveillance.
    static PC_SURVEILLES: Vec<u32> = std::env::var("FUZZ_PC")
        .map(|v| v.split(',').filter_map(|x| u32::from_str_radix(x.trim_start_matches("0x"), 16).ok()).collect())
        .unwrap_or_default();
}

fn ecriture_originale(pc: u32, a: u32, taille: u32, v: u32) {
    let s = SURVEILLEE.with(|x| x.get());
    let f = SURVEILLEE_FIN.with(|x| x.get()).max(s + 1);
    let a = a & 0xff_ffff;
    if s != 0 && a < f && a + taille > s {
        eprintln!("ORIGINAL {pc:06x} : {a:06x} = {v:x}");
    }
}

/// Une perturbation : écrire `valeur` (1, 2 ou 4 octets) en `adresse`.
#[derive(Clone, Copy, Debug)]
struct Mutation {
    adresse: u32,
    taille: u8,
    valeur: u32,
    /// valeur déjà vue dans ce champ (même champ d'un autre objet du pool, autre appel) ou comparée par le code
    plausible: bool,
}

/// Clé d'un champ : (pool, type de l'objet, décalage dans l'emplacement) pour les objets, (joueur, décalage) pour les
/// joueurs, sinon l'adresse.
fn cle(b: &Cps2Bus, a: u32) -> u32 {
    use ddsom_game::objects::pools::POOLS;
    for (i, p) in POOLS.iter().enumerate() {
        let debut = ddsom_game::mem::A5 + p.first;
        if (debut..debut + p.stride * p.slots).contains(&a) {
            let slot = debut + (a - debut) / p.stride * p.stride;
            let type_ = lire(b, slot + 2, 1);
            return 0x0100_0000 | (i as u32) << 20 | type_ << 12 | (a - slot);
        }
    }
    if (0xff_85de..0xff_85de + 0x800).contains(&a) {
        return 0x0200_0000 | (a - 0xff_85de) % 0x200;
    }
    a
}

/// Têtes et queues des listes triées (A5+0x5be-0x5c5) et liens `+0x24`/`+0x26` des objets listés et des joueurs.
fn lien_de_liste(a: u32) -> bool {
    use ddsom_game::objects::pools::POOLS;
    if (0xff_85be..0xff_85c6).contains(&a) {
        return true;
    }
    let lien = |debut: u32, pas: u32, n: u32| (debut..debut + pas * n).contains(&a) && (0x24..0x28).contains(&((a - debut) % pas));
    lien(0xff_85de, 0x200, 4) || POOLS.iter().any(|p| p.listed && lien(ddsom_game::mem::A5 + p.first, p.stride, p.slots))
}

/// Valeurs vues de chaque champ (clé, taille) dans les états de base.
type Domaines = HashMap<(u32, u8), Vec<u32>>;

fn lire(b: &Cps2Bus, a: u32, taille: u8) -> u32 {
    let mot = |a: u32| b.ram[(a as usize & 0xffff) >> 1] as u32;
    match (taille, a & 1) {
        (4, _) => mot(a) << 16 | mot(a + 2),
        (2, _) => mot(a),
        (_, 0) => mot(a) >> 8,
        _ => mot(a) & 0xff,
    }
}

/// Lecture chaude : faite juste avant un saut conditionnel pris d'un seul côté, avec les valeurs qui pourraient
/// basculer le saut.
#[derive(Clone, Debug)]
struct Chaude {
    adresse: u32,
    taille: u8,
    valeurs: Vec<u32>,
}

struct Base {
    c: Cpu,
    b: Cps2Bus,
    /// état synthétisé depuis l'appel d'une autre entrée (substitution)
    subst: bool,
    lectures: Vec<(u32, u8)>,
    chaudes: Vec<Chaude>,
}

#[derive(Default)]
struct Bilan {
    appels: u64,
    essais: u64,
    ecarts: u64,
    paniques: u64,
    abandons: u64,
    /// essais écartés : l'original a sauté hors du code (table de sauts lue hors de ses bornes)
    hors_code: u64,
    /// essais dont l'original passe par une instruction que les scénarios n'exécutent pas, et écarts parmi eux
    neufs: u64,
    ecarts_neufs: u64,
    /// écarts dont toutes les perturbations sont plausibles
    ecarts_plausibles: u64,
    nouvelles: u64,
    exemples: Vec<String>,
    /// signatures d'écart déjà vues (première adresse en écart) : nombre d'occurrences
    signatures: BTreeMap<String, u64>,
}

struct Cible {
    n: &'static ddsom_game::Native,
    bases: Vec<Base>,
    /// (base, perturbations) qui ont atteint une instruction nouvelle
    corpus: Vec<(usize, Vec<Mutation>)>,
    bilan: Bilan,
    /// appels retenus pour le fuzz
    points: u64,
    /// zone de l'atlas de l'entrée
    zone: String,
    /// gestionnaire d'une classe d'objets : (pool, type)
    gestionnaire: Option<(usize, u8)>,
    /// appels synthétisés (entrée jamais appelée, essayée depuis l'état d'une voisine)
    substitutions: u64,
    /// bases qui ne passent pas par les zones visées
    hors_focus: u64,
}

struct Etat {
    cibles: HashMap<u32, Cible>,
    union: Vec<u8>,
    vus: Vec<u8>,
    /// sauts conditionnels pris d'un seul côté par les scénarios (adresse -> (cible, suite))
    frontiere: HashMap<u32, (u32, u32)>,
    alea: Alea,
    domaines: Domaines,
    /// essayer les entrées jamais appelées depuis l'état d'une voisine (`FUZZ_SUBSTITUTS`)
    substituts: bool,
    /// zones visées (`FUZZ_FOCUS=F3,C1`) : seules les bases qui y passent sont fuzzées
    focus: Vec<(u32, u32)>,
    periode: u64,
    points_max: u64,
    essais: u32,
    pas_max: u32,
    exemples: usize,
    trace: bool,
}

thread_local! {
    static ETAT: RefCell<Option<Etat>> = const { RefCell::new(None) };
}

fn bit(bm: &[u8], a: u32) -> bool {
    let w = (a as usize & 0xfffff) >> 1;
    bm.get(w >> 3).is_some_and(|x| x >> (w & 7) & 1 != 0)
}

fn poser_bit(bm: &mut [u8], a: u32) -> bool {
    let w = (a as usize & 0xfffff) >> 1;
    let neuf = bm[w >> 3] >> (w & 7) & 1 == 0;
    bm[w >> 3] |= 1 << (w & 7);
    neuf
}

struct Alea(u64);

impl Alea {
    fn alea(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn sur(&mut self, n: u64) -> u64 {
        self.alea() % n
    }
}

impl Etat {
    fn alea(&mut self) -> u64 {
        self.alea.alea()
    }
    fn sur(&mut self, n: u64) -> u64 {
        self.alea.sur(n)
    }
}

/// Résultat d'une exécution de l'original.
struct Original {
    c: Cpu,
    b: Cps2Bus,
    pcs: Vec<u32>,
    /// resté dans le code (débuts d'instructions, tables de sauts lues dans leurs bornes)
    valide: bool,
}

/// Exécute le code d'origine (interprété) jusqu'au retour de la routine ; rend l'état final et les PC exécutés.
fn executer_original(c0: &Cpu, b0: &Cps2Bus, pas_max: u32, mut lectures: Option<&mut Vec<(u32, u8, u32)>>) -> Option<Original> {
    let (mut c, mut b) = (c0.clone(), b0.clone());
    let ret = rd32(&mut b, c.a[7]);
    let sp_apres = c.a[7].wrapping_add(4);
    let mut pcs = Vec::new();
    let mut valide = true;
    let r = catch_unwind(AssertUnwindSafe(|| {
        for _ in 0..pas_max {
            if c.pc == ret && c.a[7] == sp_apres {
                return true;
            }
            pcs.push(c.pc);
            if SURVEILLEE.with(|x| x.get()) != 0 && PC_SURVEILLES.with(|p| p.contains(&c.pc)) {
                eprintln!("ORIGINAL en {:06x} : D={:08x?} A={:08x?}", c.pc, c.d, c.a);
            }
            if valide && !dans_les_tables(&c, &mut b) {
                valide = false;
            }
            let mut j = Journal { b: &mut b, pc: c.pc, lectures: lectures.as_deref_mut() };
            c.step(&mut j);
        }
        false
    }));
    match r {
        Ok(true) => Some(Original { c, b, pcs, valide }),
        _ => None,
    }
}

/// Débuts d'instructions de listing.txt (`FUZZ_LISTING`) ; vide : pas de contrôle.
static INSTRUCTIONS: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();

fn instruction(a: u32) -> bool {
    INSTRUCTIONS.get().is_none_or(|bm| bm.is_empty() || a >= 0x10_0000 || bit(bm, a)) || execute(a)
}

/// Cibles de sauts du listing (`LAB_xxxxxxxx:`, `FUN_`) : du code, jamais une table.
static ETIQUETTES: std::sync::OnceLock<HashSet<u32>> = std::sync::OnceLock::new();

fn etiquette(a: u32) -> bool {
    ETIQUETTES.get().is_some_and(|e| e.contains(&a))
}

/// Instructions exécutées par les scénarios (`FUZZ_UNION`).
static UNION: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();

fn execute(a: u32) -> bool {
    a < 0x10_0000 && a & 1 == 0 && UNION.get().is_some_and(|u| bit(u, a))
}

thread_local! {
    /// longueur des tables de sauts (adresse de la table -> nombre d'entrées)
    static TABLES: RefCell<HashMap<(u32, bool), u32>> = RefCell::new(HashMap::new());
}

/// Nombre d'entrées d'une table de sauts : décalages 16 bits depuis la table (elle finit où commence la première
/// cible qui la suit), ou adresses longues ; chaque cible doit être un début d'instruction.
fn longueur_table(b: &mut Cps2Bus, base: u32, longue: bool) -> u32 {
    TABLES.with(|t| {
        *t.borrow_mut().entry((base, longue)).or_insert_with(|| {
            let mut fin = u32::MAX;
            for i in 0..512u32 {
                // la table finit où commence du code que les scénarios exécutent (le listing désassemble parfois les
                // tables comme des instructions)
                let a = base + if longue { 4 } else { 2 } * i;
                if i > 0 && (execute(a) || etiquette(a)) {
                    return i;
                }
                let cible = if longue {
                    (b.fetch16(base + 4 * i) as u32) << 16 | b.fetch16(base + 4 * i + 2) as u32
                } else {
                    base.wrapping_add(b.fetch16(base + 2 * i) as i16 as i32 as u32)
                };
                if cible >= 0x10_0000 || cible & 1 != 0 || !instruction(cible) {
                    return i;
                }
                if !longue && cible > base {
                    fin = fin.min(cible);
                }
                if !longue && base + 2 * (i + 1) >= fin {
                    return i + 1;
                }
            }
            512
        })
    })
}

/// Faux si l'instruction au PC lit une table de sauts (`move.w (d8,PC,Xn)` ou `move.l` suivi d'un `jmp`/`jsr`)
/// au-delà de ses entrées : l'état perturbé est impossible.
fn dans_les_tables(c: &Cpu, b: &mut Cps2Bus) -> bool {
    if c.pc & 1 != 0 || !instruction(c.pc) {
        return false;
    }
    let ins = m68k::decode(b, c.pc);
    let index = |ext: u16| {
        let r = ((ext >> 12) & 7) as usize;
        let v = if ext & 0x8000 != 0 { c.a[r] } else { c.d[r] };
        if ext & 0x800 != 0 { v } else { v as u16 as i16 as i32 as u32 }
    };
    let (taille, base, ext) = match ins.op {
        Op::Move(t, Ea::PcIdx(base, ext), _) | Op::Movea(t, Ea::PcIdx(base, ext), _) => (t, base, ext),
        // `jmp`/`jsr (d8,PC,Xn)` dans une liste de `bra` : la cible doit être l'un d'eux
        Op::Jmp(Ea::PcIdx(base, ext)) | Op::Jsr(Ea::PcIdx(base, ext)) => {
            let mut a = base;
            let mut debuts = Vec::new();
            loop {
                let i = m68k::decode(b, a);
                if !matches!(i.op, Op::Bra(_)) || debuts.len() > 256 {
                    break;
                }
                debuts.push(a);
                a += i.len;
            }
            return debuts.is_empty() || debuts.contains(&base.wrapping_add(index(ext)));
        }
        _ => return true,
    };
    let suivante = m68k::decode(b, c.pc + ins.len);
    if !matches!(suivante.op, Op::Jmp(_) | Op::Jsr(_)) {
        return true;
    }
    let index = index(ext);
    let pas = if taille == m68k::Size::Long { 4 } else { 2 };
    index % pas == 0 && index / pas < longueur_table(b, base, pas == 4)
}

/// Valeurs qu'un saut conditionnel compare à la mémoire lue par l'instruction `ins` (valeur immédiate et voisines,
/// bit testé), à défaut des valeurs usuelles.
fn valeurs_chaudes(op: &Op) -> Vec<u32> {
    match *op {
        Op::Cmp(_, Ea::Imm(v), _) => vec![v, v.wrapping_add(1), v.wrapping_sub(1), 0],
        Op::Bit(_, m68k::decode::ShiftCount::Imm(n), _) => {
            let m = 1u32 << (n & 7);
            vec![m, 0, 0xff, m | 1]
        }
        _ => vec![0, 1, 0xff, 0x80, 2, 0xffff_ffff],
    }
}

/// Exécute l'original sur la base pour journaliser ses lectures et repérer les lectures chaudes.
fn analyser_base(e: &Etat, c: &Cpu, b: &Cps2Bus) -> Option<(Vec<(u32, u8)>, Vec<Chaude>)> {
    let mut lectures = Vec::new();
    let o = executer_original(c, b, e.pas_max, Some(&mut lectures))?;
    // zones visées (`FUZZ_FOCUS`) : la base doit y passer
    if !e.focus.is_empty() && !o.pcs.iter().any(|&p| e.focus.iter().any(|(d, f)| (*d..*f).contains(&p))) {
        return Some((Vec::new(), Vec::new()));
    }
    let mut bb = b.clone();
    // lectures par instruction
    let mut par_pc: HashMap<u32, Vec<(u32, u8)>> = HashMap::new();
    let mut ordre = Vec::new();
    let mut vues = HashSet::new();
    for &(a, t, pc) in &lectures {
        par_pc.entry(pc).or_default().push((a, t));
        if vues.insert((a, t)) {
            ordre.push((a, t));
        }
    }
    // chaudes : lectures des deux instructions qui précèdent un saut de la frontière
    let mut chaudes = Vec::new();
    let mut vues_ch = HashSet::new();
    for w in o.pcs.windows(3) {
        let saut = w[2];
        if !e.frontiere.contains_key(&saut) {
            continue;
        }
        for &pc in &w[..2] {
            if let Some(l) = par_pc.get(&pc) {
                let ins = m68k::decode(&mut bb, pc);
                for &(a, t) in l {
                    if vues_ch.insert((a, t, pc)) {
                        chaudes.push(Chaude { adresse: a, taille: t, valeurs: valeurs_chaudes(&ins.op) });
                    }
                }
            }
        }
    }
    // aiguillages par table de sauts : les lectures des trois instructions précédentes reçoivent les indices valides
    // (bruts, ou divisés par 2 ou 4 : l'index est souvent doublé avant l'accès)
    for w in o.pcs.windows(4) {
        let Some(indices) = indices_table(&mut bb, w[3]) else { continue };
        let mut valeurs: Vec<u32> = Vec::new();
        for i in indices {
            for d in [1, 2, 4] {
                if i % d == 0 && !valeurs.contains(&(i / d)) {
                    valeurs.push(i / d);
                }
            }
        }
        for &pc in &w[..3] {
            if let Some(l) = par_pc.get(&pc) {
                for &(a, t) in l {
                    if t <= 2 && vues_ch.insert((a, t, pc)) {
                        chaudes.push(Chaude { adresse: a, taille: t, valeurs: valeurs.clone() });
                    }
                }
            }
        }
    }
    Some((ordre, chaudes))
}

/// Index valides de l'aiguillage par table de sauts à `pc` (`move.w (d8,PC,Xn)` suivi d'un `jmp`/`jsr`, ou
/// `jmp (d8,PC,Xn)` dans une liste de `bra`).
fn indices_table(b: &mut Cps2Bus, pc: u32) -> Option<Vec<u32>> {
    let ins = m68k::decode(b, pc);
    match ins.op {
        Op::Move(t, Ea::PcIdx(base, ext), _) | Op::Movea(t, Ea::PcIdx(base, ext), _) => {
            let suivante = m68k::decode(b, pc + ins.len);
            if !matches!(suivante.op, Op::Jmp(_) | Op::Jsr(_)) {
                return None;
            }
            let pas = if t == m68k::Size::Long { 4 } else { 2 };
            let d8 = ext as i8 as i32 as u32;
            let _ = d8;
            Some((0..longueur_table(b, base, pas == 4).min(64)).map(|i| i * pas).collect())
        }
        Op::Jmp(Ea::PcIdx(base, _)) | Op::Jsr(Ea::PcIdx(base, _)) => {
            let mut a = base;
            let mut v = Vec::new();
            while v.len() < 64 {
                let i = m68k::decode(b, a);
                if !matches!(i.op, Op::Bra(_)) {
                    break;
                }
                v.push(a - base);
                a += i.len;
            }
            (!v.is_empty()).then_some(v)
        }
        _ => None,
    }
}

fn muter(e: &mut Alea, domaines: &Domaines, base: &Base, muts: &mut Vec<Mutation>) {
    let n = 1 + e.sur(4);
    for _ in 0..n {
        let chaude = !base.chaudes.is_empty() && e.sur(2) == 0;
        let (adresse, taille, valeurs) = if chaude {
            let h = &base.chaudes[e.sur(base.chaudes.len() as u64) as usize];
            (h.adresse, h.taille, Some(h.valeurs.clone()))
        } else if !base.lectures.is_empty() {
            let (a, t) = base.lectures[e.sur(base.lectures.len() as u64) as usize];
            (a, t, None)
        } else {
            return;
        };
        // taille : parfois l'autre octet du mot, parfois l'octet seul d'un mot
        let (adresse, taille) = match (taille, e.sur(8)) {
            (2, 0) => (adresse + 1, 1),
            (2, 1) => (adresse, 1),
            (4, 0) => (adresse + 2, 2),
            _ => (adresse, taille),
        };
        // le type de l'objet courant (octet +2) ne change pas sous son gestionnaire
        let type_courant = (base.c.a[0] & 0xff_ffff) + 2;
        if (adresse..adresse + taille as u32).contains(&type_courant) {
            continue;
        }
        let actuelle = lire(&base.b, adresse, taille);
        let masque = match taille {
            1 => 0xff,
            2 => 0xffff,
            _ => 0xffff_ffff,
        };
        let domaine = domaines.get(&(cle(&base.b, adresse), taille)).filter(|d| d.len() > 1);
        let tirage = e.sur(10);
        let valeur = match (&valeurs, domaine) {
            (Some(v), _) if tirage >= 5 && tirage < 8 => v[e.sur(v.len() as u64) as usize],
            (_, Some(d)) if tirage < 8 => d[e.sur(d.len() as u64) as usize],
            _ => match e.sur(12) {
                0 => 0,
                1 => 1,
                2 => 2 * e.sur(8) as u32,
                3 => e.sur(16) as u32,
                4 => 0xffff_ffff,
                5 => 0x80,
                6 => actuelle ^ (1 << e.sur(8 * taille as u64)),
                7 => actuelle.wrapping_add(1),
                8 => actuelle.wrapping_sub(1),
                9 => 0x8000,
                10 => {
                    // valeur d'une autre lecture
                    let (a2, _) = base.lectures[e.sur(base.lectures.len() as u64) as usize];
                    lire(&base.b, a2 & !1, 2)
                }
                _ => e.alea() as u32,
            },
        } & masque;
        let plausible = valeurs.as_ref().is_some_and(|v| v.contains(&valeur))
            || domaines.get(&(cle(&base.b, adresse), taille)).is_some_and(|d| d.contains(&valeur));
        muts.push(Mutation { adresse, taille, valeur, plausible });
    }
}

fn appliquer(b: &mut Cps2Bus, muts: &[Mutation]) {
    for m in muts {
        match m.taille {
            1 => ddsom_game::mem::wr8(b, m.adresse, m.valeur as u8),
            2 => ddsom_game::mem::wr16(b, m.adresse & !1, m.valeur as u16),
            _ => ddsom_game::mem::wr32(b, m.adresse & !1, m.valeur),
        }
    }
}

fn decrire(muts: &[Mutation]) -> String {
    muts.iter()
        .map(|m| {
            let h = if m.plausible { "" } else { "!" };
            match m.taille {
                1 => format!("{:06x}={:02x}{h}", m.adresse, m.valeur),
                2 => format!("{:06x}.w={:04x}{h}", m.adresse, m.valeur),
                _ => format!("{:06x}.l={:08x}{h}", m.adresse, m.valeur),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Table des tâches Rust de la native, reconstruite depuis la table FF0064 et le tas des piles de l'original (la
/// machine de référence n'a pas de table Rust) : états, comptes, piles, entrées, piles libres.
fn table_depuis_origine(b: &mut Cps2Bus) {
    use ddsom_game::mem::{rd32, rd8};
    use ddsom_game::taches::table::{self, Emplacement, Etat, Reprise, Tache};
    let mut t = table::Table::default();
    for (k, e) in t.emplacements.iter_mut().enumerate() {
        let r = table::enregistrement(k);
        let Some(etat) = Etat::depuis_octet(rd8(b, r)) else { continue };
        let entree = rd32(b, r + 4);
        *e = Emplacement {
            tache: Tache::depuis_entree(entree),
            etat,
            compte: rd8(b, r + 1),
            pile: rd32(b, r + 0xc),
            reprise: Reprise::Entree(entree),
            ..Emplacement::default()
        };
    }
    let sommet = match rd32(b, 0xff_005a) {
        0 => 0xff_01a6,
        s => s,
    };
    t.piles = (sommet..0xff_01a6).step_by(4).rev().map(|a| rd32(b, a)).collect();
    *table::table(b) = t;
}

/// Exécute la native sur l'état `c0`/`b0` et la compare au résultat `o` de l'original : rend le message de panique ou
/// les écarts (vide : identique).
fn comparer(n: &ddsom_game::Native, c0: &Cpu, b0: &Cps2Bus, o: &Original) -> Result<Vec<String>, String> {
    let (mut c1, mut b1) = (c0.clone(), b0.clone());
    b1.irq_horizon = u64::MAX;
    table_depuis_origine(&mut b1);
    let r = catch_unwind(AssertUnwindSafe(|| (n.run)(&mut c1, &mut b1)));
    if let Err(p) = r {
        return Err(p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default());
    }
    let mut diffs = Vec::new();
    let (c2, b2) = (&o.c, &o.b);
    for i in 0..8 {
        if n.outputs & ddsom_game::regs::d(i as u32) != 0 && c1.d[i] != c2.d[i] {
            diffs.push(format!("d{i}={:08x} au lieu de {:08x}", c1.d[i], c2.d[i]));
        }
        if (n.outputs & ddsom_game::regs::a(i as u32) != 0 || i == 7) && c1.a[i] != c2.a[i] {
            diffs.push(format!("a{i}={:08x} au lieu de {:08x}", c1.a[i], c2.a[i]));
        }
    }
    if n.outputs & ddsom_game::regs::CCR != 0 && c1.sr & 0x1f != c2.sr & 0x1f {
        diffs.push(format!("ccr={:02x} au lieu de {:02x}", c1.sr & 0x1f, c2.sr & 0x1f));
    }
    if c1.pc != c2.pc {
        diffs.push(format!("pc={:06x} au lieu de {:06x}", c1.pc, c2.pc));
    }
    // RAM du noyau d'origine : la native tient la table des tâches en Rust ; comparée à part
    if let Some(t) = ddsom_game::taches::table::lire(&b1) {
        diffs.extend(ddsom_game::taches::table::ecarts(t, &b2.ram, false));
    }
    for i in 0..0x800usize {
        if ddsom_game::taches::table::ram_du_noyau(2 * i as u32) {
            b1.ram[i] = b2.ram[i];
        }
    }
    let sp = c2.a[7] & 0xff_ffff;
    ddsom_game::mem::diff_bus(&b1, b2, sp.saturating_sub(0x400)..sp, &mut diffs);
    Ok(diffs)
}

/// Signature d'un écart : première chose qui diffère (ou la panique).
fn signature(r: &Result<Vec<String>, String>) -> Option<String> {
    match r {
        Err(msg) => Some(format!("PANIQUE {}", msg.chars().take(100).collect::<String>())),
        Ok(d) if d.is_empty() => None,
        Ok(d) => Some(d[0].split('=').next().unwrap_or("").to_string()),
    }
}

/// L'original n'est resté que sur des débuts d'instructions du listing (sinon l'état perturbé l'a fait sauter dans
/// des données, par une table de sauts lue hors de ses bornes : état impossible, essai écarté).
fn chemin_valide(_: &Etat, o: &Original) -> bool {
    o.valide
}

/// Un essai : la base perturbée, exécutée par l'original puis par la native.
fn essai(e: &mut Etat, entree: u32, ib: usize, muts: Vec<Mutation>) {
    let pas_max = e.pas_max;
    let etat_de = |e: &Etat, muts: &[Mutation]| {
        let base = &e.cibles[&entree].bases[ib];
        let mut b = base.b.clone();
        appliquer(&mut b, muts);
        (base.c.clone(), b)
    };
    let (c0, b0) = etat_de(e, &muts);
    let o = match executer_original(&c0, &b0, pas_max, None) {
        Some(o) if chemin_valide(e, &o) => o,
        Some(_) => {
            e.cibles.get_mut(&entree).unwrap().bilan.hors_code += 1;
            return;
        }
        None => {
            e.cibles.get_mut(&entree).unwrap().bilan.abandons += 1;
            return;
        }
    };
    let mut nouvelles = 0;
    for &pc in &o.pcs {
        if poser_bit(&mut e.vus, pc) && !bit(&e.union, pc) {
            nouvelles += 1;
        }
    }
    let neuf = o.pcs.iter().any(|&p| !bit(&e.union, p));
    let n = e.cibles[&entree].n;
    let r = comparer(n, &c0, &b0, &o);
    let cible = e.cibles.get_mut(&entree).unwrap();
    cible.bilan.essais += 1;
    cible.bilan.neufs += neuf as u64;
    cible.bilan.nouvelles += nouvelles;
    if nouvelles > 0 && !muts.is_empty() {
        if cible.corpus.len() < 64 {
            cible.corpus.push((ib, muts.clone()));
        } else {
            let k = (o.pcs.len() + muts.len()) % 64;
            cible.corpus[k] = (ib, muts.clone());
        }
    }
    let Some(sig) = signature(&r) else { return };
    if r.is_err() {
        cible.bilan.paniques += 1;
    } else {
        cible.bilan.ecarts += 1;
    }
    if neuf {
        cible.bilan.ecarts_neufs += 1;
    }
    let hors = muts.iter().any(|m| !m.plausible);
    // écart limité aux registres de sortie (sans effet tant qu'aucun appelant ne les relit)
    let registres = matches!(&r, Ok(d) if d.iter().all(|x| x.starts_with('d') || x.starts_with('a') && !x.starts_with("au") || x.starts_with("ccr")));
    cible.bilan.ecarts_plausibles += (!hors && !registres && !cible.bases[ib].subst) as u64;
    // entrée jamais appelée, essayée depuis l'état d'une voisine : ses registres d'entrée sont ceux d'un autre appel
    let subst = cible.bases[ib].subst;
    let sig = format!("{}{}{}{}{}{sig}", if subst { "SUBST " } else { "" }, if registres { "REG " } else { "" }, if muts.is_empty() { "BASE " } else { "" }, if hors { "HORS " } else { "" }, if neuf { "NEUF " } else { "" });
    let compte = cible.bilan.signatures.entry(sig.clone()).or_insert(0);
    *compte += 1;
    if *compte != 1 || cible.bilan.exemples.len() >= e.exemples {
        return;
    }
    // réduction : on retire les perturbations dont l'écart ne dépend pas
    let sig0 = signature(&r);
    let mut min = muts.clone();
    let mut k = 0;
    while k < min.len() {
        let mut essai = min.clone();
        essai.remove(k);
        let (c, b) = etat_de(e, &essai);
        let garde = match executer_original(&c, &b, pas_max, None) {
            Some(o2) if chemin_valide(e, &o2) => signature(&comparer(n, &c, &b, &o2)) == sig0,
            _ => false,
        };
        if garde {
            min = essai;
        } else {
            k += 1;
        }
    }
    let (c0, b0) = etat_de(e, &min);
    let o = executer_original(&c0, &b0, pas_max, None).unwrap_or(o);
    let r = comparer(n, &c0, &b0, &o);
    let texte = format!(
        "{} ; perturbations [{}] ; A0={:06x} A6={:06x} D0={:08x} D1={:08x} ; chemin neuf {}",
        match &r {
            Err(m) => format!("PANIQUE {m}"),
            Ok(d) => d.join(", "),
        },
        decrire(&min),
        c0.a[0],
        c0.a[6],
        c0.d[0],
        c0.d[1],
        o.pcs.iter().filter(|&&p| !bit(&e.union, p)).take(16).map(|p| format!("{p:x}")).collect::<Vec<_>>().join(" ")
    );
    #[cfg(feature = "surveille")]
    if let Ok(a) = std::env::var("FUZZ_SURVEILLE") {
        // pile d'appels Rust des écritures de la native à cette adresse
        let (a, fin) = a.split_once('-').unwrap_or((&a, "0"));
        let a = u32::from_str_radix(a.trim_start_matches("0x"), 16).unwrap();
        let fin = u32::from_str_radix(fin.trim_start_matches("0x"), 16).unwrap();
        ddsom_game::mem::surveille::ADRESSE.store(a, std::sync::atomic::Ordering::Relaxed);
        ddsom_game::mem::surveille::FIN.store(fin, std::sync::atomic::Ordering::Relaxed);
        SURVEILLEE_FIN.with(|x| x.set(fin));
        eprintln!("SURVEILLE {a:06x} pour {} {:06x} ({})", n.name, n.entry, decrire(&min));
        SURVEILLEE.with(|x| x.set(a));
        let _ = executer_original(&c0, &b0, pas_max, None);
        SURVEILLEE.with(|x| x.set(0));
        let _ = comparer(n, &c0, &b0, &o);
        ddsom_game::mem::surveille::ADRESSE.store(0, std::sync::atomic::Ordering::Relaxed);
    }
    let texte = if e.trace {
        let mut bb = b0.clone();
        let objet: String = (0..0xc0).map(|i| format!("{:02x}", ddsom_game::mem::rd8(&mut bb, c0.a[0] + i))).collect();
        format!("{texte}\n      A0 {objet}\n      TRACE {}", o.pcs.iter().take(4000).map(|p| format!("{p:x}")).collect::<Vec<_>>().join(" "))
    } else {
        texte
    };
    e.cibles.get_mut(&entree).unwrap().bilan.exemples.push(format!("[{sig}] {texte}"));
}

fn fuzzer(entree: u32, c: &Cpu, b: &Cps2Bus) {
    ETAT.with(|x| {
        let mut x = x.borrow_mut();
        let e = x.as_mut().unwrap();
        let periode = e.periode;
        let k = {
            let cible = e.cibles.get_mut(&entree).unwrap();
            cible.bilan.appels += 1;
            cible.bilan.appels
        };
        // les premiers appels, puis un sur `période`, au plus `points` fois
        if k > 4 && k % periode != 1 || e.cibles[&entree].points >= e.points_max {
            return;
        }
        e.cibles.get_mut(&entree).unwrap().points += 1;
        fuzzer_depuis(e, entree, c, b, false);
        if !e.substituts {
            return;
        }
        // une entrée de la même zone (ou un gestionnaire du même pool) jamais appelée, depuis cet état
        let ici = &e.cibles[&entree];
        let (zone, pool) = (ici.zone.clone(), ici.gestionnaire.map(|g| g.0));
        let mut candidats: Vec<u32> = e
            .cibles
            .iter()
            .filter(|(_, y)| y.bilan.appels == 0 && y.substitutions < e.points_max)
            .filter(|(_, y)| match (pool, y.gestionnaire) {
                (Some(p), Some((q, _))) => p == q,
                (None, None) => y.zone == zone,
                _ => false,
            })
            .map(|(&a, _)| a)
            .collect();
        if candidats.is_empty() {
            return;
        }
        candidats.sort();
        let y = candidats[e.sur(candidats.len() as u64) as usize];
        let mut c2 = c.clone();
        let mut b2 = b.clone();
        c2.pc = y;
        if e.cibles[&y].gestionnaire.is_none() && cle(&b2, c2.a[0] & 0xff_ffff) < 0x0100_0000 {
            // A0 n'est pas un objet : un joueur (les sons lisent la position de l'objet A0)
            c2.a[0] = 0xff_85de + 0x200 * e.sur(4) as u32;
        }
        if let Some((_, t)) = e.cibles[&y].gestionnaire {
            // l'objet A0 devient du type de `y`, souvent à son état initial
            let obj = c2.a[0] & 0xff_ffff;
            ddsom_game::mem::wr8(&mut b2, obj + 2, t);
            ddsom_game::mem::wr8(&mut b2, obj + 3, 0);
            if e.sur(2) == 0 {
                ddsom_game::mem::wr32(&mut b2, obj + 4, 0);
            }
        }
        e.cibles.get_mut(&y).unwrap().substitutions += 1;
        fuzzer_depuis(e, y, &c2, &b2, true);
    });
}

/// Fuzz de `entree` depuis l'état `c`, `b` (PC sur l'entrée) : nouvelle base, puis les essais.
fn fuzzer_depuis(e: &mut Etat, entree: u32, c: &Cpu, b: &Cps2Bus, subst: bool) {
    // nouvelle base (au plus 3 par native ; ensuite, remplacement d'une base au hasard avec son corpus)
    let analyse = analyser_base(e, c, b);
    let Some((lectures, chaudes)) = analyse else {
        e.cibles.get_mut(&entree).unwrap().bilan.abandons += 1;
        return;
    };
    if !e.focus.is_empty() && lectures.is_empty() {
        // la base ne passe pas par les zones visées : au bout de 16, la native n'est plus essayée
        let cible = e.cibles.get_mut(&entree).unwrap();
        cible.hors_focus += 1;
        if cible.hors_focus >= 16 && cible.bases.is_empty() {
            cible.points = u64::MAX;
            cible.substitutions = u64::MAX;
        }
        return;
    }
    for &(a, t) in &lectures {
        let v = lire(b, a, t);
        let d = e.domaines.entry((cle(b, a), t)).or_default();
        if d.len() < 48 && !d.contains(&v) {
            d.push(v);
        }
    }
    let base = Base { c: c.clone(), b: b.clone(), subst, lectures, chaudes };
    // la base elle-même (sans perturbation) : couverture et contrôle
    let nb = e.cibles[&entree].bases.len();
    let ib = if nb < 3 {
        e.cibles.get_mut(&entree).unwrap().bases.push(base);
        nb
    } else {
        let ib = e.sur(3) as usize;
        let cible = e.cibles.get_mut(&entree).unwrap();
        cible.bases[ib] = base;
        cible.corpus.retain(|(i, _)| *i != ib);
        ib
    };
    essai(e, entree, ib, Vec::new());
    for _ in 0..e.essais {
        let (ib, mut muts) = {
            let nb = e.cibles[&entree].bases.len() as u64;
            let nc = e.cibles[&entree].corpus.len() as u64;
            if nc > 0 && e.alea() % 3 != 0 {
                let k = (e.alea() % nc) as usize;
                let (ib, m) = &e.cibles[&entree].corpus[k];
                (*ib, m.clone())
            } else {
                ((e.alea() % nb) as usize, Vec::new())
            }
        };
        let Etat { cibles, alea, domaines, .. } = &mut *e;
        muter(alea, domaines, &cibles[&entree].bases[ib], &mut muts);
        essai(e, entree, ib, muts);
    }
}

/// Tables de sauts des dix classes d'objets (0x398e2-0x39ff0, W0) : gestionnaire -> (pool, type).
fn gestionnaires(b: &mut Cps2Bus) -> HashMap<u32, (usize, u8)> {
    use ddsom_game::objects::pools::POOLS;
    const TABLES: [(u32, u32); 10] = [
        (0x10de, 0x39908),
        (0x179e, 0x39992),
        (0x299e, 0x39a78),
        (0x3a1e, 0x39b12),
        (0x3c5e, 0x39b96),
        (0x549e, 0x39ce0),
        (0x6ede, 0x39e76),
        (0x4a9e, 0x39eac),
        (0x62de, 0x39f5a),
        (0x795e, 0x39fa8),
    ];
    let mut g = HashMap::new();
    for (premier, table) in TABLES {
        let pool = POOLS.iter().position(|p| p.first == premier).expect("pool");
        for t in 0..128u32 {
            let a = table + 4 * t;
            if t > 0 && instruction(a) && INSTRUCTIONS.get().is_some_and(|i| !i.is_empty()) {
                break;
            }
            let entree = (b.fetch16(a) as u32) << 16 | b.fetch16(a + 2) as u32;
            if entree >= 0x10_0000 || entree & 1 != 0 || !instruction(entree) {
                break;
            }
            g.entry(entree).or_insert((pool, t as u8));
        }
    }
    g
}

fn run(pc: u32, c: &mut Cpu, b: &mut Cps2Bus, dl: u64) -> Option<u32> {
    let cible = ETAT.with(|x| x.borrow().as_ref().unwrap().cibles.contains_key(&pc));
    if cible {
        fuzzer(pc, c, b);
    }
    ddsom_rc::run_block(pc, c, b, dl)
}

/// Zones de docs/atlas.md : (début, fin, code).
fn zones(atlas: &str) -> Vec<(u32, u32, String)> {
    let mut v = Vec::new();
    for l in std::fs::read_to_string(atlas).expect("atlas").lines() {
        let champs: Vec<&str> = l.split('|').map(str::trim).collect();
        if champs.len() > 3 && champs[1].starts_with("0x") && champs[1].len() == 17 && champs[1].as_bytes()[8] == b'-' {
            let d = u32::from_str_radix(&champs[1][2..8], 16).unwrap();
            let f = u32::from_str_radix(&champs[1][11..17], 16).unwrap();
            v.push((d, f, champs[2].to_string()));
        }
    }
    v
}

/// Sauts conditionnels du programme dont un seul côté a été exécuté par les scénarios.
fn frontiere(b: &mut Cps2Bus, union: &[u8]) -> HashMap<u32, (u32, u32)> {
    let mut f = HashMap::new();
    for w in 0..0x80000u32 {
        let pc = 2 * w;
        if !bit(union, pc) {
            continue;
        }
        let ins = m68k::decode(b, pc);
        let cible = match ins.op {
            Op::Bcc(_, t) | Op::Dbcc(_, _, t) => t,
            _ => continue,
        };
        let suite = pc + ins.len;
        if bit(union, cible) != bit(union, suite) {
            f.insert(pc, (cible, suite));
        }
    }
    f
}

/// Débuts d'instructions de listing.txt (lignes `adresse  mnémonique`) ; étiquettes (`LAB_`, `; ---- FUN_`).
fn instructions(listing: &str) -> Vec<u8> {
    let mut bm = vec![0u8; 0x10000];
    let mut etiquettes = HashSet::new();
    for l in std::fs::read_to_string(listing).expect("listing").lines() {
        for prefixe in ["LAB_", "; ---- FUN_"] {
            if let Some(h) = l.strip_prefix(prefixe) {
                if let Ok(a) = u32::from_str_radix(h.trim_end_matches(':').trim(), 16) {
                    etiquettes.insert(a);
                }
            }
        }
        if l.len() > 8 && l.as_bytes()[6] == b' ' && l.as_bytes()[7] == b' ' {
            if let Ok(a) = u32::from_str_radix(&l[..6], 16) {
                if a < 0x10_0000 {
                    poser_bit(&mut bm, a);
                }
            }
        }
    }
    let _ = ETIQUETTES.set(etiquettes);
    bm
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 7 {
        eprintln!("usage : fuzz_couv <roms> <eeprom> <scénario> <images> <cibles> <couverture.bin>");
        std::process::exit(2);
    }
    let env = |k: &str, d: u64| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
    m.set_overclock(std::env::var("DDSOM_OC").ok().and_then(|v| v.parse().ok()).unwrap_or(16.0));
    ddsom_game::install_reference(&mut m);
    m.executor = Some(run);
    let atlas = std::env::var("FUZZ_ATLAS").unwrap_or_else(|_| "docs/atlas.md".into());
    let zs = zones(&atlas);
    let zone_de = |x: u32| zs.iter().find(|(d, f, _)| (*d..*f).contains(&x)).map(|z| z.2.as_str()).unwrap_or("?");
    INSTRUCTIONS.set(std::env::var("FUZZ_LISTING").ok().map(|f| instructions(&f)).unwrap_or_default()).unwrap();
    let gestionnaires = gestionnaires(&mut m.bus.clone());
    let mut cibles = HashMap::new();
    let specs: Vec<&str> = a[5].split(',').collect();
    for n in ddsom_game::all_natives().chain(ddsom_game::hud::NATIVES_DE_TEST) {
        let voulu = |s: &str| {
            if let Some(p) = s.strip_prefix("nom:") {
                n.name.starts_with(p)
            } else if let Some(h) = s.strip_prefix("0x") {
                u32::from_str_radix(h, 16).is_ok_and(|x| x == n.entry)
            } else {
                s == "*" || s == "tout" || zone_de(n.entry) == s
            }
        };
        let exclu = specs.iter().filter_map(|s| s.strip_prefix('-')).any(voulu);
        if !exclu && specs.iter().filter(|s| !s.starts_with('-')).any(|s| voulu(s)) && !ddsom_game::kernel::NATIVES.iter().any(|k| k.entry == n.entry) {
            cibles.insert(n.entry, Cible { n, bases: Vec::new(), corpus: Vec::new(), bilan: Bilan::default(), points: 0, zone: zone_de(n.entry).to_string(), gestionnaire: gestionnaires.get(&n.entry).copied(), substitutions: 0, hors_focus: 0 });
        }
    }
    eprintln!("{} natives ciblées", cibles.len());
    let union = std::env::var("FUZZ_UNION").ok().map(|f| std::fs::read(f).expect("union")).unwrap_or_else(|| vec![0; 0x10000]);
    let front = frontiere(&mut m.bus.clone(), &union);
    UNION.set(union.clone()).unwrap();
    let graine = env("FUZZ_GRAINE", 1);
    ETAT.with(|x| {
        *x.borrow_mut() = Some(Etat {
            cibles,
            union,
            vus: vec![0; 0x10000],
            frontiere: front,
            domaines: HashMap::new(),
            substituts: std::env::var_os("FUZZ_SUBSTITUTS").is_some(),
            focus: std::env::var("FUZZ_FOCUS")
                .map(|v| zs.iter().filter(|z| v.split(',').any(|f| f == z.2)).map(|z| (z.0, z.1)).collect())
                .unwrap_or_default(),
            alea: Alea(0x9e37_79b9_7f4a_7c15 ^ graine.wrapping_mul(0x1000_0000_01b3)),
            periode: env("FUZZ_PERIODE", 16),
            points_max: env("FUZZ_POINTS", 400),
            essais: env("FUZZ_ESSAIS", 24) as u32,
            pas_max: env("FUZZ_PAS", 300_000) as u32,
            exemples: env("FUZZ_EXEMPLES", 3) as usize,
            trace: std::env::var_os("FUZZ_TRACE").is_some(),
        })
    });
    std::panic::set_hook(Box::new(|_| {}));
    let script = ddsom_game::scenario::Scenario::load(Path::new(&a[3])).expect("scénario");
    for f in 0..a[4].parse().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    let e = ETAT.with(|x| x.borrow_mut().take().unwrap());
    std::fs::write(&a[6], &e.vus).expect("sortie");
    let mut total = 0;
    let mut tri: Vec<&Cible> = e.cibles.values().filter(|c| c.bilan.essais > 0 || c.bilan.ecarts + c.bilan.paniques > 0).collect();
    tri.sort_by_key(|c| c.n.entry);
    for c in tri {
        let b = &c.bilan;
        println!(
            "{:06x} {:32} {:>6} appels {:>6} essais ({:>5} neufs) {:>5} écarts ({:>5} neufs, {:>5} plausibles) {:>4} paniques {:>4} abandons {:>4} hors code {:>5} nouvelles{}",
            c.n.entry, c.n.name, b.appels, b.essais, b.neufs, b.ecarts, b.ecarts_neufs, b.ecarts_plausibles, b.paniques, b.abandons, b.hors_code, b.nouvelles, if c.substitutions > 0 && c.substitutions < u64::MAX { format!(" ({} substitutions)", c.substitutions) } else { String::new() }
        );
        total += b.ecarts + b.paniques;
        for (s, k) in &b.signatures {
            println!("    [{k}] {s}");
        }
        for x in &b.exemples {
            println!("    {x}");
        }
    }
    println!("{}", if total == 0 { "FUZZ OK" } else { "FUZZ ÉCARTS" });
}
