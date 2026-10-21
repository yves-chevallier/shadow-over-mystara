//! Défilement (B1) : les caméras 1 (couche de défilement 2) et 2 (couche 3) recopient dans leur
//! couche les colonnes et lignes de décor qui entrent à l'écran. Voir `docs/video.md`.
//!
//! Une caméra est un enregistrement de la RAM de travail (`A5 + 0x4fe` et `A5 + 0x54e`, adressés par
//! A0 dans le code d'origine). Chaque image, son script de mode (code d'origine, table de sauts par
//! mode) déplace la position cible (x, y) ; `draw` compare ensuite la cible à la position déjà
//! dessinée et, par pas de 16 pixels (32 pour la caméra 2), recopie une colonne ou une ligne de cases
//! de la carte du décor (table d'indices de blocs en ROM, bandes choisies par scène) vers la couche.
//!
//! Les natives s'enchaînent dans le code d'origine aux points d'entrée situés après l'appel du script
//! de mode (0x2f81c : avec butées de la cible ; 0x2f86e et 0x328a8 : recopie seule) ; les scripts de
//! mode et l'initialisation de la scène (premier appel) restent du code d'origine. La durée est
//! exacte : le parcours est rejoué en simulation sur le code d'origine (`Exec`), tronçon par
//! tronçon, pour que les boucles de 19 à 35 cases ne dépassent pas la capacité d'un `Chemin`.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::chemin::Chemin;
use crate::mem::rts;
use crate::video::hw;
use crate::vue::Vue;
use crate::Native;

/// Champs de l'enregistrement caméra (offsets depuis son adresse).
pub(crate) mod cam {
    /// Position cible.
    pub const X: u32 = 0x08;
    pub const Y: u32 = 0x0c;
    /// Cible de l'image précédente.
    pub const PREVIOUS_X: u32 = 0x26;
    pub const PREVIOUS_Y: u32 = 0x2a;
    /// Position déjà dessinée.
    pub const DRAWN_X: u32 = 0x10;
    pub const DRAWN_Y: u32 = 0x14;
    /// Écart cible - dessiné.
    pub const DX: u32 = 0x1c;
    pub const DY: u32 = 0x1e;
    pub const MODE: u32 = 0x20;
    /// Curseur dans la carte : décalage dans la bande (+0x22) et numéro de bande (+0x24).
    pub const CURSOR_OFFSET: u32 = 0x22;
    pub const CURSOR_ROW: u32 = 0x24;
    /// Butée gauche (ne recule pas), butée droite.
    pub const MIN_X: u32 = 0x2e;
    pub const MAX_X: u32 = 0x34;
    /// Non nul : la butée gauche ne suit pas la cible.
    pub const HOLD_MIN: u32 = 0x3d;
    /// Non nul : pas de défilement.
    pub const FROZEN: u32 = 0x3e;
    /// Pas de bande par ligne de 256 pixels.
    pub const ROW_STEP: u32 = 0x40;
}

/// Enregistrements des caméras 1 et 2 (offsets depuis A5).
pub(crate) const CAMERA_1: u32 = 0x4fe;
pub(crate) const CAMERA_2: u32 = 0x54e;
const STAGE: u32 = 0x34;
const TERRAIN_VIEW: u32 = 0x37;
const MAP: u32 = 0x1f9ba0;
const BANKS: u32 = 0x320f6;
/// Table (en code) des tables de bandes par scène.
const ROW_TABLES: u32 = 0x2ff94;
/// Cases par colonne et par ligne recopiées à chaque pas.
const COLUMN_CELLS: u32 = 19;
const ROW_CELLS: u32 = 35;
/// Tuiles de terrain affichées en mode débogage selon le bit 13 de l'attribut.
const TERRAIN_TILES: [u16; 2] = [0x1bc0, 0x1d80];
/// Décalage de tuile ajouté aux cases de la carte.
const MAP_TILE_OFFSET: u32 = 0x1800_0000;

/// Routines d'origine appelées (leur durée est comptée à part).
const HELPERS: &[u32] = &[0x2fdfc, 0x2fe30, 0x2fe5c, 0x2fe6a, 0x2fe78, 0x2fec0, 0x2fefa, 0x2ff1c, 0x2ff66];

// ---------------------------------------------------------------------------------------------
// Exécution réelle ou simulée (pour la durée exacte)
// ---------------------------------------------------------------------------------------------

/// Parcours du code d'origine en simulation : durée du tronçon en cours depuis `from`, chaque
/// sous-routine ayant son propre tronçon.
struct Sim {
    total: u64,
    frames: Vec<(u32, Chemin)>,
    /// Sous-routines d'origine appelées (comptées à part).
    helpers: &'static [u32],
}

struct Exec<'a> {
    v: Vue<'a>,
    sim: Option<Sim>,
}

impl<'a> Exec<'a> {
    fn real(b: &'a mut Cps2Bus) -> Self {
        Exec { v: Vue::reelle(b), sim: None }
    }

    fn simulated(b: &'a mut Cps2Bus, entry: u32, helpers: &'static [u32]) -> Self {
        Exec { v: Vue::simulee(b), sim: Some(Sim { total: 0, frames: vec![(entry, Chemin::default())], helpers }) }
    }

    /// Décision d'un branchement conditionnel de l'original (vrai : pris).
    fn pris(&mut self, pris: bool) -> bool {
        if let Some(s) = &mut self.sim {
            s.frames.last_mut().unwrap().1.pris(pris);
        }
        pris
    }

    /// L'exécution atteint `addr` : le tronçon en cours est terminé.
    fn mark(&mut self, addr: u32) {
        if let Some(s) = &mut self.sim {
            let (from, path) = s.frames.last_mut().unwrap();
            s.total += std::mem::take(path).duree_hors(self.v.b, *from, Some(addr), s.helpers);
            *from = addr;
        }
    }

    /// Boucle `dbf` de `n` tours dont la tête est `head` (dont l'exécution vient d'être marquée) : les
    /// `n - 1` premiers tours sont comptés ici, le dernier est le suivant dans le tronçon.
    fn repeat(&mut self, head: u32, n: u32) {
        if let Some(s) = &mut self.sim {
            let mut again = Chemin::default();
            again.pris(true);
            s.total += (n as u64 - 1) * again.duree_hors(self.v.b, head, Some(head), &[]);
            s.frames.last_mut().unwrap().1.pris(false);
        }
    }

    /// Entrée dans une sous-routine d'origine `entry` (son tronçon est compté à part).
    fn enter(&mut self, entry: u32) {
        if let Some(s) = &mut self.sim {
            s.frames.push((entry, Chemin::default()));
        }
    }

    /// Retour de la sous-routine.
    fn leave(&mut self) {
        if let Some(s) = &mut self.sim {
            let (from, path) = s.frames.pop().unwrap();
            s.total += path.duree_hors(self.v.b, from, None, s.helpers);
        }
    }

    /// Durée totale jusqu'au RTS.
    fn finish(mut self) -> u64 {
        let s = self.sim.as_mut().expect("simulation");
        let (from, path) = s.frames.pop().unwrap();
        s.total + path.duree_hors(self.v.b, from, None, s.helpers)
    }
}

/// Enregistre la décision d'un branchement (l'expression est évaluée avant d'emprunter la caméra).
macro_rules! pris {
    ($cam:expr, $cond:expr) => {{
        let taken = $cond;
        $cam.x.pris(taken)
    }};
}

// ---------------------------------------------------------------------------------------------
// Caméra 1 : décor de la couche de défilement 2
// ---------------------------------------------------------------------------------------------

struct Camera1<'a> {
    x: Exec<'a>,
    a5: u32,
    cam: u32,
    /// D4 laissé par la dernière recopie (0x3f : colonne, 0x40 : ligne), relu par les routines suivantes.
    d4: Option<u32>,
}

/// Ce qui suit un pas de recopie : reprendre les tests, ou quitter la routine.
enum Step {
    Continue,
    Return,
}

impl Camera1<'_> {
    /// Appel d'une sous-routine d'origine `entry`.
    fn helper<R>(&mut self, entry: u32, f: impl FnOnce(&mut Self) -> R) -> R {
        self.x.enter(entry);
        let r = f(self);
        self.x.leave();
        r
    }

    fn get(&mut self, field: u32) -> u16 {
        self.x.v.rd16(self.cam + field)
    }
    fn set(&mut self, field: u32, v: u16) {
        self.x.v.wr16(self.cam + field, v);
    }
    fn add(&mut self, field: u32, d: u16) {
        let v = self.get(field).wrapping_add(d);
        self.set(field, v);
    }

    /// 0x2f81c : suite du script de mode : limites de la cible, puis recopie du décor.
    fn after_mode(&mut self) {
        if pris!(self, self.x.v.rd8(self.cam + cam::FROZEN) != 0) {
            return;
        }
        if pris!(self, self.get(cam::MODE) == 4) {
            return self.draw();
        }
        let (limit, mut x) = (self.get(cam::MAX_X) as i16, self.get(cam::X) as i16);
        if !pris!(self, limit >= x) {
            self.set(cam::X, limit as u16);
            x = limit;
        }
        let behind = x.wrapping_sub(0x60);
        let min = self.get(cam::MIN_X) as i16;
        if !pris!(self, behind <= min) {
            if !pris!(self, self.x.v.rd8(self.cam + cam::HOLD_MIN) != 0) {
                self.set(cam::MIN_X, behind as u16);
            }
        } else if !pris!(self, x >= min) {
            self.set(cam::X, min as u16);
        }
        self.draw();
    }

    /// 0x2f86e : écarts cible - dessiné, puis recopie des colonnes et lignes manquantes. Les quatre
    /// tests se suivent ; après un pas, l'original reprend au test du même sens.
    fn draw(&mut self) {
        let dx = self.get(cam::X).wrapping_sub(self.get(cam::DRAWN_X));
        self.set(cam::DX, dx);
        let dy = self.get(cam::Y).wrapping_sub(self.get(cam::DRAWN_Y));
        self.set(cam::DY, dy);
        let mut test = 0;
        loop {
            match test {
                0 => {
                    self.x.mark(0x2f886);
                    if pris!(self, self.get(cam::DX) as i16 >= 0x10) {
                        self.column(true);
                    } else {
                        test = 1;
                    }
                }
                1 => {
                    self.x.mark(0x2f890);
                    if pris!(self, self.get(cam::DX) as i16 <= -0x10) {
                        if let Step::Return = self.column(false) {
                            return;
                        }
                    } else {
                        test = 2;
                    }
                }
                2 => {
                    self.x.mark(0x2f89a);
                    if pris!(self, self.get(cam::DY) as i16 >= 0x10) {
                        self.row(true);
                    } else {
                        test = 3;
                    }
                }
                _ => {
                    self.x.mark(0x2f8a4);
                    if pris!(self, self.get(cam::DY) as i16 <= -0x10) {
                        if let Step::Return = self.row(false) {
                            return;
                        }
                    } else {
                        return;
                    }
                }
            }
        }
    }

    // --- sous-routines d'origine ---

    /// 0x2fdfc : ligne de la case à écrire (décalage dans la couche) pour le bas de l'écran,
    /// y dessiné arrondi au multiple de 16 supérieur.
    fn row_offset_down(&mut self) -> u16 {
        self.helper(0x2fdfc, |s| {
            let mut y = s.get(cam::DRAWN_Y);
            if !pris!(s, y & 0xf == 0) {
                y = y.wrapping_add(0x10);
            }
            layer_row_offset(y.wrapping_add(0x100), y)
        })
    }

    /// 0x2fe30 : idem pour le haut de l'écran (y dessiné - 16).
    fn row_offset_up(&mut self) -> u16 {
        self.helper(0x2fe30, |s| {
            let y = s.get(cam::DRAWN_Y).wrapping_sub(0x10) & 0x3ff;
            layer_row_offset(y, y)
        })
    }

    /// 0x2fe5c : colonne (décalage dans la couche) à gauche de l'écran : (x dessiné - 0x40) x 4.
    fn column_left(&mut self) -> u16 {
        self.helper(0x2fe5c, |s| s.get(cam::DRAWN_X).wrapping_sub(0x40).wrapping_mul(4))
    }

    /// 0x2fe6a : colonne à droite de l'écran : (x dessiné + 0x1e0) x 4.
    fn column_right(&mut self) -> u16 {
        self.helper(0x2fe6a, |s| s.get(cam::DRAWN_X).wrapping_add(0x1e0).wrapping_mul(4))
    }

    /// 0x2fe78 (bas) / 0x2fec0 (haut) : curseur de carte pour la ligne en bas / en haut de l'écran. La
    /// bande avance de `ROW_STEP` par 256 pixels ; le décalage dans la bande vient du quartet haut de y.
    fn row_cursor(&mut self, down: bool) {
        self.helper(if down { 0x2fe78 } else { 0x2fec0 }, |s| {
            let mut y = s.get(cam::DRAWN_Y);
            // bas : y arrondi au multiple de 16 supérieur, distance mesurée 256 pixels plus bas ;
            // haut : 16 pixels au-dessus
            let (mut distance, nibble_source) = if down {
                if !pris!(s, y & 0xf == 0) {
                    y = y.wrapping_add(0x10);
                }
                (y.wrapping_add(0x100), y)
            } else {
                let above = y.wrapping_sub(0x10);
                (above, above)
            };
            let mut step = s.get(cam::ROW_STEP);
            if !pris!(s, distance as i16 >= 0) {
                distance = distance.wrapping_neg();
                step = step.wrapping_neg();
            }
            let strips = distance >> 8;
            let head = if down { 0x2fea4 } else { 0x2fede };
            s.x.mark(head);
            s.x.repeat(head, strips as u32 + 1);
            s.set(cam::CURSOR_ROW, step.wrapping_mul(strips).wrapping_neg());
            s.set(cam::CURSOR_OFFSET, 0x3c - ((nibble_source & 0xf0) >> 2));
        });
    }

    /// 0x2ff1c (colonne droite) / 0x2fefa (colonne gauche) : avance le curseur de la colonne ajoutée.
    fn cursor_for_column(&mut self, right: bool) {
        self.helper(if right { 0x2ff1c } else { 0x2fefa }, |s| {
            let x = s.get(cam::DRAWN_X);
            let column = if right { x.wrapping_add(0x1e0) } else { x.wrapping_sub(0x40) } & 0xfff0;
            s.add(cam::CURSOR_OFFSET, (column & 0xff).wrapping_mul(4));
            let strip = if right { column >> 8 } else { (column >> 8) as u8 as i8 as u16 };
            s.add(cam::CURSOR_ROW, strip);
        });
    }

    /// 0x2ff66 : adresse en ROM de la bande de la carte désignée par le curseur.
    fn strip_address(&mut self) -> u32 {
        let stage = self.x.v.rd8(self.a5 + STAGE) as u32;
        self.helper(0x2ff66, |s| {
            let table = s.x.v.prog16(ROW_TABLES + 4 * stage) as u32;
            let table = table << 16 | s.x.v.prog16(ROW_TABLES + 4 * stage + 2) as u32;
            let strip = s.get(cam::CURSOR_ROW).wrapping_mul(2) as i16 as u32;
            let first = s.x.v.rd16(table.wrapping_add(strip)) as u32;
            MAP + (first << 10)
        })
    }

    // --- recopie ---

    /// Case de la carte à la position `at` (adresse dans la table d'indices), au format de la couche.
    fn map_cell(&mut self, at: u32) -> u32 {
        let off = at.wrapping_sub(MAP);
        let bank = self.x.v.rd32(BANKS + (off >> 16 & 0xc));
        let c = off >> 2;
        let index = self.x.v.rd16(MAP + (c & 0xe) + (c >> 1 & 0x3fff0)) as u32;
        let sub = (off as u16 >> 3 & 8) as u32 + (off & 4);
        self.x.v.rd32(bank.wrapping_add(index << 4).wrapping_add(sub))
    }

    /// Écrit la case `cell` en `at` : tuile de la carte, ou tuile de terrain en mode débogage.
    fn put_cell(&mut self, at: u32, cell: u32) {
        let terrain_view = self.x.v.rd8(self.a5 + TERRAIN_VIEW) != 0;
        if !pris!(self, !terrain_view) {
            let attribute = cell & 0xff80;
            let number = (attribute >> 7) as u16;
            let tile = number.wrapping_add(TERRAIN_TILES[pris!(self, number & 0x40 != 0) as usize]);
            self.x.v.wr16(at, tile);
            self.x.v.wr16(at + 2, attribute as u16);
        } else {
            self.x.v.wr32(at, cell.wrapping_add(MAP_TILE_OFFSET));
        }
    }

    /// Colonne de 19 cases à droite (`right`) ou à gauche de l'écran, écrite de haut en bas.
    fn column(&mut self, right: bool) -> Step {
        self.add(cam::DX, if right { 0x10u16.wrapping_neg() } else { 0x10 });
        let drawn = self.get(cam::DRAWN_X).wrapping_add(if right { 0x10 } else { 0x10u16.wrapping_neg() });
        self.set(cam::DRAWN_X, drawn);
        if !right && pris!(self, (drawn as i16) < 0) {
            return Step::Return;
        }
        self.d4 = Some(0x3f);
        let d3 = self.row_offset_down();
        let d2 = if right { self.column_right() } else { self.column_left() };
        let mut a1 = hw::SCROLL2 + (d2 & 0xfc0) as u32;
        self.row_cursor(true);
        self.cursor_for_column(right);
        let mut a2 = self.strip_address();
        let offset = self.get(cam::CURSOR_OFFSET);
        a2 = a2.wrapping_add(offset as i16 as u32);
        a1 = a1.wrapping_add(d3 as i16 as u32);
        let wrap_offset = offset & 0x3c0;
        let head = if right { 0x2f8f2 } else { 0x2fa48 };
        for i in 0..COLUMN_CELLS {
            self.x.mark(head);
            let cell = self.map_cell(a2);
            a2 += 4;
            self.put_cell(a1, cell);
            a1 += 4;
            if !pris!(self, a1 as u16 & 0x3f != 0) {
                // fin de colonne de la couche : bande suivante de la carte, colonne suivante de la couche
                let step = self.get(cam::ROW_STEP);
                self.add(cam::CURSOR_ROW, step);
                a2 = self.strip_address().wrapping_add(wrap_offset as i16 as u32);
                let next = (a1 as u16).wrapping_add(0xfc0) & 0x3fc0;
                a1 = hw::SCROLL2.wrapping_add(next as i16 as u32);
            }
            pris!(self, i + 1 < COLUMN_CELLS);
        }
        Step::Continue
    }

    /// Ligne de 35 cases en bas (`down`) ou en haut de l'écran, écrite de gauche à droite.
    fn row(&mut self, down: bool) -> Step {
        self.add(cam::DY, if down { 0x10u16.wrapping_neg() } else { 0x10 });
        let drawn = self.get(cam::DRAWN_Y).wrapping_add(if down { 0x10 } else { 0x10u16.wrapping_neg() });
        self.set(cam::DRAWN_Y, drawn);
        if !down && pris!(self, (drawn as i16) < 0) {
            return Step::Return;
        }
        self.d4 = Some(0x40);
        let d3 = if down { self.row_offset_down() } else { self.row_offset_up() };
        let d2 = self.column_left();
        let mut column = d2 & 0xfc0;
        let a1 = hw::SCROLL2 + d3 as u32;
        self.row_cursor(down);
        self.cursor_for_column(false);
        let mut a2 = self.strip_address();
        let mut offset = self.get(cam::CURSOR_OFFSET);
        let head = if down { 0x2fb90 } else { 0x2fce4 };
        for i in 0..ROW_CELLS {
            self.x.mark(head);
            let cell = self.map_cell(a2.wrapping_add(offset as u32));
            self.put_cell(a1.wrapping_add(column as i16 as u32), cell);
            column = column.wrapping_add(0x40) & 0xfc0;
            offset = offset.wrapping_add(0x40);
            if !pris!(self, offset & 0x3c0 != 0) {
                offset = offset.wrapping_sub(0x400);
                self.add(cam::CURSOR_ROW, 1);
                a2 = self.strip_address();
            }
            pris!(self, i + 1 < ROW_CELLS);
        }
        Step::Continue
    }
}

/// Décalage (dans la couche de défilement 2) de la ligne de cases pour l'ordonnée `y` : la couche est
/// découpée en pages de 0x1000 octets (4 pages de 256 pixels), 16 cases de 4 octets par colonne.
/// `page` sert à choisir la page, `y` la case dans la page.
fn layer_row_offset(page: u16, y: u16) -> u16 {
    0x3000u16.wrapping_sub((page & 0x300) << 4).wrapping_add(0x3c - (((y & 0xf0) >> 2) as u8 as u16))
}

fn camera<'a>(x: Exec<'a>, c: &Cpu) -> Camera1<'a> {
    Camera1 { x, a5: c.a[5], cam: c.a[0], d4: None }
}

/// 0x2f81c : suite du script de mode de la caméra 1 (A0), après le script.
pub(crate) fn after_mode(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut cam = camera(Exec::real(b), c);
    cam.after_mode();
    if let Some(d4) = cam.d4 {
        c.d[4] = d4;
    }
}

/// 0x2f86e : écarts cible - dessiné de la caméra 1 (A0), puis recopie des colonnes et lignes manquantes.
pub(crate) fn draw1(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut cam = camera(Exec::real(b), c);
    cam.draw();
    if let Some(d4) = cam.d4 {
        c.d[4] = d4;
    }
}


// ---------------------------------------------------------------------------------------------
// Caméra 2 : décor de la couche de défilement 3 (cases de 32x32 pixels, pas de 32 pixels)
// ---------------------------------------------------------------------------------------------

/// Carte du décor de la caméra 2 (indices de blocs 2x2 cases) et ses 16 banques de blocs.
const MAP_3: u32 = 0x2459c0;
const BANKS_3: u32 = 0x33a9c;
/// Table (en code) des tables de bandes par scène.
const ROW_TABLES_3: u32 = 0x32d7c;
/// Cases par colonne et par ligne recopiées à chaque pas.
const COLUMN_CELLS_3: u32 = 11;
const ROW_CELLS_3: u32 = 15;
/// Décalage de tuile ajouté aux cases de la carte.
const MAP_TILE_OFFSET_3: u32 = 0x1f00_0000;
/// Routines d'origine appelées par la caméra 2.
const HELPERS_3: &[u32] = &[0x32c2a, 0x32c5e, 0x32c8a, 0x32c96, 0x32ca2, 0x32cea, 0x32d22, 0x32d3c, 0x32d54];

struct Camera2<'a> {
    x: Exec<'a>,
    a5: u32,
    cam: u32,
    /// D4 laissé par la dernière recopie : 0x1f (long) pour une colonne, 0xe0 (mot bas) pour une ligne.
    d4: Option<(u32, u32)>,
}

impl Camera2<'_> {
    /// Appel d'une sous-routine d'origine `entry`.
    fn helper<R>(&mut self, entry: u32, f: impl FnOnce(&mut Self) -> R) -> R {
        self.x.enter(entry);
        let r = f(self);
        self.x.leave();
        r
    }

    fn get(&mut self, field: u32) -> u16 {
        self.x.v.rd16(self.cam + field)
    }
    fn set(&mut self, field: u32, v: u16) {
        self.x.v.wr16(self.cam + field, v);
    }
    fn add(&mut self, field: u32, d: u16) {
        let v = self.get(field).wrapping_add(d);
        self.set(field, v);
    }

    /// 0x328a8 : écarts cible - dessiné, puis recopie des colonnes et lignes manquantes (pas de 32).
    fn draw(&mut self) {
        let dx = self.get(cam::X).wrapping_sub(self.get(cam::DRAWN_X));
        self.set(cam::DX, dx);
        let dy = self.get(cam::Y).wrapping_sub(self.get(cam::DRAWN_Y));
        self.set(cam::DY, dy);
        let mut test = 0;
        loop {
            match test {
                0 => {
                    self.x.mark(0x328c0);
                    if pris!(self, self.get(cam::DX) as i16 >= 0x20) {
                        self.column(true);
                    } else {
                        test = 1;
                    }
                }
                1 => {
                    self.x.mark(0x328ca);
                    if pris!(self, self.get(cam::DX) as i16 <= -0x20) {
                        if let Step::Return = self.column(false) {
                            return;
                        }
                    } else {
                        test = 2;
                    }
                }
                2 => {
                    self.x.mark(0x328d4);
                    if pris!(self, self.get(cam::DY) as i16 >= 0x20) {
                        self.row(true);
                    } else {
                        test = 3;
                    }
                }
                _ => {
                    self.x.mark(0x328de);
                    if pris!(self, self.get(cam::DY) as i16 <= -0x20) {
                        if let Step::Return = self.row(false) {
                            return;
                        }
                    } else {
                        return;
                    }
                }
            }
        }
    }

    // --- sous-routines d'origine ---

    /// 0x32c2a (bas) / 0x32c5e (haut) : décalage dans la couche de la ligne de cases, y dessiné arrondi
    /// au multiple de 32 supérieur (bas) ou diminué de 32 (haut).
    fn row_offset(&mut self, down: bool) -> u16 {
        self.helper(if down { 0x32c2a } else { 0x32c5e }, |s| {
            let mut y = s.get(cam::DRAWN_Y);
            let page_source = if down {
                if !pris!(s, y & 0x1f == 0) {
                    y = y.wrapping_add(0x20);
                }
                y.wrapping_add(0x100)
            } else {
                y = y.wrapping_sub(0x20) & 0x7ff;
                y
            };
            0x3800u16.wrapping_sub((page_source & 0x700) << 3).wrapping_add(0x1c - ((y & 0xe0) >> 3))
        })
    }

    /// 0x32c8a / 0x32c96 : colonne (x - 0x20 ou x + 0x180) à gauche / à droite de l'écran.
    fn column_x(&mut self, right: bool) -> u16 {
        self.helper(if right { 0x32c96 } else { 0x32c8a }, |s| {
            let x = s.get(cam::DRAWN_X);
            if right { x.wrapping_add(0x180) } else { x.wrapping_sub(0x20) }
        })
    }

    /// 0x32ca2 (bas) / 0x32cea (haut) : curseur de carte pour la ligne du bas / du haut.
    fn row_cursor(&mut self, down: bool) {
        self.helper(if down { 0x32ca2 } else { 0x32cea }, |s| {
            let mut y = s.get(cam::DRAWN_Y);
            let mut step = s.get(cam::ROW_STEP);
            let (distance, nibble_source) = if down {
                if !pris!(s, y & 0x1f == 0) {
                    y = y.wrapping_add(0x20);
                }
                let mut distance = y.wrapping_add(0x100);
                if !pris!(s, distance as i16 >= 0) {
                    distance = distance.wrapping_neg();
                    step = step.wrapping_neg();
                }
                (distance, y)
            } else {
                let mut above = y.wrapping_sub(0x20);
                if !pris!(s, above as i16 >= 0) {
                    above = 0;
                }
                (above, above)
            };
            let strips = distance >> 8;
            let head = if down { 0x32cce } else { 0x32d06 };
            s.x.mark(head);
            s.x.repeat(head, strips as u32 + 1);
            s.set(cam::CURSOR_ROW, step.wrapping_mul(strips).wrapping_neg());
            s.set(cam::CURSOR_OFFSET, 0x1c - ((nibble_source & 0xe0) >> 3));
        });
    }

    /// 0x32d3c (colonne droite) / 0x32d22 (colonne gauche) : avance le curseur de la colonne ajoutée ;
    /// le décalage s'additionne sur l'octet bas seulement (sans retenue).
    fn cursor_for_column(&mut self, right: bool) {
        self.helper(if right { 0x32d3c } else { 0x32d22 }, |s| {
            let x = s.get(cam::DRAWN_X);
            let column = if right { x.wrapping_add(0x180) } else { x.wrapping_sub(0x20) } & 0xffe0;
            let low = s.x.v.rd8(s.cam + cam::CURSOR_OFFSET + 1).wrapping_add(column as u8);
            s.x.v.wr8(s.cam + cam::CURSOR_OFFSET + 1, low);
            let strip = if right { column >> 8 } else { (column >> 8) as u8 as i8 as u16 };
            s.add(cam::CURSOR_ROW, strip);
        });
    }

    /// 0x32d54 : adresse en ROM de la bande de la carte désignée par le curseur.
    fn strip_address(&mut self) -> u32 {
        let stage = self.x.v.rd8(self.a5 + STAGE) as u32;
        self.helper(0x32d54, |s| {
            let table = (s.x.v.prog16(ROW_TABLES_3 + 4 * stage) as u32) << 16 | s.x.v.prog16(ROW_TABLES_3 + 4 * stage + 2) as u32;
            let strip = s.get(cam::CURSOR_ROW).wrapping_mul(2) as i16 as u32;
            let first = s.x.v.rd16(table.wrapping_add(strip)) as u32;
            MAP_3 + (first << 8)
        })
    }

    // --- recopie ---

    /// Case de la carte à la position `at` ; `banks` : bits d'adresse qui choisissent la banque.
    fn map_cell(&mut self, at: u32, banks: u32) -> u32 {
        let off = at.wrapping_sub(MAP_3);
        let bank = self.x.v.rd32(BANKS_3 + ((off >> 16 & banks) << 2));
        let c = off >> 2;
        let index = self.x.v.rd16(MAP_3 + (c & 6) + (c >> 1 & !7)) as u32;
        let sub = (off as u16 >> 2 & 8) as u32 + (off & 4);
        self.x.v.rd32(bank.wrapping_add(index << 4).wrapping_add(sub)).wrapping_add(MAP_TILE_OFFSET_3)
    }

    /// Colonne de 11 cases à droite (`right`) ou à gauche de l'écran, écrite de haut en bas.
    fn column(&mut self, right: bool) -> Step {
        self.add(cam::DX, if right { 0x20u16.wrapping_neg() } else { 0x20 });
        let old = self.get(cam::DRAWN_X);
        let drawn = old.wrapping_add(if right { 0x20 } else { 0x20u16.wrapping_neg() });
        self.set(cam::DRAWN_X, drawn);
        if !right && pris!(self, old <= 0x20) {
            return Step::Return;
        }
        let d3 = self.row_offset(true);
        self.d4 = Some((0x1f, u32::MAX));
        let d2 = self.column_x(right);
        let mut a1 = hw::SCROLL3 + (d2 & 0x7e0) as u32;
        self.row_cursor(true);
        self.cursor_for_column(right);
        let mut a2 = self.strip_address();
        let offset = self.get(cam::CURSOR_OFFSET);
        a2 = a2.wrapping_add(offset as i16 as u32);
        a1 = a1.wrapping_add(d3 as i16 as u32);
        let wrap_offset = offset & 0xe0;
        let head = if right { 0x3292a } else { 0x329fc };
        for i in 0..COLUMN_CELLS_3 {
            self.x.mark(head);
            let cell = self.map_cell(a2, 0xf);
            a2 += 4;
            self.x.v.wr32(a1, cell);
            a1 += 4;
            if !pris!(self, a1 as u16 & 0x1f != 0) {
                // fin de colonne de la couche : bande suivante de la carte, colonne suivante de la couche
                let step = self.get(cam::ROW_STEP);
                self.add(cam::CURSOR_ROW, step);
                a2 = self.strip_address().wrapping_add(wrap_offset as i16 as u32);
                let here = a1 as u16 & 0x3fe0;
                a1 = a1.wrapping_sub(here as i16 as u32);
                a1 = a1.wrapping_add((here.wrapping_add(0x7e0) & 0x3fe0) as i16 as u32);
                if !right && !pris!(self, (a1 as i32) < hw::PALETTES as i32) {
                    a1 = a1.wrapping_sub(0x4000);
                }
            }
            pris!(self, i + 1 < COLUMN_CELLS_3);
        }
        Step::Continue
    }

    /// Ligne de 15 cases en bas (`down`) ou en haut de l'écran, écrite de gauche à droite.
    fn row(&mut self, down: bool) -> Step {
        self.add(cam::DY, if down { 0x20u16.wrapping_neg() } else { 0x20 });
        let old = self.get(cam::DRAWN_Y);
        let drawn = old.wrapping_add(if down { 0x20 } else { 0x20u16.wrapping_neg() });
        self.set(cam::DRAWN_Y, drawn);
        if !down {
            if pris!(self, old <= 0x20) {
                return Step::Return;
            }
            if pris!(self, drawn as i16 <= 0) {
                return Step::Continue;
            }
        }
        let d3 = self.row_offset(down);
        self.d4 = Some((0xe0, 0xffff));
        let mut column = self.column_x(false) & 0x7e0;
        let a1 = hw::SCROLL3 + d3 as u32;
        self.row_cursor(down);
        self.cursor_for_column(false);
        let mut a2 = self.strip_address();
        let mut offset = self.get(cam::CURSOR_OFFSET);
        let head = if down { 0x32ad0 } else { 0x32b9e };
        for i in 0..ROW_CELLS_3 {
            self.x.mark(head);
            let cell = self.map_cell(a2.wrapping_add(offset as u32), 1);
            self.x.v.wr32(a1.wrapping_add(column as i16 as u32), cell);
            column = column.wrapping_add(0x20) & 0x7e0;
            offset = offset.wrapping_add(0x20);
            if !pris!(self, offset & 0xe0 != 0) {
                offset = offset.wrapping_sub(0x100);
                self.add(cam::CURSOR_ROW, 1);
                a2 = self.strip_address();
            }
            pris!(self, i + 1 < ROW_CELLS_3);
        }
        Step::Continue
    }
}

fn camera2<'a>(x: Exec<'a>, c: &Cpu) -> Camera2<'a> {
    Camera2 { x, a5: c.a[5], cam: c.a[0], d4: None }
}

/// 0x328a8 : écarts cible - dessiné de la caméra 2 (A0), puis recopie des colonnes et lignes manquantes.
pub(crate) fn draw2(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut cam = camera2(Exec::real(b), c);
    cam.draw();
    if let Some((value, mask)) = cam.d4 {
        c.d[4] = c.d[4] & !mask | value;
    }
}



// ---------------------------------------------------------------------------------------------
// Décors à parallaxe : un objet se déplace d'une fraction du défilement de l'image
// ---------------------------------------------------------------------------------------------

/// Défilement courant (x, y) de la caméra, en virgule fixe 16.16, et celui de l'image précédente.
const SCROLL: [u32; 2] = [0x506, 0x50a];
const PREVIOUS_SCROLL: [u32; 2] = [0x524, 0x528];
/// Position (x, y) d'un objet en virgule fixe 16.16.
const OBJECT_X: u32 = 0x08;
const OBJECT_Y: u32 = 0x0c;
/// Position entière dessinée (x) d'un objet.
const OBJECT_DRAWN_X: u32 = 0x10;

/// Défilement de l'image sur l'axe 0 (x) ou 1 (y).
pub(crate) fn scroll_delta(b: &mut Cps2Bus, a5: u32, axis: usize) -> i32 {
    crate::mem::rd32(b, a5 + SCROLL[axis]).wrapping_sub(crate::mem::rd32(b, a5 + PREVIOUS_SCROLL[axis])) as i32
}

/// Ajoute `d` à une position de l'objet A0 ; rend les drapeaux de l'addition au registre de statut.
fn move_object(c: &mut Cpu, b: &mut Cps2Bus, field: u32, d: i32) {
    let at = c.a[0] + field;
    let v = crate::mem::rd32(b, at);
    let sum = crate::ops::add(c, d as u32, v, Size::Long);
    crate::mem::wr32(b, at, sum);
}

/// 0x33178 : le décor suit le quart du défilement en x et la moitié en y.
pub(crate) fn parallax_quarter(c: &mut Cpu, b: &mut Cps2Bus) {
    let (dx, dy) = (scroll_delta(b, c.a[5], 0), scroll_delta(b, c.a[5], 1));
    move_object(c, b, OBJECT_X, dx >> 2);
    move_object(c, b, OBJECT_Y, dy >> 1);
}

/// 0x33196 : le décor suit la moitié du défilement en x et les trois quarts en y.
pub(crate) fn parallax_three_quarters(c: &mut Cpu, b: &mut Cps2Bus) {
    let (dx, dy) = (scroll_delta(b, c.a[5], 0), scroll_delta(b, c.a[5], 1));
    move_object(c, b, OBJECT_X, dx >> 1);
    move_object(c, b, OBJECT_Y, (dy >> 1) + (dy >> 2));
}

/// 0x331e8 / 0x3326c : le décor avance de 6 / 3 pixels par image, sa position dessinée suit.
pub(crate) fn creep(c: &mut Cpu, b: &mut Cps2Bus, pixels: u16) {
    let old = crate::mem::rd16(b, c.a[0] + OBJECT_X);
    let x = crate::ops::add(c, pixels as u32, old as u32, Size::Word) as u16;
    crate::mem::wr16(b, c.a[0] + OBJECT_X, x);
    crate::mem::wr16(b, c.a[0] + OBJECT_DRAWN_X, x);
    crate::ops::logique(c, x as u32, Size::Word);
}

const fn leaf(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, cost: |_, _| 0, run, touches_sound: false, outputs: crate::regs::CCR, exact_cost: true }
}

pub const PARALLAX_QUARTER: Native = Native {
    cost: |_, b| crate::video::ligne_droite(b, 0x33178),
    ..leaf(0x33178, "decor_parallaxe_quart", |c, b| {
        parallax_quarter(c, b);
        rts(c, b)
    })
};
pub const PARALLAX_THREE_QUARTERS: Native = Native {
    cost: |_, b| crate::video::ligne_droite(b, 0x33196),
    ..leaf(0x33196, "decor_parallaxe_trois_quarts", |c, b| {
        parallax_three_quarters(c, b);
        rts(c, b)
    })
};
pub const CREEP_6: Native = Native {
    cost: |_, b| crate::video::ligne_droite(b, 0x331e8),
    ..leaf(0x331e8, "decor_avance_6", |c, b| {
        creep(c, b, 6);
        rts(c, b)
    })
};
pub const CREEP_3: Native = Native {
    cost: |_, b| crate::video::ligne_droite(b, 0x3326c),
    ..leaf(0x3326c, "decor_avance_3", |c, b| {
        creep(c, b, 3);
        rts(c, b)
    })
};

pub const NATIVES: &[Native] = &[FOLLOW_PLAYERS_Y, UPDATE_CAMERAS, RESET_CAMERAS, PARALLAX_QUARTER, PARALLAX_THREE_QUARTERS, CREEP_6, CREEP_3];

// ---------------------------------------------------------------------------------------------
// Suivi vertical des joueurs et mise à jour des deux caméras
// ---------------------------------------------------------------------------------------------

/// Enregistrements des joueurs (RAM de travail, A5 + 0x200 k) : octet « présent », PV (mot) et
/// profondeur (mot) ; la caméra suit l'étendue verticale des joueurs vivants.
const PLAYER_RECORD: [u32; 4] = [0x5de, 0x7de, 0x9de, 0xbde];
const PLAYER_HP: u32 = 0x62;
const PLAYER_Y: u32 = 0x10;
/// Marges (pixels) entre la caméra et le joueur le plus haut / le plus bas avant de la déplacer,
/// et pas maximal par image.
const FOLLOW_ABOVE: u32 = 0x30;
const FOLLOW_BELOW: u32 = 0x70;
const FOLLOW_STEP: u32 = 6;

/// 0x31616 : déplace la position verticale de la caméra A0 pour garder les joueurs vivants dans
/// la fenêtre `[caméra + 0x30, caméra + 0x70]` (6 pixels par image au plus). Transcrite
/// instruction par instruction : les routines qui suivent relisent D2, D4-D6 et les drapeaux.
fn follow_players_y(c: &mut Cpu, b: &mut Cps2Bus) {
    use crate::mem::{rd16, rd8, set_low_word, wr16, A5};
    use crate::ops::{self, cc};
    c.d[6] = 0; // profondeur maximale des joueurs
    c.d[5] = u32::MAX; // minimale
    for (k, &record) in PLAYER_RECORD.iter().enumerate() {
        let alive = rd8(b, A5 + record) != 0 && rd16(b, A5 + record + PLAYER_HP) != 0;
        if !alive {
            continue;
        }
        let y = rd16(b, A5 + record + PLAYER_Y);
        if k == 0 {
            set_low_word(&mut c.d[6], y);
            set_low_word(&mut c.d[5], y);
            continue;
        }
        if (c.d[6] as u16) < y {
            set_low_word(&mut c.d[6], y);
        }
        if (c.d[5] as u16) > y {
            set_low_word(&mut c.d[5], y);
        }
    }
    let cam_y = |c: &Cpu, b: &mut Cps2Bus| rd16(b, c.a[0] + cam::Y) as u32;
    // sub.w src,Dn
    let sub_to = |c: &mut Cpu, n: usize, src: u32| {
        let dst = c.d[n];
        let r = ops::sub(c, src, dst, Size::Word);
        set_low_word(&mut c.d[n], r as u16);
    };
    { let v = c.d[6] as u16; set_low_word(&mut c.d[4], v); }
    let y = cam_y(c, b);
    sub_to(c, 6, y);
    sub_to(c, 6, FOLLOW_BELOW);
    let step_down; // 0x316ca : D2 = D6 ; sinon D2 = D5 est déjà posé
    if c.cond(cc::LE) {
        // les joueurs ne sont pas trop bas : remonte la caméra s'ils sont trop haut
        let y = cam_y(c, b);
        sub_to(c, 5, y);
        sub_to(c, 5, FOLLOW_ABOVE);
        if c.cond(cc::PL) {
            return rts(c, b);
        }
        let d5 = c.d[5];
        ops::cmp(c, 0xfffa, d5, Size::Word);
        if !c.cond(cc::GE) {
            c.d[5] = (-(FOLLOW_STEP as i32)) as u32;
        }
        let (d5, y) = (c.d[5] & 0xffff, cam_y(c, b));
        let y = ops::add(c, d5, y, Size::Word) as u16;
        wr16(b, c.a[0] + cam::Y, y);
        { let v = c.d[4] as u16; set_low_word(&mut c.d[6], v); }
        let y = cam_y(c, b);
        sub_to(c, 6, y);
        sub_to(c, 6, FOLLOW_BELOW);
        if !c.cond(cc::GT) {
            return rts(c, b);
        }
        step_down = true;
    } else {
        let y = cam_y(c, b);
        sub_to(c, 5, y);
        sub_to(c, 5, FOLLOW_ABOVE);
        if c.cond(cc::LE) {
            return rts(c, b);
        }
        { let v = c.d[5] as u16; set_low_word(&mut c.d[2], v); }
        let (d6, d5) = (c.d[6] & 0xffff, c.d[5]);
        ops::cmp(c, d6, d5, Size::Word);
        step_down = !c.cond(cc::LE);
    }
    if step_down {
        { let v = c.d[6] as u16; set_low_word(&mut c.d[2], v); }
    }
    let d2 = c.d[2];
    ops::cmp(c, FOLLOW_STEP, d2, Size::Word);
    if !c.cond(cc::LE) {
        c.d[2] = FOLLOW_STEP;
    }
    let (d2, y) = (c.d[2] & 0xffff, cam_y(c, b));
    let y = ops::add(c, d2, y, Size::Word) as u16;
    wr16(b, c.a[0] + cam::Y, y);
    rts(c, b);
}

pub const FOLLOW_PLAYERS_Y: Native = Native {
    entry: 0x31616,
    name: "camera_suit_joueurs_y",
    cost: |_, _| 200,
    run: follow_players_y,
    touches_sound: false,
    outputs: crate::regs::ALL,
    exact_cost: false,
};

/// 0x33e20 : met à jour les deux caméras. La première mémorise sa cible précédente (+0x26, +0x2a)
/// avant que son script de mode (0x2ede0) la déplace ; la seconde est traitée par 0x32108.
fn update_cameras(c: &mut Cpu, b: &mut Cps2Bus) {
    use crate::mem::{rd32, wr32, A5};
    c.a[0] = A5 + CAMERA_1;
    let (x, y) = (rd32(b, c.a[0] + cam::X), rd32(b, c.a[0] + cam::Y));
    wr32(b, c.a[0] + cam::PREVIOUS_X, x);
    wr32(b, c.a[0] + cam::PREVIOUS_Y, y);
    crate::call(c, b, 0x2ede0);
    c.a[0] = A5 + CAMERA_2;
    crate::call(c, b, 0x32108);
    rts(c, b);
}

/// 0x33aa6 : caméras de la scène à leur position de départ. Les deux caméras (`A5+0x4fe`, `A5+0x54e`)
/// sont remises à zéro, le début de la couche 1 effacé (0x3525c) ; la scène (`A5+0x34`) choisit la caméra
/// en service (`A5+0x36`, table 0x33bdc) et la position de départ de la première (table 0x33c54 : x, y),
/// puis les caméras sont mises à jour (0x33e20).
fn reset_cameras(c: &mut Cpu, b: &mut Cps2Bus) {
    use crate::mem::{rd16, rd8, wr16, wr32, wr8, A5};
    const SERVICE: u32 = 0x33bdc;
    const STARTS: u32 = 0x33c54;
    for off in [0x54e, 0x556, 0x55a, 0x55e, 0x562, 0x56a, 0x4fe, 0x506, 0x50a, 0x50e, 0x512, 0x51a] {
        wr32(b, A5 + off, 0);
    }
    wr16(b, A5 + 0x566, 0);
    wr8(b, 0xff858b, 0);
    wr8(b, A5 + 0x53a, 0);
    crate::call(c, b, 0x3525c);
    let stage = rd8(b, A5 + 0x34) as u32;
    let service = rd8(b, SERVICE + stage);
    wr8(b, A5 + 0x36, service);
    for off in [0x508, 0x50c, 0x52e] {
        wr16(b, A5 + off, 0);
    }
    let start = STARTS + 4 * stage;
    let (x, y) = (rd16(b, start), rd16(b, start + 2));
    wr16(b, A5 + 0x506, x);
    wr16(b, A5 + 0x50a, y);
    wr32(b, A5 + 0x50e, 0);
    c.d[0] = 4 * stage;
    c.a[1] = STARTS;
    update_cameras(c, b);
}

pub const RESET_CAMERAS: Native = Native {
    entry: 0x33aa6,
    name: "cameras_depart",
    cost: |_, _| 300,
    run: reset_cameras,
    touches_sound: false,
    outputs: crate::regs::ALL,
    exact_cost: false,
};

pub const UPDATE_CAMERAS: Native = Native {
    entry: 0x33e20,
    name: "cameras_mise_a_jour",
    cost: |_, _| 100,
    run: update_cameras,
    touches_sound: false,
    outputs: crate::regs::ALL,
    exact_cost: false,
};
