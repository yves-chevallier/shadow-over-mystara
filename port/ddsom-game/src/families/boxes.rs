//! Famille BOX (classe 0xA) : caisses, tonneaux et coffres destructibles. Dispatch `0x39e50`, table
//! des types `0x39e76` (4 entrées dont une seule exercée : le type 0). 21 emplacements de 0x80 octets.
//! Voir `docs/familles.md`.

use crate::objects::pools;
use crate::objects::Obj;

use super::{ajoute_u8, gestionnaire, Ctx};

/// Valeurs de l'octet d'état `+4`.
mod etat {
    pub const INIT: u8 = 0;
    pub const ACTIF: u8 = 2;
    pub const LIBERE: u8 = 4;
    pub const HORS_ECRAN: u8 = 6;
}

/// Sous-états `+5` de l'état actif.
mod sous {
    /// Intact : attend un coup.
    pub const INTACT: u8 = 0;
    /// Touché : animation de casse, éclats, butin.
    pub const TOUCHE: u8 = 0x0a;
}

/// Champs d'une BOX.
mod champ {
    pub const SOUS_ETAT: u32 = 0x05;
    pub const SOUS_SOUS_ETAT: u32 = 0x06;
    pub const JEU_OMBRES: u32 = 0x2e;
    /// Coup reçu, posé par le code des joueurs (+0x33 : seconde marque de coup).
    pub const COUP: u32 = 0x33;
    pub const COUP_B: u32 = 0x6e;
    /// Variété de caisse (0 à 2 : se brise d'un coup ; 3 et plus : coffre qui résiste).
    pub const SORTE: u32 = 0x60;
    /// Butin : objet au centre (`+0x62`), objets lancés à droite (par classe de joueur, `+0x63..`) et à
    /// gauche (`+0x6a`).
    pub const BUTIN_HAUT: u32 = 0x62;
    pub const BUTIN_CLASSES: u32 = 0x63;
    pub const BUTIN_GAUCHE: u32 = 0x6a;
    /// Effet spécial éventuel (`+0x6b`) et marque « effet déjà joué » (`+0x7d`).
    pub const SPECIAL: u32 = 0x6b;
    pub const SPECIAL_JOUE: u32 = 0x7d;
    /// Joueur qui a frappé (pointeur RAM 16 bits).
    pub const FRAPPEUR: u32 = 0x6c;
    pub const DELAI: u32 = 0x6f;
    pub const ORIGINE_X: u32 = 0x78;
    pub const ORIGINE_Y: u32 = 0x7a;
    pub const BUTIN_LACHE: u32 = 0x7c;
    pub const SELF: u32 = 0x5c;
}

/// Tables et sous-routines.
mod rom {
    pub const ENTREE: u32 = 0xd99fa;
    pub const TABLE_ETATS: u32 = 0xd9a10;
    pub const TABLE_SOUS_ETATS: u32 = 0xd9a6a;
    pub const TABLE_SOUS_SOUS_ETATS: u32 = 0xd9abc;
    /// Instruction `jsr (d8,PC,Xn)` du sous-état « touché ».
    pub const APPEL_TOUCHE: u32 = 0xd9ab2;
    pub const ANIMATIONS: u32 = 0x1bebc2;
    /// Par variété de caisse : adresse de sa fiche de 0x2a octets (caractéristiques, butins aléatoires).
    pub const FICHES: u32 = 0x1051ae;
    /// Par numéro de butin : (type, variante) de l'ITEM créé.
    pub const BUTINS: u32 = 0xda28a;
    /// Effets spéciaux (adresses de routines), par valeur de `SPECIAL`.
    pub const SPECIAUX: u32 = 0xda06a;
    pub const COMPTEUR_CASSEES: i32 = -0x5b21;
    // sous-routines non réécrites
    pub const SON_COUP: u32 = 0xeb59e;
    pub const SON_CASSE: u32 = 0xeb610;
    pub const EFFET_INITIAL: u32 = 0xb6596;
}

const OMBRES: u8 = 0x35;
const DUREE_CLIGNOTEMENT: u8 = 0x1e;

/// `0xda082` : recopie la fiche de la variété (10 octets) et tire deux butins au hasard.
fn charge_fiche(ctx: &mut Ctx, o: Obj) {
    let variante = o.u8(ctx.b, Obj::VARIANT) as u32;
    let fiche = crate::mem::rd32(ctx.b, rom::FICHES + 4 * variante);
    let champs = [champ::SORTE, 0x61, champ::BUTIN_HAUT, 0x69, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68];
    for (i, off) in champs.into_iter().enumerate() {
        let v = ctx.rom8(fiche + i as u32);
        o.set_u8(ctx.b, off, v);
    }
    ctx.jsr(super::rom::RNG);
    let tirage = ctx.c.d[0] as u16 & 0xf;
    let v = ctx.rom8(fiche + 0xa + tirage as u32);
    o.set_u8(ctx.b, champ::BUTIN_GAUCHE, v);
    ctx.jsr(super::rom::RNG);
    let tirage = ctx.c.d[0] as u16 & 0xf;
    let v = ctx.rom8(fiche + 0x1a + tirage as u32);
    o.set_u8(ctx.b, champ::SPECIAL, v);
}

fn box_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, 0x70, 0);
    o.set_u8(ctx.b, champ::COUP_B, 0);
    o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
    let x = o.u16(ctx.b, Obj::X);
    o.set_u16(ctx.b, champ::ORIGINE_X, x);
    let y = o.u16(ctx.b, Obj::Y);
    o.set_u16(ctx.b, champ::ORIGINE_Y, y);
    o.set_u16(ctx.b, champ::SELF, o.0 as u16);
    charge_fiche(ctx, o);
    let special = o.u8(ctx.b, champ::SPECIAL);
    if !ctx.pris(special == 0) {
        ctx.jsr(rom::EFFET_INITIAL);
    }
    let sorte = o.u8(ctx.b, champ::SORTE);
    ctx.anim_start(rom::ANIMATIONS, sorte as u16);
}

/// Sous-état intact : un coup (`COUP_B`) fait passer à l'état « touché » ; une caisse ordinaire
/// (sorte < 3) reste là à s'animer, un coffre (sorte >= 3) se brise aussitôt.
fn box_intacte(ctx: &mut Ctx, o: Obj) {
    let coup = o.u8(ctx.b, champ::COUP_B);
    if !ctx.pris(coup == 0) {
        o.set_u32(ctx.b, Obj::STATE, 0x020a_0000);
        let sorte = o.u8(ctx.b, champ::SORTE);
        if !ctx.pris(sorte < 3) {
            o.set_u8(ctx.b, champ::SOUS_SOUS_ETAT, 4);
            o.set_u8(ctx.b, Obj::ACTIVE, 2);
            o.set_u8(ctx.b, champ::DELAI, 8);
            ctx.jsr(rom::SON_CASSE);
        }
    }
    ctx.reinsert();
}

/// `0xda04e` : joue l'effet spécial de la caisse, une seule fois (`D1` : moment de l'appel).
fn effet_special(ctx: &mut Ctx, o: Obj, moment: u32) {
    ctx.c.d[1] = moment;
    let deja = o.u8(ctx.b, champ::SPECIAL_JOUE) != 0;
    if ctx.pris(deja) {
        return;
    }
    o.set_u8(ctx.b, champ::SPECIAL_JOUE, 1);
    let special = o.u8(ctx.b, champ::SPECIAL) as u32;
    let effet = ctx.prog32(rom::SPECIAUX + 4 * special);
    ctx.jsr(effet);
}

/// Crée l'ITEM `butin` (type et variante lus dans `BUTINS`) au-dessus de la caisse, lancé avec les
/// vitesses `(vx, vy)`. `None` si le pool d'ITEM est plein.
fn cree_butin(ctx: &mut Ctx, o: Obj, butin: u8, vx: u16, vy: u16) -> Option<Obj> {
    let nouveau = ctx.alloc_item();
    ctx.pris(nouveau.is_none()); // bcs : pool plein
    let n = nouveau?;
    let rang = (butin as u32) * 2 & 0xffff;
    n.set_u8(ctx.b, Obj::ACTIVE, 1);
    let kind = ctx.rom8(rom::BUTINS + rang);
    n.set_u8(ctx.b, Obj::KIND, kind);
    let variante = ctx.rom8(rom::BUTINS + rang + 1);
    n.set_u8(ctx.b, Obj::VARIANT, variante);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = o.u16(ctx.b, off);
        n.set_u16(ctx.b, off, v);
    }
    let z = n.u16(ctx.b, Obj::Z).wrapping_add(0x18);
    n.set_u16(ctx.b, Obj::Z, z);
    let y = n.u16(ctx.b, Obj::Y).wrapping_sub(1);
    n.set_u16(ctx.b, Obj::Y, y);
    n.set_u16(ctx.b, Obj::VX, vx);
    n.set_u16(ctx.b, Obj::VZ, 0x300);
    n.set_u16(ctx.b, Obj::AZ, 0xffd8);
    n.set_u16(ctx.b, Obj::VY, vy);
    Some(n)
}

/// `0xda152` : la caisse brisée lâche son butin (une seule fois) : un objet au centre, un selon la
/// classe du joueur qui a frappé (lancé à droite) et un tiré au hasard (lancé à gauche).
fn lache_butin(ctx: &mut Ctx, o: Obj) {
    let deja = o.u8(ctx.b, champ::BUTIN_LACHE) != 0;
    if ctx.pris(deja) {
        return;
    }
    o.set_u8(ctx.b, champ::BUTIN_LACHE, 1);
    let haut = o.u8(ctx.b, champ::BUTIN_HAUT);
    if !ctx.pris(haut == 0) && cree_butin(ctx, o, haut, 0, 0).is_none() {
        return;
    }
    let frappeur = Obj(o.u16(ctx.b, champ::FRAPPEUR) as i16 as i32 as u32 & 0xff_ffff);
    let classe = (frappeur.u8(ctx.b, 3) >> 1) as u32;
    let droite = o.u8(ctx.b, champ::BUTIN_CLASSES + classe);
    if !ctx.pris(droite == 0) && cree_butin(ctx, o, droite, 0x140, 0x40).is_none() {
        return;
    }
    let gauche = o.u8(ctx.b, champ::BUTIN_GAUCHE);
    if ctx.pris(gauche == 0) {
        return;
    }
    cree_butin(ctx, o, gauche, 0xfec0, 0xffc0);
}

/// Clignotement final : compte à rebours `DELAI`, la caisse clignote ; à zéro, état suivant.
fn clignote_et_passe(ctx: &mut Ctx, o: Obj) {
    let reste = o.u8(ctx.b, champ::DELAI).wrapping_sub(1);
    o.set_u8(ctx.b, champ::DELAI, reste);
    if !ctx.pris(reste != 0) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    if ctx.pris(reste & 1 != 0) {
        return;
    }
    o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
}

/// Sous-sous-état 0 du sous-état « touché » : la caisse joue son animation de casse puis lâche son butin.
fn box_casse(ctx: &mut Ctx, o: Obj) {
    let sorte = o.u8(ctx.b, champ::SORTE);
    if !ctx.pris(sorte < 3) {
        o.set_u32(ctx.b, Obj::STATE, 0x0200_0000);
        o.set_u8(ctx.b, champ::COUP_B, 0);
        o.set_u8(ctx.b, Obj::ACTIVE, 1);
        return;
    }
    let coup = o.u8(ctx.b, champ::COUP);
    if !ctx.pris(coup == 0) {
        o.set_u8(ctx.b, champ::COUP, 0);
        lache_butin(ctx, o);
        ctx.jsr(rom::SON_COUP);
        let special = o.u8(ctx.b, champ::SPECIAL);
        if !ctx.pris(special == 0) {
            effet_special(ctx, o, 0);
        }
    }
    let fin = ctx.anim_step();
    if !ctx.pris(!fin) {
        ajoute_u8(ctx, o, champ::SOUS_SOUS_ETAT, 2);
        o.set_u8(ctx.b, Obj::ACTIVE, 2);
        o.set_u8(ctx.b, champ::DELAI, DUREE_CLIGNOTEMENT);
        let n = ctx.ram8(rom::COMPTEUR_CASSEES).wrapping_add(1);
        ctx.set_ram8(rom::COMPTEUR_CASSEES, n);
    }
}

fn box_touchee(ctx: &mut Ctx, o: Obj) {
    let sous = o.u8(ctx.b, champ::SOUS_SOUS_ETAT);
    ctx.jsr_indirect(rom::APPEL_TOUCHE, |ctx| {
        let cible = ctx.saut_table(rom::TABLE_SOUS_SOUS_ETATS, sous as u32);
        match sous {
            0 => box_casse(ctx, o),
            2 => clignote_et_passe(ctx, o),
            _ => ctx.deleguer(cible),
        }
    });
    ctx.reinsert();
}

fn box_active(ctx: &mut Ctx, o: Obj) {
    let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
    let cible = ctx.saut_table(rom::TABLE_SOUS_ETATS, sous_etat as u32);
    match sous_etat {
        sous::INTACT => box_intacte(ctx, o),
        sous::TOUCHE => box_touchee(ctx, o),
        _ => ctx.deleguer(cible),
    }
}

/// Type 0 : caisse, tonneau ou coffre destructible (variantes 0 à 3). Une fiche de la ROM donne ses
/// caractéristiques ; frappée, elle joue son animation de casse, lâche jusqu'à trois objets ITEM puis
/// clignote et disparaît. Les sous-états d'envol d'éclats et de tremblement des coffres ne sont pas
/// exercés par les scénarios : ils restent en code d'origine.
fn box_0(ctx: &mut Ctx) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if ctx.pris(cache) {
        return;
    }
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(rom::TABLE_ETATS, etat as u32);
    match etat {
        etat::INIT => box_init(ctx, o),
        etat::ACTIF => box_active(ctx, o),
        etat::LIBERE | etat::HORS_ECRAN => ctx.free(&pools::FREE_BOX),
        _ => ctx.deleguer(cible),
    }
}

gestionnaire!(BOX_0, rom::ENTREE, "box_caisse", box_0);
