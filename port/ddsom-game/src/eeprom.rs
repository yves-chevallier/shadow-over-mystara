//! EEPROM série 93C46 et comptabilité (M2).
//!
//! L'EEPROM (64 mots de 16 bits) est pilotée bit à bit par le port 0x804040 (bit 12 : DI, bit 13 :
//! CLK, bit 14 : CS) ; sa sortie DO se lit dans le bit 0 de 0x804020. Le jeu en garde une copie en RAM
//! (`CONFIG`, 64 mots) et la réécrit par blocs : l'écriture est suivie d'une relecture dans `COPIE`
//! et d'une comparaison, avec 10 essais.
//!
//! Les routines natives émettent exactement la même suite d'écritures sur le port (l'ordre compte :
//! l'EEPROM émulée est une machine à états).
//!
//! Paramètres de transfert (en RAM de travail, dans l'ordre où le jeu les pose) :
//!
//! | Variable | Rôle |
//! |---|---|
//! | `MOTS` | nombre de mots moins un |
//! | `DEBUT` | décalage en octets dans la copie (adresse EEPROM = décalage / 2) |
//! | `TAMPON` / `MOTS_COURANT` / `DEBUT_COURANT` | copies de travail du transfert en cours |
//!
//! Compteurs de comptabilité (`compter_*`) : un compteur long en RAM, son image dans `CONFIG`, et le
//! bloc de deux mots de l'EEPROM où il est enregistré.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_X};

use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8};
use crate::{regs, Native};

pub mod carte_dev;

/// Port de sortie (lignes EEPROM) et d'entrée (DO sur le bit 0).
const PORT_SORTIE: u32 = 0x804040;
const PORT_ENTREE: u32 = 0x804020;
/// Lignes de l'EEPROM dans le mot de sortie.
const DI: u16 = 0x1000;
const CLK: u16 = 0x2000;
const CS: u16 = 0x4000;

/// Octets de commande envoyés (10 bits : un 0 de remplissage, le bit de départ, le code opération 2 bits, 6 bits d'adresse).
const CMD_LIRE: u16 = 0x180;
const CMD_EFFACER: u16 = 0x1c0;
const CMD_ECRIRE: u16 = 0x140;
const CMD_AUTORISER: u16 = 0x130;

/// Variables, relatives à A5.
mod ram {
    /// Copie du registre 0x804040 hors lignes EEPROM (mot) et bits de compteurs (octet).
    pub const SORTIE_MOT: u32 = 0x16;
    pub const SORTIE_OCTET: u32 = 0x18;
    /// Non nul pendant un accès EEPROM (la lecture des entrées n'y touche pas).
    pub const OCCUPE: u32 = 0x74;
    /// Mode « carte de développement » : pas de comptabilité.
    pub const SANS_COMPTA: u32 = 0x82;
    pub const TAMPON: u32 = 0xffff_9e8e;
    pub const MOTS: u32 = 0xffff_9e92;
    pub const MOTS_COURANT: u32 = 0xffff_9e94;
    pub const DEBUT: u32 = 0xffff_9e96;
    pub const DEBUT_COURANT: u32 = 0xffff_9e98;
    pub const ESSAIS: u32 = 0xffff_9e9a;
    /// Copie en RAM du contenu de l'EEPROM (64 mots) et tampon de relecture.
    pub const CONFIG: u32 = 0xffff_9d8e;
    pub const COPIE: u32 = 0xffff_9e0e;
}

/// Essais d'écriture / de lecture.
const ESSAIS: u8 = 10;
/// Durée maximale de l'attente de fin d'écriture (itérations).
const ATTENTE_MAX: u32 = 0x2ffff;

/// Adresse `A5 + off` (A5 vaut toujours `mem::A5`).
const fn a5(off: u32) -> u32 {
    crate::mem::A5.wrapping_add(off)
}

/// L'état des lignes de l'EEPROM, que l'original garde dans D2 d'un bout à l'autre du transfert.
struct Lignes {
    etat: u16,
}

impl Lignes {
    /// 0x2da2c : écrit les lignes sur le port, complétées par les autres bits de sortie.
    fn ecrire(&mut self, b: &mut Cps2Bus) {
        let mut v = self.etat | rd16(b, a5(ram::SORTIE_MOT));
        v &= !6;
        v |= rd8(b, a5(ram::SORTIE_OCTET)) as u16;
        wr16(b, PORT_SORTIE, v);
        self.etat = v & (DI | CLK | CS);
    }

    /// 0x2da22 : une impulsion d'horloge.
    fn impulsion(&mut self, b: &mut Cps2Bus) {
        self.etat |= CLK;
        self.ecrire(b);
        self.etat &= !CLK;
        self.ecrire(b);
    }

    /// Fin de commande : CS bas, une écriture et une impulsion.
    fn relacher(&mut self, b: &mut Cps2Bus) {
        self.etat &= !CS;
        self.ecrire(b);
        self.impulsion(b);
    }

    /// 0x2d9f4 : CS haut, puis les 10 bits de `commande` (poids fort d'abord), chacun suivi d'une impulsion.
    fn commande(&mut self, b: &mut Cps2Bus, commande: u16) {
        self.etat |= CS;
        self.ecrire(b);
        for i in (0..10).rev() {
            self.etat = self.etat & !DI | ((commande >> i & 1) << 12);
            self.ecrire(b);
            self.impulsion(b);
        }
    }

    /// 0x2d9c0 : attend la fin d'une écriture (DO haut). Faux si le délai est dépassé.
    fn attendre(&mut self, b: &mut Cps2Bus) -> bool {
        self.etat |= CS;
        self.ecrire(b);
        let mut restant = ATTENTE_MAX;
        loop {
            restant -= 1;
            if restant == 0 {
                return false;
            }
            self.impulsion(b);
            if rd16(b, PORT_ENTREE) & 1 != 0 {
                break;
            }
        }
        self.etat &= !CS;
        self.ecrire(b);
        self.impulsion(b);
        true
    }
}

/// Adresse EEPROM (6 bits) d'un décalage en octets de la copie.
fn adresse(decalage: u16) -> u16 {
    (decalage >> 1) & 0x3f
}

/// Paramètres du transfert en cours.
struct Transfert {
    tampon: u32,
    mots: u16,
    debut: u16,
}

fn transfert(b: &mut Cps2Bus) -> Transfert {
    Transfert {
        tampon: rd32(b, a5(ram::TAMPON)),
        mots: rd16(b, a5(ram::MOTS_COURANT)),
        debut: rd16(b, a5(ram::DEBUT_COURANT)),
    }
}

/// Mot suivant de la copie, à partir de `debut` octets du tampon.
fn debut_tampon(t: &Transfert) -> u32 {
    t.tampon.wrapping_add(t.debut as i16 as u32)
}

// ---------------------------------------------------------------------------------------------
// Lecture et écriture

/// 0x2d846 : lit `mots + 1` mots depuis l'adresse `debut / 2` de l'EEPROM vers le tampon.
fn lire(b: &mut Cps2Bus) {
    wr8(b, a5(ram::OCCUPE), 1);
    let mut lignes = Lignes { etat: 0 };
    let t = transfert(b);
    let mut ptr = debut_tampon(&t);
    let mut cmd = adresse(t.debut) | CMD_LIRE;
    for _ in 0..=t.mots {
        lignes.commande(b, cmd);
        let mut mot = 0u16;
        for _ in 0..16 {
            lignes.impulsion(b);
            mot = mot << 1 | rd16(b, PORT_ENTREE) & 1;
        }
        lignes.impulsion(b);
        lignes.relacher(b);
        wr16(b, ptr, mot);
        ptr = ptr.wrapping_add(2);
        cmd = cmd.wrapping_add(1);
    }
    wr8(b, a5(ram::OCCUPE), 0);
}

/// Résultat d'un transfert : `true` si tout s'est bien passé.
type Succes = bool;

/// 0x2d90e : écrit `mots + 1` mots du tampon dans l'EEPROM (autorisation d'écriture, puis pour chaque
/// mot : effacement, attente, écriture, attente).
fn ecrire(b: &mut Cps2Bus) -> Succes {
    wr8(b, a5(ram::OCCUPE), 1);
    let mut lignes = Lignes { etat: 0 };
    lignes.commande(b, CMD_AUTORISER);
    lignes.relacher(b);
    let t = transfert(b);
    let mut ptr = debut_tampon(&t);
    let mut cmd = adresse(t.debut);
    for _ in 0..=t.mots {
        let cmd_effacer = cmd & 0x3f | CMD_EFFACER;
        lignes.commande(b, cmd_effacer);
        lignes.impulsion(b);
        lignes.relacher(b);
        if !lignes.attendre(b) {
            wr8(b, a5(ram::OCCUPE), 0);
            return false;
        }
        let cmd_ecrire = cmd_effacer & 0x3f | CMD_ECRIRE;
        lignes.commande(b, cmd_ecrire);
        let mot = rd16(b, ptr);
        ptr = ptr.wrapping_add(2);
        for i in (0..16).rev() {
            lignes.etat = lignes.etat & !DI | ((mot >> i & 1) << 12);
            lignes.ecrire(b);
            lignes.impulsion(b);
        }
        lignes.etat &= !CS;
        lignes.ecrire(b);
        lignes.impulsion(b);
        lignes.impulsion(b);
        if !lignes.attendre(b) {
            wr8(b, a5(ram::OCCUPE), 0);
            return false;
        }
        lignes.etat &= !CLK;
        lignes.ecrire(b);
        cmd = cmd_ecrire.wrapping_add(1);
    }
    wr8(b, a5(ram::OCCUPE), 0);
    true
}

fn poser_transfert(b: &mut Cps2Bus, tampon: u32) {
    wr32(b, a5(ram::TAMPON), tampon);
    let mots = rd16(b, a5(ram::MOTS));
    wr16(b, a5(ram::MOTS_COURANT), mots);
    let debut = rd16(b, a5(ram::DEBUT));
    wr16(b, a5(ram::DEBUT_COURANT), debut);
}

/// Résultat de `moveq #0/-1,D0` : D0, N, Z, V, C ; X est à zéro dans ces routines (dernière
/// instruction touchant X : un `addq` sans retenue).
fn rendre(c: &mut Cpu, b: &mut Cps2Bus, succes: Succes) {
    c.d[0] = if succes { 0 } else { u32::MAX };
    c.flags_logic(c.d[0], Size::Long);
    c.sr &= !SR_X;
    rts(c, b);
}

fn natif_lire(c: &mut Cpu, b: &mut Cps2Bus) {
    lire(b);
    rts(c, b);
}

fn natif_ecrire(c: &mut Cpu, b: &mut Cps2Bus) {
    let ok = ecrire(b);
    rendre(c, b, ok);
}

/// 0x2d8aa : écrit la copie en RAM dans l'EEPROM, relit dans `COPIE` et compare ; 10 essais.
pub(crate) fn ecrire_verifier(b: &mut Cps2Bus) -> Succes {
    wr8(b, a5(ram::ESSAIS), ESSAIS);
    loop {
        poser_transfert(b, a5(ram::CONFIG));
        ecrire(b);
        poser_transfert(b, a5(ram::COPIE));
        lire(b);
        let debut = rd16(b, a5(ram::DEBUT)) as i16 as u32;
        let mots = rd16(b, a5(ram::MOTS));
        let (config, copie) = (a5(ram::CONFIG).wrapping_add(debut), a5(ram::COPIE).wrapping_add(debut));
        let egal = (0..=mots as u32).all(|i| rd16(b, config + 2 * i) == rd16(b, copie + 2 * i));
        if egal {
            return true;
        }
        let reste = rd8(b, a5(ram::ESSAIS)).wrapping_sub(1);
        wr8(b, a5(ram::ESSAIS), reste);
        if reste == 0 {
            return false;
        }
    }
}

fn natif_ecrire_verifier(c: &mut Cpu, b: &mut Cps2Bus) {
    let ok = ecrire_verifier(b);
    rendre(c, b, ok);
}

/// 0x2d808 : lit l'EEPROM dans la copie en RAM jusqu'à ce que ses deux exemplaires de la
/// configuration (mots 0-23 et 24-47) concordent ; 10 essais.
pub(crate) fn lire_verifier(b: &mut Cps2Bus) -> Succes {
    wr8(b, a5(ram::ESSAIS), ESSAIS);
    loop {
        poser_transfert(b, a5(ram::CONFIG));
        lire(b);
        let config = a5(ram::CONFIG);
        if (0..24).all(|i| rd16(b, config + 2 * i) == rd16(b, config + 0x30 + 2 * i)) {
            return true;
        }
        let reste = rd8(b, a5(ram::ESSAIS)).wrapping_sub(1);
        wr8(b, a5(ram::ESSAIS), reste);
        if reste == 0 {
            return false;
        }
    }
}

fn natif_lire_verifier(c: &mut Cpu, b: &mut Cps2Bus) {
    let ok = lire_verifier(b);
    rendre(c, b, ok);
}

// ---------------------------------------------------------------------------------------------
// Test de l'EEPROM (menu TEST, 8. MEMORY CHECK)

/// Mots de l'EEPROM.
const MOTS_EEPROM: u16 = 0x40;
/// Motifs écrits dans toute l'EEPROM.
const MOTIFS: [u16; 4] = [0xffff, 0xaaaa, 0x5555, 0];

/// Pose le transfert de toute l'EEPROM (`MOTS`, `DEBUT`).
pub(crate) fn toute_eeprom(b: &mut Cps2Bus) {
    wr16(b, a5(ram::MOTS), MOTS_EEPROM - 1);
    wr16(b, a5(ram::DEBUT), 0);
}

/// 0x2da90 : écrit `motif` dans les 64 mots de l'EEPROM (depuis `COPIE`), relit et compare.
fn tester_motif(b: &mut Cps2Bus, motif: u16) -> bool {
    let copie = a5(ram::COPIE);
    for i in 0..MOTS_EEPROM as u32 {
        wr16(b, copie + 2 * i, motif);
    }
    wr32(b, a5(ram::TAMPON), copie);
    wr16(b, a5(ram::MOTS_COURANT), MOTS_EEPROM - 1);
    wr16(b, a5(ram::DEBUT_COURANT), 0);
    ecrire(b);
    lire(b);
    (0..MOTS_EEPROM as u32).all(|i| rd16(b, copie + 2 * i) == motif)
}

/// 0x2da4a : lit l'EEPROM dans la copie en RAM, la teste avec quatre motifs, puis y réécrit la copie
/// (validée par `config::valider_copie`). Faux si un motif n'a pas été relu.
pub fn tester(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    toute_eeprom(b);
    lire_verifier(b);
    if !MOTIFS.iter().all(|&m| tester_motif(b, m)) {
        return false;
    }
    crate::config::valider_copie(c, b);
    toute_eeprom(b);
    ecrire_verifier(b);
    true
}

// ---------------------------------------------------------------------------------------------
// Compteurs de comptabilité

/// Un compteur long en RAM, son image dans `CONFIG`, et sa place dans l'EEPROM.
struct Compteur {
    /// Seulement si cet octet (relatif à A5) est non nul.
    condition: Option<u32>,
    valeur: u32,
    image: u32,
    /// Décalage en octets dans la copie de l'EEPROM.
    place: u16,
}

const PIECES: Compteur = Compteur { condition: None, valeur: 0x76, image: 0xffff_9dee, place: 0x60 };
const DEPARTS: Compteur = Compteur { condition: None, valeur: 0x7a, image: 0xffff_9df2, place: 0x64 };
const CONTINUES: Compteur =
    Compteur { condition: Some(0x25), valeur: 0x7e, image: 0xffff_9df6, place: 0x68 };
const PARTIES: Compteur = Compteur { condition: None, valeur: 0x7fd4, image: 0xffff_9dfa, place: 0x6c };

/// Ce qu'un compteur laisse dans D0 (et les drapeaux) de l'original.
enum Compte {
    /// Pas de comptage (condition fausse, ou comptabilité sur la carte de développement) : D0 intact.
    Rien,
    /// D0 = la nouvelle valeur (inchangée par rapport à son image : pas d'écriture).
    Valeur(u32),
    /// L'EEPROM a été écrite (0x2d8aa) : D0 = 0 ou -1 selon le succès.
    Ecrit(Succes),
}

/// Incrémente le compteur et, s'il a changé par rapport à son image, l'écrit dans l'EEPROM (0x2d8aa).
fn compter(b: &mut Cps2Bus, k: &Compteur) -> Compte {
    if let Some(cond) = k.condition {
        if rd8(b, a5(cond)) == 0 {
            return Compte::Rien;
        }
    }
    let valeur = rd32(b, a5(k.valeur)).wrapping_add(1);
    wr32(b, a5(k.valeur), valeur);
    if rd8(b, a5(ram::SANS_COMPTA)) != 0 {
        return Compte::Rien;
    }
    if valeur == rd32(b, a5(k.image)) {
        return Compte::Valeur(valeur);
    }
    wr32(b, a5(k.image), valeur);
    wr16(b, a5(ram::MOTS), 1);
    wr16(b, a5(ram::DEBUT), k.place);
    Compte::Ecrit(ecrire_verifier(b))
}

/// `compter` avec les registres de l'original.
fn compter_68k(c: &mut Cpu, b: &mut Cps2Bus, k: &Compteur) {
    match compter(b, k) {
        Compte::Rien => {}
        Compte::Valeur(v) => c.d[0] = v,
        Compte::Ecrit(ok) => {
            c.d[0] = if ok { 0 } else { u32::MAX };
            c.flags_logic(c.d[0], Size::Long);
            c.sr &= !SR_X;
        }
    }
}

macro_rules! natif_compteur {
    ($fn:ident, $k:expr) => {
        fn $fn(c: &mut Cpu, b: &mut Cps2Bus) {
            compter_68k(c, b, &$k);
            rts(c, b);
        }
    };
}

natif_compteur!(compter_pieces, PIECES);
natif_compteur!(compter_departs, DEPARTS);
natif_compteur!(compter_continues, CONTINUES);
natif_compteur!(compter_parties, PARTIES);

// ---------------------------------------------------------------------------------------------
// Comptabilité complète : EEPROM puis compteurs de la RAM supplémentaire

/// Compteur de comptabilité complet (0x2e448 pièces, 0x2e49a départs) : le compteur de l'EEPROM,
/// puis, si la RAM supplémentaire est utilisée (`SANS_COMPTA` ≠ 0, carte de développement), les trois
/// exemplaires du compteur dans cette RAM (`carte_dev`).
struct Complet {
    compteur: &'static Compteur,
    carte: fn(&mut Cps2Bus) -> u32,
}

const COMPLET_PIECES: Complet = Complet { compteur: &PIECES, carte: carte_dev::pieces };
const COMPLET_DEPARTS: Complet = Complet { compteur: &DEPARTS, carte: carte_dev::departs };

/// Une pièce (0x2e448) ou un départ (0x2e49a) comptés : l'EEPROM, puis la RAM de la carte de développement. Rend la
/// valeur de la carte (`None` sur une borne).
fn compta_complete(b: &mut Cps2Bus, k: &Complet) -> Option<u32> {
    compter(b, k.compteur);
    (rd8(b, a5(ram::SANS_COMPTA)) != 0).then(|| (k.carte)(b))
}

/// 0x2e448 : une pièce comptée (lecture des monnayeurs, VBLANK).
pub fn compter_piece(b: &mut Cps2Bus) {
    compta_complete(b, &COMPLET_PIECES);
}

/// 0x2e49a : un départ compté (bouton de départ du premier monnayeur).
pub fn compter_depart(b: &mut Cps2Bus) {
    compta_complete(b, &COMPLET_DEPARTS);
}

/// `compta_complete` avec les registres de l'original (D0 du compteur, drapeaux du dernier test).
fn compta_complete_68k(c: &mut Cpu, b: &mut Cps2Bus, k: &Complet) {
    compter_68k(c, b, k.compteur);
    let actif = rd8(b, a5(ram::SANS_COMPTA)) != 0;
    c.flags_logic(actif as u32, Size::Byte);
    if actif {
        let valeur = (k.carte)(b);
        c.flags_logic(valeur, Size::Long);
    }
    rts(c, b);
}

fn natif_compta_pieces(c: &mut Cpu, b: &mut Cps2Bus) {
    compta_complete_68k(c, b, &COMPLET_PIECES);
}

fn natif_compta_departs(c: &mut Cpu, b: &mut Cps2Bus) {
    compta_complete_68k(c, b, &COMPLET_DEPARTS);
}

// ---------------------------------------------------------------------------------------------
// VBLANK

/// 0x2dcf6 (VBLANK) : lève le bit 0 du mot 0x664000 de la RAM supplémentaire (si la comptabilité est active). Rend le
/// mot final (D0.w de l'original), s'il a été lu.
pub fn vblank(b: &mut Cps2Bus) -> Option<u16> {
    if rd8(b, a5(ram::SANS_COMPTA)) == 0 {
        return None;
    }
    let mot = rd16(b, 0x664000);
    if mot & 1 == 0 {
        wr16(b, 0x664000, mot | 1);
    }
    Some(mot | 1)
}

fn vblank_compta(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(mot) = vblank(b) {
        c.d[0] = c.d[0] & 0xffff_0000 | mot as u32;
    }
    rts(c, b);
}

macro_rules! natif {
    ($nom:ident, $entree:expr, $name:expr, $run:expr, $sorties:expr) => {
        pub const $nom: Native = Native {
            entry: $entree,
            name: $name,
            run: crate::Corps::Processeur($run),
            touches_sound: false,
            outputs: $sorties,
        };
    };
}

natif!(LIRE, 0x2d846, "eeprom_lire", natif_lire, 0);
natif!(ECRIRE, 0x2d90e, "eeprom_ecrire", natif_ecrire, regs::d(0) | regs::CCR);
natif!(ECRIRE_VERIFIER, 0x2d8aa, "eeprom_ecrire_verifier", natif_ecrire_verifier, regs::d(0) | regs::CCR);
natif!(LIRE_VERIFIER, 0x2d808, "eeprom_lire_verifier", natif_lire_verifier, regs::d(0) | regs::CCR);
natif!(COMPTER_PIECES, 0x2daca, "compta_pieces", compter_pieces, regs::d(0));
natif!(COMPTER_DEPARTS, 0x2daf4, "compta_departs", compter_departs, regs::d(0));
natif!(COMPTER_CONTINUES, 0x2db1e, "compta_continues", compter_continues, regs::d(0));
natif!(COMPTER_PARTIES, 0x2db4e, "compta_parties", compter_parties, regs::d(0));
natif!(VBLANK_COMPTA, 0x2dcf6, "compta_vblank", vblank_compta, regs::d(0));
natif!(COMPLET_PIECES_NATIF, 0x2e448, "compta_complete_pieces", natif_compta_pieces, regs::CCR);
natif!(COMPLET_DEPARTS_NATIF, 0x2e49a, "compta_complete_departs", natif_compta_departs, regs::CCR);

// ---------------------------------------------------------------------------------------------
// Parties gratuites et RAM de la carte de développement

/// Jeu gratuit (A5+0x25 non nul) : le compteur des parties gratuites (`CONTINUES`, 0x2db1e) ; sur la carte de
/// développement (`SANS_COMPTA` non nul : jamais sur une borne), le compteur correspondant de la RAM supplémentaire.
const JEU_GRATUIT: u32 = 0x25;

/// `tst.b (SANS_COMPTA,A5)` : rend vrai si la RAM supplémentaire est utilisée (drapeaux comme l'original).
fn carte_de_developpement(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    let v = rd8(b, a5(ram::SANS_COMPTA));
    c.flags_logic(v as u32, Size::Byte);
    v != 0
}

/// 0x2e4ec (départ d'un joueur) : en jeu gratuit, le compteur des parties gratuites ; sur la carte de développement,
/// celui de sa RAM.
pub fn compter_partie(b: &mut Cps2Bus) {
    let gratuit = rd8(b, a5(JEU_GRATUIT)) != 0;
    if gratuit {
        compter(b, &CONTINUES);
    }
    if rd8(b, a5(ram::SANS_COMPTA)) != 0 {
        if gratuit {
            carte_dev::parties_gratuites(b);
        } else {
            carte_dev::parties_payantes(b);
        }
    }
}

/// 0x2dc9a : vérification de la RAM de la carte de développement.
pub fn initialiser_compta(b: &mut Cps2Bus) {
    if rd8(b, a5(ram::SANS_COMPTA)) != 0 {
        carte_dev::demarrer(b);
    }
}

/// 0x2e630 : statistique du personnage choisi (carte de développement seulement).
pub fn stat_personnage_choisi(b: &mut Cps2Bus, personnage: u16) {
    if rd8(b, a5(ram::SANS_COMPTA)) != 0 {
        carte_dev::partie_du_personnage(b, personnage);
    }
}

/// 0x2e696 : statistique de la durée d'une partie, en images (carte de développement seulement).
pub fn stat_duree_de_partie(b: &mut Cps2Bus, images: u32) {
    if rd8(b, a5(ram::SANS_COMPTA)) != 0 {
        carte_dev::duree_de_partie(b, images);
    }
}

fn compta_parties(c: &mut Cpu, b: &mut Cps2Bus) {
    let gratuit = rd8(b, a5(JEU_GRATUIT)) != 0;
    if gratuit {
        compter_68k(c, b, &CONTINUES);
    }
    if carte_de_developpement(c, b) {
        let valeur = if gratuit { carte_dev::parties_gratuites(b) } else { carte_dev::parties_payantes(b) };
        c.flags_logic(valeur, Size::Long);
    }
    rts(c, b);
}

/// 0x2dc9a (démarrage) : vérification de la RAM supplémentaire, seulement sur la carte de développement.
fn compta_init(c: &mut Cpu, b: &mut Cps2Bus) {
    if carte_de_developpement(c, b) {
        carte_dev::demarrer(b);
    }
    rts(c, b);
}

/// 0x2e8e4 (reset avec les boutons de réinitialisation tenus) : remise à zéro de la RAM supplémentaire.
pub fn reinitialiser_carte(c: &mut Cpu, b: &mut Cps2Bus) {
    if carte_de_developpement(c, b) {
        carte_dev::reinitialiser(b);
    }
}

/// 0x2e630 (personnage choisi, D0) : statistique de la RAM de la carte de développement ; sur une borne, rien.
fn stat_personnage(c: &mut Cpu, b: &mut Cps2Bus) {
    if carte_de_developpement(c, b) {
        carte_dev::partie_du_personnage(b, c.d[0] as u16);
    }
    rts(c, b);
}

/// 0x2e696 (durée de partie, D0, en images) : idem.
fn stat_duree(c: &mut Cpu, b: &mut Cps2Bus) {
    if carte_de_developpement(c, b) {
        carte_dev::duree_de_partie(b, c.d[0]);
    }
    rts(c, b);
}

pub const STAT_PERSONNAGE: Native = Native {
    entry: 0x2e630,
    name: "compta_stat_personnage",
    run: crate::Corps::Processeur(stat_personnage),
    touches_sound: false,
    outputs: regs::CCR,
};

pub const STAT_DUREE: Native = Native {
    entry: 0x2e696,
    name: "compta_stat_duree",
    run: crate::Corps::Processeur(stat_duree),
    touches_sound: false,
    outputs: regs::CCR,
};

pub const COMPTA_PARTIES: Native = Native {
    entry: 0x2e4ec,
    name: "compta_parties",
    run: crate::Corps::Processeur(compta_parties),
    touches_sound: false,
    outputs: regs::CCR,
};

pub const COMPTA_INIT: Native = Native {
    entry: 0x2dc9a,
    name: "compta_init",
    run: crate::Corps::Processeur(compta_init),
    touches_sound: false,
    outputs: regs::CCR,
};
