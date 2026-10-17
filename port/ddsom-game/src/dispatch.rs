//! Dispatch des objets (W0) : les dix boucles `0x398e2-0x39f82` qui parcourent un pool et appellent,
//! pour chaque objet actif, le gestionnaire de son type.
//!
//! Chaque boucle de l'original est la même : `A0` parcourt les emplacements, `D7` compte (`dbf`) ;
//! un emplacement dont l'octet `Obj::ACTIVE` est nul est sauté, sinon `jsr (table + 4 * KIND)`. La
//! table de sauts est rangée dans le code, juste après la boucle (lecture relative au PC).
//!
//! Les gestionnaires sont encore en 68000 : `call` les exécute. Pour en remplacer un par une
//! fonction Rust, il suffit de l'inscrire dans `RUST_HANDLERS` (classe, type, fonction) : la boucle
//! l'appelle à la place. Un gestionnaire Rust reçoit l'objet, ajoute lui-même sa durée à
//! `c.cycles`, et n'a pas à restaurer de registre (la boucle n'en dépend plus).
//!
//! Durée : la boucle ajoute elle-même sa durée propre (parcours, `jsr`) à `c.cycles`, car elle
//! dépend des emplacements actifs à l'instant où elle les rencontre (un gestionnaire peut en créer
//! ou en libérer). Les gestionnaires comptent leur durée eux-mêmes, `rts` compris.

use std::cell::RefCell;
use std::collections::HashMap;

use cps2::Cps2Bus;
use m68k::{Bus, Cpu};

use crate::garde::Garde;
use crate::mem::rts;
use crate::objects::cost::{walk_cycles, Trace};
use crate::objects::pools::{self, Pool};
use crate::objects::Obj;
use crate::{call, Native};

/// Une classe d'objets : son pool, sa table de gestionnaires et la boucle de l'original.
pub struct Class {
    pub pool: &'static Pool,
    /// Table de gestionnaires (longs, un par type), dans le code.
    pub table: u32,
    /// Adresse de la boucle de dispatch de l'original.
    pub entry: u32,
}

pub const PSHL: Class = Class { pool: &pools::PSHL, table: 0x39908, entry: 0x398e2 };
pub const PEFF: Class = Class { pool: &pools::PEFF, table: 0x39992, entry: 0x3996c };
pub const ENEMY: Class = Class { pool: &pools::ENEMY, table: 0x39a78, entry: 0x39a52 };
pub const BOSS: Class = Class { pool: &pools::BOSS, table: 0x39b12, entry: 0x39aec };
pub const ESHL: Class = Class { pool: &pools::ESHL, table: 0x39b96, entry: 0x39b70 };
pub const EFFCT: Class = Class { pool: &pools::EFFCT, table: 0x39ce0, entry: 0x39cba };
pub const BOX: Class = Class { pool: &pools::BOX, table: 0x39e76, entry: 0x39e50 };
pub const SET: Class = Class { pool: &pools::SET, table: 0x39eac, entry: 0x39e86 };
pub const ITEM: Class = Class { pool: &pools::ITEM, table: 0x39f5a, entry: 0x39f34 };
pub const SOBJ: Class = Class { pool: &pools::SOBJ, table: 0x39fa8, entry: 0x39f82 };

/// Gestionnaire d'objet réécrit en Rust.
pub type RustHandler = fn(&mut Cpu, &mut Cps2Bus, Obj);

/// Gestionnaires réécrits : (code de classe, type, fonction). Vide tant qu'aucun type n'est réécrit.
pub const RUST_HANDLERS: &[(u8, u8, RustHandler)] = &[];

/// Gestionnaire d'un type : routine 68000 ou fonction Rust.
#[derive(Clone, Copy)]
pub enum Handler {
    Rom(u32),
    Rust(RustHandler),
}

impl Class {
    /// Gestionnaire du type `kind`. L'original indexe la table par `(kind << 2) as u16`.
    pub fn handler(&self, b: &mut Cps2Bus, kind: u8) -> Handler {
        if let Some(&(_, _, f)) = RUST_HANDLERS.iter().find(|&&(c, k, _)| c == self.pool.class && k == kind) {
            return Handler::Rust(f);
        }
        let at = self.table.wrapping_add(((kind as u16) << 2) as u32);
        let mut word = |a: u32| b.read_prog16(a & 0xff_ffff) as u32;
        Handler::Rom(word(at) << 16 | word(at.wrapping_add(2)))
    }
}

/// Durées propres de la boucle de l'original (`jsr (A1)` compris, gestionnaire exclu).
struct LoopCost {
    /// Un emplacement vide, qui est aussi le dernier.
    one_empty: u64,
    /// Supplément par emplacement vide non final.
    per_empty: u64,
    /// Supplément d'un emplacement actif par rapport à un vide.
    per_active: u64,
}

fn loop_cost(b: &mut Cps2Bus, class: &Class) -> LoopCost {
    thread_local! {
        static CACHE: RefCell<HashMap<u32, (u64, u64, u64)>> = RefCell::new(HashMap::new());
    }
    let cached = CACHE.with(|cache| cache.borrow().get(&class.entry).copied());
    let (one_empty, two_empty, one_active) = cached.unwrap_or_else(|| {
        // décisions par emplacement : `beq` pris (emplacement vide), puis `dbf` qui boucle
        let mut walk = |decisions: &[bool]| {
            let mut t = Trace::new();
            for &d in decisions {
                t.take(d);
            }
            walk_cycles(b, class.entry, &t)
        };
        let v = (walk(&[true, false]), walk(&[true, true, true, false]), walk(&[false, false]));
        CACHE.with(|cache| cache.borrow_mut().insert(class.entry, v));
        v
    });
    LoopCost { one_empty, per_empty: two_empty - one_empty, per_active: one_active - one_empty }
}

/// Parcourt le pool de la classe et appelle le gestionnaire de chaque objet actif.
///
/// Laisse les registres comme l'original : `A0` après le dernier emplacement, `D7.w` = 0xffff,
/// `D0` et `A1` du dernier objet actif. Ajoute la durée propre de la boucle à `c.cycles`.
pub fn run_class(c: &mut Cpu, b: &mut Cps2Bus, class: &Class) {
    let pool = class.pool;
    c.a[0] = pool.slot(0).0;
    c.d[7] = c.d[7] & 0xffff_0000 | (pool.slots - 1);
    let (mut visited, mut active) = (0u64, 0u64);
    loop {
        let obj = Obj(c.a[0]);
        visited += 1;
        if obj.is_active(b) {
            active += 1;
            let kind = obj.u8(b, Obj::KIND);
            c.d[0] = ((kind as u16) << 2) as u32;
            match class.handler(b, kind) {
                Handler::Rom(addr) => {
                    c.a[1] = addr;
                    call(c, b, addr);
                }
                Handler::Rust(f) => f(c, b, obj),
            }
        }
        // `lea (stride,A0),A0` puis `dbf D7`
        c.a[0] = c.a[0].wrapping_add(pool.stride);
        let d7 = (c.d[7] as u16).wrapping_sub(1);
        c.d[7] = c.d[7] & 0xffff_0000 | d7 as u32;
        if d7 == 0xffff {
            break;
        }
    }
    let cost = loop_cost(b, class);
    c.cycles += cost.one_empty + (visited - 1) * cost.per_empty + active * cost.per_active;
}

/// Une garde par boucle (voir `garde`) : la boucle compte elle-même ses cycles.
static GARDES: [Garde; 10] = [const { Garde::new() }; 10];

macro_rules! natives {
    ($($nom:ident, $fonction:ident, $name:literal, $class:expr, $garde:literal;)*) => {$(
        fn $fonction(c: &mut Cpu, b: &mut Cps2Bus) {
            run_class(c, b, &$class);
            rts(c, b);
        }

        pub const $nom: Native = Native {
            entry: $class.entry,
            name: $name,
            cost: |c, b| GARDES[$garde].cout(c, b, $fonction),
            run: |c, b| GARDES[$garde].executer(c, b, $fonction),
            touches_sound: false,
            outputs: 0, // les appelants enchaînent sur d'autres appels
            exact_cost: true,
        };
    )*};
}

natives! {
    DISPATCH_PSHL, dispatch_pshl, "dispatch_pshl", PSHL, 0;
    DISPATCH_PEFF, dispatch_peff, "dispatch_peff", PEFF, 1;
    DISPATCH_ENEMY, dispatch_enemy, "dispatch_enemy", ENEMY, 2;
    DISPATCH_BOSS, dispatch_boss, "dispatch_boss", BOSS, 3;
    DISPATCH_ESHL, dispatch_eshl, "dispatch_eshl", ESHL, 4;
    DISPATCH_EFFCT, dispatch_effct, "dispatch_effct", EFFCT, 5;
    DISPATCH_BOX, dispatch_box, "dispatch_box", BOX, 6;
    DISPATCH_SET, dispatch_set, "dispatch_set", SET, 7;
    DISPATCH_ITEM, dispatch_item, "dispatch_item", ITEM, 8;
    DISPATCH_SOBJ, dispatch_sobj, "dispatch_sobj", SOBJ, 9;
}

/// Les dix boucles de dispatch.
pub const NATIVES: &[Native] = &[
    DISPATCH_PSHL,
    DISPATCH_PEFF,
    DISPATCH_ENEMY,
    DISPATCH_BOSS,
    DISPATCH_ESHL,
    DISPATCH_EFFCT,
    DISPATCH_BOX,
    DISPATCH_SET,
    DISPATCH_ITEM,
    DISPATCH_SOBJ,
];
