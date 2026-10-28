//! Familles d'objets ITEM (ramassables), BOX (coffres) et EFFCT (effets) : un gestionnaire par type.
//!
//! La boucle de dispatch d'une classe (`0x39f34` pour ITEM, `0x39e50` BOX, `0x39cba` EFFCT) appelle,
//! pour chaque objet actif, le gestionnaire du type de l'objet (`jsr (table + 4 × type)`) avec
//! `A0` = objet et `D7` = compteur de boucle. Chaque gestionnaire est une petite machine à états
//! (octets `+4`, `+5`, `+6` de l'objet) ; il rend la main par `rts` en laissant `A0` et `D7` intacts.
//!
//! Une routine native remplace un gestionnaire entier : la logique est écrite sur un [`Ctx`]. Les
//! sous-routines non réécrites (sons, génération d'objets...) sont appelées par
//! [`Ctx::jsr`] ; les états d'un type qu'aucun scénario n'exerce restent en code d'origine
//! ([`Ctx::deleguer`]).
//!
//! Voir `docs/familles.md` pour la description de chaque type.

pub mod boss_common;
pub mod boss_0;
pub mod boss_16;
pub mod boss_22;
pub mod boss_17;
pub mod boss_6;
pub mod boss_10;
pub mod boss_5;
pub mod boss_21;
pub mod boss_19;
pub mod boss_15;
pub mod boss_18;
pub mod boss_4;
pub mod boss_7;
pub mod boss_8;
pub mod boss_11;
pub mod boss_dragon;
pub mod boss_1;
pub mod boss_3;
pub mod boss_9;
pub mod boxes;
pub mod creators;
pub mod petites;
pub mod cx;
pub mod effects;
pub mod effects_b;
pub mod effects_c;
pub mod effects_d;
pub mod effects_e;
pub mod effects_creators;
pub mod effects_f;
pub mod effects_g;
pub mod items_b;
pub mod eshl_creators;
pub mod enemy;
pub mod enemy_beetle;
pub mod enemy_common;
pub mod enemy_dbeast;
pub mod enemy_double;
pub mod enemy_gargoyle;
pub mod enemy_kobold;
pub mod enemy_goblin;
pub mod enemy_troll;
pub mod enemy_gnoll;
pub mod enemy_bow;
pub mod enemy_golem;
pub mod enemy_warmachine;
pub mod enemy_lib;
pub mod enemy_ogre;
pub mod enemy_elf;
pub mod enemy_ghoul;
pub mod enemy_harpy;
pub mod enemy_hellhound;
pub mod enemy_owlbear;
pub mod enemy_part;
pub mod reactions;
pub mod enemy_warrior;
pub mod enemy_riyakaa;
pub mod enemy_scorpion;
pub mod enemy_trog;
pub mod enemy_skeleton;
pub mod eshl;
pub mod eshl_b;
pub mod eshl_c;
pub mod eshl_d;
pub mod items;
pub mod joueur;
pub mod set;
pub mod set_b;
pub mod set_residu;
pub mod set_c;
pub mod shell;
pub mod sobj;

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::mem::{rd8, rts, A5};
use crate::objects::Obj;
use crate::Native;

/// Toutes les routines natives des familles.
pub const NATIVES: &[Native] = &[
    items::ITEM_0,
    items::ITEM_1,
    items::ITEM_3,
    items::ITEM_4,
    items::ITEM_5,
    items::ITEM_6,
    items::ITEM_7,
    items::ITEM_8,
    items::ITEM_9,
    items::EFFACE_RAMASSES,
    items::CHUTE_EQUIPEMENT,
    boxes::BOX_1,
    boxes::BOX_2,
    effects::EFFCT_1,
    effects::EFFCT_4,
    effects::EFFCT_5,
    effects::EFFCT_6,
    effects::EFFCT_7,
    effects::EFFCT_8,
    effects::EFFCT_9,
    effects::EFFCT_15,
    effects::EFFCT_17,
    effects::EFFCT_19,
    effects::EFFCT_21,
    effects::EFFCT_24,
    effects::EFFCT_25,
    effects::EFFCT_26,
    effects::EFFCT_27,
    effects::EFFCT_28,
    effects::EFFCT_29,
    effects::EFFCT_32,
    effects::EFFCT_33,
    effects::EFFCT_45,
    effects::EFFCT_51,
    effects::EFFCT_52,
    effects::EFFCT_56,
    effects::EFFCT_57,
    effects::EFFCT_62,
    effects::BOITE_TOUCHEE,
    effects::JOUEUR_TOUCHE,
    effects::FILS_FINIS,
];

/// Routines du jeu appelées par les gestionnaires (adresses d'origine).
pub mod rom {
    pub const DANS_LA_ZONE: u32 = 0xfbae;
    pub const OMBRE: u32 = 0x33fe4;
}

/// Contexte d'exécution d'un gestionnaire : la machine.
pub struct Ctx<'a> {
    pub c: &'a mut Cpu,
    pub b: &'a mut Cps2Bus,
}

impl<'a> Ctx<'a> {
    /// Exécution réelle.
    pub fn reel(c: &'a mut Cpu, b: &'a mut Cps2Bus) -> Self {
        Ctx { c, b }
    }

    /// L'objet traité (registre A0).
    pub fn obj(&self) -> Obj {
        Obj(self.c.a[0])
    }

    /// Compteur de la boucle de dispatch (D7), qui sert de phase aux clignotements.
    pub fn loop_counter(&self) -> u16 {
        self.c.d[7] as u16
    }

    /// Saut par table `jmp (table,PC,D1.w)` : lit le mot de la table à l'indice `index` (octets) et
    /// renvoie l'adresse de la cible.
    pub fn saut_table(&mut self, table: u32, index: u32) -> u32 {
        let mot = self.prog16(table.wrapping_add(index)) as i16;
        let cible = table.wrapping_add(mot as i32 as u32);
        // les registres que l'original laisse à la cible (D0 = indice, D1.w = mot) servent aux états
        // qui s'exécutent en code d'origine (`deleguer`)
        self.c.d[0] = index;
        self.c.d[1] = self.c.d[1] & 0xffff_0000 | mot as u16 as u32;
        cible
    }

    /// Variante de `saut_table` pour les sauts qui passent par D1 seul (`moveq 0,D1 ... move.w (t,PC,D1.w),D1.w`).
    pub fn saut_table_d1(&mut self, table: u32, index: u32) -> u32 {
        let mot = self.prog16(table.wrapping_add(index)) as i16;
        let cible = table.wrapping_add(mot as i32 as u32);
        self.c.d[1] = mot as u16 as u32;
        cible
    }

    /// Saut de l'original dans l'épilogue d'une autre routine (`movem.l (SP)+,{A1-A4} ; rts`) : les quatre registres
    /// sont rechargés depuis la pile, puis le retour se fait à l'adresse suivante. Ce sont les retours des appelants :
    /// un effet de bulle (`0xaa9fe`) revient ainsi directement au début du tour de la boucle de partie (`0x1c468`),
    /// par-dessus l'aiguillage des EFFCT, la mise à jour du monde, la fin de la phase et la fin d'image. Ici, ces
    /// retours sont les quatre appels en cours les plus récents de la tâche (A1-A4 reçoivent leurs adresses d'origine) ;
    /// le `rts` du gestionnaire revient au cinquième, et les fonctions Rust dépassées (boucle de dispatch...)
    /// abandonnent (`taches::table::revenu`).
    /// Glitch d'origine : le reste de l'image (objets suivants, sprites, collisions, fin d'image) est sauté.
    pub fn epilogue_etranger(&mut self) {
        let retours = crate::taches::table::depiler_appels(self.b, 4);
        for (r, suite) in [1usize, 2, 3, 4].into_iter().zip(retours) {
            self.c.a[r] = suite.adresse();
        }
    }

    /// Exécute le code d'origine à partir de `adresse` (jusqu'à son `rts`) : états non réécrits.
    pub fn deleguer(&mut self, adresse: u32) {
        // un état qui n'est qu'un `jmp abs.l` (la libération, le plus souvent) : appel direct de sa cible
        const JMP_ABS_L: u16 = 0x4ef9;
        let adresse = if self.prog16(adresse) == JMP_ABS_L { self.prog32(adresse + 2) } else { adresse };
        crate::call(self.c, self.b, adresse);
    }

    // ---- appels ----

    /// `jsr adresse` d'une routine non réécrite (ou déjà native), registres tels que préparés.
    pub fn jsr(&mut self, adresse: u32) {
        crate::call(self.c, self.b, adresse);
    }

    // Le moteur d'objets (animation, listes, pools...) : trait `objects::Courant` sur l'objet A0.

    /// Condition « plus grand » (bgt) des drapeaux courants.
    pub fn gt(&self) -> bool {
        self.c.cond(14)
    }

    // ---- lectures de la ROM programme et de la RAM de travail ----

    /// Octet de la ROM programme lu comme une donnée (tables adressées par `lea (x,PC),An`).
    pub fn rom8(&mut self, a: u32) -> u8 {
        rd8(self.b, a)
    }

    pub fn rom16(&mut self, a: u32) -> u16 {
        crate::mem::rd16(self.b, a)
    }

    pub fn rom32(&mut self, a: u32) -> u32 {
        crate::mem::rd32(self.b, a)
    }

    pub fn prog8(&mut self, a: u32) -> u8 {
        let mot = m68k::Bus::read_prog16(self.b, a & 0xff_fffe);
        if a & 1 == 0 { (mot >> 8) as u8 } else { mot as u8 }
    }

    pub fn prog16(&mut self, a: u32) -> u16 {
        (self.prog8(a) as u16) << 8 | self.prog8(a.wrapping_add(1)) as u16
    }

    pub fn prog32(&mut self, a: u32) -> u32 {
        (self.prog16(a) as u32) << 16 | self.prog16(a.wrapping_add(2)) as u32
    }

    /// Octet de la RAM de travail à `A5 + decalage` (décalage signé étendu).
    pub fn ram8(&mut self, decalage: i32) -> u8 {
        rd8(self.b, A5.wrapping_add(decalage as u32))
    }
    pub fn ram16(&mut self, decalage: i32) -> u16 {
        crate::mem::rd16(self.b, A5.wrapping_add(decalage as u32))
    }
    pub fn set_ram8(&mut self, decalage: i32, v: u8) {
        crate::mem::wr8(self.b, A5.wrapping_add(decalage as u32), v)
    }

    /// Caméra de l'image : (x, y) écran, avec la caméra alternative si `A5+0x36` est non nul.
    pub fn camera(&mut self) -> (u16, u16) {
        let alt = self.ram8(0x36) != 0;
        if alt {
            (self.ram16(0x556), self.ram16(0x55a))
        } else {
            (self.ram16(0x506), self.ram16(0x50a))
        }
    }
}

/// `addq.b #n,(off,A0)` : ajoute `n` à un octet de l'objet (modulo 256).
pub(crate) fn ajoute_u8(ctx: &mut Ctx, o: Obj, off: u32, n: u8) {
    let v = o.u8(ctx.b, off).wrapping_add(n);
    o.set_u8(ctx.b, off, v);
}

/// Objet désigné par un pointeur RAM de 16 bits étendu en signe (`movea.w (off,A0),An`).
pub(crate) fn pointeur(ctx: &mut Ctx, o: Obj, off: u32) -> Obj {
    Obj(o.u16(ctx.b, off) as i16 as i32 as u32 & 0xff_ffff)
}

/// Exécute un gestionnaire (RTS compris).
pub fn executer(c: &mut Cpu, b: &mut Cps2Bus, logique: fn(&mut Ctx)) {
    logique(&mut Ctx::reel(c, b));
    rts(c, b);
}

/// Déclare la routine native d'un gestionnaire : adresse d'entrée, nom, logique.
macro_rules! gestionnaire {
    ($const:ident, $entree:expr, $nom:expr, $logique:path) => {
        pub const $const: crate::Native = crate::Native {
            entry: $entree,
            name: $nom,
            run: |c, b| crate::families::executer(c, b, $logique),
            touches_sound: false,
            outputs: crate::regs::a(0) | crate::regs::d(7),
        };
    };
}
pub(crate) use gestionnaire;

/// Routines natives des familles ESHL, SET et SOBJ (voir `docs/familles-eshl-set-sobj.md`) : un
/// gestionnaire par type, écrit sur [`cx::Cx`].
pub fn natives() -> impl Iterator<Item = &'static Native> {
    [eshl::NATIVES, eshl_b::NATIVES, eshl_c::NATIVES, eshl_d::NATIVES, set::NATIVES, set_b::NATIVES, set_c::NATIVES, sobj::NATIVES, effects_b::NATIVES, effects_c::NATIVES, effects_d::NATIVES, effects_e::NATIVES, effects_f::NATIVES, enemy::NATIVES, enemy_double::NATIVES, enemy_part::NATIVES, enemy_common::NATIVES, enemy_riyakaa::NATIVES, enemy_dbeast::NATIVES, enemy_owlbear::NATIVES, enemy_harpy::NATIVES, enemy_warrior::NATIVES, enemy_skeleton::NATIVES, enemy_elf::NATIVES, enemy_trog::NATIVES, enemy_ghoul::NATIVES, enemy_beetle::NATIVES, enemy_scorpion::NATIVES, enemy_gargoyle::NATIVES, enemy_hellhound::NATIVES, enemy_ogre::NATIVES, enemy_kobold::NATIVES, enemy_goblin::NATIVES, enemy_troll::NATIVES, enemy_gnoll::NATIVES, enemy_bow::NATIVES, enemy_golem::NATIVES, enemy_warmachine::NATIVES, boss_dragon::NATIVES, boss_17::NATIVES, boss_6::NATIVES, boss_10::NATIVES, boss_5::NATIVES, boss_21::NATIVES, boss_19::NATIVES, boss_15::NATIVES, boss_15::HELPERS, boss_18::NATIVES, boss_4::NATIVES, boss_7::NATIVES, boss_8::NATIVES, boss_11::NATIVES, boss_1::NATIVES, boss_3::NATIVES, boss_9::NATIVES]
        .into_iter()
        .flatten()
        .chain(items_b::NATIVES)
        .chain(eshl_creators::NATIVES)
        .chain(boxes::caisse::NATIVES)
        .chain(boxes::pousse::NATIVES)
        .chain(boss_common::NATIVES)
        .chain(boss_0::NATIVES)
        .chain(boss_16::NATIVES)
        .chain(boss_22::NATIVES)
        .chain(creators::NATIVES)
        .chain([&set_b::SPAWN_BUMPERS])
        .chain(set_residu::NATIVES)
        .chain(petites::NATIVES)
        .chain(petites::SHAKES)
        .chain(effects_creators::NATIVES)
        .chain(effects_g::NATIVES)
        .chain(joueur::natives())
        .chain(reactions::NATIVES)
        .chain([&reactions::PLAYER_HURT_ANIMATION])
}
