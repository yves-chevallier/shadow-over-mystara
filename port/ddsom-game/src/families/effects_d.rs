//! Famille EFFCT, type 31 (`0xb1a98`) : écrans de fin de scène et de décompte, dix variantes.
//!
//! - 0 : pièce projetée par un coffre (vole, retombe, attend la fin de la séquence) ;
//! - 2 : icône de récompense qui, à la fin de la séquence, devient l'objet ramassé (`0xe24d8`) ;
//! - 3 : rang affiché ; 4 : plaque ; 5 : ligne de résultat d'un joueur, qui se dessine dans le fond 2
//!   quand elle arrive à l'écran ; 6 : compteurs sur trois chiffres ; 8 : ligne par joueur ;
//! - 7 : personnage de l'écran de fin qui marche vers sa place, saute, se retourne ;
//! - 1 : trophée du tableau de fin, qui attend le signal de son contrôleur, s'anime après un délai tiré
//!   au hasard puis se décompte auprès du contrôleur ; 9 : enseigne posée sur la caméra alternative.

use super::cx::{handlers, Cx};
use crate::mem::{rd8, wr16, wr32};
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Courant;
use crate::machine::Machine;

mod f {
    pub const STATE: u32 = 0x04;
    pub const SUB_STATE: u32 = 0x05;
    pub const STEP: u32 = 0x06;
    pub const SUB_STEP: u32 = 0x07;
    pub const VARIANT: u32 = 0x03;
    pub const HIDDEN: u32 = 0x40;
    pub const FACING: u32 = 0x2c;
    pub const MODE: u32 = 0x2f;
    /// Valeur affichée (variantes 3, 5, 9) ou rang du personnage (variante 7).
    pub const VALUE: u32 = 0x42;
    pub const SHOWN: u32 = 0x43;
    /// Contrôleur de la séquence (long) : son octet `+8` donne l'étape (2 : fin ; négatif : suspendu).
    pub const SEQUENCE: u32 = 0x4c;
}

const ANIM_SET: u32 = 0x1c03a6;

fn free(x: &mut Cx) {
    x.free(&pools::EFFCT);
}

fn random(x: &mut Cx) -> u32 {
    let alea = x.random_68k();
    x.c.d[0] & 0xffff_0000 | alea as u32
}

/// État 4 (`0xb2916`) : passe à 6 et libère ; état 6 : libère.
fn finish(x: &mut Cx) {
    if x.u8(f::STATE) == 4 {
        x.add8(f::STATE, 2);
    }
    free(x)
}

fn sequence_step(x: &mut Cx) -> u8 {
    let seq = x.u32(f::SEQUENCE);
    rd8(x.b, seq + 8)
}

fn countdown_screen(x: &mut Cx) {
    let v = x.u8(f::VARIANT);
    match (v, x.u8(f::STATE)) {
        (_, 4 | 6) => finish(x),
        (0, 0) => coin_init(x),
        (0, 2) => coin_active(x),
        (1, 0) => trophy_init(x),
        (1, 2) => trophy_active(x),
        (2, 0) => reward_init(x),
        (2, 2) => reward_active(x),
        (3, 0) => rank_init(x),
        (3, 2) => rank_active(x),
        (4, 0) => plate_init(x),
        (5, 0) => line_init(x),
        (5, 2) => line_active(x),
        (6, 0) => counters_init(x),
        (6, 2) => counters_active(x),
        (7, 0) => walker_init(x),
        (7, 2) => walker_active(x),
        (8, 0) => player_line_init(x),
        (9, 0) => sign_init(x),
        (9, 2) => {
            x.anim_step();
        }
        (8, 2) => {
            if rd8(x.b, 0xff2a39) as i8 >= 10 {
                finish_from_active(x);
            }
        }
        _ => {}
    }
}

/// Fin depuis l'état actif (`0xb2916` : `addq.b #2` puis libération).
fn finish_from_active(x: &mut Cx) {
    x.add8(f::STATE, 2);
    free(x)
}

// ---------------------------------------------------------------------------------------------
// variante 0 : pièce projetée
// ---------------------------------------------------------------------------------------------

/// Vitesses (vx, vz, az) par tirage (8 entrées de 8 octets, espace programme) et animations.
const COIN_SPEEDS: u32 = 0xb1b76;
const COIN_ANIMS: u32 = 0xb1b66;

fn coin_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    let source = x.linked(0x42);
    let (sx, sz) = (source.u16(x.b, Obj::X), source.u16(x.b, Obj::Z));
    x.set16(Obj::X, sx);
    x.set16(Obj::Z, sz);
    x.set32(Obj::Y, 0x1_0000);
    let r = random(x) as u16 & 0x7f;
    let dx = if r & 1 != 0 { r.wrapping_neg() } else { r };
    x.add16(Obj::X, dx);
    let k = (random(x) & 7) << 3;
    x.set16(Obj::AX, 0);
    for (off, field) in [(0, Obj::VX), (2, Obj::VZ), (4, Obj::AZ)] {
        let v = x.rom16(COIN_SPEEDS + k + off);
        x.set16(field, v);
    }
    let r = random(x);
    x.set8(f::FACING, if r & 1 != 0 { 0x20 } else { 0 });
    let r = random(x);
    let anim = (x.rom8(COIN_ANIMS + (r & 0xf)) as u16).wrapping_add(0x104);
    x.anim_start(ANIM_SET, (r & 0xffff_0000 | anim as u32) as u16);
    x.set8(f::STEP, 0);
    x.reinsert();
}

fn coin_active(x: &mut Cx) {
    if x.u8(f::SUB_STATE) == 2 {
        if sequence_step(x) == 2 {
            x.add8(f::SUB_STATE, 2);
            x.set8(f::HIDDEN, 0);
        }
        return;
    }
    if (sequence_step(x) as i8) < 0 {
        return x.set8(f::HIDDEN, 1);
    }
    x.anim_step();
    x.move_xz();
    if (x.u16(Obj::VZ) as i16) < 0 {
        x.set32(Obj::Y, 0);
    }
    if x.u16(Obj::Z) >= x.a5_u16(0x50a) {
        return x.reinsert();
    }
    // sortie par le bas de l'écran
    x.add8(f::STATE, 2);
    x.set8(f::HIDDEN, 1);
}

// ---------------------------------------------------------------------------------------------
// variante 1 : trophée du tableau de fin
// ---------------------------------------------------------------------------------------------

mod trophy {
    /// Contrôleur du tableau (pointeur de 16 bits) : `+0x30` = phase (1 : s'animer, 2 : fin),
    /// `+0x31` = trophées qui ne se sont pas encore animés.
    pub const CONTROLLER: u32 = 0x5c;
    pub const PHASE: u32 = 0x30;
    pub const PENDING: u32 = 0x31;
    /// Objet dont la variante choisit l'animation (pointeur négatif), sinon `+0x50` est le rang lui-même.
    pub const SOURCE: u32 = 0x50;
    /// Places (x, profondeur, orientation : 8 octets) par mode `+0x2f`, décalées de 6 places pour
    /// un contrôleur de variante 11 (données).
    pub const PLACES: u32 = 0xb1cb4;
    /// Délais avant l'animation, tirés au hasard (octets, espace programme).
    pub const DELAYS: u32 = 0xb1d4e;
    pub const DELAY: u32 = 0x42;
    /// Décor créé quand il ne reste que cinq trophées, et qui doit avoir fini (`+0x22`).
    pub const SCENERY: u32 = 0x58;
    pub const SCENERY_KIND: u32 = 0x1d;
}

fn trophy_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    let source = x.u16(trophy::SOURCE);
    let rank = if (source as i16) < 0 { x.linked(trophy::SOURCE).u8(x.b, f::VARIANT) as u16 } else { source };
    let mut place = (x.u8(f::MODE) as u32) << 3;
    let controller = x.linked(trophy::CONTROLLER);
    if controller.u8(x.b, f::VARIANT) == 0xb {
        place += 0x30;
    }
    let at = trophy::PLACES + place;
    let (px, py, facing) = (x.data16(at), x.data16(at + 2), x.data8(at + 4));
    x.set16(Obj::X, px);
    x.set16(Obj::Y, py);
    x.set8(f::FACING, facing);
    x.set32(Obj::Z, 0);
    let mut anim = x.data16(REWARD_ANIMS + 2 * rank as u32);
    if x.u8(f::MODE) & 1 != 0 {
        anim = anim & 0xff00 | (anim as u8).wrapping_add(1) as u16;
    }
    x.anim_start(REWARD_SET, (anim as u32) as u16);
    x.reinsert();
}

fn trophy_active(x: &mut Cx) {
    let controller = x.linked(trophy::CONTROLLER);
    match x.u8(f::SUB_STATE) {
        0 => {
            if controller.u8(x.b, trophy::PHASE) != 1 {
                return;
            }
            x.add8(f::SUB_STATE, 2);
            let r = random(x) & 7;
            let delay = x.rom8(trophy::DELAYS + r);
            x.set8(trophy::DELAY, delay);
        }
        2 => {
            let n = x.u8(trophy::DELAY).wrapping_sub(1);
            x.set8(trophy::DELAY, n);
            if n == 0 {
                x.add8(f::SUB_STATE, 2);
            }
        }
        4 => {
            if !x.anim_step() {
                return;
            }
            x.add8(f::SUB_STATE, 2);
            let pending = controller.u8(x.b, trophy::PENDING).wrapping_sub(1);
            controller.set_u8(x.b, trophy::PENDING, pending);
            if pending != 5 {
                return x.add8(f::SUB_STATE, 2);
            }
            x.c.d[0] = trophy::SCENERY_KIND;
            x.call(0xc8312);
            if x.c.sr & m68k::SR_C == 0 {
                x.set16(trophy::SCENERY, x.c.a[1] as u16);
            }
        }
        6 => {
            if x.linked(trophy::SCENERY).u8(x.b, 0x22) == 0 {
                return;
            }
            x.add8(f::SUB_STATE, 2);
            let pending = controller.u8(x.b, trophy::PENDING).wrapping_sub(1);
            controller.set_u8(x.b, trophy::PENDING, pending);
        }
        _ => {
            if controller.u8(x.b, trophy::PHASE) == 2 {
                finish_from_active(x);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// variante 9 : enseigne posée sur la caméra alternative
// ---------------------------------------------------------------------------------------------

/// Hauteurs par valeur `+0x42` (mots, espace programme).
const SIGN_HEIGHTS: u32 = 0xb2908;

fn sign_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    let (cy, cx) = (x.a5_u16(0x55a), x.a5_u16(0x556));
    x.set16(Obj::Z, cy);
    x.set16(Obj::X, cx);
    x.set32(Obj::Y, 0xffff_ffff);
    x.add16(Obj::X, 0x110);
    let k = 2 * x.u8(f::VALUE) as u32;
    let lift = x.rom16(SIGN_HEIGHTS + k);
    x.add16(Obj::Z, lift);
    x.anim_start(ANIM_SET, 0x1b7);
    x.reinsert();
}

// ---------------------------------------------------------------------------------------------
// variante 2 : icône de récompense
// ---------------------------------------------------------------------------------------------

/// Animations de base par mode `+0x2f` (mots, données) dans le jeu 0x190ba8.
const REWARD_ANIMS: u32 = 0xb1c9a;
const REWARD_SET: u32 = 0x190ba8;

/// 0xb24b6 : animation de base du mode + `n`.
fn reward_anim(x: &mut Cx, n: u16) {
    let mode = x.u8(f::MODE) as u32;
    let base = x.data16(REWARD_ANIMS + 2 * mode);
    x.anim_start(REWARD_SET, (base.wrapping_add(n) as u32) as u16);
}

fn reward_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    reward_anim(x, 2);
    x.reinsert();
}

fn reward_active(x: &mut Cx) {
    x.anim_step();
    if x.u8(f::SUB_STATE) != 0 || sequence_step(x) != 2 {
        return;
    }
    reward_anim(x, 0xe);
    // devient pour un instant l'objet (type `+0x42`, variante `+0x2f`) le temps de l'effet de ramassage
    let variant = x.u8(f::VARIANT);
    let (mode, kind) = (x.u8(f::MODE), x.u8(f::VALUE));
    x.set8(f::VARIANT, mode);
    x.set8(0x02, kind);
    x.c.d[0] = x.c.d[0] & !0xff | variant as u32;
    x.call(0xe24d8);
    let back = x.c.d[0] as u8;
    x.set8(f::VARIANT, back);
    x.set8(0x02, 31);
    x.add8(f::SUB_STATE, 2);
}

// ---------------------------------------------------------------------------------------------
// variantes 3, 4, 6, 8 : rang, plaque, compteurs, lignes des joueurs
// ---------------------------------------------------------------------------------------------

fn at_camera(x: &mut Cx, cx: u32, cy: u32) {
    let (px, pz) = (x.a5_u16(cx), x.a5_u16(cy));
    x.set16(Obj::X, px);
    x.set16(Obj::Z, pz);
    x.set32(Obj::Y, 0);
}

fn rank_anim(x: &mut Cx, value: u8) {
    x.anim_start(ANIM_SET, (0xe3u16.wrapping_sub(value as u16) as u32) as u16);
    x.reinsert();
}

fn rank_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    at_camera(x, 0x506, 0x50a);
    x.add16(Obj::X, 0x13c);
    x.add16(Obj::Z, 0xcc);
    let value = x.u8(f::VALUE);
    x.set8(f::SHOWN, value);
    rank_anim(x, value);
}

fn rank_active(x: &mut Cx) {
    let value = x.u8(f::VALUE);
    if value != x.u8(f::SHOWN) {
        x.set8(f::SHOWN, value);
        rank_anim(x, value);
    }
}

fn plate_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    let cam = x.a5_u16(0x556);
    x.set16(Obj::X, cam);
    x.set32(Obj::Y, 0);
    x.set16(Obj::Z, 0x460);
    x.add16(Obj::X, 0x80);
    let anim = if x.a5_u16(0x70) == 0 { 0x168 } else { 0x169 };
    x.anim_start(ANIM_SET, anim);
    x.reinsert();
}

/// Compteurs de la scène (deux octets à `A5-0x5b21`) et leurs places dans le fond 2.
const COUNTERS: u32 = 0xff24df;
const COUNTER_TILES: [u32; 2] = [0x910f6c, 0x910f74];
/// Tuiles des chiffres (mots, espace programme).
const DIGIT_TILES: u32 = 0xb2334;

/// 0xb2348 : valeur binaire (< 1000) en BCD sur trois chiffres.
fn to_bcd(value: u16) -> u16 {
    let (mut rest, mut bcd) = (value, 0u16);
    loop {
        rest = rest.wrapping_sub(100);
        if (rest as i16) < 0 {
            break;
        }
        bcd += 0x100;
    }
    rest = rest.wrapping_add(100);
    loop {
        rest = rest.wrapping_sub(10);
        if (rest as i16) < 0 {
            break;
        }
        bcd += 0x10;
    }
    rest.wrapping_add(10).wrapping_add(bcd)
}

/// 0xb230e : bloc de 2 × 2 tuiles (`tile`, `tile + 0x10` ; rangée suivante `tile + 1`...), puis la rangée
/// d'après. Renvoie la tuile suivante (`tile + 2`) et la nouvelle adresse.
fn draw_block(x: &mut Cx, at: &mut u32, tile: u16, attr: u16) -> u16 {
    let mut t = tile;
    for _ in 0..2 {
        wr16(x.b, *at, t);
        wr16(x.b, *at + 2, attr);
        wr16(x.b, *at + 4, t.wrapping_add(0x10));
        wr16(x.b, *at + 6, attr);
        *at += 0x80;
        t = t.wrapping_add(1);
    }
    t
}

/// 0xb22d0 : nombre BCD de trois chiffres en gros chiffres (zéros de tête effacés).
fn draw_big_number(x: &mut Cx, mut at: u32, bcd: u16) {
    let mut shown = false;
    for i in 0..4 {
        let digit = bcd.rotate_left(4 * (i + 1)) & 0xf;
        if i == 3 {
            shown = true;
        }
        if digit != 0 {
            shown = true;
        } else if !shown {
            at += 0x100;
            continue;
        }
        let tile = x.rom16(DIGIT_TILES + 2 * digit as u32);
        draw_block(x, &mut at, tile, 3);
    }
}

fn counters_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    let cam = x.a5_u16(0x556);
    x.set16(Obj::X, cam);
    x.set32(Obj::Y, 0);
    x.set16(Obj::Z, 0x448);
    x.add16(Obj::X, 0x50);
    let at = if x.a5_u16(0x70) == 0 { 0xb2af0 } else { 0xb2b6a };
    x.anim_start_entry(at);
    x.reinsert();
    for (k, at) in COUNTER_TILES.into_iter().enumerate() {
        let value = rd8(x.b, COUNTERS + k as u32) as u16;
        draw_big_number(x, at, to_bcd(value));
    }
}

fn counters_active(x: &mut Cx) {
    let gap = x.u16(Obj::Z).wrapping_sub(x.a5_u16(0x55a));
    if (gap as i16) < 0x100 {
        return;
    }
    for at in COUNTER_TILES {
        for row in 0..3 {
            wr32(x.b, at + 0x80 * row, 0x20_0000);
            wr32(x.b, at + 0x80 * row + 4, 0x20_0000);
        }
    }
}

fn player_line_init(x: &mut Cx) {
    let mode = x.u8(f::MODE) as u32;
    let player = Obj(0xff85de + 0x200 * mode);
    let present = player.is_active(x.b)
        && (player.u8(x.b, f::STATE) as i8) < 4
        && player.u32(x.b, 0x118) == 0xffff_ffff
        && player.u32(x.b, 0x11c) == 0xffff_ffff;
    if !present {
        return finish_from_active(x);
    }
    x.add8(f::STATE, 2);
    at_camera(x, 0x506, 0x50a);
    x.add16(Obj::X, 0x10);
    x.add16(Obj::Z, 0xb8);
    x.add16(Obj::Z, ((mode as u16) << 5).wrapping_neg());
    x.anim_start(ANIM_SET, (0x173 + mode) as u16);
    x.reinsert();
}

// ---------------------------------------------------------------------------------------------
// variante 5 : ligne de résultat d'un joueur
// ---------------------------------------------------------------------------------------------

mod line {
    /// Décalage vertical par rang (mots, espace programme).
    pub const OFFSETS: u32 = 0xb1f6a;
    /// Place dans le fond 2 (décalage depuis 0x912000) par rang, espace programme.
    pub const PLACES: u32 = 0xb1fa8;
    pub const LAYER: u32 = 0x912000;
    /// Portrait : palettes de 8 longs par personnage (0x25c4c0), vers la palette 0x91c600 + 32 × joueur.
    pub const PORTRAITS: u32 = 0x25c4c0;
    pub const PALETTES: u32 = 0x91c600;
    /// Première tuile du portrait par personnage (mots, espace programme).
    pub const PORTRAIT_TILES: u32 = 0xb1fe0;
    /// Attribut du numéro et du nom par joueur (octets, espace programme).
    pub const NUMBER_ATTRS: u32 = 0xb21d4;
    pub const NAME_ATTRS: u32 = 0xb20b4;
    /// Noms des personnages (pointeurs longs par paire de classes, espace programme ; chaînes terminées
    /// par 0x40, données).
    pub const NAMES: u32 = 0xb20e8;
    /// Petits chiffres (mots, espace programme) ; texte qui suit le score (données).
    pub const SMALL_DIGITS: u32 = 0xb2240;
    pub const SCORE_SUFFIX: u32 = 0xb21d0;
    /// Gros caractères du titre (mots, données), titres par défaut (8 octets par personnage, + 0x60
    /// avec `A5+0x70`), titres spéciaux (codes 0xf9 à 0xfe).
    pub const GLYPHS: u32 = 0xb2920;
    pub const DEFAULT_TITLES: u32 = 0xb2a30;
    pub const SPECIAL_TITLES: u32 = 0xb2a24;
}

fn line_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    let (cam_x, cam_y) = (x.a5_u16(0x556), x.a5_u16(0x55a));
    x.set16(Obj::Z, cam_y);
    x.set16(Obj::X, cam_x);
    x.set32(Obj::Y, 0);
    x.add16(Obj::X, 0x20);
    let rank = x.u8(f::VALUE) as u32;
    let dz = x.rom16(line::OFFSETS + 2 * rank);
    x.add16(Obj::Z, dz);
    let mode = x.u8(f::MODE) ^ 3;
    x.set8(f::MODE, mode);
    x.anim_start(ANIM_SET, 0x16a);
    x.reinsert();
}

/// La ligne (montante) atteint le haut de la caméra : elle se dessine une fois dans le fond 2.
fn line_active(x: &mut Cx) {
    if x.u8(f::SUB_STATE) != 0 || (x.u16(Obj::Z) as i16) < (x.a5_u16(0x55a) as i16) {
        return;
    }
    x.add8(f::SUB_STATE, 2);
    let rank = x.u8(f::VALUE) as u32;
    let place = x.rom16(line::PLACES + 2 * rank) as i16 as i32 as u32;
    let base = line::LAYER.wrapping_add(place);
    let record = x.u32(f::SEQUENCE);
    let mut d = LineDraw { record, attr: 0, shown: false };
    d.portrait(x, base + 0x180);
    d.number(x, base);
    d.level(x, base + 0x8c);
    d.name(x, base + 0x400);
    d.score(x, base + 0xa88);
    d.title(x, base + 0x408);
}

/// Dessin d'une ligne : la fiche du joueur (`+0x4c`) et l'attribut courant (D1 de l'original), qui
/// passe d'un élément au suivant.
struct LineDraw {
    record: u32,
    attr: u16,
    shown: bool,
}

impl LineDraw {
    fn class(&self, x: &mut Cx) -> u8 {
        rd8(x.b, self.record + 0xc)
    }

    /// 0xb222e : paire de tuiles `tile`, `tile + 0x10`, puis rangée suivante.
    fn pair(&self, x: &mut Cx, at: &mut u32, tile: u16) {
        wr16(x.b, *at, tile);
        wr16(x.b, *at + 2, self.attr);
        wr16(x.b, *at + 4, tile.wrapping_add(0x10));
        wr16(x.b, *at + 6, self.attr);
        *at += 0x80;
    }

    /// 0xb2212 : petit chiffre (zéros de tête effacés tant que `shown` est faux).
    fn digit(&mut self, x: &mut Cx, at: &mut u32, digit: u16) {
        let digit = digit & 0xf;
        if digit == 0 && !self.shown {
            *at += 0x80;
            return;
        }
        self.shown = true;
        let tile = x.rom16(line::SMALL_DIGITS + 2 * digit as u32);
        self.pair(x, at, tile);
    }

    /// 0xb20d2 : chaîne de tuiles (une par octet) terminée par 0x40, une colonne par caractère.
    fn text(&self, x: &mut Cx, at: &mut u32, mut s: u32) {
        loop {
            let c = x.data8(s);
            s += 1;
            if c == 0x40 {
                return;
            }
            wr16(x.b, *at, c as u16);
            wr16(x.b, *at + 2, self.attr);
            *at += 0x80;
        }
    }

    /// 0xb1ff8 : portrait (palette recopiée, puis 4 colonnes de 2 tuiles).
    fn portrait(&mut self, x: &mut Cx, mut at: u32) {
        let class = self.class(x) as u32;
        let mode = x.u8(f::MODE) as u32;
        let src = line::PORTRAITS + (class << 5);
        let dst = line::PALETTES + (mode << 5);
        for k in 0..8 {
            let v = x.data32(src + 4 * k) | 0xf000_f000;
            wr32(x.b, dst + 4 * k, v);
        }
        let mut tile = x.rom16(line::PORTRAIT_TILES + 2 * class);
        self.attr = 0x10u8.wrapping_add(mode as u8) as u16;
        for _ in 0..4 {
            self.pair(x, &mut at, tile);
            tile = tile.wrapping_add(1);
        }
    }

    /// 0xb21d8 : numéro du joueur (petit chiffre) et une tuile fixe.
    fn number(&mut self, x: &mut Cx, mut at: u32) {
        let mode = x.u8(f::MODE);
        self.attr = x.rom8(line::NUMBER_ATTRS + mode as u32) as u16;
        self.shown = true;
        self.digit(x, &mut at, mode.wrapping_add(1) as u16);
        self.pair(x, &mut at, 0x211);
    }

    /// 0xb2078 : niveau du joueur (deux chiffres) après deux tuiles fixes.
    fn level(&mut self, x: &mut Cx, mut at: u32) {
        for tile in [0x4c, 0x5f] {
            wr16(x.b, at, tile);
            wr16(x.b, at + 2, self.attr);
            at += 0x80;
        }
        let bcd = to_bcd(rd8(x.b, self.record + 0xd) as u16);
        for digit in [(bcd >> 4) & 0xf, bcd & 0xf] {
            wr16(x.b, at, digit);
            wr16(x.b, at + 2, self.attr);
            at += 0x80;
        }
    }

    /// 0xb20b8 : nom du personnage.
    fn name(&mut self, x: &mut Cx, mut at: u32) {
        let mode = x.u8(f::MODE) as u32;
        self.attr = x.rom8(line::NAME_ATTRS + mode) as u16;
        let pair = (self.class(x) & !1) as u32;
        let s = x.rom16(line::NAMES + 2 * pair) as u32;
        let s = s << 16 | x.rom16(line::NAMES + 2 * pair + 2) as u32;
        self.text(x, &mut at, s);
    }

    /// 0xb219e : trois tuiles fixes, le score (huit chiffres BCD), puis le texte qui suit.
    fn score(&mut self, x: &mut Cx, mut at: u32) {
        for tile in [0x1f6, 0x219, 0x211] {
            self.pair(x, &mut at, tile);
        }
        at += 0x100;
        self.shown = false;
        let score = x.data32(self.record + 8);
        for i in 0..8 {
            self.digit(x, &mut at, (score >> (28 - 4 * i)) as u16);
        }
        at += 4;
        self.text(x, &mut at, line::SCORE_SUFFIX);
    }

    /// 0xb212c : titre en gros caractères ; 0xff : titre par défaut du personnage recopié dans la fiche ;
    /// 0xf9 à 0xfe : titre spécial de six caractères consécutifs.
    fn title(&mut self, x: &mut Cx, mut at: u32) {
        let mut p = self.record;
        if rd8(x.b, p) == 0xff {
            let mut k = (self.class(x) as u32) << 3;
            if x.a5_u16(0x70) != 0 {
                k += 0x60;
            }
            let (a, b) = (x.data32(line::DEFAULT_TITLES + k), x.data32(line::DEFAULT_TITLES + k + 4));
            wr32(x.b, p, a);
            wr32(x.b, p + 4, b);
        }
        let first = rd8(x.b, p);
        if first >= 0xf9 {
            let k = ((first as i8 as i16).wrapping_neg() - 2) as i32 * 2;
            let mut tile = x.data16(line::SPECIAL_TITLES.wrapping_add(k as u32));
            for _ in 0..6 {
                tile = draw_block(x, &mut at, tile, self.attr);
            }
            return;
        }
        loop {
            let c = rd8(x.b, p);
            p += 1;
            if c == 0xff {
                return;
            }
            let tile = x.data16(line::GLYPHS + 2 * c as u32);
            draw_block(x, &mut at, tile, self.attr);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// variante 7 : personnage de l'écran de fin
// ---------------------------------------------------------------------------------------------

mod walker {
    /// Fiches de 12 octets par rang : x, z, y, cible x, cible y (mots) et premier sous-état (octet),
    /// espace programme.
    pub const PLACES: u32 = 0xb2432;
    pub const TARGET_X: u32 = 0x50;
    pub const TARGET_Y: u32 = 0x52;
    /// Objet que le personnage prévient en partant (pointeur de 16 bits), dont `+0x23` compte les
    /// personnages encore en route.
    pub const WAITER: u32 = 0x46;
    pub const WALK_SPEEDS: u32 = 0x1c75b6;
    pub const RUN_SPEEDS: u32 = 0x1c7eb6;
}

fn walker_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    let place = walker::PLACES + 12 * x.u8(f::VALUE) as u32;
    for (k, off) in [Obj::X, Obj::Z, Obj::Y, walker::TARGET_X, walker::TARGET_Y].into_iter().enumerate() {
        let v = x.rom16(place + 2 * k as u32);
        x.set16(off, v);
    }
    let sub = x.rom8(place + 10);
    x.set8(f::SUB_STATE, sub);
    let (cam_x, cam_y) = (x.a5_u16(0x556), x.a5_u16(0x55a));
    x.add16(Obj::X, cam_x);
    x.add16(walker::TARGET_X, cam_x);
    x.add16(Obj::Y, cam_y);
    x.add16(walker::TARGET_Y, cam_y);
    let left = (x.u16(Obj::X) as i16) < (x.u16(walker::TARGET_X) as i16);
    x.set8(f::FACING, if left { 0x20 } else { 0 });
}

fn walker_active(x: &mut Cx) {
    match x.u8(f::SUB_STATE) {
        0 => walker_wait(x),
        2 => walker_walk(x),
        4 => walker_jump(x),
        6 => walker_turn(x),
        8 => walker_run(x),
        _ => {}
    }
    x.reinsert();
}

fn walker_facing_target(x: &mut Cx) -> u8 {
    if (x.u16(Obj::X) as i16) > (x.u16(walker::TARGET_X) as i16) {
        0
    } else {
        0x20
    }
}

/// 0xb27aa : demi-tour, puis retour au sous-état courant.
fn walker_start_turn(x: &mut Cx) {
    let sub = x.u8(f::SUB_STATE);
    x.set8(f::SUB_STEP, sub);
    x.set8(f::STEP, 0);
    x.set8(f::SUB_STATE, 6);
    walker_turn(x);
}

fn walker_wait(x: &mut Cx) {
    if x.u8(f::STEP) == 0 {
        if x.u8(f::FACING) != 0 {
            return walker_start_turn(x);
        }
        x.add8(f::STEP, 2);
        reward_anim(x, 2);
        let waiter = x.linked(walker::WAITER);
        let n = waiter.u8(x.b, 0x23).wrapping_sub(1);
        waiter.set_u8(x.b, 0x23, n);
    }
    x.anim_step();
}

/// Un pas vers la cible (direction 0x137e, table de vitesses courante).
fn walker_move(x: &mut Cx) {
    let (tx, ty) = (x.u16(walker::TARGET_X), x.u16(walker::TARGET_Y));
    x.c.d[1] = x.c.d[1] & 0xffff_0000 | tx as u32;
    x.c.d[2] = x.c.d[2] & 0xffff_0000 | ty as u32;
    x.call(0x137e);
    let direction = x.c.d[0] as u8;
    x.set8(Obj::SPEED_INDEX, direction);
    x.move_by_table_xy();
}

/// La cible est atteinte à 8 près sur l'axe `off`.
fn walker_near(x: &mut Cx, off: u32, target: u32) -> bool {
    x.u16(off).wrapping_sub(x.u16(target)).wrapping_add(8) <= 0x10
}

fn walker_walk(x: &mut Cx) {
    if x.u8(f::STEP) == 0 {
        if walker_facing_target(x) != x.u8(f::FACING) {
            return walker_start_turn(x);
        }
        x.add8(f::STEP, 2);
        reward_anim(x, 4);
        x.set32(Obj::SPEED_TABLE, walker::WALK_SPEEDS);
    }
    walker_move(x);
    x.anim_step();
    if walker_near(x, Obj::X, walker::TARGET_X) && walker_near(x, Obj::Y, walker::TARGET_Y) {
        x.set8(f::SUB_STATE, 0);
        x.set16(f::STEP, 0);
        reward_anim(x, 2);
    }
}

fn walker_jump(x: &mut Cx) {
    match x.u8(f::STEP) {
        0 => {
            x.add8(f::STEP, 2);
            x.set8(f::SUB_STEP, 0);
            reward_anim(x, 5);
            x.set16(Obj::VX, 0xfc00);
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x400);
            x.set16(Obj::AZ, 0xffc0);
        }
        2 => match x.u8(f::SUB_STEP) {
            0 => {
                if x.anim_step() {
                    reward_anim(x, 6);
                    x.add8(f::SUB_STEP, 2);
                }
            }
            2 => {
                x.anim_step();
                x.move_xz();
                if (x.u16(Obj::VZ) as i16) <= 0 {
                    reward_anim(x, 7);
                    x.add8(f::SUB_STEP, 2);
                }
            }
            4 => {
                x.anim_step();
                x.move_xz();
                if (x.u16(Obj::Z) as i16) <= 0 {
                    x.add8(f::STEP, 2);
                    x.set8(f::SUB_STEP, 0);
                    x.set32(Obj::Z, 0);
                    reward_anim(x, 8);
                }
            }
            _ => {}
        },
        4 => {
            if x.anim_step() {
                x.set8(f::SUB_STATE, 2);
                x.set16(f::STEP, 0);
            }
        }
        _ => {}
    }
}

fn walker_turn(x: &mut Cx) {
    let late = (x.u8(f::MODE) as i8) >= 0xa;
    if x.u8(f::STEP) == 0 {
        x.add8(f::STEP, 2);
        if !late {
            let facing = x.u8(f::FACING) ^ 0x20;
            x.set8(f::FACING, facing);
        }
        reward_anim(x, 3);
    }
    if !x.anim_step() {
        return;
    }
    let back = x.u8(f::SUB_STEP);
    x.set8(f::SUB_STATE, back);
    x.set16(f::STEP, 0);
    if late {
        let facing = x.u8(f::FACING) ^ 0x20;
        x.set8(f::FACING, facing);
    }
    reward_anim(x, 2);
}

fn walker_run(x: &mut Cx) {
    match x.u8(f::STEP) {
        0 => {
            x.add8(f::STEP, 2);
            x.set8(f::SUB_STEP, 0);
            x.set16(Obj::AX, 0);
            let vx: u16 = if x.u8(f::FACING) == 0 { 0x380u16.wrapping_neg() } else { 0x380 };
            x.set16(Obj::VX, vx);
            x.set32(Obj::SPEED_TABLE, walker::RUN_SPEEDS);
            reward_anim(x, 9);
        }
        2 => {
            x.anim_step();
            walker_move(x);
            if !walker_near(x, Obj::X, walker::TARGET_X) {
                return;
            }
            x.add8(f::STEP, 2);
            if x.u8(f::FACING) != 0 {
                x.add8(f::STEP, 2);
                x.set16(Obj::AX, 0xffe0);
                reward_anim(x, 0xc);
            } else {
                x.set16(Obj::AX, 0x20);
                reward_anim(x, 0xa);
            }
        }
        step @ (4 | 6) => {
            if x.u8(f::SUB_STEP) == 0 {
                x.anim_step();
                x.move_x();
                if x.u8(Obj::VX) != 0 {
                    return;
                }
                x.add8(f::SUB_STEP, 2);
                reward_anim(x, if step == 4 { 0xb } else { 0xd });
            } else if x.u8(f::SUB_STEP) == 2 {
                if !x.anim_step() {
                    return;
                }
                x.set8(f::SUB_STATE, 0);
                x.set16(f::STEP, 0);
                if step == 6 {
                    let facing = x.u8(f::FACING) ^ 0x20;
                    x.set8(f::FACING, facing);
                    reward_anim(x, 2);
                }
            }
        }
        _ => {}
    }
}

handlers! {
    0xb1a98 "effct_decompte" countdown_screen,
}
