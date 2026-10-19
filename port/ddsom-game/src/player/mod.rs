//! Personnages joueurs : enregistrement, machine à états commune, création, réactions aux coups.
//!
//! Quatre enregistrements de 0x200 octets à `A5+0x5de` (`PLAYER_BASE`, pas `PLAYER_STRIDE`) ; un joueur
//! est aussi un objet (`objects::Obj`) : les champs communs (position, vitesses, animation, liens de
//! liste) sont ceux de `docs/objets.md`, les champs propres aux joueurs sont nommés dans `Player`.
//! Voir `docs/joueur.md` pour le détail des structures et des états.
//!
//! Organisation (les lettres P1.. sont celles de `docs/atlas.md`) :
//! - `states` : valeurs de la machine à états (`+4..+7`) ;
//! - `init` (P1) : remise à zéro et création des enregistrements ;
//! - `update` (P4) : boucle des quatre joueurs et aiguillage des états principaux ;
//! - `actions` (P4) : actions communes (immobile, marche, demi-tour, saisie) ;
//! - `checks` (P4) : transitions selon les boutons (demi-tour, attaque, saut, lancer, spécial) ;
//! - `walk` (P4) : vitesses de marche, mouvement, limites de l'écran ;
//! - `animation` : animations propres aux joueurs (entrées longues) ;
//! - `effects`, `overlays` (P4) : auras, palettes, arme et bouclier dessinés avec le joueur ;
//! - `hurt` (P4) : joueur touché, à terre ou mort ;
//! - `combos` (P6) : reconnaissance des coups spéciaux par séquences d'entrées ;
//! - `hit_reactions` (P6) : réaction d'un ennemi frappé par un joueur ;
//! - `scripted` (P3) : état scénarisé (entrée en scène, résurrection, cinématiques) ;
//! - `characters` : points d'accroche pour le code des six personnages (P5, réécrit ailleurs) ;
//! - `cov` : couverture des fonctions Rust par `examples/player_coverage`.
//!
//! Interface pour les personnages (P5) : voir `characters`. Les routines d'origine pas encore
//! réécrites sont appelées par `Ctx::rom`, qui passe par l'exécuteur et donc par les natives : déclarer
//! une `Native` à l'adresse d'une routine d'origine suffit à la remplacer partout.

/// Compte l'appel de la fonction englobante quand la couverture est active (`cov`).
macro_rules! cov {
    () => {{
        fn here() {}
        $crate::player::cov::hit(std::any::type_name_of_val(&here))
    }};
}

pub mod actions;
pub mod animation;
pub mod cov;
pub mod characters;
pub mod combos;
pub mod checks;
pub mod effects;
pub mod hit_reactions;
pub mod hurt;
pub mod init;
pub mod overlays;
pub mod scripted;
pub mod spawn;
pub mod states;
pub mod update;
pub mod walk;

use cps2::Cps2Bus;
use m68k::{Bus, Cpu, SR_Z};

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::objects::Obj;
use crate::rng::Rng;
use crate::{call, Native};

/// Routines natives des joueurs.
pub const NATIVES: &[Native] = &[
    init::CLEAR_PLAYERS,
    init::CLEAR_OVERLAYS,
    init::INIT_P1,
    init::INIT_P2,
    init::INIT_P3,
    init::INIT_P4,
    update::UPDATE_PLAYERS,
    overlays::UPDATE,
    walk::CLAMP,
    hit_reactions::ENEMY_REACTS,
    characters::fighter::JUMP,
    characters::fighter::ATTACK,
    characters::fighter::SPECIAL,
    characters::fighter::DASH,
    characters::cleric::JUMP,
    characters::cleric::ATTACK,
    characters::cleric::SPECIAL,
    characters::cleric::DASH,
    characters::elf::JUMP,
    characters::elf::ATTACK,
    characters::elf::SPECIAL,
    characters::elf::DASH,
    characters::magic_user::JUMP,
    characters::magic_user::ATTACK,
    characters::magic_user::SPECIAL,
    characters::magic_user::DASH,
    characters::dwarf::JUMP,
    characters::dwarf::ATTACK,
    characters::dwarf::SPECIAL,
    characters::dwarf::DASH,
    characters::dwarf::ROLL,
    characters::thief::JUMP,
    characters::thief::ATTACK,
    characters::thief::SPECIAL,
    characters::thief::DASH,
    characters::thief::ROLL,
    spawn::SLASH,
    spawn::BLOOD,
    spawn::CURE,
    spawn::POTION,
    spawn::HOLY_LIGHT,
    spawn::BEAM,
    spawn::BLESSING,
    spawn::BANISH,
    spawn::WEAPON_TRAIL,
    spawn::SHOWER,
    spawn::AURA,
    spawn::LIGHTNING_TARGET,
    spawn::SWARM,
    spawn::STORM,
    spawn::RING,
    spawn::BEAM_CAST,
    spawn::METEORS,
    spawn::ICE_COLUMNS,
    spawn::ICE_MIST,
    spawn::ICE_SHARDS,
    spawn::ICE_CROWN,
    spawn::FIREBALL,
    spawn::ICE_GROUND,
    spawn::THROWN_A,
    spawn::THROWN_B,
    spawn::THROWN_C,
    spawn::THROWN_D,
    spawn::THROWN_E,
    spawn::THROWN_F,
    spawn::THROWN_G,
    spawn::THROWN_H,
    spawn::LEAP,
    spawn::SWAP,
    spawn::DASH,
    spawn::WALL_TARGET,
    spawn::LIGHTNING,
    spawn::KNIFE,
    spawn::SHADOWS,
];

/// Premier enregistrement joueur (offset depuis A5) et pas entre deux joueurs.
pub const PLAYER_BASE: u32 = 0x5de;
pub const PLAYER_STRIDE: u32 = 0x200;
pub const PLAYER_COUNT: u32 = 4;

/// Variables du jeu (offsets depuis A5) lues ou écrites par le code des joueurs.
pub mod ram {
    /// Bit k : le joueur k participe à la partie.
    pub const PLAYERS_MASK: u32 = 0x31;
    /// Numéro de scène (étape).
    pub const SCENE: u32 = 0x34;
    /// Nombre de joueurs en vie.
    pub const PLAYERS_ALIVE: u32 = 0x35;
    /// Non nul : deuxième caméra (voir `objects::ram`).
    pub const USE_ALT_CAMERA: u32 = 0x36;
    /// Pause : les joueurs ne vieillissent plus.
    pub const PAUSE: u32 = 0x3d;
    /// Jeu gelé (cinématique).
    pub const FROZEN: u32 = 0x42;
    /// Masque d'entrées des joueurs.
    pub const INPUT_MASK: u32 = 0x43;
    /// Entrées gelées (scénario en cours).
    pub const INPUT_RAW: u32 = 0x44;
    /// Compteur d'images (bas = parité, clignotements).
    pub const FRAME_COUNTER: u32 = 0x6f;
    /// Mode « ralenti » : tous les joueurs marchent lentement (h).
    pub const SLOW_MODE: u32 = 0x59;
    /// Mot : compteur du hasard des tables (voir combat).
    pub const TICK: u32 = 0x7f92;
    /// Phase de partie.
    pub const PHASE: u32 = 0x2c;
}

/// Contexte d'exécution des routines natives : registres et mémoire du 68000.
pub struct Ctx<'a> {
    pub c: &'a mut Cpu,
    pub b: &'a mut Cps2Bus,
}

impl<'a> Ctx<'a> {
    pub fn new(c: &'a mut Cpu, b: &'a mut Cps2Bus) -> Self {
        Ctx { c, b }
    }

    /// Octet de la RAM de travail (offset depuis A5).
    pub fn a5u8(&mut self, off: u32) -> u8 {
        rd8(self.b, A5 + off)
    }
    pub fn a5u16(&mut self, off: u32) -> u16 {
        rd16(self.b, A5 + off)
    }
    pub fn set_a5u8(&mut self, off: u32, v: u8) {
        wr8(self.b, A5 + off, v)
    }
    pub fn set_a5u16(&mut self, off: u32, v: u16) {
        wr16(self.b, A5 + off, v)
    }

    /// Lectures dans l'espace données (tables de la ROM atteintes par un pointeur).
    pub fn data8(&mut self, a: u32) -> u8 {
        rd8(self.b, a)
    }
    pub fn data16(&mut self, a: u32) -> u16 {
        rd16(self.b, a)
    }
    pub fn data32(&mut self, a: u32) -> u32 {
        rd32(self.b, a)
    }

    /// Lecture d'un long dans l'espace programme (opérande relatif au PC de l'original).
    pub fn prog32(&mut self, a: u32) -> u32 {
        (self.b.read_prog16(a) as u32) << 16 | self.b.read_prog16(a + 2) as u32
    }
    pub fn prog16(&mut self, a: u32) -> u16 {
        self.b.read_prog16(a)
    }
    /// Octet de l'espace programme.
    pub fn prog8(&mut self, a: u32) -> u8 {
        let w = self.b.read_prog16(a & !1);
        if a & 1 == 0 {
            (w >> 8) as u8
        } else {
            w as u8
        }
    }

    /// Tirage du générateur pseudo-aléatoire (0x19a88) ; les appelants masquent le résultat.
    pub fn random(&mut self) -> u16 {
        let mut rng = Rng(rd16(self.b, A5 + 0x1a));
        let (v, _) = rng.suivant();
        wr16(self.b, A5 + 0x1a, v);
        v
    }

    /// Appelle une routine d'origine (ou native) pour le joueur `p` : A0 = joueur, A5 = base de la
    /// RAM. Renvoie vrai si le drapeau Z est levé au retour.
    pub fn rom(&mut self, addr: u32, p: Player) -> bool {
        cov::rom_call(addr);
        self.c.a[0] = p.0;
        self.c.a[5] = A5;
        call(self.c, self.b, addr);
        self.c.sr & SR_Z != 0
    }

    /// Idem avec le paramètre D6 (numéro d'animation, d'effet...).
    pub fn rom_d6(&mut self, addr: u32, p: Player, d6: u32) -> bool {
        self.c.d[6] = d6;
        self.rom(addr, p)
    }
}

/// Un joueur, désigné par l'adresse de son enregistrement (le registre A0 de l'original).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Player(pub u32);

/// Champs de l'enregistrement joueur (décalages). Les noms marqués (h) sont des hypothèses.
impl Player {
    /// Octet : emplacement occupé (0 = libre) ; 2 = en sortie.
    pub const ACTIVE: u32 = Obj::ACTIVE;
    /// Bit 1 : joueur masqué (clignotement d'invulnérabilité).
    pub const FLAGS: u32 = Obj::FLAGS;
    /// Numéro du joueur 0..3.
    pub const INDEX: u32 = 0x02;
    /// Personnage 0..11 : `CHARACTER >> 1` = classe (combattant, clerc, elfe, nain, magicien, voleur).
    pub const CHARACTER: u32 = Obj::VARIANT;
    /// État principal (`states::Main`), sous-état (`states::Sub`), action (`states::Action`), pas.
    pub const MAIN: u32 = 0x04;
    pub const SUB: u32 = 0x05;
    pub const ACTION: u32 = 0x06;
    pub const STEP: u32 = 0x07;
    /// Groupe et numéro de palette.
    pub const PALETTE_GROUP: u32 = 0x2a;
    pub const PALETTE: u32 = 0x2b;
    /// Orientation : 0 = regarde à droite, 0x20 = à gauche.
    pub const FACING: u32 = 0x2c;
    /// Jeu de boîtes de collision (classe du personnage).
    pub const BOX_SET: u32 = 0x2e;
    /// Contact avec le sol / un mur (h) : testé après un déplacement.
    pub const BLOCKED: u32 = 0x29;
    /// Gel de coup (images restantes ; négatif : à effacer).
    pub const HITSTOP: u32 = Obj::COUNTER_53;
    /// Arme / sous-objets rattachés (mots : adresses d'objets).
    pub const OVERLAY_BODY: u32 = 0x48;
    pub const OVERLAY_ATTACK: u32 = 0x4a;
    /// Compteur d'animation de la palette du compte à rebours de l'écran (h).
    pub const PAL_STEP: u32 = 0x52;
    /// Points de vie maximum, courants, précédents.
    pub const HP_MAX: u32 = 0x60;
    pub const HP: u32 = 0x62;
    pub const HP_PREV: u32 = 0x64;
    /// Paramètre de l'action en cours (numéro d'animation de repli, durée...).
    pub const PARAM: u32 = 0x69;
    pub const SUBACTION: u32 = 0x6a;
    pub const PARAM_6B: u32 = 0x6b;
    /// Entrées maintenues, de l'image précédente, nouvelles.
    pub const KEYS_HELD: u32 = 0x6c;
    pub const KEYS_PREV: u32 = 0x6d;
    pub const KEYS_NEW: u32 = 0x6e;
    pub const TIMER_6F: u32 = 0x6f;
    /// Compte à rebours (images) après lequel `RECOIL` s'efface.
    pub const RECOIL: u32 = 0x70;
    pub const RECOIL_TIMER: u32 = 0x71;
    /// Invulnérabilité : images restantes.
    pub const INVULN: u32 = 0x7e;
    pub const POWERUP: u32 = 0x79;
    pub const POWERUP_TIMER: u32 = 0x7a;
    /// Marche lente forcée.
    pub const SLOW: u32 = 0x66;
    /// Cavalier / objet lié (pointeur de 16 bits en +0x86 : l'objet rattaché).
    pub const LINK: u32 = 0x86;
    /// Marche lente (monture, boue...) (h).
    pub const SLOW_8A: u32 = 0x8a;
    pub const LEVEL: u32 = 0x10c;
    /// Arme et bouclier courants (numéros), voir tables de P1.
    pub const WEAPON_A: u32 = 0x10d;
    pub const WEAPON_B: u32 = 0x10e;
    pub const POWER_10F: u32 = 0x10f;
    /// Bourse (BCD, long) (h) : somme de points.
    pub const SCORE_BCD: u32 = 0x110;
    pub const SLOT_TABLE: u32 = 0x120;
    pub const SPECIAL_COUNT: u32 = 0x15d;
    pub const SPECIAL_ID: u32 = 0x15f;
    pub const SPEED_CLASS: u32 = 0x162;
    pub const AURA: u32 = 0x182;
    pub const AURA_B: u32 = 0x183;
    pub const AURA_C: u32 = 0x184;
    pub const AURA_TIMER: u32 = 0x180;
    pub const FREEZE_194: u32 = 0x194;
    pub const DYING_TIMER: u32 = 0x196;
}

impl Player {
    pub fn obj(self) -> Obj {
        Obj(self.0)
    }

    pub fn at(self, off: u32) -> u32 {
        self.0.wrapping_add(off)
    }

    /// Joueur numéro `k` (0..3).
    pub fn nth(k: u32) -> Player {
        Player(A5 + PLAYER_BASE + PLAYER_STRIDE * k)
    }

    pub fn all() -> impl Iterator<Item = Player> {
        (0..PLAYER_COUNT).map(Player::nth)
    }

    pub fn u8(self, g: &mut Ctx, off: u32) -> u8 {
        rd8(g.b, self.at(off))
    }
    pub fn u16(self, g: &mut Ctx, off: u32) -> u16 {
        rd16(g.b, self.at(off))
    }
    pub fn u32(self, g: &mut Ctx, off: u32) -> u32 {
        rd32(g.b, self.at(off))
    }
    pub fn set_u8(self, g: &mut Ctx, off: u32, v: u8) {
        wr8(g.b, self.at(off), v)
    }
    pub fn set_u16(self, g: &mut Ctx, off: u32, v: u16) {
        wr16(g.b, self.at(off), v)
    }
    pub fn set_u32(self, g: &mut Ctx, off: u32, v: u32) {
        wr32(g.b, self.at(off), v)
    }

    /// `+4..+7` lus ensemble (état principal, sous-état, action, pas).
    pub fn state(self, g: &mut Ctx) -> u32 {
        self.u32(g, Self::MAIN)
    }
    pub fn set_state(self, g: &mut Ctx, s: u32) {
        self.set_u32(g, Self::MAIN, s)
    }

    pub fn is_active(self, g: &mut Ctx) -> bool {
        self.u8(g, Self::ACTIVE) != 0
    }

    /// Orientation vers la gauche ?
    pub fn faces_left(self, g: &mut Ctx) -> bool {
        self.u8(g, Self::FACING) != 0
    }

    /// Vrai si la direction (bits 0..1 des entrées) demande de regarder dans l'autre sens que
    /// l'orientation courante. Bit 0 = droite (orientation 0), sinon gauche (0x20).
    pub fn wants_flip(self, g: &mut Ctx, keys: u8) -> bool {
        let want = if keys & 1 != 0 { 0x20 } else { 0 };
        want != self.u8(g, Self::FACING)
    }

    /// Bascule la moitié du bit d'orientation (retourne le personnage).
    pub fn flip(self, g: &mut Ctx) {
        let v = self.u8(g, Self::FACING) ^ 0x20;
        self.set_u8(g, Self::FACING, v);
    }

    /// Classe du personnage (0 combattant ... 5 voleur).
    pub fn class(self, g: &mut Ctx) -> u8 {
        self.u8(g, Self::CHARACTER) >> 1
    }
}
