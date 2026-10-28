//! Famille ITEM (classe 0xC) : objets ramassables. Dispatch `0x39f34`, table des types `0x39f5a`
//! (10 types, 32 emplacements de 0x60 octets).
//!
//! Un objet ITEM naît (créé par un ennemi, un coffre ou un décor) à la position de son créateur ;
//! il tombe s'il est en l'air, reste au sol un temps limité en clignotant avant de disparaître, et
//! produit son effet quand un joueur le ramasse (le code du joueur pose `RAMASSE` et `PROPRIETAIRE`).
//! Voir `docs/familles.md` pour la description de chaque type.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::mem::{rd16, rts, wr16, wr32, A5};
use crate::objects::pools;
use crate::objects::Obj;
use crate::{regs, Native};

use super::{ajoute_u8, effects_creators, gestionnaire, pointeur, Ctx};
use crate::objects::Courant;
use crate::machine::Machine;

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
    pub const FLIP: u32 = 0x2c;
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
    /// Fin commune des effets de potion.
    pub const FIN_EFFET: u32 = 0xebaf6;
}

/// `0xae3ec` : scintillement (EFFCT 15) au-dessus de l'objet, d'après `SCINTILLEMENT`.
fn scintille(ctx: &mut Ctx, o: Obj) {
    effects_creators::twinkle(ctx.b, o);
}

/// `0xb12c0` (D0 = `variante`) : effet de ramassage (EFFCT 27) attaché au joueur propriétaire de l'objet.
fn effet_ramassage(ctx: &mut Ctx, o: Obj, variante: u8) {
    effects_creators::pickup(ctx.b, o, variante);
}

/// `0xc8312` : objet SET de la scène ; `None` si le pool est plein.
fn set_de_scene(ctx: &mut Ctx, variante: u8) -> Option<Obj> {
    super::creators::scene_set(ctx.b, variante)
}

/// `0xaba16` : éclat à animation unique (EFFCT 7) sur l'objet.
fn eclat(ctx: &mut Ctx, o: Obj) {
    effects_creators::flash(ctx.b, o);
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
    !(z == 0) // tst.w (0xc,A0) ; beq : au sol
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
    if !(y_min < y) {
        o.set_u16(ctx.b, Obj::Y, y_min);
    }
    let x_min = cam_x.wrapping_add(0x18);
    let x = o.u16(ctx.b, Obj::X);
    if !(x_min as i16 <= x as i16) {
        o.set_u16(ctx.b, Obj::X, x_min);
    } else {
        let x_max = cam_x.wrapping_add(0x168);
        if !(x_max > x) {
            o.set_u16(ctx.b, Obj::X, x_max);
        }
    }
}

/// Compte à rebours de la durée de vie ; sous 0x3c images l'objet clignote (le bit 1 de `FLAGS`
/// cache l'objet une image sur deux, la phase est donnée par `D7`) ; à zéro il est libéré.
fn expire_et_clignote(ctx: &mut Ctx, o: Obj) {
    let reste = o.u16(ctx.b, champ::DUREE).wrapping_sub(1);
    o.set_u16(ctx.b, champ::DUREE, reste);
    if !(reste as i16 >= 0) {
        o.set_u8(ctx.b, Obj::STATE, etat::LIBERE);
        o.set_u8(ctx.b, Obj::ACTIVE, 2);
    }
    if reste > 0x3c {
        return;
    }
    let phase = reste.wrapping_add(ctx.loop_counter());
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    if phase & 1 == 0 {
        return;
    }
    o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
}

/// Fin commune de l'état de chute : ramener dans la zone, ombre, tri par profondeur.
fn fin_de_chute(ctx: &mut Ctx) {
    crate::ai::keep_in_zone_d5(ctx.c, ctx.b, ctx.obj());
    crate::terrain::push_68k(ctx.c, ctx.b, ctx.obj());
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
    if ramasse == 2 {
        return true;
    }
    let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
    let pret = joueur.u8(ctx.b, 0x33) != 0;
    pret
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
        scintille(ctx, o);
    }
    anime_et_trie(ctx, tresor::ANIMATIONS, variante);
}

fn tresor_actif(ctx: &mut Ctx, o: Obj) {
    let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
    let cible = super::effects::cible(ctx, tresor::TABLE_SOUS_ETATS, sous_etat as u32);
    match sous_etat {
        sous::CHUTE => {
            let en_vol = ctx.move_xyz().gt();
            if !en_vol {
                // posé : attente au sol
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
                o.set_u16(ctx.b, champ::DUREE, tresor::DUREE_AU_SOL);
                o.set_u32(ctx.b, Obj::Z, 0);
                dans_la_vue(ctx, o);
                scintille(ctx, o);
            }
            fin_de_chute(ctx);
        }
        sous::ATTENTE => {
            // en cinématique, un objet resté à gauche de l'écran disparaît
            let cinematique = ctx.ram8(0x44) != 0;
            if cinematique {
                let limite = crate::ram::CAMERA_X.get(ctx.b).wrapping_sub(0x40);
                let x = o.u16(ctx.b, Obj::X);
                if !((limite as i16) < x as i16) {
                    o.set_u8(ctx.b, Obj::STATE, etat::HORS_ECRAN);
                    return;
                }
            }
            let ramasse = o.u8(ctx.b, champ::RAMASSE);
            if !(ramasse == 0) {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
            }
            expire_et_clignote(ctx, o);
        }
        sous::RAMASSE => {
            if !ramassage_pret(ctx, o) {
                return;
            }
            ajoute_u8(ctx, o, Obj::STATE, 2);
            let son = o.u8(ctx.b, tresor::SON);
            effet_ramassage(ctx, o, son);
            let joueur_bits = o.u16(ctx.b, 0x48) & 0x300;
            let evenement = (o.u8(ctx.b, tresor::SCORE) as u16).wrapping_add(joueur_bits);
            crate::evenements::enfiler_points(ctx.b, evenement);
            marque_ramasse(ctx, o);
        }
        _ => crate::families::effects::hors_table(cible),
    }
}

fn tresor(ctx: &mut Ctx) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = super::effects::cible(ctx, tresor::TABLE_ETATS, etat as u32);
    match etat {
        etat::INIT => tresor_init(ctx, o),
        etat::ACTIF => tresor_actif(ctx, o),
        etat::LIBERE | etat::HORS_ECRAN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::hors_table(cible),
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
        scintille(ctx, o);
    }
    anime_et_trie(ctx, conso::ANIMATIONS, variante);
}

/// Effet d'une nourriture : soigne le joueur ; à pleine santé, événement de score et son.
fn effet_nourriture(ctx: &mut Ctx, o: Obj) {
    let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
    let (vie_max, vie) = (joueur.u16(ctx.b, 0x60), joueur.u16(ctx.b, 0x62));
    if !(vie_max != vie) {
        let evenement = 0x11 + ((joueur.u8(ctx.b, 2) as u16) << 8);
        crate::evenements::enfiler_points(ctx.b, evenement);
        effet_ramassage(ctx, o, 5);
        return;
    }
    let variante = o.u8(ctx.b, Obj::VARIANT);
    let soin: u16 = if variante == 0 { 0x14 } else { 0x28 };
    let vie = vie.wrapping_add(soin);
    joueur.set_u16(ctx.b, 0x62, vie);
    if !(vie <= vie_max) {
        joueur.set_u16(ctx.b, 0x62, vie_max);
    }
    crate::sound::jouer(ctx.b, appel::FIN_EFFET, ctx.obj());
}

/// Potions : le joueur reçoit un compte à rebours de 0x2d0 images et des drapeaux +0x180..0x184.
fn effet_potion(ctx: &mut Ctx, o: Obj, octets: &[(u32, u8)]) {
    let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
    joueur.set_u16(ctx.b, 0x180, 0x2d0);
    for &(off, v) in octets {
        joueur.set_u8(ctx.b, off, v);
    }
    crate::sound::jouer(ctx.b, appel::FIN_EFFET, ctx.obj());
}

fn conso_effet(ctx: &mut Ctx, o: Obj) {
    let variante = o.u8(ctx.b, Obj::VARIANT) as u32;
    let cible = ctx.prog32(conso::EFFETS + 4 * variante);
    // registres de l'original au moment du `jsr (A1)` (utiles si la variante est hors table)
    match cible {
        conso::NOURRITURE => effet_nourriture(ctx, o),
        conso::POTION_B => effet_potion(ctx, o, &[(1, 1), (0x183, 1), (0x184, 0), (0x182, 0)]),
        conso::POTION_A => effet_potion(ctx, o, &[(0x184, 1), (0x183, 0), (0x182, 0)]),
        conso::SANS_EFFET => {}
        conso::COMPTEUR => {
            let n = ctx.ram8(0x45).wrapping_add(1);
            ctx.set_ram8(0x45, n);
            if !(n <= 9) {
                ctx.set_ram8(0x45, 9);
            }
        }
        _ => crate::families::effects::hors_table(cible),
    }
    marque_ramasse(ctx, o);
}

fn conso_actif(ctx: &mut Ctx, o: Obj) {
    let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
    let cible = super::effects::cible(ctx, conso::TABLE_SOUS_ETATS, sous_etat as u32);
    match sous_etat {
        sous::CHUTE => {
            let en_vol = ctx.move_xyz().gt();
            if !en_vol {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
                o.set_u16(ctx.b, champ::DUREE, conso::DUREE_APRES_CHUTE);
                o.set_u32(ctx.b, Obj::Z, 0);
                dans_la_vue(ctx, o);
                scintille(ctx, o);
            }
            fin_de_chute(ctx);
        }
        sous::ATTENTE => {
            let ramasse = o.u8(ctx.b, champ::RAMASSE);
            if !(ramasse == 0) {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
            }
            // seules les variantes 2 et suivantes disparaissent
            let variante = o.u8(ctx.b, Obj::VARIANT);
            if (variante == 0) || (variante == 1) {
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
        _ => crate::families::effects::hors_table(cible),
    }
}

fn conso_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = super::effects::cible(ctx, conso::TABLE_ETATS, etat as u32);
    match etat {
        etat::INIT => conso_init(ctx, o),
        etat::ACTIF => conso_actif(ctx, o),
        etat::LIBERE | etat::HORS_ECRAN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::hors_table(cible),
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
/// `index` : D0.w de l'appelant, l'identifiant dans l'octet bas (l'octet haut n'est nul qu'avec un vrai joueur, voir
/// `equipement_ajoute`).
fn objet_expulse(ctx: &mut Ctx, o: Obj, joueur: Obj, index: u16, quantite: u8) {
    let Some(n) = ctx.alloc(&pools::ITEM) else {
        return;
    };
    n.set_u8(ctx.b, Obj::ACTIVE, 1);
    n.set_u8(ctx.b, champ::MODE, 1);
    n.set_u8(ctx.b, inventaire::QUANTITE, quantite);
    // `add.w D0,D0` puis `(d8,PC,D0.w)` : mot étendu en signe
    let rang = index.wrapping_mul(2) as i16 as i32 as u32;
    let kind = ctx.prog8(inventaire::RECREES.wrapping_add(rang));
    n.set_u8(ctx.b, Obj::KIND, kind);
    let variante = ctx.prog8(inventaire::RECREES.wrapping_add(rang).wrapping_add(1));
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
    crate::evenements::enfiler_points(ctx.b, ((personnage << 8) + 0x11) as u16);
    effet_ramassage(ctx, ctx.obj(), 5);
}

/// `0xdc0aa` : range l'objet dans l'inventaire du propriétaire. Renvoie vrai si le ramassage est refusé.
fn inventaire_ajoute(ctx: &mut Ctx, o: Obj) -> bool {
    let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
    let ligne = (o.u8(ctx.b, Obj::VARIANT) as u32) << 3 & 0xffff;
    let id = ctx.rom8(inventaire::FICHES + ligne);
    let classe = joueur.u8(ctx.b, 3) >> 1;
    let interdit = ctx.rom8(inventaire::FICHES + 1 + (classe.wrapping_add(ligne as u8)) as u32);
    if interdit != 0 {
        return ramassage_refuse(ctx, o);
    }
    if !(id != 0x23) && (classe == 2) {
        evenement_inventaire(ctx, joueur);
        return false;
    }
    let quantite = o.u8(ctx.b, inventaire::QUANTITE);
    // 1. l'objet est déjà porté : la quantité augmente
    for i in 0..6u32 {
        let porte = joueur.u8(ctx.b, inventaire::IDENTIFIANTS + i);
        if porte != id {
            continue;
        }
        let n = joueur.u8(ctx.b, inventaire::QUANTITES + i).wrapping_add(quantite);
        joueur.set_u8(ctx.b, inventaire::QUANTITES + i, n);
        let variante = o.u8(ctx.b, Obj::VARIANT);
        // variantes à exemplaire unique : 7, 0x10, 0x11
        let unique = (variante == 7) || (variante == 0x10) || (variante == 0x11);
        if unique {
            joueur.set_u8(ctx.b, inventaire::QUANTITES + i, 1);
        } else if !(n <= 9) {
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
        if porte != inventaire::VIDE {
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
        if !(premier != verrou) {
            return ramassage_refuse(ctx, o);
        }
    }
    let ancienne_quantite = joueur.u8(ctx.b, inventaire::QUANTITES);
    joueur.set_u8(ctx.b, inventaire::IDENTIFIANTS, id);
    joueur.set_u8(ctx.b, inventaire::QUANTITES, quantite);
    objet_expulse(ctx, o, joueur, premier as u16, ancienne_quantite);
    false
}

fn inventaire_init(ctx: &mut Ctx, o: Obj) {
    let variante = o.u8(ctx.b, Obj::VARIANT);
    let mode = o.u8(ctx.b, champ::MODE);
    if mode != 0 {
        // objet jeté : attend la fin du délai
        let delai = o.u8(ctx.b, inventaire::DELAI);
        if !(delai == 0) {
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
        scintille(ctx, o);
    }
    anime_et_trie(ctx, inventaire::ANIMATIONS, variante);
}

/// Sous-état 6 : l'objet refusé est rejeté en arc vers l'extérieur de l'écran puis retombe.
fn inventaire_rejet(ctx: &mut Ctx, o: Obj) {
    let lance = o.u8(ctx.b, champ::SOUS_SOUS_ETAT) != 0;
    if !lance {
        // attente du délai, puis lancer
        let delai = o.u8(ctx.b, inventaire::DELAI).wrapping_sub(1);
        o.set_u8(ctx.b, inventaire::DELAI, delai);
        if delai != 0 {
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
        if !(limite as i16 > x as i16) {
            let v = o.u32(ctx.b, Obj::VX).wrapping_neg();
            o.set_u32(ctx.b, Obj::VX, v);
        }
        ctx.reinsert();
        return;
    }
    ctx.move_xz();
    let z = o.u16(ctx.b, Obj::Z);
    if z as i16 >= 0 {
        return;
    }
    o.set_u8(ctx.b, champ::SOUS_ETAT, sous::ATTENTE);
    o.set_u8(ctx.b, champ::SOUS_SOUS_ETAT, 0);
    o.set_u32(ctx.b, Obj::Z, 0);
    o.set_u8(ctx.b, champ::RAMASSE, 0);
}

fn inventaire_actif(ctx: &mut Ctx, o: Obj) {
    let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
    let cible = super::effects::cible(ctx, inventaire::TABLE_SOUS_ETATS, sous_etat as u32);
    match sous_etat {
        sous::CHUTE => {
            let en_vol = ctx.move_xyz().gt();
            if !en_vol {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
                o.set_u16(ctx.b, champ::DUREE, inventaire::DUREE_AU_SOL);
                o.set_u32(ctx.b, Obj::Z, 0);
                let mode = o.u8(ctx.b, champ::MODE);
                if !(mode != 0) {
                    dans_la_vue(ctx, o);
                    scintille(ctx, o);
                }
            }
            ctx.anim_step();
            fin_de_chute(ctx);
        }
        sous::ATTENTE => {
            let ramasse = o.u8(ctx.b, champ::RAMASSE);
            if !(ramasse == 0) {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
            }
            ctx.anim_step();
            expire_et_clignote(ctx, o);
        }
        sous::RAMASSE => {
            let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
            let pret = joueur.u8(ctx.b, 0x33) != 0;
            if !pret {
                return;
            }
            let refuse = inventaire_ajoute(ctx, o);
            if !refuse {
                ajoute_u8(ctx, o, Obj::STATE, 2);
            }
            marque_ramasse(ctx, o);
        }
        6 => inventaire_rejet(ctx, o),
        _ => crate::families::effects::hors_table(cible),
    }
}

fn inventaire_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = super::effects::cible(ctx, inventaire::TABLE_ETATS, etat as u32);
    match etat {
        etat::INIT => inventaire_init(ctx, o),
        etat::ACTIF => inventaire_actif(ctx, o),
        etat::LIBERE | etat::HORS_ECRAN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::hors_table(cible),
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
}

fn bonus_init(ctx: &mut Ctx, o: Obj) {
    let variante = o.u8(ctx.b, Obj::VARIANT);
    if en_l_air(ctx, o) {
        etat_suivant(ctx, o, sous::CHUTE);
        o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
        let mode = o.u8(ctx.b, champ::MODE);
        if !(mode == 0) {
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
    let cible = super::effects::cible(ctx, bonus::TABLE_SOUS_ETATS, sous_etat as u32);
    match sous_etat {
        sous::CHUTE => {
            ctx.anim_step();
            let en_vol = ctx.move_xyz().gt();
            if !en_vol {
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
            if !(ramasse == 0) {
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
            let evenement = (o.u8(ctx.b, Obj::VARIANT) as u16).wrapping_add(joueur_bits);
            crate::evenements::enfiler_compteur(ctx.b, evenement);
        }
        _ => crate::families::effects::hors_table(cible),
    }
}

fn bonus_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = super::effects::cible(ctx, bonus::TABLE_ETATS, etat as u32);
    match etat {
        etat::INIT => bonus_init(ctx, o),
        etat::ACTIF => bonus_actif(ctx, o),
        etat::LIBERE | etat::HORS_ECRAN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::hors_table(cible),
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
    pub const ICONE_ENVOL: u32 = 0xdc7a0;
    pub const PROPRIETAIRE_2: u32 = 0x46;
    pub const DECALAGES_X: u32 = 0xdcdd8;
    pub const DELAI_SON: u8 = 0x3c;
    // guide
    pub const GUIDE_ETATS: u32 = 0xdcb30;
    pub const GUIDE_SOUS_ETATS: u32 = 0xdcb92;
    pub const GUIDE_ANIMATIONS: u32 = 0xdcb6e;
    pub const GUIDE_ANIMATIONS_ARRIVEE: u32 = 0xdcbbc;
    pub const TABLE_VITESSES: u32 = 0x1c65b6;
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
    let decalage = if !retourne { 0 } else { 2 };
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
    let alea = ctx.random();
    let tirage = alea & 3;
    o.set_u8(ctx.b, champ::MODE, tirage as u8);
    let animation = ctx.prog8(scene::ICONE_ANIMATIONS + tirage as u32);
    ctx.anim_start(scene::ANIMATIONS, animation as u16);
}

/// `0xdc6e8` : toutes les 0x3c images (hors pause), joue le son de l'icône tiré dans la table
/// (`base` : 0 ou 0x10 selon l'appelant, première ou seconde série de sons).
fn icone_son(ctx: &mut Ctx, o: Obj, base: u32) {
    let en_pause = ctx.ram8(scene::PAUSE) != 0;
    if en_pause {
        return;
    }
    let reste = o.u8(ctx.b, scene::DELAI).wrapping_sub(1);
    o.set_u8(ctx.b, scene::DELAI, reste);
    if reste != 0 {
        return;
    }
    o.set_u8(ctx.b, scene::DELAI, scene::DELAI_SON);
    let tirage = o.u8(ctx.b, champ::MODE) as u32;
    let son = ctx.prog32(scene::ICONE_SONS + base + tirage * 4);
    crate::sound::jouer(ctx.b, son, ctx.obj());
}

/// Sous-état 0 de l'icône : suit le joueur, anime, trie, et joue le son tiré toutes les 0x3c images.
fn icone_suivi(ctx: &mut Ctx, o: Obj) {
    suit_proprietaire(ctx, o);
    ctx.anim_step();
    o.set_u8(ctx.b, 0x3b, 0);
    ctx.reinsert();
    icone_son(ctx, o, 0);
}

/// Vitesses de l'envol (`0xdc7c2`) : retournées avec l'orientation, puis pas d'animation.
fn icone_envol_vitesse(ctx: &mut Ctx, o: Obj) {
    let flip = o.u8(ctx.b, champ::FLIP) != 0;
    if flip {
        let v = o.u16(ctx.b, Obj::VX).wrapping_neg();
        o.set_u16(ctx.b, Obj::VX, v);
    }
    ctx.anim_step();
}

/// Sous-état 2 de l'icône : l'icône s'envole en arc (quatre temps, selon `+6`).
fn icone_envol(ctx: &mut Ctx, o: Obj) {
    let temps = o.u8(ctx.b, champ::SOUS_SOUS_ETAT);
    let cible = super::effects::cible(ctx, scene::ICONE_ENVOL, temps as u32);
    match temps {
        0 | 4 => {
            ajoute_u8(ctx, o, champ::SOUS_SOUS_ETAT, 2);
            o.set_u16(ctx.b, Obj::VX, 0x200);
            o.set_u16(ctx.b, Obj::VZ, if temps == 0 { 0x400 } else { 0x200 });
            o.set_u16(ctx.b, Obj::AZ, 0xff80);
            o.set_u16(ctx.b, Obj::AX, 0);
            icone_envol_vitesse(ctx, o);
        }
        2 => {
            ctx.move_xz();
            let z = o.u16(ctx.b, Obj::Z);
            if !(z as i16 > 0) {
                ajoute_u8(ctx, o, champ::SOUS_SOUS_ETAT, 2);
                o.set_u32(ctx.b, Obj::Z, 0);
            }
            ctx.anim_step();
        }
        6 => {
            ctx.move_xz();
            let z = o.u16(ctx.b, Obj::Z);
            if !(z as i16 > 0) {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
                o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
                o.set_u8(ctx.b, champ::SOUS_SOUS_ETAT, 0);
                o.set_u32(ctx.b, Obj::Z, 0);
                o.set_u8(ctx.b, Obj::ACTIVE, 1);
                o.set_u16(ctx.b, scene::MINUTEUR, 0x12c);
            }
            ctx.anim_step();
        }
        _ => crate::families::effects::hors_table(cible),
    }
}

/// Sous-état 4 de l'icône : attend `+0x58`, rejoue son son, puis disparaît quand le délai `+0x56` est
/// écoulé (ou revient au suivi si le propriétaire reprend).
fn icone_attente(ctx: &mut Ctx, o: Obj) {
    let ramasse = o.u8(ctx.b, champ::RAMASSE);
    if ramasse == 1 {
        let pause = ctx.ram8(scene::PAUSE) != 0;
        if pause {
            o.set_u8(ctx.b, champ::RAMASSE, 0);
        } else {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, champ::SOUS_ETAT, 0);
            let proprietaire = pointeur(ctx, o, scene::PROPRIETAIRE_2);
            let variante = o.u8(ctx.b, Obj::VARIANT);
            if !(variante != 2) {
                let reserve = proprietaire.u8(ctx.b, 0x31);
                if !(reserve == 0) {
                    o.set_u8(ctx.b, Obj::VARIANT, 0);
                }
            }
            ajoute_u8(ctx, proprietaire, 0x31, 1);
            return;
        }
    }
    icone_son(ctx, o, 0x10);
    let variante = o.u8(ctx.b, Obj::VARIANT);
    if !(variante != 2) {
        let proprietaire = pointeur(ctx, o, scene::PROPRIETAIRE_2);
        let reserve = proprietaire.u8(ctx.b, 0x31);
        if !(reserve == 0) {
            o.set_u16(ctx.b, scene::MINUTEUR, 1);
        }
    }
    let reste = o.u16(ctx.b, scene::MINUTEUR).wrapping_sub(1);
    o.set_u16(ctx.b, scene::MINUTEUR, reste);
    if !(reste != 0) {
        o.set_u8(ctx.b, Obj::VARIANT, 0);
        o.set_u32(ctx.b, Obj::STATE, 0x0400_0000);
    }
    ctx.anim_step();
    ctx.reinsert();
}

/// État 4 de l'icône (`0xdc9b8`) : l'icône quitte la scène. Elle part (étincelle, son, routine propre à son
/// mode), marche en X jusqu'à sortir de l'écran, ou s'arrête près du joueur, crée un SET et attend comme le
/// guide (`0xdcc5a`, `0xdcc82`).
mod depart {
    pub const SOUS_ETATS: u32 = 0xdc9c4;
    /// Par mode : routine appelée au départ (longs, 0 = aucune), animation de départ, son et animation
    /// d'arrêt.
    pub const ROUTINES: u32 = 0xdca50;
    pub const ANIMATIONS: u32 = 0xdca60;
    pub const SONS_ARRET: u32 = 0xdcaa0;
    pub const ANIMATIONS_ARRET: u32 = 0xdca9c;
    pub const SON_ARRET: u32 = 0xe1d60;
    /// Compteur d'icônes de l'objet `+0x46`.
    pub const ICONES: u32 = 0x30;
    /// Arrêt demandé (`+0x33`).
    pub const ARRET: u32 = 0x33;
}

fn depart_lance(ctx: &mut Ctx, o: Obj) {
    eclat(ctx, o);
    ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
    let variante = o.u8(ctx.b, Obj::VARIANT);
    ajoute_u8(ctx, o, champ::SOUS_ETAT, variante);
    o.set_u16(ctx.b, Obj::VX, 0x380);
    if o.u8(ctx.b, champ::RAMASSE) != 0 {
        o.set_u16(ctx.b, Obj::VX, 0x200);
        let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
        // le son se joue sur l'icône si le joueur n'est pas devant elle
        if joueur.u16(ctx.b, Obj::Y) as i16 >= o.u16(ctx.b, Obj::Y) as i16 {
            o.set_u16(ctx.b, champ::PROPRIETAIRE, o.0 as u16);
        }
        effet_ramassage(ctx, o, 0xa);
        o.set_u16(ctx.b, champ::PROPRIETAIRE, joueur.0 as u16);
        let mode = o.u8(ctx.b, champ::MODE) as u32;
        let routine = ctx.prog32(depart::ROUTINES + 4 * mode);
        if routine != 0 {
            ctx.jsr(routine);
        }
    }
    if o.u8(ctx.b, 0x2c) == 0 {
        let v = o.u16(ctx.b, Obj::VX).wrapping_neg();
        o.set_u16(ctx.b, Obj::VX, v);
    }
    let mode = o.u8(ctx.b, champ::MODE) as u32;
    let animation = ctx.prog8(depart::ANIMATIONS + mode);
    ctx.anim_start(scene::ANIMATIONS, animation as u16);
}

/// Un objet de scène de moins pour l'objet `+0x46`.
fn depart_decompte(ctx: &mut Ctx, o: Obj) {
    let porteur = pointeur(ctx, o, scene::PROPRIETAIRE_2);
    let n = porteur.u8(ctx.b, depart::ICONES).wrapping_sub(1);
    porteur.set_u8(ctx.b, depart::ICONES, n);
}

/// Sous-état 2 : marche en X ; sortie de l'écran, l'icône est libérée.
fn depart_marche(ctx: &mut Ctx, o: Obj) {
    if o.u8(ctx.b, depart::ARRET) == 0 {
        ctx.move_x();
        let ecart = o.u16(ctx.b, Obj::X).wrapping_sub(crate::ram::CAMERA_X.get(ctx.b)).wrapping_add(0x20);
        if ecart > 0x1c0 {
            o.set_u32(ctx.b, Obj::STATE, 0x0600_0000);
            depart_decompte(ctx, o);
        }
    }
    ctx.anim_step();
}

/// Sous-état 4 : arrêt demandé : son et animation d'arrêt, SET créé, l'icône se tourne vers le joueur.
fn depart_arret(ctx: &mut Ctx, o: Obj) {
    if o.u8(ctx.b, depart::ARRET) != 0 {
        ctx.anim_step();
        return;
    }
    let mode = o.u8(ctx.b, champ::MODE) as u32;
    ctx.c.d[0] = mode;
    let son = ctx.prog8(depart::SONS_ARRET + mode);
    if son != 0 {
        crate::sound::jouer(ctx.b, depart::SON_ARRET, ctx.obj());
    }
    let animation = ctx.prog8(depart::ANIMATIONS_ARRET + mode);
    ctx.anim_start(scene::ANIMATIONS, animation as u16);
    ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
    depart_decompte(ctx, o);
    o.set_u16(ctx.b, scene::CREE, 0);
    if let Some(n) = cree_set(ctx, 1) {
        o.set_u16(ctx.b, scene::CREE, n.0 as u16);
    }
    let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
    if o.u16(ctx.b, Obj::X) <= joueur.u16(ctx.b, Obj::X) {
        o.set_u8(ctx.b, 0x2c, 0x20);
    }
    if o.u8(ctx.b, 0x2c) == joueur.u8(ctx.b, 0x2c) {
        let v = joueur.u8(ctx.b, 0x2c) ^ 0x20;
        joueur.set_u8(ctx.b, 0x2c, v);
    }
    ctx.anim_step();
}

fn icone_depart(ctx: &mut Ctx, o: Obj) {
    let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
    let cible = super::effects::cible(ctx, depart::SOUS_ETATS, sous_etat as u32);
    match sous_etat {
        0 => depart_lance(ctx, o),
        2 => depart_marche(ctx, o),
        4 => depart_arret(ctx, o),
        6 => guide_attente(ctx, o),
        8 => guide_second_objet(ctx, o),
        0xa => {
            let set = pointeur(ctx, o, scene::CREE);
            if set.u8(ctx.b, 0x22) != 0 {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
            }
        }
        0xc => {
            let n = ctx.ram8(scene::COMPTEUR).wrapping_add(4);
            ctx.set_ram8(scene::COMPTEUR, n);
        }
        _ => crate::families::effects::hors_table(cible),
    }
}

fn icone(ctx: &mut Ctx, o: Obj) {
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = super::effects::cible(ctx, scene::ICONE_ETATS, etat as u32);
    match etat {
        etat::INIT => icone_init(ctx, o),
        etat::ACTIF => {
            let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
            let cible = super::effects::cible(ctx, scene::ICONE_SOUS_ETATS, sous_etat as u32);
            match sous_etat {
                sous::CHUTE => icone_suivi(ctx, o),
                2 => icone_envol(ctx, o),
                4 => icone_attente(ctx, o),
                _ => crate::families::effects::hors_table(cible),
            }
        }
        4 => icone_depart(ctx, o),
        6 => ctx.free(&pools::ITEM),
        _ => crate::families::effects::hors_table(cible),
    }
}

/// `0xdcddc` : place le guide au bord droit de l'écran et choisit le joueur vivant le plus proche,
/// dont il retient le propriétaire (`+0x50`), la hauteur (`+0x54`) et un point d'arrivée (`+0x52`).
fn guide_cible(ctx: &mut Ctx, o: Obj) {
    let cam_x = crate::ram::CAMERA_X.get(ctx.b);
    let cam_y = crate::ram::CAMERA_Y.get(ctx.b);
    o.set_u16(ctx.b, Obj::X, cam_x.wrapping_add(0x1a0));
    o.set_u16(ctx.b, Obj::Y, cam_y.wrapping_add(0x40));
    o.set_u32(ctx.b, Obj::Z, 0);
    o.set_u8(ctx.b, 0x2c, 0);
    let mut meilleur: u16 = 0xffff;
    let mut choisi = ctx.c.a[2];
    for i in 0..4u32 {
        let joueur = Obj(scene::JOUEURS + i * scene::PAS_JOUEUR);
        let actif = joueur.u8(ctx.b, Obj::ACTIVE) != 0;
        let vivant = actif && {
            let etat = joueur.u8(ctx.b, Obj::STATE) as i8;
            !(etat >= 4)
        };
        if vivant {
            let mut dx = o.u16(ctx.b, Obj::X).wrapping_sub(joueur.u16(ctx.b, Obj::X));
            if !(dx as i16 >= 0) {
                dx = dx.wrapping_neg();
            }
            let mut dy = o.u16(ctx.b, Obj::Y).wrapping_sub(joueur.u16(ctx.b, Obj::Y));
            if !(dy as i16 >= 0) {
                dy = dy.wrapping_neg();
            }
            let distance = dx.wrapping_add(dy);
            if !(meilleur <= distance) {
                meilleur = distance;
                choisi = joueur.0;
            }
        }
    }
    o.set_u16(ctx.b, champ::PROPRIETAIRE, choisi as u16);
    let cible = Obj(choisi);
    let y = cible.u16(ctx.b, Obj::Y);
    o.set_u16(ctx.b, scene::CIBLE_Y, y);
    let ecart = cible.u16(ctx.b, Obj::X).wrapping_sub(cam_x);
    let decalage: u16 = if ecart <= 0x140 { 0x20 } else { 0x20u16.wrapping_neg() };
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
    if !(variante == 3) {
        let mode = o.u8(ctx.b, champ::MODE) as u32;
        animation = ctx.prog8(scene::GUIDE_ANIMATIONS_ARRIVEE + mode);
    }
    ctx.anim_start(scene::ANIMATIONS, animation as u16);
}

/// Crée un objet SET de type `type_set` ; renvoie son adresse, ou `None` si le pool est plein.
fn cree_set(ctx: &mut Ctx, type_set: u32) -> Option<Obj> {
    set_de_scene(ctx, type_set as u8)
}

/// Sous-état 0 du guide : marche vers le point d'arrivée ; arrivé, change d'animation et crée un SET.
fn guide_marche(ctx: &mut Ctx, o: Obj) {
    let cible_x = o.u16(ctx.b, scene::CIBLE_X);
    let cible_y = o.u16(ctx.b, scene::CIBLE_Y);
    let direction = crate::aides::direction_from(ctx.b, o, (cible_x, cible_y));
    o.set_u8(ctx.b, Obj::SPEED_INDEX, direction);
    ctx.move_by_table_xy();
    let dx = cible_x.wrapping_sub(o.u16(ctx.b, Obj::X)).wrapping_add(8);
    let dy = cible_y.wrapping_sub(o.u16(ctx.b, Obj::Y)).wrapping_add(8);
    let arrive = !(dx > 0x10) && !(dy > 0x10);
    if arrive {
        ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
        guide_animation(ctx, o);
        o.set_u16(ctx.b, scene::MINUTEUR, 0x3c);
        o.set_u16(ctx.b, scene::CREE, 0);
        let variante = o.u8(ctx.b, Obj::VARIANT);
        let type_set = if variante != 3 { 1 } else { 0x26 };
        if let Some(n) = cree_set(ctx, type_set) {
            o.set_u16(ctx.b, scene::CREE, n.0 as u16);
        }
        let proprietaire = pointeur(ctx, o, champ::PROPRIETAIRE);
        let x = o.u16(ctx.b, Obj::X);
        let x_proprietaire = proprietaire.u16(ctx.b, Obj::X);
        if !(x > x_proprietaire) {
            o.set_u8(ctx.b, 0x2c, 0x20);
        }
        let (a, b) = (o.u8(ctx.b, 0x2c), proprietaire.u8(ctx.b, 0x2c));
        if !(a != b) {
            let v = proprietaire.u8(ctx.b, 0x2c) ^ 0x20;
            proprietaire.set_u8(ctx.b, 0x2c, v);
        }
    }
    ctx.anim_step();
}

/// Sous-état 2 du guide : attend la fin de la minuterie ou la disparition de l'objet SET créé.
fn guide_attente(ctx: &mut Ctx, o: Obj) {
    let cree = o.u16(ctx.b, scene::CREE);
    if !(cree == 0) {
        let set = pointeur(ctx, o, scene::CREE);
        let fini = set.u8(ctx.b, 0x22) != 0;
        if !fini {
            return;
        }
    } else {
        let reste = o.u16(ctx.b, scene::MINUTEUR).wrapping_sub(1);
        o.set_u16(ctx.b, scene::MINUTEUR, reste);
        if reste != 0 {
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
    if compteur != 0x10 {
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
    if !en_cours {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
}

fn guide_actif(ctx: &mut Ctx, o: Obj) {
    let fige = ctx.ram8(scene::SCENE_FIGEE) != 0;
    if !fige {
        let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
        let cible = super::effects::cible(ctx, scene::GUIDE_SOUS_ETATS, sous_etat as u32);
        match sous_etat {
            0 => guide_marche(ctx, o),
            2 => guide_attente(ctx, o),
            4 => guide_second_objet(ctx, o),
            6 => guide_fin(ctx, o),
            _ => crate::families::effects::hors_table(cible),
        }
    }
    ctx.reinsert();
}

fn guide(ctx: &mut Ctx, o: Obj) {
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = super::effects::cible(ctx, scene::GUIDE_ETATS, etat as u32);
    match etat {
        etat::INIT => guide_init(ctx, o),
        etat::ACTIF => guide_actif(ctx, o),
        etat::LIBERE => {
            let n = ctx.ram8(scene::COMPTEUR).wrapping_add(4);
            ctx.set_ram8(scene::COMPTEUR, n);
        }
        etat::HORS_ECRAN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::hors_table(cible),
    }
}

/// Variante 3 (`0xdccd6`), le « guide » d'une scène d'attract (h) : charge une palette, puis marche comme
/// le guide de la variante 1 et attend la fin de son objet SET (type 0x26), dont il recopie le résultat
/// (`+0x23`) dans `A5-0x551c`.
mod guide_3 {
    pub const ETATS: u32 = 0xdcce4;
    pub const SOUS_ETATS: u32 = 0xdcd76;
    /// Palette de 16 couleurs (données en clair) et sa destination.
    pub const PALETTE: u32 = 0xdccec;
    pub const PALETTE_DEST: u32 = 0x91c300;
    /// Première couleur de la palette 0 : ses bits de luminosité (0xf000f000) sont recopiés.
    pub const REFERENCE: u32 = 0x91c000;
    pub const ANIMATION: u16 = 0x13;
    pub const RESULTAT: i32 = -0x551c;
}

fn guide_3_init(ctx: &mut Ctx, o: Obj) {
    let lumiere = crate::mem::rd32(ctx.b, guide_3::REFERENCE) & 0xf000_f000;
    for i in 0..8 {
        let couleur = crate::mem::rd32(ctx.b, guide_3::PALETTE + 4 * i);
        // registres du `movem` de l'original (D1-D6, A3) ; D0 reçoit le masque
        match i {
            0 => {}
            7 => ctx.c.a[3] = couleur,
            _ => ctx.c.d[i as usize] = couleur,
        }
        crate::mem::wr32(ctx.b, guide_3::PALETTE_DEST + 4 * i, couleur | lumiere);
    }
    ctx.c.a[2] = guide_3::PALETTE_DEST + 0x20;
    ajoute_u8(ctx, o, Obj::STATE, 2);
    guide_cible(ctx, o);
    o.set_u32(ctx.b, Obj::SPEED_TABLE, scene::TABLE_VITESSES);
    o.set_u8(ctx.b, 0x2a, 1);
    o.set_u8(ctx.b, 0x2b, 0x18);
    ctx.anim_start(scene::ANIMATIONS, guide_3::ANIMATION);
    ctx.reinsert();
}

/// Sous-état 2 : attend la fin de l'objet SET (dont il garde le résultat) ou de la minuterie.
fn guide_3_attente(ctx: &mut Ctx, o: Obj) {
    if o.u16(ctx.b, scene::CREE) != 0 {
        let set = pointeur(ctx, o, scene::CREE);
        if set.u8(ctx.b, 0x22) == 0 {
            return;
        }
        let resultat = set.u8(ctx.b, 0x23);
        ctx.set_ram8(guide_3::RESULTAT, resultat);
    } else {
        let reste = o.u16(ctx.b, scene::MINUTEUR).wrapping_sub(1);
        o.set_u16(ctx.b, scene::MINUTEUR, reste);
        if reste != 0 {
            return;
        }
    }
    ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
    let n = ctx.ram8(scene::COMPTEUR).wrapping_add(4);
    ctx.set_ram8(scene::COMPTEUR, n);
}

fn guide_3(ctx: &mut Ctx, o: Obj) {
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = super::effects::cible(ctx, guide_3::ETATS, etat as u32);
    match etat {
        etat::INIT => guide_3_init(ctx, o),
        etat::ACTIF => {
            let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
            let cible = super::effects::cible(ctx, guide_3::SOUS_ETATS, sous_etat as u32);
            match sous_etat {
                0 => guide_marche(ctx, o),
                2 => guide_3_attente(ctx, o),
                4 => {}
                _ => crate::families::effects::hors_table(cible),
            }
        }
        etat::LIBERE => {}
        etat::HORS_ECRAN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::hors_table(cible),
    }
}

fn scene_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let variante = o.u8(ctx.b, Obj::VARIANT);
    let cible = super::effects::cible(ctx, scene::TABLE_VARIANTES, 2 * variante as u32);
    match variante {
        0 | 2 => icone(ctx, o),
        1 => guide(ctx, o),
        3 => guide_3(ctx, o),
        _ => crate::families::effects::hors_table(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 7 : réserve du joueur (0xdce6e)

/// Type 7, objets qui alimentent une réserve du joueur (compteurs `+0x150 + variante`, plafonnés à 9)
/// pour les personnages autorisés (table par personnage, puis par classe). Même cycle que le type 4 :
/// chute, attente (0x4b0 images, clignotement), ramassage (refusé : rejet en arc), mode 1 : objet jeté.
mod reserve {
    pub const ENTREE: u32 = 0xdce6e;
    pub const TABLE_ETATS: u32 = 0xdce84;
    pub const TABLE_SOUS_ETATS: u32 = 0xdcf38;
    pub const ANIMATIONS: u32 = 0x1c001e;
    pub const DUREE_AU_SOL: u16 = 0x4b0;
    /// Par personnage (espace programme) : l'objet est-il utilisable ?
    pub const PERSONNAGES: u32 = 0xdd01e;
    /// Par classe (données) : réserve acceptée ?
    pub const CLASSES: u32 = 0xdd122;
    pub const COMPTEURS: u32 = 0x150;
}

fn reserve_init(ctx: &mut Ctx, o: Obj) {
    let variante = o.u8(ctx.b, Obj::VARIANT);
    let mode = o.u8(ctx.b, champ::MODE);
    if mode != 0 {
        let delai = o.u8(ctx.b, inventaire::DELAI);
        if !(delai == 0) {
            o.set_u8(ctx.b, inventaire::DELAI, delai - 1);
            return;
        }
        ajoute_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
        return anime_et_trie(ctx, reserve::ANIMATIONS, variante.wrapping_add(0xa));
    }
    if en_l_air(ctx, o) {
        etat_suivant(ctx, o, sous::CHUTE);
        o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
    } else {
        etat_suivant(ctx, o, sous::ATTENTE);
        o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
        o.set_u16(ctx.b, champ::DUREE, reserve::DUREE_AU_SOL);
    }
    anime_et_trie(ctx, reserve::ANIMATIONS, variante);
}

/// Sous-état d'attente : le joueur qui touche l'objet (`RAMASSE`) le fait passer au ramassage.
fn reserve_attente(ctx: &mut Ctx, o: Obj) {
    let ramasse = o.u8(ctx.b, champ::RAMASSE);
    if !(ramasse == 0) {
        ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
    }
    expire_et_clignote(ctx, o);
}

/// `0xdd0da` : ajoute l'objet à la réserve du propriétaire. Vrai si le ramassage est refusé.
fn reserve_ajoute(ctx: &mut Ctx, o: Obj) -> bool {
    let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
    let classe = (joueur.u8(ctx.b, 3) >> 1) as u32;
    let accepte = ctx.rom8(reserve::CLASSES + classe);
    if !(accepte != 0) {
        o.set_u8(ctx.b, champ::SOUS_ETAT, 6);
        o.set_u8(ctx.b, champ::SOUS_SOUS_ETAT, 0);
        o.set_u8(ctx.b, inventaire::DELAI, 0xc);
        return true;
    }
    let variante = o.u8(ctx.b, Obj::VARIANT) as u32;
    let compteur = joueur.0 + reserve::COMPTEURS + variante;
    let n = crate::mem::rd8(ctx.b, compteur).wrapping_add(1);
    crate::mem::wr8(ctx.b, compteur, n);
    if !(n <= 9) {
        crate::mem::wr8(ctx.b, compteur, 9);
    }
    false
}

fn reserve_ramasse(ctx: &mut Ctx, o: Obj) {
    let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
    let ramasse = o.u8(ctx.b, champ::RAMASSE);
    if !(ramasse == 2) {
        let pret = joueur.u8(ctx.b, 0x33) != 0;
        if !pret {
            return;
        }
    }
    let personnage = joueur.u8(ctx.b, 3) as u32;
    let autorise = ctx.prog8(reserve::PERSONNAGES + personnage);
    if !(autorise != 0) {
        ajoute_u8(ctx, o, champ::SOUS_ETAT, 0xfe);
        o.set_u8(ctx.b, champ::RAMASSE, 0);
        return;
    }
    let refuse = reserve_ajoute(ctx, o);
    if !refuse {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
}

fn reserve_actif(ctx: &mut Ctx, o: Obj) {
    let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
    let cible = super::effects::cible(ctx, reserve::TABLE_SOUS_ETATS, sous_etat as u32);
    match sous_etat {
        sous::CHUTE => {
            let pose = o.u8(ctx.b, champ::SOUS_SOUS_ETAT);
            if !(pose != 0) {
                let en_vol = ctx.move_xyz().gt();
                if !en_vol {
                    ajoute_u8(ctx, o, champ::SOUS_SOUS_ETAT, 2);
                    o.set_u16(ctx.b, champ::DUREE, reserve::DUREE_AU_SOL);
                    o.set_u32(ctx.b, Obj::Z, 0);
                    let mode = o.u8(ctx.b, champ::MODE);
                    if !(mode != 0) {
                        dans_la_vue(ctx, o);
                    }
                }
                return fin_de_chute(ctx);
            }
            let mode = o.u8(ctx.b, champ::MODE);
            if !(mode == 0) {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
                return reserve_attente(ctx, o);
            }
            let fin = ctx.anim_step();
            if fin {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
            }
        }
        sous::ATTENTE => reserve_attente(ctx, o),
        sous::RAMASSE => reserve_ramasse(ctx, o),
        6 => inventaire_rejet(ctx, o),
        _ => crate::families::effects::hors_table(cible),
    }
}

fn reserve_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = super::effects::cible(ctx, reserve::TABLE_ETATS, etat as u32);
    match etat {
        etat::INIT => reserve_init(ctx, o),
        etat::ACTIF => reserve_actif(ctx, o),
        etat::LIBERE | etat::HORS_ECRAN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::hors_table(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 9 : icône qui accompagne un objet (0xdd5a2)

/// Type 9 : étincelle d'interface qui suit un objet (`+0x5c`), un pixel devant lui, pendant `+0x14`
/// images (le compte est préparé par le créateur), puis disparaît. Palette selon la variante.
mod accompagnement {
    pub const ENTREE: u32 = 0xdd5a2;
    pub const PALETTES: u32 = 0xdd5de;
    pub const ANIMATIONS: u32 = 0x1c03a6;
    pub const CIBLE: u32 = 0x5c;
    pub const DUREE: u32 = 0x14;
}

fn accompagnement_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let etat = o.u8(ctx.b, Obj::STATE);
    if !(etat != 0) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, 0x2a, 1);
        let variante = o.u8(ctx.b, Obj::VARIANT);
        let palette = ctx.prog8(accompagnement::PALETTES + variante as u32);
        o.set_u8(ctx.b, 0x2b, palette);
        o.set_u16(ctx.b, accompagnement::DUREE, 0x100);
        ctx.anim_start(accompagnement::ANIMATIONS, 0x8e + variante as u16);
        return ctx.reinsert();
    }
    let cible = pointeur(ctx, o, accompagnement::CIBLE);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = cible.u16(ctx.b, off);
        o.set_u16(ctx.b, off, v);
    }
    let y = o.u16(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u16(ctx.b, Obj::Y, y);
    ctx.anim_step();
    let reste = o.u16(ctx.b, accompagnement::DUREE).wrapping_sub(1);
    o.set_u16(ctx.b, accompagnement::DUREE, reste);
    if reste as i16 >= 0 {
        return;
    }
    ctx.free(&pools::ITEM);
}

// ---------------------------------------------------------------------------------------------
// Type 8 : objets à contact (0xdd132)

/// Type 8, objets ramassés par simple contact (test de boîtes avec les quatre joueurs, `0xdd470`) qui
/// déclenchent un effet de scène par un objet SET (`0xc8312`) : trois variantes.
///
/// - 0 : bonus posé qui attend `+0x58` (touché par un joueur), crée un SET (`+0x42`) et le suit ;
/// - 1 : bonus qui oscille horizontalement (vitesse `+0x14`, demi-tour toutes les 0x3c images) jusqu'au
///   contact d'un joueur ;
/// - 2 : même principe que la 0, sans oscillation.
///
/// `A5-0x5ac4` est non nul tant qu'un joueur est en train d'en ramasser un : un seul à la fois.
mod contact {
    pub const ENTREE: u32 = 0xdd132;
    pub const TABLE_VARIANTES: u32 = 0xdd142;
    pub const ANIMATIONS: u32 = 0x1bf8fe;
    // variante 0
    pub const V0_ETATS: u32 = 0xdd156;
    pub const V0_SOUS_ETATS: u32 = 0xdd190;
    pub const V0_ANIMATIONS: u32 = 0xdd180;
    pub const V0_ANIMATIONS_TOUCHE: u32 = 0xdd1d2;
    pub const V0_ANIMATIONS_SET: u32 = 0xdd22a;
    pub const V0_ANIMATIONS_FIN: u32 = 0xdd246;
    // variante 1
    pub const V1_ETATS: u32 = 0xdd280;
    pub const V1_SOUS_ETATS: u32 = 0xdd2d4;
    // variante 2
    pub const V2_ETATS: u32 = 0xdd3ca;
    pub const V2_SOUS_ETATS: u32 = 0xdd400;
    /// Verrou : un joueur est en train de ramasser (A5-0x5ac4) et l'événement posé en `A5+0x5e`.
    pub const OCCUPE: i32 = -0x5ac4;
    pub const SET_CREE: u32 = 0x42;
    pub const DELAI: u32 = 0x4c;
    /// Boîtes de contact : une par type (`+0x39`) pour l'objet, une par personnage et par `+0x38` pour le joueur.
    pub const BOITES_OBJET: u32 = 0xdd590;
    pub const BOITES_JOUEUR: u32 = 0xdd578;
    pub const JOUEURS: u32 = 0xff85de;
}

/// `0xdd470` : un joueur touche-t-il l'objet ? Si oui, le sous-état avance, le verrou se lève et le
/// joueur devient le propriétaire.
fn contact_joueur(ctx: &mut Ctx, o: Obj) -> bool {
    let occupe = ctx.ram8(contact::OCCUPE) != 0;
    if occupe {
        return false;
    }
    let boite = o.u8(ctx.b, 0x39) as u32;
    if boite == 0 {
        return false;
    }
    let ma_boite = contact::BOITES_OBJET + (boite << 3);
    for i in 0..4u32 {
        let joueur = Obj(contact::JOUEURS + i * 0x200);
        if joueur_touche(ctx, o, joueur, ma_boite) {
            ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
            ctx.set_ram8(contact::OCCUPE, 1);
            o.set_u16(ctx.b, champ::PROPRIETAIRE, joueur.0 as u16);
            return true;
        }
    }
    false
}

/// Le joueur est-il présent, en vie, libre, et sa boîte recouvre-t-elle celle de l'objet ?
fn joueur_touche(ctx: &mut Ctx, o: Obj, joueur: Obj, ma_boite: u32) -> bool {
    let absent = joueur.u8(ctx.b, Obj::ACTIVE) != 1;
    if absent {
        return false;
    }
    let mort = joueur.u8(ctx.b, Obj::STATE) as i8 >= 4;
    if mort {
        return false;
    }
    let occupe = joueur.u8(ctx.b, 0x182) != 0;
    if occupe {
        return false;
    }
    let numero = joueur.u8(ctx.b, 0x38) as u32;
    if numero == 0 {
        return false;
    }
    let classe = (joueur.u8(ctx.b, 3) >> 1) as u32;
    let table = ctx.rom32(contact::BOITES_JOUEUR + (classe << 2));
    let sa_boite = table + (numero << 3);
    chevauche(ctx, o, joueur, ma_boite, sa_boite)
}

/// Les trois recouvrements de boîtes (X, Y puis Z) entre l'objet `o` (boîte `a`) et le joueur (boîte `b`).
fn chevauche(ctx: &mut Ctx, o: Obj, joueur: Obj, a: u32, b: u32) -> bool {
    let octet = |ctx: &mut Ctx, adresse: u32| ctx.rom8(adresse) as i8 as i16 as u16;
    // X : les décalages s'inversent avec l'orientation de chacun
    let mut da = octet(ctx, a);
    let retourne = o.u8(ctx.b, champ::FLIP) != 0;
    if retourne {
        da = da.wrapping_neg();
    }
    let mut db = octet(ctx, b);
    let joueur_retourne = joueur.u8(ctx.b, champ::FLIP) != 0;
    if joueur_retourne {
        db = db.wrapping_neg();
    }
    let ecart = da.wrapping_add(o.u16(ctx.b, Obj::X)).wrapping_sub(db).wrapping_sub(joueur.u16(ctx.b, Obj::X));
    let demi = ctx.rom8(a + 1).wrapping_add(ctx.rom8(b + 1)) as u16;
    if ecart.wrapping_add(demi) > demi.wrapping_mul(2) {
        return false;
    }
    // Y puis Z : décalages fixes
    for (decalage, taille, position) in [(4u32, 5u32, Obj::Y), (2, 3, Obj::Z)] {
        let da = octet(ctx, a + decalage);
        let db = octet(ctx, b + decalage);
        let ecart = da.wrapping_add(o.u16(ctx.b, position)).wrapping_sub(db.wrapping_add(joueur.u16(ctx.b, position)));
        let demi = ctx.rom8(a + taille).wrapping_add(ctx.rom8(b + taille)) as u16;
        if ecart.wrapping_add(demi) > demi.wrapping_mul(2) {
            return false;
        }
    }
    true
}

/// `0xdd16e` : démarre l'animation `animation`, puis trie l'objet.
fn contact_anime(ctx: &mut Ctx, animation: u8) {
    ctx.anim_start(contact::ANIMATIONS, animation as u16);
    ctx.reinsert();
}

/// Animation tirée d'une table indexée par le mode (`+0x2f`).
fn contact_anime_mode(ctx: &mut Ctx, o: Obj, table: u32) {
    let mode = o.u8(ctx.b, champ::MODE) as u32;
    let animation = ctx.prog8(table + mode);
    contact_anime(ctx, animation);
}

/// Crée l'objet SET de la scène de ramassage (type `+0x42`, mot) et le rattache à l'objet. Faux si le
/// type est négatif (pas de scène) ou si le pool est plein.
fn contact_cree_set(ctx: &mut Ctx, o: Obj) -> bool {
    let mot = o.u16(ctx.b, contact::SET_CREE);
    if mot as i16 <= -1 {
        return false;
    }
    let Some(nouveau) = set_de_scene(ctx, mot as u8) else {
        return false;
    };
    o.set_u16(ctx.b, contact::SET_CREE, nouveau.0 as u16);
    let proprietaire = pointeur(ctx, o, champ::PROPRIETAIRE);
    let personnage = proprietaire.u8(ctx.b, 2);
    nouveau.set_u8(ctx.b, 0x37, personnage);
    true
}

/// Oriente l'objet vers le côté où se trouve le propriétaire (retenue de `x_proprietaire - x`).
fn contact_oriente(ctx: &mut Ctx, o: Obj) {
    let proprietaire = pointeur(ctx, o, champ::PROPRIETAIRE);
    let (x_proprietaire, x) = (proprietaire.u16(ctx.b, Obj::X), o.u16(ctx.b, Obj::X));
    let sens = if x_proprietaire < x { 0 } else { 0x20 };
    o.set_u8(ctx.b, champ::FLIP, sens);
}

/// L'objet SET a-t-il fini ? (`+0x22` de l'objet SET non nul.)
fn contact_set_fini(ctx: &mut Ctx, o: Obj) -> bool {
    let set = pointeur(ctx, o, contact::SET_CREE);
    let fini = set.u8(ctx.b, 0x22) != 0;
    fini
}

/// Variante 0, fin de la scène (`0xdd22e`) : lève le verrou et passe au sous-état de fuite.
fn contact_0_fin(ctx: &mut Ctx, o: Obj) {
    ctx.set_ram8(contact::OCCUPE, 0);
    o.set_u8(ctx.b, champ::SOUS_ETAT, 6);
    contact_anime_mode(ctx, o, contact::V0_ANIMATIONS_FIN);
}

fn decale_x(ctx: &mut Ctx, o: Obj, pas: u16) {
    let x = o.u16(ctx.b, Obj::X).wrapping_add(pas);
    o.set_u16(ctx.b, Obj::X, x);
}

fn contact_0(ctx: &mut Ctx, o: Obj) {
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = super::effects::cible(ctx, contact::V0_ETATS, etat as u32);
    match etat {
        etat::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            let mode = o.u8(ctx.b, champ::MODE) as u32;
            let animation = ctx.prog8(contact::V0_ANIMATIONS + mode);
            o.set_u8(ctx.b, champ::SOUS_ETAT, 0);
            contact_anime(ctx, animation);
        }
        etat::ACTIF => {
            let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
            let cible = super::effects::cible(ctx, contact::V0_SOUS_ETATS, sous_etat as u32);
            match sous_etat {
                0 => {
                    ctx.anim_step();
                    let ramasse = o.u8(ctx.b, champ::RAMASSE);
                    if ramasse == 0 {
                        return;
                    }
                    let occupe = ctx.ram8(contact::OCCUPE) != 0;
                    if occupe {
                        o.set_u8(ctx.b, champ::RAMASSE, 0);
                        return;
                    }
                    ctx.set_ram8(contact::OCCUPE, 1);
                    ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
                    eclat(ctx, o);
                    contact_anime_mode(ctx, o, contact::V0_ANIMATIONS_TOUCHE);
                }
                2 => {
                    ctx.anim_step();
                    let touche = o.u8(ctx.b, 0x33);
                    if touche == 1 {
                        return;
                    }
                    ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
                    if !contact_cree_set(ctx, o) {
                        return contact_0_fin(ctx, o);
                    }
                    contact_oriente(ctx, o);
                    contact_anime_mode(ctx, o, contact::V0_ANIMATIONS_SET);
                }
                4 => {
                    if contact_set_fini(ctx, o) {
                        contact_0_fin(ctx, o);
                    }
                }
                6 => {
                    let flip = o.u8(ctx.b, champ::FLIP) != 0;
                    let pas: u16 = if flip { 3 } else { 3u16.wrapping_neg() };
                    decale_x(ctx, o, pas);
                    ctx.anim_step();
                    ctx.cull_offscreen();
                }
                _ => crate::families::effects::hors_table(cible),
            }
        }
        etat::LIBERE | etat::HORS_ECRAN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::hors_table(cible),
    }
}

/// Variante 1 : animation de balancement (`0xdd2a8`), tri et mise au point des bords.
fn contact_1_anime(ctx: &mut Ctx) {
    contact_anime(ctx, 0x18);
    crate::ai::keep_in_zone_d5(ctx.c, ctx.b, ctx.obj());
    crate::terrain::push_68k(ctx.c, ctx.b, ctx.obj());
}

/// Variante 1 : déplacement horizontal `X += VX << 8`.
fn contact_1_avance(ctx: &mut Ctx, o: Obj) {
    let vx = o.u16(ctx.b, Obj::VX) as i16 as i32;
    let x = o.u32(ctx.b, Obj::X).wrapping_add((vx << 8) as u32);
    o.set_u32(ctx.b, Obj::X, x);
}

/// Variante 1 : vitesse 0x200 dans le sens de l'orientation (`0xdd296`).
fn contact_1_vitesse(ctx: &mut Ctx, o: Obj) {
    o.set_u16(ctx.b, Obj::VX, 0x200);
    let flip = o.u8(ctx.b, champ::FLIP) != 0;
    if !flip {
        let v = o.u16(ctx.b, Obj::VX).wrapping_neg();
        o.set_u16(ctx.b, Obj::VX, v);
    }
}

/// Variante 1, sortie de scène (`0xdd37c`) : le verrou se lève, le bonus s'éloigne.
fn contact_1_sortie(ctx: &mut Ctx, o: Obj) {
    contact_1_vitesse(ctx, o);
    ctx.set_ram8(contact::OCCUPE, 0);
    o.set_u8(ctx.b, champ::SOUS_ETAT, 6);
    contact_1_anime(ctx);
}

fn contact_1(ctx: &mut Ctx, o: Obj) {
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = super::effects::cible(ctx, contact::V1_ETATS, etat as u32);
    match etat {
        etat::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, champ::SOUS_ETAT, 0);
            o.set_u8(ctx.b, contact::DELAI, 0x3c);
            contact_1_vitesse(ctx, o);
            contact_1_anime(ctx);
        }
        etat::ACTIF => {
            let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
            let cible = super::effects::cible(ctx, contact::V1_SOUS_ETATS, sous_etat as u32);
            match sous_etat {
                0 => {
                    let touche = contact_joueur(ctx, o);
                    if touche {
                        // pas de contact suivant : les sous-états avancent d'eux-mêmes
                        ctx.set_ram8(0x5e, 1);
                        ctx.anim_start(contact::ANIMATIONS, 0x19);
                        return;
                    }
                    contact_1_avance(ctx, o);
                    let reste = o.u8(ctx.b, contact::DELAI).wrapping_sub(1);
                    o.set_u8(ctx.b, contact::DELAI, reste);
                    if !(reste != 0) {
                        o.set_u8(ctx.b, contact::DELAI, 0x3c);
                        let flip = o.u8(ctx.b, champ::FLIP) ^ 0x20;
                        o.set_u8(ctx.b, champ::FLIP, flip);
                        let v = o.u16(ctx.b, Obj::VX).wrapping_neg();
                        o.set_u16(ctx.b, Obj::VX, v);
                    }
                    ctx.anim_step();
                    ctx.reinsert();
                    crate::ai::keep_in_zone_d5(ctx.c, ctx.b, ctx.obj());
                    crate::terrain::push_68k(ctx.c, ctx.b, ctx.obj());
                }
                2 => {
                    ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
                    if !contact_cree_set(ctx, o) {
                        return contact_1_sortie(ctx, o);
                    }
                    contact_oriente(ctx, o);
                }
                4 => {
                    let set = pointeur(ctx, o, contact::SET_CREE);
                    let en_cours = set.u8(ctx.b, 0x22) == 0;
                    if !en_cours {
                        contact_1_sortie(ctx, o);
                    }
                }
                6 => {
                    contact_1_avance(ctx, o);
                    ctx.anim_step();
                    ctx.reinsert();
                    ctx.cull_offscreen();
                }
                _ => crate::families::effects::hors_table(cible),
            }
        }
        etat::LIBERE | etat::HORS_ECRAN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::hors_table(cible),
    }
}

/// Variante 2, fin de la scène (`0xdd44e`).
fn contact_2_fin(ctx: &mut Ctx, o: Obj) {
    ctx.set_ram8(contact::OCCUPE, 0);
    ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
}

fn contact_2(ctx: &mut Ctx, o: Obj) {
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = super::effects::cible(ctx, contact::V2_ETATS, etat as u32);
    match etat {
        etat::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, champ::SOUS_ETAT, 0);
            let mode = o.u8(ctx.b, champ::MODE);
            contact_anime(ctx, mode.wrapping_add(0x14));
        }
        etat::ACTIF => {
            let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
            let cible = super::effects::cible(ctx, contact::V2_SOUS_ETATS, sous_etat as u32);
            match sous_etat {
                0 => {
                    ctx.anim_step();
                    let ramasse = o.u8(ctx.b, champ::RAMASSE);
                    if ramasse == 0 {
                        return;
                    }
                    let occupe = ctx.ram8(contact::OCCUPE) != 0;
                    if occupe {
                        o.set_u8(ctx.b, champ::RAMASSE, 0);
                        return;
                    }
                    ctx.set_ram8(contact::OCCUPE, 1);
                    ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
                    if !contact_cree_set(ctx, o) {
                        contact_2_fin(ctx, o);
                    }
                }
                2 => {
                    if contact_set_fini(ctx, o) {
                        contact_2_fin(ctx, o);
                    }
                }
                4 => {
                    ctx.anim_step();
                }
                _ => crate::families::effects::hors_table(cible),
            }
        }
        etat::LIBERE | etat::HORS_ECRAN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::hors_table(cible),
    }
}

fn contact_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let variante = o.u8(ctx.b, Obj::VARIANT);
    let cible = super::effects::cible(ctx, contact::TABLE_VARIANTES, 2 * variante as u32);
    match variante {
        0 => contact_0(ctx, o),
        1 => contact_1(ctx, o),
        2 => contact_2(ctx, o),
        _ => crate::families::effects::hors_table(cible),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 3 : équipement (0xdb7de)

/// Type 3, équipement (armes, protections et accessoires, identifiants `0x2e` et plus ; en dessous,
/// objets d'inventaire). Une fiche de huit octets par variante (`0xdbc8c`) donne six drapeaux « utilisable
/// par la classe » puis l'identifiant d'équipement et son groupe. Ramassage : l'équipement s'ajoute aux
/// six emplacements (`+0x140`) du joueur, remplace un équipement du même groupe s'il est meilleur, ou
/// prend un emplacement libre ; refusé si la classe ne peut pas le porter ou si le premier emplacement
/// est verrouillé (rejet en arc). Les variantes 0x24 à 0x27 donnent aussi un bonus temporaire au joueur
/// (`+0x18b..0x191`).
mod equipement {
    pub const ENTREE: u32 = 0xdb7de;
    pub const ETATS: u32 = 0xdb7f4;
    pub const SOUS_ETATS: u32 = 0xdb88c;
    pub const ANIMATIONS: u32 = 0x1bf156;
    /// Indice de ramassage par variante.
    pub const INDICES: u32 = 0xdbdcc;
    /// Fiches (données) ; octets 0 à 5 : classes autorisées, 6 : identifiant, 7 : groupe.
    pub const FICHES: u32 = 0xdbc8c;
    /// Par classe, 8 identifiants que le premier emplacement ne peut pas remplacer.
    pub const VERROUILLES: u32 = 0xdbc5c;
    pub const PREMIER_EQUIPE: u32 = 0x10d;
    pub const SECOND_EQUIPE: u32 = 0x10e;
}

/// `0xdba96` : bonus temporaire donné par les variantes 0x24 à 0x27.
fn equipement_bonus(ctx: &mut Ctx, o: Obj) {
    let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
    let variante = o.u8(ctx.b, Obj::VARIANT);
    if !(variante != 0x24) {
        return joueur.set_u8(ctx.b, 0x18c, 0x14);
    }
    if !(variante != 0x25) {
        return joueur.set_u8(ctx.b, 0x18b, 0x14);
    }
    if !(variante != 0x26) {
        joueur.set_u8(ctx.b, 0x18d, 0x14);
    }
    if !(variante != 0x27) {
        joueur.set_u8(ctx.b, 0x191, 0x14);
    }
}

/// `0xdbb0e` : ramassage réussi : bonus, événement de score et son. Renvoie faux (accepté).
fn equipement_accepte(ctx: &mut Ctx, o: Obj, joueur: Obj) -> bool {
    equipement_bonus(ctx, o);
    let evenement = 0x11 + ((joueur.u8(ctx.b, 2) as u16) << 8);
    crate::evenements::enfiler_points(ctx.b, evenement);
    effet_ramassage(ctx, ctx.obj(), 5);
    false
}

/// `0xdbafa` : ramassage refusé : l'objet sera rejeté.
fn equipement_refuse(ctx: &mut Ctx, o: Obj) -> bool {
    o.set_u8(ctx.b, champ::SOUS_ETAT, 6);
    o.set_u8(ctx.b, champ::SOUS_SOUS_ETAT, 0);
    o.set_u8(ctx.b, inventaire::DELAI, 0xc);
    true
}

/// Groupe d'un équipement (octet 7 de sa fiche).
fn equipement_groupe(ctx: &mut Ctx, identifiant: u8) -> u8 {
    let rang = ((identifiant.wrapping_sub(0x2e) as u32) << 3) & 0xffff;
    ctx.rom8(equipement::FICHES + 7 + rang)
}

/// `0xdbad8` : range l'équipement dans l'inventaire du propriétaire. Renvoie vrai si refusé.
fn equipement_ajoute(ctx: &mut Ctx, o: Obj) -> bool {
    let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
    let ligne = ((o.u8(ctx.b, Obj::VARIANT) as u32) << 3) & 0xffff;
    let classe = (joueur.u8(ctx.b, 3) >> 1) as u32;
    let autorise = ctx.rom8(equipement::FICHES + ligne + classe);
    if !(autorise != 0) {
        return equipement_refuse(ctx, o);
    }
    let mut id = ctx.rom8(equipement::FICHES + 6 + ligne);
    // déjà équipé ?
    for off in [equipement::PREMIER_EQUIPE, equipement::SECOND_EQUIPE] {
        let equipe = joueur.u8(ctx.b, off).wrapping_add(0x2e);
        if equipe == id {
            return equipement_accepte(ctx, o, joueur);
        }
    }
    // l'identifiant est-il déjà dans l'inventaire ?
    for i in 0..6u32 {
        let slot = joueur.u8(ctx.b, inventaire::IDENTIFIANTS + i);
        let vide = slot == inventaire::VIDE;
        if !vide {
            let objet = slot <= 0x2d;
            if objet {
                // objet d'inventaire : seulement s'il est identique
                if slot == id {
                    return equipement_accepte(ctx, o, joueur);
                }
            } else {
                let (g_slot, g_nouveau) = (equipement_groupe(ctx, slot), equipement_groupe(ctx, id));
                if !(g_nouveau != g_slot) {
                    // même groupe : on garde le meilleur
                    if !(id as i8 > slot as i8) {
                        id = slot;
                    }
                    joueur.set_u8(ctx.b, inventaire::IDENTIFIANTS + i, id);
                    return equipement_accepte(ctx, o, joueur);
                }
            }
        }
    }
    // équipement porté en main (octet 0x10d, ou 0x10e pour la classe 4)
    let mut porte = joueur.u8(ctx.b, equipement::PREMIER_EQUIPE);
    let classe_main = joueur.u8(ctx.b, 3) >> 1;
    if !(classe_main != 4) {
        porte = joueur.u8(ctx.b, equipement::SECOND_EQUIPE);
    }
    let groupe_porte = ctx.rom8(equipement::FICHES + 7 + ((porte as u32) << 3));
    let groupe_nouveau = equipement_groupe(ctx, id);
    if !(groupe_nouveau != groupe_porte) {
        let nouveau = id.wrapping_sub(0x2e);
        if !(porte as i8 > nouveau as i8) {
            joueur.set_u8(ctx.b, equipement::PREMIER_EQUIPE, nouveau);
        }
        return equipement_accepte(ctx, o, joueur);
    }
    // un emplacement libre
    for i in 0..6u32 {
        let slot = joueur.u8(ctx.b, inventaire::IDENTIFIANTS + i);
        if !(slot != inventaire::VIDE) {
            joueur.set_u8(ctx.b, inventaire::IDENTIFIANTS + i, id);
            equipement_bonus(ctx, o);
            return false;
        }
    }
    // échange avec le premier emplacement, sauf s'il est verrouillé pour cette classe
    let premier = joueur.u8(ctx.b, inventaire::IDENTIFIANTS);
    for k in 0..8u32 {
        let verrou = ctx.rom8(equipement::VERROUILLES + (classe << 3) + k);
        if premier == verrou {
            return equipement_refuse(ctx, o);
        }
    }
    let ancienne_quantite = joueur.u8(ctx.b, inventaire::QUANTITES);
    // D0.w : la classe × 8 (`lsr.w #1` puis `lsl.w #3` de l'octet +3) garde son octet haut sous l'identifiant ; il
    // n'est non nul que si le propriétaire n'est pas un joueur (pointeur nul : octets de la ROM)
    let index = ((classe << 3) as u16 & 0xff00) | premier as u16;
    objet_expulse(ctx, o, joueur, index, ancienne_quantite);
    joueur.set_u8(ctx.b, inventaire::IDENTIFIANTS, id);
    joueur.set_u8(ctx.b, inventaire::QUANTITES, 0);
    equipement_bonus(ctx, o);
    false
}

fn equipement_init(ctx: &mut Ctx, o: Obj) {
    let mode = o.u8(ctx.b, champ::MODE);
    if mode != 0 {
        let delai = o.u8(ctx.b, inventaire::DELAI);
        if !(delai == 0) {
            o.set_u8(ctx.b, inventaire::DELAI, delai - 1);
            return;
        }
        ajoute_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
    } else if en_l_air(ctx, o) {
        etat_suivant(ctx, o, sous::CHUTE);
        o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
    } else {
        etat_suivant(ctx, o, sous::ATTENTE);
        o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
        o.set_u16(ctx.b, champ::DUREE, 0x4b0);
    }
    let variante = o.u8(ctx.b, Obj::VARIANT);
    let indice = ctx.rom8(equipement::INDICES + variante as u32);
    o.set_u8(ctx.b, champ::INDICE, indice);
    anime_et_trie(ctx, equipement::ANIMATIONS, variante);
}

/// Sous-état 6 : rejet en arc de l'équipement refusé ; à l'atterrissage, une variante 0x0f ramassée
/// par la classe 1 plus de sept fois déclenche une scène.
fn equipement_rejet(ctx: &mut Ctx, o: Obj) {
    let lance = o.u8(ctx.b, champ::SOUS_SOUS_ETAT) != 0;
    if !lance {
        let delai = o.u8(ctx.b, inventaire::DELAI).wrapping_sub(1);
        o.set_u8(ctx.b, inventaire::DELAI, delai);
        if delai != 0 {
            return;
        }
        ajoute_u8(ctx, o, champ::SOUS_SOUS_ETAT, 2);
        let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
        let y = joueur.u16(ctx.b, Obj::Y).wrapping_sub(1);
        o.set_u16(ctx.b, Obj::Y, y);
        o.set_u16(ctx.b, Obj::Z, 0x10);
        o.set_u16(ctx.b, Obj::VZ, 0x200);
        o.set_u16(ctx.b, Obj::AZ, 0xffc0);
        o.set_u32(ctx.b, Obj::VX, 0x0280_0000);
        let (cam_x, _) = ctx.camera();
        let limite = cam_x.wrapping_add(0xc0);
        let x = o.u16(ctx.b, Obj::X);
        if !(limite as i16 > x as i16) {
            let v = o.u32(ctx.b, Obj::VX).wrapping_neg();
            o.set_u32(ctx.b, Obj::VX, v);
        }
        return ctx.reinsert();
    }
    ctx.move_xz();
    let z = o.u16(ctx.b, Obj::Z);
    if z as i16 >= 0 {
        return;
    }
    o.set_u8(ctx.b, champ::SOUS_ETAT, sous::ATTENTE);
    o.set_u8(ctx.b, champ::SOUS_SOUS_ETAT, 0);
    o.set_u32(ctx.b, Obj::Z, 0);
    o.set_u8(ctx.b, champ::RAMASSE, 0);
    let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
    let classe = joueur.u8(ctx.b, 3) >> 1;
    if classe != 1 {
        return;
    }
    let variante = o.u8(ctx.b, Obj::VARIANT);
    if variante != 0xf {
        return;
    }
    let n = ctx.ram8(0x57).wrapping_add(1);
    ctx.set_ram8(0x57, n);
    if n <= 7 {
        return;
    }
    o.set_u16(ctx.b, champ::DUREE, 0x4b0);
    o.set_u8(ctx.b, Obj::FLAGS, 1);
    o.set_u8(ctx.b, Obj::VARIANT, 0xc);
    ctx.anim_start(equipement::ANIMATIONS, 0xc);
    effects_creators::posed_flash(ctx.b, o);
    ctx.set_ram8(-0x5b83, 1);
    set_de_scene(ctx, 0x29);
}

fn equipement_actif(ctx: &mut Ctx, o: Obj) {
    let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
    let cible = super::effects::cible(ctx, equipement::SOUS_ETATS, sous_etat as u32);
    match sous_etat {
        sous::CHUTE => {
            let en_vol = ctx.move_xyz().gt();
            if !en_vol {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
                o.set_u16(ctx.b, champ::DUREE, 0x4b0);
                o.set_u32(ctx.b, Obj::Z, 0);
                let mode = o.u8(ctx.b, champ::MODE);
                if !(mode != 0) {
                    dans_la_vue(ctx, o);
                }
            }
            fin_de_chute(ctx);
        }
        sous::ATTENTE => reserve_attente(ctx, o),
        sous::RAMASSE => {
            let joueur = pointeur(ctx, o, champ::PROPRIETAIRE);
            let pret = joueur.u8(ctx.b, 0x33) != 0;
            if !pret {
                return;
            }
            let refuse = equipement_ajoute(ctx, o);
            if !refuse {
                ajoute_u8(ctx, o, Obj::STATE, 2);
            }
            marque_ramasse(ctx, o);
        }
        6 => equipement_rejet(ctx, o),
        _ => crate::families::effects::hors_table(cible),
    }
}

fn equipement_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = super::effects::cible(ctx, equipement::ETATS, etat as u32);
    match etat {
        etat::INIT => equipement_init(ctx, o),
        etat::ACTIF => equipement_actif(ctx, o),
        etat::LIBERE | etat::HORS_ECRAN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::hors_table(cible),
    }
}

gestionnaire!(ITEM_3, equipement::ENTREE, "item_equipement", equipement_gestionnaire);
gestionnaire!(ITEM_0, tresor::ENTREE, "item_tresor", tresor);
gestionnaire!(ITEM_1, conso::ENTREE, "item_conso", conso_gestionnaire);
gestionnaire!(ITEM_4, inventaire::ENTREE, "item_inventaire", inventaire_gestionnaire);
gestionnaire!(ITEM_5, bonus::ENTREE, "item_bonus", bonus_gestionnaire);
gestionnaire!(ITEM_6, scene::ENTREE, "item_scene", scene_gestionnaire);
gestionnaire!(ITEM_7, reserve::ENTREE, "item_reserve", reserve_gestionnaire);
gestionnaire!(ITEM_8, contact::ENTREE, "item_contact", contact_gestionnaire);
gestionnaire!(ITEM_9, accompagnement::ENTREE, "item_accompagnement", accompagnement_gestionnaire);

// ---------------------------------------------------------------------------------------------
// Sous-routines appelées hors des gestionnaires

/// `0xdabba` : efface le tableau des objets de collection ramassés (48 longs en `A5 - 0x5b88`).
/// Registres laissés comme l'original : D0 = 0, D6.w = 0xffff, A0 = fin du tableau.
fn efface_ramasses(c: &mut Cpu, b: &mut Cps2Bus) {
    let debut = A5.wrapping_add(RAMASSES as u32);
    for k in 0..48 {
        wr32(b, debut + 4 * k, 0);
    }
    c.d[0] = 0;
    c.d[6] = c.d[6] & 0xffff_0000 | 0xffff;
    c.a[0] = debut + 4 * 48;
    rts(c, b);
}

pub const EFFACE_RAMASSES: Native = Native {
    entry: 0xdabba,
    name: "item_efface_ramasses",
    run: crate::Corps::Processeur(efface_ramasses),
    touches_sound: false,
    outputs: regs::d(0) | regs::d(6) | regs::a(0),
};

/// Un équipement du joueur qui peut tomber quand il est touché : (octet de l'objet porté, compteur de
/// coups, table des chances par coup).
struct Porte {
    objet: u32,
    coups: u32,
    chances: u32,
}

/// Le joueur touché perd au plus un objet par coup : bouclier (`+0x165`, 0 à 3), arme spéciale (`+0x163`)
/// puis anneau (`+0x162`, sauf 1). Chaque coup décrémente un compteur ; tant qu'il reste positif, la chute
/// dépend d'un tirage (bit `rng & 0xf` du mot de la table pour ce compteur, en clair) ; l'objet tombe par
/// `0xcc352` (`creators::set_a`, D0.b = son numéro d'objet).
const BOUCLIER: Porte = Porte { objet: 0x165, coups: 0x16d, chances: 0xdb7c4 };
const ARME: Porte = Porte { objet: 0x163, coups: 0x16c, chances: 0xdb7b8 };
const ANNEAU: Porte = Porte { objet: 0x162, coups: 0x16b, chances: 0xdb7d0 };
/// Nombre d'objets lâchés à ce coup (mot).
const LACHES: u32 = A5 - 0x5ac8;

/// Le compteur de coups de `p` est épuisé, ou le tirage fait tomber l'objet.
fn tombe(c: &mut Cpu, b: &mut Cps2Bus, o: Obj, p: &Porte) -> bool {
    let n = o.u8(b, p.coups).wrapping_sub(1);
    o.set_u8(b, p.coups, n);
    if (n as i8) < 0 {
        return true;
    }
    c.a[4] = p.chances;
    let i = 2 * n as u32;
    c.d[0] = i;
    let masque = rd16(b, p.chances + i);
    c.d[2] = c.d[2] & 0xffff_0000 | masque as u32;
    let bit = crate::rng::random(b) & 0xf;
    c.d[0] = c.d[0] & 0xffff_0000 | bit as u32;
    c.d[2] >> bit & 1 != 0
}

fn lache(c: &mut Cpu, b: &mut Cps2Bus, o: Obj, p: &Porte, numero: u8) {
    c.d[0] = c.d[0] & !0xff | numero as u32;
    super::creators::set_a_par(c, b, numero, |b| o.u8(b, Obj::VARIANT));
    o.set_u8(b, p.objet, 0xff);
    o.set_u8(b, p.coups, 0);
}

fn compte_lache(b: &mut Cps2Bus) {
    let n = rd16(b, LACHES).wrapping_add(1);
    wr16(b, LACHES, n);
}

/// `0xdb6c0` : chute d'équipement du joueur A0 touché.
fn chute_equipement(c: &mut Cpu, b: &mut Cps2Bus) {
    let o = Obj(c.a[0]);
    wr16(b, LACHES, 0);
    let bouclier = o.u8(b, BOUCLIER.objet);
    if bouclier <= 3 && tombe(c, b, o, &BOUCLIER) {
        lache(c, b, o, &BOUCLIER, bouclier + 4);
        compte_lache(b);
    }
    if (o.u8(b, ARME.objet) as i8) >= 0 && tombe(c, b, o, &ARME) && rd16(b, LACHES) == 0 {
        c.d[0] = 0;
        lache(c, b, o, &ARME, 3);
        compte_lache(b);
    }
    let anneau = o.u8(b, ANNEAU.objet);
    if (anneau as i8) >= 0 && anneau != 1 && tombe(c, b, o, &ANNEAU) && rd16(b, LACHES) == 0 {
        c.d[0] = c.d[0] & !0xff | anneau as u32;
        super::creators::set_a_par(c, b, anneau, |b| o.u8(b, Obj::VARIANT));
        o.set_u8(b, ANNEAU.objet, 0xff);
        // le personnage 5 garde son anneau d'origine (1)
        let personnage = o.u8(b, 0x3) >> 1;
        c.d[0] = personnage as u32;
        if personnage == 5 {
            o.set_u8(b, ANNEAU.objet, 1);
        }
        o.set_u8(b, ANNEAU.coups, 0);
    }
    rts(c, b);
}

pub const CHUTE_EQUIPEMENT: Native = Native {
    entry: 0xdb6c0,
    name: "item_chute_equipement",
    run: crate::Corps::Processeur(chute_equipement),
    touches_sound: false,
    outputs: regs::d(0) | regs::d(2) | regs::a(4),
};
