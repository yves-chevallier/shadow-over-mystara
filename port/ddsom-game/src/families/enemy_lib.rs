//! Façade des aides communes aux ennemis et aux boss : les familles du lot A appellent ces fonctions (`lib::…`),
//! qui appellent les cœurs Rust des routines d'origine (`ai`, `ai_aids`, `camera_modes`, `creators`, `evenements`,
//! `popups`...) sur l'objet traité. Entrées en arguments, résultats en valeurs Rust ; aucun registre.
//!
//! Les rares registres que la suite relit encore (glitches d'origine) sont écrits en clair par l'appelant, à
//! l'endroit qui les relit (voir `docs/refactorisation.md`, lot IA).

use crate::families::cx::Banc;
use super::cx::Cx;
use super::enemy_common::*;
use crate::ai;
use crate::ai_aids;
use crate::machine::Machine;
use crate::objects::{Courant, Obj};

/// 0x1046c : le type de coup `+0x44` suit la clé d'animation `+0x3e`.
fn copy_strike_kind<R: Banc>(x: &mut Cx<'_, R>) {
    let k = x.u8(0x3e);
    x.set8(0x44, k);
}

/// 0x103d6 : démarre le script `n` du jeu `set`.
pub fn start<R: Banc>(x: &mut Cx<'_, R>, set: u32, n: u32) {
    x.anim_start(set, n as u16);
    copy_strike_kind(x);
}

/// 0x103e0 : reprend l'animation de même clé (`+0x33`) dans le jeu `set`, en gardant la durée (`+0x30`).
pub fn switch_keeping_time<R: Banc>(x: &mut Cx<'_, R>, set: u32) {
    ai_aids::animation_by_variant(x.b, x.o, set);
}

/// 0x1040c : un pas d'animation ; vrai à la fin du script.
pub fn step<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    let end = x.anim_step();
    copy_strike_kind(x);
    end
}

/// 0x10474 : au moins un joueur en vie (`A5+0x35`).
pub fn players_alive<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    ai_aids::players_alive(x.b)
}

/// 0x104a2 : vrai si l'objet est hors de la bande `[écran + left, écran + right]`.
pub fn outside_band<R: Banc>(x: &mut Cx<'_, R>, left: i16, right: i16) -> bool {
    ai_aids::outside_band(x.b, x.o, left as u16, right as u16)
}

/// 0x1050c : vrai si l'objet regarde vers sa cible (ou n'en a pas).
pub fn facing_target<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    ai::facing_target(x.b, x.o)
}

/// 0x10560 : vrai si la vitesse en X suit l'orientation.
pub fn moving_forward<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    ai::moving_forward(x.b, x.o)
}

/// 0x10574 : le cap `Obj::SPEED_INDEX` change d'au plus un cran au hasard.
pub fn drift_heading<R: Banc>(x: &mut Cx<'_, R>) {
    ai::drift_heading(x.b, x.o);
}

/// 0x1059e : rang de PV `+0xac` selon le nombre de joueurs en vie et le rang.
pub fn spread_index<R: Banc>(x: &mut Cx<'_, R>) {
    ai::hp_rank(x.b, x.o);
}

/// 0x105b6 : franchit le palier de PV suivant de la table `thresholds` ; vrai si un palier est passé.
pub fn next_threshold<R: Banc>(x: &mut Cx<'_, R>, thresholds: u32) -> bool {
    ai_aids::next_threshold(x.b, x.o, thresholds)
}

/// Geste choisi par `turn_decision` (0x1047a).
pub use crate::ai::Turn;

/// 0x1047a : décision de l'IA selon le côté de la cible.
pub fn turn_decision<R: Banc>(x: &mut Cx<'_, R>) -> Turn {
    ai::turn_decision(x.b, x.o)
}

/// 0xa470 : tire l'attitude `+0x71` dans la ligne `+0x70` de la table d'attitudes (indexée par le rang).
pub fn draw_stance<R: Banc>(x: &mut Cx<'_, R>) {
    ai_aids::stance(x.b, x.o);
}

/// 0x788c : `+0x6b` tiré au hasard dans la ligne `+0x6a` de sa table.
pub fn draw_6b<R: Banc>(x: &mut Cx<'_, R>) {
    ai_aids::draw_6b(x.b, x.o);
}

/// 0x79f6 : dans l'écran, `+0x6e` est tiré dans la ligne `+0x6d` de sa table ; sinon il vaut 1.
pub fn draw_6e<R: Banc>(x: &mut Cx<'_, R>) {
    ai_aids::draw_6e(x.b, x.o);
}

/// 0x79ce : si le bit tiré dans le masque de la ligne `+0x6c` est levé, comme `draw_6e` et vrai ; sinon faux.
pub fn maybe_draw_6e<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    ai_aids::maybe_draw_6e(x.b, x.o)
}

/// 0xf118 : hors de l'écran, `+0x84` = 1 ; sinon `+0x84` = 0 et `+0x85` tiré dans la ligne `+0x8e`.
pub fn draw_84<R: Banc>(x: &mut Cx<'_, R>) {
    ai_aids::draw_84(x.b, x.o);
}

/// 0x10112 : `+0x68` selon le niveau `A5+0x5a` et le nombre de joueurs en vie (8 sans niveau).
pub fn set_68<R: Banc>(x: &mut Cx<'_, R>) {
    ai_aids::set_68(x.b, x.o);
}

/// 0xac3a : `+0x77` tiré dans la table choisie par `+0x76` (indexée par le rang).
pub fn pick_byte<R: Banc>(x: &mut Cx<'_, R>) {
    ai::pick_byte(x.b, x.o);
}

/// 0xe76c : vise le point d'approche de la cible `+0x50` (le plus proche ; loin de l'écran, le plus éloigné) et
/// tourne l'objet d'un cran vers lui.
pub fn approach_target<R: Banc>(x: &mut Cx<'_, R>) {
    ai_aids::approach_target(x.b, x.o);
}

/// 0xf0e8 : tourne d'un cran le cap `Obj::SPEED_INDEX` vers le point (`px`, `py`).
pub fn turn_toward<R: Banc>(x: &mut Cx<'_, R>, px: u16, py: u16) {
    ai::turn_toward(x.b, x.o, (px, py));
}

/// 0xfbae : garde l'objet dans l'écran (marge de 0x30 pixels) ; c'est le test de zone de fin d'image. Vrai s'il a
/// été déplacé.
pub fn keep_in_zone(x: &mut Cx) -> bool {
    ai::keep_in_zone_d5(x.c, x.b, x.o)
}

/// 0x33fe4 : collision avec le décor (`terrain.rs`) : l'objet est repoussé hors d'un obstacle.
/// Glitch d'origine : les registres de la recherche du terrain (D0-D3, D5 = 1 en cas de choc, A1, A2) traversent
/// les gestionnaires ; l'étincelle de `flash_and_shake` en tire ses vitesses (D2, D3 : `long_route_14_2p`), le
/// squelette relit D5 : la routine travaille donc sur les registres de l'original.
pub fn collide_terrain(x: &mut Cx) {
    crate::terrain::push_68k(x.c, x.b, x.o);
}

/// 0xfbb0 : garde l'objet dans l'écran, à `margin` pixels près ; vrai s'il a été déplacé.
pub fn keep_on_screen(x: &mut Cx, margin: u16) -> bool {
    ai::keep_on_screen_d5(x.c, x.b, x.o, margin)
}

/// 0x424e : choisit un joueur comme cible (`+0x50`, critère tiré au hasard) ; rend ce joueur.
pub fn pick_target(x: &mut Cx) -> Option<Obj> {
    let t = ai::pick_target(x.b, x.o, ai::Rule::Strict);
    found_d5(x, t)
}

/// D5 = 2 si un joueur est trouvé, 0 sinon (`moveq`) : le D5 de l'original traverse les gestionnaires (voir
/// `ai::keep_on_screen_d5`).
fn found_d5(x: &mut Cx, t: Option<Obj>) -> Option<Obj> {
    x.c.d[5] = if t.is_some() { 2 } else { 0 };
    t
}

/// 0x43ae : comme `pick_target`, avec la règle d'éligibilité large.
pub fn pick_target_any(x: &mut Cx) -> bool {
    let t = ai::pick_target(x.b, x.o, ai::Rule::Lenient);
    found_d5(x, t).is_some()
}

/// 0x44ee : un joueur ciblable tiré au hasard (trois essais) devient la cible ; faux si aucun.
pub fn random_player<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    ai::random_player(x.b, x.o).is_some()
}

/// Critères de `target_by_criterion` (D0 de 0x43bc) : le plus proche, le moins de PV, le plus éloigné.
pub const NEAREST: u8 = 0;
pub const LOWEST_HP: u8 = 2;
pub const FARTHEST: u8 = 4;

/// 0x43bc : la cible devient le joueur qui répond au critère (`NEAREST`...) ; rend ce joueur.
pub fn target_by_criterion(x: &mut Cx, criterion: u8) -> Option<Obj> {
    let t = ai::target_by(x.b, x.o, ai::Criterion::from_byte(criterion), ai::Rule::Lenient);
    found_d5(x, t)
}

/// 0x453c : PV initiaux d'un boss (table 0x458c), copiés en `+0x60`, `+0x62` et `+0x64`.
pub fn initial_hp_boss<R: Banc>(x: &mut Cx<'_, R>) {
    ai::initial_hp(x.b, x.o, ai::HP_TABLE_BOSS);
}

/// 0x4544 : PV initiaux d'un ennemi (table 0x5c8c).
pub fn initial_hp<R: Banc>(x: &mut Cx<'_, R>) {
    ai::initial_hp(x.b, x.o, ai::HP_TABLE_ENEMY);
}

/// 0xfe5a : immunités (`+0x94`) selon la classe et le type. Rend l'index lu dans la table (D0 de l'original, que la
/// harpie relit).
pub fn set_immunities<R: Banc>(x: &mut Cx<'_, R>) -> u16 {
    ai::set_immunities(x.b, x.o)
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
    pub fn found(self) -> bool {
        self != InBox::No
    }
}

impl From<ai_aids::DansBoite> for InBox {
    fn from(d: ai_aids::DansBoite) -> InBox {
        match d {
            ai_aids::DansBoite::Non => InBox::No,
            ai_aids::DansBoite::Face => InBox::Ahead,
            ai_aids::DansBoite::Dos => InBox::Behind,
        }
    }
}

/// La boîte de décalage `dx` (retourné selon l'orientation), de demi-largeur `half_w` et de demi-profondeur
/// `half_d`.
fn boite(dx: i16, half_w: u16, half_d: u16) -> ai_aids::Boite {
    ai_aids::Boite { decalage: dx as u16, demi_largeur: half_w, demi_profondeur: half_d }
}

/// 0xb5e0 : la cible `+0x50` (au sol) est-elle dans la boîte devant l'objet ?
///
/// Les tests de boîte laissent leurs restes dans D0-D3 (et A1, A4 pour la garde) comme l'original : D2 et D3
/// traversent les gestionnaires et donnent ses vitesses à l'étincelle de `flash_and_shake` (glitch d'origine, vu sur
/// `long_route_14_2p`) ; le boss 11 relit D2 (`roll`).
pub fn target_in_front(x: &mut Cx, dx: i16, half_w: u16, half_d: u16) -> InBox {
    ai_aids::target_in_front_68k(x.c, x.b, x.o, boite(dx, half_w, half_d)).into()
}

/// `target_in_front` : la cible est-elle dans la boîte, d'un côté ou de l'autre ?
pub fn target_in_box(x: &mut Cx, dx: i16, half_w: u16, half_d: u16) -> bool {
    target_in_front(x, dx, half_w, half_d).found()
}

/// 0x7f19c : l'objet `other` est-il dans la boîte devant l'objet (mêmes paramètres que `target_in_front`) ?
pub fn in_box(x: &mut Cx, other: Obj, dx: i16, half_w: u16, half_d: u16) -> InBox {
    ai_aids::box_around_68k(x.c, x.b, x.o, other, boite(dx, half_w, half_d)).into()
}

/// 0x7ef4 : la cible, en garde (`+6` = 12), est-elle dans la boîte devant l'objet et lui fait-elle face ? Si oui
/// et après un tirage, l'objet passe à l'état 6 avec une parade tirée au hasard, et la fonction rend vrai.
pub fn guard_in_front(x: &mut Cx, dx: i16, half_w: u16, half_d: u16) -> bool {
    ai_aids::in_front_68k(x.c, x.b, x.o, false, boite(dx, half_w, half_d))
}

/// 0x7f0c : comme `guard_in_front`, la garde pouvant aussi être 14, et la cible au sol.
pub fn guard_in_front_strict(x: &mut Cx, dx: i16, half_w: u16, half_d: u16) -> bool {
    ai_aids::in_front_68k(x.c, x.b, x.o, true, boite(dx, half_w, half_d))
}

/// 0xa4b4 (`+0x72`) : l'IA agit-elle à cette image ? (bit tiré dans un masque choisi par le rang.)
pub fn decision_a<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    ai::decision_a(x.b, x.o)
}

/// 0xa4c2 : comme `decision_a`, avec la table de `+0x73`.
pub fn decision_b<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    ai::decision_b(x.b, x.o)
}

/// Décision de l'IA selon le côté de la cible : `decision_b` si elle regarde du même côté que l'objet, sinon
/// `decision_a`.
pub fn decision<R: Banc>(x: &mut Cx<'_, R>, same_side: bool) -> bool {
    if same_side { decision_b(x) } else { decision_a(x) }
}

/// 0x57c66 (`creators.rs`) : l'objet est-il dans la moitié droite de l'écran (x >= caméra + 0xc0) ?
pub fn in_right_half<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    super::creators::in_right_half(x.b, x.o)
}

/// 0x5cdc6 (`creators.rs`) : tirage au sort, vrai 10 fois sur 16.
pub fn chance_10_16<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    super::creators::chance_10_16(x.b)
}

/// 0x1386 (`aides::direction`) : direction (0 à 31) de `de` vers `vers` ; D0 = la direction, comme l'original
/// (des appelants la relisent).
pub fn aim(x: &mut Cx, de: (u16, u16), vers: (u16, u16)) -> u8 {
    let a = crate::aides::direction(x.b, de, vers);
    x.c.d[0] = a as u32;
    a
}

/// 0x137e : direction de l'objet vers le point `vers` (D0 posé).
pub fn aim_from(x: &mut Cx, vers: (u16, u16)) -> u8 {
    let a = crate::aides::direction_from(x.b, x.o, vers);
    x.c.d[0] = a as u32;
    a
}

/// 0x1376 : direction de l'objet vers `cible` (D0 posé).
pub fn aim_to(x: &mut Cx, cible: Obj) -> u8 {
    let a = crate::aides::direction_to(x.b, x.o, cible);
    x.c.d[0] = a as u32;
    a
}

/// 0xfc44 : un joueur est-il touché par la boîte d'attaque `attack` de l'objet ? Rend ce joueur.
pub fn attack_hits_a_player<R: Banc>(x: &mut Cx<'_, R>, attack: u8) -> Option<Obj> {
    ai_aids::attack_hits_a_player(x.b, x.o, attack)
}

/// 0xfd64 : le joueur `player` est saisi par l'objet (animation de prise `anim`).
pub fn seize_player<R: Banc>(x: &mut Cx<'_, R>, player: Obj, anim: u8) {
    ai_aids::seize_player(x.b, x.o, player, anim);
}

/// 0xfd8c : le joueur `player` est relâché.
pub fn release_player(_x: &mut Cx, player: Obj) {
    ai_aids::release_player(_x.b, player);
}

/// 0xfda2 : le joueur saisi (`+0x50`) suit l'objet selon l'étape de l'animation de prise.
pub fn hold_seized<R: Banc>(x: &mut Cx<'_, R>) {
    ai_aids::hold_seized(x.b, x.o);
}

/// 0xff54 : compte à rebours `+0x98`, puis nouvel état selon le nombre de joueurs et un tirage.
pub fn countdown_then_state<R: Banc>(x: &mut Cx<'_, R>) {
    ai_aids::countdown_then_state(x.b, x.o);
}

/// 0xfffc : paramètre de comportement `+0x98` lu selon le type.
pub fn behaviour_by_kind<R: Banc>(x: &mut Cx<'_, R>) {
    ai_aids::behaviour_by_kind(x.b, x.o);
}

/// 0x1172e : jeu de débris n° `set` autour de l'objet.
pub fn debris(x: &mut Cx, set: u32) {
    ai_aids::debris_set(x.b, x.o, set as u16);
    // D5 = le compteur épuisé du `dbf` (D5 traverse les gestionnaires, voir `ai::keep_on_screen_d5`)
    x.c.d[5] = 0xffff;
}

/// 0x11a36 : cinq débris au sol (seconde table si `second` est non nul), puis le son 0xede08.
pub fn ground_debris(x: &mut Cx, second: u8) {
    ai_aids::ground_debris(x.b, x.o, second != 0);
    // D5.w = le compteur épuisé du `dbf` (D5 traverse les gestionnaires, voir `ai::keep_on_screen_d5`)
    x.c.d[5] |= 0xffff;
}

/// 0xb648 : une seule fois par objet, selon un tirage, lâche l'objet propre à son type.
pub fn drop_once<R: Banc>(x: &mut Cx<'_, R>) {
    ai_aids::drop_once(x.b, x.o);
}

/// Butin de fin d'un ennemi ou d'un boss : 0xb780, 0xbad6 puis 0xe2f6.
/// Glitch d'origine : D2 et D3 laissés par ces routines (la ligne de trésor, la ligne de difficulté du butin spécial)
/// donnent ses vitesses à l'étincelle de `flash_and_shake`, qui suit chez la plupart des boss, et D5 (compteur de la
/// dernière boucle) traverse les gestionnaires jusqu'à 0xa17d0 (`ai::keep_on_screen_d5`) : on les rend en clair.
pub fn drop_rewards(x: &mut Cx) {
    ai_aids::drop_loot(x.b, x.o);
    let d3 = ai_aids::drop_special(x.b, x.o);
    let (d2, d5) = ai_aids::drop_treasure(x.b, x.o);
    // relu : vitesses de l'étincelle de 0x103ba (`flash_and_shake`)
    x.c.d[2] = x.c.d[2] & !0xffff | d2 as u32;
    if let Some(d3) = d3 {
        x.c.d[3] = x.c.d[3] & !0xffff | d3 as u32;
    }
    if let Some(d5) = d5 {
        x.c.d[5] = d5;
    }
}

/// 0x115e2 : le boss pousse les joueurs présents devant lui.
pub fn push_players<R: Banc>(x: &mut Cx<'_, R>) {
    ai_aids::push_players(x.b, x.o);
}

/// 0x103ba : éclair d'écran, secousse et étincelle d'impact.
/// Glitch d'origine : les vitesses de l'étincelle sont les D2 et D3 laissés par ce qui précède (`drop_rewards`, ou
/// le gestionnaire) : ils sont lus ici en clair.
pub fn flash_and_shake(x: &mut Cx) {
    let (vx, ax) = (x.c.d[2] as u16, x.c.d[3] as u16);
    ai_aids::flash_and_shake(x.b, x.o, vx, ax);
}

/// 0x3e480 : routine d'atterrissage propre au type de l'objet (table de la ROM, appelée par son adresse).
pub fn landing_by_kind(x: &mut Cx) {
    if let Some((routine, index)) = ai_aids::landing_routine(x.b, x.o) {
        // `jmp (A1)` : la routine trouve D0 = l'index et A1 = elle-même
        x.c.d[0] = index as u32;
        x.c.a[1] = routine;
        x.call(routine);
    }
}

/// 0x10428 : démarre l'animation du jeu `set` dont la marque est la variante (`+0x33`) du maître (`+0xb2`).
pub fn animation_of_master<R: Banc>(x: &mut Cx<'_, R>, set: u32) {
    ai_aids::animation_of_master(x.b, x.o, set);
}

/// 0x10450 : démarre l'animation du jeu `set` dont la marque est celle de `+0x3e` du maître.
pub fn animation_of_mark<R: Banc>(x: &mut Cx<'_, R>, set: u32) {
    ai_aids::animation_of_mark(x.b, x.o, set);
}

/// 0x2824, 0x2860, 0x289c : secousse de la caméra en service pendant `frames` images, dans le mode 0x54, 0x58 ou
/// 0x5c (`camera_modes.rs`).
pub fn shake_54<R: Banc>(x: &mut Cx<'_, R>, frames: u16) {
    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::A, frames);
}
pub fn shake_58<R: Banc>(x: &mut Cx<'_, R>, frames: u16) {
    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, frames);
}
pub fn shake_5c<R: Banc>(x: &mut Cx<'_, R>, frames: u16) {
    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::C, frames);
}

/// Points de marche autour de la cible : 0xe886 (le plus éloigné, sauf le dernier), 0xe914 (atteint : un autre),
/// 0xeff2 (le plus proche, retourné selon la cible), 0xf084 (le plus éloigné), 0xe7fa (le plus proche, numéroté).
pub fn choose_walk_point<R: Banc>(x: &mut Cx<'_, R>) {
    ai_aids::choose_walk_point(x.b, x.o);
}
pub fn walk_point_reached<R: Banc>(x: &mut Cx<'_, R>) {
    ai_aids::walk_point_reached(x.b, x.o);
}
pub fn walk_near_point<R: Banc>(x: &mut Cx<'_, R>) {
    ai_aids::walk_near_point(x.b, x.o);
}
pub fn walk_far_point<R: Banc>(x: &mut Cx<'_, R>) {
    ai_aids::walk_far_point(x.b, x.o);
}
pub fn choose_near_walk_point<R: Banc>(x: &mut Cx<'_, R>) {
    ai_aids::choose_near_walk_point(x.b, x.o);
}

// --- sons ---------------------------------------------------------------------------------------------

/// Joue le son `wrapper` (petite routine d'origine de la table `sound_table.rs`) pour l'objet.
pub fn sound<R: Banc>(x: &mut Cx<'_, R>, wrapper: u32) {
    crate::sound::jouer(x.b, wrapper, x.o);
}

// --- créateurs d'objets et services d'autres modules ---------------------------------------------------

/// 0xd5f30 : en partie, objet SET 0x14 lié au boss (barre de vie, retenue en `A5-0x5b8a`).
pub fn create_set_14<R: Banc>(x: &mut Cx<'_, R>) {
    super::creators::set_14(x.b, x.o);
}

/// 0xd6952 : objet SET 0x15 (variante 2) dont `+0x42` dépend du type du créateur.
pub fn create_set_15<R: Banc>(x: &mut Cx<'_, R>) {
    super::creators::set_15_b(x.b, x.o);
}

/// 0xd5ef8 : la barre de vie affichée est rattachée au boss.
pub fn create_life_bar<R: Banc>(x: &mut Cx<'_, R>) {
    super::set_residu::attach_life_bar(x.b, x.o);
}

/// 0x19ad6 : événement de l'objet (type `+0x7e`, joueur `+0x48`) dans l'anneau des points.
pub fn object_event<R: Banc>(x: &mut Cx<'_, R>) {
    crate::evenements::evenement_objet(x.b, x.o);
}

/// 0x19e10 : efface le pop-up courant.
pub fn clear_popup<R: Banc>(x: &mut Cx<'_, R>) {
    crate::evenements::effacer_popup_courant(x.b);
}

/// 0x19db4 : vide la file des pop-ups.
pub fn empty_popups<R: Banc>(x: &mut Cx<'_, R>) {
    crate::evenements::vider_popups(x.b);
}

/// 0x19eb0 : efface tous les pop-ups affichés.
pub fn clear_all_popups<R: Banc>(x: &mut Cx<'_, R>) {
    crate::popups::effacer_tout(x.b);
}

/// 0xb10c8 : effet de choc à la position de l'objet : variante, réglage (`+0x2d`, `+0x2f`), vitesses `+0x14`, `+0x1c`.
#[track_caller]
pub fn shock(x: &mut Cx, variant: u32, setting: u16, vx: u16, vy: u16) {
    // A1, D1 et la retenue de l'original restent : un gestionnaire suivant les relit (boss15_scene46, mash_c5)
    crate::effects_aids::shock_68k(x.c, x.b, x.o, variant as u8, setting, vx, vy);
}

/// 0xabec8 : effet d'attaque (EFFCT 11) lié par `+0x5c`, de variante et réglage donnés.
pub fn attack_effect<R: Banc>(x: &mut Cx<'_, R>, variant: u32, setting: u32) {
    super::effects_creators::attack(x.b, x.o, variant as u8, setting as u8);
}

/// Série d'effets d'attaque (0xabec8) de variante `variant`, réglages `first` à 0 (`dbf D1`). Faux si le pool a
/// refusé un effet (la série s'arrête là).
pub fn attack_effects(x: &mut Cx, variant: u32, first: u32) -> bool {
    x.c.d[0] = variant;
    x.c.d[1] = first;
    loop {
        if super::effects_creators::attack(x.b, x.o, variant as u8, x.c.d[1] as u8).is_none() {
            return false;
        }
        let n = (x.c.d[1] as u16).wrapping_sub(1);
        x.c.d[1] = x.c.d[1] & !0xffff | n as u32; // relu : compteur de boucle relu au tour suivant
        if n == 0xffff {
            return true;
        }
    }
}

/// 0xab7d8 : gerbe de débris (EFFCT 6) de variante donnée, liée par `+0x50`.
pub fn debris_spray<R: Banc>(x: &mut Cx<'_, R>, variant: u32) {
    super::effects_creators::debris(x.b, x.o, variant as u8);
}

// --- routines d'origine sans native, jamais atteintes par les scénarios ---------------------------------
//
// Appelées par adresse depuis les familles du lot A, elles n'avaient pas de native : le jeu sans 68000 se serait
// arrêté en les atteignant. Transcrites de `listing.txt` (registres de sortie de l'original posés), non validées
// par un scénario (voir docs/code-mort.md, lot A).

/// Copie des positions et de l'orientation de l'objet vers un tir créé.
fn copy_place<R: Banc>(x: &mut Cx<'_, R>, o: Obj, with_z: bool) {
    let (px, py) = (x.u32(Obj::X), x.u32(Obj::Y));
    o.set_u32(x.b, Obj::X, px);
    o.set_u32(x.b, Obj::Y, py);
    if with_z {
        let pz = x.u32(Obj::Z);
        o.set_u32(x.b, Obj::Z, pz);
    }
}

/// Allocation ESHL de ces créateurs (`jsr 0x22ea ; bcs fin`) : A1 = l'objet comme l'original.
fn alloc_eshl<R: Banc>(x: &mut Cx<'_, R>) -> Option<Obj> {
    let o = x.alloc(&crate::objects::pools::ESHL)?;
    Some(o)
}

/// 0xa09ca : tir ESHL 0x12 posé 0x30 pixels au-dessus de l'objet et 0x60 pixels devant lui. Rend le tir et la
/// retenue finale de l'original.
/// Glitch d'origine : la retenue que l'appelant teste ensuite (`bcc`) est celle de l'addition de la position en X,
/// pas celle de l'allocation, quand le tir a pu être créé.
pub fn eshl_12<R: Banc>(x: &mut Cx<'_, R>) -> (Option<Obj>, bool) {
    let Some(o) = alloc_eshl(x) else { return (None, true) };
    o.set_u8(x.b, 0, 1);
    o.set_u8(x.b, Obj::KIND, 0x12);
    let flip = x.u8(f::FLIP);
    o.set_u8(x.b, f::FLIP, flip);
    copy_place(x, o, false);
    o.set_u16(x.b, Obj::Z, 0x30);
    let d2: u16 = if flip != 0 { 0x60 } else { (-0x60i16) as u16 };
    let old = o.u16(x.b, Obj::X);
    o.set_u16(x.b, Obj::X, old.wrapping_add(d2));
    (Some(o), old.checked_add(d2).is_none())
}

/// 0xa0ae0 : tir ESHL 0x13 visant `target` (long `+0x50`), lancé par l'objet (`+0x54`), à sa position.
pub fn eshl_13<R: Banc>(x: &mut Cx<'_, R>, target: u32) -> Option<Obj> {
    let o = alloc_eshl(x)?;
    let link = x.u16(0x42);
    o.set_u16(x.b, 0x42, link);
    o.set_u32(x.b, 0x50, target);
    o.set_u32(x.b, 0x54, x.o.0);
    o.set_u8(x.b, f::BOX_SET, 0);
    o.set_u8(x.b, 0, 1);
    o.set_u8(x.b, Obj::KIND, 0x13);
    copy_place(x, o, true);
    let flip = x.u8(f::FLIP);
    o.set_u8(x.b, f::FLIP, flip);
    Some(o)
}

/// 0xa114a : tir ESHL 0x16 de variante `variant` à la position de l'objet (`+0x2f` = sa marque d'image `+0x33`).
pub fn eshl_16(x: &mut Cx, variant: u8) {
    x.c.d[0] = x.c.d[0] & !0xff | variant as u32;
    let Some(o) = alloc_eshl(x) else { return };
    o.set_u8(x.b, 0, 1);
    o.set_u8(x.b, Obj::KIND, 0x16);
    copy_place(x, o, true);
    let flip = x.u8(f::FLIP);
    o.set_u8(x.b, f::FLIP, flip);
    o.set_u8(x.b, Obj::VARIANT, variant);
    let mark = x.u8(f::KEY_FRAME);
    o.set_u8(x.b, 0x2f, mark);
}

/// 0x9f346 : tir ESHL 0xa lancé par l'objet (`+0x50`), à sa position, orienté à l'opposé.
pub fn eshl_0a<R: Banc>(x: &mut Cx<'_, R>) -> Option<Obj> {
    let o = alloc_eshl(x)?;
    o.set_u16(x.b, 0x50, x.o.0 as u16);
    o.set_u8(x.b, 0, 1);
    o.set_u8(x.b, Obj::KIND, 0xa);
    copy_place(x, o, false);
    let flip = x.u8(f::FLIP) ^ 0x20;
    o.set_u8(x.b, f::FLIP, flip);
    Some(o)
}

/// 0xa2d22 : six tirs ESHL 0x26 (`+0x2f` = 5 à 0) liés par `+0x46` ; le premier refus du pool arrête la série.
pub fn eshl_burst_26(x: &mut Cx) {
    for n in (0..=5u8).rev() {
        x.c.d[1] = x.c.d[1] & !0xffff | n as u32; // relu : compteur de boucle relu au tour suivant
        let Some(o) = alloc_eshl(x) else { return };
        o.set_u8(x.b, 0, 1);
        o.set_u8(x.b, Obj::KIND, 0x26);
        o.set_u8(x.b, Obj::VARIANT, 0);
        o.set_u8(x.b, 0x2f, n);
        o.set_u16(x.b, 0x46, x.o.0 as u16);
    }
    x.c.d[1] |= 0xffff;
}

/// Tirage d'un bit (`random & mask`) dans un masque ; D0 = le tirage, comme l'original.
fn roll_bit(x: &mut Cx, mask: u32, bits: u32) -> bool {
    let r = (x.random() as u32) & bits;
    x.c.d[0] = x.c.d[0] & !0xffff | r;
    mask >> r & 1 != 0
}

/// 0xffda : tirage d'un bit dans le masque (mot) de la table 0xfff4, choisi par le rang / 8. (Le lot A lisait la
/// table en 0xffd8 : corrigé par `fuzz_ia`.)
pub fn roll_ffda(x: &mut Cx) -> bool {
    let i = (var::RANK.get(x.b) >> 3) << 1;
    let mask = x.rom16(0xfff4u32.wrapping_add(i as i16 as u32));
    x.c.d[2] = x.c.d[2] & !0xffff | mask as u32;
    roll_bit(x, mask as u32, 0xf)
}

/// 0xad7c : si `+0x78` est positif, tirage d'un bit dans le masque (long) de sa ligne de la table 0xadb0, choisi
/// par le compteur d'images ; faux sinon. D0 = 1 / 0 comme l'original.
pub fn roll_78(x: &mut Cx) -> bool {
    let line = x.u8(0x78);
    if (line as i8) < 0 {
        x.c.d[0] = 0;
        return false;
    }
    let row = x.data32(0xadb0 + 4 * line as u32);
    let tick = var::RANK.get(x.b);
    let mask = x.data32(row.wrapping_add(((tick << 2) as i16) as u32));
    let hit = roll_bit(x, mask, 0x1f);
    hit
}

/// 0x10298 : script de palette `index` : huit longs de la table 0x102ba copiés en palette (0x91c380), luminosité
/// maximale (`ori.l #0xf000f000`).
pub fn palette_script(x: &mut Cx, index: u16) {
    const PALETTE: u32 = 0x91c380;
    let from = 0x102bau32.wrapping_add((index << 5) as i16 as u32);
    // movem.l (A2),{D0-D6,A3} puis movem.l ...,(A1)
    let mut longs = [0u32; 8];
    for (k, v) in longs.iter_mut().enumerate() {
        let a = from + 4 * k as u32;
        // `movem.l (A2)` : lecture de données (le lot A lisait l'espace programme : corrigé par `fuzz_ia`)
        *v = x.data32(a);
    }
    for (k, &v) in longs.iter().enumerate() {
        crate::mem::wr32(x.b, PALETTE + 4 * k as u32, v);
    }
    x.c.d[..6].copy_from_slice(&longs[..6]);
    for k in 0..8 {
        let v = crate::mem::rd32(x.b, PALETTE + 4 * k);
        crate::mem::wr32(x.b, PALETTE + 4 * k, v | 0xf000_f000);
    }
}
