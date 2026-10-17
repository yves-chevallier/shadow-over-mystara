//! Objets de décor (classe SET, pool de 32 objets de 0x50 octets, table 0x39eac) : types 0, 1 et
//! 3 à 14. Les objets SET ne figurent pas dans les listes triées (voir `docs/objets.md`).

use super::cx::{handlers, sub, Cx};
use crate::objects::Obj;
use m68k::SR_C;

/// Type 8 (0xcc25a), partagé par les tables SET (8), SOBJ (4) et EFFCT (18) : générateur du
/// contrôleur de scène. À la première image il attend ; à la suivante il crée un objet SET de
/// type 2 (le contrôleur, de variante `+0x2f`) puis passe à l'état 6. Dans les états 4 et 6 il se
/// libère dans le pool de sa classe (EFFCT, SET ou SOBJ selon `+0x28`).
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
                x.call(sub::ALLOC_SET);
                if x.c.sr & SR_C != 0 {
                    return;
                }
                let child = Obj(x.c.a[1]);
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
    pub fn alive(x: &mut Cx, p: Obj) -> bool {
        p.u8(x.b, P_ACTIVE) != 0 && p.u16(x.b, P_LIFE) != 0
    }

    /// Alloue un objet SET (0x2348). `None` quand le pool est plein.
    pub fn alloc(x: &mut Cx) -> Option<Obj> {
        x.call(sub::ALLOC_SET);
        (x.c.sr & SR_C == 0).then(|| Obj(x.c.a[1]))
    }

    /// Avance chaque composante (rouge, vert, bleu : un quartet chacune) d'une couleur 4+4+4 bits
    /// d'un cran vers son maximum, et force le quartet de luminosité à 0xf.
    pub fn brighten(x: &mut Cx, addr: u32, count: u32) {
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

    pub fn run(x: &mut Cx) {
        if x.u8(0x40) != 0 {
            return;
        }
        match x.u8(0x4) {
            0 => x.add8(0x4, 2),
            2 => {}
            _ => x.call(sub::FREE_SET),
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

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                wait_players(x);
            }
            2 => wait_players(x),
            _ => x.call(sub::FREE_SET),
        }
    }

    fn wait_players(x: &mut Cx) {
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

    pub fn run(x: &mut Cx) {
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
            _ => x.call(sub::FREE_SET),
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

    pub fn run(x: &mut Cx) {
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
                    x.call(sub::FREE_SET);
                }
            }
            4 => {
                x.add8(0x4, 2);
                x.call(sub::FREE_SET);
            }
            _ => x.call(sub::FREE_SET),
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

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                trigger(x);
            }
            2 => trigger(x),
            _ => x.call(sub::FREE_SET),
        }
    }

    fn trigger(x: &mut Cx) {
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
            _ => x.call(sub::FREE_SET),
        }
    }

    /// Un pas : attend la fin de la durée du bloc courant, puis applique le bloc suivant.
    fn step(x: &mut Cx, skip_transparent: bool) {
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
    const ALLOC_ENEMY: u32 = 0x22bc;
    const RNG: u32 = 0x19a88;
    const CAMERA_LOCK: u32 = 0x2860;
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
            _ => x.call(sub::FREE_SET),
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
                x.c.d[0] = x.c.d[0] & !0xffff | CAMERA_LOCK_PARAM;
                x.call(CAMERA_LOCK);
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
                    x.call(RNG);
                    let delay = x.rom8(DELAYS + (x.c.d[0] & 0xf));
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
    fn release_one(x: &mut Cx, boss: Obj) {
        let n = boss.u16(x.b, BOSS_COUNT).wrapping_sub(1);
        boss.set_u16(x.b, BOSS_COUNT, n);
    }

    /// Vague finale : le meneur est de variante 2 et le verdict de vague passe à 0xff.
    fn final_wave(x: &mut Cx) {
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
    fn spawn_wave(x: &mut Cx) -> Option<Obj> {
        if (x.a5_u16(ENEMY_FREE) as i16) < ENEMIES_PER_WAVE {
            return None;
        }
        let mut slots = [Obj(0); 3];
        for slot in &mut slots {
            x.call(ALLOC_ENEMY);
            *slot = Obj(x.c.a[1]);
            slot.set_u8(x.b, 0, 1);
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
            x.call(RNG);
            side = ((x.c.d[0] & 1) << 5) as u8;
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

handlers! {
    0xcc25a "set_commun_cc25a" 120 scene_spawner::run,
    0xbc518 "set_1" 600 wave_leader::run,
    0xc935a "set_4" 600 tile_anim::run4,
    0xcafec "set_6" 600 tile_anim::run6,
    0xcc2c8 "set_9" 120 inert::run,
    0xcdcaa "set_11" 400 level_end::run,
    0xcdd70 "set_12" 1500 fade_white::run,
    0xcdef6 "set_13" 120 palette_anim::run,
    0xcdf86 "set_14" 200 scene_trigger::run,
}
