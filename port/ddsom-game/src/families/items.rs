//! Famille ITEM (classe 0xC) : objets ramassables. Dispatch `0x39f34`, table des types `0x39f5a`
//! (10 types, 32 emplacements de 0x60 octets).
//!
//! Un objet ITEM naît (créé par un ennemi, un coffre ou un décor) à la position de son créateur ;
//! il tombe s'il est en l'air, reste au sol un temps limité en clignotant avant de disparaître, et
//! produit son effet quand un joueur le ramasse (le code du joueur pose `RAMASSE` et `OWNER`).
//! Voir `docs/families.md` pour la description de chaque type.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::mem::{rd16, wr16, wr32, A5};
use crate::objects::pools;
use crate::objects::Obj;

use super::{ajoute_u8, effects_creators, handler, pointeur};
use super::cx::{Bench, Cx};
use crate::objects::Current;
use crate::machine::Machine;

/// Valeurs de l'octet d'état `+4`.
mod state {
    pub const INIT: u8 = 0;
    pub const ACTIVE: u8 = 2;
    pub const LIBERE: u8 = 4;
    /// Posé par `obj_cull_offscreen` : traité comme `LIBERE`.
    pub const OUTSIDE_SCREEN: u8 = 6;
}

/// Valeurs de l'octet de sous-état `+5` de l'état actif (types à chute).
mod sub {
    pub const CHUTE: u8 = 0;
    pub const WAIT: u8 = 2;
    pub const RAMASSE: u8 = 4;
}

/// Champs propres aux ITEM.
mod field {
    pub const SUB_STATE: u32 = 0x05;
    pub const SUB_SUB_STATE: u32 = 0x06;
    pub const FLIP: u32 = 0x2c;
    pub const GAME_SHADOWS: u32 = 0x2e;
    pub const MODE: u32 = 0x2f;
    pub const DURATION: u32 = 0x42;
    pub const OWNER: u32 = 0x50;
    pub const SCINTILLEMENT: u32 = 0x54;
    pub const RAMASSE: u32 = 0x58;
    pub const INDEX: u32 = 0x5d;
}

/// Jeu d'ombres commun aux ITEM.
const SHADOWS: u8 = 0x3c;
/// Indicateurs « objet de collection ramassé » (un octet par objet), `A5 - 0x5b88`.
const PICKED: i32 = -0x5b88;

/// Sous-routines non réécrites.
mod call {
    /// Fin commune des effets de potion.
    pub const END_EFFECT: u32 = 0xebaf6;
}

/// `0xae3ec` : scintillement (EFFCT 15) au-dessus de l'objet, d'après `SCINTILLEMENT`.
fn scintille<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    effects_creators::twinkle(ctx.b, o);
}

/// `0xb12c0` (D0 = `variant`) : effet de ramassage (EFFCT 27) attaché au joueur propriétaire de l'objet.
fn effect_pickup<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, variant: u8) {
    effects_creators::pickup(ctx.b, o, variant);
}

/// `0xc8312` : objet SET de la scène ; `None` si le pool est plein.
fn scene_set_of<R: Bench>(ctx: &mut Cx<'_, R>, variant: u8) -> Option<Obj> {
    super::creators::scene_set(ctx.b, variant)
}

/// `0xaba16` : éclat à animation unique (EFFCT 7) sur l'objet.
fn eclat<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    effects_creators::flash(ctx.b, o);
}

/// Passe à l'état suivant avec un sous-état donné (`addq.b #2,(4,A0)`, sous-sous-état à zéro).
fn state_next<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, sub_state: u8) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, field::SUB_STATE, sub_state);
    o.set_u16(ctx.b, field::SUB_SUB_STATE, 0);
}

/// Un objet créé au-dessus du sol (altitude non nulle) commence par tomber.
fn airborne<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let z = o.u16(ctx.b, Obj::Z);
    !(z == 0) // tst.w (0xc,A0) ; beq : au sol
}

/// Marque l'objet de collection `+0x5d` comme ramassé.
fn mark_pickup<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let index = o.u8(ctx.b, field::INDEX) as i32;
    ctx.set_ram8(PICKED + index, 1);
}

/// Ramène l'objet dans la zone visible : au plus haut à `caméra.y + 0x20`, entre `caméra.x + 0x18`
/// et `caméra.x + 0x168` (bloc recopié dans chaque type).
fn in_view<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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
fn expire_and_blinks<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let remainder = o.u16(ctx.b, field::DURATION).wrapping_sub(1);
    o.set_u16(ctx.b, field::DURATION, remainder);
    if !(remainder as i16 >= 0) {
        o.set_u8(ctx.b, Obj::STATE, state::LIBERE);
        o.set_u8(ctx.b, Obj::ACTIVE, 2);
    }
    if remainder > 0x3c {
        return;
    }
    let phase = remainder.wrapping_add(ctx.counter());
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    if phase & 1 == 0 {
        return;
    }
    o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
}

/// Fin commune de l'état de chute : ramener dans la zone, ombre, tri par profondeur.
fn fall_end<R: Bench>(ctx: &mut Cx<'_, R>) {
    ctx.keep_in_zone();
    ctx.push_terrain();
    ctx.reinsert();
}

/// Démarre le script `number` du jeu d'animations `game`, puis trie l'objet (fin des initialisations).
fn animate_and_sort<R: Bench>(ctx: &mut Cx<'_, R>, game: u32, number: u8) {
    ctx.anim_start(game, number as u16);
    ctx.reinsert();
}

/// Sous-état « ramassé » : attend que le joueur ait fini son geste (propriétaire prêt) ou un
/// ramassage forcé (`RAMASSE` = 2). Renvoie faux tant qu'il faut attendre.
fn pickup_ready<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let ramasse = o.u8(ctx.b, field::RAMASSE);
    if ramasse == 2 {
        return true;
    }
    let player = pointeur(ctx, o, field::OWNER);
    let ready = player.u8(ctx.b, 0x33) != 0;
    ready
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
    pub const ENTRY: u32 = 0xdabca;
    pub const TABLE_STATES: u32 = 0xdabe0;
    pub const TABLE_SUB_STATES: u32 = 0xdac68;
    /// Paramètres par variante (8 octets) : lus en espace programme.
    pub const PARAMS: u32 = 0xdadb6;
    pub const ANIMATIONS: u32 = 0x1bed1a;
    pub const DURATION_TO_GROUND: u16 = 0x4b0;
    pub const SCORE: u32 = 0x56;
    pub const SOUND: u32 = 0x57;
}

/// `0xdad8e` : recopie les paramètres de la variante.
fn treasure_params<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let base = tresor::PARAMS + ((o.u8(ctx.b, Obj::VARIANT) as u32) << 3);
    for (i, off) in [field::SCINTILLEMENT, 0x55, tresor::SCORE, tresor::SOUND, field::INDEX].into_iter().enumerate() {
        let v = ctx.prog8(base + i as u32);
        o.set_u8(ctx.b, off, v);
    }
}

fn tresor_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if airborne(ctx, o) {
        state_next(ctx, o, sub::CHUTE);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        treasure_params(ctx, o);
    } else {
        state_next(ctx, o, sub::WAIT);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        o.set_u16(ctx.b, field::DURATION, tresor::DURATION_TO_GROUND);
        treasure_params(ctx, o);
        scintille(ctx, o);
    }
    animate_and_sort(ctx, tresor::ANIMATIONS, variant);
}

fn treasure_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = super::effects::target(ctx, tresor::TABLE_SUB_STATES, sub_state as u32);
    match sub_state {
        sub::CHUTE => {
            let in_flight = ctx.move_xyz().gt();
            if !in_flight {
                // posé : attente au sol
                ajoute_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u16(ctx.b, field::DURATION, tresor::DURATION_TO_GROUND);
                o.set_u32(ctx.b, Obj::Z, 0);
                in_view(ctx, o);
                scintille(ctx, o);
            }
            fall_end(ctx);
        }
        sub::WAIT => {
            // en cinématique, un objet resté à gauche de l'écran disparaît
            let cinematique = ctx.ram8(0x44) != 0;
            if cinematique {
                let limite = crate::ram::CAMERA_X.get(ctx.b).wrapping_sub(0x40);
                let x = o.u16(ctx.b, Obj::X);
                if !((limite as i16) < x as i16) {
                    o.set_u8(ctx.b, Obj::STATE, state::OUTSIDE_SCREEN);
                    return;
                }
            }
            let ramasse = o.u8(ctx.b, field::RAMASSE);
            if !(ramasse == 0) {
                ajoute_u8(ctx, o, field::SUB_STATE, 2);
            }
            expire_and_blinks(ctx, o);
        }
        sub::RAMASSE => {
            if !pickup_ready(ctx, o) {
                return;
            }
            ajoute_u8(ctx, o, Obj::STATE, 2);
            let sound = o.u8(ctx.b, tresor::SOUND);
            effect_pickup(ctx, o, sound);
            let player_bits = o.u16(ctx.b, 0x48) & 0x300;
            let event = (o.u8(ctx.b, tresor::SCORE) as u16).wrapping_add(player_bits);
            crate::events::enqueue_points(ctx.b, event);
            mark_pickup(ctx, o);
        }
        _ => crate::families::effects::outside_table(target),
    }
}

fn tresor<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, tresor::TABLE_STATES, state as u32);
    match state {
        state::INIT => tresor_init(ctx, o),
        state::ACTIVE => treasure_active(ctx, o),
        state::LIBERE | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
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
    pub const ENTRY: u32 = 0xdae26;
    pub const TABLE_STATES: u32 = 0xdae3c;
    pub const TABLE_SUB_STATES: u32 = 0xdaef6;
    pub const PARAMS: u32 = 0xdaedc;
    pub const EFFECTS: u32 = 0xdafb8;
    pub const ANIMATIONS: u32 = 0x1bee16;
    pub const DURATION_INITIAL: u16 = 0x960;
    pub const DURATION_AFTER_FALL: u16 = 0x708;
    /// Effets de ramassage (adresses d'origine, lues dans la table `EFFECTS`).
    pub const NOURRITURE: u32 = 0xdafd0;
    pub const POTION_B: u32 = 0xdb044;
    pub const POTION_A: u32 = 0xdb026;
    pub const WITHOUT_EFFECT: u32 = 0xdb068;
    pub const COUNTER: u32 = 0xdb06a;
}

fn consumable_params<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let base = conso::PARAMS + ((o.u8(ctx.b, Obj::VARIANT) as u32) * 2 & 0xff);
    let scintillement = ctx.prog8(base);
    o.set_u8(ctx.b, field::SCINTILLEMENT, scintillement);
    let index = ctx.prog8(base + 1);
    o.set_u8(ctx.b, field::INDEX, index);
}

fn conso_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if airborne(ctx, o) {
        state_next(ctx, o, sub::CHUTE);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        o.set_u16(ctx.b, field::DURATION, conso::DURATION_INITIAL);
        consumable_params(ctx, o);
    } else {
        state_next(ctx, o, sub::WAIT);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        o.set_u16(ctx.b, field::DURATION, conso::DURATION_INITIAL);
        consumable_params(ctx, o);
        scintille(ctx, o);
    }
    animate_and_sort(ctx, conso::ANIMATIONS, variant);
}

/// Effet d'une nourriture : soigne le joueur ; à pleine santé, événement de score et son.
fn effect_food<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let player = pointeur(ctx, o, field::OWNER);
    let (life_max, life) = (player.u16(ctx.b, 0x60), player.u16(ctx.b, 0x62));
    if !(life_max != life) {
        let event = 0x11 + ((player.u8(ctx.b, 2) as u16) << 8);
        crate::events::enqueue_points(ctx.b, event);
        effect_pickup(ctx, o, 5);
        return;
    }
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let soin: u16 = if variant == 0 { 0x14 } else { 0x28 };
    let life = life.wrapping_add(soin);
    player.set_u16(ctx.b, 0x62, life);
    if !(life <= life_max) {
        player.set_u16(ctx.b, 0x62, life_max);
    }
    crate::sound::play(ctx.b, call::END_EFFECT, ctx.obj());
}

/// Potions : le joueur reçoit un compte à rebours de 0x2d0 images et des drapeaux +0x180..0x184.
fn effect_potion<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, bytes: &[(u32, u8)]) {
    let player = pointeur(ctx, o, field::OWNER);
    player.set_u16(ctx.b, 0x180, 0x2d0);
    for &(off, v) in bytes {
        player.set_u8(ctx.b, off, v);
    }
    crate::sound::play(ctx.b, call::END_EFFECT, ctx.obj());
}

fn consumable_effect<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT) as u32;
    let target = ctx.prog32(conso::EFFECTS + 4 * variant);
    // registres de l'original au moment du `jsr (A1)` (utiles si la variante est hors table)
    match target {
        conso::NOURRITURE => effect_food(ctx, o),
        conso::POTION_B => effect_potion(ctx, o, &[(1, 1), (0x183, 1), (0x184, 0), (0x182, 0)]),
        conso::POTION_A => effect_potion(ctx, o, &[(0x184, 1), (0x183, 0), (0x182, 0)]),
        conso::WITHOUT_EFFECT => {}
        conso::COUNTER => {
            let n = ctx.ram8(0x45).wrapping_add(1);
            ctx.set_ram8(0x45, n);
            if !(n <= 9) {
                ctx.set_ram8(0x45, 9);
            }
        }
        _ => crate::families::effects::outside_table(target),
    }
    mark_pickup(ctx, o);
}

fn consumable_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = super::effects::target(ctx, conso::TABLE_SUB_STATES, sub_state as u32);
    match sub_state {
        sub::CHUTE => {
            let in_flight = ctx.move_xyz().gt();
            if !in_flight {
                ajoute_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u16(ctx.b, field::DURATION, conso::DURATION_AFTER_FALL);
                o.set_u32(ctx.b, Obj::Z, 0);
                in_view(ctx, o);
                scintille(ctx, o);
            }
            fall_end(ctx);
        }
        sub::WAIT => {
            let ramasse = o.u8(ctx.b, field::RAMASSE);
            if !(ramasse == 0) {
                ajoute_u8(ctx, o, field::SUB_STATE, 2);
            }
            // seules les variantes 2 et suivantes disparaissent
            let variant = o.u8(ctx.b, Obj::VARIANT);
            if (variant == 0) || (variant == 1) {
                return;
            }
            expire_and_blinks(ctx, o);
        }
        sub::RAMASSE => {
            if !pickup_ready(ctx, o) {
                return;
            }
            ajoute_u8(ctx, o, Obj::STATE, 2);
            consumable_effect(ctx, o);
        }
        _ => crate::families::effects::outside_table(target),
    }
}

fn consumable_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, conso::TABLE_STATES, state as u32);
    match state {
        state::INIT => conso_init(ctx, o),
        state::ACTIVE => consumable_active(ctx, o),
        state::LIBERE | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
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
    pub const ENTRY: u32 = 0xdbdf6;
    pub const TABLE_STATES: u32 = 0xdbe0c;
    pub const TABLE_SUB_STATES: u32 = 0xdbf36;
    pub const ANIMATIONS: u32 = 0x1bf3c6;
    /// Scintillement par variante ; indice de ramassage par variante.
    pub const SCINTILLEMENTS: u32 = 0xdbf04;
    pub const INDICES: u32 = 0xdbf16;
    /// Par variante, 8 octets : identifiant de l'objet puis un octet d'interdiction par classe.
    pub const SHEETS: u32 = 0xdc1fa;
    /// Par classe de joueur, 8 identifiants d'objets qui ne peuvent pas être remplacés.
    pub const VERROUILLES: u32 = 0xdc1ca;
    /// Par identifiant d'objet : (type, variante) de l'ITEM recréé quand il est remplacé.
    pub const RECREES: u32 = 0xdc2d6;
    pub const DURATION_TO_GROUND: u16 = 0x4b0;
    pub const EMPTY: u8 = 0x56;
    /// Champs du joueur.
    pub const IDENTIFIANTS: u32 = 0x140;
    pub const QUANTITES: u32 = 0x148;
    /// Quantité de l'objet (`+0x5a`) et délai avant apparition (`+0x59`).
    pub const QUANTITE: u32 = 0x5a;
    pub const DELAY: u32 = 0x59;
}

/// `0xdc28a` : recrée en l'air l'objet `old` (remplacé dans l'inventaire) avec sa quantité.
/// `index` : D0.w de l'appelant, l'identifiant dans l'octet bas (l'octet haut n'est nul qu'avec un vrai joueur, voir
/// `equipement_ajoute`).
fn object_ejected<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, player: Obj, index: u16, quantite: u8) {
    let Some(n) = ctx.alloc(&pools::ITEM) else {
        return;
    };
    n.set_u8(ctx.b, Obj::ACTIVE, 1);
    n.set_u8(ctx.b, field::MODE, 1);
    n.set_u8(ctx.b, inventaire::QUANTITE, quantite);
    // `add.w D0,D0` puis `(d8,PC,D0.w)` : mot étendu en signe
    let rank = index.wrapping_mul(2) as i16 as i32 as u32;
    let kind = ctx.prog8(inventaire::RECREES.wrapping_add(rank));
    n.set_u8(ctx.b, Obj::KIND, kind);
    let variant = ctx.prog8(inventaire::RECREES.wrapping_add(rank).wrapping_add(1));
    n.set_u8(ctx.b, Obj::VARIANT, variant);
    n.set_u16(ctx.b, field::OWNER, player.0 as u16);
    let x = o.u16(ctx.b, Obj::X);
    n.set_u16(ctx.b, Obj::X, x);
    let y = player.u16(ctx.b, Obj::Y).wrapping_sub(1);
    n.set_u16(ctx.b, Obj::Y, y);
    n.set_u16(ctx.b, Obj::Z, 0x10);
    n.set_u16(ctx.b, Obj::AZ, 0xffc0);
    n.set_u8(ctx.b, inventaire::DELAY, 0xc);
}

/// Ramassage refusé (`0xdc19a`) : l'objet sera rejeté (sous-état 6).
fn ramassage_refuse<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    o.set_u8(ctx.b, field::SUB_STATE, 6);
    o.set_u8(ctx.b, field::SUB_SUB_STATE, 0);
    o.set_u8(ctx.b, inventaire::DELAY, 0xc);
    true
}

/// Événement de score et son d'un objet qui ne change rien d'autre (`0xdc124`).
fn event_inventory<R: Bench>(ctx: &mut Cx<'_, R>, player: Obj) {
    let character = player.u8(ctx.b, 2) as u32;
    crate::events::enqueue_points(ctx.b, ((character << 8) + 0x11) as u16);
    effect_pickup(ctx, ctx.obj(), 5);
}

/// `0xdc0aa` : range l'objet dans l'inventaire du propriétaire. Renvoie vrai si le ramassage est refusé.
fn inventaire_ajoute<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let player = pointeur(ctx, o, field::OWNER);
    let line = (o.u8(ctx.b, Obj::VARIANT) as u32) << 3 & 0xffff;
    let id = ctx.data8(inventaire::SHEETS + line);
    let class = player.u8(ctx.b, 3) >> 1;
    let interdit = ctx.data8(inventaire::SHEETS + 1 + (class.wrapping_add(line as u8)) as u32);
    if interdit != 0 {
        return ramassage_refuse(ctx, o);
    }
    if !(id != 0x23) && (class == 2) {
        event_inventory(ctx, player);
        return false;
    }
    let quantite = o.u8(ctx.b, inventaire::QUANTITE);
    // 1. l'objet est déjà porté : la quantité augmente
    for i in 0..6u32 {
        let door = player.u8(ctx.b, inventaire::IDENTIFIANTS + i);
        if door != id {
            continue;
        }
        let n = player.u8(ctx.b, inventaire::QUANTITES + i).wrapping_add(quantite);
        player.set_u8(ctx.b, inventaire::QUANTITES + i, n);
        let variant = o.u8(ctx.b, Obj::VARIANT);
        // variantes à exemplaire unique : 7, 0x10, 0x11
        let unique = (variant == 7) || (variant == 0x10) || (variant == 0x11);
        if unique {
            player.set_u8(ctx.b, inventaire::QUANTITES + i, 1);
        } else if !(n <= 9) {
            player.set_u8(ctx.b, inventaire::QUANTITES + i, 9);
        } else {
            return false;
        }
        event_inventory(ctx, player);
        return false;
    }
    // 2. un emplacement vide
    for i in 0..6u32 {
        let door = player.u8(ctx.b, inventaire::IDENTIFIANTS + i);
        if door != inventaire::EMPTY {
            continue;
        }
        player.set_u8(ctx.b, inventaire::IDENTIFIANTS + i, id);
        player.set_u8(ctx.b, inventaire::QUANTITES + i, quantite);
        return false;
    }
    // 3. échange avec le premier emplacement, sauf s'il est verrouillé pour cette classe
    let first = player.u8(ctx.b, inventaire::IDENTIFIANTS);
    for k in 0..8u32 {
        let lock = ctx.data8(inventaire::VERROUILLES + ((class as u32) << 3) + k);
        if !(first != lock) {
            return ramassage_refuse(ctx, o);
        }
    }
    let old_quantity = player.u8(ctx.b, inventaire::QUANTITES);
    player.set_u8(ctx.b, inventaire::IDENTIFIANTS, id);
    player.set_u8(ctx.b, inventaire::QUANTITES, quantite);
    object_ejected(ctx, o, player, first as u16, old_quantity);
    false
}

fn inventaire_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let mode = o.u8(ctx.b, field::MODE);
    if mode != 0 {
        // objet jeté : attend la fin du délai
        let delay = o.u8(ctx.b, inventaire::DELAY);
        if !(delay == 0) {
            o.set_u8(ctx.b, inventaire::DELAY, delay - 1);
            return;
        }
        ajoute_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        let index = ctx.data8(inventaire::INDICES + variant as u32);
        o.set_u8(ctx.b, field::INDEX, index);
        animate_and_sort(ctx, inventaire::ANIMATIONS, variant);
        return;
    }
    if airborne(ctx, o) {
        state_next(ctx, o, sub::CHUTE);
        o.set_u8(ctx.b, inventaire::QUANTITE, 1);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        let scintillement = ctx.data8(inventaire::SCINTILLEMENTS + variant as u32);
        o.set_u8(ctx.b, field::SCINTILLEMENT, scintillement);
        let index = ctx.data8(inventaire::INDICES + variant as u32);
        o.set_u8(ctx.b, field::INDEX, index);
    } else {
        state_next(ctx, o, sub::WAIT);
        o.set_u8(ctx.b, inventaire::QUANTITE, 1);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        o.set_u16(ctx.b, field::DURATION, inventaire::DURATION_TO_GROUND);
        let scintillement = ctx.data8(inventaire::SCINTILLEMENTS + variant as u32);
        o.set_u8(ctx.b, field::SCINTILLEMENT, scintillement);
        let index = ctx.data8(inventaire::INDICES + variant as u32);
        o.set_u8(ctx.b, field::INDEX, index);
        scintille(ctx, o);
    }
    animate_and_sort(ctx, inventaire::ANIMATIONS, variant);
}

/// Sous-état 6 : l'objet refusé est rejeté en arc vers l'extérieur de l'écran puis retombe.
fn inventaire_rejet<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let launches = o.u8(ctx.b, field::SUB_SUB_STATE) != 0;
    if !launches {
        // attente du délai, puis lancer
        let delay = o.u8(ctx.b, inventaire::DELAY).wrapping_sub(1);
        o.set_u8(ctx.b, inventaire::DELAY, delay);
        if delay != 0 {
            return;
        }
        ajoute_u8(ctx, o, field::SUB_SUB_STATE, 2);
        let player = pointeur(ctx, o, field::OWNER);
        let y = player.u16(ctx.b, Obj::Y).wrapping_sub(1);
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
    o.set_u8(ctx.b, field::SUB_STATE, sub::WAIT);
    o.set_u8(ctx.b, field::SUB_SUB_STATE, 0);
    o.set_u32(ctx.b, Obj::Z, 0);
    o.set_u8(ctx.b, field::RAMASSE, 0);
}

fn inventory_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = super::effects::target(ctx, inventaire::TABLE_SUB_STATES, sub_state as u32);
    match sub_state {
        sub::CHUTE => {
            let in_flight = ctx.move_xyz().gt();
            if !in_flight {
                ajoute_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u16(ctx.b, field::DURATION, inventaire::DURATION_TO_GROUND);
                o.set_u32(ctx.b, Obj::Z, 0);
                let mode = o.u8(ctx.b, field::MODE);
                if !(mode != 0) {
                    in_view(ctx, o);
                    scintille(ctx, o);
                }
            }
            ctx.anim_step();
            fall_end(ctx);
        }
        sub::WAIT => {
            let ramasse = o.u8(ctx.b, field::RAMASSE);
            if !(ramasse == 0) {
                ajoute_u8(ctx, o, field::SUB_STATE, 2);
            }
            ctx.anim_step();
            expire_and_blinks(ctx, o);
        }
        sub::RAMASSE => {
            let player = pointeur(ctx, o, field::OWNER);
            let ready = player.u8(ctx.b, 0x33) != 0;
            if !ready {
                return;
            }
            let refuse = inventaire_ajoute(ctx, o);
            if !refuse {
                ajoute_u8(ctx, o, Obj::STATE, 2);
            }
            mark_pickup(ctx, o);
        }
        6 => inventaire_rejet(ctx, o),
        _ => crate::families::effects::outside_table(target),
    }
}

fn inventory_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, inventaire::TABLE_STATES, state as u32);
    match state {
        state::INIT => inventaire_init(ctx, o),
        state::ACTIVE => inventory_active(ctx, o),
        state::LIBERE | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 5 : bonus animés (0xdc382)

/// Type 5, bonus animés : lancés en l'air selon un mode de création (`+0x2f` : vitesses initiales
/// lues dans une table de huit octets par mode), ils retombent, attendent 0x4b0 images en clignotant
/// et donnent au ramassage un événement de score (`0x19b08`, variante + joueur).
mod bonus {
    pub const ENTRY: u32 = 0xdc382;
    pub const TABLE_STATES: u32 = 0xdc398;
    pub const TABLE_SUB_STATES: u32 = 0xdc4f2;
    pub const ANIMATIONS: u32 = 0x1bf5c2;
    /// Par mode : vx, vz, az, vy (mots), lus en espace programme.
    pub const SPEEDS: u32 = 0xdc42c;
    pub const DURATION_TO_GROUND: u16 = 0x4b0;
    /// Écart entre l'animation en chute et l'animation au sol, et animation au sol d'un objet posé.
    pub const ANIMATION_GROUND: u8 = 6;
    pub const ANIMATION_POSE: u8 = 0xc;
}

fn bonus_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if airborne(ctx, o) {
        state_next(ctx, o, sub::CHUTE);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        let mode = o.u8(ctx.b, field::MODE);
        if !(mode == 0) {
            let base = bonus::SPEEDS + ((mode as u32) << 3);
            for (i, off) in [Obj::VX, Obj::VZ, Obj::AZ, Obj::VY].into_iter().enumerate() {
                let v = ctx.prog16(base + 2 * i as u32);
                o.set_u16(ctx.b, off, v);
            }
        }
        animate_and_sort(ctx, bonus::ANIMATIONS, variant);
    } else {
        state_next(ctx, o, sub::WAIT);
        o.set_u16(ctx.b, field::DURATION, bonus::DURATION_TO_GROUND);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        animate_and_sort(ctx, bonus::ANIMATIONS, variant.wrapping_add(bonus::ANIMATION_POSE));
    }
}

fn bonus_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = super::effects::target(ctx, bonus::TABLE_SUB_STATES, sub_state as u32);
    match sub_state {
        sub::CHUTE => {
            ctx.anim_step();
            let in_flight = ctx.move_xyz().gt();
            if !in_flight {
                ajoute_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u16(ctx.b, field::DURATION, bonus::DURATION_TO_GROUND);
                o.set_u32(ctx.b, Obj::Z, 0);
                let variant = o.u8(ctx.b, Obj::VARIANT);
                ctx.anim_start(bonus::ANIMATIONS, variant.wrapping_add(bonus::ANIMATION_GROUND) as u16);
                in_view(ctx, o);
            }
            fall_end(ctx);
        }
        sub::WAIT => {
            let ramasse = o.u8(ctx.b, field::RAMASSE);
            if !(ramasse == 0) {
                ajoute_u8(ctx, o, field::SUB_STATE, 2);
            }
            ctx.anim_step();
            expire_and_blinks(ctx, o);
        }
        sub::RAMASSE => {
            if !pickup_ready(ctx, o) {
                return;
            }
            ajoute_u8(ctx, o, Obj::STATE, 2);
            let player_bits = o.u16(ctx.b, 0x48) & 0x300;
            let event = (o.u8(ctx.b, Obj::VARIANT) as u16).wrapping_add(player_bits);
            crate::events::enqueue_counter(ctx.b, event);
        }
        _ => crate::families::effects::outside_table(target),
    }
}

fn bonus_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, bonus::TABLE_STATES, state as u32);
    match state {
        state::INIT => bonus_init(ctx, o),
        state::ACTIVE => bonus_active(ctx, o),
        state::LIBERE | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
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
    pub const ENTRY: u32 = 0xdc624;
    pub const TABLE_VARIANTS: u32 = 0xdc63c;
    pub const ANIMATIONS: u32 = 0x1bf8fe;
    // icône
    pub const ICON_STATES: u32 = 0xdc654;
    pub const ICON_SUB_STATES: u32 = 0xdc6ac;
    pub const ICONE_ANIMATIONS: u32 = 0xdc69c;
    pub const ICON_SOUNDS: u32 = 0xdc6b2;
    pub const ICONE_ENVOL: u32 = 0xdc7a0;
    pub const OWNER_2: u32 = 0x46;
    pub const OFFSETS_X: u32 = 0xdcdd8;
    pub const DELAY_SOUND: u8 = 0x3c;
    // guide
    pub const GUIDE_STATES: u32 = 0xdcb30;
    pub const GUIDE_SUB_STATES: u32 = 0xdcb92;
    pub const GUIDE_ANIMATIONS: u32 = 0xdcb6e;
    pub const GUIDE_ANIMATIONS_ARRIVAL: u32 = 0xdcbbc;
    pub const TABLE_SPEEDS: u32 = 0x1c65b6;
    /// Champs.
    pub const DELAY: u32 = 0x4c;
    pub const TARGET_X: u32 = 0x52;
    pub const TARGET_Y: u32 = 0x54;
    pub const MINUTEUR: u32 = 0x56;
    pub const CREATED: u32 = 0x5c;
    /// Variables de scène.
    pub const PAUSE: i32 = 0x42;
    pub const SCENE_FIGEE: i32 = 0x3d;
    pub const LEVEL: i32 = -0x5ac6;
    pub const COUNTER: i32 = -0x55c8;
    pub const PLAYERS: u32 = 0xff85de;
    pub const STEP_PLAYER: u32 = 0x200;
    // sous-routines non réécrites
}

/// `0xdcdae` : se place à côté du propriétaire (un pixel devant, décalé selon l'orientation).
/// Renvoie le propriétaire.
fn follow_owner<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> Obj {
    let player = pointeur(ctx, o, field::OWNER);
    let x = player.u32(ctx.b, Obj::X);
    o.set_u32(ctx.b, Obj::X, x);
    let y = player.u32(ctx.b, Obj::Y);
    o.set_u32(ctx.b, Obj::Y, y);
    let y = o.u16(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u16(ctx.b, Obj::Y, y);
    let retourne = o.u8(ctx.b, 0x2c) != 0;
    let offset = if !retourne { 0 } else { 2 };
    let dx = ctx.prog16(scene::OFFSETS_X + offset);
    let x = o.u16(ctx.b, Obj::X).wrapping_add(dx);
    o.set_u16(ctx.b, Obj::X, x);
    player
}

fn icone_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, field::SUB_STATE, 0);
    let player = follow_owner(ctx, o);
    let z = player.u32(ctx.b, Obj::Z);
    o.set_u32(ctx.b, Obj::Z, z);
    let z = o.u16(ctx.b, Obj::Z).wrapping_add(0x18);
    o.set_u16(ctx.b, Obj::Z, z);
    o.set_u8(ctx.b, scene::DELAY, scene::DELAY_SOUND);
    o.set_u8(ctx.b, Obj::ACTIVE, 2);
    let rng = ctx.random();
    let draw = rng & 3;
    o.set_u8(ctx.b, field::MODE, draw as u8);
    let animation = ctx.prog8(scene::ICONE_ANIMATIONS + draw as u32);
    ctx.anim_start(scene::ANIMATIONS, animation as u16);
}

/// `0xdc6e8` : toutes les 0x3c images (hors pause), joue le son de l'icône tiré dans la table
/// (`base` : 0 ou 0x10 selon l'appelant, première ou seconde série de sons).
fn icon_sound<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, base: u32) {
    let paused = ctx.ram8(scene::PAUSE) != 0;
    if paused {
        return;
    }
    let remainder = o.u8(ctx.b, scene::DELAY).wrapping_sub(1);
    o.set_u8(ctx.b, scene::DELAY, remainder);
    if remainder != 0 {
        return;
    }
    o.set_u8(ctx.b, scene::DELAY, scene::DELAY_SOUND);
    let draw = o.u8(ctx.b, field::MODE) as u32;
    let sound = ctx.prog32(scene::ICON_SOUNDS + base + draw * 4);
    crate::sound::play(ctx.b, sound, ctx.obj());
}

/// Sous-état 0 de l'icône : suit le joueur, anime, trie, et joue le son tiré toutes les 0x3c images.
fn icone_suivi<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    follow_owner(ctx, o);
    ctx.anim_step();
    o.set_u8(ctx.b, 0x3b, 0);
    ctx.reinsert();
    icon_sound(ctx, o, 0);
}

/// Vitesses de l'envol (`0xdc7c2`) : retournées avec l'orientation, puis pas d'animation.
fn icon_takeoff_speed<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let flip = o.u8(ctx.b, field::FLIP) != 0;
    if flip {
        let v = o.u16(ctx.b, Obj::VX).wrapping_neg();
        o.set_u16(ctx.b, Obj::VX, v);
    }
    ctx.anim_step();
}

/// Sous-état 2 de l'icône : l'icône s'envole en arc (quatre temps, selon `+6`).
fn icone_envol<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let time = o.u8(ctx.b, field::SUB_SUB_STATE);
    let target = super::effects::target(ctx, scene::ICONE_ENVOL, time as u32);
    match time {
        0 | 4 => {
            ajoute_u8(ctx, o, field::SUB_SUB_STATE, 2);
            o.set_u16(ctx.b, Obj::VX, 0x200);
            o.set_u16(ctx.b, Obj::VZ, if time == 0 { 0x400 } else { 0x200 });
            o.set_u16(ctx.b, Obj::AZ, 0xff80);
            o.set_u16(ctx.b, Obj::AX, 0);
            icon_takeoff_speed(ctx, o);
        }
        2 => {
            ctx.move_xz();
            let z = o.u16(ctx.b, Obj::Z);
            if !(z as i16 > 0) {
                ajoute_u8(ctx, o, field::SUB_SUB_STATE, 2);
                o.set_u32(ctx.b, Obj::Z, 0);
            }
            ctx.anim_step();
        }
        6 => {
            ctx.move_xz();
            let z = o.u16(ctx.b, Obj::Z);
            if !(z as i16 > 0) {
                ajoute_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
                o.set_u8(ctx.b, field::SUB_SUB_STATE, 0);
                o.set_u32(ctx.b, Obj::Z, 0);
                o.set_u8(ctx.b, Obj::ACTIVE, 1);
                o.set_u16(ctx.b, scene::MINUTEUR, 0x12c);
            }
            ctx.anim_step();
        }
        _ => crate::families::effects::outside_table(target),
    }
}

/// Sous-état 4 de l'icône : attend `+0x58`, rejoue son son, puis disparaît quand le délai `+0x56` est
/// écoulé (ou revient au suivi si le propriétaire reprend).
fn icon_wait<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let ramasse = o.u8(ctx.b, field::RAMASSE);
    if ramasse == 1 {
        let pause = ctx.ram8(scene::PAUSE) != 0;
        if pause {
            o.set_u8(ctx.b, field::RAMASSE, 0);
        } else {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, field::SUB_STATE, 0);
            let owner = pointeur(ctx, o, scene::OWNER_2);
            let variant = o.u8(ctx.b, Obj::VARIANT);
            if !(variant != 2) {
                let reserve = owner.u8(ctx.b, 0x31);
                if !(reserve == 0) {
                    o.set_u8(ctx.b, Obj::VARIANT, 0);
                }
            }
            ajoute_u8(ctx, owner, 0x31, 1);
            return;
        }
    }
    icon_sound(ctx, o, 0x10);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if !(variant != 2) {
        let owner = pointeur(ctx, o, scene::OWNER_2);
        let reserve = owner.u8(ctx.b, 0x31);
        if !(reserve == 0) {
            o.set_u16(ctx.b, scene::MINUTEUR, 1);
        }
    }
    let remainder = o.u16(ctx.b, scene::MINUTEUR).wrapping_sub(1);
    o.set_u16(ctx.b, scene::MINUTEUR, remainder);
    if !(remainder != 0) {
        o.set_u8(ctx.b, Obj::VARIANT, 0);
        o.set_u32(ctx.b, Obj::STATE, 0x0400_0000);
    }
    ctx.anim_step();
    ctx.reinsert();
}

/// État 4 de l'icône (`0xdc9b8`) : l'icône quitte la scène. Elle part (étincelle, son, routine propre à son
/// mode), marche en X jusqu'à sortir de l'écran, ou s'arrête près du joueur, crée un SET et attend comme le
/// guide (`0xdcc5a`, `0xdcc82`).
mod start {
    pub const SUB_STATES: u32 = 0xdc9c4;
    /// Par mode : routine appelée au départ (longs, 0 = aucune), animation de départ, son et animation
    /// d'arrêt.
    pub const ROUTINES: u32 = 0xdca50;
    pub const ANIMATIONS: u32 = 0xdca60;
    pub const SOUNDS_STOP: u32 = 0xdcaa0;
    pub const ANIMATIONS_STOP: u32 = 0xdca9c;
    pub const SOUND_STOP: u32 = 0xe1d60;
    /// Compteur d'icônes de l'objet `+0x46`.
    pub const ICONES: u32 = 0x30;
    /// Arrêt demandé (`+0x33`).
    pub const STOP: u32 = 0x33;
}

fn start_launches<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    eclat(ctx, o);
    ajoute_u8(ctx, o, field::SUB_STATE, 2);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    ajoute_u8(ctx, o, field::SUB_STATE, variant);
    o.set_u16(ctx.b, Obj::VX, 0x380);
    if o.u8(ctx.b, field::RAMASSE) != 0 {
        o.set_u16(ctx.b, Obj::VX, 0x200);
        let player = pointeur(ctx, o, field::OWNER);
        // le son se joue sur l'icône si le joueur n'est pas devant elle
        if player.u16(ctx.b, Obj::Y) as i16 >= o.u16(ctx.b, Obj::Y) as i16 {
            o.set_u16(ctx.b, field::OWNER, o.0 as u16);
        }
        effect_pickup(ctx, o, 0xa);
        o.set_u16(ctx.b, field::OWNER, player.0 as u16);
        let mode = o.u8(ctx.b, field::MODE) as u32;
        // departure sound of the mode (0xe1da0, 0xe1d60), played on the icon
        let sound = ctx.prog32(start::ROUTINES + 4 * mode);
        if sound != 0 {
            crate::sound::play(ctx.b, sound, o);
        }
    }
    if o.u8(ctx.b, 0x2c) == 0 {
        let v = o.u16(ctx.b, Obj::VX).wrapping_neg();
        o.set_u16(ctx.b, Obj::VX, v);
    }
    let mode = o.u8(ctx.b, field::MODE) as u32;
    let animation = ctx.prog8(start::ANIMATIONS + mode);
    ctx.anim_start(scene::ANIMATIONS, animation as u16);
}

/// Un objet de scène de moins pour l'objet `+0x46`.
fn start_countdown<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let porteur = pointeur(ctx, o, scene::OWNER_2);
    let n = porteur.u8(ctx.b, start::ICONES).wrapping_sub(1);
    porteur.set_u8(ctx.b, start::ICONES, n);
}

/// Sous-état 2 : marche en X ; sortie de l'écran, l'icône est libérée.
fn start_running<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    if o.u8(ctx.b, start::STOP) == 0 {
        ctx.move_x();
        let gap = o.u16(ctx.b, Obj::X).wrapping_sub(crate::ram::CAMERA_X.get(ctx.b)).wrapping_add(0x20);
        if gap > 0x1c0 {
            o.set_u32(ctx.b, Obj::STATE, 0x0600_0000);
            start_countdown(ctx, o);
        }
    }
    ctx.anim_step();
}

/// Sous-état 4 : arrêt demandé : son et animation d'arrêt, SET créé, l'icône se tourne vers le joueur.
fn start_stop<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    if o.u8(ctx.b, start::STOP) != 0 {
        ctx.anim_step();
        return;
    }
    let mode = o.u8(ctx.b, field::MODE) as u32;
    let sound = ctx.prog8(start::SOUNDS_STOP + mode);
    if sound != 0 {
        crate::sound::play(ctx.b, start::SOUND_STOP, ctx.obj());
    }
    let animation = ctx.prog8(start::ANIMATIONS_STOP + mode);
    ctx.anim_start(scene::ANIMATIONS, animation as u16);
    ajoute_u8(ctx, o, field::SUB_STATE, 2);
    start_countdown(ctx, o);
    o.set_u16(ctx.b, scene::CREATED, 0);
    if let Some(n) = created_set(ctx, 1) {
        o.set_u16(ctx.b, scene::CREATED, n.0 as u16);
    }
    let player = pointeur(ctx, o, field::OWNER);
    if o.u16(ctx.b, Obj::X) <= player.u16(ctx.b, Obj::X) {
        o.set_u8(ctx.b, 0x2c, 0x20);
    }
    if o.u8(ctx.b, 0x2c) == player.u8(ctx.b, 0x2c) {
        let v = player.u8(ctx.b, 0x2c) ^ 0x20;
        player.set_u8(ctx.b, 0x2c, v);
    }
    ctx.anim_step();
}

fn icon_start<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = super::effects::target(ctx, start::SUB_STATES, sub_state as u32);
    match sub_state {
        0 => start_launches(ctx, o),
        2 => start_running(ctx, o),
        4 => start_stop(ctx, o),
        6 => guide_wait(ctx, o),
        8 => guide_second_object(ctx, o),
        0xa => {
            let set = pointeur(ctx, o, scene::CREATED);
            if set.u8(ctx.b, 0x22) != 0 {
                ajoute_u8(ctx, o, field::SUB_STATE, 2);
            }
        }
        0xc => {
            let n = ctx.ram8(scene::COUNTER).wrapping_add(4);
            ctx.set_ram8(scene::COUNTER, n);
        }
        _ => crate::families::effects::outside_table(target),
    }
}

fn icone<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, scene::ICON_STATES, state as u32);
    match state {
        state::INIT => icone_init(ctx, o),
        state::ACTIVE => {
            let sub_state = o.u8(ctx.b, field::SUB_STATE);
            let target = super::effects::target(ctx, scene::ICON_SUB_STATES, sub_state as u32);
            match sub_state {
                sub::CHUTE => icone_suivi(ctx, o),
                2 => icone_envol(ctx, o),
                4 => icon_wait(ctx, o),
                _ => crate::families::effects::outside_table(target),
            }
        }
        4 => icon_start(ctx, o),
        6 => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

/// `0xdcddc` : place le guide au bord droit de l'écran et choisit le joueur vivant le plus proche,
/// dont il retient le propriétaire (`+0x50`), la hauteur (`+0x54`) et un point d'arrivée (`+0x52`).
/// Without a living player, the original keeps its caller's A2: `a2` (guide 3 leaves there the end of its palette copy;
/// guide 1 inherits it from the previous handlers, a value the register-free game does not hold: `None`).
fn guide_target<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, a2: Option<u32>) {
    let cam_x = crate::ram::CAMERA_X.get(ctx.b);
    let cam_y = crate::ram::CAMERA_Y.get(ctx.b);
    o.set_u16(ctx.b, Obj::X, cam_x.wrapping_add(0x1a0));
    o.set_u16(ctx.b, Obj::Y, cam_y.wrapping_add(0x40));
    o.set_u32(ctx.b, Obj::Z, 0);
    o.set_u8(ctx.b, 0x2c, 0);
    let mut best: u16 = 0xffff;
    let mut chosen = None;
    for i in 0..4u32 {
        let player = Obj(scene::PLAYERS + i * scene::STEP_PLAYER);
        let active = player.u8(ctx.b, Obj::ACTIVE) != 0;
        let vivant = active && {
            let state = player.u8(ctx.b, Obj::STATE) as i8;
            !(state >= 4)
        };
        if vivant {
            let mut dx = o.u16(ctx.b, Obj::X).wrapping_sub(player.u16(ctx.b, Obj::X));
            if !(dx as i16 >= 0) {
                dx = dx.wrapping_neg();
            }
            let mut dy = o.u16(ctx.b, Obj::Y).wrapping_sub(player.u16(ctx.b, Obj::Y));
            if !(dy as i16 >= 0) {
                dy = dy.wrapping_neg();
            }
            let distance = dx.wrapping_add(dy);
            if !(best <= distance) {
                best = distance;
                chosen = Some(player.0);
            }
        }
    }
    // Original dead code (docs/code-mort.md, familles3): guide 1 is created by the scene script, with players present
    let chosen = chosen.or(a2).unwrap_or_else(|| unreachable!("guide 1 without a living player: A2 inherited from the previous handlers"));
    o.set_u16(ctx.b, field::OWNER, chosen as u16);
    let target = Obj(chosen);
    let y = target.u16(ctx.b, Obj::Y);
    o.set_u16(ctx.b, scene::TARGET_Y, y);
    let gap = target.u16(ctx.b, Obj::X).wrapping_sub(cam_x);
    let offset: u16 = if gap <= 0x140 { 0x20 } else { 0x20u16.wrapping_neg() };
    let x = target.u16(ctx.b, Obj::X).wrapping_add(offset);
    o.set_u16(ctx.b, scene::TARGET_X, x);
}

fn guide_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ajoute_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, field::SUB_STATE, 0);
    guide_target(ctx, o, None);
    o.set_u32(ctx.b, Obj::SPEED_TABLE, scene::TABLE_SPEEDS);
    let level = ctx.ram8(scene::LEVEL) as u32;
    let animation = ctx.prog8(scene::GUIDE_ANIMATIONS + 2 * level);
    let mode = ctx.prog8(scene::GUIDE_ANIMATIONS + 2 * level + 1);
    o.set_u8(ctx.b, field::MODE, mode);
    ctx.anim_start(scene::ANIMATIONS, animation as u16);
}

/// `0xdcb9a` : animation du guide selon son mode (0x12 pour la variante 3).
fn guide_animation<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let mut animation = 0x12;
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if !(variant == 3) {
        let mode = o.u8(ctx.b, field::MODE) as u32;
        animation = ctx.prog8(scene::GUIDE_ANIMATIONS_ARRIVAL + mode);
    }
    ctx.anim_start(scene::ANIMATIONS, animation as u16);
}

/// Crée un objet SET de type `type_set` ; renvoie son adresse, ou `None` si le pool est plein.
fn created_set<R: Bench>(ctx: &mut Cx<'_, R>, type_set: u32) -> Option<Obj> {
    scene_set_of(ctx, type_set as u8)
}

/// Sous-état 0 du guide : marche vers le point d'arrivée ; arrivé, change d'animation et crée un SET.
fn guide_running<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let target_x = o.u16(ctx.b, scene::TARGET_X);
    let target_y = o.u16(ctx.b, scene::TARGET_Y);
    let direction = crate::aids::direction_from(ctx.b, o, (target_x, target_y));
    o.set_u8(ctx.b, Obj::SPEED_INDEX, direction);
    ctx.move_by_table_xy();
    let dx = target_x.wrapping_sub(o.u16(ctx.b, Obj::X)).wrapping_add(8);
    let dy = target_y.wrapping_sub(o.u16(ctx.b, Obj::Y)).wrapping_add(8);
    let arrive = !(dx > 0x10) && !(dy > 0x10);
    if arrive {
        ajoute_u8(ctx, o, field::SUB_STATE, 2);
        guide_animation(ctx, o);
        o.set_u16(ctx.b, scene::MINUTEUR, 0x3c);
        o.set_u16(ctx.b, scene::CREATED, 0);
        let variant = o.u8(ctx.b, Obj::VARIANT);
        let type_set = if variant != 3 { 1 } else { 0x26 };
        if let Some(n) = created_set(ctx, type_set) {
            o.set_u16(ctx.b, scene::CREATED, n.0 as u16);
        }
        let owner = pointeur(ctx, o, field::OWNER);
        let x = o.u16(ctx.b, Obj::X);
        let x_owner = owner.u16(ctx.b, Obj::X);
        if !(x > x_owner) {
            o.set_u8(ctx.b, 0x2c, 0x20);
        }
        let (a, b) = (o.u8(ctx.b, 0x2c), owner.u8(ctx.b, 0x2c));
        if !(a != b) {
            let v = owner.u8(ctx.b, 0x2c) ^ 0x20;
            owner.set_u8(ctx.b, 0x2c, v);
        }
    }
    ctx.anim_step();
}

/// Sous-état 2 du guide : attend la fin de la minuterie ou la disparition de l'objet SET créé.
fn guide_wait<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let created = o.u16(ctx.b, scene::CREATED);
    if !(created == 0) {
        let set = pointeur(ctx, o, scene::CREATED);
        let finished = set.u8(ctx.b, 0x22) != 0;
        if !finished {
            return;
        }
    } else {
        let remainder = o.u16(ctx.b, scene::MINUTEUR).wrapping_sub(1);
        o.set_u16(ctx.b, scene::MINUTEUR, remainder);
        if remainder != 0 {
            return;
        }
    }
    ajoute_u8(ctx, o, field::SUB_STATE, 2);
    let n = ctx.ram8(scene::COUNTER).wrapping_add(4);
    ctx.set_ram8(scene::COUNTER, n);
}

/// Sous-état 4 du guide : quand le compteur de scène atteint 0x10, crée un second SET.
fn guide_second_object<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let counter = ctx.ram8(scene::COUNTER);
    if counter != 0x10 {
        return;
    }
    o.set_u16(ctx.b, scene::CREATED, 0);
    if let Some(n) = created_set(ctx, 2) {
        ajoute_u8(ctx, o, field::SUB_STATE, 2);
        o.set_u16(ctx.b, scene::CREATED, n.0 as u16);
        let mode = o.u8(ctx.b, field::MODE);
        n.set_u8(ctx.b, 0x42, mode);
    }
}

/// Sous-état 6 du guide : attend la fin de l'objet SET, puis passe à l'état 4.
fn guide_end<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let set = pointeur(ctx, o, scene::CREATED);
    let in_progress = set.u8(ctx.b, 0x22) == 0;
    if !in_progress {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
}

fn guide_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let fige = ctx.ram8(scene::SCENE_FIGEE) != 0;
    if !fige {
        let sub_state = o.u8(ctx.b, field::SUB_STATE);
        let target = super::effects::target(ctx, scene::GUIDE_SUB_STATES, sub_state as u32);
        match sub_state {
            0 => guide_running(ctx, o),
            2 => guide_wait(ctx, o),
            4 => guide_second_object(ctx, o),
            6 => guide_end(ctx, o),
            _ => crate::families::effects::outside_table(target),
        }
    }
    ctx.reinsert();
}

fn guide<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, scene::GUIDE_STATES, state as u32);
    match state {
        state::INIT => guide_init(ctx, o),
        state::ACTIVE => guide_active(ctx, o),
        state::LIBERE => {
            let n = ctx.ram8(scene::COUNTER).wrapping_add(4);
            ctx.set_ram8(scene::COUNTER, n);
        }
        state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

/// Variante 3 (`0xdccd6`), le « guide » d'une scène d'attract (h) : charge une palette, puis marche comme
/// le guide de la variante 1 et attend la fin de son objet SET (type 0x26), dont il recopie le résultat
/// (`+0x23`) dans `A5-0x551c`.
mod guide_3 {
    pub const STATES: u32 = 0xdcce4;
    pub const SUB_STATES: u32 = 0xdcd76;
    /// Palette de 16 couleurs (données en clair) et sa destination.
    pub const PALETTE: u32 = 0xdccec;
    pub const PALETTE_DEST: u32 = 0x91c300;
    /// Première couleur de la palette 0 : ses bits de luminosité (0xf000f000) sont recopiés.
    pub const REFERENCE: u32 = 0x91c000;
    pub const ANIMATION: u16 = 0x13;
    pub const RESULT: i32 = -0x551c;
}

fn guide_3_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let lumiere = crate::mem::rd32(ctx.b, guide_3::REFERENCE) & 0xf000_f000;
    for i in 0..8 {
        let colour = crate::mem::rd32(ctx.b, guide_3::PALETTE + 4 * i);
        // Original glitch: the original's `movem` passes colours 1 to 6 through D1-D6; D2-D5 stay for the next
        // handlers (`dispatch::Leftovers`)
        match i {
            2 => *ctx.c.d2() = colour,
            3 => *ctx.c.d3() = colour,
            4 => *ctx.c.d4() = colour,
            5 => *ctx.c.d5() = colour,
            _ => {}
        }
        crate::mem::wr32(ctx.b, guide_3::PALETTE_DEST + 4 * i, colour | lumiere);
    }
    ajoute_u8(ctx, o, Obj::STATE, 2);
    // A2: the end of the palette copy
    guide_target(ctx, o, Some(guide_3::PALETTE_DEST + 0x20));
    o.set_u32(ctx.b, Obj::SPEED_TABLE, scene::TABLE_SPEEDS);
    o.set_u8(ctx.b, 0x2a, 1);
    o.set_u8(ctx.b, 0x2b, 0x18);
    ctx.anim_start(scene::ANIMATIONS, guide_3::ANIMATION);
    ctx.reinsert();
}

/// Sous-état 2 : attend la fin de l'objet SET (dont il garde le résultat) ou de la minuterie.
fn guide_3_wait<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    if o.u16(ctx.b, scene::CREATED) != 0 {
        let set = pointeur(ctx, o, scene::CREATED);
        if set.u8(ctx.b, 0x22) == 0 {
            return;
        }
        let result = set.u8(ctx.b, 0x23);
        ctx.set_ram8(guide_3::RESULT, result);
    } else {
        let remainder = o.u16(ctx.b, scene::MINUTEUR).wrapping_sub(1);
        o.set_u16(ctx.b, scene::MINUTEUR, remainder);
        if remainder != 0 {
            return;
        }
    }
    ajoute_u8(ctx, o, field::SUB_STATE, 2);
    let n = ctx.ram8(scene::COUNTER).wrapping_add(4);
    ctx.set_ram8(scene::COUNTER, n);
}

fn guide_3<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, guide_3::STATES, state as u32);
    match state {
        state::INIT => guide_3_init(ctx, o),
        state::ACTIVE => {
            let sub_state = o.u8(ctx.b, field::SUB_STATE);
            let target = super::effects::target(ctx, guide_3::SUB_STATES, sub_state as u32);
            match sub_state {
                0 => guide_running(ctx, o),
                2 => guide_3_wait(ctx, o),
                4 => {}
                _ => crate::families::effects::outside_table(target),
            }
        }
        state::LIBERE => {}
        state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

fn scene_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let target = super::effects::target(ctx, scene::TABLE_VARIANTS, 2 * variant as u32);
    match variant {
        0 | 2 => icone(ctx, o),
        1 => guide(ctx, o),
        3 => guide_3(ctx, o),
        _ => crate::families::effects::outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 7 : réserve du joueur (0xdce6e)

/// Type 7, objets qui alimentent une réserve du joueur (compteurs `+0x150 + variant`, plafonnés à 9)
/// pour les personnages autorisés (table par personnage, puis par classe). Même cycle que le type 4 :
/// chute, attente (0x4b0 images, clignotement), ramassage (refusé : rejet en arc), mode 1 : objet jeté.
mod reserve {
    pub const ENTRY: u32 = 0xdce6e;
    pub const TABLE_STATES: u32 = 0xdce84;
    pub const TABLE_SUB_STATES: u32 = 0xdcf38;
    pub const ANIMATIONS: u32 = 0x1c001e;
    pub const DURATION_TO_GROUND: u16 = 0x4b0;
    /// Par personnage (espace programme) : l'objet est-il utilisable ?
    pub const CHARACTERS: u32 = 0xdd01e;
    /// Par classe (données) : réserve acceptée ?
    pub const CLASSES: u32 = 0xdd122;
    pub const COUNTERS: u32 = 0x150;
}

fn reserve_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let mode = o.u8(ctx.b, field::MODE);
    if mode != 0 {
        let delay = o.u8(ctx.b, inventaire::DELAY);
        if !(delay == 0) {
            o.set_u8(ctx.b, inventaire::DELAY, delay - 1);
            return;
        }
        ajoute_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        return animate_and_sort(ctx, reserve::ANIMATIONS, variant.wrapping_add(0xa));
    }
    if airborne(ctx, o) {
        state_next(ctx, o, sub::CHUTE);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
    } else {
        state_next(ctx, o, sub::WAIT);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        o.set_u16(ctx.b, field::DURATION, reserve::DURATION_TO_GROUND);
    }
    animate_and_sort(ctx, reserve::ANIMATIONS, variant);
}

/// Sous-état d'attente : le joueur qui touche l'objet (`RAMASSE`) le fait passer au ramassage.
fn reserve_wait<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let ramasse = o.u8(ctx.b, field::RAMASSE);
    if !(ramasse == 0) {
        ajoute_u8(ctx, o, field::SUB_STATE, 2);
    }
    expire_and_blinks(ctx, o);
}

/// `0xdd0da` : ajoute l'objet à la réserve du propriétaire. Vrai si le ramassage est refusé.
fn reserve_ajoute<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let player = pointeur(ctx, o, field::OWNER);
    let class = (player.u8(ctx.b, 3) >> 1) as u32;
    let accepte = ctx.data8(reserve::CLASSES + class);
    if !(accepte != 0) {
        o.set_u8(ctx.b, field::SUB_STATE, 6);
        o.set_u8(ctx.b, field::SUB_SUB_STATE, 0);
        o.set_u8(ctx.b, inventaire::DELAY, 0xc);
        return true;
    }
    let variant = o.u8(ctx.b, Obj::VARIANT) as u32;
    let counter = player.0 + reserve::COUNTERS + variant;
    let n = crate::mem::rd8(ctx.b, counter).wrapping_add(1);
    crate::mem::wr8(ctx.b, counter, n);
    if !(n <= 9) {
        crate::mem::wr8(ctx.b, counter, 9);
    }
    false
}

fn reserve_ramasse<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let player = pointeur(ctx, o, field::OWNER);
    let ramasse = o.u8(ctx.b, field::RAMASSE);
    if !(ramasse == 2) {
        let ready = player.u8(ctx.b, 0x33) != 0;
        if !ready {
            return;
        }
    }
    let character = player.u8(ctx.b, 3) as u32;
    let allowed = ctx.prog8(reserve::CHARACTERS + character);
    if !(allowed != 0) {
        ajoute_u8(ctx, o, field::SUB_STATE, 0xfe);
        o.set_u8(ctx.b, field::RAMASSE, 0);
        return;
    }
    let refuse = reserve_ajoute(ctx, o);
    if !refuse {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
}

fn reserve_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = super::effects::target(ctx, reserve::TABLE_SUB_STATES, sub_state as u32);
    match sub_state {
        sub::CHUTE => {
            let pose = o.u8(ctx.b, field::SUB_SUB_STATE);
            if !(pose != 0) {
                let in_flight = ctx.move_xyz().gt();
                if !in_flight {
                    ajoute_u8(ctx, o, field::SUB_SUB_STATE, 2);
                    o.set_u16(ctx.b, field::DURATION, reserve::DURATION_TO_GROUND);
                    o.set_u32(ctx.b, Obj::Z, 0);
                    let mode = o.u8(ctx.b, field::MODE);
                    if !(mode != 0) {
                        in_view(ctx, o);
                    }
                }
                return fall_end(ctx);
            }
            let mode = o.u8(ctx.b, field::MODE);
            if !(mode == 0) {
                ajoute_u8(ctx, o, field::SUB_STATE, 2);
                return reserve_wait(ctx, o);
            }
            let end = ctx.anim_step();
            if end {
                ajoute_u8(ctx, o, field::SUB_STATE, 2);
            }
        }
        sub::WAIT => reserve_wait(ctx, o),
        sub::RAMASSE => reserve_ramasse(ctx, o),
        6 => inventaire_rejet(ctx, o),
        _ => crate::families::effects::outside_table(target),
    }
}

fn reserve_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, reserve::TABLE_STATES, state as u32);
    match state {
        state::INIT => reserve_init(ctx, o),
        state::ACTIVE => reserve_active(ctx, o),
        state::LIBERE | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 9 : icône qui accompagne un objet (0xdd5a2)

/// Type 9 : étincelle d'interface qui suit un objet (`+0x5c`), un pixel devant lui, pendant `+0x14`
/// images (le compte est préparé par le créateur), puis disparaît. Palette selon la variante.
mod accompagnement {
    pub const ENTRY: u32 = 0xdd5a2;
    pub const PALETTES: u32 = 0xdd5de;
    pub const ANIMATIONS: u32 = 0x1c03a6;
    pub const TARGET: u32 = 0x5c;
    pub const DURATION: u32 = 0x14;
}

fn escort_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    if !(state != 0) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, 0x2a, 1);
        let variant = o.u8(ctx.b, Obj::VARIANT);
        let palette = ctx.prog8(accompagnement::PALETTES + variant as u32);
        o.set_u8(ctx.b, 0x2b, palette);
        o.set_u16(ctx.b, accompagnement::DURATION, 0x100);
        ctx.anim_start(accompagnement::ANIMATIONS, 0x8e + variant as u16);
        return ctx.reinsert();
    }
    let target = pointeur(ctx, o, accompagnement::TARGET);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = target.u16(ctx.b, off);
        o.set_u16(ctx.b, off, v);
    }
    let y = o.u16(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u16(ctx.b, Obj::Y, y);
    ctx.anim_step();
    let remainder = o.u16(ctx.b, accompagnement::DURATION).wrapping_sub(1);
    o.set_u16(ctx.b, accompagnement::DURATION, remainder);
    if remainder as i16 >= 0 {
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
    pub const ENTRY: u32 = 0xdd132;
    pub const TABLE_VARIANTS: u32 = 0xdd142;
    pub const ANIMATIONS: u32 = 0x1bf8fe;
    // variante 0
    pub const V0_STATES: u32 = 0xdd156;
    pub const V0_SUB_STATES: u32 = 0xdd190;
    pub const V0_ANIMATIONS: u32 = 0xdd180;
    pub const V0_ANIMATIONS_KEY: u32 = 0xdd1d2;
    pub const V0_ANIMATIONS_SET: u32 = 0xdd22a;
    pub const V0_ANIMATIONS_END: u32 = 0xdd246;
    // variante 1
    pub const V1_STATES: u32 = 0xdd280;
    pub const V1_SUB_STATES: u32 = 0xdd2d4;
    // variante 2
    pub const V2_STATES: u32 = 0xdd3ca;
    pub const V2_SUB_STATES: u32 = 0xdd400;
    /// Verrou : un joueur est en train de ramasser (A5-0x5ac4) et l'événement posé en `A5+0x5e`.
    pub const BUSY: i32 = -0x5ac4;
    pub const SET_CREATED: u32 = 0x42;
    pub const DELAY: u32 = 0x4c;
    /// Boîtes de contact : une par type (`+0x39`) pour l'objet, une par personnage et par `+0x38` pour le joueur.
    pub const BOXES_OBJECT: u32 = 0xdd590;
    pub const BOXES_PLAYER: u32 = 0xdd578;
    pub const PLAYERS: u32 = 0xff85de;
}

/// `0xdd470` : un joueur touche-t-il l'objet ? Si oui, le sous-état avance, le verrou se lève et le
/// joueur devient le propriétaire.
fn contact_player<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let busy = ctx.ram8(contact::BUSY) != 0;
    if busy {
        return false;
    }
    let hitbox = o.u8(ctx.b, 0x39) as u32;
    if hitbox == 0 {
        return false;
    }
    let my_box = contact::BOXES_OBJECT + (hitbox << 3);
    for i in 0..4u32 {
        let player = Obj(contact::PLAYERS + i * 0x200);
        if player_key(ctx, o, player, my_box) {
            ajoute_u8(ctx, o, field::SUB_STATE, 2);
            ctx.set_ram8(contact::BUSY, 1);
            o.set_u16(ctx.b, field::OWNER, player.0 as u16);
            return true;
        }
    }
    false
}

/// Le joueur est-il présent, en vie, libre, et sa boîte recouvre-t-elle celle de l'objet ?
fn player_key<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, player: Obj, my_box: u32) -> bool {
    let absent = player.u8(ctx.b, Obj::ACTIVE) != 1;
    if absent {
        return false;
    }
    let dead = player.u8(ctx.b, Obj::STATE) as i8 >= 4;
    if dead {
        return false;
    }
    let busy = player.u8(ctx.b, 0x182) != 0;
    if busy {
        return false;
    }
    let number = player.u8(ctx.b, 0x38) as u32;
    if number == 0 {
        return false;
    }
    let class = (player.u8(ctx.b, 3) >> 1) as u32;
    let table = ctx.data32(contact::BOXES_PLAYER + (class << 2));
    let its_box = table + (number << 3);
    chevauche(ctx, o, player, my_box, its_box)
}

/// Les trois recouvrements de boîtes (X, Y puis Z) entre l'objet `o` (boîte `a`) et le joueur (boîte `b`).
fn chevauche<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, player: Obj, a: u32, b: u32) -> bool {
    let byte = |ctx: &mut Cx<'_, R>, address: u32| ctx.data8(address) as i8 as i16 as u16;
    // X : les décalages s'inversent avec l'orientation de chacun
    let mut da = byte(ctx, a);
    let retourne = o.u8(ctx.b, field::FLIP) != 0;
    if retourne {
        da = da.wrapping_neg();
    }
    let mut db = byte(ctx, b);
    let player_turned = player.u8(ctx.b, field::FLIP) != 0;
    if player_turned {
        db = db.wrapping_neg();
    }
    let gap = da.wrapping_add(o.u16(ctx.b, Obj::X)).wrapping_sub(db).wrapping_sub(player.u16(ctx.b, Obj::X));
    let half = ctx.data8(a + 1).wrapping_add(ctx.data8(b + 1)) as u16;
    if gap.wrapping_add(half) > half.wrapping_mul(2) {
        return false;
    }
    // Y puis Z : décalages fixes
    for (offset, size, position) in [(4u32, 5u32, Obj::Y), (2, 3, Obj::Z)] {
        let da = byte(ctx, a + offset);
        let db = byte(ctx, b + offset);
        let gap = da.wrapping_add(o.u16(ctx.b, position)).wrapping_sub(db.wrapping_add(player.u16(ctx.b, position)));
        let half = ctx.data8(a + size).wrapping_add(ctx.data8(b + size)) as u16;
        if gap.wrapping_add(half) > half.wrapping_mul(2) {
            return false;
        }
    }
    true
}

/// `0xdd16e` : démarre l'animation `animation`, puis trie l'objet.
fn contact_anime<R: Bench>(ctx: &mut Cx<'_, R>, animation: u8) {
    ctx.anim_start(contact::ANIMATIONS, animation as u16);
    ctx.reinsert();
}

/// Animation tirée d'une table indexée par le mode (`+0x2f`).
fn contact_anime_mode<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, table: u32) {
    let mode = o.u8(ctx.b, field::MODE) as u32;
    let animation = ctx.prog8(table + mode);
    contact_anime(ctx, animation);
}

/// Crée l'objet SET de la scène de ramassage (type `+0x42`, mot) et le rattache à l'objet. Faux si le
/// type est négatif (pas de scène) ou si le pool est plein.
fn contact_created_set<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let word = o.u16(ctx.b, contact::SET_CREATED);
    if word as i16 <= -1 {
        return false;
    }
    let Some(new) = scene_set_of(ctx, word as u8) else {
        return false;
    };
    o.set_u16(ctx.b, contact::SET_CREATED, new.0 as u16);
    let owner = pointeur(ctx, o, field::OWNER);
    let character = owner.u8(ctx.b, 2);
    new.set_u8(ctx.b, 0x37, character);
    true
}

/// Oriente l'objet vers le côté où se trouve le propriétaire (retenue de `x_owner - x`).
fn contact_oriente<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let owner = pointeur(ctx, o, field::OWNER);
    let (x_owner, x) = (owner.u16(ctx.b, Obj::X), o.u16(ctx.b, Obj::X));
    let sens = if x_owner < x { 0 } else { 0x20 };
    o.set_u8(ctx.b, field::FLIP, sens);
}

/// L'objet SET a-t-il fini ? (`+0x22` de l'objet SET non nul.)
fn contact_set_finished<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let set = pointeur(ctx, o, contact::SET_CREATED);
    let finished = set.u8(ctx.b, 0x22) != 0;
    finished
}

/// Variante 0, fin de la scène (`0xdd22e`) : lève le verrou et passe au sous-état de fuite.
fn contact_0_end<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ctx.set_ram8(contact::BUSY, 0);
    o.set_u8(ctx.b, field::SUB_STATE, 6);
    contact_anime_mode(ctx, o, contact::V0_ANIMATIONS_END);
}

fn shifted_x<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, step: u16) {
    let x = o.u16(ctx.b, Obj::X).wrapping_add(step);
    o.set_u16(ctx.b, Obj::X, x);
}

fn contact_0<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, contact::V0_STATES, state as u32);
    match state {
        state::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            let mode = o.u8(ctx.b, field::MODE) as u32;
            let animation = ctx.prog8(contact::V0_ANIMATIONS + mode);
            o.set_u8(ctx.b, field::SUB_STATE, 0);
            contact_anime(ctx, animation);
        }
        state::ACTIVE => {
            let sub_state = o.u8(ctx.b, field::SUB_STATE);
            let target = super::effects::target(ctx, contact::V0_SUB_STATES, sub_state as u32);
            match sub_state {
                0 => {
                    ctx.anim_step();
                    let ramasse = o.u8(ctx.b, field::RAMASSE);
                    if ramasse == 0 {
                        return;
                    }
                    let busy = ctx.ram8(contact::BUSY) != 0;
                    if busy {
                        o.set_u8(ctx.b, field::RAMASSE, 0);
                        return;
                    }
                    ctx.set_ram8(contact::BUSY, 1);
                    ajoute_u8(ctx, o, field::SUB_STATE, 2);
                    eclat(ctx, o);
                    contact_anime_mode(ctx, o, contact::V0_ANIMATIONS_KEY);
                }
                2 => {
                    ctx.anim_step();
                    let key = o.u8(ctx.b, 0x33);
                    if key == 1 {
                        return;
                    }
                    ajoute_u8(ctx, o, field::SUB_STATE, 2);
                    if !contact_created_set(ctx, o) {
                        return contact_0_end(ctx, o);
                    }
                    contact_oriente(ctx, o);
                    contact_anime_mode(ctx, o, contact::V0_ANIMATIONS_SET);
                }
                4 => {
                    if contact_set_finished(ctx, o) {
                        contact_0_end(ctx, o);
                    }
                }
                6 => {
                    let flip = o.u8(ctx.b, field::FLIP) != 0;
                    let step: u16 = if flip { 3 } else { 3u16.wrapping_neg() };
                    shifted_x(ctx, o, step);
                    ctx.anim_step();
                    ctx.cull_offscreen();
                }
                _ => crate::families::effects::outside_table(target),
            }
        }
        state::LIBERE | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

/// Variante 1 : animation de balancement (`0xdd2a8`), tri et mise au point des bords.
fn contact_1_anime<R: Bench>(ctx: &mut Cx<'_, R>) {
    contact_anime(ctx, 0x18);
    ctx.keep_in_zone();
    ctx.push_terrain();
}

/// Variante 1 : déplacement horizontal `X += VX << 8`.
fn contact_1_advance<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let vx = o.u16(ctx.b, Obj::VX) as i16 as i32;
    let x = o.u32(ctx.b, Obj::X).wrapping_add((vx << 8) as u32);
    o.set_u32(ctx.b, Obj::X, x);
}

/// Variante 1 : vitesse 0x200 dans le sens de l'orientation (`0xdd296`).
fn contact_1_speed<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    o.set_u16(ctx.b, Obj::VX, 0x200);
    let flip = o.u8(ctx.b, field::FLIP) != 0;
    if !flip {
        let v = o.u16(ctx.b, Obj::VX).wrapping_neg();
        o.set_u16(ctx.b, Obj::VX, v);
    }
}

/// Variante 1, sortie de scène (`0xdd37c`) : le verrou se lève, le bonus s'éloigne.
fn contact_1_exit<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    contact_1_speed(ctx, o);
    ctx.set_ram8(contact::BUSY, 0);
    o.set_u8(ctx.b, field::SUB_STATE, 6);
    contact_1_anime(ctx);
}

fn contact_1<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, contact::V1_STATES, state as u32);
    match state {
        state::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, field::SUB_STATE, 0);
            o.set_u8(ctx.b, contact::DELAY, 0x3c);
            contact_1_speed(ctx, o);
            contact_1_anime(ctx);
        }
        state::ACTIVE => {
            let sub_state = o.u8(ctx.b, field::SUB_STATE);
            let target = super::effects::target(ctx, contact::V1_SUB_STATES, sub_state as u32);
            match sub_state {
                0 => {
                    let key = contact_player(ctx, o);
                    if key {
                        // pas de contact suivant : les sous-états avancent d'eux-mêmes
                        ctx.set_ram8(0x5e, 1);
                        ctx.anim_start(contact::ANIMATIONS, 0x19);
                        return;
                    }
                    contact_1_advance(ctx, o);
                    let remainder = o.u8(ctx.b, contact::DELAY).wrapping_sub(1);
                    o.set_u8(ctx.b, contact::DELAY, remainder);
                    if !(remainder != 0) {
                        o.set_u8(ctx.b, contact::DELAY, 0x3c);
                        let flip = o.u8(ctx.b, field::FLIP) ^ 0x20;
                        o.set_u8(ctx.b, field::FLIP, flip);
                        let v = o.u16(ctx.b, Obj::VX).wrapping_neg();
                        o.set_u16(ctx.b, Obj::VX, v);
                    }
                    ctx.anim_step();
                    ctx.reinsert();
                    ctx.keep_in_zone();
                    ctx.push_terrain();
                }
                2 => {
                    ajoute_u8(ctx, o, field::SUB_STATE, 2);
                    if !contact_created_set(ctx, o) {
                        return contact_1_exit(ctx, o);
                    }
                    contact_oriente(ctx, o);
                }
                4 => {
                    let set = pointeur(ctx, o, contact::SET_CREATED);
                    let in_progress = set.u8(ctx.b, 0x22) == 0;
                    if !in_progress {
                        contact_1_exit(ctx, o);
                    }
                }
                6 => {
                    contact_1_advance(ctx, o);
                    ctx.anim_step();
                    ctx.reinsert();
                    ctx.cull_offscreen();
                }
                _ => crate::families::effects::outside_table(target),
            }
        }
        state::LIBERE | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

/// Variante 2, fin de la scène (`0xdd44e`).
fn contact_2_end<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ctx.set_ram8(contact::BUSY, 0);
    ajoute_u8(ctx, o, field::SUB_STATE, 2);
}

fn contact_2<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, contact::V2_STATES, state as u32);
    match state {
        state::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, field::SUB_STATE, 0);
            let mode = o.u8(ctx.b, field::MODE);
            contact_anime(ctx, mode.wrapping_add(0x14));
        }
        state::ACTIVE => {
            let sub_state = o.u8(ctx.b, field::SUB_STATE);
            let target = super::effects::target(ctx, contact::V2_SUB_STATES, sub_state as u32);
            match sub_state {
                0 => {
                    ctx.anim_step();
                    let ramasse = o.u8(ctx.b, field::RAMASSE);
                    if ramasse == 0 {
                        return;
                    }
                    let busy = ctx.ram8(contact::BUSY) != 0;
                    if busy {
                        o.set_u8(ctx.b, field::RAMASSE, 0);
                        return;
                    }
                    ctx.set_ram8(contact::BUSY, 1);
                    ajoute_u8(ctx, o, field::SUB_STATE, 2);
                    if !contact_created_set(ctx, o) {
                        contact_2_end(ctx, o);
                    }
                }
                2 => {
                    if contact_set_finished(ctx, o) {
                        contact_2_end(ctx, o);
                    }
                }
                4 => {
                    ctx.anim_step();
                }
                _ => crate::families::effects::outside_table(target),
            }
        }
        state::LIBERE | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

fn contact_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let target = super::effects::target(ctx, contact::TABLE_VARIANTS, 2 * variant as u32);
    match variant {
        0 => contact_0(ctx, o),
        1 => contact_1(ctx, o),
        2 => contact_2(ctx, o),
        _ => crate::families::effects::outside_table(target),
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
    pub const ENTRY: u32 = 0xdb7de;
    pub const STATES: u32 = 0xdb7f4;
    pub const SUB_STATES: u32 = 0xdb88c;
    pub const ANIMATIONS: u32 = 0x1bf156;
    /// Indice de ramassage par variante.
    pub const INDICES: u32 = 0xdbdcc;
    /// Fiches (données) ; octets 0 à 5 : classes autorisées, 6 : identifiant, 7 : groupe.
    pub const SHEETS: u32 = 0xdbc8c;
    /// Par classe, 8 identifiants que le premier emplacement ne peut pas remplacer.
    pub const VERROUILLES: u32 = 0xdbc5c;
    pub const FIRST_EQUIPPED: u32 = 0x10d;
    pub const SECOND_EQUIPE: u32 = 0x10e;
}

/// `0xdba96` : bonus temporaire donné par les variantes 0x24 à 0x27.
fn equipement_bonus<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let player = pointeur(ctx, o, field::OWNER);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if !(variant != 0x24) {
        return player.set_u8(ctx.b, 0x18c, 0x14);
    }
    if !(variant != 0x25) {
        return player.set_u8(ctx.b, 0x18b, 0x14);
    }
    if !(variant != 0x26) {
        player.set_u8(ctx.b, 0x18d, 0x14);
    }
    if !(variant != 0x27) {
        player.set_u8(ctx.b, 0x191, 0x14);
    }
}

/// `0xdbb0e` : ramassage réussi : bonus, événement de score et son. Renvoie faux (accepté).
fn equipement_accepte<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, player: Obj) -> bool {
    equipement_bonus(ctx, o);
    let event = 0x11 + ((player.u8(ctx.b, 2) as u16) << 8);
    crate::events::enqueue_points(ctx.b, event);
    effect_pickup(ctx, ctx.obj(), 5);
    false
}

/// `0xdbafa` : ramassage refusé : l'objet sera rejeté.
fn equipement_refuse<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    o.set_u8(ctx.b, field::SUB_STATE, 6);
    o.set_u8(ctx.b, field::SUB_SUB_STATE, 0);
    o.set_u8(ctx.b, inventaire::DELAY, 0xc);
    true
}

/// Groupe d'un équipement (octet 7 de sa fiche).
fn equipment_group<R: Bench>(ctx: &mut Cx<'_, R>, identifiant: u8) -> u8 {
    let rank = ((identifiant.wrapping_sub(0x2e) as u32) << 3) & 0xffff;
    ctx.data8(equipement::SHEETS + 7 + rank)
}

/// `0xdbad8` : range l'équipement dans l'inventaire du propriétaire. Renvoie vrai si refusé.
fn equipement_ajoute<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let player = pointeur(ctx, o, field::OWNER);
    let line = ((o.u8(ctx.b, Obj::VARIANT) as u32) << 3) & 0xffff;
    let class = (player.u8(ctx.b, 3) >> 1) as u32;
    let allowed = ctx.data8(equipement::SHEETS + line + class);
    if !(allowed != 0) {
        return equipement_refuse(ctx, o);
    }
    let mut id = ctx.data8(equipement::SHEETS + 6 + line);
    // déjà équipé ?
    for off in [equipement::FIRST_EQUIPPED, equipement::SECOND_EQUIPE] {
        let equipe = player.u8(ctx.b, off).wrapping_add(0x2e);
        if equipe == id {
            return equipement_accepte(ctx, o, player);
        }
    }
    // l'identifiant est-il déjà dans l'inventaire ?
    for i in 0..6u32 {
        let slot = player.u8(ctx.b, inventaire::IDENTIFIANTS + i);
        let empty = slot == inventaire::EMPTY;
        if !empty {
            let object = slot <= 0x2d;
            if object {
                // objet d'inventaire : seulement s'il est identique
                if slot == id {
                    return equipement_accepte(ctx, o, player);
                }
            } else {
                let (g_slot, g_new) = (equipment_group(ctx, slot), equipment_group(ctx, id));
                if !(g_new != g_slot) {
                    // même groupe : on garde le meilleur
                    if !(id as i8 > slot as i8) {
                        id = slot;
                    }
                    player.set_u8(ctx.b, inventaire::IDENTIFIANTS + i, id);
                    return equipement_accepte(ctx, o, player);
                }
            }
        }
    }
    // équipement porté en main (octet 0x10d, ou 0x10e pour la classe 4)
    let mut door = player.u8(ctx.b, equipement::FIRST_EQUIPPED);
    let class_main = player.u8(ctx.b, 3) >> 1;
    if !(class_main != 4) {
        door = player.u8(ctx.b, equipement::SECOND_EQUIPE);
    }
    let group_door = ctx.data8(equipement::SHEETS + 7 + ((door as u32) << 3));
    let group_new = equipment_group(ctx, id);
    if !(group_new != group_door) {
        let new = id.wrapping_sub(0x2e);
        if !(door as i8 > new as i8) {
            player.set_u8(ctx.b, equipement::FIRST_EQUIPPED, new);
        }
        return equipement_accepte(ctx, o, player);
    }
    // un emplacement libre
    for i in 0..6u32 {
        let slot = player.u8(ctx.b, inventaire::IDENTIFIANTS + i);
        if !(slot != inventaire::EMPTY) {
            player.set_u8(ctx.b, inventaire::IDENTIFIANTS + i, id);
            equipement_bonus(ctx, o);
            return false;
        }
    }
    // échange avec le premier emplacement, sauf s'il est verrouillé pour cette classe
    let first = player.u8(ctx.b, inventaire::IDENTIFIANTS);
    for k in 0..8u32 {
        let lock = ctx.data8(equipement::VERROUILLES + (class << 3) + k);
        if first == lock {
            return equipement_refuse(ctx, o);
        }
    }
    let old_quantity = player.u8(ctx.b, inventaire::QUANTITES);
    // D0.w : la classe × 8 (`lsr.w #1` puis `lsl.w #3` de l'octet +3) garde son octet haut sous l'identifiant ; il
    // n'est non nul que si le propriétaire n'est pas un joueur (pointeur nul : octets de la ROM)
    let index = ((class << 3) as u16 & 0xff00) | first as u16;
    object_ejected(ctx, o, player, index, old_quantity);
    player.set_u8(ctx.b, inventaire::IDENTIFIANTS, id);
    player.set_u8(ctx.b, inventaire::QUANTITES, 0);
    equipement_bonus(ctx, o);
    false
}

fn equipement_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let mode = o.u8(ctx.b, field::MODE);
    if mode != 0 {
        let delay = o.u8(ctx.b, inventaire::DELAY);
        if !(delay == 0) {
            o.set_u8(ctx.b, inventaire::DELAY, delay - 1);
            return;
        }
        ajoute_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
    } else if airborne(ctx, o) {
        state_next(ctx, o, sub::CHUTE);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
    } else {
        state_next(ctx, o, sub::WAIT);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        o.set_u16(ctx.b, field::DURATION, 0x4b0);
    }
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let index = ctx.data8(equipement::INDICES + variant as u32);
    o.set_u8(ctx.b, field::INDEX, index);
    animate_and_sort(ctx, equipement::ANIMATIONS, variant);
}

/// Sous-état 6 : rejet en arc de l'équipement refusé ; à l'atterrissage, une variante 0x0f ramassée
/// par la classe 1 plus de sept fois déclenche une scène.
fn equipement_rejet<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let launches = o.u8(ctx.b, field::SUB_SUB_STATE) != 0;
    if !launches {
        let delay = o.u8(ctx.b, inventaire::DELAY).wrapping_sub(1);
        o.set_u8(ctx.b, inventaire::DELAY, delay);
        if delay != 0 {
            return;
        }
        ajoute_u8(ctx, o, field::SUB_SUB_STATE, 2);
        let player = pointeur(ctx, o, field::OWNER);
        let y = player.u16(ctx.b, Obj::Y).wrapping_sub(1);
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
    o.set_u8(ctx.b, field::SUB_STATE, sub::WAIT);
    o.set_u8(ctx.b, field::SUB_SUB_STATE, 0);
    o.set_u32(ctx.b, Obj::Z, 0);
    o.set_u8(ctx.b, field::RAMASSE, 0);
    let player = pointeur(ctx, o, field::OWNER);
    let class = player.u8(ctx.b, 3) >> 1;
    if class != 1 {
        return;
    }
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if variant != 0xf {
        return;
    }
    let n = ctx.ram8(0x57).wrapping_add(1);
    ctx.set_ram8(0x57, n);
    if n <= 7 {
        return;
    }
    o.set_u16(ctx.b, field::DURATION, 0x4b0);
    o.set_u8(ctx.b, Obj::FLAGS, 1);
    o.set_u8(ctx.b, Obj::VARIANT, 0xc);
    ctx.anim_start(equipement::ANIMATIONS, 0xc);
    effects_creators::posed_flash(ctx.b, o);
    ctx.set_ram8(-0x5b83, 1);
    scene_set_of(ctx, 0x29);
}

fn equipment_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = super::effects::target(ctx, equipement::SUB_STATES, sub_state as u32);
    match sub_state {
        sub::CHUTE => {
            let in_flight = ctx.move_xyz().gt();
            if !in_flight {
                ajoute_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u16(ctx.b, field::DURATION, 0x4b0);
                o.set_u32(ctx.b, Obj::Z, 0);
                let mode = o.u8(ctx.b, field::MODE);
                if !(mode != 0) {
                    in_view(ctx, o);
                }
            }
            fall_end(ctx);
        }
        sub::WAIT => reserve_wait(ctx, o),
        sub::RAMASSE => {
            let player = pointeur(ctx, o, field::OWNER);
            let ready = player.u8(ctx.b, 0x33) != 0;
            if !ready {
                return;
            }
            let refuse = equipement_ajoute(ctx, o);
            if !refuse {
                ajoute_u8(ctx, o, Obj::STATE, 2);
            }
            mark_pickup(ctx, o);
        }
        6 => equipement_rejet(ctx, o),
        _ => crate::families::effects::outside_table(target),
    }
}

fn equipment_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, equipement::STATES, state as u32);
    match state {
        state::INIT => equipement_init(ctx, o),
        state::ACTIVE => equipment_active(ctx, o),
        state::LIBERE | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

handler!(ITEM_3, equipement::ENTRY, "item_equipement", #[without] equipment_handler);
handler!(ITEM_0, tresor::ENTRY, "item_tresor", #[without] tresor);
handler!(ITEM_1, conso::ENTRY, "item_conso", #[without] consumable_handler);
handler!(ITEM_4, inventaire::ENTRY, "item_inventaire", #[without] inventory_handler);
handler!(ITEM_5, bonus::ENTRY, "item_bonus", #[without] bonus_handler);
handler!(ITEM_6, scene::ENTRY, "item_scene", #[without] scene_handler);
handler!(ITEM_7, reserve::ENTRY, "item_reserve", #[without] reserve_handler);
handler!(ITEM_8, contact::ENTRY, "item_contact", #[without] contact_handler);
handler!(ITEM_9, accompagnement::ENTRY, "item_accompagnement", #[without] escort_handler);

// ---------------------------------------------------------------------------------------------
// Sous-routines appelées hors des gestionnaires

/// `0xdabba` : efface le tableau des objets de collection ramassés (48 longs en `A5 - 0x5b88`).
pub fn clear_picked_up(b: &mut Cps2Bus) {
    let start = A5.wrapping_add(PICKED as u32);
    for k in 0..48 {
        wr32(b, start + 4 * k, 0);
    }
}

/// `clear_picked_up` for a caller still on the processor (`game::leave`), registers left like the original: D0 = 0,
/// D6.w = 0xffff, A0 = end of the array.
pub fn clear_picked_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    clear_picked_up(b);
    c.d[0] = 0;
    c.d[6] = c.d[6] & 0xffff_0000 | 0xffff;
    c.a[0] = A5.wrapping_add(PICKED as u32) + 4 * 48;
}

/// Un équipement du joueur qui peut tomber quand il est touché : (octet de l'objet porté, compteur de
/// coups, table des chances par coup).
struct Door {
    object: u32,
    coups: u32,
    chances: u32,
}

/// Le joueur touché perd au plus un objet par coup : bouclier (`+0x165`, 0 à 3), arme spéciale (`+0x163`)
/// puis anneau (`+0x162`, sauf 1). Chaque coup décrémente un compteur ; tant qu'il reste positif, la chute
/// dépend d'un tirage (bit `rng & 0xf` du mot de la table pour ce compteur, en clair) ; l'objet tombe par
/// `0xcc352` (`creators::set_a`, D0.b = son numéro d'objet).
const BOUCLIER: Door = Door { object: 0x165, coups: 0x16d, chances: 0xdb7c4 };
const ARME: Door = Door { object: 0x163, coups: 0x16c, chances: 0xdb7b8 };
const RING: Door = Door { object: 0x162, coups: 0x16b, chances: 0xdb7d0 };
/// Nombre d'objets lâchés à ce coup (mot).
const LACHES: u32 = A5 - 0x5ac8;

/// Le compteur de coups de `p` est épuisé, ou le tirage fait tomber l'objet.
fn falls(c: &mut Cpu, b: &mut Cps2Bus, o: Obj, p: &Door) -> bool {
    let n = o.u8(b, p.coups).wrapping_sub(1);
    o.set_u8(b, p.coups, n);
    if (n as i8) < 0 {
        return true;
    }
    c.a[4] = p.chances;
    let i = 2 * n as u32;
    c.d[0] = i;
    let mask = rd16(b, p.chances + i);
    c.d[2] = c.d[2] & 0xffff_0000 | mask as u32;
    let bit = crate::rng::random(b) & 0xf;
    c.d[0] = c.d[0] & 0xffff_0000 | bit as u32;
    c.d[2] >> bit & 1 != 0
}

fn lache(c: &mut Cpu, b: &mut Cps2Bus, o: Obj, p: &Door, number: u8) {
    c.d[0] = c.d[0] & !0xff | number as u32;
    super::creators::set_a_via(c, b, number, |b| o.u8(b, Obj::VARIANT));
    o.set_u8(b, p.object, 0xff);
    o.set_u8(b, p.coups, 0);
}

fn count_dropped(b: &mut Cps2Bus) {
    let n = rd16(b, LACHES).wrapping_add(1);
    wr16(b, LACHES, n);
}

/// `0xdb6c0`: equipment dropped by the hit player `o`, for a caller still on the processor (`player::hurt`): the
/// registers are left like the original (D0, D2, A4, A0 = the player).
pub fn chute_equipement_68k(c: &mut Cpu, b: &mut Cps2Bus, o: Obj) {
    c.a[0] = o.0;
    wr16(b, LACHES, 0);
    let bouclier = o.u8(b, BOUCLIER.object);
    if bouclier <= 3 && falls(c, b, o, &BOUCLIER) {
        lache(c, b, o, &BOUCLIER, bouclier + 4);
        count_dropped(b);
    }
    if (o.u8(b, ARME.object) as i8) >= 0 && falls(c, b, o, &ARME) && rd16(b, LACHES) == 0 {
        c.d[0] = 0;
        lache(c, b, o, &ARME, 3);
        count_dropped(b);
    }
    let ring = o.u8(b, RING.object);
    if (ring as i8) >= 0 && ring != 1 && falls(c, b, o, &RING) && rd16(b, LACHES) == 0 {
        c.d[0] = c.d[0] & !0xff | ring as u32;
        super::creators::set_a_via(c, b, ring, |b| o.u8(b, Obj::VARIANT));
        o.set_u8(b, RING.object, 0xff);
        // le personnage 5 garde son anneau d'origine (1)
        let character = o.u8(b, 0x3) >> 1;
        c.d[0] = character as u32;
        if character == 5 {
            o.set_u8(b, RING.object, 1);
        }
        o.set_u8(b, RING.coups, 0);
    }
}
