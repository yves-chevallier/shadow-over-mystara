//! Objets de décor (classe SET, pool de 32 objets de 0x50 octets, table 0x39eac) : types 0, 1 et
//! 3 à 14. Les objets SET ne figurent pas dans les listes triées (voir `docs/objets.md`).

use crate::families::cx::Banc;
use super::cx::{Cx, Gx};
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Courant;
use crate::machine::Machine;

/// Type 8 (0xcc25a), partagé par les tables SET (8), SOBJ (4) et EFFCT (18) : générateur du
/// contrôleur de scène. À la première image il attend ; à la suivante il crée un objet SET de
/// type 2 (le contrôleur, de variante `+0x2f`) puis passe à l'état 6. Dans les états 4 et 6 il se
/// libère dans le pool de sa classe (EFFCT, SET ou SOBJ selon `+0x28`).
///
/// Variante `+3` : la table de sauts de l'original n'a qu'une entrée (la variante 1 retombe sur la même
/// cible), la variante 2 saute au milieu d'un `jsr` et au-delà dans des données : hors contrat, la
/// réécriture traite toute variante comme 0 (le fuzz ne tire que 0 et 1).
mod scene_spawner {
    use super::*;

    /// Table (espace programme) des routines de libération, indexée par `(classe - 0xe) * 2` octets.
    const FREE_BY_CLASS: u32 = 0xcc2b8;
    const WAIT: u8 = 0;
    const SPAWN: u8 = 2;
    const SCENE_CONTROLLER: u8 = 2;
    /// Paramètre du créateur : variante du contrôleur à créer.
    const VARIANT_PARAM: u32 = 0x2f;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            WAIT => x.add8(0x4, 2),
            SPAWN => {
                let Some(child) = x.alloc(&pools::SET) else { return };
                child.set_u8(x.b, 0, 1);
                child.set_u8(x.b, 2, SCENE_CONTROLLER);
                let variant = x.u8(VARIANT_PARAM);
                child.set_u8(x.b, 3, variant);
                x.set8(0x4, 6);
            }
            _ => {
                let class = x.u8(0x28) as u32;
                let offset = (class.wrapping_sub(0xe) as u16).wrapping_mul(2) as i16;
                let at = FREE_BY_CLASS.wrapping_add(offset as i32 as u32);
                let free = (x.rom16(at) as u32) << 16 | x.rom16(at + 2) as u32;
                x.call(free);
            }
        }
    }
}

/// Aides communes aux types de la classe SET.
mod util {
    use super::*;
    use crate::mem::{rd16, wr16, A5};

    /// Premier enregistrement joueur (A5 + 0x5de), quatre joueurs espacés de 0x200 octets.
    pub const PLAYERS: u32 = A5 + 0x5de;
    pub const PLAYER_STRIDE: u32 = 0x200;
    /// Champs d'un joueur : actif (octet), points de vie (mot).
    pub const P_ACTIVE: u32 = 0x0;
    pub const P_LIFE: u32 = 0x62;
    /// Variable A5 : la partie est en cours (non nul).
    pub const IN_GAME: u32 = 0x35;

    pub fn players() -> impl Iterator<Item = Obj> {
        (0..4).map(|i| Obj(PLAYERS + PLAYER_STRIDE * i))
    }

    /// Joueur présent dans la partie : emplacement occupé et points de vie non nuls.
    pub fn alive<R: Banc>(x: &mut Cx<'_, R>, p: Obj) -> bool {
        p.u8(x.b, P_ACTIVE) != 0 && p.u16(x.b, P_LIFE) != 0
    }

    /// Avance chaque composante (rouge, vert, bleu : un quartet chacune) d'une couleur 4+4+4 bits
    /// d'un cran vers son maximum, et force le quartet de luminosité à 0xf.
    pub fn brighten<R: Banc>(x: &mut Cx<'_, R>, addr: u32, count: u32) {
        for i in 0..count {
            let a = addr + 2 * i;
            let c = rd16(x.b, a);
            let step = |mask: u16, one: u16| {
                let v = c & mask;
                if v == mask { v } else { v + one }
            };
            wr16(x.b, a, 0xf000 + step(0xf, 1) + step(0xf0, 0x10) + step(0xf00, 0x100));
        }
    }
}

/// Type 9 (0xcc2c8) : objet inerte. Suspendu (`+0x40`), il ne fait rien ; sinon l'état 0 passe à
/// l'état 2, qui ne fait rien non plus (l'original y teste une collision avec le joueur A6 dont
/// aucun résultat n'est utilisé) ; les états 4 et 6 le libèrent.
mod inert {
    use super::*;

    pub fn run<R: Banc>(x: &mut Cx<'_, R>) {
        if x.u8(0x40) != 0 {
            return;
        }
        match x.u8(0x4) {
            0 => x.add8(0x4, 2),
            2 => {}
            _ => x.free(&pools::SET),
        }
    }
}

/// Type 11 (0xcdcaa) : fin de niveau de la partie à bonus (h). Quand la partie est en cours et que
/// la variable `A5+0x556` dépasse 0x320, il attend que tous les joueurs vivants soient à l'état
/// 0x200 (+4/+5) sans sous-état, puis les remet à l'état 0x80000, éteint leur arme (`+0x8a`) et leur
/// numérote l'ordre (`+0x7d`), fixe `A5+0x2c` et passe à l'état 4 (libération à l'image suivante).
mod level_end {
    use super::*;
    use util::*;

    const PROGRESS: u32 = 0x556;
    const PROGRESS_LIMIT: i16 = 0x320;
    const RESET_STATE: u32 = 0x80000;
    const END_FLAGS: u32 = 0x18000000;
    const P_STATE: u32 = 0x4;
    const P_SUBSTATE: u32 = 0x6;
    const P_ORDER: u32 = 0x7d;
    const P_WEAPON: u32 = 0x8a;
    const P_WEAPON_OBJ: u32 = 0x86;

    pub fn run<R: Banc>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                wait_players(x);
            }
            2 => wait_players(x),
            _ => x.free(&pools::SET),
        }
    }

    fn wait_players<R: Banc>(x: &mut Cx<'_, R>) {
        if x.a5_u8(IN_GAME) == 0 || (x.a5_u16(PROGRESS) as i16) <= PROGRESS_LIMIT {
            return;
        }
        x.set_a5_u8(0x43, 0);
        x.set_a5_u16(0x51e, 0);
        x.set_a5_u8(0x4ff, 4);
        for p in players() {
            if alive(x, p) && (p.u16(x.b, P_STATE) != 0x200 || p.u8(x.b, P_SUBSTATE) != 0) {
                return;
            }
        }
        let mut order = 0;
        for p in players() {
            if !alive(x, p) {
                continue;
            }
            p.set_u32(x.b, P_STATE, RESET_STATE);
            p.set_u8(x.b, 0x66, 0);
            p.set_u8(x.b, 0x15c, 0);
            if p.u8(x.b, P_WEAPON) != 0 {
                p.set_u8(x.b, P_WEAPON, 0);
                let weapon = Obj(p.u32(x.b, P_WEAPON_OBJ));
                weapon.set_u8(x.b, 5, 8);
                weapon.set_u16(x.b, 6, 0);
            }
            p.set_u8(x.b, P_ORDER, order);
            order += 1;
        }
        x.set_a5_u32(0x2c, END_FLAGS);
        x.add8(0x4, 2);
    }
}

/// Type 12 (0xcdd70) : fondu au blanc des palettes (h). L'état 0 note `+0x3c` = 0xf (non relu) ;
/// l'état 2, à chaque image, éclaircit d'un cran les couleurs de quatre blocs de la RAM palette
/// (4 palettes de 0x91c100, puis 0x91c720, 0x91c800 et 0x91cc00 : 32 couleurs chacun). Un autre
/// objet l'arrête en changeant son état (4 ou 6 : libération).
mod fade_white {
    use super::*;

    const PALETTE_BLOCKS: [(u32, u32); 4] = [(0x91c100, 128), (0x91c720, 32), (0x91c800, 32), (0x91cc00, 32)];

    pub fn run<R: Banc>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(0x3c, 0xf);
            }
            2 => {
                for (addr, count) in PALETTE_BLOCKS {
                    util::brighten(x, addr, count);
                }
            }
            _ => x.free(&pools::SET),
        }
    }
}

/// Type 13 (0xcdef6) : animation de palette (h). La variante choisit la palette de destination
/// (0x91c000 + 32 octets par variante) ; une table de 17 palettes en ROM (0x291f00, ou 0x291f00 +
/// 0x360 quand `A5+0x70` est non nul) est recopiée, de la dernière à la première, toutes les 8
/// images. Après la dernière il se libère.
mod palette_anim {
    use super::*;
    use crate::mem::{rd32, wr32};

    const PALETTE_RAM: u32 = 0x91c000;
    const PALETTE_BYTES: u32 = 32;
    const TABLES: u32 = 0x291f00;
    const ALT_TABLE_OFFSET: u32 = 0x360;
    const ALT_TABLE_FLAG: u32 = 0x70;
    const STEPS: u8 = 0x10;
    const PERIOD: u8 = 8;
    /// Champs : destination, table source, compte à rebours, palette courante.
    const DEST: u32 = 0x20;
    const SOURCE: u32 = 0x24;
    const TICK: u32 = 0x29;
    const STEP: u32 = 0x2a;

    pub fn run<R: Banc>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                let variant = x.u8(0x3) as u16;
                x.set32(DEST, PALETTE_RAM + ((variant << 5) as i16 as i32 as u32));
                x.set8(TICK, 1);
                let alt = x.a5_u16(ALT_TABLE_FLAG) != 0;
                x.set32(SOURCE, TABLES + if alt { ALT_TABLE_OFFSET } else { 0 });
                x.set8(STEP, STEPS);
            }
            2 => {
                let t = x.u8(TICK).wrapping_sub(1);
                x.set8(TICK, t);
                if t != 0 {
                    return;
                }
                x.set8(TICK, PERIOD);
                let step = ((x.u8(STEP) as u16) << 5) as i16 as i32 as u32;
                let dest = x.u32(DEST);
                let src = x.u32(SOURCE).wrapping_add(step);
                for i in (0..PALETTE_BYTES).step_by(4) {
                    let v = rd32(x.b, src + i);
                    wr32(x.b, dest + i, v);
                }
                let s = x.u8(STEP).wrapping_sub(1);
                x.set8(STEP, s);
                if (s as i8) < 0 {
                    x.add8(0x4, 2);
                    x.free(&pools::SET);
                }
            }
            4 => {
                x.add8(0x4, 2);
                x.free(&pools::SET);
            }
            _ => x.free(&pools::SET),
        }
    }
}

/// Type 14 (0xcdf86) : déclencheur de fin de scène (h). Quand la partie est en cours et que
/// l'abscisse de caméra `A5+0x506` a atteint 0x1c80, il met tous les joueurs vivants dans l'état 2
/// (octet `+0` du joueur), lève `A5+0x42` et `A5+0x4a`, fixe `A5+0x2c` à 0x10000000 et passe à
/// l'état 4 (libération). La variante `+3` n'a aucun effet : les deux branches de l'original sont
/// identiques.
mod scene_trigger {
    use super::*;
    use util::*;

    const CAMERA_X: u32 = 0x506;
    const CAMERA_LIMIT: u16 = 0x1c80;
    const PLAYER_MODE_DONE: u8 = 2;

    pub fn run<R: Banc>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                trigger(x);
            }
            2 => trigger(x),
            _ => x.free(&pools::SET),
        }
    }

    fn trigger<R: Banc>(x: &mut Cx<'_, R>) {
        if x.a5_u8(IN_GAME) == 0 || x.a5_u16(CAMERA_X) < CAMERA_LIMIT {
            return;
        }
        let mut count = 0;
        for p in players() {
            if alive(x, p) {
                p.set_u8(x.b, P_ACTIVE, PLAYER_MODE_DONE);
                count += 1;
            }
        }
        if count == 0 {
            return;
        }
        x.add8(0x4, 2);
        x.set_a5_u8(0x42, 1);
        x.set_a5_u8(0x4a, 1);
        x.set_a5_u32(0x2c, 0x10000000);
    }
}

/// Types 4 (0xc935a) et 6 (0xcafec) : animation de décor par réécriture de cases de tuiles (h).
///
/// La variante `+3` choisit une entrée de 16 octets d'une table de la ROM (espace programme) :
/// adresse du script (`+0x30`), adresse de la carte de tuiles à modifier en mémoire vidéo
/// (`+0x34`), mot `+0x38` non nul pour boucler. Le script est une suite de blocs : durée (mot,
/// négatif = fin), nombre de colonnes, nombre de lignes, puis un long par case. Chaque case de la
/// carte devient `(attribut & 0xff80) + long` : le code de tuile est remplacé, les bits hauts de
/// l'attribut (palette, retournements) sont conservés. Les lignes sont espacées de 0x40 octets
/// (0x20 si `+0x2f` est non nul). En fin de script, l'objet repart de l'état 0 si `+0x38` est non
/// nul, sinon passe à l'état 4 (libération). Le type 4 s'arrête pendant la pause du jeu et quand
/// l'objet est suspendu ; le type 6 ignore la pause, mais laisse intactes les cases dont le
/// premier mot vaut 0x1800.
mod tile_anim {
    use super::*;
    use crate::mem::{rd16, rd32, wr32};

    /// Une entrée de la table des scripts.
    const ENTRY_SIZE_SHIFT: u16 = 4;
    const ENTRY_SCRIPT: u32 = 0;
    const ENTRY_TILEMAP: u32 = 4;
    const ENTRY_LOOP: u32 = 8;
    /// Champs de l'objet.
    const SCRIPT: u32 = 0x30;
    const TILEMAP: u32 = 0x34;
    const LOOP: u32 = 0x38;
    const TIMER: u32 = 0x3a;
    const SMALL_ROWS: u32 = 0x2f;
    const ROW_BYTES: u32 = 0x40;
    const ROW_BYTES_SMALL: u32 = 0x20;
    const ATTR_KEEP: u32 = 0xff80;
    const TRANSPARENT: u16 = 0x1800;

    /// Table des scripts du type 4 et du type 6 (espace programme).
    const TABLE_4: u32 = 0xc93a2;
    const TABLE_6: u32 = 0xcb024;
    const GAME_PAUSED: u32 = 0x3d;

    pub fn run4(x: &mut Cx) {
        if x.a5_u8(GAME_PAUSED) != 0 || x.u8(0x40) != 0 {
            return;
        }
        run(x, TABLE_4, false);
    }

    pub fn run6(x: &mut Cx) {
        run(x, TABLE_6, true);
    }

    fn run(x: &mut Cx, table: u32, skip_transparent: bool) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(TIMER, 0);
                let at = table.wrapping_add((((x.u8(0x3) as u16) << ENTRY_SIZE_SHIFT) as i16) as u32);
                let long = |x: &mut Cx, off: u32| (x.rom16(at + off) as u32) << 16 | x.rom16(at + off + 2) as u32;
                let (script, tilemap, looping) = (long(x, ENTRY_SCRIPT), long(x, ENTRY_TILEMAP), x.rom16(at + ENTRY_LOOP));
                x.set32(SCRIPT, script);
                x.set32(TILEMAP, tilemap);
                x.set16(LOOP, looping);
                step(x, skip_transparent);
            }
            2 => step(x, skip_transparent),
            _ => x.free(&pools::SET),
        }
    }

    /// Un pas : attend la fin de la durée du bloc courant, puis applique le bloc suivant.
    fn step<R: Banc>(x: &mut Cx<'_, R>, skip_transparent: bool) {
        let t = x.u16(TIMER).wrapping_sub(1);
        x.set16(TIMER, t);
        if (t as i16) >= 0 {
            return;
        }
        let mut script = x.u32(SCRIPT);
        let map = x.u32(TILEMAP);
        let duration = rd16(x.b, script);
        script += 2;
        x.set16(TIMER, duration);
        if (duration as i16) < 0 {
            if x.u16(LOOP) != 0 {
                x.set8(0x4, 0);
            } else {
                x.add8(0x4, 2);
            }
            return;
        }
        let cols = rd16(x.b, script).wrapping_sub(1);
        let rows = rd16(x.b, script + 2).wrapping_sub(1);
        script += 4;
        let stride = if x.u8(SMALL_ROWS) != 0 { ROW_BYTES_SMALL } else { ROW_BYTES };
        let mut row_start = map;
        for _ in 0..=rows as u32 {
            for col in 0..=cols as u32 {
                let cell = row_start + 4 * col;
                let tile = rd32(x.b, script);
                script += 4;
                let keep = skip_transparent && rd16(x.b, script - 4) == TRANSPARENT;
                if !keep {
                    let attr = rd16(x.b, cell + 2) as u32 & ATTR_KEEP;
                    wr32(x.b, cell, attr.wrapping_add(tile));
                } else {
                    let old = rd32(x.b, cell);
                    wr32(x.b, cell, old);
                }
            }
            row_start += stride;
        }
        x.set32(SCRIPT, script);
    }
}

/// Type 1 (0xbc518) : meneur de vagues d'ennemis (h), actif seulement en partie (`A5+0x35`).
///
/// Un chef (le propriétaire `+0x46`, objet ENEMY) réclame des vagues de trois ennemis. Le meneur
/// attend d'abord 180 images (`+0x22` descend de 0xf0 à 0x3c, puis il verrouille la caméra par
/// 0x2860), puis enchaîne les sous-états `+5` :
///
/// | `+5` | rôle |
/// |---|---|
/// | 0 | attente de 180 images, puis verrouillage de la caméra |
/// | 2 | lance une vague (`spawn_wave`), puis attend un délai tiré dans une table ; passe à 4 quand `+0x21` ou `+0x20` vaut 2 (vagues vaincues, h) ou après 8 vagues (`+0x25`) ; prévient le chef par `+0x24` = 1 |
/// | 4 | attend que le compteur `+0x2a` du chef vaille 1 |
/// | 6 | attend `+0x30` = 0 ; si `+0x31` est non nul, retire une unité au compteur du chef et termine, sinon lance la vague finale |
/// | 8 | vague finale : premier ennemi de variante 2, résultat de vague 0xff |
/// | 0xa, 0xc | attente de fin de vague puis temporisation de 300 images |
/// | 0xe | attend `+0x30` = 0, retire une unité au compteur du chef et termine |
///
/// L'état principal passe alors à 4 puis 6 (libération).
mod wave_leader {
    use super::*;
    use crate::mem::{wr8, A5};

    /// Délais entre deux vagues (octets, espace programme), indexés par un tirage de 4 bits.
    const DELAYS: u32 = 0xbc63e;
    const CAMERA_LOCK_PARAM: u32 = 0x3c;
    /// Places libres du pool ENEMY (A5+) ; il en faut trois par vague.
    const ENEMY_FREE: u32 = 0x7e62;
    const ENEMIES_PER_WAVE: i16 = 3;
    const WAIT_START: u8 = 0xf0;
    const WAIT_END: u8 = 0x3c;
    const FINAL_WAIT: u16 = 0x12c;
    const MAX_WAVES: u8 = 8;
    const KIND_WING: u8 = 0x13;
    const KIND_LEAD: u8 = 0x14;
    const FINAL_VARIANT: u8 = 2;
    /// Coordonnées de départ du meneur de vague par rapport à la caméra (`A5+0x506`).
    const CAMERA_X: u32 = 0x506;
    const SPAWN_OFFSET_X: u16 = 0x200;
    const SPAWN_Y: u16 = 0x200;
    /// Variable globale (A5 - 0x5ac6) : verdict de la vague pour le reste du jeu (h).
    const WAVE_RESULT: u32 = A5 - 0x5ac6;
    const WAVE_RESULT_FINAL: u8 = 0xff;
    /// Champs du meneur.
    const DEFEATED_A: u32 = 0x20;
    const DEFEATED_B: u32 = 0x21;
    const DEFEATED_TARGET: u8 = 2;
    const WAIT: u32 = 0x22;
    const BUSY: u32 = 0x24;
    const WAVE_COUNT: u32 = 0x25;
    const SIDE: u32 = 0x26;
    const LIMIT: u32 = 0x30;
    const FLAG: u32 = 0x31;
    const DIFF: u32 = 0x32;
    /// Chef : adresse (`+0x46`, aussi copiée dans les ennemis créés), drapeau de fin de vague
    /// (`+0x24`) et compteur d'ennemis en vie (`+0x2a`).
    /// Adresses (mots) des trois ennemis de la dernière vague.
    const WINGS: u32 = 0x2a;
    const BOSS: u32 = 0x46;
    const BOSS_STATE: u32 = 0x4;
    const BOSS_STATE_READY: u8 = 4;
    const BOSS_DONE: u32 = 0x24;
    const BOSS_COUNT: u32 = 0x2a;
    /// Champs des ennemis créés.
    const E_LINK_A: u32 = 0xac;
    const E_LINK_B: u32 = 0xae;
    const E_COUNT: u32 = 0xb0;
    const E_SIDE: u32 = 0xb1;
    const E_WING_SIDE: u32 = 0xbc;
    const E_PARENT: u32 = 0x42;
    const E_ANCHOR: u32 = 0x50;

    pub fn run(x: &mut Cx) {
        if x.a5_u8(util::IN_GAME) == 0 {
            return;
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(DEFEATED_A, 0);
                x.set16(BUSY, 0);
                x.set8(0x5, 0);
                x.set16(0x6, 0);
                x.set8(LIMIT, 0);
                x.set8(FLAG, 0);
                x.set8(DIFF, 0);
                x.set8(SIDE, 0xff);
                x.set8(WAIT, WAIT_START);
                substate(x);
            }
            2 => substate(x),
            4 => x.add8(0x4, 2),
            _ => x.free(&pools::SET),
        }
    }

    fn substate(x: &mut Cx) {
        let boss = x.linked(BOSS);
        match x.u8(0x5) {
            0 => {
                let w = x.u8(WAIT).wrapping_sub(1);
                x.set8(WAIT, w);
                if w > WAIT_END {
                    return;
                }
                x.add8(0x5, 2);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, CAMERA_LOCK_PARAM as u16);
            }
            2 => {
                let (a, b) = (x.u8(DEFEATED_A), x.u8(DEFEATED_B));
                let finished = |x: &mut Cx| {
                    x.add8(0x5, 2);
                    boss.set_u16(x.b, BOSS_DONE, 1);
                    x.set8(DIFF, a.wrapping_sub(b));
                };
                if b == DEFEATED_TARGET {
                    x.add8(0x5, 2);
                    boss.set_u16(x.b, BOSS_DONE, 1);
                } else if a == DEFEATED_TARGET {
                    if boss.u8(x.b, BOSS_STATE) == BOSS_STATE_READY {
                        finished(x);
                    }
                } else if x.u8(BUSY) != 0 {
                } else if x.u8(WAVE_COUNT) == MAX_WAVES {
                    finished(x);
                } else if x.u8(WAIT) != 0 {
                    { let w = x.u8(WAIT); x.set8(WAIT, w - 1); }
                } else if spawn_wave(x).is_some() {
                    let alea = x.random();
                    let delay = x.rom8(DELAYS + (alea as u32 & 0xf));
                    x.set8(WAIT, delay);
                }
            }
            4 => {
                if boss.u16(x.b, BOSS_COUNT) == 1 {
                    x.add8(0x5, 2);
                }
            }
            6 => {
                if x.u8(LIMIT) != 0 {
                    return;
                }
                let flag = x.u8(FLAG);
                wr8(x.b, WAVE_RESULT, flag);
                if flag != 0 {
                    release_one(x, boss);
                    x.set_a5_u8(0x42, 1);
                    x.add8(0x4, 2);
                } else {
                    x.add8(0x5, 2);
                    final_wave(x);
                }
            }
            8 => final_wave(x),
            0xa => {
                if x.u8(BUSY) == 0 {
                    x.set16(WAIT, FINAL_WAIT);
                    x.add8(0x5, 2);
                }
            }
            0xc => {
                if x.u8(FLAG) == 0 {
                    let w = x.u16(WAIT).wrapping_sub(1);
                    x.set16(WAIT, w);
                    if w != 0 {
                        return;
                    }
                    wr8(x.b, WAVE_RESULT, 1);
                }
                x.set_a5_u8(0x42, 1);
                x.add8(0x5, 2);
            }
            0xe => {
                if x.u8(LIMIT) == 0 {
                    release_one(x, boss);
                    x.add8(0x4, 2);
                }
            }
            _ => {}
        }
    }

    /// Le chef compte un ennemi de moins.
    fn release_one<R: Banc>(x: &mut Cx<'_, R>, boss: Obj) {
        let n = boss.u16(x.b, BOSS_COUNT).wrapping_sub(1);
        boss.set_u16(x.b, BOSS_COUNT, n);
    }

    /// Vague finale : le meneur est de variante 2 et le verdict de vague passe à 0xff.
    fn final_wave<R: Banc>(x: &mut Cx<'_, R>) {
        if let Some(lead) = spawn_wave(x) {
            lead.set_u8(x.b, 3, FINAL_VARIANT);
            x.add8(0x5, 2);
            wr8(x.b, WAVE_RESULT, WAVE_RESULT_FINAL);
        }
    }

    /// Crée trois ennemis d'un coup : un meneur (type 0x14, rattaché au meneur de vagues par
    /// `+0x50`, placé à droite de la caméra) et deux ailiers (type 0x13) rattachés à lui.
    /// `None` quand le pool ENEMY a moins de trois places libres. (L'original trie ensuite les
    /// trois adresses par une boucle qui ne modifie rien.)
    fn spawn_wave<R: Banc>(x: &mut Cx<'_, R>) -> Option<Obj> {
        if (x.a5_u16(ENEMY_FREE) as i16) < ENEMIES_PER_WAVE {
            return None;
        }
        let mut slots = [Obj(0); 3];
        for (i, slot) in slots.iter_mut().enumerate() {
            // Glitch d'origine : l'échec de l'allocation n'est pas testé (voir `Pool::alloc_unchecked`).
            *slot = x.alloc_unchecked(&pools::ENEMY);
            slot.set_u8(x.b, 0, 1);
            x.set16(WINGS + 2 * i as u32, slot.0 as u16);
        }
        let low = |o: Obj| o.0 as u16;
        let [lead, wing_a, wing_b] = slots;
        let boss = x.u16(BOSS);
        lead.set_u16(x.b, BOSS, boss);
        for (wing, side, link) in [(wing_a, 0, E_LINK_A), (wing_b, 1, E_LINK_B)] {
            wing.set_u8(x.b, 0, 1);
            wing.set_u8(x.b, E_WING_SIDE, side);
            wing.set_u8(x.b, 3, 0);
            wing.set_u8(x.b, 0x2f, 1);
            wing.set_u8(x.b, 2, KIND_WING);
            wing.set_u16(x.b, E_PARENT, low(lead));
            wing.set_u16(x.b, BOSS, boss);
            lead.set_u16(x.b, link, low(wing));
        }
        lead.set_u8(x.b, E_COUNT, 3);
        let owner = x.linked(BOSS);
        let n = owner.u16(x.b, BOSS_COUNT).wrapping_add(3);
        owner.set_u16(x.b, BOSS_COUNT, n);
        lead.set_u8(x.b, 0, 1);
        lead.set_u8(x.b, 2, KIND_LEAD);
        lead.set_u8(x.b, 3, 0);
        lead.set_u16(x.b, E_ANCHOR, low(x.o));
        let camera = x.a5_u16(CAMERA_X);
        lead.set_u16(x.b, 8, camera.wrapping_add(SPAWN_OFFSET_X));
        lead.set_u32(x.b, 0xc, 0);
        lead.set_u16(x.b, 0x10, SPAWN_Y);
        // côté d'arrivée : tiré au sort la première fois, puis alterné
        let mut side = x.u8(SIDE);
        if (side as i8) < 0 {
            let alea = x.random();
            side = ((alea as u32 & 1) << 5) as u8;
            x.set8(SIDE, side ^ 0x20);
        }
        lead.set_u8(x.b, E_SIDE, side);
        let y = lead.u8(x.b, 0x11).wrapping_add(side.wrapping_add(0x10));
        lead.set_u8(x.b, 0x11, y);
        x.add8(DEFEATED_A, 1);
        x.set8(BUSY, 1);
        x.add8(WAVE_COUNT, 1);
        Some(lead)
    }
}

/// Fondu au noir des portes (SET 7 [`exit_gate`], SET 29 `set_b::door_b`) : le sous-programme 0x1460 cède la main
/// pendant ses 16 pas. La porte l'appelle comme un sous-programme de tâche (lot E) : la suite, une [`Etape`] qui
/// porte la porte (A0), est rangée dans les appels en cours de la tâche ; à la fin du fondu, la tâche y reprend,
/// puis revient dans la boucle de dispatch (son appel en cours, `dispatch::Etape::Aiguillage`).
pub mod fondu_de_sortie {
    use super::super::set_b;
    use super::Cx;
    use crate::mem::A5;
    use crate::modes::{Flow, Sequence};
    use crate::objects::Obj;

    /// Paramètres du fondu de luminosité (`taches::fondu`) : pages (bit k : page k) et délai entre deux pas.
    pub(super) const PAGES: u32 = 0x48e;
    const DELAI: u32 = 0x490;

    /// Où la tâche reprend après le fondu (adresse de retour de l'appel de 0x1460 dans l'original).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Etape {
        /// Porte de sortie (SET 7), changement de zone (0xcb896) : `exit_gate::after_fade`.
        Zone(Obj),
        /// Porte de sortie (SET 7), scène suivante (0xcb9c0) : `exit_gate::next_scene`.
        Scene(Obj),
        /// Porte de passage (SET 29, 0xd8f06) : `set_b::door_b::after_fade`.
        Passage(Obj),
    }

    impl Etape {
        pub const fn adresse(self) -> u32 {
            match self {
                Etape::Zone(_) => 0xcb896,
                Etape::Scene(_) => 0xcb9c0,
                Etape::Passage(_) => 0xd8f06,
            }
        }

        /// La fin du gestionnaire de la porte, puis son retour (`rts`) dans la boucle de dispatch.
        pub fn pas(self, s: &mut crate::scene::Ctx) -> Flow {
            let (Etape::Zone(porte) | Etape::Scene(porte) | Etape::Passage(porte)) = self;
            let mut x = Cx { c: &mut *s.c, b: &mut *s.b, o: porte };
            match self {
                Etape::Zone(_) => super::exit_gate::after_fade(&mut x),
                Etape::Scene(_) => super::exit_gate::next_scene(&mut x),
                Etape::Passage(_) => set_b::door_b::after_fade(&mut x),
            }
            Flow::Done
        }
    }

    /// Fondu complet vers le noir (`jsr 0x1460`, délai 2, quatre pages), suivi de `suite`. Le fondu cède la main : le
    /// gestionnaire doit revenir aussitôt (appel terminal), et la boucle de dispatch qui l'a appelé abandonne ; la
    /// tâche reprendra par les étapes du fondu, puis `suite`, puis la boucle (`dispatch::Etape::Aiguillage`).
    pub fn fondu_au_noir(x: &mut Cx, suite: fn(Obj) -> Etape) {
        x.set_a5_u16(DELAI, 2);
        x.set_a5_u8(PAGES, 0xf);
        // registres de l'appel d'origine, gardés par le contexte de la tâche jusqu'à la reprise
        let porte = x.o;
        x.c.a[0] = porte.0;
        x.c.a[5] = A5;
        let mut s = crate::scene::Ctx::new(&mut *x.c, &mut *x.b);
        // s'il revenait sans céder, la suite et le retour du gestionnaire seraient déjà faits (`revenir`)
        s.sous_programme(crate::taches::fondu::descente, suite(porte));
    }
}

/// Type 7 (0xcb29c) : porte de sortie de zone (h). Quand un joueur reste 24 images dans l'emprise de la
/// porte, il « déclenche » la sortie (état imposé par la zone) ; quand tous les joueurs vivants sont
/// prêts, la zone change. Elle n'agit que si `+0x2f` est nul, hors pause et sans transition en cours
/// (`A5+0x3d`, `A5+0x50`, `A5+0x42`), et si aucune autre porte n'a pris la main (`A5+0x52`).
///
/// | `+4` | rôle |
/// |---|---|
/// | 0 | initialisation depuis la table des portes (16 octets par variante `+3`, espace programme) |
/// | 2 | surveille les quatre joueurs ; `+0x42..+0x45` comptent les images passées dans l'emprise ; `+0x16` compte les joueurs prêts |
/// | 4 | retour de la zone : recharge la scène (cartes, palettes, caméra mémorisée, joueurs replacés) |
/// | 6 | libération |
///
/// Quand tous les joueurs vivants sont prêts, la zone change selon le genre de sortie `+0x48` :
///
/// | `+0x48` | sortie |
/// |---|---|
/// | 0 | entrée dans une zone annexe : fondu au noir, la porte mémorise la caméra, suspend le décor et les objets (`+0x40`), charge la scène `+0x4f`, et attend dans l'état 4 le retour |
/// | 1 | retour d'une zone annexe : fondu, joueurs relâchés, les objets suspendus se réveillent (ceux dont le compteur passe sous zéro sont libérés) |
/// | 2 | scène suivante `+0x4f` : fondu, puis la partie repasse en phase 8 (entrée de scène) |
/// | 3 | fin de partie (`A5+0x42`) |
///
/// Le fondu au noir (0x1460) cède la main pendant 16 images : la tâche reprend aux étapes
/// [`fondu_de_sortie::Etape::Zone`] (0xcb896) et [`fondu_de_sortie::Etape::Scene`] (0xcb9c0).
mod exit_gate {
    use super::super::set_b;
    use super::*;
    use crate::mem::{rd16, rd8, wr8, A5};
    use cps2::Cps2Bus;

    // --- table de la ROM (espace programme) ---
    const GATES: u32 = 0xcb344;
    const GATE_SHIFT: u16 = 4;
    const GATE_X: u32 = 0;
    const GATE_Z: u32 = 2;
    const GATE_Y: u32 = 4;
    const GATE_EXIT_KIND: u32 = 6;
    const GATE_ZONE: u32 = 7;
    const GATE_SCENE: u32 = 8;
    const GATE_IGNORE_DEPTH: u32 = 9;
    const GATE_BONUS: u32 = 10;
    /// États de joueur imposés selon l'indice de zone (`+0x46`), longs (4 octets par indice).
    const PLAYER_STATES: u32 = 0xcb998;

    // --- variables de A5 ---
    const IN_GAME: u32 = 0x35;
    const PAUSED: u32 = 0x3d;
    const TRANSITION: u32 = 0x42;
    const EXIT_LOCK: u32 = 0x50;
    const OWNER_GATE: u32 = 0x52;
    /// Variable (A5 - 0x5ac4) : non nulle, plus personne n'est jugé dans l'emprise.
    const NEAR_BLOCK: u32 = A5 - 0x5ac4;

    // --- champs de la porte ---
    const PLAYER_COUNT_TIMER: u32 = 0x20;
    const ARRIVED: u32 = 0x16;
    const TRIGGERED: u32 = 0x17;
    const COUNTED: u32 = 0x1a;
    const NEAR_COUNTERS: u32 = 0x42;
    const EXIT_KIND: u32 = 0x48;
    const ZONE: u32 = 0x46;
    const NEXT_SCENE: u32 = 0x4f;
    const IGNORE_DEPTH: u32 = 0x1b;
    const BONUS: u32 = 0x24;

    // --- champs d'un joueur ---
    const P_ACTIVE: u32 = 0x0;
    const P_STATE: u32 = 0x4;
    const P_X: u32 = 0x8;
    const P_Z: u32 = 0xc;
    const P_Y: u32 = 0x10;
    const P_LIFE: u32 = 0x62;
    const P_BLOCK_A: u32 = 0x66;
    const P_BLOCK_B: u32 = 0x15c;
    const P_BLOCK_C: u32 = 0x8a;
    const P_ZONES: u32 = 0x6c;
    const P_WEAPON_OBJ: u32 = 0x86;
    const P_RESET: u32 = 0x10f;

    /// Emprise de la porte autour de sa position (largeur 0x60, profondeur 0x40) et durée en images.
    const REACH_X: u16 = 0x30;
    const REACH_Y: u16 = 0x20;
    const NEAR_FRAMES: i8 = 0x18;
    /// États de joueur reconnus dans l'emprise (octet bas ignoré).
    const WALKING_STATES: [u32; 3] = [0x2000000, 0x2000600, 0x2000400];
    const PLAYER_STATE_IDLE: u32 = 0x2002010;
    const PLAYER_STATE_GONE: u16 = 0x402;
    const STATE_DONE: u8 = 2;

    pub fn run(x: &mut Cx) {
        refresh_player_count(x);
        if !gate_active(x.b, x.o) {
            return;
        }
        match x.u8(0x4) {
            0 => init(x),
            2 => watch(x),
            4 => arrive(x),
            _ => x.free(&pools::SET),
        }
    }

    // --- conditions communes (lecture seule) ---------------------------------------------------------

    fn players() -> impl Iterator<Item = Obj> {
        util::players()
    }

    // --- changement de zone (0xcb844) ---------------------------------------------------------------

    /// Rectangle de texte effacé au changement de zone et au retour (`text::CLEAR_BOX`, A4 = colonne, ligne,
    /// largeur, hauteur).
    const CLEARED_TEXT_BOX: u32 = 0xcbc02;
    /// Prime de sortie cumulée (octet en A5 - 0x5b20).
    const BONUS_TOTAL: u32 = A5 - 0x5b20;
    const P_FLAGS: u32 = 0x1;
    const P_HIDDEN: u8 = 0x2;
    const PLAYER_STATE_DEAD: u32 = 0x402_0000;
    /// Variantes de porte qui marquent la scène (`A5+0x4a`).
    const MARKING_VARIANTS: [u8; 3] = [0x13, 0x15, 0x16];
    const SCENE: u32 = 0x34;
    const NEXT_SCENE_FLAG: u32 = 0x51;
    const GAME_OVER: u32 = 0x42;
    const PHASE: u32 = 0x2c;
    const PHASE_SCENE_ENTRY: u8 = 8;
    /// Mots longs remis à zéro avant la scène suivante (armes des quatre joueurs).
    const PLAYER_WEAPONS: [u32; 4] = [0x602, 0x802, 0xa02, 0xc02];
    /// Genres de sortie (`+0x48`).
    const KIND_RETURN: u8 = 1;
    const KIND_NEXT_SCENE: u8 = 2;
    const KIND_GAME_OVER: u8 = 3;

    /// Tous les joueurs sont prêts (0xcb844) : prime, palette, joueurs morts cachés, puis la sortie.
    fn change_zone(x: &mut Cx) {
        let bonus = rd8(x.b, BONUS_TOTAL).wrapping_add(x.u8(BONUS));
        wr8(x.b, BONUS_TOTAL, bonus);
        crate::text::clear_box(x.b, CLEARED_TEXT_BOX);
        for p in players() {
            if p.u8(x.b, P_ACTIVE) != 0 && p.u16(x.b, P_LIFE) == 0 {
                let flags = p.u8(x.b, P_FLAGS) | P_HIDDEN;
                p.set_u8(x.b, P_FLAGS, flags);
                p.set_u32(x.b, P_STATE, PLAYER_STATE_DEAD);
            }
        }
        match x.u8(EXIT_KIND) {
            KIND_NEXT_SCENE => {
                x.set_a5_u8(NEXT_SCENE_FLAG, 1);
                fondu_de_sortie::fondu_au_noir(x, fondu_de_sortie::Etape::Scene)
            }
            KIND_GAME_OVER => x.set_a5_u8(GAME_OVER, 1),
            _ => {
                x.set_a5_u16(OWNER_GATE, 0);
                if MARKING_VARIANTS.contains(&x.u8(0x3)) {
                    x.set_a5_u8(0x4a, 1);
                }
                fondu_de_sortie::fondu_au_noir(x, fondu_de_sortie::Etape::Zone)
            }
        }
    }

    /// Après le fondu d'un changement de zone (0xcb896) : retour d'une zone annexe, ou entrée.
    pub fn after_fade(x: &mut Cx) {
        let scene = x.u8(NEXT_SCENE);
        x.set_a5_u8(SCENE, scene);
        if x.u8(EXIT_KIND) == KIND_RETURN {
            return leave_annex(x);
        }
        save_and_suspend(x);
        x.set16(0x4, 0x400);
        enter_annex(x);
    }

    /// Après le fondu de fin de scène (0xcb9c0) : la partie repasse en phase 8 sur la scène `+0x4f`.
    pub fn next_scene<R: Banc>(x: &mut Cx<'_, R>) {
        for off in PLAYER_WEAPONS {
            x.set_a5_u32(off, 0);
        }
        let scene = x.u8(NEXT_SCENE);
        x.set_a5_u8(SCENE, scene);
        x.set_a5_u8(EXIT_LOCK, 0);
        x.set_a5_u8(PHASE, PHASE_SCENE_ENTRY);
        x.set_a5_u8(PHASE + 1, 0);
        x.set_a5_u16(PHASE + 2, 0);
        x.set_a5_u16(OWNER_GATE, 0);
        set_b::reset_players_after_exit(x);
    }

    /// Pools suspendus à l'entrée d'une zone annexe, dans l'ordre de l'original (les effets des joueurs,
    /// entre les tirs ennemis et les tirs des joueurs, sont libérés au lieu d'être suspendus).
    const SUSPENDED: [&set_b::Pool; 8] = [
        &set_b::SET_POOL, &set_b::SOBJ_POOL, &set_b::ENEMY_POOL, &set_b::EFFCT_POOL, &set_b::BOX_POOL,
        &set_b::ITEM_POOL, &set_b::BOSS_POOL, &set_b::ESHL_POOL,
    ];
    const PEFF_POOL: set_b::Pool = set_b::Pool { base: A5 + 0x179e, size: 0x60, count: 48 };
    const PSHL_POOL: set_b::Pool = set_b::Pool { base: A5 + 0x10de, size: 0x60, count: 18 };
    /// Décors de type 0x13 libérés à l'entrée des portes de variante 8 et 0xa.
    const FREED_KIND: u8 = 0x13;
    /// Caméras imposées par les variantes 8 et 0xa (`+0x30..+0x3c`).
    const CAMERA_VARIANT_8: [u32; 4] = [0x2a0_0000, 0x108_0000, 0x150_0000, 0];
    const CAMERA_VARIANT_A: [u32; 4] = [0x400_0000, 0x1fe_0000, 0x200_0000, 0x7b_0000];

    /// Mémorise l'état de la scène dans la porte et suspend le monde (0xcbcba).
    fn save_and_suspend(x: &mut Cx) {
        set_b::save_camera(x);
        let v = x.a5_u16(0x52c);
        x.set16(0x22, v);
        let v = x.a5_u8(NEXT_SCENE_FLAG);
        x.set8(0x25, v);
        x.set_a5_u8(NEXT_SCENE_FLAG, 0);
        let v = x.a5_u8(0x58);
        x.set8(0x26, v);
        let camera = match x.u8(0x3) {
            0xa => Some(CAMERA_VARIANT_A),
            0x8 => Some(CAMERA_VARIANT_8),
            _ => None,
        };
        if let Some(camera) = camera {
            for (i, v) in camera.into_iter().enumerate() {
                x.set32(0x30 + 4 * i as u32, v);
            }
            for i in 0..set_b::SET_POOL.count {
                let o = Obj(set_b::SET_POOL.base + i * set_b::SET_POOL.size);
                if o.u8(x.b, 0) != 0 && o.u8(x.b, 2) == FREED_KIND {
                    pools::SET.release_68k(x.c, x.b, o);
                }
            }
        }
        for pool in SUSPENDED {
            set_b::suspend(x, pool);
        }
        free_player_effects(x);
        set_b::suspend(x, &PSHL_POOL);
    }

    fn free_player_effects<R: Banc>(x: &mut Cx<'_, R>) {
        for i in 0..PEFF_POOL.count {
            let o = Obj(PEFF_POOL.base + i * PEFF_POOL.size);
            if o.u8(x.b, 0) != 0 {
                o.set_u8(x.b, 0x4, 6);
            }
        }
    }

    /// Entrée dans la zone annexe (0xcc0ca) : caméras, script de scène, palettes, joueurs replacés.
    fn enter_annex(x: &mut Cx) {
        set_b::call_keeping(x, &[0x33aa6]);
        x.call(crate::scene::script::SCRIPT_RESET.entry);
        set_b::call_keeping(x, &[0x1a360, 0x1a678, 0x1a8ac]);
        x.set_a5_u8(fondu_de_sortie::PAGES, 0xf);
        crate::palette::full_brightness(x.b);
        x.set_a5_u16(OWNER_GATE, 0);
        set_b::reset_players_after_exit(x);
        x.call(crate::scene::entry::PLACE_PLAYERS.entry);
        for p in players() {
            if p.u8(x.b, P_ACTIVE) != 0 && p.u16(x.b, P_LIFE) != 0 {
                x.c.a[0] = p.0;
                crate::objects::lists::ACTORS.reinsert_68k(x.c, x.b, p);
            }
        }
        x.c.a[0] = x.o.0;
    }

    /// Retour d'une zone annexe (0xcbeba) : joueurs relâchés, la porte se libère, le monde se réveille.
    fn leave_annex(x: &mut Cx) {
        for p in players() {
            if p.u8(x.b, P_ACTIVE) == 0 {
                continue;
            }
            p.set_u8(x.b, P_ACTIVE, 1);
            let flags = p.u8(x.b, P_FLAGS) & !P_HIDDEN;
            p.set_u8(x.b, P_FLAGS, flags);
            for off in [P_BLOCK_A, P_BLOCK_B, P_BLOCK_C, P_RESET] {
                p.set_u8(x.b, off, 0);
            }
            p.set_u8(x.b, 0x7e, 0x46);
        }
        x.set8(0x4, 6);
        // les autres portes réveillées s'exécutent aussitôt (`bsr` récursif de l'original)
        set_b::resume_with(x, &set_b::SET_POOL, |x, o| {
            if o.u8(x.b, 2) == 7 {
                set_b::run_on(x, o, run);
            }
        });
        for pool in &SUSPENDED[1..] {
            set_b::resume(x, pool);
        }
        free_player_effects(x);
        set_b::resume(x, &PSHL_POOL);
    }

    /// État 4 (0xcba48) : retour de la zone annexe, la scène d'avant est rechargée.
    fn arrive(x: &mut Cx) {
        set_b::call_keeping(x, &[0x33b24]);
        x.call(crate::scene::script::SCRIPT_RESET.entry);
        set_b::call_keeping(x, &[0x1a360, 0x1a678, 0x1a8ac]);
        x.set_a5_u8(fondu_de_sortie::PAGES, 0xf);
        crate::palette::full_brightness(x.b);
        x.set_a5_u16(OWNER_GATE, 0);
        crate::text::clear_box(x.b, CLEARED_TEXT_BOX);
        set_b::restore_camera(x);
        let v = x.u16(0x22);
        x.set_a5_u16(0x52c, v);
        let v = x.u8(0x25);
        x.set_a5_u8(NEXT_SCENE_FLAG, v);
        let v = x.u8(0x26);
        x.set_a5_u8(0x58, v);
        let musique = x.u16(0x1c);
        crate::sound::jouer_avec(x.b, 0xe17a4, crate::sound::Source { d4: Some(musique as u32), ..Default::default() });
        let camera = if x.a5_u8(0x36) != 0 { 0x54e } else { 0x4fe };
        x.c.a[4] = A5 + camera;
        for p in players() {
            if p.u8(x.b, P_ACTIVE) != 0 {
                place(x, p);
            }
        }
        x.set8(0x4, 6);
    }

    /// Ligne de placement par variante (octet, espace programme) puis décalages X, Y par ligne et par
    /// joueur (0x10 octets par ligne, espace programme).
    const PLACEMENT_ROW: u32 = 0xcbb92;
    const PLACEMENT: u32 = 0xcbbe2;
    const PLAYER_STATE_WALK: u32 = 0x200_0000;

    /// Replace un joueur devant la porte (0xcbb3c) ; un joueur hors de l'état 2 est seulement caché
    /// et posé au sol.
    fn place(x: &mut Cx, p: Obj) {
        if p.u8(x.b, P_STATE) != 2 {
            let flags = p.u8(x.b, P_FLAGS) | P_HIDDEN;
            p.set_u8(x.b, P_FLAGS, flags);
            crate::placement::place_on_free_ground(x.b, p);
            return;
        }
        p.set_u32(x.b, P_STATE, PLAYER_STATE_WALK);
        let flags = p.u8(x.b, P_FLAGS) & !P_HIDDEN;
        p.set_u8(x.b, P_FLAGS, flags);
        p.set_u8(x.b, P_ACTIVE, 1);
        let variant = x.u8(0x3) as u32;
        let row = (x.rom8(PLACEMENT_ROW + variant) as u16) << 4;
        let index = row.wrapping_add(4 * p.u8(x.b, 2) as u16) as u32;
        let (dx, dy) = (x.rom16(PLACEMENT + index), x.rom16(PLACEMENT + 2 + index));
        let (gx, gy) = (x.u16(0x8), x.u16(0x10));
        p.set_u16(x.b, P_X, gx.wrapping_add(dx));
        p.set_u16(x.b, P_Y, gy.wrapping_add(dy));
        p.set_u16(x.b, P_Z, 0);
        crate::mem::set_low_word(&mut x.c.d[1], dx);
        crate::mem::set_low_word(&mut x.c.d[2], dy);
    }

    fn alive(b: &mut Cps2Bus, p: Obj) -> bool {
        p.u8(b, P_ACTIVE) != 0 && p.u16(b, P_LIFE) != 0
    }

    /// Conditions de 0xcb2a0-0xcb2d8 : la porte agit-elle à cette image ?
    fn gate_active(b: &mut Cps2Bus, me: Obj) -> bool {
        if me.u8(b, 0x2f) != 0 || me.u8(b, 0x40) != 0 {
            return false;
        }
        if rd8(b, A5 + PAUSED) != 0 || rd8(b, A5 + EXIT_LOCK) != 0 || rd8(b, A5 + TRANSITION) != 0 {
            return false;
        }
        let owner = rd16(b, A5 + OWNER_GATE);
        owner == 0 || owner == me.0 as u16
    }

    /// Le joueur `p` se tient-il dans l'emprise de la porte, libre de ses mouvements, et la zone de
    /// la porte figure-t-elle dans son masque `+0x6c` ? (L'original teste le bit de l'indice de zone
    /// dans D0, dont les bits hauts sont des restes : les indices de zone sont inférieurs à 8.)
    fn in_reach(b: &mut Cps2Bus, me: Obj, p: Obj) -> bool {
        if rd8(b, NEAR_BLOCK) != 0 {
            return false;
        }
        let blocked = p.u8(b, P_BLOCK_A).wrapping_add(p.u8(b, P_BLOCK_B)).wrapping_add(p.u8(b, P_BLOCK_C));
        let zones = p.u8(b, P_ZONES) as u32;
        if blocked != 0 || zones >> (me.u8(b, ZONE) as u32 & 31) & 1 == 0 {
            return false;
        }
        let state = p.u32(b, P_STATE);
        if state >> 24 != 2 || !WALKING_STATES.contains(&(state & !0xff)) || p.u16(b, P_Z) != 0 {
            return false;
        }
        let dx = p.u16(b, P_X).wrapping_sub(me.u16(b, 0x8)).wrapping_add(REACH_X);
        if dx > 2 * REACH_X {
            return false;
        }
        if me.u8(b, IGNORE_DEPTH) != 0 {
            return true;
        }
        let dy = p.u16(b, P_Y).wrapping_sub(me.u16(b, 0x10)).wrapping_add(REACH_Y);
        dy <= 2 * REACH_Y
    }

    /// Tous les joueurs vivants sont-ils prêts (0xcb80c-0xcb840) ? `arrived` : joueurs déjà comptés.
    fn everyone_ready(b: &mut Cps2Bus, in_game: u8, arrived: u8) -> bool {
        if in_game == 0 {
            return false;
        }
        let mut missing = in_game;
        for p in players() {
            if p.u8(b, P_ACTIVE) != 0 && p.u16(b, P_LIFE) == 0 && p.u16(b, P_STATE) != PLAYER_STATE_GONE {
                missing = missing.wrapping_add(1);
            }
        }
        (missing as i8) <= (arrived as i8)
    }

    // --- l'automate ---------------------------------------------------------------------------------

    /// Toutes les 32 images : nombre de joueurs vivants dans `A5+0x35`.
    fn refresh_player_count<R: Banc>(x: &mut Cx<'_, R>) {
        let t = x.u16(PLAYER_COUNT_TIMER).wrapping_sub(1);
        x.set16(PLAYER_COUNT_TIMER, t);
        if (t as i16) >= 0 {
            return;
        }
        x.set16(PLAYER_COUNT_TIMER, 0x20);
        let n = players().filter(|&p| alive(x.b, p)).count();
        x.set_a5_u8(IN_GAME, n as u8);
    }

    fn init<R: Banc>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        for off in [NEAR_COUNTERS, NEAR_COUNTERS + 1, NEAR_COUNTERS + 2, NEAR_COUNTERS + 3, ARRIVED, TRIGGERED] {
            x.set8(off, 0);
        }
        let row = GATES + (((x.u8(0x3) as u16) << GATE_SHIFT) as i16 as i32 as u32);
        let (gx, gz, gy) = (x.rom16(row + GATE_X), x.rom16(row + GATE_Z), x.rom16(row + GATE_Y));
        x.set16(0x8, gx);
        x.set16(0xc, gz);
        x.set16(0x10, gy);
        let fields = [(EXIT_KIND, GATE_EXIT_KIND), (ZONE, GATE_ZONE), (NEXT_SCENE, GATE_SCENE), (IGNORE_DEPTH, GATE_IGNORE_DEPTH), (BONUS, GATE_BONUS)];
        for (field, entry) in fields {
            let v = x.rom8(row + entry);
            x.set8(field, v);
        }
    }

    /// État 2 : un joueur reste dans l'emprise assez longtemps (déclenchement), ou arrive à l'état
    /// « prêt » (compté une fois) ; quand tous sont prêts, la zone change.
    fn watch(x: &mut Cx) {
        for (i, p) in players().enumerate() {
            let bit = 1u8 << i;
            let counter = NEAR_COUNTERS + i as u32;
            if x.u8(TRIGGERED) & bit == 0 {
                if p.u8(x.b, P_ACTIVE) != 1 {
                    continue;
                }
                if (x.u8(counter) as i8) >= 0 {
                    let inside = in_reach(x.b, x.o, p);
                    let n = if inside { x.u8(counter).wrapping_add(1) } else { 0 };
                    x.set8(counter, n);
                    if (n as i8) > NEAR_FRAMES {
                        return trigger(x, p, bit);
                    }
                    continue;
                }
            }
            if x.u8(COUNTED) & bit == 0 && p.u32(x.b, P_STATE) == PLAYER_STATE_IDLE {
                x.add8(ARRIVED, 1);
                x.set_bit(COUNTED, i as u8, true);
            }
        }
        let (in_game, arrived) = (x.a5_u8(IN_GAME), x.u8(ARRIVED));
        if everyone_ready(x.b, in_game, arrived) {
            change_zone(x);
        }
    }

    /// Le joueur `p` déclenche la sortie : il passe dans l'état imposé par la zone ; la porte
    /// devient propriétaire de la transition (`A5+0x52`).
    fn trigger<R: Banc>(x: &mut Cx<'_, R>, p: Obj, bit: u8) {
        if p.u8(x.b, P_ACTIVE) == 0 || p.u16(x.b, P_LIFE) == 0 {
            return;
        }
        let triggered = x.u8(TRIGGERED) | bit;
        x.set8(TRIGGERED, triggered);
        if x.a5_u16(OWNER_GATE) == 0 {
            crate::families::effects_creators::beacon(x.b, x.o);
        }
        let me = x.o.0 as u16;
        x.set_a5_u16(OWNER_GATE, me);
        let zone = x.u8(ZONE) as u32;
        let state = (x.rom16(PLAYER_STATES + 4 * zone) as u32) << 16 | x.rom16(PLAYER_STATES + 4 * zone + 2) as u32;
        p.set_u32(x.b, P_STATE, state);
        p.set_u8(x.b, P_ACTIVE, STATE_DONE);
        p.set_u8(x.b, P_RESET, 1);
        match x.u8(0x3) {
            0xa | 0xe => p.set_u32(x.b, P_STATE, 0x2002008),
            0x8 => p.set_u32(x.b, P_STATE, 0x200200a),
            0x28 => {
                p.set_u16(x.b, P_WEAPON_OBJ, x.o.0 as u16);
                p.set_u32(x.b, P_STATE, 0x200200c);
            }
            _ => {}
        }
    }
}

/// Type 0 (0xba310) : animation de palette par script (h), le décor le plus fréquent (plus de 13 000
/// appels par scénario). La variante `+3` choisit une entrée de 8 octets de la table `TABLE` : adresse
/// du script (`+0x30`) et adresse des palettes sources (`+0x34`). Le paramètre `+0x2f` choisit la
/// page de la RAM palette où écrire (0 : 0x91c800, 1 : 0x91cc00, 2 : 0x91c000, autre : 0x91c400).
///
/// Le script commence par un en-tête (numéro de ligne de palette de départ, durée du premier bloc)
/// puis se compose de blocs : durée (mot : 0 → état 4, négatif → fin), nombre de lignes, et pour
/// chaque ligne l'indice (en lignes de 32 octets) de la palette source. Chaque ligne copiée garde
/// le quartet de luminosité (`& 0xf000f000`) de la ligne de destination. Une durée 0xffff relance
/// le script, tout autre négatif saute à l'adresse longue qui suit.
///
/// | `+4` | rôle |
/// |---|---|
/// | 0 | initialisation et premier bloc |
/// | 2 | un bloc à chaque échéance de `+0x3c` ; si `A5+0x42` (scène en transition), une table par variante décide de continuer (0), d'attendre (1) ou de finir (2, 3) |
/// | 4 | fin : une table par variante choisit la libération (0) ou la reconversion en variante 0x48, 0x4c ou 0x4d |
/// | 6 | libération |
///
/// La variante 0x4e ignore la pause et la suspension.
mod palette_script {
    use super::*;
    use crate::mem::{rd16, rd32, wr32};

    /// Table des scripts (espace de données) : script puis palettes sources, 8 octets par variante.
    const TABLE: u32 = 0xba57a;
    /// Pendant une transition de scène : action par variante (octet, espace programme).
    const TRANSITION_ACTION: u32 = 0xba44c;
    /// Fin de script : suite par variante (octet, espace programme).
    const END_ACTION: u32 = 0xba4ca;
    const PALETTE_PAGES: [u32; 4] = [0x91c800, 0x91cc00, 0x91c000, 0x91c400];
    /// Champs : script, palettes sources, destination, compte à rebours.
    const SCRIPT: u32 = 0x30;
    const SOURCE: u32 = 0x34;
    const DEST: u32 = 0x38;
    const TIMER: u32 = 0x3c;
    const PAGE: u32 = 0x2f;
    const KEEP_BRIGHTNESS: u32 = 0xf000f000;
    const LINE_BYTES: u32 = 32;
    const STATE_FINISH: u16 = 0x400;
    const GAME_PAUSED: u32 = 0x3d;
    const TRANSITION: u32 = 0x42;
    /// Variante qui ignore pause et suspension.
    const ALWAYS_RUNS: u8 = 0x4e;
    /// Variantes de remplacement en fin de script.
    const NEXT_VARIANTS: [(u8, u8); 3] = [(0x48, 0), (0x4c, 0), (0x4d, 1)];

    pub fn run(x: &mut Gx) {
        let variant = x.u8(0x3);
        if variant != ALWAYS_RUNS && (x.a5_u8(GAME_PAUSED) != 0 || x.u8(0x40) != 0) {
            return;
        }
        match x.u8(0x4) {
            0 => start(x),
            2 => play(x),
            4 => finish(x),
            _ => x.free(&pools::SET),
        }
    }

    fn start(x: &mut Gx) {
        x.add8(0x4, 2);
        let entry = TABLE.wrapping_add((((x.u8(0x3) as u16) << 3) as i16) as u32);
        let (script, source) = (x.data32(entry), x.data32(entry + 4));
        x.set32(SOURCE, source);
        x.set32(SCRIPT, script);
        let page = PALETTE_PAGES[x.u8(PAGE).min(3) as usize];
        let first_line = x.data16(script) as i16;
        x.set32(DEST, page.wrapping_add(((first_line as u16) << 5) as i16 as i32 as u32));
        let timer = x.data16(script + 2);
        x.set16(TIMER, timer);
        let dest = x.u32(DEST);
        copy_lines(x, script + 4, source, dest);
    }

    fn play(x: &mut Gx) {
        if x.a5_u8(TRANSITION) != 0 {
            let variant = x.u8(0x3) as u32;
            let action = x.rom8(TRANSITION_ACTION + variant);
            match action {
                0 => {}
                1 => return,
                2 => {
                    x.set16(0x4, STATE_FINISH);
                    return finish(x);
                }
                _ => return finish(x),
            }
        }
        let t = x.u16(TIMER).wrapping_sub(1);
        x.set16(TIMER, t);
        if (t as i16) > 0 {
            return;
        }
        let mut script = x.u32(SCRIPT);
        let source = x.u32(SOURCE);
        let duration = x.data16(script);
        script += 2;
        x.set16(TIMER, duration);
        if (duration as i16) < 0 {
            if duration == 0xffff {
                x.set16(0x4, 0);
                start(x);
            } else {
                let target = x.data32(script);
                x.set32(SCRIPT, target);
            }
        } else if duration == 0 {
            x.set16(0x4, STATE_FINISH);
            finish(x);
        } else {
            let dest = x.u32(DEST);
            copy_lines(x, script, source, dest);
        }
    }

    /// Copie les lignes d'un bloc (nombre, puis un indice par ligne) vers la palette de destination
    /// et mémorise la suite du script.
    /// Glitch d'origine : le dernier long copié (avec la luminosité) reste dans D4, que le radeau sans pilote
    /// (0xad7b0) ajoute à la position des joueurs : c'est un reste de la boucle d'aiguillage (`Restes::d4`).
    fn copy_lines(x: &mut Gx, mut script: u32, source: u32, mut dest: u32) {
        let count = rd16(x.b, script).wrapping_sub(1);
        script += 2;
        let mut last = 0;
        for _ in 0..=count as u32 {
            let line = (rd16(x.b, script) << 5) as u32;
            script += 2;
            let keep = rd32(x.b, dest) & KEEP_BRIGHTNESS;
            for i in (0..LINE_BYTES).step_by(4) {
                last = rd32(x.b, source + line + i) | keep;
                wr32(x.b, dest + i, last);
            }
            dest += LINE_BYTES;
        }
        x.set32(SCRIPT, script);
        x.restes().d4 = last;
    }

    fn finish(x: &mut Gx) {
        let variant = x.u8(0x3) as u32;
        match x.rom8(END_ACTION + variant) {
            0 => x.set8(0x4, 6),
            n @ 1..=3 => {
                let (next, page) = NEXT_VARIANTS[n as usize - 1];
                x.set8(0x3, next);
                x.set8(PAGE, page);
                x.set32(0x4, 0);
            }
            _ => {}
        }
    }
}

/// Type 10 (0xcc3c2) : bulle de dialogue temporisée (h). À sa création (état 0) elle choisit un texte
/// dans les tables de la scène (une table par mode de jeu `A5+0x72`, une entrée par variante `+3`),
/// crée un objet SOBJ de type 0xc (le cadre, par 0xdfc78) dont elle garde l'adresse en `+0x22`,
/// publie le texte en `A5+0x5ce..0x5d4` et charge la palette 0x91c180. Elle vit 120 images
/// (`+0x20`), ou jusqu'à ce que la variable `A5-0x5fbe` devienne non nulle, puis efface le texte,
/// demande la libération de son cadre (état 6) et se libère.
mod dialogue {
    use super::*;
    use crate::mem::{wr32, wr8, A5};

    /// Tables de textes par mode de jeu (longs, espace programme), puis par variante (longs ×4).
    const TEXT_TABLES: u32 = 0xcc438;
    const GAME_MODE: u32 = 0x72;
    /// Création d'un objet SOBJ de type 0xc : D0 = variante, retenue = échec ; A1 = l'objet.
    const SPAWN_FRAME: u32 = 0xdfc78;
    const PALETTE_SRC: u32 = 0x292260;
    const PALETTE_DST: u32 = 0x91c180;
    /// Variable d'arrêt (A5 - 0x5fbe) et variable « bulle affichée » (A5 - 0x5b8c).
    const STOP: u32 = A5 - 0x5fbe;
    const SHOWN: u32 = A5 - 0x5b8c;
    /// Variables de texte publiées pour l'affichage.
    const TEXT_ADDR: u32 = 0x5ce;
    const TEXT_SIZE: u32 = 0x5d2;
    const TEXT_WIDTH: u32 = 0x5d4;
    const TEXT_WIDTH_VALUE: u16 = 0xc4;
    const LIFETIME: u8 = 0x78;
    const TIMER: u32 = 0x20;
    const FRAME: u32 = 0x22;
    const FRAME_WIDTH: u32 = 0x14;
    const FRAME_FREE_STATE: u32 = 0x6000000;

    pub fn run(x: &mut Cx) {
        if crate::mem::rd8(x.b, STOP) == 0 {
            if x.u8(0x4) == 0 {
                return create(x);
            }
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if t != 0 {
                return;
            }
        }
        wr8(x.b, SHOWN, 0);
        x.set_a5_u32(TEXT_ADDR, 0);
        let frame = x.u16(FRAME);
        if frame != 0 {
            wr32(x.b, (frame as i16 as i32 as u32 & 0xff_ffff) + 4, FRAME_FREE_STATE);
        }
        x.free(&pools::SET);
    }

    fn create(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(TIMER, LIFETIME);
        let variant = (x.u8(0x3) << 2) as u32;
        let mode = x.a5_u16(GAME_MODE).wrapping_sub(2).wrapping_mul(2) as i16 as i32 as u32;
        let table = (x.rom16(TEXT_TABLES.wrapping_add(mode)) as u32) << 16 | x.rom16(TEXT_TABLES.wrapping_add(mode) + 2) as u32;
        let text = x.data32(table + variant);
        let width = x.data16(text);
        x.set16(FRAME, 0);
        x.c.d[0] = x.u8(0x2f) as u32;
        x.c.d[1] = x.c.d[1] & !0xffff | width as u32;
        x.call(SPAWN_FRAME);
        if x.c.sr & m68k::SR_C == 0 {
            let frame = Obj(x.c.a[1]);
            frame.set_u16(x.b, FRAME_WIDTH, width);
            x.set16(FRAME, frame.0 as u16);
        }
        let size = x.data16(text + 2);
        x.set_a5_u16(TEXT_SIZE, size);
        x.set_a5_u16(TEXT_WIDTH, TEXT_WIDTH_VALUE);
        x.set_a5_u32(TEXT_ADDR, text + 4);
        for i in (0..32).step_by(4) {
            let v = x.data32(PALETTE_SRC + i);
            wr32(x.b, PALETTE_DST + i, v);
        }
    }
}

/// Type 5 (0xc9780) : saisie du nom au tableau des scores (h). Deux sortes d'objets selon `+3` :
///
/// * variante 0 : le **tableau** (un seul objet). Il garde dans `+0x18..+0x1f` les adresses des quatre
///   curseurs, détecte les cases où deux curseurs se superposent (deux enregistrements de 8 octets
///   en `+0x30` et `+0x40` pour l'image précédente) et dessine ces cases ;
/// * variante non nulle : le **curseur** d'un joueur (`+0x2f` = numéro de joueur, `+0x22` = son
///   enregistrement). Il déplace le curseur (`+0x8` colonne, `+0xc` ligne) sur une grille de
///   caractères (11 colonnes × 6 lignes, ou une grille large si `A5+0x70`), écrit les lettres dans le
///   tampon du joueur (`+0x118..+0x11f`, 0xff = vide, `+0x18` = longueur), gère l'effacement, les
///   voyelles voisées (0x9d, 0x9f, table de conversion), deux codes secrets (suites de touches
///   reconnues dans le tampon) et calcule un chiffre de contrôle en BCD.
///
/// États (`+4`) : 0 initialisation, 2 saisie, 4 terminé (le curseur attend), 6 libération. Le tableau
/// libère les curseurs (état 6 dans l'objet) puis lui-même aux états 4 et 6.
mod name_entry {
    use super::*;
    use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8};

    // --- RAM vidéo et tables ---
    const TILEMAP: u32 = 0x912000;
    const PALETTE_BANK: u32 = 0x91c400;
    const PALETTE_SOURCE: u32 = 0x25cd40;
    const PALETTE_ENTRY: u32 = 0x20;
    const KEEP_BRIGHTNESS: u32 = 0xf000f000;
    /// Cellule de départ des curseurs (espace programme) : deux mots par joueur et par mode.
    const START_CELLS: u32 = 0xc9abe;
    /// Largeur de chaque ligne de la grille (mots, espace de données).
    const ROW_WIDTH_NARROW: u32 = 0xcad74;
    const ROW_WIDTH_WIDE: u32 = 0xcad80;
    /// Noms par défaut : 8 octets par variante de joueur (+0x60 en grille large), espace de données.
    const DEFAULT_NAMES: u32 = 0xc9daa;
    /// Caractère de chaque case (octets, espace programme), index = colonne + 16 × ligne.
    const GRID_NARROW: u32 = 0xcaacc;
    const GRID_WIDE: u32 = 0xcab36;
    /// Conversions des voyelles voisées (octets, espace de données) pour 0x9f et 0x9d.
    const VOICED: u32 = 0xcabdc;
    const SEMI_VOICED: u32 = 0xcac64;
    /// Codes secrets : suites de caractères (octet de longueur, caractères), fin = octet négatif.
    const RESET_CODES: u32 = 0xcade6;
    const SPECIAL_CODES: u32 = 0xcad8a;
    const SPECIAL_REPLACEMENTS: u32 = 0xca5a6;
    /// Valeur BCD de chaque caractère (octets, espace de données) et chiffre de contrôle par défaut.
    const CHAR_DIGITS: u32 = 0xcacec;
    const DEFAULT_CHECK: u32 = 0xca6aa;
    /// Teintes de la case spéciale (octets, espace programme).
    const SPECIAL_TILES: u32 = 0xca83e;
    /// Tables de placement des tuiles (mots, espace programme), grille étroite puis large.
    const NARROW_ROW_OFFSETS: u32 = 0xca894;
    const NARROW_CELLS: u32 = 0xca8c8;
    const WIDE_ROW_OFFSETS: u32 = 0xca9c8;
    const WIDE_CELLS: u32 = 0xca9f2;
    /// Positions de l'indicateur de fin (mots, espace programme) et enregistrements de fin (longs).
    const FINISH_POSITIONS: u32 = 0xca0e4;
    const FINISH_RECORDS: u32 = 0xca1a4;
    const FLAGS_ARRAY: u32 = 0xff2478;

    // --- variables de A5 ---
    const INPUT_HELD: u32 = 0x0;
    const INPUT_PRESSED: u32 = 0xc;
    const BUTTONS_MASK: u32 = 0x4;
    const BUTTONS_OLD: u32 = 0xa;
    const GAME_MODE: u32 = 0x34;
    const WIDE_FLAG: u32 = 0x70;
    const BLINK_FLAGS: u32 = 0x344;

    // --- champs communs du tableau et des curseurs ---
    const WIDE: u32 = 0x13;
    const COUNT_TYPED: u32 = 0x14;
    const PAIRS: u32 = 0x15;
    const COUNT_DONE: u32 = 0x16;
    const COUNT_ACTIVE: u32 = 0x17;
    const OWNER: u32 = 0x22;
    const ACTIVE: u32 = 0x24;
    const PLAYER: u32 = 0x2f;
    // --- champs du curseur ---
    const LENGTH: u32 = 0x18;
    const VOICED_PENDING: u32 = 0x19;
    const DONE: u32 = 0x1a;
    const NO_NAME: u32 = 0x1b;
    const PREV_X: u32 = 0x1c;
    const PREV_Y: u32 = 0x1e;
    const REPEAT: u32 = 0x20;
    const TYPED: u32 = 0x21;
    const HELD_FRAMES: u32 = 0x2e;
    const COL: u32 = 0x8;
    const ROW: u32 = 0xc;
    /// Tampon de caractères du joueur : adresse relative, taille.
    const BUFFER: u32 = 0x118;
    const EMPTY: u8 = 0xff;
    const ENTER: u8 = 0x81;
    const NAME_END: u8 = 0xf0;
    // --- champs d'un enregistrement de superposition du tableau ---
    const REC_DIR: u32 = 0;
    const REC_TIMER: u32 = 1;
    const REC_COL: u32 = 2;
    const REC_ROW: u32 = 4;
    const REC_MASK: u32 = 6;
    const RECORDS: u32 = 0x30;
    const PREV_RECORDS: u32 = 0x40;
    const SLAVES: u32 = 0x18;
    const FREE_ON_NEXT: u32 = 0x6000000;
    const GRID_COLS: u16 = 11;
    const GRID_ROWS: u16 = 6;

    pub fn run(x: &mut Cx) {
        if x.u8(0x3) != 0 {
            return cursor::run(x);
        }
        match x.u8(0x4) {
            0 => {
                board::init(x);
                board::update(x);
            }
            2 => board::update(x),
            _ => board::release(x),
        }
    }

    // --- aides communes -------------------------------------------------------------------------

    /// Octet haut de l'enregistrement d'un autre objet désigné par un mot.
    fn at<R: Banc>(x: &mut Cx<'_, R>, off: u32) -> Obj {
        x.linked(off)
    }

    fn wide<R: Banc>(x: &mut Cx<'_, R>) -> bool {
        x.u8(WIDE) != 0
    }

    /// 0xca85c : touches maintenues et touches pressées de l'image pour le joueur `player`.
    fn inputs<R: Banc>(x: &mut Cx<'_, R>, player: u32) -> (u8, u8) {
        (x.a5_u8(INPUT_HELD + player), x.a5_u8(INPUT_PRESSED + player))
    }

    /// 0xca8a0 : adresse dans la carte de tuiles et taille (colonnes, lignes - 1) de la case (`col`,
    /// `row`). Rend (adresse, D0 = taille).
    fn cell_region<R: Banc>(x: &mut Cx<'_, R>, col: u16, row: u16) -> (u32, u16) {
        let (row_offsets, cells) = if wide(x) { (WIDE_ROW_OFFSETS, WIDE_CELLS) } else { (NARROW_ROW_OFFSETS, NARROW_CELLS) };
        let row_off = x.rom16(row_offsets.wrapping_add((row.wrapping_mul(2)) as i16 as i32 as u32));
        let index = col.wrapping_mul(4).wrapping_add(row_off);
        let at = |o: u32| cells.wrapping_add(o).wrapping_add(index as i16 as i32 as u32);
        let (offset, size) = (x.rom16(at(0)), x.rom16(at(2)));
        (TILEMAP + 2 + offset as i16 as i32 as u32, size)
    }

    /// 0xca86e : remplit la région `size` (octet haut : lignes - 1, octet bas : colonnes - 1) avec la
    /// tuile `tile`, un mot toutes les 4 octets.
    fn fill_tiles<R: Banc>(x: &mut Cx<'_, R>, mut a1: u32, size: u16, tile: u16) {
        let cols = size & 0xff;
        for _ in 0..=(size >> 8) as u32 {
            let mut advance = 0;
            for _ in 0..=cols as u32 {
                wr16(x.b, a1, tile);
                a1 += 4;
                advance += 4;
            }
            a1 = a1 + 0x80 - advance;
        }
    }

    /// 0xca81a : redessine la case (`col`, `row`) : tuile 0x1c, ou une teinte de la table quand elle
    /// est dans la colonne spéciale.
    fn redraw_cell<R: Banc>(x: &mut Cx<'_, R>, col: u16, row: u16) {
        let (a1, size) = cell_region(x, col, row);
        let mut tile: u16 = 0x1c;
        if !wide(x) {
            if col == 0xa {
                tile = x.rom8(SPECIAL_TILES.wrapping_add(row.wrapping_sub(1) as i16 as i32 as u32)) as u16;
            }
        } else if row == 4 && (col as i16) >= 3 {
            tile = x.rom8(SPECIAL_TILES.wrapping_add(row.wrapping_sub(3) as i16 as i32 as u32)) as u16;
        }
        fill_tiles(x, a1, size, tile);
    }

    // --- le tableau (variante 0) -------------------------------------------------------------

    mod board {
        use super::*;

        /// 0xc979e : état 0. Remet les enregistrements à zéro et charge quatre palettes (avec la
        /// luminosité de la première couleur de la page).
        pub fn init<R: Banc>(x: &mut Cx<'_, R>) {
            x.add8(0x4, 2);
            for off in [0x30, 0x34, 0x38, 0x3c] {
                x.set32(off, 0);
            }
            x.set8(COUNT_DONE, 0);
            x.set8(PAIRS, 0);
            let wide = x.a5_u16(WIDE_FLAG) != 0;
            x.set8(WIDE, wide as u8);
            let keep = rd32(x.b, 0x91c000) & KEEP_BRIGHTNESS;
            for i in 0..4u32 {
                for j in (0..PALETTE_ENTRY).step_by(4) {
                    let v = x.data32(PALETTE_SOURCE + i * 0x100 + j);
                    wr32(x.b, PALETTE_BANK + i * PALETTE_ENTRY + j, v | keep);
                }
            }
        }

        /// États 4 et 6 : libère les curseurs puis le tableau.
        pub fn release<R: Banc>(x: &mut Cx<'_, R>) {
            for i in 0..4 {
                let slave = at(x, SLAVES + 2 * i);
                slave.set_u32(x.b, 4, FREE_ON_NEXT);
            }
            x.free(&pools::SET);
        }

        /// 0xc9810 : une image. Compte les curseurs, repère les cases partagées, anime et dessine.
        pub fn update<R: Banc>(x: &mut Cx<'_, R>) {
            count_cursors(x);
            count_finished(x);
            // enregistrements de l'image précédente (cases partagées à ignorer selon le nombre)
            for (dst, src) in [(0x40, 0x30), (0x48, 0x38), (0x44, 0x34), (0x4c, 0x3c)] {
                let v = x.u32(src);
                x.set32(dst, v);
            }
            if x.u8(PAIRS) != 2 {
                if x.u8(PAIRS) == 0 {
                    x.set32(0x40, u32::MAX);
                    x.set32(0x44, u32::MAX);
                }
                x.set32(0x48, u32::MAX);
                x.set32(0x4c, u32::MAX);
            }
            for off in [0x30, 0x34, 0x38, 0x3c] {
                x.set32(off, 0);
            }
            let pairs = find_shared_cells(x);
            x.set8(PAIRS, pairs);
            animate_records(x);
            draw_cursors(x);
            for i in 0..pairs as u32 {
                let rec = x.o.0 + RECORDS + 8 * i;
                let (col, row) = (rd16(x.b, rec + REC_COL), rd16(x.b, rec + REC_ROW));
                let (a1, size) = cell_region(x, col, row);
                let mut tile: u16 = 0x18;
                if shared_in_special(x, col, row) {
                    tile = 0;
                }
                tile = (tile & 0xff00) | (tile as u8).wrapping_add(rd8(x.b, rec + REC_DIR)) as u16;
                fill_tiles(x, a1, size, tile);
            }
        }

        /// Cases où au moins deux curseurs se superposent : enregistre (colonne, ligne, masque des
        /// joueurs) pour les deux premières, balayage ligne par ligne. Rend leur nombre.
        fn find_shared_cells<R: Banc>(x: &mut Cx<'_, R>) -> u8 {
            let mut found = 0u8;
            let (mut col, mut row) = (0u16, 0u16);
            loop {
                let rec = x.o.0 + RECORDS + 8 * found as u32;
                let mut hits = 0u16;
                for i in 0..4 {
                    let slave = at(x, SLAVES + 2 * i);
                    if slave.u8(x.b, ACTIVE) != 0 && slave.u16(x.b, COL) == col && slave.u16(x.b, ROW) == row {
                        wr16(x.b, rec + REC_COL, col);
                        wr16(x.b, rec + REC_ROW, row);
                        let player = slave.u8(x.b, PLAYER) as u32;
                        let mask = rd8(x.b, rec + REC_MASK) | 1u8.wrapping_shl(player & 7);
                        wr8(x.b, rec + REC_MASK, mask);
                        hits += 1;
                    }
                }
                let last = row == GRID_ROWS - 1 && col == GRID_COLS - 1;
                if !last {
                    let mask = rd8(x.b, rec + REC_MASK);
                    wr8(x.b, rec + REC_MASK, 0);
                    if hits >= 2 {
                        found += 1;
                        wr8(x.b, rec + REC_MASK, mask);
                    }
                    col += 1;
                    if col >= GRID_COLS {
                        row += 1;
                        col = 0;
                    }
                    if found == 2 {
                        break;
                    }
                    continue;
                }
                break;
            }
            found
        }

        /// 0xc9912 et suivants : pour chaque case partagée, tient à jour le joueur « en vedette »
        /// (`REC_DIR`) et son temps d'affichage (`REC_TIMER`) en tenant compte de l'image précédente.
        fn animate_records<R: Banc>(x: &mut Cx<'_, R>) {
            for k in 0..2u32 {
                let rec = x.o.0 + RECORDS + 8 * k;
                if rd8(x.b, rec + REC_MASK) == 0 {
                    return;
                }
                let (col, row) = (rd16(x.b, rec + REC_COL), rd16(x.b, rec + REC_ROW));
                let mask = rd8(x.b, rec + REC_MASK);
                let previous = (0..2u32).map(|p| x.o.0 + PREV_RECORDS + 8 * p).find(|&p| rd16(x.b, p + REC_COL) == col && rd16(x.b, p + REC_ROW) == row);
                if let Some(old) = previous {
                    let dir = rd8(x.b, old + REC_DIR);
                    wr8(x.b, rec + REC_DIR, dir);
                    let timer = rd8(x.b, old + REC_TIMER);
                    wr8(x.b, rec + REC_TIMER, timer);
                    let old_mask = rd8(x.b, old + REC_MASK);
                    if old_mask != mask {
                        let new_bits = !old_mask & mask;
                        if new_bits != 0 {
                            // un nouveau joueur arrive : il prend la vedette
                            let first = new_bits.trailing_zeros() as u8;
                            wr8(x.b, rec + REC_DIR, first);
                            wr8(x.b, rec + REC_TIMER, 0xd);
                        } else if mask >> (dir & 7) & 1 != 0 {
                            // le joueur en vedette est toujours là
                        } else {
                            next_player(x, rec, mask);
                            continue;
                        }
                    }
                    let timer = rd8(x.b, rec + REC_TIMER).wrapping_sub(1);
                    wr8(x.b, rec + REC_TIMER, timer);
                    if timer == 0 {
                        next_player(x, rec, mask);
                    }
                } else {
                    // case nouvelle : on préfère les joueurs qui ont déjà un nom saisi
                    let mut with_name = 0u8;
                    for bit in (0..4u32).rev() {
                        let slave = at(x, SLAVES + 2 * bit);
                        if mask >> bit & 1 != 0 && slave.u8(x.b, COUNT_ACTIVE) != 0 {
                            with_name |= 1 << bit;
                        }
                    }
                    let choose = if with_name != 0 { with_name } else { mask };
                    wr8(x.b, rec + REC_TIMER, 0xc);
                    let mut d = 0u8;
                    while choose >> d & 1 == 0 {
                        d = (d + 1) & 3;
                    }
                    wr8(x.b, rec + REC_DIR, d);
                }
            }
        }

        /// 0xc998a : joueur suivant dans le masque, tour à tour (temps d'affichage 12 images).
        fn next_player<R: Banc>(x: &mut Cx<'_, R>, rec: u32, mask: u8) {
            let mut d = rd8(x.b, rec + REC_DIR);
            loop {
                d = d.wrapping_add(1) & 3;
                if mask >> d & 1 != 0 {
                    break;
                }
            }
            wr8(x.b, rec + REC_DIR, d);
            wr8(x.b, rec + REC_TIMER, 0xc);
        }

        /// 0xca7ea : la case partagée est-elle dans la colonne spéciale ? (tuile de base 0 sinon 0x18).
        fn shared_in_special<R: Banc>(x: &mut Cx<'_, R>, col: u16, row: u16) -> bool {
            if !wide(x) {
                col == 0xa
            } else {
                row == 4 && (col as i16) >= 3
            }
        }

        /// 0xca6b0 : nombre de curseurs actifs (`+0x17`) et de ceux qui ont tapé une lettre (`+0x14`).
        fn count_cursors<R: Banc>(x: &mut Cx<'_, R>) {
            let (mut active, mut typed) = (0u8, 0u8);
            for i in 0..4 {
                let slave = at(x, SLAVES + 2 * i);
                let player = Obj(slave.u16(x.b, OWNER) as i16 as i32 as u32 & 0xff_ffff);
                if player.u8(x.b, 0) != 0 && (player.u8(x.b, 4) as i8) < 4 {
                    active += 1;
                    if slave.u8(x.b, TYPED) != 0 {
                        typed += 1;
                    }
                }
            }
            x.set8(COUNT_ACTIVE, active);
            x.set8(COUNT_TYPED, typed);
        }

        /// 0xca6ea : nombre de curseurs qui ont terminé leur saisie (`+0x16`).
        fn count_finished<R: Banc>(x: &mut Cx<'_, R>) {
            let n = (0..4).filter(|&i| at(x, SLAVES + 2 * i).u8(x.b, DONE) != 0).count();
            x.set8(COUNT_DONE, n as u8);
        }

        /// 0xca78c : dessine les curseurs (leur case, teinte spéciale dans la colonne de commandes).
        fn draw_cursors<R: Banc>(x: &mut Cx<'_, R>) {
            for i in 0..4 {
                let slave = at(x, SLAVES + 2 * i);
                if slave.u8(x.b, ACTIVE) == 0 {
                    continue;
                }
                let (col, row) = (slave.u16(x.b, COL), slave.u16(x.b, ROW));
                let (a1, size) = cell_region(x, col, row);
                let base: u8 = if !wide(x) {
                    if col == 0xa { 0 } else { 0x18 }
                } else if row == 4 && (col as i16) >= 3 {
                    0
                } else {
                    0x18
                };
                let tile = base.wrapping_add(slave.u8(x.b, PLAYER)) as u16;
                fill_tiles(x, a1, size, tile);
            }
        }
    }

    // --- le curseur d'un joueur (variantes non nulles) --------------------------------------------

    mod cursor {
        use super::*;

        pub fn run(x: &mut Cx) {
            match x.u8(0x4) {
                0 => init(x),
                2 => step(x),
                4 => {}
                _ => finish_and_free(x),
            }
        }

        fn owner<R: Banc>(x: &mut Cx<'_, R>) -> Obj {
            at(x, OWNER)
        }

        /// 0xc9a74 : état 0. Un joueur qui a déjà un nom (tampon non vide) est marqué terminé ; sinon
        /// le curseur se place sur la case de départ de son joueur.
        fn init(x: &mut Cx) {
            let player = owner(x);
            x.set8(ACTIVE, 0);
            x.set8(DONE, 0);
            x.set8(HELD_FRAMES, 0);
            if player.u8(x.b, 0) == 0 || (player.u8(x.b, 4) as i8) >= 4 {
                return;
            }
            if x.a5_u8(GAME_MODE) != 1 && !(player.u32(x.b, BUFFER) == u32::MAX && player.u32(x.b, BUFFER + 4) == u32::MAX) {
                x.set8(DONE, 1);
                x.set8(0x4, 4);
                return;
            }
            let wide = x.a5_u16(WIDE_FLAG) != 0;
            x.set8(WIDE, wide as u8);
            let index = ((x.u8(PLAYER) << 2) as u16).wrapping_add((wide as u16) << 4) as u32;
            let (col, row) = (x.rom16(START_CELLS + index), x.rom16(START_CELLS + 2 + index));
            x.set16(COL, col);
            x.set16(ROW, row);
            x.add8(0x4, 2);
            x.set8(ACTIVE, 1);
            x.set8(LENGTH, 0);
            x.set8(VOICED_PENDING, 0);
            x.set8(NO_NAME, 0);
            x.set8(REPEAT, 0x1e);
            step(x);
        }

        /// 0xc9b9e : état 2, une image de saisie.
        fn step(x: &mut Cx) {
            x.set8(TYPED, 0);
            let (c, r) = (x.u16(COL), x.u16(ROW));
            x.set16(PREV_X, c);
            x.set16(PREV_Y, r);
            handle_input(x);
            x.set8(COUNT_ACTIVE, 1);
            if x.u16(COL) == x.u16(PREV_X) && x.u16(ROW) == x.u16(PREV_Y) {
                x.set8(COUNT_ACTIVE, 0);
            }
            let player = x.u8(PLAYER) as u32;
            let (held, _) = inputs(x, player);
            if held & 0x80 != 0 {
                x.add8(HELD_FRAMES, 1);
            }
        }

        /// Un déplacement : la grille répète la direction toutes les 6 images, 30 la première fois.
        fn handle_input(x: &mut Cx) {
            let player = x.u8(PLAYER) as u32;
            let (held, pressed) = inputs(x, player);
            let mut repeat = 0x1e;
            for dir in 0..4u32 {
                if held >> dir & 1 != 0 {
                    if pressed >> dir & 1 != 0 {
                        return step_cursor(x, dir, repeat);
                    }
                    repeat = 6;
                    let t = x.u8(REPEAT).wrapping_sub(1);
                    x.set8(REPEAT, t);
                    if t == 0 {
                        return step_cursor(x, dir, repeat);
                    }
                    break;
                }
            }
            if pressed & 0x10 != 0 {
                return confirm_button(x);
            }
            if pressed & 0x20 != 0 {
                return erase_button(x);
            }
            // bouton de départ : validé quand tous les autres boutons sont relâchés
            let released = !x.a5_u8(BUTTONS_OLD) & x.a5_u8(BUTTONS_MASK);
            if released >> (x.u8(PLAYER) as u32 & 31) & 1 == 0 {
                return;
            }
            if x.u8(LENGTH) == 0 {
                load_default_name(x);
            }
            finish(x);
        }

        /// 0xc9ef8 : déplace le curseur d'une case dans la direction `dir`.
        fn step_cursor(x: &mut Cx, dir: u32, repeat: u8) {
            x.set8(REPEAT, repeat);
            let (col, row) = (x.u16(COL), x.u16(ROW));
            let slot = (dir * 2 + if wide(x) { 8 } else { 0 }) as u16;
            move_cursor(x, slot, col, row);
        }

        /// Routines de déplacement (table 0xc9f26) : `slot` = direction × 2 (+ 8 en grille large).
        fn move_cursor(x: &mut Cx, slot: u16, col: u16, row: u16) {
            let width = |x: &mut Cx, table: u32, row: u16| x.data16(table.wrapping_add(row.wrapping_mul(2) as i16 as i32 as u32));
            match slot {
                // vers la droite, grille étroite
                0 => {
                    x.add16(COL, 1);
                    let w = width(x, ROW_WIDTH_NARROW, row);
                    if w < x.u16(COL) {
                        x.set16(COL, 0);
                        x.add16(ROW, 1);
                        if x.u16(ROW) > 5 {
                            x.set16(ROW, 0);
                        }
                    }
                }
                // vers la gauche, grille étroite
                2 => {
                    x.add16(COL, 0xffff);
                    if (x.u16(COL) as i16) < 0 {
                        x.add16(ROW, 0xffff);
                        if (x.u16(ROW) as i16) < 0 {
                            x.set16(ROW, 5);
                        }
                        let r = x.u16(ROW);
                        let w = width(x, ROW_WIDTH_NARROW, r);
                        x.set16(COL, w);
                    }
                }
                // vers le bas, grille étroite
                4 => {
                    if col as u8 == 0xa && row as u8 == 4 {
                        x.set16(COL, 9);
                    }
                    x.add16(ROW, 1);
                    if x.u16(ROW) > 5 {
                        x.set16(ROW, 0);
                    }
                }
                // vers le haut, grille étroite
                6 => {
                    if col as u8 == 0xa && row as u8 == 1 {
                        x.set16(COL, 9);
                    }
                    x.add16(ROW, 0xffff);
                    if (x.u16(ROW) as i16) < 0 {
                        x.set16(ROW, 5);
                    }
                }
                // vers la droite, grille large
                8 => {
                    x.add16(COL, 1);
                    let w = width(x, ROW_WIDTH_WIDE, row);
                    if w < x.u16(COL) {
                        x.set16(COL, 0);
                        x.add16(ROW, 1);
                        if x.u16(ROW) > 4 {
                            x.set16(ROW, 0);
                        }
                    }
                }
                // vers la gauche, grille large
                0xa => {
                    x.add16(COL, 0xffff);
                    if (x.u16(COL) as i16) < 0 {
                        x.add16(ROW, 0xffff);
                        if (x.u16(ROW) as i16) < 0 {
                            x.set16(ROW, 4);
                        }
                        let r = x.u16(ROW);
                        let w = width(x, ROW_WIDTH_WIDE, r);
                        x.set16(COL, w);
                    }
                }
                // vers le bas, grille large
                0xc => {
                    x.add16(ROW, 1);
                    if (x.u16(ROW) as i16) > 4 {
                        x.set16(ROW, 0);
                    }
                    clamp_wide(x);
                }
                // vers le haut, grille large
                _ => {
                    x.add16(ROW, 0xffff);
                    if (x.u16(ROW) as i16) < 0 {
                        x.set16(ROW, 4);
                    }
                    clamp_wide(x);
                }
            }
            redraw_cell(x, col, row);
        }

        /// Ramène la colonne dans la largeur de la ligne courante (grille large).
        fn clamp_wide<R: Banc>(x: &mut Cx<'_, R>) {
            let r = x.u16(ROW);
            let w = x.data16(ROW_WIDTH_WIDE.wrapping_add(r.wrapping_mul(2) as i16 as i32 as u32));
            if w < x.u16(COL) {
                x.set16(COL, w);
            }
        }

        /// 0xc9c1a : bouton 1 — choisit la case courante : commande (colonne spéciale) ou caractère.
        fn confirm_button(x: &mut Cx) {
            let special = if !wide(x) {
                x.u16(COL) == 0xa
            } else {
                x.u16(ROW) == 4 && (x.u16(COL) as i16) >= 3
            };
            if special {
                return special_command(x);
            }
            let ch = char_under_cursor(x);
            let buffer = owner(x).0 + BUFFER;
            let len = x.u8(LENGTH);
            enter_char(x, ch, buffer, len);
        }

        /// 0xcaaae : caractère de la case courante.
        fn char_under_cursor<R: Banc>(x: &mut Cx<'_, R>) -> u8 {
            let index = x.u16(COL).wrapping_add(x.u16(ROW) << 4);
            let table = if wide(x) { GRID_WIDE } else { GRID_NARROW };
            x.rom8(table.wrapping_add(index as i16 as i32 as u32))
        }

        /// 0xc9cd8 : commande de la colonne spéciale selon la ligne : 1 valider (0x81), 2 nom par
        /// défaut, 3 effacer, 4 terminer.
        fn special_command(x: &mut Cx) {
            let n = if !wide(x) { x.u16(ROW) } else { x.u16(COL).wrapping_sub(2) };
            match n {
                1 => {
                    let buffer = owner(x).0 + BUFFER;
                    let len = x.u8(LENGTH);
                    enter_char(x, ENTER, buffer, len);
                }
                2 => load_default_name(x),
                3 => erase_button(x),
                4 => finish(x),
                _ => {}
            }
        }

        /// 0xcab86 : ajoute le caractère `ch` ; 0x9f et 0x9d transforment la lettre précédente.
        fn enter_char<R: Banc>(x: &mut Cx<'_, R>, mut ch: u8, buffer: u32, len: u8) {
            if x.u8(NO_NAME) != 0 {
                return;
            }
            if ch == 0x9f || ch == 0x9d {
                let table = if ch == 0x9f { VOICED } else { SEMI_VOICED };
                if len == 0 {
                    return;
                }
                let previous = rd8(x.b, buffer + len as u32 - 1);
                if previous == 0x6f {
                    return;
                }
                let converted = x.data8(table + previous as u32);
                if converted == 0 {
                    return;
                }
                ch = converted;
                x.add8(LENGTH, 0xff);
                x.set8(VOICED_PENDING, 0);
            }
            if x.u8(VOICED_PENDING) != 0 {
                return;
            }
            store_char(x, ch, buffer);
        }

        /// 0xc9c90 : écrit le caractère dans le tampon ; nom plein → curseur sur « valider ».
        fn store_char<R: Banc>(x: &mut Cx<'_, R>, ch: u8, buffer: u32) {
            let len = x.u8(LENGTH) as u32;
            wr8(x.b, buffer + len, ch);
            x.set8(TYPED, 1);
            x.add8(LENGTH, 1);
            if x.u8(LENGTH) != 6 {
                return;
            }
            let (col, row) = (x.u16(COL), x.u16(ROW));
            x.set8(VOICED_PENDING, 1);
            x.set16(ROW, 4);
            let target = if wide(x) { 6 } else { 0xa };
            x.set16(COL, target);
            redraw_cell(x, col, row);
        }

        /// 0xc9e6a : efface le dernier caractère (ou tout le tampon après un nom par défaut).
        fn erase_button<R: Banc>(x: &mut Cx<'_, R>) {
            if x.u8(LENGTH) == 0 {
                return;
            }
            let player = owner(x);
            if x.u8(NO_NAME) == 0 {
                x.set8(VOICED_PENDING, 0);
                x.add8(LENGTH, 0xff);
                let len = x.u8(LENGTH) as u32;
                wr8(x.b, player.0 + BUFFER + len, EMPTY);
            } else {
                x.set8(VOICED_PENDING, 0);
                x.set8(NO_NAME, 0);
                player.set_u32(x.b, BUFFER, u32::MAX);
                player.set_u32(x.b, BUFFER + 4, u32::MAX);
                x.set8(LENGTH, 0);
            }
        }

        /// 0xc9d16 : charge le nom par défaut du joueur dans le tampon.
        fn load_default_name<R: Banc>(x: &mut Cx<'_, R>) {
            x.set8(NO_NAME, 0);
            let player = owner(x);
            let mut offset = (player.u8(x.b, 3) as u16) << 3;
            if wide(x) {
                offset += 0x60;
            }
            let mut length = 0u8;
            for i in 0..8u32 {
                let mut c = x.data8(DEFAULT_NAMES + offset as u32 + i);
                wr8(x.b, player.0 + BUFFER + i, c);
                length = length.wrapping_add(1);
                if c == NAME_END {
                    length = length.wrapping_sub(1);
                    c = EMPTY;
                    wr8(x.b, player.0 + BUFFER + i, c);
                }
            }
            x.set8(LENGTH, length);
            if length == 6 {
                x.set8(VOICED_PENDING, 1);
            }
            if player.u8(x.b, BUFFER) >= NAME_END {
                x.set8(NO_NAME, 1);
                x.set8(LENGTH, 1);
            }
            let (col, row) = (x.u16(COL), x.u16(ROW));
            redraw_cell(x, col, row);
            x.set16(ROW, 4);
            let target = if wide(x) { 6 } else { 0xa };
            x.set16(COL, target);
        }

        /// 0xc9eb0 : termine la saisie (nom vide → nom par défaut), contrôle les codes secrets, calcule
        /// le chiffre de contrôle et écrit le résultat dans le joueur.
        fn finish(x: &mut Cx) {
            let n;
            if x.u8(LENGTH) == 0 {
                load_default_name(x);
                x.add16(ROW, 0xfffe);
                n = special_codes(x);
                scan_reset_codes(x, n);
            } else {
                let (col, row) = (x.u16(COL), x.u16(ROW));
                redraw_cell(x, col, row);
                n = special_codes(x);
                scan_reset_codes(x, n);
                check_repeated_name(x);
            }
            check_digit(x);
            commit_name(x);
        }

        /// 0xca33c : un tampon de lettres identiques ou entièrement « valider » redevient le nom par
        /// défaut.
        fn check_repeated_name(x: &mut Cx) {
            let player = owner(x);
            let b = |x: &mut Cx, i: u32| player.u8(x.b, BUFFER + i);
            let first = b(x, 0);
            let mut same = true;
            for i in 0..5 {
                if b(x, i) != first {
                    same = false;
                    break;
                }
            }
            if same {
                return load_default_name(x);
            }
            for i in 0..6 {
                let c = b(x, i);
                if c == EMPTY {
                    return load_default_name(x);
                }
                if c != ENTER {
                    return;
                }
            }
            load_default_name(x);
        }

        /// 0xca374 : recherche une suite de la table des codes de remise à zéro dans le tampon.
        /// `n` : indicateur N laissé par 0xca524 (le balayage n'a lieu que s'il est levé).
        fn scan_reset_codes<R: Banc>(x: &mut Cx<'_, R>, n: bool) {
            if !n {
                return;
            }
            let buffer = owner(x).0 + BUFFER;
            if find_code(x, buffer, RESET_CODES, 0).is_some() {
                load_default_name(x);
            }
        }

        /// Cherche dans les 6 octets du tampon une suite de la table (octet de longueur, caractères,
        /// puis `trailer` octets d'action), comme les routines 0xca384 et 0xca54c : la première lettre
        /// est cherchée plus loin dans le tampon ; un désaccord relit la lettre fautive comme début de
        /// suite sans rembobiner le compteur. Rend l'adresse qui suit les caractères de la suite trouvée.
        fn find_code<R: Banc>(x: &mut Cx<'_, R>, buffer: u32, table: u32, trailer: u32) -> Option<u32> {
            enum Step {
                Start,
                ReadLength,
                Search,
                Continue,
                NextPattern,
            }
            let (mut step, mut a4, mut a3, mut a1) = (Step::Start, table, table, buffer);
            let (mut left, mut remaining, mut first) = (0u16, 5i32, 0u8);
            loop {
                match step {
                    Step::Start => {
                        a1 = buffer;
                        remaining = 5;
                        a3 = a4;
                        step = Step::ReadLength;
                    }
                    Step::ReadLength => {
                        let length = x.data8(a4);
                        a4 += 1;
                        if (length as i8) < 0 {
                            return None;
                        }
                        left = length as u16;
                        first = x.data8(a4);
                        a4 += 1;
                        step = Step::Search;
                    }
                    Step::Search => {
                        let c = rd8(x.b, a1);
                        a1 += 1;
                        if c == first {
                            step = Step::Continue;
                        } else {
                            remaining -= 1;
                            if remaining < 0 {
                                step = Step::NextPattern;
                            }
                        }
                    }
                    Step::Continue => {
                        left = left.wrapping_sub(1);
                        if left == 0 {
                            return Some(a4);
                        }
                        remaining -= 1;
                        if remaining < 0 {
                            step = Step::NextPattern;
                            continue;
                        }
                        let want = x.data8(a4);
                        a4 += 1;
                        let c = rd8(x.b, a1);
                        a1 += 1;
                        if want != c {
                            a1 -= 1;
                            a4 = a3;
                            step = Step::ReadLength;
                        }
                    }
                    Step::NextPattern => {
                        a4 = a3;
                        let length = x.data8(a4) as u32;
                        a4 += 1 + length + trailer;
                        step = Step::Start;
                    }
                }
            }
        }

        /// 0xca524 : si le joueur maintient les trois boutons (0x70) plus d'une seconde, cherche un
        /// code spécial et remplace le tampon. Rend l'indicateur N laissé par la routine.
        fn special_codes<R: Banc>(x: &mut Cx<'_, R>) -> bool {
            let player = x.u8(PLAYER) as u32;
            let (_, pressed) = inputs(x, player);
            let n_of = |a: u8, b: u8| (a.wrapping_sub(b) as i8) < 0;
            if pressed != 0x70 {
                return n_of(pressed, 0x70);
            }
            let held_frames = x.u8(HELD_FRAMES);
            if held_frames <= 0x3c {
                return n_of(held_frames, 0x3c);
            }
            let buffer = owner(x).0 + BUFFER;
            match find_code(x, buffer, SPECIAL_CODES, 1) {
                None => true,
                Some(after) => {
                    let action = x.data8(after) as u32;
                    let src = SPECIAL_REPLACEMENTS + (action << 3);
                    let (a, b) = (x.data32(src), x.data32(src + 4));
                    wr32(x.b, buffer, a);
                    wr32(x.b, buffer + 4, b);
                    (b as i32) < 0
                }
            }
        }

        /// 0xca5fe : chiffre de contrôle du nom en BCD (`+0x2a..+0x2d`, résultat dans `+0x2b`).
        fn check_digit<R: Banc>(x: &mut Cx<'_, R>) {
            let buffer = owner(x).0 + BUFFER;
            let first = rd8(x.b, buffer) as i8 as i16;
            let d0 = (-first).wrapping_sub(2) as u16;
            if d0 < 0x10 {
                let v = x.rom8(DEFAULT_CHECK + (d0 & 0xff) as u32);
                x.set8(0x2b, v);
                return;
            }
            for i in 0..6 {
                let c = rd8(x.b, buffer + i);
                if c == 0xff {
                    continue;
                }
                x.set8(0x2c, 0);
                let digit = x.data8(CHAR_DIGITS + c as u32);
                x.set8(0x2d, digit);
                add_bcd(x);
            }
            while x.u8(0x2a) != 0 {
                let carry_in = x.u8(0x2a);
                x.set8(0x2d, carry_in);
                x.set8(0x2c, 0);
                x.set8(0x2a, 0);
                add_bcd(x);
            }
            while x.u8(0x2b) >= 0x10 {
                let high = x.u8(0x2b) >> 4;
                let low = x.u8(0x2b) & 0xf;
                x.set8(0x2b, low);
                x.set8(0x2d, high);
                let (v, _) = abcd(x.u8(0x2d), x.u8(0x2b), false);
                x.set8(0x2b, v);
            }
        }

        /// Ajoute la valeur BCD (`+0x2c:+0x2d`) à (`+0x2a:+0x2b`) avec retenue (deux ABCD).
        fn add_bcd<R: Banc>(x: &mut Cx<'_, R>) {
            let (low, carry) = abcd(x.u8(0x2d), x.u8(0x2b), false);
            x.set8(0x2b, low);
            let (high, _) = abcd(x.u8(0x2c), x.u8(0x2a), carry);
            x.set8(0x2a, high);
        }

        /// ABCD : `dst + src + retenue` en BCD ; rend (résultat, retenue).
        fn abcd(src: u8, dst: u8, carry: bool) -> (u8, bool) {
            let mut r = (src & 0xf) as u16 + (dst & 0xf) as u16 + carry as u16;
            if r > 9 {
                r += 6;
            }
            r += (src & 0xf0) as u16 + (dst & 0xf0) as u16;
            let carry = r > 0x99;
            if carry {
                r -= 0xa0;
            }
            (r as u8, carry)
        }

        /// 0xca0f4 : écrit le résultat : indicateur de fin à l'écran, enregistrement du joueur
        /// (nom validé, chiffre de contrôle), le curseur passe à l'état 4.
        fn commit_name<R: Banc>(x: &mut Cx<'_, R>) {
            let player = owner(x);
            let kind = player.u8(x.b, 2) as u16;
            let mut index = kind.wrapping_mul(2);
            if x.a5_u8(BLINK_FLAGS) & 1 != 0 {
                index = index.wrapping_add(8);
            }
            let offset = x.rom16(FINISH_POSITIONS.wrapping_add(index as i16 as i32 as u32));
            let mut a1 = TILEMAP.wrapping_add(offset as i16 as i32 as u32);
            let mut tile: u32 = 0x6700013;
            tile = tile & !0xff | (tile as u8).wrapping_add(kind as u8) as u32;
            for _ in 0..4 {
                wr32(x.b, a1, tile);
                a1 += 0x80;
                tile = tile.wrapping_add(0x10000);
            }
            let swapped = tile.rotate_left(16);
            let swapped = swapped & !0xff | (swapped as u8).wrapping_add(x.u8(0x2b)) as u32;
            wr32(x.b, a1, swapped.rotate_left(16));
            x.set32(0x4, 0x4000000);
            x.set8(DONE, 1);
            x.set8(ACTIVE, 0);
            let check = (x.u8(0x2b) << 2) as u32;
            let record = {
                let at = FINISH_RECORDS + check;
                (x.rom16(at) as u32) << 16 | x.rom16(at + 2) as u32
            };
            let variant = (player.u8(x.b, 3) >> 1) as u32;
            let a1 = record + variant * 5;
            let (i0, v0, i1, v1, flag) = (x.data8(a1) as u32, x.data8(a1 + 1), x.data8(a1 + 2) as u32, x.data8(a1 + 3), x.data8(a1 + 4) as u32);
            wr8(x.b, player.0 + 0x160 + i0, v0);
            if (i1 as u8 as i8) >= 0 {
                wr8(x.b, player.0 + 0x16b + i1, v1);
            }
            wr8(x.b, FLAGS_ARRAY + flag, 1);
        }

        /// 0xca2f8 : état 6. Si le joueur est encore là et la saisie pas terminée, la termine d'office ;
        /// puis libération.
        fn finish_and_free(x: &mut Cx) {
            let player = owner(x);
            if player.u8(x.b, 0) != 0 && (player.u8(x.b, 4) as i8) < 4 && x.u8(DONE) == 0 {
                x.set8(DONE, 1);
                finish(x);
            }
            x.free(&pools::SET);
        }
    }
}

/// Déclare une native de gestionnaire : adresse, nom, fonction.
macro_rules! entry {
    ($entry:literal $name:literal, $(#[$sans:ident])? $f:path) => {
        super::cx::handlers!(@une $entry $name $(#[$sans])? $f)
    };
}

pub const NATIVES: &[crate::Native] = &[
    entry!(0xcc25a "set_commun_cc25a", scene_spawner::run),
    entry!(0xba310 "set_0", #[sans] palette_script::run),
    entry!(0xbc518 "set_1", wave_leader::run),
    entry!(0xc935a "set_4", tile_anim::run4),
    entry!(0xc9780 "set_5", name_entry::run),
    entry!(0xcafec "set_6", tile_anim::run6),
    entry!(0xcb29c "set_7", exit_gate::run),
    entry!(0xcc2c8 "set_9", #[sans] inert::run),
    entry!(0xcc3c2 "set_10", dialogue::run),
    entry!(0xcdcaa "set_11", #[sans] level_end::run),
    entry!(0xcdd70 "set_12", #[sans] fade_white::run),
    entry!(0xcdef6 "set_13", #[sans] palette_anim::run),
    entry!(0xcdf86 "set_14", #[sans] scene_trigger::run),
];
