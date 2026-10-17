//! Famille BOX (classe 0xA) : caisses, tonneaux et coffres destructibles. Dispatch `0x39e50`, table
//! des types `0x39e76` (4 entrées dont une seule exercée : le type 0). 21 emplacements de 0x80 octets.
//! Voir `docs/familles.md`.

use crate::objects::pools;
use crate::objects::Obj;

use super::{ajoute_u8, gestionnaire, pointeur, Ctx};

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
    pub const MODE: u32 = 0x2f;
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

// ---------------------------------------------------------------------------------------------
// Type 2 : décor inerte (0xda7ac)

/// Type 2 : élément de décor sans comportement (animation 5 du jeu des BOX) ; l'objet s'initialise,
/// puis se contente de se trier chaque image jusqu'à sa libération. Non exercé par les scénarios.
mod decor {
    pub const ENTREE: u32 = 0xda7ac;
    pub const TABLE_ETATS: u32 = 0xda7c2;
    pub const ANIMATION: u16 = 5;
}

fn box_2(ctx: &mut Ctx) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if ctx.pris(cache) {
        return;
    }
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(decor::TABLE_ETATS, etat as u32);
    match etat {
        etat::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, Obj::ACTIVE, 2);
            o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
            ctx.anim_start(rom::ANIMATIONS, decor::ANIMATION);
            ctx.reinsert();
        }
        etat::ACTIF => ctx.reinsert(),
        etat::LIBERE => ctx.free(&pools::FREE_BOX),
        _ => ctx.deleguer(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 1 : barrière à ouvrir (0xda578)

/// Type 1 : élément de décor interactif frappé par les joueurs (`+0x6e`, posé par leur code de coup) :
/// variante 0, grille qui crée un objet SET (`0x8b`, d'où sort une porte) puis attend sa fin ; variante
/// 1, mécanisme à deux temps dont l'animation fait changer d'état l'objet frappeur (`+0x6c`) ;
/// variante 3 comme la 0 à l'initialisation. L'objet disparaît quand la caméra l'a dépassé de 0x200.
/// Non exercé.
mod barriere {
    pub const ENTREE: u32 = 0xda578;
    pub const ETATS: u32 = 0xda58e;
    pub const INITS: u32 = 0xda5a6;
    pub const ACTIFS: u32 = 0xda5f4;
    pub const SOUS_ETATS_0: u32 = 0xda624;
    pub const SOUS_ETATS_1: u32 = 0xda65e;
    pub const ANIMATIONS_0: u32 = 0x1bed06;
    pub const ANIMATIONS_1: u32 = 0x1712d4;
    pub const POSITIONS: u32 = 0xda72c;
    pub const EFFET_INITIAL: u32 = 0xa5f38;
    pub const SON: u32 = 0xeae86;
    pub const CREER_SET: u32 = 0xc8312;
    pub const FRAPPEUR: u32 = 0x6c;
    pub const SET_CREE: u32 = 0x7c;
}

/// `0xda6f4` : crée l'objet SET de la barrière et le rattache.
fn barriere_cree_set(ctx: &mut Ctx, o: Obj) {
    ctx.c.d[0] = ctx.c.d[0] & 0xffff_ff00 | 0x8b;
    ctx.jsr(barriere::CREER_SET);
    let echec = ctx.c.cond(5);
    if ctx.pris(echec) {
        return;
    }
    let nouveau = Obj(ctx.c.a[1]);
    let frappeur = pointeur(ctx, o, barriere::FRAPPEUR);
    let mode = (o.u8(ctx.b, champ::MODE) as u32) << 3;
    let classe = ((frappeur.u8(ctx.b, 3) & 0xf) >> 1) as u32;
    let position = ctx.prog8(barriere::POSITIONS + ((mode + classe) & 0xffff));
    nouveau.set_u8(ctx.b, 0x42, position);
    let personnage = frappeur.u8(ctx.b, 2);
    nouveau.set_u8(ctx.b, 0x43, personnage);
    o.set_u16(ctx.b, barriere::SET_CREE, nouveau.0 as u16);
}

fn barriere_init(ctx: &mut Ctx, o: Obj) {
    let variante = o.u8(ctx.b, Obj::VARIANT);
    let cible = ctx.saut_table(barriere::INITS, 2 * variante as u32);
    match variante {
        0 | 3 => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
            o.set_u32(ctx.b, 0x78, 0);
            ctx.anim_start(barriere::ANIMATIONS_0, 0);
        }
        1 => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
            ctx.jsr(barriere::EFFET_INITIAL);
            ctx.anim_start(barriere::ANIMATIONS_1, 4);
        }
        _ => ctx.deleguer(cible),
    }
}

/// Variante 0, actif : sous-état 0 attend le coup, sous-état 2 attend la fin du SET ; puis disparaît
/// quand elle sort de l'écran.
fn barriere_actif_0(ctx: &mut Ctx, o: Obj) {
    let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
    ctx.jsr_indirect(0xda602, |ctx| {
        let cible = ctx.saut_table(barriere::SOUS_ETATS_0, sous_etat as u32);
        match sous_etat {
            0 => {
                let coup = o.u8(ctx.b, champ::COUP_B);
                if !ctx.pris(coup == 0) {
                    ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
                    barriere_cree_set(ctx, o);
                }
                ctx.reinsert();
            }
            2 => {
                let set = pointeur(ctx, o, barriere::SET_CREE);
                let fini = set.u8(ctx.b, 0x22) != 0;
                if !ctx.pris(!fini) {
                    o.set_u8(ctx.b, champ::SOUS_ETAT, 0);
                    o.set_u8(ctx.b, champ::COUP_B, 0);
                }
            }
            _ => ctx.deleguer(cible),
        }
    });
    let (cam_x, _) = ctx.camera();
    let ecart = cam_x.wrapping_sub(o.u16(ctx.b, Obj::X));
    if !ctx.pris((ecart as i16) < 0x200) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
}

fn barriere_actif_1(ctx: &mut Ctx, o: Obj) {
    let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
    let cible = ctx.saut_table(barriere::SOUS_ETATS_1, sous_etat as u32);
    match sous_etat {
        0 | 4 => {
            let coup = o.u8(ctx.b, champ::COUP_B);
            if ctx.pris(coup == 0) {
                return ctx.reinsert();
            }
            ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
            let frappeur = pointeur(ctx, o, barriere::FRAPPEUR);
            frappeur.set_u32(ctx.b, Obj::STATE, 0x0200_3000);
            let animation = if sous_etat == 0 { 5 } else { 6 };
            ctx.anim_start(barriere::ANIMATIONS_1, animation);
        }
        2 | 6 => {
            let fin = ctx.anim_step();
            if ctx.pris(!fin) {
                return;
            }
            if sous_etat == 2 {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
            } else {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 0xfa);
            }
            o.set_u8(ctx.b, champ::COUP_B, 0);
            ctx.jsr(barriere::SON);
        }
        _ => ctx.deleguer(cible),
    }
}

fn barriere_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if ctx.pris(cache) {
        return;
    }
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(barriere::ETATS, etat as u32);
    match etat {
        etat::INIT => barriere_init(ctx, o),
        etat::ACTIF => {
            let variante = o.u8(ctx.b, Obj::VARIANT);
            let cible = ctx.saut_table(barriere::ACTIFS, 2 * variante as u32);
            match variante {
                0 => barriere_actif_0(ctx, o),
                1 => barriere_actif_1(ctx, o),
                _ => ctx.deleguer(cible),
            }
        }
        etat::LIBERE | etat::HORS_ECRAN => ctx.free(&pools::FREE_BOX),
        _ => ctx.deleguer(cible),
    }
}

gestionnaire!(BOX_1, barriere::ENTREE, "box_barriere", barriere_gestionnaire);
gestionnaire!(BOX_0, rom::ENTREE, "box_caisse", box_0);
gestionnaire!(BOX_2, decor::ENTREE, "box_decor", box_2);
