//! Contexte d'exécution d'un gestionnaire d'objet (ESHL, SET, SOBJ).
//!
//! Un gestionnaire est appelé par la boucle de dispatch de sa classe (`jsr (table + type * 4)`)
//! avec A0 = l'objet, A5 = 0xFF8000 et D7 = compteur de la boucle ; il rend la main par RTS en
//! laissant A0 et D7 intacts. `Cx` regroupe le processeur, le bus et l'objet courant, et offre
//! les appels de sous-routines du moteur d'objets sous des noms lisibles.
//!
//! Les sous-routines non réécrites sont exécutées par `crate::call` (durée comprise) ; celles qui
//! le sont déjà (animation, déplacements, listes triées, sons...) sont des routines natives que
//! `call` retrouve par leur adresse : leur durée exacte s'ajoute donc à celle du gestionnaire.

use cps2::Cps2Bus;
use m68k::decode::Fetch;
use m68k::{Cpu, SR_C};

use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8, A5};
use crate::objects::Obj;
use crate::{call, regs, Native};

/// Contexte d'un gestionnaire : l'objet courant (A0) et la machine.
pub struct Cx<'a> {
    pub c: &'a mut Cpu,
    pub b: &'a mut Cps2Bus,
    /// L'objet que le dispatch fait traiter (A0).
    pub o: Obj,
}

/// Sous-routines du moteur d'objets, appelées par adresse.
pub mod sub {
    /// Charge l'entrée d'animation `D0` du jeu `A4` (`anim_start_indexed`).
    pub const ANIM_START: u32 = 0x1a32;
    /// Un pas d'animation ; retenue = fin du script.
    pub const ANIM_STEP: u32 = 0x1a56;
    /// Choisit la table de vitesses `D0` (variante A : 0x1a94, variante B : 0x1b64).
    pub const SPEED_TABLE_B: u32 = 0x1b64;
    pub const SPEED_TABLE_A: u32 = 0x1a94;
    /// Déplacement selon la table de vitesses (X et Y : 0x1cb4, X et Z : 0x1cee).
    pub const MOVE_TABLE_XY: u32 = 0x1cb4;
    pub const MOVE_TABLE_XZ: u32 = 0x1cee;
    /// Déplacements balistiques : Y puis X, X, Y X Z, X Z, Z, Y Z, Y.
    pub const MOVE_XY: u32 = 0x1d62;
    pub const MOVE_X: u32 = 0x1d78;
    pub const MOVE_XYZ: u32 = 0x1d90;
    pub const MOVE_XZ: u32 = 0x1da6;
    pub const MOVE_Z: u32 = 0x1dbc;
    pub const MOVE_YZ: u32 = 0x1dd4;
    pub const MOVE_Y: u32 = 0x1e02;
    /// Retire l'objet (état 6) s'il est sorti de l'écran.
    pub const CULL_OFFSCREEN: u32 = 0x1e1a;
    /// Remet l'objet à sa place dans la liste triée des acteurs / du premier plan.
    pub const REINSERT_ACTORS: u32 = 0x1ee1a;
    pub const REINSERT_FOREGROUND: u32 = 0x1ef6c;
    /// Hook de pause : Z = objet non suspendu.
    pub const PAUSE_HOOK: u32 = 0x29ea;
    /// Attribut du terrain sous l'objet (D2) ; `terrain_at` prend (D0, D1) = (x, y).
    pub const TERRAIN_OF_OBJECT: u32 = 0x24a8;
    pub const TERRAIN_AT: u32 = 0x24b0;
    /// Libération de l'objet dans son pool.
    pub const FREE_ESHL: u32 = 0x2202;
    pub const FREE_SET: u32 = 0x2210;
    pub const FREE_SOBJ: u32 = 0x2250;
    /// Allocation d'un objet : A1 = emplacement, retenue = échec.
    pub const ALLOC_ESHL: u32 = 0x22ea;
    pub const ALLOC_SET: u32 = 0x2348;
    pub const ALLOC_SOBJ: u32 = 0x2408;
}

impl<'a> Cx<'a> {
    /// Contexte pour l'objet désigné par A0.
    pub fn new(c: &'a mut Cpu, b: &'a mut Cps2Bus) -> Self {
        let o = Obj(c.a[0]);
        Cx { c, b, o }
    }

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

    // --- sous-routines -------------------------------------------------------------------------

    /// JSR `addr` avec A0 = l'objet et A5 = base de la RAM de travail.
    pub fn call(&mut self, addr: u32) {
        self.c.a[0] = self.o.0;
        self.c.a[5] = A5;
        call(self.c, self.b, addr);
    }

    /// `lea set,A4 ; moveq #n,D0 ; jsr 0x1a32` : démarre le script `n` du jeu d'animations `set`.
    pub fn anim_start(&mut self, set: u32, n: u32) {
        self.c.a[4] = set;
        self.c.d[0] = n;
        self.call(sub::ANIM_START);
    }

    /// Pas d'animation ; vrai quand le script est terminé (retenue levée).
    pub fn anim_step(&mut self) -> bool {
        self.call(sub::ANIM_STEP);
        self.c.sr & SR_C != 0
    }

    /// Fin du gestionnaire à l'ordinaire : remise en place dans la liste des acteurs.
    pub fn reinsert(&mut self) {
        self.call(sub::REINSERT_ACTORS);
    }

    /// Fin du gestionnaire : l'objet est libéré (tir détruit).
    pub fn free_eshl(&mut self) {
        self.call(sub::FREE_ESHL);
    }

    /// RTS.
    pub fn ret(self) {
        rts(self.c, self.b);
    }
}

/// Native d'un gestionnaire : contrat de la boucle de dispatch (A0 et D7 conservés).
pub const fn handler(entry: u32, name: &'static str, cost: fn(&Cpu, &mut Cps2Bus) -> u64, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, cost, run, touches_sound: false, outputs: regs::a(0) | regs::d(7), exact_cost: false }
}

/// Déclare la table `NATIVES` d'un fichier de types : `adresse "nom" coût chemin::du::gestionnaire`.
/// Le coût est une estimation de la durée propre du gestionnaire en cycles (hors sous-routines).
macro_rules! handlers {
    ($( $entry:literal $name:literal $cost:literal $f:path ),* $(,)?) => {
        pub const NATIVES: &[$crate::Native] = &[ $(
            $crate::families::cx::handler($entry, $name, |_, _| $cost, |c, b| {
                let mut x = $crate::families::cx::Cx::new(c, b);
                $f(&mut x);
                x.ret();
            }),
        )* ];
    };
}
pub(crate) use handlers;
