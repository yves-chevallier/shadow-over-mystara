//! Lecteur de script d'apparition (`0xf2e5c`) : fait entrer ennemis, décors et objets au fil du défilement.
//!
//! Chaque scène a un script en ROM (table de pointeurs `0xf2f62`, un long par scène) : une suite
//! d'entrées triées par abscisse de caméra croissante. Le lecteur tourne à chaque image de jeu (et
//! d'entrée de scène) ; il consomme les entrées dont l'abscisse est atteinte par la caméra, puis
//! attend. Deux formats d'entrée, distingués par le mot `+2` :
//!
//! | Entrée | Taille | Contenu |
//! |---|---|---|
//! | apparition | 0x12 | `+0` abscisse de déclenchement, `+2` décalage en X, `+4` Z, `+6` Y, `+8` rang maximal, `+9` joueurs minimaux, `+0xb` classe (octet pair, index dans la table des allocateurs), `+0xc` `+0x2c`, `+0xd` type, `+0xe` variante, `+0xf` `+0x2f`, `+0x10` mot `+0x42` de l'objet |
//! | appel | 10 | `+0` abscisse, `+2` = 0xffff, `+4` adresse d'une routine (long), `+8` son argument D0 (mot) |
//!
//! Un mot `+0` égal à 0xffff termine le script. Une apparition dont le rang dépasse le maximum de
//! l'entrée (`A5+0x7f93`) ou qui exige plus de joueurs que `A5+0x35` est sautée ; si le pool de la
//! classe est plein, la lecture s'arrête et reprend à la même entrée à l'image suivante.
//!
//! États (octet `A5-0x55cc`) : 0 charge le script de la scène, 2 lit, 4 passe à 6, 6 fini.

use cps2::Cps2Bus;
use m68k::decode::Fetch;
use m68k::{Cpu, SR_C};

use crate::mem::{rd16, rd32, rd8, rts, set_low_byte, set_low_word, wr32, wr8, A5};
use crate::objects::Obj;
use crate::scene::var;
use crate::{call, Native};

/// État du lecteur (octet) et curseur dans le script (long).
const STATE: u32 = A5 - 0x55cc;
const CURSOR: u32 = A5 - 0x55d0;
/// Table des scripts (un long par scène, en données) et des allocateurs par classe (mots, espace programme).
const SCRIPTS: u32 = 0xf2f62;
const ALLOCATORS: u32 = 0xf2f38;
/// Table de branchement des états (mots, espace programme).
const STATE_JUMPS: u32 = 0xf2e70;
/// Rang du jeu (octet bas du mot `A5+0x7f92`).
const RANK: u32 = 0x7f93;

mod state {
    pub const LOAD: u8 = 0;
    pub const READ: u8 = 2;
    pub const ENDING: u8 = 4;
}

/// Fin du script (mot `+0`) et marque d'une entrée d'appel (mot `+2`).
const END: u16 = 0xffff;
const CALL_MARK: u16 = 0xffff;
const CALL_ENTRY_SIZE: u32 = 10;
const SPAWN_ENTRY_SIZE: u32 = 0x12;

/// Champs d'une entrée d'apparition.
mod entry {
    pub const TRIGGER_X: u32 = 0;
    pub const OFFSET_X: u32 = 2;
    pub const Z: u32 = 4;
    pub const Y: u32 = 6;
    pub const MAX_RANK: u32 = 8;
    pub const MIN_PLAYERS: u32 = 9;
    pub const CLASS: u32 = 0xb;
    pub const FIELD_2C: u32 = 0xc;
    pub const KIND: u32 = 0xd;
    pub const VARIANT: u32 = 0xe;
    pub const FIELD_2F: u32 = 0xf;
    pub const FIELD_42: u32 = 0x10;
}

/// Un pas du lecteur (une image).
pub fn step(c: &mut Cpu, b: &mut Cps2Bus) {
    if rd8(b, A5 + var::PLAYERS) == 0 {
        return;
    }
    let state = rd8(b, STATE);
    c.d[0] = state as u32;
    set_low_word(&mut c.d[1], b.fetch16(STATE_JUMPS + state as u32));
    let mut cursor = match state {
        state::LOAD => {
            wr8(b, STATE, state::READ);
            c.d[0] = (rd8(b, A5 + var::SCENE) as u32 * 4) & 0xffff;
            let script = rd32(b, SCRIPTS + c.d[0]);
            wr32(b, CURSOR, script);
            script
        }
        state::READ => rd32(b, CURSOR),
        state::ENDING => {
            wr8(b, STATE, state::ENDING + 2);
            return;
        }
        _ => return,
    };
    let camera = A5 + if rd8(b, A5 + var::USE_ALT_CAMERA) != 0 { var::ALT_CAMERA } else { var::CAMERA };
    c.a[4] = camera;
    loop {
        c.a[6] = cursor;
        let trigger = rd16(b, cursor + entry::TRIGGER_X);
        set_low_word(&mut c.d[6], trigger);
        if trigger == END {
            wr8(b, STATE, state::ENDING);
            return;
        }
        if trigger as i16 > rd16(b, camera + 8) as i16 {
            wr32(b, CURSOR, cursor);
            return;
        }
        if rd16(b, cursor + entry::OFFSET_X) == CALL_MARK {
            c.a[1] = rd32(b, cursor + 4);
            set_low_word(&mut c.d[0], rd16(b, cursor + 8));
            call(c, b, c.a[1]);
            cursor += CALL_ENTRY_SIZE;
            continue;
        }
        if spawn(c, b, cursor) == Spawn::PoolFull {
            wr32(b, CURSOR, cursor);
            return;
        }
        cursor += SPAWN_ENTRY_SIZE;
    }
}

#[derive(PartialEq)]
enum Spawn {
    /// Créé, ou sauté (rang ou nombre de joueurs insuffisant).
    Done,
    /// Pas de place dans le pool : on réessaie à l'image suivante.
    PoolFull,
}

/// Une entrée d'apparition.
fn spawn(c: &mut Cpu, b: &mut Cps2Bus, e: u32) -> Spawn {
    let max_rank = rd8(b, e + entry::MAX_RANK);
    set_low_byte(&mut c.d[6], max_rank);
    if max_rank > rd8(b, A5 + RANK) {
        return Spawn::Done;
    }
    let min_players = rd8(b, e + entry::MIN_PLAYERS);
    set_low_byte(&mut c.d[6], min_players);
    if min_players > rd8(b, A5 + var::PLAYERS) {
        return Spawn::Done;
    }
    let class = rd8(b, e + entry::CLASS) as i8 as i16 as u16;
    set_low_word(&mut c.d[6], class);
    let alloc = b.fetch16(ALLOCATORS + class as i16 as i32 as u32) as i16 as i32 as u32;
    c.a[1] = alloc;
    call(c, b, alloc);
    if c.sr & SR_C != 0 {
        return Spawn::PoolFull;
    }
    let o = Obj(c.a[1]);
    let offset = rd16(b, e + entry::OFFSET_X);
    set_low_word(&mut c.d[0], offset);
    let bytes = [
        (0x2c, rd8(b, e + entry::FIELD_2C)),
        (Obj::KIND, rd8(b, e + entry::KIND)),
        (Obj::VARIANT, rd8(b, e + entry::VARIANT)),
        (0x2f, rd8(b, e + entry::FIELD_2F)),
    ];
    let words = [
        (Obj::X, rd16(b, e + entry::TRIGGER_X).wrapping_add(offset)),
        (Obj::Y, rd16(b, e + entry::Y)),
        (Obj::Z, rd16(b, e + entry::Z)),
        (0x42, rd16(b, e + entry::FIELD_42)),
    ];
    o.set_u8(b, Obj::ACTIVE, 1);
    for (off, v) in bytes {
        o.set_u8(b, off, v);
    }
    for (off, v) in words {
        o.set_u16(b, off, v);
    }
    Spawn::Done
}

/// Entrées des deux routines.
pub const SPAWN_ENTRY: u32 = 0xf2e5c;
pub const RESET_ENTRY: u32 = 0xf2e56;

pub const SPAWN_SCRIPT: Native = Native {
    entry: SPAWN_ENTRY,
    name: "scene_script",
    run: crate::Corps::Processeur(|c, b| {
        step(c, b);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: 0,
};

/// Remet le lecteur à l'état « charger le script » (changement de scène).
pub const SCRIPT_RESET: Native = Native {
    entry: RESET_ENTRY,
    name: "scene_script_reset",
    run: crate::Corps::Memoire(|b| wr8(b, STATE, state::LOAD)),
    touches_sound: false,
    outputs: 0,
};
