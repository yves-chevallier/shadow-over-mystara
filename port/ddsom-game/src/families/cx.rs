//! Contexte d'exécution d'un gestionnaire d'objet (ESHL, SET, SOBJ).
//!
//! Un gestionnaire est appelé par la boucle de dispatch de sa classe (`jsr (table + type * 4)`)
//! avec A0 = l'objet, A5 = 0xFF8000 et D7 = compteur de la boucle ; il rend la main par RTS en
//! laissant A0 et D7 intacts. `Cx` regroupe le processeur, le bus et l'objet courant. Le moteur d'objets
//! (animation, mouvement, listes, écran, pools) s'appelle par le trait `objects::Courant` (`x.move_xz()`,
//! `x.anim_step()`, `x.reinsert()`...), les services communs par `machine::Machine` (`x.random()`).
//!
//! Les sous-routines encore appelées par adresse (`call`) sont exécutées par `crate::call`, qui retrouve leur
//! native.

use cps2::Cps2Bus;
use m68k::decode::Fetch;
use m68k::Cpu;

use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8, A5};
use crate::dispatch::{Aiguillage, Restes};
use crate::objects::pools;
use crate::objects::{Courant, Obj};
use crate::{call, regs, Native};

/// Contexte d'un gestionnaire : l'objet courant (A0) et la machine.
///
/// `R` est le banc de registres : le processeur (`&mut Cpu`, par défaut) tant que le gestionnaire lit ou laisse des
/// registres de l'original, ou [`SansRegistres`] pour un gestionnaire converti, dont rien n'entre ni ne sort par les
/// registres (le compilateur le garantit : `x.c.d[0]` n'existe pas). Les fonctions qui n'utilisent pas les registres
/// s'écrivent pour les deux (`fn f<R>(x: &mut Cx<'_, R>)`).
pub struct Cx<'a, R = &'a mut Cpu> {
    pub c: R,
    pub b: &'a mut Cps2Bus,
    /// L'objet que le dispatch fait traiter (A0).
    pub o: Obj,
}

/// Contexte d'un gestionnaire sans registres : ce que la boucle d'aiguillage lui passe (`dispatch::Aiguillage`).
pub type Gx<'a> = Cx<'a, &'a mut Aiguillage>;

impl<'a> Gx<'a> {
    /// Contexte du gestionnaire appelé par la boucle d'aiguillage.
    pub fn aiguille(b: &'a mut Cps2Bus, a: &'a mut Aiguillage) -> Self {
        let o = a.objet;
        Cx { c: a, b, o }
    }

    /// Restes de registres que les gestionnaires se transmettent (glitches d'origine).
    pub fn restes(&mut self) -> &mut Restes {
        &mut self.c.restes
    }
}

impl<R> crate::machine::Machine for Cx<'_, R> {
    fn bus(&mut self) -> &mut Cps2Bus {
        &mut *self.b
    }
}

impl crate::machine::Processeur for Cx<'_> {
    fn machine(&mut self) -> (&mut Cpu, &mut Cps2Bus) {
        (&mut *self.c, &mut *self.b)
    }
}

/// Banc de registres d'un gestionnaire : ce qui change selon qu'il travaille encore sur le processeur ou non.
pub trait Banc {
    /// Remise en liste (0x1ee1a / 0x1ef6c, `premier_plan`) : sur le processeur, l'adaptateur laisse les registres de
    /// l'original au gestionnaire suivant ; sans registres, le cœur.
    fn remettre_en_liste(&mut self, b: &mut Cps2Bus, o: Obj, premier_plan: bool);
    /// Les restes D2 à D5 (voir `dispatch::Restes`) : le registre du processeur, ou le reste de la boucle
    /// d'aiguillage.
    fn d2(&mut self) -> &mut u32;
    fn d3(&mut self) -> &mut u32;
    fn d4(&mut self) -> &mut u32;
    fn d5(&mut self) -> &mut u32;
    /// Compteur de la boucle d'aiguillage (D7.w de l'original) : phase des clignotements « une image sur deux ».
    fn compteur(&self) -> u16;
}

impl Banc for &mut Cpu {
    #[track_caller]
    fn remettre_en_liste(&mut self, b: &mut Cps2Bus, o: Obj, premier_plan: bool) {
        let liste = if premier_plan { crate::objects::lists::FOREGROUND } else { crate::objects::lists::ACTORS };
        liste.reinsert_68k(self, b, o);
    }
    fn d2(&mut self) -> &mut u32 {
        &mut self.d[2]
    }
    fn d3(&mut self) -> &mut u32 {
        &mut self.d[3]
    }
    fn d4(&mut self) -> &mut u32 {
        &mut self.d[4]
    }
    fn d5(&mut self) -> &mut u32 {
        &mut self.d[5]
    }
    fn compteur(&self) -> u16 {
        self.d[7] as u16
    }
}

impl Banc for &mut Aiguillage {
    fn remettre_en_liste(&mut self, b: &mut Cps2Bus, o: Obj, premier_plan: bool) {
        let liste = if premier_plan { crate::objects::lists::FOREGROUND } else { crate::objects::lists::ACTORS };
        liste.reinsert(b, o);
    }
    fn d2(&mut self) -> &mut u32 {
        &mut self.restes.d2
    }
    fn d3(&mut self) -> &mut u32 {
        &mut self.restes.d3
    }
    fn d4(&mut self) -> &mut u32 {
        &mut self.restes.d4
    }
    fn d5(&mut self) -> &mut u32 {
        &mut self.restes.d5
    }
    fn compteur(&self) -> u16 {
        self.compteur
    }
}

impl<R: Banc> Cx<'_, R> {
    /// Compteur de la boucle d'aiguillage (D7.w de l'original).
    pub fn compteur(&self) -> u16 {
        self.c.compteur()
    }

    /// 0x1ee1a : remet l'objet à sa place dans la liste des acteurs (fin ordinaire d'un gestionnaire ; voir [`Banc`]).
    #[track_caller]
    pub fn reinsert(&mut self) {
        self.c.remettre_en_liste(self.b, self.o, false);
    }

    /// 0x1ef6c : remet l'objet à sa place dans la liste du premier plan (voir [`Banc`]).
    #[track_caller]
    pub fn reinsert_foreground(&mut self) {
        self.c.remettre_en_liste(self.b, self.o, true);
    }

    /// 0xfbb0 : garde l'objet dans l'écran à `marge` pixels près ; vrai s'il a été déplacé.
    /// Glitch d'origine : le résultat reste dans D5 (0 ou 1), que relit la rangée d'éclats d'une caisse (`Restes::d5`).
    pub fn keep_on_screen(&mut self, marge: u16) -> bool {
        let moved = crate::ai::keep_on_screen(self.b, self.o, marge);
        *self.c.d5() = moved as u32;
        moved
    }

    /// 0xfbae : `keep_on_screen` avec la marge de la zone de jeu (0x30).
    pub fn keep_in_zone(&mut self) -> bool {
        self.keep_on_screen(crate::ai::ZONE_MARGIN)
    }

    /// 0x33fe4 : collision avec le décor (`terrain::push`) ; rend le drapeau Z de l'original.
    /// Glitch d'origine : la recherche du terrain laisse D2 et D3, et un choc D5 = 1 (`Restes`).
    pub fn push_terrain(&mut self) -> bool {
        let (mut d2, mut d3, mut d5) = (*self.c.d2(), *self.c.d3(), *self.c.d5());
        let z = crate::terrain::push_restes(self.b, self.o, &mut d2, &mut d3, &mut d5);
        (*self.c.d2(), *self.c.d3(), *self.c.d5()) = (d2, d3, d5);
        z
    }
}

/// Sous-routines appelées par adresse (le moteur d'objets, lui, s'appelle par `objects::Courant`).
pub mod sub {
    /// Attribut du terrain sous l'objet (D2) ; `terrain_at` prend (D0, D1) = (x, y).
    pub const TERRAIN_OF_OBJECT: u32 = 0x24a8;
    pub const TERRAIN_AT: u32 = 0x24b0;
}

impl<'a> Cx<'a> {
    /// Contexte pour l'objet désigné par A0.
    pub fn new(c: &'a mut Cpu, b: &'a mut Cps2Bus) -> Self {
        let o = Obj(c.a[0]);
        Cx { c, b, o }
    }

    /// Appelle la fonction convertie `f` sur le même objet, sans registres (les restes passent par le processeur).
    pub fn sans<T>(&mut self, f: impl FnOnce(&mut Gx) -> T) -> T {
        let mut a = Aiguillage::du_processeur(self.c);
        a.objet = self.o;
        let r = f(&mut Cx { c: &mut a, b: &mut *self.b, o: self.o });
        a.rendre(self.c);
        r
    }

    // --- sous-routines -------------------------------------------------------------------------

    /// JSR `addr` avec A0 = l'objet et A5 = base de la RAM de travail.
    #[track_caller]
    pub fn call(&mut self, addr: u32) {
        self.c.a[0] = self.o.0;
        self.c.a[5] = A5;
        call(self.c, self.b, addr);
    }

    /// RTS.
    pub fn ret(self) {
        rts(self.c, self.b);
    }
}

impl<R> Cx<'_, R> {
    // --- champs de l'objet courant -----------------------------------------------------------

    pub fn u8(&mut self, off: u32) -> u8 {
        rd8(self.b, self.o.0 + off)
    }
    pub fn u16(&mut self, off: u32) -> u16 {
        rd16(self.b, self.o.0 + off)
    }
    pub fn u32(&mut self, off: u32) -> u32 {
        rd32(self.b, self.o.0 + off)
    }
    pub fn set8(&mut self, off: u32, v: u8) {
        wr8(self.b, self.o.0 + off, v)
    }
    pub fn set16(&mut self, off: u32, v: u16) {
        wr16(self.b, self.o.0 + off, v)
    }
    pub fn set32(&mut self, off: u32, v: u32) {
        wr32(self.b, self.o.0 + off, v)
    }
    /// `add.w #v,(off,A0)` : addition modulo 2^16 sur place.
    pub fn add16(&mut self, off: u32, v: u16) {
        let x = self.u16(off).wrapping_add(v);
        self.set16(off, x)
    }
    pub fn add8(&mut self, off: u32, v: u8) {
        let x = self.u8(off).wrapping_add(v);
        self.set8(off, x)
    }
    pub fn add32(&mut self, off: u32, v: u32) {
        let x = self.u32(off).wrapping_add(v);
        self.set32(off, x)
    }
    /// Remplace un bit d'un octet du record (`bset` / `bclr`).
    pub fn set_bit(&mut self, off: u32, bit: u8, on: bool) {
        let v = self.u8(off);
        self.set8(off, if on { v | 1 << bit } else { v & !(1 << bit) });
    }
    pub fn bit(&mut self, off: u32, bit: u8) -> bool {
        self.u8(off) >> bit & 1 != 0
    }

    // --- autres objets et variables du jeu --------------------------------------------------

    /// Objet pointé par un champ de 16 bits (`movea.w (off,A0),A1` : adresse étendue en signe).
    pub fn linked(&mut self, off: u32) -> Obj {
        Obj(self.u16(off) as i16 as i32 as u32 & 0xff_ffff)
    }
    pub fn a5_u8(&mut self, off: u32) -> u8 {
        rd8(self.b, A5 + off)
    }
    pub fn a5_u16(&mut self, off: u32) -> u16 {
        rd16(self.b, A5 + off)
    }
    pub fn a5_u32(&mut self, off: u32) -> u32 {
        rd32(self.b, A5 + off)
    }
    pub fn set_a5_u8(&mut self, off: u32, v: u8) {
        wr8(self.b, A5 + off, v)
    }
    pub fn set_a5_u16(&mut self, off: u32, v: u16) {
        wr16(self.b, A5 + off, v)
    }
    pub fn set_a5_u32(&mut self, off: u32, v: u32) {
        wr32(self.b, A5 + off, v)
    }
    /// Mot et long lus en espace de données (tables de la ROM adressées en absolu).
    pub fn data16(&mut self, addr: u32) -> u16 {
        rd16(self.b, addr)
    }
    pub fn data32(&mut self, addr: u32) -> u32 {
        rd32(self.b, addr)
    }
    /// Mot lu dans l'espace programme (table adressée relativement au PC).
    pub fn rom16(&mut self, addr: u32) -> u16 {
        self.b.fetch16(addr)
    }
    pub fn rom8(&mut self, addr: u32) -> u8 {
        let w = self.b.fetch16(addr & !1);
        if addr & 1 == 0 { (w >> 8) as u8 } else { w as u8 }
    }
    pub fn data8(&mut self, addr: u32) -> u8 {
        rd8(self.b, addr)
    }
    pub fn rom_i16(&mut self, addr: u32) -> i16 {
        self.b.fetch16(addr) as i16
    }

    /// Fin du gestionnaire : l'objet est libéré (tir détruit).
    pub fn free_eshl(&mut self)
    where
        Self: Courant,
    {
        self.free(&pools::ESHL);
    }
}

/// Native d'un gestionnaire : contrat de la boucle de dispatch (A0 et D7 conservés).
pub const fn handler(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, run: crate::Corps::Processeur(run), touches_sound: false, outputs: regs::a(0) | regs::d(7) }
}

/// Native d'un gestionnaire converti (`Gx`, sans registres) : ce que passe la boucle d'aiguillage en argument.
pub const fn handler_sans(entry: u32, name: &'static str, run: fn(&mut Cps2Bus, &mut Aiguillage)) -> Native {
    Native { entry, name, run: crate::Corps::Gestionnaire(run), touches_sound: false, outputs: regs::a(0) | regs::d(7) }
}

/// Déclare la table `NATIVES` d'un fichier de types : `adresse "nom" chemin::du::gestionnaire` (sur `Cx`), ou
/// `adresse "nom" #[sans] chemin::du::gestionnaire` (sur `Gx`, sans registres).
macro_rules! handlers {
    ($( $entry:literal $name:literal $(#[$sans:ident])? $f:path ),* $(,)?) => {
        pub const NATIVES: &[$crate::Native] = &[ $(
            $crate::families::cx::handlers!(@une $entry $name $(#[$sans])? $f),
        )* ];
    };
    (@une $entry:literal $name:literal $f:path) => {
        $crate::families::cx::handler($entry, $name, |c, b| {
            let mut x = $crate::families::cx::Cx::new(c, b);
            $f(&mut x);
            x.ret();
        })
    };
    (@une $entry:literal $name:literal #[sans] $f:path) => {
        $crate::families::cx::handler_sans($entry, $name, |b, a| {
            $f(&mut $crate::families::cx::Gx::aiguille(b, a));
        })
    };
}
pub(crate) use handlers;
