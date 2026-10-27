//! Aides de création d'effets (EFFCT) appelées par les handlers des joueurs et des décors.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::families::effects_creators::restes::{Pur, Registres};
use crate::mem::{rd16, rd32, rd8, rts, wr16, wr8, A5};
use crate::{regs, Native};
use crate::objects::{pools, Obj};
use m68k::decode::Size;

/// Scène dont les effets de choc ne sont pas créés.
const NO_SHOCK_SCENE: u8 = 41;

/// Type des effets créés ici.
const SHOCK_KIND: u8 = 25;
const SPARK_KIND: u8 = 4;

/// 0xb10c8 : effet de choc (poussière, étincelle ; type 25) à la position du créateur : variante, réglage (octet bas :
/// `+0x2d`, haut : `+0x2f`), vitesses (`+0x14`, `+0x1c`). Rien dans la scène 41.
pub fn shock(b: &mut Cps2Bus, createur: Obj, variante: u8, reglage: u16, vx: u16, vy: u16) -> Option<Obj> {
    shock_par(&mut Pur, b, createur, variante, reglage, vx, vy)
}

/// `shock` avec les registres de l'original (appelants par adresse : `player/`) : A0 = le créateur ; A1, D1.w (octet
/// haut du réglage), retenue et drapeaux comme l'original.
#[track_caller]
pub fn shock_68k(c: &mut Cpu, b: &mut Cps2Bus, createur: Obj, variante: u8, reglage: u16, vx: u16, vy: u16) -> Option<Obj> {
    c.a[0] = createur.0;
    let r = shock_par(c, b, createur, variante, reglage, vx, vy);
    crate::diag::registres_morts(c, "effets", regs::a(1) | regs::d(1) | regs::CCR);
    r
}

pub(crate) fn shock_par(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, variante: u8, reglage: u16, vx: u16, vy: u16) -> Option<Obj> {
    let scene = crate::ram::SCENE.get(b);
    r.comparaison(NO_SHOCK_SCENE as u32, scene as u32, Size::Byte);
    if scene == NO_SHOCK_SCENE {
        return None;
    }
    let fx = r.alloue(b, &pools::EFFCT)?;
    fx.set_u8(b, Obj::ACTIVE, 1);
    fx.set_u8(b, Obj::KIND, SHOCK_KIND);
    for field in [Obj::X, Obj::Y, Obj::Z] {
        let v = me.u32(b, field);
        fx.set_u32(b, field, v);
    }
    fx.set_u16(b, Obj::VX, vx);
    fx.set_u16(b, Obj::VY, vy);
    let facing = me.u8(b, 0x2c);
    fx.set_u8(b, 0x2c, facing);
    fx.set_u8(b, Obj::VARIANT, variante);
    fx.set_u8(b, 0x2d, reglage as u8);
    // `lsr.w #8,D1w` : X = dernier bit sorti (bit 7), gardé jusqu'au retour
    let high = reglage >> 8;
    r.x(reglage & 0x80 != 0);
    r.d_mot(1, high);
    fx.set_u8(b, 0x2f, high as u8);
    fx.set_u16(b, 0x42, 0);
    r.drapeaux(0, Size::Word);
    Some(fx)
}

fn shock_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let (me, variante, reglage, vx, vy) = (Obj(c.a[0]), c.d[0] as u8, c.d[1] as u16, c.d[2] as u16, c.d[3] as u16);
    shock_par(c, b, me, variante, reglage, vx, vy);
    rts(c, b);
}

/// 0xaa96c : étincelle (type 4, variante `variante`) liée au créateur et posée entre lui et `autre` (A1), sauf pour un
/// joueur dans certains états. Le processeur reçoit D0 à D3 de la pose (`place_between`) ; la native rend A1 à A4.
pub(crate) fn spark_par(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, autre: u32, variante: u8) -> Option<Obj> {
    let skip = (rd8(b, me.0 + 0x3c) as i8) < 2 && rd8(b, me.0 + 0x10d).wrapping_sub(16) <= 3;
    if skip {
        return None;
    }
    let fx = r.alloue(b, &pools::EFFCT)?;
    fx.set_u8(b, Obj::ACTIVE, 1);
    fx.set_u8(b, Obj::KIND, SPARK_KIND);
    fx.set_u8(b, Obj::VARIANT, variante);
    let facing = me.u8(b, 0x2c) ^ 32;
    fx.set_u8(b, 0x2c, facing);
    fx.set_u16(b, 0x50, me.word());
    fx.set_u16(b, 0x54, autre as u16);
    poser(r, b, me, fx, autre);
    Some(fx)
}

/// Pose l'étincelle (`place_between`) ; le processeur reçoit D0, D1 (longs) et les mots bas de D2, D3.
fn poser(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, fx: Obj, autre: u32) -> u16 {
    let (d0, d1, d2, d3) = place_between(b, me.0, fx.0, autre);
    r.d(0, d0 as u32);
    r.d(1, d1 as u32);
    r.d_mot(2, d2);
    r.d_mot(3, d3);
    d1
}

/// 0xaaa04 (`owner_first`) / 0xaaa84 : étincelle (type 4, variante `variante`) entre le créateur et l'objet touché
/// `autre` ; `+0x50` / `+0x54` désignent l'un ou l'autre selon la routine. Drapeaux de D1.w.
pub(crate) fn spark_between_par(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, autre: u32, variante: u8, owner_first: bool) -> Option<Obj> {
    let fx = r.alloue(b, &pools::EFFCT)?;
    fx.set_u8(b, Obj::ACTIVE, 1);
    fx.set_u8(b, Obj::KIND, SPARK_KIND);
    fx.set_u8(b, Obj::VARIANT, variante);
    let facing = me.u8(b, 0x2c) ^ 32;
    fx.set_u8(b, 0x2c, facing);
    let (first, second) = if owner_first { (me.0, autre) } else { (autre, me.0) };
    fx.set_u16(b, 0x50, first as u16);
    fx.set_u16(b, 0x54, second as u16);
    let d1 = poser(r, b, me, fx, autre);
    r.drapeaux(d1 as u32, Size::Word);
    Some(fx)
}

/// Natives des étincelles : A0 = le créateur, A1 = l'autre objet, D1.b = variante ; A1 à A4 sont rendus.
fn spark_native(c: &mut Cpu, b: &mut Cps2Bus, entre: Option<bool>) {
    let saved = [c.a[1], c.a[2], c.a[3], c.a[4]];
    let (me, autre, variante) = (Obj(c.a[0]), c.a[1], c.d[1] as u8);
    match entre {
        None => spark_par(c, b, me, autre, variante),
        Some(owner_first) => spark_between_par(c, b, me, autre, variante, owner_first),
    };
    [c.a[1], c.a[2], c.a[3], c.a[4]] = saved;
    rts(c, b);
}

/// Boîtes de collision : par jeu (`+0x2e`), adresse d'un tableau de boîtes de 8 octets (dx, _, dz...),
/// choisies par `+0x38` (boîte de frappe) ou `+0x3a` (boîte de corps).
const HIT_BOXES: u32 = 0x11bffa;
const BODY_BOXES: u32 = 0x11c23a;

/// Boîte de l'objet `o` (frappe ou corps).
fn hit_box(b: &mut Cps2Bus, o: u32, hit: bool) -> u32 {
    let (table, index) = if hit { (HIT_BOXES, 0x38) } else { (BODY_BOXES, 0x3a) };
    let set = rd8(b, o + 0x2e) as u32;
    let boxes = rd32(b, table + 4 * set);
    let k = rd8(b, o + index) as u32;
    boxes + 8 * k
}

/// 0xab2b4 : pose l'étincelle `fx` à mi-chemin entre une boîte de l'attaquant `me` et une boîte de la
/// cible `victim`, selon la variante (variantes 0, 1, 3 : corps de l'un contre frappe de l'autre...),
/// en profondeur à mi-chemin, juste devant la plus proche. Rend D0 à D3 tels que l'original les laisse.
fn place_between(b: &mut Cps2Bus, me: u32, fx: u32, victim: u32) -> (u16, u16, u16, u16) {
    let (me_hit, victim_hit) = match rd8(b, fx + 3) {
        0 | 1 | 3 => (false, true),
        4 | 6 => (false, false),
        _ => (true, false),
    };
    let (mine, theirs) = (hit_box(b, me, me_hit), hit_box(b, victim, victim_hit));
    // milieu des deux bords, sur un axe : x (décalage 0 de la boîte, retourné selon l'orientation) ou z
    let middle = |b: &mut Cps2Bus, off: u32, field: u32, flip: bool| -> (u16, u16) {
        let mut d0 = rd8(b, mine + off) as i8 as i16 as u16;
        let mut d1 = rd8(b, theirs + off) as i8 as i16 as u16;
        if flip && rd8(b, me + 0x2c) != 0 {
            d0 = d0.wrapping_neg();
        }
        if flip && rd8(b, victim + 0x2c) != 0 {
            d1 = d1.wrapping_neg();
        }
        let a = d0.wrapping_add(rd16(b, me + field));
        let c = d1.wrapping_add(rd16(b, victim + field));
        let mut half = ((c.wrapping_sub(a) as i16).wrapping_abs() >> 1) as u16;
        if c as i16 <= a as i16 {
            half = half.wrapping_neg();
        }
        wr16(b, fx + field, a.wrapping_add(half));
        (a, c)
    };
    middle(b, 0, 0x08, true);
    let (z, their_z) = middle(b, 2, 0x0c, false);
    let (my_depth, their_depth) = (rd16(b, me + 0x10), rd16(b, victim + 0x10));
    let gap = my_depth.wrapping_sub(their_depth);
    let (front, gap) = if (gap as i16) < 0 { (my_depth, gap.wrapping_neg()) } else { (their_depth, gap) };
    let z = z.wrapping_add(gap >> 1);
    wr16(b, fx + 0x0c, z);
    wr16(b, fx + 0x10, front.wrapping_sub(1));
    (z, front.wrapping_sub(1), gap >> 1, their_z)
}


/// Crée un objet de type `kind` dans `pool`, variante D0 ; drapeaux de D0.b.
fn spawn(c: &mut Cpu, b: &mut Cps2Bus, pool: &pools::Pool, kind: u8) -> Option<Obj> {
    let object = pool.alloc_68k(c, b)?;
    object.set_u8(b, Obj::ACTIVE, 1);
    object.set_u8(b, Obj::KIND, kind);
    object.set_u8(b, Obj::VARIANT, c.d[0] as u8);
    c.flags_logic(c.d[0] & 0xff, Size::Byte);
    Some(object)
}

/// 0xddeaa : décor de type 6.
fn spawn_scenery_6(c: &mut Cpu, b: &mut Cps2Bus) {
    spawn(c, b, &pools::SOBJ, 6);
}

/// 0xdfa4a : décor de type 11, orienté selon D1.
fn spawn_scenery_11(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(object) = spawn(c, b, &pools::SOBJ, 11) {
        object.set_u8(b, 0x2c, c.d[1] as u8);
        c.flags_logic(c.d[1] & 0xff, Size::Byte);
    }
}

/// 0x517ba : effet de joueur de type 44 ; son réglage `+0x2f` est l'octet suivant dans la table A4.
fn spawn_player_effect_44(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(object) = spawn(c, b, &pools::PEFF, 44) {
        let setting = rd8(b, c.a[4]);
        c.a[4] = c.a[4].wrapping_add(1);
        object.set_u8(b, 0x2f, setting);
    }
}

/// 0xcc18e : toutes les 32 images, compte les joueurs vivants dans `A5+0x35`.
fn count_living_players(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    let timer = rd16(b, me + 0x20).wrapping_sub(1);
    wr16(b, me + 0x20, timer);
    if (timer as i16) >= 0 {
        return;
    }
    wr16(b, me + 0x20, 32);
    let alive = (0..4).filter(|k| rd8(b, A5 + 0x5de + k * 0x200) != 0 && rd16(b, A5 + 0x5de + k * 0x200 + 0x62) != 0).count();
    wr8(b, A5 + 0x35, alive as u8);
    c.d[0] = alive as u32;
}

/// 0x25260 : un joueur scripté se place devant l'objet (`A5+0x46`), décalé selon son numéro (`+2`).
fn place_scripted_player(c: &mut Cpu, b: &mut Cps2Bus) {
    /// Décalages en X par numéro de joueur (mots, espace programme : 0x40, 0x10, -0x18, -0x44).
    const OFFSETS: u32 = 0x2528c;
    let me = c.a[0];
    let focus = rd16(b, A5 + 0x46) as i16 as i32 as u32;
    c.a[1] = focus;
    let x = rd16(b, focus + 0x08);
    wr16(b, me + 0x08, x);
    let depth = rd16(b, focus + 0x10);
    wr16(b, me + 0x10, depth);
    wr16(b, me + 0x0c, 0);
    let index = 2 * rd8(b, me + 2) as u32;
    let shift = crate::mem::prog16(b, OFFSETS + index);
    c.d[0] = shift as u32;
    let x = rd16(b, me + 0x08).wrapping_add(shift);
    wr16(b, me + 0x08, x);
    let depth = crate::ops::sub(c, 18, rd16(b, me + 0x10) as u32, Size::Word);
    wr16(b, me + 0x10, depth as u16);
}

const fn aid(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus), outputs: u32) -> Native {
    Native { entry, name, run, touches_sound: false, outputs }
}

macro_rules! leaf {
    ($entry:literal, $name:literal, $body:path, $outputs:expr) => {
        aid($entry, $name, |c, b| {
            $body(c, b);
            rts(c, b);
        }, $outputs)
    };
}

pub const NATIVES: &[Native] = &[
    leaf!(0xddeaa, "decor_type_6", spawn_scenery_6, regs::a(1) | regs::CCR),
    leaf!(0xdfa4a, "decor_type_11", spawn_scenery_11, regs::a(1) | regs::CCR),
    leaf!(0x517ba, "effet_joueur_44", spawn_player_effect_44, regs::a(1) | regs::a(4) | regs::CCR),
    leaf!(0xcc18e, "decompte_joueurs_vivants", count_living_players, regs::d(0)),
    leaf!(0x25260, "joueur_scripte_place", place_scripted_player, regs::a(1)),
    aid(0xb10c8, "effet_choc", shock_native, regs::a(1) | regs::d(1) | regs::CCR),
    aid(0xaa96c, "effet_etincelle", |c, b| spark_native(c, b, None), regs::a(1) | regs::a(2) | regs::a(3) | regs::a(4)),
    aid(0xaaa04, "effet_etincelle_entre", |c, b| spark_native(c, b, Some(true)), regs::ALL),
    aid(0xaaa84, "effet_etincelle_entre_b", |c, b| spark_native(c, b, Some(false)), regs::ALL),
];
