//! Listes d'objets triées par profondeur (ordre de dessin).
//!
//! Deux listes doublement chaînées intrusives contiennent les objets à dessiner : les *acteurs*
//! (personnages, ennemis, décors) et le *premier plan*. Les liens sont les mots `Obj::PREV` (vers
//! la tête) et `Obj::NEXT` (vers la queue) ; la liste est triée par `Obj::Y` décroissant de la tête
//! à la queue. Une liste vide a tête et queue nulles ; les bouts de liste portent la marque de fin
//! de leur liste (1 : acteurs, 2 : premier plan).
//!
//! Chaque handler d'objet termine par `reinsert` : l'objet est inséré s'il n'est dans aucune liste,
//! sinon déplacé seulement si sa profondeur ne respecte plus l'ordre.
//!
//! Piège du jeu d'origine : les handlers finissent souvent par `jmp 0x1ee1a`, et le dispatch
//! passe à l'objet suivant sans remettre les registres à zéro. Certains handlers lisent alors un
//! registre laissé par le précédent : 0x4bd32 fait `andi.b #$f,D0` puis indexe une table avec tout
//! D0.w, dont l'octet haut est le lien de liste laissé par `reinsert`. Les natives 0x1ee1a et
//! 0x1ef6c (`Reinsert`) transcrivent donc l'original instruction par instruction et rendent
//! D0-D2, A1-A4 et les drapeaux tels que lui, selon le chemin suivi (`SortedList::reinsert_68k`). Les
//! appelants qui n'ont pas ce contrat utilisent `SortedList::reinsert`, la même transcription sans registres.

use cps2::Cps2Bus;
use m68k::Cpu;

use super::{Link, Obj};
use crate::mem::{rd16, rd32, rts, set_low_word, wr16, A5};
use crate::{ops, regs, Native};
use m68k::decode::Size;

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

impl SortedList {
    fn head(self, b: &mut Cps2Bus) -> Link {
        Link(rd16(b, A5 + self.head))
    }
    fn set_head(self, b: &mut Cps2Bus, l: Link) {
        wr16(b, A5 + self.head, l.0)
    }
    fn set_tail(self, b: &mut Cps2Bus, l: Link) {
        wr16(b, A5 + self.tail, l.0)
    }

    /// Insère `obj` s'il n'est dans aucune liste, sinon le replace si l'ordre est rompu.
    ///
    /// Les écritures sont celles de 0x1ee1a, dans le même ordre (comme `Reinsert`, sans registres) : sur une liste
    /// incohérente (liens qui se recouvrent), l'original et cette fonction laissent la même mémoire, ce que la
    /// version par plan (`plan_reinsert`, `apply`) ne garantissait pas (écart trouvé par `fuzz_couv` sur ESHL 44).
    pub fn reinsert(self, b: &mut Cps2Bus, obj: Obj) {
        let a0 = obj.0;
        let (prev, next) = (rd16(b, a0 + Obj::PREV), rd16(b, a0 + Obj::NEXT));
        if prev.wrapping_add(next) == 0 {
            return self.insert_exact(b, obj);
        }
        if self.relocate_exact(b, obj, Way::toward_head(self)) {
            return;
        }
        self.relocate_exact(b, obj, Way::toward_tail(self));
    }

    /// `Reinsert::relocate` sans registres.
    fn relocate_exact(self, b: &mut Cps2Bus, obj: Obj, w: Way) -> bool {
        let a0 = obj.0;
        let link = rd16(b, a0 + w.ahead);
        if (link as i16) >= 0 {
            return false;
        }
        let mut a1 = link as i16 as i32 as u32;
        let y = rd32(b, a0 + Obj::Y);
        if w.settled(y, rd32(b, a1.wrapping_add(Obj::Y))) {
            return false;
        }
        let mut d0;
        loop {
            d0 = rd16(b, a1.wrapping_add(w.ahead));
            if (d0 as i16) >= 0 {
                wr16(b, A5 + w.reach, a0 as u16);
                break;
            }
            let a2 = d0 as i16 as i32 as u32;
            if w.settled(y, rd32(b, a2.wrapping_add(Obj::Y))) {
                break;
            }
            a1 = a2;
        }
        let a3 = rd16(b, a0 + w.ahead) as i16 as i32 as u32;
        let d2 = rd16(b, a0 + w.behind);
        if (d2 as i16) < 0 {
            wr16(b, (d2 as i16 as i32 as u32).wrapping_add(w.ahead), a3 as u16);
        }
        wr16(b, a3.wrapping_add(w.behind), d2);
        if (d2 as i16) >= 0 {
            wr16(b, A5 + w.far, a3 as u16);
        }
        wr16(b, a0 + w.ahead, d0);
        if (d0 as i16) < 0 {
            wr16(b, (d0 as i16 as i32 as u32).wrapping_add(w.behind), a0 as u16);
        }
        wr16(b, a1.wrapping_add(w.ahead), a0 as u16);
        wr16(b, a0 + w.behind, a1 as u16);
        true
    }

    /// `Reinsert::insert` sans registres (0x1eef2).
    fn insert_exact(self, b: &mut Cps2Bus, obj: Obj) {
        let a0 = obj.0;
        let y = rd32(b, a0 + Obj::Y);
        let tail = rd16(b, A5 + self.tail);
        if tail == 0 {
            wr16(b, a0 + Obj::PREV, self.end.0);
            wr16(b, a0 + Obj::NEXT, self.end.0);
            wr16(b, A5 + self.head, a0 as u16);
            wr16(b, A5 + self.tail, a0 as u16);
            return;
        }
        let mut a1 = tail as i16 as i32 as u32;
        if y as i32 <= rd32(b, a1.wrapping_add(Obj::Y)) as i32 {
            wr16(b, a1.wrapping_add(Obj::NEXT), a0 as u16);
            wr16(b, a0 + Obj::PREV, a1 as u16);
            wr16(b, a0 + Obj::NEXT, self.end.0);
            wr16(b, A5 + self.tail, a0 as u16);
            return;
        }
        loop {
            let before = rd16(b, a1.wrapping_add(Obj::PREV));
            if (before as i16) >= 0 {
                wr16(b, a1.wrapping_add(Obj::PREV), a0 as u16);
                wr16(b, a0 + Obj::NEXT, a1 as u16);
                wr16(b, a0 + Obj::PREV, self.end.0);
                wr16(b, A5 + self.head, a0 as u16);
                return;
            }
            a1 = before as i16 as i32 as u32;
            if y as i32 <= rd32(b, a1.wrapping_add(Obj::Y)) as i32 {
                break;
            }
        }
        let a2 = rd16(b, a1.wrapping_add(Obj::NEXT)) as i16 as i32 as u32;
        wr16(b, a1.wrapping_add(Obj::NEXT), a0 as u16);
        wr16(b, a2.wrapping_add(Obj::PREV), a0 as u16);
        wr16(b, a0 + Obj::PREV, a1 as u16);
        wr16(b, a0 + Obj::NEXT, a2 as u16);
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

/// Analyse le retrait de `obj` (sans rien modifier), comme `0x1f0be`.
fn plan_removal(b: &mut Cps2Bus, obj: Obj) -> Removal {
    let (prev, next) = (obj.prev(b), obj.next(b));
    if prev.is_none() {
        return Removal::NotListed;
    }
    if !is_sentinel_or_none(prev) {
        // un objet précède
        let p = prev.target();
        if !is_sentinel_or_none(next) {
            Removal::Middle { prev: p, next: next.target() }
        } else {
            let list = list_ending_with_mark(next);
            Removal::Last { prev: p, list }
        }
    } else if !is_sentinel_or_none(next) {
        let list = list_ending_with_mark(prev);
        Removal::First { next: next.target(), list }
    } else {
        let list = list_ending_with_mark(next);
        Removal::Only { list }
    }
}

/// La liste dont `end` est la marque de fin (`bgt` de l'original : > 1, premier plan).
fn list_ending_with_mark(end: Link) -> SortedList {
    if end.0 as i16 > 1 { FOREGROUND } else { ACTORS }
}

/// Retire `obj` de sa liste (s'il y est) et remet ses liens à zéro : `0x1f0be`.
pub fn unlink(b: &mut Cps2Bus, obj: Obj) {
    let removal = plan_removal(b, obj);
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

/// Sens dans lequel un objet mal placé cherche sa nouvelle place.
#[derive(Clone, Copy)]
struct Way {
    /// Lien suivi pour chercher (`Obj::PREV` vers la tête, `Obj::NEXT` vers la queue) et lien opposé.
    ahead: u32,
    behind: u32,
    /// Extrémité de liste atteinte en suivant `ahead`, et extrémité opposée (adresses relatives à A5).
    reach: u32,
    far: u32,
    /// Vers la tête (profondeurs décroissantes en tête) : on s'arrête dès que `y <= voisin`, sinon `y >= voisin`.
    toward_head: bool,
}

impl Way {
    fn toward_head(list: SortedList) -> Way {
        Way { ahead: Obj::PREV, behind: Obj::NEXT, reach: list.head, far: list.tail, toward_head: true }
    }
    fn toward_tail(list: SortedList) -> Way {
        Way { ahead: Obj::NEXT, behind: Obj::PREV, reach: list.tail, far: list.head, toward_head: false }
    }
    /// L'objet de profondeur `y` est-il déjà du bon côté de `neighbour` ?
    fn settled(self, y: u32, neighbour: u32) -> bool {
        if self.toward_head { y as i32 <= neighbour as i32 } else { y as i32 >= neighbour as i32 }
    }
}

/// Réinsertion de A0 dans `list` (0x1ee1a, 0x1ef6c), instruction par instruction : les handlers
/// d'objets qui suivent relisent D0, D1, D2, A1-A4 et les drapeaux tels que l'original les laisse
/// (voir l'en-tête du module), selon le chemin suivi. Les `move.w` vers la mémoire fixent N et Z
/// (`store`), les `movea.w` étendent en signe et ne touchent aucun drapeau.
struct Reinsert<'a> {
    c: &'a mut Cpu,
    b: &'a mut Cps2Bus,
    /// L'objet réinséré (A0 de l'original).
    obj: Obj,
    list: SortedList,
}

impl Reinsert<'_> {
    fn a0(&self) -> u32 {
        self.obj.0
    }
    fn a0w(&self) -> u16 {
        self.obj.0 as u16
    }
    /// `movea.w src,An`.
    fn movea(&mut self, n: usize, w: u16) -> u32 {
        self.c.a[n] = w as i16 as i32 as u32;
        self.c.a[n]
    }
    /// `movea.w (off,Am),An`.
    fn movea_mem(&mut self, n: usize, base: u32, off: u32) -> u32 {
        let w = rd16(self.b, base.wrapping_add(off));
        self.movea(n, w)
    }
    /// `move.w (off,An),Dn` : mot bas de Dn et drapeaux.
    fn load(&mut self, d: usize, base: u32, off: u32) -> u16 {
        let v = rd16(self.b, base.wrapping_add(off));
        set_low_word(&mut self.c.d[d], v);
        ops::logique(self.c, v as u32, Size::Word);
        v
    }
    /// `move.w src,(off,An)` : écrit et fixe les drapeaux.
    fn store(&mut self, base: u32, off: u32, v: u16) {
        wr16(self.b, base.wrapping_add(off), v);
        ops::logique(self.c, v as u32, Size::Word);
    }
    /// `cmp.l (Y,An),Dn` : drapeaux ; renvoie la profondeur du voisin.
    fn compare_depth(&mut self, d: usize, an: u32) -> u32 {
        let y = rd32(self.b, an.wrapping_add(Obj::Y));
        let dn = self.c.d[d];
        ops::cmp(self.c, y, dn, Size::Long);
        y
    }

    fn run(&mut self) {
        let a0 = self.a0();
        let prev = self.load(0, a0, Obj::PREV);
        let sum = ops::add(self.c, rd16(self.b, a0 + Obj::NEXT) as u32, prev as u32, Size::Word);
        set_low_word(&mut self.c.d[0], sum as u16);
        if sum as u16 == 0 {
            return self.insert();
        }
        if self.relocate(Way::toward_head(self.list)) {
            return;
        }
        self.relocate(Way::toward_tail(self.list));
    }

    /// L'objet est déjà dans la liste : le déplace s'il est du mauvais côté d'un voisin dans le sens
    /// `w`. Renvoie vrai quand la routine est terminée (déplacé, ou rien à faire en bout de liste).
    fn relocate(&mut self, w: Way) -> bool {
        let a0 = self.a0();
        let link = self.load(0, a0, w.ahead);
        if (link as i16) >= 0 {
            return false;
        }
        let mut a1 = self.movea(1, link);
        self.c.d[1] = rd32(self.b, a0 + Obj::Y);
        let y1 = self.compare_depth(1, a1);
        if w.settled(self.c.d[1], y1) {
            return false;
        }
        // cherche le dernier voisin que l'objet doit encore dépasser
        loop {
            let link = self.load(0, a1, w.ahead);
            if (link as i16) >= 0 {
                self.store(A5, w.reach, self.a0w());
                break;
            }
            let a2 = self.movea(2, link);
            let y2 = self.compare_depth(1, a2);
            if w.settled(self.c.d[1], y2) {
                break;
            }
            a1 = self.movea(1, link);
        }
        // retrait : le voisin `behind` et celui de `ahead` se raccordent
        let a3 = self.movea_mem(3, a0, w.ahead);
        let d2 = self.load(2, a0, w.behind);
        if (d2 as i16) < 0 {
            let a4 = self.movea(4, d2);
            self.store(a4, w.ahead, a3 as u16);
        }
        self.store(a3, w.behind, d2);
        if (d2 as i16) >= 0 {
            self.store(A5, w.far, a3 as u16);
        }
        // insertion devant A1 (dans le sens `ahead`)
        let d0 = self.c.d[0] as u16;
        self.store(a0, w.ahead, d0);
        if (d0 as i16) < 0 {
            let a2 = self.movea(2, d0);
            self.store(a2, w.behind, self.a0w());
        }
        self.store(a1, w.ahead, self.a0w());
        self.store(a0, w.behind, a1 as u16);
        true
    }

    /// L'objet n'est dans aucune liste : insertion en partant de la queue (0x1eef2).
    fn insert(&mut self) {
        let (a0, list) = (self.a0(), self.list);
        let y = rd32(self.b, a0 + Obj::Y);
        self.c.d[0] = y;
        let tail = self.load(1, A5, list.tail);
        if tail == 0 {
            self.store(a0, Obj::PREV, list.end.0);
            self.store(a0, Obj::NEXT, list.end.0);
            self.store(A5, list.head, self.a0w());
            self.store(A5, list.tail, self.a0w());
            return;
        }
        let mut a1 = self.movea(1, tail);
        let y1 = self.compare_depth(0, a1);
        if y as i32 <= y1 as i32 {
            // plus profond que toute la liste : nouvelle queue
            self.store(a1, Obj::NEXT, self.a0w());
            self.store(a0, Obj::PREV, a1 as u16);
            self.store(a0, Obj::NEXT, list.end.0);
            self.store(A5, list.tail, self.a0w());
            return;
        }
        loop {
            let before = self.load(1, a1, Obj::PREV);
            if (before as i16) >= 0 {
                // devient la nouvelle tête
                self.store(a1, Obj::PREV, self.a0w());
                self.store(a0, Obj::NEXT, a1 as u16);
                self.store(a0, Obj::PREV, list.end.0);
                self.store(A5, list.head, self.a0w());
                return;
            }
            a1 = self.movea(1, before);
            let y1 = self.compare_depth(0, a1);
            if y as i32 <= y1 as i32 {
                break;
            }
        }
        // entre A1 et son suivant
        let a2 = self.movea_mem(2, a1, Obj::NEXT);
        self.store(a1, Obj::NEXT, self.a0w());
        self.store(a2, Obj::PREV, self.a0w());
        self.store(a0, Obj::PREV, a1 as u16);
        self.store(a0, Obj::NEXT, a2 as u16);
    }
}

/// Registres que laisse la réinsertion d'origine (voir `diag::registres_morts`, étiquette `lists`).
pub const REGS_REINSERT: u32 =
    regs::d(0) | regs::d(1) | regs::d(2) | regs::a(1) | regs::a(2) | regs::a(3) | regs::a(4) | regs::CCR;

impl SortedList {
    /// `reinsert` avec les registres de l'original, selon le chemin suivi (voir `Reinsert`).
    #[track_caller]
    pub fn reinsert_68k(self, c: &mut Cpu, b: &mut Cps2Bus, obj: Obj) {
        Reinsert { c, b, obj, list: self }.run();
        crate::diag::registres_morts(c, "lists", REGS_REINSERT);
    }
}

/// `unlink_all` avec le registre de l'original : A0 = dernier objet retiré (inchangé si les listes sont vides).
#[track_caller]
pub fn unlink_all_68k(c: &mut Cpu, b: &mut Cps2Bus) -> Option<Obj> {
    let last = unlink_all_regs(c, b);
    crate::diag::registres_morts(c, "lists", regs::a(0));
    last
}

fn unlink_all_regs(c: &mut Cpu, b: &mut Cps2Bus) -> Option<Obj> {
    let last = unlink_all(b);
    if let Some(o) = last {
        c.a[0] = o.0 | 0xff00_0000; // movea.w : étendu en signe
    }
    last
}

fn reinsert_native(list: SortedList, c: &mut Cpu, b: &mut Cps2Bus) {
    let obj = Obj(c.a[0]);
    Reinsert { c, b, obj, list }.run();
    rts(c, b);
}

/// 0x1f0be : retire l'objet A0 de sa liste. L'original ne laisse aucun registre utile : les appelants Rust
/// appellent `unlink` directement.
fn unlink_native(c: &mut Cpu, b: &mut Cps2Bus) {
    unlink(b, Obj(c.a[0]));
    rts(c, b);
}

/// 0x1f184 : vide les deux listes.
fn unlink_all_native(c: &mut Cpu, b: &mut Cps2Bus) {
    unlink_all_regs(c, b);
    rts(c, b);
}

pub const REINSERT_ACTORS: Native = Native {
    entry: 0x1ee1a,
    name: "list_reinsert_actors",
    run: crate::Corps::Processeur(|c, b| reinsert_native(ACTORS, c, b)),
    touches_sound: false,
    outputs: regs::ALL,
};

pub const REINSERT_FOREGROUND: Native = Native {
    entry: 0x1ef6c,
    name: "list_reinsert_foreground",
    run: crate::Corps::Processeur(|c, b| reinsert_native(FOREGROUND, c, b)),
    touches_sound: false,
    outputs: regs::ALL,
};

pub const UNLINK: Native = Native {
    entry: 0x1f0be,
    name: "list_unlink",
    run: crate::Corps::Processeur(unlink_native),
    touches_sound: false,
    outputs: regs::a(0),
};

pub const UNLINK_ALL: Native = Native {
    entry: 0x1f184,
    name: "list_unlink_all",
    run: crate::Corps::Processeur(unlink_all_native),
    touches_sound: false,
    outputs: regs::a(0),
};
