//! Boss de type 16 (0x9325e) : boss de la scène 14 (h). Voir `docs/familles/boss.md`.
//!
//! Gabarit ancien, comme Ezerhorden (`boss_5`) : `+5` (multiples de 2) et `+6`, `+7` sont posés champ par champ, les
//! trois jeux d'animations (corps 0x17efe8, attaques 0x17f002, coups reçus 0x17f046) sont choisis par `anim_a`,
//! `anim_b` et `anim_c` qui mémorisent le jeu et l'animation (`+0xa0`, `+0xa1`) pour ne pas relancer la même. Le boss
//! se tient accroupi puis charge les joueurs (`+3` : 0 en bas de l'écran, 1 en haut, d'après la caméra).
//!
//! | `+5` | Fichier | Rôle |
//! |---|---|---|
//! | 0 | `moves` | réveil : les deux variantes attendent qu'un joueur s'approche |
//! | 2 | `moves` | combat : attente (`+6` = 0), demi-tour (2), marche (4 et 8), bond vers la cible (6) |
//! | 4 | `attacks` | attaques : huit coups (`+6` = 0 à 14) |
//! | 6 | `moves` | course vers un point de l'écran (`+6` = 0 et 2), puis saut (4) |
//! | 8 | `moves` | attente avant une attaque (`+6` = 0), course (2), saut (4) |
//! | 10 | `moves` | pause |
//! | 12 | `moves` | un joueur saisi (`+0x50`) : morsures, puis relâché |
//! | état 4 | `hit` | réactions aux coups (routines propres), recul, à terre, mort |

use super::boss_common::*;
use super::cx::handlers;

mod attacks;
mod hit;
mod moves;

/// Champs propres à ce type.
pub(self) mod g {
    /// Étage de proximité des joueurs (0 à 7) pendant le réveil.
    pub const NEAR: u32 = 0x2f;
    pub const NEAR_MAX: u32 = 0x30;
    /// Octet non nul : la pesanteur est appliquée en fin d'image (`gravity`).
    pub const FALLS: u32 = 0xa2;
    pub const VULNERABLE: u32 = 0x67;
    pub const WALK_STEPS: u32 = 0x6b;
    pub const IDLE_COUNT: u32 = 0x6e;
    pub const WALK_DELAY: u32 = 0x71;
    /// Mémoire de l'animation en cours : jeu (0, 2 ou 4) et numéro.
    pub const ANIM_SET: u32 = 0xa0;
    pub const ANIM_N: u32 = 0xa1;
    pub const HIT_BUDGET: u32 = 0xa5;
    /// Minuterie (octet ou mot selon l'étape).
    pub const TIMER: u32 = 0xa6;
    /// État à reprendre après un demi-tour (long).
    pub const SAVED_STATE: u32 = 0xa8;
    /// Point visé (mots) et sens de rotation de la trajectoire.
    pub const SPOT_X: u32 = 0xae;
    pub const SPOT_Z: u32 = 0xb0;
    pub const SPIN: u32 = 0xb3;
    pub const DOWN_TIME: u32 = 0x84;
    pub const SHAKE: u32 = 0x90;
    /// Octet mémorisé « la pause a été vue ».
    pub const PAUSE_SEEN: u32 = 0x42;
}
use g::*;
use crate::objects::pools;
use crate::objects::Courant;

const SET_A: u32 = 0x17efe8;
const SET_B: u32 = 0x17f002;
const SET_C: u32 = 0x17f046;

/// États posés d'un coup (`+4..+7`).
pub(self) mod state {
    pub const STAND: u32 = 0x0202_0000;
    pub const TURN: u32 = 0x0202_0200;
    pub const WALK: u32 = 0x0202_0400;
    pub const LEAP: u32 = 0x0202_0600;
    pub const WALK_B: u32 = 0x0202_0800;
    pub const IDLE: u32 = 0x020a_0000;
    pub const IDLE_PAUSED: u32 = 0x020a_0200;
}

/// Routines du moteur appelées par ce boss.
pub(self) mod call {
    pub const EDGE_PAIR: u32 = 0xa4b4;
    pub const EDGE_SAME: u32 = 0xa4c2;
    pub const SCRIPT_PALETTE: u32 = 0x10298;
    pub const SPLASH: u32 = 0xaaa40;
}

// --- animations --------------------------------------------------------------------------------

/// 0x95256 : lance l'animation `n` du jeu `set` (de sorte `kind`) par l'IA, sauf si c'est déjà celle en cours.
fn play_set(x: &mut Cx, set: u32, kind: u8, n: u32) {
    x.c.a[4] = set;
    x.c.d[0] = n;
    x.c.d[1] = x.c.d[1] & !0xff | kind as u32;
    if x.u8(ANIM_SET) == kind && x.u8(ANIM_N) == n as u8 {
        return;
    }
    x.set8(ANIM_SET, kind);
    x.set8(ANIM_N, n as u8);
    lib::start(x, set, n);
}

/// 0x95234 : animation `n` du jeu des mouvements.
pub(self) fn anim_a(x: &mut Cx, n: u32) {
    play_set(x, SET_A, 0, n);
}

/// 0x95240 : animation `n` du jeu des attaques.
pub(self) fn anim_b(x: &mut Cx, n: u32) {
    play_set(x, SET_B, 2, n);
}

/// 0x9524c : animation `n` du jeu des coups reçus.
pub(self) fn anim_c(x: &mut Cx, n: u32) {
    play_set(x, SET_C, 4, n);
}

// --- aides --------------------------------------------------------------------------------------

/// 0x95210 : annule les vitesses.
pub(self) fn stop(x: &mut Cx) {
    x.c.d[0] = 0;
    x.set32(0x14, 0);
    x.set32(0x18, 0);
    x.set32(0x1c, 0);
}

/// 0x93420 : plus de cible, attente.
pub(self) fn idle(x: &mut Cx) {
    x.set32(f::STATE, state::IDLE);
}

pub(self) fn next7(x: &mut Cx) {
    x.add8(0x7, 2);
}
pub(self) fn next6(x: &mut Cx) {
    x.add8(f::SUB2, 2);
}

/// 0x95220 : la caméra en service (son enregistrement ; X en `+8`, Y en `+0xc`).
pub(self) fn camera_record(x: &mut Cx) -> crate::objects::Obj {
    crate::objects::Obj(if x.a5_u8(var::CAMERA_SELECT) != 0 { 0xff854e } else { 0xff84fe })
}

/// 0x950b8 : la cible est-elle derrière le boss ? (sans cible, le boss attend et la réponse est oui.)
pub(self) fn target_behind(x: &mut Cx) -> bool {
    if x.u16(f::TARGET) as i16 >= 0 {
        idle(x);
        return true;
    }
    let t = x.linked(f::TARGET);
    let mut d = t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8));
    if x.u8(f::FLIP) == 0 {
        d = d.wrapping_neg();
    }
    (d.wrapping_add(0x20) as i16) < 0
}

/// 0x9489a : la cible est-elle loin (hors de ±0x20 en profondeur ou ±0xc0 en X) ?
pub(self) fn target_far(x: &mut Cx) -> bool {
    if x.u16(f::TARGET) as i16 >= 0 {
        idle(x);
        return true;
    }
    let t = x.linked(f::TARGET);
    let dz = t.u16(x.b, 0x10).wrapping_sub(x.u16(0x10)).wrapping_add(0x20);
    if dz <= 0x40 {
        return false;
    }
    let dx = t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)).wrapping_add(0xc0);
    dx > 0x180
}

/// 0x94902, 0x9491e, 0x94968 : une cible dans la boîte de portée (et, pour les deux dernières, des coups possibles).
pub(self) fn close_target(x: &mut Cx) -> bool {
    lib::target_in_box(x, 0, 0x28, 0x20)
}
pub(self) fn edge_target(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> bool {
    if !lib::target_in_box(x, d0, d1, d2) {
        return false;
    }
    if x.u16(f::TARGET) as i16 >= 0 {
        idle(x);
        return true;
    }
    let t = x.linked(f::TARGET);
    let same_side = t.u8(x.b, f::FLIP) == x.u8(f::FLIP);
    x.c.a[1] = t.0;
    x.call(if same_side { call::EDGE_SAME } else { call::EDGE_PAIR });
    !zero(x)
}

/// 0x951dc : pesanteur douce quand l'étape l'a demandée (`+0xa2`) : le boss retombe sur le sol.
fn gravity(x: &mut Cx) {
    if x.u8(FALLS) == 0 {
        return;
    }
    x.set8(FALLS, 0);
    x.move_z();
    if (x.u16(0x1a) as i16) >= 0 {
        x.set16(0x1a, (-0x18i16) as u16);
    }
    if (x.u16(0xc) as i16) < 0 {
        x.set16(0xc, 0);
        x.set16(0x18, 0x200);
        x.set16(0x1a, (-0x18i16) as u16);
    }
}

/// 0x95272 : joueurs devant le boss dans l'état 0x0200_0e00 (qu'il laisse ainsi sans défense) : ils reçoivent `+0x189`.
fn count_victims(x: &mut Cx) -> u32 {
    let mut n = 0;
    for k in 0..4u32 {
        let p = crate::objects::Obj(0xff85de + 0x200 * k);
        if p.u8(x.b, 0) == 0 {
            continue;
        }
        let mut d = p.u16(x.b, 0x8).wrapping_sub(x.u16(0x8));
        if x.u8(f::FLIP) == 0 {
            d = d.wrapping_neg();
        }
        if (d as i16) < 0 {
            continue;
        }
        if p.u32(x.b, f::STATE) & !0xff == 0x0200_0e00 {
            p.set_u8(x.b, 0x189, 1);
            n += 1;
        }
    }
    x.c.d[0] = n;
    n
}

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    x.set8(0xa4, 0);
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        _ => x.free(&pools::BOSS),
    }
}

// --- état 0 : création --------------------------------------------------------------------------

/// 0x93286 : attend la fin de la pause ; les variantes 0 et 1 sont choisies d'après la hauteur de la caméra.
fn init(x: &mut Cx) {
    if x.a5_u8(var::PAUSE) != 0 {
        return;
    }
    x.add8(f::STATE, 2);
    x.set_a5_u32(0x38, 0x0459_5900);
    x.set_a5_u8(0x5a, 0);
    if (x.u8(3) as i8) < 2 {
        let (variant, z) = if (x.a5_u16(var::SCROLL_Y) as i16) >= 0x130 { (0, 0x1f8) } else { (1, 0x140) };
        x.set8(3, variant);
        x.set16(0x10, z);
    }
    x.set8(f::SUB, 0);
    x.set16(f::SUB2, 0);
    x.set_a5_u8(0x50, 1);
    x.set8(0x93, 1);
    x.set8(0x9a, 0xff);
    pick_target(x);
    lib::draw_6b(x);
    x.set8(f::BOX_SET, 0x30);
    lib::initial_hp_boss(x);
    for r in [0xd5f30, 0xd6952, 0x19e10, 0x19db4, 0x19eb0] {
        x.call(r);
    }
    for (off, v) in [(0x70, 0), (0x8e, 3), (0x78, 0xff), (0x68, 8), (0x69, 8)] {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(0, 2);
    for (off, v) in [(0x6a, 3), (0x6c, 4), (0x6d, 8), (TIMER, 0)] {
        x.set8(off, v);
    }
    let speed = if x.u16(f::TARGET) != 0 && x.a5_u8(var::PLAYERS_ALIVE) != 1 { 7 } else { 6 };
    x.set8(0x72, speed);
    x.set8(0x73, speed);
    for (off, v) in [(0x74, 1), (0x75, 0x44), (0x76, 6), (0x7e, 0x4a), (0x7f, 0), (0x81, 0), (0x80, 0x1a), (0xbf, 0)] {
        x.set8(off, v);
    }
    effct_trio_2e(x);
    x.set16(ANIM_SET, 0xffff);
    x.c.d[0] = 0;
    anim_a(x, 0);
    lib::sound(x, 0xe1470);
}

// --- état 2 : actif -----------------------------------------------------------------------------

/// 0x933b8 : un groupe par image ; pendant une pause de scène, le boss reste en attente (0x020a0200) tant que des
/// joueurs sont sous son emprise.
fn active(x: &mut Cx) {
    x.set8(HIT_BUDGET, 5);
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if x.u8(f::SUB) == 0 || paused_dispatch(x) {
        group(x);
        gravity(x);
        if tst8(x, TIMER).gt() {
            sub8(x, TIMER, 1);
            x.set8(0x38, 0);
        }
    }
    lib::keep_on_screen(x, 0x40);
    x.call(SHADOW);
    x.reinsert();
}

/// 0x933d0..0x93452 : vrai si le groupe doit tourner cette image (hors pause, ou en attente de pause).
fn paused_dispatch(x: &mut Cx) -> bool {
    if x.a5_u8(var::PAUSE) == 0 {
        x.set8(PAUSE_SEEN, 0);
        return true;
    }
    let held = x.u32(f::STATE) & !0xff;
    if held == state::IDLE_PAUSED {
        return true;
    }
    if x.u8(PAUSE_SEEN) != 0 {
        return false;
    }
    x.set8(PAUSE_SEEN, 0xff);
    if count_victims(x) == 0 {
        return false;
    }
    x.set32(f::STATE, state::IDLE_PAUSED);
    true
}

fn group(x: &mut Cx) {
    match x.u8(f::SUB) {
        0 => moves::wake(x),
        2 => moves::fight(x),
        4 => attacks::run(x),
        6 => moves::jump_to_spot(x),
        8 => moves::before_attack(x),
        10 => moves::paused(x),
        12 => moves::holding(x),
        _ => {}
    }
}

/// 0x94e24 : choix de la suite d'une attaque : debout si la cible est hors de portée, sinon une nouvelle attaque tirée
/// dans la table de la difficulté (0x94e7c, une ligne de 32 octets par couple de niveaux).
pub(self) fn next_attack(x: &mut Cx) {
    if x.u16(f::TARGET) as i16 >= 0 {
        return idle(x);
    }
    if !lib::target_in_box(x, -0x40, 0x40, 0x20) {
        return x.set32(f::STATE, state::STAND);
    }
    let row = ((x.a5_u16(var::DIFFICULTY) >> 1) as u32) << 5;
    let r = rand_and(x, 0x1f);
    let step = x.data8(0x94e7c + row + r);
    x.set8(f::SUB2, step);
    x.set16(f::STATE, 0x0208);
    x.set8(0x7, 0);
}

handlers! {
    0x9325e "boss_16" run,
}
