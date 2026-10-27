//! Phase 16 (`0xf3142`) : sortie de scène.
//!
//! Quand une scène est gagnée, la phase passe de 12 à 16 : une *séquence de sortie* (propre à la scène,
//! 36 variantes) fait sortir les joueurs, affiche les écrans de fin de niveau, choisit la scène suivante
//! (« choix de route ») puis relance la phase 8 (entrée de la scène suivante).
//!
//! Le bloc d'état `A5-0x55ca` (A0 dans l'original) contient : `+0` état (0 : initialisation, 2 :
//! séquence), `+1` numéro de séquence (deux fois l'indice, d'après la table des scènes `0xf3170`), `+2`
//! état de la séquence (pas de 4), `+4` minuterie, `+0x10`, `+0x12`, `+0x16` objets (messages, effets).
//!
//! Chaque séquence est un tableau d'états ([`St`]) exécuté un par image, puis un « sillage » de fin
//! d'image ([`Update`]) qui met le monde à jour sauf à l'état final. Les cinq *états partagés* (0xf590e,
//! 0xf5938, 0xf6492, 0xf792e, 0xf7ecc : décompte, bonus, intermède des personnages, boutique, fondu au noir)
//! sont dans leurs propres modules ([`super::tally`], [`super::bonus`], [`super::characters`],
//! [`super::shop`], [`super::blackout`]) ; ils sont appelés comme par l'aiguillage d'origine (`jsr (A1)`, son
//! suite rangée dans les appels en cours) et cèdent souvent la main à l'ordonnanceur : la tâche reprend alors à
//! une étape de leur module, qui finit par revenir à la fin d'aiguillage de la séquence (`entrée + 10`,
//! [`Etape::DispatchTail`]).
//!
//! L'état final d'une séquence (`Leave`) prépare la scène suivante puis cède la main une fois
//! (`0xa40`, quelques images) avant de finir par des appels sons (`0xc0c`, `0xe0882`) : la tâche reprend à une
//! étape qui finit le travail ([`Etape`] : `Both`, `Sound`, `Command`, `Twice`).
//!
//! Les séquences 0, 6, 20, 29, 31 et 33 n'ont pas d'aiguillage par `jsr` : leurs états finissent eux-mêmes
//! l'image (`Update::Nothing`). La séquence 27 devient la 29 quand `A5+0x66` est posé.

use cps2::Cps2Bus;
use m68k::decode::Fetch;
use m68k::Cpu;

use super::{player, scratch, sub, var, Ctx, PLAYERS, PLAYER_STRIDE};
use crate::mem::{rd16, rd8, rts, wr16, wr32, wr8, A5};
use crate::modes::Sequence as _;
use crate::objects::Obj;
use crate::taches::table::Suite;
use crate::Native;
use crate::objects::pools;

/// Bloc d'état de la sortie de scène (A0 dans l'original) et ses champs.
const BASE: u32 = A5 - 0x55ca;
mod blk {
    pub const STATE: u32 = 0;
    pub const SEQUENCE: u32 = 1;
    pub const STEP: u32 = 2;
    pub const TIMER: u32 = 4;
    pub const OBJECT: u32 = 0x10;
    pub const MESSAGE: u32 = 0x12;
    pub const NEXT_SCENE: u32 = 0x14;
    pub const EFFECT: u32 = 0x16;
}

/// Tables en ROM : numéro de séquence par scène (octets) et branchements (mots, relatifs à la table).
const SEQUENCE_OF_SCENE: u32 = 0xf3170;
const SEQUENCE_JUMPS: u32 = 0xf31ea;

/// Sous-routines du jeu appelées par les séquences.
mod call {
    pub const PLAYER_INPUT: u32 = 0x2bdc;
    pub const WAIT: u32 = 0xa40;
    pub const SOUND_C0C: u32 = 0xc0c;
    pub const SOUND_E0882: u32 = 0xe0882;
    pub const COLLISIONS: u32 = 0x260ee;
    pub const PLAYERS: u32 = 0x35292;
    pub const PLAYER_PARTS: u32 = 0x392e4;
    /// Boucles de dispatch des classes d'objets, dans l'ordre de la mise à jour réduite.
    pub const CLASSES: [u32; 9] = [0x398e2, 0x3996c, 0x39a52, 0x39b70, 0x39cba, 0x39e86, 0x39e50, 0x39f34, 0x39f82];
    pub const SPRITES_RESET: u32 = 0x1de7c;
    pub const EVENT_E57D6: u32 = 0xe57d6;
}

/// Joueur : champs écrits quand les joueurs quittent l'écran.
const WALK_OUT_STATE: u32 = 0x000c_0000;
const P_STATE_LONG: u32 = 4;
const WALK_OUT_CLEAR_BYTES: [u32; 3] = [0x66, 0x8a, 0x15c];
const P_RANK: u32 = 0x7d;
/// Longs des joueurs remis à zéro en quittant (état `+0x24` de chacun).
const PLAYER_LONGS: [u32; 4] = [0x602, 0x802, 0xa02, 0xc02];

/// Interdit de sauter (`A5+0x3d`), verrou d'entrées, drapeaux d'animation.
const PAUSE: u32 = 0x3d;
const PHASE_ENTRY: u32 = 0x0800_0000;
const PHASE_ENDING: u32 = 0x1c00_0000;
const FLAGS: u32 = 0x14;
const FLAGS_KEEP: u16 = 0xfff1;

/// Contexte d'une image de sortie de scène.
pub struct Ex<'a> {
    pub x: Ctx<'a>,
    /// La main a été rendue à l'ordonnanceur ou à l'original : pas de retour (RTS) à faire.
    handed_off: bool,
    /// Adresse d'origine de la séquence en cours (pour lui céder la main).
    entry: u32,
    /// Où l'aiguillage d'origine reprend après l'état (`jsr (A1)`), vide pour la séquence 0.
    dispatch_return: Option<Suite>,
    /// La mise à jour du monde faite dans l'état a laissé A0 sur un objet : l'aiguillage d'origine compare
    /// alors l'octet `+2` de cet objet (et non l'état de la séquence) à l'état final.
    a0_clobbered: bool,
}

impl<'a> Ex<'a> {
    // --- bloc d'état ----------------------------------------------------------------------------

    fn b8(&mut self, off: u32) -> u8 {
        rd8(self.x.b, BASE + off)
    }
    fn set_b8(&mut self, off: u32, v: u8) {
        wr8(self.x.b, BASE + off, v)
    }
    fn b16(&mut self, off: u32) -> u16 {
        rd16(self.x.b, BASE + off)
    }
    fn set_b16(&mut self, off: u32, v: u16) {
        wr16(self.x.b, BASE + off, v)
    }
    fn next(&mut self) {
        let step = self.b8(blk::STEP).wrapping_add(4);
        self.set_b8(blk::STEP, step);
    }
    fn set_step(&mut self, step: u8) {
        self.set_b8(blk::STEP, step);
    }
    /// Retire une image à la minuterie ; vrai quand elle arrive à zéro.
    fn countdown(&mut self) -> bool {
        let left = self.b16(blk::TIMER).wrapping_sub(1);
        self.set_b16(blk::TIMER, left);
        left == 0
    }

    /// Appel d'une sous-routine du jeu avec A0 sur le bloc d'état.
    fn call(&mut self, addr: u32) {
        self.x.c.a[0] = BASE;
        self.x.call(addr);
        self.x.c.a[0] = BASE;
    }

    /// Lance la tâche de fondu `entry` (délai `delay`), A0 est conservé.
    fn fade(&mut self, delay: u16, entry: u32) {
        self.x.set_v16(var::FADE_DELAY, delay);
        self.x.set_v8(var::FADE_PAGES, 15);
        self.x.c.a[0] = entry;
        self.x.c.d[0] = 0x10;
        crate::kernel::creer_tache(self.x.b, 0x10, entry);
        self.x.c.a[0] = BASE;
    }

    /// Ouvre le message `n` ; son adresse (16 bits) va dans `+0x12`.
    fn message(&mut self, n: u32) {
        self.x.c.d[0] = n;
        self.call(sub::MESSAGE);
        let a1 = self.x.c.a[1] as u16;
        self.set_b16(blk::MESSAGE, a1);
    }
    fn message_done(&mut self) -> bool {
        let m = self.object(blk::MESSAGE);
        m.u8(self.x.b, 0x22) != 0
    }
    /// Objet désigné par un mot du bloc (`movea.w` : adresse étendue en signe).
    fn object(&mut self, off: u32) -> Obj {
        Obj(self.b16(off) as i16 as i32 as u32 & 0xff_ffff)
    }

    /// Les joueurs en jeu et vivants sortent : état « marche » et indicateurs remis à zéro. Si `rank`,
    /// chacun reçoit son rang d'arrivée (`+0x7d`).
    fn release_players(&mut self, rank: bool) {
        let mut n = 0u8;
        for k in 0..4 {
            let p = PLAYERS + k * PLAYER_STRIDE;
            if rd8(self.x.b, p + player::ACTIVE) == 0 || rd16(self.x.b, p + player::HEALTH) == 0 {
                continue;
            }
            wr8(self.x.b, p + player::ACTIVE, 1);
            wr32(self.x.b, p + P_STATE_LONG, WALK_OUT_STATE);
            if rank {
                wr8(self.x.b, p + P_RANK, n);
                n += 1;
            }
            for off in WALK_OUT_CLEAR_BYTES {
                wr8(self.x.b, p + off, 0);
            }
        }
        // registres laissés par la boucle d'origine (relus par les gestionnaires de l'image suivante)
        self.x.c.a[1] = PLAYERS + 4 * PLAYER_STRIDE;
        self.x.c.d[6] = 0xffff;
        if rank {
            self.x.c.d[5] = n as u32;
        }
    }
    /// Début de sortie : la pause est levée, les boutons libérés, les joueurs sortent.
    fn start_walk_out(&mut self, rank: bool) {
        self.x.set_v8(PAUSE, 0);
        self.x.set_v8(var::INPUT_LOCK, 0);
        self.call(call::PLAYER_INPUT);
        self.release_players(rank);
    }
    /// Les joueurs en jeu et vivants sont-ils tous dans l'état `state` (octet `+6`) ?
    fn all_in_state(&mut self, state: u8) -> bool {
        (0..4).all(|k| {
            let p = PLAYERS + k * PLAYER_STRIDE;
            rd8(self.x.b, p + player::ACTIVE) == 0
                || rd16(self.x.b, p + player::HEALTH) == 0
                || rd8(self.x.b, p + player::STATE) == state
        })
    }

    /// 0xf7ee0 : variables des scripts de transition remises à zéro.
    fn reset_transition(&mut self) {
        super::screen::clear_longs(self.x.b, super::screen::blocks::TRANSITION, 4);
    }
    /// 0xf7916 : bloc de la boutique remis à zéro (D0 = 0, comme l'original).
    fn reset_shop(&mut self) {
        super::screen::clear_longs(self.x.b, super::screen::blocks::SHOP, 5);
        self.x.c.d[0] = 0;
    }
    /// 0xf683c : zone de texte des écrans de fin effacée.
    fn clear_text(&mut self) {
        super::screen::clear_text(self.x.c, self.x.b);
    }

    /// Cède la main à l'original à l'adresse `pc` (aucun retour de la native).
    fn delegate(&mut self, pc: u32) {
        self.x.c.a[0] = BASE;
        self.x.jump_to(pc);
        self.handed_off = true;
    }
    /// Cède la main à l'ordonnanceur pour `frames` images ; la reprise a lieu à `resume` (sinon à la suite de
    /// l'appelant).
    fn wait_frames(&mut self, frames: u32, resume: Option<Etape>) {
        self.x.c.d[0] = frames;
        if let Some(back) = self.dispatch_return {
            self.x.push_return(back);
        }
        if let Some(resume) = resume {
            self.x.push_return(resume);
        }
        self.x.c.pc = call::WAIT;
        self.handed_off = true;
    }
}

// ---------------------------------------------------------------------------------------------
// États d'une séquence

/// Comment finit l'état de sortie d'une séquence : appels après la cession.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tail {
    /// `0xc0c` puis `0xe0882` (reprise `RESUME_BOTH`).
    Both,
    /// `0xc0c` seul.
    Sound,
    /// `0xe0882` seul.
    Command,
    /// `0xc0c`, deux images de plus, puis `0xe0882`.
    Twice,
    /// Aucun : `jmp 0xa40`, la cession revient directement à l'appelant de la phase.
    Jump,
}

/// Changement de scène à la sortie.
#[derive(Clone, Copy)]
enum NextScene {
    Same,
    Plus1,
    /// Scène suivante, sauf si le compteur d'images `A5+0x54` dépasse 0x1500 : scène 10.
    Plus1UnlessLate,
    Fixed(u8),
    /// Scène mémorisée dans le bloc (`+0x14`).
    FromBlock,
}

/// Attente de la fin du fondu avant de partir (`A5+0x48e` nul) : ce que fait l'état en attendant.
#[derive(Clone, Copy, PartialEq, Eq)]
enum WaitFade {
    No,
    /// Mise à jour du monde et sprites (le dispatcher n'a rien à ajouter).
    World,
    /// Entrées, mise à jour du monde et sprites.
    InputsWorld,
    /// Rien : le dispatcher fait la fin d'image.
    Nothing,
}

#[derive(Clone, Copy)]
struct Leave {
    wait_fade: WaitFade,
    /// Palettes au noir avant de partir (`A5+0x48e` = 15, `0x168c`).
    black: bool,
    /// Mot de la RAM remis à zéro (séquence 1).
    clear_word: Option<u32>,
    clear_flags: bool,
    next_scene: NextScene,
    phase: u32,
    /// Appel avant la cession.
    extra_call: Option<u32>,
    frames: u32,
    tail: Tail,
}

/// Sortie ordinaire : scène suivante inchangée, 4 images, `0xc0c` puis `0xe0882`.
const LEAVE: Leave = Leave {
    wait_fade: WaitFade::No,
    black: true,
    clear_word: None,
    clear_flags: true,
    next_scene: NextScene::Same,
    phase: PHASE_ENTRY,
    extra_call: None,
    frames: 4,
    tail: Tail::Both,
};
const fn leave_to(scene: u8) -> St {
    St::Leave(Leave { next_scene: NextScene::Fixed(scene), ..LEAVE })
}

/// Un état d'une séquence.
#[derive(Clone, Copy)]
enum St {
    /// Ne fait rien (attend un événement extérieur).
    Idle,
    /// État partagé par plusieurs séquences (écrans de fin de niveau).
    Shared(Shared),
    /// Remet à zéro les variables de transition (`0xf7ee0`) et le bloc de la boutique (`0xf7916`) ; minuterie
    /// `+4` à `timer`.
    Clear { timer: Option<u16> },
    /// Attend la fin de la minuterie, puis fait sortir les joueurs ; `timer` : nouvelle minuterie.
    WalkWait { timer: Option<u16> },
    /// Attend la fin de la minuterie, puis passe à l'état suivant.
    TimerWait,
    /// Fondu `0x14ba` (délai 2) et zone de texte effacée (`0xf683c`), minuterie `timer`.
    FadeScript { timer: u16 },
    /// Attend que tous les joueurs soient dans l'état `state`, puis passe à l'état suivant avec `timer`.
    PlayersThen { state: u8, timer: Option<u16> },
    /// État sur mesure.
    Custom(fn(&mut Ex)),
    /// État final : prépare la scène suivante.
    Leave(Leave),
}

/// Fin d'image de la séquence, après son état (sauf à l'état final).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Update {
    /// Entrées, monde, sprites, collisions.
    Full,
    /// Entrées, monde, sprites.
    NoCollisions,
    /// Monde et sprites.
    World,
    /// Entrées, joueurs, puis les classes sauf BOSS et caméras, sprites.
    Reduced,
    /// Rien : les états font eux-mêmes la mise à jour (séquence 0).
    Nothing,
}

struct Sequence {
    update: Update,
    /// État après lequel la fin d'image est omise.
    last: u8,
    states: &'static [St],
}

use Update::{Full, NoCollisions, Nothing, Reduced, World};
use St::Idle;

/// Les états partagés des séquences de sortie, à leur adresse d'origine.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shared {
    /// 0xf590e : décompte de fin de niveau ([`super::tally`]).
    Tally,
    /// 0xf5938 : écran de bonus.
    Bonus,
    /// 0xf6492 : scène propre à certains personnages.
    Characters,
    /// 0xf792e : boutique.
    Shop,
    /// 0xf7ecc : fondu au noir et remise à zéro de l'écran.
    Blackout,
}
const TALLY: St = St::Shared(Shared::Tally);
const BONUS: St = St::Shared(Shared::Bonus);
const CHARACTERS: St = St::Shared(Shared::Characters);
const SHOP: St = St::Shared(Shared::Shop);
const BLACKOUT: St = St::Shared(Shared::Blackout);

const CLEAR: St = St::Clear { timer: None };
const WALK: St = St::WalkWait { timer: None };

// ---------------------------------------------------------------------------------------------
// Exécution d'une image

pub fn phase_16(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    let mut ex = Ex { x: Ctx::new(c, b), handed_off: false, entry: 0, dispatch_return: None, a0_clobbered: false };
    ex.x.c.a[0] = BASE;
    match ex.b8(blk::STATE) {
        0 => init(&mut ex),
        2 => run_sequence(&mut ex),
        _ => {}
    }
    ex.handed_off
}

/// État 0 : verrou des joueurs et choix de la séquence d'après la scène.
fn init(ex: &mut Ex) {
    let state = ex.b8(blk::STATE).wrapping_add(2);
    ex.set_b8(blk::STATE, state);
    wr8(ex.x.b, A5 - 0x5b8e, 0);
    ex.x.set_v8(var::PLAYERS_FROZEN, 1);
    let scene = ex.x.v8(var::SCENE) as u32;
    let sequence = ex.x.b.fetch16((SEQUENCE_OF_SCENE + scene) & !1);
    let sequence = if scene & 1 == 0 { (sequence >> 8) as u8 } else { sequence as u8 };
    ex.set_b8(blk::SEQUENCE, sequence);
}

/// État 2 : la séquence choisie.
fn run_sequence(ex: &mut Ex) {
    let number = ex.b8(blk::SEQUENCE) as u32;
    ex.x.c.d[0] = number;
    let jump = ex.x.prog16(SEQUENCE_JUMPS + number);
    ex.x.c.d[1] = jump as u32;
    ex.entry = SEQUENCE_JUMPS + jump as u32;
    let (sequence, dispatcher) = match number as u8 / 2 {
        // la séquence 27 devient la 29 quand `A5+0x66` est posé ; sinon son aiguillage suit ce test (10 octets)
        27 if ex.x.v8(0x66) != 0 => (&SEQ_29, None),
        27 => (&SEQ_27, Some(ex.entry + 20)),
        index => {
            let Some(s) = sequence(index) else { return ex.delegate(ex.entry) };
            // l'aiguillage d'origine (10 octets) appelle l'état par `jsr (A1)` : sa pile en garde le retour ;
            // les séquences dont les états finissent eux-mêmes l'image sautent à l'état (`jmp (A1)`)
            (s, (s.update != Nothing).then(|| ex.entry + 10))
        }
    };
    ex.dispatch_return = dispatcher.map(dispatch_tail_at);
    let step = ex.b8(blk::STEP);
    match sequence.states.get(step as usize / 4) {
        None => return ex.delegate(ex.entry),
        Some(&St::Shared(shared)) => run_shared(ex, shared),
        Some(&state) => run_state(ex, state),
    }
    if !ex.handed_off {
        finish_frame(ex, sequence);
    }
}

/// Un état partagé, appelé comme par l'aiguillage d'origine (`jsr (A1)`, A0 sur le bloc) : s'il cède la main,
/// la reprise finira par revenir à l'aiguillage d'origine ; sinon la fin d'image compare l'octet `+2` du bloc
/// désigné par A0, que l'état peut avoir changé.
fn run_shared(ex: &mut Ex, shared: Shared) {
    use crate::modes::{Flow, Sequence};
    let run: fn(&mut Ctx) -> Flow = match shared {
        Shared::Tally => super::tally::run,
        Shared::Bonus => super::bonus::run,
        Shared::Characters => super::characters::run,
        Shared::Shop => super::shop::run,
        Shared::Blackout => super::blackout::run,
    };
    let Some(back) = ex.dispatch_return else { return ex.delegate(ex.entry) };
    ex.x.c.a[0] = BASE;
    ex.x.push_return(back);
    if run(&mut ex.x) == Flow::Yield {
        ex.handed_off = true;
        return;
    }
    ex.x.drop_return();
    ex.a0_clobbered = true;
}

/// Un état saute au début de l'aiguillage d'origine (`bra entrée`) après avoir changé d'étape : l'état de la
/// nouvelle étape s'exécute dans la même image, appelé comme par l'aiguillage (D0.b = étape, A1 = état).
fn redispatch(ex: &mut Ex, sequence: &Sequence) {
    const TABLE: u32 = 0x2c; // table des états de l'aiguillage, après son code
    let step = ex.b8(blk::STEP);
    crate::mem::set_low_byte(&mut ex.x.c.d[0], step);
    let table = ex.entry + TABLE + step as u32;
    ex.x.c.a[1] = (ex.x.prog16(table) as u32) << 16 | ex.x.prog16(table + 2) as u32;
    ex.x.c.a[0] = BASE;
    match sequence.states.get(step as usize / 4) {
        None => ex.delegate(ex.entry),
        Some(&St::Shared(shared)) => run_shared(ex, shared),
        Some(&state) => run_state(ex, state),
    }
}

/// Fin d'image : sauf à l'état final, mise à jour du monde.
fn finish_frame(ex: &mut Ex, sequence: &Sequence) {
    let step = if ex.a0_clobbered { rd8(ex.x.b, ex.x.c.a[0].wrapping_add(blk::STEP)) } else { ex.b8(blk::STEP) };
    if step == sequence.last {
        return;
    }
    run_update(ex, sequence.update);
}

fn run_update(ex: &mut Ex, update: Update) {
    if update == Update::Nothing {
        return;
    }
    if update != Update::World {
        ex.x.call(sub::INPUTS);
    }
    match update {
        Update::Reduced => {
            ex.x.call(call::PLAYERS);
            ex.x.call(call::PLAYER_PARTS);
            for class in call::CLASSES {
                ex.x.call(class);
            }
            ex.x.call(sub::SPRITES);
        }
        _ => {
            ex.x.call(sub::WORLD_UPDATE);
            ex.x.call(sub::SPRITES);
            if update == Update::Full {
                ex.x.call(call::COLLISIONS);
            }
        }
    }
}

/// Attend la fin de la minuterie, puis les joueurs sortent.
fn walk_wait(ex: &mut Ex, timer: Option<u16>) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    if let Some(t) = timer {
        ex.set_b16(blk::TIMER, t);
    }
    ex.start_walk_out(false);
}

fn run_state(ex: &mut Ex, state: St) {
    match state {
        St::Idle | St::Shared(_) => {}
        St::Clear { timer } => {
            ex.next();
            if let Some(t) = timer {
                ex.set_b16(blk::TIMER, t);
            }
            ex.reset_transition();
            ex.reset_shop();
        }
        St::WalkWait { timer } => walk_wait(ex, timer),
        St::TimerWait => {
            if ex.countdown() {
                ex.next();
            }
        }
        St::FadeScript { timer } => {
            ex.next();
            ex.set_b16(blk::TIMER, timer);
            ex.fade(2, sub::FADE_TASK);
            ex.clear_text();
        }
        St::PlayersThen { state, timer } => {
            if ex.all_in_state(state) {
                ex.next();
                if let Some(t) = timer {
                    ex.set_b16(blk::TIMER, t);
                }
            }
        }
        St::Custom(f) => f(ex),
        St::Leave(leave) => do_leave(ex, &leave),
    }
}

/// État final : la scène suivante se prépare, puis cession à l'ordonnanceur.
fn do_leave(ex: &mut Ex, leave: &Leave) {
    if leave.wait_fade != WaitFade::No && ex.x.v8(var::FADE_PAGES) != 0 {
        match leave.wait_fade {
            WaitFade::World => ex.x.world(),
            WaitFade::InputsWorld => {
                ex.x.call(sub::INPUTS);
                ex.x.world();
            }
            _ => {}
        }
        ex.a0_clobbered = leave.wait_fade != WaitFade::Nothing;
        return;
    }
    ex.x.set_v8(0x42, 0);
    if leave.black {
        ex.x.set_v8(var::FADE_PAGES, 15);
        ex.x.call(sub::PALETTES_BLACK);
    }
    if let Some(w) = leave.clear_word {
        wr16(ex.x.b, w, 0);
    }
    for off in PLAYER_LONGS {
        wr32(ex.x.b, A5 + off, 0);
    }
    ex.x.call(call::SPRITES_RESET);
    if leave.clear_flags {
        let flags = ex.x.v16(FLAGS) & FLAGS_KEEP;
        ex.x.set_v16(FLAGS, flags);
    }
    match leave.next_scene {
        NextScene::Same => {}
        NextScene::Plus1 => {
            let scene = ex.x.v8(var::SCENE).wrapping_add(1);
            ex.x.set_v8(var::SCENE, scene);
        }
        NextScene::Plus1UnlessLate => {
            let scene = ex.x.v8(var::SCENE).wrapping_add(1);
            ex.x.set_v8(var::SCENE, scene);
            if ex.x.v16(0x54) > 0x1500 {
                ex.x.set_v8(var::SCENE, 10);
            }
        }
        NextScene::Fixed(scene) => ex.x.set_v8(var::SCENE, scene),
        NextScene::FromBlock => {
            let scene = ex.b8(blk::NEXT_SCENE);
            ex.x.set_v8(var::SCENE, scene);
        }
    }
    ex.x.set_v32(var::PHASE, leave.phase);
    if let Some(addr) = leave.extra_call {
        ex.call(addr);
    }
    let resume = match leave.tail {
        Tail::Both => Some(Etape::Both),
        Tail::Sound => Some(Etape::Sound),
        Tail::Command => Some(Etape::Command),
        Tail::Twice => Some(Etape::Twice),
        Tail::Jump => None,
    };
    ex.wait_frames(leave.frames, resume);
}

// ---------------------------------------------------------------------------------------------
// Reprises

/// Étapes de la sortie de scène : fins d'aiguillage des séquences (retour d'un état partagé qui a cédé la main) et
/// reprises après la cession de l'état final.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Etape {
    /// Fin d'aiguillage de la séquence d'indice donné (`entrée + 10`).
    DispatchTail(u8),
    /// `0xf32cc` : `0xc0c` puis `0xe0882`.
    Both,
    /// `0xf481a` : `0xc0c`.
    Sound,
    /// `0xf4b48` : `0xe0882`.
    Command,
    /// `0xf3cec` : `0xc0c`, puis deux images d'attente avant `TwiceEnd`.
    Twice,
    /// `0xf3cfa` : `0xe0882`.
    TwiceEnd,
}

/// Entrées des aiguillages à `jsr (A1)`, par indice de séquence (la 27 a son aiguillage après un test de 10 octets).
const DISPATCHERS: [(u8, u32); 29] = [
    (1, 0xf32d8), (2, 0xf33fe), (3, 0xf3594), (4, 0xf3686), (5, 0xf3812), (7, 0xf39f4), (8, 0xf3b18),
    (9, 0xf3c0a), (10, 0xf3d00), (11, 0xf3e3c), (12, 0xf3f2e), (13, 0xf4020), (14, 0xf4176), (15, 0xf434a),
    (16, 0xf4480), (17, 0xf455e), (18, 0xf464a), (19, 0xf4738), (21, 0xf493a), (22, 0xf4a68), (23, 0xf4b4e),
    (25, 0xf4c6e), (26, 0xf4da2), (27, 0xf4ea4), (28, 0xf4f9c), (30, 0xf5390), (32, 0xf565e), (34, 0xf52a4),
    (35, 0xf5782),
];

/// Fin d'aiguillage à l'adresse `at` (`entrée + 10`).
fn dispatch_tail_at(at: u32) -> Suite {
    match DISPATCHERS.iter().find(|&&(_, entry)| entry + 10 == at) {
        Some(&(index, _)) => Etape::DispatchTail(index).into(),
        None => Suite::Adresse(at),
    }
}

impl Etape {
    /// Adresse que l'original reprend à cette étape.
    pub fn adresse(self) -> u32 {
        match self {
            Etape::DispatchTail(index) => DISPATCHERS.iter().find(|d| d.0 == index).expect("aiguillage").1 + 10,
            Etape::Both => 0xf32cc,
            Etape::Sound => 0xf481a,
            Etape::Command => 0xf4b48,
            Etape::Twice => 0xf3cec,
            Etape::TwiceEnd => 0xf3cfa,
        }
    }

    pub fn pas(self, x: &mut Ctx) -> crate::modes::Flow {
        match self {
            Etape::DispatchTail(index) => dispatch_tail(x, index),
            Etape::Both => {
                x.call(call::SOUND_C0C);
                x.call(call::SOUND_E0882);
            }
            Etape::Sound => x.call(call::SOUND_C0C),
            Etape::Command | Etape::TwiceEnd => x.call(call::SOUND_E0882),
            Etape::Twice => {
                x.call(call::SOUND_C0C);
                x.c.d[0] = 2;
                crate::kernel::attendre(x.c, x.b, Etape::TwiceEnd);
                return crate::modes::Flow::Yield;
            }
        }
        crate::modes::Flow::Done
    }
}

pub const PHASE_16: Native = Native {
    entry: 0xf3142,
    name: "scene_phase_16",
    run: |c, b| {
        if !phase_16(c, b) {
            rts(c, b);
        }
    },
    touches_sound: false,
    outputs: 0,
};

/// Fin d'image de l'aiguillage d'origine d'une séquence (`entrée + 10` : `cmpi.b #dernier,2(A0)`, puis la mise à
/// jour), où reviennent les reprises après une cession. A0 est celui que l'état a laissé.
fn dispatch_tail(x: &mut Ctx, index: u8) {
    let sequence = if index == 27 { &SEQ_27 } else { sequence(index).expect("séquence réécrite") };
    let mut ex = Ex { x: Ctx::new(x.c, x.b), handed_off: false, entry: 0, dispatch_return: None, a0_clobbered: true };
    ex.x.c.a[5] = A5;
    finish_frame(&mut ex, sequence);
}

pub const NATIVES: &[Native] = &[PHASE_16];

// ---------------------------------------------------------------------------------------------
// Les séquences (indice = numéro de séquence / 2 ; voir `docs/scenes.md` pour les scènes)

/// État final sans effacement des drapeaux, cession de 8 images, seule la commande finale ensuite.
const L_SILENT: Leave = Leave { clear_flags: false, frames: 8, tail: Tail::Command, ..LEAVE };

static SEQ_0: Sequence = Sequence {
    update: Nothing,
    last: 0xff,
    states: &[
        St::Custom(seq0_start),
        St::Leave(Leave { wait_fade: WaitFade::World, next_scene: NextScene::Plus1, ..LEAVE }),
    ],
};
static SEQ_1: Sequence = Sequence {
    update: Reduced,
    last: 24,
    states: &[
        St::Custom(walk_out_now),
        St::Custom(seq1_clear),
        Idle,
        BONUS,
        Idle,
        BLACKOUT,
        St::Leave(Leave { clear_word: Some(scratch(0xa53a)), ..LEAVE }),
    ],
};
static SEQ_2: Sequence = Sequence {
    update: Full,
    last: 40,
    states: &[
        St::Custom(seq2_start),
        St::Custom(seq2_wait_exit),
        St::Custom(seq2_message_done),
        St::Custom(seq2_boss_state),
        St::Custom(seq2_countdown),
        St::TimerWait,
        TALLY,
        BONUS,
        CHARACTERS,
        BLACKOUT,
        St::Leave(LEAVE),
    ],
};
/// Modèle commun (séquences 3, 8, 11, 12) : image, sortie, états partagés, départ.
static SEQ_3: Sequence =
    Sequence { update: Full, last: 28, states: &[CLEAR, WALK, TALLY, BONUS, CHARACTERS, SHOP, BLACKOUT, St::Leave(LEAVE)] };
static SEQ_4: Sequence = Sequence {
    update: Full,
    last: 40,
    states: &[
        CLEAR,
        St::WalkWait { timer: Some(144) },
        St::Custom(seq4_probe),
        St::Custom(seq4_effect),
        St::Custom(message_then_next),
        TALLY,
        BONUS,
        CHARACTERS,
        SHOP,
        BLACKOUT,
        St::Leave(LEAVE),
    ],
};
static SEQ_5: Sequence = Sequence {
    update: World,
    last: 8,
    states: &[
        St::Clear { timer: Some(32) },
        St::Custom(walk_with_effect),
        St::Leave(Leave { wait_fade: WaitFade::World, next_scene: NextScene::Plus1UnlessLate, ..LEAVE }),
    ],
};
/// Séquence 6 (scène 7) : comme la séquence 0, avec la commande sonore 0xe0a82(4).
static SEQ_6: Sequence = Sequence {
    update: Nothing,
    last: 0xff,
    states: &[
        St::Custom(seq6_start),
        St::Leave(Leave { wait_fade: WaitFade::World, next_scene: NextScene::Plus1, ..LEAVE }),
    ],
};
static SEQ_7: Sequence = Sequence {
    update: World,
    last: 8,
    states: &[
        St::Clear { timer: Some(32) },
        St::Custom(walk_with_effect),
        St::Leave(Leave { wait_fade: WaitFade::World, next_scene: NextScene::Plus1, ..LEAVE }),
    ],
};
static SEQ_9: Sequence = Sequence {
    update: Full,
    last: 24,
    states: &[CLEAR, WALK, TALLY, BONUS, CHARACTERS, BLACKOUT, St::Leave(Leave { frames: 2, tail: Tail::Twice, ..LEAVE })],
};
static SEQ_10: Sequence = Sequence {
    update: NoCollisions,
    last: 36,
    states: &[
        St::Custom(seq10_clear),
        St::Custom(seq10_walk),
        St::PlayersThen { state: 10, timer: Some(112) },
        Idle,
        St::TimerWait,
        TALLY,
        BONUS,
        CHARACTERS,
        BLACKOUT,
        St::Leave(LEAVE),
    ],
};
static SEQ_13: Sequence = Sequence {
    update: Full,
    last: 32,
    states: &[
        CLEAR,
        WALK,
        TALLY,
        St::FadeScript { timer: 60 },
        Idle,
        St::PlayersThen { state: 16, timer: None },
        BONUS,
        CHARACTERS,
        leave_to(106),
    ],
};
static SEQ_14: Sequence = Sequence {
    update: Full,
    last: 44,
    states: &[
        CLEAR,
        St::WalkWait { timer: Some(96) },
        TALLY,
        St::FadeScript { timer: 60 },
        St::Custom(seq14_wait_players),
        St::Custom(seq14_message_done),
        St::Custom(seq14_countdown),
        St::Custom(seq14_message_done_wait),
        St::TimerWait,
        BONUS,
        CHARACTERS,
        leave_to(27),
    ],
};
static SEQ_15: Sequence = Sequence {
    update: Full,
    last: 36,
    states: &[
        St::Custom(seq15_clear),
        WALK,
        TALLY,
        St::Custom(seq15_fade),
        Idle,
        BLACKOUT,
        BONUS,
        CHARACTERS,
        SHOP,
        St::Leave(LEAVE),
    ],
};
static SEQ_16: Sequence = Sequence {
    update: NoCollisions,
    last: 24,
    states: &[St::Custom(seq16_start), BLACKOUT, BONUS, CHARACTERS, SHOP, BLACKOUT, St::Leave(LEAVE)],
};
static SEQ_17: Sequence = Sequence {
    update: Full,
    last: 28,
    states: &[CLEAR, WALK, TALLY, BONUS, CHARACTERS, SHOP, BLACKOUT, St::Leave(Leave { tail: Tail::Sound, ..LEAVE })],
};
static SEQ_18: Sequence =
    Sequence { update: Full, last: 24, states: &[CLEAR, WALK, TALLY, BONUS, CHARACTERS, BLACKOUT, St::Leave(LEAVE)] };
static SEQ_19: Sequence = Sequence {
    update: NoCollisions,
    last: 24,
    states: &[St::Clear { timer: Some(32) }, WALK, BONUS, CHARACTERS, SHOP, BLACKOUT, St::Leave(Leave { tail: Tail::Sound, ..LEAVE })],
};
/// Séquence 20 (scènes 35 et 111) : sortie des joueurs après 48 images de plus, fondu, scène 33.
static SEQ_20: Sequence = Sequence {
    update: Nothing,
    last: 0xff,
    states: &[
        St::Custom(seq20_walk),
        St::Custom(fade_out_world),
        St::Leave(Leave { wait_fade: WaitFade::World, next_scene: NextScene::Fixed(33), ..LEAVE }),
    ],
};
static SEQ_21: Sequence = Sequence {
    update: NoCollisions,
    last: 12,
    states: &[
        St::Custom(seq21_start),
        St::Custom(seq21_route),
        St::Leave(Leave { wait_fade: WaitFade::Nothing, next_scene: NextScene::FromBlock, ..LEAVE }),
    ],
};
static SEQ_22: Sequence =
    Sequence { update: Full, last: 28, states: &[CLEAR, WALK, TALLY, BONUS, CHARACTERS, SHOP, BLACKOUT, St::Leave(L_SILENT)] };
static SEQ_23: Sequence = Sequence {
    update: NoCollisions,
    last: 8,
    states: &[
        St::Clear { timer: Some(128) },
        St::Custom(seq23_walk),
        St::Leave(Leave { wait_fade: WaitFade::InputsWorld, next_scene: NextScene::Fixed(44), ..LEAVE }),
    ],
};
static SEQ_25: Sequence = Sequence {
    update: Full,
    last: 36,
    states: &[CLEAR, WALK, TALLY, St::Custom(seq25_route), St::TimerWait, BONUS, CHARACTERS, SHOP, BLACKOUT, St::Leave(L_SILENT)],
};
static SEQ_26: Sequence =
    Sequence { update: Full, last: 28, states: &[CLEAR, WALK, TALLY, BLACKOUT, BONUS, CHARACTERS, SHOP, leave_to(38)] };
/// Séquence 27 (scène 54), quand `A5+0x66` est nul : décompte, bonus, scène 55.
static SEQ_27: Sequence =
    Sequence { update: Full, last: 16, states: &[St::Custom(seq27_clear), WALK, TALLY, BONUS, leave_to(55)] };
static SEQ_28: Sequence =
    Sequence { update: Full, last: 24, states: &[St::Custom(seq28_clear), WALK, TALLY, BONUS, CHARACTERS, SHOP, leave_to(59)] };
/// Séquence 29 (scène 59, et 54 quand `A5+0x66` est posé) : sortie et fondu, scène suivante.
static SEQ_29: Sequence = Sequence {
    update: Nothing,
    last: 0xff,
    states: &[
        St::Custom(seq29_start),
        St::Custom(seq29_walk),
        St::Leave(Leave { wait_fade: WaitFade::InputsWorld, next_scene: NextScene::Plus1, ..LEAVE }),
    ],
};
static SEQ_30: Sequence = Sequence {
    update: Full,
    last: 52,
    states: &[
        CLEAR,
        St::WalkWait { timer: Some(96) },
        TALLY,
        St::FadeScript { timer: 60 },
        St::PlayersThen { state: 12, timer: Some(64) },
        St::Custom(seq30_message_79),
        St::Custom(seq30_message_done),
        St::Custom(seq30_effect),
        St::Custom(seq30_players_wait),
        St::TimerWait,
        BONUS,
        CHARACTERS,
        SHOP,
        leave_to(65),
    ],
};
/// Séquence 31 (scène 65) : fondu après 192 images, scène suivante sans palettes au noir ni son.
static SEQ_31: Sequence = Sequence {
    update: Nothing,
    last: 0xff,
    states: &[
        St::Custom(seq31_start),
        St::Custom(fade_out_world),
        St::Leave(Leave {
            wait_fade: WaitFade::World,
            black: false,
            clear_flags: false,
            next_scene: NextScene::Plus1,
            tail: Tail::Jump,
            ..LEAVE
        }),
    ],
};
static SEQ_32: Sequence = Sequence {
    update: Full,
    last: 24,
    states: &[St::Custom(seq32_clear), WALK, TALLY, BONUS, St::Custom(seq32_route), CHARACTERS, leave_to(105)],
};
/// Séquence 33 (scène 67) : sortie des joueurs et fondu, scène 38.
static SEQ_33: Sequence = Sequence {
    update: Nothing,
    last: 0xff,
    states: &[
        St::Custom(seq33_start),
        St::Custom(seq33_walk),
        St::Leave(Leave { wait_fade: WaitFade::World, next_scene: NextScene::Fixed(38), tail: Tail::Sound, ..LEAVE }),
    ],
};
static SEQ_34: Sequence = Sequence {
    update: NoCollisions,
    last: 28,
    states: &[
        St::Clear { timer: Some(4) },
        WALK,
        SHOP,
        BONUS,
        CHARACTERS,
        SHOP,
        BLACKOUT,
        St::Leave(Leave { tail: Tail::Sound, ..LEAVE }),
    ],
};
static SEQ_35: Sequence = Sequence {
    update: Full,
    last: 20,
    states: &[
        St::Custom(seq35_clear),
        St::Custom(seq35_walk),
        BLACKOUT,
        TALLY,
        St::Custom(seq35_fade),
        St::Custom(seq35_message),
        St::Custom(seq35_boss),
        St::Leave(Leave { phase: PHASE_ENDING, extra_call: Some(call::EVENT_E57D6), ..LEAVE }),
    ],
};

/// Séquence réécrite d'indice `index` ; `None` : cédée à l'original.
fn sequence(index: u8) -> Option<&'static Sequence> {
    Some(match index {
        0 => &SEQ_0,
        1 => &SEQ_1,
        2 => &SEQ_2,
        3 | 8 | 11 | 12 => &SEQ_3,
        4 => &SEQ_4,
        5 => &SEQ_5,
        6 => &SEQ_6,
        7 => &SEQ_7,
        9 => &SEQ_9,
        10 => &SEQ_10,
        13 => &SEQ_13,
        14 => &SEQ_14,
        15 => &SEQ_15,
        16 => &SEQ_16,
        17 => &SEQ_17,
        18 => &SEQ_18,
        19 => &SEQ_19,
        20 => &SEQ_20,
        21 => &SEQ_21,
        22 | 24 => &SEQ_22,
        23 => &SEQ_23,
        25 => &SEQ_25,
        26 => &SEQ_26,
        28 => &SEQ_28,
        29 => &SEQ_29,
        30 => &SEQ_30,
        31 => &SEQ_31,
        32 => &SEQ_32,
        33 => &SEQ_33,
        34 => &SEQ_34,
        35 => &SEQ_35,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------------------------
// États sur mesure

/// Séquence 0 (la plus courante) : les joueurs sortent, fondu.
fn seq0_start(ex: &mut Ex) {
    ex.next();
    ex.fade(2, 0x14d0);
    ex.x.set_v8(var::INPUT_LOCK, 0);
    ex.call(call::PLAYER_INPUT);
    ex.x.world();
}

/// Séquence 6 : comme `seq0_start`, avec la commande sonore 0xe0a82(4).
fn seq6_start(ex: &mut Ex) {
    ex.next();
    ex.fade(2, 0x14d0);
    ex.x.set_v8(var::INPUT_LOCK, 0);
    ex.call(call::PLAYER_INPUT);
    ex.x.c.d[0] = ex.x.c.d[0] & !0xffff | 4;
    ex.call(0xe0a82);
    ex.x.world();
}

/// Séquence 20 : les joueurs sortent à la fin de la minuterie (48 images de plus) ; mise à jour complète.
fn seq20_walk(ex: &mut Ex) {
    walk_wait(ex, Some(48));
    run_update(ex, Update::Full);
}

/// Séquences 20 et 31 : fondu de sortie à la fin de la minuterie ; le monde continue.
fn fade_out_world(ex: &mut Ex) {
    if ex.countdown() {
        ex.next();
        ex.fade(2, 0x14d0);
    }
    ex.x.world();
}

/// Séquence 27 : caméra, minuterie + 90, variables de transition et bloc de la boutique remis à zéro.
fn seq27_clear(ex: &mut Ex) {
    ex.next();
    ex.x.set_v16(0x51e, 4);
    let t = ex.b16(blk::TIMER).wrapping_add(90);
    ex.set_b16(blk::TIMER, t);
    ex.reset_transition();
    ex.reset_shop();
}

/// Séquence 29 : minuterie + 80, variables de transition remises à zéro, puis l'attente dans la même image.
fn seq29_start(ex: &mut Ex) {
    ex.next();
    let t = ex.b16(blk::TIMER).wrapping_add(80);
    ex.set_b16(blk::TIMER, t);
    ex.reset_transition();
    seq29_walk(ex);
}

/// Séquence 29 : à la fin de la minuterie, les joueurs sortent et le fondu commence ; mise à jour complète.
fn seq29_walk(ex: &mut Ex) {
    if ex.countdown() {
        ex.next();
        ex.start_walk_out(false);
        ex.fade(2, 0x14d0);
    }
    run_update(ex, Update::Full);
}

/// Séquence 31 : minuterie 192, entrées libérées.
fn seq31_start(ex: &mut Ex) {
    ex.next();
    ex.set_b16(blk::TIMER, 192);
    ex.x.set_v8(var::INPUT_LOCK, 0);
    ex.call(call::PLAYER_INPUT);
    ex.x.world();
}

/// Séquence 33 : variables de transition et bloc de la boutique remis à zéro, puis l'attente dans la même image.
fn seq33_start(ex: &mut Ex) {
    ex.next();
    ex.reset_transition();
    ex.reset_shop();
    seq33_walk(ex);
}

/// Séquence 33 : à la fin de la minuterie, fondu de sortie puis les joueurs sortent.
fn seq33_walk(ex: &mut Ex) {
    if ex.countdown() {
        ex.next();
        ex.x.set_v8(PAUSE, 0);
        ex.x.set_v8(var::INPUT_LOCK, 0);
        ex.call(call::PLAYER_INPUT);
        ex.fade(2, 0x14d0);
        ex.release_players(false);
    }
    ex.x.world();
}

/// Séquence 10 : variables de transition remises à zéro, minuterie 128.
fn seq10_clear(ex: &mut Ex) {
    ex.next();
    ex.set_b16(blk::TIMER, 128);
    ex.reset_transition();
}

/// Séquence 32 : selon la route (`A5+0x5c`), la sortie ou l'écran de bonus 0xf6492.
fn seq32_route(ex: &mut Ex) {
    if ex.x.v8(0x5c) != 0 {
        ex.set_step(24);
        if let Some(St::Leave(leave)) = SEQ_32.states.get(6) {
            do_leave(ex, leave);
        }
    } else {
        ex.set_step(20);
        redispatch(ex, &SEQ_32);
    }
}

/// Séquence 1 : les joueurs sortent aussitôt.
fn walk_out_now(ex: &mut Ex) {
    ex.next();
    ex.start_walk_out(false);
}

/// Séquence 1 : variables de transition remises à zéro, puis l'événement 0xdc5fe(1).
fn seq1_clear(ex: &mut Ex) {
    ex.next();
    ex.reset_transition();
    ex.x.c.d[0] = 1;
    ex.call(0xdc5fe);
}

/// Séquence 2 : la minuterie (mot) est décrémentée une fois, puis les joueurs sortent s'il y en a.
fn seq2_start(ex: &mut Ex) {
    // la minuterie (mot) part de 0 : elle est décrémentée tant qu'elle reste positive
    let left = ex.b16(blk::TIMER).wrapping_sub(1);
    ex.set_b16(blk::TIMER, left);
    if left as i16 >= 0 || ex.x.v8(var::PLAYERS) == 0 {
        return;
    }
    ex.next();
    ex.start_walk_out(false);
}

/// Séquence 2 : attend que tous les joueurs soient à l'état de sortie 0xc0c00, puis dialogue 7.
fn seq2_wait_exit(ex: &mut Ex) {
    ex.reset_transition();
    for k in 0..4 {
        let p = PLAYERS + k * PLAYER_STRIDE;
        if rd8(ex.x.b, p) != 0 && rd16(ex.x.b, p + player::HEALTH) != 0 && crate::mem::rd32(ex.x.b, p + P_STATE_LONG) != 0x000c_0c00 {
            return;
        }
    }
    ex.next();
    ex.message(7);
}

/// Séquence 2 : message fini, le boss d'objet `+0x10` passe à l'état 0x04020a00.
fn seq2_message_done(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
        let o = ex.object(blk::OBJECT);
        wr32(ex.x.b, o.0 + 4, 0x0402_0a00);
        ex.set_b16(blk::TIMER, 240);
    }
}

fn seq2_boss_state(ex: &mut Ex) {
    let o = ex.object(blk::OBJECT);
    if o.u8(ex.x.b, 4) == 6 {
        ex.next();
        ex.set_b16(blk::TIMER, 160);
    }
}

fn seq2_countdown(ex: &mut Ex) {
    if ex.countdown() {
        ex.next();
        ex.set_b16(blk::TIMER, 128);
        ex.x.c.d[0] = 30;
        ex.call(0x2860);
        ex.call(0xe4830);
    }
}

/// Séquence 4 : après la sortie, un effet apparaît ; si un joueur est dans l'état 4 ou 5, dialogue 33.
fn seq4_probe(ex: &mut Ex) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    for k in 0..4 {
        let p = PLAYERS + k * PLAYER_STRIDE;
        if rd8(ex.x.b, p) != 0 && rd16(ex.x.b, p + player::HEALTH) != 0 {
            let character = rd8(ex.x.b, p + 3);
            if character == 4 || character == 5 {
                ex.next();
                ex.message(33);
                return;
            }
        }
    }
    let effect = ex.x.spawn(&pools::EFFCT, 0x59);
    ex.set_b16(blk::EFFECT, effect.0 as u16);
}

fn seq4_effect(ex: &mut Ex) {
    let e = ex.object(blk::EFFECT);
    if e.u8(ex.x.b, 5) == 2 {
        ex.next();
        ex.message(33);
    }
}

fn message_then_next(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
    }
}

/// Séquences 5 et 7 : les joueurs sortent, l'effet `A5+0x46` reçoit 0x20000 en `+0x58`, fondu de sortie.
fn walk_with_effect(ex: &mut Ex) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    ex.x.set_v8(PAUSE, 0);
    ex.x.set_v8(var::INPUT_LOCK, 0);
    ex.call(call::PLAYER_INPUT);
    ex.set_b16(blk::TIMER, 144);
    ex.release_players(false);
    let effect = Obj(ex.x.v16(0x46) as i16 as i32 as u32 & 0xff_ffff);
    wr32(ex.x.b, effect.0 + 0x58, 0x2_0000);
    ex.fade(1, 0x14d0);
}

/// Séquence 10 : les joueurs reçoivent leur rang puis 0xd5f12.
fn seq10_walk(ex: &mut Ex) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    ex.start_walk_out(true);
    ex.call(0xd5f12);
}

/// Séquence 14 : attend l'état 12 des joueurs, puis dialogue 35.
fn seq14_wait_players(ex: &mut Ex) {
    if ex.all_in_state(12) {
        ex.next();
        ex.message(35);
    }
}

fn seq14_message_done(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
        ex.set_b16(blk::TIMER, 256);
        ex.call(0x3525c);
    }
}

fn seq14_countdown(ex: &mut Ex) {
    if ex.countdown() {
        ex.next();
        ex.message(36);
        ex.call(0xe61ca);
    }
}

fn seq14_message_done_wait(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
        ex.set_b16(blk::TIMER, 288);
    }
}

/// Séquence 15 : annule la variable temporaire A5-0x65ac, remet à zéro transition et boutique.
fn seq15_clear(ex: &mut Ex) {
    ex.next();
    ex.x.wr16(scratch(0x9a54), 0);
    ex.reset_transition();
    ex.reset_shop();
}

/// Séquence 15 : fondu, script de fondu, événement 0xdc5fe(3).
fn seq15_fade(ex: &mut Ex) {
    ex.next();
    ex.fade(2, sub::FADE_TASK);
    ex.clear_text();
    ex.x.c.d[0] = 3;
    ex.call(0xdc5fe);
}

/// Séquence 16 : transition et boutique remises à zéro, les joueurs sortent.
fn seq16_start(ex: &mut Ex) {
    ex.next();
    ex.reset_transition();
    ex.reset_shop();
    ex.start_walk_out(false);
}

/// Séquence 21 : les joueurs sortent, dialogue 70 ; la scène suivante dépend du résultat.
fn seq21_start(ex: &mut Ex) {
    ex.next();
    ex.x.set_v8(PAUSE, 0);
    ex.x.set_v8(var::INPUT_LOCK, 0);
    ex.call(call::PLAYER_INPUT);
    ex.reset_transition();
    ex.reset_shop();
    ex.message(70);
    ex.release_players(false);
}

/// Séquence 21 : le message fini donne un indice (`+0x23`) dans une table de scènes suivantes.
fn seq21_route(ex: &mut Ex) {
    const NEXT_SCENES: u32 = 0xf4a0c;
    let m = ex.object(blk::MESSAGE);
    if m.u8(ex.x.b, 0x22) != 0 {
        ex.next();
        let index = m.u8(ex.x.b, 0x23) as u32;
        let scene = ex.x.prog16((NEXT_SCENES + index) & !1);
        let scene = if (NEXT_SCENES + index) & 1 == 0 { (scene >> 8) as u8 } else { scene as u8 };
        ex.set_b8(blk::NEXT_SCENE, scene);
        ex.fade(1, 0x14d0);
    }
}

/// Séquence 23 : les joueurs sortent puis fondu de sortie.
fn seq23_walk(ex: &mut Ex) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    ex.start_walk_out(false);
    ex.fade(2, 0x14d0);
}

/// Séquence 25 : selon la route (`A5+0x5c`), saute à l'état 20 (écran de bonus) ou fond.
fn seq25_route(ex: &mut Ex) {
    if ex.x.v8(0x5c) == 0 {
        ex.set_step(20);
        return redispatch(ex, &SEQ_25);
    }
    ex.next();
    ex.set_b16(blk::TIMER, 128);
    ex.fade(2, sub::FADE_TASK);
    ex.clear_text();
}

/// Séquence 28 : comme `Clear`, la minuterie augmente de 90.
fn seq28_clear(ex: &mut Ex) {
    ex.next();
    let t = ex.b16(blk::TIMER).wrapping_add(90);
    ex.set_b16(blk::TIMER, t);
    ex.reset_transition();
    ex.reset_shop();
}

/// Séquence 30 : suite de dialogues (79, 80) autour d'un effet 75 et de la sortie.
fn seq30_message_79(ex: &mut Ex) {
    if ex.countdown() {
        ex.next();
        ex.message(79);
    }
}

fn seq30_message_done(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
        ex.set_b16(blk::TIMER, 80);
    }
}

fn seq30_effect(ex: &mut Ex) {
    if ex.b16(blk::TIMER) == 32 {
        let e = ex.x.spawn(&pools::EFFCT, 75);
        e.set_u8(ex.x.b, Obj::VARIANT, 1);
        ex.call(0xed0c0);
    }
    if ex.countdown() {
        ex.next();
        ex.message(80);
    }
}

fn seq30_players_wait(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
        let players = ex.x.v8(var::PLAYERS) as u32;
        let t = ex.x.prog16(0xf5542 + 2 * players);
        ex.set_b16(blk::TIMER, t);
    }
}

/// Séquence 32 : efface, dessine ; si la route est prise (`A5+0x5c`) saute à l'état 12.
fn seq32_clear(ex: &mut Ex) {
    ex.next();
    ex.reset_transition();
    ex.reset_shop();
    if ex.x.v8(0x5c) != 0 {
        ex.set_step(12);
        return redispatch(ex, &SEQ_32);
    }
    walk_wait(ex, None);
}

/// Séquence 35 : variables de transition remises à zéro, puis les joueurs sortent après l'attente.
fn seq35_clear(ex: &mut Ex) {
    ex.next();
    ex.reset_transition();
    seq35_walk(ex);
}

fn seq35_walk(ex: &mut Ex) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    ex.x.set_v8(PAUSE, 0);
    ex.x.wr16(scratch(0xaa84), 0x1400);
    ex.x.set_v8(var::INPUT_LOCK, 0);
    ex.call(call::PLAYER_INPUT);
    ex.release_players(false);
}

fn seq35_fade(ex: &mut Ex) {
    ex.next();
    ex.set_b16(blk::TIMER, 64);
    ex.fade(2, sub::FADE_TASK);
    ex.clear_text();
    ex.x.c.d[0] = 4;
    ex.call(0xe0a82);
}

fn seq35_message(ex: &mut Ex) {
    if !ex.countdown() {
        return;
    }
    ex.next();
    ex.message(88);
    ex.x.c.d[0] = 24;
    ex.call(0xce00a);
    let a1 = ex.x.c.a[1] as u16;
    ex.set_b16(blk::OBJECT, a1);
    ex.x.c.d[0] = 0x7fff;
    ex.call(0x2860);
    ex.call(0xe5776);
}

fn seq35_boss(ex: &mut Ex) {
    if ex.message_done() {
        ex.next();
        let o = ex.object(blk::OBJECT);
        o.set_u8(ex.x.b, 4, 4);
        ex.x.set_v16(0x544, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresses_du_bloc_et_des_variables_temporaires() {
        assert_eq!(BASE, 0xff_2a36);
        assert_eq!(scratch(0x9a2a), 0xff_1a2a);
    }

    /// Chaque séquence a un état final multiple de 4 (ou 0xff) et se termine par un état de sortie.
    #[test]
    fn sequences_coherentes() {
        for index in 0..36 {
            let Some(s) = sequence(index) else { continue };
            assert!(s.last % 4 == 0 || s.last == 0xff, "séquence {index}");
            assert!(matches!(s.states.last(), Some(St::Leave(_)) | Some(St::Custom(_))), "séquence {index}");
        }
    }
}

/// État partagé 0xf792e (écran de fin de niveau) : après le message, 0xf7b62 choisit la suite d'après l'octet `+0xc` de
/// son bloc (A6) par une table de 20 octets (0xf7b88). Pour les scènes de démonstration 109 à 114, l'octet est hors
/// de la table et l'original saute à une adresse lue dans du code : il plante et redémarre. Hors table : aucune suite
/// (octet de la table nul, branchement en 0xf792c). Même correctif dans le code d'origine (machine de référence).
pub const SHARED_NEXT_ENTRY: u32 = 0xf7b62;
const SHARED_NEXT_TABLE_LEN: u8 = 20;
const SHARED_NEXT_NONE: u32 = 0xf792c;

pub fn patch_shared_next(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    if rd8(b, c.a[6] + 0xc) < SHARED_NEXT_TABLE_LEN {
        return false;
    }
    wr16(b, c.a[6] + 0x12, 0x65);
    c.d[0] &= 0xffff_0000;
    c.a[1] = 0xf7b88;
    c.pc = SHARED_NEXT_NONE;
    true
}
