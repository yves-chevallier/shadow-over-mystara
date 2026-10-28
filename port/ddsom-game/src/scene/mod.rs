//! Déroulement des scènes : entrée de scène, scripts d'apparition, transitions. Voir `docs/scenes.md`.
//!
//! Une partie est une suite de *scènes* (`A5+0x34`, 1 à 114 : niveaux, boss, boutiques, démos de
//! l'attract). La boucle de partie (`game.rs`) appelle chaque image le gestionnaire de la *phase*
//! courante (`A5+0x2c`, pas de 4) ; ce module réécrit ceux qui orchestrent les scènes :
//!
//! | Phase | Rôle | Module |
//! |---|---|---|
//! | 8 | entrée de scène : initialisation, puis séquence d'ouverture propre à la scène | [`entry`] |
//! | 12 | le jeu ; le lecteur de script d'apparition tourne à chaque image | [`script`] (+ `game.rs`) |
//! | 24 | séquences scriptées (arrivée d'un boss, défilement forcé) | [`intermission`] |
//!
//! Chaque séquence de scène est une machine à états à deux niveaux : l'étape (`A5+0x2d`) et la
//! sous-étape (`A5+0x2e`, pas de 2) ; elle vit dans la RAM de travail, jamais dans la pile, de sorte
//! que chaque phase fait un pas par image et revient (aucune cession à l'ordonnanceur).

pub mod blackout;
pub mod bonus;
pub mod characters;
pub mod entry;
pub mod exit;
pub mod intermission;
pub mod screen;
pub mod script;
pub mod shop;
pub mod tally;
pub mod tiles;
pub mod transition;

use cps2::Cps2Bus;
use m68k::decode::Fetch;
use m68k::Cpu;

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::objects::{pools, Obj};
use crate::Native;

/// Variables de la RAM de travail (offsets depuis A5) communes aux séquences de scène.
pub mod var {
    /// Phase de partie (octet, pas de 4), étape (octet), sous-étape (mot, pas de 2).
    pub const PHASE: u32 = 0x2c;
    pub const STEP: u32 = 0x2d;
    pub const SUB: u32 = 0x2e;
    /// Nombre de joueurs en jeu (vivants à l'entrée de scène).
    pub const PLAYERS: u32 = 0x35;
    /// Numéro de scène (octet).
    pub const SCENE: u32 = 0x34;
    /// Octet non nul : la scène est en cours de transition, le monde attend.
    pub const HOLD: u32 = 0x33;
    /// Octet non nul : les joueurs ne répondent plus aux commandes.
    pub const PLAYERS_FROZEN: u32 = 0x4a;
    /// Masque de boutons verrouillés (lu par la copie des entrées).
    pub const INPUT_LOCK: u32 = 0x43;
    /// Fondu de palette : masque des pages, nombre de pas restants, délai entre pas.
    pub const FADE_PAGES: u32 = 0x48e;
    pub const FADE_STEPS: u32 = 0x48f;
    pub const FADE_DELAY: u32 = 0x490;
    /// Caméra principale et caméra alternative (choisie si `USE_ALT_CAMERA` est non nul).
    pub const CAMERA: u32 = 0x4fe;
    pub const ALT_CAMERA: u32 = 0x54e;
    pub const USE_ALT_CAMERA: u32 = 0x36;
}

/// Enregistrements des joueurs : quatre de 0x200 octets.
pub const PLAYERS: u32 = A5 + 0x5de;
pub const PLAYER_STRIDE: u32 = 0x200;
/// Champs d'un joueur lus par les séquences.
pub mod player {
    pub const ACTIVE: u32 = 0x00;
    /// Sous-état de l'automate du joueur (octet) : les séquences attendent qu'il atteigne une valeur.
    pub const STATE: u32 = 0x06;
    pub const HEALTH: u32 = 0x62;
}

/// Adresse d'une variable temporaire de séquence (mot de la page A5-0x8000..A5-0x6000, désignée
/// dans l'original par son déplacement 16 bits relatif à A5, `0x9a2a` = A5 - 0x65d6).
pub const fn scratch(offset16: u16) -> u32 {
    A5 - 0x1_0000 + offset16 as u32
}

/// Tâches de fondu lancées par les séquences.
pub mod sub {
    /// Tâche de fondu de palette vers la cible (emplacement 0x10).
    pub const FADE_TASK: u32 = 0x14ba;
    /// Tâche de fondu à 1 niveau par pas (emplacement 0x10).
    pub const FADE_STEP_TASK: u32 = 0x1ced4;
}

/// Contexte d'une séquence de scène : la machine, et des accès nommés à la RAM de travail.
pub struct Ctx<'a> {
    pub c: &'a mut Cpu,
    pub b: &'a mut Cps2Bus,
}

impl<'a> Ctx<'a> {
    pub fn new(c: &'a mut Cpu, b: &'a mut Cps2Bus) -> Self {
        Ctx { c, b }
    }

    // --- variables de la RAM de travail ---------------------------------------------------------

    pub fn v8(&mut self, off: u32) -> u8 {
        rd8(self.b, A5 + off)
    }
    pub fn v16(&mut self, off: u32) -> u16 {
        rd16(self.b, A5 + off)
    }
    pub fn set_v8(&mut self, off: u32, v: u8) {
        wr8(self.b, A5 + off, v)
    }
    pub fn set_v16(&mut self, off: u32, v: u16) {
        wr16(self.b, A5 + off, v)
    }
    pub fn set_v32(&mut self, off: u32, v: u32) {
        wr32(self.b, A5 + off, v)
    }
    /// Mot ou long à une adresse absolue (variables temporaires, tables de la ROM en données).
    pub fn rd16(&mut self, addr: u32) -> u16 {
        rd16(self.b, addr)
    }
    pub fn rd32(&mut self, addr: u32) -> u32 {
        rd32(self.b, addr)
    }
    pub fn rd8(&mut self, addr: u32) -> u8 {
        rd8(self.b, addr)
    }
    pub fn wr16(&mut self, addr: u32, v: u16) {
        wr16(self.b, addr, v)
    }
    /// Mot de l'espace programme (table adressée relativement au PC dans l'original).
    pub fn prog16(&mut self, addr: u32) -> u16 {
        self.b.fetch16(addr)
    }
    /// Octet de l'espace programme.
    pub fn prog8(&mut self, addr: u32) -> u8 {
        let word = self.b.fetch16(addr & !1);
        if addr & 1 == 0 { (word >> 8) as u8 } else { word as u8 }
    }

    // --- étapes ----------------------------------------------------------------------------------

    /// Sous-étape courante (octet bas du mot `A5+0x2e`, toujours pair).
    pub fn sub(&mut self) -> u8 {
        self.v8(var::SUB)
    }
    /// Passe à la sous-étape suivante.
    pub fn next_sub(&mut self) {
        let sub = self.v8(var::SUB).wrapping_add(2);
        self.set_v8(var::SUB, sub);
    }
    pub fn set_sub(&mut self, sub: u8) {
        self.set_v8(var::SUB, sub);
    }
    /// Fin de la séquence : phase suivante, étapes remises à zéro, plus de verrou.
    pub fn end_scene(&mut self) {
        self.leave_phase();
        self.set_v8(var::HOLD, 0);
        self.set_v8(var::PLAYERS_FROZEN, 0);
    }
    /// Phase suivante (`A5+0x2c` += 4), étape et sous-étape à zéro.
    pub fn leave_phase(&mut self) {
        let phase = self.v8(var::PHASE).wrapping_add(4);
        self.set_v8(var::PHASE, phase);
        self.set_v8(var::STEP, 0);
        self.set_v16(var::SUB, 0);
    }
    /// Début de séquence : la scène et les joueurs sont verrouillés.
    pub fn lock(&mut self) {
        self.set_v8(var::HOLD, 1);
        self.set_v8(var::PLAYERS_FROZEN, 1);
    }

    /// Fin de l'image d'une séquence : mise à jour du monde (0x1c86e), puis liste de sprites (0x1de96). Aucun
    /// gestionnaire n'y cède la main (l'appel par adresse de l'original ne l'admettait pas).
    pub fn world(&mut self) {
        let flow = crate::game::update_world(self.c, self.b);
        assert!(flow == crate::modes::Flow::Done, "séquence de scène : un gestionnaire a cédé la main pendant la mise à jour du monde");
        crate::sprites::construire_68k(self.c, self.b);
    }

    /// Objet système « carton » de début de scène (SOBJ de type 14, 0xdfe26).
    pub fn title_card(&mut self) {
        let _ = crate::families::creators::sobj_14_pur(self.b);
    }

    /// Objet système de type 6, de variante `variante` (0xddeaa).
    pub fn sobj_6(&mut self, variante: u8) {
        let _ = crate::effects_aids::decor_6(self.b, variante);
    }

    /// Copie des entrées vers les joueurs (0x2aec). Glitch d'origine : D0-D4 qu'elle laisse sont relus par des
    /// gestionnaires de la mise à jour du monde (`inputs::recopier_68k`).
    pub fn inputs(&mut self) {
        crate::inputs::recopier_68k(self.c, self.b);
    }

    // --- minuteries ------------------------------------------------------------------------------

    /// Retire une image à la minuterie (mot) ; vrai quand elle atteint zéro.
    pub fn tick(&mut self, timer: u32) -> bool {
        let left = self.rd16(timer).wrapping_sub(1);
        self.wr16(timer, left);
        left == 0
    }

    // --- fondu, objets, messages ----------------------------------------------------------------

    /// Écran noir puis fondu d'ouverture : délai 2, toutes les pages, tâche de fondu.
    pub fn fade_in(&mut self) {
        self.set_v16(var::FADE_DELAY, 2);
        self.set_v8(var::FADE_PAGES, 15);
        crate::palette::no_brightness(self.b);
        self.start_fade_task(sub::FADE_TASK);
    }
    /// Variante : fondu à un niveau par pas (`A5+0x48f` = 1), sans passer par le noir.
    pub fn fade_in_stepped(&mut self) {
        self.set_v16(var::FADE_DELAY, 2);
        self.set_v8(var::FADE_PAGES, 15);
        self.set_v8(var::FADE_STEPS, 1);
        self.start_fade_task(sub::FADE_STEP_TASK);
    }
    fn start_fade_task(&mut self, entry: u32) {
        self.c.a[0] = entry;
        self.c.d[0] = 0x10;
        crate::kernel::creer_tache(self.b, 0x10, entry);
    }

    /// Crée un objet (allocateur `alloc`) actif, de type `kind` ; l'original écrit sans tester la retenue.
    pub fn spawn(&mut self, pool: &pools::Pool, kind: u8) -> Obj {
        let o = pool.alloc_unchecked(self.b);
        o.set_u8(self.b, Obj::ACTIVE, 1);
        o.set_u8(self.b, Obj::KIND, kind);
        o
    }

    /// Ouvre le message `n` (0xc8312) et mémorise (sur 16 bits) l'adresse de l'objet dans `slot`. Glitch d'origine :
    /// l'échec n'est pas testé, l'A1 laissé par l'allocation est mémorisé.
    pub fn message(&mut self, n: u8, slot: u32) {
        let o = crate::families::creators::scene_set_ou_reste(self.b, n);
        self.store_obj(slot, o);
    }
    /// Message terminé (octet `+0x22` non nul) ?
    pub fn message_done(&mut self, slot: u32) -> bool {
        let o = self.load_obj(slot);
        o.u8(self.b, 0x22) != 0
    }

    /// Mémorise un objet sur 16 bits (`movea.w`/`move.w An` de l'original).
    pub fn store_obj(&mut self, slot: u32, o: Obj) {
        self.wr16(slot, o.0 as u16);
    }
    pub fn load_obj(&mut self, slot: u32) -> Obj {
        Obj(self.rd16(slot) as i16 as i32 as u32 & 0xff_ffff)
    }

    // --- joueurs ---------------------------------------------------------------------------------

    /// Tous les joueurs en jeu et vivants sont-ils dans l'état `state` (octet `+6`) ? Laisse A1 et D6
    /// comme la boucle d'origine (A1 sur le joueur qui bloque, ou au-delà du dernier ; D6 = reste).
    pub fn players_in_state(&mut self, state: u8) -> bool {
        for k in 0..4u32 {
            let p = PLAYERS + k * PLAYER_STRIDE;
            let alive = rd8(self.b, p + player::ACTIVE) != 0 && rd16(self.b, p + player::HEALTH) != 0;
            if alive && rd8(self.b, p + player::STATE) != state {
                self.c.a[1] = p;
                self.c.d[6] = 3 - k;
                return false;
            }
        }
        self.c.a[1] = PLAYERS + 4 * PLAYER_STRIDE;
        self.c.d[6] = 0xffff;
        true
    }
}

/// Routines natives du déroulement des scènes.
pub const NATIVES: &[Native] = &[entry::PHASE_8, entry::PLACE_PLAYERS, intermission::PHASE_24, script::SPAWN_SCRIPT, script::SCRIPT_RESET];

/// Routines natives de tous les modules des scènes.
pub fn natives() -> impl Iterator<Item = &'static Native> {
    NATIVES
        .iter()
        .chain(exit::NATIVES)
        .chain(tally::NATIVES)
        .chain(bonus::NATIVES)
        .chain(characters::NATIVES)
        .chain(shop::NATIVES)
        .chain(blackout::NATIVES)
        .chain(screen::NATIVES)
        .chain(transition::natives())
}
