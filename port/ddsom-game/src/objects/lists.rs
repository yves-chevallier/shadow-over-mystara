//! Listes d'objets triées par profondeur (ordre de dessin).
//!
//! Deux listes doublement chaînées intrusives contiennent les objets à dessiner : les *acteurs*
//! (personnages, ennemis, décors) et le *premier plan*. Les liens sont les mots `Obj::PREV` (vers
//! la tête) et `Obj::NEXT` (vers la queue) ; la liste est triée par `Obj::Y` décroissant de la tête
//! à la queue. Une liste vide a tête et queue nulles ; les bouts de liste portent la marque de fin
//! de leur liste (1 : acteurs, 2 : premier plan).
//!
//! Chaque handler d'objet termine par `reinsert` : l'objet est inséré s'il n'est dans aucune liste,
//! sinon déplacé seulement si sa profondeur ne respecte plus l'ordre. Les gestionnaires natifs
//! reproduisent aussi la durée de l'original, qui dépend du nombre de voisins parcourus.

use cps2::Cps2Bus;
use m68k::Cpu;

use super::cost::{walk_cycles, Trace};
use super::{Link, Obj};
use crate::mem::{rd16, rts, wr16, A5};
use crate::{regs, Native};

#[derive(Clone, Copy, Debug)]
pub struct SortedList {
    head: u32,
    tail: u32,
    end: Link,
}

/// Personnages, ennemis, décors.
pub const ACTORS: SortedList = SortedList { head: super::ram::ACTORS_HEAD, tail: super::ram::ACTORS_TAIL, end: Link(1) };
/// Premier plan.
pub const FOREGROUND: SortedList =
    SortedList { head: super::ram::FOREGROUND_HEAD, tail: super::ram::FOREGROUND_TAIL, end: Link(2) };

fn is_sentinel_or_none(l: Link) -> bool {
    l.0 as i16 >= 0
}

/// Où replacer un objet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    /// À sa place : rien à faire.
    Keep,
    /// La liste est vide.
    Alone,
    /// Juste avant l'objet (plus près de la tête).
    Before(Obj),
    /// Juste après l'objet (plus près de la queue).
    After(Obj),
}

impl SortedList {
    fn head(self, b: &mut Cps2Bus) -> Link {
        Link(rd16(b, A5 + self.head))
    }
    fn tail(self, b: &mut Cps2Bus) -> Link {
        Link(rd16(b, A5 + self.tail))
    }
    fn set_head(self, b: &mut Cps2Bus, l: Link) {
        wr16(b, A5 + self.head, l.0)
    }
    fn set_tail(self, b: &mut Cps2Bus, l: Link) {
        wr16(b, A5 + self.tail, l.0)
    }

    /// Insère `obj` s'il n'est dans aucune liste, sinon le replace si l'ordre est rompu.
    pub fn reinsert(self, b: &mut Cps2Bus, obj: Obj) {
        let place = self.plan_reinsert(b, obj, &mut Trace::new());
        self.apply(b, obj, place);
    }

    /// Cherche la nouvelle place de `obj` sans rien modifier. `t` reçoit les décisions des
    /// branchements de l'original, dans l'ordre (pour la durée).
    fn plan_reinsert(self, b: &mut Cps2Bus, obj: Obj, t: &mut Trace) -> Place {
        let (prev, next) = (obj.prev(b), obj.next(b));
        let y = obj.depth(b);
        if t.take(prev.0.wrapping_add(next.0) == 0) {
            return self.plan_insert(b, y, t);
        }
        // trop loin de la tête ? alors on remonte vers la tête
        if !t.take(is_sentinel_or_none(prev)) {
            let p = prev.target();
            if !t.take(y <= p.depth(b)) {
                let mut a1 = p;
                loop {
                    let before = a1.prev(b);
                    if t.take(is_sentinel_or_none(before)) {
                        break;
                    }
                    let a2 = before.target();
                    if t.take(y <= a2.depth(b)) {
                        break;
                    }
                    a1 = a2;
                }
                // retrait (voisin suivant : objet ou fin) puis insertion devant a1
                t.take(is_sentinel_or_none(next));
                t.take(!is_sentinel_or_none(next));
                t.take(is_sentinel_or_none(a1.prev(b)));
                return Place::Before(a1);
            }
        }
        // trop près de la tête ? alors on descend vers la queue
        if t.take(is_sentinel_or_none(next)) {
            return Place::Keep;
        }
        let n = next.target();
        if t.take(y >= n.depth(b)) {
            return Place::Keep;
        }
        let mut a1 = n;
        loop {
            let after = a1.next(b);
            if t.take(is_sentinel_or_none(after)) {
                break;
            }
            let a2 = after.target();
            if t.take(y >= a2.depth(b)) {
                break;
            }
            a1 = a2;
        }
        t.take(is_sentinel_or_none(prev));
        t.take(!is_sentinel_or_none(prev));
        t.take(is_sentinel_or_none(a1.next(b)));
        Place::After(a1)
    }

    /// Place d'un objet absent de la liste, en partant de la queue.
    fn plan_insert(self, b: &mut Cps2Bus, y: i32, t: &mut Trace) -> Place {
        let tail = self.tail(b);
        if t.take(tail.is_none()) {
            return Place::Alone;
        }
        let mut a1 = tail.target();
        if t.take(y <= a1.depth(b)) {
            return Place::After(a1);
        }
        loop {
            let before = a1.prev(b);
            if t.take(is_sentinel_or_none(before)) {
                return Place::Before(a1);
            }
            a1 = before.target();
            if !t.take(y > a1.depth(b)) {
                return Place::After(a1);
            }
        }
    }

    fn apply(self, b: &mut Cps2Bus, obj: Obj, place: Place) {
        let linked = obj.prev(b).0.wrapping_add(obj.next(b).0) != 0;
        match place {
            Place::Keep => {}
            Place::Alone => {
                obj.set_prev(b, self.end);
                obj.set_next(b, self.end);
                self.set_head(b, Link::to(obj));
                self.set_tail(b, Link::to(obj));
            }
            Place::Before(anchor) => {
                if linked {
                    self.detach(b, obj);
                }
                let before = anchor.prev(b);
                obj.set_prev(b, before);
                if is_sentinel_or_none(before) {
                    self.set_head(b, Link::to(obj));
                } else {
                    before.target().set_next(b, Link::to(obj));
                }
                anchor.set_prev(b, Link::to(obj));
                obj.set_next(b, Link::to(anchor));
            }
            Place::After(anchor) => {
                if linked {
                    self.detach(b, obj);
                }
                let after = anchor.next(b);
                obj.set_next(b, after);
                if is_sentinel_or_none(after) {
                    self.set_tail(b, Link::to(obj));
                } else {
                    after.target().set_prev(b, Link::to(obj));
                }
                anchor.set_next(b, Link::to(obj));
                obj.set_prev(b, Link::to(anchor));
            }
        }
    }

    /// Raccorde les voisins de `obj` entre eux (ses propres liens restent à jour par l'appelant).
    fn detach(self, b: &mut Cps2Bus, obj: Obj) {
        let (prev, next) = (obj.prev(b), obj.next(b));
        if is_sentinel_or_none(prev) {
            self.set_head(b, next);
        } else {
            prev.target().set_next(b, next);
        }
        if is_sentinel_or_none(next) {
            self.set_tail(b, prev);
        } else {
            next.target().set_prev(b, prev);
        }
    }
}

/// Cas de retrait d'un objet de sa liste.
#[derive(Clone, Copy, Debug)]
pub(super) enum Removal {
    /// Pas dans une liste.
    NotListed,
    /// Entre deux objets.
    Middle { prev: Obj, next: Obj },
    /// Dernier de sa liste (la queue devient `prev`).
    Last { prev: Obj, list: SortedList },
    /// Premier de sa liste (la tête devient `next`).
    First { next: Obj, list: SortedList },
    /// Seul dans sa liste.
    Only { list: SortedList },
}

/// Analyse le retrait de `obj` (sans rien modifier) ; `t` reçoit les décisions de `0x1f0be`.
pub(super) fn plan_removal(b: &mut Cps2Bus, obj: Obj, t: &mut Trace) -> Removal {
    let (prev, next) = (obj.prev(b), obj.next(b));
    if t.take(prev.is_none()) {
        return Removal::NotListed;
    }
    if !t.take(is_sentinel_or_none(prev)) {
        // un objet précède
        let p = prev.target();
        if !t.take(is_sentinel_or_none(next)) {
            Removal::Middle { prev: p, next: next.target() }
        } else {
            let list = list_ending_with_taking(next, t);
            Removal::Last { prev: p, list }
        }
    } else if !t.take(is_sentinel_or_none(next)) {
        let list = list_ending_with_taking(prev, t);
        Removal::First { next: next.target(), list }
    } else {
        let list = list_ending_with_taking(next, t);
        Removal::Only { list }
    }
}

/// Comme `list_ending_with`, en notant la décision `bgt` de l'original.
fn list_ending_with_taking(end: Link, t: &mut Trace) -> SortedList {
    if t.take(end.0 as i16 > 1) { FOREGROUND } else { ACTORS }
}

/// Retire `obj` de sa liste (s'il y est) et remet ses liens à zéro : `0x1f0be`.
pub fn unlink(b: &mut Cps2Bus, obj: Obj) {
    let removal = plan_removal(b, obj, &mut Trace::new());
    apply_removal(b, obj, removal);
}

fn apply_removal(b: &mut Cps2Bus, obj: Obj, removal: Removal) {
    match removal {
        Removal::NotListed => return,
        Removal::Middle { prev, next } => {
            let (p, n) = (obj.prev(b), obj.next(b));
            next.set_prev(b, p);
            prev.set_next(b, n);
        }
        Removal::Last { prev, list } => {
            list.set_tail(b, Link::to(prev));
            prev.set_next(b, list.end);
        }
        Removal::First { next, list } => {
            list.set_head(b, Link::to(next));
            next.set_prev(b, list.end);
        }
        Removal::Only { list } => {
            list.set_head(b, Link::NONE);
            list.set_tail(b, Link::NONE);
        }
    }
    obj.set_prev(b, Link::NONE);
    obj.set_next(b, Link::NONE);
}

/// Retire de leurs listes tous les objets des acteurs puis, si les acteurs n'étaient pas vides,
/// ceux du premier plan : `0x1f184`. Renvoie le dernier objet retiré.
pub fn unlink_all(b: &mut Cps2Bus) -> Option<Obj> {
    let mut last = None;
    for list in [ACTORS, FOREGROUND] {
        let head = list.head(b);
        if head.is_none() {
            break;
        }
        let mut cur = head.target();
        loop {
            let next = cur.next(b);
            unlink(b, cur);
            last = Some(cur);
            if is_sentinel_or_none(next) {
                break;
            }
            cur = next.target();
        }
    }
    last
}

// ---- adaptation aux registres du 68000 ----

fn cost_reinsert(entry: u32, list: SortedList, c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let mut t = Trace::new();
    list.plan_reinsert(b, Obj(c.a[0]), &mut t);
    walk_cycles(b, entry, &t)
}

/// 0x1ee1a : (ré)insère l'objet A0 dans la liste des acteurs.
fn reinsert_actors(c: &mut Cpu, b: &mut Cps2Bus) {
    ACTORS.reinsert(b, Obj(c.a[0]));
    rts(c, b);
}

/// 0x1ef6c : (ré)insère l'objet A0 dans la liste du premier plan.
fn reinsert_foreground(c: &mut Cpu, b: &mut Cps2Bus) {
    FOREGROUND.reinsert(b, Obj(c.a[0]));
    rts(c, b);
}

/// 0x1f0be : retire l'objet A0 de sa liste.
fn unlink_native(c: &mut Cpu, b: &mut Cps2Bus) {
    unlink(b, Obj(c.a[0]));
    rts(c, b);
}

/// 0x1f184 : vide les deux listes.
fn unlink_all_native(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(last) = unlink_all(b) {
        c.a[0] = last.0 | 0xff00_0000; // movea.w : étendu en signe
    }
    rts(c, b);
}

pub const REINSERT_ACTORS: Native = Native {
    entry: 0x1ee1a,
    name: "list_reinsert_actors",
    cost: |c, b| cost_reinsert(0x1ee1a, ACTORS, c, b),
    run: reinsert_actors,
    touches_sound: false,
    outputs: regs::a(0),
    exact_cost: true,
};

pub const REINSERT_FOREGROUND: Native = Native {
    entry: 0x1ef6c,
    name: "list_reinsert_foreground",
    cost: |c, b| cost_reinsert(0x1ef6c, FOREGROUND, c, b),
    run: reinsert_foreground,
    touches_sound: false,
    outputs: regs::a(0),
    exact_cost: true,
};

pub const UNLINK: Native = Native {
    entry: 0x1f0be,
    name: "list_unlink",
    cost: |c, b| {
        let mut t = Trace::new();
        plan_removal(b, Obj(c.a[0]), &mut t);
        walk_cycles(b, 0x1f0be, &t)
    },
    run: unlink_native,
    touches_sound: false,
    outputs: regs::a(0),
    exact_cost: true,
};

pub const UNLINK_ALL: Native = Native {
    entry: 0x1f184,
    name: "list_unlink_all",
    cost: unlink_all_cost,
    run: unlink_all_native,
    touches_sound: false,
    outputs: regs::a(0),
    exact_cost: true,
};

/// Durée de `unlink_all` : chaque objet retiré est alors en tête de sa liste.
fn unlink_all_cost(_c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let mut t = Trace::new();
    for (i, list) in [ACTORS, FOREGROUND].into_iter().enumerate() {
        let head = list.head(b);
        if t.take(head.is_none()) {
            break;
        }
        let mut cur = head.target();
        loop {
            let next = cur.next(b);
            let last = t.take(is_sentinel_or_none(next));
            // retrait d'un premier (ou d'un seul) objet : dans une liste, précédé de la marque de fin
            t.take(false);
            t.take(true);
            t.take(last);
            t.take(i == 1); // marque de fin > 1 : liste du premier plan
            if last {
                break;
            }
            cur = next.target();
        }
    }
    walk_cycles(b, 0x1f184, &t)
}
