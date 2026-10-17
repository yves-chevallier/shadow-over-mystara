//! Sous-objets des joueurs : arme et bouclier dessinés à côté du personnage (P4).
//!
//! Chaque joueur a deux sous-objets de 0x60 octets (`init::overlay`) : le premier porte l'arme tenue
//! (`WEAPON_A`), le second le bouclier ou la seconde arme (`WEAPON_B`). Chaque image, `update`
//! (0x392e4, appelée juste après la boucle des joueurs) les recale sur le joueur : même orientation,
//! décalage et animation tirés de tables de la ROM selon le personnage, le cadre d'animation du
//! joueur (`+0x40`, `+0x42`, `+0x44`) et l'arme. Elle fait aussi scintiller la palette des armes
//! quand un joueur est sous bonus de puissance.

use cps2::Cps2Bus;
use m68k::Cpu;

use super::checks::team_score_reached;
use super::init::overlay;
use super::{Ctx, Player, PLAYER_COUNT};
use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8, A5};
use crate::objects::anim::load_entry;
use crate::objects::Obj;
use crate::Native;

/// Tables de la ROM (données) : un pointeur par personnage vers des lignes de 8 octets
/// (animation, décalage x, décalage z, retournement, profondeur).
mod tables {
    /// Arme tenue (`+0x40`) et bouclier de bas numéro (`+0x42`).
    pub const BODY: u32 = 0x395d4;
    /// Bouclier de numéro ≥ 0x28 (`+0x44`).
    pub const SHIELD_HIGH: u32 = 0x39604;
    /// Scripts d'animation par numéro d'arme (pointeurs).
    pub const ANIMATIONS: u32 = 0x396e8;
    /// Palettes qui scintillent sous bonus (8 blocs de 32 octets).
    pub const POWER_PALETTES: u32 = 0x397e0;
}

/// Drapeau « l'équipe est prête pour le sort ultime » (mot en `A5-0x5fc2`).
const TEAM_READY: u32 = A5 - 0x5fc2;
/// Drapeau d'arrêt des scintillements (octet en `A5-0x65ec`).
const NO_SPARKLE: u32 = A5 - 0x65ec;
/// Palette qui scintille (RAM de palette CPS).
const SPARKLE_RAM: u32 = 0x91_c1c0;

/// Champs d'un joueur lus ici.
mod f {
    pub const WEAPON_FRAME: u32 = 0x40;
    pub const SHIELD_LOW_FRAME: u32 = 0x42;
    pub const SHIELD_HIGH_FRAME: u32 = 0x44;
    pub const HAS_WEAPON: u32 = 0x68;
    pub const WEAPON_KIND: u32 = 0x107;
    pub const POWERUP: u32 = 0x79;
    pub const SPARKLE_STEP: u32 = 0x52;
}

/// Ligne de table d'un sous-objet : 8 octets.
struct Row {
    anim: u16,
    dx: i16,
    dz: i16,
    flip: bool,
    behind: bool,
}

fn row(g: &mut Ctx, table: u32, character: u32, index: u32) -> Row {
    cov!();
    let base = g.data32(table + 4 * character) + 8 * index;
    Row {
        anim: g.data16(base),
        dx: g.data16(base + 2) as i16,
        dz: g.data16(base + 4) as i16,
        flip: g.data8(base + 6) != 0,
        behind: g.data8(base + 7) != 0,
    }
}

/// Le sous-objet suit l'état d'activité et l'orientation du joueur.
fn follow(g: &mut Ctx, p: Player, o: Obj) {
    cov!();
    let (active, facing) = (p.u8(g, Player::ACTIVE), p.u8(g, Player::FACING));
    wr8(g.b, o.0, active);
    wr8(g.b, o.at(0x2c), facing);
}

/// Place le sous-objet `o` d'après la ligne `r` du joueur `p`.
fn place(g: &mut Ctx, p: Player, o: Obj, r: &Row) {
    cov!();
    if r.flip {
        let facing = rd8(g.b, o.at(0x2c)) ^ 0x20;
        wr8(g.b, o.at(0x2c), facing);
    }
    wr16(g.b, o.at(0x10), if r.behind { 0x8000 } else { 0 });
    let dx = if p.u8(g, Player::FACING) != 0 { r.dx.wrapping_neg() } else { r.dx };
    wr16(g.b, o.at(0x08), dx as u16);
    wr16(g.b, o.at(0x0c), r.dz as u16);
}

/// Charge l'animation `n` du script d'armes `weapon` dans le sous-objet.
fn load_animation(g: &mut Ctx, o: Obj, weapon: u32, n: u16) {
    cov!();
    let set = g.data32(tables::ANIMATIONS + 4 * weapon);
    let off = rd16(g.b, set.wrapping_add(2 * n as u32)) as i16 as i32 as u32;
    load_entry(g.b, o, set.wrapping_add(off), false);
    if rd8(g.b, o.at(0x38)) != 0 {
        let facing = rd8(g.b, o.at(0x2c)) ^ 0x20;
        wr8(g.b, o.at(0x2c), facing);
    }
}

/// 0x393c8 : sous-objet de l'arme tenue.
fn body(g: &mut Ctx, p: Player, o: Obj) {
    cov!();
    let hidden = (p.u8(g, Player::WEAPON_A) as i8) < 0 || p.u8(g, f::HAS_WEAPON) == 0;
    if hidden {
        p.set_u16(g, Player::OVERLAY_BODY, 0);
        return;
    }
    follow(g, p, o);
    wr8(g.b, o.at(0x2a), 1);
    wr8(g.b, o.at(0x2b), 0x12);
    p.set_u8(g, f::WEAPON_KIND, 1);
    let weapon = p.u8(g, Player::WEAPON_A);
    if weapon != 0x10 {
        wr8(g.b, o.at(0x2b), 0x13);
        p.set_u8(g, f::WEAPON_KIND, 2);
        if weapon != 0x11 {
            wr8(g.b, o.at(0x2b), 0x14);
            p.set_u8(g, f::WEAPON_KIND, 3);
            if weapon != 0x12 {
                p.set_u8(g, f::WEAPON_KIND, 5);
                if weapon != 0xb {
                    p.set_u8(g, f::WEAPON_KIND, 0);
                    wr8(g.b, o.at(0x2b), 0xe);
                    if p.u8(g, f::POWERUP) == 0 {
                        wr8(g.b, o.at(0x2a), 0);
                    }
                }
            }
        }
    }
    let frame = p.u16(g, f::WEAPON_FRAME) as u32;
    if frame == 0 {
        p.set_u16(g, Player::OVERLAY_BODY, 0);
        return;
    }
    let character = p.u8(g, Player::CHARACTER) as u32;
    let r = row(g, tables::BODY, character, frame);
    place(g, p, o, &r);
    p.set_u16(g, Player::OVERLAY_BODY, o.0 as u16);
    load_animation(g, o, weapon as u32, r.anim);
}

/// 0x394a2 : sous-objet du bouclier ou de la seconde arme.
fn shield(g: &mut Ctx, p: Player, o: Obj) {
    cov!();
    let shield_kind = p.u8(g, Player::WEAPON_B);
    let weapon = p.u8(g, Player::WEAPON_A);
    if (shield_kind as i8) < 0 || weapon == 8 || weapon == 0x18 {
        p.set_u16(g, Player::OVERLAY_ATTACK, 0);
        return;
    }
    let character = p.u8(g, Player::CHARACTER) as u32;
    if shield_kind >= 0x28 {
        let frame = p.u16(g, f::SHIELD_HIGH_FRAME) as u32;
        if frame == 0 {
            p.set_u16(g, Player::OVERLAY_ATTACK, 0);
            return;
        }
        follow(g, p, o);
        wr8(g.b, o.at(0x2a), 2);
        wr8(g.b, o.at(0x2b), 0x10);
        let mut r = row(g, tables::SHIELD_HIGH, character, frame);
        // une palette par paire de personnages : l'index d'animation est décalé de 0x20 par classe
        r.anim = r.anim.wrapping_add(((character >> 1) << 5) as u16);
        place(g, p, o, &r);
        p.set_u16(g, Player::OVERLAY_ATTACK, o.0 as u16);
        load_animation(g, o, shield_kind as u32, r.anim);
        return;
    }
    if p.u8(g, f::HAS_WEAPON) == 0 {
        p.set_u16(g, Player::OVERLAY_ATTACK, 0);
        return;
    }
    let frame = p.u16(g, f::SHIELD_LOW_FRAME) as u32;
    if frame == 0 {
        p.set_u16(g, Player::OVERLAY_ATTACK, 0);
        return;
    }
    follow(g, p, o);
    wr8(g.b, o.at(0x2a), 0);
    let team_ready = rd16(g.b, TEAM_READY) != 0;
    if p.u8(g, f::POWERUP) != 0 || (team_ready && shield_kind == 0x21) {
        wr8(g.b, o.at(0x2a), 1);
        wr8(g.b, o.at(0x2b), 0xe);
    }
    let r = row(g, tables::BODY, character, frame);
    place(g, p, o, &r);
    p.set_u16(g, Player::OVERLAY_ATTACK, o.0 as u16);
    load_animation(g, o, shield_kind as u32, r.anim);
    blink_unless_idle(g, p);
}

/// 0x39634 : certains sorts (magiciens 8 et 9, ou charge de baguette 0..5) font clignoter l'arme :
/// l'objet est masqué une image sur deux.
fn blink_unless_idle(g: &mut Ctx, p: Player) {
    cov!();
    let character = p.u8(g, Player::CHARACTER);
    if !matches!(character, 8 | 9) && p.u8(g, Player::AURA) != 0 {
        return;
    }
    let charge = match p.u8(g, Player::WEAPON_B) {
        0x24 => p.u8(g, 0x18c),
        0x25 => p.u8(g, 0x18b),
        0x26 => p.u8(g, 0x18d),
        0x27 => p.u8(g, 0x191),
        _ => return,
    };
    if (charge as i8) > 5 {
        return;
    }
    if g.a5u8(super::ram::FRAME_COUNTER) & 4 != 0 {
        p.set_u16(g, Player::OVERLAY_ATTACK, 0);
    }
}

/// 0x397a8 : fait défiler la palette scintillante des armes (huit blocs, un toutes les 8 images).
/// `target` est le sous-objet dont l'octet `SPARKLE_STEP` compte les étapes (l'original utilise le
/// dernier sous-objet manipulé).
fn sparkle(g: &mut Ctx, target: u32) {
    cov!();
    if g.a5u8(super::ram::FRAME_COUNTER) & 7 != 0 {
        return;
    }
    let step = rd8(g.b, target + f::SPARKLE_STEP).wrapping_add(1) & 7;
    wr8(g.b, target + f::SPARKLE_STEP, step);
    let src = tables::POWER_PALETTES + ((step as u32) << 5);
    for i in 0..8 {
        let v = rd32(g.b, src + 4 * i);
        wr32(g.b, SPARKLE_RAM + 4 * i, v);
    }
}

/// 0x392e4 : met à jour tous les sous-objets. `a0` : valeur de A0 à l'entrée (restée dans l'original
/// si aucun joueur n'est actif).
pub fn update(g: &mut Ctx, a0: u32) {
    cov!();
    wr16(g.b, TEAM_READY, 0);
    if g.a5u8(super::ram::PLAYERS_ALIVE) >= 3 && team_score_reached(g) {
        wr16(g.b, TEAM_READY, 1);
    }
    let mut last = a0;
    for k in 0..PLAYER_COUNT {
        let p = Player::nth(k);
        if p.u8(g, Player::ACTIVE) == 0 || p.u16(g, Player::MAIN) == 0x402 {
            continue;
        }
        body(g, p, Obj(overlay(k, false)));
        last = overlay(k, true);
        shield(g, p, Obj(last));
    }
    if rd8(g.b, NO_SPARKLE) != 0 {
        return;
    }
    let powered: u8 = Player::all().map(|q| q.u8(g, f::POWERUP)).fold(0, |a, b| a.wrapping_add(b));
    if powered != 0 || Player::all().any(|q| q.u8(g, Player::WEAPON_B) == 0x21) {
        sparkle(g, last);
    }
}

fn update_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let a0 = c.a[0];
    update(&mut Ctx::new(c, b), a0);
    rts(c, b);
}

pub const UPDATE: Native = Native {
    entry: 0x392e4,
    name: "player_overlays_update",
    cost: |_, _| 4_000,
    run: update_native,
    touches_sound: false,
    outputs: 0,
    exact_cost: false,
};
