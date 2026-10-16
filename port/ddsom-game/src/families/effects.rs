//! Famille EFFCT (classe 0xE) : effets visuels (étincelles, éclats, bannières, éléments d'interface
//! animés). Dispatch `0x39cba`, table des types `0x39ce0` (64 types, 38 emplacements de 0x60 octets).
//!
//! Tous les gestionnaires ont la même charpente : l'octet d'état `+4` choisit, par une table de mots
//! relatifs, l'initialisation (0), l'état actif (2) ou la libération (4). Les types portés ici sont
//! ceux que les scénarios exercent ; les autres restent en code d'origine. Voir `docs/familles.md`.

use crate::objects::pools;
use crate::objects::Obj;

use super::{ajoute_u8, gestionnaire, pointeur, rom, Ctx};

/// Valeurs de l'octet d'état `+4`.
mod etat {
    pub const INIT: u8 = 0;
    pub const ACTIF: u8 = 2;
    pub const LIBERE: u8 = 4;
}

/// Champs partagés.
mod champ {
    pub const SOUS_ETAT: u32 = 0x05;
    pub const MODE: u32 = 0x2f;
    pub const FLIP: u32 = 0x2c;
    pub const PROPRIETAIRE: u32 = 0x50;
}

/// Joueurs (4 enregistrements de 0x200 octets).
const JOUEURS: u32 = 0xff85de;
const PAS_JOUEUR: u32 = 0x200;
/// Jeu d'animations « interface » commun à de nombreux effets.
const ANIMATIONS_INTERFACE: u32 = 0x1c03a6;

fn lit_ram16(ctx: &mut Ctx, decalage: i32) -> u16 {
    ctx.ram16(decalage)
}

/// Passe `X`, `Z`, `Y` (mots hauts) de `o` à ceux de `source`.
fn copie_position(ctx: &mut Ctx, o: Obj, source: Obj) {
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = source.u16(ctx.b, off);
        o.set_u16(ctx.b, off, v);
    }
}

/// Ajoute `n` au mot haut d'une coordonnée.
fn decale_u16(ctx: &mut Ctx, o: Obj, off: u32, n: u16) {
    let v = o.u16(ctx.b, off).wrapping_add(n);
    o.set_u16(ctx.b, off, v);
}

/// Gestionnaire à trois états (init, actif, libération) : lit l'état, saute par la table de mots
/// `table`, exécute `init` ou `actif` ; l'état 4 libère l'objet.
fn trois_etats(
    ctx: &mut Ctx,
    table: u32,
    init: fn(&mut Ctx, Obj),
    actif: fn(&mut Ctx, Obj),
) {
    let o = ctx.obj();
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(table, etat as u32);
    match etat {
        etat::INIT => init(ctx, o),
        etat::ACTIF => actif(ctx, o),
        etat::LIBERE => ctx.free(&pools::FREE_EFFCT),
        _ => ctx.deleguer(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 5 : halo attaché à un joueur (0xab726)

/// Type 5 : apparaît à la position du joueur propriétaire (`+0x50`), décalée selon la variante (paire,
/// 0 à 10) et l'orientation du joueur, attend un délai (octet `+0x18`, fonction de l'octet `+0x7d` du
/// joueur) puis vit `0x28` images avant de disparaître. Animation 0 du jeu `0x18e650`.
mod halo {
    pub const ENTREE: u32 = 0xab726;
    pub const TABLE: u32 = 0xab734;
    /// Décalages (dx, dz) par demi-variante, mots.
    pub const DECALAGES: u32 = 0xab79a;
    /// Délais d'apparition selon `+0x7d` du joueur.
    pub const DELAIS: u32 = 0xab7b2;
    pub const ANIMATIONS: u32 = 0x18e650;
    pub const DUREE: u32 = 0x14;
    pub const DELAI: u32 = 0x18;
}

fn halo_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, halo::DUREE, 0x28);
    let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
    copie_position(ctx, o, joueur);
    let rang = ((o.u8(ctx.b, Obj::VARIANT) >> 1) as u32) << 2;
    let mut dx = ctx.prog16(halo::DECALAGES + rang);
    let dz = ctx.prog16(halo::DECALAGES + 2 + rang);
    let retourne = joueur.u8(ctx.b, champ::FLIP) != 0;
    if !ctx.pris(retourne) {
        dx = dx.wrapping_neg();
    }
    decale_u16(ctx, o, Obj::X, dx);
    decale_u16(ctx, o, Obj::Z, dz);
    let phase = joueur.u8(ctx.b, 0x7d) as u32;
    let delai = ctx.prog8(halo::DELAIS + phase);
    o.set_u8(ctx.b, halo::DELAI, delai);
    ctx.anim_start(halo::ANIMATIONS, 0);
}

fn halo_actif(ctx: &mut Ctx, o: Obj) {
    let delai = o.u8(ctx.b, halo::DELAI);
    if !ctx.pris(delai == 0) {
        o.set_u8(ctx.b, halo::DELAI, delai - 1);
        return;
    }
    let reste = o.u8(ctx.b, halo::DUREE).wrapping_sub(1);
    o.set_u8(ctx.b, halo::DUREE, reste);
    if !ctx.pris(reste != 0) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
    ctx.reinsert();
}

fn halo(ctx: &mut Ctx) {
    trois_etats(ctx, halo::TABLE, halo_init, halo_actif)
}

// ---------------------------------------------------------------------------------------------
// Type 21 : bandeau d'interface qui glisse (0xaf360)

/// Type 21 : bandeau d'interface (variante 0 : en bas, 1 : en haut) qui glisse en `Z` d'un pixel par
/// image (tant que `A5+0x6f` bit 0 est nul) jusqu'à la hauteur cible `+0x18`, puis se libère en
/// décrémentant l'octet pointé par `+0x58` (compteur de bandeaux du créateur).
mod bandeau {
    pub const ENTREE: u32 = 0xaf360;
    pub const TABLE: u32 = 0xaf36e;
    pub const CIBLE: u32 = 0x18;
    pub const CREATEUR: u32 = 0x58;
}

fn bandeau_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u32(ctx.b, Obj::X, 0);
    o.set_u32(ctx.b, Obj::Y, 0);
    o.set_u16(ctx.b, Obj::Z, 0xc8);
    o.set_u16(ctx.b, bandeau::CIBLE, 0x100);
    let variante = o.u8(ctx.b, Obj::VARIANT);
    if !ctx.pris(variante == 0) {
        o.set_u16(ctx.b, Obj::Z, 0x88);
        o.set_u16(ctx.b, bandeau::CIBLE, 0x50);
    }
    ctx.anim_start(ANIMATIONS_INTERFACE, 0xec);
    ctx.reinsert();
}

/// Fin du bandeau : décrémente le compteur du créateur et libère l'objet.
fn bandeau_fin(ctx: &mut Ctx, o: Obj) {
    let compteur = crate::mem::rd32(ctx.b, o.0 + bandeau::CREATEUR);
    let v = crate::mem::rd8(ctx.b, compteur).wrapping_sub(1);
    crate::mem::wr8(ctx.b, compteur, v);
    ctx.free(&pools::FREE_EFFCT);
}

fn bandeau_actif(ctx: &mut Ctx, o: Obj) {
    let pause = ctx.ram8(0x6f) & 1 != 0;
    if !ctx.pris(pause) {
        let variante = o.u8(ctx.b, Obj::VARIANT);
        let pas: u16 = if ctx.pris(variante == 0) { 1 } else { 1u16.wrapping_neg() };
        decale_u16(ctx, o, Obj::Z, pas);
        let cible = o.u16(ctx.b, bandeau::CIBLE);
        let z = o.u16(ctx.b, Obj::Z);
        if ctx.pris(cible == z) {
            return bandeau_fin(ctx, o);
        }
    }
    ctx.reinsert();
}

fn bandeau(ctx: &mut Ctx) {
    let o = ctx.obj();
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(bandeau::TABLE, etat as u32);
    match etat {
        etat::INIT => bandeau_init(ctx, o),
        etat::ACTIF => bandeau_actif(ctx, o),
        etat::LIBERE => bandeau_fin(ctx, o),
        _ => ctx.deleguer(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 32 : aura qui suit son propriétaire (0xb2c56)

/// Type 32 : se place 0x10 pixels devant (selon son orientation) et un sous-pixel devant le
/// propriétaire (`+0x50`), 0x18 pixels au-dessus de lui, et le suit à chaque image (animation 0 du
/// jeu `0x193fea`).
mod aura {
    pub const ENTREE: u32 = 0xb2c56;
    pub const TABLE: u32 = 0xb2c64;
    pub const ANIMATIONS: u32 = 0x193fea;
}

/// `0xb2ca4` : position du propriétaire, un seizième de pixel devant, décalée de 0x10 pixels.
fn aura_suit(ctx: &mut Ctx, o: Obj) -> Obj {
    let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
    let x = joueur.u32(ctx.b, Obj::X);
    o.set_u32(ctx.b, Obj::X, x);
    let y = joueur.u32(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u32(ctx.b, Obj::Y, y);
    let retourne = o.u8(ctx.b, champ::FLIP) != 0;
    let decalage: u16 = if ctx.pris(!retourne) { 0xfff0 } else { 0x10 };
    decale_u16(ctx, o, Obj::X, decalage);
    joueur
}

fn aura_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let joueur = aura_suit(ctx, o);
    let z = joueur.u32(ctx.b, Obj::Z);
    o.set_u32(ctx.b, Obj::Z, z);
    decale_u16(ctx, o, Obj::Z, 0x18);
    ctx.anim_start(aura::ANIMATIONS, 0);
    ctx.reinsert();
}

fn aura_actif(ctx: &mut Ctx, o: Obj) {
    aura_suit(ctx, o);
    ctx.reinsert();
}

fn aura(ctx: &mut Ctx) {
    trois_etats(ctx, aura::TABLE, aura_init, aura_actif)
}

// ---------------------------------------------------------------------------------------------
// Type 15 : scintillement d'un objet (0xae428)

/// Type 15 : scintillement posé sur un objet ramassable (`+0x5c`, créé par `0xae3ec` avec la variante
/// `+0x54` de l'objet) ; suit l'objet, 0x10 pixels plus haut, tant qu'il est actif (état 2). Tant
/// qu'aucun joueur n'est passé à proximité (±0x20 en profondeur, ±0x60 en abscisse) il reste visible ;
/// ensuite il clignote par périodes de 0x30 images, huit fois, puis disparaît.
mod scintillement {
    pub const ENTREE: u32 = 0xae428;
    pub const TABLE: u32 = 0xae436;
    pub const ANIMATIONS: u32 = 0x18fd48;
    pub const REPETITIONS: u32 = 0x57;
    pub const DELAI: u32 = 0x58;
    pub const BASCULE: u32 = 0x5a;
    pub const DECLENCHE: u32 = 0x5b;
    pub const PORTEUR: u32 = 0x5c;
}

fn scintillement_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    o.set_u8(ctx.b, scintillement::REPETITIONS, 8);
    ctx.jsr(rom::RNG);
    let tirage = ctx.c.d[0] as u16 & 0x1f;
    o.set_u16(ctx.b, scintillement::DELAI, tirage);
    let variante = o.u8(ctx.b, Obj::VARIANT);
    ctx.anim_start(scintillement::ANIMATIONS, variante as u16);
    ctx.reinsert();
}

/// `0xae4e4` : un joueur est-il passé près du scintillement ?
fn scintillement_joueur_proche(ctx: &mut Ctx, o: Obj) {
    for i in 0..4u32 {
        let joueur = Obj(JOUEURS + i * PAS_JOUEUR);
        let absent = joueur.u8(ctx.b, Obj::ACTIVE) == 0;
        if !ctx.pris(absent) {
            let dy = o.u16(ctx.b, Obj::Y).wrapping_sub(joueur.u16(ctx.b, Obj::Y)).wrapping_add(0x20);
            if !ctx.pris(dy > 0x40) {
                let dx = o.u16(ctx.b, Obj::X).wrapping_sub(joueur.u16(ctx.b, Obj::X)).wrapping_add(0x60);
                if !ctx.pris(dx > 0xc0) {
                    o.set_u8(ctx.b, scintillement::DECLENCHE, 1);
                    return;
                }
            }
        }
        ctx.pris(i < 3); // dbf
    }
}

fn scintillement_actif(ctx: &mut Ctx, o: Obj) {
    let porteur = pointeur(ctx, o, scintillement::PORTEUR);
    copie_position(ctx, o, porteur);
    decale_u16(ctx, o, Obj::Z, 0x10);
    let etat_porteur = porteur.u8(ctx.b, Obj::STATE);
    if !ctx.pris(etat_porteur == 2) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
    let declenche = o.u8(ctx.b, scintillement::DECLENCHE);
    if !ctx.pris(declenche != 0) {
        // pas encore déclenché : on guette les joueurs (et on rend la main sans trier)
        return scintillement_joueur_proche(ctx, o);
    }
    let reste = o.u16(ctx.b, scintillement::DELAI).wrapping_sub(1);
    o.set_u16(ctx.b, scintillement::DELAI, reste);
    if !ctx.pris(reste != 0) {
        o.set_u16(ctx.b, scintillement::DELAI, 0x30);
        let bascule = o.u8(ctx.b, scintillement::BASCULE) ^ 1;
        o.set_u8(ctx.b, scintillement::BASCULE, bascule);
        let flags = o.u8(ctx.b, Obj::FLAGS);
        o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
        if !ctx.pris(bascule != 0) {
            o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
            let reste = o.u8(ctx.b, scintillement::REPETITIONS).wrapping_sub(1);
            o.set_u8(ctx.b, scintillement::REPETITIONS, reste);
            if !ctx.pris(reste != 0) {
                o.set_u32(ctx.b, Obj::STATE, 0x0400_0000);
            }
        }
    }
    ctx.reinsert();
}

fn scintillement(ctx: &mut Ctx) {
    trois_etats(ctx, scintillement::TABLE, scintillement_init, scintillement_actif)
}

// ---------------------------------------------------------------------------------------------
// Type 9 : traînée qui traverse l'écran (0xabbee)

/// Type 9 : objet qui naît à une position d'écran tirée d'une table (selon `+0x52`), avance en `X`
/// (vitesse 0x200), attend 0x120 images (animation `variante + 0xc`) puis joue son animation de
/// sortie 0x200 images, en avançant les 0x50 dernières, avant de disparaître.
mod trainee {
    pub const ENTREE: u32 = 0xabbee;
    pub const TABLE: u32 = 0xabbfc;
    pub const POSITIONS: u32 = 0xabc54;
    pub const ANIMATIONS: u32 = 0x18ea30;
    pub const DELAI: u32 = 0x50;
    pub const POSITION: u32 = 0x52;
}

fn trainee_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u16(ctx.b, trainee::DELAI, 0x120);
    o.set_u32(ctx.b, Obj::VX, 0x0200_0000);
    let rang = (o.u8(ctx.b, trainee::POSITION) as u32) << 2;
    let x = ctx.prog16(trainee::POSITIONS + rang);
    o.set_u16(ctx.b, Obj::X, x);
    let y = ctx.prog16(trainee::POSITIONS + rang + 2);
    o.set_u16(ctx.b, Obj::Y, y);
    o.set_u16(ctx.b, Obj::Z, 0);
    let (cam_x, cam_y) = (lit_ram16(ctx, 0x506), lit_ram16(ctx, 0x50a));
    decale_u16(ctx, o, Obj::X, cam_x);
    decale_u16(ctx, o, Obj::Y, cam_y);
    let animation = o.u8(ctx.b, Obj::VARIANT).wrapping_add(0xc);
    ctx.anim_start(trainee::ANIMATIONS, animation as u16);
}

fn trainee_actif(ctx: &mut Ctx, o: Obj) {
    let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
    if !ctx.pris(sous_etat != 0) {
        let reste = o.u16(ctx.b, trainee::DELAI).wrapping_sub(1);
        o.set_u16(ctx.b, trainee::DELAI, reste);
        if !ctx.pris(reste != 0) {
            ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
            o.set_u16(ctx.b, trainee::DELAI, 0x200);
            let variante = o.u8(ctx.b, Obj::VARIANT);
            return ctx.anim_start(trainee::ANIMATIONS, variante as u16);
        }
        if ctx.pris(reste > 0x50) {
            return;
        }
        ctx.anim_step();
        ctx.jsr(rom::MOVE_X);
        return ctx.reinsert();
    }
    let reste = o.u16(ctx.b, trainee::DELAI).wrapping_sub(1);
    o.set_u16(ctx.b, trainee::DELAI, reste);
    if !ctx.pris(reste != 0) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
    ctx.anim_step();
    ctx.reinsert();
}

fn trainee(ctx: &mut Ctx) {
    trois_etats(ctx, trainee::TABLE, trainee_init, trainee_actif)
}

// ---------------------------------------------------------------------------------------------
// Type 33 : gravats qui retombent (0xb2cca)

/// Type 33 : éclat lancé depuis une position d'écran (variante 0 à 3, vitesses tirées d'une table),
/// dessiné au premier plan. Il retombe ; au contact du sol, les variantes non nulles disparaissent, la
/// variante 0 (hors pause) crée deux tirs ennemis (ESHL 0x17, modes 0 et 1) avant de disparaître.
mod eclat {
    pub const ENTREE: u32 = 0xb2cca;
    pub const TABLE: u32 = 0xb2cd8;
    pub const POSITIONS: u32 = 0xb2d72;
    pub const VITESSES: u32 = 0xb2d52;
    pub const ANIMATIONS: u32 = 0x193ffe;
    pub const DECALAGE: u32 = 0x5c;
    pub const CREER_TIR_A: u32 = 0xeab50;
    pub const CREER_TIR_B: u32 = 0xa8ae8;
}

fn eclat_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    ctx.jsr(rom::RNG);
    let tirage = ctx.c.d[0] as u16 & 0x3f;
    o.set_u16(ctx.b, eclat::DECALAGE, tirage);
    let flip = o.u8(ctx.b, champ::FLIP) ^ 0x20;
    o.set_u8(ctx.b, champ::FLIP, flip);
    let rang = (o.u8(ctx.b, Obj::VARIANT) as u32) << 2;
    let dx = ctx.prog16(eclat::POSITIONS + rang);
    let dz = ctx.prog16(eclat::POSITIONS + rang + 2);
    o.set_u16(ctx.b, Obj::Y, 0x40);
    let (x, z) = (lit_ram16(ctx, 0x556), lit_ram16(ctx, 0x55a));
    o.set_u16(ctx.b, Obj::X, x);
    o.set_u16(ctx.b, Obj::Z, z);
    decale_u16(ctx, o, Obj::X, dx);
    decale_u16(ctx, o, Obj::Z, dz);
    let rang = rang << 1;
    for (i, off) in [Obj::VX, Obj::VZ, Obj::AZ].into_iter().enumerate() {
        let v = ctx.prog16(eclat::VITESSES + rang + 2 * i as u32);
        o.set_u16(ctx.b, off, v);
    }
    let az = o.u16(ctx.b, Obj::AZ).wrapping_neg();
    o.set_u16(ctx.b, Obj::AZ, az);
    ctx.anim_start(eclat::ANIMATIONS, 0);
    ctx.reinsert_foreground();
}

/// Atterrissage (`0xb2d92`) : disparition, ou deux tirs pour la variante 0.
fn eclat_atterrit(ctx: &mut Ctx, o: Obj) {
    let variante = o.u8(ctx.b, Obj::VARIANT);
    if !ctx.pris(variante != 0) {
        let pause = ctx.ram8(0x42) != 0;
        if !ctx.pris(pause) {
            ctx.jsr(eclat::CREER_TIR_A);
            let tir = ctx.alloc(&pools::ALLOC_ESHL);
            if ctx.pris(tir.is_none()) {
                return;
            }
            let tir = tir.unwrap();
            tir.set_u8(ctx.b, Obj::ACTIVE, 1);
            tir.set_u8(ctx.b, Obj::KIND, 0x17);
            tir.set_u8(ctx.b, champ::MODE, 0);
            ctx.c.d[0] = 3;
            ctx.jsr(eclat::CREER_TIR_B);
            let tir = ctx.alloc(&pools::ALLOC_ESHL);
            if ctx.pris(tir.is_none()) {
                return;
            }
            let tir = tir.unwrap();
            tir.set_u8(ctx.b, Obj::ACTIVE, 1);
            tir.set_u8(ctx.b, Obj::KIND, 0x17);
            tir.set_u8(ctx.b, champ::MODE, 1);
        }
    }
    ctx.free(&pools::FREE_EFFCT);
}

fn eclat_actif(ctx: &mut Ctx, o: Obj) {
    ctx.anim_step();
    ctx.jsr(rom::MOVE_XZ);
    let en_vol = ctx.c.cond(10); // bpl
    if ctx.pris(en_vol) {
        return;
    }
    eclat_atterrit(ctx, o)
}

fn eclat(ctx: &mut Ctx) {
    let o = ctx.obj();
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(eclat::TABLE, etat as u32);
    match etat {
        etat::INIT => eclat_init(ctx, o),
        etat::ACTIF => eclat_actif(ctx, o),
        etat::LIBERE => eclat_atterrit(ctx, o),
        _ => ctx.deleguer(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 25 : poussière projetée (0xb111a)

/// Type 25 : éclats projetés à la naissance (vitesse `+0x14` / `+0x1c`, dispersés au hasard selon le
/// mode `+0x2f`, orientés selon `+0x2c`), puis qui dérivent horizontalement en clignotant jusqu'à la
/// fin de leur animation.
mod poussiere {
    pub const ENTREE: u32 = 0xb111a;
    pub const TABLE: u32 = 0xb1128;
    pub const ANIMATIONS: u32 = 0x1901ba;
    /// Sens de la dérive (`+0x2d`) et sur quel axe se fait le saut (`+0x43`).
    pub const SENS: u32 = 0x2d;
    pub const SUR_Z: u32 = 0x43;
}

fn poussiere_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let mut dx = o.u16(ctx.b, Obj::VX);
    let mut dy = o.u16(ctx.b, Obj::VY);
    let retourne = o.u8(ctx.b, champ::FLIP) != 0;
    if !ctx.pris(!retourne) {
        dx = dx.wrapping_neg();
    }
    let mode = o.u8(ctx.b, champ::MODE);
    if !ctx.pris(mode == 0) {
        let d3 = (mode as u16) << 3;
        dx = dx.wrapping_sub(d3);
        dy = dy.wrapping_sub(d3);
        let masque = (d3 << 1).wrapping_sub(1);
        ctx.jsr(rom::RNG);
        dx = dx.wrapping_add(ctx.c.d[0] as u16 & masque);
        ctx.jsr(rom::RNG);
        dy = dy.wrapping_add(ctx.c.d[0] as u16 & masque);
    }
    decale_u16(ctx, o, Obj::X, dx);
    let sur_z = o.u8(ctx.b, poussiere::SUR_Z);
    if !ctx.pris(sur_z == 0) {
        decale_u16(ctx, o, Obj::Z, dy);
    } else {
        decale_u16(ctx, o, Obj::Y, dy);
    }
    o.set_u16(ctx.b, Obj::VX, 0);
    let sens = o.u8(ctx.b, poussiere::SENS);
    if !ctx.pris(sens == 0) {
        let mut v: u16 = 0x180;
        let retourne = o.u8(ctx.b, champ::FLIP) != 0;
        if !ctx.pris(!retourne) {
            v = v.wrapping_neg();
        }
        let sens = o.u8(ctx.b, poussiere::SENS);
        if !ctx.pris(sens == 1) {
            v = v.wrapping_neg();
        }
        o.set_u16(ctx.b, Obj::VX, v);
    }
    let variante = o.u8(ctx.b, Obj::VARIANT);
    ctx.anim_start(poussiere::ANIMATIONS, variante as u16);
    ctx.reinsert();
}

fn poussiere_actif(ctx: &mut Ctx, o: Obj) {
    let vx = o.u16(ctx.b, Obj::VX) as i16 as i32;
    let x = o.u32(ctx.b, Obj::X).wrapping_add((vx << 8) as u32);
    o.set_u32(ctx.b, Obj::X, x);
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags ^ 2);
    let fin = ctx.anim_step();
    if ctx.pris(fin) {
        return ctx.free(&pools::FREE_EFFCT);
    }
    ctx.reinsert();
}

fn poussiere(ctx: &mut Ctx) {
    trois_etats(ctx, poussiere::TABLE, poussiere_init, poussiere_actif)
}

// ---------------------------------------------------------------------------------------------
// Type 26 : nuage qui traverse l'écran (0xb11f4)

/// Type 26 : nuage qui naît à droite de l'écran (hauteur aléatoire), attend un délai aléatoire (`+0x5c`,
/// 0 à 63 images) puis traverse l'écran vers la gauche (vitesse selon la variante) jusqu'à 0x40 pixels
/// au-delà du bord.
mod nuage {
    pub const ENTREE: u32 = 0xb11f4;
    pub const TABLE: u32 = 0xb1202;
    pub const VITESSES: u32 = 0xb126a;
    pub const ANIMATIONS_NUMERO: u32 = 0xb1264;
    pub const ANIMATIONS: u32 = 0x1bf8fe;
    pub const DELAI: u32 = 0x5c;
}

fn nuage_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    ctx.jsr(rom::RNG);
    let tirage = ctx.c.d[0] as u16 & 0x3f;
    o.set_u16(ctx.b, nuage::DELAI, tirage);
    ctx.jsr(rom::RNG);
    let tirage = ctx.c.d[0] as u16 & 0x3f;
    o.set_u16(ctx.b, Obj::Y, tirage);
    decale_u16(ctx, o, Obj::Y, 0x50);
    o.set_u16(ctx.b, Obj::Z, 0);
    let cam = lit_ram16(ctx, 0x556);
    o.set_u16(ctx.b, Obj::X, cam.wrapping_add(0x1c0));
    let variante = o.u8(ctx.b, Obj::VARIANT) as u32;
    let vitesse = ctx.prog16(nuage::VITESSES + 2 * variante);
    o.set_u16(ctx.b, Obj::VX, vitesse);
    let animation = ctx.prog8(nuage::ANIMATIONS_NUMERO + variante);
    ctx.anim_start(nuage::ANIMATIONS, animation as u16);
}

fn nuage_actif(ctx: &mut Ctx, o: Obj) {
    let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
    if !ctx.pris(sous_etat != 0) {
        let reste = o.u16(ctx.b, nuage::DELAI).wrapping_sub(1);
        o.set_u16(ctx.b, nuage::DELAI, reste);
        if !ctx.pris(reste as i16 >= 0) {
            ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
        }
        return;
    }
    ctx.anim_step();
    // D0 garde ses mots hauts d'avant : on exécute l'original pour en reproduire les restes
    ctx.jsr_origine(rom::REINSERT_ACTORS);
    let vx = o.u16(ctx.b, Obj::VX) as u32;
    let d0 = ((ctx.c.d[0] & 0xffff_0000 | vx).rotate_left(16)) as i32 >> 8;
    let x = o.u32(ctx.b, Obj::X).wrapping_sub(d0 as u32);
    o.set_u32(ctx.b, Obj::X, x);
    let limite = lit_ram16(ctx, 0x556).wrapping_sub(0x40);
    let xw = o.u16(ctx.b, Obj::X);
    if !ctx.pris((limite as i16) < xw as i16) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
}

fn nuage(ctx: &mut Ctx) {
    trois_etats(ctx, nuage::TABLE, nuage_init, nuage_actif)
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
    pub const ENTREE: u32 = 0xae78c;
    pub const TABLE: u32 = 0xae79c;
    pub const ETATS_0: u32 = 0xae7bc;
    pub const ETATS_1: u32 = 0xae86a;
    pub const ETATS_2: u32 = 0xae96c;
    pub const ETATS_3: u32 = 0xaebda;
    pub const ETATS_4: u32 = 0xaec3c;
    pub const PALETTE: u32 = 0x91c000;
    pub const PALETTE_0: u32 = 0x91c0e0;
    /// Palettes sources (données) et listes d'adresses de palette (par joueur).
    pub const SOURCES: u32 = 0x28ff00;
    pub const LISTES: u32 = 0xaeb62;
    /// Décalages de la variante 1 par personnage.
    pub const DECALAGES_1: u32 = 0xae93c;
    /// Variante 2 : animations et décalages par joueur.
    pub const ANIMATIONS_2: u32 = 0xae9e6;
    pub const FORMES_2: u32 = 0xae9c2;
    pub const DECALAGES_2: u32 = 0xae9ce;
    pub const DECALAGES_4: u32 = 0xaec8e;
    pub const PORTEUR: u32 = 0x1e;
    pub const JOUEUR: u32 = 0x1a;
    pub const PERSONNAGE: u32 = 0x42;
    // sous-routines non réécrites
    pub const CHANGE_PERSONNAGE: u32 = 0xea344;
    pub const SON_JAUGE: u32 = 0xea3b6;
}

/// Phase de jeu : 8 ou plus, la partie est finie.
fn partie_finie(ctx: &mut Ctx) -> bool {
    let phase = ctx.ram8(0x2c) as i8;
    ctx.pris(phase >= 8)
}

/// Place l'objet sur la caméra (`X` = caméra x, `Z` = caméra y, `Y` = 0).
fn place_sur_camera(ctx: &mut Ctx, o: Obj) {
    let (cam_x, cam_y) = (lit_ram16(ctx, 0x506), lit_ram16(ctx, 0x50a));
    o.set_u16(ctx.b, Obj::X, cam_x);
    o.set_u16(ctx.b, Obj::Z, cam_y);
    o.set_u32(ctx.b, Obj::Y, 0);
}

fn interface_0(ctx: &mut Ctx, o: Obj) {
    if partie_finie(ctx) {
        return ctx.free(&pools::FREE_EFFCT);
    }
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(interface::ETATS_0, etat as u32);
    match etat {
        etat::INIT => {
            let pret = o.u8(ctx.b, 0x1c);
            if ctx.pris(pret == 0) {
                return;
            }
            ajoute_u8(ctx, o, Obj::STATE, 2);
            for i in 0..8 {
                crate::mem::wr32(ctx.b, interface::PALETTE_0 + 4 * i, 0xffff_ffff);
            }
        }
        etat::ACTIF => {}
        _ => ctx.deleguer(cible),
    }
}

fn interface_1(ctx: &mut Ctx, o: Obj) {
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(interface::ETATS_1, etat as u32);
    match etat {
        etat::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            place_sur_camera(ctx, o);
            let numero = o.u8(ctx.b, champ::MODE) as u32;
            for _ in 0..numero {
                ctx.pris(true); // dbf : un joueur de plus à sauter
            }
            ctx.pris(false);
            let joueur = Obj(JOUEURS + numero * PAS_JOUEUR);
            o.set_u16(ctx.b, interface::JOUEUR, joueur.0 as u16);
            let personnage = joueur.u8(ctx.b, 3) & 0x7e;
            o.set_u8(ctx.b, interface::PERSONNAGE, personnage);
            ctx.anim_start(ANIMATIONS_INTERFACE, 0x8e + numero as u16);
        }
        etat::ACTIF => {
            let porteur = pointeur(ctx, o, interface::PORTEUR);
            let porteur_absent = porteur.u8(ctx.b, 0x1c) == 0;
            let joueur = pointeur(ctx, o, interface::JOUEUR);
            if ctx.pris(porteur_absent) || {
                let absent = joueur.u8(ctx.b, Obj::ACTIVE) == 0;
                ctx.pris(absent)
            } {
                if partie_finie(ctx) {
                    ctx.free(&pools::FREE_EFFCT);
                }
                return;
            }
            place_sur_camera(ctx, o);
            let personnage = joueur.u8(ctx.b, 3) & 0x7e;
            let mut change = false;
            let connu = o.u8(ctx.b, interface::PERSONNAGE);
            if !ctx.pris(personnage == connu) {
                o.set_u8(ctx.b, interface::PERSONNAGE, personnage);
                change = true;
            }
            let rang = (personnage as u32) << 1;
            let dx = ctx.prog16(interface::DECALAGES_1 + rang);
            let dz = ctx.prog16(interface::DECALAGES_1 + 2 + rang);
            decale_u16(ctx, o, Obj::X, dx);
            decale_u16(ctx, o, Obj::Z, dz);
            if !ctx.pris(!change) {
                ctx.jsr(interface::CHANGE_PERSONNAGE);
            }
            let rehausse = o.u8(ctx.b, 0x16) != 0;
            if !ctx.pris(!rehausse) {
                o.set_u16(ctx.b, 0x12, 1);
                decale_u16(ctx, o, Obj::Z, 0x10);
            }
            ctx.anim_step();
            ctx.reinsert();
        }
        etat::LIBERE => ctx.free(&pools::FREE_EFFCT),
        _ => ctx.deleguer(cible),
    }
}

/// `0xaeb08` : recopie dans la palette (OR du bit de priorité 0xF000F000) les deux fois 16 octets de
/// chaque emplacement de la liste du joueur ; `decalage` choisit la palette source (0 ou 0x400/0x800).
fn recopie_palette(ctx: &mut Ctx, o: Obj, decalage: u16) {
    let joueur = (o.u8(ctx.b, champ::MODE) as u32) << 1 & 0xff;
    let rang = ctx.prog16(interface::LISTES + joueur) as i16 as i32 as u32;
    let mut liste = interface::LISTES.wrapping_add(rang);
    loop {
        let emplacement = ctx.rom16(liste);
        liste += 2;
        if ctx.pris((emplacement as i16) < 0) {
            return; // bmi : fin de liste
        }
        let destination = interface::PALETTE.wrapping_add(emplacement as i16 as i32 as u32);
        let source = interface::SOURCES.wrapping_add(emplacement.wrapping_add(decalage) as i16 as i32 as u32);
        for k in 0..8 {
            let v = ctx.rom32(source + 4 * k) | 0xf000_f000;
            crate::mem::wr32(ctx.b, destination + 4 * k, v);
        }
    }
}

fn interface_2(ctx: &mut Ctx, o: Obj) {
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(interface::ETATS_2, etat as u32);
    match etat {
        etat::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            place_sur_camera(ctx, o);
            o.set_u16(ctx.b, Obj::VX, 0);
            o.set_u16(ctx.b, Obj::VY, 0);
            o.set_u8(ctx.b, 0x2a, 0);
            o.set_u8(ctx.b, 0x2b, 7);
            let numero = (o.u8(ctx.b, champ::MODE) as u32) << 1;
            let forme = ctx.prog16(interface::FORMES_2 + numero);
            o.set_u16(ctx.b, 0x12, forme);
            let dx = ctx.prog16(interface::DECALAGES_2 + 2 * numero);
            let dz = ctx.prog16(interface::DECALAGES_2 + 2 * numero + 2);
            decale_u16(ctx, o, Obj::X, dx);
            decale_u16(ctx, o, Obj::Z, dz);
            interface_2_animation(ctx, o, 0);
        }
        etat::ACTIF => {
            if partie_finie(ctx) {
                return ctx.free(&pools::FREE_EFFCT);
            }
            let porteur = pointeur(ctx, o, interface::PORTEUR);
            let absent = porteur.u8(ctx.b, 0x1c) == 0;
            if ctx.pris(absent) {
                return ctx.reinsert();
            }
            let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
            if ctx.pris(sous_etat != 0) {
                return ctx.reinsert();
            }
            let numero = o.u8(ctx.b, champ::MODE) as u32;
            let mot = porteur.u16(ctx.b, 8 + 2 * numero);
            let mut d1: u8 = 0;
            if !ctx.pris(mot == 0xffff) {
                d1 = 1;
            }
            let connu = o.u8(ctx.b, 0x14);
            if !ctx.pris(d1 == connu) {
                o.set_u8(ctx.b, 0x14, d1);
                // 0xae9f2
                let decalage = if ctx.pris(d1 == 0) { 0 } else { 0x400 };
                recopie_palette(ctx, o, decalage);
                // la routine enchaîne sur l'animation (0xaea0c), qui trie l'objet puis revient ici
                interface_2_animation(ctx, o, d1 as u16);
            }
            let bits = 3u16 << ((numero << 1) & 0x1f);
            let valeur = porteur.u16(ctx.b, 0x14) & bits;
            let ancienne = o.u16(ctx.b, 0x1c);
            if !ctx.pris(valeur == ancienne) {
                o.set_u16(ctx.b, 0x1c, valeur);
                let clignote = o.u8(ctx.b, 0x15) != 0;
                if !ctx.pris(clignote) {
                    o.set_u8(ctx.b, 0x15, 0xc);
                    ctx.jsr(interface::SON_JAUGE);
                }
            }
            let reste = o.u8(ctx.b, 0x15);
            if ctx.pris(reste == 0) {
                return ctx.reinsert();
            }
            let drapeaux = o.u8(ctx.b, 0x2a);
            o.set_u8(ctx.b, 0x2a, drapeaux & !1);
            let reste = reste.wrapping_sub(1);
            o.set_u8(ctx.b, 0x15, reste);
            if !ctx.pris(reste == 0) {
                if ctx.pris(reste & 2 == 0) {
                    return ctx.reinsert();
                }
                let drapeaux = o.u8(ctx.b, 0x2a);
                o.set_u8(ctx.b, 0x2a, drapeaux | 1);
                return ctx.reinsert();
            }
            let valeur = o.u16(ctx.b, 0x1c);
            let bit = ((numero as u8) << 1) as u32;
            if ctx.pris(valeur >> bit & 1 == 0) {
                return ctx.reinsert();
            }
            if ctx.pris(valeur >> (bit + 1) & 1 == 0) {
                return ctx.reinsert();
            }
            ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
            recopie_palette(ctx, o, 0x800);
            ctx.reinsert();
        }
        etat::LIBERE => ctx.free(&pools::FREE_EFFCT),
        _ => ctx.deleguer(cible),
    }
}

/// Fin de l'initialisation de la variante 2 (`0xaea0c`) : animation de la fenêtre.
fn interface_2_animation(ctx: &mut Ctx, o: Obj, d1: u16) {
    let numero = (o.u8(ctx.b, champ::MODE) as u32) << 1;
    let animation = ctx.prog16(interface::ANIMATIONS_2 + numero).wrapping_add(d1);
    ctx.anim_start(ANIMATIONS_INTERFACE, animation);
    ctx.reinsert();
}

fn interface_3(ctx: &mut Ctx, o: Obj) {
    if partie_finie(ctx) {
        return ctx.free(&pools::FREE_EFFCT);
    }
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(interface::ETATS_3, etat as u32);
    match etat {
        etat::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            place_sur_camera(ctx, o);
            decale_u16(ctx, o, Obj::X, 0x10);
            decale_u16(ctx, o, Obj::Z, 0xd8);
            interface_3_animation(ctx, o);
        }
        etat::ACTIF => {
            let (a, b) = (o.u8(ctx.b, 0x14), o.u8(ctx.b, 0x15));
            if !ctx.pris(a != b) {
                return ctx.reinsert();
            }
            interface_3_animation(ctx, o);
        }
        _ => ctx.deleguer(cible),
    }
}

/// `0xaebfe` : animation `0xe3 - +0x14` du jeu d'interface.
fn interface_3_animation(ctx: &mut Ctx, o: Obj) {
    let valeur = o.u8(ctx.b, 0x14);
    o.set_u8(ctx.b, 0x15, valeur);
    ctx.anim_start(ANIMATIONS_INTERFACE, 0xe3u8.wrapping_sub(valeur) as u16);
    ctx.reinsert();
}

fn interface_4(ctx: &mut Ctx, o: Obj) {
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(interface::ETATS_4, etat as u32);
    match etat {
        etat::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            place_sur_camera(ctx, o);
            let rang = (o.u8(ctx.b, champ::MODE) as u32) << 3;
            let dx = ctx.prog16(interface::DECALAGES_4 + rang);
            decale_u16(ctx, o, Obj::X, dx);
            let dz = ctx.prog16(interface::DECALAGES_4 + rang + 2);
            decale_u16(ctx, o, Obj::Z, dz);
            let dy = ctx.prog16(interface::DECALAGES_4 + rang + 4);
            decale_u16(ctx, o, 0x12, dy);
            let animation = ctx.prog16(interface::DECALAGES_4 + rang + 6);
            ctx.anim_start(ANIMATIONS_INTERFACE, animation);
            ctx.reinsert();
        }
        etat::ACTIF => {
            if partie_finie(ctx) {
                return ctx.free(&pools::FREE_EFFCT);
            }
            ctx.reinsert();
        }
        _ => ctx.deleguer(cible),
    }
}

fn interface_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let variante = o.u8(ctx.b, Obj::VARIANT);
    let cible = ctx.saut_table(interface::TABLE, 2 * variante as u32);
    match variante {
        0 => interface_0(ctx, o),
        1 => interface_1(ctx, o),
        2 => interface_2(ctx, o),
        3 => interface_3(ctx, o),
        4 => interface_4(ctx, o),
        _ => ctx.deleguer(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 29 : étincelle d'impact sur une cible (0xb1440)

/// Type 29 : étincelle collée à une cible (`+0x5c`, ennemi ou boss) à l'endroit d'impact tiré d'une
/// table selon le type de la cible ; elle joue une animation (tirée au hasard selon un drapeau `+0x42`
/// lu dans une table par type de cible) et disparaît à la fin de l'animation. Si la cible n'existe plus
/// ou ne convient pas, l'étincelle passe directement à l'état 6 (libération).
mod impact {
    pub const ENTREE: u32 = 0xb1440;
    pub const TABLE: u32 = 0xb144e;
    pub const ANIMATIONS: u32 = 0x1908d6;
    pub const HASARD_A: u32 = 0xb14a2;
    pub const HASARD_B: u32 = 0xb14b2;
    /// Drapeaux par type d'ennemi / de boss (deux octets : interdit, drapeau).
    pub const ENNEMIS: u32 = 0xb1544;
    pub const BOSS: u32 = 0xb1586;
    /// Décalages d'impact (dx, dz) par type de cible (4 octets), pour les ennemis puis pour le reste.
    pub const DECALAGES_ENNEMI: u32 = 0xb165a;
    pub const DECALAGES_AUTRE: u32 = 0xb16de;
    pub const CIBLE: u32 = 0x5c;
    pub const VARIANTE_HASARD: u32 = 0x42;
}

/// Pose l'état 6 (libération à l'image suivante).
fn impact_abandon(ctx: &mut Ctx, o: Obj) {
    o.set_u32(ctx.b, Obj::STATE, 0x0600_0000);
}

/// `0xb14dc` : lit dans les tables le drapeau de la cible, ou abandonne.
fn impact_drapeau(ctx: &mut Ctx, o: Obj) {
    let pointeur_cible = o.u16(ctx.b, impact::CIBLE);
    if ctx.pris(pointeur_cible == 0) {
        return impact_abandon(ctx, o);
    }
    let cible = pointeur(ctx, o, impact::CIBLE);
    let mot = cible.u16(ctx.b, 4);
    if ctx.pris(mot == 0x402) {
        return impact_abandon(ctx, o);
    }
    let classe = cible.u8(ctx.b, Obj::CLASS);
    let rang = (cible.u8(ctx.b, Obj::KIND) as u32) * 2;
    if !ctx.pris(classe != 2) {
        let interdit = ctx.prog8(impact::ENNEMIS + rang);
        if ctx.pris(interdit != 0) {
            return impact_abandon(ctx, o);
        }
        let drapeau = ctx.prog8(impact::ENNEMIS + rang + 1);
        return o.set_u8(ctx.b, impact::VARIANTE_HASARD, drapeau);
    }
    if ctx.pris(classe != 4) {
        return impact_abandon(ctx, o);
    }
    let interdit = ctx.prog8(impact::BOSS + rang);
    if ctx.pris(interdit != 0) {
        return impact_abandon(ctx, o);
    }
    let drapeau = ctx.prog8(impact::BOSS + rang + 1);
    o.set_u8(ctx.b, impact::VARIANTE_HASARD, drapeau);
}

/// `0xb15c8` (variante non nulle) : position de la cible décalée de `(+0x14, +0x16)`.
fn impact_position_decalee(ctx: &mut Ctx, o: Obj) {
    let pointeur_cible = o.u16(ctx.b, impact::CIBLE);
    if ctx.pris(pointeur_cible == 0) {
        return;
    }
    let cible = pointeur(ctx, o, impact::CIBLE);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = cible.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    let flip = cible.u8(ctx.b, champ::FLIP);
    o.set_u8(ctx.b, champ::FLIP, flip);
    let (dx, dz) = (o.u16(ctx.b, Obj::VX), o.u16(ctx.b, 0x16));
    decale_u16(ctx, o, Obj::X, dx);
    decale_u16(ctx, o, Obj::Z, dz);
}

/// `0xb15fe` (variante 0) : position de la cible décalée de l'impact tiré de la table.
fn impact_position_table(ctx: &mut Ctx, o: Obj) {
    let pointeur_cible = o.u16(ctx.b, impact::CIBLE);
    if ctx.pris(pointeur_cible == 0) {
        return;
    }
    let cible = pointeur(ctx, o, impact::CIBLE);
    for off in [Obj::X, Obj::Y, Obj::Z] {
        let v = cible.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    let flip = cible.u8(ctx.b, champ::FLIP);
    o.set_u8(ctx.b, champ::FLIP, flip);
    let classe = cible.u8(ctx.b, Obj::CLASS);
    let table = if ctx.pris(classe == 2) { impact::DECALAGES_ENNEMI } else { impact::DECALAGES_AUTRE };
    let rang = ((cible.u8(ctx.b, Obj::KIND) << 2) as u32) & 0xff;
    let mut dx = ctx.rom16(table + rang);
    let dz = ctx.rom16(table + rang + 2);
    let retourne = cible.u8(ctx.b, champ::FLIP) != 0;
    if !ctx.pris(!retourne) {
        dx = dx.wrapping_neg();
    }
    decale_u16(ctx, o, Obj::X, dx);
    decale_u16(ctx, o, Obj::Z, dz);
}

fn impact_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let variante = o.u8(ctx.b, Obj::VARIANT);
    if !ctx.pris(variante == 0) {
        impact_position_decalee(ctx, o);
        o.set_u8(ctx.b, impact::VARIANTE_HASARD, 0);
    } else {
        impact_position_table(ctx, o);
    }
    impact_drapeau(ctx, o);
    ctx.jsr(rom::RNG);
    let tirage = ctx.c.d[0] as u16 & 0xf;
    let drapeau = o.u8(ctx.b, impact::VARIANTE_HASARD);
    let table = if ctx.pris(drapeau == 0) { impact::HASARD_A } else { impact::HASARD_B };
    let animation = ctx.prog8(table + tirage as u32);
    ctx.anim_start(impact::ANIMATIONS, animation as u16);
}

fn impact_actif(ctx: &mut Ctx, o: Obj) {
    let fin = ctx.anim_step();
    if !ctx.pris(!fin) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
    ctx.reinsert();
}

fn impact_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(impact::TABLE, etat as u32);
    match etat {
        etat::INIT => impact_init(ctx, o),
        etat::ACTIF => impact_actif(ctx, o),
        etat::LIBERE | 6 => ctx.free(&pools::FREE_EFFCT),
        _ => ctx.deleguer(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 31 : éléments de l'écran de fin de niveau (0xb1a98)

/// Type 31 : objets de l'écran de décompte (11 variantes, posées sur la caméra du fond 2). Réécrits :
/// variante 3 (indicateur de rang, animation `0xe3 - +0x42`), variante 4 (plaque, animation `0x168` ou
/// `0x169` selon `A5+0x70`), variante 5 état 0 (lignes de score), variante 6 (compteurs `A5-0x5b21` /
/// `-0x5b20` en décimal sur 3 chiffres dessinés dans le fond 2, effacés quand l'objet atteint le bas
/// de l'écran). Le reste (variantes 0, 1, 2, 7 à 10, variante 5 état 2) reste en code d'origine.
mod decompte {
    pub const ENTREE: u32 = 0xb1a98;
    pub const TABLE: u32 = 0xb1aa8;
    pub const ETATS_3: u32 = 0xb1e58;
    pub const ETATS_4: u32 = 0xb1ec4;
    pub const ETATS_5: u32 = 0xb1f1a;
    pub const ETATS_6: u32 = 0xb2262;
    pub const DECALAGES_5: u32 = 0xb1f6a;
    pub const CHIFFRES: u32 = 0xb2334;
    pub const ANIMATIONS_6: u32 = 0xb2af0;
    pub const ANIMATIONS_6_B: u32 = 0xb2b6a;
    pub const FOND_CENTAINES: u32 = 0x910f6c;
    pub const FOND_UNITES: u32 = 0x910f74;
    pub const COMPTEURS: i32 = -0x5b21;
    pub const VALEUR: u32 = 0x42;
    pub const VALEUR_AFFICHEE: u32 = 0x43;
}

fn decompte_3(ctx: &mut Ctx, o: Obj) {
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(decompte::ETATS_3, etat as u32);
    match etat {
        etat::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            place_sur_camera(ctx, o);
            decale_u16(ctx, o, Obj::X, 0x13c);
            decale_u16(ctx, o, Obj::Z, 0xcc);
            let valeur = o.u8(ctx.b, decompte::VALEUR);
            o.set_u8(ctx.b, decompte::VALEUR_AFFICHEE, valeur);
            decompte_3_animation(ctx, valeur);
        }
        etat::ACTIF => {
            let valeur = o.u8(ctx.b, decompte::VALEUR);
            let affichee = o.u8(ctx.b, decompte::VALEUR_AFFICHEE);
            if ctx.pris(valeur == affichee) {
                return;
            }
            o.set_u8(ctx.b, decompte::VALEUR_AFFICHEE, valeur);
            decompte_3_animation(ctx, valeur);
        }
        etat::LIBERE => decompte_fin(ctx, o),
        _ => ctx.deleguer(cible),
    }
}

/// `0xb1e8c` : animation `0xe3 - valeur` puis tri.
fn decompte_3_animation(ctx: &mut Ctx, valeur: u8) {
    ctx.anim_start(ANIMATIONS_INTERFACE, 0xe3u16.wrapping_sub(valeur as u16));
    ctx.reinsert();
}

/// `0xb2916` : fin des variantes (état 4).
fn decompte_fin(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    ctx.free(&pools::FREE_EFFCT);
}

fn decompte_4(ctx: &mut Ctx, o: Obj) {
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(decompte::ETATS_4, etat as u32);
    match etat {
        etat::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            let cam = lit_ram16(ctx, 0x556);
            o.set_u16(ctx.b, Obj::X, cam);
            o.set_u32(ctx.b, Obj::Y, 0);
            o.set_u16(ctx.b, Obj::Z, 0x460);
            decale_u16(ctx, o, Obj::X, 0x80);
            let mut animation = 0x168;
            let mot = ctx.ram16(0x70);
            if !ctx.pris(mot == 0) {
                animation = 0x169;
            }
            ctx.anim_start(ANIMATIONS_INTERFACE, animation);
            ctx.reinsert();
        }
        etat::ACTIF => {}
        etat::LIBERE => decompte_fin(ctx, o),
        _ => ctx.deleguer(cible),
    }
}

fn decompte_5(ctx: &mut Ctx, o: Obj) {
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(decompte::ETATS_5, etat as u32);
    match etat {
        etat::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            let (cam_x, cam_y) = (lit_ram16(ctx, 0x556), lit_ram16(ctx, 0x55a));
            o.set_u16(ctx.b, Obj::Z, cam_y);
            o.set_u16(ctx.b, Obj::X, cam_x);
            o.set_u32(ctx.b, Obj::Y, 0);
            decale_u16(ctx, o, Obj::X, 0x20);
            let rang = (o.u8(ctx.b, decompte::VALEUR) as u32) << 1;
            let dz = ctx.prog16(decompte::DECALAGES_5 + rang);
            decale_u16(ctx, o, Obj::Z, dz);
            let mode = o.u8(ctx.b, champ::MODE) ^ 3;
            o.set_u8(ctx.b, champ::MODE, mode);
            ctx.anim_start(ANIMATIONS_INTERFACE, 0x16a);
            ctx.reinsert();
        }
        etat::LIBERE => decompte_fin(ctx, o),
        _ => ctx.deleguer(cible),
    }
}

/// `0xb2348` : valeur binaire (< 1000) en décimal codé binaire sur trois quartets.
fn en_bcd(ctx: &mut Ctx, valeur: u16) -> u16 {
    let (mut reste, mut bcd) = (valeur, 0u16);
    loop {
        reste = reste.wrapping_sub(0x64);
        if ctx.pris(reste as i16 <= -1) {
            break;
        }
        bcd += 0x100;
    }
    reste = reste.wrapping_add(0x64);
    loop {
        reste = reste.wrapping_sub(0xa);
        if ctx.pris(reste as i16 <= -1) {
            break;
        }
        bcd += 0x10;
    }
    reste.wrapping_add(0xa).wrapping_add(bcd)
}

/// `0xb22f2` : dessine un chiffre (quartet) à l'adresse `a1` du fond 2 (tuile 2 × 2) ; les zéros de tête
/// (`imprime` faux) font seulement avancer `a1`.
fn dessine_chiffre(ctx: &mut Ctx, a1: &mut u32, quartet: u16, attribut: u16, imprime: &mut bool) {
    let chiffre = quartet & 0xf;
    if ctx.pris(chiffre != 0) {
        *imprime = true;
    } else {
        let deja = *imprime;
        if !ctx.pris(deja) {
            *a1 += 0x100;
            return;
        }
    }
    let mut tuile = ctx.prog16(decompte::CHIFFRES + 2 * chiffre as u32);
    // 0xb230e : deux rangées de deux tuiles
    for _ in 0..2 {
        crate::mem::wr16(ctx.b, *a1, tuile);
        crate::mem::wr16(ctx.b, *a1 + 2, attribut);
        crate::mem::wr16(ctx.b, *a1 + 4, tuile.wrapping_add(0x10));
        crate::mem::wr16(ctx.b, *a1 + 6, attribut);
        *a1 += 8 + 0x78;
        tuile = tuile.wrapping_add(1);
    }
}

/// `0xb22d0` : dessine un nombre BCD de trois chiffres (et un quartet de tête nul).
fn dessine_nombre(ctx: &mut Ctx, mut a1: u32, bcd: u16, attribut: u16) {
    let mut imprime = false;
    for i in 0..4 {
        let tourne = bcd.rotate_left(4 * (i + 1));
        if i == 3 {
            imprime = true;
        }
        dessine_chiffre(ctx, &mut a1, tourne, attribut, &mut imprime);
    }
}

fn decompte_6_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let cam = lit_ram16(ctx, 0x556);
    o.set_u16(ctx.b, Obj::X, cam);
    o.set_u32(ctx.b, Obj::Y, 0);
    o.set_u16(ctx.b, Obj::Z, 0x448);
    decale_u16(ctx, o, Obj::X, 0x50);
    let mot = ctx.ram16(0x70);
    ctx.c.a[4] = if ctx.pris(mot == 0) { decompte::ANIMATIONS_6 } else { decompte::ANIMATIONS_6_B };
    ctx.natif(&crate::objects::anim::ANIM_START);
    ctx.reinsert();
    let centaines = ctx.ram8(decompte::COMPTEURS) as u16;
    let bcd = en_bcd(ctx, centaines);
    dessine_nombre(ctx, decompte::FOND_CENTAINES, bcd, 3);
    let unites = ctx.ram8(decompte::COMPTEURS + 1) as u16;
    let bcd = en_bcd(ctx, unites);
    dessine_nombre(ctx, decompte::FOND_UNITES, bcd, 3);
}

/// `0xb2392` : efface trois rangées de deux longs.
fn efface_nombre(ctx: &mut Ctx, mut a1: u32) {
    for _ in 0..3 {
        crate::mem::wr32(ctx.b, a1, 0x20_0000);
        crate::mem::wr32(ctx.b, a1 + 4, 0x20_0000);
        a1 += 8 + 0x78;
    }
}

fn decompte_6(ctx: &mut Ctx, o: Obj) {
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(decompte::ETATS_6, etat as u32);
    match etat {
        etat::INIT => decompte_6_init(ctx, o),
        etat::ACTIF => {
            let ecart = o.u16(ctx.b, Obj::Z).wrapping_sub(lit_ram16(ctx, 0x55a));
            if ctx.pris((ecart as i16) < 0x100) {
                return;
            }
            efface_nombre(ctx, decompte::FOND_CENTAINES);
            efface_nombre(ctx, decompte::FOND_UNITES);
        }
        etat::LIBERE => decompte_fin(ctx, o),
        _ => ctx.deleguer(cible),
    }
}

fn decompte_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let variante = o.u8(ctx.b, Obj::VARIANT);
    let cible = ctx.saut_table(decompte::TABLE, 2 * variante as u32);
    match variante {
        3 => decompte_3(ctx, o),
        4 => decompte_4(ctx, o),
        5 => decompte_5(ctx, o),
        6 => decompte_6(ctx, o),
        _ => ctx.deleguer(cible),
    }
}

gestionnaire!(EFFCT_31, decompte::ENTREE, "effct_decompte", decompte_gestionnaire);
gestionnaire!(EFFCT_29, impact::ENTREE, "effct_impact", impact_gestionnaire);
gestionnaire!(EFFCT_17, interface::ENTREE, "effct_interface", interface_gestionnaire);
gestionnaire!(EFFCT_5, halo::ENTREE, "effct_halo", halo);
gestionnaire!(EFFCT_9, trainee::ENTREE, "effct_trainee", trainee);
gestionnaire!(EFFCT_15, scintillement::ENTREE, "effct_scintillement", scintillement);
gestionnaire!(EFFCT_21, bandeau::ENTREE, "effct_bandeau", bandeau);
gestionnaire!(EFFCT_25, poussiere::ENTREE, "effct_poussiere", poussiere);
gestionnaire!(EFFCT_26, nuage::ENTREE, "effct_nuage", nuage);
gestionnaire!(EFFCT_32, aura::ENTREE, "effct_aura", aura);
gestionnaire!(EFFCT_33, eclat::ENTREE, "effct_eclat", eclat);
