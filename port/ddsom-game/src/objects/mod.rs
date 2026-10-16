//! Modèle d'objets du jeu : pools, listes triées par profondeur, animation, vitesses, culling.
//!
//! Tout ce qui bouge à l'écran (tirs, effets, ennemis, boss, décors animés, objets ramassables...)
//! est un *objet* : un enregistrement de 0x50 à 0xC0 octets dans l'un des 10 pools de la RAM
//! (`pools`). Les joueurs ont leur propre enregistrement (0x200 octets) mais partagent les champs
//! communs ci-dessous. Voir `docs/objets.md` pour le détail des formats.
//!
//! - `Obj` : vue typée sur un objet en RAM (adresse + constantes de champs).
//! - `pools` : allocation, libération et initialisation des pools.
//! - `lists` : deux listes doublement chaînées triées par profondeur (ordre de dessin).
//! - `anim` : scripts d'animation (durée, drapeaux, cadre de sprite).
//! - `motion` : vitesses, accélérations, tables de vitesse par index.
//! - `rank` : compteur de rang (difficulté dynamique).
//! - `cull` : test de présence à l'écran et inscription dans les files de collision.
//! - `cost` : durée des routines à boucles (suivi du code d'origine).

pub mod anim;
pub mod cost;
pub mod cull;
pub mod lists;
pub mod motion;
pub mod pools;
pub mod rank;

use cps2::Cps2Bus;

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8};
use crate::Native;

/// Toutes les routines natives du modèle d'objets.
pub const NATIVES: &[Native] = &[
    pools::INIT_ALL,
    pools::INIT_SET,
    pools::INIT_EFFCT,
    pools::INIT_SOBJ,
    pools::ALLOC_PSHL,
    pools::ALLOC_PEFF,
    pools::ALLOC_ENEMY,
    pools::ALLOC_ESHL,
    pools::ALLOC_EFFCT,
    pools::ALLOC_SET,
    pools::ALLOC_BOX,
    pools::ALLOC_BOSS,
    pools::ALLOC_ITEM,
    pools::ALLOC_SOBJ,
    pools::FREE_PSHL,
    pools::FREE_PEFF,
    pools::FREE_ENEMY,
    pools::FREE_ESHL,
    pools::FREE_SET,
    pools::FREE_EFFCT,
    pools::FREE_BOX,
    pools::FREE_BOSS,
    pools::FREE_ITEM,
    pools::FREE_SOBJ,
    lists::REINSERT_ACTORS,
    lists::REINSERT_FOREGROUND,
    lists::UNLINK,
    lists::UNLINK_ALL,
    anim::ANIM_START_INDEXED,
    anim::ANIM_START,
    anim::ANIM_STEP,
    anim::ANIM_START_LONG_DOUBLE,
    anim::ANIM_START_LONG_BY_CHAR,
    anim::ANIM_START_LONG_BY_VARIANT,
    anim::ANIM_STEP_LONG,
    motion::MOVE_XY,
    motion::MOVE_X,
    motion::MOVE_XYZ,
    motion::MOVE_XZ,
    motion::MOVE_Z,
    motion::MOVE_YZ,
    motion::MOVE_Y,
    motion::SPEED_TABLE_A,
    motion::SPEED_TABLE_B,
    motion::MOVE_BY_TABLE_XY,
    motion::MOVE_BY_TABLE_XZ,
    motion::PAUSE_HOOK,
    motion::COUNTDOWNS,
    cull::CULL_OFFSCREEN,
    cull::IN_ARENA,
    cull::IN_ARENA_DEFAULT,
    cull::ON_SCREEN,
    rank::RANK_RESET,
    rank::RANK_LOAD_PRESET,
    rank::RANK_RAISE,
    rank::RANK_LOWER,
    rank::RANK_TICK,
];

/// Variables de la RAM de travail (offsets depuis A5 = 0xFF8000) utilisées par le modèle d'objets.
pub mod ram {
    /// Liste des personnages et décors : tête, queue (liens `Obj::PREV`/`Obj::NEXT`).
    pub const ACTORS_HEAD: u32 = 0x5be;
    pub const ACTORS_TAIL: u32 = 0x5c0;
    /// Liste du premier plan : tête, queue.
    pub const FOREGROUND_HEAD: u32 = 0x5c2;
    pub const FOREGROUND_TAIL: u32 = 0x5c4;
    /// Caméra principale (x en +8, y en +0xc) et caméra alternative, choisie si `USE_ALT_CAMERA != 0`.
    pub const CAMERA: u32 = 0x4fe;
    pub const ALT_CAMERA: u32 = 0x54e;
    pub const USE_ALT_CAMERA: u32 = 0x36;
}

/// Lien d'une liste d'objets : adresse RAM sur 16 bits (bit 15 à 1 : 0xFF8000-0xFFFFFF), ou une
/// valeur positive qui marque la fin de liste (1 : liste des acteurs, 2 : liste du premier plan).
/// 0 : l'objet n'est dans aucune liste.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Link(pub u16);

impl Link {
    pub const NONE: Link = Link(0);

    pub fn to(o: Obj) -> Link {
        Link(o.0 as u16)
    }

    /// L'objet désigné, sauf pour 0 et les marques de fin.
    pub fn obj(self) -> Option<Obj> {
        (self.0 & 0x8000 != 0).then(|| Obj(0xff_0000 | self.0 as u32))
    }

    /// Objet désigné, l'adresse étant étendue en signe comme le fait le 68000 (`movea.w`).
    pub fn target(self) -> Obj {
        Obj(self.0 as i16 as i32 as u32 & 0xff_ffff)
    }

    pub fn is_none(self) -> bool {
        self.0 == 0
    }
}

/// Un objet du jeu, désigné par son adresse en RAM (le registre A0 de l'original).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Obj(pub u32);

impl Obj {
    /// Octet non nul : l'emplacement du pool est occupé.
    pub const ACTIVE: u32 = 0x00;
    /// Bit 0 : visible à cette image ; bit 1 : caché (non dessiné, hors collisions).
    pub const FLAGS: u32 = 0x01;
    /// Type : index du gestionnaire dans la table de sa classe.
    pub const KIND: u32 = 0x02;
    /// Variante (sens propre à chaque type ; personnage pour les tables d'animation).
    pub const VARIANT: u32 = 0x03;
    /// Octets d'état de l'automate du gestionnaire : état principal, sous-états.
    pub const STATE: u32 = 0x04;
    /// Abscisse (16.16, pixels).
    pub const X: u32 = 0x08;
    /// Altitude (16.16) : s'ajoute à `Y` pour la hauteur à l'écran.
    pub const Z: u32 = 0x0c;
    /// Profondeur sur le sol (16.16) : clé de tri des listes d'affichage.
    pub const Y: u32 = 0x10;
    /// Octet non nul : objet caché ou suspendu (testé par `motion::pause_hook`).
    pub const HIDDEN: u32 = 0x40;
    /// Vitesse et accélération en X, Z, Y (8.8 pixels/image).
    pub const VX: u32 = 0x14;
    pub const AX: u32 = 0x16;
    pub const VZ: u32 = 0x18;
    pub const AZ: u32 = 0x1a;
    pub const VY: u32 = 0x1c;
    pub const AY: u32 = 0x1e;
    /// Script d'animation : adresse de l'entrée suivante (long).
    pub const ANIM_NEXT: u32 = 0x20;
    /// Liens de la liste triée : précédent (vers la tête), suivant (vers la queue). Mots.
    pub const PREV: u32 = 0x24;
    pub const NEXT: u32 = 0x26;
    /// Code de classe (= pool d'origine : voir `pools::Pool::class`).
    pub const CLASS: u32 = 0x28;
    /// Index dans la table de vitesses (`SPEED_TABLE`).
    pub const SPEED_INDEX: u32 = 0x2d;
    /// Entrée d'animation courante : durée restante (images), drapeaux (octet haut : 0 suite,
    /// 0xFE fin, autre saut), cadre de sprite, puis deux longs de données par cadre.
    pub const ANIM_TIMER: u32 = 0x30;
    pub const ANIM_FLAGS: u32 = 0x32;
    pub const FRAME: u32 = 0x34;
    pub const FRAME_DATA_A: u32 = 0x38;
    pub const FRAME_DATA_B: u32 = 0x3c;
    /// Deux longs de données supplémentaires pour les entrées d'animation longues.
    pub const FRAME_DATA_C: u32 = 0x40;
    pub const FRAME_DATA_D: u32 = 0x44;
    /// Octet remis à zéro par `motion::tick_counters` quand il est négatif.
    pub const COUNTER_53: u32 = 0x53;
    /// Compte à rebours (images) : à zéro, `COUNTDOWN_FLAG` est effacé.
    pub const COUNTDOWN: u32 = 0x89;
    pub const COUNTDOWN_FLAG: u32 = 0x88;
    /// Table de vitesses courante (adresse ROM d'un tableau de (vx, vy) 8.8 indexé par `SPEED_INDEX`).
    pub const SPEED_TABLE: u32 = 0x4c;

    pub fn at(self, off: u32) -> u32 {
        self.0.wrapping_add(off)
    }

    pub fn u8(self, b: &mut Cps2Bus, off: u32) -> u8 {
        rd8(b, self.at(off))
    }
    pub fn u16(self, b: &mut Cps2Bus, off: u32) -> u16 {
        rd16(b, self.at(off))
    }
    pub fn u32(self, b: &mut Cps2Bus, off: u32) -> u32 {
        rd32(b, self.at(off))
    }
    pub fn set_u8(self, b: &mut Cps2Bus, off: u32, v: u8) {
        wr8(b, self.at(off), v)
    }
    pub fn set_u16(self, b: &mut Cps2Bus, off: u32, v: u16) {
        wr16(b, self.at(off), v)
    }
    pub fn set_u32(self, b: &mut Cps2Bus, off: u32, v: u32) {
        wr32(b, self.at(off), v)
    }

    pub fn is_active(self, b: &mut Cps2Bus) -> bool {
        self.u8(b, Self::ACTIVE) != 0
    }

    /// Profondeur (clé de tri), signée.
    pub fn depth(self, b: &mut Cps2Bus) -> i32 {
        self.u32(b, Self::Y) as i32
    }

    pub fn prev(self, b: &mut Cps2Bus) -> Link {
        Link(self.u16(b, Self::PREV))
    }
    pub fn next(self, b: &mut Cps2Bus) -> Link {
        Link(self.u16(b, Self::NEXT))
    }
    pub fn set_prev(self, b: &mut Cps2Bus, l: Link) {
        self.set_u16(b, Self::PREV, l.0)
    }
    pub fn set_next(self, b: &mut Cps2Bus, l: Link) {
        self.set_u16(b, Self::NEXT, l.0)
    }
}
