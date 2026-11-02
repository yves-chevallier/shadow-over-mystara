//! Boutique : le second comptoir (PEFF 42, 0x50b9c) et l'interprète des dialogues du marchand (0x514ac /
//! 0x514cc) que lance le comptoir PEFF 43 (`peff.rs`). Voir `docs/familles/joueur.md`.
//!
//! Le client est l'objet A2 lu dans la liste des quatre joueurs (`0xff2aa4`) ; son `+0x58` positif veut dire
//! « il s'adresse au marchand » (remis à -1 dès qu'il est servi), son `+0x50` désigne l'enregistrement du
//! joueur : bourse BCD `+0x112..+0x117` (long `+0x114` comparé aux prix), six cases d'inventaire `+0x140`
//! (0x56 : vide) et leurs quantités `+0x148`, état du dialogue de la boutique `+0x167`.

use crate::families::cx::Banc;
use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::mem::{rd16, rd8, wr16, wr8, A5};
use crate::objects::Obj;
use crate::objects::Courant;

/// Message du marchand à afficher (mot, A5 - 0x554e) et les deux sons de la boutique (D0 inutile : le
/// message est lu en RAM).
const MESSAGE: u32 = A5 - 0x554e;
const SOUND_OK: u32 = 0xea57e;
const SOUND_MESSAGE: u32 = 0xea5f0;
/// Liste des quatre clients (mots d'adresse, 0 = absent).
const CUSTOMERS: u32 = 0xff2aa4;

/// Champs du client et de son joueur.
mod c {
    pub const READY: u32 = 0x58;
    pub const PLAYER: u32 = 0x50;
}
mod p {
    pub const CHARACTER: u32 = 0x3;
    pub const STATE_10D: u32 = 0x10d;
    pub const GOLD: u32 = 0x114;
    pub const GOLD_END: u32 = 0x118;
    pub const SLOTS: u32 = 0x140;
    pub const COUNTS: u32 = 0x148;
    pub const SHOP: u32 = 0x167;
}
const SLOTS: u32 = 6;
const EMPTY: u8 = 0x56;

/// Joueur du client (`movea.w (0x50,A2),A6`).
fn player_of<R: Banc>(x: &mut Cx<'_, R>, customer: Obj) -> Obj {
    let w = customer.u16(x.b, c::PLAYER);
    Obj(w as i16 as i32 as u32 & 0xff_ffff)
}

/// Première case de l'inventaire qui contient `item`.
fn find_slot<R: Banc>(x: &mut Cx<'_, R>, player: Obj, item: u8) -> Option<u32> {
    (0..SLOTS).find(|&k| player.u8(x.b, p::SLOTS + k) == item)
}

/// Pose le message et joue le son du message.
fn say<R: Banc>(x: &mut Cx<'_, R>, message: u16) {
    wr16(x.b, MESSAGE, message);
    crate::sound::jouer(x.b, SOUND_MESSAGE, x.o);
}

// ---------------------------------------------------------------------------------------------
// Type 42 (0x50b9c) : article en vitrine

/// Type 42 : un article posé sur le comptoir (variante `+3` : 0 à 11). Il apparaît quand son comptoir parent
/// (`+0x5c`) est dans l'état 2 (et, pour les variantes 0 et 7, quand la boutique le propose : octets
/// `A5-0x553f` / `A5-0x5540`), suit le défilement, et guette les clients dans sa zone (`+0x14..+0x16` en X,
/// `+0x54..+0x56` en profondeur) : il affiche son message (`+0x5a`) et vend au client qui s'adresse au
/// marchand. Prix BCD `+0x4e..+0x53` (long `+0x50`), article `+0x59`, case de la table des achats `+0x58`,
/// stock `+0x2f` (négatif : épuisé).
mod item {
    use super::*;

    const STATES: u32 = 0x50baa;
    const PARENT: u32 = 0x5c;
    const ZONE: u32 = 0x14;
    const DEPTH: u32 = 0x54;
    const SHOWN: u32 = 0x1a;
    const NEAR: u32 = 0x1b;
    const STOCK: u32 = 0x2f;
    const PRICE: u32 = 0x50;
    const PRICE_END: u32 = 0x54;
    const PURCHASE_INDEX: u32 = 0x58;
    const ITEM: u32 = 0x59;
    const OWN_MESSAGE: u32 = 0x5a;
    /// Zone de vente (quatre mots, espace programme) et tables par variante.
    const ZONE_SIZES: u32 = 0x50cf0;
    const MESSAGES: u32 = 0x50c78;
    const PRICES: u32 = 0x50c90;
    const ANIMATIONS: u32 = 0x50cc0;
    const PURCHASES: u32 = 0x50cd8;
    const ANIMS: u32 = 0x1980e4;
    const ANIMS_ALT: u32 = 0x1980fc;
    /// Variantes 0 et 7 : proposées seulement si la boutique les a (octets en A5).
    const OFFER_0: u32 = A5 - 0x553f;
    const OFFER_7: u32 = A5 - 0x5540;
    /// Achats faits dans la partie (un octet par article).
    const PURCHASED: u32 = A5 - 0x5b88;
    const MAX_COUNT: i8 = 9;

    pub fn run<R: Banc>(x: &mut Cx<'_, R>) {
        match x.state(STATES) {
            0 => init(x),
            2 => active(x),
            _ => x.free_peff(),
        }
    }

    fn parent_ready<R: Banc>(x: &mut Cx<'_, R>) -> bool {
        let parent = x.linked(PARENT);
        parent.u8(x.b, 0x6) == 2
    }

    fn follow_scroll<R: Banc>(x: &mut Cx<'_, R>) {
        let d = x.a5_u32(a5::CAM_X).wrapping_sub(x.a5_u32(0x524));
        x.add32(0x8, d);
    }

    fn init<R: Banc>(x: &mut Cx<'_, R>) {
        if !parent_ready(x) {
            return x.free_peff();
        }
        let variant = x.u8(0x3);
        let offered = match variant {
            0 => rd8(x.b, OFFER_0) != 0,
            7 => rd8(x.b, OFFER_7) != 0,
            _ => true,
        };
        if !offered {
            return follow_scroll(x);
        }
        x.add8(0x4, 2);
        for (k, off) in [ZONE, ZONE + 2, DEPTH, DEPTH + 2].into_iter().enumerate() {
            let v = x.rom16(ZONE_SIZES + 2 * k as u32);
            x.set16(off, v);
        }
        if x.u8(STOCK) != 0 {
            let v = x.u8(STOCK).wrapping_add(x.a5_u8(0x35));
            x.set8(STOCK, v);
        }
        let v = variant as u32;
        let price = x.rom32(PRICES + 4 * v);
        x.set32(PRICE, price);
        let message = x.rom16(MESSAGES + 2 * v);
        x.set16(OWN_MESSAGE, message);
        let purchase = x.data16(PURCHASES + 2 * v);
        x.set16(PURCHASE_INDEX, purchase);
        let anim = x.data16(ANIMATIONS + 2 * v);
        let set = if x.a5_u16(a5::ALT_PALETTE) != 0 { ANIMS_ALT } else { ANIMS };
        x.anim_start(set, anim);
        x.reinsert();
    }

    fn active<R: Banc>(x: &mut Cx<'_, R>) {
        follow_scroll(x);
        if !parent_ready(x) {
            return x.free_peff();
        }
        x.set8(NEAR, 0);
        for k in 0..4 {
            let w = rd16(x.b, CUSTOMERS + 2 * k);
            if w == 0 {
                continue;
            }
            let customer = Obj(w as i16 as i32 as u32 & 0xff_ffff);
            if in_zone(x, customer) {
                x.set8(NEAR, 1);
                if x.u8(SHOWN) == 0 && x.u16(OWN_MESSAGE) != 0 {
                    let m = x.u16(OWN_MESSAGE);
                    wr16(x.b, MESSAGE, m);
                    x.set8(SHOWN, 1);
                }
                if (customer.u8(x.b, c::READY) as i8) > 0 {
                    sell(x, customer);
                }
            }
        }
        if x.u8(NEAR) == 0 {
            x.set8(SHOWN, 0);
        }
    }

    /// Le client est-il dans la zone de vente ?
    fn in_zone<R: Banc>(x: &mut Cx<'_, R>, customer: Obj) -> bool {
        let dx = x.u16(0x8).wrapping_sub(customer.u16(x.b, 0x8)).wrapping_add(x.u16(ZONE));
        if dx > x.u16(ZONE + 2) {
            return false;
        }
        let dz = x
            .u16(0xc)
            .wrapping_add(x.u16(0x10))
            .wrapping_sub(customer.u16(x.b, 0xc))
            .wrapping_sub(customer.u16(x.b, 0x10))
            .wrapping_add(x.u16(DEPTH));
        dz <= x.u16(DEPTH + 2)
    }

    /// Le client s'adresse au marchand (0x50d9c) : selon la variante, l'article est refusé (message 0x86 :
    /// déjà équipé ; 0x84 : pas pour ce personnage) ou vendu.
    fn sell<R: Banc>(x: &mut Cx<'_, R>, customer: Obj) {
        customer.set_u8(x.b, c::READY, 0xff);
        let variant = x.u8(0x3);
        let player = player_of(x, customer);
        let state = player.u8(x.b, p::STATE_10D);
        let character = player.u8(x.b, p::CHARACTER);
        // D0 = 4 * variante, octet bas remplacé par le personnage, puis D0.w >> 1 : 4 * variante < 0x100 pour les
        // variantes qui s'en servent (0 à 11), la « classe » est le personnage divisé par deux
        let class = character >> 1;
        // équipement (variantes 0 et 5 à 11) : refusé s'il est déjà porté ou interdit au personnage
        let equipment = match variant {
            0 => Some((Some(8), class != 0)),
            5 => Some((Some(9), class == 1 || class == 4)),
            6 => Some((Some(0x1d), class == 4)),
            7 => Some((Some(0x1e), class == 4)),
            8..=11 => Some((None, class != 4)),
            _ => None,
        };
        match equipment {
            Some((worn, forbidden)) => {
                if worn == Some(state) {
                    return say(x, 0x86);
                }
                if forbidden {
                    return say(x, 0x84);
                }
                buy(x, player, false);
            }
            None => buy(x, player, true),
        }
    }

    /// Achat (0x50f2a équipement, 0x50f94 consommable empilable) : stock, bourse, case libre, puis paiement.
    fn buy<R: Banc>(x: &mut Cx<'_, R>, player: Obj, stackable: bool) {
        if (x.u8(STOCK) as i8) < 0 {
            return crate::sound::jouer(x.b, SOUND_MESSAGE, x.o);
        }
        let gold = player.u32(x.b, p::GOLD);
        if (gold as i32) < (x.u32(PRICE) as i32) {
            return say(x, 0x82);
        }
        let item = x.u8(ITEM);
        let slot = match find_slot(x, player, item) {
            Some(_) if !stackable => return say(x, 0x86),
            Some(k) => k,
            None => match find_slot(x, player, EMPTY) {
                Some(k) => {
                    player.set_u8(x.b, p::COUNTS + k, 0);
                    k
                }
                None => return say(x, 0x87),
            },
        };
        let count = player.u8(x.b, p::COUNTS + slot);
        if (count as i8) >= MAX_COUNT {
            return say(x, 0x86);
        }
        player.set_u8(x.b, p::SLOTS + slot, item);
        player.set_u8(x.b, p::COUNTS + slot, count.wrapping_add(1));
        let index = x.u8(PURCHASE_INDEX) as u32;
        wr8(x.b, PURCHASED + index, 1);
        let stock = x.u8(STOCK).wrapping_sub(1);
        x.set8(STOCK, stock);
        if (stock as i8) < 0 {
            x.anim_step();
            x.anim_step();
        }
        pay(x, player);
        crate::sound::jouer(x.b, SOUND_OK, x.o);
    }

    /// Bourse du joueur (six octets BCD `+0x112..+0x117`) moins le prix (`+0x4e..+0x53`), par `sbcd`.
    fn pay<R: Banc>(x: &mut Cx<'_, R>, player: Obj) {
        let mut borrow = false;
        for k in 1..=6 {
            let price = x.u8(PRICE_END - k);
            let gold = player.u8(x.b, p::GOLD_END - k);
            let (v, b) = crate::game::sbcd(gold, price, borrow);
            borrow = b;
            player.set_u8(x.b, p::GOLD_END - k, v);
        }
        // Glitch d'origine : le compteur épuisé du `dbf` reste dans D5 (`dispatch::Restes`)
        *x.c.d5() = *x.c.d5() & 0xffff_0000 | 0xffff;
    }
}

// ---------------------------------------------------------------------------------------------
// Dialogue du marchand (0x514ac, 0x514cc)

/// Interprète d'un script de dialogue du comptoir (PEFF 43) : `field` = `+0x1c` (zone A) ou `+0x58` (zone B),
/// pointeur du script, que l'interprète avance et range. Une commande = opcode (`+5`), paramètre (`+6`), mot
/// D1 (message ou saut relatif). L'état du dialogue du joueur (`+0x167`) est posé par le menu de la boutique.
mod talk {
    use super::*;

    /// Commandes (opcode = décalage dans la table de sauts 0x514ec).
    const SAY: u8 = 0x00;
    const STOP: u8 = 0x04;
    const SAY_NEXT_UNLESS: u8 = 0x08;
    const OPEN_MENU: u8 = 0x0c;
    const JUMP: u8 = 0x10;
    const SELL_CONFIRMED: u8 = 0x14;
    const CANCELLED: u8 = 0x18;
    const LEARN: u8 = 0x1c;
    const JUMP_AND_STOP: u8 = 0x20;
    const GIVE_WEAPON: u8 = 0x24;
    const GIVE_ITEM_2B: u8 = 0x28;
    const FREE: u8 = 0x2c;

    const MENU_FLAG: u32 = A5 - 0x5b48;
    const SOLD_FLAG: u32 = A5 - 0x5b22;
    const LEARNED_FLAG: u32 = A5 - 0x5b6d;
    const ITEM_2B_FLAG: u32 = A5 - 0x5b60;
    const IDLE: u8 = 0xff;

    pub fn zone_a<R: Banc>(x: &mut Cx<'_, R>, customer: Obj) {
        run(x, customer, 0x1c);
    }

    pub fn zone_b<R: Banc>(x: &mut Cx<'_, R>, customer: Obj) {
        run(x, customer, 0x58);
    }

    /// Lecture du script : position (A3 de l'original), champ du comptoir qui la range (A4), mot de la commande
    /// courante (D1.w : message ou saut relatif).
    struct Script {
        pos: u32,
        field: u32,
        word: u16,
    }

    /// Prochaine commande : `+5`, `+6`, mot ; position avancée.
    fn fetch<R: Banc>(x: &mut Cx<'_, R>, s: &mut Script) -> u8 {
        let (op, param) = (rd8(x.b, s.pos), rd8(x.b, s.pos + 1));
        x.set8(0x5, op);
        x.set8(0x6, param);
        s.word = rd16(x.b, s.pos + 2);
        s.pos += 4;
        op
    }

    /// Saut relatif du mot (`lea (0,A3,D1.w),A3`).
    fn skip(s: &mut Script) {
        s.pos = s.pos.wrapping_add(s.word as i16 as i32 as u32);
    }

    /// Range la position du script (`move.l A3,(A4)`).
    fn save<R: Banc>(x: &mut Cx<'_, R>, s: &Script) {
        let field = s.field;
        x.set32(field, s.pos);
    }

    /// Le client entre dans le menu (état 2 du joueur, sous-état 5) : son, menu ouvert.
    fn open_menu<R: Banc>(x: &mut Cx<'_, R>, s: &mut Script, player: Obj) {
        crate::sound::jouer(x.b, SOUND_OK, x.o);
        player.set_u8(x.b, 0x162, 2);
        player.set_u8(x.b, 0x16b, 5);
        player.set_u8(x.b, p::SHOP, IDLE);
        wr8(x.b, MENU_FLAG, 1);
        skip(s);
    }

    fn run<R: Banc>(x: &mut Cx<'_, R>, customer: Obj, field: u32) {
        customer.set_u8(x.b, c::READY, 0xff);
        let pos = x.u32(field);
        let s = &mut Script { pos, field, word: 0 };
        let mut op = fetch(x, s);
        loop {
            // l'original indexe sa table par D0.w, dont l'octet haut vient de l'appelant : le comptoir y laisse
            // un écart en profondeur au plus égal à l'étendue de la zone, inférieure à 0x100 (0x51306, 0x51376),
            // donc nul : seul l'octet de commande compte
            match op {
                SAY => {
                    if rd16(x.b, MESSAGE) == 0 {
                        wr16(x.b, MESSAGE, s.word);
                        save(x, s);
                    }
                    return;
                }
                STOP => return,
                SAY_NEXT_UNLESS => {
                    if x.a5_u8(0x5e) != 0 {
                        skip(s);
                    }
                    fetch(x, s);
                    op = SAY;
                    continue;
                }
                OPEN_MENU => {
                    let waiting = x.u8(0x6) != 0;
                    if waiting {
                        save(x, s);
                    }
                    let player = player_of(x, customer);
                    if player.u8(x.b, p::SHOP) != 0 {
                        if waiting {
                            return;
                        }
                    } else {
                        open_menu(x, s, player);
                    }
                }
                JUMP => skip(s),
                SELL_CONFIRMED => {
                    let player = player_of(x, customer);
                    if player.u8(x.b, p::SHOP) == 1 {
                        crate::sound::jouer(x.b, SOUND_OK, x.o);
                        player.set_u8(x.b, p::SHOP, 0xa);
                        wr8(x.b, SOLD_FLAG, 1);
                        skip(s);
                    }
                }
                CANCELLED => {
                    let player = player_of(x, customer);
                    if player.u8(x.b, p::SHOP) == 4 {
                        player.set_u8(x.b, p::SHOP, IDLE);
                        skip(s);
                    }
                }
                LEARN => {
                    let player = player_of(x, customer);
                    if player.u8(x.b, p::SHOP) == 3 {
                        let character = player.u8(x.b, p::CHARACTER);
                        // D0.w = personnage : l'octet haut est celui de l'écart en profondeur que le comptoir y
                        // laisse, nul (étendues de 0x51306 / 0x51376 toutes inférieures à 0x100)
                        if character < 8 {
                            player.set_u8(x.b, p::SHOP, IDLE);
                            player.set_u8(x.b, 0x164, 4);
                            player.set_u8(x.b, 0x10e, 0x2c);
                            wr8(x.b, LEARNED_FLAG, 1);
                            crate::sound::jouer(x.b, SOUND_OK, x.o);
                        } else {
                            player.set_u8(x.b, p::SHOP, 0xa);
                            wr8(x.b, SOLD_FLAG, 1);
                        }
                        wr16(x.b, MESSAGE, 0xeb);
                        save(x, s);
                        return;
                    }
                }
                JUMP_AND_STOP => {
                    skip(s);
                    save(x, s);
                    return;
                }
                GIVE_WEAPON => {
                    let player = player_of(x, customer);
                    if player.u8(x.b, p::SHOP) == 8 {
                        if give_weapon(x, s, player) {
                            return;
                        }
                    }
                }
                GIVE_ITEM_2B => {
                    let player = player_of(x, customer);
                    if player.u8(x.b, p::SHOP) == 5 {
                        if give_item_2b(x, s, player) {
                            return;
                        }
                    }
                }
                FREE => return x.free_peff(),
                _ => unreachable!("commande de dialogue {op:#x}"),
            }
            op = fetch(x, s);
        }
    }

    /// Arme offerte selon la classe (0x5164c) dans la première case vide ; sans place, message 0x87 et la
    /// lecture continue. Vrai quand le dialogue s'arrête.
    fn give_weapon<R: Banc>(x: &mut Cx<'_, R>, s: &Script, player: Obj) -> bool {
        let Some(k) = find_slot(x, player, EMPTY) else {
            wr16(x.b, MESSAGE, 0x87);
            crate::sound::jouer(x.b, SOUND_MESSAGE, x.o);
            return false;
        };
        player.set_u8(x.b, p::SHOP, IDLE);
        crate::sound::jouer(x.b, SOUND_OK, x.o);
        let character = player.u8(x.b, p::CHARACTER);
        // D0.w = personnage (octet haut nul, voir `LEARN`), divisé par deux
        let class = character >> 1;
        let (item, flag) = match class {
            4 => (0x4e, A5 - 0x5b76),
            1 => (0x51, A5 - 0x5b73),
            _ => (0x39, A5 - 0x5b84),
        };
        player.set_u8(x.b, p::SLOTS + k, item);
        player.set_u8(x.b, p::COUNTS + k, 0);
        wr8(x.b, flag, 1);
        wr16(x.b, MESSAGE, 0xea);
        save(x, s);
        true
    }

    /// Objet 0x2b offert (0x51706) : déjà possédé, le dialogue s'arrête sur un son ; sinon dans la première
    /// case vide (message 0xfa), et sans place, message 0x87 et la lecture continue.
    fn give_item_2b<R: Banc>(x: &mut Cx<'_, R>, s: &Script, player: Obj) -> bool {
        const ITEM: u8 = 0x2b;
        if find_slot(x, player, ITEM).is_some() {
            save(x, s);
            player.set_u8(x.b, p::SHOP, IDLE);
            crate::sound::jouer(x.b, SOUND_OK, x.o);
            return true;
        }
        let Some(k) = find_slot(x, player, EMPTY) else {
            wr16(x.b, MESSAGE, 0x87);
            crate::sound::jouer(x.b, SOUND_MESSAGE, x.o);
            return false;
        };
        player.set_u8(x.b, p::SHOP, IDLE);
        crate::sound::jouer(x.b, SOUND_OK, x.o);
        player.set_u8(x.b, p::SLOTS + k, ITEM);
        player.set_u8(x.b, p::COUNTS + k, 1);
        wr8(x.b, ITEM_2B_FLAG, 1);
        wr16(x.b, MESSAGE, 0xfa);
        save(x, s);
        true
    }
}

handlers! {
    0x50b9c "peff_42" #[sans] item::run,
}

/// Interprètes du dialogue (0x514ac, 0x514cc) : appelés par le comptoir (PEFF 43, `peff.rs`) avec x.o = le comptoir,
/// `customer` = le client (A2 de l'original).
pub fn talk_a<R: Banc>(x: &mut Cx<'_, R>, customer: Obj) {
    talk::zone_a(x, customer);
}
pub fn talk_b<R: Banc>(x: &mut Cx<'_, R>, customer: Obj) {
    talk::zone_b(x, customer);
}
