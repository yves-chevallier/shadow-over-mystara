//! Items and spells usable by the special attack (button 4): the fighter, the cleric, the elf and
//! the magic-user share the same code for thrown items, lightning, the wall, spells cast
//! in front of the darkened screen (13..19) and meteors; only the animations and a few details
//! change (`Style`).
//!
//! The special attack starts when `checks::special_attack` sets `field::SPECIAL_KIND` (number of
//! the launch animation, taken from a code table according to the identifier of the chosen row). The
//! caster is a stepped automaton (`Player::STEP`); at the carried gesture (`field::EVENT`) it calls the
//! original routine that creates the shot or effect (see `rom`). A charge is consumed at the end
//! (`common::consume_charge`).
//!
//! The cast spells darken the screen, freeze the other players and pause the game
//! (`common::begin_spell`) until `common::end_spell`. If `field::SPELL_CANCELLED` is non-zero (a
//! protection cancels the spell), only a spark is played.

use super::common::{self, field, next_step};
use super::foes::{self, Boost, Damage, Live, Strike};
use super::super::spawn;
use super::super::actions::to_stand;
use super::super::{animation, Ctx, Player};

/// Original routines called by these automata: sounds, targets, screen flashes and palettes (the shot and
/// effect creators are in `spawn`).
pub mod rom {
    /// Sound of a used item.
    pub const SOUND_ITEM: crate::sound::Sound = crate::sound::ITEM;
    /// End of the auras after the red magic.
    pub const SPELL_RESET_AURA: u32 = 0x37882;
}

/// Character peculiarities.
#[derive(Clone, Copy)]
pub struct Style {
    /// Gesture of the cast spells (13..20).
    pub cast_anim: u16,
    /// Gesture of lightning (9) and of the wall (10).
    pub bolt_anim: u16,
    /// Kind of spark played when the spell is cancelled (row of the table `common::item_spark`).
    pub spark: u16,
    /// Record field that keeps the target of lightning and of the wall (word).
    pub target: u32,
    /// Value of the lightning countdown where the monsters are hurt.
    pub thunder_at: u16,
    /// Final pose of the cast spells, played after the countdown; without it (`None`) the spell ends with
    /// the countdown.
    pub finish_anim: Option<u16>,
    /// The effect of spell 13 reads D0.b, which the original dispatch leaves: the action step (elf,
    /// magic-user) or twice the attack number (fighter, cleric).
    pub d0_is_step: bool,
    /// Screen restoration mode at the end of the spell (the magic-user passes 1 instead of 2).
    pub undim_mode: u16,
}

impl Style {
    /// Values of the fighter and the cleric.
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

/// Thrown item: animation `anim`; at the carried gesture, the routine `launcher` creates the shot (Z set: nothing
/// was created); a charge is consumed.
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

/// Object address read from a word of the record (sign extension of `movea.w`).
fn target_of(g: &mut Ctx, p: Player, off: u32) -> u32 {
    p.u16(g, off) as i16 as i32 as u32 & 0xff_ffff
}

/// Spells 9..10 and 13..20, common to all characters. Returns false for another number.
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

/// Lightning (no. 9): target chosen at the gesture, bolt, then end of the spell 192 frames later.
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
            // Original glitch: the variant of the created effect is D0.b, which the original animation step leaves
            let step = p.u8(g, Player::STEP);
            let (_, d0) = common::step_with_d0(g, p, step);
            if p.u8(g, field::EVENT) != 0 {
                next_step(g, p);
                p.set_u8(g, field::EVENT, 0);
                match spawn::lightning_target(g, p, d0) {
                    Some(target) => p.set_u16(g, style.target, target.word()),
                    None => {
                        to_stand(g, p);
                    }
                }
            }
        }
        4 => {
            animation::step(g, p);
            let target = target_of(g, p, style.target);
            if crate::mem::rd16(g.b, target + 0x0c) == 0 {
                // the bolt is born at the target's position (A2)
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0xc0);
                spawn::lightning(g, p, target);
                { crate::palette::flash(g.b, crate::palette::Flash::Gray); };
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
                crate::palette::restore_flash(g.b);
                common::consume_charge(g, p);
                common::end_spell_mode(g, p, style.undim_mode);
            }
        }
        _ => {}
    }
}

/// 0x474c4: the lightning hurts the unprotected monsters in play (bit 5 of `+0x94`).
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

/// Wall (no. 10): without darkening; the spell ends when its target has disappeared.
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
                // Original glitch: the caller tests Z, that of the wall counter's `addq`, or that of the allocation
                // that fails (Z clear): pool full, it believes the target created and keeps the A1 left by the
                // allocation.
                let (target_found, target) = spawn::wall_target(g, p);
                if target_found {
                    p.set_u16(g, style.target, target.word());
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

/// Description of a spell cast on the darkened screen (no. 13..19).
pub struct Cast {
    /// Duration of the spell in frames (`field::TIMER`).
    pub frames: u16,
    /// Spell-specific initialization, before the orientation and the freezing of the other players.
    pub prepare: fn(&mut Ctx, Player),
    /// After the freezing of the other players, before the darkening.
    pub after_freeze: fn(&mut Ctx, Player),
    /// On each frame after the first step.
    pub every_frame: fn(&mut Ctx, Player),
    /// At the gesture (event) when the spell is not cancelled.
    /// Receives the D0.b left by the animation step (see `common::step_with_d0`).
    pub fire: fn(&mut Ctx, Player, u8),
    /// At the end of the countdown, before the final pose (characters that have one).
    pub on_end: fn(&mut Ctx, Player),
}

/// Cast spell: darkens the screen, plays the gesture; the event triggers `fire` (or the failure
/// spark if the spell is cancelled); the counter `field::TIMER` sets the duration.
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
    // the effect of spell 13 reads D0.b: the step, or twice the attack number, or the flags of
    // the animation
    let d0 = if style.d0_is_step { step } else { p.u8(g, field::SPECIAL_KIND).wrapping_mul(2) };
    let (_, d0) = common::step_with_d0(g, p, d0);
    (spell.every_frame)(g, p);
    if p.u8(g, field::EVENT) != 0 {
        p.set_u8(g, field::EVENT, 0);
        g.sound(rom::SOUND_ITEM, p);
        if p.u8(g, field::SPELL_CANCELLED) != 0 {
            common::item_spark(g, p, style.spark);
            p.set_u16(g, field::TIMER, 0xa);
        } else {
            (spell.fire)(g, p, d0);
        }
    }
    cast_timer(g, p, spell, style);
}

/// 0x3f4b6: the spell lasts `frames` frames. At the end, either the final pose begins (`finish_anim`), or
/// the spell ends (`finish_cast`).
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

/// Last step of the spells with a final pose: at the end of the gesture the spell ends.
pub fn cast_finish(g: &mut Ctx, p: Player, style: Style) {
    cov!();
    if animation::step(g, p) {
        finish_cast(g, p, style);
    }
}

/// A charge is consumed, the healing effect of the potions is applied and the screen is restored.
fn finish_cast(g: &mut Ctx, p: Player, style: Style) {
    cov!();
    common::consume_charge(g, p);
    if p.u8(g, field::SPECIAL_KIND) == 0x13 {
        common::apply_heal(g, p);
    }
    common::end_spell_mode(g, p, style.undim_mode);
}

fn no_effect(_: &mut Ctx, _: Player, _: u8) {}

const BASE: Cast = Cast {
    frames: 0x28,
    prepare: common::nothing,
    after_freeze: common::nothing,
    every_frame: common::nothing,
    fire: no_effect,
    on_end: common::nothing,
};

pub const CAST_13: Cast = Cast {
    // Original glitch: the side of the shower is D0.b left by the animation step
    fire: |g, p, d0| spawn::shower(g, p, d0),
    ..BASE
};

pub const CAST_14: Cast = Cast {
    frames: 0x3c,
    fire: |g, p, _| {
        spawn::fireball(g, p);
        crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::LIGHT);
    },
    ..BASE
};

pub const CAST_15: Cast = Cast {
    frames: 0x78,
    prepare: |g, p| p.set_u8(g, 0x1e9, 2),
    fire: |g, p, _| {
        spawn::ice_spell(g, p);
        spawn::ice_crown(g, p);
    },
    ..BASE
};

/// Red ring (no. 16): the gesture creates the ring and the aura.
pub fn ring_fire(g: &mut Ctx, p: Player) {
    cov!();
    spawn::ring(g, p, 1);
    crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::RED);
}

/// Red aura placed on the caster for `duration` frames; an accessory (`+0x167` = 2) strengthens it.
pub fn red_aura(g: &mut Ctx, p: Player, duration: u16) {
    cov!();
    p.set_u16(g, Player::AURA_TIMER, duration);
    p.set_u8(g, Player::AURA, 1);
    if p.u8(g, Player::FREE_ITEM) == 2 {
        p.set_u8(g, Player::AURA, 3);
        p.set_u8(g, Player::FREE_ITEM, 0xff);
    }
    p.set_u8(g, Player::AURA_B, 0);
    p.set_u8(g, Player::AURA_C, 0);
    crate::palette::character_palette(g.b, p.obj());
}

/// Fighter and cleric: the aura is placed at the gesture.
pub const CAST_16: Cast = Cast {
    fire: |g, p, _| {
        ring_fire(g, p);
        red_aura(g, p, 0x8000);
    },
    ..BASE
};

pub const CAST_17: Cast = Cast {
    fire: |g, p, _| {
        spawn::beam_cast(g, p, 1);
        crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::LIGHT);
    },
    ..BASE
};

pub const CAST_18: Cast = Cast {
    frames: 0x78,
    fire: |g, p, _| {
        spawn::storm(g, p, 0);
    },
    ..BASE
};

/// Healing potion (no. 19): nearby allies are healed as by the cleric's spell.
pub const CAST_19: Cast = Cast {
    frames: 0x78,
    prepare: |g, p| p.set_u8(g, 0x1ed, 0),
    after_freeze: common::heal_scan,
    every_frame: common::heal_flicker,
    fire: |g, p, _| {
        spawn::potion(g, p);
        crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::NORMAL);
    },
    ..BASE
};

/// Meteors (no. 20): the spell lasts 176 frames; 96 frames before the end, the monsters are hurt.
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
        g.sound(rom::SOUND_ITEM, p);
        if p.u8(g, field::SPELL_CANCELLED) != 0 {
            common::item_spark(g, p, style.spark);
            p.set_u16(g, field::TIMER, 0xa);
        } else {
            spawn::meteors(g, p);
            { crate::palette::flash(g.b, crate::palette::Flash::G); };
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
                    crate::palette::restore_flash(g.b);
                }
            }
            None => {
                common::consume_charge(g, p);
                if p.u8(g, field::SPELL_CANCELLED) == 0 {
                    crate::palette::restore_flash(g.b);
                }
                common::end_spell_mode(g, p, style.undim_mode);
            }
        }
    } else if left == 0x60 {
        meteor_damage(g, p);
    }
}

/// 0x4414c: the meteors hurt the unprotected monsters in play (bit 7 of `+0x94`); the elf's spell 0x1c
/// uses other tables.
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
