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
use crate::objects::Obj;
use crate::{call, Native};

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

/// Sous-routines du moteur appelées par les séquences.
pub mod sub {
    /// Palettes : tout noir (ET 0x0fff) sur les pages de `A5+0x48e`, avant un fondu d'ouverture.
    pub const PALETTES_BLACK: u32 = 0x168c;
    /// Palettes : tout blanc (OU 0xf000).
    pub const PALETTES_WHITE: u32 = 0x1686;
    /// Création d'une tâche : D0 = numéro d'emplacement × 0x10, A0 = adresse d'entrée.
    pub const TASK_CREATE: u32 = 0x9b6;
    /// Tâche de fondu de palette vers la cible (emplacement 0x10).
    pub const FADE_TASK: u32 = 0x14ba;
    /// Tâche de fondu à 1 niveau par pas (emplacement 0x10).
    pub const FADE_STEP_TASK: u32 = 0x1ced4;
    /// Mise à jour du monde et construction de la liste de sprites : fin de chaque image de scène.
    pub const WORLD_UPDATE: u32 = 0x1c86e;
    pub const SPRITES: u32 = 0x1de96;
    /// Copie des entrées vers les joueurs.
    pub const INPUTS: u32 = 0x2aec;
    /// Remise à zéro du rang (difficulté dynamique).
    pub const RANK_RESET: u32 = 0x1f258;
    /// Ouvre un message (bulle de dialogue, SET) numéro D0 ; A1 = le message, dont l'octet `+0x22`
    /// passe à 1 quand il est terminé.
    pub const MESSAGE: u32 = 0xc8312;
    /// Allocation d'un objet dans le pool de sa classe : A1 = objet, retenue = échec.
    pub const ALLOC_ENEMY: u32 = 0x22bc;
    pub const ALLOC_EFFCT: u32 = 0x2318;
    pub const ALLOC_SET: u32 = 0x2348;
    pub const ALLOC_BOSS: u32 = 0x23a8;
    pub const ALLOC_SOBJ: u32 = 0x2408;
    /// Objet système « carton » de début de scène (type 14).
    pub const TITLE_CARD: u32 = 0xdfe26;
    /// Objet système de type 6, de variante D0.
    pub const SOBJ_TYPE_6: u32 = 0xddeaa;
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

    /// Sous-routine d'origine (JSR) avec A5 en place.
    #[track_caller]
    pub fn call(&mut self, addr: u32) {
        self.c.a[5] = A5;
        call(self.c, self.b, addr);
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

    /// Fin de l'image d'une séquence : mise à jour du monde, puis liste de sprites.
    pub fn world(&mut self) {
        self.call(sub::WORLD_UPDATE);
        self.call(sub::SPRITES);
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
        self.call(sub::PALETTES_BLACK);
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
        self.call(sub::TASK_CREATE);
    }

    /// Crée un objet (allocateur `alloc`) actif, de type `kind` ; l'original écrit sans tester la retenue.
    pub fn spawn(&mut self, alloc: u32, kind: u8) -> Obj {
        self.call(alloc);
        let o = Obj(self.c.a[1]);
        o.set_u8(self.b, Obj::ACTIVE, 1);
        o.set_u8(self.b, Obj::KIND, kind);
        o
    }

    /// Ouvre le message `n` et mémorise (sur 16 bits) l'adresse de l'objet dans `slot`.
    pub fn message(&mut self, n: u32, slot: u32) {
        self.c.d[0] = n;
        self.call(sub::MESSAGE);
        self.store_obj(slot, Obj(self.c.a[1]));
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
pub const NATIVES: &[Native] = &[entry::PHASE_8, intermission::PHASE_24, script::SPAWN_SCRIPT, script::SCRIPT_RESET];

/// Routines natives de tous les modules des scènes.
pub fn natives() -> impl Iterator<Item = &'static Native> {
    NATIVES
        .iter()
        .chain(exit::NATIVES)
        .chain(exit::DISPATCH_TAILS)
        .chain(tally::NATIVES)
        .chain(bonus::NATIVES)
        .chain(characters::NATIVES)
        .chain(shop::NATIVES)
        .chain(blackout::NATIVES)
        .chain(screen::NATIVES)
        .chain(transition::natives())
}
