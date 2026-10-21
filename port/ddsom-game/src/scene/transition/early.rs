//! Scripts de transition des premiers niveaux : scènes 1, 2, 4 et 10, 5, 14 et 20, 11, 13.

use super::*;

/// Routines du moteur propres à ces scripts.
mod ext {
    /// Musiques et sons d'ambiance des intermèdes.
    pub const MUSIC_A: u32 = 0xe14b0;
    pub const MUSIC_B: u32 = 0xe1564;
    pub const MUSIC_C: u32 = 0xe14ec;
    pub const MUSIC_D: u32 = 0xe1528;
    pub const MUSIC_E: u32 = 0xe15a0;
    pub const SOUND_2: u32 = 0xe0a70;
    pub const SCENE_PALETTES: u32 = 0x1a360;
    pub const PALETTE_SET: u32 = 0x1a366;
    pub const PALETTE_3: u32 = 0x1a8b2;
    pub const SCROLL_2_ON: u32 = 0x2a52;
    pub const FADE_BY_COMPONENT: u32 = 0x1ccc2;
    pub const FADE_TO_PALETTES: u32 = 0x1ce9e;
}
use sub::{TILE_BLOCK as TB, TILE_BLOCK_2 as TB2};

// ---------------------------------------------------------------------------------------------
// Scène 1 : dialogue 96, puis la scène 2

pub fn scene_1(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf8292) {
        0 => show_and_fade_in(x, At(S1_SHOWN.entry)),
        _ => {
            if !message_done(x) {
                return Flow::Done;
            }
            x.c.d[0] = 2;
            finish(x, 2)
        }
    }
}
resume!(S1_SHOWN, 0xf829a, |x| {
    call_d0(x, sub::SOUND, 8);
    next(x);
    s1_message(x)
});
resume!(S1_RETRY, 0xf82b8, s1_message);
fn s1_message(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 96, S1_RETRY.entry));
    Flow::Done
}

// ---------------------------------------------------------------------------------------------
// Scène 2 : dialogues 9 à 13 ; la réponse au dernier choisit la scène suivante (table 0xf847c)

pub fn scene_2(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf82e4) {
        0 => {
            x.call(ext::MUSIC_A);
            if !try_message(x, 9) {
                return Flow::Done;
            }
            next(x);
            x.set_v16(a5::LAYERS, 0x12c0);
            show_and_fade_in(x, At(S2_SHOWN.entry))
        }
        2 => {
            if message_done(x) {
                next(x);
                call_d0(x, sub::SET, 8);
            }
            Flow::Done
        }
        4 => {
            if try_message(x, 10) {
                next(x);
            }
            Flow::Done
        }
        6 => {
            if !message_done(x) {
                return Flow::Done;
            }
            next(x);
            for off in [a5::CAM1_Y, a5::CAM1_Y + a5::COPY, a5::CAM2_Y, a5::CAM2_Y + a5::COPY] {
                x.set_v16(off, 0xfff0);
            }
            hide_layers(x, At(S2_HIDDEN.entry))
        }
        8 => {
            if try_message(x, 11) {
                next(x);
            }
            Flow::Done
        }
        10 | 12 => {
            let n = if get8(x, var::STATE) == 10 { 12 } else { 13 };
            if message_done(x) && try_message(x, n) {
                next(x);
            }
            Flow::Done
        }
        14 => {
            if !message_done(x) {
                return Flow::Done;
            }
            let scene = answer(x, 0xf847c);
            set8(x, var::MESSAGE, scene);
            call_d0(x, sub::SOUND, 0);
            fade(x, ALL_PAGES, sub::FADE_OUT, At(S2_FADED.entry))
        }
        _ => Flow::Done,
    }
}
resume!(S2_SHOWN, 0xf8316, |x| {
    x.call(ext::SCENE_PALETTES);
    x.set_v8(a5::FADE_PAGES, SCENE_PAGES);
    x.call(sub::PALETTES_BLACK);
    Flow::Done
});
resume!(S2_HIDDEN, 0xf8382, |x| {
    tiles(x, TB, 0x20a, Some(SCROLL_2));
    tiles(x, TB, 0x20b, None);
    tiles(x, TB2, 0x14c, Some(SCROLL_3));
    tiles(x, TB2, 0x14d, None);
    layers_or(x, 0xc);
    x.wait(1, S2_DRAWN.entry)
});
resume!(S2_DRAWN, 0xf83cc, |x| fade(x, ALL_PAGES, sub::FADE_IN, Caller));
resume!(S2_FADED, 0xf8456, |x| hide_layers(x, At(S2_CLEARED.entry)));
resume!(S2_CLEARED, 0xf8464, |x| {
    x.call(sub::CLEAR_SCROLL_2);
    x.call(sub::CLEAR_SCROLL_3);
    reset_pools(x);
    let scene = get8(x, var::MESSAGE);
    finish(x, scene)
});

// ---------------------------------------------------------------------------------------------
// Scènes 5, 14 et 20 : dialogues 93 et 94, texte défilant ; scène 108 après la 5, sinon 24

pub fn scene_5(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf848a) {
        0 => {
            next(x);
            x.call(ext::MUSIC_B);
            tiles(x, TB, 0x218, Some(SCROLL_2));
            tiles(x, TB, 0x219, None);
            set16(x, var::TIMER, 60);
            show_and_fade_in(x, Caller)
        }
        2 => {
            if !any_button(x) && !tick(x) {
                return Flow::Done;
            }
            s5_message(x)
        }
        4 => {
            if !message_done(x) {
                return Flow::Done;
            }
            fade(x, SCENE_PAGES, sub::FADE_OUT, At(S5_FADED.entry))
        }
        6 => {
            if !message_done(x) {
                return Flow::Done;
            }
            fade(x, SCENE_PAGES, sub::FADE_OUT, At(S5_TEXT_FADED.entry))
        }
        8 => {
            if !any_button(x) && !tick(x) {
                return Flow::Done;
            }
            call_d0(x, sub::SOUND, 0);
            fade(x, SCENE_PAGES, sub::FADE_OUT, At(S5_END_FADED.entry))
        }
        _ => Flow::Done,
    }
}
resume!(S5_RETRY, 0xf84e8, s5_message);
fn s5_message(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 93, S5_RETRY.entry));
    next(x);
    Flow::Done
}
resume!(S5_FADED, 0xf8502, |x| hide_layers(x, At(S5_HIDDEN.entry)));
resume!(S5_HIDDEN, 0xf8506, |x| {
    tiles(x, TB, 0x24a, Some(SCROLL_2));
    tiles(x, TB, 0, None);
    show_layers(x, At(S5_SHOWN.entry))
});
resume!(S5_SHOWN, 0xf8524, |x| fade(x, SCENE_PAGES, sub::FADE_IN, At(S5_FADED_IN.entry)));
resume!(S5_FADED_IN, 0xf8528, s5_second_message);
resume!(S5_RETRY_2, 0xf853c, s5_second_message);
fn s5_second_message(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 94, S5_RETRY_2.entry));
    next(x);
    Flow::Done
}
resume!(S5_TEXT_FADED, 0xf8556, |x| hide_layers(x, At(S5_TEXT_HIDDEN.entry)));
resume!(S5_TEXT_HIDDEN, 0xf855a, |x| {
    x.call(sub::CLEAR_SCROLL_2);
    scroll_text(x, 0xf858a);
    set16(x, var::TIMER, 120);
    sprites_next_frame(x, S5_TEXT_SHOWN.entry)
});
resume!(S5_TEXT_SHOWN, 0xf8582, |x| fade(x, SCENE_PAGES, sub::FADE_IN, At(S5_TEXT_FADED_IN.entry)));
resume!(S5_TEXT_FADED_IN, 0xf8586, |x| {
    next(x);
    Flow::Done
});
resume!(S5_END_FADED, 0xf8734, |x| {
    x.set_v32(a5::SCROLL_TEXT, 0);
    sprites_next_frame(x, S5_END_DRAWN.entry)
});
resume!(S5_END_DRAWN, 0xf873c, |x| yield_frames(x, 1, At(S5_END.entry)));
resume!(S5_END, 0xf8744, |x| {
    let scene = if x.v8(a5::SCENE) == 5 { 108 } else { 24 };
    finish(x, scene)
});

// ---------------------------------------------------------------------------------------------
// Scènes 4 et 10 : textes défilants, dialogues 15, 92 et 16, deux objets système ; puis la scène 11

pub fn scene_4(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf876a) {
        0 => {
            next(x);
            x.call(ext::MUSIC_A);
            tiles_w(x, TB, 0x20c, Some(SCROLL_2));
            tiles_w(x, TB, 0x20c, None);
            tiles_w(x, TB2, 0x14e, Some(SCROLL_3));
            tiles_w(x, TB2, 0x14f, None);
            x.set_v16(a5::LAYERS, 0x138e);
            yield_frames(x, 1, At(S4_SHOWN.entry))
        }
        2 => {
            drift(x);
            if x.v8(a5::FADE_STEPS) != 0 {
                return Flow::Done;
            }
            if any_button(x) {
                set16(x, var::TIMER, 0);
            }
            if get16(x, var::TIMER) != 0 {
                return Flow::Done;
            }
            next(x);
            fade_task(x, 4, SCENE_PAGES, sub::FADE_TASK_PAGES);
            Flow::Done
        }
        4 => {
            drift(x);
            if x.v8(a5::FADE_STEPS) != 0 {
                return Flow::Done;
            }
            x.set_v32(a5::SCROLL_TEXT, 0);
            sprites_next_frame(x, S4_TEXT_GONE.entry)
        }
        6 => {
            if any_button(x) {
                set16(x, var::TIMER, 1);
            }
            if tick(x) && try_message(x, 15) {
                next(x);
                x.set_v32(a5::SCROLL_TEXT, 0);
            }
            Flow::Done
        }
        8 => {
            if !message_done(x) {
                return Flow::Done;
            }
            fade(x, SCENE_PAGES, sub::FADE_OUT, At(S4_FADED.entry))
        }
        10 => {
            if !message_done(x) {
                return Flow::Done;
            }
            fade(x, SCENE_PAGES, sub::FADE_OUT, At(S4_SECOND_FADED.entry))
        }
        12 => {
            if !message_done(x) {
                return Flow::Done;
            }
            call_d0(x, sub::SOUND, 0);
            fade_task(x, 3, SCENE_PAGES, sub::FADE_TASK_PAGES);
            s4_fading_out(x)
        }
        _ => Flow::Done,
    }
}

/// La minuterie descend ; toutes les quatre images, la caméra 1 avance d'un pixel.
fn drift(x: &mut Ctx) {
    let t = get16(x, var::TIMER).wrapping_sub(1);
    set16(x, var::TIMER, t);
    let phase = t as u8 & 3;
    set_d0_byte(x, phase);
    if phase == 0 {
        let cam = x.v16(a5::CAM1_X).wrapping_add(1);
        set_camera(x, a5::CAM1_X, cam);
    }
}

resume!(S4_SHOWN, 0xf87c2, |x| {
    // palette de la page 7 (huit longs, `movem.l` : les registres gardent les données)
    const FROM: u32 = 0x292260;
    const TO: u32 = 0x91c1e0;
    for k in 0..8 {
        let v = rd32(x.b, FROM + 4 * k);
        wr32(x.b, TO + 4 * k, v);
        if k < 7 {
            x.c.d[k as usize] = v;
        } else {
            x.c.a[3] = v;
        }
    }
    (x.c.a[1], x.c.a[2]) = (TO, FROM);
    scroll_text(x, 0xf8810);
    set16(x, var::TIMER, 120);
    x.set_v8(a5::FADE_PAGES, ALL_PAGES);
    x.call(sub::PALETTES_BLACK);
    fade_task(x, 3, ALL_PAGES, sub::FADE_TASK);
    Flow::Done
});
resume!(S4_TEXT_GONE, 0xf890e, |x| hide_layers(x, At(S4_HIDDEN.entry)));
resume!(S4_HIDDEN, 0xf8912, |x| {
    x.call(sub::CLEAR_SCROLL_2);
    x.call(sub::CLEAR_SCROLL_3);
    set_camera(x, a5::CAM1_X, 0);
    set_camera(x, a5::CAM1_Y, 0xfff0);
    yield_frames(x, 1, At(S4_SECOND_TEXT.entry))
});
resume!(S4_SECOND_TEXT, 0xf893c, |x| {
    tiles(x, TB, 0x20d, Some(SCROLL_2));
    tiles(x, TB, 0x20e, None);
    scroll_text(x, 0xf8986);
    set16(x, var::TIMER, 60);
    sprites_next_frame(x, S4_SECOND_DRAWN.entry)
});
resume!(S4_SECOND_DRAWN, 0xf897c, |x| {
    next(x);
    show_layers(x, At(S4_SECOND_SHOWN.entry))
});
resume!(S4_SECOND_SHOWN, 0xf8982, |x| fade(x, SCENE_PAGES, sub::FADE_IN, Caller));

resume!(S4_FADED, 0xf8a84, |x| hide_layers(x, At(S4_FADED_HIDDEN.entry)));
resume!(S4_FADED_HIDDEN, 0xf8a88, |x| home_camera_1(x, At(S4_HOMED.entry)));
resume!(S4_HOMED, 0xf8a8c, |x| {
    tiles(x, TB, 0x210, Some(SCROLL_2));
    tiles(x, TB, 0x211, None);
    x.c.d[0] = 87;
    dark_palette(x);
    show_layers(x, At(S4_DARK.entry))
});
resume!(S4_DARK, 0xf8ab4, |x| fade(x, SCENE_PAGES, sub::FADE_IN, At(S4_DARK_IN.entry)));
resume!(S4_DARK_IN, 0xf8ab8, s4_message_92);
resume!(S4_RETRY_92, 0xf8acc, s4_message_92);
fn s4_message_92(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 92, S4_RETRY_92.entry));
    set8(x, var::SIGNAL, 0);
    s4_wizard(x)
}
resume!(S4_RETRY_WIZARD, 0xf8aea, s4_wizard);
/// Objet système 0, placé (sous-type 2, X 144, Y 188).
fn s4_wizard(x: &mut Ctx) -> Flow {
    let Some(o) = system_object(x, 0, S4_RETRY_WIZARD.entry) else { return Flow::Yield };
    wr8(x.b, o.0 + 0x2f, 2);
    wr16(x.b, o.0 + 8, 144);
    wr16(x.b, o.0 + 0x10, 0);
    wr16(x.b, o.0 + 0xc, 188);
    next(x);
    Flow::Done
}

resume!(S4_SECOND_FADED, 0xf8b1c, |x| {
    set8(x, var::SIGNAL, 0xff);
    world(x);
    sprites_next_frame(x, S4_SECOND_CLEAR.entry)
});
resume!(S4_SECOND_CLEAR, 0xf8b2a, |x| {
    x.call(sub::CLEAR_SPRITES);
    hide_layers(x, At(S4_SECOND_HIDDEN.entry))
});
resume!(S4_SECOND_HIDDEN, 0xf8b34, |x| {
    x.call(sub::CLEAR_SCROLL_2);
    home_camera_1(x, At(S4_SECOND_HOMED.entry))
});
resume!(S4_SECOND_HOMED, 0xf8b3e, |x| {
    set8(x, var::SIGNAL, 0);
    s4_second_object(x)
});
resume!(S4_RETRY_OBJECT, 0xf8b56, s4_second_object);
fn s4_second_object(x: &mut Ctx) -> Flow {
    if system_object(x, 1, S4_RETRY_OBJECT.entry).is_none() {
        return Flow::Yield;
    }
    world(x);
    sprites_next_frame(x, S4_SCENERY.entry)
}
resume!(S4_SCENERY, 0xf8b64, |x| {
    // décor en huit blocs de tuiles du plan 2 (le troisième avec D4 = 0)
    for (n, at) in [(206, 0x917000), (173, 0x917400), (175, 0x917800), (175, 0x917c00), (172, 0x916000), (205, 0x916400),
        (174, 0x916800), (174, 0x916c00)]
    {
        if at == 0x917800 {
            x.c.d[4] = 0;
        }
        tiles_w(x, TB, n, Some(at));
    }
    x.set_v16(a5::CAM2_Y, 0xffe0);
    x.set_v16(a5::CAM1_Y, 40);
    tiles(x, TB2, 0x150, Some(SCROLL_3));
    tiles(x, TB2, 0x151, None);
    x.call(ext::SCROLL_2_ON);
    x.set_v8(0x36, 1);
    x.set_v16(a5::LAYERS, 0x138e);
    wr16(x.b, 0x400004, 0x3155);
    show_layers(x, At(S4_SCENERY_SHOWN.entry))
});
resume!(S4_SCENERY_SHOWN, 0xf8c2c, |x| {
    fade_task(x, 3, SCENE_PAGES, sub::FADE_TASK);
    s4_fading_in(x)
});
/// Le monde tourne jusqu'à la fin du fondu (0xf8c3c), puis le dialogue 16.
fn s4_fading_in(x: &mut Ctx) -> Flow {
    world(x);
    sprites_next_frame(x, S4_FADING_IN.entry)
}
resume!(S4_FADING_IN, 0xf8c44, |x| yield_frames(x, 1, At(S4_FADING_IN_NEXT.entry)));
resume!(S4_FADING_IN_NEXT, 0xf8c4c, |x| {
    if x.v8(a5::FADE_STEPS) != 0 {
        return s4_fading_in(x);
    }
    s4_message_16(x)
});
resume!(S4_RETRY_16, 0xf8c66, s4_message_16);
fn s4_message_16(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 16, S4_RETRY_16.entry));
    next(x);
    Flow::Done
}
/// 0xf8c94 : le monde tourne jusqu'à la fin du fondu de sortie.
fn s4_fading_out(x: &mut Ctx) -> Flow {
    world(x);
    sprites_next_frame(x, S4_FADING_OUT.entry)
}
resume!(S4_FADING_OUT, 0xf8c9c, |x| yield_frames(x, 1, At(S4_FADING_OUT_NEXT.entry)));
resume!(S4_FADING_OUT_NEXT, 0xf8ca4, |x| {
    if x.v8(a5::FADE_STEPS) != 0 {
        return s4_fading_out(x);
    }
    set8(x, var::SIGNAL, 1);
    world(x);
    sprites_next_frame(x, S4_FADED_OUT.entry)
});
resume!(S4_FADED_OUT, 0xf8cb8, |x| yield_frames(x, 1, At(S4_END.entry)));
resume!(S4_END, 0xf8cc0, |x| hide_layers(x, At(S4_END_HIDDEN.entry)));
resume!(S4_END_HIDDEN, 0xf8cc4, |x| {
    x.call(sub::CLEAR_SCROLL_2);
    x.call(sub::CLEAR_SCROLL_3);
    finish(x, 11)
});

// ---------------------------------------------------------------------------------------------
// Scène 11 (s'il reste du temps) : objet système 2, dialogues 17 et 18 ; puis la scène 13

pub fn scene_11(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf8ce4) {
        0 => {
            if x.v8(a5::CLOCK) == 0 {
                x.c.d[0] = 13;
                return finish(x, 13);
            }
            next(x);
            x.call(ext::MUSIC_C);
            set_camera(x, a5::CAM1_X, 288);
            tiles(x, TB, 0x214, Some(0x917400));
            tiles(x, TB, 0x215, None);
            tiles(x, TB2, 0x152, Some(SCROLL_3));
            tiles(x, TB2, 0x153, None);
            s11_object(x)
        }
        2 => {
            if !tick(x) {
                return Flow::Done;
            }
            set16(x, var::TIMER, 60);
            s11_message_17(x)
        }
        4 => {
            if get8(x, var::FLAG) != 0 && message_done(x) {
                set8(x, var::FLAG, 0);
                set16(x, var::TIMER, 1);
            }
            if tick(x) {
                set16(x, var::TIMER, 120);
                set8(x, var::SIGNAL, 1);
                next(x);
            }
            Flow::Done
        }
        6 => {
            if get8(x, var::FLAG) != 0 && message_done(x) {
                set8(x, var::FLAG, 0);
            }
            if get16(x, var::TIMER) != 0 {
                if !tick(x) {
                    return Flow::Done;
                }
                set8(x, var::SIGNAL, 0);
            }
            let busy = get8(x, var::SIGNAL).wrapping_add(get8(x, var::FLAG));
            x.c.d[0] = busy as u32;
            if busy != 0 {
                return Flow::Done;
            }
            s11_message_18(x)
        }
        8 => {
            if !message_done(x) {
                return Flow::Done;
            }
            call_d0(x, sub::SOUND, 0);
            fade(x, SCENE_PAGES, sub::FADE_OUT, At(S11_FADED.entry))
        }
        _ => Flow::Done,
    }
}
resume!(S11_RETRY_OBJECT, 0xf8d5a, s11_object);
fn s11_object(x: &mut Ctx) -> Flow {
    if system_object(x, 2, S11_RETRY_OBJECT.entry).is_none() {
        return Flow::Yield;
    }
    set8(x, var::SIGNAL, 0);
    world(x);
    sprites_next_frame(x, S11_DRAWN.entry)
}
resume!(S11_DRAWN, 0xf8d6c, |x| {
    layers_or(x, 0xe);
    yield_frames(x, 1, At(S11_SHOWN.entry))
});
resume!(S11_SHOWN, 0xf8d7a, |x| {
    set16(x, var::TIMER, 10);
    x.set_v16(a5::FADE_DELAY, 4);
    x.set_v8(a5::FADE_PAGES, ALL_PAGES);
    x.jump_to(sub::FADE_IN)
});
resume!(S11_RETRY_17, 0xf8db4, s11_message_17);
fn s11_message_17(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 17, S11_RETRY_17.entry));
    next(x);
    set8(x, var::FLAG, 1);
    Flow::Done
}
resume!(S11_RETRY_18, 0xf8e48, s11_message_18);
fn s11_message_18(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 18, S11_RETRY_18.entry));
    next(x);
    Flow::Done
}
resume!(S11_FADED, 0xf8e6a, |x| {
    set8(x, var::SIGNAL, 1);
    world(x);
    sprites_next_frame(x, S11_DRAWN_OUT.entry)
});
resume!(S11_DRAWN_OUT, 0xf8e78, |x| hide_layers(x, At(S11_HIDDEN.entry)));
resume!(S11_HIDDEN, 0xf8e7c, |x| home_camera_1(x, At(S11_END.entry)));
resume!(S11_END, 0xf8e80, |x| {
    x.call(sub::CLEAR_SCROLL_2);
    x.call(sub::CLEAR_SCROLL_3);
    x.c.d[0] = 13;
    finish(x, 13)
});

// ---------------------------------------------------------------------------------------------
// Scène 13 : selon le temps restant, objets SET 10 et 11 ou 9 ; dialogue 30 puis un choix (dialogues 136 à 138)
// qui fixe la scène suivante (tables 0xf92e4, 0xf932e, 0xf937a ; 14 si aucun joueur n'est dans l'état attendu)

pub fn scene_13(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf8e9e) {
        0 => {
            next(x);
            x.call(ext::MUSIC_D);
            tiles_w(x, TB2, 0x154, Some(SCROLL_3));
            tiles_w(x, TB2, 0x155, None);
            s13_object(x)
        }
        2 => {
            if get8(x, var::SIGNAL) != 0 {
                set8(x, var::SIGNAL, 0);
                next(x);
                set8(x, var::SUB, 0);
            }
            Flow::Done
        }
        4 if x.v16(a5::CLOCK) != 0 => s13_time_left(x),
        4 => s13_time_out(x),
        6 => {
            world(x);
            sprites_next_frame(x, S13_CLOSING.entry)
        }
        8 => {
            if !message_done(x) {
                return Flow::Done;
            }
            next(x);
            set8(x, var::SUB, 0);
            s13_route(x);
            Flow::Done
        }
        10 => match dispatch(x, var::FLAG, 0xf9294) {
            0 => choice(x, 138, 0xf92a8, 0xf92e4),
            2 => choice(x, 136, 0xf92f2, 0xf932e),
            6 => choice(x, 137, 0xf933e, 0xf937a),
            _ => {
                next(x);
                set8(x, var::FLAG, 14);
                Flow::Done
            }
        },
        12 => fade(x, SCENE_PAGES, sub::FADE_OUT, At(S13_FADED.entry)),
        _ => Flow::Done,
    }
}
resume!(S13_RETRY_OBJECT, 0xf8ee2, s13_object);
fn s13_object(x: &mut Ctx) -> Flow {
    if system_object(x, 3, S13_RETRY_OBJECT.entry).is_none() {
        return Flow::Yield;
    }
    set8(x, var::SIGNAL, 0);
    world(x);
    sprites_next_frame(x, S13_DRAWN.entry)
}
resume!(S13_DRAWN, 0xf8ef4, |x| {
    layers_or(x, 0xe);
    yield_frames(x, 1, At(S13_SHOWN.entry))
});
resume!(S13_SHOWN, 0xf8f02, |x| {
    x.set_v16(a5::FADE_DELAY, 4);
    x.set_v8(a5::FADE_PAGES, ALL_PAGES);
    x.jump_to(sub::FADE_IN)
});

/// Temps restant : objet SET 10, fondu, nouveau décor et objet SET 11 ; l'objet signale sa fin (`+8`).
fn s13_time_left(x: &mut Ctx) -> Flow {
    match dispatch(x, var::SUB, 0xf8f4e) {
        0 => {
            if try_set_object(x, 10).is_some() {
                next_sub(x);
                x.set_v8(a5::FADE_STEPS, 1);
                fade_task(x, 4, SCENE_PAGES, sub::FADE_TASK_PAGES);
            }
            Flow::Done
        }
        2 => {
            if x.v8(a5::FADE_STEPS) != 0 {
                return Flow::Done;
            }
            set8(x, var::SIGNAL, 0xff);
            world(x);
            sprites_next_frame(x, S13_LEFT_DRAWN.entry)
        }
        4 => {
            if x.v8(a5::FADE_STEPS) == 0 {
                set8(x, var::SIGNAL, 1);
                next_sub(x);
            }
            Flow::Done
        }
        6 => {
            if get8(x, var::SIGNAL) == 2 {
                next_sub(x);
            }
            Flow::Done
        }
        8 => s13_leave(x),
        _ => Flow::Done,
    }
}
/// Fin de l'intermède quand l'objet a signalé (`+8` négatif) : fondu vers le noir.
fn s13_leave(x: &mut Ctx) -> Flow {
    if (get8(x, var::SIGNAL) as i8) >= 0 {
        return Flow::Done;
    }
    next(x);
    set8(x, var::SUB, 0);
    fade(x, SCENE_PAGES, sub::FADE_OUT, Caller)
}
resume!(S13_LEFT_DRAWN, 0xf8f98, |x| yield_frames(x, 1, At(S13_LEFT_WAITED.entry)));
resume!(S13_LEFT_WAITED, 0xf8fa0, |x| hide_layers(x, At(S13_LEFT_HIDDEN.entry)));
resume!(S13_LEFT_HIDDEN, 0xf8fa4, |x| yield_frames(x, 4, At(S13_LEFT_SCENERY.entry)));
resume!(S13_LEFT_SCENERY, 0xf8fac, |x| {
    let a6 = x.c.a[6];
    call_d0(x, ext::PALETTE_SET, 1);
    call_d0(x, sub::LOAD_PALETTE, 29);
    call_d0(x, ext::PALETTE_3, 11);
    x.c.a[6] = a6;
    x.set_v8(0x36, 1);
    tiles(x, TB, 8, Some(SCROLL_2));
    for n in [9, 10, 8] {
        tiles(x, TB, n, None);
    }
    tiles(x, TB2, 115, Some(SCROLL_3));
    tiles(x, TB2, 116, None);
    s13_set_11(x)
});
resume!(S13_RETRY_11, 0xf9022, s13_set_11);
fn s13_set_11(x: &mut Ctx) -> Flow {
    or_yield!(set_object(x, 11, S13_RETRY_11.entry));
    next_sub(x);
    world(x);
    sprites_next_frame(x, S13_SET_11_DRAWN.entry)
}
resume!(S13_SET_11_DRAWN, 0xf9034, |x| yield_frames(x, 1, At(S13_SET_11_SHOWN.entry)));
resume!(S13_SET_11_SHOWN, 0xf903c, |x| {
    x.c.d[0] = 0;
    x.c.d[1] = x.c.d[1] & !0xffff | 144;
    x.call(ext::SOUND_2);
    x.set_v16(a5::LAYERS, 0x138e);
    x.set_v8(a5::FADE_PAGES, SCENE_PAGES);
    x.set_v16(a5::FADE_DELAY, 4);
    x.set_v8(a5::FADE_STEPS, 1);
    start_task(x, sub::FADE_TASK);
    Flow::Done
});

/// Temps écoulé : fondu par composante, autre décor, objet SET 9, puis fondu vers des palettes fixées.
fn s13_time_out(x: &mut Ctx) -> Flow {
    match dispatch(x, var::SUB, 0xf90a6) {
        0 => {
            call_d0(x, sub::SOUND, 0);
            next_sub(x);
            x.set_v16(a5::FADE_DELAY, 1);
            x.set_v8(a5::FADE_PAGES, SCENE_PAGES);
            goto(x, ext::FADE_BY_COMPONENT, At(S13_OUT_FADED.entry))
        }
        2 => s13_out_scenery(x),
        4 => {
            if get8(x, var::SIGNAL) == 0 {
                return Flow::Done;
            }
            set8(x, var::SIGNAL, 0);
            world(x);
            sprites_next_frame(x, S13_OUT_DRAWN.entry)
        }
        6 => s13_leave(x),
        _ => Flow::Done,
    }
}
resume!(S13_OUT_FADED, 0xf90cc, |x| yield_frames(x, 30, At(S13_OUT_SCENERY.entry)));
resume!(S13_OUT_SCENERY, 0xf90d4, s13_out_scenery);
fn s13_out_scenery(x: &mut Ctx) -> Flow {
    next_sub(x);
    tiles_w(x, TB, 0x22a, Some(SCROLL_2));
    tiles_w(x, TB, 0x22b, None);
    tiles_w(x, TB2, 0x164, Some(SCROLL_3));
    tiles(x, TB2, 0x165, None);
    set8(x, var::SIGNAL, 0xff);
    world(x);
    sprites_next_frame(x, S13_OUT_SCENERY_DRAWN.entry)
}
resume!(S13_OUT_SCENERY_DRAWN, 0xf911c, |x| {
    set8(x, var::SIGNAL, 0);
    s13_set_9(x)
});
resume!(S13_RETRY_9, 0xf9134, s13_set_9);
fn s13_set_9(x: &mut Ctx) -> Flow {
    or_yield!(set_object(x, 9, S13_RETRY_9.entry));
    Flow::Done
}
resume!(S13_OUT_DRAWN, 0xf9150, |x| {
    next_sub(x);
    x.call(ext::MUSIC_E);
    for (off, palettes) in [(0x492, 0x253000), (0x49a, 0x272a60), (0x49e, 0x283140)] {
        x.set_v32(off, palettes);
    }
    x.set_v16(a5::FADE_DELAY, 1);
    x.set_v8(a5::FADE_PAGES, SCENE_PAGES);
    goto(x, ext::FADE_TO_PALETTES, Caller)
});

// état 6 : nouveau décor, dialogue 30
resume!(S13_CLOSING, 0xf919e, |x| yield_frames(x, 1, At(S13_CLOSING_WAITED.entry)));
resume!(S13_CLOSING_WAITED, 0xf91a6, |x| hide_layers(x, At(S13_CLOSING_HIDDEN.entry)));
resume!(S13_CLOSING_HIDDEN, 0xf91aa, |x| {
    set_camera(x, a5::CAM1_X, 8);
    for off in [a5::CAM1_Y, a5::CAM2_X, a5::CAM2_Y] {
        set_camera(x, off, 0);
    }
    x.call(sub::CLEAR_SCROLL_3);
    set_d0_word(x, 87);
    x.call(sub::LOAD_PALETTE);
    set_d0_word(x, 87);
    x.call(ext::PALETTE_3);
    tiles_w(x, TB, 0x216, Some(SCROLL_2));
    tiles_w(x, TB, 0x217, None);
    yield_frames(x, 4, At(S13_CLOSING_DRAWN.entry))
});
resume!(S13_CLOSING_DRAWN, 0xf9210, |x| show_layers(x, At(S13_CLOSING_SHOWN.entry)));
resume!(S13_CLOSING_SHOWN, 0xf9214, |x| fade(x, SCENE_PAGES, sub::FADE_IN, At(S13_CLOSING_IN.entry)));
resume!(S13_CLOSING_IN, 0xf921a, s13_message_30);
resume!(S13_RETRY_30, 0xf922e, s13_message_30);
fn s13_message_30(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 30, S13_RETRY_30.entry));
    next(x);
    Flow::Done
}

/// État 8 : la question à poser (`+6`) dépend des joueurs : 0 si un joueur en jeu (état `+4` < 4) est dans la
/// posture 4 ou 5 (`+3`), 4 sinon ; plus 2 s'il reste du temps.
fn s13_route(x: &mut Ctx) {
    let mut route = 0u8;
    let mut found = false;
    for k in 0..4u32 {
        let p = A5 + a5::PLAYERS + k * 0x200;
        x.c.a[1] = p;
        x.c.d[1] = 3 - k;
        if rd8(x.b, p) != 0 && (rd8(x.b, p + 4) as i8) < 4 {
            let pose = (rd8(x.b, p + 3) >> 1).wrapping_sub(2);
            x.c.d[2] = x.c.d[2] & !0xff | pose as u32;
            if pose == 0 {
                found = true;
                break;
            }
        }
    }
    if !found {
        x.c.a[1] = A5 + a5::PLAYERS + 0x800;
        x.c.d[1] = 0xffff;
        route += 4;
    }
    if x.v16(a5::CLOCK) != 0 {
        route += 2;
    }
    x.c.d[0] = route as u32;
    set8(x, var::FLAG, route);
}

/// Question `n`, puis la scène suivante d'après la réponse (table `answers`).
fn choice(x: &mut Ctx, n: u16, table: u32, answers: u32) -> Flow {
    match dispatch(x, var::SUB, table) {
        0 => {
            set_d0_word(x, n);
            x.call(sub::MESSAGE);
            if x.c.sr & 1 == 0 {
                next_sub(x);
                store_message(x);
            }
        }
        _ => {
            if message_done(x) {
                let scene = answer(x, answers);
                set8(x, var::FLAG, scene);
                next(x);
                set8(x, var::SUB, 0);
            }
        }
    }
    Flow::Done
}

resume!(S13_FADED, 0xf938a, |x| hide_layers(x, At(S13_END.entry)));
resume!(S13_END, 0xf938e, |x| {
    x.call(sub::CLEAR_SCROLL_2);
    let scene = get8(x, var::FLAG);
    x.c.d[0] = scene as u32;
    finish(x, scene)
});

pub const NATIVES: &[Native] = &[
    S1_SHOWN, S1_RETRY, S2_SHOWN, S2_HIDDEN, S2_DRAWN, S2_FADED, S2_CLEARED, S5_RETRY, S5_FADED, S5_HIDDEN, S5_SHOWN,
    S5_FADED_IN, S5_RETRY_2, S5_TEXT_FADED, S5_TEXT_HIDDEN, S5_TEXT_SHOWN, S5_TEXT_FADED_IN, S5_END_FADED, S5_END_DRAWN,
    S5_END, S4_SHOWN, S4_TEXT_GONE, S4_HIDDEN, S4_SECOND_TEXT, S4_SECOND_DRAWN, S4_SECOND_SHOWN, S4_FADED,
    S4_FADED_HIDDEN, S4_HOMED, S4_DARK, S4_DARK_IN, S4_RETRY_92, S4_RETRY_WIZARD, S4_SECOND_FADED, S4_SECOND_CLEAR,
    S4_SECOND_HIDDEN, S4_SECOND_HOMED, S4_RETRY_OBJECT, S4_SCENERY, S4_SCENERY_SHOWN, S4_FADING_IN, S4_FADING_IN_NEXT,
    S4_RETRY_16, S4_FADING_OUT, S4_FADING_OUT_NEXT, S4_FADED_OUT, S4_END, S4_END_HIDDEN, S11_RETRY_OBJECT, S11_DRAWN,
    S11_SHOWN, S11_RETRY_17, S11_RETRY_18, S11_FADED, S11_DRAWN_OUT, S11_HIDDEN, S11_END, S13_RETRY_OBJECT, S13_DRAWN,
    S13_SHOWN, S13_LEFT_DRAWN, S13_LEFT_WAITED, S13_LEFT_HIDDEN, S13_LEFT_SCENERY, S13_RETRY_11, S13_SET_11_DRAWN,
    S13_SET_11_SHOWN, S13_OUT_FADED, S13_OUT_SCENERY, S13_OUT_SCENERY_DRAWN, S13_RETRY_9, S13_OUT_DRAWN, S13_CLOSING,
    S13_CLOSING_WAITED, S13_CLOSING_HIDDEN, S13_CLOSING_DRAWN, S13_CLOSING_SHOWN, S13_CLOSING_IN, S13_RETRY_30,
    S13_FADED, S13_END,
];
