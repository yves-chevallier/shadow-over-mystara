//! Placement d'un joueur sur un sol libre et palettes d'effets des joueurs (sous-système X2).

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::mem::{prog16, rd16, rd32, rd8, rts, set_low_word, wr16, wr32, A5};
use crate::{call, regs, Native};

/// Numéro de terrain au point (D0, D1) -> D2 (`video::terrain`).
const TERRAIN_AT: u32 = 0x24b0;
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
    rts(c, b);
}

/// Palettes de 16 couleurs (données) copiées dans la palette 0x91c1e0 par 0x3af8..0x3b14.
const EFFECT_PALETTES: u32 = 0x25a640;
const EFFECT_PALETTE_RAM: u32 = 0x91c1e0;

/// 0x3b1c : copie la palette d'effet de décalage D0 (32 octets) ; D0-D6 et A3 gardent les huit longs
/// copiés, A1 et A2 la destination et la source suivante, comme le `movem` de l'original.
fn load_effect_palette(c: &mut Cpu, b: &mut Cps2Bus, offset: u16) {
    let from = EFFECT_PALETTES.wrapping_add(offset as i16 as i32 as u32);
    let longs: [u32; 8] = std::array::from_fn(|i| rd32(b, from + 4 * i as u32));
    for (i, &v) in longs.iter().enumerate() {
        wr32(b, EFFECT_PALETTE_RAM + 4 * i as u32, v);
    }
    c.d[..7].copy_from_slice(&longs[..7]);
    c.a[3] = longs[7];
    c.a[1] = EFFECT_PALETTE_RAM;
    c.a[2] = from + 32;
    rts(c, b);
}

const fn palette(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    let outputs = regs::d(0) | regs::d(1) | regs::d(2) | regs::d(3) | regs::d(4) | regs::d(5) | regs::d(6) | regs::a(1) | regs::a(2) | regs::a(3);
    Native { entry, name, cost: |_, _| 0, run, touches_sound: false, outputs, exact_cost: false }
}

pub const NATIVES: &[Native] = &[
    Native {
        entry: 0x3668,
        name: "joueur_place_au_sol",
        cost: |_, _| 0,
        run: place_on_free_ground,
        touches_sound: false,
        outputs: regs::d(0) | regs::d(1) | regs::d(2) | regs::d(5) | regs::d(6),
        exact_cost: false,
    },
    palette(0x3af8, "palette_effet_20", |c, b| load_effect_palette(c, b, 0x20)),
    palette(0x3afe, "palette_effet_0", |c, b| load_effect_palette(c, b, 0)),
    palette(0x3b04, "palette_effet_360", |c, b| load_effect_palette(c, b, 0x360)),
    palette(0x3b0c, "palette_effet_380", |c, b| load_effect_palette(c, b, 0x380)),
    palette(0x3b14, "palette_effet_3a0", |c, b| load_effect_palette(c, b, 0x3a0)),
];
