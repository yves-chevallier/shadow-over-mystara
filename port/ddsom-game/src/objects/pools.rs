//! Les 10 pools d'objets : initialisation, allocation, libération.
//!
//! Un pool est un tableau d'emplacements de taille fixe dans la RAM de travail. Un emplacement est
//! libre quand son octet `Obj::ACTIVE` est nul. Un compteur de places libres, tenu à part, permet
//! de refuser rapidement une allocation ; l'allocation ne rend pas l'objet actif (c'est l'appelant
//! qui écrit `ACTIVE`) : elle en fixe seulement la classe (`Obj::CLASS`).
//!
//! Quatre pools (tirs joueur, ennemis et boss, tirs ennemis, boîtes, objets) ont aussi une file de
//! candidats aux collisions (`Queue`), remise à vide à l'initialisation.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use super::cost::{walk_cycles, Trace};
use super::lists::{self, plan_removal};
use super::Obj;
use crate::mem::{rd16, rts, wr16, wr32, wr8, A5};
use crate::{regs, Native};

/// File des objets candidats aux collisions d'une catégorie : nombre d'entrées, pointeur
/// d'écriture (adresse RAM sur 16 bits) et tableau de pointeurs d'objets (offsets depuis A5).
pub struct Queue {
    pub count: u32,
    pub cursor: u32,
    pub array: u32,
}

impl Queue {
    pub fn reset(&self, b: &mut Cps2Bus) {
        wr16(b, A5 + self.cursor, (A5 + self.array) as u16);
        wr16(b, A5 + self.count, 0);
    }
}

pub const QUEUE_PSHL: Queue = Queue { count: 0x7e72, cursor: 0x7e7c, array: 0x7e86 };
pub const QUEUE_ENEMY_BOSS: Queue = Queue { count: 0x7e76, cursor: 0x7e80, array: 0x7ef6 };
pub const QUEUE_ESHL: Queue = Queue { count: 0x7e74, cursor: 0x7e7e, array: 0x7eaa };
pub const QUEUE_BOX: Queue = Queue { count: 0x7e78, cursor: 0x7e82, array: 0x7f28 };
pub const QUEUE_ITEM: Queue = Queue { count: 0x7e7a, cursor: 0x7e84, array: 0x7f52 };

/// Un pool d'objets.
pub struct Pool {
    /// Nom donné par le jeu (écran d'exception).
    pub name: &'static str,
    /// Code de classe écrit dans `Obj::CLASS` à l'allocation.
    pub class: u8,
    /// Premier emplacement (offset depuis A5), taille d'un emplacement, nombre d'emplacements.
    pub first: u32,
    pub stride: u32,
    pub slots: u32,
    /// Compteur de places libres (offset depuis A5, mot).
    pub free: u32,
    /// Les objets du pool figurent dans les listes triées : la libération les en retire.
    pub listed: bool,
    pub queue: Option<&'static Queue>,
    /// Routines d'allocation et de libération de l'original (adresses d'entrée de leurs natives).
    pub alloc_routine: u32,
    pub free_routine: u32,
}

/// Tirs du joueur.
pub const PSHL: Pool = Pool { name: "PSHL", class: 6, first: 0x10de, stride: 0x60, slots: 18, free: 0x7e5e, listed: true, queue: Some(&QUEUE_PSHL), alloc_routine: 0x225e, free_routine: 0x21d8 };
/// Effets liés au joueur.
pub const PEFF: Pool = Pool { name: "PEFF", class: 0x12, first: 0x179e, stride: 0x60, slots: 48, free: 0x7e60, listed: true, queue: None, alloc_routine: 0x228e, free_routine: 0x21e6 };
/// Ennemis.
pub const ENEMY: Pool = Pool { name: "ENEMY", class: 2, first: 0x299e, stride: 0xc0, slots: 22, free: 0x7e62, listed: true, queue: Some(&QUEUE_ENEMY_BOSS), alloc_routine: 0x22bc, free_routine: 0x21f4 };
/// Boss (file de collisions partagée avec les ennemis).
pub const BOSS: Pool = Pool { name: "BOSS", class: 4, first: 0x3a1e, stride: 0xc0, slots: 3, free: 0x7e6c, listed: true, queue: Some(&QUEUE_ENEMY_BOSS), alloc_routine: 0x23a8, free_routine: 0x2234 };
/// Tirs et attaques ennemis.
pub const ESHL: Pool = Pool { name: "ESHL", class: 8, first: 0x3c5e, stride: 0x60, slots: 38, free: 0x7e64, listed: true, queue: Some(&QUEUE_ESHL), alloc_routine: 0x22ea, free_routine: 0x2202 };
/// Décors et mises en place (hors listes triées).
pub const SET: Pool = Pool { name: "SET", class: 0x10, first: 0x4a9e, stride: 0x50, slots: 32, free: 0x7e66, listed: false, queue: None, alloc_routine: 0x2348, free_routine: 0x2210 };
/// Effets visuels.
pub const EFFCT: Pool = Pool { name: "EFFCT", class: 0xe, first: 0x549e, stride: 0x60, slots: 38, free: 0x7e68, listed: true, queue: None, alloc_routine: 0x2318, free_routine: 0x2218 };
/// Objets ramassables.
pub const ITEM: Pool = Pool { name: "ITEM", class: 0xc, first: 0x62de, stride: 0x60, slots: 32, free: 0x7e6e, listed: true, queue: Some(&QUEUE_ITEM), alloc_routine: 0x23d8, free_routine: 0x2242 };
/// Caisses et coffres.
pub const BOX: Pool = Pool { name: "BOX", class: 0xa, first: 0x6ede, stride: 0x80, slots: 21, free: 0x7e6a, listed: true, queue: Some(&QUEUE_BOX), alloc_routine: 0x2378, free_routine: 0x2226 };
/// Objets système et interface.
pub const SOBJ: Pool = Pool { name: "SOBJ", class: 0x14, first: 0x795e, stride: 0x50, slots: 16, free: 0x7e70, listed: true, queue: None, alloc_routine: 0x2408, free_routine: 0x2250 };

/// Les pools dans l'ordre de leur initialisation (`0x2062`).
pub const POOLS: [&Pool; 10] = [&PSHL, &PEFF, &ENEMY, &BOSS, &ESHL, &SET, &EFFCT, &BOX, &ITEM, &SOBJ];

/// Pool dont `routine` est la routine d'allocation : certaines tables de la ROM désignent le pool d'un objet à créer
/// par l'adresse de sa routine d'allocation.
pub fn pool_of_alloc_routine(routine: u32) -> &'static Pool {
    POOLS.into_iter().find(|p| p.alloc_routine == routine).unwrap_or_else(|| panic!("{routine:#x} : pas une routine d'allocation"))
}

/// Pool correspondant à un code de classe (`Obj::CLASS`).
pub fn pool_of_class(class: u8) -> Option<&'static Pool> {
    POOLS.into_iter().find(|p| p.class == class)
}

/// Résultat de la recherche d'une place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scan {
    /// Compteur de places libres nul : pas de recherche.
    NoneLeft,
    /// Emplacement libre d'indice donné.
    Found(u32),
    /// Compteur non nul mais aucun emplacement libre.
    Exhausted,
}

impl Pool {
    pub fn slot(&self, i: u32) -> Obj {
        Obj(A5 + self.first + i * self.stride)
    }

    /// Adresse qui suit le dernier emplacement.
    pub fn end(&self) -> u32 {
        A5 + self.first + self.slots * self.stride
    }

    pub fn free_count(&self, b: &mut Cps2Bus) -> u16 {
        rd16(b, A5 + self.free)
    }

    /// Emplacements du pool, dans l'ordre.
    pub fn iter(&self) -> impl Iterator<Item = Obj> + '_ {
        (0..self.slots).map(|i| self.slot(i))
    }

    /// Vide tous les emplacements et remet les compteurs à neuf.
    pub fn reset(&self, b: &mut Cps2Bus) {
        for i in 0..self.slots {
            self.clear_slot(b, self.slot(i));
        }
        if let Some(q) = self.queue {
            q.reset(b);
        }
        wr16(b, A5 + self.free, self.slots as u16);
    }

    fn clear_slot(&self, b: &mut Cps2Bus, obj: Obj) {
        for off in (0..self.stride).step_by(4) {
            wr32(b, obj.at(off), 0);
        }
    }

    fn scan(&self, b: &mut Cps2Bus) -> Scan {
        if self.free_count(b) == 0 {
            return Scan::NoneLeft;
        }
        match (0..self.slots).find(|&i| !self.slot(i).is_active(b)) {
            Some(i) => Scan::Found(i),
            None => Scan::Exhausted,
        }
    }

    /// Réserve un emplacement : le compteur baisse et la classe est écrite. L'objet n'est pas
    /// encore actif.
    pub fn alloc(&self, b: &mut Cps2Bus) -> Option<Obj> {
        match self.scan(b) {
            Scan::Found(i) => {
                let obj = self.slot(i);
                let left = self.free_count(b);
                wr16(b, A5 + self.free, left.wrapping_sub(1));
                obj.set_u8(b, Obj::CLASS, self.class);
                Some(obj)
            }
            _ => None,
        }
    }

    /// Libère un objet : retiré des listes triées, compteur incrémenté, emplacement remis à zéro.
    pub fn release(&self, b: &mut Cps2Bus, obj: Obj) {
        if self.listed {
            lists::unlink(b, obj);
        }
        let left = self.free_count(b);
        wr16(b, A5 + self.free, left.wrapping_add(1));
        self.clear_slot(b, obj);
    }

    /// Décisions de l'original pour la recherche d'une place.
    fn scan_trace(&self, scan: Scan, t: &mut Trace) {
        match scan {
            Scan::NoneLeft => {
                t.take(true);
            }
            Scan::Found(i) => {
                t.take(false);
                for _ in 0..i {
                    t.take(false);
                    t.take(true);
                }
                t.take(true);
            }
            Scan::Exhausted => {
                t.take(false);
                for j in 0..self.slots {
                    t.take(false);
                    t.take(j + 1 < self.slots);
                }
            }
        }
    }

    /// Décisions de l'original pour remettre à zéro un emplacement (boucle de 32 octets).
    fn clear_trace(&self, t: &mut Trace) {
        let passes = self.stride / 32;
        for j in 0..passes {
            t.take(j + 1 < passes);
        }
    }

    /// Décisions de l'original pour `reset` (une boucle `dbf` autour de la remise à zéro).
    fn reset_trace(&self, t: &mut Trace) {
        for i in 0..self.slots {
            self.clear_trace(t);
            t.take(i + 1 < self.slots);
        }
    }
}

// ---- adaptation aux registres du 68000 ----

/// Masques des registres que l'original laisse (voir `diag::registres_morts`).
pub const REGS_ALLOC: u32 = regs::a(1) | regs::d(6) | regs::CCR;
pub const REGS_FREE: u32 = regs::d(0) | regs::d(1) | regs::a(4) | regs::CCR;

impl Pool {
    /// Comme `alloc`, et laisse les registres de l'original : A1 = emplacement trouvé (en cas d'échec : le premier
    /// emplacement si le compteur est nul, la fin du pool si aucune place n'est libre), D6 = compteur de la boucle,
    /// retenue = échec. Pour les appelants pas encore libérés des registres (`docs/guide-refactorisation.md`).
    pub fn alloc_68k(&self, c: &mut Cpu, b: &mut Cps2Bus) -> Option<Obj> {
        let (r, _) = self.alloc_regs(c, b);
        crate::diag::registres_morts(c, "alloc", REGS_ALLOC);
        r
    }

    /// Allocation de l'original, registres compris : rend aussi l'A1 laissé.
    fn alloc_regs(&self, c: &mut Cpu, b: &mut Cps2Bus) -> (Option<Obj>, Obj) {
        let r = match self.scan(b) {
            Scan::NoneLeft => {
                c.a[1] = A5 + self.first;
                c.set_ccr(1);
                None
            }
            Scan::Found(i) => {
                let obj = self.slot(i);
                let left = self.free_count(b);
                let new = c.addsub(true, 1, left as u32, false, Size::Word, false);
                wr16(b, A5 + self.free, new as u16);
                wr8(b, obj.at(Obj::CLASS), self.class);
                c.a[1] = obj.0;
                c.d[6] = self.slots - 1 - i;
                // or.b D0b,D0b
                c.flags_logic(c.d[0], Size::Byte);
                Some(obj)
            }
            Scan::Exhausted => {
                c.a[1] = self.end();
                c.d[6] = 0xffff;
                c.set_ccr(1);
                None
            }
        };
        (r, Obj(c.a[1]))
    }

    /// Allocation dont l'appelant d'origine ne teste pas l'échec : l'objet rendu est l'A1 que l'original laisse.
    /// Glitch d'origine : pool plein, l'appelant écrit dans le premier emplacement (compteur nul, objet peut-être
    /// actif) ou juste après le dernier (dans le pool suivant).
    pub fn alloc_unchecked(&self, b: &mut Cps2Bus) -> Obj {
        self.alloc_or_leftover(b).unwrap_or_else(|a1| a1)
    }

    /// Allocation dont l'échec rend aussi l'A1 que l'original laisse : le premier emplacement (compteur nul) ou la fin du
    /// pool (aucune place libre). Pour les appelants d'origine qui relisent A1 après un échec.
    pub fn alloc_or_leftover(&self, b: &mut Cps2Bus) -> Result<Obj, Obj> {
        match self.scan(b) {
            Scan::NoneLeft => Err(Obj(A5 + self.first)),
            Scan::Found(_) => Ok(self.alloc(b).expect("place trouvée")),
            Scan::Exhausted => Err(Obj(self.end())),
        }
    }

    /// `alloc_unchecked` avec les registres de l'original (voir `alloc_68k`).
    pub fn alloc_unchecked_68k(&self, c: &mut Cpu, b: &mut Cps2Bus) -> Obj {
        let (_, a1) = self.alloc_regs(c, b);
        crate::diag::registres_morts(c, "alloc", REGS_ALLOC);
        a1
    }

    /// Comme `release`, et laisse les registres de l'original : D0 = 0, D1.w = -1, A4 = fin de l'emplacement,
    /// drapeaux de `tst.l D0`. A0 (l'objet) n'est pas touché.
    pub fn release_68k(&self, c: &mut Cpu, b: &mut Cps2Bus, obj: Obj) {
        self.release_regs(c, b, obj);
        crate::diag::registres_morts(c, "free", REGS_FREE);
    }

    fn release_regs(&self, c: &mut Cpu, b: &mut Cps2Bus, obj: Obj) {
        if self.listed {
            lists::unlink(b, obj);
        }
        let left = self.free_count(b);
        let new = c.addsub(false, 1, left as u32, false, Size::Word, false);
        wr16(b, A5 + self.free, new as u16);
        self.clear_slot(b, obj);
        c.flags_logic(0, Size::Long);
        c.d[0] = 0;
        c.d[1] |= 0xffff;
        c.a[4] = obj.0 + self.stride;
    }
}

/// Allocation : A1 = emplacement trouvé, C si échec ; D6 sert de compteur de boucle. Les natives, appelées par
/// adresse, rendent toujours les registres (jamais empoisonnés).
fn alloc_native(pool: &Pool, c: &mut Cpu, b: &mut Cps2Bus) {
    pool.alloc_regs(c, b);
    rts(c, b);
}

fn alloc_cost(pool: &Pool, entry: u32, b: &mut Cps2Bus) -> u64 {
    let mut t = Trace::new();
    let scan = pool.scan(b);
    pool.scan_trace(scan, &mut t);
    walk_cycles(b, entry, &t)
}

/// Libération : A0 = objet. L'original laisse D0 = 0, D1.w = -1, A4 = fin de l'emplacement.
fn free_native(pool: &Pool, c: &mut Cpu, b: &mut Cps2Bus) {
    pool.release_regs(c, b, Obj(c.a[0]));
    rts(c, b);
}

fn free_cost(pool: &Pool, entry: u32, c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let mut t = Trace::new();
    if pool.listed {
        plan_removal(b, Obj(c.a[0]), &mut t);
    }
    pool.clear_trace(&mut t);
    walk_cycles(b, entry, &t)
}

/// Initialisation de pools consécutifs. Laisse D0 = 0, D1.w = D7.w = -1 et A0 = A4 = fin du
/// dernier emplacement (A0 = tableau de la file de collisions si le dernier pool en a une).
fn init_native(pools: &[&Pool], c: &mut Cpu, b: &mut Cps2Bus) {
    for pool in pools {
        pool.reset(b);
    }
    let last = pools[pools.len() - 1];
    c.d[0] = 0;
    c.d[1] |= 0xffff;
    c.d[7] |= 0xffff;
    c.a[4] = last.end();
    c.a[0] = last.queue.map_or(last.end(), |q| A5 + q.array);
    c.flags_logic(last.slots, Size::Word);
    rts(c, b);
}

fn init_cost(pools: &[&Pool], entry: u32, b: &mut Cps2Bus) -> u64 {
    let mut t = Trace::new();
    for pool in pools {
        pool.reset_trace(&mut t);
    }
    walk_cycles(b, entry, &t)
}

macro_rules! pool_natives {
    ($pool:ident, $alloc:ident, $alloc_name:expr, $free:ident, $free_name:expr) => {
        pub const $alloc: Native = Native {
            entry: $pool.alloc_routine,
            name: $alloc_name,
            cost: |_, b| alloc_cost(&$pool, $pool.alloc_routine, b),
            run: |c, b| alloc_native(&$pool, c, b),
            touches_sound: false,
            outputs: regs::a(1) | regs::d(6) | regs::CCR,
            exact_cost: true,
        };
        pub const $free: Native = Native {
            entry: $pool.free_routine,
            name: $free_name,
            cost: |c, b| free_cost(&$pool, $pool.free_routine, c, b),
            run: |c, b| free_native(&$pool, c, b),
            touches_sound: false,
            outputs: regs::d(0) | regs::d(1) | regs::a(0) | regs::a(4) | regs::CCR,
            exact_cost: true,
        };
    };
}

pool_natives!(PSHL, ALLOC_PSHL, "pool_alloc_pshl", FREE_PSHL, "pool_free_pshl");
pool_natives!(PEFF, ALLOC_PEFF, "pool_alloc_peff", FREE_PEFF, "pool_free_peff");
pool_natives!(ENEMY, ALLOC_ENEMY, "pool_alloc_enemy", FREE_ENEMY, "pool_free_enemy");
pool_natives!(ESHL, ALLOC_ESHL, "pool_alloc_eshl", FREE_ESHL, "pool_free_eshl");
pool_natives!(EFFCT, ALLOC_EFFCT, "pool_alloc_effct", FREE_EFFCT, "pool_free_effct");
pool_natives!(SET, ALLOC_SET, "pool_alloc_set", FREE_SET, "pool_free_set");
pool_natives!(BOX, ALLOC_BOX, "pool_alloc_box", FREE_BOX, "pool_free_box");
pool_natives!(BOSS, ALLOC_BOSS, "pool_alloc_boss", FREE_BOSS, "pool_free_boss");
pool_natives!(ITEM, ALLOC_ITEM, "pool_alloc_item", FREE_ITEM, "pool_free_item");
pool_natives!(SOBJ, ALLOC_SOBJ, "pool_alloc_sobj", FREE_SOBJ, "pool_free_sobj");

macro_rules! init_native {
    ($const:ident, $entry:expr, $name:expr, $pools:expr) => {
        pub const $const: Native = Native {
            entry: $entry,
            name: $name,
            cost: |_, b| init_cost(&$pools, $entry, b),
            run: |c, b| init_native(&$pools, c, b),
            touches_sound: false,
            outputs: regs::ALL,
            exact_cost: true,
        };
    };
}

// 0x2062 : initialise les 10 pools.
init_native!(INIT_ALL, 0x2062, "pools_init_all", POOLS);
// 0x2134 : initialise le pool SET.
init_native!(INIT_SET, 0x2134, "pool_init_set", [&SET]);
// 0x2150 : initialise le pool EFFCT.
init_native!(INIT_EFFCT, 0x2150, "pool_init_effct", [&EFFCT]);
// 0x21bc : initialise le pool SOBJ.
init_native!(INIT_SOBJ, 0x21bc, "pool_init_sobj", [&SOBJ]);
