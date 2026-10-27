//! Petites routines communes à plusieurs ennemis (bibliothèque 0x103d6-0x105b6 du programme), réécrites en
//! fonctions Rust : jeux d'animation avec recopie du type de coup, portes de décision de l'IA, tests de position.
//! Les originaux restent en 68000 pour leurs autres appelants.

use super::cx::Cx;
use super::enemy_common::*;
use crate::objects::{Courant, Obj};

/// 0x1046c : le type de coup `+0x44` suit la clé d'animation `+0x3e`.
fn copy_strike_kind(x: &mut Cx) {
    let k = x.u8(0x3e);
    x.set8(0x44, k);
}

/// 0x103d6 : démarre le script `n` du jeu `set`.
pub fn start(x: &mut Cx, set: u32, n: u32) {
    x.anim_start(set, n as u16);
    copy_strike_kind(x);
}

/// 0x103e0 : reprend l'animation de même clé (`+0x33`) dans le jeu `table`, en gardant la durée (`+0x30`).
pub fn switch_keeping_time(x: &mut Cx, table: u32) {
    let time = x.u16(0x30);
    let key = x.u8(0x33) & 0x7f;
    let mut at = table;
    while x.data8(at + 3) & 0x7f != key {
        at += 0x10;
    }
    x.c.a[4] = at;
    x.c.d[2] = x.c.d[2] & !0xffff | time as u32;
    x.c.d[0] = x.c.d[0] & !0xff | key as u32;
    let at = x.c.a[4];
    x.anim_start_entry(at);
    x.set16(0x30, time);
    copy_strike_kind(x);
}

/// 0x1040c : un pas d'animation ; vrai à la fin du script.
pub fn step(x: &mut Cx) -> bool {
    let end = x.anim_step();
    copy_strike_kind(x);
    end
}

/// 0x10474 : au moins un joueur en vie (`A5+0x35`).
pub fn players_alive(x: &mut Cx) -> bool {
    x.a5_u8(0x35) != 0
}

/// 0x104a2 : l'objet est hors de la zone `[caméra + gauche, caméra + droite]` (selon le mode deux écrans).
pub fn outside(x: &mut Cx, left: i16, right: i16) -> bool {
    let cam = if x.a5_u8(0x36) != 0 { x.a5_u16(0x5ae) } else { x.a5_u16(0x5a6) };
    let pos = x.u16(0x8) as i16;
    let (lo, hi) = (cam.wrapping_add(left as u16) as i16, cam.wrapping_add(right as u16) as i16);
    x.c.d[0] = x.c.d[0] & !0xffff | lo as u16 as u32;
    x.c.d[1] = x.c.d[1] & !0xffff | hi as u16 as u32;
    lo >= pos || hi <= pos
}

/// 0x1050c : vrai si l'ennemi regarde déjà vers sa cible (ou n'en a pas).
pub fn facing_target(x: &mut Cx) -> bool {
    if x.u16(f::TARGET) as i16 >= 0 {
        x.c.d[0] &= !0xff;
        return true;
    }
    let target = x.linked(f::TARGET);
    let dx = x.u16(0x8).wrapping_sub(target.u16(x.b, 0x8));
    let want = if (dx as i16) < 0 { 0x20 } else { 0 };
    x.c.a[1] = target.0 | 0xff00_0000;
    x.c.d[0] = x.c.d[0] & !0xff | want;
    x.c.d[1] = x.c.d[1] & !0xffff | dx as u32;
    want as u8 == x.u8(f::FLIP)
}

/// 0x10560 : le sens de la vitesse `+0x14` est celui de l'orientation.
pub fn moving_forward(x: &mut Cx) -> bool {
    let flip = (x.u8(f::FLIP) as u16).rotate_right(6);
    let sign = x.u16(0x14) & 0x8000;
    x.c.d[1] = flip as u32;
    x.c.d[2] = x.c.d[2] & !0xffff | sign as u32;
    flip == sign
}

/// 0x10574 : tourne l'angle `+0x2d` d'un cran tiré au hasard.
pub fn jitter_angle(x: &mut Cx) {
    let r = rand_and(x, 0xf);
    let d = x.rom8(0x1058e + r);
    x.c.d[0] = x.c.d[0] & !0xff | d as u32;
    let a = x.u8(0x2d).wrapping_add(d) & 0x1f;
    x.set8(0x2d, a);
}

/// 0x1059e : rang de PV (`+0xac`) selon le nombre de joueurs et la difficulté.
pub fn init_hp_rank(x: &mut Cx) {
    let players = x.a5_u8(0x35).wrapping_sub(1) << 5;
    let level = x.a5_u8(0x7f93) & 0x1e;
    x.c.d[0] = x.c.d[0] & !0xff | players.wrapping_add(level) as u32;
    x.set8(0xac, players.wrapping_add(level));
}

/// 0x105b6 : franchit le palier de PV suivant (table `A1`, un palier par rang) ; vrai si un palier est passé.
pub fn hp_threshold(x: &mut Cx, table: u32) -> bool {
    let n = x.u8(0xaa) as i8;
    if n >= x.u8(0xab) as i8 {
        x.c.d[0] &= !0xff;
        return false;
    }
    let rank = x.u8(0xac) as i8 as i16 as u16;
    let at = (n as i16 as u16).wrapping_shl(7).wrapping_add(rank);
    let limit = x.data16(table + at as u32);
    if (limit as i16) < (x.u16(0x62) as i16) {
        return false;
    }
    x.add8(0xaa, 1);
    true
}

/// 0x104e0 (non Z) : la cible regarde du même côté que l'ennemi, qui lui tourne le dos.
pub fn back_to_target(x: &mut Cx) -> bool {
    if x.u16(f::TARGET) as i16 >= 0 {
        return false;
    }
    let target = x.linked(f::TARGET);
    if x.u8(f::FLIP) != target.u8(x.b, f::FLIP) {
        return false;
    }
    let dx = x.u16(0x8).wrapping_sub(target.u16(x.b, 0x8));
    let want = if (dx as i16) < 0 { 0 } else { 0x20 };
    want != x.u8(f::FLIP)
}

/// 0x1052e : la cible regarde vers l'ennemi alors qu'elle est de son côté.
pub fn target_faces_me(x: &mut Cx) -> bool {
    if x.u16(f::TARGET) as i16 >= 0 {
        return false;
    }
    let target = x.linked(f::TARGET);
    let dx = x.u16(0x8).wrapping_sub(target.u16(x.b, 0x8)) & 0x8000;
    let d1 = (x.u8(f::FLIP) as u16).rotate_right(6) & dx;
    let want = if (d1 as i16) < 0 { 0 } else { 0x20 };
    want == target.u8(x.b, f::FLIP)
}

/// 0x1047a : décision d'attaque de l'IA : 0 rien, 1 ou 2 geste choisi selon le côté de la cible.
pub fn ai_gate(x: &mut Cx) -> u32 {
    let n = if target_faces_me(x) {
        x.call(0xa4b4);
        if x.c.cond(7) { 0 } else { 1 }
    } else if back_to_target(x) {
        x.call(0xa4c2);
        if x.c.cond(7) { 0 } else { 2 }
    } else {
        0
    };
    x.c.d[0] = n;
    n
}

// --- routines de l'IA d'autres modules -------------------------------------------------------------
//
// Les routines suivantes appartiennent à `ai.rs` et `ai_aids.rs` (lot D) et ne s'y appellent encore que par
// adresse (natives à contrat de registres). Chacune n'est appelée qu'ici : les familles du lot A appellent ces
// fonctions Rust, qui posent les registres d'entrée et rendent le résultat en valeur Rust. Les registres que la
// native laisse restent posés (des gestionnaires suivants en relisent certains).

/// Z de la dernière routine appelée par adresse.
fn z(x: &Cx) -> bool {
    x.c.cond(7)
}

/// `move.w #v,Dn` : seul le mot bas du registre change.
fn set_word(x: &mut Cx, n: usize, v: u16) {
    x.c.d[n] = x.c.d[n] & 0xffff_0000 | v as u32;
}

/// 0xa470 : tire l'attitude `+0x71` dans la ligne `+0x70` de la table d'attitudes (indexée par le compteur
/// d'images).
pub fn draw_stance(x: &mut Cx) {
    x.call(0xa470);
}

/// 0x788c : `+0x6b` tiré au hasard dans la ligne `+0x6a` de sa table.
pub fn draw_6b(x: &mut Cx) {
    x.call(0x788c);
}

/// 0x79f6 : dans l'écran, `+0x6e` est tiré dans la ligne `+0x6d` de sa table ; sinon il vaut 1.
pub fn draw_6e(x: &mut Cx) {
    x.call(0x79f6);
}

/// 0x79ce : si le bit tiré dans le masque de la ligne `+0x6c` est levé, comme `draw_6e` et vrai ; sinon faux.
pub fn maybe_draw_6e(x: &mut Cx) -> bool {
    x.call(0x79ce);
    !z(x)
}

/// 0xf118 : hors de l'écran, `+0x84` = 1 ; sinon `+0x84` = 0 et `+0x85` tiré dans la ligne `+0x8e`.
pub fn draw_84(x: &mut Cx) {
    x.call(0xf118);
}

/// 0x10112 : `+0x68` selon le niveau `A5+0x5a` et le nombre de joueurs en vie (8 sans niveau).
pub fn set_68(x: &mut Cx) {
    x.call(0x10112);
}

/// 0xac3a : `+0x77` tiré dans la table choisie par `+0x76` (indexée par le compteur d'images).
pub fn pick_byte(x: &mut Cx) {
    x.call(0xac3a);
}

/// 0xe76c : choisit le point d'approche de la cible `+0x50` (le plus proche ; de loin, le plus éloigné), le
/// mémorise et tourne l'objet vers lui.
pub fn approach_target(x: &mut Cx) {
    x.call(0xe76c);
}

/// 0xf0e8 : tourne d'un cran le cap `Obj::SPEED_INDEX` vers le point (`x`, `y`).
pub fn turn_toward(x: &mut Cx, px: u16, py: u16) {
    set_word(x, 1, px);
    set_word(x, 2, py);
    x.call(0xf0e8);
}

/// 0xfbae : garde l'objet dans l'écran (marge de 0x30 pixels) ; c'est le test de zone de fin d'image.
pub fn keep_in_zone(x: &mut Cx) {
    x.call(0xfbae);
}

/// 0x33fe4 : collision avec le décor (`terrain.rs`) : l'objet est repoussé hors d'un obstacle.
pub fn collide_terrain(x: &mut Cx) {
    x.call(0x33fe4);
}

/// 0xfbb0 : garde l'objet dans l'écran, à `margin` pixels près ; vrai s'il a été déplacé.
pub fn keep_on_screen(x: &mut Cx, margin: u16) -> bool {
    set_word(x, 0, margin);
    x.call(0xfbb0);
    x.c.d[5] == 1
}

/// 0x424e : choisit un joueur comme cible (`+0x50`, critère tiré au hasard) ; faux si aucun n'est ciblable.
pub fn pick_target(x: &mut Cx) -> bool {
    x.call(0x424e);
    !z(x)
}

/// 0x43ae : comme `pick_target`, avec la règle d'éligibilité large.
pub fn pick_target_any(x: &mut Cx) -> bool {
    x.call(0x43ae);
    !z(x)
}

/// 0x44ee : un joueur ciblable tiré au hasard (trois essais) devient la cible ; faux si aucun.
pub fn random_player(x: &mut Cx) -> bool {
    x.call(0x44ee);
    !z(x)
}

/// Critères de `target_by_criterion` (D0 de 0x43bc) : le plus proche, le moins de PV, le plus éloigné.
pub const NEAREST: u8 = 0;
pub const LOWEST_HP: u8 = 2;
pub const FARTHEST: u8 = 4;

/// 0x43bc : la cible devient le joueur qui répond au critère (`NEAREST`...) ; faux si aucun.
pub fn target_by_criterion(x: &mut Cx, criterion: u8) -> bool {
    x.c.d[0] = criterion as u32;
    x.call(0x43bc);
    !z(x)
}

/// 0x453c : PV initiaux d'un boss (table 0x458c), copiés en `+0x60`, `+0x62` et `+0x64`.
pub fn initial_hp_boss(x: &mut Cx) {
    x.call(0x453c);
}

/// 0x4544 : PV initiaux d'un ennemi (table 0x5c8c).
pub fn initial_hp(x: &mut Cx) {
    x.call(0x4544);
}

/// 0xfe5a : immunités (`+0x94`) selon la classe et le type.
pub fn set_immunities(x: &mut Cx) {
    x.call(0xfe5a);
}

/// 0x1059e : rang de PV `+0xac` selon le nombre de joueurs en vie et le compteur d'images.
pub fn spread_index(x: &mut Cx) {
    x.call(0x1059e);
}

/// Côté de la cible trouvée dans une boîte (D0 de 0xb5e0 et 0x7f19c).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InBox {
    No = 0,
    /// Du côté vers lequel l'objet regarde.
    Ahead = 1,
    Behind = 2,
}

impl InBox {
    fn from_d0(d0: u32) -> InBox {
        match d0 {
            0 => InBox::No,
            1 => InBox::Ahead,
            2 => InBox::Behind,
            _ => unreachable!("boîte : D0 = {d0} (0, 1 ou 2 seulement)"),
        }
    }
    pub fn found(self) -> bool {
        self != InBox::No
    }
}

/// 0xb5e0 : la cible `+0x50` (au sol) est-elle dans la boîte de décalage `dx` (retourné selon l'orientation), de
/// demi-largeur `half_w` et de demi-profondeur `half_d` ?
pub fn target_in_front(x: &mut Cx, dx: i16, half_w: u16, half_d: u16) -> InBox {
    set_word(x, 0, dx as u16);
    set_word(x, 1, half_w);
    set_word(x, 2, half_d);
    x.call(0xb5e0);
    InBox::from_d0(x.c.d[0])
}

/// `target_in_front` : la cible est-elle dans la boîte, d'un côté ou de l'autre ?
pub fn target_in_box(x: &mut Cx, dx: i16, half_w: u16, half_d: u16) -> bool {
    target_in_front(x, dx, half_w, half_d).found()
}

/// 0x7f19c : l'objet `other` est-il dans la boîte devant l'objet (mêmes paramètres que `target_in_front`) ?
pub fn in_box(x: &mut Cx, other: Obj, dx: i16, half_w: u16, half_d: u16) -> InBox {
    x.c.a[2] = other.0;
    set_word(x, 0, dx as u16);
    set_word(x, 1, half_w);
    set_word(x, 2, half_d);
    x.call(0x7f19c);
    InBox::from_d0(x.c.d[0])
}

/// 0x7ef4 : la cible, en garde (`+6` = 12), est-elle dans la boîte devant l'objet et lui fait-elle face ? Si oui
/// et après un tirage, l'objet passe à l'état 6 avec une parade tirée au hasard, et la fonction rend vrai.
pub fn guard_in_front(x: &mut Cx, dx: i16, half_w: u16, half_d: u16) -> bool {
    guard(x, 0x7ef4, dx, half_w, half_d)
}

/// 0x7f0c : comme `guard_in_front`, la garde pouvant aussi être 14, et la cible au sol.
pub fn guard_in_front_strict(x: &mut Cx, dx: i16, half_w: u16, half_d: u16) -> bool {
    guard(x, 0x7f0c, dx, half_w, half_d)
}

fn guard(x: &mut Cx, routine: u32, dx: i16, half_w: u16, half_d: u16) -> bool {
    set_word(x, 0, dx as u16);
    set_word(x, 1, half_w);
    set_word(x, 2, half_d);
    x.call(routine);
    !z(x)
}

/// 0xa4b4 (`+0x72`) : l'IA agit-elle à cette image ? (bit tiré dans un masque choisi par le compteur d'images.)
pub fn decision_a(x: &mut Cx) -> bool {
    x.call(0xa4b4);
    !z(x)
}

/// 0xa4c2 : comme `decision_a`, avec la table de `+0x73`.
pub fn decision_b(x: &mut Cx) -> bool {
    x.call(0xa4c2);
    !z(x)
}

/// 0xfc44 : un joueur est-il touché par la boîte d'attaque `attack` de l'objet ? Rend ce joueur.
pub fn attack_hits_a_player(x: &mut Cx, attack: u8) -> Option<Obj> {
    x.c.d[0] = attack as u32;
    x.call(0xfc44);
    (!z(x)).then(|| Obj(x.c.a[1]))
}

/// 0xfd64 : le joueur `player` est saisi par l'objet (animation de prise `anim`).
pub fn seize_player(x: &mut Cx, player: Obj, anim: u8) {
    x.c.a[1] = player.0;
    x.c.d[0] = anim as u32;
    x.call(0xfd64);
}

/// 0xfd8c : le joueur `player` est relâché (A1 = son adresse étendue en signe, comme après `movea.w`).
pub fn release_player(x: &mut Cx, player: Obj) {
    x.c.a[1] = player.0 | 0xff00_0000;
    x.call(0xfd8c);
}

/// 0xfda2 : le joueur saisi (`+0x50`) suit l'objet selon l'étape de l'animation de prise.
pub fn hold_seized(x: &mut Cx) {
    x.call(0xfda2);
}

/// 0xff54 : compte à rebours `+0x98`, puis nouvel état selon le nombre de joueurs et un tirage.
pub fn countdown_then_state(x: &mut Cx) {
    x.call(0xff54);
}

/// 0xfffc : paramètre de comportement `+0x98` lu selon le type.
pub fn behaviour_by_kind(x: &mut Cx) {
    x.call(0xfffc);
}

/// 0x10574 : le cap `Obj::SPEED_INDEX` change d'au plus un cran au hasard.
pub fn drift_heading(x: &mut Cx) {
    x.call(0x10574);
}

/// 0x105b6 : franchit le palier de PV suivant de la table `thresholds` ; vrai si un palier est passé.
pub fn next_threshold(x: &mut Cx, thresholds: u32) -> bool {
    x.c.a[1] = thresholds;
    x.call(0x105b6);
    !z(x)
}

/// 0x1050c : vrai si l'objet regarde vers sa cible (ou n'en a pas).
pub fn faces_target(x: &mut Cx) -> bool {
    x.call(0x1050c);
    z(x)
}

/// 0x10560 : vrai si la vitesse en X suit l'orientation.
pub fn moves_forward(x: &mut Cx) -> bool {
    x.call(0x10560);
    z(x)
}

/// 0x104a2 : vrai si l'objet est hors de la bande `[écran + left, écran + right]`.
pub fn outside_band(x: &mut Cx, left: i16, right: i16) -> bool {
    set_word(x, 0, left as u16);
    set_word(x, 1, right as u16);
    x.call(0x104a2);
    !z(x)
}

/// 0x10474 : au moins un joueur en vie.
pub fn any_player_alive(x: &mut Cx) -> bool {
    x.call(0x10474);
    !z(x)
}

/// Geste choisi par `turn_decision` (D0 de 0x1047a).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Turn {
    None = 0,
    /// Face à face, tirage A.
    FaceToFace = 1,
    /// Même côté, tirage B.
    SameSide = 2,
}

/// 0x1047a : décision de se tourner vers la cible.
pub fn turn_decision(x: &mut Cx) -> Turn {
    x.call(0x1047a);
    match x.c.d[0] {
        0 => Turn::None,
        1 => Turn::FaceToFace,
        2 => Turn::SameSide,
        d0 => unreachable!("0x1047a : D0 = {d0} (0, 1 ou 2 seulement)"),
    }
}

/// 0x1172e : jeu de débris n° `set` autour de l'objet.
pub fn debris(x: &mut Cx, set: u32) {
    x.c.d[5] = set;
    x.call(0x1172e);
}

/// 0x11a36 : cinq débris au sol (seconde table si `second` est non nul), puis le son 0xede08.
pub fn ground_debris(x: &mut Cx, second: u8) {
    x.c.d[0] = x.c.d[0] & !0xff | second as u32;
    x.call(0x11a36);
}

/// 0xb648 : une seule fois par objet, selon un tirage, lâche l'objet propre à son type.
pub fn drop_once(x: &mut Cx) {
    x.call(0xb648);
}

/// 0xb780 : butin de l'objet (ligne `+0x80`).
pub fn drop_loot(x: &mut Cx) {
    x.call(0xb780);
}

/// 0xbad6 : objet spécial lâché par l'objet (ligne `+0x80`).
pub fn drop_special(x: &mut Cx) {
    x.call(0xbad6);
}

/// 0xe2f6 : trésor de l'objet (ligne `+0x80`).
pub fn drop_treasure(x: &mut Cx) {
    x.call(0xe2f6);
}

/// 0x115e2 : le boss pousse les joueurs présents devant lui.
pub fn push_players(x: &mut Cx) {
    x.call(0x115e2);
}

/// 0x103ba : éclair d'écran, secousse et effet d'impact.
pub fn flash_and_shake(x: &mut Cx) {
    x.call(0x103ba);
}

/// 0x3e480 : routine d'atterrissage propre au type de l'objet.
pub fn landing_by_kind(x: &mut Cx) {
    x.call(0x3e480);
}

/// 0x10428 : démarre l'animation du jeu `set` dont la marque est la variante (`+0x33`) du maître (`+0xb2`).
pub fn animation_of_master(x: &mut Cx, set: u32) {
    x.c.a[4] = set;
    x.call(0x10428);
}

/// 0x10450 : démarre l'animation du jeu `set` dont la marque est celle de `+0x3e` du maître.
pub fn animation_of_mark(x: &mut Cx, set: u32) {
    x.c.a[4] = set;
    x.call(0x10450);
}

/// 0x2824, 0x2860, 0x289c : secousse de la caméra en service pendant `frames` images, dans le mode 0x54, 0x58 ou
/// 0x5c (`camera_modes.rs`).
pub fn shake_54(x: &mut Cx, frames: u16) {
    shake(x, 0x2824, frames)
}
pub fn shake_58(x: &mut Cx, frames: u16) {
    shake(x, 0x2860, frames)
}
pub fn shake_5c(x: &mut Cx, frames: u16) {
    shake(x, 0x289c, frames)
}
fn shake(x: &mut Cx, routine: u32, frames: u16) {
    x.c.d[0] = frames as u32;
    x.call(routine);
}

/// Points de marche autour de la cible : 0xe886 (le plus éloigné, sauf le dernier), 0xe914 (atteint : un autre),
/// 0xeff2 (le plus proche, retourné selon la cible), 0xf084 (le plus éloigné), 0xe7fa (le plus proche, numéroté).
pub fn choose_walk_point(x: &mut Cx) {
    x.call(0xe886);
}
pub fn walk_point_reached(x: &mut Cx) {
    x.call(0xe914);
}
pub fn walk_near_point(x: &mut Cx) {
    x.call(0xeff2);
}
pub fn walk_far_point(x: &mut Cx) {
    x.call(0xf084);
}
pub fn choose_near_walk_point(x: &mut Cx) {
    x.call(0xe7fa);
}

// --- sons ---------------------------------------------------------------------------------------------

/// Joue le son dont `wrapper` est la petite routine d'origine (table `sound_table.rs` : commande, canal et
/// panoramique selon la position de l'objet). En attendant une API de `sound.rs`, la routine est appelée par son
/// adresse ; elle rend tous les registres intacts.
pub fn sound(x: &mut Cx, wrapper: u32) {
    x.call(wrapper);
}

// --- créateurs d'objets et services d'autres modules ---------------------------------------------------
//
// Créateurs de `creators.rs`, `effects_creators.rs`, `effects_aids.rs` (lot B), événements (`evenements.rs`,
// `popups.rs`), palette : appelés par adresse ici seulement, en attendant leur API.

/// 0xd5f30 : en partie, objet SET 0x14 lié au boss (barre de vie, retenue en `A5-0x5b8a`).
pub fn create_set_14(x: &mut Cx) {
    x.call(0xd5f30);
}

/// 0xd6952 : objet SET 0x15 (variante 2) dont `+0x42` dépend du type du créateur.
pub fn create_set_15(x: &mut Cx) {
    x.call(0xd6952);
}

/// 0xd5ef8 : barre de vie du boss (objet SET).
pub fn create_life_bar(x: &mut Cx) {
    x.call(0xd5ef8);
}

/// 0x19ad6 : événement de l'objet (type `+0x7e`, joueur `+0x48`) dans l'anneau des points.
pub fn object_event(x: &mut Cx) {
    x.call(0x19ad6);
}

/// 0x19e10 : efface le pop-up courant.
pub fn clear_popup(x: &mut Cx) {
    x.call(0x19e10);
}

/// 0x19db4 : vide la file des pop-ups.
pub fn empty_popups(x: &mut Cx) {
    x.call(0x19db4);
}

/// 0x19eb0 : efface tous les pop-ups affichés.
pub fn clear_all_popups(x: &mut Cx) {
    x.call(0x19eb0);
}

/// 0xb10c8 : effet de choc à la position de l'objet : variante, réglage (`+0x2d`, `+0x2f`), vitesses `+0x14`, `+0x1c`.
pub fn shock(x: &mut Cx, variant: u32, setting: u16, vx: u16, vy: u16) {
    x.c.d[0] = variant;
    set_word(x, 1, setting);
    set_word(x, 2, vx);
    set_word(x, 3, vy);
    x.call(0xb10c8);
}

/// 0xabec8 : effet d'attaque (EFFCT 11) lié par `+0x5c`, de variante et réglage donnés.
pub fn attack_effect(x: &mut Cx, variant: u32, setting: u32) {
    x.c.d[0] = variant;
    x.c.d[1] = setting;
    x.call(0xabec8);
}

/// 0xab7d8 : gerbe de débris (EFFCT 6) de variante donnée, liée par `+0x50`.
pub fn debris_spray(x: &mut Cx, variant: u32) {
    x.c.d[0] = variant;
    x.call(0xab7d8);
}
