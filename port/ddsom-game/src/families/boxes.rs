//! Famille BOX (classe 0xA) : caisses, tonneaux et coffres destructibles. Dispatch `0x39e50`, table
//! des types `0x39e76` (4 types). 21 emplacements de 0x80 octets. Voir `docs/familles.md`.

use crate::objects::pools;
use crate::objects::Obj;

use super::{ajoute_u8, gestionnaire, pointeur, Ctx};

/// Valeurs de l'octet d'état `+4`.
mod etat {
    pub const INIT: u8 = 0;
    pub const ACTIF: u8 = 2;
    pub const LIBERE: u8 = 4;
    pub const HORS_ECRAN: u8 = 6;
}

/// Champs communs des BOX.
mod champ {
    pub const SOUS_ETAT: u32 = 0x05;
    pub const MODE: u32 = 0x2f;
    pub const JEU_OMBRES: u32 = 0x2e;
    /// Coup reçu, posé par le code des joueurs.
    pub const COUP_B: u32 = 0x6e;
}

mod rom {
    pub const ANIMATIONS: u32 = 0x1bebc2;
}

const OMBRES: u8 = 0x35;

// ---------------------------------------------------------------------------------------------
// Type 0 : caisse, tonneau ou coffre (0xd99fa)

/// Type 0 : caisse, tonneau ou coffre destructible (variantes 0 à 3). Une fiche de la ROM donne ses
/// caractéristiques ; frappée, elle joue son animation de casse, lâche jusqu'à trois objets ITEM puis
/// clignote et disparaît. Un coffre (sorte >= 3) tremble quand on le frappe ; il s'ouvre (sous-état 2)
/// quand un joueur l'ouvre, en consommant une clé s'il est verrouillé. Un joueur peut aussi soulever la
/// caisse (sous-état 4 : elle suit ses mains), la lancer (6 : vol, rebond sur le bord de l'écran,
/// casse à l'atterrissage) ou la lâcher (8 : chute).
pub mod caisse {
    use crate::families::cx::{handlers, sub, Cx};
    use crate::mem::A5;
    use crate::objects::{pools, Obj};

    /// Sous-états `+5` de l'état actif.
    mod sous {
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
        pub const VARIANTE: u32 = 0x03;
        pub const ETAT: u32 = 0x04;
        pub const SOUS_ETAT: u32 = 0x05;
        pub const SOUS_SOUS_ETAT: u32 = 0x06;
        pub const X: u32 = 0x08;
        pub const Z: u32 = 0x0c;
        pub const Y: u32 = 0x10;
        pub const VX: u32 = 0x14;
        pub const AX: u32 = 0x16;
        pub const VZ: u32 = 0x18;
        pub const AZ: u32 = 0x1a;
        pub const VY: u32 = 0x1c;
        pub const RETOURNE: u32 = 0x2c;
        pub const JEU_OMBRES: u32 = 0x2e;
        /// Coup reçu, seconde marque posée par le code des joueurs.
        pub const COUP: u32 = 0x33;
        /// Non nul : en l'air (pas d'ombre au sol).
        pub const EN_VOL: u32 = 0x3a;
        pub const HIDDEN: u32 = 0x40;
        /// Délai avant que la caisse lancée ne reprenne son vol (`+0x53`, modulo 8).
        pub const ATTENTE_VOL: u32 = 0x53;
        pub const SELF: u32 = 0x5c;
        /// Variété (0 à 2 : se brise d'un coup ; 3 et plus : coffre qui résiste).
        pub const SORTE: u32 = 0x60;
        /// Non nul : coffre verrouillé (une clé `A5+0x45` l'ouvre, le personnage de classe 5 s'en passe).
        pub const VERROU: u32 = 0x61;
        /// Butins : au centre, par classe du joueur (`+0x63..`), bonus (`+0x69`), à gauche (`+0x6a`).
        pub const BUTIN_HAUT: u32 = 0x62;
        pub const BUTIN_CLASSES: u32 = 0x63;
        pub const BUTIN_BONUS: u32 = 0x69;
        pub const BUTIN_GAUCHE: u32 = 0x6a;
        /// Effet spécial éventuel (`+0x6b`), marque « effet joué » (`+0x7d`).
        pub const SPECIAL: u32 = 0x6b;
        /// Joueur qui a frappé, ouvert ou porte la caisse (pointeur RAM 16 bits).
        pub const JOUEUR: u32 = 0x6c;
        pub const COUP_B: u32 = 0x6e;
        pub const DELAI: u32 = 0x6f;
        /// Coffre ouvert par un joueur de classe 3 : un second coup lâche le butin bonus.
        pub const SECOND_COUP: u32 = 0x70;
        /// Non nul : la caisse lancée ignore le décor.
        pub const SANS_DECOR: u32 = 0x71;
        pub const EFFET_CREE: u32 = 0x76;
        /// Non nul : le coffre ouvert reste frappable.
        pub const FRAPPABLE: u32 = 0x77;
        pub const ORIGINE_X: u32 = 0x78;
        pub const ORIGINE_Y: u32 = 0x7a;
        pub const BUTIN_LACHE: u32 = 0x7c;
        pub const SPECIAL_JOUE: u32 = 0x7d;
        pub const BONUS_LACHE: u32 = 0x7e;
        /// Le coffre lancé a rebondi sur le décor.
        pub const REBOND: u32 = 0x7f;
    }

    /// Champs du joueur lié.
    mod j {
        pub const CLASSE: u32 = 0x03;
        pub const RETOURNE: u32 = 0x2c;
        /// Pose du joueur qui porte (0 : il ne porte plus).
        pub const POSE: u32 = 0x46;
    }

    /// Tables et sous-routines.
    mod rom {
        pub const ETATS: u32 = 0xd9a10;
        pub const SOUS_ETATS: u32 = 0xd9a6a;
        pub const TOUCHEE: u32 = 0xd9abc;
        pub const OUVERTE: u32 = 0xd9b94;
        pub const LANCEE: u32 = 0xd9d64;
        pub const LACHEE: u32 = 0xd9f82;
        pub const ANIMATIONS: u32 = 0x1bebc2;
        /// Par variété : adresse de sa fiche de 0x2a octets (caractéristiques, butins aléatoires).
        pub const FICHES: u32 = 0x1051ae;
        /// Par numéro de butin : (type, variante) de l'ITEM créé (lu relativement au PC).
        pub const BUTINS: u32 = 0xda28a;
        /// Effets spéciaux (adresses de routines), par valeur de `SPECIAL`.
        pub const SPECIAUX: u32 = 0xda06a;
        /// Par classe du porteur : table des décalages (X, Z) de la caisse portée, par pose.
        pub const PRISES: u32 = 0xda398;
        // sous-routines non réécrites, ou natives
        pub const TERRAIN: u32 = 0x24a8;
        pub const COLLISION_DECOR: u32 = 0x33fe4;
        pub const EFFET_INITIAL: u32 = 0xb6596;
        pub const EFFET_OUVERTURE: u32 = 0xb1352;
        pub const ECLATS: u32 = 0xab7d8;
        pub const SON_COUP: u32 = 0xeb59e;
        pub const SON_CASSE: u32 = 0xeb610;
        pub const SON_ATTERRIT: u32 = 0xeb52c;
        pub const SON_POSE: u32 = 0xeb766;
    }

    /// Caisses brisées dans la scène (octet, `A5-0x5b21`).
    const CASSEES: u32 = A5 - 0x5b21;
    /// Clés des joueurs (octet, `A5+0x45`).
    const CLES: u32 = A5 + 0x45;
    const OMBRES: u8 = 0x35;
    const DUREE_CLIGNOTEMENT: u8 = 0x1e;

    fn run(x: &mut Cx) {
        if x.u8(f::HIDDEN) != 0 {
            return;
        }
        let etat = x.u8(f::ETAT);
        let cible = saut(x, rom::ETATS, etat);
        match etat {
            0 => init(x),
            2 => actif(x),
            4 | 6 => x.free(&pools::BOX),
            _ => x.call(cible),
        }
    }

    /// `move.w (table,PC,D0.w),D1.w` : D0 = indice, D1.w = mot de la table (restes lus après le saut) ;
    /// renvoie la cible (pour les indices hors table, laissés au code d'origine).
    fn saut(x: &mut Cx, table: u32, i: u8) -> u32 {
        x.c.d[0] = i as u32;
        let w = x.rom16(table + i as u32);
        x.c.d[1] = x.c.d[1] & 0xffff_0000 | w as u32;
        table.wrapping_add(w as i16 as i32 as u32)
    }

    /// Classe du joueur lié (`+3 >> 1`), A1 = le joueur.
    fn classe(x: &mut Cx) -> u8 {
        let joueur = x.link_a1(f::JOUEUR);
        let c = joueur.u8(x.b, j::CLASSE) >> 1;
        x.c.d[0] = c as u32;
        c
    }

    fn compte_cassee(x: &mut Cx) {
        let n = crate::mem::rd8(x.b, CASSEES).wrapping_add(1);
        crate::mem::wr8(x.b, CASSEES, n);
    }

    /// `0xda082` : recopie la fiche de la variété (10 octets) et tire deux butins au hasard.
    fn charge_fiche(x: &mut Cx) {
        let variante = x.u8(f::VARIANTE) as u32;
        let fiche = x.data32(rom::FICHES + 4 * variante);
        let champs = [f::SORTE, f::VERROU, f::BUTIN_HAUT, f::BUTIN_BONUS, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68];
        for (i, off) in champs.into_iter().enumerate() {
            let v = x.data8(fiche + i as u32);
            x.set8(off, v);
        }
        let tirage = x.rand_and(0xf);
        let v = x.data8(fiche + 0xa + tirage);
        x.set8(f::BUTIN_GAUCHE, v);
        let tirage = x.rand_and(0xf);
        let v = x.data8(fiche + 0x1a + tirage);
        x.set8(f::SPECIAL, v);
        // pointeurs de lecture de la fiche laissés par l'original
        x.c.a[1] = fiche + 0xa;
        x.c.a[2] = fiche + 0xa;
        x.c.a[3] = fiche + 0x1a;
    }

    fn init(x: &mut Cx) {
        x.add8(f::ETAT, 2);
        x.set8(f::SECOND_COUP, 0);
        x.set8(f::COUP_B, 0);
        x.set8(f::JEU_OMBRES, OMBRES);
        let px = x.u16(f::X);
        x.set16(f::ORIGINE_X, px);
        let py = x.u16(f::Y);
        x.set16(f::ORIGINE_Y, py);
        x.set16(f::SELF, x.o.0 as u16);
        charge_fiche(x);
        if x.u8(f::SPECIAL) != 0 {
            x.call(rom::EFFET_INITIAL);
        }
        let sorte = x.u8(f::SORTE) as u32;
        x.anim(rom::ANIMATIONS, sorte);
    }

    fn actif(x: &mut Cx) {
        let sous_etat = x.u8(f::SOUS_ETAT);
        let cible = saut(x, rom::SOUS_ETATS, sous_etat);
        match sous_etat {
            sous::INTACTE => intacte(x),
            sous::OUVERTE => {
                let n = x.u8(f::SOUS_SOUS_ETAT);
                let cible = saut(x, rom::OUVERTE, n);
                match n {
                    0 => ouverture(x),
                    2 => ouverte_attente(x),
                    4 => clignote(x),
                    _ => x.call(cible),
                }
            }
            sous::PORTEE => {
                x.set8(f::REBOND, 0);
                suit_porteur(x);
            }
            sous::LANCEE => lancee(x),
            sous::LACHEE => {
                let n = x.u8(f::SOUS_SOUS_ETAT);
                let cible = saut(x, rom::LACHEE, n);
                match n {
                    0 => {
                        x.add8(f::SOUS_SOUS_ETAT, 2);
                        x.set16(f::VZ, 0xff00);
                        x.set16(f::AZ, 0xffe0);
                    }
                    2 => lachee_chute(x),
                    4 => lachee_casse(x),
                    6 => clignote(x),
                    _ => x.call(cible),
                }
            }
            sous::TOUCHEE => {
                let n = x.u8(f::SOUS_SOUS_ETAT);
                let cible = saut(x, rom::TOUCHEE, n);
                match n {
                    0 => casse(x),
                    2 => clignote(x),
                    4 => tremble(x),
                    _ => x.call(cible),
                }
            }
            _ => return x.call(cible),
        }
        x.reinsert();
    }

    /// Intacte : un coup (`COUP_B`) la fait passer à l'état « touchée » ; un coffre se met aussitôt à
    /// trembler.
    fn intacte(x: &mut Cx) {
        if x.u8(f::COUP_B) == 0 {
            return;
        }
        x.set32(f::ETAT, 0x020a_0000);
        if x.u8(f::SORTE) >= 3 {
            x.set8(f::SOUS_SOUS_ETAT, 4);
            x.set8(f::ACTIVE, 2);
            x.set8(f::DELAI, 8);
            x.call(rom::SON_CASSE);
        }
    }

    /// `0xda04e` : joue l'effet spécial de la caisse, une seule fois (`moment` dans D1).
    fn effet_special(x: &mut Cx, moment: u32) {
        x.c.d[1] = moment;
        if x.u8(f::SPECIAL_JOUE) != 0 {
            return;
        }
        x.set8(f::SPECIAL_JOUE, 1);
        let i = 4 * x.u8(f::SPECIAL) as u32;
        x.c.d[0] = i;
        let effet = x.rom32(rom::SPECIAUX + i);
        x.c.a[1] = effet;
        x.call(effet);
    }

    /// Crée l'ITEM `butin` (type et variante lus dans `BUTINS`) au-dessus de la caisse, lancé avec
    /// les vitesses `(vx, vy)`. `None` si le pool d'ITEM est plein.
    fn cree_butin(x: &mut Cx, butin: u8, vx: u16, vy: u16) -> Option<Obj> {
        x.c.d[0] = butin as u32;
        let n = x.alloc(&pools::ITEM)?;
        x.c.a[3] = rom::BUTINS;
        let rang = 2 * butin as u32;
        x.c.d[0] = rang;
        n.set_u8(x.b, f::ACTIVE, 1);
        let kind = x.data8(rom::BUTINS + rang);
        n.set_u8(x.b, 0x2, kind);
        let variante = x.data8(rom::BUTINS + rang + 1);
        n.set_u8(x.b, f::VARIANTE, variante);
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
    fn lache_butin(x: &mut Cx) {
        if x.u8(f::BUTIN_LACHE) != 0 {
            return;
        }
        x.set8(f::BUTIN_LACHE, 1);
        let haut = x.u8(f::BUTIN_HAUT);
        if haut != 0 && cree_butin(x, haut, 0, 0).is_none() {
            return;
        }
        let classe = classe(x) as u32;
        let droite = x.u8(f::BUTIN_CLASSES + classe);
        if droite != 0 && cree_butin(x, droite, 0x140, 0x40).is_none() {
            return;
        }
        let gauche = x.u8(f::BUTIN_GAUCHE);
        if gauche != 0 {
            cree_butin(x, gauche, 0xfec0, 0xffc0);
        }
    }

    /// `0xda0e6` : le butin bonus, une seule fois.
    fn lache_bonus(x: &mut Cx) {
        if x.u8(f::BONUS_LACHE) != 0 {
            return;
        }
        x.set8(f::BONUS_LACHE, 1);
        let bonus = x.u8(f::BUTIN_BONUS);
        if bonus != 0 {
            cree_butin(x, bonus, 0, 0);
        }
    }

    /// Clignotement final : compte à rebours `DELAI`, la caisse clignote ; à zéro, état suivant.
    fn clignote(x: &mut Cx) {
        let reste = x.u8(f::DELAI).wrapping_sub(1);
        x.set8(f::DELAI, reste);
        x.c.d[0] = x.c.d[0] & !0xff | (reste & 1) as u32;
        if reste == 0 {
            x.add8(f::ETAT, 2);
        }
        x.set_bit(f::FLAGS, 1, reste & 1 != 0);
    }

    /// Touchée, sous-sous-état 0 : la caisse joue son animation de casse et lâche son butin ; un coffre
    /// redevient intact.
    fn casse(x: &mut Cx) {
        if x.u8(f::SORTE) >= 3 {
            x.set32(f::ETAT, 0x0200_0000);
            x.set8(f::COUP_B, 0);
            x.set8(f::ACTIVE, 1);
            return;
        }
        if x.u8(f::COUP) != 0 {
            x.set8(f::COUP, 0);
            lache_butin(x);
            x.call(rom::SON_COUP);
            if x.u8(f::SPECIAL) != 0 {
                effet_special(x, 0);
            }
        }
        if x.anim_step() {
            x.add8(f::SOUS_SOUS_ETAT, 2);
            x.set8(f::ACTIVE, 2);
            x.set8(f::DELAI, DUREE_CLIGNOTEMENT);
            x.c.a[1] = CASSEES;
            compte_cassee(x);
        }
    }

    /// Touchée, sous-sous-état 4 : le coffre tremble (±2 en X, ±1 en Y une image sur deux), puis
    /// reprend sa place.
    fn tremble(x: &mut Cx) {
        let reste = x.u8(f::DELAI).wrapping_sub(1);
        x.set8(f::DELAI, reste);
        if reste == 0 {
            x.set32(f::ETAT, 0x0200_0000);
            x.set8(f::COUP_B, 0);
            x.set8(f::ACTIVE, 1);
            let ox = x.u16(f::ORIGINE_X);
            x.set16(f::X, ox);
            let oy = x.u16(f::ORIGINE_Y);
            x.set16(f::Y, oy);
            return;
        }
        x.add16(f::X, 2);
        x.add16(f::Y, 1);
        x.c.d[0] = x.c.d[0] & !0xff | (reste & 1) as u32;
        if reste & 1 == 0 {
            x.add16(f::X, 0xfffc);
            x.add16(f::Y, 0xfffe);
        }
    }

    /// Ouverte, sous-sous-état 0 (`0xd9b9a`) : un coffre verrouillé consomme une clé (sauf pour la classe
    /// 5) ou refuse de s'ouvrir ; l'effet spécial est créé une fois ; puis animation d'ouverture et
    /// butin au premier coup.
    fn ouverture(x: &mut Cx) {
        if x.u8(f::VERROU) != 0 && classe(x) != 5 {
            let cles = crate::mem::rd8(x.b, CLES);
            if cles == 0 {
                x.set32(f::ETAT, 0x0200_0000);
                x.set8(f::ACTIVE, 1);
                x.c.d[0] = 6;
                return x.call(rom::EFFET_OUVERTURE);
            }
            crate::mem::wr8(x.b, CLES, cles - 1);
            x.set8(f::VERROU, 0);
        }
        let special = x.u8(f::SPECIAL);
        x.c.d[0] = special as u32;
        if special != 0 {
            if x.u8(f::EFFET_CREE) == 0 {
                x.call(rom::EFFET_OUVERTURE);
                if !x.c.cond(5) {
                    x.set8(f::EFFET_CREE, 1);
                }
            }
            if x.u8(f::FRAPPABLE) == 0 {
                return;
            }
        }
        if x.u8(f::COUP) != 0 {
            x.set8(f::COUP, 0);
            if x.u8(f::SPECIAL) != 0 {
                effet_special(x, 1);
            }
            lache_butin(x);
            x.call(rom::SON_COUP);
        }
        if !x.anim_step() {
            return;
        }
        x.add8(f::SOUS_SOUS_ETAT, 2);
        x.set8(f::DELAI, 0xa);
        x.c.a[1] = CASSEES;
        compte_cassee(x);
        if classe(x) == 3 {
            x.set8(f::ACTIVE, 1);
            x.set8(f::SECOND_COUP, 1);
            x.set8(f::DELAI, 0xff);
            x.set8(f::COUP_B, 0);
        }
    }

    /// Ouverte, sous-sous-état 2 (`0xd9c5a`) : délai, puis clignotement ; ouverte par la classe 3, un
    /// nouveau coup lâche le bonus puis brise la caisse (ou fait clignoter le coffre).
    fn ouverte_attente(x: &mut Cx) {
        let reste = x.u8(f::DELAI).wrapping_sub(1);
        x.set8(f::DELAI, reste);
        if reste == 0 {
            x.add8(f::SOUS_SOUS_ETAT, 2);
            x.set8(f::ACTIVE, 2);
            x.set8(f::DELAI, DUREE_CLIGNOTEMENT);
        }
        if x.u8(f::SECOND_COUP) == 0 || x.u8(f::COUP_B) == 0 {
            return;
        }
        if classe(x) == 3 {
            lache_bonus(x);
        }
        if x.u8(f::SORTE) < 3 {
            x.add8(f::ETAT, 2);
            eclats(x);
        } else {
            x.set8(f::SOUS_SOUS_ETAT, 4);
            x.set8(f::ACTIVE, 2);
            x.set8(f::DELAI, DUREE_CLIGNOTEMENT);
        }
    }

    /// `moveq #2,D0 ; moveq #0,D1 ; jsr 0xab7d8` : gerbe d'éclats de la caisse.
    fn eclats(x: &mut Cx) {
        x.c.d[0] = 2;
        x.c.d[1] = 0;
        x.call(rom::ECLATS);
    }

    /// `0xd9cea` : la caisse portée suit les mains du joueur (décalage par classe et par pose, retourné
    /// avec lui) ; rien si le joueur ne porte plus.
    fn suit_porteur(x: &mut Cx) {
        let joueur = x.link_a1(f::JOUEUR);
        let classe = joueur.u8(x.b, j::CLASSE) as u32;
        let prises = x.data32(rom::PRISES + 4 * classe);
        x.c.a[4] = prises;
        let pose = joueur.u8(x.b, j::POSE);
        x.c.d[0] = (4 * classe & 0xff00) | pose as u32;
        if pose == 0 {
            return;
        }
        let i = (4 * x.c.d[0]) & 0xffff;
        x.c.d[0] = i;
        let mut dx = x.data16(prises + i);
        let dz = x.data16(prises + i + 2);
        x.c.d[2] = x.c.d[2] & 0xffff_0000 | dz as u32;
        let retourne = joueur.u8(x.b, j::RETOURNE);
        if retourne != 0 {
            dx = dx.wrapping_neg();
        }
        x.c.d[1] = x.c.d[1] & 0xffff_0000 | dx as u32;
        x.set8(f::RETOURNE, retourne);
        x.copy_pos(joueur);
        x.add16(f::X, dx);
        x.add16(f::Z, dz);
    }

    /// Lancée (`0xd9d38`) : attend `ATTENTE_VOL`, puis vole (sous-sous-états) et se recale sur le décor.
    fn lancee(x: &mut Cx) {
        let attente = x.u8(f::ATTENTE_VOL);
        if attente != 0 {
            x.set8(f::ATTENTE_VOL, attente.wrapping_sub(1) & 7);
            return;
        }
        let n = x.u8(f::SOUS_SOUS_ETAT);
        let cible = saut(x, rom::LANCEE, n);
        match n {
            0 => lancee_depart(x),
            2 => lancee_vol(x),
            4 => lancee_retombe(x),
            6 => clignote(x),
            _ => x.call(cible),
        }
        x.call(rom::COLLISION_DECOR);
    }

    /// Lancée, 0 : tant que le joueur la tient, elle suit ses mains ; lâchée, elle part en arc devant
    /// lui (et commence aussitôt son vol).
    fn lancee_depart(x: &mut Cx) {
        let joueur = x.link_a1(f::JOUEUR);
        if joueur.u8(x.b, j::POSE) != 0 {
            return suit_porteur(x);
        }
        x.add8(f::SOUS_SOUS_ETAT, 2);
        x.set8(f::ACTIVE, 1);
        x.set8(f::EN_VOL, 1);
        x.set16(f::VX, 0x380);
        x.set16(f::AZ, 0xffc8);
        x.set16(f::AX, 0);
        x.set16(f::VZ, 0);
        if joueur.u8(x.b, j::RETOURNE) == 0 {
            x.set16(f::VX, 0xfc80);
        }
        lancee_vol(x);
    }

    /// Terrain sous la caisse (`0x24a8`, D2) : vrai si c'est un obstacle (attributs 5 à 8).
    fn obstacle(x: &mut Cx) -> bool {
        x.call(rom::TERRAIN);
        let d0 = (x.c.d[2] as u16 & 0x3f).wrapping_sub(5);
        x.c.d[0] = x.c.d[0] & 0xffff_0000 | d0 as u32;
        d0 <= 3
    }

    /// Avance en X et Z (`0x1da6`) ; rend Z.
    fn avance(x: &mut Cx) -> i16 {
        x.call(sub::MOVE_XZ);
        x.u16(f::Z) as i16
    }

    /// Lancée, 2 : une caisse se brise contre un obstacle ou en touchant le sol ; un coffre rebondit une
    /// fois sur l'obstacle, puis retombe.
    fn lancee_vol(x: &mut Cx) {
        if x.u8(f::SORTE) < 3 {
            if x.u8(f::SANS_DECOR) == 0 && !obstacle(x) && avance(x) >= 0 {
                return;
            }
            x.add8(f::ETAT, 2);
            if x.u8(f::SECOND_COUP) != 0 && classe(x) == 3 {
                lache_bonus(x);
            } else {
                lache_butin(x);
            }
            eclats(x);
            x.call(rom::SON_ATTERRIT);
            x.c.a[1] = CASSEES;
            return compte_cassee(x);
        }
        let rebondit = x.u8(f::REBOND) == 0 && (x.u8(f::SANS_DECOR) != 0 || obstacle(x));
        let pose = if rebondit {
            x.set8(f::REBOND, 1);
            x.set8(f::SANS_DECOR, 0);
            let vx = x.u16(f::VX).wrapping_neg();
            x.set16(f::VX, vx);
            let r = x.u8(f::RETOURNE) ^ 0x20;
            x.set8(f::RETOURNE, r);
            avance(x) <= 0
        } else {
            avance(x) < 0
        };
        if pose {
            x.call(rom::SON_POSE);
            x.add8(f::SOUS_SOUS_ETAT, 2);
            x.set8(f::SANS_DECOR, 0);
            x.set16(f::Z, 0);
            x.set8(f::EN_VOL, 0);
            x.set8(f::REBOND, 0);
            x.set16(f::VX, 0x200);
            x.set16(f::AX, 0);
            x.set16(f::VZ, 0x300);
            x.set16(f::AZ, 0xffc8);
            if x.u8(f::RETOURNE) == 0 {
                x.set16(f::VX, 0xfe00);
            }
        }
        reste_a_l_ecran(x);
    }

    /// Lancée, 4 : le coffre retombe après son rebond ; posé, il redevient intact (ou, ouvert par la
    /// classe 3, attend son clignotement en lâchant le bonus).
    fn lancee_retombe(x: &mut Cx) {
        if avance(x) >= 0 {
            return reste_a_l_ecran(x);
        }
        if x.u8(f::SECOND_COUP) != 0 && classe(x) == 3 {
            x.add8(f::SOUS_SOUS_ETAT, 2);
            x.set8(f::ACTIVE, 2);
            x.set8(f::DELAI, DUREE_CLIGNOTEMENT);
            return lache_bonus(x);
        }
        x.set32(f::ETAT, 0x0200_0000);
        x.set16(f::Z, 0);
        x.set8(f::COUP_B, 0);
        reste_a_l_ecran(x);
    }

    /// `0xd9f06` : la caisse reste entre les bords de l'écran (X de la caméra + 0x18 à + 0x168) ; sa
    /// position devient sa nouvelle origine.
    fn reste_a_l_ecran(x: &mut Cx) {
        let cam2 = x.a5_u8(0x36) != 0;
        let cam_x = x.a5_u16(if cam2 { 0x556 } else { 0x506 });
        let gauche = cam_x.wrapping_add(0x18);
        let px = x.u16(f::X);
        if (gauche as i16) > px as i16 {
            x.set16(f::X, gauche);
            x.c.d[0] = x.c.d[0] & 0xffff_0000 | cam_x as u32;
        } else {
            let droite = cam_x.wrapping_add(0x168);
            x.c.d[0] = x.c.d[0] & 0xffff_0000 | droite as u32;
            if droite <= px {
                x.set16(f::X, droite);
            }
        }
        x.c.d[1] = x.c.d[1] & 0xffff_0000 | gauche as u32;
        let px = x.u16(f::X);
        x.set16(f::ORIGINE_X, px);
        let py = x.u16(f::Y);
        x.set16(f::ORIGINE_Y, py);
    }

    /// Lâchée, 2 : chute ; au sol, un coffre sans effet spécial redevient intact, sinon la caisse attend.
    fn lachee_chute(x: &mut Cx) {
        x.call(sub::MOVE_Z);
        if x.u16(f::Z) as i16 >= 0 {
            return;
        }
        x.add8(f::SOUS_SOUS_ETAT, 2);
        x.set32(f::Z, 0);
        if (x.u8(f::SORTE) as i8) < 3 || x.u8(f::SPECIAL) != 0 {
            return;
        }
        x.set32(f::ETAT, 0x0200_0000);
        x.set8(f::ACTIVE, 1);
        x.set8(f::COUP_B, 0);
        let px = x.u16(f::X);
        x.set16(f::ORIGINE_X, px);
        let py = x.u16(f::Y);
        x.set16(f::ORIGINE_Y, py);
        x.call(rom::SON_POSE);
    }

    /// Lâchée, 4 : au premier coup, butin et effet spécial ; fin d'animation : clignotement.
    fn lachee_casse(x: &mut Cx) {
        if x.u8(f::COUP) != 0 {
            x.set8(f::COUP, 0);
            lache_butin(x);
            x.call(rom::SON_COUP);
            x.c.a[1] = CASSEES;
            compte_cassee(x);
            if x.u8(f::SPECIAL) != 0 {
                effet_special(x, 2);
            }
        }
        if x.anim_step() {
            x.add8(f::SOUS_SOUS_ETAT, 2);
            x.set8(f::ACTIVE, 2);
            x.set8(f::DELAI, DUREE_CLIGNOTEMENT);
        }
    }

    handlers! {
        0xd99fa "box_caisse" run,
    }
}

// ---------------------------------------------------------------------------------------------
// Type 2 : décor inerte (0xda7ac)

/// Type 2 : élément de décor sans comportement (animation 5 du jeu des BOX) ; l'objet s'initialise,
/// puis se contente de se trier chaque image jusqu'à sa libération. Non exercé par les scénarios.
mod decor {
    pub const ENTREE: u32 = 0xda7ac;
    pub const TABLE_ETATS: u32 = 0xda7c2;
    pub const ANIMATION: u16 = 5;
}

fn box_2(ctx: &mut Ctx) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(decor::TABLE_ETATS, etat as u32);
    match etat {
        etat::INIT => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, Obj::ACTIVE, 2);
            o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
            ctx.anim_start(rom::ANIMATIONS, decor::ANIMATION);
            ctx.reinsert();
        }
        etat::ACTIF => ctx.reinsert(),
        etat::LIBERE | etat::HORS_ECRAN => ctx.free(&pools::BOX),
        _ => ctx.deleguer(cible),
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
    pub const ENTREE: u32 = 0xda578;
    pub const ETATS: u32 = 0xda58e;
    pub const INITS: u32 = 0xda5a6;
    pub const ACTIFS: u32 = 0xda5f4;
    pub const SOUS_ETATS_0: u32 = 0xda624;
    pub const SOUS_ETATS_1: u32 = 0xda65e;
    pub const ANIMATIONS_0: u32 = 0x1bed06;
    pub const ANIMATIONS_1: u32 = 0x1712d4;
    pub const POSITIONS: u32 = 0xda72c;
    pub const EFFET_INITIAL: u32 = 0xa5f38;
    pub const SON: u32 = 0xeae86;
    pub const CREER_SET: u32 = 0xc8312;
    pub const FRAPPEUR: u32 = 0x6c;
    pub const SET_CREE: u32 = 0x7c;
}

/// `0xda6f4` : crée l'objet SET de la barrière et le rattache.
fn barriere_cree_set(ctx: &mut Ctx, o: Obj) {
    ctx.c.d[0] = ctx.c.d[0] & 0xffff_ff00 | 0x8b;
    ctx.jsr(barriere::CREER_SET);
    let echec = ctx.c.cond(5);
    if echec {
        return;
    }
    let nouveau = Obj(ctx.c.a[1]);
    let frappeur = pointeur(ctx, o, barriere::FRAPPEUR);
    let mode = (o.u8(ctx.b, champ::MODE) as u32) << 3;
    let classe = ((frappeur.u8(ctx.b, 3) & 0xf) >> 1) as u32;
    let position = ctx.prog8(barriere::POSITIONS + ((mode + classe) & 0xffff));
    nouveau.set_u8(ctx.b, 0x42, position);
    let personnage = frappeur.u8(ctx.b, 2);
    nouveau.set_u8(ctx.b, 0x43, personnage);
    o.set_u16(ctx.b, barriere::SET_CREE, nouveau.0 as u16);
}

fn barriere_init(ctx: &mut Ctx, o: Obj) {
    let variante = o.u8(ctx.b, Obj::VARIANT);
    let cible = ctx.saut_table(barriere::INITS, 2 * variante as u32);
    match variante {
        0 | 3 => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
            o.set_u32(ctx.b, 0x78, 0);
            ctx.anim_start(barriere::ANIMATIONS_0, 0);
        }
        1 => {
            ajoute_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, champ::JEU_OMBRES, OMBRES);
            ctx.jsr(barriere::EFFET_INITIAL);
            ctx.anim_start(barriere::ANIMATIONS_1, 4);
        }
        _ => ctx.deleguer(cible),
    }
}

/// Variante 0, actif : sous-état 0 attend le coup, sous-état 2 attend la fin du SET ; puis disparaît
/// quand elle sort de l'écran.
fn barriere_actif_0(ctx: &mut Ctx, o: Obj) {
    let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
    let cible = ctx.saut_table(barriere::SOUS_ETATS_0, sous_etat as u32);
    match sous_etat {
        0 => {
            let coup = o.u8(ctx.b, champ::COUP_B);
            if !(coup == 0) {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
                barriere_cree_set(ctx, o);
            }
            ctx.reinsert();
        }
        2 => {
            let set = pointeur(ctx, o, barriere::SET_CREE);
            let fini = set.u8(ctx.b, 0x22) != 0;
            if fini {
                o.set_u8(ctx.b, champ::SOUS_ETAT, 0);
                o.set_u8(ctx.b, champ::COUP_B, 0);
            }
        }
        _ => ctx.deleguer(cible),
    }
    let (cam_x, _) = ctx.camera();
    let ecart = cam_x.wrapping_sub(o.u16(ctx.b, Obj::X));
    if !((ecart as i16) < 0x200) {
        ajoute_u8(ctx, o, Obj::STATE, 2);
    }
}

fn barriere_actif_1(ctx: &mut Ctx, o: Obj) {
    let sous_etat = o.u8(ctx.b, champ::SOUS_ETAT);
    let cible = ctx.saut_table(barriere::SOUS_ETATS_1, sous_etat as u32);
    match sous_etat {
        0 | 4 => {
            let coup = o.u8(ctx.b, champ::COUP_B);
            if coup == 0 {
                return ctx.reinsert();
            }
            ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
            let frappeur = pointeur(ctx, o, barriere::FRAPPEUR);
            frappeur.set_u32(ctx.b, Obj::STATE, 0x0200_3000);
            let animation = if sous_etat == 0 { 5 } else { 6 };
            ctx.anim_start(barriere::ANIMATIONS_1, animation);
        }
        2 | 6 => {
            let fin = ctx.anim_step();
            if !fin {
                return;
            }
            if sous_etat == 2 {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 2);
            } else {
                ajoute_u8(ctx, o, champ::SOUS_ETAT, 0xfa);
            }
            o.set_u8(ctx.b, champ::COUP_B, 0);
            ctx.jsr(barriere::SON);
        }
        _ => ctx.deleguer(cible),
    }
}

fn barriere_gestionnaire(ctx: &mut Ctx) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let etat = o.u8(ctx.b, Obj::STATE);
    let cible = ctx.saut_table(barriere::ETATS, etat as u32);
    match etat {
        etat::INIT => barriere_init(ctx, o),
        etat::ACTIF => {
            let variante = o.u8(ctx.b, Obj::VARIANT);
            let cible = ctx.saut_table(barriere::ACTIFS, 2 * variante as u32);
            match variante {
                0 => barriere_actif_0(ctx, o),
                1 => barriere_actif_1(ctx, o),
                _ => ctx.deleguer(cible),
            }
        }
        etat::LIBERE | etat::HORS_ECRAN => ctx.free(&pools::BOX),
        _ => ctx.deleguer(cible),
    }
}

gestionnaire!(BOX_1, barriere::ENTREE, "box_barriere", barriere_gestionnaire);
gestionnaire!(BOX_2, decor::ENTREE, "box_decor", box_2);

// ---------------------------------------------------------------------------------------------
// Type 3 : bloc poussable (0xda7f2)

/// Type 3 : bloc qu'un joueur pousse (action 0x2e, enregistré par le joueur en `+0x6c`). Variante 0 : glisse
/// en X entre deux bornes, accompagné d'un SET (type 7, variante 0xc) ; sorti des bornes, il s'arrête (sous-état
/// 2) et prévient son SET. Variante 1 : glisse en hauteur (`+0x10`) et se recale sur le décor à chaque pas.
/// Le joueur poussant est replacé contre le bloc (décalage par personnage et direction).
pub mod pousse {
    use crate::families::cx::{handlers, Cx};
    use crate::objects::{pools, Obj};

    const ETATS: u32 = 0xda808;
    const INITS: u32 = 0xda826;
    const ACTIFS: u32 = 0xda8d0;
    const ANIMATIONS: u32 = 0x1bebc2;
    /// Collision avec le décor (0x33fe4) puis recalage de l'objet (0x35c0).
    const TERRAIN: u32 = 0x33fe4;
    const RECALE: u32 = 0x35c0;
    /// Décalage du joueur contre le bloc, par personnage (mots, en clair) : selon la direction de poussée.
    const CONTRE_HAUT: u32 = 0xdab52;
    const CONTRE_BAS: u32 = 0xdab62;
    const CONTRE_GAUCHE_1: u32 = 0xdab72;
    const CONTRE_DROITE_1: u32 = 0xdab82;
    const CONTRE_GAUCHE_0: u32 = 0xdab92;
    const CONTRE_DROITE_0: u32 = 0xdaba2;

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
        pub const PERSONNAGE: u32 = 0x68;
        /// Joueur poussant (mot, 0 = aucun).
        pub const JOUEUR: u32 = 0x6c;
    }
    /// Champs du joueur poussant.
    mod j {
        /// États `+4..+6` (le long `+4` masqué) du joueur en train de pousser.
        pub const ETATS: u32 = 0x4;
        pub const EN_POUSSEE: u32 = 0x0200_2e00;
        pub const PERSONNAGE: u32 = 0x3;
        pub const DIRECTION: u32 = 0x7;
        /// Bit 4 de `+0x6e` : pas de deux pixels ; `+0x33` non nul : un pixel de plus.
        pub const ELAN: u32 = 0x6e;
        pub const FORCE: u32 = 0x33;
    }
    /// Directions de poussée (`+7` du joueur).
    mod dir {
        pub const HAUT: u8 = 2;
        pub const BAS: u8 = 4;
        pub const GAUCHE: u8 = 6;
        pub const DROITE: u8 = 8;
    }

    fn run(x: &mut Cx) {
        if x.u8(f::HIDDEN) != 0 {
            return;
        }
        let etat = x.u8(0x4);
        saut(x, ETATS, etat as u32);
        match etat {
            0 => {
                let v = 2 * x.u8(0x3) as u32;
                saut(x, INITS, v);
                if v == 0 { init_0(x) } else { init_1(x) }
                x.reinsert();
            }
            2 => {
                if x.a5_u8(0x3d) == 0 {
                    let v = 2 * x.u8(0x3) as u32;
                    saut(x, ACTIFS, v);
                    if x.u8(0x5) == 0 {
                        if v == 0 { actif_0(x) } else { actif_1(x) }
                    }
                }
                x.reinsert();
            }
            _ => x.free(&pools::BOX),
        }
    }

    /// `move.w (table,PC,D0.w),D1.w` : D0 = indice, D1.w = mot de la table (restes lus après le saut).
    fn saut(x: &mut Cx, table: u32, i: u32) {
        x.c.d[0] = i;
        let w = x.rom16(table + i);
        x.c.d[1] = x.c.d[1] & 0xffff_0000 | w as u32;
    }

    fn init_commun(x: &mut Cx, mobile: u8) {
        x.add8(0x4, 2);
        x.set8(0x0, 1);
        x.set8(0x2e, 0x35);
        x.set8(f::MOBILE, mobile);
    }

    fn init_0(x: &mut Cx) {
        init_commun(x, 1);
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

    fn init_1(x: &mut Cx) {
        init_commun(x, 0);
        x.anim_start(ANIMATIONS, 6);
    }

    /// Le joueur poussant (A1), s'il est toujours en train de pousser ; sinon le bloc l'oublie.
    fn pousseur(x: &mut Cx) -> Option<Obj> {
        if x.u16(f::JOUEUR) == 0 {
            return None;
        }
        let joueur = x.linked(f::JOUEUR);
        x.c.a[1] = x.u16(f::JOUEUR) as i16 as i32 as u32;
        let etats = joueur.u32(x.b, j::ETATS) & 0xffff_ff00;
        x.c.d[0] = etats;
        if etats != j::EN_POUSSEE {
            x.set8(f::POUSSE, 0);
            x.set16(f::JOUEUR, 0);
            return None;
        }
        let personnage = (joueur.u8(x.b, j::PERSONNAGE) & 0xfe) as u32;
        x.c.d[0] = personnage;
        x.set16(f::PERSONNAGE, personnage as u16);
        Some(joueur)
    }

    /// Replace le joueur contre le bloc sur l'axe `axe` (+8 ou +0x10).
    fn recale_joueur(x: &mut Cx, axe: u32, table: u32) {
        let joueur = x.linked(f::JOUEUR);
        x.c.a[1] = x.u16(f::JOUEUR) as i16 as i32 as u32;
        let v = x.u16(axe);
        joueur.set_u16(x.b, axe, v);
        let i = x.u16(f::PERSONNAGE) as i16 as i32 as u32;
        let d = x.data16(table.wrapping_add(i));
        x.c.a[2] = table;
        x.set_d0w(d);
        joueur.set_u16(x.b, axe, v.wrapping_add(d));
    }

    /// Variante 0 : glissement en X entre les bornes.
    fn actif_0(x: &mut Cx) {
        let px = x.u16(0x8);
        x.set_d0w(px);
        if px > x.u16(f::X_MAX) || px <= x.u16(f::X_MIN) {
            x.add8(0x5, 2);
            x.set8(f::ARRETE, 1);
            let set = x.linked(f::SET);
            x.c.a[1] = x.u16(f::SET) as i16 as i32 as u32;
            set.set_u8(x.b, 0x2f, 0);
            return;
        }
        let Some(joueur) = pousseur(x) else { return };
        let (signe, table) = match joueur.u8(x.b, j::DIRECTION) {
            dir::GAUCHE => (-1i16, CONTRE_GAUCHE_0),
            dir::DROITE => (1, CONTRE_DROITE_0),
            _ => return,
        };
        if joueur.u8(x.b, j::ELAN) & 0x10 != 0 {
            x.add16(0x8, (2 * signe) as u16);
        }
        if joueur.u8(x.b, j::FORCE) != 0 {
            x.add16(0x8, signe as u16);
        }
        recale_joueur(x, 0x8, table);
    }

    /// Un pas de la variante 1 : le bloc avance puis se recale sur le décor.
    fn pas(x: &mut Cx, axe: u32, d: i16) {
        x.add16(axe, d as u16);
        recale_decor(x);
    }

    fn recale_decor(x: &mut Cx) {
        x.call(TERRAIN);
        x.call(RECALE);
    }

    /// Variante 1 : glissement en hauteur (directions 2 et 4) ou en X (6 et 8).
    fn actif_1(x: &mut Cx) {
        let Some(joueur) = pousseur(x) else { return };
        let direction = joueur.u8(x.b, j::DIRECTION);
        let (axe, signe, table) = match direction {
            dir::HAUT => (0x10, 1i16, CONTRE_HAUT),
            dir::BAS => (0x10, -1, CONTRE_BAS),
            dir::GAUCHE => (0x8, -1, CONTRE_GAUCHE_1),
            dir::DROITE => (0x8, 1, CONTRE_DROITE_1),
            _ => return,
        };
        let horizontal = axe == 0x8;
        let pas_long = if horizontal { 2 * signe } else { signe };
        if joueur.u8(x.b, j::ELAN) & 0x10 != 0 {
            pas(x, axe, pas_long);
        }
        // le joueur est relu après chaque appel
        let joueur = x.linked(f::JOUEUR);
        if joueur.u8(x.b, j::FORCE) != 0 {
            if horizontal {
                x.add16(axe, signe as u16);
            } else {
                pas(x, axe, signe);
            }
        }
        if horizontal {
            recale_decor(x);
        }
        recale_joueur(x, axe, table);
    }

    handlers! {
        0xda7f2 "box_pousse" run,
    }
}
