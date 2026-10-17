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
    pub const ETATS_8: u32 = 0xb2812;
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

/// Variante 8 : ligne de l'écran de décompte propre au joueur `+0x2f` ; naît seulement si ce joueur est
/// présent, en vie et que ses deux compteurs `+0x118` / `+0x11c` valent -1 ; se libère quand le compteur
/// de scène `A5-0x55c7` atteint 10.
fn decompte_8(ctx: &mut Ctx, o: Obj) {
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(decompte::ETATS_8, etat as u32);
    match etat {
        etat::INIT => {
            let mode = o.u8(ctx.b, champ::MODE) as u32;
            for _ in 0..mode {
                ctx.pris(false); // boucle de recherche du joueur
            }
            ctx.pris(true);
            let joueur = Obj(JOUEURS + mode * PAS_JOUEUR);
            let absent = joueur.u8(ctx.b, Obj::ACTIVE) == 0;
            if ctx.pris(absent) {
                return decompte_fin(ctx, o);
            }
            let mort = joueur.u8(ctx.b, Obj::STATE) as i8 >= 4;
            if ctx.pris(mort) {
                return decompte_fin(ctx, o);
            }
            let compteur_a = joueur.u32(ctx.b, 0x118) != 0xffff_ffff;
            if ctx.pris(compteur_a) {
                return decompte_fin(ctx, o);
            }
            let compteur_b = joueur.u32(ctx.b, 0x11c) != 0xffff_ffff;
            if ctx.pris(compteur_b) {
                return decompte_fin(ctx, o);
            }
            ajoute_u8(ctx, o, Obj::STATE, 2);
            let (cam_x, cam_y) = (lit_ram16(ctx, 0x506), lit_ram16(ctx, 0x50a));
            o.set_u16(ctx.b, Obj::X, cam_x);
            o.set_u16(ctx.b, Obj::Z, cam_y);
            o.set_u32(ctx.b, Obj::Y, 0);
            decale_u16(ctx, o, Obj::X, 0x10);
            decale_u16(ctx, o, Obj::Z, 0xb8);
            decale_u16(ctx, o, Obj::Z, ((mode as u16) << 5).wrapping_neg());
            ctx.anim_start(ANIMATIONS_INTERFACE, 0x173 + mode as u16);
            ctx.reinsert();
        }
        etat::ACTIF => {
            let compteur = ctx.ram8(-0x55c7) as i8;
            if ctx.pris(compteur >= 10) {
                decompte_fin(ctx, o);
            }
        }
        etat::LIBERE => decompte_fin(ctx, o),
        6 => ctx.free(&pools::FREE_EFFCT),
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
        8 => decompte_8(ctx, o),
        _ => ctx.deleguer(cible),
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
    pub const ENTREE: u32 = 0xab7fa;
    pub const TABLE: u32 = 0xab808;
    /// Nombre de frères par variante (espace programme) ; fiches de la ROM (données).
    pub const NOMBRES: u32 = 0xab95e;
    pub const FICHES: u32 = 0xab8a2;
    pub const ANIMATIONS: u32 = 0x18e664;
    pub const ENFANT: u32 = 0x42;
    pub const CLIGNOTEMENT: u32 = 0x53;
}

/// `0xab7d8` : crée un débris (type 6, variante `variante`) rattaché à l'objet courant.
fn cree_debris(ctx: &mut Ctx, o: Obj, variante: u8) -> Option<Obj> {
    let nouveau = ctx.alloc(&pools::ALLOC_EFFCT);
    // deux `bcs` successifs : dans `0xab7d8`, puis chez l'appelant (la retenue est restée levée)
    ctx.pris(nouveau.is_none());
    ctx.pris(nouveau.is_none());
    let n = nouveau?;
    n.set_u8(ctx.b, Obj::ACTIVE, 1);
    n.set_u8(ctx.b, Obj::KIND, 6);
    n.set_u8(ctx.b, Obj::VARIANT, variante);
    n.set_u8(ctx.b, debris::ENFANT, 0);
    n.set_u16(ctx.b, champ::PROPRIETAIRE, o.0 as u16);
    Some(n)
}

/// Premier débris : crée ses frères, puis passe à l'initialisation commune.
fn debris_parent(ctx: &mut Ctx, o: Obj) {
    o.set_u8(ctx.b, debris::ENFANT, 1);
    let variante = o.u8(ctx.b, Obj::VARIANT);
    let nombre = ctx.prog8(debris::NOMBRES + variante as u32);
    o.set_u8(ctx.b, champ::MODE, nombre.wrapping_add(1));
    let proprietaire = o.u16(ctx.b, champ::PROPRIETAIRE);
    for restant in (0..=nombre as u16).rev() {
        let Some(frere) = cree_debris(ctx, o, variante) else {
            break;
        };
        frere.set_u16(ctx.b, champ::PROPRIETAIRE, proprietaire);
        frere.set_u8(ctx.b, debris::ENFANT, 1);
        frere.set_u8(ctx.b, champ::MODE, restant as u8);
        ctx.pris(restant > 0); // dbf
    }
    debris_init(ctx, o);
}

fn debris_init(ctx: &mut Ctx, o: Obj) {
    let enfant = o.u8(ctx.b, debris::ENFANT);
    if ctx.pris(enfant == 0) {
        return debris_parent(ctx, o);
    }
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let proprietaire = pointeur(ctx, o, champ::PROPRIETAIRE);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = proprietaire.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    o.set_u8(ctx.b, debris::CLIGNOTEMENT, 0);
    o.set_u16(ctx.b, 0x16, 0);
    let variante = o.u8(ctx.b, Obj::VARIANT) as u32;
    let decalage = ctx.rom16(debris::FICHES + 2 * variante) as i16 as i32 as u32;
    let mode = (o.u8(ctx.b, champ::MODE) as u32) << 4;
    let mut fiche = debris::FICHES.wrapping_add(decalage).wrapping_add(mode);
    let vx = ctx.rom16(fiche);
    o.set_u16(ctx.b, Obj::VX, vx);
    let vz_az = ctx.rom32(fiche + 2);
    o.set_u32(ctx.b, Obj::VZ, vz_az);
    let flip = ctx.rom8(fiche + 6);
    o.set_u8(ctx.b, champ::FLIP, flip);
    let animation = ctx.rom8(fiche + 7);
    let (mut dx, dz, dy) = (ctx.rom16(fiche + 8), ctx.rom16(fiche + 10), ctx.rom16(fiche + 12));
    fiche += 14;
    let _ = fiche;
    let retourne = proprietaire.u8(ctx.b, champ::FLIP) != 0;
    if !ctx.pris(!retourne) {
        let flip = o.u8(ctx.b, champ::FLIP) ^ 0x20;
        o.set_u8(ctx.b, champ::FLIP, flip);
        let vx = o.u16(ctx.b, Obj::VX).wrapping_neg();
        o.set_u16(ctx.b, Obj::VX, vx);
        dx = dx.wrapping_neg();
        o.set_u8(ctx.b, debris::CLIGNOTEMENT, 1);
    }
    decale_u16(ctx, o, Obj::X, dx);
    decale_u16(ctx, o, Obj::Z, dz);
    decale_u16(ctx, o, 0x12, dy);
    ctx.anim_start(debris::ANIMATIONS, animation as u16);
}

fn debris_actif(ctx: &mut Ctx, o: Obj) {
    ctx.jsr(rom::MOVE_XZ);
    ctx.anim_step();
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
    let clignote = o.u8(ctx.b, debris::CLIGNOTEMENT) ^ 1;
    o.set_u8(ctx.b, debris::CLIGNOTEMENT, clignote);
    if !ctx.pris(clignote == 0) {
        let flags = o.u8(ctx.b, Obj::FLAGS);
        o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    }
    let z = o.u16(ctx.b, Obj::Z);
    if !ctx.pris(z as i16 > 0) {
        o.set_u32(ctx.b, Obj::Z, 0);
        o.set_u8(ctx.b, debris::CLIGNOTEMENT, 6);
        ajoute_u8(ctx, o, Obj::STATE, 2);
        let variante = o.u8(ctx.b, Obj::VARIANT);
        if !ctx.pris(variante == 0) {
            ajoute_u8(ctx, o, debris::CLIGNOTEMENT, 0xff);
        }
    }
    ctx.reinsert();
}

fn debris_fin(ctx: &mut Ctx, o: Obj) {
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
    let reste = o.u8(ctx.b, debris::CLIGNOTEMENT).wrapping_sub(1);
    o.set_u8(ctx.b, debris::CLIGNOTEMENT, reste);
    if ctx.pris(reste == 0) {
        return ctx.free(&pools::FREE_EFFCT);
    }
    if !ctx.pris(reste & 1 == 0) {
        let flags = o.u8(ctx.b, Obj::FLAGS);
        o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    }
    ctx.reinsert();
}

fn debris_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(debris::TABLE, etat as u32);
    match etat {
        etat::INIT => debris_init(ctx, o),
        etat::ACTIF => debris_actif(ctx, o),
        etat::LIBERE => debris_fin(ctx, o),
        6 => ctx.free(&pools::FREE_EFFCT),
        _ => ctx.deleguer(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 1 : effet supprimé (0xa8758)

/// Type 1 : emplacement de table inutilisé dans cette version du jeu : le gestionnaire libère
/// l'objet aussitôt (`bra` vers `jmp 0x2218`).
fn supprime(ctx: &mut Ctx) {
    ctx.free(&pools::FREE_EFFCT);
}

// ---------------------------------------------------------------------------------------------
// Type 7 : éclat à animation unique (0xaba4e)

/// Type 7 : joue l'animation (tirée selon le mode `+0x2f`) ; à la fin de l'animation il monte de deux
/// unités en profondeur puis clignote 0x1e images avant de disparaître. Non exercé.
mod eclat_unique {
    pub const ENTREE: u32 = 0xaba4e;
    pub const TABLE: u32 = 0xaba5c;
    pub const ANIMATIONS_MODE: u32 = 0xaba84;
    pub const COMPTE: u32 = 0x53;
}

fn eclat_unique_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let mode = o.u8(ctx.b, champ::MODE) as u32;
    let animation = ctx.prog8(eclat_unique::ANIMATIONS_MODE + mode);
    ctx.anim_start(0x1bf8fe, animation as u16);
    ctx.reinsert();
}

fn eclat_unique_actif(ctx: &mut Ctx, o: Obj) {
    let fin = o.u8(ctx.b, 0x32);
    if !ctx.pris(fin == 0) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, eclat_unique::COMPTE, 0x1e);
        let y = o.u32(ctx.b, Obj::Y).wrapping_add(2);
        o.set_u32(ctx.b, Obj::Y, y);
    }
    ctx.jsr(rom::ANIM_STEP);
}

fn eclat_unique_clignote(ctx: &mut Ctx, o: Obj) {
    let reste = o.u8(ctx.b, eclat_unique::COMPTE).wrapping_sub(1);
    o.set_u8(ctx.b, eclat_unique::COMPTE, reste);
    if ctx.pris(reste == 0) {
        return ctx.free(&pools::FREE_EFFCT);
    }
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags ^ 2);
}

fn eclat_unique_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(eclat_unique::TABLE, etat as u32);
    match etat {
        etat::INIT => eclat_unique_init(ctx, o),
        etat::ACTIF => eclat_unique_actif(ctx, o),
        etat::LIBERE => eclat_unique_clignote(ctx, o),
        _ => ctx.deleguer(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 19 : balayage d'écran (0xaecce)

/// Type 19 : objet qui entre par la gauche de l'écran (x = -0x80, y = 0x150), attend 0x70 images en
/// avançant de deux pixels par image, puis ralentit (accélération -0x10) jusqu'à repartir en arrière.
/// Non exercé.
mod balayage {
    pub const ENTREE: u32 = 0xaecce;
    pub const TABLE: u32 = 0xaecdc;
    pub const DELAI: u32 = 0x54;
}

fn balayage_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, balayage::DELAI, 0x70);
    o.set_u16(ctx.b, Obj::X, 0xff80);
    o.set_u16(ctx.b, Obj::Y, 0x150);
    o.set_u16(ctx.b, Obj::VX, 0x200);
    o.set_u16(ctx.b, Obj::AX, 0xfff0);
    ctx.anim_start(ANIMATIONS_INTERFACE, 0);
}

fn balayage_actif(ctx: &mut Ctx, o: Obj) {
    let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
    if !ctx.pris(sous_etat != 0) {
        let delai = o.u8(ctx.b, balayage::DELAI);
        if ctx.pris(delai != 0) {
            o.set_u8(ctx.b, balayage::DELAI, delai - 1);
            decale_u16(ctx, o, Obj::X, 2);
        } else {
            ctx.jsr(rom::MOVE_X);
            let vx = o.u16(ctx.b, Obj::VX);
            if !ctx.pris(vx as i16 >= 0) {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 1);
            }
        }
    }
    ctx.jsr(rom::ANIM_STEP);
    ctx.reinsert();
}

fn balayage_gestionnaire(ctx: &mut Ctx) {
    trois_etats(ctx, balayage::TABLE, balayage_init, balayage_actif)
}

// ---------------------------------------------------------------------------------------------
// Type 27 : effet de ramassage qui monte (0xb12e6)

/// Type 27 : effet posé sur l'objet propriétaire (`+0x50`, créé par `0xb12c0` : ramassage) qui monte de 0x8000 (un demi
/// pixel) par image pendant 0x1c images. Non exercé.
mod fumee {
    pub const ENTREE: u32 = 0xb12e6;
    pub const TABLE: u32 = 0xb12f4;
    pub const ANIMATIONS: u32 = 0x19046c;
    pub const DUREE: u32 = 0x57;
}

fn fumee_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, fumee::DUREE, 0x1c);
    let proprietaire = pointeur(ctx, o, champ::PROPRIETAIRE);
    let x = proprietaire.u16(ctx.b, Obj::X);
    o.set_u16(ctx.b, Obj::X, x);
    let y = proprietaire.u16(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u16(ctx.b, Obj::Y, y);
    o.set_u16(ctx.b, Obj::Z, 0x10);
    let variante = o.u8(ctx.b, Obj::VARIANT);
    ctx.anim_start(fumee::ANIMATIONS, variante as u16);
    ctx.reinsert();
}

fn fumee_actif(ctx: &mut Ctx, o: Obj) {
    let reste = o.u8(ctx.b, fumee::DUREE).wrapping_sub(1);
    o.set_u8(ctx.b, fumee::DUREE, reste);
    if !ctx.pris(reste != 0) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
    let z = o.u32(ctx.b, Obj::Z).wrapping_add(0x8000);
    o.set_u32(ctx.b, Obj::Z, z);
}

fn fumee_gestionnaire(ctx: &mut Ctx) {
    trois_etats(ctx, fumee::TABLE, fumee_init, fumee_actif)
}

// ---------------------------------------------------------------------------------------------
// Type 28 : étincelle posée sur un objet (0xb136e)

/// Type 28 : étincelle collée à un objet (`+0x5c`), 0x20 au-dessus de lui ; l'animation dépend de la
/// variante et de `A5+0x70`. À la fin de l'animation, elle prévient son objet (`+0x77` ← 1, sauf
/// variante 6) puis disparaît. Non exercé.
mod etincelle {
    pub const ENTREE: u32 = 0xb136e;
    pub const TABLE: u32 = 0xb137c;
    pub const ANIMATIONS: u32 = 0x190534;
    pub const PORTEUR: u32 = 0x5c;
}

fn etincelle_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let porteur = pointeur(ctx, o, etincelle::PORTEUR);
    copie_position(ctx, o, porteur);
    let y = o.u16(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u16(ctx.b, Obj::Y, y);
    decale_u16(ctx, o, Obj::Z, 0x20);
    let variante = o.u8(ctx.b, Obj::VARIANT) as u16;
    let mot = ctx.ram16(0x70);
    let animation = variante.wrapping_add((mot & 0xf) >> 1 << 3);
    ctx.anim_start(etincelle::ANIMATIONS, animation);
    ctx.reinsert();
}

fn etincelle_actif(ctx: &mut Ctx, o: Obj) {
    let fin = ctx.anim_step();
    if ctx.pris(!fin) {
        return;
    }
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let variante = o.u8(ctx.b, Obj::VARIANT);
    if ctx.pris(variante == 6) {
        return;
    }
    let porteur = pointeur(ctx, o, etincelle::PORTEUR);
    porteur.set_u8(ctx.b, 0x77, 1);
}

fn etincelle_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(etincelle::TABLE, etat as u32);
    match etat {
        etat::INIT => etincelle_init(ctx, o),
        etat::ACTIF => etincelle_actif(ctx, o),
        etat::LIBERE => ajoute_u8(ctx, o, Obj::STATE, 2),
        6 => ctx.free(&pools::FREE_EFFCT),
        _ => ctx.deleguer(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 57 : marque attachée à un joueur (0xb6bca)

/// Type 57 : marque posée 0x18 à droite et 0x2d au-dessus du propriétaire (`+0x50`) tant qu'il reste
/// dans l'état `0x0204` / `+6 = 0x2c` ; sinon elle disparaît. Non exercé.
mod marque {
    pub const ENTREE: u32 = 0xb6bca;
    pub const TABLE: u32 = 0xb6bd8;
    pub const ANIMATIONS: u32 = 0x196b62;
}

fn marque_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let proprietaire = pointeur(ctx, o, champ::PROPRIETAIRE);
    for off in [Obj::X, Obj::Z] {
        let v = proprietaire.u16(ctx.b, off);
        o.set_u16(ctx.b, off, v);
    }
    let y = proprietaire.u32(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u32(ctx.b, Obj::Y, y);
    decale_u16(ctx, o, Obj::X, 0x18);
    decale_u16(ctx, o, Obj::Z, 0x2d);
    ctx.anim_start(marque::ANIMATIONS, 0);
    ctx.reinsert();
}

fn marque_actif(ctx: &mut Ctx, o: Obj) {
    let proprietaire = pointeur(ctx, o, champ::PROPRIETAIRE);
    let etat = proprietaire.u16(ctx.b, 4);
    if ctx.pris(etat != 0x204) {
        return ctx.free(&pools::FREE_EFFCT);
    }
    let sous = proprietaire.u8(ctx.b, 6);
    if ctx.pris(sous != 0x2c) {
        return ctx.free(&pools::FREE_EFFCT);
    }
    ctx.jsr(rom::ANIM_STEP);
}

fn marque_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(marque::TABLE, etat as u32);
    match etat {
        etat::INIT => marque_init(ctx, o),
        etat::ACTIF => marque_actif(ctx, o),
        etat::LIBERE => ctx.free(&pools::FREE_EFFCT),
        _ => ctx.deleguer(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 62 : gerbe d'éclats (0xb77ac)

/// Type 62 : reprend variante et mode de l'objet qu'il suit (`+0x5c`), se place 0x48 à gauche de lui et
/// attend qu'il soit touché (`+0x33` non nul) ; il explose alors en neuf objets EFFCT 0x40 (et un dixième
/// décrit par le mode) puis disparaît. Non exercé.
mod gerbe {
    pub const ENTREE: u32 = 0xb77ac;
    pub const TABLE: u32 = 0xb77ba;
    pub const ANIMATIONS: u32 = 0x197a5c;
    pub const PORTEUR: u32 = 0x5c;
}

fn gerbe_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let porteur = pointeur(ctx, o, gerbe::PORTEUR);
    let variante = porteur.u8(ctx.b, Obj::VARIANT);
    o.set_u8(ctx.b, Obj::VARIANT, variante);
    let mode = porteur.u8(ctx.b, champ::MODE);
    o.set_u8(ctx.b, champ::MODE, mode);
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
fn gerbe_explose(ctx: &mut Ctx, o: Obj) {
    let mut dernier = None;
    for restant in (0..=8u32).rev() {
        let nouveau = ctx.alloc(&pools::ALLOC_EFFCT);
        if ctx.pris(nouveau.is_none()) {
            return ctx.free(&pools::FREE_EFFCT);
        }
        let n = nouveau.unwrap();
        n.set_u8(ctx.b, Obj::ACTIVE, 1);
        n.set_u8(ctx.b, Obj::KIND, 0x40);
        for off in [Obj::X, Obj::Y] {
            let v = o.u16(ctx.b, off);
            n.set_u16(ctx.b, off, v);
        }
        dernier = Some(n);
        ctx.pris(restant > 0); // dbf
    }
    let mode = o.u8(ctx.b, champ::MODE);
    if !ctx.pris(mode == 0) {
        // le dernier objet créé est repris pour porter le mode
        let n = dernier.unwrap();
        n.set_u8(ctx.b, Obj::ACTIVE, 1);
        n.set_u8(ctx.b, Obj::KIND, 0x40);
        n.set_u8(ctx.b, Obj::VARIANT, 1);
        let variante = o.u8(ctx.b, Obj::VARIANT);
        n.set_u8(ctx.b, champ::MODE, variante);
        for off in [Obj::X, Obj::Y] {
            let v = o.u16(ctx.b, off);
            n.set_u16(ctx.b, off, v);
        }
    }
    ctx.free(&pools::FREE_EFFCT);
}

fn gerbe_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(gerbe::TABLE, etat as u32);
    match etat {
        etat::INIT => gerbe_init(ctx, o),
        etat::ACTIF => {
            let porteur = pointeur(ctx, o, gerbe::PORTEUR);
            let touche = porteur.u8(ctx.b, 0x33);
            if ctx.pris(touche == 0) {
                return;
            }
            gerbe_explose(ctx, o);
        }
        etat::LIBERE => gerbe_explose(ctx, o),
        _ => ctx.deleguer(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 56 : éclats de débris (0xb6c6e)

/// Type 56 : éclat lancé en l'air depuis sa position (dispersion aléatoire de ±128 en X et ±32 en
/// profondeur, vitesses tirées dans deux tables, animation 0x164 à 0x167) ; disparaît quand il
/// retombe sous le sol. Non exercé.
mod eclats_debris {
    pub const ENTREE: u32 = 0xb6c6e;
    pub const TABLE: u32 = 0xb6c7c;
    pub const VITESSES_X: u32 = 0xb6d18;
    pub const VITESSES_Y: u32 = 0xb6d28;
}

/// Tirage aléatoire masqué.
fn tirage(ctx: &mut Ctx, masque: u16) -> u16 {
    ctx.jsr(rom::RNG);
    ctx.c.d[0] as u16 & masque
}

fn eclats_debris_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u16(ctx.b, 0x42, 0x40);
    o.set_u8(ctx.b, 0x2a, 1);
    o.set_u8(ctx.b, 0x2b, 0x1d);
    let flip = tirage(ctx, 0x20);
    o.set_u8(ctx.b, champ::FLIP, flip as u8);
    let dx = tirage(ctx, 0xff).wrapping_sub(0x80);
    decale_u16(ctx, o, Obj::X, dx);
    let dy = tirage(ctx, 0x3f).wrapping_sub(0x20);
    decale_u16(ctx, o, Obj::Y, dy);
    o.set_u16(ctx.b, Obj::VZ, 0x400);
    o.set_u16(ctx.b, Obj::AZ, 0xffc0);
    let r = tirage(ctx, 0x1c) as u32;
    let vx = ctx.prog16(eclats_debris::VITESSES_X + r);
    o.set_u16(ctx.b, Obj::VX, vx);
    let r = tirage(ctx, 0x1c) as u32;
    let vy = ctx.prog16(eclats_debris::VITESSES_Y + r);
    o.set_u16(ctx.b, Obj::VY, vy);
    let animation = tirage(ctx, 3).wrapping_add(0x164);
    ctx.anim_start(ANIMATIONS_INTERFACE, animation);
    ctx.reinsert();
}

fn eclats_debris_actif(ctx: &mut Ctx, _o: Obj) {
    ctx.jsr(rom::MOVE_XYZ);
    let positif = ctx.c.cond(10); // bpl
    if ctx.pris(positif) {
        return;
    }
    ctx.free(&pools::FREE_EFFCT);
}

fn eclats_debris_fin(ctx: &mut Ctx, _o: Obj) {
    ctx.free(&pools::FREE_EFFCT);
}

fn eclats_debris_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(eclats_debris::TABLE, etat as u32);
    match etat {
        etat::INIT => eclats_debris_init(ctx, o),
        etat::ACTIF => eclats_debris_actif(ctx, o),
        etat::LIBERE => eclats_debris_fin(ctx, o),
        _ => ctx.deleguer(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 51 : projectile lancé d'un objet (0xb64c8)

/// Type 51 : naît sur l'objet `+0x5c` (si cet objet disparaît, il passe à l'état 6), décalé de 0x84
/// pixels vers l'avant et 0x16 en profondeur, puis part en arc (vitesses 0x140 et 0x3c0, gravité 0x38)
/// jusqu'à toucher le sol. Non exercé.
mod projectile {
    pub const ENTREE: u32 = 0xb64c8;
    pub const TABLE: u32 = 0xb64de;
    pub const SOUS_ETATS: u32 = 0xb6572;
    pub const ANIMATIONS: u32 = 0x1733ac;
    pub const CIBLE: u32 = 0x5c;
}

fn projectile_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let pointeur_cible = o.u16(ctx.b, projectile::CIBLE);
    if ctx.pris(pointeur_cible == 0) {
        return;
    }
    let cible = pointeur(ctx, o, projectile::CIBLE);
    for off in [Obj::X, Obj::Y, Obj::Z] {
        let v = cible.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    let flip = cible.u8(ctx.b, champ::FLIP);
    o.set_u8(ctx.b, champ::FLIP, flip);
    decale_u16(ctx, o, Obj::Y, 1);
    let (mut dx, dy) = (0x84u16, 0xffeau16);
    o.set_u16(ctx.b, Obj::VX, 0x140);
    o.set_u16(ctx.b, Obj::VZ, 0x3c0);
    o.set_u16(ctx.b, Obj::AZ, 0xffc8);
    if !ctx.pris(flip == 0) {
        dx = dx.wrapping_neg();
        let v = o.u16(ctx.b, Obj::VX).wrapping_neg();
        o.set_u16(ctx.b, Obj::VX, v);
    }
    decale_u16(ctx, o, Obj::X, dx);
    decale_u16(ctx, o, Obj::Y, dy);
    let orientation = o.u8(ctx.b, champ::FLIP) ^ 0x20;
    o.set_u8(ctx.b, champ::FLIP, orientation);
    ctx.anim_start(projectile::ANIMATIONS, 0x19);
    ctx.reinsert();
}

fn projectile_actif(ctx: &mut Ctx, o: Obj) {
    let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
    let cible = ctx.saut_table(projectile::SOUS_ETATS, sous_etat as u32);
    match sous_etat {
        0 => {
            ctx.jsr(rom::MOVE_XZ);
            let en_vol = ctx.gt();
            if !ctx.pris(en_vol) {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
                o.set_u16(ctx.b, Obj::Z, 0);
            }
            ctx.jsr(rom::ANIM_STEP);
        }
        2 => {}
        _ => ctx.deleguer(cible),
    }
}

fn projectile_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let pointeur_cible = o.u16(ctx.b, projectile::CIBLE);
    if ctx.pris(pointeur_cible == 0) {
        o.set_u32(ctx.b, Obj::STATE, 0x0600_0000);
        return;
    }
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(projectile::TABLE, etat as u32);
    match etat {
        etat::INIT => projectile_init(ctx, o),
        etat::ACTIF => projectile_actif(ctx, o),
        etat::LIBERE | 6 => ctx.free(&pools::FREE_EFFCT),
        _ => ctx.deleguer(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 52 : scintillement d'un personnage (0xb65e0)

/// Type 52 : variante du scintillement (type 15) qui guette seulement les joueurs de la classe 5 encore
/// en vie (points de vie `+0x62` non nuls) ; une fois déclenché il clignote huit fois, caché tant que
/// son objet (`+0x5c`) l'est. Non exercé.
mod scintille_classe {
    pub const ENTREE: u32 = 0xb65e0;
    pub const TABLE: u32 = 0xb65ee;
    pub const ANIMATIONS: u32 = 0x1bc77c;
    pub const REPETITIONS: u32 = 0x57;
    pub const DELAI: u32 = 0x58;
    pub const BASCULE: u32 = 0x5a;
    pub const DECLENCHE: u32 = 0x5b;
    pub const PORTEUR: u32 = 0x5c;
}

fn scintille_classe_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    o.set_u8(ctx.b, scintille_classe::REPETITIONS, 8);
    let t = tirage(ctx, 0x1f);
    o.set_u16(ctx.b, scintille_classe::DELAI, t);
    ctx.anim_start(scintille_classe::ANIMATIONS, 0);
    ctx.reinsert();
}

/// `0xb668a` : un joueur de la classe 5 en vie est-il passé près de l'objet ?
fn scintille_classe_joueur(ctx: &mut Ctx, o: Obj) {
    for i in 0..4u32 {
        let joueur = Obj(JOUEURS + i * PAS_JOUEUR);
        let absent = joueur.u8(ctx.b, Obj::ACTIVE) == 0;
        if !ctx.pris(absent) {
            let mort = joueur.u16(ctx.b, 0x62) == 0;
            if !ctx.pris(mort) {
                let classe = joueur.u8(ctx.b, 3) >> 1;
                if !ctx.pris(classe != 5) {
                    let dy = o.u16(ctx.b, Obj::Y).wrapping_sub(joueur.u16(ctx.b, Obj::Y)).wrapping_add(0x20);
                    if !ctx.pris(dy > 0x40) {
                        let dx = o.u16(ctx.b, Obj::X).wrapping_sub(joueur.u16(ctx.b, Obj::X)).wrapping_add(0x60);
                        if !ctx.pris(dx > 0xc0) {
                            o.set_u8(ctx.b, scintille_classe::DECLENCHE, 1);
                            return;
                        }
                    }
                }
            }
        }
        ctx.pris(i < 3); // dbf
    }
}

fn scintille_classe_actif(ctx: &mut Ctx, o: Obj) {
    let porteur = pointeur(ctx, o, scintille_classe::PORTEUR);
    let etat_porteur = porteur.u16(ctx.b, 4);
    if !ctx.pris(etat_porteur == 0x200) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
    let declenche = o.u8(ctx.b, scintille_classe::DECLENCHE);
    if !ctx.pris(declenche != 0) {
        return scintille_classe_joueur(ctx, o);
    }
    let cache = porteur.u8(ctx.b, Obj::HIDDEN);
    if !ctx.pris(cache == 0) {
        let flags = o.u8(ctx.b, Obj::FLAGS);
        o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
        return;
    }
    let reste = o.u16(ctx.b, scintille_classe::DELAI).wrapping_sub(1);
    o.set_u16(ctx.b, scintille_classe::DELAI, reste);
    if ctx.pris(reste != 0) {
        return;
    }
    o.set_u16(ctx.b, scintille_classe::DELAI, 0x30);
    let bascule = o.u8(ctx.b, scintille_classe::BASCULE) ^ 1;
    o.set_u8(ctx.b, scintille_classe::BASCULE, bascule);
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
    if ctx.pris(bascule != 0) {
        return;
    }
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    let reste = o.u8(ctx.b, scintille_classe::REPETITIONS).wrapping_sub(1);
    o.set_u8(ctx.b, scintille_classe::REPETITIONS, reste);
    if ctx.pris(reste != 0) {
        return;
    }
    o.set_u32(ctx.b, Obj::STATE, 0x0400_0000);
}

fn scintille_classe_gestionnaire(ctx: &mut Ctx) {
    trois_etats(ctx, scintille_classe::TABLE, scintille_classe_init, scintille_classe_actif)
}

// ---------------------------------------------------------------------------------------------
// Type 24 : accessoire de personnage (0xb0ff2)

/// Type 24 : accessoire collé à son propriétaire (`+0x50`) à un décalage (dx, dy, dz) lu selon la
/// variante ; actif seulement quand la scène n'est pas figée (`A5+0x3d`). Si le propriétaire a disparu
/// il passe à l'état 6. Non exercé.
mod accessoire {
    pub const ENTREE: u32 = 0xb0ff2;
    pub const TABLE: u32 = 0xb1000;
    pub const DECALAGES: u32 = 0xb10c0;
    pub const ANIMATIONS: u32 = 0x16e752;
    pub const FIN_ANIMATION: u32 = 0x43;
}

/// `0xb1052` : recolle l'objet au propriétaire.
fn accessoire_suit(ctx: &mut Ctx, o: Obj) {
    let pointeur_proprietaire = o.u16(ctx.b, champ::PROPRIETAIRE);
    if !ctx.pris(pointeur_proprietaire != 0) {
        o.set_u32(ctx.b, Obj::STATE, 0x0600_0000);
        return;
    }
    let proprietaire = pointeur(ctx, o, champ::PROPRIETAIRE);
    for off in [Obj::X, Obj::Y, Obj::Z] {
        let v = proprietaire.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    let flip = proprietaire.u8(ctx.b, champ::FLIP);
    o.set_u8(ctx.b, champ::FLIP, flip);
    let variante = o.u8(ctx.b, Obj::VARIANT);
    let rang = (variante << 3) as u32; // décalage sur un octet, comme l'original
    let mut dx = ctx.prog16(accessoire::DECALAGES + rang);
    let dy = ctx.prog16(accessoire::DECALAGES + rang + 2);
    let dprofondeur = ctx.prog16(accessoire::DECALAGES + rang + 4) as i16 as i32;
    let dz = ctx.prog16(accessoire::DECALAGES + rang + 6);
    if !ctx.pris(flip == 0) {
        dx = dx.wrapping_neg();
    }
    decale_u16(ctx, o, Obj::X, dx);
    decale_u16(ctx, o, Obj::Y, dy);
    let y = o.u32(ctx.b, Obj::Y).wrapping_add(dprofondeur as u32);
    o.set_u32(ctx.b, Obj::Y, y);
    decale_u16(ctx, o, Obj::Z, dz);
}

fn accessoire_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(accessoire::TABLE, etat as u32);
    match etat {
        etat::INIT => {
            let figee = ctx.ram8(0x3d) != 0;
            if ctx.pris(figee) {
                return;
            }
            ajoute_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, accessoire::FIN_ANIMATION, 0);
            accessoire_suit(ctx, o);
            ctx.anim_start(accessoire::ANIMATIONS, 3);
        }
        etat::ACTIF => {
            let figee = ctx.ram8(0x3d) != 0;
            if ctx.pris(figee) {
                return;
            }
            let fin = o.u8(ctx.b, accessoire::FIN_ANIMATION);
            if !ctx.pris(fin == 0) {
                ajoute_u8(ctx, o, Obj::STATE, 2);
            }
            accessoire_suit(ctx, o);
            ctx.jsr(rom::ANIM_STEP);
            ctx.reinsert();
        }
        etat::LIBERE => ctx.free(&pools::FREE_EFFCT),
        _ => ctx.deleguer(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 45 : déclencheur de proximité (0xb5340)

/// Type 45 : objet posé dans le décor qui attend que le joueur principal (`A5+0x46`) passe sur lui : il
/// donne alors un événement à chaque joueur présent (`0x19b08`, décalé par joueur) puis disparaît ; il
/// disparaît aussi s'il reste en arrière de la caméra. Non exercé.
mod declencheur {
    pub const ENTREE: u32 = 0xb5340;
    pub const TABLE: u32 = 0xb534e;
    pub const ANIMATIONS: u32 = 0x1bf5c2;
    pub const PRINCIPAL: i32 = 0x46;
    pub const EFFET: u32 = 0xea50c;
    pub const EVENEMENT: u32 = 0x19b08;
    pub const DECALAGES: u32 = 0xb53ec;
}

/// `0xb53b0` : déclenchement.
fn declencheur_effet(ctx: &mut Ctx, o: Obj) {
    ctx.jsr(declencheur::EFFET);
    let niveau = ctx.ram8(0x35) as u32;
    let mut decalage = ctx.prog8(declencheur::DECALAGES + niveau) as u16;
    for i in 0..4u32 {
        let joueur = Obj(JOUEURS + i * PAS_JOUEUR);
        let absent = joueur.u8(ctx.b, Obj::ACTIVE) == 0;
        if !ctx.pris(absent) {
            let variante = o.u8(ctx.b, Obj::VARIANT) as u32;
            ctx.c.d[1] = decalage as u32;
            ctx.c.d[0] = variante + decalage as u32;
            ctx.jsr(declencheur::EVENEMENT);
        }
        decalage = decalage.wrapping_add(0x100);
        ctx.pris(i < 3); // dbf
    }
    ctx.free(&pools::FREE_EFFCT);
}

fn declencheur_actif(ctx: &mut Ctx, o: Obj) {
    let limite = ctx.ram16(0x506).wrapping_sub(0x60);
    let x = o.u16(ctx.b, Obj::X);
    if ctx.pris(limite as i16 > x as i16) {
        return ctx.free(&pools::FREE_EFFCT);
    }
    let principal = Obj(ctx.ram16(declencheur::PRINCIPAL) as i16 as i32 as u32 & 0xff_ffff);
    let dx = principal.u16(ctx.b, Obj::X).wrapping_sub(x).wrapping_add(0x44);
    if ctx.pris(dx > 0x74) {
        return;
    }
    let dy = principal.u16(ctx.b, Obj::Y).wrapping_sub(o.u16(ctx.b, Obj::Y)).wrapping_sub(9);
    if !ctx.pris(dy <= 0x1c) {
        return;
    }
    declencheur_effet(ctx, o);
}

fn declencheur_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let variante = o.u8(ctx.b, Obj::VARIANT);
    ctx.anim_start(declencheur::ANIMATIONS, variante as u16);
    ctx.reinsert();
}

fn declencheur_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(declencheur::TABLE, etat as u32);
    match etat {
        etat::INIT => declencheur_init(ctx, o),
        etat::ACTIF => declencheur_actif(ctx, o),
        etat::LIBERE => declencheur_effet(ctx, o),
        _ => ctx.deleguer(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 4 : bulle d'action d'un personnage (0xaaac0)

/// Type 4 : bulle (icône animée) liée à un joueur (`+0x50`) ou à l'objet qu'il manipule (`+0x54`).
/// L'animation est choisie d'après l'action en cours du porteur (`+0x3d` du joueur, `+0x10d/0x10e`,
/// personnage `+3`) dans de petites tables de la ROM ; huit variantes :
///
/// - 0 : action du joueur (dix cas selon `+0x3d`) ;
/// - 1, 2, 5, 6 : action de l'objet porté (`+0x54`) ou fixe (`0x10`, `0x25` à `0x27`) ;
/// - 3 : bulle secondaire qui se pose sur l'objet porté (`0xaadba`) ;
/// - 4, 7 : animations fixes (`0x23`, `4`).
///
/// L'état 2 anime la bulle (variante 3 : la fait clignoter et la recolle à son porteur) ; l'état 4
/// libère ; un code de table valant `0x29` fait naître une bulle de variante 3.
mod bulle {
    pub const ENTREE: u32 = 0xaaac0;
    pub const ETATS: u32 = 0xaaace;
    pub const VARIANTES: u32 = 0xaaaee;
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
    pub const TABLE_PERSONNAGE: u32 = 0xab4f0;
    pub const TABLE_OBJET_A: u32 = 0xab4e4;
    pub const TABLE_OBJET_B: u32 = 0xab4ea;
    pub const TABLE_10D_A: u32 = 0xab676;
    pub const TABLE_10D_B: u32 = 0xab6a6;
    pub const TABLE_10D_C: u32 = 0xab6d6;
    /// Décalages de la variante 3 (selon la classe du porteur, `+0x43` et `+0x86` du joueur).
    pub const DECALAGES_3: u32 = 0xaae2a;
    pub const PROPRIETAIRE: u32 = 0x50;
    pub const PORTE: u32 = 0x54;
    pub const FIN_ANIMATION: u32 = 0x43;
    pub const COUP: u32 = 0x33;
    pub const MINUTEUR: u32 = 0x42;
    pub const PALETTE_JAUGE: u32 = 0x91c41c;
    /// Création d'une bulle de variante 0x1d (suite de la bulle d'un joueur). Quand le personnage vaut 1,
    /// l'original saute dans l'épilogue `0xaa9fe` d'une autre routine (voir `Ctx::epilogue_etranger`).
    pub const CREER_SUITE: u32 = 0xb13fa;
}

/// `0xaac00` : affiche l'animation `numero` du jeu des bulles, puis trie l'objet.
fn bulle_affiche(ctx: &mut Ctx, o: Obj, numero: u8) {
    if !ctx.pris(numero != 0x2b) {
        o.set_u16(ctx.b, 0x2a, 0x115);
    }
    ctx.anim_start(bulle::ANIMATIONS, numero as u16);
    ctx.reinsert();
}

/// `0xab26a` : un code `0x29` fait naître une bulle de variante 3 (ou, si le porteur est une BOX, joue
/// l'animation 4).
fn bulle_cree_suite(ctx: &mut Ctx, o: Obj) {
    let porte = pointeur(ctx, o, bulle::PORTE);
    let classe = porte.u8(ctx.b, Obj::CLASS);
    if !ctx.pris(classe != 0xa) {
        return bulle_affiche(ctx, o, 4);
    }
    o.set_u32(ctx.b, Obj::STATE, 0);
    o.set_u8(ctx.b, Obj::VARIANT, 3);
    o.set_u8(ctx.b, bulle::FIN_ANIMATION, 0);
    let nouveau = ctx.alloc(&pools::ALLOC_EFFCT);
    if ctx.pris(nouveau.is_none()) {
        return;
    }
    let n = nouveau.unwrap();
    n.set_u8(ctx.b, Obj::ACTIVE, 1);
    n.set_u8(ctx.b, Obj::KIND, 4);
    let p = o.u16(ctx.b, bulle::PORTE);
    n.set_u16(ctx.b, bulle::PORTE, p);
    n.set_u8(ctx.b, Obj::VARIANT, 3);
    n.set_u8(ctx.b, bulle::FIN_ANIMATION, 1);
}

/// `0xaabf4` : lit le code d'animation dans la table `table` (donnée) à l'indice `indice` ; le code
/// `0x29` crée la suite, les autres sont affichés.
fn bulle_lit(ctx: &mut Ctx, o: Obj, table: u32, indice: u16) {
    let code = ctx.rom8(table + indice as u32);
    if ctx.pris(code == 0x29) {
        return bulle_cree_suite(ctx, o);
    }
    bulle_affiche(ctx, o, code);
}

/// `0xaace8` : suite commune. Pour le personnage 1 l'original saute dans l'épilogue d'une autre
/// routine (`movem.l (SP)+,{A1-A4} ; rts`) : on reproduit l'effet sur la pile.
fn bulle_suite(ctx: &mut Ctx, o: Obj) {
    let proprietaire = pointeur(ctx, o, bulle::PROPRIETAIRE);
    let personnage = (proprietaire.u8(ctx.b, 3) >> 1).wrapping_sub(1);
    if ctx.pris(personnage == 0) {
        return ctx.epilogue_etranger();
    }
    ctx.jsr(bulle::CREER_SUITE);
}

/// `0xaacfe` : une fois sur seize environ, tire au hasard si on poursuit (`0x5555` : bits pairs).
fn bulle_hasard(ctx: &mut Ctx, o: Obj) {
    ctx.jsr(rom::RNG);
    let tirage = ctx.c.d[0] as u16 & 0xf;
    if ctx.pris(0x5555u16 >> tirage & 1 != 0) {
        bulle_suite(ctx, o);
    }
}

/// Cas communs de la variante 0 : `0xaabc4` (octet `+0x10d` ou `+0x10e` du joueur dans D0).
fn bulle_0_commun(ctx: &mut Ctx, o: Obj, joueur: Obj, d0: u8) {
    let classe = joueur.u8(ctx.b, 3) >> 1;
    let marque = joueur.u8(ctx.b, 0x77);
    if !ctx.pris(marque == 0) {
        joueur.set_u8(ctx.b, 0x77, 0);
        bulle_lit(ctx, o, bulle::TABLE_77, classe as u16);
        return bulle_hasard(ctx, o);
    }
    let indice = ((d0 as u16) << 3).wrapping_add(classe as u16);
    bulle_lit(ctx, o, bulle::TABLE_COUP, indice);
    bulle_hasard(ctx, o);
}

/// `0xaabac` : animation selon `+0x10d` (ou `+0x10e` pour le personnage 4).
fn bulle_0_action_10d(ctx: &mut Ctx, o: Obj, joueur: Obj) {
    let mut d0 = joueur.u8(ctx.b, 0x10d);
    let classe = joueur.u8(ctx.b, 3) >> 1;
    if !ctx.pris(classe != 4) {
        d0 = joueur.u8(ctx.b, 0x10e);
    }
    bulle_0_commun(ctx, o, joueur, d0);
}

/// Tables de la variante 0 : l'action `0x3d` du joueur mène à l'une des dix routines.
fn bulle_0(ctx: &mut Ctx, o: Obj) {
    let joueur = pointeur(ctx, o, bulle::PROPRIETAIRE);
    let action = joueur.u8(ctx.b, 0x3d);
    ctx.c.a[1] = o.u16(ctx.b, bulle::PROPRIETAIRE) as i16 as i32 as u32;
    let cible = ctx.saut_table(bulle::ACTIONS_0, action as u32);
    let octet = |ctx: &mut Ctx, off: u32| joueur.u8(ctx.b, off) as u16;
    let classe = joueur.u8(ctx.b, 3) >> 1;
    match action {
        0 => bulle_0_action_10d(ctx, o, joueur),
        2 => {
            let d0 = joueur.u8(ctx.b, 0x10e);
            bulle_0_commun(ctx, o, joueur, d0);
        }
        4 => {
            let indice = octet(ctx, 0x10d);
            bulle_lit(ctx, o, bulle::TABLE_10D_A, indice);
        }
        6 => {
            let indice = octet(ctx, 0x10e);
            bulle_lit(ctx, o, bulle::TABLE_10D_A, indice);
        }
        8 => {
            let indice = octet(ctx, 0x10d);
            bulle_lit(ctx, o, bulle::TABLE_10D_C, indice);
        }
        10 => {
            let indice = octet(ctx, 0x10e);
            bulle_lit(ctx, o, bulle::TABLE_10D_C, indice);
        }
        12 => {
            let mut indice = octet(ctx, 0x10d);
            if !ctx.pris(classe != 4) {
                indice = octet(ctx, 0x10e);
            }
            bulle_lit(ctx, o, bulle::TABLE_10D_B, indice);
        }
        14 => bulle_lit(ctx, o, bulle::TABLE_PERSONNAGE, classe as u16),
        16 => {
            let porte = pointeur(ctx, o, bulle::PORTE);
            let est_box = porte.u8(ctx.b, Obj::CLASS) == 0xa;
            if ctx.pris(est_box) {
                return bulle_0_action_10d(ctx, o, joueur);
            }
            let jauge_inactive = ctx.ram16(0x4be) != 0;
            if !ctx.pris(jauge_inactive) {
                crate::mem::wr16(ctx.b, bulle::PALETTE_JAUGE, 0xf0ff);
            }
            bulle_lit(ctx, o, bulle::TABLE_OBJET_A, classe as u16);
            bulle_suite(ctx, o);
        }
        18 => {
            let porte = pointeur(ctx, o, bulle::PORTE);
            let est_box = porte.u8(ctx.b, Obj::CLASS) == 0xa;
            if ctx.pris(est_box) {
                return bulle_0_action_10d(ctx, o, joueur);
            }
            let jauge_inactive = ctx.ram16(0x4be) != 0;
            if !ctx.pris(jauge_inactive) {
                crate::mem::wr16(ctx.b, bulle::PALETTE_JAUGE, 0xf0ff);
            }
            bulle_lit(ctx, o, bulle::TABLE_OBJET_B, classe as u16);
            bulle_suite(ctx, o);
        }
        _ => ctx.deleguer(cible),
    }
}

/// Variantes 1, 2, 5 et 6 : l'action `0x3d` de l'objet porté (`+0x54` ; joueur pour la variante 1) choisit
/// une animation fixe.
fn bulle_selon_action(ctx: &mut Ctx, o: Obj, offset_pointeur: u32, doubler: bool, table: u32, nombre: u32) {
    let source = pointeur(ctx, o, offset_pointeur);
    let action = source.u8(ctx.b, 0x3d) as u32;
    ctx.c.a[1] = o.u16(ctx.b, offset_pointeur) as i16 as i32 as u32;
    let indice = if doubler { action * 2 } else { action };
    let cible = ctx.saut_table(table, indice);
    if indice >= nombre || (!doubler && indice & 1 != 0) {
        return ctx.deleguer(cible);
    }
    let code = match (table, indice / if doubler { 2 } else { 1 }) {
        (bulle::ACTIONS_1, _) => 0x10,
        (_, 0) => 0x25,
        (_, 1) => 0x26,
        _ => 0x27,
    };
    bulle_affiche(ctx, o, code);
}

/// Variante 3 (`0xaadba`) : recolle la bulle à son porteur.
fn bulle_3_recolle(ctx: &mut Ctx, o: Obj) {
    let porte = pointeur(ctx, o, bulle::PORTE);
    let mut d0: u8 = 0;
    let classe = porte.u8(ctx.b, Obj::CLASS);
    if !ctx.pris(classe == 2) {
        d0 = 0x20;
    }
    d0 = d0.wrapping_add(porte.u8(ctx.b, Obj::KIND));
    let flip = porte.u8(ctx.b, champ::FLIP) ^ 0x20;
    o.set_u8(ctx.b, champ::FLIP, flip);
    for off in [Obj::X, Obj::Z] {
        let v = porte.u16(ctx.b, off);
        o.set_u16(ctx.b, off, v);
    }
    let y = porte.u32(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u32(ctx.b, Obj::Y, y);
    let suite = (o.u8(ctx.b, bulle::FIN_ANIMATION) & 1) as u16;
    let base = bulle::DECALAGES_3 + (((d0 as u16) << 4).wrapping_add(suite << 3)) as u32;
    let index = ((porte.u8(ctx.b, 0x86) as u32) << 2) & 0xffff;
    let mut dx = ctx.rom16(base + index);
    let dz = ctx.rom16(base + index + 2);
    decale_u16(ctx, o, Obj::Z, dz);
    if !ctx.pris(flip != 0) {
        dx = dx.wrapping_neg();
    }
    decale_u16(ctx, o, Obj::X, dx);
}

fn bulle_3_init(ctx: &mut Ctx, o: Obj) {
    let fin = o.u8(ctx.b, bulle::FIN_ANIMATION);
    let numero = if ctx.pris(fin == 0) { 0x29 } else { 0x2a };
    ctx.anim_start(bulle::ANIMATIONS, numero);
    o.set_u8(ctx.b, bulle::MINUTEUR, 0x1e);
    bulle_3_recolle(ctx, o);
}

fn bulle_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, champ::MODE, 0);
    let variante = o.u8(ctx.b, Obj::VARIANT);
    let cible = ctx.saut_table(bulle::VARIANTES, 2 * variante as u32);
    match variante {
        0 => bulle_0(ctx, o),
        1 => bulle_selon_action(ctx, o, bulle::PROPRIETAIRE, false, bulle::ACTIONS_1, 6),
        2 => bulle_selon_action(ctx, o, bulle::PORTE, true, bulle::ACTIONS_2, 20),
        3 => bulle_3_init(ctx, o),
        4 => bulle_affiche(ctx, o, 0x23),
        5 => bulle_selon_action(ctx, o, bulle::PORTE, true, bulle::ACTIONS_5, 6),
        6 => bulle_selon_action(ctx, o, bulle::PORTE, true, bulle::ACTIONS_6, 6),
        7 => bulle_affiche(ctx, o, 4),
        _ => ctx.deleguer(cible),
    }
}

fn bulle_actif(ctx: &mut Ctx, o: Obj) {
    let variante = o.u8(ctx.b, Obj::VARIANT);
    if ctx.pris(variante == 3) {
        // variante 3 : clignote, se recolle, et passe à l'état suivant au bout de `MINUTEUR` images
        let flags = o.u8(ctx.b, Obj::FLAGS);
        o.set_u8(ctx.b, Obj::FLAGS, flags ^ 2);
        ctx.jsr(rom::ANIM_STEP);
        bulle_3_recolle(ctx, o);
        let reste = o.u8(ctx.b, bulle::MINUTEUR).wrapping_sub(1);
        o.set_u8(ctx.b, bulle::MINUTEUR, reste);
        if !ctx.pris(reste != 0) {
            ajoute_u8(ctx, o, Obj::STATE, 2);
        }
        return ctx.reinsert();
    }
    let fin = ctx.anim_step();
    if !ctx.pris(!fin) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
        return;
    }
    // la jauge de scroll du fond suit l'appui du bouton lié au coup (`+0x33`)
    let mut valeur = 0u16;
    let coup = o.u8(ctx.b, bulle::COUP);
    if !ctx.pris(coup == 0) {
        let jauge = ctx.ram16(0x4be) != 0;
        if !ctx.pris(jauge) {
            let bloque = ctx.ram8(0x4f) != 0;
            if !ctx.pris(bloque) {
                valeur = 0x100;
            }
        }
    }
    let adresse = crate::mem::A5.wrapping_add(0x4ba);
    crate::mem::wr16(ctx.b, adresse, valeur);
    ctx.reinsert();
}

/// Libération (`0xab24c`) : rend la palette de la jauge si la bulle l'avait modifiée.
fn bulle_fin(ctx: &mut Ctx, o: Obj) {
    let modifiee = o.u8(ctx.b, champ::MODE);
    if !ctx.pris(modifiee == 0) {
        let jauge = ctx.ram16(0x4be) != 0;
        if !ctx.pris(jauge) {
            crate::mem::wr16(ctx.b, bulle::PALETTE_JAUGE, 0xffff);
        }
    }
    ctx.free(&pools::FREE_EFFCT);
}

fn bulle_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(bulle::ETATS, etat as u32);
    match etat {
        etat::INIT => bulle_init(ctx, o),
        etat::ACTIF => bulle_actif(ctx, o),
        etat::LIBERE => bulle_fin(ctx, o),
        6 => ctx.free(&pools::FREE_EFFCT),
        _ => ctx.deleguer(cible),
    }
}

gestionnaire!(EFFCT_4, bulle::ENTREE, "effct_bulle", bulle_gestionnaire);
gestionnaire!(EFFCT_24, accessoire::ENTREE, "effct_accessoire", accessoire_gestionnaire);
gestionnaire!(EFFCT_45, declencheur::ENTREE, "effct_declencheur", declencheur_gestionnaire);
gestionnaire!(EFFCT_51, projectile::ENTREE, "effct_projectile", projectile_gestionnaire);
gestionnaire!(EFFCT_52, scintille_classe::ENTREE, "effct_scintille_classe", scintille_classe_gestionnaire);
gestionnaire!(EFFCT_56, eclats_debris::ENTREE, "effct_eclats_debris", eclats_debris_gestionnaire);
// ---------------------------------------------------------------------------------------------
// Type 8 : trace sur un objet (0xababa)

/// Type 8 : trace collée à un objet (`+0x5c`), un sous-pixel devant ou derrière selon la variante ; son
/// animation dépend du type de l'objet (et de sa classe : BOX) ; elle disparaît à la fin de l'animation.
/// Non exercé.
mod trace {
    pub const ENTREE: u32 = 0xababa;
    pub const TABLE: u32 = 0xabac8;
    pub const ANIMATIONS_TYPE: u32 = 0xabb26;
    pub const ANIMATIONS: u32 = 0x18e872;
    pub const PORTEUR: u32 = 0x5c;
}

fn trace_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    let porteur = pointeur(ctx, o, trace::PORTEUR);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = porteur.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    let variante = o.u8(ctx.b, Obj::VARIANT);
    let decalage: u32 = if ctx.pris(variante == 0) { 0xffff_ffff } else { 1 };
    let y = o.u32(ctx.b, Obj::Y).wrapping_add(decalage);
    o.set_u32(ctx.b, Obj::Y, y);
    let mut d0: u8 = 0;
    let classe = porteur.u8(ctx.b, Obj::CLASS);
    if !ctx.pris(classe != 4) {
        d0 = 0x20;
    }
    d0 = d0.wrapping_add(porteur.u8(ctx.b, Obj::KIND));
    let animation = ctx.prog8(trace::ANIMATIONS_TYPE + d0 as u32).wrapping_add(variante);
    ctx.anim_start(trace::ANIMATIONS, animation as u16);
    ctx.reinsert();
}

fn trace_actif(ctx: &mut Ctx, _o: Obj) {
    let fin = ctx.anim_step();
    if ctx.pris(fin) {
        ctx.free(&pools::FREE_EFFCT);
    }
}

fn trace_gestionnaire(ctx: &mut Ctx) {
    trois_etats(ctx, trace::TABLE, trace_init, trace_actif)
}

gestionnaire!(EFFCT_8, trace::ENTREE, "effct_trace", trace_gestionnaire);
gestionnaire!(EFFCT_28, etincelle::ENTREE, "effct_etincelle", etincelle_gestionnaire);
gestionnaire!(EFFCT_57, marque::ENTREE, "effct_marque", marque_gestionnaire);
gestionnaire!(EFFCT_62, gerbe::ENTREE, "effct_gerbe", gerbe_gestionnaire);
gestionnaire!(EFFCT_7, eclat_unique::ENTREE, "effct_eclat_unique", eclat_unique_gestionnaire);
gestionnaire!(EFFCT_19, balayage::ENTREE, "effct_balayage", balayage_gestionnaire);
gestionnaire!(EFFCT_27, fumee::ENTREE, "effct_fumee", fumee_gestionnaire);
gestionnaire!(EFFCT_1, 0xa8758, "effct_supprime", supprime);
gestionnaire!(EFFCT_6, debris::ENTREE, "effct_debris", debris_gestionnaire);
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
