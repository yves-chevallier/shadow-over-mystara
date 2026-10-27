//! Boss 7 : aides communes (choix d'un joueur, balayage des joueurs, positions, choix de l'attaque suivante).

use crate::objects::Obj;
use super::*;
use crate::mem::A5;
use crate::machine::Machine;

/// 0x86fee : choisit au hasard un joueur en vie pas encore désigné (bit dans `+0xaf`) comme cible `+0x50`.
pub(super) fn pick_new(x: &mut Cx) {
    pick(x, true)
}

/// 0x87020 : idem sans mémoriser le choix.
pub(super) fn pick_any(x: &mut Cx) {
    pick(x, false)
}

fn pick(x: &mut Cx, remember: bool) {
    x.c.a[1] = 0;
    for _ in 0..4096 {
        x.c.a[2] = A5 + 0x5de;
        let alea = x.random();
        let n = alea as u32 & 3;
        setw(x, 0, n as u16);
        if remember && x.u8(PICKED) >> n & 1 != 0 {
            continue;
        }
        let o = (n as u16) << 9;
        setw(x, 0, o);
        x.c.a[2] = x.c.a[2].wrapping_add(o as i16 as i32 as u32);
        if crate::mem::rd8(x.b, x.c.a[2]) == 0 {
            continue;
        }
        if remember {
            let mask = x.u8(PICKED) | 1 << n;
            x.set8(PICKED, mask);
        }
        x.c.a[1] = x.c.a[2];
        x.set16(f::TARGET, x.c.a[1] as u16);
        return;
    }
}

/// Balayage des joueurs en vie (4 × 0x200 octets depuis `A5+0x5de`) : la boîte (`d0`, `d1`, `d2`) devant le boss
/// (0x7f19c) contient-elle l'un d'eux ? D6 est le compteur de boucle du 68000.
fn scan(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> bool {
    x.c.d[6] = 3;
    x.c.a[2] = A5 + 0x5de;
    loop {
        if crate::mem::rd8(x.b, x.c.a[2]) != 0 {
            let player = Obj(x.c.a[2]);
            if lib::in_box(x, player, d0, d1, d2).found() {
                return true;
            }
        }
        x.c.a[2] = x.c.a[2].wrapping_add(0x200);
        let k = (x.c.d[6] as u16).wrapping_sub(1);
        setw(x, 6, k); // relu : compteur de boucle relu au tour suivant
        if k == 0xffff {
            return false;
        }
    }
}

/// 0x87c9a (`far`) et 0x87ccc : un joueur en vie est-il dans la boîte ? D0 = 0 ou 1.
pub(super) fn player_near(x: &mut Cx, far: bool) -> bool {
    let found = if far { scan(x, -0x130, 0x40, 0x20) } else { scan(x, -0x20, 0x20, 8) };
    x.c.d[0] = found as u32;
    found
}

/// 0x87b86 : vrai si le boss ne regarde pas son adversaire (`+0x50`) et qu'une animation est en cours (`+0xa3`).
pub(super) fn must_turn(x: &mut Cx) -> bool {
    if x.u8(ANIM_LAST) == 0 || x.u16(f::TARGET) == 0 {
        return false;
    }
    let t = load_target(x);
    let mine = x.u16(0x8);
    let r = Fl::sub16(t.u16(x.b, 0x8), mine);
    let face = if x.u8(f::FLIP) != 0 {
        if r.gt() { 0x20 } else { 0 }
    } else if r.le() {
        0
    } else {
        0x20
    };
    if face == x.u8(f::FLIP) {
        return false;
    }
    true
}

/// 0x87c6c : pas de côté tiré dans la table 0x87c8a.
pub(super) fn random_sidestep(x: &mut Cx) {
    let alea = x.random();
    let i = alea & 0xe;
    let mut d = x.rom16(0x87c8a + i as u32);
    if x.u8(f::FLIP) != 0 {
        d = d.wrapping_neg();
    }
    x.add16(0x8, d);
}

/// 0x87cfe : ramène le boss dans l'aire de jeu (x de 0x45f à 0x68f, y de 0x168 à 0x19f).
pub(super) fn clamp(x: &mut Cx) {
    let y = x.u16(0x10);
    if Fl::sub16(y, 0x168).gt() {
        if !Fl::sub16(y, 0x19f).lt() {
            x.set16(0x10, 0x19f);
        }
    } else {
        x.set16(0x10, 0x168);
    }
    let px = x.u16(0x8);
    if Fl::sub16(px, 0x45f).gt() {
        if !Fl::sub16(px, 0x68f).lt() {
            x.set16(0x8, 0x68f);
        }
    } else {
        x.set16(0x8, 0x45f);
    }
}

/// Une série de bouffées d'effets (0xb10c8) le long de l'axe : D0 = 1, D1 = 0, D2 = position, D3 = 4.
fn bursts(x: &mut Cx, spots: &[i16]) {
    for &p in spots {
        lib::shock(x, 1, 0, p as u16, 4);
    }
}

/// 0x87d48 : cinq bouffées.
pub(super) fn burst_small(x: &mut Cx) {
    bursts(x, &[-0x28, -0x18, 0, 0x18, 0x28]);
}

/// 0x87da2 : sept bouffées vers l'avant.
pub(super) fn burst_front(x: &mut Cx) {
    bursts(x, &[-0x50, -0x38, -0x20, -8, 0x10, 0x28, 0x40]);
}

/// 0x87e20 : sept bouffées vers l'arrière.
pub(super) fn burst_back(x: &mut Cx) {
    bursts(x, &[0x50, 0x38, 0x20, 8, -0x10, -0x28, -0x40]);
}

/// 0x87e9e : si un joueur est tout près (0x7f19c : 0, 0x50, 0x20), le boss saute ailleurs (0x87eca).
pub(super) fn dodge_nearby(x: &mut Cx) {
    if scan(x, 0, 0x50, 0x20) {
        dodge_spot(x);
    }
}

/// 0x87eca : un point de chute tiré dans une table choisie selon le quart d'écran où se trouve le boss.
pub(super) fn dodge_spot(x: &mut Cx) {
    let dx = x.u16(0x8).wrapping_sub(var::SCROLL_X.get(x.b));
    let dy = x.u16(0x10).wrapping_sub(var::SCROLL_Y.get(x.b));
    let right = Fl::sub16(dx, 0xc0).gt();
    let low = Fl::sub16(dy, 0x64).gt();
    let d6 = match (right, low) {
        (false, false) => 0,
        (false, true) => 4,
        (true, false) => 8,
        (true, true) => 0xc,
    };
    let table = rom32(x, 0x87f30 + d6);
    let alea = x.random();
    let i = ((alea) & 3) << 3;
    let a = table.wrapping_add(i as u32);
    let dx = x.data16(a);
    x.add16(0x8, dx);
    let dy = x.data16(a + 2);
    x.add16(0x10, dy);
    let side = x.data8(a + 4);
    x.set8(f::FLIP, side);
    clamp(x)
}

/// 0x870a2 (`front`) / 0x870b4 : tire la prochaine attaque dans la table de la ROM (selon le ralenti et la
/// difficulté). Ralenti, la troisième attaque 4 de suite devient l'attaque 14.
pub(super) fn choose_attack(x: &mut Cx, front: bool) {
    let slow = x.u8(SLOW) != 0;
    let table = match (front, slow) {
        (true, false) => 0x87126,
        (true, true) => 0x87526,
        (false, false) => 0x87326,
        (false, true) => 0x87726,
    };
    pick_row(x, table, 0x204, true);
    if slow && x.u8(f::SUB2) == 4 {
        addq8(x, THIRD, 1);
        if x.u8(THIRD) == 3 {
            x.set32(f::STATE, 0x0204_0e00);
            x.set8(THIRD, 0);
        }
    }
}

/// Choisit une ligne de 32 octets de `table` selon la difficulté, en tire une étape et passe à l'état `state` (mot `+4`).
fn pick_row(x: &mut Cx, table: u32, state: u16, clear_d0: bool) {
    if clear_d0 {
        x.c.d[0] = 0;
    }
    let level = var::RANK.get(x.b);
    setw(x, 0, level >> 1);
    let row = ((x.c.d[0] as u16) << 5) as i16 as i32 as u32;
    let a4 = table.wrapping_add(row);
    let alea = x.random();
    let i = alea & 0x1f;
    let step = x.data8(a4 + i as u32);
    x.set8(f::SUB2, step);
    x.set16(f::STATE, state);
    x.set8(f::SUB3, 0);
}

/// 0x87926 : fin d'une attaque. Un joueur en vie dans la zone (0x7f19c : -0x38, 0x38, 0x18) fait tirer la suite dans
/// la table 0x87986 (état 2, groupe 8) ; sinon le boss revient au combat.
pub(super) fn after_attack(x: &mut Cx) {
    if !scan(x, -0x38, 0x38, 0x18) {
        x.set32(f::STATE, FIGHT);
        return;
    }
    pick_row(x, 0x87986, 0x208, false);
}
