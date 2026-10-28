//! Dispatch des objets (W0) : les dix boucles `0x398e2-0x39f82` qui parcourent un pool et appellent,
//! pour chaque objet actif, le gestionnaire de son type.
//!
//! Chaque boucle de l'original est la même : un emplacement dont l'octet `Obj::ACTIVE` est nul est
//! sauté, sinon on appelle le gestionnaire de `Obj::KIND`, lu dans une table de longs rangée dans le
//! code. Ici, une `Class` décrit le pool et sa table ; `update` est l'unique boucle. Les gestionnaires sont des
//! natives, désignées par leur adresse dans la table de la ROM.
//!
//! Un gestionnaire peut faire céder la main à la tâche (fondu au noir des portes, feu d'artifice) : la boucle est alors
//! abandonnée, et la tâche la reprendra à l'étape [`Etape::Aiguillage`] (après l'emplacement courant, avec A0 et D7
//! rendus par le contexte de la tâche), puis à la suite de l'appelant de la boucle.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::mem::rts;
use crate::modes::Flow;
use crate::objects::pools::{self, Pool};
use crate::objects::Obj;
use crate::Native;

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

/// Les dix classes, dans l'ordre des boucles d'origine (indice de [`Etape::Aiguillage`]).
const CLASSES: [&Class; 10] = [&PSHL, &PEFF, &ENEMY, &BOSS, &ESHL, &EFFCT, &BOX, &SET, &ITEM, &SOBJ];

impl Class {
    /// Adresse du gestionnaire du type `kind` (l'original indexe la table par `(kind << 2) as u16`).
    pub fn handler(&self, b: &mut Cps2Bus, kind: u8) -> u32 {
        crate::mem::prog32(b, self.table.wrapping_add(((kind as u16) << 2) as u32))
    }

    /// Appelle le gestionnaire de chaque objet actif, dans l'ordre des emplacements. L'état d'un
    /// emplacement est lu au moment où la boucle y arrive : un objet créé par un gestionnaire dans
    /// un emplacement suivant est traité dans la même image.
    ///
    /// Les registres circulent d'un gestionnaire à l'autre comme dans la boucle d'origine (A0 =
    /// objet, D7.w = compteur, D0 = type × 4, A1 = gestionnaire) : le jeu en dépend (un gestionnaire
    /// lit le D0 laissé par le précédent) et un gestionnaire peut modifier A0 et D7.
    ///
    /// `Flow::Yield` : un gestionnaire a cédé la main (la tâche reprendra à [`Etape::Aiguillage`], puis à la suite que
    /// l'appelant a rangée dans ses appels en cours avant l'appel), ou est revenu plus haut.
    pub fn update(&self, c: &mut Cpu, b: &mut Cps2Bus) -> Flow {
        c.a[0] = self.pool.slot(0).0;
        set_low_word(&mut c.d[7], (self.pool.slots - 1) as u16);
        self.sweep(c, b, false)
    }

    /// Parcours appelé sans suite (phase 0, écrans hors partie) : aucun gestionnaire n'y cède la main. Sonde
    /// `DDSOM_POISON=rendu` : les registres hérités de l'appelant (tous sauf A5 et A7) sont inversés avant le parcours,
    /// pour prouver que les gestionnaires ne relisent rien de l'écran qui les appelle.
    #[track_caller]
    pub fn update_sans_suite(&self, c: &mut Cpu, b: &mut Cps2Bus) {
        crate::diag::registres_morts(c, "rendu", crate::diag::REGISTRES_DE_TACHE);
        assert!(self.update(c, b) == Flow::Done, "dispatch {:x} : un gestionnaire a cédé la main sans suite", self.entry);
    }

    /// Parcours des emplacements à partir de A0 / D7. `after_current` : on reprend juste après le
    /// `jsr (A1)` du gestionnaire de l'emplacement courant, qu'il ne faut pas rappeler.
    fn sweep(&self, c: &mut Cpu, b: &mut Cps2Bus, mut after_current: bool) -> Flow {
        let index = CLASSES.iter().position(|k| k.entry == self.entry).expect("classe d'objets") as u8;
        loop {
            let obj = Obj(c.a[0]);
            if !after_current && obj.is_active(b) {
                let kind = obj.u8(b, Obj::KIND);
                c.d[0] = ((kind as u16) << 2) as u32;
                // drapeaux du `lsl.w #2,D0` qui précède le saut : N et Z du résultat, V, C et X nuls
                c.flags_logic(c.d[0], Size::Word);
                c.sr &= !SR_X;
                let address = self.handler(b, kind);
                c.a[1] = address;
                let appel = crate::taches::table::appeler_attendu(c, b, Etape::Aiguillage(index));
                if !crate::appeler_attendu(c, b, address, appel) {
                    return Flow::Yield;
                }
            }
            after_current = false;
            c.a[0] = c.a[0].wrapping_add(self.pool.stride);
            let counter = (c.d[7] as u16).wrapping_sub(1);
            set_low_word(&mut c.d[7], counter);
            if counter == 0xffff {
                return Flow::Done;
            }
        }
    }
}

/// Reprise d'une boucle de dispatch dont un gestionnaire a fait céder la main à la tâche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Etape {
    /// Retour du gestionnaire de l'emplacement courant de la classe `CLASSES[k]` (A0 et D7 rendus par la tâche) : le
    /// parcours continue à l'emplacement suivant, puis revient à l'appelant de la boucle.
    Aiguillage(u8),
}

impl Etape {
    /// Adresse que l'original reprend : celle qui suit le `jsr (A1)` de la boucle.
    pub const fn adresse(self) -> u32 {
        let Etape::Aiguillage(k) = self;
        CLASSES[k as usize].entry + HANDLER_RETURN
    }

    pub fn pas(self, x: &mut crate::scene::Ctx) -> Flow {
        let Etape::Aiguillage(k) = self;
        CLASSES[k as usize].sweep(x.c, x.b, true)
    }
}

/// Décalage de l'adresse de retour du `jsr (A1)` qui appelle le gestionnaire, dans chacune des dix
/// boucles d'origine (`entry + 0x1a`).
const HANDLER_RETURN: u32 = 0x1c;

fn set_low_word(r: &mut u32, v: u16) {
    *r = *r & 0xffff_0000 | v as u32;
}

macro_rules! natives {
    ($($nom:ident, $name:literal, $class:expr;)*) => {$(
        pub const $nom: Native = Native {
            entry: $class.entry,
            name: $name,
            run: crate::Corps::Processeur(|c, b| {
                // appelée par adresse (écrans hors partie) : un gestionnaire n'y cède jamais la main
                if $class.update(c, b) == Flow::Done {
                    rts(c, b);
                }
            }),
            touches_sound: false,
            outputs: 0, // les appelants enchaînent sur d'autres appels
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

/// Les dix boucles de dispatch, pour les appels par adresse qui subsistent (écrans hors partie).
pub const NATIVES: &[Native] = &[
    UPDATE_PSHL, UPDATE_PEFF, UPDATE_ENEMY, UPDATE_BOSS, UPDATE_ESHL, UPDATE_EFFCT, UPDATE_BOX, UPDATE_SET,
    UPDATE_ITEM, UPDATE_SOBJ,
];
