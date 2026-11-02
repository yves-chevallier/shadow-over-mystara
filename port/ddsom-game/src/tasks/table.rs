//! Table des tâches en Rust : remplace les enregistrements FF0064 du noyau d'origine (voir `docs/noyau.md`).
//!
//! 16 emplacements, numérotés par leur décalage dans la table d'origine divisé par 16 (`kill(0x30)` : emplacement 3).
//! L'ordonnanceur les examine de 0 à 15 (dans l'original, D0 décompte alors de 15 à 0). Chaque emplacement porte
//! l'état et le compte d'attente de l'original, la pile 68000 donnée à la tâche et ce qu'il faut pour la reprendre :
//! son entrée tant qu'elle n'a pas tourné (son étape d'entrée : `tasks::entry`), puis l'étape Rust où elle reprendra
//! (ce qu'elle relit à sa reprise est dans les champs de l'étape, et non plus dans des registres sauvés), et ses
//! appels en cours ([`Frame`]). La table garde aussi où en est le processeur (`kernel::Point`).
//!
//! La table vit dans le bus (`Cps2Bus::ext`) : la machine du jeu n'écrit plus FF0052-FF01A5 (pile de
//! l'ordonnanceur, tâche courante, tas des piles, enregistrements), et les enveloppes de cession ne sauvent plus les
//! registres sur la pile de la tâche. [`gaps`] compare la table à celle de la RAM d'une machine d'origine.
//!
//! Une tâche reprend toujours à une étape ([`Step`], `Resume::Step`) et ses appels en cours continuent à des étapes
//! ([`Next::Step`]) : l'ordonnanceur appelle son pas sans poser de PC. L'adresse d'origine de l'étape
//! (`Step::address`) ne sert plus qu'au diagnostic, et aux registres que l'original y laisse (épilogue étranger des
//! bulles). Une suite désignée par une adresse ([`Next::Address`]) n'est plus qu'un retour de la pile 68000 : celui
//! d'un appel par adresse (`crate::call`, sentinelle) ou d'un outil de diagnostic qui appelle une native.

use cps2::Cps2Bus;
use m68k::Cpu;

use super::Step;
use crate::mem::rd32;

/// Nombre d'emplacements.
pub const SLOTS: usize = 16;
/// Adresse de l'enregistrement d'origine de l'emplacement 0 ; l'emplacement k est à `+ 0x10 k`.
pub const RECORDS: u32 = 0xff_0064;

/// Adresse (fictive dans la machine du jeu) de l'enregistrement de l'emplacement `k`, que l'original laisse dans A0.
pub fn record(k: usize) -> u32 {
    RECORDS + 0x10 * k as u32
}

/// Emplacement désigné par un décalage dans la table (octet bas de D0 des traps).
pub fn slot(offset: u32) -> usize {
    (offset as usize & 0xff) / 0x10
}

/// État d'un emplacement (octet +0 de l'enregistrement d'origine).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Free,
    /// Attend que le VBLANK ait décompté son compte jusqu'à zéro.
    Waiting,
    /// Attend qu'une autre tâche la réveille (trap 6).
    Suspended,
    /// Sera lancée au prochain passage de l'ordonnanceur.
    Ready,
    /// Lancée, n'a pas encore cédé la main.
    InProgress,
    /// Créée (trap 0) ou relancée (trap 7) : démarrera à son entrée.
    Fresh,
}

impl State {
    /// Octet d'état de l'original.
    pub fn byte(self) -> u8 {
        match self {
            State::Free => 0,
            State::Waiting => 1,
            State::Suspended => 2,
            State::Ready => 4,
            State::InProgress => 8,
            State::Fresh => 0xc,
        }
    }

    pub fn from_byte(byte: u8) -> Option<State> {
        [State::Free, State::Waiting, State::Suspended, State::Ready, State::InProgress, State::Fresh]
            .into_iter()
            .find(|e| e.byte() == byte)
    }

    /// L'ordonnanceur lance la tâche (état ≥ 4 dans l'original).
    pub fn castable(self) -> bool {
        matches!(self, State::Ready | State::InProgress | State::Fresh)
    }
}

/// Les tâches du jeu, reconnues à leur entrée. Toutes sont en Rust (natives à leur entrée et à leurs adresses de
/// reprise) sauf l'écran `0x19068`, jamais atteint par les scénarios.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Task {
    /// `0x11b16` (emplacement 0) : initialisation, avertissements, création des crédits et du gel (`modes/boot.rs`),
    /// puis la machine à modes (`modes/`), dans laquelle elle continue sans changer d'entrée.
    Startup,
    /// `0x133fe` (emplacement 0) : la machine à modes relancée après une partie (`modes/gameover.rs`).
    Modes,
    /// `0x1c45a` (emplacement 0) : la partie (`tasks/game.rs`).
    Game,
    /// `0x29162` (emplacement 0) : menu TEST (`tasks/service.rs`).
    Service,
    /// `0x14ba`... `0x1ced4` (emplacement 1) : fondus de palette (`tasks/fading.rs`).
    Fading,
    /// `0x13c0a` (emplacement 2) : crédits et départs (`credits_task.rs`).
    Credits,
    /// `0x150ac` (emplacement 3) : interface des joueurs (`hud/`).
    Interface,
    /// `0xf7dfe` (emplacement 12) : portraits de la boutique (`scene/shop.rs`).
    Portraits,
    /// `0xe03fe` (emplacement 13) : « INSERT COIN » clignotant (`tasks/prompt.rs`).
    Prompt,
    /// `0xee080` (emplacement 14) : attente active et avance rapide (`tasks/freeze.rs`).
    Freeze,
    /// `0x1835e` (emplacement 15) : tableau des scores de l'attract (`tasks/fixed_screen.rs`).
    Scores,
    /// Autre entrée : code d'origine (l'écran `0x19068`, emplacement 15).
    #[default]
    Original,
}

impl Task {
    pub fn from_entry(entry: u32) -> Task {
        match entry {
            0x11b16 => Task::Startup,
            0x133fe => Task::Modes,
            0x1c45a => Task::Game,
            0x29162 => Task::Service,
            0x14ba | 0x14c8 | 0x14d0 | 0x14de | 0x1536 | 0x153e | 0x1ccf8 | 0x1ce64 | 0x1ced4 | 0x1cf12 => Task::Fading,
            0x13c0a => Task::Credits,
            0x150ac => Task::Interface,
            0xf7dfe => Task::Portraits,
            0xe03fe => Task::Prompt,
            0xee080 => Task::Freeze,
            0x1835e => Task::Scores,
            _ => Task::Original,
        }
    }
}

/// Où reprendre une tâche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resume {
    /// Pas encore lancée : son entrée (son étape Rust : `tasks::entry`).
    Entry(u32),
    /// A cédé la main ; reprendra à cette étape Rust. L'original sauve ses 15 registres et son SR à la cession et les
    /// restaure ; la sonde `DDSOM_POISON=resume` (`diag::probe_launch`, tous les registres et drapeaux inversés à la
    /// reprise, 296 scénarios) montre que seuls quelques-uns sont relus, et ils sont des champs nommés des étapes qui
    /// les relisent (voir `tasks::resume_step`). Le SR rendu est toujours celui du mode utilisateur, interruptions
    /// permises : aucune tâche ne cède en les masquant (`kernel::cede`).
    Step(Step),
}

/// Ce qui suit un appel en cours, ou une cession : une native à son adresse, ou une étape Rust.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    Address(u32),
    Step(Step),
}

impl Next {
    /// Adresse d'origine (celle que l'original empile).
    pub fn address(self) -> u32 {
        match self {
            Next::Address(a) => a,
            Next::Step(e) => e.address(),
        }
    }
}


impl From<Step> for Next {
    fn from(e: Step) -> Self {
        Next::Step(e)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    /// La tâche qui occupe l'emplacement (d'après sa dernière entrée).
    pub task: Task,
    pub state: State,
    /// Images d'attente restantes (gardé tel quel par les états qui ne s'en servent pas, comme l'original).
    pub count: u8,
    /// Pile 68000 donnée à la tâche à sa création, rendue à sa mort.
    pub stack: u32,
    pub resume: Resume,
    /// Appels en cours de la tâche, tenus en Rust au lieu de sa pile 68000 (voir [`Frame`]).
    pub frames: Frames,
}

impl Default for Slot {
    fn default() -> Self {
        Slot {
            task: Task::Original,
            state: State::Free,
            count: 0,
            stack: 0,
            resume: Resume::Entry(0),
            frames: Frames::default(),
        }
    }
}

/// Un appel en cours d'une tâche, à la place de sa pile 68000. Quand un pas Rust appelle un sous-programme qui peut
/// céder la main (fondu, écran noir, gestionnaire d'objet, mise à jour du monde...), la suite de l'appelant est rangée
/// ici au lieu d'une adresse de retour empilée (ce que le sous-programme garde pendant qu'il cède est dans son étape) :
/// à chaque cession, la pile 68000 de la tâche reste ainsi vide, et sa reprise ne dépend plus que de la table (étape,
/// contexte, cadres).
///
/// Le retour d'un sous-programme (`rts`) revient au plus récent des appels en cours : le dernier cadre de la tâche, ou
/// un appel par adresse ouvert après lui ([`Table::calls`]). La profondeur de chacun dit lequel ; le pointeur de pile
/// 68000 (A7) n'y sert plus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    /// Nombre d'appels hors cadre en cours ([`Table::calls`]) quand le cadre a été ouvert : un appel par adresse
    /// ouvert après lui (`crate::call`, imbriqué) revient avant lui.
    pub calls: usize,
    /// Quand l'appelé revient (`rts` d'une native, fin d'une étape, ou cession terminale `jmp 0xa40`), la tâche
    /// continue à cette suite.
    pub next: Next,
    /// Appel attendu par une fonction Rust en cours (voir [`call_expected`]) : son retour rend la main à cette
    /// fonction au lieu d'exécuter la suite. La suite ne sert que si la tâche cède avant.
    pub expected: Option<u32>,
}

/// Profondeur maximale des appels en cours d'une tâche (6 dans le jeu : tour, fin d'image, phase, monde, aiguillage,
/// sous-programme d'un gestionnaire).
pub const DEPTH: usize = 8;

/// Pile des appels en cours d'une tâche (le dernier est le plus récent).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frames {
    list: [Frame; DEPTH],
    n: usize,
}

impl Default for Frames {
    fn default() -> Self {
        Frames { list: [Frame { calls: 0, next: Next::Address(0), expected: None }; DEPTH], n: 0 }
    }
}

impl Frames {
    pub fn push(&mut self, frame: Frame) {
        assert!(self.n < DEPTH, "trop d'appels en cours pour une tâche : {:x?}", self.iter().collect::<Vec<_>>());
        self.list[self.n] = frame;
        self.n += 1;
    }

    pub fn pop(&mut self) -> Option<Frame> {
        self.n = self.n.checked_sub(1)?;
        Some(self.list[self.n])
    }

    pub fn top(&self) -> Option<Frame> {
        self.n.checked_sub(1).map(|i| self.list[i])
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// Du plus ancien au plus récent.
    pub fn iter(&self) -> impl Iterator<Item = Frame> + '_ {
        self.list[..self.n].iter().copied()
    }

    /// L'appel attendu `id` n'est plus attendu (la fonction Rust qui l'attendait l'abandonne) : s'il est encore là, son
    /// retour exécutera sa suite.
    fn leave(&mut self, id: u32) {
        for frame in &mut self.list[..self.n] {
            if frame.expected == Some(id) {
                frame.expected = None;
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Table {
    pub slots: [Slot; SLOTS],
    /// Dernière tâche lancée (celle qui cède quand une enveloppe de cession est appelée).
    pub current: usize,
    /// Piles libres : la dernière est la prochaine donnée (le tas FF005A de l'original, rempli par `boot_init`).
    pub stacks: Vec<u32>,
    /// Où en est le processeur (point du démarrage ou de l'ordonnanceur, ou tâche en cours).
    pub point: crate::kernel::Point,
    /// Pendant l'interruption de VBLANK (`Point::Interruption`) : le point où le processeur reviendra.
    pub interrupted: Option<crate::kernel::Point>,
    /// Numéro du prochain appel attendu, et dernier appel attendu revenu (voir [`call_expected`]).
    next_call: u32,
    returned: Option<u32>,
    /// Appels en cours hors des cadres des tâches, du plus ancien au plus récent : appels par adresse
    /// (`crate::call`), et, dans les outils, natives appelées par le code d'origine. Ils ne cèdent jamais la main :
    /// aucun n'est en cours à une cession (ils ne sont donc à aucune tâche).
    pub calls: Vec<OutsideFrame>,
    /// Restes de registres que les gestionnaires d'objets se transmettent (`dispatch::Leftovers`) : un état de la partie.
    /// Le code encore sur le processeur (gestionnaires non convertis, liste des sprites, caméras...) les tient dans le
    /// banc (D2-D5) : ils passent du banc à la table à l'entrée d'un gestionnaire converti, et reviennent au banc à sa
    /// sortie (`Native::execute`).
    pub leftovers: crate::dispatch::Leftovers,
}

/// Un appel en cours hors des cadres d'une tâche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutsideFrame {
    /// Appel par adresse d'une fonction Rust (`crate::call`) : le retour de la native lui rend la main.
    ByAddress,
    /// Outils (machine qui exécute aussi le code d'origine) : native appelée par un `jsr` du 68000, dont l'adresse de
    /// retour est sur la pile 68000.
    By68000,
}

/// Ce que devient la tâche au retour d'un sous-programme (voir [`ret`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Return {
    /// La suite de l'appel en cours, rangée par un pas Rust.
    Next(Next),
    /// La fonction Rust qui attendait ce retour reprend la main (appel attendu, appel par adresse).
    Render,
    /// Aucun appel en cours tenu en Rust : l'adresse de retour est sur la pile 68000 (outils seulement ; dans le jeu,
    /// c'est une erreur).
    Stack68000,
}

impl Table {
    /// Trap 0 : la tâche `k` (re)part de `entry` avec une pile neuve ; celle d'une tâche déjà là est rendue.
    pub fn create(&mut self, k: usize, entry: u32) {
        if self.slots[k].state != State::Free {
            self.give_back_stack(k);
        }
        let stack = self.stacks.pop().expect("plus de pile libre pour une tâche");
        let task = Task::from_entry(entry);
        self.slots[k] = Slot { task, state: State::Fresh, stack, resume: Resume::Entry(entry), ..Slot::default() };
    }

    /// Trap 1 (la tâche se termine) et trap 2 sur une tâche vivante.
    pub fn free(&mut self, k: usize) {
        self.slots[k].state = State::Free;
        self.slots[k].frames = Frames::default();
        self.give_back_stack(k);
    }

    /// Trap 2 : tue la tâche `k`, si elle existe.
    pub fn kill(&mut self, k: usize) {
        if self.slots[k].state != State::Free {
            self.free(k);
        }
    }

    fn give_back_stack(&mut self, k: usize) {
        self.stacks.push(self.slots[k].stack);
    }

    /// Trap 5 : suspend la tâche `k`.
    pub fn suspend(&mut self, k: usize) {
        let e = &mut self.slots[k];
        e.state = State::Suspended;
        e.count = 0;
    }

    /// Trap 6 : la tâche `k` redevient prête (avec ce compte, que rien ne lit).
    pub fn wake(&mut self, k: usize, count: u8) {
        let e = &mut self.slots[k];
        e.state = State::Ready;
        e.count = count;
    }

    /// Trap 7 : la tâche courante repart de `entry` (même pile).
    pub fn restart(&mut self, entry: u32) {
        let e = &mut self.slots[self.current];
        e.task = Task::from_entry(entry);
        e.state = State::Fresh;
        e.count = 0;
        e.resume = Resume::Entry(entry);
        e.frames = Frames::default();
    }

    /// Cession de la tâche courante : nouvel état, et où la reprendre.
    pub fn cede(&mut self, state: State, count: Option<u8>, resume: Resume) {
        let e = &mut self.slots[self.current];
        e.state = state;
        if let Some(count) = count {
            e.count = count;
        }
        e.resume = resume;
    }

    /// Une image de moins pour la tâche `k` si elle attend ; à zéro, elle est prête.
    pub fn count_down(&mut self, k: usize) {
        let e = &mut self.slots[k];
        if e.state == State::Waiting {
            e.count = e.count.wrapping_sub(1);
            if e.count == 0 {
                e.state = State::Ready;
            }
        }
    }

    /// Réveils du VBLANK : chaque tâche qui attend perd une image ; l'emplacement 0 (le premier examiné) ne
    /// décompte qu'une image sur quatre tant que le ralenti (`A5+0x4f`, décompté ici) n'est pas épuisé.
    pub fn count_down_frame(&mut self, slowdown: &mut u8) {
        for k in 0..SLOTS {
            if self.slots[k].state != State::Waiting {
                continue;
            }
            if *slowdown != 0 && k == 0 {
                *slowdown -= 1;
                if *slowdown & 3 != 0 {
                    continue;
                }
            }
            self.count_down(k);
        }
    }

    /// Appels en cours de la tâche courante.
    pub fn frames(&mut self) -> &mut Frames {
        &mut self.slots[self.current].frames
    }
}

/// La table des tâches de la machine (créée au premier accès).
pub fn table(b: &mut Cps2Bus) -> &mut Table {
    let ext = b.ext.get_or_insert_with(|| Box::new(Table::default()));
    ext.as_mut().as_any_mut().downcast_mut().expect("extension du bus : table des tâches")
}

/// La table des tâches, si la machine en a une (machine du jeu ; la machine d'origine garde la sienne en RAM).
pub fn read(b: &Cps2Bus) -> Option<&Table> {
    b.ext.as_deref()?.as_any().downcast_ref()
}

/// La table des tâches, si la machine en a une, sans la créer.
fn table_if(b: &mut Cps2Bus) -> Option<&mut Table> {
    b.ext.as_deref_mut()?.as_any_mut().downcast_mut()
}

// ---------------------------------------------------------------------------------------------
// Appels en cours (voir `Frame`)

/// Un pas Rust de la tâche courante appelle un sous-programme qui peut céder la main : quand celui-ci reviendra, la
/// tâche continuera à `next`. Rien n'est empilé sur la pile 68000.
pub fn call(b: &mut Cps2Bus, next: impl Into<Next>) {
    table(b).open_frame(next.into(), None);
}

/// Appel attendu (voir [`call_expected`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub struct Call(u32);

/// Une fonction Rust appelle une native qui revient par `rts` (le gestionnaire d'un objet) et continue après elle : le
/// retour de la native lui rend la main ([`returned`]). Si la native cède la main avant, la tâche reprendra plus tard à
/// `next`, la fonction Rust ayant abandonné.
pub fn call_expected(b: &mut Cps2Bus, next: impl Into<Next>) -> Call {
    let t = table(b);
    t.next_call = t.next_call.wrapping_add(1);
    let id = t.next_call;
    t.open_frame(next.into(), Some(id));
    Call(id)
}

/// L'appel attendu est encore en cours (son cadre est dans la pile des appels de la tâche courante).
pub fn pending(b: &Cps2Bus, call: Call) -> bool {
    read(b).is_some_and(|t| t.slots[t.current].frames.iter().any(|k| k.expected == Some(call.0)))
}

/// L'appel attendu est-il revenu ? Sinon (la tâche a cédé, ou un appelé a dépilé l'appel pour revenir plus haut), la
/// fonction Rust qui l'attendait abandonne : son cadre, s'il est encore là, exécutera sa suite quand l'appelé reviendra.
pub fn returned(b: &mut Cps2Bus, call: Call) -> bool {
    let t = table(b);
    if t.returned == Some(call.0) {
        t.returned = None;
        return true;
    }
    t.frames().leave(call.0);
    false
}

/// Outils : l'appel attendu `call`, fait à du code d'origine, est revenu par la pile 68000 (adresse de retour
/// sentinelle) : son cadre, le plus récent de la tâche courante, est fermé comme par le `rts` d'une native.
pub fn give_back_expected(b: &mut Cps2Bus, call: Call) {
    let t = table(b);
    let frame = t.frames().pop();
    assert!(frame.is_some_and(|k| k.expected == Some(call.0)), "appel attendu {call:?} : ce n'est pas le dernier cadre ({frame:x?})");
    t.returned = Some(call.0);
}

impl Table {
    /// Le plus récent des appels en cours de la tâche courante : un appel hors cadre ouvert après le dernier cadre (il
    /// est dépilé et rendu), sinon le dernier cadre (dépilé et rendu).
    fn pop_most_recent(&mut self) -> Option<Result<OutsideFrame, Frame>> {
        let top = self.frames().top();
        let height = top.map_or(0, |k| k.calls);
        assert!(
            self.calls.len() >= height,
            "appels hors cadre {:?} revenus avant le cadre {top:x?} ouvert pendant eux",
            self.calls
        );
        if self.calls.len() > height {
            return self.calls.pop().map(Ok);
        }
        self.frames().pop().map(Err)
    }

    /// Retour d'un sous-programme (`rts`) : il revient au plus récent des appels en cours ([`Frame`]).
    pub fn ret(&mut self) -> Return {
        match self.pop_most_recent() {
            Some(Ok(OutsideFrame::ByAddress)) => Return::Render,
            Some(Ok(OutsideFrame::By68000)) | None => Return::Stack68000,
            Some(Err(Frame { expected: Some(id), .. })) => {
                self.returned = Some(id);
                Return::Render
            }
            Some(Err(frame)) => Return::Next(frame.next),
        }
    }

    /// Un pas Rust de la tâche courante appelle un sous-programme qui peut céder ; `expected` : appel attendu.
    fn open_frame(&mut self, next: Next, expected: Option<u32>) {
        let calls = self.calls.len();
        self.frames().push(Frame { calls, next, expected });
    }
}

/// Retour d'un sous-programme (`rts`) : il revient au plus récent des appels en cours ([`Frame`]).
pub fn ret(b: &mut Cps2Bus) -> Return {
    table_if(b).map_or(Return::Stack68000, Table::ret)
}

/// Outils : dépile l'adresse de retour de la pile 68000 (`rts` d'une native appelée par le code d'origine).
pub fn pop_68000(c: &mut Cpu, b: &mut Cps2Bus) -> u32 {
    let address = rd32(b, c.a[7]);
    c.a[7] = c.a[7].wrapping_add(4);
    address
}

/// Un appel par adresse (`crate::call`) commence ; son `rts` le fermera ([`ret`]). Rend la profondeur à laquelle
/// revenir.
pub fn open_call(b: &mut Cps2Bus) -> usize {
    let t = table(b);
    t.calls.push(OutsideFrame::ByAddress);
    t.calls.len() - 1
}

/// L'appel par adresse ouvert à la profondeur `depth` est-il revenu ?
pub fn call_returned(b: &Cps2Bus, depth: usize) -> bool {
    read(b).is_some_and(|t| t.calls.len() == depth)
}

/// Outils : le code d'origine appelle une native (son adresse de retour est sur la pile 68000). Rend la profondeur à
/// rétablir après elle ([`close_calls`]) si la machine a une table des tâches (sans table, tous les retours sont sur
/// la pile 68000).
pub fn open_call_68000(b: &mut Cps2Bus) -> Option<usize> {
    let t = table_if(b)?;
    t.calls.push(OutsideFrame::By68000);
    Some(t.calls.len() - 1)
}

/// Outils : la native appelée par le code d'origine est finie ; si elle n'est pas revenue par `rts` (saut, cession),
/// son appel est fermé.
pub fn close_calls(b: &mut Cps2Bus, depth: Option<usize>) {
    if let (Some(p), Some(t)) = (depth, table_if(b)) {
        t.calls.truncate(p);
    }
}

/// Comme [`ret`], pour une cession terminale (`jmp 0xa40`) ou un appel abandonné : la suite du dernier cadre, qu'elle
/// soit attendue ou non (la fonction Rust qui l'attendait voit qu'elle n'est pas revenue). Une cession ne peut avoir
/// lieu dans un appel par adresse.
pub fn pop_return(c: &mut Cpu, b: &mut Cps2Bus) -> Next {
    match table_if(b).and_then(Table::pop_most_recent) {
        Some(Err(frame)) => frame.next,
        Some(Ok(OutsideFrame::ByAddress)) => panic!("cession ou abandon de l'appel en cours dans un appel par adresse"),
        Some(Ok(OutsideFrame::By68000)) | None => {
            assert!(!crate::no68k::active(), "cession ou abandon sans appel en cours");
            Next::Address(pop_68000(c, b))
        }
    }
}

/// Les `n` appels en cours les plus récents de la tâche courante sont dépilés sans revenir (une routine qui dépile les
/// retours de ses appelants pour revenir plus haut : épilogue étranger des bulles) ; rend leurs suites, de la plus
/// récente à la plus ancienne.
pub fn pop_calls(b: &mut Cps2Bus, n: usize) -> Vec<Next> {
    let frames = table(b).frames();
    (0..n)
        .map(|_| frames.pop().unwrap_or_else(|| panic!("{n} appels en cours à dépiler : il n'y en a pas assez")).next)
        .collect()
}

/// Octets de la RAM du noyau d'origine (décalage depuis FF0000) que le jeu ne tient plus en RAM : pile de
/// l'ordonnanceur, tâche courante et sommet du tas (FF0052-FF005D), enregistrements (FF0064-FF0163), tas des piles
/// (FF0166-FF01A5), piles des tâches (FF01A6-FF0DA5) et du superviseur (jusqu'à FF0E65). Les outils qui comparent la
/// RAM du jeu à celle de l'original les ignorent et comparent la table avec [`gaps`]. FF0E66 et la suite sont des
/// variables du jeu (relatives à A5 : `placement::LAST_X`...), comparées comme le reste de la RAM.
pub fn kernel_ram(offset: u32) -> bool {
    (0x52..0x5e).contains(&offset) || (0x64..0x164).contains(&offset) || (0x166..0xe66).contains(&offset)
}

// ---------------------------------------------------------------------------------------------
// Test différentiel : table Rust contre la table FF0064 d'une machine d'origine

/// Champs d'un enregistrement de l'original.
mod field {
    pub const STATE: u32 = 0;
    pub const COUNT: u32 = 1;
    pub const PC: u32 = 4;
    pub const USP: u32 = 8;
    pub const STACK: u32 = 0xc;
}

/// Reprises des enveloppes de cession de l'original (`0xa40`/`0xb64`, `0xa86`) : elles restaurent les 15 registres
/// sauvés sur la pile de la tâche puis reviennent (`rts`) à l'adresse de reprise.
const ORIGINAL_RESUMES: [u32; 2] = [0xa4c, 0xa8c];
/// Tas des piles de l'original : sommet (FF005A) et fond.
const HEAP: u32 = 0xff_005a;
const HEAP_BOTTOM: u32 = 0xff_01a6;

/// Emplacement de la tâche principale (partie, machine à modes), que la tâche du gel décompte elle-même.
const PRINCIPALE: usize = 0;

/// Les deux états de la tâche principale ne diffèrent que d'une image décomptée par la tâche du gel (avance rapide).
/// Quand le jeu travaille jusqu'au bout de l'image (avance rapide d'une scène chargée), la fin d'image tombe à un
/// instant qui dépend de la durée des routines, natives ou d'origine : peu avant ou peu après un passage du gel, voire
/// au milieu (compte décrémenté, état pas encore « prête »).
fn one_gap_countdown(rust: (State, u8), original: (u8, u8)) -> bool {
    let waits = State::Waiting.byte();
    let next = |(state, count): (u8, u8)| match (state == waits, count.wrapping_sub(1)) {
        (true, 0) => Some((State::Ready.byte(), 0)),
        (true, remainder) => Some((waits, remainder)),
        _ => None,
    };
    let rust = (rust.0.byte(), rust.1);
    // « en cours » vaut « prête » ; « attend » à zéro est un décompte du gel interrompu entre le compte et l'état
    let normal = |(state, count): (u8, u8)| {
        let ready = state == State::InProgress.byte() || state == waits && count == 0;
        (if ready { State::Ready.byte() } else { state }, count)
    };
    let (rust, original) = (normal(rust), normal(original));
    rust == original || next(rust) == Some(original) || next(original) == Some(rust)
}

/// Différences entre la table Rust et la table FF0064 de la RAM `ram` (FF0000-FFFFFF) d'une machine d'origine, à la
/// même image : état (« prête » et « en cours » confondus : la fin d'image peut tomber pendant qu'une tâche tourne
/// dans l'original ; pour la tâche principale, une image d'écart décomptée par le gel est admise, voir
/// [`one_gap_countdown`]), compte, pile, entrée d'une tâche neuve, et tas des piles. Avec `all`, l'adresse de reprise
/// des tâches qui ont cédé aussi (diagnostic) : elle peut différer sans effet (une native reprend parfois à une copie
/// identique du code de reprise d'origine, `0xf32cc` pour `0xf5902`).
pub fn gaps(t: &Table, ram: &[u16], all: bool) -> Vec<String> {
    let word = |a: u32| ram[(a as usize & 0xffff) / 2];
    let byte = |a: u32| (word(a & !1) >> if a & 1 == 0 { 8 } else { 0 }) as u8;
    let long = |a: u32| (word(a) as u32) << 16 | word(a + 2) as u32;
    let mut gaps = Vec::new();
    for (k, e) in t.slots.iter().enumerate() {
        let r = record(k);
        let state = byte(r + field::STATE);
        let normal = |o: u8| if o == State::InProgress.byte() { State::Ready.byte() } else { o };
        let mut gap = |what: &str, rust: u32, original: u32| {
            if rust != original {
                gaps.push(format!("tâche {k} : {what} {rust:x} au lieu de {original:x}"));
            }
        };
        let count = byte(r + field::COUNT);
        if !(k == PRINCIPALE && one_gap_countdown((e.state, e.count), (state, count))) {
            gap("état", normal(e.state.byte()) as u32, normal(state) as u32);
            gap("compte", e.count as u32, count as u32);
        }
        if e.state == State::Free || state == State::Free.byte() {
            continue;
        }
        gap("pile", e.stack, long(r + field::STACK));
        let in_progress = e.state == State::InProgress || state == State::InProgress.byte();
        match e.resume {
            Resume::Entry(entry) => gap("entrée", entry, long(r + field::PC)),
            // la pile d'une tâche en cours a changé depuis sa cession
            Resume::Step(step) if all && !in_progress => {
                let usp = long(r + field::USP);
                if !ORIGINAL_RESUMES.contains(&long(r + field::PC)) {
                    gap("reprise (PC de l'enregistrement)", step.address(), long(r + field::PC));
                    continue;
                }
                gap("reprise", step.address(), long(usp + 60));
            }
            Resume::Step(_) => {}
        }
    }
    let top = if long(HEAP) == 0 { HEAP_BOTTOM } else { long(HEAP) };
    let heap: Vec<u32> = (top..HEAP_BOTTOM).step_by(4).rev().map(long).collect();
    if heap != t.stacks {
        gaps.push(format!("tas des piles {:x?} au lieu de {:x?}", t.stacks, heap));
    }
    gaps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation_and_death_return_stacks_in_order() {
        let mut t = Table { stacks: vec![3, 2, 1], ..Table::default() };
        t.create(0, 0x100);
        t.create(3, 0x200);
        assert_eq!((t.slots[0].stack, t.slots[3].stack), (1, 2));
        t.create(0, 0x300);
        assert_eq!(t.slots[0].stack, 1, "la pile rendue est reprise aussitôt");
        t.kill(3);
        t.kill(3);
        assert_eq!(t.stacks, vec![3, 2]);
    }

    #[test]
    fn a_return_goes_back_to_the_most_recent_pending_call() {
        let mut t = Table::default();
        t.open_frame(Next::Address(0x1341a), None);
        t.open_frame(Next::Address(0x1376e), Some(1));
        // un appel par adresse imbriqué revient le premier
        t.calls.push(OutsideFrame::ByAddress);
        assert_eq!(t.ret(), Return::Render);
        assert!(t.calls.is_empty());
        assert_eq!(t.ret(), Return::Render);
        assert_eq!(t.returned, Some(1));
        // un appel par adresse ouvert avant un cadre revient après lui
        t.calls.push(OutsideFrame::ByAddress);
        t.open_frame(Next::Address(0x1376e), None);
        assert_eq!(t.ret(), Return::Next(Next::Address(0x1376e)));
        assert_eq!(t.ret(), Return::Render);
        assert_eq!(t.ret(), Return::Next(Next::Address(0x1341a)));
        assert_eq!(t.ret(), Return::Stack68000);
    }

    #[test]
    fn a_frame_counted_down_by_freeze_is_allowed() {
        assert!(one_gap_countdown((State::Waiting, 1), (4, 0)));
        assert!(!one_gap_countdown((State::Waiting, 2), (8, 0)));
        assert!(one_gap_countdown((State::Waiting, 2), (1, 3)));
        assert!(one_gap_countdown((State::Ready, 0), (1, 1)));
        assert!(!one_gap_countdown((State::Waiting, 3), (1, 1)));
        assert!(one_gap_countdown((State::Waiting, 1), (1, 0)));
        assert!(one_gap_countdown((State::Ready, 0), (1, 0)));
    }

    #[test]
    fn slowdown_counts_slot_0_down_one_frame_in_four() {
        let mut t = Table::default();
        for k in [0, 1] {
            t.slots[k] = Slot { state: State::Waiting, count: 2, ..Slot::default() };
        }
        let mut slowdown = 8;
        for _ in 0..4 {
            t.count_down_frame(&mut slowdown);
        }
        assert_eq!(slowdown, 4);
        assert_eq!((t.slots[0].state, t.slots[0].count), (State::Waiting, 1));
        assert_eq!(t.slots[1].state, State::Ready);
    }
}
