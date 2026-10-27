//! Le moteur d'objets vu d'un gestionnaire : les routines appliquées à l'objet traité.
//!
//! Le trait [`Courant`] donne aux contextes qui traitent un objet (`families::cx::Cx`, `families::Ctx`) les routines
//! du moteur sous des noms Rust : `x.move_xz()`, `x.anim_step()`, `x.reinsert()`... au lieu de
//! `x.call(sub::MOVE_XZ)`. Chaque méthode appelle la fonction du module (`motion`, `anim`, `lists`, `cull`, `pools`)
//! sur `self.obj()`.
//!
//! Trois formes par routine (`docs/guide-refactorisation.md`) : le cœur (`motion::ballistic(b, o, motion::XZ)`), son
//! adaptateur `_68k` qui laisse aussi les registres de l'original (`motion::ballistic_68k(c, b, o, motion::XZ)`), la
//! native qui sert les appels par adresse restants. Les méthodes ci-dessous appellent le cœur quand les registres sont
//! prouvés morts pour tous les gestionnaires (`DDSOM_POISON`) ; seules `reinsert` et `reinsert_foreground` gardent
//! l'adaptateur (registres hérités par le gestionnaire suivant).

use super::anim::{self, CharacterSets};
use super::cull;
use super::lists::{self, ACTORS, FOREGROUND};
use super::motion::{self, LastAdd, SpeedTables, AXIS_Y, AXIS_Z};
use super::pools::Pool;
use super::Obj;
use crate::machine::Machine;



/// Contexte qui traite un objet (A0 de l'original).
pub trait Courant: Machine {
    /// L'objet traité.
    fn obj(&self) -> Obj;

    // --- pools ----------------------------------------------------------------------------------

    /// Alloue un objet dans `pool` : l'emplacement réservé (classe écrite, pas encore actif), ou `None` si le pool
    /// est plein. Cœur.
    fn alloc(&mut self, pool: &Pool) -> Option<Obj> {
        pool.alloc(self.bus())
    }

    /// Allocation dont l'original ne teste pas l'échec (voir `Pool::alloc_unchecked`, glitch d'origine). Cœur.
    fn alloc_unchecked(&mut self, pool: &Pool) -> Obj {
        pool.alloc_unchecked(self.bus())
    }

    /// Allocation dont l'échec rend l'A1 que l'original laisse (voir `Pool::alloc_or_leftover`). Cœur.
    fn alloc_or_leftover(&mut self, pool: &Pool) -> Result<Obj, Obj> {
        pool.alloc_or_leftover(self.bus())
    }

    /// Libère l'objet traité dans son pool (fin de gestionnaire : l'objet disparaît). Cœur.
    fn free(&mut self, pool: &Pool) {
        let o = self.obj();
        pool.release(self.bus(), o)
    }

    // --- animation (`anim`) -----------------------------------------------------------------------

    /// 0x1a32 : démarre le script `n` du jeu d'animations `set` (`lea set,A4 ; moveq #n,D0 ; jsr 0x1a32`). Rend
    /// l'adresse qui suit l'entrée chargée (jamais nulle : le `bne` qui suit parfois l'appel est toujours pris).
    fn anim_start(&mut self, set: u32, n: u16) -> u32 {
        let o = self.obj();
        let b = self.bus();
        anim::start(b, o, set, n)
    }

    /// 0x1a3c : démarre l'animation à l'entrée `at`.
    fn anim_start_entry(&mut self, at: u32) {
        let o = self.obj();
        let b = self.bus();
        anim::start_entry(b, o, at);
    }

    /// 0x2c16, 0x2c88, 0x2cf0 : démarre l'animation `n` du jeu propre au personnage de l'objet.
    fn anim_start_by_character(&mut self, sets: &CharacterSets, n: u16) {
        let o = self.obj();
        let b = self.bus();
        anim::start_by_character(b, o, sets, n);
    }

    /// 0x1a56 : un pas d'animation ; vrai quand le script est terminé, ou que la durée était nulle (retenue de
    /// l'original, voir `anim::Step::finished`).
    fn anim_step(&mut self) -> bool {
        let o = self.obj();
        let b = self.bus();
        anim::step(b, o, false).finished()
    }

    /// 0x2d58 : un pas d'animation à entrées longues (personnages) ; vrai à la fin du script.
    fn anim_step_long(&mut self) -> bool {
        let o = self.obj();
        let b = self.bus();
        anim::step(b, o, true).finished()
    }

    // --- mouvement (`motion`) ---------------------------------------------------------------------
    // Les mouvements rendent la dernière addition (`LastAdd`) : `if x.move_xz().gt()` remplace le `bgt` de
    // l'original ; un appelant qui ne teste rien ignore la valeur.


    /// Mouvements balistiques (vitesse += accélération, position += vitesse) sur les axes de la routine d'origine.
    fn move_axes(&mut self, axes: motion::Axes) -> LastAdd {
        let o = self.obj();
        let b = self.bus();
        motion::ballistic(b, o, axes)
    }
    /// 0x1d78 : X.
    fn move_x(&mut self) -> LastAdd {
        self.move_axes(motion::X)
    }
    /// 0x1e02 : Y (profondeur).
    fn move_y(&mut self) -> LastAdd {
        self.move_axes(motion::Y)
    }
    /// 0x1dbc : Z (altitude).
    fn move_z(&mut self) -> LastAdd {
        self.move_axes(motion::Z)
    }
    /// 0x1d62 : Y puis X.
    fn move_xy(&mut self) -> LastAdd {
        self.move_axes(motion::XY)
    }
    /// 0x1da6 : X puis Z.
    fn move_xz(&mut self) -> LastAdd {
        self.move_axes(motion::XZ)
    }
    /// 0x1dd4 : Y puis Z.
    fn move_yz(&mut self) -> LastAdd {
        self.move_axes(motion::YZ)
    }
    /// 0x1d90 : Y, X puis Z.
    fn move_xyz(&mut self) -> LastAdd {
        self.move_axes(motion::XYZ)
    }

    /// 0x1cb4 : X et Y d'après la table de vitesses de l'objet.
    fn move_by_table_xy(&mut self) -> LastAdd {
        let o = self.obj();
        let b = self.bus();
        motion::move_by_table(b, o, AXIS_Y)
    }
    /// 0x1cee : X et Z d'après la table de vitesses de l'objet.
    fn move_by_table_xz(&mut self) -> LastAdd {
        let o = self.obj();
        let b = self.bus();
        motion::move_by_table(b, o, AXIS_Z)
    }

    /// 0x1a94 / 0x1b64 : choisit la table de vitesses `n` de `tables` (`motion::SPEEDS_A`, `motion::SPEEDS_B`) et
    /// prend sa vitesse courante.
    fn select_speed(&mut self, tables: &SpeedTables, n: u16) {
        let o = self.obj();
        let b = self.bus();
        motion::select_speed(b, o, tables, n);
    }

    /// 0x29ea : l'objet est suspendu (octet `Obj::HIDDEN`).
    fn suspended(&mut self) -> bool {
        let o = self.obj();
        let b = self.bus();
        motion::suspended(b, o)
    }

    /// 0xf1bfc : compteurs de fin de gestionnaire.
    fn tick_counters(&mut self) {
        let o = self.obj();
        let b = self.bus();
        motion::tick_counters(b, o);
    }

    // --- listes d'affichage (`lists`) -------------------------------------------------------------

    /// 0x1ee1a : remet l'objet à sa place dans la liste des acteurs (fin ordinaire d'un gestionnaire).
    /// Adaptateur : les registres que laisse la réinsertion (D0-D2, A1-A4) passent au gestionnaire suivant de la boucle
    /// de dispatch, qui les relit parfois (glitch d'origine : `joueur/peff_e.rs` tire son rebond dans le D2 hérité ;
    /// `DDSOM_POISON=lists` le montre sur `scorpion_scene36`, `mash_c5`).
    #[track_caller]
    fn reinsert(&mut self) {
        let o = self.obj();
        let (c, b) = self.machine();
        ACTORS.reinsert_68k(c, b, o);
    }

    /// 0x1ef6c : remet l'objet à sa place dans la liste du premier plan (adaptateur, comme `reinsert`).
    #[track_caller]
    fn reinsert_foreground(&mut self) {
        let o = self.obj();
        let (c, b) = self.machine();
        FOREGROUND.reinsert_68k(c, b, o);
    }

    /// 0x1f0be : retire l'objet de sa liste. Cœur (l'original ne laisse aucun registre utile).
    fn unlink(&mut self) {
        let o = self.obj();
        lists::unlink(self.bus(), o)
    }

    // --- écran (`cull`) ----------------------------------------------------------------------------

    /// 0x1e1a : un objet actif sorti de l'écran passe à l'état 6.
    fn cull_offscreen(&mut self) {
        let o = self.obj();
        let b = self.bus();
        cull::despawn_if_offscreen(b, o);
    }

    /// 0x1e92 : l'objet est dans la zone de jeu élargie de `margin`.
    fn in_arena(&mut self, margin: u16) -> bool {
        let o = self.obj();
        let b = self.bus();
        cull::in_arena(b, o, margin)
    }

    /// 0x1e90 : `in_arena` avec la marge par défaut (`cull::ARENA_MARGIN`).
    fn in_arena_default(&mut self) -> bool {
        self.in_arena(cull::ARENA_MARGIN)
    }

    /// 0x1ed2 : visibilité de l'image et inscription aux collisions ; vrai si l'objet est visible.
    fn update_visibility(&mut self) -> bool {
        let o = self.obj();
        let b = self.bus();
        cull::update_visibility(b, o)
    }
}

impl Courant for crate::families::cx::Cx<'_> {
    fn obj(&self) -> Obj {
        self.o
    }
}

impl Courant for crate::families::Ctx<'_> {
    /// L'objet désigné par A0 au moment de l'appel (un gestionnaire peut le changer).
    fn obj(&self) -> Obj {
        Obj(self.c.a[0])
    }
}
