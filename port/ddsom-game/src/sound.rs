//! Son côté 68000 : file de commandes envoyées au Z80 (format : `docs/sound-68000.md`).
//!
//! Le jeu enfile ses commandes dans une file circulaire en RAM de travail (64 entrées de 16 octets) ;
//! chaque image, le VBLANK en transmet **une** au Z80 par la RAM partagée 0x618000. Les quelque 520
//! « wrappers » `sfx_*` (0xe0be2-0xede08) ne diffèrent que par leurs paramètres : ils sont décrits
//! par la table `sound_table::WRAPPERS` et joués par une seule fonction, [`play`] (ou [`play_with`]).
//!
//! Un wrapper d'origine sauvegarde tous les registres sur une pile privée (pointeur en `PRIVATE_STACK`),
//! puis compose trois longs (D1 = commande, D2 et D3 = paramètres) et appelle l'enfileur. Tous les
//! registres sont rendus intacts ; seule la mémoire change : la file, et la copie des registres reste écrite
//! sous le pointeur de pile privée. Cette « sauvegarde fantôme » n'est relue par personne et n'est plus
//! reproduite (écart assumé, `play_wrapper`) : un son ne lit chez son appelant que l'objet (A0) et, pour
//! quelques-uns, D4 ou A1 ([`Source`]).

use std::sync::OnceLock;

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C};

use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8};
use crate::objects::Obj;
use crate::{regs, Native};

#[path = "sound_table.rs"]
mod table;

/// Décalages par rapport à A5 (mots/octets signés étendus : `A5 + n` modulo 2^32).
mod ram {
    /// Début de la file (64 entrées de 16 octets).
    pub const FILE: u32 = 0xffff_a55c;
    /// Tête de lecture (prochaine entrée à transmettre au Z80), mot.
    pub const READ_HEAD: u32 = 0xffff_a95c;
    /// Tête d'écriture (prochaine case libre), mot.
    pub const WRITE_HEAD: u32 = 0xffff_a95e;
    /// Alternance 0 / -1 des wrappers à deux variantes (voix A / voix B), mot.
    pub const ALTERNATION: u32 = 0xffff_a960;
    /// Pointeur de la pile privée des wrappers (long).
    pub const PRIVATE_STACK: u32 = 0xffff_aa1a;
    /// Octet non nul : les wrappers « gardés » n'émettent rien.
    pub const MUTE_GUARD: u32 = 0xffff_aa1e;
    /// Sons autorisés si ce mot est non nul...
    pub const SOUND_WORD: u32 = 0x7fcc;
    /// ... ou si l'un de ces octets l'est (partie en cours / mode attract avec son).
    pub const GAME_SOUND: u32 = 0x30;
    pub const ATTRACT_SOUND: u32 = 0x27;
    /// Dernière commande « gardée » émise (mémo).
    pub const MEMO_COMMAND: u32 = 0x7fc4;
    /// Compte à rebours décrémenté à chaque VBLANK (délai posé par certaines commandes).
    pub const DELAY: u32 = 0x7fc2;
    /// Octet non nul : verrou de la sortie son (copié en 0x619ffd).
    pub const SILENCE: u32 = 0x28;
    /// Position X de la caméra 1 / 2 et choix de la caméra (octet).
    pub const CAMERA_1_X: u32 = 0x5a6;
    pub const CAMERA_2_X: u32 = 0x5ae;
    pub const CAMERA_CHOICE: u32 = 0x36;
}

/// Taille de la file : 64 entrées de 16 octets.
const MASK_HEAD: u16 = 0x3f0;
const ENTRY: u16 = 0x10;
/// Table de panoramique (un octet par tranche de 8 pixels d'écran), en ROM.
const TABLE_PAN: u32 = 0xee04e;

/// RAM partagée avec le Z80 : octets aux adresses impaires.
mod z80 {
    pub const BASE: u32 = 0x618000;
    /// Octets de la commande, dans l'ordre d'envoi : (octet de l'entrée de file, adresse).
    /// L'octet 4 de l'entrée (poids fort de D2) n'est pas transmis.
    pub const FIELDS: [(usize, u32); 11] = [
        (0, BASE + 0x07),
        (1, BASE + 0x09),
        (2, BASE + 0x01),
        (3, BASE + 0x03),
        (5, BASE + 0x0d),
        (6, BASE + 0x0f),
        (7, BASE + 0x11),
        (8, BASE + 0x13),
        (9, BASE + 0x15),
        (10, BASE + 0x17),
        (11, BASE + 0x19),
    ];
    /// Poignée de main : le Z80 met 0xff quand il a lu la commande, le 68000 remet 0.
    pub const ACQUIT: u32 = BASE + 0x1f;
    /// Octet de verrou lu par le Z80 (0xff si la sortie son est verrouillée).
    pub const LOCK: u32 = BASE + 0x1ffd;
}

/// Affichage des verrous du Z80 (écran de débogage), en queue du VBLANK : actif seulement si le mot de la
/// ROM en 0xc0 (drapeau de débogage) et le mot `A5+0x7fc6` sont non nuls ; chaque octet des verrous
/// 0x618001... s'écrit en hexadécimal (0x29d78, deux chiffres) à une adresse de la couche de texte.
const DEBUG_FLAG: u32 = 0xc0;
const DEBUG_DISPLAY: u32 = 0x7fc6;
const HEX_BYTE: u32 = 0x29d78;
/// (adresse de la couche de texte si elle change, verrou lu), dans l'ordre d'affichage.
const LATCH_DISPLAY: [(Option<u32>, u32); 11] = [
    (Some(0x9135cc), 0x618001),
    (None, 0x618003),
    (Some(0x9135d0), 0x618007),
    (None, 0x618009),
    (Some(0x9136d4), 0x61800d),
    (Some(0x9136d8), 0x61800f),
    (Some(0x9136dc), 0x618011),
    (Some(0x9135e0), 0x618013),
    (None, 0x618015),
    (Some(0x9135e4), 0x618017),
    (None, 0x618019),
];

fn show_locks(c: &mut Cpu, b: &mut Cps2Bus) {
    let flag = rd16(b, DEBUG_FLAG);
    c.flags_logic(flag as u32, Size::Word);
    if flag == 0 {
        return;
    }
    let shown = rd16(b, a5(DEBUG_DISPLAY));
    c.flags_logic(shown as u32, Size::Word);
    if shown == 0 {
        return;
    }
    c.d[1] = 2;
    for (at, latch) in LATCH_DISPLAY {
        if let Some(at) = at {
            c.a[1] = at;
        }
        c.d[0] = c.d[0] & !0xff | rd8(b, latch) as u32;
        crate::call(c, b, HEX_BYTE);
    }
}

/// Adresse `A5 + off` (A5 vaut toujours `mem::A5` ; les décalages négatifs sont écrits modulo 2^32).
const fn a5(off: u32) -> u32 {
    crate::mem::A5.wrapping_add(off)
}

// ---------------------------------------------------------------------------------------------
// La file de commandes

/// Une commande : trois longs, tels que le jeu les compose (D1, D2, D3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    pub d1: u32,
    pub d2: u32,
    pub d3: u32,
}

/// Issue d'un enfilage, avec le contenu des registres que l'original laisse derrière lui.
struct Queueing {
    /// Mot bas de D0 au retour (nouvelle tête d'écriture, ou tête calculée si la file est pleine).
    d0: u16,
    /// Case écrite (A1 = fin de l'écriture), si la commande a été enfilée.
    written: Option<u32>,
}

/// La commande peut-elle être émise ? (son autorisé : 0xe0836)
fn sound_allowed(b: &mut Cps2Bus) -> bool {
    rd16(b, a5(ram::SOUND_WORD)) != 0
        || rd8(b, a5(ram::GAME_SOUND)) != 0
        || rd8(b, a5(ram::ATTRACT_SOUND)) != 0
}

/// La file est pleine quand l'écriture rattraperait la lecture (une case reste toujours libre).
fn queue_full(b: &mut Cps2Bus) -> bool {
    let read = rd16(b, a5(ram::READ_HEAD));
    let write = rd16(b, a5(ram::WRITE_HEAD));
    read.wrapping_sub(ENTRY) & MASK_HEAD == write
}

/// Enfile une commande (0xe0848, sans le test d'autorisation).
fn enqueue(b: &mut Cps2Bus, cmd: Command) -> Queueing {
    let read = rd16(b, a5(ram::READ_HEAD));
    let write = rd16(b, a5(ram::WRITE_HEAD));
    if queue_full(b) {
        return Queueing { d0: read.wrapping_sub(ENTRY) & MASK_HEAD, written: None };
    }
    let case = a5(ram::FILE).wrapping_add(write as i16 as u32);
    wr32(b, case, cmd.d1);
    wr32(b, case + 4, cmd.d2);
    wr32(b, case + 8, cmd.d3);
    let next = write.wrapping_add(ENTRY) & MASK_HEAD;
    wr16(b, a5(ram::WRITE_HEAD), next);
    Queueing { d0: next, written: Some(case + 8) }
}

/// Reproduit D0, A1 et les drapeaux laissés par 0xe0848.
fn finish_enqueue(c: &mut Cpu, e: &Queueing) {
    c.d[0] = c.d[0] & 0xffff_0000 | e.d0 as u32;
    match e.written {
        Some(end) => {
            c.a[1] = end;
            c.flags_logic(e.d0 as u32, Size::Word);
        }
        None => c.sr = c.sr & 0xff00 | SR_C, // move #1,CCR : seule la retenue est levée
    }
}

fn register_command(c: &Cpu) -> Command {
    Command { d1: c.d[1], d2: c.d[2], d3: c.d[3] }
}

/// 0xe0836 : enfile la commande si le son est autorisé ; rend `false` si rien n'est enfilé (son interdit, file
/// pleine).
pub fn emit(b: &mut Cps2Bus, cmd: Command) -> bool {
    sound_allowed(b) && enqueue(b, cmd).written.is_some()
}

/// 0xe0848 : enfile D1, D2, D3 (C levé si la file est pleine).
fn enqueue_direct(c: &mut Cpu, b: &mut Cps2Bus) {
    let e = enqueue(b, register_command(c));
    finish_enqueue(c, &e);
    rts(c, b);
}

/// 0xe0836 : enfile D1, D2, D3 si le son est autorisé.
fn enqueue_if_sound(c: &mut Cpu, b: &mut Cps2Bus) {
    if sound_allowed(b) {
        enqueue_direct(c, b);
    } else {
        c.sr = c.sr & 0xff00 | SR_C;
        rts(c, b);
    }
}

pub const ENQUEUE_IF_SOUND: Native = Native {
    entry: 0xe0836,
    name: "son_enfiler",
    run: crate::Body::Processeur(enqueue_if_sound),
    touches_sound: false,
    outputs: regs::d(0) | regs::a(1) | regs::CCR,
};

pub const ENQUEUE_DIRECT: Native = Native {
    entry: 0xe0848,
    name: "son_enfiler_direct",
    run: crate::Body::Processeur(enqueue_direct),
    touches_sound: false,
    outputs: regs::d(0) | regs::a(1) | regs::CCR,
};

// ---------------------------------------------------------------------------------------------
// Les wrappers

/// Lecture « espace programme » : les tables de commandes sont rangées dans le code et lues par
/// des accès relatifs au PC, qui voient les opcodes déchiffrés.
fn program_read(b: &mut Cps2Bus, address: u32) -> u16 {
    m68k::Bus::read_prog16(b, address & 0xff_ffff)
}

/// Origine de l'indice de commande dans une table de mots en ROM.
#[derive(Clone, Copy, Debug)]
pub enum Cmd {
    /// Commande constante.
    Fixed(u16),
    /// Deux commandes en alternance (adresse de la table) : 0xedee4 retourne 0 puis 2 à tour de rôle.
    Alternate(u32),
    /// Indice = 2 × octet 2 de l'objet + 4 × (octet 3 & 0xe) (0xedef2).
    ByObject(u32),
    /// Indice = 2 × octet 2 de l'objet.
    ByByte2(u32),
    /// La dernière commande gardée (`MEMO_COMMAND`), rejouée.
    Memo,
    /// Commande passée par l'appelant dans D4.w (musique).
    FromD4,
}

// Options d'un wrapper.
/// Panoramique calculé depuis la position X de l'objet A0, ajouté à l'octet 1 de D2.
pub const PAN: u8 = 1;
/// Ne rien émettre si `MUTE_GUARD` est non nul.
pub const GUARD: u8 = 2;
/// Recopie la commande en `MEMO_COMMAND`.
pub const MEMO: u8 = 4;
/// Enfile sans tester l'autorisation du son (0xe0848 au lieu de 0xe0836).
pub const DIRECT: u8 = 8;

/// Description d'un point d'entrée « sfx » : la commande est D1, ses paramètres D2 et D3.
#[derive(Clone, Copy, Debug)]
pub struct Wrapper {
    pub entry: u32,
    pub cmd: Cmd,
    pub d2: u32,
    pub d3: u32,
    pub opts: u8,
}

/// Ce qu'un son lit chez son appelant : les registres que l'original consulte, et rien d'autre (voir l'en-tête,
/// « sauvegarde fantôme »). Un son qui lit une valeur absente s'arrête (panique) : l'appelant doit la fournir.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Source {
    /// A0 : l'objet à l'origine du bruit (panoramique, numéro de joueur, personnage, variante).
    pub object: Option<Obj>,
    /// A1 : l'objet désigné des aiguillages `ByA1` (0xec046, 0xec200).
    pub a1: Option<u32>,
    /// D4 : commande de musique (`FromD4`) ou choix des aiguillages `ByD4`.
    pub d4: Option<u32>,
}

impl Source {
    fn object(&self) -> u32 {
        self.object.expect("ce son lit son objet (A0)").0
    }
    fn a1(&self) -> u32 {
        self.a1.expect("ce son lit A1")
    }
    fn d4(&self) -> u32 {
        self.d4.expect("ce son lit D4")
    }
}

/// Joue le son `sound` (entrée d'un wrapper `sfx_*` ou d'un aiguillage, son identifiant) pour l'objet `object` (A0 de
/// l'original). Les quelques sons qui lisent aussi D4 ou A1 passent par [`play_with`].
pub fn play(b: &mut Cps2Bus, sound: u32, object: Obj) {
    play_with(b, sound, Source { object: Some(object), ..Source::default() });
}

/// Joue un son qui ne lit rien chez son appelant (pas de panoramique, de joueur ni de variante : musiques, bruits
/// de l'interface), pour les appelants sans objet.
pub fn play_global(b: &mut Cps2Bus, sound: u32) {
    debug_assert!(!reads_object(sound), "son {sound:x} : lit son objet, utiliser jouer");
    play_with(b, sound, Source::default());
}

/// Joue le son `sound` avec tout ce qu'il peut lire (voir [`Source`]).
pub fn play_with(b: &mut Cps2Bus, sound: u32, source: Source) {
    if let Ok(i) = table::WRAPPERS.binary_search_by_key(&sound, |w| w.entry) {
        play_wrapper(b, &table::WRAPPERS[i], source);
    } else if let Some(&(_, kind)) = DISPATCHES.iter().find(|(e, _)| *e == sound) {
        dispatch(b, kind, source);
    } else if let Some(&(_, normal, alternate)) = BY_VERSION.iter().find(|(e, ..)| *e == sound) {
        let target = if version_alternative(b) { alternate } else { normal };
        play_with(b, target, source);
    } else if sound == NONE.entry {
        // objets sans son
    } else {
        panic!("son {sound:x} : ni wrapper ni aiguillage")
    }
}

/// Le son lit-il son objet (A0) ? Les cibles des aiguillages par D4 ou A1 ne sont connues qu'à l'exécution :
/// réputées le lire.
fn reads_object(sound: u32) -> bool {
    if let Ok(i) = table::WRAPPERS.binary_search_by_key(&sound, |w| w.entry) {
        let w = &table::WRAPPERS[i];
        return w.opts & PAN != 0 || matches!(w.cmd, Cmd::ByObject(_) | Cmd::ByByte2(_));
    }
    if let Some(&(_, kind)) = DISPATCHES.iter().find(|(e, _)| *e == sound) {
        return !matches!(kind, Dispatch::Music { .. });
    }
    if let Some(&(_, normal, alternate)) = BY_VERSION.iter().find(|(e, ..)| *e == sound) {
        return reads_object(normal) || reads_object(alternate);
    }
    sound != NONE.entry
}

/// Panoramique (0-0x2f -> octet de la table de ROM) d'un objet : position écran de l'objet par
/// rapport à la caméra, bornée à 0..0x178, par tranches de 8 pixels.
fn pan(b: &mut Cps2Bus, object: u32) -> u8 {
    let x = rd16(b, object + 8);
    let camera_2 = rd8(b, a5(ram::CAMERA_CHOICE)) != 0;
    let camera = rd16(b, a5(if camera_2 { ram::CAMERA_2_X } else { ram::CAMERA_1_X }));
    let gap = x.wrapping_sub(camera);
    let gap = if (gap as i16) < 0 { 0 } else { gap };
    let gap = if gap < 0x180 { gap } else { 0x178 };
    rd8(b, TABLE_PAN + ((gap & 0x1ff) >> 3) as u32)
}

/// Résultat de l'évaluation d'un wrapper (sans effet sur la mémoire).
struct Plan {
    /// Commande à enfiler, ou `None` si le wrapper est gardé (rien n'est émis).
    command: Option<Command>,
    /// Le son est-il autorisé (sinon la commande est abandonnée par l'enfileur) ?
    allowed: bool,
}

fn schedule(b: &mut Cps2Bus, w: &Wrapper, source: Source) -> Plan {
    // indice de la table de commandes, calculé par les sous-routines 0xedee4 / 0xedef2
    let index = match w.cmd {
        Cmd::Fixed(_) => 0,
        Cmd::Alternate(_) => {
            let voice_b = rd16(b, a5(ram::ALTERNATION)) != 0;
            if voice_b { 2 } else { 0 }
        }
        Cmd::ByObject(_) => {
            let byte3 = rd8(b, source.object() + 3) as u16;
            let byte2 = rd8(b, source.object() + 2) as u16;
            byte2 * 2 + ((byte3 & 0xe) << 2)
        }
        Cmd::ByByte2(_) => rd8(b, source.object() + 2) as u16 * 2,
        Cmd::Memo | Cmd::FromD4 => 0,
    };
    let d1 = match w.cmd {
        Cmd::Fixed(v) => v,
        Cmd::Alternate(t) | Cmd::ByObject(t) | Cmd::ByByte2(t) => program_read(b, t + index as i16 as u32),
        Cmd::Memo => rd16(b, a5(ram::MEMO_COMMAND)),
        Cmd::FromD4 => source.d4() as u16,
    };
    if w.opts & GUARD != 0 {
        let mute = rd8(b, a5(ram::MUTE_GUARD)) != 0;
        if mute {
            return Plan { command: None, allowed: false };
        }
    }
    let mut d2 = w.d2;
    if w.opts & PAN != 0 {
        d2 |= (pan(b, source.object()) as u32) << 8;
    }
    let direct = w.opts & DIRECT != 0;
    let allowed = direct || sound_allowed(b);
    Plan { command: Some(Command { d1: d1 as u32, d2, d3: w.d3 }), allowed }
}

/// Un wrapper : compose sa commande et l'enfile.
//
// Écart assumé : l'original commence par empiler D0-D7/A0-A4 sur la pile privée (pointeur `PRIVATE_STACK`, FF2A1A)
// et les dépile en sortie ; la copie reste en RAM sous le pointeur (« sauvegarde fantôme », FF297E-FF2A19 sur
// tous les scénarios, trois niveaux d'aiguillage au plus). Rien ne relit ces mots hors des restaurations des
// wrappers eux-mêmes (trace de lecture sur la machine d'origine, tous les scénarios : docs/son-68000.md), et
// `validate` les ignore : le jeu ne les écrit plus, et un son ne dépend plus des registres de son appelant.
fn play_wrapper(b: &mut Cps2Bus, w: &Wrapper, source: Source) {
    let plan = schedule(b, w, source);
    if let Cmd::Alternate(_) = w.cmd {
        let v = rd16(b, a5(ram::ALTERNATION));
        wr16(b, a5(ram::ALTERNATION), !v);
    }
    if let Some(cmd) = plan.command {
        if w.opts & MEMO != 0 {
            wr16(b, a5(ram::MEMO_COMMAND), cmd.d1 as u16);
        }
        if plan.allowed {
            enqueue(b, cmd);
        }
    }
}

/// Native d'un wrapper ou d'un aiguillage (appels par adresse) : le cœur, avec ce qu'il lit dans les registres ;
/// tous les registres sont rendus intacts, les drapeaux sont ceux du dernier `move.l` du pointeur de pile privée.
fn native_sound_in(c: &mut Cpu, b: &mut Cps2Bus, sound: u32) {
    play_with(b, sound, Source { object: Some(Obj(c.a[0])), a1: Some(c.a[1]), d4: Some(c.d[4]) });
    let top = rd32(b, a5(ram::PRIVATE_STACK));
    c.flags_logic(top, Size::Long);
    rts(c, b);
}

/// Les routines natives de chaque wrapper décrit et des aiguillages (construites une fois, puis conservées).
pub fn native_wrappers() -> &'static [Native] {
    static LIST: OnceLock<Vec<Native>> = OnceLock::new();
    LIST.get_or_init(|| {
        let wrappers = table::WRAPPERS.iter().map(|w| (w.entry, "son_wrapper"));
        let dispatches = DISPATCHES.iter().map(|&(entry, _)| (entry, "son_aiguillage"));
        let versions = BY_VERSION.iter().map(|&(entry, ..)| (entry, "son_selon_version"));
        wrappers
            .chain(dispatches)
            .chain(versions)
            .map(|(entry, name)| Native {
                entry,
                name,
                run: crate::Body::CpuIn(native_sound_in),
                touches_sound: false,
                outputs: 0, // tous les registres sont rendus intacts : rien à comparer
            })
            .collect()
    })
}

// ---------------------------------------------------------------------------------------------
// Les aiguillages

/// Wrappers qui ne composent pas eux-mêmes leur commande : ils choisissent dans une table, d'après un
/// registre ou l'objet, un autre wrapper qu'ils appellent (après leur propre sauvegarde sur la pile
/// privée, D0-D3 mis à zéro), ou bien une commande de musique.
#[derive(Clone, Copy, Debug)]
enum Dispatch {
    /// `tst.b D4 ; bmi` puis D4.w = 2 × D4.b, D0.w = table[D4], cible = table + D0 (rien si D4 < 0).
    ByD4(u32),
    /// D1.w = table[A0[3] & 0xe], cible = table + D1.
    ByVariant(u32),
    /// A1 = long table[2 × (A0[3] & 0xe)], cible = A1.
    ByVariantLong(u32),
    /// A1 = long table[2 × (A0[3] & 0xe)], puis D1.b = A0[0x47], A1 = long (en clair) en A1 + D1.w.
    ByVariantAndCategory(u32),
    /// D1 = 4 × A1[0x44], A1 = long table[D1], cible = A1.
    ByA1(u32),
    /// Musique de la scène : D1.w = table[2 × scène] (rien si gardé, ou si D1 <= 0 / D1 < 0 selon
    /// `strict`), recopiée en mémo puis enfilée si le son est autorisé.
    Music { table: u32, strict: bool },
}

const DISPATCHES: &[(u32, Dispatch)] = {
    use Dispatch::*;
    &[
        (0xe0c5c, Music { table: 0xe0ca6, strict: true }),
        (0xe0d8c, Music { table: 0xe0dd6, strict: false }),
        (0xe662a, ByVariantAndCategory(0xe666c)),
        (0xe6910, ByD4(0xe694a)),
        (0xe7622, ByVariant(0xe765c)),
        (0xe774c, ByVariant(0xe7786)),
        (0xe78e8, ByVariant(0xe7922)),
        (0xe7a12, ByVariant(0xe7a4c)),
        (0xe7c20, ByD4(0xe7c5a)),
        (0xe7e10, ByD4(0xe7e4a)),
        (0xe823a, ByVariantLong(0xe8274)),
        (0xec046, ByA1(0xec080)),
        (0xec200, ByA1(0xec23a)),
    ]
};

/// Long de l'espace programme (tables lues relativement au PC).
fn program_read32(b: &mut Cps2Bus, address: u32) -> u32 {
    (program_read(b, address) as u32) << 16 | program_read(b, address + 2) as u32
}

fn low_word(r: u32, v: u16) -> u32 {
    r & 0xffff_0000 | v as u32
}

/// Un aiguillage : choisit le son à jouer, qui lit la source modifiée comme dans l'original (D4 doublé, A1 désigné),
/// ou enfile la musique de la scène. L'original sauve et rend ses registres (pile privée, voir `play_wrapper`).
fn dispatch(b: &mut Cps2Bus, kind: Dispatch, source: Source) {
    let (target, source) = match kind {
        Dispatch::ByD4(table) => {
            let d4 = source.d4() as u8 as i8;
            if d4 < 0 {
                return;
            }
            let i = (d4 as i16 * 2) as u16;
            let d = program_read(b, table.wrapping_add(i as i16 as u32));
            (table.wrapping_add(d as i16 as u32), Source { d4: Some(low_word(source.d4(), i)), ..source })
        }
        Dispatch::ByVariant(table) => {
            let i = (rd8(b, source.object() + 3) & 0xe) as u32;
            let d = program_read(b, table + i);
            (table.wrapping_add(d as i16 as u32), source)
        }
        Dispatch::ByVariantLong(table) => {
            let i = 2 * (rd8(b, source.object() + 3) & 0xe) as u32;
            let a1 = program_read32(b, table + i);
            (a1, Source { a1: Some(a1), ..source })
        }
        Dispatch::ByVariantAndCategory(table) => {
            let i = 2 * (rd8(b, source.object() + 3) & 0xe) as u32;
            let list = program_read32(b, table + i);
            let category = rd8(b, source.object() + 0x47) as u32;
            let a1 = rd32(b, list.wrapping_add(category));
            (a1, Source { a1: Some(a1), ..source })
        }
        Dispatch::ByA1(table) => {
            let i = 4 * rd8(b, source.a1() + 0x44) as u32;
            let a1 = program_read32(b, table + i);
            (a1, Source { a1: Some(a1), ..source })
        }
        Dispatch::Music { table, strict } => {
            if rd8(b, a5(ram::MUTE_GUARD)) == 0 {
                let scene = rd8(b, a5(0x34)) as i8 as i16;
                let i = (scene * 2) as u16;
                let d1 = program_read(b, table.wrapping_add(i as i16 as u32));
                if !strict {
                    wr16(b, a5(ram::MEMO_COMMAND), d1);
                }
                let played = if strict { d1 as i16 > 0 } else { d1 as i16 >= 0 };
                if played {
                    if strict {
                        wr16(b, a5(ram::MEMO_COMMAND), d1);
                    }
                    if sound_allowed(b) {
                        enqueue(b, Command { d1: d1 as u32, d2: 0, d3: 0 });
                    }
                }
            }
            return;
        }
    };
    play_with(b, target, source);
}

// ---------------------------------------------------------------------------------------------
// Transmission au Z80 (VBLANK)

/// Transmet l'entrée de file `entry` aux octets de la RAM partagée.
fn transmit(b: &mut Cps2Bus, entry: u32) {
    for (byte, address) in z80::FIELDS {
        let v = rd8(b, entry + byte as u32);
        wr8(b, address, v);
    }
}

/// 0x68c (VBLANK) : une commande de la file part vers le Z80 si celui-ci a acquitté la précédente. Rend l'entrée de file
/// transmise.
pub fn send(b: &mut Cps2Bus) -> Option<u32> {
    let silence = rd8(b, a5(ram::SILENCE)) != 0;
    wr8(b, z80::LOCK, if silence { 0xff } else { 0 });
    let delay = rd16(b, a5(ram::DELAY));
    if delay != 0 {
        wr16(b, a5(ram::DELAY), delay - 1);
    }
    let read = rd16(b, a5(ram::READ_HEAD));
    let write = rd16(b, a5(ram::WRITE_HEAD));
    let mut sent = None;
    if read != write && rd8(b, z80::ACQUIT) == 0xff {
        let entry = a5(ram::FILE).wrapping_add(read as i16 as u32);
        transmit(b, entry);
        wr8(b, z80::ACQUIT, 0);
        wr16(b, a5(ram::READ_HEAD), read.wrapping_add(ENTRY) & MASK_HEAD);
        sent = Some(entry);
    }
    // l'original se termine par un JMP vers l'affichage des verrous
    if shown_locks(b) {
        // Code mort d'origine : écran de débogage, voir `show_locks` (docs/code-mort.md, lot « sortie du Cpu »)
        unreachable!("affichage des verrous du Z80 : le mot 0xc0 du programme est nul dans cette version");
    }
    sent
}

/// L'affichage de débogage des verrous est actif (mot 0xc0 du programme et `A5+0x7fc6` non nuls).
fn shown_locks(b: &mut Cps2Bus) -> bool {
    rd16(b, DEBUG_FLAG) != 0 && rd16(b, a5(DEBUG_DISPLAY)) != 0
}

/// `send` avec les registres de l'original (D0 : tête de lecture, D0 haut = -1 si silence ; A4 après l'entrée
/// transmise ; drapeaux du test de l'affichage des verrous).
fn send_to_z80(c: &mut Cpu, b: &mut Cps2Bus) {
    let silence = rd8(b, a5(ram::SILENCE)) != 0;
    if let Some(entry) = send(b) {
        c.a[4] = entry + 12;
    }
    let read = rd16(b, a5(ram::READ_HEAD));
    c.d[0] = if silence { 0xffff_0000 } else { 0 } | read as u32;
    show_locks(c, b);
    rts(c, b);
}

pub const SEND_TO_Z80: Native = Native {
    entry: 0x68c,
    name: "son_envoyer_z80",
    run: crate::Body::Processeur(send_to_z80),
    touches_sound: true,
    outputs: 0, // le VBLANK a sauvegardé tous les registres
};

// ---------------------------------------------------------------------------------------------
// Commandes de service (0xe0882, 0xe0a10, 0xe0a2e-0xe0aae)

/// Octet de la RAM partagée écrit par les commandes à délai (lu par le Z80).
const SERVICE_LOCK: u32 = z80::BASE + 0x05;
/// Par indice D0.w (en octets) : (délai `A5+0x7fc2`, mot haut de D1), mots en clair.
const DELAYS: u32 = 0xe0ab0;

/// Commande de service, enfilée sans test d'autorisation du son.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    /// 0xe0882 : commande 0xff00 (arrêt général).
    GeneralStop,
    /// 0xe0a10 : commande 0xff05.
    Ff05,
    /// 0xe0a2e (verrou = D1.b) / 0xe0a40 (verrou 0xff) : commande 0xff06 à délai. `line` (D0.w) : décalage en
    /// octets de la ligne de la table des délais.
    Ff06 { lock: u8, line: u16 },
    /// 0xe0a70 (verrou = D1.b) / 0xe0a82 (verrou 0) : commande 0xff07 à délai (voir `Ff06`).
    Ff07 { lock: u8, line: u16 },
    /// 0xe0ad8 / 0xe0af8 : commandes 0xff08 et 0xff09, de paramètre D0.w (mot haut de D1).
    Ff08(u16),
    Ff09(u16),
}

/// Enfile une commande de service. Une commande à délai écrit d'abord le verrou du Z80, puis lit le délai
/// (`A5+0x7fc2`) et le mot haut de la commande dans la table des délais.
pub fn service(b: &mut Cps2Bus, s: Service) {
    service_enqueue(b, s);
}

fn service_enqueue(b: &mut Cps2Bus, s: Service) -> Queueing {
    let simple = |d1: u32| Command { d1, d2: 0, d3: 0 };
    let cmd = match s {
        Service::GeneralStop => simple(0xff00),
        Service::Ff05 => simple(0xff05),
        Service::Ff06 { lock, line } => has_delay(b, 0xff06, lock, line),
        Service::Ff07 { lock, line } => has_delay(b, 0xff07, lock, line),
        Service::Ff08(p) => simple((p as u32) << 16 | 0xff08),
        Service::Ff09(p) => simple((p as u32) << 16 | 0xff09),
    };
    enqueue(b, cmd)
}

fn has_delay(b: &mut Cps2Bus, command: u16, lock: u8, line: u16) -> Command {
    wr8(b, SERVICE_LOCK, lock);
    let line = DELAYS.wrapping_add(line as i16 as u32);
    let delay = rd16(b, line);
    wr16(b, a5(ram::DELAY), delay);
    let high = rd16(b, line + 2) as u32;
    Command { d1: high << 16 | command as u32, d2: 0, d3: 0 }
}

/// Native d'une commande de service : tous les registres sont rendus (movem sur la pile), seuls les drapeaux de
/// 0xe0848 restent.
fn service_native_run(c: &mut Cpu, b: &mut Cps2Bus, s: Service) {
    let e = service_enqueue(b, s);
    let (d0, a1) = (c.d[0], c.a[1]);
    finish_enqueue(c, &e);
    c.d[0] = d0;
    c.a[1] = a1;
    rts(c, b);
}

/// Commande de service de l'entrée `entry`, d'après D0 et D1 (pour les natives).
fn service_in(entry: u32, c: &Cpu) -> Service {
    let (d0, d1) = (c.d[0] as u16, c.d[1] as u8);
    match entry {
        0xe0882 => Service::GeneralStop,
        0xe0a10 => Service::Ff05,
        0xe0a2e => Service::Ff06 { lock: d1, line: d0 },
        0xe0a40 => Service::Ff06 { lock: 0xff, line: d0 },
        0xe0a70 => Service::Ff07 { lock: d1, line: d0 },
        0xe0a82 => Service::Ff07 { lock: 0, line: d0 },
        0xe0ad8 => Service::Ff08(d0),
        0xe0af8 => Service::Ff09(d0),
        _ => unreachable!("{entry:x} n'est pas une commande de service"),
    }
}

const fn service_native(entry: u32, name: &'static str, touches_sound: bool) -> Native {
    Native { entry, name, run: crate::Body::CpuIn(|c, b, entry| service_native_run(c, b, service_in(entry, c))), touches_sound, outputs: regs::CCR }
}

pub const SERVICE_FF00: Native = service_native(0xe0882, "son_service_ff00", false);
pub const SERVICE_FF05: Native = service_native(0xe0a10, "son_service_ff05", false);
pub const SERVICE_FF06_D1: Native = service_native(0xe0a2e, "son_service_ff06", true);
pub const SERVICE_FF06: Native = service_native(0xe0a40, "son_service_ff06", true);
pub const SERVICE_FF07_D1: Native = service_native(0xe0a70, "son_service_ff07", true);
pub const SERVICE_FF07: Native = service_native(0xe0a82, "son_service_ff07", true);
pub const SERVICE_FF08: Native = service_native(0xe0ad8, "son_service_ff08", false);
pub const SERVICE_FF09: Native = service_native(0xe0af8, "son_service_ff09", false);

/// Entrées qui testent la version à palette alternative (`A5+0x70` non nul) avant d'appeler le wrapper
/// `normal` ou, dans cette version, le wrapper `alternate` : 0xe2240 / 0xe22e8 (bruit du personnage),
/// 0xe2430 (bruit par objet), 0xe27da (bruit par genre), 0xe285c (son 0xd5 ou 0xd6, prière du clerc), 0xe2a74 (son
/// 0xf7 ou 0xf6, tempête du magicien), 0xe296c (son 0xea ou 0xe6, lune de l'elfe : trou trouvé par le bot
/// `longue_d`).
const BY_VERSION: [(u32, u32, u32); 7] = [
    (0xe2240, 0xe2248, 0xe2390),
    (0xe22e8, 0xe22f0, 0xe2390),
    (0xe2430, 0xe2438, 0xe24d8),
    (0xe27da, 0xe27e2, 0xe28a4),
    (0xe285c, 0xe2864, 0xe28a4),
    (0xe2a74, 0xe2a7a, 0xe2a34),
    (0xe296c, 0xe2974, 0xe29f4),
];

/// Version à palette alternative (`A5+0x70` non nul).
fn version_alternative(b: &mut Cps2Bus) -> bool {
    rd16(b, a5(0x70)) != 0
}

/// 0xee044 : pile privée des wrappers vide (pointeur = sa propre adresse).
pub fn clear_private_stack(b: &mut Cps2Bus) {
    let p = a5(ram::PRIVATE_STACK);
    wr32(b, p, p);
}

/// `clear_private_stack` ; A0 = le pointeur, drapeaux de sa valeur.
fn init_private_stack(c: &mut Cpu, b: &mut Cps2Bus) {
    clear_private_stack(b);
    let p = a5(ram::PRIVATE_STACK);
    c.a[0] = p;
    c.flags_logic(p, Size::Long);
    rts(c, b);
}

pub const INIT_PRIVATE_STACK: Native = Native {
    entry: 0xee044,
    name: "son_init_pile",
    run: crate::Body::Processeur(init_private_stack),
    touches_sound: false,
    outputs: regs::a(0) | regs::CCR,
};

/// 0xec238 : son vide (un simple `rts`), cible de l'aiguillage 0xec200 pour les objets sans son.
pub const NONE: Native = Native {
    entry: 0xec238,
    name: "son_aucun",
    run: crate::Body::Processeur(rts),
    touches_sound: false,
    outputs: regs::ALL,
};
