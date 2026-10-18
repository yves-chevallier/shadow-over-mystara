//! Dispatch des objets (W0) : les dix boucles `0x398e2-0x39f82` qui parcourent un pool et appellent,
//! pour chaque objet actif, le gestionnaire de son type.
//!
//! Chaque boucle de l'original est la même : un emplacement dont l'octet `Obj::ACTIVE` est nul est
//! sauté, sinon on appelle le gestionnaire de `Obj::KIND`, lu dans une table de longs rangée dans le
//! code. Ici, une `Class` décrit le pool et sa table ; `update` est l'unique boucle.
//!
//! Les gestionnaires sont encore en 68000 (`call`). Pour en remplacer un par une fonction Rust, il
//! suffit de l'inscrire dans `RUST_HANDLERS` (code de classe, type, fonction) : la boucle l'appelle
//! à la place. Un gestionnaire Rust ne reçoit que l'objet.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Bus, Cpu};

use crate::mem::rts;
use crate::objects::pools::{self, Pool};
use crate::objects::Obj;
use crate::{call, Native};

/// Indicateur X du registre d'état.
const SR_X: u16 = 0x10;

/// Une classe d'objets : son pool et sa table de gestionnaires.
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

/// Gestionnaire d'un type : fonction Rust ou routine 68000.
#[derive(Clone, Copy)]
pub enum Handler {
    Rust(RustHandler),
    Rom(u32),
}

impl Class {
    /// Gestionnaire du type `kind` (l'original indexe la table par `(kind << 2) as u16`).
    pub fn handler(&self, b: &mut Cps2Bus, kind: u8) -> Handler {
        let class = self.pool.class;
        if let Some(&(_, _, f)) = RUST_HANDLERS.iter().find(|&&(c, k, _)| c == class && k == kind) {
            return Handler::Rust(f);
        }
        let at = self.table.wrapping_add(((kind as u16) << 2) as u32);
        let mut word = |a: u32| b.read_prog16(a & 0xff_ffff) as u32;
        Handler::Rom(word(at) << 16 | word(at.wrapping_add(2)))
    }

    /// Appelle le gestionnaire de chaque objet actif, dans l'ordre des emplacements. L'état d'un
    /// emplacement est lu au moment où la boucle y arrive : un objet créé par un gestionnaire dans
    /// un emplacement suivant est traité dans la même image.
    ///
    /// Les registres circulent d'un gestionnaire à l'autre comme dans la boucle d'origine (A0 =
    /// objet, D7.w = compteur, D0 = type × 4, A1 = gestionnaire) : le jeu en dépend (un gestionnaire
    /// lit le D0 laissé par le précédent) et un gestionnaire peut modifier A0 et D7.
    pub fn update(&self, c: &mut Cpu, b: &mut Cps2Bus) {
        // place de l'adresse de retour de la boucle d'origine (un gestionnaire peut la dépiler)
        c.a[7] = c.a[7].wrapping_sub(4);
        c.a[0] = self.pool.slot(0).0;
        set_low_word(&mut c.d[7], (self.pool.slots - 1) as u16);
        loop {
            let obj = Obj(c.a[0]);
            if obj.is_active(b) {
                let kind = obj.u8(b, Obj::KIND);
                c.d[0] = ((kind as u16) << 2) as u32;
                // drapeaux du `lsl.w #2,D0` qui précède le saut : N et Z du résultat, V, C et X nuls
                c.flags_logic(c.d[0], Size::Word);
                c.sr &= !SR_X;
                match self.handler(b, kind) {
                    Handler::Rust(f) => f(c, b, obj),
                    Handler::Rom(address) => {
                        c.a[1] = address;
                        call(c, b, address);
                    }
                }
            }
            c.a[0] = c.a[0].wrapping_add(self.pool.stride);
            let counter = (c.d[7] as u16).wrapping_sub(1);
            set_low_word(&mut c.d[7], counter);
            if counter == 0xffff {
                break;
            }
        }
        c.a[7] = c.a[7].wrapping_add(4);
    }
}

fn set_low_word(r: &mut u32, v: u16) {
    *r = *r & 0xffff_0000 | v as u32;
}

macro_rules! natives {
    ($($nom:ident, $name:literal, $class:expr;)*) => {$(
        pub const $nom: Native = Native {
            entry: $class.entry,
            name: $name,
            cost: |_, _| 100,
            run: |c, b| {
                $class.update(c, b);
                rts(c, b);
            },
            touches_sound: false,
            outputs: 0, // les appelants enchaînent sur d'autres appels
            exact_cost: false,
        };
    )*};
}

natives! {
    UPDATE_PSHL, "dispatch_pshl", PSHL;
    UPDATE_PEFF, "dispatch_peff", PEFF;
    UPDATE_ENEMY, "dispatch_enemy", ENEMY;
    UPDATE_BOSS, "dispatch_boss", BOSS;
    UPDATE_ESHL, "dispatch_eshl", ESHL;
    UPDATE_EFFCT, "dispatch_effct", EFFCT;
    UPDATE_BOX, "dispatch_box", BOX;
    UPDATE_SET, "dispatch_set", SET;
    UPDATE_ITEM, "dispatch_item", ITEM;
    UPDATE_SOBJ, "dispatch_sobj", SOBJ;
}

/// Les dix boucles de dispatch, pour les appelants 68000 qui subsistent.
pub const NATIVES: &[Native] = &[
    UPDATE_PSHL, UPDATE_PEFF, UPDATE_ENEMY, UPDATE_BOSS, UPDATE_ESHL, UPDATE_EFFCT, UPDATE_BOX, UPDATE_SET,
    UPDATE_ITEM, UPDATE_SOBJ,
];
