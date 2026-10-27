//! Démarrage de la machine (K1 `reset` 0x350 et `boot_init` 0x492, K2 test de RAM 0x182a et 0x1912).
//!
//! À la mise sous tension, le 68000 (interruptions masquées, pile du superviseur en FF0E66) :
//!
//! 1. `reset` : registres d'E/S et CPS, Z80 maintenu au reset, RAM de travail effacée, couches et objets
//!    effacés ;
//! 2. test de RAM : palette de départ (8 longs en 0x91C400), GFX RAM 0x910000-0x913FFF remplie de cases
//!    (0x20, 0), puis tests *non destructifs* (chaque mot reçoit 0x8888, 0x4444, 0x2222, 0x1111 et
//!    retrouve sa valeur) de la GFX RAM 0x910000-0x91BFFF et de la RAM de travail ;
//! 3. relâche le Z80, attend sa poignée de main (0x77 en CFFF), teste les octets de la RAM partagée
//!    (0, puis 0x55, puis 0xFF sauf les trois derniers, qui gardent 0x55) ;
//! 4. `boot_init` : boîte aux lettres du Z80 (CFFD ← 0x88, CFFE et CFFF ← 0xFF), tas des piles des tâches,
//!    réglages (0x2d30a), interruptions autorisées (le VBLANK en attente est pris aussitôt), tâche de
//!    démarrage (0x11b16), ordonnanceur.
//!
//! Sur la machine émulée toute la RAM est bonne : la branche d'erreur du test (« RAM ERROR !! » affiché
//! en 0x19ae, puis boucle infinie) n'est jamais prise et n'est pas réécrite. Restent les effets.
//!
//! **Calendrier.** À ×16, le test dure cinq images : le Z80 est relâché à l'image 5, ligne 108, et
//! `boot_init` commence ligne 173. Ces deux instants comptent (le Z80 démarre en parallèle, il attend
//! 0xFF en CFFF pour entrer dans sa boucle principale) : la version native produit tous les effets au
//! reset, puis attend la même position du faisceau que la référence surcadencée (`RELACHE_Z80`,
//! `BOOT_INIT`), quelle que soit la fréquence du 68000. Le démarrage ne dépend pas des entrées (aucun
//! scénario n'en donne avant l'image 600) : ce calendrier est fixe (`examples/demarrage`). Une remise à
//! zéro logicielle (0x28cd0, après une exception du processeur, K4 hors périmètre) ne retrouverait pas
//! la durée du test. `validate` tolère le mot en cours de test des images 1 à 5 (`docs/noyau.md`).

use cps2::{Cps2Bus, LINES, VBLANK_LINE};
use m68k::Cpu;

use crate::mem::{rd16, rd32, rd8, set_low_word, wr16, wr32, wr8, A5};
use crate::call;
use crate::kernel::Point;
use crate::taches::table::{table, Table};

/// Position du faisceau : lignes écoulées depuis la mise sous tension (une image commence au vblank).
const fn faisceau(image: u64, ligne: u32) -> u64 {
    image * LINES as u64 + ((ligne + LINES - VBLANK_LINE) % LINES) as u64
}

fn position(b: &Cps2Bus) -> u64 {
    faisceau(b.frame, b.scanline)
}

/// Le code d'origine à ×16 relâche le Z80 ici (0x18cc)...
const RELACHE_Z80: u64 = faisceau(5, 108);
/// ... et entre dans `boot_init` (0x492) ici.
const BOOT_INIT: u64 = faisceau(5, 173);

/// Exécute le point `point` du démarrage ; rend le suivant, ou `None` quand il attend son heure (position du
/// faisceau, poignée de main du Z80) : il réessaiera à la ligne suivante, sans avoir rien changé. Après `boot_init`
/// (`move #0x2000,SR`), l'interruption en attente est prise avant le lancement.
pub fn executer(c: &mut Cpu, b: &mut Cps2Bus, point: Point) -> Option<Point> {
    match point {
        Point::Reset => Some(reset(c, b)),
        Point::RelacheZ80 => relacher_z80(b),
        Point::Poignee => poignee_de_main(c, b),
        Point::BootInit => boot_init(c, b),
        Point::Lancement => Some(lancement(c, b)),
        _ => unreachable!("point {point:?} : pas un point du démarrage"),
    }
}

/// Sous-routines d'origine et adresses de retour de leurs `jsr`.
mod ext {
    pub const EFFACER_OBJETS: (u32, u32) = (0xc58, 0x436);
    pub const EFFACER_PLAN_1: (u32, u32) = (0xca8, 0x43c);
    pub const EFFACER_PLAN_2: (u32, u32) = (0xcc8, 0x442);
    pub const EFFACER_PLAN_3: (u32, u32) = (0xce8, 0x448);
    pub const CONFIG: (u32, u32) = (0x2dc9a, 0x468);
    pub const REGLAGES: (u32, u32) = (0x2d30a, 0x500);
    pub const CREER_TACHE: (u32, u32) = (0x9b6, 0x530);
    pub const TACHE_DEMARRAGE: u32 = 0x11b16;
}

/// Registres d'E/S.
mod io {
    /// Sorties 0x400000-0x40000A (priorités, contrôle) : valeurs du reset.
    pub const SORTIES: [(u32, u16); 6] =
        [(0x400000, 0x7080), (0x400002, 0x807d), (0x400004, 0x2355), (0x400006, 0), (0x400008, 0x40), (0x40000a, 0x10)];
    /// Lignes de l'EEPROM et reset du Z80 (bit 3 : Z80 en marche).
    pub const COMMANDES: u32 = 0x804040;
    pub const Z80_AU_RESET: u16 = 0x0f00;
    pub const Z80_EN_MARCHE: u16 = 0x0f08;
    pub const CPS_A: u32 = 0x804100;
    pub const CPS_B_OBJETS: u32 = 0x804170;
    pub const VIDEO: u32 = 0x804122;
    pub const VIDEO_B: u32 = 0x804166;
    /// Port lu au reset (0xe021 sur cette carte) : son bit 14 nul (jamais vu) fait lire la configuration
    /// (0x2dc9a) dès le reset, et éventuellement réinitialiser la RAM supplémentaire (0x2e8e4, `eeprom::reinitialiser_carte`).
    pub const CARTE: u32 = 0x804030;
    pub const TEST: u32 = 0x804020;
    pub const BOUTONS: u32 = 0x804000;
    pub const INCONNU: u32 = 0x8040a0;
}

/// RAM partagée avec le Z80, côté 68000 (octets impairs).
mod partagee {
    pub const DEBUT: u32 = 0x618001;
    pub const FIN: u32 = 0x61a000;
    /// CFFD, CFFE, CFFF : drapeau de la boîte aux lettres, stéréo, poignée de main.
    pub const BOITE: u32 = 0x619ffb;
    pub const STEREO: u32 = 0x619ffd;
    pub const POIGNEE: u32 = 0x619fff;
    /// Indice de CFFF dans `Cps2Bus::qram`.
    pub const POIGNEE_Z80: usize = 0xfff;
    pub const PRET_Z80: u8 = 0x77;
}

/// Constantes du programme lues au démarrage.
mod rom {
    /// Mot de région (copié en `A5+0x70`) et octet « carte de développement ».
    pub const REGION: u32 = 0xc2;
    pub const DEVELOPPEMENT: u32 = 0xc4;
    /// Palette de départ (8 longs, copiés en 0x91C400).
    pub const PALETTE: u32 = 0x19f0;
    /// Piles des 16 tâches (16 longs, empilés sous FF01A6 : le tas des piles).
    pub const PILES: u32 = 0x534;
    /// Texte « RAM ERROR !! » du dernier bloc testé (laissé dans A2).
    pub const TEXTE_ERREUR: u32 = 0x19e0;
}

const GFX_TEST: u32 = 0x910000;
const GFX_REMPLIE: u32 = 0x914000;
const PALETTE_DEPART: u32 = 0x91c400;
const RAM_DEBUT: u32 = 0xff0000;
/// Fond du tas des piles des tâches de l'original (pointeur en FF005A, piles empilées sous FF01A6).
const PILES_FIN: u32 = A5 - 0x7e5a;
/// Indicateur posé quand le bouton 1 du joueur 1 est pressé au démarrage (cartes de développement).
const BOUTON_AU_DEMARRAGE: u32 = A5 - 0x55e2;

/// Variables de la RAM de travail (décalages depuis A5).
mod var {
    pub const VIDEO: u32 = 0x12;
    pub const VIDEO_B: u32 = 0x14;
    /// Image du port 0x804040 que les écritures de l'EEPROM et des compteurs recomposent : octet haut, bits des
    /// compteurs de pièces, bits forcés (dont le bit 3, Z80 en marche ; `docs/feuilles.md`, `docs/video.md`).
    pub const PORT_COMMANDES: [(u32, u8); 3] = [(0x16, 0xf), (0x17, 8), (0x18, 8)];
    pub const REGION: u32 = 0x70;
    /// Bit 14 de `io::CARTE` nul (sens inconnu ; aussi testé par la tâche de partie, `docs/boucle.md` §1).
    pub const CARTE_SPECIALE: u32 = 0x82;
    pub const SORTIE: u32 = 0x5bc;
    pub const DSW: u32 = 0x7fd0;
}

fn a5(off: u32) -> u32 {
    A5.wrapping_add(off)
}

/// `jsr` d'une routine du démarrage (l'adresse de retour d'origine n'est plus empilée : la pile du démarrage n'est
/// relue par personne).
fn jsr(c: &mut Cpu, b: &mut Cps2Bus, (routine, _retour): (u32, u32)) {
    call(c, b, routine);
}

/// 0x350 : reset, puis les effets du test de RAM (0x182a) jusqu'au relâchement du Z80.
fn reset(c: &mut Cpu, b: &mut Cps2Bus) -> Point {
    wr16(b, io::INCONNU, 0);
    for (port, v) in io::SORTIES[..2].iter().copied() {
        wr16(b, port, v);
    }
    wr16(b, io::CPS_A, 0x9100);
    for (port, v) in io::SORTIES[2..].iter().copied() {
        wr16(b, port, v);
    }
    wr16(b, io::COMMANDES, io::Z80_AU_RESET);
    c.a[5] = A5;
    b.ram.fill(0);
    *table(b) = Table::default();
    let region = rd16(b, rom::REGION);
    wr16(b, a5(var::REGION), region);
    wr16(b, io::VIDEO, 0xe);
    wr16(b, a5(var::VIDEO), 0xe);
    wr16(b, io::VIDEO_B, 0x12ce);
    wr16(b, a5(var::VIDEO_B), 0x12ce);
    wr16(b, io::CPS_B_OBJETS, 0xe);
    wr8(b, a5(var::SORTIE), 1);
    // bases des objets et des plans 1 et 2, défilement des trois plans à zéro
    for (k, base) in [0x9100u16, 0x9140, 0x9180].into_iter().enumerate() {
        wr16(b, io::CPS_A + 2 + 2 * k as u32, base);
    }
    for reg in [0x0c, 0x10, 0x14] {
        wr32(b, io::CPS_A + reg, 0);
    }
    for routine in [ext::EFFACER_OBJETS, ext::EFFACER_PLAN_1, ext::EFFACER_PLAN_2, ext::EFFACER_PLAN_3] {
        jsr(c, b, routine);
    }
    wr8(b, a5(var::CARTE_SPECIALE), 0);
    if rd16(b, io::CARTE) & 0x4000 == 0 {
        wr8(b, a5(var::CARTE_SPECIALE), 1);
        jsr(c, b, ext::CONFIG);
        // combinaison de boutons 0x3304 (actifs bas) de IN2 pressée au démarrage
        if !rd16(b, io::TEST) & 0x3304 == 0x3304 {
            crate::eeprom::reinitialiser_carte(c, b);
        }
    }
    // 0x182a : palette de départ, puis cases vides dans le premier quart de la GFX RAM testée
    for i in 0..8 {
        let v = rd32(b, rom::PALETTE + 4 * i);
        wr32(b, PALETTE_DEPART + 4 * i, v);
    }
    wr16(b, io::CPS_B_OBJETS, 0xf);
    wr16(b, io::CPS_A + 0x0a, 0x91c0);
    for a in (GFX_TEST..GFX_REMPLIE).step_by(4) {
        wr16(b, a, 0x20);
        wr16(b, a + 2, 0);
    }
    Point::RelacheZ80
}

/// 0x18cc : à l'heure de l'original, relâche le Z80 et efface le drapeau de sa boîte aux lettres.
fn relacher_z80(b: &mut Cps2Bus) -> Option<Point> {
    if position(b) < RELACHE_Z80 {
        return None;
    }
    wr16(b, io::COMMANDES, io::Z80_EN_MARCHE);
    wr8(b, a5(var::PORT_COMMANDES[2].0), var::PORT_COMMANDES[2].1);
    wr8(b, partagee::BOITE, 0);
    Some(Point::Poignee)
}

/// 0x18e2 : attend la poignée de main du Z80, puis laisse la RAM partagée comme son test.
fn poignee_de_main(c: &mut Cpu, b: &mut Cps2Bus) -> Option<Point> {
    if b.qram[partagee::POIGNEE_Z80] != partagee::PRET_Z80 {
        return None;
    }
    for a in (partagee::DEBUT..partagee::FIN).step_by(2) {
        wr8(b, a, if a < partagee::BOITE { 0xff } else { 0x55 });
    }
    registres_du_test(c, b);
    Some(Point::BootInit)
}

/// Registres que le test de RAM laisse à `boot_init` (et, par lui, aux réglages 0x2d30a) : les longs de la
/// palette chargés par `movem` en 0x1834, puis ce que les boucles de test y ont mis.
fn registres_du_test(c: &mut Cpu, b: &mut Cps2Bus) {
    let palette: [u32; 8] = std::array::from_fn(|i| rd32(b, rom::PALETTE + 4 * i as u32));
    c.d[..7].copy_from_slice(&palette[..7]);
    set_low_word(&mut c.d[0], 0x5555); // dernier motif du test de la RAM partagée
    c.d[1] = 0;
    let dernier_mot = rd16(b, RAM_DEBUT + 0xfffc); // valeur du dernier mot de RAM testé
    set_low_word(&mut c.d[2], dernier_mot);
    c.d[3] = 0xff;
    c.d[7] = 0xffff;
    c.a[0] = partagee::FIN - 6;
    c.a[1] = partagee::FIN - 6;
    c.a[2] = rom::TEXTE_ERREUR;
    c.a[3] = partagee::DEBUT - 1;
    c.a[4] = 0;
    c.a[6] = 0x492; // `boot_init`, adresse de retour du test (`lea`)
}

/// 0x492 : à l'heure de l'original, boîte aux lettres du Z80, tas des piles, réglages, puis interruptions.
fn boot_init(c: &mut Cpu, b: &mut Cps2Bus) -> Option<Point> {
    if position(b) < BOOT_INIT {
        return None;
    }
    wr8(b, partagee::BOITE, 0x88);
    wr8(b, partagee::STEREO, 0xff);
    wr8(b, partagee::POIGNEE, 0xff);
    c.a[5] = A5;
    wr16(b, io::COMMANDES, io::Z80_EN_MARCHE);
    for (off, v) in var::PORT_COMMANDES {
        wr8(b, a5(off), v);
    }
    wr16(b, a5(var::DSW), 0xffff);
    // 16 piles, empilées (`move.l (A0)+,-(A1)` sous FF01A6) : la dernière de la table au sommet. Le tas est celui de
    // la table des tâches Rust ; A1 garde le sommet de celui de l'original.
    let piles = (0..16).map(|i| rd32(b, rom::PILES + 4 * i)).collect();
    table(b).piles = piles;
    set_low_word(&mut c.d[0], 0xffff);
    c.a[0] = rom::PILES + 0x40;
    c.a[1] = PILES_FIN - 0x40;
    jsr(c, b, ext::REGLAGES);
    c.set_sr(0x2000);
    Some(Point::Lancement)
}

/// 0x50a : crée la tâche de démarrage (emplacement 0) et passe la main à l'ordonnanceur.
fn lancement(c: &mut Cpu, b: &mut Cps2Bus) -> Point {
    if rd8(b, rom::DEVELOPPEMENT) != 0 {
        let presse = rd8(b, io::BOUTONS) & 0x10 == 0;
        wr8(b, BOUTON_AU_DEMARRAGE, if presse { 0xff } else { 0 });
    }
    c.a[0] = ext::TACHE_DEMARRAGE;
    set_low_word(&mut c.d[0], 0);
    jsr(c, b, ext::CREER_TACHE);
    Point::Debut
}
