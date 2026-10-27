//! Phase 28 (`0xfb292`) : la fin du jeu (générique).
//!
//! Après la dernière scène (séquence de sortie 35, `scene::exit`), la partie passe en phase 28 : le boss est
//! vaincu, les joueurs reçoivent leurs récompenses, des dialogues et des écrans fixes se succèdent, puis la
//! phase 20 (fin de partie) rend la main à la machine à modes. Les états (`A5+0x2d`, pas de 2) sont rangés
//! par ordre de passage ; l'état 0 *tombe* dans l'état 2 (le compteur avance de 4).
//!
//! Le bloc d'état est à `FF2B00` : `+0` minuterie (mot), `+2` lien vers l'objet de dialogue en cours
//! (mot), `+8` octet de signal lu et écrit par les objets (par le pointeur `+0x4c` de leur enregistrement).
//! Chaque état revient en `0xfb2a4` (la native de reprise rétablit A0).

use super::{Block, Flow, Sequence};
use crate::mem::{rd16, rd32, rd8, rts, set_low_word, wr16, wr32, wr8, A5};
use crate::scene::Ctx;
use crate::Native;
use crate::objects::{pools, Obj};

const BLOCK: u32 = 0xff_2b00;
mod f {
    pub const TIMER: u32 = 0;
    pub const LINK: u32 = 2;
    pub const SIGNAL: u32 = 8;
}

mod var {
    use crate::mem::A5;
    pub const STEP: u32 = A5 + 0x2d;
    pub const PHASE: u32 = A5 + 0x2c;
    pub const VIDEO: u32 = A5 + 0x14;
    pub const FADE_PAGES: u32 = A5 + 0x48e;
    pub const FADE_STEPS: u32 = A5 + 0x48f;
    pub const FADE_DELAY: u32 = A5 + 0x490;
    pub const SCROLL_1: u32 = A5 + 0x4b6;
    pub const SCROLL_2: u32 = A5 + 0x4ba;
    pub const CAMERA: u32 = A5 + 0x506;
    pub const CAMERA_Y: u32 = A5 + 0x50a;
    pub const CAMERA_MODE: u32 = A5 + 0x51e;
    pub const SCREEN_ID: u32 = A5 + 0x36;
    pub const CAMERA_2: u32 = A5 + 0x556;
    pub const CAMERA_2_Y: u32 = A5 + 0x55a;
    pub const CAMERA_2_COPY: u32 = A5 + 0x55e;
    pub const CAMERA_2_COPY_Y: u32 = A5 + 0x562;
    pub const CAMERA_2_MODE: u32 = A5 + 0x56e;
    pub const PLAYERS: u32 = A5 + 0x5de;
    pub const CPS_REG_4: u32 = 0x400004;
    pub const COUNTER: u32 = A5 - 0x5b8e;
}

mod ext {
    pub const POOLS: u32 = 0x2062;
    pub const POOLS_EFFCT: u32 = 0x2150;
    pub const POOLS_SOBJ: u32 = 0x21bc;
    pub const SOUND_C0C: u32 = 0xc0c;
    pub const RESET_PLAYERS: u32 = 0x1f184;
    pub const SPRITES_RESET: u32 = 0x1de7c;
    pub const SPRITES: u32 = 0x1de96;
    pub const CLEAR_SPRITES: u32 = 0xc58;
    pub const CLEAR_SCROLL_1: u32 = 0xca8;
    pub const CLEAR_SCROLL_2: u32 = 0xcc8;
    pub const CLEAR_SCROLL_3: u32 = 0xce8;
    pub const PALETTE_PAGE_B: u32 = 0x1a608;
    pub const PALETTE_PAGE_C: u32 = 0x1a3d4;
    pub const PALETTE_SET: u32 = 0x1a67e;
    pub const PALETTE_FADE: u32 = 0x1a8b2;
    pub const BRIGHTNESS_WHITE: u32 = 0x1686;
    pub const BRIGHTNESS_BLACK: u32 = 0x168c;
    pub const TILE_BLOCK: u32 = 0x26d4;
    pub const TILE_BLOCK_2: u32 = 0x2766;
    pub const SOBJ_VARIANT: u32 = 0xddeaa;
    pub const SET_VARIANT: u32 = 0xce00a;
    pub const MESSAGE: u32 = 0xc8312;
    pub const SCORE_OBJECT: u32 = 0xb1a7a;
    pub const FADE_TASK_IN: u32 = 0x14ba;
    pub const FADE_TASK_OUT: u32 = 0x14d0;
    pub const FADE_TASK_C: u32 = 0x14de;
    pub const FADE_TASK_D: u32 = 0x14c8;
    pub const FADE_TASK_STEP: u32 = 0x1ced4;
    pub const FADE_TASK_E: u32 = 0x1ccf8;
    pub const SET: u32 = 0x39e86;
    pub const EFFCT: u32 = 0x39cba;
    pub const SOBJ: u32 = 0x39f82;
    pub const CAMERAS: u32 = 0x33e20;
    pub const SOUND_E0A82: u32 = 0xe0a82;
    pub const SOUND_E0882: u32 = 0xe0882;
    pub const SOUND_E1528: u32 = 0xe1528;
    pub const SOUND_E16F0: u32 = 0xe16f0;
    pub const PLAYER_PREPARE: u32 = 0x3525c;
    pub const PLAYERS_UPDATE: u32 = 0x35292;
    pub const CAMERA_INIT: u32 = 0x2a52;
    pub const PALETTE_360: u32 = 0x1a360;
    pub const REWARD: u32 = 0xce668;
    pub const BOOTED_SET: u32 = 0x133f6;
}

mod at {
    pub const RETURN: u32 = 0xfb2a4;
}

const FADE_ALL: u8 = 0xf;

pub const PHASE_28: Native = Native {
    entry: 0xfb292,
    name: "ending_phase_28",
    run: |c, b| {
        let mut x = Ctx::new(c, b);
        x.c.a[5] = A5;
        x.c.a[0] = BLOCK;
        let step = rd8(x.b, var::STEP);
        x.c.d[0] = step as u32;
        x.push_return(Etape::Return);
        let flow = state(&mut x, step);
        if flow == Flow::Done {
            rts(x.c, x.b);
        }
    },
    touches_sound: false,
    outputs: 0,
};
crate::etapes! {
    /// Étapes (points de reprise) de la machine à états.
    pub enum Etape {
        Return = at::RETURN => |x| {
            x.c.a[0] = BLOCK;
            Flow::Done
        },
        S00Video = S00_VIDEO => |x| {
            wr16(x.b, var::CPS_REG_4, 0x1355);
            x.c.d[0] = 0;
            for v in [var::SCROLL_1, var::SCROLL_2, var::CAMERA, var::CAMERA_Y] {
                wr32(x.b, v, 0);
            }
            wr16(x.b, var::CAMERA_MODE, 4);
            wr8(x.b, A5 + 0x53c, 0);
            wr32(x.b, var::CAMERA_2, 0);
            wr32(x.b, var::CAMERA_2_Y, 0);
            wr16(x.b, var::CAMERA_2_MODE, 4);
            wr16(x.b, var::VIDEO, 0x12c0);
            let v = rd8(x.b, A5 + 0x13) & !1;
            wr8(x.b, A5 + 0x13, v);
            x.wait(4, Etape::S00Palette)
        },
        S00Palette = S00_PALETTE => |x| {
            x.call(ext::PALETTE_PAGE_B);
            x.call_d0(ext::PALETTE_SET, 89);
            x.call_d0(ext::PALETTE_FADE, 86);
            wr8(x.b, var::FADE_PAGES, 12);
            x.call(ext::BRIGHTNESS_WHITE);
            s02_clear(x)
        },
        S02Cleared = S02_CLEARED => |x| {
            x.call(ext::CLEAR_SCROLL_2);
            x.call(ext::CLEAR_SCROLL_3);
            Flow::Done
        },
        S04Fade = S04_FADE => |x| {
            x.call(ext::SOUND_E1528);
            fade_settings(x, 3, 4);
            x.sous_programme(crate::taches::fondu::montee, Etape::S04Timer)
        },
        S04Timer = S04_TIMER => |x| {
            wr16(x.b, BLOCK + f::TIMER, 60);
            Flow::Done
        },
        S06Retry = S06_RETRY => s06_object,
        S08Fade = S08_FADE => |x| {
            next_step(x);
            fade_settings(x, 3, 4);
            x.sous_programme(crate::taches::fondu::descente, Etape::S08Faded)
        },
        S08Faded = S08_FADED => |x| {
            let v = rd16(x.b, var::VIDEO) & !0xe;
            wr16(x.b, var::VIDEO, v);
            x.wait(4, Etape::S08Picture)
        },
        S08Picture = S08_PICTURE => |x| {
            x.call_d0(ext::PALETTE_FADE, 87);
            x.call_d0(ext::PALETTE_PAGE_C, 6);
            tile_blocks(x, 0x91b800, 350, 351, ext::TILE_BLOCK_2);
            set_low_word(&mut x.c.d[0], 548);
            x.c.a[1] = 0x917000;
            x.call(ext::TILE_BLOCK);
            set_low_word(&mut x.c.d[0], 549);
            x.call(ext::TILE_BLOCK);
            wr8(x.b, BLOCK + f::SIGNAL, 0);
            s08_object(x)
        },
        S08Retry = S08_RETRY => s08_object,
        S0aRetry = S0A_RETRY => |x| open_message(x, 89, Etape::S0aRetry),
        S0cObject = S0C_OBJECT => s0c_object,
        S0cRetry = S0C_RETRY => s0c_object,
        S0eLoop = S0E_LOOP => |x| {
            render(x);
            s0e_wait_fade(x)
        },
        S0eMusic = S0E_MUSIC => |x| {
            render(x);
            s0e_wait_music(x)
        },
        S0eObjects = S0E_OBJECTS => |x| {
            wr8(x.b, var::SCREEN_ID, 1);
            x.call_d0(ext::SOBJ_VARIANT, 11);
            x.call_d0(ext::SOBJ_VARIANT, 12);
            wr8(x.b, x.c.a[1] + 0x2f, 0);
            x.call_d0(ext::SOBJ_VARIANT, 12);
            wr8(x.b, x.c.a[1] + 0x2f, 1);
            x.call_d0(ext::SET_VARIANT, 14);
            wr8(x.b, BLOCK + f::SIGNAL, 0xff);
            render(x);
            wr8(x.b, BLOCK + f::SIGNAL, 1);
            wr16(x.b, BLOCK + f::TIMER, 180);
            fade_settings(x, 3, FADE_ALL);
            x.c.a[0] = ext::FADE_TASK_IN;
            set_low_word(&mut x.c.d[0], 0x10);
            crate::taches::creer_puis_revenir(x)
        },
        S10Retry = S10_RETRY => |x| {
            render(x);
            open_message(x, 90, Etape::S10Retry)
        },
        S14Retry = S14_RETRY => |x| {
            render(x);
            open_message(x, 91, Etape::S14Retry)
        },
        S18Retry = S18_RETRY => s18_object,
        S1aLoop = S1A_LOOP => |x| {
            render(x);
            s1a_wait_fade(x)
        },
        S1cScroll = S1C_SCROLL => |x| {
            if !timer_tick(x) {
                return s1c_scroll(x);
            }
            wr16(x.b, var::SCROLL_2, 0);
            x.wait(4, Etape::S1cPalette)
        },
        S1cPalette = S1C_PALETTE => |x| {
            x.call(ext::PALETTE_360);
            // sauvegarde les 1024 octets de palette en 0x910000, puis les remplit de blanc
            for i in 0..256u32 {
                let v = rd32(x.b, 0x91c000 + 4 * i);
                wr32(x.b, 0x910000 + 4 * i, v);
            }
            for i in 0..256u32 {
                wr32(x.b, 0x91c000 + 4 * i, 0xffff_ffff);
            }
            // registres laissés par la copie (`movem.l (A1)+,D1-D6/A3-A4`, dernier bloc) puis par le remplissage
            let last: Vec<u32> = (248..256u32).map(|i| rd32(x.b, 0x910000 + 4 * i)).collect();
            x.c.d[2..7].copy_from_slice(&last[1..6]);
            x.c.a[3] = last[6];
            x.c.a[4] = last[7];
            x.c.a[1] = 0x91c000 + 1024;
            x.c.a[2] = 0x910000 + 1024;
            x.c.d[0] = 0xffff;
            x.c.d[1] = 0xffff_ffff;
            wr16(x.b, var::FADE_DELAY, 1);
            wr8(x.b, var::FADE_PAGES, FADE_ALL);
            wr32(x.b, A5 + 0x492, 0x910000);
            wr32(x.b, A5 + 0x496, 0x25c0c0);
            wr32(x.b, A5 + 0x49a, 0x269120);
            wr32(x.b, A5 + 0x49e, 0x27e600);
            x.c.a[0] = ext::FADE_TASK_STEP;
            set_low_word(&mut x.c.d[0], 0x10);
            crate::kernel::creer_tache(x.b, x.c.d[0], x.c.a[0]);
            x.call(ext::SOUND_E16F0);
            wr8(x.b, var::FADE_STEPS, 1);
            x.c.a[0] = BLOCK;
            s1c_wait_fade(x)
        },
        S1cFadeLoop = S1C_FADE_LOOP => |x| {
            render_scroll(x);
            if rd8(x.b, var::FADE_STEPS) != 0 {
                return s1c_wait_fade(x);
            }
            wr16(x.b, BLOCK + f::TIMER, 30);
            Flow::Done
        },
        S1eLoop = S1E_LOOP => |x| {
            render_scroll(x);
            if rd8(x.b, var::FADE_STEPS) != 0 {
                return s1e_loop(x);
            }
            next_step(x);
            s1e_object(x)
        },
        S1eRetry = S1E_RETRY => s1e_object,
        S22Loop = S22_LOOP => |x| {
            if rd8(x.b, var::FADE_STEPS) != 0 {
                return s22_wait(x);
            }
            next_step(x);
            wr16(x.b, BLOCK + f::TIMER, 60);
            wr8(x.b, BLOCK + f::SIGNAL, 0);
            render_scroll(x);
            x.call_d0(ext::SET_VARIANT, 20);
            if !x.ok() {
                return Flow::Done;
            }
            let link = x.c.a[1] as u16;
            wr16(x.b, BLOCK + f::LINK, link);
            let n = rd8(x.b, var::COUNTER).wrapping_add(1);
            wr8(x.b, var::COUNTER, n);
            Flow::Done
        },
        S28Loop = S28_LOOP => |x| {
            x.c.a[0] = BLOCK;
            render_scroll(x);
            if rd8(x.b, var::FADE_STEPS) != 0 {
                return s28_loop(x);
            }
            Flow::Done
        },
        S2aPlanes = S2A_PLANES => |x| {
            render_scroll(x);
            let v = rd16(x.b, var::VIDEO) & !0xc;
            wr16(x.b, var::VIDEO, v);
            x.wait(1, Etape::S2aScrolls)
        },
        S2aScrolls = S2A_SCROLLS => |x| {
            x.call(ext::CLEAR_SCROLL_2);
            x.call(ext::CLEAR_SCROLL_3);
            wr8(x.b, var::CAMERA_MODE, 4);
            x.c.d[0] = 0;
            for v in [var::CAMERA, var::CAMERA_Y, var::CAMERA_2, var::CAMERA_2_Y] {
                wr16(x.b, v, 0);
            }
            next_step(x);
            wr16(x.b, BLOCK + f::TIMER, 4);
            x.call_d0(ext::PALETTE_FADE, 87);
            x.call_d0(ext::PALETTE_PAGE_C, 6);
            s2a_wait(x)
        },
        S2aWait = S2A_WAIT => |x| {
            if !timer_tick(x) {
                return s2a_wait(x);
            }
            wr16(x.b, BLOCK + f::TIMER, 600);
            tile_blocks(x, 0x91b800, 354, 355, ext::TILE_BLOCK_2);
            wr8(x.b, BLOCK + f::SIGNAL, 0);
            s2a_object(x)
        },
        S2aRetry = S2A_RETRY => s2a_object,
        S2aShow = S2A_SHOW => |x| {
            wr16(x.b, var::VIDEO, 0x12ce);
            render(x);
            fade_settings(x, 3, 13);
            x.c.a[0] = ext::FADE_TASK_IN;
            set_low_word(&mut x.c.d[0], 0x10);
            crate::taches::creer_puis_revenir(x)
        },
        S2cLoop = S2C_LOOP => |x| {
            if rd8(x.b, var::FADE_STEPS) != 0 {
                return s2c_loop(x);
            }
            wr8(x.b, BLOCK + f::SIGNAL, 0xff);
            for f in [ext::RESET_PLAYERS, ext::SPRITES_RESET, ext::POOLS_EFFCT, ext::POOLS_SOBJ, ext::CLEAR_SPRITES] {
                x.call(f);
            }
            x.wait(1, Etape::S2cPlanes)
        },
        S2cPlanes = S2C_PLANES => |x| {
            let v = rd16(x.b, var::VIDEO) & !0xc;
            wr16(x.b, var::VIDEO, v);
            x.wait(1, Etape::S2cCleared)
        },
        S2cCleared = S2C_CLEARED => |x| {
            x.call(ext::CLEAR_SCROLL_3);
            x.call(ext::CLEAR_SCROLL_1);
            wr16(x.b, BLOCK + f::TIMER, 4);
            let o = link(x);
            wr32(x.b, o + 4, 0x0600_0000);
            let n = rd8(x.b, var::COUNTER).wrapping_sub(1);
            wr8(x.b, var::COUNTER, n);
            wr8(x.b, var::SCREEN_ID, 0);
            wr8(x.b, A5 + 0x53c, 0);
            let v = rd8(x.b, A5 + 0x13) & !1;
            wr8(x.b, A5 + 0x13, v);
            for v in [var::CAMERA, var::CAMERA_Y, var::SCROLL_1, var::SCROLL_2] {
                wr16(x.b, v, 0);
            }
            s2c_wait_timer(x)
        },
        S2cTimer = S2C_TIMER => |x| {
            if !timer_tick(x) {
                return s2c_wait_timer(x);
            }
            x.call_d0(ext::PALETTE_SET, 84);
            x.call_d0(ext::PALETTE_FADE, 84);
            wr8(x.b, var::FADE_PAGES, 12);
            x.call(ext::BRIGHTNESS_BLACK);
            x.call_d0(ext::SOUND_E0A82, 4);
            // joueurs présents et joueurs « prêts à partir » (octet +0x192)
            let mut present = 0u8;
            let mut ready = 0u8;
            let count = rd8(x.b, A5 + 0x20) as u32 + 1;
            let mut fp = var::PLAYERS;
            for _ in 0..=count {
                if rd8(x.b, fp) != 0 && (rd8(x.b, fp + 4) as i8) < 4 {
                    present += 1;
                    if rd8(x.b, fp + 0x192) != 0 {
                        ready += 1;
                    }
                }
                fp += 0x200;
            }
            if ready != 0 || present.wrapping_sub(2) == rd8(x.b, A5 + 0x20) {
                x.call_d0(ext::SET_VARIANT, 22);
                if x.ok() {
                    let link = x.c.a[1] as u16;
                    wr16(x.b, BLOCK + f::LINK, link);
                    return render_flow(x);
                }
            }
            next_step(x);
            render_flow(x)
        },
        S2eOpened = S2E_OPENED => |x| {
            let o = link(x);
            let v = rd8(x.b, o + 5).wrapping_add(2);
            wr8(x.b, o + 5, v);
            render_flow(x)
        },
        S2eClosed = S2E_CLOSED => |x| {
            let o = link(x);
            let v = rd8(x.b, o + 5).wrapping_add(2);
            wr8(x.b, o + 5, v);
            render_flow(x)
        },
        S30Wait = S30_WAIT => |x| {
            if (rd16(x.b, A5 + 0x7e68) as i16) < 3 {
                return Flow::Done;
            }
            next_step(x);
            x.c.d[0] = 0;
            x.c.d[4] = 13;
            for counter in (0..3u32).rev() {
                x.c.d[1] = counter;
                wr8(x.b, BLOCK + f::SIGNAL, 0);
                x.call(ext::SOBJ_VARIANT);
                let o = x.c.a[1];
                wr32(x.b, o + 0x4c, BLOCK);
                let v = rd16(x.b, var::CAMERA);
                wr16(x.b, o + 8, v);
                let v = rd16(x.b, var::CAMERA_Y);
                wr16(x.b, o + 0xc, v);
                wr32(x.b, o + 0x10, 0);
                let d4 = x.c.d[4] as u8;
                wr8(x.b, o + 0x2f, d4);
                x.c.d[4] = (x.c.d[4] & !0xff) | d4.wrapping_add(1) as u32;
                let idx = counter * 4;
                let dx = x.prog16(0xfc3e4 + idx);
                let dy = x.prog16(0xfc3e6 + idx);
                let ox = rd16(x.b, o + 8).wrapping_add(dx);
                wr16(x.b, o + 8, ox);
                let oy = rd16(x.b, o + 0xc).wrapping_add(dy);
                wr16(x.b, o + 0xc, oy);
            }
            render(x);
            wr16(x.b, var::VIDEO, 0x12ce);
            wr16(x.b, BLOCK + f::TIMER, 240);
            fade_settings(x, 3, FADE_ALL);
            x.c.a[0] = ext::FADE_TASK_IN;
            set_low_word(&mut x.c.d[0], 0x10);
            crate::taches::creer_puis_revenir(x)
        },
        S32Faded = S32_FADED => |x| {
            for f in [ext::RESET_PLAYERS, ext::SPRITES_RESET, ext::POOLS, ext::CLEAR_SPRITES] {
                x.call(f);
            }
            x.wait(1, Etape::S32Cleared)
        },
        S32Cleared = S32_CLEARED => |x| {
            for f in [ext::CLEAR_SCROLL_1, ext::CLEAR_SCROLL_2, ext::CLEAR_SCROLL_3] {
                x.call(f);
            }
            // le score (long BCD à +0x110) reçoit la récompense (+0x114, sans son quartet faible), plafonné à 9 999 999
            for k in 0..4u32 {
                let p = var::PLAYERS + 0x200 * k;
                if rd8(x.b, p) == 0 {
                    continue;
                }
                wr32(x.b, p + 4, 0x0600_0000);
                let reward = rd32(x.b, p + 0x114) & 0xffff_fff0;
                wr32(x.b, p + 0x114, reward);
                let score = bcd_add(rd32(x.b, p + 0x110), reward);
                wr32(x.b, p + 0x110, if score as i32 > 0x0999_9999 { 0x0999_9999 } else { score });
            }
            x.call(ext::PLAYERS_UPDATE);
            wr8(x.b, var::PHASE, 20);
            x.call(ext::SOUND_E0882);
            x.jump_to(ext::BOOTED_SET)
        },
    }
}

fn state(x: &mut Ctx, step: u8) -> Flow {
    match step {
        0 => s00_reset(x),
        2 => s02_clear(x),
        4 => s04_picture(x),
        6 => s06_wait(x),
        8 => s08_wait_dialogue(x),
        10 => s0a_wait_signal(x),
        12 => s0c_wait_dialogue(x),
        14 => s0e_wait_timer(x),
        16 => s10_wait_camera(x),
        18 | 22 => s12_wait_dialogue(x),
        20 => s14_wait_timer(x),
        24 => s18_object(x),
        26 => s1a_wait_signal(x),
        28 => s1c_wait(x),
        30 => s1e_fade(x),
        32 => render_scroll(x),
        34 => s22_object(x),
        36 => render_scroll(x),
        38 => s26_wait(x),
        40 => s28_fade(x),
        42 => s2a_clear(x),
        44 => s2c_wait(x),
        46 => s2e_dialogue(x),
        48 => s30_rewards(x),
        50 => s32_end(x),
        _ => Flow::Done,
    }
}

// ---------------------------------------------------------------------------------------------
// Aides

fn next_step(x: &mut Ctx) {
    let s = rd8(x.b, var::STEP).wrapping_add(2);
    wr8(x.b, var::STEP, s);
}

/// `0xfc4ea` : décor (SET, SOBJ) et sprites ; A0 est conservé.
fn render(x: &mut Ctx) {
    let a0 = x.c.a[0];
    for f in [ext::SET, ext::SOBJ, ext::SPRITES] {
        x.call(f);
    }
    x.c.a[0] = a0;
}

/// `0xfc506` : comme `render`, avec les effets et les caméras.
fn render_scroll(x: &mut Ctx) -> Flow {
    let a0 = x.c.a[0];
    for f in [ext::SET, ext::EFFCT, ext::CAMERAS, ext::SOBJ, ext::SPRITES] {
        x.call(f);
    }
    x.c.a[0] = a0;
    Flow::Done
}

fn timer_tick(x: &mut Ctx) -> bool {
    let left = rd16(x.b, BLOCK + f::TIMER).wrapping_sub(1);
    wr16(x.b, BLOCK + f::TIMER, left);
    left == 0
}

fn link(x: &mut Ctx) -> u32 {
    rd16(x.b, BLOCK + f::LINK) as i16 as i32 as u32 & 0xff_ffff
}

fn fade_settings(x: &mut Ctx, delay: u16, pages: u8) {
    wr16(x.b, var::FADE_DELAY, delay);
    wr8(x.b, var::FADE_PAGES, pages);
}

/// Crée la tâche d'entrée `entry` (emplacement 0x10), A0 restauré (`movem.l A0`).
fn task_keep_a0(x: &mut Ctx, entry: u32) {
    let a0 = x.c.a[0];
    x.c.a[0] = entry;
    set_low_word(&mut x.c.d[0], 0x10);
    crate::kernel::creer_tache(x.b, x.c.d[0], x.c.a[0]);
    x.c.a[0] = a0;
}
/// Idem sans restaurer A0 (l'original le laisse sur l'entrée de la tâche).
fn task_clobber_a0(x: &mut Ctx, entry: u32) {
    x.c.a[0] = entry;
    set_low_word(&mut x.c.d[0], 0x10);
    crate::kernel::creer_tache(x.b, x.c.d[0], x.c.a[0]);
}

fn tile_blocks(x: &mut Ctx, a1: u32, first: u16, second: u16, which: u32) {
    x.c.a[1] = a1;
    set_low_word(&mut x.c.d[0], first);
    x.call(which);
    set_low_word(&mut x.c.d[0], second);
    x.call(which);
}

// ---------------------------------------------------------------------------------------------
// États

/// État 0 : vide tout, copie les récompenses des joueurs, puis tombe dans l'état 2.
fn s00_reset(x: &mut Ctx) -> Flow {
    next_step(x);
    set_low_word(&mut x.c.d[0], 0x30);
    crate::kernel::tuer_tache(x.b, x.c.d[0]);
    for f in [ext::POOLS, ext::SOUND_C0C, ext::RESET_PLAYERS, ext::SPRITES_RESET, ext::CLEAR_SPRITES] {
        x.call(f);
    }
    for k in 0..4u32 {
        let p = var::PLAYERS + 0x200 * k;
        if rd8(x.b, p) != 0
            && (rd8(x.b, p + 4) as i8) < 4
            && rd32(x.b, p + 0x118) == 0xffff_ffff
            && rd32(x.b, p + 0x11c) == 0xffff_ffff
        {
            reward_table(x, p);
        }
    }
    x.c.a[0] = var::PLAYERS + 0x800;
    x.c.d[6] = 0xffff;
    x.wait(1, Etape::S00Video)
}

/// `0xfb798` : deux longs de la table des fins (par personnage et région) dans le joueur.
fn reward_table(x: &mut Ctx, p: u32) {
    let mut offset = (rd8(x.b, p + 3) as u32) << 3;
    if rd16(x.b, A5 + 0x70) != 0 {
        offset += 96;
    }
    for i in 0..2 {
        let v = rd32(x.b, 0xfb7bc + offset + 4 * i);
        wr32(x.b, p + 0x118 + 4 * i, v);
    }
    x.c.d[1] = 0;
}
const S00_VIDEO: u32 = 0xfb712;
const S00_PALETTE: u32 = 0xfb758;

/// État 2 (où tombe l'état 0) : l'écran est vidé.
fn s02_clear(x: &mut Ctx) -> Flow {
    next_step(x);
    wr16(x.b, var::VIDEO, 0x12c0);
    x.wait(1, Etape::S02Cleared)
}
const S02_CLEARED: u32 = 0xfb78c;

/// État 4 : image de fond, fondu d'ouverture.
fn s04_picture(x: &mut Ctx) -> Flow {
    next_step(x);
    tile_blocks(x, 0x917000, 0x222, 0x223, ext::TILE_BLOCK);
    wr16(x.b, var::VIDEO, 0x12ce);
    x.wait(1, Etape::S04Fade)
}
const S04_FADE: u32 = 0xfb8a8;
const S04_TIMER: u32 = 0xfb8c0;

/// État 6 : après la minuterie, crée l'objet de dialogue (SET 70).
fn s06_wait(x: &mut Ctx) -> Flow {
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    s06_object(x)
}
fn s06_object(x: &mut Ctx) -> Flow {
    let Some(Obj(o)) = pools::SET.alloc_68k(x.c, x.b) else { return x.wait(1, Etape::S06Retry) };
    wr8(x.b, o, 1);
    wr8(x.b, o + 2, 0);
    wr8(x.b, o + 3, 70);
    wr8(x.b, o + 0x2f, 0);
    wr16(x.b, BLOCK + f::LINK, o as u16);
    Flow::Done
}
const S06_RETRY: u32 = 0xfb8e0;

/// État 8 : l'objet de dialogue arrive à l'état 6 : fondu au noir puis nouveau décor.
fn s08_wait_dialogue(x: &mut Ctx) -> Flow {
    render(x);
    let o = link(x);
    if rd8(x.b, o + 4) != 6 {
        return Flow::Done;
    }
    x.c.d[0] = 60;
    x.wait(60, Etape::S08Fade)
}
const S08_FADE: u32 = 0xfb916;
const S08_FADED: u32 = 0xfb92c;
const S08_PICTURE: u32 = 0xfb93a;
fn s08_object(x: &mut Ctx) -> Flow {
    x.call_d0(ext::SOBJ_VARIANT, 10);
    if !x.ok() {
        return x.wait(1, Etape::S08Retry);
    }
    wr32(x.b, x.c.a[1] + 0x4c, BLOCK);
    render(x);
    let v = rd16(x.b, var::VIDEO) | 0xe;
    wr16(x.b, var::VIDEO, v);
    fade_settings(x, 3, FADE_ALL);
    crate::taches::fondu::montee(x)
}
/// Réessai de la création de l'objet : `bra 0xfb982` (la remise à zéro du signal n'est pas refaite).
const S08_RETRY: u32 = 0xfb996;

/// État 10 : attend le signal de l'objet, puis ouvre le message 89.
fn s0a_wait_signal(x: &mut Ctx) -> Flow {
    render(x);
    if rd8(x.b, BLOCK + f::SIGNAL) == 0 {
        return Flow::Done;
    }
    next_step(x);
    open_message(x, 89, Etape::S0aRetry)
}
/// Message `n` : A1 est l'objet de dialogue, rangé dans le bloc ; `retry` : cession en cas d'échec.
fn open_message(x: &mut Ctx, n: u32, retry: Etape) -> Flow {
    x.call_d0(ext::MESSAGE, n);
    if !x.ok() {
        return x.wait(1, retry);
    }
    let link = x.c.a[1] as u16;
    wr16(x.b, BLOCK + f::LINK, link);
    Flow::Done
}
const S0A_RETRY: u32 = 0xfb9dc;

/// État 12 : le message est fini : 30 images, puis l'objet SET 13.
fn s0c_wait_dialogue(x: &mut Ctx) -> Flow {
    render(x);
    let o = link(x);
    if rd8(x.b, o + 0x22) == 0 {
        return Flow::Done;
    }
    next_step(x);
    x.wait(30, Etape::S0cObject)
}
const S0C_OBJECT: u32 = 0xfb9fa;
fn s0c_object(x: &mut Ctx) -> Flow {
    x.call_d0(ext::SET_VARIANT, 13);
    if !x.ok() {
        return x.wait(1, Etape::S0cRetry);
    }
    wr32(x.b, x.c.a[1] + 0x4c, BLOCK);
    wr16(x.b, BLOCK + f::TIMER, 180);
    Flow::Done
}
const S0C_RETRY: u32 = 0xfba14;

/// État 14 : après la minuterie, fondu au noir (tâche), image suivante, palette, objets des joueurs.
fn s0e_wait_timer(x: &mut Ctx) -> Flow {
    render(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    fade_settings(x, 3, FADE_ALL);
    wr8(x.b, var::FADE_STEPS, 1);
    task_keep_a0(x, ext::FADE_TASK_OUT);
    s0e_wait_fade(x)
}
fn s0e_wait_fade(x: &mut Ctx) -> Flow {
    if rd8(x.b, var::FADE_STEPS) != 0 {
        return x.wait(1, Etape::S0eLoop);
    }
    tile_blocks(x, 0x91b000, 352, 353, ext::TILE_BLOCK_2);
    set_low_word(&mut x.c.d[0], 543);
    x.c.a[1] = 0x916000;
    x.call(ext::TILE_BLOCK);
    set_low_word(&mut x.c.d[0], 0);
    x.call(ext::TILE_BLOCK);
    wr16(x.b, var::CAMERA_2_Y, 256);
    wr16(x.b, var::CAMERA_2, 0);
    wr16(x.b, var::CAMERA_Y, 256);
    wr16(x.b, var::CAMERA, 128);
    // palette : huit longs copiés puis assombris (ET 0x0fff0fff sur les huit suivants)
    let words: Vec<u32> = (0..8).map(|i| rd32(x.b, 0x291980 + 4 * i)).collect();
    for (i, w) in words.iter().enumerate() {
        wr32(x.b, 0x91c280 + 4 * i as u32, w & 0x0fff_0fff);
    }
    // `movem.l (A1)+,D0-D6/A3` : les registres gardent les huit longs
    x.c.d[..7].copy_from_slice(&words[..7]);
    x.c.a[3] = words[7];
    x.c.a[1] = 0x291980 + 32;
    x.c.a[2] = 0x91c280 + 32;
    s0e_wait_music(x)
}
const S0E_LOOP: u32 = 0xfba68;
/// Attend que la musique ait avancé (`7e70` ≥ 3 ou `7e66` ≥ 1), une image à la fois.
fn s0e_wait_music(x: &mut Ctx) -> Flow {
    let ready = (rd16(x.b, A5 + 0x7e70) as i16) >= 3 && (rd16(x.b, A5 + 0x7e66) as i16) >= 1;
    x.wait(1, if ready { Etape::S0eObjects } else { Etape::S0eMusic })
}
const S0E_MUSIC: u32 = 0xfbb18;
const S0E_OBJECTS: u32 = 0xfbb26;

/// État 16 : attend que la caméra se soit arrêtée et la minuterie, puis ouvre le message 90.
fn s10_wait_camera(x: &mut Ctx) -> Flow {
    render(x);
    if rd16(x.b, var::CAMERA) != 0 || !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    open_message(x, 90, Etape::S10Retry)
}
const S10_RETRY: u32 = 0xfbbae;

/// États 18 et 22 : le message est fini : 30 images.
fn s12_wait_dialogue(x: &mut Ctx) -> Flow {
    render(x);
    let o = link(x);
    if rd8(x.b, o + 0x22) == 0 {
        return Flow::Done;
    }
    wr16(x.b, BLOCK + f::TIMER, 30);
    next_step(x);
    Flow::Done
}

/// État 20 : après la minuterie, ouvre le message 91.
fn s14_wait_timer(x: &mut Ctx) -> Flow {
    render(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    open_message(x, 91, Etape::S14Retry)
}
const S14_RETRY: u32 = 0xfbbf6;

/// État 24 : objet SET 13, caméras remises à zéro.
fn s18_object(x: &mut Ctx) -> Flow {
    x.call_d0(ext::SET_VARIANT, 13);
    if !x.ok() {
        return x.wait(1, Etape::S18Retry);
    }
    wr32(x.b, x.c.a[1] + 0x4c, BLOCK);
    for v in [var::CAMERA, var::CAMERA_Y, var::CAMERA_2, var::CAMERA_2_Y] {
        wr16(x.b, v, 0);
    }
    next_step(x);
    wr8(x.b, BLOCK + f::SIGNAL, 2);
    render(x);
    Flow::Done
}
const S18_RETRY: u32 = 0xfbc16;

/// État 26 : le signal passe à 1 : fondu (tâche), vide tout, prépare les joueurs.
fn s1a_wait_signal(x: &mut Ctx) -> Flow {
    render(x);
    if rd8(x.b, BLOCK + f::SIGNAL) != 1 {
        return Flow::Done;
    }
    next_step(x);
    x.call_d0(ext::SOUND_E0A82, 4);
    wr8(x.b, var::FADE_STEPS, 1);
    wr16(x.b, var::FADE_DELAY, 4);
    wr8(x.b, var::FADE_PAGES, FADE_ALL);
    task_keep_a0(x, ext::FADE_TASK_E);
    s1a_wait_fade(x)
}
fn s1a_wait_fade(x: &mut Ctx) -> Flow {
    if rd8(x.b, var::FADE_STEPS) != 0 {
        return x.wait(1, Etape::S1aLoop);
    }
    wr16(x.b, BLOCK + f::TIMER, 120);
    wr8(x.b, BLOCK + f::SIGNAL, 0xff);
    render(x);
    for f in [ext::RESET_PLAYERS, ext::SPRITES_RESET, ext::POOLS, ext::PLAYER_PREPARE] {
        x.call(f);
    }
    wr16(x.b, var::SCROLL_2, 256);
    x.jump_to(ext::CLEAR_SPRITES)
}
const S1A_LOOP: u32 = 0xfbc92;

/// État 28 : le générique : décor, objets, caméra, attente de l'ouverture.
fn s1c_wait(x: &mut Ctx) -> Flow {
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    wr16(x.b, var::VIDEO, 0x138e);
    x.c.a[1] = 0x91b900;
    x.call_d0(ext::TILE_BLOCK_2, 115);
    x.call_d0(ext::TILE_BLOCK_2, 116);
    wr8(x.b, var::SCREEN_ID, 1);
    wr16(x.b, var::CAMERA_2, 384);
    wr16(x.b, var::CAMERA_2_Y, 0);
    let v = rd16(x.b, var::CAMERA_2);
    wr16(x.b, var::CAMERA_2_COPY, v);
    let v = rd16(x.b, var::CAMERA_2_Y);
    wr16(x.b, var::CAMERA_2_COPY_Y, v);
    wr16(x.b, var::CAMERA_2_MODE, 4);
    x.call(ext::CAMERA_INIT);
    credits_objects_a(x);
    credits_objects_b(x);
    credits_objects_c(x);
    wr8(x.b, A5 + 0x34, 11);
    wr8(x.b, A5 + 0x54e, 2);
    wr16(x.b, BLOCK + f::TIMER, 3);
    s1c_scroll(x)
}
/// `0xfbdd2` : objets du décor d'après une table de la ROM (dix octets par objet, 0xffff termine).
fn credits_objects_a(x: &mut Ctx) {
    let mut a2 = 0xfbe92;
    while rd16(x.b, a2) != 0xffff {
        let Some(Obj(o)) = pools::SOBJ.alloc_68k(x.c, x.b) else { return };
        wr8(x.b, o, 1);
        for (dst, size) in [(8u32, 2u32), (0xc, 2), (0x10, 2), (0x2c, 1), (2, 1), (3, 1), (0x2f, 1)] {
            if size == 2 {
                let v = rd16(x.b, a2);
                wr16(x.b, o + dst, v);
            } else {
                let v = rd8(x.b, a2);
                wr8(x.b, o + dst, v);
            }
            a2 += size;
        }
    }
}
/// `0xfbe0a` : un objet de score par joueur présent.
fn credits_objects_b(x: &mut Ctx) {
    let mut fp = var::PLAYERS;
    let mut a2 = 0xfbebc;
    for (counter, d3) in (0..4u32).rev().zip(0u8..) {
        if rd8(x.b, fp) != 0 && (rd8(x.b, fp + 4) as i8) < 4 {
            x.c.d[0] = 2;
            x.c.d[1] = counter;
            x.call(ext::SCORE_OBJECT);
            if x.ok() {
                let o = x.c.a[1];
                let variant = rd8(x.b, fp + 3);
                wr8(x.b, o + 0x2f, variant);
                let (px, py) = (rd16(x.b, a2), rd16(x.b, a2 + 2));
                a2 += 4;
                let cx = rd16(x.b, var::CAMERA_2);
                let cy = rd16(x.b, var::CAMERA_2_Y);
                wr16(x.b, o + 8, px.wrapping_add(cx));
                wr16(x.b, o + 0x10, py.wrapping_add(cy));
                wr32(x.b, o + 0xc, 0);
                wr8(x.b, o + 0x42, d3);
                wr32(x.b, o + 0x4c, BLOCK);
            }
        }
        fp += 0x200;
    }
}
/// `0xfbe6e` : deux objets (variante 18).
fn credits_objects_c(x: &mut Ctx) {
    x.call_d0(ext::SOBJ_VARIANT, 18);
    if !x.ok() {
        return;
    }
    wr8(x.b, x.c.a[1] + 0x2f, 0);
    x.call_d0(ext::SOBJ_VARIANT, 18);
    if !x.ok() {
        return;
    }
    wr8(x.b, x.c.a[1] + 0x2f, 1);
}
/// `0xfbd38` : rendu avec défilement pendant trois images.
fn s1c_scroll(x: &mut Ctx) -> Flow {
    render_scroll(x);
    x.c.d[0] = 1;
    x.wait(1, Etape::S1cScroll)
}
const S1C_SCROLL: u32 = 0xfbd44;
const S1C_PALETTE: u32 = 0xfbd56;
fn s1c_wait_fade(x: &mut Ctx) -> Flow {
    x.c.d[0] = 1;
    x.wait(1, Etape::S1cFadeLoop)
}
const S1C_FADE_LOOP: u32 = 0xfbdc0;

/// État 30 : le générique défile ; à la fin de la minuterie, fondu (tâche), puis objet SET 18.
fn s1e_fade(x: &mut Ctx) -> Flow {
    render_scroll(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    wr8(x.b, var::FADE_STEPS, 8);
    fade_settings(x, 3, FADE_ALL);
    task_clobber_a0(x, ext::FADE_TASK_C);
    s1e_loop(x)
}
fn s1e_loop(x: &mut Ctx) -> Flow {
    x.c.d[0] = 1;
    x.wait(1, Etape::S1eLoop)
}
const S1E_LOOP: u32 = 0xfbf00;
fn s1e_object(x: &mut Ctx) -> Flow {
    x.call_d0(ext::SET_VARIANT, 18);
    if x.ok() {
        return Flow::Done;
    }
    // `moveq 1,D0 ; bsr 0xfc506 ; jsr 0xa40` : la cession dure D0 images, tel que le rendu le laisse
    x.c.d[0] = 1;
    render_scroll(x);
    crate::taches::attendre(x, Etape::S1eRetry)
}
const S1E_RETRY: u32 = 0xfbf26;

/// État 34 : objet SET 16, fondu (tâche), puis objet SET 20.
fn s22_object(x: &mut Ctx) -> Flow {
    x.call_d0(ext::SET_VARIANT, 16);
    if !x.ok() {
        return Flow::Done;
    }
    wr8(x.b, var::FADE_STEPS, 8);
    fade_settings(x, 3, FADE_ALL);
    task_clobber_a0(x, ext::FADE_TASK_D);
    s22_wait(x)
}
fn s22_wait(x: &mut Ctx) -> Flow {
    render_scroll(x);
    x.c.d[0] = 1;
    x.wait(1, Etape::S22Loop)
}
const S22_LOOP: u32 = 0xfbf64;

/// État 38 : après la minuterie, signal 2 et nouvelle minuterie.
fn s26_wait(x: &mut Ctx) -> Flow {
    render_scroll(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    wr8(x.b, BLOCK + f::SIGNAL, 2);
    next_step(x);
    wr16(x.b, BLOCK + f::TIMER, 120);
    Flow::Done
}

/// État 40 : après la minuterie, fondu au noir (tâche, attente de la fin).
fn s28_fade(x: &mut Ctx) -> Flow {
    render_scroll(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    wr8(x.b, var::FADE_STEPS, 1);
    fade_settings(x, 3, 13);
    task_clobber_a0(x, ext::FADE_TASK_OUT);
    s28_loop(x)
}
fn s28_loop(x: &mut Ctx) -> Flow {
    x.c.d[0] = 1;
    x.wait(1, Etape::S28Loop)
}
const S28_LOOP: u32 = 0xfbfe8;

/// État 42 : vide tout, retire les plans, nouvelle image, objet SOBJ 0.
fn s2a_clear(x: &mut Ctx) -> Flow {
    render_scroll(x);
    for f in [ext::RESET_PLAYERS, ext::SPRITES_RESET, ext::POOLS_EFFCT, ext::POOLS_SOBJ, ext::CLEAR_SPRITES] {
        x.call(f);
    }
    x.wait(1, Etape::S2aPlanes)
}
const S2A_PLANES: u32 = 0xfc022;
const S2A_SCROLLS: u32 = 0xfc034;
fn s2a_wait(x: &mut Ctx) -> Flow {
    render(x);
    x.c.d[0] = 1;
    x.wait(1, Etape::S2aWait)
}
const S2A_WAIT: u32 = 0xfc07e;
fn s2a_object(x: &mut Ctx) -> Flow {
    x.call_d0(ext::SOBJ_VARIANT, 0);
    if !x.ok() {
        render(x);
        x.c.d[0] = 1;
        return x.wait(1, Etape::S2aRetry);
    }
    let o = x.c.a[1];
    wr32(x.b, o + 0x4c, BLOCK);
    wr8(x.b, o + 0x2f, 12);
    wr16(x.b, o + 8, 32);
    wr16(x.b, o + 0xc, 208);
    wr32(x.b, o + 0x10, 0);
    render(x);
    x.wait(1, Etape::S2aShow)
}
const S2A_RETRY: u32 = 0xfc0c2;
const S2A_SHOW: u32 = 0xfc0ee;

/// État 44 : après la minuterie, fondu au noir, vide tout et prépare l'écran des récompenses.
fn s2c_wait(x: &mut Ctx) -> Flow {
    if !timer_tick(x) {
        render(x);
        return Flow::Done;
    }
    next_step(x);
    fade_settings(x, 3, 13);
    wr8(x.b, var::FADE_STEPS, 1);
    task_clobber_a0(x, ext::FADE_TASK_OUT);
    s2c_loop(x)
}
fn s2c_loop(x: &mut Ctx) -> Flow {
    render(x);
    x.c.d[0] = 1;
    x.wait(1, Etape::S2cLoop)
}
const S2C_LOOP: u32 = 0xfc14c;
const S2C_PLANES: u32 = 0xfc17e;
const S2C_CLEARED: u32 = 0xfc18c;
fn s2c_wait_timer(x: &mut Ctx) -> Flow {
    render(x);
    x.c.d[0] = 1;
    x.wait(1, Etape::S2cTimer)
}
const S2C_TIMER: u32 = 0xfc1d8;
/// `bra 0xfc4ea` : un rendu puis retour.
fn render_flow(x: &mut Ctx) -> Flow {
    render(x);
    Flow::Done
}

/// État 46 : le dialogue de fin ouvre puis ferme l'écran (fondus au noir et depuis le noir).
fn s2e_dialogue(x: &mut Ctx) -> Flow {
    let o = link(x);
    if rd8(x.b, o + 5) == 6 {
        fade_settings(x, 3, FADE_ALL);
        return x.sous_programme(crate::taches::fondu::montee, Etape::S2eOpened);
    }
    if rd8(x.b, o + 5) != 10 {
        return render_flow(x);
    }
    fade_settings(x, 3, FADE_ALL);
    x.sous_programme(crate::taches::fondu::descente, Etape::S2eClosed)
}
const S2E_OPENED: u32 = 0xfc286;
const S2E_CLOSED: u32 = 0xfc2ae;

/// État 48 : récompenses : palettes, puis un objet par joueur ayant gagné une récompense.
fn s30_rewards(x: &mut Ctx) -> Flow {
    for (src, dst) in [(0x28fa80u32, 0x91c120u32), (0x28faa0, 0x91c1a0), (0x28f4c0, 0x91c180), (0x292260, 0x91c1e0), (0x291f00, 0x91c200)] {
        for i in 0..8 {
            let v = rd32(x.b, src + 4 * i);
            wr32(x.b, dst + 4 * i, v);
        }
    }
    wr8(x.b, var::FADE_PAGES, 1);
    x.call(ext::BRIGHTNESS_BLACK);
    render(x);
    let mut fp = var::PLAYERS;
    for _ in 0..4 {
        if rd8(x.b, fp) != 0 && (rd8(x.b, fp + 4) as i8) < 4 && rd8(x.b, fp + 0x18a) != 0 {
            let character = rd8(x.b, fp + 3) as u32;
            x.c.d[0] = character;
            x.c.a[6] = fp;
            x.call(ext::REWARD);
            break;
        }
        fp += 0x200;
    }
    x.wait(1, Etape::S30Wait)
}
const S30_WAIT: u32 = 0xfc366;

/// État 50 : dernier écran (quatre cadres de tuiles), puis fin du jeu : phase 20.
fn s32_end(x: &mut Ctx) -> Flow {
    render(x);
    if rd16(x.b, BLOCK + f::TIMER) == 120 {
        let mut a1 = 0x91336c;
        let mut a2 = 0xfc4e2;
        for _ in 0..4 {
            let first = rd16(x.b, a2);
            a2 += 2;
            // deux lignes de quatre tuiles : n, n + 16 puis n + 1, n + 17 (attribut 2)
            for tile in [first, first.wrapping_add(1)] {
                for t in [tile, tile.wrapping_add(16)] {
                    wr16(x.b, a1, t);
                    wr16(x.b, a1 + 2, 2);
                    a1 += 4;
                }
                a1 += 120;
            }
        }
        x.c.a[1] = a1;
        x.c.a[2] = a2;
        x.c.d[1] = 2;
        x.c.d[2] = 0xffff;
    }
    if !timer_tick(x) {
        return Flow::Done;
    }
    fade_settings(x, 3, FADE_ALL);
    x.sous_programme(crate::taches::fondu::descente, Etape::S32Faded)
}
const S32_FADED: u32 = 0xfc450;
const S32_CLEARED: u32 = 0xfc470;

/// Addition décimale codée (huit chiffres, retenue finale perdue) : quatre `abcd`.
fn bcd_add(dest: u32, src: u32) -> u32 {
    let mut carry = 0u32;
    let mut out = 0u32;
    for i in 0..4 {
        let (d, s) = ((dest >> (8 * i)) & 0xff, (src >> (8 * i)) & 0xff);
        let mut lo = (d & 0xf) + (s & 0xf) + carry;
        let mut hi = (d >> 4) + (s >> 4);
        if lo > 9 {
            lo -= 10;
            hi += 1;
        }
        carry = 0;
        if hi > 9 {
            hi -= 10;
            carry = 1;
        }
        out |= (hi << 4 | lo) << (8 * i);
    }
    out
}

pub const NATIVES: &[Native] = &[
    PHASE_28,
];

#[cfg(test)]
mod tests {
    use super::bcd_add;

    #[test]
    fn addition_decimale() {
        assert_eq!(bcd_add(0x0000_0199, 0x0000_0001), 0x0000_0200);
        assert_eq!(bcd_add(0x0123_4567, 0x0876_5430), 0x0999_9997);
        // la retenue finale est perdue
        assert_eq!(bcd_add(0x9999_9999, 0x0000_0001), 0x0000_0000);
    }
}
