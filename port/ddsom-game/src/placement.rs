//! Placement d'un joueur sur un sol libre et palettes d'effets des joueurs (sous-système X2).

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::mem::{prog16, rd16, rd32, rd8, rts, set_low_word, wr16, wr32, A5};
use crate::objects::Obj;
use crate::{call, regs, Native};

/// Numéro de terrain au point (D0, D1) -> D2 (`video::terrain`).
use crate::terrain::TERRAIN_ATTRIBUTE as TERRAIN_AT;
/// Terrains libres : numéros inférieurs à 4.
const FREE_TERRAIN: u16 = 4;
/// Position de la caméra des joueurs (x, profondeur) ; `A5+0x36` choisit la seconde caméra.
const CAMERA: (u32, u32) = (0x506, 0x50a);
const ALT_CAMERA: (u32, u32) = (0x556, 0x55a);
/// Décalages (mots, code) du point de départ de la recherche, par joueur (`+2`).
const START_X: u32 = 0x3658;
const START_Y: u32 = 0x3660;
/// Dernier point de départ (mots de la RAM).
const LAST_X: u32 = A5 - 0x719a;
const LAST_Y: u32 = A5 - 0x7198;

fn camera(b: &mut Cps2Bus) -> (u16, u16) {
    let (x, y) = if rd8(b, A5 + 0x36) != 0 { ALT_CAMERA } else { CAMERA };
    (rd16(b, A5 + x), rd16(b, A5 + y))
}

/// 0x3668 : place le joueur A0 à la résurrection. Depuis un point de départ propre au joueur, cherche un
/// terrain libre en essayant, comme l'original, le point de départ puis le même point décalé de 16
/// pixels (en profondeur, puis en x) ; à défaut, le centre de l'écran (+192, +80). La hauteur fait
/// tomber le joueur du haut de l'écran (caméra + 256 - profondeur).
fn place_on_free_ground(c: &mut Cpu, b: &mut Cps2Bus) {
    place_on_free_ground_regs(c, b);
    rts(c, b);
}

/// 0x3668 appelée de Rust pour le joueur `o` : laisse les registres de l'original (D0, D1 : hauteur et profondeur
/// trouvées, D2 : terrain, D5, D6 : compteurs de la recherche, A0 = `o`).
pub fn place_on_free_ground_68k(c: &mut Cpu, b: &mut Cps2Bus, o: Obj) {
    c.a[0] = o.0;
    place_on_free_ground_regs(c, b);
}

fn place_on_free_ground_regs(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    let player = 2 * rd8(b, me + 2) as u32;
    let (cam_x, _) = camera(b);
    let x0 = cam_x.wrapping_add(prog16(b, START_X + player));
    wr16(b, LAST_X, x0);
    let mut found = None;
    'search: for i in 0..5u16 {
        c.d[5] = (4 - i) as u32;
        set_low_word(&mut c.d[0], if i == 0 { x0 } else { x0.wrapping_add(16) });
        let (_, cam_y) = camera(b);
        let y0 = cam_y.wrapping_add(prog16(b, START_Y + player));
        wr16(b, LAST_Y, y0);
        for j in 0..10u16 {
            c.d[6] = (9 - j) as u32;
            set_low_word(&mut c.d[1], if j == 0 { y0 } else { y0.wrapping_add(16) });
            call(c, b, TERRAIN_AT);
            if (c.d[2] as u16) < FREE_TERRAIN {
                found = Some((c.d[0] as u16, c.d[1] as u16));
                break 'search;
            }
        }
        set_low_word(&mut c.d[6], 0xffff);
    }
    let (x, y) = found.unwrap_or_else(|| {
        set_low_word(&mut c.d[5], 0xffff);
        let (cam_x, cam_y) = camera(b);
        (cam_x.wrapping_add(192), cam_y.wrapping_add(80))
    });
    set_low_word(&mut c.d[1], y);
    wr16(b, me + 8, x);
    wr16(b, me + 0x10, y);
    let (_, cam_y) = camera(b);
    let height = cam_y.wrapping_add(256).wrapping_sub(y);
    set_low_word(&mut c.d[0], height);
    wr16(b, me + 0x0c, height);
}

/// Palettes de 16 couleurs (données) copiées dans la palette 0x91c1e0 par 0x3af8..0x3b14.
const EFFECT_PALETTES: u32 = 0x25a640;
const EFFECT_PALETTE_RAM: u32 = 0x91c1e0;

/// Palettes d'effet des joueurs : décalage dans `EFFECT_PALETTES` (entrées 0x3af8, 0x3afe, 0x3b04, 0x3b0c, 0x3b14).
pub mod effect_palette {
    /// Éclaircie (0x3af8).
    pub const LIGHT: u16 = 0x20;
    /// Couleurs ordinaires (0x3afe).
    pub const NORMAL: u16 = 0;
    pub const P360: u16 = 0x360;
    pub const P380: u16 = 0x380;
    /// Rouge (0x3b14).
    pub const RED: u16 = 0x3a0;
}

/// 0x3b1c : copie la palette d'effet de décalage `offset` (32 octets) ; rend les huit longs copiés et la source
/// suivante.
pub fn load_effect_palette(b: &mut Cps2Bus, offset: u16) -> ([u32; 8], u32) {
    let from = EFFECT_PALETTES.wrapping_add(offset as i16 as i32 as u32);
    let longs: [u32; 8] = std::array::from_fn(|i| rd32(b, from + 4 * i as u32));
    for (i, &v) in longs.iter().enumerate() {
        wr32(b, EFFECT_PALETTE_RAM + 4 * i as u32, v);
    }
    (longs, from + 32)
}

/// Registres que laisse 0x3b1c (étiquette `DDSOM_POISON` : `placement`).
const REGS_EFFECT_PALETTE: u32 = regs::d(0) | regs::d(1) | regs::d(2) | regs::d(3) | regs::d(4) | regs::d(5) | regs::d(6) | regs::a(1) | regs::a(2) | regs::a(3);

/// `load_effect_palette` avec les registres de l'original : D0-D6 et A3 gardent les huit longs copiés, A1 et A2 la
/// destination et la source suivante, comme le `movem` de l'original.
fn load_effect_palette_regs(c: &mut Cpu, b: &mut Cps2Bus, offset: u16) {
    let (longs, next) = load_effect_palette(b, offset);
    c.d[..7].copy_from_slice(&longs[..7]);
    c.a[3] = longs[7];
    c.a[1] = EFFECT_PALETTE_RAM;
    c.a[2] = next;
}

/// Adaptateur de `load_effect_palette` pour les appelants Rust pas encore prouvés indépendants des registres.
#[track_caller]
pub fn load_effect_palette_68k(c: &mut Cpu, b: &mut Cps2Bus, offset: u16) {
    load_effect_palette_regs(c, b, offset);
    crate::diag::registres_morts(c, "placement", REGS_EFFECT_PALETTE);
}

fn effect_palette_native(c: &mut Cpu, b: &mut Cps2Bus, offset: u16) {
    load_effect_palette_regs(c, b, offset);
    rts(c, b);
}

const fn palette(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, run, touches_sound: false, outputs: REGS_EFFECT_PALETTE }
}

pub const NATIVES: &[Native] = &[
    Native {
        entry: 0x3668,
        name: "joueur_place_au_sol",
        run: place_on_free_ground,
        touches_sound: false,
        outputs: regs::d(0) | regs::d(1) | regs::d(2) | regs::d(5) | regs::d(6),
    },
    palette(0x3af8, "palette_effet_20", |c, b| effect_palette_native(c, b, effect_palette::LIGHT)),
    palette(0x3afe, "palette_effet_0", |c, b| effect_palette_native(c, b, effect_palette::NORMAL)),
    palette(0x3b04, "palette_effet_360", |c, b| effect_palette_native(c, b, effect_palette::P360)),
    palette(0x3b0c, "palette_effet_380", |c, b| effect_palette_native(c, b, effect_palette::P380)),
    palette(0x3b14, "palette_effet_3a0", |c, b| effect_palette_native(c, b, effect_palette::RED)),
];
