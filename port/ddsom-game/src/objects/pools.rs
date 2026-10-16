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
}

/// Tirs du joueur.
pub const PSHL: Pool = Pool { name: "PSHL", class: 6, first: 0x10de, stride: 0x60, slots: 18, free: 0x7e5e, listed: true, queue: Some(&QUEUE_PSHL) };
/// Effets liés au joueur.
pub const PEFF: Pool = Pool { name: "PEFF", class: 0x12, first: 0x179e, stride: 0x60, slots: 48, free: 0x7e60, listed: true, queue: None };
/// Ennemis.
pub const ENEMY: Pool = Pool { name: "ENEMY", class: 2, first: 0x299e, stride: 0xc0, slots: 22, free: 0x7e62, listed: true, queue: Some(&QUEUE_ENEMY_BOSS) };
/// Boss (file de collisions partagée avec les ennemis).
pub const BOSS: Pool = Pool { name: "BOSS", class: 4, first: 0x3a1e, stride: 0xc0, slots: 3, free: 0x7e6c, listed: true, queue: Some(&QUEUE_ENEMY_BOSS) };
/// Tirs et attaques ennemis.
pub const ESHL: Pool = Pool { name: "ESHL", class: 8, first: 0x3c5e, stride: 0x60, slots: 38, free: 0x7e64, listed: true, queue: Some(&QUEUE_ESHL) };
/// Décors et mises en place (hors listes triées).
pub const SET: Pool = Pool { name: "SET", class: 0x10, first: 0x4a9e, stride: 0x50, slots: 32, free: 0x7e66, listed: false, queue: None };
/// Effets visuels.
pub const EFFCT: Pool = Pool { name: "EFFCT", class: 0xe, first: 0x549e, stride: 0x60, slots: 38, free: 0x7e68, listed: true, queue: None };
/// Objets ramassables.
pub const ITEM: Pool = Pool { name: "ITEM", class: 0xc, first: 0x62de, stride: 0x60, slots: 32, free: 0x7e6e, listed: true, queue: Some(&QUEUE_ITEM) };
/// Caisses et coffres.
pub const BOX: Pool = Pool { name: "BOX", class: 0xa, first: 0x6ede, stride: 0x80, slots: 21, free: 0x7e6a, listed: true, queue: Some(&QUEUE_BOX) };
/// Objets système et interface.
pub const SOBJ: Pool = Pool { name: "SOBJ", class: 0x14, first: 0x795e, stride: 0x50, slots: 16, free: 0x7e70, listed: true, queue: None };

/// Les pools dans l'ordre de leur initialisation (`0x2062`).
pub const POOLS: [&Pool; 10] = [&PSHL, &PEFF, &ENEMY, &BOSS, &ESHL, &SET, &EFFCT, &BOX, &ITEM, &SOBJ];

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

/// Allocation : A1 = emplacement trouvé, C si échec ; D6 sert de compteur de boucle.
fn alloc_native(pool: &Pool, c: &mut Cpu, b: &mut Cps2Bus) {
    let scan = pool.scan(b);
    match scan {
        Scan::NoneLeft => {
            c.a[1] = A5 + pool.first;
            c.set_ccr(1);
        }
        Scan::Found(i) => {
            let obj = pool.slot(i);
            let left = pool.free_count(b);
            let new = c.addsub(true, 1, left as u32, false, Size::Word, false);
            wr16(b, A5 + pool.free, new as u16);
            wr8(b, obj.at(Obj::CLASS), pool.class);
            c.a[1] = obj.0;
            c.d[6] = pool.slots - 1 - i;
            // or.b D0b,D0b
            c.flags_logic(c.d[0], Size::Byte);
        }
        Scan::Exhausted => {
            c.a[1] = pool.end();
            c.d[6] = 0xffff;
            c.set_ccr(1);
        }
    }
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
    let obj = Obj(c.a[0]);
    if pool.listed {
        lists::unlink(b, obj);
    }
    let left = pool.free_count(b);
    let new = c.addsub(false, 1, left as u32, false, Size::Word, false);
    wr16(b, A5 + pool.free, new as u16);
    pool.clear_slot(b, obj);
    c.flags_logic(0, Size::Long);
    c.d[0] = 0;
    c.d[1] |= 0xffff;
    c.a[4] = obj.0 + pool.stride;
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
    ($pool:ident, $alloc:ident, $alloc_entry:expr, $alloc_name:expr, $free:ident, $free_entry:expr, $free_name:expr) => {
        pub const $alloc: Native = Native {
            entry: $alloc_entry,
            name: $alloc_name,
            cost: |_, b| alloc_cost(&$pool, $alloc_entry, b),
            run: |c, b| alloc_native(&$pool, c, b),
            touches_sound: false,
            outputs: regs::a(1) | regs::d(6) | regs::CCR,
            exact_cost: true,
        };
        pub const $free: Native = Native {
            entry: $free_entry,
            name: $free_name,
            cost: |c, b| free_cost(&$pool, $free_entry, c, b),
            run: |c, b| free_native(&$pool, c, b),
            touches_sound: false,
            outputs: regs::d(0) | regs::d(1) | regs::a(0) | regs::a(4) | regs::CCR,
            exact_cost: true,
        };
    };
}

pool_natives!(PSHL, ALLOC_PSHL, 0x225e, "pool_alloc_pshl", FREE_PSHL, 0x21d8, "pool_free_pshl");
pool_natives!(PEFF, ALLOC_PEFF, 0x228e, "pool_alloc_peff", FREE_PEFF, 0x21e6, "pool_free_peff");
pool_natives!(ENEMY, ALLOC_ENEMY, 0x22bc, "pool_alloc_enemy", FREE_ENEMY, 0x21f4, "pool_free_enemy");
pool_natives!(ESHL, ALLOC_ESHL, 0x22ea, "pool_alloc_eshl", FREE_ESHL, 0x2202, "pool_free_eshl");
pool_natives!(EFFCT, ALLOC_EFFCT, 0x2318, "pool_alloc_effct", FREE_EFFCT, 0x2218, "pool_free_effct");
pool_natives!(SET, ALLOC_SET, 0x2348, "pool_alloc_set", FREE_SET, 0x2210, "pool_free_set");
pool_natives!(BOX, ALLOC_BOX, 0x2378, "pool_alloc_box", FREE_BOX, 0x2226, "pool_free_box");
pool_natives!(BOSS, ALLOC_BOSS, 0x23a8, "pool_alloc_boss", FREE_BOSS, 0x2234, "pool_free_boss");
pool_natives!(ITEM, ALLOC_ITEM, 0x23d8, "pool_alloc_item", FREE_ITEM, 0x2242, "pool_free_item");
pool_natives!(SOBJ, ALLOC_SOBJ, 0x2408, "pool_alloc_sobj", FREE_SOBJ, 0x2250, "pool_free_sobj");

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
