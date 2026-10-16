//! Son côté 68000 : file de commandes envoyées au Z80 (format : `docs/son-68000.md`).
//!
//! Le jeu enfile ses commandes dans une file circulaire en RAM de travail (64 entrées de 16 octets) ;
//! chaque image, le VBLANK en transmet **une** au Z80 par la RAM partagée 0x618000. Les quelque 430
//! « wrappers » `sfx_*` (0xe0be2-0xede08) ne diffèrent que par leurs paramètres : ils sont décrits
//! par la table `sound_table::WRAPPERS` et exécutés par une seule routine (`run_wrapper`).
//!
//! Un wrapper d'origine sauvegarde tous les registres sur une pile privée (pointeur en `PILE_PRIVEE`),
//! puis compose trois longs (D1 = commande, D2 et D3 = paramètres) et appelle l'enfileur. Tous les
//! registres sont rendus intacts ; seule la mémoire change : la file, et les registres sauvegardés
//! restent écrits sous le pointeur de pile privée (copie morte, reproduite à l'identique).

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

use cps2::Cps2Bus;
use m68k::decode::{Ea, Size};
use m68k::{Cpu, SR_C};

use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8};
use crate::{regs, Native};

#[path = "sound_table.rs"]
mod table;

/// Décalages par rapport à A5 (mots/octets signés étendus : `A5 + n` modulo 2^32).
mod ram {
    /// Début de la file (64 entrées de 16 octets).
    pub const FILE: u32 = 0xffff_a55c;
    /// Tête de lecture (prochaine entrée à transmettre au Z80), mot.
    pub const TETE_LECTURE: u32 = 0xffff_a95c;
    /// Tête d'écriture (prochaine case libre), mot.
    pub const TETE_ECRITURE: u32 = 0xffff_a95e;
    /// Alternance 0 / -1 des wrappers à deux variantes (voix A / voix B), mot.
    pub const ALTERNANCE: u32 = 0xffff_a960;
    /// Pointeur de la pile privée des wrappers (long).
    pub const PILE_PRIVEE: u32 = 0xffff_aa1a;
    /// Octet non nul : les wrappers « gardés » n'émettent rien.
    pub const MUET_GARDE: u32 = 0xffff_aa1e;
    /// Sons autorisés si ce mot est non nul...
    pub const SON_MOT: u32 = 0x7fcc;
    /// ... ou si l'un de ces octets l'est (partie en cours / mode attract avec son).
    pub const SON_PARTIE: u32 = 0x30;
    pub const SON_ATTRACT: u32 = 0x27;
    /// Dernière commande « gardée » émise (mémo).
    pub const MEMO_COMMANDE: u32 = 0x7fc4;
    /// Compte à rebours décrémenté à chaque VBLANK (délai posé par certaines commandes).
    pub const DELAI: u32 = 0x7fc2;
    /// Octet non nul : verrou de la sortie son (copié en 0x619ffd).
    pub const SILENCE: u32 = 0x28;
    /// Position X de la caméra 1 / 2 et choix de la caméra (octet).
    pub const CAMERA_1_X: u32 = 0x5a6;
    pub const CAMERA_2_X: u32 = 0x5ae;
    pub const CAMERA_CHOIX: u32 = 0x36;
}

/// Taille de la file : 64 entrées de 16 octets.
const MASQUE_TETE: u16 = 0x3f0;
const ENTREE: u16 = 0x10;
/// Table de panoramique (un octet par tranche de 8 pixels d'écran), en ROM.
const TABLE_PAN: u32 = 0xee04e;

/// RAM partagée avec le Z80 : octets aux adresses impaires.
mod z80 {
    pub const BASE: u32 = 0x618000;
    /// Octets de la commande, dans l'ordre d'envoi : (octet de l'entrée de file, adresse).
    /// L'octet 4 de l'entrée (poids fort de D2) n'est pas transmis.
    pub const CHAMPS: [(usize, u32); 11] = [
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
    pub const VERROU: u32 = BASE + 0x1ffd;
}

/// Affichage des verrous (menu de test) : routine d'origine appelée en queue du VBLANK.
const AFFICHAGE_VERROUS: u32 = 0xedf80;

fn a5(c: &Cpu, off: u32) -> u32 {
    c.a[5].wrapping_add(off)
}

// ---------------------------------------------------------------------------------------------
// La file de commandes

/// Une commande : trois longs, tels que le jeu les compose (D1, D2, D3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Commande {
    pub d1: u32,
    pub d2: u32,
    pub d3: u32,
}

/// Issue d'un enfilage, avec le contenu des registres que l'original laisse derrière lui.
struct Enfilage {
    /// Mot bas de D0 au retour (nouvelle tête d'écriture, ou tête calculée si la file est pleine).
    d0: u16,
    /// Case écrite (A1 = fin de l'écriture), si la commande a été enfilée.
    ecrit: Option<u32>,
}

/// La commande peut-elle être émise ? (son autorisé : 0xe0836)
fn son_autorise(c: &Cpu, b: &mut Cps2Bus) -> bool {
    rd16(b, a5(c, ram::SON_MOT)) != 0
        || rd8(b, a5(c, ram::SON_PARTIE)) != 0
        || rd8(b, a5(c, ram::SON_ATTRACT)) != 0
}

/// La file est pleine quand l'écriture rattraperait la lecture (une case reste toujours libre).
fn file_pleine(c: &Cpu, b: &mut Cps2Bus) -> bool {
    let lecture = rd16(b, a5(c, ram::TETE_LECTURE));
    let ecriture = rd16(b, a5(c, ram::TETE_ECRITURE));
    lecture.wrapping_sub(ENTREE) & MASQUE_TETE == ecriture
}

/// Enfile une commande (0xe0848, sans le test d'autorisation).
fn enfiler(c: &Cpu, b: &mut Cps2Bus, cmd: Commande) -> Enfilage {
    let lecture = rd16(b, a5(c, ram::TETE_LECTURE));
    let ecriture = rd16(b, a5(c, ram::TETE_ECRITURE));
    if file_pleine(c, b) {
        return Enfilage { d0: lecture.wrapping_sub(ENTREE) & MASQUE_TETE, ecrit: None };
    }
    let case = a5(c, ram::FILE).wrapping_add(ecriture as i16 as u32);
    wr32(b, case, cmd.d1);
    wr32(b, case + 4, cmd.d2);
    wr32(b, case + 8, cmd.d3);
    let suivante = ecriture.wrapping_add(ENTREE) & MASQUE_TETE;
    wr16(b, a5(c, ram::TETE_ECRITURE), suivante);
    Enfilage { d0: suivante, ecrit: Some(case + 8) }
}

/// Reproduit D0, A1 et les drapeaux laissés par 0xe0848.
fn conclure_enfilage(c: &mut Cpu, e: &Enfilage) {
    c.d[0] = c.d[0] & 0xffff_0000 | e.d0 as u32;
    match e.ecrit {
        Some(fin) => {
            c.a[1] = fin;
            c.flags_logic(e.d0 as u32, Size::Word);
        }
        None => c.sr = c.sr & 0xff00 | SR_C, // move #1,CCR : seule la retenue est levée
    }
}

fn commande_registres(c: &Cpu) -> Commande {
    Commande { d1: c.d[1], d2: c.d[2], d3: c.d[3] }
}

/// 0xe0848 : enfile D1, D2, D3 (C levé si la file est pleine).
fn enfiler_direct(c: &mut Cpu, b: &mut Cps2Bus) {
    let e = enfiler(c, b, commande_registres(c));
    conclure_enfilage(c, &e);
    rts(c, b);
}

/// 0xe0836 : enfile D1, D2, D3 si le son est autorisé.
fn enfiler_si_son(c: &mut Cpu, b: &mut Cps2Bus) {
    if son_autorise(c, b) {
        enfiler_direct(c, b);
    } else {
        c.sr = c.sr & 0xff00 | SR_C;
        rts(c, b);
    }
}

impl Chemin {
    /// Décisions prises par 0xe0836 (si `avec_test`) puis 0xe0848.
    fn enfilage(&mut self, c: &Cpu, b: &mut Cps2Bus, avec_test: bool) {
        if avec_test {
            // bne (mot) ; bne (partie) ; beq (attract nul -> abandon)
            let mot = rd16(b, a5(c, ram::SON_MOT)) != 0;
            self.pris(mot);
            if !mot {
                let partie = rd8(b, a5(c, ram::SON_PARTIE)) != 0;
                self.pris(partie);
                if !partie {
                    let abandon = rd8(b, a5(c, ram::SON_ATTRACT)) == 0;
                    self.pris(abandon);
                    if abandon {
                        return;
                    }
                }
            }
        }
        let pleine = file_pleine(c, b);
        self.pris(pleine);
    }

    /// Durée du code d'origine depuis `entree` en suivant ces décisions.
    fn duree(&self, b: &mut Cps2Bus, entree: u32) -> u64 {
        let mut m = Marche { decisions: self.bits, restantes: self.n };
        let v = marche(b, entree, &mut m);
        assert_eq!(m.restantes, 0, "{entree:06x} : décisions non consommées");
        v
    }
}

fn enfiler_cout(c: &Cpu, b: &mut Cps2Bus, entree: u32, avec_test: bool) -> u64 {
    let mut p = Chemin::default();
    p.enfilage(c, b, avec_test);
    p.duree(b, entree)
}

pub const ENFILER_SI_SON: Native = Native {
    entry: 0xe0836,
    name: "son_enfiler",
    cost: |c, b| enfiler_cout(c, b, 0xe0836, true),
    run: enfiler_si_son,
    touches_sound: false,
    outputs: regs::d(0) | regs::a(1) | regs::CCR,
    exact_cost: true,
};

pub const ENFILER_DIRECT: Native = Native {
    entry: 0xe0848,
    name: "son_enfiler_direct",
    cost: |c, b| enfiler_cout(c, b, 0xe0848, false),
    run: enfiler_direct,
    touches_sound: false,
    outputs: regs::d(0) | regs::a(1) | regs::CCR,
    exact_cost: true,
};

// ---------------------------------------------------------------------------------------------
// Les wrappers

/// Lecture « espace programme » : les tables de commandes sont rangées dans le code et lues par
/// des accès relatifs au PC, qui voient les opcodes déchiffrés.
fn lecture_programme(b: &mut Cps2Bus, adresse: u32) -> u16 {
    m68k::Bus::read_prog16(b, adresse & 0xff_ffff)
}

/// Origine de l'indice de commande dans une table de mots en ROM.
#[derive(Clone, Copy, Debug)]
pub enum Cmd {
    /// Commande constante.
    Fixe(u16),
    /// Deux commandes en alternance (adresse de la table) : 0xedee4 retourne 0 puis 2 à tour de rôle.
    Alternee(u32),
    /// Indice = 2 × octet 2 de l'objet + 4 × (octet 3 & 0xe) (0xedef2).
    ParObjet(u32),
    /// Indice = 2 × octet 2 de l'objet.
    ParOctet2(u32),
}

// Options d'un wrapper.
/// Panoramique calculé depuis la position X de l'objet A0, ajouté à l'octet 1 de D2.
pub const PAN: u8 = 1;
/// Ne rien émettre si `MUET_GARDE` est non nul.
pub const GARDE: u8 = 2;
/// Recopie la commande en `MEMO_COMMANDE`.
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

fn wrapper_en(entree: u32) -> &'static Wrapper {
    let i = table::WRAPPERS.binary_search_by_key(&entree, |w| w.entry).expect("wrapper son inconnu");
    &table::WRAPPERS[i]
}

/// Panoramique (0-0x2f -> octet de la table de ROM) d'un objet : position écran de l'objet par
/// rapport à la caméra, bornée à 0..0x178, par tranches de 8 pixels. Trois décisions de branchement
/// sont enregistrées dans `chemin`.
fn panoramique(c: &Cpu, b: &mut Cps2Bus, chemin: &mut Chemin) -> u8 {
    let x = rd16(b, c.a[0] + 8);
    let camera_2 = rd8(b, a5(c, ram::CAMERA_CHOIX)) != 0;
    chemin.pris(!camera_2);
    let camera = rd16(b, a5(c, if camera_2 { ram::CAMERA_2_X } else { ram::CAMERA_1_X }));
    let ecart = x.wrapping_sub(camera);
    chemin.pris(ecart as i16 >= 0);
    let ecart = if (ecart as i16) < 0 { 0 } else { ecart };
    chemin.pris(ecart < 0x180);
    let ecart = if ecart < 0x180 { ecart } else { 0x178 };
    rd8(b, TABLE_PAN + ((ecart & 0x1ff) >> 3) as u32)
}

/// Suite des décisions des branchements conditionnels rencontrés, dans l'ordre d'exécution : sert à
/// calculer la durée exacte du chemin dans le code d'origine.
#[derive(Default)]
struct Chemin {
    bits: u64,
    n: u32,
}

impl Chemin {
    fn pris(&mut self, pris: bool) {
        self.bits |= (pris as u64) << self.n;
        self.n += 1;
    }
}

/// Résultat de l'évaluation d'un wrapper (sans effet sur la mémoire).
struct Plan {
    /// Commande à enfiler, ou `None` si le wrapper est gardé (rien n'est émis).
    commande: Option<Commande>,
    /// Le son est-il autorisé (sinon la commande est abandonnée par l'enfileur) ?
    autorise: bool,
    chemin: Chemin,
}

fn planifier(w: &Wrapper, c: &Cpu, b: &mut Cps2Bus) -> Plan {
    let mut chemin = Chemin::default();
    // indice de la table de commandes, calculé par les sous-routines 0xedee4 / 0xedef2
    let indice = match w.cmd {
        Cmd::Fixe(_) => 0,
        Cmd::Alternee(_) => {
            let voix_b = rd16(b, a5(c, ram::ALTERNANCE)) != 0;
            chemin.pris(!voix_b); // beq : voix A
            if voix_b { 2 } else { 0 }
        }
        Cmd::ParObjet(_) => {
            let octet3 = rd8(b, c.a[0] + 3) as u16;
            let octet2 = rd8(b, c.a[0] + 2) as u16;
            octet2 * 2 + ((octet3 & 0xe) << 2)
        }
        Cmd::ParOctet2(_) => rd8(b, c.a[0] + 2) as u16 * 2,
    };
    let d1 = match w.cmd {
        Cmd::Fixe(v) => v,
        Cmd::Alternee(t) | Cmd::ParObjet(t) | Cmd::ParOctet2(t) => lecture_programme(b, t + indice as i16 as u32),
    };
    if w.opts & GARDE != 0 {
        let muet = rd8(b, a5(c, ram::MUET_GARDE)) != 0;
        chemin.pris(muet);
        if muet {
            return Plan { commande: None, autorise: false, chemin };
        }
    }
    let mut d2 = w.d2;
    if w.opts & PAN != 0 {
        d2 |= (panoramique(c, b, &mut chemin) as u32) << 8;
    }
    let direct = w.opts & DIRECT != 0;
    chemin.enfilage(c, b, !direct);
    let autorise = direct || son_autorise(c, b);
    Plan { commande: Some(Commande { d1: d1 as u32, d2, d3: w.d3 }), autorise, chemin }
}

/// Sauvegarde fantôme : l'original empile D0-D7, A0-A4 sur la pile privée (copie morte restée en RAM).
fn sauvegarde_fantome(c: &Cpu, b: &mut Cps2Bus) {
    let sommet = rd32(b, a5(c, ram::PILE_PRIVEE));
    let registres = c.a[..5].iter().rev().chain(c.d.iter().rev());
    for (k, &r) in registres.enumerate() {
        wr32(b, sommet.wrapping_sub(4 * (k as u32 + 1)), r);
    }
}

/// Exécute le wrapper dont l'entrée est le PC courant.
fn run_wrapper(c: &mut Cpu, b: &mut Cps2Bus) {
    let w = wrapper_en(c.pc);
    let plan = planifier(w, c, b);
    sauvegarde_fantome(c, b);
    if let Cmd::Alternee(_) = w.cmd {
        let v = rd16(b, a5(c, ram::ALTERNANCE));
        wr16(b, a5(c, ram::ALTERNANCE), !v);
    }
    if let Some(cmd) = plan.commande {
        if w.opts & MEMO != 0 {
            wr16(b, a5(c, ram::MEMO_COMMANDE), cmd.d1 as u16);
        }
        if plan.autorise {
            enfiler(c, b, cmd);
        }
    }
    // drapeaux du dernier move.l (pointeur de pile privée) ; registres rendus intacts
    let sommet = rd32(b, a5(c, ram::PILE_PRIVEE));
    c.flags_logic(sommet, Size::Long);
    rts(c, b);
}

thread_local! {
    static COUTS: RefCell<HashMap<(u32, u64, u32), u64>> = RefCell::new(HashMap::new());
}

/// Durée du wrapper d'origine sur le chemin choisi.
fn cout_wrapper(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let w = wrapper_en(c.pc);
    let plan = planifier(w, c, b);
    let cle = (w.entry, plan.chemin.bits, plan.chemin.n);
    if let Some(v) = COUTS.with(|m| m.borrow().get(&cle).copied()) {
        return v;
    }
    let v = plan.chemin.duree(b, w.entry);
    COUTS.with(|m| m.borrow_mut().insert(cle, v));
    v
}

/// Décisions restant à consommer lors du parcours du code d'origine.
struct Marche {
    decisions: u64,
    restantes: u32,
}

/// Parcourt le code d'origine depuis `pc` jusqu'au RTS (ou au JMP final, compté) en additionnant les
/// durées, les branchements conditionnels suivant les décisions de `chemin` (dans l'ordre).
fn marche(b: &mut Cps2Bus, mut pc: u32, chemin: &mut Marche) -> u64 {
    use m68k::decode::{Op, ShiftCount};
    let mut cycles = 0u64;
    for _ in 0..10_000 {
        let ins = m68k::decode(b, pc);
        cycles += m68k::timing::base_cycles(&ins) as u64;
        let suite = pc.wrapping_add(ins.len);
        match ins.op {
            Op::Rts => return cycles,
            Op::Bra(t) => {
                pc = t;
                continue;
            }
            Op::Bcc(_, t) => {
                assert!(chemin.restantes > 0, "marche : branchement sans décision en {pc:06x}");
                chemin.restantes -= 1;
                let pris = chemin.decisions & 1 != 0;
                chemin.decisions >>= 1;
                if pris {
                    cycles += 2;
                    pc = t;
                    continue;
                } else if ins.opcode & 0xff == 0 {
                    cycles += 4;
                }
            }
            Op::Bsr(t) | Op::Jsr(Ea::AbsL(t) | Ea::AbsW(t) | Ea::PcDisp(t)) => cycles += marche(b, t, chemin),
            Op::Jsr(_) => panic!("marche : JSR non statique en {pc:06x}"),
            Op::Jmp(_) => return cycles,
            Op::Movem { size, mask, .. } => {
                cycles += mask.count_ones() as u64 * if size == Size::Long { 8 } else { 4 }
            }
            Op::Shift { count: ShiftCount::Imm(n), .. } => cycles += 2 * n as u64,
            Op::Shift { .. } | Op::Dbcc(..) | Op::Mulu(..) | Op::Muls(..) | Op::Divu(..) | Op::Divs(..) => {
                panic!("marche : durée dépendant des données en {pc:06x}")
            }
            _ => {}
        }
        pc = suite;
    }
    panic!("marche : pas de RTS depuis {pc:06x}");
}

fn wrapper_natif(w: &Wrapper) -> Native {
    Native {
        entry: w.entry,
        name: "son_wrapper",
        cost: cout_wrapper,
        run: run_wrapper,
        touches_sound: false,
        outputs: 0, // tous les registres sont rendus intacts : rien à comparer
        exact_cost: true,
    }
}

/// Les routines natives de chaque wrapper décrit (construites une fois, puis conservées).
pub fn wrappers_natifs() -> &'static [Native] {
    static LISTE: OnceLock<Vec<Native>> = OnceLock::new();
    LISTE.get_or_init(|| table::WRAPPERS.iter().map(wrapper_natif).collect())
}

// ---------------------------------------------------------------------------------------------
// Transmission au Z80 (VBLANK)

/// Transmet l'entrée de file `entree` aux octets de la RAM partagée.
fn transmettre(b: &mut Cps2Bus, entree: u32) {
    for (octet, adresse) in z80::CHAMPS {
        let v = rd8(b, entree + octet as u32);
        wr8(b, adresse, v);
    }
}

/// Décisions de 0x68c : drapeau de silence, délai nul, file vide, Z80 pas prêt.
fn chemin_envoi(c: &Cpu, b: &mut Cps2Bus) -> Chemin {
    let mut p = Chemin::default();
    p.pris(rd8(b, a5(c, ram::SILENCE)) != 0);
    p.pris(rd16(b, a5(c, ram::DELAI)) == 0);
    let vide = rd16(b, a5(c, ram::TETE_LECTURE)) == rd16(b, a5(c, ram::TETE_ECRITURE));
    p.pris(vide);
    if !vide {
        p.pris(rd8(b, z80::ACQUIT) != 0xff);
    }
    p
}

/// 0x68c (VBLANK) : une commande de la file part vers le Z80 si celui-ci a acquitté la précédente.
fn envoyer_au_z80(c: &mut Cpu, b: &mut Cps2Bus) {
    let silence = rd8(b, a5(c, ram::SILENCE)) != 0;
    wr8(b, z80::VERROU, if silence { 0xff } else { 0 });
    let delai = rd16(b, a5(c, ram::DELAI));
    if delai != 0 {
        wr16(b, a5(c, ram::DELAI), delai - 1);
    }
    let mut lecture = rd16(b, a5(c, ram::TETE_LECTURE));
    c.d[0] = if silence { 0xffff_0000 } else { 0 } | lecture as u32;
    let ecriture = rd16(b, a5(c, ram::TETE_ECRITURE));
    if lecture != ecriture && rd8(b, z80::ACQUIT) == 0xff {
        let entree = a5(c, ram::FILE).wrapping_add(lecture as i16 as u32);
        transmettre(b, entree);
        c.a[4] = entree + 12;
        wr8(b, z80::ACQUIT, 0);
        lecture = lecture.wrapping_add(ENTREE) & MASQUE_TETE;
        wr16(b, a5(c, ram::TETE_LECTURE), lecture);
        c.d[0] = c.d[0] & 0xffff_0000 | lecture as u32;
    }
    // l'original se termine par un JMP vers l'affichage des verrous
    crate::call(c, b, AFFICHAGE_VERROUS);
    rts(c, b);
}

/// Durée jusqu'au JMP final inclus (l'affichage des verrous est compté par son propre appel).
fn envoyer_cout(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    chemin_envoi(c, b).duree(b, 0x68c)
}

pub const ENVOYER_AU_Z80: Native = Native {
    entry: 0x68c,
    name: "son_envoyer_z80",
    cost: envoyer_cout,
    run: envoyer_au_z80,
    touches_sound: true,
    outputs: 0, // le VBLANK a sauvegardé tous les registres
    exact_cost: false,
};

/// Routines natives du son, hors wrappers.
pub const NATIVES: &[Native] = &[ENFILER_SI_SON, ENFILER_DIRECT, ENVOYER_AU_Z80];
