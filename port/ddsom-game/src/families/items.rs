//! Famille ITEM (classe 0xC) : objets ramassables. Dispatch `0x39f34`, table des types `0x39f5a`
//! (10 types, 32 emplacements de 0x60 octets).
//!
//! Un objet ITEM naît (créé par un ennemi, un coffre ou un décor) à la position de son créateur ;
//! il tombe s'il est en l'air, reste au sol un temps limité en clignotant avant de disparaître, et
//! produit son effet quand un joueur le ramasse (le code du joueur pose `RAMASSE` et `PROPRIETAIRE`).
//! Voir `docs/familles.md` pour la description de chaque type.

use crate::objects::pools;
use crate::objects::Obj;

use super::{ajoute_u8, gestionnaire, pointeur, rom, Ctx};

/// Valeurs de l'octet d'état `+4`.
mod etat {
    pub const INIT: u8 = 0;
    pub const ACTIF: u8 = 2;
    pub const LIBERE: u8 = 4;
    /// Posé par `obj_cull_offscreen` : traité comme `LIBERE`.
    pub const HORS_ECRAN: u8 = 6;
}

/// Valeurs de l'octet de sous-état `+5` de l'état actif (types à chute).
mod sous {
    pub const CHUTE: u8 = 0;
    pub const ATTENTE: u8 = 2;
    pub const RAMASSE: u8 = 4;
}

/// Champs propres aux ITEM.
mod champ {
    pub const SOUS_ETAT: u32 = 0x05;
    pub const SOUS_SOUS_ETAT: u32 = 0x06;
    pub const JEU_OMBRES: u32 = 0x2e;
    pub const MODE: u32 = 0x2f;
    pub const DUREE: u32 = 0x42;
    pub const PROPRIETAIRE: u32 = 0x50;
    pub const SCINTILLEMENT: u32 = 0x54;
    pub const RAMASSE: u32 = 0x58;
    pub const INDICE: u32 = 0x5d;
}

/// Jeu d'ombres commun aux ITEM.
const OMBRES: u8 = 0x3c;
/// Indicateurs « objet de collection ramassé » (un octet par objet), `A5 - 0x5b88`.
const RAMASSES: i32 = -0x5b88;

/// Sous-routines non réécrites.
mod appel {
    /// Crée le scintillement (EFFCT 15) au-dessus de l'objet, d'après `SCINTILLEMENT`.
    pub const SCINTILLEMENT: u32 = 0xae3ec;
    /// Ramène l'objet dans la zone de jeu visible.
    pub const DANS_LA_ZONE: u32 = 0xfbae;
    /// Ombre et marque au sol.
    pub const OMBRE: u32 = 0x33fe4;
    /// Événement de score `(joueur << 8) | type`.
    pub const EVENEMENT: u32 = 0x19ae6;
    /// Son associé à un ramassage (numéro dans D0).
    pub const SON_RAMASSAGE: u32 = 0xb12c0;
    /// Fin commune des effets de potion.
    pub const FIN_EFFET: u32 = 0xebaf6;
}

/// Passe à l'état suivant avec un sous-état donné (`addq.b #2,(4,A0)`, sous-sous-état à zéro).
fn etat_suivant(ctx: &mut Ctx, o: Obj, sous_etat: u8) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, champ::SOUS_ETAT, sous_etat);
    o.set_u16(ctx.b, champ::SOUS_SOUS_ETAT, 0);
}

/// Un objet créé au-dessus du sol (altitude non nulle) commence par tomber.
fn en_l_air(ctx: &mut Ctx, o: Obj) -> bool {
    let z = o.u16(ctx.b, Obj::Z);
    !ctx.pris(z == 0) // tst.w (0xc,A0) ; beq : au sol
}

/// Marque l'objet de collection `+0x5d` comme ramassé.
fn marque_ramasse(ctx: &mut Ctx, o: Obj) {
    let indice = o.u8(ctx.b, champ::INDICE) as i32;
    ctx.set_ram8(RAMASSES + indice, 1);
}

/// Ramène l'objet dans la zone visible : au plus haut à `caméra.y + 0x20`, entre `caméra.x + 0x18`
/// et `caméra.x + 0x168` (bloc recopié dans chaque type).
fn dans_la_vue(ctx: &mut Ctx, o: Obj) {
    let (cam_x, cam_y) = ctx.camera();
    let y_min = cam_y.wrapping_add(0x20);
    let y = o.u16(ctx.b, Obj::Y);
    if !ctx.pris(y_min < y) {
        o.set_u16(ctx.b, Obj::Y, y_min);
    }
    let x_min = cam_x.wrapping_add(0x18);
    let x = o.u16(ctx.b, Obj::X);
    if !ctx.pris(x_min as i16 <= x as i16) {
        o.set_u16(ctx.b, Obj::X, x_min);
    } else {
        let x_max = cam_x.wrapping_add(0x168);
        if !ctx.pris(x_max > x) {
            o.set_u16(ctx.b, Obj::X, x_max);
        }
    }
}

/// Compte à rebours de la durée de vie ; sous 0x3c images l'objet clignote (le bit 1 de `FLAGS`
/// cache l'objet une image sur deux, la phase est donnée par `D7`) ; à zéro il est libéré.
fn expire_et_clignote(ctx: &mut Ctx, o: Obj) {
    let reste = o.u16(ctx.b, champ::DUREE).wrapping_sub(1);
    o.set_u16(ctx.b, champ::DUREE, reste);
    if !ctx.pris(reste as i16 >= 0) {
        o.set_u8(ctx.b, Obj::STATE, etat::LIBERE);
        o.set_u8(ctx.b, Obj::ACTIVE, 2);
    }
    if ctx.pris(reste > 0x3c) {
        return;
    }
    let phase = reste.wrapping_add(ctx.loop_counter());
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    if ctx.pris(phase & 1 == 0) {
        return;
    }
    o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
}

/// Fin commune de l'état de chute : ramener dans la zone, ombre, tri par profondeur.
fn fin_de_chute(ctx: &mut Ctx) {
    ctx.jsr(appel::DANS_LA_ZONE);
    ctx.jsr(appel::OMBRE);
    ctx.reinsert();
}

/// Démarre le script `numero` du jeu d'animations `jeu`, puis trie l'objet (fin des initialisations).
fn anime_et_trie(ctx: &mut Ctx, jeu: u32, numero: u8) {
    ctx.anim_start(jeu, numero as u16);
    ctx.reinsert();
}

/// Sous-état « ramassé » : attend que le joueur ait fini son geste (propriétaire prêt) ou un
/// ramassage forcé (`RAMASSE` = 2). Renvoie faux tant qu'il faut attendre.
fn ramassage_pret(ctx: &mut Ctx, o: Obj) -> bool {
    let ramasse = o.u8(ctx.b, champ::RAMASSE);
    if ctx.pris(ramasse == 2) {
        return true;
    }
    let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
    let pret = joueur.u8(ctx.b, 0x33) != 0;
    !ctx.pris(!pret)
}

// ---------------------------------------------------------------------------------------------
// Type 0 : trésors (0xdabca)

/// Type 0, « trésor » : pièces et gemmes ramassées pour des points. Variantes 0 à 5 ; une table de
/// huit octets par variante donne scintillement (+0x54), un octet propre (+0x55), événement de
/// score (+0x56), son (+0x57) et indice de ramassage (+0x5d).
///
/// États : 0 init (en l'air : chute ; au sol : attente 0x4b0 images + scintillement), 2 actif
/// (chute, attente avec clignotement, ramassé : son + score), 4/6 libération.
mod tresor {
    pub const ENTREE: u32 = 0xdabca;
    pub const TABLE_ETATS: u32 = 0xdabe0;
    pub const TABLE_SOUS_ETATS: u32 = 0xdac68;
    /// Paramètres par variante (8 octets) : lus en espace programme.
    pub const PARAMETRES: u32 = 0xdadb6;
    pub const ANIMATIONS: u32 = 0x1bed1a;
    pub const DUREE_AU_SOL: u16 = 0x4b0;
    pub const SCORE: u32 = 0x56;
    pub const SON: u32 = 0x57;
}

/// `0xdad8e` : recopie les paramètres de la variante.
fn tresor_parametres(ctx: &mut Ctx, o: Obj) {
    let base = tresor::PARAMETRES + ((o.u8(ctx.b, Obj::VARIANT) as u32) << 3);
    for (i, off) in [champ::SCINTILLEMENT, 0x55, tresor::SCORE, tresor::SON, champ::INDICE].into_iter().enumerate() {
        let v = ctx.prog8(base + i as u32);
        o.set_u8(ctx.b, off, v);
    }
}

fn tresor_init(ctx: &mut Ctx, o: Obj) {
    let variante = o.u8(ctx.b, Obj::VARIANT);
    if en_l_air(ctx, o) {
        etat_suivant(ctx, o, sous::CHUTE);
        o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
        tresor_parametres(ctx, o);
    } else {
        etat_suivant(ctx, o, sous::ATTENTE);
        o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
        o.set_u16(ctx.b, champ::DUREE, tresor::DUREE_AU_SOL);
        tresor_parametres(ctx, o);
        ctx.jsr(appel::SCINTILLEMENT);
    }
    anime_et_trie(ctx, tresor::ANIMATIONS, variante);
}

fn tresor_actif(ctx: &mut Ctx, o: Obj) {
    let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
    let cible = ctx.saut_table(tresor::TABLE_SOUS_ETATS, sous_etat as u32);
    match sous_etat {
        sous::CHUTE => {
            ctx.jsr(rom::MOVE_XYZ);
            let en_vol = ctx.gt();
            if !ctx.pris(en_vol) {
                // posé : attente au sol
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
                o.set_u16(ctx.b, champ::DUREE, tresor::DUREE_AU_SOL);
                o.set_u32(ctx.b, Obj::Z, 0);
                dans_la_vue(ctx, o);
                ctx.jsr(appel::SCINTILLEMENT);
            }
            fin_de_chute(ctx);
        }
        sous::ATTENTE => {
            // en cinématique, un objet resté à gauche de l'écran disparaît
            let cinematique = ctx.ram8(0x44) != 0;
            if !ctx.pris(!cinematique) {
                let limite = ctx.ram16(0x506).wrapping_sub(0x40);
                let x = o.u16(ctx.b, Obj::X);
                if !ctx.pris((limite as i16) < x as i16) {
                    o.set_u8(ctx.b, Obj::STATE, etat::HORS_ECRAN);
                    return;
                }
            }
            let ramasse = o.u8(ctx.b, champ::RAMASSE);
            if !ctx.pris(ramasse == 0) {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
            }
            expire_et_clignote(ctx, o);
        }
        sous::RAMASSE => {
            if !ramassage_pret(ctx, o) {
                return;
            }
            ajoute_u8(ctx, o, Obj::STATE, 2);
            ctx.c.d[0] = o.u8(ctx.b, tresor::SON) as u32;
            ctx.jsr(appel::SON_RAMASSAGE);
            let joueur_bits = o.u16(ctx.b, 0x48) & 0x300;
            ctx.c.d[1] = joueur_bits as u32;
            ctx.c.d[0] = o.u8(ctx.b, tresor::SCORE) as u32 + joueur_bits as u32;
            ctx.jsr(appel::EVENEMENT);
            marque_ramasse(ctx, o);
        }
        _ => ctx.deleguer(cible),
    }
}

fn tresor(ctx: &mut Ctx) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if ctx.pris(cache) {
        return;
    }
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(tresor::TABLE_ETATS, etat as u32);
    match etat {
        etat::INIT => tresor_init(ctx, o),
        etat::ACTIF => tresor_actif(ctx, o),
        etat::LIBERE | etat::HORS_ECRAN => ctx.free(&pools::FREE_ITEM),
        _ => ctx.deleguer(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 1 : nourriture et potions (0xdae26)

/// Type 1, consommables : variantes 0 et 1 nourriture (rend 20 ou 40 points de vie ; à pleine santé,
/// un score et un son), 2 et 3 potions à durée (0x2d0 images, drapeaux du joueur +0x180..0x184),
/// 4 sans effet, 5 compteur `A5+0x45` (plafonné à 9). Le scintillement (+0x54) et l'indice de
/// ramassage (+0x5d) viennent d'une table de deux octets par variante. Seules les variantes 2 à 5
/// disparaissent au bout de la durée de vie (0x960 images, 0x708 après une chute).
mod conso {
    pub const ENTREE: u32 = 0xdae26;
    pub const TABLE_ETATS: u32 = 0xdae3c;
    pub const TABLE_SOUS_ETATS: u32 = 0xdaef6;
    pub const PARAMETRES: u32 = 0xdaedc;
    pub const EFFETS: u32 = 0xdafb8;
    pub const ANIMATIONS: u32 = 0x1bee16;
    pub const DUREE_INITIALE: u16 = 0x960;
    pub const DUREE_APRES_CHUTE: u16 = 0x708;
    /// Instruction `jsr (A1)` qui appelle l'effet du ramassage.
    pub const APPEL_EFFET: u32 = 0xdafa4;
    /// Effets de ramassage (adresses d'origine, lues dans la table `EFFETS`).
    pub const NOURRITURE: u32 = 0xdafd0;
    pub const POTION_B: u32 = 0xdb044;
    pub const POTION_A: u32 = 0xdb026;
    pub const SANS_EFFET: u32 = 0xdb068;
    pub const COMPTEUR: u32 = 0xdb06a;
}

fn conso_parametres(ctx: &mut Ctx, o: Obj) {
    let base = conso::PARAMETRES + ((o.u8(ctx.b, Obj::VARIANT) as u32) * 2 & 0xff);
    let scintillement = ctx.prog8(base);
    o.set_u8(ctx.b, champ::SCINTILLEMENT, scintillement);
    let indice = ctx.prog8(base + 1);
    o.set_u8(ctx.b, champ::INDICE, indice);
}

fn conso_init(ctx: &mut Ctx, o: Obj) {
    let variante = o.u8(ctx.b, Obj::VARIANT);
    if en_l_air(ctx, o) {
        etat_suivant(ctx, o, sous::CHUTE);
        o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
        o.set_u16(ctx.b, champ::DUREE, conso::DUREE_INITIALE);
        conso_parametres(ctx, o);
    } else {
        etat_suivant(ctx, o, sous::ATTENTE);
        o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
        o.set_u16(ctx.b, champ::DUREE, conso::DUREE_INITIALE);
        conso_parametres(ctx, o);
        ctx.jsr(appel::SCINTILLEMENT);
    }
    anime_et_trie(ctx, conso::ANIMATIONS, variante);
}

/// Effet d'une nourriture : soigne le joueur ; à pleine santé, événement de score et son.
fn effet_nourriture(ctx: &mut Ctx, o: Obj) {
    ctx.at(conso::NOURRITURE);
    let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
    let (vie_max, vie) = (joueur.u16(ctx.b, 0x60), joueur.u16(ctx.b, 0x62));
    if !ctx.pris(vie_max != vie) {
        ctx.c.d[1] = (joueur.u8(ctx.b, 2) as u32) << 8;
        ctx.c.d[0] = 0x11 + ctx.c.d[1];
        ctx.jsr(appel::EVENEMENT);
        ctx.c.d[0] = 5;
        ctx.jsr(appel::SON_RAMASSAGE);
        return;
    }
    let variante = o.u8(ctx.b, Obj::VARIANT);
    let soin: u16 = if ctx.pris(variante == 0) { 0x14 } else { 0x28 };
    let vie = vie.wrapping_add(soin);
    joueur.set_u16(ctx.b, 0x62, vie);
    if !ctx.pris(vie <= vie_max) {
        joueur.set_u16(ctx.b, 0x62, vie_max);
    }
    ctx.jsr(appel::FIN_EFFET);
}

/// Potions : le joueur reçoit un compte à rebours de 0x2d0 images et des drapeaux +0x180..0x184.
fn effet_potion(ctx: &mut Ctx, o: Obj, adresse: u32, octets: &[(u32, u8)]) {
    ctx.at(adresse);
    let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
    joueur.set_u16(ctx.b, 0x180, 0x2d0);
    for &(off, v) in octets {
        joueur.set_u8(ctx.b, off, v);
    }
    ctx.jsr(appel::FIN_EFFET);
}

fn conso_effet(ctx: &mut Ctx, o: Obj) {
    let variante = o.u8(ctx.b, Obj::VARIANT) as u32;
    let cible = ctx.prog32(conso::EFFETS + 4 * variante);
    ctx.jsr_indirect(conso::APPEL_EFFET, |ctx| match cible {
        conso::NOURRITURE => effet_nourriture(ctx, o),
        conso::POTION_B => effet_potion(ctx, o, cible, &[(1, 1), (0x183, 1), (0x184, 0), (0x182, 0)]),
        conso::POTION_A => effet_potion(ctx, o, cible, &[(0x184, 1), (0x183, 0), (0x182, 0)]),
        conso::SANS_EFFET => ctx.at(cible),
        conso::COMPTEUR => {
            ctx.at(cible);
            let n = ctx.ram8(0x45).wrapping_add(1);
            ctx.set_ram8(0x45, n);
            if !ctx.pris(n <= 9) {
                ctx.set_ram8(0x45, 9);
            }
        }
        _ => ctx.deleguer(cible),
    });
    marque_ramasse(ctx, o);
}

fn conso_actif(ctx: &mut Ctx, o: Obj) {
    let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
    let cible = ctx.saut_table(conso::TABLE_SOUS_ETATS, sous_etat as u32);
    match sous_etat {
        sous::CHUTE => {
            ctx.jsr(rom::MOVE_XYZ);
            let en_vol = ctx.gt();
            if !ctx.pris(en_vol) {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
                o.set_u16(ctx.b, champ::DUREE, conso::DUREE_APRES_CHUTE);
                o.set_u32(ctx.b, Obj::Z, 0);
                dans_la_vue(ctx, o);
                ctx.jsr(appel::SCINTILLEMENT);
            }
            fin_de_chute(ctx);
        }
        sous::ATTENTE => {
            let ramasse = o.u8(ctx.b, champ::RAMASSE);
            if !ctx.pris(ramasse == 0) {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
            }
            // seules les variantes 2 et suivantes disparaissent
            let variante = o.u8(ctx.b, Obj::VARIANT);
            if ctx.pris(variante == 0) || ctx.pris(variante == 1) {
                return;
            }
            expire_et_clignote(ctx, o);
        }
        sous::RAMASSE => {
            if !ramassage_pret(ctx, o) {
                return;
            }
            ajoute_u8(ctx, o, Obj::STATE, 2);
            conso_effet(ctx, o);
        }
        _ => ctx.deleguer(cible),
    }
}

fn conso_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if ctx.pris(cache) {
        return;
    }
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(conso::TABLE_ETATS, etat as u32);
    match etat {
        etat::INIT => conso_init(ctx, o),
        etat::ACTIF => conso_actif(ctx, o),
        etat::LIBERE | etat::HORS_ECRAN => ctx.free(&pools::FREE_ITEM),
        _ => ctx.deleguer(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 4 : objets d'inventaire (0xdbdf6)

/// Type 4, objets d'inventaire (fioles, armes secondaires, clés...). Le joueur porte six emplacements
/// (identifiants en `+0x140`, quantités en `+0x148` ; 0x56 : emplacement vide). Au ramassage :
/// refus si la classe du joueur ne peut pas porter l'objet (table de huit octets par variante) ; sinon
/// quantité + `+0x5a` (plafonnée à 9, ou 1 pour les variantes 7, 0x10 et 0x11), emplacement vide, ou
/// échange avec le premier emplacement dont l'objet remplacé est recréé en l'air (mode 1). Un objet
/// refusé est rejeté en l'air puis redevient ramassable. Le mode de création `+0x2f` vaut 1 pour un
/// objet jeté par un joueur : il attend `+0x59` images avant de paraître.
mod inventaire {
    pub const ENTREE: u32 = 0xdbdf6;
    pub const TABLE_ETATS: u32 = 0xdbe0c;
    pub const TABLE_SOUS_ETATS: u32 = 0xdbf36;
    pub const ANIMATIONS: u32 = 0x1bf3c6;
    /// Scintillement par variante ; indice de ramassage par variante.
    pub const SCINTILLEMENTS: u32 = 0xdbf04;
    pub const INDICES: u32 = 0xdbf16;
    /// Par variante, 8 octets : identifiant de l'objet puis un octet d'interdiction par classe.
    pub const FICHES: u32 = 0xdc1fa;
    /// Par classe de joueur, 8 identifiants d'objets qui ne peuvent pas être remplacés.
    pub const VERROUILLES: u32 = 0xdc1ca;
    /// Par identifiant d'objet : (type, variante) de l'ITEM recréé quand il est remplacé.
    pub const RECREES: u32 = 0xdc2d6;
    pub const DUREE_AU_SOL: u16 = 0x4b0;
    pub const VIDE: u8 = 0x56;
    /// Champs du joueur.
    pub const IDENTIFIANTS: u32 = 0x140;
    pub const QUANTITES: u32 = 0x148;
    /// Quantité de l'objet (`+0x5a`) et délai avant apparition (`+0x59`).
    pub const QUANTITE: u32 = 0x5a;
    pub const DELAI: u32 = 0x59;
}

/// `0xdc28a` : recrée en l'air l'objet `ancien` (remplacé dans l'inventaire) avec sa quantité.
fn objet_expulse(ctx: &mut Ctx, o: Obj, joueur: Obj, ancien: u8, quantite: u8) {
    let Some(n) = ctx.alloc_item() else {
        ctx.pris(true); // bcs : pool plein
        return;
    };
    ctx.pris(false);
    n.set_u8(ctx.b, Obj::ACTIVE, 1);
    n.set_u8(ctx.b, champ::MODE, 1);
    n.set_u8(ctx.b, inventaire::QUANTITE, quantite);
    let rang = ((ancien as u32) * 2) & 0xffff;
    let kind = ctx.prog8(inventaire::RECREES + rang);
    n.set_u8(ctx.b, Obj::KIND, kind);
    let variante = ctx.prog8(inventaire::RECREES + rang + 1);
    n.set_u8(ctx.b, Obj::VARIANT, variante);
    n.set_u16(ctx.b, champ::PROPRIETAIRE, joueur.0 as u16);
    let x = o.u16(ctx.b, Obj::X);
    n.set_u16(ctx.b, Obj::X, x);
    let y = joueur.u16(ctx.b, Obj::Y).wrapping_sub(1);
    n.set_u16(ctx.b, Obj::Y, y);
    n.set_u16(ctx.b, Obj::Z, 0x10);
    n.set_u16(ctx.b, Obj::AZ, 0xffc0);
    n.set_u8(ctx.b, inventaire::DELAI, 0xc);
}

/// Ramassage refusé (`0xdc19a`) : l'objet sera rejeté (sous-état 6).
fn ramassage_refuse(ctx: &mut Ctx, o: Obj) -> bool {
    o.set_u8(ctx.b, champ::SOUS_ETAT, 6);
    o.set_u8(ctx.b, champ::SOUS_SOUS_ETAT, 0);
    o.set_u8(ctx.b, inventaire::DELAI, 0xc);
    true
}

/// Événement de score et son d'un objet qui ne change rien d'autre (`0xdc124`).
fn evenement_inventaire(ctx: &mut Ctx, joueur: Obj) {
    let personnage = joueur.u8(ctx.b, 2) as u32;
    ctx.c.d[1] = 0x11;
    ctx.c.d[0] = (personnage << 8) + 0x11;
    ctx.jsr(appel::EVENEMENT);
    ctx.c.d[0] = 5;
    ctx.jsr(appel::SON_RAMASSAGE);
}

/// `0xdc0aa` : range l'objet dans l'inventaire du propriétaire. Renvoie vrai si le ramassage est refusé.
fn inventaire_ajoute(ctx: &mut Ctx, o: Obj) -> bool {
    let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
    let ligne = (o.u8(ctx.b, Obj::VARIANT) as u32) << 3 & 0xffff;
    let id = ctx.rom8(inventaire::FICHES + ligne);
    let classe = joueur.u8(ctx.b, 3) >> 1;
    let interdit = ctx.rom8(inventaire::FICHES + 1 + (classe.wrapping_add(ligne as u8)) as u32);
    if ctx.pris(interdit != 0) {
        return ramassage_refuse(ctx, o);
    }
    if !ctx.pris(id != 0x23) && ctx.pris(classe == 2) {
        evenement_inventaire(ctx, joueur);
        return false;
    }
    let quantite = o.u8(ctx.b, inventaire::QUANTITE);
    // 1. l'objet est déjà porté : la quantité augmente
    for i in 0..6u32 {
        let porte = joueur.u8(ctx.b, inventaire::IDENTIFIANTS + i);
        if ctx.pris(porte != id) {
            ctx.pris(i < 5); // dbf
            continue;
        }
        let n = joueur.u8(ctx.b, inventaire::QUANTITES + i).wrapping_add(quantite);
        joueur.set_u8(ctx.b, inventaire::QUANTITES + i, n);
        let variante = o.u8(ctx.b, Obj::VARIANT);
        // variantes à exemplaire unique : 7, 0x10, 0x11
        let unique = ctx.pris(variante == 7) || ctx.pris(variante == 0x10) || ctx.pris(variante == 0x11);
        if unique {
            joueur.set_u8(ctx.b, inventaire::QUANTITES + i, 1);
        } else if !ctx.pris(n <= 9) {
            joueur.set_u8(ctx.b, inventaire::QUANTITES + i, 9);
        } else {
            return false;
        }
        evenement_inventaire(ctx, joueur);
        return false;
    }
    // 2. un emplacement vide
    for i in 0..6u32 {
        let porte = joueur.u8(ctx.b, inventaire::IDENTIFIANTS + i);
        if ctx.pris(porte != inventaire::VIDE) {
            ctx.pris(i < 5);
            continue;
        }
        joueur.set_u8(ctx.b, inventaire::IDENTIFIANTS + i, id);
        joueur.set_u8(ctx.b, inventaire::QUANTITES + i, quantite);
        return false;
    }
    // 3. échange avec le premier emplacement, sauf s'il est verrouillé pour cette classe
    let premier = joueur.u8(ctx.b, inventaire::IDENTIFIANTS);
    for k in 0..8u32 {
        let verrou = ctx.rom8(inventaire::VERROUILLES + ((classe as u32) << 3) + k);
        if !ctx.pris(premier != verrou) {
            return ramassage_refuse(ctx, o);
        }
        ctx.pris(k < 7);
    }
    let ancienne_quantite = joueur.u8(ctx.b, inventaire::QUANTITES);
    joueur.set_u8(ctx.b, inventaire::IDENTIFIANTS, id);
    joueur.set_u8(ctx.b, inventaire::QUANTITES, quantite);
    objet_expulse(ctx, o, joueur, premier, ancienne_quantite);
    false
}

fn inventaire_init(ctx: &mut Ctx, o: Obj) {
    let variante = o.u8(ctx.b, Obj::VARIANT);
    let mode = o.u8(ctx.b, champ::MODE);
    if ctx.pris(mode != 0) {
        // objet jeté : attend la fin du délai
        let delai = o.u8(ctx.b, inventaire::DELAI);
        if !ctx.pris(delai == 0) {
            o.set_u8(ctx.b, inventaire::DELAI, delai - 1);
            return;
        }
        ajoute_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
        let indice = ctx.rom8(inventaire::INDICES + variante as u32);
        o.set_u8(ctx.b, champ::INDICE, indice);
        anime_et_trie(ctx, inventaire::ANIMATIONS, variante);
        return;
    }
    if en_l_air(ctx, o) {
        etat_suivant(ctx, o, sous::CHUTE);
        o.set_u8(ctx.b, inventaire::QUANTITE, 1);
        o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
        let scintillement = ctx.rom8(inventaire::SCINTILLEMENTS + variante as u32);
        o.set_u8(ctx.b, champ::SCINTILLEMENT, scintillement);
        let indice = ctx.rom8(inventaire::INDICES + variante as u32);
        o.set_u8(ctx.b, champ::INDICE, indice);
    } else {
        etat_suivant(ctx, o, sous::ATTENTE);
        o.set_u8(ctx.b, inventaire::QUANTITE, 1);
        o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
        o.set_u16(ctx.b, champ::DUREE, inventaire::DUREE_AU_SOL);
        let scintillement = ctx.rom8(inventaire::SCINTILLEMENTS + variante as u32);
        o.set_u8(ctx.b, champ::SCINTILLEMENT, scintillement);
        let indice = ctx.rom8(inventaire::INDICES + variante as u32);
        o.set_u8(ctx.b, champ::INDICE, indice);
        ctx.jsr(appel::SCINTILLEMENT);
    }
    anime_et_trie(ctx, inventaire::ANIMATIONS, variante);
}

/// Sous-état 6 : l'objet refusé est rejeté en arc vers l'extérieur de l'écran puis retombe.
fn inventaire_rejet(ctx: &mut Ctx, o: Obj) {
    let lance = o.u8(ctx.b, champ::SOUS_SOUS_ETAT) != 0;
    if !ctx.pris(lance) {
        // attente du délai, puis lancer
        let delai = o.u8(ctx.b, inventaire::DELAI).wrapping_sub(1);
        o.set_u8(ctx.b, inventaire::DELAI, delai);
        if ctx.pris(delai != 0) {
            return;
        }
        ajoute_u8(ctx, o, champ::SOUS_SOUS_ETAT, 2);
        let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
        let y = joueur.u16(ctx.b, Obj::Y).wrapping_sub(1);
        o.set_u16(ctx.b, Obj::Y, y);
        o.set_u16(ctx.b, Obj::Z, 0x10);
        o.set_u16(ctx.b, Obj::VZ, 0);
        o.set_u16(ctx.b, Obj::AZ, 0xffc0);
        o.set_u32(ctx.b, Obj::VX, 0x0200_0000);
        let (cam_x, _) = ctx.camera();
        let limite = cam_x.wrapping_add(0xc0);
        let x = o.u16(ctx.b, Obj::X);
        if !ctx.pris(limite as i16 > x as i16) {
            let v = o.u32(ctx.b, Obj::VX).wrapping_neg();
            o.set_u32(ctx.b, Obj::VX, v);
        }
        ctx.reinsert();
        return;
    }
    ctx.jsr(rom::MOVE_XZ);
    let z = o.u16(ctx.b, Obj::Z);
    if ctx.pris(z as i16 >= 0) {
        return;
    }
    o.set_u8(ctx.b, champ::SOUS_ETAT, sous::ATTENTE);
    o.set_u8(ctx.b, champ::SOUS_SOUS_ETAT, 0);
    o.set_u32(ctx.b, Obj::Z, 0);
    o.set_u8(ctx.b, champ::RAMASSE, 0);
}

fn inventaire_actif(ctx: &mut Ctx, o: Obj) {
    let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
    let cible = ctx.saut_table(inventaire::TABLE_SOUS_ETATS, sous_etat as u32);
    match sous_etat {
        sous::CHUTE => {
            ctx.jsr(rom::MOVE_XYZ);
            let en_vol = ctx.gt();
            if !ctx.pris(en_vol) {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
                o.set_u16(ctx.b, champ::DUREE, inventaire::DUREE_AU_SOL);
                o.set_u32(ctx.b, Obj::Z, 0);
                let mode = o.u8(ctx.b, champ::MODE);
                if !ctx.pris(mode != 0) {
                    dans_la_vue(ctx, o);
                    ctx.jsr(appel::SCINTILLEMENT);
                }
            }
            ctx.anim_step();
            fin_de_chute(ctx);
        }
        sous::ATTENTE => {
            let ramasse = o.u8(ctx.b, champ::RAMASSE);
            if !ctx.pris(ramasse == 0) {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
            }
            ctx.anim_step();
            expire_et_clignote(ctx, o);
        }
        sous::RAMASSE => {
            let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
            let pret = joueur.u8(ctx.b, 0x33) != 0;
            if ctx.pris(!pret) {
                return;
            }
            let refuse = inventaire_ajoute(ctx, o);
            if !ctx.pris(refuse) {
                ajoute_u8(ctx, o, Obj::STATE, 2);
            }
            marque_ramasse(ctx, o);
        }
        6 => inventaire_rejet(ctx, o),
        _ => ctx.deleguer(cible),
    }
}

fn inventaire_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if ctx.pris(cache) {
        return;
    }
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(inventaire::TABLE_ETATS, etat as u32);
    match etat {
        etat::INIT => inventaire_init(ctx, o),
        etat::ACTIF => inventaire_actif(ctx, o),
        etat::LIBERE | etat::HORS_ECRAN => ctx.free(&pools::FREE_ITEM),
        _ => ctx.deleguer(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 5 : bonus animés (0xdc382)

/// Type 5, bonus animés : lancés en l'air selon un mode de création (`+0x2f` : vitesses initiales
/// lues dans une table de huit octets par mode), ils retombent, attendent 0x4b0 images en clignotant
/// et donnent au ramassage un événement de score (`0x19b08`, variante + joueur).
mod bonus {
    pub const ENTREE: u32 = 0xdc382;
    pub const TABLE_ETATS: u32 = 0xdc398;
    pub const TABLE_SOUS_ETATS: u32 = 0xdc4f2;
    pub const ANIMATIONS: u32 = 0x1bf5c2;
    /// Par mode : vx, vz, az, vy (mots), lus en espace programme.
    pub const VITESSES: u32 = 0xdc42c;
    pub const DUREE_AU_SOL: u16 = 0x4b0;
    /// Écart entre l'animation en chute et l'animation au sol, et animation au sol d'un objet posé.
    pub const ANIMATION_SOL: u8 = 6;
    pub const ANIMATION_POSE: u8 = 0xc;
    /// Événement de score.
    pub const EVENEMENT: u32 = 0x19b08;
}

fn bonus_init(ctx: &mut Ctx, o: Obj) {
    let variante = o.u8(ctx.b, Obj::VARIANT);
    if en_l_air(ctx, o) {
        etat_suivant(ctx, o, sous::CHUTE);
        o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
        let mode = o.u8(ctx.b, champ::MODE);
        if !ctx.pris(mode == 0) {
            let base = bonus::VITESSES + ((mode as u32) << 3);
            for (i, off) in [Obj::VX, Obj::VZ, Obj::AZ, Obj::VY].into_iter().enumerate() {
                let v = ctx.prog16(base + 2 * i as u32);
                o.set_u16(ctx.b, off, v);
            }
        }
        anime_et_trie(ctx, bonus::ANIMATIONS, variante);
    } else {
        etat_suivant(ctx, o, sous::ATTENTE);
        o.set_u16(ctx.b, champ::DUREE, bonus::DUREE_AU_SOL);
        o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
        anime_et_trie(ctx, bonus::ANIMATIONS, variante.wrapping_add(bonus::ANIMATION_POSE));
    }
}

fn bonus_actif(ctx: &mut Ctx, o: Obj) {
    let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
    let cible = ctx.saut_table(bonus::TABLE_SOUS_ETATS, sous_etat as u32);
    match sous_etat {
        sous::CHUTE => {
            ctx.anim_step();
            ctx.jsr(rom::MOVE_XYZ);
            let en_vol = ctx.gt();
            if !ctx.pris(en_vol) {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
                o.set_u16(ctx.b, champ::DUREE, bonus::DUREE_AU_SOL);
                o.set_u32(ctx.b, Obj::Z, 0);
                let variante = o.u8(ctx.b, Obj::VARIANT);
                ctx.anim_start(bonus::ANIMATIONS, variante.wrapping_add(bonus::ANIMATION_SOL) as u16);
                dans_la_vue(ctx, o);
            }
            fin_de_chute(ctx);
        }
        sous::ATTENTE => {
            let ramasse = o.u8(ctx.b, champ::RAMASSE);
            if !ctx.pris(ramasse == 0) {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
            }
            ctx.anim_step();
            expire_et_clignote(ctx, o);
        }
        sous::RAMASSE => {
            if !ramassage_pret(ctx, o) {
                return;
            }
            ajoute_u8(ctx, o, Obj::STATE, 2);
            let joueur_bits = o.u16(ctx.b, 0x48) & 0x300;
            ctx.c.d[1] = joueur_bits as u32;
            ctx.c.d[0] = o.u8(ctx.b, Obj::VARIANT) as u32 + joueur_bits as u32;
            ctx.jsr(bonus::EVENEMENT);
        }
        _ => ctx.deleguer(cible),
    }
}

fn bonus_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if ctx.pris(cache) {
        return;
    }
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(bonus::TABLE_ETATS, etat as u32);
    match etat {
        etat::INIT => bonus_init(ctx, o),
        etat::ACTIF => bonus_actif(ctx, o),
        etat::LIBERE | etat::HORS_ECRAN => ctx.free(&pools::FREE_ITEM),
        _ => ctx.deleguer(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 6 : objets de scène (0xdc624)

/// Type 6, objets de scène attachés à un joueur (`+0x50`). Quatre variantes, trois automates :
///
/// - variantes 0 et 2, « icône de joueur » (h) : suit le joueur (position du joueur décalée de
///   0x20 pixels, ou 0x18 si retournée, un pixel devant lui, 0x18 au-dessus), joue une animation
///   tirée au sort (4) et un son toutes les 0x3c images (hors pause `A5+0x42`) ;
/// - variante 1, « guide » (h) : choisit le joueur vivant le plus proche, marche vers un point à 0x20
///   pixels de lui par table de vitesses, s'arrête, crée des objets SET (`0xc8312`) et pilote le
///   compteur `A5-0x55c8` ;
/// - variante 3 : voir le code d'origine (non exercée, déléguée).
///
/// États communs : 0 initialisation, 2 actif, 4 fin (variante 1), 6 libération. Les sous-états 2 et 4
/// de l'icône (envol, ramassage) et l'état 4 de l'icône ne sont pas exercés : ils restent en code
/// d'origine.
mod scene {
    pub const ENTREE: u32 = 0xdc624;
    pub const TABLE_VARIANTES: u32 = 0xdc63c;
    pub const ANIMATIONS: u32 = 0x1bf8fe;
    // icône
    pub const ICONE_ETATS: u32 = 0xdc654;
    pub const ICONE_SOUS_ETATS: u32 = 0xdc6ac;
    pub const ICONE_ANIMATIONS: u32 = 0xdc69c;
    pub const ICONE_SONS: u32 = 0xdc6b2;
    pub const DECALAGES_X: u32 = 0xdcdd8;
    pub const DELAI_SON: u8 = 0x3c;
    // guide
    pub const GUIDE_ETATS: u32 = 0xdcb30;
    pub const GUIDE_SOUS_ETATS: u32 = 0xdcb92;
    pub const GUIDE_ANIMATIONS: u32 = 0xdcb6e;
    pub const GUIDE_ANIMATIONS_ARRIVEE: u32 = 0xdcbbc;
    pub const TABLE_VITESSES: u32 = 0x1c65b6;
    /// Appel de `jsr (d8,PC,Xn)` du guide.
    pub const GUIDE_APPEL: u32 = 0xdcb88;
    /// Champs.
    pub const DELAI: u32 = 0x4c;
    pub const CIBLE_X: u32 = 0x52;
    pub const CIBLE_Y: u32 = 0x54;
    pub const MINUTEUR: u32 = 0x56;
    pub const CREE: u32 = 0x5c;
    /// Variables de scène.
    pub const PAUSE: i32 = 0x42;
    pub const SCENE_FIGEE: i32 = 0x3d;
    pub const NIVEAU: i32 = -0x5ac6;
    pub const COMPTEUR: i32 = -0x55c8;
    pub const JOUEURS: u32 = 0xff85de;
    pub const PAS_JOUEUR: u32 = 0x200;
    // sous-routines non réécrites
    pub const ANGLE: u32 = 0x137e;
    pub const CREER_SET: u32 = 0xc8312;
}

/// `0xdcdae` : se place à côté du propriétaire (un pixel devant, décalé selon l'orientation).
/// Renvoie le propriétaire.
fn suit_proprietaire(ctx: &mut Ctx, o: Obj) -> Obj {
    let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
    let x = joueur.u32(ctx.b, Obj::X);
    o.set_u32(ctx.b, Obj::X, x);
    let y = joueur.u32(ctx.b, Obj::Y);
    o.set_u32(ctx.b, Obj::Y, y);
    let y = o.u16(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u16(ctx.b, Obj::Y, y);
    let retourne = o.u8(ctx.b, 0x2c) != 0;
    let decalage = if ctx.pris(!retourne) { 0 } else { 2 };
    let dx = ctx.prog16(scene::DECALAGES_X + decalage);
    let x = o.u16(ctx.b, Obj::X).wrapping_add(dx);
    o.set_u16(ctx.b, Obj::X, x);
    joueur
}

fn icone_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, champ::SOUS_ETAT, 0);
    let joueur = suit_proprietaire(ctx, o);
    let z = joueur.u32(ctx.b, Obj::Z);
    o.set_u32(ctx.b, Obj::Z, z);
    let z = o.u16(ctx.b, Obj::Z).wrapping_add(0x18);
    o.set_u16(ctx.b, Obj::Z, z);
    o.set_u8(ctx.b, scene::DELAI, scene::DELAI_SON);
    o.set_u8(ctx.b, Obj::ACTIVE, 2);
    ctx.jsr(rom::RNG);
    let tirage = ctx.c.d[0] as u16 & 3;
    ctx.c.d[0] = ctx.c.d[0] & 0xffff_0000 | tirage as u32;
    o.set_u8(ctx.b, champ::MODE, tirage as u8);
    let animation = ctx.prog8(scene::ICONE_ANIMATIONS + tirage as u32);
    ctx.anim_start(scene::ANIMATIONS, animation as u16);
}

/// Sous-état 0 de l'icône : suit le joueur, anime, trie, et joue le son tiré toutes les 0x3c images.
fn icone_suivi(ctx: &mut Ctx, o: Obj) {
    suit_proprietaire(ctx, o);
    ctx.anim_step();
    o.set_u8(ctx.b, 0x3b, 0);
    ctx.reinsert();
    // 0xdc6e8
    let en_pause = ctx.ram8(scene::PAUSE) != 0;
    if ctx.pris(en_pause) {
        return;
    }
    let reste = o.u8(ctx.b, scene::DELAI).wrapping_sub(1);
    o.set_u8(ctx.b, scene::DELAI, reste);
    if ctx.pris(reste != 0) {
        return;
    }
    o.set_u8(ctx.b, scene::DELAI, scene::DELAI_SON);
    let tirage = o.u8(ctx.b, champ::MODE) as u32;
    let son = ctx.prog32(scene::ICONE_SONS + tirage * 4);
    ctx.jsr(son);
}

fn icone(ctx: &mut Ctx, o: Obj) {
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(scene::ICONE_ETATS, etat as u32);
    match etat {
        etat::INIT => icone_init(ctx, o),
        etat::ACTIF => {
            let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
            let cible = ctx.saut_table(scene::ICONE_SOUS_ETATS, sous_etat as u32);
            match sous_etat {
                sous::CHUTE => icone_suivi(ctx, o),
                _ => ctx.deleguer(cible),
            }
        }
        6 => ctx.free(&pools::FREE_ITEM),
        _ => ctx.deleguer(cible),
    }
}

/// `0xdcddc` : place le guide au bord droit de l'écran et choisit le joueur vivant le plus proche,
/// dont il retient le propriétaire (`+0x50`), la hauteur (`+0x54`) et un point d'arrivée (`+0x52`).
fn guide_cible(ctx: &mut Ctx, o: Obj) {
    let cam_x = ctx.ram16(0x506);
    let cam_y = ctx.ram16(0x50a);
    o.set_u16(ctx.b, Obj::X, cam_x.wrapping_add(0x1a0));
    o.set_u16(ctx.b, Obj::Y, cam_y.wrapping_add(0x40));
    o.set_u32(ctx.b, Obj::Z, 0);
    o.set_u8(ctx.b, 0x2c, 0);
    let mut meilleur: u16 = 0xffff;
    let mut choisi = ctx.c.a[2];
    for i in 0..4u32 {
        let joueur = Obj(scene::JOUEURS + i * scene::PAS_JOUEUR);
        let actif = joueur.u8(ctx.b, Obj::ACTIVE) != 0;
        let vivant = !ctx.pris(!actif) && {
            let etat = joueur.u8(ctx.b, Obj::STATE) as i8;
            !ctx.pris(etat >= 4)
        };
        if vivant {
            let mut dx = o.u16(ctx.b, Obj::X).wrapping_sub(joueur.u16(ctx.b, Obj::X));
            if !ctx.pris(dx as i16 >= 0) {
                dx = dx.wrapping_neg();
            }
            let mut dy = o.u16(ctx.b, Obj::Y).wrapping_sub(joueur.u16(ctx.b, Obj::Y));
            if !ctx.pris(dy as i16 >= 0) {
                dy = dy.wrapping_neg();
            }
            let distance = dx.wrapping_add(dy);
            if !ctx.pris(meilleur <= distance) {
                meilleur = distance;
                choisi = joueur.0;
            }
        }
        ctx.pris(i < 3); // dbf
    }
    o.set_u16(ctx.b, champ::PROPRIETAIRE, choisi as u16);
    let cible = Obj(choisi);
    let y = cible.u16(ctx.b, Obj::Y);
    o.set_u16(ctx.b, scene::CIBLE_Y, y);
    let ecart = cible.u16(ctx.b, Obj::X).wrapping_sub(cam_x);
    let decalage: u16 = if ctx.pris(ecart <= 0x140) { 0x20 } else { 0x20u16.wrapping_neg() };
    let x = cible.u16(ctx.b, Obj::X).wrapping_add(decalage);
    o.set_u16(ctx.b, scene::CIBLE_X, x);
}

fn guide_init(ctx: &mut Ctx, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, champ::SOUS_ETAT, 0);
    guide_cible(ctx, o);
    o.set_u32(ctx.b, Obj::SPEED_TABLE, scene::TABLE_VITESSES);
    let niveau = ctx.ram8(scene::NIVEAU) as u32;
    let animation = ctx.prog8(scene::GUIDE_ANIMATIONS + 2 * niveau);
    let mode = ctx.prog8(scene::GUIDE_ANIMATIONS + 2 * niveau + 1);
    o.set_u8(ctx.b, champ::MODE, mode);
    ctx.anim_start(scene::ANIMATIONS, animation as u16);
}

/// `0xdcb9a` : animation du guide selon son mode (0x12 pour la variante 3).
fn guide_animation(ctx: &mut Ctx, o: Obj) {
    let mut animation = 0x12;
    let variante = o.u8(ctx.b, Obj::VARIANT);
    if !ctx.pris(variante == 3) {
        let mode = o.u8(ctx.b, champ::MODE) as u32;
        animation = ctx.prog8(scene::GUIDE_ANIMATIONS_ARRIVEE + mode);
    }
    ctx.anim_start(scene::ANIMATIONS, animation as u16);
}

/// Crée un objet SET de type `type_set` ; renvoie son adresse, ou `None` si le pool est plein.
fn cree_set(ctx: &mut Ctx, type_set: u32) -> Option<Obj> {
    ctx.c.d[0] = type_set;
    ctx.jsr(scene::CREER_SET);
    let echec = ctx.c.cond(5);
    (!ctx.pris(echec)).then(|| Obj(ctx.c.a[1]))
}

/// Sous-état 0 du guide : marche vers le point d'arrivée ; arrivé, change d'animation et crée un SET.
fn guide_marche(ctx: &mut Ctx, o: Obj) {
    let cible_x = o.u16(ctx.b, scene::CIBLE_X);
    let cible_y = o.u16(ctx.b, scene::CIBLE_Y);
    ctx.c.d[1] = ctx.c.d[1] & 0xffff_0000 | cible_x as u32;
    ctx.c.d[2] = ctx.c.d[2] & 0xffff_0000 | cible_y as u32;
    ctx.jsr(scene::ANGLE);
    o.set_u8(ctx.b, Obj::SPEED_INDEX, ctx.c.d[0] as u8);
    ctx.natif(&crate::objects::motion::MOVE_BY_TABLE_XY);
    let dx = cible_x.wrapping_sub(o.u16(ctx.b, Obj::X)).wrapping_add(8);
    let dy = cible_y.wrapping_sub(o.u16(ctx.b, Obj::Y)).wrapping_add(8);
    let arrive = !ctx.pris(dx > 0x10) && !ctx.pris(dy > 0x10);
    if arrive {
        ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
        guide_animation(ctx, o);
        o.set_u16(ctx.b, scene::MINUTEUR, 0x3c);
        o.set_u16(ctx.b, scene::CREE, 0);
        let variante = o.u8(ctx.b, Obj::VARIANT);
        let type_set = if ctx.pris(variante != 3) { 1 } else { 0x26 };
        if let Some(n) = cree_set(ctx, type_set) {
            o.set_u16(ctx.b, scene::CREE, n.0 as u16);
        }
        let proprietaire = pointeur(ctx, o, champ::PROPRIETAIRE);
        let x = o.u16(ctx.b, Obj::X);
        let x_proprietaire = proprietaire.u16(ctx.b, Obj::X);
        if !ctx.pris(x > x_proprietaire) {
            o.set_u8(ctx.b, 0x2c, 0x20);
        }
        let (a, b) = (o.u8(ctx.b, 0x2c), proprietaire.u8(ctx.b, 0x2c));
        if !ctx.pris(a != b) {
            let v = proprietaire.u8(ctx.b, 0x2c) ^ 0x20;
            proprietaire.set_u8(ctx.b, 0x2c, v);
        }
    }
    ctx.jsr(rom::ANIM_STEP);
}

/// Sous-état 2 du guide : attend la fin de la minuterie ou la disparition de l'objet SET créé.
fn guide_attente(ctx: &mut Ctx, o: Obj) {
    let cree = o.u16(ctx.b, scene::CREE);
    if !ctx.pris(cree == 0) {
        let set = pointeur(ctx, o, scene::CREE);
        let fini = set.u8(ctx.b, 0x22) != 0;
        if !ctx.pris(fini) {
            return;
        }
    } else {
        let reste = o.u16(ctx.b, scene::MINUTEUR).wrapping_sub(1);
        o.set_u16(ctx.b, scene::MINUTEUR, reste);
        if ctx.pris(reste != 0) {
            return;
        }
    }
    ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
    let n = ctx.ram8(scene::COMPTEUR).wrapping_add(4);
    ctx.set_ram8(scene::COMPTEUR, n);
}

/// Sous-état 4 du guide : quand le compteur de scène atteint 0x10, crée un second SET.
fn guide_second_objet(ctx: &mut Ctx, o: Obj) {
    let compteur = ctx.ram8(scene::COMPTEUR);
    if ctx.pris(compteur != 0x10) {
        return;
    }
    o.set_u16(ctx.b, scene::CREE, 0);
    if let Some(n) = cree_set(ctx, 2) {
        ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
        o.set_u16(ctx.b, scene::CREE, n.0 as u16);
        let mode = o.u8(ctx.b, champ::MODE);
        n.set_u8(ctx.b, 0x42, mode);
    }
}

/// Sous-état 6 du guide : attend la fin de l'objet SET, puis passe à l'état 4.
fn guide_fin(ctx: &mut Ctx, o: Obj) {
    let set = pointeur(ctx, o, scene::CREE);
    let en_cours = set.u8(ctx.b, 0x22) == 0;
    if !ctx.pris(en_cours) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
}

fn guide_actif(ctx: &mut Ctx, o: Obj) {
    let fige = ctx.ram8(scene::SCENE_FIGEE) != 0;
    if !ctx.pris(fige) {
        let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
        ctx.jsr_indirect(scene::GUIDE_APPEL, |ctx| {
            let cible = ctx.saut_table(scene::GUIDE_SOUS_ETATS, sous_etat as u32);
            match sous_etat {
                0 => guide_marche(ctx, o),
                2 => guide_attente(ctx, o),
                4 => guide_second_objet(ctx, o),
                6 => guide_fin(ctx, o),
                _ => ctx.deleguer(cible),
            }
        });
    }
    ctx.reinsert();
}

fn guide(ctx: &mut Ctx, o: Obj) {
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(scene::GUIDE_ETATS, etat as u32);
    match etat {
        etat::INIT => guide_init(ctx, o),
        etat::ACTIF => guide_actif(ctx, o),
        etat::LIBERE => {
            let n = ctx.ram8(scene::COMPTEUR).wrapping_add(4);
            ctx.set_ram8(scene::COMPTEUR, n);
        }
        etat::HORS_ECRAN => ctx.free(&pools::FREE_ITEM),
        _ => ctx.deleguer(cible),
    }
}

fn scene_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if ctx.pris(cache) {
        return;
    }
    let variante = o.u8(ctx.b, Obj::VARIANT);
    let cible = ctx.saut_table(scene::TABLE_VARIANTES, 2 * variante as u32);
    match variante {
        0 | 2 => icone(ctx, o),
        1 => guide(ctx, o),
        _ => ctx.deleguer(cible),
    }
}

gestionnaire!(ITEM_0, tresor::ENTREE, "item_tresor", tresor);
gestionnaire!(ITEM_1, conso::ENTREE, "item_conso", conso_gestionnaire);
gestionnaire!(ITEM_4, inventaire::ENTREE, "item_inventaire", inventaire_gestionnaire);
gestionnaire!(ITEM_5, bonus::ENTREE, "item_bonus", bonus_gestionnaire);
gestionnaire!(ITEM_6, scene::ENTREE, "item_scene", scene_gestionnaire);
