//! Créateurs d'effets joueur (pool PEFF) et de tirs joueur (pool PSHL).
//!
//! Les actions des personnages ne dessinent rien elles-mêmes : au geste porté elles créent un objet lié
//! au joueur (coup d'épée, souffle, lumière, ombre...). L'objet naît dans un pool avec son genre
//! (`Obj::KIND`), qui choisit le gestionnaire en ROM chargé de le faire vivre, et quelques paramètres
//! propres au genre. Le mot `+0x50` le relie au joueur qui l'a créé ; le gestionnaire y retrouve la
//! position et l'orientation du joueur.
//!
//! Chaque créateur a une fonction Rust (appelée par les actions) et une native à l'adresse d'origine pour
//! les appelants restés en ROM. Quand le pool est plein l'allocation échoue et rien n'est créé.

use m68k::decode::Size;

use super::{Ctx, Player};
use crate::mem::{rd16, rd8, rts, wr16, wr8, A5};
use crate::objects::Obj;
use crate::{regs, Native};
use crate::objects::pools;
use crate::player::ram;
use crate::machine::Machine;

/// Octets de l'objet créé.
mod field {
    /// Lien vers le joueur créateur (mot : adresse).
    pub const LINK: u32 = 0x50;
    /// Paramètre du genre.
    pub const PARAM: u32 = 0x2f;
    /// Numéro de la scène (octet de A5) copié dans l'objet : l'objet disparaît quand la scène change.
    pub const SCENE: u32 = 0x5b;
}

/// Genres d'effets joueur.
mod kind {
    pub const BLOOD: u8 = 2;
    pub const SLASH: u8 = 3;
    pub const CURE: u8 = 4;
    pub const HOLY_LIGHT: u8 = 5;
    pub const BEAM_CAST: u8 = 6;
    pub const ICE_GROUND: u8 = 7;
    pub const SHADOW: u8 = 8;
    pub const BEAM: u8 = 9;
    pub const BLESSING: u8 = 0xd;
    pub const BANISH: u8 = 0xe;
    pub const ICE: u8 = 0xc;
    pub const WEAPON_TRAIL: u8 = 0xf;
    pub const LIGHTNING: u8 = 0x13;
    pub const FIREBALL: u8 = 0x16;
    pub const KNIFE: u8 = 0x15;
    pub const RING: u8 = 0x1b;
    pub const SHOWER: u8 = 0x19;
    pub const AURA: u8 = 0x1d;
    pub const STORM: u8 = 0x1f;
    pub const LIGHTNING_TARGET: u8 = 0x20;
    pub const SWARM: u8 = 0x22;
    pub const LEAP: u8 = 0x23;
    pub const SWAP: u8 = 0x24;
    pub const DASH: u8 = 0x25;
    pub const METEORS: u8 = 0x28;
    pub const WALL_TARGET: u8 = 0x29;
    /// Créature invoquée par le magicien (PEFF).
    pub const SUMMON: u8 = 0x12;
    /// Portail du magicien (PEFF ; même numéro que la boule de feu, qui est un tir PSHL).
    pub const PORTAL: u8 = 0x16;
    /// Sorcière invoquée par le magicien (tir PSHL).
    pub const WITCH: u8 = 0x12;
    /// Arme maudite qui s'échappe (PEFF).
    pub const CURSED_WEAPON: u8 = 0x26;
    /// Éclats du joueur mourant (PEFF).
    pub const SHARD: u8 = 0x27;
    /// Effets des sorts du magicien (PEFF) : étincelles et éclat du portail, lueur, nuée de l'orage ; et pluie de
    /// traits (PSHL).
    pub const PORTAL_SPARK: u8 = 0x13;
    pub const PORTAL_BURST: u8 = 0x14;
    pub const RAIN: u8 = 0x14;
    pub const PORTAL_GLOW: u8 = 0x2d;
    pub const STORM_CLOUD: u8 = 0x2e;
    /// Effets des sorts orage (vers la gauche), pluie, mur de feu et vague du magicien (PEFF).
    pub const STORM_SPELL: u8 = 0x14;
    pub const RAIN_GLOW: u8 = 0x17;
    pub const WALL_SPARK: u8 = 0x15;
    pub const WAVE_BOLT: u8 = 0x1a;
    /// Lunes de l'elfe (PEFF).
    pub const MOON: u8 = 0x1c;
    pub const MOON_B: u8 = 0x1e;
}

/// Réserve un emplacement dans `pool` et l'active avec le genre `kind`. Les créateurs sont des natives appelées par
/// adresse (tables des sorts, `g.rom`) : l'allocation garde ses registres (A1 = l'objet, ou le reste laissé par un
/// échec ; retenue), que relisent les appelants (`items::lightning`, `items::wall`) et les drapeaux de sortie.
fn create(g: &mut Ctx, _p: Player, pool: &pools::Pool, kind: u8) -> Option<Obj> {
    cov!();
    let o = pool.alloc_68k(g.c, g.b)?;
    o.set_u8(g.b, Obj::ACTIVE, 1);
    o.set_u8(g.b, Obj::KIND, kind);
    Some(o)
}

/// Dernier geste d'un créateur : le lien vers le joueur. Les drapeaux N et Z sont ceux de ce `move.w`.
fn link(g: &mut Ctx, o: Obj, p: Player) {
    cov!();
    o.set_u16(g.b, field::LINK, p.0 as u16);
    g.c.flags_logic(p.0 & 0xffff, Size::Word);
}

/// Crée un effet lié au joueur : genre, variante (`Obj::VARIANT`), puis le lien. `fill` écrit les autres octets.
fn effect(g: &mut Ctx, p: Player, kind: u8, variant: Option<u8>, fill: impl FnOnce(&mut Ctx, Obj)) -> Option<Obj> {
    cov!();
    let o = create(g, p, &pools::PEFF, kind)?;
    if let Some(v) = variant {
        o.set_u8(g.b, Obj::VARIANT, v);
    }
    fill(g, o);
    link(g, o, p);
    Some(o)
}

/// Le pas de l'action en cours du créateur (copié dans l'objet pour que son gestionnaire sache d'où il vient).
fn step_of(g: &mut Ctx, p: Player) -> u8 {
    p.u8(g, Player::STEP)
}

// ---- créateurs d'effets joueur --------------------------------------------------------------------

/// 0x4a99a : trace d'un coup. `Some(n)` : trace du coup n° `n` (variante 0), sauf avec les armes 0x10 à 0x13 qui
/// n'en laissent pas ; `None` : trace simple (variante 1).
pub fn slash(g: &mut Ctx, p: Player, numbered: Option<u8>) {
    cov!();
    match numbered {
        Some(n) => {
            if p.u8(g, Player::WEAPON_A).wrapping_sub(0x10) <= 3 {
                return;
            }
            let step = step_of(g, p);
            effect(g, p, kind::SLASH, Some(0), |g, o| {
                o.set_u8(g.b, field::PARAM, n);
                o.set_u8(g.b, 0x5c, step);
            });
        }
        None => {
            let step = step_of(g, p);
            effect(g, p, kind::SLASH, Some(1), |g, o| o.set_u8(g.b, 0x5c, step));
        }
    }
}

/// 0x4a860 : sang du tourbillon du combattant.
pub fn blood(g: &mut Ctx, p: Player) {
    cov!();
    let step = step_of(g, p);
    effect(g, p, kind::BLOOD, None, |g, o| o.set_u8(g.b, 0x5c, step));
}

/// 0x4ab48 : soin d'un allié (prière du clerc), numéro de prière `n`.
pub fn cure(g: &mut Ctx, p: Player, n: u8) {
    cov!();
    effect(g, p, kind::CURE, None, |g, o| o.set_u8(g.b, field::PARAM, n));
}

/// 0x4ab64 : soin de la potion (variante 2).
pub fn potion(g: &mut Ctx, p: Player) {
    cov!();
    effect(g, p, kind::CURE, Some(2), |_, _| {});
}

/// 0x4aeb2 : lumière sacrée du clerc.
pub fn holy_light(g: &mut Ctx, p: Player) {
    cov!();
    let step = step_of(g, p);
    effect(g, p, kind::HOLY_LIGHT, None, |g, o| o.set_u8(g.b, 0x56, step));
}

/// 0x4b5ac : rayon. `aimed` faux : deux rayons (paramètre 0 et 1) ; vrai : un seul, dirigé (variante 1).
pub fn beam(g: &mut Ctx, p: Player, aimed: bool) {
    cov!();
    if aimed {
        effect(g, p, kind::BEAM, Some(1), |_, _| {});
        return;
    }
    let step = step_of(g, p);
    for param in [0, 1] {
        let made = effect(g, p, kind::BEAM, Some(0), |g, o| {
            o.set_u8(g.b, field::PARAM, param);
            o.set_u8(g.b, 0x5f, step);
        });
        if made.is_none() {
            return;
        }
    }
}

/// 0x4c19a : bénédiction, variante `v`.
pub fn blessing(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    effect(g, p, kind::BLESSING, Some(v), |_, _| {});
}

/// 0x4c26c : renvoi de monstres. Sans monstre visé (`None`) : un seul effet ; sinon quatre (paramètres 3 à 0)
/// qui suivent le monstre `foe`.
pub fn banish(g: &mut Ctx, p: Player, foe: Option<u32>) {
    cov!();
    let Some(foe) = foe else {
        effect(g, p, kind::BANISH, None, |_, _| {});
        return;
    };
    for param in (0..4).rev() {
        let made = effect(g, p, kind::BANISH, Some(1), |g, o| {
            o.set_u8(g.b, field::PARAM, param);
            o.set_u16(g.b, 0x5c, foe as u16);
        });
        if made.is_none() {
            return;
        }
    }
}

/// 0x4c58e : traînée de l'arme (armes 0x10 à 0x13), variante `v` ; le paramètre est le numéro de l'arme.
pub fn weapon_trail(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    let weapon = p.u8(g, Player::WEAPON_A).wrapping_sub(0x10);
    if weapon > 3 {
        return;
    }
    effect(g, p, kind::WEAPON_TRAIL, Some(v), |g, o| o.set_u8(g.b, field::PARAM, weapon));
}

/// 0x4dcda : averse de sorts, côté `side`.
pub fn shower(g: &mut Ctx, p: Player, side: u8) {
    cov!();
    let step = step_of(g, p);
    effect(g, p, kind::SHOWER, Some(1), |g, o| {
        o.set_u8(g.b, field::PARAM, side);
        o.set_u8(g.b, 0x5d, step);
    });
}

/// 0x4e6a8 : aura de prière, variante `v`.
pub fn aura(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    let step = step_of(g, p);
    effect(g, p, kind::AURA, Some(v), |g, o| o.set_u8(g.b, 0x59, step));
}

/// 0x4f004 : cible de la foudre, variante `v`. Renvoie l'effet créé (ou rien si le pool est plein).
pub fn lightning_target(g: &mut Ctx, p: Player, v: u8) -> Option<Obj> {
    cov!();
    effect(g, p, kind::LIGHTNING_TARGET, Some(v), |_, _| {})
}

/// 0x4f272 : essaim de `count` objets, tant qu'il en reste dans la réserve de la scène.
pub fn swarm(g: &mut Ctx, p: Player, count: u32) {
    cov!();
    const RESERVE: u32 = 0x5fc0;
    for n in (0..count).rev() {
        let left = rd16(g.b, A5.wrapping_sub(RESERVE));
        if left == 0 {
            return;
        }
        wr16(g.b, A5.wrapping_sub(RESERVE), left - 1);
        let scene = ram::SCENE.get(g.b);
        let made = create(g, p, &pools::PEFF, kind::SWARM);
        let Some(o) = made else { return };
        o.set_u16(g.b, field::LINK, p.0 as u16);
        o.set_u8(g.b, Obj::VARIANT, n as u8);
        o.set_u8(g.b, field::SCENE, scene);
    }
}

/// 0x4fbde : effet du bond (variante `v`), sauf avec les armes 0x10 à 0x12.
pub fn leap(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    if matches!(p.u8(g, Player::WEAPON_A), 0x10..=0x12) {
        return;
    }
    let step = step_of(g, p);
    effect(g, p, kind::LEAP, Some(v), |g, o| o.set_u8(g.b, 0x5c, step));
}

/// 0x4fd2c : échange d'équipement, variante `v`.
pub fn swap(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    effect(g, p, kind::SWAP, Some(v), |_, _| {});
}

/// 0x4fe18 : longue glissade du nain ; l'objet garde la sous-action en cours.
pub fn dash(g: &mut Ctx, p: Player) {
    cov!();
    let sub = p.u8(g, Player::SUBACTION);
    effect(g, p, kind::DASH, None, |g, o| o.set_u8(g.b, 0x5c, sub));
}

/// 0x50324 : cible du mur ; la réserve de murs de la scène (`A5 - 0x5fbe`) compte un de plus.
pub fn wall_target(g: &mut Ctx, p: Player) -> Option<Obj> {
    cov!();
    let scene = ram::SCENE.get(g.b);
    let o = effect(g, p, kind::WALL_TARGET, None, |g, o| o.set_u8(g.b, field::SCENE, scene))?;
    let walls = A5.wrapping_sub(0x5fbe);
    let old = rd8(g.b, walls);
    // addq.b #1 : les drapeaux de cette addition sont ceux que testent les appelants
    let new = g.c.addsub(false, 1, old as u32, false, Size::Byte, false);
    wr8(g.b, walls, new as u8);
    Some(o)
}

/// 0x4cdd0 : éclair sur la cible `target`, à sa position, 0x40 au-dessus.
pub fn lightning(g: &mut Ctx, p: Player, target: u32) {
    cov!();
    let step = step_of(g, p);
    let facing = p.u8(g, Player::FACING);
    let created = create(g, p, &pools::PEFF, kind::LIGHTNING);
    let Some(o) = created else { return };
    let (x, z, y) = (rd16(g.b, target + Obj::X), rd16(g.b, target + Obj::Z), rd16(g.b, target + Obj::Y));
    o.set_u16(g.b, field::LINK, p.0 as u16);
    o.set_u8(g.b, Player::FACING, facing);
    o.set_u16(g.b, Obj::X, x);
    o.set_u16(g.b, Obj::Z, z);
    o.set_u16(g.b, Obj::Y, y);
    o.set_u16(g.b, Obj::Z, z.wrapping_add(0x40));
    o.set_u8(g.b, 0x5d, step);
}

/// 0x4cd8e : éclair de la tempête, à la position du joueur, 0x48 plus haut et juste devant lui.
pub fn flare(g: &mut Ctx, p: Player) {
    cov!();
    let step = step_of(g, p);
    let Some(o) = create(g, p, &pools::PEFF, kind::LIGHTNING) else { return };
    o.set_u16(g.b, field::LINK, p.0 as u16);
    let facing = p.u8(g, Player::FACING);
    o.set_u8(g.b, Player::FACING, facing);
    for axis in [Obj::X, Obj::Z, Obj::Y] {
        let v = p.u16(g, axis);
        o.set_u16(g.b, axis, v);
    }
    let z = o.u16(g.b, Obj::Z).wrapping_add(0x48);
    o.set_u16(g.b, Obj::Z, z);
    let y = o.u16(g.b, Obj::Y).wrapping_sub(1);
    o.set_u16(g.b, Obj::Y, y);
    o.set_u8(g.b, 0x5d, step);
}

/// 0x4ccac : créature invoquée, tournée comme le joueur.
pub fn summon(g: &mut Ctx, p: Player) {
    cov!();
    let Some(o) = create(g, p, &pools::PEFF, kind::SUMMON) else { return };
    let facing = p.u8(g, Player::FACING);
    o.set_u8(g.b, Player::FACING, facing);
    link(g, o, p);
}

/// 0x4d3c6 : portail, variante `v`.
pub fn portal(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    effect(g, p, kind::PORTAL, Some(v), |_, _| {});
}

// ---- effets des magies lancées ---------------------------------------------------------------------

/// 0x4e2f4 : anneau (magie 16), variante `v` ; l'objet garde le pas de l'action.
pub fn ring(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    let step = step_of(g, p);
    effect(g, p, kind::RING, Some(v), |g, o| o.set_u8(g.b, 0x5b, step));
}

/// 0x4b196 : rayon (magie 17), variante `v`.
pub fn beam_cast(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    effect(g, p, kind::BEAM_CAST, Some(v), |g, o| o.set_u8(g.b, field::PARAM, 0));
}

/// 0x4ec66 : tempête (magie 18), variante `v`.
pub fn storm(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    effect(g, p, kind::STORM, Some(v), |_, _| {});
}

/// 0x5012e : météores (magie 20).
pub fn meteors(g: &mut Ctx, p: Player) {
    cov!();
    effect(g, p, kind::METEORS, None, |_, _| {});
}

/// 0x4bb68 : gel (magie 15), colonnes de glace : quatre effets (paramètres 0 à 3, décalages 1, 9, 17, 25).
pub fn ice_columns(g: &mut Ctx, p: Player) {
    cov!();
    for n in 0..4u8 {
        let made = effect(g, p, kind::ICE, Some(0), |g, o| {
            o.set_u16(g.b, 0x54, 1 + 8 * n as u16);
            o.set_u8(g.b, field::PARAM, n);
        });
        if made.is_none() {
            return;
        }
    }
}

/// Le gel (magie 15, une partie) : sol gelé, colonnes, brume et éclats, créés l'un après l'autre même si l'un échoue.
pub fn ice_spell(g: &mut Ctx, p: Player) {
    cov!();
    ice_ground(g, p);
    ice_columns(g, p);
    ice_mist(g, p);
    ice_shards(g, p);
}

/// 0x4bbbe : gel, brume de glace (variante 2).
pub fn ice_mist(g: &mut Ctx, p: Player) {
    cov!();
    effect(g, p, kind::ICE, Some(2), |_, _| {});
}

/// 0x4bbf6 : gel, six éclats (paramètres 0, 2... 10) qui naissent à la position du lanceur, un pixel plus bas.
pub fn ice_shards(g: &mut Ctx, p: Player) {
    cov!();
    let facing = p.u8(g, Player::FACING);
    let (x, z, y) = (p.u16(g, Obj::X), p.u16(g, Obj::Z), p.u16(g, Obj::Y));
    for n in 0..6u8 {
        let created = create(g, p, &pools::PEFF, kind::ICE);
        let Some(o) = created else { return };
        o.set_u8(g.b, Obj::VARIANT, 4);
        o.set_u8(g.b, Player::FACING, facing);
        o.set_u8(g.b, field::PARAM, 2 * n);
        o.set_u16(g.b, 0x54, 2 + 2 * n as u16);
        o.set_u16(g.b, Obj::X, x);
        o.set_u16(g.b, Obj::Z, z);
        o.set_u16(g.b, Obj::Y, y.wrapping_sub(1));
    }
}

/// 0x4bc42 : gel, dernier effet (variante 5, garde le pas de l'action).
pub fn ice_crown(g: &mut Ctx, p: Player) {
    cov!();
    let step = step_of(g, p);
    effect(g, p, kind::ICE, Some(5), |g, o| o.set_u8(g.b, 0x56, step));
}

// ---- créateurs de tirs joueur ---------------------------------------------------------------------

/// Les huit tirs d'objets lancés (attaque spéciale, numéros 0 à 8 sauf 3), dans l'ordre de leurs adresses d'origine :
/// 0x51d02, 0x51d54, 0x51f6a, 0x51fb6, 0x52002, 0x52204, 0x5308c, 0x52394.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Thrown {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
}

/// Description d'un tir lancé : genre, puissances (contre les ennemis ordinaires et contre les boss), vitesse horizontale
/// de départ dans le sens du regard (les tirs F, G et H n'en ont pas : leur gestionnaire les place), le tir copie
/// l'orientation du lanceur ou (A et B) choisit une direction d'animation.
struct Shot {
    kind: u8,
    power: (u8, u8),
    speed: Option<u16>,
    copies_facing: bool,
}

impl Thrown {
    fn shot(self) -> Shot {
        let (kind, power, speed, copies_facing) = match self {
            Thrown::A => (1, (0xc, 0xc), Some(0x700), false),
            Thrown::B => (9, (0x12, 0x12), Some(0x700), false),
            Thrown::C => (2, (0xc, 6), Some(0xa00), true),
            Thrown::D => (0xa, (0x12, 0x12), Some(0xa00), true),
            Thrown::E => (0xb, (0x10, 0x10), Some(0xa00), true),
            Thrown::F => (3, (0x10, 0x10), None, false),
            Thrown::G => (0xc, (0x14, 0x14), None, false),
            Thrown::H => (4, (0xe, 0xe), None, false),
        };
        Shot { kind, power, speed, copies_facing }
    }
}

/// 0x51d02 à 0x5308c : tir d'un objet lancé. Renvoie faux si le pool est plein (D0 = 0 et Z levé comme l'original).
pub fn thrown(g: &mut Ctx, p: Player, which: Thrown) -> bool {
    cov!();
    let shot = which.shot();
    let Some(o) = create(g, p, &pools::PSHL, shot.kind) else {
        g.c.d[0] = 0;
        g.c.flags_logic(0, Size::Long);
        return false;
    };
    o.set_u8(g.b, 0x2a, 2);
    o.set_u8(g.b, 0x45, shot.power.0);
    o.set_u8(g.b, 0x46, shot.power.1);
    let facing = p.u8(g, Player::FACING);
    if shot.copies_facing {
        o.set_u8(g.b, Player::FACING, facing);
    }
    o.set_u16(g.b, field::LINK, p.0 as u16);
    let Some(speed) = shot.speed else {
        g.c.flags_logic(p.0 & 0xffff, Size::Word);
        return true;
    };
    o.set_u16(g.b, Obj::VX, speed);
    o.set_u16(g.b, Obj::AX, 0);
    o.set_u32(g.b, Obj::VZ, 0);
    // les tirs A et B choisissent leur direction d'animation (2 vers la droite, 6 vers la gauche)
    let directional = !shot.copies_facing;
    if directional {
        o.set_u8(g.b, Obj::SPEED_INDEX, 2);
    }
    g.c.flags_logic(facing as u32, Size::Byte);
    if facing == 0 {
        if directional {
            o.set_u8(g.b, Obj::SPEED_INDEX, 6);
        }
        let reversed = g.c.addsub(true, speed as u32, 0, false, Size::Word, false);
        o.set_u16(g.b, Obj::VX, reversed as u16);
    }
    true
}

/// 0x54522 : boule de feu (magie 14), tir de genre 0x16.
pub fn fireball(g: &mut Ctx, p: Player) {
    cov!();
    let Some(o) = create(g, p, &pools::PSHL, kind::FIREBALL) else { return };
    link(g, o, p);
}

/// 0x52b54 : gel, sol gelé (magie 15) : deux tirs de genre 7 (variantes 0 et 1). Renvoie faux si le pool est plein.
pub fn ice_ground(g: &mut Ctx, p: Player) -> bool {
    cov!();
    for v in 0..2 {
        let Some(o) = create(g, p, &pools::PSHL, kind::ICE_GROUND) else {
            g.c.d[0] = 0;
            g.c.flags_logic(0, Size::Long);
            return false;
        };
        o.set_u8(g.b, Obj::VARIANT, v);
        link(g, o, p);
    }
    true
}

/// 0x54426 : couteau lancé (tir de genre 0x15, puissances 10 et 5). Renvoie faux si le pool est plein.
pub fn knife(g: &mut Ctx, p: Player) -> bool {
    cov!();
    let Some(o) = create(g, p, &pools::PSHL, kind::KNIFE) else {
        g.c.d[0] = 0;
        g.c.flags_logic(0, Size::Long);
        return false;
    };
    o.set_u8(g.b, 0x45, 0xa);
    o.set_u8(g.b, 0x46, 5);
    link(g, o, p);
    true
}

/// 0x52e20 : six ombres du coup de pied arrière (tirs de genre 8, variantes 5 à 0).
pub fn shadows(g: &mut Ctx, p: Player) {
    cov!();
    for v in (0..6).rev() {
        let Some(o) = create(g, p, &pools::PSHL, kind::SHADOW) else { return };
        o.set_u8(g.b, Obj::VARIANT, v);
        link(g, o, p);
    }
}

/// Ombre du coup de pied sauté (tir de genre 8, paramètre 2), tel que le crée 0x49212 du voleur.
pub fn shadow(g: &mut Ctx, p: Player) {
    cov!();
    let Some(o) = create(g, p, &pools::PSHL, kind::SHADOW) else { return };
    o.set_u8(g.b, field::PARAM, 2);
    link(g, o, p);
}

/// 0x53918 : la sorcière, cinq tirs (paramètres 0 à 4) ; l'octet `+0x2d` de chacun vient de la table 0x5394e.
pub fn witch(g: &mut Ctx, p: Player) {
    cov!();
    const SLOTS: u32 = 0x5394e;
    for n in 0..5u8 {
        let Some(o) = create(g, p, &pools::PSHL, kind::WITCH) else { return };
        let facing = p.u8(g, Player::FACING);
        o.set_u8(g.b, Player::FACING, facing);
        o.set_u16(g.b, field::LINK, p.0 as u16);
        let slot = g.prog8(SLOTS + n as u32);
        o.set_u8(g.b, 0x2d, slot);
        o.set_u8(g.b, 0x5d, n);
    }
}

/// 0x53954 (variante 1) et 0x539e4 (variante 4) : coups de la sorcière ; leurs puissances (contre les ennemis
/// ordinaires et contre les boss) dépendent du niveau du joueur (`+0x10c`).
pub fn witch_strike(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    const POWER: u32 = 0x10_41f2;
    const POWER_BOSS: u32 = 0x10_4210;
    let Some(o) = create(g, p, &pools::PSHL, kind::WITCH) else { return };
    o.set_u8(g.b, Obj::VARIANT, v);
    o.set_u16(g.b, field::LINK, p.0 as u16);
    let level = p.u8(g, 0x10c) as u32;
    let (power, boss) = (g.data8(POWER + level), g.data8(POWER_BOSS + level));
    o.set_u8(g.b, 0x45, power);
    o.set_u8(g.b, 0x46, boss);
}

// ---- créateurs sans native jusqu'ici (atteignables, jamais exercés par les scénarios) --------------------

/// 0x4fefe : l'arme maudite qui s'échappe des mains (`checks::item_curse`), avec la sous-action en cours (`+0x5c`).
pub fn cursed_weapon(g: &mut Ctx, p: Player) {
    cov!();
    let sub = p.u8(g, Player::SUBACTION);
    effect(g, p, kind::CURSED_WEAPON, None, |g, o| o.set_u8(g.b, 0x5c, sub));
}

/// Recopie l'orientation et la position du joueur dans l'éclat, un peu devant lui (profondeur + 1).
fn shard_place(g: &mut Ctx, p: Player, o: Obj) {
    let facing = p.u8(g, Player::FACING);
    o.set_u8(g.b, Player::FACING, facing);
    let (x, z) = (p.u16(g, Obj::X), p.u16(g, Obj::Z));
    o.set_u16(g.b, Obj::X, x);
    o.set_u16(g.b, Obj::Z, z);
    let y = p.u32(g, Obj::Y).wrapping_add(1);
    o.set_u32(g.b, Obj::Y, y);
}

/// 0x5009e : éclats du joueur mourant (`effects::dying_countdown`) : le premier reprend le cadre et la palette du
/// joueur (`+0x34`, `+0x3c`, `+0x3f`) ; un second (variante 1), qui retient le joueur (`+0x42`), si `+0x42` est non
/// nul. Rien sans `+0x68`, ni pendant un scénario.
pub fn dying_shards(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, 0x68) == 0 || ram::INPUT_RAW.get(g.b) != 0 {
        return;
    }
    let Some(o) = create(g, p, &pools::PEFF, kind::SHARD) else { return };
    shard_place(g, p, o);
    let frame = p.u32(g, Obj::FRAME);
    o.set_u32(g.b, Obj::FRAME, frame);
    for off in [0x3c, 0x3f] {
        let v = p.u8(g, off);
        o.set_u8(g.b, off, v);
    }
    if p.u16(g, 0x42) == 0 {
        return;
    }
    let Some(o) = create(g, p, &pools::PEFF, kind::SHARD) else { return };
    o.set_u8(g.b, Obj::VARIANT, 1);
    shard_place(g, p, o);
    o.set_u16(g.b, 0x42, p.0 as u16);
}

/// 0x51aa4 : nuée de l'orage (genre 0x2e), en deux moitiés : la seconde (variante 1) tournée vers la gauche.
pub fn storm_cloud(g: &mut Ctx, p: Player) {
    cov!();
    if create(g, p, &pools::PEFF, kind::STORM_CLOUD).is_none() {
        return;
    }
    let Some(o) = create(g, p, &pools::PEFF, kind::STORM_CLOUD) else { return };
    o.set_u8(g.b, Obj::VARIANT, 1);
    o.set_u8(g.b, Player::FACING, 0x20);
}

/// 0x51872 : lueur du portail (genre 0x2d) au-dessus du joueur (0x48 plus haut, un peu devant), avec le pas de
/// l'action (`+0x5d`).
pub fn portal_glow(g: &mut Ctx, p: Player) {
    cov!();
    let Some(o) = create(g, p, &pools::PEFF, kind::PORTAL_GLOW) else { return };
    o.set_u16(g.b, field::LINK, p.0 as u16);
    let facing = p.u8(g, Player::FACING);
    o.set_u8(g.b, Player::FACING, facing);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = p.u16(g, off);
        o.set_u16(g.b, off, v);
    }
    let z = o.u16(g.b, Obj::Z).wrapping_add(0x48);
    o.set_u16(g.b, Obj::Z, z);
    let y = o.u16(g.b, Obj::Y).wrapping_sub(1);
    o.set_u16(g.b, Obj::Y, y);
    let step = step_of(g, p);
    o.set_u8(g.b, 0x5d, step);
}

/// 0x4cfee : quatre étincelles du portail (genre 0x13, variante 1, numéros 3 à 0 en `+0x2f`), puis l'écran prend la
/// couleur de l'éclair (0x1ac68), même si le pool est plein.
pub fn portal_sparks(g: &mut Ctx, p: Player) {
    cov!();
    for n in (0..=3u8).rev() {
        let Some(o) = create(g, p, &pools::PEFF, kind::PORTAL_SPARK) else { break };
        o.set_u8(g.b, Obj::VARIANT, 1);
        o.set_u8(g.b, field::PARAM, n);
        o.set_u8(g.b, 0x5a, 4);
        link(g, o, p);
    }
    crate::palette::set_flash_color(g.b, crate::palette::FLASH_COLOR_RED);
}

/// 0x4d020 : effet du sort d'orage lancé vers la gauche (genre 0x14, gestionnaire 0x4d08c), lié au joueur.
pub fn storm_spell(g: &mut Ctx, p: Player) {
    cov!();
    effect(g, p, kind::STORM_SPELL, None, |_, _| {});
}

/// 0x4d660 : lueur de la pluie (genre 0x17), variante `v` (0 : préparation, 1 : lancer), avec le pas de l'action
/// du joueur (`+0x5d`).
pub fn rain_glow(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    let step = step_of(g, p);
    effect(g, p, kind::RAIN_GLOW, Some(v), |g, o| o.set_u8(g.b, 0x5d, step));
}

/// 0x4d2c0 : étincelle du mur de feu (genre 0x15), variante `v` (0 : préparation, 1 : lancer).
pub fn wall_spark(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    effect(g, p, kind::WALL_SPARK, Some(v), |_, _| {});
}

/// 0x4dfe2 : foudre de la vague (genre 0x1a) avec le pas de l'action du joueur (`+0x59`). `columns` faux : la
/// colonne (variante laissée par l'allocation), puis le mot `+0x12` du joueur passe à 0x8000 si elle a pu être
/// créée ; vrai : deux effets de variante 1 (paramètres 1 puis 0).
pub fn wave_bolt(g: &mut Ctx, p: Player, columns: bool) {
    cov!();
    let step = step_of(g, p);
    if !columns {
        if effect(g, p, kind::WAVE_BOLT, None, |g, o| o.set_u8(g.b, 0x59, step)).is_some() {
            p.set_u16(g, 0x12, 0x8000);
        }
        return;
    }
    for param in [1, 0] {
        let made = effect(g, p, kind::WAVE_BOLT, Some(1), |g, o| {
            o.set_u8(g.b, field::PARAM, param);
            o.set_u8(g.b, 0x59, step);
        });
        if made.is_none() {
            return;
        }
    }
}

/// 0x4e4de : lune de l'elfe (genre 0x1c), variante `v` (0, 2 et 3 : les trois lunes posées ; 1 : la frappe).
pub fn moon(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    effect(g, p, kind::MOON, Some(v), |_, _| {});
}

/// 0x4e782 : second effet de la lune de l'elfe (genre 0x1e).
pub fn moon_b(g: &mut Ctx, p: Player) {
    cov!();
    effect(g, p, kind::MOON_B, None, |_, _| {});
}

/// 0x4d064 : éclat du portail (genre 0x14, variante 1) numéroté `n` (`+0x5e`). Vrai s'il a pu être créé (D0 = 0).
pub fn portal_burst(g: &mut Ctx, p: Player, n: u8) -> bool {
    cov!();
    effect(g, p, kind::PORTAL_BURST, Some(1), |g, o| o.set_u8(g.b, 0x5e, n)).is_some()
}

/// 0x54274 : pluie de traits (tirs PSHL de genre 0x14, variantes 0, 1, 2...) : un trait tous les 0x40 pixels, à
/// partir de 0x80 pixels derrière le joueur, jusqu'au bord de l'écran devant lui (bord gauche de la caméra - 0x40,
/// ou + 0x1c0 en regardant à gauche). Les positions ne sont pas écrites : seul le nombre de traits en dépend.
pub fn rain(g: &mut Ctx, p: Player) {
    cov!();
    let left = p.faces_left(g);
    let toward = |d: u16| if left { d.wrapping_neg() } else { d };
    let mut x = p.u16(g, Obj::X).wrapping_add(toward(0xff80));
    let camera = if ram::USE_ALT_CAMERA.get(g.b) != 0 { ram::CAMERA_ALT_X.get(g.b) } else { ram::CAMERA_X.get(g.b) };
    let edge = camera.wrapping_add(if left { 0x1c0 } else { 0xffc0 });
    let mut n = 0u8;
    loop {
        let Some(o) = create(g, p, &pools::PSHL, kind::RAIN) else { return };
        let facing = p.u8(g, Player::FACING);
        o.set_u8(g.b, Player::FACING, facing);
        o.set_u8(g.b, Obj::VARIANT, n);
        o.set_u16(g.b, field::LINK, p.0 as u16);
        x = x.wrapping_add(toward(0xffc0));
        if (toward(x.wrapping_sub(edge)) as i16) < 0 {
            return;
        }
        n = n.wrapping_add(1);
    }
}

// ---- natives --------------------------------------------------------------------------------------

/// Exécute un créateur sur l'état des registres du 68000 (A0 = le joueur) puis rend la main.
fn run(c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, f: impl FnOnce(&mut Ctx, Player)) {
    let p = Player(c.a[0]);
    f(&mut Ctx::new(c, b), p);
    rts(c, b);
}

/// Déclare la native d'un créateur à son adresse d'origine ; `$body` lit ses paramètres dans les
/// registres (`g.c.d[0]`...).
macro_rules! creator {
    ($name:ident, $entry:expr, $label:expr, $outputs:expr, |$g:ident, $p:ident| $body:expr) => {
        pub const $name: Native = Native {
            entry: $entry,
            name: $label,
            run: crate::Corps::Processeur(|c, b| run(c, b, |$g, $p| $body)),
            touches_sound: false,
            outputs: $outputs,
        };
    };
}

creator!(SLASH, 0x4a99a, "spawn_slash", regs::a(1), |g, p| {
    let (d0, d1) = (g.c.d[0] as u8, g.c.d[1] as u8);
    slash(g, p, if d0 == 0 { Some(d1) } else { None })
});
creator!(BLOOD, 0x4a860, "spawn_blood", regs::a(1), |g, p| blood(g, p));
creator!(CURE, 0x4ab48, "spawn_cure", regs::a(1), |g, p| {
    let n = g.c.d[0] as u8;
    cure(g, p, n)
});
creator!(POTION, 0x4ab64, "spawn_potion", regs::a(1), |g, p| potion(g, p));
creator!(HOLY_LIGHT, 0x4aeb2, "spawn_holy_light", regs::a(1), |g, p| holy_light(g, p));
creator!(BEAM, 0x4b5ac, "spawn_beam", regs::a(1), |g, p| {
    let aimed = g.c.d[0] as u8 != 0;
    beam(g, p, aimed)
});
creator!(BLESSING, 0x4c19a, "spawn_blessing", regs::a(1), |g, p| {
    let v = g.c.d[0] as u8;
    blessing(g, p, v)
});
creator!(BANISH, 0x4c26c, "spawn_banish", regs::a(1), |g, p| {
    let foe = (g.c.d[0] as u8 != 0).then_some(g.c.a[2]);
    banish(g, p, foe)
});
creator!(WEAPON_TRAIL, 0x4c58e, "spawn_weapon_trail", regs::a(1) | regs::d(1), |g, p| {
    let v = g.c.d[0] as u8;
    // `moveq 0,D1 ; move.b WEAPON_A,D1b ; subi.b #0x10,D1b`
    g.c.d[1] = p.u8(g, Player::WEAPON_A).wrapping_sub(0x10) as u32;
    weapon_trail(g, p, v)
});
creator!(SHOWER, 0x4dcda, "spawn_shower", regs::a(1), |g, p| {
    let side = g.c.d[0] as u8;
    shower(g, p, side)
});
creator!(AURA, 0x4e6a8, "spawn_aura", regs::a(1), |g, p| {
    let v = g.c.d[0] as u8;
    aura(g, p, v)
});
creator!(LIGHTNING_TARGET, 0x4f004, "spawn_lightning_target", regs::a(1) | regs::d(0) | regs::CCR, |g, p| {
    let v = g.c.d[0] as u8;
    let found = lightning_target(g, p, v).is_some();
    g.c.d[0] = found as u32;
    g.c.flags_logic(found as u32, Size::Long);
});
creator!(SWARM, 0x4f272, "spawn_swarm", regs::a(1), |g, p| {
    let count = (g.c.d[0] as u16 as u32) + 1;
    swarm(g, p, count)
});
creator!(LEAP, 0x4fbde, "spawn_leap", regs::a(1), |g, p| {
    let v = g.c.d[0] as u8;
    leap(g, p, v)
});
creator!(SWAP, 0x4fd2c, "spawn_swap", regs::a(1), |g, p| {
    let v = g.c.d[0] as u8;
    swap(g, p, v)
});
creator!(DASH, 0x4fe18, "spawn_dash", regs::a(1), |g, p| dash(g, p));
creator!(WALL_TARGET, 0x50324, "spawn_wall_target", regs::a(1) | regs::CCR, |g, p| {
    wall_target(g, p);
});
creator!(LIGHTNING, 0x4cdd0, "spawn_lightning", regs::a(1), |g, p| {
    let target = g.c.a[2];
    lightning(g, p, target)
});
creator!(RING, 0x4e2f4, "spawn_ring", regs::a(1), |g, p| {
    let v = g.c.d[0] as u8;
    ring(g, p, v)
});
creator!(BEAM_CAST, 0x4b196, "spawn_beam_cast", regs::a(1), |g, p| {
    let v = g.c.d[0] as u8;
    beam_cast(g, p, v)
});
creator!(STORM, 0x4ec66, "spawn_storm", regs::a(1), |g, p| {
    let v = g.c.d[0] as u8;
    storm(g, p, v)
});
creator!(METEORS, 0x5012e, "spawn_meteors", regs::a(1), |g, p| meteors(g, p));
creator!(ICE_COLUMNS, 0x4bb68, "spawn_ice_columns", regs::a(1), |g, p| ice_columns(g, p));
creator!(ICE_MIST, 0x4bbbe, "spawn_ice_mist", regs::a(1), |g, p| ice_mist(g, p));
creator!(ICE_SHARDS, 0x4bbf6, "spawn_ice_shards", regs::a(1), |g, p| ice_shards(g, p));
creator!(ICE_CROWN, 0x4bc42, "spawn_ice_crown", regs::a(1), |g, p| ice_crown(g, p));
creator!(FIREBALL, 0x54522, "spawn_fireball", regs::a(1), |g, p| fireball(g, p));
creator!(ICE_GROUND, 0x52b54, "spawn_ice_ground", regs::a(1) | regs::d(0) | regs::CCR, |g, p| {
    ice_ground(g, p);
});
creator!(THROWN_A, 0x51d02, "spawn_thrown_a", regs::a(1) | regs::d(0) | regs::CCR, |g, p| {
    thrown(g, p, Thrown::A);
});
creator!(THROWN_B, 0x51d54, "spawn_thrown_b", regs::a(1) | regs::d(0) | regs::CCR, |g, p| {
    thrown(g, p, Thrown::B);
});
creator!(THROWN_C, 0x51f6a, "spawn_thrown_c", regs::a(1) | regs::d(0) | regs::CCR, |g, p| {
    thrown(g, p, Thrown::C);
});
creator!(THROWN_D, 0x51fb6, "spawn_thrown_d", regs::a(1) | regs::d(0) | regs::CCR, |g, p| {
    thrown(g, p, Thrown::D);
});
creator!(THROWN_E, 0x52002, "spawn_thrown_e", regs::a(1) | regs::d(0) | regs::CCR, |g, p| {
    thrown(g, p, Thrown::E);
});
creator!(THROWN_F, 0x52204, "spawn_thrown_f", regs::a(1) | regs::d(0) | regs::CCR, |g, p| {
    thrown(g, p, Thrown::F);
});
creator!(THROWN_G, 0x5308c, "spawn_thrown_g", regs::a(1) | regs::d(0) | regs::CCR, |g, p| {
    thrown(g, p, Thrown::G);
});
creator!(THROWN_H, 0x52394, "spawn_thrown_h", regs::a(1) | regs::d(0) | regs::CCR, |g, p| {
    thrown(g, p, Thrown::H);
});
creator!(KNIFE, 0x54426, "spawn_knife", regs::a(1) | regs::d(0) | regs::CCR, |g, p| {
    knife(g, p);
});
creator!(SHADOWS, 0x52e20, "spawn_shadows", regs::a(1), |g, p| shadows(g, p));
creator!(FLARE, 0x4cd8e, "spawn_flare", regs::a(1), |g, p| flare(g, p));
creator!(SUMMON, 0x4ccac, "spawn_summon", regs::a(1), |g, p| summon(g, p));
creator!(PORTAL, 0x4d3c6, "spawn_portal", regs::a(1), |g, p| {
    let v = g.c.d[0] as u8;
    portal(g, p, v)
});
creator!(WITCH, 0x53918, "spawn_witch", regs::a(1), |g, p| witch(g, p));
creator!(CURSED_WEAPON, 0x4fefe, "spawn_cursed_weapon", regs::a(1) | regs::CCR, |g, p| cursed_weapon(g, p));
creator!(DYING_SHARDS, 0x5009e, "spawn_dying_shards", 0, |g, p| dying_shards(g, p));
creator!(STORM_CLOUD, 0x51aa4, "spawn_storm_cloud", regs::a(1), |g, p| storm_cloud(g, p));
creator!(PORTAL_GLOW, 0x51872, "spawn_portal_glow", regs::a(1), |g, p| portal_glow(g, p));
creator!(PORTAL_SPARKS, 0x4cfee, "spawn_portal_sparks", 0, |g, p| portal_sparks(g, p));
creator!(PORTAL_BURST, 0x4d064, "spawn_portal_burst", regs::a(1) | regs::d(0) | regs::CCR, |g, p| {
    let n = g.c.d[0] as u8;
    let made = portal_burst(g, p, n);
    // `moveq #0` si l'éclat est créé, `moveq #1` sinon
    g.c.d[0] = (!made) as u32;
    g.c.flags_logic(g.c.d[0], Size::Long);
});
creator!(RAIN, 0x54274, "spawn_rain", regs::a(1), |g, p| rain(g, p));
creator!(MOON, 0x4e4de, "spawn_moon", regs::a(1), |g, p| {
    let v = g.c.d[0] as u8;
    moon(g, p, v)
});
creator!(MOON_B, 0x4e782, "spawn_moon_b", regs::a(1), |g, p| moon_b(g, p));
creator!(STORM_SPELL, 0x4d020, "spawn_storm_spell", regs::a(1), |g, p| storm_spell(g, p));
creator!(RAIN_GLOW, 0x4d660, "spawn_rain_glow", regs::a(1), |g, p| {
    let v = g.c.d[0] as u8;
    rain_glow(g, p, v)
});
creator!(WALL_SPARK, 0x4d2c0, "spawn_wall_spark", regs::a(1), |g, p| {
    let v = (g.c.d[0] as u8 != 0) as u8;
    wall_spark(g, p, v)
});
creator!(WAVE_BOLT, 0x4dfe2, "spawn_wave_bolt", regs::a(1), |g, p| {
    let columns = g.c.d[0] as u8 != 0;
    wave_bolt(g, p, columns)
});
creator!(WITCH_A, 0x53954, "spawn_witch_a", regs::a(1), |g, p| witch_strike(g, p, 1));
creator!(WITCH_B, 0x539e4, "spawn_witch_b", regs::a(1), |g, p| witch_strike(g, p, 4));
