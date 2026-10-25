//! Objets de décor et d'animation (classe SET, pool de 32 objets de 0x50 octets, table 0x39eac),
//! types 16 à 32. Voir `docs/familles/set_b.md`.
//!
//! Les objets SET ne figurent pas dans les listes triées de profondeur par construction (les
//! appels à `reinsert` des créations les y inscrivent tout de même quand ils les rendent visibles).

use super::cx::{handlers, sub, Cx};
use crate::mem::{rd32, rd8, wr32, wr8, A5};
use crate::objects::Obj;

/// Les quatre enregistrements de joueurs : A5+0x5de, 0x200 octets chacun.
const PLAYERS: u32 = A5 + 0x5de;
const PLAYER_SIZE: u32 = 0x200;

/// Champs d'un enregistrement de joueur utilisés par les décors.
mod pl {
    pub const ACTIVE: u32 = 0x00;
    pub const STATE: u32 = 0x04;
    pub const X: u32 = 0x08;
    pub const Z: u32 = 0x0c;
    pub const Y: u32 = 0x10;
    pub const HEALTH: u32 = 0x62;
}

fn players() -> impl Iterator<Item = Obj> {
    (0..4).map(|i| Obj(PLAYERS + i * PLAYER_SIZE))
}

// ---------------------------------------------------------------------------------------------
// Type 16 (0xd4110) : tremplin (h)
// ---------------------------------------------------------------------------------------------

/// 0xd40e8, routine des scripts d'apparition (scènes 14 et 109) : libère le verrou des tremplins
/// (`A5+0x46`) et crée les treize tremplins de la scène, SET 16 de variantes 13 à 1 ; s'arrête si le
/// pool est plein. Laisse comme l'original D5 (compteur de boucle), A1 et D6 (de l'allocation).
pub const SPAWN_BUMPERS: crate::Native = crate::Native {
    entry: 0xd40e8,
    name: "set_tremplins_d40e8",
    cost: |_, _| 400,
    run: spawn_bumpers,
    touches_sound: false,
    outputs: crate::regs::d(5) | crate::regs::d(6) | crate::regs::a(1),
    exact_cost: false,
};

fn spawn_bumpers(c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus) {
    crate::mem::wr16(b, A5 + 0x46, 0);
    for counter in (0..13u32).rev() {
        c.d[5] = c.d[5] & !0xffff | counter;
        crate::call(c, b, sub::ALLOC_SET);
        if c.sr & m68k::SR_C != 0 {
            return crate::mem::rts(c, b);
        }
        let o = Obj(c.a[1]);
        o.set_u8(b, Obj::ACTIVE, 1);
        o.set_u8(b, Obj::KIND, 0x10);
        o.set_u8(b, Obj::VARIANT, counter as u8 + 1);
    }
    c.d[5] |= 0xffff;
    crate::mem::rts(c, b);
}

/// Tremplin / pare-chocs (h) : une zone de contact (boîtes lues dans la ROM d'après `+0x2e` et les
/// octets `+0x38..+0x3a`) qui, quand un joueur la touche en appuyant la bonne touche, le lance
/// dans les airs (vitesses en dur par type de tremplin `+0x42`, état du joueur 0x2002400 ou
/// 0x2002402). Un seul joueur à la fois : `A5+0x46` mémorise l'objet qui tient le verrou et
/// `+0x14` le délai avant de le relâcher. Tant qu'un joueur vivant est hors de portée des deux
/// boîtes de présence, l'objet ne fait rien.
mod bumper {
    use super::*;

    const ANIMS: u32 = 0x1c4738;
    /// Mot de la RAM (A5+0x46) : tremplin occupé (adresse de l'objet).
    const LOCK: u32 = 0x46;
    /// Octet de la RAM (A5+0x50) : jeu figé.
    const FROZEN: u32 = 0x50;
    const LOCK_FRAMES: u16 = 0x20;
    /// Variante -> sorte de tremplin (`+0x42`) ; variante -> position X, Y (deux mots).
    const KIND: u32 = 0xd4198;
    const POSITION: u32 = 0xd41a6;
    /// Tables de pointeurs de boîtes (par `+0x2e`), de 8 octets par boîte (dx, demi-largeur,
    /// dz, demi-hauteur ...) : boîte de contact, de présence 1 et de présence 2.
    const BOXES_HIT: u32 = 0x11bffa;
    const BOXES_NEAR: u32 = 0x11c11a;
    const BOXES_NEAR2: u32 = 0x11c23a;
    const BOX_SET: u8 = 0x42;
    /// Sous-champs de la boîte de contact (`+0x38`) et des boîtes de présence (`+0x39`, `+0x3a`).
    const HIT_SEL: u32 = 0x38;
    const NEAR_SEL: u32 = 0x39;
    const NEAR2_SEL: u32 = 0x3a;
    /// États du joueur lancé.
    const AIRBORNE: u32 = 0x0200_2400;
    const AIRBORNE_BACK: u32 = 0x0200_2402;

    /// Un lancer : touche du joueur (bit de `+0x6c`), vitesses, hauteur initiale, amortissement
    /// (`+0x1e`), état d'arrivée, et si l'orientation est remise à zéro.
    struct Launch {
        button: u8,
        vx: i16,
        vy: i16,
        z: u32,
        drag: i16,
        state: u32,
        reset_flip: bool,
    }

    const LAUNCHES: [Launch; 8] = [
        Launch { button: 0, vx: 0x300, vy: 0x400, z: 0x2_0000, drag: -0x20, state: AIRBORNE, reset_flip: false },
        Launch { button: 1, vx: -0x380, vy: 0x380, z: 0x2_0000, drag: -0x20, state: AIRBORNE, reset_flip: false },
        Launch { button: 2, vx: 0, vy: 0, z: 0x2_0000, drag: -0x30, state: AIRBORNE, reset_flip: false },
        // l'entrée 3 est spéciale (voir `kick_up`)
        Launch { button: 3, vx: 0, vy: 0, z: 0, drag: 0, state: 0, reset_flip: false },
        Launch { button: 0, vx: 0x480, vy: 0, z: 0x2_0000, drag: -0x30, state: AIRBORNE_BACK, reset_flip: false },
        Launch { button: 2, vx: -0x200, vy: 0, z: 0x3_0000, drag: -0x30, state: AIRBORNE_BACK, reset_flip: true },
        Launch { button: 0, vx: 0x300, vy: 0x500, z: 0x2_0000, drag: -0x20, state: AIRBORNE, reset_flip: false },
        Launch { button: 2, vx: -0x300, vy: 0, z: 0x2_4000, drag: -0x30, state: AIRBORNE_BACK, reset_flip: true },
    ];

    pub fn run(x: &mut Cx) {
        let lock = x.a5_u16(LOCK);
        if lock != 0 {
            if lock != x.o.0 as u16 {
                return;
            }
            let n = x.u16(0x14).wrapping_sub(1);
            x.set16(0x14, n);
            if (n as i16) < 0 {
                x.set_a5_u16(LOCK, 0);
            }
        }
        if x.u8(0x40) != 0 || x.a5_u8(FROZEN) != 0 {
            return;
        }
        match x.u8(0x4) {
            0 => init(x),
            2 => watch(x),
            _ => x.call(sub::FREE_SET),
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(0x2e, BOX_SET);
        let variant = x.u8(0x3) as u32;
        let kind = x.rom8(KIND + variant);
        x.set8(0x42, kind);
        let px = x.rom16(POSITION + 4 * variant);
        x.set16(0x8, px);
        let py = x.rom16(POSITION + 4 * variant + 2);
        x.set16(0x10, py);
        x.set16(0xc, 0);
        x.anim_start(ANIMS, variant);
        x.set32(0x34, 0);
        x.reinsert();
    }

    fn watch(x: &mut Cx) {
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) == 0 || p.u16(x.b, pl::HEALTH) == 0 {
                continue;
            }
            if !within(x, p, BOXES_NEAR, NEAR_SEL) && !within(x, p, BOXES_NEAR2, NEAR2_SEL) {
                return;
            }
        }
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) != 0 && touches(x, p) {
                launch(x, p);
            }
        }
    }

    /// Adresse de la boîte de la ligne `sel` du jeu `+0x2e` de la table `table`.
    fn box_of(x: &mut Cx, table: u32, sel: u32) -> u32 {
        let set = x.u8(0x2e) as u32;
        let boxes = x.data32(table + 4 * set);
        boxes.wrapping_add(8 * x.u8(sel) as u32)
    }

    /// Test d'un axe : la distance corrigée est dans l'intervalle (comparaison non signée).
    fn in_range(delta: u16, half: u8, shift: Option<i8>) -> bool {
        let mut d = delta.wrapping_add(half as u16);
        if let Some(s) = shift {
            d = d.wrapping_sub(s as i16 as u16);
        }
        d <= (half as u16).wrapping_mul(2)
    }

    /// Le joueur est dans une boîte de présence (boîte décalée de deux octets de décalage).
    fn within(x: &mut Cx, p: Obj, table: u32, sel: u32) -> bool {
        let b = box_of(x, table, sel);
        let dx = p.u16(x.b, pl::X).wrapping_sub(x.u16(0x8));
        let shift_x = x.data8(b) as i8;
        let half_x = x.data8(b + 1);
        if !in_range(dx, half_x, Some(shift_x)) {
            return false;
        }
        let dy = p.u16(x.b, pl::Y).wrapping_sub(x.u16(0x10));
        let shift_y = x.data8(b + 2) as i8;
        let half_y = x.data8(b + 3);
        in_range(dy, half_y, Some(shift_y))
    }

    /// Le joueur est dans la boîte de contact et dans un état qui le permet.
    fn touches(x: &mut Cx, p: Obj) -> bool {
        let b = box_of(x, BOXES_HIT, HIT_SEL);
        if p.u8(x.b, 5) != 0 || p.u8(x.b, 0x8a) != 0 {
            return false;
        }
        if p.u8(x.b, 6) == 0x22 || p.u16(x.b, pl::STATE) == 0x204 || p.u8(x.b, 6) == 0xe {
            return false;
        }
        if p.u16(x.b, pl::HEALTH) == 0 {
            return false;
        }
        if p.u32(x.b, pl::STATE) & 0xffff_ff00 == 0x0200_2000 {
            return false;
        }
        let dx = p.u16(x.b, pl::X).wrapping_sub(x.u16(0x8));
        if !in_range(dx, x.data8(b + 1), None) {
            return false;
        }
        let dy = p.u16(x.b, pl::Y).wrapping_sub(x.u16(0x10));
        in_range(dy, x.data8(b + 3), None)
    }

    /// Lance le joueur selon la sorte du tremplin (`+0x42`).
    fn launch(x: &mut Cx, p: Obj) {
        let kind = x.u8(0x42) as usize;
        if kind >= LAUNCHES.len() {
            return;
        }
        if kind == 3 {
            return kick_up(x, p);
        }
        let l = &LAUNCHES[kind];
        if p.u8(x.b, 0x6e) & 0x20 == 0 || (p.u16(x.b, pl::Z) as i16) > 0 || p.u8(x.b, 0x6c) >> l.button & 1 == 0 {
            return;
        }
        grab(x);
        p.set_u8(x.b, 0x66, 0);
        p.set_u32(x.b, pl::Z, l.z);
        p.set_u16(x.b, 0x16, 0);
        p.set_u16(x.b, 0x18, 0);
        p.set_u16(x.b, 0x14, l.vx as u16);
        p.set_u16(x.b, 0x1c, l.vy as u16);
        p.set_u16(x.b, 0x1a, 0xffff);
        p.set_u16(x.b, 0x1e, l.drag as u16);
        if l.reset_flip {
            p.set_u8(x.b, 0x2c, 0);
        }
        p.set_u32(x.b, pl::STATE, l.state);
    }

    /// Prend le verrou et arme le délai de relâchement.
    fn grab(x: &mut Cx) {
        x.set_a5_u16(LOCK, x.o.0 as u16);
        x.set16(0x14, LOCK_FRAMES);
    }

    /// Sorte 3 : pousse un joueur qui appuie la touche 3 et dont la hauteur est au moins 0x28 :
    /// il est déplacé de 0x28 en profondeur et abaissé d'autant.
    fn kick_up(x: &mut Cx, p: Obj) {
        if p.u8(x.b, 0x6c) & 8 == 0 || (p.u16(x.b, pl::Z) as i16) < 0x28 {
            return;
        }
        grab(x);
        let y = p.u16(x.b, pl::Y).wrapping_add(0x28);
        p.set_u16(x.b, pl::Y, y);
        let z = p.u16(x.b, pl::Z).wrapping_sub(0x28);
        p.set_u16(x.b, pl::Z, z);
    }
}

/// Appel d'une routine du jeu avec D0 = `d0`.
fn call_with(x: &mut Cx, addr: u32, d0: u32) {
    x.c.d[0] = d0;
    x.call(addr);
}

/// Variables de la RAM de travail (A5+) : abscisse et ordonnée de la caméra, caméra secondaire.
mod ram {
    pub const CAMERA_X: u32 = 0x506;
    pub const CAMERA_Y: u32 = 0x50a;
    pub const CAMERA2_Y: u32 = 0x55a;
}

/// Générateur de nombres aléatoires (D0 en retour).
const RANDOM: u32 = 0x19a88;

/// Fige la partie et assombrit l'écran : `A5+0x490` = 1, `A5+0x48e` = 0xc, `A5+0x48f` = `flag` pas, palettes
/// blanchies (0x1686) puis tâche de fondu 0x14de (0x9b6, A0 = l'entrée, D0 = 0x10 ; `taches/fondu.rs`).
fn show_message(x: &mut Cx, flag: u8) {
    x.set_a5_u16(0x490, 1);
    x.set_a5_u8(0x48e, 0xc);
    x.set_a5_u8(0x48f, flag);
    x.call(0x1686);
    call_on_list(x, 0x14de);
}

/// Appelle la routine 0x9b6 avec A0 = `list` et D0 = 0x10 (A0 est rétabli ensuite).
fn call_on_list(x: &mut Cx, list: u32) {
    let me = x.o.0;
    x.c.a[1] = me;
    x.c.a[0] = list;
    x.c.d[0] = 0x10;
    crate::call(x.c, x.b, 0x9b6);
    x.c.a[0] = me;
}

// ---------------------------------------------------------------------------------------------
// Type 17 (0xd466a) : séquence d'arrivée du boss (h)
// ---------------------------------------------------------------------------------------------

/// Séquenceur de la cinématique d'arrivée du boss (h) : une suite de 13 étapes (`+5`) qui
/// attendent que la caméra atteigne une abscisse (0x100, 0x180, 0x200, 0x2b0) ou qu'un
/// compte à rebours (`+0x42`) s'écoule, et lancent à chaque fois des sons, des messages
/// (0x2824, 0x2860, 0x289c, 0xeda26, 0xeda66...) et des explosions aléatoires ; la dernière
/// étape crée le boss (objet BOSS de type 2).
mod boss_intro {
    use super::*;

    const STEP: u32 = 0x42;
    const BLAST: u32 = 0x46;
    /// Routines de son / message (D0 = numéro).
    const SAY_A: u32 = 0x2824;
    const SAY_B: u32 = 0x2860;
    const SAY_C: u32 = 0x289c;
    const EDA26: u32 = 0xeda26;
    const EDA66: u32 = 0xeda66;
    const ED94A: u32 = 0xed94a;
    const ED9BC: u32 = 0xed9bc;
    const A2B4A: u32 = 0xa2b4a;
    const SMOKE: u32 = 0xb4bcc;
    const BOSS_ALLOC: u32 = 0x23a8;
    const FADE: u32 = 0x1686;
    const BLAST_AT: u32 = 0x667f4;
    /// Tables de décalages (16 mots chacune) de l'explosion en X et en Y.
    const BLAST_DX: u32 = 0xd4910;
    const BLAST_DY: u32 = 0xd4930;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(BLAST, 0x14);
            }
            2 => step(x),
            4 => x.add8(0x4, 2),
            _ => x.call(sub::FREE_SET),
        }
    }

    fn camera_x(x: &mut Cx) -> i16 {
        x.a5_u16(ram::CAMERA_X) as i16
    }

    /// Passe à l'étape suivante et arme le compte à rebours de l'étape.
    fn next(x: &mut Cx, frames: u8) {
        x.set8(STEP, frames);
        x.add8(0x5, 2);
    }

    fn countdown(x: &mut Cx) -> bool {
        let n = x.u8(STEP).wrapping_sub(1);
        x.set8(STEP, n);
        n == 0
    }

    fn step(x: &mut Cx) {
        match x.u8(0x5) {
            0 => {
                x.add8(0x5, 2);
                freeze_and_show(x);
            }
            2 => {
                if camera_x(x) >= 0x100 {
                    x.call(EDA26);
                    x.add8(0x5, 2);
                    call_with(x, SAY_A, 0x1e);
                }
            }
            4 => {
                if camera_x(x) >= 0x180 {
                    next(x, 0x28);
                    call_with(x, SAY_A, 0x28);
                }
            }
            6 => {
                if countdown(x) {
                    next(x, 0x28);
                    call_with(x, SAY_B, 0x28);
                }
            }
            8 => {
                if countdown(x) {
                    next(x, 0x28);
                    call_with(x, SAY_A, 0x28);
                }
            }
            10 => {
                if countdown(x) {
                    x.add8(0x5, 2);
                    x.call(EDA66);
                }
            }
            12 => {
                if camera_x(x) >= 0x200 {
                    next(x, 0x32);
                    call_with(x, SAY_B, 0x32);
                    x.call(EDA26);
                }
            }
            14 => {
                if countdown(x) {
                    x.call(A2B4A);
                    next(x, 0x32);
                    call_with(x, SAY_C, 0x32);
                }
            }
            16 => explosions(x),
            18 => {
                if countdown(x) {
                    x.add8(0x5, 2);
                    x.call(EDA66);
                }
            }
            20 => {
                if camera_x(x) >= 0x2b0 {
                    next(x, 0x28);
                    call_with(x, SAY_C, 0x50);
                    x.call(EDA26);
                }
            }
            22 => spawn_boss(x),
            24 => {
                let n = x.u8(BLAST).wrapping_sub(1);
                x.set8(BLAST, n);
                if n == 0 {
                    x.call(EDA66);
                }
                let n = x.u8(STEP).wrapping_sub(1);
                x.set8(STEP, n);
                if (n as i8) < 0 {
                    x.add8(0x4, 2);
                }
            }
            _ => {}
        }
    }

    /// Étape 0 : fige la partie (`A5+0x48e` = 0xc, `A5+0x48f` = 7), lance le fondu 0x1686 et la
    /// tâche de fondu 0x14de (0x9b6).
    fn freeze_and_show(x: &mut Cx) {
        show_message(x, 7);
    }

    /// Étape 16 : toutes les 20 images un coup de ED94A ; quand `+0x42` est un multiple de 4
    /// une explosion au hasard ; à 0, fin.
    fn explosions(x: &mut Cx) {
        let n = x.u8(BLAST).wrapping_sub(1);
        x.set8(BLAST, n);
        if (n as i8) < 0 {
            x.call(ED94A);
            x.set8(BLAST, 0x14);
        }
        if x.u8(STEP) & 3 == 0 {
            x.call(A2B4A);
            random_blast(x);
        }
        if countdown(x) {
            x.add8(0x5, 2);
            call_with(x, SAY_B, 0x32);
        }
    }

    fn random_blast(x: &mut Cx) {
        x.call(RANDOM);
        let i = (x.c.d[0] & 0xf) << 1;
        let dx = x.rom16(BLAST_DX + i);
        x.call(RANDOM);
        let i = (x.c.d[0] & 0xf) << 1;
        let dy = x.rom16(BLAST_DY + i);
        let px = dx.wrapping_add(x.a5_u16(ram::CAMERA_X));
        let py = dy.wrapping_add(x.a5_u16(ram::CAMERA_Y));
        x.c.d[2] = (x.c.d[2] & 0xffff_0000) | px as u32;
        x.c.d[3] = (x.c.d[3] & 0xffff_0000) | py as u32;
        x.c.d[0] = (x.c.d[0] & 0xffff_0000) | px as u32;
        x.c.d[1] = (x.c.d[1] & 0xffff_0000) | py as u32;
        x.call(BLAST_AT);
    }

    /// Étape 22 : 30 fumées, puis le boss (BOSS type 2) apparaît.
    fn spawn_boss(x: &mut Cx) {
        if !countdown(x) {
            return;
        }
        x.c.d[1] = 0x1d;
        x.c.d[2] = 0;
        loop {
            x.call(SMOKE);
            let n = (x.c.d[1] as u16).wrapping_sub(1);
            x.c.d[1] = (x.c.d[1] & 0xffff_0000) | n as u32;
            if n == 0xffff {
                break;
            }
        }
        x.call(ED9BC);
        x.set_a5_u8(0x48e, 0xf);
        x.call(FADE);
        x.call(BOSS_ALLOC);
        if x.c.sr & m68k::SR_C != 0 {
            return;
        }
        let boss = Obj(x.c.a[1]);
        boss.set_u8(x.b, 0, 1);
        boss.set_u8(x.b, 2, 2);
        x.add8(0x5, 2);
        x.set8(STEP, 0x3c);
        x.set8(BLAST, 0x28);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 18 (0xd4950) : défilement vertical vers une cible
// ---------------------------------------------------------------------------------------------

/// Fait descendre la caméra secondaire d'un pixel par image jusqu'à la hauteur `+0x42`, puis
/// libère l'objet lié (`+0x46`) et prévient la routine 0x2824.
mod scroll_to {
    use super::*;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => x.add8(0x4, 2),
            2 => {
                let y = x.a5_u32(ram::CAMERA2_Y).wrapping_add(0x10000);
                x.set_a5_u32(ram::CAMERA2_Y, y);
                let target = x.u16(0x42);
                if target as i16 > (y >> 16) as i16 {
                    return;
                }
                x.set_a5_u16(ram::CAMERA2_Y, target);
                if x.u16(0x46) != 0 {
                    let linked = x.linked(0x46);
                    linked.set_u8(x.b, 0x2e, 0);
                    let n = linked.u16(x.b, 0x2a).wrapping_sub(1);
                    linked.set_u16(x.b, 0x2a, n);
                    call_with(x, 0x2824, 0x10);
                }
                x.call(sub::FREE_SET);
            }
            _ => x.call(sub::FREE_SET),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 19 (0xd49a2) : générateur de vagues d'ennemis (h)
// ---------------------------------------------------------------------------------------------

/// Générateur de vagues (h) : il se déclenche quand un joueur vivant arrive à portée (0xc0 en X,
/// 0x30 en profondeur), lit alors un script de la ROM (entrées de 0x14 octets, une liste par
/// variante) et crée les objets décrits (ennemis, boss ou décors) quand la partie est en jeu
/// (`A5+0x2c` = 0xc), en tenant le compte des objets encore en vie (`+0x2a`, que ces objets
/// décrémentent à leur mort en retrouvant leur générateur dans `+0x46`).
///
/// Entrée de script : +0 mot (> 0 : délai avant la suivante ; < 0 : attendre que le nombre
/// d'objets vivants soit au plus l'octet bas, bit 7 de cet octet : attendre qu'aucun ne vive),
/// +2 rang minimal, +4/+6/+8 position (X, Z, Y), +0xa nombre de joueurs minimal, +0xb routine de
/// création (index en octets dans la table 0xd4b92), +0xc type, +0xd variante, +0xe/+0xf
/// paramètres, +0x10 mot `+0x42`, +0x12 position absolue si non nul (sinon relative à la caméra).
/// Un mot 0x8000 termine le script.
mod wave_spawner {
    use super::*;

    /// Pointeurs de scripts, par variante (espace de données).
    const SCRIPTS: u32 = 0xd4c5e;
    /// Variante -> forme du démarrage / de la fin (octets, espace programme).
    const START_KIND: u32 = 0xd4a0e;
    const END_KIND: u32 = 0xd4bdc;
    /// Routines de création par index (longs, espace de données).
    const CREATORS: u32 = 0xd4b92;
    const ENTRY_SIZE: u32 = 0x14;
    const SCRIPT_END: u16 = 0x8000;
    /// Routines propres à la variante, appelées au démarrage et à la fin.
    const ON_START: u32 = 0xd98b8;
    const ON_END: u32 = 0xd995a;
    /// Octet de la RAM (A5+0x51) : générateurs actifs. Octet (A5+0x2c) : mode de jeu (0xc : en jeu).
    const ACTIVE_COUNT: u32 = 0x51;
    const GAME_MODE: u32 = 0x2c;
    const PLAYING: u8 = 0xc;
    /// Mot de la RAM (A5+0x51e) : limite de défilement imposée par la vague.
    const SCROLL_LIMIT: u32 = 0x51e;
    /// Champs de l'objet : délai global, pointeur de script, attente, seuil, vivants.
    const TIMER: u32 = 0x24;
    const CURSOR: u32 = 0x20;
    const DELAY: u32 = 0x26;
    const THRESHOLD: u32 = 0x18;
    const ALIVE: u32 = 0x2a;
    const DONE: u32 = 0x2c;
    const ABORT: u32 = 0x2d;
    const WAIT_ALL: u32 = 0x2e;

    pub fn run(x: &mut Cx) {
        if x.u8(0x40) != 0 {
            return;
        }
        match x.u8(0x4) {
            0 => start(x),
            2 => play(x),
            4 => finish(x),
            _ => x.call(sub::FREE_SET),
        }
    }

    /// Un joueur vivant est à portée du générateur.
    fn player_near(x: &mut Cx) -> bool {
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) == 0 || p.u16(x.b, pl::HEALTH) == 0 {
                continue;
            }
            let dx = p.u16(x.b, pl::X).wrapping_sub(x.u16(0x8)).wrapping_add(0xc0);
            if dx > 0x180 {
                continue;
            }
            let dy = p.u16(x.b, pl::Y).wrapping_sub(x.u16(0x10)).wrapping_add(0x30);
            if dy <= 0x60 {
                return true;
            }
        }
        false
    }

    fn start(x: &mut Cx) {
        if !player_near(x) {
            return;
        }
        let n = x.a5_u8(ACTIVE_COUNT).wrapping_add(1);
        x.set_a5_u8(ACTIVE_COUNT, n);
        x.set16(THRESHOLD, 0xffff);
        x.set16(ALIVE, 0);
        x.set8(DONE, 0);
        x.set8(ABORT, 0);
        let variant = x.u8(0x3) as u32;
        let script = x.data32(SCRIPTS + 4 * variant);
        let timer = x.data16(script);
        x.set16(TIMER, timer);
        x.set32(CURSOR, script + 2);
        x.call(ON_START);
        match x.rom8(START_KIND + variant) {
            0 => x.set_a5_u16(SCROLL_LIMIT, 0x20),
            2 => x.set_a5_u16(SCROLL_LIMIT, 4),
            _ => {}
        }
        x.add8(0x4, 2);
    }

    /// Fin du déroulement (arrêt demandé, temps écoulé ou script terminé).
    fn stop(x: &mut Cx) {
        x.set8(DONE, 1);
        end_script(x);
    }

    fn end_script(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set16(DELAY, 0);
    }

    fn play(x: &mut Cx) {
        if x.a5_u8(GAME_MODE) != PLAYING {
            return;
        }
        if x.a5_u8(0x42) != 0 || x.u8(ABORT) != 0 {
            return stop(x);
        }
        let n = x.u16(TIMER).wrapping_sub(1);
        x.set16(TIMER, n);
        if (n as i16) < 0 {
            return stop(x);
        }
        if x.u8(WAIT_ALL) != 0 {
            return;
        }
        let threshold = x.u16(THRESHOLD) as i16;
        if threshold < 0 {
            let d = x.u16(DELAY).wrapping_sub(1);
            x.set16(DELAY, d);
            if (d as i16) >= 0 {
                return;
            }
        } else if threshold < x.u16(ALIVE) as i16 {
            return;
        }
        next_entry(x);
    }

    /// Exécute l'entrée courante du script (en sautant celles que le nombre de joueurs ou le rang
    /// excluent).
    fn next_entry(x: &mut Cx) {
        let mut e = x.u32(CURSOR);
        loop {
            let head = x.data16(e);
            if head == SCRIPT_END {
                return end_script(x);
            }
            let players_needed = x.data8(e + 0xa) as i8;
            let rank_needed = x.data16(e + 2) as i16;
            if players_needed > x.a5_u8(0x35) as i8 || rank_needed > x.a5_u16(0x7f92) as i16 {
                e += ENTRY_SIZE;
                x.set32(CURSOR, e);
                continue;
            }
            return run_entry(x, e, head);
        }
    }

    fn run_entry(x: &mut Cx, e: u32, head: u16) {
        x.set8(WAIT_ALL, 0);
        x.set16(THRESHOLD, 0xffff);
        x.set16(DELAY, head);
        if (head as i16) < 0 {
            let low = head & 0xff;
            if low as u8 as i8 <= -1 {
                x.set8(WAIT_ALL, 1);
            }
            x.set16(THRESHOLD, low);
        }
        let index = x.data8(e + 0xb) as u32;
        let creator = x.data32(CREATORS + index);
        x.call(creator);
        if x.c.sr & m68k::SR_C != 0 {
            x.set16(THRESHOLD, 0xffff);
            x.set16(DELAY, 0);
            return;
        }
        let o = Obj(x.c.a[1]);
        o.set_u8(x.b, 0, 1);
        let v = x.data8(e + 0xc);
        o.set_u8(x.b, 2, v);
        let v = x.data8(e + 0xd);
        o.set_u8(x.b, 3, v);
        let v = x.data8(e + 0xe);
        o.set_u8(x.b, 0x2f, v);
        let v = x.data8(e + 0xf);
        o.set_u8(x.b, 0x2c, v);
        let v = x.data16(e + 4);
        o.set_u16(x.b, 0x8, v);
        let v = x.data16(e + 6);
        o.set_u16(x.b, 0xc, v);
        let v = x.data16(e + 8);
        o.set_u16(x.b, 0x10, v);
        let v = x.data16(e + 0x10);
        o.set_u16(x.b, 0x42, v);
        x.add16(ALIVE, 1);
        let me = x.o.0 as u16;
        o.set_u16(x.b, 0x46, me);
        if x.data8(e + 0x12) == 0 {
            // position relative à la caméra
            let camera = if x.a5_u8(0x36) != 0 { 0x54e } else { 0x4fe };
            let cam = Obj(A5 + camera);
            let v = o.u16(x.b, 0x8).wrapping_add(cam.u16(x.b, 0x8));
            o.set_u16(x.b, 0x8, v);
            let v = o.u16(x.b, 0x10).wrapping_add(cam.u16(x.b, 0xc));
            o.set_u16(x.b, 0x10, v);
        }
        x.set32(CURSOR, e + ENTRY_SIZE);
    }

    /// Plus aucun objet vivant : libère le compte, laisse la variante conclure et libère la
    /// limite de défilement.
    fn finish(x: &mut Cx) {
        let n = x.u16(TIMER).wrapping_sub(1);
        x.set16(TIMER, n);
        if (n as i16) < 0 {
            x.set8(DONE, 1);
            x.set16(TIMER, 0x7fff);
        }
        if x.u16(ALIVE) != 0 {
            return;
        }
        let n = x.a5_u8(ACTIVE_COUNT).wrapping_sub(1);
        x.set_a5_u8(ACTIVE_COUNT, if (n as i8) < 0 { 0 } else { n });
        x.call(ON_END);
        let variant = x.u8(0x3) as u32;
        match x.rom8(END_KIND + variant) {
            0 => {
                // fixe la limite de défilement à la caméra courante
                let v = x.a5_u16(ram::CAMERA_X);
                x.set_a5_u16(0x52c, v);
                x.set_a5_u16(SCROLL_LIMIT, 0);
            }
            2 => x.set_a5_u16(SCROLL_LIMIT, 0),
            _ => {}
        }
        x.add8(0x4, 2);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 20 (0xd5f5a) : barre de vie
// ---------------------------------------------------------------------------------------------

/// Barre de vie d'un joueur (ou de deux joueurs réunis si `+3`), dessinée dans trois colonnes de
/// 44 cases de la couche de fond 1 (GFX RAM 0x91215c, 0x912160, 0x912164 ; 0x80 octets par ligne).
/// La barre se remplit par pièces de 8 points de vie : la vie affichée (`+0x2a`) rejoint la vie
/// réelle (`+0x32`, somme de `+0x62` du joueur) d'un point par image, en avançant le curseur
/// d'écriture (`+0x22`) et son sous-pas (`+0x26`). Au-delà de 0x160 points la barre passe à la
/// colonne suivante. `+0x30` = vie maximale (barre vide à droite), `+0x2c` / `+0x27` = longueur
/// du fond en pièces entières et en reste.
mod life_bar {
    use super::*;
    use crate::mem::{wr16, wr32};

    const COLUMN_A: u32 = 0x91215c;
    const COLUMN_B: u32 = 0x912160;
    const COLUMN_C: u32 = 0x912164;
    const ROW: u32 = 0x80;
    /// Colonne remise au vide quand la barre repasse sous 0x160.
    const CURSOR_AFTER_RESET: u32 = 0x913760;
    const ROWS: u32 = 44;
    const MAX_LEN: i16 = 0x2c0;
    const SECOND_COLUMN_FROM: i16 = 0x160;
    /// Case pleine, case vide.
    const FULL: u32 = 0x1318_000d;
    const BLANK: u32 = 0x0020_0000;
    /// Attribut des pièces.
    const PIECE_ATTR: u16 = 0xd;
    /// Pièces partielles : table de mots indexée par le nombre de points (1 à 8) * 2.
    const PIECES: u32 = 0xd61b2;
    /// Pointeurs (espace de données) vers l'habillage (octets, fin 0x40) selon le type du joueur.
    const FRAMES: u32 = 0xd6232;
    const FRAME_SHARED: u32 = 0xd628c;
    const FRAME_END: u8 = 0x40;
    const FRAME_ATTR: u16 = 2;
    const CAP: u32 = 0x003f_0002;

    pub fn run(x: &mut Cx) {
        let owner = x.linked(0x20);
        let (hp, max) = (owner.u16(x.b, 0x62), owner.u16(x.b, 0x60));
        x.set16(0x32, hp);
        x.set16(0x30, max);
        if x.u8(0x3) != 0 {
            let second = x.linked(0x34);
            let (hp2, max2) = (second.u16(x.b, 0x62), second.u16(x.b, 0x60));
            x.add16(0x32, hp2);
            x.add16(0x30, max2);
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                draw_frame(x);
                bar(x);
            }
            2 => bar(x),
            4 => dismiss(x),
            _ => x.call(sub::FREE_SET),
        }
    }

    fn bar(x: &mut Cx) {
        match x.u8(0x5) {
            0 => draw_full(x),
            2 => animate(x),
            _ => {}
        }
    }

    fn clear_column(x: &mut Cx, mut at: u32) {
        for _ in 0..ROWS {
            wr32(x.b, at, BLANK);
            at += ROW;
        }
    }

    /// Fin : colonnes vidées, objet libéré.
    fn dismiss(x: &mut Cx) {
        x.set8(0x4, 6);
        for c in [COLUMN_A, COLUMN_B, COLUMN_C] {
            clear_column(x, c);
        }
        x.call(sub::FREE_SET);
    }

    /// Habillage de la colonne de gauche (cases de la liste d'octets du type de joueur), et
    /// capuchon de la colonne suivante quand la vie dépasse la longueur maximale.
    fn draw_frame(x: &mut Cx) {
        let player = x.linked(0x20);
        let kind = player.u8(x.b, 2);
        let mut list = x.data32(FRAMES + 4 * kind as u32);
        if x.u8(0x3) != 0 && kind == 0 {
            list = FRAME_SHARED;
        }
        let mut at = COLUMN_A;
        loop {
            let tile = x.data8(list);
            list += 1;
            if tile == FRAME_END {
                break;
            }
            wr16(x.b, at, tile as u16);
            wr16(x.b, at + 2, FRAME_ATTR);
            at += ROW;
        }
        if x.u16(0x32) as i16 <= MAX_LEN {
            return;
        }
        let mut at = COLUMN_B;
        for _ in 0..5 {
            wr32(x.b, at, CAP);
            at += ROW;
        }
    }

    /// Case de la pièce de `d1` points (au plus 8), décalée de `d0`.
    fn piece(x: &mut Cx, at: u32, d0: i16, d1: i16) {
        let points = d1.min(8);
        let base = x.rom16(PIECES.wrapping_add((points.wrapping_mul(2)) as i32 as u32));
        wr16(x.b, at, base.wrapping_add(d0 as u16));
        wr16(x.b, at + 2, PIECE_ATTR);
    }

    /// Sous-état 0 : trace toute la barre (la vie affichée prend la vie réelle).
    fn draw_full(x: &mut Cx) {
        if x.u16(0x32) as i16 > MAX_LEN {
            return;
        }
        x.add8(0x5, 2);
        let mut at = COLUMN_B;
        let mut d0 = x.u16(0x32) as i16;
        x.set16(0x2a, d0 as u16);
        let mut d1 = x.u16(0x30) as i16;
        if d1 == 0 {
            return;
        }
        d1 = d1.min(MAX_LEN);
        x.set16(0x2c, d1 as u16 & !7);
        x.set8(0x27, d1 as u8 & 7);
        let mut columns_left = 2;
        loop {
            let mut d2: i16 = (ROWS - 1) as i16;
            loop {
                d1 -= 8;
                d0 -= 8;
                if d0 <= 0 {
                    d1 += 8;
                    d0 += 8;
                    if d0 == 0 {
                        return tail(x, at, d0, d1, d2, true);
                    }
                    return partial(x, at, d0, d1, d2);
                }
                wr32(x.b, at, FULL);
                at += ROW;
                d2 -= 1;
                if d2 == -1 {
                    break;
                }
            }
            if d0 == 0 {
                return tail(x, at - ROW, 0, d1, d2, true);
            }
            columns_left -= 1;
            if columns_left == 0 {
                return tail(x, at, d0, d1, d2, true);
            }
            at = COLUMN_C;
        }
    }

    /// Dernière pièce de la partie pleine, puis la partie vide.
    fn partial(x: &mut Cx, at: u32, d0: i16, d1: i16, d2: i16) {
        piece(x, at, d0, d1);
        if d1 <= 8 {
            return tail(x, at, d0, d1, d2, true);
        }
        x.set32(0x22, at);
        tail(x, at + ROW, d0, d1, d2 - 1, false)
    }

    /// Mémorise le curseur (`+0x22`) et le sous-pas (`+0x26`), puis trace la partie restante
    /// (pièces vides) jusqu'à épuisement de `d1`.
    fn tail(x: &mut Cx, mut at: u32, d0: i16, mut d1: i16, mut d2: i16, set_cursor: bool) {
        if set_cursor {
            x.set32(0x22, at);
        }
        x.set8(0x26, d0 as u8);
        if x.u8(0x26) == 0 {
            x.set8(0x26, 8);
        }
        loop {
            d1 -= 8;
            if d1 <= 0 {
                return;
            }
            piece(x, at, 0, d1);
            at += ROW;
            d2 -= 1;
            if d2 == -1 {
                d2 = (ROWS - 1) as i16;
                at = COLUMN_C;
            }
        }
    }

    /// Sous-état 2 : la vie affichée rejoint la vie réelle d'un point par image.
    fn animate(x: &mut Cx) {
        let shown = x.u16(0x2a) as i16;
        if shown == 0 {
            return dismiss(x);
        }
        let target = x.u16(0x32) as i16;
        if shown == target {
            return;
        }
        let mut at = x.u32(0x22);
        let full_len = x.u16(0x2c) as i16;
        if shown > target {
            let d0 = shown - 1;
            x.set16(0x2a, d0 as u16);
            if d0 == SECOND_COLUMN_FROM {
                clear_column(x, COLUMN_C);
                x.set8(0x26, 0);
                x.set32(0x22, CURSOR_AFTER_RESET);
                return;
            }
            let n = x.u8(0x26).wrapping_sub(1);
            x.set8(0x26, n);
            if (n as i8) < 0 {
                x.set8(0x26, 7);
                at -= ROW;
            }
            let d1 = if d0 < full_len { 8 } else { x.u8(0x27) as i16 };
            let sub_step = x.u8(0x26) as i16;
            piece(x, at, sub_step, d1);
            x.set32(0x22, at);
        } else {
            let d0 = shown + 1;
            x.set16(0x2a, d0 as u16);
            if d0 == SECOND_COLUMN_FROM + 1 {
                x.set8(0x26, 1);
                let d1 = (x.u16(0x30) as i16).min(MAX_LEN) - SECOND_COLUMN_FROM;
                partial(x, COLUMN_C, d0 - SECOND_COLUMN_FROM, d1, (ROWS - 1) as i16);
                x.set32(0x22, COLUMN_C);
                return;
            }
            let n = x.u8(0x26) + 1;
            x.set8(0x26, n);
            if n == 9 {
                x.set8(0x26, 1);
                at += ROW;
            }
            let d1 = if d0 <= full_len { 8 } else { x.u8(0x27) as i16 };
            let sub_step = x.u8(0x26) as i16;
            piece(x, at, sub_step, d1);
            x.set32(0x22, at);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 21 (0xd6372) : compte à rebours de continuation
// ---------------------------------------------------------------------------------------------

/// Affichage du compte à rebours d'un joueur qui peut continuer (h) : trois chiffres (`+0x14`,
/// `+0x15`, `+0x16`, de 0:00 à 9:59 (h)) dessinés dans la couche de fond 1 (0x912b38) et décomptés
/// à la seconde (`+0x17` compte 60 images) ; l'inscription « prêt » clignote à côté (rangées de
/// cases constantes en 0x912830). À zéro, les joueurs encore vivants sont mis à mort. Variante 1 :
/// version sans compte (invitation seule), variante 2 : ne dépend pas de `A5+0x50`.
/// États : 0 initialisation (palette 0x91c420), 2 comptage, 4 fin de partie (inscription finale
/// en 0x9127b0), 6 libération. Le script de son 0xd6922 annonce l'affichage, 0xd694e son retrait.
mod continue_countdown {
    use super::*;
    use crate::mem::{rd8, wr16, wr32};

    const PALETTE_SRC: u32 = 0x25c0e0;
    const PALETTE_DST: u32 = 0x91c420;
    const BRIGHT: u32 = 0xf000_f000;
    /// Octet (A5-0x5b8e = 0xff2472) : son coupé.
    const MUTED: u32 = 0xff_2472;
    const SCRIPT_ON: u32 = 0xd6922;
    const SCRIPT_OFF: u32 = 0xd694e;
    const PLAY_SCRIPT: u32 = 0x1324;
    const SPAWN_INVITE: u32 = 0xdfa4a;
    /// Variante -> paramètre de l'invitation (`+0x2c`), selon le niveau `A5+0x34` (octets).
    const INVITE_BY_STAGE: u32 = 0xd6464;
    const DIGITS: u32 = 0x912b38;
    const GET_READY: u32 = 0x912830;
    const GAME_OVER: u32 = 0x9127b0;
    const ROW: u32 = 0x80;
    const ATTR: u32 = 3;
    const COLON: u16 = 0x3a;
    /// Rangées (tuile gauche, tuile droite) de l'inscription clignotante (attribut 3).
    const GET_READY_ROWS: [(u16, u16); 16] = [
        (0x192, 0x1a2), (0x193, 0x1a3), (0x1bc, 0x1cc), (0x1bd, 0x1cd),
        (0x1b6, 0x1c6), (0x1b7, 0x1c7), (0x1b6, 0x1c6), (0x1b7, 0x1c7),
        (0x1d4, 0x1e4), (0x1d5, 0x1e5), (0x2fc, 0x2fc), (0x2fc, 0x2fc),
        (0x1bc, 0x1cc), (0x1bd, 0x1cd), (0x1b2, 0x1c2), (0x1b3, 0x1c3),
    ];
    /// Inscription de la fin de partie.
    const GAME_OVER_ROWS: [(u16, u16); 18] = [
        (0x1ba, 0x1ca), (0x1bb, 0x1cb), (0x194, 0x1a4), (0x195, 0x1a5),
        (0x19c, 0x1ac), (0x19d, 0x1ad), (0x17c, 0x18c), (0x17d, 0x18d),
        (0x2fc, 0x2fc), (0x2fc, 0x2fc), (0x1b0, 0x1c0), (0x1b1, 0x1c1),
        (0x1be, 0x1ce), (0x1bf, 0x1cf), (0x17c, 0x18c), (0x17d, 0x18d),
        (0x1b6, 0x1c6), (0x1b7, 0x1c7),
    ];
    /// États du joueur condamné.
    const DOOMED: u32 = 0x0204_0600;
    const SAFE_STATE: u32 = 0x0200_2000;

    pub fn run(x: &mut Cx) {
        let now = x.u8(0x40) as i8;
        if now > x.u8(0x1c) as i8 {
            x.add8(0x14, 1);
            hide(x);
        }
        x.set8(0x1c, now as u8);
        if x.u8(0x3) != 2 && x.a5_u8(0x50) != 0 {
            return hide(x);
        }
        if x.a5_u8(0x42) != 0 || x.a5_u8(GAME_PAUSED) != 0 {
            return hide(x);
        }
        let camera = if x.a5_u8(0x36) != 0 { 0x54e } else { 0x4fe };
        let camera = Obj(A5 + camera);
        match x.u8(0x4) {
            0 => init(x, camera),
            2 => counting(x, camera),
            4 => finish(x),
            _ => x.call(sub::FREE_SET),
        }
    }

    const GAME_PAUSED: u32 = 0x3d;

    /// Retire l'annonce (une fois) et coupe l'affichage.
    fn hide(x: &mut Cx) {
        if x.u8(0x18) == 0 {
            return;
        }
        x.set8(0x18, 0);
        if rd8(x.b, MUTED) != 0 {
            return;
        }
        x.c.a[4] = SCRIPT_OFF;
        x.call(PLAY_SCRIPT);
    }

    fn rows(x: &mut Cx, base: u32, rows: &[(u16, u16)]) {
        for (i, (left, right)) in rows.iter().enumerate() {
            let at = base + ROW * i as u32;
            wr32(x.b, at, (*left as u32) << 16 | ATTR);
            wr32(x.b, at + 4, (*right as u32) << 16 | ATTR);
        }
    }

    fn init(x: &mut Cx, camera: Obj) {
        x.add8(0x4, 2);
        x.set16(0x4c, 0x400);
        let v = camera.u16(x.b, 0x8);
        x.set16(0x4e, v);
        x.set8(0x17, 0);
        let v = x.a5_u8(0x35);
        x.set8(0x1b, v);
        if x.u8(0x5) == 0 {
            let stage = x.a5_u8(0x34) as u32;
            let v = x.rom8(INVITE_BY_STAGE + stage);
            x.set8(0x2c, v);
        }
        x.set8(0x5, 0);
        // palette : six longs forcés au maximum de luminosité, les deux derniers additionnés
        for i in 0..8 {
            let v = x.data32(PALETTE_SRC + 4 * i);
            let v = if i < 6 { v | BRIGHT } else { v.wrapping_add(BRIGHT) };
            wr32(x.b, PALETTE_DST + 4 * i, v);
        }
        let v = x.u8(0x42);
        x.set8(0x14, v);
        let packed = x.u8(0x43);
        x.set8(0x16, packed);
        x.set8(0x15, packed >> 4);
        x.set8(0x16, packed & 0xf);
    }

    fn counting(x: &mut Cx, camera: Obj) {
        let players = x.a5_u8(0x35);
        let known = x.u8(0x1b);
        if players != known {
            if (players as i8) < (known as i8) {
                x.set8(0x1b, players);
            } else {
                x.set16(0x4, 0x401);
                x.set16(0x4c, 0);
                return;
            }
        }
        if players == 0 {
            return hide(x);
        }
        if x.a5_u8(0x51) != 0 {
            x.set16(0x4c, 0);
            return hide(x);
        }
        if x.u8(0x3) != 1 {
            if !tick(x) {
                return;
            }
            if camera.u16(x.b, 0x20) == 4 {
                x.set16(0x4c, 0x200);
                return;
            }
        }
        invite(x, camera)
    }

    /// Décompte d'une image ; faux si le temps est écoulé (les joueurs sont alors condamnés).
    fn tick(x: &mut Cx) -> bool {
        let n = x.u8(0x17).wrapping_sub(1);
        x.set8(0x17, n);
        if (n as i8) < 0 {
            x.set8(0x17, 0x3c);
            if x.u8(0x14) == 0 {
                x.set8(0x18, 1);
            }
            let n = x.u8(0x16).wrapping_sub(1);
            x.set8(0x16, n);
            if (n as i8) < 0 {
                x.set8(0x16, 9);
                let n = x.u8(0x15).wrapping_sub(1);
                x.set8(0x15, n);
                if (n as i8) < 0 {
                    x.set8(0x15, 5);
                    let n = x.u8(0x14).wrapping_sub(1);
                    x.set8(0x14, n);
                    if (n as i8) < 0 {
                        time_out(x);
                        return false;
                    }
                }
            }
        }
        if x.u8(0x18) != 0 {
            draw(x);
        }
        true
    }

    /// Le temps est écoulé : les joueurs vivants sont condamnés.
    fn time_out(x: &mut Cx) {
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) == 0 || p.u8(x.b, pl::STATE) as i8 >= 4 {
                continue;
            }
            if p.u32(x.b, pl::STATE) & 0xffff_ff00 == SAFE_STATE {
                continue;
            }
            p.set_u16(x.b, pl::HEALTH, 0);
            p.set_u32(x.b, pl::STATE, DOOMED);
            let n = x.a5_u8(0x35).wrapping_sub(1);
            x.set_a5_u8(0x35, n);
            if p.u8(x.b, 0x8a) != 0 {
                p.set_u8(x.b, 0x8a, 0);
                let a2 = Obj(p.u32(x.b, 0x86) & 0xff_ffff);
                a2.set_u8(x.b, 5, 8);
                a2.set_u16(x.b, 6, 0);
            }
        }
        x.set16(0x4, 0x400);
        x.set_a5_u8(0x43, 0xff);
        x.set_a5_u16(0x4ba, 0);
    }

    /// Affiche les chiffres et fait clignoter l'inscription.
    fn draw(x: &mut Cx) {
        let (d0, d1, d2) = (x.u8(0x14) as u16, x.u8(0x15) as u16, x.u8(0x16) as u16);
        for (i, tile) in [0, d0, COLON, d1, d2].into_iter().enumerate() {
            wr16(x.b, DIGITS + ROW * i as u32, tile);
            wr16(x.b, DIGITS + ROW * i as u32 + 2, ATTR as u16);
        }
        let n = x.u8(0x19).wrapping_sub(1);
        x.set8(0x19, n);
        if n != 0 {
            return;
        }
        x.set8(0x19, 0x80);
        if x.u8(0x15) == 0 {
            x.set8(0x19, 0x10);
        }
        let was = x.bit(0x1a, 0);
        x.set_bit(0x1a, 0, !was);
        if !was {
            rows(x, GET_READY, &GET_READY_ROWS);
        } else if rd8(x.b, MUTED) == 0 {
            x.c.a[4] = SCRIPT_OFF;
            x.call(PLAY_SCRIPT);
        }
    }

    /// Invitation périodique (h) : tant que la caméra ne bouge pas, crée toutes les 256 images
    /// un objet SOBJ d'invitation.
    fn invite(x: &mut Cx, camera: Obj) {
        if x.u8(0x40) != 0 {
            return;
        }
        if x.a5_u8(0x58) != 0 {
            return x.set16(0x4c, 0);
        }
        let here = camera.u16(x.b, 0x8);
        if here != x.u16(0x4e) {
            x.set16(0x4e, here);
            return x.set16(0x4c, 0x200);
        }
        let n = x.u16(0x4c).wrapping_sub(1);
        x.set16(0x4c, n);
        if (n as i16) >= 0 || x.u8(0x3) == 2 {
            return;
        }
        x.set16(0x4c, 0x100);
        x.c.d[1] = (x.c.d[1] & !0xff) | x.u8(0x2c) as u32;
        x.c.d[0] = 0;
        x.call(SPAWN_INVITE);
    }

    /// État 4 : inscription finale puis retour à l'état 0 (sous-état 1) après 0x80 images.
    fn finish(x: &mut Cx) {
        if x.u8(0x5) == 0 {
            x.add8(0x5, 1);
            x.set16(0x4c, 0x80);
            if rd8(x.b, MUTED) != 0 {
                return;
            }
            x.c.a[4] = SCRIPT_ON;
            x.call(PLAY_SCRIPT);
            rows(x, GAME_OVER, &GAME_OVER_ROWS);
            return;
        }
        let n = x.u16(0x4c).wrapping_sub(1);
        x.set16(0x4c, n);
        if (n as i16) >= 0 {
            return;
        }
        x.set8(0x4, 0);
        x.set8(0x5, 1);
        x.set8(0x18, 0);
        if rd8(x.b, MUTED) != 0 {
            return;
        }
        x.c.a[4] = SCRIPT_ON;
        x.call(PLAY_SCRIPT);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 22 (0xd69c2) : ajustement du rang en fin de zone
// ---------------------------------------------------------------------------------------------

/// Ajuste le rang de difficulté (A5+0x7f92) à l'arrivée dans une zone (h) : d'après le plus faible
/// niveau de vie (`+0x62`) parmi les joueurs présents, classé en quatre catégories (<= 0x10,
/// <= 0x30, <= 0x50, au-delà), il lit une variation dans une table de la ROM (une ligne de 4 mots
/// par nombre de joueurs, une table par variante) et la ramène dans l'intervalle [minimum,
/// maximum]. Il agit une seule fois, puis se libère.
mod rank_adjust {
    use super::*;

    /// Pointeurs de tables (espace de données), par variante.
    const TABLES: u32 = 0xd6aa0;
    const RANK: u32 = 0x7f92;
    const RANK_MAX: u32 = 0x7f94;
    const RANK_MIN: u32 = 0x7f96;
    const RANK_SAVED_FROM: u32 = 0x7f9c;
    const RANK_SAVED: u32 = 0x7fa4;
    /// Seuils de vie et catégories correspondantes (0 = presque mort ... 6 = en forme).
    const THRESHOLDS: [(u16, u16); 3] = [(0x30, 2), (0x50, 4), (0xffff, 6)];

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            // l'état 0 se prolonge dans l'état 2 : deux fois +2
            0 => {
                x.add8(0x4, 4);
                adjust(x);
            }
            2 => {
                x.add8(0x4, 2);
                adjust(x);
            }
            _ => x.call(sub::FREE_SET),
        }
    }

    fn adjust(x: &mut Cx) {
        if x.a5_u8(0x30) == 0 {
            return;
        }
        let variant = x.u8(0x3) as u32;
        let table = x.data32(TABLES + 4 * variant);
        let count = x.a5_u8(0x35) as u32;
        if count == 0 {
            return;
        }
        let row = count << 3;
        let mut category: u16 = 0xffff;
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) == 0 {
                continue;
            }
            let hp = p.u16(x.b, pl::HEALTH);
            if hp <= 0x10 {
                category = 0;
                break;
            }
            let class = THRESHOLDS.iter().find(|(limit, _)| hp <= *limit).map_or(6, |t| t.1);
            if class <= category {
                category = class;
            }
        }
        let at = table.wrapping_add(row.wrapping_add(category as u32) as u16 as i16 as i32 as u32);
        let delta = x.data16(at) as i16;
        if delta == 0 {
            return;
        }
        let saved = x.a5_u16(RANK_SAVED_FROM);
        x.set_a5_u16(RANK_SAVED, saved);
        let mut rank = (x.a5_u16(RANK) as i16).wrapping_add(delta);
        if delta > 0 {
            if rank >= x.a5_u16(RANK_MAX) as i16 {
                rank = x.a5_u16(RANK_MAX) as i16;
            }
        } else if rank < x.a5_u16(RANK_MIN) as i16 {
            rank = x.a5_u16(RANK_MIN) as i16;
        }
        x.set_a5_u16(RANK, rank as u16 & 0x1f);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 23 (0xd6c5c) : porte de sortie de zone (h)
// ---------------------------------------------------------------------------------------------

/// Porte de sortie de la zone (h) : chaque joueur vivant qui reste immobile au sol devant la porte
/// (X >= 0x1e0, à moins de 0x20 en profondeur), la touche 0 enfoncée (octet A5+numéro), pendant
/// plus de 8 images est « entré » : il est mis dans l'état 0x2002006 et un effet (EFFCT 0x2f) est créé
/// pour le premier. Quand tous les joueurs vivants (ou morts qui restent) sont entrés, la porte
/// passe à l'état 4 : changement de niveau (`A5+0x34`) ou, à la dernière zone, fin de partie.
/// Bits de `+0x17` : joueurs entrés ; de `+0x1a` : joueurs déjà comptés ; `+0x16` : leur nombre ;
/// `+0x42..+0x45` : compteurs d'attente de chaque joueur ; `A5+0x52` : porte active.
mod exit_gate {
    use super::*;

    const GATE_LOCK: u32 = 0x52;
    const SCENE_HELPER: u32 = 0xcc18e;
    const SCENE_END: u32 = 0xcc1c2;
    const ALLOC_EFFCT: u32 = 0x2318;
    const PLAY_SCRIPT: u32 = 0x1324;
    /// Script de son joué en quittant la zone.
    const LEAVE_SCRIPT: u32 = 0xd6f0e;
    const EFFECT_TYPE: u8 = 0x2f;
    const ENTERED: u32 = 0x0200_2006;
    const WAITING_AT_DOOR: u32 = 0x0200_2010;
    const WAIT_FRAMES: i8 = 8;
    const DOOR_X: i16 = 0x1e0;
    const LAST_STAGE: u8 = 0x1f;
    const BONUS_STAGE: u8 = 0x1c;
    const END_STAGE: u8 = 0x20;
    const BONUS_STAGES: u8 = 4;

    pub fn run(x: &mut Cx) {
        x.call(SCENE_HELPER);
        let lock = x.a5_u16(GATE_LOCK);
        if lock != 0 && lock != x.o.0 as u16 {
            return;
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                for off in [0x42, 0x43, 0x44, 0x45, 0x16, 0x17] {
                    x.set8(off, 0);
                }
                let door_y = if x.u8(0x3) == 0 { 0x160 } else { 0x114 };
                x.set16(0x10, door_y);
            }
            2 => watch(x),
            4 => leave(x),
            _ => x.call(sub::FREE_SET),
        }
    }

    /// Un joueur est-il en position d'entrer ? Compte les images consécutives (octet `counter`).
    fn at_door(x: &mut Cx, p: Obj, input: u8, counter: u32) {
        let busy = p.u8(x.b, 0x66).wrapping_add(p.u8(x.b, 0x15c)).wrapping_add(p.u8(x.b, 0x8a));
        let state = p.u32(x.b, pl::STATE) & 0xffff_ff00;
        let standing = matches!(state, 0x0200_0000 | 0x0200_0600 | 0x0200_0400);
        let y = p.u16(x.b, pl::Y).wrapping_sub(x.u16(0x10)).wrapping_add(0x20);
        if busy == 0
            && input & 1 != 0
            && p.u8(x.b, pl::STATE) == 2
            && standing
            && p.u16(x.b, pl::Z) == 0
            && (p.u16(x.b, pl::X) as i16) >= DOOR_X
            && y <= 0x40
        {
            x.add8(counter, 1);
        } else {
            x.set8(counter, 0);
        }
    }

    fn watch(x: &mut Cx) {
        for (i, p) in players().enumerate() {
            let bit = 1u8 << i;
            let counter = 0x42 + i as u32;
            let entered = x.u8(0x17) & bit != 0;
            let mut counted_check = entered;
            if !entered {
                if p.u8(x.b, pl::ACTIVE) != 1 {
                    continue;
                }
                if (x.u8(counter) as i8) < 0 {
                    counted_check = true;
                } else {
                    let input = x.a5_u8(i as u32);
                    at_door(x, p, input, counter);
                    if (x.u8(counter) as i8) > WAIT_FRAMES {
                        return enter(x, p, bit);
                    }
                    continue;
                }
            }
            if counted_check && x.u8(0x1a) & bit == 0 && p.u32(x.b, pl::STATE) == WAITING_AT_DOOR {
                x.add8(0x16, 1);
                x.set_bit(0x1a, i as u8, true);
            }
        }
        // tous les joueurs concernés sont-ils entrés ?
        let mut needed = x.a5_u8(0x35);
        if needed == 0 {
            return;
        }
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) != 0 && p.u16(x.b, pl::HEALTH) == 0 && p.u16(x.b, pl::STATE) != 0x402 {
                needed = needed.wrapping_add(1);
            }
        }
        if needed as i8 > x.u8(0x16) as i8 {
            return;
        }
        x.call(SCENE_END);
        x.set_a5_u16(GATE_LOCK, 0);
        x.set16(0x4, 0x400);
    }

    /// Le joueur `p` (numéro `bit`) entre : il est mis à l'état « entré ».
    fn enter(x: &mut Cx, p: Obj, bit: u8) {
        if p.u8(x.b, pl::ACTIVE) == 0 || p.u16(x.b, pl::HEALTH) == 0 {
            return;
        }
        let v = x.u8(0x17) | bit;
        x.set8(0x17, v);
        if x.a5_u16(GATE_LOCK) == 0 {
            x.call(ALLOC_EFFCT);
            if x.c.sr & m68k::SR_C == 0 {
                let effect = Obj(x.c.a[1]);
                effect.set_u8(x.b, 0, 1);
                effect.set_u8(x.b, 2, EFFECT_TYPE);
                effect.set_u8(x.b, 0x2f, 0);
                effect.set_u8(x.b, 3, 0xff);
                let v = x.u16(0x10);
                effect.set_u16(x.b, 0x10, v);
            }
        }
        x.set_a5_u16(GATE_LOCK, x.o.0 as u16);
        p.set_u8(x.b, 0, 2);
        p.set_u32(x.b, pl::STATE, ENTERED);
        p.set_u8(x.b, 0x10f, 1);
    }

    /// État 4 : rend la main aux joueurs, annonce la sortie et passe au niveau suivant.
    fn leave(x: &mut Cx) {
        x.set_a5_u8(0x50, 0);
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) != 0 && p.u16(x.b, pl::HEALTH) != 0 {
                let v = p.u8(x.b, 1) & !2;
                p.set_u8(x.b, 1, v);
            }
        }
        x.set_a5_u32(0x2c, 0x0800_0000);
        x.c.a[4] = LEAVE_SCRIPT;
        x.call(PLAY_SCRIPT);
        let stage = x.a5_u8(0x34);
        if stage != LAST_STAGE {
            x.set_a5_u8(0x34, stage + 1);
            if x.u8(0x2f) != 0 {
                bonus_stage(x);
            }
        } else if x.u8(0x2f) == 0 {
            x.set_a5_u8(0x42, 1);
        } else {
            bonus_stage(x);
        }
        x.call(sub::FREE_SET);
    }

    /// Zone bonus : retour au niveau 0x1c, quatre fois au plus, puis fin de partie.
    fn bonus_stage(x: &mut Cx) {
        x.set_a5_u8(0x34, BONUS_STAGE);
        let n = x.a5_u8(0x4e).wrapping_add(1);
        x.set_a5_u8(0x4e, n);
        if n as i8 >= BONUS_STAGES as i8 {
            x.set_a5_u8(0x34, END_STAGE);
            x.set_a5_u8(0x4e, 0);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 25 (0xd801e) : volée de projectiles quand la caméra recule (h)
// ---------------------------------------------------------------------------------------------

/// Quand la partie est en jeu, que la caméra se trouve dans la moitié gauche du niveau (X <= 0x180)
/// et qu'elle a reculé depuis l'image précédente (le joueur fait demi-tour), l'objet lance trois
/// tirs ennemis (ESHL de type 0x12) à droite de l'écran, à la profondeur d'un joueur tiré au hasard
/// (décalages -0x18, +0x18, 0). Il attend 0x60 images (`+8`) entre deux volées.
mod rearguard {
    use super::*;

    const ESHL_TYPE: u8 = 0x12;
    const COOLDOWN: u8 = 0x60;
    const FIRST_X: u16 = 0x1a0;
    const SPACING_X: u16 = 0xc;
    const HEIGHT: u16 = 0x20;
    /// Décalages de profondeur des trois tirs (mots).
    const OFFSETS: u32 = 0xd80fe;
    /// A5+0x53a : cinématique en cours (h).
    const CUTSCENE: u32 = 0x53a;
    const PLAYING: u8 = 0xc;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                let v = x.a5_u16(ram::CAMERA_X);
                x.set16(0xa, v);
                watch(x);
            }
            2 => watch(x),
            _ => x.call(sub::FREE_SET),
        }
    }

    fn remember_camera(x: &mut Cx) {
        let v = x.a5_u16(ram::CAMERA_X);
        x.set16(0xa, v);
    }

    fn watch(x: &mut Cx) {
        if x.u8(0x8) != 0 {
            x.add8(0x8, 0xff);
            return remember_camera(x);
        }
        if x.a5_u8(0x50) != 0 {
            return x.add8(0x4, 2);
        }
        if x.a5_u8(0x2c) != PLAYING || x.a5_u8(0x35) == 0 || x.a5_u8(0x3d) != 0 {
            return remember_camera(x);
        }
        if x.a5_u8(CUTSCENE) != 0 {
            return;
        }
        let camera = x.a5_u16(ram::CAMERA_X) as i16;
        if camera > 0x180 || camera >= x.u16(0xa) as i16 {
            return remember_camera(x);
        }
        // un joueur présent, tiré au hasard
        let target = loop {
            x.call(RANDOM);
            let p = Obj(PLAYERS + (x.c.d[0] & 3) * PLAYER_SIZE);
            if p.u8(x.b, pl::ACTIVE) != 0 {
                break p;
            }
        };
        x.set8(0x8, COOLDOWN);
        let mut shot_x = FIRST_X;
        for i in (0..3u32).rev() {
            x.call(sub::ALLOC_ESHL);
            if x.c.sr & m68k::SR_C != 0 {
                return remember_camera(x);
            }
            let shot = Obj(x.c.a[1]);
            shot.set_u8(x.b, 0, 1);
            shot.set_u8(x.b, 2, ESHL_TYPE);
            shot.set_u8(x.b, 0x2c, 0);
            let v = x.a5_u16(ram::CAMERA_X).wrapping_add(shot_x);
            shot.set_u16(x.b, 0x8, v);
            shot_x += SPACING_X;
            shot.set_u16(x.b, 0xc, HEIGHT);
            let y = target.u16(x.b, pl::Y).wrapping_add(x.rom16(OFFSETS + 2 * i));
            shot.set_u16(x.b, 0x10, y);
        }
        remember_camera(x);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 26 (0xd810a) : chasseur de joueurs immobiles (h)
// ---------------------------------------------------------------------------------------------

/// Pénalise les joueurs qui restent immobiles (h) : pour chaque joueur vivant dont la position
/// (X + sa partie fractionnaire haute) ne change pas pendant `+0x1c` images, il crée des poursuivants
/// (routine 0xa44bc, trois ESHL de type 0x33), dans la limite d'un nombre d'actifs qui dépend du
/// nombre de joueurs. Le délai `+0x1c` est tiré dans une table de la ROM selon le rang. L'ordre
/// d'examen des quatre joueurs est tiré au hasard parmi quatre permutations.
mod idle_hunter {
    use super::*;

    /// Délais d'immobilité : 16 octets par tranche de 4 rangs (espace programme).
    const IDLE_FRAMES: u32 = 0xd8186;
    /// Nombre maximal de poursuivants selon le nombre de joueurs (mots).
    const MAX_ACTIVE: u32 = 0xd8448;
    const SPAWN_CHASERS: u32 = 0xa44bc;
    const CUTSCENE: u32 = 0x53a;
    const PLAYING: u8 = 0xc;
    const WARMUP: u16 = 0x168;
    /// Les quatre ordres d'examen.
    const ORDERS: [[u32; 4]; 4] = [[0, 1, 2, 3], [2, 3, 1, 0], [1, 0, 3, 2], [3, 2, 0, 1]];

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => init(x),
            2 => step(x),
            _ => x.call(sub::FREE_SET),
        }
    }

    /// Position de référence d'un joueur : X + octets suivants (`+8` et `+0x10` du joueur).
    fn mark(x: &mut Cx, i: u32) -> u16 {
        let base = 0x5de + PLAYER_SIZE * i;
        x.a5_u16(base + 8).wrapping_add(x.a5_u16(base + 0x10))
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        for i in 0..4 {
            let v = mark(x, i);
            x.set16(0x10 + 2 * i, v);
            x.set8(0x18 + i, 0);
        }
        let tier = (x.a5_u16(0x7f92) >> 2) << 4;
        x.call(RANDOM);
        let index = (x.c.d[0] as u16 & 0xf).wrapping_add(tier);
        let frames = x.rom8(IDLE_FRAMES + index as u32);
        x.set8(0x1c, frames);
        x.set16(0x1e, WARMUP);
    }

    fn step(x: &mut Cx) {
        if x.u8(0x5) == 0 {
            if x.a5_u8(0x2c) == PLAYING {
                let n = x.u16(0x1e).wrapping_sub(1);
                x.set16(0x1e, n);
                if n == 0 {
                    x.add8(0x5, 2);
                }
            }
            return;
        }
        if x.a5_u8(0x50) != 0 {
            return x.set8(0x4, 4);
        }
        if x.a5_u8(0x2c) != PLAYING
            || x.a5_u8(0x35) == 0
            || x.a5_u8(0x3d) != 0
            || x.a5_u8(CUTSCENE) != 0
            || (x.a5_u16(ram::CAMERA_X) as i16) > 0x180
        {
            return;
        }
        x.call(RANDOM);
        for i in ORDERS[(x.c.d[0] & 3) as usize] {
            check(x, i);
        }
    }

    /// Examen d'un joueur : immobile assez longtemps, il attire des poursuivants.
    fn check(x: &mut Cx, i: u32) {
        let p = Obj(PLAYERS + PLAYER_SIZE * i);
        if p.u8(x.b, pl::ACTIVE) & 1 == 0 || p.u16(x.b, pl::HEALTH) == 0 || x.u16(0x8 + 2 * i) != 0 {
            return;
        }
        let here = mark(x, i);
        if here == x.u16(0x10 + 2 * i) {
            let n = x.u8(0x18 + i).wrapping_add(1);
            x.set8(0x18 + i, n);
            if x.u8(0x1c) > n {
                return;
            }
            if let Some(first) = chase(x, p, 2 * i) {
                x.set16(0x8 + 2 * i, first);
            }
        }
        x.set16(0x10 + 2 * i, here);
        x.set8(0x18 + i, 0);
    }

    /// Crée des poursuivants pour le joueur `p` si la limite n'est pas atteinte ; rend l'adresse
    /// du dernier créé.
    fn chase(x: &mut Cx, p: Obj, slot: u32) -> Option<u16> {
        let active = [0x8, 0xa, 0xc, 0xe].into_iter().filter(|&o| x.u16(o) != 0).count() as i16;
        let players = x.a5_u8(0x35) as u32;
        let allowed = x.rom16(MAX_ACTIVE + 2 * players) as i16;
        if active >= allowed {
            return None;
        }
        x.c.a[2] = p.0;
        x.c.d[0] = slot;
        x.call(SPAWN_CHASERS);
        if x.c.cond(7) {
            return None;
        }
        Some(x.c.a[1] as u16)
    }
}

// ---------------------------------------------------------------------------------------------
// Types 27 et 29 : portes de passage (h)
// ---------------------------------------------------------------------------------------------

/// Pools d'objets suspendus ou libérés lors d'un passage de porte : début, taille d'un objet,
/// nombre d'emplacements.
struct Pool {
    base: u32,
    size: u32,
    count: u32,
}

const SET_POOL: Pool = Pool { base: A5 + 0x4a9e, size: 0x50, count: 32 };
const SOBJ_POOL: Pool = Pool { base: A5 + 0x795e, size: 0x50, count: 16 };
const EFFCT_POOL: Pool = Pool { base: A5 + 0x549e, size: 0x60, count: 38 };
const BOX_POOL: Pool = Pool { base: A5 + 0x6ede, size: 0x80, count: 21 };
const ITEM_POOL: Pool = Pool { base: A5 + 0x62de, size: 0x60, count: 32 };
const ENEMY_POOL: Pool = Pool { base: A5 + 0x299e, size: 0xc0, count: 22 };
const BOSS_POOL: Pool = Pool { base: A5 + 0x3a1e, size: 0xc0, count: 3 };
const ESHL_POOL: Pool = Pool { base: A5 + 0x3c5e, size: 0x60, count: 38 };

/// Champs d'un objet de la boucle de dispatch communs à tous les pools.
mod obj {
    pub const SUSPEND: u32 = 0x40;
    pub const SUSPEND_PHASE: u32 = 0x41;
    pub const STATE: u32 = 0x4;
}

/// Exécute `run` du gestionnaire avec un autre objet comme objet courant (appel récursif
/// `bsr` de l'original, qui met A0 sur l'autre objet).
fn run_on(x: &mut Cx, other: Obj, run: fn(&mut Cx)) {
    let me = x.o;
    let d6 = x.c.d[6];
    let mut y = Cx { c: &mut *x.c, b: &mut *x.b, o: other };
    y.c.a[0] = other.0;
    run(&mut y);
    x.c.a[0] = me.0;
    x.c.d[6] = d6;
}

/// Appel d'une routine du jeu qui peut modifier D7 : D7 est conservé (movem de l'original).
fn call_keeping(x: &mut Cx, addrs: &[u32]) {
    let d7 = x.c.d[7];
    for a in addrs {
        x.call(*a);
    }
    x.c.d[7] = d7;
    x.c.a[0] = x.o.0;
}

/// Suspend les objets d'un pool (compteur `+0x40` + 1, phase remise à zéro).
fn suspend(x: &mut Cx, pool: &Pool) {
    for i in 0..pool.count {
        let o = Obj(pool.base + i * pool.size);
        if o.u8(x.b, 0) != 0 {
            let v = o.u8(x.b, obj::SUSPEND).wrapping_add(1);
            o.set_u8(x.b, obj::SUSPEND, v);
            o.set_u8(x.b, obj::SUSPEND_PHASE, 0);
        }
    }
}

/// Réveille les objets d'un pool ; ceux dont le compteur de suspension devient négatif sont
/// supprimés (état 6).
fn resume(x: &mut Cx, pool: &Pool) {
    resume_with(x, pool, |_, _| ());
}

fn resume_with(x: &mut Cx, pool: &Pool, mut when_awake: impl FnMut(&mut Cx, Obj)) {
    for i in 0..pool.count {
        let o = Obj(pool.base + i * pool.size);
        if o.u8(x.b, 0) == 0 {
            continue;
        }
        let n = o.u8(x.b, obj::SUSPEND).wrapping_sub(1);
        o.set_u8(x.b, obj::SUSPEND, n);
        if (n as i8) < 0 {
            o.set_u8(x.b, obj::STATE, 6);
            o.set_u8(x.b, obj::SUSPEND, 0);
            o.set_u8(x.b, obj::SUSPEND_PHASE, 0);
        } else {
            when_awake(x, o);
        }
    }
}

/// Variables (A5-0x55d0 et A5-0x55cc) mémorisées avec la caméra.
const SCROLL_STATE: u32 = 0xff_2a30;
const STAGE_FLAG: u32 = 0xff_2a34;

/// Mémorise dans la porte l'état de la caméra et du défilement (champs `+0x30..+0x4a`).
fn save_camera(x: &mut Cx) {
    for (src, dst) in [(0x506, 0x30), (0x50a, 0x34), (0x556, 0x38), (0x55a, 0x3c)] {
        let v = x.a5_u32(src);
        x.set32(dst, v);
    }
    let v = rd32(x.b, SCROLL_STATE);
    x.set32(0x4a, v);
    let v = x.a5_u16(0x57c);
    x.set16(0x18, v);
    let v = x.a5_u16(0x51e);
    x.set16(0x14, v);
    let v = rd8(x.b, STAGE_FLAG);
    x.set8(0x49, v);
    let v = x.a5_u16(0x7fc4);
    x.set16(0x1c, v);
}

/// Rétablit la caméra et le défilement mémorisés par `save_camera`.
fn restore_camera(x: &mut Cx) {
    for (dst, src) in [(0x506, 0x30), (0x50a, 0x34), (0x556, 0x38), (0x55a, 0x3c)] {
        let v = x.u32(src);
        x.set_a5_u32(dst, v);
    }
    let v = x.a5_u32(0x506);
    x.set_a5_u32(0x50e, v);
    let v = x.a5_u32(0x50a);
    x.set_a5_u32(0x512, v);
    let v = x.a5_u16(0x556);
    x.set_a5_u16(0x55e, v);
    let v = x.a5_u16(0x55a);
    x.set_a5_u16(0x562, v);
    let v = x.a5_u16(0x50e) & !0x1f;
    x.set_a5_u16(0x50e, v);
    let v = x.a5_u16(0x55e) & !0x1f;
    x.set_a5_u16(0x55e, v);
    let v = x.a5_u16(0x512).wrapping_sub(0x160);
    x.set_a5_u16(0x512, v);
    let v = x.a5_u16(0x562).wrapping_sub(0x160);
    x.set_a5_u16(0x562, v);
    let v = x.u32(0x4a);
    wr32(x.b, SCROLL_STATE, v);
    let v = x.u16(0x18);
    x.set_a5_u16(0x57c, v);
    let v = x.u16(0x14);
    x.set_a5_u16(0x51e, v);
    let v = x.u8(0x49);
    wr8(x.b, STAGE_FLAG, v);
}

/// Remet les joueurs en jeu après un passage (état, drapeaux, délais) ; ceux qui étaient
/// au-delà de l'état 2 sont marqués morts (0x402).
fn reset_players_after_exit(x: &mut Cx) {
    for p in players() {
        if p.u8(x.b, pl::ACTIVE) == 0 || p.u16(x.b, pl::HEALTH) == 0 {
            continue;
        }
        p.set_u8(x.b, pl::ACTIVE, 1);
        let before = p.u8(x.b, pl::STATE);
        p.set_u32(x.b, pl::STATE, 0);
        let v = p.u8(x.b, 1) & !2;
        p.set_u8(x.b, 1, v);
        for off in [0x66, 0x15c, 0x8a, 0x10f] {
            p.set_u8(x.b, off, 0);
        }
        if (before as i8) > 2 {
            let v = p.u8(x.b, 1) | 2;
            p.set_u8(x.b, 1, v);
            p.set_u16(x.b, pl::STATE, 0x402);
        }
    }
}

/// Remet les joueurs en marche à l'arrivée d'une porte de sortie (état 1, délai 0x20).
fn release_players(x: &mut Cx) {
    for p in players() {
        if p.u8(x.b, pl::ACTIVE) == 0 {
            continue;
        }
        p.set_u8(x.b, pl::ACTIVE, 1);
        let v = p.u8(x.b, 1) & !2;
        p.set_u8(x.b, 1, v);
        for off in [0x66, 0x15c, 0x8a, 0x10f] {
            p.set_u8(x.b, off, 0);
        }
        p.set_u8(x.b, 0x7e, 0x20);
    }
}

/// Le joueur `p` est-il immobile devant la porte ? Compte les images (octet `counter`).
/// Conditions : rien en cours (`+0x66`, `+0x15c`, `+0x8a`), la touche `bit` de `input`, l'état 2
/// (marche ou arrêt), au sol, à moins de 0x10 en X et de 8 en profondeur (sauf `+0x1b`).
fn at_gate(x: &mut Cx, p: Obj, input: u32, counter: u32) {
    let busy = p.u8(x.b, 0x66).wrapping_add(p.u8(x.b, 0x15c)).wrapping_add(p.u8(x.b, 0x8a));
    let bit = x.u8(0x46) as u32 & 31;
    let state = p.u32(x.b, pl::STATE) & 0xffff_ff00;
    let dx = p.u16(x.b, pl::X).wrapping_sub(x.u16(0x8)).wrapping_add(0x10);
    let dy = p.u16(x.b, pl::Y).wrapping_sub(x.u16(0x10)).wrapping_add(8);
    let aligned = dx <= 0x20 && (x.u8(0x1b) != 0 || dy <= 0x10);
    if busy == 0
        && input >> bit & 1 != 0
        && p.u8(x.b, pl::STATE) == 2
        && matches!(state, 0x0200_0000 | 0x0200_0400)
        && p.u16(x.b, pl::Z) == 0
        && aligned
    {
        x.add8(counter, 1);
    } else {
        x.set8(counter, 0);
    }
}

/// Porte de passage entre zones (type 27, h) : comme `exit_gate`, mais le passage mène à une autre
/// zone ou en revient (suspension des objets de la zone courante dans `+0x40`, caméra sauvegardée
/// dans `+0x30..+0x4a`), avec une porte, de variante `+3` : 0 entrée dans la zone cachée, 1
/// retour. Les paramètres de chaque variante (position, touche `+0x46`, mode `+0x48`, niveau
/// d'arrivée `+0x4f`, `+0x1b`) viennent de lignes de 16 octets de la table 0xd84e8.
mod door_a {
    use super::*;

    const GATE_LOCK: u32 = 0x52;
    const HELPER: u32 = 0xcc18e;
    const PARAMS: u32 = 0xd84e8;
    const ENTER_KINDS: u32 = 0xd8824;
    const PLACEMENT_SIDE: u32 = 0xd896e;
    const PLACEMENT: u32 = 0xd89be;
    const PLAY_SCRIPT: u32 = 0x1324;
    const SCRIPT: u32 = 0xd86e8;
    const CHECK_FRAMES: i8 = 0x18;
    const SPAWN_SET: u32 = 0xc8312;
    const TRANSITION: u32 = 0xe0a2e;
    const AFTER: u32 = 0xdd12c;
    const ON_LEAVE: u32 = 0x20de6;

    pub fn run(x: &mut Cx) {
        x.call(HELPER);
        if x.u8(0x40) != 0 || x.a5_u8(0x3d) != 0 {
            return;
        }
        let lock = x.a5_u16(GATE_LOCK);
        if lock != 0 && lock != x.o.0 as u16 {
            return;
        }
        match x.u8(0x4) {
            0 => init(x),
            2 => watch(x),
            4 => arrive(x),
            _ => x.call(sub::FREE_SET),
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        for off in [0x42, 0x43, 0x44, 0x45, 0x16, 0x17] {
            x.set8(off, 0);
        }
        x.set_a5_u8(0x4e, 0);
        let row = PARAMS + ((x.u8(0x3) as u32) << 4);
        let v = x.rom16(row);
        x.set16(0x8, v);
        let v = x.rom16(row + 2);
        x.set16(0xc, v);
        let v = x.rom16(row + 4);
        x.set16(0x10, v);
        let v = x.rom8(row + 6);
        x.set8(0x48, v);
        let v = x.rom8(row + 7);
        x.set8(0x46, v);
        let v = x.rom8(row + 8);
        x.set8(0x4f, v);
        let v = x.rom8(row + 9);
        x.set8(0x1b, v);
    }

    fn watch(x: &mut Cx) {
        if x.a5_u8(0x59) == 0 {
            if x.u8(0x48) == 0 {
                return;
            }
            let v = x.a5_u8(0x35);
            x.set8(0x16, v);
            x.set_a5_u16(GATE_LOCK, x.o.0 as u16);
        }
        for (i, p) in players().enumerate() {
            let bit = 1u8 << i;
            let counter = 0x42 + i as u32;
            let entered = x.u8(0x17) & bit != 0;
            let mut counted_check = entered;
            if !entered {
                if p.u8(x.b, pl::ACTIVE) != 1 {
                    continue;
                }
                if (x.u8(counter) as i8) < 0 {
                    counted_check = true;
                } else {
                    let input = p.u8(x.b, 0x6c) as u32;
                    at_gate(x, p, input, counter);
                    if (x.u8(counter) as i8) > CHECK_FRAMES {
                        return enter(x, p, bit);
                    }
                    continue;
                }
            }
            if counted_check && x.u8(0x1a) & bit == 0 && p.u32(x.b, pl::STATE) == 0x0200_2010 {
                x.add8(0x16, 1);
                x.set_bit(0x1a, i as u8, true);
            }
        }
        let mut needed = x.a5_u8(0x35);
        if needed == 0 {
            return;
        }
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) != 0 && p.u16(x.b, pl::HEALTH) == 0 && p.u16(x.b, pl::STATE) != 0x402 {
                needed = needed.wrapping_add(1);
            }
        }
        if needed as i8 > x.u8(0x16) as i8 {
            return;
        }
        leave(x);
    }

    /// Un joueur franchit la porte : soit une apparition (variante 0 sans `A5+0x4e`), soit il est
    /// mis à l'état d'entrée.
    fn enter(x: &mut Cx, p: Obj, bit: u8) {
        if x.u8(0x3) == 0 && x.a5_u8(0x4e) == 0 {
            call_with(x, SPAWN_SET, 0x8b);
            let spawned = Obj(x.c.a[1]);
            spawned.set_u8(x.b, 0x42, 0xf);
            let v = p.u8(x.b, 3);
            spawned.set_u8(x.b, 0x43, v);
            let i = bit.trailing_zeros();
            x.set8(0x42 + i, 0);
            return;
        }
        let v = x.u8(0x17) | bit;
        x.set8(0x17, v);
        x.set_a5_u16(GATE_LOCK, x.o.0 as u16);
        let kind = x.u8(0x46) as u32;
        let state = x.rom16(ENTER_KINDS + 4 * kind) as u32;
        let state = state << 16 | x.rom16(ENTER_KINDS + 4 * kind + 2) as u32;
        p.set_u32(x.b, pl::STATE, state);
        p.set_u8(x.b, 0, 2);
        p.set_u8(x.b, 0x10f, 1);
    }

    /// Tous les joueurs concernés sont à la porte : le passage commence.
    fn leave(x: &mut Cx) {
        x.c.d[1] = (x.c.d[1] & 0xffff_0000) | 0xff;
        call_with(x, TRANSITION, 4);
        x.set_a5_u8(0x51, 1);
        let level = x.u8(0x4f);
        x.set_a5_u8(0x34, level);
        if x.u8(0x3) == 0 {
            x.set_a5_u8(0x42, 1);
            return;
        }
        x.c.a[4] = SCRIPT;
        x.call(PLAY_SCRIPT);
        x.set_a5_u32(0x5c6, 0);
        x.call(AFTER);
        x.set_a5_u16(GATE_LOCK, 0);
        match x.u8(0x48) as i8 {
            1 => exit_zone(x),
            mode if mode > 1 => {}
            _ => {
                save_camera(x);
                suspend(x, &SET_POOL);
                suspend(x, &EFFCT_POOL);
                suspend(x, &SOBJ_POOL);
                x.set16(0x4, 0x400);
                enter_zone(x);
            }
        }
    }

    /// Chargement de la zone cachée (état 0x400 de la porte).
    fn enter_zone(x: &mut Cx) {
        call_keeping(x, &[0x33aa6]);
        x.call(0xf2e56);
        call_keeping(x, &[0x1a360, 0x1a678, 0x1a8ac]);
        x.set_a5_u8(0x48e, 0xf);
        x.call(0x1686);
        x.set_a5_u16(GATE_LOCK, 0);
        reset_players_after_exit(x);
        x.call(ON_LEAVE);
    }

    /// Retour à la zone d'origine : les joueurs sont libérés et tous les objets de la zone cachée
    /// supprimés ; chaque porte de type 0x1b de la zone retrouvée est rejouée.
    fn exit_zone(x: &mut Cx) {
        x.set_a5_u8(0x4e, 1);
        release_players(x);
        x.set8(0x4, 6);
        resume_with(x, &SET_POOL, |x, o| {
            if o.u8(x.b, 2) == 0x1b {
                run_on(x, o, run);
            }
        });
        resume(x, &EFFCT_POOL);
        resume(x, &BOX_POOL);
        resume(x, &ITEM_POOL);
        resume(x, &SOBJ_POOL);
    }

    /// État 4 : arrivée dans la zone cachée (chargement de la nouvelle zone).
    fn arrive(x: &mut Cx) {
        call_keeping(x, &[0x33b24]);
        x.call(0xf2e56);
        call_keeping(x, &[0x1a360, 0x1a678, 0x1a8ac]);
        x.set_a5_u8(0x48e, 0xf);
        x.call(0x1686);
        x.set_a5_u16(GATE_LOCK, 0);
        x.set_a5_u8(0x51, 0);
        restore_camera(x);
        let d4 = x.u16(0x1c);
        x.c.d[4] = (x.c.d[4] & 0xffff_0000) | d4 as u32;
        x.c.d[1] = (x.c.d[1] & 0xffff_0000) | 0x90;
        call_with(x, TRANSITION, 0);
        let camera = if x.a5_u8(0x36) != 0 { 0x54e } else { 0x4fe };
        x.c.a[4] = A5 + camera;
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) != 0 {
                place(x, p);
            }
        }
        x.set8(0x4, 6);
    }

    /// Place un joueur à l'arrivée (position relative à la porte, table 0xd89be selon le côté).
    fn place(x: &mut Cx, p: Obj) {
        if p.u8(x.b, 4) != 2 {
            let v = p.u8(x.b, 1) | 2;
            p.set_u8(x.b, 1, v);
            let d6 = x.c.d[6];
            x.c.a[0] = p.0;
            crate::call(x.c, x.b, 0x3668);
            x.c.a[0] = x.o.0;
            x.c.d[6] = d6;
            return;
        }
        p.set_u32(x.b, pl::STATE, 0x0200_0000);
        let v = p.u8(x.b, 1) & !2;
        p.set_u8(x.b, 1, v);
        p.set_u8(x.b, pl::ACTIVE, 1);
        let variant = x.u8(0x3) as u32;
        let side = x.rom8(PLACEMENT_SIDE + variant) as u32;
        let at = (side << 4) + 4 * p.u8(x.b, 2) as u32;
        let dx = x.rom16(PLACEMENT + at);
        let dy = x.rom16(PLACEMENT + 2 + at);
        let v = x.u16(0x8);
        p.set_u16(x.b, pl::X, v);
        let v = x.u16(0x10);
        p.set_u16(x.b, pl::Y, v);
        p.set_u16(x.b, pl::Z, 0);
        let v = p.u16(x.b, pl::X).wrapping_add(dx);
        p.set_u16(x.b, pl::X, v);
        let v = p.u16(x.b, pl::Y).wrapping_add(dy);
        p.set_u16(x.b, pl::Y, v);
    }
}

/// Porte de zone en trois temps (type 28, h) : quand la caméra atteint X = 0x1c0 elle bloque le
/// défilement (`A5+0x51e` = 4), crée un événement SET (type 0x3f) et attend qu'il se termine
/// (`+0x22` de cet événement) ; elle lance alors l'invitation (SOBJ 11) et un délai de 0x78
/// images, puis libère le défilement et lance les routines de fin de zone (0xa462c, 0xe55a2,
/// 0xe13b0).
mod zone_end {
    use super::*;

    const EVENT_TYPE: u32 = 0x3f;
    const SPAWN_SET: u32 = 0xc8312;
    const SPAWN_INVITE: u32 = 0xdfa4a;
    const LOCK_X: u16 = 0x1c0;
    const SCROLL_LOCK: u32 = 0x51e;
    const EVENT_FLAG: u32 = 0x43;
    const DELAY: u16 = 0x78;
    /// Les objets SET de ce type voient leur orientation (`+0x2c` bit 5) inversée à la fin.
    const FLIPPED_TYPE: u8 = 0x15;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                sub_state(x);
            }
            2 => sub_state(x),
            _ => x.call(sub::FREE_SET),
        }
    }

    fn sub_state(x: &mut Cx) {
        match x.u8(0x5) {
            0 => {
                if x.a5_u8(0x35) == 0 || x.a5_u16(ram::CAMERA_X) < LOCK_X {
                    return;
                }
                x.add8(0x5, 2);
                x.set_a5_u16(SCROLL_LOCK, 4);
                x.set_a5_u8(EVENT_FLAG, 0x1f);
                call_with(x, SPAWN_SET, EVENT_TYPE);
                let event = x.c.a[1] as u16;
                x.set16(0x10, event);
            }
            2 => {
                let event = x.linked(0x10);
                if event.u8(x.b, 0x22) == 0 {
                    return;
                }
                if event.u8(x.b, 0x23) == 0 {
                    x.add8(0x5, 2);
                    let v = x.a5_u16(ram::CAMERA_X);
                    x.set_a5_u16(0x52c, v);
                    x.set16(0x14, DELAY);
                    x.c.d[1] = 0;
                    call_with(x, SPAWN_INVITE, 0);
                    return;
                }
                x.add8(0x4, 2);
                x.set_a5_u16(SCROLL_LOCK, 0);
                x.set_a5_u8(EVENT_FLAG, 0xff);
                let v = x.a5_u16(ram::CAMERA_X);
                x.set_a5_u16(0x532, v);
                // 33 emplacements : le dernier est le premier objet EFFCT (pool contigu)
                for i in 0..33u32 {
                    let o = Obj(SET_POOL.base + i * SET_POOL.size);
                    if o.u8(x.b, 0) != 0 && o.u8(x.b, 2) == FLIPPED_TYPE {
                        let v = o.u8(x.b, 0x2c) ^ 0x20;
                        o.set_u8(x.b, 0x2c, v);
                    }
                }
            }
            4 => {
                let n = x.u16(0x14).wrapping_sub(1);
                x.set16(0x14, n);
                if n != 0 {
                    return;
                }
                x.add8(0x4, 2);
                x.set_a5_u16(SCROLL_LOCK, 0);
                x.call(0xa462c);
                x.call(0xe55a2);
                x.call(0xe13b0);
            }
            _ => {}
        }
    }
}

/// Porte de passage (type 29, h) : variante de la porte 27 qui décale tout le décor (`SET`, `SOBJ`,
/// `EFFCT`, `BOX`, `ITEM`, `BOSS`, `ESHL`) au lieu de trois pools, et replace les joueurs sur un
/// repère fixe (table 0xd9052). Mêmes champs (`+0x42..+0x45` compteurs, `+0x16` nombre entré, `+0x17`
/// bits entrés, `+0x30..` caméra sauvegardée).
mod door_b {
    use super::*;

    const GATE_LOCK: u32 = 0x52;
    const HELPER: u32 = 0xcc18e;
    const PARAMS: u32 = 0xd8e5e;
    const PLACEMENT: u32 = 0xd9052;
    const SOUND_FLAG: u32 = 0xff_24e0;
    const NEAR_X: u16 = 0x30;
    const NEAR_Y: u16 = 0x20;

    pub fn run(x: &mut Cx) {
        x.call(HELPER);
        if x.u8(0x40) != 0 || x.a5_u8(0x3d) != 0 {
            return;
        }
        let lock = x.a5_u16(GATE_LOCK);
        if lock != 0 && lock != x.o.0 as u16 {
            return;
        }
        match x.u8(0x4) {
            0 => init(x),
            2 => watch(x),
            4 => arrive(x),
            _ => x.call(sub::FREE_SET),
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        for off in [0x42, 0x43, 0x44, 0x45, 0x16, 0x17] {
            x.set8(off, 0);
        }
        x.set_a5_u8(0x50, 0);
        let row = PARAMS + ((x.u8(0x3) as u32) << 4);
        let v = x.rom16(row);
        x.set16(0x8, v);
        let v = x.rom16(row + 2);
        x.set16(0xc, v);
        let v = x.rom16(row + 4);
        x.set16(0x10, v);
        for (off, at) in [(0x48, 6), (0x46, 7), (0x4f, 8), (0x1b, 9)] {
            let v = x.rom8(row + at);
            x.set8(off, v);
        }
    }

    /// Le joueur est à portée de la porte.
    fn near(x: &mut Cx, p: Obj) -> bool {
        let dx = p.u16(x.b, pl::X).wrapping_sub(x.u16(0x8)).wrapping_add(NEAR_X);
        if dx > 2 * NEAR_X {
            return false;
        }
        if x.u8(0x1b) != 0 {
            return true;
        }
        let dy = p.u16(x.b, pl::Y).wrapping_sub(x.u16(0x10)).wrapping_add(NEAR_Y);
        dy <= 2 * NEAR_Y
    }

    fn watch(x: &mut Cx) {
        if !players().any(|p| p.u8(x.b, pl::ACTIVE) == 1 && near(x, p)) {
            return;
        }
        let v = rd8(x.b, SOUND_FLAG).wrapping_add(1);
        wr8(x.b, SOUND_FLAG, v);
        x.set_a5_u16(GATE_LOCK, 0);
        x.set_a5_u8(0x43, 0xff);
        x.set_a5_u16(0x490, 2);
        x.set_a5_u8(0x48e, 0xf);
        // fondu de sortie : cède la main, la tâche reprend en 0xd8f06
        x.call_at(0x1460, 0xd8f06);
        let level = x.u8(0x4f);
        x.set_a5_u8(0x34, level);
        match x.u8(0x48) as i8 {
            1 => exit_zone(x),
            mode if mode > 1 => x.set_a5_u8(0x42, 1),
            _ => {
                x.set_a5_u8(0x4a, 0);
                save_camera(x);
                for pool in [&SET_POOL, &SOBJ_POOL, &EFFCT_POOL, &BOX_POOL, &ITEM_POOL, &BOSS_POOL, &ESHL_POOL] {
                    suspend(x, pool);
                }
                x.set16(0x4, 0x400);
                enter_zone(x);
            }
        }
    }

    fn enter_zone(x: &mut Cx) {
        call_keeping(x, &[0x33aa6]);
        x.call(0xf2e56);
        call_keeping(x, &[0x1a360, 0x1a678, 0x1a8ac]);
        x.set_a5_u8(0x48e, 0xf);
        x.call(0x1686);
        x.call(0xe0c5c);
        x.set_a5_u16(GATE_LOCK, 0);
        reset_players_after_exit(x);
        x.call(0x20de6);
    }

    fn exit_zone(x: &mut Cx) {
        release_players(x);
        x.set8(0x4, 6);
        resume_with(x, &SET_POOL, |x, o| {
            if o.u8(x.b, 2) == 0x1d {
                run_on(x, o, run);
            }
        });
        for pool in [&ENEMY_POOL, &EFFCT_POOL, &BOX_POOL, &ITEM_POOL, &BOSS_POOL, &ESHL_POOL, &SOBJ_POOL] {
            resume(x, pool);
        }
    }

    fn arrive(x: &mut Cx) {
        call_keeping(x, &[0x33b24]);
        x.call(0xf2e56);
        call_keeping(x, &[0x1a360, 0x1a678, 0x1a8ac]);
        x.set_a5_u8(0x48e, 0xf);
        x.call(0x1686);
        x.set_a5_u16(GATE_LOCK, 0);
        x.set_a5_u8(0x4a, 1);
        x.set_a5_u8(0x43, 0xf);
        restore_camera(x);
        let d4 = x.u16(0x1c);
        x.c.d[4] = (x.c.d[4] & 0xffff_0000) | d4 as u32;
        x.call(0xe17a4);
        let camera = if x.a5_u8(0x36) != 0 { 0x54e } else { 0x4fe };
        x.c.a[4] = A5 + camera;
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) != 0 {
                place(x, p);
            }
        }
        x.set8(0x4, 6);
    }

    fn place(x: &mut Cx, p: Obj) {
        if p.u8(x.b, 4) != 2 {
            let v = p.u8(x.b, 1) | 2;
            p.set_u8(x.b, 1, v);
            let d6 = x.c.d[6];
            x.c.a[0] = p.0;
            crate::call(x.c, x.b, 0x3668);
            x.c.a[0] = x.o.0;
            x.c.d[6] = d6;
            return;
        }
        p.set_u32(x.b, pl::STATE, 0x0200_2a00);
        let v = p.u8(x.b, 1) & !2;
        p.set_u8(x.b, 1, v);
        p.set_u8(x.b, pl::ACTIVE, 1);
        let slot = 2 * p.u8(x.b, 2) as u32;
        let px = x.rom16(PLACEMENT + slot);
        p.set_u16(x.b, pl::X, px);
        p.set_u16(x.b, pl::Z, 0xa0);
        let y = x.a5_u16(ram::CAMERA_Y).wrapping_add(0xf0);
        p.set_u16(x.b, pl::Y, y);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 30 (0xd9424) : fin de partie (écran de continuation)
// ---------------------------------------------------------------------------------------------

/// Quand `A5+0x42` (fin de partie) est levé, joue la musique de fin (routine 0xa8ae8, D0 = 7),
/// attend 0x80 images puis lance la tâche de fondu par composante 0x1ccf8 et se libère.
mod game_over {
    use super::*;

    const WAIT_FRAMES: u16 = 0x80;
    const MUSIC: u32 = 0xa8ae8;
    const AFTER_MUSIC: u32 = 0x1ad2a;
    const MESSAGES: u32 = 0x1ccf8;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 | 2 => {
                if x.a5_u8(0x42) == 0 {
                    return;
                }
                call_with(x, MUSIC, 7);
                x.add8(0x4, 2);
                x.set16(0x4c, WAIT_FRAMES);
                x.call(AFTER_MUSIC);
                // l'original enchaîne sur le décompte (0xd9458) dès cette image : 0x80 devient 0x7f
                countdown(x);
            }
            4 => countdown(x),
            _ => x.call(sub::FREE_SET),
        }
    }

    fn countdown(x: &mut Cx) {
        let n = x.u16(0x4c).wrapping_sub(1);
        x.set16(0x4c, n);
        if (n as i16) >= 0 {
            return;
        }
        x.set_a5_u16(0x490, 1);
        x.set_a5_u8(0x48e, 0xd);
        x.set_a5_u8(0x48f, 1);
        call_on_list(x, MESSAGES);
        x.call(sub::FREE_SET);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 31 (0xd9492) : message d'attente dans un niveau long (h)
// ---------------------------------------------------------------------------------------------

/// Chronomètre de niveau (h) : compte les images tant qu'un joueur est en jeu (`+0x42`) et déclenche
/// à 0x258, 0x4b0 et 0x708 images trois messages (paroles + effets : routines 0xa66c0, 0x2824,
/// 0x2860, 0x289c, 0xeda26, 0xeda66) ; si la caméra avance de plus de 0xa00 depuis le départ
/// (`+0x1e`) il abrège et passe directement à la dernière étape.
mod level_clock {
    use super::*;

    const SPEECH: u32 = 0xa66c0;
    const SAY_A: u32 = 0x2824;
    const SAY_B: u32 = 0x2860;
    const SAY_C: u32 = 0x289c;
    const ON: u32 = 0xeda26;
    const OFF: u32 = 0xeda66;
    const TIME: u32 = 0x42;
    const CUE: u32 = 0x1a;
    const TICK: u32 = 0x1c;
    const FAR: i16 = 0xa00;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(TIME, 0);
                let v = x.a5_u16(ram::CAMERA_X);
                x.set16(0x1e, v);
            }
            2 => step(x),
            _ => x.call(sub::FREE_SET),
        }
    }

    fn speech(x: &mut Cx, kind: u32) {
        x.c.d[2] = kind;
        x.call(SPEECH);
    }

    fn countdown(x: &mut Cx, reset: u16, kind: u32) {
        let n = x.u16(TICK).wrapping_sub(1);
        x.set16(TICK, n);
        if (n as i16) < 0 {
            x.set16(TICK, reset);
            speech(x, kind);
        }
        let n = x.u16(CUE).wrapping_sub(1);
        x.set16(CUE, n);
        if n == 0 {
            x.call(OFF);
        }
    }

    fn step(x: &mut Cx) {
        if x.a5_u8(0x35) != 0 {
            x.add16(TIME, 1);
        }
        if x.u8(0x5) != 6 {
            let far = x.a5_u16(ram::CAMERA_X).wrapping_sub(x.u16(0x1e)) as i16;
            if far >= FAR {
                x.set16(CUE, 0x78);
                x.set8(0x5, 6);
            }
        }
        let time = x.u16(TIME) as i16;
        match x.u8(0x5) {
            0 => {
                if time < 0x258 {
                    return;
                }
                x.add8(0x5, 2);
                show_message(x, 2);
                x.set16(TICK, 0x50);
                speech(x, 0);
                x.set16(CUE, 0x28);
                call_with(x, SAY_A, 0x28);
                x.call(ON);
            }
            2 => {
                countdown(x, 0x50, 0);
                if time < 0x4b0 {
                    return;
                }
                x.add8(0x5, 2);
                show_message(x, 2);
                x.set16(TICK, 0x3c);
                speech(x, 1);
                x.set16(CUE, 0x28);
                call_with(x, SAY_B, 0x28);
            }
            4 => {
                countdown(x, 0x3c, 1);
                if time < 0x708 {
                    return;
                }
                x.add8(0x5, 2);
                speech(x, 2);
                x.set16(CUE, 0x78);
                call_with(x, SAY_C, 0x78);
                x.call(ON);
            }
            6 => {
                let n = x.u16(CUE).wrapping_sub(1);
                x.set16(CUE, n);
                if n != 0 {
                    return;
                }
                x.add8(0x4, 2);
                x.set_a5_u8(0x42, 1);
                x.call(OFF);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 32 (0xd96d0) : sélecteur de test (h)
// ---------------------------------------------------------------------------------------------

/// Sélecteur de test (h) : avec les touches du joueur 1 (octets A5+2 pressées, A5+8 précédent),
/// le bouton 5 augmente un numéro (0 à 0x32, `+0x2f`), le bouton 6 le diminue, le bouton 4 le
/// valide (routine 0xcc352 sur le premier joueur). Le numéro est affiché en deux chiffres
/// hexadécimaux dans le fond (0x912bc0).
mod test_selector {
    use super::*;
    use crate::mem::wr16;

    const SCREEN: u32 = 0x912bc0;
    const LAST: u8 = 0x32;
    const APPLY: u32 = 0xcc352;
    const ROW: u32 = 0x80;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x2f, 0);
            }
            2 => step(x),
            _ => x.call(sub::FREE_SET),
        }
    }

    fn step(x: &mut Cx) {
        let pressed = !x.a5_u8(8) & x.a5_u8(2);
        if pressed >> 4 & 1 != 0 {
            let index = x.u8(0x2f) as u32;
            x.c.d[0] = index;
            let me = x.o.0;
            x.c.a[0] = PLAYERS;
            crate::call(x.c, x.b, APPLY);
            x.c.a[0] = me;
        } else if pressed >> 5 & 1 != 0 {
            let n = x.u8(0x2f) + 1;
            x.set8(0x2f, if n > LAST { 0 } else { n });
            show(x);
        } else if pressed >> 6 & 1 != 0 {
            let n = x.u8(0x2f);
            x.set8(0x2f, if n == 0 { LAST } else { n - 1 });
            show(x);
        }
    }

    fn show(x: &mut Cx) {
        let n = x.u8(0x2f) as u16;
        for (i, digit) in [n >> 4, n & 0xf].into_iter().enumerate() {
            wr16(x.b, SCREEN + ROW * i as u32, digit & 0xf);
            wr16(x.b, SCREEN + ROW * i as u32 + 2, 0);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 24 (0xd7056) : fenêtre de dialogue
// ---------------------------------------------------------------------------------------------

/// Fenêtre de dialogue (h) : cadre dessiné dans la couche de fond 1 (GFX RAM 0x912000, 0x80 octets
/// par ligne) qui s'ouvre, reçoit un texte, attend une touche puis se referme. La variante `+3`
/// désigne, par la table 0x1d78d0, une définition de fenêtre dans la ROM (position, taille,
/// texte `+0x24`, sorte `+0x2f`...). La sorte (24 valeurs) règle l'initialisation (palettes, choix
/// d'un autre texte selon le nombre de joueurs...) et la construction de la liste de jetons du
/// texte (mots dans le tampon 0xff2072, relue par le moteur de texte via la case `A5+0x5c6+
/// (+0x38)`). Jetons spéciaux : 0 fin, négatifs = commandes, 0x7000 à 0x7009 = substitutions
/// (nombre de joueurs, noms des joueurs...).
///
/// États : 0 ouverture, 2 vie (sous-états `+5` : 0 ouverture du cadre, 2 déploiement, 4 attente d'une
/// touche, 6 fermeture), 4 et 6 libération.
mod dialog {
    use super::*;
    use crate::mem::{rd16, rd32, wr16, wr32};

    /// Définitions de fenêtres par variante (pointeurs, espace de données).
    const DEFINITIONS: u32 = 0x1d78d0;
    const SCREEN: u32 = 0x912000;
    /// Tampon des jetons (A5-0x5f8e) et cases du moteur de texte (A5+0x5c6, 8 octets chacune).
    const BUFFER: u32 = 0xff_2072;
    const SLOTS: u32 = A5 + 0x5c6;
    /// Compteur de fenêtres ouvertes (A5-0x5b8e) et drapeau (A5-0x5fbe).
    const OPEN_COUNT: u32 = 0xff_2472;
    const FLAG_SKIP: u32 = 0xff_2042;
    const PALETTE_SRC: u32 = 0x292260;
    const PALETTE_ALT: u32 = 0x292200;
    const CHARS: u32 = 0xd7e8a;
    const SEPARATOR: u32 = 0xd7e26;
    const NAME_TABLE: u32 = 0xd7604;
    const PLAYER_TITLES: u32 = 0xd7db6;
    const PLAYERS_ONE_TWO: u32 = 0xd7d96;
    const PLAYERS_ONE_TWO_B: u32 = 0xd7da6;
    const TOK_LINK_A: u16 = 0xfffe;
    /// Mots d'ouverture et de fermeture d'un nom de joueur.
    const NAME_OPEN: u32 = 0xfffe_0018;
    const NAME_CLOSE: u32 = 0xfffe_000f;
    /// Création / recherche d'objets liés.
    const FIND_OBJECT: u32 = 0xafe02;
    const SPAWN_SET: u32 = 0xce00a;
    const SPAWN_SOBJ: u32 = 0xddeaa;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => open(x),
            2 => step(x),
            _ => close(x),
        }
    }

    // --- sortie des jetons -----------------------------------------------------------------

    /// Pointeur d'écriture (A4) et nombre de caractères (D6) de la liste de jetons.
    struct Out {
        at: u32,
        n: u16,
    }

    impl Out {
        fn put(&mut self, x: &mut Cx, w: u16) {
            wr16(x.b, self.at, w);
            self.at += 2;
        }
        fn put32(&mut self, x: &mut Cx, v: u32) {
            self.put(x, (v >> 16) as u16);
            self.put(x, v as u16);
        }
        /// Terminateur, non avancé.
        fn end(&mut self, x: &mut Cx) {
            wr16(x.b, self.at, 0);
        }
    }

    /// Commande (jeton négatif) : recopie le jeton, `-8 * n`, puis le mot qui suit le mot suivant
    /// (0xfffe : le mot suivant seul).
    fn command(x: &mut Cx, o: &mut Out, a6: &mut u32, tok: u16) {
        o.put(x, tok);
        if tok == TOK_LINK_A {
            let w = x.data16(*a6);
            *a6 += 2;
            return o.put(x, w);
        }
        o.put(x, (o.n << 3).wrapping_neg());
        o.n = 0;
        *a6 += 2;
        let w = x.data16(*a6);
        *a6 += 2;
        o.put(x, w);
    }

    /// Lit le mot suivant du texte.
    fn next(x: &mut Cx, a6: &mut u32) -> u16 {
        let w = x.data16(*a6);
        *a6 += 2;
        w
    }

    /// Texte brut : jetons recopiés, 0x7000 (nombre de joueurs) remplacé selon le nombre de joueurs.
    fn plain(x: &mut Cx, o: &mut Out, a6: &mut u32) {
        loop {
            let tok = next(x, a6);
            if tok == 0 {
                return o.end(x);
            }
            if (tok as i16) < 0 {
                command(x, o, a6, tok);
            } else if tok == 0x7000 {
                if x.a5_u8(0x35) != 1 {
                    o.put(x, 0x18f3);
                    o.n += 1;
                }
            } else {
                o.put(x, tok);
                o.n += 1;
            }
        }
    }

    /// Chaîne de caractères d'un nom (jusqu'à 6 octets, index dans la table de tuiles 0xd7e8a ;
    /// 0xff termine). `special` : les octets >= 0xf9 désignent un nom prédéfini de la table 0xd7604.
    fn name(x: &mut Cx, o: &mut Out, mut a1: u32) {
        let mut remaining: u16 = 5;
        let first = x.data8(a1);
        if first.wrapping_add(1) >= 0xfa {
            let index = (0u8.wrapping_sub(first).wrapping_sub(3)).wrapping_mul(4) as u32;
            a1 = x.data32(NAME_TABLE + index);
            remaining = x.data16(a1);
            a1 += 2;
        }
        loop {
            let c = x.data8(a1);
            a1 += 1;
            if c == 0xff {
                return;
            }
            let tile = x.data16(CHARS + 2 * c as u32);
            o.put(x, tile);
            o.n += 1;
            remaining = remaining.wrapping_sub(1);
            if remaining == 0xffff {
                return;
            }
        }
    }

    /// Variante pour les noms ayant une catégorie (octet de départ >= 0x82 : 0x88, 0x90, 0x82,
    /// 0xa0 ou autre, chacun avec son propre texte dans la table 0xd7604).
    fn name_category(x: &mut Cx, o: &mut Out, mut a1: u32) {
        let mut remaining: u16 = 5;
        let first = x.data8(a1);
        if first >= 0x82 {
            let mut d0 = 0u32;
            for probe in [0x88, 0x90, 0x82, 0xa0] {
                if first == probe {
                    break;
                }
                d0 += 4;
            }
            a1 = x.data32(NAME_TABLE + d0);
            remaining = x.data16(a1);
            a1 += 2;
        }
        loop {
            let c = x.data8(a1);
            a1 += 1;
            if c == 0xff {
                return;
            }
            let tile = x.data16(CHARS + 2 * c as u32);
            o.put(x, tile);
            o.n += 1;
            remaining = remaining.wrapping_sub(1);
            if remaining == 0xffff {
                return;
            }
        }
    }

    /// Fin d'un bloc de nom : mot -1, `-8 * n`, 0x10 (routine 0xd75a6).
    fn name_tail(x: &mut Cx, o: &mut Out) {
        o.put(x, 0xffff);
        o.put(x, (o.n << 3).wrapping_neg());
        o.put(x, 0x10);
    }

    /// Nom du joueur `p` entre les ouvertures 0xfffe0018 et 0xfffe000f (routine 0xd75b6 sans la fin).
    fn player_name(x: &mut Cx, o: &mut Out, p: Obj) {
        o.put32(x, NAME_OPEN);
        name(x, o, p.0 + 0x118);
    }

    /// Bloc complet d'un joueur de la sorte 1 (routine 0xd7536) : titre, nom, fin.
    fn player_block(x: &mut Cx, o: &mut Out, p: Obj) {
        let index = ((p.u8(x.b, 3) >> 1).wrapping_mul(4)) as u32;
        let mut a1 = x.data32(PLAYER_TITLES + index);
        loop {
            let w = x.data16(a1);
            if w == 0 {
                break;
            }
            o.put(x, w);
            a1 += 2;
            o.n += 1;
        }
        player_name(x, o, p);
        o.put32(x, NAME_CLOSE);
        name_tail(x, o);
    }

    /// `movem.l (A2)+,D0-D6/A3 ; movem.l D0-D6/A3,(A1)` : les huit longs restent dans D0-D6 et A3 (le moteur de
    /// fenêtres relit D6 : la fermeture d'une fenêtre y lit le nombre de lignes).
    fn copy_palette(x: &mut Cx, src: u32, dst: u32) {
        copy_longs(x, src, dst, 8);
        for i in 0..7 {
            x.c.d[i] = x.data32(src + 4 * i as u32);
        }
        x.c.a[3] = x.data32(src + 28);
    }

    fn copy_longs(x: &mut Cx, from: u32, to: u32, n: u32) {
        for i in 0..n {
            let v = x.data32(from + 4 * i);
            wr32(x.b, to + 4 * i, v);
        }
    }

    // --- construction du texte, selon la sorte -------------------------------------------------

    /// Construit la liste de jetons de la sorte `kind` (table 0xd7464 : l'index est décalé d'un cran,
    /// la sorte `k` saute à la `k`-ième entrée de la table 0xd7466 ; la sorte 0 est traitée avant).
    fn build(x: &mut Cx, kind: u8) {
        let mut o = Out { at: BUFFER, n: 0 };
        let mut a6 = x.u32(0x24);
        match kind {
            2 => {
                copy_palette(x, PALETTE_ALT, 0x91c300);
                for p in players() {
                    if p.u8(x.b, pl::ACTIVE) != 0 && p.u8(x.b, 4) != 4 {
                        o.n = 0;
                        player_block(x, &mut o, p);
                    }
                }
                o.end(x);
            }
            3 => {
                x.c.d[0] = 0;
                let d1 = x.u8(0x34) as u32;
                x.c.d[1] = (x.c.d[1] & !0xff) | d1;
                x.call(FIND_OBJECT);
                if x.c.sr & m68k::SR_C != 0 {
                    x.add8(0x5, 0xfe);
                    return;
                }
                let other = Obj(x.c.a[1]);
                other.set_u16(x.b, 0x50, x.o.0 as u16);
                x.set16(0x32, 0xffff);
                build(x, 1);
            }
            4 => {
                copy_palette(x, PALETTE_ALT, 0x91c300);
                let mut left = x.a5_u8(0x35);
                for p in players() {
                    if p.u8(x.b, pl::ACTIVE) == 0 || (p.u8(x.b, 4) as i8) >= 4 {
                        continue;
                    }
                    o.n = 0;
                    player_name(x, &mut o, p);
                    left = left.wrapping_sub(1);
                    if left == 0 {
                        continue;
                    }
                    let sep = x.data16(SEPARATOR);
                    o.put(x, sep);
                    o.n += 1;
                    name_tail(x, &mut o);
                }
                let sep = x.data16(SEPARATOR);
                o.put(x, sep);
                o.end(x);
            }
            6 | 7 => {
                loop {
                    let tok = next(x, &mut a6);
                    if tok == 0 {
                        break;
                    }
                    if (tok as i16) < 0 {
                        command(x, &mut o, &mut a6, tok);
                        continue;
                    }
                    let table = match tok {
                        0x7001 => PLAYERS_ONE_TWO,
                        0x7002 => PLAYERS_ONE_TWO_B,
                        _ => {
                            o.put(x, tok);
                            o.n += 1;
                            continue;
                        }
                    };
                    let index = ((x.a5_u8(0x35).wrapping_sub(1)) as u16).wrapping_mul(4) as u32;
                    let mut a1 = x.data32(table + index);
                    loop {
                        let w = x.data16(a1);
                        if w == 0 {
                            break;
                        }
                        o.put(x, w);
                        a1 += 2;
                        o.n += 1;
                    }
                }
                o.end(x);
            }
            8 => {
                loop {
                    let tok = next(x, &mut a6);
                    if tok == 0 {
                        break;
                    }
                    if (tok as i16) < 0 {
                        command(x, &mut o, &mut a6, tok);
                    } else if tok == 0x7003 {
                        let w = if x.a5_u8(0x35) == 1 { 0x2013 } else { 0x2018 };
                        o.put(x, w);
                        o.n += 1;
                    } else {
                        o.put(x, tok);
                        o.n += 1;
                    }
                }
                o.end(x);
            }
            10 => build_9(x, &mut o, a6),
            12 => x.set16(0x32, 0xffff),
            13 => text_with_name(x, &mut o, a6, 0x7005, false),
            15 => {
                let a1 = x.u32(0x24);
                let first = x.data32(a1);
                x.set32(0x24, first);
                if x.a5_u8(0x35) != 1 {
                    let second = x.data32(a1 + 4);
                    x.set32(0x24, second);
                }
                build(x, 3);
            }
            16 => {
                x.set8(0x23, 1);
                let slot = x.linked(0x3a);
                let v = x.u32(0x24);
                slot.set_u32(x.b, 0, v);
            }
            17 => build_16(x, &mut o, a6),
            18 => {
                build(x, 1);
                call_with(x, SPAWN_SOBJ, 0x13);
                if x.c.sr & m68k::SR_C == 0 {
                    let child = Obj(x.c.a[1]);
                    child.set_u16(x.b, 0x46, x.o.0 as u16);
                    let v = x.u8(0x34);
                    child.set_u8(x.b, 0x2f, v);
                }
            }
            21 => build_20(x, &mut o, a6),
            1 | 5 | 9 | 11 | 14 | 19 | 20 | 22 | 23 => {
                plain(x, &mut o, &mut a6);
                let v = rd16(x.b, BUFFER);
                x.set16(0x30, v);
            }
            _ => {}
        }
    }

    /// Texte dont le jeton `special` appelle le nom d'un joueur situé en 0xff0fd2 (sortes 12, 20).
    fn text_with_name(x: &mut Cx, o: &mut Out, mut a6: u32, special: u16, _alt: bool) {
        loop {
            let tok = next(x, &mut a6);
            if tok == 0 {
                return o.end(x);
            }
            if (tok as i16) < 0 {
                command(x, o, &mut a6, tok);
            } else if tok == special {
                o.put32(x, NAME_OPEN);
                name_category(x, o, 0xff_0fd2);
                o.put32(x, NAME_CLOSE);
            } else {
                o.put(x, tok);
                o.n += 1;
            }
        }
    }

    /// Sorte 9 : le joueur de classe 5 (octet `+3` >> 1) donne son nom à la place du jeton 0x7004 ;
    /// sans ce joueur, le texte n'est qu'un lien vers l'autre texte.
    fn build_9(x: &mut Cx, o: &mut Out, a6: u32) {
        let mut found = None;
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) == 0 || (p.u8(x.b, 4) as i8) >= 4 {
                continue;
            }
            if p.u8(x.b, 3) >> 1 == 5 {
                found = Some(p);
                break;
            }
        }
        let Some(player) = found else {
            let slot = x.linked(0x3a);
            let v = x.data32(a6);
            slot.set_u32(x.b, 0, v);
            return;
        };
        let mut a6 = x.data32(a6 + 4);
        loop {
            let tok = next(x, &mut a6);
            if tok == 0 {
                return o.end(x);
            }
            if (tok as i16) < 0 {
                command(x, o, &mut a6, tok);
            } else if tok == 0x7004 {
                name(x, o, player.0 + 0x118);
            } else {
                o.put(x, tok);
                o.n += 1;
            }
        }
    }

    /// Sorte 16 : jetons 0x7006 à 0x7009 = noms du propriétaire `+0x46`, objets équipés...
    fn build_16(x: &mut Cx, o: &mut Out, mut a6: u32) {
        loop {
            let tok = next(x, &mut a6);
            if tok == 0 {
                // fin : cherche une pièce de test (0xce00a, type 17) à lier à ce texte
                wr16(x.b, o.at, tok);
                call_with(x, SPAWN_SET, 0x11);
                if x.c.sr & m68k::SR_C == 0 {
                    let created = Obj(x.c.a[1]);
                    created.set_u16(x.b, 0x46, x.o.0 as u16);
                    let v = x.u16(0x34);
                    created.set_u16(x.b, 0x42, v);
                } else {
                    x.add8(0x5, 0xfe);
                }
                return;
            }
            if (tok as i16) < 0 {
                command(x, o, &mut a6, tok);
                continue;
            }
            if (tok as i16) < 0x7006 {
                o.put(x, tok);
                o.n += 1;
                continue;
            }
            let entry = x.rom16(0xd7988 + 2 * (tok - 0x7006) as u32) as u32;
            match 0xd7988 + entry {
                0xd7990 => owner_name(x, o),
                0xd799c => equipped_item(x, o),
                0xd7a72 => {
                    if x.u16(0x46) != 0 {
                        owner_name(x, o)
                    }
                }
                _ => {}
            }
        }
    }

    /// Nom du propriétaire de la fenêtre (`+0x46`) avec ses ouvertures (routine 0xd75ba).
    fn owner_name(x: &mut Cx, o: &mut Out) {
        let owner = x.linked(0x46);
        o.put32(x, NAME_OPEN);
        name(x, o, owner.0 + 0x118);
    }

    /// Premier objet équipé du propriétaire qui figure dans l'une des listes de la sorte 16
    /// (routines 0xd799c, 0xd7a38 et 0xd7a5c) : recopie le texte de l'objet.
    fn equipped_item(x: &mut Cx, o: &mut Out) {
        let owner = x.linked(0x46);
        let (a, b, c) = if owner.u8(x.b, 3) >> 1 == 0 {
            (0xd7a20, 0xd7a20, 0xd7a26)
        } else {
            (0xd7a2c, 0xd7a2c, 0xd7a32)
        };
        let mut candidates = vec![(a, owner.u8(x.b, 0x10d)), (b, owner.u8(x.b, 0x10e))];
        for i in 0..6 {
            candidates.push((c, owner.u8(x.b, 0x140 + i)));
        }
        for (list, value) in candidates {
            let mut index = None;
            let mut at = list;
            let mut i = 0;
            loop {
                let e = x.data8(at);
                at += 1;
                if (e as i8) < 0 {
                    break;
                }
                if e == value {
                    index = Some(i);
                    break;
                }
                i += 1;
            }
            if let Some(i) = index {
                let text = x.rom16(0xd7a48 + 4 * i);
                let mut text = (text as u32) << 16 | x.rom16(0xd7a48 + 4 * i + 2) as u32;
                loop {
                    let w = x.data16(text);
                    if w == 0 {
                        return;
                    }
                    o.put(x, w);
                    o.n += 1;
                    text += 2;
                }
            }
        }
    }

    /// Sorte 20 : comme la sorte 12 (jeton 0x7005), mais précédée d'un changement de palette en
    /// partie à un joueur ; jeton 0x7000 sauté en solo ou remplacé par 0x2003.
    fn build_20(x: &mut Cx, o: &mut Out, mut a6: u32) {
        o.put32(x, 0xfffe_000c);
        if x.a5_u8(0x50) != 0 && x.a5_u8(0x42) != 0 {
            o.at -= 4;
            copy_palette(x, 0x292260, 0x91c260);
        }
        loop {
            let tok = next(x, &mut a6);
            if tok == 0 {
                return o.end(x);
            }
            if (tok as i16) < 0 {
                command(x, o, &mut a6, tok);
            } else if tok == 0x7000 {
                if x.a5_u8(0x35) != 1 {
                    o.put(x, 0x2003);
                    o.n += 1;
                }
            } else if tok == 0x7005 {
                name_category(x, o, 0xff_0fd2);
                // le reste du texte est lu comme pour la sorte 12
                return text_with_name(x, o, a6, 0x7005, true);
            } else {
                o.put(x, tok);
                o.n += 1;
            }
        }
    }

    // --- chargement d'une définition de fenêtre --------------------------------------------------

    /// Routine 0xd7124 : lit la définition à `def`, calcule l'adresse d'écran et applique
    /// l'initialisation de la sorte. Rend l'adresse d'écran (A1 à la sortie de la routine).
    fn load(x: &mut Cx, mut def: u32) -> u32 {
        x.set16(0x30, 0);
        wr16(x.b, BUFFER, 0);
        let col = x.data8(def) as u16;
        let row = x.data8(def + 1) as u16;
        let v = x.data8(def + 2);
        x.set8(0x18, v);
        let v = x.data8(def + 3);
        x.set8(0x19, v);
        let v = x.data32(def + 4);
        x.set32(0x24, v);
        let v = x.data16(def + 8);
        x.set16(0x8, v);
        let v = x.data16(def + 10);
        x.set16(0xc, v);
        let v = x.data8(def + 12);
        x.set8(0x2f, v);
        let v = x.data8(def + 13);
        x.set8(0x1a, v);
        let v = x.data16(def + 14);
        x.set16(0x32, v);
        let v = x.data16(def + 16);
        x.set16(0x34, v);
        def += 18;
        x.set32(0x1c, def);
        let offset = (col << 7).wrapping_add(row << 2) as i16 as i32 as u32;
        let a1 = SCREEN.wrapping_add(offset);
        x.set32(0x14, a1);
        let height = if x.u8(0x18) != 0 { 0xb } else { 0xf };
        x.set8(0x20, height);
        let v = x.u8(0x19);
        x.set8(0x21, v);
        kind_init(x, a1)
    }

    /// Initialisation propre à la sorte (table de sauts 0xd7196).
    fn kind_init(x: &mut Cx, a1: u32) -> u32 {
        match x.u8(0x2f) {
            1 => {
                let script = x.u32(0x24);
                let pick = if x.a5_u8(0x35) == 1 { 0 } else { 4 };
                let next = x.data32(script + pick);
                load(x, next)
            }
            2 | 4 => {
                let more = x.a5_u8(0x35).wrapping_sub(1).wrapping_mul(2);
                x.add8(0x19, more);
                let v = x.u8(0x19);
                x.set8(0x21, v);
                a1
            }
            5 => {
                let mut d0 = (x.u8(0x42) as u32 * 2) as u8;
                d0 = d0.wrapping_add(x.a5_u8(0x6f) & 1);
                let step = x.rom8(0xd7218 + d0 as u32) as u32;
                let script = x.u32(0x24).wrapping_add(step);
                load(x, script)
            }
            9 | 17 | 21 => {
                x.set8(0x5, 2);
                x.set8(0x19, 0);
                x.set8(0x21, 0);
                x.set8(0x20, 0xff);
                match x.u8(0x2f) {
                    9 => {
                        x.set8(0x3, 1);
                        x.set8(0x2f, 0);
                        a1
                    }
                    17 => {
                        x.set8(0x1b, 1);
                        copy_palette(x, PALETTE_ALT, 0x91c300);
                        x.set16(0x32, 0xffff);
                        copy_palette(x, PALETTE_SRC, 0x91c1e0);
                        for base in [0x91c1e0, 0x91c300] {
                            for i in 0..8 {
                                let v = rd32(x.b, base + 4 * i) & 0x0fff_0fff;
                                wr32(x.b, base + 4 * i, v);
                            }
                        }
                        0x91c200
                    }
                    _ => {
                        if rd8(x.b, FLAG_SKIP) != 0 {
                            x.set8(0x5, 6);
                            x.set8(0x1b, 1);
                            return a1;
                        }
                        x.set8(0x1b, 1);
                        copy_palette(x, PALETTE_SRC, 0x91c180);
                        palette_tail(x, 0x91c180)
                    }
                }
            }
            11 => {
                let script = x.u32(0x24);
                let kind = x.u8(0x23).wrapping_mul(4) as u32;
                let next = x.data32(script + kind);
                load(x, next)
            }
            13 => {
                copy_palette(x, PALETTE_ALT, 0x91c300);
                0x91c320
            }
            14 => {
                x.set8(0x36, 1);
                x.set8(0x2f, 0);
                a1
            }
            19 => {
                x.set8(0x38, 0x10);
                let mut script = x.u32(0x24);
                for _ in 0..x.u8(0x42) {
                    script += 0x12;
                }
                load(x, script)
            }
            20 => {
                let mut script = x.u32(0x24);
                for p in players() {
                    if p.u8(x.b, pl::ACTIVE) == 0 || (p.u8(x.b, 4) as i8) >= 4 {
                        continue;
                    }
                    if p.u8(x.b, 3) >> 1 == 3 {
                        script += 0x24;
                        break;
                    }
                }
                load(x, script)
            }
            22 => {
                x.set8(0x36, 1);
                x.set8(0x2f, 1);
                a1
            }
            23 => {
                x.set32(0x4, 0x0400_0000);
                a1
            }
            _ => a1,
        }
    }

    /// Lumière de la fenêtre : si une partie est en cours et que son bit 0 est levé
    /// (`A5+0x48e`), la luminosité de la palette 0x91c2a0 est reportée sur les huit longs à
    /// partir de `a1` (0x91c1e0 ou 0x91c380). Rend l'adresse après les huit longs.
    fn palette_tail(x: &mut Cx, a1: u32) -> u32 {
        if x.a5_u8(0x48f) == 0 || x.a5_u8(0x48e) & 1 == 0 {
            return a1;
        }
        let bright = rd32(x.b, 0x91c2a0) & 0xf000_f000;
        for i in 0..8 {
            let v = rd32(x.b, a1 + 4 * i) & 0x0fff_0fff | bright;
            wr32(x.b, a1 + 4 * i, v);
        }
        a1 + 0x20
    }

    // --- états ------------------------------------------------------------------------------

    fn open(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(0x36, 3);
        let variant = x.u8(0x3) as u32;
        let def = x.data32(DEFINITIONS + 4 * variant);
        load(x, def);
        let stage = x.a5_u8(0x34);
        let a1 = if (0x45..0x5a).contains(&stage) { 0x91c380 } else { 0x91c1e0 };
        match x.u8(0x2f) {
            0x11 => return,
            0x15 => {
                if rd8(x.b, FLAG_SKIP) == 0 {
                    return;
                }
                return x.set8(0x2f, 0);
            }
            _ => {}
        }
        copy_palette(x, PALETTE_SRC, a1);
        palette_tail(x, a1);
    }

    fn step(x: &mut Cx) {
        match x.u8(0x5) {
            0 => {
                x.add8(0x5, 2);
                let v = x.u8(0x36);
                x.set8(0x1b, v);
                let a1 = x.u32(0x14);
                frame_top(x, a1);
                x.add16(0x16, 8);
                let a1 = x.u32(0x14);
                frame_middle(x, a1);
            }
            2 => grow(x),
            4 => wait_key(x),
            6 => shrink(x),
            _ => {}
        }
    }

    fn rd8(b: &mut cps2::Cps2Bus, a: u32) -> u8 {
        crate::mem::rd8(b, a)
    }

    /// Sous-état 2 : le cadre grandit d'une ligne toutes les `+0x36` images.
    fn grow(x: &mut Cx) {
        let n = x.u8(0x1b).wrapping_sub(1);
        x.set8(0x1b, n);
        if (n as i8) > 0 {
            return;
        }
        let v = x.u8(0x36);
        x.set8(0x1b, v);
        if x.u8(0x21) != 0 {
            x.add8(0x21, 0xff);
            let a1 = x.u32(0x14);
            frame_side(x, a1);
            x.add16(0x16, 4);
            let a1 = x.u32(0x14);
            frame_middle(x, a1);
            return;
        }
        x.add8(0x5, 2);
        let v = x.u8(0x19);
        x.set8(0x21, v);
        let slot = SLOTS + x.u8(0x38) as u32;
        x.set16(0x3a, slot as u16);
        let v = x.u16(0x8);
        wr16(x.b, slot + 4, v);
        let v = x.u16(0xc);
        wr16(x.b, slot + 6, v);
        let kind = x.u8(0x2f);
        if kind == 0 {
            let v = x.u32(0x24);
            wr32(x.b, slot, v);
            return;
        }
        if kind != 0xc {
            wr32(x.b, slot, BUFFER);
        }
        build(x, kind);
    }

    /// Sous-état 4 : attend qu'un joueur appuie (compte à rebours `+0x32` ; -1 = sans fin).
    fn wait_key(x: &mut Cx) {
        if x.u16(0x32) == 0xffff {
            return;
        }
        if poll_keys(x) != 0 {
            x.set16(0x32, 0);
        }
        if x.u16(0x32) == 0 {
            x.add8(0x5, 2);
            if x.u8(0x2f) == 0xc {
                return;
            }
            let slot = x.linked(0x3a);
            slot.set_u32(x.b, 0, 0);
            return;
        }
        x.add16(0x32, 0xffff);
    }

    /// Touches pressées par les joueurs concernés (octet `A5+0xc+n`, bits 4 à 7).
    fn poll_keys(x: &mut Cx) -> u8 {
        let mut who: i8 = -1;
        if x.u8(0x3) == 0x8b {
            who = x.u8(0x43) as i8;
        } else if (x.u8(0x37) as i8) >= 0 {
            who = x.u8(0x37) as i8;
        }
        if x.a5_u8(0x30) == 0 {
            return 0;
        }
        let mut keys = 0u8;
        for n in (0..4u32).rev() {
            let p = Obj(PLAYERS + n * PLAYER_SIZE);
            if p.u8(x.b, pl::ACTIVE) == 0 || (p.u8(x.b, 4) as i8) >= 4 {
                continue;
            }
            if who >= 0 && p.u8(x.b, 2) as i8 != who {
                continue;
            }
            keys |= x.a5_u8(0xc + n);
        }
        keys & 0xf0
    }

    /// Sous-état 6 : le cadre se referme d'une ligne par pas ; puis l'objet est supprimé.
    fn shrink(x: &mut Cx) {
        let n = x.u8(0x1b).wrapping_sub(1);
        x.set8(0x1b, n);
        if (n as i8) > 0 {
            return;
        }
        let v = x.u8(0x36);
        x.set8(0x1b, v);
        let mut a6;
        // D6 : nombre de lignes de la fenêtre (octet), que le chargement d'une définition peut écraser
        let d6;
        if x.u8(0x21) != 0 {
            x.add8(0x21, 0xff);
            let a1 = x.u32(0x14);
            erase_column(x, a1);
            x.add16(0x16, 0xfffc);
            let a1 = x.u32(0x14);
            return frame_middle(x, a1);
        }
        if (x.u8(0x1a) as i8) >= 0 {
            x.set8(0x5, 0);
            a6 = x.u32(0x14).wrapping_sub(8);
            x.c.d[6] = x.u8(0x20) as u32;
            let def = x.u32(0x1c);
            let a1 = load(x, def);
            d6 = x.c.d[6];
            if x.u8(0x20) == d6 as u8 && a1 == a6 {
                return;
            }
        } else {
            x.add8(0x5, 2);
            x.set8(0x22, 1);
            x.set32(0x4, 0x0600_0000);
            a6 = x.u32(0x14).wrapping_sub(8);
            d6 = x.u8(0x20) as u32;
        }
        if (d6 as u8 as i8) < 0 {
            return;
        }
        let count = (d6 as u16).wrapping_mul(2).wrapping_add(1);
        for _ in 0..=count {
            for k in 0..3 {
                wr32(x.b, a6 + 4 * k, 0x20_0000);
            }
            a6 += 0xc + 0x74;
        }
    }

    fn close(x: &mut Cx) {
        let n = x.a5_u8(0x51).wrapping_sub(1);
        x.set_a5_u8(0x51, n);
        x.set8(0x22, 0);
        let v = crate::mem::rd8(x.b, OPEN_COUNT).wrapping_sub(1);
        crate::mem::wr8(x.b, OPEN_COUNT, v);
        x.call(sub::FREE_SET);
    }

    // --- dessin du cadre ----------------------------------------------------------------------

    fn frame_top(x: &mut Cx, mut a1: u32) {
        let tiles: [u32; 2] = if x.u8(0x18) != 0 { [0x0434_000f, 0x0444_000f] } else { [0x0400_000f, 0x0410_000f] };
        let (mut d0, mut d1) = (tiles[0], tiles[1]);
        let rows = x.u8(0x20) as i16;
        for _ in 0..=rows {
            wr32(x.b, a1, d0);
            wr32(x.b, a1 + 4, d1);
            d0 = d0.wrapping_add(0x1_0000);
            d1 = d1.wrapping_add(0x1_0000);
            a1 += 0x80;
        }
        d0 = (d0 & !0xff) | (d0 as u8 | 0x20) as u32;
        d1 = (d1 & !0xff) | (d1 as u8 | 0x20) as u32;
        d0 = d0.wrapping_add(0xffff_0000);
        d1 = d1.wrapping_add(0xffff_0000);
        let rows = x.u8(0x20) as i16;
        for _ in 0..=rows {
            wr32(x.b, a1, d0);
            wr32(x.b, a1 + 4, d1);
            d0 = d0.wrapping_add(0xffff_0000);
            d1 = d1.wrapping_add(0xffff_0000);
            a1 += 0x80;
        }
    }

    fn frame_side(x: &mut Cx, mut a1: u32) {
        let mut d0 = if x.u8(0x18) != 0 { 0x0432_000f } else { 0x0430_000f };
        wr32(x.b, a1, d0);
        a1 += 0x80;
        d0 = (d0 & !0xff) | (d0 as u8 | 0x20) as u32;
        let count = (x.u8(0x20) as u16).wrapping_mul(2).wrapping_sub(1) as u32 + 1;
        for _ in 0..count {
            wr32(x.b, a1, 0x0431_000f);
            a1 += 0x80;
        }
        wr32(x.b, a1, d0);
    }

    fn frame_middle(x: &mut Cx, mut a1: u32) {
        let mut d0 = if x.u8(0x18) != 0 { 0x0454_000f } else { 0x0420_000f };
        let rows = x.u8(0x20) as i16;
        for _ in 0..=rows {
            wr32(x.b, a1, d0);
            d0 = d0.wrapping_add(0x1_0000);
            a1 += 0x80;
        }
        d0 = (d0 & !0xff) | (d0 as u8 | 0x20) as u32;
        d0 = d0.wrapping_add(0xffff_0000);
        let rows = x.u8(0x20) as i16;
        for _ in 0..=rows {
            wr32(x.b, a1, d0);
            d0 = d0.wrapping_add(0xffff_0000);
            a1 += 0x80;
        }
    }

    fn erase_column(x: &mut Cx, mut a1: u32) {
        let rows = (x.u8(0x20) as u16).wrapping_mul(2).wrapping_add(1);
        for _ in 0..=rows {
            wr32(x.b, a1, 0x20_0000);
            a1 += 0x80;
        }
    }
}

handlers! {
    0xd4110 "set_16" 400 bumper::run,
    0xd7056 "set_24" 800 dialog::run,
    0xd9424 "set_30" 200 game_over::run,
    0xd9492 "set_31" 300 level_clock::run,
    0xd96d0 "set_32" 200 test_selector::run,
    0xd8458 "set_27" 600 door_a::run,
    0xd8ce4 "set_28" 300 zone_end::run,
    0xd8dce "set_29" 600 door_b::run,
    0xd801e "set_25" 300 rearguard::run,
    0xd810a "set_26" 300 idle_hunter::run,
    0xd69c2 "set_22" 300 rank_adjust::run,
    0xd6c5c "set_23" 500 exit_gate::run,
    0xd5f5a "set_20" 600 life_bar::run,
    0xd6372 "set_21" 500 continue_countdown::run,
    0xd466a "set_17" 300 boss_intro::run,
    0xd49a2 "set_19" 400 wave_spawner::run,
    0xd4950 "set_18" 200 scroll_to::run,
}
