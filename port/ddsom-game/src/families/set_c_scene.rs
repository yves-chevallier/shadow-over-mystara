//! Type SET 2 (0xbc814) : le contrôleur de scène (h).
//!
//! Il y en a un par niveau (créé par le type 8, `scene_spawner`). Sa variante `+3` choisit un
//! *script d'événements* en ROM (table de pointeurs 0xbd276) qui fait apparaître, au fil du temps,
//! les ennemis, objets et décors de la scène. Beaucoup de variantes n'ont pas de script utile : ce
//! sont des marqueurs qui ouvrent ou ferment une scène (intertitre, rideau, fondu, sortie des
//! joueurs...).
//!
//! Machine à états (`+4`) :
//! * 0 : initialisation. Charge le script de la variante, appelle 0xd9778, puis une table d'octets
//!   (0xbc880) indexée par la variante donne l'action de départ (12 actions, voir `start`).
//! * 2 : interpréteur du script d'événements (voir `script`), actif en niveau 4 ou en phase 0xc.
//! * 4 : fin de scène : attend que tous les objets créés aient disparu, puis une table d'octets
//!   (0xbcba8) donne l'action de fin (16 actions, voir `end_action`).
//! * 6 : libération de l'objet.
//!
//! Format d'une entrée de script (0x14 octets, fin = mot 0x8000) :
//! `+0` mot : attente avant l'entrée suivante (bit 15 nul : autant d'images + 1 ; bit 15 levé : que le
//! nombre d'objets vivants créés par le script retombe à l'octet bas, dont le bit 7 demande en plus un
//! déblocage extérieur de `+0x2e`), `+2` mot : rang maximal, `+4/+6/+8` : X, Z, Y, `+0xa` : difficulté
//! minimale, `+0xb` : déplacement dans la table 0xbcb4e des allocateurs (classe de l'objet), `+0xc` :
//! type, `+0xd` : variante, `+0xe` : `+0x2f`, `+0xf` : `+0x2c`, `+0x10` : mot `+0x42`, `+0x12` : non
//! nul = coordonnées absolues (sinon relatives à la caméra).

use crate::families::cx::{sub, Cx};
use crate::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use crate::objects::pools::{BOSS, EFFCT, ENEMY, ESHL, ITEM, PEFF, SET, SOBJ};
use crate::objects::Obj;
use m68k::SR_C;

/// Champs du contrôleur.
mod f {
    /// Adresse de la prochaine entrée du script (long).
    pub const CURSOR: u32 = 0x20;
    /// Images restantes avant la fin de la scène (le script s'arrête en dessous de zéro).
    pub const TIMER: u32 = 0x24;
    /// Images à attendre avant l'entrée suivante (-1 : attente par comptage d'objets).
    pub const WAIT: u32 = 0x26;
    /// Nombre d'objets vivants toléré avant l'entrée suivante (-1 : attente en images).
    pub const LIVE_LIMIT: u32 = 0x18;
    /// Nombre d'objets créés par le script et encore vivants (mot).
    pub const LIVE: u32 = 0x2a;
    /// Octet : attend le déblocage extérieur de l'entrée.
    pub const WAIT_FLAG: u32 = 0x2e;
    /// Octet : le script est terminé ou interrompu.
    pub const DONE: u32 = 0x2c;
    /// Octet : suspension du script par une entrée extérieure.
    pub const HOLD: u32 = 0x2d;
}

/// Variables de la RAM de travail (offsets depuis A5).
mod a5 {
    pub const PHASE: u32 = 0x2c;
    /// Niveau en cours.
    pub const STAGE: u32 = 0x34;
    /// Difficulté (0 : le contrôleur ne travaille pas).
    pub const DIFFICULTY: u32 = 0x35;
    pub const USE_ALT_CAMERA: u32 = 0x36;
    pub const CAMERA: u32 = 0x4fe;
    pub const ALT_CAMERA: u32 = 0x54e;
    /// Non nul : la scène est figée (les scripts attendent).
    pub const FROZEN: u32 = 0x42;
    pub const SCENE_LATCH: u32 = 0x43;
    pub const PLAYERS_LOCK: u32 = 0x4a;
    /// Interdit de lancer le boss.
    pub const NO_BOSS: u32 = 0x50;
    /// Nombre de contrôleurs actifs.
    pub const SCENES: u32 = 0x51;
    pub const TRANSITION_FLAG: u32 = 0x58;
    pub const SCENE_SYNC: u32 = 0x5f;
    pub const TRANSITION: u32 = 0x51e;
    /// Rang (difficulté dynamique), mot.
    pub const RANK: u32 = 0x7f92;
}

/// Octet de l'écran de fond à A5 - 0x5b8e.
const PARALLAX_FLAG: u32 = 0xff2472;
/// Joueurs : quatre enregistrements de 0x200 octets.
const PLAYERS: u32 = A5 + 0x5de;
const PLAYER_STRIDE: u32 = 0x200;

// Tables en ROM.
/// Pointeurs des scripts d'événements, un long par variante.
const SCRIPT_TABLE: u32 = 0xbd276;
/// Action de départ par variante (octets).
const START_ACTION: u32 = 0xbc880;
/// Action de fin par variante (octets).
const END_ACTION: u32 = 0xbcba8;
/// Allocateurs de la classe de l'objet créé (longs ; index = octet `+0xb` de l'entrée).
const ALLOC_TABLE: u32 = 0xbcb4e;
/// Classe « objets ramassables » dans cette table : l'objet n'est pas compté dans `LIVE`.
const ITEM_CLASS: u8 = 0x10;
/// Rideau : enregistrements de 6 octets (décalage dans la palette, adresse de GFX RAM).
const CURTAIN_TABLE: u32 = 0xbcea4;
const CURTAIN_PALETTE: u32 = 0xbcfc6;

// Routines du moteur appelées.
const INIT_SCENE: u32 = 0xd9778;
const EXIT_SCENE: u32 = 0xd981a;
const PREPARE_EFFECTS: u32 = 0xebe86;
const FADE_OUT: u32 = 0xe0c5c;
const PLAYER_HOOK_A: u32 = 0x19db4;
const PLAYER_HOOK_B: u32 = 0x19e10;
const ALLOC_EFFCT: u32 = 0x2318;
const ALLOC_BOSS: u32 = 0x23a8;

pub fn run(x: &mut Cx) {
    if x.u8(0x40) != 0 || x.a5_u8(a5::DIFFICULTY) == 0 {
        return;
    }
    match x.u8(0x4) {
        0 => init(x),
        2 => script(x),
        4 => finish(x),
        6 => x.call(sub::FREE_SET),
        _ => {}
    }
}

/// Alloue un objet avec l'allocateur `allocator` ; `None` si le pool est plein.
fn alloc(x: &mut Cx, allocator: u32) -> Option<Obj> {
    x.call(allocator);
    (x.c.sr & SR_C == 0).then(|| Obj(x.c.a[1]))
}

/// Alloue et active un objet de type `kind` (et de variante `variant`).
fn spawn(x: &mut Cx, allocator: u32, kind: u8, variant: Option<u8>) -> Option<Obj> {
    let o = alloc(x, allocator)?;
    o.set_u8(x.b, 0, 1);
    o.set_u8(x.b, 2, kind);
    if let Some(v) = variant {
        o.set_u8(x.b, 3, v);
    }
    Some(o)
}

fn next_state(x: &mut Cx, by: u8) {
    x.add8(0x4, by);
}

// --- état 0 : initialisation ----------------------------------------------------------------

fn init(x: &mut Cx) {
    let n = x.a5_u8(a5::SCENES).wrapping_add(1);
    x.set_a5_u8(a5::SCENES, n);
    x.set16(f::LIVE_LIMIT, 0xffff);
    x.set16(f::LIVE, 0);
    x.set8(f::DONE, 0);
    x.set8(f::HOLD, 0);
    let variant = x.u8(0x3) as u32;
    let script = x.data32(SCRIPT_TABLE + 4 * variant);
    let time = x.data16(script);
    x.set16(f::TIMER, time);
    x.set32(f::CURSOR, script + 2);
    x.call(INIT_SCENE);
    let action = x.rom8(START_ACTION + variant);
    start(x, action);
}

/// Action de départ (0 à 11), choisie par la variante.
fn start(x: &mut Cx, action: u8) {
    match action {
        // 10 et 11 : sans partie en cours (A5+0x66 nul), le contrôleur se libère
        10 | 11 if x.a5_u8(0x66) == 0 => x.call(sub::FREE_SET),
        10 | 0 => {
            x.set_a5_u16(a5::TRANSITION, 4);
            next_state(x, 2);
            x.set_a5_u8(a5::TRANSITION_FLAG, 1);
        }
        11 | 1 | 2 | 6 => next_state(x, 2),
        5 => {
            x.set_a5_u16(a5::TRANSITION, 4);
            x.set_a5_u8(a5::TRANSITION_FLAG, 1);
            next_state(x, 2)
        }
        // saute directement à la libération
        7 => next_state(x, 6),
        8 => {
            x.set_a5_u16(0x46, 0);
            next_state(x, 2)
        }
        3 => {
            next_state(x, 2);
            x.set_a5_u16(a5::TRANSITION, 0x30);
            let v = x.a5_u16(0x556);
            x.set_a5_u16(0x574, v);
        }
        4 => opening_wipe(x),
        9 => {
            x.set_a5_u16(a5::TRANSITION, 0x20);
            next_state(x, 2);
            x.set_a5_u8(a5::TRANSITION_FLAG, 1);
        }
        _ => unreachable!("table de départ : 0 à 11"),
    }
}

/// Action 4 : efface la tilemap de l'écran par colonnes (une de chaque côté du centre par image,
/// 11 images), puis passe à l'état 2.
fn opening_wipe(x: &mut Cx) {
    const LEFT: u32 = 0x91203c;
    const RIGHT: u32 = 0x912040;
    let column = x.u16(f::WAIT) as i16;
    if column > 10 {
        next_state(x, 2);
        x.set_a5_u8(a5::SCENE_SYNC, 1);
        wr8(x.b, PARALLAX_FLAG, 0);
        return;
    }
    let step = column.wrapping_mul(4) as i32 as u32;
    clear_column(x, LEFT.wrapping_sub(step));
    x.add16(f::WAIT, 1);
    clear_column(x, RIGHT.wrapping_add(step));
}

/// Remplit de blanc une colonne de la tilemap (48 lignes de 0x80 octets).
fn clear_column(x: &mut Cx, top: u32) {
    for row in 0..0x30 {
        wr32(x.b, top.wrapping_add(row * 0x80), 0x100000);
    }
}

// --- état 2 : interpréteur du script d'événements -----------------------------------------------

fn script(x: &mut Cx) {
    if x.a5_u8(a5::STAGE) != 4 && x.a5_u8(a5::PHASE) != 0xc {
        return;
    }
    if x.a5_u8(a5::FROZEN) != 0 || x.u8(f::HOLD) != 0 {
        return end_of_script(x, true);
    }
    x.add16(f::TIMER, 0xffff);
    if x.u16(f::TIMER) & 0x8000 != 0 {
        return end_of_script(x, true);
    }
    if x.u8(f::WAIT_FLAG) != 0 {
        return;
    }
    let limit = x.u16(f::LIVE_LIMIT) as i16;
    if limit >= 0 {
        if limit < x.u16(f::LIVE) as i16 {
            return;
        }
    } else {
        x.add16(f::WAIT, 0xffff);
        if x.u16(f::WAIT) & 0x8000 == 0 {
            return;
        }
    }
    next_entry(x);
}

/// Fin du script (`done` : interrompu plutôt qu'épuisé) : passe à l'état 4.
fn end_of_script(x: &mut Cx, done: bool) {
    if done {
        x.set8(f::DONE, 1);
    }
    next_state(x, 2);
    x.set16(f::WAIT, 0);
}

/// Saute les entrées hors de la difficulté ou du rang courants, exécute la première convenable.
fn next_entry(x: &mut Cx) {
    loop {
        let entry = x.u32(f::CURSOR);
        let word = x.data16(entry);
        if word == 0x8000 {
            return end_of_script(x, false);
        }
        let min_difficulty = x.data8(entry + 0xa) as i8;
        let max_rank = x.data16(entry + 2) as i16;
        if min_difficulty > x.a5_u8(a5::DIFFICULTY) as i8 || max_rank > x.a5_u16(a5::RANK) as i16 {
            x.set32(f::CURSOR, entry + 0x14);
            continue;
        }
        return run_entry(x, entry, word);
    }
}

/// Exécute une entrée : règle l'attente puis crée l'objet.
fn run_entry(x: &mut Cx, entry: u32, word: u16) {
    x.set8(f::WAIT_FLAG, 0);
    x.set16(f::LIVE_LIMIT, 0xffff);
    x.set16(f::WAIT, word);
    if word & 0x8000 != 0 {
        let low = word & 0xff;
        if low & 0x80 != 0 {
            x.set8(f::WAIT_FLAG, 1);
        }
        x.set16(f::LIVE_LIMIT, low);
    }
    let class_offset = x.data8(entry + 0xb);
    let allocator = x.data32(ALLOC_TABLE + class_offset as u32);
    let Some(o) = alloc(x, allocator) else {
        x.set16(f::LIVE_LIMIT, 0xffff);
        x.set16(f::WAIT, 0);
        return;
    };
    o.set_u8(x.b, 0, 1);
    for (to, from) in [(2, 0xc), (3, 0xd), (0x2f, 0xe), (0x2c, 0xf)] {
        let v = x.data8(entry + from);
        o.set_u8(x.b, to, v);
    }
    for (to, from) in [(0x8, 4), (0xc, 6), (0x10, 8), (0x42, 0x10)] {
        let v = x.data16(entry + from);
        o.set_u16(x.b, to, v);
    }
    if class_offset != ITEM_CLASS {
        x.add16(f::LIVE, 1);
        o.set_u16(x.b, 0x46, x.o.0 as u16);
    }
    if x.data8(entry + 0x12) == 0 {
        let cam = if x.a5_u8(a5::USE_ALT_CAMERA) != 0 { a5::ALT_CAMERA } else { a5::CAMERA };
        let (cam_x, cam_y) = (x.a5_u16(cam + 8), x.a5_u16(cam + 0xc));
        let (px, py) = (o.u16(x.b, 0x8), o.u16(x.b, 0x10));
        o.set_u16(x.b, 0x8, px.wrapping_add(cam_x));
        o.set_u16(x.b, 0x10, py.wrapping_add(cam_y));
    }
    x.set32(f::CURSOR, entry + 0x14);
}

// --- état 4 : fin de scène ----------------------------------------------------------------------

fn finish(x: &mut Cx) {
    x.add16(f::TIMER, 0xffff);
    if x.u16(f::TIMER) & 0x8000 != 0 {
        x.set8(f::DONE, 1);
        x.set16(f::TIMER, 0x7fff);
    }
    if x.u16(f::LIVE) != 0 {
        return;
    }
    let n = x.a5_u8(a5::SCENES).wrapping_sub(1);
    x.set_a5_u8(a5::SCENES, if n & 0x80 != 0 { 0 } else { n });
    x.set_a5_u8(a5::TRANSITION_FLAG, 0);
    x.call(EXIT_SCENE);
    let variant = x.u8(0x3) as u32;
    let action = x.rom8(END_ACTION + variant);
    end_action(x, action);
}

/// Action de fin de scène (0 à 15), choisie par la variante.
fn end_action(x: &mut Cx, action: u8) {
    match action {
        12 => {
            x.set_a5_u8(a5::FROZEN, 1);
            end_action(x, 0)
        }
        0 => {
            let v = x.a5_u16(0x506);
            x.set_a5_u16(0x52c, v);
            end_action(x, 11)
        }
        11 => {
            x.set_a5_u16(a5::TRANSITION, 0);
            next_state(x, 2)
        }
        8 => next_state(x, 2),
        15 => {
            next_state(x, 2);
            x.set_a5_u8(a5::FROZEN, 1)
        }
        1 => {
            x.set_a5_u16(a5::TRANSITION, 0x3c);
            next_state(x, 2)
        }
        2 => title_cards(x, 0x10, [(0x0b, 0), (0x49, 2)]),
        3 => title_cards(x, 0x20, [(0x4a, 0), (0x4b, 2)]),
        4 => players_leave(x),
        5 => {
            x.set_a5_u16(a5::TRANSITION, 0);
            let v = x.a5_u16(0x556);
            x.set_a5_u16(0x57c, v);
            next_state(x, 2)
        }
        6 => curtain(x),
        7 => effects_wave(x),
        9 => {
            if spawn(x, sub::ALLOC_SET, 0x11, None).is_some() {
                next_state(x, 2)
            }
        }
        10 => {
            x.set_a5_u8(a5::FROZEN, 1);
            next_state(x, 2)
        }
        13 => spawn_boss(x, 3),
        14 => spawn_boss(x, 1),
        _ => unreachable!("table de fin : 0 à 15"),
    }
}

/// Actions 2 et 3 : intertitre. Deux décors SET (variantes `cards`, la seconde avec `+0x2f = 2`) et
/// un objet SOBJ (variante 0x56 puis 0x57), puis demande de transition de 0x3c images.
fn title_cards(x: &mut Cx, param: u16, cards: [(u8, u8); 2]) {
    x.set_a5_u16(0x56e, param);
    x.set_a5_u8(0x54f, 0);
    for (variant, extra) in cards {
        let Some(o) = spawn(x, sub::ALLOC_SET, 0, Some(variant)) else { return };
        o.set_u8(x.b, 0x2f, extra);
    }
    let Some(o) = spawn(x, sub::ALLOC_SOBJ, 0, Some(0x56)) else { return };
    o.set_u8(x.b, 3, 0x57);
    end_action(x, 1)
}

/// Action 4 : les joueurs vivants doivent être revenus à l'état d'attente (`+4 = 0x200`, `+6 = 0`) ;
/// alors ils reçoivent l'ordre de sortie (état 0x80400) et leur rang d'arrivée (`+0x7d`), et la phase
/// de partie passe à 0x18 (sortie du niveau).
fn players_leave(x: &mut Cx) {
    x.set_a5_u8(a5::SCENE_LATCH, 0);
    x.set_a5_u8(a5::PLAYERS_LOCK, 1);
    let alive = |x: &mut Cx, p: u32| rd8(x.b, p) != 0 && rd16(x.b, p + 0x62) != 0;
    for i in 0..4 {
        let p = PLAYERS + i * PLAYER_STRIDE;
        if alive(x, p) && (rd16(x.b, p + 4) != 0x200 || rd8(x.b, p + 6) != 0) {
            return;
        }
    }
    let mut rank = 0;
    for i in 0..4 {
        let p = PLAYERS + i * PLAYER_STRIDE;
        if alive(x, p) {
            wr32(x.b, p + 4, 0x80400);
            wr8(x.b, p + 0x66, 0);
            wr8(x.b, p + 0x15c, 0);
            wr8(x.b, p + 0x7d, rank);
            rank += 1;
        }
    }
    x.set_a5_u32(a5::PHASE, 0x1802_0000);
    next_state(x, 2);
}

/// Action 6 : le rideau. Au premier passage, fige la scène ; 0x80 images plus tard, et dès qu'un
/// joueur est dans l'état 2, copie deux blocs de 22 couleurs (0xbcfc6) vers les adresses de la table
/// 0xbcea4 à chaque image ; quand la table est épuisée, repousse tous les objets vers leur
/// disparition (voir `clear_all`).
fn curtain(x: &mut Cx) {
    if x.u8(0x5) == 0 {
        x.add8(0x5, 2);
        x.set16(0x14, 0x80);
        x.set_a5_u8(a5::SCENE_LATCH, 0x1f);
        x.set_a5_u8(a5::FROZEN, 1);
        x.set_a5_u8(a5::SCENE_SYNC, 0);
        wr8(x.b, PARALLAX_FLAG, 1);
        return;
    }
    x.add16(0x14, 0xffff);
    if x.u16(0x14) & 0x8000 == 0 {
        return;
    }
    x.set16(0x14, 0);
    if !(0..4).any(|i| rd8(x.b, PLAYERS + i * PLAYER_STRIDE + 4) == 2) {
        return;
    }
    x.call(PLAYER_HOOK_A);
    x.call(PLAYER_HOOK_B);
    for _ in 0..2 {
        let at = x.u16(f::WAIT) as i16 as i32 as u32;
        let offset = x.rom16(CURTAIN_TABLE.wrapping_add(at)) as i16;
        if offset < 0 {
            return clear_all(x);
        }
        let hi = x.rom16(CURTAIN_TABLE.wrapping_add(at) + 2) as u32;
        let dest = hi << 16 | x.rom16(CURTAIN_TABLE.wrapping_add(at) + 4) as u32;
        x.add16(f::WAIT, 6);
        let colors = CURTAIN_PALETTE.wrapping_add(offset as i32 as u32);
        for i in 0..22 {
            let c = x.data16(colors + 2 * i);
            wr16(x.b, dest + 4 * i, c);
            wr16(x.b, dest + 4 * i + 2, 0x1f);
        }
    }
}

/// Fin du rideau : tout objet actif dont le compteur `+0x40` tombe sous zéro passe à l'état 6
/// (disparition), dans les huit pools.
fn clear_all(x: &mut Cx) {
    next_state(x, 2);
    x.set_a5_u8(a5::SCENE_LATCH, 0xff);
    x.set_a5_u8(a5::FROZEN, 0);
    x.call(FADE_OUT);
    for pool in [&SET, &ENEMY, &SOBJ, &BOSS, &EFFCT, &ITEM, &ESHL, &PEFF] {
        for i in 0..pool.slots {
            let o = pool.slot(i);
            if o.u8(x.b, 0) == 0 {
                continue;
            }
            let hold = o.u8(x.b, 0x40).wrapping_sub(1);
            o.set_u8(x.b, 0x40, hold);
            if hold & 0x80 != 0 {
                o.set_u16(x.b, 0x4, 0x600);
                o.set_u8(x.b, 0x40, 0);
                o.set_u8(x.b, 0x41, 0);
            }
        }
    }
}

/// Action 7 : quatre effets de type 0x21 (variantes 0 à 3, les 1 et 3 avec `+0x2c = 0x20`).
fn effects_wave(x: &mut Cx) {
    if x.a5_u8(a5::FROZEN) != 0 {
        return next_state(x, 2);
    }
    x.call(PREPARE_EFFECTS);
    if spawn(x, ALLOC_EFFCT, 0x21, Some(0)).is_none() {
        return;
    }
    next_state(x, 2);
    for (variant, extra) in [(1, 0x20), (2, 0), (3, 0x20)] {
        let Some(o) = spawn(x, ALLOC_EFFCT, 0x21, Some(variant)) else { return };
        if extra != 0 {
            o.set_u8(x.b, 0x2c, extra);
        }
    }
}

/// Actions 13 et 14 : crée un boss (type 3 ou 1) sauf s'il est interdit ou si la scène est figée.
fn spawn_boss(x: &mut Cx, kind: u8) {
    if x.a5_u8(a5::NO_BOSS) == 0 && x.a5_u8(a5::FROZEN) == 0 && spawn(x, ALLOC_BOSS, kind, None).is_none() {
        return;
    }
    next_state(x, 2);
}
