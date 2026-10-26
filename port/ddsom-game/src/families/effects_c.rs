//! Famille EFFCT (suite) : types de scène encore exercés par les scénarios, réécrits sur [`Cx`]
//! (table des types `0x39ce0`). Voir `docs/familles.md` §4.
//!
//! - 3 : élément d'écran fixé à la caméra ;
//! - 10 : dalles en diagonale qui s'enfoncent sous les joueurs ;
//! - 12 : maillon d'une chaîne qui rejoue avec retard les directions du précédent ;
//! - 16 : double d'un porteur, qui glisse à sa place puis suit les étapes du porteur ;
//! - 20 : curseur de don d'or de la boutique (dessine la jauge de vie et l'or du joueur) ;
//! - 22 : comptoir de boutique (zone de vente, qui annonce l'article au joueur qui y entre) ;
//! - 23 : vote des joueurs pour le chemin suivant (scrutin, curseurs, compte à rebours) ;
//! - 30 : poissons de la scène d'introduction (banc qui traverse, poisson qui nage et saute) ;
//! - 34 : plateau qui avance en emportant les joueurs ;
//! - 36 : objet flottant d'une scène d'introduction (ondule, est touché, s'efface ; son reflet) ;
//! - 37, 38 : panneaux d'un écran de fin d'épreuve (titre qui descend, message qui tombe et clignote) ;
//! - 39 : morceau projeté vers un point de l'écran, qui éclate en touchant le sol ;
//! - 40 : effets d'une attaque (éclats qui suivent l'attaquant, posés devant lui ou tombant du haut de l'écran, débris) ;
//! - 41 : objet lancé au hasard (quatre sortes) qui retombe puis glisse ou rebondit ;
//! - 42 : effets de scène, onze sous-types par variante (débris lancés ou repoussés, éclats, caméra alternative qui suit un
//!   objet avec retard, étincelles, lueur, bouffées, poussière) ;
//! - 44 : effets des sorts d'un ennemi magicien (six modes) ;
//! - 47 : repère qui clignote lentement ;
//! - 49 : accessoire tenu par un porteur, qui tombe quand le porteur passe dans un état donné ;
//! - 50 : effets attachés à un objet porteur (reflets, éclats, tremblement), seize variantes ;
//! - 58 : machine de la scène d'introduction (même séquence que le type 36) et son reflet ;
//! - 59 : créature qui surgit, s'élève avec la caméra, tire puis bondit hors de l'écran ;
//! - 60 : second double d'un porteur (comme le 16, avec éclats et morceau projeté) ;
//! - 61 : créature de la scène d'introduction qui sort de l'eau, ondule et s'en va ;
//! - 64 : éclat projeté qui retombe sous l'horizon ;
//! - 65 : objet qui suit un trajet puis fait naître un autre objet ;
//! - 66 : quatre gros caractères animés dans le fond 2 ;
//! - 67 : colonne qui s'élève sous un objet, puis relâche les joueurs ;
//! - 69 : silhouette qui fait les cent pas en haut de l'écran ;
//! - 70 : comptoir d'une boutique de scène (vente : vendeur, enseigne et articles, jamais exercée) ;
//! - 76 : minuteur de scène qui lance une transition ;
//! - 82 : éclat posé à côté de son créateur ; 83 : pluie d'éclats depuis le haut de l'écran ;
//! - 84 : déclencheur de la séquence de fin (fondu, caméra, tâches de script) ;
//! - 85 : pièce de la séquence de fin ; 86 : flammèche qui s'éloigne de son créateur ;
//! - 87 : interrupteur de barrière (posé par la variante, signale un joueur ou un tonneau qui le touche) ;
//! - 88 : barrière, qui crée ses interrupteurs et s'ouvre quand tous sont appuyés ;
//! - 90 : poussière qui dérive en haut de l'écran ; 91 : autre élément d'écran fixé à la caméra.
//!
//! Les états qu'aucun scénario n'exerce restent en code d'origine (`Cx::call` sur leur adresse).

use super::cx::{handlers, Cx};
use crate::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use crate::objects::Obj;

/// Champs partagés.
mod f {
    pub const STATE: u32 = 0x04;
    pub const SUB_STATE: u32 = 0x05;
    pub const VARIANT: u32 = 0x03;
    pub const HIDDEN: u32 = 0x40;
}

/// Routines d'origine utilisées.
mod rom {
    pub const FREE_EFFECT: u32 = 0x2218;
}

/// Allocation d'un EFFCT : A1 = emplacement, retenue = pool plein.
const ALLOC_EFFECT: u32 = 0x2318;

/// Joueurs : quatre enregistrements de 0x200 octets.
const PLAYERS: u32 = A5 + 0x5de;
const PLAYER_STRIDE: u32 = 0x200;

fn players() -> impl Iterator<Item = Obj> {
    (0..4).map(|k| Obj(PLAYERS + k * PLAYER_STRIDE))
}

fn free(x: &mut Cx) {
    x.call(rom::FREE_EFFECT);
}

/// Indique si `o` est dans la boîte `(dx, demi-largeur, dy, demi-profondeur)` centrée sur l'objet courant
/// décalé de (dx, dy) : comparaisons non signées sur 16 bits, comme l'original.
fn in_box(x: &mut Cx, o: Obj, (dx, half_w, dy, half_d): (u16, u16, u16, u16)) -> bool {
    let (ox, oy) = (o.u16(x.b, Obj::X), o.u16(x.b, Obj::Y));
    let left = x.u16(Obj::X).wrapping_add(dx).wrapping_sub(half_w);
    if left > ox || left.wrapping_add(half_w.wrapping_mul(2)) < ox {
        return false;
    }
    let far = x.u16(Obj::Y).wrapping_add(dy).wrapping_add(half_d);
    far >= oy && far.wrapping_sub(half_d.wrapping_mul(2)) <= oy
}

// ---------------------------------------------------------------------------------------------
// Type 22 : comptoir de boutique
// ---------------------------------------------------------------------------------------------

mod shop {
    /// Tables indexées par la variante : zone (4 mots, données), prix (long), article (mot), annonce
    /// (mot, espace programme), animation (mot).
    pub const ZONES: u32 = 0xaf7d6;
    pub const PRICES: u32 = 0xaf566;
    pub const ITEMS: u32 = 0xaf73a;
    pub const MESSAGES: u32 = 0xaf4ca;
    pub const ANIMATIONS: u32 = 0xaf69e;
    /// Stock par difficulté (`A5+0x35`, octets, espace programme).
    pub const STOCK: u32 = 0xaf4c4;
    pub const ANIM_SET: u32 = 0x1c40c6;
    /// Achat (`0xafad0`) : par variante, un `bra.w` vers la sorte d'achat (78 entrées).
    pub const PURCHASES: u32 = 0xafade;
    pub const PURCHASE_COUNT: u32 = 78;
    /// Sortes d'achat (cibles des `bra.w`).
    pub const EXIT: u32 = 0xafc16;
    pub const MEAL_BIG: u32 = 0xafc34;
    pub const MEAL: u32 = 0xafc3a;
    pub const NOT_CLASS_4: u32 = 0xafcb4;
    pub const NOT_CLASSES_2_4_1: u32 = 0xafcd0;
    pub const NOT_CLASSES_4_1: u32 = 0xafce8;
    pub const NOT_CLASS_1: u32 = 0xafd00;
    /// Messages de la boutique (`A5-0x554e`) : or insuffisant, refus de la classe, vie pleine,
    /// objet au maximum, inventaire plein.
    pub const NO_GOLD: u16 = 0x82;
    pub const WRONG_CLASS: u16 = 0x84;
    pub const FULL_LIFE: u16 = 0x85;
    pub const TOO_MANY: u16 = 0x86;
    pub const NO_ROOM: u16 = 0x87;
    pub const SOUND_REFUSED: u32 = 0xea5f0;
    pub const SOUND_BOUGHT: u32 = 0xea57e;
    /// Sortie de la boutique : palette pleine (`0x1686`, couches `A5+0x48e`).
    pub const FULL_BRIGHTNESS: u32 = 0x1686;
    /// Pointeurs (mots) des quatre acheteurs possibles.
    pub const BUYERS: u32 = 0xff2aa4;
    /// Annonce en cours (`A5-0x554e`).
    pub const MESSAGE: u32 = 0xff2ab2;
    // champs
    pub const STOCK_LEFT: u32 = 0x18;
    pub const PRICE: u32 = 0x50;
    pub const ITEM: u32 = 0x58;
    pub const MESSAGE_ID: u32 = 0x5a;
    pub const ANNOUNCED: u32 = 0x5c;
    pub const OCCUPIED: u32 = 0x5d;
}

fn shop(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => shop_init(x),
        2 => shop_watch(x, shop_buy),
        _ => free(x),
    }
}

fn shop_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    // scène 0x4c, deuxième passage : articles de la seconde moitié de la table
    let v = x.u8(f::VARIANT);
    if x.a5_u8(0x34) == 0x4c && (v as i8) < 0x40 && x.a5_u8(0x5e) != 0 && v != 0 {
        x.set8(f::VARIANT, if v == 0x14 { 0x0d } else { v }.wrapping_add(0x40));
    }
    let v = x.u8(f::VARIANT) as u32;
    let zone = shop::ZONES + 8 * v;
    for (k, off) in [0x14, 0x16, 0x54, 0x56].into_iter().enumerate() {
        let w = x.data16(zone + 2 * k as u32);
        x.set16(off, w);
    }
    let level = x.a5_u8(0x35) as u32;
    let stock = x.rom8(shop::STOCK + level);
    x.set8(shop::STOCK_LEFT, stock);
    let price = x.data32(shop::PRICES + 4 * v);
    x.set32(shop::PRICE, price);
    let message = x.rom16(shop::MESSAGES + 2 * v);
    x.set16(shop::MESSAGE_ID, message);
    let item = x.data16(shop::ITEMS + 2 * v);
    x.set16(shop::ITEM, item);
    let anim = x.data16(shop::ANIMATIONS + 2 * v) as u32;
    x.anim_start(shop::ANIM_SET, anim);
    x.reinsert();
}

/// Cherche un acheteur dans la zone ; annonce l'article au premier qui entre.
fn shop_watch(x: &mut Cx, buy: fn(&mut Cx, Obj)) {
    x.set8(shop::OCCUPIED, 0);
    // la boucle de l'original tient dans A1 (pointeur) et D6 (compteur), que l'achat peut modifier
    x.c.a[1] = shop::BUYERS;
    x.c.d[6] = 3;
    loop {
        let p = rd16(x.b, x.c.a[1]);
        if p != 0 {
            let buyer = Obj(p as i16 as i32 as u32 & 0xff_ffff);
            if shop_in_zone(x, buyer) {
                x.set8(shop::OCCUPIED, 1);
                let message = x.u16(shop::MESSAGE_ID);
                if x.u8(shop::ANNOUNCED) == 0 && message != 0 {
                    crate::mem::wr16(x.b, shop::MESSAGE, message);
                    x.set8(shop::ANNOUNCED, 1);
                }
                if (buyer.u8(x.b, 0x58) as i8) > 0 {
                    x.c.a[2] = buyer.0;
                    buy(x, buyer);
                }
            }
        }
        x.c.a[1] = x.c.a[1].wrapping_add(2);
        let n = (x.c.d[6] as u16).wrapping_sub(1);
        x.c.d[6] = x.c.d[6] & 0xffff_0000 | n as u32;
        if n == 0xffff {
            break;
        }
    }
    if x.u8(shop::OCCUPIED) == 0 {
        x.set8(shop::ANNOUNCED, 0);
    }
}

/// Joueur d'un acheteur (`+0x50`).
fn buyer_player(x: &mut Cx, buyer: Obj) -> Obj {
    let p = buyer.u16(x.b, 0x50);
    Obj(p as i16 as i32 as u32 & 0xff_ffff)
}

/// Refus : son, puis message.
fn shop_refuse(x: &mut Cx, message: u16) {
    x.call(shop::SOUND_REFUSED);
    crate::mem::wr16(x.b, shop::MESSAGE, message);
}

/// `0xafad0` : achat par l'acheteur `buyer` (A2), selon la sorte d'achat de la variante. A1 et D6 (la boucle
/// de `shop_watch`) sont rendus comme l'original les laisse.
fn shop_buy(x: &mut Cx, buyer: Obj) {
    let v = x.u8(f::VARIANT) as u32;
    if v >= shop::PURCHASE_COUNT {
        return;
    }
    let at = shop::PURCHASES + 4 * v;
    let kind = (at + 2).wrapping_add(x.rom16(at + 2) as i16 as i32 as u32);
    match kind {
        shop::EXIT => {
            // la sortie : l'acheteur quitte la liste et passe à l'état 4, la palette revient
            crate::mem::wr16(x.b, x.c.a[1], 0);
            buyer.set_u16(x.b, Obj::STATE, 0x400);
            let (a1, d6) = (x.c.a[1], x.c.d[6]);
            x.set_a5_u8(0x48e, 0xf);
            x.call(shop::FULL_BRIGHTNESS);
            (x.c.a[1], x.c.d[6]) = (a1, d6);
        }
        shop::MEAL_BIG => shop_meal(x, buyer, 0x30),
        shop::MEAL => shop_meal(x, buyer, 0x20),
        _ => {
            buyer.set_u8(x.b, 0x58, 0xff);
            let class = buyer_player(x, buyer).u8(x.b, 3) >> 1;
            let refused: &[(u8, u16)] = match kind {
                shop::NOT_CLASS_4 => &[(4, shop::WRONG_CLASS)],
                shop::NOT_CLASSES_2_4_1 => &[(2, shop::TOO_MANY), (4, shop::WRONG_CLASS), (1, shop::WRONG_CLASS)],
                shop::NOT_CLASSES_4_1 => &[(4, shop::WRONG_CLASS), (1, shop::WRONG_CLASS)],
                shop::NOT_CLASS_1 => &[(1, shop::WRONG_CLASS)],
                _ => &[],
            };
            if let Some(&(_, message)) = refused.iter().find(|&&(c, _)| c == class) {
                if message == shop::TOO_MANY {
                    crate::mem::wr16(x.b, shop::MESSAGE, message);
                    return x.call(shop::SOUND_REFUSED);
                }
                return shop_refuse(x, message);
            }
            shop_take(x, buyer);
        }
    }
}

/// Or du joueur suffisant (ou objet gratuit : `+0x167` = 9) ? Sinon refus.
fn shop_can_pay(x: &mut Cx, player: Obj) -> bool {
    if player.u8(x.b, 0x167) == 9 || player.u32(x.b, GOLD) as i32 >= x.u32(shop::PRICE) as i32 {
        return true;
    }
    shop_refuse(x, shop::NO_GOLD);
    false
}

/// `0xafd18` : l'article rejoint l'inventaire (six emplacements `+0x140`, quantités `+0x148`) : même
/// article, ou emplacement vide (0x56) ; au plus 9.
fn shop_take(x: &mut Cx, buyer: Obj) {
    buyer.set_u8(x.b, 0x58, 0xff);
    let player = buyer_player(x, buyer);
    if !shop_can_pay(x, player) {
        return;
    }
    let item = x.u16(shop::ITEM) as u8;
    let slot = match (0..6).find(|&k| player.u8(x.b, 0x140 + k) == item) {
        Some(k) => k,
        None => match (0..6).find(|&k| player.u8(x.b, 0x140 + k) == 0x56) {
            Some(k) => {
                player.set_u8(x.b, 0x148 + k, 0);
                k
            }
            None => {
                crate::mem::wr16(x.b, shop::MESSAGE, shop::NO_ROOM);
                return x.call(shop::SOUND_REFUSED);
            }
        },
    };
    if player.u8(x.b, 0x148 + slot) as i8 >= 9 {
        crate::mem::wr16(x.b, shop::MESSAGE, shop::TOO_MANY);
        return x.call(shop::SOUND_REFUSED);
    }
    let id = x.u8(shop::ITEM + 1);
    player.set_u8(x.b, 0x140 + slot, id);
    let n = player.u8(x.b, 0x148 + slot).wrapping_add(1);
    player.set_u8(x.b, 0x148 + slot, n);
    let collected = x.u8(shop::ITEM) as u32;
    crate::mem::wr8(x.b, A5.wrapping_sub(0x5b88).wrapping_add(collected), 1);
    shop_pay(x, player);
}

/// `0xafdaa` : le prix (six octets BCD finissant en `+0x53`) est retiré de l'or du joueur (six octets
/// finissant en `+0x117`), sauf pour un objet gratuit (`+0x167` = 9, remis à 0xff).
fn shop_pay(x: &mut Cx, player: Obj) {
    if player.u8(x.b, 0x167) == 9 {
        player.set_u8(x.b, 0x167, 0xff);
    } else {
        let mut borrow = false;
        for k in 1..=6 {
            let (v, b) = crate::game::sbcd(player.u8(x.b, 0x118 - k), x.u8(0x54 - k), borrow);
            player.set_u8(x.b, 0x118 - k, v);
            borrow = b;
        }
        x.c.sr = x.c.sr & !m68k::SR_X | if borrow { m68k::SR_X } else { 0 };
    }
    x.call(shop::SOUND_BOUGHT);
}

/// `0xafc3c` : repas (soin de `heal`) : refusé si la vie est pleine ; le dernier du stock fait reculer
/// l'étal et efface son annonce.
fn shop_meal(x: &mut Cx, buyer: Obj, heal: u16) {
    buyer.set_u8(x.b, 0x58, 0xff);
    if x.u8(shop::STOCK_LEFT) == 0 {
        return;
    }
    let player = buyer_player(x, buyer);
    let full = player.u16(x.b, 0x60);
    if full as i16 <= player.u16(x.b, 0x62) as i16 {
        crate::mem::wr16(x.b, shop::MESSAGE, shop::FULL_LIFE);
        return x.call(shop::SOUND_REFUSED);
    }
    if !shop_can_pay(x, player) {
        return;
    }
    let left = x.u8(shop::STOCK_LEFT).wrapping_sub(1);
    x.set8(shop::STOCK_LEFT, left);
    if left == 0 {
        x.add16(Obj::X, 0xfff0);
        x.add16(Obj::Z, 0xffe8);
        x.anim_step();
        x.set16(shop::MESSAGE_ID, 0);
    }
    let life = player.u16(x.b, 0x62).wrapping_add(heal);
    player.set_u16(x.b, 0x62, life);
    if (full as i16) < life as i16 {
        player.set_u16(x.b, 0x62, full);
    }
    shop_pay(x, player);
}

fn shop_in_zone(x: &mut Cx, buyer: Obj) -> bool {
    let across = x.u16(Obj::X).wrapping_sub(buyer.u16(x.b, Obj::X)).wrapping_add(x.u16(0x14));
    if across > x.u16(0x16) {
        return false;
    }
    let depth = x
        .u16(Obj::Z)
        .wrapping_add(x.u16(Obj::Y))
        .wrapping_sub(buyer.u16(x.b, Obj::Z))
        .wrapping_sub(buyer.u16(x.b, Obj::Y))
        .wrapping_add(x.u16(0x54));
    depth <= x.u16(0x56)
}

// ---------------------------------------------------------------------------------------------
// Type 87 : interrupteur de barrière
// ---------------------------------------------------------------------------------------------

mod switch {
    /// Positions (x, profondeur) par variante, espace programme.
    pub const POSITIONS: u32 = 0xb9e08;
    /// Boîte de contact (dx, demi-largeur, dy, demi-profondeur), données.
    pub const HIT_BOX: u32 = 0xb9f2c;
    pub const ANIM_SET: u32 = 0x1bebc2;
    /// La barrière commandée (pointeur de 16 bits) ; son octet `+0x5a` non nul : déjà ouverte.
    pub const BARRIER: u32 = 0x42;
    /// Drapeau « touché » lu par la barrière.
    pub const PRESSED: u32 = 0x30;
    /// Pool BOX : 21 emplacements de 0x80 octets.
    pub const BOXES: u32 = 0xffeede;
}

fn barrier_switch(x: &mut Cx) {
    if x.u8(f::HIDDEN) != 0 {
        return;
    }
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let v = x.u8(f::VARIANT) as u32;
            let (px, py) = (x.rom16(switch::POSITIONS + 4 * v), x.rom16(switch::POSITIONS + 4 * v + 2));
            x.set16(Obj::X, px);
            x.set16(Obj::Y, py);
            x.anim_start(switch::ANIM_SET, 8);
        }
        2 => {
            if x.u8(f::SUB_STATE) == 0 {
                let barrier = x.linked(switch::BARRIER);
                if barrier.u8(x.b, 0x5a) != 0 {
                    x.add8(f::SUB_STATE, 2);
                } else {
                    x.set8(switch::PRESSED, 0);
                    switch_watch(x);
                }
            }
        }
        4 => {}
        _ => free(x),
    }
    if x.u8(0) != 0 {
        x.call(super::cx::sub::REINSERT_FOREGROUND);
    }
}

/// Un joueur au sol, puis un tonneau (BOX 3 variante 1) au sol, sur l'interrupteur.
fn switch_watch(x: &mut Cx) {
    let hit_box = (
        x.data16(switch::HIT_BOX),
        x.data16(switch::HIT_BOX + 2),
        x.data16(switch::HIT_BOX + 4),
        x.data16(switch::HIT_BOX + 6),
    );
    for p in players() {
        if p.is_active(x.b) && (p.u16(x.b, Obj::Z) as i16) <= 0 && in_box(x, p, hit_box) {
            x.set8(switch::PRESSED, 1);
            break;
        }
    }
    for k in 0..21 {
        let o = Obj(switch::BOXES + 0x80 * k);
        if rd8(x.b, o.0) != 0
            && o.u8(x.b, f::HIDDEN) == 0
            && (o.u16(x.b, Obj::Z) as i16) <= 0
            && o.u8(x.b, 2) == 3
            && o.u8(x.b, 3) == 1
            && in_box(x, o, hit_box)
        {
            return x.set8(switch::PRESSED, 1);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 20 : curseur de don d'or (boutique)
// ---------------------------------------------------------------------------------------------

mod purse {
    /// Décalage initial (x, z) par mode de création `+0x2f`, selon `A5+0x20` (1 : jeu à deux).
    pub const OFFSETS_2P: u32 = 0xaef98;
    pub const OFFSETS: u32 = 0xaef88;
    /// Adresses dans la couche de texte : chiffres de l'or (`+0x52`) et jauge (`+0x14`), par joueur.
    pub const DIGITS_2P: u32 = 0xaef60;
    pub const DIGITS: u32 = 0xaef50;
    pub const GAUGE_2P: u32 = 0xaef7c;
    pub const GAUGE: u32 = 0xaef6c;
    /// Icône du joueur (SOBJ 0) : variante (octets) et position (x, y) par joueur.
    pub const ICON_VARIANT_2P: u32 = 0xaefa8;
    pub const ICON_VARIANT: u32 = 0xaefa4;
    pub const ICON_POS_2P: u32 = 0xaefbc;
    pub const ICON_POS: u32 = 0xaefac;
    /// Palette du portrait par classe (décalages dans 0x256000).
    pub const PORTRAIT: u32 = 0xaefc8;
    pub const PORTRAITS: u32 = 0x256000;
    /// Fin de la jauge : tuile selon le reste (mots, indexés de -7 à 0).
    pub const GAUGE_END: u32 = 0xaefee;
    pub const ANIM_SET: u32 = 0x1c40c6;
    /// Allocations : 0x23d8 (ITEM), 0x228e (PEFF), 0x2408 (SOBJ).
    pub const ALLOC_ITEM: u32 = 0x23d8;
    pub const ALLOC_PEFF: u32 = 0x228e;
    pub const ALLOC_SOBJ: u32 = 0x2408;
    pub const PALETTE_PAGE: u32 = 0x1a608;
    /// Suite du code d'origine quand un pool est plein.
    pub const POOL_FULL: u32 = 0xaf2bc;
    /// Nombre de curseurs encore ouverts (`A5-0x5553`).
    pub const OPEN: u32 = 0xff2aad;
    pub const OWNER: u32 = 0x50;
    pub const GAUGE_LEN: u32 = 0x18;
    pub const DIGITS_AT: u32 = 0x52;
    pub const GAUGE_AT: u32 = 0x14;
    pub const GIFT: u32 = 0x1a;
    pub const LINKED: u32 = 0x5c;
    pub const OPENED: u32 = 0x58;
}

/// Or du joueur (`+0x114`, BCD sur quatre octets).
const GOLD: u32 = 0x114;

fn two_players(x: &mut Cx) -> bool {
    x.a5_u8(0x20) == 1
}

fn purse(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => purse_init(x),
        2 => purse_active(x),
        4 => purse_leave(x),
        _ => free(x),
    }
}

/// Allocation par une routine d'origine : l'emplacement (A1), ou `None` (retenue).
fn alloc(x: &mut Cx, routine: u32) -> Option<Obj> {
    x.call(routine);
    (x.c.sr & m68k::SR_C == 0).then(|| Obj(x.c.a[1]))
}

fn purse_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.set16(0x18, 0);
    let (cx, cy) = (x.a5_u16(0x506), x.a5_u16(0x50a));
    x.set16(Obj::X, cx);
    x.set16(Obj::Z, cy);
    let mode = 4 * x.u8(0x2f) as u32;
    let offsets = if two_players(x) { purse::OFFSETS_2P } else { purse::OFFSETS };
    let (dx, dz) = (x.data16(offsets + mode), x.data16(offsets + mode + 2));
    x.add16(Obj::X, dx);
    x.add16(Obj::Z, dz);
    let d7 = x.c.d[7];
    x.call(purse::PALETTE_PAGE);
    x.c.d[7] = d7;
    x.set8(0x2a, 1);
    let owner = x.linked(purse::OWNER);
    let player = owner.u8(x.b, 2);
    let palette = 0x18u8.wrapping_add(player);
    x.set8(0x2b, palette);
    crate::mem::wr32(x.b, 0x91c54e, 0xf062_f0f6);
    crate::mem::wr32(x.b, 0x91c552, 0xf800_ff33);
    // l'original garde D0 (la palette) à travers l'allocation
    x.c.d[0] = palette as u32;
    let Some(item) = alloc(x, purse::ALLOC_ITEM) else { return x.call(purse::POOL_FULL) };
    wr8(x.b, item.0, 1);
    wr8(x.b, item.0 + 2, 9);
    wr8(x.b, item.0 + 3, player);
    wr16(x.b, item.0 + 0x5c, x.o.0 as u16);
    // portrait : 8 longs de la palette de la classe, avec le bit de luminosité
    let dest = 0x91c000u32.wrapping_add(((x.c.d[0] as u16) << 5) as i16 as i32 as u32);
    let class = owner.u8(x.b, 3) as u32;
    let src = purse::PORTRAITS.wrapping_add(x.data16(purse::PORTRAIT + 2 * class) as i16 as i32 as u32);
    for k in 0..8 {
        let v = x.data32(src + 4 * k);
        let v = if k < 6 { v | 0xf000_f000 } else { v.wrapping_add(0xf000_f000) };
        wr32(x.b, dest + 4 * k, v);
    }
    let p4 = 4 * player as u32;
    let digits = if two_players(x) { purse::DIGITS_2P } else { purse::DIGITS };
    let v = x.data32(digits + p4);
    x.set32(purse::DIGITS_AT, v);
    let gauge = if two_players(x) { purse::GAUGE_2P } else { purse::GAUGE };
    let v = x.data32(gauge + p4);
    x.set32(purse::GAUGE_AT, v);
    let Some(peff) = alloc(x, purse::ALLOC_PEFF) else { return x.call(purse::POOL_FULL) };
    wr8(x.b, peff.0, 1);
    wr8(x.b, peff.0 + 2, 0x18);
    wr16(x.b, peff.0 + 0x5c, x.o.0 as u16);
    let w = x.u16(purse::OWNER);
    wr16(x.b, peff.0 + 0x50, w);
    let (pos, var) = if two_players(x) {
        (purse::ICON_POS_2P, purse::ICON_VARIANT_2P)
    } else {
        (purse::ICON_POS, purse::ICON_VARIANT)
    };
    let (pos, var) = (pos + p4, var + player as u32);
    let Some(icon) = alloc(x, purse::ALLOC_SOBJ) else { return x.call(purse::POOL_FULL) };
    wr8(x.b, icon.0, 1);
    wr8(x.b, icon.0 + 2, 0);
    let v = x.data8(var);
    wr8(x.b, icon.0 + 3, v);
    let (px, py) = (x.data16(pos).wrapping_add(cx), x.data16(pos + 2).wrapping_add(cy));
    wr16(x.b, icon.0 + Obj::X, px);
    wr16(x.b, icon.0 + Obj::Y, py);
    wr16(x.b, icon.0 + Obj::Z, 0);
    // six PEFF 0x11 (D5 : compteur de l'original, conservé par l'allocation)
    x.c.d[5] = 5;
    loop {
        let Some(o) = alloc(x, purse::ALLOC_PEFF) else { return x.call(purse::POOL_FULL) };
        wr8(x.b, o.0, 1);
        wr8(x.b, o.0 + 2, 0x11);
        wr8(x.b, o.0 + 3, x.c.d[5] as u8);
        wr16(x.b, o.0 + 0x5c, x.o.0 as u16);
        wr16(x.b, o.0 + 0x50, w);
        let n = (x.c.d[5] as u16).wrapping_sub(1);
        x.c.d[5] = x.c.d[5] & 0xffff_0000 | n as u32;
        if n == 0xffff {
            break;
        }
    }
    purse_anim(x, false);
}

/// 0xaf2c0 / 0xaf302 : animation du curseur fermé ou ouvert (`open`), selon la classe du joueur.
fn purse_anim(x: &mut Cx, open: bool) {
    let owner = x.linked(purse::OWNER);
    x.set8(purse::OPENED, open as u8);
    let class = owner.u8(x.b, 3);
    let base: u8 = match (class & 1 != 0, open) {
        (false, false) => 0x17,
        (true, false) => 0x2f,
        (false, true) => 0x18,
        (true, true) => 0x30,
    };
    x.anim_start(purse::ANIM_SET, (class & !1).wrapping_add(base) as u32);
    x.reinsert();
}

/// Libère l'objet lié (`+0x5c`) : son octet `+0x56` passe à 0xff.
fn purse_release_linked(x: &mut Cx) {
    let linked = x.linked(purse::LINKED);
    x.set16(purse::LINKED, 0);
    linked.set_u8(x.b, 0x56, 0xff);
}

fn purse_active(x: &mut Cx) {
    let owner = x.linked(purse::OWNER);
    let player = owner.u8(x.b, 2) as u32;
    let moves = A5 + player;
    let buttons = A5 + 0xc + player;
    purse_give(x, owner, buttons);
    purse_draw_gauge(x, owner);
    purse_draw_gold(x, owner);
    if x.u16(purse::LINKED) != 0 {
        if rd8(x.b, moves) & 0x10 == 0 {
            purse_release_linked(x);
            return purse_anim(x, false);
        }
    } else {
        if x.u8(purse::OPENED) != 0 {
            if !x.anim_step() {
                return;
            }
            purse_anim(x, false);
        }
        if rd8(x.b, buttons) & 0x10 != 0 {
            purse_anim(x, true);
        }
    }
    purse_move(x, moves);
}

/// Don d'or : le curseur sur un autre joueur, bouton 0x40 (10 pièces) ou 0x80 (100) appuyé.
fn purse_give(x: &mut Cx, owner: Obj, buttons: u32) {
    for k in 0..4 {
        let p = rd16(x.b, shop::BUYERS + 2 * k);
        if p == 0 || p == x.o.0 as u16 {
            continue;
        }
        let other = Obj(p as i16 as i32 as u32 & 0xff_ffff);
        let pressed = rd8(x.b, buttons);
        let amount: i32 = if pressed & 0x40 != 0 {
            0x10
        } else if pressed & 0x80 != 0 {
            0x100
        } else {
            continue;
        };
        let dx = x.u16(Obj::X).wrapping_sub(other.u16(x.b, Obj::X)).wrapping_add(4);
        let dz = x.u16(Obj::Z).wrapping_sub(other.u16(x.b, Obj::Z)).wrapping_add(8);
        if dx > 8 || dz > 0x10 {
            continue;
        }
        let amount = amount.min(owner.u32(x.b, GOLD) as i32);
        x.set32(purse::GIFT, amount as u32);
        let receiver = Obj(other.u16(x.b, 0x50) as i16 as i32 as u32 & 0xff_ffff);
        let mut carry = false;
        for i in (1..4).rev() {
            let (v, c) = crate::ops::abcd(x.u8(purse::GIFT + i), receiver.u8(x.b, GOLD + i), carry);
            receiver.set_u8(x.b, GOLD + i, v);
            carry = c;
        }
        if receiver.u32(x.b, GOLD) as i32 > 0x99_9999 {
            receiver.set_u32(x.b, GOLD, 0x99_9999);
        }
        let mut borrow = false;
        for i in (1..4).rev() {
            let (v, c) = crate::game::sbcd(owner.u8(x.b, GOLD + i), x.u8(purse::GIFT + i), borrow);
            owner.set_u8(x.b, GOLD + i, v);
            borrow = c;
        }
    }
}

/// Jauge de vie du joueur (`+0x60` maximum, `+0x62` courant) : rattrape la valeur courante d'un cran
/// par image, puis dessine les tuiles (8 points chacune) dans la couche de texte.
fn purse_draw_gauge(x: &mut Cx, owner: Obj) {
    let mut at = x.u32(purse::GAUGE_AT);
    let len = x.u16(purse::GAUGE_LEN);
    let (mut max, mut cur) = (owner.u16(x.b, 0x60), owner.u16(x.b, 0x62));
    if two_players(x) {
        max >>= 1;
        cur >>= 1;
    }
    if len == cur {
        return;
    }
    let len = if (len as i16) > (cur as i16) { len.wrapping_sub(1) } else { len.wrapping_add(1) };
    x.set16(purse::GAUGE_LEN, len);
    let mut left = len;
    loop {
        // subq.w #8 ; ble
        let last = (max as i16) <= 8;
        max = max.wrapping_sub(8);
        if last {
            break;
        }
        left = left.wrapping_sub(8);
        let tile = if (left as i16) < 0 {
            let t = left.wrapping_add(0x1318);
            left = 0;
            t
        } else {
            0x1318
        };
        wr16(x.b, at, tile);
        wr16(x.b, at + 2, 0xa);
        at = at.wrapping_add(0x80);
    }
    let end = purse::GAUGE_END.wrapping_add(max.wrapping_mul(2) as i16 as i32 as u32);
    let tile = x.data16(end).wrapping_add(left);
    wr16(x.b, at, tile);
    wr16(x.b, at + 2, 0xa);
}

/// Or du joueur en six chiffres (zéros de tête effacés), puis trois tuiles fixes.
fn purse_draw_gold(x: &mut Cx, owner: Obj) {
    let mut gold = owner.u32(x.b, GOLD).rotate_left(8);
    let mut at = x.u32(purse::DIGITS_AT);
    let mut shown = false;
    for _ in 0..5 {
        gold = gold.rotate_left(4);
        let digit = gold as u16 & 0xf;
        let tile = if digit != 0 || shown {
            shown = true;
            digit + 0x310
        } else {
            0x10
        };
        wr16(x.b, at, tile);
        wr16(x.b, at + 2, 0);
        at = at.wrapping_add(0x80);
    }
    gold = gold.rotate_left(4);
    wr16(x.b, at, (gold as u16 & 0xf) + 0x310);
    wr16(x.b, at + 2, 0);
    wr32(x.b, at + 0x80, 0x10_0000);
    wr32(x.b, at + 0x100, 0x36c_0000);
    wr32(x.b, at + 0x180, 0x369_0000);
}

/// Déplacement par les directions du joueur (bit 5 : rapide), borné à l'écran.
fn purse_move(x: &mut Cx, moves: u32) {
    let m = rd8(x.b, moves);
    let step: u16 = if m & 0x20 != 0 { 5 } else { 2 };
    if m & 8 != 0 {
        x.add16(Obj::Z, step);
    }
    if m & 4 != 0 {
        x.add16(Obj::Z, step.wrapping_neg());
    }
    if m & 2 != 0 {
        x.add16(Obj::X, step.wrapping_neg());
    }
    if m & 1 != 0 {
        x.add16(Obj::X, step);
    }
    let clamp = |x: &mut Cx, off: u32, origin: u16, span: u16| {
        let v = x.u16(off) as i16;
        let (low, high) = (origin.wrapping_add(0x14), origin.wrapping_add(span));
        if low as i16 > v {
            x.set16(off, low);
        } else if (high as i16) < v {
            x.set16(off, high);
        }
    };
    let (cx, cy) = (x.a5_u16(0x506), x.a5_u16(0x50a));
    clamp(x, Obj::X, cx, 0x16c);
    clamp(x, Obj::Z, cy, 0x100);
}

/// Sortie : le curseur se ferme puis glisse vers la droite hors de l'écran.
fn purse_leave(x: &mut Cx) {
    if x.u8(f::SUB_STATE) == 0 {
        purse_anim(x, false);
        x.add8(f::SUB_STATE, 1);
        if x.u16(purse::LINKED) != 0 {
            purse_release_linked(x);
        }
    }
    x.add16(Obj::X, 4);
    let cx = x.a5_u16(0x506);
    if (x.u16(Obj::X).wrapping_sub(0x1a0) as i16) <= cx as i16 {
        return;
    }
    let n = rd8(x.b, purse::OPEN).wrapping_sub(1);
    wr8(x.b, purse::OPEN, if (n as i8) < 0 { 0 } else { n });
    free(x);
}

// ---------------------------------------------------------------------------------------------
// Type 50 : effets attachés à un objet (16 variantes)
// ---------------------------------------------------------------------------------------------

mod attached {
    pub const ANIM_SET: u32 = 0x19626c;
    pub const ANIM_SET_B: u32 = 0x162ebc;
    /// Durée (variante 5) et décalages aléatoires, mots indexés par 4 bits du générateur (espace
    /// programme).
    pub const DELAYS: u32 = 0xb6160;
    pub const JITTER: u32 = 0xb6180;
    /// L'objet porteur (pointeur de 16 bits).
    pub const CARRIER: u32 = 0x42;
    pub const TIMER: u32 = 0x58;
    pub const BLINK: u32 = 0x30;
}

/// `subq.w #1,(off,A0)` suivi de `bgt` : vrai si le compteur reste positif.
fn countdown(x: &mut Cx, off: u32) -> bool {
    let old = x.u16(off);
    x.set16(off, old.wrapping_sub(1));
    (old as i16) > 1
}

/// `bchg #1,(1,A0)` : bascule l'affichage.
fn toggle_hidden(x: &mut Cx) {
    let v = x.u8(0x01) ^ 2;
    x.set8(0x01, v);
}

fn set_hidden(x: &mut Cx, on: bool) {
    x.set_bit(0x01, 1, on);
}

/// Tirage du générateur du jeu (0x19a88), 4 bits.
fn random_nibble(x: &mut Cx) -> u32 {
    x.call(super::rom::RNG);
    x.c.d[0] & 0xf
}

fn copy_position(x: &mut Cx, from: Obj) {
    for field in [Obj::X, Obj::Z, Obj::Y] {
        let v = from.u16(x.b, field);
        x.set16(field, v);
    }
}

fn attached(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => attached_init(x),
        2 => attached_active(x),
        _ => free(x),
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

fn attached_init(x: &mut Cx) {
    let v = x.u8(f::VARIANT);
    if v != 4 {
        x.add8(f::STATE, 2);
    }
    let carrier = x.linked(attached::CARRIER);
    match v {
        0..=3 => {
            copy_position(x, carrier);
            if v == 3 {
                x.add16(Obj::Y, 4u16.wrapping_neg());
            }
            x.anim_start(attached::ANIM_SET, v as u32);
        }
        4 => {
            // reflet : recopie le cadre de sprite du porteur
            copy_position(x, carrier);
            x.add16(Obj::Y, 1);
            let frame = carrier.u32(x.b, 0x34);
            x.set32(0x34, frame);
            for off in [0x3c, 0x3f] {
                let b = carrier.u8(x.b, off);
                x.set8(off, b);
            }
            x.set16(attached::BLINK, 0x14);
            x.add8(f::STATE, 2);
        }
        5 => {
            set_hidden(x, true);
            let k = random_nibble(x);
            let delay = x.rom16(attached::DELAYS + 2 * k);
            x.set16(attached::TIMER, delay);
            let k = random_nibble(x);
            let dx = x.rom16(attached::JITTER + 2 * k);
            x.add16(Obj::X, dx);
            if (carrier.u8(x.b, f::VARIANT) as i8) < 5 {
                let k = random_nibble(x);
                let dy = x.rom16(attached::JITTER + 2 * k);
                x.add16(Obj::Y, dy);
            }
            x.anim_start(attached::ANIM_SET, 4);
            let carrier = x.linked(attached::CARRIER);
            if matches!(carrier.u8(x.b, f::VARIANT), 4 | 9) {
                x.call(0xe5d36);
            }
        }
        6 => x.anim_start(attached::ANIM_SET, 5),
        7 => {}
        8..=11 => {
            const SOUNDS: [u32; 4] = [0xe6a8e, 0xe6fe6, 0xe713c, 0xe6e90];
            x.call(SOUNDS[v as usize - 8]);
            x.anim_start(attached::ANIM_SET, v as u32 - 3);
        }
        12 | 13 => {
            set_hidden(x, true);
            x.anim_start(attached::ANIM_SET, v as u32 - 3);
        }
        14 => x.anim_start(attached::ANIM_SET_B, 0x16),
        15 => {
            x.set16(attached::TIMER, 0x28);
            x.anim_start(attached::ANIM_SET, 0xb);
        }
        _ => {}
    }
}

fn attached_active(x: &mut Cx) {
    let v = x.u8(f::VARIANT);
    let sub = x.u8(f::SUB_STATE);
    match v {
        0..=3 => {
            let carrier = x.linked(attached::CARRIER);
            if carrier.u8(x.b, f::STATE) == 4 {
                return free(x);
            }
            copy_position(x, carrier);
            if v == 3 {
                x.add16(Obj::Y, 4u16.wrapping_neg());
            }
            if x.anim_step() {
                free(x);
            }
        }
        4 if sub == 0 => {
            if !countdown(x, attached::BLINK) {
                x.add8(f::SUB_STATE, 2);
                x.set16(attached::BLINK, 0xe);
            }
        }
        4 | 7 => {
            if countdown(x, attached::BLINK) {
                toggle_hidden(x);
            } else {
                free(x);
            }
        }
        5 if sub == 0 => {
            if countdown(x, attached::TIMER) {
                return;
            }
            x.add8(f::SUB_STATE, 2);
            set_hidden(x, false);
            x.c.d[0] = 0xc;
            x.call(0x2860);
            x.anim_step();
        }
        5 | 6 | 8..=11 => {
            if x.anim_step() {
                free(x);
            }
        }
        12 | 13 => {
            if sub == 0 {
                if countdown(x, attached::TIMER) {
                    return;
                }
                x.add8(f::SUB_STATE, 2);
                set_hidden(x, false);
                x.call(super::cx::sub::MOVE_XYZ);
                x.anim_step();
            } else {
                x.call(super::cx::sub::MOVE_XYZ);
                if x.anim_step() {
                    free(x);
                } else {
                    toggle_hidden(x);
                }
            }
        }
        14 => attached_shake(x, sub),
        15 => {
            if !countdown(x, attached::TIMER) {
                return free(x);
            }
            toggle_hidden(x);
            x.anim_step();
        }
        _ => {}
    }
}

/// Variante 14 : attend que le porteur quitte l'état actif, tremble, puis s'efface en clignotant.
fn attached_shake(x: &mut Cx, sub: u8) {
    match sub {
        0 => {
            let carrier = x.linked(attached::CARRIER);
            if carrier.u16(x.b, f::STATE) != 0x200 {
                x.add8(f::SUB_STATE, 2);
                x.set16(attached::TIMER, 10);
            }
        }
        2 => {
            if !countdown(x, attached::TIMER) {
                x.add8(f::SUB_STATE, 2);
                x.set16(attached::TIMER, 0x1e);
                x.set16(Obj::VX, 2);
            }
        }
        4 | 6 => {
            let vx = x.u16(Obj::VX).wrapping_neg();
            x.set16(Obj::VX, vx);
            x.add16(Obj::X, vx);
            if sub == 4 {
                if !countdown(x, attached::TIMER) {
                    x.add8(f::SUB_STATE, 2);
                    let carrier = x.linked(attached::CARRIER);
                    let facing = carrier.u8(x.b, 0x2c);
                    x.set8(0x2c, facing);
                    x.anim_start(attached::ANIM_SET_B, 0x17);
                }
            } else if x.anim_step() {
                x.add8(f::SUB_STATE, 2);
                x.set16(attached::TIMER, 0x3c);
                x.c.d[5] = 0xb;
                x.call(0x1172e);
            }
        }
        8 => {
            if !countdown(x, attached::TIMER) {
                return free(x);
            }
            if (x.u16(attached::TIMER) as i16) <= 0x1e {
                toggle_hidden(x);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Type 42 : effets de scène (onze sous-types selon la variante)
// ---------------------------------------------------------------------------------------------

mod scenery {
    /// Variante 1 : par réglage, fiche de 16 octets (dx, dy, vx, ax, vy, ay ; données).
    pub const THROWS_1: u32 = 0xb4202;
    /// Variante 2 : par réglage, décalage (dx, dy) (données) ; poussée avec les joueurs.
    pub const PLACES_2: u32 = 0xb4394;
    pub const PUSH_PLAYERS: u32 = 0x81aa4;
    /// Variante 9 : par réglage (`& 7`), fiche de 8 octets (dx, dz en octets signés, puis vz, az ; programme).
    pub const SPRAY: u32 = 0xb4a32;
    pub const ANIM_SPRAY: u32 = 0x17b9d6;
    /// Variante 0 : éclat bref (animation 0xf, ou 0x10 retournée), jeu d'animations 0x174776.
    pub const ANIM_FLASH: u32 = 0x174776;
    /// Objet suivi (pointeur de 16 bits, négatif quand il existe).
    pub const TARGET: u32 = 0x46;
    pub const ANIM_DEBRIS: u32 = 0x1948ea;
    pub const ANIM_DUST: u32 = 0x1901ba;
    pub const ANIM_GLOW: u32 = 0x17e048;
    /// Vitesses (vz, az) par mode `+0x2f` : variante 3 (données), variante 4 (données).
    pub const FALL_3: u32 = 0xb4480;
    pub const FALL_4: u32 = 0xb45ba;
    /// Vitesses x de la variante 4 (espace programme, indice pair aléatoire).
    pub const DRIFT_4: u32 = 0xb45aa;
    /// Zones d'apparition de la variante 8 : (x, masque, y, masque) par mode (données).
    pub const AREAS_8: u32 = 0xb4986;
    /// Variante 10 : animation (octets) et vitesses (vx, ax) par mode (espace programme).
    pub const ANIMS_10: u32 = 0xb4af2;
    pub const SPEEDS_10: u32 = 0xb4b56;
    /// Cadres de sprite de la lueur (variante 7), longs.
    pub const GLOW_FRAMES: u32 = 0x17efca;
    /// Historique des positions de l'objet suivi (variante 5) : 8 longs (x, y + z) à partir de `+8`.
    pub const TRAIL: u32 = 0x08;
    pub const TRAIL_WRITE: u32 = 0x06;
    pub const TRAIL_READ: u32 = 0x07;
    /// Caméra alternative (`A5+0x556`, `A5+0x55a`).
    pub const ALT_CAMERA_X: u32 = 0x556;
    pub const ALT_CAMERA_Y: u32 = 0x55a;
}

/// Le jeu est en pause (`A5+0x3d`).
fn paused(x: &mut Cx) -> bool {
    x.a5_u8(0x3d) != 0
}

/// L'objet suivi, s'il existe (pointeur négatif), sans test d'activité.
fn scenery_target(x: &mut Cx) -> Option<Obj> {
    let p = x.u16(scenery::TARGET);
    ((p as i16) < 0).then(|| Obj(p as i16 as i32 as u32 & 0xff_ffff))
}

/// L'objet suivi, s'il existe et est actif.
fn scenery_live_target(x: &mut Cx) -> Option<Obj> {
    scenery_target(x).filter(|t| t.is_active(x.b))
}

fn reinsert_foreground(x: &mut Cx) {
    x.call(super::cx::sub::REINSERT_FOREGROUND);
}

fn scenery(x: &mut Cx) {
    let v = x.u8(f::VARIANT);
    match x.u8(f::STATE) {
        0 => match v {
            0 => flash_init(x),
            1 | 2 => thrown_init(x, v),
            9 => spray_init(x),
            3 => debris_init(x),
            4 => shard_init(x),
            5 => follow_init(x),
            6 => spark_init(x),
            7 => glow_init(x),
            8 => puff_init(x),
            10 => dust_init(x),
            _ => {}
        },
        2 => match v {
            0 => flash_active(x),
            1 | 2 => thrown_active(x, v),
            9 => spray_active(x),
            3 => debris_active(x),
            4 => shard_active(x),
            5 => follow_active(x),
            6 => spark_active(x),
            7 => glow_active(x),
            8 => puff_active(x),
            10 => dust_active(x),
            _ => {}
        },
        4 => x.add8(f::STATE, 2),
        6 => free(x),
        _ => {}
    }
}

/// Variantes 1 et 2 (0xb4194, 0xb4342) : posé sur l'objet suivi, décalé selon le réglage `+0x2f` ; la
/// variante 1 est lancée (vitesses de la fiche, données en `THROWS_1`), la variante 2 posée au sol et
/// repoussée avec les joueurs (`0x81aa4`). Disparaît à la fin de son animation.
fn thrown_init(x: &mut Cx, v: u8) {
    x.add8(f::STATE, 2);
    let Some(target) = scenery_target(x) else { return x.set8(f::STATE, 6) };
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let w = target.u16(x.b, off);
        x.set16(off, w);
    }
    let facing = target.u8(x.b, 0x2c);
    x.set8(0x2c, facing);
    let mode = x.u8(0x2f) as u32;
    if v == 1 {
        let at = scenery::THROWS_1.wrapping_add(((mode as u16) << 4) as i16 as i32 as u32);
        let (mut dx, dy, mut vx, mut ax) = (x.data16(at), x.data16(at + 2), x.data16(at + 4), x.data16(at + 6));
        if facing == 0 {
            dx = dx.wrapping_neg();
            vx = vx.wrapping_neg();
            ax = ax.wrapping_neg();
        }
        x.add16(Obj::X, dx);
        x.add16(Obj::Y, dy);
        x.set16(Obj::Z, 0);
        x.set16(Obj::VX, vx);
        x.set16(Obj::AX, ax);
        let (vy, ay) = (x.data16(at + 8), x.data16(at + 10));
        x.set16(Obj::VY, vy);
        x.set16(Obj::AY, ay);
        x.set32(Obj::VZ, 0);
    } else {
        let at = scenery::PLACES_2.wrapping_add(((mode as u16) << 2) as i16 as i32 as u32);
        let (mut dx, dy) = (x.data16(at), x.data16(at + 2));
        if facing == 0 {
            dx = dx.wrapping_neg();
        }
        x.add16(Obj::X, dx);
        x.add16(Obj::Y, dy);
        x.set16(Obj::Z, 0);
    }
    x.anim_start(scenery::ANIM_DUST, 0);
}

fn thrown_active(x: &mut Cx, v: u8) {
    if !paused(x) {
        if v == 1 {
            x.call(super::cx::sub::MOVE_XY);
        } else {
            x.call(scenery::PUSH_PLAYERS);
        }
        if x.anim_step() {
            return x.add8(f::STATE, 4);
        }
    }
    x.call(super::rom::OMBRE);
    x.reinsert();
}

/// Variante 9 (0xb49c4) : éclat projeté au hasard (vitesse x tirée, signe selon le réglage), qui retombe et
/// disparaît au sol ; il clignote une image sur deux.
fn spray_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.set16(Obj::AX, 0);
    x.set16(Obj::AY, 0);
    let k = 8 * (x.u8(0x2f) & 7) as u32;
    let dx = x.rom8(scenery::SPRAY + k) as i8 as i16 as u16;
    x.add16(Obj::X, dx);
    let dz = x.rom8(scenery::SPRAY + k + 1) as i8 as i16 as u16;
    x.add16(Obj::Z, dz);
    x.set32(Obj::VY, 0);
    let (vz, az) = (x.rom16(scenery::SPRAY + k + 4), x.rom16(scenery::SPRAY + k + 6));
    x.set16(Obj::VZ, vz);
    x.set16(Obj::AZ, az);
    x.call(super::rom::RNG);
    let mut vx = x.c.d[0] as u16 & 0x3ff;
    if x.u8(0x2f) & 1 != 0 {
        vx = vx.wrapping_neg();
    }
    x.set16(Obj::VX, vx);
    x.anim_start(scenery::ANIM_SPRAY, 0x19);
}

fn spray_active(x: &mut Cx) {
    if !paused(x) {
        x.anim_step();
        x.call(super::cx::sub::MOVE_XYZ);
        if (x.u16(Obj::Z) as i16) < 0 {
            x.set16(Obj::Z, 0);
            return x.add8(f::STATE, 4);
        }
        let blink = x.a5_u16(0x6e) & 1 != 0;
        x.set_bit(0x01, 1, blink);
    }
    x.reinsert();
}

/// Variante 0 (0xb411e) : un pixel devant, juste au-dessus du sol (ou au-dessous si `+0x2f`).
fn flash_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.add16(Obj::Y, 0xffff);
    x.set16(Obj::Z, 1);
    let anim = if x.u8(0x2f) != 0 {
        x.add16(Obj::Z, 0xfffe);
        x.add16(Obj::Y, 2);
        0x10
    } else {
        0xf
    };
    x.anim_start(scenery::ANIM_FLASH, anim);
}

fn flash_active(x: &mut Cx) {
    if !paused(x) && x.anim_step() {
        return scenery_done(x);
    }
    x.reinsert();
}

/// 0xb4bb2 : position et orientation de l'objet suivi.
fn copy_target(x: &mut Cx, target: Obj) {
    copy_position(x, target);
    let facing = target.u8(x.b, 0x2c);
    x.set8(0x2c, facing);
}

/// Fin d'animation : passe directement à la libération (`addq.b #2` deux fois).
fn scenery_done(x: &mut Cx) {
    x.add8(f::STATE, 4);
}

/// `subq.b #1` sur le compteur `+0x42` ; faux (et passage à la libération) quand il s'épuise.
fn scenery_countdown(x: &mut Cx) -> Option<u8> {
    let old = x.u8(0x42);
    if (old as i8) <= 1 {
        scenery_done(x);
        return None;
    }
    let n = old - 1;
    x.set8(0x42, n);
    Some(n)
}

// variante 3 : débris qui tombe et rebondit

fn debris_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    let Some(target) = scenery_target(x) else { return x.set8(f::STATE, 6) };
    copy_target(x, target);
    let mode = x.u8(0x2f) as u32;
    let (vz, az) = (x.data16(scenery::FALL_3 + 4 * mode), x.data16(scenery::FALL_3 + 4 * mode + 2));
    x.set16(Obj::VZ, vz);
    x.set16(Obj::AZ, az);
    let px = x.u16(Obj::X).wrapping_sub(0x80).wrapping_add((mode << 4) as u16);
    x.set16(Obj::X, px);
    let y = x.a5_u16(0x50a).wrapping_add(0x20);
    x.set16(Obj::Y, y);
    x.set16(Obj::Z, 0);
    x.anim_start(scenery::ANIM_DEBRIS, 0);
}

fn debris_active(x: &mut Cx) {
    if paused(x) {
        return reinsert_foreground(x);
    }
    x.call(super::cx::sub::MOVE_Z);
    if x.u8(f::SUB_STATE) == 0 {
        let vz = x.u16(Obj::VZ) as i16;
        if vz < 0 {
            // rebond : vitesse divisée par deux, freinage d'un quart
            x.add8(f::SUB_STATE, 2);
            x.set8(0x42, 0x20);
            let vz = vz >> 1;
            x.set16(Obj::VZ, vz as u16);
            x.set16(Obj::AZ, (vz.wrapping_neg() >> 2) as u16);
        }
        x.anim_step();
        return;
    }
    let Some(n) = scenery_countdown(x) else { return };
    set_hidden(x, n & 1 == 0);
    reinsert_foreground(x);
}

// variante 4 : éclat qui glisse puis clignote

fn shard_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    let Some(target) = scenery_target(x) else { return x.set8(f::STATE, 6) };
    copy_target(x, target);
    x.set16(Obj::Z, 0xffb0);
    x.call(super::rom::RNG);
    let k = x.c.d[0] & 0xe;
    let vx = x.rom16(scenery::DRIFT_4 + k);
    x.set16(Obj::VX, vx);
    x.set16(Obj::AX, 0);
    let mode = x.u8(0x2f) as u32;
    let (vz, az) = (x.data16(scenery::FALL_4 + 4 * mode), x.data16(scenery::FALL_4 + 4 * mode + 2));
    x.set16(Obj::VZ, vz);
    x.set16(Obj::AZ, az);
    let px = x.u16(Obj::X).wrapping_sub(0x40).wrapping_add((mode << 5) as u16);
    x.set16(Obj::X, px);
    let y = x.a5_u16(0x50a).wrapping_add(0x20);
    x.set16(Obj::Y, y);
    x.set16(Obj::Z, 0);
    x.anim_start(scenery::ANIM_DEBRIS, 3);
}

fn shard_active(x: &mut Cx) {
    if !paused(x) {
        x.call(super::cx::sub::MOVE_XZ);
        if x.u8(f::SUB_STATE) == 0 {
            if x.u8(0x33) != 0 {
                x.add8(f::SUB_STATE, 2);
                x.set8(0x42, 0x20);
                let vz = (x.u16(Obj::VZ) as i16) >> 2;
                x.set16(Obj::VZ, vz as u16);
                x.set16(Obj::AZ, 0);
            }
            x.anim_step();
        } else if scenery_countdown(x).is_none() {
            return;
        }
    }
    // une image sur deux selon la parité de `+0x58`
    let hidden = (x.a5_u16(0x6e) as u8 & 1) ^ x.u8(0x58) == 0;
    set_hidden(x, hidden);
    reinsert_foreground(x);
}

// variante 5 : caméra alternative qui suit un objet avec huit images de retard

/// 0xb4686 : remplit l'historique avec la position actuelle de l'objet suivi.
fn follow_fill(x: &mut Cx, target: Obj) {
    let (px, depth) = (target.u16(x.b, Obj::X), target.u16(x.b, Obj::Z).wrapping_add(target.u16(x.b, Obj::Y)));
    for k in 0..8 {
        x.set16(scenery::TRAIL + 4 * k, px);
        x.set16(scenery::TRAIL + 4 * k + 2, depth);
    }
}

/// 0xb4784 : caméra alternative au repos, puis libération.
fn follow_end(x: &mut Cx) {
    follow_rest(x);
    x.add8(f::STATE, 2);
}

fn follow_rest(x: &mut Cx) {
    let y = if x.a5_u8(0x34) == 0x2e { 0 } else { 0x500 };
    x.set_a5_u16(scenery::ALT_CAMERA_X, 0);
    x.set_a5_u16(scenery::ALT_CAMERA_Y, y);
}

fn follow_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.set8(scenery::TRAIL_WRITE, 7);
    x.set8(scenery::TRAIL_READ, 0);
    match scenery_target(x) {
        Some(target) if x.u8(0) != 0 => follow_fill(x, target),
        _ => follow_end(x),
    }
}

fn follow_active(x: &mut Cx) {
    if paused(x) {
        return;
    }
    let Some(target) = scenery_live_target(x) else { return follow_end(x) };
    // b46a2 : note la position actuelle ; b46cc : relit celle d'il y a huit images
    let w = (x.u8(scenery::TRAIL_WRITE) as u32 + 1) & 7;
    x.set8(scenery::TRAIL_WRITE, w as u8);
    let (px, depth) = (target.u16(x.b, Obj::X), target.u16(x.b, Obj::Z).wrapping_add(target.u16(x.b, Obj::Y)));
    x.set16(scenery::TRAIL + 4 * w, px);
    x.set16(scenery::TRAIL + 4 * w + 2, depth);
    let r = (x.u8(scenery::TRAIL_READ) as u32 + 1) & 7;
    x.set8(scenery::TRAIL_READ, r as u8);
    let (old_x, old_depth) = (x.u16(scenery::TRAIL + 4 * r), x.u16(scenery::TRAIL + 4 * r + 2));
    let (big_x, big_y) = if target.u8(x.b, 2) == 0x14 { (0x440, 0x400) } else { (0x230, 0x320) };
    let cam_x = x.a5_u16(0x506).wrapping_add(0xc0).wrapping_sub(old_x).wrapping_add(big_x);
    let mut cam_y = x.a5_u16(0x50a).wrapping_add(0x40).wrapping_sub(old_depth).wrapping_add(big_y);
    let limit: u16 = if x.a5_u8(0x34) == 0x2e { 0x2c0 } else { 0x3c0 };
    if cam_y as i16 > limit as i16 {
        cam_y = limit;
    }
    x.set_a5_u16(scenery::ALT_CAMERA_X, cam_x);
    x.set_a5_u16(scenery::ALT_CAMERA_Y, cam_y);
    if target.u8(x.b, 0x42) != 0 {
        follow_fill(x, target);
        follow_rest(x);
    }
}

// variante 6 : étincelle autour d'un objet

fn spark_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    let Some(target) = scenery_live_target(x) else { return x.set8(f::STATE, 6) };
    x.call(super::rom::RNG);
    let px = (x.c.d[0] as u16 & 0x1ff).wrapping_sub(0xff).wrapping_add(target.u16(x.b, Obj::X));
    x.set16(Obj::X, px);
    x.call(super::rom::RNG);
    let y = 0x1fu16.wrapping_sub(x.c.d[0] as u16 & 0x7f).wrapping_add(target.u16(x.b, Obj::Y));
    x.set16(Obj::Y, y);
    x.anim_start(scenery::ANIM_DUST, 0);
}

fn spark_active(x: &mut Cx) {
    if !paused(x) {
        if let Some(target) = scenery_live_target(x) {
            x.c.a[1] = target.0;
            x.call(0x930a4);
        }
        if x.anim_step() {
            return scenery_done(x);
        }
    }
    x.reinsert();
}

// variante 7 : lueur sur un objet de type 0x19 (selon son cadre `+0x33`)

fn glow_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    let Some(target) = scenery_live_target(x) else { return x.add8(f::STATE, 4) };
    let facing = target.u8(x.b, 0x2c);
    x.set8(0x2c, facing);
    x.anim_start(scenery::ANIM_GLOW, 0x20);
}

fn glow_active(x: &mut Cx) {
    if !paused(x) {
        let target = scenery_live_target(x)
            .filter(|t| t.u8(x.b, 2) == 0x19 && t.u16(x.b, 0xa0) == 0x225);
        let Some(target) = target else { return scenery_done(x) };
        set_hidden(x, true);
        let frame = target.u8(x.b, 0x33) as u32;
        if frame != 0 {
            let sprite = x.data32(scenery::GLOW_FRAMES + 4 * frame);
            x.set32(0x34, sprite);
            let (px, z, y) = (target.u16(x.b, Obj::X), target.u16(x.b, Obj::Z), target.u16(x.b, Obj::Y));
            x.set16(Obj::X, px);
            x.set16(Obj::Z, z.wrapping_add(9));
            x.set16(Obj::Y, y.wrapping_sub(1));
            set_hidden(x, false);
        }
    }
    x.reinsert();
}

// variante 8 : bouffée tirée au hasard dans une zone de l'écran

fn puff_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    let area = scenery::AREAS_8 + 8 * x.u8(0x2f) as u32;
    for (field, origin, k) in [(Obj::X, 0x506, 0), (Obj::Y, 0x50a, 4)] {
        let mut v = x.a5_u16(origin).wrapping_add(x.data16(area + k));
        x.call(super::rom::RNG);
        let mask = x.data16(area + k + 2);
        x.c.d[6] = x.c.d[6] & 0xffff_0000 | mask as u32;
        v = v.wrapping_add(x.c.d[0] as u16 & mask);
        x.call(super::rom::RNG);
        v = v.wrapping_add(x.c.d[0] as u16 & 0xf);
        x.set16(field, v);
    }
    x.call(super::rom::RNG);
    let d0 = x.c.d[0];
    x.anim_start(scenery::ANIM_DEBRIS, d0 & 0xffff_0000 | (d0 & 1) + 1);
}

fn puff_active(x: &mut Cx) {
    if !paused(x) && x.anim_step() {
        return scenery_done(x);
    }
    x.reinsert();
}

// variante 10 : poussière qui traverse l'écran ; le mode 0 crée les autres

fn dust_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    let mode = x.u8(0x2f) as u32;
    if mode == 0 {
        x.call(super::rom::RNG);
        let mut dx = x.c.d[0] as u16 & 0x1ff;
        if dx as i16 > 0x190 {
            dx -= 0x70;
        }
        let px = dx.wrapping_add(x.a5_u16(0x506));
        x.set16(Obj::X, px);
        x.call(super::rom::RNG);
        let y = (x.c.d[0] as u16 & 0x7f).wrapping_add(x.a5_u16(0x50a)).wrapping_add(0x20);
        x.set16(Obj::Y, y);
        x.set8(Obj::Z, 0);
        dust_spawn(x);
    }
    let (vx, ax) = (x.rom16(scenery::SPEEDS_10 + 4 * mode), x.rom16(scenery::SPEEDS_10 + 4 * mode + 2));
    x.set16(Obj::VX, vx);
    x.set16(Obj::AX, ax);
    x.set32(0x18, 0);
    x.set32(0x1c, 0);
    let anim = x.rom8(scenery::ANIMS_10 + mode) as u32;
    x.anim_start(scenery::ANIM_DUST, anim);
}

/// 0xb40ba : remplit le pool d'effets de poussières de mode 1 à la même position.
fn dust_spawn(x: &mut Cx) {
    while let Some(o) = alloc(x, ALLOC_EFFECT) {
        wr8(x.b, o.0, 1);
        wr8(x.b, o.0 + 2, 42);
        wr8(x.b, o.0 + 3, 10);
        wr8(x.b, o.0 + 0x2f, 1);
        for field in [Obj::X, Obj::Z, Obj::Y] {
            let v = x.u16(field);
            wr16(x.b, o.0 + field, v);
        }
    }
}

fn dust_active(x: &mut Cx) {
    if !paused(x) {
        if x.anim_step() {
            return scenery_done(x);
        }
        x.call(super::cx::sub::MOVE_XYZ);
        let dx: u16 = if x.a5_u16(0x6e) & 1 != 0 { 0xc0u16.wrapping_neg() } else { 0xc0 };
        x.add16(Obj::X, dx);
    }
    x.call(super::rom::DANS_LA_ZONE);
    x.call(super::rom::OMBRE);
    x.reinsert();
}

// ---------------------------------------------------------------------------------------------
// Type 88 : barrière commandée par des interrupteurs (type 87)
// ---------------------------------------------------------------------------------------------

mod barrier {
    pub const ANIM_SET: u32 = 0x1c03a6;
    /// Variante du décor SET 7 posé sous la barrière (octets, espace programme).
    pub const SET_VARIANTS: u32 = 0xba034;
    /// Variantes des interrupteurs, quatre octets par variante (données) ; une valeur négative arrête la liste.
    pub const SWITCHES: u32 = 0xba07c;
    pub const ALLOC_SET: u32 = 0x2348;
    pub const OPEN_SOUND: u32 = 0xeaf6a;
    /// Décor SET associé (pointeur de 16 bits).
    pub const SET: u32 = 0x46;
    /// Interrupteurs (jusqu'à quatre pointeurs de 16 bits) et leur nombre.
    pub const CHILDREN: u32 = 0x50;
    pub const COUNT: u32 = 0x58;
    pub const OPEN: u32 = 0x5a;
}

fn barrier(x: &mut Cx) {
    if x.u8(f::HIDDEN) != 0 {
        return;
    }
    match x.u8(f::STATE) {
        0 => barrier_init(x),
        2 => {
            if x.u8(f::SUB_STATE) == 0 {
                if barrier_all_pressed(x) {
                    x.add8(f::SUB_STATE, 2);
                    x.set8(barrier::OPEN, 1);
                    x.call(barrier::OPEN_SOUND);
                    x.set16(Obj::VZ, 0x180);
                    x.set16(Obj::VX, 0x200);
                }
            } else {
                // s'élève en tremblant, puis disparaît
                x.call(super::cx::sub::MOVE_XZ);
                let vx = x.u16(Obj::VX).wrapping_neg();
                x.set16(Obj::VX, vx);
                if (x.u16(Obj::Z) as i16) >= 0x80 {
                    let set = x.linked(barrier::SET);
                    set.set_u8(x.b, 0x2f, 0);
                    free(x);
                }
            }
        }
        4 => {}
        _ => free(x),
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

fn barrier_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    let v = x.u8(f::VARIANT) as u32;
    // le décor : l'original réessaie jusqu'à trouver une place
    let set = loop {
        if let Some(o) = alloc(x, barrier::ALLOC_SET) {
            break o;
        }
    };
    wr8(x.b, set.0, 1);
    wr8(x.b, set.0 + 2, 7);
    wr8(x.b, set.0 + 0x2f, 1);
    let sv = x.rom8(barrier::SET_VARIANTS + v);
    wr8(x.b, set.0 + 3, sv);
    x.set16(barrier::SET, set.0 as u16);
    // les interrupteurs
    let list = barrier::SWITCHES + 4 * v;
    let mut slot = barrier::CHILDREN;
    for k in 0..4 {
        if k > 0 && (x.data8(list + k) as i8) < 0 {
            break;
        }
        let variant = x.data8(list + k);
        if let Some(o) = alloc(x, ALLOC_EFFECT) {
            wr8(x.b, o.0, 1);
            wr8(x.b, o.0 + 2, 87);
            wr8(x.b, o.0 + 3, variant);
            wr16(x.b, o.0 + 0x42, x.o.0 as u16);
            x.set16(slot, o.0 as u16);
            slot += 2;
            x.add16(barrier::COUNT, 1);
        }
    }
    let anim = if matches!(v, 1 | 4) { 0x19 } else { 0x18 };
    x.anim_start(barrier::ANIM_SET, anim);
}

/// 0xba094 : tous les interrupteurs sont appuyés (bit 0 de `+0x30` de chacun).
fn barrier_all_pressed(x: &mut Cx) -> bool {
    let n = x.u16(barrier::COUNT);
    if n == 0 {
        return false;
    }
    let mut all = 1u8;
    for k in 0..n as u32 {
        let child = x.linked(barrier::CHILDREN + 2 * k);
        all &= child.u8(x.b, 0x30);
    }
    all != 0
}

// ---------------------------------------------------------------------------------------------
// Type 36 : objet flottant d'une scène d'introduction (trois variantes)
// ---------------------------------------------------------------------------------------------

mod floater {
    pub const ANIM_SET: u32 = 0x194062;
    /// Étape de la séquence de la scène (`A5-0x55c8`).
    pub const STEP: u32 = 0xff2a38;
    pub const TIMER: u32 = 0x54;
    /// Ondulation : période (images par pas, mot) et indice dans la table de vitesses (`+0x2d`).
    pub const WAVE_TICK: u32 = 0x56;
    /// Compte à rebours du tremblement de l'écran (`A5+0x4ba`).
    pub const SHAKE: u32 = 0x58;
}

fn floater(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let anim = match x.u8(f::VARIANT) {
                0 => 0,
                1 => 2,
                _ => {
                    x.set16(Obj::X, 0x120);
                    x.set16(Obj::Y, 0x198);
                    x.set16(Obj::Z, 8);
                    1
                }
            };
            x.anim_start(floater::ANIM_SET, anim);
            x.reinsert();
        }
        2 => match x.u8(f::VARIANT) {
            0 => floater_main(x),
            1 => {
                // reflet collé sous son créateur
                if x.a5_u8(0x42) != 0 {
                    x.add8(f::STATE, 2);
                }
                let creator = x.linked(0x5c);
                let z = creator.u16(x.b, Obj::Z).wrapping_add(0x3e);
                x.set16(Obj::Z, z);
                x.anim_step();
            }
            _ => floater_rise(x),
        },
        _ => free(x),
    }
}

/// Ondulation verticale : la vitesse en Z suit la table de vitesses courante, un pas toutes les quatre images.
fn floater_wave(x: &mut Cx) {
    let k = x.u8(0x2d) as u32;
    let table = x.u32(Obj::SPEED_TABLE);
    let v = x.data16(table + 4 * k + 2);
    x.set16(Obj::VZ, v);
    let dz = ((v as u32) << 16) as i32 >> 8;
    x.add32(Obj::Z, dz as u32);
    let t = x.u16(floater::WAVE_TICK).wrapping_sub(1) & 3;
    x.set16(floater::WAVE_TICK, t);
    if t == 0 {
        x.set16(floater::WAVE_TICK, 4);
        let k = (x.u8(0x2d) + 1) & 0x1f;
        x.set8(0x2d, k);
    }
    x.anim_step();
}

fn floater_start_wave(x: &mut Cx) {
    x.set32(Obj::VZ, 0x100_0000);
    x.set16(floater::WAVE_TICK, 4);
    x.set8(0x2d, 0);
    x.c.d[0] = 1;
    x.call(super::cx::sub::SPEED_TABLE_B);
}

fn floater_main(x: &mut Cx) {
    match x.u8(f::SUB_STATE) {
        0 => {
            if x.a5_u8(0x2e) == 6 {
                x.add8(f::SUB_STATE, 2);
                x.set16(floater::TIMER, 0xc);
                x.anim_start(floater::ANIM_SET, 1);
            } else {
                x.anim_step();
            }
        }
        2 => {
            x.add16(Obj::Z, 1);
            let t = x.u16(floater::TIMER).wrapping_sub(1);
            x.set16(floater::TIMER, t);
            if t != 0 {
                x.anim_step();
                return;
            }
            x.add8(f::SUB_STATE, 2);
            floater_start_wave(x);
            // 0xb337e : le reflet (type 36 variante 1)
            if let Some(o) = alloc(x, ALLOC_EFFECT) {
                wr8(x.b, o.0, 1);
                wr8(x.b, o.0 + 2, 36);
                wr8(x.b, o.0 + 3, 1);
                for field in [Obj::X, Obj::Z, Obj::Y] {
                    let v = x.u16(field);
                    wr16(x.b, o.0 + field, v);
                }
                let z = rd16(x.b, o.0 + Obj::Z).wrapping_add(0x3e);
                wr16(x.b, o.0 + Obj::Z, z);
                x.set16(0x12, 0x8000);
                wr16(x.b, o.0 + 0x5c, x.o.0 as u16);
            }
        }
        4 => {
            if rd8(x.b, floater::STEP) == 0x18 {
                x.add8(f::SUB_STATE, 2);
                x.set16(floater::TIMER, 0x80);
                x.anim_start(floater::ANIM_SET, 3);
            } else {
                floater_wave(x);
            }
        }
        6 => {
            let t = x.u16(floater::TIMER).wrapping_sub(1);
            x.set16(floater::TIMER, t);
            if t == 0 {
                x.add8(f::SUB_STATE, 2);
                x.anim_start(floater::ANIM_SET, 1);
                return;
            }
            if x.u8(floater::SHAKE) != 0 {
                let s = x.u8(floater::SHAKE) - 1;
                x.set8(floater::SHAKE, s);
                if s == 0 {
                    x.set_a5_u16(0x4ba, 0);
                }
            }
            if x.u8(0x33) != 0 {
                // touché : secousse de l'écran
                x.set8(0x33, 0);
                x.call(0x3af8);
                x.call(0xb7dec);
                x.call(0xe637c);
                x.set_a5_u16(0x4ba, 0x100);
                x.set8(floater::SHAKE, 8);
                x.call(0x1ac86);
            }
            x.anim_step();
        }
        8 => {
            if rd8(x.b, floater::STEP) == 0x20 {
                x.add8(f::SUB_STATE, 2);
                x.set16(floater::TIMER, 0xf0);
            }
            floater_wave(x);
        }
        10 => fade_out(x, floater::TIMER),
        _ => {}
    }
}

/// Disparition : clignote de plus en plus vite pendant le compte à rebours `timer`, puis passe à la
/// libération.
fn fade_out(x: &mut Cx, timer: u32) {
    let t = x.u16(timer).wrapping_sub(1);
    x.set16(timer, t);
    if t == 0 {
        return x.add8(f::STATE, 2);
    }
    let mask = if t as i16 > 0x80 {
        7
    } else if t as i16 > 0x30 {
        3
    } else {
        1
    };
    set_hidden(x, t & mask == 0);
}

fn floater_rise(x: &mut Cx) {
    match x.u8(f::SUB_STATE) {
        0 => {
            x.add8(f::SUB_STATE, 2);
            floater_start_wave(x);
            x.anim_step();
        }
        2 => {
            if x.a5_u8(0x2e) == 0xc {
                x.add8(f::SUB_STATE, 2);
            }
            floater_wave(x);
        }
        4 => {
            let hidden = x.a5_u8(0x6f) & 1 != 0;
            set_hidden(x, hidden);
            x.add16(Obj::Z, 4);
            if x.u16(Obj::Z) > 0x70 {
                x.add8(f::STATE, 2);
            }
            x.anim_step();
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Type 49 : accessoire d'un porteur, qui tombe quand le porteur passe dans un état donné
// ---------------------------------------------------------------------------------------------

mod prop {
    pub const ANIM_SET: u32 = 0x1961be;
    pub const CARRIER: u32 = 0x42;
}

/// 0xb5f66 / 0xb5f8e : posé à `dx` du porteur, 0x45 plus haut, juste devant lui.
fn prop_place(x: &mut Cx, dx: u16) {
    let carrier = x.linked(prop::CARRIER);
    let (cx_, cz) = (carrier.u16(x.b, Obj::X), carrier.u16(x.b, Obj::Z));
    x.set16(Obj::X, cx_.wrapping_add(dx));
    x.set16(Obj::Z, cz.wrapping_sub(0x45));
    let depth = carrier.u32(x.b, Obj::Y).wrapping_add(1);
    x.set32(Obj::Y, depth);
}

/// États (`+4..+7`) du porteur.
fn carrier_state(x: &mut Cx) -> u32 {
    let carrier = x.linked(prop::CARRIER);
    carrier.u32(x.b, f::STATE)
}

/// Chute : profondeur et x en 16.16, jusqu'à 0xf0 sous le haut de l'écran.
fn prop_fall(x: &mut Cx, dz: u32, dx: u32, alt_camera: bool) {
    x.add32(Obj::Z, dz);
    x.add32(Obj::X, dx.wrapping_neg());
    let top = if alt_camera && x.a5_u8(0x36) != 0 { x.a5_u16(0x5b2) } else { x.a5_u16(0x5aa) };
    if (top.wrapping_add(0xf0) as i16) <= (x.u16(Obj::Z) as i16) {
        return free(x);
    }
    x.anim_step();
}

fn prop(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            prop_place(x, 0xffd0);
            let anim = (x.u8(f::VARIANT) != 0) as u32;
            x.anim_start(prop::ANIM_SET, anim);
        }
        2 => prop_active(x),
        _ => free(x),
    }
    if x.u8(0) != 0 {
        if x.u8(f::VARIANT) != 0 {
            x.reinsert();
        } else {
            x.call(super::cx::sub::REINSERT_FOREGROUND);
        }
    }
}

fn prop_active(x: &mut Cx) {
    let sub = x.u8(f::SUB_STATE);
    match (x.u8(f::VARIANT), sub) {
        (0, 0) => {
            if carrier_state(x) == 0x0200_000c {
                return free(x);
            }
            prop_place(x, 0xffd0);
            x.anim_step();
        }
        (1, 0) => {
            if matches!(carrier_state(x), 0x0200_0608 | 0x0200_0808) {
                x.add8(f::SUB_STATE, 2);
            } else {
                prop_place(x, 0x30);
            }
            x.anim_step();
        }
        (1, 2) => {
            if matches!(carrier_state(x), 0x0200_060c | 0x0200_080c) {
                x.add8(f::SUB_STATE, 2);
                x.call(0xe4e5a);
            }
            x.anim_step();
        }
        (1, 4) => prop_fall(x, 0x1_8000, 0x8000, true),
        (2, 0) => {
            if carrier_state(x) == 0x0200_0008 {
                x.add8(f::SUB_STATE, 2);
            } else {
                prop_place(x, 0x30);
            }
            x.anim_step();
        }
        (2, 2) => {
            if carrier_state(x) == 0x0200_000c {
                x.add8(f::SUB_STATE, 2);
                x.call(0xe4e5a);
            }
            x.anim_step();
        }
        (2, 4) => prop_fall(x, 0x8000, 0x1_8000, false),
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Types 82 et 83 : éclat posé sur un objet ; pluie d'éclats
// ---------------------------------------------------------------------------------------------

mod shards {
    pub const ANIM_SPARK: u32 = 0x1b670a;
    pub const ANIM_RAIN: u32 = 0x1bb058;
    /// Décalages (x, z) de l'éclat par paire de variantes du porteur (espace programme).
    pub const SPARK_OFFSETS: u32 = 0xb98b6;
    /// Décalages x des gouttes, 16 mots par sens (espace programme).
    pub const RAIN_OFFSETS: u32 = 0xb9a2a;
    pub const OWNER: u32 = 0x5c;
    pub const BLINK: u32 = 0x50;
    pub const TIMER: u32 = 0x58;
    pub const COUNT: u32 = 0x5e;
}

/// Type 82 : éclat posé à côté de son créateur, qui clignote jusqu'à la fin de son action.
fn spark(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set8(shards::BLINK, 0x30);
            let owner = x.linked(shards::OWNER);
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            copy_position(x, owner);
            let (dx, dz) = if x.u8(f::VARIANT) == 0 {
                (0x10, 5)
            } else {
                let k = ((owner.u8(x.b, f::VARIANT) as u32) >> 1) << 2;
                (x.rom16(shards::SPARK_OFFSETS + k), x.rom16(shards::SPARK_OFFSETS + k + 2))
            };
            let dx = if x.u8(0x2c) != 0 { dx.wrapping_neg() } else { dx };
            x.add16(Obj::X, dx);
            x.add16(Obj::Z, dz);
            x.add16(Obj::Y, 0xffff);
            x.anim_start(shards::ANIM_SPARK, 0);
            x.call(0x3af8);
        }
        2 => {
            x.anim_step();
            let owner = x.linked(shards::OWNER);
            let done = if x.u8(f::VARIANT) == 0 {
                if owner.u8(x.b, 0x58) != 0 {
                    true
                } else {
                    let n = x.u8(shards::BLINK).wrapping_sub(1);
                    x.set8(shards::BLINK, n);
                    n == 0
                }
            } else {
                owner.u8(x.b, 6) != 0x26
            };
            if done {
                x.add8(f::STATE, 2);
            }
            let odd = x.u8(shards::BLINK) & 1 != 0;
            set_hidden(x, odd);
        }
        _ => return free(x),
    }
    x.reinsert();
}

/// Type 83 : la variante 0 crée pendant 0xe6 images une goutte (variante 1) par image, qui tombe en
/// arc depuis le haut de l'écran.
fn shard_rain(x: &mut Cx) {
    let v = x.u8(f::VARIANT);
    match x.u8(f::STATE) {
        0 if v == 0 => {
            x.add8(f::STATE, 2);
            x.set16(shards::TIMER, 0xe6);
            x.call(0x1ac7c);
        }
        0 => {
            // A1 : adresse du gestionnaire, laissée par la boucle de dispatch (octet `+0x2c` constant)
            let handler = Obj(x.c.a[1]);
            x.add8(f::STATE, 2);
            x.set16(Obj::VX, 0xfc00);
            x.set16(Obj::VZ, 0xf900);
            let (cx_, cy) = (x.a5_u16(0x506), x.a5_u16(0x50a));
            x.set16(Obj::X, cx_);
            x.set16(Obj::Y, cy.wrapping_add(0x18));
            x.set16(Obj::Z, 0xe8);
            let r = random_nibble16(x) & 0x1f;
            x.add16(Obj::Z, r);
            let mut k = (x.u8(shards::COUNT) & 0xf) as u32 * 2;
            if handler.u8(x.b, 0x2c) != 0 {
                let vx = x.u16(Obj::VX).wrapping_neg();
                x.set16(Obj::VX, vx);
                k += 0x20;
            }
            let base = x.rom16(shards::RAIN_OFFSETS + k);
            let r = random_nibble16(x) & 3;
            x.add16(Obj::X, base.wrapping_add(r));
            x.call(super::rom::RNG);
            let d0 = x.c.d[0];
            x.anim_start(shards::ANIM_RAIN, d0 & 0xffff_0000 | (d0 & 1) + 1);
            x.reinsert();
        }
        2 if v == 0 => {
            let t = x.u16(shards::TIMER).wrapping_sub(1);
            x.set16(shards::TIMER, t);
            if t == 0 {
                x.add8(f::STATE, 2);
            }
            // 0xb9956 : la goutte suivante
            if let Some(o) = alloc(x, ALLOC_EFFECT) {
                wr8(x.b, o.0, 1);
                wr8(x.b, o.0 + 2, 83);
                wr8(x.b, o.0 + 3, 1);
                let n = x.u8(shards::COUNT);
                wr8(x.b, o.0 + 0x5e, n);
                x.add8(shards::COUNT, 1);
            }
        }
        2 => {
            x.call(super::cx::sub::MOVE_XZ);
            if (x.u16(Obj::Z) as i16) < 0 {
                x.add8(f::STATE, 2);
            }
        }
        _ => free(x),
    }
}

/// Tirage du générateur, mot bas.
fn random_nibble16(x: &mut Cx) -> u16 {
    x.call(super::rom::RNG);
    x.c.d[0] as u16
}

// ---------------------------------------------------------------------------------------------
// Type 58 : machine de la scène d'introduction (variante 0) et son reflet (variante 1)
// ---------------------------------------------------------------------------------------------

mod machine {
    pub const ANIM_SET: u32 = 0x196baa;
    pub const TIMER: u32 = 0x54;
}

fn machine(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let anim = if x.u8(f::VARIANT) != 0 { 3 } else { 0 };
            x.anim_start(machine::ANIM_SET, anim);
            x.reinsert();
        }
        2 if x.u8(f::VARIANT) != 0 => {
            if x.a5_u8(0x42) != 0 {
                x.add8(f::STATE, 2);
            }
            x.anim_step();
        }
        2 => machine_active(x),
        _ => free(x),
    }
}

fn machine_active(x: &mut Cx) {
    match x.u8(f::SUB_STATE) {
        0 => {
            if x.a5_u8(0x2e) == 6 {
                x.add8(f::SUB_STATE, 2);
                x.anim_start(machine::ANIM_SET, 2);
            } else {
                x.anim_step();
            }
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB_STATE, 2);
                x.set16(machine::TIMER, 0x40);
                x.anim_start(machine::ANIM_SET, 0);
                // un objet BOX 0x12 ; l'original ne teste pas l'échec de l'allocation
                x.call(0x23a8);
                let o = x.c.a[1];
                wr8(x.b, o, 1);
                wr8(x.b, o + 2, 0x12);
            }
        }
        4 => {
            x.anim_step();
            let t = x.u16(machine::TIMER).wrapping_sub(1);
            x.set16(machine::TIMER, t);
            if t == 0 {
                x.add8(f::SUB_STATE, 2);
                x.anim_start(machine::ANIM_SET, 1);
            }
        }
        6 => {
            if x.anim_step() {
                x.add8(f::SUB_STATE, 2);
                x.anim_start(machine::ANIM_SET, 0);
                // 0xb6eb0 : le reflet (type 58 variante 1)
                if let Some(o) = alloc(x, ALLOC_EFFECT) {
                    wr8(x.b, o.0, 1);
                    wr8(x.b, o.0 + 2, 58);
                    wr8(x.b, o.0 + 3, 1);
                    for field in [Obj::X, Obj::Z, Obj::Y] {
                        let v = x.u16(field);
                        wr16(x.b, o.0 + field, v);
                    }
                    let px = rd16(x.b, o.0 + Obj::X).wrapping_sub(6);
                    wr16(x.b, o.0 + Obj::X, px);
                    let pz = rd16(x.b, o.0 + Obj::Z).wrapping_add(0x1c);
                    wr16(x.b, o.0 + Obj::Z, pz);
                    x.set16(0x12, 0x8000);
                    wr16(x.b, o.0 + 0x5c, x.o.0 as u16);
                }
            } else if x.u8(0x33) != 0 {
                x.set8(0x33, 0);
                x.set16(Obj::X, 0x130);
                x.set16(Obj::Y, 0x180);
            }
        }
        8 => {
            if rd8(x.b, floater::STEP) == 0x20 {
                x.add8(f::SUB_STATE, 2);
                x.set16(machine::TIMER, 0xf0);
            }
            x.anim_step();
        }
        10 => fade_out(x, machine::TIMER),
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Type 16 : double d'un objet porteur, qui glisse jusqu'à sa place et suit les étapes du porteur
// ---------------------------------------------------------------------------------------------

mod double {
    /// Par variante : x d'arrivée (mots), direction initiale et table de vitesses (octets), espace
    /// programme.
    pub const TARGETS: u32 = 0xae76e;
    pub const DIRECTIONS: u32 = 0xae756;
    pub const SPEEDS: u32 = 0xae75a;
    pub const CARRIER: u32 = 0x42;
    pub const TARGET_X: u32 = 0x58;
}

/// 0xae748 + 0x1b64 : table de vitesses de la variante.
fn double_speed_table(x: &mut Cx) {
    let v = x.u8(f::VARIANT) as u32;
    x.c.d[0] = x.rom8(double::SPEEDS + v) as u32;
    x.c.d[1] = v;
    x.call(super::cx::sub::SPEED_TABLE_B);
}

/// Recopie le cadre de sprite du porteur (et sa hauteur si `with_z`).
fn double_copy_frame(x: &mut Cx, with_flip: bool, with_z: bool) {
    let carrier = x.linked(double::CARRIER);
    let frame = carrier.u32(x.b, 0x34);
    x.set32(0x34, frame);
    if with_flip {
        let b = carrier.u8(x.b, 0x3b);
        x.set8(0x3b, b);
    }
    if with_z {
        let z = carrier.u16(x.b, Obj::Z);
        x.set16(Obj::Z, z);
    }
}

/// Déplacement en x et y ; vrai quand le x d'arrivée est atteint (dans le sens de la marche).
fn double_slide(x: &mut Cx) -> bool {
    x.call(super::cx::sub::MOVE_XY);
    let (mut a, mut b) = (x.u16(Obj::X) as i16, x.u16(double::TARGET_X) as i16);
    if (x.u8(Obj::SPEED_INDEX) as i8) >= 0x10 {
        std::mem::swap(&mut a, &mut b);
    }
    b <= a
}

/// 0xae700 : crée l'objet suivant (type 39), en réessayant jusqu'à trouver une place.
fn double_spawn(x: &mut Cx) {
    let o = loop {
        if let Some(o) = alloc(x, ALLOC_EFFECT) {
            break o;
        }
    };
    wr8(x.b, o.0, 1);
    wr8(x.b, o.0 + 2, 39);
    let v = x.u8(f::VARIANT);
    wr8(x.b, o.0 + 3, v);
    let carrier = x.u16(double::CARRIER);
    wr16(x.b, o.0 + 0x42, carrier);
    let facing = x.u8(0x2c);
    wr8(x.b, o.0 + 0x2c, facing);
    let (px, py) = (x.u16(Obj::X), x.u16(Obj::Y));
    wr16(x.b, o.0 + Obj::X, px);
    wr16(x.b, o.0 + Obj::Y, py);
    wr16(x.b, o.0 + Obj::Z, 0x66);
}

fn double(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let v = x.u8(f::VARIANT) as u32;
            let target = x.rom16(double::TARGETS + 2 * v);
            x.set16(double::TARGET_X, target);
            let dir = x.rom8(double::DIRECTIONS + v);
            x.set8(Obj::SPEED_INDEX, dir);
            double_speed_table(x);
            let carrier = x.linked(double::CARRIER);
            copy_position(x, carrier);
            let frame = carrier.u32(x.b, 0x34);
            x.set32(0x34, frame);
            x.set8(0x3b, 0);
            for off in [0x3c, 0x3f] {
                let b = carrier.u8(x.b, off);
                x.set8(off, b);
            }
        }
        2 => double_active(x),
        _ => return free(x),
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

fn double_active(x: &mut Cx) {
    let sub = x.u8(f::SUB_STATE);
    // étapes 2 à 8 : attend que le porteur atteigne l'état attendu
    const AWAITED: [(u8, u32); 4] = [(2, 0x0200_0412), (4, 0x0200_0420), (6, 0x0200_0424), (8, 0x0200_0428)];
    match sub {
        0 => {
            toggle_hidden(x);
            if !double_slide(x) {
                return double_copy_frame(x, false, false);
            }
            x.add8(f::SUB_STATE, 2);
            set_hidden(x, false);
            let target = x.u16(double::TARGET_X);
            x.set16(Obj::X, target);
            double_copy_frame(x, true, false);
        }
        10 => {
            toggle_hidden(x);
            let carrier = x.linked(double::CARRIER);
            let (cx_, cy) = (carrier.u16(x.b, Obj::X), carrier.u16(x.b, Obj::Y));
            x.c.d[1] = x.c.d[1] & 0xffff_0000 | cx_ as u32;
            x.c.d[2] = x.c.d[2] & 0xffff_0000 | cy as u32;
            x.call(0x137e);
            let dir = x.c.d[0] as u8;
            x.set8(Obj::SPEED_INDEX, dir);
            double_speed_table(x);
            if double_slide(x) {
                return free(x);
            }
            double_copy_frame(x, true, true);
        }
        _ => {
            let Some(&(_, awaited)) = AWAITED.iter().find(|(s, _)| *s == sub) else { return };
            let carrier = x.linked(double::CARRIER);
            if carrier.u32(x.b, f::STATE) == awaited {
                x.add8(f::SUB_STATE, 2);
                match sub {
                    2 | 4 => double_spawn(x),
                    8 => {
                        let cx_ = carrier.u16(x.b, Obj::X);
                        x.set16(double::TARGET_X, cx_);
                    }
                    _ => {}
                }
            }
            double_copy_frame(x, true, sub == 8);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 12 : maillon d'une chaîne, qui rejoue avec retard les directions du maillon précédent
// ---------------------------------------------------------------------------------------------

mod link {
    pub const ANIM_SET: u32 = 0x15cdb6;
    /// Retard (indice dans l'historique du précédent) par rang `+0x2f` (octets, espace programme).
    pub const DELAYS: u32 = 0xad536;
    pub const PREVIOUS: u32 = 0x5c;
    /// La tête de la chaîne : son octet `+0x58` vaut 0x80 (fin) ou 0xfe (rupture), `+0x42` compte les
    /// maillons.
    pub const HEAD: u32 = 0x46;
    /// Historique des directions (4 octets, le plus récent en premier).
    pub const HISTORY: u32 = 0x58;
    /// Rupture : fiches de 16 octets par type du porteur de la tête (données).
    pub const BREAKS: u32 = 0xad5e2;
}

fn chain_link(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let prev = x.linked(link::PREVIOUS);
            for off in [Obj::X, Obj::Z, Obj::Y, Obj::SPEED_TABLE] {
                let v = prev.u32(x.b, off);
                x.set32(off, v);
            }
            let facing = prev.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            x.set32(link::HISTORY, 0xffff_ffff);
            let anim = 1u8.wrapping_add(x.u8(0x2f));
            x.anim_start(link::ANIM_SET, anim as u32);
            x.reinsert();
        }
        2 if x.u8(f::SUB_STATE) != 0 => {
            if x.anim_step() {
                free(x);
            }
        }
        2 => chain_link_follow(x),
        4 => {
            // A1 : adresse du gestionnaire laissée par le dispatch (écriture en ROM, sans effet)
            let a1 = x.c.a[1];
            let n = rd8(x.b, a1 + 0x42).wrapping_sub(1);
            wr8(x.b, a1 + 0x42, n);
            free(x);
        }
        _ => free(x),
    }
}

/// `0xad53a` : la chaîne se rompt. Chaque maillon (rang `+0x2f` décrémenté) joue son animation de rupture ;
/// le dernier disparaît. Une fiche par type du porteur de la tête (`+0x54` de la tête ; 0x20 pour un porteur
/// qui n'est pas un ENEMY) dit si le maillon disparaît (octet négatif), reste en place (positif) ou se pose
/// sur le porteur (nul). L'original relit toujours la première entrée de la fiche (son indice est remis à zéro).
fn chain_break(x: &mut Cx, head: Obj) {
    x.add8(f::SUB_STATE, 2);
    let n = head.u8(x.b, 0x42).wrapping_sub(1);
    head.set_u8(x.b, 0x42, n);
    let rank = x.u8(0x2f).wrapping_sub(1);
    x.set8(0x2f, rank);
    if (rank as i8) < 0 {
        x.add8(f::STATE, 2);
        return free(x);
    }
    let p = head.u16(x.b, 0x54);
    let carrier = Obj(p as i16 as i32 as u32 & 0xff_ffff);
    let row = if carrier.u8(x.b, Obj::CLASS) == 2 { carrier.u8(x.b, Obj::KIND) as u16 } else { 0x20 };
    let sheet = link::BREAKS.wrapping_add((row << 4) as i16 as i32 as u32);
    let what = x.data8(sheet) as i8;
    if what < 0 {
        return free(x);
    }
    if what == 0 {
        let (cx_, cz, cy) = (carrier.u16(x.b, Obj::X), carrier.u16(x.b, Obj::Z), carrier.u32(x.b, Obj::Y));
        x.set16(Obj::X, cx_);
        x.set16(Obj::Z, cz);
        x.set32(Obj::Y, cy.wrapping_sub(1));
        let mut d = x.data16(sheet + 4 + 2);
        x.add16(Obj::Z, d);
        let facing = carrier.u8(x.b, 0x2c);
        x.set8(0x2c, facing);
        if facing != 0 {
            d = d.wrapping_neg();
        }
        x.add16(Obj::X, d);
        x.set8(0x2c, facing ^ 0x20);
    }
    let anim = 5u8.wrapping_add(x.u8(0x2f));
    x.anim_start(link::ANIM_SET, anim as u32);
}

fn chain_link_follow(x: &mut Cx) {
    let head = x.linked(link::HEAD);
    match head.u8(x.b, 0x58) {
        0x80 => {
            let n = head.u8(x.b, 0x42).wrapping_sub(1);
            head.set_u8(x.b, 0x42, n);
            return free(x);
        }
        0xfe => return chain_break(x, head),
        _ => {}
    }
    for k in (0..3).rev() {
        let b = x.u8(link::HISTORY + k);
        x.set8(link::HISTORY + k + 1, b);
    }
    let prev = x.linked(link::PREVIOUS);
    let rank = x.u8(0x2f) as u32;
    let delay = x.rom8(link::DELAYS + rank) as u32;
    let dir = prev.u8(x.b, link::HISTORY + delay);
    x.set8(Obj::SPEED_INDEX, dir);
    x.set8(link::HISTORY, dir);
    if (dir as i8) >= 0 {
        // bits 5 et 6 : saut d'un étage vers le haut ou le bas
        if dir & 0x20 != 0 {
            x.add32(Obj::Z, 0x3_0000);
            x.set8(Obj::SPEED_INDEX, dir & 0x1f);
        } else if dir & 0x40 != 0 {
            x.add32(Obj::Z, 0x3_0000u32.wrapping_neg());
            x.set8(Obj::SPEED_INDEX, dir & 0x1f);
        }
        x.call(super::cx::sub::MOVE_TABLE_XY);
    }
    x.anim_step();
    x.reinsert();
}

// ---------------------------------------------------------------------------------------------
// Type 30 : poissons de la scène d'introduction (variante 0 : banc qui traverse ; autres : poisson
// qui nage, saute quand l'écran tremble)
// ---------------------------------------------------------------------------------------------

mod fish {
    pub const ANIM_SET: u32 = 0x190a20;
    pub const TIMER: u32 = 0x58;
}

/// Vitesse de nage tirée au hasard (0x100 ou 0x180), vers la gauche si `left`.
fn fish_speed(x: &mut Cx, left: bool) {
    let r = random_nibble16(x);
    let v: u16 = if r & 1 != 0 { 0x180 } else { 0x100 };
    x.set16(Obj::VX, if left { v.wrapping_neg() } else { v });
}

fn fish(x: &mut Cx) {
    let v = x.u8(f::VARIANT);
    match x.u8(f::STATE) {
        0 if v == 0 => {
            let t = x.u16(fish::TIMER).wrapping_sub(1);
            x.set16(fish::TIMER, t);
            if t != 0 {
                return;
            }
            x.add8(f::STATE, 2);
            x.set16(fish::TIMER, 4);
            x.set8(Obj::SPEED_INDEX, 0);
            x.c.d[0] = 1;
            x.call(super::cx::sub::SPEED_TABLE_B);
            x.anim_start(fish::ANIM_SET, 6);
            x.reinsert();
        }
        0 => {
            x.add8(f::STATE, 2);
            x.set16(fish::TIMER, 0xa0);
            let left = x.u8(0x2c) == 0;
            fish_speed(x, left);
            x.anim_start(fish::ANIM_SET, 1);
            x.reinsert();
        }
        2 if v == 0 => {
            x.add16(Obj::X, 2);
            // ondulation en profondeur, comme le type 36 mais sans vitesse notée
            let k = x.u8(Obj::SPEED_INDEX) as u32;
            let table = x.u32(Obj::SPEED_TABLE);
            let w = x.data16(table + 4 * k + 2);
            x.add32(Obj::Y, (((w as u32) << 16) as i32 >> 8) as u32);
            let t = x.u16(fish::TIMER).wrapping_sub(1) & 3;
            x.set16(fish::TIMER, t);
            if t == 0 {
                x.set16(fish::TIMER, 4);
                let k = (x.u8(Obj::SPEED_INDEX) + 1) & 0x1f;
                x.set8(Obj::SPEED_INDEX, k);
            }
            if (x.a5_u16(0x506).wrapping_add(0x1c0) as i16) <= (x.u16(Obj::X) as i16) {
                x.add8(f::STATE, 2);
            }
            x.anim_step();
        }
        2 => fish_swim(x),
        4 => x.add8(f::STATE, 2),
        _ => free(x),
    }
}

fn fish_swim(x: &mut Cx) {
    match x.u8(f::SUB_STATE) {
        0 => {
            let t = x.u16(fish::TIMER).wrapping_sub(1);
            x.set16(fish::TIMER, t);
            if t == 0 {
                x.add8(f::SUB_STATE, 2);
                x.set16(fish::TIMER, 0x60);
                return x.anim_start(fish::ANIM_SET, 0);
            }
            x.call(super::cx::sub::MOVE_X);
            // demi-tour au bord de la caméra alternative
            let cam = x.a5_u16(0x556);
            let px = x.u16(Obj::X);
            let turn = if (x.u16(Obj::VX) as i16) < 0 {
                (cam.wrapping_add(0x40).wrapping_sub(px) as i16) >= 0
            } else {
                (cam.wrapping_add(0x140).wrapping_sub(px) as i16) < 0
            };
            if turn {
                let r = random_nibble16(x);
                let v: u16 = if r & 1 != 0 { 0x180 } else { 0x100 };
                let facing = x.u8(0x2c) ^ 0x20;
                x.set8(0x2c, facing);
                x.set16(Obj::VX, if facing == 0 { v.wrapping_neg() } else { v });
            }
            x.anim_step();
        }
        2 => {
            if x.a5_u8(0x2e) == 6 {
                x.add8(f::SUB_STATE, 2);
                x.set8(0x2c, 0x20);
                x.anim_start(fish::ANIM_SET, 2);
            }
        }
        4 => {
            if x.a5_u8(0x2e) == 8 {
                x.add8(f::SUB_STATE, 2);
                x.anim_start(fish::ANIM_SET, 0);
            }
        }
        6 => {
            if x.a5_u16(0x4ba) == 0 {
                return;
            }
            // l'écran tremble : saut
            x.add8(f::SUB_STATE, 2);
            if random_nibble16(x) & 1 != 0 {
                let facing = x.u8(0x2c) ^ 0x20;
                x.set8(0x2c, facing);
            }
            x.set16(Obj::VZ, 0x400);
            x.set16(Obj::AZ, 0xffc8);
            let vx: u16 = if x.u8(0x2c) != 0 { 0x200u16.wrapping_neg() } else { 0x200 };
            x.set16(Obj::VX, vx);
            x.anim_start(fish::ANIM_SET, 4);
        }
        8 => {
            x.call(super::cx::sub::MOVE_XZ);
            if (x.u16(Obj::VZ) as i16) < 0 && (x.u16(Obj::Z) as i16) <= -0x10 {
                x.add8(f::SUB_STATE, 2);
                x.set32(Obj::Z, 0);
                x.set16(fish::TIMER, 0x60);
                x.anim_start(fish::ANIM_SET, 5);
            }
        }
        10 => {
            let t = x.u16(fish::TIMER).wrapping_sub(1);
            x.set16(fish::TIMER, t);
            if t == 0 {
                x.add8(f::STATE, 2);
            }
            let mask = if t > 0x30 { 3 } else { 1 };
            set_hidden(x, t & mask == 0);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Type 39 : morceau projeté par un porteur, qui vole vers un point de l'écran et éclate en tombant
// ---------------------------------------------------------------------------------------------

mod lob {
    pub const ANIM_SET: u32 = 0x16f58c;
    /// Par variante : cible (x, y) relative à la caméra (mots), table de vitesses (octets), vitesse
    /// (long, 16.16), espace programme.
    pub const TARGETS: u32 = 0xb3762;
    pub const SPEED_TABLES: u32 = 0xb3614;
    pub const SPEEDS: u32 = 0xb361e;
    pub const CARRIER: u32 = 0x42;
    pub const TARGET_X: u32 = 0x58;
    pub const TARGET_Y: u32 = 0x5a;
}

/// 0x17ea : division non signée sur 32 bits par décalages (quotient ; D1 nul : tous les bits à 1).
fn divide(mut n: u32, d: u32) -> u32 {
    let mut r = 0u32;
    for _ in 0..32 {
        let top = n >> 31;
        n <<= 1;
        r = r.wrapping_shl(1) | top;
        if r >= d {
            r = r.wrapping_sub(d);
            n |= 1;
        }
    }
    n
}

fn lob(x: &mut Cx) {
    x.call(super::cx::sub::PAUSE_HOOK);
    if x.c.sr & m68k::SR_Z == 0 {
        return;
    }
    match x.u8(f::STATE) {
        0 => lob_init(x),
        2 => lob_active(x),
        _ => free(x),
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

fn lob_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.call(0x3af8);
    // 0xb3734 : cible sur l'écran, direction
    let v = x.u8(f::VARIANT) as u32;
    let tx = x.rom16(lob::TARGETS + 4 * v).wrapping_add(x.a5_u16(0x5a6));
    let ty = x.rom16(lob::TARGETS + 4 * v + 2).wrapping_add(x.a5_u16(0x5aa));
    x.set16(lob::TARGET_X, tx);
    x.set16(lob::TARGET_Y, ty);
    x.c.d[1] = x.c.d[1] & 0xffff_0000 | tx as u32;
    x.c.d[2] = x.c.d[2] & 0xffff_0000 | ty as u32;
    x.call(0x137e);
    let dir = x.c.d[0] as u8;
    x.set8(Obj::SPEED_INDEX, dir);
    x.c.d[0] = x.rom8(lob::SPEED_TABLES + v) as u32;
    x.call(super::cx::sub::SPEED_TABLE_B);
    // durée du vol : la plus grande distance (x ou y) divisée par la vitesse
    let dx = (tx.wrapping_sub(x.u16(Obj::X)) as i16).wrapping_abs() as u16;
    let dy = (ty.wrapping_sub(x.u16(Obj::Y)) as i16).wrapping_abs() as u16;
    let dist = if (dx as i16) < (dy as i16) { dy } else { dx };
    let speed = x.rom16(lob::SPEEDS + 4 * v) as u32;
    let speed = speed << 16 | x.rom16(lob::SPEEDS + 4 * v + 2) as u32;
    let mut left = (dist as u32) << 16;
    let mut frames = 0u32;
    loop {
        left = left.wrapping_sub(speed);
        if left as i32 <= 0 {
            break;
        }
        frames = frames.wrapping_add(1) & 0xffff;
    }
    // vitesse verticale pour arriver au sol en même temps
    let q = divide(x.u16(Obj::Z) as u32, frames);
    let vz = (q.wrapping_neg().rotate_left(16) >> 8) as u16;
    x.set16(Obj::VZ, vz);
    x.anim_start(lob::ANIM_SET, 0);
}

fn lob_active(x: &mut Cx) {
    let v = x.u8(f::VARIANT);
    match x.u8(f::SUB_STATE) {
        0 => {
            let carrier = x.linked(lob::CARRIER);
            let launched = if (v as i8) > 4 {
                carrier.u16(x.b, f::STATE) == 0x214
            } else {
                match carrier.u32(x.b, f::STATE) {
                    0x0200_0414 => true,
                    0x0200_0424 => return free(x),
                    _ => false,
                }
            };
            if launched {
                x.add8(f::SUB_STATE, 2);
            }
            x.anim_step();
        }
        2 => {
            x.call(super::cx::sub::MOVE_XYZ);
            if (x.u16(Obj::Z) as i16) > 0 {
                x.anim_step();
                return;
            }
            // au sol : éclats (type 50 variante 5), son, et pour les variantes 4 et 9 un tremblement
            x.add8(f::SUB_STATE, 2);
            x.set16(Obj::Z, 0);
            let pieces = if (v as i8) < 5 { 3 } else { 2 };
            for _ in 0..pieces {
                if let Some(o) = alloc(x, ALLOC_EFFECT) {
                    wr8(x.b, o.0, 1);
                    wr8(x.b, o.0 + 2, 50);
                    wr8(x.b, o.0 + 3, 5);
                    wr16(x.b, o.0 + 0x42, x.o.0 as u16);
                    let (px, py) = (x.u16(Obj::X), x.u16(Obj::Y));
                    wr16(x.b, o.0 + Obj::X, px);
                    wr16(x.b, o.0 + Obj::Y, py);
                }
            }
            x.c.d[0] = 0xf;
            x.call(0x2860);
            x.anim_start(lob::ANIM_SET, 2);
            let shake = match v {
                4 => 0xa,
                9 => 6,
                _ => return,
            };
            x.set_a5_u16(0x4be, shake);
            x.c.d[1] = shake as u32;
            x.call(0x1ac68);
            x.call(0xe5d36);
        }
        4 => {
            if x.anim_step() {
                x.add8(f::STATE, 2);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Types 85 et 86 : objets de la séquence de fin (pièce qui glisse ; flammèche qui s'éloigne)
// ---------------------------------------------------------------------------------------------

/// Jeu d'animations de l'interface.
const ANIM_INTERFACE: u32 = 0x1c03a6;

fn ending_piece(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set16(Obj::X, 0xd0);
            x.set16(Obj::Z, 0xffe6);
            x.set16(Obj::Y, 0x190);
            x.anim_start(ANIM_INTERFACE, 0x17);
            x.reinsert();
        }
        2 => match x.u8(f::SUB_STATE) {
            0 => {
                if x.a5_u8(0x42) != 0 && rd8(x.b, floater::STEP) == 0xc {
                    x.add8(f::SUB_STATE, 2);
                    x.call(0xead60);
                }
            }
            2 => {
                x.add16(Obj::X, 2u16.wrapping_neg());
                if (x.u16(Obj::X) as i16) <= 0x80 {
                    x.add8(f::SUB_STATE, 2);
                    wr8(x.b, floater::STEP, 0x14);
                    x.c.d[0] = 0x10;
                    return x.call(0x2824);
                }
                // tremble sur place, une image sur deux
                x.add16(Obj::X, 2);
                x.add16(Obj::Y, 2);
                if x.a5_u8(0x6f) & 1 != 0 {
                    x.add16(Obj::X, 4u16.wrapping_neg());
                    x.add16(Obj::Y, 4u16.wrapping_neg());
                }
            }
            _ => {}
        },
        _ => free(x),
    }
}

fn ending_wisp(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            if x.u16(0x5c) == 0 {
                return;
            }
            let owner = x.linked(0x5c);
            let (mut px, mut py) = (owner.u16(x.b, Obj::X), owner.u16(x.b, Obj::Y));
            if x.u8(f::VARIANT) == 0 {
                let k = (owner.u8(x.b, f::VARIANT) as u32) << 2;
                px = px.wrapping_add(x.rom16(0xb9d3e + k));
                py = py.wrapping_add(x.rom16(0xb9d40 + k));
            }
            x.set16(Obj::X, px);
            x.set16(Obj::Y, py);
            x.set16(Obj::VX, 0xff20);
            x.anim_start(0x1991ca, 0);
            x.call(super::cx::sub::REINSERT_FOREGROUND);
        }
        2 => {
            if x.anim_step() {
                x.add8(f::STATE, 2);
            }
            let odd = x.a5_u16(0x6e) & 1 != 0;
            set_hidden(x, odd);
            x.call(super::cx::sub::MOVE_X);
        }
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 23 : vote des joueurs pour le chemin suivant (variante 0 : scrutin ; 1 : curseur d'un joueur ;
// 2 : compte à rebours)
// ---------------------------------------------------------------------------------------------

mod vote {
    /// Nombre de choix par mode `+0x2f` (octets, données).
    pub const CHOICES: u32 = 0xafec8;
    /// Positions des curseurs : par nombre de joueurs (`A5+0x72`), table de décalages par mode, puis
    /// (x, z) par choix et par joueur (données).
    pub const CURSOR_PLACES: u32 = 0xb042c;
    /// Position du compte à rebours, même indexation (données).
    pub const TIMER_PLACES: u32 = 0xb09e0;
    /// Zones de texte des choix à faire clignoter (données) ; tampon de sauvegarde.
    pub const TEXT_ZONES: u32 = 0xb0a9c;
    pub const TEXT: u32 = 0xff2072;
    pub const SAVE: u32 = 0x910000;
    /// Modes dont les directions haut/bas comptent aussi (octets, espace programme).
    pub const VERTICAL: u32 = 0xb03b6;
    pub const ANIM_SET: u32 = 0x1c03a6;
    /// Scrutin : pointeur (créateur `+0x50`), votants restants `+0x42`, curseurs vivants `+0x44`,
    /// dépouillé `+0x53`, durée `+0x54` (nulle : fin du temps), voix par choix `+0x58..+0x5b`.
    pub const BALLOT: u32 = 0x50;
}

fn spawn_vote(x: &mut Cx, variant: u8, mode: u8) -> Option<Obj> {
    let o = alloc(x, ALLOC_EFFECT)?;
    wr8(x.b, o.0, 1);
    wr8(x.b, o.0 + 2, 23);
    wr8(x.b, o.0 + 3, variant);
    wr8(x.b, o.0 + 0x2f, mode);
    Some(o)
}

fn vote(x: &mut Cx) {
    match x.u8(f::VARIANT) {
        0 => ballot(x),
        1 => ballot_cursor(x),
        2 => ballot_timer(x),
        _ => {}
    }
}

/// Mot de la table indexée par le nombre de joueurs (`A5+0x72` : 2 ou 4).
fn by_players(x: &mut Cx, table: u32) -> u32 {
    let k = (x.a5_u16(0x72).wrapping_sub(2).wrapping_mul(2)) as i16 as i32 as u32;
    x.data32(table.wrapping_add(k))
}

fn ballot(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.set8(0x53, 0);
            x.add8(f::STATE, 2);
            x.set16(0x54, 0xffff);
            x.set32(0x58, 0);
            let mode = x.u8(0x2f) as u32;
            let choices = x.data8(vote::CHOICES + mode);
            x.set8(0x43, choices);
            // un curseur par joueur en jeu
            let mut voters = 0u8;
            for k in 0..4 {
                let p = Obj(A5 + 0x5de + 0x200 * k);
                if !p.is_active(x.b) || (p.u8(x.b, f::STATE) as i8) >= 4 {
                    continue;
                }
                if let Some(o) = spawn_vote(x, 1, voters) {
                    wr16(x.b, o.0 + 0x50, x.o.0 as u16);
                    wr16(x.b, o.0 + 0x54, p.0 as u16);
                    let index = p.u8(x.b, 2);
                    wr8(x.b, o.0 + 0x2f, index);
                    wr8(x.b, o.0 + 0x43, choices);
                    voters += 1;
                }
            }
            x.set8(0x42, voters);
            x.set8(0x44, voters);
            if let Some(o) = spawn_vote(x, 2, voters) {
                wr16(x.b, o.0 + 0x50, x.o.0 as u16);
            }
        }
        2 => {
            if x.u8(0x42) == 0 {
                x.set16(0x54, 0);
                x.add8(f::STATE, 2);
                x.set8(f::SUB_STATE, 0);
            }
        }
        4 => ballot_count(x),
        6 => ballot_end(x),
        _ => {}
    }
}

/// 0xb01e2 : fin, quand tous les curseurs sont partis ; prévient le créateur.
fn ballot_end(x: &mut Cx) {
    if x.u8(0x44) != 0 {
        return;
    }
    let creator = x.linked(vote::BALLOT);
    creator.set_u16(x.b, 0x32, 1);
    free(x);
}

fn ballot_count(x: &mut Cx) {
    match x.u8(f::SUB_STATE) {
        0 => {
            // choix gagnant : le plus de voix, au hasard parmi les ex æquo
            let creator = x.linked(vote::BALLOT);
            let mut best = x.u8(0x58) as i8;
            for k in 1..4 {
                best = best.max(x.u8(0x58 + k) as i8);
            }
            for k in 0..4 {
                let v = if x.u8(0x58 + k) as i8 == best { 0xff } else { 0 };
                x.set8(0x58 + k, v);
            }
            x.call(super::rom::RNG);
            let mut k = x.c.d[0] & 3;
            while x.u8(0x58 + k) == 0 {
                k = (k + 1) & 3;
            }
            let choice = k as u8;
            creator.set_u8(x.b, 0x23, choice);
            x.set8(0x58, choice);
            x.set8(0x53, 1);
            x.add8(f::SUB_STATE, 2);
        }
        2 => {
            x.add8(f::SUB_STATE, 2);
            let zones = by_players(x, vote::TEXT_ZONES);
            let mode = x.u8(0x2f) as u32;
            let mut off = x.data16(zones + 2 * mode);
            off = off.wrapping_add((x.u8(0x58) as u16) << 4);
            let level = (x.a5_u8(0x35).wrapping_sub(1) as u16) << 2;
            off = off.wrapping_add(level);
            let zone = zones.wrapping_add(off as i16 as i32 as u32);
            let count = x.data16(zone);
            let size = x.data16(zone + 2);
            x.set16(0x34, size);
            ballot_save_text(x, count);
            x.set8(0x36, 8);
            x.set8(0x37, 3);
            x.set8(0x38, 1);
        }
        4 => {
            let t = x.u8(0x37).wrapping_sub(1);
            x.set8(0x37, t);
            if t != 0 {
                return;
            }
            x.set8(0x37, 2);
            let at = x.u32(0x30);
            let n = x.u16(0x34);
            let shown = x.u8(0x38) ^ 1;
            x.set8(0x38, shown);
            if shown == 0 {
                return ballot_erase_text(x, at, n);
            }
            x.set8(0x37, 3);
            let format = text_format(x);
            text_copy(x, format, vote::SAVE, at, n);
            let left = x.u8(0x36).wrapping_sub(1);
            x.set8(0x36, left);
            if left == 0 {
                x.add8(f::SUB_STATE, 2);
                x.set8(0x37, 0x1e);
            }
        }
        6 => {
            let t = x.u8(0x37).wrapping_sub(1);
            x.set8(0x37, t);
            if t == 0 {
                let creator = x.linked(vote::BALLOT);
                creator.set_u16(x.b, 0x32, 1);
                x.add8(f::STATE, 2);
            }
        }
        _ => {}
    }
}

/// Texte de la couche de texte, en mots (`A5+0x72` = 2) ou en octets (autres langues).
/// En mots : 0xffff introduit une commande de 6 octets, 0xfffe de 4, les autres mots sont des
/// caractères. En octets : 0xff introduit une commande de 3 octets, 0xfe de 2, un octet ≤ 0x0f un
/// caractère de 2 octets, les autres octets sont des caractères.
#[derive(Clone, Copy, PartialEq)]
enum TextFormat {
    Words,
    Bytes,
}

fn text_format(x: &mut Cx) -> TextFormat {
    if x.a5_u16(0x72) == 2 { TextFormat::Words } else { TextFormat::Bytes }
}

/// Longueur de l'élément de texte en `at`, et s'il s'agit d'un caractère.
fn text_token(x: &mut Cx, format: TextFormat, at: u32) -> (u32, bool) {
    match format {
        TextFormat::Words => match rd16(x.b, at) {
            0xffff => (6, false),
            0xfffe => (4, false),
            _ => (2, true),
        },
        TextFormat::Bytes => match rd8(x.b, at) {
            0xff => (3, false),
            0xfe => (2, false),
            c if c <= 0xf => (2, true),
            _ => (1, true),
        },
    }
}

/// Avance de `n + 1` caractères (commandes comprises) depuis `at`, en appelant `each` sur chaque
/// élément (adresse, longueur, caractère ?). Rend l'adresse qui suit.
fn text_walk(x: &mut Cx, format: TextFormat, mut at: u32, n: u16, mut each: impl FnMut(&mut Cx, u32, u32, bool)) -> u32 {
    let mut n = n;
    loop {
        let (len, is_char) = text_token(x, format, at);
        each(x, at, len, is_char);
        at += len;
        if is_char {
            n = n.wrapping_sub(1);
            if n == 0xffff {
                return at;
            }
        }
    }
}

/// Recopie `n + 1` caractères (commandes comprises) de `from` vers `to`.
fn text_copy(x: &mut Cx, format: TextFormat, from: u32, to: u32, n: u16) {
    text_walk(x, format, from, n, |x, at, len, _| {
        for k in 0..len {
            let v = rd8(x.b, at + k);
            wr8(x.b, to + (at - from) + k, v);
        }
    });
}

/// 0xb0102 : saute `count + 1` caractères, note l'adresse et sauvegarde le texte du choix gagnant.
fn ballot_save_text(x: &mut Cx, count: u16) {
    let format = text_format(x);
    let at = text_walk(x, format, vote::TEXT, count, |_, _, _, _| {});
    x.set32(0x30, at);
    let size = x.u16(0x34);
    text_copy(x, format, at, vote::SAVE, size);
}

/// 0xb0076 : efface le texte (caractère blanc : 0x1800 en mots ; 0x80, ou 1 puis 0 pour un
/// caractère de 2 octets), commandes conservées.
fn ballot_erase_text(x: &mut Cx, at: u32, n: u16) {
    let format = text_format(x);
    text_walk(x, format, at, n, |x, at, len, is_char| {
        if !is_char {
            return;
        }
        match (format, len) {
            (TextFormat::Words, _) => wr16(x.b, at, 0x1800),
            (TextFormat::Bytes, 2) => {
                wr8(x.b, at, 1);
                wr8(x.b, at + 1, 0);
            }
            _ => wr8(x.b, at, 0x80),
        }
    });
}

/// 0xb03c6 : place le curseur sur son choix (`+0x42`), selon le mode du scrutin et le joueur.
fn cursor_place(x: &mut Cx) {
    let ballot = x.linked(vote::BALLOT);
    let places = by_players(x, vote::CURSOR_PLACES);
    let mode = ballot.u8(x.b, 0x2f) as u32;
    let places = places.wrapping_add(x.data16(places + 2 * mode) as i16 as i32 as u32);
    let k = ((x.u8(0x42) as u32) << 4) + 4 * x.u8(0x2f) as u32;
    let (px, pz) = (x.data16(places + k), x.data16(places + k + 2));
    let (cx_, cz) = if x.a5_u8(0x36) != 0 {
        (x.a5_u16(0x556), x.a5_u16(0x55a))
    } else {
        (x.a5_u16(0x506), x.a5_u16(0x50a))
    };
    x.set16(Obj::X, px.wrapping_add(cx_));
    x.set16(Obj::Z, pz.wrapping_add(cz));
}

fn ballot_cursor(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set8(0x42, 0);
            x.set32(Obj::Y, 0xffff_ff00);
            cursor_place(x);
            let player = x.u8(0x2f);
            x.add32(Obj::Y, player as u32);
            x.set8(0x2a, 1);
            x.set8(0x2b, 0x12u8.wrapping_add(player));
            x.anim_start(vote::ANIM_SET, 0x173 + player as u32);
            x.reinsert();
        }
        2 => {
            let ballot = x.linked(vote::BALLOT);
            if ballot.u16(x.b, 0x54) == 0 {
                return cursor_vote(x);
            }
            x.anim_step();
            // 0xb0390 : les entrées du joueur (gauche/droite, et haut/bas pour certains modes)
            let mut mask: u8 = 0xfc;
            let ballot_mode = ballot.u8(x.b, 0x2f) as u32;
            if x.rom8(vote::VERTICAL + ballot_mode) != 0 {
                mask = 0xf3;
            }
            let player = x.u8(0x2f) as u32;
            let input = x.a5_u8(0xc + player) & mask;
            if input == 0 {
                return cursor_place(x);
            }
            let step: u8 = if input & 5 != 0 {
                1
            } else if input & 0xa != 0 {
                0xff
            } else {
                0
            };
            x.add8(0x42, step);
            let last = x.u8(0x43);
            let choice = x.u8(0x42);
            if last < choice {
                // au-delà du dernier choix : retour au premier, ou au dernier en dessous du premier
                x.set8(0x42, if (last.wrapping_sub(choice) as i8) < 0 { 0 } else { last });
            }
            if input & 0xf0 == 0 {
                return cursor_place(x);
            }
            cursor_vote(x);
        }
        4 => {
            let ballot = x.linked(vote::BALLOT);
            if ballot.u8(x.b, 0x53) == 0 {
                return cursor_place(x);
            }
            let n = ballot.u8(x.b, 0x44).wrapping_sub(1);
            ballot.set_u8(x.b, 0x44, n);
            free(x);
        }
        _ => {}
    }
}

/// 0xb02b4 : le joueur vote pour le choix courant.
fn cursor_vote(x: &mut Cx) {
    let ballot = x.linked(vote::BALLOT);
    let n = ballot.u8(x.b, 0x42).wrapping_sub(1);
    ballot.set_u8(x.b, 0x42, n);
    let choice = x.u8(0x42) as u32;
    let votes = ballot.u8(x.b, 0x58 + choice).wrapping_add(1);
    ballot.set_u8(x.b, 0x58 + choice, votes);
    x.add8(f::STATE, 2);
    cursor_place(x);
    x.call(0x1f0be);
}

/// 0xb0990 : le compte à rebours est posé sur la caméra, à la place donnée par le mode du scrutin.
fn timer_place(x: &mut Cx) -> Obj {
    let ballot = x.linked(vote::BALLOT);
    let (cx_, cz) = if x.a5_u8(0x36) != 0 {
        (x.a5_u16(0x556), x.a5_u16(0x55a))
    } else {
        (x.a5_u16(0x506), x.a5_u16(0x50a))
    };
    x.set16(Obj::X, cx_);
    x.set16(Obj::Z, cz);
    let places = by_players(x, vote::TIMER_PLACES);
    let k = 4 * ballot.u8(x.b, 0x2f) as u32;
    let (dx, dz) = (x.data16(places + k), x.data16(places + k + 2));
    x.add16(Obj::X, dx);
    x.add16(Obj::Z, dz);
    ballot
}

fn timer_palette(x: &mut Cx, src: u32) {
    for k in 0..8 {
        let v = x.data32(src + 4 * k) | 0xf000_f000;
        wr32(x.b, 0x91c1c0 + 4 * k, v);
    }
}

/// 0xb0330 : chiffre du compte à rebours (secondes restantes, octet haut de `+0x54`).
fn timer_show(x: &mut Cx) {
    let anim = 0xe3u8.wrapping_sub(x.u8(0x54));
    x.anim_start(vote::ANIM_SET, anim as u32);
    x.set8(0x3f, 6);
    x.reinsert();
}

fn ballot_timer(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            timer_palette(x, 0x28ff20);
            x.set8(0x2a, 1);
            x.set8(0x2b, 0xe);
            x.set32(Obj::Y, 0);
            timer_place(x);
            x.set16(0x54, 0x143c);
            timer_show(x);
        }
        2 => {
            let ballot = timer_place(x);
            if ballot.u16(x.b, 0x54) == 0 {
                return ballot_end(x);
            }
            let t = x.u8(0x55).wrapping_sub(1);
            x.set8(0x55, t);
            if (t as i8) >= 0 {
                return;
            }
            x.set8(0x55, 0x3c);
            let s = x.u8(0x54).wrapping_sub(1);
            x.set8(0x54, s);
            if s != 0 {
                return timer_show(x);
            }
            // temps écoulé : le scrutin se termine
            ballot.set_u16(x.b, 0x54, 0);
            timer_palette(x, 0x2531c0);
            free(x);
        }
        // état 4 : jamais atteint par la table (code d'origine, A6 non défini)
        4 => x.call(0xb0374),
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Type 60 : second double d'un porteur (même principe que le type 16, sept étapes)
// ---------------------------------------------------------------------------------------------

mod double2 {
    pub const TARGETS: u32 = 0xb7516;
    pub const DIRECTIONS: u32 = 0xb74fe;
    pub const SPEEDS: u32 = 0xb7502;
}

/// 0xb74ea : direction initiale de la variante, et sa table de vitesses dans D0 (D1 = variante).
fn double2_direction(x: &mut Cx) {
    let v = x.u8(f::VARIANT) as u32;
    let dir = x.rom8(double2::DIRECTIONS + v);
    x.set8(Obj::SPEED_INDEX, dir);
    x.c.d[0] = x.rom8(double2::SPEEDS + v) as u32;
    x.c.d[1] = v;
}

fn double2(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let v = x.u8(f::VARIANT) as u32;
            let target = x.rom16(double2::TARGETS + 2 * v);
            x.set16(double::TARGET_X, target);
            double2_direction(x);
            x.call(super::cx::sub::SPEED_TABLE_B);
            let carrier = x.linked(double::CARRIER);
            copy_position(x, carrier);
            set_hidden(x, true);
            let frame = carrier.u32(x.b, 0x34);
            x.set32(0x34, frame);
            x.set8(0x3b, 0);
            for off in [0x3c, 0x3f] {
                let b = carrier.u8(x.b, off);
                x.set8(off, b);
            }
        }
        2 => double2_active(x),
        _ => return free(x),
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

fn double2_active(x: &mut Cx) {
    let carrier = x.linked(double::CARRIER);
    let sub = x.u8(f::SUB_STATE);
    let awaited = |sub: u8| match sub {
        0 => 0x202,
        4 => 0x208,
        6 => 0x20a,
        8 => 0x212,
        _ => 0x216,
    };
    match sub {
        0 => {
            if carrier.u16(x.b, f::STATE) == 0x202 {
                x.add8(f::SUB_STATE, 2);
                let z = carrier.u16(x.b, Obj::Z);
                x.set16(Obj::Z, z);
                set_hidden(x, false);
                x.set8(0x3b, 0);
                for off in [0x3c, 0x3f] {
                    let b = carrier.u8(x.b, off);
                    x.set8(off, b);
                }
            }
        }
        2 | 12 => {
            toggle_hidden(x);
            if !double_slide(x) {
                return double_copy_frame(x, sub == 12, true);
            }
            if sub == 12 {
                return free(x);
            }
            x.add8(f::SUB_STATE, 2);
            set_hidden(x, false);
            let target = x.u16(double::TARGET_X);
            x.set16(Obj::X, target);
            double_copy_frame(x, true, true);
        }
        4 | 6 | 8 | 10 => {
            if carrier.u16(x.b, f::STATE) == awaited(sub) {
                x.add8(f::SUB_STATE, 2);
                match sub {
                    4 => {
                        if let Some(o) = alloc(x, ALLOC_EFFECT) {
                            wr8(x.b, o.0, 1);
                            wr8(x.b, o.0 + 2, 50);
                            wr8(x.b, o.0 + 3, 2);
                            wr16(x.b, o.0 + 0x42, x.o.0 as u16);
                            let facing = x.u8(0x2c);
                            wr8(x.b, o.0 + 0x2c, facing);
                        }
                    }
                    6 => {
                        if let Some(o) = alloc(x, ALLOC_EFFECT) {
                            wr8(x.b, o.0, 1);
                            wr8(x.b, o.0 + 2, 50);
                            wr8(x.b, o.0 + 3, 6);
                            for field in [Obj::X, Obj::Y, Obj::Z] {
                                let v = x.u16(field);
                                wr16(x.b, o.0 + field, v);
                            }
                            let dx: u16 = if x.u8(0x2c) != 0 { 0x10 } else { 0xfff0 };
                            let px = rd16(x.b, o.0 + Obj::X).wrapping_add(dx);
                            wr16(x.b, o.0 + Obj::X, px);
                        }
                    }
                    8 => {
                        // le morceau projeté (type 39, variante + 5) : l'original réessaie jusqu'à trouver une place
                        let o = loop {
                            if let Some(o) = alloc(x, ALLOC_EFFECT) {
                                break o;
                            }
                        };
                        wr8(x.b, o.0, 1);
                        wr8(x.b, o.0 + 2, 39);
                        let v = x.u8(f::VARIANT).wrapping_add(5);
                        wr8(x.b, o.0 + 3, v);
                        let c = x.u16(double::CARRIER);
                        wr16(x.b, o.0 + 0x42, c);
                        let facing = x.u8(0x2c);
                        wr8(x.b, o.0 + 0x2c, facing);
                        let (px, py) = (x.u16(Obj::X), x.u16(Obj::Y));
                        wr16(x.b, o.0 + Obj::X, px);
                        wr16(x.b, o.0 + Obj::Y, py);
                        wr16(x.b, o.0 + Obj::Z, 0x66);
                    }
                    _ => {
                        // retour : demi-tour vers le porteur
                        let cx_ = carrier.u16(x.b, Obj::X);
                        x.set16(double::TARGET_X, cx_);
                        double2_direction(x);
                        let dir = (x.u8(Obj::SPEED_INDEX).wrapping_add(0x10)) & 0x1f;
                        x.set8(Obj::SPEED_INDEX, dir);
                        x.call(super::cx::sub::SPEED_TABLE_B);
                    }
                }
            }
            double_copy_frame(x, true, true);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Type 61 : créature de la scène d'introduction, qui sort de l'eau, nage en ondulant puis s'en va
// ---------------------------------------------------------------------------------------------

mod swimmer {
    pub const ANIM_SET: u32 = 0x197768;
    pub const TIMER: u32 = 0x42;
}

fn swimmer(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let side = random_nibble16(x) as u8 & 1;
            x.set8(f::VARIANT, side);
            let wave = random_nibble16(x) as u8 & 1;
            x.set8(0x2f, wave);
            // 0xb7792 : son remous (type 62)
            if let Some(o) = alloc(x, ALLOC_EFFECT) {
                wr8(x.b, o.0, 1);
                wr8(x.b, o.0 + 2, 62);
                wr16(x.b, o.0 + 0x5c, x.o.0 as u16);
            }
            // deux décors SET 0x17 (variantes 0 et 1) ; un pool plein mène l'original dans des données
            for (variant, flip) in [(0u8, 1u8), (1, 0)] {
                let Some(o) = alloc(x, 0x2348) else { return x.call(0xb778c) };
                wr8(x.b, o.0, 1);
                wr8(x.b, o.0 + 2, 0x17);
                wr8(x.b, o.0 + 3, variant);
                wr8(x.b, o.0 + 0x2f, side ^ flip);
            }
            swimmer_active(x);
        }
        2 => swimmer_active(x),
        _ => return free(x),
    }
    x.reinsert();
}

fn swimmer_anim(x: &mut Cx, n: u32) {
    x.anim_start(swimmer::ANIM_SET, n);
    x.anim_step();
}

/// Étapes de 4 en 4 ; une étape achevée enchaîne la suivante dans la même image.
fn swimmer_active(x: &mut Cx) {
    let mut right = x.u8(f::VARIANT) != 0;
    let mut sub = x.u8(f::SUB_STATE);
    loop {
        match sub {
            0 => {
                x.add8(f::SUB_STATE, 4);
                x.set16(Obj::X, 399);
                x.set16(Obj::Y, 312);
                x.set8(0x2a, 1);
                x.set8(0x2b, 0x1b);
                for k in 0..8 {
                    let v = x.data32(0x253440 + 4 * k);
                    let v = if k < 6 { v | 0xf000_f000 } else { v.wrapping_add(0xf000_f000) };
                    wr32(x.b, 0x91c360 + 4 * k, v);
                }
                swimmer_anim(x, 4);
            }
            4 => {
                if (x.a5_u16(0x506) as i16) < 0x10 || !x.anim_step() {
                    return;
                }
                x.add8(f::SUB_STATE, 4);
                swimmer_anim(x, 2);
            }
            8 => {
                if !x.anim_step() {
                    return;
                }
                x.add8(f::SUB_STATE, 4);
                x.set16(swimmer::TIMER, 0x42);
                swimmer_anim(x, 3);
            }
            12 => {
                // ondulation : montée (variante 0) ou descente, sens inversé par `+0x2f`
                let (base, wave): (u32, u32) = if right { (0x2000, 0x4000u32.wrapping_neg()) } else { (0x2000u32.wrapping_neg(), 0x4000) };
                x.add32(Obj::Y, base);
                if x.u8(0x2f) != 0 {
                    x.add32(Obj::Y, wave);
                }
                x.anim_step();
                let t = x.u16(swimmer::TIMER).wrapping_sub(1);
                x.set16(swimmer::TIMER, t);
                if (t as i16) >= 0 {
                    return;
                }
                x.add8(f::SUB_STATE, 4);
                swimmer_anim(x, 1);
                if !right {
                    return;
                }
            }
            16 => {
                if !x.anim_step() {
                    return;
                }
                x.add8(f::SUB_STATE, 4);
                let facing = x.u8(0x2c) ^ 0x20;
                x.set8(0x2c, facing);
                swimmer_anim(x, 0);
            }
            20 => {
                x.add16(Obj::X, 4);
                x.anim_step();
                x.call(0x3403c);
                let gap = x.u16(Obj::X).wrapping_sub(x.a5_u16(0x506));
                if (gap as i16) > 0x1a0 {
                    free(x);
                }
                return;
            }
            _ => return,
        }
        // l'étape 8 achevée enchaîne toujours le code de l'étape 12 de la variante 0
        right &= sub != 8;
        sub += 4;
    }
}

// ---------------------------------------------------------------------------------------------
// Type 70 : comptoir d'une boutique de scène (même zone de vente que le type 22)
// ---------------------------------------------------------------------------------------------

mod counter {
    /// Zone (4 mots, données : une seule entrée), annonce par variante (mots, espace programme),
    /// animation par variante (mots, données).
    pub const ZONE: u32 = 0xb8132;
    pub const MESSAGES: u32 = 0xb8126;
    pub const ANIMATIONS: u32 = 0xb812c;
    /// Le comptoir qui tient la vente en cours (`A5-0x5542`).
    pub const ACTIVE: u32 = 0xff2abe;
    /// Décor de scène 0x64 (le vendeur) créé à l'achat, gardé en `+0x1a`.
    pub const SELLER: u32 = 0x1a;
    pub const TIMER: u32 = 0x18;
    pub const STEP: u32 = 0x06;
    /// Message de la vente, tuiles de l'enseigne (20 longs dans le fond 1), listes d'objets à poser (par
    /// variante : longs vers des fiches de 8 octets, octet négatif en fin).
    pub const SALE_MESSAGE: u16 = 0x8e;
    pub const SIGN: u32 = 0x912ad4;
    pub const SIGN_TILES: [(u32, u32); 20] = [
        (0, 0x1b780005), (4, 0x1b880005), (0x80, 0x1b790005), (0x84, 0x1b890005),
        (0x100, 0x1b9a0005), (0x104, 0x1baa0005), (0x180, 0x1b9b0005), (0x184, 0x1bab0005),
        (0x200, 0x1bb00005), (0x204, 0x1bc00005), (0x280, 0x1bb10005), (0x284, 0x1bc10005),
        (0x300, 0x1bb80005), (0x304, 0x1bc80005), (0x380, 0x1bb90005), (0x384, 0x1bc90005),
        (0x400, 0x1b7c0005), (0x404, 0x1b8c0005), (0x480, 0x1b7d0005), (0x484, 0x1b8d0005),
    ];
    pub const GOODS: u32 = 0xb82e0;
    pub const SCENE_SET: u32 = 0xc8312;
    pub const ALLOC_PEFF: u32 = 0x228e;
}

/// `0xb83d8` : premier acheteur dans la zone : le comptoir prend la vente (s'il n'y en a pas d'autre) et
/// fait venir le vendeur (décor 0x64) ; A1 et D6 gardés comme l'original.
fn counter_buy(x: &mut Cx, _buyer: Obj) {
    let active = rd16(x.b, counter::ACTIVE);
    if active != 0 && active != x.o.0 as u16 {
        return;
    }
    crate::mem::wr16(x.b, counter::ACTIVE, x.o.0 as u16);
    let (a1, d6) = (x.c.a[1], x.c.d[6]);
    x.c.d[0] = 0x64;
    x.call(counter::SCENE_SET);
    let failed = x.c.sr & m68k::SR_C != 0;
    let seller = x.c.a[1];
    (x.c.a[1], x.c.d[6]) = (a1, d6);
    if failed {
        return crate::mem::wr16(x.b, counter::ACTIVE, 0);
    }
    x.set16(counter::SELLER, seller as u16);
    x.add8(f::SUB_STATE, 2);
    x.set16(counter::TIMER, 0x18);
}

/// `0xb81d6` : la vente. Après un délai, l'enseigne s'affiche, le message est annoncé et les articles sont
/// posés (PEFF 0x2a) ; puis on attend un acheteur devant le vendeur, qui fait accélérer le vendeur ; enfin
/// la vente se termine.
fn counter_sale(x: &mut Cx) {
    let step = x.u8(counter::STEP) as i8;
    if step > 2 {
        if countdown_word(x, counter::TIMER) {
            x.set8(f::SUB_STATE, 0);
            x.set8(counter::STEP, 0);
            crate::mem::wr16(x.b, counter::ACTIVE, 0);
        }
        return;
    }
    if step == 2 {
        // la boucle de l'original tient dans A1 (pointeur) et D6 (compteur)
        for k in 0..4u32 {
            x.c.a[1] = shop::BUYERS + 2 * (k + 1);
            x.c.d[6] = x.c.d[6] & 0xffff_0000 | (3 - k);
            let p = rd16(x.b, shop::BUYERS + 2 * k);
            if p == 0 {
                continue;
            }
            let buyer = Obj(p as i16 as i32 as u32 & 0xff_ffff);
            if buyer.u8(x.b, 0x58) as i8 <= 0 {
                continue;
            }
            let across = buyer.u16(x.b, Obj::X).wrapping_sub(x.a5_u16(0x506)).wrapping_sub(0xa8);
            if across > 0x50 {
                continue;
            }
            let depth = buyer.u16(x.b, Obj::Z).wrapping_add(buyer.u16(x.b, Obj::Y)).wrapping_sub(x.a5_u16(0x50a)).wrapping_sub(0x48);
            if depth <= 0x10 {
                let seller = x.linked(counter::SELLER);
                if seller.u16(x.b, 0x32) > 1 {
                    seller.set_u16(x.b, 0x32, 1);
                }
                x.add8(counter::STEP, 2);
                x.set16(counter::TIMER, 0x18);
                return;
            }
        }
        x.c.d[6] |= 0xffff;
        return;
    }
    if !countdown_word(x, counter::TIMER) {
        return;
    }
    x.add8(counter::STEP, 2);
    crate::mem::wr16(x.b, shop::MESSAGE, counter::SALE_MESSAGE);
    for (off, tile) in counter::SIGN_TILES {
        wr32(x.b, counter::SIGN + off, tile);
    }
    let v = x.u8(f::VARIANT) as u32;
    let mut at = crate::mem::prog32(x.b, counter::GOODS + 4 * v);
    loop {
        let setting = x.data8(at);
        at += 1;
        x.c.a[2] = at;
        if (setting as i8) < 0 {
            return;
        }
        let Some(o) = alloc(x, counter::ALLOC_PEFF) else { return };
        wr8(x.b, o.0, 1);
        wr8(x.b, o.0 + 2, 0x2a);
        wr16(x.b, o.0 + 0x5c, x.o.0 as u16);
        wr8(x.b, o.0 + 0x2f, setting);
        let variant = x.data8(at);
        wr8(x.b, o.0 + 3, variant);
        let px = x.a5_u16(0x506).wrapping_add(x.data16(at + 1));
        wr16(x.b, o.0 + Obj::X, px);
        let (z, y) = (x.data16(at + 3), x.data16(at + 5));
        wr16(x.b, o.0 + Obj::Z, z);
        wr16(x.b, o.0 + Obj::Y, y);
        at += 7;
        x.c.a[2] = at;
    }
}

/// `sub.w #1,(off,A0) ; bpl` : vrai quand le décompte passe sous zéro.
fn countdown_word(x: &mut Cx, off: u32) -> bool {
    let t = x.u16(off).wrapping_sub(1);
    x.set16(off, t);
    (t as i16) < 0
}

fn counter(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            for (k, off) in [0x14, 0x16, 0x54, 0x56].into_iter().enumerate() {
                let w = x.data16(counter::ZONE + 2 * k as u32);
                x.set16(off, w);
            }
            let v = x.u8(f::VARIANT) as u32;
            let message = x.rom16(counter::MESSAGES + 2 * v);
            x.set16(shop::MESSAGE_ID, message);
            let anim = x.data16(counter::ANIMATIONS + 2 * v) as u32;
            x.anim_start(shop::ANIM_SET, anim);
            x.reinsert();
        }
        2 if x.u8(f::SUB_STATE) != 0 => counter_sale(x),
        2 => shop_watch(x, counter_buy),
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 40 : effets d'une attaque d'ennemi (0 : éclat qui suit l'attaquant ; 1 : éclat posé devant lui ;
// 2 : éclat qui tombe du haut de l'écran ; 3 et 4 : débris projeté, la variante 3 tant que l'attaque dure)
// ---------------------------------------------------------------------------------------------

mod blow {
    /// Variante 1 : décalage (dx, dz) par paire de variantes de l'attaquant (programme).
    pub const GLOW_PLACES: u32 = 0xb3968;
    /// Variante 2 : décalage en x par paire de variantes, animation par réglage, hauteur d'arrêt par
    /// réglage et paire de variantes (programme).
    pub const FALL_PLACES: u32 = 0xb3a2a;
    pub const FALL_ANIMS: u32 = 0xb3a36;
    pub const FALL_FLOORS: u32 = 0xb3a98;
    pub const ANIM_SET: u32 = 0x18f412;
    /// Variante 0 : vitesses (x, z) par paire de variantes de l'attaquant, selon le mode (données).
    pub const SPEEDS_BEHIND: u32 = 0xb3862;
    pub const SPEEDS_FRONT: u32 = 0xb3894;
    /// Variante 4 : décalage (x, z) et vitesses (x, ax), 8 octets par paire de variantes (programme).
    pub const DEBRIS: u32 = 0xb3af8;
}

/// L'attaquant est-il encore dans l'étape `step` de son attaque (état `0x204`, `+6`) ? Sinon l'effet est
/// retiré (état 6).
fn blow_holds(x: &mut Cx, owner: Obj, step: u8) -> bool {
    if owner.u16(x.b, f::STATE) == 0x204 && owner.u8(x.b, 6) == step {
        return true;
    }
    x.set32(f::STATE, 0x0600_0000);
    false
}

fn blow(x: &mut Cx) {
    let v = x.u8(f::VARIANT);
    let owner = x.linked(0x5c);
    match (v, x.u8(f::STATE)) {
        (1, 0) => {
            // éclat posé devant l'attaquant (décalage par paire de variantes de l'attaquant)
            x.add8(f::STATE, 2);
            let (ox, oz, oy) = (owner.u16(x.b, Obj::X), owner.u16(x.b, Obj::Z), owner.u32(x.b, Obj::Y));
            x.set16(Obj::X, ox);
            x.set16(Obj::Z, oz);
            x.set32(Obj::Y, oy.wrapping_sub(1));
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            let k = 2 * (owner.u8(x.b, f::VARIANT) & !1) as u32;
            let mut dx = x.rom16(blow::GLOW_PLACES + k);
            let dz = x.rom16(blow::GLOW_PLACES + k + 2);
            if facing != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(Obj::X, dx);
            x.add16(Obj::Z, dz);
            x.anim_start(blow::ANIM_SET, 0xb);
            x.reinsert();
        }
        (1, 2) => {
            if blow_holds(x, owner, 0x3c) {
                x.anim_step();
            }
        }
        (2, 0) => {
            // éclat qui tombe du haut de l'écran devant l'attaquant
            x.add8(f::STATE, 2);
            let (ox, oy) = (owner.u32(x.b, Obj::X), owner.u32(x.b, Obj::Y));
            x.set32(Obj::X, ox);
            x.set32(Obj::Y, oy);
            let side = if x.u8(0x2f) & 1 != 0 { 1 } else { 0xffff_ffff };
            x.add32(Obj::Y, side);
            let camera = if x.a5_u8(0x36) != 0 { 0x55a } else { 0x50a };
            let top = x.a5_u16(camera).wrapping_add(0x100).wrapping_sub(x.u16(Obj::Y));
            x.set16(Obj::Z, top);
            let k = (owner.u8(x.b, f::VARIANT) & !1) as u32;
            let mut dx = x.rom16(blow::FALL_PLACES + k);
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            if facing != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(Obj::X, dx);
            let setting = x.u8(0x2f) as u32;
            let anim = x.rom8(blow::FALL_ANIMS + setting);
            x.anim_start(blow::ANIM_SET, anim as u32);
            x.reinsert();
        }
        (2, 2) => {
            if x.u8(f::SUB_STATE) == 0 {
                // chute de 4 pixels par image jusqu'à la hauteur prévue ; le premier (réglage 0) arrivé
                // fait avancer l'attaquant (`+7`)
                x.add16(Obj::Z, 0xfffc);
                let k = (x.u8(0x2f) & !1) as u32 + (((owner.u8(x.b, f::VARIANT) as u32) >> 1) << 3);
                let floor = x.rom16(blow::FALL_FLOORS + k);
                if floor as i16 >= x.u16(Obj::Z) as i16 {
                    x.add8(f::SUB_STATE, 2);
                    x.set16(Obj::Z, floor);
                    if x.u8(0x2f) == 0 {
                        let step = owner.u8(x.b, 7).wrapping_add(2);
                        owner.set_u8(x.b, 7, step);
                    }
                }
            }
            if blow_holds(x, owner, 0x3a) {
                x.anim_step();
            }
        }
        (3, 2) => {
            if blow_holds(x, owner, 0x38) {
                blow_debris(x);
            }
        }
        (0, 0) => {
            x.add8(f::STATE, 2);
            for off in [Obj::X, Obj::Z, Obj::Y] {
                let w = owner.u32(x.b, off);
                x.set32(off, w);
            }
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            let k = (owner.u8(x.b, f::VARIANT) >> 1) as u32;
            let (table, anim) = match x.u8(0x2f) {
                0 => {
                    x.add32(Obj::Y, 0xffff_ffff);
                    (blow::SPEEDS_BEHIND, 6)
                }
                1 => {
                    x.add32(Obj::Y, 1);
                    (blow::SPEEDS_FRONT, 4)
                }
                _ => return,
            };
            let (mut vx, vz) = (x.data16(table + 4 * k), x.data16(table + 4 * k + 2));
            if x.u8(0x2c) != 0 {
                vx = vx.wrapping_neg();
            }
            x.set16(Obj::VX, vx);
            x.set16(Obj::VZ, vz);
            x.add16(Obj::X, vx);
            x.add16(Obj::Z, vz);
            x.anim_start(blow::ANIM_SET, anim);
            x.reinsert();
        }
        (0, 2) => {
            if owner.u16(x.b, f::STATE) != 0x204 || (owner.u8(x.b, 7) as i8) >= 4 {
                return x.set32(f::STATE, 0x0600_0000);
            }
            for off in [Obj::X, Obj::Z] {
                let w = owner.u32(x.b, off);
                x.set32(off, w);
            }
            let (vx, vz) = (x.u16(Obj::VX), x.u16(Obj::VZ));
            x.add16(Obj::X, vx);
            x.add16(Obj::Z, vz);
            if x.anim_step() {
                x.set32(f::STATE, 0x0600_0000);
            }
        }
        (3 | 4, 0) => {
            x.add8(f::STATE, 2);
            x.set8(0x42, 0x3c);
            x.set16(Obj::VZ, 0x100);
            x.set16(Obj::AZ, 0);
            let (ox, oz) = (owner.u16(x.b, Obj::X), owner.u16(x.b, Obj::Z));
            x.set16(Obj::X, ox);
            x.set16(Obj::Z, oz);
            let depth = owner.u32(x.b, Obj::Y).wrapping_sub(1);
            x.set32(Obj::Y, depth);
            let k = ((owner.u8(x.b, f::VARIANT) & !1) as u32) << 2;
            let mut dx = x.rom16(blow::DEBRIS + k);
            let dz = x.rom16(blow::DEBRIS + k + 2);
            let (mut vx, mut ax) = (x.rom16(blow::DEBRIS + k + 4), x.rom16(blow::DEBRIS + k + 6));
            if owner.u8(x.b, 0x2c) != 0 {
                vx = vx.wrapping_neg();
                ax = ax.wrapping_neg();
                dx = dx.wrapping_neg();
            }
            x.set16(Obj::VX, vx);
            x.set16(Obj::AX, ax);
            x.add16(Obj::X, dx);
            x.add16(Obj::Z, dz);
            x.call(super::rom::RNG);
            let d0 = x.c.d[0];
            let anim = d0 & 0xffff_0000 | ((d0 & 1) as u8).wrapping_add(0x10) as u32;
            x.anim_start(blow::ANIM_SET, anim);
            x.reinsert();
        }
        (4, 2) => blow_debris(x),
        (_, 4 | 6) => free(x),
        _ => {}
    }
}

/// Débris projeté (variantes 3 et 4) : glisse jusqu'à l'arrêt, attend, puis finit son animation.
fn blow_debris(x: &mut Cx) {
    x.call(super::cx::sub::MOVE_XZ);
    match x.u8(f::SUB_STATE) {
        0 => {
            if x.u16(Obj::VX) == 0 {
                x.set32(Obj::AX, 0);
                x.add8(f::SUB_STATE, 2);
            }
        }
        2 => {
            let n = x.u8(0x42).wrapping_sub(1);
            x.set8(0x42, n);
            if n != 0 {
                x.add8(f::SUB_STATE, 2);
            }
        }
        _ => {
            if x.anim_step() {
                x.add8(f::STATE, 2);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 67 : colonne qui s'élève sous un objet jusqu'à la fin d'un compte de la scène, puis relâche
// les joueurs
// ---------------------------------------------------------------------------------------------

fn pillar(x: &mut Cx) {
    let v = x.u8(f::VARIANT);
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let base = x.linked(0x50);
            let (bx, by) = (base.u16(x.b, Obj::X), base.u16(x.b, Obj::Y));
            x.set16(Obj::X, bx);
            x.set16(Obj::Y, by);
            base.set_u16(x.b, 0x12, 0x4000);
            x.set16(0x12, if v & 2 != 0 { 0x8000 } else { 0 });
            let top = if x.a5_u8(0x36) != 0 { x.a5_u16(0x55a) } else { x.a5_u16(0x50a) };
            x.set16(Obj::Z, top.wrapping_add(0x100).wrapping_sub(by));
            let delay = x.rom8(0xb7ea2 + v as u32);
            x.set8(0x58, delay);
            x.anim_start(0x1b67f2, 5u8.wrapping_add(v) as u32);
        }
        2 => {
            let delay = x.u8(0x58);
            if delay != 0 {
                x.set8(0x58, delay - 1);
            } else {
                let z = x.u16(Obj::Z);
                let rising = if v & 1 != 0 { z > 0x60 } else { (z as i16) >= 0 };
                if rising {
                    x.set16(Obj::Z, z.wrapping_sub(3));
                }
            }
            if rd16(x.b, 0xff2a3a) <= 0x40 {
                x.add8(f::STATE, 2);
                for k in 0..4 {
                    let p = Obj(A5 + 0x5de + 0x200 * k);
                    if p.is_active(x.b) && p.u16(x.b, 0x62) != 0 {
                        p.set_u8(x.b, 6, 0xe);
                    }
                }
            }
            x.anim_step();
            x.reinsert();
        }
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 34 : plateau qui avance et emporte les joueurs, jusqu'à ce que son décor l'arrête
// ---------------------------------------------------------------------------------------------

fn ferry(x: &mut Cx) {
    x.call(super::cx::sub::PAUSE_HOOK);
    if x.c.sr & m68k::SR_Z == 0 {
        return x.set8(f::STATE, 4);
    }
    if x.a5_u8(0x3d) != 0 {
        return;
    }
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set32(0x58, 0x1_0000);
            x.set16(Obj::Z, 0);
            x.set_a5_u8(0x42, 0);
            x.anim_start(0x1c03a6, 0);
            x.reinsert();
        }
        2 if x.u8(f::SUB_STATE) == 0 => {
            let v = x.u32(0x58);
            x.add32(Obj::X, v);
            if x.a5_u8(0x2c) == 0xc {
                x.add8(f::SUB_STATE, 2);
                x.c.d[0] = 0x63;
                x.call(0xc8312);
                x.set16(0x5c, x.c.a[1] as u16);
            }
        }
        2 => {
            let stop = x.linked(0x5c);
            if stop.u8(x.b, 0x22) != 0 {
                return x.set_a5_u8(0x42, 1);
            }
            let dx = x.u32(0x58);
            x.c.d[4] = dx;
            x.add32(Obj::X, dx);
            let (fx, fy) = (x.u16(Obj::X), x.u16(Obj::Y));
            for k in 0..4 {
                let p = Obj(A5 + 0x5de + 0x200 * k);
                if !p.is_active(x.b) {
                    continue;
                }
                p.set_u16(x.b, Obj::Y, fy.wrapping_sub(0x12));
                let px = p.u32(x.b, Obj::X).wrapping_add(dx);
                p.set_u32(x.b, Obj::X, px);
                let px = p.u16(x.b, Obj::X) as i16;
                let (right, left) = (fx.wrapping_add(0x48), fx.wrapping_sub(0x48));
                if (right as i16) < px {
                    p.set_u16(x.b, Obj::X, right);
                } else if (left as i16) > px {
                    p.set_u16(x.b, Obj::X, left);
                }
            }
            x.anim_step();
            x.reinsert();
        }
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 59 : créature d'une scène qui surgit, s'élève avec la caméra, lance ses projectiles puis
// bondit hors de l'écran (seize étapes)
// ---------------------------------------------------------------------------------------------

mod riser {
    pub const ANIM_SET: u32 = 0x197208;
    pub const TIMER: u32 = 0x56;
}

fn riser_anim(x: &mut Cx, n: u32) {
    x.add8(f::SUB_STATE, 2);
    x.anim_start(riser::ANIM_SET, n);
}

/// Monte de 3 points par image et entraîne la caméra (`A5+0x50a`, 16.16).
fn riser_climb(x: &mut Cx) {
    x.add16(Obj::Z, 3u16.wrapping_neg());
    let cam = x.a5_u32(0x50a).wrapping_sub(0x2_1000);
    x.set_a5_u32(0x50a, cam);
}

/// Un projectile (type 50, variante `variant`) part du créateur quand il est touché (`+0x33`).
fn riser_shoot(x: &mut Cx, variant: u8) {
    if x.u8(0x33) == 0 {
        return;
    }
    x.add8(f::SUB_STATE, 2);
    x.set8(0x33, 0);
    let Some(o) = alloc(x, ALLOC_EFFECT) else { return };
    wr8(x.b, o.0, 1);
    wr8(x.b, o.0 + 2, 50);
    wr8(x.b, o.0 + 3, variant);
    if variant == 2 {
        wr16(x.b, o.0 + 0x42, x.o.0 as u16);
        let facing = x.u8(0x2c);
        wr8(x.b, o.0 + 0x2c, facing);
        x.call(0xe5dea);
    } else {
        for field in [Obj::X, Obj::Y, Obj::Z] {
            let v = x.u16(field);
            wr16(x.b, o.0 + field, v);
        }
        let dx: u16 = if x.u8(0x2c) != 0 { 0xfff0 } else { 0x10 };
        let px = rd16(x.b, o.0 + Obj::X).wrapping_add(dx);
        wr16(x.b, o.0 + Obj::X, px);
    }
}

fn riser(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.add16(Obj::Z, 0x32);
            x.call(0x1ac86);
            x.set_a5_u16(0x4be, 0xa);
            x.call(0xead04);
            x.call(0xe5daa);
            x.anim_start(riser::ANIM_SET, 9);
        }
        2 => riser_active(x),
        _ => return free(x),
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

fn riser_active(x: &mut Cx) {
    match x.u8(f::SUB_STATE) {
        0 => {
            if x.anim_step() {
                x.set16(riser::TIMER, 0x28);
                riser_anim(x, 0);
            }
        }
        2 => {
            x.anim_step();
            if !countdown(x, riser::TIMER) {
                riser_anim(x, 1);
            } else if x.u16(riser::TIMER) == 0x27 {
                x.add16(Obj::Z, 0x32u16.wrapping_neg());
            }
        }
        4 => {
            riser_climb(x);
            if (x.u16(Obj::Z) as i16) <= 0x68 {
                riser_anim(x, 2);
            } else {
                x.anim_step();
            }
        }
        6 | 8 => {
            riser_climb(x);
            x.anim_step();
            let variant = if x.u8(f::SUB_STATE) == 6 { 2 } else { 6 };
            riser_shoot(x, variant);
        }
        10 => {
            x.add16(Obj::Z, 3u16.wrapping_neg());
            let cam = x.a5_u16(0x50a).wrapping_sub(2);
            x.set_a5_u16(0x50a, cam);
            if (x.u16(Obj::Z) as i16) <= 0 {
                x.set16(Obj::Z, 0);
                riser_anim(x, 3);
            } else {
                x.anim_step();
            }
        }
        12 => {
            let carrier = x.linked(0x42);
            if carrier.u32(x.b, f::STATE) == 0x0200_042a {
                riser_anim(x, 4);
            } else {
                x.anim_step();
            }
        }
        14 => {
            x.anim_step();
            if x.u8(0x33) != 0 {
                x.add8(f::SUB_STATE, 2);
                x.set16(riser::TIMER, 0x14);
            }
        }
        16 => {
            if countdown(x, riser::TIMER) {
                return;
            }
            x.add8(f::SUB_STATE, 2);
            x.set16(riser::TIMER, 0x50);
            // le morceau projeté (type 39 variante 9) : l'original réessaie jusqu'à trouver une place
            let o = loop {
                if let Some(o) = alloc(x, ALLOC_EFFECT) {
                    break o;
                }
            };
            wr8(x.b, o.0, 1);
            wr8(x.b, o.0 + 2, 39);
            wr8(x.b, o.0 + 3, 9);
            wr16(x.b, o.0 + 0x42, x.o.0 as u16);
            let facing = x.u8(0x2c);
            wr8(x.b, o.0 + 0x2c, facing);
            let (px, py) = (x.u16(Obj::X), x.u16(Obj::Y));
            wr16(x.b, o.0 + Obj::X, px);
            wr16(x.b, o.0 + Obj::Y, py);
            wr16(x.b, o.0 + Obj::Z, 0x66);
            x.call(0xe5c7a);
        }
        18 => {
            if countdown(x, riser::TIMER) {
                return;
            }
            x.set16(riser::TIMER, 0x3c);
            x.call(0xe5cd6);
            riser_anim(x, 5);
        }
        20 => {
            if !countdown(x, riser::TIMER) {
                x.add8(f::SUB_STATE, 2);
                x.set16(riser::TIMER, 0x5a);
            }
            x.anim_step();
        }
        22 => {
            if countdown(x, riser::TIMER) {
                x.anim_step();
                return;
            }
            x.add8(f::SUB_STATE, 2);
            x.set16(Obj::VZ, 0x380);
            x.set16(Obj::AZ, 0xffd0);
            x.set16(Obj::VX, 0x280);
            x.set16(Obj::AX, 0);
            x.set16(Obj::VY, 0xfea0);
            x.set16(Obj::AY, 0);
            x.call(super::cx::sub::MOVE_XYZ);
            x.anim_start(riser::ANIM_SET, 6);
        }
        24 => {
            x.call(super::cx::sub::MOVE_XYZ);
            if (x.u16(Obj::VZ) as i16) > 0 {
                x.anim_step();
            } else {
                riser_anim(x, 7);
            }
        }
        26 => {
            x.call(super::cx::sub::MOVE_XYZ);
            if (x.u16(Obj::Z) as i16) > 0 {
                x.anim_step();
            } else {
                x.set16(Obj::Z, 0);
                riser_anim(x, 8);
            }
        }
        28 => {
            if x.anim_step() {
                riser_anim(x, 0xa);
            }
        }
        _ => {
            x.anim_step();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 44 : effets d'un ennemi magicien (six modes : aura, flammes, rayon, éclat), liés à `+0x5c`
// ---------------------------------------------------------------------------------------------

mod spell {
    pub const ANIM_SET: u32 = 0x194e1e;
    pub const OWNER: u32 = 0x5c;
    /// Décalages (x, z) par variante (4 octets) ; (x, z, demi-tour) par variante (8 octets) ;
    /// animations par variante (octets) ; teintes du clignotement (octets) : espace programme.
    pub const OFFSETS_A: u32 = 0xb5298;
    pub const OFFSETS_B: u32 = 0xb530e;
    pub const ANIMS_A: u32 = 0xb51e4;
    pub const ANIMS_B: u32 = 0xb5200;
    pub const TINTS: u32 = 0xb5224;
}

/// Recopie position et orientation de `from`, un cran devant ou derrière (`dy`).
fn spell_copy(x: &mut Cx, from: Obj) {
    for off in [Obj::X, Obj::Y, Obj::Z] {
        let v = from.u32(x.b, off);
        x.set32(off, v);
    }
    let facing = from.u8(x.b, 0x2c);
    x.set8(0x2c, facing);
}

/// 0xb524a / 0xb52b0 : posé sur le propriétaire, décalé selon la variante (D1, D2, D3 : ceux de l'original).
fn spell_place(x: &mut Cx, wide: bool) {
    let owner = x.linked(spell::OWNER);
    spell_copy(x, owner);
    x.add16(Obj::Y, 0xffff);
    let v = x.u8(f::VARIANT) as u32;
    let (mut dx, dz, turn) = if wide {
        let k = spell::OFFSETS_B + 8 * v;
        (x.rom16(k), x.rom16(k + 2), Some(x.rom16(k + 4)))
    } else {
        let k = spell::OFFSETS_A + 4 * v;
        (x.rom16(k), x.rom16(k + 2), None)
    };
    if x.u8(0x2c) != 0 {
        dx = dx.wrapping_neg();
    }
    if let Some(t) = turn {
        crate::mem::set_low_word(&mut x.c.d[3], t);
        if t != 0 {
            let facing = x.u8(0x2c) ^ 0x20;
            x.set8(0x2c, facing);
        }
    }
    crate::mem::set_low_word(&mut x.c.d[1], dx);
    crate::mem::set_low_word(&mut x.c.d[2], dz);
    x.add16(Obj::X, dx);
    x.add16(Obj::Z, dz);
}

/// Posé sur le porteur du propriétaire (`+0x50` de celui-ci), au sol, juste devant.
fn spell_on_carrier(x: &mut Cx, anim: u32) {
    let owner = x.linked(spell::OWNER);
    let carrier = Obj(owner.u16(x.b, 0x50) as i16 as i32 as u32 & 0xff_ffff);
    spell_copy(x, carrier);
    x.add16(Obj::Y, 1);
    x.set16(Obj::Z, 0);
    x.anim_start(spell::ANIM_SET, anim);
}

/// 0xb5206 : teinte suivante du clignotement.
fn spell_tint(x: &mut Cx) {
    x.set8(0x2a, 1);
    let k = (x.u8(0x46) + 1) & 3;
    x.set8(0x46, k);
    let tint = x.rom8(spell::TINTS + k as u32);
    x.set8(0x2b, tint);
}

fn spell(x: &mut Cx) {
    if x.u16(spell::OWNER) == 0 {
        return x.set32(f::STATE, 0x0600_0000);
    }
    let mode = x.u8(0x2f);
    let v = x.u8(f::VARIANT) as u32;
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            match mode {
                0 | 4 => {
                    x.set16(0x42, if mode == 0 { 0x26 } else { 0xb });
                    spell_place(x, false);
                    let anim = x.rom8(spell::ANIMS_A + v) as u32;
                    x.anim_start(spell::ANIM_SET, anim);
                }
                1 => spell_on_carrier(x, 5),
                5 => spell_on_carrier(x, 7),
                2 => {
                    x.set16(0x42, 0x26);
                    spell_place(x, true);
                    let anim = x.rom8(spell::ANIMS_B + v) as u32;
                    x.anim_start(spell::ANIM_SET, anim);
                }
                3 => {
                    let owner = x.linked(spell::OWNER);
                    spell_copy(x, owner);
                    let facing = x.u8(0x2c) ^ 0x20;
                    x.set8(0x2c, facing);
                    x.add16(Obj::Y, 0xffff);
                    let dx: u16 = if facing == 0 { 0x42 } else { 0x42u16.wrapping_neg() };
                    x.add16(Obj::X, dx);
                    x.add16(Obj::Z, 0x42);
                    // 0xb5228 : palette de l'éclat
                    for k in 0..8 {
                        let w = x.data32(0x25a6e0 + 4 * k);
                        wr32(x.b, 0x91c2e0 + 4 * k, w);
                    }
                    x.set8(0x2a, 1);
                    x.set8(0x2b, 0x17);
                    x.call(0xed2c4);
                    x.anim_start(spell::ANIM_SET, 6);
                }
                _ => return,
            }
            x.reinsert();
        }
        2 => {
            let owner = x.linked(spell::OWNER);
            match mode {
                0 | 2 | 4 => {
                    if owner.u8(x.b, f::STATE) == 4 {
                        free(x);
                    } else {
                        let t = x.u16(0x42).wrapping_sub(1);
                        x.set16(0x42, t);
                        if (t as i16) < 0 {
                            x.add8(f::STATE, 2);
                        }
                        x.anim_step();
                    }
                }
                1 | 3 | 5 => {
                    if owner.u16(x.b, f::STATE) == 0x406 {
                        free(x);
                    } else {
                        if mode != 3 && x.u8(0x33) == 2 {
                            x.set8(0x33, 0);
                            spell_tint(x);
                        }
                        if mode == 1 && x.u8(0x33) == 1 {
                            x.set8(0x33, 0);
                            x.call(0xa31c2);
                            x.call(0xed186);
                        }
                        if x.anim_step() {
                            match mode {
                                1 => x.add8(f::STATE, 2),
                                3 => {
                                    x.add8(f::STATE, 2);
                                    x.call(0xa3354);
                                }
                                _ => {
                                    x.call(0xa5c7c);
                                    x.add8(f::STATE, 2);
                                }
                            }
                        }
                    }
                }
                _ => return,
            }
            // l'original remet l'objet dans la liste même quand il vient d'être libéré
            x.reinsert();
        }
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Types 37 et 38 : panneaux d'un écran de fin d'épreuve (titre qui descend ; message qui tombe,
// clignote et change avec l'état de son créateur)
// ---------------------------------------------------------------------------------------------

fn title_card(x: &mut Cx) {
    let creator = x.linked(0x42);
    let gone = |x: &mut Cx| {
        x.call(0xed90a);
        free(x);
    };
    if creator.u8(x.b, f::STATE) == 4 {
        return gone(x);
    }
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.call(0xed11c);
            x.anim_start(0x1942b2, 0);
        }
        2 => {
            if x.u8(f::SUB_STATE) == 0 {
                x.add16(Obj::Z, 8u16.wrapping_neg());
                if (x.u16(Obj::Z) as i16) <= 0x55 {
                    x.add8(f::SUB_STATE, 2);
                    x.set16(Obj::Z, 0x55);
                }
                x.anim_step();
            } else if creator.u32(x.b, f::STATE) == 0x0204_1008 {
                gone(x);
            } else {
                x.anim_step();
            }
        }
        _ => gone(x),
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

/// 0xb3556 : affiché une image sur deux.
fn blink_frames(x: &mut Cx) {
    let hidden = x.a5_u16(0x6e) & 1 == 0;
    set_hidden(x, hidden);
}

fn result_card(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let (cx_, cy) = (x.a5_u16(0x5a6).wrapping_add(0xc0), x.a5_u16(0x5aa).wrapping_add(0x68));
            x.set16(Obj::X, cx_);
            x.set16(Obj::Y, cy);
            x.set16(Obj::Z, 0x80);
            x.set16(Obj::VZ, 0xf000);
            x.call(0xed11c);
            x.anim_start(0x19431a, 0);
        }
        2 => {
            let creator = x.linked(0x42);
            match x.u8(f::SUB_STATE) {
                0 => {
                    x.call(super::cx::sub::MOVE_Z);
                    if (x.u16(Obj::Z) as i16) <= 0 {
                        x.add8(f::SUB_STATE, 2);
                        x.set16(Obj::Z, 0);
                    }
                    x.anim_step();
                }
                2 => {
                    if creator.u32(x.b, f::STATE) == 0x0200_0200 {
                        x.add8(f::SUB_STATE, 2);
                        blink_frames(x);
                        x.call(0xed056);
                        x.anim_start(0x19431a, 1);
                    } else {
                        x.anim_step();
                    }
                }
                4 => {
                    blink_frames(x);
                    if creator.u32(x.b, f::STATE) == 0x0200_0600 {
                        x.add8(f::SUB_STATE, 2);
                        x.call(0xed90a);
                        x.anim_start(0x19431a, 2);
                    } else {
                        x.anim_step();
                    }
                }
                6 => {
                    blink_frames(x);
                    if x.anim_step() {
                        free(x);
                    }
                }
                _ => {}
            }
        }
        _ => free(x),
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 41 : objet lancé au hasard (quatre sortes), qui retombe puis glisse ou rebondit
// ---------------------------------------------------------------------------------------------

mod toss {
    pub const ANIM_SET: u32 = 0x1944f0;
    /// Par sorte : vitesses (x, az) (mots) et son de départ / d'atterrissage (longs, 0 : aucun),
    /// espace programme.
    pub const SPEEDS: u32 = 0xb3cbc;
    pub const SOUNDS_START: u32 = 0xb3cd0;
    pub const SOUNDS_LAND: u32 = 0xb3d52;
    /// Vitesses (x, az) du rebond selon `+0x33` (programme).
    pub const BOUNCES: u32 = 0xb3dbe;
}

/// Son de la table `table` pour la sorte courante (`jsr (A1)` de l'original, s'il y en a un).
fn toss_sound(x: &mut Cx, table: u32) {
    let k = 4 * x.u8(f::VARIANT) as u32;
    let routine = (x.rom16(table + k) as u32) << 16 | x.rom16(table + k + 2) as u32;
    if routine != 0 {
        x.c.a[1] = routine;
        x.call(routine);
    }
}

fn toss_land(x: &mut Cx) {
    let kind = x.u8(f::VARIANT) as u32;
    x.anim_start(toss::ANIM_SET, 5 + kind);
    toss_sound(x, toss::SOUNDS_LAND);
}

fn toss(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set16(0x5c, 0x20);
            x.call(super::rom::RNG);
            let kind = (x.c.d[0] & 3) as u8;
            x.set8(f::VARIANT, kind);
            let k = 4 * kind as u32;
            let (vx, az) = (x.rom16(toss::SPEEDS + k), x.rom16(toss::SPEEDS + k + 2));
            let vx = if x.u8(0x2c) == 0 { vx.wrapping_neg() } else { vx };
            x.set16(Obj::VX, vx);
            x.set16(Obj::AZ, az);
            toss_sound(x, toss::SOUNDS_START);
            x.anim_start(toss::ANIM_SET, kind as u32);
            x.reinsert();
        }
        2 => match x.u8(f::SUB_STATE) {
            0 => {
                x.anim_step();
                x.call(super::cx::sub::MOVE_Z);
                if x.c.cond(15) {
                    // au sol
                    x.add8(f::SUB_STATE, 4);
                    x.set16(Obj::Z, 0);
                    return toss_land(x);
                }
                if x.u16(Obj::AZ) != 0 {
                    return;
                }
                let t = x.u16(0x5c).wrapping_sub(1);
                x.set16(0x5c, t);
                if (t as i16) >= 0 {
                    return;
                }
                x.add8(f::SUB_STATE, 8);
                toss_land(x);
            }
            4 => {
                if x.u8(0x01) == 0 {
                    return free(x);
                }
                x.reinsert();
                x.anim_step();
                if x.u8(0x33) != 0 {
                    x.call(super::cx::sub::MOVE_X);
                }
            }
            8 => {
                if x.u8(0x01) == 0 {
                    return free(x);
                }
                x.anim_step();
                let k = 4 * x.u8(0x33) as u32;
                let (vx, az) = (x.rom16(toss::BOUNCES + k), x.rom16(toss::BOUNCES + k + 2));
                let vx = if x.u8(0x2c) == 0 { vx.wrapping_neg() } else { vx };
                x.set16(Obj::AX, vx);
                x.set16(Obj::AZ, az);
                x.call(super::cx::sub::MOVE_XZ);
            }
            _ => {}
        },
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 3 : élément d'écran fixé à la caméra (hauteur suivant le tremblement `A5+0x4ba`)
// ---------------------------------------------------------------------------------------------

fn screen_piece(x: &mut Cx) {
    let follow = |x: &mut Cx| {
        let z = x.a5_u16(0x50a).wrapping_sub(x.a5_u16(0x4ba)).wrapping_add(x.u16(0x50));
        x.set16(Obj::Z, z);
    };
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let v = x.u8(f::VARIANT) as u32;
            let base: u8 = if v == 0 { 0xc } else { 0 };
            let (px, height) = (x.rom16(0xaa944 + 4 * v), x.rom16(0xaa946 + 4 * v));
            x.set16(Obj::X, px);
            x.set16(0x50, height);
            follow(x);
            let anim = base.wrapping_add(x.u8(0x2f));
            x.anim_start(0x18c6ec, anim as u32);
            x.call(super::cx::sub::REINSERT_FOREGROUND);
        }
        2 => {
            follow(x);
            x.anim_step();
        }
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 76 : minuteur de scène (joue un son, puis lance la transition 0x15 ou 0x14 après un délai)
// ---------------------------------------------------------------------------------------------

fn scene_timer(x: &mut Cx) {
    let v = x.u8(f::VARIANT);
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let (delay, sound) = if v == 0 { (0x168, 0xeac08) } else { (0x10e, 0xeabac) };
            x.set16(0x30, delay);
            x.call(sound);
        }
        2 => {
            if countdown(x, 0x30) {
                return;
            }
            x.c.d[4] = if v == 0 { 0x15 } else { 0x14 };
            x.call(0xe17a4);
            free(x);
        }
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 69 : silhouette qui fait les cent pas en haut de l'écran ; type 64 : éclat projeté qui
// retombe sous l'horizon
// ---------------------------------------------------------------------------------------------

fn pacer(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set8(0x2c, 0x20);
            x.set16(Obj::X, 0xc0);
            x.set16(Obj::Z, 0);
            x.set16(Obj::Y, 0x58);
            x.anim_start(0x1c03a6, 0xbc);
            x.call(super::cx::sub::REINSERT_FOREGROUND);
        }
        2 => {
            x.anim_step();
            if x.u8(0x2c) == 0 {
                x.add32(Obj::X, 0x1_0000u32.wrapping_neg());
                if (x.u16(Obj::X) as i16) <= 0xc0 {
                    x.set8(0x2c, 0x20);
                }
            } else {
                x.add32(Obj::X, 0x1_0000);
                if (x.u16(Obj::X) as i16) >= 0x1c0 {
                    x.set8(0x2c, 0);
                }
            }
        }
        _ => free(x),
    }
}

fn spark_fall(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let r = random_nibble16(x) & 0x1f;
            x.add16(Obj::Z, r);
            let r = random_nibble16(x) & 0x3ff;
            x.set16(Obj::VX, r.wrapping_sub(0x200));
            x.set16(Obj::AX, 0);
            let r = random_nibble16(x) & 0x3ff;
            x.set16(Obj::VZ, r);
            x.set16(Obj::AZ, 0xffe0);
            x.anim_start(0x197a5c, 1);
            x.reinsert();
        }
        2 => {
            x.anim_step();
            x.call(super::cx::sub::MOVE_XZ);
            if x.c.sr & m68k::SR_N == 0 {
                return;
            }
            if x.u8(f::VARIANT) == 0 {
                return free(x);
            }
            x.add8(f::STATE, 2);
            x.set16(0x42, 0x40);
            let anim = (x.u8(0x2f) ^ 1) as u32 + 1;
            x.anim_start(0x197a5c, anim);
            spark_fall_wait(x);
        }
        4 => spark_fall_wait(x),
        _ => free(x),
    }
}

fn spark_fall_wait(x: &mut Cx) {
    let t = x.u16(0x42).wrapping_sub(1);
    x.set16(0x42, t);
    if (t as i16) < 0 {
        free(x);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 84 : déclencheur d'une séquence de fin de scène (fondu, caméra, tâches de script) ; type 90 :
// poussière qui dérive en haut de l'écran
// ---------------------------------------------------------------------------------------------

/// Lance la tâche de script `entry` (0x9b6, `trap #0`), A0 conservé.
fn start_script(x: &mut Cx, entry: u32) {
    let a0 = x.c.a[0];
    x.c.a[0] = entry;
    x.c.d[0] = x.c.d[0] & 0xffff_0000 | 0x10;
    crate::call(x.c, x.b, 0x9b6);
    x.c.a[0] = a0;
}

fn finale_trigger(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            finale_step(x);
        }
        2 => finale_step(x),
        4 => {
            x.set8(0x2f, 1);
            x.set16(f::STATE, 0x600);
        }
        _ => free(x),
    }
}

fn finale_step(x: &mut Cx) {
    let mut sub = x.u8(f::SUB_STATE);
    if sub == 0 {
        x.add8(f::SUB_STATE, 4);
        x.set16(0x42, 0xc0);
        sub = 4;
    }
    match sub {
        4 => {
            let t = x.u16(0x42).wrapping_sub(1);
            x.set16(0x42, t);
            if (t as i16) >= 0 {
                return;
            }
            x.add8(f::SUB_STATE, 4);
            x.call(0xe633c);
            x.set_a5_u16(0x490, 1);
            x.set_a5_u8(0x48e, 8);
            x.set_a5_u16(0x49e, 0xf20);
            start_script(x, 0x1ce64);
        }
        8 => {
            if x.a5_u8(0x48f) != 0 {
                return;
            }
            x.add8(f::SUB_STATE, 4);
            x.set_a5_u16(0x556, 0x440);
            x.set_a5_u16(0x55a, 0x38b);
            x.set_a5_u16(0x490, 4);
            x.set_a5_u8(0x48e, 8);
            x.set_a5_u8(0x48f, 1);
            let d7 = x.c.d[7];
            x.call(0x1ad6a);
            x.c.d[7] = d7;
            x.set_a5_u32(0x49e, 0xff1190);
            start_script(x, 0x1cf12);
            finale_end(x);
        }
        12 => finale_end(x),
        _ => {}
    }
}

/// Fin : une fois le fondu terminé, crée l'objet BOX 0x14 et se met en attente (état 6).
fn finale_end(x: &mut Cx) {
    if x.a5_u8(0x48f) != 0 {
        return;
    }
    x.call(0x23a8);
    if x.c.sr & m68k::SR_C != 0 {
        return;
    }
    let o = x.c.a[1];
    wr8(x.b, o, 1);
    wr8(x.b, o + 2, 0x14);
    x.set8(0x2f, 1);
    x.set16(f::STATE, 0x600);
}

fn drift_dust(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let mut px = (random_nibble16(x) & 0xff).wrapping_add(x.a5_u16(0x506));
            if x.u8(0x2c) != 0 {
                px = px.wrapping_add(0x80);
            }
            x.set16(Obj::X, px);
            let py = (random_nibble16(x) & 0x3f).wrapping_add(x.a5_u16(0x50a)).wrapping_add(0x20);
            x.set16(Obj::Y, py);
            x.set16(Obj::VZ, 0);
            x.set16(Obj::AZ, 0x10);
            x.set16(Obj::VX, 0);
            let ax: u16 = if x.u8(0x2c) != 0 { 0xffc0 } else { 0x40 };
            x.set16(Obj::AX, ax);
            x.anim_start(0x1901ba, 0);
            x.reinsert();
        }
        2 => {
            x.call(super::cx::sub::MOVE_XZ);
            if x.anim_step() {
                free(x);
            }
        }
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 66 : animation de quatre gros caractères dans le fond 2 (une étape toutes les 0x68 images)
// ---------------------------------------------------------------------------------------------

/// Dessine quatre blocs de 2 × 2 tuiles (`tile`, `tile + 1` dessous, `tile + 0x10` à droite...) en
/// colonnes espacées de 0x100.
fn draw_four_blocks(x: &mut Cx, tiles: [u16; 4], attr: u16) {
    for (k, tile) in tiles.into_iter().enumerate() {
        let at = 0x912220 + 0x100 * k as u32;
        for (off, t) in [(0, tile), (0x80, tile.wrapping_add(1)), (4, tile.wrapping_add(0x10)), (0x84, tile.wrapping_add(0x11))] {
            wr16(x.b, at + off, t);
            wr16(x.b, at + off + 2, attr);
        }
    }
}

fn big_letters(x: &mut Cx) {
    if x.u8(Obj::HIDDEN) != 0 {
        // caché : efface une fois les quatre blocs et retarde l'étape suivante
        if x.u8(0x41) != 0 {
            return;
        }
        x.add8(0x41, 1);
        x.add16(0x42, 0x40);
        return draw_four_blocks(x, [0x2fc; 4], 0);
    }
    match x.u8(f::STATE) {
        0 | 2 => {
            if x.u8(f::STATE) == 0 {
                x.add8(f::STATE, 2);
                x.set16(0x42, 0x67);
                x.set32(0x5c, 0xb7d84);
            }
            let t = x.u16(0x42).wrapping_sub(1);
            x.set16(0x42, t);
            if (t as i16) >= 0 {
                return;
            }
            x.set16(0x42, 0x67);
            let p = x.u32(0x5c);
            let first = x.data16(p);
            if (first as i16) < 0 {
                return free(x);
            }
            let (b, c, d, attr) = (x.data16(p + 2), x.data16(p + 4), x.data16(p + 6), x.data16(p + 8));
            x.set32(0x5c, p + 10);
            draw_four_blocks(x, [first, b, c, d], attr);
        }
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 91 : autre élément d'écran fixé à la caméra ; type 47 : repère qui clignote lentement
// ---------------------------------------------------------------------------------------------

fn screen_piece_b(x: &mut Cx) {
    let follow = |x: &mut Cx| {
        let z = x.a5_u16(0x50a).wrapping_sub(x.a5_u16(0x4ba)).wrapping_add(x.u16(0x50));
        x.set16(Obj::Z, z);
    };
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let (px, height, anim) = if x.u8(0x2f) != 0 {
                (0xc0, 0x1a0, 0x170)
            } else {
                let v = x.u8(f::VARIANT) as u32;
                (x.rom16(0xba2a0 + 4 * v), x.rom16(0xba2a2 + 4 * v), 0x1b7)
            };
            x.set16(Obj::X, px);
            x.set16(0x50, height);
            follow(x);
            x.anim_start(0x1c03a6, anim);
            x.reinsert();
        }
        2 => {
            follow(x);
            x.anim_step();
        }
        _ => free(x),
    }
}

fn beacon(x: &mut Cx) {
    if x.u8(Obj::HIDDEN) != 0 {
        return free(x);
    }
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set16(0x5c, 0);
            let k = ((x.u8(f::VARIANT) as i8 as i16 as u16) << 3) as i16 as i32 as u32;
            x.c.d[4] = k & 0xffff;
            let base = 0xb57ecu32.wrapping_add(k);
            for (i, field) in [Obj::X, Obj::Z, Obj::Y].into_iter().enumerate() {
                let d = x.rom16(base + 2 * i as u32);
                x.add16(field, d);
            }
            let anim = x.rom16(base + 6);
            let anim = anim & 0xff00 | (anim as u8).wrapping_add(x.u8(0x2f)) as u16;
            x.anim_start(0x19610c, anim as u32);
            x.reinsert();
        }
        2 => {
            let t = x.u16(0x5c).wrapping_sub(1);
            x.set16(0x5c, t);
            if (t as i16) < 0 {
                x.set16(0x5c, 0x10);
                toggle_hidden(x);
            }
        }
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 10 : dalles d'une rangée en diagonale, qui s'enfoncent quand un joueur au sol marche dessus
// ---------------------------------------------------------------------------------------------

mod tiles {
    /// X de chaque dalle (mots, espace programme) ; variantes du décor SET 6 posé à l'appui et au
    /// relâchement (octets, espace programme).
    pub const POSITIONS: u32 = 0xabd28;
    pub const PRESSED: u32 = 0xabe7e;
    pub const RELEASED: u32 = 0xabe8e;
    /// Voisines (pointeurs de 16 bits) : précédente `+0x30`, suivante `+0x32` ; appuyée `+0x34`.
    pub const PREVIOUS: u32 = 0x30;
    pub const NEXT: u32 = 0x32;
    pub const DOWN: u32 = 0x34;
}

/// Dalle suivante (type 10, variante + 1), jusqu'à la neuvième.
fn floor_tile(x: &mut Cx) {
    let v = x.u8(f::VARIANT);
    match x.u8(f::STATE) {
        0 => {
            x.set8(tiles::DOWN, 0);
            x.set16(Obj::Y, 0x70);
            let px = x.rom16(tiles::POSITIONS + 2 * v as u32);
            x.set16(Obj::X, px);
            if (v as i8) < 9 {
                let Some(o) = alloc(x, ALLOC_EFFECT) else { return };
                x.set16(tiles::NEXT, o.0 as u16);
                let a = rd8(x.b, o.0).wrapping_add(1);
                wr8(x.b, o.0, a);
                wr8(x.b, o.0 + 2, 10);
                wr8(x.b, o.0 + 3, v.wrapping_add(1));
                wr16(x.b, o.0 + tiles::PREVIOUS, x.o.0 as u16);
            }
            x.add8(f::STATE, 2);
        }
        2 => floor_tile_watch(x, v),
        _ => free(x),
    }
}

fn floor_tile_watch(x: &mut Cx, v: u8) {
    // une voisine déjà enfoncée bloque la dalle
    if v != 0 {
        let prev = x.linked(tiles::PREVIOUS);
        if prev.u8(x.b, tiles::DOWN) != 0 {
            return;
        }
    }
    if v != 9 {
        let next = x.linked(tiles::NEXT);
        if next.u8(x.b, tiles::DOWN) != 0 {
            return;
        }
    }
    let tx = x.u16(Obj::X);
    let mut stepped = false;
    for k in 0..4 {
        let p = Obj(A5 + 0x5de + 0x200 * k);
        if !p.is_active(x.b) || p.u16(x.b, Obj::Z) != 0 {
            continue;
        }
        // la rangée est en diagonale : le décalage x suit la profondeur
        let d = p.u16(x.b, Obj::Y).wrapping_sub(0x1a8);
        if d > 0x27 {
            continue;
        }
        let px = p.u16(x.b, Obj::X) as i16;
        let left = tx.wrapping_sub(d).wrapping_sub(8);
        x.c.d[4] = 0;
        if (left as i16) > px || (left.wrapping_add(0x10) as i16) <= px {
            continue;
        }
        x.c.d[4] = 1;
        stepped = true;
        break;
    }
    let down = x.u8(tiles::DOWN) != 0;
    if stepped == down {
        return;
    }
    let Some(o) = alloc(x, 0x2348) else { return };
    let a = rd8(x.b, o.0).wrapping_add(1);
    wr8(x.b, o.0, a);
    wr8(x.b, o.0 + 2, 6);
    let table = if stepped { tiles::PRESSED } else { tiles::RELEASED };
    let variant = x.rom8(table + v as u32);
    wr8(x.b, o.0 + 3, variant);
    wr8(x.b, o.0 + 0x2f, 0);
    if stepped {
        x.add8(tiles::DOWN, 1);
    } else {
        x.set8(tiles::DOWN, 0);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 65 : objet qui suit un trajet (directions et durées lues dans une liste), puis fait naître
// un autre objet décrit par une fiche
// ---------------------------------------------------------------------------------------------

mod courier {
    pub const ANIM_SET: u32 = 0x197afe;
    /// Trajet : couples (direction, durée) terminés par une direction négative (données).
    pub const PATH: u32 = 0xb7b1e;
    /// Fiches de 8 octets par variante : animation, type, variante, mode, x, z (données).
    pub const CARDS: u32 = 0xb7bec;
}

fn courier(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set32(0x5c, courier::PATH);
            x.set8(0x42, 1);
            x.c.d[0] = 0x15;
            x.call(super::cx::sub::SPEED_TABLE_B);
            let card = courier::CARDS + 8 * x.u8(f::VARIANT) as u32;
            let anim = x.data8(card) as u32;
            x.anim_start(courier::ANIM_SET, anim);
            x.call(super::cx::sub::REINSERT_FOREGROUND);
        }
        2 => {
            x.call(super::cx::sub::MOVE_TABLE_XY);
            x.anim_step();
            let t = x.u8(0x42).wrapping_sub(1);
            x.set8(0x42, t);
            if t != 0 {
                return;
            }
            let p = x.u32(0x5c);
            let dir = x.data8(p);
            x.set8(Obj::SPEED_INDEX, dir);
            if (dir as i8) < 0 {
                x.add8(f::STATE, 2);
                x.set16(0x42, 0x20);
                return x.call(0x1f0be);
            }
            let len = x.data8(p + 1);
            x.set8(0x42, len);
            x.set32(0x5c, p + 2);
        }
        4 => {
            if x.u8(f::SUB_STATE) == 0 {
                let t = x.u16(0x42).wrapping_sub(1);
                x.set16(0x42, t);
                if (t as i16) < 0 {
                    x.add8(f::SUB_STATE, 1);
                }
                return;
            }
            x.call(0x22bc);
            if x.c.sr & m68k::SR_C != 0 {
                return;
            }
            let o = x.c.a[1];
            x.add8(f::STATE, 2);
            let card = courier::CARDS + 8 * x.u8(f::VARIANT) as u32;
            let link = x.u16(0x46);
            wr16(x.b, o + 0x46, link);
            wr8(x.b, o, 1);
            for (k, off) in [(1, 2), (2, 3), (3, 0x2f)] {
                let v = x.data8(card + k);
                wr8(x.b, o + off, v);
            }
            wr8(x.b, o + 0x2c, 0x20);
            let cam = x.a5_u16(0x506);
            let (px, pz) = (x.data16(card + 4), x.data16(card + 6));
            wr16(x.b, o + Obj::X, px.wrapping_add(cam));
            wr16(x.b, o + Obj::Z, pz);
            let r = random_nibble16(x) & 0x1f;
            wr16(x.b, o + Obj::Y, r + 0x50);
        }
        _ => free(x),
    }
}

handlers! {
    0xaa8e8 "effct_ecran_fixe" 150 screen_piece,
    0xabccc "effct_dalle" 200 floor_tile,
    0xad476 "effct_maillon" 200 chain_link,
    0xae52a "effct_double" 200 double,
    0xaed4c "effct_bourse" 400 purse,
    0xaf3f2 "effct_boutique" 300 shop,
    0xafe20 "effct_vote" 300 vote,
    0xb1818 "effct_poisson" 200 fish,
    0xb2dea "effct_plateau" 200 ferry,
    0xb3050 "effct_flotteur" 200 floater,
    0xb33ba "effct_titre" 200 title_card,
    0xb3454 "effct_resultat" 200 result_card,
    0xb3570 "effct_projete" 300 lob,
    0xb37ac "effct_coup" 200 blow,
    0xb3c48 "effct_lance" 200 toss,
    0xb40f8 "effct_decor" 200 scenery,
    0xb4f88 "effct_sort" 300 spell,
    0xb5764 "effct_repere" 150 beacon,
    0xb5dbe "effct_accessoire_porte" 200 prop,
    0xb5fb6 "effct_attache" 200 attached,
    0xb6d48 "effct_machine" 200 machine,
    0xb6ef2 "effct_surgissant" 200 riser,
    0xb7248 "effct_double_b" 200 double2,
    0xb751e "effct_nageur" 200 swimmer,
    0xb7a14 "effct_eclat_tombant" 200 spark_fall,
    0xb7ac8 "effct_messager" 200 courier,
    0xb7c40 "effct_lettres" 200 big_letters,
    0xb7e26 "effct_colonne" 200 pillar,
    0xb802a "effct_silhouette" 150 pacer,
    0xb80bc "effct_comptoir" 300 counter,
    0xb88a8 "effct_minuteur" 150 scene_timer,
    0xb97d8 "effct_eclat_pose" 200 spark,
    0xb997c "effct_pluie" 200 shard_rain,
    0xb9aba "effct_finale" 200 finale_trigger,
    0xb9bbc "effct_piece_fin" 200 ending_piece,
    0xb9ca6 "effct_flammeche" 200 ending_wisp,
    0xb9db8 "effct_interrupteur" 300 barrier_switch,
    0xb9f36 "effct_barriere" 200 barrier,
    0xba1aa "effct_poussiere_haute" 150 drift_dust,
    0xba244 "effct_ecran_fixe_b" 150 screen_piece_b,
}
