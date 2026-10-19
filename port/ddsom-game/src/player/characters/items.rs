//! Objets et magies utilisables par l'attaque spéciale (bouton 4) : le combattant, le clerc, l'elfe et
//! le magicien partagent le même code pour les objets lancés, la foudre, le mur, les magies lancées
//! devant l'écran assombri (13..19) et les météores ; seules les animations et quelques détails
//! changent (`Style`).
//!
//! L'attaque spéciale démarre quand `checks::special_attack` pose `field::SPECIAL_KIND` (numéro de
//! l'animation de lancement, tiré d'une table du code selon l'identifiant de la ligne choisie). Le
//! lanceur est un automate à pas (`Player::STEP`) ; au geste porté (`field::EVENT`) il appelle la
//! routine d'origine qui crée le tir ou l'effet (voir `rom`). Une charge est consommée à la fin
//! (`common::consume_charge`).
//!
//! Les magies lancées assombrissent l'écran, figent les autres joueurs et mettent le jeu en pause
//! (`common::begin_spell`) jusqu'à `common::end_spell`. Si `field::SPELL_CANCELLED` est non nul (une
//! protection annule le sort), seule une étincelle est jouée.

use super::common::{self, field, next_step};
use super::foes::{self, Boost, Damage, Live, Strike};
use super::super::spawn;
use super::super::actions::to_stand;
use super::super::{animation, Ctx, Player};

/// Routines d'origine appelées par ces automates : sons, cibles, éclairs d'écran et palettes (les créateurs de tirs et
/// d'effets sont dans `spawn`).
pub mod rom {
    /// Son d'un objet utilisé.
    pub const SOUND_ITEM: u32 = 0xe2430;
    /// Foudre : choix de la cible (A1 en retour, Z levé si aucune).
    pub const LIGHTNING_TARGET: u32 = 0x4f004;
    /// Mur : choix de la cible.
    pub const WALL_TARGET: u32 = 0x50324;
    /// Éclairs de l'écran (palettes) : foudre, fin, feu.
    pub const FLASH_LIGHTNING: u32 = 0x1ab56;
    pub const FLASH_END: u32 = 0x1abe0;
    pub const FLASH_FIRE: u32 = 0x1ab3c;
    /// Palettes du joueur : lumière, normale, rouge.
    pub const PALETTE_LIGHT: u32 = 0x3af8;
    pub const PALETTE_NORMAL: u32 = 0x3afe;
    pub const PALETTE_RED: u32 = 0x3b14;
    /// Fin des auras après la magie rouge.
    pub const SPELL_RESET_AURA: u32 = 0x37882;
}

/// Particularités du personnage.
#[derive(Clone, Copy)]
pub struct Style {
    /// Geste des magies lancées (13..20).
    pub cast_anim: u16,
    /// Geste de la foudre (9) et du mur (10).
    pub bolt_anim: u16,
    /// Genre de l'étincelle jouée quand le sort est annulé (ligne de la table `common::item_spark`).
    pub spark: u16,
    /// Champ de l'enregistrement qui garde la cible de la foudre et du mur (mot).
    pub target: u32,
    /// Valeur du décompte de la foudre où les monstres sont blessés.
    pub thunder_at: u16,
    /// Pose finale des magies lancées, jouée après le décompte ; sans elle (`None`) le sort s'achève avec
    /// le décompte.
    pub finish_anim: Option<u16>,
    /// L'effet de la magie 13 lit D0.b, que l'aiguillage d'origine laisse : le pas de l'action (elfe,
    /// magicien) ou deux fois le numéro d'attaque (combattant, clerc).
    pub d0_is_step: bool,
    /// Mode de rétablissement de l'écran à la fin du sort (le magicien passe 1 au lieu de 2).
    pub undim_mode: u16,
}

impl Style {
    /// Valeurs du combattant et du clerc.
    pub const CLASSIC: Style = Style {
        cast_anim: 0,
        bolt_anim: 0,
        spark: 0,
        target: field::SPELL_TARGET,
        thunder_at: 0x60,
        finish_anim: None,
        d0_is_step: false,
        undim_mode: 2,
    };
}

/// Objet lancé : l'animation `anim` ; au geste porté, la routine `launcher` crée le tir (Z levé : rien
/// n'a été créé) ; une charge est consommée.
pub fn throw_item(g: &mut Ctx, p: Player, anim: u16, launcher: spawn::Thrown) {
    cov!();
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        animation::start_plain(g, p, anim);
        return;
    }
    if animation::step(g, p) {
        to_stand(g, p);
        return;
    }
    if p.u8(g, field::EVENT) != 0 {
        p.set_u8(g, field::EVENT, 0);
        if spawn::thrown(g, p, launcher) {
            common::consume_charge(g, p);
        }
    }
}

/// Adresse d'objet lue dans un mot de l'enregistrement (extension de signe de `movea.w`).
fn target_of(g: &mut Ctx, p: Player, off: u32) -> u32 {
    p.u16(g, off) as i16 as i32 as u32 & 0xff_ffff
}

/// Les magies 9..10 et 13..20, communes à tous les personnages. Renvoie faux pour un autre numéro.
pub fn run_shared(g: &mut Ctx, p: Player, which: u8, style: Style) -> bool {
    cov!();
    match which {
        9 => lightning(g, p, style),
        10 => wall(g, p, style),
        13 => cast(g, p, &CAST_13, style),
        14 => cast(g, p, &CAST_14, style),
        15 => cast(g, p, &CAST_15, style),
        16 => cast(g, p, &CAST_16, style),
        17 => cast(g, p, &CAST_17, style),
        18 => cast(g, p, &CAST_18, style),
        19 => cast(g, p, &CAST_19, style),
        20 => meteor(g, p, style),
        _ => return false,
    }
    true
}

/// Foudre (n° 9) : cible choisie au geste, éclair, puis fin du sort 192 images plus tard.
pub fn lightning(g: &mut Ctx, p: Player, style: Style) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            common::begin_spell(g, p, None);
            common::freeze_others(g, p);
            common::dim_screen(g, p);
            animation::start_plain(g, p, style.bolt_anim);
        }
        2 => {
            // la variante de l'effet créé est D0.b, que le pas d'animation d'origine laisse
            let step = p.u8(g, Player::STEP);
            common::step_keeping_regs(g, p, step);
            if p.u8(g, field::EVENT) != 0 {
                next_step(g, p);
                p.set_u8(g, field::EVENT, 0);
                let target_found = !g.rom(rom::LIGHTNING_TARGET, p);
                if target_found {
                    p.set_u16(g, style.target, g.c.a[1] as u16);
                } else {
                    to_stand(g, p);
                }
            }
        }
        4 => {
            animation::step(g, p);
            let target = target_of(g, p, style.target);
            if crate::mem::rd16(g.b, target + 0x0c) == 0 {
                // l'éclair naît à la position de la cible (A2)
                g.c.a[2] = target;
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0xc0);
                spawn::lightning(g, p, target);
                g.rom(rom::FLASH_LIGHTNING, p);
                animation::start(g, p, 0);
            }
        }
        6 => {
            if p.u16(g, field::TIMER) == style.thunder_at {
                lightning_damage(g, p);
            }
            let left = p.u16(g, field::TIMER).wrapping_sub(1);
            p.set_u16(g, field::TIMER, left);
            if left == 0 {
                g.rom(rom::FLASH_END, p);
                common::consume_charge(g, p);
                common::end_spell_mode(g, p, style.undim_mode);
            }
        }
        _ => {}
    }
}

/// 0x474c4 : la foudre blesse les monstres en jeu non protégés (bit 5 de `+0x94`).
pub fn lightning_damage(g: &mut Ctx, p: Player) {
    cov!();
    let strike = Strike {
        damage: Damage::Table(0x10_44a4, 0x10_4486),
        live: Live::Bit0,
        needs_hp: true,
        grounded: false,
        immune_bit: 5,
        reaction: 0x60,
        random_facing: true,
        boosts: &[Boost::Boss(0xb), Boost::Held(0x20, 2), Boost::Held(0x21, 1), Boost::Charm(4)],
        undead_die: true,
        sound: None,
    };
    foes::strike_foes(g, p, &strike);
}

/// Mur (n° 10) : sans assombrissement ; le sort se termine quand sa cible a disparu.
pub fn wall(g: &mut Ctx, p: Player, style: Style) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            animation::start_plain(g, p, style.bolt_anim);
        }
        2 => {
            animation::step(g, p);
            if p.u8(g, field::EVENT) != 0 {
                next_step(g, p);
                p.set_u8(g, field::EVENT, 0);
                let target_found = !g.rom(rom::WALL_TARGET, p);
                if target_found {
                    p.set_u16(g, style.target, g.c.a[1] as u16);
                } else {
                    to_stand(g, p);
                }
            }
        }
        4 => {
            animation::step(g, p);
            let target = target_of(g, p, style.target);
            if crate::mem::rd16(g.b, target + 0x0c) == 0 {
                common::consume_charge(g, p);
                to_stand(g, p);
            }
        }
        _ => {}
    }
}

/// Description d'une magie lancée sur l'écran assombri (n° 13..19).
pub struct Cast {
    /// Durée du sort en images (`field::TIMER`).
    pub frames: u16,
    /// Initialisation propre au sort, avant l'orientation et le gel des autres joueurs.
    pub prepare: fn(&mut Ctx, Player),
    /// Après le gel des autres joueurs, avant l'assombrissement.
    pub after_freeze: fn(&mut Ctx, Player),
    /// À chaque image après le premier pas.
    pub every_frame: fn(&mut Ctx, Player),
    /// Au geste (événement) quand le sort n'est pas annulé.
    pub fire: fn(&mut Ctx, Player),
    /// À la fin du décompte, avant la pose finale (personnages qui en ont une).
    pub on_end: fn(&mut Ctx, Player),
}

/// Magie lancée : assombrit l'écran, joue le geste ; l'événement déclenche `fire` (ou l'étincelle
/// d'échec si le sort est annulé) ; le compteur `field::TIMER` règle la durée.
pub fn cast(g: &mut Ctx, p: Player, spell: &Cast, style: Style) {
    cov!();
    let step = p.u8(g, Player::STEP);
    if step == 0 {
        common::begin_spell(g, p, Some(spell.frames));
        (spell.prepare)(g, p);
        common::face_held_direction(g, p);
        common::freeze_others(g, p);
        (spell.after_freeze)(g, p);
        common::dim_screen(g, p);
        animation::start_plain(g, p, style.cast_anim);
        return;
    }
    if step == 4 && style.finish_anim.is_some() {
        return cast_finish(g, p, style);
    }
    // l'effet de la magie 13 lit D0.b : le pas, ou deux fois le numéro d'attaque, ou les drapeaux de
    // l'animation
    let d0 = if style.d0_is_step { step } else { p.u8(g, field::SPECIAL_KIND).wrapping_mul(2) };
    common::step_keeping_regs(g, p, d0);
    (spell.every_frame)(g, p);
    if p.u8(g, field::EVENT) != 0 {
        p.set_u8(g, field::EVENT, 0);
        g.rom(rom::SOUND_ITEM, p);
        if p.u8(g, field::SPELL_CANCELLED) != 0 {
            common::item_spark(g, p, style.spark);
            p.set_u16(g, field::TIMER, 0xa);
        } else {
            (spell.fire)(g, p);
        }
    }
    cast_timer(g, p, spell, style);
}

/// 0x3f4b6 : le sort dure `frames` images. À la fin, soit la pose finale commence (`finish_anim`), soit
/// le sort s'achève (`finish_cast`).
fn cast_timer(g: &mut Ctx, p: Player, spell: &Cast, style: Style) {
    cov!();
    let left = p.u16(g, field::TIMER).wrapping_sub(1);
    p.set_u16(g, field::TIMER, left);
    if left != 0 {
        return;
    }
    match style.finish_anim {
        Some(anim) => {
            next_step(g, p);
            (spell.on_end)(g, p);
            animation::start_plain(g, p, anim);
        }
        None => finish_cast(g, p, style),
    }
}

/// Dernier pas des magies à pose finale : à la fin du geste le sort s'achève.
pub fn cast_finish(g: &mut Ctx, p: Player, style: Style) {
    cov!();
    if animation::step(g, p) {
        finish_cast(g, p, style);
    }
}

/// Une charge est consommée, l'effet de soin des potions est appliqué et l'écran est rétabli.
fn finish_cast(g: &mut Ctx, p: Player, style: Style) {
    cov!();
    common::consume_charge(g, p);
    if p.u8(g, field::SPECIAL_KIND) == 0x13 {
        common::apply_heal(g, p);
    }
    common::end_spell_mode(g, p, style.undim_mode);
}

fn no_effect(_: &mut Ctx, _: Player) {}

const BASE: Cast = Cast {
    frames: 0x28,
    prepare: common::nothing,
    after_freeze: common::nothing,
    every_frame: common::nothing,
    fire: no_effect,
    on_end: common::nothing,
};

pub const CAST_13: Cast = Cast {
    fire: |g, p| {
        let side = g.c.d[0] as u8;
        spawn::shower(g, p, side);
    },
    ..BASE
};

pub const CAST_14: Cast = Cast {
    frames: 0x3c,
    fire: |g, p| {
        spawn::fireball(g, p);
        g.rom(rom::PALETTE_LIGHT, p);
    },
    ..BASE
};

pub const CAST_15: Cast = Cast {
    frames: 0x78,
    prepare: |g, p| p.set_u8(g, 0x1e9, 2),
    fire: |g, p| {
        spawn::ice_spell(g, p);
        spawn::ice_crown(g, p);
    },
    ..BASE
};

/// Anneau rouge (n° 16) : le geste crée l'anneau et l'aura.
pub fn ring_fire(g: &mut Ctx, p: Player) {
    cov!();
    spawn::ring(g, p, 1);
    g.rom(rom::PALETTE_RED, p);
}

/// Aura rouge posée sur le lanceur pour `duration` images ; un accessoire (`+0x167` = 2) la renforce.
pub fn red_aura(g: &mut Ctx, p: Player, duration: u16) {
    cov!();
    p.set_u16(g, Player::AURA_TIMER, duration);
    p.set_u8(g, Player::AURA, 1);
    if p.u8(g, 0x167) == 2 {
        p.set_u8(g, Player::AURA, 3);
        p.set_u8(g, 0x167, 0xff);
    }
    p.set_u8(g, Player::AURA_B, 0);
    p.set_u8(g, Player::AURA_C, 0);
    g.rom(rom::SPELL_RESET_AURA, p);
}

/// Combattant et clerc : l'aura est posée au geste.
pub const CAST_16: Cast = Cast {
    fire: |g, p| {
        ring_fire(g, p);
        red_aura(g, p, 0x8000);
    },
    ..BASE
};

pub const CAST_17: Cast = Cast {
    fire: |g, p| {
        spawn::beam_cast(g, p, 1);
        g.rom(rom::PALETTE_LIGHT, p);
    },
    ..BASE
};

pub const CAST_18: Cast = Cast {
    frames: 0x78,
    fire: |g, p| {
        spawn::storm(g, p, 0);
    },
    ..BASE
};

/// Potion de soin (n° 19) : les alliés proches sont soignés comme par le sort du clerc.
pub const CAST_19: Cast = Cast {
    frames: 0x78,
    prepare: |g, p| p.set_u8(g, 0x1ed, 0),
    after_freeze: common::heal_scan,
    every_frame: common::heal_flicker,
    fire: |g, p| {
        spawn::potion(g, p);
        g.rom(rom::PALETTE_NORMAL, p);
    },
    ..BASE
};

/// Météores (n° 20) : le sort dure 176 images ; 96 images avant la fin, les monstres sont blessés.
pub fn meteor(g: &mut Ctx, p: Player, style: Style) {
    cov!();
    let step = p.u8(g, Player::STEP);
    if step == 0 {
        common::begin_spell(g, p, Some(0xb0));
        common::face_held_direction(g, p);
        common::freeze_others(g, p);
        common::dim_screen(g, p);
        animation::start_plain(g, p, style.cast_anim);
        return;
    }
    if step == 4 && style.finish_anim.is_some() {
        return cast_finish(g, p, style);
    }
    animation::step(g, p);
    if p.u8(g, field::EVENT) != 0 {
        p.set_u8(g, field::EVENT, 0);
        g.rom(rom::SOUND_ITEM, p);
        if p.u8(g, field::SPELL_CANCELLED) != 0 {
            common::item_spark(g, p, style.spark);
            p.set_u16(g, field::TIMER, 0xa);
        } else {
            spawn::meteors(g, p);
            g.rom(rom::FLASH_FIRE, p);
        }
    }
    let left = p.u16(g, field::TIMER).wrapping_sub(1);
    p.set_u16(g, field::TIMER, left);
    if left == 0 {
        match style.finish_anim {
            Some(anim) => {
                next_step(g, p);
                animation::start_plain(g, p, anim);
                if p.u8(g, field::SPELL_CANCELLED) == 0 {
                    g.rom(rom::FLASH_END, p);
                }
            }
            None => {
                common::consume_charge(g, p);
                if p.u8(g, field::SPELL_CANCELLED) == 0 {
                    g.rom(rom::FLASH_END, p);
                }
                common::end_spell_mode(g, p, style.undim_mode);
            }
        }
    } else if left == 0x60 {
        meteor_damage(g, p);
    }
}

/// 0x4414c : les météores blessent les monstres en jeu non protégés (bit 7 de `+0x94`) ; le sort 0x1c de
/// l'elfe utilise d'autres tables.
pub fn meteor_damage(g: &mut Ctx, p: Player) {
    cov!();
    let (boss, enemy) = if p.u8(g, field::SPECIAL_KIND) == 0x1c { (0x10_4468, 0x10_444a) } else { (0x10_44e0, 0x10_44c2) };
    let strike = Strike {
        damage: Damage::Table(boss, enemy),
        live: Live::Any,
        needs_hp: true,
        grounded: false,
        immune_bit: 7,
        reaction: 0xa4,
        random_facing: true,
        boosts: &[Boost::Armor],
        undead_die: true,
        sound: None,
    };
    foes::strike_foes(g, p, &strike);
}
