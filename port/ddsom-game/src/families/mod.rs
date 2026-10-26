//! Familles d'objets ITEM (ramassables), BOX (coffres) et EFFCT (effets) : un gestionnaire par type.
//!
//! La boucle de dispatch d'une classe (`0x39f34` pour ITEM, `0x39e50` BOX, `0x39cba` EFFCT) appelle,
//! pour chaque objet actif, le gestionnaire du type de l'objet (`jsr (table + 4 × type)`) avec
//! `A0` = objet et `D7` = compteur de boucle. Chaque gestionnaire est une petite machine à états
//! (octets `+4`, `+5`, `+6` de l'objet) ; il rend la main par `rts` en laissant `A0` et `D7` intacts.
//!
//! Une routine native remplace un gestionnaire entier : la logique est écrite une seule fois sur un
//! [`Ctx`] ; elle sert à la fois à calculer la durée exacte de l'original (exécution à blanc sur une
//! copie de la machine, en notant chaque branchement conditionnel de l'original) et à l'exécution
//! réelle. Les sous-routines non réécrites (sons, génération d'objets...) sont appelées par
//! [`Ctx::jsr`] ; les états d'un type qu'aucun scénario n'exerce restent en code d'origine
//! ([`Ctx::deleguer`]).
//!
//! Voir `docs/familles.md` pour la description de chaque type.

pub mod boss_common;
pub mod boss_17;
pub mod boss_6;
pub mod boss_10;
pub mod boss_5;
pub mod boss_21;
pub mod boss_19;
pub mod boss_15;
pub mod boss_18;
pub mod boss_4;
pub mod boss_7;
pub mod boss_8;
pub mod boss_11;
pub mod boss_dragon;
pub mod boss_1;
pub mod boss_3;
pub mod boss_9;
pub mod boxes;
pub mod creators;
pub mod petites;
pub mod cx;
pub mod effects;
pub mod effects_b;
pub mod effects_c;
pub mod effects_d;
pub mod effects_e;
pub mod effects_creators;
pub mod effects_f;
pub mod effects_g;
pub mod items_b;
pub mod eshl_creators;
pub mod enemy;
pub mod enemy_beetle;
pub mod enemy_common;
pub mod enemy_dbeast;
pub mod enemy_double;
pub mod enemy_gargoyle;
pub mod enemy_kobold;
pub mod enemy_goblin;
pub mod enemy_troll;
pub mod enemy_gnoll;
pub mod enemy_bow;
pub mod enemy_golem;
pub mod enemy_warmachine;
pub mod enemy_lib;
pub mod enemy_ogre;
pub mod enemy_elf;
pub mod enemy_ghoul;
pub mod enemy_harpy;
pub mod enemy_hellhound;
pub mod enemy_owlbear;
pub mod enemy_part;
pub mod reactions;
pub mod enemy_warrior;
pub mod enemy_riyakaa;
pub mod enemy_scorpion;
pub mod enemy_trog;
pub mod enemy_skeleton;
pub mod eshl;
pub mod eshl_b;
pub mod eshl_c;
pub mod eshl_d;
pub mod items;
pub mod joueur;
pub mod set;
pub mod set_b;
pub mod set_residu;
pub mod set_c;
pub mod shell;
pub mod sobj;

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::chemin::Chemin;
use crate::mem::{rd8, rts, A5};
use crate::objects::{anim, lists, pools, Obj};
use crate::Native;

/// Toutes les routines natives des familles.
pub const NATIVES: &[Native] = &[
    items::ITEM_0,
    items::ITEM_1,
    items::ITEM_3,
    items::ITEM_4,
    items::ITEM_5,
    items::ITEM_6,
    items::ITEM_7,
    items::ITEM_8,
    items::ITEM_9,
    items::EFFACE_RAMASSES,
    items::CHUTE_EQUIPEMENT,
    boxes::BOX_1,
    boxes::BOX_2,
    effects::EFFCT_1,
    effects::EFFCT_4,
    effects::EFFCT_5,
    effects::EFFCT_6,
    effects::EFFCT_7,
    effects::EFFCT_8,
    effects::EFFCT_9,
    effects::EFFCT_15,
    effects::EFFCT_17,
    effects::EFFCT_19,
    effects::EFFCT_21,
    effects::EFFCT_24,
    effects::EFFCT_25,
    effects::EFFCT_26,
    effects::EFFCT_27,
    effects::EFFCT_28,
    effects::EFFCT_29,
    effects::EFFCT_32,
    effects::EFFCT_33,
    effects::EFFCT_45,
    effects::EFFCT_51,
    effects::EFFCT_52,
    effects::EFFCT_56,
    effects::EFFCT_57,
    effects::EFFCT_62,
    effects::BOITE_TOUCHEE,
    effects::JOUEUR_TOUCHE,
    effects::FILS_FINIS,
];

/// Routines du jeu appelées par les gestionnaires (adresses d'origine).
pub mod rom {
    pub const ANIM_STEP: u32 = 0x1a56;
    pub const REINSERT_ACTORS: u32 = 0x1ee1a;
    pub const REINSERT_FOREGROUND: u32 = 0x1ef6c;
    pub const MOVE_X: u32 = 0x1d78;
    pub const MOVE_XYZ: u32 = 0x1d90;
    pub const MOVE_XZ: u32 = 0x1da6;
    pub const CULL_OFFSCREEN: u32 = 0x1e1a;
    pub const DANS_LA_ZONE: u32 = 0xfbae;
    pub const OMBRE: u32 = 0x33fe4;
    pub const RNG: u32 = 0x19a88;
}

/// Couverture des décisions (fichier, ligne, résultat) observées pendant les exécutions réelles, quand
/// elle est activée (outils de validation : voir `examples/familles.rs`).
static COUVERTURE_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static COUVERTURE: std::sync::Mutex<std::collections::BTreeSet<(&'static str, u32, bool)>> =
    std::sync::Mutex::new(std::collections::BTreeSet::new());

/// Affiche, pour chaque calcul de durée, les tronçons du code d'origine parcourus (outil de diagnostic).
static DIAGNOSTIC: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn activer_diagnostic(actif: bool) {
    DIAGNOSTIC.store(actif, std::sync::atomic::Ordering::Relaxed);
}

pub fn activer_couverture() {
    COUVERTURE_ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub fn couverture() -> Vec<(&'static str, u32, bool)> {
    COUVERTURE.lock().unwrap().iter().copied().collect()
}

/// Contexte d'exécution d'un gestionnaire : la machine, et en mode « à blanc » le relevé des
/// décisions de l'original.
pub struct Ctx<'a> {
    pub c: &'a mut Cpu,
    pub b: &'a mut Cps2Bus,
    rec: Option<Rec>,
    /// Le gestionnaire a déjà dépilé son adresse de retour (voir `epilogue_etranger`).
    retour_deja: bool,
}

/// Un tronçon de code d'origine parcouru, avec les décisions de ses branchements.
struct Troncon {
    depart: u32,
    arret: Option<u32>,
    chemin: Chemin,
    /// Nombre de décisions relevées.
    decisions: u32,
}

#[derive(Default)]
struct Rec {
    troncons: Vec<Troncon>,
    /// Appels dont la durée est ajoutée par l'exécution elle-même (cycles de `c`).
    externes: Vec<u32>,
    /// Instructions isolées (appels indirects) comptées à part.
    isolees: u64,
}

impl<'a> Ctx<'a> {
    /// Exécution réelle.
    pub fn reel(c: &'a mut Cpu, b: &'a mut Cps2Bus) -> Self {
        Ctx { c, b, rec: None, retour_deja: false }
    }

    /// Exécution à blanc (sur une copie) : relève le chemin suivi par l'original.
    fn a_blanc(c: &'a mut Cpu, b: &'a mut Cps2Bus) -> Self {
        Ctx { c, b, rec: Some(Rec::default()), retour_deja: false }
    }

    /// L'objet traité (registre A0).
    pub fn obj(&self) -> Obj {
        Obj(self.c.a[0])
    }

    /// Compteur de la boucle de dispatch (D7), qui sert de phase aux clignotements.
    pub fn loop_counter(&self) -> u16 {
        self.c.d[7] as u16
    }

    // ---- relevé du chemin de l'original ----

    /// Le code d'origine continue en `adresse` (début d'un tronçon).
    pub fn at(&mut self, adresse: u32) {
        if let Some(r) = &mut self.rec {
            r.troncons.push(Troncon { depart: adresse, arret: None, chemin: Chemin::default(), decisions: 0 });
        }
    }

    /// Décision d'un branchement conditionnel de l'original (vrai : branchement pris).
    #[track_caller]
    pub fn pris(&mut self, pris: bool) -> bool {
        if self.rec.is_none() && COUVERTURE_ACTIVE.load(std::sync::atomic::Ordering::Relaxed) {
            let l = std::panic::Location::caller();
            COUVERTURE.lock().unwrap().insert((l.file(), l.line(), pris));
        }
        if let Some(r) = &mut self.rec {
            let t = r.troncons.last_mut().expect("tronçon");
            t.chemin.pris(pris);
            t.decisions += 1;
        }
        pris
    }

    /// Saut par table `jmp (table,PC,D1.w)` : lit le mot de la table à l'indice `index` (octets),
    /// ouvre le tronçon de la cible et renvoie son adresse.
    pub fn saut_table(&mut self, table: u32, index: u32) -> u32 {
        let mot = self.prog16(table.wrapping_add(index)) as i16;
        let cible = table.wrapping_add(mot as i32 as u32);
        // les registres que l'original laisse à la cible (D0 = indice, D1.w = mot) servent aux états
        // qui s'exécutent en code d'origine (`deleguer`)
        self.c.d[0] = index;
        self.c.d[1] = self.c.d[1] & 0xffff_0000 | mot as u16 as u32;
        self.at(cible);
        cible
    }

    /// Variante de `saut_table` pour les sauts qui passent par D1 seul (`moveq 0,D1 ... move.w (t,PC,D1.w),D1.w`).
    pub fn saut_table_d1(&mut self, table: u32, index: u32) -> u32 {
        let mot = self.prog16(table.wrapping_add(index)) as i16;
        let cible = table.wrapping_add(mot as i32 as u32);
        self.c.d[1] = mot as u16 as u32;
        self.at(cible);
        cible
    }

    /// Saut de l'original dans l'épilogue d'une autre routine (`movem.l (SP)+,{A1-A4} ; rts`) : les
    /// quatre registres sont rechargés depuis la pile, puis le retour se fait à l'adresse suivante.
    /// Comportement hors contrat de l'original : la routine native le signale par un coût démesuré
    /// (`COUT_HORS_CONTRAT`), ce qui fait exécuter le code d'origine.
    pub fn epilogue_etranger(&mut self) {
        let sp = self.c.a[7];
        for (i, r) in [1usize, 2, 3, 4].into_iter().enumerate() {
            self.c.a[r] = crate::mem::rd32(self.b, sp.wrapping_add(4 * i as u32));
        }
        self.c.a[7] = sp.wrapping_add(16);
        self.c.pc = crate::mem::rd32(self.b, self.c.a[7]);
        self.c.a[7] = self.c.a[7].wrapping_add(4);
        self.retour_deja = true;
    }

    /// Exécute le code d'origine à partir de `adresse` (jusqu'à son `rts`) : états non réécrits.
    pub fn deleguer(&mut self, adresse: u32) {
        if let Some(r) = &mut self.rec {
            // le tronçon ouvert pour cette cible ne doit pas être compté : l'exécution s'en charge
            if r.troncons.last().is_some_and(|t| t.depart == adresse && t.decisions == 0) {
                r.troncons.pop();
            }
        }
        // un état qui n'est qu'un `jmp abs.l` (la libération, le plus souvent) : appel direct de sa cible
        const JMP_ABS_L: u16 = 0x4ef9;
        let adresse = if self.prog16(adresse) == JMP_ABS_L { self.prog32(adresse + 2) } else { adresse };
        crate::call(self.c, self.b, adresse);
    }

    /// État réécrit sans relevé de chemin, à la place de l'état d'origine `adresse` ouvert par
    /// `saut_table` : comme `deleguer`, mais `corps` (Rust) s'exécute au lieu du code d'origine. Sa durée
    /// est celle de ses sous-routines ; ses `pris` ne relèvent rien.
    pub fn sans_chemin(&mut self, adresse: u32, corps: impl FnOnce(&mut Ctx)) {
        if let Some(r) = &mut self.rec {
            if r.troncons.last().is_some_and(|t| t.depart == adresse && t.decisions == 0) {
                r.troncons.pop();
            }
        }
        corps(&mut Ctx::reel(&mut *self.c, &mut *self.b));
    }

    // ---- appels ----

    /// `jsr adresse` d'une routine non réécrite (ou déjà native), registres tels que préparés.
    pub fn jsr(&mut self, adresse: u32) {
        if let Some(r) = &mut self.rec {
            if !r.externes.contains(&adresse) {
                r.externes.push(adresse);
            }
        }
        crate::call(self.c, self.b, adresse);
    }

    /// Appel direct d'une routine native du modèle d'objets (sans passer par l'index des adresses).
    pub fn natif(&mut self, n: &Native) {
        if let Some(r) = &mut self.rec {
            if !r.externes.contains(&n.entry) {
                r.externes.push(n.entry);
            }
        }
        let cout = (n.cost)(self.c, self.b);
        self.c.a[7] = self.c.a[7].wrapping_sub(4);
        crate::mem::wr32(self.b, self.c.a[7], 0xfffe);
        (n.run)(self.c, self.b);
        self.c.cycles += cout;
    }

    /// `jsr adresse` exécuté par le code d'origine seul (aucune routine native, même imbriquée) : les
    /// registres que l'original laisse derrière lui (restes de calcul) sont alors exactement les siens.
    pub fn jsr_origine(&mut self, adresse: u32) {
        if let Some(r) = &mut self.rec {
            if !r.externes.contains(&adresse) {
                r.externes.push(adresse);
            }
        }
        const RETOUR: u32 = 0x00_fffe;
        let sp = self.c.a[7];
        self.c.a[7] = sp.wrapping_sub(4);
        crate::mem::wr32(self.b, self.c.a[7], RETOUR);
        self.c.pc = adresse;
        while !(self.c.pc == RETOUR && self.c.a[7] == sp) {
            if ddsom_rc::run_block(self.c.pc, self.c, self.b, u64::MAX).is_none() {
                self.c.step(self.b);
            }
        }
    }

    /// `jsr (An)` à l'adresse `instruction` : l'instruction est comptée à part, le tronçon courant
    /// s'arrête là, `corps` exécute la routine appelée (qui ouvre ses propres tronçons avec `at`, ou
    /// se délègue au code d'origine), puis un autre tronçon reprend après l'instruction.
    pub fn jsr_indirect(&mut self, instruction: u32, corps: impl FnOnce(&mut Ctx)) {
        let mut suite = None;
        if self.rec.is_some() {
            let ins = m68k::decode(self.b, instruction);
            let cycles = m68k::timing::base_cycles(&ins) as u64;
            let r = self.rec.as_mut().unwrap();
            r.troncons.last_mut().expect("tronçon").arret = Some(instruction);
            r.isolees += cycles;
            suite = Some(instruction.wrapping_add(ins.len));
        }
        corps(self);
        if let Some(suite) = suite {
            self.at(suite);
        }
    }

    // ---- services du modèle d'objets ----

    /// `anim_start_indexed` : démarre le script `numero` du jeu d'animations `jeu`.
    pub fn anim_start(&mut self, jeu: u32, numero: u16) {
        self.c.a[4] = jeu;
        self.c.d[0] = numero as u32;
        self.natif(&anim::ANIM_START_INDEXED);
    }

    /// Pas d'animation ; vrai si le script est arrivé à sa fin.
    pub fn anim_step(&mut self) -> bool {
        self.natif(&anim::ANIM_STEP);
        self.c.cond(5) // C
    }

    /// Réinsère l'objet dans la liste triée des acteurs (tri par profondeur). Le handler suivant hérite
    /// des registres : la native les rend comme l'original (voir `objects::lists`).
    pub fn reinsert(&mut self) {
        self.natif(&lists::REINSERT_ACTORS);
    }

    /// Réinsère l'objet dans la liste triée du premier plan.
    pub fn reinsert_foreground(&mut self) {
        self.natif(&lists::REINSERT_FOREGROUND);
    }

    /// Libère l'emplacement de l'objet dans son pool.
    pub fn free(&mut self, pool: &Native) {
        self.natif(pool);
    }

    /// Alloue un objet dans le pool `pool` : renvoie l'emplacement, ou `None` si le pool est plein.
    pub fn alloc(&mut self, pool: &Native) -> Option<Obj> {
        self.natif(pool);
        (!self.c.cond(5)).then(|| Obj(self.c.a[1]))
    }

    /// Alloue un objet ITEM (voir `alloc`).
    pub fn alloc_item(&mut self) -> Option<Obj> {
        self.alloc(&pools::ALLOC_ITEM)
    }

    /// Condition « plus grand » (bgt) des drapeaux courants.
    pub fn gt(&self) -> bool {
        self.c.cond(14)
    }

    // ---- lectures de la ROM programme et de la RAM de travail ----

    /// Octet de la ROM programme lu comme une donnée (tables adressées par `lea (x,PC),An`).
    pub fn rom8(&mut self, a: u32) -> u8 {
        rd8(self.b, a)
    }

    pub fn rom16(&mut self, a: u32) -> u16 {
        crate::mem::rd16(self.b, a)
    }

    pub fn rom32(&mut self, a: u32) -> u32 {
        crate::mem::rd32(self.b, a)
    }

    pub fn prog8(&mut self, a: u32) -> u8 {
        let mot = m68k::Bus::read_prog16(self.b, a & 0xff_fffe);
        if a & 1 == 0 { (mot >> 8) as u8 } else { mot as u8 }
    }

    pub fn prog16(&mut self, a: u32) -> u16 {
        (self.prog8(a) as u16) << 8 | self.prog8(a.wrapping_add(1)) as u16
    }

    pub fn prog32(&mut self, a: u32) -> u32 {
        (self.prog16(a) as u32) << 16 | self.prog16(a.wrapping_add(2)) as u32
    }

    /// Octet de la RAM de travail à `A5 + decalage` (décalage signé étendu).
    pub fn ram8(&mut self, decalage: i32) -> u8 {
        rd8(self.b, A5.wrapping_add(decalage as u32))
    }
    pub fn ram16(&mut self, decalage: i32) -> u16 {
        crate::mem::rd16(self.b, A5.wrapping_add(decalage as u32))
    }
    pub fn set_ram8(&mut self, decalage: i32, v: u8) {
        crate::mem::wr8(self.b, A5.wrapping_add(decalage as u32), v)
    }

    /// Caméra de l'image : (x, y) écran, avec la caméra alternative si `A5+0x36` est non nul.
    pub fn camera(&mut self) -> (u16, u16) {
        let alt = self.ram8(0x36) != 0;
        self.pris(!alt); // beq : la caméra principale
        if alt {
            (self.ram16(0x556), self.ram16(0x55a))
        } else {
            (self.ram16(0x506), self.ram16(0x50a))
        }
    }

    /// Durée de l'exécution à blanc : le code d'origine parcouru + les cycles des sous-routines.
    fn cout(self, b_rom: &mut Cps2Bus, cycles_sous_routines: u64) -> u64 {
        let rec = self.rec.expect("exécution à blanc");
        if DIAGNOSTIC.load(std::sync::atomic::Ordering::Relaxed) {
            // diagnostic : décrit chaque tronçon avant le calcul (qui peut échouer)
            for t in &rec.troncons {
                eprintln!("   tronçon {:06x}..{:?} : {} décisions {:?}", t.depart, t.arret, t.decisions, t.chemin);
            }
        }
        let propres: u64 = rec
            .troncons
            .iter()
            .map(|t| t.chemin.duree_hors(b_rom, t.depart, t.arret, &rec.externes))
            .sum();
        if DIAGNOSTIC.load(std::sync::atomic::Ordering::Relaxed) {
            for t in &rec.troncons {
                let d = t.chemin.duree_hors(b_rom, t.depart, t.arret, &rec.externes);
                eprintln!("   tronçon {:06x}..{:?} : {} cycles, {} décisions", t.depart, t.arret, d, t.decisions);
            }
            eprintln!("   propres {propres}, isolées {}, sous-routines {cycles_sous_routines}, externes {:x?}", rec.isolees, rec.externes);
        }
        propres + rec.isolees + cycles_sous_routines
    }
}

/// `addq.b #n,(off,A0)` : ajoute `n` à un octet de l'objet (modulo 256).
pub(crate) fn ajoute_u8(ctx: &mut Ctx, o: Obj, off: u32, n: u8) {
    let v = o.u8(ctx.b, off).wrapping_add(n);
    o.set_u8(ctx.b, off, v);
}

/// Objet désigné par un pointeur RAM de 16 bits étendu en signe (`movea.w (off,A0),An`).
pub(crate) fn pointeur(ctx: &mut Ctx, o: Obj, off: u32) -> Obj {
    Obj(o.u16(ctx.b, off) as i16 as i32 as u32 & 0xff_ffff)
}

/// Durée exacte d'un gestionnaire pour l'état courant : exécution à blanc sur une copie.
pub fn cout(c: &Cpu, b: &mut Cps2Bus, entree: u32, logique: fn(&mut Ctx)) -> u64 {
    let mut c2 = c.clone();
    let mut b2 = b.clone();
    let debut = c2.cycles;
    let mut ctx = Ctx::a_blanc(&mut c2, &mut b2);
    ctx.at(entree);
    logique(&mut ctx);
    if ctx.retour_deja {
        // le gestionnaire quitte par un chemin hors contrat (voir `epilogue_etranger`) : la durée annoncée
        // dépasse toute échéance, donc l'exécuteur lance le code d'origine à sa place
        return COUT_HORS_CONTRAT;
    }
    let sous_routines = ctx.c.cycles - debut;
    ctx.cout(b, sous_routines)
}

/// Durée annoncée pour un appel que la routine native ne sait pas faire (l'original s'exécute alors).
pub const COUT_HORS_CONTRAT: u64 = u64::MAX / 4;

/// Exécute un gestionnaire (RTS compris). Les cycles des sous-routines sont comptés dans `cout`.
pub fn executer(c: &mut Cpu, b: &mut Cps2Bus, logique: fn(&mut Ctx)) {
    let debut = c.cycles;
    let retour_deja = {
        let mut ctx = Ctx::reel(c, b);
        logique(&mut ctx);
        ctx.retour_deja
    };
    c.cycles = debut;
    if !retour_deja {
        rts(c, b);
    }
}

/// Déclare la routine native d'un gestionnaire : adresse d'entrée, nom, logique.
macro_rules! gestionnaire {
    ($const:ident, $entree:expr, $nom:expr, $logique:path) => {
        pub const $const: crate::Native = crate::Native {
            entry: $entree,
            name: $nom,
            cost: |c, b| crate::families::cout(c, b, $entree, $logique),
            run: |c, b| crate::families::executer(c, b, $logique),
            touches_sound: false,
            outputs: crate::regs::a(0) | crate::regs::d(7),
            exact_cost: false,
        };
    };
}
pub(crate) use gestionnaire;

/// Routines natives des familles ESHL, SET et SOBJ (voir `docs/familles-eshl-set-sobj.md`) : un
/// gestionnaire par type, écrit sur [`cx::Cx`] (durée indicative, pas d'exécution à blanc).
pub fn natives() -> impl Iterator<Item = &'static Native> {
    [eshl::NATIVES, eshl_b::NATIVES, eshl_c::NATIVES, eshl_d::NATIVES, set::NATIVES, set_b::NATIVES, set_c::NATIVES, sobj::NATIVES, effects_b::NATIVES, effects_c::NATIVES, effects_d::NATIVES, effects_e::NATIVES, effects_f::NATIVES, enemy::NATIVES, enemy_double::NATIVES, enemy_part::NATIVES, enemy_common::NATIVES, enemy_riyakaa::NATIVES, enemy_dbeast::NATIVES, enemy_owlbear::NATIVES, enemy_harpy::NATIVES, enemy_warrior::NATIVES, enemy_skeleton::NATIVES, enemy_elf::NATIVES, enemy_trog::NATIVES, enemy_ghoul::NATIVES, enemy_beetle::NATIVES, enemy_scorpion::NATIVES, enemy_gargoyle::NATIVES, enemy_hellhound::NATIVES, enemy_ogre::NATIVES, enemy_kobold::NATIVES, enemy_goblin::NATIVES, enemy_troll::NATIVES, enemy_gnoll::NATIVES, enemy_bow::NATIVES, enemy_golem::NATIVES, enemy_warmachine::NATIVES, boss_dragon::NATIVES, boss_17::NATIVES, boss_6::NATIVES, boss_10::NATIVES, boss_5::NATIVES, boss_21::NATIVES, boss_19::NATIVES, boss_15::NATIVES, boss_15::HELPERS, boss_18::NATIVES, boss_4::NATIVES, boss_7::NATIVES, boss_8::NATIVES, boss_11::NATIVES, boss_1::NATIVES, boss_3::NATIVES, boss_9::NATIVES]
        .into_iter()
        .flatten()
        .chain(items_b::NATIVES)
        .chain(eshl_creators::NATIVES)
        .chain(boxes::caisse::NATIVES)
        .chain(boxes::pousse::NATIVES)
        .chain(boss_common::NATIVES)
        .chain(creators::NATIVES)
        .chain([&set_b::SPAWN_BUMPERS, &set_b::DOOR_B_RESUME])
        .chain(set_residu::NATIVES)
        .chain(petites::NATIVES)
        .chain(petites::SHAKES)
        .chain(effects_creators::NATIVES)
        .chain(effects_g::NATIVES)
        .chain(effects_g::RESUMES)
        .chain(joueur::natives())
        .chain(reactions::NATIVES)
        .chain([&reactions::PLAYER_HURT_ANIMATION])
}
