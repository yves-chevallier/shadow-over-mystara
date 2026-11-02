//! Files d'événements d'affichage (U1) : points de score, second compteur, pop-ups.
//!
//! Trois anneaux en RAM de travail, vidés d'un seul coup à l'initialisation. Une case vide vaut 0xFFFF.
//!
//! | Anneau | Index écriture / lecture | Cases | Rôle |
//! |---|---|---|---|
//! | points | `A5+0x108` / `A5+0x109` | 64 mots en `A5+0x10a` | événements de score |
//! | second compteur | `A5+0x18a` / `A5+0x18b` | 64 mots en `A5+0x18c` | idem, compteur à 5 chiffres |
//! | pop-ups | `A5+0x20c` / `A5+0x20d` | 32 entrées de 8 octets en `A5+0x20e` | textes flottants |
//!
//! Un événement de score est un mot `(player << 8) | type` : le joueur est lu dans l'objet émetteur
//! (mot `+0x48`, bits 8-9), le type est son octet `+0x7e`. Chaque image, la tâche d'interface des
//! joueurs (0x15200) vide les anneaux : la valeur BCD associée au type (table de ROM, 4 octets par
//! type) est ajoutée au compteur BCD du joueur (`+0x110`, 8 chiffres ; `+0x114`, 5 chiffres), plafonné.
//!
//! Cœurs sans registres (`clear_points`, `enqueue_points`, `object_event`, `credit_points`, `create_popup`...)
//! et natives `_68k` pour les appels par adresse : celles-ci reproduisent les registres et drapeaux de l'original,
//! que des centaines d'appelants (objets ennemis...) peuvent relire.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_X};

use crate::mem::{rd16, rd32, rd8, rts, set_low_byte, wr16, wr32, wr8};
use crate::ops;
use crate::{regs, Native};

const EMPTY: u16 = 0xffff;
/// Les index des anneaux avancent de 2 (octets) modulo 0x80.
const MASK: u8 = 0x7f;

/// Un anneau d'événements de score.
struct Ring {
    /// Index d'écriture (octet) ; l'index de lecture est l'octet suivant.
    write: u32,
    /// Première case (mots).
    cases: u32,
}

const POINTS: Ring = Ring { write: 0x108, cases: 0x10a };
const COUNTER: Ring = Ring { write: 0x18a, cases: 0x18c };

/// File des pop-ups : index d'écriture et de lecture (octets), première entrée.
const POPUP_WRITE: u32 = 0x20c;
const POPUP_READ: u32 = 0x20d;
const POPUP_INPUTS: u32 = 0x20e;
/// Pop-up courant (4 longs en `A5-0x6e80`).
const CURRENT_POPUP: u32 = 0xffff_9180;
/// Octet non nul : plus de nouveaux pop-ups.
const POPUP_INHIBITED: u32 = 0x50;

/// Enregistrements joueur : le premier et la taille (0x200).
const PLAYER_1: u32 = 0x5de;

/// Adresse `A5 + off` (A5 vaut toujours `mem::A5`).
const fn a5(off: u32) -> u32 {
    crate::mem::A5.wrapping_add(off)
}

fn set_word(r: &mut u32, v: u16) {
    *r = *r & 0xffff_0000 | v as u32;
}

// ---------------------------------------------------------------------------------------------
// Remises à vide

/// `n` longs à 0xFFFFFFFF depuis `start`.
fn fill(b: &mut Cps2Bus, start: u32, n: u32) {
    for k in 0..n {
        wr32(b, start + 4 * k, 0xffff_ffff);
    }
}

/// Registres de `move.l D1,(A)+ ; dbf D0` : D1 = -1, mot bas de D0 = 0xFFFF, le registre d'adresse `ptr` rend la fin
/// de la zone.
fn fill_68k(c: &mut Cpu, end: u32, ptr: usize) {
    c.a[ptr] = end;
    set_word(&mut c.d[0], 0xffff);
    c.d[1] = 0xffff_ffff;
    ops::logic(c, 0xffff_ffff, Size::Long);
}

fn clear_ring(b: &mut Cps2Bus, ring: &Ring) {
    wr8(b, a5(ring.write), 0);
    wr8(b, a5(ring.write + 1), 0);
    fill(b, a5(ring.cases), 32);
}

/// 0x19aa2 : vide l'anneau des points.
pub fn clear_points(b: &mut Cps2Bus) {
    clear_ring(b, &POINTS);
}

/// 0x19abc : vide l'anneau du second compteur.
pub fn clear_counter(b: &mut Cps2Bus) {
    clear_ring(b, &COUNTER);
}

/// 0x19db4 : vide la file des pop-ups.
pub fn clear_popups(b: &mut Cps2Bus) {
    wr8(b, a5(POPUP_WRITE), 0);
    wr8(b, a5(POPUP_READ), 0);
    fill(b, a5(POPUP_INPUTS), 64);
}

/// 0x19e10 : efface le pop-up courant (4 longs).
pub fn clear_current_popup(b: &mut Cps2Bus) {
    for k in 0..4 {
        wr32(b, a5(CURRENT_POPUP) + 4 * k, 0);
    }
}

fn clear_ring_68k(c: &mut Cpu, b: &mut Cps2Bus, ring: &Ring) {
    clear_ring(b, ring);
    fill_68k(c, a5(ring.cases) + 4 * 32, 0);
    rts(c, b);
}

fn clear_popups_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    clear_popups(b);
    fill_68k(c, a5(POPUP_INPUTS) + 4 * 64, 1);
    rts(c, b);
}

/// Laisse A1 après la zone, D0 = 0, D6 = 0xFFFF.
fn clear_popup_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    clear_current_popup(b);
    c.a[1] = a5(CURRENT_POPUP) + 16;
    c.d[0] = 0;
    c.d[6] = 0xffff;
    ops::logic(c, 0, Size::Long);
    rts(c, b);
}

// ---------------------------------------------------------------------------------------------
// Enfilage d'un événement de score

/// Issue d'un enfilage : l'index d'écriture lu, et le contenu de la case s'il n'était pas libre.
struct Queued {
    index: u8,
    busy: Option<u16>,
}

fn enqueue(b: &mut Cps2Bus, ring: &Ring, event: u16) -> Queued {
    let index = rd8(b, a5(ring.write));
    let case = a5(ring.cases).wrapping_add(index as u32);
    let content = rd16(b, case);
    if content != EMPTY {
        return Queued { index, busy: Some(content) };
    }
    wr16(b, case, event);
    wr8(b, a5(ring.write), index.wrapping_add(2) & MASK);
    Queued { index, busy: None }
}

/// 0x19ae6 : enfile l'événement de score `event` (`(player << 8) | type`) si la case d'écriture est libre ;
/// rend vrai s'il est enfilé.
pub fn enqueue_points(b: &mut Cps2Bus, event: u16) -> bool {
    enqueue(b, &POINTS, event).busy.is_none()
}

/// 0x19b08 : idem dans l'anneau du second compteur.
pub fn enqueue_counter(b: &mut Cps2Bus, event: u16) -> bool {
    enqueue(b, &COUNTER, event).busy.is_none()
}

/// Événement de score de l'objet `o` : type `+0x7e`, joueur `+0x48` bits 8-9.
pub fn event_of(b: &mut Cps2Bus, o: crate::objects::Obj) -> u16 {
    let type_ = rd8(b, o.0 + 0x7e) as u16;
    (rd16(b, o.0 + 0x48) & 0x300).wrapping_add(type_)
}

/// 0x19ad6 : enfile l'événement de score de l'objet `o` dans l'anneau des points.
pub fn object_event(b: &mut Cps2Bus, o: crate::objects::Obj) -> bool {
    let event = event_of(b, o);
    enqueue_points(b, event)
}

/// Registres et drapeaux de 0x19ae6 / 0x19b08 : D6 (index), A4 (début des cases).
fn enqueue_68k(c: &mut Cpu, b: &mut Cps2Bus, ring: &Ring, event: u16) {
    let e = enqueue(b, ring, event);
    c.d[6] = e.index as u32;
    c.a[4] = a5(ring.cases);
    match e.busy {
        Some(content) => ops::cmp(c, EMPTY as u32, content as u32, Size::Word),
        None => {
            ops::add(c, 2, e.index as u32, Size::Byte);
            let reduced = e.index.wrapping_add(2) & MASK;
            set_word(&mut c.d[6], reduced as u16);
            ops::logic(c, reduced as u32, Size::Byte);
        }
    }
}

fn enqueue_points_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    enqueue_68k(c, b, &POINTS, c.d[0] as u16);
    rts(c, b);
}

fn enqueue_counter_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    enqueue_68k(c, b, &COUNTER, c.d[0] as u16);
    rts(c, b);
}

/// 0x19ad6 : D0 = l'événement, D1.w = le joueur (bits 8-9).
fn object_event_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    let o = crate::objects::Obj(c.a[0]);
    let type_ = rd8(b, o.0 + 0x7e) as u32;
    c.d[0] = type_;
    let player = rd16(b, o.0 + 0x48) & 0x300;
    set_word(&mut c.d[1], player);
    let event = ops::add(c, player as u32, type_, Size::Word) as u16;
    set_word(&mut c.d[0], event);
    enqueue_68k(c, b, &POINTS, event);
    rts(c, b);
}

// ---------------------------------------------------------------------------------------------
// Crédit des points

/// Un anneau à vider, avec la table de valeurs et le compteur du joueur à incrémenter.
struct Credit {
    ring: Ring,
    /// Index de lecture (octet).
    read: u32,
    /// Table de ROM des valeurs BCD (4 octets par type).
    table: u32,
    /// Décalage du compteur BCD dans l'enregistrement joueur.
    counter: u32,
    /// Plafond BCD (comparaison signée de 32 bits).
    cap: u32,
}

const CREDIT_POINTS: Credit = Credit {
    ring: POINTS,
    read: 0x109,
    table: 0x19c0e,
    counter: 0x110,
    cap: 0x0999_9999,
};
const CREDIT_COUNTER: Credit = Credit {
    ring: COUNTER,
    read: 0x18b,
    table: 0x19d52,
    counter: 0x114,
    cap: 0x0009_9999,
};

/// Ce que le traitement d'un événement laisse dans les registres.
struct Handled {
    player: u32,
    line: u32,
}

/// Traite l'événement de la case `index` : vide la case, ajoute la valeur BCD du type au compteur
/// du joueur, plafonne, avance l'index de lecture.
fn handle(b: &mut Cps2Bus, k: &Credit, index: u8, event: u16) -> Handled {
    wr16(b, a5(k.ring.cases).wrapping_add(index as u32), EMPTY);
    let player = a5(PLAYER_1) + (((event & 0x300) as u32) << 1);
    let line = (event as u32 & 0xff) << 2;
    let (table, counter) = (k.table + line, player + k.counter);
    // abcd -(A3),-(A4) ×4 : octet de poids faible d'abord, retenue chaînée, X initial nul
    let mut carry = false;
    for i in (0..4).rev() {
        let (sum, x) = ops::abcd(rd8(b, table + i), rd8(b, counter + i), carry);
        wr8(b, counter + i, sum);
        carry = x;
    }
    // cmpi.l #plafond,(A4) ; blt : on remplace par le plafond si la valeur n'est pas inférieure
    let capped = rd32(b, counter) as i32 >= k.cap as i32;
    if capped {
        wr32(b, counter, k.cap);
    }
    wr8(b, a5(k.read), index.wrapping_add(2) & MASK);
    Handled { player, line }
}

/// Évènement en attente à l'index de lecture, s'il y en a un.
fn pending(b: &mut Cps2Bus, k: &Credit) -> Option<(u8, u16)> {
    let index = rd8(b, a5(k.read));
    let event = rd16(b, a5(k.ring.cases).wrapping_add(index as u32));
    (event != EMPTY).then_some((index, event))
}

/// Dernier événement traité par un crédit : de quoi reconstituer les registres de l'original.
struct Last {
    index: u8,
    handled: Handled,
}

fn credit(b: &mut Cps2Bus, k: &Credit) -> Option<Last> {
    let mut last = None;
    while let Some((index, event)) = pending(b, k) {
        let handled = handle(b, k, index, event);
        last = Some(Last { index, handled });
    }
    last
}

/// 0x19b2a : crédite au compteur de points de chaque joueur les événements en attente.
pub fn credit_points(b: &mut Cps2Bus) {
    credit(b, &CREDIT_POINTS);
}

/// 0x19b9c : idem pour le second compteur.
pub fn credit_counter(b: &mut Cps2Bus) {
    credit(b, &CREDIT_COUNTER);
}

/// Registres de 0x19b2a / 0x19b9c : restes de la dernière itération, D0 = index de lecture final.
fn credit_68k(c: &mut Cpu, b: &mut Cps2Bus, k: &Credit) {
    c.d[1] = 0;
    c.d[2] = 0;
    c.a[2] = a5(k.ring.cases);
    let last = credit(b, k);
    if let Some(d) = &last {
        c.d[0] = d.index as u32;
        c.d[1] = d.handled.line;
        c.a[0] = d.handled.player;
        c.a[3] = k.table + d.handled.line;
        c.a[4] = d.handled.player + k.counter;
        c.d[6] = 0xffff;
    }
    c.d[0] = rd8(b, a5(k.read)) as u32;
    // dernier cmpi.w #-1 : égalité ; X : retenue de l'`addq.b #2` sur l'index de lecture de la dernière itération
    ops::cmp(c, EMPTY as u32, EMPTY as u32, Size::Word);
    if let Some(d) = last {
        ops::drapeaux(c, SR_X, if d.index as u16 + 2 > 0xff { SR_X } else { 0 });
    }
    rts(c, b);
}

// ---------------------------------------------------------------------------------------------
// Pop-ups

/// Un pop-up (barre de vie d'un monstre frappé, `player::external::effect::HEALTH_BAR`) : D0, D1, D2 de l'original.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Popup {
    /// Joueur et type (D0.w) : l'octet bas gagne 0x20 sauf si `raw`.
    pub header: u16,
    /// D1.w, D2.w : PV maximum et PV (h).
    pub max: u16,
    pub value: u16,
    /// D3.b = 2 : l'en-tête est pris tel quel.
    pub raw: bool,
}

/// Issue de 0x19dce : rien (pop-ups inhibés), case occupée, ou enfilé.
enum Creation {
    Inhibited(u8),
    Busy { index: u8, content: u16 },
    Done { index: u8, header: u16 },
}

fn create(b: &mut Cps2Bus, p: Popup) -> Creation {
    let inhibited = rd8(b, a5(POPUP_INHIBITED));
    if inhibited != 0 {
        return Creation::Inhibited(inhibited);
    }
    let index = rd8(b, a5(POPUP_WRITE));
    let case = a5(POPUP_INPUTS).wrapping_add(index as u32);
    let content = rd16(b, case);
    if content != EMPTY {
        return Creation::Busy { index, content };
    }
    let header = if p.raw { p.header } else { p.header & 0xff00 | (p.header as u8).wrapping_add(0x20) as u16 };
    for (offset, value) in [(0, header), (2, p.max), (4, p.value), (6, 0x80)] {
        wr16(b, case + offset, value);
    }
    // l'index avance de 8 sans masque : il fait le tour des 32 entrées en débordant de son octet
    wr8(b, a5(POPUP_WRITE), index.wrapping_add(8));
    Creation::Done { index, header }
}

/// Les pop-ups sont-ils inhibés (`A5+0x50`) ? 0x19dce n'écrit alors rien et laisse A2 intact ; sinon il laisse
/// A2 = [`POPUP_INPUTS_ADDR`] (relu par le combat : `combat::show_enemy_hp`).
pub fn popups_inhibited(b: &mut Cps2Bus) -> bool {
    rd8(b, a5(POPUP_INHIBITED)) != 0
}

/// Première entrée de la file des pop-ups (A2 que laisse 0x19dce).
pub const POPUP_INPUTS_ADDR: u32 = a5(POPUP_INPUTS);

/// 0x19dce : enfile un pop-up dans la case d'écriture si elle est libre et si les pop-ups ne sont pas inhibés ;
/// rend vrai s'il est enfilé.
pub fn create_popup(b: &mut Cps2Bus, p: Popup) -> bool {
    matches!(create(b, p), Creation::Done { .. })
}

/// Registres et drapeaux de 0x19dce : D6 (index), A2 (entrées), D0.b (en-tête), drapeaux du dernier test.
fn create_popup_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    let p = Popup { header: c.d[0] as u16, max: c.d[1] as u16, value: c.d[2] as u16, raw: c.d[3] as u8 == 2 };
    match create(b, p) {
        Creation::Inhibited(inhibited) => ops::logic(c, inhibited as u32, Size::Byte),
        Creation::Busy { index, content } => {
            c.d[6] = index as u32;
            c.a[2] = a5(POPUP_INPUTS);
            ops::cmp(c, EMPTY as u32, content as u32, Size::Word);
        }
        Creation::Done { index, header } => {
            c.a[2] = a5(POPUP_INPUTS);
            ops::cmp(c, 2, c.d[3] & 0xff, Size::Byte);
            if !p.raw {
                ops::add(c, 0x20, c.d[0] & 0xff, Size::Byte);
                set_low_byte(&mut c.d[0], header as u8);
            }
            let next = ops::add(c, 8, index as u32, Size::Byte) as u8;
            c.d[6] = next as u32;
            ops::logic(c, next as u32, Size::Byte);
        }
    }
    rts(c, b);
}

macro_rules! native {
    ($ident:ident, $entry:expr, $name:expr, $run:expr) => {
        pub const $ident: Native = Native {
            entry: $entry,
            name: $name,
            run: crate::Body::Processeur($run),
            touches_sound: false,
            outputs: regs::ALL,
        };
    };
}

native!(CLEAR_POINTS, 0x19aa2, "evt_vider_points", |c, b| clear_ring_68k(c, b, &POINTS));
native!(CLEAR_COUNTER, 0x19abc, "evt_vider_compteur", |c, b| clear_ring_68k(c, b, &COUNTER));
native!(CLEAR_POPUPS, 0x19db4, "evt_vider_popups", clear_popups_68k);
native!(CLEAR_POPUP, 0x19e10, "evt_effacer_popup", clear_popup_68k);
native!(ENQUEUE_POINTS, 0x19ae6, "evt_enfiler_points", enqueue_points_68k);
native!(ENQUEUE_COUNTER, 0x19b08, "evt_enfiler_compteur", enqueue_counter_68k);
native!(OBJECT_EVENT, 0x19ad6, "evt_evenement_objet", object_event_68k);
native!(CREATE_POPUP, 0x19dce, "evt_creer_popup", create_popup_68k);
native!(CREDIT_POINTS_NATIVE, 0x19b2a, "evt_crediter_points", |c, b| credit_68k(c, b, &CREDIT_POINTS));
native!(CREDIT_COUNTER_NATIVE, 0x19b9c, "evt_crediter_compteur", |c, b| credit_68k(c, b, &CREDIT_COUNTER));
