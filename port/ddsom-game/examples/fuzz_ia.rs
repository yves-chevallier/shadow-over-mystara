//! Fuzz différentiel des cœurs du lot IA (`ai.rs`, `ai_aids.rs`, routines de `families::enemy_lib`) : chaque cœur
//! Rust est comparé à la routine d'origine, exécutée par l'interpréteur 68000 seul (aucune native), sur des états
//! tirés au sort à partir d'une partie réelle (un ennemi ou un boss actif sert d'objet A0 ; joueurs, cible, caméra,
//! rang et tirage pseudo-aléatoire perturbés). On compare la mémoire (RAM, vidéo, palettes...) et le résultat du
//! cœur à ce que l'original laisse dans ses registres (Z, D0, A1...). Les registres de travail ne sont pas comparés :
//! les cœurs n'en écrivent plus.
//!
//!   fuzz_ia <roms> <eeprom> <scénario> <images de chauffe> <tours> [entrée hexadécimale...]
//!
//! Variables : `DDSOM_GRAINE` (graine du tirage), `DDSOM_VERBOSE` (chaque écart).

use std::path::Path;

use ddsom_game::families::cx::Cx;
use ddsom_game::families::enemy_lib as lib;
use ddsom_game::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use ddsom_game::objects::{pools, Obj};
use ddsom_game::{ai, ai_aids};

type Cpu = m68k::Cpu;
type Bus = cps2::Cps2Bus;

struct Alea(u64);

impl Alea {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn dans(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn parmi<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.dans(v.len() as u32) as usize]
    }
}

/// Adresse de retour fictive de l'appel de l'original.
const RETOUR: u32 = 0x00_fffe;

/// Exécute la routine d'origine `entree` (interpréteur seul) jusqu'à son retour.
fn original(c: &mut Cpu, b: &mut Bus, entree: u32) {
    let sp = c.a[7];
    c.a[7] = sp.wrapping_sub(4);
    wr32(b, c.a[7], RETOUR);
    c.pc = entree;
    let mut n = 0u32;
    while !(c.pc == RETOUR && c.a[7] == sp) {
        c.step(b);
        n += 1;
        assert!(n < 5_000_000, "{entree:06x} : l'original ne revient pas");
    }
}

/// Un objet actif d'un pool d'ennemis ou de boss, s'il y en a.
fn acteurs(b: &mut Bus) -> Vec<u32> {
    let mut v = Vec::new();
    for pool in [&pools::ENEMY, &pools::BOSS] {
        for k in 0..pool.slots {
            let o = A5 + pool.first + k * pool.stride;
            if rd8(b, o) != 0 {
                v.push(o);
            }
        }
    }
    v
}

/// Perturbe l'état autour de l'objet `o` : joueurs (présence, PV, état, aura, position), cible, caméra, rang, hasard.
fn perturbe(c: &mut Cpu, b: &mut Bus, alea: &mut Alea, o: u32) {
    for r in 0..8 {
        c.d[r] = alea.next();
    }
    for r in 1..5 {
        c.a[r] = 0xff0000 | (alea.next() & 0xfffe);
    }
    c.a[0] = o;
    c.a[5] = A5;
    let (x, y) = (rd16(b, o + 8), rd16(b, o + 0x10));
    let mut vivants = 0;
    for k in 0..4u32 {
        let p = A5 + 0x5de + 0x200 * k;
        let actif = alea.dans(4) != 0;
        vivants += actif as u8;
        wr8(b, p, if actif { alea.parmi(&[1, 1, 1, 2]) } else { 0 });
        wr8(b, p + 1, alea.parmi(&[0, 0, 1, 2, 3]));
        wr8(b, p + 3, alea.dans(12) as u8);
        wr8(b, p + 4, alea.parmi(&[2, 2, 2, 4, 6]));
        wr8(b, p + 6, alea.parmi(&[0, 12, 14, 0x34]));
        wr16(b, p + 0x62, if alea.dans(5) == 0 { 0 } else { alea.dans(0x200) as u16 });
        wr8(b, p + 0x182, (alea.dans(5) == 0) as u8);
        wr8(b, p + 0x188, (alea.dans(6) == 0) as u8);
        wr8(b, p + 0x66, (alea.dans(6) == 0) as u8);
        wr8(b, p + 0x67, alea.dans(3) as u8);
        wr8(b, p + 0x68, alea.dans(4) as u8);
        wr8(b, p + 0x69, alea.dans(4) as u8);
        wr16(b, p + 8, x.wrapping_add(alea.dans(0x200) as u16).wrapping_sub(0x100));
        wr16(b, p + 0x10, y.wrapping_add(alea.dans(0x80) as u16).wrapping_sub(0x40));
        wr16(b, p + 0xc, if alea.dans(3) == 0 { alea.dans(0x40) as u16 } else { 0 });
        wr8(b, p + 0x2c, alea.parmi(&[0, 0x20]));
    }
    wr8(b, A5 + 0x35, vivants.min(alea.dans(5) as u8));
    wr8(b, A5 + 0x36, (alea.dans(4) == 0) as u8);
    wr8(b, A5 + 0x48, alea.dans(8) as u8);
    wr8(b, A5 + 0x49, alea.dans(8) as u8);
    wr8(b, A5 + 0x5a, alea.dans(6) as u8);
    wr16(b, A5 + 0x7f92, alea.dans(32) as u16);
    wr16(b, A5 + 0x1a, alea.next() as u16);
    for off in [0x506u32, 0x556, 0x5a6, 0x5ae] {
        wr16(b, A5 + off, x.wrapping_add(alea.dans(0x300) as u16).wrapping_sub(0x200));
    }
    for off in [0x50au32, 0x55a, 0x5aa] {
        wr16(b, A5 + off, y.wrapping_add(alea.dans(0x200) as u16).wrapping_sub(0x100));
    }
    // cible : un joueur, rien, ou un autre acteur
    let cible = match alea.dans(6) {
        0 => 0,
        1 => acteurs(b).first().copied().unwrap_or(0) as u16,
        _ => (A5 + 0x5de + 0x200 * alea.dans(4)) as u16,
    };
    wr16(b, o + 0x50, cible);
    wr8(b, o + 0x2c, alea.parmi(&[0, 0x20]));
    wr8(b, o + 0x2d, alea.dans(32) as u8);
    wr16(b, o + 0x14, alea.next() as u16);
    wr16(b, o + 0xc, if alea.dans(3) == 0 { alea.dans(0x40) as u16 } else { 0 });
    if alea.dans(3) == 0 {
        wr16(b, o + 8, alea.next() as u16 & 0xfff);
        wr16(b, o + 0x10, alea.next() as u16 & 0x7ff);
    }
    wr16(b, o + 0x7a, x.wrapping_add(alea.dans(0x20) as u16).wrapping_sub(0x10));
    wr16(b, o + 0x7c, y.wrapping_add(alea.dans(0x20) as u16).wrapping_sub(0x10));
    wr8(b, o + 0x99, alea.dans(6) as u8);
    wr8(b, o + 0x9d, (alea.dans(3) == 0) as u8);
    wr8(b, o + 0x98, alea.dans(4) as u8);
    wr8(b, o + 0x3e, alea.dans(8) as u8);
    wr8(b, o + 0xaa, alea.dans(4) as u8);
    wr8(b, o + 0xab, alea.dans(5) as u8);
    wr8(b, o + 0xac, (alea.dans(4) * 32 + alea.dans(16) * 2) as u8);
    wr16(b, o + 0x62, alea.dans(0x400) as u16);
}

/// Résultat comparé : celui du cœur, et ce que l'original laisse (lu dans ses registres).
type Coeur = fn(&mut Cpu, &mut Bus, Obj) -> u32;
type Lecture = fn(&Cpu) -> u32;

fn z(c: &Cpu) -> u32 {
    (c.sr & m68k::SR_Z != 0) as u32
}
fn nz(c: &Cpu) -> u32 {
    (c.sr & m68k::SR_Z == 0) as u32
}
fn d0(c: &Cpu) -> u32 {
    c.d[0]
}
fn d5(c: &Cpu) -> u32 {
    c.d[5] as u16 as u32
}
fn rien(_: &Cpu) -> u32 {
    0
}
/// La cible trouvée (A1, 0 si aucune) quand Z est clair.
fn a1_si(c: &Cpu) -> u32 {
    if c.sr & m68k::SR_Z == 0 {
        c.a[1] & 0xffffff
    } else {
        0
    }
}
fn obj(o: Option<Obj>) -> u32 {
    o.map_or(0, |o| o.0 & 0xffffff)
}

struct Cas {
    entree: u32,
    nom: &'static str,
    /// Registres d'entrée posés avant l'appel (selon l'état tiré).
    entrees: fn(&mut Cpu, &mut Bus, &mut Alea),
    coeur: Coeur,
    lecture: Lecture,
}

fn aucune(_: &mut Cpu, _: &mut Bus, _: &mut Alea) {}

fn boite(c: &mut Cpu, _: &mut Bus, a: &mut Alea) {
    c.d[0] = c.d[0] & !0xffff | (a.dans(0x80) as u16).wrapping_sub(0x40) as u32;
    c.d[1] = c.d[1] & !0xffff | a.dans(0xa0) + 4;
    c.d[2] = c.d[2] & !0xffff | a.dans(0x80) + 2;
}

fn critere(c: &mut Cpu, _: &mut Bus, a: &mut Alea) {
    c.d[0] = a.parmi(&[0, 2, 4]);
}

fn point(c: &mut Cpu, b: &mut Bus, a: &mut Alea) {
    let o = c.a[0];
    c.d[1] = c.d[1] & !0xffff | rd16(b, o + 8).wrapping_add(a.dans(0x200) as u16).wrapping_sub(0x100) as u32;
    c.d[2] = c.d[2] & !0xffff | rd16(b, o + 0x10).wrapping_add(a.dans(0x200) as u16).wrapping_sub(0x100) as u32;
}

fn marge(c: &mut Cpu, _: &mut Bus, a: &mut Alea) {
    c.d[0] = c.d[0] & !0xffff | a.parmi(&[0x10, 0x20, 0x30, 0x40]);
}

fn bande(c: &mut Cpu, _: &mut Bus, a: &mut Alea) {
    c.d[0] = c.d[0] & !0xffff | (a.dans(0x100) as u16).wrapping_sub(0x80) as u32;
    c.d[1] = c.d[1] & !0xffff | 0x100 + a.dans(0x100);
}

/// Table de paliers (A1) : une table de boss connue (0x83... ), ou une zone de la RAM.
fn paliers(c: &mut Cpu, b: &mut Bus, a: &mut Alea) {
    let t = 0xffe000;
    for k in 0..0x400u32 {
        wr16(b, t + 2 * k, a.dans(0x400) as u16);
    }
    c.a[1] = t;
}

fn attaque(c: &mut Cpu, _: &mut Bus, a: &mut Alea) {
    c.d[0] = a.dans(40);
}

fn saisie(c: &mut Cpu, _: &mut Bus, a: &mut Alea) {
    c.a[1] = A5 + 0x5de + 0x200 * a.dans(4);
    c.d[0] = a.next() & 0xff;
}

fn joueur_a1(c: &mut Cpu, _: &mut Bus, a: &mut Alea) {
    c.a[1] = (A5 + 0x5de + 0x200 * a.dans(4)) | 0xff00_0000;
}

fn parade(c: &mut Cpu, b: &mut Bus, a: &mut Alea) {
    // agresseur A0 = un joueur, victime A1 = l'objet
    let o = c.a[0];
    c.a[1] = o;
    c.a[0] = A5 + 0x5de + 0x200 * a.dans(4);
    wr8(b, o + 0x67, a.dans(3) as u8);
    wr8(b, o + 0x68, a.dans(8) as u8);
    wr8(b, o + 0x69, a.dans(8) as u8);
}

fn jeu_debris(c: &mut Cpu, _: &mut Bus, a: &mut Alea) {
    c.d[5] = a.dans(16);
}

fn debris_sol(c: &mut Cpu, _: &mut Bus, a: &mut Alea) {
    c.d[0] = c.d[0] & !0xff | a.dans(2);
}

fn jeu_animation(c: &mut Cpu, b: &mut Bus, a: &mut Alea) {
    // jeu d'animation : celui que l'objet joue (adresse de l'entrée courante, alignée sur le début du jeu inconnu)
    // ; on prend la table d'animations du squelette (0x1c74b6 joue la marche) et un maître = l'objet lui-même
    let o = c.a[0];
    c.a[4] = 0x1c74b6;
    wr16(b, o + 0xb2, o as u16);
    wr8(b, o + 0x33, a.dans(4) as u8);
    let marque = rd8(b, 0x1c74b6 + 3 + 16 * a.dans(4));
    wr8(b, o + 0x3e, marque);
}

fn vitesses(c: &mut Cpu, _: &mut Bus, a: &mut Alea) {
    c.d[2] = a.next();
    c.d[3] = a.next();
}

fn cas() -> Vec<Cas> {
    vec![
        Cas { entree: 0x424e, nom: "pick_target", entrees: aucune, coeur: |_, b, o| obj(ai::pick_target(b, o, ai::Rule::Strict)), lecture: a1_si },
        Cas { entree: 0x43ae, nom: "pick_target_any", entrees: aucune, coeur: |_, b, o| obj(ai::pick_target(b, o, ai::Rule::Lenient)), lecture: a1_si },
        Cas {
            entree: 0x43bc,
            nom: "target_by",
            entrees: critere,
            coeur: |c, b, o| obj(ai::target_by(b, o, ai::Criterion::from_byte(c.d[0] as u8), ai::Rule::Lenient)),
            lecture: a1_si,
        },
        Cas { entree: 0x44ee, nom: "random_player", entrees: aucune, coeur: |_, b, o| obj(ai::random_player(b, o)), lecture: a1_si },
        Cas { entree: 0x453c, nom: "initial_hp_boss", entrees: aucune, coeur: |_, b, o| { ai::initial_hp(b, o, ai::HP_TABLE_BOSS); 0 }, lecture: rien },
        Cas { entree: 0x4544, nom: "initial_hp", entrees: aucune, coeur: |_, b, o| { ai::initial_hp(b, o, ai::HP_TABLE_ENEMY); 0 }, lecture: rien },
        Cas { entree: 0xa4b4, nom: "decision_a", entrees: aucune, coeur: |_, b, o| ai::decision_a(b, o) as u32, lecture: nz },
        Cas { entree: 0xa4c2, nom: "decision_b", entrees: aucune, coeur: |_, b, o| ai::decision_b(b, o) as u32, lecture: nz },
        Cas { entree: 0xac3a, nom: "pick_byte", entrees: aucune, coeur: |_, b, o| { ai::pick_byte(b, o); 0 }, lecture: rien },
        Cas { entree: 0xfe5a, nom: "set_immunities", entrees: aucune, coeur: |_, b, o| { ai::set_immunities(b, o); 0 }, lecture: rien },
        Cas { entree: 0x1059e, nom: "hp_rank", entrees: aucune, coeur: |_, b, o| { ai::hp_rank(b, o); 0 }, lecture: rien },
        Cas {
            entree: 0xb5e0,
            nom: "target_in_front",
            entrees: boite,
            coeur: |c, b, o| {
                let boite = ai_aids::Boite { decalage: c.d[0] as u16, demi_largeur: c.d[1] as u16, demi_profondeur: c.d[2] as u16 };
                ai::target_in_front(b, o, boite) as u32
            },
            lecture: d0,
        },
        Cas { entree: 0x1050c, nom: "facing_target", entrees: aucune, coeur: |_, b, o| ai::facing_target(b, o) as u32, lecture: z },
        Cas { entree: 0x1052e, nom: "target_faces_me", entrees: aucune, coeur: |_, b, o| ai::target_faces_me(b, o) as u32, lecture: d0 },
        Cas { entree: 0x104e0, nom: "back_to_target", entrees: aucune, coeur: |_, b, o| ai::back_to_target(b, o) as u32, lecture: nz },
        Cas { entree: 0x1047a, nom: "turn_decision", entrees: aucune, coeur: |_, b, o| ai::turn_decision(b, o) as u32, lecture: d0 },
        Cas { entree: 0x10560, nom: "moving_forward", entrees: aucune, coeur: |_, b, o| ai::moving_forward(b, o) as u32, lecture: z },
        Cas { entree: 0xfbae, nom: "keep_in_zone", entrees: aucune, coeur: |_, b, o| ai::keep_in_zone(b, o) as u32, lecture: d5 },
        Cas { entree: 0xfbb0, nom: "keep_on_screen", entrees: marge, coeur: |c, b, o| ai::keep_on_screen(b, o, c.d[0] as u16) as u32, lecture: d5 },
        Cas { entree: 0xf0e8, nom: "turn_toward", entrees: point, coeur: |c, b, o| { ai::turn_toward(b, o, (c.d[1] as u16, c.d[2] as u16)); 0 }, lecture: rien },
        Cas { entree: 0x10574, nom: "drift_heading", entrees: aucune, coeur: |_, b, o| { ai::drift_heading(b, o); 0 }, lecture: rien },
        Cas { entree: 0xe76c, nom: "approach_target", entrees: aucune, coeur: |_, b, o| { ai_aids::approach_target(b, o); 0 }, lecture: rien },
        Cas { entree: 0xe7fa, nom: "choose_near_walk_point", entrees: aucune, coeur: |_, b, o| { ai_aids::choose_near_walk_point(b, o); 0 }, lecture: rien },
        Cas { entree: 0xe886, nom: "choose_walk_point", entrees: aucune, coeur: |_, b, o| { ai_aids::choose_walk_point(b, o); 0 }, lecture: rien },
        Cas { entree: 0xe914, nom: "walk_point_reached", entrees: aucune, coeur: |_, b, o| { ai_aids::walk_point_reached(b, o); 0 }, lecture: rien },
        Cas { entree: 0xeff2, nom: "walk_near_point", entrees: aucune, coeur: |_, b, o| { ai_aids::walk_near_point(b, o); 0 }, lecture: rien },
        Cas { entree: 0xf084, nom: "walk_far_point", entrees: aucune, coeur: |_, b, o| { ai_aids::walk_far_point(b, o); 0 }, lecture: rien },
        Cas { entree: 0x103e0, nom: "animation_by_variant", entrees: jeu_animation, coeur: |c, b, o| { ai_aids::animation_by_variant(b, o, c.a[4]); 0 }, lecture: rien },
        Cas { entree: 0x10428, nom: "animation_of_master", entrees: jeu_animation, coeur: |c, b, o| { ai_aids::animation_of_master(b, o, c.a[4]); 0 }, lecture: rien },
        Cas { entree: 0x10450, nom: "animation_of_mark", entrees: jeu_animation, coeur: |c, b, o| { ai_aids::animation_of_mark(b, o, c.a[4]); 0 }, lecture: rien },
        Cas { entree: 0x10474, nom: "players_alive", entrees: aucune, coeur: |_, b, _| ai_aids::players_alive(b) as u32, lecture: nz },
        Cas {
            entree: 0x104a2,
            nom: "outside_band",
            entrees: bande,
            coeur: |c, b, o| ai_aids::outside_band(b, o, c.d[0] as u16, c.d[1] as u16) as u32,
            lecture: d0,
        },
        Cas { entree: 0x105b6, nom: "next_threshold", entrees: paliers, coeur: |c, b, o| ai_aids::next_threshold(b, o, c.a[1]) as u32, lecture: d0 },
        Cas {
            entree: 0xfc44,
            nom: "attack_hits_a_player",
            entrees: attaque,
            coeur: |c, b, o| obj(ai_aids::attack_hits_a_player(b, o, c.d[0] as u8)),
            lecture: |c| if c.d[0] != 0 { c.a[1] & 0xffffff } else { 0 },
        },
        Cas {
            entree: 0xfd64,
            nom: "seize_player",
            entrees: saisie,
            coeur: |c, b, o| { ai_aids::seize_player(b, o, Obj(c.a[1]), c.d[0] as u8); 0 },
            lecture: rien,
        },
        Cas { entree: 0xfd8c, nom: "release_player", entrees: joueur_a1, coeur: |c, b, _| { ai_aids::release_player(b, Obj(c.a[1] & 0xffffff)); 0 }, lecture: rien },
        Cas { entree: 0xfda2, nom: "hold_seized", entrees: aucune, coeur: |_, b, o| { ai_aids::hold_seized(b, o); 0 }, lecture: rien },
        Cas { entree: 0xff54, nom: "countdown_then_state", entrees: aucune, coeur: |_, b, o| { ai_aids::countdown_then_state(b, o); 0 }, lecture: rien },
        Cas { entree: 0xfffc, nom: "behaviour_by_kind", entrees: aucune, coeur: |_, b, o| { ai_aids::behaviour_by_kind(b, o); 0 }, lecture: rien },
        Cas { entree: 0x788c, nom: "draw_6b", entrees: aucune, coeur: |_, b, o| { ai_aids::draw_6b(b, o); 0 }, lecture: rien },
        Cas { entree: 0x79f6, nom: "draw_6e", entrees: aucune, coeur: |_, b, o| { ai_aids::draw_6e(b, o); 0 }, lecture: rien },
        Cas { entree: 0x79ce, nom: "maybe_draw_6e", entrees: aucune, coeur: |_, b, o| ai_aids::maybe_draw_6e(b, o) as u32, lecture: d0 },
        Cas {
            entree: 0xb048,
            nom: "parries_a",
            entrees: parade,
            coeur: |c, b, _| ai_aids::parries(b, Obj(c.a[0]), Obj(c.a[1]), ai_aids::Parry::A) as u32,
            lecture: d0,
        },
        Cas {
            entree: 0xb068,
            nom: "parries_b",
            entrees: parade,
            coeur: |c, b, _| ai_aids::parries(b, Obj(c.a[0]), Obj(c.a[1]), ai_aids::Parry::B) as u32,
            lecture: d0,
        },
        Cas { entree: 0xf118, nom: "draw_84", entrees: aucune, coeur: |_, b, o| { ai_aids::draw_84(b, o); 0 }, lecture: rien },
        Cas { entree: 0x10112, nom: "set_68", entrees: aucune, coeur: |_, b, o| { ai_aids::set_68(b, o); 0 }, lecture: rien },
        Cas { entree: 0x115e2, nom: "push_players", entrees: aucune, coeur: |_, b, o| { ai_aids::push_players(b, o); 0 }, lecture: rien },
        Cas { entree: 0x1172e, nom: "debris_set", entrees: jeu_debris, coeur: |c, b, o| { ai_aids::debris_set(b, o, c.d[5] as u16); 0 }, lecture: rien },
        Cas { entree: 0x11a36, nom: "ground_debris", entrees: debris_sol, coeur: |c, b, o| { ai_aids::ground_debris(b, o, c.d[0] as u8 != 0); 0 }, lecture: rien },
        Cas { entree: 0xb648, nom: "drop_once", entrees: aucune, coeur: |_, b, o| { ai_aids::drop_once(b, o); 0 }, lecture: rien },
        Cas { entree: 0xb780, nom: "drop_loot", entrees: aucune, coeur: |_, b, o| { ai_aids::drop_loot(b, o); 0 }, lecture: rien },
        Cas {
            entree: 0xbad6,
            nom: "drop_special",
            entrees: aucune,
            coeur: |c, b, o| ai_aids::drop_special(b, o).unwrap_or(c.d[3] as u16) as u32,
            lecture: |c| c.d[3] as u16 as u32,
        },
        Cas { entree: 0xe2f6, nom: "drop_treasure", entrees: aucune, coeur: |_, b, o| ai_aids::drop_treasure(b, o) as u32, lecture: |c| c.d[2] as u16 as u32 },
        Cas { entree: 0x57c66, nom: "in_right_half", entrees: aucune, coeur: |_, b, o| ddsom_game::families::creators::in_right_half(b, o) as u32, lecture: d0 },
        Cas { entree: 0x5cdc6, nom: "chance_10_16", entrees: aucune, coeur: |_, b, _| ddsom_game::families::creators::chance_10_16(b) as u32, lecture: d0 },
        Cas { entree: 0x19eb0, nom: "popups_effacer_tout", entrees: aucune, coeur: |_, b, _| { ddsom_game::popups::effacer_tout(b); 0 }, lecture: rien },
        Cas { entree: 0x19e20, nom: "popups_animer", entrees: aucune, coeur: |_, b, _| { ddsom_game::popups::animer(b); 0 }, lecture: rien },
        Cas { entree: 0xd5f30, nom: "set_14", entrees: aucune, coeur: |_, b, o| { ddsom_game::families::creators::set_14(b, o); 0 }, lecture: rien },
        Cas { entree: 0xd6952, nom: "set_15_b", entrees: aucune, coeur: |_, b, o| { ddsom_game::families::creators::set_15_b(b, o); 0 }, lecture: rien },
        Cas { entree: 0xd5ef8, nom: "attach_life_bar", entrees: aucune, coeur: |_, b, o| { ddsom_game::families::set_residu::attach_life_bar(b, o); 0 }, lecture: rien },
        Cas {
            entree: 0x103ba,
            nom: "flash_and_shake",
            entrees: vitesses,
            coeur: |c, b, o| { ai_aids::flash_and_shake(b, o, c.d[2] as u16, c.d[3] as u16); 0 },
            lecture: rien,
        },
        Cas {
            entree: 0xa09ca,
            nom: "lib_eshl_12",
            entrees: aucune,
            coeur: |c, b, _| {
                let mut x = Cx::new(c, b);
                let (o, retenue) = lib::eshl_12(&mut x);
                (o.is_some() as u32) << 1 | retenue as u32
            },
            lecture: |c| ((c.a[1] != 0) as u32) << 1 | (c.sr & m68k::SR_C != 0) as u32,
        },
        Cas { entree: 0xa0ae0, nom: "lib_eshl_13", entrees: aucune, coeur: |c, b, _| { let t = c.a[2]; lib::eshl_13(&mut Cx::new(c, b), t); 0 }, lecture: rien },
        Cas { entree: 0xa114a, nom: "lib_eshl_16", entrees: debris_sol, coeur: |c, b, _| { let v = c.d[0] as u8; lib::eshl_16(&mut Cx::new(c, b), v); 0 }, lecture: rien },
        Cas { entree: 0x9f346, nom: "lib_eshl_0a", entrees: aucune, coeur: |c, b, _| { lib::eshl_0a(&mut Cx::new(c, b)); 0 }, lecture: rien },
        Cas { entree: 0xa2d22, nom: "lib_eshl_burst_26", entrees: aucune, coeur: |c, b, _| { lib::eshl_burst_26(&mut Cx::new(c, b)); 0 }, lecture: rien },
        Cas { entree: 0xffda, nom: "lib_roll_ffda", entrees: aucune, coeur: |c, b, _| lib::roll_ffda(&mut Cx::new(c, b)) as u32, lecture: nz },
        Cas { entree: 0xad7c, nom: "lib_roll_78", entrees: aucune, coeur: |c, b, _| lib::roll_78(&mut Cx::new(c, b)) as u32, lecture: d0 },
        Cas { entree: 0x10298, nom: "lib_palette_script", entrees: debris_sol, coeur: |c, b, _| { let i = c.d[0] as u16; lib::palette_script(&mut Cx::new(c, b), i); 0 }, lecture: rien },
    ]
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 6 {
        eprintln!("usage : fuzz_ia <roms> <eeprom> <scénario> <images de chauffe> <tours> [entrées...]");
        std::process::exit(2);
    }
    let (roms, eeprom, scenario) = (&a[1], &a[2], &a[3]);
    let chauffe: u32 = a[4].parse().unwrap();
    let tours: u32 = a[5].parse().unwrap();
    let filtre: Vec<u32> = a[6..].iter().map(|e| u32::from_str_radix(e.trim_start_matches("0x"), 16).unwrap()).collect();
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..chauffe as usize {
        script.apply(f, &mut m);
        m.run_frame();
    }
    let graine: u64 = std::env::var("DDSOM_GRAINE").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut alea = Alea(0x2545_f491_4f6c_dd1d ^ graine.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let (c0, mut b0) = (m.cpu.clone(), m.bus.clone());
    b0.irq_horizon = u64::MAX;
    let objets = acteurs(&mut b0);
    if objets.is_empty() {
        eprintln!("aucun ennemi ni boss actif à l'image {chauffe}");
        std::process::exit(2);
    }
    let verbeux = std::env::var("DDSOM_VERBOSE").is_ok();
    let mut mauvais = 0;
    for cas in cas() {
        if !filtre.is_empty() && !filtre.contains(&cas.entree) {
            continue;
        }
        let (mut ecarts, mut premier, mut sautes) = (0, None, 0);
        for _ in 0..tours {
            let (mut c, mut b) = (c0.clone(), b0.clone());
            let o = alea.parmi(&objets);
            perturbe(&mut c, &mut b, &mut alea, o);
            (cas.entrees)(&mut c, &mut b, &mut alea);
            let (mut c2, mut b2) = (c.clone(), b.clone());
            let attendu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                original(&mut c2, &mut b2, cas.entree);
                (cas.lecture)(&c2)
            }));
            let Ok(attendu) = attendu else {
                sautes += 1;
                continue;
            };
            let a0 = Obj(c.a[0]);
            let obtenu = (cas.coeur)(&mut c, &mut b, a0);
            let mut diffs = Vec::new();
            if obtenu != attendu {
                diffs.push(format!("résultat {obtenu:x} au lieu de {attendu:x}"));
            }
            let sp = c2.a[7] & 0xff_ffff;
            ddsom_game::mem::diff_bus(&b, &b2, sp.saturating_sub(0x400)..sp + 4, &mut diffs);
            if !diffs.is_empty() {
                ecarts += 1;
                let msg = format!("A0={:06x} : {}", a0.0, diffs.join(", "));
                if verbeux {
                    eprintln!("[{}] {msg}", cas.nom);
                }
                premier.get_or_insert(msg);
            }
        }
        println!("{:06x} {:24} {:6} tours {:6} écarts {:4} sautés {}", cas.entree, cas.nom, tours, ecarts, sautes, premier.as_deref().unwrap_or(""));
        mauvais += ecarts;
    }
    println!("{}", if mauvais == 0 { "FUZZ OK" } else { "FUZZ ÉCARTS" });
}
