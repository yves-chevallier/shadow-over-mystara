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
//! mode et l'initialisation de la scène (premier appel) restent du code d'origine.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8, A5};
use crate::objects::Obj;
use crate::video::hw;
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

/// Accès à la machine d'une caméra.
struct Exec<'a> {
    b: &'a mut Cps2Bus,
}

impl<'a> Exec<'a> {
    fn real(b: &'a mut Cps2Bus) -> Self {
        Exec { b }
    }
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

    fn get(&mut self, field: u32) -> u16 {
        rd16(self.x.b, self.cam + field)
    }
    fn set(&mut self, field: u32, v: u16) {
        wr16(self.x.b, self.cam + field, v);
    }
    fn add(&mut self, field: u32, d: u16) {
        let v = self.get(field).wrapping_add(d);
        self.set(field, v);
    }

    /// 0x2f81c : suite du script de mode : limites de la cible, puis recopie du décor.
    fn after_mode(&mut self) {
        if rd8(self.x.b, self.cam + cam::FROZEN) != 0 {
            return;
        }
        if self.get(cam::MODE) == 4 {
            return self.draw();
        }
        let (limit, mut x) = (self.get(cam::MAX_X) as i16, self.get(cam::X) as i16);
        if !(limit >= x) {
            self.set(cam::X, limit as u16);
            x = limit;
        }
        let behind = x.wrapping_sub(0x60);
        let min = self.get(cam::MIN_X) as i16;
        if !(behind <= min) {
            if !(rd8(self.x.b, self.cam + cam::HOLD_MIN) != 0) {
                self.set(cam::MIN_X, behind as u16);
            }
        } else if !(x >= min) {
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
                    if self.get(cam::DX) as i16 >= 0x10 {
                        self.column(true);
                    } else {
                        test = 1;
                    }
                }
                1 => {
                    if self.get(cam::DX) as i16 <= -0x10 {
                        if let Step::Return = self.column(false) {
                            return;
                        }
                    } else {
                        test = 2;
                    }
                }
                2 => {
                    if self.get(cam::DY) as i16 >= 0x10 {
                        self.row(true);
                    } else {
                        test = 3;
                    }
                }
                _ => {
                    if self.get(cam::DY) as i16 <= -0x10 {
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
        let mut y = self.get(cam::DRAWN_Y);
        if !(y & 0xf == 0) {
            y = y.wrapping_add(0x10);
        }
        layer_row_offset(y.wrapping_add(0x100), y)
    
    }

    /// 0x2fe30 : idem pour le haut de l'écran (y dessiné - 16).
    fn row_offset_up(&mut self) -> u16 {
        let y = self.get(cam::DRAWN_Y).wrapping_sub(0x10) & 0x3ff;
        layer_row_offset(y, y)
    
    }

    /// 0x2fe5c : colonne (décalage dans la couche) à gauche de l'écran : (x dessiné - 0x40) x 4.
    fn column_left(&mut self) -> u16 {
        self.get(cam::DRAWN_X).wrapping_sub(0x40).wrapping_mul(4)
    }

    /// 0x2fe6a : colonne à droite de l'écran : (x dessiné + 0x1e0) x 4.
    fn column_right(&mut self) -> u16 {
        self.get(cam::DRAWN_X).wrapping_add(0x1e0).wrapping_mul(4)
    }

    /// 0x2fe78 (bas) / 0x2fec0 (haut) : curseur de carte pour la ligne en bas / en haut de l'écran. La
    /// bande avance de `ROW_STEP` par 256 pixels ; le décalage dans la bande vient du quartet haut de y.
    fn row_cursor(&mut self, down: bool) {
        let mut y = self.get(cam::DRAWN_Y);
        // bas : y arrondi au multiple de 16 supérieur, distance mesurée 256 pixels plus bas ;
        // haut : 16 pixels au-dessus
        let (mut distance, nibble_source) = if down {
            if !(y & 0xf == 0) {
                y = y.wrapping_add(0x10);
            }
            (y.wrapping_add(0x100), y)
        } else {
            let above = y.wrapping_sub(0x10);
            (above, above)
        };
        let mut step = self.get(cam::ROW_STEP);
        if !(distance as i16 >= 0) {
            distance = distance.wrapping_neg();
            step = step.wrapping_neg();
        }
        let strips = distance >> 8;
        self.set(cam::CURSOR_ROW, step.wrapping_mul(strips).wrapping_neg());
        self.set(cam::CURSOR_OFFSET, 0x3c - ((nibble_source & 0xf0) >> 2));
    
    }

    /// 0x2ff1c (colonne droite) / 0x2fefa (colonne gauche) : avance le curseur de la colonne ajoutée.
    fn cursor_for_column(&mut self, right: bool) {
        let x = self.get(cam::DRAWN_X);
        let column = if right { x.wrapping_add(0x1e0) } else { x.wrapping_sub(0x40) } & 0xfff0;
        self.add(cam::CURSOR_OFFSET, (column & 0xff).wrapping_mul(4));
        let strip = if right { column >> 8 } else { (column >> 8) as u8 as i8 as u16 };
        self.add(cam::CURSOR_ROW, strip);
    
    }

    /// 0x2ff66 : adresse en ROM de la bande de la carte désignée par le curseur.
    fn strip_address(&mut self) -> u32 {
        let stage = rd8(self.x.b, self.a5 + STAGE) as u32;
        let table = crate::mem::prog16(self.x.b, ROW_TABLES + 4 * stage) as u32;
        let table = table << 16 | crate::mem::prog16(self.x.b, ROW_TABLES + 4 * stage + 2) as u32;
        let strip = self.get(cam::CURSOR_ROW).wrapping_mul(2) as i16 as u32;
        let first = rd16(self.x.b, table.wrapping_add(strip)) as u32;
        MAP + (first << 10)
    
    }

    // --- recopie ---

    /// Case de la carte à la position `at` (adresse dans la table d'indices), au format de la couche.
    fn map_cell(&mut self, at: u32) -> u32 {
        let off = at.wrapping_sub(MAP);
        let bank = rd32(self.x.b, BANKS + (off >> 16 & 0xc));
        let c = off >> 2;
        let index = rd16(self.x.b, MAP + (c & 0xe) + (c >> 1 & 0x3fff0)) as u32;
        let sub = (off as u16 >> 3 & 8) as u32 + (off & 4);
        rd32(self.x.b, bank.wrapping_add(index << 4).wrapping_add(sub))
    }

    /// Écrit la case `cell` en `at` : tuile de la carte, ou tuile de terrain en mode débogage.
    fn put_cell(&mut self, at: u32, cell: u32) {
        let terrain_view = rd8(self.x.b, self.a5 + TERRAIN_VIEW) != 0;
        if !(!terrain_view) {
            let attribute = cell & 0xff80;
            let number = (attribute >> 7) as u16;
            let tile = number.wrapping_add(TERRAIN_TILES[(number & 0x40 != 0) as usize]);
            wr16(self.x.b, at, tile);
            wr16(self.x.b, at + 2, attribute as u16);
        } else {
            wr32(self.x.b, at, cell.wrapping_add(MAP_TILE_OFFSET));
        }
    }

    /// Colonne de 19 cases à droite (`right`) ou à gauche de l'écran, écrite de haut en bas.
    fn column(&mut self, right: bool) -> Step {
        self.add(cam::DX, if right { 0x10u16.wrapping_neg() } else { 0x10 });
        let drawn = self.get(cam::DRAWN_X).wrapping_add(if right { 0x10 } else { 0x10u16.wrapping_neg() });
        self.set(cam::DRAWN_X, drawn);
        if !right && ((drawn as i16) < 0) {
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
        for _ in 0..COLUMN_CELLS {
            let cell = self.map_cell(a2);
            a2 += 4;
            self.put_cell(a1, cell);
            a1 += 4;
            if !(a1 as u16 & 0x3f != 0) {
                // fin de colonne de la couche : bande suivante de la carte, colonne suivante de la couche
                let step = self.get(cam::ROW_STEP);
                self.add(cam::CURSOR_ROW, step);
                a2 = self.strip_address().wrapping_add(wrap_offset as i16 as u32);
                let next = (a1 as u16).wrapping_add(0xfc0) & 0x3fc0;
                a1 = hw::SCROLL2.wrapping_add(next as i16 as u32);
            }
        }
        Step::Continue
    }

    /// Ligne de 35 cases en bas (`down`) ou en haut de l'écran, écrite de gauche à droite.
    fn row(&mut self, down: bool) -> Step {
        self.add(cam::DY, if down { 0x10u16.wrapping_neg() } else { 0x10 });
        let drawn = self.get(cam::DRAWN_Y).wrapping_add(if down { 0x10 } else { 0x10u16.wrapping_neg() });
        self.set(cam::DRAWN_Y, drawn);
        if !down && ((drawn as i16) < 0) {
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
        for _ in 0..ROW_CELLS {
            let cell = self.map_cell(a2.wrapping_add(offset as u32));
            self.put_cell(a1.wrapping_add(column as i16 as u32), cell);
            column = column.wrapping_add(0x40) & 0xfc0;
            offset = offset.wrapping_add(0x40);
            if !(offset & 0x3c0 != 0) {
                offset = offset.wrapping_sub(0x400);
                self.add(cam::CURSOR_ROW, 1);
                a2 = self.strip_address();
            }
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

fn camera(b: &mut Cps2Bus) -> Camera1<'_> {
    Camera1 { x: Exec::real(b), a5: A5, cam: A5 + CAMERA_1, d4: None }
}

/// D4 que laisse la recopie d'une colonne ou d'une ligne de décor : valeur et bits remplacés (`D4 & !masque |
/// valeur`).
pub type RestesD4 = Option<(u32, u32)>;

/// 0x2f81c : suite du script de mode de la caméra 1, après le script.
pub(crate) fn after_mode(b: &mut Cps2Bus) -> RestesD4 {
    let mut cam = camera(b);
    cam.after_mode();
    cam.d4.map(|v| (v, u32::MAX))
}

/// 0x2f86e : écarts cible - dessiné de la caméra 1, puis recopie des colonnes et lignes manquantes.
pub(crate) fn draw1(b: &mut Cps2Bus) -> RestesD4 {
    let mut cam = camera(b);
    cam.draw();
    cam.d4.map(|v| (v, u32::MAX))
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

struct Camera2<'a> {
    x: Exec<'a>,
    a5: u32,
    cam: u32,
    /// D4 laissé par la dernière recopie : 0x1f (long) pour une colonne, 0xe0 (mot bas) pour une ligne.
    d4: Option<(u32, u32)>,
}

impl Camera2<'_> {

    fn get(&mut self, field: u32) -> u16 {
        rd16(self.x.b, self.cam + field)
    }
    fn set(&mut self, field: u32, v: u16) {
        wr16(self.x.b, self.cam + field, v);
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
                    if self.get(cam::DX) as i16 >= 0x20 {
                        self.column(true);
                    } else {
                        test = 1;
                    }
                }
                1 => {
                    if self.get(cam::DX) as i16 <= -0x20 {
                        if let Step::Return = self.column(false) {
                            return;
                        }
                    } else {
                        test = 2;
                    }
                }
                2 => {
                    if self.get(cam::DY) as i16 >= 0x20 {
                        self.row(true);
                    } else {
                        test = 3;
                    }
                }
                _ => {
                    if self.get(cam::DY) as i16 <= -0x20 {
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
        let mut y = self.get(cam::DRAWN_Y);
        let page_source = if down {
            if !(y & 0x1f == 0) {
                y = y.wrapping_add(0x20);
            }
            y.wrapping_add(0x100)
        } else {
            y = y.wrapping_sub(0x20) & 0x7ff;
            y
        };
        0x3800u16.wrapping_sub((page_source & 0x700) << 3).wrapping_add(0x1c - ((y & 0xe0) >> 3))
    
    }

    /// 0x32c8a / 0x32c96 : colonne (x - 0x20 ou x + 0x180) à gauche / à droite de l'écran.
    fn column_x(&mut self, right: bool) -> u16 {
        let x = self.get(cam::DRAWN_X);
        if right { x.wrapping_add(0x180) } else { x.wrapping_sub(0x20) }
    
    }

    /// 0x32ca2 (bas) / 0x32cea (haut) : curseur de carte pour la ligne du bas / du haut.
    fn row_cursor(&mut self, down: bool) {
        let mut y = self.get(cam::DRAWN_Y);
        let mut step = self.get(cam::ROW_STEP);
        let (distance, nibble_source) = if down {
            if !(y & 0x1f == 0) {
                y = y.wrapping_add(0x20);
            }
            let mut distance = y.wrapping_add(0x100);
            if !(distance as i16 >= 0) {
                distance = distance.wrapping_neg();
                step = step.wrapping_neg();
            }
            (distance, y)
        } else {
            let mut above = y.wrapping_sub(0x20);
            if !(above as i16 >= 0) {
                above = 0;
            }
            (above, above)
        };
        let strips = distance >> 8;
        self.set(cam::CURSOR_ROW, step.wrapping_mul(strips).wrapping_neg());
        self.set(cam::CURSOR_OFFSET, 0x1c - ((nibble_source & 0xe0) >> 3));
    
    }

    /// 0x32d3c (colonne droite) / 0x32d22 (colonne gauche) : avance le curseur de la colonne ajoutée ;
    /// le décalage s'additionne sur l'octet bas seulement (sans retenue).
    fn cursor_for_column(&mut self, right: bool) {
        let x = self.get(cam::DRAWN_X);
        let column = if right { x.wrapping_add(0x180) } else { x.wrapping_sub(0x20) } & 0xffe0;
        let low = rd8(self.x.b, self.cam + cam::CURSOR_OFFSET + 1).wrapping_add(column as u8);
        wr8(self.x.b, self.cam + cam::CURSOR_OFFSET + 1, low);
        let strip = if right { column >> 8 } else { (column >> 8) as u8 as i8 as u16 };
        self.add(cam::CURSOR_ROW, strip);
    
    }

    /// 0x32d54 : adresse en ROM de la bande de la carte désignée par le curseur.
    fn strip_address(&mut self) -> u32 {
        let stage = rd8(self.x.b, self.a5 + STAGE) as u32;
        let table = (crate::mem::prog16(self.x.b, ROW_TABLES_3 + 4 * stage) as u32) << 16 | crate::mem::prog16(self.x.b, ROW_TABLES_3 + 4 * stage + 2) as u32;
        let strip = self.get(cam::CURSOR_ROW).wrapping_mul(2) as i16 as u32;
        let first = rd16(self.x.b, table.wrapping_add(strip)) as u32;
        MAP_3 + (first << 8)
    
    }

    // --- recopie ---

    /// Case de la carte à la position `at` ; `banks` : bits d'adresse qui choisissent la banque.
    fn map_cell(&mut self, at: u32, banks: u32) -> u32 {
        let off = at.wrapping_sub(MAP_3);
        let bank = rd32(self.x.b, BANKS_3 + ((off >> 16 & banks) << 2));
        let c = off >> 2;
        let index = rd16(self.x.b, MAP_3 + (c & 6) + (c >> 1 & !7)) as u32;
        let sub = (off as u16 >> 2 & 8) as u32 + (off & 4);
        rd32(self.x.b, bank.wrapping_add(index << 4).wrapping_add(sub)).wrapping_add(MAP_TILE_OFFSET_3)
    }

    /// Colonne de 11 cases à droite (`right`) ou à gauche de l'écran, écrite de haut en bas.
    fn column(&mut self, right: bool) -> Step {
        self.add(cam::DX, if right { 0x20u16.wrapping_neg() } else { 0x20 });
        let old = self.get(cam::DRAWN_X);
        let drawn = old.wrapping_add(if right { 0x20 } else { 0x20u16.wrapping_neg() });
        self.set(cam::DRAWN_X, drawn);
        if !right && (old <= 0x20) {
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
        for _ in 0..COLUMN_CELLS_3 {
            let cell = self.map_cell(a2, 0xf);
            a2 += 4;
            wr32(self.x.b, a1, cell);
            a1 += 4;
            if !(a1 as u16 & 0x1f != 0) {
                // fin de colonne de la couche : bande suivante de la carte, colonne suivante de la couche
                let step = self.get(cam::ROW_STEP);
                self.add(cam::CURSOR_ROW, step);
                a2 = self.strip_address().wrapping_add(wrap_offset as i16 as u32);
                let here = a1 as u16 & 0x3fe0;
                a1 = a1.wrapping_sub(here as i16 as u32);
                a1 = a1.wrapping_add((here.wrapping_add(0x7e0) & 0x3fe0) as i16 as u32);
                if !right && !((a1 as i32) < hw::PALETTES as i32) {
                    a1 = a1.wrapping_sub(0x4000);
                }
            }
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
            if old <= 0x20 {
                return Step::Return;
            }
            if drawn as i16 <= 0 {
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
        for _ in 0..ROW_CELLS_3 {
            let cell = self.map_cell(a2.wrapping_add(offset as u32), 1);
            wr32(self.x.b, a1.wrapping_add(column as i16 as u32), cell);
            column = column.wrapping_add(0x20) & 0x7e0;
            offset = offset.wrapping_add(0x20);
            if !(offset & 0xe0 != 0) {
                offset = offset.wrapping_sub(0x100);
                self.add(cam::CURSOR_ROW, 1);
                a2 = self.strip_address();
            }
        }
        Step::Continue
    }
}

/// 0x328a8 : écarts cible - dessiné de la caméra 2, puis recopie des colonnes et lignes manquantes.
pub(crate) fn draw2(b: &mut Cps2Bus) -> RestesD4 {
    let mut cam = Camera2 { x: Exec::real(b), a5: A5, cam: A5 + CAMERA_2, d4: None };
    cam.draw();
    cam.d4
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

/// Ajoute `d` à une position de l'objet `o`. Rend la somme (les drapeaux de l'addition : `move_object_68k`).
fn move_object(b: &mut Cps2Bus, o: Obj, field: u32, d: i32) -> (u32, u32) {
    let at = o.0 + field;
    let v = crate::mem::rd32(b, at);
    crate::mem::wr32(b, at, v.wrapping_add(d as u32));
    (d as u32, v)
}

/// `move_object` avec les drapeaux de l'addition (`add.l`).
fn move_object_68k(c: &mut Cpu, b: &mut Cps2Bus, field: u32, d: i32) {
    let (d, v) = move_object(b, Obj(c.a[0]), field, d);
    crate::ops::add(c, d, v, Size::Long);
}

/// Défilement de l'image pour un décor qui suit `(fx, fy)` du défilement (x, y).
fn parallaxe(b: &mut Cps2Bus, fx: fn(i32) -> i32, fy: fn(i32) -> i32) -> (i32, i32) {
    (fx(scroll_delta(b, A5, 0)), fy(scroll_delta(b, A5, 1)))
}
const QUART: (fn(i32) -> i32, fn(i32) -> i32) = (|dx| dx >> 2, |dy| dy >> 1);
const TROIS_QUARTS: (fn(i32) -> i32, fn(i32) -> i32) = (|dx| dx >> 1, |dy| (dy >> 1) + (dy >> 2));

/// 0x33178 : le décor `o` suit le quart du défilement en x et la moitié en y.
pub(crate) fn parallax_quarter(b: &mut Cps2Bus, o: Obj) {
    let (dx, dy) = parallaxe(b, QUART.0, QUART.1);
    move_object(b, o, OBJECT_X, dx);
    move_object(b, o, OBJECT_Y, dy);
}

/// 0x33196 : le décor `o` suit la moitié du défilement en x et les trois quarts en y.
pub(crate) fn parallax_three_quarters(b: &mut Cps2Bus, o: Obj) {
    let (dx, dy) = parallaxe(b, TROIS_QUARTS.0, TROIS_QUARTS.1);
    move_object(b, o, OBJECT_X, dx);
    move_object(b, o, OBJECT_Y, dy);
}

/// Natives des deux parallaxes (objet A0 ; drapeaux de la dernière addition).
fn parallaxe_68k(c: &mut Cpu, b: &mut Cps2Bus, f: (fn(i32) -> i32, fn(i32) -> i32)) {
    let (dx, dy) = parallaxe(b, f.0, f.1);
    move_object_68k(c, b, OBJECT_X, dx);
    move_object_68k(c, b, OBJECT_Y, dy);
}

/// 0x331e8 / 0x3326c : le décor `o` avance de 6 / 3 pixels par image, sa position dessinée suit. Rend la nouvelle
/// position (drapeaux de l'original : `creep_68k`).
pub(crate) fn creep(b: &mut Cps2Bus, o: Obj, pixels: u16) -> u16 {
    let x = crate::mem::rd16(b, o.0 + OBJECT_X).wrapping_add(pixels);
    crate::mem::wr16(b, o.0 + OBJECT_X, x);
    crate::mem::wr16(b, o.0 + OBJECT_DRAWN_X, x);
    x
}

fn creep_68k(c: &mut Cpu, b: &mut Cps2Bus, pixels: u16) {
    let old = crate::mem::rd16(b, c.a[0] + OBJECT_X);
    let x = creep(b, Obj(c.a[0]), pixels);
    // add.w (X), puis move.w (N, Z, V, C)
    crate::ops::add(c, pixels as u32, old as u32, Size::Word);
    crate::ops::logique(c, x as u32, Size::Word);
}

const fn leaf(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, run: crate::Corps::Processeur(run), touches_sound: false, outputs: crate::regs::CCR }
}

pub const PARALLAX_QUARTER: Native = Native {
    ..leaf(0x33178, "decor_parallaxe_quart", |c, b| {
        parallaxe_68k(c, b, QUART);
        rts(c, b)
    })
};
pub const PARALLAX_THREE_QUARTERS: Native = Native {
    ..leaf(0x33196, "decor_parallaxe_trois_quarts", |c, b| {
        parallaxe_68k(c, b, TROIS_QUARTS);
        rts(c, b)
    })
};
pub const CREEP_6: Native = Native {
    ..leaf(0x331e8, "decor_avance_6", |c, b| {
        creep_68k(c, b, 6);
        rts(c, b)
    })
};
pub const CREEP_3: Native = Native {
    ..leaf(0x3326c, "decor_avance_3", |c, b| {
        creep_68k(c, b, 3);
        rts(c, b)
    })
};

pub const NATIVES: &[Native] = &[FOLLOW_PLAYERS_Y, UPDATE_CAMERAS, RESET_CAMERAS, RETURN_CAMERAS, PARALLAX_QUARTER, PARALLAX_THREE_QUARTERS, CREEP_6, CREEP_3];

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
    run: crate::Corps::Processeur(follow_players_y),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

/// 0x33e20 : met à jour les deux caméras. La première mémorise sa cible précédente (+0x26, +0x2a)
/// avant que son script de mode (0x2ede0) la déplace ; la seconde est traitée par 0x32108.
/// Rend le D4 que laissent les recopies de décor des deux caméras (dans l'ordre).
pub fn update_cameras(b: &mut Cps2Bus) -> [RestesD4; 2] {
    let camera = A5 + CAMERA_1;
    let (x, y) = (rd32(b, camera + cam::X), rd32(b, camera + cam::Y));
    wr32(b, camera + cam::PREVIOUS_X, x);
    wr32(b, camera + cam::PREVIOUS_Y, y);
    [crate::camera_modes::update_camera_1(b), crate::camera_modes::update_camera_2(b)]
}

/// `update_cameras` avec les registres de l'original : D4 des recopies de décor, A0 = la caméra 2.
pub fn update_cameras_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    for (v, m) in update_cameras(b).into_iter().flatten() {
        c.d[4] = c.d[4] & !m | v;
    }
    c.a[0] = A5 + CAMERA_2;
}

/// Native de 0x33e20.
fn update_cameras_native(c: &mut Cpu, b: &mut Cps2Bus) {
    update_cameras_68k(c, b);
    rts(c, b);
}

/// Table (par scène) de la caméra en service, et positions de départ (x, y) de la première caméra.
const SERVICE: u32 = 0x33bdc;
const STARTS: u32 = 0x33c54;

/// 0x33aa6 : caméras de la scène à leur position de départ. Les deux caméras (`A5+0x4fe`, `A5+0x54e`)
/// sont remises à zéro, le début de la couche 1 effacé (0x3525c) ; la scène (`A5+0x34`) choisit la caméra
/// en service (`A5+0x36`, table 0x33bdc) et la position de départ de la première (table 0x33c54 : x, y),
/// puis les caméras sont mises à jour (0x33e20, par l'appelant : `cameras_depart` ne fait que la remise à zéro).
pub fn cameras_depart(b: &mut Cps2Bus) {
    for off in [0x54e, 0x556, 0x55a, 0x55e, 0x562, 0x56a, 0x4fe, 0x506, 0x50a, 0x50e, 0x512, 0x51a] {
        wr32(b, A5 + off, 0);
    }
    wr16(b, A5 + 0x566, 0);
    wr8(b, 0xff858b, 0);
    wr8(b, A5 + 0x53a, 0);
    crate::video::effacer_debut_couche1(b);
    let stage = rd8(b, A5 + STAGE) as u32;
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
}

/// Native de 0x33aa6 : registres de l'effacement de la couche 1, D0 = 4 x scène, A1 = table des départs, A0 = la
/// caméra 2.
fn reset_cameras(c: &mut Cpu, b: &mut Cps2Bus) {
    cameras_depart(b);
    crate::video::restes_effacer_debut_couche1(c);
    c.d[0] = 4 * rd8(b, A5 + STAGE) as u32;
    c.a[1] = STARTS;
    update_cameras_68k(c, b);
    rts(c, b);
}

pub const RESET_CAMERAS: Native = Native {
    entry: 0x33aa6,
    name: "cameras_depart",
    run: crate::Corps::Processeur(reset_cameras),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

/// 0x33b24 : caméras au retour d'une zone annexe (porte de sortie SET 7 et portes 27, 29, avant qu'elles ne
/// rétablissent la caméra mémorisée). La scène (`A5+0x34`) choisit la caméra en service (`A5+0x36`, table
/// 0x33bdc), une limite (`A5+0x532`), le mot de contrôle vidéo 0x400004 et les couches affichées (0xbc6),
/// puis les caméras sont remises à leur position initiale (0x200) et leurs bornes rechargées (tables par
/// scène, espace données), avant leur mise à jour (0x33e20, par l'appelant).
pub fn cameras_retour(b: &mut Cps2Bus) {
    use crate::camera_modes::tables::{
        FROZEN as MODES, HOLD_MIN as PRIORITIES, LAYERS, MAX_X_1 as LIMITS, OUTPUT as VIDEO_CONTROL,
        ROW_STEP_1 as BOUNDS_A, ROW_STEP_2 as BOUNDS_B,
    };
    use crate::camera_modes::OUTPUT_PORT as VIDEO_CONTROL_PORT;
    let stage = rd8(b, A5 + STAGE) as u32;
    let service = rd8(b, SERVICE + stage);
    wr8(b, A5 + 0x36, service);
    let limit = rd16(b, LIMITS + 2 * stage);
    wr16(b, A5 + 0x532, limit);
    let control = rd16(b, VIDEO_CONTROL + 2 * stage);
    wr16(b, VIDEO_CONTROL_PORT, control);
    let couches = rd16(b, LAYERS + 2 * stage);
    wr16(b, A5 + 0x14, 0);
    crate::video::activer_couches(b, couches);
    let priority = rd8(b, PRIORITIES + stage);
    wr8(b, A5 + 0x53b, priority);
    wr16(b, A5 + 0x4fe, 0x200);
    wr16(b, A5 + 0x51e, 0);
    wr32(b, A5 + 0x548, 0);
    wr16(b, A5 + 0x54c, 0);
    wr8(b, A5 + 0x53a, 0);
    wr32(b, A5 + 0x7fb2, 0);
    wr16(b, A5 + 0x54e, 0x200);
    wr16(b, A5 + 0x56e, 0);
    let x = rd16(b, A5 + 0x506);
    wr16(b, A5 + 0x52c, x);
    let bound = rd32(b, BOUNDS_A + 4 * stage);
    let v = rd16(b, bound);
    wr16(b, A5 + 0x53e, v);
    let bound = rd32(b, BOUNDS_B + 4 * stage);
    let v = rd16(b, bound);
    wr16(b, A5 + 0x58e, v);
    let mode = rd8(b, MODES + stage);
    wr8(b, A5 + 0x53c, mode);
}

/// Native de 0x33b24 : D0 = scène, A1 = table du contrôle vidéo, A4 = table des modes, A0 = la caméra 2.
fn return_cameras(c: &mut Cpu, b: &mut Cps2Bus) {
    cameras_retour(b);
    c.d[0] = rd8(b, A5 + STAGE) as u32;
    c.a[1] = crate::camera_modes::tables::OUTPUT;
    c.a[4] = crate::camera_modes::tables::FROZEN;
    update_cameras_68k(c, b);
    rts(c, b);
}

pub const RETURN_CAMERAS: Native = Native {
    entry: 0x33b24,
    name: "cameras_retour",
    run: crate::Corps::Processeur(return_cameras),
    touches_sound: false,
    // appelée par les portes (natives), qui ne relisent aucun registre
    outputs: 0,
};

pub const UPDATE_CAMERAS: Native = Native {
    entry: 0x33e20,
    name: "cameras_mise_a_jour",
    run: crate::Corps::Processeur(update_cameras_native),
    touches_sound: false,
    outputs: crate::regs::ALL,
};
