//! Contexte commun des routines natives : la machine (processeur et bus) et les services partagés.
//!
//! Chaque sous-système a son contexte (`families::cx::Cx`, `families::Ctx`, `player::Ctx`, `scene::Ctx`,
//! `combat::World`...) ; tous tiennent `c: &mut Cpu` et `b: &mut Cps2Bus`. Plutôt qu'une structure de base qu'ils
//! embarqueraient (les accès `x.c`, `x.b` séparés, partout, ne survivraient pas à un `Deref`), ils implémentent le
//! trait [`Machine`], qui leur donne les services communs une seule fois : le tirage du hasard, les lectures dans
//! l'espace programme et l'espace données. Les variables de la RAM de travail se lisent par leurs vues typées
//! (`ram::SCENE.get(x.bus())`, `Obj`, `Player`). Le moteur d'objets (animation, mouvement, listes, écran) est dans
//! [`crate::objects::Courant`], pour les contextes qui traitent un objet.
//!
//! Le processeur ne sert qu'aux adaptateurs `_68k` (registres de l'original, `docs/guide-refactorisation.md`) ; une
//! routine convertie n'utilise que `bus()`.

use cps2::Cps2Bus;
use m68k::decode::Fetch;
use m68k::Cpu;

use crate::mem::{rd16, rd32, rd8};
use crate::rng;

/// Accès commun à la machine (voir l'en-tête du module).
pub trait Machine {
    /// Le bus : la mémoire du jeu.
    fn bus(&mut self) -> &mut Cps2Bus;

    /// Tirage du générateur pseudo-aléatoire du jeu (0x19a88, `rng::random`) ; les appelants masquent le résultat.
    fn random(&mut self) -> u16 {
        rng::random(self.bus())
    }

    /// Octet, mot et long de l'espace programme (tables lues relativement au PC dans l'original : le programme
    /// déchiffré).
    fn prog8(&mut self, a: u32) -> u8 {
        crate::mem::prog8(self.bus(), a)
    }
    fn prog16(&mut self, a: u32) -> u16 {
        self.bus().fetch16(a)
    }
    fn prog32(&mut self, a: u32) -> u32 {
        crate::mem::prog32(self.bus(), a)
    }

    /// Octet, mot et long de l'espace données (tables de la ROM atteintes par une adresse absolue ou un pointeur,
    /// RAM).
    fn data8(&mut self, a: u32) -> u8 {
        rd8(self.bus(), a)
    }
    fn data16(&mut self, a: u32) -> u16 {
        rd16(self.bus(), a)
    }
    fn data32(&mut self, a: u32) -> u32 {
        rd32(self.bus(), a)
    }
}

/// Contexte qui tient aussi le processeur : les adaptateurs `_68k` (registres de l'original) passent par lui.
pub trait Processeur: Machine {
    /// Le processeur et le bus, empruntés ensemble (les adaptateurs `_68k` ont besoin des deux).
    fn machine(&mut self) -> (&mut Cpu, &mut Cps2Bus);
}

/// Implémente `Machine` et `Processeur` pour un contexte qui a les champs `c` et `b`.
macro_rules! machine {
    ($($t:ty),* $(,)?) => {
        $(impl Machine for $t {
            fn bus(&mut self) -> &mut Cps2Bus {
                &mut *self.b
            }
        }
        impl Processeur for $t {
            fn machine(&mut self) -> (&mut Cpu, &mut Cps2Bus) {
                (&mut *self.c, &mut *self.b)
            }
        })*
    };
}

machine!(
    crate::families::Ctx<'_>,
    crate::player::Ctx<'_>,
    crate::scene::Ctx<'_>,
    crate::combat::World<'_>,
);
