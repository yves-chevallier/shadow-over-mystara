//! Tâche d'écran fixe (`0x1835e`, F4) : le tableau des meilleurs scores de l'attract (sous-modes 0 et 6 de la
//! machine à modes, `modes::attract::fixed_screen`).
//!
//! À chaque image : l'étape de l'écran (bloc d'état `FF0F50`), puis les objets EFFCT et SET et la liste de
//! sprites, puis une image de cession (`jsr 0xb64`, reprise à l'étape `Reprise`, `0x18384`). Étapes (`+0`, pas de 2) :
//!
//! * 0 (`0x1838a`) : décor (palettes, deux blocs de tuiles par couche, palettes du titre), six lignes du
//!   tableau (rangs 5 à 10, `0x18b8e`), objet SET du cadre, luminosités ;
//! * 2 (`0x18530`) : sous-étapes (`+1`) — 0 attend 0x40 images puis dessine les trois premiers rangs
//!   (`0x18a3e`), leurs quatre objets EFFCT, le titre et le meilleur score ; 2 fait monter la couche 2 de
//!   0xe0 lignes ; 4 attend 0x90 images, lève l'indicateur « écran fini » (`+3`) et termine la tâche.
//!
//! Une ligne du tableau (14 octets) : `+0` niveau, `+1` personnage, `+2..+7` nom (0xff : blanc), `+8` score
//! (7 chiffres BCD), `+0xc` variante de l'emblème, `+0xd` emblème présent (objet EFFCT de type 0x5b).
//!
//! Les cases de la couche de texte (scroll 1) sont des paires de mots (tuile, attribut), rangées par colonnes :
//! `+4` ligne suivante, `+0x80` colonne suivante. Les tables de tuiles sont lues dans la ROM (données).

use crate::aides::divide;
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8};
use crate::modes::{Flow, Sequence};
use crate::scene::Ctx;
use crate::video::Grille;
use crate::objects::pools;

/// Bloc d'état de l'écran (`A0`, A5 - 0x70b0).
mod etat {
    pub const BASE: u32 = 0xff_0f50;
    pub const ETAPE: u32 = BASE;
    pub const SOUS_ETAPE: u32 = BASE + 1;
    /// Écran fini : la machine à modes passe à la suite.
    pub const FINI: u32 = BASE + 3;
    /// Minuterie (mot signé).
    pub const MINUTERIE: u32 = BASE + 0x10;
}

/// Lignes du tableau : rangs 5 à 10 (de `RANGS_BAS` vers le bas de la mémoire), rangs 2 à 4, meilleur score.
const RANGS_BAS: u32 = 0xff_104e;
const RANGS_HAUTS: u32 = 0xff_0fde;
const MEILLEUR: u32 = 0xff_0fd0;
const LIGNE: u32 = 0xe;
/// Octet `+1` des quatre objets EFFCT des premiers rangs, lu de `EMBLEMES` vers le bas (pas `LIGNE`).
const EMBLEMES: u32 = 0xff_0ffa;

/// Variables de la RAM de travail (décalages depuis A5).
mod var {
    pub const LAYERS: u32 = 0x14;
    pub const TERRAIN_VIEW: u32 = 0x36;
    pub const SCROLL_2: u32 = 0x4ba;
    pub const CAMERA_X: u32 = 0x506;
    pub const CAMERA_Y: u32 = 0x50a;
    pub const CAMERA_2_X: u32 = 0x556;
    pub const CAMERA_2_Y: u32 = 0x55a;
    pub const FADE_PAGES: u32 = 0x48e;
}

crate::etapes! {
    /// Étapes de la tâche : l'entrée et le retour de la cession, qui font chacune un tour.
    pub enum Etape {
        Entree = 0x1835e => tour,
        Reprise = 0x18384 => tour,
        /// Retour de l'appel de l'étape (`jsr (table,PC)`) : les objets, les sprites, une image de cession.
        ApresEtape = at::APRES_ETAPE => apres_etape,
    }
}

/// Adresses de l'original.
mod at {
    /// Retour de l'appel de l'étape (`jsr (table,PC)`).
    pub const APRES_ETAPE: u32 = 0x1836e;
}

/// Tables de tuiles (ROM, données).
mod tuiles {
    /// Grands chiffres (4 x 4 cases) du meilleur score.
    pub const GRANDS_CHIFFRES: u32 = 0x18d58;
    /// Chiffres (2 x 2 cases).
    pub const CHIFFRES: u32 = 0x18d6c;
    /// Numéros de rang 5 à 10 (2 x 2).
    pub const RANGS: u32 = 0x18d82;
    /// Portrait de chaque personnage : tuile et attribut (4 x 2 cases).
    pub const PORTRAITS: u32 = 0x18d8e;
    /// Lettres des noms (2 x 2).
    pub const LETTRES: u32 = 0x18dbe;
    /// Numéros des rangs 2 à 4 (6 x 4 cases).
    pub const GRANDS_RANGS: u32 = 0x18b88;
    /// Premier rang (6 x 4).
    pub const PREMIER_RANG: u16 = 0x230;
    /// Case vide des grands caractères.
    pub const BLANC: u16 = 0x2fc;
}

/// Palettes de la première étape : source en ROM et destination, toutes à pleine luminosité.
const PALETTES: [(u32, u32); 2] = [(0x2537c0, 0x91c3c0), (0x28f4c0, 0x91c180)];
/// Titre : grands caractères (2 x 2 cases, attribut 1) écrits à partir de `TITRE_CASE` ; 0 = blanc.
const TITRE_CASE: u32 = 0x91021c;
const TITRE: [u16; 20] = [
    0x192, 0x194, 0x190, 0x192, 0, 0x190, 0x1b6, 0x174, 0x17a, 0x17c, 0, 0x1ba, 0x1b6, 0x174, 0x1be, 0x17c, 0x19a, 0x17c,
    0x1b6, 0x1b8,
];

/// Une ligne du tableau.
#[derive(Clone, Copy)]
struct Ligne(u32);

impl Ligne {
    fn niveau(self, x: &mut Ctx) -> u8 {
        rd8(x.b, self.0)
    }
    fn personnage(self, x: &mut Ctx) -> u8 {
        rd8(x.b, self.0 + 1)
    }
    fn lettre(self, x: &mut Ctx, i: u32) -> u8 {
        rd8(x.b, self.0 + 2 + i)
    }
    fn score(self, x: &mut Ctx) -> u32 {
        rd32(x.b, self.0 + 8)
    }
}

/// Pinceau sur la couche de texte : `p` est l'adresse de la case courante.
struct Pinceau(u32);

impl Pinceau {
    fn avance(&mut self, d: i32) {
        self.0 = self.0.wrapping_add(d as u32);
    }
    fn case(&self, x: &mut Ctx, d: u32, tuile: u16, attribut: u16) {
        wr16(x.b, self.0 + d, tuile);
        wr16(x.b, self.0 + d + 2, attribut);
    }
    /// Caractère de 2 x 2 cases : `t`, `t + 1` à droite, `t + 0x10` et `t + 0x11` dessous.
    fn car(&self, x: &mut Ctx, d: u32, t: u16, attribut: u16) {
        self.case(x, d, t, attribut);
        self.case(x, d + 0x80, t + 1, attribut);
        self.case(x, d + 4, t + 0x10, attribut);
        self.case(x, d + 0x84, t + 0x11, attribut);
    }
    /// Bloc de `largeur` x `hauteur` cases, tuiles consécutives par ligne, lignes de 16 tuiles.
    fn bloc(&mut self, x: &mut Ctx, t: u16, largeur: u16, hauteur: u16, attribut: u16) {
        for ligne in 0..hauteur {
            for col in 0..largeur {
                self.case(x, 0x80 * col as u32 + 4 * ligne as u32, t + 0x10 * ligne + col, attribut);
            }
        }
    }
    /// Nom de six lettres (2 x 2, attribut 2), une lettre toutes les deux colonnes.
    fn nom(&mut self, x: &mut Ctx, ligne: Ligne) {
        for i in 0..6 {
            let c = match ligne.lettre(x, i) {
                0xff => 0x81,
                c => c,
            };
            let t = rd16(x.b, tuiles::LETTRES + 2 * c as u32);
            self.car(x, 0, t, 2);
            self.avance(0x100);
        }
    }
    /// Les sept chiffres bas du score (2 x 2, attribut 3).
    fn score(&mut self, x: &mut Ctx, ligne: Ligne) {
        let score = ligne.score(x);
        for i in (0..7).rev() {
            let t = rd16(x.b, tuiles::CHIFFRES + 2 * (score >> (4 * i) & 0xf));
            self.car(x, 0, t, 3);
            self.avance(0x100);
        }
    }
    /// Niveau en deux chiffres : tuile = chiffre, attribut 1, sur deux colonnes.
    fn niveau(&self, x: &mut Ctx, ligne: Ligne) {
        let (dizaines, unites) = divide(ligne.niveau(x) as u32, 10);
        self.case(x, 0, dizaines as u16, 1);
        self.case(x, 0x80, unites as u16, 1);
    }
}

/// `0x18a0e` : l'emblème de la ligne (objet EFFCT de type 0x5b), s'il en a un.
fn embleme(x: &mut Ctx, ligne: Ligne) {
    if rd8(x.b, ligne.0 + 0xd) == 0 {
        return;
    }
    if let Some(o) = alloue(x, &pools::EFFCT) {
        wr8(x.b, o, 1);
        wr8(x.b, o + 2, 0x5b);
        let variante = rd8(x.b, ligne.0 + 0xc);
        wr8(x.b, o + 3, variante);
        wr8(x.b, o + 0x2f, 0);
    }
}

/// Allocation d'un objet : son adresse, ou rien (retenue levée).
fn alloue(x: &mut Ctx, pool: &pools::Pool) -> Option<u32> {
    pool.alloc(x.b).map(|o| o.0)
}

/// `0x18b8e` : ligne des rangs 5 à 10 (`rang` = 10 à 5) à partir de la case `p`.
fn ligne_basse(x: &mut Ctx, ligne: Ligne, p: u32, rang: u16) {
    embleme(x, ligne);
    let mut p = Pinceau(p);
    let t = rd16(x.b, tuiles::RANGS + 2 * (rang - 5) as u32);
    p.car(x, 0, t, 2);
    p.avance(0x100);
    p.case(x, 0, 0x215, 1);
    p.case(x, 4, 0x225, 1);
    p.avance(0x80);
    p.case(x, 0, 0x1f9, 1);
    p.case(x, 4, 0x209, 1);
    p.avance(0x200);
    let portrait = tuiles::PORTRAITS + 4 * ligne.personnage(x) as u32;
    let (t, attribut) = (rd16(x.b, portrait), rd16(x.b, portrait + 2));
    p.bloc(x, t, 4, 2, attribut);
    p.avance(0x2fc);
    p.nom(x, ligne);
    p.avance(0x100);
    p.score(x, ligne);
    // « exp » après le score, « LEVEL » sur la ligne du dessus, puis le niveau
    p.avance(4);
    for (k, t) in [0x65, 0x78, 0x70].into_iter().enumerate() {
        p.case(x, 0x80 * k as u32, t, 3);
    }
    p.avance(0x100 - 0xefc);
    for (k, t) in [0x4c, 0x45, 0x56, 0x45, 0x4c].into_iter().enumerate() {
        p.case(x, 0x80 * k as u32, t, 1);
    }
    p.avance(0x300);
    p.niveau(x, ligne);
}

/// `0x18a3e` : ligne des rangs 2 à 4 (`rang` = 2 à 4) à partir de la case `p`.
fn ligne_haute(x: &mut Ctx, ligne: Ligne, p: u32, rang: u16) {
    embleme(x, ligne);
    let mut p = Pinceau(p);
    let t = rd16(x.b, tuiles::GRANDS_RANGS + 2 * (rang - 2) as u32);
    p.bloc(x, t, 6, 4, 0x1e);
    p.avance(0x14);
    p.score(x, ligne);
    p.avance(-0x6f8);
    p.nom(x, ligne);
    p.avance(-0x6fc + 0x384);
    // « LV » puis le niveau
    p.case(x, 0, 0x4c, 1);
    p.case(x, 0x80, 0x56, 1);
    p.avance(0x100);
    p.niveau(x, ligne);
}

/// Étape 0 (`0x1838a`) : le décor et les rangs 5 à 10.
fn decor(x: &mut Ctx) -> Flow {
    crate::video::vider_objets(x.b);
    crate::video::effacer_couche(x.b, crate::video::Couche::Un);
    let etape = rd8(x.b, etat::ETAPE).wrapping_add(2);
    wr8(x.b, etat::ETAPE, etape);
    wr16(x.b, etat::MINUTERIE, 0x40);
    x.set_v8(var::TERRAIN_VIEW, 0);
    x.set_v16(var::CAMERA_X, 0);
    x.set_v16(var::CAMERA_Y, 0x10);
    x.set_v16(var::CAMERA_2_X, 0);
    x.set_v16(var::CAMERA_2_Y, 0x20);
    // 0x1a366 (palettes de la scène 1), pages 1 à 3 (0x1a60a, 0x1a67e, 0x1a8b2), 0x1a5e8
    crate::palette::charger_palettes_de_scene(x.b, 1);
    for (page, n) in [(1, 2), (2, 0x3a), (3, 0x3a)] {
        crate::palette::charger_page(x.b, page, n);
    }
    crate::families::petites::palette_douze_blocs(x.b);
    // `ori.l #0xf000f000` sur les six premiers longs, `adda.l` (addition) sur les deux derniers
    for (source, dest) in PALETTES {
        for i in 0..8 {
            let v = rd32(x.b, source + 4 * i);
            let v = if i < 6 { v | 0xf000_f000 } else { v.wrapping_add(0xf000_f000) };
            wr32(x.b, dest + 4 * i, v);
        }
    }
    for (grille, n, dest) in [
        (Grille::Couche2, 0x80, 0x917000),
        (Grille::Couche2, 0x81, 0x917400),
        (Grille::Couche3, 0x111, 0x91b800),
        (Grille::Couche3, 0x112, 0x91b900),
    ] {
        crate::video::charger_grille(x.b, grille, n, dest);
    }
    wr16(x.b, 0x400004, 0x1144);
    x.set_v16(var::LAYERS, 0);
    crate::video::activer_couches(x.b, 0x1e0e);
    for k in 0..6 {
        ligne_basse(x, Ligne(RANGS_BAS - LIGNE * k), 0x912168 - 0x10 * k, 10 - k as u16);
    }
    if let Some(o) = alloue(x, &pools::SET) {
        wr8(x.b, o, 1);
        wr8(x.b, o + 2, 4);
        wr8(x.b, o + 3, 3);
        wr8(x.b, o + 0x2f, 1);
    }
    for (pages, niveau) in [(0xc, 0xa000), (3, 0xf000)] {
        x.set_v8(var::FADE_PAGES, pages);
        crate::palette::or_brightness(x.b, niveau);
    }
    Flow::Done
}

/// Minuterie de l'écran : décomptée ; vrai quand elle passe sous zéro.
fn minuterie(x: &mut Ctx) -> bool {
    let reste = rd16(x.b, etat::MINUTERIE).wrapping_sub(1);
    wr16(x.b, etat::MINUTERIE, reste);
    (reste as i16) < 0
}

fn sous_etape_suivante(x: &mut Ctx, minuterie: u16) {
    let s = rd8(x.b, etat::SOUS_ETAPE).wrapping_add(2);
    wr8(x.b, etat::SOUS_ETAPE, s);
    wr16(x.b, etat::MINUTERIE, minuterie);
}

/// Sous-étape 0 (`0x18544`) : après 0x40 images, les rangs 1 à 4 et le titre ; puis la montée commence.
fn premiers_rangs(x: &mut Ctx) -> Flow {
    if !minuterie(x) {
        return Flow::Done;
    }
    sous_etape_suivante(x, 0xe0);
    for k in 0..3 {
        ligne_haute(x, Ligne(RANGS_HAUTS + LIGNE * k), 0x9100d8 + 0x800 * k, 2 + k as u16);
    }
    for k in 0..4 {
        if let Some(o) = alloue(x, &pools::EFFCT) {
            wr8(x.b, o, 1);
            wr8(x.b, o + 2, 3);
            wr8(x.b, o + 3, 3 - k as u8);
            let v = rd8(x.b, EMBLEMES - LIGNE * k + 1);
            wr8(x.b, o + 0x2f, v);
        }
    }
    let titre = Pinceau(TITRE_CASE);
    for (k, t) in TITRE.into_iter().enumerate() {
        let d = 0x100 * k as u32;
        if t == 0 {
            for case in [0, 4, 0x80, 0x84] {
                titre.case(x, d + case, tuiles::BLANC, 1);
            }
        } else {
            titre.car(x, d, t, 1);
        }
    }
    meilleur_score(x);
    montee(x)
}

/// Fin de `0x18544` : le premier rang (`MEILLEUR`), en grands chiffres.
fn meilleur_score(x: &mut Ctx) {
    let ligne = Ligne(MEILLEUR);
    let mut p = Pinceau(0x9100ac);
    embleme(x, ligne);
    if let Some(o) = alloue(x, &pools::EFFCT) {
        wr8(x.b, o, 1);
        wr8(x.b, o + 2, 0x5b);
        wr8(x.b, o + 0x2f, 1);
    }
    p.bloc(x, tuiles::PREMIER_RANG, 6, 4, 0x1e);
    p.avance(0x710);
    let score = ligne.score(x);
    for i in (0..7).rev() {
        let t = rd16(x.b, tuiles::GRANDS_CHIFFRES + 2 * (score >> (4 * i) & 0xf));
        p.bloc(x, t, 4, 4, 3);
        p.avance(0x200);
    }
    p.avance(-0xcf0);
    p.nom(x, ligne);
    p.avance(0x300);
    // « LV » puis le niveau en chiffres 2 x 2
    p.car(x, 0, 0x19a, 1);
    p.car(x, 0x100, 0x1be, 1);
    p.avance(0x200);
    let (dizaines, unites) = divide(ligne.niveau(x) as u32, 10);
    for (k, chiffre) in [dizaines, unites].into_iter().enumerate() {
        let t = rd16(x.b, tuiles::CHIFFRES + 2 * chiffre);
        p.car(x, 0x100 * k as u32, t, 1);
    }
}

/// Sous-étape 2 (`0x189e0`) : la couche 2 monte d'une ligne par image pendant 0xe0 images.
fn montee(x: &mut Ctx) -> Flow {
    let y = x.v16(var::SCROLL_2).wrapping_add(1);
    x.set_v16(var::SCROLL_2, y);
    if minuterie(x) {
        sous_etape_suivante(x, 0x90);
    }
    Flow::Done
}

/// Sous-étape 4 (`0x189f8`) : après 0x90 images, l'écran est fini et la tâche se termine (`jmp 0x9f8` depuis
/// l'étape : le retour vers la boucle reste sur la pile).
fn fin(x: &mut Ctx) -> Flow {
    if !minuterie(x) {
        return Flow::Done;
    }
    let fini = rd8(x.b, etat::FINI).wrapping_add(1);
    wr8(x.b, etat::FINI, fini);
    super::terminer(x)
}

/// Un tour de la boucle : l'étape, les objets, les sprites, une image de cession.
fn tour(x: &mut Ctx) -> Flow {
    x.c.a[0] = etat::BASE;
    let etape = rd8(x.b, etat::ETAPE);
    x.push_return(Etape::ApresEtape);
    match (etape, rd8(x.b, etat::SOUS_ETAPE)) {
        (0, _) => decor(x),
        (2, 0) => premiers_rangs(x),
        (2, 2) => montee(x),
        (2, 4) => fin(x),
        // les tables 0x18386 (deux étapes) et 0x1853c (trois sous-étapes) sont réécrites ; `fin` termine la tâche
        // (code-mort.md, lot « sortie du Cpu »)
        (e, s) => unreachable!("tableau des scores : étape {e:#x}, sous-étape {s:#x} hors des tables"),
    }
}

/// `0x1836e` : retour de l'étape.
fn apres_etape(x: &mut Ctx) -> Flow {
    x.c.a[0] = etat::BASE;
    crate::dispatch::EFFCT.update_sans_suite(x.c, x.b);
    crate::dispatch::SET.update_sans_suite(x.c, x.b);
    crate::sprites::construire_68k(x.c, x.b);
    super::une_image(x, Etape::Reprise)
}
