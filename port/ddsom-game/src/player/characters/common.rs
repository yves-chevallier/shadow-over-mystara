//! Éléments communs au code des six personnages (P5).
//!
//! Chaque personnage (combattant, clerc, elfe, nain, magicien, voleur) a ses propres actions : saut,
//! attaque, attaque spéciale, action 0x10. Elles sont écrites sur le même modèle et partagent une
//! boîte à outils que le programme d'origine range à la fin du code du combattant (0x3f036-0x3fbb8) :
//! prolongement d'un coup, orientation, gel des autres joueurs, poussières, assombrissement de
//! l'écran pendant un sort, fin de sort. Cette boîte à outils est ici.
//!
//! Vocabulaire des actions :
//! - le pas de l'action (`Player::STEP`, toujours pair) choisit l'étape de l'automate ;
//! - le script d'animation signale un événement dans `field::EVENT` (1 : coup porté, 2 : second geste) ;
//!   l'action le lit puis l'efface ;
//! - les sons et effets sont créés par des routines d'origine appelées à travers `call`.

use super::super::actions::to_stand;
use super::super::combos::clear_recognizers;
use super::super::walk::{move_x, move_xz, move_z};
use super::super::{animation, ram, Ctx, Player};
use crate::mem::{wr16, wr8, A5};
use crate::objects::Obj;

/// Octets de l'enregistrement joueur propres aux actions des personnages.
pub mod field {
    /// Événement posé par le script d'animation : 1 coup porté (ou lancer), 2 second geste.
    pub const EVENT: u32 = 0x33;
    /// Effet joueur à créer à la fin de l'image (numéro de variante ; 0 : rien).
    pub const PENDING_EFFECT: u32 = 0x46;
    /// Mouvement porté par l'attaque en cours (h) : 1 la boîte d'attaque reste active.
    pub const MOVE_STATE: u32 = 0x18f;
    /// Genre du mouvement en cours (h) : 1 saut ou charge, 2 attaque tournoyante.
    pub const MOVE_KIND: u32 = 0x18e;
    /// Une attaque a déjà été lancée pendant ce saut.
    pub const AIR_ATTACK_DONE: u32 = 0x6b;
    /// Le joueur a changé de direction en l'air (une seule fois par saut).
    pub const AIR_STEERED: u32 = 0x73;
    /// Abscisse au décollage (mot).
    pub const TAKEOFF_X: u32 = 0x80;
    /// Coups qui ont porté pendant l'attaque : bit k = le coup du pas k.
    pub const HITS: u32 = 0x70;
    /// Rang dans l'enchaînement de coups (0, 1, 2...).
    pub const COMBO_RANK: u32 = 0x72;
    /// Fenêtre (images) pendant laquelle le bouton d'attaque doit rester appuyé pour enchaîner.
    pub const COMBO_WINDOW: u32 = 0x74;
    /// Le bouton d'attaque a été relâché pendant la fenêtre.
    pub const COMBO_RELEASED: u32 = 0x75;
    /// Compteur propre à l'action (images, décomptes de gestes).
    pub const TIMER: u32 = 0x168;
    /// Les autres joueurs sont figés par le sort (1) ; un à terre ou protégé n'est pas touché.
    pub const SPELL_FROZEN: u32 = 0x16a;
    /// Ligne choisie dans la table des attaques spéciales / objets (`checks::special_attack`).
    pub const SPECIAL_KIND: u32 = 0x15e;
    /// Ligne de l'objet de la liste d'équipement en cours d'usage (numéro 0x56 : épuisé).
    pub const ITEM: u32 = 0x140;
    /// Charges restantes de l'objet en cours d'usage.
    pub const ITEM_CHARGES: u32 = 0x148;
    /// Nouvel équipement à enfiler (objet d'échange).
    pub const ITEM_NEW: u32 = 0x15f;
    /// Le sort est annulé (par une protection) : seule l'étincelle d'échec est jouée.
    pub const SPELL_CANCELLED: u32 = 0x189;
    /// Objet cible créé par le sort (mot : adresse).
    pub const SPELL_TARGET: u32 = 0x1ea;
    /// Le sort touche un allié à portée (1) ou seulement le lanceur (0) : posé par `heal_scan` et la bénédiction.
    pub const AREA: u32 = 0x1ee;
}

/// Routines d'origine appelées par ces outils.
pub mod rom {
    /// Mise en scène d'un effet de poussière ou d'étincelle (D0 genre, D1 paramètres, D2/D3 vitesses).
    pub const DUST: u32 = 0xb10c8;
    /// Son associé à l'étincelle de `item_spark`.
    pub const SPARK_SOUND: u32 = 0xea166;
    /// Voix du personnage à l'arme tenue (D4 = numéro de l'arme).
    pub const WEAPON_VOICE: u32 = 0xe7e10;
    /// Pas d'animation d'un joueur (retenue = fin du script).
    pub const ANIMATION_STEP: u32 = 0x2d58;
    /// Voix du saut, son de l'atterrissage et de la glissade.
    pub const JUMP_VOICE: u32 = 0xe7622;
    pub const LANDING_SOUND: u32 = 0xe774c;
    pub const SLIDE_SOUND: u32 = 0xe78e8;
    /// Création d'un effet joueur (pool PEFF) du type donné par D0, d'après l'arme.
    pub const SPAWN_EFFECT: u32 = 0x4c58e;
    /// Assombrissement / retour de l'écran (routine de fondu).
    pub const FADE: u32 = 0x1686;
    /// Lancement d'une tâche (A0 = programme, D0 = priorité) : trap 0.
    pub const START_TASK: u32 = 0x9b6;
    /// Programmes des tâches d'écran du sort : assombrir et rétablir.
    pub const DIM_TASK: u32 = 0x153e;
    pub const UNDIM_TASK: u32 = 0x1536;
}

/// Tables de la ROM (relatives au PC dans l'original).
mod tables {
    /// Poussière en course : (vx, vy) par classe, quatre octets.
    pub const DUST_STAND: u32 = 0x3faac;
    pub const DUST_RUN: u32 = 0x3fb02;
    pub const DUST_JUMP: u32 = 0x3fb48;
    /// Étincelles d'objet : lignes de 8 octets (D0, D1, D2, hauteur) indexées par le genre.
    pub const SPARKS: u32 = 0x3fbb8;
}

/// Appelle une routine d'origine avec des registres de données (indice, valeur) ; renvoie Z.
pub fn call(g: &mut Ctx, p: Player, addr: u32, data: &[(usize, u32)]) -> bool {
    for &(r, v) in data {
        g.c.d[r] = v;
    }
    g.rom(addr, p)
}

/// Avance l'automate de l'action d'un pas (`addq.b #2,(7,A0)`).
pub fn next_step(g: &mut Ctx, p: Player) {
    let step = p.u8(g, Player::STEP).wrapping_add(2);
    p.set_u8(g, Player::STEP, step);
}

/// Attend la fin de l'animation puis retourne à l'état immobile.
pub fn wait_then_stand(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        to_stand(g, p);
    }
}

/// Pas d'animation par la routine d'origine (0x2d58, fidèle aux registres) : elle laisse D0.b (drapeaux
/// de l'entrée quittée) que certaines routines appelées ensuite lisent comme paramètre de l'effet créé.
/// `d0` : valeur de D0.b avant l'appel, héritée de l'aiguillage d'origine (le pas de l'action pour les
/// actions à table, deux fois le numéro de l'attaque spéciale pour les autres).
pub fn step_keeping_regs(g: &mut Ctx, p: Player, d0: u8) -> bool {
    cov!();
    g.c.d[0] = g.c.d[0] & !0xff | d0 as u32;
    g.rom(rom::ANIMATION_STEP, p);
    g.c.sr & m68k::SR_C != 0
}

/// Joue la voix de l'arme tenue (D4 = `WEAPON_A`).
pub fn weapon_voice(g: &mut Ctx, p: Player) {
    cov!();
    let weapon = p.u8(g, Player::WEAPON_A) as u32;
    call(g, p, rom::WEAPON_VOICE, &[(4, weapon)]);
}

/// Démarre l'animation `n` de la table sans décalage d'arme, puis joue la voix de l'arme.
pub fn start_with_voice(g: &mut Ctx, p: Player, n: u16) {
    cov!();
    animation::start_plain(g, p, n);
    weapon_voice(g, p);
}

/// L'arme tenue est l'une des deux armes à un geste particulier (numéros 8 et 0x18) (h).
pub fn holds_special_weapon(g: &mut Ctx, p: Player) -> bool {
    matches!(p.u8(g, Player::WEAPON_A), 8 | 0x18)
}

/// Le second équipement est d'une classe lourde (numéro ≥ 0x28).
pub fn holds_heavy_second(g: &mut Ctx, p: Player) -> bool {
    p.u8(g, Player::WEAPON_B) >= 0x28
}

/// Fin de l'attaque : si une seule requête d'effet est en attente (`field::PENDING_EFFECT`), l'effet
/// joueur correspondant est créé (0x4c58e).
pub fn flush_pending_effect(g: &mut Ctx, p: Player) {
    cov!();
    let kind = p.u8(g, field::PENDING_EFFECT) as u32;
    if kind != 0 {
        p.set_u8(g, field::PENDING_EFFECT, 0);
        call(g, p, rom::SPAWN_EFFECT, &[(0, kind)]);
    }
}

/// Vitesse du saut de type `kind` (0..3), lue dans la table `table`
/// de la ROM : lignes de quatre mots (vx, ax, vz, az) ; le sens suit l'orientation (0x3fb60).
pub fn launch_jump(g: &mut Ctx, p: Player, table: u32, kind: u8) {
    cov!();
    let row = table + 8 * kind as u32;
    for (i, off) in [0x14, 0x16, 0x18, 0x1a].into_iter().enumerate() {
        let v = g.data16(row + 2 * i as u32);
        p.set_u16(g, off, v);
    }
    if p.u8(g, Player::FACING) == 0 {
        negate_horizontal_speed(g, p);
    }
}

/// `neg.l (0x14,A0)` : le long (vitesse, accélération) est nié d'un bloc.
pub fn negate_horizontal_speed(g: &mut Ctx, p: Player) {
    let v = p.u32(g, 0x14).wrapping_neg();
    p.set_u32(g, 0x14, v);
}

/// La vitesse horizontale a changé de sens par rapport au regard (elle est négative une fois orientée
/// dans le sens du regard) : l'élan est épuisé.
pub fn momentum_reversed(g: &mut Ctx, p: Player) -> bool {
    let v = p.u16(g, 0x14) as i16;
    let along = if p.u8(g, Player::FACING) == 0 { v.wrapping_neg() } else { v };
    along < 0
}

/// Dérive horizontale d'un saut (sauf saut sur place) : le personnage avance en X ; s'il a changé de
/// direction en l'air (`AIR_STEERED`), il ne bouge plus tant qu'il n'est pas revenu à son point de départ.
pub fn drift(g: &mut Ctx, p: Player) {
    cov!();
    let jump_kind = p.u8(g, 0x67);
    if jump_kind == 0 {
        return;
    }
    if p.u8(g, field::AIR_STEERED) != 0 {
        // la direction a changé en l'air : l'élan est repris tant qu'on n'est pas revenu au départ
        let travelled = p.u16(g, 0x08).wrapping_sub(p.u16(g, field::TAKEOFF_X)) as i16;
        let travelled = if (p.u16(g, 0x16) as i16) < 0 { travelled } else { travelled.wrapping_neg() };
        if travelled < 0 {
            return;
        }
    }
    move_x(g, p);
}

/// 0x3e712 / 0x3fe88 : en l'air, une direction opposée au regard retourne le personnage et donne une
/// poussée horizontale (une seule fois par saut).
pub fn air_steer(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, field::AIR_STEERED) != 0 || (p.u16(g, 0x18) as i16) < 0 {
        return;
    }
    let keys = p.u8(g, Player::KEYS_HELD) & 3;
    if keys == 0 {
        return;
    }
    let (facing, push): (u8, i16) = if keys & 1 != 0 { (0x20, 0x30) } else { (0, -0x30) };
    if facing == p.u8(g, Player::FACING) {
        return;
    }
    p.flip(g);
    p.set_u16(g, 0x16, push as u16);
    p.set_u8(g, field::AIR_STEERED, 1);
}

/// Le bouton 1 vient d'être appuyé en l'air et aucun coup n'a encore été donné pendant ce saut.
pub fn air_attack_pressed(g: &mut Ctx, p: Player) -> bool {
    cov!();
    p.u8(g, field::AIR_ATTACK_DONE) == 0 && p.u8(g, Player::KEYS_NEW) & 0x10 != 0
}

/// 0x377a6 : une arme maudite qui a trop servi (compteur `A5+0x56` > 0x7f) tombe : le joueur ramasse
/// l'arme 0xd et passe à l'action 0x26. Appelée à la fin d'un saut.
pub fn curse_check(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::WEAPON_A) == 0xe && g.a5u8(0x56) > 0x7f {
        p.set_u8(g, Player::WEAPON_A, 0xd);
        p.set_state(g, 0x0200_2600);
        wr8(g.b, A5.wrapping_sub(0x5b82), 1);
    }
}

/// Aura qui diminue à chaque coup en l'air : à zéro l'aura s'éteint (0x3e7c6).
pub fn aura_hit_countdown(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::AURA) == 0 {
        return;
    }
    let left = (p.u8(g, Player::AURA) - 1) & 3;
    p.set_u8(g, Player::AURA, left);
    if left == 0 {
        p.set_u16(g, Player::AURA_TIMER, 0);
        let flags = p.u8(g, Player::FLAGS) & !2;
        p.set_u8(g, Player::FLAGS, flags);
    }
}

/// 0x3f036 : prolongement d'un coup. Au sol, une attaque spéciale demandée avec l'objet 0x2d en main
/// (touche 4) ou un retournement avec le bouton 1 appuyé lance l'attaque en demi-tour (`turn_attack`).
pub fn follow_up(g: &mut Ctx, p: Player) {
    cov!();
    if p.u16(g, 0x0c) != 0 {
        return;
    }
    if p.u8(g, Player::KEYS_NEW) & 0x80 != 0 && p.u8(g, 0x15c) == 0 && p.u8(g, field::ITEM) == 0x2d {
        if turn_attack(g, p) {
            return;
        }
    }
    let keys = p.u8(g, Player::KEYS_HELD);
    if keys & 3 == 0 {
        return;
    }
    let wanted = if keys & 1 != 0 { 0x20 } else { 0 };
    if wanted == p.u8(g, Player::FACING) || keys & 0x10 == 0 {
        return;
    }
    turn_attack(g, p);
}

/// 0x3f082 : attaque en demi-tour, réservée aux équipements de classe 0x28..0x30 ; passe à l'action
/// 0x16. Renvoie vrai si elle démarre.
pub fn turn_attack(g: &mut Ctx, p: Player) -> bool {
    cov!();
    if holds_special_weapon(g, p) || p.u8(g, Player::WEAPON_B).wrapping_sub(0x28) > 8 {
        return false;
    }
    p.set_state(g, 0x0200_1600);
    true
}

/// 0x3f8d2 : l'orientation suit la direction maintenue.
pub fn face_held_direction(g: &mut Ctx, p: Player) {
    cov!();
    let dirs = p.u8(g, Player::KEYS_HELD) & 3;
    if dirs == 0 {
        return;
    }
    p.set_u8(g, Player::FACING, 0);
    if dirs & 2 == 0 {
        p.set_u8(g, Player::FACING, 0x20);
    }
}

/// 0x3f8f0 : le sort fige les autres joueurs en vie (`SPELL_FROZEN`) et les rend visibles.
pub fn freeze_others(g: &mut Ctx, p: Player) {
    cov!();
    for q in Player::all() {
        if q.u8(g, Player::ACTIVE) == 0 || q.u8(g, Player::POWER_10F) != 0 {
            continue;
        }
        if q.u8(g, Player::MAIN) != 4 {
            let flags = q.u8(g, Player::FLAGS) & !2;
            q.set_u8(g, Player::FLAGS, flags);
        }
        q.set_u8(g, field::SPELL_FROZEN, (q != p) as u8);
    }
}

/// Assombrit ou rétablit l'écran : démarre la tâche d'écran `program` (0x153e / 0x1536) après le
/// fondu 0x1686. `mode` : 1 assombrir, 2 rétablir.
fn screen_task(g: &mut Ctx, p: Player, mode: u16, program: u32, fade: bool) {
    wr16(g.b, A5 + 0x490, mode);
    wr8(g.b, A5 + 0x48e, 0xd);
    wr8(g.b, A5 + 0x48f, 5);
    if fade {
        g.rom(rom::FADE, p);
    }
    g.c.a[1] = p.0;
    g.c.a[0] = program;
    g.c.d[0] = 0x10;
    crate::call(g.c, g.b, rom::START_TASK);
    g.c.a[0] = p.0;
}

/// 0x3f926 : le sort commence, l'écran s'assombrit.
pub fn dim_screen(g: &mut Ctx, p: Player) {
    cov!();
    screen_task(g, p, 1, rom::DIM_TASK, true);
}

/// 0x3f954 : le sort est fini, l'écran est rétabli.
pub fn undim_screen(g: &mut Ctx, p: Player) {
    cov!();
    screen_task(g, p, 2, rom::UNDIM_TASK, false);
}

/// Début de sort (pas suivant de l'action) : pause du jeu (`A5+0x3d`), indicateur de sort (`A5+0x4a`),
/// durée de l'action (`field::TIMER`) si elle est connue dès le début.
pub fn begin_spell(g: &mut Ctx, p: Player, timer: Option<u16>) {
    cov!();
    next_step(g, p);
    g.set_a5u8(ram::PAUSE, 1);
    g.set_a5u8(0x4a, 1);
    g.set_a5u16(0x4ba, 0);
    if let Some(timer) = timer {
        p.set_u16(g, field::TIMER, timer);
    }
}

/// Fin de sort : lève la pause et les gels, rétablit l'écran, retour à l'état immobile.
pub fn end_spell(g: &mut Ctx, p: Player) {
    cov!();
    clear_spell_state(g);
    p.set_u8(g, field::SPELL_CANCELLED, 0);
    undim_screen(g, p);
    to_stand(g, p);
}

/// Fin de sort qui ne touche pas au drapeau d'annulation (le sort n'a pas été annulé).
pub fn end_spell_keep_cancel(g: &mut Ctx, p: Player) {
    cov!();
    clear_spell_state(g);
    undim_screen(g, p);
    to_stand(g, p);
}

/// Retombée des indicateurs de sort : indicateur et pause, ligne choisie, gel des quatre joueurs.
pub fn clear_spell_state(g: &mut Ctx) {
    cov!();
    g.set_a5u8(0x4a, 0);
    g.set_a5u8(ram::PAUSE, 0);
    g.set_a5u8(0x5b, 0);
    for k in 0..4 {
        Player::nth(k).set_u8(g, field::SPELL_FROZEN, 0);
    }
}

/// Une charge de l'objet en usage est consommée ; la dernière l'épuise (numéro 0x56).
pub fn consume_charge(g: &mut Ctx, p: Player) {
    cov!();
    let left = p.u8(g, field::ITEM_CHARGES).wrapping_sub(1);
    p.set_u8(g, field::ITEM_CHARGES, left);
    if left == 0 {
        p.set_u8(g, field::ITEM, 0x56);
    }
}

/// Poussière aux pieds : effet 0xb10c8 avec des vitesses lues dans une table à deux mots par classe.
fn dust(g: &mut Ctx, p: Player, table: u32) {
    let class = (p.u8(g, Player::CHARACTER) >> 1) as u32;
    let vx = g.prog16(table + 4 * class) as u32;
    let vy = g.prog16(table + 2 + 4 * class) as u32;
    call(g, p, rom::DUST, &[(0, 0), (1, 0x101), (2, vx), (3, vy)]);
}

/// 0x3fa7c : poussière du freinage ou de la marche (une image sur quatre).
pub fn dust_stand(g: &mut Ctx, p: Player) {
    cov!();
    let phase = p.u8(g, Player::CHARACTER).wrapping_add(g.a5u8(ram::FRAME_COUNTER)) & 3;
    if phase == 0 {
        dust(g, p, tables::DUST_STAND);
    }
}

/// 0x3fac4 : poussière de la charge, tant que la vitesse horizontale dépasse 2 pixels par image.
pub fn dust_run(g: &mut Ctx, p: Player) {
    cov!();
    let phase = p.u8(g, Player::CHARACTER).wrapping_add(g.a5u8(ram::FRAME_COUNTER)) & 3;
    if phase != 0 {
        return;
    }
    let speed = (p.u16(g, 0x14) as i16).unsigned_abs();
    if speed > 0x200 {
        dust(g, p, tables::DUST_RUN);
    }
}

/// 0x3fb1a : poussière du saut, une image sur deux (selon le numéro du joueur).
pub fn dust_jump(g: &mut Ctx, p: Player) {
    cov!();
    let phase = p.u8(g, Player::INDEX).wrapping_add(g.a5u8(ram::FRAME_COUNTER)) & 1;
    if phase == 0 {
        dust(g, p, tables::DUST_JUMP);
    }
}

/// 0x3fb8c : étincelle de l'objet utilisé : effet 0xb10c8 décrit par la ligne `kind` de la table
/// 0x3fbb8, avec un son. Sans effet si le pool est plein.
pub fn item_spark(g: &mut Ctx, p: Player, kind: u16) {
    cov!();
    let row = tables::SPARKS + 8 * kind as u32;
    let (d0, d1, d2, height) = (g.prog16(row), g.prog16(row + 2), g.prog16(row + 4), g.prog16(row + 6));
    g.c.d[3] = g.c.d[3] & 0xffff_0000 | 0xffff;
    g.c.d[4] = height as u32;
    let z = call(g, p, rom::DUST, &[(0, d0 as i16 as u32), (1, d1 as u32), (2, d2 as u32)]);
    let failed = g.c.sr & m68k::SR_C != 0;
    let _ = z;
    if failed {
        return;
    }
    let spark = g.c.a[1];
    wr16(g.b, spark + 0x0c, height);
    g.rom(rom::SPARK_SOUND, p);
}

// ---- soins (clerc, potion de soin du combattant) -----------------------------------------------

/// Points de vie rendus par le sort de soin selon le niveau (`WEAPON` 0x13 : potion), mots de la ROM.
const HEAL_TABLE: u32 = 0x416cc;
/// Couleurs du scintillement de soin : trois blocs de 16 octets copiés dans la palette du joueur.
const HEAL_FLICKER: u32 = 0x41792;
/// Adresse de la palette modifiée par le scintillement.
const FLICKER_PALETTE: u32 = 0x91_c1f0;

/// 0x41586 : cherche un allié à soigner à portée devant le lanceur (`find_ally`, 80 pixels). Le premier
/// trouvé reçoit la pose de soin ; sans allié le lanceur se soigne lui-même. `field::AREA` = 1 si c'est
/// un allié, et chaque joueur à soigner porte le drapeau `0x78` (remis à zéro pour ceux examinés).
pub fn heal_scan(g: &mut Ctx, p: Player) {
    cov!();
    let ally = find_ally(g, p, 0x50, |g, q| q.set_u8(g, 0x78, 0));
    if let Some(q) = ally {
        q.set_u8(g, 0x78, 1);
        if p.u8(g, field::SPELL_CANCELLED) == 0 {
            q.set_state(g, 0x0200_1c00);
            q.set_u8(g, field::SPELL_FROZEN, 0);
            face_toward(g, p, q);
        }
    }
    p.set_u8(g, field::AREA, 1);
    let flags: u8 = Player::all().map(|q| q.u8(g, 0x78)).fold(0, u8::wrapping_add);
    if flags == 0 {
        p.set_u8(g, field::AREA, 0);
        p.set_u8(g, 0x78, 1);
    }
}

/// Le joueur `q` est devant `p` à moins de `reach` pixels (profondeur : 12 pixels de part et d'autre).
pub fn within_reach(g: &mut Ctx, p: Player, q: Player, reach: u16) -> bool {
    cov!();
    let depth = p.u16(g, Obj::Y).wrapping_sub(q.u16(g, Obj::Y)).wrapping_add(0xc);
    if depth > 0x18 {
        return false;
    }
    let mut dx = p.u16(g, Obj::X).wrapping_sub(q.u16(g, Obj::X)) as i16;
    if p.u8(g, Player::FACING) != 0 {
        dx = dx.wrapping_neg();
    }
    dx >= 0 && dx as u16 <= reach
}

/// La cible `q` se retourne vers `p` (elle regarde du côté de l'abscisse la plus petite de `p`).
fn face_toward(g: &mut Ctx, p: Player, q: Player) {
    q.set_u8(g, Player::FACING, 0);
    if p.u16(g, Obj::X) > q.u16(g, Obj::X) {
        q.set_u8(g, Player::FACING, 0x20);
    }
}

/// 0x4175c : toutes les 8 images, la palette du lanceur prend l'un des trois blocs de couleurs du soin.
pub fn heal_flicker(g: &mut Ctx, p: Player) {
    cov!();
    if g.a5u8(0x6d) & 7 != 0 {
        return;
    }
    let mut n = p.u8(g, 0x1ed) + 1;
    if n == 3 {
        n = 0;
    }
    p.set_u8(g, 0x1ed, n);
    for i in 0..4 {
        let v = g.data32(HEAL_FLICKER + 16 * n as u32 + 4 * i);
        crate::mem::wr32(g.b, FLICKER_PALETTE + 4 * i, v);
    }
}

/// 0x41656 : rend des points de vie au lanceur ou aux alliés marqués par `heal_scan`, dans la limite
/// du maximum. Les sorts 0x2c et 0x2d rendent deux fois le niveau, les autres la valeur de la table.
/// Particularité de l'original : après chaque soin le registre du montant reçoit la somme obtenue, si bien
/// que chaque allié suivant est soigné d'autant de points de vie que le précédent en avait (avant plafond).
pub fn apply_heal(g: &mut Ctx, p: Player) {
    cov!();
    let level2 = 2 * p.u8(g, Player::LEVEL) as u16;
    let kind = p.u8(g, field::SPECIAL_KIND);
    let mut amount = if kind == 0x2c || kind == 0x2d { level2 } else { g.data16(HEAL_TABLE + level2 as u32) };
    let mut heal = |g: &mut Ctx, q: Player| {
        let sum = q.u16(g, Player::HP).wrapping_add(amount);
        let max = q.u16(g, Player::HP_MAX);
        q.set_u16(g, Player::HP, sum.min(max));
        amount = sum;
    };
    if p.u8(g, field::AREA) == 0 {
        p.set_u8(g, 0x78, 0);
        heal(g, p);
        return;
    }
    for q in Player::all() {
        if q.u8(g, 0x78) != 0 {
            q.set_u8(g, 0x78, 0);
            heal(g, q);
        }
    }
}

// ---- saut -------------------------------------------------------------------------------------

/// Ce qui change d'un personnage à l'autre dans le saut.
pub struct JumpKit {
    /// Table de la ROM (données) : quatre mots (vx, ax, vz, az) par type de saut `Player::+0x67`.
    pub speeds: u32,
    /// Coup donné en l'air avec le bouton 1.
    pub air_attack: fn(&mut Ctx, Player),
    /// Appelé quand la récupération est finie et que le personnage est revenu à l'état immobile.
    pub after_recover: fn(&mut Ctx, Player),
    /// Appelé à la fin de chaque image de saut.
    pub finish: fn(&mut Ctx, Player),
}

/// Ne fait rien (variante par défaut des crochets d'un `JumpKit`).
pub fn nothing(_: &mut Ctx, _: Player) {}

/// Animation selon le type de saut `+0x67` : 2 après une course, 1 en avant, sinon sur place.
fn by_jump_kind(g: &mut Ctx, p: Player, after_run: u16, forward: u16, standing: u16) -> u16 {
    match p.u8(g, 0x67) {
        2 => after_run,
        1 => forward,
        _ => standing,
    }
}

/// Saut (0x3e578 pour le combattant) : pas 0 attend la fin de l'accroupissement, 2 la montée, 4 le
/// sommet, 6 la descente, 8 la récupération. Le type de saut (`+0x67`, posé par `checks::jump_button`)
/// choisit les animations : 0 sur place, 1 en avant, 2 après une course, 3 vertical.
pub fn jump(g: &mut Ctx, p: Player, kit: &JumpKit) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => jump_takeoff(g, p, kit),
        2 => jump_rise(g, p, kit),
        4 => jump_apex(g, p, kit),
        6 => jump_fall(g, p, kit),
        8 => jump_recover(g, p, kit),
        _ => {}
    }
    (kit.finish)(g, p);
}

/// Pas 0 : à la fin de l'accroupissement le saut part, avec la vitesse du type de saut.
fn jump_takeoff(g: &mut Ctx, p: Player, kit: &JumpKit) {
    cov!();
    if !animation::step(g, p) {
        return;
    }
    next_step(g, p);
    p.set_u8(g, field::AIR_STEERED, 0);
    p.set_u8(g, field::AIR_ATTACK_DONE, 0);
    let x = p.u16(g, Obj::X);
    p.set_u16(g, field::TAKEOFF_X, x);
    let anim = by_jump_kind(g, p, 0x15, 0x10, 0xb);
    animation::start(g, p, anim);
    g.rom(rom::JUMP_VOICE, p);
    let kind = p.u8(g, 0x67);
    launch_jump(g, p, kit.speeds, kind);
}

/// Pas 2 : montée jusqu'à ce que la vitesse verticale tombe à 0x180.
fn jump_rise(g: &mut Ctx, p: Player, kit: &JumpKit) {
    cov!();
    animation::step(g, p);
    move_z(g, p);
    drift(g, p);
    air_steer(g, p);
    (kit.air_attack)(g, p);
    if p.u16(g, Obj::VZ) > 0x180 {
        return;
    }
    next_step(g, p);
    if p.u8(g, field::AIR_ATTACK_DONE) == 0 {
        let anim = by_jump_kind(g, p, 0x16, 0x11, 0xc);
        animation::start(g, p, anim);
    }
}

/// Pas 4 : sommet du saut.
fn jump_apex(g: &mut Ctx, p: Player, kit: &JumpKit) {
    cov!();
    move_z(g, p);
    drift(g, p);
    air_steer(g, p);
    (kit.air_attack)(g, p);
    if animation::step(g, p) {
        next_step(g, p);
        if p.u8(g, field::AIR_ATTACK_DONE) == 0 {
            let anim = by_jump_kind(g, p, 0x17, 0x12, 0xd);
            animation::start(g, p, anim);
        }
    }
}

/// Pas 6 : descente ; à l'atterrissage la récupération démarre.
fn jump_fall(g: &mut Ctx, p: Player, kit: &JumpKit) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    (kit.air_attack)(g, p);
    if (p.u16(g, Obj::Z) as i16) > 0 {
        return;
    }
    next_step(g, p);
    p.set_u32(g, Obj::Z, 0);
    let anim = by_jump_kind(g, p, 0x18, 0x13, 0xe);
    animation::start(g, p, anim);
    g.rom(rom::LANDING_SOUND, p);
}

/// Pas 8 : récupération, puis retour à l'état immobile.
fn jump_recover(g: &mut Ctx, p: Player, kit: &JumpKit) {
    cov!();
    if animation::step(g, p) {
        to_stand(g, p);
        (kit.after_recover)(g, p);
    }
}

// ---- action 0x10 : élan et glissade -------------------------------------------------------------

/// Vitesse horizontale vers l'avant (le long (vitesse, accélération) est nié si le personnage regarde
/// vers la gauche, comme `neg.l (0x14,A0)`).
pub fn set_forward_motion(g: &mut Ctx, p: Player, speed: u16, accel: u16) {
    p.set_u16(g, Obj::VX, speed);
    p.set_u16(g, Obj::AX, accel);
    if p.u8(g, Player::FACING) == 0 {
        negate_horizontal_speed(g, p);
    }
}

/// Action 0x10 (0x3f9bc combattant, 0x41f22 clerc) : élan, bond bref, glissade. Pas 0 démarre l'élan, 2
/// attend la fin du geste, 4 prolonge l'élan tant que le bouton de saut est tenu (huit images au
/// plus), 6 freine.
pub fn dash(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            set_forward_motion(g, p, 0xb00, 0);
            animation::start(g, p, 0x1c);
            clear_recognizers(g, p);
        }
        2 => {
            if animation::step(g, p) {
                next_step(g, p);
                p.set_u8(g, 0x1e8, 8);
                animation::start(g, p, 0x1d);
                g.rom(rom::SLIDE_SOUND, p);
            }
        }
        4 => {
            let left = p.u8(g, 0x1e8).wrapping_sub(1);
            p.set_u8(g, 0x1e8, left);
            if left != 0 && p.u8(g, Player::KEYS_HELD) & 0x20 != 0 {
                dust_jump(g, p);
                move_x(g, p);
                return;
            }
            next_step(g, p);
            p.set_u8(g, 0x1e8, 0xc);
            let brake = if p.u8(g, Player::FACING) != 0 { -0xa0i16 } else { 0xa0 };
            p.set_u16(g, Obj::AX, brake as u16);
            dash_slide(g, p);
        }
        6 => dash_slide(g, p),
        _ => {}
    }
}

/// 0x3fa4e : glissade freinée jusqu'à l'arrêt, avec de la poussière.
fn dash_slide(g: &mut Ctx, p: Player) {
    cov!();
    move_x(g, p);
    animation::step(g, p);
    if momentum_reversed(g, p) {
        to_stand(g, p);
        return;
    }
    let left = p.u8(g, 0x1e8).wrapping_sub(1);
    p.set_u8(g, 0x1e8, left);
    if (left as i8) >= 0 {
        dust_jump(g, p);
    }
}

// ---- coups communs ----------------------------------------------------------------------------

/// Rang de l'enchaînement de coups : le coup précédent a-t-il porté (bit `hit_bit` de `field::HITS`) ?
/// Alors le rang avance, et revient à 0 au-delà de `last`. Sinon le rang repart de 0.
pub fn chain_rank(g: &mut Ctx, p: Player, hit_bit: u8, last: u8) -> u8 {
    cov!();
    let mut rank = 0;
    if p.u8(g, field::HITS) & hit_bit != 0 {
        rank = p.u8(g, field::COMBO_RANK) + 1;
        if rank > last {
            rank = 0;
        }
    }
    p.set_u8(g, field::COMBO_RANK, rank);
    rank
}

/// Second pas d'un coup de base (0x3e8de, 0x3ffea) : si le bouton 1 reste appuyé pendant la fenêtre et
/// que le coup a porté, le geste `follow_anim` prolonge le coup ; sinon retour à l'état immobile.
pub fn combo_follow(g: &mut Ctx, p: Player, follow_anim: u16) {
    cov!();
    let window = p.u8(g, field::COMBO_WINDOW);
    if window != 0 {
        p.set_u8(g, field::COMBO_WINDOW, window - 1);
        if p.u8(g, Player::KEYS_HELD) & 0x10 == 0 {
            p.set_u8(g, field::COMBO_RELEASED, 1);
        }
    }
    if !animation::step(g, p) {
        return;
    }
    if p.u8(g, field::COMBO_RELEASED) == 0 && p.u8(g, field::HITS) != 0 {
        next_step(g, p);
        animation::start_plain(g, p, follow_anim);
    } else {
        to_stand(g, p);
    }
}

/// Un coup à un seul geste : animation `anim` avec la voix de l'arme, puis retour à l'état immobile.
pub fn single_strike(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            start_with_voice(g, p, anim);
        }
        2 => wait_then_stand(g, p),
        _ => {}
    }
}

/// Saut en arrière (0x3ece2, 0x4033a) : petit saut qui s'éloigne de l'adversaire. `animate` : le
/// script d'animation avance à chaque image (clerc) ou non (combattant).
pub fn back_hop(g: &mut Ctx, p: Player, anim: u16, animate: bool) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            p.set_u16(g, Obj::VX, 0x100);
            p.set_u16(g, Obj::AX, 0);
            p.set_u16(g, Obj::VZ, 0x300);
            p.set_u16(g, Obj::AZ, (-0x38i16) as u16);
            if p.u8(g, Player::FACING) != 0 {
                let v = (p.u16(g, Obj::VX) as i16).wrapping_neg();
                p.set_u16(g, Obj::VX, v as u16);
            }
            animation::start_plain(g, p, anim);
        }
        2 => {
            if animate {
                animation::step(g, p);
            }
            move_xz(g, p);
            if (p.u16(g, Obj::VZ) as i16) < 0 && (p.u16(g, Obj::Z) as i16) < 0 {
                p.set_u32(g, Obj::Z, 0);
                to_stand(g, p);
            }
        }
        _ => {}
    }
}

/// Un geste vers l'avant (0x3e824, 0x3ff56) : animation `anim` puis retour à l'état immobile.
pub fn pose(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            animation::start_plain(g, p, anim);
        }
        2 => wait_then_stand(g, p),
        _ => {}
    }
}

/// Pas 2 du coup donné en courant (0x3ec84, 0x402d8) : l'élan s'épuise (nouvelle animation) ou le
/// personnage continue de glisser avec de la poussière.
pub fn run_strike_slide(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if momentum_reversed(g, p) {
        next_step(g, p);
        p.set_u32(g, Obj::VX, 0);
        animation::start_plain(g, p, 0xf);
        return;
    }
    dust_run(g, p);
    move_x(g, p);
}

/// Pas 4 de la charge (0x3ebbc, 0x4023a) : les coups portés créent des effets ; la glissade ralentit
/// puis s'arrête.
pub fn charge_slide(g: &mut Ctx, p: Player, events: fn(&mut Ctx, Player)) {
    cov!();
    events(g, p);
    move_x(g, p);
    dust_stand(g, p);
    if animation::step(g, p) {
        p.set_u8(g, Player::SLOW, 0);
        to_stand(g, p);
        return;
    }
    if momentum_reversed(g, p) {
        p.set_u32(g, Obj::VX, 0);
    }
}

// ---- alliés à portée ----------------------------------------------------------------------------

/// Premier allié que le lanceur `p` peut soigner ou bénir : un autre joueur actif, au sol, en vie,
/// sans objet tenu ni saisie, devant `p` à moins de `reach` pixels. `visit` est appelé sur chaque
/// joueur examiné avant le test (la recherche s'arrête au premier allié trouvé).
pub fn find_ally(g: &mut Ctx, p: Player, reach: u16, mut visit: impl FnMut(&mut Ctx, Player)) -> Option<Player> {
    cov!();
    for q in Player::all() {
        visit(g, q);
        if q == p || q.u8(g, Player::ACTIVE) & 1 == 0 || q.u16(g, Obj::Z) != 0 {
            continue;
        }
        if q.u8(g, Player::POWER_10F) != 0 || q.u16(g, Player::HP) == 0 || q.u8(g, Player::SLOW_8A) != 0 {
            continue;
        }
        if q.u8(g, Player::ACTION) == 0x22 || !within_reach(g, p, q, reach) {
            continue;
        }
        return Some(q);
    }
    None
}
