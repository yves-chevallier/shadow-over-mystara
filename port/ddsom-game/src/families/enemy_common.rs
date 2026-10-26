//! Éléments communs aux gestionnaires de la classe ENEMY (ennemis, pool de 22 × 0xC0 octets) :
//! champs, états, allocation de sous-objets, mort, aides de rang et de pause.
//! Voir `docs/familles/enemy.md`.
//!
//! Tous les ennemis ont la même charpente : l'octet `+4` est l'état principal (0 création, 2 actif,
//! 4 touché, 6 mort), `+5` et `+6` des sous-états ; les valeurs sont des octets pairs qui indexent
//! des tables de sauts. L'état « mort » libère l'emplacement (`0x21f4`).

use m68k::SR_C;

use super::cx::Cx;
use crate::objects::Obj;

/// Champs des objets ENEMY.
pub mod f {
    /// État principal, sous-état, sous-sous-état.
    pub const STATE: u32 = 0x4;
    pub const SUB: u32 = 0x5;
    pub const SUB2: u32 = 0x6;
    /// Jeu de boîtes de collision (voir `docs/combat.md` §3).
    pub const BOX_SET: u32 = 0x2e;
    /// Orientation (non nul : regarde à gauche).
    pub const FLIP: u32 = 0x2c;
    /// Hôte (mot, adresse RAM) d'une partie d'ennemi composite.
    pub const HOST: u32 = 0x46;
    /// Parent ou propriétaire (mot, adresse RAM) ; `+0x42` : créateur d'un sous-objet.
    pub const OWNER: u32 = 0x50;
    pub const TARGET: u32 = 0x50;
    pub const CREATOR: u32 = 0x42;
    /// Compteur de gel après un coup (décompté par `0xf1bfc`).
    pub const FREEZE: u32 = 0x53;
    /// Points de vie (mot signé) et valeur mémorisée au dernier coup.
    pub const HP: u32 = 0x62;
    /// Immunités (long : un bit par comportement de tir).
    pub const IMMUNITY: u32 = 0x94;
}

/// États principaux (`+4`), valeurs longues pour `Cx::set32(0x4, …)` : état dans l'octet haut.
pub mod st {
    pub const INIT: u8 = 0;
    pub const ACTIVE: u8 = 2;
    pub const HIT: u8 = 4;
    pub const DEAD: u8 = 6;
    /// Long d'état « mort » (`+4..+7`) : libération au prochain passage.
    pub const DEAD_LONG: u32 = 0x0600_0000;
}

/// Routines du moteur appelées par les ennemis.
pub mod rom {
    pub const FREE_ENEMY: u32 = 0x21f4;
    pub const ALLOC_ENEMY: u32 = 0x22bc;
    pub const ALLOC_ESHL: u32 = 0x22ea;
    pub const ALLOC_EFFCT: u32 = 0x2318;
    pub const ALLOC_ITEM: u32 = 0x23d8;
    /// PV initiaux selon le type, la difficulté et le nombre de joueurs.
    pub const INITIAL_HP: u32 = 0x4544;
    /// Immunités du type (rangées en `+0x94`).
    pub const IMMUNITIES: u32 = 0xfe5a;
    pub const RNG: u32 = 0x19a88;
    /// Compteurs de fin de handler (gel `+0x53`, compte à rebours `+0x89`).
    pub const TICK_COUNTERS: u32 = 0xf1bfc;
    /// Variable `A5+0x3d` : jeu en pause.
    pub const PAUSE_FLAG: u32 = 0x3d;
}

/// Alloue un objet du pool `alloc` (adresse d'une routine d'allocation) et rend son adresse (`A1`).
/// L'original n'examine pas la retenue : il écrit à travers `A1` même si le pool est plein.
pub fn alloc(x: &mut Cx, alloc: u32) -> Obj {
    x.call(alloc);
    Obj(x.c.a[1])
}

/// Alloue un tir ESHL actif de type `kind`, avec la variante `variant`, lié à l'objet courant par le mot `link`
/// (`None` si le pool est plein). Les créateurs ci-dessous sont les petites routines 0x9ddee..0xa6824 de l'original.
fn spawn_eshl(x: &mut Cx, kind: u8, variant: u8, link: u32) -> Option<Obj> {
    let o = alloc(x, rom::ALLOC_ESHL);
    if alloc_failed(x) {
        return None;
    }
    o.set_u8(x.b, 0, 1);
    o.set_u8(x.b, 2, kind);
    o.set_u8(x.b, 3, variant);
    o.set_u16(x.b, link, x.o.0 as u16);
    Some(o)
}

/// 0x9ddee : un tir ESHL 1 de variante D1, indice `+0x2f` = 1, propriétaire `+0x50`.
pub fn eshl_thrown_by_beast(x: &mut Cx) {
    let variant = x.c.d[1] as u8;
    if let Some(o) = spawn_eshl(x, 1, variant, 0x50) {
        o.set_u8(x.b, 0x2f, 1);
    }
}

/// 0xa2f96 : seize tirs ESHL 0x27 (variantes 15 à 0) liés par `+0x5c`.
pub fn eshl_burst_27(x: &mut Cx) {
    for variant in (0..16).rev() {
        x.c.d[1] = x.c.d[1] & !0xffff | variant;
        if spawn_eshl(x, 0x27, variant as u8, 0x5c).is_none() {
            return;
        }
    }
    x.c.d[1] |= 0xffff;
}

/// 0xa6824 : deux tirs ESHL 0x3f (orientation 0 puis 0x20) liés par `+0x5c`.
pub fn eshl_pair_3f(x: &mut Cx) {
    for flip in [0, 0x20] {
        let Some(o) = spawn_eshl(x, 0x3f, 0, 0x5c) else { return };
        o.set_u8(x.b, f::FLIP, flip);
    }
}

/// 0xa460a, 0xa464e, 0xa4676 : un tir ESHL 0x34 de variante 2, 1 ou 0 lié par `+0x46` (la variante 1 copie
/// l'orientation du créateur).
pub fn eshl_shot_34(x: &mut Cx, variant: u8) {
    let flip = x.u8(f::FLIP);
    if let Some(o) = spawn_eshl(x, 0x34, variant, f::HOST) {
        o.set_u8(x.b, 0x2f, 0);
        if variant == 1 {
            o.set_u8(x.b, f::FLIP, flip);
        }
    }
}

/// Vrai si l'allocation a échoué (retenue levée par la routine d'allocation).
pub fn alloc_failed(x: &Cx) -> bool {
    x.c.sr & SR_C != 0
}

/// Libère l'ennemi courant (état « mort » traité).
pub fn free(x: &mut Cx) {
    x.call(rom::FREE_ENEMY);
}

/// Tirage pseudo-aléatoire du jeu (mot dans D0).
pub fn random(x: &mut Cx) -> u32 {
    x.call(rom::RNG);
    x.c.d[0]
}

/// Jeu en pause (`A5+0x3d` non nul).
pub fn paused(x: &mut Cx) -> bool {
    x.a5_u8(rom::PAUSE_FLAG) != 0
}

/// Le gestionnaire est suspendu (hook de pause `0x29ea`, Z = non suspendu).
pub fn suspended(x: &mut Cx) -> bool {
    x.call(super::cx::sub::PAUSE_HOOK);
    x.c.cond(6)
}

/// Objet désigné par un mot de 16 bits de l'objet courant (`movea.w (off,A0),A1`).
pub fn link(x: &mut Cx, off: u32) -> Obj {
    x.linked(off)
}

/// Écrit le long d'état (`+4..+7`) d'un autre objet.
pub fn set_state(x: &mut Cx, o: Obj, state: u32) {
    o.set_u32(x.b, f::STATE, state);
}

/// Test « dans la zone de jeu » et ombre portée, fin commune des états actifs.
pub const IN_ZONE: u32 = 0xfbae;
pub const SHADOW: u32 = 0x33fe4;

/// Boucle commune des états actifs des ennemis à IA : tant que le gel `+0x53` est positif il décroît
/// et rien d'autre ne bouge ; sinon, hors pause, `sub` exécute le sous-état (il rend vrai pour
/// abandonner l'image sans la suite, comme l'original qui dépile son adresse de retour). La suite
/// teste la zone et dessine l'ombre dès que le sous-état n'est plus 0, puis remet l'objet en liste.
pub fn active_loop(x: &mut Cx, sub: impl FnOnce(&mut Cx) -> bool) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else if !paused(x) && sub(x) {
        return;
    }
    if x.u8(f::SUB) != 0 {
        x.call(IN_ZONE);
        x.call(SHADOW);
    }
    x.reinsert();
}

/// Boucle commune de l'état « touché » : même gel ; la zone n'est testée que si `+0xae` est nul.
pub fn hit_loop(x: &mut Cx, sub: impl FnOnce(&mut Cx)) {
    hit_loop_at(x, 0xae, sub)
}

/// `hit_loop` avec l'octet « pas de test de zone » à un autre décalage (`+0xa5` pour le type 4).
pub fn hit_loop_at(x: &mut Cx, no_zone: u32, sub: impl FnOnce(&mut Cx)) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        sub(x);
    }
    if x.u8(no_zone) == 0 {
        x.call(IN_ZONE);
        x.call(SHADOW);
    }
    x.reinsert();
}

/// Long lu dans une table du programme (adressage relatif au PC, espace des opcodes).
pub fn rom32(x: &mut Cx, addr: u32) -> u32 {
    (x.rom16(addr) as u32) << 16 | x.rom16(addr + 2) as u32
}

/// Réaction à un coup : la routine commune, choisie dans `table` (longs, indexés par le code de
/// réaction `+0x49`) par la collision, joue l'animation de recul, de chute ou de mort.
pub fn react(x: &mut Cx, table: u32) {
    let code = x.u8(0x49) as u32;
    x.c.d[0] = code;
    let routine = rom32(x, table + code);
    x.c.a[1] = routine;
    x.call(routine);
}

/// `move.w #v,Dn` : ne change que le mot de poids faible du registre.
pub fn set_dw(x: &mut Cx, n: usize, v: u16) {
    x.c.d[n] = x.c.d[n] & 0xffff_0000 | v as u32;
}

/// `add.b #v,(off,An)` sur un autre objet (modulo 2^8).
pub fn add8(x: &mut Cx, o: Obj, off: u32, v: u8) {
    let n = o.u8(x.b, off).wrapping_add(v);
    o.set_u8(x.b, off, n);
}

/// `add.w #v,(off,An)` sur un autre objet (modulo 2^16).
pub fn add16(x: &mut Cx, o: Obj, off: u32, v: u16) {
    let n = o.u16(x.b, off).wrapping_add(v);
    o.set_u16(x.b, off, n);
}

/// Démarre le script `n` du jeu d'animations `set` (`movea.l set,A4 ; moveq n,D0 ; jsr 0x1a32`).
pub fn anim(x: &mut Cx, set: u32, n: u32) {
    x.anim_start(set, n);
}

// --- aides communes des types du troisième lot --------------------------------------------------

/// Fin commune des sous-états actifs : test de zone (0xfbae, qui remet D5 à 0) puis ombre (0x33fe4).
pub fn zone_shadow(x: &mut Cx) {
    x.call(IN_ZONE);
    x.call(SHADOW);
}

/// `neg.w (off,A0)`.
pub fn negate(x: &mut Cx, off: u32) {
    let v = x.u16(off).wrapping_neg();
    x.set16(off, v);
}

/// `movea.w (0x50,A0),A1` : A1 = la cible (adresse étendue en signe), qui est rendue.
pub fn load_target(x: &mut Cx) -> Obj {
    x.c.a[1] = x.u16(f::TARGET) as i16 as i32 as u32;
    x.linked(f::TARGET)
}

/// Tirage `rand & mask` sur le mot de D0 (`andi.w #mask,D0w` : le mot haut de D0 reste celui du tirage).
pub fn rand_and(x: &mut Cx, mask: u32) -> u32 {
    let r = random(x);
    x.c.d[0] = x.c.d[0] & 0xffff_0000 | r & mask;
    r & mask
}

/// Écrit le long d'état de l'objet courant (`move.l #v,(4,A0)`).
pub fn set_state_long(x: &mut Cx, v: u32) {
    x.set32(f::STATE, v);
}

// --- aides d'IA communes ------------------------------------------------------------------------

/// Un joueur est-il dans cette boîte devant l'ennemi ? (0xb5e0 : décalage, portée, profondeur.)
pub fn target_in_box(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> bool {
    set_dw(x, 0, d0 as u16);
    set_dw(x, 1, d1);
    set_dw(x, 2, d2);
    x.call(0xb5e0);
    !x.c.cond(7)
}

/// Variante de `target_in_box` pour la routine 0x7f0c (même convention).
pub fn target_in_box_far(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> bool {
    set_dw(x, 0, d0 as u16);
    set_dw(x, 1, d1);
    set_dw(x, 2, d2);
    x.call(0x7f0c);
    !x.c.cond(7)
}

/// Choisit un joueur comme cible (0x424e) ; faux si aucun n'est vivant.
pub fn pick_target(x: &mut Cx) -> bool {
    x.call(0x424e);
    !x.c.cond(7)
}

/// L'ennemi est saisi par un joueur (0x2810).
pub fn grabbed(x: &mut Cx) -> bool {
    x.call(0x2810);
    !x.c.cond(7)
}

/// La cible de l'ennemi (joueur désigné par `+0x50`), ou `None` s'il n'en a pas.
pub fn target(x: &mut Cx) -> Option<Obj> {
    if x.u16(f::TARGET) == 0 {
        return None;
    }
    Some(x.linked(f::TARGET))
}

/// L'ennemi ne regarde pas vers sa cible (0x74eac, 0x5f710) : faux sans animation en cours
/// (`+0xa2` nul) ou sans cible.
pub fn needs_turn(x: &mut Cx) -> bool {
    if x.u8(0xa2) == 0 {
        return false;
    }
    let Some(t) = target(x) else { return false };
    let dx = t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)) as i16;
    let facing = if dx > 0 { 0x20 } else { 0 };
    facing != x.u8(f::FLIP)
}

// --- aides des types du quatrième lot -----------------------------------------------------------

/// 0x67452 : cherche le joueur le plus proche dans une boîte devant l'ennemi (décalage `d0`, demi-largeur
/// `d1`, demi-profondeur `d2`) et le range en `+0x50` ; faux si aucun. La boîte suit l'orientation de l'ennemi.
/// Les registres de travail sont ceux de l'original (D1 = meilleure distance, D4 = doubles largeurs...).
pub fn find_player(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> bool {
    x.set16(f::TARGET, 0);
    let d0 = if x.u8(f::FLIP) != 0 { d0.wrapping_neg() } else { d0 } as u16;
    let mut best = 0xffffu16;
    let mut d3 = 0u16;
    let mut d5 = x.c.d[5] as u16;
    for k in 0..4u32 {
        let p = 0xff85de + 0x200 * k;
        if crate::mem::rd8(x.b, p) == 0 {
            continue;
        }
        let dy = x.u16(0x10).wrapping_sub(crate::mem::rd16(x.b, p + 0x10)).wrapping_add(d2);
        d5 = dy;
        if dy > d2.wrapping_mul(2) {
            continue;
        }
        d3 = dy;
        let dx = x.u16(0x8).wrapping_add(d0).wrapping_sub(crate::mem::rd16(x.b, p + 0x8)).wrapping_add(d1);
        d5 = dx;
        if dx > d1.wrapping_mul(2) {
            continue;
        }
        d3 = dx.wrapping_add(dy);
        if d3 <= best {
            best = d3;
            x.set16(f::TARGET, p as u16);
        }
    }
    x.c.d[0] = x.c.d[0] & !0xffff | d0 as u32;
    x.c.d[1] = 0xffff_0000 | best as u32;
    x.c.d[2] = (d1 as u32) << 16 | d2 as u32;
    x.c.d[3] = d3 as u32;
    x.c.d[4] = (d1.wrapping_mul(2) as u32) << 16 | d2.wrapping_mul(2) as u32;
    x.c.d[5] = x.c.d[5] & !0xffff | d5 as u32;
    x.c.d[6] = 0xffff;
    x.c.a[1] = 0xff8dde;
    x.u16(f::TARGET) != 0
}

/// Crée les objets d'une liste de la ROM : `lists` (pointeurs, espace des opcodes) donne la liste `index`, dont
/// chaque entrée est `routine d'allocation (octet), type, variante, +0x2f, X, hauteur, profondeur` ; le créateur
/// est rangé en `+0x5c` de chaque objet. Les allocations qui échouent sautent leur entrée.
pub fn spawn_list(x: &mut Cx, lists: u32, allocs: u32, index: u32) {
    let mut at = rom32(x, lists + 4 * index);
    x.c.a[2] = at;
    let count = x.data16(at);
    at += 2;
    for _ in 0..=count {
        let routine_index = x.data8(at) as u32;
        at += 1;
        let routine = rom32(x, allocs + 4 * routine_index);
        let o = alloc(x, routine);
        if !alloc_failed(x) {
            let (kind, variant, group) = (x.data8(at), x.data8(at + 1), x.data8(at + 2));
            let (px, ph, pz) = (x.data16(at + 3), x.data16(at + 5), x.data16(at + 7));
            o.set_u8(x.b, 0, 1);
            o.set_u8(x.b, 2, kind);
            o.set_u8(x.b, 3, variant);
            o.set_u8(x.b, 0x2f, group);
            o.set_u16(x.b, 0x8, px);
            o.set_u16(x.b, 0xc, ph);
            o.set_u16(x.b, 0x10, pz);
            o.set_u16(x.b, 0x5c, x.o.0 as u16);
        }
        at += 9;
    }
    x.c.a[2] = at;
}

/// 0x27ee : l'ennemi se détache de son hôte (`HOST`) : si l'hôte est actif et compte encore des parties
/// (`+0x2a`), le compte diminue et le lien est effacé. D0 et A1 gardent le lien et l'hôte, comme l'original.
fn detach_from_host(c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus) {
    use crate::mem::{rd16, rd8, rts, wr16};
    const PARTS: u32 = 0x2a;
    let me = c.a[0];
    let host = rd16(b, me + f::HOST);
    c.d[0] = c.d[0] & 0xffff_0000 | host as u32;
    if host != 0 {
        c.a[1] = host as i16 as i32 as u32;
        let parts = rd16(b, c.a[1] + PARTS);
        if rd8(b, c.a[1]) != 0 && parts != 0 {
            wr16(b, c.a[1] + PARTS, parts - 1);
            wr16(b, me + f::HOST, 0);
        }
    }
    rts(c, b);
}

pub const NATIVES: &[crate::Native] = &[crate::Native {
    entry: 0x27ee,
    name: "ennemi_detacher_hote",
    cost: |_, _| 60,
    run: detach_from_host,
    touches_sound: false,
    outputs: crate::regs::d(0) | crate::regs::a(1),
    exact_cost: false,
}];
