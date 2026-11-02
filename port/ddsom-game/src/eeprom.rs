//! EEPROM série 93C46 et comptabilité (M2).
//!
//! L'EEPROM (64 mots de 16 bits) est pilotée bit à bit par le port 0x804040 (bit 12 : DI, bit 13 :
//! CLK, bit 14 : CS) ; sa sortie DO se lit dans le bit 0 de 0x804020. Le jeu en garde une copie en RAM
//! (`CONFIG`, 64 mots) et la réécrit par blocs : l'écriture est suivie d'une relecture dans `COPY`
//! et d'une comparaison, avec 10 essais.
//!
//! Les routines natives émettent exactement la même suite d'écritures sur le port (l'ordre compte :
//! l'EEPROM émulée est une machine à états).
//!
//! Paramètres de transfert (en RAM de travail, dans l'ordre où le jeu les pose) :
//!
//! | Variable | Rôle |
//! |---|---|
//! | `WORDS` | nombre de mots moins un |
//! | `START` | décalage en octets dans la copie (adresse EEPROM = décalage / 2) |
//! | `BUFFER` / `CURRENT_WORDS` / `CURRENT_START` | copies de travail du transfert en cours |
//!
//! Compteurs de comptabilité (`compter_*`) : un compteur long en RAM, son image dans `CONFIG`, et le
//! bloc de deux mots de l'EEPROM où il est enregistré.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_X};

use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8};
use crate::{regs, Native};

pub mod dev_card;

/// Port de sortie (lignes EEPROM) et d'entrée (DO sur le bit 0).
const PORT_EXIT: u32 = 0x804040;
const PORT_ENTRY: u32 = 0x804020;
/// Lignes de l'EEPROM dans le mot de sortie.
const DI: u16 = 0x1000;
const CLK: u16 = 0x2000;
const CS: u16 = 0x4000;

/// Octets de commande envoyés (10 bits : un 0 de remplissage, le bit de départ, le code opération 2 bits, 6 bits d'adresse).
const CMD_READ: u16 = 0x180;
const CMD_CLEAR: u16 = 0x1c0;
const CMD_WRITE: u16 = 0x140;
const CMD_ALLOW: u16 = 0x130;

/// Variables, relatives à A5.
mod ram {
    /// Copie du registre 0x804040 hors lignes EEPROM (mot) et bits de compteurs (octet).
    pub const WORD_OUT: u32 = 0x16;
    pub const BYTE_OUT: u32 = 0x18;
    /// Non nul pendant un accès EEPROM (la lecture des entrées n'y touche pas).
    pub const BUSY: u32 = 0x74;
    /// Mode « carte de développement » : pas de comptabilité.
    pub const NO_ACCOUNTING: u32 = 0x82;
    pub const BUFFER: u32 = 0xffff_9e8e;
    pub const WORDS: u32 = 0xffff_9e92;
    pub const CURRENT_WORDS: u32 = 0xffff_9e94;
    pub const START: u32 = 0xffff_9e96;
    pub const CURRENT_START: u32 = 0xffff_9e98;
    pub const TESTS: u32 = 0xffff_9e9a;
    /// Copie en RAM du contenu de l'EEPROM (64 mots) et tampon de relecture.
    pub const CONFIG: u32 = 0xffff_9d8e;
    pub const COPY: u32 = 0xffff_9e0e;
}

/// Essais d'écriture / de lecture.
const TESTS: u8 = 10;
/// Durée maximale de l'attente de fin d'écriture (itérations).
const WAIT_MAX: u32 = 0x2ffff;

/// Adresse `A5 + off` (A5 vaut toujours `mem::A5`).
const fn a5(off: u32) -> u32 {
    crate::mem::A5.wrapping_add(off)
}

/// L'état des lignes de l'EEPROM, que l'original garde dans D2 d'un bout à l'autre du transfert.
struct Lines {
    state: u16,
}

impl Lines {
    /// 0x2da2c : écrit les lignes sur le port, complétées par les autres bits de sortie.
    fn write(&mut self, b: &mut Cps2Bus) {
        let mut v = self.state | rd16(b, a5(ram::WORD_OUT));
        v &= !6;
        v |= rd8(b, a5(ram::BYTE_OUT)) as u16;
        wr16(b, PORT_EXIT, v);
        self.state = v & (DI | CLK | CS);
    }

    /// 0x2da22 : une impulsion d'horloge.
    fn pulse(&mut self, b: &mut Cps2Bus) {
        self.state |= CLK;
        self.write(b);
        self.state &= !CLK;
        self.write(b);
    }

    /// Fin de commande : CS bas, une écriture et une impulsion.
    fn release(&mut self, b: &mut Cps2Bus) {
        self.state &= !CS;
        self.write(b);
        self.pulse(b);
    }

    /// 0x2d9f4 : CS haut, puis les 10 bits de `command` (poids fort d'abord), chacun suivi d'une impulsion.
    fn command(&mut self, b: &mut Cps2Bus, command: u16) {
        self.state |= CS;
        self.write(b);
        for i in (0..10).rev() {
            self.state = self.state & !DI | ((command >> i & 1) << 12);
            self.write(b);
            self.pulse(b);
        }
    }

    /// 0x2d9c0 : attend la fin d'une écriture (DO haut). Faux si le délai est dépassé.
    fn wait(&mut self, b: &mut Cps2Bus) -> bool {
        self.state |= CS;
        self.write(b);
        let mut remaining = WAIT_MAX;
        loop {
            remaining -= 1;
            if remaining == 0 {
                return false;
            }
            self.pulse(b);
            if rd16(b, PORT_ENTRY) & 1 != 0 {
                break;
            }
        }
        self.state &= !CS;
        self.write(b);
        self.pulse(b);
        true
    }
}

/// Adresse EEPROM (6 bits) d'un décalage en octets de la copie.
fn address(offset: u16) -> u16 {
    (offset >> 1) & 0x3f
}

/// Paramètres du transfert en cours.
struct Transfer {
    buffer: u32,
    words: u16,
    start: u16,
}

fn transfer(b: &mut Cps2Bus) -> Transfer {
    Transfer {
        buffer: rd32(b, a5(ram::BUFFER)),
        words: rd16(b, a5(ram::CURRENT_WORDS)),
        start: rd16(b, a5(ram::CURRENT_START)),
    }
}

/// Mot suivant de la copie, à partir de `start` octets du tampon.
fn buffer_start(t: &Transfer) -> u32 {
    t.buffer.wrapping_add(t.start as i16 as u32)
}

// ---------------------------------------------------------------------------------------------
// Lecture et écriture

/// 0x2d846 : lit `words + 1` mots depuis l'adresse `start / 2` de l'EEPROM vers le tampon.
fn read(b: &mut Cps2Bus) {
    wr8(b, a5(ram::BUSY), 1);
    let mut lines = Lines { state: 0 };
    let t = transfer(b);
    let mut ptr = buffer_start(&t);
    let mut cmd = address(t.start) | CMD_READ;
    for _ in 0..=t.words {
        lines.command(b, cmd);
        let mut word = 0u16;
        for _ in 0..16 {
            lines.pulse(b);
            word = word << 1 | rd16(b, PORT_ENTRY) & 1;
        }
        lines.pulse(b);
        lines.release(b);
        wr16(b, ptr, word);
        ptr = ptr.wrapping_add(2);
        cmd = cmd.wrapping_add(1);
    }
    wr8(b, a5(ram::BUSY), 0);
}

/// Résultat d'un transfert : `true` si tout s'est bien passé.
type Success = bool;

/// 0x2d90e : écrit `words + 1` mots du tampon dans l'EEPROM (autorisation d'écriture, puis pour chaque
/// mot : effacement, attente, écriture, attente).
fn write(b: &mut Cps2Bus) -> Success {
    wr8(b, a5(ram::BUSY), 1);
    let mut lines = Lines { state: 0 };
    lines.command(b, CMD_ALLOW);
    lines.release(b);
    let t = transfer(b);
    let mut ptr = buffer_start(&t);
    let mut cmd = address(t.start);
    for _ in 0..=t.words {
        let cmd_clear = cmd & 0x3f | CMD_CLEAR;
        lines.command(b, cmd_clear);
        lines.pulse(b);
        lines.release(b);
        if !lines.wait(b) {
            wr8(b, a5(ram::BUSY), 0);
            return false;
        }
        let cmd_write = cmd_clear & 0x3f | CMD_WRITE;
        lines.command(b, cmd_write);
        let word = rd16(b, ptr);
        ptr = ptr.wrapping_add(2);
        for i in (0..16).rev() {
            lines.state = lines.state & !DI | ((word >> i & 1) << 12);
            lines.write(b);
            lines.pulse(b);
        }
        lines.state &= !CS;
        lines.write(b);
        lines.pulse(b);
        lines.pulse(b);
        if !lines.wait(b) {
            wr8(b, a5(ram::BUSY), 0);
            return false;
        }
        lines.state &= !CLK;
        lines.write(b);
        cmd = cmd_write.wrapping_add(1);
    }
    wr8(b, a5(ram::BUSY), 0);
    true
}

fn set_transfer(b: &mut Cps2Bus, buffer: u32) {
    wr32(b, a5(ram::BUFFER), buffer);
    let words = rd16(b, a5(ram::WORDS));
    wr16(b, a5(ram::CURRENT_WORDS), words);
    let start = rd16(b, a5(ram::START));
    wr16(b, a5(ram::CURRENT_START), start);
}

/// Résultat de `moveq #0/-1,D0` : D0, N, Z, V, C ; X est à zéro dans ces routines (dernière
/// instruction touchant X : un `addq` sans retenue).
fn give_back(c: &mut Cpu, b: &mut Cps2Bus, success: Success) {
    c.d[0] = if success { 0 } else { u32::MAX };
    c.flags_logic(c.d[0], Size::Long);
    c.sr &= !SR_X;
    rts(c, b);
}

fn native_read(c: &mut Cpu, b: &mut Cps2Bus) {
    read(b);
    rts(c, b);
}

fn native_write(c: &mut Cpu, b: &mut Cps2Bus) {
    let ok = write(b);
    give_back(c, b, ok);
}

/// 0x2d8aa : écrit la copie en RAM dans l'EEPROM, relit dans `COPY` et compare ; 10 essais.
pub(crate) fn write_verify(b: &mut Cps2Bus) -> Success {
    wr8(b, a5(ram::TESTS), TESTS);
    loop {
        set_transfer(b, a5(ram::CONFIG));
        write(b);
        set_transfer(b, a5(ram::COPY));
        read(b);
        let start = rd16(b, a5(ram::START)) as i16 as u32;
        let words = rd16(b, a5(ram::WORDS));
        let (config, copy) = (a5(ram::CONFIG).wrapping_add(start), a5(ram::COPY).wrapping_add(start));
        let equal = (0..=words as u32).all(|i| rd16(b, config + 2 * i) == rd16(b, copy + 2 * i));
        if equal {
            return true;
        }
        let remainder = rd8(b, a5(ram::TESTS)).wrapping_sub(1);
        wr8(b, a5(ram::TESTS), remainder);
        if remainder == 0 {
            return false;
        }
    }
}

fn native_write_verify(c: &mut Cpu, b: &mut Cps2Bus) {
    let ok = write_verify(b);
    give_back(c, b, ok);
}

/// 0x2d808 : lit l'EEPROM dans la copie en RAM jusqu'à ce que ses deux exemplaires de la
/// configuration (mots 0-23 et 24-47) concordent ; 10 essais.
pub(crate) fn read_verify(b: &mut Cps2Bus) -> Success {
    wr8(b, a5(ram::TESTS), TESTS);
    loop {
        set_transfer(b, a5(ram::CONFIG));
        read(b);
        let config = a5(ram::CONFIG);
        if (0..24).all(|i| rd16(b, config + 2 * i) == rd16(b, config + 0x30 + 2 * i)) {
            return true;
        }
        let remainder = rd8(b, a5(ram::TESTS)).wrapping_sub(1);
        wr8(b, a5(ram::TESTS), remainder);
        if remainder == 0 {
            return false;
        }
    }
}

fn native_read_verify(c: &mut Cpu, b: &mut Cps2Bus) {
    let ok = read_verify(b);
    give_back(c, b, ok);
}

// ---------------------------------------------------------------------------------------------
// Test de l'EEPROM (menu TEST, 8. MEMORY CHECK)

/// Mots de l'EEPROM.
const EEPROM_WORDS: u16 = 0x40;
/// Motifs écrits dans toute l'EEPROM.
const PATTERNS: [u16; 4] = [0xffff, 0xaaaa, 0x5555, 0];

/// Pose le transfert de toute l'EEPROM (`WORDS`, `START`).
pub(crate) fn all_eeprom(b: &mut Cps2Bus) {
    wr16(b, a5(ram::WORDS), EEPROM_WORDS - 1);
    wr16(b, a5(ram::START), 0);
}

/// 0x2da90 : écrit `pattern` dans les 64 mots de l'EEPROM (depuis `COPY`), relit et compare.
fn test_pattern_word(b: &mut Cps2Bus, pattern: u16) -> bool {
    let copy = a5(ram::COPY);
    for i in 0..EEPROM_WORDS as u32 {
        wr16(b, copy + 2 * i, pattern);
    }
    wr32(b, a5(ram::BUFFER), copy);
    wr16(b, a5(ram::CURRENT_WORDS), EEPROM_WORDS - 1);
    wr16(b, a5(ram::CURRENT_START), 0);
    write(b);
    read(b);
    (0..EEPROM_WORDS as u32).all(|i| rd16(b, copy + 2 * i) == pattern)
}

/// 0x2da4a : lit l'EEPROM dans la copie en RAM, la teste avec quatre motifs, puis y réécrit la copie
/// (validée par `config::validate_copy`). Faux si un motif n'a pas été relu.
pub fn tester(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    all_eeprom(b);
    read_verify(b);
    if !PATTERNS.iter().all(|&m| test_pattern_word(b, m)) {
        return false;
    }
    crate::config::validate_copy(c, b);
    all_eeprom(b);
    write_verify(b);
    true
}

// ---------------------------------------------------------------------------------------------
// Compteurs de comptabilité

/// Un compteur long en RAM, son image dans `CONFIG`, et sa place dans l'EEPROM.
struct Counter {
    /// Seulement si cet octet (relatif à A5) est non nul.
    condition: Option<u32>,
    value: u32,
    image: u32,
    /// Décalage en octets dans la copie de l'EEPROM.
    place: u16,
}

const PIECES: Counter = Counter { condition: None, value: 0x76, image: 0xffff_9dee, place: 0x60 };
const STARTS: Counter = Counter { condition: None, value: 0x7a, image: 0xffff_9df2, place: 0x64 };
const CONTINUES: Counter =
    Counter { condition: Some(0x25), value: 0x7e, image: 0xffff_9df6, place: 0x68 };
const GAMES: Counter = Counter { condition: None, value: 0x7fd4, image: 0xffff_9dfa, place: 0x6c };

/// Ce qu'un compteur laisse dans D0 (et les drapeaux) de l'original.
enum Count {
    /// Pas de comptage (condition fausse, ou comptabilité sur la carte de développement) : D0 intact.
    Nothing,
    /// D0 = la nouvelle valeur (inchangée par rapport à son image : pas d'écriture).
    Value(u32),
    /// L'EEPROM a été écrite (0x2d8aa) : D0 = 0 ou -1 selon le succès.
    Written(Success),
}

/// Incrémente le compteur et, s'il a changé par rapport à son image, l'écrit dans l'EEPROM (0x2d8aa).
fn count(b: &mut Cps2Bus, k: &Counter) -> Count {
    if let Some(cond) = k.condition {
        if rd8(b, a5(cond)) == 0 {
            return Count::Nothing;
        }
    }
    let value = rd32(b, a5(k.value)).wrapping_add(1);
    wr32(b, a5(k.value), value);
    if rd8(b, a5(ram::NO_ACCOUNTING)) != 0 {
        return Count::Nothing;
    }
    if value == rd32(b, a5(k.image)) {
        return Count::Value(value);
    }
    wr32(b, a5(k.image), value);
    wr16(b, a5(ram::WORDS), 1);
    wr16(b, a5(ram::START), k.place);
    Count::Written(write_verify(b))
}

/// `count` avec les registres de l'original.
fn count_68k(c: &mut Cpu, b: &mut Cps2Bus, k: &Counter) {
    match count(b, k) {
        Count::Nothing => {}
        Count::Value(v) => c.d[0] = v,
        Count::Written(ok) => {
            c.d[0] = if ok { 0 } else { u32::MAX };
            c.flags_logic(c.d[0], Size::Long);
            c.sr &= !SR_X;
        }
    }
}

macro_rules! native_counter {
    ($fn:ident, $k:expr) => {
        fn $fn(c: &mut Cpu, b: &mut Cps2Bus) {
            count_68k(c, b, &$k);
            rts(c, b);
        }
    };
}

native_counter!(count_coins, PIECES);
native_counter!(count_starts, STARTS);
native_counter!(count_continues, CONTINUES);
native_counter!(count_games, GAMES);

// ---------------------------------------------------------------------------------------------
// Comptabilité complète : EEPROM puis compteurs de la RAM supplémentaire

/// Compteur de comptabilité complet (0x2e448 pièces, 0x2e49a départs) : le compteur de l'EEPROM,
/// puis, si la RAM supplémentaire est utilisée (`NO_ACCOUNTING` ≠ 0, carte de développement), les trois
/// exemplaires du compteur dans cette RAM (`dev_card`).
struct Full {
    counter: &'static Counter,
    card: fn(&mut Cps2Bus) -> u32,
}

const FULL_COINS: Full = Full { counter: &PIECES, card: dev_card::pieces };
const FULL_STARTS: Full = Full { counter: &STARTS, card: dev_card::starts };

/// Une pièce (0x2e448) ou un départ (0x2e49a) comptés : l'EEPROM, puis la RAM de la carte de développement. Rend la
/// valeur de la carte (`None` sur une borne).
fn full_accounting(b: &mut Cps2Bus, k: &Full) -> Option<u32> {
    count(b, k.counter);
    (rd8(b, a5(ram::NO_ACCOUNTING)) != 0).then(|| (k.card)(b))
}

/// 0x2e448 : une pièce comptée (lecture des monnayeurs, VBLANK).
pub fn count_coin(b: &mut Cps2Bus) {
    full_accounting(b, &FULL_COINS);
}

/// 0x2e49a : un départ compté (bouton de départ du premier monnayeur).
pub fn count_start(b: &mut Cps2Bus) {
    full_accounting(b, &FULL_STARTS);
}

/// `full_accounting` avec les registres de l'original (D0 du compteur, drapeaux du dernier test).
fn full_accounting_68k(c: &mut Cpu, b: &mut Cps2Bus, k: &Full) {
    count_68k(c, b, k.counter);
    let active = rd8(b, a5(ram::NO_ACCOUNTING)) != 0;
    c.flags_logic(active as u32, Size::Byte);
    if active {
        let value = (k.card)(b);
        c.flags_logic(value, Size::Long);
    }
    rts(c, b);
}

fn native_count_coins(c: &mut Cpu, b: &mut Cps2Bus) {
    full_accounting_68k(c, b, &FULL_COINS);
}

fn native_count_starts(c: &mut Cpu, b: &mut Cps2Bus) {
    full_accounting_68k(c, b, &FULL_STARTS);
}

// ---------------------------------------------------------------------------------------------
// VBLANK

/// 0x2dcf6 (VBLANK) : lève le bit 0 du mot 0x664000 de la RAM supplémentaire (si la comptabilité est active). Rend le
/// mot final (D0.w de l'original), s'il a été lu.
pub fn vblank(b: &mut Cps2Bus) -> Option<u16> {
    if rd8(b, a5(ram::NO_ACCOUNTING)) == 0 {
        return None;
    }
    let word = rd16(b, 0x664000);
    if word & 1 == 0 {
        wr16(b, 0x664000, word | 1);
    }
    Some(word | 1)
}

fn vblank_accounting(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(word) = vblank(b) {
        c.d[0] = c.d[0] & 0xffff_0000 | word as u32;
    }
    rts(c, b);
}

macro_rules! native {
    ($ident:ident, $entry:expr, $name:expr, $run:expr, $exits:expr) => {
        pub const $ident: Native = Native {
            entry: $entry,
            name: $name,
            run: crate::Body::Processeur($run),
            touches_sound: false,
            outputs: $exits,
        };
    };
}

native!(READ, 0x2d846, "eeprom_lire", native_read, 0);
native!(WRITE, 0x2d90e, "eeprom_ecrire", native_write, regs::d(0) | regs::CCR);
native!(WRITE_VERIFY, 0x2d8aa, "eeprom_ecrire_verifier", native_write_verify, regs::d(0) | regs::CCR);
native!(READ_VERIFY, 0x2d808, "eeprom_lire_verifier", native_read_verify, regs::d(0) | regs::CCR);
native!(COUNT_COINS, 0x2daca, "compta_pieces", count_coins, regs::d(0));
native!(COUNT_STARTS, 0x2daf4, "compta_departs", count_starts, regs::d(0));
native!(COUNT_CONTINUES, 0x2db1e, "compta_continues", count_continues, regs::d(0));
native!(COUNT_GAMES, 0x2db4e, "compta_parties", count_games, regs::d(0));
native!(VBLANK_ACCOUNTING, 0x2dcf6, "compta_vblank", vblank_accounting, regs::d(0));
native!(FULL_COINS_NATIVE, 0x2e448, "compta_complete_pieces", native_count_coins, regs::CCR);
native!(FULL_STARTS_NATIVE, 0x2e49a, "compta_complete_departs", native_count_starts, regs::CCR);

// ---------------------------------------------------------------------------------------------
// Parties gratuites et RAM de la carte de développement

/// Jeu gratuit (A5+0x25 non nul) : le compteur des parties gratuites (`CONTINUES`, 0x2db1e) ; sur la carte de
/// développement (`NO_ACCOUNTING` non nul : jamais sur une borne), le compteur correspondant de la RAM supplémentaire.
const FREE_PLAY: u32 = 0x25;

/// `tst.b (NO_ACCOUNTING,A5)` : rend vrai si la RAM supplémentaire est utilisée (drapeaux comme l'original).
fn has_dev_card(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    let v = rd8(b, a5(ram::NO_ACCOUNTING));
    c.flags_logic(v as u32, Size::Byte);
    v != 0
}

/// 0x2e4ec (départ d'un joueur) : en jeu gratuit, le compteur des parties gratuites ; sur la carte de développement,
/// celui de sa RAM.
pub fn count_game(b: &mut Cps2Bus) {
    let free = rd8(b, a5(FREE_PLAY)) != 0;
    if free {
        count(b, &CONTINUES);
    }
    if rd8(b, a5(ram::NO_ACCOUNTING)) != 0 {
        if free {
            dev_card::free_games(b);
        } else {
            dev_card::paid_games(b);
        }
    }
}

/// 0x2dc9a : vérification de la RAM de la carte de développement.
pub fn initialize_accounting(b: &mut Cps2Bus) {
    if rd8(b, a5(ram::NO_ACCOUNTING)) != 0 {
        dev_card::start_up(b);
    }
}

/// 0x2e630 : statistique du personnage choisi (carte de développement seulement).
pub fn stat_chosen_character(b: &mut Cps2Bus, character: u16) {
    if rd8(b, a5(ram::NO_ACCOUNTING)) != 0 {
        dev_card::character_part(b, character);
    }
}

/// 0x2e696 : statistique de la durée d'une partie, en images (carte de développement seulement).
pub fn stat_game_duration(b: &mut Cps2Bus, images: u32) {
    if rd8(b, a5(ram::NO_ACCOUNTING)) != 0 {
        dev_card::game_duration(b, images);
    }
}

fn count_games_acc(c: &mut Cpu, b: &mut Cps2Bus) {
    let free = rd8(b, a5(FREE_PLAY)) != 0;
    if free {
        count_68k(c, b, &CONTINUES);
    }
    if has_dev_card(c, b) {
        let value = if free { dev_card::free_games(b) } else { dev_card::paid_games(b) };
        c.flags_logic(value, Size::Long);
    }
    rts(c, b);
}

/// 0x2dc9a (démarrage) : vérification de la RAM supplémentaire, seulement sur la carte de développement.
fn accounting_init(c: &mut Cpu, b: &mut Cps2Bus) {
    if has_dev_card(c, b) {
        dev_card::start_up(b);
    }
    rts(c, b);
}

/// 0x2e8e4 (reset avec les boutons de réinitialisation tenus) : remise à zéro de la RAM supplémentaire.
pub fn reset_card(c: &mut Cpu, b: &mut Cps2Bus) {
    if has_dev_card(c, b) {
        dev_card::reset(b);
    }
}

/// 0x2e630 (personnage choisi, D0) : statistique de la RAM de la carte de développement ; sur une borne, rien.
fn stat_character(c: &mut Cpu, b: &mut Cps2Bus) {
    if has_dev_card(c, b) {
        dev_card::character_part(b, c.d[0] as u16);
    }
    rts(c, b);
}

/// 0x2e696 (durée de partie, D0, en images) : idem.
fn stat_duration(c: &mut Cpu, b: &mut Cps2Bus) {
    if has_dev_card(c, b) {
        dev_card::game_duration(b, c.d[0]);
    }
    rts(c, b);
}

pub const STAT_CHARACTER: Native = Native {
    entry: 0x2e630,
    name: "compta_stat_personnage",
    run: crate::Body::Processeur(stat_character),
    touches_sound: false,
    outputs: regs::CCR,
};

pub const STAT_DURATION: Native = Native {
    entry: 0x2e696,
    name: "compta_stat_duree",
    run: crate::Body::Processeur(stat_duration),
    touches_sound: false,
    outputs: regs::CCR,
};

pub const COUNT_GAMES_ACC: Native = Native {
    entry: 0x2e4ec,
    name: "compta_parties",
    run: crate::Body::Processeur(count_games_acc),
    touches_sound: false,
    outputs: regs::CCR,
};

pub const ACCOUNTING_INIT: Native = Native {
    entry: 0x2dc9a,
    name: "compta_init",
    run: crate::Body::Processeur(accounting_init),
    touches_sound: false,
    outputs: regs::CCR,
};
