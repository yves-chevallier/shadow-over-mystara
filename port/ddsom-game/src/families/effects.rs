//! Famille EFFCT (classe 0xE) : effets visuels (étincelles, éclats, bannières, éléments d'interface
//! animés). Dispatch `0x39cba`, table des types `0x39ce0` (92 types, 38 emplacements de 0x60 octets).
//!
//! Tous les gestionnaires ont la même charpente : l'octet d'état `+4` choisit, par une table de mots
//! relatifs, l'initialisation (0), l'état actif (2) ou la libération (4). Les types portés ici sont
//! ceux que les scénarios exercent ; les autres restent en code d'origine. Voir `docs/families.md`.

use crate::objects::pools;
use crate::objects::Obj;

use super::{ajoute_u8, handler, pointeur};
use super::cx::{Bench, Cx};
use crate::objects::Current;
use crate::machine::Machine;

/// Valeurs de l'octet d'état `+4`.
mod state {
    pub const INIT: u8 = 0;
    pub const ACTIVE: u8 = 2;
    pub const LIBERE: u8 = 4;
    /// Posé par l'objet parent qui détruit ses effets (ou par l'effet lui-même, `accessoire_suit`) : libération,
    /// sans le travail de l'état 4 (`jmp 0x2218`).
    pub const DETRUIT: u8 = 6;
}

/// Champs partagés.
mod field {
    pub const SUB_STATE: u32 = 0x05;
    pub const MODE: u32 = 0x2f;
    pub const FLIP: u32 = 0x2c;
    pub const OWNER: u32 = 0x50;
}

/// Joueurs (4 enregistrements de 0x200 octets).
const PLAYERS: u32 = 0xff85de;
const STEP_PLAYER: u32 = 0x200;
/// Jeu d'animations « interface » commun à de nombreux effets.
const ANIMATIONS_INTERFACE: u32 = 0x1c03a6;

/// Cible du saut par table `move.w (table,PC,Dn.w),D1 ; jmp (table,PC,D1.w)` pour l'indice `index` (l'adresse que
/// l'original atteindrait ; seuls les indices hors table y mènent encore, `outside_table`).
pub(crate) fn target<R: Bench>(ctx: &mut Cx<'_, R>, table: u32, index: u32) -> u32 {
    let word = ctx.prog16(table.wrapping_add(index)) as i16;
    table.wrapping_add(word as i32 as u32)
}

/// Valeur d'état ou de variante hors de la table de sauts de l'original, qui sauterait dans des données ou au milieu
/// du code : jamais produite (docs/code-mort.md, lot B).
pub(crate) fn outside_table(target: u32) -> ! {
    unreachable!("indice hors de la table de sauts : l'original sauterait en {target:#x}")
}

/// Pool plein là où l'original saute en `address` : `tst.w (1,A5)`, lecture d'un mot à une adresse impaire. Le 68000
/// lève une erreur d'adresse et le jeu s'arrête sur son écran d'exception ; le moteur s'arrête aussi
/// (docs/code-mort.md, lot B).
pub(crate) fn address_error(address: u32) -> ! {
    panic!("pool plein : l'original lit un mot à une adresse impaire en {address:#x} (erreur d'adresse, écran d'exception)")
}

/// Passe `X`, `Z`, `Y` (mots hauts) de `o` à ceux de `source`.
fn copy_position<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, source: Obj) {
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = source.u16(ctx.b, off);
        o.set_u16(ctx.b, off, v);
    }
}

/// Ajoute `n` au mot haut d'une coordonnée.
fn shifted_u16<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, off: u32, n: u16) {
    let v = o.u16(ctx.b, off).wrapping_add(n);
    o.set_u16(ctx.b, off, v);
}

/// Gestionnaire à trois états (init, actif, libération) : lit l'état, saute par la table de mots
/// `table`, exécute `init` ou `active` ; l'état 4 libère l'objet.
fn three_states<R: Bench>(
    ctx: &mut Cx<'_, R>,
    table: u32,
    init: fn(&mut Cx<'_, R>, Obj),
    active: fn(&mut Cx<'_, R>, Obj),
) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, table, state as u32);
    match state {
        state::INIT => init(ctx, o),
        state::ACTIVE => active(ctx, o),
        state::LIBERE => ctx.free(&pools::EFFCT),
        state::DETRUIT => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 5 : halo attaché à un joueur (0xab726)

/// Type 5 : apparaît à la position du joueur propriétaire (`+0x50`), décalée selon la variante (paire,
/// 0 à 10) et l'orientation du joueur, attend un délai (octet `+0x18`, fonction de l'octet `+0x7d` du
/// joueur) puis vit `0x28` images avant de disparaître. Animation 0 du jeu `0x18e650`.
mod halo {
    pub const ENTRY: u32 = 0xab726;
    pub const TABLE: u32 = 0xab734;
    /// Décalages (dx, dz) par demi-variante, mots.
    pub const OFFSETS: u32 = 0xab79a;
    /// Délais d'apparition selon `+0x7d` du joueur.
    pub const DELAYS: u32 = 0xab7b2;
    pub const ANIMATIONS: u32 = 0x18e650;
    pub const DURATION: u32 = 0x14;
    pub const DELAY: u32 = 0x18;
}

fn halo_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, halo::DURATION, 0x28);
    let player = pointeur(ctx, o, field::OWNER);
    copy_position(ctx, o, player);
    let rank = ((o.u8(ctx.b, Obj::VARIANT) >> 1) as u32) << 2;
    let mut dx = ctx.prog16(halo::OFFSETS + rank);
    let dz = ctx.prog16(halo::OFFSETS + 2 + rank);
    let retourne = player.u8(ctx.b, field::FLIP) != 0;
    if !retourne {
        dx = dx.wrapping_neg();
    }
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Z, dz);
    let phase = player.u8(ctx.b, 0x7d) as u32;
    let delay = ctx.prog8(halo::DELAYS + phase);
    o.set_u8(ctx.b, halo::DELAY, delay);
    ctx.anim_start(halo::ANIMATIONS, 0);
}

fn halo_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let delay = o.u8(ctx.b, halo::DELAY);
    if !(delay == 0) {
        o.set_u8(ctx.b, halo::DELAY, delay - 1);
        return;
    }
    let remainder = o.u8(ctx.b, halo::DURATION).wrapping_sub(1);
    o.set_u8(ctx.b, halo::DURATION, remainder);
    if !(remainder != 0) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
    ctx.reinsert();
}

fn halo<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, halo::TABLE, halo_init, halo_active)
}

// ---------------------------------------------------------------------------------------------
// Type 21 : bandeau d'interface qui glisse (0xaf360)

/// Type 21 : bandeau d'interface (variante 0 : en bas, 1 : en haut) qui glisse en `Z` d'un pixel par
/// image (tant que `A5+0x6f` bit 0 est nul) jusqu'à la hauteur cible `+0x18`, puis se libère en
/// décrémentant l'octet pointé par `+0x58` (compteur de bandeaux du créateur).
mod banner {
    pub const ENTRY: u32 = 0xaf360;
    pub const TABLE: u32 = 0xaf36e;
    pub const TARGET: u32 = 0x18;
    pub const CREATOR: u32 = 0x58;
}

fn banner_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u32(ctx.b, Obj::X, 0);
    o.set_u32(ctx.b, Obj::Y, 0);
    o.set_u16(ctx.b, Obj::Z, 0xc8);
    o.set_u16(ctx.b, banner::TARGET, 0x100);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if !(variant == 0) {
        o.set_u16(ctx.b, Obj::Z, 0x88);
        o.set_u16(ctx.b, banner::TARGET, 0x50);
    }
    ctx.anim_start(ANIMATIONS_INTERFACE, 0xec);
    ctx.reinsert();
}

/// Fin du bandeau : décrémente le compteur du créateur et libère l'objet.
fn banner_end<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let counter = crate::mem::rd32(ctx.b, o.0 + banner::CREATOR);
    let v = crate::mem::rd8(ctx.b, counter).wrapping_sub(1);
    crate::mem::wr8(ctx.b, counter, v);
    ctx.free(&pools::EFFCT);
}

fn banner_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let pause = ctx.ram8(0x6f) & 1 != 0;
    if !pause {
        let variant = o.u8(ctx.b, Obj::VARIANT);
        let step: u16 = if variant == 0 { 1 } else { 1u16.wrapping_neg() };
        shifted_u16(ctx, o, Obj::Z, step);
        let target = o.u16(ctx.b, banner::TARGET);
        let z = o.u16(ctx.b, Obj::Z);
        if target == z {
            return banner_end(ctx, o);
        }
    }
    ctx.reinsert();
}

fn banner<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, banner::TABLE, state as u32);
    match state {
        state::INIT => banner_init(ctx, o),
        state::ACTIVE => banner_active(ctx, o),
        state::LIBERE => banner_end(ctx, o),
        state::DETRUIT => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 32 : aura qui suit son propriétaire (0xb2c56)

/// Type 32 : se place 0x10 pixels devant (selon son orientation) et un sous-pixel devant le
/// propriétaire (`+0x50`), 0x18 pixels au-dessus de lui, et le suit à chaque image (animation 0 du
/// jeu `0x193fea`).
mod aura {
    pub const ENTRY: u32 = 0xb2c56;
    pub const TABLE: u32 = 0xb2c64;
    pub const ANIMATIONS: u32 = 0x193fea;
}

/// `0xb2ca4` : position du propriétaire, un seizième de pixel devant, décalée de 0x10 pixels.
fn aura_follow<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> Obj {
    let player = pointeur(ctx, o, field::OWNER);
    let x = player.u32(ctx.b, Obj::X);
    o.set_u32(ctx.b, Obj::X, x);
    let y = player.u32(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u32(ctx.b, Obj::Y, y);
    let retourne = o.u8(ctx.b, field::FLIP) != 0;
    let offset: u16 = if !retourne { 0xfff0 } else { 0x10 };
    shifted_u16(ctx, o, Obj::X, offset);
    player
}

fn aura_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let player = aura_follow(ctx, o);
    let z = player.u32(ctx.b, Obj::Z);
    o.set_u32(ctx.b, Obj::Z, z);
    shifted_u16(ctx, o, Obj::Z, 0x18);
    ctx.anim_start(aura::ANIMATIONS, 0);
    ctx.reinsert();
}

fn aura_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    aura_follow(ctx, o);
    ctx.reinsert();
}

fn aura<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, aura::TABLE, aura_init, aura_active)
}

// ---------------------------------------------------------------------------------------------
// Type 15 : scintillement d'un objet (0xae428)

/// Type 15 : scintillement posé sur un objet ramassable (`+0x5c`, créé par `0xae3ec` avec la variante
/// `+0x54` de l'objet) ; suit l'objet, 0x10 pixels plus haut, tant qu'il est actif (état 2). Tant
/// qu'aucun joueur n'est passé à proximité (±0x20 en profondeur, ±0x60 en abscisse) il reste visible ;
/// ensuite il clignote par périodes de 0x30 images, huit fois, puis disparaît.
mod scintillement {
    pub const ENTRY: u32 = 0xae428;
    pub const TABLE: u32 = 0xae436;
    pub const ANIMATIONS: u32 = 0x18fd48;
    pub const REPETITIONS: u32 = 0x57;
    pub const DELAY: u32 = 0x58;
    pub const BASCULE: u32 = 0x5a;
    pub const DECLENCHE: u32 = 0x5b;
    pub const PORTEUR: u32 = 0x5c;
}

fn scintillement_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    o.set_u8(ctx.b, scintillement::REPETITIONS, 8);
    let rng = ctx.random();
    let draw = rng & 0x1f;
    o.set_u16(ctx.b, scintillement::DELAY, draw);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    ctx.anim_start(scintillement::ANIMATIONS, variant as u16);
    ctx.reinsert();
}

/// `0xae4e4` : un joueur est-il passé près du scintillement ?
fn shimmer_player_near<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    for i in 0..4u32 {
        let player = Obj(PLAYERS + i * STEP_PLAYER);
        let absent = player.u8(ctx.b, Obj::ACTIVE) == 0;
        if !absent {
            let dy = o.u16(ctx.b, Obj::Y).wrapping_sub(player.u16(ctx.b, Obj::Y)).wrapping_add(0x20);
            if !(dy > 0x40) {
                let dx = o.u16(ctx.b, Obj::X).wrapping_sub(player.u16(ctx.b, Obj::X)).wrapping_add(0x60);
                if !(dx > 0xc0) {
                    o.set_u8(ctx.b, scintillement::DECLENCHE, 1);
                    return;
                }
            }
        }
    }
}

fn shimmer_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let porteur = pointeur(ctx, o, scintillement::PORTEUR);
    copy_position(ctx, o, porteur);
    shifted_u16(ctx, o, Obj::Z, 0x10);
    let state_carrier = porteur.u8(ctx.b, Obj::STATE);
    if !(state_carrier == 2) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
    let declenche = o.u8(ctx.b, scintillement::DECLENCHE);
    if !(declenche != 0) {
        // pas encore déclenché : on guette les joueurs (et on rend la main sans trier)
        return shimmer_player_near(ctx, o);
    }
    let remainder = o.u16(ctx.b, scintillement::DELAY).wrapping_sub(1);
    o.set_u16(ctx.b, scintillement::DELAY, remainder);
    if !(remainder != 0) {
        o.set_u16(ctx.b, scintillement::DELAY, 0x30);
        let bascule = o.u8(ctx.b, scintillement::BASCULE) ^ 1;
        o.set_u8(ctx.b, scintillement::BASCULE, bascule);
        let flags = o.u8(ctx.b, Obj::FLAGS);
        o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
        if !(bascule != 0) {
            o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
            let remainder = o.u8(ctx.b, scintillement::REPETITIONS).wrapping_sub(1);
            o.set_u8(ctx.b, scintillement::REPETITIONS, remainder);
            if !(remainder != 0) {
                o.set_u32(ctx.b, Obj::STATE, 0x0400_0000);
            }
        }
    }
    ctx.reinsert();
}

fn scintillement<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, scintillement::TABLE, scintillement_init, shimmer_active)
}

// ---------------------------------------------------------------------------------------------
// Type 9 : traînée qui traverse l'écran (0xabbee)

/// Type 9 : objet qui naît à une position d'écran tirée d'une table (selon `+0x52`), avance en `X`
/// (vitesse 0x200), attend 0x120 images (animation `variant + 0xc`) puis joue son animation de
/// sortie 0x200 images, en avançant les 0x50 dernières, avant de disparaître.
mod trainee {
    pub const ENTRY: u32 = 0xabbee;
    pub const TABLE: u32 = 0xabbfc;
    pub const POSITIONS: u32 = 0xabc54;
    pub const ANIMATIONS: u32 = 0x18ea30;
    pub const DELAY: u32 = 0x50;
    pub const POSITION: u32 = 0x52;
}

fn trainee_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u16(ctx.b, trainee::DELAY, 0x120);
    o.set_u32(ctx.b, Obj::VX, 0x0200_0000);
    let rank = (o.u8(ctx.b, trainee::POSITION) as u32) << 2;
    let x = ctx.prog16(trainee::POSITIONS + rank);
    o.set_u16(ctx.b, Obj::X, x);
    let y = ctx.prog16(trainee::POSITIONS + rank + 2);
    o.set_u16(ctx.b, Obj::Y, y);
    o.set_u16(ctx.b, Obj::Z, 0);
    let (cam_x, cam_y) = (crate::ram::CAMERA_X.get(ctx.b), crate::ram::CAMERA_Y.get(ctx.b));
    shifted_u16(ctx, o, Obj::X, cam_x);
    shifted_u16(ctx, o, Obj::Y, cam_y);
    let animation = o.u8(ctx.b, Obj::VARIANT).wrapping_add(0xc);
    ctx.anim_start(trainee::ANIMATIONS, animation as u16);
}

fn trail_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    if !(sub_state != 0) {
        let remainder = o.u16(ctx.b, trainee::DELAY).wrapping_sub(1);
        o.set_u16(ctx.b, trainee::DELAY, remainder);
        if !(remainder != 0) {
            ajoute_u8(ctx, o, field::SUB_STATE, 2);
            o.set_u16(ctx.b, trainee::DELAY, 0x200);
            let variant = o.u8(ctx.b, Obj::VARIANT);
            ctx.anim_start(trainee::ANIMATIONS, variant as u16);
            return;
        }
        if remainder > 0x50 {
            return;
        }
        ctx.anim_step();
        ctx.move_x();
        return ctx.reinsert();
    }
    let remainder = o.u16(ctx.b, trainee::DELAY).wrapping_sub(1);
    o.set_u16(ctx.b, trainee::DELAY, remainder);
    if !(remainder != 0) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
    ctx.anim_step();
    ctx.reinsert();
}

fn trainee<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, trainee::TABLE, trainee_init, trail_active)
}

// ---------------------------------------------------------------------------------------------
// Type 33 : gravats qui retombent (0xb2cca)

/// Type 33 : éclat lancé depuis une position d'écran (variante 0 à 3, vitesses tirées d'une table),
/// dessiné au premier plan. Il retombe ; au contact du sol, les variantes non nulles disparaissent, la
/// variante 0 (hors pause) crée deux tirs ennemis (ESHL 0x17, modes 0 et 1) avant de disparaître.
mod eclat {
    pub const ENTRY: u32 = 0xb2cca;
    pub const TABLE: u32 = 0xb2cd8;
    pub const POSITIONS: u32 = 0xb2d72;
    pub const SPEEDS: u32 = 0xb2d52;
    pub const ANIMATIONS: u32 = 0x193ffe;
    pub const OFFSET: u32 = 0x5c;
    pub const CREATE_SHOT_A: u32 = 0xeab50;
}

fn eclat_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let rng = ctx.random();
    let draw = rng & 0x3f;
    o.set_u16(ctx.b, eclat::OFFSET, draw);
    let flip = o.u8(ctx.b, field::FLIP) ^ 0x20;
    o.set_u8(ctx.b, field::FLIP, flip);
    let rank = (o.u8(ctx.b, Obj::VARIANT) as u32) << 2;
    let dx = ctx.prog16(eclat::POSITIONS + rank);
    let dz = ctx.prog16(eclat::POSITIONS + rank + 2);
    o.set_u16(ctx.b, Obj::Y, 0x40);
    let (x, z) = (crate::ram::CAMERA_ALT_X.get(ctx.b), crate::ram::CAMERA_ALT_Y.get(ctx.b));
    o.set_u16(ctx.b, Obj::X, x);
    o.set_u16(ctx.b, Obj::Z, z);
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Z, dz);
    let rank = rank << 1;
    for (i, off) in [Obj::VX, Obj::VZ, Obj::AZ].into_iter().enumerate() {
        let v = ctx.prog16(eclat::SPEEDS + rank + 2 * i as u32);
        o.set_u16(ctx.b, off, v);
    }
    let az = o.u16(ctx.b, Obj::AZ).wrapping_neg();
    o.set_u16(ctx.b, Obj::AZ, az);
    ctx.anim_start(eclat::ANIMATIONS, 0);
    ctx.reinsert_foreground();
}

/// Atterrissage (`0xb2d92`) : disparition, ou deux tirs pour la variante 0.
fn eclat_atterrit<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if !(variant != 0) {
        let pause = ctx.ram8(0x42) != 0;
        if !pause {
            crate::sound::play(ctx.b, eclat::CREATE_SHOT_A, ctx.obj());
            let tir = ctx.alloc(&pools::ESHL);
            if tir.is_none() {
                return;
            }
            let tir = tir.unwrap();
            tir.set_u8(ctx.b, Obj::ACTIVE, 1);
            tir.set_u8(ctx.b, Obj::KIND, 0x17);
            tir.set_u8(ctx.b, field::MODE, 0);
            super::effects_creators::set_list(ctx.b, 3);
            let tir = ctx.alloc(&pools::ESHL);
            if tir.is_none() {
                return;
            }
            let tir = tir.unwrap();
            tir.set_u8(ctx.b, Obj::ACTIVE, 1);
            tir.set_u8(ctx.b, Obj::KIND, 0x17);
            tir.set_u8(ctx.b, field::MODE, 1);
        }
    }
    ctx.free(&pools::EFFCT);
}

fn shard_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ctx.anim_step();
    let in_flight = ctx.move_xz().pl(); // bpl
    if in_flight {
        return;
    }
    eclat_atterrit(ctx, o)
}

fn eclat<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, eclat::TABLE, state as u32);
    match state {
        state::INIT => eclat_init(ctx, o),
        state::ACTIVE => shard_active(ctx, o),
        state::LIBERE => eclat_atterrit(ctx, o),
        state::DETRUIT => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 25 : poussière projetée (0xb111a)

/// Type 25 : éclats projetés à la naissance (vitesse `+0x14` / `+0x1c`, dispersés au hasard selon le
/// mode `+0x2f`, orientés selon `+0x2c`), puis qui dérivent horizontalement en clignotant jusqu'à la
/// fin de leur animation.
mod poussiere {
    pub const ENTRY: u32 = 0xb111a;
    pub const TABLE: u32 = 0xb1128;
    pub const ANIMATIONS: u32 = 0x1901ba;
    /// Sens de la dérive (`+0x2d`) et sur quel axe se fait le saut (`+0x43`).
    pub const SENS: u32 = 0x2d;
    pub const ON_Z: u32 = 0x43;
}

fn poussiere_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let mut dx = o.u16(ctx.b, Obj::VX);
    let mut dy = o.u16(ctx.b, Obj::VY);
    let retourne = o.u8(ctx.b, field::FLIP) != 0;
    if retourne {
        dx = dx.wrapping_neg();
    }
    let mode = o.u8(ctx.b, field::MODE);
    if !(mode == 0) {
        let d3 = (mode as u16) << 3;
        dx = dx.wrapping_sub(d3);
        dy = dy.wrapping_sub(d3);
        let mask = (d3 << 1).wrapping_sub(1);
        let rng = ctx.random();
        dx = dx.wrapping_add(rng & mask);
        let rng = ctx.random();
        dy = dy.wrapping_add(rng & mask);
    }
    shifted_u16(ctx, o, Obj::X, dx);
    let on_z = o.u8(ctx.b, poussiere::ON_Z);
    if !(on_z == 0) {
        shifted_u16(ctx, o, Obj::Z, dy);
    } else {
        shifted_u16(ctx, o, Obj::Y, dy);
    }
    o.set_u16(ctx.b, Obj::VX, 0);
    let sens = o.u8(ctx.b, poussiere::SENS);
    if !(sens == 0) {
        let mut v: u16 = 0x180;
        let retourne = o.u8(ctx.b, field::FLIP) != 0;
        if retourne {
            v = v.wrapping_neg();
        }
        let sens = o.u8(ctx.b, poussiere::SENS);
        if !(sens == 1) {
            v = v.wrapping_neg();
        }
        o.set_u16(ctx.b, Obj::VX, v);
    }
    let variant = o.u8(ctx.b, Obj::VARIANT);
    ctx.anim_start(poussiere::ANIMATIONS, variant as u16);
    ctx.reinsert();
}

fn dust_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let vx = o.u16(ctx.b, Obj::VX) as i16 as i32;
    let x = o.u32(ctx.b, Obj::X).wrapping_add((vx << 8) as u32);
    o.set_u32(ctx.b, Obj::X, x);
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags ^ 2);
    let end = ctx.anim_step();
    if end {
        return ctx.free(&pools::EFFCT);
    }
    ctx.reinsert();
}

fn poussiere<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, poussiere::TABLE, poussiere_init, dust_active)
}

// ---------------------------------------------------------------------------------------------
// Type 26 : nuage qui traverse l'écran (0xb11f4)

/// Type 26 : nuage qui naît à droite de l'écran (hauteur aléatoire), attend un délai aléatoire (`+0x5c`,
/// 0 à 63 images) puis traverse l'écran vers la gauche (vitesse selon la variante) jusqu'à 0x40 pixels
/// au-delà du bord.
mod nuage {
    pub const ENTRY: u32 = 0xb11f4;
    pub const TABLE: u32 = 0xb1202;
    pub const SPEEDS: u32 = 0xb126a;
    pub const ANIMATIONS_NUMBER: u32 = 0xb1264;
    pub const ANIMATIONS: u32 = 0x1bf8fe;
    pub const DELAY: u32 = 0x5c;
}

fn nuage_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let rng = ctx.random();
    let draw = rng & 0x3f;
    o.set_u16(ctx.b, nuage::DELAY, draw);
    let rng = ctx.random();
    let draw = rng & 0x3f;
    o.set_u16(ctx.b, Obj::Y, draw);
    shifted_u16(ctx, o, Obj::Y, 0x50);
    o.set_u16(ctx.b, Obj::Z, 0);
    let cam = crate::ram::CAMERA_ALT_X.get(ctx.b);
    o.set_u16(ctx.b, Obj::X, cam.wrapping_add(0x1c0));
    let variant = o.u8(ctx.b, Obj::VARIANT) as u32;
    let speed = ctx.prog16(nuage::SPEEDS + 2 * variant);
    o.set_u16(ctx.b, Obj::VX, speed);
    let animation = ctx.prog8(nuage::ANIMATIONS_NUMBER + variant);
    ctx.anim_start(nuage::ANIMATIONS, animation as u16);
}

fn cloud_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    if !(sub_state != 0) {
        let remainder = o.u16(ctx.b, nuage::DELAY).wrapping_sub(1);
        o.set_u16(ctx.b, nuage::DELAY, remainder);
        if !(remainder as i16 >= 0) {
            ajoute_u8(ctx, o, field::SUB_STATE, 2);
        }
        return;
    }
    ctx.anim_step();
    // l'original relit D0 après la remise en liste, qui n'en change que le mot bas : le mot haut est celui de
    // l'aiguillage des états (`moveq #0,D0`), nul ; `swap D0 ; asr.l #8,D0` donne donc la vitesse × 256
    ctx.reinsert();
    let o = ctx.obj();
    let vx = o.u16(ctx.b, Obj::VX) as u32;
    let d0 = (vx << 16) as i32 >> 8;
    let x = o.u32(ctx.b, Obj::X).wrapping_sub(d0 as u32);
    o.set_u32(ctx.b, Obj::X, x);
    let limite = crate::ram::CAMERA_ALT_X.get(ctx.b).wrapping_sub(0x40);
    let xw = o.u16(ctx.b, Obj::X);
    if !((limite as i16) < xw as i16) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
}

fn nuage<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, nuage::TABLE, nuage_init, cloud_active)
}

// ---------------------------------------------------------------------------------------------
// Type 17 : éléments d'interface des joueurs (0xae78c)

/// Type 17 : éléments de l'interface de partie, cinq variantes (0 à 4), posés par rapport à la
/// caméra. Tous se libèrent quand la phase de jeu `A5+0x2c` atteint 8 (fin de partie).
///
/// - variante 0 : attend que `+0x1c` soit non nul puis remplit 8 longs de la palette `0x91c0e0` ;
/// - variante 1 : curseur du joueur `+0x2f` (pointeur du joueur en `+0x1a`), animation selon le
///   personnage ;
/// - variante 2 : fenêtre de joueur (jauge) : recopie dans la palette, selon l'état de l'objet
///   parent (`+0x1e`), les couleurs de la fenêtre ; clignote quand la jauge change ;
/// - variante 3 : indicateur placé en bas à droite, animation `0xe3 - +0x14` ;
/// - variante 4 : élément fixe décalé selon le joueur.
mod interface {
    pub const ENTRY: u32 = 0xae78c;
    pub const TABLE: u32 = 0xae79c;
    pub const STATES_0: u32 = 0xae7bc;
    pub const STATES_1: u32 = 0xae86a;
    pub const STATES_2: u32 = 0xae96c;
    pub const STATES_3: u32 = 0xaebda;
    pub const STATES_4: u32 = 0xaec3c;
    pub const PALETTE: u32 = 0x91c000;
    pub const PALETTE_0: u32 = 0x91c0e0;
    /// Palettes sources (données) et listes d'adresses de palette (par joueur).
    pub const SOURCES: u32 = 0x28ff00;
    pub const LISTS: u32 = 0xaeb62;
    /// Décalages de la variante 1 par personnage.
    pub const OFFSETS_1: u32 = 0xae93c;
    /// Variante 2 : animations et décalages par joueur.
    pub const ANIMATIONS_2: u32 = 0xae9e6;
    pub const FORMES_2: u32 = 0xae9c2;
    pub const OFFSETS_2: u32 = 0xae9ce;
    pub const OFFSETS_4: u32 = 0xaec8e;
    pub const PORTEUR: u32 = 0x1e;
    pub const PLAYER: u32 = 0x1a;
    pub const CHARACTER: u32 = 0x42;
    // sous-routines non réécrites
    pub const CHANGE_CHARACTER: u32 = 0xea344;
    pub const SOUND_GAUGE: u32 = 0xea3b6;
}

/// Phase de jeu : 8 ou plus, la partie est finie.
fn game_finished<R: Bench>(ctx: &mut Cx<'_, R>) -> bool {
    let phase = crate::ram::PHASE.get(ctx.b) as i8;
    phase >= 8
}

/// Place l'objet sur la caméra (`X` = caméra x, `Z` = caméra y, `Y` = 0).
fn place_on_camera<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let (cam_x, cam_y) = (crate::ram::CAMERA_X.get(ctx.b), crate::ram::CAMERA_Y.get(ctx.b));
    o.set_u16(ctx.b, Obj::X, cam_x);
    o.set_u16(ctx.b, Obj::Z, cam_y);
    o.set_u32(ctx.b, Obj::Y, 0);
}

fn interface_0<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    if game_finished(ctx) {
        return ctx.free(&pools::EFFCT);
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, interface::STATES_0, state as u32);
    match state {
        state::INIT => {
            let ready = o.u8(ctx.b, 0x1c);
            if ready == 0 {
                return;
            }
            ajoute_u8(ctx, o, Obj::STATE, 2);
            for i in 0..8 {
                crate::mem::wr32(ctx.b, interface::PALETTE_0 + 4 * i, 0xffff_ffff);
            }
        }
        state::ACTIVE => {}
        _ => outside_table(target),
    }
}

fn interface_1<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, interface::STATES_1, state as u32);
    match state {
        state::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            place_on_camera(ctx, o);
            let number = o.u8(ctx.b, field::MODE) as u32;
            for _ in 0..number {
            }
            let player = Obj(PLAYERS + number * STEP_PLAYER);
            o.set_u16(ctx.b, interface::PLAYER, player.0 as u16);
            let character = player.u8(ctx.b, 3) & 0x7e;
            o.set_u8(ctx.b, interface::CHARACTER, character);
            ctx.anim_start(ANIMATIONS_INTERFACE, 0x8e + number as u16);
        }
        state::ACTIVE => {
            let porteur = pointeur(ctx, o, interface::PORTEUR);
            let porteur_absent = porteur.u8(ctx.b, 0x1c) == 0;
            let player = pointeur(ctx, o, interface::PLAYER);
            if porteur_absent || {
                let absent = player.u8(ctx.b, Obj::ACTIVE) == 0;
                absent
            } {
                if game_finished(ctx) {
                    ctx.free(&pools::EFFCT);
                }
                return;
            }
            place_on_camera(ctx, o);
            let character = player.u8(ctx.b, 3) & 0x7e;
            let mut change = false;
            let connu = o.u8(ctx.b, interface::CHARACTER);
            if !(character == connu) {
                o.set_u8(ctx.b, interface::CHARACTER, character);
                change = true;
            }
            let rank = (character as u32) << 1;
            let dx = ctx.prog16(interface::OFFSETS_1 + rank);
            let dz = ctx.prog16(interface::OFFSETS_1 + 2 + rank);
            shifted_u16(ctx, o, Obj::X, dx);
            shifted_u16(ctx, o, Obj::Z, dz);
            if change {
                crate::sound::play(ctx.b, interface::CHANGE_CHARACTER, ctx.obj());
            }
            let rehausse = o.u8(ctx.b, 0x16) != 0;
            if rehausse {
                o.set_u16(ctx.b, 0x12, 1);
                shifted_u16(ctx, o, Obj::Z, 0x10);
            }
            ctx.anim_step();
            ctx.reinsert();
        }
        state::LIBERE => ctx.free(&pools::EFFCT),
        state::DETRUIT => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

/// `0xaeb08` : recopie dans la palette (OR du bit de priorité 0xF000F000) les deux fois 16 octets de
/// chaque emplacement de la liste du joueur ; `offset` choisit la palette source (0 ou 0x400/0x800).
fn copy_palette<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, offset: u16) {
    let player = (o.u8(ctx.b, field::MODE) as u32) << 1 & 0xff;
    let rank = ctx.prog16(interface::LISTS + player) as i16 as i32 as u32;
    let mut list = interface::LISTS.wrapping_add(rank);
    loop {
        let slot = ctx.data16(list);
        list += 2;
        if (slot as i16) < 0 {
            return; // bmi : fin de liste
        }
        let destination = interface::PALETTE.wrapping_add(slot as i16 as i32 as u32);
        let source = interface::SOURCES.wrapping_add(slot.wrapping_add(offset) as i16 as i32 as u32);
        for k in 0..8 {
            let v = ctx.data32(source + 4 * k) | 0xf000_f000;
            crate::mem::wr32(ctx.b, destination + 4 * k, v);
        }
    }
}

fn interface_2<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, interface::STATES_2, state as u32);
    match state {
        state::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            place_on_camera(ctx, o);
            o.set_u16(ctx.b, Obj::VX, 0);
            o.set_u16(ctx.b, Obj::VY, 0);
            o.set_u8(ctx.b, 0x2a, 0);
            o.set_u8(ctx.b, 0x2b, 7);
            let number = (o.u8(ctx.b, field::MODE) as u32) << 1;
            let shape = ctx.prog16(interface::FORMES_2 + number);
            o.set_u16(ctx.b, 0x12, shape);
            let dx = ctx.prog16(interface::OFFSETS_2 + 2 * number);
            let dz = ctx.prog16(interface::OFFSETS_2 + 2 * number + 2);
            shifted_u16(ctx, o, Obj::X, dx);
            shifted_u16(ctx, o, Obj::Z, dz);
            interface_2_animation(ctx, o, 0);
        }
        state::ACTIVE => {
            if game_finished(ctx) {
                return ctx.free(&pools::EFFCT);
            }
            let porteur = pointeur(ctx, o, interface::PORTEUR);
            let absent = porteur.u8(ctx.b, 0x1c) == 0;
            if absent {
                return ctx.reinsert();
            }
            let sub_state = o.u8(ctx.b, field::SUB_STATE);
            if sub_state != 0 {
                return ctx.reinsert();
            }
            let number = o.u8(ctx.b, field::MODE) as u32;
            let word = porteur.u16(ctx.b, 8 + 2 * number);
            let mut d1: u8 = 0;
            if !(word == 0xffff) {
                d1 = 1;
            }
            let connu = o.u8(ctx.b, 0x14);
            if !(d1 == connu) {
                o.set_u8(ctx.b, 0x14, d1);
                // 0xae9f2
                let offset = if d1 == 0 { 0 } else { 0x400 };
                copy_palette(ctx, o, offset);
                // la routine enchaîne sur l'animation (0xaea0c), qui trie l'objet puis revient ici
                interface_2_animation(ctx, o, d1 as u16);
            }
            let bits = 3u16 << ((number << 1) & 0x1f);
            let value = porteur.u16(ctx.b, 0x14) & bits;
            let old = o.u16(ctx.b, 0x1c);
            if !(value == old) {
                o.set_u16(ctx.b, 0x1c, value);
                let clignote = o.u8(ctx.b, 0x15) != 0;
                if !clignote {
                    o.set_u8(ctx.b, 0x15, 0xc);
                    crate::sound::play(ctx.b, interface::SOUND_GAUGE, ctx.obj());
                }
            }
            let remainder = o.u8(ctx.b, 0x15);
            if remainder == 0 {
                return ctx.reinsert();
            }
            let drapeaux = o.u8(ctx.b, 0x2a);
            o.set_u8(ctx.b, 0x2a, drapeaux & !1);
            let remainder = remainder.wrapping_sub(1);
            o.set_u8(ctx.b, 0x15, remainder);
            if !(remainder == 0) {
                if remainder & 2 == 0 {
                    return ctx.reinsert();
                }
                let drapeaux = o.u8(ctx.b, 0x2a);
                o.set_u8(ctx.b, 0x2a, drapeaux | 1);
                return ctx.reinsert();
            }
            let value = o.u16(ctx.b, 0x1c);
            let bit = ((number as u8) << 1) as u32;
            if value >> bit & 1 == 0 {
                return ctx.reinsert();
            }
            if value >> (bit + 1) & 1 == 0 {
                return ctx.reinsert();
            }
            ajoute_u8(ctx, o, field::SUB_STATE, 2);
            copy_palette(ctx, o, 0x800);
            ctx.reinsert();
        }
        state::LIBERE => ctx.free(&pools::EFFCT),
        state::DETRUIT => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

/// Fin de l'initialisation de la variante 2 (`0xaea0c`) : animation de la fenêtre.
fn interface_2_animation<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, d1: u16) {
    let number = (o.u8(ctx.b, field::MODE) as u32) << 1;
    let animation = ctx.prog16(interface::ANIMATIONS_2 + number).wrapping_add(d1);
    ctx.anim_start(ANIMATIONS_INTERFACE, animation);
    ctx.reinsert();
}

fn interface_3<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    if game_finished(ctx) {
        return ctx.free(&pools::EFFCT);
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, interface::STATES_3, state as u32);
    match state {
        state::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            place_on_camera(ctx, o);
            shifted_u16(ctx, o, Obj::X, 0x10);
            shifted_u16(ctx, o, Obj::Z, 0xd8);
            interface_3_animation(ctx, o);
        }
        state::ACTIVE => {
            let (a, b) = (o.u8(ctx.b, 0x14), o.u8(ctx.b, 0x15));
            if !(a != b) {
                return ctx.reinsert();
            }
            interface_3_animation(ctx, o);
        }
        _ => outside_table(target),
    }
}

/// `0xaebfe` : animation `0xe3 - +0x14` du jeu d'interface.
fn interface_3_animation<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let value = o.u8(ctx.b, 0x14);
    o.set_u8(ctx.b, 0x15, value);
    ctx.anim_start(ANIMATIONS_INTERFACE, 0xe3u8.wrapping_sub(value) as u16);
    ctx.reinsert();
}

fn interface_4<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, interface::STATES_4, state as u32);
    match state {
        state::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            place_on_camera(ctx, o);
            let rank = (o.u8(ctx.b, field::MODE) as u32) << 3;
            let dx = ctx.prog16(interface::OFFSETS_4 + rank);
            shifted_u16(ctx, o, Obj::X, dx);
            let dz = ctx.prog16(interface::OFFSETS_4 + rank + 2);
            shifted_u16(ctx, o, Obj::Z, dz);
            let dy = ctx.prog16(interface::OFFSETS_4 + rank + 4);
            shifted_u16(ctx, o, 0x12, dy);
            let animation = ctx.prog16(interface::OFFSETS_4 + rank + 6);
            ctx.anim_start(ANIMATIONS_INTERFACE, animation);
            ctx.reinsert();
        }
        state::ACTIVE => {
            if game_finished(ctx) {
                return ctx.free(&pools::EFFCT);
            }
            ctx.reinsert();
        }
        state::LIBERE | state::DETRUIT => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

fn interface_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let target = target(ctx, interface::TABLE, 2 * variant as u32);
    match variant {
        0 => interface_0(ctx, o),
        1 => interface_1(ctx, o),
        2 => interface_2(ctx, o),
        3 => interface_3(ctx, o),
        4 => interface_4(ctx, o),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 29 : étincelle d'impact sur une cible (0xb1440)

/// Type 29 : étincelle collée à une cible (`+0x5c`, ennemi ou boss) à l'endroit d'impact tiré d'une
/// table selon le type de la cible ; elle joue une animation (tirée au hasard selon un drapeau `+0x42`
/// lu dans une table par type de cible) et disparaît à la fin de l'animation. Si la cible n'existe plus
/// ou ne convient pas, l'étincelle passe directement à l'état 6 (libération).
mod impact {
    pub const ENTRY: u32 = 0xb1440;
    pub const TABLE: u32 = 0xb144e;
    pub const ANIMATIONS: u32 = 0x1908d6;
    pub const HASARD_A: u32 = 0xb14a2;
    pub const HASARD_B: u32 = 0xb14b2;
    /// Drapeaux par type d'ennemi / de boss (deux octets : interdit, drapeau).
    pub const ENEMIES: u32 = 0xb1544;
    pub const BOSS: u32 = 0xb1586;
    /// Décalages d'impact (dx, dz) par type de cible (4 octets), pour les ennemis puis pour le reste.
    pub const OFFSETS_ENEMY: u32 = 0xb165a;
    pub const OFFSETS_OTHER: u32 = 0xb16de;
    pub const TARGET: u32 = 0x5c;
    pub const VARIANT_RANDOM: u32 = 0x42;
}

/// Pose l'état 6 (libération à l'image suivante).
fn impact_abandon<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    o.set_u32(ctx.b, Obj::STATE, 0x0600_0000);
}

/// `0xb14dc` : lit dans les tables le drapeau de la cible, ou abandonne.
fn impact_drapeau<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let pointer_target = o.u16(ctx.b, impact::TARGET);
    if pointer_target == 0 {
        return impact_abandon(ctx, o);
    }
    let target = pointeur(ctx, o, impact::TARGET);
    let word = target.u16(ctx.b, 4);
    if word == 0x402 {
        return impact_abandon(ctx, o);
    }
    let class = target.u8(ctx.b, Obj::CLASS);
    let rank = (target.u8(ctx.b, Obj::KIND) as u32) * 2;
    if !(class != 2) {
        let interdit = ctx.prog8(impact::ENEMIES + rank);
        if interdit != 0 {
            return impact_abandon(ctx, o);
        }
        let drapeau = ctx.prog8(impact::ENEMIES + rank + 1);
        return o.set_u8(ctx.b, impact::VARIANT_RANDOM, drapeau);
    }
    if class != 4 {
        return impact_abandon(ctx, o);
    }
    let interdit = ctx.prog8(impact::BOSS + rank);
    if interdit != 0 {
        return impact_abandon(ctx, o);
    }
    let drapeau = ctx.prog8(impact::BOSS + rank + 1);
    o.set_u8(ctx.b, impact::VARIANT_RANDOM, drapeau);
}

/// `0xb15c8` (variante non nulle) : position de la cible décalée de `(+0x14, +0x16)`.
fn impact_position_decalee<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let pointer_target = o.u16(ctx.b, impact::TARGET);
    if pointer_target == 0 {
        return;
    }
    let target = pointeur(ctx, o, impact::TARGET);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = target.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    let flip = target.u8(ctx.b, field::FLIP);
    o.set_u8(ctx.b, field::FLIP, flip);
    let (dx, dz) = (o.u16(ctx.b, Obj::VX), o.u16(ctx.b, 0x16));
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Z, dz);
}

/// `0xb15fe` (variante 0) : position de la cible décalée de l'impact tiré de la table.
fn impact_position_table<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let pointer_target = o.u16(ctx.b, impact::TARGET);
    if pointer_target == 0 {
        return;
    }
    let target = pointeur(ctx, o, impact::TARGET);
    for off in [Obj::X, Obj::Y, Obj::Z] {
        let v = target.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    let flip = target.u8(ctx.b, field::FLIP);
    o.set_u8(ctx.b, field::FLIP, flip);
    let class = target.u8(ctx.b, Obj::CLASS);
    let table = if class == 2 { impact::OFFSETS_ENEMY } else { impact::OFFSETS_OTHER };
    let rank = ((target.u8(ctx.b, Obj::KIND) << 2) as u32) & 0xff;
    let mut dx = ctx.data16(table + rank);
    let dz = ctx.data16(table + rank + 2);
    let retourne = target.u8(ctx.b, field::FLIP) != 0;
    if retourne {
        dx = dx.wrapping_neg();
    }
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Z, dz);
}

fn impact_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if !(variant == 0) {
        impact_position_decalee(ctx, o);
        o.set_u8(ctx.b, impact::VARIANT_RANDOM, 0);
    } else {
        impact_position_table(ctx, o);
    }
    impact_drapeau(ctx, o);
    let rng = ctx.random();
    let draw = rng & 0xf;
    let drapeau = o.u8(ctx.b, impact::VARIANT_RANDOM);
    let table = if drapeau == 0 { impact::HASARD_A } else { impact::HASARD_B };
    let animation = ctx.prog8(table + draw as u32);
    ctx.anim_start(impact::ANIMATIONS, animation as u16);
}

fn impact_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let end = ctx.anim_step();
    if end {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
    ctx.reinsert();
}

fn impact_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, impact::TABLE, state as u32);
    match state {
        state::INIT => impact_init(ctx, o),
        state::ACTIVE => impact_active(ctx, o),
        state::LIBERE | 6 => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 6 : gerbe de débris (0xab7fa)

/// Type 6 : débris projetés depuis un propriétaire (`+0x50`). Le premier objet (`+0x42` nul) crée
/// autant de frères qu'indiqué par une table (selon la variante) puis devient lui-même un débris ;
/// chaque débris reprend la position du propriétaire, lit sa vitesse, son orientation, son décalage et
/// son animation dans une fiche (variante, mode `+0x2f`) de la ROM, monte et retombe en clignotant,
/// puis disparaît.
mod debris {
    pub const ENTRY: u32 = 0xab7fa;
    pub const TABLE: u32 = 0xab808;
    /// Nombre de frères par variante (espace programme) ; fiches de la ROM (données).
    pub const NOMBRES: u32 = 0xab95e;
    pub const SHEETS: u32 = 0xab8a2;
    pub const ANIMATIONS: u32 = 0x18e664;
    pub const ENFANT: u32 = 0x42;
    pub const CLIGNOTEMENT: u32 = 0x53;
}

/// `0xab7d8` : crée un débris (type 6, variante `variant`) rattaché à l'objet courant.
fn created_debris<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, variant: u8) -> Option<Obj> {
    let new = ctx.alloc(&pools::EFFCT);
    // deux `bcs` successifs : dans `0xab7d8`, puis chez l'appelant (la retenue est restée levée)
    let n = new?;
    n.set_u8(ctx.b, Obj::ACTIVE, 1);
    n.set_u8(ctx.b, Obj::KIND, 6);
    n.set_u8(ctx.b, Obj::VARIANT, variant);
    n.set_u8(ctx.b, debris::ENFANT, 0);
    n.set_u16(ctx.b, field::OWNER, o.0 as u16);
    Some(n)
}

/// Premier débris : crée ses frères, puis passe à l'initialisation commune.
fn debris_parent<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    o.set_u8(ctx.b, debris::ENFANT, 1);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let count = ctx.prog8(debris::NOMBRES + variant as u32);
    o.set_u8(ctx.b, field::MODE, count.wrapping_add(1));
    let owner = o.u16(ctx.b, field::OWNER);
    for remaining in (0..=count as u16).rev() {
        let Some(sibling) = created_debris(ctx, o, variant) else {
            break;
        };
        sibling.set_u16(ctx.b, field::OWNER, owner);
        sibling.set_u8(ctx.b, debris::ENFANT, 1);
        sibling.set_u8(ctx.b, field::MODE, remaining as u8);
    }
    debris_init(ctx, o);
}

fn debris_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let enfant = o.u8(ctx.b, debris::ENFANT);
    if enfant == 0 {
        return debris_parent(ctx, o);
    }
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let owner = pointeur(ctx, o, field::OWNER);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = owner.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    o.set_u8(ctx.b, debris::CLIGNOTEMENT, 0);
    o.set_u16(ctx.b, 0x16, 0);
    let variant = o.u8(ctx.b, Obj::VARIANT) as u32;
    let offset = ctx.data16(debris::SHEETS + 2 * variant) as i16 as i32 as u32;
    let mode = (o.u8(ctx.b, field::MODE) as u32) << 4;
    let mut sheet = debris::SHEETS.wrapping_add(offset).wrapping_add(mode);
    let vx = ctx.data16(sheet);
    o.set_u16(ctx.b, Obj::VX, vx);
    let vz_az = ctx.data32(sheet + 2);
    o.set_u32(ctx.b, Obj::VZ, vz_az);
    let flip = ctx.data8(sheet + 6);
    o.set_u8(ctx.b, field::FLIP, flip);
    let animation = ctx.data8(sheet + 7);
    let (mut dx, dz, dy) = (ctx.data16(sheet + 8), ctx.data16(sheet + 10), ctx.data16(sheet + 12));
    sheet += 14;
    let _ = sheet;
    let retourne = owner.u8(ctx.b, field::FLIP) != 0;
    if retourne {
        let flip = o.u8(ctx.b, field::FLIP) ^ 0x20;
        o.set_u8(ctx.b, field::FLIP, flip);
        let vx = o.u16(ctx.b, Obj::VX).wrapping_neg();
        o.set_u16(ctx.b, Obj::VX, vx);
        dx = dx.wrapping_neg();
        o.set_u8(ctx.b, debris::CLIGNOTEMENT, 1);
    }
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Z, dz);
    shifted_u16(ctx, o, 0x12, dy);
    ctx.anim_start(debris::ANIMATIONS, animation as u16);
}

fn debris_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ctx.move_xz();
    ctx.anim_step();
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
    let clignote = o.u8(ctx.b, debris::CLIGNOTEMENT) ^ 1;
    o.set_u8(ctx.b, debris::CLIGNOTEMENT, clignote);
    if !(clignote == 0) {
        let flags = o.u8(ctx.b, Obj::FLAGS);
        o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    }
    let z = o.u16(ctx.b, Obj::Z);
    if !(z as i16 > 0) {
        o.set_u32(ctx.b, Obj::Z, 0);
        o.set_u8(ctx.b, debris::CLIGNOTEMENT, 6);
        ajoute_u8(ctx, o, Obj::STATE, 2);
        let variant = o.u8(ctx.b, Obj::VARIANT);
        if !(variant == 0) {
            ajoute_u8(ctx, o, debris::CLIGNOTEMENT, 0xff);
        }
    }
    ctx.reinsert();
}

fn debris_end<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
    let remainder = o.u8(ctx.b, debris::CLIGNOTEMENT).wrapping_sub(1);
    o.set_u8(ctx.b, debris::CLIGNOTEMENT, remainder);
    if remainder == 0 {
        return ctx.free(&pools::EFFCT);
    }
    if !(remainder & 1 == 0) {
        let flags = o.u8(ctx.b, Obj::FLAGS);
        o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    }
    ctx.reinsert();
}

fn debris_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, debris::TABLE, state as u32);
    match state {
        state::INIT => debris_init(ctx, o),
        state::ACTIVE => debris_active(ctx, o),
        state::LIBERE => debris_end(ctx, o),
        6 => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 1 : effet supprimé (0xa8758)

/// Type 1 : emplacement de table inutilisé dans cette version du jeu : le gestionnaire libère
/// l'objet aussitôt (`bra` vers `jmp 0x2218`).
fn supprime<R: Bench>(ctx: &mut Cx<'_, R>) {
    ctx.free(&pools::EFFCT);
}

// ---------------------------------------------------------------------------------------------
// Type 7 : éclat à animation unique (0xaba4e)

/// Type 7 : joue l'animation (tirée selon le mode `+0x2f`) ; à la fin de l'animation il monte de deux
/// unités en profondeur puis clignote 0x1e images avant de disparaître. Non exercé.
mod eclat_unique {
    pub const ENTRY: u32 = 0xaba4e;
    pub const TABLE: u32 = 0xaba5c;
    pub const ANIMATIONS_MODE: u32 = 0xaba84;
    pub const COUNT: u32 = 0x53;
}

fn eclat_unique_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let mode = o.u8(ctx.b, field::MODE) as u32;
    let animation = ctx.prog8(eclat_unique::ANIMATIONS_MODE + mode);
    ctx.anim_start(0x1bf8fe, animation as u16);
    ctx.reinsert();
}

fn shard_single_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let end = o.u8(ctx.b, 0x32);
    if !(end == 0) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, eclat_unique::COUNT, 0x1e);
        let y = o.u32(ctx.b, Obj::Y).wrapping_add(2);
        o.set_u32(ctx.b, Obj::Y, y);
    }
    ctx.anim_step();
}

fn eclat_unique_clignote<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let remainder = o.u8(ctx.b, eclat_unique::COUNT).wrapping_sub(1);
    o.set_u8(ctx.b, eclat_unique::COUNT, remainder);
    if remainder == 0 {
        return ctx.free(&pools::EFFCT);
    }
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags ^ 2);
}

fn shard_single_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, eclat_unique::TABLE, state as u32);
    match state {
        state::INIT => eclat_unique_init(ctx, o),
        state::ACTIVE => shard_single_active(ctx, o),
        state::LIBERE => eclat_unique_clignote(ctx, o),
        state::DETRUIT => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 19 : balayage d'écran (0xaecce)

/// Type 19 : objet qui entre par la gauche de l'écran (x = -0x80, y = 0x150), attend 0x70 images en
/// avançant de deux pixels par image, puis ralentit (accélération -0x10) jusqu'à repartir en arrière.
/// Non exercé.
mod balayage {
    pub const ENTRY: u32 = 0xaecce;
    pub const TABLE: u32 = 0xaecdc;
    pub const DELAY: u32 = 0x54;
}

fn balayage_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, balayage::DELAY, 0x70);
    o.set_u16(ctx.b, Obj::X, 0xff80);
    o.set_u16(ctx.b, Obj::Y, 0x150);
    o.set_u16(ctx.b, Obj::VX, 0x200);
    o.set_u16(ctx.b, Obj::AX, 0xfff0);
    ctx.anim_start(ANIMATIONS_INTERFACE, 0);
}

fn sweep_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    if !(sub_state != 0) {
        let delay = o.u8(ctx.b, balayage::DELAY);
        if delay != 0 {
            o.set_u8(ctx.b, balayage::DELAY, delay - 1);
            shifted_u16(ctx, o, Obj::X, 2);
        } else {
            ctx.move_x();
            let vx = o.u16(ctx.b, Obj::VX);
            if !(vx as i16 >= 0) {
                ajoute_u8(ctx, o, field::SUB_STATE, 1);
            }
        }
    }
    ctx.anim_step();
    ctx.reinsert();
}

fn sweep_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, balayage::TABLE, balayage_init, sweep_active)
}

// ---------------------------------------------------------------------------------------------
// Type 27 : effet de ramassage qui monte (0xb12e6)

/// Type 27 : effet posé sur l'objet propriétaire (`+0x50`, créé par `0xb12c0` : ramassage) qui monte de 0x8000 (un demi
/// pixel) par image pendant 0x1c images. Non exercé.
mod fumee {
    pub const ENTRY: u32 = 0xb12e6;
    pub const TABLE: u32 = 0xb12f4;
    pub const ANIMATIONS: u32 = 0x19046c;
    pub const DURATION: u32 = 0x57;
}

fn fumee_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, fumee::DURATION, 0x1c);
    let owner = pointeur(ctx, o, field::OWNER);
    let x = owner.u16(ctx.b, Obj::X);
    o.set_u16(ctx.b, Obj::X, x);
    let y = owner.u16(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u16(ctx.b, Obj::Y, y);
    o.set_u16(ctx.b, Obj::Z, 0x10);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    ctx.anim_start(fumee::ANIMATIONS, variant as u16);
    ctx.reinsert();
}

fn smoke_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let remainder = o.u8(ctx.b, fumee::DURATION).wrapping_sub(1);
    o.set_u8(ctx.b, fumee::DURATION, remainder);
    if !(remainder != 0) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
    let z = o.u32(ctx.b, Obj::Z).wrapping_add(0x8000);
    o.set_u32(ctx.b, Obj::Z, z);
}

fn smoke_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, fumee::TABLE, fumee_init, smoke_active)
}

// ---------------------------------------------------------------------------------------------
// Type 28 : étincelle posée sur un objet (0xb136e)

/// Type 28 : étincelle collée à un objet (`+0x5c`), 0x20 au-dessus de lui ; l'animation dépend de la
/// variante et de `A5+0x70`. À la fin de l'animation, elle prévient son objet (`+0x77` ← 1, sauf
/// variante 6) puis disparaît. Non exercé.
mod etincelle {
    pub const ENTRY: u32 = 0xb136e;
    pub const TABLE: u32 = 0xb137c;
    pub const ANIMATIONS: u32 = 0x190534;
    pub const PORTEUR: u32 = 0x5c;
}

fn etincelle_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let porteur = pointeur(ctx, o, etincelle::PORTEUR);
    copy_position(ctx, o, porteur);
    let y = o.u16(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u16(ctx.b, Obj::Y, y);
    shifted_u16(ctx, o, Obj::Z, 0x20);
    let variant = o.u8(ctx.b, Obj::VARIANT) as u16;
    let word = ctx.ram16(0x70);
    let animation = variant.wrapping_add((word & 0xf) >> 1 << 3);
    ctx.anim_start(etincelle::ANIMATIONS, animation);
    ctx.reinsert();
}

fn spark_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let end = ctx.anim_step();
    if !end {
        return;
    }
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if variant == 6 {
        return;
    }
    let porteur = pointeur(ctx, o, etincelle::PORTEUR);
    porteur.set_u8(ctx.b, 0x77, 1);
}

fn spark_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, etincelle::TABLE, state as u32);
    match state {
        state::INIT => etincelle_init(ctx, o),
        state::ACTIVE => spark_active(ctx, o),
        state::LIBERE => ajoute_u8(ctx, o, Obj::STATE, 2),
        6 => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 57 : marque attachée à un joueur (0xb6bca)

/// Type 57 : marque posée 0x18 à droite et 0x2d au-dessus du propriétaire (`+0x50`) tant qu'il reste
/// dans l'état `0x0204` / `+6 = 0x2c` ; sinon elle disparaît. Non exercé.
mod mark {
    pub const ENTRY: u32 = 0xb6bca;
    pub const TABLE: u32 = 0xb6bd8;
    pub const ANIMATIONS: u32 = 0x196b62;
}

fn mark_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let owner = pointeur(ctx, o, field::OWNER);
    for off in [Obj::X, Obj::Z] {
        let v = owner.u16(ctx.b, off);
        o.set_u16(ctx.b, off, v);
    }
    let y = owner.u32(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u32(ctx.b, Obj::Y, y);
    shifted_u16(ctx, o, Obj::X, 0x18);
    shifted_u16(ctx, o, Obj::Z, 0x2d);
    ctx.anim_start(mark::ANIMATIONS, 0);
    ctx.reinsert();
}

fn mark_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let owner = pointeur(ctx, o, field::OWNER);
    let state = owner.u16(ctx.b, 4);
    if state != 0x204 {
        return ctx.free(&pools::EFFCT);
    }
    let sub = owner.u8(ctx.b, 6);
    if sub != 0x2c {
        return ctx.free(&pools::EFFCT);
    }
    ctx.anim_step();
}

fn mark_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, mark::TABLE, state as u32);
    match state {
        state::INIT => mark_init(ctx, o),
        state::ACTIVE => mark_active(ctx, o),
        state::LIBERE => ctx.free(&pools::EFFCT),
        state::DETRUIT => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 62 : gerbe d'éclats (0xb77ac)

/// Type 62 : reprend variante et mode de l'objet qu'il suit (`+0x5c`), se place 0x48 à gauche de lui et
/// attend qu'il soit touché (`+0x33` non nul) ; il explose alors en neuf objets EFFCT 0x40 (et un dixième
/// décrit par le mode) puis disparaît. Non exercé.
mod gerbe {
    pub const ENTRY: u32 = 0xb77ac;
    pub const TABLE: u32 = 0xb77ba;
    pub const ANIMATIONS: u32 = 0x197a5c;
    pub const PORTEUR: u32 = 0x5c;
}

fn gerbe_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let porteur = pointeur(ctx, o, gerbe::PORTEUR);
    let variant = porteur.u8(ctx.b, Obj::VARIANT);
    o.set_u8(ctx.b, Obj::VARIANT, variant);
    let mode = porteur.u8(ctx.b, field::MODE);
    o.set_u8(ctx.b, field::MODE, mode);
    for off in [Obj::X, Obj::Y] {
        let v = porteur.u16(ctx.b, off);
        o.set_u16(ctx.b, off, v);
    }
    let x = o.u16(ctx.b, Obj::X).wrapping_sub(0x48);
    o.set_u16(ctx.b, Obj::X, x);
    o.set_u16(ctx.b, Obj::Z, 0x40);
    ctx.anim_start(gerbe::ANIMATIONS, 0);
    o.set_u8(ctx.b, 0x3b, 0);
    ctx.reinsert();
}

/// `0xb7810` : l'explosion.
fn gerbe_explose<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let mut last = None;
    for _ in 0..9 {
        let new = ctx.alloc(&pools::EFFCT);
        if new.is_none() {
            return ctx.free(&pools::EFFCT);
        }
        let n = new.unwrap();
        n.set_u8(ctx.b, Obj::ACTIVE, 1);
        n.set_u8(ctx.b, Obj::KIND, 0x40);
        for off in [Obj::X, Obj::Y] {
            let v = o.u16(ctx.b, off);
            n.set_u16(ctx.b, off, v);
        }
        last = Some(n);
    }
    let mode = o.u8(ctx.b, field::MODE);
    if !(mode == 0) {
        // le dernier objet créé est repris pour porter le mode
        let n = last.unwrap();
        n.set_u8(ctx.b, Obj::ACTIVE, 1);
        n.set_u8(ctx.b, Obj::KIND, 0x40);
        n.set_u8(ctx.b, Obj::VARIANT, 1);
        let variant = o.u8(ctx.b, Obj::VARIANT);
        n.set_u8(ctx.b, field::MODE, variant);
        for off in [Obj::X, Obj::Y] {
            let v = o.u16(ctx.b, off);
            n.set_u16(ctx.b, off, v);
        }
    }
    ctx.free(&pools::EFFCT);
}

fn sheaf_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, gerbe::TABLE, state as u32);
    match state {
        state::INIT => gerbe_init(ctx, o),
        state::ACTIVE => {
            let porteur = pointeur(ctx, o, gerbe::PORTEUR);
            let key = porteur.u8(ctx.b, 0x33);
            if key == 0 {
                return;
            }
            gerbe_explose(ctx, o);
        }
        state::LIBERE => gerbe_explose(ctx, o),
        state::DETRUIT => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 56 : éclats de débris (0xb6c6e)

/// Type 56 : éclat lancé en l'air depuis sa position (dispersion aléatoire de ±128 en X et ±32 en
/// profondeur, vitesses tirées dans deux tables, animation 0x164 à 0x167) ; disparaît quand il
/// retombe sous le sol. Non exercé.
mod eclats_debris {
    pub const ENTRY: u32 = 0xb6c6e;
    pub const TABLE: u32 = 0xb6c7c;
    pub const SPEEDS_X: u32 = 0xb6d18;
    pub const SPEEDS_Y: u32 = 0xb6d28;
}

/// Tirage aléatoire masqué.
fn draw<R: Bench>(ctx: &mut Cx<'_, R>, mask: u16) -> u16 {
    let rng = ctx.random();
    rng & mask
}

fn eclats_debris_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u16(ctx.b, 0x42, 0x40);
    o.set_u8(ctx.b, 0x2a, 1);
    o.set_u8(ctx.b, 0x2b, 0x1d);
    let flip = draw(ctx, 0x20);
    o.set_u8(ctx.b, field::FLIP, flip as u8);
    let dx = draw(ctx, 0xff).wrapping_sub(0x80);
    shifted_u16(ctx, o, Obj::X, dx);
    let dy = draw(ctx, 0x3f).wrapping_sub(0x20);
    shifted_u16(ctx, o, Obj::Y, dy);
    o.set_u16(ctx.b, Obj::VZ, 0x400);
    o.set_u16(ctx.b, Obj::AZ, 0xffc0);
    let r = draw(ctx, 0x1c) as u32;
    let vx = ctx.prog16(eclats_debris::SPEEDS_X + r);
    o.set_u16(ctx.b, Obj::VX, vx);
    let r = draw(ctx, 0x1c) as u32;
    let vy = ctx.prog16(eclats_debris::SPEEDS_Y + r);
    o.set_u16(ctx.b, Obj::VY, vy);
    let animation = draw(ctx, 3).wrapping_add(0x164);
    ctx.anim_start(ANIMATIONS_INTERFACE, animation);
    ctx.reinsert();
}

fn shards_debris_active<R: Bench>(ctx: &mut Cx<'_, R>, _o: Obj) {
    let positif = ctx.move_xyz().pl(); // bpl
    if positif {
        return;
    }
    ctx.free(&pools::EFFCT);
}

fn shards_debris_end<R: Bench>(ctx: &mut Cx<'_, R>, _o: Obj) {
    ctx.free(&pools::EFFCT);
}

fn shards_debris_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, eclats_debris::TABLE, state as u32);
    match state {
        state::INIT => eclats_debris_init(ctx, o),
        state::ACTIVE => shards_debris_active(ctx, o),
        state::LIBERE => shards_debris_end(ctx, o),
        state::DETRUIT => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 51 : projectile lancé d'un objet (0xb64c8)

/// Type 51 : naît sur l'objet `+0x5c` (si cet objet disparaît, il passe à l'état 6), décalé de 0x84
/// pixels vers l'avant et 0x16 en profondeur, puis part en arc (vitesses 0x140 et 0x3c0, gravité 0x38)
/// jusqu'à toucher le sol. Non exercé.
mod projectile {
    pub const ENTRY: u32 = 0xb64c8;
    pub const TABLE: u32 = 0xb64de;
    pub const SUB_STATES: u32 = 0xb6572;
    pub const ANIMATIONS: u32 = 0x1733ac;
    pub const TARGET: u32 = 0x5c;
}

fn projectile_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let pointer_target = o.u16(ctx.b, projectile::TARGET);
    if pointer_target == 0 {
        return;
    }
    let target = pointeur(ctx, o, projectile::TARGET);
    for off in [Obj::X, Obj::Y, Obj::Z] {
        let v = target.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    let flip = target.u8(ctx.b, field::FLIP);
    o.set_u8(ctx.b, field::FLIP, flip);
    shifted_u16(ctx, o, Obj::Y, 1);
    let (mut dx, dy) = (0x84u16, 0xffeau16);
    o.set_u16(ctx.b, Obj::VX, 0x140);
    o.set_u16(ctx.b, Obj::VZ, 0x3c0);
    o.set_u16(ctx.b, Obj::AZ, 0xffc8);
    if !(flip == 0) {
        dx = dx.wrapping_neg();
        let v = o.u16(ctx.b, Obj::VX).wrapping_neg();
        o.set_u16(ctx.b, Obj::VX, v);
    }
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Y, dy);
    let orientation = o.u8(ctx.b, field::FLIP) ^ 0x20;
    o.set_u8(ctx.b, field::FLIP, orientation);
    ctx.anim_start(projectile::ANIMATIONS, 0x19);
    ctx.reinsert();
}

fn projectile_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = target(ctx, projectile::SUB_STATES, sub_state as u32);
    match sub_state {
        0 => {
            let in_flight = ctx.move_xz().gt();
            if !in_flight {
                ajoute_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u16(ctx.b, Obj::Z, 0);
            }
            ctx.anim_step();
        }
        2 => {}
        _ => outside_table(target),
    }
}

fn projectile_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let pointer_target = o.u16(ctx.b, projectile::TARGET);
    if pointer_target == 0 {
        o.set_u32(ctx.b, Obj::STATE, 0x0600_0000);
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, projectile::TABLE, state as u32);
    match state {
        state::INIT => projectile_init(ctx, o),
        state::ACTIVE => projectile_active(ctx, o),
        state::LIBERE | 6 => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 52 : scintillement d'un personnage (0xb65e0)

/// Type 52 : variante du scintillement (type 15) qui guette seulement les joueurs de la classe 5 encore
/// en vie (points de vie `+0x62` non nuls) ; une fois déclenché il clignote huit fois, caché tant que
/// son objet (`+0x5c`) l'est. Non exercé.
mod shimmer_class {
    pub const ENTRY: u32 = 0xb65e0;
    pub const TABLE: u32 = 0xb65ee;
    pub const ANIMATIONS: u32 = 0x1bc77c;
    pub const REPETITIONS: u32 = 0x57;
    pub const DELAY: u32 = 0x58;
    pub const BASCULE: u32 = 0x5a;
    pub const DECLENCHE: u32 = 0x5b;
    pub const PORTEUR: u32 = 0x5c;
}

fn shimmer_class_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    o.set_u8(ctx.b, shimmer_class::REPETITIONS, 8);
    let t = draw(ctx, 0x1f);
    o.set_u16(ctx.b, shimmer_class::DELAY, t);
    ctx.anim_start(shimmer_class::ANIMATIONS, 0);
    ctx.reinsert();
}

/// `0xb668a` : un joueur de la classe 5 en vie est-il passé près de l'objet ?
fn shimmer_class_player<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    for i in 0..4u32 {
        let player = Obj(PLAYERS + i * STEP_PLAYER);
        let absent = player.u8(ctx.b, Obj::ACTIVE) == 0;
        if !absent {
            let dead = player.u16(ctx.b, 0x62) == 0;
            if !dead {
                let class = player.u8(ctx.b, 3) >> 1;
                if !(class != 5) {
                    let dy = o.u16(ctx.b, Obj::Y).wrapping_sub(player.u16(ctx.b, Obj::Y)).wrapping_add(0x20);
                    if !(dy > 0x40) {
                        let dx = o.u16(ctx.b, Obj::X).wrapping_sub(player.u16(ctx.b, Obj::X)).wrapping_add(0x60);
                        if !(dx > 0xc0) {
                            o.set_u8(ctx.b, shimmer_class::DECLENCHE, 1);
                            return;
                        }
                    }
                }
            }
        }
    }
}

fn shimmer_class_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let porteur = pointeur(ctx, o, shimmer_class::PORTEUR);
    let state_carrier = porteur.u16(ctx.b, 4);
    if !(state_carrier == 0x200) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
    let declenche = o.u8(ctx.b, shimmer_class::DECLENCHE);
    if !(declenche != 0) {
        return shimmer_class_player(ctx, o);
    }
    let cache = porteur.u8(ctx.b, Obj::HIDDEN);
    if !(cache == 0) {
        let flags = o.u8(ctx.b, Obj::FLAGS);
        o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
        return;
    }
    let remainder = o.u16(ctx.b, shimmer_class::DELAY).wrapping_sub(1);
    o.set_u16(ctx.b, shimmer_class::DELAY, remainder);
    if remainder != 0 {
        return;
    }
    o.set_u16(ctx.b, shimmer_class::DELAY, 0x30);
    let bascule = o.u8(ctx.b, shimmer_class::BASCULE) ^ 1;
    o.set_u8(ctx.b, shimmer_class::BASCULE, bascule);
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
    if bascule != 0 {
        return;
    }
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    let remainder = o.u8(ctx.b, shimmer_class::REPETITIONS).wrapping_sub(1);
    o.set_u8(ctx.b, shimmer_class::REPETITIONS, remainder);
    if remainder != 0 {
        return;
    }
    o.set_u32(ctx.b, Obj::STATE, 0x0400_0000);
}

fn shimmer_class_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, shimmer_class::TABLE, shimmer_class_init, shimmer_class_active)
}

// ---------------------------------------------------------------------------------------------
// Type 24 : accessoire de personnage (0xb0ff2)

/// Type 24 : accessoire collé à son propriétaire (`+0x50`) à un décalage (dx, dy, dz) lu selon la
/// variante ; actif seulement quand la scène n'est pas figée (`A5+0x3d`). Si le propriétaire a disparu
/// il passe à l'état 6. Non exercé.
mod accessoire {
    pub const ENTRY: u32 = 0xb0ff2;
    pub const TABLE: u32 = 0xb1000;
    pub const OFFSETS: u32 = 0xb10c0;
    pub const ANIMATIONS: u32 = 0x16e752;
    pub const END_ANIMATION: u32 = 0x43;
}

/// `0xb1052` : recolle l'objet au propriétaire.
fn accessoire_suit<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let pointer_owner = o.u16(ctx.b, field::OWNER);
    if !(pointer_owner != 0) {
        o.set_u32(ctx.b, Obj::STATE, 0x0600_0000);
        return;
    }
    let owner = pointeur(ctx, o, field::OWNER);
    for off in [Obj::X, Obj::Y, Obj::Z] {
        let v = owner.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    let flip = owner.u8(ctx.b, field::FLIP);
    o.set_u8(ctx.b, field::FLIP, flip);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let rank = (variant << 3) as u32; // décalage sur un octet, comme l'original
    let mut dx = ctx.prog16(accessoire::OFFSETS + rank);
    let dy = ctx.prog16(accessoire::OFFSETS + rank + 2);
    let dprofondeur = ctx.prog16(accessoire::OFFSETS + rank + 4) as i16 as i32;
    let dz = ctx.prog16(accessoire::OFFSETS + rank + 6);
    if !(flip == 0) {
        dx = dx.wrapping_neg();
    }
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Y, dy);
    let y = o.u32(ctx.b, Obj::Y).wrapping_add(dprofondeur as u32);
    o.set_u32(ctx.b, Obj::Y, y);
    shifted_u16(ctx, o, Obj::Z, dz);
}

fn accessory_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, accessoire::TABLE, state as u32);
    match state {
        state::INIT => {
            let figee = ctx.ram8(0x3d) != 0;
            if figee {
                return;
            }
            ajoute_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, accessoire::END_ANIMATION, 0);
            accessoire_suit(ctx, o);
            ctx.anim_start(accessoire::ANIMATIONS, 3);
        }
        state::ACTIVE => {
            let figee = ctx.ram8(0x3d) != 0;
            if figee {
                return;
            }
            let end = o.u8(ctx.b, accessoire::END_ANIMATION);
            if !(end == 0) {
                ajoute_u8(ctx, o, Obj::STATE, 2);
            }
            accessoire_suit(ctx, o);
            ctx.anim_step();
            ctx.reinsert();
        }
        state::LIBERE => ctx.free(&pools::EFFCT),
        state::DETRUIT => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 45 : déclencheur de proximité (0xb5340)

/// Type 45 : objet posé dans le décor qui attend que le joueur principal (`A5+0x46`) passe sur lui : il
/// donne alors un événement à chaque joueur présent (`0x19b08`, décalé par joueur) puis disparaît ; il
/// disparaît aussi s'il reste en arrière de la caméra. Non exercé.
mod declencheur {
    pub const ENTRY: u32 = 0xb5340;
    pub const TABLE: u32 = 0xb534e;
    pub const ANIMATIONS: u32 = 0x1bf5c2;
    pub const PRINCIPAL: i32 = 0x46;
    pub const EFFECT: u32 = 0xea50c;
    pub const OFFSETS: u32 = 0xb53ec;
}

/// `0xb53b0` : déclenchement.
fn trigger_effect<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    crate::sound::play(ctx.b, declencheur::EFFECT, ctx.obj());
    let level = ctx.ram8(0x35) as u32;
    let mut offset = ctx.prog8(declencheur::OFFSETS + level) as u16;
    for i in 0..4u32 {
        let player = Obj(PLAYERS + i * STEP_PLAYER);
        let absent = player.u8(ctx.b, Obj::ACTIVE) == 0;
        if !absent {
            let variant = o.u8(ctx.b, Obj::VARIANT) as u32;
            crate::events::enqueue_counter(ctx.b, (variant as u16).wrapping_add(offset as u16));
        }
        offset = offset.wrapping_add(0x100);
    }
    ctx.free(&pools::EFFCT);
}

fn trigger_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let limite = crate::ram::CAMERA_X.get(ctx.b).wrapping_sub(0x60);
    let x = o.u16(ctx.b, Obj::X);
    if limite as i16 > x as i16 {
        return ctx.free(&pools::EFFCT);
    }
    let principal = Obj(ctx.ram16(declencheur::PRINCIPAL) as i16 as i32 as u32 & 0xff_ffff);
    let dx = principal.u16(ctx.b, Obj::X).wrapping_sub(x).wrapping_add(0x44);
    if dx > 0x74 {
        return;
    }
    let dy = principal.u16(ctx.b, Obj::Y).wrapping_sub(o.u16(ctx.b, Obj::Y)).wrapping_sub(9);
    if !(dy <= 0x1c) {
        return;
    }
    trigger_effect(ctx, o);
}

fn declencheur_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    ctx.anim_start(declencheur::ANIMATIONS, variant as u16);
    ctx.reinsert();
}

fn trigger_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, declencheur::TABLE, state as u32);
    match state {
        state::INIT => declencheur_init(ctx, o),
        state::ACTIVE => trigger_active(ctx, o),
        state::LIBERE => trigger_effect(ctx, o),
        state::DETRUIT => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 4 : bulle d'action d'un personnage (0xaaac0)

/// Type 4 : bulle (icône animée) liée à un joueur (`+0x50`) ou à l'objet qu'il manipule (`+0x54`).
/// L'animation est choisie d'après l'action en cours du porteur (`+0x3d` du joueur, `+0x10d/0x10e`,
/// personnage `+3`) dans de petites tables de la ROM ; huit variantes :
///
/// - 0 : action du joueur (`+0x3d` de 0 à 32 ; au-delà, la table de l'original saute dans des données) ;
/// - 1, 2, 5, 6 : action de l'objet porté (`+0x54`) ou fixe (`0x10`, `0x25` à `0x27`) ;
/// - 3 : bulle secondaire qui se pose sur l'objet porté (`0xaadba`) ;
/// - 4, 7 : animations fixes (`0x23`, `4`).
///
/// L'état 2 anime la bulle (variante 3 : la fait clignoter et la recolle à son porteur) ; l'état 4
/// libère ; un code de table valant `0x29` fait naître une bulle de variante 3.
mod bulle {
    pub const ENTRY: u32 = 0xaaac0;
    pub const STATES: u32 = 0xaaace;
    pub const VARIANTS: u32 = 0xaaaee;
    pub const ANIMATIONS: u32 = 0x18d53e;
    // tables de sauts selon l'action (`+0x3d`)
    pub const ACTIONS_0: u32 = 0xaab0e;
    pub const ACTIONS_1: u32 = 0xaab40;
    pub const ACTIONS_2: u32 = 0xaab5e;
    pub const ACTIONS_5: u32 = 0xaab84;
    pub const ACTIONS_6: u32 = 0xaab9c;
    // tables d'animations (données)
    pub const TABLE_77: u32 = 0xab4de;
    pub const TABLE_COUP: u32 = 0xab4f6;
    pub const TABLE_CHARACTER: u32 = 0xab4f0;
    pub const TABLE_OBJECT_A: u32 = 0xab4e4;
    pub const TABLE_OBJECT_B: u32 = 0xab4ea;
    pub const TABLE_10D_A: u32 = 0xab676;
    pub const TABLE_10D_B: u32 = 0xab6a6;
    pub const TABLE_10D_C: u32 = 0xab6d6;
    /// Décalages de la variante 3 (selon la classe du porteur, `+0x43` et `+0x86` du joueur).
    pub const OFFSETS_3: u32 = 0xaae2a;
    pub const OWNER: u32 = 0x50;
    pub const DOOR: u32 = 0x54;
    pub const END_ANIMATION: u32 = 0x43;
    pub const COUP: u32 = 0x33;
    pub const MINUTEUR: u32 = 0x42;
    pub const PALETTE_GAUGE: u32 = 0x91c41c;
}

/// `0xaac00` : affiche l'animation `number` du jeu des bulles, puis trie l'objet.
fn bulle_affiche<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, number: u8) {
    if !(number != 0x2b) {
        o.set_u16(ctx.b, 0x2a, 0x115);
    }
    ctx.anim_start(bulle::ANIMATIONS, number as u16);
    ctx.reinsert();
}

/// `0xab26a` : un code `0x29` fait naître une bulle de variante 3 (ou, si le porteur est une BOX, joue
/// l'animation 4).
fn bubble_created_next<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let door = pointeur(ctx, o, bulle::DOOR);
    let class = door.u8(ctx.b, Obj::CLASS);
    if !(class != 0xa) {
        return bulle_affiche(ctx, o, 4);
    }
    o.set_u32(ctx.b, Obj::STATE, 0);
    o.set_u8(ctx.b, Obj::VARIANT, 3);
    o.set_u8(ctx.b, bulle::END_ANIMATION, 0);
    let new = ctx.alloc(&pools::EFFCT);
    if new.is_none() {
        return;
    }
    let n = new.unwrap();
    n.set_u8(ctx.b, Obj::ACTIVE, 1);
    n.set_u8(ctx.b, Obj::KIND, 4);
    let p = o.u16(ctx.b, bulle::DOOR);
    n.set_u16(ctx.b, bulle::DOOR, p);
    n.set_u8(ctx.b, Obj::VARIANT, 3);
    n.set_u8(ctx.b, bulle::END_ANIMATION, 1);
}

/// `0xaabf4` : lit le code d'animation dans la table `table` (donnée) à l'indice `index` ; le code
/// `0x29` crée la suite, les autres sont affichés.
fn bulle_lit<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, table: u32, index: u16) {
    let code = ctx.data8(table + index as u32);
    if code == 0x29 {
        return bubble_created_next(ctx, o);
    }
    bulle_affiche(ctx, o, code);
}

/// The original jumps into another routine's epilogue (`movem.l (SP)+,{A1-A4} ; rts`, 0xaa9fe): the next four return
/// addresses are popped into A1-A4, and the return goes to the fifth. These are the callers' returns: the bubble goes
/// straight back to the top of the game loop (`0x1c468`), over the EFFCT dispatch, the world update, the end of the
/// phase and the end of the frame. Here they are the task's four most recent pending calls; the handler's return goes
/// to the fifth, and the overtaken Rust functions (dispatch loop...) give up (`tasks::table::returned`). Nobody reads
/// A1-A4 (those addresses): the task resumes in Rust, without registers.
/// Original glitch: the rest of the frame (next objects, sprites, collisions, end of frame) is skipped.
fn foreign_epilogue<R>(ctx: &mut Cx<'_, R>) {
    crate::tasks::table::pop_calls(ctx.b, 4);
}

/// `0xaace8` : suite commune. Pour le personnage 1 l'original saute dans l'épilogue d'une autre
/// routine (`movem.l (SP)+,{A1-A4} ; rts`) : on reproduit l'effet sur la pile.
fn bubble_next<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let owner = pointeur(ctx, o, bulle::OWNER);
    let character = (owner.u8(ctx.b, 3) >> 1).wrapping_sub(1);
    if character == 0 {
        return foreign_epilogue(ctx);
    }
    create_impact(ctx, o);
}

/// `0xb13fa` : crée l'étincelle d'impact (type 29, variante 0) sur l'objet porté (`+0x54`). Pour le personnage 1,
/// l'appelant saute dans l'épilogue `0xaa9fe` d'une autre routine (voir `foreign_epilogue`).
fn create_impact<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    if let Some(n) = ctx.alloc(&pools::EFFCT) {
        n.set_u8(ctx.b, Obj::ACTIVE, 1);
        n.set_u8(ctx.b, Obj::KIND, 0x1d);
        n.set_u8(ctx.b, Obj::VARIANT, 0);
        let door = o.u16(ctx.b, bulle::DOOR);
        n.set_u16(ctx.b, 0x5c, door);
    }
}

/// `0xaacfe` : une fois sur seize environ, tire au hasard si on poursuit (`0x5555` : bits pairs).
fn bulle_hasard<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let rng = ctx.random();
    let draw = rng & 0xf;
    if 0x5555u16 >> draw & 1 != 0 {
        bubble_next(ctx, o);
    }
}

/// Cas communs de la variante 0 : `0xaabc4` (octet `+0x10d` ou `+0x10e` du joueur dans D0).
fn bubble_0_common<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, player: Obj, d0: u8) {
    let class = player.u8(ctx.b, 3) >> 1;
    let mark = player.u8(ctx.b, 0x77);
    if !(mark == 0) {
        player.set_u8(ctx.b, 0x77, 0);
        bulle_lit(ctx, o, bulle::TABLE_77, class as u16);
        return bulle_hasard(ctx, o);
    }
    let index = ((d0 as u16) << 3).wrapping_add(class as u16);
    bulle_lit(ctx, o, bulle::TABLE_COUP, index);
    bulle_hasard(ctx, o);
}

/// `0xaabac` : animation selon `+0x10d` (ou `+0x10e` pour le personnage 4).
fn bulle_0_action_10d<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, player: Obj) {
    let mut d0 = player.u8(ctx.b, 0x10d);
    let class = player.u8(ctx.b, 3) >> 1;
    if !(class != 4) {
        d0 = player.u8(ctx.b, 0x10e);
    }
    bubble_0_common(ctx, o, player, d0);
}

/// Tables de la variante 0 : l'action `0x3d` du joueur mène à l'une des dix routines.
fn bulle_0<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let player = pointeur(ctx, o, bulle::OWNER);
    let action = player.u8(ctx.b, 0x3d);
    let target = target(ctx, bulle::ACTIONS_0, action as u32);
    let byte = |ctx: &mut Cx<'_, R>, off: u32| player.u8(ctx.b, off) as u16;
    let class = player.u8(ctx.b, 3) >> 1;
    match action {
        0 => bulle_0_action_10d(ctx, o, player),
        2 => {
            let d0 = player.u8(ctx.b, 0x10e);
            bubble_0_common(ctx, o, player, d0);
        }
        4 => {
            let index = byte(ctx, 0x10d);
            bulle_lit(ctx, o, bulle::TABLE_10D_A, index);
        }
        6 => {
            let index = byte(ctx, 0x10e);
            bulle_lit(ctx, o, bulle::TABLE_10D_A, index);
        }
        8 => {
            let index = byte(ctx, 0x10d);
            bulle_lit(ctx, o, bulle::TABLE_10D_C, index);
        }
        10 => {
            let index = byte(ctx, 0x10e);
            bulle_lit(ctx, o, bulle::TABLE_10D_C, index);
        }
        12 => {
            let mut index = byte(ctx, 0x10d);
            if !(class != 4) {
                index = byte(ctx, 0x10e);
            }
            bulle_lit(ctx, o, bulle::TABLE_10D_B, index);
        }
        14 => bulle_lit(ctx, o, bulle::TABLE_CHARACTER, class as u16),
        16 => {
            let door = pointeur(ctx, o, bulle::DOOR);
            let is_box = door.u8(ctx.b, Obj::CLASS) == 0xa;
            if is_box {
                return bulle_0_action_10d(ctx, o, player);
            }
            let gauge_inactive = ctx.ram16(0x4be) != 0;
            if !gauge_inactive {
                crate::mem::wr16(ctx.b, bulle::PALETTE_GAUGE, 0xf0ff);
            }
            bulle_lit(ctx, o, bulle::TABLE_OBJECT_A, class as u16);
            bubble_next(ctx, o);
        }
        // 0xaacba (les actions 18 et 20 partagent le même code)
        18 | 20 => {
            let door = pointeur(ctx, o, bulle::DOOR);
            let is_box = door.u8(ctx.b, Obj::CLASS) == 0xa;
            if is_box {
                return bulle_0_action_10d(ctx, o, player);
            }
            let gauge_inactive = ctx.ram16(0x4be) != 0;
            if !gauge_inactive {
                crate::mem::wr16(ctx.b, bulle::PALETTE_GAUGE, 0xf0ff);
            }
            bulle_lit(ctx, o, bulle::TABLE_OBJECT_B, class as u16);
            bubble_next(ctx, o);
        }
        // 0xaac82
        22 => bulle_affiche(ctx, o, 0x23),
        // 0xaad12 : la jauge du joueur (`+0x77`) vient d'être remplie
        24 | 26 | 28 | 30 | 32 => {
            let gauge = player.u8(ctx.b, 0x77) != 0;
            if gauge {
                player.set_u8(ctx.b, 0x77, 0);
                o.set_u8(ctx.b, field::MODE, 1);
                crate::mem::wr16(ctx.b, bulle::PALETTE_GAUGE, 0xff00);
                return bulle_affiche(ctx, o, 0x24);
            }
            bulle_affiche(ctx, o, 0x10);
        }
        _ => outside_table(target),
    }
}

/// Variantes 1, 2, 5 et 6 : l'action `0x3d` de l'objet porté (`+0x54` ; joueur pour la variante 1) choisit
/// une animation fixe.
fn bubble_by_action<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, offset_pointeur: u32, doubler: bool, table: u32, count: u32) {
    let source = pointeur(ctx, o, offset_pointeur);
    let action = source.u8(ctx.b, 0x3d) as u32;
    let index = if doubler { action * 2 } else { action };
    let target = target(ctx, table, index);
    if index >= count || (!doubler && index & 1 != 0) {
        // une action hors de la table mènerait l'original dans le code qui la suit
        outside_table(target);
    }
    let code = match (table, index / if doubler { 2 } else { 1 }) {
        (bulle::ACTIONS_1, _) => 0x10,
        (_, 0) => 0x25,
        (_, 1) => 0x26,
        _ => 0x27,
    };
    bulle_affiche(ctx, o, code);
}

/// Variante 3 (`0xaadba`) : recolle la bulle à son porteur.
fn bulle_3_recolle<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let door = pointeur(ctx, o, bulle::DOOR);
    let mut d0: u8 = 0;
    let class = door.u8(ctx.b, Obj::CLASS);
    if !(class == 2) {
        d0 = 0x20;
    }
    d0 = d0.wrapping_add(door.u8(ctx.b, Obj::KIND));
    let flip = door.u8(ctx.b, field::FLIP) ^ 0x20;
    o.set_u8(ctx.b, field::FLIP, flip);
    for off in [Obj::X, Obj::Z] {
        let v = door.u16(ctx.b, off);
        o.set_u16(ctx.b, off, v);
    }
    let y = door.u32(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u32(ctx.b, Obj::Y, y);
    let next = (o.u8(ctx.b, bulle::END_ANIMATION) & 1) as u16;
    let base = bulle::OFFSETS_3 + (((d0 as u16) << 4).wrapping_add(next << 3)) as u32;
    let index = ((door.u8(ctx.b, 0x86) as u32) << 2) & 0xffff;
    let mut dx = ctx.data16(base + index);
    let dz = ctx.data16(base + index + 2);
    shifted_u16(ctx, o, Obj::Z, dz);
    if !(flip != 0) {
        dx = dx.wrapping_neg();
    }
    shifted_u16(ctx, o, Obj::X, dx);
}

fn bulle_3_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let end = o.u8(ctx.b, bulle::END_ANIMATION);
    let number = if end == 0 { 0x29 } else { 0x2a };
    ctx.anim_start(bulle::ANIMATIONS, number);
    o.set_u8(ctx.b, bulle::MINUTEUR, 0x1e);
    bulle_3_recolle(ctx, o);
}

fn bulle_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, field::MODE, 0);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let target = target(ctx, bulle::VARIANTS, 2 * variant as u32);
    match variant {
        0 => bulle_0(ctx, o),
        1 => bubble_by_action(ctx, o, bulle::OWNER, false, bulle::ACTIONS_1, 6),
        2 => bubble_by_action(ctx, o, bulle::DOOR, true, bulle::ACTIONS_2, 20),
        3 => bulle_3_init(ctx, o),
        4 => bulle_affiche(ctx, o, 0x23),
        5 => bubble_by_action(ctx, o, bulle::DOOR, true, bulle::ACTIONS_5, 6),
        6 => bubble_by_action(ctx, o, bulle::DOOR, true, bulle::ACTIONS_6, 6),
        7 => bulle_affiche(ctx, o, 4),
        _ => outside_table(target),
    }
}

fn bubble_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if variant == 3 {
        // variante 3 : clignote, se recolle, et passe à l'état suivant au bout de `MINUTEUR` images
        let flags = o.u8(ctx.b, Obj::FLAGS);
        o.set_u8(ctx.b, Obj::FLAGS, flags ^ 2);
        ctx.anim_step();
        bulle_3_recolle(ctx, o);
        let remainder = o.u8(ctx.b, bulle::MINUTEUR).wrapping_sub(1);
        o.set_u8(ctx.b, bulle::MINUTEUR, remainder);
        if !(remainder != 0) {
            ajoute_u8(ctx, o, Obj::STATE, 2);
        }
        return ctx.reinsert();
    }
    let end = ctx.anim_step();
    if end {
        ajoute_u8(ctx, o, Obj::STATE, 2);
        return;
    }
    // la jauge de scroll du fond suit l'appui du bouton lié au coup (`+0x33`)
    let mut value = 0u16;
    let coup = o.u8(ctx.b, bulle::COUP);
    if !(coup == 0) {
        let gauge = ctx.ram16(0x4be) != 0;
        if !gauge {
            let blocked = ctx.ram8(0x4f) != 0;
            if !blocked {
                value = 0x100;
            }
        }
    }
    let address = crate::mem::A5.wrapping_add(0x4ba);
    crate::mem::wr16(ctx.b, address, value);
    ctx.reinsert();
}

/// Libération (`0xab24c`) : rend la palette de la jauge si la bulle l'avait modifiée.
fn bubble_end<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let modifiee = o.u8(ctx.b, field::MODE);
    if !(modifiee == 0) {
        let gauge = ctx.ram16(0x4be) != 0;
        if !gauge {
            crate::mem::wr16(ctx.b, bulle::PALETTE_GAUGE, 0xffff);
        }
    }
    ctx.free(&pools::EFFCT);
}

fn bubble_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, bulle::STATES, state as u32);
    match state {
        state::INIT => bulle_init(ctx, o),
        state::ACTIVE => bubble_active(ctx, o),
        state::LIBERE => bubble_end(ctx, o),
        6 => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

handler!(EFFCT_4, bulle::ENTRY, "effct_bulle", #[without] bubble_handler);
handler!(EFFCT_24, accessoire::ENTRY, "effct_accessoire", #[without] accessory_handler);
handler!(EFFCT_45, declencheur::ENTRY, "effct_declencheur", #[without] trigger_handler);
handler!(EFFCT_51, projectile::ENTRY, "effct_projectile", #[without] projectile_handler);
handler!(EFFCT_52, shimmer_class::ENTRY, "effct_scintille_classe", #[without] shimmer_class_handler);
handler!(EFFCT_56, eclats_debris::ENTRY, "effct_eclats_debris", #[without] shards_debris_handler);
// ---------------------------------------------------------------------------------------------
// Type 8 : trace sur un objet (0xababa)

/// Type 8 : trace collée à un objet (`+0x5c`), un sous-pixel devant ou derrière selon la variante ; son
/// animation dépend du type de l'objet (et de sa classe : BOX) ; elle disparaît à la fin de l'animation.
/// Non exercé.
mod trace {
    pub const ENTRY: u32 = 0xababa;
    pub const TABLE: u32 = 0xabac8;
    pub const ANIMATIONS_TYPE: u32 = 0xabb26;
    pub const ANIMATIONS: u32 = 0x18e872;
    pub const PORTEUR: u32 = 0x5c;
}

fn trace_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let porteur = pointeur(ctx, o, trace::PORTEUR);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = porteur.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let offset: u32 = if variant == 0 { 0xffff_ffff } else { 1 };
    let y = o.u32(ctx.b, Obj::Y).wrapping_add(offset);
    o.set_u32(ctx.b, Obj::Y, y);
    let mut d0: u8 = 0;
    let class = porteur.u8(ctx.b, Obj::CLASS);
    if !(class != 4) {
        d0 = 0x20;
    }
    d0 = d0.wrapping_add(porteur.u8(ctx.b, Obj::KIND));
    let animation = ctx.prog8(trace::ANIMATIONS_TYPE + d0 as u32).wrapping_add(variant);
    ctx.anim_start(trace::ANIMATIONS, animation as u16);
    ctx.reinsert();
}

fn trace_active<R: Bench>(ctx: &mut Cx<'_, R>, _o: Obj) {
    let end = ctx.anim_step();
    if end {
        ctx.free(&pools::EFFCT);
    }
}

fn trace_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, trace::TABLE, trace_init, trace_active)
}

handler!(EFFCT_8, trace::ENTRY, "effct_trace", #[without] trace_handler);
handler!(EFFCT_28, etincelle::ENTRY, "effct_etincelle", #[without] spark_handler);
handler!(EFFCT_57, mark::ENTRY, "effct_marque", #[without] mark_handler);
handler!(EFFCT_62, gerbe::ENTRY, "effct_gerbe", #[without] sheaf_handler);
handler!(EFFCT_7, eclat_unique::ENTRY, "effct_eclat_unique", #[without] shard_single_handler);
handler!(EFFCT_19, balayage::ENTRY, "effct_balayage", #[without] sweep_handler);
handler!(EFFCT_27, fumee::ENTRY, "effct_fumee", #[without] smoke_handler);
handler!(EFFCT_1, 0xa8758, "effct_supprime", #[without] supprime);
handler!(EFFCT_6, debris::ENTRY, "effct_debris", #[without] debris_handler);
handler!(EFFCT_29, impact::ENTRY, "effct_impact", #[without] impact_handler);
handler!(EFFCT_17, interface::ENTRY, "effct_interface", #[without] interface_handler);
handler!(EFFCT_5, halo::ENTRY, "effct_halo", #[without] halo);
handler!(EFFCT_9, trainee::ENTRY, "effct_trainee", #[without] trainee);
handler!(EFFCT_15, scintillement::ENTRY, "effct_scintillement", #[without] scintillement);
handler!(EFFCT_21, banner::ENTRY, "effct_bandeau", #[without] banner);
handler!(EFFCT_25, poussiere::ENTRY, "effct_poussiere", #[without] poussiere);
handler!(EFFCT_26, nuage::ENTRY, "effct_nuage", #[without] nuage);
handler!(EFFCT_32, aura::ENTRY, "effct_aura", #[without] aura);
handler!(EFFCT_33, eclat::ENTRY, "effct_eclat", #[without] eclat);
