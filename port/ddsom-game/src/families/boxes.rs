//! Famille BOX (classe 0xA) : caisses, tonneaux et coffres destructibles. Dispatch `0x39e50`, table
//! des types `0x39e76` (4 types). 21 emplacements de 0x80 octets. Voir `docs/families.md`.

use crate::objects::pools;
use crate::objects::Obj;

use super::{ajoute_u8, handler, pointeur};
use super::cx::{Bench, Cx};
use crate::objects::Current;
use crate::machine::Machine;

/// Valeurs de l'octet d'état `+4`.
mod state {
    pub const INIT: u8 = 0;
    pub const ACTIVE: u8 = 2;
    pub const LIBERE: u8 = 4;
    pub const OUTSIDE_SCREEN: u8 = 6;
}

/// Champs communs des BOX.
mod field {
    pub const SUB_STATE: u32 = 0x05;
    pub const MODE: u32 = 0x2f;
    pub const GAME_SHADOWS: u32 = 0x2e;
    /// Coup reçu, posé par le code des joueurs.
    pub const COUP_B: u32 = 0x6e;
}

mod rom {
    pub const ANIMATIONS: u32 = 0x1bebc2;
}

const SHADOWS: u8 = 0x35;

// ---------------------------------------------------------------------------------------------
// Type 0 : caisse, tonneau ou coffre (0xd99fa)

/// Type 0 : caisse, tonneau ou coffre destructible (variantes 0 à 3). Une fiche de la ROM donne ses
/// caractéristiques ; frappée, elle joue son animation de casse, lâche jusqu'à trois objets ITEM puis
/// clignote et disparaît. Un coffre (sorte >= 3) tremble quand on le frappe ; il s'ouvre (sous-état 2)
/// quand un joueur l'ouvre, en consommant une clé s'il est verrouillé. Un joueur peut aussi soulever la
/// caisse (sous-état 4 : elle suit ses mains), la lancer (6 : vol, rebond sur le bord de l'écran,
/// casse à l'atterrissage) ou la lâcher (8 : chute).
pub mod caisse {
use crate::families::cx::Bench;
use crate::objects::Current;
    use crate::families::cx::{handlers, Cx};
    use crate::mem::A5;
    use crate::objects::{pools, Obj};
    use crate::families::effects_creators;
    use crate::machine::Machine;

    /// Sous-états `+5` de l'état actif.
    mod sub {
        pub const INTACTE: u8 = 0;
        pub const OUVERTE: u8 = 2;
        pub const PORTEE: u8 = 4;
        pub const LANCEE: u8 = 6;
        pub const LACHEE: u8 = 8;
        pub const TOUCHEE: u8 = 0x0a;
    }

    mod f {
        pub const ACTIVE: u32 = 0x00;
        pub const FLAGS: u32 = 0x01;
        pub const VARIANT: u32 = 0x03;
        pub const STATE: u32 = 0x04;
        pub const SUB_STATE: u32 = 0x05;
        pub const SUB_SUB_STATE: u32 = 0x06;
        pub const X: u32 = 0x08;
        pub const Z: u32 = 0x0c;
        pub const Y: u32 = 0x10;
        pub const VX: u32 = 0x14;
        pub const AX: u32 = 0x16;
        pub const VZ: u32 = 0x18;
        pub const AZ: u32 = 0x1a;
        pub const VY: u32 = 0x1c;
        pub const RETOURNE: u32 = 0x2c;
        pub const GAME_SHADOWS: u32 = 0x2e;
        /// Coup reçu, seconde marque posée par le code des joueurs.
        pub const COUP: u32 = 0x33;
        /// Non nul : en l'air (pas d'ombre au sol).
        pub const IN_FLIGHT: u32 = 0x3a;
        pub const HIDDEN: u32 = 0x40;
        /// Délai avant que la caisse lancée ne reprenne son vol (`+0x53`, modulo 8).
        pub const WAIT_FLIGHT: u32 = 0x53;
        pub const SELF: u32 = 0x5c;
        /// Variété (0 à 2 : se brise d'un coup ; 3 et plus : coffre qui résiste).
        pub const KIND: u32 = 0x60;
        /// Non nul : coffre verrouillé (une clé `A5+0x45` l'ouvre, le personnage de classe 5 s'en passe).
        pub const LOCK: u32 = 0x61;
        /// Butins : au centre, par classe du joueur (`+0x63..`), bonus (`+0x69`), à gauche (`+0x6a`).
        pub const LOOT_HIGH: u32 = 0x62;
        pub const BUTIN_CLASSES: u32 = 0x63;
        pub const BUTIN_BONUS: u32 = 0x69;
        pub const LOOT_LEFT: u32 = 0x6a;
        /// Effet spécial éventuel (`+0x6b`), marque « effet joué » (`+0x7d`).
        pub const SPECIAL: u32 = 0x6b;
        /// Joueur qui a frappé, ouvert ou porte la caisse (pointeur RAM 16 bits).
        pub const PLAYER: u32 = 0x6c;
        pub const COUP_B: u32 = 0x6e;
        pub const DELAY: u32 = 0x6f;
        /// Coffre ouvert par un joueur de classe 3 : un second coup lâche le butin bonus.
        pub const SECOND_COUP: u32 = 0x70;
        /// Non nul : la caisse lancée ignore le décor.
        pub const WITHOUT_SCENERY: u32 = 0x71;
        pub const EFFECT_CREATED: u32 = 0x76;
        /// Non nul : le coffre ouvert reste frappable.
        pub const FRAPPABLE: u32 = 0x77;
        pub const ORIGINAL_X: u32 = 0x78;
        pub const ORIGINAL_Y: u32 = 0x7a;
        pub const BUTIN_LACHE: u32 = 0x7c;
        pub const SPECIAL_PLAYED: u32 = 0x7d;
        pub const BONUS_LACHE: u32 = 0x7e;
        /// Le coffre lancé a rebondi sur le décor.
        pub const REBOND: u32 = 0x7f;
    }

    /// Champs du joueur lié.
    mod j {
        pub const CLASS: u32 = 0x03;
        pub const RETOURNE: u32 = 0x2c;
        /// Pose du joueur qui porte (0 : il ne porte plus).
        pub const POSE: u32 = 0x46;
    }

    /// Tables et sous-routines.
    mod rom {
        pub const STATES: u32 = 0xd9a10;
        pub const SUB_STATES: u32 = 0xd9a6a;
        pub const TOUCHEE: u32 = 0xd9abc;
        pub const OUVERTE: u32 = 0xd9b94;
        pub const LANCEE: u32 = 0xd9d64;
        pub const LACHEE: u32 = 0xd9f82;
        pub const ANIMATIONS: u32 = 0x1bebc2;
        /// Par variété : adresse de sa fiche de 0x2a octets (caractéristiques, butins aléatoires).
        pub const SHEETS: u32 = 0x1051ae;
        /// Par numéro de butin : (type, variante) de l'ITEM créé (lu relativement au PC).
        pub const BUTINS: u32 = 0xda28a;
        /// Effets spéciaux (adresses de routines), par valeur de `SPECIAL`.
        pub const SPECIAUX: u32 = 0xda06a;
        /// Par classe du porteur : table des décalages (X, Z) de la caisse portée, par pose.
        pub const PRISES: u32 = 0xda398;
        // sous-routines non réécrites, ou natives
        pub const SOUND_HIT: u32 = 0xeb59e;
        pub const SOUND_BROKEN: u32 = 0xeb610;
        pub const SOUND_LANDS: u32 = 0xeb52c;
        pub const SOUND_SET_DOWN: u32 = 0xeb766;
    }

    /// Caisses brisées dans la scène (octet, `A5-0x5b21`).
    const CASSEES: u32 = A5 - 0x5b21;
    /// Clés des joueurs (octet, `A5+0x45`).
    const CLES: u32 = A5 + 0x45;
    const SHADOWS: u8 = 0x35;
    const DURATION_BLINK: u8 = 0x1e;

    fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::HIDDEN) != 0 {
            return;
        }
        let state = x.u8(f::STATE);
        let target = jump(x, rom::STATES, state);
        match state {
            0 => init(x),
            2 => active(x),
            4 | 6 => x.free(&pools::BOX),
            _ => crate::families::effects::outside_table(target),
        }
    }

    /// Cible du saut par table `move.w (table,PC,D0.w),D1.w ; jmp` pour l'indice `i` (`outside_table`).
    fn jump<R: Bench>(x: &mut Cx<'_, R>, table: u32, i: u8) -> u32 {
        let w = x.rom16(table + i as u32);
        table.wrapping_add(w as i16 as i32 as u32)
    }

    /// Classe du joueur lié (`+3 >> 1`).
    fn class<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
        let player = x.linked(f::PLAYER);
        player.u8(x.b, j::CLASS) >> 1
    }

    fn count_broken<R: Bench>(x: &mut Cx<'_, R>) {
        let n = crate::mem::rd8(x.b, CASSEES).wrapping_add(1);
        crate::mem::wr8(x.b, CASSEES, n);
    }

    /// `0xda082` : recopie la fiche de la variété (10 octets) et tire deux butins au hasard.
    fn charge_sheet<R: Bench>(x: &mut Cx<'_, R>) {
        let variant = x.u8(f::VARIANT) as u32;
        let sheet = x.data32(rom::SHEETS + 4 * variant);
        let fields = [f::KIND, f::LOCK, f::LOOT_HIGH, f::BUTIN_BONUS, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68];
        for (i, off) in fields.into_iter().enumerate() {
            let v = x.data8(sheet + i as u32);
            x.set8(off, v);
        }
        let draw = (x.random() & 0xf) as u32;
        let v = x.data8(sheet + 0xa + draw);
        x.set8(f::LOOT_LEFT, v);
        let draw = (x.random() & 0xf) as u32;
        let v = x.data8(sheet + 0x1a + draw);
        x.set8(f::SPECIAL, v);
        // pointeurs de lecture de la fiche laissés par l'original
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(f::STATE, 2);
        x.set8(f::SECOND_COUP, 0);
        x.set8(f::COUP_B, 0);
        x.set8(f::GAME_SHADOWS, SHADOWS);
        let px = x.u16(f::X);
        x.set16(f::ORIGINAL_X, px);
        let py = x.u16(f::Y);
        x.set16(f::ORIGINAL_Y, py);
        x.set16(f::SELF, x.o.0 as u16);
        charge_sheet(x);
        if x.u8(f::SPECIAL) != 0 {
            effects_creators::shimmer(x.b, x.o);
        }
        let kind = x.u8(f::KIND) as u32;
        x.anim(rom::ANIMATIONS, kind);
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        let sub_state = x.u8(f::SUB_STATE);
        let target = jump(x, rom::SUB_STATES, sub_state);
        match sub_state {
            sub::INTACTE => intacte(x),
            sub::OUVERTE => {
                let n = x.u8(f::SUB_SUB_STATE);
                let target = jump(x, rom::OUVERTE, n);
                match n {
                    0 => opening(x),
                    2 => open_wait(x),
                    4 => clignote(x),
                    _ => crate::families::effects::outside_table(target),
                }
            }
            sub::PORTEE => {
                x.set8(f::REBOND, 0);
                suit_porteur(x);
            }
            sub::LANCEE => lancee(x),
            sub::LACHEE => {
                let n = x.u8(f::SUB_SUB_STATE);
                let target = jump(x, rom::LACHEE, n);
                match n {
                    0 => {
                        x.add8(f::SUB_SUB_STATE, 2);
                        x.set16(f::VZ, 0xff00);
                        x.set16(f::AZ, 0xffe0);
                    }
                    2 => lachee_chute(x),
                    4 => lachee_casse(x),
                    6 => clignote(x),
                    _ => crate::families::effects::outside_table(target),
                }
            }
            sub::TOUCHEE => {
                let n = x.u8(f::SUB_SUB_STATE);
                let target = jump(x, rom::TOUCHEE, n);
                match n {
                    0 => casse(x),
                    2 => clignote(x),
                    4 => tremble(x),
                    _ => crate::families::effects::outside_table(target),
                }
            }
            _ => crate::families::effects::outside_table(target),
        }
        x.reinsert();
    }

    /// Intacte : un coup (`COUP_B`) la fait passer à l'état « touchée » ; un coffre se met aussitôt à
    /// trembler.
    fn intacte<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::COUP_B) == 0 {
            return;
        }
        x.set32(f::STATE, 0x020a_0000);
        if x.u8(f::KIND) >= 3 {
            x.set8(f::SUB_SUB_STATE, 4);
            x.set8(f::ACTIVE, 2);
            x.set8(f::DELAY, 8);
            crate::sound::play(x.b, rom::SOUND_BROKEN, x.o);
        }
    }

    /// `0xda04e` : joue l'effet spécial de la caisse, une seule fois (`moment` dans D1).
    fn effect_special<R: Bench>(x: &mut Cx<'_, R>, moment: u32) {
        if x.u8(f::SPECIAL_PLAYED) != 0 {
            return;
        }
        x.set8(f::SPECIAL_PLAYED, 1);
        let i = 4 * x.u8(f::SPECIAL) as u32;
        let effect = x.rom32(rom::SPECIAUX + i);
        // créateurs ESHL désignés par la table d'origine
        use crate::families::eshl_creators as eshl;
        let r = &mut crate::families::effects_creators::leftovers::Pure;
        match effect {
            0xa1360 => eshl::single_18(r, x.b, x.o),
            0xa1a40 => {
                eshl::single_1b(r, x.b, x.o, moment as u16);
            }
            0xa1670 => {
                eshl::row_19(r, x.b, x.o);
            }
            // Glitch d'origine : le D5 que laisse ce qui précède (`Leftovers`) fixe l'octet haut du compte des éclats.
            0xa17d0 => {
                let d5 = *x.c.d5() as u16;
                eshl::row_1a(r, x.b, x.o, moment as u16, d5);
            }
            0xa1c6a => eshl::four_1c(r, x.b, x.o),
            _ => crate::families::effects::outside_table(effect),
        }
    }

    /// Crée l'ITEM `butin` (type et variante lus dans `BUTINS`) au-dessus de la caisse, lancé avec
    /// les vitesses `(vx, vy)`. `None` si le pool d'ITEM est plein.
    fn created_loot<R: Bench>(x: &mut Cx<'_, R>, butin: u8, vx: u16, vy: u16) -> Option<Obj> {
        let n = x.alloc(&pools::ITEM)?;
        let rank = 2 * butin as u32;
        n.set_u8(x.b, f::ACTIVE, 1);
        let kind = x.data8(rom::BUTINS + rank);
        n.set_u8(x.b, 0x2, kind);
        let variant = x.data8(rom::BUTINS + rank + 1);
        n.set_u8(x.b, f::VARIANT, variant);
        for off in [f::X, f::Z, f::Y] {
            let v = x.u16(off);
            n.set_u16(x.b, off, v);
        }
        let z = n.u16(x.b, f::Z).wrapping_add(0x18);
        n.set_u16(x.b, f::Z, z);
        let y = n.u16(x.b, f::Y).wrapping_sub(1);
        n.set_u16(x.b, f::Y, y);
        n.set_u16(x.b, f::VX, vx);
        n.set_u16(x.b, f::VZ, 0x300);
        n.set_u16(x.b, f::AZ, 0xffd8);
        n.set_u16(x.b, f::VY, vy);
        Some(n)
    }

    /// `0xda152` : la caisse brisée lâche son butin (une seule fois) : un objet au centre, un selon la
    /// classe du joueur qui a frappé (lancé à droite) et un tiré au hasard (lancé à gauche).
    fn lache_butin<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::BUTIN_LACHE) != 0 {
            return;
        }
        x.set8(f::BUTIN_LACHE, 1);
        let high = x.u8(f::LOOT_HIGH);
        if high != 0 && created_loot(x, high, 0, 0).is_none() {
            return;
        }
        let class = class(x) as u32;
        let right = x.u8(f::BUTIN_CLASSES + class);
        if right != 0 && created_loot(x, right, 0x140, 0x40).is_none() {
            return;
        }
        let left = x.u8(f::LOOT_LEFT);
        if left != 0 {
            created_loot(x, left, 0xfec0, 0xffc0);
        }
    }

    /// `0xda0e6` : le butin bonus, une seule fois.
    fn lache_bonus<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::BONUS_LACHE) != 0 {
            return;
        }
        x.set8(f::BONUS_LACHE, 1);
        let bonus = x.u8(f::BUTIN_BONUS);
        if bonus != 0 {
            created_loot(x, bonus, 0, 0);
        }
    }

    /// Clignotement final : compte à rebours `DELAY`, la caisse clignote ; à zéro, état suivant.
    fn clignote<R: Bench>(x: &mut Cx<'_, R>) {
        let remainder = x.u8(f::DELAY).wrapping_sub(1);
        x.set8(f::DELAY, remainder);
        if remainder == 0 {
            x.add8(f::STATE, 2);
        }
        x.set_bit(f::FLAGS, 1, remainder & 1 != 0);
    }

    /// Touchée, sous-sous-état 0 : la caisse joue son animation de casse et lâche son butin ; un coffre
    /// redevient intact.
    fn casse<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::KIND) >= 3 {
            x.set32(f::STATE, 0x0200_0000);
            x.set8(f::COUP_B, 0);
            x.set8(f::ACTIVE, 1);
            return;
        }
        if x.u8(f::COUP) != 0 {
            x.set8(f::COUP, 0);
            lache_butin(x);
            crate::sound::play(x.b, rom::SOUND_HIT, x.o);
            if x.u8(f::SPECIAL) != 0 {
                effect_special(x, 0);
            }
        }
        if x.anim_step() {
            x.add8(f::SUB_SUB_STATE, 2);
            x.set8(f::ACTIVE, 2);
            x.set8(f::DELAY, DURATION_BLINK);
            count_broken(x);
        }
    }

    /// Touchée, sous-sous-état 4 : le coffre tremble (±2 en X, ±1 en Y une image sur deux), puis
    /// reprend sa place.
    fn tremble<R: Bench>(x: &mut Cx<'_, R>) {
        let remainder = x.u8(f::DELAY).wrapping_sub(1);
        x.set8(f::DELAY, remainder);
        if remainder == 0 {
            x.set32(f::STATE, 0x0200_0000);
            x.set8(f::COUP_B, 0);
            x.set8(f::ACTIVE, 1);
            let ox = x.u16(f::ORIGINAL_X);
            x.set16(f::X, ox);
            let oy = x.u16(f::ORIGINAL_Y);
            x.set16(f::Y, oy);
            return;
        }
        x.add16(f::X, 2);
        x.add16(f::Y, 1);
        if remainder & 1 == 0 {
            x.add16(f::X, 0xfffc);
            x.add16(f::Y, 0xfffe);
        }
    }

    /// Ouverte, sous-sous-état 0 (`0xd9b9a`) : un coffre verrouillé consomme une clé (sauf pour la classe
    /// 5) ou refuse de s'ouvrir ; l'effet spécial est créé une fois ; puis animation d'ouverture et
    /// butin au premier coup.
    fn opening<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::LOCK) != 0 && class(x) != 5 {
            let cles = crate::mem::rd8(x.b, CLES);
            if cles == 0 {
                x.set32(f::STATE, 0x0200_0000);
                x.set8(f::ACTIVE, 1);
                etincelle_posee(x, 6);
                return;
            }
            crate::mem::wr8(x.b, CLES, cles - 1);
            x.set8(f::LOCK, 0);
        }
        let special = x.u8(f::SPECIAL);
        if special != 0 {
            if x.u8(f::EFFECT_CREATED) == 0 {
                if etincelle_posee(x, special).is_some() {
                    x.set8(f::EFFECT_CREATED, 1);
                }
            }
            if x.u8(f::FRAPPABLE) == 0 {
                return;
            }
        }
        if x.u8(f::COUP) != 0 {
            x.set8(f::COUP, 0);
            if x.u8(f::SPECIAL) != 0 {
                effect_special(x, 1);
            }
            lache_butin(x);
            crate::sound::play(x.b, rom::SOUND_HIT, x.o);
        }
        if !x.anim_step() {
            return;
        }
        x.add8(f::SUB_SUB_STATE, 2);
        x.set8(f::DELAY, 0xa);
        count_broken(x);
        if class(x) == 3 {
            x.set8(f::ACTIVE, 1);
            x.set8(f::SECOND_COUP, 1);
            x.set8(f::DELAY, 0xff);
            x.set8(f::COUP_B, 0);
        }
    }

    /// Ouverte, sous-sous-état 2 (`0xd9c5a`) : délai, puis clignotement ; ouverte par la classe 3, un
    /// nouveau coup lâche le bonus puis brise la caisse (ou fait clignoter le coffre).
    fn open_wait<R: Bench>(x: &mut Cx<'_, R>) {
        let remainder = x.u8(f::DELAY).wrapping_sub(1);
        x.set8(f::DELAY, remainder);
        if remainder == 0 {
            x.add8(f::SUB_SUB_STATE, 2);
            x.set8(f::ACTIVE, 2);
            x.set8(f::DELAY, DURATION_BLINK);
        }
        if x.u8(f::SECOND_COUP) == 0 || x.u8(f::COUP_B) == 0 {
            return;
        }
        if class(x) == 3 {
            lache_bonus(x);
        }
        if x.u8(f::KIND) < 3 {
            x.add8(f::STATE, 2);
            eclats(x);
        } else {
            x.set8(f::SUB_SUB_STATE, 4);
            x.set8(f::ACTIVE, 2);
            x.set8(f::DELAY, DURATION_BLINK);
        }
    }

    /// `moveq #2,D0 ; moveq #0,D1 ; jsr 0xab7d8` : gerbe d'éclats de la caisse.
    fn eclats<R: Bench>(x: &mut Cx<'_, R>) {
        effects_creators::debris(x.b, x.o, 2);
    }

    /// `0xb1352` : étincelle posée sur la caisse ; `None` si le pool est plein.
    fn etincelle_posee<R: Bench>(x: &mut Cx<'_, R>, variant: u8) -> Option<Obj> {
        effects_creators::posed_spark(x.b, x.o, variant)
    }

    /// `0xd9cea` : la caisse portée suit les mains du joueur (décalage par classe et par pose, retourné
    /// avec lui) ; rien si le joueur ne porte plus.
    fn suit_porteur<R: Bench>(x: &mut Cx<'_, R>) {
        let player = x.linked(f::PLAYER);
        let class = player.u8(x.b, j::CLASS) as u32;
        let prises = x.data32(rom::PRISES + 4 * class);
        let pose = player.u8(x.b, j::POSE);
        if pose == 0 {
            return;
        }
        let i = (4 * ((4 * class & 0xff00) | pose as u32)) & 0xffff;
        let mut dx = x.data16(prises + i);
        let dz = x.data16(prises + i + 2);
        let retourne = player.u8(x.b, j::RETOURNE);
        if retourne != 0 {
            dx = dx.wrapping_neg();
        }
        x.set8(f::RETOURNE, retourne);
        x.copy_pos(player);
        x.add16(f::X, dx);
        x.add16(f::Z, dz);
    }

    /// Lancée (`0xd9d38`) : attend `WAIT_FLIGHT`, puis vole (sous-sous-états) et se recale sur le décor.
    fn lancee<R: Bench>(x: &mut Cx<'_, R>) {
        let wait_kind = x.u8(f::WAIT_FLIGHT);
        if wait_kind != 0 {
            x.set8(f::WAIT_FLIGHT, wait_kind.wrapping_sub(1) & 7);
            return;
        }
        let n = x.u8(f::SUB_SUB_STATE);
        let target = jump(x, rom::LANCEE, n);
        match n {
            0 => launched_start(x),
            2 => launched_flight(x),
            4 => lancee_retombe(x),
            6 => clignote(x),
            _ => crate::families::effects::outside_table(target),
        }
        x.push_terrain();
    }

    /// Lancée, 0 : tant que le joueur la tient, elle suit ses mains ; lâchée, elle part en arc devant
    /// lui (et commence aussitôt son vol).
    fn launched_start<R: Bench>(x: &mut Cx<'_, R>) {
        let player = x.linked(f::PLAYER);
        if player.u8(x.b, j::POSE) != 0 {
            return suit_porteur(x);
        }
        x.add8(f::SUB_SUB_STATE, 2);
        x.set8(f::ACTIVE, 1);
        x.set8(f::IN_FLIGHT, 1);
        x.set16(f::VX, 0x380);
        x.set16(f::AZ, 0xffc8);
        x.set16(f::AX, 0);
        x.set16(f::VZ, 0);
        if player.u8(x.b, j::RETOURNE) == 0 {
            x.set16(f::VX, 0xfc80);
        }
        launched_flight(x);
    }

    /// Terrain sous la caisse (`0x24a8`, D2) : vrai si c'est un obstacle (attributs 5 à 8).
    fn obstacle<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        let t = crate::video::terrain_of(x.b, x.o);
        // Original glitch: 0x24a8 leaves its work registers as leftovers (D2-D4)
        crate::families::player::pshell::leftovers_terrain(x, &t);
        (t.number & 0x3f).wrapping_sub(5) <= 3
    }

    /// Avance en X et Z (`0x1da6`) ; rend Z.
    fn advance<R: Bench>(x: &mut Cx<'_, R>) -> i16 {
        x.move_xz();
        x.u16(f::Z) as i16
    }

    /// Lancée, 2 : une caisse se brise contre un obstacle ou en touchant le sol ; un coffre rebondit une
    /// fois sur l'obstacle, puis retombe.
    fn launched_flight<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::KIND) < 3 {
            if x.u8(f::WITHOUT_SCENERY) == 0 && !obstacle(x) && advance(x) >= 0 {
                return;
            }
            x.add8(f::STATE, 2);
            if x.u8(f::SECOND_COUP) != 0 && class(x) == 3 {
                lache_bonus(x);
            } else {
                lache_butin(x);
            }
            eclats(x);
            crate::sound::play(x.b, rom::SOUND_LANDS, x.o);
            return count_broken(x);
        }
        let rebondit = x.u8(f::REBOND) == 0 && (x.u8(f::WITHOUT_SCENERY) != 0 || obstacle(x));
        let pose = if rebondit {
            x.set8(f::REBOND, 1);
            x.set8(f::WITHOUT_SCENERY, 0);
            let vx = x.u16(f::VX).wrapping_neg();
            x.set16(f::VX, vx);
            let r = x.u8(f::RETOURNE) ^ 0x20;
            x.set8(f::RETOURNE, r);
            advance(x) <= 0
        } else {
            advance(x) < 0
        };
        if pose {
            crate::sound::play(x.b, rom::SOUND_SET_DOWN, x.o);
            x.add8(f::SUB_SUB_STATE, 2);
            x.set8(f::WITHOUT_SCENERY, 0);
            x.set16(f::Z, 0);
            x.set8(f::IN_FLIGHT, 0);
            x.set8(f::REBOND, 0);
            x.set16(f::VX, 0x200);
            x.set16(f::AX, 0);
            x.set16(f::VZ, 0x300);
            x.set16(f::AZ, 0xffc8);
            if x.u8(f::RETOURNE) == 0 {
                x.set16(f::VX, 0xfe00);
            }
        }
        stays_on_screen(x);
    }

    /// Lancée, 4 : le coffre retombe après son rebond ; posé, il redevient intact (ou, ouvert par la
    /// classe 3, attend son clignotement en lâchant le bonus).
    fn lancee_retombe<R: Bench>(x: &mut Cx<'_, R>) {
        if advance(x) >= 0 {
            return stays_on_screen(x);
        }
        if x.u8(f::SECOND_COUP) != 0 && class(x) == 3 {
            x.add8(f::SUB_SUB_STATE, 2);
            x.set8(f::ACTIVE, 2);
            x.set8(f::DELAY, DURATION_BLINK);
            return lache_bonus(x);
        }
        x.set32(f::STATE, 0x0200_0000);
        x.set16(f::Z, 0);
        x.set8(f::COUP_B, 0);
        stays_on_screen(x);
    }

    /// `0xd9f06` : la caisse reste entre les bords de l'écran (X de la caméra + 0x18 à + 0x168) ; sa
    /// position devient sa nouvelle origine.
    fn stays_on_screen<R: Bench>(x: &mut Cx<'_, R>) {
        let cam2 = crate::ram::CAMERA_ALT_ACTIVE.get(x.b) != 0;
        let cam_x = x.a5_u16(if cam2 { 0x556 } else { 0x506 });
        let left = cam_x.wrapping_add(0x18);
        let px = x.u16(f::X);
        if (left as i16) > px as i16 {
            x.set16(f::X, left);
        } else {
            let right = cam_x.wrapping_add(0x168);
            if right <= px {
                x.set16(f::X, right);
            }
        }
        let px = x.u16(f::X);
        x.set16(f::ORIGINAL_X, px);
        let py = x.u16(f::Y);
        x.set16(f::ORIGINAL_Y, py);
    }

    /// Lâchée, 2 : chute ; au sol, un coffre sans effet spécial redevient intact, sinon la caisse attend.
    fn lachee_chute<R: Bench>(x: &mut Cx<'_, R>) {
        x.move_z();
        if x.u16(f::Z) as i16 >= 0 {
            return;
        }
        x.add8(f::SUB_SUB_STATE, 2);
        x.set32(f::Z, 0);
        if (x.u8(f::KIND) as i8) < 3 || x.u8(f::SPECIAL) != 0 {
            return;
        }
        x.set32(f::STATE, 0x0200_0000);
        x.set8(f::ACTIVE, 1);
        x.set8(f::COUP_B, 0);
        let px = x.u16(f::X);
        x.set16(f::ORIGINAL_X, px);
        let py = x.u16(f::Y);
        x.set16(f::ORIGINAL_Y, py);
        crate::sound::play(x.b, rom::SOUND_SET_DOWN, x.o);
    }

    /// Lâchée, 4 : au premier coup, butin et effet spécial ; fin d'animation : clignotement.
    fn lachee_casse<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::COUP) != 0 {
            x.set8(f::COUP, 0);
            lache_butin(x);
            crate::sound::play(x.b, rom::SOUND_HIT, x.o);
            count_broken(x);
            if x.u8(f::SPECIAL) != 0 {
                effect_special(x, 2);
            }
        }
        if x.anim_step() {
            x.add8(f::SUB_SUB_STATE, 2);
            x.set8(f::ACTIVE, 2);
            x.set8(f::DELAY, DURATION_BLINK);
        }
    }

    handlers! {
        0xd99fa "box_caisse" #[without] run,
    }
}

// ---------------------------------------------------------------------------------------------
// Type 2 : décor inerte (0xda7ac)

/// Type 2 : élément de décor sans comportement (animation 5 du jeu des BOX) ; l'objet s'initialise,
/// puis se contente de se trier chaque image jusqu'à sa libération. Non exercé par les scénarios.
mod scenery {
    pub const ENTRY: u32 = 0xda7ac;
    pub const TABLE_STATES: u32 = 0xda7c2;
    pub const ANIMATION: u16 = 5;
}

fn box_2<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, scenery::TABLE_STATES, state as u32);
    match state {
        state::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, Obj::ACTIVE, 2);
            o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
            ctx.anim_start(rom::ANIMATIONS, scenery::ANIMATION);
            ctx.reinsert();
        }
        state::ACTIVE => ctx.reinsert(),
        state::LIBERE | state::OUTSIDE_SCREEN => ctx.free(&pools::BOX),
        _ => crate::families::effects::outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 1 : barrière à ouvrir (0xda578)

/// Type 1 : élément de décor interactif frappé par les joueurs (`+0x6e`, posé par leur code de coup) :
/// variante 0, grille qui crée un objet SET (`0x8b`, d'où sort une porte) puis attend sa fin ; variante
/// 1, mécanisme à deux temps dont l'animation fait changer d'état l'objet frappeur (`+0x6c`) ;
/// variante 3 comme la 0 à l'initialisation. L'objet disparaît quand la caméra l'a dépassé de 0x200.
/// Non exercé.
mod barriere {
    pub const ENTRY: u32 = 0xda578;
    pub const STATES: u32 = 0xda58e;
    pub const INITS: u32 = 0xda5a6;
    pub const ACTIFS: u32 = 0xda5f4;
    pub const SUB_STATES_0: u32 = 0xda624;
    pub const SUB_STATES_1: u32 = 0xda65e;
    pub const ANIMATIONS_0: u32 = 0x1bed06;
    pub const ANIMATIONS_1: u32 = 0x1712d4;
    pub const POSITIONS: u32 = 0xda72c;
    pub const SOUND: u32 = 0xeae86;
    pub const FRAPPEUR: u32 = 0x6c;
    pub const SET_CREATED: u32 = 0x7c;
}

/// `0xda6f4` : crée l'objet SET de la barrière et le rattache.
fn barrier_created_set<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let Some(new) = super::creators::scene_set(ctx.b, 0x8b) else { return };
    let frappeur = pointeur(ctx, o, barriere::FRAPPEUR);
    let mode = (o.u8(ctx.b, field::MODE) as u32) << 3;
    let class = ((frappeur.u8(ctx.b, 3) & 0xf) >> 1) as u32;
    let position = ctx.prog8(barriere::POSITIONS + ((mode + class) & 0xffff));
    new.set_u8(ctx.b, 0x42, position);
    let character = frappeur.u8(ctx.b, 2);
    new.set_u8(ctx.b, 0x43, character);
    o.set_u16(ctx.b, barriere::SET_CREATED, new.0 as u16);
}

fn barriere_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let target = super::effects::target(ctx, barriere::INITS, 2 * variant as u32);
    match variant {
        0 | 3 => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
            o.set_u32(ctx.b, 0x78, 0);
            ctx.anim_start(barriere::ANIMATIONS_0, 0);
        }
        1 => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
            super::eshl_creators::single_3c(&mut crate::families::effects_creators::leftovers::Pure, ctx.b, o);
            ctx.anim_start(barriere::ANIMATIONS_1, 4);
        }
        _ => crate::families::effects::outside_table(target),
    }
}

/// Variante 0, actif : sous-état 0 attend le coup, sous-état 2 attend la fin du SET ; puis disparaît
/// quand elle sort de l'écran.
fn barrier_active_0<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = super::effects::target(ctx, barriere::SUB_STATES_0, sub_state as u32);
    match sub_state {
        0 => {
            let coup = o.u8(ctx.b, field::COUP_B);
            if !(coup == 0) {
                ajoute_u8(ctx, o, field::SUB_STATE, 2);
                barrier_created_set(ctx, o);
            }
            ctx.reinsert();
        }
        2 => {
            let set = pointeur(ctx, o, barriere::SET_CREATED);
            let finished = set.u8(ctx.b, 0x22) != 0;
            if finished {
                o.set_u8(ctx.b, field::SUB_STATE, 0);
                o.set_u8(ctx.b, field::COUP_B, 0);
            }
        }
        _ => crate::families::effects::outside_table(target),
    }
    let (cam_x, _) = ctx.camera();
    let gap = cam_x.wrapping_sub(o.u16(ctx.b, Obj::X));
    if !((gap as i16) < 0x200) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
}

fn barrier_active_1<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = super::effects::target(ctx, barriere::SUB_STATES_1, sub_state as u32);
    match sub_state {
        0 | 4 => {
            let coup = o.u8(ctx.b, field::COUP_B);
            if coup == 0 {
                return ctx.reinsert();
            }
            ajoute_u8(ctx, o, field::SUB_STATE, 2);
            let frappeur = pointeur(ctx, o, barriere::FRAPPEUR);
            frappeur.set_u32(ctx.b, Obj::STATE, 0x0200_3000);
            let animation = if sub_state == 0 { 5 } else { 6 };
            ctx.anim_start(barriere::ANIMATIONS_1, animation);
        }
        2 | 6 => {
            let end = ctx.anim_step();
            if !end {
                return;
            }
            if sub_state == 2 {
                ajoute_u8(ctx, o, field::SUB_STATE, 2);
            } else {
                ajoute_u8(ctx, o, field::SUB_STATE, 0xfa);
            }
            o.set_u8(ctx.b, field::COUP_B, 0);
            crate::sound::play(ctx.b, barriere::SOUND, ctx.obj());
        }
        _ => crate::families::effects::outside_table(target),
    }
}

fn barrier_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, barriere::STATES, state as u32);
    match state {
        state::INIT => barriere_init(ctx, o),
        state::ACTIVE => {
            let variant = o.u8(ctx.b, Obj::VARIANT);
            let target = super::effects::target(ctx, barriere::ACTIFS, 2 * variant as u32);
            match variant {
                0 => barrier_active_0(ctx, o),
                1 => barrier_active_1(ctx, o),
                _ => crate::families::effects::outside_table(target),
            }
        }
        state::LIBERE | state::OUTSIDE_SCREEN => ctx.free(&pools::BOX),
        _ => crate::families::effects::outside_table(target),
    }
}

handler!(BOX_1, barriere::ENTRY, "box_barriere", #[without] barrier_handler);
handler!(BOX_2, scenery::ENTRY, "box_decor", #[without] box_2);

// ---------------------------------------------------------------------------------------------
// Type 3 : bloc poussable (0xda7f2)

/// Type 3 : bloc qu'un joueur pousse (action 0x2e, enregistré par le joueur en `+0x6c`). Variante 0 : glisse
/// en X entre deux bornes, accompagné d'un SET (type 7, variante 0xc) ; sorti des bornes, il s'arrête (sous-état
/// 2) et prévient son SET. Variante 1 : glisse en hauteur (`+0x10`) et se recale sur le décor à chaque pas.
/// Le joueur poussant est replacé contre le bloc (décalage par personnage et direction).
pub mod pousse {
use crate::families::cx::Bench;
use crate::objects::Current;
    use crate::families::cx::{handlers, Cx};
    use crate::objects::{pools, Obj};

    const ANIMATIONS: u32 = 0x1bebc2;
    /// Recalage de l'objet après la collision avec le décor (`terrain::push_68k`).
    /// Décalage du joueur contre le bloc, par personnage (mots, en clair) : selon la direction de poussée.
    const AGAINST_HIGH: u32 = 0xdab52;
    const AGAINST_LOW: u32 = 0xdab62;
    const AGAINST_LEFT_1: u32 = 0xdab72;
    const AGAINST_RIGHT_1: u32 = 0xdab82;
    const AGAINST_LEFT_0: u32 = 0xdab92;
    const AGAINST_RIGHT_0: u32 = 0xdaba2;

    mod f {
        /// Non nul : objet suspendu.
        pub const HIDDEN: u32 = 0x40;
        pub const SET: u32 = 0x5c;
        /// Bornes de la variante 0 (X minimal exclu, X maximal).
        pub const X_MIN: u32 = 0x60;
        pub const X_MAX: u32 = 0x62;
        pub const ARRETE: u32 = 0x64;
        pub const MOBILE: u32 = 0x65;
        pub const POUSSE: u32 = 0x66;
        /// Personnage du joueur poussant × 2.
        pub const CHARACTER: u32 = 0x68;
        /// Joueur poussant (mot, 0 = aucun).
        pub const PLAYER: u32 = 0x6c;
    }
    /// Champs du joueur poussant.
    mod j {
        /// États `+4..+6` (le long `+4` masqué) du joueur en train de pousser.
        pub const STATES: u32 = 0x4;
        pub const PUSHING: u32 = 0x0200_2e00;
        pub const CHARACTER: u32 = 0x3;
        pub const DIRECTION: u32 = 0x7;
        /// Bit 4 de `+0x6e` : pas de deux pixels ; `+0x33` non nul : un pixel de plus.
        pub const ELAN: u32 = 0x6e;
        pub const FORCE: u32 = 0x33;
    }
    /// Directions de poussée (`+7` du joueur).
    mod dir {
        pub const HIGH: u8 = 2;
        pub const LOW: u8 = 4;
        pub const LEFT: u8 = 6;
        pub const RIGHT: u8 = 8;
    }

    fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::HIDDEN) != 0 {
            return;
        }
        let state = x.u8(0x4);
        match state {
            0 => {
                let v = 2 * x.u8(0x3) as u32;
                if v == 0 { init_0(x) } else { init_1(x) }
                x.reinsert();
            }
            2 => {
                if x.a5_u8(0x3d) == 0 {
                    let v = 2 * x.u8(0x3) as u32;
                    if x.u8(0x5) == 0 {
                        if v == 0 { active_0(x) } else { active_1(x) }
                    }
                }
                x.reinsert();
            }
            _ => x.free(&pools::BOX),
        }
    }

    fn init_common<R: Bench>(x: &mut Cx<'_, R>, mobile: u8) {
        x.add8(0x4, 2);
        x.set8(0x0, 1);
        x.set8(0x2e, 0x35);
        x.set8(f::MOBILE, mobile);
    }

    fn init_0<R: Bench>(x: &mut Cx<'_, R>) {
        init_common(x, 1);
        x.set16(0x8, 0x234);
        x.set16(0x10, 0x290);
        x.set16(f::X_MAX, 0x290);
        x.set16(f::X_MIN, 0x1d8);
        x.anim_start(ANIMATIONS, 7);
        let Some(set) = x.alloc(&pools::SET) else { return };
        set.set_u8(x.b, 0x0, 1);
        set.set_u8(x.b, 0x2, 7);
        set.set_u8(x.b, 0x3, 0xc);
        set.set_u8(x.b, 0x2f, 1);
        x.set16(f::SET, set.0 as u16);
    }

    fn init_1<R: Bench>(x: &mut Cx<'_, R>) {
        init_common(x, 0);
        x.anim_start(ANIMATIONS, 6);
    }

    /// Le joueur poussant (A1), s'il est toujours en train de pousser ; sinon le bloc l'oublie.
    fn pousseur<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
        if x.u16(f::PLAYER) == 0 {
            return None;
        }
        let player = x.linked(f::PLAYER);
        let states = player.u32(x.b, j::STATES) & 0xffff_ff00;
        if states != j::PUSHING {
            x.set8(f::POUSSE, 0);
            x.set16(f::PLAYER, 0);
            return None;
        }
        let character = (player.u8(x.b, j::CHARACTER) & 0xfe) as u32;
        x.set16(f::CHARACTER, character as u16);
        Some(player)
    }

    /// Replace le joueur contre le bloc sur l'axe `axe` (+8 ou +0x10).
    fn realign_player<R: Bench>(x: &mut Cx<'_, R>, axe: u32, table: u32) {
        let player = x.linked(f::PLAYER);
        let v = x.u16(axe);
        player.set_u16(x.b, axe, v);
        let i = x.u16(f::CHARACTER) as i16 as i32 as u32;
        let d = x.data16(table.wrapping_add(i));
        player.set_u16(x.b, axe, v.wrapping_add(d));
    }

    /// Variante 0 : glissement en X entre les bornes.
    fn active_0<R: Bench>(x: &mut Cx<'_, R>) {
        let px = x.u16(0x8);
        if px > x.u16(f::X_MAX) || px <= x.u16(f::X_MIN) {
            x.add8(0x5, 2);
            x.set8(f::ARRETE, 1);
            let set = x.linked(f::SET);
            set.set_u8(x.b, 0x2f, 0);
            return;
        }
        let Some(player) = pousseur(x) else { return };
        let (signe, table) = match player.u8(x.b, j::DIRECTION) {
            dir::LEFT => (-1i16, AGAINST_LEFT_0),
            dir::RIGHT => (1, AGAINST_RIGHT_0),
            _ => return,
        };
        if player.u8(x.b, j::ELAN) & 0x10 != 0 {
            x.add16(0x8, (2 * signe) as u16);
        }
        if player.u8(x.b, j::FORCE) != 0 {
            x.add16(0x8, signe as u16);
        }
        realign_player(x, 0x8, table);
    }

    /// Un pas de la variante 1 : le bloc avance puis se recale sur le décor.
    fn step<R: Bench>(x: &mut Cx<'_, R>, axe: u32, d: i16) {
        x.add16(axe, d as u16);
        realign_scenery(x);
    }

    fn realign_scenery<R: Bench>(x: &mut Cx<'_, R>) {
        x.push_terrain();
        // 0x35c0: keeps the crate on screen like a player
        crate::player::walk::clamp_to_screen(x.b, crate::player::Player(x.o.0));
    }

    /// Variante 1 : glissement en hauteur (directions 2 et 4) ou en X (6 et 8).
    fn active_1<R: Bench>(x: &mut Cx<'_, R>) {
        let Some(player) = pousseur(x) else { return };
        let direction = player.u8(x.b, j::DIRECTION);
        let (axe, signe, table) = match direction {
            dir::HIGH => (0x10, 1i16, AGAINST_HIGH),
            dir::LOW => (0x10, -1, AGAINST_LOW),
            dir::LEFT => (0x8, -1, AGAINST_LEFT_1),
            dir::RIGHT => (0x8, 1, AGAINST_RIGHT_1),
            _ => return,
        };
        let horizontal = axe == 0x8;
        let step_long = if horizontal { 2 * signe } else { signe };
        if player.u8(x.b, j::ELAN) & 0x10 != 0 {
            step(x, axe, step_long);
        }
        // le joueur est relu après chaque appel
        let player = x.linked(f::PLAYER);
        if player.u8(x.b, j::FORCE) != 0 {
            if horizontal {
                x.add16(axe, signe as u16);
            } else {
                step(x, axe, signe);
            }
        }
        if horizontal {
            realign_scenery(x);
        }
        realign_player(x, axe, table);
    }

    handlers! {
        0xda7f2 "box_pousse" #[without] run,
    }
}
